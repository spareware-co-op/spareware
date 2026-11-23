//! Parameters, categories and types through the edit log, on the test store: the commands, the
//! records they write, the shapes built from them, and the log replayed. Shape tests run on two
//! generators, as `shapes.rs` does, so nothing here relies on what a shape is.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::{COLUMNS, CategoryTable, DOORS, GENERIC_MODELS, WALLS};
use construct_model::element::Element;
use construct_model::geometry::{
    Bounds, Built, Display, Edition, FaceName, Geometry, GeometryError, Mesh, Recipe,
};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_params::{Definition, Holder, Kind, ProjectError, Units};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_types::{ElementType, Parameters, TypeError};
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Reader, Scope, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply, read_log, verify_log};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// Says yes. Access has tests of its own, in the server, against the real rules.
struct Yes;

impl Decider for Yes {
    fn decide(&self, _: &Question) -> Result<bool, String> {
        Ok(true)
    }
}

/// The orderer's time, set by the test, so a replay can run every edit at its recorded time.
struct Fixed(AtomicU64);

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// A second generator: every shape is a wedge as long as its `width` or `length`.
struct Wedges;

impl Geometry for Wedges {
    fn edition(&self) -> Edition {
        Edition {
            engine: "test.wedges".to_owned(),
            version: 7,
            name_scheme: 2,
            report: None,
        }
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        let arg = |name: &str| recipe.args.get(name).and_then(Value::as_i64);
        let long = arg("length").or(arg("width")).ok_or(GeometryError {
            code: "NoSize".to_owned(),
            message: "wedges need a length".to_owned(),
        })?;
        let mesh = Mesh {
            positions: vec![0, 0, 0, long, 0, 0, 0, long, 0],
            indices: vec![0, 1, 2],
        };
        Ok(Built {
            body: format!("wedge {long} {}", arg("thickness").unwrap_or(0)).into_bytes(),
            bounds: Bounds {
                min: [0, 0, 0],
                max: [long, long, 1],
            },
            names: vec![FaceName {
                name: format!("slope.{long}"),
                role: "exterior".to_owned(),
            }],
            displays: vec![Display {
                lod: 0,
                deviation: 0,
                mesh,
                outline: vec![[0, 0], [long, 0], [0, long]],
                parts: Vec::new(),
            }],
        })
    }
}

