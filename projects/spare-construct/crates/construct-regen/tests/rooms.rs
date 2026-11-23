//! A room through the log and the regenerator, on the test store: found from the layered walls of
//! its level by their inner faces, up to the level above; built again when a wall moves, is made
//! or is deleted, its record never written; kept and reported not enclosed when its loop is
//! broken; a door in a wall of its loop leaves it enclosed; a second room in one loop is
//! redundant, naming the first, and takes the loop over when the first is deleted. The records go
//! in as they are, through commands of this file's own that only store or delete them: the
//! commands that place rooms are a later job's (construct-api).
//!
//! The builder is the stand-in, which builds a room as the box around its outer loop; the
//! kernel's own room is the kernel suite's (`tests/kernel_suite`).

#![cfg(feature = "server")]
// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{CategoryTable, WALLS};
use construct_levels::{Level, Tie, write_base};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Model, Reads};
use construct_params::{Area, Choice, Definition, Holder, Kind, Length, ProjectParams, Value};
use construct_regen::parameters::{self, At, Context, TYPE_KEY};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_regen::walls::{self, type_arg};
use construct_regen::{hosting, rooms};
use construct_rooms::{Limit, Outcome as Room, room_record};
use construct_walls::{
    FLIPPED, LOCATION_LINE, LocationLine, default_wall_types, location_line_value,
};
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Clock, Commands, Context as Moment, Orderers, Outcome, Reply};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const PROJECT: &str = "p";
const DOCUMENT: &str = "doc";
const CONCRETE: &str = "wall-concrete-200";
const MM: i64 = 6400;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// The only commands here: `put` stores a record as it is (`{"record": ...}`), `delete` takes
/// one away (`{"id": ...}`), and `model` makes the document on the stand-in's edition.
struct Puts;

impl Commands<Memory> for Puts {
    fn version(&self) -> u32 {
        1
    }

    fn permit(
        &self,
        _: &Scope,
        _: &Edit,
    ) -> impl Future<Output = Result<Result<(), String>, StoreError>> + Send {
        std::future::ready(Ok(Ok(())))
    }

    async fn apply(
        &self,
        txn: &mut <Memory as Kv>::Txn,
        _: &Moment<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        if edit.cmd == "model" {
            let place = Place::Model {
                project: i(PROJECT),
                model: edit.target.clone(),
            };
            let model = Model {
                id: edit.target.clone(),
                name: "Doc".to_owned(),
                team: i("t"),
                package: "Doc package".to_owned(),
                edition: Boxes.edition(),
            };
            txn.put(place.key(), to_row(&model)?).await?;
            return Ok(Ok(Applied::default()));
        }
        if edit.cmd == "delete" {
            let piece = i(edit.args["id"].as_str().unwrap());
            let pointer = kit_store::Place::Piece {
                project: i(PROJECT),
                document: edit.target.clone(),
                piece: piece.clone(),
            };
            let before: Option<String> = read_row(txn, &pointer.key()).await?;
            txn.delete(pointer.key()).await?;
            return Ok(Ok(Applied {
                changes: vec![Change {
                    piece,
                    before: before.map(|h| i(&h)),
                    after: None,
                }],
                ..Applied::default()
            }));
        }
        let record: Element = serde_json::from_value(edit.args["record"].clone()).unwrap();
        let bytes = to_row(&record)?;
        let hash = i(&kit_ops::sha256_hex(&bytes));
        let place = kit_store::Place::Record {
            project: i(PROJECT),
            hash: hash.clone(),
        };
        txn.put(place.key(), bytes).await?;
        let pointer = kit_store::Place::Piece {
            project: i(PROJECT),
            document: edit.target.clone(),
            piece: record.id.clone(),
        };
        let before: Option<String> = read_row(txn, &pointer.key()).await?;
        txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        Ok(Ok(Applied {
            changes: vec![Change {
                piece: record.id,
                before: before.map(|h| i(&h)),
                after: Some(hash),
            }],
            ..Applied::default()
        }))
    }
}

/// The orderer's time, fixed.
struct Fixed;

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        1_000_000
    }
}

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, Puts>,
    regen: Regenerator<Memory, MemoryFiles>,
    next: AtomicU64,
}

