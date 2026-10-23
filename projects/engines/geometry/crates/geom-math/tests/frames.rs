//! Tests of `geom_math::frame` against exact rational arithmetic (`num-rational`).
//!
//! What this file proves:
//!
//! - Map to frame, map frames and quarter turns: every result is the exact value rounded to the
//!   nearest step, ties to even, with the exact flag exactly right, for random map coordinates
//!   (arbitrary doubles and dyadic values that land on steps and midpoints) at q0, q1 and q2;
//!   out-of-range results are refused exactly when the rounded value leaves ±(2^40 − 1).
//! - Map to frame, rational Ground frames (Pythagorean directions, scale factors such as
//!   0.99987654): the same exactness, ties included, although the arithmetic is double-double.
//! - Map to frame, irrational directions: every result is the exact value correctly rounded
//!   (checked with exact comparisons of squares), out of range exactly when it should be, and
//!   exact only where both horizontal coordinates are zero.
//! - Frame to map: the nearest doubles to the exact map coordinates, ties to even, with the exact
//!   flag right, for every frame, irrational directions included (checked between the midpoints
//!   with exact comparisons of squares). At q0, frame → map → frame is the identity.
//! - The NZTM2000 example: a site near Auckland, a value that lands on steps converts exactly both
//!   ways, and an arbitrary survey value round-trips within half a step.
//! - Lattices: shifts between frames are exact (checked against the exact map position of the
//!   shifted point), refine q0 to q1 by 25, connect neighbouring frames of a 2^39-step lattice,
//!   and refuse every mismatch with the documented error, a different vertical CRS included.
//! - Frame kinds: identity rotation and unit scale make a Map frame, anything else a Ground frame;
//!   scale ratios far from one (chapter 17.1 sets no limit) convert exactly.
//! - Regional lattices: cell origins every 2^39 steps, half-open cell ownership, the home cell of a
//!   box and of operands spread over several cells, and `FrameRangeExceeded` when they do not fit.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::unreadable_literal,
    reason = "test cases and oracles, named as in the formulas they check; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]

mod common;

use std::cmp::Ordering;

use common::{
    config, exact, int, is_integer, is_nearest_double, round_half_even, sign_minus_sqrt, step_down,
    step_up, to_i128,
};
use geom_math::frame::{CELL_HALF, LATTICE_SPACING, ORIGIN_MAX, steps_per_metre};
use geom_math::{
    Box3, CrsId, FrameKind, FrameMismatch, FrameRecord, MapPoint, MathError, Rotation, Scale,
};
use geom_num::grid::{COORD_MAX, GridStep, P3};
use geom_num::{NumError, math};
use num_rational::BigRational;
use proptest::prelude::*;

const NZTM: CrsId = CrsId::Epsg(2193);

/// Pythagorean directions and quarter turns: rational rotations.
const RATIONAL_DIRECTIONS: [(i32, i32); 12] = [
    (1, 0),
    (0, 1),
    (-1, 0),
    (0, -1),
    (3, 4),
    (4, -3),
    (-5, 12),
    (-12, -5),
    (8, 15),
    (20, 21),
    (119, -120),
    (-7, 24),
];

/// No rotation: map frames.
const MAP_DIRECTIONS: [(i32, i32); 1] = [(1, 0)];

/// Directions whose length is irrational.
const IRRATIONAL_DIRECTIONS: [(i32, i32); 5] = [(1, 2), (2, 3), (-5, 7), (1000, 1), (12345, -6789)];

/// Scale factors as (num, den).
const SCALES: [(u32, u32); 7] = [
    (1, 1),
    (99_987_654, 100_000_000),
    (9996, 10000),
    (3, 2),
    (2, 3),
    (1, 2),
    (1_000_001, 1_000_000),
];

fn s_of(step: GridStep) -> i128 {
    i128::from(steps_per_metre(step))
}

/// The exact frame coordinate along (α, β) for a rational frame: `k_d·(α·u` + `β·v)/(k_n·r`).
fn exact_horizontal(f: &FrameRecord, e: f64, n: f64, alpha: i128, beta: i128) -> BigRational {
    let s = int(s_of(f.step()));
    let [oe, on, _] = f.origin();
    let u = exact(e) * &s - int(oe.into());
    let v = exact(n) * &s - int(on.into());
    let r = i128::from(f.rotation().exact_norm().expect("rational frame"));
    let (kn, kd) = (i128::from(f.scale().num()), i128::from(f.scale().den()));
    (u * int(alpha) + v * int(beta)) * int(kd) / int(kn * r)
}

/// The exact frame coordinates (x, y, z) of a map point in a rational frame.
fn exact_frame(f: &FrameRecord, m: MapPoint) -> [BigRational; 3] {
    let (c, s) = (
        i128::from(f.rotation().cos()),
        i128::from(f.rotation().sin()),
    );
    let [_, _, oh] = f.origin();
    [
        exact_horizontal(f, m.easting, m.northing, c, s),
        exact_horizontal(f, m.easting, m.northing, -s, c),
        exact(m.height) * int(s_of(f.step())) - int(oh.into()),
    ]
}

/// The exact map coordinates (E, N, H) in metres of a grid point in a rational frame.
fn exact_map(f: &FrameRecord, p: P3) -> [BigRational; 3] {
    let s = int(s_of(f.step()));
    let [oe, on, oh] = f.origin().map(|o| int(o.into()));
    let (x, y, z) = (
        int(p.x.get().into()),
        int(p.y.get().into()),
        int(p.z.get().into()),
    );
    let (c, sn) = (
        int(f.rotation().cos().into()),
        int(f.rotation().sin().into()),
    );
    let r = int(f.rotation().exact_norm().expect("rational frame").into());
    let k = int(f.scale().num().into()) / int(f.scale().den().into());
    let ue = &k * (&c * &x - &sn * &y) / &r;
    let un = &k * (&sn * &x + &c * &y) / &r;
    [(oe + ue) / &s, (on + un) / &s, (oh + z) / &s]
}

