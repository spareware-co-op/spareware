//! A stair through the log and the regenerator, on the test store: its rise read from the height
//! between its levels and its layout and pieces worked out from it, built again when a level
//! moves (its record never written), refused in words when its levels give a height no count of
//! risers lays out, and read back as read-only parameters. The records go in as they are, through
//! a command of this file's own that only stores them: the commands that make stairs are the
//! server's (construct-api's `cloud::stairs`).
//!
//! The builder is the stand-in, which builds a stair as the box around its pieces and draws each
//! one; the kernel's own stair is the kernel suite's (`tests/kernel_suite`).

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

use construct_levels::{Length, Level, Tie};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Mesh, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Model};
use construct_params::{Units, Value};
use construct_regen::parameters::At;
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_regen::{levels, stairs};
use construct_stairs::{
    Direction, LandingSpec, RunSpec, Settings, Stair, Turn, stair_recipe, stair_record,
};
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Clock, Commands, Context as Moment, Orderers, Outcome, Reply};
use serde_json::{Value as Json, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const PROJECT: &str = "p";
const DOCUMENT: &str = "doc";

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(v: i64) -> Length {
    Length::checked_from_mm(v).unwrap()
}

/// The only commands here: `put` stores a record as it is (`{"record": ...}`), and `model`
/// makes the document on the stand-in's edition.
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
}

/// A stair's settings: of `category`, from the origin towards +x, 1000 mm wide, the least going,
/// a 150 mm waist, one run.
fn settings(category: &str) -> Settings {
    Settings {
        rules: "nz-d1-as1".to_owned(),
        category: category.to_owned(),
        start: [mm(0), mm(0)],
        direction: Direction::PlusX,
        width: mm(1000),
        going: None,
        waist: mm(150),
        runs: vec![RunSpec::default()],
        landings: Vec::new(),
    }
}

/// An L: two runs sharing the risers, a 1000 mm landing turning left.
fn l_stair() -> Stair {
    let mut s = settings("common");
    s.runs = vec![RunSpec::default(); 2];
    s.landings = vec![LandingSpec {
        length: mm(1000),
        turn: Turn::Left,
    }];
    Stair::new(s).unwrap()
}

/// Stair `id`, its base on `base` and its top on `top`, both at no offset.
fn stair(id: &str, base: &str, top: &str, s: &Stair) -> Element {
    let tie = |level: &str| Tie {
        level: i(level),
        offset: mm(0),
    };
    stair_record(&i(id), &i(DOCUMENT), id, &tie(base), &tie(top), s)
}

/// A stair's recipe with its rise read: as the levels' reader fills it.
fn with_rise(s: &Stair, rise: i64) -> Recipe {
    let mut recipe = stair_recipe(s);
    recipe
        .args
        .insert("rise".to_owned(), json!(Value::Length(mm(rise)).stored()));
    recipe
}

/// Six times the signed volume a mesh holds, in cubic millimetres.
fn six_volume(mesh: &Mesh) -> i128 {
    let point = |k: u32| -> [i128; 3] {
        let k = usize::try_from(k).unwrap() * 3;
        [0, 1, 2].map(|c| i128::from(mesh.positions[k + c]))
    };
    mesh.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let [p, q, r] = [point(t[0]), point(t[1]), point(t[2])];
            p[0] * (q[1] * r[2] - q[2] * r[1])
                + p[1] * (q[2] * r[0] - q[0] * r[2])
                + p[2] * (q[0] * r[1] - q[1] * r[0])
        })
        .sum()
}

