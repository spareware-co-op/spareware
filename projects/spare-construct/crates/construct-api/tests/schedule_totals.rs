//! Schedules, second half, through the edit log on the test store: grouping with a header and a
//! footer, totals worked by hand, equal rows folded into one with a count, a calculated field and
//! its refusals in words, an edit in a cell going back to the element (one undo, claimed, read-only
//! fields refused in words), the sheet list and the view list, and a schedule placed on a sheet,
//! drawn from its live table, taken off with the schedule or the sheet.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::schedules::{ItemView, ScheduleTable, placed_on, read_table};
use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::{DOORS, SHEETS, VIEWS};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_regen::parameters::{self as records};
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::Memory;
use kit_store::{Kv, Scope, Txn, to_row};
use kit_sync::{Clock, Orderers, Reply};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
const CLAIM_MS: u64 = 30_000;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// A length in millimetres as it is stored: whole grid steps of 1/6,400 mm.
fn mm(n: i64) -> String {
    format!("length:q0:{}", n * 6400)
}

struct Yes;

impl Decider for Yes {
    fn decide(&self, _: &Question) -> Result<bool, String> {
        Ok(true)
    }
}

struct Fixed(AtomicU64);

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    async fn new() -> Self {
        let store = Memory::new();
        let geometry: Arc<dyn Geometry> = Arc::new(Boxes);
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, CLAIM_MS, geometry.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
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
        let stack = Self {
            store,
            orderers,
            clock,
            next: AtomicU64::new(0),
        };
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        stack
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

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
        self.edit_as("anna", cmd, args).await
    }

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

    async fn table(&self, id: &str) -> ScheduleTable {
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_table(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    }

    /// The text drawn for the schedules on a sheet, by placement.
    async fn drawn(&self, sheet: &str) -> Vec<(String, Vec<String>)> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        placed_on(&mut snapshot, &i("p"), &i(DOCUMENT), &i(sheet))
            .await
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|placed| {
                (
                    placed.placement,
                    placed.texts.into_iter().map(|t| t.text).collect(),
                )
            })
            .collect()
    }
}

fn recipe(op: &str, args: Value) -> Value {
    serde_json::to_value(Recipe::construct(op, serde_json::from_value(args).unwrap())).unwrap()
}

