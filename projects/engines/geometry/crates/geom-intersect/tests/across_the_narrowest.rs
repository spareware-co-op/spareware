//! A pair in space is solved in the two coordinates that spread it most, leaving out the narrowest
//! (12.5): a crossing the engine proves in those two, it cannot in the one it should leave out.
//!
//! This file is named to come first among the crate's test files. Cargo runs them in name order and
//! stops at the first that fails, as the mutation gate runs them, so a change that solves every pair
//! in one coordinate twice, which sends many of the later files' searches to their floor over a
//! whole curve of boxes, fails here in milliseconds instead of running past the gate's time.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::CurveKind;
use geom_intersect::curve::{curves, curves_over};
use geom_intersect::result::{Coverage, Item, Unresolved};
use geom_nurbs::{Basis, Curve};
use geom_run::{Budget, WorkMeter};

/// A meter with a budget the tests here fit in many times over, so a change that sends the engine
/// round for ever fails on the budget instead of hanging.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1_000_000))
}

/// The quadratic Bezier curve on [0, 1] with these control points.
fn bezier(points: &[[f64; 3]; 3]) -> CurveKind<3> {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    CurveKind::Nurbs(Curve::new(basis, points).unwrap())
}

/// Two parabolic arcs, a(u) = (2u - 1, -(2u - 1)^2 / 4, 0) and b(t) = (0, (2t - 1)^2 / 4, 2t - 1),
/// meet at the origin, at u = t = 1/2, and nowhere else: x = 0 on b gives u = 1/2 on a, and z = 0
/// on a gives t = 1/2 on b. The pair's box is 2 wide in x and in z and 1/2 in y, so the engine leaves
/// out y and solves in x and z, where a runs along x and b along z: they cross at right angles, a
/// zero the engine proves the only one near (1/2, 1/2). The left-out y agrees there within its
/// enclosure but is not proven equal, so the crossing is a near miss, as every crossing in space is
/// (12.5). In y alone a lies at or below 0 and b at or above, touching only at the crossing, where
/// both are flat in y: solved in y twice, no box proves a zero, and the engine gives back the
/// middle half of each domain as a contact of higher order than its tests reach, not the crossing.
/// Either way round, and over a box of the two domains.
#[test]
fn arcs_crossing_at_right_angles_in_x_and_z_are_a_near_miss_found_leaving_out_y() {
    let a = bezier(&[[-1.0, -0.25, 0.0], [0.0, 0.25, 0.0], [1.0, -0.25, 0.0]]);
    let b = bezier(&[[0.0, 0.25, -1.0], [0.0, -0.25, 0.0], [0.0, 0.25, 1.0]]);
    for (p, q) in [(&a, &b), (&b, &a)] {
        for r in [
            curves(p, q, &mut meter()).unwrap(),
            curves_over(p, (0.25, 0.75), q, (0.25, 0.75), &mut meter()).unwrap(),
        ] {
            assert_eq!(r.coverage(), Coverage::CompleteOutsideFlagged);
            let at = match r.items() {
                [
                    Item::Flagged {
                        at,
                        why: Unresolved::NearMiss { .. },
                        ..
                    },
                ] => at.spans(),
                other => panic!("{other:?}"),
            };
            assert!(at[0].contains(0.5) && at[1].contains(0.5), "{at:?}");
        }
    }
}
