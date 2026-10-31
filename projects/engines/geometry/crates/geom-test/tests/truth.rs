//! The closed-form truth corpus (chapter 28.9, piece G179): the truths against published digits and
//! against one another, the judge on planted results, and every Certified result the kernel gives
//! today against its closed form.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_curve::{CurvatureLaw, Frame, ParamCurve, Transition};
use geom_make::{BoxInput, make_box};
use geom_num::interval::Interval;
use geom_num::plane::{Plane as GridPlane, Tpi};
use geom_num::{P3, Steps};
use geom_nurbs::Limit;
use geom_op::{OpContext, OpId, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Sphere, Torus};
use geom_test::truth::{
    BITS, CORPUS, Enclosed, Status, Verdict, box_volume, capsule, cospi, fresnel, judge, pappus,
    pi, sinpi, sphere, sphere_cap, steinmetz, torus, villarceau,
};
use geom_topo::mass::{Corner, shell_volume};
use geom_topo::query;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn whole(n: i64) -> RBig {
    RBig::from(IBig::from(n))
}

fn fraction(n: i64, d: u64) -> RBig {
    RBig::from_parts(IBig::from(n), UBig::from(d))
}

/// A decimal string as a rational: `3.14` is 314/100.
fn decimal(text: &str) -> RBig {
    let (head, tail) = text.split_once('.').unwrap();
    let digits: IBig = format!("{head}{tail}").parse().unwrap();
    let mut scale = UBig::ONE;
    for _ in 0..tail.len() {
        scale *= UBig::from(10_u8);
    }
    RBig::from_parts(digits, scale)
}

/// Whether `e` holds the true value published to the digits of `text`.
fn agrees(e: &Enclosed, text: &str) {
    let digits = text.split_once('.').unwrap().1.len();
    let mut last = UBig::ONE;
    for _ in 0..digits {
        last *= UBig::from(10_u8);
    }
    let unit = RBig::from_parts(IBig::ONE, last);
    let middle = decimal(text);
    assert!(
        *e.lo() >= &middle - &unit && *e.hi() <= &middle + &unit,
        "{text}"
    );
}

/// A width below 2^-(`BITS` − 40): the truth is as tight as it says.
fn tight(e: &Enclosed) {
    let limit = RBig::from_parts(IBig::ONE, UBig::ONE << (BITS - 40));
    assert!(e.width() < limit, "{e:?}");
}

#[test]
fn pi_agrees_with_its_published_digits() {
    let p = pi();
    agrees(
        &p,
        "3.14159265358979323846264338327950288419716939937510582097494459",
    );
    tight(&p);
}

#[test]
fn sine_and_cosine_meet_their_identities() {
    for (q, sin, cos) in [
        (fraction(0, 1), "0.000000000000", "1.000000000000"),
        (fraction(1, 2), "1.000000000000", "0.000000000000"),
        (fraction(1, 6), "0.500000000000", "0.866025403784"),
        (fraction(1, 3), "0.866025403784", "0.500000000000"),
        (fraction(5, 4), "-0.707106781187", "-0.707106781187"),
    ] {
        agrees(&sinpi(&q), sin);
        agrees(&cospi(&q), cos);
        let one = sinpi(&q).square().add(&cospi(&q).square());
        assert!(one.holds(&RBig::ONE), "{q:?}");
        tight(&one);
    }
}

#[test]
fn fresnel_agrees_with_its_published_values() {
    // C(1) and S(1) from NIST's DLMF table 7.3 and by the same series to more digits elsewhere.
    let half_pi = pi().div_whole(2);
    let [c, s] = fresnel(&half_pi, &RBig::ONE);
    agrees(&c, "0.7798934003768228");
    agrees(&s, "0.4382591473903548");
    tight(&c);
    tight(&s);
    // k = 0 is a straight line.
    let [x, y] = fresnel(&Enclosed::of(&RBig::ZERO), &whole(3));
    assert!(x.holds(&whole(3)));
    assert!(y.holds(&RBig::ZERO));
}

#[test]
fn the_closed_forms_agree_with_one_another() {
    let r = whole(7);
    // A cap of the whole height is the ball, a capsule of no length is the ball.
    let ball = sphere(&r);
    assert!(sphere_cap(&r, &whole(14)).sub(&ball).holds(&RBig::ZERO));
    assert!(capsule(&r, &RBig::ZERO).sub(&ball).holds(&RBig::ZERO));
    // A torus is Pappus's revolve of a disc.
    let disc = pi().mul(&Enclosed::of(&(whole(3) * whole(3))));
    assert!(
        torus(&whole(11), &whole(3))
            .sub(&pappus(&disc, &whole(11)))
            .holds(&RBig::ZERO)
    );
    // A cap's slices, Simpson's rule exact on its cubic: π ∫ (r² − x²) dx from r − h to r.
    let (r, h) = (fraction(5, 1), fraction(2, 1));
    let slice = |x: &RBig| &r * &r - x * x;
    let a = &r - &h;
    let m = (&a + &r) / whole(2);
    let simpson = &h / whole(6) * (slice(&a) + whole(4) * slice(&m) + slice(&r));
    let cap = pi().mul(&Enclosed::of(&simpson));
    assert!(cap.sub(&sphere_cap(&r, &h)).holds(&RBig::ZERO));
    // The Steinmetz solid's square slices: 4(r² − x²) across [−r, r], the same rule.
    let square = |x: &RBig| whole(4) * (&r * &r - x * x);
    let ends = square(&-r.clone()) + square(&r);
    let across = &r * whole(2) / whole(6) * (ends + whole(4) * square(&RBig::ZERO));
    assert_eq!(across, steinmetz(&r));
    assert_eq!(box_volume(&whole(2), &whole(3), &whole(5)), whole(30));
}

