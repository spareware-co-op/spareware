//! Grid lines and reference planes through the edit log, on the test store: the commands, the
//! records they write (exactly what construct-grids writes), their refusals as the commands'
//! results, their display shape built by the regenerator, the log replayed, and undo and redo.

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
use construct_grids::{Datums, Grid, PlaneShape, Point, ReferencePlane, Shape};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::{Place, list};
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_params::{Angle, Length};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
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
        let geometry: Arc<dyn Geometry> = Arc::new(Boxes);
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

    async fn undo(&self, person: &str) -> Result<u64, String> {
        self.edit_as(person, "edit.undo", json!({})).await
    }

    async fn redo(&self, person: &str) -> Result<u64, String> {
        self.edit_as(person, "edit.redo", json!({})).await
    }

    /// The current record of an element, if it has one.
    async fn maybe(&self, id: &str) -> Option<Element> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let pointer = kit_store::Place::Piece {
            project: i("p"),
            document: i(DOCUMENT),
            piece: i(id),
        };
        let hash: Option<String> = read_row(&mut snapshot, &pointer.key()).await.unwrap();
        let record = kit_store::Place::Record {
            project: i("p"),
            hash: i(&hash?),
        };
        read_row(&mut snapshot, &record.key()).await.unwrap()
    }

    async fn record(&self, id: &str) -> Element {
        self.maybe(id).await.unwrap()
    }

    /// Every grid and reference plane of the document, as construct-grids reads them.
    async fn datums(&self, ids: &[&str]) -> Datums {
        let mut records = Vec::new();
        for id in ids {
            records.extend(self.maybe(id).await);
        }
        Datums::from_records(&records).unwrap()
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

fn mm(value: i32) -> Length {
    Length::from_mm(value)
}

fn pt(x: i32, y: i32) -> Point {
    Point::mm(x, y)
}

fn degrees(value: i64) -> Angle {
    Angle::from_units(value * Angle::UNITS_PER_DEGREE)
}

fn line(grid: &str, name: Value, x: i32) -> Value {
    json!({ "grid": grid, "name": name, "x0": x.to_string(), "y0": "-1000",
            "x1": x.to_string(), "y1": "7000" })
}

fn arc(grid: &str, name: &str) -> Value {
    json!({ "grid": grid, "name": name, "cx": "0", "cy": "0", "radius": "1 m",
            "start": "0", "end": "90" })
}

fn vertical(plane: &str, name: Value) -> Value {
    json!({ "plane": plane, "name": name, "x0": "3000", "y0": "0", "x1": "3000", "y1": "1000" })
}

fn horizontal(plane: &str, name: Value, elevation: &str) -> Value {
    json!({ "plane": plane, "name": name, "elevation": elevation })
}

#[tokio::test]
async fn grids_and_planes_are_made_through_the_log_as_construct_grids_writes_them() {
    let stack = Stack::new().await;
    stack
        .edit("grid.create", line("G-A", json!("A"), 6000))
        .await
        .unwrap();
    stack.edit("grid.create", arc("G-B", "B")).await.unwrap();
    stack
        .edit("plane.create", vertical("P-1", json!("Mid")))
        .await
        .unwrap();
    stack
        .edit("plane.create", horizontal("P-2", Value::Null, "-300"))
        .await
        .unwrap();

    let document = i(DOCUMENT);
    let expect_a = Grid::new(
        i("G-A"),
        "A",
        Shape::line(pt(6000, -1000), pt(6000, 7000)).unwrap(),
    )
    .unwrap();
    assert_eq!(stack.record("G-A").await, expect_a.to_record(&document));
    let quarter = Shape::arc(pt(0, 0), mm(1000), degrees(0), degrees(90)).unwrap();
    let expect_b = Grid::new(i("G-B"), "B", quarter).unwrap();
    assert_eq!(stack.record("G-B").await, expect_b.to_record(&document));
    let mid = PlaneShape::Vertical {
        from: pt(3000, 0),
        to: pt(3000, 1000),
    };
    let expect_mid = ReferencePlane::new(i("P-1"), Some("Mid"), mid).unwrap();
    assert_eq!(stack.record("P-1").await, expect_mid.to_record(&document));
    let low = PlaneShape::Horizontal {
        elevation: mm(-300),
    };
    let expect_low = ReferencePlane::new(i("P-2"), None, low).unwrap();
    assert_eq!(stack.record("P-2").await, expect_low.to_record(&document));

    // Every record carries its category, a grid's and a plane's, and no level.
    assert_eq!(stack.record("G-A").await.props["@category"], "3700");
    assert_eq!(stack.record("P-1").await.props["@category"], "3800");
    assert_eq!(stack.record("G-A").await.level, 0);

    // A named plane is found by name.
    let datums = stack.datums(&["G-A", "G-B", "P-1", "P-2"]).await;
    assert_eq!(datums.plane_named("mid").unwrap().id(), &i("P-1"));
}

#[tokio::test]
async fn a_grid_made_with_no_name_is_given_the_next_one() {
    let stack = Stack::new().await;
    let make = |grid: &'static str, after: Value| {
        let mut args = line(grid, Value::Null, 1000);
        args["after"] = after;
        args
    };
    for (id, after, name) in [
        ("G-1", Value::Null, "1"),
        ("G-2", json!("1"), "2"),
        ("G-3", Value::Null, "3"),
        ("G-4", json!("1"), "4"),
        ("G-5", json!("Y"), "Z"),
        ("G-6", json!("Z"), "AA"),
    ] {
        stack.edit("grid.create", make(id, after)).await.unwrap();
        assert_eq!(stack.record(id).await.name, name, "{id}");
    }
}

