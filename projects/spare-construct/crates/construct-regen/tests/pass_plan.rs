//! What a pass of the regenerator reads its neighbours from: the grid of walls' lines finds every
//! wall a full read finds, and no other ([`Grid::within_two`] against every pair tested, as the
//! joins search tested them before there was a grid, over lines at every slope and length, ends
//! within the tolerance and just beyond it); a box finds every line whose box it meets; a value
//! kept with the rows it read is given back exactly while they read the same, and the log of the
//! kind index's writes is what decides that without reading them; the walls kept between passes
//! are taken only where regeneration still stands; and an outline the room search refuses is
//! known for it.

#![cfg(feature = "server")]
// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_model::keys::Place;
use construct_params::Length;
use construct_regen::pass_plan::{Documents, Kept, Outlines, box_of, boxes_meet};
use construct_regen::read_memo::{Memo, Recording, Written};
use construct_regen::wall_index::{Grid, Line};
use construct_walls::lines_meet;
use kit_core::Id;
use kit_store::memory::Memory;
use kit_store::{Kv, Txn};
use std::collections::{BTreeMap, BTreeSet};

/// Steps of the grid in a millimetre.
const MM: i64 = Length::STEPS_PER_MM;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// A line from `a` to `b`, in steps.
fn line(a: [i64; 2], b: [i64; 2]) -> Line {
    [a.map(Length::from_steps), b.map(Length::from_steps)]
}

/// Numbers that look random and are the same on every run (a linear congruential generator).
struct Numbers(u64);

impl Numbers {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    /// A number from `low` to `high`, both in.
    fn between(&mut self, low: i64, high: i64) -> i64 {
        low + (self.next() % ((high - low + 1) as u64)) as i64
    }
}

/// Lines in a 60 m square: along an axis and at a slant, short and long, some longer than the
/// grid lists cell by cell, and many starting where another ends, at it, within the joins
/// tolerance of it (0.01 mm, 64 steps) or just beyond.
fn lines() -> Vec<Line> {
    let mut numbers = Numbers(7);
    let mut out: Vec<Line> = Vec::new();
    for k in 0..400 {
        let start = match (k % 4, out.last()) {
            // From the end of the last, or near it.
            (0 | 1, Some(last)) => {
                let [x, y] = last[1].map(Length::steps);
                let off = [0, 40, 63, 64, 65, 90][numbers.between(0, 5) as usize];
                [x + off, y]
            }
            _ => [
                numbers.between(0, 60_000) * MM,
                numbers.between(0, 60_000) * MM,
            ],
        };
        let length = if k % 50 == 0 {
            250_000 * MM
        } else {
            numbers.between(200, 9000) * MM
        };
        let end = match numbers.between(0, 3) {
            0 => [start[0] + length, start[1]],
            1 => [start[0], start[1] - length],
            2 => [start[0] + length * 3 / 5, start[1] + length * 4 / 5],
            _ => [
                start[0] - length / 7,
                start[1] + numbers.between(1, 9000) * MM,
            ],
        };
        out.push(line(start, end));
    }
    out
}

/// Within two meetings, as the joins search found it reading every wall.
fn every_pair(lines: &[Line], me: usize) -> BTreeSet<usize> {
    let meets = |a: Line| {
        (0..lines.len())
            .filter(move |&j| j != me && lines_meet(a, lines[j]))
            .collect::<BTreeSet<_>>()
    };
    let first = meets(lines[me]);
    let mut all = first.clone();
    for &j in &first {
        all.extend(meets(lines[j]));
    }
    all
}

#[test]
fn the_grid_finds_every_wall_within_two_meetings_and_no_other() {
    let lines = lines();
    let mut grid: Grid<usize> = Grid::default();
    for (k, l) in lines.iter().enumerate() {
        grid.insert(k, None, Some(*l));
    }
    let mut met = 0;
    for (k, l) in lines.iter().enumerate() {
        let found = grid.within_two(None, *l, &k);
        assert_eq!(found, every_pair(&lines, k), "line {k}");
        met += found.len();
    }
    // The lines meet often enough for the test to mean something.
    assert!(met > 400, "{met}");
    // A line taken out is found no more, and one put back is.
    grid.remove(&3);
    assert!(
        lines
            .iter()
            .enumerate()
            .all(|(k, l)| !grid.within_two(None, *l, &k).contains(&3))
    );
    grid.insert(3, None, Some(lines[3]));
    assert_eq!(grid.within_two(None, lines[4], &4), every_pair(&lines, 4));
    // A line on another level meets nothing here.
    let level = Some(i("L-1"));
    let mut other: Grid<usize> = Grid::default();
    other.insert(0, level.clone(), Some(lines[0]));
    other.insert(1, None, Some(lines[0]));
    assert_eq!(
        other.within_two(level.as_ref(), lines[0], &9),
        BTreeSet::from([0])
    );
}

