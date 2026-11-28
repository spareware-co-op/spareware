//! Angular, radial, diameter, arc length and ordinate dimensions through the edit log, on the
//! test store, with hand-worked values: a right angle and a 3:4 grid, a curved grid's radius,
//! diameter and length, ordinate values along a run; a three-segment string made equal by moving
//! grids, walls and doors as their own commands move them, refused by name when another person is
//! working on one or a box wall cannot move by part of a millimetre; dimension styles and
//! overrides; undo and redo of every new command; the log replayed from empty. The harness is the
//! one of `tests/dimensions.rs` (piece A81), kept whole so each file stands alone.

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

use construct_api::cloud::dimensions::{self, DimensionListing, DimensionView, PointView};
use construct_api::cloud::undo::{self, State};
use construct_api::cloud::{Access, ProjectCommands, walls};
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

fn angle_text(view: &DimensionView) -> Option<String> {
    view.segments.first().and_then(|s| s.text.clone())
}

fn texts(view: &DimensionView) -> Vec<Option<String>> {
    view.segments.iter().map(|s| s.text.clone()).collect()
}

/// A grid line `id` from `(x0, y0)` to `(x1, y1)` millimetres.
fn grid_line(id: &str, name: &str, from: (i32, i32), to: (i32, i32)) -> Value {
    json!({ "grid": id, "name": name, "x0": from.0.to_string(), "y0": from.1.to_string(),
            "x1": to.0.to_string(), "y1": to.1.to_string() })
}

/// A grid arc `id` about `(cx, cy)` of radius `r` millimetres, anticlockwise from `start` to `end`
/// degrees.
fn grid_arc(id: &str, name: &str, centre: (i32, i32), r: i32, start: &str, end: &str) -> Value {
    json!({ "grid": id, "name": name, "cx": centre.0.to_string(), "cy": centre.1.to_string(),
            "radius": r.to_string(), "start": start, "end": end })
}

fn made(id: &str, layout: &Value, references: &[Value]) -> Value {
    json!({ "dimension": id, "view": "V-g", "layout": layout, "references": references })
}

fn place(x: &str, y: &str) -> Value {
    json!({ "x": x, "y": y })
}

/// A grid line's first x, in steps, as its record holds it.
async fn grid_x(stack: &Stack, id: &str) -> i64 {
    let record = stack.record(id).await;
    let datum = construct_grids::datum_of_recipe(&record.recipe)
        .unwrap()
        .unwrap();
    match datum {
        construct_grids::Datum::Grid(construct_grids::Shape::Line { from, .. }) => from.x.steps(),
        other => panic!("{id} is not a straight grid: {other:?}"),
    }
}

