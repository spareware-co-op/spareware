//! Fuzz target: the box tree and the cell index answer every query as brute force does, on any
//! bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_BOXES`] boxes in
//! two or three dimensions, a query box, a second set of boxes, a set of moved boxes, up to
//! [`MAX_POINTS`] grid points, a centre and a radius. [`check`] builds `geom_bvh::BoxTree`s and a
//! `geom_bvh::CellIndex` and holds every query to the answer of a brute force written here, from
//! the boxes' corners and exact integer distances, never from `geom-bvh`'s own tests: overlapping
//! a box, all overlapping pairs, pairs across two trees, a refit's answers (piece G54), and the
//! points within a radius of a point and all pairs within a radius (G55). [`harness`] is `decode`
//! then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Coordinates are small whole numbers most of the time, so boxes
//!   touch, nest and coincide often, with the frame's far reaches through a rule byte; keys are
//!   distinct. A tree or index `geom-bvh` refuses to build is legal.
//! - **A panic is a failure; a different answer is a [`Finding`].**
//! - **No hang.** Every call runs under a work meter of [`WORK`] units, serially.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below.
//!
//! Method: the queries' definitions in chapter 19 (closed boxes meet when they meet on every axis;
//! Euclidean distance at most r, compared as squared integers;
//! `projects/engines/geometry/architecture/19-meshes-spatial-clash.md`), checked by all-pairs
//! brute force as 28.3's row for spatial queries asks, driven by coverage-guided fuzzing (28.5).
//!
//! Cost: building and querying trees of at most [`MAX_BOXES`] boxes and an index of at most
//! [`MAX_POINTS`] points, and brute force over them, O(n²); at most [`MAX_INPUT_BYTES`] bytes.

// The lint is for fixed-width arithmetic in kernel code. The brute force works on coordinates
// within ±2^43 (`geom_bvh::LIMIT`), whose differences and squares fit `i128` exactly.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "brute force on coordinates within ±2^43, whose differences and squares fit i128, indexed by axes below the dimension"
)]

use geom_bvh::{Aabb, BoxTree, CellIndex, Entry, Point};
use geom_num::grid::P3;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most boxes in a set.
pub const MAX_BOXES: usize = 16;

/// The most points in the index.
pub const MAX_POINTS: usize = 16;

/// The work units each call may spend.
pub const WORK: u64 = 10_000_000;

/// The most bytes [`decode`] reads from any input: 3118.
///
/// Four header bytes (the dimension and three counts); the boxes of the three sets and the query,
/// (16 + 16 + 16 + 1) boxes of six coordinates; a count byte and 16 points of three coordinates;
/// a centre of three; each coordinate a byte and, for a far one read from the input, eight more;
/// and the radius's eight bytes: 4 + 49 · 6 · 9 + 1 + 16 · 3 · 9 + 3 · 9 + 8 = 3118. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds an input that reads exactly this many.
pub const MAX_INPUT_BYTES: usize =
    4 + (3 * MAX_BOXES + 1) * 6 * 9 + 1 + MAX_POINTS * 3 * 9 + 27 + 8;

/// One input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// Whether the boxes are in three dimensions.
    pub space: bool,
    /// The boxes, as corner pairs (the third coordinate unused in 2D).
    pub boxes: Vec<([i64; 3], [i64; 3])>,
    /// The second tree's boxes.
    pub others: Vec<([i64; 3], [i64; 3])>,
    /// The moved boxes, by index into `boxes`.
    pub moved: Vec<([i64; 3], [i64; 3])>,
    /// The query box.
    pub query: ([i64; 3], [i64; 3]),
    /// The points.
    pub points: Vec<[i64; 3]>,
    /// The centre of the radius query.
    pub center: [i64; 3],
    /// The radius.
    pub radius: u64,
}

/// One coordinate: a small whole number from the byte itself, or, for the rule's top values, the
/// frame's far reaches or a word read from the input folded into ±2^40.
fn coord(src: &mut ByteSource<'_>) -> i64 {
    let b = src.byte();
    match b {
        0..=239 => i64::from(b) / 4 - 30,
        240..=247 => (1i64 << 40) - 1 - i64::from(b - 240),
        248..=251 => -((1i64 << 40) - 1) + i64::from(b - 248),
        _ => {
            let w = src.word();
            i64::try_from(w.checked_rem((1u64 << 41) - 1).unwrap_or(0)).unwrap_or(0)
                - ((1i64 << 40) - 1)
        }
    }
}

