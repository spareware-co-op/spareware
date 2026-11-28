//! Ceilings through the edit log, on the test store (job A101): types and their defaults; a ceiling
//! sketched by its outline and tied over a level by its exposed face (its upper face a type's
//! thickness higher); a ceiling placed by clicking inside a room's walls, outlined by the very loop
//! a room is; the grid pattern and its exact count; a room's volume bounded by the ceiling over
//! it, rebuilt by regeneration alone; every refusal in words; the generic commands refused where a
//! ceiling's own exist; undo and redo of every ceiling command; and the log replayed to the same
//! bytes.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::undo::{self, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_params::Area;
use construct_regen::parameters as records;
use construct_regen::parameters::At;
use construct_regen::regenerator::Regenerator;
use construct_regen::rooms::found_at;
use construct_regen::stand_in::Boxes;
use construct_rooms::Outcome;
use construct_walls::default_walls;
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
const PARTITION: &str = "wall-partition-stud-90";

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

/// Ground at 0 and First at 3 m.
async fn two_levels(stack: &Stack) {
    for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
        let args = json!({ "level": id, "name": name, "elevation": elevation });
        stack.edit("level.create", args).await.unwrap();
    }
}

/// The default partition wall type, as `wall.type.create` takes it.
fn partition_type() -> Value {
    let walls = default_walls().unwrap();
    let wall = walls.iter().find(|wall| wall.id == PARTITION).unwrap();
    let layers: Vec<Value> = wall
        .assembly
        .layers()
        .iter()
        .map(|layer| {
            json!({ "function": layer.function, "thickness": layer.thickness.to_string(),
                    "material": layer.material.as_str(), "wraps": layer.wraps })
        })
        .collect();
    let assembly = json!({
        "core": {
            "exterior": wall.assembly.boundary(construct_walls::Side::Exterior),
            "interior": wall.assembly.boundary(construct_walls::Side::Interior),
        },
        "layers": layers,
    });
    json!({ "type": PARTITION, "name": wall.name, "assembly": assembly })
}

/// A wall of the partition type along its centre line, on Ground, its top on First.
async fn wall(stack: &Stack, id: &str, start: [&str; 2], end: [&str; 2]) {
    let args = json!({ "element": id, "type": PARTITION, "start": start, "end": end,
                       "level": "L-g", "top": "L-1" });
    stack.edit("wall.draw", args).await.unwrap();
}

/// Two levels, the partition type and four walls round a 4 m by 3 m box, their centre lines on
/// its edges: inside the faces it is 3885 mm by 2885 mm.
async fn a_box(stack: &Stack) {
    two_levels(stack).await;
    stack
        .edit("wall.type.create", partition_type())
        .await
        .unwrap();
    wall(stack, "W-s", ["0", "0"], ["4 m", "0"]).await;
    wall(stack, "W-e", ["4 m", "0"], ["4 m", "3 m"]).await;
    wall(stack, "W-n", ["4 m", "3 m"], ["0", "3 m"]).await;
    wall(stack, "W-w", ["0", "3 m"], ["0", "0"]).await;
}

/// `room.place`'s arguments: room `id` on Ground at (`x`, `y`) mm, up to First.
fn place(id: &str, x: i64, y: i64) -> Value {
    json!({ "element": id, "name": "Kitchen", "level": "L-g", "at": [x.to_string(), y.to_string()],
            "top": "L-1" })
}

/// What becomes of the room `id` as the document stands.
async fn outcome(stack: &Stack, id: &str) -> Outcome {
    let record = stack.record(id).await.unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    found_at(&mut snapshot, &i("p"), &i(DOCUMENT), &record, At::Now)
        .await
        .unwrap()
        .unwrap()
}

// ---- ceilings (job A101) ----

use construct_params::{Length, Volume};
use construct_slabs::{Grid, Ring, area, ceiling_boundary, grid_of, is_ceiling};

/// A rectangle's outline at (`x`, `y`) mm, `w` by `d` mm, as a command takes it.
fn rect(x: i64, y: i64, w: i64, d: i64) -> Value {
    let at = |x: i64, y: i64| json!({ "at": [format!("{x} mm"), format!("{y} mm")] });
    json!({ "outer": [at(x, y), at(x + w, y), at(x + w, y + d), at(x, y + d)] })
}

