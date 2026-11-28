//! View templates, per-element overrides and view filters through the edit log, on the test store
//! (CON-A8-F3-P4, CON-A8-F3-P6): a template tied to two views changes both in one step, a tied
//! view's change to an included setting is refused in words, a template applied once copies, an
//! override is cleared by one command, two filters decide in order and are asked again when a
//! parameter changes, every command is undone and redone, and the log replays to the same bytes.

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
use construct_api::cloud::view_templates::{element_styles, filters_of, overrides_of};
use construct_api::cloud::views::{World, read_world, template_graphics_of};
use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::CategoryId;
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_views::{DetailLevel, Edge, View};
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

const WALLS: u32 = 100;
const DOORS: u32 = 300;

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

fn cat(n: u32) -> CategoryId {
    CategoryId(n)
}

async fn world(stack: &Stack) -> World {
    let mut snapshot = stack.store.snapshot().await.unwrap();
    read_world(&mut snapshot, &i("p"), &i(DOCUMENT))
        .await
        .unwrap()
        .unwrap()
}

/// A view as it shows: its own settings with its template's included ones over them.
async fn shown(stack: &Stack, id: &str) -> View {
    world(stack).await.views().get(&i(id)).unwrap().clone()
}

/// Two plans, a section and a drafting view, two walls; the first plan at 1:50 with its walls cut
/// in red halftone.
async fn setup() -> Stack {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    for (id, name, level) in [
        ("V-a", "Level 1 plan", "L-g"),
        ("V-b", "Level 2 plan", "L-1"),
    ] {
        stack
            .edit("view.create", plan(id, name, level))
            .await
            .unwrap();
    }
    stack
        .edit("view.create", section("V-s", "Section A"))
        .await
        .unwrap();
    stack
        .edit(
            "view.create",
            json!({ "view": "V-d", "kind": "drafting", "name": "Detail" }),
        )
        .await
        .unwrap();
    for (id, x) in [("W-1", 0), ("W-2", 4000)] {
        stack
            .edit("element.create", wall(id, "L-g", x))
            .await
            .unwrap();
    }
    stack
        .edit("view.set", json!({ "view": "V-a", "scale": 50 }))
        .await
        .unwrap();
    let rule = json!({ "view": "V-a", "category": WALLS, "edge": "cut",
                       "style": { "colour": [200, 10, 20], "halftone": true } });
    stack.edit("view.graphics.set", rule).await.unwrap();
    stack
}

async fn make_template(stack: &Stack) {
    let args = json!({ "template": "T-1", "name": "Working", "source": "V-a" });
    stack.edit("view.template.create", args).await.unwrap();
}

async fn tie(stack: &Stack, view: &str) {
    let args = json!({ "view": view, "template": "T-1" });
    stack.edit("view.template.tie", args).await.unwrap();
}

#[tokio::test]
async fn a_template_made_from_a_view_tied_to_two_views_changes_both_in_one_step() {
    let stack = setup().await;
    make_template(&stack).await;
    let made = stack.record("T-1").await;
    assert_eq!(made.kind, "view_template");
    assert_eq!(made.name, "Working");
    for view in ["V-b", "V-s"] {
        tie(&stack, view).await;
        let one = shown(&stack, view).await;
        assert_eq!(one.scale().denominator(), 50, "{view}");
        let rule = one.graphics().style(cat(WALLS), Edge::Cut).unwrap();
        assert!(rule.halftone && rule.colour.0 == 200, "{view}");
        assert_eq!(one.template(), Some(&i("T-1")));
    }
    let before = stack.rows("piece").await;

    // One command changes the template; both views change with it and no view record is written.
    let args = json!({ "template": "T-1", "scale": 20, "detail": "fine" });
    stack.edit("view.template.set", args).await.unwrap();
    let hide = json!({ "template": "T-1", "category": WALLS, "edge": "cut",
                       "style": { "shown": false } });
    stack.edit("view.graphics.set", hide).await.unwrap();
    for view in ["V-b", "V-s"] {
        let one = shown(&stack, view).await;
        assert_eq!(one.scale().denominator(), 20, "{view}");
        assert_eq!(one.detail(), DetailLevel::Fine, "{view}");
        let rule = one.graphics().style(cat(WALLS), Edge::Cut).unwrap();
        assert!(
            !rule.shown && rule.halftone,
            "{view}: the named part changed, the rest stayed"
        );
    }
    let after = stack.rows("piece").await;
    let changed: Vec<&String> = before
        .keys()
        .filter(|key| before.get(*key) != after.get(*key))
        .collect();
    assert_eq!(
        changed.len(),
        1,
        "only the template's pointer moved: {changed:?}"
    );
    assert_eq!(view(&stack, "V-b").await.scale().denominator(), 50);

    // The plan's own drawing asks the same view: hiding walls in the template hides them in both.
    let world = world(&stack).await;
    for view in ["V-b", "V-s"] {
        let look =
            construct_api::cloud::views::look_in(&world, &i(view), cat(WALLS), Edge::Cut).unwrap();
        assert!(!look.style.shown, "{view}");
    }
    let summaries = construct_api::cloud::view_templates::template_summaries(&world);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].views, ["V-b", "V-s"]);
    assert_eq!(summaries[0].included.len(), 5);
}

