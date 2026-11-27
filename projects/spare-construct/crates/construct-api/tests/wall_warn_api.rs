//! Walls' quantities and the new warnings through the edit log, on the test store (the three
//! faults found by comparing Construct with another building-modelling program on the same cases):
//! a wall's length, one-face area and volume with its openings taken out, as read-only parameters
//! and as schedule fields, updating as the wall's openings change; a second room in one enclosed
//! space; and an opening that crosses a wall joined to its host.
//!
//! The model is the house script's own steps (`tests/house/house.json`: an 8000 by 6000 mm house of
//! four 250 mm walls with a 115 mm partition at x = 4000 mm), so the numbers are the house's.

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

use construct_api::cloud::schedules::{ScheduleTable, read_table};
use construct_api::cloud::warnings::read_warnings;
use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::WALLS;
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_params::{Area, Length, Value, Volume};
use construct_regen::parameters::{self as records, ParameterView};
use construct_regen::stand_in::Boxes;
use construct_regen::wall_measure::read_only_view;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::Memory;
use kit_store::{Kv, Scope, Txn, to_row};
use kit_sync::{Clock, Orderers, Reply};
use serde_json::{Value as Json, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
const CLAIM_MS: u64 = 30_000;
const SCRIPT: &str = include_str!("house/house.json");

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

    async fn edit(&self, cmd: &str, args: Json) -> Result<u64, String> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        self.clock.0.fetch_add(10, Ordering::SeqCst);
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

    /// Lets every claim run out.
    fn later(&self) {
        self.clock.0.fetch_add(CLAIM_MS + 1, Ordering::SeqCst);
    }

    /// Runs the house script's steps `wanted` says yes to, in the script's order.
    async fn build(&self, wanted: impl Fn(&str, &str) -> bool) {
        let script: Json = serde_json::from_str(SCRIPT).unwrap();
        for step in script["steps"].as_array().unwrap() {
            let cmd = step["cmd"].as_str().unwrap();
            let args = step["args"].clone();
            let element = args["element"].as_str().unwrap_or_default().to_owned();
            if wanted(cmd, &element) {
                self.edit(cmd, args).await.unwrap_or_else(|why| {
                    panic!("{cmd} {element}: {why}");
                });
            }
        }
    }

    async fn record(&self, id: &str) -> Element {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_element(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .map(|(_, record)| record)
            .unwrap()
    }

    /// A wall's read-only parameters, as a page is shown them.
    async fn rows(&self, wall: &str) -> Vec<ParameterView> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let record = self.record(wall).await;
        let context = records::read_context(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .unwrap();
        read_only_view(
            &mut snapshot,
            &i("p"),
            &i(DOCUMENT),
            &record,
            &context.units,
        )
        .await
        .unwrap()
    }

    async fn table(&self, id: &str) -> ScheduleTable {
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_table(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    }

    async fn warnings(&self) -> Vec<(String, String)> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_warnings(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .warnings
            .into_iter()
            .map(|w| (w.identity, w.words))
            .collect()
    }
}

/// The stored text of the row `id`, if it has a value.
fn stored<'a>(rows: &'a [ParameterView], id: &str) -> Option<&'a str> {
    rows.iter()
        .find(|row| row.id == id)
        .and_then(|row| row.value.as_deref())
}

fn length(mm: i32) -> String {
    Value::Length(Length::from_mm(mm)).stored()
}

fn area(mm2: i64) -> String {
    Value::Area(Area::from_mm2(mm2)).stored()
}

fn volume(mm3: i64) -> String {
    Value::Volume(Volume::from_mm3(mm3)).stored()
}

/// The south wall alone, with its door and window: the issue's wall.
async fn the_south_wall() -> Stack {
    let stack = Stack::new().await;
    stack
        .build(|cmd, element| {
            cmd == "level.create"
                || cmd == "wall.type.create"
                || matches!(element, "W-s" | "D-1" | "N-1")
        })
        .await;
    stack
}

