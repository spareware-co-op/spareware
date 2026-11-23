//! Sheets and viewports through the edit log, on the test store: the sheet and viewport commands
//! and their refusals, the generic element commands refused for them, sheet parameters through the
//! ordinary parameter command, a placed view kept, sheets with no shape, undo and redo of every
//! command, and the log replayed from empty.

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

use construct_api::cloud::undo::{self, CANNOT, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_params::Length;
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_sheets::{Anchor, Sheet, SheetError, Viewport};
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

fn sheet(id: &str, number: &str, name: &str) -> Value {
    json!({ "sheet": id, "number": number, "name": name })
}

/// A drafting view: it needs no level.
fn drafting(id: &str, name: &str) -> Value {
    json!({ "view": id, "kind": "drafting", "name": name })
}

fn viewport(id: &str, on: &str, view: &str) -> Value {
    json!({ "viewport": id, "sheet": on, "view": view })
}

fn wall(id: &str) -> Value {
    let args = json!({ "length": 3000, "thickness": 200, "height": 3000 });
    let recipe = serde_json::to_value(Recipe::construct(
        "wall.straight",
        serde_json::from_value(args).unwrap(),
    ))
    .unwrap();
    json!({ "element": id, "kind": "wall", "level": 0,
            "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe })
}

async fn the_sheet(stack: &Stack, id: &str) -> Sheet {
    Sheet::from_record(&stack.record(id).await).unwrap()
}

async fn the_viewport(stack: &Stack, id: &str) -> Viewport {
    Viewport::from_record(&stack.record(id).await).unwrap()
}

/// A sheet S-1 numbered A101, and two drafting views.
async fn with_sheet() -> Stack {
    let stack = Stack::new().await;
    stack
        .edit("sheet.create", sheet("S-1", "A101", "Ground floor"))
        .await
        .unwrap();
    for (id, name) in [("V-1", "Detail 1"), ("V-2", "Detail 2")] {
        stack.edit("view.create", drafting(id, name)).await.unwrap();
    }
    stack
}

#[tokio::test]
async fn a_sheet_is_made_with_the_defaults_and_the_standard_parameters() {
    let stack = Stack::new().await;
    stack
        .edit("sheet.create", sheet("S-1", "  A101 ", "Ground floor"))
        .await
        .unwrap();
    let made = the_sheet(&stack, "S-1").await;
    assert_eq!(made.number(), "A101");
    assert_eq!(made.paper().describe(), "A1 landscape (841 mm by 594 mm)");
    assert_eq!(stack.record("S-1").await.props["@category"], "7100");
    // The first sheet defines the standard parameters, bound to the sheets category.
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let context = construct_regen::parameters::read_context(&mut snapshot, &i("p"), &i(DOCUMENT))
        .await
        .unwrap()
        .unwrap();
    for (key, name) in construct_sheets::STANDARD {
        assert_eq!(context.table.definition(&i(key)).unwrap().name(), name);
    }
    // A second sheet changes the sheet only: the table is already right.
    let table_before = stack.rows("record").await.len();
    stack
        .edit("sheet.create", sheet("S-2", "A102", "First floor"))
        .await
        .unwrap();
    assert_eq!(stack.rows("record").await.len(), table_before + 1);
    // Paper by size and orientation, and custom to the step.
    let portrait = json!({ "sheet": "S-3", "number": "A3", "name": "P",
        "paper": { "size": "a3", "orientation": "portrait" } });
    stack.edit("sheet.create", portrait).await.unwrap();
    assert_eq!(the_sheet(&stack, "S-3").await.paper().width(), mm(297));
    let custom = json!({ "sheet": "S-4", "number": "D1", "name": "Detail",
        "paper": { "width": "297.5 mm", "height": "200" } });
    stack.edit("sheet.create", custom).await.unwrap();
    assert_eq!(
        the_sheet(&stack, "S-4").await.paper().width().steps(),
        1_904_000
    );
    assert_eq!(the_sheet(&stack, "S-4").await.paper().height(), mm(200));
}

#[tokio::test]
async fn a_sheet_is_refused_with_the_reason_it_has_in_the_crate() {
    let stack = with_sheet().await;
    let refused = |args: Value| {
        let stack = &stack;
        async move { stack.edit("sheet.create", args).await.unwrap_err() }
    };
    assert_eq!(
        refused(sheet("S-2", "a101", "Another")).await,
        SheetError::DuplicateNumber("a101".to_owned()).to_string()
    );
    assert_eq!(
        refused(sheet("S-1", "B1", "Same ID")).await,
        "That element already exists."
    );
    assert_eq!(
        refused(json!({ "sheet": "S-2", "number": "B1" })).await,
        "This command needs a name."
    );
    assert_eq!(
        refused(json!({ "sheet": "S-2", "name": "N" })).await,
        "This command needs a number."
    );
    assert_eq!(
        refused(sheet("S-2", "  ", "N")).await,
        SheetError::EmptyNumber.to_string()
    );
    assert_eq!(
        refused(sheet("S-2", "B1", " ")).await,
        SheetError::EmptyName.to_string()
    );
    let paper =
        |paper: Value| json!({ "sheet": "S-2", "number": "B1", "name": "N", "paper": paper });
    assert_eq!(
        refused(paper(json!({ "size": "a9" }))).await,
        "\"a9\" is not a paper size: a0, a1, a2, a3, a4, a5."
    );
    assert_eq!(
        refused(paper(json!({ "size": "a4", "orientation": "tilted" }))).await,
        "\"tilted\" is not an orientation: landscape or portrait."
    );
    assert_eq!(
        refused(paper(json!({ "size": "a4", "width": "100" }))).await,
        "Paper is a size (and an orientation), or a width and a height."
    );
    assert_eq!(
        refused(paper(json!({ "width": "49 mm", "height": "200" }))).await,
        "A custom paper width of 49 mm is outside 50 mm to 5000 mm."
    );
    assert!(
        refused(paper(json!({ "width": "100.001 mm", "height": "200" })))
            .await
            .contains("100.001")
    );
    assert!(
        stack
            .edit("sheet.create", json!({ "sheet": "S-2", "bogus": 1 }))
            .await
            .is_err()
    );
    assert!(!stack.exists("S-2").await);
}

#[tokio::test]
async fn a_sheet_is_set_by_the_rules_of_the_crate() {
    let stack = with_sheet().await;
    stack
        .edit("sheet.create", sheet("S-2", "A102", "First floor"))
        .await
        .unwrap();
    stack
        .edit(
            "sheet.set",
            json!({ "sheet": "S-1", "number": "A-101", "name": " Level 0 " }),
        )
        .await
        .unwrap();
    let now = the_sheet(&stack, "S-1").await;
    assert_eq!((now.number(), now.name()), ("A-101", "Level 0"));
    // Another sheet's number is taken; this sheet's own number, in another case, is not.
    assert_eq!(
        stack
            .edit("sheet.set", json!({ "sheet": "S-1", "number": "a102" }))
            .await
            .unwrap_err(),
        SheetError::DuplicateNumber("a102".to_owned()).to_string()
    );
    stack
        .edit("sheet.set", json!({ "sheet": "S-1", "number": "a-101" }))
        .await
        .unwrap();
    // Paper changes; paper that would leave a viewport's centre off it is refused.
    stack
        .edit(
            "viewport.create",
            json!({ "viewport": "P-1", "sheet": "S-1", "view": "V-1",
                                         "x": "800", "y": "500" }),
        )
        .await
        .unwrap();
    let a4 = json!({ "size": "a4" });
    let error = stack
        .edit("sheet.set", json!({ "sheet": "S-1", "paper": a4 }))
        .await
        .unwrap_err();
    assert_eq!(
        error,
        "A viewport centred at 800 mm, 500 mm is off sheet \"a-101\", which is A4 landscape (297 mm by 210 mm)."
    );
    stack
        .edit(
            "sheet.set",
            json!({ "sheet": "S-1", "paper": { "size": "a0", "orientation": "portrait" } }),
        )
        .await
        .unwrap();
    assert_eq!(the_sheet(&stack, "S-1").await.paper().height(), mm(1189));
    // What it already is writes nothing and is no step.
    let before = stack.rows("piece").await;
    let step = stack.state("anna").await.undo;
    stack
        .edit("sheet.set", json!({ "sheet": "S-1", "name": "Level 0" }))
        .await
        .unwrap();
    assert_eq!(stack.rows("piece").await, before);
    assert_eq!(stack.state("anna").await.undo, step);
    // Not a sheet, and not there.
    assert_eq!(
        stack
            .edit("sheet.set", json!({ "sheet": "V-1" }))
            .await
            .unwrap_err(),
        SheetError::UnknownSheet(i("V-1")).to_string()
    );
    assert_eq!(
        stack.edit("sheet.set", json!({})).await.unwrap_err(),
        "This command needs a sheet."
    );
}

#[tokio::test]
async fn sheet_parameters_are_the_ordinary_parameters_and_survive_a_rename() {
    let stack = with_sheet().await;
    stack
        .edit(
            "element.param",
            json!({ "element": "S-1", "parameter": "sheet.drawn-by", "value": "Cara" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "element.param",
            json!({ "element": "S-1", "parameter": "sheet.issue-date", "value": "2026-11-24" }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.record("S-1").await.props["sheet.drawn-by"],
        "text:Cara"
    );
    stack
        .edit(
            "sheet.set",
            json!({ "sheet": "S-1", "name": "Renamed", "paper": { "size": "a2" } }),
        )
        .await
        .unwrap();
    let record = stack.record("S-1").await;
    assert_eq!(record.props["sheet.drawn-by"], "text:Cara");
    assert_eq!(record.props["sheet.issue-date"], "text:2026-11-24");
    assert_eq!(Sheet::from_record(&record).unwrap().name(), "Renamed");
    // A parameter the sheets do not have, and a viewport, take no value.
    assert!(
        stack
            .edit(
                "element.param",
                json!({ "element": "S-1", "parameter": "nope", "value": "x" })
            )
            .await
            .is_err()
    );
    stack
        .edit("viewport.create", viewport("P-1", "S-1", "V-1"))
        .await
        .unwrap();
    assert!(
        stack
            .edit(
                "element.param",
                json!({ "element": "P-1", "parameter": "sheet.drawn-by", "value": "x" })
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_viewport_is_placed_at_the_centre_by_default_and_refused_by_name() {
    let stack = with_sheet().await;
    stack
        .edit("viewport.create", viewport("P-1", "S-1", "V-1"))
        .await
        .unwrap();
    let placed = the_viewport(&stack, "P-1").await;
    // A1 landscape is 841 by 594 mm; its centre is 420.5 by 297 mm.
    assert_eq!(
        (placed.x().steps(), placed.y().steps()),
        (2_691_200, 1_900_800)
    );
    assert!(placed.title());
    assert_eq!(stack.record("P-1").await.props["@category"], "7400");
    let again = stack
        .edit("viewport.create", viewport("P-2", "S-1", "V-1"))
        .await
        .unwrap_err();
    assert_eq!(
        again,
        "The view V-1 is already placed, on sheet \"A101\"; a view is on one sheet only."
    );
    stack
        .edit("sheet.create", sheet("S-2", "A102", "First floor"))
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit("viewport.create", viewport("P-2", "S-2", "V-1"))
            .await
            .unwrap_err(),
        again
    );
    for (args, why) in [
        (viewport("P-2", "S-9", "V-2"), SheetError::UnknownSheet(i("S-9")).to_string()),
        (viewport("P-2", "S-1", "V-9"), SheetError::UnknownView(i("V-9")).to_string()),
        (viewport("P-1", "S-1", "V-2"), "That element already exists.".to_owned()),
        (json!({ "viewport": "P-2", "view": "V-2" }), "This command needs a sheet.".to_owned()),
        (
            json!({ "viewport": "P-2", "sheet": "S-1", "view": "V-2", "x": "842", "y": "1" }),
            "A viewport centred at 842 mm, 1 mm is off sheet \"A101\", which is A1 landscape (841 mm by 594 mm)."
                .to_owned(),
        ),
        (
            json!({ "viewport": "P-2", "sheet": "S-1", "view": "V-2", "x": "-1", "y": "1" }),
            "A viewport centred at -1 mm, 1 mm is off sheet \"A101\", which is A1 landscape (841 mm by 594 mm)."
                .to_owned(),
        ),
    ] {
        assert_eq!(stack.edit("viewport.create", args).await.unwrap_err(), why);
    }
    // A wall is not a view; a view is not a sheet.
    stack.edit("element.create", wall("W-1")).await.unwrap();
    assert!(
        stack
            .edit("viewport.create", viewport("P-2", "S-1", "W-1"))
            .await
            .is_err()
    );
    assert!(
        stack
            .edit("viewport.create", viewport("P-2", "V-2", "V-2"))
            .await
            .is_err()
    );
    assert!(!stack.exists("P-2").await);
}

#[tokio::test]
async fn a_viewport_moves_exactly_and_an_aligned_axis_is_not_placed_by_hand() {
    let stack = with_sheet().await;
    stack
        .edit("viewport.create", viewport("P-1", "S-1", "V-1"))
        .await
        .unwrap();
    stack
        .edit(
            "viewport.create",
            json!({ "viewport": "P-2", "sheet": "S-1", "view": "V-2",
                                         "x": "100", "y": "100", "title": false }),
        )
        .await
        .unwrap();
    assert!(!the_viewport(&stack, "P-2").await.title());
    stack
        .edit(
            "viewport.set",
            json!({ "viewport": "P-1", "x": "12.5 mm", "y": "0.5 mm", "title": false }),
        )
        .await
        .unwrap();
    let moved = the_viewport(&stack, "P-1").await;
    assert_eq!((moved.x().steps(), moved.y().steps()), (80_000, 3_200));
    assert!(!moved.title());
    // Off the grid is refused by construct-params, never rounded.
    let off_grid = stack
        .edit(
            "viewport.set",
            json!({ "viewport": "P-1", "x": "0.001 mm" }),
        )
        .await
        .unwrap_err();
    assert!(off_grid.contains("0.001"), "{off_grid}");
    assert!(
        stack
            .edit("viewport.set", json!({ "viewport": "P-1", "x": "842 mm" }))
            .await
            .unwrap_err()
            .starts_with("A viewport centred at 842 mm")
    );
    assert!(
        stack
            .edit("viewport.set", json!({ "viewport": "P-1", "sheet": "S-1" }))
            .await
            .unwrap_err()
            .starts_with("A viewport keeps its sheet and its view")
    );
    // Aligned across the paper: its horizontal position follows P-1, and a hand placement is refused.
    let align = json!({ "viewport": "P-2", "across": { "to": "P-1", "anchor": "centre" } });
    stack.edit("viewport.align", align).await.unwrap();
    assert_eq!(
        stack
            .edit("viewport.set", json!({ "viewport": "P-2", "x": "300" }))
            .await
            .unwrap_err(),
        SheetError::Aligned("its horizontal position".to_owned()).to_string()
    );
    stack
        .edit(
            "viewport.set",
            json!({ "viewport": "P-2", "y": "300", "title": true }),
        )
        .await
        .unwrap();
    // What it already is writes nothing.
    let before = stack.rows("piece").await;
    stack
        .edit("viewport.set", json!({ "viewport": "P-2", "y": "300" }))
        .await
        .unwrap();
    assert_eq!(stack.rows("piece").await, before);
}

#[tokio::test]
async fn alignments_are_checked_and_kept() {
    let stack = with_sheet().await;
    stack
        .edit("view.create", drafting("V-3", "Detail 3"))
        .await
        .unwrap();
    stack
        .edit("sheet.create", sheet("S-2", "A102", "First floor"))
        .await
        .unwrap();
    for (id, on, view) in [
        ("P-1", "S-1", "V-1"),
        ("P-2", "S-1", "V-2"),
        ("P-3", "S-2", "V-3"),
    ] {
        stack
            .edit("viewport.create", viewport(id, on, view))
            .await
            .unwrap();
    }
    let to = |viewport: &str, axis: &str, other: &str, anchor: &str| json!({ "viewport": viewport, axis: { "to": other, "anchor": anchor } });
    stack
        .edit("viewport.align", to("P-2", "across", "P-1", "min"))
        .await
        .unwrap();
    stack
        .edit("viewport.align", to("P-2", "up", "P-1", "max"))
        .await
        .unwrap();
    let kept = the_viewport(&stack, "P-2").await;
    assert_eq!(kept.align_x().unwrap().anchor, Anchor::Min);
    assert_eq!(kept.align_y().unwrap().to, i("P-1"));
    for (args, starts) in [
        (
            to("P-1", "across", "P-1", "min"),
            "Cannot align: across the paper, a viewport cannot be aligned to itself",
        ),
        (
            to("P-1", "across", "P-2", "min"),
            "Cannot align: across the paper, a viewport cannot be aligned to itself or round a loop",
        ),
        (
            to("P-1", "up", "P-3", "min"),
            "Cannot align: up the paper, the viewport P-3 is on another sheet",
        ),
        (to("P-1", "up", "P-9", "min"), "There is no viewport P-9"),
        (
            to("P-1", "up", "P-2", "left"),
            "\"left\" is not an anchor: min, centre or max.",
        ),
        (
            json!({ "viewport": "P-1" }),
            "This command needs an alignment",
        ),
    ] {
        let why = stack.edit("viewport.align", args).await.unwrap_err();
        assert!(why.starts_with(starts), "{why}");
    }
    assert!(the_viewport(&stack, "P-1").await.align_x().is_none());
    // A followed viewport is not taken away; once nothing follows it, it is.
    let why = stack
        .edit("viewport.delete", json!({ "viewport": "P-1" }))
        .await
        .unwrap_err();
    assert_eq!(
        why,
        "The viewport P-1 is followed by P-2, which is aligned to it."
    );
    stack
        .edit(
            "viewport.align",
            json!({ "viewport": "P-2", "across": null, "up": null }),
        )
        .await
        .unwrap();
    assert!(the_viewport(&stack, "P-2").await.align_y().is_none());
    stack
        .edit("viewport.delete", json!({ "viewport": "P-1" }))
        .await
        .unwrap();
    assert!(!stack.exists("P-1").await);
    assert_eq!(
        stack
            .edit("viewport.delete", json!({ "viewport": "P-1" }))
            .await
            .unwrap_err(),
        SheetError::UnknownViewport(i("P-1")).to_string()
    );
    // The freed view can be placed again.
    stack
        .edit("viewport.create", viewport("P-4", "S-2", "V-1"))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_view_that_is_placed_is_not_deleted_and_a_deleted_sheet_frees_its_views() {
    let stack = with_sheet().await;
    stack
        .edit("viewport.create", viewport("P-1", "S-1", "V-1"))
        .await
        .unwrap();
    let why = stack
        .edit("view.delete", json!({ "view": "V-1" }))
        .await
        .unwrap_err();
    assert_eq!(
        why,
        "The view is placed on sheet A101 (Ground floor); take it off with viewport.delete P-1 first."
    );
    assert!(stack.exists("V-1").await);
    // Deleting the sheet takes its viewports with it, in one step.
    stack
        .edit("sheet.delete", json!({ "sheet": "S-1" }))
        .await
        .unwrap();
    assert!(!stack.exists("S-1").await && !stack.exists("P-1").await);
    stack
        .edit("view.delete", json!({ "view": "V-1" }))
        .await
        .unwrap();
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.exists("V-1").await && !stack.exists("S-1").await);
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.exists("S-1").await && stack.exists("P-1").await);
}

#[tokio::test]
async fn the_generic_commands_do_not_go_behind_the_sheet_commands() {
    let stack = with_sheet().await;
    stack
        .edit("viewport.create", viewport("P-1", "S-1", "V-1"))
        .await
        .unwrap();
    let recipe = |op: &str| serde_json::to_value(Recipe::construct(op, BTreeMap::new())).unwrap();
    let create = |kind: &str, id: &str| {
        json!({ "element": id, "kind": kind, "level": 0,
                "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe(kind) })
    };
    for (cmd, args, says) in [
        (
            "element.create",
            create("sheet", "S-9"),
            "use sheet.create or viewport.create",
        ),
        (
            "element.create",
            create("viewport", "P-9"),
            "use sheet.create or viewport.create",
        ),
        (
            "element.set",
            json!({ "element": "S-1", "name": "x" }),
            "A sheet is changed: use sheet.set",
        ),
        (
            "element.move",
            json!({ "element": "S-1", "placement": { "x": 1, "y": 0, "z": 0 } }),
            "A sheet is changed",
        ),
        (
            "element.delete",
            json!({ "element": "S-1" }),
            "A sheet is deleted: use sheet.delete.",
        ),
        (
            "element.set",
            json!({ "element": "P-1", "name": "x" }),
            "A viewport is changed: use viewport.set or viewport.align.",
        ),
        (
            "element.delete",
            json!({ "element": "P-1" }),
            "A viewport is deleted: use viewport.delete.",
        ),
    ] {
        let why = stack.edit(cmd, args).await.unwrap_err();
        assert!(why.contains(says), "{cmd}: {why}");
    }
    assert!(stack.exists("S-1").await && stack.exists("P-1").await && !stack.exists("S-9").await);
    // A wall with a sheet's recipe is refused too.
    let mut sneaky = wall("W-9");
    sneaky["recipe"] = recipe("sheet");
    assert!(stack.edit("element.create", sneaky).await.is_err());
}

#[tokio::test]
async fn another_persons_claim_on_a_sheet_or_viewport_refuses_the_edit() {
    let stack = with_sheet().await;
    stack
        .edit("viewport.create", viewport("P-1", "S-1", "V-1"))
        .await
        .unwrap();
    let mine = json!({ "sheet": "S-1", "name": "Mine" });
    assert_eq!(
        stack
            .edit_as("ben", "sheet.set", mine.clone())
            .await
            .unwrap_err(),
        "anna is working on this."
    );
    assert_eq!(
        stack
            .edit_as(
                "ben",
                "viewport.set",
                json!({ "viewport": "P-1", "title": false })
            )
            .await
            .unwrap_err(),
        "anna is working on this."
    );
    // Deleting a sheet is refused while one of its viewports is another's.
    stack.later();
    stack
        .edit_as(
            "ben",
            "viewport.set",
            json!({ "viewport": "P-1", "title": false }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit("sheet.delete", json!({ "sheet": "S-1" }))
            .await
            .unwrap_err(),
        "ben is working on this."
    );
    assert!(stack.exists("S-1").await);
    stack.later();
    stack.edit_as("ben", "sheet.set", mine).await.unwrap();
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
async fn every_sheet_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    for (id, name) in [("V-1", "Detail 1"), ("V-2", "Detail 2")] {
        stack.edit("view.create", drafting(id, name)).await.unwrap();
    }
    undone_and_redone(&stack, "sheet.create", sheet("S-1", "A101", "Ground floor")).await;
    undone_and_redone(&stack, "sheet.create", sheet("S-2", "A102", "First floor")).await;
    undone_and_redone(&stack, "viewport.create", viewport("P-1", "S-1", "V-1")).await;
    undone_and_redone(&stack, "viewport.create", viewport("P-2", "S-1", "V-2")).await;
    for (cmd, args) in [
        (
            "sheet.set",
            json!({ "sheet": "S-1", "number": "B-1", "name": "Renamed" }),
        ),
        (
            "sheet.set",
            json!({ "sheet": "S-1", "paper": { "size": "a1", "orientation": "portrait" } }),
        ),
        (
            "viewport.set",
            json!({ "viewport": "P-1", "x": "100 mm", "title": false }),
        ),
        (
            "viewport.align",
            json!({ "viewport": "P-2", "across": { "to": "P-1", "anchor": "centre" } }),
        ),
        (
            "viewport.align",
            json!({ "viewport": "P-2", "across": null }),
        ),
        ("viewport.delete", json!({ "viewport": "P-2" })),
        ("sheet.delete", json!({ "sheet": "S-1" })),
        ("sheet.delete", json!({ "sheet": "S-2" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn an_undo_may_not_leave_a_viewport_on_a_view_that_is_gone() {
    let stack = Stack::new().await;
    stack
        .edit("sheet.create", sheet("S-1", "A101", "Ground floor"))
        .await
        .unwrap();
    stack
        .edit("view.create", drafting("V-1", "Detail 1"))
        .await
        .unwrap();
    stack.later();
    // Ben places Anna's new view; her undo of making it would leave a viewport on nothing.
    stack
        .edit_as("ben", "viewport.create", viewport("P-1", "S-1", "V-1"))
        .await
        .unwrap();
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert_eq!(
        refused,
        format!("{CANNOT}{}", SheetError::UnknownView(i("V-1")))
    );
    assert!(stack.exists("V-1").await);
}

#[tokio::test]
async fn an_undo_may_not_leave_a_number_taken_twice() {
    let stack = Stack::new().await;
    stack
        .edit("sheet.create", sheet("S-1", "A101", "Ground floor"))
        .await
        .unwrap();
    stack
        .edit("sheet.set", json!({ "sheet": "S-1", "number": "A102" }))
        .await
        .unwrap();
    stack.later();
    // Ben takes the number Anna's rename gave up; her undo would give it back to a second sheet.
    stack
        .edit_as("ben", "sheet.create", sheet("S-2", "A101", "Ben's"))
        .await
        .unwrap();
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert_eq!(
        refused,
        format!("{CANNOT}{}", SheetError::DuplicateNumber("A101".to_owned()))
    );
    assert_eq!(the_sheet(&stack, "S-1").await.number(), "A102");
}

#[tokio::test]
async fn sheets_have_no_shape_and_add_no_derived_row() {
    let stack = Stack::new().await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    let mut before = BTreeMap::new();
    for table in ["body", "display", "built", "area"] {
        before.insert(table, stack.rows(table).await);
    }
    stack
        .edit("view.create", drafting("V-1", "Detail 1"))
        .await
        .unwrap();
    stack
        .edit("sheet.create", sheet("S-1", "A101", "Ground floor"))
        .await
        .unwrap();
    stack
        .edit("viewport.create", viewport("P-1", "S-1", "V-1"))
        .await
        .unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    for table in ["body", "display", "built", "area"] {
        assert_eq!(stack.rows(table).await, before[table], "{table}");
    }
    // Only the wall has a shape to check.
    assert_eq!(stack.regen.verify(&i("p")).await, Ok(1));
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    for (cmd, args) in [
        ("view.create", drafting("V-1", "Detail 1")),
        ("view.create", drafting("V-2", "Detail 2")),
        ("sheet.create", sheet("S-1", "A101", "Ground floor")),
        ("sheet.create", sheet("S-2", "a101", "Taken")),
        ("sheet.create", sheet("S-3", "A103", "Detail sheet")),
        ("viewport.create", viewport("P-1", "S-1", "V-1")),
        ("viewport.create", viewport("P-2", "S-1", "V-2")),
        ("viewport.create", viewport("P-3", "S-3", "V-1")),
        (
            "viewport.align",
            json!({ "viewport": "P-2", "across": { "to": "P-1", "anchor": "max" } }),
        ),
        ("viewport.set", json!({ "viewport": "P-2", "x": "1 mm" })),
        (
            "viewport.set",
            json!({ "viewport": "P-1", "x": "250.5 mm", "title": false }),
        ),
        (
            "sheet.set",
            json!({ "sheet": "S-1", "paper": { "width": "500", "height": "300 mm" } }),
        ),
        (
            "element.param",
            json!({ "element": "S-1", "parameter": "sheet.checked-by", "value": "Ben" }),
        ),
        ("view.delete", json!({ "view": "V-1" })),
        ("edit.undo", json!({})),
        ("viewport.delete", json!({ "viewport": "P-1" })),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
        ("sheet.delete", json!({ "sheet": "S-1" })),
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
    assert!(entries.iter().any(|entry| entry.cmd == "viewport.align"));

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
