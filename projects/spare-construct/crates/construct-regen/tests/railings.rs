//! A railing through the log and the regenerator, on the test store: a sketched railing built as
//! its pieces; a railing on a stair laid out from the stair as its levels lay it out, laid out
//! again when a level moves or the stair changes (its record never written), a build failure in
//! words when its stair is not in the model; and read back as read-only parameters and warnings.
//! The records go in as they are, through a command of this file's own that only stores them: the
//! commands that make railings are the server's (construct-api's `cloud::railings`).
//!
//! The builder is the stand-in, which builds a railing as the box around its pieces and draws each
//! one; the kernel's own railing is the kernel suite's (`tests/kernel_suite`).

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
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Model, Reads};
use construct_params::Units;
use construct_railings::{
    Path, Railing, RailingType, Settings as RailingSettings, Side, railing_record,
};
use construct_regen::parameters::At;
use construct_regen::railings;
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_stairs::{Direction, RunSpec, Settings, Stair, stair_record};
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

/// A main private stair from the origin towards +x, 1000 mm wide, on its least going, one run,
/// its base on L-1 and its top on L-2.
fn stair(direction: Direction) -> Element {
    let s = Stair::new(Settings {
        rules: "nz-d1-as1".to_owned(),
        category: "main-private".to_owned(),
        start: [mm(0), mm(0)],
        direction,
        width: mm(1000),
        going: None,
        waist: mm(150),
        runs: vec![RunSpec::default()],
        landings: Vec::new(),
    })
    .unwrap();
    stair_record(&i("S-1"), &i(DOCUMENT), "S-1", &tie("L-1"), &tie("L-2"), &s)
}

fn tie(level: &str) -> Tie {
    Tie {
        level: i(level),
        offset: mm(0),
    }
}

/// A railing `id` on its base L-1 along `path`, of the standard type with balusters `spacing` mm
/// apart.
fn railing(id: &str, path: Path, spacing: i64) -> Element {
    let mut kind = RailingType::standard();
    kind.balusters.spacing = mm(spacing);
    let category = if matches!(path, Path::OnStair { .. }) {
        "household-stair"
    } else {
        "household-edge"
    };
    let railing = Railing::new(RailingSettings {
        rules: "nz-f4-as1".to_owned(),
        category: category.to_owned(),
        openings: "children".to_owned(),
        kind,
        path,
        flipped: false,
    })
    .unwrap();
    railing_record(&i(id), &i(DOCUMENT), id, &tie("L-1"), &railing)
}

fn on_stair() -> Path {
    Path::OnStair {
        stair: i("S-1"),
        side: Side::Right,
        inset: mm(50),
    }
}

/// The levels: Ground at 0 and First at `first` mm.
async fn levels(stack: &Stack, first: i64) {
    let one = Level::new(i("L-1"), "Ground", mm(0), true).unwrap();
    let two = Level::new(i("L-2"), "First", mm(first), true).unwrap();
    stack.put(&one.to_record(&i(DOCUMENT))).await;
    stack.put(&two.to_record(&i(DOCUMENT))).await;
}

/// A sketched railing, 3 m at 100 mm, is built as its pieces: the box round its two 50 mm posts,
/// one rail and 29 balusters, each drawn as a part in its material with its function.
#[tokio::test]
async fn a_sketched_railing_is_built_as_its_pieces() {
    let stack = Stack::new().await;
    levels(&stack, 3060).await;
    let points = vec![[mm(0), mm(0)], [mm(3000), mm(0)]];
    stack
        .put(&railing("R-1", Path::Sketched { points }, 100))
        .await;
    assert_eq!(
        stack.bounds("R-1").await.unwrap(),
        ([-25, -25, 0], [3025, 25, 1000])
    );
    let BuiltResult::Shape { displays, .. } = stack.built("R-1").await.result else {
        panic!("built");
    };
    assert_eq!(displays.len(), 2);
}

/// A railing on the stair of 17 risers follows it: its last post at the top nosing, 4480 mm
/// along and 3060 mm up, its rail 1000 mm above. First moved to 2700 mm lays the stair out in 15
/// risers, 3920 mm long, and the railing with it, its record unchanged; the stair turned to run
/// towards +y lays it out along y. Its `reads` row lists the stair and the stair's levels.
#[tokio::test]
async fn a_railing_follows_its_stair() {
    let stack = Stack::new().await;
    levels(&stack, 3060).await;
    stack.put(&stair(Direction::PlusX)).await;
    stack.put(&railing("R-1", on_stair(), 110)).await;
    let (min, max) = stack.bounds("R-1").await.unwrap();
    assert_eq!((min, max), ([-25, 25, 0], [4505, 75, 4060]));
    let input = stack.built("R-1").await.input;
    let reads = Place::Reads {
        project: i(PROJECT),
        document: i(DOCUMENT),
        element: i("R-1"),
    };
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let row: Reads = read_row(&mut snapshot, &reads.key())
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.reads("S-1") && row.reads("L-1") && row.reads("L-2"),
        "{row:?}"
    );

    let moved = Level::new(i("L-2"), "First", mm(2700), true).unwrap();
    stack.put(&moved.to_record(&i(DOCUMENT))).await;
    assert_eq!(stack.bounds("R-1").await.unwrap().1, [3945, 75, 3700]);
    assert_eq!(
        stack.built("R-1").await.input,
        input,
        "its record is not written"
    );

    stack.put(&stair(Direction::PlusY)).await;
    let (min, max) = stack.bounds("R-1").await.unwrap();
    assert_eq!((min, max), ([-75, -25, 0], [-25, 3945, 3700]));
}

/// A railing whose stair is not in the model is a build failure naming it; its record stays.
#[tokio::test]
async fn a_railing_without_its_stair_is_a_failure_in_words() {
    let stack = Stack::new().await;
    levels(&stack, 3060).await;
    stack.put(&railing("R-1", on_stair(), 110)).await;
    assert_eq!(
        stack.bounds("R-1").await.unwrap_err(),
        "HostMissing: its stair S-1 is not in the model"
    );
}

/// What a page reads of a railing: its quantities shown in units, read-only, and its warnings.
#[tokio::test]
async fn a_railing_reads_back_its_quantities_and_warnings() {
    let stack = Stack::new().await;
    levels(&stack, 3060).await;
    let points = vec![[mm(0), mm(0)], [mm(3000), mm(0)]];
    let element = railing("R-1", Path::Sketched { points }, 130);
    stack.put(&element).await;
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let (project, document) = (i(PROJECT), i(DOCUMENT));
    let rows = railings::read_only_at(
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
    assert_eq!(
        shown[0],
        ("railing.rail-length", "read-only", Some("3000 mm"))
    );
    assert_eq!(shown[2], ("railing.balusters", "read-only", Some("23")));
    let warnings = railings::warnings_at(&mut snapshot, &project, &document, &element, At::Now)
        .await
        .unwrap();
    assert_eq!(
        warnings.iter().map(|w| w.identity()).collect::<Vec<_>>(),
        vec!["railing.opening|R-1"]
    );
    let now =
        construct_regen::levels::recipe_at(&mut snapshot, &project, &document, &element, At::Now)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(now.args["pieces"].as_array().unwrap().len(), 26);
}
