//! A view's detail items through the edit log, on the test store: lines, filled and masking
//! regions, detail components placed at any angle and repeated along a line, spot slopes read from
//! a roof or a floor, and a person's own line styles; every command and its refusals by name, a
//! slope worked out from the face each time it is asked, the generic element commands refused,
//! a view that holds items kept, items with no shape, undo and redo of every command, and the
//! log replayed from empty.

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
    clippy::needless_pass_by_value,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::detailing::{self, DetailListing};
use construct_api::cloud::undo::{self, CANNOT, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_params::{Area, Length, Value as Param};
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
    json!({ "view": id, "kind": "floor_plan", "name": id, "level": level })
}

fn drafting(id: &str) -> Value {
    json!({ "view": id, "kind": "drafting", "name": id })
}

fn p(x: &str, y: &str) -> Value {
    json!({ "x": x, "y": y })
}

fn straight(start: Value, end: Value) -> Value {
    json!({ "kind": "straight", "start": start, "end": end })
}

/// The L: 4 m by 2 m with a 2 m by 2 m block on one end (12 m2).
fn ell() -> Value {
    json!([
        p("0", "0"),
        p("4 m", "0"),
        p("4 m", "2 m"),
        p("2 m", "2 m"),
        p("2 m", "4 m"),
        p("0", "4 m")
    ])
}

/// Ground at 0 and First at 3 m, the plan `V-1` on First and the drafting view `V-d`.
async fn with_views() -> Stack {
    let stack = Stack::new().await;
    for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
        stack
            .edit("level.create", level(id, name, elevation))
            .await
            .unwrap();
    }
    stack.edit("view.create", plan("V-1", "L-1")).await.unwrap();
    stack.edit("view.create", plan("V-2", "L-g")).await.unwrap();
    stack.edit("view.create", drafting("V-d")).await.unwrap();
    stack
}