/// The 8 m by 4 m outline with 4 m by 4 m more over its left half (48 m2), as a command takes it.
fn l_shape() -> Value {
    let at = |x: i64, y: i64| json!({ "at": [format!("{x} mm"), format!("{y} mm")] });
    json!({ "outer": [at(0, 0), at(8000, 0), at(8000, 4000), at(4000, 4000), at(4000, 8000), at(0, 8000)] })
}

const PLASTER: &str = "ceiling-plaster-13";
const TIMBER: &str = "ceiling-plaster-13-timber-45";

/// `ceiling.create`'s arguments for a ceiling sketched by `boundary`, its exposed face
/// `offset` above Ground.
#[allow(
    clippy::needless_pass_by_value,
    reason = "callers hand over the shape they have just built"
)]
fn sketched(id: &str, ty: &str, offset: &str, boundary: Value) -> Value {
    json!({ "element": id, "type": ty, "level": "L-g", "offset": offset, "boundary": boundary })
}

/// `ceiling.create`'s arguments for a ceiling placed by clicking at (`x`, `y`) mm in a room's walls.
fn clicked(id: &str, offset: &str, x: i64, y: i64) -> Value {
    json!({ "element": id, "type": PLASTER, "level": "L-g", "offset": offset,
            "at": [format!("{x} mm"), format!("{y} mm")] })
}

/// A model with its levels and the default ceiling types; `walls` also draws the box of walls.
async fn with_defaults(walls: bool) -> Stack {
    let stack = Stack::new().await;
    if walls {
        a_box(&stack).await;
    } else {
        two_levels(&stack).await;
    }
    stack.edit("ceiling.defaults", json!({})).await.unwrap();
    stack
}

fn length_text(mm: i32) -> String {
    construct_params::Value::Length(Length::from_mm(mm)).stored()
}

/// The volume a room reads back, or why it has none.
async fn volume_of(stack: &Stack, id: &str) -> Volume {
    match outcome(stack, id).await {
        Outcome::Enclosed { measures, .. } => measures.volume,
        other => panic!("{id} is {}", other.word()),
    }
}

