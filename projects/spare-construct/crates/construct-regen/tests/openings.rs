//! Openings and shafts through the log and the regenerator, on the test store: a floor opening is
//! hosted on its floor as a door is on its wall, and a shaft cuts every floor of its document that
//! it passes wholly through, as their levels stand when regeneration reaches them: a floor made
//! later inside its reach is cut too, a floor outside it is not, a level's move brings a floor
//! into its reach, and a shaft that ends inside a floor's thickness fails that floor by name. The
//! records go in as they are, through a command of this file's own that only stores them; the
//! commands that make openings and shafts are the server's (construct-api, a later piece).
//!
//! The builder is the stand-in, wrapped so a test can see what each host was handed: the cut
//! itself is the kernel suite's (`tests/kernel_suite`).

#![cfg(feature = "server")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::CategoryTable;
use construct_levels::{Length, Level, Tie};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{
    Built, Edition, FaceName, Geometry, GeometryError, Hosted, Recipe,
};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Model};
use construct_openings::{Named, Outline, floor_opening_record, shaft_record};
use construct_regen::hosting;
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_slabs::{Boundary, default_floor_types, floor_record};
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
const SLAB: &str = "floor-concrete-150-screed-50";
/// The role a test builder gives each element a host was handed, so a test reads it back.
const HANDED: &str = "handed";

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// The stand-in, whose host also lists, among its names, the key of each element it was handed.
struct Seen;

impl Geometry for Seen {
    fn edition(&self) -> Edition {
        Boxes.edition()
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        Boxes.build(recipe)
    }

    fn build_hosting(&self, host: &Recipe, hosted: &[Hosted<'_>]) -> Result<Built, GeometryError> {
        let mut built = Boxes.build_hosting(host, hosted)?;
        for h in hosted {
            built.names.push(FaceName {
                name: format!("{}@{},{}", h.key, h.at.x, h.at.y),
                role: HANDED.to_owned(),
            });
        }
        Ok(built)
    }
}

/// The only commands here: `put` stores a record as it is, and `model` makes the document.
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
        let regen = Regenerator::new(store.clone(), MemoryFiles::default(), Arc::new(Seen));
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