#[test]
fn the_script_is_the_house_the_numbers_are_worked_for() {
    // The south wall runs 0 to 8000 mm on level Ground (0) to First (3000 mm), its door 900 x 2100
    // mm 2 m along and its window 1200 x 1200 mm 5.5 m along; the partition W-i runs from (4000, 0)
    // to (4000, 6000); the north window N-2 is 4 m along the north wall, walked east to west, so
    // centred on x = 4000 mm, where the partition meets the north wall.
    let script: Json = serde_json::from_str(SCRIPT).unwrap();
    let step = |element: &str| {
        script["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["args"]["element"] == element)
            .unwrap()["args"]
            .clone()
    };
    assert_eq!(step("W-s")["end"], json!(["8000", "0"]));
    assert_eq!(step("W-i")["start"], json!(["4000", "0"]));
    assert_eq!(step("N-2")["host"], "W-n");
    assert_eq!(step("N-2")["along"], "4 m");
}

#[tokio::test]
async fn a_wall_reads_its_length_area_and_volume_after_its_openings_and_follows_them() {
    let stack = the_south_wall().await;
    let rows = stack.rows("W-s").await;
    // Length: 8000 mm on its line.
    assert_eq!(stored(&rows, "wall.length"), Some(length(8000).as_str()));
    // Face area: 8000 x 3000 - 900 x 2100 - 1200 x 1200 = 24,000,000 - 1,890,000 - 1,440,000
    // = 20,670,000 mm2.
    assert_eq!(stored(&rows, "wall.area"), Some(area(20_670_000).as_str()));
    // Volume: 20,670,000 x 250 = 5,167,500,000 mm3, and by layer (110, 40, 0, 90, 10 mm): the face
    // by the thickness.
    assert_eq!(
        stored(&rows, "wall.volume"),
        Some(volume(5_167_500_000).as_str())
    );
    for (k, mm3) in [
        (1, 2_273_700_000),
        (2, 826_800_000),
        (3, 0),
        (4, 1_860_300_000),
        (5, 206_700_000),
    ] {
        assert_eq!(
            stored(&rows, &format!("wall.layer-{k}.volume")),
            Some(volume(mm3).as_str()),
            "layer {k}"
        );
    }
    assert!(rows.iter().all(|row| row.from == "read-only"));

    // A schedule of walls lists them: no more "Walls have no field quantity:Length".
    let mut args = json!({ "schedule": "S-w", "name": "Walls", "category": WALLS.0 });
    args["fields"] = json!([
        "name",
        "quantity:Length",
        "quantity:Area",
        "quantity:Volume"
    ]);
    stack.edit("schedule.create", args).await.unwrap();
    let table = stack.table("S-w").await;
    assert_eq!(table.rows.len(), 1);
    let cells = &table.rows[0].cells;
    assert_eq!(cells[1].value.as_deref(), Some(length(8000).as_str()));
    assert_eq!(cells[2].value.as_deref(), Some(area(20_670_000).as_str()));
    assert_eq!(
        cells[3].value.as_deref(),
        Some(volume(5_167_500_000).as_str())
    );
    assert_eq!(
        cells[2].shown,
        rows.iter().find(|r| r.id == "wall.area").unwrap().shown
    );

    // The door goes: its 900 x 2100 mm = 1,890,000 mm2 are wall again.
    stack.later();
    stack
        .edit("family.delete", json!({ "element": "D-1" }))
        .await
        .unwrap();
    let rows = stack.rows("W-s").await;
    assert_eq!(stored(&rows, "wall.area"), Some(area(22_560_000).as_str()));
    // 22,560,000 x 250 = 5,640,000,000 mm3.
    assert_eq!(
        stored(&rows, "wall.volume"),
        Some(volume(5_640_000_000).as_str())
    );
    let table = stack.table("S-w").await;
    assert_eq!(
        table.rows[0].cells[3].value.as_deref(),
        Some(volume(5_640_000_000).as_str())
    );
    // The window goes too: the whole face, 8000 x 3000 = 24,000,000 mm2, x 250 = 6,000,000,000 mm3.
    stack.later();
    stack
        .edit("family.delete", json!({ "element": "N-1" }))
        .await
        .unwrap();
    let rows = stack.rows("W-s").await;
    assert_eq!(stored(&rows, "wall.area"), Some(area(24_000_000).as_str()));
    assert_eq!(
        stored(&rows, "wall.volume"),
        Some(volume(6_000_000_000).as_str())
    );
}

#[tokio::test]
async fn a_record_that_is_not_a_wall_reads_none_of_it() {
    let stack = the_south_wall().await;
    assert_eq!(stack.rows("D-1").await.len(), 0);
    assert_eq!(stack.rows("L-g").await.len(), 0);
}

/// The whole house, and a second room in the living room's space and a third outside the walls.
async fn the_house_with_three_rooms() -> Stack {
    let stack = Stack::new().await;
    stack.build(|_, _| true).await;
    for (id, name, at) in [
        ("Rm-2", "Living again", ["1000", "1000"]),
        ("Rm-3", "Yard", ["20000", "20000"]),
    ] {
        let args = json!({ "element": id, "name": name, "level": "L-g", "at": at, "top": "L-1" });
        stack.edit("room.place", args).await.unwrap();
    }
    stack
}

#[tokio::test]
async fn a_second_room_in_one_space_and_an_opening_across_a_joined_wall_are_warnings() {
    let stack = the_house_with_three_rooms().await;
    let found = stack.warnings().await;
    let identities: Vec<&str> = found
        .iter()
        .map(|(identity, _)| identity.as_str())
        .collect();
    // The north window N-2 spans x = 3400 to 4600 mm; the partition W-i, 115 mm thick, has its
    // faces at 3942.5 and 4057.5 mm, inside it, and meets the north wall W-n: the opening would
    // have to cut the partition. The door and the south window clear it (1550 to 2450 mm, 4900 to
    // 6100 mm). Rm-2 is in the living room's loop (Rm-1 comes first), Rm-3 is outside any walls.
    assert_eq!(
        identities,
        [
            "insert.crosses_joined_wall|N-2|W-n|W-i",
            "room.not_enclosed|Rm-3",
            "room.redundant|Rm-2|Rm-1",
        ]
    );
    assert_eq!(
        found[0].1,
        "Window N-2 crosses wall W-i, which joins its host wall W-n: its opening would have to cut the joined wall"
    );
    // Rooms are numbered as they are placed: Living is room 1, Living again room 2.
    assert_eq!(
        found[2].1,
        "Room 2 Living again is redundant: room 1 Living already holds the same enclosed space"
    );

    // The north wall still reads its length and face area (8000 x 3000 - 1200 x 1200 =
    // 22,560,000 mm2); its volume says why it has none: the opening reaches into a joint.
    let rows = stack.rows("W-n").await;
    assert_eq!(stored(&rows, "wall.length"), Some(length(8000).as_str()));
    assert_eq!(stored(&rows, "wall.area"), Some(area(22_560_000).as_str()));
    let blank = rows.iter().find(|row| row.id == "wall.volume").unwrap();
    assert_eq!(blank.value, None);
    assert!(
        blank
            .reason
            .as_deref()
            .unwrap()
            .contains("reaches into a joint")
    );

    // Fixing each cause removes its warning: the window moved 2 m along (x = 6000 mm, 5400 to 6600
    // mm, clear of the partition), the second room deleted.
    stack.later();
    stack
        .edit("family.move", json!({ "element": "N-2", "along": "2 m" }))
        .await
        .unwrap();
    stack
        .edit("room.delete", json!({ "element": "Rm-2" }))
        .await
        .unwrap();
    let identities: Vec<String> = stack.warnings().await.into_iter().map(|w| w.0).collect();
    assert_eq!(identities, ["room.not_enclosed|Rm-3"]);
    // And now the north wall's volume reads.
    let rows = stack.rows("W-n").await;
    assert!(stored(&rows, "wall.volume").is_some());
}
