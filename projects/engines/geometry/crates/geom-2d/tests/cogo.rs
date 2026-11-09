//! Core COGO (`geom_2d::cogo`, piece G359, 17.2) against integer arithmetic done here.
//!
//! - **The nearest root** is the integer k with (2k − 1)² ≤ 4n < (2k + 1)², checked for every n
//!   up to 2^20, about every perfect square to 2^64, and at the end of `u128`.
//! - **Forward** is exact on the quarter turns; at 30°, 45° and 60° in every quadrant, whose sines
//!   and cosines are ½, √½ and √¾, the committed coordinate is decided here in integers (the
//!   nearest whole k to d·√r is the k with (2k − 1)² ≤ 4rd² < (2k + 1)²); its bound is 1 and it is
//!   certain except next to a half step; leaving the frame or the longest distance is refused.
//! - **Inverse** gives the exact squared distance and its nearest root, the exact vector, the
//!   bearing of the vector, and no bearing for one point; and forward from a point along the
//!   inverse's bearing and distance lands within a step of the other point, for distances up to
//!   2^26 steps.
//! - **Misclosure** encloses the exact misclosure of traverses on the quarter turns, which is an
//!   integer vector here, and of random closed traverses built from inverses, which is within the
//!   bound of the legs' roundings; the meter charges one unit per leg and stops cleanly.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![allow(
    clippy::many_single_char_names,
    reason = "points, bearings and distances are p, b and d as in the module"
)]

use geom_2d::cogo::{CogoError, LONGEST, Leg, forward, inverse, misclosure, nearest_root};
use geom_num::angle::{AngleUnits, Bearing, FULL_TURN, PER_DEGREE, QUARTER_TURN};
use geom_num::grid::COORD_MAX;
use geom_num::{Exactness, GridStep, NumError, P2, Steps};
use geom_run::{Budget, WorkMeter};

/// The default seed of every randomised test here.
const DEFAULT_SEED: u64 = 0x0359_c060_5eed_0003;

/// `SplitMix64`, as the kernel's other tests write it.
struct Rng(u64);

impl Rng {
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

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo) as u64 + 1)) as i64
    }
}

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Whether k is the nearest integer to √n: (2k − 1)² ≤ 4n < (2k + 1)², in 256-bit arithmetic made
/// of two u128 halves where needed. Here n is below 2^126, so every term fits u128.
fn is_nearest_root(n: u128, k: u128) -> bool {
    let four_n = 4 * n;
    let below = if k == 0 { 0 } else { (2 * k - 1) * (2 * k - 1) };
    below <= four_n && four_n < (2 * k + 1) * (2 * k + 1)
}

#[test]
fn the_nearest_root_is_the_nearest_integer_to_the_square_root() {
    for n in 0..=(1u128 << 20) {
        let r = nearest_root(n);
        assert!(is_nearest_root(n, r.value()), "{n}");
        assert_eq!(r.is_exact(), r.value() * r.value() == n, "{n}");
    }
    for k in [
        1u128 << 20,
        (1 << 32) - 1,
        1 << 32,
        (1 << 41) + 3,
        (1 << 62) - 1,
    ] {
        for n in [k * k - 1, k * k, k * k + 1, k * k + k, k * k + k + 1] {
            assert!(is_nearest_root(n, nearest_root(n).value()), "{n}");
        }
        assert!(nearest_root(k * k).is_exact());
        // k² + k is nearer k, k² + k + 1 nearer k + 1.
        assert_eq!(nearest_root(k * k + k).value(), k);
        assert_eq!(nearest_root(k * k + k + 1).value(), k + 1);
    }
    // At the end of u128, where 4n and (2k + 1)² would not fit: √(2^128 − 1) is just below 2^64.
    let top = nearest_root(u128::MAX);
    assert_eq!(top.value(), 1u128 << 64);
    assert!(!top.is_exact());
    let k = u128::from(u64::MAX);
    assert_eq!(nearest_root(k * k).value(), k);
    assert!(nearest_root(k * k).is_exact());
    assert_eq!(nearest_root(k * k + k).value(), k);
    assert_eq!(nearest_root(k * k + k + 1).value(), k + 1);
}

