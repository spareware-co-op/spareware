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
    clippy::needless_range_loop,
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
use geom_test::truth::oracle::{self, RegionSpec};
use geom_test::truth::{
    BITS, CORPUS, Enclosed, Status, Verdict, acos, asin, atan, atan2, box_volume, capsule, cos,
    cospi, fresnel, judge, pappus, pi, sin, sinpi, sphere, sphere_cap, steinmetz, tan, torus,
    villarceau,
};
use geom_topo::mass::{Corner, shell_volume};
use geom_topo::query;
use std::collections::BTreeMap;

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

/// `a` and `b` enclose the same real number, as far as either can tell, and both are tight.
fn same(a: &Enclosed, b: &Enclosed) {
    assert!(a.sub(b).holds(&RBig::ZERO), "{a:?} {b:?}");
    tight(a);
}

#[test]
fn roots_quotients_and_inverse_circular_functions_meet_their_identities() {
    let one = Enclosed::from_whole(1);
    let root = Enclosed::from_whole(2).sqrt().unwrap();
    agrees(
        &root,
        "1.41421356237309504880168872420969807856967187537694",
    );
    tight(&root);
    assert!(root.square().holds(&whole(2)));
    assert!(
        one.div(&Enclosed::from_whole(3))
            .unwrap()
            .holds(&fraction(1, 3))
    );
    assert!(one.div(&Enclosed::of(&RBig::ZERO)).is_none());
    assert!(one.sub(&Enclosed::from_whole(2)).sqrt().is_none());
    let quarter = pi().div_whole(4);
    let half = Enclosed::of(&fraction(1, 2));
    let third = Enclosed::of(&fraction(1, 3));
    same(&atan(&one).unwrap(), &quarter);
    same(&atan(&one.neg()).unwrap().neg(), &quarter);
    same(&atan(&half).unwrap().add(&atan(&third).unwrap()), &quarter);
    // atan 2 is a right angle less atan 1/2, which is past the unit interval's own series.
    same(
        &atan(&Enclosed::from_whole(2)).unwrap(),
        &pi().div_whole(2).sub(&atan(&half).unwrap()),
    );
    same(&atan2(&one, &one).unwrap(), &quarter);
    assert!(atan2(&one, &one.neg()).is_none());
    same(&asin(&half).unwrap(), &pi().div_whole(6));
    same(&acos(&half).unwrap(), &pi().div_whole(3));
    assert!(asin(&one).is_none());
    assert!(tan(&quarter).unwrap().holds(&RBig::ONE));
    assert!(tan(&pi().div_whole(3)).unwrap().square().holds(&whole(3)));
    assert!(sin(&pi().div_whole(6)).holds(&fraction(1, 2)));
    let angle = pi().div_whole(7);
    let unit = sin(&angle).square().add(&cos(&angle).square());
    assert!(unit.holds(&RBig::ONE));
    tight(&unit);
}

/// A segment of a region's loop, from its kind and formulas.
fn seg(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| (*p).to_owned()).collect()
}

#[test]
fn region_moments_meet_their_closed_forms() {
    let names = BTreeMap::new();
    let region = |loops: Vec<Vec<Vec<String>>>| RegionSpec {
        name: "r".to_owned(),
        loops,
    };
    // The rectangle 1..3 by 2..5: area 6, centroid (2, 7/2), mixed moment 4 times 21/2, perimeter
    // 10, and the boundary's first moment in x 4 + 9 + 4 + 3.
    let rectangle = region(vec![vec![seg(&["R", "1", "3", "2", "5"])]]);
    let m = oracle::region_moments(&rectangle, &names).unwrap();
    same(&m.area, &Enclosed::from_whole(6));
    same(&m.mx, &Enclosed::from_whole(12));
    same(&m.my, &Enclosed::from_whole(21));
    same(&m.mxy, &Enclosed::from_whole(42));
    same(&m.perimeter, &Enclosed::from_whole(10));
    same(&m.px, &Enclosed::from_whole(20));
    // The disc of radius 3 about (2, -1), as one arc through a full turn.
    let disc = region(vec![vec![seg(&["A", "2", "-1", "3", "0", "2*pi"])]]);
    let m = oracle::region_moments(&disc, &names).unwrap();
    let times = |n: u64| pi().mul(&Enclosed::from_whole(n));
    same(&m.area, &times(9));
    same(&m.mx, &times(18));
    same(&m.my, &times(9).neg());
    same(&m.mxy, &times(18).neg());
    same(&m.perimeter, &times(6));
    same(&m.px, &times(12));
    // The half annulus between radii 1 and 3 above the x axis.
    let annulus = region(vec![vec![seg(&["S", "0", "0", "1", "3", "0", "pi"])]]);
    let m = oracle::region_moments(&annulus, &names).unwrap();
    same(&m.area, &times(4));
    same(&m.mx, &Enclosed::from_whole(0));
    same(&m.my, &Enclosed::from_whole(52).div_whole(3));
    same(&m.perimeter, &times(4).add(&Enclosed::from_whole(4)));
}