impl Stack {
    /// A document with the wall parameters, the concrete wall type, level L-0 at 0 and level L-1
    /// at 3000 mm.
    async fn new() -> Self {
        let store = Memory::new();
        let orderers = Orderers::new(store.clone(), Arc::new(Puts), Arc::new(Fixed), 50);
        let regen = Regenerator::new(store.clone(), MemoryFiles::default(), Arc::new(Boxes));
        let stack = Self {
            store,
            orderers,
            regen,
            next: AtomicU64::new(0),
        };
        stack.run("model", json!({})).await;
        stack
            .put(&parameters::table_record(&i(DOCUMENT), &table()).unwrap())
            .await;
        let context = Context::new(table()).unwrap();
        let ty = default_wall_types(&context.params, &context.categories)
            .unwrap()
            .into_iter()
            .find(|ty| ty.id().as_str() == CONCRETE)
            .unwrap();
        stack.put(&ty.to_record(&i(DOCUMENT))).await;
        for (id, mm) in [("L-0", 0), ("L-1", 3000)] {
            let level = Level::new(i(id), id, Length::from_mm(mm), true).unwrap();
            stack.put(&level.to_record(&i(DOCUMENT))).await;
        }
        stack
    }

    async fn run(&self, cmd: &str, args: Json) {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        let edit = Edit {
            person: i("anna"),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(DOCUMENT),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        };
        let reply = self
            .orderers
            .run(&Scope::Project(i(PROJECT)), edit)
            .await
            .unwrap();
        assert!(matches!(reply, Reply::Ack { .. }), "{reply:?}");
    }

    async fn put(&self, record: &Element) {
        self.run("put", json!({ "record": record })).await;
    }

    async fn delete(&self, id: &str) {
        self.run("delete", json!({ "id": id })).await;
    }