fn generators() -> [Arc<dyn Geometry>; 2] {
    [Arc::new(Boxes), Arc::new(Wedges)]
}

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    regen: Regenerator<Memory, MemoryFiles>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    /// A store with the rows the directory would have written, and no model yet.
    async fn bare(geometry: Arc<dyn Geometry>) -> Self {
        let store = Memory::new();
        let files = MemoryFiles::default();
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, 30_000, geometry.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
        let regen = Regenerator::new(store.clone(), files, geometry);
        let mut txn = store.begin().await.unwrap();
        let project = Project {
            id: i("p"),
            name: "P".to_owned(),
            hub: i("h"),
        };
        txn.put(Place::Project(i("p")).key(), to_row(&project).unwrap())
            .await
            .unwrap();
        let team = Team {
            id: i("t"),
            name: "T".to_owned(),
        };
        let place = Place::Team {
            project: i("p"),
            team: i("t"),
        };
        txn.put(place.key(), to_row(&team).unwrap()).await.unwrap();
        txn.commit().await.unwrap();
        Self {
            store,
            orderers,
            regen,
            clock,
            next: AtomicU64::new(0),
        }
    }

    async fn new(geometry: Arc<dyn Geometry>) -> Self {
        let stack = Self::bare(geometry).await;
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        stack
    }

    async fn run(&self, edit: Edit) -> Result<u64, String> {
        match self
            .orderers
            .run(&Scope::Project(i("p")), edit)
            .await
            .unwrap()
        {
            Reply::Ack { seq, .. } => Ok(seq),
            Reply::Rejected { reason, .. } => Err(reason),
            Reply::Retry { .. } => Err("retry".to_owned()),
        }
    }

    async fn edit_as(&self, person: &str, cmd: &str, args: Value) -> Result<u64, String> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        self.clock.0.fetch_add(10, Ordering::SeqCst);
        let edit = Edit {
            person: i(person),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(DOCUMENT),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        };
        self.run(edit).await
    }

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
        self.edit_as("anna", cmd, args).await
    }

    async fn record(&self, id: &str) -> Element {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let pointer = kit_store::Place::Piece {
            project: i("p"),
            document: i(DOCUMENT),
            piece: i(id),
        };
        let hash: String = read_row(&mut snapshot, &pointer.key())
            .await
            .unwrap()
            .unwrap();
        let record = kit_store::Place::Record {
            project: i("p"),
            hash: i(&hash),
        };
        read_row(&mut snapshot, &record.key())
            .await
            .unwrap()
            .unwrap()
    }

    async fn exists(&self, id: &str) -> bool {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let pointer = kit_store::Place::Piece {
            project: i("p"),
            document: i(DOCUMENT),
            piece: i(id),
        };
        snapshot.get(&pointer.key()).await.unwrap().is_some()
    }

    async fn built(&self, id: &str) -> Option<BuiltElement> {
        let place = Place::Built {
            project: i("p"),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_row(&mut snapshot, &place.key()).await.unwrap()
    }

    /// How wide an element's shape is along x, once regenerated.
    async fn width(&self, id: &str) -> i64 {
        self.regen.catch_up(&i("p")).await.unwrap();
        match self.built(id).await.unwrap().result {
            BuiltResult::Shape { bounds, .. } => bounds.max[0] - bounds.min[0],
            failed @ BuiltResult::Failed { .. } => panic!("{id}: {failed:?}"),
        }
    }

    async fn body(&self, id: &str) -> Id {
        self.regen.catch_up(&i("p")).await.unwrap();
        match self.built(id).await.unwrap().result {
            BuiltResult::Shape { body, .. } => body,
            failed @ BuiltResult::Failed { .. } => panic!("{id}: {failed:?}"),
        }
    }

    /// Every row under a table of the project, as bytes.
    async fn rows(&self, table: &str) -> BTreeMap<String, Vec<u8>> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let prefix = Scope::Project(i("p")).table_prefix(table, &[]);
        snapshot
            .scan(&prefix)
            .await
            .unwrap()
            .into_iter()
            .map(|(key, value)| (key.as_str().to_owned(), value))
            .collect()
    }

    async fn derived(&self) -> BTreeMap<String, Vec<u8>> {
        let mut rows = BTreeMap::new();
        for table in ["body", "display", "built", "area", "kind", "reads"] {
            rows.extend(self.rows(table).await);
        }
        rows
    }
}

fn recipe(op: &str, args: Value) -> Value {
    serde_json::to_value(Recipe::construct(op, serde_json::from_value(args).unwrap())).unwrap()
}

/// A door whose width is its type's, whose thickness is its own, and whose height is a number.
fn door(id: &str, ty: &str) -> Value {
    let args = json!({ "width": { "param": "P-width" }, "thickness": { "param": "P-thick" },
                       "height": 2100 });
    json!({ "element": id, "kind": "door", "level": 1, "type": ty,
            "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe("door.single", args),
            "props": { "fire rating": "FD30" } })
}

fn def(id: &str, name: &str, kind: &str, holder: &str) -> Value {
    json!({ "definition": { "id": id, "name": name, "kind": kind, "holder": holder,
                            "choices": [] } })
}