/// Checks a conversion against the exact values: correctly rounded, ties to even, exact flag right,
/// or `OutOfRange` exactly when a rounded coordinate leaves the frame.
fn check_rounded(f: &FrameRecord, m: MapPoint) -> Result<(), TestCaseError> {
    let want = exact_frame(f, m);
    let rounded: Vec<i128> = want.iter().map(|w| to_i128(&round_half_even(w))).collect();
    let out = rounded
        .iter()
        .any(|v| v.unsigned_abs() > COORD_MAX.unsigned_abs().into());
    match f.to_frame(m) {
        Ok(r) => {
            prop_assert!(!out, "accepted an out-of-range point {m:?}");
            let got = [r.value().x.get(), r.value().y.get(), r.value().z.get()].map(i128::from);
            prop_assert_eq!(got.to_vec(), rounded, "frame {:?} point {:?}", f, m);
            prop_assert_eq!(r.is_exact(), want.iter().all(is_integer));
        }
        Err(NumError::OutOfRange { .. }) => prop_assert!(out, "refused an in-range point {m:?}"),
        Err(e) => prop_assert!(false, "unexpected error {e:?}"),
    }
    Ok(())
}

fn step() -> BoxedStrategy<GridStep> {
    prop_oneof![6 => Just(GridStep::Q0), 1 => Just(GridStep::Q1), 1 => Just(GridStep::Q2)].boxed()
}

/// An origin of whole metres plus a few extra steps, so map values near whole metres land near
/// steps and midpoints.
fn origin_parts() -> BoxedStrategy<([i64; 3], [i64; 3])> {
    (
        [
            -3_000_000i64..3_000_000,
            4_000_000i64..7_000_000,
            -1000i64..3000,
        ],
        prop::array::uniform3(prop_oneof![Just(0i64), -1000i64..1000]),
    )
        .boxed()
}

/// A map offset from the origin's whole metres: arbitrary, dyadic with a small denominator, or
/// zero.
fn offset(range: f64) -> BoxedStrategy<f64> {
    let j = (range * 4096.0) as i64;
    prop_oneof![
        3 => (-j..=j).prop_map(|j| j as f64 / 4096.0),
        2 => -range..range,
        1 => Just(0.0f64),
        1 => (-64i64..=64).prop_map(|j| j as f64 / 4096.0),
    ]
    .boxed()
}

/// A frame and a map point near its origin.
fn frame_and_point(directions: &'static [(i32, i32)]) -> BoxedStrategy<(FrameRecord, MapPoint)> {
    let offsets = (offset(200_000.0), offset(200_000.0), offset(300.0));
    (
        step(),
        origin_parts(),
        0..directions.len(),
        0..SCALES.len(),
        offsets,
    )
        .prop_map(move |(step, (metres, extra), d, k, (de, dn, dh))| {
            let s = steps_per_metre(step) as i64;
            // Keep the origin within 2^60 at every step: whole metres times the steps per metre.
            let origin = [0, 1, 2].map(|i| metres[i] * s + extra[i]);
            let (c, sn) = directions[d];
            let rot = Rotation::direction(c, sn).unwrap();
            let (kn, kd) = SCALES[k];
            let f = FrameRecord::new(NZTM, step, origin, rot, Scale::new(kn, kd).unwrap()).unwrap();
            let m = MapPoint {
                easting: metres[0] as f64 + de,
                northing: metres[1] as f64 + dn,
                height: metres[2] as f64 + dh,
            };
            (f, m)
        })
        .boxed()
}