    async fn built(&self, id: &str) -> BuiltElement {
        self.regen.catch_up(&i(PROJECT)).await.unwrap();
        let place = Place::Built {
            project: i(PROJECT),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap()
    }

    /// An element's built bounds, or its failure's code and words.
    async fn bounds(&self, id: &str) -> Result<([i64; 3], [i64; 3]), String> {
        match self.built(id).await.result {
            BuiltResult::Shape { bounds, .. } => Ok((bounds.min, bounds.max)),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
    }

    /// What a room's `reads` row lists.
    async fn reads(&self, id: &str) -> Vec<String> {
        self.regen.catch_up(&i(PROJECT)).await.unwrap();
        let place = Place::Reads {
            project: i(PROJECT),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        let row: Option<Reads> = read_row(&mut snapshot, &place.key()).await.unwrap();
        row.unwrap_or_default().records.into_iter().collect()
    }

    /// What becomes of a room as the document stands now.
    async fn room(&self, record: &Element) -> Room {
        let mut snapshot = self.store.snapshot().await.unwrap();
        rooms::found_at(&mut snapshot, &i(PROJECT), &i(DOCUMENT), record, At::Now)
            .await
            .unwrap()
            .unwrap()
    }
}

/// A document's parameters with the two wall instance parameters bound to walls.
fn table() -> ProjectParams {
    let categories = CategoryTable::new();
    let choices = LocationLine::ALL
        .iter()
        .map(|line| Choice::new(line.key(), line.label()))
        .collect();
    let line =
        Definition::choice(i(LOCATION_LINE), "Location line", Holder::Instance, choices).unwrap();
    let flipped = Definition::new(i(FLIPPED), "Flipped", Kind::YesNo, Holder::Instance).unwrap();
    ProjectParams::new()
        .add_project_parameter(line)
        .unwrap()
        .add_project_parameter(flipped)
        .unwrap()
        .bind(
            &categories,
            &i(LOCATION_LINE),
            &[WALLS],
            Holder::Instance,
            Some(location_line_value(LocationLine::WallCentre)),
        )
        .unwrap()
        .bind(
            &categories,
            &i(FLIPPED),
            &[WALLS],
            Holder::Instance,
            Some(Value::YesNo(false)),
        )
        .unwrap()
}

fn q0(steps: i64) -> String {
    Value::Length(Length::from_steps(steps)).stored()
}

fn on(level: &str) -> Tie {
    Tie {
        level: i(level),
        offset: Length::from_mm(0),
    }
}

/// A 200 mm concrete wall on level L-0, its centre line from `start` to `end` in millimetres,
/// 2700 mm high, its layers read from its type.
fn wall(id: &str, start: [i64; 2], end: [i64; 2]) -> Element {
    let mm = |v: i64| q0(v * MM);
    let mut args = BTreeMap::new();
    args.insert("start".to_owned(), json!([mm(start[0]), mm(start[1])]));
    args.insert("end".to_owned(), json!([mm(end[0]), mm(end[1])]));
    args.insert("height".to_owned(), json!(mm(2700)));
    args.insert("layers".to_owned(), type_arg());
    let mut props = BTreeMap::new();
    props.insert(
        parameters::CATEGORY_KEY.to_owned(),
        parameters::write_category(WALLS),
    );
    props.insert(TYPE_KEY.to_owned(), CONCRETE.to_owned());
    write_base(&mut props, Some(&on("L-0")));
    Element {
        id: i(id),
        document: i(DOCUMENT),
        kind: "wall".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct("wall.layered", args),
        props,
        references: Vec::new(),
    }
}

/// Four walls drawn centre line to centre line round a 4000 by 3000 mm box, the west one at `west`.
fn box_walls(west: i64) -> Vec<Element> {
    vec![
        wall("W-S", [0, 0], [4000, 0]),
        wall("W-E", [4000, 0], [4000, 3000]),
        wall("W-N", [4000, 3000], [0, 3000]),
        wall("W-W", [west, 3000], [west, 0]),
    ]
}

/// Room `id` on level L-0 at `at` (millimetres), up to `limit`.
fn room(id: &str, number: &str, at: [i64; 2], limit: &Limit) -> Element {
    let at = at.map(|v| Length::checked_from_mm(v).unwrap());
    room_record(
        &i(id),
        &i(DOCUMENT),
        "Kitchen",
        number,
        at,
        &on("L-0"),
        limit,
    )
    .unwrap()
}

fn mm2(v: i64) -> Area {
    Area::from_mm2(v)
}

/// The room's area, or the word of what became of it.
fn area(outcome: &Room) -> Result<Area, &'static str> {
    match outcome {
        Room::Enclosed { measures, .. } => Ok(measures.area),
        other => Err(other.word()),
    }
}

/// Through the log: a room is found from the walls of its level by their inner faces, up to the
/// level above; its `reads` row lists its levels and its walls; when a wall moves, is deleted or
/// is made, the room is built again and its record is never written.
#[tokio::test]
async fn a_room_follows_the_walls_round_it() {
    let stack = Stack::new().await;
    for wall in box_walls(0) {
        stack.put(&wall).await;
    }
    let up = Limit::Level(on("L-1"));
    let kitchen = room("R-1", "101", [2000, 1000], &up);
    stack.put(&kitchen).await;
    assert_eq!(
        stack.bounds("R-1").await,
        Ok(([100, 100, 0], [3900, 2900, 3000]))
    );
    let input = stack.built("R-1").await.input;
    assert_eq!(
        stack.reads("R-1").await,
        ["L-0", "L-1", "W-E", "W-N", "W-S", "W-W"]
    );
    let Room::Enclosed { measures, found } = stack.room(&kitchen).await else {
        panic!("not enclosed")
    };
    assert_eq!(measures.area, mm2(10_640_000));
    assert_eq!(measures.perimeter, Length::from_mm(13_200));
    assert_eq!(
        measures.volume,
        mm2(10_640_000).checked_mul(Length::from_mm(3000)).unwrap()
    );
    assert_eq!(found.outer[0].at, [100 * MM, 100 * MM]);

    // The west wall moves 500 mm in.
    stack.put(&wall("W-W", [500, 3000], [500, 0])).await;
    assert_eq!(
        stack.bounds("R-1").await,
        Ok(([600, 100, 0], [3900, 2900, 3000]))
    );
    assert_eq!(area(&stack.room(&kitchen).await), Ok(mm2(9_240_000)));

    // The north wall is deleted: the room stays, not enclosed, with no area.
    stack.delete("W-N").await;
    let failed = stack.bounds("R-1").await.unwrap_err();
    assert!(
        failed.starts_with("NotEnclosed: room 101 Kitchen"),
        "{failed}"
    );
    assert_eq!(stack.built("R-1").await.input, input, "its record is kept");
    assert_eq!(area(&stack.room(&kitchen).await), Err("not enclosed"));

    // Drawn again, it closes the loop; a partition made across the room halves it.
    stack.put(&wall("W-N", [4000, 3000], [0, 3000])).await;
    assert_eq!(
        stack.bounds("R-1").await,
        Ok(([600, 100, 0], [3900, 2900, 3000]))
    );
    stack.put(&wall("W-P", [3000, 0], [3000, 3000])).await;
    assert_eq!(
        stack.bounds("R-1").await,
        Ok(([600, 100, 0], [2900, 2900, 3000]))
    );
    assert!(stack.reads("R-1").await.contains(&"W-P".to_owned()));
    assert_eq!(area(&stack.room(&kitchen).await), Ok(mm2(2300 * 2800)));
    assert_eq!(stack.built("R-1").await.input, input, "never written");
}

/// A door in a wall of the loop does not break it; a second room in the same loop is redundant,
/// naming the first, and takes the loop over when the first is deleted; a room in the other half
/// of a T-meeting is a room of its own.
#[tokio::test]
async fn a_door_leaves_a_room_enclosed_and_a_second_room_is_redundant() {
    let stack = Stack::new().await;
    for wall in box_walls(0) {
        stack.put(&wall).await;
    }
    let host = wall("W-S", [0, 0], [4000, 0]);
    let context = Context::new(table()).unwrap();
    let ty = default_wall_types(&context.params, &context.categories)
        .unwrap()
        .into_iter()
        .find(|ty| ty.id().as_str() == CONCRETE)
        .unwrap();
    let filled = walls::filled(&host.recipe, &context, &host, Some(&ty)).unwrap();
    let face = construct_regen::stand_in::names(&filled)
        .unwrap()
        .into_iter()
        .find(|n| n.role == "interior")
        .unwrap();
    let mut door = Element {
        id: i("D-1"),
        document: i(DOCUMENT),
        kind: "door".to_owned(),
        name: "D-1".to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct(
            "door.single",
            serde_json::from_value(json!({ "width": 900, "thickness": 200, "height": 2100 }))
                .unwrap(),
        ),
        props: BTreeMap::new(),
        references: Vec::new(),
    };
    hosting::write(
        &mut door,
        &i("W-S"),
        &face,
        Length::from_mm(1000),
        Length::from_mm(0),
    );
    stack.put(&door).await;
    let height = Limit::Height(Length::from_mm(2700));
    let first = room("R-1", "101", [2000, 1000], &height);
    stack.put(&first).await;
    assert_eq!(
        stack.bounds("R-1").await,
        Ok(([100, 100, 0], [3900, 2900, 2700])),
        "the doorway does not open the loop"
    );

    let second = room("R-2", "102", [3000, 2000], &height);
    stack.put(&second).await;
    let failed = stack.bounds("R-2").await.unwrap_err();
    assert!(
        failed.starts_with("Redundant: room 102 Kitchen"),
        "{failed}"
    );
    assert!(failed.contains("R-1"), "{failed}");
    assert_eq!(
        stack.room(&second).await,
        Room::Redundant { first: i("R-1") }
    );
    assert_eq!(
        stack.bounds("R-1").await,
        Ok(([100, 100, 0], [3900, 2900, 2700])),
        "the first keeps it"
    );

    stack.delete("R-1").await;
    assert_eq!(
        stack.bounds("R-2").await,
        Ok(([100, 100, 0], [3900, 2900, 2700])),
        "the second takes the loop over"
    );

    // A partition in two T's: the room on its other side is its own.
    stack.put(&wall("W-P", [1500, 0], [1500, 3000])).await;
    let west = room("R-3", "103", [500, 1500], &height);
    stack.put(&west).await;
    assert_eq!(
        stack.bounds("R-3").await,
        Ok(([100, 100, 0], [1400, 2900, 2700]))
    );
    assert_eq!(
        stack.bounds("R-2").await,
        Ok(([1600, 100, 0], [3900, 2900, 2700]))
    );
    assert_eq!(area(&stack.room(&west).await), Ok(mm2(1300 * 2800)));

    // A point in a wall is refused in words, and the record stays.
    let lost = room("R-4", "104", [1500, 1500], &height);
    stack.put(&lost).await;
    let failed = stack.bounds("R-4").await.unwrap_err();
    assert!(failed.starts_with("PointInWall"), "{failed}");
}
