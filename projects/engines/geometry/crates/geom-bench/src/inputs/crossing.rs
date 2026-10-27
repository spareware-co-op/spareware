//! Inputs for where two lines cross in the plane (G29): [`ssi_new`] (`Ssi::new`),
//! [`ssi_homogeneous`] (`Ssi::homogeneous`), [`ssi_orient2d`] (`Ssi::orient2d`) and
//! [`ssi_to_grid`] (`Ssi::to_grid`).
//!
//! Classes:
//!
//! - `typical`: lines through random points within ±2^30 steps; for `orient2d`, only answers off
//!   the classifying line; for `to_grid`, only crossings inside the frame.
//! - `degenerate`: parallel lines (no crossing); a crossing at a grid point that the classifying
//!   line passes through.
//! - `nearly_degenerate`: a crossing at a grid point with orient2d(p, q, x) = ±1, from a unimodular
//!   basis, so the filter cannot decide by margin.
//! - `extreme`: points within 2^10 steps of the frame's edge.
//! - `on_grid`: a crossing at a grid point, which commits exactly.
//! - `overflow`: nearly parallel lines that cross outside the frame.
//!
//! Guarantees: each class's batch is deterministic (its seed is the operation and class name)
//! and its inputs take the path its class names, which `tests/inputs.rs` checks.
//!
//! Method: the degenerate cases of a crossing as `Ssi::new` documents them, with Shewchuk's input
//! classes (<https://doi.org/10.1007/PL00009321>, section 6) and a unimodular 2D basis (Knuth,
//! volume 2, section 4.5.2) for the nearly degenerate ones, built from the shared draws of
//! `inputs` as the plane inputs are.
//!
//! Cost: a batch of up to 64 inputs, built once per benchmark run.

use geom_num::crossing::Ssi;
use geom_num::grid::P2;

use super::predicate::unimodular2;
use super::{Class, M, TYPICAL_REACH, batch_of, extreme2, p2, rng, typical2, within};
use crate::rng::SplitMix64;

/// Largest component of a direction in degenerate inputs.
const STEP: i64 = 1 << 10;

/// A grid point as raw step counts.
fn raw(p: P2) -> [i64; 2] {
    [p.x.get(), p.y.get()]
}

/// `p + k · v` for a grid point and a raw direction.
fn along(p: P2, [x, y]: [i64; 2], k: i64) -> P2 {
    let [px, py] = raw(p);
    p2(px + k * x, py + k * y)
}

/// A small direction with components within ±[`STEP`].
fn small(g: &mut SplitMix64) -> [i64; 2] {
    [0; 2].map(|_| within(g, STEP))
}

/// The crossing of the lines from a to b and from c to d, if they cross in one point.
fn ssi([a, b, c, d]: [P2; 4]) -> Option<Ssi> {
    Ssi::new(a, b, c, d).ok()
}

/// Two lines through the grid point `at`, crossing only there.
fn ssi_at(g: &mut SplitMix64, at: P2) -> Option<Ssi> {
    let (u, v) = (small(g), small(g));
    ssi([
        along(at, u, -1),
        along(at, u, 1),
        along(at, v, -1),
        along(at, v, 1),
    ])
}

/// Two lines through typical points.
fn typical_ssi(g: &mut SplitMix64) -> Option<Ssi> {
    ssi([(); 4].map(|()| typical2(g)))
}

/// Two lines through extreme points.
fn extreme_ssi(g: &mut SplitMix64) -> Option<Ssi> {
    ssi([(); 4].map(|()| extreme2(g)))
}