proptest! {
    #![proptest_config(config(3000))]

    #[test]
    fn rational_frames_round_exactly((f, m) in frame_and_point(&RATIONAL_DIRECTIONS)) {
        check_rounded(&f, m)?;
    }

    #[test]
    fn map_frames_round_exactly((f, m) in frame_and_point(&MAP_DIRECTIONS)) {
        prop_assert!(f.rotation() == Rotation::IDENTITY);
        check_rounded(&FrameRecord::map(f.crs(), f.step(), f.origin()).unwrap(), m)?;
    }

    #[test]
    fn irrational_frames_round_exactly((f, m) in frame_and_point(&IRRATIONAL_DIRECTIONS)) {
        let s = int(s_of(f.step()));
        let [oe, on, oh] = f.origin();
        let u = exact(m.easting) * &s - int(oe.into());
        let v = exact(m.northing) * &s - int(on.into());
        let (c, sn) = (i128::from(f.rotation().cos()), i128::from(f.rotation().sin()));
        let n2 = f.rotation().norm2();
        let (kn, kd) = (int(f.scale().num().into()), int(f.scale().den().into()));
        let half = BigRational::new(1.into(), 2.into());
        // X = A / (k_n·√n2) with A = k_d·(α·u + β·v): irrational unless A = 0, so never a tie.
        let ax = (&u * int(c) + &v * int(sn)) * &kd;
        let ay = (&v * int(c) - &u * int(sn)) * &kd;
        let z = exact(m.height) * &s - int(oh.into());
        // |X| < 2^40 − 1/2 exactly: the rounded coordinate is in range.
        let limit = (int(COORD_MAX.into()) + &half) * &kn;
        let fits = |a: &BigRational| {
            sign_minus_sqrt(a, &limit, n2) == Ordering::Less
                && sign_minus_sqrt(a, &(-limit.clone()), n2) == Ordering::Greater
        };
        let z_round = to_i128(&round_half_even(&z));
        let z_in = z_round.unsigned_abs() <= COORD_MAX.unsigned_abs().into();
        let in_range = fits(&ax) && fits(&ay) && z_in;
        match f.to_frame(m) {
            Ok(r) => {
                prop_assert!(in_range, "accepted an out-of-range point {m:?}");
                for (a, got) in [(&ax, r.value().x.get()), (&ay, r.value().y.get())] {
                    let g = int(got.into());
                    // got − 1/2 < X < got + 1/2, exactly: the correctly rounded value.
                    let lo = (&g - &half) * &kn;
                    let hi = (&g + &half) * &kn;
                    prop_assert_eq!(sign_minus_sqrt(a, &lo, n2), Ordering::Greater);
                    prop_assert_eq!(sign_minus_sqrt(a, &hi, n2), Ordering::Less);
                }
                prop_assert_eq!(i128::from(r.value().z.get()), z_round);
                // Exact only where every coordinate is: X and Y are exact only when zero.
                let want = ax == int(0) && ay == int(0) && is_integer(&z);
                prop_assert_eq!(r.is_exact(), want);
            }
            Err(NumError::OutOfRange { .. }) => prop_assert!(!in_range, "refused {m:?}"),
            Err(e) => prop_assert!(false, "unexpected error {e:?}"),
        }
    }

    #[test]
    fn frame_to_map_is_correctly_rounded(
        (f, _) in frame_and_point(&RATIONAL_DIRECTIONS),
        p in prop::array::uniform3(prop_oneof![
            -COORD_MAX..=COORD_MAX,
            Just(COORD_MAX),
            Just(-COORD_MAX),
            -1000i64..1000
        ]),
    ) {
        let p = P3::new(p[0], p[1], p[2]).unwrap();
        let r = f.to_map(p).unwrap();
        let want = exact_map(&f, p);
        let got = [r.value().easting, r.value().northing, r.value().height];
        for (g, w) in got.iter().zip(want.iter()) {
            prop_assert!(is_nearest_double(*g, w), "{g} is not the double nearest {w}");
        }
        prop_assert_eq!(r.is_exact(), got.iter().zip(want.iter()).all(|(g, w)| exact(*g) == *w));
        // At q0 the map values' ulps are far below a step, so converting back is the identity.
        if f.step() == GridStep::Q0 {
            prop_assert_eq!(f.to_frame(r.value()).unwrap().value(), p);
        }
    }

    #[test]
    fn frame_to_map_irrational_is_correctly_rounded(
        (f, _) in frame_and_point(&IRRATIONAL_DIRECTIONS),
        p in prop::array::uniform3(prop_oneof![-COORD_MAX..=COORD_MAX, -1000i64..1000]),
    ) {
        let p = P3::new(p[0], p[1], p[2]).unwrap();
        let r = f.to_map(p).unwrap();
        let s = int(s_of(f.step()));
        let [oe, on, _] = f.origin();
        let (c, sn) = (i128::from(f.rotation().cos()), i128::from(f.rotation().sin()));
        let (x, y) = (i128::from(p.x.get()), i128::from(p.y.get()));
        let n2 = f.rotation().norm2();
        let (kn, kd) = (int(f.scale().num().into()), int(f.scale().den().into()));
        // T(E) = (E·S − O)·k_d/k_n must bracket a/√n2 strictly between E's two midpoints.
        let axes = [(r.value().easting, oe, c * x - sn * y), (r.value().northing, on, sn * x + c * y)];
        for (e, o, a) in axes {
            let t = |d: f64| (exact(d) * &s - int(o.into())) * &kd / &kn;
            let mid = |d: f64| (exact(d) + exact(e)) / int(2) * &s;
            let tm = |d: f64| (mid(d) - int(o.into())) * &kd / &kn;
            let a = int(a);
            if a == int(0) {
                // The rational value O/S, correctly rounded and exact or not.
                prop_assert!(is_nearest_double(e, &(int(o.into()) / &s)));
                continue;
            }
            prop_assert_eq!(sign_minus_sqrt(&a, &tm(step_down(e)), n2), Ordering::Greater);
            prop_assert_eq!(sign_minus_sqrt(&a, &tm(step_up(e)), n2), Ordering::Less);
            prop_assert_ne!(sign_minus_sqrt(&a, &t(e), n2), Ordering::Equal);
        }
        prop_assert!(!r.is_exact() || (c * x - sn * y == 0 && sn * x + c * y == 0));
        if f.step() == GridStep::Q0 {
            prop_assert_eq!(f.to_frame(r.value()).unwrap().value(), p);
        }
    }

    #[test]
    fn lattice_shifts_are_exact(
        (f, _) in frame_and_point(&RATIONAL_DIRECTIONS),
        moves in prop::array::uniform3(-1_000_000i64..1_000_000),
        rungs in 0u32..3,
        p in prop::array::uniform3(-1_000_000i64..1_000_000),
    ) {
        let to_step = match (f.step().rung() + rungs).min(2) {
            0 => GridStep::Q0,
            1 => GridStep::Q1,
            _ => GridStep::Q2,
        };
        let factor = [1i64, 25, 625][(to_step.rung() - f.step().rung()) as usize];
        // A target whose origin differs by whole steps along the target's own axes: move the source
        // origin by a frame vector, mapped to map steps with the frame's rotation and scale.
        let (c, sn) = (i128::from(f.rotation().cos()), i128::from(f.rotation().sin()));
        let r = i128::from(f.rotation().exact_norm().unwrap());
        let (kn, kd) = (i128::from(f.scale().num()), i128::from(f.scale().den()));
        let [mx, my, mz] = moves.map(i128::from);
        // Map displacement k·R·(m·r·k_d) is an integer: k_n·(c·m_x − s·m_y)·… / (k_d·r).
        let de = kn * (c * mx - sn * my);
        let dn = kn * (sn * mx + c * my);
        let [oe, on, oh] = f.origin().map(i128::from);
        let fac = i128::from(factor);
        let target_origin =
            [fac * oe + de, fac * on + dn, fac * oh + mz].map(|v| i64::try_from(v).unwrap());
        let target =
            FrameRecord::new(f.crs(), to_step, target_origin, f.rotation(), f.scale()).unwrap();
        let shift = f.shift_to(&target).unwrap();
        prop_assert_eq!(shift.refine(), factor);
        let want = [-(mx * kd * r), -(my * kd * r), -mz].map(|v| i64::try_from(v).unwrap());
        prop_assert_eq!(shift.offset(), want);
        let p = P3::new(p[0], p[1], p[2]).unwrap();
        if let Ok(q) = shift.apply(p) {
            // The same physical point: equal exact map coordinates.
            prop_assert_eq!(exact_map(&f, p), exact_map(&target, q));
        }
    }
}