async fn listing(stack: &Stack, view: &str) -> DetailListing {
    let mut snapshot = stack.store.snapshot().await.unwrap();
    detailing::listing(&mut snapshot, &i("p"), &i(DOCUMENT), &i(view))
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

fn stored(length: Length) -> String {
    Param::Length(length).stored()
}

// ---------------------------------------------------------------------------------------------
// Roofs and floors to read slopes from

const STEEL: &str = "roof-steel-rafter-150";
const SCREED: &str = "floor-concrete-150-screed-50";

fn corner(id: &str, x: i64, y: i64) -> Value {
    json!({ "id": id, "at": [x.to_string(), y.to_string()] })
}

fn rectangle(x: i64, y: i64) -> Value {
    json!({ "outer": [corner("r0", 0, 0), corner("r1", x, 0), corner("r2", x, y), corner("r3", 0, y)] })
}

/// An 8 m by 6 m roof `R-1` of `form` at `pitch`, on First.
async fn with_roof(stack: &Stack, form: &str, pitch: Option<&str>) {
    stack.edit("roof.defaults", json!({})).await.unwrap();
    let mut args = json!({ "element": "R-1", "type": STEEL, "level": "L-1",
                           "footprint": rectangle(8000, 6000), "form": form });
    if let Some(pitch) = pitch {
        args["pitch"] = json!(pitch);
    }
    stack.edit("roof.create", args).await.unwrap();
}

async fn with_floor(stack: &Stack) {
    stack.edit("floor.defaults", json!({})).await.unwrap();
    let args = json!({ "element": "F-1", "type": SCREED, "level": "L-1",
                       "boundary": rectangle(8000, 6000) });
    stack.edit("floor.create", args).await.unwrap();
}

fn slope(id: &str, view: &str, face: &str, x: &str, y: &str) -> Value {
    json!({ "slope": id, "view": view, "face": face, "at": p(x, y) })
}

// ---------------------------------------------------------------------------------------------
// Lines, regions, components and repeats

#[tokio::test]
async fn a_detail_line_belongs_to_one_view_and_is_never_in_another() {
    let stack = with_views().await;
    let line = json!({ "line": "DL-1", "view": "V-1",
                       "curve": straight(p("0", "0"), p("3 m", "1.5 m")) });
    stack.edit("detail.line.create", line).await.unwrap();
    let record = stack.record("DL-1").await;
    assert_eq!(record.kind, "detail.line");
    assert_eq!(
        record.props.get("@detail.line.style").unwrap(),
        "builtin:standard"
    );
    let here = listing(&stack, "V-1").await;
    assert_eq!(here.lines.len(), 1);
    assert_eq!(here.lines[0].kind, "straight");
    assert_eq!(here.lines[0].points[1].x, stored(mm(3000)));
    assert_eq!(here.lines[0].points[1].y, stored(mm(1500)));
    // Nothing of it in another plan, nor in the drafting view.
    for other in ["V-2", "V-d"] {
        let there = listing(&stack, other).await;
        assert_eq!(there.lines.len(), 0, "{other}");
    }
    // An arc, edited from a straight line's points.
    let arc = json!({ "kind": "arc", "centre": p("0", "0"), "start": p("1 m", "0"),
                      "end": p("0", "1 m") });
    stack
        .edit(
            "detail.line.points",
            json!({ "line": "DL-1", "curve": arc }),
        )
        .await
        .unwrap();
    let arced = listing(&stack, "V-1").await;
    assert_eq!(arced.lines[0].kind, "arc");
    assert_eq!(arced.lines[0].points.len(), 3);
    // Setting its style to a built-in one, and then to one of the document's own.
    stack
        .edit(
            "detail.line.style",
            json!({ "line": "DL-1", "style": "builtin:centre" }),
        )
        .await
        .unwrap();
    assert_eq!(
        listing(&stack, "V-1").await.lines[0].style,
        "builtin:centre"
    );
    assert_eq!(
        stack
            .edit(
                "detail.line.style",
                json!({ "line": "DL-1", "style": "grout" })
            )
            .await
            .unwrap_err(),
        "There is no line style \"grout\"."
    );
}

#[tokio::test]
async fn a_filled_region_reads_its_area_and_a_masking_region_has_no_pattern() {
    let stack = with_views().await;
    let filled = json!({ "region": "FR-1", "view": "V-1", "kind": "filled", "outline": ell(),
                         "pattern": "timber", "colour": "#102030" });
    stack.edit("detail.region.create", filled).await.unwrap();
    let region = &listing(&stack, "V-1").await.regions[0];
    assert_eq!(region.kind, "filled");
    assert_eq!(region.pattern.as_deref(), Some("timber"));
    assert_eq!(region.colour.as_deref(), Some("#102030"));
    assert_eq!(region.boundary.as_deref(), Some("builtin:standard"));
    // 4000 x 2000 + 2000 x 2000 = 12,000,000 mm2, exactly.
    assert_eq!(
        region.area,
        Param::Area(Area::from_mm2(12_000_000)).stored()
    );
    assert!(region.area_exact);
    assert!(region.area_text.contains("12"), "{}", region.area_text);
    // A masking region: opaque, no pattern or colour; its boundary may be none.
    let masking = json!({ "region": "MR-1", "view": "V-1", "kind": "masking", "boundary": null,
                          "outline": [p("0", "0"), p("1 m", "0"), p("1 m", "1 m"), p("0", "1 m")] });
    stack.edit("detail.region.create", masking).await.unwrap();
    let both = listing(&stack, "V-1").await;
    let mask = both.regions.iter().find(|r| r.id == "MR-1").unwrap();
    assert_eq!(
        (
            mask.kind.as_str(),
            mask.pattern.clone(),
            mask.boundary.clone()
        ),
        ("masking", None, None)
    );
    assert_eq!(mask.area, Param::Area(Area::from_mm2(1_000_000)).stored());
    for (args, why) in [
        (
            json!({ "region": "MR-2", "view": "V-1", "kind": "masking", "pattern": "timber",
                    "outline": ell() }),
            "A masking region is opaque and has no pattern or colour.",
        ),
        (
            json!({ "region": "FR-2", "view": "V-1", "kind": "filled", "outline": ell(),
                    "pattern": "sparkle", "colour": "#000000" }),
            "There is no fill pattern \"sparkle\".",
        ),
        (
            json!({ "region": "FR-2", "view": "V-1", "kind": "filled", "outline": ell(),
                    "pattern": "timber" }),
            "This command needs a colour.",
        ),
        (
            json!({ "region": "FR-2", "view": "V-1", "kind": "filled", "outline": ell(),
                    "pattern": "timber", "colour": "#10203G" }),
            "\"#10203G\" is not a colour written #rrggbb.",
        ),
    ] {
        assert_eq!(
            stack.edit("detail.region.create", args).await.unwrap_err(),
            why
        );
    }
    // A bow tie crosses itself, and a few corners are not a region.
    let bow = json!([p("0", "0"), p("1 m", "1 m"), p("1 m", "0"), p("0", "1 m")]);
    let args = json!({ "region": "FR-2", "view": "V-1", "kind": "masking", "outline": bow });
    assert_eq!(
        stack.edit("detail.region.create", args).await.unwrap_err(),
        "The outline crosses itself: edges 1 and 3 cross, touch or overlap."
    );
    let two = json!([p("0", "0"), p("1 m", "0")]);
    let args = json!({ "region": "FR-2", "view": "V-1", "kind": "masking", "outline": two });
    assert_eq!(
        stack.edit("detail.region.create", args).await.unwrap_err(),
        "An outline needs at least three corners; this has 2."
    );
    // Edits: the corners, the boundary and the fill.
    stack
        .edit(
            "detail.region.points",
            json!({ "region": "MR-1", "outline": [p("0", "0"), p("2 m", "0"), p("0", "1 m")] }),
        )
        .await
        .unwrap();
    let moved = listing(&stack, "V-1").await;
    assert_eq!(
        moved.regions.iter().find(|r| r.id == "MR-1").unwrap().area,
        Param::Area(Area::from_mm2(1_000_000)).stored()
    );
    stack
        .edit(
            "detail.region.style",
            json!({ "region": "FR-1", "boundary": "builtin:bold" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "detail.region.fill",
            json!({ "region": "FR-1", "pattern": "concrete", "colour": "#ff0000" }),
        )
        .await
        .unwrap();
    let edited = listing(&stack, "V-1").await;
    let fr = edited.regions.iter().find(|r| r.id == "FR-1").unwrap();
    assert_eq!(
        (
            fr.pattern.as_deref(),
            fr.colour.as_deref(),
            fr.boundary.as_deref()
        ),
        (Some("concrete"), Some("#ff0000"), Some("builtin:bold"))
    );
    assert_eq!(
        stack
            .edit(
                "detail.region.fill",
                json!({ "region": "MR-1", "pattern": "concrete", "colour": "#ff0000" })
            )
            .await
            .unwrap_err(),
        "A masking region is opaque and has no pattern."
    );
}

#[tokio::test]
async fn a_component_is_placed_and_turned_by_any_angle() {
    let stack = with_views().await;
    let place = json!({ "component": "C-1", "view": "V-1", "recipe": "timber-section-90x45",
                        "position": p("1 m", "2 m"), "rotation": "37.5°" });
    stack.edit("detail.component.create", place).await.unwrap();
    let record = stack.record("C-1").await;
    assert_eq!(record.kind, "detail.component");
    assert_eq!(
        record.props.get("@detail.component.rotation").unwrap(),
        "angle:337500000"
    );
    let shown = &listing(&stack, "V-1").await.components[0];
    assert_eq!(shown.recipe_name, "Timber section 90 x 45");
    assert_eq!(shown.position.x, stored(mm(1000)));
    // A turn of 360 and a half degrees is a turn of a half degree.
    stack
        .edit(
            "detail.component.move",
            json!({ "component": "C-1", "rotation": "360.5°" }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack
            .record("C-1")
            .await
            .props
            .get("@detail.component.rotation")
            .unwrap(),
        "angle:4500000"
    );
    stack
        .edit(
            "detail.component.move",
            json!({ "component": "C-1", "position": p("0", "0") }),
        )
        .await
        .unwrap();
    assert_eq!(
        listing(&stack, "V-1").await.components[0].position.x,
        stored(mm(0))
    );
    for (cmd, args, why) in [
        (
            "detail.component.create",
            json!({ "component": "C-2", "view": "V-1", "recipe": "sofa", "position": p("0", "0") }),
            "There is no detail component \"sofa\" in the library.",
        ),
        (
            "detail.component.create",
            json!({ "component": "C-2", "view": "V-9", "recipe": "blocking-cross-90x45",
                    "position": p("0", "0") }),
            "There is no view V-9 in this model.",
        ),
        (
            "detail.component.create",
            json!({ "component": "C-1", "view": "V-1", "recipe": "blocking-cross-90x45",
                    "position": p("0", "0") }),
            "That element already exists.",
        ),
        (
            "detail.component.move",
            json!({ "component": "C-1" }),
            "This command needs a position or a rotation.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), why, "{cmd}");
    }
}

#[tokio::test]
async fn a_repeating_component_along_a_three_metre_line_makes_six() {
    let stack = with_views().await;
    let repeat = json!({ "repeat": "RP-1", "view": "V-1", "recipe": "blocking-cross-90x45",
                         "from": p("0", "0"), "to": p("3 m", "0"), "spacing": "600 mm" });
    stack.edit("detail.repeat.create", repeat).await.unwrap();
    let shown = &listing(&stack, "V-1").await.repeats[0];
    assert_eq!(shown.count, 6);
    assert_eq!(shown.positions.len(), 6);
    assert_eq!(shown.positions[5].x, stored(mm(3000)));
    assert_eq!(shown.spacing, stored(mm(600)));
    // A closer spacing and a shorter line change the count; nothing about copies is stored.
    stack
        .edit(
            "detail.repeat.spacing",
            json!({ "repeat": "RP-1", "spacing": "500 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(listing(&stack, "V-1").await.repeats[0].count, 7);
    stack
        .edit(
            "detail.repeat.line",
            json!({ "repeat": "RP-1", "from": p("0", "0"), "to": p("0", "2 m") }),
        )
        .await
        .unwrap();
    assert_eq!(listing(&stack, "V-1").await.repeats[0].count, 5);
    assert_eq!(
        stack
            .record("RP-1")
            .await
            .props
            .keys()
            .filter(|k| k.contains("count") || k.contains("positions"))
            .count(),
        0
    );
    for (cmd, args, why) in [
        (
            "detail.repeat.spacing",
            json!({ "repeat": "RP-1", "spacing": "0" }),
            "A repeating component's spacing must be more than zero.",
        ),
        (
            "detail.repeat.line",
            json!({ "repeat": "RP-1", "from": p("1 m", "1 m"), "to": p("1 m", "1 m") }),
            "A line needs two different ends.",
        ),
        (
            "detail.repeat.create",
            json!({ "repeat": "RP-2", "view": "V-1", "recipe": "blocking-cross-90x45",
                    "from": p("0", "0"), "to": p("100 m", "0"), "spacing": "1 mm" }),
            "That would place the component 100001 times; at most 2000 at once.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), why, "{cmd}");
    }
}

#[tokio::test]
async fn a_line_that_is_not_a_line_is_refused_by_name() {
    let stack = with_views().await;
    for (curve, why) in [
        (
            straight(p("1 m", "1 m"), p("1 m", "1 m")),
            "A line needs two different ends.".to_owned(),
        ),
        (
            json!({ "kind": "straight", "centre": p("0", "0"), "start": p("0", "0"), "end": p("1 m", "0") }),
            "A straight line takes a start and an end, and no centre.".to_owned(),
        ),
        (
            json!({ "kind": "spiral" }),
            "\"spiral\" is not a curve: straight or arc.".to_owned(),
        ),
        (
            json!({ "kind": "arc", "start": p("0", "0"), "end": p("1 m", "0") }),
            "This command needs a centre.".to_owned(),
        ),
    ] {
        let args = json!({ "line": "DL-9", "view": "V-1", "curve": curve });
        assert_eq!(
            stack.edit("detail.line.create", args).await.unwrap_err(),
            why
        );
    }
    let off = json!({ "kind": "arc", "centre": p("0", "0"), "start": p("1 m", "0"),
                      "end": p("0", "1010 mm") });
    let refused = stack
        .edit(
            "detail.line.create",
            json!({ "line": "DL-9", "view": "V-1", "curve": off }),
        )
        .await
        .unwrap_err();
    assert!(refused.starts_with("Not an arc: "), "{refused}");
    assert!(!stack.exists("DL-9").await);
    let missing = json!({ "line": "DL-9", "view": "V-9",
                          "curve": straight(p("0", "0"), p("1 m", "0")) });
    assert_eq!(
        stack.edit("detail.line.create", missing).await.unwrap_err(),
        "There is no view V-9 in this model."
    );
}

// ---------------------------------------------------------------------------------------------
// A person's own line styles

#[tokio::test]
async fn an_own_line_style_is_followed_by_every_line_that_uses_it() {
    let stack = with_views().await;
    let style = json!({ "style": "S-grout", "name": "Grout joint", "pen": 3, "line_pattern": 2,
                        "colour": "#c81e1e" });
    stack.edit("detail.style.create", style).await.unwrap();
    for (id, view) in [("DL-1", "V-1"), ("DL-2", "V-2")] {
        let line = json!({ "line": id, "view": view, "style": "S-grout",
                           "curve": straight(p("0", "0"), p("1 m", "0")) });
        stack.edit("detail.line.create", line).await.unwrap();
    }
    let region = json!({ "region": "FR-1", "view": "V-1", "kind": "filled", "outline": ell(),
                         "pattern": "timber", "colour": "#000000", "boundary": "S-grout" });
    stack.edit("detail.region.create", region).await.unwrap();
    let before = listing(&stack, "V-1").await;
    let own = before.styles.iter().find(|s| s.style == "S-grout").unwrap();
    assert_eq!(
        (own.name.as_str(), own.built_in, own.pen),
        ("Grout joint", false, 3)
    );
    assert_eq!(before.styles.iter().filter(|s| s.built_in).count(), 6);
    // The style changes once; the lines are not written.
    let lines_before = (stack.record("DL-1").await, stack.record("DL-2").await);
    stack
        .edit(
            "detail.style.set",
            json!({ "style": "S-grout", "pen": 6, "colour": "#00ff00" }),
        )
        .await
        .unwrap();
    assert_eq!(
        (stack.record("DL-1").await, stack.record("DL-2").await),
        lines_before
    );
    let after = listing(&stack, "V-2").await;
    let own = after.styles.iter().find(|s| s.style == "S-grout").unwrap();
    assert_eq!((own.pen, own.colour.as_str()), (6, "#00ff00"));
    // Refusals by name.
    for (cmd, args, why) in [
        (
            "detail.style.create",
            json!({ "style": "S-2", "name": "grout JOINT", "pen": 3, "line_pattern": 2, "colour": "#000000" }),
            "There is already a line style named \"grout JOINT\".",
        ),
        (
            "detail.style.create",
            json!({ "style": "S-2", "name": "Fine", "pen": 3, "line_pattern": 2, "colour": "#000000" }),
            "\"Fine\" is the name of a built-in style.",
        ),
        (
            "detail.style.create",
            json!({ "style": "S-2", "name": "Wide", "pen": 99, "line_pattern": 2, "colour": "#000000" }),
            "There is no model pen 99.",
        ),
        (
            "detail.style.create",
            json!({ "style": "S-2", "name": "Odd", "pen": 3, "line_pattern": 77, "colour": "#000000" }),
            "There is no line pattern 77.",
        ),
        (
            "detail.style.delete",
            json!({ "style": "S-grout" }),
            "The style is used by 3 item(s), such as DL-1: give them another style first.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), why, "{cmd}");
    }
    // Once nothing uses it, it can go; and a line cannot name a style that went.
    for (cmd, args) in [
        ("detail.line.delete", json!({ "line": "DL-1" })),
        ("detail.line.delete", json!({ "line": "DL-2" })),
        ("detail.region.delete", json!({ "region": "FR-1" })),
        ("detail.style.delete", json!({ "style": "S-grout" })),
    ] {
        stack.edit(cmd, args).await.unwrap();
    }
    assert!(!stack.exists("S-grout").await);
}

// ---------------------------------------------------------------------------------------------
// Spot slopes

#[tokio::test]
async fn a_spot_slope_on_a_quarter_pitch_roof_reads_one_in_four() {
    let stack = with_views().await;
    with_roof(&stack, "mono", Some("1:4")).await;
    stack
        .edit(
            "detail.slope.create",
            slope("SS-1", "V-1", "R-1", "4 m", "3 m"),
        )
        .await
        .unwrap();
    let record = stack.record("SS-1").await;
    // Nothing worked out is stored.
    let keys: Vec<&str> = record.props.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "@category",
            "@detail.slope.face",
            "@detail.slope.point.x",
            "@detail.slope.point.y",
            "@detail.view"
        ]
    );
    let shown = &listing(&stack, "V-1").await.slopes[0];
    let reading = shown.reading.as_ref().unwrap();
    assert_eq!(reading.rise_over_run.as_deref(), Some("1/4"));
    assert_eq!(reading.squared, "1/16");
    // atan(0.25) = 14.0362434679 degrees: 126,326,191.2 angle units.
    let units: i64 = reading
        .angle
        .strip_prefix("angle:")
        .unwrap()
        .parse()
        .unwrap();
    assert!((units - 126_326_191).abs() <= 2, "{units}");
    assert_eq!(reading.text, "14.04 degrees, rise over run 1/4");
    assert!(shown.warnings.is_empty());
    // The pitch changes: the spot slope, untouched, now reads the new slope.
    let rows = stack.rows("piece").await;
    stack
        .edit("roof.pitch", json!({ "element": "R-1", "pitch": "1:2" }))
        .await
        .unwrap();
    let steeper = &listing(&stack, "V-1").await.slopes[0];
    assert_eq!(
        steeper.reading.as_ref().unwrap().rise_over_run.as_deref(),
        Some("1/2")
    );
    assert_eq!(stack.record("SS-1").await, record);
    assert_ne!(stack.rows("piece").await, rows);
}

#[tokio::test]
async fn a_spot_slope_on_a_thirty_degree_roof_reads_thirty_degrees_to_the_grid() {
    let stack = with_views().await;
    with_roof(&stack, "mono", Some("30°")).await;
    stack
        .edit(
            "detail.slope.create",
            slope("SS-1", "V-1", "R-1", "4 m", "3 m"),
        )
        .await
        .unwrap();
    let reading = listing(&stack, "V-1").await.slopes[0]
        .reading
        .clone()
        .unwrap();
    // The roof's face is on the grid: its rise over a 6600 mm run is held to the nearest step, so
    // the angle is 30 degrees to within a few angle units (1/9,000,000 degree), not exactly.
    let units: i64 = reading
        .angle
        .strip_prefix("angle:")
        .unwrap()
        .parse()
        .unwrap();
    assert!((units - 270_000_000).abs() <= 20, "{units}");
    assert_eq!(reading.text, "30.00 degrees, rise over run 0.5774");
}

#[tokio::test]
async fn a_spot_slope_is_refused_by_name_on_a_flat_or_missing_face() {
    let stack = with_views().await;
    with_roof(&stack, "flat", None).await;
    with_floor(&stack).await;
    for (face, why) in [
        ("R-1", "The face is flat: it has no slope to read."),
        ("F-1", "The face is flat: it has no slope to read."),
        ("R-9", "There is no floor or roof R-9 in this model."),
    ] {
        assert_eq!(
            stack
                .edit(
                    "detail.slope.create",
                    slope("SS-1", "V-1", face, "4 m", "3 m")
                )
                .await
                .unwrap_err(),
            why,
            "{face}"
        );
    }
    assert!(!stack.exists("SS-1").await);
    // A point that is not over the face, a face that is not a floor or roof, and a view that is no
    // plan are refused too.
    assert_eq!(
        stack
            .edit(
                "detail.slope.create",
                slope("SS-1", "V-1", "F-1", "20 m", "3 m")
            )
            .await
            .unwrap_err(),
        "The point is not over the floor F-1."
    );
    stack
        .edit("level.create", level("L-x", "X", "9 m"))
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit(
                "detail.slope.create",
                slope("SS-1", "V-1", "L-x", "1 m", "1 m")
            )
            .await
            .unwrap_err(),
        "L-x is a level: a spot slope reads a floor or a roof."
    );
    let refused = stack
        .edit(
            "detail.slope.create",
            slope("SS-1", "V-d", "R-1", "1 m", "1 m"),
        )
        .await
        .unwrap_err();
    assert!(refused.contains("is a drafting"), "{refused}");
    assert!(refused.contains("made in a plan"), "{refused}");
}

#[tokio::test]
async fn a_slope_whose_roof_went_flat_or_away_is_a_warning_by_name_not_a_wrong_number() {
    let stack = with_views().await;
    with_roof(&stack, "mono", Some("1:4")).await;
    stack
        .edit(
            "detail.slope.create",
            slope("SS-1", "V-1", "R-1", "4 m", "3 m"),
        )
        .await
        .unwrap();
    // The roof goes flat.
    stack
        .edit("roof.form", json!({ "element": "R-1", "form": "flat" }))
        .await
        .unwrap();
    let flat = &listing(&stack, "V-1").await.slopes[0];
    assert!(flat.reading.is_none());
    let [warning] = flat.warnings.as_slice() else {
        panic!("one warning");
    };
    assert_eq!(warning.kind, "detail.slope.flat");
    assert_eq!(warning.elements, ["SS-1", "R-1"]);
    assert_eq!(warning.identity, "detail.slope.flat|SS-1|R-1");
    assert_eq!(warning.words, "The face is flat: it has no slope to read.");
    // The roof is deleted.
    stack
        .edit("roof.delete", json!({ "element": "R-1" }))
        .await
        .unwrap();
    let gone = &listing(&stack, "V-1").await.slopes[0];
    assert_eq!(gone.warnings[0].kind, "detail.slope.missing");
    assert_eq!(
        gone.warnings[0].words,
        "There is no floor or roof R-1 in this model."
    );
    // An undo puts the roof back, and the number with it.
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(
        listing(&stack, "V-1").await.slopes[0].warnings[0].kind,
        "detail.slope.flat"
    );
    stack.edit("edit.undo", json!({})).await.unwrap();
    let back = &listing(&stack, "V-1").await.slopes[0];
    assert_eq!(
        back.reading.as_ref().unwrap().rise_over_run.as_deref(),
        Some("1/4")
    );
}

#[tokio::test]
async fn a_spot_slope_moves_only_to_a_point_that_reads() {
    let stack = with_views().await;
    with_roof(&stack, "mono", Some("1:4")).await;
    stack
        .edit(
            "detail.slope.create",
            slope("SS-1", "V-1", "R-1", "4 m", "3 m"),
        )
        .await
        .unwrap();
    let refused = stack
        .edit(
            "detail.slope.move",
            json!({ "slope": "SS-1", "at": p("30 m", "3 m") }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "The point is not over the roof R-1.");
    stack
        .edit(
            "detail.slope.move",
            json!({ "slope": "SS-1", "at": p("1 m", "1 m") }),
        )
        .await
        .unwrap();
    assert_eq!(
        listing(&stack, "V-1").await.slopes[0].at.x,
        stored(mm(1000))
    );
}

// ---------------------------------------------------------------------------------------------
// The one edit log

#[tokio::test]
async fn the_generic_commands_do_not_go_behind_the_detail_commands() {
    let stack = with_views().await;
    let line = json!({ "line": "DL-1", "view": "V-1",
                       "curve": straight(p("0", "0"), p("1 m", "0")) });
    stack.edit("detail.line.create", line).await.unwrap();
    for kind in [
        "detail.line",
        "detail.region",
        "detail.slope",
        "detail.linestyle",
    ] {
        let by_kind = json!({ "element": "X-1", "kind": kind, "level": 0,
                              "placement": { "x": 0, "y": 0, "z": 0 },
                              "recipe": recipe(kind, json!({})) });
        assert_eq!(
            stack.edit("element.create", by_kind).await.unwrap_err(),
            "A detail item is made: use the detail commands. Detail items are made and changed \
             by the detail commands.",
            "{kind}"
        );
    }
    for (cmd, args, want) in [
        (
            "element.delete",
            json!({ "element": "DL-1" }),
            "A detail item is deleted: use detail.line.delete",
        ),
        (
            "element.move",
            json!({ "element": "DL-1", "placement": { "x": 1, "y": 1, "z": 0 } }),
            "A detail item is changed by the detail commands",
        ),
        (
            "element.set",
            json!({ "element": "DL-1", "name": "x" }),
            "A detail item is changed by the detail commands",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert!(refused.starts_with(want), "{cmd}: {refused}");
    }
    assert!(stack.exists("DL-1").await);
    // The wrong delete for the kind says there is no such item.
    assert_eq!(
        stack
            .edit("detail.region.delete", json!({ "region": "DL-1" }))
            .await
            .unwrap_err(),
        "There is no detail item DL-1 in this model."
    );
    // The view holds an item: deleting it is refused until the item is gone.
    assert_eq!(
        stack
            .edit("view.delete", json!({ "view": "V-1" }))
            .await
            .unwrap_err(),
        "The view holds 1 detail item(s), such as DL-1; delete them with the detail commands first."
    );
    stack
        .edit("detail.line.delete", json!({ "line": "DL-1" }))
        .await
        .unwrap();
    stack
        .edit("view.delete", json!({ "view": "V-1" }))
        .await
        .unwrap();
}

#[tokio::test]
async fn an_undo_may_not_leave_an_item_in_a_view_that_is_not_there() {
    let stack = Stack::new().await;
    stack
        .edit("level.create", level("L-g", "Ground", "0"))
        .await
        .unwrap();
    stack.edit("view.create", plan("V-1", "L-g")).await.unwrap();
    stack.later();
    // Ben puts a line in Anna's new plan; her undo of making it would leave it on nothing.
    let line = json!({ "line": "DL-1", "view": "V-1",
                       "curve": straight(p("0", "0"), p("1 m", "0")) });
    stack
        .edit_as("ben", "detail.line.create", line)
        .await
        .unwrap();
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert!(refused.starts_with(CANNOT), "{refused}");
    assert!(
        refused.contains("the detail item DL-1 would be left in a view that is not there"),
        "{refused}"
    );
    assert!(stack.exists("V-1").await);
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

/// Every command, in an order that gives each something to work on.
fn every_command(roof_there: bool) -> Vec<(&'static str, Value)> {
    let mut all = vec![
        (
            "detail.style.create",
            json!({ "style": "S-1", "name": "Grout", "pen": 3, "line_pattern": 2, "colour": "#112233" }),
        ),
        (
            "detail.style.set",
            json!({ "style": "S-1", "name": "Grout joint", "pen": 4 }),
        ),
        (
            "detail.line.create",
            json!({ "line": "DL-1", "view": "V-1", "style": "S-1",
                    "curve": straight(p("0", "0"), p("2 m", "0")) }),
        ),
        (
            "detail.line.points",
            json!({ "line": "DL-1", "curve": straight(p("0", "0"), p("2 m", "1 m")) }),
        ),
        (
            "detail.line.style",
            json!({ "line": "DL-1", "style": "builtin:hidden" }),
        ),
        (
            "detail.region.create",
            json!({ "region": "FR-1", "view": "V-1", "kind": "filled", "outline": ell(),
                    "pattern": "timber", "colour": "#000000" }),
        ),
        (
            "detail.region.points",
            json!({ "region": "FR-1", "outline": [p("0", "0"), p("1 m", "0"), p("0", "1 m")] }),
        ),
        (
            "detail.region.style",
            json!({ "region": "FR-1", "boundary": "S-1" }),
        ),
        (
            "detail.region.fill",
            json!({ "region": "FR-1", "pattern": "metal", "colour": "#0000ff" }),
        ),
        (
            "detail.component.create",
            json!({ "component": "C-1", "view": "V-1", "recipe": "timber-section-90x45",
                    "position": p("1 m", "1 m") }),
        ),
        (
            "detail.component.move",
            json!({ "component": "C-1", "rotation": "15°" }),
        ),
        (
            "detail.repeat.create",
            json!({ "repeat": "RP-1", "view": "V-1", "recipe": "blocking-cross-90x45",
                    "from": p("0", "0"), "to": p("3 m", "0"), "spacing": "600 mm" }),
        ),
        (
            "detail.repeat.line",
            json!({ "repeat": "RP-1", "from": p("0", "0"), "to": p("0", "3 m") }),
        ),
        (
            "detail.repeat.spacing",
            json!({ "repeat": "RP-1", "spacing": "1 m", "rotation": "90°" }),
        ),
    ];
    if roof_there {
        all.push((
            "detail.slope.create",
            slope("SS-1", "V-1", "R-1", "4 m", "3 m"),
        ));
        all.push((
            "detail.slope.move",
            json!({ "slope": "SS-1", "at": p("2 m", "2 m") }),
        ));
        all.push(("detail.slope.delete", json!({ "slope": "SS-1" })));
    }
    all.extend([
        ("detail.repeat.delete", json!({ "repeat": "RP-1" })),
        ("detail.component.delete", json!({ "component": "C-1" })),
        ("detail.region.delete", json!({ "region": "FR-1" })),
        ("detail.line.delete", json!({ "line": "DL-1" })),
        ("detail.style.delete", json!({ "style": "S-1" })),
    ]);
    all
}

#[tokio::test]
async fn every_detail_command_is_undone_and_redone() {
    let stack = with_views().await;
    with_roof(&stack, "mono", Some("1:4")).await;
    let all = every_command(true);
    let names: Vec<&str> = all.iter().map(|(cmd, _)| *cmd).collect();
    for cmd in detailing::COMMANDS {
        assert!(names.contains(&cmd), "{cmd} is exercised here");
    }
    for (cmd, args) in all {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn another_persons_claim_turns_a_change_back() {
    let stack = with_views().await;
    let line = json!({ "line": "DL-1", "view": "V-1",
                       "curve": straight(p("0", "0"), p("1 m", "0")) });
    stack.edit("detail.line.create", line).await.unwrap();
    let change = json!({ "line": "DL-1", "style": "builtin:bold" });
    assert_eq!(
        stack
            .edit_as("ben", "detail.line.style", change.clone())
            .await
            .unwrap_err(),
        "anna is working on this."
    );
    stack.later();
    stack
        .edit_as("ben", "detail.line.style", change)
        .await
        .unwrap();
}

#[tokio::test]
async fn detail_items_have_no_shape_and_add_no_derived_row() {
    let stack = with_views().await;
    stack
        .edit("element.create", box_wall("W-1", 0, 0, "x"))
        .await
        .unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    let before = stack.derived().await;
    let checked = stack.regen.verify(&i("p")).await;
    for (cmd, args) in every_command(false).into_iter().take(12) {
        stack.edit(cmd, args).await.unwrap();
    }
    stack.regen.catch_up(&i("p")).await.unwrap();
    let after = stack.derived().await;
    // The only derived row an item adds is its kind row, which every record has.
    let added: Vec<&String> = after
        .keys()
        .filter(|key| !before.contains_key(*key))
        .collect();
    assert!(!added.is_empty());
    assert!(added.iter().all(|key| key.contains("kind")), "{added:?}");
    assert_eq!(stack.regen.verify(&i("p")).await, checked);
}

fn box_wall(id: &str, x: i64, y: i64, axis: &str) -> Value {
    let args = json!({ "length": 4000, "thickness": 200, "height": 2700, "axis": axis });
    json!({ "element": id, "kind": "wall", "level": 1,
            "placement": { "x": x, "y": y, "z": 0 }, "recipe": recipe("wall.straight", args) })
}

fn recipe(op: &str, args: Value) -> Recipe {
    Recipe::construct(op, serde_json::from_value(args).unwrap())
}

#[tokio::test]
async fn every_detail_kind_is_one_the_regenerator_leaves_without_a_shape() {
    // The regenerator tests the prefix; this holds it to construct-detailing's own list.
    let library = construct_detailing::Library::built_in().unwrap();
    let stack = with_views().await;
    with_roof(&stack, "mono", Some("1:4")).await;
    for (cmd, args) in every_command(true) {
        if cmd.ends_with("delete") {
            break;
        }
        stack.edit(cmd, args).await.unwrap();
    }
    let mut seen = std::collections::BTreeSet::new();
    for id in ["S-1", "DL-1", "FR-1", "C-1", "RP-1", "SS-1"] {
        let record = stack.record(id).await;
        assert!(construct_detailing::is_detail_kind(&record.kind), "{id}");
        assert!(!construct_regen::parameters::has_shape(&record), "{id}");
        seen.insert(record.kind);
    }
    let masking = json!({ "region": "MR-1", "view": "V-1", "kind": "masking",
                          "outline": [p("0", "0"), p("1 m", "0"), p("1 m", "1 m")] });
    stack.edit("detail.region.create", masking).await.unwrap();
    seen.insert(stack.record("MR-1").await.kind);
    assert_eq!(seen.len(), 7);
    assert!(library.get("timber-section-90x45").is_some());
    assert!(!construct_regen::parameters::has_shape(
        &stack.record("MR-1").await
    ));
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = with_views().await;
    with_roof(&stack, "mono", Some("1:4")).await;
    let mut all = every_command(true);
    all.extend([
        ("edit.undo", json!({})),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
        ("view.delete", json!({ "view": "V-1" })),
        ("detail.line.delete", json!({ "line": "NOPE" })),
    ]);
    for (cmd, args) in all {
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
    assert!(
        entries
            .iter()
            .any(|entry| entry.cmd == "detail.repeat.spacing")
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
}
