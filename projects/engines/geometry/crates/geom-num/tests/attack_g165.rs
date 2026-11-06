//! Attacks on G165's exact decisions: each against a big-integer oracle over the whole frame and on
//! geometry a hand can check, and two cones through one apex also against the same sum in doubles,
//! which shares nothing with the integer form it judges.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_num::grid::{P3, Steps};
use geom_num::quadric::{
    Cone, Plane, Slope, plane_across, plane_along, plane_ball, plane_cone, shared_apex,
};
use geom_num::{NumError, Sign};
use num_bigint::BigInt;

/// The largest coordinate of the frame.
const EDGE: i64 = (1 << 40) - 1;

fn point(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn sign(v: &BigInt) -> Sign {
    match v.sign() {
        num_bigint::Sign::Minus => Sign::Negative,
        num_bigint::Sign::NoSign => Sign::Zero,
        num_bigint::Sign::Plus => Sign::Positive,
    }
}

fn big(p: P3) -> [BigInt; 3] {
    [p.x.get(), p.y.get(), p.z.get()].map(BigInt::from)
}

fn minus(p: P3, q: P3) -> [BigInt; 3] {
    let ([a, b, c], [x, y, z]) = (big(p), big(q));
    [a - x, b - y, c - z]
}

fn inner(u: &[BigInt; 3], v: &[BigInt; 3]) -> BigInt {
    &u[0] * &v[0] + &u[1] * &v[1] + &u[2] * &v[2]
}

fn outer(u: &[BigInt; 3], v: &[BigInt; 3]) -> [BigInt; 3] {
    [
        &u[1] * &v[2] - &u[2] * &v[1],
        &u[2] * &v[0] - &u[0] * &v[2],
        &u[0] * &v[1] - &u[1] * &v[0],
    ]
}

fn normal(plane: &Plane) -> [BigInt; 3] {
    outer(&minus(plane.b, plane.a), &minus(plane.c, plane.a))
}

fn ball_oracle(plane: &Plane, centre: P3, radius: Steps) -> Sign {
    let n = normal(plane);
    let along = inner(&n, &minus(centre, plane.a));
    let r = BigInt::from(radius.0);
    sign(&(&r * &r * inner(&n, &n) - &along * &along))
}

fn cone_oracle(plane: &Plane, cone: &Cone) -> Sign {
    let n = normal(plane);
    let e = minus(cone.along, cone.apex);
    let (s, t) = (BigInt::from(cone.slope.rise), BigInt::from(cone.slope.run));
    let (s2, t2) = (&s * &s, &t * &t);
    let dot = inner(&n, &e);
    sign(&(&dot * &dot * (&s2 + &t2) - s2 * inner(&n, &n) * inner(&e, &e)))
}

/// The sign of X + Y for Y = ±√(y²), by the integer square root: strict unless X² = Y².
fn sum_sign(x: &BigInt, y2: &BigInt, y_positive: bool) -> Sign {
    let y = y2.sqrt();
    let exact = &y * &y == *y2;
    if exact {
        let total = if y_positive { x + &y } else { x - &y };
        return sign(&total);
    }
    let (low, high) = if y_positive {
        (x + &y, x + &y + 1)
    } else {
        (x - &y - 1, x - &y)
    };
    if sign(&low) == sign(&high) || sign(&high) == Sign::Zero {
        sign(&low)
    } else {
        sign(&high)
    }
}

/// `shared_apex`'s sum, in big integers.
fn apex_oracle(first: &Cone, second: &Cone, upper: bool) -> Sign {
    let e = minus(first.along, first.apex);
    let f = minus(second.along, second.apex);
    let run = |c: &Cone| {
        let (s, t) = (BigInt::from(c.slope.rise), BigInt::from(c.slope.run));
        (&t * &t, &s * &s + &t * &t)
    };
    let ((a, p), (b, q)) = (run(first), run(second));
    let (both, d) = (inner(&e, &e) * inner(&f, &f), inner(&e, &f));
    let pq = &p * &q;
    let x = (&both - &d * &d) * &pq - &a * &q * &both - &b * &p * &both;
    let y2 = BigInt::from(4) * &a * &b * &d * &d * &both * &pq;
    let down = sign(&d) == Sign::Negative;
    sum_sign(&x, &y2, upper != down)
}

/// `shared_apex`'s sum in doubles: what is left of a unit vector once its parts along the two axes
/// are spent.
fn apex_in_doubles(first: &Cone, second: &Cone, upper: bool) -> Option<f64> {
    let unit = |c: &Cone| {
        let v = [
            (c.along.x.get() - c.apex.x.get()) as f64,
            (c.along.y.get() - c.apex.y.get()) as f64,
            (c.along.z.get() - c.apex.z.get()) as f64,
        ];
        let size = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        v.map(|x| x / size)
    };
    let cosine = |c: &Cone| {
        let k = c.slope.rise as f64 / c.slope.run as f64;
        1.0 / (1.0 + k * k).sqrt()
    };
    let (e, f) = (unit(first), unit(second));
    let g = e[0] * f[0] + e[1] * f[1] + e[2] * f[2];
    let under = 1.0 - g * g;
    if under < 1e-3 {
        return None;
    }
    let c = cosine(first);
    let d = if upper {
        cosine(second)
    } else {
        -cosine(second)
    };
    let (a, b) = ((c - d * g) / under, (d - c * g) / under);
    Some(1.0 - a * a - b * b - 2.0 * a * b * g)
}

fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A number of at most `bits` bits, either sign.
fn draw(state: &mut u64, bits: u32) -> i64 {
    let limit = (1_i64 << bits) - 1;
    (next(state) >> 1) as i64 % (2 * limit + 1) - limit
}

fn drawn_point(state: &mut u64, bits: u32) -> P3 {
    point(draw(state, bits), draw(state, bits), draw(state, bits))
}

fn z_up() -> Plane {
    Plane {
        a: point(0, 0, 0),
        b: point(1, 0, 0),
        c: point(0, 1, 0),
    }
}

fn cone(apex: P3, along: P3, rise: u64, run: u64) -> Cone {
    Cone {
        apex,
        along,
        slope: Slope { rise, run },
    }
}

#[test]
fn a_plane_cuts_touches_or_misses_a_ball() {
    let centre = point(7, -3, 5);
    assert_eq!(plane_ball(&z_up(), centre, Steps(6)), Ok(Sign::Positive));
    assert_eq!(plane_ball(&z_up(), centre, Steps(5)), Ok(Sign::Zero));
    assert_eq!(plane_ball(&z_up(), centre, Steps(4)), Ok(Sign::Negative));
    // The far corner of the frame, where a double cannot tell.
    let far = Plane {
        a: point(EDGE, EDGE, EDGE),
        b: point(EDGE - 1, EDGE, EDGE),
        c: point(EDGE, EDGE - 1, EDGE),
    };
    let under = point(EDGE, EDGE, EDGE - (1 << 40));
    assert_eq!(plane_ball(&far, under, Steps(1 << 40)), Ok(Sign::Zero));
    assert_eq!(
        plane_ball(&far, under, Steps((1 << 40) + 1)),
        Ok(Sign::Positive)
    );
    assert_eq!(
        plane_ball(&z_up(), centre, Steps(1 << 63)),
        Err(NumError::BudgetExceeded)
    );
}

#[test]
fn a_direction_lies_along_a_plane_or_stands_across_it() {
    let o = point(0, 0, 0);
    assert_eq!(plane_along(&z_up(), o, point(5, -9, 0)), Ok(Sign::Zero));
    assert_eq!(plane_along(&z_up(), o, point(5, -9, 1)), Ok(Sign::Positive));
    assert_eq!(
        plane_along(&z_up(), o, point(5, -9, -1)),
        Ok(Sign::Negative)
    );
    assert_eq!(plane_across(&z_up(), o, point(0, 0, 3)), Ok(true));
    assert_eq!(plane_across(&z_up(), o, point(0, 0, -3)), Ok(true));
    assert_eq!(plane_across(&z_up(), o, point(1, 0, 3)), Ok(false));
    assert_eq!(plane_across(&z_up(), o, point(0, 1, 3)), Ok(false));
    assert_eq!(plane_across(&z_up(), o, point(1, 1, 0)), Ok(false));
    // The difference of a pair is the direction, wherever the pair stands.
    assert_eq!(
        plane_across(&z_up(), point(9, 9, 1), point(9, 9, 4)),
        Ok(true)
    );
}

#[test]
fn a_plane_cuts_each_conic_from_a_cone() {
    let double = cone(point(0, 0, 0), point(0, 0, 7), 1, 1);
    assert_eq!(plane_cone(&z_up(), &double), Ok(Sign::Positive));
    let ruling = Plane {
        a: point(0, 0, 0),
        b: point(0, 1, 0),
        c: point(1, 0, 1),
    };
    assert_eq!(plane_cone(&ruling, &double), Ok(Sign::Zero));
    let upright = Plane {
        a: point(0, 0, 0),
        b: point(0, 1, 0),
        c: point(0, 0, 1),
    };
    assert_eq!(plane_cone(&upright, &double), Ok(Sign::Negative));
    assert_eq!(
        plane_cone(&z_up(), &cone(point(0, 0, 0), point(0, 0, 7), 1 << 63, 1)),
        Err(NumError::BudgetExceeded)
    );
}

#[test]
fn the_decisions_agree_with_big_integers_over_the_frame() {
    let mut state = 0x165;
    for bits in [3, 8, 20, 40] {
        for _ in 0..4_000 {
            let plane = Plane {
                a: drawn_point(&mut state, bits),
                b: drawn_point(&mut state, bits),
                c: drawn_point(&mut state, bits),
            };
            let (p, q) = (drawn_point(&mut state, bits), drawn_point(&mut state, bits));
            let radius = Steps(next(&mut state) >> (64 - bits.min(53)));
            assert_eq!(
                plane_ball(&plane, p, radius),
                Ok(ball_oracle(&plane, p, radius)),
                "{plane:?} {p:?} {radius:?}"
            );
            let n = normal(&plane);
            let e = minus(q, p);
            assert_eq!(plane_along(&plane, p, q), Ok(sign(&inner(&n, &e))));
            let flat = outer(&n, &e).iter().all(|v| sign(v) == Sign::Zero);
            assert_eq!(plane_across(&plane, p, q), Ok(flat));
            let rise = 1 + (next(&mut state) >> (64 - bits.min(53)));
            let run = 1 + (next(&mut state) >> (64 - bits.min(53)));
            let c = cone(p, q, rise, run);
            assert_eq!(plane_cone(&plane, &c), Ok(cone_oracle(&plane, &c)), "{c:?}");
        }
    }
}

#[test]
fn a_plane_through_two_grid_directions_is_read_exactly_near_tangency() {
    // A plane one part in 2^40 off tangent, where the interval cannot tell.
    let plane = Plane {
        a: point(0, 0, 0),
        b: point(EDGE, 0, 1),
        c: point(0, EDGE, 0),
    };
    for r in [1, 2, 1 << 20, (1 << 40) - 1] {
        let centre = point(0, 0, r);
        assert_eq!(
            plane_ball(&plane, centre, Steps(r as u64)),
            Ok(ball_oracle(&plane, centre, Steps(r as u64)))
        );
    }
}

#[test]
fn two_cones_through_one_apex_meet_in_rulings() {
    let o = point(0, 0, 0);
    let (z, x) = (point(0, 0, 3), point(5, 0, 0));
    // Square axes at half-angles of more than 45 degrees cross in two rulings on each nappe.
    for upper in [true, false] {
        assert_eq!(
            shared_apex(&cone(o, z, 2, 1), &cone(o, x, 2, 1), upper),
            Ok(Sign::Positive)
        );
        assert_eq!(
            shared_apex(&cone(o, z, 1, 1), &cone(o, x, 1, 1), upper),
            Ok(Sign::Zero)
        );
        assert_eq!(
            shared_apex(&cone(o, z, 1, 2), &cone(o, x, 1, 2), upper),
            Ok(Sign::Negative)
        );
    }
}

#[test]
fn cones_on_a_slant_cross_touch_or_nest() {
    let o = point(0, 0, 0);
    let (z, slant) = (point(0, 0, 1), point(1, 0, 1));
    // Half-angles of atan 1 about axes 45 degrees apart cross on the near nappes, not the far.
    let (first, second) = (cone(o, z, 1, 1), cone(o, slant, 1, 1));
    assert_eq!(shared_apex(&first, &second, true), Ok(Sign::Positive));
    assert_eq!(shared_apex(&first, &second, false), Ok(Sign::Negative));
    // atan 1/3 + atan 1/2 is 45 degrees: the cones touch along one ruling, outside each other.
    let (first, second) = (cone(o, z, 1, 3), cone(o, slant, 1, 2));
    assert_eq!(shared_apex(&first, &second, true), Ok(Sign::Zero));
    assert_eq!(shared_apex(&first, &second, false), Ok(Sign::Negative));
    // atan 2 − atan 1/3 is 45 degrees: one inside the other, touching along one ruling.
    let (first, second) = (cone(o, z, 1, 3), cone(o, slant, 2, 1));
    assert_eq!(shared_apex(&first, &second, true), Ok(Sign::Zero));
    assert_eq!(shared_apex(&first, &second, false), Ok(Sign::Negative));
}

#[test]
fn the_nappes_turn_with_an_axis_and_the_order_changes_nothing() {
    let mut state = 0x1650;
    for _ in 0..4_000 {
        let o = drawn_point(&mut state, 8);
        let (x, y, z) = (o.x.get(), o.y.get(), o.z.get());
        let (e, f) = (drawn_point(&mut state, 5), drawn_point(&mut state, 5));
        let at = |p: P3, k: i64| point(x + k * p.x.get(), y + k * p.y.get(), z + k * p.z.get());
        let (s, t) = (1 + next(&mut state) % 9, 1 + next(&mut state) % 9);
        let (u, v) = (1 + next(&mut state) % 9, 1 + next(&mut state) % 9);
        let (first, second) = (cone(o, at(e, 1), s, t), cone(o, at(f, 1), u, v));
        let turned = cone(o, at(f, -1), u, v);
        for upper in [true, false] {
            assert_eq!(
                shared_apex(&first, &second, upper),
                shared_apex(&first, &turned, !upper)
            );
            assert_eq!(
                shared_apex(&first, &second, upper),
                shared_apex(&second, &first, upper)
            );
        }
    }
}

#[test]
fn two_cones_agree_with_big_integers_and_with_doubles() {
    let mut state = 0x16500;
    let mut seen = [0_u32; 3];
    for _ in 0..20_000 {
        let o = drawn_point(&mut state, 30);
        let (x, y, z) = (o.x.get(), o.y.get(), o.z.get());
        let mut along = || {
            point(
                x + draw(&mut state, 6),
                y + draw(&mut state, 6),
                z + draw(&mut state, 6),
            )
        };
        let (e, f) = (along(), along());
        if e == o || f == o {
            continue;
        }
        let (s, t) = (1 + next(&mut state) % 32, 1 + next(&mut state) % 32);
        let (u, v) = (1 + next(&mut state) % 32, 1 + next(&mut state) % 32);
        let (first, second) = (cone(o, e, s, t), cone(o, f, u, v));
        let upper = next(&mut state) % 2 == 0;
        let got = shared_apex(&first, &second, upper).unwrap();
        assert_eq!(
            got,
            apex_oracle(&first, &second, upper),
            "{first:?} {second:?}"
        );
        seen[usize::from(got == Sign::Zero) + 2 * usize::from(got == Sign::Positive)] += 1;
        let Some(rest) = apex_in_doubles(&first, &second, upper) else {
            continue;
        };
        if rest.abs() > 1e-9 {
            let want = if rest > 0.0 {
                Sign::Positive
            } else {
                Sign::Negative
            };
            assert_eq!(got, want, "{rest} {first:?} {second:?}");
        }
    }
    assert!(seen[0] > 0 && seen[2] > 0, "{seen:?}");
}

#[test]
fn the_widest_cones_fit_and_a_slope_past_i64_does_not() {
    let (o, x, z) = (
        point(-EDGE, -EDGE, -EDGE),
        point(EDGE, -EDGE, -EDGE),
        point(-EDGE, -EDGE, EDGE),
    );
    let huge = (1 << 53) - 1;
    assert_eq!(
        shared_apex(&cone(o, z, huge, 1), &cone(o, x, huge, 1), true),
        Ok(apex_oracle(
            &cone(o, z, huge, 1),
            &cone(o, x, huge, 1),
            true
        ))
    );
    assert_eq!(
        shared_apex(&cone(o, z, huge, huge - 1), &cone(o, x, 1, huge), false),
        Ok(apex_oracle(
            &cone(o, z, huge, huge - 1),
            &cone(o, x, 1, huge),
            false
        ))
    );
    let past = 1 << 63;
    assert_eq!(
        shared_apex(&cone(o, z, past, 1), &cone(o, x, 1, 1), true),
        Err(NumError::BudgetExceeded)
    );
    assert_eq!(
        shared_apex(&cone(o, z, 1, 1), &cone(o, x, 1, past), true),
        Err(NumError::BudgetExceeded)
    );
}
