//! Floor types and floors through the edit log, on the test store: the default floor types made
//! where the document lacks them, and floor types made and changed by their own commands; a floor
//! drawn from a closed boundary with a hole, of a chosen type, on a level, as the builders take it;
//! its boundary changed; a type edit and a level move building it again; the generic commands
//! refused where a floor's own exist; every refusal in words; undo and redo of every floor
//! command; and the log replayed from empty to the same bytes.

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

use construct_api::cloud::undo::{self, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::FLOORS;
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::parameters::{self as records, Context};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_slabs::{boundary_of, default_floors, layers_of};
use construct_types::ElementType;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Reader, Scope, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply, read_log};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
/// How long a claim lasts here, as the server's setting would.
const CLAIM_MS: u64 = 30_000;
const CONCRETE: &str = "floor-concrete-200";
const SCREED: &str = "floor-concrete-150-screed-50";
const TIMBER: &str = "floor-timber-joist-240";

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

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    regen: Regenerator<Memory, MemoryFiles>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    /// A store with the rows the directory would have written, and no model yet.
    async fn bare() -> Self {
        let store = Memory::new();
        let geometry: Arc<dyn Geometry> = Arc::new(Boxes);
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, CLAIM_MS, geometry.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
        let regen = Regenerator::new(store.clone(), MemoryFiles::default(), geometry);
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

    async fn new() -> Self {
        let stack = Self::bare().await;
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

    /// Lets every claim run out.
    fn later(&self) {
        self.clock.0.fetch_add(CLAIM_MS + 1, Ordering::SeqCst);
    }

    async fn record(&self, id: &str) -> Option<Element> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_element(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .map(|(_, record)| record)
    }

    async fn context(&self) -> Context {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_context(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .unwrap()
    }

    /// A type of the document, read as construct-types reads it.
    async fn floor_type(&self, id: &str) -> ElementType {
        let context = self.context().await;
        let record = self.record(id).await.unwrap();
        ElementType::from_record(&record, &context.params, &context.categories).unwrap()
    }

    async fn built(&self, id: &str) -> BuiltElement {
        self.regen.catch_up(&i("p")).await.unwrap();
        let place = Place::Built {
            project: i("p"),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap()
    }

    /// An element's built bounds, in whole millimetres, or its failure in words.
    async fn bounds(&self, id: &str) -> Result<([i64; 3], [i64; 3]), String> {
        match self.built(id).await.result {
            BuiltResult::Shape { bounds, .. } => Ok((bounds.min, bounds.max)),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
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

    async fn state(&self, person: &str) -> State {
        let mut snapshot = self.store.snapshot().await.unwrap();
        undo::state(&mut snapshot, &i("p"), &i(DOCUMENT), &i(person))
            .await
            .unwrap()
    }
}

/// A default floor type's layers, as `floor.type.create` and `floor.type.layers` take them: each
/// thickness as typed text.
fn layers(default: &str) -> Value {
    let floors = default_floors().unwrap();
    let floor = floors.iter().find(|floor| floor.id == default).unwrap();
    let layers: Vec<Value> = floor
        .layers
        .layers()
        .iter()
        .map(|layer| {
            json!({ "function": layer.function, "thickness": layer.thickness.to_string(),
                    "material": layer.material.as_str() })
        })
        .collect();
    Value::Array(layers)
}

/// A corner of a boundary as a command takes it: its edge's ID and where it is, in millimetres as
/// typed text.
fn at(id: &str, x: i64, y: i64) -> Value {
    json!({ "id": id, "at": [x.to_string(), y.to_string()] })
}

/// The L-shaped floor, 8 m by 8 m less its notch, with a 2 m square hole at (1 m, 1 m).
fn l_with_hole() -> Value {
    json!({
        "outer": [at("e0", 0, 0), at("e1", 8000, 0), at("e2", 8000, 4000), at("e3", 4000, 4000),
                  at("e4", 4000, 8000), at("e5", 0, 8000)],
        "holes": [[at("h0", 1000, 1000), at("h1", 3000, 1000), at("h2", 3000, 3000),
                   at("h3", 1000, 3000)]],
    })
}

/// A rectangle from (0, 0) to `x` by `y` mm, its edges `r0` to `r3`.
fn rectangle(x: i64, y: i64) -> Value {
    json!({ "outer": [at("r0", 0, 0), at("r1", x, 0), at("r2", x, y), at("r3", 0, y)] })
}

/// Ground at 0 and First at 3 m.
async fn two_levels(stack: &Stack) {
    for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
        let args = json!({ "level": id, "name": name, "elevation": elevation });
        stack.edit("level.create", args).await.unwrap();
    }
}

/// `floor.create`'s arguments: floor `id` of type `ty` on First, with `boundary`.
fn create(id: &str, ty: &str, boundary: &Value) -> Value {
    json!({ "element": id, "type": ty, "level": "L-1", "boundary": boundary })
}

/// The default floor types are made where the document lacks them, in one entry, each listing
/// every parameter bound to floors, and the parameter table is left as it is; asked again, nothing
/// is written and it is no step. A floor type is made, its layers changed, renamed, copied with
/// its layers and deleted only when no floor is of it, by its own commands, each refusal in words;
/// the generic type commands refuse a floor type, naming the floor command.
#[tokio::test]
async fn floor_types_are_made_and_changed_by_their_own_commands() {
    let stack = Stack::new().await;
    let note = json!({ "definition": { "id": "P-note", "name": "Note", "kind": "text",
                                       "holder": "type", "choices": [] } });
    stack.edit("parameter.define", note).await.unwrap();
    let bind = json!({ "parameter": "P-note", "categories": [FLOORS.0], "default": "none" });
    stack.edit("parameter.bind", bind).await.unwrap();
    let table = stack.record("parameters").await;
    stack.edit("floor.defaults", json!({})).await.unwrap();
    assert_eq!(stack.record("parameters").await, table);
    for default in default_floors().unwrap() {
        let ty = stack.floor_type(default.id).await;
        assert_eq!(ty.name(), default.name);
        assert_eq!(ty.category(), FLOORS);
        assert_eq!(layers_of(&ty).unwrap(), default.layers);
        assert!(
            ty.has(&i("P-note")),
            "{} lists the floors' parameters",
            default.id
        );
    }
    let made = stack.state("anna").await.undo;
    let rows = stack.rows("piece").await;
    stack.edit("floor.defaults", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, rows, "nothing more to make");
    assert_eq!(stack.state("anna").await.undo, made, "and no step");

    // A type of its own: its layers in its floor block; a name or ID taken, layers construct-slabs
    // refuses and text that is no length are refused in words.
    let own = json!({ "type": "F-own", "name": "Own", "layers": layers(TIMBER) });
    stack.edit("floor.type.create", own).await.unwrap();
    assert_eq!(
        layers_of(&stack.floor_type("F-own").await).unwrap(),
        layers_of(&stack.floor_type(TIMBER).await).unwrap()
    );
    for (args, words) in [
        (
            json!({ "type": "F-2", "name": "Concrete 200", "layers": layers(CONCRETE) }),
            "category 500 already has a type named \"Concrete 200\"",
        ),
        (
            json!({ "type": CONCRETE, "name": "Again", "layers": layers(CONCRETE) }),
            "That element already exists.",
        ),
        (
            json!({ "type": "F-3", "name": "Empty", "layers": [] }),
            "a floor type needs at least one layer",
        ),
        (
            json!({ "type": "F-4", "name": "Flat", "layers": [{ "function": "structure",
                    "thickness": "0", "material": "concrete-regular" }] }),
            "layer 0 is 0 thick; only a membrane may be 0",
        ),
        (
            json!({ "type": "F-5", "name": "Odd", "layers": [{ "function": "structure",
                    "thickness": "two hundred", "material": "concrete-regular" }] }),
            "",
        ),
    ] {
        let refused = stack.edit("floor.type.create", args).await.unwrap_err();
        if words.is_empty() {
            assert!(refused.starts_with("layer 0's thickness: "), "{refused}");
        } else {
            assert_eq!(refused, words);
        }
    }

    stack
        .edit(
            "floor.type.layers",
            json!({ "type": "F-own", "layers": layers(SCREED) }),
        )
        .await
        .unwrap();
    assert_eq!(
        layers_of(&stack.floor_type("F-own").await).unwrap(),
        layers_of(&stack.floor_type(SCREED).await).unwrap()
    );
    stack
        .edit(
            "floor.type.rename",
            json!({ "type": "F-own", "name": "Mine" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.floor_type("F-own").await.name(), "Mine");
    stack
        .edit(
            "floor.type.duplicate",
            json!({ "type": "F-own", "new": "F-copy", "name": "Copy" }),
        )
        .await
        .unwrap();
    assert_eq!(
        layers_of(&stack.floor_type("F-copy").await).unwrap(),
        layers_of(&stack.floor_type(SCREED).await).unwrap()
    );
    stack
        .edit("floor.type.delete", json!({ "type": "F-copy" }))
        .await
        .unwrap();
    assert!(stack.record("F-copy").await.is_none());
    two_levels(&stack).await;
    stack
        .edit(
            "floor.create",
            create("F-1", "F-own", &rectangle(4000, 3000)),
        )
        .await
        .unwrap();
    let refused = stack
        .edit("floor.type.delete", json!({ "type": "F-own" }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "type \"Mine\" cannot be deleted: 1 instances use it"
    );

    // The floor type commands refuse a type that is not a floor type.
    stack
        .edit(
            "type.create",
            json!({ "type": "T-door", "name": "Door", "category": 300 }),
        )
        .await
        .unwrap();
    for (cmd, args) in [
        (
            "floor.type.layers",
            json!({ "type": "T-door", "layers": layers(CONCRETE) }),
        ),
        (
            "floor.type.rename",
            json!({ "type": "T-door", "name": "D" }),
        ),
        (
            "floor.type.duplicate",
            json!({ "type": "T-door", "new": "T-d2", "name": "D2" }),
        ),
        ("floor.type.delete", json!({ "type": "T-door" })),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(
            refused, "a type of category 300 is not a floor type",
            "{cmd}"
        );
    }

    // The generic type commands do not go behind them.
    for (cmd, args, words) in [
        (
            "type.create",
            json!({ "type": "T-f", "name": "Plain floor", "category": FLOORS.0 }),
            "A floor type is made with floor.type.create, which checks it as a floor.",
        ),
        (
            "type.rename",
            json!({ "type": CONCRETE, "name": "C" }),
            "A floor type is renamed with floor.type.rename, which checks it as a floor.",
        ),
        (
            "type.duplicate",
            json!({ "type": CONCRETE, "new": "T-c2", "name": "C2" }),
            "A floor type is copied with floor.type.duplicate, which checks it as a floor.",
        ),
        (
            "type.delete",
            json!({ "type": CONCRETE }),
            "A floor type is deleted with floor.type.delete, which checks it as a floor.",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(refused, words, "{cmd}");
    }
    stack
        .edit("type.rename", json!({ "type": "T-door", "name": "Doors" }))
        .await
        .unwrap();

    // Ben may not change a floor type while Anna is working on it.
    let refused = stack
        .edit_as(
            "ben",
            "floor.type.layers",
            json!({ "type": "F-own", "layers": layers(CONCRETE) }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "anna is working on this.");
}

/// A floor is drawn from a closed boundary with a hole, of a chosen type, on a level: its record
/// holds its boundary in its stored form, its layers read from its type, its tie and its type, and
/// it is built from them; a change to its type's layers and a move of its level build it again,
/// its record unwritten. Every refusal is in words, a boundary's naming the loop and the edge,
/// and writes nothing; the generic element commands do not go behind it.
#[tokio::test]
async fn a_floor_is_drawn_from_a_closed_boundary_with_holes() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack.edit("floor.defaults", json!({})).await.unwrap();
    let args = json!({ "element": "F-1", "name": "First floor", "type": SCREED, "level": "L-1",
                       "offset": "-20 mm", "boundary": l_with_hole() });
    stack.edit("floor.create", args).await.unwrap();
    let floor = stack.record("F-1").await.unwrap();
    assert_eq!(floor.kind, "floor");
    assert_eq!(floor.name, "First floor");
    assert_eq!(floor.level, 0);
    assert_eq!(
        [floor.placement.x, floor.placement.y, floor.placement.z],
        [0, 0, 0]
    );
    assert_eq!(floor.recipe.op, "floor");
    assert_eq!(floor.recipe.args["layers"], json!({ "type": "layers" }));
    let boundary = boundary_of(&floor.recipe).unwrap();
    let outer: Vec<&str> = boundary.outer().iter().map(|s| s.id.as_str()).collect();
    assert_eq!(outer, ["e0", "e1", "e2", "e3", "e4", "e5"]);
    assert_eq!(boundary.holes().len(), 1);
    assert_eq!(floor.props["@type"], SCREED);
    assert_eq!(floor.props["@category"], records::write_category(FLOORS));
    assert_eq!(floor.props["@level"], "L-1");
    // The stand-in's box around the L, its top face at 0 and its 200 mm below; standing 20 mm
    // below First.
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([0, 0, -200], [8000, 8000, 0]))
    );
    let row = stack.built("F-1").await;
    assert_eq!(row.level, Some(i("L-1")));
    assert_eq!(row.elevation.as_deref(), Some("length:q0:19072000"));

    // Its type's layers changed: built again, 260 mm deep; its record is not written.
    stack
        .edit(
            "floor.type.layers",
            json!({ "type": SCREED, "layers": layers(TIMBER) }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("F-1").await.unwrap(), floor);
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([0, 0, -260], [8000, 8000, 0]))
    );
    // First moved up: the floor stands on it again; its record is not written.
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "3.2 m" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("F-1").await.unwrap(), floor);
    let row = stack.built("F-1").await;
    assert_eq!(row.elevation.as_deref(), Some("length:q0:20352000"));

    stack
        .edit(
            "type.create",
            json!({ "type": "T-wall", "name": "Wall", "category": 100 }),
        )
        .await
        .unwrap();
    let base = create("F-9", CONCRETE, &rectangle(1000, 1000));
    let with = |key: &str, value: Value| {
        let mut args = base.clone();
        if value.is_null() {
            args.as_object_mut().unwrap().remove(key);
        } else {
            args[key] = value;
        }
        args
    };
    let bow = json!({ "outer": [at("b0", 0, 0), at("b1", 1000, 1000), at("b2", 1000, 0),
                                at("b3", 0, 1000)] });
    let outside = json!({ "outer": rectangle(1000, 1000)["outer"],
                          "holes": [[at("x0", 2000, 2000), at("x1", 3000, 2000),
                                     at("x2", 3000, 3000)]] });
    let same = json!({ "outer": [at("s0", 0, 0), at("s0", 1000, 0), at("s2", 0, 1000)] });
    for (args, words) in [
        (with("type", json!("T-none")), "no type has id T-none"),
        (
            with("type", json!("T-wall")),
            "an instance of category 500 cannot take a type of category 100",
        ),
        (
            with("level", Value::Null),
            "A floor stands on a level: this command needs a level.",
        ),
        (
            with("level", json!("L-none")),
            "There is no level L-none in this model.",
        ),
        (
            with("element", json!("F-1")),
            "That element already exists.",
        ),
        (
            with(
                "boundary",
                json!({ "outer": [at("t0", 0, 0), at("t1", 1000, 0)] }),
            ),
            "F-9's boundary: the outer loop has 2 edges; a loop needs at least three",
        ),
        (
            with("boundary", outside),
            "F-9's boundary: hole 0 is outside the outer loop",
        ),
        (
            with("boundary", same),
            "F-9's boundary: two edges of the floor are both s0",
        ),
        (
            with("boundary", Value::Null),
            "This command needs a boundary.",
        ),
    ] {
        let refused = stack.edit("floor.create", args).await.unwrap_err();
        assert_eq!(refused, words);
    }
    let refused = stack
        .edit("floor.create", with("boundary", bow))
        .await
        .unwrap_err();
    assert!(
        refused.starts_with("F-9's boundary: the outer loop crosses or touches itself where edges"),
        "{refused}"
    );
    let typed = json!({ "outer": [{ "at": ["0", "0"] }, { "at": ["two", "0"] },
                                  { "at": ["0", "1"] }] });
    let refused = stack
        .edit("floor.create", with("boundary", typed))
        .await
        .unwrap_err();
    // Text that is no length is refused in construct-levels' words.
    let words = construct_levels::parse_length("two", &construct_params::Units::default())
        .unwrap_err()
        .to_string();
    assert_eq!(refused, words);
    assert!(stack.record("F-9").await.is_none());

    // Corners without IDs take the default ones.
    let plain = json!({ "outer": [{ "at": ["0", "0"] }, { "at": ["1 m", "0"] },
                                  { "at": ["1 m", "1 m"] }] });
    stack
        .edit("floor.create", with("boundary", plain))
        .await
        .unwrap();
    let defaulted = boundary_of(&stack.record("F-9").await.unwrap().recipe).unwrap();
    let ids: Vec<&str> = defaulted.outer().iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["o0", "o1", "o2"]);

    // The generic element commands do not go behind the floor's own.
    let recipe = floor.recipe.clone();
    for (cmd, args, words) in [
        (
            "element.create",
            json!({ "element": "F-x", "kind": "floor", "placement": { "x": 0, "y": 0, "z": 0 },
                    "recipe": recipe }),
            "A floor is made with floor.create, which checks it as a floor.",
        ),
        (
            "element.set",
            json!({ "element": "F-1", "recipe": recipe }),
            "First floor's boundary is changed with floor.boundary, which checks it as a floor.",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(refused, words, "{cmd}");
    }
    stack
        .edit(
            "element.set",
            json!({ "element": "F-1", "name": "Level one" }),
        )
        .await
        .unwrap();
}

/// A floor's boundary is changed by its own command: built again to the new one, its edges' IDs
/// its sides' names; a boundary construct-slabs refuses and an element that is not a floor are
/// refused in words, and another person's claim turns it back.
#[tokio::test]
async fn a_floors_boundary_is_changed_by_its_own_command() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack.edit("floor.defaults", json!({})).await.unwrap();
    stack
        .edit("floor.create", create("F-1", CONCRETE, &l_with_hole()))
        .await
        .unwrap();
    stack
        .edit(
            "floor.boundary",
            json!({ "element": "F-1", "boundary": rectangle(4000, 3000) }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([0, 0, -200], [4000, 3000, 0]))
    );
    let BuiltResult::Shape { names, .. } = stack.built("F-1").await.result else {
        panic!("F-1 builds");
    };
    let roles: Vec<&str> = names.iter().map(|n| n.role.as_str()).collect();
    assert_eq!(
        roles,
        ["top", "bottom", "side.r0", "side.r1", "side.r2", "side.r3"]
    );

    let crossing = json!({ "outer": rectangle(4000, 3000)["outer"],
                           "holes": [[at("k0", 3000, 1000), at("k1", 5000, 1000),
                                      at("k2", 5000, 2000)]] });
    let refused = stack
        .edit(
            "floor.boundary",
            json!({ "element": "F-1", "boundary": crossing }),
        )
        .await
        .unwrap_err();
    assert!(
        refused
            .starts_with("F-1's boundary: hole 0 crosses or touches the outer loop where its edge"),
        "{refused}"
    );
    stack
        .edit(
            "level.create",
            json!({ "level": "L-2", "name": "Second", "elevation": "6 m" }),
        )
        .await
        .unwrap();
    let refused = stack
        .edit(
            "floor.boundary",
            json!({ "element": "L-2", "boundary": rectangle(1000, 1000) }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "Second is not a floor.");
    let refused = stack
        .edit(
            "floor.boundary",
            json!({ "element": "F-none", "boundary": rectangle(1000, 1000) }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "That element no longer exists.");
    let refused = stack
        .edit_as(
            "ben",
            "floor.boundary",
            json!({ "element": "F-1", "boundary": rectangle(1000, 1000) }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "anna is working on this.");
    stack.later();
    stack
        .edit_as(
            "ben",
            "floor.boundary",
            json!({ "element": "F-1", "boundary": rectangle(1000, 1000) }),
        )
        .await
        .unwrap();
}

/// Runs a command, undoes it and redoes it: the undo puts every piece back as it was before the
/// command, and the redo as it was after.
async fn undone_and_redone(stack: &Stack, cmd: &str, args: Value) {
    let before = stack.rows("piece").await;
    stack
        .edit(cmd, args)
        .await
        .unwrap_or_else(|why| panic!("{cmd}: {why}"));
    let after = stack.rows("piece").await;
    assert_ne!(before, after, "{cmd} changed something");
    assert_eq!(
        stack.state("anna").await.undo.map(|step| step.cmd),
        Some(cmd.to_owned())
    );
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, before, "{cmd} undone");
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, after, "{cmd} redone");
}

#[tokio::test]
async fn every_floor_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    // The three default types in one entry; one undo takes back all three.
    undone_and_redone(&stack, "floor.defaults", json!({})).await;
    for (cmd, args) in [
        (
            "floor.type.create",
            json!({ "type": "F-own", "name": "Own", "layers": layers(TIMBER) }),
        ),
        ("floor.create", create("F-1", CONCRETE, &l_with_hole())),
        (
            "floor.boundary",
            json!({ "element": "F-1", "boundary": rectangle(4000, 3000) }),
        ),
        (
            "floor.type.layers",
            json!({ "type": CONCRETE, "layers": layers(SCREED) }),
        ),
        (
            "floor.type.rename",
            json!({ "type": CONCRETE, "name": "Renamed" }),
        ),
        (
            "floor.type.duplicate",
            json!({ "type": "F-own", "new": "F-copy", "name": "Copy" }),
        ),
        ("floor.type.delete", json!({ "type": "F-copy" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
    // Undone back past drawing it, the floor is gone; past the defaults, so are they.
    for _ in 0..6 {
        stack.edit("edit.undo", json!({})).await.unwrap();
    }
    assert!(stack.record("F-1").await.is_none());
    assert!(stack.record(CONCRETE).await.is_some());
    stack.edit("edit.undo", json!({})).await.unwrap();
    stack.edit("edit.undo", json!({})).await.unwrap();
    for default in [CONCRETE, SCREED, TIMBER] {
        assert!(stack.record(default).await.is_none(), "{default}");
    }
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    for (cmd, args) in [
        ("floor.defaults", json!({})),
        ("floor.create", create("F-1", SCREED, &l_with_hole())),
        (
            "floor.create",
            json!({ "element": "F-2", "type": CONCRETE, "level": "L-g",
                    "boundary": rectangle(0, 3000) }),
        ),
        (
            "floor.create",
            json!({ "element": "F-3", "type": CONCRETE, "level": "L-g",
                    "boundary": rectangle(6000, 3000) }),
        ),
        (
            "floor.type.layers",
            json!({ "type": SCREED, "layers": layers(TIMBER) }),
        ),
        ("edit.undo", json!({})),
        (
            "floor.boundary",
            json!({ "element": "F-3", "boundary": rectangle(5000, 3000) }),
        ),
        ("floor.defaults", json!({})),
        (
            "level.elevation",
            json!({ "level": "L-1", "elevation": "2.95 m" }),
        ),
    ] {
        let _ = stack.edit(cmd, args).await;
    }
    stack.regen.catch_up(&i("p")).await.unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let head: u64 = read_row(&mut snapshot, &Scope::Project(i("p")).head())
        .await
        .unwrap()
        .unwrap();
    let entries = read_log(&mut snapshot, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert!(entries.iter().any(|entry| entry.failed.is_some()));

    let again = Stack::bare().await;
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
    again.regen.catch_up(&i("p")).await.unwrap();
    for table in ["record", "piece"] {
        assert_eq!(again.rows(table).await, stack.rows(table).await, "{table}");
    }
    assert_eq!(again.derived().await, stack.derived().await);
    let mut replayed = again.store.snapshot().await.unwrap();
    let after = read_log(&mut replayed, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert_eq!(after, entries);
    // Built from empty, the derived rows are the same bytes.
    let incremental = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, incremental);
}
