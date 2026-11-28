//! What one entry of a replay costs, counted by the store, and that the shapes it leaves are the
//! ones every wall gives. A plan of rooms is drawn wall by wall (every room's edge its own wall, so
//! walls meet at every crossing), with a floor, a roof, rooms and a stair, through the edit log,
//! the shapes regenerated after every entry as a server keeps them; the rows regeneration reads
//! for each entry are counted. A plan with four times the walls costs no entry more: regeneration
//! reads what an entry touches, not the model (the walls near a wall, a room's loop among the
//! walls near it, and only the rooms a change can reach). Counts, not times, so the test answers
//! the same on any machine.
//!
//! Then every room's shape is held to its loop found among every wall of its level, the old way
//! ([`rooms::worked_at`], which reads the whole level), after each of a run of edits: walls deleted
//! and drawn again, a wall type made thicker, a level moved, rooms moved into another's loop and
//! out of it, a room outside every loop, undo and redo; every derived row to what a rebuild from
//! the log on a fresh regenerator gives; and, after each of those edits and of a run picked at
//! random, every element to what a build from what it reads now gives (the regenerator's own
//! `verify`). The builder here is the stand-in with each body carrying the recipe it was built
//! from ([`Echo`]), so a wall's joints and a room's loop count in those checks, as they do in the
//! kernel's bodies.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::{Access, ProjectCommands};
use construct_model::geometry::{Built, Edition, Geometry, GeometryError, Hosted, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::parameters::{self as records, At};
use construct_regen::regenerator::Regenerator;
use construct_regen::rooms;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::{Memory, MemoryFiles, MemoryTxn, View};
use kit_store::{Key, Kv, Pair, Prefix, Reader, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
/// A room's side along x and along y, in millimetres.
const ACROSS: i64 = 4000;
const UP: i64 = 5000;

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

/// The stand-in builder, each body carrying the recipe it was built from: so two builds of
/// different recipes have different bodies, a layered wall's joints included, which the stand-in's
/// boxes alone do not show (they are the kernel's to build).
struct Echo(Arc<dyn Geometry>);

impl Geometry for Echo {
    fn edition(&self) -> Edition {
        self.0.edition()
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        let mut built = self.0.build(recipe)?;
        built.body.extend(serde_json::to_vec(recipe).unwrap());
        Ok(built)
    }

    fn build_hosting(&self, host: &Recipe, hosted: &[Hosted<'_>]) -> Result<Built, GeometryError> {
        let mut built = self.0.build_hosting(host, hosted)?;
        built.body.extend(serde_json::to_vec(host).unwrap());
        Ok(built)
    }
}

/// Rows read: each `get`, and each row a scan returned.
#[derive(Default)]
struct Counts(AtomicU64);

impl Counts {
    fn now(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    fn add(&self, rows: usize) {
        self.0.fetch_add(rows as u64, Ordering::SeqCst);
    }
}

/// The test store, counting the rows read through it.
#[derive(Clone, Default)]
struct Counted {
    inner: Memory,
    counts: Arc<Counts>,
}

struct Reading<R> {
    inner: R,
    counts: Arc<Counts>,
}

impl<R: Reader> Reader for Reading<R> {
    async fn get(&mut self, key: &Key) -> Result<Option<Vec<u8>>, StoreError> {
        self.counts.add(1);
        self.inner.get(key).await
    }

    async fn scan(&mut self, prefix: &Prefix) -> Result<Vec<Pair>, StoreError> {
        let rows = self.inner.scan(prefix).await?;
        self.counts.add(rows.len().max(1));
        Ok(rows)
    }

    async fn scan_between(&mut self, from: &Key, to: &Key) -> Result<Vec<Pair>, StoreError> {
        let rows = self.inner.scan_between(from, to).await?;
        self.counts.add(rows.len().max(1));
        Ok(rows)
    }
}

impl<T: Txn> Txn for Reading<T> {
    async fn put(&mut self, key: Key, value: Vec<u8>) -> Result<(), StoreError> {
        self.inner.put(key, value).await
    }

    async fn delete(&mut self, key: Key) -> Result<(), StoreError> {
        self.inner.delete(key).await
    }

    async fn commit(self) -> Result<(), StoreError> {
        self.inner.commit().await
    }

    async fn rollback(self) -> Result<(), StoreError> {
        self.inner.rollback().await
    }
}

impl Kv for Counted {
    type Snapshot = Reading<View>;
    type Txn = Reading<MemoryTxn>;

    async fn snapshot(&self) -> Result<Self::Snapshot, StoreError> {
        Ok(Reading {
            inner: self.inner.snapshot().await?,
            counts: self.counts.clone(),
        })
    }

    async fn begin(&self) -> Result<Self::Txn, StoreError> {
        Ok(Reading {
            inner: self.inner.begin().await?,
            counts: self.counts.clone(),
        })
    }
}

struct Stack {
    store: Counted,
    orderers: Orderers<Counted, ProjectCommands<Counted, Yes>>,
    regen: Regenerator<Counted, MemoryFiles>,
    geometry: Arc<dyn Geometry>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    /// A store with the project, its team and one document, and nothing in it yet.
    async fn new() -> Self {
        let store = Counted::default();
        let geometry: Arc<dyn Geometry> = Arc::new(Echo(construct_regen::geometry()));
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, 30_000, geometry.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
        let regen = Regenerator::new(store.clone(), MemoryFiles::default(), geometry.clone());
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
            geometry,
            clock,
            next: AtomicU64::new(0),
        };
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        stack
    }

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        // Each edit a minute after the last, so no claim is still held.
        self.clock.0.fetch_add(60_000, Ordering::SeqCst);
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

    /// Regenerates what the log holds, as a server keeps up after each entry: the rows read.
    async fn catch_up(&self) -> u64 {
        let before = self.store.counts.now();
        self.regen.catch_up(&i("p")).await.unwrap();
        self.store.counts.now() - before
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

    /// Every element's shape is the one a build from what it reads now gives, joints and loops
    /// included (the builder's bodies carry their recipes): the regenerator's own check.
    async fn verify(&self, after: &str) {
        if let Err(why) = self.regen.verify(&i("p")).await {
            panic!("{after}: {why}");
        }
    }

    /// Every room's built shape is the one its loop among every wall of its level gives: its body,
    /// or its failure's code and words.
    async fn rooms_match_their_whole_level(&self, after: &str) {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let all = records::read_document(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap();
        for (id, (_, room)) in all.iter().filter(|(_, (_, r))| r.kind == "room") {
            let place = Place::Built {
                project: i("p"),
                document: i(DOCUMENT),
                element: id.clone(),
            };
            let built: BuiltElement = read_row(&mut snapshot, &place.key())
                .await
                .unwrap()
                .unwrap_or_else(|| panic!("{after}: {id} was never built"));
            let worked =
                rooms::worked_at(&mut snapshot, &i("p"), &i(DOCUMENT), room, At::Regenerated)
                    .await
                    .unwrap();
            let wanted = match worked.and_then(|(recipe, _)| recipe) {
                Ok(recipe) => match self.geometry.build(&recipe) {
                    Ok(body) => Ok(body.body_id()),
                    Err(e) => Err((e.code, e.message)),
                },
                Err(e) => Err((e.code, e.message)),
            };
            let got = match built.result {
                BuiltResult::Shape { body, .. } => Ok(body.to_string()),
                BuiltResult::Failed { code, message } => Err((code, message)),
            };
            assert_eq!(got, wanted, "{after}: room {id}");
        }
    }
}

/// A wall of the plan from `(x0, y0)` to `(x1, y1)` in millimetres, on level `level` to `top`.
fn wall(
    id: &str,
    ty: &str,
    [x0, y0]: [i64; 2],
    [x1, y1]: [i64; 2],
    level: &str,
    top: &str,
) -> Value {
    json!({ "element": id, "type": ty, "start": [x0.to_string(), y0.to_string()],
            "end": [x1.to_string(), y1.to_string()], "level": level, "top": top })
}

/// A room at the middle of cell `(col, row)`.
fn room(id: &str, number: &str, (col, row): (i64, i64), level: &str) -> Value {
    json!({ "element": id, "name": "Office", "number": number, "level": level,
            "height": "2700 mm",
            "at": [(col * ACROSS + ACROSS / 2).to_string(), (row * UP + UP / 2).to_string()] })
}

/// The levels, the types, and a plan of `k` by `k` rooms on level L0, each room's edge its own
/// wall; a floor and a roof over it all, `rooms` rooms along the middle row, and a stair.
fn plan(k: i64, rooms: i64) -> Vec<(String, Value)> {
    let mut steps: Vec<(String, Value)> = Vec::new();
    let mut s = |cmd: &str, args: Value| steps.push((cmd.to_owned(), args));
    for (level, mm) in [("L0", 0), ("L1", 3000), ("L2", 6000)] {
        s(
            "level.create",
            json!({ "level": level, "name": level, "elevation": format!("{mm} mm") }),
        );
    }
    s(
        "wall.type.create",
        json!({ "type": "wt", "name": "Partition", "assembly": { "core": { "exterior": 1, "interior": 2 },
            "layers": [
                { "function": "finish", "thickness": "12.5 mm", "material": "plasterboard-gypsum", "wraps": true },
                { "function": "structure", "thickness": "90 mm", "material": "concrete-regular", "wraps": false },
                { "function": "finish", "thickness": "12.5 mm", "material": "plasterboard-gypsum", "wraps": true }] } }),
    );
    s(
        "floor.type.create",
        json!({ "type": "ft", "name": "Slab", "layers": [
            { "function": "structure", "thickness": "200 mm", "material": "concrete-regular" }] }),
    );
    s(
        "roof.type.create",
        json!({ "type": "rt", "name": "Roof", "layers": [
            { "function": "structure", "thickness": "200 mm", "material": "concrete-regular" }] }),
    );
    for row in 0..=k {
        for col in 0..k {
            let (a, b) = ([col * ACROSS, row * UP], [(col + 1) * ACROSS, row * UP]);
            s(
                "wall.draw",
                wall(&format!("WH{row}x{col}"), "wt", a, b, "L0", "L1"),
            );
        }
    }
    for col in 0..=k {
        for row in 0..k {
            let (a, b) = ([col * ACROSS, row * UP], [col * ACROSS, (row + 1) * UP]);
            s(
                "wall.draw",
                wall(&format!("WV{col}x{row}"), "wt", a, b, "L0", "L1"),
            );
        }
    }
    let corner = |x: i64, y: i64| json!({ "at": [x.to_string(), y.to_string()] });
    let outline = json!({ "outer": [corner(0, 0), corner(k * ACROSS, 0),
                                    corner(k * ACROSS, k * UP), corner(0, k * UP)] });
    s(
        "floor.create",
        json!({ "element": "F0", "type": "ft", "level": "L0", "boundary": outline }),
    );
    s(
        "roof.create",
        json!({ "element": "R0", "type": "rt", "level": "L1", "footprint": outline }),
    );
    for col in 0..rooms.min(k) {
        s(
            "room.place",
            room(
                &format!("Rm{col}"),
                &format!("{col:03}"),
                (col, k / 2),
                "L0",
            ),
        );
    }
    s(
        "stair.create",
        json!({ "element": "S0", "level": "L0", "top": "L1", "start": ["300", "500"],
                "direction": "+y", "width": "1000 mm", "runs": [{}, {}],
                "landings": [{ "length": "1000 mm", "turn": "back-left" }] }),
    );
    steps
}

/// The most rows one entry read, by command: when it was run, and when it was regenerated.
#[derive(Default)]
struct Most {
    edits: BTreeMap<String, u64>,
    regens: BTreeMap<String, u64>,
}

/// Replays the plan of `k` by `k` rooms with `rooms` rooms, regenerating after every entry, and
/// counts what each entry read.
async fn replay(k: i64, rooms: i64) -> Most {
    let stack = Stack::new().await;
    let mut most = Most::default();
    for (cmd, args) in plan(k, rooms) {
        let before = stack.store.counts.now();
        stack.edit(&cmd, args).await.unwrap();
        let ran = stack.store.counts.now() - before;
        let regenerated = stack.catch_up().await;
        for (map, read) in [(&mut most.edits, ran), (&mut most.regens, regenerated)] {
            let at = map.entry(cmd.clone()).or_default();
            *at = (*at).max(read);
        }
    }
    most
}

/// Regenerating an entry reads what it touches: with nearly four times the walls (a plan of 12 by
/// 12 rooms, 312 walls, against 6 by 6, 84) no entry reads more than a tenth more rows, the walls
/// near the middle of the larger plan meeting a few more than any of the smaller; and drawing a
/// wall, a floor, a roof or a stair reads no more either. Before the walls were found through a
/// grid of their lines, each wall's joints read every wall of the document and each room every
/// wall of its level, so an entry read about as many more rows as the model had walls. (Placing a
/// room still reads the document once, to check its number and its loop as it stands now: the
/// current records have no index by kind.)
#[tokio::test]
async fn regenerating_an_entry_reads_what_it_touches_not_the_model() {
    let small = replay(6, 3).await;
    let large = replay(12, 3).await;
    let flat =
        |what: &str, cmd: &str, small: &BTreeMap<String, u64>, large: &BTreeMap<String, u64>| {
            let (small, large) = (small[cmd], large[cmd]);
            assert!(
                large <= small + small / 10,
                "{what} {cmd}: an entry read {large} rows at 312 walls, {small} at 84"
            );
        };
    for cmd in [
        "wall.draw",
        "room.place",
        "floor.create",
        "roof.create",
        "stair.create",
    ] {
        flat("regenerating", cmd, &small.regens, &large.regens);
    }
    for cmd in [
        "wall.draw",
        "floor.create",
        "roof.create",
        "stair.create",
        "level.create",
    ] {
        flat("running", cmd, &small.edits, &large.edits);
    }
}

/// Rooms found among the walls near them, and found again only where a change reaches, are the
/// rooms every wall of their level gives, after every edit of a run that moves walls, types,
/// levels and rooms; the derived rows are a rebuild's from the log, and verify holds.
#[tokio::test]
async fn rooms_found_near_are_the_rooms_every_wall_gives() {
    let stack = Stack::new().await;
    for (cmd, args) in plan(3, 0) {
        stack.edit(&cmd, args).await.unwrap();
        stack.catch_up().await;
    }
    let mut placed = 0;
    for row in 0..3 {
        for col in 0..3 {
            let id = format!("Rm{row}{col}");
            stack
                .edit(
                    "room.place",
                    room(&id, &format!("{row}{col}"), (col, row), "L0"),
                )
                .await
                .unwrap();
            stack.catch_up().await;
            placed += 1;
            stack
                .rooms_match_their_whole_level(&format!("{placed} rooms placed"))
                .await;
            stack.verify(&format!("{placed} rooms placed")).await;
        }
    }
    let thicker = json!({ "type": "wt", "assembly": { "core": { "exterior": 1, "interior": 2 },
        "layers": [
            { "function": "finish", "thickness": "12.5 mm", "material": "plasterboard-gypsum", "wraps": true },
            { "function": "structure", "thickness": "190 mm", "material": "concrete-regular", "wraps": false },
            { "function": "finish", "thickness": "12.5 mm", "material": "plasterboard-gypsum", "wraps": true }] } });
    let edits: Vec<(&str, Value)> = vec![
        // A wall between two rooms taken away and put back: its neighbours' joints follow.
        ("element.delete", json!({ "element": "WV2x1" })),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
        ("edit.undo", json!({})),
        // Two rooms in one loop: the later of them is redundant.
        ("element.delete", json!({ "element": "WV1x1" })),
        ("element.delete", json!({ "element": "WH2x2" })),
        (
            "room.move",
            json!({ "element": "Rm00", "at": ["6000", "7500"] }),
        ),
        (
            "room.move",
            json!({ "element": "Rm00", "at": ["2000", "2500"] }),
        ),
        (
            "wall.draw",
            wall("WV1x1b", "wt", [ACROSS, UP], [ACROSS, 2 * UP], "L0", "L1"),
        ),
        // An outside wall gone: the rooms along it are not enclosed.
        ("element.delete", json!({ "element": "WH0x1" })),
        ("wall.type.layers", thicker),
        (
            "level.elevation",
            json!({ "level": "L1", "elevation": "3200 mm" }),
        ),
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
        ("room.place", room("Rm99", "99", (5, 5), "L0")),
        ("room.delete", json!({ "element": "Rm11" })),
        ("edit.undo", json!({})),
        (
            "wall.draw",
            wall("WH0x1b", "wt", [ACROSS, 0], [2 * ACROSS, 0], "L0", "L1"),
        ),
        (
            "wall.draw",
            wall("Wx", "wt", [6000, 5000], [6000, 10_000], "L0", "L1"),
        ),
    ];
    for (cmd, args) in edits {
        let what = format!("{cmd} {args}");
        stack
            .edit(cmd, args)
            .await
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        stack.catch_up().await;
        stack.rooms_match_their_whole_level(&what).await;
        stack.verify(&what).await;
    }
    let incremental = stack.derived().await;
    let fresh = Regenerator::new(
        stack.store.clone(),
        MemoryFiles::default(),
        stack.geometry.clone(),
    );
    fresh.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, incremental, "a rebuild from the log");
    assert!(fresh.verify(&i("p")).await.is_ok());
}

/// Numbers that look random and are the same on every run (a linear congruential generator).
struct Numbers(u64);

impl Numbers {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

/// A run of edits picked by numbers: walls drawn on a 2 m grid (crossing, meeting at an end or
/// along, or apart), deleted, undone and redone, rooms placed and moved, and the wall type made
/// thicker or thinner; after each, every element is what a build from what it reads now gives.
#[tokio::test]
async fn edits_picked_at_random_leave_every_shape_a_fresh_build_gives() {
    let stack = Stack::new().await;
    for (cmd, args) in plan(2, 0) {
        stack.edit(&cmd, args).await.unwrap();
        stack.catch_up().await;
    }
    let mut numbers = Numbers(5);
    let mut drawn: Vec<String> = Vec::new();
    for step in 0..30 {
        let pick = numbers.below(10);
        let at = |n: &mut Numbers| (n.below(6) as i64) * 2000 - 1000;
        let (cmd, args) = match pick {
            0..=3 => {
                let (x, y) = (at(&mut numbers), at(&mut numbers));
                let len = (numbers.below(3) as i64 + 1) * 2000;
                let (x1, y1) = if numbers.below(2) == 0 {
                    (x + len, y)
                } else {
                    (x, y + len)
                };
                let id = format!("Wr{step}");
                drawn.push(id.clone());
                ("wall.draw", wall(&id, "wt", [x, y], [x1, y1], "L0", "L1"))
            }
            4 if !drawn.is_empty() => {
                let id = drawn.remove(numbers.below(drawn.len() as u64) as usize);
                ("element.delete", json!({ "element": id }))
            }
            5 => ("edit.undo", json!({})),
            6 => ("edit.redo", json!({})),
            7 => {
                let id = format!("Rr{}", numbers.below(4));
                let (x, y) = (at(&mut numbers) + 1000, at(&mut numbers) + 1000);
                let args = json!({ "element": id, "name": "Office", "number": id, "level": "L0",
                                   "height": "2700 mm", "at": [x.to_string(), y.to_string()] });
                ("room.place", args)
            }
            8 => {
                let id = format!("Rr{}", numbers.below(4));
                let (x, y) = (at(&mut numbers) + 1000, at(&mut numbers) + 1000);
                (
                    "room.move",
                    json!({ "element": id, "at": [x.to_string(), y.to_string()] }),
                )
            }
            _ => {
                let thick = 90 + 50 * numbers.below(3);
                (
                    "wall.type.layers",
                    json!({ "type": "wt", "assembly": { "core": { "exterior": 1, "interior": 2 },
                    "layers": [
                        { "function": "finish", "thickness": "12.5 mm", "material": "plasterboard-gypsum", "wraps": true },
                        { "function": "structure", "thickness": format!("{thick} mm"), "material": "concrete-regular", "wraps": false },
                        { "function": "finish", "thickness": "12.5 mm", "material": "plasterboard-gypsum", "wraps": true }] } }),
                )
            }
        };
        let what = format!("step {step}: {cmd} {args}");
        // Refused edits (a room in a wall, nothing to undo) change nothing, and are fine here.
        if stack.edit(cmd, args).await.is_ok() {
            stack.catch_up().await;
            stack.verify(&what).await;
        }
    }
}
