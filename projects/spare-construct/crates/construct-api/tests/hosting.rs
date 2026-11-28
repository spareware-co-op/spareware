//! Hosting through the edit log, on the test store: a door placed on a face of a wall at an offset
//! along it and up it (`hosted.place`), moved along it (`hosted.move`), the wall built with its
//! doors and built again when one changes, the doors moving with their wall without their records
//! being written, the refusals in words, a wall that hosts kept from deletion and from an undo,
//! an older door hosted by its position, and the log replayed from empty to the same bytes.
//!
//! The builder here records what it is handed: [`Cutter`] is the stand-in on the stand-in's own
//! edition (so the faces are the stand-in's), except that a wall built with its doors lists each
//! door's key, where it sits in the wall and its width in the body's bytes. The kernel's builder
//! cuts real openings on the same calls in the server's `tests/kernel.rs`.

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
                        "width": h.recipe.args.get("width") })
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

fn recipe(op: &str, args: Value) -> Value {
    serde_json::to_value(Recipe::construct(op, serde_json::from_value(args).unwrap())).unwrap()
}

/// A wall along x, 4000 long, 200 thick and 2700 high, at x 1000 and y 2000 of level 1.
fn wall(id: &str) -> Value {
    let args = json!({ "length": 4000, "thickness": 200, "height": 2700 });
    json!({ "element": id, "kind": "wall", "level": 1,
            "placement": { "x": 1000, "y": 2000, "z": 0 },
            "recipe": recipe("wall.straight", args) })
}

/// A door 900 wide, 100 thick and 2100 high, along x.
fn door_recipe(width: i64) -> Value {
    recipe(
        "door.single",
        json!({ "width": width, "thickness": 100, "height": 2100 }),
    )
}

/// `hosted.place`'s arguments: door `id` on the stand-in's face `role` of `host`, `along` it.
fn place(id: &str, host: &str, role: &str, along: &str) -> Value {
    json!({ "element": id, "host": host, "face": format!("box.{role}"), "along": along,
            "recipe": door_recipe(900) })
}

/// A wall, `W-1`, with door `D-1` on its exterior 1200 along and `D-2` on its interior 2500 along.
async fn two_doors(stack: &Stack) {
    stack.edit("element.create", wall("W-1")).await.unwrap();
    stack
        .edit("hosted.place", place("D-1", "W-1", "exterior", "1200 mm"))
        .await
        .unwrap();
    stack
        .edit("hosted.place", place("D-2", "W-1", "interior", "2.5 m"))
        .await
        .unwrap();
}

/// A door placed on a wall's face is stored with its host's face by persistent name and its
/// offsets as stored lengths, on its host's level, where they put it; the wall is built with every
/// door that names it, in the order of their keys, each where its offsets put it in the wall: the
/// exterior flush at the wall's start side, the interior flush at its far side.
#[tokio::test]
async fn a_door_is_placed_on_a_face_and_its_wall_is_built_with_it() {
    let stack = Stack::new(Arc::new(Cutter)).await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    assert_eq!(stack.cuts("W-1").await, []);
    stack
        .edit("hosted.place", place("D-1", "W-1", "exterior", "1200 mm"))
        .await
        .unwrap();
    let (_, door) = stack.record("D-1").await.unwrap();
    assert_eq!(door.kind, "door");
    // On its wall's level: tied as the wall's base is, and on no level by number.
    let (_, wall) = stack.record("W-1").await.unwrap();
    assert_eq!(door.level, 0);
    assert_eq!(door.props["@level"], wall.props["@level"]);
    assert_eq!(door.props["@level.offset"], wall.props["@level.offset"]);
    assert_eq!(
        serde_json::to_value(&door.references).unwrap(),
        json!([{ "doc": null, "element": "W-1", "name": "box.exterior", "kind": "face",
                 "want": "carrier", "signature": "exterior" }])
    );
    assert_eq!(door.props["@host.along"], "length:q0:7680000");
    assert_eq!(door.props["@host.up"], "length:q0:0");
    assert!(door.props.contains_key("@category"));
    assert_eq!(
        [door.placement.x, door.placement.y, door.placement.z],
        [2200, 2000, 0]
    );
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [1200, 0, 0], 900)]
    );
    stack
        .edit("hosted.place", place("D-2", "W-1", "interior", "2.5 m"))
        .await
        .unwrap();
    assert_eq!(
        stack.cuts("W-1").await,
        [
            ("D-1".to_owned(), [1200, 0, 0], 900),
            ("D-2".to_owned(), [2500, 100, 0], 900)
        ]
    );
    let raised = json!({ "element": "D-3", "host": "W-1", "face": "box.exterior",
                         "along": "300", "up": "200 mm", "recipe": door_recipe(400),
                         "name": "Hatch", "kind": "door" });
    stack.edit("hosted.place", raised).await.unwrap();
    let (_, hatch) = stack.record("D-3").await.unwrap();
    assert_eq!(hatch.name, "Hatch");
    assert_eq!(
        [hatch.placement.x, hatch.placement.y, hatch.placement.z],
        [1300, 2000, 200]
    );
    assert_eq!(
        stack.cuts("W-1").await[2],
        ("D-3".to_owned(), [300, 0, 200], 400)
    );
}