#[tokio::test]
async fn refusals_are_the_commands_result_in_the_data_crates_words() {
    let stack = Stack::new().await;
    stack
        .edit("grid.create", line("G-A", json!("A"), 0))
        .await
        .unwrap();
    stack
        .edit("plane.create", vertical("P-1", json!("Mid")))
        .await
        .unwrap();
    let before = stack.rows("piece").await;

    let refused = |cmd: &'static str, args: Value, words: &'static str| {
        let stack = &stack;
        async move {
            let reason = stack.edit(cmd, args.clone()).await.unwrap_err();
            assert!(reason.contains(words), "{cmd} {args}: {reason}");
        }
    };
    // Names.
    refused(
        "grid.create",
        line("G-2", json!("a"), 100),
        "Another grid is already named",
    )
    .await;
    refused(
        "grid.create",
        line("G-2", json!("  "), 100),
        "besides spaces",
    )
    .await;
    refused(
        "grid.rename",
        json!({ "grid": "G-A", "name": "" }),
        "besides spaces",
    )
    .await;
    refused(
        "plane.create",
        vertical("P-2", json!("MID")),
        "Another reference plane is already named",
    )
    .await;
    // Geometry.
    let same = json!({ "grid": "G-2", "name": "Z", "x0": "5", "y0": "5", "x1": "5", "y1": "5" });
    refused("grid.create", same, "needs two different points").await;
    let no_radius = json!({ "grid": "G-2", "name": "Z", "cx": "0", "cy": "0", "radius": "0",
                            "start": "0", "end": "90" });
    refused("grid.create", no_radius, "needs a radius above zero").await;
    let no_sweep = json!({ "grid": "G-2", "name": "Z", "cx": "0", "cy": "0", "radius": "5",
                           "start": "40", "end": "40" });
    refused("grid.create", no_sweep, "needs a start that is not its end").await;
    let both = json!({ "grid": "G-2", "name": "Z", "x0": "0", "y0": "0", "x1": "1", "y1": "1",
                       "radius": "5" });
    refused("grid.create", both, "a line or an arc, not both").await;
    refused(
        "grid.create",
        json!({ "grid": "G-2", "name": "Z" }),
        "needs its geometry",
    )
    .await;
    let half = json!({ "grid": "G-2", "name": "Z", "x0": "0", "y0": "0", "x1": "1" });
    refused("grid.create", half, "needs end y").await;
    let far = json!({ "grid": "G-2", "name": "Z", "x0": "0", "y0": "0", "x1": "900 m", "y1": "0" });
    assert!(
        stack.edit("grid.create", far).await.is_ok(),
        "900 m is inside the limit"
    );
    let past =
        json!({ "grid": "G-3", "name": "Y", "x0": "0", "y0": "0", "x1": "300000 m", "y1": "0" });
    refused("grid.create", past, "from the origin").await;
    // Typed text: a wrong unit, and a value between two steps of the kernel's grid.
    let unit = json!({ "grid": "G-2", "name": "Z", "x0": "0", "y0": "0", "x1": "3 kg", "y1": "0" });
    refused("grid.create", unit, "End x").await;
    let off =
        json!({ "grid": "G-2", "name": "Z", "x0": "0", "y0": "0", "x1": "0.001 mm", "y1": "0" });
    refused("grid.create", off, "End x").await;
    // The plane's shapes.
    refused(
        "plane.create",
        json!({ "plane": "P-2", "name": null }),
        "needs its geometry",
    )
    .await;
    let two = json!({ "plane": "P-2", "name": null, "x0": "0", "y0": "0", "x1": "1", "y1": "1",
                      "elevation": "0" });
    refused("plane.create", two, "vertical or horizontal, not both").await;
    refused(
        "plane.move",
        json!({ "plane": "P-1", "dz": "10" }),
        "no height of its own",
    )
    .await;
    // Who and what.
    refused(
        "grid.create",
        line("G-A", json!("Q"), 100),
        "is already a grid or a reference plane",
    )
    .await;
    refused(
        "grid.rename",
        json!({ "grid": "G-nope", "name": "Q" }),
        "No grid has the ID",
    )
    .await;
    refused(
        "grid.rename",
        json!({ "grid": "P-1", "name": "Q" }),
        "No grid has the ID",
    )
    .await;
    refused(
        "plane.rename",
        json!({ "plane": "G-A", "name": "Q" }),
        "No reference plane has the ID",
    )
    .await;
    refused("grid.rename", json!({ "name": "Q" }), "needs a grid").await;
    refused(
        "grid.delete",
        json!({ "grid": "G-nope" }),
        "No grid has the ID",
    )
    .await;
    // A line stays a line.
    let to_arc =
        json!({ "grid": "G-A", "cx": "0", "cy": "0", "radius": "5", "start": "0", "end": "9" });
    refused("grid.reshape", to_arc, "A line stays a line").await;

    // Nothing the refusals asked for was written (only the 900 m grid that was allowed).
    let mut after = stack.rows("piece").await;
    assert_eq!(after.len(), before.len() + 1);
    after.retain(|key, _| before.contains_key(key));
    assert_eq!(after, before);
}

