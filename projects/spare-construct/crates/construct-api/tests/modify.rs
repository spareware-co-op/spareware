//! The modify commands through the edit log, on the test store: a wall moved, turned, mirrored,
//! copied and arrayed with the door and the opening it hosts; a door slid along its wall alone;
//! a floor moved with its opening; a room and a grid copied under new numbers and names; each one
//! undo step; every refusal in words, writing nothing; and the log replayed from empty to the same
//! bytes.

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
use construct_api::cloud::{Access, ProjectCommands, walls};
use construct_model::element::Element;
use construct_model::geometry::{Built, Edition, Geometry, GeometryError, Hosted, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_params::{Length, Value as Stored};
use construct_regen::hosting;
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
const DOOR: &str = "library.doors-windows.single-swing-door";
/// Steps in a millimetre.
const MM: i64 = Length::STEPS_PER_MM;

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

/// The stand-in, on its own edition, except that a host built with what is hosted on it lists
/// each hosted element's key and where it sits in the host's frame.
struct Cutter;

impl Geometry for Cutter {
    fn edition(&self) -> Edition {
        Boxes.edition()
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        Boxes.build(recipe)
    }

    fn build_hosting(&self, host: &Recipe, hosted: &[Hosted<'_>]) -> Result<Built, GeometryError> {
        let mut built = Boxes.build(host)?;
        let cuts: Vec<Value> = hosted
            .iter()
            .map(|h| json!({ "key": h.key, "at": [h.at.x, h.at.y, h.at.z] }))
            .collect();
        built.body = serde_json::to_vec(&json!({ "cut": cuts })).unwrap();
        Ok(built)
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
        let geometry: Arc<dyn Geometry> = Arc::new(Cutter);
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

    /// Where each element sat in a host it was built with.
    async fn cuts(&self, id: &str) -> Vec<(String, [i64; 3])> {
        let BuiltResult::Shape { body, .. } = self.built(id).await.result else {
            panic!("{id} did not build");
        };
        let place = Place::Body {
            project: i("p"),
            id: body.clone(),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        let stored = read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap();
        let bytes = self.regen.read_bytes(&stored, &body).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        body["cut"]
            .as_array()
            .map(|cuts| {
                cuts.iter()
                    .map(|c| {
                        let at = |n: usize| c["at"][n].as_i64().unwrap();
                        (c["key"].as_str().unwrap().to_owned(), [at(0), at(1), at(2)])
                    })
                    .collect()
            })
            .unwrap_or_default()
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

/// A point of a layered wall's line, in grid steps.
fn point_of(record: &Element, arg: &str) -> [i64; 2] {
    let pair = record.recipe.args[arg].as_array().unwrap();
    let read = |v: &Value| match Stored::from_stored(v.as_str().unwrap()) {
        Some(Stored::Length(l)) => l.steps(),
        other => panic!("{other:?}"),
    };
    [read(&pair[0]), read(&pair[1])]
}

/// A hosted element's offset along its host, in grid steps.
fn along(record: &Element) -> i64 {
    match Stored::from_stored(&record.props[hosting::ALONG_KEY]) {
        Some(Stored::Length(l)) => l.steps(),
        other => panic!("{other:?}"),
    }
}

/// Ground at 0 and First at 3 m; the default partition; `W-1` 6 m along x from the origin on its
/// exterior finish face, a 900 mm door `F-1` on its exterior 1.5 m along it and a 1 m square
/// opening `O-1` from 4 m along it; and `W-2`, 6 m along x, 4 m off.
async fn a_house(stack: &Stack) {
    for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
        let args = json!({ "level": id, "name": name, "elevation": elevation });
        stack.edit("level.create", args).await.unwrap();
    }
    let partition = walls::default_type_args().unwrap().remove(0);
    let ty = partition["type"].clone();
    stack.edit("wall.type.create", partition).await.unwrap();
    for (id, y) in [("W-1", "0"), ("W-2", "4 m")] {
        let draw = json!({ "element": id, "type": ty, "start": ["0", y], "end": ["6 m", y],
                           "level": "L-g", "top": "L-1", "line": "exterior-finish-face" });
        stack.edit("wall.draw", draw).await.unwrap();
    }
    let door = json!({ "element": "F-1", "family": DOOR, "type": "d900x2100", "host": "W-1",
                       "side": "exterior", "along": "1500 mm" });
    stack.edit("family.place", door).await.unwrap();
    let opening = json!({ "element": "O-1", "host": "W-1", "side": "exterior", "along": "4 m",
                          "sill": "900 mm", "width": "1 m", "height": "1 m" });
    stack.edit("opening.place", opening).await.unwrap();
}

/// The places of what a wall was built with, by key.
async fn cuts_of(stack: &Stack, id: &str) -> BTreeMap<String, [i64; 3]> {
    stack.cuts(id).await.into_iter().collect()
}

/// A wall moved moves what it hosts with it, writing only the wall; one undo takes it back and a
/// redo moves it again.
#[tokio::test]
async fn a_wall_moves_with_what_it_hosts_and_one_undo_takes_it_back() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    let before = cuts_of(&stack, "W-1").await;
    let door = stack.record("F-1").await.unwrap();
    let pieces = stack.rows("piece").await;
    stack
        .edit(
            "modify.move",
            json!({ "elements": ["W-1"], "by": ["1 m", "500 mm"] }),
        )
        .await
        .unwrap();
    let moved = stack.record("W-1").await.unwrap();
    assert_eq!(point_of(&moved, "start"), [1000 * MM, 500 * MM]);
    assert_eq!(point_of(&moved, "end"), [7000 * MM, 500 * MM]);
    assert_eq!(
        stack.record("F-1").await.unwrap(),
        door,
        "a door's record is not rewritten"
    );
    let after = cuts_of(&stack, "W-1").await;
    for (key, [x, y, z]) in &before {
        assert_eq!(after[key], [x + 1000, y + 500, *z], "{key}");
    }
    assert_eq!(
        stack.state("anna").await.undo.map(|step| step.cmd),
        Some("modify.move".to_owned())
    );
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, pieces);
    assert_eq!(cuts_of(&stack, "W-1").await, before);
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.record("W-1").await.unwrap(), moved);
}

/// A quarter turn takes a wall to run along y; the opening it hosts names that axis now, and the
/// door turns with it. A turn from one direction to another lands a wall of its own exactly. A
/// wall that hosts a door or an opening turns off the axes too, by any angle: the builder hosts
/// them at any angle, and the turn undoes whole.
#[tokio::test]
async fn a_wall_turns_by_quarter_turns_and_by_whole_turns_between_directions() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    stack
        .edit(
            "modify.rotate",
            json!({ "elements": ["W-1"], "about": ["0", "0"], "angle": "90°" }),
        )
        .await
        .unwrap();
    let turned = stack.record("W-1").await.unwrap();
    assert_eq!(point_of(&turned, "end"), [0, 6000 * MM]);
    let opening = stack.record("O-1").await.unwrap();
    assert_eq!(opening.recipe.args["axis"], "y");
    let cuts = cuts_of(&stack, "W-1").await;
    assert_eq!(cuts.keys().collect::<Vec<_>>(), ["F-1", "O-1"]);
    assert!(stack.bounds("F-1").await.is_ok());

    // Any other angle turns it too: the builder hosts an opening in a wall at any angle, and the
    // turn is one step that undoes whole.
    let before = stack.rows("piece").await;
    stack
        .edit(
            "modify.rotate",
            json!({ "elements": ["W-1"], "about": ["0", "0"], "angle": "30°" }),
        )
        .await
        .unwrap();
    assert!(stack.bounds("W-1").await.is_ok());
    assert_eq!(
        cuts_of(&stack, "W-1").await.keys().collect::<Vec<_>>(),
        ["F-1", "O-1"]
    );
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, before);

    // A 3:4:5 turn about the origin takes (0, 4 m) to (-3.2 m, 2.4 m) and (6 m, 4 m) to
    // (0.4 m, 7.2 m): on the grid.
    stack
        .edit(
            "modify.rotate",
            json!({ "elements": ["W-2"], "about": ["0", "0"],
                    "from": ["1 m", "0"], "to": ["3 m", "4 m"] }),
        )
        .await
        .unwrap();
    let angled = stack.record("W-2").await.unwrap();
    assert_eq!(point_of(&angled, "start"), [-3200 * MM, 2400 * MM]);
    assert_eq!(point_of(&angled, "end"), [400 * MM, 7200 * MM]);
    assert!(stack.bounds("W-2").await.is_ok());

    // The same turn of the wall that hosts the opening: it goes with its wall.
    let before = stack.rows("piece").await;
    stack
        .edit(
            "modify.rotate",
            json!({ "elements": ["W-1"], "about": ["0", "0"],
                    "from": ["1 m", "0"], "to": ["3 m", "4 m"] }),
        )
        .await
        .unwrap();
    assert!(stack.bounds("W-1").await.is_ok());
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, before);
}

