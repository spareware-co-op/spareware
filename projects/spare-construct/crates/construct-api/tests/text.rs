//! Text styles, notes and leaders through the edit log, on the test store: every command with undo
//! and redo, every refusal by name, a style change reaching its notes, the generic element commands
//! refused for them, a view that holds notes kept, undos turned back that would leave a note on
//! nothing, claims, the laid-out listing with exact numbers, and the log replayed from empty.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::text::{World, listing_of_view, read_world};
use construct_api::cloud::undo::{self, CANNOT, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_params::Length;
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_text::{Arrow, Face, FixedWidth, FontMetrics, TextError, TextNote, TextStyle};
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

fn drafting(id: &str, name: &str) -> Value {
    json!({ "view": id, "kind": "drafting", "name": name })
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

fn style(id: &str, name: &str) -> Value {
    json!({ "style": id, "name": name, "font": "Test Sans", "size": "5" })
}

fn note(id: &str, view: &str, style: &str, text: &str) -> Value {
    json!({ "note": id, "view": view, "style": style, "text": text, "x": "100", "y": "200" })
}

async fn the_note(stack: &Stack, id: &str) -> TextNote {
    TextNote::from_record(&stack.record(id).await).unwrap()
}

async fn the_style(stack: &Stack, id: &str) -> TextStyle {
    TextStyle::from_record(&stack.record(id).await).unwrap()
}

/// Two drafting views, a style ST-1 and a note N-1 in V-1.
async fn with_note() -> Stack {
    let stack = Stack::new().await;
    for (id, name) in [("V-1", "Detail 1"), ("V-2", "Detail 2")] {
        stack.edit("view.create", drafting(id, name)).await.unwrap();
    }
    stack
        .edit("text.style.create", style("ST-1", "General"))
        .await
        .unwrap();
    stack
        .edit("text.note.create", note("N-1", "V-1", "ST-1", "Hello"))
        .await
        .unwrap();
    stack
}

#[tokio::test]
async fn a_style_is_made_with_the_defaults_and_changed_by_set() {
    let stack = Stack::new().await;
    stack
        .edit("text.style.create", style("ST-1", "  General "))
        .await
        .unwrap();
    let made = the_style(&stack, "ST-1").await;
    assert_eq!(made.name(), "General");
    assert_eq!(made.font(), "Test Sans");
    assert_eq!(made.size(), mm(5));
    assert_eq!(
        (made.bold(), made.italic(), made.underline()),
        (false, false, false)
    );
    assert_eq!((made.colour(), made.spacing()), ([0, 0, 0], 100));
    assert_eq!(stack.record("ST-1").await.props["@category"], "5200");

    let full = json!({ "style": "ST-2", "name": "Heading", "font": "Other", "size": "7.5 mm",
        "bold": true, "italic": true, "underline": true, "colour": [200, 16, 0], "spacing": 120 });
    stack.edit("text.style.create", full).await.unwrap();
    let heading = the_style(&stack, "ST-2").await;
    assert_eq!(heading.size().steps(), 48_000);
    assert_eq!(heading.colour(), [200, 16, 0]);
    assert!(heading.bold() && heading.italic() && heading.underline());

    stack
        .edit(
            "text.style.set",
            json!({ "style": "ST-1", "name": "general", "size": "6", "bold": true,
                    "font": "Sans", "colour": [1, 2, 3], "spacing": 150 }),
        )
        .await
        .unwrap();
    let set = the_style(&stack, "ST-1").await;
    assert_eq!(set.name(), "general");
    assert_eq!((set.size(), set.bold(), set.font()), (mm(6), true, "Sans"));
    // A set that changes nothing writes nothing and makes no step.
    let before = stack.state("anna").await;
    stack
        .edit("text.style.set", json!({ "style": "ST-1", "size": "6 mm" }))
        .await
        .unwrap();
    assert_eq!(stack.state("anna").await, before);
}

#[tokio::test]
async fn every_style_refusal_is_by_name() {
    let stack = Stack::new().await;
    stack
        .edit("text.style.create", style("ST-1", "General"))
        .await
        .unwrap();
    let refused = |args: Value| {
        let stack = &stack;
        async move { stack.edit("text.style.create", args).await.unwrap_err() }
    };
    let with = |key: &str, value: Value| {
        let mut args = style("ST-9", "Nine");
        args[key] = value;
        args
    };
    assert_eq!(
        refused(with("name", json!("   "))).await,
        TextError::EmptyName.to_string()
    );
    assert_eq!(
        refused(with("name", json!(" GENERAL "))).await,
        TextError::DuplicateName("GENERAL".to_owned()).to_string()
    );
    for size in ["0", "-3", "501"] {
        let why = refused(with("size", json!(size))).await;
        assert!(
            why.contains("A text size on paper is above zero"),
            "{size}: {why}"
        );
    }
    assert!(refused(with("size", json!("tall"))).await.contains("tall"));
    assert!(
        refused(with("font", json!("")))
            .await
            .contains("A font name")
    );
    assert!(
        refused(with("colour", json!([1, 2])))
            .await
            .contains("A colour is")
    );
    assert!(
        refused(with("colour", json!([1, 2, 256])))
            .await
            .contains("A colour is")
    );
    assert!(
        refused(with("spacing", json!(10)))
            .await
            .contains("line spacing")
    );
    assert!(
        refused(with("bogus", json!(1)))
            .await
            .contains("isn't valid")
    );
    assert_eq!(
        refused(style("ST-1", "Other")).await,
        "That element already exists."
    );
    let mut nameless = style("ST-9", "x");
    nameless.as_object_mut().unwrap().remove("size");
    assert_eq!(refused(nameless).await, "This command needs a size.");
    // And in a set.
    for (args, words) in [
        (
            json!({ "style": "ST-9", "name": "x" }),
            "There is no text style ST-9",
        ),
        (
            json!({ "style": "ST-1", "size": "0" }),
            "A text size on paper",
        ),
        (json!({}), "needs a style"),
    ] {
        let why = stack.edit("text.style.set", args).await.unwrap_err();
        assert!(why.contains(words), "{why}");
    }
    stack
        .edit("text.style.create", style("ST-2", "Heading"))
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit(
                "text.style.set",
                json!({ "style": "ST-2", "name": "general" })
            )
            .await
            .unwrap_err(),
        TextError::DuplicateName("general".to_owned()).to_string()
    );
}

#[tokio::test]
async fn a_note_is_made_with_the_defaults_and_every_refusal_is_by_name() {
    let stack = with_note().await;
    let made = the_note(&stack, "N-1").await;
    assert_eq!((made.x(), made.y()), (mm(100), mm(200)));
    assert_eq!(made.lines(), ["Hello"]);
    assert_eq!(made.width(), None);
    assert_eq!(stack.record("N-1").await.props["@category"], "5200");
    let full = json!({ "note": "N-2", "view": "V-2", "style": "ST-1", "text": "a\nb",
        "x": "-5.5 mm", "y": "0", "width": "40", "align": "right", "list": "number",
        "rotation": "90" });
    stack.edit("text.note.create", full).await.unwrap();
    let wide = the_note(&stack, "N-2").await;
    assert_eq!(wide.lines(), ["a", "b"]);
    assert_eq!(wide.width(), Some(mm(40)));
    assert_eq!(wide.rotation().units(), 90 * 9_000_000);
    assert_eq!(wide.x().steps(), -35_200);

    let refused = |args: Value| {
        let stack = &stack;
        async move { stack.edit("text.note.create", args).await.unwrap_err() }
    };
    let with = |key: &str, value: Value| {
        let mut args = note("N-9", "V-1", "ST-1", "x");
        args[key] = value;
        args
    };
    assert_eq!(
        refused(with("view", json!("V-9"))).await,
        TextError::UnknownView(i("V-9")).to_string()
    );
    assert_eq!(
        refused(with("view", json!("ST-1"))).await,
        TextError::UnknownView(i("ST-1")).to_string()
    );
    assert_eq!(
        refused(with("style", json!("ST-9"))).await,
        TextError::UnknownStyle(i("ST-9")).to_string()
    );
    assert!(
        refused(with("text", json!("  ")))
            .await
            .contains("needs some text")
    );
    for width in ["0", "-4"] {
        assert!(
            refused(with("width", json!(width)))
                .await
                .contains("A wrap width is above zero")
        );
    }
    assert!(
        refused(with("align", json!("middle")))
            .await
            .contains("is not an alignment")
    );
    assert!(
        refused(with("list", json!("roman")))
            .await
            .contains("is not a list kind")
    );
    assert!(
        refused(with("rotation", json!("400")))
            .await
            .contains("one turn")
    );
    assert!(refused(with("x", json!("far"))).await.contains("far"));
    assert_eq!(
        refused(note("N-1", "V-1", "ST-1", "again")).await,
        "That element already exists."
    );
}

#[tokio::test]
async fn a_note_changes_by_its_own_commands() {
    let stack = with_note().await;
    stack
        .edit("text.style.create", style("ST-2", "Heading"))
        .await
        .unwrap();
    stack
        .edit(
            "text.note.text",
            json!({ "note": "N-1", "text": "One\nTwo" }),
        )
        .await
        .unwrap();
    assert_eq!(the_note(&stack, "N-1").await.lines(), ["One", "Two"]);
    stack
        .edit("text.note.move", json!({ "note": "N-1", "x": "12.5 mm" }))
        .await
        .unwrap();
    let moved = the_note(&stack, "N-1").await;
    assert_eq!((moved.x().steps(), moved.y()), (80_000, mm(200)));
    stack
        .edit("text.note.style", json!({ "note": "N-1", "style": "ST-2" }))
        .await
        .unwrap();
    assert_eq!(the_note(&stack, "N-1").await.style(), &i("ST-2"));
    stack
        .edit(
            "text.note.layout",
            json!({ "note": "N-1", "width": "60", "align": "centre", "list": "bullet",
                    "rotation": "-22.5" }),
        )
        .await
        .unwrap();
    let laid = the_note(&stack, "N-1").await;
    assert_eq!(laid.width(), Some(mm(60)));
    assert_eq!(laid.rotation().units(), -202_500_000);
    stack
        .edit("text.note.layout", json!({ "note": "N-1", "width": null }))
        .await
        .unwrap();
    assert_eq!(the_note(&stack, "N-1").await.width(), None);

    for (cmd, args, words) in [
        (
            "text.note.text",
            json!({ "note": "N-1", "text": "" }),
            "needs some text",
        ),
        ("text.note.text", json!({ "note": "N-1" }), "needs text"),
        (
            "text.note.move",
            json!({ "note": "N-1" }),
            "needs x, y or both",
        ),
        (
            "text.note.style",
            json!({ "note": "N-1", "style": "ST-9" }),
            "no text style ST-9",
        ),
        (
            "text.note.layout",
            json!({ "note": "N-1" }),
            "needs a width",
        ),
        (
            "text.note.layout",
            json!({ "note": "N-1", "width": "0" }),
            "above zero",
        ),
        (
            "text.note.text",
            json!({ "note": "N-9", "text": "x" }),
            "no text note N-9",
        ),
        (
            "text.note.text",
            json!({ "note": "ST-1", "text": "x" }),
            "no text note ST-1",
        ),
        (
            "text.note.delete",
            json!({ "note": "N-9" }),
            "no text note N-9",
        ),
    ] {
        let why = stack.edit(cmd, args).await.unwrap_err();
        assert!(why.contains(words), "{cmd}: {why}");
    }
}

#[tokio::test]
async fn leaders_are_added_moved_and_removed_and_refused_by_name() {
    let stack = with_note().await;
    stack
        .edit(
            "text.leader.add",
            json!({ "note": "N-1", "points": [["0", "0"], ["10", "10"], ["10", "25.5 mm"]],
                    "arrow": "dot" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "text.leader.add",
            json!({ "note": "N-1", "points": [["1", "2"], ["3", "4"]] }),
        )
        .await
        .unwrap();
    let held = the_note(&stack, "N-1").await;
    assert_eq!(held.leaders().len(), 2);
    assert_eq!(held.leaders()[&1].arrow(), Arrow::Dot);
    assert_eq!(held.leaders()[&1].points().len(), 3);
    assert_eq!(held.leaders()[&1].points()[2].1.steps(), 163_200);
    assert_eq!(held.leaders()[&2].arrow(), Arrow::Filled);
    stack
        .edit(
            "text.leader.move",
            json!({ "note": "N-1", "leader": 2, "points": [["5", "5"], ["6", "6"]] }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "text.leader.move",
            json!({ "note": "N-1", "leader": 2, "arrow": "slash" }),
        )
        .await
        .unwrap();
    let moved = the_note(&stack, "N-1").await;
    assert_eq!(moved.leaders()[&2].arrow(), Arrow::Slash);
    assert_eq!(moved.leaders()[&2].points()[0], (mm(5), mm(5)));
    stack
        .edit("text.leader.remove", json!({ "note": "N-1", "leader": 1 }))
        .await
        .unwrap();
    assert_eq!(the_note(&stack, "N-1").await.leaders().len(), 1);

    for (cmd, args, words) in [
        (
            "text.leader.add",
            json!({ "note": "N-1", "points": [["0", "0"]] }),
            "A leader has from 2 to 32 points; this one has 1",
        ),
        (
            "text.leader.add",
            json!({ "note": "N-1", "points": [] }),
            "this one has 0",
        ),
        ("text.leader.add", json!({ "note": "N-1" }), "needs points"),
        (
            "text.leader.add",
            json!({ "note": "N-1", "points": [["0", "0"], ["1", "1"]], "arrow": "star" }),
            "is not an arrowhead",
        ),
        (
            "text.leader.add",
            json!({ "note": "N-1", "points": [["0", "0"], ["x", "1"]] }),
            "x",
        ),
        (
            "text.leader.move",
            json!({ "note": "N-1", "leader": 9, "arrow": "dot" }),
            "has no leader 9",
        ),
        (
            "text.leader.move",
            json!({ "note": "N-1", "leader": 2, "points": [["0", "0"]] }),
            "this one has 1",
        ),
        (
            "text.leader.move",
            json!({ "note": "N-1", "leader": 2 }),
            "points, an arrow or both",
        ),
        (
            "text.leader.remove",
            json!({ "note": "N-1", "leader": 1 }),
            "has no leader 1",
        ),
        (
            "text.leader.remove",
            json!({ "note": "N-1" }),
            "needs a leader number",
        ),
    ] {
        let why = stack.edit(cmd, args).await.unwrap_err();
        assert!(why.contains(words), "{cmd}: {why}");
    }
    // A leader's number is one past the largest left, and 33 leaders are too many.
    let more = json!({ "note": "N-1", "points": [["0", "0"], ["1", "1"]] });
    stack.edit("text.leader.add", more.clone()).await.unwrap();
    assert_eq!(
        the_note(&stack, "N-1")
            .await
            .leaders()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [2, 3]
    );
    for _ in 0..30 {
        stack.edit("text.leader.add", more.clone()).await.unwrap();
    }
    let why = stack.edit("text.leader.add", more).await.unwrap_err();
    assert!(why.contains("at most 32 leaders"), "{why}");
}

#[tokio::test]
async fn a_style_in_use_is_refused_naming_how_many_notes_use_it() {
    let stack = with_note().await;
    stack
        .edit("text.note.create", note("N-2", "V-2", "ST-1", "Two"))
        .await
        .unwrap();
    let why = stack
        .edit("text.style.delete", json!({ "style": "ST-1" }))
        .await
        .unwrap_err();
    assert_eq!(
        why,
        TextError::StyleInUse {
            style: i("ST-1"),
            notes: 2
        }
        .to_string()
    );
    assert!(why.contains("used by 2 text notes"));
    stack
        .edit("text.note.delete", json!({ "note": "N-2" }))
        .await
        .unwrap();
    assert!(
        stack
            .edit("text.style.delete", json!({ "style": "ST-1" }))
            .await
            .unwrap_err()
            .contains("used by 1 text note;")
    );
    stack
        .edit("text.note.delete", json!({ "note": "N-1" }))
        .await
        .unwrap();
    stack
        .edit("text.style.delete", json!({ "style": "ST-1" }))
        .await
        .unwrap();
    assert!(!stack.exists("ST-1").await);
    assert!(
        stack
            .edit("text.style.delete", json!({ "style": "ST-1" }))
            .await
            .unwrap_err()
            .contains("no text style ST-1")
    );
}

#[tokio::test]
async fn a_view_that_holds_notes_is_not_deleted() {
    let stack = with_note().await;
    let why = stack
        .edit("view.delete", json!({ "view": "V-1" }))
        .await
        .unwrap_err();
    assert_eq!(
        why,
        TextError::ViewHasNotes {
            view: i("V-1"),
            notes: 1
        }
        .to_string()
    );
    assert!(stack.exists("V-1").await);
    stack
        .edit("text.note.delete", json!({ "note": "N-1" }))
        .await
        .unwrap();
    stack
        .edit("view.delete", json!({ "view": "V-1" }))
        .await
        .unwrap();
    assert!(!stack.exists("V-1").await);
}

#[tokio::test]
async fn the_generic_element_commands_are_not_a_back_door() {
    let stack = with_note().await;
    let recipe =
        |kind: &str| serde_json::to_value(Recipe::construct(kind, BTreeMap::new())).unwrap();
    let made = |id: &str, kind: &str| {
        json!({ "element": id, "kind": kind, "level": 0,
                "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe(kind) })
    };
    for kind in ["text-note", "text-style"] {
        let why = stack
            .edit("element.create", made("E-1", kind))
            .await
            .unwrap_err();
        assert!(
            why.contains("text.note.create or text.style.create"),
            "{why}"
        );
    }
    // A wall carrying a note's recipe is refused too.
    let mut sneaky = wall("W-9");
    sneaky["recipe"] = recipe("text-note");
    assert!(stack.edit("element.create", sneaky).await.is_err());
    for (id, cmd, words) in [
        ("N-1", "element.delete", "text.note.delete"),
        ("N-1", "element.move", "text.note.text"),
        ("N-1", "element.set", "text.leader.*"),
        ("ST-1", "element.delete", "text.style.delete"),
        ("ST-1", "element.set", "text.style.set"),
        ("ST-1", "element.move", "text.style.set"),
    ] {
        let args = json!({ "element": id, "name": "x", "dx": 1, "dy": 1, "dz": 0,
                           "placement": { "x": 1, "y": 1, "z": 0 } });
        let why = stack.edit(cmd, args).await.unwrap_err();
        assert!(why.contains(words), "{cmd} {id}: {why}");
    }
    for id in ["N-1", "ST-1"] {
        assert!(
            stack
                .edit(
                    "element.param",
                    json!({ "element": id, "parameter": "p", "value": "x" })
                )
                .await
                .is_err()
        );
    }
    assert_eq!(the_note(&stack, "N-1").await.lines(), ["Hello"]);
}

#[tokio::test]
async fn another_persons_claim_on_a_note_or_style_refuses_the_edit() {
    let stack = with_note().await;
    assert_eq!(
        stack
            .edit_as(
                "ben",
                "text.note.text",
                json!({ "note": "N-1", "text": "Mine" })
            )
            .await
            .unwrap_err(),
        "anna is working on this."
    );
    assert_eq!(
        stack
            .edit_as(
                "ben",
                "text.style.set",
                json!({ "style": "ST-1", "size": "9" })
            )
            .await
            .unwrap_err(),
        "anna is working on this."
    );
    stack.later();
    stack
        .edit_as(
            "ben",
            "text.note.text",
            json!({ "note": "N-1", "text": "Mine" }),
        )
        .await
        .unwrap();
    assert_eq!(the_note(&stack, "N-1").await.lines(), ["Mine"]);
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
async fn every_text_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    for (id, name) in [("V-1", "Detail 1"), ("V-2", "Detail 2")] {
        stack.edit("view.create", drafting(id, name)).await.unwrap();
    }
    undone_and_redone(&stack, "text.style.create", style("ST-1", "General")).await;
    undone_and_redone(&stack, "text.style.create", style("ST-2", "Heading")).await;
    undone_and_redone(
        &stack,
        "text.note.create",
        note("N-1", "V-1", "ST-1", "Hello"),
    )
    .await;
    for (cmd, args) in [
        (
            "text.style.set",
            json!({ "style": "ST-2", "name": "Big", "size": "10", "bold": true }),
        ),
        (
            "text.note.text",
            json!({ "note": "N-1", "text": "One\nTwo" }),
        ),
        (
            "text.note.move",
            json!({ "note": "N-1", "x": "1", "y": "2" }),
        ),
        ("text.note.style", json!({ "note": "N-1", "style": "ST-2" })),
        (
            "text.note.layout",
            json!({ "note": "N-1", "width": "50", "align": "right", "list": "number",
                    "rotation": "30" }),
        ),
        ("text.note.layout", json!({ "note": "N-1", "width": null })),
        (
            "text.leader.add",
            json!({ "note": "N-1", "points": [["0", "0"], ["4", "4"]], "arrow": "open" }),
        ),
        (
            "text.leader.move",
            json!({ "note": "N-1", "leader": 1, "points": [["0", "0"], ["8", "8"], ["9", "9"]] }),
        ),
        ("text.leader.remove", json!({ "note": "N-1", "leader": 1 })),
        ("text.note.style", json!({ "note": "N-1", "style": "ST-1" })),
        ("text.note.delete", json!({ "note": "N-1" })),
        ("text.style.delete", json!({ "style": "ST-2" })),
        ("text.style.delete", json!({ "style": "ST-1" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn an_undo_may_not_leave_a_note_on_a_view_that_is_gone() {
    let stack = Stack::new().await;
    stack
        .edit("text.style.create", style("ST-1", "General"))
        .await
        .unwrap();
    stack
        .edit("view.create", drafting("V-1", "Detail 1"))
        .await
        .unwrap();
    stack.later();
    // Ben puts a note in Anna's new view; her undo of making the view would leave it on nothing.
    stack
        .edit_as(
            "ben",
            "text.note.create",
            note("N-1", "V-1", "ST-1", "Hello"),
        )
        .await
        .unwrap();
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert_eq!(
        refused,
        format!("{CANNOT}{}", TextError::UnknownView(i("V-1")))
    );
    assert!(stack.exists("V-1").await);
}

#[tokio::test]
async fn an_undo_may_not_take_a_style_a_note_reads_or_repeat_a_name() {
    let stack = Stack::new().await;
    stack
        .edit("view.create", drafting("V-1", "Detail 1"))
        .await
        .unwrap();
    stack
        .edit("text.style.create", style("ST-1", "General"))
        .await
        .unwrap();
    stack.later();
    // Ben's note reads Anna's style, so her undo of making it would leave the note on nothing.
    stack
        .edit_as(
            "ben",
            "text.note.create",
            note("N-1", "V-1", "ST-1", "Hello"),
        )
        .await
        .unwrap();
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert_eq!(
        refused,
        format!("{CANNOT}{}", TextError::UnknownStyle(i("ST-1")))
    );
    assert!(stack.exists("ST-1").await);

    // Anna renames a style; Ben takes the name she gave up; her undo would give it back to a
    // second style.
    stack
        .edit("text.style.create", style("ST-2", "Heading"))
        .await
        .unwrap();
    stack
        .edit(
            "text.style.set",
            json!({ "style": "ST-2", "name": "Spare" }),
        )
        .await
        .unwrap();
    stack.later();
    stack
        .edit_as("ben", "text.style.create", style("ST-3", "Heading"))
        .await
        .unwrap();
    let refused = stack.edit("edit.undo", json!({})).await.unwrap_err();
    assert_eq!(
        refused,
        format!("{CANNOT}{}", TextError::DuplicateName("Heading".to_owned()))
    );
    assert_eq!(the_style(&stack, "ST-2").await.name(), "Spare");
}

#[tokio::test]
async fn text_records_have_no_shape_and_add_no_derived_row() {
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
        .edit("text.style.create", style("ST-1", "General"))
        .await
        .unwrap();
    stack
        .edit("text.note.create", note("N-1", "V-1", "ST-1", "Hello"))
        .await
        .unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    for table in ["body", "display", "built", "area"] {
        assert_eq!(stack.rows(table).await, before[table], "{table}");
    }
    // Only the wall has a shape to check.
    assert_eq!(stack.regen.verify(&i("p")).await, Ok(1));
}

const METRICS: FixedWidth = FixedWidth {
    advance: 600,
    line_height: 1200,
};

async fn world(stack: &Stack) -> World {
    let mut snapshot = stack.store.snapshot().await.unwrap();
    read_world(&mut snapshot, &i("p"), &i(DOCUMENT))
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn the_listing_lays_each_note_out_from_the_record_the_style_and_the_metrics() {
    let stack = with_note().await;
    stack
        .edit(
            "text.note.layout",
            json!({ "note": "N-1", "width": "30", "align": "centre" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "text.note.text",
            json!({ "note": "N-1", "text": "hello wide world" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "text.leader.add",
            json!({ "note": "N-1", "points": [["0", "0"], ["50", "150"]], "arrow": "filled" }),
        )
        .await
        .unwrap();
    stack
        .edit("text.note.create", note("N-2", "V-2", "ST-1", "elsewhere"))
        .await
        .unwrap();
    // 5 mm text: a character is 3 mm wide, a line 6 mm high.
    let listed = listing_of_view(&world(&stack).await, &i("V-1"), Some(&METRICS))
        .unwrap()
        .unwrap();
    assert!(listed.laid_out);
    assert_eq!(listed.notes.len(), 1);
    let shown = serde_json::to_value(&listed).unwrap();
    assert_eq!(shown["view"], "V-1");
    assert_eq!(
        shown["styles"],
        json!([{ "id": "ST-1", "name": "General", "font": "Test Sans", "size": "length:q0:32000",
                 "bold": false, "italic": false, "underline": false, "colour": [0, 0, 0],
                 "spacing": 100, "notes": 2 }])
    );
    let held = &shown["notes"][0];
    assert_eq!(held["id"], "N-1");
    assert_eq!(held["lines"], json!(["hello wide world"]));
    assert_eq!(held["width"], "length:q0:192000");
    assert_eq!(held["align"], "centre");
    assert_eq!(held["leaders"][0]["arrow"], "filled");
    assert_eq!(
        held["leaders"][0]["points"][1],
        json!(["length:q0:320000", "length:q0:960000"])
    );
    let laid = &held["laid"];
    assert_eq!(laid["width"], "length:q0:192000");
    assert_eq!(laid["height"], "length:q0:76800");
    assert_eq!(laid["line_height"], "length:q0:38400");
    // Anchored at 100 mm, 200 mm: the box runs right and down.
    assert_eq!(laid["min_x"], "length:q0:640000");
    assert_eq!(laid["max_x"], "length:q0:832000");
    assert_eq!(laid["max_y"], "length:q0:1280000");
    assert_eq!(laid["min_y"], "length:q0:1203200");
    assert_eq!(laid["lines"][0]["text"], "hello wide");
    assert_eq!(laid["lines"][1]["text"], "world");
    // "world" is 15 mm in a box 30 mm wide: centred, its block starts at 7.5 mm.
    assert_eq!(laid["lines"][1]["x"], "length:q0:48000");
    assert_eq!(laid["lines"][1]["top"], "length:q0:38400");

    // A style change reaches the layout; the note's own record is the same.
    let before = stack.record("N-1").await;
    stack
        .edit("text.style.set", json!({ "style": "ST-1", "size": "10" }))
        .await
        .unwrap();
    assert_eq!(stack.record("N-1").await, before);
    let after = world(&stack).await;
    let listed = listing_of_view(&after, &i("V-1"), Some(&METRICS))
        .unwrap()
        .unwrap();
    let laid = listed.notes[0].laid.as_ref().unwrap();
    assert_eq!(laid.line_height, "length:q0:76800");
    assert_eq!(laid.lines.len(), 3);

    // With no metrics nothing is guessed: the notes are listed with no layout.
    let bare = listing_of_view(&after, &i("V-1"), None).unwrap().unwrap();
    assert!(!bare.laid_out && bare.notes[0].laid.is_none());
    assert_eq!(bare.notes[0].lines, ["hello wide world"]);
    // A view that is not there has no listing, and a note of another view is not in this one.
    assert!(listing_of_view(&after, &i("V-9"), None).unwrap().is_none());
    assert_eq!(
        listing_of_view(&after, &i("V-2"), None)
            .unwrap()
            .unwrap()
            .notes[0]
            .id,
        "N-2"
    );
    // A font the metrics do not know is the caller's refusal, in words.
    struct NoFonts;
    impl FontMetrics for NoFonts {
        fn advance(&self, face: &Face<'_>, _: &str) -> Result<u64, TextError> {
            Err(TextError::Metrics(format!("no font {}", face.font)))
        }
        fn line_height(&self, face: &Face<'_>) -> Result<u64, TextError> {
            Err(TextError::Metrics(format!("no font {}", face.font)))
        }
    }
    assert_eq!(
        listing_of_view(&after, &i("V-1"), Some(&NoFonts)).unwrap_err(),
        "no font Test Sans"
    );
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    for (cmd, args) in [
        ("view.create", drafting("V-1", "Detail 1")),
        ("view.create", drafting("V-2", "Detail 2")),
        ("text.style.create", style("ST-1", "General")),
        ("text.style.create", style("ST-2", "general")),
        ("text.style.create", style("ST-3", "Heading")),
        ("text.note.create", note("N-1", "V-1", "ST-1", "Hello")),
        ("text.note.create", note("N-2", "V-2", "ST-3", "Two\nlines")),
        (
            "text.leader.add",
            json!({ "note": "N-1", "points": [["0", "0"], ["9", "9"]], "arrow": "dot" }),
        ),
        (
            "text.note.layout",
            json!({ "note": "N-1", "width": "40", "list": "bullet", "rotation": "45" }),
        ),
        ("text.style.set", json!({ "style": "ST-1", "size": "7.5" })),
        ("text.style.delete", json!({ "style": "ST-1" })),
        ("view.delete", json!({ "view": "V-2" })),
        ("text.note.delete", json!({ "note": "N-2" })),
        ("edit.undo", json!({})),
        ("text.note.style", json!({ "note": "N-1", "style": "ST-3" })),
        ("text.style.delete", json!({ "style": "ST-1" })),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
        ("text.leader.remove", json!({ "note": "N-1", "leader": 1 })),
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
    assert!(entries.iter().any(|entry| entry.cmd == "text.leader.add"));

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