/// The exact value of a coordinate is a midpoint between two steps.
fn is_midpoint(x: &BigRational) -> bool {
    is_integer(&(x * int(2))) && !is_integer(x)
}

#[test]
fn ground_frame_sweep_settles_ties_and_exact_values() {
    let s = steps_per_metre(GridStep::Q0) as i64;
    let origin = [1000 * s, 2000 * s, 0];
    let frames = [
        ((3, 4), (1, 1)),
        ((-5, 12), (1, 1)),
        ((0, 1), (2, 3)),
        ((8, 15), (1, 1)),
        ((4, 3), (3, 2)),
    ];
    let (mut ties, mut exacts, mut total) = (0, 0, 0);
    for ((c, sn), (kn, kd)) in frames {
        let rot = Rotation::direction(c, sn).unwrap();
        let k = Scale::new(kn, kd).unwrap();
        let f = FrameRecord::new(NZTM, GridStep::Q0, origin, rot, k).unwrap();
        // E = 1000 + j/4096 m gives u = 1562.5·j steps: rotated and scaled, many coordinates land
        // on whole steps or exactly on midpoints, the cases the exact settling decides.
        for j in -60i32..=60 {
            for l in -60i32..=60 {
                let m = MapPoint {
                    easting: 1000.0 + f64::from(j) / 4096.0,
                    northing: 2000.0 + f64::from(l) / 4096.0,
                    height: 0.0,
                };
                check_rounded(&f, m).unwrap();
                let want = exact_frame(&f, m);
                ties += usize::from(want[..2].iter().any(is_midpoint));
                exacts += usize::from(want.iter().all(is_integer));
                total += 1;
            }
        }
    }
    // Both cases are common, so the exact settling is exercised thousands of times.
    assert!(ties > total / 20, "{ties} ties in {total}");
    assert!(exacts > total / 50, "{exacts} exact points in {total}");
}

#[test]
fn nztm2000_example() {
    let s = steps_per_metre(GridStep::Q0);
    assert_eq!(s, 6_400_000);
    // A site near central Auckland: E 1,757,000 m, N 5,921,000 m, NZVD2016 height 0.
    let origin = [1_757_000 * s as i64, 5_921_000 * s as i64, 0];
    // The northing origin is about 2^45 steps from the grid's zero.
    assert!(origin[1] > 1 << 45 && origin[1] < 1 << 46 && origin[1] < ORIGIN_MAX);
    let site = FrameRecord::map(NZTM, GridStep::Q0, origin).unwrap();

    // Values that land on whole steps convert exactly, both ways.
    let m = MapPoint {
        easting: 1_757_000.25,
        northing: 5_921_000.125,
        height: 12.5,
    };
    let r = site.to_frame(m).unwrap();
    assert!(r.is_exact());
    assert_eq!(r.value(), P3::new(1_600_000, 800_000, 80_000_000).unwrap());
    let back = site.to_map(r.value()).unwrap();
    assert!(back.is_exact());
    assert_eq!(back.value(), m);

    // An arbitrary survey value rounds once, and round-trips within half a step.
    let survey = MapPoint {
        easting: 1_757_123.456_7,
        northing: 5_921_456.789,
        height: 34.567,
    };
    let r = site.to_frame(survey).unwrap();
    assert!(!r.is_exact());
    let back = site.to_map(r.value()).unwrap().value();
    let half_step = 0.5 / s as f64;
    let pairs = [
        (back.easting, survey.easting),
        (back.northing, survey.northing),
        (back.height, survey.height),
    ];
    for (b, o) in pairs {
        let ulp = step_up(o) - o;
        assert!(
            math::abs(b - o).unwrap().get() <= half_step + ulp,
            "{b} vs {o}"
        );
    }
    assert_eq!(
        site.to_frame(site.to_map(r.value()).unwrap().value())
            .unwrap()
            .value(),
        r.value()
    );

    // A Ground frame at the same site: rotated by the Pythagorean direction (4, 3), scaled by
    // 0.99987654. Exact data still converts exactly where the exact value is a whole step.
    let ground = FrameRecord::new(
        NZTM,
        GridStep::Q0,
        origin,
        Rotation::direction(4, 3).unwrap(),
        Scale::new(99_987_654, 100_000_000).unwrap(),
    )
    .unwrap();
    // c·x − s·y = 5^14 and s·x + c·y = 0, so E = 1,757,000 + k_n/2^18 m exactly, and z is 0.5 m.
    let p = P3::new(976_562_500, -732_421_875, 3_200_000).unwrap();
    let m = ground.to_map(p).unwrap();
    assert_eq!(m.value().easting, 1_757_000.0 + 49_993_827.0 / 262_144.0);
    assert_eq!((m.value().northing, m.value().height), (5_921_000.0, 0.5));
    assert!(m.is_exact(), "{m:?}");
    let r = ground.to_frame(m.value()).unwrap();
    assert_eq!(r.value(), p);
    assert!(r.is_exact());
}