/// Point quadruples for [`Ssi::new`].
#[must_use]
pub fn ssi_new(class: Class) -> Vec<[P2; 4]> {
    let mut g = rng("crossing::ssi_new", class);
    match class {
        Class::Typical => batch_of(|_| {
            let q = [(); 4].map(|()| typical2(&mut g));
            ssi(q).map(|_| q)
        }),
        Class::Degenerate => batch_of(|n| {
            // Parallel lines: the second along a whole multiple of the first's direction, or the
            // same line when it starts on the first, in rotation.
            let (a, u) = (typical2(&mut g), small(&mut g));
            let c = if n % 2 == 0 {
                along(a, u, 3)
            } else {
                typical2(&mut g)
            };
            let k = g.range(1, 4) * g.sign();
            Some([a, along(a, u, 1), c, along(c, u, k)])
        }),
        Class::Extreme => batch_of(|_| {
            let q = [(); 4].map(|()| extreme2(&mut g));
            ssi(q).map(|_| q)
        }),
        Class::NearlyDegenerate | Class::Overflow | Class::OnGrid | Class::WrongDirection => {
            Vec::new()
        }
    }
}

/// Crossings for [`Ssi::homogeneous`].
#[must_use]
pub fn ssi_homogeneous(class: Class) -> Vec<Ssi> {
    let mut g = rng("crossing::ssi_homogeneous", class);
    match class {
        Class::Typical => batch_of(|_| typical_ssi(&mut g)),
        Class::Extreme => batch_of(|_| extreme_ssi(&mut g)),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (crossing, p, q) triples for [`Ssi::orient2d`].
#[must_use]
pub fn ssi_orient2d(class: Class) -> Vec<(Ssi, P2, P2)> {
    ssi_orient2d_from("crossing::ssi_orient2d", class)
}

/// [`ssi_orient2d`]'s classes drawn from the seed of `op`: the batch the generated `ssi_side` and
/// `ssi_weight` benchmarks measure ([`super::generated`]), on inputs of their own.
pub(crate) fn ssi_orient2d_from(op: &str, class: Class) -> Vec<(Ssi, P2, P2)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (s, p, q) = (typical_ssi(&mut g)?, typical2(&mut g), typical2(&mut g));
            matches!(
                s.orient2d(p, q),
                Ok(geom_num::Sign::Positive | geom_num::Sign::Negative)
            )
            .then_some((s, p, q))
        }),
        Class::Degenerate => batch_of(|_| {
            // The crossing is a grid point on the classifying line.
            let at = typical2(&mut g);
            let s = ssi_at(&mut g, at)?;
            Some((s, at, along(at, small(&mut g), 1)))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            // orient2d(p, q, x) = (q − p) × (x − p) = det[u; v] = ±1 for p = x − v, q = p + u.
            let at = typical2(&mut g);
            let s = ssi_at(&mut g, at)?;
            let [u, v] = unimodular2(&mut g)?;
            let p = along(at, v, -1);
            Some((s, p, along(p, u, 1)))
        }),
        Class::Extreme => {
            batch_of(|_| Some((extreme_ssi(&mut g)?, extreme2(&mut g), extreme2(&mut g))))
        }
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Crossings for [`Ssi::to_grid`].
#[must_use]
pub fn ssi_to_grid(class: Class) -> Vec<Ssi> {
    let mut g = rng("crossing::ssi_to_grid", class);
    match class {
        Class::Typical => batch_of(|_| typical_ssi(&mut g).filter(|s| s.to_grid().is_ok())),
        Class::Extreme => batch_of(|_| extreme_ssi(&mut g).filter(|s| s.to_grid().is_ok())),
        Class::OnGrid => batch_of(|_| {
            let at = typical2(&mut g);
            ssi_at(&mut g, at)
        }),
        Class::Overflow => batch_of(|_| far_ssi(&mut g)),
        Class::Degenerate | Class::NearlyDegenerate | Class::WrongDirection => Vec::new(),
    }
}

/// Two lines that cross outside the frame: from (−M, y0) to (M, y0 + 1), and from (−M, y0 + k)
/// to (M, y0 + k + 2) for 2 ≤ k ≤ 2^20. Their gaps close by one step over the frame's width 2M, so
/// they cross k widths to the left, beyond −M.
fn far_ssi(g: &mut SplitMix64) -> Option<Ssi> {
    let y0 = within(g, TYPICAL_REACH);
    let k = g.range(2, 1 << 20);
    ssi([p2(-M, y0), p2(M, y0 + 1), p2(-M, y0 + k), p2(M, y0 + k + 2)])
}