fn door(id: &str, name: &str) -> Value {
    let args = json!({ "width": 820, "thickness": 40, "height": 2100 });
    json!({ "element": id, "kind": "door", "name": name, "level": 1,
            "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe("door.single", args) })
}

async fn param(stack: &Stack, id: &str, name: &str, kind: &str, holder: &str) {
    let args = json!({ "definition": { "id": id, "name": name, "kind": kind,
                                       "holder": holder, "choices": [] } });
    stack.edit("parameter.define", args).await.unwrap();
    let bind = json!({ "parameter": id, "categories": [DOORS.0], "default": null });
    stack.edit("parameter.bind", bind).await.unwrap();
}

async fn set(stack: &Stack, door: &str, parameter: &str, value: &str) {
    let args = json!({ "element": door, "parameter": parameter, "value": value });
    stack.edit("element.param", args).await.unwrap();
}

/// Five doors: on level L1 a 900 mm and a 100 mm sill, on L2 a 1200 mm and two of 900 mm.
async fn five_doors() -> Stack {
    let stack = Stack::new().await;
    param(&stack, "P-level", "Level", "text", "instance").await;
    param(&stack, "P-sill", "Sill height", "length", "instance").await;
    param(&stack, "P-fire", "Fire rating", "text", "type").await;
    for (id, name, level, sill) in [
        ("D-1", "Front", "L1", "900"),
        ("D-2", "Back", "L1", "100 mm"),
        ("D-3", "Garage", "L2", "1.2 m"),
        ("D-4", "Side", "L2", "900"),
        ("D-5", "Shed", "L2", "900"),
    ] {
        stack.edit("element.create", door(id, name)).await.unwrap();
        set(&stack, id, "P-level", level).await;
        set(&stack, id, "P-sill", sill).await;
    }
    let args = json!({
        "schedule": "S-1", "name": "Door schedule", "category": DOORS.0,
        "fields": ["name", "parameter:P-level", "parameter:P-sill"],
    });
    stack.edit("schedule.create", args).await.unwrap();
    stack
}

/// What each item of a table is, in words.
fn lines(table: &ScheduleTable) -> Vec<String> {
    table
        .items
        .iter()
        .map(|item| match item {
            ItemView::Header { name, shown, .. } => {
                format!("header {name}={}", shown.clone().unwrap_or_default())
            }
            ItemView::Row {
                elements, count, ..
            } => format!("row {} x{count}", elements.join("+")),
            ItemView::Footer(footer) => format!(
                "footer {}={} n={}",
                footer.name,
                footer.shown.clone().unwrap_or_default(),
                footer.elements
            ),
        })
        .collect()
}

#[tokio::test]
async fn grouping_and_totals_through_the_commands_by_hand() {
    let stack = five_doors().await;
    let sorting = json!({ "schedule": "S-1", "sorting": [
        { "field": "parameter:P-level", "header": true, "footer": true },
        { "field": "name" },
    ] });
    stack.edit("schedule.sorting", sorting).await.unwrap();
    let totals = json!({ "schedule": "S-1", "grand": true, "totals": [
        { "field": "parameter:P-sill", "do": ["sum", "min", "max", "count"] },
    ] });
    stack.edit("schedule.totals", totals).await.unwrap();
    let table = stack.table("S-1").await;
    assert_eq!(
        lines(&table),
        [
            "header Level=L1",
            "row D-2 x1",
            "row D-1 x1",
            "footer Level=L1 n=2",
            "header Level=L2",
            "row D-3 x1",
            "row D-5 x1",
            "row D-4 x1",
            "footer Level=L2 n=3",
        ]
    );
    let total = |footer: &construct_api::cloud::schedules::FooterView, kind: &str| {
        footer
            .totals
            .iter()
            .find(|t| t.total == kind)
            .unwrap()
            .clone()
    };
    let ItemView::Footer(one) = &table.items[3] else {
        panic!("a footer");
    };
    // Level 1: 900 + 100 = 1000 mm, least 100, greatest 900, two doors.
    assert_eq!(total(one, "sum").value, Some(mm(1000)));
    assert_eq!(total(one, "sum").shown.as_deref(), Some("1000 mm"));
    assert_eq!(total(one, "min").value, Some(mm(100)));
    assert_eq!(total(one, "max").value, Some(mm(900)));
    assert_eq!(total(one, "count").shown.as_deref(), Some("2"));
    let ItemView::Footer(two) = &table.items[8] else {
        panic!("a footer");
    };
    // Level 2: 1200 + 900 + 900 = 3000 mm.
    assert_eq!(total(two, "sum").value, Some(mm(3000)));
    // Grand: five doors, 4000 mm.
    let grand = table.grand.unwrap();
    assert_eq!(grand.elements, 5);
    assert_eq!(total(&grand, "sum").value, Some(mm(4000)));
    assert_eq!(total(&grand, "max").value, Some(mm(1200)));
    // The flat rows are still one for each door.
    assert_eq!(table.rows.len(), 5);

    // A sum of text is refused in words, and nothing is written.
    let before = stack.record("S-1").await;
    let bad =
        json!({ "schedule": "S-1", "totals": [{ "field": "parameter:P-level", "do": ["sum"] }] });
    let why = stack.edit("schedule.totals", bad).await.unwrap_err();
    assert!(why.contains("Level holds text"), "{why}");
    let bad =
        json!({ "schedule": "S-1", "totals": [{ "field": "parameter:P-level", "do": ["total"] }] });
    let why = stack.edit("schedule.totals", bad).await.unwrap_err();
    assert!(why.contains("is not a total"), "{why}");
    assert_eq!(stack.record("S-1").await, before);
}

#[tokio::test]
async fn equal_rows_fold_with_a_count_when_instances_are_not_itemized() {
    let stack = five_doors().await;
    let fields = json!({ "schedule": "S-1", "fields": ["parameter:P-level", "parameter:P-sill"] });
    stack.edit("schedule.fields", fields).await.unwrap();
    let sorting = json!({ "schedule": "S-1", "sorting": [
        { "field": "parameter:P-level" }, { "field": "parameter:P-sill" },
    ] });
    stack.edit("schedule.sorting", sorting).await.unwrap();
    stack
        .edit(
            "schedule.itemize",
            json!({ "schedule": "S-1", "itemize": false }),
        )
        .await
        .unwrap();
    let table = stack.table("S-1").await;
    // L1 100, L1 900, L2 900 (D-4 and D-5 folded: a count of two), L2 1200.
    assert_eq!(
        lines(&table),
        ["row D-2 x1", "row D-1 x1", "row D-4+D-5 x2", "row D-3 x1"]
    );
    assert!(!table.schedule.itemize);
    // The flat rows still list every door.
    assert_eq!(table.rows.len(), 5);
    stack
        .edit(
            "schedule.itemize",
            json!({ "schedule": "S-1", "itemize": true }),
        )
        .await
        .unwrap();
    assert_eq!(lines(&stack.table("S-1").await).len(), 5);
}

#[tokio::test]
async fn a_calculated_field_is_exact_and_refused_in_words() {
    let stack = five_doors().await;
    let calculated = json!({ "schedule": "S-1", "calculated": [
        { "name": "Double sill", "formula": "Sill_height * 2", "kind": "length" },
    ] });
    stack.edit("schedule.calculated", calculated).await.unwrap();
    let fields =
        json!({ "schedule": "S-1", "fields": ["parameter:P-sill", "calculated:Double sill"] });
    stack.edit("schedule.fields", fields).await.unwrap();
    let table = stack.table("S-1").await;
    assert_eq!(table.rows[0].cells[1].value, Some(mm(1800)));
    assert_eq!(table.rows[1].cells[1].shown.as_deref(), Some("200 mm"));
    // Offered like any field, and totalled like one.
    assert!(
        table
            .offered
            .iter()
            .any(|f| f.field == "calculated:Double sill")
    );
    let totals = json!({ "schedule": "S-1", "grand": true, "totals": [
        { "field": "calculated:Double sill", "do": ["sum"] },
    ] });
    stack.edit("schedule.totals", totals).await.unwrap();
    // 2 * (900 + 100 + 1200 + 900 + 900) = 8000 mm.
    let grand = stack.table("S-1").await.grand.unwrap();
    assert_eq!(grand.totals[0].value, Some(mm(8000)));

    // A wrong unit, an unknown field and a formula that does not read: refused in words.
    let before = stack.record("S-1").await;
    let refuse = |formula: &str| {
        json!({ "schedule": "S-1", "calculated": [
            { "name": "Bad", "formula": formula, "kind": "length" },
        ] })
    };
    let why = stack
        .edit("schedule.calculated", refuse("Sill_height + 2"))
        .await
        .unwrap_err();
    assert!(why.contains("Bad") && why.contains("unit"), "{why}");
    let why = stack
        .edit("schedule.calculated", refuse("Sill_height * Depth"))
        .await
        .unwrap_err();
    assert!(why.contains("no field called Depth"), "{why}");
    let why = stack
        .edit("schedule.calculated", refuse("Sill_height *"))
        .await
        .unwrap_err();
    assert!(why.contains("Bad"), "{why}");
    assert_eq!(stack.record("S-1").await, before);
}

#[tokio::test]
async fn an_edit_in_a_cell_goes_back_to_the_element_in_one_undoable_claimed_step() {
    let stack = five_doors().await;
    let calculated = json!({ "schedule": "S-1", "calculated": [
        { "name": "Double sill", "formula": "Sill_height * 2", "kind": "length" },
    ] });
    stack.edit("schedule.calculated", calculated).await.unwrap();
    let fields = json!({ "schedule": "S-1", "fields": [
        "name", "parameter:P-sill", "parameter:P-fire", "calculated:Double sill",
    ] });
    stack.edit("schedule.fields", fields).await.unwrap();
    let cell = |element: &str, field: &str, value: &str| json!({ "schedule": "S-1", "element": element, "field": field, "value": value });
    stack.later();

    // The sill of D-2 goes from 100 mm to 250 mm: the element's own record has it, and the
    // schedule shows it.
    stack
        .edit("schedule.cell", cell("D-2", "parameter:P-sill", "250 mm"))
        .await
        .unwrap();
    let record = stack.record("D-2").await.unwrap();
    assert_eq!(record.props["P-sill"], mm(250));
    let table = stack.table("S-1").await;
    let row = table.rows.iter().find(|r| r.element == "D-2").unwrap();
    assert_eq!(row.cells[1].shown.as_deref(), Some("250 mm"));
    assert_eq!(
        row.cells[3].shown.as_deref(),
        Some("500 mm"),
        "the formula follows"
    );

    // One undo puts it back; one redo again.
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.record("D-2").await.unwrap().props["P-sill"], mm(100));
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.record("D-2").await.unwrap().props["P-sill"], mm(250));

    // The element is claimed like any edit: Ben, straight after, is turned away in Anna's name.
    let why = stack
        .edit_as(
            "ben",
            "schedule.cell",
            cell("D-2", "parameter:P-sill", "300 mm"),
        )
        .await
        .unwrap_err();
    assert!(why.ends_with("is working on this."), "{why}");
    stack.later();

    // The name goes back to the element too.
    stack
        .edit("schedule.cell", cell("D-1", "name", "Front entrance"))
        .await
        .unwrap();
    assert_eq!(stack.record("D-1").await.unwrap().name, "Front entrance");

    // A value that is not a length is the element's own refusal, in its words.
    let why = stack
        .edit("schedule.cell", cell("D-3", "parameter:P-sill", "tall"))
        .await
        .unwrap_err();
    assert_ne!(why, "");
    assert_eq!(stack.record("D-3").await.unwrap().props["P-sill"], mm(1200));

    // Read-only: the type's value shown in an instance schedule, and a calculated field.
    let why = stack
        .edit("schedule.cell", cell("D-3", "parameter:P-fire", "60 min"))
        .await
        .unwrap_err();
    assert!(
        why.starts_with("Fire rating cannot be edited here:")
            && why.contains("held by the element's type"),
        "{why}"
    );
    let why = stack
        .edit(
            "schedule.cell",
            cell("D-3", "calculated:Double sill", "1 m"),
        )
        .await
        .unwrap_err();
    assert!(why.contains("worked out from other fields"), "{why}");
    // A field the schedule lacks, an element it does not list, an element that is not there.
    let why = stack
        .edit("schedule.cell", cell("D-3", "parameter:P-none", "L9"))
        .await
        .unwrap_err();
    assert!(why == "Doors have no field parameter:P-none.", "{why}");
    let why = stack
        .edit("schedule.cell", cell("D-9", "name", "x"))
        .await
        .unwrap_err();
    assert_eq!(why, "That element no longer exists.");
}