/// A stair's recipe is laid out from its rise and filled with its pieces, as construct-stairs'
/// `filled` writes it; a recipe whose rise is not read yet, and one that is not a stair's, are
/// returned as they are; a rise no count lays out fails in construct-stairs' words (two runs of
/// two risers at least: four risers of 150 mm, 600 mm, is the least height that works).
#[test]
fn a_stairs_recipe_is_laid_out_from_its_rise() {
    let recipe = with_rise(&l_stair(), 2700);
    let filled = stairs::filled(&recipe).unwrap();
    assert_eq!(filled, construct_stairs::filled(&recipe).unwrap());
    assert_eq!(filled.args["pieces"].as_array().unwrap().len(), 3);
    let unread = stair_recipe(&l_stair());
    assert_eq!(stairs::filled(&unread).unwrap(), unread);
    let wall = Recipe::construct("wall.straight", std::collections::BTreeMap::default());
    assert_eq!(stairs::filled(&wall).unwrap(), wall);
    let low = stairs::filled(&with_rise(&l_stair(), 400)).unwrap_err();
    assert_eq!(low.code, "BadStair");
    assert!(
        low.message
            .ends_with("at least 4 risers, two to each run: the lowest a riser may be is 150 mm (D1/AS1 4.1.1 and Figure 11 (the least riser height it charts)), and the highest a riser may be is 190 mm (D1/AS1 4.1.1 and Table 6); the nearest floor-to-floor height that works is 600 mm."),
        "{}",
        low.message
    );
}

/// The stand-in builds a stair as the box around its pieces, each drawn as a part in concrete: the
/// L of 2700 mm (risers of 180 mm, runs of 8 and 7 on a 280 mm going) reaches 2960 mm along x and
/// 2680 mm along y, and up to its last inner corner, 2520 mm. Every piece is drawn closed, its
/// faces outward: the run is its profile's strips, each a quadrilateral to the nearest
/// millimetre, so its volume is within a millimetre's rounding of the run's.
#[test]
fn the_stand_in_draws_each_piece() {
    let filled = stairs::filled(&with_rise(&l_stair(), 2700)).unwrap();
    let built = Boxes.build(&filled).unwrap();
    assert_eq!(built.bounds.min, [0, 0, 0]);
    assert_eq!(built.bounds.max, [2960, 2680, 2520]);
    let display = &built.displays[0];
    assert_eq!(display.parts.len(), 3);
    assert!(
        display
            .parts
            .iter()
            .all(|p| p.material == "concrete-regular")
    );
    assert_eq!(
        display.outline,
        construct_regen::stair_plan::StairPlan::of(&filled)
            .unwrap()
            .pieces[0]
            .plan
            .iter()
            .map(|p| [p[0] / 6400, p[1] / 6400])
            .collect::<Vec<_>>()
    );
    // The landing: 1000 by 1000 by 150 mm.
    let [first, count] = display.parts[1].triangles;
    let landing = Mesh {
        positions: display.mesh.positions.clone(),
        indices: display.mesh.indices[first as usize * 3..(first as usize + count as usize) * 3]
            .to_vec(),
    };
    assert_eq!(six_volume(&landing), 6 * 1000 * 1000 * 150);
    assert!(six_volume(&display.mesh) > 0);
    // 7 treads, a tread and a riser each, and 4 more faces; the landing's 6; 6 treads and 4.
    assert_eq!(built.names.len(), 18 + 6 + 16);
    assert!(
        built
            .names
            .iter()
            .all(|n| n.name == format!("box.{}", n.role))
    );
    assert_eq!(
        construct_regen::stand_in::names(&filled).unwrap(),
        built.names
    );
}

