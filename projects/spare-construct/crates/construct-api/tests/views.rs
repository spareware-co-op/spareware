//! Views through the edit log, on the test store: the view commands and their refusals, the
//! generic element commands refused for a view, a level a view's range is tied to kept, views
//! with no shape, undo and redo of every view command, and the log replayed from empty.

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
use construct_api::cloud::views::options;
use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::{CategoryId, CategoryTable};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_params::Length;
use construct_phases::{OptionRef, PhaseError};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_styles::{Colour, Edge, LineStyles, ObjectStyles};
use construct_views::{DetailLevel, Origin, Scale, Side, View, ViewError, ViewKind};
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

fn recipe(op: &str, args: Value) -> Value {
    serde_json::to_value(Recipe::construct(op, serde_json::from_value(args).unwrap())).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

/// A 3000 mm wall along x, 3000 mm high, at `x`, on `level`.
fn wall(id: &str, level: &str, x: i64) -> Value {
    let args = json!({ "length": 3000, "thickness": 200, "height": 3000 });
    json!({ "element": id, "kind": "wall", "level": level,
            "placement": { "x": x, "y": 0, "z": 0 }, "recipe": recipe("wall.straight", args) })
}

fn level(id: &str, name: &str, elevation: &str) -> Value {
    json!({ "level": id, "name": name, "elevation": elevation })
}

/// A floor plan on `level`, with the defaults.
fn plan(id: &str, name: &str, level: &str) -> Value {
    json!({ "view": id, "kind": "floor_plan", "name": name, "level": level })
}

/// A section along x, 6 m long, looking left 5 m, with the defaults.
fn section(id: &str, name: &str) -> Value {
    json!({ "view": id, "kind": "section", "name": name, "look": "left", "depth": "5 m",
            "from": { "x": "0", "y": "0" }, "to": { "x": "6 m", "y": "0" } })
}

/// Ground at 0 and First at 3.2 m.
async fn two_levels(stack: &Stack) {
    stack
        .edit("level.create", level("L-g", "Ground", "0"))
        .await
        .unwrap();
    stack
        .edit("level.create", level("L-1", "First", "3.2 m"))
        .await
        .unwrap();
}

async fn view(stack: &Stack, id: &str) -> View {
    View::from_record(&stack.record(id).await).unwrap()
}

#[tokio::test]
async fn a_plan_is_made_with_the_defaults_and_a_section_with_its_line() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    let ground = view(&stack, "V-g").await;
    assert_eq!(ground.name(), "Ground floor");
    assert_eq!(ground.scale(), Scale::new(100).unwrap());
    assert_eq!(ground.detail(), DetailLevel::Medium);
    let setup = ground.kind().plan().unwrap();
    assert_eq!(setup.level, i("L-g"));
    let heights = [
        (&setup.range.top, 2300),
        (&setup.range.cut, 1200),
        (&setup.range.bottom, 0),
        (&setup.range.depth, -500),
    ];
    for (tie, offset) in heights {
        assert_eq!(tie.level, i("L-g"));
        assert_eq!(tie.offset, mm(offset));
    }
    assert!(matches!(ground.kind(), ViewKind::FloorPlan(_)));
    let record = stack.record("V-g").await;
    assert_eq!(record.kind, "view");
    assert_eq!(record.props["@category"], "7000");
    assert_eq!(record.level, 0);

    // Scale, detail level and a range of its own.
    let args = json!({ "view": "V-c", "kind": "ceiling_plan", "name": "First ceiling",
        "level": "L-1", "scale": 50, "detail": "fine",
        "range": { "top": { "offset": "2.8 m" }, "cut": { "offset": "2.4 m" },
                   "bottom": { "offset": "0" }, "depth": { "level": "L-g", "offset": "0" } } });
    stack.edit("view.create", args).await.unwrap();
    let ceiling = view(&stack, "V-c").await;
    assert_eq!(ceiling.scale(), Scale::new(50).unwrap());
    assert_eq!(ceiling.detail(), DetailLevel::Fine);
    let setup = ceiling.kind().plan().unwrap();
    assert_eq!(setup.range.top.level, i("L-1"));
    assert_eq!(setup.range.cut.level, i("L-1"));
    assert_eq!(setup.range.cut.offset, mm(2400));
    assert_eq!(setup.range.depth.level, i("L-g"));

    stack
        .edit("view.create", section("V-s", "Section A"))
        .await
        .unwrap();
    let cut = view(&stack, "V-s").await;
    let line = cut.kind().cut_line().unwrap();
    assert_eq!(line.to().x, mm(6000));
    assert_eq!(line.look(), Side::Left);
    assert_eq!(line.far(), mm(5000));
    assert!(matches!(cut.kind(), ViewKind::Section(_)));
    let args = json!({ "view": "V-e", "kind": "elevation", "name": "South",
        "look": "right", "depth": "3 m",
        "from": { "x": "0", "y": "0" }, "to": { "x": "6 m", "y": "0" } });
    stack.edit("view.create", args).await.unwrap();
    assert!(matches!(
        view(&stack, "V-e").await.kind(),
        ViewKind::Elevation(_)
    ));
    for (id, name, kind) in [
        ("V-3", "Model", "three_d"),
        ("V-d", "Details", "drafting"),
        ("V-l", "Legend", "legend"),
    ] {
        let args = json!({ "view": id, "kind": kind, "name": name });
        stack.edit("view.create", args).await.unwrap();
    }
    assert!(matches!(view(&stack, "V-3").await.kind(), ViewKind::ThreeD));
    assert!(matches!(
        view(&stack, "V-d").await.kind(),
        ViewKind::Drafting
    ));
    assert!(matches!(view(&stack, "V-l").await.kind(), ViewKind::Legend));
}