/// A door moves with its wall: moving the wall or lengthening it writes no door, and each door
/// is where its offsets put it on the wall's new place. A door moved along its wall, resized, or
/// deleted builds its wall again with it where it now is, or without it.
#[tokio::test]
async fn a_door_moves_with_its_wall_and_its_wall_is_built_again_when_it_changes() {
    let stack = Stack::new(Arc::new(Cutter)).await;
    two_doors(&stack).await;
    let cut = stack.cuts("W-1").await;
    let (hash, door) = stack.record("D-1").await.unwrap();
    assert_eq!(stack.built("D-1").await.cells, [(0, 0)]);

    let moved = json!({ "element": "W-1", "placement": { "x": 11_000, "y": 2000, "z": 0 } });
    stack.edit("element.move", moved).await.unwrap();
    assert_eq!(
        stack.record("D-1").await.unwrap().0,
        hash,
        "no door is written"
    );
    assert_eq!(stack.cuts("W-1").await, cut);
    let row = stack.built("D-1").await;
    assert_eq!(row.input.as_str(), hash);
    assert_eq!(
        row.cells,
        [(2, 0)],
        "the door is indexed where its wall now is"
    );

    let (_, wall) = stack.record("W-1").await.unwrap();
    let mut longer = wall.recipe.clone();
    longer.args.insert("length".to_owned(), json!(6000));
    let set = json!({ "element": "W-1", "recipe": longer });
    stack.edit("element.set", set).await.unwrap();
    assert_eq!(stack.record("D-1").await.unwrap().0, hash);
    assert_eq!(stack.cuts("W-1").await, cut, "lengthened at its end");

    let offset = json!({ "element": "D-1", "along": "4000 mm" });
    stack.edit("hosted.move", offset).await.unwrap();
    let (_, now) = stack.record("D-1").await.unwrap();
    assert_eq!(now.props["@host.along"], "length:q0:25600000");
    assert_eq!(now.props["@host.up"], door.props["@host.up"], "kept");
    assert_eq!(now.placement.x, 15_000);
    assert_eq!(
        stack.cuts("W-1").await[0],
        ("D-1".to_owned(), [4000, 0, 0], 900)
    );

    let wider = json!({ "element": "D-2", "recipe": door_recipe(1000) });
    stack.edit("element.set", wider).await.unwrap();
    assert_eq!(
        stack.cuts("W-1").await[1],
        ("D-2".to_owned(), [2500, 100, 0], 1000)
    );

    stack
        .edit("element.delete", json!({ "element": "D-2" }))
        .await
        .unwrap();
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [4000, 0, 0], 900)]
    );
    stack
        .edit("element.delete", json!({ "element": "D-1" }))
        .await
        .unwrap();
    assert_eq!(stack.cuts("W-1").await, []);
    let alone = Boxes
        .build(&stack.record("W-1").await.unwrap().1.recipe)
        .unwrap();
    let BuiltResult::Shape { body, .. } = stack.built("W-1").await.result else {
        panic!("the wall builds");
    };
    assert_eq!(
        body.as_str(),
        alone.body_id(),
        "a wall with no doors is itself"
    );
}