/// The stored numbers of `SolidCraft`'s analytic cases.
fn stored() -> serde_json::Value {
    serde_json::from_str(include_str!("data/solidcraft-stored.json")).unwrap()
}

/// Whether the derived `value` is within `tolerance` of the stored number, and tight.
fn near(value: &Enclosed, stored: f64, tolerance: f64, what: &str) {
    let want = RBig::try_from(stored).unwrap();
    let allowed = RBig::try_from(tolerance).unwrap();
    assert!(value.within(&want, &allowed), "{what}: stored {stored}");
    tight(value);
}

#[test]
fn solidcraft_cases_agree_with_their_stored_values() {
    let table = stored();
    let cases = oracle::cases().unwrap();
    assert_eq!(cases.len(), 70);
    let mut complete = 0;
    for case in &cases {
        let derived = oracle::derive(case).unwrap();
        let want = &table[case.name.as_str()];
        let number = |v: &serde_json::Value| v.as_f64().unwrap();
        let low: Vec<f64> = (0..3).map(|i| number(&want["bbox"]["min"][i])).collect();
        let high: Vec<f64> = (0..3).map(|i| number(&want["bbox"]["max"][i])).collect();
        let diagonal = low
            .iter()
            .zip(&high)
            .map(|(l, h)| (h - l) * (h - l))
            .sum::<f64>()
            .sqrt();
        // SolidCraft's tolerances: relative 1e-3 on volume and area, and 1e-3 of the box's
        // diagonal on the centre and the box.
        let spatial = 1e-3 * diagonal.max(1.0);
        let relative = |x: f64| 1e-3 * x.abs().max(1.0);
        let (volume, area) = (number(&want["volume_mm3"]), number(&want["area_mm2"]));
        if let Some(v) = &derived.volume {
            near(v, volume, relative(volume), &case.name);
        }
        if let Some(a) = &derived.area {
            near(a, area, relative(area), &case.name);
        }
        for k in 0..3 {
            if let Some(c) = &derived.com[k] {
                near(c, number(&want["center_of_mass"][k]), spatial, &case.name);
            }
            if let Some(m) = &derived.min[k] {
                near(m, low[k], spatial, &case.name);
            }
            if let Some(m) = &derived.max[k] {
                near(m, high[k], spatial, &case.name);
            }
        }
        if case.complete() {
            complete += 1;
        }
    }
    // 64 are derived whole; the helical sweep lacks its side area, and the loft and the four
    // variable-radius rounds lack everything but their boxes.
    assert_eq!(complete, 64);
}

#[test]
fn every_solidcraft_case_is_held_or_awaits_a_board_piece() {
    let board = include_str!("../../../architecture/30-build-order.md");
    let cases = oracle::cases().unwrap();
    let mut by_piece: BTreeMap<String, usize> = BTreeMap::new();
    for case in &cases {
        *by_piece.entry(case.needs.clone()).or_default() += 1;
        if case.needs == "kernel" {
            continue;
        }
        assert!(
            CORPUS
                .iter()
                .any(|f| matches!(f.status, Status::Awaits(p) if p == case.needs)),
            "{}",
            case.name
        );
        assert!(
            board.contains(&format!("| {} |", case.needs)),
            "{}",
            case.name
        );
    }
    let counts: Vec<(&str, usize)> = by_piece.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(
        counts,
        [
            ("G255", 37),
            ("G256", 2),
            ("G257", 1),
            ("G259", 1),
            ("G263", 8),
            ("G266", 4),
            ("G275", 3),
            ("G277", 1),
            ("G278", 1),
            ("G472", 10),
            ("kernel", 2),
        ]
    );
}

#[test]
fn solidcraft_box_cases_hold_in_the_kernel() {
    let cases = oracle::cases().unwrap();
    for (name, (a, b, c)) in [
        ("01-box", (40, 30, 20)),
        ("06-symmetric-extrude", (50, 30, 20)),
    ] {
        let case = cases.iter().find(|case| case.name == name).unwrap();
        assert_eq!(case.needs, "kernel");
        let truth = oracle::derive(case).unwrap().volume.unwrap();
        let input = BoxInput {
            min: P3::new(-3, 4, -5).unwrap(),
            size: [Steps(a), Steps(b), Steps(c)],
        };
        let made = make_box(&ctx(), &input).unwrap();
        let six = query::volume(&made.result.body, &mut meter()).unwrap();
        let mut got = UBig::ZERO;
        for limb in six.to_le_limbs().iter().rev() {
            got = (got << 64) + UBig::from(*limb);
        }
        assert!(truth.holds(&(RBig::from(got) / whole(6))), "{name}");
    }
}
