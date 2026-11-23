//! Placed families through the edit log, on the test store (job 7.1, piece A64): a door or a window
//! of the library placed on a wall's side (`family.place`), changed to another type
//! (`family.retype`), moved along its wall or to another (`family.move`), flipped (`family.flip`),
//! given values of its own (`family.set`) and deleted (`family.delete`), each with undo and redo,
//! each refusal in words, the generic commands turned away naming the right one, and the library
//! as the page lists it.
//!
//! The builder is the stand-in on its own edition, except that a wall built with what is hosted
//! on it lists each hosted element's key and its place in the wall in the body's bytes. The
//! kernel's builder cuts real openings on the same calls in the server's `tests/kernel.rs`.

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

use construct_api::cloud::{Access, ProjectCommands, walls};
use construct_model::element::Element;
use construct_model::geometry::{Built, Edition, Geometry, GeometryError, Hosted, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Reader, Scope, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply};
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

/// The orderer's time, set by the test.
struct Fixed(AtomicU64);

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// The stand-in, on its own edition, except that a host built with what is hosted in it lists
/// each hosted element in its body: its key, where it sits in the host's frame and its width.
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
            .map(|h| {
                json!({ "key": h.key, "at": [h.at.x, h.at.y, h.at.z],
                        "width": h.recipe.args.get("turn") })
            })
            .collect();
        built.body = serde_json::to_vec(&json!({ "host": host.op, "cut": cuts })).unwrap();
        Ok(built)
    }
}

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    regen: Regenerator<Memory, MemoryFiles>,
    #[allow(dead_code)]
    files: MemoryFiles,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    async fn new(geometry: Arc<dyn Geometry>) -> Self {
        let store = Memory::new();
        let files = MemoryFiles::default();
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, 30_000, geometry.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
        let regen = Regenerator::new(store.clone(), files.clone(), geometry);
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
            regen,
            files,
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

    async fn record(&self, id: &str) -> Option<(String, Element)> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let pointer = kit_store::Place::Piece {
            project: i("p"),
            document: i(DOCUMENT),
            piece: i(id),
        };
        let hash: String = read_row(&mut snapshot, &pointer.key()).await.unwrap()?;
        let record = kit_store::Place::Record {
            project: i("p"),
            hash: i(&hash),
        };
        let element = read_row(&mut snapshot, &record.key()).await.unwrap()?;
        Some((hash, element))
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

    /// A body's bytes, once regenerated.
    async fn body(&self, id: &str) -> Value {
        let row = self.built(id).await;
        let BuiltResult::Shape { body, .. } = row.result else {
            panic!("{id} did not build: {:?}", row.result);
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
        serde_json::from_slice(&bytes).unwrap()
    }

    /// What a wall was cut by: each door's key, where it sits in the wall and its width.
    async fn cuts(&self, id: &str) -> Vec<(String, [i64; 3], i64)> {
        let body = self.body(id).await;
        body["cut"]
            .as_array()
            .map(|cuts| {
                cuts.iter()
                    .map(|c| {
                        let at = |n: usize| c["at"][n].as_i64().unwrap();
                        let key = c["key"].as_str().unwrap().to_owned();
                        (key, [at(0), at(1), at(2)], c["width"].as_i64().unwrap())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every derived row of the project, as bytes.
    #[allow(dead_code)]
    async fn derived(&self) -> BTreeMap<String, Vec<u8>> {
        let mut rows = BTreeMap::new();
        let mut snapshot = self.store.snapshot().await.unwrap();
        for table in ["body", "display", "built", "area", "kind", "reads"] {
            let prefix = Scope::Project(i("p")).table_prefix(table, &[]);
            for (key, value) in snapshot.scan(&prefix).await.unwrap() {
                rows.insert(key.as_str().to_owned(), value);
            }
        }
        rows
    }
}

const WINDOW: &str = "library.doors-windows.fixed-window";
const DOOR: &str = "library.doors-windows.single-swing-door";

/// A layered wall `id` of the default partition, 4 m along x from (1000, 2000), 2.7 m high on
/// level 1, drawn as a person draws one.
async fn a_layered_wall(stack: &Stack, id: &str) {
    let partition = walls::default_type_args().unwrap().remove(0);
    let ty = partition["type"].clone();
    // a second wall of the same type finds its type already made
    let _ = stack.edit("wall.type.create", partition).await;
    let draw = json!({ "element": id, "type": ty, "start": ["1000 mm", "2000 mm"],
                       "end": ["5000 mm", "2000 mm"], "level": 1, "height": "2700 mm" });
    stack.edit("wall.draw", draw).await.unwrap();
}

/// `family.place`'s arguments: family `family` of type `ty`, as `id`, on the `side` of `host`,
/// `along` it.
fn place(id: &str, family: &str, ty: &str, host: &str, side: &str, along: &str) -> Value {
    json!({ "element": id, "family": family, "type": ty, "host": host, "side": side,
            "along": along })
}

async fn window_on_wall() -> Stack {
    let stack = Stack::new(Arc::new(Cutter)).await;
    a_layered_wall(&stack, "W-1").await;
    stack
        .edit(
            "family.place",
            place("F-1", WINDOW, "w1200x1200", "W-1", "exterior", "1200 mm"),
        )
        .await
        .unwrap();
    stack
}

/// A window of the library is placed on a wall's side: its record names the family by its content
/// id and its type, its wall by the face, its offsets as stored lengths, and carries its family's
/// category (windows), not generic models; the wall is built with it, seated on the wall's centre
/// plane; undo takes it away and redo puts it back, the same bytes.
#[tokio::test]
async fn a_window_is_placed_on_a_wall_with_its_familys_category() {
    let stack = window_on_wall().await;
    let (_, window) = stack.record("F-1").await.unwrap();
    assert_eq!(window.kind, "window");
    assert_eq!(window.props["@category"], "400");
    assert_eq!(window.props["@host.along"], "length:q0:7680000");
    assert_eq!(window.props["@host.up"], "length:q0:0");
    assert_eq!(window.recipe.op, "family.instance");
    assert_eq!(window.recipe.args["type"], "w1200x1200");
    assert!(
        window.recipe.args["family"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(window.references.len(), 1);
    assert_eq!(window.references[0].element.as_str(), "W-1");
    assert_eq!(window.references[0].kind, "face");
    assert_eq!(window.references[0].signature.as_deref(), Some("exterior"));
    // on the centre plane of the wall (y 2000), 1200 along from x 1000
    assert_eq!(
        [window.placement.x, window.placement.y, window.placement.z],
        [2200, 2000, 0]
    );
    assert_eq!(
        stack.cuts("W-1").await,
        [("F-1".to_owned(), [2200, 2000, 0], 2)]
    );
    let (hash, _) = stack.record("F-1").await.unwrap();
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.record("F-1").await.is_none());
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.record("F-1").await.unwrap().0, hash);
    let door = place("F-2", DOOR, "d900x2100", "W-1", "interior", "3 m");
    stack.edit("family.place", door).await.unwrap();
    let (_, door) = stack.record("F-2").await.unwrap();
    assert_eq!(door.kind, "door");
    assert_eq!(door.props["@category"], "300");
}

/// A family is changed to another type of its own: a type it does not have is refused saying what
/// it has, the same type is refused, a type is undone and redone; a type too wide for its place is
/// refused.
#[tokio::test]
async fn a_family_is_retyped_with_undo_and_redo() {
    let stack = window_on_wall().await;
    let (before, _) = stack.record("F-1").await.unwrap();
    let nothing = stack
        .edit("family.retype", json!({ "element": "F-1", "type": "w1x1" }))
        .await
        .unwrap_err();
    assert!(
        nothing.starts_with("Fixed window has no type w1x1; its types are 600 x 600 (w600x600)"),
        "{nothing}"
    );
    let same = stack
        .edit(
            "family.retype",
            json!({ "element": "F-1", "type": "w1200x1200" }),
        )
        .await
        .unwrap_err();
    assert_eq!(same, "F-1 is already of type w1200x1200.");
    stack
        .edit(
            "family.retype",
            json!({ "element": "F-1", "type": "w1800x1200" }),
        )
        .await
        .unwrap();
    let (_, window) = stack.record("F-1").await.unwrap();
    assert_eq!(window.recipe.args["type"], "w1800x1200");
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.record("F-1").await.unwrap().0, before);
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.record("F-1").await.unwrap().1, window);
    // at 3500 mm along a 4 m wall, 1800 wide crosses the wall's end
    stack
        .edit(
            "family.move",
            json!({ "element": "F-1", "along": "3500 mm" }),
        )
        .await
        .unwrap_err();
}

/// A family moves along its wall, to another wall, and is flipped to the other side at the same
/// place; past the wall's end is refused in words; each can be undone and redone.
#[tokio::test]
async fn a_family_is_moved_flipped_and_refused_past_the_walls_end() {
    let stack = window_on_wall().await;
    stack
        .edit("family.move", json!({ "element": "F-1", "along": "2 m" }))
        .await
        .unwrap();
    let (_, moved) = stack.record("F-1").await.unwrap();
    assert_eq!(moved.props["@host.along"], "length:q0:12800000");
    assert_eq!(moved.placement.x, 3000);
    let refused = stack
        .edit(
            "family.move",
            json!({ "element": "F-1", "along": "3900 mm" }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "family F-1: it crosses the wall's end");
    assert_eq!(stack.record("F-1").await.unwrap().1, moved);
    stack
        .edit("family.flip", json!({ "element": "F-1" }))
        .await
        .unwrap();
    let (_, flipped) = stack.record("F-1").await.unwrap();
    assert_eq!(flipped.references[0].signature.as_deref(), Some("interior"));
    assert_eq!(flipped.props["@host.along"], moved.props["@host.along"]);
    stack
        .edit("family.flip", json!({ "element": "F-1" }))
        .await
        .unwrap();
    assert_eq!(
        stack.record("F-1").await.unwrap().1.references[0]
            .signature
            .as_deref(),
        Some("exterior")
    );
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.record("F-1").await.unwrap().1, flipped);
    stack.edit("edit.redo", json!({})).await.unwrap();
    // to another wall
    let ty = walls::default_type_args().unwrap()[0]["type"].clone();
    stack
        .edit(
            "wall.draw",
            json!({ "element": "W-2", "type": ty,
                    "start": ["1000 mm", "6000 mm"], "end": ["3000 mm", "6000 mm"],
                    "level": 1, "height": "2700 mm" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "family.move",
            json!({ "element": "F-1", "host": "W-2", "along": "1 m" }),
        )
        .await
        .unwrap();
    let (_, other) = stack.record("F-1").await.unwrap();
    assert_eq!(other.references[0].element.as_str(), "W-2");
    assert_eq!(other.placement.y, 6000);
    let nowhere = stack
        .edit("family.move", json!({ "element": "F-1" }))
        .await
        .unwrap_err();
    assert!(nowhere.starts_with("A move needs"), "{nowhere}");
}

/// A window's sill height is its own value: set in words a person types, refused for a value its
/// type holds, for one it has not, for one that is worked out and for one that is not a length;
/// taken away, it reads its default again; each is undone and redone.
#[tokio::test]
async fn an_instance_value_is_set_changed_and_refused() {
    let stack = window_on_wall().await;
    let (plain, _) = stack.record("F-1").await.unwrap();
    stack
        .edit(
            "family.set",
            json!({ "element": "F-1", "values": { "sill_height": "1.1 m" } }),
        )
        .await
        .unwrap();
    let (_, window) = stack.record("F-1").await.unwrap();
    assert_eq!(window.recipe.args["value.sill_height"], "length:q0:7040000");
    let held = stack
        .edit(
            "family.set",
            json!({ "element": "F-1", "values": { "width": "1 m" } }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        held,
        "Width belongs to the type of Fixed window: change its type to change it."
    );
    let none = stack
        .edit(
            "family.set",
            json!({ "element": "F-1", "values": { "colour": "red" } }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        none,
        "Fixed window has no value called colour to set on a placed one."
    );
    let formula = stack
        .edit(
            "family.set",
            json!({ "element": "F-1", "values": { "glass_width": "1 m" } }),
        )
        .await
        .unwrap_err();
    assert_eq!(formula, "Glass width is worked out, so nothing sets it.");
    let text = stack
        .edit(
            "family.set",
            json!({ "element": "F-1", "values": { "sill_height": "high" } }),
        )
        .await
        .unwrap_err();
    assert!(text.contains("Sill height"), "{text}");
    stack
        .edit(
            "family.set",
            json!({ "element": "F-1", "values": { "sill_height": null } }),
        )
        .await
        .unwrap();
    let (reset, back) = stack.record("F-1").await.unwrap();
    assert!(!back.recipe.args.contains_key("value.sill_height"));
    assert_eq!(reset, plain);
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.record("F-1").await.unwrap().1, window);
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.record("F-1").await.unwrap().0, plain);
}

/// A family is deleted with its own command and the delete is undone; the generic commands refuse
/// a family and name the right one.
#[tokio::test]
async fn the_generic_commands_refuse_a_family_and_name_its_own() {
    let stack = window_on_wall().await;
    let (hash, window) = stack.record("F-1").await.unwrap();
    let made = stack
        .edit(
            "element.create",
            json!({ "element": "F-9", "kind": "window", "placement": { "x": 0, "y": 0, "z": 0 },
                    "recipe": window.recipe }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        made,
        "A placed family is made with family.place, which seats it on its wall."
    );
    for (cmd, args, want) in [
        (
            "element.delete",
            json!({ "element": "F-1" }),
            "F-1 is a placed family: delete it with family.delete.",
        ),
        (
            "element.move",
            json!({ "element": "F-1", "placement": { "x": 0, "y": 0, "z": 0 } }),
            "F-1 is a placed family: move it along its wall or to another wall with family.move.",
        ),
        (
            "element.set",
            json!({ "element": "F-1", "name": "Window" }),
            "F-1 is a placed family: change its type with family.retype or its values with \
             family.set.",
        ),
        (
            "hosted.move",
            json!({ "element": "F-1", "along": "1 m" }),
            "F-1 is a placed family: move it along its wall or to another wall with family.move.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), want, "{cmd}");
    }
    let hosted = stack
        .edit(
            "hosted.place",
            json!({ "element": "F-8", "host": "W-1", "face": "x", "along": "1 m",
                    "recipe": window.recipe }),
        )
        .await
        .unwrap_err();
    assert!(hosted.starts_with("A placed family is made with family.place"));
    // the wall that hosts it is not deleted; the family is, and the delete is undone
    let wall = stack
        .edit("element.delete", json!({ "element": "W-1" }))
        .await
        .unwrap_err();
    assert_eq!(
        wall,
        "W-1 still hosts 1 element: delete them, or host them elsewhere, first."
    );
    stack
        .edit("family.delete", json!({ "element": "F-1" }))
        .await
        .unwrap();
    assert!(stack.record("F-1").await.is_none());
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.record("F-1").await.unwrap().0, hash);
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert!(stack.record("F-1").await.is_none());
    let gone = stack
        .edit("family.delete", json!({ "element": "F-1" }))
        .await
        .unwrap_err();
    assert_eq!(gone, "That element no longer exists.");
    stack
        .edit("element.delete", json!({ "element": "W-1" }))
        .await
        .unwrap();
}

/// Every refusal of placing one, in words: a wall that is not there or not a wall, a side that is
/// not one, a family or a type the library lacks, an offset that is not a length, a family past
/// the wall's end, one on top of another; nothing is written by a refusal.
#[tokio::test]
async fn placing_a_family_is_refused_in_words() {
    let stack = window_on_wall().await;
    let mut refusals = Vec::new();
    for args in [
        place("F-2", WINDOW, "w1200x1200", "W-9", "exterior", "1 m"),
        place("F-2", WINDOW, "w1200x1200", "W-1", "top", "1 m"),
        place("F-2", "library.nothing", "x", "W-1", "exterior", "1 m"),
        place("F-2", WINDOW, "w1x1", "W-1", "exterior", "1 m"),
        place("F-2", WINDOW, "w1200x1200", "W-1", "exterior", "far"),
        place("F-2", WINDOW, "w1200x1200", "W-1", "exterior", "3900 mm"),
        place("F-2", WINDOW, "w1200x1200", "W-1", "exterior", "1300 mm"),
        place("F-1", WINDOW, "w1200x1200", "W-1", "exterior", "3 m"),
        json!({ "element": "F-2" }),
    ] {
        refusals.push(stack.edit("family.place", args).await.unwrap_err());
    }
    assert_eq!(
        refusals[0],
        "There is no element W-9 in this model to host it."
    );
    assert!(
        refusals[1].contains("top is not one of its sides"),
        "{}",
        refusals[1]
    );
    assert!(
        refusals[2].starts_with("There is no family library.nothing in the library; it has "),
        "{}",
        refusals[2]
    );
    assert!(
        refusals[3].starts_with("Fixed window has no type w1x1;"),
        "{}",
        refusals[3]
    );
    assert_ne!(refusals[4], "");
    assert_eq!(refusals[5], "family F-2: it crosses the wall's end");
    assert_eq!(
        refusals[6],
        "family F-1 and family F-2: their openings overlap"
    );
    assert_eq!(refusals[7], "That element already exists.");
    assert!(refusals[8].starts_with("A placed family needs"));
    assert!(stack.record("F-2").await.is_none());
    let door_wall = stack
        .edit(
            "family.place",
            place("F-3", DOOR, "d900x2100", "F-1", "exterior", "1 m"),
        )
        .await
        .unwrap_err();
    assert_eq!(
        door_wall,
        "F-1 is not a wall: only a wall hosts a door or a window."
    );
}

/// The library as the page lists it: the three families, each with its category, types and the
/// values each placed one holds.
#[test]
fn the_library_lists_its_families_for_the_page() {
    let all = construct_api::cloud::families::read_library();
    assert_eq!(all.len(), 3);
    let window = all.iter().find(|f| f.id == WINDOW).unwrap();
    assert_eq!(window.kind, "window");
    assert_eq!(window.category, 400);
    assert_eq!(window.category_name, "Windows");
    assert_eq!(window.types.len(), 8);
    assert_eq!(window.values[0].key, "sill_height");
    assert_eq!(window.values[0].shown, "900 mm");
    let door = all.iter().find(|f| f.id == DOOR).unwrap();
    assert_eq!(door.kind, "door");
    assert_eq!(door.values[0].key, "swing");
    assert_eq!(door.values[0].choices.len(), 2);
}