/// Every derived row made by placing, moving and deleting doors and moving and lengthening their
/// wall is made again, byte for byte, by replaying the log from empty; and every stored shape
/// matches what its inputs build.
#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new(Arc::new(Cutter)).await;
    two_doors(&stack).await;
    stack
        .edit(
            "hosted.move",
            json!({ "element": "D-1", "along": "1500", "up": "10" }),
        )
        .await
        .unwrap();
    let moved = json!({ "element": "W-1", "placement": { "x": 0, "y": 0, "z": 0 } });
    stack.edit("element.move", moved).await.unwrap();
    stack
        .edit("element.delete", json!({ "element": "D-2" }))
        .await
        .unwrap();
    stack
        .edit("hosted.place", place("D-3", "W-1", "exterior", "3000"))
        .await
        .unwrap();
    stack.regen.catch_up(&i("p")).await.unwrap();
    let before = stack.derived().await;
    assert!(
        before.keys().any(|k| k.contains("/reads/")),
        "the hosts are listed"
    );
    let fresh = Regenerator::new(stack.store.clone(), stack.files.clone(), Arc::new(Cutter));
    fresh.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, before);
    assert_eq!(fresh.verify(&i("p")).await.unwrap(), 3);
    assert_eq!(
        stack.cuts("W-1").await,
        [
            ("D-1".to_owned(), [1500, 0, 10], 900),
            ("D-3".to_owned(), [3000, 0, 0], 900)
        ]
    );
}

/// What cannot be hosted is refused as the command's result, in words, and nothing is written: a
/// missing host, a host that is not a wall, a face that is not one of its sides, an offset that is
/// not a length or finer than a placement holds, and a door that does not fit with the others, in
/// the kernel builder's own words, whichever builder the model is on.
#[tokio::test]
async fn what_cannot_be_hosted_is_refused_in_words() {
    for geometry in [Arc::new(Cutter) as Arc<dyn Geometry>, Arc::new(Boxes)] {
        let stack = Stack::new(geometry).await;
        two_doors(&stack).await;
        let block = json!({ "element": "B-1", "kind": "block", "level": 1,
                            "placement": { "x": 0, "y": 0, "z": 0 },
                            "recipe": recipe("block", json!({ "width": 900, "depth": 900,
                                                               "height": 900 })) });
        stack.edit("element.create", block).await.unwrap();
        for (args, refusal) in [
            (
                place("D-9", "W-9", "exterior", "100"),
                "There is no element W-9 in this model to host it.",
            ),
            (
                place("D-9", "B-1", "exterior", "100"),
                "B-1 is not a wall: only a wall hosts a door.",
            ),
            (
                place("D-9", "W-1", "top", "100"),
                "W-1: the face box.top is not one of its sides; an element sits on a wall's \
                 exterior or its interior.",
            ),
            (
                place("D-9", "W-1", "exterior", "12 m2"),
                "The offset along its host takes a length, and \"m2\" is a unit of an area",
            ),
            (
                place("D-9", "W-1", "exterior", "12.5 mm"),
                "door D-9: its offset along its host's face, 12.5 mm, is not a whole number of \
                 millimetres, as every placement is",
            ),
            (
                place("D-9", "W-1", "exterior", "3500"),
                "door D-9: it crosses the wall's end",
            ),
            (
                place("D-9", "W-1", "exterior", "1500"),
                "door D-1 and door D-9: their openings overlap",
            ),
            (
                place("D-1", "W-1", "exterior", "100"),
                "That element already exists.",
            ),
        ] {
            assert_eq!(
                stack.edit("hosted.place", args.clone()).await.unwrap_err(),
                refusal,
                "{args}"
            );
        }
        assert!(stack.record("D-9").await.is_none());
        let tall = json!({ "element": "D-1", "along": "1200", "up": "700" });
        assert_eq!(
            stack.edit("hosted.move", tall).await.unwrap_err(),
            "door D-1: its head is at or above the wall's top, which leaves no wall over it"
        );
        let free = json!({ "element": "B-1", "along": "100" });
        assert_eq!(
            stack.edit("hosted.move", free).await.unwrap_err(),
            "B-1 is not hosted: move it with element.move."
        );
    }
}

