//! Fuzz target: `geom-mesh`'s triangulation is a true constrained Delaunay triangulation, and one
//! answer whatever order its input comes in.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_POINTS`] points on
//! an 8 × 8 lattice, where ties and collinear points are the rule, scaled by one of [`SCALES`], and
//! up to [`MAX_CONSTRAINTS`] constraints between them. [`check`] triangulates the case, holds the
//! result to the definitions of chapter 17.3, written here without the code's help, and
//! triangulates it again with the sites turned and the constraints reversed. [`harness`] is
//! `decode` then `check`, the function every fuzzing engine drives (the crate README sets out the
//! layout).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case: a
//!   repeated point is dropped, and so is a constraint from a point to itself.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **A panic or an error is a failure.** Every decoded case is valid, so an error is a [`Finding`].
//! - **A broken law is a [`Finding`]** naming the law and the case.
//!
//! Method: the laws, by exact predicates and exact integers. Every triangle is counter-clockwise,
//! no edge is used twice one way, and the triangles' area is the hull's, so they fill it without
//! overlap. Every edge that is not constrained is locally Delaunay: the far corner is outside the
//! circle, and at a tie the edge misses the lowest-ranked of the four corners, which simulation of
//! simplicity raises most (chapter 17.3,
//! `projects/engines/geometry/architecture/17-survey-terrain-civil.md`). Each constraint's two ends
//! are joined by constrained edges. The turned input gives the same result: the same value, so the
//! same bytes.
//!
//! Cost: at most [`MAX_POINTS`] points and [`MAX_CONSTRAINTS`] constraints, two triangulations, and
//! one pass over each edge and constraint of the result.

use std::collections::{BTreeMap, BTreeSet};

use geom_mesh::cdt::{Cdt, Site, triangulate};
use geom_num::Sign;
use geom_num::grid::P2;
use geom_num::predicate::{incircle, orient2d};
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most points a case holds: enough for several crossings, few enough to run fast.
pub const MAX_POINTS: u8 = 16;

/// The most constraints a case holds.
pub const MAX_CONSTRAINTS: u8 = 6;

/// What the lattice is scaled by: 1 and 3 and 7, where crossings fall off the grid and round, and
/// one that takes the lattice near the edge of the frame, where the exact paths are widest.
pub const SCALES: [i64; 4] = [1, 3, 7, 1 << 36];

/// The most bytes [`decode`] reads from any input: 47, three for the scale and the two counts, two
/// for each of 16 points and two for each of 6 constraints.
pub const MAX_INPUT_BYTES: usize = 47;

/// One decoded input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// Distinct points, sorted. A point's index is its key, so also its rank.
    pub points: Vec<(i64, i64)>,
    /// Pairs of distinct indices into `points`.
    pub constraints: Vec<[u32; 2]>,
}

/// The case `data` holds, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let scale = SCALES
        .get(usize::from(src.byte() % 4))
        .copied()
        .unwrap_or(1);
    let count = src.byte().checked_rem(MAX_POINTS.saturating_add(1));
    let wanted = src.byte().checked_rem(MAX_CONSTRAINTS.saturating_add(1));
    let mut seen = BTreeSet::new();
    for _ in 0..count.unwrap_or(0) {
        let (x, y) = (src.byte() % 8, src.byte() % 8);
        // At most 7 · 2^36 < 2^40, inside the frame.
        seen.insert((
            i64::from(x).saturating_mul(scale),
            i64::from(y).saturating_mul(scale),
        ));
    }
    let n = u8::try_from(seen.len()).unwrap_or(MAX_POINTS);
    let mut constraints = Vec::new();
    for _ in 0..wanted.unwrap_or(0) {
        match (src.byte().checked_rem(n), src.byte().checked_rem(n)) {
            (Some(a), Some(b)) if a != b => constraints.push([u32::from(a), u32::from(b)]),
            _ => {}
        }
    }
    let read = data.len().saturating_sub(src.remaining());
    let points = seen.into_iter().collect();
    (
        Case {
            points,
            constraints,
        },
        read,
    )
}

/// Twice the area of `polygon`, by the shoelace formula. Coordinates are below 2^40 and there are
/// few corners, so no step comes near the ends of `i128`.
fn twice_area(polygon: &[P2]) -> i128 {
    let next = polygon.iter().cycle().skip(1);
    polygon.iter().zip(next).fold(0, |sum, (a, b)| {
        let up = i128::from(a.x.get()).saturating_mul(i128::from(b.y.get()));
        let down = i128::from(a.y.get()).saturating_mul(i128::from(b.x.get()));
        sum.saturating_add(up.saturating_sub(down))
    })
}

/// One side of the hull of `points`, sorted, by Andrew's monotone chain: every step turns left.
fn side(points: impl Iterator<Item = P2>) -> Vec<P2> {
    let mut chain: Vec<P2> = Vec::new();
    for q in points {
        while let [.., a, b] = *chain.as_slice() {
            if orient2d(a, b, q) == Ok(Sign::Positive) {
                break;
            }
            chain.pop();
        }
        chain.push(q);
    }
    chain.pop();
    chain
}

