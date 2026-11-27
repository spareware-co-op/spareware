//! Columns through the edit log, on the test store: column types made (and the three of our own
//! offered), a column placed at a point and many at once at grid intersections in one step, given
//! another type, base and top, moved (a column on grids is let go of them), turned and deleted; a
//! moved level or grid moving its columns by regeneration alone; every refusal in words (a top
//! below its base by name, two grids that do not cross, a type that is not a column type) and
//! nothing written by a refusal; the generic commands refused where a column's own exist; another
//! person's claim; undo and redo of every column command, a placement of many being one step; the
//! server's read-only view; and the log replayed from empty to the same bytes.

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

use construct_api::cloud::columns::{AtView, read_columns};
use construct_api::cloud::undo::{self, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::parameters as records;
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

    /// A model with levels Ground (0) and First (3 m), the default column types, and a 3 by 2 grid:
    /// vertical lines G-1, G-2, G-3 at x = 0, 6 m and 12 m, horizontal lines G-A and G-B at y = 0
    /// and 5 m.
    async fn new() -> Self {
        let stack = Self::bare().await;
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
            let args = json!({ "level": id, "name": name, "elevation": elevation });
            stack.edit("level.create", args).await.unwrap();
        }
        for (id, x) in [("G-1", "0"), ("G-2", "6 m"), ("G-3", "12 m")] {
            let args =
                json!({ "grid": id, "name": id, "x0": x, "y0": "-1 m", "x1": x, "y1": "6 m" });
            stack.edit("grid.create", args).await.unwrap();
        }
        for (id, y) in [("G-A", "0"), ("G-B", "5 m")] {
            let args =
                json!({ "grid": id, "name": id, "x0": "-1 m", "y0": y, "x1": "13 m", "y1": y });
            stack.edit("grid.create", args).await.unwrap();
        }
        stack.edit("column.defaults", json!({})).await.unwrap();
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

    /// Where a column stands, read from its built box: the centre in whole millimetres.
    async fn centre(&self, id: &str) -> [i64; 2] {
        let (lo, hi) = self.bounds(id).await.unwrap();
        [i64::midpoint(lo[0], hi[0]), i64::midpoint(lo[1], hi[1])]
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

const SQUARE: &str = "column-concrete-400x400";

/// `column.place`'s arguments: column `id` of the 400 by 400 type at (`x`, `y`) mm, from Ground to
/// First.
fn place(id: &str, x: i64, y: i64) -> Value {
    json!({ "element": id, "type": SQUARE, "level": "L-g", "at": [x.to_string(), y.to_string()],
            "top": { "level": "L-1" } })
}

/// The six crossings of the 3 by 2 grid, as `column.place.grids` takes them.
fn on_the_grid() -> Value {
    let mut columns = Vec::new();
    for (n, v) in ["G-1", "G-2", "G-3"].into_iter().enumerate() {
        for (m, h) in ["G-A", "G-B"].into_iter().enumerate() {
            columns.push(json!({ "element": format!("C-{}{}", n + 1, ["a", "b"][m]),
                                  "first": v, "second": h }));
        }
    }
    json!({ "type": SQUARE, "level": "L-g", "top": { "level": "L-1" }, "columns": columns })
}

/// A column is placed at a point: its record is kind column with its type, base and top ties and
/// the recipe of construct-columns; built it is the 400 by 400 box from Ground to First (3000 mm)
/// about the point; a name defaults to the ID; and the three default types are in the document, once.
#[tokio::test]
async fn a_column_is_placed_at_a_point_and_built() {
    let stack = Stack::new().await;
    stack
        .edit("column.place", place("C-1", 1000, 2000))
        .await
        .unwrap();
    let record = stack.record("C-1").await.unwrap();
    assert_eq!(record.kind, "column");
    assert_eq!(record.name, "C-1");
    assert_eq!(record.props.get("@category"), Some(&"1100".to_owned()));
    assert_eq!(record.props.get("@type"), Some(&SQUARE.to_owned()));
    assert_eq!(record.props.get("@level"), Some(&"L-g".to_owned()));
    assert_eq!(record.props.get("@top.level"), Some(&"L-1".to_owned()));
    assert_eq!(record.recipe.op, "column");
    assert_eq!(
        stack.bounds("C-1").await.unwrap(),
        ([800, 1800, 0], [1200, 2200, 3000])
    );
    // Offered once: the second time there is nothing to add, so no step.
    let before = stack.rows("piece").await;
    stack.edit("column.defaults", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, before);
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let view = read_columns(&mut snapshot, &i("p"), &i(DOCUMENT))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(view.types.len(), 3);
    assert_eq!(view.columns.len(), 1);
}

/// A column's top is a level and an offset, or an unconnected height; the base takes an offset too:
/// base 100 mm above Ground and top 50 mm below First stand 2850 mm; an unconnected 2700 mm from
/// Ground stands 2700 mm; a turn of 90 degrees turns the box.
#[tokio::test]
async fn base_top_height_and_turn_are_taken() {
    let stack = Stack::new().await;
    let mut args = place("C-1", 0, 0);
    args["offset"] = json!("100 mm");
    args["top"] = json!({ "level": "L-1", "offset": "-50 mm" });
    stack.edit("column.place", args).await.unwrap();
    assert_eq!(stack.bounds("C-1").await.unwrap().1[2], 2850);
    let mut args = place("C-2", 0, 0);
    args.as_object_mut().unwrap().remove("top");
    args["height"] = json!("2700 mm");
    stack.edit("column.place", args).await.unwrap();
    assert_eq!(stack.bounds("C-2").await.unwrap().1[2], 2700);
    let mut args = place("C-3", 0, 0);
    args["type"] = json!("column-timber-140x140");
    args["turn"] = json!("90");
    stack.edit("column.place", args).await.unwrap();
    assert_eq!(stack.bounds("C-3").await.unwrap().0[..2], [-70, -70]);
    // A section turned 30 degrees is built too, its corners each rounded once.
    stack
        .edit(
            "column.type.create",
            json!({ "type": "T-long", "name": "Long", "material": "concrete-regular",
                    "section": { "kind": "rect", "width": "600 mm", "depth": "200 mm" } }),
        )
        .await
        .unwrap();
    let mut args = place("C-4", 0, 0);
    args["type"] = json!("T-long");
    args["turn"] = json!("30");
    stack.edit("column.place", args).await.unwrap();
    // 600 by 200 turned 30 degrees: the box is 2 x (300 cos 30 + 100 sin 30) = 619.6 mm by
    // 2 x (300 sin 30 + 100 cos 30) = 473.2 mm.
    let (lo, hi) = stack.bounds("C-4").await.unwrap();
    assert_eq!([hi[0] - lo[0], hi[1] - lo[1]], [620, 474]);
}

/// Six columns at the crossings of a 3 by 2 grid are one command and one undo step; they stand on
/// the crossings; a grid moved by `grid.move` moves the columns on it and no other; the undo takes
/// all six back, the redo puts all six back.
#[tokio::test]
async fn many_columns_at_grid_intersections_are_one_step() {
    let stack = Stack::new().await;
    let before = stack.rows("piece").await;
    stack
        .edit("column.place.grids", on_the_grid())
        .await
        .unwrap();
    assert_eq!(stack.centre("C-1a").await, [0, 0]);
    assert_eq!(stack.centre("C-2b").await, [6000, 5000]);
    assert_eq!(stack.centre("C-3a").await, [12000, 0]);
    let step = stack.state("anna").await.undo.unwrap();
    assert_eq!(step.cmd, "column.place.grids");
    let after = stack.rows("piece").await;
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, before);
    assert!(stack.record("C-2a").await.is_none());
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, after);

    stack
        .edit(
            "grid.move",
            json!({ "grid": "G-2", "dx": "500 mm", "dy": "0" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.centre("C-2a").await, [6500, 0]);
    assert_eq!(stack.centre("C-2b").await, [6500, 5000]);
    assert_eq!(stack.centre("C-1a").await, [0, 0]);
    assert_eq!(stack.centre("C-3b").await, [12000, 5000]);
    // The records never changed with the grid.
    let record = stack.record("C-2a").await.unwrap();
    assert_eq!(record.recipe.args["at"]["grids"], json!(["G-2", "G-A"]));
}

/// A moved level moves its columns by regeneration alone: First up 200 mm makes every column
/// 3200 mm tall, and a column with an unconnected height keeps its height and follows its base.
#[tokio::test]
async fn a_moved_level_moves_its_columns() {
    let stack = Stack::new().await;
    stack
        .edit("column.place.grids", on_the_grid())
        .await
        .unwrap();
    let mut free = place("C-free", 0, 0);
    free["level"] = json!("L-1");
    free.as_object_mut().unwrap().remove("top");
    free["height"] = json!("2000 mm");
    stack.edit("column.place", free).await.unwrap();
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "3200 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.bounds("C-1a").await.unwrap().1[2], 3200);
    assert_eq!(stack.bounds("C-3b").await.unwrap().1[2], 3200);
    assert_eq!(stack.bounds("C-free").await.unwrap().1[2], 2000);
    assert_eq!(
        stack.built("C-free").await.elevation,
        Some("length:q0:20480000".to_owned())
    );
}

/// A top below its base is refused by name and writes nothing; a top at its base has no height;
/// the level that would put a placed column's top below its base is a build failure, not a
/// refusal, and the column is built again when the level is put right.
#[tokio::test]
async fn a_top_below_its_base_is_refused_by_name() {
    let stack = Stack::new().await;
    let before = stack.rows("piece").await;
    let mut down = place("C-1", 0, 0);
    down["level"] = json!("L-1");
    down["top"] = json!({ "level": "L-g" });
    assert_eq!(
        stack.edit("column.place", down).await.unwrap_err(),
        "C-1: the column's top is 3000 mm below its base: a column stands up from its base"
    );
    let mut flat = place("C-1", 0, 0);
    flat["top"] = json!({ "level": "L-g" });
    assert_eq!(
        stack.edit("column.place", flat).await.unwrap_err(),
        "C-1: the column's top is at its base: it has no height"
    );
    assert_eq!(stack.rows("piece").await, before);

    stack
        .edit("column.place", place("C-2", 0, 0))
        .await
        .unwrap();
    let words = stack
        .edit(
            "column.levels",
            json!({ "elements": ["C-2"], "top": { "level": "L-g", "offset": "-250 mm" } }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        words,
        "C-2: the column's top is 250 mm below its base: a column stands up from its base"
    );
    // A level moved under a column is a build failure, naming it, and recovers.
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "-100 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.bounds("C-2").await.unwrap_err(),
        "TopBelowBase: the column's top is 100 mm below its base: a column stands up from its base"
    );
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "2800 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.bounds("C-2").await.unwrap().1[2], 2800);
}

/// Every other refusal is in words and writes nothing: an element that exists, a type the model
/// lacks, a type that is not a column type, a level not in the model, a top given twice or not at
/// all, a point that is not a length, a size past 5 m or odd, grids that do not cross, grids the
/// model lacks, an empty or repeated list.
#[tokio::test]
async fn refusals_are_in_words_and_change_nothing() {
    let stack = Stack::new().await;
    stack
        .edit("column.place", place("C-1", 0, 0))
        .await
        .unwrap();
    stack
        .edit(
            "type.create",
            json!({ "type": "T-door", "name": "Door", "category": 300 }),
        )
        .await
        .unwrap();
    let before = stack.rows("piece").await;
    let with = |change: &dyn Fn(&mut Value)| {
        let mut args = place("C-9", 0, 0);
        change(&mut args);
        args
    };
    for (cmd, args, words) in [
        (
            "column.place",
            place("C-1", 0, 0),
            "That element already exists.",
        ),
        (
            "column.place",
            with(&|a| a["type"] = json!("T-none")),
            "no type has id T-none",
        ),
        (
            "column.place",
            with(&|a| a["type"] = json!("T-door")),
            "an instance of category 1100 cannot take a type of category 300",
        ),
        (
            "column.place",
            with(&|a| a["level"] = json!("L-none")),
            "There is no level L-none in this model.",
        ),
        (
            "column.place",
            with(&|a| a["height"] = json!("3 m")),
            "A column's top is a level or a height, not both.",
        ),
        (
            "column.place",
            with(&|a| {
                a.as_object_mut().unwrap().remove("top");
            }),
            "This command needs the column's top: a level (`top`) or a `height`.",
        ),
        ("column.place", with(&|a| a["turn"] = json!("a lot")), ""),
        (
            "column.place",
            with(&|a| {
                a.as_object_mut().unwrap().remove("at");
            }),
            "This command needs where it stands (`at`).",
        ),
        (
            "column.place.grids",
            json!({ "type": SQUARE, "level": "L-g", "top": { "level": "L-1" }, "columns": [] }),
            "This command needs at least one column.",
        ),
        (
            "column.delete",
            json!({ "elements": ["C-1", "C-1"] }),
            "C-1 is listed twice.",
        ),
        (
            "column.delete",
            json!({ "elements": ["C-nope"] }),
            "C-nope no longer exists.",
        ),
        (
            "column.delete",
            json!({ "elements": ["L-g"] }),
            "Ground is not a column.",
        ),
        (
            "column.type.create",
            json!({ "type": "T-big", "name": "Big", "material": "concrete-regular",
                    "section": { "kind": "round", "diameter": "5001 mm" } }),
            "a column's diameter is at most 5000 mm, and this one is 5001 mm",
        ),
        (
            "column.type.create",
            json!({ "type": "T-zero", "name": "Zero", "material": "concrete-regular",
                    "section": { "kind": "rect", "width": "0", "depth": "400 mm" } }),
            "a column's width is above zero, and this one is 0 mm",
        ),
    ] {
        let words_were = stack.edit(cmd, args).await.unwrap_err();
        assert!(
            words_were.starts_with(words) && (!words.is_empty() || !words_were.is_empty()),
            "{cmd}: {words_were}"
        );
    }
    // Grids that do not cross, and grids the model lacks, name the column.
    for (first, second, words) in [
        (
            "G-1",
            "G-2",
            "C-x: grids G-1 and G-2 do not cross, so a column cannot stand where they do",
        ),
        (
            "G-1",
            "G-nope",
            "C-x: the column stands on grid G-nope, which is not in this model",
        ),
    ] {
        let mut args = on_the_grid();
        args["columns"] = json!([
            { "element": "C-ok", "first": "G-1", "second": "G-A" },
            { "element": "C-x", "first": first, "second": second },
        ]);
        let words_were = stack.edit("column.place.grids", args).await.unwrap_err();
        assert_eq!(words_were, words);
    }
    assert_eq!(stack.rows("piece").await, before, "nothing was written");
    assert!(stack.record("C-ok").await.is_none());
}

/// A column's type, base and top, turn and position are changed by their commands: a changed type
/// changes every column of it; levels and offsets re-tie; a column on grids that is moved is let go
/// of them and stands where it was moved to; the same columns at once are one step.
#[tokio::test]
async fn columns_are_retyped_retied_moved_turned_and_deleted() {
    let stack = Stack::new().await;
    stack
        .edit("column.place.grids", on_the_grid())
        .await
        .unwrap();
    stack
        .edit(
            "column.type",
            json!({ "elements": ["C-1a", "C-1b"], "type": "column-concrete-round-450" }),
        )
        .await
        .unwrap();
    let (lo, hi) = stack.bounds("C-1a").await.unwrap();
    assert_eq!([hi[0] - lo[0], hi[1] - lo[1]], [450, 450]);
    assert_eq!(
        stack.record("C-1b").await.unwrap().props.get("@type"),
        Some(&"column-concrete-round-450".to_owned())
    );
    // A type's section changed changes every column of it.
    stack
        .edit(
            "column.type.section",
            json!({ "type": SQUARE, "section": { "kind": "rect", "width": "600 mm", "depth": "300 mm" } }),
        )
        .await
        .unwrap();
    let (lo, hi) = stack.bounds("C-2a").await.unwrap();
    assert_eq!([hi[0] - lo[0], hi[1] - lo[1]], [600, 300]);
    // Base and top.
    stack
        .edit(
            "column.levels",
            json!({ "elements": ["C-2a", "C-2b"], "offset": "200 mm", "height": "2500 mm" }),
        )
        .await
        .unwrap();
    let record = stack.record("C-2b").await.unwrap();
    assert_eq!(
        record.props.get("@level.offset"),
        Some(&"length:q0:1280000".to_owned())
    );
    assert!(!record.props.contains_key("@top.level"));
    assert_eq!(stack.bounds("C-2b").await.unwrap().1[2], 2500);
    // Moved: the column on grids stands where it was moved to, off them.
    stack
        .edit(
            "column.move",
            json!({ "elements": ["C-3a", "C-3b"], "dx": "100 mm", "dy": "-250 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.centre("C-3a").await, [12100, -250]);
    assert_eq!(stack.centre("C-3b").await, [12100, 4750]);
    stack
        .edit(
            "grid.move",
            json!({ "grid": "G-3", "dx": "1 m", "dy": "0" }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.centre("C-3a").await,
        [12100, -250],
        "let go of its grid"
    );
    // Turned.
    stack
        .edit("column.turn", json!({ "elements": ["C-2a"], "turn": "90" }))
        .await
        .unwrap();
    let (lo, hi) = stack.bounds("C-2a").await.unwrap();
    assert_eq!([hi[0] - lo[0], hi[1] - lo[1]], [300, 600]);
    // Deleted.
    stack
        .edit(
            "column.delete",
            json!({ "elements": ["C-1a", "C-1b", "C-2a"] }),
        )
        .await
        .unwrap();
    for gone in ["C-1a", "C-1b", "C-2a"] {
        assert!(stack.record(gone).await.is_none());
    }
    assert!(stack.record("C-2b").await.is_some());
}

/// The generic commands do not go behind the column commands.
#[tokio::test]
async fn the_generic_commands_are_refused_where_a_columns_own_exist() {
    let stack = Stack::new().await;
    stack
        .edit("column.place", place("C-1", 0, 0))
        .await
        .unwrap();
    let record = stack.record("C-1").await.unwrap();
    for (cmd, args, words) in [
        (
            "element.create",
            json!({ "element": "C-9", "kind": "column", "name": "x", "recipe": record.recipe }),
            "A column is made with column.place, which checks it as a column.",
        ),
        (
            "element.set",
            json!({ "element": "C-1", "recipe": record.recipe }),
            "C-1 is changed with the column commands, which checks it as a column.",
        ),
        (
            "element.move",
            json!({ "element": "C-1", "x": 10, "y": 10 }),
            "C-1 is moved with column.move, which checks it as a column.",
        ),
        (
            "type.create",
            json!({ "type": "T-c", "name": "Plain column", "category": 1100 }),
            "A column type is made with column.type.create, which checks it as a column.",
        ),
        (
            "type.rename",
            json!({ "type": SQUARE, "name": "Q" }),
            "A column type is renamed with column.type.rename, which checks it as a column.",
        ),
        (
            "type.delete",
            json!({ "type": SQUARE }),
            "A column type is deleted with column.type.delete, which checks it as a column.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), words, "{cmd}");
    }
    // The column type commands are the type commands, once it is known to be a column type: a type
    // in use is not deleted, one that is not a column type is not theirs.
    assert!(
        stack
            .edit("column.type.delete", json!({ "type": SQUARE }))
            .await
            .is_err()
    );
    stack
        .edit(
            "column.type.rename",
            json!({ "type": SQUARE, "name": "Square 400" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "column.type.duplicate",
            json!({ "type": SQUARE, "new": "T-copy", "name": "Square copy" }),
        )
        .await
        .unwrap();
    stack
        .edit("column.type.delete", json!({ "type": "T-copy" }))
        .await
        .unwrap();
    stack
        .edit(
            "type.create",
            json!({ "type": "T-door", "name": "Door", "category": 300 }),
        )
        .await
        .unwrap();
    assert!(
        stack
            .edit(
                "column.type.rename",
                json!({ "type": "T-door", "name": "x" })
            )
            .await
            .is_err()
    );
    // Deleting a column goes by the generic command too, and by its own.
    stack
        .edit("element.delete", json!({ "element": "C-1" }))
        .await
        .unwrap();
    assert!(stack.record("C-1").await.is_none());
}

/// Another person's claim turns a change back; once it has run out the change goes through.
#[tokio::test]
async fn another_persons_claim_turns_a_change_back() {
    let stack = Stack::new().await;
    stack
        .edit("column.place", place("C-1", 0, 0))
        .await
        .unwrap();
    stack
        .edit("column.turn", json!({ "elements": ["C-1"], "turn": "10" }))
        .await
        .unwrap();
    let refused = stack
        .edit_as(
            "ben",
            "column.turn",
            json!({ "elements": ["C-1"], "turn": "20" }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "anna is working on this.");
    stack.later();
    stack
        .edit_as(
            "ben",
            "column.turn",
            json!({ "elements": ["C-1"], "turn": "20" }),
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
async fn every_column_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    for (cmd, args) in [
        (
            "column.type.create",
            json!({ "type": "T-new", "name": "New", "material": "concrete-regular",
                    "section": { "kind": "round", "diameter": "300 mm" } }),
        ),
        (
            "column.type.section",
            json!({ "type": "T-new", "section": { "kind": "round", "diameter": "350 mm" } }),
        ),
        ("column.place", place("C-1", 0, 0)),
        ("column.place.grids", on_the_grid()),
        (
            "column.type",
            json!({ "elements": ["C-1"], "type": "T-new" }),
        ),
        (
            "column.levels",
            json!({ "elements": ["C-1", "C-1a"], "level": "L-1", "height": "2 m" }),
        ),
        (
            "column.move",
            json!({ "elements": ["C-1", "C-1a"], "dx": "1 m", "dy": "1 m" }),
        ),
        ("column.turn", json!({ "elements": ["C-1"], "turn": "45" })),
        (
            "column.delete",
            json!({ "elements": ["C-1", "C-1a", "C-2b"] }),
        ),
        (
            "column.type.rename",
            json!({ "type": "T-new", "name": "Newer" }),
        ),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

/// The server's read-only view: each type with its section and material, and each column with
/// where it stands, its tie or height, its turn, its state and what it reads back in the
/// document's units: a 400 by 400 column from Ground to First is 3000 mm tall, 160,000 mm2 of
/// section and 480,000,000 mm3 of concrete.
#[tokio::test]
async fn the_read_only_view_lists_types_and_columns() {
    let stack = Stack::new().await;
    stack
        .edit("column.place.grids", on_the_grid())
        .await
        .unwrap();
    let mut loose = place("C-p", 1500, 500);
    loose["turn"] = json!("37.5");
    stack.edit("column.place", loose).await.unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let view = read_columns(&mut snapshot, &i("p"), &i(DOCUMENT))
        .await
        .unwrap()
        .unwrap();
    let names: Vec<&str> = view.types.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Concrete 400 x 400",
            "Concrete round 450",
            "Timber 140 x 140"
        ]
    );
    assert_eq!(view.types[0].section, "rect");
    assert_eq!(view.types[0].width.as_deref(), Some("400 mm"));
    assert_eq!(view.types[1].diameter.as_deref(), Some("450 mm"));
    assert_eq!(view.columns.len(), 7);
    let first = view.columns.iter().find(|c| c.id == "C-1a").unwrap();
    assert_eq!(first.state, "built");
    assert_eq!(first.base.as_ref().unwrap().level, "L-g");
    assert_eq!(first.top.as_ref().unwrap().level, "L-1");
    assert_eq!(
        first.at,
        AtView::Grids {
            first: "G-1".to_owned(),
            second: "G-A".to_owned(),
            cross: 0
        }
    );
    let shown: Vec<(&str, Option<&str>)> = first
        .quantities
        .iter()
        .map(|q| (q.id.as_str(), q.shown.as_deref()))
        .collect();
    assert_eq!(shown[0], ("column.height", Some("3000 mm")));
    let volume = first
        .quantities
        .iter()
        .find(|q| q.id == "column.volume")
        .unwrap();
    assert_eq!(volume.shown.as_deref(), Some("0.48 m³"));
    let loose = view.columns.iter().find(|c| c.id == "C-p").unwrap();
    assert_eq!(loose.turn, "75/2");
    assert_eq!(
        loose.at,
        AtView::Point {
            x: "1500 mm".to_owned(),
            y: "500 mm".to_owned()
        }
    );
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    for (cmd, args) in [
        ("column.place.grids", on_the_grid()),
        ("column.place", place("C-p", 1500, 500)),
        (
            "column.turn",
            json!({ "elements": ["C-p"], "turn": "37.5" }),
        ),
        (
            "grid.move",
            json!({ "grid": "G-2", "dx": "500 mm", "dy": "0" }),
        ),
        (
            "level.elevation",
            json!({ "level": "L-1", "elevation": "3300 mm" }),
        ),
        (
            "column.move",
            json!({ "elements": ["C-3a"], "dx": "1 m", "dy": "0" }),
        ),
        ("edit.undo", json!({})),
        ("column.delete", json!({ "elements": ["C-1a"] })),
        ("column.delete", json!({ "elements": ["C-none"] })),
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
    let incremental = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, incremental);
}
