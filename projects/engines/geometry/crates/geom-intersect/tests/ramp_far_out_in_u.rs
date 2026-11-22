//! A self-crossing on knots far from zero in u alone (12.13 over 12.8's search). A square ramp round
//! the z axis between radii 1 and 2 (v = r − 1), up from z = 0 to ½ over its first quarter, level,
//! then down to 0 over its last, which lies over its first, on knots in u moved from 0 to 2⁴⁷ or 2⁵⁰
//! and in v left at 0 to 1. A NURBS reads its knots only through their differences, and every knot
//! moved is a double, so the ramp is the one at 0 moved along u: its crossing is where its first
//! and last quarters are both at z = ¼, along u = U + ½ of one sheet and u = U + 9/2 of the other,
//! at the same v, for every v from 0 to 1 (U the move).
//!
//! There the doubles in u stand 1/32 or ¼ apart, a span of the ramp cut into 32 or 4 pieces at
//! most, each further across on the ramp than the floor: no piece can be cut to the floor along u.
//! Before, the search cut such a piece across v instead, again and again, to widths near 10⁻¹⁶⁹ at
//! which the ramp's partials could no longer be bounded, and the whole search failed as if its
//! input broke a precondition. Now a piece whose longer side in space cannot be cut has no room and
//! is flagged as it is, so the crossing is held whole by the pairs the search leaves undecided:
//! every point of it, on both sheets and in both orders, lies in a flagged pair, and nothing is
//! traced or settled that is not there.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_intersect::selfcross::self_intersect;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::SurfaceKind;

const FLOOR: f64 = 1.0 / 128.0;
const CEILING: f64 = 1.0 / 16.0;

const SQUARE: [[f64; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];

/// The ramp, degree 1 both ways, its six rows on the knots `far`, `far`, `far` + 1, ..., `far` + 5,
/// `far` + 5 in u and 0, 0, 1, 1 in v.
fn ramp(far: f64) -> SurfaceKind {
    let z = [0.0, 0.5, 0.5, 0.5, 0.5, 0.0];
    let knots = |base: f64, n: u32| {
        let mut k = vec![base];
        for i in 0..n {
            k.push(base + f64::from(i));
        }
        k.push(base + f64::from(n - 1));
        Basis::new(1, k).unwrap()
    };
    let mut net = Vec::new();
    for (i, h) in z.iter().enumerate() {
        for r in [1.0, 2.0] {
            net.push([r * SQUARE[i % 4][0], r * SQUARE[i % 4][1], *h]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(knots(far, 6), knots(0.0, 2), &net).unwrap())
}

/// Whether the region holds the point (u, v).
fn holds(((a, b), (c, d)): ((f64, f64), (f64, f64)), u: f64, v: f64) -> bool {
    a <= u && u <= b && c <= v && v <= d
}

#[test]
fn a_ramp_far_out_in_u_holds_its_whole_crossing_in_flagged_pairs() {
    for far in [140_737_488_355_328.0, 1_125_899_906_842_624.0] {
        let mut meter = WorkMeter::new(Budget::of_work(1 << 24));
        let found = self_intersect(&ramp(far), FLOOR, CEILING, &mut meter).unwrap();
        // No piece along the crossing is small enough to settle, so nothing is traced or settled.
        assert!(found.branches.is_empty(), "{far}: {:?}", found.branches);
        assert!(found.unproven.is_empty(), "{far}: {:?}", found.unproven);
        let (one, other) = (far + 0.5, far + 4.5);
        for (p, q) in [(one, other), (other, one)] {
            for j in 0..=16 {
                let v = f64::from(j) / 16.0;
                assert!(
                    found
                        .flagged
                        .iter()
                        .any(|pair| holds(pair.first, p, v) && holds(pair.second, q, v)),
                    "{far}: ({p}, {v}) with ({q}, {v})"
                );
            }
        }
    }
}