#[test]
fn a_box_finds_every_line_whose_box_it_meets() {
    let lines = lines();
    let mut grid: Grid<usize> = Grid::default();
    for (k, l) in lines.iter().enumerate() {
        grid.insert(k, None, Some(*l));
    }
    let mut numbers = Numbers(11);
    for _ in 0..200 {
        let [x, y] = [
            numbers.between(-5000, 65_000) * MM,
            numbers.between(-5000, 65_000) * MM,
        ];
        let half = numbers.between(0, 12_000) * MM;
        let area = ([x - half, y - half], [x + half, y + half]);
        let wanted: BTreeSet<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                let corners: Vec<[i64; 2]> = l.iter().map(|p| p.map(Length::steps)).collect();
                boxes_meet(box_of(&corners).unwrap(), area)
            })
            .map(|(k, _)| k)
            .collect();
        let [low, high] = [
            area.0.map(Length::from_steps),
            area.1.map(Length::from_steps),
        ];
        assert_eq!(grid.in_box(None, low, high), wanted);
    }
}

/// A record whose work reads rows `keys`.
fn record(id: &str) -> Element {
    Element {
        id: i(id),
        document: i("doc"),
        kind: "wall".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct("wall.layered", BTreeMap::new()),
        props: BTreeMap::new(),
        references: Vec::new(),
    }
}

/// A row of the kind index, and a row of another table.
fn keys() -> (kit_store::Key, kit_store::Key) {
    let kind = Place::Kind {
        project: i("p"),
        document: i("doc"),
        kind: i("type"),
        element: i("T-1"),
    };
    let built = Place::Built {
        project: i("p"),
        document: i("doc"),
        element: i("W-1"),
    };
    (kind.key(), built.key())
}

async fn put(store: &Memory, key: &kit_store::Key, value: &[u8]) {
    let mut txn = store.begin().await.unwrap();
    txn.put(key.clone(), value.to_vec()).await.unwrap();
    txn.commit().await.unwrap();
}

/// Works out a value from `key`'s row, as a build reads its type, and keeps it.
async fn worked(
    store: &Memory,
    memo: &mut Memo<Vec<u8>>,
    key: &kit_store::Key,
    written: Option<&Written>,
) -> Vec<u8> {
    let mut snapshot = store.snapshot().await.unwrap();
    if let Some(kept) = memo
        .kept(&mut snapshot, &record("W-1"), written)
        .await
        .unwrap()
    {
        return kept;
    }
    let mut recording = Recording::new(&mut snapshot);
    let value = kit_store::Reader::get(&mut recording, key)
        .await
        .unwrap()
        .unwrap_or_default();
    let noted = recording.noted();
    memo.keep(&record("W-1"), noted, value.clone(), written);
    value
}

#[tokio::test]
async fn a_kept_value_is_given_back_only_while_what_it_read_reads_the_same() {
    let (kind, built) = keys();
    for key in [&kind, &built] {
        let store = Memory::new();
        put(&store, key, b"one").await;
        let mut memo: Memo<Vec<u8>> = Memo::default();
        assert_eq!(worked(&store, &mut memo, key, None).await, b"one");
        // Read again and the same: kept.
        let mut snapshot = store.snapshot().await.unwrap();
        assert_eq!(
            memo.kept(&mut snapshot, &record("W-1"), None)
                .await
                .unwrap(),
            Some(b"one".to_vec())
        );
        // Another record of the element: not kept for it.
        let mut other = record("W-1");
        other.name = "Other".to_owned();
        assert_eq!(memo.kept(&mut snapshot, &other, None).await.unwrap(), None);
        // The row changed: worked out again.
        put(&store, key, b"two").await;
        assert_eq!(worked(&store, &mut memo, key, None).await, b"two");
    }
}