#[test]
fn ties_go_to_even() {
    let s = steps_per_metre(GridStep::Q0) as i64;
    let f = FrameRecord::map(NZTM, GridStep::Q0, [1000 * s, 2000 * s, 0]).unwrap();
    // E = 1000 + j/4096 m gives E·S − O = 1562.5·j steps: a midpoint for odd j.
    let x = |j: i64| {
        let m = MapPoint {
            easting: 1000.0 + j as f64 / 4096.0,
            northing: 2000.0,
            height: 0.0,
        };
        let r = f.to_frame(m).unwrap();
        (r.value().x.get(), r.is_exact())
    };
    assert_eq!(x(1), (1562, false));
    assert_eq!(x(3), (4688, false));
    assert_eq!(x(-1), (-1562, false));
    assert_eq!(x(-3), (-4688, false));
    assert_eq!(x(2), (3125, true));
    // A frame whose origin is one step further along: the same map value is now 1561.5, which
    // also rounds to even, to 1562, a different map position (see the module documentation).
    let g = FrameRecord::map(NZTM, GridStep::Q0, [1000 * s + 1, 2000 * s, 0]).unwrap();
    let m = MapPoint {
        easting: 1000.0 + 1.0 / 4096.0,
        northing: 2000.0,
        height: 0.0,
    };
    assert_eq!(g.to_frame(m).unwrap().value().x.get(), 1562);
    // A Ground frame with direction (3, 4) and origin 0: E = j/4096 m gives u = 1562.5·j steps,
    // so x = 3u/5 = 937.5·j is a midpoint for odd j, and y = −4u/5 = −1250·j is exact.
    let r34 = Rotation::direction(3, 4).unwrap();
    let q = FrameRecord::new(NZTM, GridStep::Q0, [0, 0, 0], r34, Scale::ONE).unwrap();
    let at = |j: i64| {
        let m = MapPoint {
            easting: j as f64 / 4096.0,
            northing: 0.0,
            height: 0.0,
        };
        let r = q.to_frame(m).unwrap();
        (r.value().x.get(), r.value().y.get(), r.is_exact())
    };
    assert_eq!(at(1), (938, -1250, false));
    assert_eq!(at(3), (2812, -3750, false));
    assert_eq!(at(-1), (-938, 1250, false));
    assert_eq!(at(2), (1875, -2500, true));
}

#[test]
fn range_limits_at_2_40() {
    let m0 = MapPoint {
        easting: 0.0,
        northing: 0.0,
        height: 0.0,
    };
    let at = |o: i64| {
        FrameRecord::map(NZTM, GridStep::Q0, [o, -o, o])
            .unwrap()
            .to_frame(m0)
    };
    let r = at(-COORD_MAX).unwrap();
    assert_eq!(
        r.value(),
        P3::new(COORD_MAX, -COORD_MAX, COORD_MAX).unwrap()
    );
    assert!(r.is_exact());
    assert_eq!(
        at(-(1 << 40)),
        Err(NumError::OutOfRange { value: 1i128 << 40 })
    );
    assert_eq!(
        at(1 << 40),
        Err(NumError::OutOfRange {
            value: -(1i128 << 40)
        })
    );
    let f = FrameRecord::map(NZTM, GridStep::Q0, [0, 0, 0]).unwrap();
    assert_eq!(
        f.to_frame(MapPoint {
            easting: f64::NAN,
            ..m0
        }),
        Err(NumError::NotFinite)
    );
    let out = |m: MapPoint| matches!(f.to_frame(m), Err(NumError::OutOfRange { .. }));
    assert!(out(MapPoint {
        northing: 1e300,
        ..m0
    }));
    assert!(out(MapPoint { height: -1e6, ..m0 }));
    // Tiny values round to the origin, inexactly.
    let r = f
        .to_frame(MapPoint {
            easting: 1e-310,
            northing: -1e-300,
            height: 0.0,
        })
        .unwrap();
    assert_eq!(r.value(), P3::new(0, 0, 0).unwrap());
    assert!(!r.is_exact());
    // Ground frames refuse out-of-range points too, at the limit exactly.
    let r34 = Rotation::direction(3, 4).unwrap();
    let g = FrameRecord::new(NZTM, GridStep::Q0, [0, 0, 0], r34, Scale::ONE).unwrap();
    let p = P3::new(COORD_MAX, -COORD_MAX, 0).unwrap();
    let m = g.to_map(p).unwrap();
    assert_eq!(g.to_frame(m.value()).unwrap().value(), p);
    assert_eq!(
        FrameRecord::map(NZTM, GridStep::Q0, [ORIGIN_MAX + 1, 0, 0]),
        Err(NumError::OutOfRange {
            value: i128::from(ORIGIN_MAX) + 1
        })
    );
}