fn corners(src: &mut ByteSource<'_>) -> ([i64; 3], [i64; 3]) {
    let a = [coord(src), coord(src), coord(src)];
    let b = [coord(src), coord(src), coord(src)];
    let (mut lo, mut hi) = (a, b);
    for (l, h) in lo.iter_mut().zip(hi.iter_mut()) {
        if *h < *l {
            core::mem::swap(l, h);
        }
    }
    (lo, hi)
}

/// Any bytes as a case, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(bytes);
    let space = src.byte() & 1 == 1;
    let count = |src: &mut ByteSource<'_>, most: usize| usize::from(src.byte()) % (most + 1);
    let (n, k, m) = (
        count(&mut src, MAX_BOXES),
        count(&mut src, MAX_BOXES),
        count(&mut src, MAX_BOXES),
    );
    let boxes = (0..n).map(|_| corners(&mut src)).collect();
    let others = (0..k).map(|_| corners(&mut src)).collect();
    let moved = (0..m.min(n)).map(|_| corners(&mut src)).collect();
    let query = corners(&mut src);
    let pc = usize::from(src.byte()) % (MAX_POINTS + 1);
    let points = (0..pc)
        .map(|_| [coord(&mut src), coord(&mut src), coord(&mut src)])
        .collect();
    let center = [coord(&mut src), coord(&mut src), coord(&mut src)];
    let radius = src.word() % 64;
    let read = bytes.len().saturating_sub(src.remaining());
    (
        Case {
            space,
            boxes,
            others,
            moved,
            query,
            points,
            center,
            radius,
        },
        read,
    )
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(WORK))
}

fn aabb<const D: usize>((lo, hi): ([i64; 3], [i64; 3])) -> Option<Aabb<D>> {
    Aabb::new(*lo.first_chunk::<D>()?, *hi.first_chunk::<D>()?).ok()
}

/// Closed boxes meet when they meet on every axis.
fn meet<const D: usize>(a: &Aabb<D>, b: &Aabb<D>) -> bool {
    let below = |p: &Aabb<D>, q: &Aabb<D>| p.lo().iter().zip(q.hi().iter()).all(|(l, h)| l <= h);
    below(a, b) && below(b, a)
}

fn entries<const D: usize>(boxes: &[([i64; 3], [i64; 3])], first: u64) -> Option<Vec<Entry<D>>> {
    boxes
        .iter()
        .zip(first..)
        .map(|(&b, key)| aabb(b).map(|bbox| Entry { key, bbox }))
        .collect()
}

fn found(law: &str, detail: impl FnOnce() -> String) -> Result<(), Finding> {
    Err(Finding::new(law, detail))
}

