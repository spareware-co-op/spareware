//! The house: one command script (`tests/house/house.json`, data) builds a small house headless
//! through the same commands a person's edits make, run through the edit log on the test store
//! (job 5.7, a fixture), and this test checks its numbers exactly: elements by kind, each wall's
//! length and its layers' volumes after its door and windows are cut, the ground floor's area and
//! volume after its opening, the roof's plan area and ridge height, the room's area, perimeter and
//! volume. Each expected number is worked out by hand in a comment beside it, from the script's
//! own lengths and the default libraries' sizes, never copied from a run. The script is then
//! undone to an empty document and redone to the same numbers, and the stored rows are copied to
//! a fresh store and read back, by regeneration alone, to the same numbers.
//!
//! The builder is the stand-in. It cuts no openings (the interim register's row), so a wall's
//! volume here is worked out by construct-openings' `wall_layer`, the rule the kernel's builder is
//! measured against, from the wall's built length and height and the built sizes of what is
//! hosted in it; the kernel's own cut is measured in the server's `tests/kernel.rs`, which does
//! not yet run this script (a finding in the pull request).
//!
//! HOW TO ADD A STEP (later waves add to this script): append `{note, cmd, args}` to the end of
//! `steps` in `house/house.json`; raise `STEPS` here by one (every step is one undo step); add
//! what the step makes to `KINDS` and to the numbers below, each worked out by hand in a comment
//! beside it; the undo, redo and read-back checks then cover it with no more code. A step the
//! commands refuse is not worked around: leave it out of the script and list it as a finding.

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
use construct_api::cloud::{Access, ProjectCommands, openings};
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_openings::wall_layer;
use construct_params::{Area, Length, Volume};
use construct_regen::parameters as records;
use construct_regen::parameters::At;
use construct_regen::regenerator::Regenerator;
use construct_regen::rooms::found_at;
use construct_regen::stand_in::Boxes;
use construct_rooms::Outcome;
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
const SCRIPT: &str = include_str!("house/house.json");