#[test]
fn rotations_and_scales_are_canonical() {
    assert_eq!(
        Rotation::direction(6, 8).unwrap(),
        Rotation::direction(3, 4).unwrap()
    );
    assert_eq!(
        Rotation::direction(0, -5).unwrap(),
        Rotation::quarter_turns(3)
    );
    assert_eq!(Rotation::quarter_turns(-1), Rotation::quarter_turns(3));
    assert_eq!(Rotation::quarter_turns(1).quarter_turn_count(), Some(1));
    assert_eq!(Rotation::direction(3, 4).unwrap().exact_norm(), Some(5));
    assert_eq!(Rotation::direction(1, 2).unwrap().exact_norm(), None);
    assert_eq!(Rotation::direction(0, 0), Err(NumError::Degenerate));
    let m = Rotation::direction(i32::MIN, i32::MIN).unwrap();
    assert_eq!((m.cos(), m.sin()), (-1, -1));
    assert_eq!(Rotation::direction(i32::MIN, 1).unwrap().cos(), i32::MIN);
    let k = Scale::new(99_987_654, 100_000_000).unwrap();
    assert_eq!((k.num(), k.den()), (49_993_827, 50_000_000));
    assert!(Scale::new(7, 7).unwrap().is_one());
    assert_eq!(Scale::new(0, 1), Err(NumError::Degenerate));
    // Any positive ratio is a scale (chapter 17.1 sets no limit); only a zero part is refused.
    assert_eq!(Scale::new(3, 1).unwrap().num(), 3);
    assert_eq!(Scale::new(2, 6).unwrap(), Scale::new(1, 3).unwrap());
    assert_eq!(Scale::new(1, 0), Err(NumError::Degenerate));
    assert!(Scale::new(1, 2).is_ok() && Scale::new(2, 1).is_ok());
}

#[test]
fn lattice_examples_and_mismatches() {
    let s = steps_per_metre(GridStep::Q0) as i64;
    let a = FrameRecord::map(NZTM, GridStep::Q0, [1_700_000 * s, 5_900_000 * s, 0]).unwrap();
    // Neighbouring cells of a regional lattice: origins every 2^39 steps.
    let east_origin = [1_700_000 * s + (1 << 39), 5_900_000 * s, 0];
    let east = FrameRecord::map(NZTM, GridStep::Q0, east_origin).unwrap();
    let shift = a.shift_to(&east).unwrap();
    assert_eq!((shift.refine(), shift.offset()), (1, [-(1 << 39), 0, 0]));
    let p = P3::new((1 << 39) + 12_345, -678, 9).unwrap();
    let q = shift.apply(p).unwrap();
    assert_eq!(q, P3::new(12_345, -678, 9).unwrap());
    assert_eq!(east.shift_to(&a).unwrap().apply(q).unwrap(), p);
    // A map value that lands on steps converts to the shifted coordinates in both frames.
    let m = MapPoint {
        easting: 1_785_000.5,
        northing: 5_900_000.25,
        height: 1.0,
    };
    let (ra, rb) = (a.to_frame(m).unwrap(), east.to_frame(m).unwrap());
    assert!(ra.is_exact() && rb.is_exact());
    assert_eq!(shift.apply(ra.value()).unwrap(), rb.value());
    // Refining q0 to q1 multiplies by 25.
    let fine_origin = [1_700_000 * s * 25 + 3, 5_900_000 * s * 25, -7];
    let fine = FrameRecord::map(NZTM, GridStep::Q1, fine_origin).unwrap();
    let shift = a.shift_to(&fine).unwrap();
    assert_eq!((shift.refine(), shift.offset()), (25, [-3, 0, 7]));
    assert_eq!(
        shift.apply(P3::new(4, 0, 0).unwrap()).unwrap(),
        P3::new(97, 0, 7).unwrap()
    );
    let near = MapPoint {
        easting: 1_700_000.5,
        northing: 5_899_999.75,
        height: -0.25,
    };
    let (rn, rf) = (a.to_frame(near).unwrap(), fine.to_frame(near).unwrap());
    assert!(rn.is_exact() && rf.is_exact());
    assert_eq!(shift.apply(rn.value()).unwrap(), rf.value());
    // Mismatches.
    let other_crs = FrameRecord::map(CrsId::Epsg(2105), GridStep::Q0, a.origin()).unwrap();
    assert_eq!(
        a.shift_to(&other_crs),
        Err(MathError::FrameMismatch(FrameMismatch::Crs))
    );
    let local = FrameRecord::map(CrsId::Local(1), GridStep::Q0, a.origin()).unwrap();
    assert_eq!(
        a.shift_to(&local),
        Err(MathError::FrameMismatch(FrameMismatch::Crs))
    );
    let turned = FrameRecord::new(
        NZTM,
        GridStep::Q0,
        a.origin(),
        Rotation::quarter_turns(1),
        Scale::ONE,
    )
    .unwrap();
    assert_eq!(
        a.shift_to(&turned),
        Err(MathError::FrameMismatch(FrameMismatch::Rotation))
    );
    let k = Scale::new(9996, 10000).unwrap();
    let scaled = FrameRecord::new(NZTM, GridStep::Q0, a.origin(), Rotation::IDENTITY, k).unwrap();
    assert_eq!(
        a.shift_to(&scaled),
        Err(MathError::FrameMismatch(FrameMismatch::Scale))
    );
    assert_eq!(
        fine.shift_to(&a),
        Err(MathError::Num(NumError::WrongDirection))
    );
    // A rotated frame whose origin moves by a step that is not whole along its axes.
    let r34 = Rotation::direction(3, 4).unwrap();
    let g = FrameRecord::new(NZTM, GridStep::Q0, [0, 0, 0], r34, Scale::ONE).unwrap();
    let off = FrameRecord::new(NZTM, GridStep::Q0, [1, 0, 0], r34, Scale::ONE).unwrap();
    assert_eq!(
        g.shift_to(&off),
        Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice))
    );
    let on = FrameRecord::new(NZTM, GridStep::Q0, [3, 4, 0], r34, Scale::ONE).unwrap();
    // (3, 4) map steps is 5 steps along the frame's x axis, the direction (3, 4).
    assert_eq!(g.shift_to(&on).unwrap().offset(), [-5, 0, 0]);
    // An irrational direction shifts only vertically.
    let r12 = Rotation::direction(1, 2).unwrap();
    let i0 = FrameRecord::new(NZTM, GridStep::Q0, [0, 0, 0], r12, Scale::ONE).unwrap();
    let up = FrameRecord::new(NZTM, GridStep::Q0, [0, 0, 5], r12, Scale::ONE).unwrap();
    assert_eq!(i0.shift_to(&up).unwrap().offset(), [0, 0, -5]);
    let side = FrameRecord::new(NZTM, GridStep::Q0, [1, 2, 0], r12, Scale::ONE).unwrap();
    assert_eq!(
        i0.shift_to(&side),
        Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice))
    );
    // Out of range after the shift.
    let far_origin = [a.origin()[0] - (1 << 39), a.origin()[1], 0];
    let far = FrameRecord::map(NZTM, GridStep::Q0, far_origin).unwrap();
    let edge = P3::new(COORD_MAX - 10, 0, 0).unwrap();
    assert!(matches!(
        a.shift_to(&far).unwrap().apply(edge),
        Err(NumError::OutOfRange { .. })
    ));
}