#[tokio::test]
async fn a_view_is_refused_with_the_reason_it_has_in_the_crate() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    stack
        .edit("view.create", section("V-s", "Section A"))
        .await
        .unwrap();
    let refused = |args: Value| {
        let stack = &stack;
        async move { stack.edit("view.create", args).await.unwrap_err() }
    };
    // Names are unique among the views, case aside, whatever their kind.
    let taken = ViewError::DuplicateName("ground FLOOR".to_owned()).to_string();
    assert_eq!(refused(plan("V-2", "ground FLOOR", "L-1")).await, taken);
    let other_kind = json!({ "view": "V-3", "kind": "three_d", "name": "section a" });
    assert_eq!(
        refused(other_kind).await,
        ViewError::DuplicateName("section a".to_owned()).to_string()
    );
    assert_eq!(
        refused(plan("V-2", "   ", "L-1")).await,
        ViewError::EmptyName.to_string()
    );
    assert_eq!(
        refused(plan("V-2", "Roof", "L-9")).await,
        ViewError::UnknownLevel(i("L-9")).to_string()
    );
    assert_eq!(
        refused(json!({ "view": "V-2", "kind": "floor_plan", "name": "Roof" })).await,
        "A plan needs a level."
    );
    assert_eq!(
        refused(json!({ "view": "V-2", "kind": "wall", "name": "Roof" })).await,
        "\"wall\" is not a kind of view. The kinds are floor_plan, ceiling_plan, area_plan, section, \
         elevation, three_d, drafting, legend."
    );
    // The same ID twice, and an ID another element has.
    assert_eq!(
        refused(plan("V-g", "Other", "L-1")).await,
        "That element already exists."
    );
    stack
        .edit("element.create", wall("W-1", "L-g", 0))
        .await
        .unwrap();
    assert_eq!(
        refused(plan("W-1", "Other", "L-1")).await,
        "That element already exists."
    );
    // Scale and detail level.
    let mut args = plan("V-2", "Roof", "L-1");
    args["scale"] = json!(0);
    assert_eq!(refused(args).await, Scale::new(0).unwrap_err().to_string());
    let mut args = plan("V-2", "Roof", "L-1");
    args["detail"] = json!("ultra");
    assert_eq!(
        refused(args).await,
        "\"ultra\" is not a detail level: coarse, medium or fine."
    );
    // An argument the kind has no use for.
    let mut args = section("V-2", "Cut");
    args["level"] = json!("L-g");
    assert_eq!(refused(args).await, "A section has no level.");
    let mut args = json!({ "view": "V-2", "kind": "legend", "name": "Key" });
    args["depth"] = json!("1 m");
    assert_eq!(refused(args).await, "A legend has no line.");
    // A line needs two ends and a depth.
    let mut args = section("V-2", "Cut");
    args["to"] = json!({ "x": "0", "y": "0" });
    assert_eq!(refused(args).await, ViewError::EmptyLine.to_string());
    let mut args = section("V-2", "Cut");
    args["depth"] = json!("0");
    assert_eq!(refused(args).await, ViewError::NoDepth.to_string());
    let mut args = section("V-2", "Cut");
    args["look"] = json!("up");
    assert_eq!(refused(args).await, "\"up\" is not a side: left or right.");
    let mut args = section("V-2", "Cut");
    args["depth"] = json!("1 furlong");
    let words = construct_levels::parse_length("1 furlong", &construct_params::Units::default())
        .unwrap_err()
        .to_string();
    assert_eq!(refused(args).await, words);
    // A range out of order, as the levels give the heights.
    let mut args = plan("V-2", "Roof", "L-1");
    args["range"] = json!({ "top": { "level": "L-g", "offset": "-1 m" } });
    assert_eq!(
        refused(args).await,
        ViewError::RangeOrder {
            top: mm(-1000).to_string(),
            cut: mm(4400).to_string(),
            bottom: mm(3200).to_string(),
            depth: mm(2700).to_string(),
        }
        .to_string()
    );
    // Arguments that do not read, and the one a command needs.
    assert!(
        refused(json!({ "view": "V-2", "kind": "legend", "name": "Key", "colour": "red" }))
            .await
            .starts_with("That edit isn't valid")
    );
    assert_eq!(
        refused(json!({ "kind": "legend", "name": "Key" })).await,
        "This command needs a view."
    );
    // None of these wrote anything.
    assert!(!stack.exists("V-2").await);
    assert_eq!(stack.rows("piece").await.len(), 5);
}

#[tokio::test]
async fn a_view_is_renamed_and_its_names_stay_unique() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    stack
        .edit("view.create", plan("V-1", "First floor", "L-1"))
        .await
        .unwrap();
    stack
        .edit(
            "view.rename",
            json!({ "view": "V-g", "name": "  Level 0 " }),
        )
        .await
        .unwrap();
    assert_eq!(view(&stack, "V-g").await.name(), "Level 0");
    // The second of a name is refused, its case aside; a view may change its own name's case.
    let refused = stack
        .edit("view.rename", json!({ "view": "V-1", "name": "LEVEL 0" }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        ViewError::DuplicateName("LEVEL 0".to_owned()).to_string()
    );
    assert_eq!(view(&stack, "V-1").await.name(), "First floor");
    stack
        .edit("view.rename", json!({ "view": "V-g", "name": "LEVEL 0" }))
        .await
        .unwrap();
    assert_eq!(view(&stack, "V-g").await.name(), "LEVEL 0");
    assert_eq!(
        stack
            .edit("view.rename", json!({ "view": "V-1", "name": " " }))
            .await
            .unwrap_err(),
        ViewError::EmptyName.to_string()
    );
    assert_eq!(
        stack
            .edit("view.rename", json!({ "view": "V-1" }))
            .await
            .unwrap_err(),
        "This command needs a name."
    );
    assert_eq!(
        stack
            .edit("view.rename", json!({ "view": "V-none", "name": "x" }))
            .await
            .unwrap_err(),
        ViewError::UnknownView(i("V-none")).to_string()
    );
    // What is not a view is not renamed by it.
    stack
        .edit("element.create", wall("W-1", "L-g", 0))
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit("view.rename", json!({ "view": "W-1", "name": "x" }))
            .await
            .unwrap_err(),
        ViewError::UnknownView(i("W-1")).to_string()
    );
}