/// Doors with a width on the type (900 unless set), a thickness on each (40 unless set), a mark
/// and a sill height on each; a door type `T-820` of width 820, and door `D-1` of it.
async fn doors(stack: &Stack) {
    for (args, categories, default) in [
        (
            def("P-width", "Width", "length", "type"),
            json!([300]),
            json!("900"),
        ),
        (
            def("P-thick", "Thickness", "length", "instance"),
            json!([300]),
            json!("40 mm"),
        ),
        (
            def("P-mark", "Mark", "text", "instance"),
            json!([100, 300]),
            Value::Null,
        ),
        (
            def("P-sill", "Sill height", "length", "instance"),
            json!([300]),
            Value::Null,
        ),
    ] {
        let id = args["definition"]["id"].clone();
        stack.edit("parameter.define", args).await.unwrap();
        stack
            .edit(
                "parameter.bind",
                json!({ "parameter": id, "categories": categories, "default": default }),
            )
            .await
            .unwrap();
    }
    stack
        .edit(
            "type.create",
            json!({ "type": "T-820", "name": "Single 820", "category": 300 }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "type.set",
            json!({ "type": "T-820", "parameter": "P-width", "value": "820" }),
        )
        .await
        .unwrap();
    stack
        .edit("element.create", door("D-1", "T-820"))
        .await
        .unwrap();
}

/// The parameters as `doors` defines them, for building what construct-types would write.
fn door_parameters() -> Parameters {
    let mut params = Parameters::new();
    let length = construct_params::Value::Length;
    for (id, name, kind, holder, default) in [
        ("P-mark", "Mark", Kind::Text, Holder::Instance, None),
        (
            "P-sill",
            "Sill height",
            Kind::Length,
            Holder::Instance,
            None,
        ),
        (
            "P-thick",
            "Thickness",
            Kind::Length,
            Holder::Instance,
            Some(length(construct_params::Length::from_mm(40))),
        ),
        (
            "P-width",
            "Width",
            Kind::Length,
            Holder::Type,
            Some(length(construct_params::Length::from_mm(900))),
        ),
    ] {
        let definition = Definition::new(i(id), name, kind, holder).unwrap();
        params.add(definition, default).unwrap();
    }
    params
}

fn wall(id: &str) -> Value {
    let args = json!({ "length": 3000, "thickness": 200, "height": 2700 });
    json!({ "element": id, "kind": "wall", "level": 1, "placement": { "x": 0, "y": 0, "z": 0 },
            "recipe": recipe("wall.straight", args) })
}

#[tokio::test]
async fn every_record_carries_its_category_and_a_type_is_construct_types_record() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    stack.edit("element.create", wall("W-1")).await.unwrap();

    let wall = stack.record("W-1").await;
    assert_eq!(wall.props["@category"], WALLS.0.to_string());
    assert!(!wall.props.contains_key("@type"));
    let door = stack.record("D-1").await;
    assert_eq!(door.props["@category"], DOORS.0.to_string());
    assert_eq!(door.props["@type"], "T-820");
    assert_eq!(door.props["fire rating"], "FD30", "other keys are kept");

    // The type record is exactly what construct-types writes for this type.
    let params = door_parameters();
    let categories = CategoryTable::new();
    let mut ty = ElementType::new(i("T-820"), "Single 820", DOORS, &categories).unwrap();
    for id in ["P-mark", "P-sill", "P-thick", "P-width"] {
        ty.add_parameter(&params, &i(id)).unwrap();
    }
    let width = construct_params::Value::Length(construct_params::Length::from_mm(820));
    ty.set(&params, &i("P-width"), width).unwrap();
    assert_eq!(stack.record("T-820").await, ty.to_record(&i(DOCUMENT)));

    // Workshop's kinds have a category; a kind with none is refused with construct-categories'
    // own words.
    let part = json!({ "owner": "workshop", "schema": 1, "op": "part", "op_version": 1,
                       "args": { "end": 0, "features": [], "material": "steel", "params": [] } });
    let create = |id: &str, kind: &str, recipe: Value| {
        json!({ "element": id, "kind": kind, "level": 0,
                "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe })
    };
    stack
        .edit("element.create", create("WP-1", "workshop.part", part))
        .await
        .unwrap();
    assert_eq!(
        stack.record("WP-1").await.props["@category"],
        GENERIC_MODELS.0.to_string()
    );
    let teapot = recipe("teapot", json!({}));
    assert_eq!(
        stack
            .edit("element.create", create("X-1", "thing", teapot))
            .await,
        Err(construct_categories::category_of_kind("teapot")
            .unwrap_err()
            .to_string())
    );
}

#[tokio::test]
async fn a_shape_reads_its_values_through_its_type_and_the_defaults() {
    for geometry in generators() {
        let stack = Stack::new(geometry).await;
        doors(&stack).await;
        assert_eq!(stack.width("D-1").await, 820, "its type's width");

        // A type with no width of its own gives its doors the default.
        stack
            .edit(
                "type.create",
                json!({ "type": "T-std", "name": "Standard", "category": 300 }),
            )
            .await
            .unwrap();
        stack
            .edit("element.create", door("D-2", "T-std"))
            .await
            .unwrap();
        assert_eq!(stack.width("D-2").await, 900, "the default");

        // Changing the type rebuilds its doors, and only those.
        let other = stack.body("D-2").await;
        stack
            .edit(
                "type.set",
                json!({ "type": "T-820", "parameter": "P-width", "value": "1.2 m" }),
            )
            .await
            .unwrap();
        assert_eq!(stack.width("D-1").await, 1200);
        assert_eq!(stack.body("D-2").await, other);

        // An instance's own value comes before the default.
        let thin = stack.body("D-1").await;
        stack
            .edit(
                "element.param",
                json!({ "element": "D-1", "parameter": "P-thick", "value": "55" }),
            )
            .await
            .unwrap();
        assert_ne!(stack.body("D-1").await, thin);

        // Moving a door to another type rebuilds it with that type's width.
        stack
            .edit("element.type", json!({ "element": "D-1", "type": "T-std" }))
            .await
            .unwrap();
        assert_eq!(stack.width("D-1").await, 900);

        // A new default reaches every door that reads it.
        stack
            .edit(
                "parameter.bind",
                json!({ "parameter": "P-width", "categories": [300], "default": "950" }),
            )
            .await
            .unwrap();
        assert_eq!(stack.width("D-2").await, 950);

        // Types and the table have no shape; everything derived comes back the same.
        stack.regen.catch_up(&i("p")).await.unwrap();
        assert!(stack.built("T-820").await.is_none());
        assert!(stack.built("parameters").await.is_none());
        let before = stack.derived().await;
        stack.regen.rebuild(&i("p")).await.unwrap();
        assert_eq!(stack.derived().await, before);
        assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 2);
        let kinds = stack.rows("kind").await;
        assert!(
            kinds
                .keys()
                .any(|key| key.ends_with("/kind/doc/type/T-820"))
        );
        assert!(
            kinds
                .keys()
                .any(|key| key.ends_with("/kind/doc/parameters/parameters"))
        );
    }
}

#[tokio::test]
async fn a_shape_that_reads_a_missing_value_is_a_failure_like_any_other() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    let args = json!({ "width": { "param": "P-sill" }, "thickness": 50, "height": 2100 });
    let unset = json!({ "element": "D-9", "kind": "door", "level": 1, "type": "T-820",
                        "placement": { "x": 0, "y": 0, "z": 0 },
                        "recipe": recipe("door.single", args) });
    stack.edit("element.create", unset).await.unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    let BuiltResult::Failed { code, .. } = stack.built("D-9").await.unwrap().result else {
        panic!("nothing holds a sill height");
    };
    assert_eq!(code, "UnsetParameter");
    assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 2);
}