#[test]
fn frame_kinds_and_vertical_crs() {
    let s = steps_per_metre(GridStep::Q0) as i64;
    let origin = [1_757_000 * s, 5_921_000 * s, 0];
    let map = FrameRecord::new(NZTM, GridStep::Q0, origin, Rotation::IDENTITY, Scale::ONE).unwrap();
    assert_eq!(map.kind(), FrameKind::Map);
    assert_eq!(map, FrameRecord::map(NZTM, GridStep::Q0, origin).unwrap());
    let k = Scale::new(99_987_654, 100_000_000).unwrap();
    let r = Rotation::direction(3, 4).unwrap();
    let ground = FrameRecord::new(NZTM, GridStep::Q0, origin, r, k).unwrap();
    assert_eq!(
        ground.kind(),
        FrameKind::Ground {
            rotation: r,
            scale: k
        }
    );
    assert_eq!((ground.rotation(), ground.scale()), (r, k));
    assert_eq!(
        (map.rotation(), map.scale()),
        (Rotation::IDENTITY, Scale::ONE)
    );
    // A scale alone or a rotation alone makes a Ground frame.
    let only_k = FrameRecord::new(NZTM, GridStep::Q0, origin, Rotation::IDENTITY, k).unwrap();
    assert!(matches!(only_k.kind(), FrameKind::Ground { .. }));
    // Heights in NZVD2016 (EPSG:7839) cannot be shifted into a frame without a vertical CRS, nor
    // into one with another vertical CRS, even when everything else matches.
    let nzvd = map.with_vertical_crs(Some(CrsId::Epsg(7839)));
    assert_eq!(nzvd.vertical_crs(), Some(CrsId::Epsg(7839)));
    assert_eq!(map.vertical_crs(), None);
    let mismatch = Err(MathError::FrameMismatch(FrameMismatch::VerticalCrs));
    assert_eq!(map.shift_to(&nzvd), mismatch);
    assert_eq!(nzvd.shift_to(&map), mismatch);
    let local = map.with_vertical_crs(Some(CrsId::Local(3)));
    assert_eq!(nzvd.shift_to(&local), mismatch);
    let same = FrameRecord::map(NZTM, GridStep::Q1, [origin[0] * 25, origin[1] * 25, 5])
        .unwrap()
        .with_vertical_crs(Some(CrsId::Epsg(7839)));
    assert_eq!(nzvd.shift_to(&same).unwrap().offset(), [0, 0, -5]);
    // The horizontal CRS is checked first.
    let other = nzvd.with_vertical_crs(None);
    let moved = FrameRecord::map(CrsId::Epsg(2105), GridStep::Q0, origin).unwrap();
    assert_eq!(
        other.shift_to(&moved),
        Err(MathError::FrameMismatch(FrameMismatch::Crs))
    );
}

/// Scale ratios far from one (chapter 17.1 allows any): exact conversions both ways.
#[test]
fn extreme_scales_convert_exactly() {
    for (kn, kd) in [(1, 1000), (4_000_000_007, 3), (1, u32::MAX), (u32::MAX, 2)] {
        let k = Scale::new(kn, kd).unwrap();
        for dir in [(3, 4), (1, 0), (1, 2)] {
            let r = Rotation::direction(dir.0, dir.1).unwrap();
            let f = FrameRecord::new(NZTM, GridStep::Q0, [12_345, -678, 9], r, k).unwrap();
            for p in [P3::new(1000, -2000, 3).unwrap(), P3::new(0, 1, 0).unwrap()] {
                let m = f.to_map(p).unwrap();
                if f.rotation().exact_norm().is_some() {
                    let want = exact_map(&f, p);
                    let got = [m.value().easting, m.value().northing, m.value().height];
                    for (g, w) in got.iter().zip(want.iter()) {
                        assert!(is_nearest_double(*g, w), "{f:?} {p:?}: {g} vs {w}");
                    }
                }
                // A scale below one maps a step to less than a step, so the map value's half-ulp
                // error stays below half a step and the round trip is the identity.
                if kn <= kd {
                    assert_eq!(f.to_frame(m.value()).unwrap().value(), p, "{f:?}");
                }
            }
        }
    }
}