#[test]
fn forward_on_the_quarter_turns_is_exact() {
    let from = p(1000, -2000);
    for (k, dx, dy) in [(0, 0, 1), (1, 1, 0), (2, 0, -1), (3, -1, 0)] {
        for turns in [0, 1, -1] {
            let b = Bearing::new(AngleUnits(k * QUARTER_TURN + turns * FULL_TURN));
            let f = forward(from, b, Steps(777)).unwrap();
            assert_eq!(f.point(), p(1000 + 777 * dx, -2000 + 777 * dy));
            assert!(f.is_exact() && f.is_certain());
            assert_eq!(f.bound(), Steps(0));
            assert_eq!(f.exactness(GridStep::Q0), Exactness::Exact);
        }
    }
    assert_eq!(
        forward(
            p(COORD_MAX, 0),
            Bearing::new(AngleUnits(QUARTER_TURN)),
            Steps(1)
        ),
        Err(CogoError::Num(NumError::OutOfRange {
            value: i128::from(COORD_MAX) + 1
        }))
    );
    assert_eq!(
        forward(p(0, 0), Bearing::NORTH, Steps(LONGEST.0 + 1)),
        Err(CogoError::TooLong)
    );
}

/// The nearest integer to `s · d · √r_num/√r_den` (r a ratio of small integers), with s the sign,
/// decided in integers: k ≥ 0 is the nearest to d√q exactly when (2k − 1)² ≤ 4qd² < (2k + 1)².
fn nearest_scaled_root(d: i64, r_num: i128, r_den: i128) -> i64 {
    let target = 4 * i128::from(d) * i128::from(d) * r_num;
    // 4 q d² as a fraction over r_den: find k with (2k − 1)² r_den ≤ target < (2k + 1)² r_den.
    let guess = ((d as f64) * ((r_num as f64) / (r_den as f64)).sqrt()).round() as i128;
    for k in (guess - 2).max(0)..=guess + 2 {
        let below = if k == 0 {
            0
        } else {
            (2 * k - 1) * (2 * k - 1) * r_den
        };
        let above = (2 * k + 1) * (2 * k + 1) * r_den;
        if below <= target && target < above {
            return k as i64;
        }
    }
    panic!("no nearest root near {guess}")
}

#[test]
fn forward_at_thirty_forty_five_and_sixty_degrees_lands_on_the_nearest_grid_point() {
    let mut rng = Rng::for_test("forward_at_thirty_forty_five_and_sixty_degrees");
    let from = p(-123_456, 654_321);
    for i in 0..3_000 {
        let d = match i % 3 {
            0 => rng.range(1, 1000),
            1 => rng.range(1, 1 << 30),
            _ => rng.range(1, 1 << 39),
        };
        for quadrant in 0..4i64 {
            for (degrees, (sin_num, sin_den), (cos_num, cos_den)) in [
                (30, (1, 4), (3, 4)),
                (45, (1, 2), (1, 2)),
                (60, (3, 4), (1, 4)),
            ] {
                // sin² and cos² of the angle within its quadrant.
                let b = Bearing::new(AngleUnits(quadrant * QUARTER_TURN + degrees * PER_DEGREE));
                let f = forward(from, b, Steps(d as u64)).unwrap();
                // Rotating by quarter turns: east, north = (sin, cos) of the angle in its quadrant,
                // turned q times: (s, c) → (c, −s).
                let s = nearest_scaled_root(d, sin_num, sin_den);
                let c = nearest_scaled_root(d, cos_num, cos_den);
                let (east, north) = match quadrant {
                    0 => (s, c),
                    1 => (c, -s),
                    2 => (-s, -c),
                    _ => (-c, s),
                };
                let want = p(from.x.get() + east, from.y.get() + north);
                assert_eq!(f.bound(), Steps(1), "{d} {degrees} {quadrant}");
                assert_eq!(
                    f.exactness(GridStep::Q1),
                    Exactness::Certified {
                        bound: Steps(1),
                        step: GridStep::Q1
                    }
                );
                if f.is_certain() {
                    assert_eq!(f.point(), want, "{d} at {degrees} in quadrant {quadrant}");
                } else {
                    // Next to a half step either neighbour may be the nearest.
                    assert!((f.point().x.get() - want.x.get()).abs() <= 1);
                    assert!((f.point().y.get() - want.y.get()).abs() <= 1);
                }
            }
        }
    }
    // Thirty degrees on an even distance: d/2 east is exact, and the point is certain.
    let f = forward(
        p(0, 0),
        Bearing::new(AngleUnits(30 * PER_DEGREE)),
        Steps(1000),
    )
    .unwrap();
    assert!(f.is_certain());
    assert_eq!(f.point().x.get(), 500);
}