#[tokio::test]
async fn commands_are_refused_with_the_data_crates_own_reasons() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    let units = Units::default();
    let thickness =
        Definition::new(i("P-thick"), "Thickness", Kind::Length, Holder::Instance).unwrap();
    let set = |element: &str, parameter: &str, value: &str| json!({ "element": element, "parameter": parameter, "value": value });

    // A wrong unit and a wrong kind of value, in construct-params' words.
    let wrong_unit = thickness.parse("2 m²", &units).unwrap_err().to_string();
    assert_eq!(
        stack
            .edit("element.param", set("D-1", "P-thick", "2 m²"))
            .await,
        Err(wrong_unit)
    );
    let not_a_number = thickness.parse("thick", &units).unwrap_err().to_string();
    assert_eq!(
        stack
            .edit("element.param", set("D-1", "P-thick", "thick"))
            .await,
        Err(not_a_number)
    );

    // Construct-types' rules: a type's value is not an instance's, a name is unique in its
    // category, a type in use stays, an element keeps to its category.
    assert_eq!(
        stack
            .edit("element.param", set("D-1", "P-width", "700"))
            .await,
        Err(TypeError::HeldByType {
            parameter: "Width".to_owned()
        }
        .to_string())
    );
    assert_eq!(
        stack
            .edit(
                "type.duplicate",
                json!({ "type": "T-820", "new": "T-2", "name": "single 820 " }),
            )
            .await,
        Err(TypeError::DuplicateName {
            category: DOORS,
            name: "single 820".to_owned()
        }
        .to_string())
    );
    assert_eq!(
        stack.edit("type.delete", json!({ "type": "T-820" })).await,
        Err(TypeError::InUse {
            type_name: "Single 820".to_owned(),
            count: 1
        }
        .to_string())
    );
    stack
        .edit(
            "type.create",
            json!({ "type": "T-column", "name": "Basic", "category": 1100 }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit(
                "element.type",
                json!({ "element": "D-1", "type": "T-column" })
            )
            .await,
        Err(TypeError::OtherCategory {
            from: DOORS,
            to: COLUMNS
        }
        .to_string())
    );

    // Construct-params' binding rules: a subcategory is never bound.
    let refusal = stack
        .edit(
            "parameter.bind",
            json!({ "parameter": "P-mark", "categories": [301] }),
        )
        .await
        .unwrap_err();
    let subcategory = ProjectError::SubcategoryNotBindable {
        category: construct_categories::CategoryId(301),
        parent: DOORS,
    };
    assert_eq!(refusal, subcategory.to_string());

    // A value is never set as plain text, nor a reserved key; a type is not an element.
    assert!(
        stack
            .edit(
                "element.set",
                json!({ "element": "D-1", "props": { "P-mark": "text:D01" } }),
            )
            .await
            .is_err()
    );
    assert!(
        stack
            .edit(
                "element.set",
                json!({ "element": "W-1", "props": { "@type": "T-column" } }),
            )
            .await
            .is_err()
    );
    assert!(
        stack
            .edit("element.delete", json!({ "element": "T-820" }))
            .await
            .is_err()
    );
    assert!(stack.exists("T-820").await);

    // Nothing that was refused changed a record.
    let door = stack.record("D-1").await;
    assert!(!door.props.contains_key("P-thick"));
    assert_eq!(door.props["@type"], "T-820");
}

