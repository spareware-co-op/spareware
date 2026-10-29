//! Adversarial tests for piece G47: the interval `atan2`, and the exact quarter turns of `sinpi`
//! and `cospi` in both their point and interval forms.
//!
//! What is attacked:
//!
//! - `Interval::atan2`'s inclusion: the exact angle of every sampled real point of a box (its
//!   corners, its edges, its inside and the points just below the negative x-axis) lies inside
//!   the result, strictly unless the angle is the exact 0, judged against the big-number reference
//!   of `math_reference.rs` and not against the port the function calls;
//! - its tightness: each bound is the reference's correctly rounded extreme widened by one step,
//!   or the exact 0, or `±next_up(RN(π))` where the box meets the branch cut from both sides;
//! - every configuration of a box against the axes, the branch cut and the origin, each with its
//!   bounds written out, and the origin alone refused;
//! - inclusion isotonicity: a box inside another gives a result inside the other's;
//! - every multiple of a quarter turn, `sinpi` and `cospi` exactly 0 or ±1, point and interval
//!   alike: exhaustively below 2^16 and at every binade above.
//!
//! The random boxes come from `geom_test`'s `SplitMix64`, from the seed this file prints
//! (`SPAREKIT_SEED` replaces it).

// Test code may use native arithmetic and casts (Cargo.toml); helpers outside `#[test]` functions
// may panic, as clippy.toml allows inside them.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may use native arithmetic, casts and std reductions, and stop loudly"
)]

#[path = "math_reference.rs"]
mod reference;

use std::num::NonZeroU64;

use geom_num::interval::Interval;
use geom_num::{NumError, math};
use geom_test::SplitMix64;
use geom_test::seed::{Seed, resolve};

/// `RN(π)`, the double nearest π (below it).
const PI: f64 = std::f64::consts::PI;

/// The bounds of every angle: the least double above π, and its negation.
const PI_UP: f64 = 3.141_592_653_589_793_6;

fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

/// The reference's correctly rounded angle of the point `(x, y)`, from a real zero `y` as +0.
fn angle(y: f64, x: f64) -> f64 {
    reference::atan2(y + 0.0, x + 0.0)
}

/// One step below the reference's angle of `(x, y)`.
fn below(y: f64, x: f64) -> f64 {
    angle(y, x).next_down()
}

/// One step above the reference's angle of `(x, y)`.
fn above(y: f64, x: f64) -> f64 {
    angle(y, x).next_up()
}

/// `y.atan2(x)` over the boxes `y × x`, as its two bounds' bits.
fn bounds(y: Interval, x: Interval) -> (u64, u64) {
    let r = y.atan2(x).unwrap();
    (r.lo().to_bits(), r.hi().to_bits())
}

#[track_caller]
fn assert_bounds(y: (f64, f64), x: (f64, f64), want: (f64, f64)) {
    assert_eq!(
        bounds(iv(y.0, y.1), iv(x.0, x.1)),
        (want.0.to_bits(), want.1.to_bits()),
        "atan2 over y ∈ [{}, {}], x ∈ [{}, {}]",
        y.0,
        y.1,
        x.0,
        x.1
    );
}

/// The `PI_UP` above is the double after `RN(π)`, and the exact π lies between the two.
#[test]
fn pi_up_is_the_double_after_pi() {
    assert_eq!(PI_UP, PI.next_up());
    assert_eq!(angle(0.0, -1.0), PI);
    // RN(π) is below π: π − RN(π) ≈ 1.22e−16 > 0 (the reference's π, to 4096 bits).
    let pi = reference::pi();
    assert_eq!(
        reference::Dy::from_f64(PI).cmp(&pi.lo),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        reference::Dy::from_f64(PI_UP).cmp(&pi.hi),
        std::cmp::Ordering::Greater
    );
}

/// A box in each open quadrant, and across each half-axis but the negative x-axis: the bounds are
/// the angles of the two corners the edges of the image pass through, each widened by one step.
#[test]
fn boxes_in_every_quadrant_and_across_the_axes() {
    // Quadrant I: smallest angle at the bottom right, largest at the top left.
    assert_bounds((1.0, 2.0), (1.0, 3.0), (below(1.0, 3.0), above(2.0, 1.0)));
    // Quadrant II: smallest at the top right, largest at the bottom left.
    assert_bounds(
        (1.0, 2.0),
        (-3.0, -1.0),
        (below(2.0, -1.0), above(1.0, -3.0)),
    );
    // Quadrant III: smallest at the top left, largest at the bottom right.
    assert_bounds(
        (-2.0, -1.0),
        (-3.0, -1.0),
        (below(-1.0, -3.0), above(-2.0, -1.0)),
    );
    // Quadrant IV: smallest at the bottom left, largest at the top right.
    assert_bounds(
        (-2.0, -1.0),
        (1.0, 3.0),
        (below(-2.0, 1.0), above(-1.0, 3.0)),
    );
    // Across the positive x-axis.
    assert_bounds((-1.0, 1.0), (1.0, 2.0), (below(-1.0, 1.0), above(1.0, 1.0)));
    // Across the positive and the negative y-axis.
    assert_bounds((1.0, 2.0), (-1.0, 1.0), (below(1.0, 1.0), above(1.0, -1.0)));
    assert_bounds(
        (-2.0, -1.0),
        (-1.0, 1.0),
        (below(-1.0, -1.0), above(-1.0, 1.0)),
    );
}