/// A door's fit is asked with the door turned as it is built, facing out of its side: on a wall
/// along y it is turned a quarter, and fits there as it is built (before the modify tools, the
/// fit was asked of the door unturned, and refused it on any wall along y).
#[tokio::test]
async fn a_door_fits_a_wall_along_y_as_it_is_built() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    let partition = walls::default_type_args().unwrap().remove(0);
    let draw = json!({ "element": "W-y", "type": partition["type"], "start": ["10 m", "0"],
                       "end": ["10 m", "5 m"], "level": "L-g", "top": "L-1" });
    stack.edit("wall.draw", draw).await.unwrap();
    for (id, side, along) in [("F-y", "exterior", "1 m"), ("F-z", "interior", "3 m")] {
        let door = json!({ "element": id, "family": DOOR, "type": "d900x2100", "host": "W-y",
                           "side": side, "along": along });
        stack.edit("family.place", door).await.unwrap();
        assert!(stack.bounds(id).await.is_ok(), "{id}");
    }
    assert_eq!(
        cuts_of(&stack, "W-y").await.keys().collect::<Vec<_>>(),
        ["F-y", "F-z"]
    );
}

/// Walls turn by any angle (37.5 degrees here), each end the nearest step to where the turn takes
/// it (the points worked out apart from this crate); two joined at a corner share the rounded
/// corner, so they stay joined, and both build. Turned back by the same angle, the corner is one
/// step (0.00016 mm) from where it began: a turn and its way back are exact, but the corner was
/// rounded between them. A radial array at 30 degrees puts its third copy a quarter turn round,
/// exactly; a sweep of a full turn spreads seven evenly round; one undo takes an array back.
#[tokio::test]
async fn walls_turn_by_any_angle_and_stay_joined() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    let partition = walls::default_type_args().unwrap().remove(0);
    for (id, start, end) in [
        ("W-3", ["10 m", "0"], ["14 m", "0"]),
        ("W-4", ["14 m", "0"], ["14 m", "3 m"]),
    ] {
        let draw = json!({ "element": id, "type": partition["type"], "start": start, "end": end,
                           "level": "L-g", "top": "L-1" });
        stack.edit("wall.draw", draw).await.unwrap();
    }
    let both =
        |angle: &str| json!({ "elements": ["W-3", "W-4"], "about": ["10 m", "0"], "angle": angle });
    stack.edit("modify.rotate", both("37.5°")).await.unwrap();
    let w3 = stack.record("W-3").await.unwrap();
    let w4 = stack.record("W-4").await.unwrap();
    assert_eq!(point_of(&w3, "start"), [10_000 * MM, 0]);
    assert_eq!(point_of(&w3, "end"), [84_309_846, 15_584_293]);
    assert_eq!(point_of(&w4, "start"), point_of(&w3, "end"), "still joined");
    assert_eq!(point_of(&w4, "end"), [72_621_626, 30_816_677]);
    for id in ["W-3", "W-4"] {
        assert!(stack.bounds(id).await.is_ok(), "{id}");
    }

    stack.edit("modify.rotate", both("-37.5°")).await.unwrap();
    let w3 = stack.record("W-3").await.unwrap();
    let w4 = stack.record("W-4").await.unwrap();
    assert_eq!(
        point_of(&w3, "end"),
        [14_000 * MM + 1, 0],
        "one step from where it began"
    );
    assert_eq!(point_of(&w4, "start"), point_of(&w3, "end"), "still joined");
    assert_eq!(point_of(&w4, "end"), [14_000 * MM, 3_000 * MM]);

    stack
        .edit(
            "modify.array",
            json!({ "elements": ["W-3"], "about": ["10 m", "0"], "angle": "30°", "count": 12,
                    "copy": "R" }),
        )
        .await
        .unwrap();
    let quarter = stack.record("R.3.1").await.unwrap();
    assert_eq!(point_of(&quarter, "end"), [10_000 * MM, 4_000 * MM + 1]);
    assert!(stack.record("R.11.1").await.is_some());
    assert!(stack.bounds("R.1.1").await.is_ok());

    stack
        .edit(
            "modify.array",
            json!({ "elements": ["W-3"], "about": ["10 m", "0"], "sweep": "360°", "count": 7,
                    "copy": "S" }),
        )
        .await
        .unwrap();
    let seventh = stack.record("S.1.1").await.unwrap();
    assert_eq!(point_of(&seventh, "end"), [79_961_340, 20_014_887]);
    assert!(stack.record("S.6.1").await.is_some());
    assert!(stack.record("S.7.1").await.is_none());
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.record("S.1.1").await.is_none());
    assert!(stack.record("R.1.1").await.is_some());
}

