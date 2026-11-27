//! Linear dimensions through the edit log, on the test store: the dimension commands and their
//! refusals, a value worked out from the model each time it is asked (a grid that moves, a door
//! that moves along its wall), a deleted reference reported by name, the generic element commands
//! refused for a dimension, a view that holds dimensions kept, dimensions with no shape, undo and
//! redo of every command, and the log replayed from empty.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    clippy::assert_is_empty,
    clippy::unnecessary_wraps,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::dimensions::{self, DimensionListing, DimensionView};
use construct_api::cloud::undo::{self, CANNOT, State};
use construct_api::cloud::{Access, ProjectCommands, walls};
use construct_dimensions::DimensionError;
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_params::{Length, Value as Param};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
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

    async fn record(&self, id: &str) -> Element {
        let mut snapshot = self.store.snapshot().await.unwrap();
        construct_regen::parameters::read_element(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .unwrap()
            .1
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

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn level(id: &str, name: &str, elevation: &str) -> Value {
    json!({ "level": id, "name": name, "elevation": elevation })
}

fn plan(id: &str, level: &str) -> Value {
    json!({ "view": id, "kind": "floor_plan", "name": "Ground floor", "level": level })
}

fn drafting(id: &str) -> Value {
    json!({ "view": id, "kind": "drafting", "name": "Detail" })
}

/// A vertical grid line at `x` millimetres.
fn grid(id: &str, name: &str, x: i32) -> Value {
    json!({ "grid": id, "name": name, "x0": x.to_string(), "y0": "-1000",
            "x1": x.to_string(), "y1": "7000" })
}

fn recipe(op: &str, args: Value) -> Recipe {
    Recipe::construct(op, serde_json::from_value(args).unwrap())
}

/// A box wall `id`, 4 m long and 200 mm thick, its lowest corner at (`x`, `y`).
fn box_wall(id: &str, x: i64, y: i64, axis: &str) -> Value {
    let args = json!({ "length": 4000, "thickness": 200, "height": 2700, "axis": axis });
    json!({ "element": id, "kind": "wall", "level": 1,
            "placement": { "x": x, "y": y, "z": 0 }, "recipe": recipe("wall.straight", args) })
}

fn door_recipe(width: i64) -> Recipe {
    recipe(
        "door.single",
        json!({ "width": width, "thickness": 100, "height": 2100 }),
    )
}

/// A layered wall `id` of the default partition (115 mm), 4 m along x from (1000, 2000), drawn as
/// a person draws one: its exterior face is 57.5 mm to the right of its line, at y = 1942.5 mm.
async fn a_layered_wall(stack: &Stack, id: &str) {
    let partition = walls::default_type_args().unwrap().remove(0);
    let ty = partition["type"].clone();
    let _ = stack.edit("wall.type.create", partition).await;
    let draw = json!({ "element": id, "type": ty, "start": ["1000 mm", "2000 mm"],
                       "end": ["5000 mm", "2000 mm"], "level": 1, "height": "2700 mm" });
    stack.edit("wall.draw", draw).await.unwrap();
}

fn reference(element: &str, feature: &str) -> Value {
    json!({ "element": element, "feature": feature })
}

/// A horizontal dimension `id` in `V-g` at y = `y`, between `references`.
fn horizontal(id: &str, y: &str, references: &[Value]) -> Value {
    json!({ "dimension": id, "view": "V-g", "layout": { "kind": "horizontal", "y": y },
            "references": references })
}

fn vertical(id: &str, x: &str, references: &[Value]) -> Value {
    json!({ "dimension": id, "view": "V-g", "layout": { "kind": "vertical", "x": x },
            "references": references })
}

/// Level `L-g` and the plan `V-g` on it.
async fn with_plan() -> Stack {
    let stack = Stack::new().await;
    stack
        .edit("level.create", level("L-g", "Ground", "0"))
        .await
        .unwrap();
    stack.edit("view.create", plan("V-g", "L-g")).await.unwrap();
    stack
}

async fn listing(stack: &Stack, view: &str) -> DimensionListing {
    let mut snapshot = stack.store.snapshot().await.unwrap();
    dimensions::listing(&mut snapshot, &i("p"), &i(DOCUMENT), &i(view))
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

/// The one dimension's listing.
async fn shown(stack: &Stack, id: &str) -> DimensionView {
    let listing = listing(stack, "V-g").await;
    listing
        .dimensions
        .into_iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("no dimension {id} in the view"))
}

fn values(view: &DimensionView) -> Vec<Option<String>> {
    view.segments.iter().map(|s| s.value.clone()).collect()
}

fn stored(length: Length) -> Option<String> {
    Some(Param::Length(length).stored())
}

#[tokio::test]
async fn a_dimension_follows_its_references_when_they_move() {
    let stack = with_plan().await;
    stack
        .edit("grid.create", grid("G-1", "1", 0))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-2", "2", 6000))
        .await
        .unwrap();
    stack
        .edit(
            "dimension.create",
            horizontal(
                "D-1",
                "3000",
                &[reference("G-1", "grid.line"), reference("G-2", "grid.line")],
            ),
        )
        .await
        .unwrap();
    let before = stack.record("D-1").await;
    let first = shown(&stack, "D-1").await;
    assert_eq!(values(&first), [stored(mm(6000))]);
    assert_eq!(first.segments[0].text.as_deref(), Some("6000 mm"));
    assert!(first.segments[0].exact);
    assert_eq!(first.overall_text.as_deref(), Some("6000 mm"));
    assert!(first.warnings.is_empty());
    assert_eq!(first.witnesses.len(), 2);
    // A grid moves: the same dimension reads the new distance, and its record is untouched.
    stack
        .edit(
            "grid.move",
            json!({ "grid": "G-2", "dx": "500", "dy": "0" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("D-1").await, before);
    assert_eq!(values(&shown(&stack, "D-1").await), [stored(mm(6500))]);
    // A string: one more reference, and the overall length.
    stack
        .edit("grid.create", grid("G-3", "3", 8000))
        .await
        .unwrap();
    stack
        .edit(
            "dimension.reference.add",
            json!({ "dimension": "D-1", "reference": reference("G-3", "grid.line") }),
        )
        .await
        .unwrap();
    let string = shown(&stack, "D-1").await;
    assert_eq!(values(&string), [stored(mm(6500)), stored(mm(1500))]);
    assert_eq!(string.overall.as_deref(), stored(mm(8000)).as_deref());
    // The line moves, and a segment is held equal as data: reported, not enforced.
    stack
        .edit(
            "dimension.move",
            json!({ "dimension": "D-1", "line": "4 m" }),
        )
        .await
        .unwrap();
    let moved = stack.record("D-1").await;
    assert_eq!(moved.props["@dimension.y"], "length:q0:25600000");
    stack
        .edit(
            "dimension.equal",
            json!({ "dimension": "D-1", "groups": [[0, 1]] }),
        )
        .await
        .unwrap();
    let held = shown(&stack, "D-1").await;
    assert_eq!(held.equalities[0].segments, [0, 1]);
    assert_eq!(held.equalities[0].equal, Some(false));
    // The listing is read-only data: asking again gives the same answer.
    assert_eq!(shown(&stack, "D-1").await, held);
}

#[tokio::test]
async fn a_layered_walls_faces_and_its_hosted_door_are_dimensioned() {
    let stack = with_plan().await;
    a_layered_wall(&stack, "W-1").await;
    stack
        .edit("grid.create", grid("G-A", "A", 1000))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-B", "B", 5000))
        .await
        .unwrap();
    // The partition is 115 mm: across it, a vertical dimension reads the thickness exactly.
    stack
        .edit(
            "dimension.create",
            vertical(
                "D-T",
                "500",
                &[
                    reference("W-1", "wall.face.exterior"),
                    reference("W-1", "wall.line"),
                    reference("W-1", "wall.face.interior"),
                ],
            ),
        )
        .await
        .unwrap();
    let thickness = shown(&stack, "D-T").await;
    assert_eq!(
        values(&thickness),
        [
            stored(Length::from_steps(368_000)),
            stored(Length::from_steps(368_000))
        ]
    );
    assert_eq!(
        thickness.overall.as_deref(),
        stored(Length::from_steps(736_000)).as_deref()
    );
    assert_eq!(thickness.overall_text.as_deref(), Some("115 mm"));
    // A door 900 wide, 1200 along from the wall's start (x 1000): edges 2200 and 3100, centre 2650.
    stack
        .edit(
            "hosted.place",
            json!({ "element": "D-1", "host": "W-1", "face": "box.exterior", "along": "1200 mm",
                    "recipe": door_recipe(900) }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "dimension.create",
            horizontal(
                "D-L",
                "1500",
                &[
                    reference("G-A", "grid.line"),
                    reference("D-1", "edge.start"),
                    reference("D-1", "centre"),
                    reference("D-1", "edge.end"),
                    reference("G-B", "grid.line"),
                ],
            ),
        )
        .await
        .unwrap();
    let along = shown(&stack, "D-L").await;
    assert_eq!(
        values(&along),
        [
            stored(mm(1200)),
            stored(mm(450)),
            stored(mm(450)),
            stored(mm(1900))
        ]
    );
    stack
        .edit(
            "dimension.equal",
            json!({ "dimension": "D-L", "groups": [[1, 2]] }),
        )
        .await
        .unwrap();
    assert_eq!(shown(&stack, "D-L").await.equalities[0].equal, Some(true));
    // The door moves along its wall: the dimension follows, its record untouched.
    let before = stack.record("D-L").await;
    stack
        .edit(
            "hosted.move",
            json!({ "element": "D-1", "along": "2000 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("D-L").await, before);
    let moved = shown(&stack, "D-L").await;
    assert_eq!(
        values(&moved),
        [
            stored(mm(2000)),
            stored(mm(450)),
            stored(mm(450)),
            stored(mm(1100))
        ]
    );
}

#[tokio::test]
async fn a_box_walls_faces_are_where_the_builder_puts_them() {
    let stack = with_plan().await;
    stack
        .edit("grid.create", grid("G-0", "0", 0))
        .await
        .unwrap();
    stack
        .edit("element.create", box_wall("W-X", 1000, 2000, "x"))
        .await
        .unwrap();
    stack
        .edit("element.create", box_wall("W-Z", 0, 0, "x"))
        .await
        .unwrap();
    stack
        .edit("element.create", box_wall("W-Y", 3000, 500, "y"))
        .await
        .unwrap();
    // Along y its exterior is at x = placement + thickness, its interior at x = placement.
    stack
        .edit(
            "dimension.create",
            horizontal(
                "D-1",
                "100",
                &[
                    reference("G-0", "grid.line"),
                    reference("W-Y", "wall.face.interior"),
                    reference("W-Y", "wall.line"),
                    reference("W-Y", "wall.face.exterior"),
                ],
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        values(&shown(&stack, "D-1").await),
        [stored(mm(3000)), stored(mm(100)), stored(mm(100))]
    );
    // Along x its exterior is at y = placement, its interior 200 above it.
    stack
        .edit(
            "dimension.create",
            vertical(
                "D-2",
                "0",
                &[
                    reference("W-Z", "wall.face.exterior"),
                    reference("W-X", "wall.face.exterior"),
                    reference("W-X", "wall.face.interior"),
                ],
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        values(&shown(&stack, "D-2").await),
        [stored(mm(2000)), stored(mm(200))]
    );
    // The wall moves with the element command: the dimension follows.
    stack
        .edit(
            "element.move",
            json!({ "element": "W-X", "placement": { "x": 1000, "y": 2600, "z": 0 } }),
        )
        .await
        .unwrap();
    assert_eq!(
        values(&shown(&stack, "D-2").await),
        [stored(mm(2600)), stored(mm(200))]
    );
    // W-Y's interior face runs along y, along a vertical dimension's line: it has no place
    // along it, and the dimension is refused.
    let refused = stack
        .edit(
            "dimension.create",
            vertical(
                "D-3",
                "0",
                &[
                    reference("W-X", "wall.face.exterior"),
                    reference("W-Y", "wall.face.interior"),
                ],
            ),
        )
        .await
        .unwrap_err();
    assert!(
        refused.contains("runs along the dimension's line"),
        "{refused}"
    );
}

#[tokio::test]
async fn a_deleted_reference_is_reported_by_name_and_comes_back_with_an_undo() {
    let stack = with_plan().await;
    stack
        .edit("grid.create", grid("G-1", "1", 0))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-2", "2", 6000))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-3", "3", 9000))
        .await
        .unwrap();
    stack
        .edit(
            "dimension.create",
            horizontal(
                "D-1",
                "3000",
                &[
                    reference("G-1", "grid.line"),
                    reference("G-2", "grid.line"),
                    reference("G-3", "grid.line"),
                ],
            ),
        )
        .await
        .unwrap();
    let before = stack.record("D-1").await;
    stack
        .edit("grid.delete", json!({ "grid": "G-2" }))
        .await
        .unwrap();
    // The dimension is not touched, and says which reference is gone; no wrong number.
    assert_eq!(stack.record("D-1").await, before);
    let gone = shown(&stack, "D-1").await;
    assert_eq!(values(&gone), [None, None]);
    assert_eq!(gone.overall, None);
    assert_eq!(
        gone.references.iter().map(|r| r.usable).collect::<Vec<_>>(),
        [true, false, true]
    );
    let [warning] = gone.warnings.as_slice() else {
        panic!("one warning");
    };
    assert_eq!(warning.kind, "dimension.reference_missing");
    assert_eq!(warning.severity, "warning");
    assert_eq!(warning.elements, ["D-1", "G-2"]);
    assert_eq!(warning.identity, "dimension.reference_missing|D-1|G-2");
    assert_eq!(
        warning.words,
        "Dimension D-1: reference 2 (the grid line G-2) cannot be found: the element is not in \
         this model any more."
    );
    // The grid comes back with an undo, and so does the number.
    stack.edit("edit.undo", json!({})).await.unwrap();
    let back = shown(&stack, "D-1").await;
    assert_eq!(values(&back), [stored(mm(6000)), stored(mm(3000))]);
    assert!(back.warnings.is_empty());
}

#[tokio::test]
async fn a_dimension_that_cannot_be_read_is_refused_by_name() {
    let stack = with_plan().await;
    stack
        .edit("grid.create", grid("G-1", "1", 0))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-2", "2", 6000))
        .await
        .unwrap();
    stack.edit("view.create", drafting("V-d")).await.unwrap();
    stack
        .edit("level.create", level("L-1", "First", "3 m"))
        .await
        .unwrap();
    stack
        .edit(
            "grid.create",
            json!({ "grid": "G-C", "name": "C", "cx": "0", "cy": "0", "radius": "1 m",
                    "start": "0", "end": "90" }),
        )
        .await
        .unwrap();
    a_layered_wall(&stack, "W-1").await;
    let g = |id: &str| reference(id, "grid.line");
    let good = [g("G-1"), g("G-2")];
    let create = |args: Value| stack.edit("dimension.create", args);
    for (args, why) in [
        (
            json!({ "dimension": "D-1", "view": "V-g", "references": good }),
            "This command needs a layout.",
        ),
        (
            json!({ "dimension": "D-1", "view": "V-g",
                    "layout": { "kind": "horizontal", "y": "1" } }),
            "This command needs references.",
        ),
        (
            json!({ "dimension": "D-1", "view": "V-g", "layout": { "kind": "slanted" },
                    "references": good }),
            "\"slanted\" is not a layout: horizontal, vertical or aligned.",
        ),
        (
            json!({ "dimension": "D-1", "view": "V-g",
                    "layout": { "kind": "horizontal", "x": "1" }, "references": good }),
            "A horizontal layout takes only y.",
        ),
        (
            json!({ "dimension": "D-1", "view": "V-g",
                    "layout": { "kind": "aligned", "through": { "x": "0", "y": "0" },
                                "direction": { "x": "0", "y": "0" } }, "references": good }),
            "An aligned dimension needs a direction with a length.",
        ),
        (
            horizontal("D-1", "1", &[g("G-1")]),
            "A dimension measures between at least two references; this has 1.",
        ),
        (
            horizontal("D-1", "1", &[g("G-1"), g("G-1")]),
            "References 1 and 2 are the same",
        ),
        (
            horizontal("D-1", "1", &[g("G-1"), reference("G-1", "grid.edge")]),
            "\"grid.edge\" is not something to dimension to",
        ),
        (
            horizontal("D-1", "1", &[g("G-1"), g("G-9")]),
            "Dimension D-1: reference 2 (the grid line G-9) cannot be found: the element is not \
             in this model any more.",
        ),
        (
            horizontal("D-1", "1", &[g("G-1"), reference("L-1", "level")]),
            "Dimension D-1: reference 2 (the level L-1) cannot be found: a level has no place in \
             a plan",
        ),
        (
            horizontal("D-1", "1", &[g("G-1"), g("G-C")]),
            "a curved grid has no straight line to measure to",
        ),
        (
            horizontal("D-1", "1", &[g("G-1"), reference("W-1", "grid.line")]),
            "it is not a grid line",
        ),
        (
            horizontal("D-1", "1", &[g("G-1"), reference("W-1", "centre")]),
            "it is not hosted on a wall at an offset",
        ),
        (
            horizontal("D-1", "1", &[g("G-1"), reference("G-2", "wall.line")]),
            "it is not a wall",
        ),
        (
            // A wall along x runs along a horizontal dimension's line.
            horizontal("D-1", "1", &[g("G-1"), reference("W-1", "wall.line")]),
            "runs along the dimension's line",
        ),
        (
            json!({ "dimension": "D-1", "view": "V-9",
                    "layout": { "kind": "horizontal", "y": "1" }, "references": good }),
            "There is no view V-9 in this model.",
        ),
        (
            json!({ "dimension": "D-1", "view": "V-d",
                    "layout": { "kind": "horizontal", "y": "1" }, "references": good }),
            "The view V-d is a drafting: dimensions are made in plans for now",
        ),
        (
            json!({ "dimension": "D-1", "view": "V-g",
                    "layout": { "kind": "horizontal", "y": "1" }, "references": good,
                    "extra": 1 }),
            "That edit isn't valid",
        ),
        (
            json!({ "dimension": "D-1", "view": "V-g",
                    "layout": { "kind": "horizontal", "y": "0.001 mm" }, "references": good }),
            "is not a whole number of grid steps",
        ),
    ] {
        let refused = create(args).await.unwrap_err();
        assert!(refused.contains(why), "{refused}  (wanted: {why})");
    }
    assert!(!stack.exists("D-1").await);
    create(horizontal("D-1", "1", &good)).await.unwrap();
    assert_eq!(
        create(horizontal("D-1", "1", &good)).await.unwrap_err(),
        "That element already exists."
    );
}

#[tokio::test]
async fn references_and_equalities_keep_their_rules() {
    let stack = with_plan().await;
    for (id, x) in [("G-1", 0), ("G-2", 3000), ("G-3", 5000), ("G-4", 9000)] {
        stack.edit("grid.create", grid(id, id, x)).await.unwrap();
    }
    let g = |id: &str| reference(id, "grid.line");
    stack
        .edit(
            "dimension.create",
            horizontal("D-1", "1", &[g("G-1"), g("G-2")]),
        )
        .await
        .unwrap();
    let add = |reference: Value, at: Option<usize>| {
        let mut args = json!({ "dimension": "D-1", "reference": reference });
        if let Some(at) = at {
            args["at"] = json!(at);
        }
        stack.edit("dimension.reference.add", args)
    };
    add(g("G-4"), Some(1)).await.unwrap();
    assert_eq!(
        values(&shown(&stack, "D-1").await),
        [stored(mm(9000)), stored(mm(6000))]
    );
    assert_eq!(
        add(g("G-4"), Some(2)).await.unwrap_err(),
        "References 2 and 3 are the same: the segment between them would have no length."
    );
    assert_eq!(
        add(g("G-3"), Some(9)).await.unwrap_err(),
        "There is no reference 10 in this dimension."
    );
    assert!(
        add(g("G-9"), None)
            .await
            .unwrap_err()
            .contains("reference 4 (the grid line G-9) cannot be found")
    );
    assert_eq!(
        stack
            .edit("dimension.reference.add", json!({ "dimension": "D-1" }))
            .await
            .unwrap_err(),
        "This command needs a reference."
    );
    let remove = |at: usize| {
        stack.edit(
            "dimension.reference.remove",
            json!({ "dimension": "D-1", "at": at }),
        )
    };
    remove(1).await.unwrap();
    assert_eq!(values(&shown(&stack, "D-1").await), [stored(mm(3000))]);
    assert_eq!(
        remove(0).await.unwrap_err(),
        "A dimension measures between at least two references; this has 1."
    );
    assert_eq!(
        remove(7).await.unwrap_err(),
        "There is no reference 8 in this dimension."
    );
    // Equalities: named segments only, and the references stand while they hold.
    add(g("G-3"), None).await.unwrap();
    let equal = |groups: Value| {
        stack.edit(
            "dimension.equal",
            json!({ "dimension": "D-1", "groups": groups }),
        )
    };
    assert!(
        equal(json!([[0, 5]]))
            .await
            .unwrap_err()
            .starts_with("Cannot make segments equal: segment 6 is not there")
    );
    assert!(
        equal(json!([[0]]))
            .await
            .unwrap_err()
            .contains("at least two segments")
    );
    equal(json!([[0, 1]])).await.unwrap();
    assert_eq!(
        add(g("G-4"), None).await.unwrap_err(),
        DimensionError::HasEqualities.to_string()
    );
    assert_eq!(
        remove(0).await.unwrap_err(),
        DimensionError::HasEqualities.to_string()
    );
    equal(json!([])).await.unwrap();
    remove(0).await.unwrap();
    // A command that changes nothing writes nothing and makes no step.
    let steps = stack.rows("piece").await;
    equal(json!([])).await.unwrap();
    stack
        .edit("dimension.move", json!({ "dimension": "D-1", "line": "1" }))
        .await
        .unwrap();
    assert_eq!(stack.rows("piece").await, steps);
    // The layout's own move.
    for (args, why) in [
        (json!({ "dimension": "D-1" }), "This command needs a line"),
        (
            json!({ "dimension": "D-1", "through": { "x": "0", "y": "0" } }),
            "A horizontal or vertical dimension's line is moved by its position across.",
        ),
        (
            json!({ "dimension": "D-1", "line": "1", "through": { "x": "0", "y": "0" } }),
            "This command needs a line",
        ),
        (
            json!({ "dimension": "D-9", "line": "1" }),
            "There is no dimension D-9",
        ),
    ] {
        let refused = stack.edit("dimension.move", args).await.unwrap_err();
        assert!(refused.contains(why), "{refused}");
    }
}

#[tokio::test]
async fn an_aligned_dimension_reads_along_its_direction() {
    let stack = with_plan().await;
    stack
        .edit("grid.create", grid("G-1", "1", 0))
        .await
        .unwrap();
    // A line at 45 degrees: from (0, 0) to (4000, 4000).
    stack
        .edit(
            "grid.create",
            json!({ "grid": "G-D", "name": "D", "x0": "0", "y0": "0", "x1": "4000", "y1": "4000" }),
        )
        .await
        .unwrap();
    // Aligned along the 45 degree direction, through (0, 3000): the vertical grid at x = 0 meets
    // it at (0, 3000); the diagonal meets it at (3000, 3000): 3000 mm apart along x, read along
    // the 45 degree line they are 3000 / sqrt(2) = 2121.3203... mm apart.
    let create = json!({
        "dimension": "D-1", "view": "V-g",
        "layout": { "kind": "aligned", "through": { "x": "0", "y": "3000" },
                    "direction": { "x": "1", "y": "1" } },
        "references": [reference("G-1", "grid.line"), reference("G-D", "grid.line")]
    });
    // The diagonal runs along the aligned line: refused, naming it.
    assert!(
        stack
            .edit("dimension.create", create)
            .await
            .unwrap_err()
            .contains("runs along the dimension's line")
    );
    let create = json!({
        "dimension": "D-1", "view": "V-g",
        "layout": { "kind": "aligned", "through": { "x": "0", "y": "3000" },
                    "direction": { "x": "1", "y": "0" } },
        "references": [reference("G-1", "grid.line"), reference("G-D", "grid.line")]
    });
    stack.edit("dimension.create", create).await.unwrap();
    assert_eq!(values(&shown(&stack, "D-1").await), [stored(mm(3000))]);
    stack
        .edit(
            "dimension.move",
            json!({ "dimension": "D-1", "through": { "x": "0", "y": "1000" } }),
        )
        .await
        .unwrap();
    assert_eq!(values(&shown(&stack, "D-1").await), [stored(mm(1000))]);
    assert_eq!(
        stack
            .edit("dimension.move", json!({ "dimension": "D-1", "line": "1" }))
            .await
            .unwrap_err(),
        "An aligned dimension's line is moved by the point it goes through."
    );
}

#[tokio::test]
async fn the_generic_commands_do_not_go_behind_the_dimension_commands() {
    let stack = with_plan().await;
    stack
        .edit("grid.create", grid("G-1", "1", 0))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-2", "2", 6000))
        .await
        .unwrap();
    let g = |id: &str| reference(id, "grid.line");
    stack
        .edit(
            "dimension.create",
            horizontal("D-1", "1", &[g("G-1"), g("G-2")]),
        )
        .await
        .unwrap();
    let by_kind = json!({ "element": "D-2", "kind": "dimension", "level": 0,
                          "placement": { "x": 0, "y": 0, "z": 0 },
                          "recipe": recipe("dimension", json!({})) });
    assert_eq!(
        stack.edit("element.create", by_kind).await.unwrap_err(),
        "A dimension is made: use dimension.create. Dimensions are made and changed by the \
         dimension commands."
    );
    for (cmd, args, want) in [
        (
            "element.delete",
            json!({ "element": "D-1" }),
            "A dimension is deleted: use dimension.delete.",
        ),
        (
            "element.move",
            json!({ "element": "D-1", "placement": { "x": 1, "y": 1, "z": 0 } }),
            "A dimension is changed: use dimension.move",
        ),
        (
            "element.set",
            json!({ "element": "D-1", "name": "x" }),
            "A dimension is changed: use dimension.move",
        ),
        (
            "element.param",
            json!({ "element": "D-1", "parameter": "x", "value": "1" }),
            "",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert!(refused.starts_with(want), "{cmd}: {refused}");
    }
    assert!(stack.exists("D-1").await);
    // A dimension holds a view: deleting the view is refused until its dimensions are gone.
    let why = stack
        .edit("view.delete", json!({ "view": "V-g" }))
        .await
        .unwrap_err();
    assert_eq!(
        why,
        "The view holds 1 dimension(s), such as D-1; delete them with dimension.delete first."
    );
    stack
        .edit("dimension.delete", json!({ "dimension": "D-1" }))
        .await
        .unwrap();
    assert!(!stack.exists("D-1").await);
    assert_eq!(
        stack
            .edit("dimension.delete", json!({ "dimension": "D-1" }))
            .await
            .unwrap_err(),
        "There is no dimension D-1 in this model."
    );
    stack
        .edit("view.delete", json!({ "view": "V-g" }))
        .await
        .unwrap();
}

#[tokio::test]
async fn an_undo_may_not_leave_a_dimension_in_a_view_that_is_not_there() {
    let stack = Stack::new().await;
    stack
        .edit("level.create", level("L-g", "Ground", "0"))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-1", "1", 0))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-2", "2", 6000))
        .await
        .unwrap();
    stack.edit("view.create", plan("V-g", "L-g")).await.unwrap();
    stack.later();
    // Ben puts a dimension in Anna's new plan; her undo of making it would leave it on nothing.
    stack
        .edit_as(
            "ben",
            "dimension.create",
            horizontal(
                "D-1",
                "1",
                &[reference("G-1", "grid.line"), reference("G-2", "grid.line")],
            ),
        )
        .await
        .unwrap();
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert!(refused.starts_with(CANNOT), "{refused}");
    assert!(refused.contains("the dimension D-1 would be left in a view that is not a plan there"));
    assert!(stack.exists("V-g").await);
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
async fn every_dimension_command_is_undone_and_redone() {
    let stack = with_plan().await;
    for (id, x) in [("G-1", 0), ("G-2", 3000), ("G-3", 5000)] {
        stack.edit("grid.create", grid(id, id, x)).await.unwrap();
    }
    let g = |id: &str| reference(id, "grid.line");
    undone_and_redone(
        &stack,
        "dimension.create",
        horizontal("D-1", "1", &[g("G-1"), g("G-2")]),
    )
    .await;
    // The redo leaves D-1 in place; carry on with it.
    for (cmd, args) in [
        (
            "dimension.reference.add",
            json!({ "dimension": "D-1", "reference": g("G-3") }),
        ),
        (
            "dimension.move",
            json!({ "dimension": "D-1", "line": "2 m" }),
        ),
        (
            "dimension.equal",
            json!({ "dimension": "D-1", "groups": [[0, 1]] }),
        ),
        (
            "dimension.equal",
            json!({ "dimension": "D-1", "groups": [] }),
        ),
        (
            "dimension.reference.remove",
            json!({ "dimension": "D-1", "at": 0 }),
        ),
        ("dimension.delete", json!({ "dimension": "D-1" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn another_persons_claim_turns_a_change_back() {
    let stack = with_plan().await;
    stack
        .edit("grid.create", grid("G-1", "1", 0))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-2", "2", 6000))
        .await
        .unwrap();
    let g = |id: &str| reference(id, "grid.line");
    stack
        .edit(
            "dimension.create",
            horizontal("D-1", "1", &[g("G-1"), g("G-2")]),
        )
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit_as(
                "ben",
                "dimension.move",
                json!({ "dimension": "D-1", "line": "5" })
            )
            .await
            .unwrap_err(),
        "anna is working on this."
    );
    stack.later();
    stack
        .edit_as(
            "ben",
            "dimension.move",
            json!({ "dimension": "D-1", "line": "5" }),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn dimensions_have_no_shape_and_add_no_derived_row() {
    let stack = with_plan().await;
    stack
        .edit("element.create", box_wall("W-1", 0, 0, "x"))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-1", "1", 0))
        .await
        .unwrap();
    stack
        .edit("grid.create", grid("G-2", "2", 6000))
        .await
        .unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    let before = stack.derived().await;
    let checked = stack.regen.verify(&i("p")).await;
    let g = |id: &str| reference(id, "grid.line");
    stack
        .edit(
            "dimension.create",
            horizontal("D-1", "1", &[g("G-1"), g("G-2")]),
        )
        .await
        .unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    let after = stack.derived().await;
    // The only derived row a dimension adds is its kind row, which every record has.
    let added: Vec<&String> = after
        .keys()
        .filter(|key| !before.contains_key(*key))
        .collect();
    assert!(added.iter().all(|key| key.contains("kind")), "{added:?}");
    assert_eq!(stack.regen.verify(&i("p")).await, checked);
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = with_plan().await;
    let g = |id: &str| reference(id, "grid.line");
    for (cmd, args) in [
        ("grid.create", grid("G-1", "1", 0)),
        ("grid.create", grid("G-2", "2", 6000)),
        ("grid.create", grid("G-3", "3", 9000)),
        (
            "dimension.create",
            horizontal("D-1", "1", &[g("G-1"), g("G-2")]),
        ),
        (
            "dimension.create",
            horizontal("D-2", "2", &[g("G-1"), g("G-1")]),
        ),
        (
            "dimension.reference.add",
            json!({ "dimension": "D-1", "reference": g("G-3") }),
        ),
        (
            "dimension.equal",
            json!({ "dimension": "D-1", "groups": [[0, 1]] }),
        ),
        (
            "dimension.move",
            json!({ "dimension": "D-1", "line": "2.5 m" }),
        ),
        ("grid.delete", json!({ "grid": "G-3" })),
        ("edit.undo", json!({})),
        (
            "dimension.equal",
            json!({ "dimension": "D-1", "groups": [] }),
        ),
        (
            "dimension.reference.remove",
            json!({ "dimension": "D-1", "at": 2 }),
        ),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
        ("view.delete", json!({ "view": "V-g" })),
        ("dimension.delete", json!({ "dimension": "D-1" })),
        ("edit.undo", json!({})),
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
    assert!(entries.iter().any(|entry| entry.cmd == "dimension.equal"));

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
}

#[tokio::test]
async fn a_placed_windows_centre_and_edges_come_from_its_opening() {
    let stack = with_plan().await;
    a_layered_wall(&stack, "W-1").await;
    stack
        .edit("grid.create", grid("G-A", "A", 1000))
        .await
        .unwrap();
    // A 1200 mm window, its middle 1200 along the wall from its start (x 1000): the opening
    // reaches from 1600 to 2800 and its centre is at 2200.
    stack
        .edit(
            "family.place",
            json!({ "element": "F-1", "family": "library.doors-windows.fixed-window",
                    "type": "w1200x1200", "host": "W-1", "side": "exterior",
                    "along": "1200 mm" }),
        )
        .await
        .unwrap();
    let g = reference("G-A", "grid.line");
    stack
        .edit(
            "dimension.create",
            horizontal(
                "D-1",
                "1500",
                &[
                    g,
                    reference("F-1", "edge.start"),
                    reference("F-1", "centre"),
                    reference("F-1", "edge.end"),
                ],
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        values(&shown(&stack, "D-1").await),
        [stored(mm(600)), stored(mm(600)), stored(mm(600))]
    );
    // The window moves along its wall; the dimension follows.
    stack
        .edit(
            "family.move",
            json!({ "element": "F-1", "host": "W-1", "along": "2 m" }),
        )
        .await
        .unwrap();
    assert_eq!(
        values(&shown(&stack, "D-1").await),
        [stored(mm(1400)), stored(mm(600)), stored(mm(600))]
    );
    // A wall has no centre or edges of its own.
    let refused = stack
        .edit(
            "dimension.reference.add",
            json!({ "dimension": "D-1", "reference": reference("W-1", "edge.end") }),
        )
        .await
        .unwrap_err();
    assert!(
        refused.contains("it is not hosted on a wall at an offset"),
        "{refused}"
    );
}