#[tokio::test]
async fn an_angle_between_lines_is_exact_at_a_right_angle_and_the_nearest_unit_otherwise() {
    let stack = with_plan().await;
    a_layered_wall(&stack, "W-1").await;
    stack
        .edit("grid.create", grid("G-A", "A", 1000))
        .await
        .unwrap();
    // The wall runs east from (1000, 2000); the grid north through x = 1000: square.
    stack
        .edit(
            "dimension.create",
            made(
                "D-R",
                &json!({ "kind": "angular", "place": place("1500", "2500") }),
                &[reference("W-1", "wall.line"), reference("G-A", "grid.line")],
            ),
        )
        .await
        .unwrap();
    let square = shown(&stack, "D-R").await;
    assert_eq!(square.kind, "angular");
    assert_eq!(angle_text(&square).as_deref(), Some("90.00°"));
    assert!(square.segments[0].exact);
    assert_eq!(square.segments[0].angle.as_deref(), Some("angle:810000000"));
    assert_eq!(square.figure.kind, "arc");
    // The place was 500 mm along each line from the corner: the arc's radius is its distance.
    assert_eq!(square.layout.radius, stored(Length::from_steps(4_525_483)));
    assert_eq!(square.layout.first.as_deref(), Some("forward"));
    assert_eq!(square.layout.second.as_deref(), Some("forward"));
    // A grid at 3:4 from the first: 36.87 degrees, not exact.
    stack
        .edit(
            "grid.create",
            grid_line("G-S", "S", (1000, -4000), (4000, 0)),
        )
        .await
        .unwrap();
    stack
        .edit(
            "dimension.create",
            made(
                "D-S",
                &json!({ "kind": "angular", "place": place("1500", "1200") }),
                &[reference("G-A", "grid.line"), reference("G-S", "grid.line")],
            ),
        )
        .await
        .unwrap();
    let slanted = shown(&stack, "D-S").await;
    assert_eq!(angle_text(&slanted).as_deref(), Some("36.87°"));
    assert!(!slanted.segments[0].exact);
    assert_eq!(
        slanted.segments[0].angle.as_deref(),
        Some("angle:331829079")
    );
    // Moved by a place on the other side of the grid: the angle opens the other way round.
    stack
        .edit(
            "dimension.move",
            json!({ "dimension": "D-S", "place": place("500", "1200") }),
        )
        .await
        .unwrap();
    let moved = shown(&stack, "D-S").await;
    assert_eq!(angle_text(&moved).as_deref(), Some("143.13°"));
    // Refused by name: parallel lines, a curve, a third reference, a place on the corner.
    stack
        .edit("grid.create", grid("G-B", "B", 5000))
        .await
        .unwrap();
    let refused = |layout: Value, references: Vec<Value>| {
        stack.edit("dimension.create", made("D-X", &layout, &references))
    };
    let at_place = json!({ "kind": "angular", "place": place("3000", "3000") });
    assert!(
        refused(
            at_place.clone(),
            vec![reference("G-A", "grid.line"), reference("G-B", "grid.line")]
        )
        .await
        .unwrap_err()
        .contains("parallel to reference 1")
    );
    assert_eq!(
        refused(
            at_place.clone(),
            vec![
                reference("G-A", "grid.line"),
                reference("G-B", "grid.line"),
                reference("G-S", "grid.line")
            ]
        )
        .await
        .unwrap_err(),
        "An angular dimension measures two references; this has 3."
    );
    assert!(
        refused(
            json!({ "kind": "angular", "place": place("1000", "2000") }),
            vec![reference("W-1", "wall.line"), reference("G-A", "grid.line")]
        )
        .await
        .unwrap_err()
        .contains("away from the point where its lines meet")
    );
    assert_eq!(
        refused(
            json!({ "kind": "angular", "place": place("1", "1"), "radius": "1" }),
            vec![reference("W-1", "wall.line"), reference("G-A", "grid.line")]
        )
        .await
        .unwrap_err(),
        "An angular layout takes a place, or first, second and radius."
    );
    // Its own fields work as well as a place.
    stack
        .edit(
            "dimension.create",
            made(
                "D-F",
                &json!({ "kind": "angular", "first": "backward", "second": "forward",
                        "radius": "600" }),
                &[reference("W-1", "wall.line"), reference("G-A", "grid.line")],
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        angle_text(&shown(&stack, "D-F").await).as_deref(),
        Some("90.00°")
    );
}

#[tokio::test]
async fn a_curved_grids_radius_diameter_and_length_are_dimensioned() {
    let stack = with_plan().await;
    stack
        .edit(
            "grid.create",
            grid_arc("G-C", "C", (1000, 2000), 6000, "0", "90"),
        )
        .await
        .unwrap();
    let curve = [reference("G-C", "grid.line")];
    for (id, kind, at) in [
        ("D-R", "radial", place("4000", "6000")),
        ("D-D", "diameter", place("4000", "6000")),
        ("D-L", "arc_length", place("1000", "8500")),
    ] {
        stack
            .edit(
                "dimension.create",
                made(id, &json!({ "kind": kind, "place": at }), &curve),
            )
            .await
            .unwrap();
    }
    let radius = shown(&stack, "D-R").await;
    assert_eq!(texts(&radius), [Some("6000 mm".to_owned())]);
    assert!(radius.segments[0].exact);
    assert_eq!(radius.figure.kind, "leader");
    assert_eq!(
        radius.figure.to,
        Some(PointView {
            x: Param::Length(mm(4600)).stored(),
            y: Param::Length(mm(6800)).stored()
        })
    );
    let diameter = shown(&stack, "D-D").await;
    assert_eq!(texts(&diameter), [Some("12000 mm".to_owned())]);
    let length = shown(&stack, "D-L").await;
    // A quarter of a 6 m circle: 3000 pi mm, 9424.778 mm, the nearest step 60,318,579.
    assert_eq!(values(&length), [stored(Length::from_steps(60_318_579))]);
    assert_eq!(texts(&length), [Some("9425 mm".to_owned())]);
    assert!(!length.segments[0].exact);
    assert_eq!(length.layout.offset, stored(mm(500)));
    // The arc reshaped: the dimensions follow.
    stack
        .edit(
            "grid.reshape",
            json!({ "grid": "G-C", "cx": "1000", "cy": "2000", "radius": "5000",
                    "start": "0", "end": "180" }),
        )
        .await
        .unwrap();
    assert_eq!(
        texts(&shown(&stack, "D-R").await),
        [Some("5000 mm".to_owned())]
    );
    assert_eq!(
        texts(&shown(&stack, "D-L").await),
        [Some("15708 mm".to_owned())]
    );
    // A straight grid has no radius; a curve has no straight line for a linear dimension.
    stack
        .edit("grid.create", grid("G-1", "1", 0))
        .await
        .unwrap();
    assert!(
        stack
            .edit(
                "dimension.create",
                made(
                    "D-X",
                    &json!({ "kind": "radial", "place": place("1", "1") }),
                    &[reference("G-1", "grid.line")]
                ),
            )
            .await
            .unwrap_err()
            .contains("is straight")
    );
    assert!(
        stack
            .edit(
                "dimension.create",
                horizontal(
                    "D-X",
                    "1",
                    &[reference("G-1", "grid.line"), curve[0].clone()]
                ),
            )
            .await
            .unwrap_err()
            .contains("a curved grid has no straight line to measure to")
    );
    // A string's references are added to; an arc's are not.
    assert!(
        stack
            .edit(
                "dimension.reference.add",
                json!({ "dimension": "D-R", "reference": reference("G-1", "grid.line") }),
            )
            .await
            .unwrap_err()
            .starts_with("A radial dimension keeps the references it was made with")
    );
}

#[tokio::test]
async fn ordinate_values_along_a_run_are_read_from_the_first() {
    let stack = with_plan().await;
    for (id, x) in [("G-1", 1000), ("G-2", 3400), ("G-3", 5000), ("G-4", 9250)] {
        stack.edit("grid.create", grid(id, id, x)).await.unwrap();
    }
    let g = |id: &str| reference(id, "grid.line");
    stack
        .edit(
            "dimension.create",
            json!({ "dimension": "D-O", "view": "V-g", "ordinate": true,
                    "layout": { "kind": "horizontal", "y": "-800" },
                    "references": [g("G-1"), g("G-2"), g("G-3"), g("G-4")] }),
        )
        .await
        .unwrap();
    let ordinate = shown(&stack, "D-O").await;
    assert_eq!(ordinate.kind, "ordinate");
    assert_eq!(
        texts(&ordinate),
        [
            Some("2400 mm".to_owned()),
            Some("4000 mm".to_owned()),
            Some("8250 mm".to_owned())
        ]
    );
    assert_eq!(ordinate.overall, None);
    assert_eq!(
        stack.record("D-O").await.props["@dimension.ordinate"],
        "yesno:yes"
    );
    // It holds no equalities.
    assert!(
        stack
            .edit(
                "dimension.equal",
                json!({ "dimension": "D-O", "groups": [[0, 1]] }),
            )
            .await
            .unwrap_err()
            .contains("an ordinate dimension has no string of segments to hold equal")
    );
}

#[tokio::test]
async fn a_three_segment_string_is_made_equal_in_one_step() {
    let stack = with_plan().await;
    for (id, x) in [("G-1", 0), ("G-2", 2000), ("G-3", 5000), ("G-4", 9000)] {
        stack.edit("grid.create", grid(id, id, x)).await.unwrap();
    }
    let g = |id: &str| reference(id, "grid.line");
    stack
        .edit(
            "dimension.create",
            horizontal("D-E", "8000", &[g("G-1"), g("G-2"), g("G-3"), g("G-4")]),
        )
        .await
        .unwrap();
    let before = stack.rows("piece").await;
    stack
        .edit(
            "dimension.equalize",
            json!({ "dimension": "D-E", "groups": [[0, 1, 2]] }),
        )
        .await
        .unwrap();
    let held = shown(&stack, "D-E").await;
    assert_eq!(values(&held), vec![stored(mm(3000)); 3]);
    assert_eq!(held.equalities[0].equal, Some(true));
    assert_eq!(held.equalities[0].holds, Some(true));
    assert_eq!(grid_x(&stack, "G-2").await, 3000 * 6400);
    assert_eq!(grid_x(&stack, "G-3").await, 6000 * 6400);
    assert_eq!(grid_x(&stack, "G-4").await, 9000 * 6400);
    // One step: the undo puts the grids and the dimension back, the redo makes them equal again.
    assert_eq!(
        stack
            .state("anna")
            .await
            .undo
            .map(|step| step.cmd)
            .as_deref(),
        Some("dimension.equalize")
    );
    let after = stack.rows("piece").await;
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, before);
    assert_eq!(grid_x(&stack, "G-2").await, 2000 * 6400);
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, after);
    // Already equal: nothing is written and no step is made.
    stack.later();
    stack
        .edit(
            "dimension.equalize",
            json!({ "dimension": "D-E", "groups": [[0, 1, 2]] }),
        )
        .await
        .unwrap();
    assert_eq!(stack.rows("piece").await, after);
    // A grid another person is working on is named, and nothing moves.
    stack
        .edit_as(
            "ben",
            "grid.move",
            json!({ "grid": "G-2", "dx": "100", "dy": "0" }),
        )
        .await
        .unwrap();
    let held_by_ben = stack.rows("piece").await;
    assert_eq!(
        stack
            .edit(
                "dimension.equalize",
                json!({ "dimension": "D-E", "groups": [[0, 1, 2]] }),
            )
            .await
            .unwrap_err(),
        "Reference 2 (the grid line G-2) cannot move: ben is working on it."
    );
    assert_eq!(stack.rows("piece").await, held_by_ben);
    // Once Ben's claim has run out, it moves back.
    stack.later();
    stack
        .edit(
            "dimension.equalize",
            json!({ "dimension": "D-E", "groups": [[0, 1, 2]] }),
        )
        .await
        .unwrap();
    assert_eq!(grid_x(&stack, "G-2").await, 3000 * 6400);
    // Groups are refused as dimension.equal refuses them, and none is not a group to make.
    assert!(
        stack
            .edit(
                "dimension.equalize",
                json!({ "dimension": "D-E", "groups": [[0, 5]] }),
            )
            .await
            .unwrap_err()
            .starts_with("Cannot make segments equal: segment 6 is not there")
    );
    assert!(
        stack
            .edit(
                "dimension.equalize",
                json!({ "dimension": "D-E", "groups": [] }),
            )
            .await
            .unwrap_err()
            .contains("dimension.equal with none clears them")
    );
}