    /// What a host was handed when last built (`key@x,y`, its place in the host's frame in
    /// millimetres), or its failure's code and words.
    async fn handed(&self, id: &str) -> Result<Vec<String>, String> {
        match self.built(id).await.result {
            BuiltResult::Shape { names, .. } => Ok(names
                .into_iter()
                .filter(|n| n.role == HANDED)
                .map(|n| n.name)
                .collect()),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
    }
}

fn mm(v: i64) -> Length {
    Length::checked_from_mm(v).unwrap()
}

fn tie(level: &str, offset: i64) -> Tie {
    Tie {
        level: i(level),
        offset: mm(offset),
    }
}

/// An 8 m square floor `id` of the 200 mm slab type, its top on `level`.
fn floor(id: &str, level: &str, offset: i64) -> Element {
    let at = |id: &str, x: i64, y: i64| json!({ "id": id, "at": [x * 6400, y * 6400] });
    let boundary = Boundary::from_plain(&json!({
        "outer": [at("e0", 0, 0), at("e1", 8000, 0), at("e2", 8000, 8000), at("e3", 0, 8000)],
    }))
    .unwrap();
    floor_record(
        &i(id),
        &i(DOCUMENT),
        id,
        &i(SLAB),
        &tie(level, offset),
        &boundary,
    )
}

fn square(from: [i64; 2], size: i64) -> Outline {
    let [x, y] = from;
    Outline::from_mm(
        &Named::opening("test"),
        &[[x, y], [x + size, y], [x + size, y + size], [x, y + size]],
    )
    .unwrap()
}

/// Shaft `id`, a 1 m square placed at `at` millimetres, from `base` up to `top`.
fn shaft(id: &str, at: [i64; 2], base: &Tie, top: &Tie) -> Element {
    let mut made = shaft_record(&i(id), &i(DOCUMENT), id, &square([0, 0], 1000), base, top);
    made.placement = Placement {
        x: at[0],
        y: at[1],
        z: 0,
    };
    made
}

/// A shaft's height is read from its top tie, filled as a layered wall's is.
#[test]
fn a_shafts_height_is_read_as_a_walls_is() {
    assert_eq!(
        construct_openings::top_arg(),
        construct_regen::levels::top_arg()
    );
}

/// Through the log: a shaft from level 0 up to level 2 cuts the floors on levels 1 and 2 and not
/// the one on level 0 (whose top it only touches) nor the one high above; a floor made later
/// inside its reach is cut too; a level's move brings a floor into its reach; a second shaft
/// ending inside a floor's thickness fails that floor by name, and once its top is moved to the
/// floor's top face the floor is cut by both. A floor opening hosted on a floor's top face is
/// handed to that floor at its own outline, and moves with it.
#[tokio::test]
async fn a_shaft_cuts_every_floor_it_passes_through() {
    let stack = Stack::new().await;
    let ty = default_floor_types(&CategoryTable::new())
        .unwrap()
        .into_iter()
        .find(|ty| ty.id().as_str() == SLAB)
        .unwrap();
    stack.put(&ty.to_record(&i(DOCUMENT))).await;
    for (id, at) in [("L-0", 0), ("L-1", 3200), ("L-2", 6400), ("L-9", 20_000)] {
        let level = Level::new(i(id), id, mm(at), true).unwrap();
        stack.put(&level.to_record(&i(DOCUMENT))).await;
    }
    for (id, level) in [
        ("F-0", "L-0"),
        ("F-1", "L-1"),
        ("F-2", "L-2"),
        ("F-9", "L-9"),
    ] {
        stack.put(&floor(id, level, 0)).await;
    }
    stack
        .put(&shaft("S-1", [5000, 1000], &tie("L-0", 0), &tie("L-2", 0)))
        .await;
    let cut = Ok(vec!["S-1@5000,1000".to_owned()]);
    let none = Ok(Vec::new());
    assert_eq!(stack.handed("F-0").await, none, "its top only touches");
    assert_eq!(stack.handed("F-1").await, cut);
    assert_eq!(stack.handed("F-2").await, cut);
    assert_eq!(stack.handed("F-9").await, none, "above its reach");

    stack.put(&floor("F-4", "L-1", -1000)).await;
    assert_eq!(stack.handed("F-4").await, cut, "a floor made later");

    let lower = Level::new(i("L-9"), "L-9", mm(5000), true).unwrap();
    stack.put(&lower.to_record(&i(DOCUMENT))).await;
    assert_eq!(stack.handed("F-9").await, cut, "its level moved into reach");

    stack
        .put(&shaft(
            "S-2",
            [2000, 5000],
            &tie("L-0", 0),
            &tie("L-1", -100),
        ))
        .await;
    assert_eq!(
        stack.handed("F-1").await,
        Err(
            "ShaftEndsInFloor: shaft S-2: its top lies inside the thickness of floor F-1; a shaft \
             passes wholly through a floor or misses it, so move its top to the floor's top or \
             bottom face or past it"
                .to_owned()
        )
    );
    stack
        .put(&shaft("S-2", [2000, 5000], &tie("L-0", 0), &tie("L-1", 0)))
        .await;
    assert_eq!(
        stack.handed("F-1").await,
        Ok(vec!["S-1@5000,1000".to_owned(), "S-2@2000,5000".to_owned()])
    );

    let mut opening =
        floor_opening_record(&i("O-1"), &i(DOCUMENT), "Hatch", &square([1000, 1000], 500));
    let face = FaceName {
        name: "box.top".to_owned(),
        role: "top".to_owned(),
    };
    hosting::write_in_plan(&mut opening, &i("F-0"), &face);
    assert_eq!(
        hosting::read(&opening).unwrap(),
        Some(hosting::Hosting::InPlan {
            host: i("F-0"),
            face: face.name.clone()
        })
    );
    stack.put(&opening).await;
    assert_eq!(stack.handed("F-0").await, Ok(vec!["O-1@0,0".to_owned()]));
    let mut moved = floor("F-0", "L-0", 0);
    moved.placement = Placement { x: 100, y: 0, z: 0 };
    stack.put(&moved).await;
    assert_eq!(stack.handed("F-0").await, Ok(vec!["O-1@0,0".to_owned()]));
    match stack.built("O-1").await.result {
        BuiltResult::Shape { bounds, .. } => assert_eq!(bounds.max, [1500, 1500, 0]),
        BuiltResult::Failed { code, message } => panic!("{code}: {message}"),
    }
}