/// A wall mirrored keeps its exterior where the mirror puts it: drawn on its exterior finish face
/// with its body on the +y side of its line, mirrored about a line along y its body is still on
/// the +y side, its line running the other way and its side flipped. Its door is now hinged on
/// the other hand, at the mirror of where it was.
#[tokio::test]
async fn a_mirrored_wall_keeps_its_exterior_and_its_door_changes_hand() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    let (low, high) = stack.bounds("W-1").await.unwrap();
    assert_eq!([low[1], high[1]], [0, 115]);
    let before = cuts_of(&stack, "W-1").await;
    stack
        .edit(
            "modify.mirror",
            json!({ "elements": ["W-1"], "line": [["3 m", "0"], ["3 m", "1 m"]] }),
        )
        .await
        .unwrap();
    let mirrored = stack.record("W-1").await.unwrap();
    assert_eq!(point_of(&mirrored, "start"), [6000 * MM, 0]);
    assert_eq!(point_of(&mirrored, "end"), [0, 0]);
    assert_eq!(mirrored.props["wall.flipped"], Stored::YesNo(true).stored());
    let (low, high) = stack.bounds("W-1").await.unwrap();
    assert_eq!([low[0], high[0], low[1], high[1]], [0, 6000, 0, 115]);
    let door = stack.record("F-1").await.unwrap();
    assert_eq!(
        door.recipe.args["value.swing"],
        Stored::Choice("right".to_owned()).stored()
    );
    let after = cuts_of(&stack, "W-1").await;
    assert_eq!(after["F-1"][0], 6000 - before["F-1"][0]);
    assert_eq!(after["F-1"][1], before["F-1"][1]);
    // Mirrored back, it is as it was but for holding its own side and hand.
    stack
        .edit(
            "modify.mirror",
            json!({ "elements": ["W-1"], "line": [["3 m", "0"], ["3 m", "1 m"]] }),
        )
        .await
        .unwrap();
    assert_eq!(cuts_of(&stack, "W-1").await, before);
    assert_eq!(
        stack.record("F-1").await.unwrap().recipe.args["value.swing"],
        Stored::Choice("left".to_owned()).stored()
    );
}