/// Through the log: a main private stair from level 1 (0 mm) to level 2 (2500 mm) has 14 risers
/// and runs 13 x 280 = 3640 mm; level 2 moved to 2700 mm lays it out again with 15 risers, 14 x
/// 280 = 3920 mm long, its record unchanged; moved to 250 mm it fails, naming the nearest height
/// that works (two risers of 150 mm); its row stands it on its base level.
#[tokio::test]
async fn a_stair_follows_its_levels() {
    let stack = Stack::new().await;
    let one = Level::new(i("L-1"), "Ground", mm(0), true).unwrap();
    let two = Level::new(i("L-2"), "First", mm(2500), true).unwrap();
    stack.put(&one.to_record(&i(DOCUMENT))).await;
    stack.put(&two.to_record(&i(DOCUMENT))).await;
    let s = Stair::new(settings("main-private")).unwrap();
    stack.put(&stair("S-1", "L-1", "L-2", &s)).await;
    let (_, max) = stack.bounds("S-1").await.unwrap();
    assert_eq!(max[0], 3640);
    let row = stack.built("S-1").await;
    assert_eq!(row.level, Some(i("L-1")));
    let input = row.input;

    let moved = Level::new(i("L-2"), "First", mm(2700), true).unwrap();
    stack.put(&moved.to_record(&i(DOCUMENT))).await;
    let (_, max) = stack.bounds("S-1").await.unwrap();
    assert_eq!(max[0], 3920);
    assert_eq!(
        stack.built("S-1").await.input,
        input,
        "its record is not written"
    );

    let low = Level::new(i("L-2"), "First", mm(250), true).unwrap();
    stack.put(&low.to_record(&i(DOCUMENT))).await;
    let failed = stack.bounds("S-1").await.unwrap_err();
    assert!(
        failed.starts_with("BadStair: No count of risers"),
        "{failed}"
    );
    assert!(
        failed.ends_with("the nearest floor-to-floor height that works is 300 mm."),
        "{failed}"
    );
}

/// What a page reads of a stair: its layout as the document stands, its read-only quantities
/// shown in units, and the recipe it is built from now (its pieces).
#[tokio::test]
async fn a_stair_reads_back_its_layout_and_quantities() {
    let stack = Stack::new().await;
    let one = Level::new(i("L-1"), "Ground", mm(0), true).unwrap();
    let two = Level::new(i("L-2"), "First", mm(2500), true).unwrap();
    stack.put(&one.to_record(&i(DOCUMENT))).await;
    stack.put(&two.to_record(&i(DOCUMENT))).await;
    let element = stair(
        "S-1",
        "L-1",
        "L-2",
        &Stair::new(settings("common")).unwrap(),
    );
    stack.put(&element).await;
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let (project, document) = (i(PROJECT), i(DOCUMENT));
    let (_, layout) = stairs::layout_at(&mut snapshot, &project, &document, &element, At::Now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(layout.riser_count(), 14);
    let rows = stairs::read_only_at(
        &mut snapshot,
        &project,
        &document,
        &element,
        &Units::default(),
        At::Now,
    )
    .await
    .unwrap();
    let shown: Vec<(&str, &str, Option<&str>)> = rows
        .iter()
        .map(|r| (r.id.as_str(), r.from, r.shown.as_deref()))
        .collect();
    assert_eq!(shown[0], ("stair.rise", "read-only", Some("2500 mm")));
    assert_eq!(shown[1], ("stair.risers", "read-only", Some("14")));
    // Shown in the document's units (whole millimetres by default); the value is exact.
    assert_eq!(
        shown[2],
        ("stair.riser-height", "read-only", Some("179 mm"))
    );
    assert_eq!(rows[2].value.as_deref(), Some("length:q0:1142858"));
    assert_eq!(shown[4], ("stair.going", "read-only", Some("280 mm")));
    assert_eq!(shown[5], ("stair.run-length", "read-only", Some("3640 mm")));
    assert_eq!(rows.len(), 8);
    let now = levels::recipe_at(&mut snapshot, &project, &document, &element, At::Now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(now.args["pieces"].as_array().unwrap().len(), 1);
    // An element that is not a stair reads back nothing here.
    let level = one.to_record(&document);
    let none = stairs::read_only_at(
        &mut snapshot,
        &project,
        &document,
        &level,
        &Units::default(),
        At::Now,
    )
    .await
    .unwrap();
    assert_eq!(none.len(), 0);
}
