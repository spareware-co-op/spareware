//! A line beside a far part of a very large sphere, where the closed form's discriminant is held
//! from exactly zero upward: it cannot count the roots, and the engine answers.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{CurveKind, Frame, Line};
use geom_intersect::curve_surface::curve_surface;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Sphere, SurfaceKind};

/// 2^e exactly, from its bits, for e from −1074 to 1023.
fn pow2(e: i32) -> f64 {
    if e >= -1022 {
        f64::from_bits(u64::try_from(e + 1023).unwrap() << 52)
    } else {
        f64::from_bits(1u64 << u32::try_from(e + 1074).unwrap())
    }
}

/// A discriminant whose enclosure starts at exactly zero cannot count the roots any more than one
/// held across zero can, and the pair goes to the engine. The sphere about the origin has radius
/// r = 3·2^510, and only its part on the far side is asked for: u from 3/4 to 5/4 of a half turn
/// and v within a quarter of one, where x runs from −r to −r/2. The segment starts at
/// (r − 7·2^459, 0.68, 0), seven steps of the double inside the sphere on the near side, and runs
/// along y by 5·2^−538 for t within ±1: every point of it has x above 2^511, so it meets nothing of
/// that part. The whole line meets the whole sphere only near t = ±2^1023, at y near ±2^487.
///
/// The line's quadratic is a·t² + 2b·t + c with a = d·d, b = o·d and c = o·o − r². Its enclosures
/// here: a from 1 to 11 steps of 2^−1074 (d·d is 25·2^−1076, and each operation rounds outward a
/// step); b² from 2 to 4 steps (b is 0.85·2^−536, whose square is 2.89 steps); and c from −20·2^971
/// up to one step above zero, since o·o rounded up and r² rounded down are the same double. So a·c
/// reaches up to one step, and b² − a·c is held from 2 − 1 = 1 step, which rounded outward is
/// exactly 0, up to about 2^−95. Its root is then held from 0 to about 2^−48, and over a's lower
/// bound of one step that is past the largest double: the two roots' enclosures cannot be formed.
#[test]
fn a_discriminant_held_from_exactly_zero_leaves_a_line_to_the_engine() {
    let r = 3.0 * pow2(510);
    let far = ((0.75, 1.25), (-0.25, 0.25));
    let ball =
        SurfaceKind::Sphere(Sphere::new([0.0; 3], r, Frame::standard().unwrap(), far).unwrap());
    let x = r - 7.0 * pow2(459);
    assert!(x > pow2(511) && x < r);
    let segment =
        Line::<3>::new([x, 0.68, 0.0], [0.0, 5.0 * pow2(-538), 0.0], (-1.0, 1.0)).unwrap();
    let segment = CurveKind::Line(segment);
    let mut meter = WorkMeter::new(Budget::of_work(1 << 16));
    let found = curve_surface(&segment, &ball, &mut meter)
        .unwrap_or_else(|e| panic!("{e:?} for a segment that is nowhere near the part"));
    assert!(found.items().is_empty(), "{:?}", found.items());
}