/// A wall that hosts doors is not deleted, and says how many; a door on a face moves along its
/// wall, not by `element.move`, and keeps its host through `element.set`, which still renames it.
#[tokio::test]
async fn a_wall_that_hosts_is_kept_and_a_door_keeps_its_host() {
    let stack = Stack::new(Arc::new(Cutter)).await;
    two_doors(&stack).await;
    assert_eq!(
        stack
            .edit("element.delete", json!({ "element": "W-1" }))
            .await
            .unwrap_err(),
        "W-1 still hosts 2 elements: delete them, or host them elsewhere, first."
    );
    stack
        .edit("element.delete", json!({ "element": "D-2" }))
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit("element.delete", json!({ "element": "W-1" }))
            .await
            .unwrap_err(),
        "W-1 still hosts 1 element: delete them, or host them elsewhere, first."
    );
    let moved = json!({ "element": "D-1", "placement": { "x": 0, "y": 0, "z": 0 } });
    assert_eq!(
        stack.edit("element.move", moved).await.unwrap_err(),
        "D-1 is hosted: move it along its host with hosted.move."
    );
    let rehosted = json!({ "element": "D-1", "references": [] });
    assert_eq!(
        stack.edit("element.set", rehosted).await.unwrap_err(),
        "D-1's host is set by hosted.place and hosted.move, not by its references."
    );
    let renamed = json!({ "element": "D-1", "name": "Front door" });
    stack.edit("element.set", renamed).await.unwrap();
    assert_eq!(stack.record("D-1").await.unwrap().1.name, "Front door");
    stack
        .edit("element.delete", json!({ "element": "D-1" }))
        .await
        .unwrap();
    stack
        .edit("element.delete", json!({ "element": "W-1" }))
        .await
        .unwrap();
}

/// An undo that would leave a door without its wall is turned back whole: Anna's wall stays while
/// Ben's door is on it. Ben's own undo takes his door away, and then Anna's undo takes her wall.
#[tokio::test]
async fn an_undo_that_would_leave_a_door_without_its_wall_is_turned_back() {
    let stack = Stack::new(Arc::new(Cutter)).await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    stack
        .edit_as(
            "ben",
            "hosted.place",
            place("D-1", "W-1", "exterior", "1200"),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.edit("edit.undo", json!({})).await.unwrap_err(),
        "That can't be undone: D-1 would be left without its host, W-1."
    );
    assert!(stack.record("W-1").await.is_some());
    stack.edit_as("ben", "edit.undo", json!({})).await.unwrap();
    assert!(stack.record("D-1").await.is_none());
    assert_eq!(stack.cuts("W-1").await, []);
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.record("W-1").await.is_none());
    // Redone in turn, the wall comes back before the door that needs it.
    stack.edit("edit.redo", json!({})).await.unwrap();
    stack.edit_as("ben", "edit.redo", json!({})).await.unwrap();
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [1200, 0, 0], 900)]
    );
}

/// An older door, written with a face reference and no offsets, finds its host by that reference
/// and sits where its own placement puts it: its wall is built with it there, a move of the wall
/// leaves it where it was, the wall is not deleted from under it, and `hosted.move` gives it
/// offsets, so it moves with its wall from then on.
#[tokio::test]
async fn an_older_door_is_hosted_by_its_reference_and_its_position() {
    let stack = Stack::new(Arc::new(Cutter)).await;
    stack.edit("element.create", wall("W-1")).await.unwrap();
    let old = json!({ "element": "D-1", "kind": "door", "level": 1,
                      "placement": { "x": 2000, "y": 2050, "z": 0 },
                      "recipe": door_recipe(900),
                      "references": [{ "doc": null, "element": "W-1", "name": "box.exterior",
                                       "kind": "face", "want": "carrier", "signature": null }] });
    stack.edit("element.create", old).await.unwrap();
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [1000, 50, 0], 900)]
    );
    let moved = json!({ "element": "W-1", "placement": { "x": 500, "y": 2000, "z": 0 } });
    stack.edit("element.move", moved).await.unwrap();
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [1500, 50, 0], 900)]
    );
    assert!(
        stack
            .edit("element.delete", json!({ "element": "W-1" }))
            .await
            .unwrap_err()
            .contains("still hosts 1 element")
    );
    let by_hand = json!({ "element": "D-1", "placement": { "x": 2500, "y": 2050, "z": 0 } });
    stack.edit("element.move", by_hand).await.unwrap();
    stack
        .edit("hosted.move", json!({ "element": "D-1", "along": "1000" }))
        .await
        .unwrap();
    let (_, now) = stack.record("D-1").await.unwrap();
    assert_eq!(now.props["@host.along"], "length:q0:6400000");
    assert_eq!(now.props["@host.up"], "length:q0:0");
    assert_eq!(now.references[0].signature.as_deref(), Some("exterior"));
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [1000, 0, 0], 900)]
    );
}