fn trees<const D: usize>(case: &Case) -> Result<(), Finding> {
    let exec = Exec::serial();
    let (Some(es), Some(os), Some(q)) = (
        entries::<D>(&case.boxes, 0),
        entries::<D>(&case.others, 1000),
        aabb::<D>(case.query),
    ) else {
        return Ok(());
    };
    let Ok(mut tree) = BoxTree::build(&es, &exec, &mut meter()) else {
        return Ok(());
    };
    // Overlapping a box.
    if let Ok(mut got) = tree.overlapping(&q, &mut meter()) {
        got.sort_unstable();
        let want: Vec<u64> = es
            .iter()
            .filter(|e| meet(&e.bbox, &q))
            .map(|e| e.key)
            .collect();
        if got != want {
            return found("a box query equals brute force", || {
                format!("{es:?} by {q:?}: {got:?} against {want:?}")
            });
        }
    }
    // All overlapping pairs.
    let brute_pairs = |es: &[Entry<D>]| {
        let mut v = Vec::new();
        for (i, a) in es.iter().enumerate() {
            for b in es.iter().skip(i + 1) {
                if meet(&a.bbox, &b.bbox) {
                    v.push((a.key.min(b.key), a.key.max(b.key)));
                }
            }
        }
        v.sort_unstable();
        v
    };
    if let Ok(mut got) = tree.pairs(&mut meter()) {
        got.sort_unstable();
        let want = brute_pairs(&es);
        if got != want {
            return found("all overlapping pairs equal brute force", || {
                format!("{es:?}: {got:?} against {want:?}")
            });
        }
    }
    // Pairs across two trees.
    if let Ok(other) = BoxTree::build(&os, &exec, &mut meter())
        && let Ok(got) = tree.pairs_with(&other, &mut meter())
    {
        let mut got: Vec<(u64, u64)> = got.into_iter().map(|(a, b)| (a.min(b), a.max(b))).collect();
        got.sort_unstable();
        let mut want = Vec::new();
        for a in &es {
            for b in &os {
                if meet(&a.bbox, &b.bbox) {
                    want.push((a.key.min(b.key), a.key.max(b.key)));
                }
            }
        }
        want.sort_unstable();
        if got != want {
            return found("pairs across two trees equal brute force", || {
                format!("{es:?} with {os:?}: {got:?} against {want:?}")
            });
        }
    }
    // A refit's answers.
    let Some(updates) = entries::<D>(&case.moved, 0) else {
        return Ok(());
    };
    if tree.refit(&updates, &exec, &mut meter()).is_ok() {
        let mut now = es.clone();
        for u in &updates {
            if let Some(e) = now.iter_mut().find(|e| e.key == u.key) {
                e.bbox = u.bbox;
            }
        }
        if let Ok(mut got) = tree.pairs(&mut meter()) {
            got.sort_unstable();
            let want = brute_pairs(&now);
            if got != want {
                return found("a refitted tree's pairs equal brute force", || {
                    format!("{now:?}: {got:?} against {want:?}")
                });
            }
        }
    }
    Ok(())
}

fn dist2(a: [i64; 3], b: [i64; 3]) -> i128 {
    a.iter().zip(&b).fold(0, |total, (x, y)| {
        let d = i128::from(x - y);
        total + d * d
    })
}

fn cells(case: &Case) -> Result<(), Finding> {
    let pts: Option<Vec<Point>> = case
        .points
        .iter()
        .zip(0u64..)
        .map(|(&[x, y, z], key)| P3::new(x, y, z).ok().map(|at| Point { key, at }))
        .collect();
    let [cx, cy, cz] = case.center;
    let (Some(pts), Ok(c)) = (pts, P3::new(cx, cy, cz)) else {
        return Ok(());
    };
    let Ok(index) = CellIndex::build(&pts, &Exec::serial(), &mut meter()) else {
        return Ok(());
    };
    let r = i128::from(case.radius);
    if let Ok(mut got) = index.within(c, case.radius, &mut meter()) {
        got.sort_unstable();
        let want: Vec<u64> = case
            .points
            .iter()
            .zip(0u64..)
            .filter(|(p, _)| dist2(**p, case.center) <= r * r)
            .map(|(_, k)| k)
            .collect();
        if got != want {
            return found("points within a radius equal brute force", || {
                format!(
                    "{:?} around {:?}, r {}: {got:?} against {want:?}",
                    case.points, case.center, case.radius
                )
            });
        }
    }
    if let Ok(mut got) = index.pairs_within(case.radius, &Exec::serial(), &mut meter()) {
        got.sort_unstable();
        let mut want = Vec::new();
        for (i, a) in case.points.iter().enumerate() {
            for (j, b) in case.points.iter().enumerate().skip(i + 1) {
                if dist2(*a, *b) <= r * r {
                    want.push((i as u64, j as u64));
                }
            }
        }
        if got != want {
            return found("pairs within a radius equal brute force", || {
                format!(
                    "{:?}, r {}: {got:?} against {want:?}",
                    case.points, case.radius
                )
            });
        }
    }
    Ok(())
}

/// The laws on the case (module documentation).
///
/// # Errors
///
/// A [`Finding`] for the first query that differs from brute force.
pub fn check(case: &Case) -> Result<(), Finding> {
    if case.space {
        trees::<3>(case)?;
    } else {
        trees::<2>(case)?;
    }
    cells(case)
}

/// [`decode`], then [`check`].
///
/// # Errors
///
/// As [`check`].
pub fn harness(bytes: &[u8]) -> Result<(), Finding> {
    check(&decode(bytes).0)
}