/// Twice the area of the hull of `points`.
fn hull_area(points: &[P2]) -> i128 {
    let mut sorted = points.to_vec();
    sorted.sort_unstable();
    let mut hull = side(sorted.iter().copied());
    hull.extend(side(sorted.iter().rev().copied()));
    twice_area(&hull)
}

/// The vertices that `start` reaches along `edges`.
fn reached(start: u32, edges: &BTreeMap<u32, Vec<u32>>) -> BTreeSet<u32> {
    let mut seen = BTreeSet::from([start]);
    let mut todo = vec![start];
    while let Some(v) = todo.pop() {
        for &w in edges.get(&v).map_or(&[][..], Vec::as_slice) {
            if seen.insert(w) {
                todo.push(w);
            }
        }
    }
    seen
}

/// Every law of the module's method on `c`, the triangulation of `case`.
fn laws(case: &Case, c: &Cdt<u32>) -> Result<(), Finding> {
    let broken = |law: &str, detail: String| Finding::new(law, || format!("{case:?}: {detail}"));
    let points: Vec<P2> = c.vertices.iter().map(|v| v.at).collect();
    let at = |v: u32| {
        usize::try_from(v)
            .ok()
            .and_then(|i| points.get(i))
            .copied()
            .ok_or_else(|| broken("every corner is a vertex", format!("{v}")))
    };
    let mut uses = BTreeMap::new();
    let mut twice = 0_i128;
    for &t in &c.triangles {
        let [a, b, d] = t;
        let corners = [at(a)?, at(b)?, at(d)?];
        let [pa, pb, pd] = corners;
        if orient2d(pa, pb, pd) != Ok(Sign::Positive) {
            let law = "every triangle is counter-clockwise";
            return Err(broken(law, format!("{t:?}")));
        }
        twice = twice.saturating_add(twice_area(&corners));
        for (x, y, z) in [(a, b, d), (b, d, a), (d, a, b)] {
            if uses.insert([x, y], z).is_some() {
                return Err(broken("no edge is used twice one way", format!("{x}-{y}")));
            }
        }
    }
    let hull = hull_area(&points);
    if twice != hull {
        let law = "the triangles fill the hull";
        return Err(broken(law, format!("{twice} of {hull}")));
    }
    let fixed: BTreeSet<[u32; 2]> = c.constraints.iter().copied().collect();
    for (&[x, y], &z) in &uses {
        if x > y || fixed.contains(&[x, y]) {
            continue;
        }
        let Some(&w) = uses.get(&[y, x]) else {
            continue;
        };
        let low = x.min(y).min(z).min(w);
        let inside = match incircle(at(x)?, at(y)?, at(z)?, at(w)?) {
            Ok(Sign::Positive) => true,
            Ok(Sign::Negative) => false,
            Ok(Sign::Zero) => low == x || low == y,
            Err(e) => return Err(broken("incircle answers", e.to_string())),
        };
        if inside {
            let law = "every free edge is locally Delaunay";
            return Err(broken(law, format!("{x}-{y}, opposite {z} and {w}")));
        }
    }
    let mut edges: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &[a, b] in &c.constraints {
        edges.entry(a).or_default().push(b);
        edges.entry(b).or_default().push(a);
    }
    for &[a, b] in &case.constraints {
        if !reached(a, &edges).contains(&b) {
            let law = "constrained edges join each constraint's ends";
            return Err(broken(law, format!("{a}-{b}")));
        }
    }
    Ok(())
}

/// Triangulates `sites` under `constraints`, without a budget.
fn run(sites: &[Site<u32>], constraints: &[[u32; 2]], case: &Case) -> Result<Cdt<u32>, Finding> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    triangulate(sites, constraints, None, &mut meter)
        .map_err(|e| Finding::new("a valid case triangulates", || format!("{case:?}: {e:?}")))
}

/// Holds the triangulation of `case` to every law, and to one answer from a turned input.
///
/// # Errors
///
/// A [`Finding`] naming the first law broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    let mut sites = Vec::with_capacity(case.points.len());
    for (&(x, y), key) in case.points.iter().zip(0_u32..) {
        let at = P2::new(x, y).map_err(|e| {
            Finding::new("a decoded point is in the frame", || {
                format!("{case:?}: {e:?}")
            })
        })?;
        sites.push(Site { at, key });
    }
    let first = run(&sites, &case.constraints, case)?;
    laws(case, &first)?;
    if sites.is_empty() {
        return Ok(());
    }

    // The last site comes first, so each index moves up by one, and the last wraps to 0.
    sites.rotate_right(1);
    let n = u32::try_from(case.points.len()).unwrap_or(u32::MAX);
    let moved = |v: u32| v.saturating_add(1).checked_rem(n).unwrap_or(0);
    let reversed: Vec<[u32; 2]> = case
        .constraints
        .iter()
        .rev()
        .map(|&[a, b]| [moved(b), moved(a)])
        .collect();
    let second = run(&sites, &reversed, case)?;
    if second != first {
        return Err(Finding::new("one answer in any order", || {
            format!("{case:?}: {first:?} then {second:?}")
        }));
    }
    Ok(())
}

/// The harness every fuzzing engine drives: [`decode`] then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