/// On the stand-in a wall is built alone whatever is hosted on it, byte for byte as before
/// hosting: its body is the stand-in's box.
#[tokio::test]
async fn on_the_stand_in_a_wall_with_doors_is_the_wall_alone() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    two_doors(&stack).await;
    let (_, wall) = stack.record("W-1").await.unwrap();
    let alone = Boxes.build(&wall.recipe).unwrap();
    let BuiltResult::Shape { body, .. } = stack.built("W-1").await.result else {
        panic!("the wall builds");
    };
    assert_eq!(body.as_str(), alone.body_id());
    let row = stack.built("D-2").await;
    assert_eq!(row.cells, [(0, 0)]);
}

/// A layered wall along x from 0 to 4000 mm in its own frame, at x 1000 and y 2000 of level 1,
/// 2700 mm high, its layers (12.5, 90 and 12.5 mm) listed as a filled recipe lists them, its
/// exterior face 57.5 mm to the right of its line.
/// A layered wall `id` of the default partition, 4 m along x from (1000, 2000), 2.7 m high on
/// level 1, drawn as a person draws one (`wall.draw`, after the partition's `wall.type.create`):
/// the generic `element.create` does not make a layered wall.
async fn a_layered_wall(stack: &Stack, id: &str) {
    let partition = walls::default_type_args().unwrap().remove(0);
    let ty = partition["type"].clone();
    stack.edit("wall.type.create", partition).await.unwrap();
    let draw = json!({ "element": id, "type": ty, "start": ["1000 mm", "2000 mm"],
                       "end": ["5000 mm", "2000 mm"], "level": 1, "height": "2700 mm" });
    stack.edit("wall.draw", draw).await.unwrap();
}