#[tokio::test]
async fn the_log_of_the_kind_index_says_when_a_kept_value_must_be_read_again() {
    let (kind, built) = keys();
    let store = Memory::new();
    put(&store, &kind, b"one").await;
    put(&store, &built, b"one").await;
    let mut written = Written::default();
    let mut memo: Memo<Vec<u8>> = Memo::default();
    assert_eq!(
        worked(&store, &mut memo, &kind, Some(&written)).await,
        b"one"
    );
    // The kind index's row written and logged: read again, and the change seen.
    put(&store, &kind, b"two").await;
    written.wrote(kind.clone());
    assert_eq!(
        worked(&store, &mut memo, &kind, Some(&written)).await,
        b"two"
    );
    // Another row of the index logged: what this read still stands, and is given back.
    written.wrote(keys_of("T-2"));
    assert_eq!(
        worked(&store, &mut memo, &kind, Some(&written)).await,
        b"two"
    );
    // Work that read a row of another table is read again every time, logged or not.
    let mut memo: Memo<Vec<u8>> = Memo::default();
    assert_eq!(
        worked(&store, &mut memo, &built, Some(&written)).await,
        b"one"
    );
    put(&store, &built, b"two").await;
    assert_eq!(
        worked(&store, &mut memo, &built, Some(&written)).await,
        b"two"
    );
}

/// The kind index's row of type `id`.
fn keys_of(id: &str) -> kit_store::Key {
    Place::Kind {
        project: i("p"),
        document: i("doc"),
        kind: i("type"),
        element: i(id),
    }
    .key()
}

/// A project's plans with document `doc`'s read, from an empty store.
async fn loaded() -> Documents {
    let store = Memory::new();
    let mut snapshot = store.snapshot().await.unwrap();
    let mut documents = Documents::default();
    documents
        .plan(&mut snapshot, &i("p"), &i("doc"))
        .await
        .unwrap();
    documents
}

#[tokio::test]
async fn the_walls_kept_are_taken_only_where_regeneration_still_stands() {
    let mut kept = Kept::default();
    kept.keep(&i("p"), 7, loaded().await);
    // Another number: nothing, and what was kept goes.
    assert!(kept.take(&i("p"), 8).read_plan(&i("doc")).is_none());
    assert!(kept.take(&i("p"), 7).read_plan(&i("doc")).is_none());
    // The same number: what was kept, once.
    kept.keep(&i("p"), 7, loaded().await);
    assert!(kept.take(&i("p"), 7).read_plan(&i("doc")).is_some());
    assert!(kept.take(&i("p"), 7).read_plan(&i("doc")).is_none());
    // The least recently kept project goes first beyond the most kept.
    let most = construct_regen::pass_plan::KEPT_PROJECTS;
    for k in 0..=most {
        kept.keep(&i(&format!("p{k}")), 1, loaded().await);
    }
    assert!(kept.take(&i("p0"), 1).read_plan(&i("doc")).is_none());
    for k in 1..=most {
        assert!(
            kept.take(&i(&format!("p{k}")), 1)
                .read_plan(&i("doc"))
                .is_some(),
            "p{k}"
        );
    }
}

#[test]
fn an_outline_the_room_search_refuses_is_known_for_it() {
    let mut outlines = Outlines::default();
    let level = Some(i("L-0"));
    let square = vec![[0, 0], [10 * MM, 0], [10 * MM, 10 * MM], [0, 10 * MM]];
    outlines.set(&i("W-1"), level.clone(), Some(square));
    outlines.set(&i("W-2"), level.clone(), None);
    assert_eq!(outlines.count_on(level.as_ref()), 2);
    assert!(!outlines.any_bad(level.as_ref()));
    // No area: the search refuses it.
    let flat = vec![[0, 0], [10 * MM, 0], [20 * MM, 0]];
    outlines.set(&i("W-3"), level.clone(), Some(flat));
    assert!(outlines.any_bad(level.as_ref()));
    outlines.forget(&i("W-3"));
    assert!(!outlines.any_bad(level.as_ref()));
    assert_eq!(outlines.count_on(level.as_ref()), 1 + 1);
    let area = ([MM, MM], [2 * MM, 2 * MM]);
    assert_eq!(
        outlines.near(level.as_ref(), area),
        BTreeSet::from([i("W-1")])
    );
}