/// A layered wall of the default partition along y at `x` millimetres, from y = 0 to 3000.
async fn a_wall_across(stack: &Stack, id: &str, x: i32) {
    let partition = walls::default_type_args().unwrap().remove(0);
    let ty = partition["type"].clone();
    let _ = stack.edit("wall.type.create", partition).await;
    let draw = json!({ "element": id, "type": ty, "start": [format!("{x} mm"), "0 mm"],
                       "end": [format!("{x} mm"), "3000 mm"], "level": 1, "height": "2700 mm" });
    stack.edit("wall.draw", draw).await.unwrap();
}

#[tokio::test]
async fn walls_doors_and_box_walls_move_as_their_own_commands_move_them() {
    let stack = with_plan().await;
    for (id, x) in [("W-1", 0), ("W-2", 1000), ("W-3", 4000)] {
        a_wall_across(&stack, id, x).await;
    }
    let line = |id: &str| reference(id, "wall.line");
    stack
        .edit(
            "dimension.create",
            horizontal("D-W", "-500", &[line("W-1"), line("W-2"), line("W-3")]),
        )
        .await
        .unwrap();
    stack
        .edit(
            "dimension.equalize",
            json!({ "dimension": "D-W", "groups": [[0, 1]] }),
        )
        .await
        .unwrap();
    // The middle wall's reference line moved to x = 2000: its start and end, to the step.
    let moved = stack.record("W-2").await;
    assert_eq!(
        moved.recipe.args["start"],
        json!(["length:q0:12800000", "length:q0:0"])
    );
    assert_eq!(
        moved.recipe.args["end"],
        json!(["length:q0:12800000", "length:q0:19200000"])
    );
    assert_eq!(
        values(&shown(&stack, "D-W").await),
        vec![stored(mm(2000)); 2]
    );

    // Two doors along a wall, between two grids 9 m apart: their centres go to 3 m and 6 m.
    stack.later();
    let partition = walls::default_type_args().unwrap().remove(0);
    let ty = partition["type"].clone();
    let draw = json!({ "element": "W-L", "type": ty, "start": ["0 mm", "6000 mm"],
                       "end": ["10000 mm", "6000 mm"], "level": 1, "height": "2700 mm" });
    stack.edit("wall.draw", draw).await.unwrap();
    for (id, along) in [("DR-1", "1000 mm"), ("DR-2", "2000 mm")] {
        stack
            .edit(
                "hosted.place",
                json!({ "element": id, "host": "W-L", "face": "box.exterior", "along": along,
                        "recipe": door_recipe(900) }),
            )
            .await
            .unwrap();
    }
    for (id, x) in [("G-A", 0), ("G-B", 9000)] {
        stack.edit("grid.create", grid(id, id, x)).await.unwrap();
    }
    stack
        .edit(
            "dimension.create",
            horizontal(
                "D-D",
                "7000",
                &[
                    reference("G-A", "grid.line"),
                    reference("DR-1", "centre"),
                    reference("DR-2", "centre"),
                    reference("G-B", "grid.line"),
                ],
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        values(&shown(&stack, "D-D").await),
        [stored(mm(1450)), stored(mm(1000)), stored(mm(6550))]
    );
    stack
        .edit(
            "dimension.equalize",
            json!({ "dimension": "D-D", "groups": [[0, 1, 2]] }),
        )
        .await
        .unwrap();
    assert_eq!(
        values(&shown(&stack, "D-D").await),
        vec![stored(mm(3000)); 3]
    );
    // Each moved along its wall, as hosted.move moves it: its edge 450 mm before its centre.
    assert_eq!(
        stack.record("DR-1").await.props["@host.along"],
        Param::Length(mm(2550)).stored()
    );
    assert_eq!(
        stack.record("DR-2").await.props["@host.along"],
        Param::Length(mm(5550)).stored()
    );

    // Box walls hold whole millimetres: a share of half a millimetre is refused by name.
    stack.later();
    for (id, x) in [("B-1", 0), ("B-2", 100), ("B-3", 1001)] {
        stack
            .edit("element.create", box_wall(id, x, 12_000, "y"))
            .await
            .unwrap();
    }
    stack
        .edit(
            "dimension.create",
            horizontal("D-B", "11000", &[line("B-1"), line("B-2"), line("B-3")]),
        )
        .await
        .unwrap();
    let refused = stack
        .edit(
            "dimension.equalize",
            json!({ "dimension": "D-B", "groups": [[0, 1]] }),
        )
        .await
        .unwrap_err();
    assert!(
        refused.starts_with(
            "Reference 2 (the reference line of B-2) cannot move: it is a box wall, which moves by \
             whole millimetres only"
        ),
        "{refused}"
    );
}

#[tokio::test]
async fn styles_and_overrides_change_what_is_shown_and_never_the_value() {
    let stack = with_plan().await;
    for (id, x) in [("G-1", 0), ("G-2", 6000)] {
        stack.edit("grid.create", grid(id, id, x)).await.unwrap();
    }
    let g = |id: &str| reference(id, "grid.line");
    stack
        .edit(
            "dimension.create",
            horizontal("D-1", "3000", &[g("G-1"), g("G-2")]),
        )
        .await
        .unwrap();
    let plain = shown(&stack, "D-1").await;
    assert_eq!(plain.style, "dimension-style.ticks");
    assert_eq!(plain.look.marker, "tick");
    // The built-in text is 2.5 mm on paper: 250 mm as drawn at 1:100.
    assert_eq!(plain.look.height, Param::Length(mm(250)).stored());
    assert_eq!(listing(&stack, "V-g").await.styles.len(), 2);
    // A person's own style: dots, text in the line, metres to three places.
    stack
        .edit(
            "dimension.style.create",
            json!({ "style": "S-1", "name": "Site", "marker": "dot", "text_place": "inline",
                    "length": { "unit": "m", "places": 3 }, "height": "3" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "dimension.style",
            json!({ "dimension": "D-1", "style": "S-1" }),
        )
        .await
        .unwrap();
    let styled = shown(&stack, "D-1").await;
    assert_eq!(styled.style, "S-1");
    assert_eq!(
        (styled.look.marker.as_str(), styled.look.text_place.as_str()),
        ("dot", "inline")
    );
    assert_eq!(texts(&styled), [Some("6.000 m".to_owned())]);
    assert_eq!(values(&styled), values(&plain));
    assert_eq!(
        styled.segments[0].text_height,
        Some(Param::Length(mm(300)).stored())
    );
    // Overrides: a prefix and a suffix around the value, then a text in its place; the true value
    // is always listed beside what is shown.
    stack
        .edit(
            "dimension.override",
            json!({ "dimension": "D-1", "prefix": "about", "suffix": "clear" }),
        )
        .await
        .unwrap();
    let around = shown(&stack, "D-1").await;
    assert_eq!(texts(&around), [Some("about 6.000 m clear".to_owned())]);
    assert_eq!(around.segments[0].value_text.as_deref(), Some("6.000 m"));
    assert!(around.segments[0].overridden);
    stack
        .edit(
            "dimension.override",
            json!({ "dimension": "D-1", "text": "VARIES" }),
        )
        .await
        .unwrap();
    let replaced = shown(&stack, "D-1").await;
    assert_eq!(texts(&replaced), [Some("VARIES".to_owned())]);
    assert_eq!(replaced.segments[0].value_text.as_deref(), Some("6.000 m"));
    assert_eq!(values(&replaced), values(&plain));
    assert!(
        stack
            .edit(
                "dimension.override",
                json!({ "dimension": "D-1", "text": " spaced" }),
            )
            .await
            .unwrap_err()
            .contains("no space at either end")
    );
    stack
        .edit("dimension.override", json!({ "dimension": "D-1" }))
        .await
        .unwrap();
    assert!(!shown(&stack, "D-1").await.segments[0].overridden);
    // The style changes, and the dimension shows it.
    stack
        .edit(
            "dimension.style.change",
            json!({ "style": "S-1", "length": null, "name": "Site plan" }),
        )
        .await
        .unwrap();
    assert_eq!(
        texts(&shown(&stack, "D-1").await),
        [Some("6000 mm".to_owned())]
    );
    assert_eq!(stack.record("S-1").await.name, "Site plan");
    // Refused by name: a built-in style changed, a name taken, a style in use deleted, a style
    // that is not there, the generic commands on a style.
    assert!(
        stack
            .edit(
                "dimension.style.change",
                json!({ "style": "dimension-style.ticks", "marker": "dot" }),
            )
            .await
            .unwrap_err()
            .starts_with("Ticks is a built-in style")
    );
    assert_eq!(
        stack
            .edit(
                "dimension.style.create",
                json!({ "style": "S-2", "name": "ARROWS" }),
            )
            .await
            .unwrap_err(),
        "There is already a style named Arrows."
    );
    assert!(
        stack
            .edit("dimension.style.delete", json!({ "style": "S-1" }))
            .await
            .unwrap_err()
            .starts_with("1 dimension(s), such as D-1, are in this style")
    );
    assert_eq!(
        stack
            .edit(
                "dimension.style",
                json!({ "dimension": "D-1", "style": "S-9" }),
            )
            .await
            .unwrap_err(),
        "There is no dimension style S-9 in this model."
    );
    assert!(
        stack
            .edit("element.delete", json!({ "element": "S-1" }))
            .await
            .unwrap_err()
            .starts_with("A dimension style is changed with dimension.style.change")
    );
    assert!(
        stack
            .edit(
                "element.create",
                json!({ "element": "S-3", "kind": "dimension.style", "level": 0,
                        "placement": { "x": 0, "y": 0, "z": 0 },
                        "recipe": construct_dimensions::style_recipe() }),
            )
            .await
            .unwrap_err()
            .starts_with("A dimension style is made: use dimension.style.create.")
    );
    // Back in the first built-in style (named by naming none), the style goes.
    stack
        .edit(
            "dimension.style",
            json!({ "dimension": "D-1", "style": null }),
        )
        .await
        .unwrap();
    assert!(
        !stack
            .record("D-1")
            .await
            .props
            .contains_key("@dimension.style")
    );
    // Ben deletes the style once nobody is in it; Anna's undo would put D-1 back in it, so it is
    // turned back, naming why; Ben's undo puts the style back.
    stack.later();
    stack
        .edit_as("ben", "dimension.style.delete", json!({ "style": "S-1" }))
        .await
        .unwrap();
    assert!(!stack.exists("S-1").await);
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert!(
        refused.contains("the dimension D-1 would be left in a style that is not there"),
        "{refused}"
    );
    stack.edit_as("ben", "edit.undo", json!({})).await.unwrap();
    assert!(stack.exists("S-1").await);
    stack.later();
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(shown(&stack, "D-1").await.style, "S-1");
}

#[tokio::test]
async fn every_new_command_is_undone_and_redone() {
    let stack = with_plan().await;
    for (id, x) in [("G-1", 0), ("G-2", 1000), ("G-3", 5000)] {
        stack.edit("grid.create", grid(id, id, x)).await.unwrap();
    }
    stack
        .edit(
            "grid.create",
            grid_arc("G-C", "C", (0, 0), 3000, "0", "270"),
        )
        .await
        .unwrap();
    let g = |id: &str| reference(id, "grid.line");
    stack
        .edit(
            "dimension.create",
            horizontal("D-1", "1", &[g("G-1"), g("G-2"), g("G-3")]),
        )
        .await
        .unwrap();
    for (cmd, args) in [
        (
            "dimension.create",
            made(
                "D-A",
                &json!({ "kind": "arc_length", "place": place("0", "3500") }),
                &[g("G-C")],
            ),
        ),
        (
            "dimension.move",
            json!({ "dimension": "D-A", "place": place("0", "4000") }),
        ),
        (
            "dimension.equalize",
            json!({ "dimension": "D-1", "groups": [[0, 1]] }),
        ),
        (
            "dimension.style.create",
            json!({ "style": "S-1", "name": "Mine", "from": "dimension-style.arrows" }),
        ),
        (
            "dimension.style.change",
            json!({ "style": "S-1", "centre_mark": false }),
        ),
        (
            "dimension.style",
            json!({ "dimension": "D-A", "style": "S-1" }),
        ),
        (
            "dimension.override",
            json!({ "dimension": "D-A", "suffix": "along the arc" }),
        ),
        (
            "dimension.style",
            json!({ "dimension": "D-A", "style": null }),
        ),
        ("dimension.style.delete", json!({ "style": "S-1" })),
    ] {
        stack.later();
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn replaying_the_new_commands_from_empty_gives_the_same_bytes() {
    let stack = with_plan().await;
    for (cmd, args) in [
        ("grid.create", grid("G-1", "1", 0)),
        ("grid.create", grid("G-2", "2", 2000)),
        ("grid.create", grid("G-3", "3", 7000)),
        (
            "grid.create",
            grid_arc("G-C", "C", (1000, 1000), 4000, "30", "300"),
        ),
        (
            "dimension.create",
            horizontal(
                "D-1",
                "1",
                &[
                    reference("G-1", "grid.line"),
                    reference("G-2", "grid.line"),
                    reference("G-3", "grid.line"),
                ],
            ),
        ),
        (
            "dimension.equalize",
            json!({ "dimension": "D-1", "groups": [[0, 1]] }),
        ),
        (
            "dimension.create",
            made(
                "D-A",
                &json!({ "kind": "angular", "place": place("500", "500") }),
                &[reference("G-1", "grid.line"), reference("G-C", "grid.line")],
            ),
        ),
        (
            "dimension.create",
            made(
                "D-R",
                &json!({ "kind": "diameter", "place": place("3000", "-2000") }),
                &[reference("G-C", "grid.line")],
            ),
        ),
        (
            "dimension.style.create",
            json!({ "style": "S-1", "name": "Mine", "angle_places": 4 }),
        ),
        (
            "dimension.style",
            json!({ "dimension": "D-R", "style": "S-1" }),
        ),
        (
            "dimension.override",
            json!({ "dimension": "D-R", "prefix": "Ø" }),
        ),
        ("edit.undo", json!({})),
        ("dimension.style.delete", json!({ "style": "S-1" })),
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
    // The angle to a curve is turned back, and the style in use is not deleted.
    assert!(entries.iter().any(|entry| entry.failed.is_some()));
    assert!(
        entries
            .iter()
            .any(|entry| entry.cmd == "dimension.equalize")
    );
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
    assert_eq!(listing(&again, "V-g").await, listing(&stack, "V-g").await);
}
