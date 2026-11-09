//! Piece G273's attack tests: the two questions about a four-plane event that complete
//! `Moving::cmp_meeting` (chapter 13.11). `Moving::cmp_meeting_time` says whether the meeting of
//! four moving planes comes before a given time, and `Moving::side_at_meeting` says which side of
//! a fifth moving plane that meeting lies on. Both are checked against exact rationals from
//! `num-bigint` and `num-rational`, solved by Gaussian elimination (not by the determinants the
//! predicates use), on small cases, on random planes through grid points, and at the extremes of
//! the budget: planes through corners of the frame, every coordinate at ±(2^40 − 1), every speed
//! at the offset's limit, 2^125 − 1, and times at ±(2^40 − 1).
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "tests: a failure stops the test, native arithmetic, and the names are the formulas'"
)]

use core::cmp::Ordering;

use geom_num::grid::{Coord, P3};
use geom_num::plane::{Moving, Plane};
use geom_num::{NumError, Sign};
use num_bigint::BigInt;
use num_rational::BigRational;

/// The largest grid coordinate, 2^40 − 1.
const M: i64 = (1 << 40) - 1;

/// The largest speed a moving plane may have, 2^125 − 1.
const LIMIT: i128 = (1 << 125) - 1;

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn rat(v: i128) -> BigRational {
    BigRational::from_integer(BigInt::from(v))
}

fn coord(v: i64) -> Coord {
    Coord::new(v).unwrap()
}

/// Where and when the four planes meet, (x, y, z, t), by Gaussian elimination on
/// n · x + t s = −d, exactly; `None` when the system is singular, which is when ns = 0.
fn meeting(m: &[Moving; 4]) -> Option<[BigRational; 4]> {
    let mut rows: Vec<[BigRational; 5]> = m
        .iter()
        .map(|q| {
            let n = q.plane().n();
            [
                rat(n[0]),
                rat(n[1]),
                rat(n[2]),
                rat(q.speed()),
                rat(-q.plane().d()),
            ]
        })
        .collect();
    for col in 0..4 {
        let pivot = (col..4).find(|&r| rows[r][col] != rat(0))?;
        rows.swap(col, pivot);
        for r in 0..4 {
            if r != col {
                let factor = &rows[r][col] / &rows[col][col];
                let source = rows[col].clone();
                for (k, cell) in rows[r].iter_mut().enumerate() {
                    *cell = &*cell - &factor * &source[k];
                }
            }
        }
    }
    Some(core::array::from_fn(|k| &rows[k][4] / &rows[k][k]))
}

/// The fifth plane's distance n · x + d + t s at the meeting of the four.
fn distance_at(four: &[Moving; 4], other: &Moving) -> Option<BigRational> {
    let [x, y, z, t] = meeting(four)?;
    let n = other.plane().n();
    Some(
        &x * rat(n[0])
            + &y * rat(n[1])
            + &z * rat(n[2])
            + rat(other.plane().d())
            + &t * rat(other.speed()),
    )
}