/// A box that meets the negative x-axis and has points below it takes the whole range; a box
/// whose lower edge is on the axis does not, since its points there have the angle π.
#[test]
fn the_branch_cut_on_the_negative_x_axis() {
    let whole = (-PI_UP, PI_UP);
    // Across the cut.
    assert_bounds((-1.0, 1.0), (-2.0, -1.0), whole);
    // The top edge on the cut: the points of the edge have the angle π, those below it angles
    // just above −π.
    assert_bounds((-1.0, 0.0), (-2.0, -1.0), whole);
    // The bottom edge on the cut: angles from the top right corner's up to π.
    assert_bounds((0.0, 1.0), (-2.0, -1.0), (below(1.0, -1.0), PI_UP));
    // A segment of the negative x-axis: π alone, widened.
    assert_bounds((0.0, 0.0), (-2.0, -1.0), (PI.next_down(), PI_UP));
    // Huge and tiny boxes across the cut.
    assert_bounds((-f64::MAX, f64::MAX), (-f64::MAX, -f64::MAX), whole);
    let tiny = f64::from_bits(1);
    assert_bounds((-tiny, 0.0), (-tiny, -tiny), whole);
    // Just short of the cut from below: no wrap; the smallest angle is next to −π, the largest
    // at the top right corner.
    assert_bounds(
        (-1.0, -tiny),
        (-2.0, -1.0),
        (below(-tiny, -2.0), above(-1.0, -1.0)),
    );
}

/// The origin inside a box, on each edge and at each corner, and the origin alone.
#[test]
fn the_origin_inside_on_an_edge_at_a_corner_and_alone() {
    let whole = (-PI_UP, PI_UP);
    let quarter = angle(1.0, 0.0);
    // Inside: every direction.
    assert_bounds((-1.0, 1.0), (-1.0, 1.0), whole);
    // On the bottom edge: the upper half-plane, from the exact 0 to π.
    assert_bounds((0.0, 1.0), (-1.0, 1.0), (0.0, PI_UP));
    // On the left edge: the right half-plane.
    assert_bounds(
        (-1.0, 1.0),
        (0.0, 1.0),
        (-quarter.next_up(), quarter.next_up()),
    );
    // On the right edge and on the top edge: across the cut.
    assert_bounds((-1.0, 1.0), (-1.0, 0.0), whole);
    assert_bounds((-1.0, 0.0), (-1.0, 1.0), whole);
    // At each corner: a quarter of the plane, the exact 0 kept exact.
    assert_bounds((0.0, 1.0), (0.0, 1.0), (0.0, quarter.next_up()));
    assert_bounds((0.0, 1.0), (-1.0, 0.0), (quarter.next_down(), PI_UP));
    assert_bounds((-1.0, 0.0), (-1.0, 0.0), whole);
    assert_bounds((-1.0, 0.0), (0.0, 1.0), (-quarter.next_up(), 0.0));
    // Segments from the origin along each axis.
    assert_bounds((0.0, 0.0), (0.0, 1.0), (0.0, 0.0));
    assert_bounds((0.0, 0.0), (-1.0, 0.0), (PI.next_down(), PI_UP));
    assert_bounds(
        (0.0, 1.0),
        (0.0, 0.0),
        (quarter.next_down(), quarter.next_up()),
    );
    assert_bounds(
        (-1.0, 0.0),
        (0.0, 0.0),
        (-quarter.next_up(), -quarter.next_down()),
    );
    // A segment of the x-axis through the origin: the angles 0 and π, and the hull between.
    assert_bounds((0.0, 0.0), (-1.0, 1.0), (0.0, PI_UP));
    // The origin alone has no angle.
    let zero = iv(0.0, 0.0);
    assert_eq!(zero.atan2(zero), Err(NumError::OutsideDomain));
    let minus_zero = iv(-0.0, -0.0);
    assert_eq!(minus_zero.atan2(zero), Err(NumError::OutsideDomain));
    assert_eq!(zero.atan2(minus_zero), Err(NumError::OutsideDomain));
}