#[test]
fn a_villarceau_point_is_on_its_torus_and_its_plane() {
    for unit in [1_i64, 2, 7, 1000] {
        let [x, y, z] = villarceau(unit);
        let (major, minor) = (25 * unit, 15 * unit);
        // The plane through the centre of slope 3/4: 3x = 4z.
        assert_eq!(3 * x, 4 * z);
        // On the torus: (ρ − R)² + z² = r², with ρ² = x² + y² a perfect square here.
        let rho = 37 * unit;
        assert_eq!(x * x + y * y, rho * rho);
        assert_eq!((rho - major).pow(2) + z * z, minor * minor);
    }
}

/// `(lo, hi)` as an interval, for planted results.
fn interval(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

#[test]
fn the_judge_names_each_verdict() {
    let third = Enclosed::of(&fraction(1, 3));
    let near = 1.0 / 3.0_f64;
    let tight_claim = interval(near.next_down(), near.next_up());
    assert_eq!(judge(tight_claim, &third), Verdict::Within);
    // A result that misses by a step of the truth.
    assert_eq!(
        judge(interval(near.next_up(), near.next_up().next_up()), &third),
        Verdict::Broken
    );
    let below = near.next_down();
    assert_eq!(
        judge(interval(below.next_down(), below), &third),
        Verdict::Broken
    );
    // The truth's own width counts: a result ending inside it does not hold it.
    let wide = Enclosed::of(&fraction(1, 3)).add(&Enclosed::of(&fraction(1, 1)));
    assert_eq!(judge(interval(1.0, 4.0 / 3.0), &wide), Verdict::Broken);
    // A bound a thousand times too wide is reported, and one just inside is not.
    assert_eq!(judge(interval(0.0, 2.0 / 3.0), &third), Verdict::Loose);
    let unit = near * f64::EPSILON;
    assert_eq!(
        judge(interval(near - 900.0 * unit, near + 900.0 * unit), &third),
        Verdict::Within
    );
    assert_eq!(
        judge(interval(near - 5000.0 * unit, near + 5000.0 * unit), &third),
        Verdict::Loose
    );
}

#[test]
fn the_corpus_names_a_check_or_a_piece_for_each_family() {
    let tests = include_str!("truth.rs");
    let board = include_str!("../../../architecture/30-build-order.md");
    for family in CORPUS {
        match family.status {
            Status::Checked(test) => {
                assert!(tests.contains(&format!("fn {test}()")), "{}", family.name);
            }
            Status::Awaits(piece) => {
                assert!(board.contains(&format!("| {piece} |")), "{}", family.name);
            }
        }
    }
}

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([3; 16])), Budget::UNLIMITED)
}

#[test]
fn boxes_are_exact() {
    for (a, b, c) in [
        (1, 1, 1),
        (3, 5, 7),
        (1000, 2000, 3000),
        (1 << 20, 1 << 20, 1 << 20),
    ] {
        let input = BoxInput {
            min: P3::new(-3, 4, -5).unwrap(),
            size: [Steps(a), Steps(b), Steps(c)],
        };
        let made = make_box(&ctx(), &input).unwrap();
        let six = query::volume(&made.result.body, &mut meter()).unwrap();
        let limbs = six.to_le_limbs();
        let mut got = UBig::ZERO;
        for limb in limbs.iter().rev() {
            got = (got << 64) + UBig::from(*limb);
        }
        let want = box_volume(&whole(a as i64), &whole(b as i64), &whole(c as i64)) * whole(6);
        assert_eq!(RBig::from(got), want);
    }
}

#[test]
fn a_cut_corner_is_held_by_its_interval() {
    // The corner cut from the first octant by 3x + y + z = s: its corner on the x axis is the
    // meeting of three planes, s/3 from the origin, so six times the volume is s³/3.
    for s in [3_i64, 7, 1000, 1 << 30] {
        let p = |x: i64, y: i64, z: i64| P3::new(x, y, z).unwrap();
        let tilted = GridPlane::through(p(0, s, 0), p(0, 0, s), p(s, -s, -s)).unwrap();
        let y0 = GridPlane::through(p(0, 0, 0), p(1, 0, 0), p(0, 0, 1)).unwrap();
        let z0 = GridPlane::through(p(0, 0, 0), p(1, 0, 0), p(0, 1, 0)).unwrap();
        let x = Corner::Tpi(Tpi::new(tilted, y0, z0).unwrap());
        let (o, y, z) = (
            Corner::Grid(p(0, 0, 0)),
            Corner::Grid(p(0, s, 0)),
            Corner::Grid(p(0, 0, s)),
        );
        let faces = [
            [o.clone(), y.clone(), x.clone()],
            [o.clone(), x.clone(), z.clone()],
            [o, z.clone(), y.clone()],
            [x, y, z],
        ];
        let loops: Vec<&[Corner]> = faces.iter().map(<[Corner; 3]>::as_slice).collect();
        let six = shell_volume(&loops, &mut meter()).unwrap();
        let truth = Enclosed::of(&(whole(s) * whole(s) * whole(s) / whole(3)));
        assert_eq!(judge(six, &truth), Verdict::Within, "{s}");
    }
}