#[tokio::test]
async fn a_set_replaces_that_parameters_entry_and_leaves_every_other_key() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    for mark in ["D01", "D02"] {
        stack
            .edit(
                "element.param",
                json!({ "element": "D-1", "parameter": "P-mark", "value": mark }),
            )
            .await
            .unwrap();
    }
    let door = stack.record("D-1").await;
    let mut want = BTreeMap::new();
    for (key, value) in [
        ("@category", "300"),
        // Made on level 1: tied to the level that integer names (construct-levels).
        ("@level", "1"),
        ("@level.offset", "length:q0:0"),
        ("@type", "T-820"),
        ("P-mark", "text:D02"),
        ("fire rating", "FD30"),
    ] {
        want.insert(key.to_owned(), value.to_owned());
    }
    assert_eq!(door.props, want);

    // An element with no type has the instance parameters its category is bound to.
    stack
        .edit(
            "element.param",
            json!({ "element": "W-1", "parameter": "P-mark", "value": "W01" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("W-1").await.props["P-mark"], "text:W01");
    assert!(
        stack
            .edit(
                "element.param",
                json!({ "element": "W-1", "parameter": "P-sill", "value": "10" }),
            )
            .await
            .is_err(),
        "walls have no sill height"
    );
}

/// Two edits of different parameters of one element both stand. Two of the same one follow the
/// existing rule for any edit: another person's claim on the element refuses it, and one
/// person's edits land in log order.
#[tokio::test]
async fn concurrent_sets_follow_the_claim_rule() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    let set = |parameter: &str, value: &str| json!({ "element": "D-1", "parameter": parameter, "value": value });
    stack
        .edit("element.param", set("P-mark", "D01"))
        .await
        .unwrap();
    stack
        .edit("element.param", set("P-sill", "150"))
        .await
        .unwrap();
    let door = stack.record("D-1").await;
    assert_eq!(door.props["P-mark"], "text:D01");
    assert_eq!(door.props["P-sill"], "length:q0:960000");

    // Ben, while Anna works on it.
    assert_eq!(
        stack
            .edit_as("ben", "element.param", set("P-mark", "B01"))
            .await,
        Err("anna is working on this.".to_owned())
    );
    assert_eq!(stack.record("D-1").await.props["P-mark"], "text:D01");

    // Once she lets go, his edit stands, and hers of the other parameter stays.
    stack
        .edit("element.release", json!({ "element": "D-1" }))
        .await
        .unwrap();
    stack
        .edit_as("ben", "element.param", set("P-mark", "B01"))
        .await
        .unwrap();
    let door = stack.record("D-1").await;
    assert_eq!(door.props["P-mark"], "text:B01");
    assert_eq!(door.props["P-sill"], "length:q0:960000");
}