/// A point box is its point's correctly rounded angle widened by one step, or the exact 0 on the
/// positive x-axis, and a zero bound is +0.
#[test]
fn point_boxes() {
    let seed = resolve("attack_g47 point boxes", Seed(0x6747_0000_0000_0001)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..300 {
        let (y, x) = (any(&mut rng), any(&mut rng));
        let r = Interval::point(y)
            .unwrap()
            .atan2(Interval::point(x).unwrap())
            .unwrap();
        assert_eq!(
            (r.lo(), r.hi()),
            (below(y, x), above(y, x)),
            "({y:e}, {x:e})"
        );
    }
    for x in [f64::from_bits(1), 1.0, f64::MAX] {
        let r = iv(0.0, 0.0).atan2(iv(x, x)).unwrap();
        assert_eq!((r.lo().to_bits(), r.hi().to_bits()), (0, 0));
    }
    let r = iv(-1.0, 0.0).atan2(iv(1.0, 2.0)).unwrap();
    assert_eq!(r.hi().to_bits(), 0, "a zero upper bound is +0");
}

/// A random double: any binade, subnormals included, either sign.
fn any(rng: &mut SplitMix64) -> f64 {
    let e = rng.below(NonZeroU64::new(2047).unwrap());
    let bits = rng.next_u64();
    f64::from_bits((bits & (1 << 63)) | (e << 52) | (bits & ((1 << 52) - 1)))
}

/// A random end of a box: often zero, often small integers (so boxes touch the axes and the
/// origin), otherwise of one scale (so boxes are not all huge or all tiny), either sign.
fn end(rng: &mut SplitMix64, scale: i32) -> f64 {
    match rng.below(NonZeroU64::new(6).unwrap()) {
        0 => 0.0,
        1 => rng.range_i64(-3, 3).unwrap() as f64,
        _ => {
            let unit = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
            (2.0 * unit - 1.0) * libm::scalbn(1.0, scale)
        }
    }
}

/// A random box `(y, x)`: each interval from two ends at one scale, ordered.
fn random_box(rng: &mut SplitMix64) -> ((f64, f64), (f64, f64)) {
    let scale =
        [-1070, -600, -20, 0, 0, 0, 30, 600, 1020][rng.below(NonZeroU64::new(9).unwrap()) as usize];
    let mut side = || {
        let (a, b) = (end(rng, scale), end(rng, scale));
        if a <= b { (a, b) } else { (b, a) }
    };
    (side(), side())
}

/// The points of a box `y × x` whose angles the test samples: the corners, the middles of the
/// edges, inside points, and points just below the negative x-axis where the box has them.
fn samples(rng: &mut SplitMix64, y: (f64, f64), x: (f64, f64)) -> Vec<(f64, f64)> {
    let mut at = |lo: f64, hi: f64| {
        let t = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        (lo + t * (hi - lo)).clamp(lo, hi)
    };
    let mut points = vec![(y.0, x.0), (y.0, x.1), (y.1, x.0), (y.1, x.1)];
    for _ in 0..3 {
        points.push((at(y.0, y.1), at(x.0, x.1)));
    }
    points.push((y.0, at(x.0, x.1)));
    points.push((at(y.0, y.1), x.1));
    if y.0 < 0.0 && y.1 >= 0.0 && x.0 < 0.0 {
        // Just below the cut: angles next to −π.
        points.push((-f64::from_bits(1).max(-y.0 * 1e-300), x.0));
    }
    points.retain(|&(py, px)| py != 0.0 || px != 0.0);
    points
}

/// Every sampled real point's exact angle lies inside the result: strictly between its bounds,
/// unless the angle is the exact 0 (on the positive x-axis), which may be a bound. A correctly
/// rounded `r` with `lo < r < hi` puts the exact angle inside, since it lies within half a step
/// of `r`.
#[test]
fn every_sampled_angle_lies_inside() {
    let seed = resolve("attack_g47 inclusion", Seed(0x6747_0000_0000_0002)).unwrap();
    let mut rng = SplitMix64::new(seed);
    let boxes = if cfg!(debug_assertions) { 150 } else { 1500 };
    for _ in 0..boxes {
        let (y, x) = random_box(&mut rng);
        let Ok(r) = iv(y.0, y.1).atan2(iv(x.0, x.1)) else {
            assert!(y == (0.0, 0.0) && x == (0.0, 0.0), "refused {y:?} × {x:?}");
            continue;
        };
        for (py, px) in samples(&mut rng, y, x) {
            let a = angle(py, px);
            let exact_zero = py == 0.0 && px > 0.0;
            assert!(
                (r.lo() < a && a < r.hi()) || (exact_zero && r.contains(0.0)),
                "angle {a:e} of ({px:e}, {py:e}) outside {r:?}, box y {y:?} × x {x:?}"
            );
        }
    }
}

/// Each bound is the reference's extreme over the corners other than the origin, widened by one
/// step (the exact 0 not widened), or the whole range where the box meets the cut from both
/// sides: the result is never wider than that.
#[test]
fn bounds_are_the_widened_extreme_corners() {
    let seed = resolve("attack_g47 tightness", Seed(0x6747_0000_0000_0003)).unwrap();
    let mut rng = SplitMix64::new(seed);
    let boxes = if cfg!(debug_assertions) { 150 } else { 1500 };
    for _ in 0..boxes {
        let (y, x) = random_box(&mut rng);
        let got = iv(y.0, y.1).atan2(iv(x.0, x.1));
        let want = if x.0 < 0.0 && y.0 < 0.0 && y.1 >= 0.0 {
            Ok((-PI_UP, PI_UP))
        } else {
            let corners: Vec<(f64, f64)> = [(y.0, x.0), (y.0, x.1), (y.1, x.0), (y.1, x.1)]
                .into_iter()
                .filter(|&(cy, cx)| cy != 0.0 || cx != 0.0)
                .map(|(cy, cx)| {
                    if cy == 0.0 && cx > 0.0 {
                        (0.0, 0.0)
                    } else {
                        (below(cy, cx), above(cy, cx))
                    }
                })
                .collect();
            if corners.is_empty() {
                Err(NumError::OutsideDomain)
            } else {
                Ok((
                    corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min) + 0.0,
                    corners
                        .iter()
                        .map(|c| c.1)
                        .fold(f64::NEG_INFINITY, f64::max)
                        + 0.0,
                ))
            }
        };
        let got = got.map(|r| (r.lo().to_bits(), r.hi().to_bits()));
        let want = want.map(|(lo, hi): (f64, f64)| (lo.to_bits(), hi.to_bits()));
        assert_eq!(got, want, "box y {y:?} × x {x:?}");
    }
}