#[tokio::test]
async fn a_view_is_set_by_the_rules_of_the_crate() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    stack
        .edit("view.create", section("V-s", "Section A"))
        .await
        .unwrap();
    let model = json!({ "view": "V-3", "kind": "three_d", "name": "Model" });
    stack.edit("view.create", model).await.unwrap();
    let set = |id: &str, args: Value| {
        let mut all = args;
        all["view"] = json!(id);
        let stack = &stack;
        async move { stack.edit("view.set", all).await }
    };
    let range =
        json!({ "cut": { "offset": "1.1 m" }, "top": { "level": "L-1", "offset": "-300" } });
    set(
        "V-g",
        json!({ "scale": 200, "detail": "coarse", "range": range }),
    )
    .await
    .unwrap();
    let ground = view(&stack, "V-g").await;
    assert_eq!(ground.scale(), Scale::new(200).unwrap());
    assert_eq!(ground.detail(), DetailLevel::Coarse);
    let range = &ground.kind().plan().unwrap().range;
    assert_eq!(range.cut.offset, mm(1100));
    assert_eq!(range.cut.level, i("L-g"));
    assert_eq!(range.top.level, i("L-1"));
    assert_eq!(range.top.offset, mm(-300));
    assert_eq!(range.bottom.offset, mm(0));

    // A plan moves to another level; its range keeps the heights it has.
    set("V-g", json!({ "level": "L-1" })).await.unwrap();
    assert_eq!(
        view(&stack, "V-g").await.kind().plan().unwrap().level,
        i("L-1")
    );
    assert_eq!(stack.record("V-g").await.props["@level"], "L-1");

    // A section's line and depth.
    let moved = json!({ "to": { "x": "8 m", "y": "1 m" }, "look": "right", "depth": "2.5 m" });
    set("V-s", moved).await.unwrap();
    let line = *view(&stack, "V-s").await.kind().cut_line().unwrap();
    assert_eq!((line.to().x, line.to().y), (mm(8000), mm(1000)));
    assert_eq!((line.from().x, line.look()), (mm(0), Side::Right));
    assert_eq!(line.far(), mm(2500));

    // Phase settings, and the view's own phase: the two every project starts with.
    let shown = json!({ "phase": "phase-new", "filter": "show_new", "choices": [] });
    set("V-3", json!({ "phasing": shown.clone() }))
        .await
        .unwrap();
    assert_eq!(
        stack.record("V-3").await.props["@view.phasing"],
        r#"{"phase":"phase-new","filter":"show_new","choices":[]}"#
    );
    set("V-3", json!({ "phasing": null })).await.unwrap();
    assert!(
        !stack
            .record("V-3")
            .await
            .props
            .contains_key("@view.phasing")
    );
    let staging =
        json!({ "created": "phase-existing", "demolished": "phase-new", "design_option": null });
    set("V-3", json!({ "staging": staging })).await.unwrap();
    assert_eq!(
        view(&stack, "V-3").await.staging().created,
        i("phase-existing")
    );

    // What the crate refuses, in its words.
    let unknown = json!({ "phasing": { "phase": "nope", "filter": "show_new", "choices": [] } });
    assert_eq!(
        set("V-3", unknown).await.unwrap_err(),
        PhaseError::UnknownPhase(i("nope")).to_string()
    );
    let option = OptionRef {
        set: i("s"),
        option: i("o"),
    };
    let staging = json!({ "created": "phase-new", "demolished": null,
                          "design_option": { "set": "s", "option": "o" } });
    assert_eq!(
        set("V-3", json!({ "staging": staging })).await.unwrap_err(),
        options().check(&option).unwrap_err().to_string()
    );
    assert_eq!(
        set("V-3", json!({ "scale": 0 })).await.unwrap_err(),
        Scale::new(0).unwrap_err().to_string()
    );
    assert_eq!(
        set("V-s", json!({ "depth": "0" })).await.unwrap_err(),
        ViewError::NoDepth.to_string()
    );
    assert_eq!(
        set("V-s", json!({ "to": { "x": "0", "y": "0" } }))
            .await
            .unwrap_err(),
        ViewError::EmptyLine.to_string()
    );
    assert_eq!(
        set("V-g", json!({ "level": "L-9" })).await.unwrap_err(),
        ViewError::UnknownLevel(i("L-9")).to_string()
    );
    let refused = set("V-g", json!({ "range": { "bottom": { "offset": "9 m" } } }))
        .await
        .unwrap_err();
    assert!(
        refused.starts_with("A view range must run top, cut plane, bottom, view depth"),
        "{refused}"
    );
    // A kind does not change, a section has no level, a model view has no range.
    assert_eq!(
        set("V-g", json!({ "kind": "section" })).await.unwrap_err(),
        "A view's kind is not changed: make another view."
    );
    assert_eq!(
        set("V-s", json!({ "level": "L-g" })).await.unwrap_err(),
        "A section has no level."
    );
    assert_eq!(
        set("V-3", json!({ "range": {} })).await.unwrap_err(),
        "A three d has no view range."
    );
    assert_eq!(
        set("V-g", json!({ "from": { "x": "0", "y": "0" } }))
            .await
            .unwrap_err(),
        "A floor plan has no line."
    );
    assert_eq!(
        set("V-g", json!({ "name": "x" })).await.unwrap_err(),
        "A view is renamed with view.rename."
    );
    assert_eq!(
        set("V-none", json!({ "scale": 50 })).await.unwrap_err(),
        ViewError::UnknownView(i("V-none")).to_string()
    );
}

#[tokio::test]
async fn setting_what_a_view_already_has_writes_nothing_and_is_no_step() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    let rows = stack.rows("piece").await;
    let step = stack.state("anna").await;
    stack.later();
    let same = json!({ "view": "V-g", "scale": 100, "detail": "medium" });
    stack.edit("view.set", same).await.unwrap();
    assert_eq!(stack.rows("piece").await, rows);
    assert_eq!(stack.state("anna").await, step);
    let same = json!({ "view": "V-g", "name": "Ground floor" });
    stack.edit("view.rename", same).await.unwrap();
    assert_eq!(stack.state("anna").await, step);
}

#[tokio::test]
async fn a_view_is_duplicated_under_a_name_that_is_free() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    let mut args = plan("V-g", "Ground floor", "L-g");
    args["scale"] = json!(50);
    stack.edit("view.create", args).await.unwrap();
    let phasing = json!({ "phase": "phase-new", "filter": "show_all", "choices": [] });
    stack
        .edit("view.set", json!({ "view": "V-g", "phasing": phasing }))
        .await
        .unwrap();
    stack
        .edit("view.duplicate", json!({ "view": "V-g", "new": "V-2" }))
        .await
        .unwrap();
    stack
        .edit("view.duplicate", json!({ "view": "V-g", "new": "V-3" }))
        .await
        .unwrap();
    let named = json!({ "view": "V-g", "new": "V-4", "name": "Tender set" });
    stack.edit("view.duplicate", named).await.unwrap();
    let original = view(&stack, "V-g").await;
    for (id, name) in [
        ("V-2", "Ground floor copy"),
        ("V-3", "Ground floor copy 2"),
        ("V-4", "Tender set"),
    ] {
        let copy = view(&stack, id).await;
        assert_eq!(copy.name(), name);
        assert_eq!(copy.kind(), original.kind());
        assert_eq!(copy.scale(), original.scale());
        assert_eq!(copy.detail(), original.detail());
        assert_eq!(copy.staging(), original.staging());
        assert_eq!(copy.phasing(), original.phasing());
    }
    let again = json!({ "view": "V-g", "new": "V-5", "name": "tender SET" });
    assert_eq!(
        stack.edit("view.duplicate", again).await.unwrap_err(),
        ViewError::DuplicateName("tender SET".to_owned()).to_string()
    );
    assert_eq!(
        stack
            .edit("view.duplicate", json!({ "view": "V-g", "new": "V-2" }))
            .await
            .unwrap_err(),
        "That element already exists."
    );
    assert_eq!(
        stack
            .edit("view.duplicate", json!({ "view": "V-g" }))
            .await
            .unwrap_err(),
        "This command needs an ID for the copy (new)."
    );
}

#[tokio::test]
async fn a_view_is_deleted() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    stack
        .edit("view.delete", json!({ "view": "V-g" }))
        .await
        .unwrap();
    assert!(!stack.exists("V-g").await);
    assert_eq!(
        stack
            .edit("view.delete", json!({ "view": "V-g" }))
            .await
            .unwrap_err(),
        ViewError::UnknownView(i("V-g")).to_string()
    );
    // The name is free again.
    stack
        .edit("view.create", plan("V-h", "Ground floor", "L-g"))
        .await
        .unwrap();
    // What is not a view is not deleted by it.
    stack
        .edit("element.create", wall("W-1", "L-g", 0))
        .await
        .unwrap();
    assert!(
        stack
            .edit("view.delete", json!({ "view": "W-1" }))
            .await
            .is_err()
    );
    assert!(stack.exists("W-1").await);
}