#[tokio::test]
async fn a_tied_view_cannot_change_an_included_setting_and_the_refusal_says_why() {
    let stack = setup().await;
    make_template(&stack).await;
    tie(&stack, "V-b").await;
    let before = stack.rows("piece").await;
    let words = "Level 2 plan follows the view template Working, which sets its";
    let hide = json!({ "view": "V-b", "category": WALLS, "shown": false });
    let rule = json!({ "view": "V-b", "category": WALLS, "edge": "cut",
                       "style": { "shown": false } });
    let filter = json!({ "view": "V-b", "name": "F", "category": WALLS, "parameter": "p",
                         "op": "equals", "value": "x" });
    for (cmd, args, setting) in [
        ("view.set", json!({ "view": "V-b", "scale": 100 }), "scale"),
        (
            "view.set",
            json!({ "view": "V-b", "detail": "coarse" }),
            "detail level",
        ),
        ("view.graphics.set", rule, "graphics rules"),
        (
            "view.graphics.clear",
            json!({ "view": "V-b", "category": WALLS }),
            "graphics rules",
        ),
        ("view.categories.set", hide, "categories shown"),
        ("view.filter.add", filter, "filters"),
    ] {
        let refusal = stack.edit(cmd, args).await.unwrap_err();
        assert!(refusal.starts_with(words), "{cmd}: {refusal}");
        assert!(refusal.contains(setting), "{cmd}: {refusal}");
        assert!(refusal.ends_with("change it in the template, or untie the view first."));
    }
    assert_eq!(
        stack.rows("piece").await,
        before,
        "a refusal changes nothing"
    );

    // A setting the template does not include is the view's own again.
    let args = json!({ "template": "T-1", "included": { "scale": false } });
    stack.edit("view.template.set", args).await.unwrap();
    stack
        .edit("view.set", json!({ "view": "V-b", "scale": 100 }))
        .await
        .unwrap();
    let one = shown(&stack, "V-b").await;
    assert_eq!(one.scale().denominator(), 100);
    assert!(one.graphics().style(cat(WALLS), Edge::Cut).is_some());
    // Detail is still the template's.
    assert!(
        stack
            .edit("view.set", json!({ "view": "V-b", "detail": "coarse" }))
            .await
            .is_err()
    );
    // Overrides are always the view's own.
    let args = json!({ "view": "V-b", "element": "W-1", "appearance": { "shown": false } });
    stack.edit("view.override.set", args).await.unwrap();
    let unknown = json!({ "template": "T-1", "included": { "paper": true } });
    assert!(
        stack
            .edit("view.template.set", unknown)
            .await
            .unwrap_err()
            .contains("is not a setting")
    );
}