#[test]
fn regional_lattice_cells() {
    let s = steps_per_metre(GridStep::Q0) as i64;
    let base = [1_600_000 * s, 5_000_000 * s, 0];
    let cell = |i: i64, j: i64| FrameRecord::lattice(NZTM, GridStep::Q0, base, [i, j]).unwrap();
    let (a, east, north) = (cell(0, 0), cell(1, 0), cell(0, -1));
    assert_eq!(a.kind(), FrameKind::Map);
    assert_eq!(a.origin(), base);
    assert_eq!(east.origin(), [base[0] + LATTICE_SPACING, base[1], 0]);
    assert_eq!(north.origin(), [base[0], base[1] - LATTICE_SPACING, 0]);
    assert_eq!(east.lattice_cell().unwrap().index(), [1, 0]);
    assert_eq!(a.lattice_cell().unwrap().base(), base);
    // Neighbours shift exactly.
    assert_eq!(
        a.shift_to(&east).unwrap().offset(),
        [-LATTICE_SPACING, 0, 0]
    );
    // Half-open ownership.
    let at = |x: i64, y: i64| P3::new(x, y, 0).unwrap();
    assert!(a.owns(at(-CELL_HALF, -CELL_HALF)));
    assert!(a.owns(at(CELL_HALF - 1, CELL_HALF - 1)));
    assert!(!a.owns(at(CELL_HALF, 0)));
    assert!(!a.owns(at(0, CELL_HALF)));
    assert!(!a.owns(at(-CELL_HALF - 1, 0)));
    // A plain frame owns its whole range.
    assert!(
        FrameRecord::map(NZTM, GridStep::Q0, base)
            .unwrap()
            .owns(at(COORD_MAX, -COORD_MAX))
    );
    // The home of a box is the cell holding its centre, with the upper boundary in the next cell.
    let bx = |x0: i64, x1: i64, y0: i64, y1: i64| Box3::new(at(x0, y0), at(x1, y1)).unwrap();
    let home = |b: Box3| a.home_frame(&b).unwrap().0.lattice_cell().unwrap().index();
    assert_eq!(home(bx(CELL_HALF - 1, CELL_HALF, 0, 0)), [0, 0]); // centre 2^38 − 1/2
    assert_eq!(home(bx(CELL_HALF, CELL_HALF, 0, 0)), [1, 0]); // centre exactly 2^38
    assert_eq!(home(bx(-CELL_HALF, -CELL_HALF, 0, 0)), [0, 0]);
    assert_eq!(home(bx(-CELL_HALF - 1, -CELL_HALF, 0, 0)), [-1, 0]); // centre −2^38 − 1/2
    assert_eq!(home(bx(0, 0, COORD_MAX - 5, COORD_MAX)), [0, 2]);
    let (h, shift) = a.home_frame(&bx(CELL_HALF, CELL_HALF + 10, 3, 4)).unwrap();
    assert_eq!(h, east);
    assert_eq!(
        shift.apply(at(CELL_HALF, 3)).unwrap(),
        at(CELL_HALF - LATTICE_SPACING, 3)
    );
    // Operands in different cells combine in lattice coordinates. These span 3·2^39 − 1 steps,
    // centred in cell 0, and fit it.
    let wide = [
        (a, bx(-(LATTICE_SPACING + CELL_HALF) + 1, 0, 0, 0)),
        (east, bx(0, CELL_HALF - 1, 0, 0)),
    ];
    assert_eq!(FrameRecord::home_of(&wide).unwrap(), a);
    let too_wide = [(a, bx(-COORD_MAX, 0, 0, 0)), (east, bx(0, COORD_MAX, 0, 0))];
    assert!(matches!(
        FrameRecord::home_of(&too_wide),
        Err(MathError::FrameRangeExceeded { .. })
    ));
    // Mismatched operands are refused.
    let origin_box = bx(0, 0, 0, 0);
    let plain = FrameRecord::map(NZTM, GridStep::Q0, base).unwrap();
    let not_lattice = Err(MathError::FrameMismatch(FrameMismatch::NotOnLattice));
    assert_eq!(FrameRecord::home_of(&[(plain, origin_box)]), not_lattice);
    let other_base = FrameRecord::lattice(NZTM, GridStep::Q0, [0, 0, 0], [0, 0]).unwrap();
    assert_eq!(
        FrameRecord::home_of(&[(a, origin_box), (other_base, origin_box)]),
        not_lattice
    );
    let fine = FrameRecord::lattice(NZTM, GridStep::Q1, base, [0, 0]).unwrap();
    assert_eq!(
        FrameRecord::home_of(&[(a, origin_box), (fine, origin_box)]),
        not_lattice
    );
    let vertical = a.with_vertical_crs(Some(CrsId::Epsg(7839)));
    assert_eq!(
        FrameRecord::home_of(&[(a, origin_box), (vertical, origin_box)]),
        Err(MathError::FrameMismatch(FrameMismatch::VerticalCrs))
    );
    assert_eq!(
        FrameRecord::home_of(&[]),
        Err(MathError::Num(NumError::Degenerate))
    );
    // Origins beyond 2^60 are refused.
    assert!(FrameRecord::lattice(NZTM, GridStep::Q0, base, [1 << 22, 0]).is_err());
    assert!(FrameRecord::lattice(NZTM, GridStep::Q0, base, [i64::MAX, 0]).is_err());
}
