//! The polyhedral envelope through the public interface (piece G152), against a rational oracle:
//! squared distances from a point to a triangle as exact fractions, written apart from the code.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_mesh::envelope::{Envelope, EnvelopeError, TriangleEnvelope, half_side};
use geom_num::NumError;
use geom_num::grid::P3;
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 20))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn xyz(c: P3) -> [i128; 3] {
    [c.x.get(), c.y.get(), c.z.get()].map(i128::from)
}

fn sub(a: [i128; 3], b: [i128; 3]) -> [i128; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [i128; 3], b: [i128; 3]) -> i128 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [i128; 3], b: [i128; 3]) -> [i128; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Squared distance from `q` to segment `ab`, as a fraction.
fn to_segment(q: [i128; 3], a: [i128; 3], b: [i128; 3]) -> (i128, i128) {
    let (ab, aq) = (sub(b, a), sub(q, a));
    let (t, l) = (dot(aq, ab), dot(ab, ab));
    if l == 0 || t <= 0 {
        (dot(aq, aq), 1)
    } else if t >= l {
        let bq = sub(q, b);
        (dot(bq, bq), 1)
    } else {
        (dot(aq, aq) * l - t * t, l)
    }
}

/// Squared distance from `q` to the plane of `t`, if its foot is inside `t`.
fn to_face(q: [i128; 3], t: [[i128; 3]; 3]) -> Option<(i128, i128)> {
    let [a, b, c] = t;
    let n = cross(sub(b, a), sub(c, a));
    let length = dot(n, n);
    if length == 0 {
        return None;
    }
    let height = dot(n, sub(q, a));
    let foot = [
        q[0] * length - height * n[0],
        q[1] * length - height * n[1],
        q[2] * length - height * n[2],
    ];
    let side = |u: [i128; 3], v: [i128; 3]| {
        let scaled = [
            foot[0] - u[0] * length,
            foot[1] - u[1] * length,
            foot[2] - u[2] * length,
        ];
        dot(n, cross(sub(v, u), scaled))
    };
    (side(a, b) >= 0 && side(b, c) >= 0 && side(c, a) >= 0).then_some((height * height, length))
}

/// Squared distance from `q` to triangle `t`, as a fraction.
fn distance(q: P3, t: [P3; 3]) -> (i128, i128) {
    let c = t.map(xyz);
    let qi = xyz(q);
    let mut best = to_segment(qi, c[0], c[1]);
    for candidate in [
        Some(to_segment(qi, c[1], c[2])),
        Some(to_segment(qi, c[2], c[0])),
        to_face(qi, c),
    ]
    .into_iter()
    .flatten()
    {
        if candidate.0 * best.1 < best.0 * candidate.1 {
            best = candidate;
        }
    }
    best
}

#[test]
fn half_side_is_the_largest_that_fits() {
    assert_eq!(half_side(0), 0);
    assert_eq!(half_side(1), 0);
    assert_eq!(half_side(2), 1);
    assert_eq!(half_side(1000), 577);
    for e in 0..2000_u64 {
        let h = half_side(e);
        assert!(3 * h * h <= e * e, "{e}");
        assert!(3 * (h + 1) * (h + 1) > e * e, "{e}");
    }
    let huge = 1_u64 << 42;
    let h = u128::from(half_side(huge));
    assert!(3 * h * h <= u128::from(huge) * u128::from(huge));
}

#[test]
fn inside_proves_within_and_within_proves_inside() {
    let seed = resolve("geom-mesh envelope oracle", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let below = |rng: &mut SplitMix64, n: u64| rng.below(NonZeroU64::new(n).unwrap());
    let (mut inside, mut outside) = (0, 0);
    for case in 0..40_000 {
        let reach = [8, 16, 64][case % 3];
        let corner = |rng: &mut SplitMix64| {
            p(
                below(rng, reach) as i64,
                below(rng, reach) as i64,
                below(rng, reach) as i64,
            )
        };
        let t = [corner(&mut rng), corner(&mut rng), corner(&mut rng)];
        let epsilon = 1 + below(&mut rng, 24);
        let h = half_side(epsilon);
        let body = TriangleEnvelope::new(t, h).unwrap();
        // Near a corner, so that the boundary of the envelope is what is tested.
        let base = t[case % 3];
        let spread = 2 * epsilon + 5;
        let near =
            |c: i64, rng: &mut SplitMix64| c + below(rng, spread) as i64 - (epsilon + 2) as i64;
        let q = p(
            near(base.x.get(), &mut rng),
            near(base.y.get(), &mut rng),
            near(base.z.get(), &mut rng),
        );
        let (top, bottom) = distance(q, t);
        let e2 = i128::from(epsilon) * i128::from(epsilon);
        let h2 = i128::from(h) * i128::from(h);
        if body.contains(q).unwrap() {
            inside += 1;
            assert!(
                top <= e2 * bottom,
                "{t:?} {q:?} ε {epsilon}: farther than ε"
            );
        } else {
            outside += 1;
            assert!(
                top > h2 * bottom,
                "{t:?} {q:?} ε {epsilon}: within h, not inside"
            );
        }
    }
    assert!(inside > 2000 && outside > 2000, "{inside} {outside}");
}

#[test]
fn every_corner_of_the_swept_cube_is_inside_and_on_a_facet() {
    let t = [p(0, 0, 0), p(40, 3, 0), p(5, 30, 7)];
    let h = 4;
    let body = TriangleEnvelope::new(t, h).unwrap();
    let s = h as i64;
    for v in t {
        for dx in [-s, s] {
            for dy in [-s, s] {
                for dz in [-s, s] {
                    let c = p(v.x.get() + dx, v.y.get() + dy, v.z.get() + dz);
                    assert!(body.contains(c).unwrap(), "{c:?}");
                }
            }
        }
    }
    // A step past the box of the swept cubes is outside.
    assert!(!body.contains(p(45, 0, 0)).unwrap());
    assert!(!body.contains(p(0, 0, -5)).unwrap());
    assert_eq!(body.planes().len(), 26);
}

#[test]
fn a_flat_triangle_still_has_its_envelope() {
    let segment = [p(0, 0, 0), p(10, 0, 0), p(20, 0, 0)];
    let body = TriangleEnvelope::new(segment, 2).unwrap();
    assert!(body.contains(p(15, 1, -1)).unwrap());
    assert!(!body.contains(p(15, 3, 0)).unwrap());
    let point = [p(5, 5, 5); 3];
    let body = TriangleEnvelope::new(point, 2).unwrap();
    assert!(body.contains(p(7, 3, 7)).unwrap());
    assert!(!body.contains(p(8, 5, 5)).unwrap());
    assert_eq!(body.planes().len(), 6);
}

#[test]
fn points_off_the_grid_are_decided_exactly() {
    let t = [p(0, 0, 0), p(100, 0, 0), p(0, 100, 0)];
    let body = TriangleEnvelope::new(t, 3).unwrap();
    let plane = |n: [i128; 3], d: i128| Plane::new(n, d).unwrap();
    // The planes 3x = 10, 3y = 10 and 3z = 8 meet at (10/3, 10/3, 8/3): under the top at z = 3.
    let under = Tpi::new(
        plane([3, 0, 0], -10),
        plane([0, 3, 0], -10),
        plane([0, 0, 3], -8),
    )
    .unwrap();
    assert!(body.contains_tpi(&under).unwrap());
    // At 3z = 10 the point is 1/3 above the top.
    let over = Tpi::new(
        plane([3, 0, 0], -10),
        plane([0, 3, 0], -10),
        plane([0, 0, 3], -10),
    )
    .unwrap();
    assert!(!body.contains_tpi(&over).unwrap());
    let top = plane([0, 0, 1], -3);
    let on_top = Lpi::new(p(5, 5, 0), p(5, 5, 9), top).unwrap();
    assert!(body.contains_lpi(&on_top).unwrap());
    let above = plane([0, 0, 1], -4);
    let past = Lpi::new(p(5, 5, 0), p(5, 5, 9), above).unwrap();
    assert!(!body.contains_lpi(&past).unwrap());
}

#[test]
fn a_surface_answers_for_every_triangle() {
    let triangles = [
        [p(0, 0, 0), p(100, 0, 0), p(0, 100, 0)],
        [p(200, 200, 50), p(300, 200, 50), p(200, 300, 50)],
    ];
    let serial = Exec::serial();
    let envelope = Envelope::new(&triangles, 10, &serial, &mut meter()).unwrap();
    assert_eq!(envelope.half_side(), 5);
    let mut m = meter();
    assert!(envelope.contains(p(20, 20, 5), &mut m).unwrap());
    assert!(envelope.contains(p(250, 220, 45), &mut m).unwrap());
    assert!(!envelope.contains(p(150, 150, 20), &mut m).unwrap());
    assert!(!envelope.contains(p(20, 20, 6), &mut m).unwrap());
    let plane = |n: [i128; 3], d: i128| Plane::new(n, d).unwrap();
    let near = Tpi::new(
        plane([3, 0, 0], -61),
        plane([0, 3, 0], -61),
        plane([0, 0, 3], -13),
    )
    .unwrap();
    assert!(envelope.contains_tpi(&near, &mut m).unwrap());
    let far = Tpi::new(
        plane([3, 0, 0], -61),
        plane([0, 3, 0], -61),
        plane([0, 0, 3], -16),
    )
    .unwrap();
    assert!(!envelope.contains_tpi(&far, &mut m).unwrap());
}

#[test]
fn the_width_is_absolute() {
    // The same triangle at either end of the frame has the same envelope, moved.
    let e = (1_i64 << 40) - 200;
    let near_edge = [p(e, e, e), p(e + 40, e, e), p(e, e + 40, e)];
    let body = TriangleEnvelope::new(near_edge, 3).unwrap();
    assert!(body.contains(p(e + 1, e + 1, e + 3)).unwrap());
    assert!(!body.contains(p(e + 1, e + 1, e + 4)).unwrap());
}

#[test]
fn a_budget_stops_a_build() {
    let t = [[p(0, 0, 0), p(10, 0, 0), p(0, 10, 0)]; 8];
    let mut small = WorkMeter::new(Budget::of_work(2));
    assert!(matches!(
        Envelope::new(&t, 10, &Exec::serial(), &mut small),
        Err(EnvelopeError::Stopped(_))
    ));
}

#[test]
fn errors_say_what_went_wrong() {
    let texts = [
        EnvelopeError::Num(NumError::Degenerate).to_string(),
        EnvelopeError::Full.to_string(),
    ];
    assert!(texts.iter().all(|t| !t.is_empty()));
}