/// A wall copied brings what it hosts, each copy hosted on the wall's copy, their IDs made from the
/// key; one undo takes every copy back. A key used again is refused.
#[tokio::test]
async fn copies_of_a_wall_bring_what_it_hosts_on_the_copy() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    let original = stack.record("W-1").await.unwrap();
    stack
        .edit(
            "modify.copy",
            json!({ "elements": ["W-1"], "by": ["0", "2 m"], "copy": "C" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("W-1").await.unwrap(), original);
    let wall = stack.record("C.3").await.unwrap();
    assert_eq!(point_of(&wall, "start"), [0, 2000 * MM]);
    assert_eq!(wall.props, original.props);
    for id in ["C.1", "C.2"] {
        let copy = stack.record(id).await.unwrap();
        assert_eq!(hosting::host_of(&copy), Some(i("C.3")), "{id}");
    }
    assert_eq!(
        cuts_of(&stack, "C.3").await.keys().collect::<Vec<_>>(),
        ["C.1", "C.2"]
    );
    let refused = stack
        .edit(
            "modify.copy",
            json!({ "elements": ["W-1"], "by": ["0", "1 m"], "copy": "C" }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "An element C.1 is already in this model: give the copies another key."
    );
    stack.edit("edit.undo", json!({})).await.unwrap();
    for id in ["C.1", "C.2", "C.3"] {
        assert!(stack.record(id).await.is_none(), "{id}");
    }
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert!(stack.record("C.3").await.is_some());
}

/// A door moved alone slides along its wall and sits where it fits; across its wall, or past its
/// end, it is refused in words; arrayed alone, its copies stay on its wall.
#[tokio::test]
async fn a_door_alone_slides_along_its_wall() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    let before = cuts_of(&stack, "W-1").await;
    stack
        .edit(
            "modify.move",
            json!({ "elements": ["F-1"], "by": ["500 mm", "0"] }),
        )
        .await
        .unwrap();
    let door = stack.record("F-1").await.unwrap();
    assert_eq!(along(&door), 2000 * MM);
    let after = cuts_of(&stack, "W-1").await;
    assert_eq!(after["F-1"][0], before["F-1"][0] + 500);
    assert_eq!(door.placement.x, after["F-1"][0], "seated where it fits");

    let refused = stack
        .edit(
            "modify.move",
            json!({ "elements": ["F-1"], "by": ["0", "10 mm"] }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "F-1 is hosted on W-1: alone it moves only along W-1, and this would take it off. Move \
         W-1 with it, or move it along W-1."
    );
    let refused = stack
        .edit(
            "modify.move",
            json!({ "elements": ["F-1"], "by": ["5 m", "0"] }),
        )
        .await
        .unwrap_err();
    assert!(refused.starts_with("family F-1: "), "{refused}");

    stack
        .edit(
            "modify.move",
            json!({ "elements": ["F-1"], "by": ["-1 m", "0"] }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "modify.array",
            json!({ "elements": ["F-1"], "count": 3, "spacing": ["1 m", "0"], "copy": "A" }),
        )
        .await
        .unwrap();
    for (id, at) in [("A.1.1", 2000), ("A.2.1", 3000)] {
        let copy = stack.record(id).await.unwrap();
        assert_eq!(hosting::host_of(&copy), Some(i("W-1")), "{id}");
        assert_eq!(along(&copy), at * MM, "{id}");
    }
    // One undo takes the whole array back.
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.record("A.1.1").await.is_none());
    assert!(stack.record("A.2.1").await.is_none());
}

/// A floor moved takes the opening drawn in its plan with it; a room copied takes the next free
/// number; a grid arrayed takes the next names.
#[tokio::test]
async fn floors_rooms_and_grids_move_and_copy_as_their_own_kinds() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    stack.edit("floor.defaults", json!({})).await.unwrap();
    let corner = |id: &str, x: &str, y: &str| json!({ "id": id, "at": [x, y] });
    let boundary = json!({ "outer": [corner("r0", "0", "0"), corner("r1", "4 m", "0"),
                                     corner("r2", "4 m", "3 m"), corner("r3", "0", "3 m")] });
    stack
        .edit(
            "floor.create",
            json!({ "element": "S-1", "type": "floor-concrete-200", "level": "L-1",
                    "boundary": boundary }),
        )
        .await
        .unwrap();
    let outline = json!([
        ["500 mm", "500 mm"],
        ["1500 mm", "500 mm"],
        ["1500 mm", "1500 mm"],
        ["500 mm", "1500 mm"]
    ]);
    stack
        .edit(
            "opening.draw",
            json!({ "element": "H-1", "host": "S-1", "outline": outline }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "modify.move",
            json!({ "elements": ["S-1"], "by": ["10 m", "0"] }),
        )
        .await
        .unwrap();
    let floor = stack.record("S-1").await.unwrap();
    let corners = construct_slabs::boundary_of(&floor.recipe)
        .unwrap()
        .corners(construct_slabs::Ring::Outer)
        .unwrap();
    assert_eq!([corners[0].1.x, corners[0].1.y], [10_000 * MM, 0]);
    let opening = stack.record("H-1").await.unwrap();
    let outline = construct_openings::outline_of("H-1", &opening.recipe).unwrap();
    assert_eq!(outline.corners()[0][0].steps(), 10_500 * MM);
    assert!(stack.bounds("S-1").await.is_ok());

    stack
        .edit(
            "room.place",
            json!({ "element": "R-1", "level": "L-g", "at": ["2 m", "2 m"], "height": "2.7 m" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "modify.copy",
            json!({ "elements": ["R-1"], "by": ["20 m", "0"], "copy": "K" }),
        )
        .await
        .unwrap();
    let room = stack.record("R-1").await.unwrap();
    let copy = stack.record("K.1").await.unwrap();
    assert_ne!(
        construct_rooms::number_of(&copy),
        construct_rooms::number_of(&room)
    );
    assert_eq!(
        construct_rooms::at_of(&copy.recipe).unwrap(),
        [22_000 * MM, 2000 * MM]
    );

    stack
        .edit(
            "grid.create",
            json!({ "grid": "G-A", "name": "A", "x0": "0", "y0": "-1 m", "x1": "0", "y1": "9 m" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "modify.array",
            json!({ "elements": ["G-A"], "count": 3, "span": ["12 m", "0"], "copy": "G" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("G.1.1").await.unwrap().name, "B");
    assert_eq!(stack.record("G.2.1").await.unwrap().name, "C");
}

/// Every refusal is in words and writes nothing.
#[tokio::test]
async fn a_modify_is_refused_in_words() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    let before = stack.rows("piece").await;
    for (cmd, args, refusal) in [
        (
            "modify.move",
            json!({ "elements": ["W-1"], "by": ["1 m", "0"], "copy": "C" }),
            "modify.move moves: modify.copy copies.",
        ),
        (
            "modify.copy",
            json!({ "elements": ["W-1"], "by": ["1 m", "0"] }),
            "Copies need a key their IDs are made from (copy).",
        ),
        (
            "modify.rotate",
            json!({ "elements": ["W-1"], "about": ["0", "0"] }),
            "A turn is an angle, or a direction from and a direction to, one or the other.",
        ),
        (
            "modify.mirror",
            json!({ "elements": ["W-1"], "line": [["1 m", "1 m"], ["1 m", "1 m"]] }),
            "A mirror line runs through two different points.",
        ),
        (
            "modify.array",
            json!({ "elements": ["W-1"], "copy": "A", "spacing": ["1 m", "0"] }),
            "An array needs a count, the originals counted.",
        ),
        (
            "modify.move",
            json!({ "elements": ["L-g"], "by": ["1 m", "0"] }),
            "Ground is a level, which the modify tools do not move: change it with its own \
             commands.",
        ),
        (
            "modify.move",
            json!({ "elements": [], "by": ["1 m", "0"] }),
            "Which elements? Name at least one.",
        ),
        (
            "modify.copy",
            json!({ "elements": ["W-1"], "by": ["0", "0"], "copy": "C" }),
            "That would put every copy on its original.",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(refused, refusal, "{cmd}");
    }
    let refused = stack
        .edit(
            "modify.move",
            json!({ "elements": ["W-1"], "by": ["1 m", "0"], "angle": "90°", "extra": 1 }),
        )
        .await
        .unwrap_err();
    assert!(refused.starts_with("That edit isn't valid: "), "{refused}");
    assert_eq!(stack.rows("piece").await, before);
}

/// Another person's live claim on an element a modify rewrites refuses it.
#[tokio::test]
async fn another_persons_claim_refuses_a_modify() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    stack
        .edit_as("ben", "element.claim", json!({ "element": "W-2" }))
        .await
        .unwrap();
    let refused = stack
        .edit(
            "modify.move",
            json!({ "elements": ["W-1", "W-2"], "by": ["1 m", "0"] }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "ben is working on this.");
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    a_house(&stack).await;
    for (cmd, args) in [
        (
            "modify.move",
            json!({ "elements": ["W-1"], "by": ["1 m", "0"] }),
        ),
        (
            "modify.rotate",
            json!({ "elements": ["W-2"], "about": ["0", "4 m"], "angle": "-90°", "copy": "T" }),
        ),
        (
            "modify.mirror",
            json!({ "elements": ["W-1"], "line": [["0", "-1 m"], ["1 m", "-1 m"]], "copy": "M" }),
        ),
        ("edit.undo", json!({})),
        (
            "modify.array",
            json!({ "elements": ["W-1"], "count": 3, "spacing": ["0", "8 m"], "copy": "A" }),
        ),
        // Refused: nothing by that name is in the model.
        (
            "modify.move",
            json!({ "elements": ["NOT-HERE"], "by": ["1 m", "0"] }),
        ),
        (
            "modify.rotate",
            json!({ "elements": ["W-1"], "about": ["0", "0"], "angle": "45°" }),
        ),
        (
            "modify.rotate",
            json!({ "elements": ["W-2"], "about": ["1 m", "1 m"], "angle": "37.5°" }),
        ),
        (
            "modify.array",
            json!({ "elements": ["W-2"], "about": ["0", "0"], "angle": "30°", "count": 4,
                    "copy": "R" }),
        ),
        (
            "modify.array",
            json!({ "elements": ["W-2"], "count": 4, "span": ["1 m", "1 m"], "copy": "L" }),
        ),
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
    let mut replayed = again.store.snapshot().await.unwrap();
    let after = read_log(&mut replayed, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert_eq!(after, entries);
    // Built from empty, the derived rows are the same bytes.
    let incremental = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, incremental);
}