#[tokio::test]
async fn types_are_renamed_duplicated_and_deleted_through_the_log() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    stack
        .edit(
            "type.duplicate",
            json!({ "type": "T-820", "new": "T-900", "name": "Single 900" }),
        )
        .await
        .unwrap();
    let copy = stack.record("T-900").await;
    assert_eq!(copy.name, "Single 900");
    assert_eq!(
        copy.props["P-width"], "length:q0:5248000",
        "a copy keeps the values"
    );
    stack
        .edit(
            "type.rename",
            json!({ "type": "T-900", "name": "Pair 900" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("T-900").await.name, "Pair 900");
    stack
        .edit("type.delete", json!({ "type": "T-900" }))
        .await
        .unwrap();
    assert!(!stack.exists("T-900").await);

    // A parameter bound later reaches every type of the category.
    stack
        .edit(
            "parameter.define",
            def("P-fire", "Fire rating", "text", "type"),
        )
        .await
        .unwrap();
    stack
        .edit(
            "parameter.bind",
            json!({ "parameter": "P-fire", "categories": [300] }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.record("T-820").await.props["@params"],
        "P-fire,P-mark,P-sill,P-thick,P-width"
    );
    stack
        .edit(
            "type.set",
            json!({ "type": "T-820", "parameter": "P-fire", "value": "FD60" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("T-820").await.props["P-fire"], "text:FD60");

    stack.regen.catch_up(&i("p")).await.unwrap();
    let kinds = stack.rows("kind").await;
    assert!(!kinds.keys().any(|key| key.ends_with("/T-900")));
    let mut snapshot = stack.store.snapshot().await.unwrap();
    verify_log(&mut snapshot, &Scope::Project(i("p")))
        .await
        .unwrap();
}

/// The log is the truth: every edit of it run again, in order, at its recorded time, on an empty
/// store gives the same records and pointers, byte for byte.
#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_records() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    for (cmd, args) in [
        (
            "element.param",
            json!({ "element": "D-1", "parameter": "P-mark", "value": "D01" }),
        ),
        (
            "element.param",
            json!({ "element": "W-1", "parameter": "P-mark", "value": "W01" }),
        ),
        (
            "type.duplicate",
            json!({ "type": "T-820", "new": "T-9", "name": "Wide" }),
        ),
        (
            "type.set",
            json!({ "type": "T-9", "parameter": "P-width", "value": "1 m" }),
        ),
        ("element.type", json!({ "element": "D-1", "type": "T-9" })),
        (
            "element.param",
            json!({ "element": "D-1", "parameter": "P-thick", "value": "2 m²" }),
        ),
    ] {
        let _ = stack.edit(cmd, args).await;
    }
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let head: u64 = read_row(&mut snapshot, &Scope::Project(i("p")).head())
        .await
        .unwrap()
        .unwrap();
    let entries = read_log(&mut snapshot, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert!(entries.iter().any(|entry| entry.failed.is_some()));

    let again = Stack::bare(Arc::new(Boxes)).await;
    for entry in &entries {
        again.clock.0.store(entry.orderer_ms, Ordering::SeqCst);
        let edit = Edit {
            person: entry.author.clone(),
            client: entry.client.clone(),
            counter: entry.counter,
            base: entry.base,
            target: entry.target.clone(),
            cmd: entry.cmd.clone(),
            args: entry.args.clone(),
            author_ms: entry.author_ms,
        };
        let _ = again.run(edit).await;
    }
    for table in ["record", "piece"] {
        assert_eq!(again.rows(table).await, stack.rows(table).await, "{table}");
    }
    let mut replayed = again.store.snapshot().await.unwrap();
    let after = read_log(&mut replayed, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert_eq!(after, entries);
}

/// A type and the parameter table are claimed by the commands that make them, and nothing else
/// gives the claim back before it runs out: so the person who holds one can let go of it, and
/// another person cannot, as for an element.
#[tokio::test]
async fn the_holder_lets_go_of_a_type_and_the_parameter_table_and_no_one_else_can() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    stack
        .edit(
            "type.duplicate",
            json!({ "type": "T-820", "new": "T-900", "name": "Single 900" }),
        )
        .await
        .unwrap();
    let define = |id: &str| def(id, "Rebate", "length", "instance");
    assert_eq!(
        stack
            .edit_as("ben", "parameter.define", define("P-rebate"))
            .await,
        Err("anna is working on this.".to_owned()),
        "while Anna holds the table, Ben's change is turned back"
    );

    for record in ["T-900", "parameters"] {
        let release = json!({ "element": record });
        assert_eq!(
            stack
                .edit_as("ben", "element.release", release.clone())
                .await,
            Err("anna is working on this.".to_owned()),
            "{record}: another person cannot let go of it"
        );
        stack
            .edit("element.release", release)
            .await
            .unwrap_or_else(|why| panic!("{record}: the holder lets go: {why}"));
    }

    // The parameter table was held by Anna, which turned Ben's change back; now it stands.
    stack
        .edit_as("ben", "parameter.define", define("P-rebate"))
        .await
        .unwrap();
    assert!(stack.exists("T-900").await);
}

#[tokio::test]
async fn letting_go_of_a_record_that_does_not_exist_is_refused() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    assert_eq!(
        stack
            .edit("element.release", json!({ "element": "T-none" }))
            .await,
        Err("That element no longer exists.".to_owned())
    );
}

#[tokio::test]
async fn every_other_command_on_a_record_with_no_shape_is_still_refused() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    doors(&stack).await;
    for record in ["T-820", "parameters"] {
        for (cmd, args) in [
            ("element.claim", json!({ "element": record })),
            ("element.delete", json!({ "element": record })),
            ("element.set", json!({ "element": record, "name": "x" })),
            (
                "element.move",
                json!({ "element": record, "placement": { "x": 0, "y": 0, "z": 0 } }),
            ),
        ] {
            let refused = stack.edit(cmd, args).await.unwrap_err();
            assert_eq!(
                refused,
                "Types, levels and the parameter table are changed by their own commands, not as elements.",
                "{cmd} on {record}"
            );
        }
    }
}