/// How many steps the script has: one undo step each.
const STEPS: usize = 22;

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

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    regen: Regenerator<Memory, MemoryFiles>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    /// A store with the project and its team, and no model yet.
    async fn bare() -> Self {
        let store = Memory::new();
        let geometry: Arc<dyn Geometry> = Arc::new(Boxes);
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, 30_000, geometry.edition());
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

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
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

    /// An empty model, then every step of the script, each of which must be taken.
    async fn built() -> Self {
        let stack = Self::bare().await;
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        let script: Value = serde_json::from_str(SCRIPT).unwrap();
        let steps = script["steps"].as_array().unwrap();
        assert_eq!(steps.len(), STEPS, "STEPS says how many the script has");
        for step in steps {
            let (cmd, args) = (step["cmd"].as_str().unwrap(), step["args"].clone());
            if let Err(why) = stack.edit(cmd, args).await {
                panic!("{cmd} ({}) was refused: {why}", step["note"]);
            }
        }
        stack
    }

    /// Every record of the document, by element ID.
    async fn document(&self) -> BTreeMap<Id, construct_model::element::Element> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_document(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .into_iter()
            .map(|(id, (_, record))| (id, record))
            .collect()
    }

    /// An element's built bounds, in whole millimetres.
    async fn bounds(&self, id: &str) -> ([i64; 3], [i64; 3]) {
        self.regen.catch_up(&i("p")).await.unwrap();
        let place = Place::Built {
            project: i("p"),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        let built: BuiltElement = read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap();
        match built.result {
            BuiltResult::Shape { bounds, .. } => (bounds.min, bounds.max),
            BuiltResult::Failed { code, message } => panic!("{id} failed: {code}: {message}"),
        }
    }

    /// Every derived row of the project, as bytes.
    async fn derived(&self) -> BTreeMap<String, Vec<u8>> {
        self.regen.catch_up(&i("p")).await.unwrap();
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

    async fn state(&self) -> State {
        let mut snapshot = self.store.snapshot().await.unwrap();
        undo::state(&mut snapshot, &i("p"), &i(DOCUMENT), &i("anna"))
            .await
            .unwrap()
    }

    /// What a page shows of an element's read-only parameter `parameter`, with a floor's cuts.
    async fn shown(&self, id: &str, parameter: &str) -> String {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let record = self.document().await.remove(&i(id)).unwrap();
        let context = records::read_context(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .unwrap();
        let ty = records::type_of(&record).unwrap();
        let ty = records::read_type(&mut snapshot, &i("p"), &i(DOCUMENT), &ty, &context)
            .await
            .unwrap()
            .unwrap();
        let cuts = if record.kind == "floor" {
            openings::floor_cuts(&mut snapshot, &i("p"), &i(DOCUMENT), &record)
                .await
                .unwrap()
        } else {
            Vec::new()
        };
        let view = context.view_cut(&record, Some(&ty), &cuts).unwrap();
        view.parameters
            .iter()
            .find(|p| p.id == parameter)
            .and_then(|p| p.shown.clone())
            .unwrap_or_else(|| panic!("{id} reads no {parameter}"))
    }
}

/// How many elements of each kind the script makes, by hand from the script's steps.
const KINDS: [(&str, usize); 12] = [
    ("level", 2),         // L-g, L-1
    ("grid", 4),          // A, B, 1, 2
    ("wall", 5),          // four outer walls and one inner wall
    ("floor", 1),         // the ground slab
    ("opening", 1),       // the hole in it
    ("door", 1),          // one family placed
    ("window", 2),        // two families placed
    ("family.recipe", 2), // the recipes of the door and the window, kept in the document when placed
    ("roof", 1),          // the hip roof
    ("room", 1),          // Living
    ("type", 7),          // 2 wall types, 3 default floor types, 2 default roof types
    ("parameters", 1),    // the document's one parameter table
];

/// The layers of the outer wall type, thickest outward, in mm: 110 brick + 40 cavity + 0 wrap +
/// 90 frame + 10 lining = 250.
const OUTER: [Length; 5] = [
    Length::from_mm(110),
    Length::from_mm(40),
    Length::from_mm(0),
    Length::from_mm(90),
    Length::from_mm(10),
];
/// The inner wall type's layers: 12.5 mm (80 000 steps of the length grid) + 90 + 12.5 = 115 mm.
const INNER: [Length; 3] = [
    Length::from_steps(80_000),
    Length::from_mm(90),
    Length::from_steps(80_000),
];

/// A wall's length along its axis and its height from its built bounds, and what is hosted in it.
async fn wall_numbers(stack: &Stack, id: &str) -> (i64, i64, Vec<(Length, Length)>) {
    let (min, max) = stack.bounds(id).await;
    // Walls here lie along x or along y: the longer side is the length.
    let length = (max[0] - min[0]).max(max[1] - min[1]);
    let height = max[2] - min[2];
    let mut cut = Vec::new();
    for (hosted, record) in stack.document().await {
        let in_it = record
            .references
            .first()
            .is_some_and(|r| r.element.as_str() == id);
        if in_it && (record.kind == "door" || record.kind == "window") {
            let (lo, hi) = stack.bounds(hosted.as_str()).await;
            // A door or window is as wide and as high as the opening it stands in.
            cut.push((
                Length::checked_from_mm(hi[0] - lo[0]).unwrap(),
                Length::checked_from_mm(hi[2] - lo[2]).unwrap(),
            ));
        }
    }
    (length, height, cut)
}

/// A wall's volume: each layer's face (length by height, less the openings) by its thickness.
fn wall_volume(length: i64, height: i64, layers: &[Length], cut: &[(Length, Length)]) -> Volume {
    let mut total = Volume::from_mm3(0);
    for &thickness in layers {
        let layer = wall_layer(
            Length::checked_from_mm(length).unwrap(),
            Length::checked_from_mm(height).unwrap(),
            thickness,
            cut,
        )
        .unwrap();
        total = total.checked_add(layer.volume).unwrap();
    }
    total
}

/// Every number the house must have, by hand.
async fn the_house_has_its_numbers(stack: &Stack) {
    // Elements by kind.
    let mut found: BTreeMap<String, usize> = BTreeMap::new();
    for record in stack.document().await.into_values() {
        *found.entry(record.kind).or_default() += 1;
    }
    let expected: BTreeMap<String, usize> = KINDS
        .iter()
        .map(|&(kind, n)| (kind.to_owned(), n))
        .collect();
    assert_eq!(found, expected, "elements by kind");

    // Walls: a level's height is the wall's: from Ground (0) to First (3 m) = 3000 mm. The
    // reference lines run on the rectangle 0..8000 by 0..6000, so the south and north walls are
    // 8000 long and the east, west and inner walls 6000 long.
    // A face is length x height less each opening, and a wall's volume is the sum over its layers
    // of that face by the layer's thickness; the layers of a type add up to its thickness, so:
    //  - south: face = 8000 x 3000 - door 900 x 2100 - window 1200 x 1200
    //          = 24 000 000 - 1 890 000 - 1 440 000 = 20 670 000 mm2; x 250 = 5 167 500 000 mm3
    //  - north: face = 24 000 000 - window 1 440 000 = 22 560 000; x 250 = 5 640 000 000 mm3
    //  - east, west: face = 6000 x 3000 = 18 000 000; x 250 = 4 500 000 000 mm3 each
    //  - inner (115 thick): 18 000 000 x 115 = 2 070 000 000 mm3
    for (id, length, openings, layers, volume) in [
        ("W-s", 8000, 2, &OUTER[..], 5_167_500_000),
        ("W-n", 8000, 1, &OUTER[..], 5_640_000_000),
        ("W-e", 6000, 0, &OUTER[..], 4_500_000_000),
        ("W-w", 6000, 0, &OUTER[..], 4_500_000_000),
        ("W-i", 6000, 0, &INNER[..], 2_070_000_000),
    ] {
        let (found_length, height, cut) = wall_numbers(stack, id).await;
        assert_eq!(found_length, length, "{id}: length");
        assert_eq!(height, 3000, "{id}: height");
        assert_eq!(cut.len(), openings, "{id}: what is cut from it");
        assert_eq!(
            wall_volume(found_length, height, layers, &cut),
            Volume::from_mm3(volume),
            "{id}: volume"
        );
    }

    // The ground floor: 8000 x 6000 = 48 m2, less the opening 1000 x 2000 = 2 m2 = 46 m2; 200 mm
    // thick: 46 x 0.2 = 9.2 m3.
    assert_eq!(stack.shown("F-g", "floor.area").await, "46.00 m²");
    assert_eq!(stack.shown("F-g", "floor.volume").await, "9.20 m³");

    // The roof: over the same rectangle, no overhang: 8000 x 6000 = 48 m2 in plan. Hip at 5:12
    // on 6000 across: the ridge is 3000 in from the eaves, rising 3000 x 5/12 = 1250 mm; the
    // type is 300 mm thick (100 insulation + 200 concrete) measured square to the slope, which is
    // 300 x sqrt(12^2 + 5^2)/12 = 300 x 13/12 = 325 mm up, so the top of the ridge is 1250 + 325
    // = 1575 mm above the roof's base.
    assert_eq!(stack.shown("R-1", "roof.area").await, "48.00 m²");
    assert_eq!(stack.shown("R-1", "roof.ridge-height").await, "1575 mm");
    let (_, top) = stack.bounds("R-1").await;
    assert_eq!(top[2], 1575, "the built roof reaches the same ridge");

    // The room, between the west wall's inner face and the inner wall's west face, and the south
    // and north walls' inner faces. West: the outer wall is 250 thick about x = 0, so its inner
    // face is at 125. The inner wall is 115 thick about x = 4000, so its west face is at
    // 4000 - 57.5 = 3942.5. Width 3942.5 - 125 = 3817.5. South and north: 125 and 6000 - 125 =
    // 5875, depth 5750. Area 3817.5 x 5750 = 21 950 625 mm2; perimeter 2 x (3817.5 + 5750) =
    // 19 135 mm; up to First, 3000 high: volume 21 950 625 x 3000 = 65 851 875 000 mm3.
    let room = stack.document().await.remove(&i("Rm-1")).unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let outcome = found_at(&mut snapshot, &i("p"), &i(DOCUMENT), &room, At::Now)
        .await
        .unwrap()
        .unwrap();
    let Outcome::Enclosed { measures, .. } = outcome else {
        panic!("the room is enclosed");
    };
    assert_eq!(measures.area, Area::from_mm2(21_950_625));
    assert_eq!(measures.perimeter, Length::checked_from_mm(19_135).unwrap());
    assert_eq!(measures.volume, Volume::from_mm3(65_851_875_000));
}

/// The script builds the house, and every number of it holds.
#[tokio::test]
async fn the_script_builds_a_house_with_its_numbers() {
    let stack = Stack::built().await;
    the_house_has_its_numbers(&stack).await;
}

/// The whole script is undone to an empty document, one step each, and redone to the same
/// numbers and the same bytes.
#[tokio::test]
async fn the_script_is_undone_to_empty_and_redone_to_the_same_numbers() {
    let stack = Stack::built().await;
    let built = stack.derived().await;
    let records = stack.document().await;
    let mut undone = 0;
    while stack.state().await.undo.is_some() {
        stack.edit("edit.undo", json!({})).await.unwrap();
        undone += 1;
        assert!(undone <= STEPS, "more undo steps than script steps");
    }
    assert_eq!(undone, STEPS, "one undo step to a script step");
    assert!(stack.document().await.is_empty(), "an empty document");
    let mut redone = 0;
    while stack.state().await.redo.is_some() {
        stack.edit("edit.redo", json!({})).await.unwrap();
        redone += 1;
        assert!(redone <= STEPS);
    }
    assert_eq!(redone, STEPS);
    assert_eq!(stack.document().await, records);
    assert_eq!(stack.derived().await, built);
    the_house_has_its_numbers(&stack).await;
}

/// The stored rows are copied to a fresh store and the house is read back from them, built again
/// by regeneration alone, to the same numbers and the same bytes.
#[tokio::test]
async fn the_stored_house_is_read_back_to_the_same_numbers() {
    let stack = Stack::built().await;
    let built = stack.derived().await;
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let rows = snapshot
        .scan(&kit_store::key::list::project(&i("p")))
        .await
        .unwrap();
    assert!(rows.len() > STEPS, "a row at least for each step");
    let again = Stack::bare().await;
    let mut txn = again.store.begin().await.unwrap();
    for (key, value) in rows {
        txn.put(key, value).await.unwrap();
    }
    txn.commit().await.unwrap();
    // Nothing derived is trusted: it is cleared and built again from the records.
    again.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(again.derived().await, built);
    the_house_has_its_numbers(&again).await;
}