#[tokio::test]
async fn another_persons_claim_on_a_view_refuses_the_edit() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    let mine = json!({ "view": "V-g", "name": "Mine" });
    let refused = stack
        .edit_as("ben", "view.rename", mine.clone())
        .await
        .unwrap_err();
    assert_eq!(refused, "anna is working on this.");
    assert_eq!(view(&stack, "V-g").await.name(), "Ground floor");
    stack.later();
    stack.edit_as("ben", "view.rename", mine).await.unwrap();
}

#[tokio::test]
async fn the_generic_commands_do_not_go_behind_the_view_commands() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    stack
        .edit("element.create", wall("W-1", "L-g", 0))
        .await
        .unwrap();
    stack.later();
    let rows = stack.rows("piece").await;
    let place = json!({ "x": 0, "y": 0, "z": 0 });
    let made = "A view is made: use view.create.";
    let changed = "A view is changed: use view.set, view.rename or view.duplicate.";
    for (cmd, args, why) in [
        (
            "element.create",
            json!({ "element": "V-x", "kind": "view", "placement": place,
                    "recipe": recipe("view", json!({})) }),
            made,
        ),
        (
            "element.create",
            json!({ "element": "V-x", "kind": "wall", "placement": place,
                    "recipe": recipe("view", json!({})) }),
            made,
        ),
        (
            "element.set",
            json!({ "element": "W-1", "recipe": recipe("view", json!({})) }),
            made,
        ),
        (
            "element.set",
            json!({ "element": "V-g", "name": "Other" }),
            changed,
        ),
        (
            "element.set",
            json!({ "element": "V-g", "props": { "@view.scale": "whole:5" } }),
            changed,
        ),
        (
            "element.move",
            json!({ "element": "V-g", "placement": place }),
            changed,
        ),
        (
            "element.delete",
            json!({ "element": "V-g" }),
            "A view is deleted: use view.delete.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), why, "{cmd}");
    }
    assert_eq!(stack.rows("piece").await, rows);
    // A wall is still set and deleted through them.
    stack
        .edit("element.set", json!({ "element": "W-1", "name": "Front" }))
        .await
        .unwrap();
    stack
        .edit("element.delete", json!({ "element": "W-1" }))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_level_a_views_range_is_tied_to_is_not_deleted() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    let mut args = plan("V-g", "Ground floor", "L-g");
    args["range"] = json!({ "top": { "level": "L-1", "offset": "0" } });
    stack.edit("view.create", args).await.unwrap();
    stack
        .edit("view.create", plan("V-1", "First floor", "L-1"))
        .await
        .unwrap();
    // First is the range of one view and the level of another: both count.
    assert_eq!(
        stack
            .edit("level.delete", json!({ "level": "L-1" }))
            .await
            .unwrap_err(),
        "First can't be deleted: 2 elements are tied to it."
    );
    stack
        .edit("view.delete", json!({ "view": "V-1" }))
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit("level.delete", json!({ "level": "L-1" }))
            .await
            .unwrap_err(),
        "First can't be deleted: 1 element is tied to it."
    );
    let back = json!({ "top": { "level": "L-g", "offset": "2.3 m" } });
    stack
        .edit("view.set", json!({ "view": "V-g", "range": back }))
        .await
        .unwrap();
    stack
        .edit("level.delete", json!({ "level": "L-1" }))
        .await
        .unwrap();
    // The ground level is the plan's own: it stays too.
    assert!(
        stack
            .edit("level.delete", json!({ "level": "L-g" }))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_view_has_no_shape_and_adds_no_derived_row() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("element.create", wall("W-1", "L-g", 0))
        .await
        .unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    let mut before = BTreeMap::new();
    for table in ["body", "display", "built", "area"] {
        before.insert(table, stack.rows(table).await);
    }
    assert!(!before["built"].is_empty());
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    stack
        .edit("view.create", section("V-s", "Section A"))
        .await
        .unwrap();
    stack
        .edit("view.set", json!({ "view": "V-g", "scale": 50 }))
        .await
        .unwrap();
    stack
        .edit("view.delete", json!({ "view": "V-s" }))
        .await
        .unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    for table in ["body", "display", "built", "area"] {
        assert_eq!(stack.rows(table).await, before[table], "{table}");
    }
    // Only the wall has a shape to check.
    assert_eq!(stack.regen.verify(&i("p")).await, Ok(1));
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
async fn every_view_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    undone_and_redone(&stack, "view.create", plan("V-g", "Ground floor", "L-g")).await;
    undone_and_redone(&stack, "view.create", section("V-s", "Section A")).await;
    let higher = json!({ "top": { "level": "L-1", "offset": "0" } });
    for (cmd, args) in [
        ("view.rename", json!({ "view": "V-g", "name": "Level 0" })),
        ("view.set", json!({ "view": "V-g", "scale": 50 })),
        ("view.set", json!({ "view": "V-s", "depth": "9 m" })),
        ("view.set", json!({ "view": "V-g", "range": higher })),
        ("view.duplicate", json!({ "view": "V-g", "new": "V-2" })),
        ("view.delete", json!({ "view": "V-2" })),
        ("view.delete", json!({ "view": "V-s" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn an_undo_may_not_leave_a_view_on_a_level_that_is_gone() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("level.create", level("L-2", "Second", "6 m"))
        .await
        .unwrap();
    stack.later();
    // Ben makes a plan of Anna's new level; her undo of making the level would leave it on
    // nothing, so it is turned back.
    stack
        .edit_as("ben", "view.create", plan("V-b", "Second floor", "L-2"))
        .await
        .unwrap();
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert_eq!(
        refused,
        format!(
            "{CANNOT}Second floor: {}",
            ViewError::UnknownLevel(i("L-2"))
        )
    );
    assert!(stack.exists("L-2").await);
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    for (cmd, args) in [
        ("view.create", plan("V-g", "Ground floor", "L-g")),
        ("view.create", plan("V-1", "First floor", "L-1")),
        ("view.create", section("V-s", "Section A")),
        ("view.create", plan("V-x", "ground floor", "L-1")),
        ("view.rename", json!({ "view": "V-1", "name": "Level 1" })),
        (
            "view.set",
            json!({ "view": "V-g", "scale": 50, "detail": "fine" }),
        ),
        (
            "view.set",
            json!({ "view": "V-s", "depth": "2 m", "look": "right" }),
        ),
        ("view.set", json!({ "view": "V-s", "depth": "0" })),
        ("view.duplicate", json!({ "view": "V-g", "new": "V-d" })),
        ("element.set", json!({ "element": "V-g", "name": "x" })),
        ("edit.undo", json!({})),
        ("view.delete", json!({ "view": "V-1" })),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
        ("level.delete", json!({ "level": "L-1" })),
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
    assert!(entries.iter().any(|entry| entry.cmd == "view.duplicate"));

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

// ---- Graphics rules (CON-A8-F3-P2) ----

const WALLS: u32 = 100;
const CORE_CUT_LINE: u32 = 101;
const DOORS: u32 = 300;
const DOOR_SWING: u32 = 304;
const VIEWS_CATEGORY: u32 = 7000;

/// A plan, a section, a three-dimensional view and a drafting view, ready for rules.
async fn rule_views() -> Stack {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    stack
        .edit("view.create", section("V-s", "Section A"))
        .await
        .unwrap();
    for (id, kind, name) in [("V-3", "three_d", "Model"), ("V-d", "drafting", "Detail")] {
        let args = json!({ "view": id, "kind": kind, "name": name });
        stack.edit("view.create", args).await.unwrap();
    }
    stack
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "a test helper called with json! literals"
)]
fn rule(view: &str, category: u32, edge: &str, style: Value) -> Value {
    json!({ "view": view, "category": category, "edge": edge, "style": style })
}

fn first_pen() -> u16 {
    LineStyles::default_metric()
        .model_pens
        .pens()
        .first()
        .unwrap()
        .number
}

#[tokio::test]
async fn a_rule_is_set_over_what_the_element_looks_like_now() {
    let stack = rule_views().await;
    assert!(view(&stack, "V-g").await.graphics().is_empty());
    let no_key = |record: &Element| !record.props.contains_key(construct_views::GRAPHICS_KEY);
    assert!(no_key(&stack.record("V-g").await));

    // Naming only what changes: the rest is the project's style for walls, cut.
    let hide = json!({ "shown": false, "transparency": 30, "halftone": true });
    stack
        .edit("view.graphics.set", rule("V-g", WALLS, "cut", hide))
        .await
        .unwrap();
    let ground = view(&stack, "V-g").await;
    let style = ground
        .graphics()
        .style(CategoryId(WALLS), Edge::Cut)
        .unwrap()
        .clone();
    assert!(!style.shown && style.halftone && style.transparency == 30);
    let project = construct_views::resolve(
        &view(&stack, "V-s").await,
        &CategoryTable::new(),
        &ObjectStyles::default_metric(),
        &LineStyles::default_metric(),
        CategoryId(WALLS),
        None,
        Edge::Cut,
    )
    .unwrap();
    assert_eq!(project.origin, Origin::Project);
    assert_eq!(style.pen, project.style.pen);
    assert_eq!(style.colour, project.style.colour);
    assert_eq!(style.fill, None);
    // Only the cut was written, and only on this view.
    let seen = ground.graphics().style(CategoryId(WALLS), Edge::Projected);
    assert!(seen.is_none());
    assert!(view(&stack, "V-s").await.graphics().is_empty());
    assert!(!no_key(&stack.record("V-g").await));

    // A second command changes some parts and keeps the rest.
    let more = json!({ "fill": { "pattern": "concrete", "colour": [10, 20, 30] },
                       "colour": [1, 2, 3], "pen": first_pen() });
    stack.later();
    stack
        .edit("view.graphics.set", rule("V-g", WALLS, "cut", more))
        .await
        .unwrap();
    let changed = view(&stack, "V-g").await;
    let style = changed
        .graphics()
        .style(CategoryId(WALLS), Edge::Cut)
        .unwrap();
    assert!(!style.shown && style.halftone && style.transparency == 30);
    assert_eq!(style.colour, Colour(1, 2, 3));
    assert_eq!(style.pen, first_pen());
    let fill = style.fill.as_ref().unwrap();
    assert_eq!(
        (fill.pattern.as_str(), fill.colour),
        ("concrete", Colour(10, 20, 30))
    );

    // `fill: null` takes the fill away; a subcategory's projection is a rule of its own.
    stack
        .edit(
            "view.graphics.set",
            rule("V-g", WALLS, "cut", json!({ "fill": null })),
        )
        .await
        .unwrap();
    stack
        .edit(
            "view.graphics.set",
            rule("V-g", CORE_CUT_LINE, "projection", json!({ "pattern": 2 })),
        )
        .await
        .unwrap();
    let last = view(&stack, "V-g").await;
    let walls_cut = last.graphics().style(CategoryId(WALLS), Edge::Cut);
    assert_eq!(walls_cut.unwrap().fill, None);
    let sub = last
        .graphics()
        .style(CategoryId(CORE_CUT_LINE), Edge::Projected)
        .unwrap();
    assert_eq!(sub.pattern, construct_views::PatternId(2));
    assert_eq!(last.graphics().rules().count(), 2);
    // The stored record reads back as the same view.
    assert_eq!(view(&stack, "V-g").await, last);
}

#[tokio::test]
async fn a_rule_that_is_not_allowed_is_refused_in_words_and_changes_nothing() {
    let stack = rule_views().await;
    stack.later();
    let before = stack.rows("piece").await;
    let colour_words = "A colour is [red, green, blue], each a whole number from 0 to 255.";
    let concrete = json!({ "fill": { "pattern": "concrete", "colour": [0, 0, 0] } });
    let tartan = json!({ "fill": { "pattern": "tartan", "colour": [0, 0, 0] } });
    let by_set = |what: &str| {
        format!(
            "{what} has no category, edge or style: graphics rules are set with view.graphics.set."
        )
    };
    let cases = [
        (
            "view.graphics.set",
            rule("V-g", 31, "cut", json!({})),
            "There is no category 31.".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", 0, "cut", json!({})),
            "There is no category 0.".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", json!({ "pen": 65_000 })),
            "There is no pen 65000 for this category.".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", json!({ "pen": 70_000 })),
            "There is no pen 70000 for this category.".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", json!({ "pattern": 9999 })),
            "There is no line pattern 9999.".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", json!({ "transparency": 101 })),
            "Transparency is a percentage from 0 to 100, not 101.".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", json!({ "transparency": -1 })),
            "Transparency is a percentage from 0 to 100, not -1.".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", json!({ "colour": [1, 2, 256] })),
            colour_words.to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", json!({ "colour": [1, 2] })),
            colour_words.to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", tartan),
            ViewError::NoSuchFill("tartan".to_owned()).to_string(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "projection", concrete),
            "Only a cut has a fill: a projection rule has none.".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-g", DOOR_SWING, "cut", json!({})),
            ViewError::CutOnUncuttable(CategoryId(DOOR_SWING)).to_string(),
        ),
        (
            "view.graphics.set",
            rule("V-g", VIEWS_CATEGORY, "projection", json!({})),
            ViewError::NotStyled(CategoryId(VIEWS_CATEGORY)).to_string(),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "both", json!({})),
            "\"both\" is not an edge: cut or projection.".to_owned(),
        ),
        (
            "view.graphics.set",
            json!({ "view": "V-g", "category": WALLS, "style": {} }),
            "This command needs an edge (cut or projection).".to_owned(),
        ),
        (
            "view.graphics.set",
            json!({ "view": "V-g", "category": WALLS, "edge": "cut" }),
            "This command needs a style.".to_owned(),
        ),
        (
            "view.graphics.set",
            json!({ "view": "V-g", "edge": "cut", "style": {} }),
            "This command needs a category (its number).".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-x", WALLS, "cut", json!({})),
            ViewError::UnknownView(i("V-x")).to_string(),
        ),
        // A kind of view that cannot hold a rule.
        (
            "view.graphics.set",
            rule("V-d", WALLS, "projection", json!({ "halftone": true })),
            "A drafting view shows no model, so it has no graphics rules.".to_owned(),
        ),
        (
            "view.graphics.set",
            rule("V-3", WALLS, "cut", json!({})),
            "A three d view cuts nothing, so it has no cut rules.".to_owned(),
        ),
        (
            "view.graphics.clear",
            json!({ "view": "V-g", "category": 31 }),
            "There is no category 31.".to_owned(),
        ),
        (
            "view.graphics.clear",
            json!({ "view": "V-g", "category": WALLS, "edge": "x" }),
            "\"x\" is not an edge: cut or projection.".to_owned(),
        ),
        (
            "view.graphics.clear",
            json!({ "view": "V-g", "category": WALLS, "style": {} }),
            "view.graphics.clear takes no style.".to_owned(),
        ),
        (
            "view.set",
            json!({ "view": "V-g", "category": WALLS }),
            by_set("view.set"),
        ),
        (
            "view.rename",
            json!({ "view": "V-g", "name": "x", "style": {} }),
            by_set("view.rename"),
        ),
    ];
    for (cmd, args, why) in cases {
        assert_eq!(
            stack.edit(cmd, args.clone()).await.unwrap_err(),
            why,
            "{args}"
        );
    }
    // A part of a style that is not one is not read either.
    let unknown = rule("V-g", WALLS, "cut", json!({ "weight": 3 }));
    let refusal = stack.edit("view.graphics.set", unknown).await.unwrap_err();
    assert!(refusal.starts_with("That edit isn't valid"), "{refusal}");
    assert_eq!(stack.rows("piece").await, before);

    // The generic commands are no way round: a view's record is not set or deleted with them.
    for (cmd, args) in [
        ("element.set", json!({ "element": "V-g", "name": "x" })),
        ("element.delete", json!({ "element": "V-g" })),
    ] {
        let refusal = stack.edit(cmd, args).await.unwrap_err();
        assert!(refusal.contains("use view."), "{refusal}");
    }
    assert_eq!(stack.rows("piece").await, before);
}

#[tokio::test]
async fn clearing_takes_a_rule_or_both_away_and_nothing_to_clear_is_no_step() {
    let stack = rule_views().await;
    let cut = rule("V-g", WALLS, "cut", json!({ "shown": false }));
    let seen = rule("V-g", WALLS, "projection", json!({ "halftone": true }));
    stack.edit("view.graphics.set", cut.clone()).await.unwrap();
    stack.edit("view.graphics.set", seen).await.unwrap();
    // The same rule again writes nothing: it is no step.
    let rows = stack.rows("piece").await;
    stack.later();
    stack.edit("view.graphics.set", cut).await.unwrap();
    assert_eq!(stack.rows("piece").await, rows);

    stack
        .edit(
            "view.graphics.clear",
            json!({ "view": "V-g", "category": WALLS, "edge": "projection" }),
        )
        .await
        .unwrap();
    let ground = view(&stack, "V-g").await;
    let walls = CategoryId(WALLS);
    assert!(ground.graphics().style(walls, Edge::Cut).is_some());
    assert!(ground.graphics().style(walls, Edge::Projected).is_none());
    stack
        .edit(
            "view.graphics.clear",
            json!({ "view": "V-g", "category": WALLS }),
        )
        .await
        .unwrap();
    assert!(view(&stack, "V-g").await.graphics().is_empty());
    let record = stack.record("V-g").await;
    assert!(!record.props.contains_key(construct_views::GRAPHICS_KEY));
    // Nothing left to clear: no step, no refusal.
    let rows = stack.rows("piece").await;
    stack
        .edit(
            "view.graphics.clear",
            json!({ "view": "V-g", "category": WALLS }),
        )
        .await
        .unwrap();
    assert_eq!(stack.rows("piece").await, rows);
}

#[tokio::test]
async fn a_duplicate_keeps_the_rules_and_other_view_commands_leave_them() {
    let stack = rule_views().await;
    stack
        .edit(
            "view.graphics.set",
            rule("V-g", DOORS, "projection", json!({ "transparency": 50 })),
        )
        .await
        .unwrap();
    stack
        .edit("view.duplicate", json!({ "view": "V-g", "new": "V-2" }))
        .await
        .unwrap();
    let copy = view(&stack, "V-2").await;
    assert_eq!(copy.graphics(), view(&stack, "V-g").await.graphics());
    assert!(!copy.graphics().is_empty());
    stack
        .edit("view.set", json!({ "view": "V-g", "scale": 50 }))
        .await
        .unwrap();
    stack
        .edit("view.rename", json!({ "view": "V-g", "name": "Level 0" }))
        .await
        .unwrap();
    assert_eq!(view(&stack, "V-g").await.graphics(), copy.graphics());
}

#[tokio::test]
async fn every_graphics_command_is_undone_and_redone() {
    let stack = rule_views().await;
    let hidden = json!({ "shown": false });
    for (cmd, args) in [
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", hidden.clone()),
        ),
        (
            "view.graphics.set",
            rule("V-g", WALLS, "cut", json!({ "transparency": 20 })),
        ),
        (
            "view.graphics.set",
            rule(
                "V-s",
                CORE_CUT_LINE,
                "projection",
                json!({ "halftone": true }),
            ),
        ),
        (
            "view.graphics.set",
            rule("V-3", WALLS, "projection", hidden),
        ),
        (
            "view.graphics.clear",
            json!({ "view": "V-g", "category": WALLS, "edge": "cut" }),
        ),
        ("view.duplicate", json!({ "view": "V-s", "new": "V-s2" })),
        (
            "view.graphics.clear",
            json!({ "view": "V-s2", "category": CORE_CUT_LINE }),
        ),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn rules_replay_from_the_log_to_the_same_bytes() {
    let stack = rule_views().await;
    for (cmd, args) in [
        (
            "view.graphics.set",
            rule(
                "V-g",
                WALLS,
                "cut",
                json!({ "shown": false, "colour": [9, 9, 9] }),
            ),
        ),
        (
            "view.graphics.set",
            rule("V-g", DOORS, "projection", json!({ "transparency": 80 })),
        ),
        ("view.graphics.set", rule("V-g", 31, "cut", json!({}))),
        ("view.duplicate", json!({ "view": "V-g", "new": "V-d2" })),
        (
            "view.graphics.clear",
            json!({ "view": "V-g", "category": DOORS }),
        ),
        ("edit.undo", json!({})),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
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
    assert!(entries.iter().any(|entry| entry.cmd == "view.graphics.set"));
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
}

fn rect_region(x0: &str, y0: &str, x1: &str, y1: &str) -> Value {
    json!({ "rectangle": { "min": { "x": x0, "y": y0 }, "max": { "x": x1, "y": y1 } } })
}

fn outline_region(corners: &[(&str, &str)]) -> Value {
    let corners: Vec<Value> = corners
        .iter()
        .map(|(x, y)| json!({ "x": x, "y": y }))
        .collect();
    json!({ "outline": corners })
}

async fn crop_views() -> Stack {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack
        .edit("view.create", plan("V-g", "Ground floor", "L-g"))
        .await
        .unwrap();
    stack
        .edit("view.create", section("V-s", "Section A"))
        .await
        .unwrap();
    for (id, kind, name) in [
        ("V-3", "three_d", "House"),
        ("V-d", "drafting", "Details"),
        ("V-l", "legend", "Legend"),
    ] {
        stack
            .edit(
                "view.create",
                json!({ "view": id, "kind": kind, "name": name }),
            )
            .await
            .unwrap();
    }
    stack
}

#[tokio::test]
async fn a_crop_is_set_changed_shown_and_cleared_through_the_log() {
    let stack = crop_views().await;
    assert!(
        view(&stack, "V-g").await.crop().is_none(),
        "an old view has none"
    );
    stack
        .edit(
            "view.crop.set",
            json!({ "view": "V-g", "region": rect_region("0", "0", "12 m", "8 m") }),
        )
        .await
        .unwrap();
    let crop = view(&stack, "V-g").await.crop().cloned().unwrap();
    assert!(crop.is_on() && crop.is_shown() && crop.annotation().is_none());
    assert_eq!(crop.region().key(), "rectangle");
    assert_eq!(
        crop.region().size(),
        Some((Length::from_mm(12_000), Length::from_mm(8_000)))
    );

    // A shaped crop replaces the region; the switches stay.
    let l = outline_region(&[
        ("0", "0"),
        ("12 m", "0"),
        ("12 m", "3 m"),
        ("4 m", "3 m"),
        ("4 m", "8 m"),
        ("0", "8 m"),
    ]);
    stack
        .edit("view.crop.set", json!({ "view": "V-g", "region": l }))
        .await
        .unwrap();
    let crop = view(&stack, "V-g").await.crop().cloned().unwrap();
    assert_eq!(crop.region().key(), "outline");
    assert_eq!(crop.region().corners().len(), 6);

    // The annotation region, and off.
    stack
        .edit(
            "view.crop.set",
            json!({ "view": "V-g", "annotation": rect_region("-1 m", "-1 m", "13 m", "9 m"),
                    "on": false }),
        )
        .await
        .unwrap();
    let crop = view(&stack, "V-g").await.crop().cloned().unwrap();
    assert!(!crop.is_on());
    assert_eq!(crop.annotation().unwrap().key(), "rectangle");
    stack
        .edit(
            "view.crop.set",
            json!({ "view": "V-g", "annotation": null }),
        )
        .await
        .unwrap();
    assert!(
        view(&stack, "V-g")
            .await
            .crop()
            .unwrap()
            .annotation()
            .is_none()
    );

    // Hide and show.
    stack
        .edit("view.crop.show", json!({ "view": "V-g", "shown": false }))
        .await
        .unwrap();
    assert!(!view(&stack, "V-g").await.crop().unwrap().is_shown());
    stack
        .edit("view.crop.show", json!({ "view": "V-g", "shown": true }))
        .await
        .unwrap();
    assert!(view(&stack, "V-g").await.crop().unwrap().is_shown());

    // A section holds one too, in its own coordinates; clearing takes it away.
    stack
        .edit(
            "view.crop.set",
            json!({ "view": "V-s", "region": rect_region("0", "0", "6 m", "3.2 m") }),
        )
        .await
        .unwrap();
    assert!(view(&stack, "V-s").await.crop().is_some());
    stack
        .edit("view.crop.clear", json!({ "view": "V-s" }))
        .await
        .unwrap();
    assert!(view(&stack, "V-s").await.crop().is_none());
}

#[tokio::test]
async fn a_crop_change_that_changes_nothing_is_no_step() {
    let stack = crop_views().await;
    let region = rect_region("0", "0", "12 m", "8 m");
    stack
        .edit(
            "view.crop.set",
            json!({ "view": "V-g", "region": region.clone() }),
        )
        .await
        .unwrap();
    let rows = stack.rows("piece").await;
    let step = stack.state("anna").await.undo.map(|step| step.cmd);
    stack.later();
    for (cmd, args) in [
        ("view.crop.set", json!({ "view": "V-g", "region": region })),
        ("view.crop.set", json!({ "view": "V-g", "on": true })),
        ("view.crop.show", json!({ "view": "V-g", "shown": true })),
        ("view.crop.clear", json!({ "view": "V-s" })),
    ] {
        stack.edit(cmd, args).await.unwrap();
    }
    assert_eq!(stack.rows("piece").await, rows);
    assert_eq!(stack.state("anna").await.undo.map(|step| step.cmd), step);
}

#[tokio::test]
async fn every_crop_command_is_undone_and_redone() {
    let stack = crop_views().await;
    let l = outline_region(&[("0", "0"), ("4 m", "0"), ("4 m", "3 m"), ("0", "3 m")]);
    for (cmd, args) in [
        (
            "view.crop.set",
            json!({ "view": "V-g", "region": rect_region("0", "0", "12 m", "8 m") }),
        ),
        ("view.crop.set", json!({ "view": "V-g", "region": l })),
        (
            "view.crop.set",
            json!({ "view": "V-g", "annotation": rect_region("-1 m", "-1 m", "13 m", "9 m") }),
        ),
        ("view.crop.set", json!({ "view": "V-g", "on": false })),
        ("view.crop.show", json!({ "view": "V-g", "shown": false })),
        ("view.duplicate", json!({ "view": "V-g", "new": "V-2" })),
        ("view.crop.clear", json!({ "view": "V-g" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn a_crop_that_is_not_allowed_is_refused_in_words_and_changes_nothing() {
    let stack = crop_views().await;
    let region = rect_region("0", "0", "12 m", "8 m");
    let refuse = |cmd: &'static str, args: Value, why: &'static str| {
        let stack = &stack;
        async move {
            assert_eq!(stack.edit(cmd, args).await.unwrap_err(), why, "{cmd}");
        }
    };
    let rows = stack.rows("piece").await;
    refuse(
        "view.crop.set",
        json!({ "view": "V-g", "region": rect_region("0", "0", "0", "8 m") }),
        "A crop rectangle needs its lower left corner below and left of its upper right.",
    )
    .await;
    refuse(
        "view.crop.set",
        json!({ "view": "V-g", "region": outline_region(&[("0", "0"), ("1 m", "0")]) }),
        "A crop outline needs at least three corners, not 2.",
    )
    .await;
    refuse(
        "view.crop.set",
        json!({ "view": "V-g", "region":
                outline_region(&[("0", "0"), ("1 m", "0"), ("1 m", "0"), ("0", "1 m")]) }),
        "A crop outline gives corner 2 twice in a row.",
    )
    .await;
    refuse(
        "view.crop.set",
        json!({ "view": "V-g", "region":
                outline_region(&[("0", "0"), ("1 m", "1 m"), ("1 m", "0"), ("0", "1 m")]) }),
        "A crop outline must not cross itself: edges 1 and 3 meet.",
    )
    .await;
    refuse(
        "view.crop.set",
        json!({ "view": "V-g", "region": outline_region(&[
                ("0", "0"), ("4 m", "0"), ("4 m", "3 m"), ("2 m", "0"), ("0", "3 m")]) }),
        "A crop outline must not cross itself: edges 1 and 3 meet.",
    )
    .await;
    refuse(
        "view.crop.set",
        json!({ "view": "V-g", "region": region.clone(),
                "annotation": outline_region(&[("0", "0"), ("1 m", "0")]) }),
        "A crop outline needs at least three corners, not 2.",
    )
    .await;
    refuse(
        "view.crop.set",
        json!({ "view": "V-g", "on": false }),
        "This view has no crop yet: view.crop.set needs a region.",
    )
    .await;
    refuse(
        "view.crop.show",
        json!({ "view": "V-g", "shown": true }),
        "The view V-g has no crop.",
    )
    .await;
    refuse(
        "view.crop.show",
        json!({ "view": "V-g" }),
        "This command needs shown (true or false).",
    )
    .await;
    for (view, kind) in [("V-3", "three d"), ("V-d", "drafting"), ("V-l", "legend")] {
        let why =
            format!("A {kind} view has no crop: only plans, sections and elevations are cropped.");
        let got = stack
            .edit(
                "view.crop.set",
                json!({ "view": view, "region": region.clone() }),
            )
            .await
            .unwrap_err();
        assert_eq!(got, why);
    }
    assert!(
        stack
            .edit(
                "view.crop.set",
                json!({ "view": "V-none", "region": region.clone() })
            )
            .await
            .unwrap_err()
            .contains("no view V-none")
    );
    assert_eq!(stack.rows("piece").await, rows, "nothing was written");

    stack
        .edit(
            "view.crop.set",
            json!({ "view": "V-g", "region": region.clone() }),
        )
        .await
        .unwrap();
    let rows = stack.rows("piece").await;
    refuse(
        "view.crop.set",
        json!({ "view": "V-g", "shown": false }),
        "view.crop.set does not show or hide the outline: use view.crop.show.",
    )
    .await;
    refuse(
        "view.crop.set",
        json!({ "view": "V-g", "scale": 50 }),
        "view.crop.set takes only a view and the crop's own parts.",
    )
    .await;
    refuse(
        "view.crop.clear",
        json!({ "view": "V-g", "on": true }),
        "view.crop.clear takes no part of a crop.",
    )
    .await;
    refuse(
        "view.crop.show",
        json!({ "view": "V-g", "shown": false, "on": true }),
        "view.crop.show takes only shown.",
    )
    .await;
    assert!(
        stack
            .edit(
                "view.crop.set",
                json!({ "view": "V-g", "region": rect_region("x", "0", "1 m", "1 m") })
            )
            .await
            .is_err()
    );
    assert_eq!(stack.rows("piece").await, rows);
}

#[tokio::test]
async fn the_generic_and_other_view_commands_are_no_way_round_the_crop() {
    let stack = crop_views().await;
    stack
        .edit(
            "view.crop.set",
            json!({ "view": "V-g", "region": rect_region("0", "0", "12 m", "8 m") }),
        )
        .await
        .unwrap();
    stack.later();
    let rows = stack.rows("piece").await;
    let changed = "A view is changed: use view.set, view.rename or view.duplicate.";
    assert_eq!(
        stack
            .edit(
                "element.set",
                json!({ "element": "V-g", "props": { "@view.crop": "{}" } })
            )
            .await
            .unwrap_err(),
        changed
    );
    let why = "view.set has no region, annotation, on or shown: a crop is set with view.crop.set.";
    let one = rect_region("0", "0", "1 m", "1 m");
    for args in [
        json!({ "view": "V-g", "region": one }),
        json!({ "view": "V-g", "on": false }),
        json!({ "view": "V-g", "shown": false }),
        json!({ "view": "V-g", "annotation": null }),
    ] {
        assert_eq!(stack.edit("view.set", args).await.unwrap_err(), why);
    }
    let made = json!({ "view": "V-n", "kind": "section", "name": "N", "look": "left",
                       "depth": "5 m", "from": { "x": "0", "y": "0" },
                       "to": { "x": "6 m", "y": "0" }, "region": one });
    assert!(
        stack
            .edit("view.create", made)
            .await
            .unwrap_err()
            .starts_with("view.create has no region")
    );
    assert_eq!(stack.rows("piece").await, rows);
    // Other view commands leave the crop be, and a duplicate keeps it.
    let before = view(&stack, "V-g").await.crop().cloned().unwrap();
    stack
        .edit("view.set", json!({ "view": "V-g", "scale": 50 }))
        .await
        .unwrap();
    stack
        .edit("view.rename", json!({ "view": "V-g", "name": "Level 0" }))
        .await
        .unwrap();
    assert_eq!(view(&stack, "V-g").await.crop(), Some(&before));
    stack
        .edit("view.duplicate", json!({ "view": "V-g", "new": "V-2" }))
        .await
        .unwrap();
    assert_eq!(view(&stack, "V-2").await.crop(), Some(&before));
}

#[tokio::test]
async fn the_view_range_is_set_by_view_set_with_its_default_and_its_refusals() {
    let stack = crop_views().await;
    // A new plan's range is the default the drawing has always used: 1200 mm up for the cut.
    let setup = view(&stack, "V-g").await.kind().plan().unwrap().clone();
    assert_eq!(setup.range.cut.offset, Length::from_mm(1200));
    assert_eq!(setup.range.top.offset, Length::from_mm(2300));
    assert_eq!(setup.range.bottom.offset, Length::from_mm(0));
    assert_eq!(setup.range.depth.offset, Length::from_mm(-500));
    let range = json!({ "top": { "offset": "4 m" }, "cut": { "level": "L-1", "offset": "0.3 m" } });
    undone_and_redone(&stack, "view.set", json!({ "view": "V-g", "range": range })).await;
    let setup = view(&stack, "V-g").await.kind().plan().unwrap().clone();
    assert_eq!(setup.range.cut.level, i("L-1"));
    // Top below the cut, cut below the bottom and a level that does not exist are each refused.
    let rows = stack.rows("piece").await;
    let top_low = stack
        .edit(
            "view.set",
            json!({ "view": "V-g", "range": { "top": { "offset": "0" } } }),
        )
        .await
        .unwrap_err();
    assert!(top_low.starts_with("A view range must run top, cut plane, bottom, view depth"));
    let bottom_high = stack
        .edit(
            "view.set",
            json!({ "view": "V-g", "range": { "bottom": { "level": "L-1", "offset": "1 m" } } }),
        )
        .await
        .unwrap_err();
    assert!(bottom_high.starts_with("A view range must run top"));
    assert_eq!(
        stack
            .edit(
                "view.set",
                json!({ "view": "V-g", "range": { "depth": { "level": "L-9" } } })
            )
            .await
            .unwrap_err(),
        "There is no level L-9 in this model."
    );
    assert_eq!(stack.rows("piece").await, rows);
}