/// A point of a surface at (u, v) against its closed form.
fn point_holds(surface: &impl ParamSurface, u: f64, v: f64, truth: &[Enclosed; 3], name: &str) {
    let sides = (Limit::FromRight, Limit::FromRight);
    let at = surface.enclose(u, v, sides, &mut meter()).unwrap();
    for (claim, want) in at[0].iter().zip(truth) {
        assert_eq!(judge(*claim, want), Verdict::Within, "{name} {u} {v}");
    }
}

/// `x` as an exact rational.
fn exact(x: f64) -> RBig {
    RBig::try_from(x).unwrap()
}

#[test]
fn surface_points_hold_their_closed_forms() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let centre = [7.0, -3.0, 2.0];
    let offset = |k: usize, value: Enclosed| Enclosed::of(&exact(centre[k])).add(&value);
    let params = [
        (0.0, 0.5),
        (0.125, -0.25),
        (0.5, 0.25),
        (0.75, 0.125),
        (1.25, -0.375),
        (1.875, 0.0625),
    ];
    let (r, major) = (fraction(5, 1), fraction(19, 1));
    // Sphere: u round the axis, v the latitude.
    let ball = Sphere::new(centre, 5.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    // Torus: u round the axis, v round the tube.
    let ring = Torus::new(centre, 19.0, 5.0, frame, ((0.0, 2.0), (-1.0, 1.0))).unwrap();
    // Cylinder: u round the axis, v the height.
    let round = Cylinder::new(centre, 5.0, frame, ((0.0, 2.0), (-10.0, 10.0))).unwrap();
    // Cone: radius 5 + v/2.
    let cone = Cone::new(centre, 5.0, 0.5, frame, ((0.0, 2.0), (-9.0, 9.0))).unwrap();
    for (u, v) in params {
        let (cu, su) = (cospi(&exact(u)), sinpi(&exact(u)));
        let (cv, sv) = (cospi(&exact(v)), sinpi(&exact(v)));
        let radius = Enclosed::of(&r);
        let flat = radius.mul(&cv);
        let want = [
            offset(0, flat.mul(&cu)),
            offset(1, flat.mul(&su)),
            offset(2, radius.mul(&sv)),
        ];
        point_holds(&ball, u, v, &want, "sphere");
        let wide = Enclosed::of(&major).add(&flat);
        let want = [
            offset(0, wide.mul(&cu)),
            offset(1, wide.mul(&su)),
            offset(2, radius.mul(&sv)),
        ];
        point_holds(&ring, u, v, &want, "torus");
        let want = [
            offset(0, radius.mul(&cu)),
            offset(1, radius.mul(&su)),
            offset(2, Enclosed::of(&exact(v))),
        ];
        point_holds(&round, u, v, &want, "cylinder");
        let opened = radius.add(&Enclosed::of(&exact(v)).div_whole(2));
        let want = [
            offset(0, opened.mul(&cu)),
            offset(1, opened.mul(&su)),
            offset(2, Enclosed::of(&exact(v))),
        ];
        point_holds(&cone, u, v, &want, "cone");
    }
}

#[test]
fn clothoid_points_hold_their_fresnel_integrals() {
    for (end, length) in [(1.0, 1.0), (0.5, 4.0), (0.001, 200.0), (2.0, 0.75)] {
        let law = CurvatureLaw::Clothoid { start: 0.0, end };
        let curve =
            Transition::<2>::new([0.0, 0.0], [1.0, 0.0], length, law, &mut meter()).unwrap();
        // κ(σ) = (end/L)·σ, so the heading is k·σ² with k = end/(2L).
        let k = Enclosed::of(&(exact(end) / exact(length) / RBig::from(UBig::from(2_u8))));
        for share in [0.125, 0.5, 0.875, 1.0] {
            let station = share * length;
            let [x, y] = fresnel(&k, &exact(station));
            let side = if share < 1.0 {
                Limit::FromRight
            } else {
                Limit::FromLeft
            };
            let at = curve.enclose(station, side, 0, &mut meter()).unwrap();
            assert_eq!(
                judge(at[0][0], &x),
                Verdict::Within,
                "x {end} {length} {share}"
            );
            assert_eq!(
                judge(at[0][1], &y),
                Verdict::Within,
                "y {end} {length} {share}"
            );
        }
    }
}