/// A box inside another gives a result inside the other's (inclusion isotonicity).
#[test]
fn a_smaller_box_gives_a_result_inside() {
    let seed = resolve("attack_g47 isotonicity", Seed(0x6747_0000_0000_0004)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..2000 {
        let (y, x) = random_box(&mut rng);
        let Ok(outer) = iv(y.0, y.1).atan2(iv(x.0, x.1)) else {
            continue;
        };
        let mut inner = |(lo, hi): (f64, f64)| {
            let a = lo + (hi - lo) * ((rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64);
            let b = lo + (hi - lo) * ((rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64);
            let (a, b) = (a.clamp(lo, hi), b.clamp(lo, hi));
            if a <= b { iv(a, b) } else { iv(b, a) }
        };
        let (iy, ix) = (inner(y), inner(x));
        if let Ok(r) = iy.atan2(ix) {
            assert!(
                outer.lo() <= r.lo() && r.hi() <= outer.hi(),
                "{r:?} from {iy:?} × {ix:?} not inside {outer:?} from {y:?} × {x:?}"
            );
        }
    }
}

/// `sinpi` and `cospi` at a multiple of ½, point and interval: exactly 0 or ±1, a zero +0.
#[track_caller]
fn quarter_turn(halves: f64) {
    // The angle is halves/2 half-turns, whose sine and cosine are fixed by halves mod 4.
    let turns = halves * 0.5;
    let (sine, cosine) = match halves.rem_euclid(4.0) as u8 {
        0 => (0.0, 1.0),
        1 => (1.0, 0.0),
        2 => (0.0, -1.0),
        _ => (-1.0, 0.0),
    };
    let point = Interval::point(turns).unwrap();
    for (got, want, what) in [
        (math::sinpi(turns).unwrap().get(), sine, "sinpi"),
        (math::cospi(turns).unwrap().get(), cosine, "cospi"),
        (point.sinpi().unwrap().lo(), sine, "Interval::sinpi lo"),
        (point.sinpi().unwrap().hi(), sine, "Interval::sinpi hi"),
        (point.cospi().unwrap().lo(), cosine, "Interval::cospi lo"),
        (point.cospi().unwrap().hi(), cosine, "Interval::cospi hi"),
    ] {
        assert_eq!(got.to_bits(), f64::to_bits(want), "{what}({turns:e})");
    }
}

/// Every multiple of a quarter turn (90°) is exact: each one below 2^15 half-turns in magnitude,
/// and at every binade up to the largest double, multiples with every residue mod 4.
#[test]
fn every_multiple_of_a_quarter_turn_is_exact() {
    let limit = if cfg!(debug_assertions) {
        1 << 13
    } else {
        1 << 16
    };
    for halves in -limit..=limit {
        quarter_turn(f64::from(halves));
    }
    for e in 0..=1023 {
        let base = libm::scalbn(1.0, e);
        for d in [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0] {
            let halves = base + d;
            if halves.is_finite() {
                quarter_turn(halves);
                quarter_turn(-halves);
            }
        }
    }
    quarter_turn(f64::MAX);
    quarter_turn(-f64::MAX);
}