#[tokio::test]
async fn a_grid_is_renamed_moved_reshaped_and_deleted() {
    let stack = Stack::new().await;
    stack
        .edit("grid.create", line("G-A", json!("A"), 0))
        .await
        .unwrap();
    stack.edit("grid.create", arc("G-B", "B")).await.unwrap();

    stack
        .edit("grid.rename", json!({ "grid": "G-A", "name": " A1 " }))
        .await
        .unwrap();
    assert_eq!(stack.record("G-A").await.name, "A1");
    // Its own name in another case is its own.
    stack
        .edit("grid.rename", json!({ "grid": "G-A", "name": "a1" }))
        .await
        .unwrap();
    assert_eq!(stack.record("G-A").await.name, "a1");

    // A move is relative: two moves add.
    let by = |dx: &str, dy: &str| json!({ "grid": "G-A", "dx": dx, "dy": dy });
    stack.edit("grid.move", by("100", "-50")).await.unwrap();
    stack
        .edit("grid.move", by("100 mm", "0.5 m"))
        .await
        .unwrap();
    let datums = stack.datums(&["G-A"]).await;
    let expect = Shape::line(pt(200, -550), pt(200, 7450)).unwrap();
    assert_eq!(datums.grid(&i("G-A")).unwrap().shape(), &expect);

    // An end dragged: the line's four numbers again.
    let drag = json!({ "grid": "G-A", "x0": "200", "y0": "-550", "x1": "700", "y1": "7450" });
    stack.edit("grid.reshape", drag).await.unwrap();
    let datums = stack.datums(&["G-A"]).await;
    let expect = Shape::line(pt(200, -550), pt(700, 7450)).unwrap();
    assert_eq!(datums.grid(&i("G-A")).unwrap().shape(), &expect);
    assert_eq!(stack.record("G-A").await.name, "a1", "the name stays");

    // An arc keeps being an arc; its angles are taken into 0 to 360.
    let swing = json!({ "grid": "G-B", "cx": "0", "cy": "0", "radius": "2 m", "start": "-90", "end": "180" });
    stack.edit("grid.reshape", swing).await.unwrap();
    let datums = stack.datums(&["G-B"]).await;
    let expect = Shape::arc(pt(0, 0), mm(2000), degrees(270), degrees(180)).unwrap();
    assert_eq!(datums.grid(&i("G-B")).unwrap().shape(), &expect);

    stack
        .edit("grid.delete", json!({ "grid": "G-A" }))
        .await
        .unwrap();
    assert!(stack.maybe("G-A").await.is_none());
    // The name is free again.
    stack
        .edit("grid.create", line("G-C", json!("A1"), 0))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_plane_is_renamed_moved_and_deleted() {
    let stack = Stack::new().await;
    stack
        .edit("plane.create", vertical("P-1", Value::Null))
        .await
        .unwrap();
    stack
        .edit("plane.create", horizontal("P-2", json!("Datum"), "0"))
        .await
        .unwrap();
    assert_eq!(
        stack.record("P-1").await.name,
        "",
        "no name is an empty one"
    );

    stack
        .edit("plane.rename", json!({ "plane": "P-1", "name": "Centre" }))
        .await
        .unwrap();
    assert_eq!(stack.record("P-1").await.name, "Centre");
    stack
        .edit("plane.rename", json!({ "plane": "P-1", "name": null }))
        .await
        .unwrap();
    assert_eq!(stack.record("P-1").await.name, "");
    // Taking a name away needs the word: a command that says nothing about it is refused.
    let missing = stack
        .edit("plane.rename", json!({ "plane": "P-1" }))
        .await
        .unwrap_err();
    assert!(missing.contains("needs a name"), "{missing}");

    stack
        .edit(
            "plane.move",
            json!({ "plane": "P-1", "dx": "250", "dy": "-10 mm" }),
        )
        .await
        .unwrap();
    stack
        .edit("plane.move", json!({ "plane": "P-2", "dz": "2.7 m" }))
        .await
        .unwrap();
    let datums = stack.datums(&["P-1", "P-2"]).await;
    let moved = PlaneShape::Vertical {
        from: pt(3250, -10),
        to: pt(3250, 990),
    };
    assert_eq!(datums.plane(&i("P-1")).unwrap().shape(), &moved);
    let up = PlaneShape::Horizontal {
        elevation: mm(2700),
    };
    assert_eq!(datums.plane(&i("P-2")).unwrap().shape(), &up);
    assert_eq!(datums.plane_named("DATUM").unwrap().id(), &i("P-2"));

    stack
        .edit("plane.delete", json!({ "plane": "P-2" }))
        .await
        .unwrap();
    assert!(stack.maybe("P-2").await.is_none());
}

#[tokio::test]
async fn another_persons_claim_refuses_and_a_grid_takes_the_claim_of_whoever_changes_it() {
    let stack = Stack::new().await;
    stack
        .edit("grid.create", line("G-A", json!("A"), 0))
        .await
        .unwrap();
    let rename = |name: &str| json!({ "grid": "G-A", "name": name });
    assert_eq!(
        stack.edit_as("ben", "grid.rename", rename("B")).await,
        Err("anna is working on this.".to_owned())
    );
    assert_eq!(stack.record("G-A").await.name, "A");
    stack
        .edit("element.release", json!({ "element": "G-A" }))
        .await
        .unwrap();
    stack
        .edit_as("ben", "grid.rename", rename("B"))
        .await
        .unwrap();
    assert_eq!(
        stack.edit("grid.delete", json!({ "grid": "G-A" })).await,
        Err("ben is working on this.".to_owned())
    );
}

#[tokio::test]
async fn the_element_commands_leave_grids_and_planes_to_their_own() {
    let stack = Stack::new().await;
    stack
        .edit("grid.create", line("G-A", json!("A"), 0))
        .await
        .unwrap();
    let set = stack
        .edit("element.set", json!({ "element": "G-A", "name": "Z" }))
        .await
        .unwrap_err();
    assert!(set.contains("their own commands"), "{set}");
    let moved = json!({ "element": "G-A", "placement": { "x": 1, "y": 0, "z": 0 } });
    assert!(
        stack
            .edit("element.move", moved)
            .await
            .unwrap_err()
            .contains("their own commands")
    );
    // A recipe of a grid's operation, whatever the kind, is refused on the way in and on a change.
    let recipe = serde_json::to_value(&stack.record("G-A").await.recipe).unwrap();
    let create = json!({ "element": "E-1", "kind": "wall", "level": 1,
                         "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe });
    assert!(
        stack
            .edit("element.create", create)
            .await
            .unwrap_err()
            .contains("their own commands")
    );
    let kind = json!({ "element": "E-2", "kind": "grid", "level": 1,
                       "placement": { "x": 0, "y": 0, "z": 0 },
                       "recipe": { "owner": "construct", "schema": 1, "op": "block",
                                   "op_version": 1, "args": {} } });
    assert!(
        stack
            .edit("element.create", kind)
            .await
            .unwrap_err()
            .contains("their own commands")
    );
    let wall = json!({ "element": "W-1", "kind": "wall", "level": 1,
                       "placement": { "x": 0, "y": 0, "z": 0 },
                       "recipe": { "owner": "construct", "schema": 1, "op": "wall.straight",
                                   "op_version": 1,
                                   "args": { "length": 3000, "thickness": 200, "height": 2700 } } });
    stack.edit("element.create", wall).await.unwrap();
    let into_grid = json!({ "element": "W-1", "recipe": recipe });
    assert!(
        stack
            .edit("element.set", into_grid)
            .await
            .unwrap_err()
            .contains("their own commands")
    );
    assert_eq!(stack.record("G-A").await.name, "A", "and nothing changed");
}

#[tokio::test]
async fn the_regenerator_gives_each_a_display_shape_the_same_after_a_rebuild() {
    let stack = Stack::new().await;
    stack
        .edit("grid.create", line("G-A", json!("A"), 6000))
        .await
        .unwrap();
    stack.edit("grid.create", arc("G-B", "B")).await.unwrap();
    stack
        .edit("plane.create", vertical("P-1", json!("Mid")))
        .await
        .unwrap();
    stack
        .edit("plane.create", horizontal("P-2", Value::Null, "-300"))
        .await
        .unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();

    let BuiltResult::Shape {
        bounds, displays, ..
    } = stack.built("G-A").await.unwrap().result
    else {
        panic!("a straight grid builds");
    };
    assert_eq!(bounds.min, [6000, -1000, 0]);
    assert_eq!(bounds.max, [6000, 7000, 3000]);
    assert_eq!(displays.len(), 2);
    let BuiltResult::Shape { bounds, .. } = stack.built("G-B").await.unwrap().result else {
        panic!("an arc grid builds");
    };
    assert_eq!(bounds.min, [0, 0, 0]);
    assert_eq!(bounds.max, [1000, 1000, 3000]);
    let BuiltResult::Shape { bounds, .. } = stack.built("P-2").await.unwrap().result else {
        panic!("a horizontal plane builds");
    };
    assert_eq!(bounds.min, [0, 0, -300]);

    // Every shape checks against its recipe, and a rebuild from the log makes the same rows.
    assert_eq!(stack.regen.verify(&i("p")).await, Ok(4));
    let before = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, before);
    // Each is listed under its own kind, so the page can find them.
    let mut snapshot = stack.store.snapshot().await.unwrap();
    for (kind, count) in [("grid", 2), ("refplane", 2)] {
        let list = list::kind(&i("p"), &i(DOCUMENT), &i(kind));
        assert_eq!(snapshot.scan(&list).await.unwrap().len(), count, "{kind}");
    }
}

/// Runs `cmd`, undoes it, redoes it: the log has the three, the records are as they were after
/// the command, before it, and after it again.
async fn undone_and_redone(stack: &Stack, cmd: &str, args: Value) {
    let before = stack.rows("piece").await;
    stack
        .edit(cmd, args)
        .await
        .unwrap_or_else(|why| panic!("{cmd}: {why}"));
    let after = stack.rows("piece").await;
    assert_ne!(before, after, "{cmd} changed something");
    stack
        .undo("anna")
        .await
        .unwrap_or_else(|why| panic!("undo {cmd}: {why}"));
    assert_eq!(stack.rows("piece").await, before, "undo {cmd}");
    stack
        .redo("anna")
        .await
        .unwrap_or_else(|why| panic!("redo {cmd}: {why}"));
    assert_eq!(stack.rows("piece").await, after, "redo {cmd}");
}

#[tokio::test]
async fn undo_and_redo_cover_each_command() {
    let stack = Stack::new().await;
    undone_and_redone(&stack, "grid.create", line("G-A", json!("A"), 0)).await;
    undone_and_redone(&stack, "grid.create", arc("G-B", "B")).await;
    undone_and_redone(
        &stack,
        "grid.rename",
        json!({ "grid": "G-A", "name": "A1" }),
    )
    .await;
    undone_and_redone(
        &stack,
        "grid.move",
        json!({ "grid": "G-A", "dx": "100", "dy": "0" }),
    )
    .await;
    let drag = json!({ "grid": "G-A", "x0": "100", "y0": "-1000", "x1": "100", "y1": "9000" });
    undone_and_redone(&stack, "grid.reshape", drag).await;
    undone_and_redone(&stack, "grid.delete", json!({ "grid": "G-A" })).await;
    undone_and_redone(&stack, "plane.create", vertical("P-1", json!("Mid"))).await;
    undone_and_redone(&stack, "plane.create", horizontal("P-2", Value::Null, "0")).await;
    undone_and_redone(
        &stack,
        "plane.rename",
        json!({ "plane": "P-1", "name": "Centre" }),
    )
    .await;
    undone_and_redone(
        &stack,
        "plane.rename",
        json!({ "plane": "P-1", "name": null }),
    )
    .await;
    undone_and_redone(&stack, "plane.move", json!({ "plane": "P-1", "dx": "50" })).await;
    undone_and_redone(
        &stack,
        "plane.move",
        json!({ "plane": "P-2", "dz": "2700" }),
    )
    .await;
    undone_and_redone(&stack, "plane.delete", json!({ "plane": "P-1" })).await;
}

#[tokio::test]
async fn an_undo_that_would_give_two_grids_one_name_is_turned_back_whole() {
    let stack = Stack::new().await;
    stack
        .edit("grid.create", line("G-1", json!("A"), 0))
        .await
        .unwrap();
    stack
        .edit("grid.delete", json!({ "grid": "G-1" }))
        .await
        .unwrap();
    // Ben takes the name while it is free.
    stack
        .edit_as("ben", "grid.create", line("G-2", json!("a"), 500))
        .await
        .unwrap();
    let before = stack.rows("piece").await;
    let refused = stack.undo("anna").await.unwrap_err();
    assert_eq!(
        refused,
        "That can't be undone: Another grid is already named \"A\"."
    );
    assert_eq!(stack.rows("piece").await, before, "nothing was written");
    assert!(stack.maybe("G-1").await.is_none());
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_records() {
    let stack = Stack::new().await;
    for (cmd, args) in [
        ("grid.create", line("G-A", json!("A"), 0)),
        ("grid.create", arc("G-B", "B")),
        ("grid.create", line("G-C", Value::Null, 12_000)),
        ("grid.create", line("G-D", json!("a"), 100)), // refused: a name in use
        ("plane.create", vertical("P-1", json!("Mid"))),
        ("plane.create", horizontal("P-2", Value::Null, "-300")),
        (
            "grid.move",
            json!({ "grid": "G-A", "dx": "250", "dy": "-250" }),
        ),
        ("grid.rename", json!({ "grid": "G-C", "name": "C" })),
        (
            "grid.reshape",
            json!({ "grid": "G-B", "cx": "0", "cy": "0", "radius": "3 m", "start": "0", "end": "45" }),
        ),
        ("plane.rename", json!({ "plane": "P-1", "name": null })),
        ("plane.move", json!({ "plane": "P-2", "dz": "5" })),
        ("edit.undo", json!({})),
        ("grid.delete", json!({ "grid": "G-C" })),
        ("plane.delete", json!({ "plane": "P-1" })),
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
    verify_log(&mut snapshot, &Scope::Project(i("p")))
        .await
        .unwrap();

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
    for table in ["record", "piece"] {
        assert_eq!(again.rows(table).await, stack.rows(table).await, "{table}");
    }
    let mut replayed = again.store.snapshot().await.unwrap();
    let after = read_log(&mut replayed, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert_eq!(after, entries);
    // The shapes come out the same too.
    stack.regen.catch_up(&i("p")).await.unwrap();
    again.regen.catch_up(&i("p")).await.unwrap();
    assert_eq!(again.derived().await, stack.derived().await);
}

#[tokio::test]
async fn a_recipe_is_the_one_construct_grids_reads() {
    // The records the commands wrote read back, whole, through construct-grids.
    let stack = Stack::new().await;
    stack
        .edit("grid.create", line("G-A", json!("A"), 0))
        .await
        .unwrap();
    stack
        .edit("plane.create", horizontal("P-2", Value::Null, "0"))
        .await
        .unwrap();
    let datums = stack.datums(&["G-A", "P-2"]).await;
    assert_eq!(datums.grids().len(), 1);
    assert_eq!(datums.planes().len(), 1);
    let recipe: Recipe = stack.record("G-A").await.recipe;
    assert!(construct_grids::datum_of_recipe(&recipe).is_some());
}