#[tokio::test]
async fn a_template_applied_once_copies_and_the_view_is_free_afterwards() {
    let stack = setup().await;
    make_template(&stack).await;
    stack
        .edit("view.create", plan("V-c", "Free plan", "L-g"))
        .await
        .unwrap();
    let args = json!({ "view": "V-c", "template": "T-1" });
    stack.edit("view.template.apply", args).await.unwrap();
    let copied = view(&stack, "V-c").await;
    assert_eq!(copied.template(), None);
    assert_eq!(copied.scale().denominator(), 50);
    assert!(copied.graphics().style(cat(WALLS), Edge::Cut).is_some());
    // Later changes to the template do not reach it, and it may change what it copied.
    let args = json!({ "template": "T-1", "scale": 10 });
    stack.edit("view.template.set", args).await.unwrap();
    assert_eq!(view(&stack, "V-c").await.scale().denominator(), 50);
    stack
        .edit("view.set", json!({ "view": "V-c", "scale": 200 }))
        .await
        .unwrap();
    // A view that follows a template is not applied another; a drafting view takes none.
    tie(&stack, "V-b").await;
    let again = json!({ "view": "V-b", "template": "T-1" });
    let refusal = stack.edit("view.template.apply", again).await.unwrap_err();
    assert!(
        refusal.contains("untie it before applying another"),
        "{refusal}"
    );
    for cmd in ["view.template.apply", "view.template.tie"] {
        let args = json!({ "view": "V-d", "template": "T-1" });
        let refusal = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(
            refusal,
            "A drafting view shows no model, so a view template does not apply to it."
        );
    }
}

#[tokio::test]
async fn untying_keeps_what_the_view_showed_and_a_template_in_use_is_not_deleted() {
    let stack = setup().await;
    make_template(&stack).await;
    tie(&stack, "V-b").await;
    tie(&stack, "V-s").await;
    stack
        .edit(
            "view.template.set",
            json!({ "template": "T-1", "scale": 25, "detail": "coarse" }),
        )
        .await
        .unwrap();
    let delete = json!({ "template": "T-1" });
    assert_eq!(
        stack
            .edit("view.template.delete", delete.clone())
            .await
            .unwrap_err(),
        "The view template Working is in use by 2 views: untie them first."
    );
    let showing = shown(&stack, "V-b").await.settings();
    stack
        .edit("view.template.untie", json!({ "view": "V-b" }))
        .await
        .unwrap();
    let after = view(&stack, "V-b").await;
    assert_eq!(after.template(), None);
    assert_eq!(after.settings(), showing, "it keeps what it showed");
    assert_eq!(shown(&stack, "V-b").await.settings(), showing);
    assert_eq!(
        stack
            .edit("view.template.untie", json!({ "view": "V-b" }))
            .await
            .unwrap_err(),
        "Level 2 plan is not tied to a view template."
    );
    assert_eq!(
        stack
            .edit("view.template.delete", delete.clone())
            .await
            .unwrap_err(),
        "The view template Working is in use by 1 view: untie it first."
    );
    stack
        .edit("view.template.untie", json!({ "view": "V-s" }))
        .await
        .unwrap();
    stack.edit("view.template.delete", delete).await.unwrap();
    assert!(!stack.exists("T-1").await);
}

#[tokio::test]
async fn templates_are_named_once_and_the_generic_commands_turn_them_back() {
    let stack = setup().await;
    make_template(&stack).await;
    let again = json!({ "template": "T-2", "name": " WORKING ", "source": "V-a" });
    assert_eq!(
        stack.edit("view.template.create", again).await.unwrap_err(),
        "There is already a view template named \"WORKING\" in this model."
    );
    let same = json!({ "template": "T-1", "name": "Other" });
    assert_eq!(
        stack.edit("view.template.create", same).await.unwrap_err(),
        "That element already exists."
    );
    let none = json!({ "template": "T-2", "name": "Fresh" });
    stack.edit("view.template.create", none).await.unwrap();
    let fresh = shown_template(&stack, "T-2").await;
    assert_eq!((fresh.scale, fresh.detail.as_str()), (100, "medium"));
    stack
        .edit(
            "view.template.rename",
            json!({ "template": "T-2", "name": "Fresh start" }),
        )
        .await
        .unwrap();
    let clash = json!({ "template": "T-2", "name": "working" });
    assert!(
        stack
            .edit("view.template.rename", clash)
            .await
            .unwrap_err()
            .contains("already a view template named")
    );
    for (cmd, args) in [
        ("element.delete", json!({ "element": "T-1" })),
        ("element.set", json!({ "element": "T-1", "name": "x" })),
        (
            "element.create",
            json!({ "element": "T-3", "kind": "view_template" }),
        ),
    ] {
        let refusal = stack.edit(cmd, args).await.unwrap_err();
        assert!(refusal.contains("view.template"), "{cmd}: {refusal}");
    }
    // Arguments a command does not take are refused by name, and a view or a template, not both.
    let stray = json!({ "view": "V-a", "name": "x", "template": "T-1" });
    assert_eq!(
        stack.edit("view.rename", stray).await.unwrap_err(),
        "view.rename has no template."
    );
    let override_on_template = json!({ "view": "V-a", "template": "T-1", "element": "W-1",
                                       "appearance": { "shown": false } });
    assert_eq!(
        stack
            .edit("view.override.set", override_on_template)
            .await
            .unwrap_err(),
        "view.override.set has no template."
    );
    let both = json!({ "view": "V-a", "template": "T-1", "category": WALLS, "shown": false });
    assert_eq!(
        stack.edit("view.categories.set", both).await.unwrap_err(),
        "Name a view or a view template, not both."
    );
    let neither = json!({ "category": WALLS, "shown": false });
    assert_eq!(
        stack
            .edit("view.categories.set", neither)
            .await
            .unwrap_err(),
        "This command needs a view or a view template."
    );
    let missing = json!({ "template": "T-9", "scale": 20 });
    assert_eq!(
        stack.edit("view.template.set", missing).await.unwrap_err(),
        "There is no view template T-9 in this model."
    );
}