/// Ceiling types: the defaults come once, a type of one's own is made, its layers changed, and
/// every refusal is in words.
#[tokio::test]
async fn ceiling_types_are_made_by_defaults_and_by_hand() {
    let stack = Stack::new().await;
    stack.edit("ceiling.defaults", json!({})).await.unwrap();
    for id in [PLASTER, TIMBER, "ceiling-suspended-tile-15"] {
        let ty = stack.record(id).await.unwrap();
        assert_eq!(
            records::stored_category(&ty),
            Some(construct_categories::CEILINGS)
        );
    }
    let before = stack.rows("piece").await;
    stack.edit("ceiling.defaults", json!({})).await.unwrap();
    assert_eq!(
        stack.rows("piece").await,
        before,
        "nothing lacking, nothing written"
    );

    let own = json!({ "type": "ct-own", "name": "Board 16", "layers": [
        { "function": "finish", "thickness": "16 mm", "material": "plaster-gypsum" }] });
    stack
        .edit("ceiling.type.create", own.clone())
        .await
        .unwrap();
    assert_eq!(
        stack.edit("ceiling.type.create", own).await.unwrap_err(),
        "That element already exists."
    );
    let empty = json!({ "type": "ct-empty", "name": "None", "layers": [] });
    assert!(stack.edit("ceiling.type.create", empty).await.is_err());
    stack
        .edit(
            "ceiling.type.layers",
            json!({ "type": "ct-own", "layers": [
                { "function": "finish", "thickness": "20 mm", "material": "plaster-gypsum" }] }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "ceiling.type.rename",
            json!({ "type": "ct-own", "name": "Board 20" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "ceiling.type.duplicate",
            json!({ "type": "ct-own", "new": "ct-copy", "name": "Board 20 copy" }),
        )
        .await
        .unwrap();
    stack
        .edit("ceiling.type.delete", json!({ "type": "ct-copy" }))
        .await
        .unwrap();
    // A floor type is no ceiling type, and the other way round.
    stack.edit("floor.defaults", json!({})).await.unwrap();
    assert!(
        stack
            .edit(
                "ceiling.type.layers",
                json!({ "type": "floor-concrete-200", "layers": [
                { "function": "finish", "thickness": "20 mm", "material": "plaster-gypsum" }] })
            )
            .await
            .is_err()
    );
    assert!(
        stack
            .edit(
                "floor.type.layers",
                json!({ "type": PLASTER, "layers": [
                { "function": "finish", "thickness": "20 mm", "material": "plaster-gypsum" }] })
            )
            .await
            .is_err()
    );
}

/// A sketched ceiling stands on its exposed face: tied 2700 mm over Ground, its 13 mm of plaster
/// runs from 2700 up to 2713 mm; a type of 58 mm from 2700 to 2758; a type's layers changed and
/// the tie moved rebuild it by regeneration alone.
#[tokio::test]
async fn a_sketched_ceiling_stands_on_its_exposed_face() {
    let stack = with_defaults(false).await;
    stack
        .edit(
            "ceiling.create",
            sketched("C-1", PLASTER, "2700 mm", rect(0, 0, 4000, 3000)),
        )
        .await
        .unwrap();
    let record = stack.record("C-1").await.unwrap();
    assert!(is_ceiling(&record));
    assert_eq!(
        area(&ceiling_boundary(&record.recipe).unwrap()).unwrap(),
        Area::from_mm2(4000 * 3000)
    );
    // Built in its own frame, its upper face at 0 and the plaster 13 mm below; its elevation is
    // that upper face, 13 mm over the 2700 it was tied at.
    let (low, high) = stack.bounds("C-1").await.unwrap();
    assert_eq!((low[2], high[2]), (-13, 0));
    assert_eq!(stack.built("C-1").await.elevation, Some(length_text(2713)));
    stack
        .edit(
            "ceiling.create",
            sketched("C-2", TIMBER, "2700 mm", rect(0, 4000, 4000, 3000)),
        )
        .await
        .unwrap();
    let (low, high) = stack.bounds("C-2").await.unwrap();
    assert_eq!((low[2], high[2]), (-58, 0));
    assert_eq!(stack.built("C-2").await.elevation, Some(length_text(2758)));
    // The type's layers changed: every ceiling of it stands on its face again.
    stack
        .edit(
            "ceiling.type.layers",
            json!({ "type": PLASTER, "layers": [
                { "function": "finish", "thickness": "20 mm", "material": "plaster-gypsum" }] }),
        )
        .await
        .unwrap();
    let (low, high) = stack.bounds("C-1").await.unwrap();
    assert_eq!((low[2], high[2]), (-20, 0));
    assert_eq!(stack.built("C-1").await.elevation, Some(length_text(2720)));
    // The tie moved: the exposed face follows it.
    stack
        .edit(
            "level.tie",
            json!({ "element": "C-1", "level": "L-g", "offset": "2600 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.built("C-1").await.elevation, Some(length_text(2620)));
    // Tied to First, 300 mm under it, it stands at 3 m less 300 mm.
    stack
        .edit(
            "level.tie",
            json!({ "element": "C-1", "level": "L-1", "offset": "-300 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.built("C-1").await.elevation, Some(length_text(2720)));
    // A new outline is checked and rebuilt.
    stack
        .edit(
            "ceiling.boundary",
            json!({ "element": "C-1", "boundary": rect(0, 0, 2000, 2000) }),
        )
        .await
        .unwrap();
    let record = stack.record("C-1").await.unwrap();
    assert_eq!(
        area(&ceiling_boundary(&record.recipe).unwrap()).unwrap(),
        Area::from_mm2(2000 * 2000)
    );
}

/// A ceiling placed by clicking inside four walls is outlined by the loop a room there has: the
/// same corners at the walls' faces, 3885 by 2885 mm, and it keeps its outline when the walls
/// change. Refused: a point in a wall, one not enclosed, both an outline and a point, neither.
#[tokio::test]
async fn a_ceiling_clicked_in_a_room_has_the_rooms_outline() {
    let stack = with_defaults(true).await;
    stack
        .edit("room.place", place("R-1", 2000, 1500))
        .await
        .unwrap();
    stack
        .edit("ceiling.create", clicked("C-1", "2700 mm", 2000, 1500))
        .await
        .unwrap();
    let record = stack.record("C-1").await.unwrap();
    let boundary = ceiling_boundary(&record.recipe).unwrap();
    assert_eq!(area(&boundary).unwrap(), Area::from_mm2(3885 * 2885));
    let Outcome::Enclosed { found, .. } = outcome(&stack, "R-1").await else {
        panic!("enclosed");
    };
    let mut room_corners: Vec<[i64; 2]> = found.outer.iter().map(|c| c.at).collect();
    let mut ceiling_corners: Vec<[i64; 2]> = boundary
        .corners(Ring::Outer)
        .unwrap()
        .iter()
        .map(|(_, p)| [p.x, p.y])
        .collect();
    room_corners.sort_unstable();
    ceiling_corners.sort_unstable();
    assert_eq!(ceiling_corners, room_corners);
    // The walls move on; the ceiling keeps what it was drawn as.
    stack
        .edit("element.delete", json!({ "element": "W-w" }))
        .await
        .unwrap();
    let after = stack.record("C-1").await.unwrap();
    assert_eq!(ceiling_boundary(&after.recipe).unwrap(), boundary);

    let stack = with_defaults(true).await;
    for (args, words) in [
        (clicked("C-9", "2700 mm", 0, 1500), "W-w"),
        (
            clicked("C-9", "2700 mm", 20_000, 20_000),
            "No closed loop of walls on that level round that point.",
        ),
    ] {
        let refused = stack.edit("ceiling.create", args).await.unwrap_err();
        assert!(refused.contains(words), "{refused}");
    }
    let mut both = clicked("C-9", "2700 mm", 2000, 1500);
    both["boundary"] = rect(0, 0, 1000, 1000);
    assert_eq!(
        stack.edit("ceiling.create", both).await.unwrap_err(),
        "A ceiling's outline is a boundary or a point in a room, not both."
    );
    let mut neither = clicked("C-9", "2700 mm", 2000, 1500);
    neither.as_object_mut().unwrap().remove("at");
    assert_eq!(
        stack.edit("ceiling.create", neither).await.unwrap_err(),
        "This command needs a boundary or a point in a room."
    );
    assert!(stack.record("C-9").await.is_none());
}

/// A room's volume is bounded by the ceiling over it. The box is 3885 by 2885 mm inside, 11 208 225
/// mm2, up to First at 3000 mm: 33 624 675 000 mm3. A ceiling over it at 2700 mm makes it 2700
/// high: 30 262 207 500 mm3, and its built shape reaches 2700. Two ceilings: the lower rules
/// (2400 mm). One above the room's own limit, one that does not hold its point and one in an
/// opening change nothing. Undone, the room is as it was.
#[tokio::test]
async fn a_room_is_bounded_by_the_ceiling_over_it() {
    let stack = with_defaults(true).await;
    stack
        .edit("room.place", place("R-1", 2000, 1500))
        .await
        .unwrap();
    let area = 3885 * 2885_i64;
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 3000)
    );
    assert_eq!(stack.bounds("R-1").await.unwrap().1[2], 3000);

    stack
        .edit("ceiling.create", clicked("C-1", "2700 mm", 2000, 1500))
        .await
        .unwrap();
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 2700)
    );
    assert_eq!(stack.bounds("R-1").await.unwrap().1[2], 2700);

    stack
        .edit(
            "ceiling.create",
            sketched("C-2", PLASTER, "2400 mm", rect(0, 0, 4000, 3000)),
        )
        .await
        .unwrap();
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 2400)
    );
    // The lower one moves up above the other, and the first rules again.
    stack
        .edit(
            "level.tie",
            json!({ "element": "C-2", "level": "L-g", "offset": "2900 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 2700)
    );
    // Above the room's own limit, a ceiling changes nothing.
    stack
        .edit(
            "level.tie",
            json!({ "element": "C-1", "level": "L-g", "offset": "3500 mm" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "level.tie",
            json!({ "element": "C-2", "level": "L-g", "offset": "3600 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 3000)
    );
    // A ceiling that does not hold the room's point (2 m, 1.5 m) bounds nothing.
    stack
        .edit(
            "ceiling.boundary",
            json!({ "element": "C-1", "boundary": rect(10_000, 10_000, 2000, 2000) }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "level.tie",
            json!({ "element": "C-1", "level": "L-g", "offset": "2000 mm" }),
        )
        .await
        .unwrap();
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 3000)
    );
    // An opening over the point: the ceiling is not over it there.
    let mut holed = rect(0, 0, 4000, 3000);
    let at = |x: i64, y: i64| json!({ "at": [format!("{x} mm"), format!("{y} mm")] });
    holed["holes"] = json!([[
        at(1500, 1000),
        at(1500, 2000),
        at(2500, 2000),
        at(2500, 1000)
    ]]);
    stack
        .edit(
            "ceiling.boundary",
            json!({ "element": "C-1", "boundary": holed }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "level.tie",
            json!({ "element": "C-2", "level": "L-g", "offset": "2800 mm" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "ceiling.boundary",
            json!({ "element": "C-2", "boundary": rect(0, 0, 1000, 1000) }),
        )
        .await
        .unwrap();
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 3000)
    );
    // Deleted, a ceiling no longer bounds anything; undone, it does again.
    stack
        .edit(
            "ceiling.boundary",
            json!({ "element": "C-2", "boundary": rect(0, 0, 4000, 3000) }),
        )
        .await
        .unwrap();
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 2800)
    );
    stack
        .edit("element.delete", json!({ "element": "C-2" }))
        .await
        .unwrap();
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 3000)
    );
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(
        volume_of(&stack, "R-1").await,
        Volume::from_mm3(area * 2800)
    );
}

/// The grid pattern: 600 by 1200 tiles over the L outline from its corner are 57 whole and 20
/// cut (construct-slabs' own count, worked by hand there), set at creation or later, turned,
/// moved and taken away; refused in words where it cannot be held.
#[tokio::test]
async fn a_ceilings_grid_is_set_changed_counted_and_cleared() {
    let stack = with_defaults(false).await;
    let mut args = sketched("C-1", PLASTER, "2700 mm", l_shape());
    args["grid"] = json!({ "width": "600 mm", "depth": "1200 mm" });
    stack.edit("ceiling.create", args).await.unwrap();
    let record = stack.record("C-1").await.unwrap();
    let grid = grid_of(&record.recipe).unwrap().unwrap();
    assert_eq!(
        (
            grid.width(),
            grid.depth(),
            grid.origin().x,
            grid.origin().y,
            grid.angle()
        ),
        (Length::from_mm(600), Length::from_mm(1200), 0, 0, 0)
    );
    let boundary = ceiling_boundary(&record.recipe).unwrap();
    let tiles = grid.tiles(&boundary).unwrap();
    assert_eq!((tiles.whole, tiles.cut), (57, 20));

    // Turned and moved: kept as given; only 0 and 90 are counted.
    stack
        .edit(
            "ceiling.grid",
            json!({ "element": "C-1", "grid": {
                "width": "600 mm", "depth": "1200 mm", "origin": ["300 mm", "0"], "angle": 45 } }),
        )
        .await
        .unwrap();
    let turned: Grid = grid_of(&stack.record("C-1").await.unwrap().recipe)
        .unwrap()
        .unwrap();
    assert_eq!(
        (turned.angle(), turned.origin().x),
        (45, Length::from_mm(300).steps())
    );
    assert!(turned.tiles(&boundary).is_err());

    for (args, words) in [
        (
            json!({ "element": "C-1", "grid": { "width": "0", "depth": "600 mm" } }),
            "a tile's width",
        ),
        (
            json!({ "element": "C-1", "grid": { "width": "600 mm", "depth": "600 mm", "angle": 180 } }),
            "whole number of degrees",
        ),
        (json!({ "element": "C-1" }), "This command needs a grid."),
    ] {
        let refused = stack.edit("ceiling.grid", args).await.unwrap_err();
        assert!(refused.contains(words), "{refused}");
    }
    stack
        .edit("ceiling.grid.clear", json!({ "element": "C-1" }))
        .await
        .unwrap();
    assert_eq!(
        grid_of(&stack.record("C-1").await.unwrap().recipe).unwrap(),
        None
    );
    assert_eq!(
        stack
            .edit("ceiling.grid.clear", json!({ "element": "C-1" }))
            .await
            .unwrap_err(),
        "C-1 has no grid to clear."
    );
}

/// Every refusal comes back in words and changes nothing.
#[tokio::test]
async fn a_ceiling_refuses_what_it_cannot_be() {
    let stack = with_defaults(false).await;
    stack.edit("floor.defaults", json!({})).await.unwrap();
    stack
        .edit(
            "ceiling.create",
            sketched("C-1", PLASTER, "2700 mm", rect(0, 0, 4000, 3000)),
        )
        .await
        .unwrap();
    let rows = stack.rows("piece").await;
    for (cmd, args, words) in [
        (
            "ceiling.create",
            sketched("C-1", PLASTER, "2700 mm", rect(0, 0, 4000, 3000)),
            "That element already exists.",
        ),
        (
            "ceiling.create",
            sketched(
                "C-9",
                "floor-concrete-200",
                "2700 mm",
                rect(0, 0, 4000, 3000),
            ),
            "category",
        ),
        (
            "ceiling.create",
            sketched("C-9", "nothing", "2700 mm", rect(0, 0, 4000, 3000)),
            "no type has id nothing",
        ),
        (
            "ceiling.create",
            json!({ "element": "C-9", "type": PLASTER, "offset": "2700 mm",
                    "boundary": rect(0, 0, 4000, 3000) }),
            "A ceiling stands over a level: this command needs a level.",
        ),
        (
            "ceiling.create",
            json!({ "element": "C-9", "type": PLASTER, "level": "L-nowhere",
                    "boundary": rect(0, 0, 4000, 3000) }),
            "L-nowhere",
        ),
        (
            "ceiling.create",
            sketched(
                "C-9",
                PLASTER,
                "2700 mm",
                json!({ "outer": [
                { "at": ["0", "0"] }, { "at": ["1 m", "0"] }] }),
            ),
            "C-9's boundary",
        ),
        (
            "ceiling.boundary",
            json!({ "element": "nobody", "boundary": rect(0, 0, 1000, 1000) }),
            "That element no longer exists.",
        ),
        (
            "ceiling.grid",
            json!({ "element": "nobody", "grid": { "width": "600 mm", "depth": "600 mm" } }),
            "That element no longer exists.",
        ),
        (
            "ceiling.type.delete",
            json!({ "type": "floor-concrete-200" }),
            "ceiling type",
        ),
        (
            "floor.boundary",
            json!({ "element": "C-1", "boundary": rect(0, 0, 1000, 1000) }),
            "C-1 is not a floor.",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert!(refused.contains(words), "{cmd}: {refused}");
    }
    assert_eq!(stack.rows("piece").await, rows);
}

/// The generic commands refuse a ceiling, naming the command to use; another person who is working
/// on a ceiling turns a change back.
#[tokio::test]
async fn the_generic_commands_refuse_a_ceiling_and_name_the_right_one() {
    let stack = with_defaults(false).await;
    stack
        .edit(
            "ceiling.create",
            sketched("C-1", PLASTER, "2700 mm", rect(0, 0, 4000, 3000)),
        )
        .await
        .unwrap();
    let recipe = stack.record("C-1").await.unwrap().recipe;
    for (cmd, args, words) in [
        (
            "element.create",
            json!({ "element": "C-9", "kind": "ceiling", "level": 0, "recipe": recipe }),
            "A ceiling is made with ceiling.create, which checks it as a ceiling.",
        ),
        (
            "element.set",
            json!({ "element": "C-1", "recipe": recipe }),
            "C-1's outline and grid are changed with ceiling.boundary, ceiling.grid or ceiling.grid.clear, which check it as a ceiling.",
        ),
        (
            "type.create",
            json!({ "type": "t-9", "name": "X", "category": 700 }),
            "A ceiling type is made with ceiling.type.create, which checks it as a ceiling.",
        ),
        (
            "type.rename",
            json!({ "type": PLASTER, "name": "X" }),
            "A ceiling type is renamed with ceiling.type.rename, which checks it as a ceiling.",
        ),
        (
            "type.delete",
            json!({ "type": PLASTER }),
            "A ceiling type is deleted with ceiling.type.delete, which checks it as a ceiling.",
        ),
    ] {
        assert_eq!(stack.edit(cmd, args).await.unwrap_err(), words, "{cmd}");
    }
    let grid = json!({ "element": "C-1", "grid": { "width": "600 mm", "depth": "600 mm" } });
    stack.edit("ceiling.grid", grid.clone()).await.unwrap();
    let refused = stack
        .edit_as("ben", "ceiling.grid.clear", json!({ "element": "C-1" }))
        .await;
    assert!(refused.unwrap_err().ends_with("is working on this."));
    stack.later();
    stack
        .edit_as("ben", "ceiling.grid.clear", json!({ "element": "C-1" }))
        .await
        .unwrap();
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
async fn every_ceiling_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    let layers =
        json!([{ "function": "finish", "thickness": "16 mm", "material": "plaster-gypsum" }]);
    for (cmd, args) in [
        ("ceiling.defaults", json!({})),
        (
            "ceiling.type.create",
            json!({ "type": "ct-own", "name": "Board", "layers": layers }),
        ),
        (
            "ceiling.type.layers",
            json!({ "type": "ct-own", "layers": [
                { "function": "finish", "thickness": "20 mm", "material": "plaster-gypsum" }] }),
        ),
        (
            "ceiling.type.rename",
            json!({ "type": "ct-own", "name": "Board 20" }),
        ),
        (
            "ceiling.type.duplicate",
            json!({ "type": "ct-own", "new": "ct-copy", "name": "Copy" }),
        ),
        ("ceiling.type.delete", json!({ "type": "ct-copy" })),
        (
            "ceiling.create",
            sketched("C-1", PLASTER, "2700 mm", rect(0, 0, 4000, 3000)),
        ),
        (
            "ceiling.boundary",
            json!({ "element": "C-1", "boundary": rect(0, 0, 2000, 2000) }),
        ),
        (
            "ceiling.grid",
            json!({ "element": "C-1", "grid": { "width": "600 mm", "depth": "1200 mm" } }),
        ),
        ("ceiling.grid.clear", json!({ "element": "C-1" })),
        ("element.delete", json!({ "element": "C-1" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = with_defaults(true).await;
    for (cmd, args) in [
        ("room.place", place("R-1", 2000, 1500)),
        ("ceiling.create", clicked("C-1", "2700 mm", 2000, 1500)),
        (
            "ceiling.grid",
            json!({ "element": "C-1", "grid": { "width": "600 mm", "depth": "1200 mm" } }),
        ),
        ("ceiling.create", clicked("C-1", "2400 mm", 2000, 1500)),
        (
            "level.tie",
            json!({ "element": "C-1", "level": "L-g", "offset": "2500 mm" }),
        ),
        ("edit.undo", json!({})),
        ("element.delete", json!({ "element": "W-w" })),
        ("ceiling.grid.clear", json!({ "element": "C-1" })),
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
    let incremental = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, incremental);
}

/// A ceiling the walls enclose is no warning; one sketched where no walls close round it is
/// `ceiling.not_enclosed`; and a wall taken away from the room a ceiling was clicked in makes it
/// one.
#[tokio::test]
async fn a_ceiling_no_walls_enclose_is_a_warning() {
    let stack = with_defaults(true).await;
    let warnings = |stack: &Stack| {
        let store = stack.store.clone();
        async move {
            let mut snapshot = store.snapshot().await.unwrap();
            construct_api::cloud::warnings::read_warnings(&mut snapshot, &i("p"), &i(DOCUMENT))
                .await
                .unwrap()
                .warnings
                .into_iter()
                .filter(|w| w.kind == "ceiling.not_enclosed")
                .map(|w| (w.identity, w.words))
                .collect::<Vec<_>>()
        }
    };
    stack
        .edit("ceiling.create", clicked("C-1", "2700 mm", 2000, 1500))
        .await
        .unwrap();
    assert_eq!(warnings(&stack).await.len(), 0);
    stack
        .edit(
            "ceiling.create",
            sketched("C-2", PLASTER, "2700 mm", rect(10_000, 10_000, 3000, 3000)),
        )
        .await
        .unwrap();
    assert_eq!(
        warnings(&stack).await,
        [(
            "ceiling.not_enclosed|C-2".to_owned(),
            "Ceiling C-2 is not enclosed: the walls of its level do not close round it".to_owned()
        )]
    );
    stack
        .edit("element.delete", json!({ "element": "W-w" }))
        .await
        .unwrap();
    let list = warnings(&stack).await;
    assert_eq!(
        list.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        ["ceiling.not_enclosed|C-1", "ceiling.not_enclosed|C-2"]
    );
}