/// A layered wall hosts as the box does: a door placed on its exterior sits flush with that side,
/// to the nearest millimetre (57.5 mm from the line), along it from its start, and the wall is
/// built with it; one that would cross its end is refused in the kernel builder's words; moved
/// along it, it is built where it now is.
#[tokio::test]
async fn a_door_is_placed_on_a_layered_wall() {
    let stack = Stack::new(Arc::new(Cutter)).await;
    a_layered_wall(&stack, "W-1").await;
    stack
        .edit("hosted.place", place("D-1", "W-1", "exterior", "1200 mm"))
        .await
        .unwrap();
    let (_, door) = stack.record("D-1").await.unwrap();
    assert_eq!(
        [door.placement.x, door.placement.y, door.placement.z],
        [2200, 1943, 0]
    );
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [2200, 1943, 0], 900)]
    );
    let refused = stack
        .edit("hosted.place", place("D-2", "W-1", "interior", "3500 mm"))
        .await
        .unwrap_err();
    assert_eq!(refused, "door D-2: it crosses the wall's end");
    stack
        .edit(
            "hosted.move",
            json!({ "element": "D-1", "along": "2000 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [3000, 1943, 0], 900)]
    );
}

/// A layered wall `id` of the default partition drawn 5 m from (1000, 2000) to (1000, 2000) plus
/// `end` millimetres, 2.7 m high on level 1.
async fn a_wall_at(stack: &Stack, id: &str, end: [i64; 2]) {
    let partition = walls::default_type_args().unwrap().remove(0);
    let ty = partition["type"].clone();
    stack.edit("wall.type.create", partition).await.unwrap();
    let to = [
        format!("{} mm", 1000 + end[0]),
        format!("{} mm", 2000 + end[1]),
    ];
    let draw = json!({ "element": id, "type": ty, "start": ["1000 mm", "2000 mm"],
                       "end": to, "level": 1, "height": "2700 mm" });
    stack.edit("wall.draw", draw).await.unwrap();
}

/// A wall at any angle hosts through the same commands as one along an axis (piece A83): on a
/// partition drawn 5 m long at 30, 45 and 60 degrees and at a 3:4:5 slope, a door, a plain
/// opening, a window and a placed door are each placed on a side of it, seated in the wall's own
/// frame (their records hold their offsets; their placements the nearest millimetre to their
/// origins: on the 3:4:5 wall the door's, 300 mm along on the exterior face 57.5 mm right of the
/// line, is exactly (226, 205.5) mm from the wall's start, placed at (1226, 2206)); the wall is
/// built with each and each builds; a door that would cross the wall's end is refused in the
/// builder's words, and on the 3:4:5 wall, exactly 5 m long, one that would touch it, both judged
/// in the wall's frame; moved along the wall, the door sits where it now is.
#[tokio::test]
async fn doors_windows_and_openings_are_placed_on_walls_at_any_angle() {
    const DOOR: &str = "library.doors-windows.single-swing-door";
    const WINDOW: &str = "library.doors-windows.fixed-window";
    for end in [[4330, 2500], [3536, 3536], [2500, 4330], [3000, 4000]] {
        let stack = Stack::new(Arc::new(Cutter)).await;
        a_wall_at(&stack, "W-1", end).await;
        let exact = end == [3000, 4000];
        let placed = [
            (
                "hosted.place",
                json!({ "element": "D-1", "host": "W-1", "face": "box.exterior",
                        "along": "300 mm", "recipe": door_recipe(900) }),
            ),
            (
                "opening.place",
                json!({ "element": "O-1", "host": "W-1", "side": "interior", "along": "2800 mm",
                        "sill": "900 mm", "width": "600 mm", "height": "1200 mm" }),
            ),
            (
                "family.place",
                json!({ "element": "F-1", "family": WINDOW, "type": "w1200x1200",
                        "host": "W-1", "side": "exterior", "along": "2000 mm" }),
            ),
            (
                "family.place",
                json!({ "element": "F-2", "family": DOOR, "type": "d900x2100",
                        "host": "W-1", "side": "interior", "along": "4000 mm" }),
            ),
        ];
        for (cmd, args) in placed {
            stack
                .edit(cmd, args)
                .await
                .unwrap_or_else(|e| panic!("{end:?}: {cmd}: {e}"));
        }
        let body = stack.body("W-1").await;
        let cut: Vec<&str> = body["cut"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["key"].as_str().unwrap())
            .collect();
        assert_eq!(cut, ["D-1", "F-1", "F-2", "O-1"], "{end:?}");
        for id in ["D-1", "O-1", "F-1", "F-2"] {
            stack.body(id).await;
        }
        let (_, door) = stack.record("D-1").await.unwrap();
        assert_eq!(door.props["@host.along"], "length:q0:1920000");
        if exact {
            assert_eq!(
                [door.placement.x, door.placement.y, door.placement.z],
                [1226, 2206, 0]
            );
        }
        let refused = stack
            .edit("hosted.place", place("D-2", "W-1", "exterior", "4500 mm"))
            .await
            .unwrap_err();
        assert_eq!(refused, "door D-2: it crosses the wall's end", "{end:?}");
        if exact {
            let refused = stack
                .edit("hosted.place", place("D-2", "W-1", "exterior", "4100 mm"))
                .await
                .unwrap_err();
            assert_eq!(refused, "door D-2: it touches the wall's end");
        }
        let before = door.placement;
        stack
            .edit(
                "hosted.move",
                json!({ "element": "D-1", "along": "450 mm" }),
            )
            .await
            .unwrap();
        let (_, moved) = stack.record("D-1").await.unwrap();
        assert_ne!(moved.placement, before, "{end:?}");
        assert_eq!(moved.props["@host.along"], "length:q0:2880000");
        stack.body("D-1").await;
    }
}