fn sign_of(v: &BigRational) -> Sign {
    match v.cmp(&rat(0)) {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// The three planes x = 0, y = 0 and z = h.
fn axes_at(h: i64) -> [Plane; 3] {
    [
        Plane::through(p(0, 0, 0), p(0, 1, 0), p(0, 0, 1)).unwrap(),
        Plane::through(p(0, 0, 0), p(0, 0, 1), p(1, 0, 0)).unwrap(),
        Plane::through(p(0, 0, h), p(1, 0, h), p(0, 1, h)).unwrap(),
    ]
}

/// The plane through three corners of the frame, each given by the signs of its coordinates:
/// normals near 2^82 and offsets near 2^122, the reach of planes through grid points.
fn through_corners(corners: [[i64; 3]; 3]) -> Plane {
    let [a, b, c] = corners.map(|[x, y, z]| p(x * M, y * M, z * M));
    Plane::through(a, b, c).unwrap()
}

/// The planes x = 0, y = 0 and z = 0 standing still, and the plane through (10 k, 0, 0),
/// (0, 10 k, 0) and (0, 0, 10 k) moving at `speed`. That plane's normal is a multiple of
/// (1, 1, 1), so the four meet at the origin at a time proportional to k and to 1 / speed.
fn corner_and(k: i64, speed: i128) -> [Moving; 4] {
    let still = axes_at(0).map(|q| Moving::at_speed(q, 0).unwrap());
    let slant = Plane::through(p(10 * k, 0, 0), p(0, 10 * k, 0), p(0, 0, 10 * k)).unwrap();
    [
        still[0],
        still[1],
        still[2],
        Moving::at_speed(slant, speed).unwrap(),
    ]
}

#[test]
fn a_meeting_is_compared_with_a_time() {
    // The oracle says when they meet; the predicate must agree on either side of it and at it.
    for (k, speed) in [(1, 100), (1, -100), (3, 7), (1, 200)] {
        let four = corner_and(k, speed);
        let te = meeting(&four).unwrap()[3].clone();
        let around = te.to_integer();
        let around: i64 = i64::try_from(around.clone()).unwrap_or(0);
        for t in [around - 2, around - 1, around, around + 1, around + 2, 0] {
            let want = te.cmp(&rat(i128::from(t)));
            assert_eq!(
                Moving::cmp_meeting_time(&four, coord(t)),
                Ok(want),
                "k = {k}, speed = {speed}, t = {t}"
            );
        }
    }
}

#[test]
fn a_time_equal_to_the_meeting_is_a_tie() {
    // Speed 100 and the plane x + y + z = 10 k: the planes meet at the origin when
    // 100 · 10 k = t · 100, at t = 10 k^3 (its normal grows as k^2 and its offset as k^3).
    for k in [1_i64, 7, 1_000, 4_000, -5] {
        let four = corner_and(k, 100);
        let t = 10 * k * k * k;
        assert_eq!(meeting(&four).unwrap()[3], rat(i128::from(t)));
        assert_eq!(
            Moving::cmp_meeting_time(&four, coord(t)),
            Ok(Ordering::Equal)
        );
        assert_eq!(
            Moving::cmp_meeting_time(&four, coord(t - 1)),
            Ok(Ordering::Greater)
        );
        assert_eq!(
            Moving::cmp_meeting_time(&four, coord(t + 1)),
            Ok(Ordering::Less)
        );
    }
}

#[test]
fn a_meeting_agrees_with_the_order_of_meetings() {
    let early = corner_and(1, -200);
    let late = corner_and(1, -100);
    let (te, tl) = (
        meeting(&early).unwrap()[3].clone(),
        meeting(&late).unwrap()[3].clone(),
    );
    assert!(
        te > tl,
        "the faster plane meets the corner at a time nearer zero"
    );
    let cut = coord(i64::try_from((&te + &tl).to_integer() / 2).unwrap());
    // `Moving::cmp_meeting` and the comparison with a time between them agree on both.
    let order = Moving::cmp_meeting(&early, &late).unwrap();
    assert_eq!(order, te.cmp(&tl));
    assert_eq!(
        Moving::cmp_meeting_time(&early, cut),
        Ok(te.cmp(&rat(i128::from(cut.get()))))
    );
    assert_eq!(
        Moving::cmp_meeting_time(&late, cut),
        Ok(tl.cmp(&rat(i128::from(cut.get()))))
    );
}

#[test]
fn the_side_of_a_fifth_plane_at_the_meeting() {
    let four = corner_and(1, 100);
    let origin_time = meeting(&four).unwrap()[3].clone();
    assert_eq!(origin_time, rat(10));
    // The plane z = 0 moving at speed 1 has moved t = 10 from the origin: its distance there is
    // d + t s = +10, and at speed −1 it is −10. At speed 0 it passes through the meeting.
    let z = axes_at(0)[2];
    let still = Moving::at_speed(z, 0).unwrap();
    assert_eq!(Moving::side_at_meeting(&four, &still), Ok(Sign::Zero));
    for speed in [1, -1, 5, -5] {
        let other = Moving::at_speed(z, speed).unwrap();
        let want = sign_of(&distance_at(&four, &other).unwrap());
        assert_ne!(want, Sign::Zero);
        assert_eq!(Moving::side_at_meeting(&four, &other), Ok(want));
    }
    let up = Moving::at_speed(z, 1).unwrap();
    let down = Moving::at_speed(z, -1).unwrap();
    assert_eq!(
        Moving::side_at_meeting(&four, &up),
        Ok(Moving::side_at_meeting(&four, &down).unwrap().negate())
    );
    // A plane that does not pass through the origin, z = 4, standing still.
    let high = Moving::at_speed(axes_at(4)[2], 0).unwrap();
    let want = sign_of(&distance_at(&four, &high).unwrap());
    assert_ne!(want, Sign::Zero);
    assert_eq!(Moving::side_at_meeting(&four, &high), Ok(want));
}

#[test]
fn planes_that_never_meet_at_one_time_are_degenerate() {
    let never = corner_and(1, 0);
    assert!(meeting(&never).is_none());
    assert_eq!(
        Moving::cmp_meeting_time(&never, coord(3)),
        Err(NumError::Degenerate)
    );
    assert_eq!(
        Moving::side_at_meeting(&never, &corner_and(1, 100)[3]),
        Err(NumError::Degenerate)
    );
}

/// Four planes through corners of the frame at the offset's limit, and three more to ask about.
fn extreme_planes() -> ([Moving; 4], [Moving; 3]) {
    let corner = [-1, -1, -1];
    let planes = [
        through_corners([corner, [-1, -1, 1], [-1, 1, -1]]),
        through_corners([corner, [-1, -1, 1], [1, -1, -1]]),
        through_corners([corner, [-1, 1, -1], [1, -1, -1]]),
        through_corners([[-1, -1, 1], [-1, 1, -1], [1, -1, -1]]),
        through_corners([[-1, -1, 1], [-1, 1, -1], [1, -1, 1]]),
        through_corners([[-1, 1, 1], [1, -1, 1], [1, 1, -1]]),
    ];
    let four = [
        Moving::at_speed(planes[0], LIMIT).unwrap(),
        Moving::at_speed(planes[1], LIMIT).unwrap(),
        Moving::at_speed(planes[2], LIMIT).unwrap(),
        Moving::at_speed(planes[3], -LIMIT).unwrap(),
    ];
    let others = [
        Moving::at_speed(planes[4], LIMIT).unwrap(),
        Moving::at_speed(planes[5], -LIMIT).unwrap(),
        // One of the four, again: its distance at their meeting is exactly zero, a tie of the
        // widest values for the exact path.
        four[3],
    ];
    (four, others)
}

#[test]
fn events_at_the_extremes_are_exact() {
    let (four, others) = extreme_planes();
    let te = meeting(&four).unwrap()[3].clone();
    for other in &others {
        let want = sign_of(&distance_at(&four, other).unwrap());
        assert_eq!(Moving::side_at_meeting(&four, other), Ok(want));
    }
    assert_eq!(Moving::side_at_meeting(&four, &others[2]), Ok(Sign::Zero));
    assert_eq!(Moving::side_at_meeting(&four, &four[0]), Ok(Sign::Zero));
    for t in [-M, -3, -1, 0, 1, 3, M] {
        let want = te.cmp(&rat(i128::from(t)));
        assert_eq!(
            Moving::cmp_meeting_time(&four, coord(t)),
            Ok(want),
            "t = {t}"
        );
    }
}

/// The default seed of every randomised test here.
const DEFAULT_SEED: u64 = 0x6273_0000_5eed_0273;

/// `SplitMix64` (Steele, Lea and Flood, *Fast splittable pseudorandom number generators*,
/// OOPSLA 2014), as `generated_predicates.rs` writes it, because `geom-num` cannot depend on
/// `geom-test`.
struct Rng(u64);

impl Rng {
    /// The test's generator: the seed from `SPAREKIT_SEED` (hex) or the default, printed.
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in lo..=hi.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (hi - lo) as u64 + 1;
        lo + (self.next() % span) as i64
    }

    /// A coordinate: anywhere on the grid, at one of its edges, or small.
    fn coord(&mut self) -> i64 {
        match self.next() % 6 {
            0 => M,
            1 => -M,
            2 => self.range(-8, 8),
            _ => self.range(-M, M),
        }
    }

    /// A plane through three grid points, or `None` where they are collinear.
    fn plane(&mut self) -> Option<Plane> {
        let mut point = || p(self.coord(), self.coord(), self.coord());
        let (a, b, c) = (point(), point(), point());
        Plane::through(a, b, c).ok()
    }

    /// A speed: anywhere in the budget, at its edges, or small.
    fn speed(&mut self) -> i128 {
        match self.next() % 5 {
            0 => LIMIT,
            1 => -LIMIT,
            2 => i128::from(self.range(-8, 8)),
            _ => (i128::from(self.next() as i64) << 61) % LIMIT,
        }
    }
}

#[test]
fn times_are_the_exact_order() {
    let mut rng = Rng::for_test("times_are_the_exact_order");
    let mut checked = 0;
    while checked < 1_000 {
        let planes: [Option<Plane>; 4] = core::array::from_fn(|_| rng.plane());
        let Some(planes) = planes.into_iter().collect::<Option<Vec<Plane>>>() else {
            continue;
        };
        let four: [Moving; 4] =
            core::array::from_fn(|k| Moving::at_speed(planes[k], rng.speed()).unwrap());
        let time = rng.coord();
        if let Some([_, _, _, t]) = meeting(&four) {
            let want = t.cmp(&rat(i128::from(time)));
            assert_eq!(Moving::cmp_meeting_time(&four, coord(time)), Ok(want));
            checked += 1;
        } else {
            assert_eq!(
                Moving::cmp_meeting_time(&four, coord(time)),
                Err(NumError::Degenerate)
            );
        }
    }
}

#[test]
fn sides_are_the_exact_signs() {
    let mut rng = Rng::for_test("sides_are_the_exact_signs");
    let mut checked = 0;
    while checked < 1_000 {
        let planes: [Option<Plane>; 5] = core::array::from_fn(|_| rng.plane());
        let Some(planes) = planes.into_iter().collect::<Option<Vec<Plane>>>() else {
            continue;
        };
        let moving: Vec<Moving> = planes
            .iter()
            .map(|q| Moving::at_speed(*q, rng.speed()).unwrap())
            .collect();
        let four: [Moving; 4] = core::array::from_fn(|k| moving[k]);
        if let Some(distance) = distance_at(&four, &moving[4]) {
            assert_eq!(
                Moving::side_at_meeting(&four, &moving[4]),
                Ok(sign_of(&distance))
            );
            // One of the four is on the point; the four in another order give the same answer.
            assert_eq!(Moving::side_at_meeting(&four, &four[2]), Ok(Sign::Zero));
            let again = [four[1], four[0], four[2], four[3]];
            assert_eq!(
                Moving::side_at_meeting(&again, &moving[4]),
                Ok(sign_of(&distance))
            );
            checked += 1;
        } else {
            assert_eq!(
                Moving::side_at_meeting(&four, &moving[4]),
                Err(NumError::Degenerate)
            );
        }
    }
}

/// The twenty-five numbers `generated::mitre_side` takes for four planes and a fifth: each plane's
/// normal, offset and speed.
fn side_args(four: &[Moving; 4], other: &Moving) -> [i128; 25] {
    let mut out = [0; 25];
    for (k, m) in four.iter().chain([other]).enumerate() {
        let [x, y, z] = m.plane().n();
        for (slot, v) in out
            .iter_mut()
            .skip(5 * k)
            .zip([x, y, z, m.plane().d(), m.speed()])
        {
            *slot = v;
        }
    }
    out
}

/// What the generated filter says about the sign of the 5 x 5 determinant: `None` when it cannot
/// tell.
fn side_filter(four: &[Moving; 4], other: &Moving) -> Option<Sign> {
    let [
        n1x,
        n1y,
        n1z,
        d1,
        s1,
        n2x,
        n2y,
        n2z,
        d2,
        s2,
        n3x,
        n3y,
        n3z,
        d3,
        s3,
        n4x,
        n4y,
        n4z,
        d4,
        s4,
        n5x,
        n5y,
        n5z,
        d5,
        s5,
    ] = side_args(four, other);
    geom_num::generated::mitre_side::filter(
        n1x, n1y, n1z, d1, s1, n2x, n2y, n2z, d2, s2, n3x, n3y, n3z, d3, s3, n4x, n4y, n4z, d4, s4,
        n5x, n5y, n5z, d5, s5,
    )
}

#[test]
fn one_step_from_degenerate_is_decided_exactly() {
    // The fifth plane is one of the four with its speed one step off: at their meeting it is
    // t steps away, the smallest distance the data can give.
    let mut rng = Rng::for_test("one_step_from_degenerate_is_decided_exactly");
    let mut checked = 0;
    while checked < 500 {
        let planes: [Option<Plane>; 4] = core::array::from_fn(|_| rng.plane());
        let Some(planes) = planes.into_iter().collect::<Option<Vec<Plane>>>() else {
            continue;
        };
        let four: [Moving; 4] =
            core::array::from_fn(|k| Moving::at_speed(planes[k], rng.speed()).unwrap());
        if meeting(&four).is_none() {
            continue;
        }
        for k in 0..4 {
            for step in [-1, 1] {
                let speed = four[k].speed() + step;
                let Ok(other) = Moving::at_speed(four[k].plane(), speed) else {
                    continue;
                };
                let want = sign_of(&distance_at(&four, &other).unwrap());
                assert_ne!(want, Sign::Zero, "one step moves the plane off the meeting");
                assert_eq!(Moving::side_at_meeting(&four, &other), Ok(want));
            }
        }
        checked += 1;
    }
}

#[test]
fn the_filter_boundary_is_crossed_both_ways() {
    // The fifth plane is the fourth with its speed moved by 2^k, so the determinant is nd · 2^k
    // and grows from tiny to huge as k does. The filter must decline the small ones, decide the
    // large ones, and `side_at_meeting` must match the oracle on both.
    let (four, _) = extreme_planes();
    let (mut declined, mut decided) = (0, 0);
    for k in 0..=123 {
        for sign in [-1_i128, 1] {
            let speed = four[3].speed() + sign * (1_i128 << k);
            let Ok(other) = Moving::at_speed(four[3].plane(), speed) else {
                continue;
            };
            let want = sign_of(&distance_at(&four, &other).unwrap());
            assert_eq!(Moving::side_at_meeting(&four, &other), Ok(want), "k = {k}");
            match side_filter(&four, &other) {
                None => declined += 1,
                Some(filtered) => {
                    decided += 1;
                    // The filter's sign is the determinant's, which is `want` times ns's sign.
                    let ns = sign_of(&determinant_ns(&four));
                    assert_eq!(filtered.mul(ns), want, "k = {k}");
                }
            }
        }
    }
    assert!(
        declined > 0 && decided > 0,
        "declined {declined}, decided {decided}"
    );
}

/// The determinant of the rows (n, s) of the four planes, by Gaussian elimination in rationals.
fn determinant_ns(four: &[Moving; 4]) -> BigRational {
    let mut m: Vec<Vec<BigRational>> = four
        .iter()
        .map(|q| {
            let n = q.plane().n();
            vec![rat(n[0]), rat(n[1]), rat(n[2]), rat(q.speed())]
        })
        .collect();
    let mut det = rat(1);
    for col in 0..4 {
        let pivot = (col..4).find(|&r| m[r][col] != rat(0)).unwrap();
        if pivot != col {
            m.swap(col, pivot);
            det = -det;
        }
        det *= m[col][col].clone();
        for r in col + 1..4 {
            let f = &m[r][col] / &m[col][col];
            let source = m[col].clone();
            for (k, cell) in m[r].iter_mut().enumerate() {
                *cell = &*cell - &f * &source[k];
            }
        }
    }
    det
}

#[test]
fn the_side_obeys_its_laws() {
    // Reordering the four planes keeps the answer; reversing the fifth plane (its points the other
    // way round, its speed negated) flips it.
    let mut rng = Rng::for_test("the_side_obeys_its_laws");
    let mut checked = 0;
    while checked < 300 {
        let pts: [P3; 3] = core::array::from_fn(|_| p(rng.coord(), rng.coord(), rng.coord()));
        let planes: [Option<Plane>; 4] = core::array::from_fn(|_| rng.plane());
        let Some(planes) = planes.into_iter().collect::<Option<Vec<Plane>>>() else {
            continue;
        };
        let (Ok(forward), Ok(backward)) = (
            Plane::through(pts[0], pts[1], pts[2]),
            Plane::through(pts[0], pts[2], pts[1]),
        ) else {
            continue;
        };
        let four: [Moving; 4] =
            core::array::from_fn(|k| Moving::at_speed(planes[k], rng.speed()).unwrap());
        let speed = rng.speed();
        let (a, b) = (
            Moving::at_speed(forward, speed).unwrap(),
            Moving::at_speed(backward, -speed).unwrap(),
        );
        let Ok(side) = Moving::side_at_meeting(&four, &a) else {
            continue;
        };
        assert_eq!(Moving::side_at_meeting(&four, &b), Ok(side.negate()));
        for order in [[1, 0, 2, 3], [3, 2, 1, 0], [2, 3, 0, 1], [0, 2, 1, 3]] {
            let permuted = order.map(|k| four[k]);
            assert_eq!(Moving::side_at_meeting(&permuted, &a), Ok(side));
        }
        checked += 1;
    }
}