async fn shown_template(
    stack: &Stack,
    id: &str,
) -> construct_api::cloud::view_templates::TemplateSummary {
    let world = world(stack).await;
    construct_api::cloud::view_templates::template_summaries(&world)
        .into_iter()
        .find(|summary| summary.id == id)
        .unwrap()
}

#[tokio::test]
async fn a_template_takes_the_settings_of_a_view_and_lists_its_graphics_and_categories() {
    let stack = setup().await;
    make_template(&stack).await;
    stack
        .edit("view.set", json!({ "view": "V-a", "detail": "fine" }))
        .await
        .unwrap();
    stack
        .edit(
            "view.categories.set",
            json!({ "view": "V-a", "category": DOORS, "shown": false }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "view.template.update",
            json!({ "template": "T-1", "source": "V-a" }),
        )
        .await
        .unwrap();
    let summary = shown_template(&stack, "T-1").await;
    assert_eq!(summary.detail, "fine");
    assert_eq!(summary.hidden, [DOORS]);
    assert_eq!(summary.rules, 1);
    // Taking it again changes nothing and is no step.
    let before = stack.rows("piece").await;
    stack
        .edit(
            "view.template.update",
            json!({ "template": "T-1", "source": "V-a" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.rows("piece").await, before);
    // The template's graphics list as a view's do, with the doors hidden under it.
    let world = world(&stack).await;
    let listing = template_graphics_of(&world, &i("T-1")).unwrap();
    assert_eq!(listing.template.name, "Working");
    assert_eq!(listing.hidden, [DOORS]);
    let walls = listing
        .looks
        .iter()
        .find(|look| look.category == WALLS)
        .unwrap();
    assert!(walls.cut.as_ref().unwrap().style.halftone);
    assert_eq!(walls.cut.as_ref().unwrap().origin, "category");
    let doors = listing
        .looks
        .iter()
        .find(|look| look.category == DOORS)
        .unwrap();
    assert!(!doors.projection.as_ref().unwrap().style.shown);
    // A category that is not there, and the views category, are refused.
    for (category, words) in [(31, "There is no category 31."), (7000, "Category 7000")] {
        let args = json!({ "template": "T-1", "category": category, "shown": false });
        let refusal = stack.edit("view.categories.set", args).await.unwrap_err();
        assert!(refusal.starts_with(words), "{refusal}");
    }
    // A template holds a drawing rule as a view does, and refuses what a view would.
    let bad = json!({ "template": "T-1", "category": WALLS, "edge": "cut",
                      "style": { "pen": 9999 } });
    assert!(
        stack
            .edit("view.graphics.set", bad)
            .await
            .unwrap_err()
            .contains("There is no pen 9999")
    );
}

#[tokio::test]
async fn an_override_is_set_merged_cleared_and_belongs_to_one_view() {
    let stack = setup().await;
    let set = json!({ "view": "V-a", "element": "W-1",
                      "appearance": { "colour": [1, 2, 3], "shown": false } });
    stack.edit("view.override.set", set).await.unwrap();
    let merge = json!({ "view": "V-a", "element": "W-1", "appearance": { "halftone": true } });
    stack.edit("view.override.set", merge).await.unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let listing = overrides_of(&mut snapshot, &i("p"), &i(DOCUMENT), &i("V-a"))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(listing.overrides.len(), 1);
    let one = &listing.overrides[0];
    assert_eq!(one.element, "W-1");
    assert_eq!(one.look.shown, Some(false));
    assert_eq!(one.look.halftone, Some(true));
    assert_eq!(one.look.colour.map(|c| c.0), Some(1));
    // Another view of the same element is not touched.
    let other = overrides_of(&mut snapshot, &i("p"), &i(DOCUMENT), &i("V-b"))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(other.overrides.len(), 0);
    // Two elements, then one clearing command per element, then all.
    let second = json!({ "view": "V-a", "element": "W-2", "appearance": { "pattern": 2 } });
    stack.edit("view.override.set", second).await.unwrap();
    stack
        .edit(
            "view.override.clear",
            json!({ "view": "V-a", "element": "W-1" }),
        )
        .await
        .unwrap();
    assert_eq!(view(&stack, "V-a").await.overrides().iter().count(), 1);
    let before = stack.rows("piece").await;
    // Clearing what is not there writes nothing.
    stack
        .edit(
            "view.override.clear",
            json!({ "view": "V-a", "element": "W-1" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.rows("piece").await, before);
    stack
        .edit("view.override.clear", json!({ "view": "V-a" }))
        .await
        .unwrap();
    assert!(view(&stack, "V-a").await.overrides().is_empty());
    assert!(
        !stack
            .record("V-a")
            .await
            .props
            .contains_key("@view.overrides")
    );
}

#[tokio::test]
async fn an_override_that_cannot_stand_is_refused_in_words() {
    let stack = setup().await;
    let with = |view: &str, element: &str, look: Value| json!({ "view": view, "element": element, "appearance": look });
    for (args, words) in [
        (
            with("V-a", "W-9", json!({ "shown": false })),
            "There is no element W-9 in this model.",
        ),
        (
            with("V-a", "V-b", json!({ "shown": false })),
            "Level 2 plan is not something a view draws, so it has no override.",
        ),
        (
            with("V-a", "W-1", json!({})),
            "A look names at least one of pen, pattern, colour, halftone and shown.",
        ),
        (
            with("V-a", "W-1", json!({ "pen": 9999 })),
            "There is no pen 9999 for this category.",
        ),
        (
            with("V-a", "W-1", json!({ "pattern": 777 })),
            "There is no line pattern 777.",
        ),
        (
            with("V-a", "W-1", json!({ "colour": [1, 2] })),
            "A colour is [red, green, blue]",
        ),
        (
            with("V-d", "W-1", json!({ "shown": false })),
            "A drafting view shows no model, so it has no overrides or filters.",
        ),
        (
            json!({ "view": "V-a", "appearance": { "shown": false } }),
            "This command needs an element.",
        ),
    ] {
        let refusal = stack.edit("view.override.set", args).await.unwrap_err();
        assert!(refusal.starts_with(words), "{words}: {refusal}");
    }
}

/// A text parameter "Fire rating" on walls, and the two walls' values.
async fn fire_ratings(stack: &Stack) {
    let definition = json!({ "definition": { "id": "P-fire", "name": "Fire rating",
                                             "kind": "text", "holder": "instance",
                                             "choices": [] } });
    stack.edit("parameter.define", definition).await.unwrap();
    let bind = json!({ "parameter": "P-fire", "categories": [WALLS] });
    stack.edit("parameter.bind", bind).await.unwrap();
    for (element, value) in [("W-1", "1h"), ("W-2", "2h")] {
        let args = json!({ "element": element, "parameter": "P-fire", "value": value });
        stack.edit("element.param", args).await.unwrap();
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "a test helper called with json! literals"
)]
fn filter(name: &str, op: &str, value: &str, look: Value) -> Value {
    json!({ "view": "V-a", "name": name, "category": WALLS, "parameter": "Fire rating",
            "op": op, "value": value, "appearance": look })
}

/// What the view V-a gives each wall of its own, as the drawing asks.
async fn styles(stack: &Stack, view: &str) -> BTreeMap<Id, construct_views::ElementStyle> {
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let world = read_world(&mut snapshot, &i("p"), &i(DOCUMENT))
        .await
        .unwrap()
        .unwrap();
    let records = [stack.record("W-1").await, stack.record("W-2").await];
    element_styles(
        &mut snapshot,
        &i("p"),
        &i(DOCUMENT),
        &world,
        &i(view),
        &records,
    )
    .await
    .unwrap()
    .unwrap()
}

fn colour(styles: &BTreeMap<Id, construct_views::ElementStyle>, element: &str) -> Option<u8> {
    styles
        .get(&i(element))
        .and_then(|style| style.colour)
        .map(|c| c.0)
}

#[tokio::test]
async fn two_filters_decide_in_order_and_are_asked_again_when_a_parameter_changes() {
    let stack = setup().await;
    fire_ratings(&stack).await;
    stack
        .edit(
            "view.filter.add",
            filter("One hour", "equals", "1h", json!({ "colour": [255, 0, 0] })),
        )
        .await
        .unwrap();
    stack
        .edit(
            "view.filter.add",
            filter(
                "Rated",
                "contains",
                "H",
                json!({ "colour": [0, 0, 255], "halftone": true }),
            ),
        )
        .await
        .unwrap();
    let found = styles(&stack, "V-a").await;
    // Both match the 1h wall: the first wins the colour, the second adds the halftone it alone names.
    assert_eq!(colour(&found, "W-1"), Some(255));
    assert_eq!(found[&i("W-1")].halftone, Some(true));
    // Only the second matches the 2h wall.
    assert_eq!(colour(&found, "W-2"), Some(0));

    // Changing a parameter changes the answer at once.
    let rate = |element: &str, value: &str| json!({ "element": element, "parameter": "P-fire", "value": value });
    stack
        .edit("element.param", rate("W-1", "3h"))
        .await
        .unwrap();
    assert_eq!(colour(&styles(&stack, "V-a").await, "W-1"), Some(0));
    stack
        .edit("element.param", rate("W-1", "1h"))
        .await
        .unwrap();
    assert_eq!(colour(&styles(&stack, "V-a").await, "W-1"), Some(255));

    // The order decides: moving "Rated" to the top gives it the colour of both.
    stack
        .edit(
            "view.filter.move",
            json!({ "view": "V-a", "filter": 2, "place": 0 }),
        )
        .await
        .unwrap();
    assert_eq!(colour(&styles(&stack, "V-a").await, "W-1"), Some(0));
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let listing = filters_of(&mut snapshot, &i("p"), &i(DOCUMENT), &i("V-a"))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let names: Vec<&str> = listing.filters.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Rated", "One hour"]);
    assert_eq!(listing.filters[0].parameter_name, "Fire rating");
    assert_eq!(listing.filters[0].category_name, "Walls");
    assert_eq!(listing.of, "view");
    let fire = listing
        .parameters
        .iter()
        .find(|p| p.id == "P-fire")
        .unwrap();
    assert_eq!(fire.kind, "text");
    assert_eq!(fire.operators, ["equals", "contains"]);

    // A filter changed in place keeps its place; one taken away stops matching.
    stack
        .edit(
            "view.filter.set",
            json!({ "view": "V-a", "filter": 1, "value": "2h" }),
        )
        .await
        .unwrap();
    // "Rated" is still first and gives W-2 its colour; the filter's place did not change.
    assert_eq!(colour(&styles(&stack, "V-a").await, "W-2"), Some(0));
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let listing = filters_of(&mut snapshot, &i("p"), &i(DOCUMENT), &i("V-a"))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(listing.filters[1].value, "2h");
    assert_eq!(listing.filters[1].name, "One hour");
    stack
        .edit(
            "view.filter.move",
            json!({ "view": "V-a", "filter": 1, "place": 0 }),
        )
        .await
        .unwrap();
    assert_eq!(colour(&styles(&stack, "V-a").await, "W-2"), Some(255));
    stack
        .edit("view.filter.remove", json!({ "view": "V-a", "filter": 1 }))
        .await
        .unwrap();
    let found = styles(&stack, "V-a").await;
    assert_eq!(colour(&found, "W-2"), Some(0));
    // An override beats a filter on the parts it names.
    let own = json!({ "view": "V-a", "element": "W-2", "appearance": { "colour": [9, 9, 9] } });
    stack.edit("view.override.set", own).await.unwrap();
    let found = styles(&stack, "V-a").await;
    assert_eq!(colour(&found, "W-2"), Some(9));
    assert_eq!(found[&i("W-2")].halftone, Some(true));
}

#[tokio::test]
async fn a_filter_that_cannot_stand_is_refused_in_words() {
    let stack = setup().await;
    fire_ratings(&stack).await;
    stack
        .edit(
            "view.filter.add",
            filter("One hour", "equals", "1h", json!({ "colour": [255, 0, 0] })),
        )
        .await
        .unwrap();
    for (args, words) in [
        (
            filter("Over", "greater_than", "1", json!({})),
            "A filter cannot compare the parameter Fire rating with greater_than",
        ),
        (
            filter("Odd", "like", "1", json!({})),
            "\"like\" is not an operator: equals, contains, greater_than.",
        ),
        (
            filter(" ", "equals", "1", json!({})),
            "A filter needs a name.",
        ),
        (
            filter("one HOUR", "equals", "1", json!({})),
            "There is already a filter named \"one HOUR\" here.",
        ),
        (
            json!({ "view": "V-a", "name": "X", "category": WALLS, "parameter": "Nothing",
                    "op": "equals", "value": "1" }),
            "\"Nothing\" is not a parameter of this category.",
        ),
        (
            json!({ "view": "V-a", "name": "X", "category": DOORS, "parameter": "Fire rating",
                    "op": "equals", "value": "1" }),
            "\"Fire rating\" is not a parameter of this category.",
        ),
        (
            json!({ "view": "V-a", "name": "X", "category": 31, "parameter": "Fire rating",
                    "op": "equals", "value": "1" }),
            "There is no category 31.",
        ),
        (
            json!({ "view": "V-a", "name": "X", "category": WALLS, "parameter": "Fire rating",
                    "op": "equals" }),
            "This command needs a value.",
        ),
        (
            filter("Bad pen", "equals", "1", json!({ "pen": 9999 })),
            "There is no pen 9999 for this category.",
        ),
        (
            json!({ "view": "V-d", "name": "X", "category": WALLS, "parameter": "Fire rating",
                    "op": "equals", "value": "1" }),
            "A drafting view shows no model, so it has no overrides or filters.",
        ),
        (
            json!({ "view": "V-a", "filter": 7, "name": "X" }),
            "There is no filter 7 here.",
        ),
    ] {
        let cmd = if args.get("filter").is_some() {
            "view.filter.set"
        } else {
            "view.filter.add"
        };
        let refusal = stack.edit(cmd, args).await.unwrap_err();
        assert!(refusal.starts_with(words), "{words}: {refusal}");
    }
    let off = json!({ "view": "V-a", "filter": 1, "place": 4 });
    assert_eq!(
        stack.edit("view.filter.move", off).await.unwrap_err(),
        "There is no place 4 in a list of 1 filter."
    );
    let gone = json!({ "view": "V-a", "filter": 5 });
    assert_eq!(
        stack.edit("view.filter.remove", gone).await.unwrap_err(),
        "There is no filter 5 here."
    );
}

#[tokio::test]
async fn filters_in_a_template_reach_the_views_tied_to_it() {
    let stack = setup().await;
    fire_ratings(&stack).await;
    make_template(&stack).await;
    tie(&stack, "V-b").await;
    let add = json!({ "template": "T-1", "name": "Two hours", "category": WALLS,
                      "parameter": "P-fire", "op": "equals", "value": "2h",
                      "appearance": { "colour": [7, 7, 7], "shown": false } });
    stack.edit("view.filter.add", add).await.unwrap();
    // The tied view asks the template's filters; the untied one has none.
    let tied = styles(&stack, "V-b").await;
    assert_eq!(colour(&tied, "W-2"), Some(7));
    assert_eq!(tied[&i("W-2")].shown, Some(false));
    assert!(!tied.contains_key(&i("W-1")));
    assert!(styles(&stack, "V-a").await.is_empty());
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let listing = filters_of(&mut snapshot, &i("p"), &i(DOCUMENT), &i("V-b"))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(listing.follows_template.as_deref(), Some("Working"));
    assert_eq!(listing.filters.len(), 1);
    let of_template = filters_of(&mut snapshot, &i("p"), &i(DOCUMENT), &i("T-1"))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(of_template.of, "template");
    assert_eq!(of_template.follows_template, None);
}

#[tokio::test]
async fn every_template_override_and_filter_command_is_undone_and_redone() {
    let stack = setup().await;
    fire_ratings(&stack).await;
    let create = json!({ "template": "T-1", "name": "Working", "source": "V-a" });
    undone_and_redone(&stack, "view.template.create", create).await;
    for (cmd, args) in [
        (
            "view.template.tie",
            json!({ "view": "V-b", "template": "T-1" }),
        ),
        (
            "view.template.rename",
            json!({ "template": "T-1", "name": "Working plan" }),
        ),
        (
            "view.template.set",
            json!({ "template": "T-1", "scale": 20, "included": { "detail": false } }),
        ),
        (
            "view.graphics.set",
            json!({ "template": "T-1", "category": DOORS, "edge": "projection",
                    "style": { "shown": false } }),
        ),
        (
            "view.graphics.clear",
            json!({ "template": "T-1", "category": DOORS }),
        ),
        (
            "view.categories.set",
            json!({ "template": "T-1", "category": DOORS, "shown": false }),
        ),
        (
            "view.filter.add",
            json!({ "template": "T-1", "name": "F", "category": WALLS,
                    "parameter": "P-fire", "op": "equals", "value": "1h" }),
        ),
        (
            "view.filter.set",
            json!({ "template": "T-1", "filter": 1, "value": "2h" }),
        ),
        (
            "view.filter.add",
            json!({ "template": "T-1", "name": "G", "category": WALLS,
                    "parameter": "P-fire", "op": "contains", "value": "h" }),
        ),
        (
            "view.filter.move",
            json!({ "template": "T-1", "filter": 2, "place": 0 }),
        ),
        (
            "view.filter.remove",
            json!({ "template": "T-1", "filter": 2 }),
        ),
        (
            "view.override.set",
            json!({ "view": "V-b", "element": "W-1", "appearance": { "shown": false } }),
        ),
        ("view.override.clear", json!({ "view": "V-b" })),
        ("view.template.untie", json!({ "view": "V-b" })),
        (
            "view.template.apply",
            json!({ "view": "V-s", "template": "T-1" }),
        ),
        (
            "view.template.update",
            json!({ "template": "T-1", "source": "V-a" }),
        ),
        ("view.template.delete", json!({ "template": "T-1" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn an_undo_may_not_leave_a_view_tied_to_a_template_the_model_lacks() {
    let stack = setup().await;
    make_template(&stack).await;
    stack.later();
    stack
        .edit_as(
            "ben",
            "view.template.tie",
            json!({ "view": "V-b", "template": "T-1" }),
        )
        .await
        .unwrap();
    // Anna made the template; undoing her making it would leave Ben's view tied to nothing.
    stack.later();
    let refusal = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert!(
        refusal.contains("is tied to the view template T-1"),
        "{refusal}"
    );
    assert!(stack.exists("T-1").await);
}

#[tokio::test]
async fn another_persons_claim_on_a_template_refuses_the_edit() {
    let stack = setup().await;
    make_template(&stack).await;
    let args = json!({ "template": "T-1", "scale": 20 });
    assert_eq!(
        stack
            .edit_as("ben", "view.template.set", args.clone())
            .await
            .unwrap_err(),
        "anna is working on this."
    );
    stack.later();
    stack
        .edit_as("ben", "view.template.set", args)
        .await
        .unwrap();
}

#[tokio::test]
async fn templates_overrides_and_filters_replay_from_the_log_to_the_same_bytes() {
    let stack = setup().await;
    fire_ratings(&stack).await;
    for (cmd, args) in [
        (
            "view.template.create",
            json!({ "template": "T-1", "name": "Working", "source": "V-a" }),
        ),
        (
            "view.template.tie",
            json!({ "view": "V-b", "template": "T-1" }),
        ),
        (
            "view.template.set",
            json!({ "template": "T-1", "scale": 20 }),
        ),
        (
            "view.categories.set",
            json!({ "template": "T-1", "category": DOORS, "shown": false }),
        ),
        (
            "view.filter.add",
            json!({ "template": "T-1", "name": "F", "category": WALLS,
                    "parameter": "P-fire", "op": "equals", "value": "1h",
                    "appearance": { "colour": [1, 2, 3] } }),
        ),
        (
            "view.override.set",
            json!({ "view": "V-b", "element": "W-1", "appearance": { "halftone": true } }),
        ),
        (
            "view.template.apply",
            json!({ "view": "V-s", "template": "T-1" }),
        ),
        ("view.template.untie", json!({ "view": "V-b" })),
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
    assert!(
        entries
            .iter()
            .any(|entry| entry.cmd == "view.template.create")
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
    for table in ["record", "piece"] {
        assert_eq!(again.rows(table).await, stack.rows(table).await, "{table}");
    }
}