#[test]
fn inverse_is_exact_where_it_can_be_and_forward_undoes_it() {
    let i = inverse(p(0, 0), p(3, 4)).unwrap();
    assert_eq!(i.delta(), [3, 4]);
    assert_eq!(i.squared(), 25);
    assert_eq!(i.distance().value(), 5);
    assert!(i.distance().is_exact());
    let same = inverse(p(9, 9), p(9, 9)).unwrap();
    assert_eq!((same.squared(), same.bearing()), (0, None));
    let diagonal = inverse(p(-5, -5), p(5, 5)).unwrap();
    let b = diagonal.bearing().unwrap();
    assert!(b.is_exact());
    assert_eq!(b.value().units(), AngleUnits(45 * PER_DEGREE));
    assert!(!diagonal.distance().is_exact());
    assert_eq!(diagonal.distance().value(), 14);
    // Corner to corner of the frame: the longest a frame holds.
    let far = inverse(p(-COORD_MAX, -COORD_MAX), p(COORD_MAX, COORD_MAX)).unwrap();
    assert_eq!(far.squared(), 2 * (2 * COORD_MAX as u128).pow(2));
    let mut rng = Rng::for_test("inverse_is_exact_where_it_can_be_and_forward_undoes_it");
    for _ in 0..20_000 {
        let reach = 1 << 25;
        let a = p(rng.range(-reach, reach), rng.range(-reach, reach));
        let z = p(rng.range(-reach, reach), rng.range(-reach, reach));
        let inv = inverse(a, z).unwrap();
        let (dx, dy) = (z.x.get() - a.x.get(), z.y.get() - a.y.get());
        assert_eq!(inv.delta(), [dx, dy]);
        let n = (i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy)) as u128;
        assert_eq!(inv.squared(), n);
        assert!(is_nearest_root(n, inv.distance().value()));
        let Some(bearing) = inv.bearing() else {
            continue;
        };
        let back = forward(a, bearing.value(), Steps(inv.distance().value() as u64)).unwrap();
        // The bearing is within a unit (under 2·10^−9 rad) and the distance within half a step,
        // so at under 2^26 steps the point is within a step of the other on each axis.
        assert!((back.point().x.get() - z.x.get()).abs() <= 1, "{a:?} {z:?}");
        assert!((back.point().y.get() - z.y.get()).abs() <= 1, "{a:?} {z:?}");
    }
}

#[test]
fn the_misclosure_of_quarter_turn_traverses_holds_the_exact_vector() {
    let leg = |k: i64, d: u64| Leg {
        bearing: Bearing::new(AngleUnits(k * QUARTER_TURN)),
        distance: Steps(d),
    };
    // A rectangle that closes, and one whose last side is a step short.
    let closed = [leg(0, 100), leg(1, 200), leg(2, 100), leg(3, 200)];
    let m = misclosure(p(7, 7), &closed, p(7, 7), &mut meter()).unwrap();
    assert!(m.east().contains(0.0) && m.north().contains(0.0));
    assert!(m.length().contains(0.0));
    assert!(m.east().hi() - m.east().lo() < 1e-9);
    assert_eq!(m.travelled(), 600);
    let short = [leg(0, 100), leg(1, 200), leg(2, 100), leg(3, 199)];
    let m = misclosure(p(7, 7), &short, p(7, 7), &mut meter()).unwrap();
    assert!(m.east().contains(1.0) && m.north().contains(0.0));
    assert!(m.length().contains(1.0));
    // An open traverse to a known end three steps north of where it arrives.
    let open = [leg(1, 1000), leg(0, 500)];
    let m = misclosure(p(0, 0), &open, p(1000, 503), &mut meter()).unwrap();
    assert!(m.north().contains(-3.0) && m.east().contains(0.0));
    // A leg over the longest distance is refused.
    assert_eq!(
        misclosure(p(0, 0), &[leg(0, LONGEST.0 + 1)], p(0, 0), &mut meter()),
        Err(CogoError::TooLong)
    );
}

