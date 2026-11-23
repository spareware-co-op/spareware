//! Wall types and layered walls through the edit log, on the test store: a wall type made with
//! the wall parameters defined and bound once, in the same entry; a wall drawn between two points
//! of a chosen type, tied to levels, as the builders take it; a type edit building its walls
//! again, a door hosted on a drawn wall and following it; every refusal in words; undo and redo
//! of every wall command; and the log replayed from empty to the same bytes.

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
use construct_categories::WALLS;
use construct_model::element::Element;
use construct_model::geometry::{Built, Edition, Geometry, GeometryError, Hosted, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::parameters::{self as records, Context};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_types::ElementType;
use construct_walls::{Assembly, FLIPPED, LOCATION_LINE, assembly_of, default_walls};
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
const PARTITION: &str = "wall-partition-stud-90";
const CONCRETE: &str = "wall-concrete-200";

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

/// The stand-in, on its own edition, except that a host built with what is hosted on it lists
/// each hosted element's key and where it sits in the host's frame.
struct Cutter;

impl Geometry for Cutter {
    fn edition(&self) -> Edition {
        Boxes.edition()
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        Boxes.build(recipe)
    }

    fn build_hosting(&self, host: &Recipe, hosted: &[Hosted<'_>]) -> Result<Built, GeometryError> {
        let mut built = Boxes.build(host)?;
        let cuts: Vec<Value> = hosted
            .iter()
            .map(|h| json!({ "key": h.key, "at": [h.at.x, h.at.y, h.at.z] }))
            .collect();
        built.body = serde_json::to_vec(&json!({ "cut": cuts })).unwrap();
        Ok(built)
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
        let geometry: Arc<dyn Geometry> = Arc::new(Cutter);
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
    async fn wall_type(&self, id: &str) -> ElementType {
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

    /// Where each door sat in a wall it was built with.
    async fn cuts(&self, id: &str) -> Vec<(String, [i64; 3])> {
        let BuiltResult::Shape { body, .. } = self.built(id).await.result else {
            panic!("{id} did not build");
        };
        let place = Place::Body {
            project: i("p"),
            id: body.clone(),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        let stored = read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap();
        let bytes = self.regen.read_bytes(&stored, &body).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        body["cut"]
            .as_array()
            .map(|cuts| {
                cuts.iter()
                    .map(|c| {
                        let at = |n: usize| c["at"][n].as_i64().unwrap();
                        (c["key"].as_str().unwrap().to_owned(), [at(0), at(1), at(2)])
                    })
                    .collect()
            })
            .unwrap_or_default()
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

/// A default wall type's layers and core, as `wall.type.create` and `wall.type.layers` take them.
fn layers_of(default: &str) -> Value {
    let walls = default_walls().unwrap();
    let wall = walls.iter().find(|wall| wall.id == default).unwrap();
    sent(&wall.assembly)
}

/// Layers and a core as a person sends them: each thickness as typed text.
fn sent(assembly: &Assembly) -> Value {
    let layers: Vec<Value> = assembly
        .layers()
        .iter()
        .map(|layer| {
            json!({ "function": layer.function, "thickness": layer.thickness.to_string(),
                    "material": layer.material.as_str(), "wraps": layer.wraps })
        })
        .collect();
    json!({
        "core": {
            "exterior": assembly.boundary(construct_walls::Side::Exterior),
            "interior": assembly.boundary(construct_walls::Side::Interior),
        },
        "layers": layers,
    })
}

/// `wall.type.create`'s arguments for the default wall type `default`, under its own ID.
fn wall_type(default: &str) -> Value {
    let walls = default_walls().unwrap();
    let wall = walls.iter().find(|wall| wall.id == default).unwrap();
    json!({ "type": default, "name": wall.name, "assembly": layers_of(default) })
}

/// Ground at 0 and First at 3 m.
async fn two_levels(stack: &Stack) {
    for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
        let args = json!({ "level": id, "name": name, "elevation": elevation });
        stack.edit("level.create", args).await.unwrap();
    }
}

/// `wall.draw`'s arguments: wall `id` of type `ty` from `start` to `end`, on Ground, its top on
/// First.
fn draw(id: &str, ty: &str, start: [&str; 2], end: [&str; 2]) -> Value {
    json!({ "element": id, "type": ty, "start": start, "end": end, "level": "L-g", "top": "L-1" })
}

/// Two levels, the partition, and `W-1` of it along x from 0 to 4 m, Ground to First.
async fn a_partition(stack: &Stack) {
    two_levels(stack).await;
    stack
        .edit("wall.type.create", wall_type(PARTITION))
        .await
        .unwrap();
    stack
        .edit(
            "wall.draw",
            draw("W-1", PARTITION, ["0", "0"], ["4 m", "0"]),
        )
        .await
        .unwrap();
}

/// The first wall type defines the two wall parameters in the document's table and binds them to
/// walls, in the same entry, and a walls type already there lists them from then on; the type
/// holds its layers in its wall block and lists both. A second wall type leaves the table as it
/// is. A table that holds either ID as another parameter, layers construct-walls refuses, a name
/// taken and an ID taken are each refused in words.
#[tokio::test]
async fn a_wall_type_is_made_with_the_wall_parameters_once() {
    let stack = Stack::new().await;
    stack
        .edit(
            "type.create",
            json!({ "type": "T-plain", "name": "Plain", "category": 100 }),
        )
        .await
        .unwrap();
    assert!(stack.record("parameters").await.is_none());
    stack
        .edit("wall.type.create", wall_type(PARTITION))
        .await
        .unwrap();
    let context = stack.context().await;
    for parameter in [LOCATION_LINE, FLIPPED] {
        let (_, categories) = context.table.binding(&i(parameter)).unwrap();
        assert_eq!(categories, [WALLS], "{parameter}");
    }
    let partition = stack.wall_type(PARTITION).await;
    assert_eq!(
        assembly_of(&partition).unwrap(),
        default_walls().unwrap()[0].assembly
    );
    assert!(partition.has(&i(LOCATION_LINE)) && partition.has(&i(FLIPPED)));
    let plain = stack.wall_type("T-plain").await;
    assert!(plain.has(&i(LOCATION_LINE)) && plain.has(&i(FLIPPED)));
    assert!(plain.system().is_none(), "a plain type stays plain");

    let table = stack.record("parameters").await;
    stack
        .edit("wall.type.create", wall_type(CONCRETE))
        .await
        .unwrap();
    assert_eq!(stack.record("parameters").await, table);
    assert_eq!(
        assembly_of(&stack.wall_type(CONCRETE).await).unwrap(),
        default_walls().unwrap()[1].assembly
    );

    let mut taken = wall_type(CONCRETE);
    taken["type"] = json!("T-2");
    let refused = stack.edit("wall.type.create", taken).await.unwrap_err();
    assert_eq!(
        refused,
        "category 100 already has a type named \"Concrete 200\""
    );
    let refused = stack
        .edit("wall.type.create", wall_type(CONCRETE))
        .await
        .unwrap_err();
    assert_eq!(refused, "That element already exists.");
    let mut wraps =
        json!({ "type": "T-3", "name": "Wrapped core", "assembly": layers_of(CONCRETE) });
    wraps["assembly"]["layers"][0]["wraps"] = json!(true);
    let refused = stack.edit("wall.type.create", wraps).await.unwrap_err();
    assert_eq!(
        refused,
        "layer 0 is in the core and so cannot wrap; only a shell layer wraps"
    );
    let mut odd = json!({ "type": "T-4", "name": "Odd", "assembly": layers_of(CONCRETE) });
    odd["assembly"]["layers"][0]["thickness"] = json!("two hundred");
    let refused = stack.edit("wall.type.create", odd).await.unwrap_err();
    assert!(refused.starts_with("layer 0's thickness: "), "{refused}");

    // A document that holds a wall parameter's ID as another parameter has no wall types.
    let other = Stack::new().await;
    let flipped = json!({ "definition": { "id": FLIPPED, "name": "Flipped", "kind": "text",
                                          "holder": "instance", "choices": [] } });
    other.edit("parameter.define", flipped).await.unwrap();
    let refused = other
        .edit("wall.type.create", wall_type(PARTITION))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "wall.flipped is another parameter in this model, so a wall type cannot read it."
    );
    assert!(other.record(PARTITION).await.is_none());
}

/// A wall is drawn between two points of a chosen type: its record holds its line as stored
/// lengths, its layers read from its type, its ties and its type, and it is built from them; one
/// on its exterior finish face, flipped, lies on the other side of its line; a level moved moves
/// its top. Every refusal is in words and writes nothing.
#[tokio::test]
async fn a_wall_is_drawn_between_two_points_of_a_chosen_type() {
    let stack = Stack::new().await;
    a_partition(&stack).await;
    let wall = stack.record("W-1").await.unwrap();
    assert_eq!(wall.kind, "wall");
    assert_eq!(wall.level, 0);
    assert_eq!(
        [wall.placement.x, wall.placement.y, wall.placement.z],
        [0, 0, 0]
    );
    assert_eq!(wall.recipe.op, "wall.layered");
    assert_eq!(
        serde_json::to_value(&wall.recipe.args).unwrap(),
        json!({
            "start": ["length:q0:0", "length:q0:0"],
            "end": ["length:q0:25600000", "length:q0:0"],
            "height": { "tie": "top" },
            "layers": { "wall": "type" },
        })
    );
    assert_eq!(wall.props["@type"], PARTITION);
    assert_eq!(wall.props["@category"], records::write_category(WALLS));
    assert_eq!(wall.props["@level"], "L-g");
    assert_eq!(wall.props["@top.level"], "L-1");
    assert!(!wall.props.contains_key(LOCATION_LINE));
    // 115 mm thick about its line, each face to the nearest millimetre; Ground to First.
    assert_eq!(
        stack.bounds("W-1").await,
        Ok(([0, -58, 0], [4000, 58, 3000]))
    );

    let args = json!({ "element": "W-2", "name": "Flipped", "type": PARTITION,
                       "start": ["0", "5 m"], "end": ["4000", "5000"], "height": "2.7 m",
                       "line": "exterior-finish-face", "flipped": true });
    stack.edit("wall.draw", args).await.unwrap();
    let flipped = stack.record("W-2").await.unwrap();
    assert_eq!(flipped.name, "Flipped");
    assert_eq!(flipped.recipe.args["height"], "length:q0:17280000");
    assert_eq!(flipped.props[LOCATION_LINE], "choice:exterior-finish-face");
    assert_eq!(flipped.props[FLIPPED], "yesno:yes");
    assert!(!flipped.props.contains_key("@level"));
    assert_eq!(
        stack.bounds("W-2").await,
        Ok(([0, 4885, 0], [4000, 5000, 2700]))
    );

    // First moved up: W-1 is built to it again; its record is not written.
    stack.later();
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "3.2 m" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("W-1").await.unwrap(), wall);
    assert_eq!(
        stack.bounds("W-1").await,
        Ok(([0, -58, 0], [4000, 58, 3200]))
    );

    stack
        .edit(
            "type.create",
            json!({ "type": "T-door", "name": "Door", "category": 300 }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "type.create",
            json!({ "type": "T-plain", "name": "Plain", "category": 100 }),
        )
        .await
        .unwrap();
    let base = draw("W-9", PARTITION, ["0", "0"], ["1 m", "0"]);
    let with = |key: &str, value: Value| {
        let mut args = base.clone();
        if value.is_null() {
            args.as_object_mut().unwrap().remove(key);
        } else {
            args[key] = value;
        }
        args
    };
    for (args, words) in [
        (
            with("end", json!(["0", "0"])),
            "W-9's start and end are the same point: a wall has a length.",
        ),
        (with("type", json!("T-none")), "no type has id T-none"),
        (
            with("type", json!("T-door")),
            "an instance of category 100 cannot take a type of category 300",
        ),
        (
            with("type", json!("T-plain")),
            "not a stored wall type: a wall type carries a \"wall\" system block",
        ),
        (
            with("top", json!("L-g")),
            "W-9's top would be 0 mm from its base: a top is above its base.",
        ),
        (
            with("height", json!("2.7 m")),
            "A wall's top is tied to a level or given as a height, not both.",
        ),
        (
            with("top", Value::Null),
            "A wall needs its top tied to a level or a height.",
        ),
        (
            with("level", json!("L-none")),
            "There is no level L-none in this model.",
        ),
        (
            with("line", json!("middle")),
            "\"middle\" is not a location line.",
        ),
        (
            with("element", json!("W-1")),
            "That element already exists.",
        ),
    ] {
        let refused = stack.edit("wall.draw", args).await.unwrap_err();
        assert_eq!(refused, words);
    }
    let no_top = json!({ "element": "W-9", "type": PARTITION, "start": ["0", "0"],
                         "end": ["1 m", "0"], "height": "0" });
    let refused = stack.edit("wall.draw", no_top).await.unwrap_err();
    assert_eq!(refused, "W-9 would be 0 mm high: a top is above its base.");
    assert!(stack.record("W-9").await.is_none());
}

/// A change to a wall type's layers builds every wall of it again with the new layers, its record
/// unwritten, and a door hosted on it sits flush with its new side; a wall type renamed, copied
/// with its layers, and deleted only when no wall is of it; the wall type commands refuse a type
/// that is not a wall type.
#[tokio::test]
async fn a_change_to_a_wall_type_builds_its_walls_again() {
    let stack = Stack::new().await;
    a_partition(&stack).await;
    let wall = stack.record("W-1").await.unwrap();
    let door = json!({ "element": "D-1", "host": "W-1", "face": "box.exterior", "along": "1200 mm",
                       "recipe": { "owner": "construct", "schema": 1, "op": "door.single",
                                   "op_version": 1,
                                   "args": { "width": 900, "thickness": 100, "height": 2100 } } });
    stack.edit("hosted.place", door).await.unwrap();
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [1200, -58, 0])]
    );
    let placed = stack.record("D-1").await.unwrap();

    stack.later();
    let concrete = json!({ "type": PARTITION, "assembly": layers_of(CONCRETE) });
    stack.edit("wall.type.layers", concrete).await.unwrap();
    assert_eq!(stack.record("W-1").await.unwrap(), wall);
    assert_eq!(
        stack.bounds("W-1").await,
        Ok(([0, -100, 0], [4000, 100, 3000]))
    );
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [1200, -100, 0])]
    );
    assert_eq!(
        stack.record("D-1").await.unwrap(),
        placed,
        "no door was written"
    );

    stack
        .edit(
            "wall.type.rename",
            json!({ "type": PARTITION, "name": "Was a partition" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.wall_type(PARTITION).await.name(), "Was a partition");
    stack
        .edit(
            "wall.type.duplicate",
            json!({ "type": PARTITION, "new": "T-copy", "name": "Copy" }),
        )
        .await
        .unwrap();
    assert_eq!(
        assembly_of(&stack.wall_type("T-copy").await).unwrap(),
        default_walls().unwrap()[1].assembly
    );
    let refused = stack
        .edit("wall.type.delete", json!({ "type": PARTITION }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "type \"Was a partition\" cannot be deleted: 1 instances use it"
    );
    stack
        .edit("wall.type.delete", json!({ "type": "T-copy" }))
        .await
        .unwrap();
    assert!(stack.record("T-copy").await.is_none());

    stack
        .edit(
            "type.create",
            json!({ "type": "T-door", "name": "Door", "category": 300 }),
        )
        .await
        .unwrap();
    for (cmd, args) in [
        (
            "wall.type.layers",
            json!({ "type": "T-door", "assembly": layers_of(CONCRETE) }),
        ),
        ("wall.type.rename", json!({ "type": "T-door", "name": "D" })),
        (
            "wall.type.duplicate",
            json!({ "type": "T-door", "new": "T-d2", "name": "D2" }),
        ),
        ("wall.type.delete", json!({ "type": "T-door" })),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(
            refused, "a type of category 300 is not a wall type",
            "{cmd}"
        );
    }
    let refused = stack
        .edit(
            "wall.type.layers",
            json!({ "type": "T-none", "assembly": layers_of(CONCRETE) }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "no type has id T-none");

    // Ben may not change the type while Anna is working on it.
    let refused = stack
        .edit_as(
            "ben",
            "wall.type.layers",
            json!({ "type": PARTITION, "assembly": layers_of(PARTITION) }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "anna is working on this.");
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
async fn every_wall_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    // The first wall type writes the table and the type in one entry; one undo takes back both.
    undone_and_redone(&stack, "wall.type.create", wall_type(PARTITION)).await;
    for (cmd, args) in [
        ("wall.type.create", wall_type(CONCRETE)),
        (
            "wall.draw",
            draw("W-1", PARTITION, ["0", "0"], ["4 m", "0"]),
        ),
        (
            "wall.type.layers",
            json!({ "type": PARTITION, "assembly": layers_of(CONCRETE) }),
        ),
        (
            "wall.type.rename",
            json!({ "type": PARTITION, "name": "Renamed" }),
        ),
        (
            "wall.type.duplicate",
            json!({ "type": CONCRETE, "new": "T-copy", "name": "Copy" }),
        ),
        ("wall.type.delete", json!({ "type": "T-copy" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
    // Undone back past drawing it, the wall is gone; past the first wall type, so is the table.
    for _ in 0..5 {
        stack.edit("edit.undo", json!({})).await.unwrap();
    }
    assert!(stack.record("W-1").await.is_none());
    assert!(stack.record("parameters").await.is_some());
    stack.edit("edit.undo", json!({})).await.unwrap();
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.record("parameters").await.is_none());
    assert!(stack.record(PARTITION).await.is_none());
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    a_partition(&stack).await;
    for (cmd, args) in [
        ("wall.type.create", wall_type(CONCRETE)),
        (
            "wall.draw",
            draw("W-2", CONCRETE, ["4 m", "0"], ["4 m", "3.5 m"]),
        ),
        (
            "wall.draw",
            json!({ "element": "W-3", "type": PARTITION, "start": ["0", "0"],
                    "end": ["0", "0"], "height": "2.4 m" }),
        ),
        (
            "wall.type.layers",
            json!({ "type": PARTITION, "assembly": layers_of(CONCRETE) }),
        ),
        ("edit.undo", json!({})),
        (
            "wall.type.duplicate",
            json!({ "type": CONCRETE, "new": "T-copy", "name": "Copy" }),
        ),
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