#[tokio::test]
async fn a_quantity_cell_is_read_only_in_words() {
    let stack = Stack::new().await;
    let args = json!({ "schedule": "S-F", "name": "Floors", "category": construct_categories::FLOORS.0,
        "fields": ["name", "quantity:Area"] });
    stack.edit("schedule.create", args).await.unwrap();
    let why = stack
        .edit(
            "schedule.cell",
            json!({ "schedule": "S-F", "element": "F-1", "field": "quantity:Area", "value": "9 m2" }),
        )
        .await
        .unwrap_err();
    assert!(
        why.starts_with("Area cannot be edited here:")
            && why.contains("worked out from the element"),
        "{why}"
    );
}

fn sheet_listing_rows(table: &ScheduleTable) -> Vec<Vec<String>> {
    table
        .rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| cell.shown.clone().unwrap_or_default())
                .collect()
        })
        .collect()
}

#[tokio::test]
async fn the_sheet_list_and_the_view_list_are_schedules_and_follow_the_model() {
    let stack = Stack::new().await;
    for (id, number, name) in [("SH-2", "A-102", "Sections"), ("SH-1", "A-101", "Plans")] {
        stack
            .edit(
                "sheet.create",
                json!({ "sheet": id, "number": number, "name": name }),
            )
            .await
            .unwrap();
    }
    for (id, name) in [("V-1", "Ground"), ("V-2", "Roof")] {
        stack
            .edit(
                "view.create",
                json!({ "view": id, "kind": "drafting", "name": name }),
            )
            .await
            .unwrap();
    }
    let sheet_list = json!({
        "schedule": "L-S", "name": "Sheet list", "category": SHEETS.0,
        "fields": ["quantity:Number", "name", "quantity:Views placed"],
        "sorting": [{ "field": "quantity:Number" }],
        "totals": [{ "field": "quantity:Views placed", "do": ["sum"] }],
        "grand": true,
    });
    stack.edit("schedule.create", sheet_list).await.unwrap();
    let view_list = json!({
        "schedule": "L-V", "name": "View list", "category": VIEWS.0,
        "fields": ["name", "quantity:Kind", "quantity:Scale", "quantity:On sheet"],
        "sorting": [{ "field": "name" }],
    });
    stack.edit("schedule.create", view_list).await.unwrap();
    let table = stack.table("L-S").await;
    assert_eq!(
        sheet_listing_rows(&table),
        [["A-101", "Plans", "0"], ["A-102", "Sections", "0"]]
    );
    // A view stands on a sheet: both lists show it, and the total follows.
    stack
        .edit(
            "viewport.create",
            json!({ "viewport": "VP-1", "sheet": "SH-1", "view": "V-2" }),
        )
        .await
        .unwrap();
    let table = stack.table("L-S").await;
    assert_eq!(sheet_listing_rows(&table)[0], ["A-101", "Plans", "1"]);
    assert_eq!(
        table.grand.unwrap().totals[0].value.as_deref(),
        Some("whole:1")
    );
    let views = stack.table("L-V").await;
    assert_eq!(
        sheet_listing_rows(&views),
        [
            ["Ground", "drafting", "1:100", ""],
            ["Roof", "drafting", "1:100", "A-101"],
        ]
    );
    // A sheet's number is a quantity: it is worked out, not typed into a schedule.
    let why = stack
        .edit(
            "schedule.cell",
            json!({ "schedule": "L-S", "element": "SH-1", "field": "quantity:Number", "value": "A-9" }),
        )
        .await
        .unwrap_err();
    assert!(why.contains("cannot be edited here"), "{why}");
    // The sheet's name is editable: it goes back to the sheet through its own command.
    stack
        .edit(
            "schedule.cell",
            json!({ "schedule": "L-S", "element": "SH-1", "field": "name", "value": "Floor plans" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("SH-1").await.unwrap().name, "Floor plans");
    assert_eq!(
        sheet_listing_rows(&stack.table("L-S").await)[0][1],
        "Floor plans"
    );
}

#[tokio::test]
async fn a_schedule_placed_on_a_sheet_is_drawn_from_the_live_table_and_goes_with_either() {
    let stack = five_doors().await;
    stack
        .edit(
            "sheet.create",
            json!({ "sheet": "SH-1", "number": "A-101", "name": "Plans" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "schedule.place",
            json!({ "placement": "PL-1", "schedule": "S-1", "sheet": "SH-1", "x": "20 mm", "y": "400 mm" }),
        )
        .await
        .unwrap();
    let record = stack.record("PL-1").await.unwrap();
    assert_eq!(record.kind, "schedule-placement");
    assert_eq!(record.props["@placement.x"], mm(20));
    let drawn = stack.drawn("SH-1").await;
    assert_eq!(drawn.len(), 1);
    assert!(drawn[0].1.contains(&"Door schedule".to_owned()));
    assert!(drawn[0].1.contains(&"900 mm".to_owned()));
    assert!(!drawn[0].1.contains(&"250 mm".to_owned()));
    // The drawing is the table as it is now: an edit in the model is in it at once.
    set(&stack, "D-2", "P-sill", "250 mm").await;
    assert!(
        stack.drawn("SH-1").await[0]
            .1
            .contains(&"250 mm".to_owned())
    );
    // The sheet list counts it.
    let list = json!({ "schedule": "L-S", "name": "Sheet list", "category": SHEETS.0,
        "fields": ["name", "quantity:Schedules placed"] });
    stack.edit("schedule.create", list).await.unwrap();
    assert_eq!(
        sheet_listing_rows(&stack.table("L-S").await),
        [["Plans", "1"]]
    );

    // Refused in words: the same schedule twice on a sheet, a corner off the paper, no such sheet.
    let again = json!({ "placement": "PL-2", "schedule": "S-1", "sheet": "SH-1" });
    let why = stack.edit("schedule.place", again).await.unwrap_err();
    assert!(why.contains("already stands on the sheet"), "{why}");
    let off = json!({ "placement": "PL-3", "schedule": "L-S", "sheet": "SH-1", "x": "9 m" });
    let why = stack.edit("schedule.place", off).await.unwrap_err();
    assert!(why.contains("is on the paper"), "{why}");
    let none = json!({ "placement": "PL-3", "schedule": "L-S", "sheet": "SH-9" });
    assert!(stack.edit("schedule.place", none).await.is_err());
    assert!(stack.record("PL-2").await.is_none() && stack.record("PL-3").await.is_none());
    // The generic commands do not reach it.
    let why = stack
        .edit("element.delete", json!({ "element": "PL-1" }))
        .await
        .unwrap_err();
    assert_eq!(why, "A placed schedule is taken off: use schedule.unplace.");

    // Moved, in one step that undoes.
    stack.later();
    stack
        .edit(
            "schedule.move",
            json!({ "placement": "PL-1", "x": "30 mm" }),
        )
        .await
        .unwrap();
    let moved = stack.record("PL-1").await.unwrap();
    assert_eq!(moved.props["@placement.x"], mm(30));
    assert_eq!(moved.props["@placement.y"], mm(400));
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(
        stack.record("PL-1").await.unwrap().props["@placement.x"],
        mm(20)
    );

    // A schedule deleted takes its placements with it, in one step that undoes whole.
    stack.later();
    stack
        .edit("schedule.delete", json!({ "schedule": "S-1" }))
        .await
        .unwrap();
    assert!(stack.record("S-1").await.is_none() && stack.record("PL-1").await.is_none());
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.record("S-1").await.is_some() && stack.record("PL-1").await.is_some());
    // A sheet deleted takes them too.
    stack.later();
    stack
        .edit("sheet.delete", json!({ "sheet": "SH-1" }))
        .await
        .unwrap();
    assert!(stack.record("PL-1").await.is_none());
    assert!(stack.record("S-1").await.is_some());
    stack.later();
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.record("PL-1").await.is_some());
    // Taken off by itself.
    stack.later();
    stack
        .edit("schedule.unplace", json!({ "placement": "PL-1" }))
        .await
        .unwrap();
    assert!(stack.record("PL-1").await.is_none());
    assert_eq!(stack.drawn("SH-1").await.len(), 0);
}