#[test]
fn random_closed_traverses_misclose_within_their_legs_roundings() {
    let mut rng = Rng::for_test("random_closed_traverses_misclose_within_their_legs_roundings");
    for _ in 0..500 {
        let count = rng.range(3, 12) as usize;
        let reach = 1 << 24;
        let corners: Vec<P2> = (0..count)
            .map(|_| p(rng.range(-reach, reach), rng.range(-reach, reach)))
            .collect();
        let mut legs = Vec::new();
        let mut room = 0.0;
        for (i, a) in corners.iter().enumerate() {
            let z = corners[(i + 1) % count];
            let inv = inverse(*a, z).unwrap();
            let Some(b) = inv.bearing() else { continue };
            let d = inv.distance().value() as u64;
            legs.push(Leg {
                bearing: b.value(),
                distance: Steps(d),
            });
            // Half a step of distance and a unit of bearing, about 1.94 · 10^−9 rad, per leg.
            room += 0.5 + d as f64 * 2e-9;
        }
        let start = corners[0];
        let m = misclosure(start, &legs, start, &mut meter()).unwrap();
        assert!(
            m.length().hi() <= room + 1e-6,
            "{} > {room}",
            m.length().hi()
        );
        assert!(m.east().lo() <= m.east().hi());
    }
}

#[test]
fn a_traverse_charges_a_unit_a_leg_and_stops_cleanly() {
    let leg = Leg {
        bearing: Bearing::new(AngleUnits(12_345_678)),
        distance: Steps(1_000),
    };
    let legs = vec![leg; 100];
    let mut m = meter();
    misclosure(p(0, 0), &legs, p(0, 0), &mut m).unwrap();
    assert_eq!(m.used().work, 100);
    for budget in [0u64, 1, 50, 99] {
        let mut small = WorkMeter::new(Budget::new(budget, u64::MAX));
        let e = misclosure(p(0, 0), &legs, p(0, 0), &mut small).unwrap_err();
        assert!(matches!(e, CogoError::Stopped(_)), "{budget}: {e}");
    }
    let mut enough = WorkMeter::new(Budget::new(100, u64::MAX));
    assert!(misclosure(p(0, 0), &legs, p(0, 0), &mut enough).is_ok());
    // Every error says something in words.
    for e in [
        CogoError::TooLong,
        CogoError::Num(NumError::Degenerate),
        misclosure(
            p(0, 0),
            &legs,
            p(0, 0),
            &mut WorkMeter::new(Budget::new(0, u64::MAX)),
        )
        .unwrap_err(),
    ] {
        assert!(e.to_string().len() > 10);
    }
}

#[test]
fn the_longest_distance_is_taken_and_one_step_more_is_refused() {
    // Forward takes LONGEST itself; from the frame's middle it then leaves the frame, which the
    // point refuses, and one step more is too long before any point is formed.
    assert_eq!(
        forward(p(0, 0), Bearing::NORTH, LONGEST),
        Err(CogoError::Num(NumError::OutOfRange {
            value: i128::from(LONGEST.0)
        }))
    );
    assert_eq!(
        forward(p(0, 0), Bearing::NORTH, Steps(LONGEST.0 + 1)),
        Err(CogoError::TooLong)
    );
    // A traverse's leg may be LONGEST too, and not a step more.
    let leg = |distance| Leg {
        bearing: Bearing::new(AngleUnits(30 * PER_DEGREE)),
        distance,
    };
    let m = misclosure(p(0, 0), &[leg(LONGEST)], p(0, 0), &mut meter()).unwrap();
    assert_eq!(m.travelled(), u128::from(LONGEST.0));
    assert_eq!(
        misclosure(p(0, 0), &[leg(Steps(LONGEST.0 + 1))], p(0, 0), &mut meter()),
        Err(CogoError::TooLong)
    );
}

#[test]
fn forward_off_the_quarter_turns_is_certified_not_exact() {
    let from = p(1000, -2000);
    for degrees in [1, 30, 45, 60, 89, 91, 135, 200, 300, 359] {
        let b = Bearing::new(AngleUnits(degrees * PER_DEGREE));
        let f = forward(from, b, Steps(1_000_000_007)).unwrap();
        assert!(!f.is_exact(), "{degrees}°");
        assert!(f.bound().0 >= 1, "{degrees}°");
        assert!(matches!(
            f.exactness(GridStep::Q0),
            Exactness::Certified { .. }
        ));
    }
}

#[test]
fn inverse_along_an_axis_has_its_exact_bearing() {
    for (to, degrees) in [
        (p(0, 7), 0),
        (p(7, 0), 90),
        (p(0, -7), 180),
        (p(-7, 0), 270),
    ] {
        let b = inverse(p(0, 0), to).unwrap().bearing().unwrap();
        assert!(b.is_exact(), "{to:?}");
        assert_eq!(
            b.value().units(),
            AngleUnits(degrees * PER_DEGREE),
            "{to:?}"
        );
    }
}
