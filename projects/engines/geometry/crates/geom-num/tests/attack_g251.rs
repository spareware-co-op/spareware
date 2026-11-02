//! Piece G251's attack tests: the two predicates `I576` and `I768` serve (decision 32.29).
//! `Tpi::cmp_height` orders three-plane points by height and `Moving::cmp_meeting` orders the
//! meetings of four moving planes in time; both are checked against exact rationals from
//! `num-bigint` and `num-rational`, on small cases, on random planes through grid points, and at
//! the extremes of the budget: planes through corners of the frame, every coordinate at
//! ±(2^40 − 1), and every speed at the offset's limit, 2^125 − 1.
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

use geom_num::NumError;
use geom_num::grid::P3;
use geom_num::plane::{Moving, Plane, Tpi};
use num_bigint::BigInt;
use num_rational::BigRational;

/// The largest grid coordinate, 2^40 − 1.
const M: i64 = (1 << 40) - 1;

/// The largest speed a moving plane may have, 2^125 − 1.
const LIMIT: i128 = (1 << 125) - 1;

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn big(v: i128) -> BigInt {
    BigInt::from(v)
}

/// The determinant of a 3 × 3 matrix of big integers.
fn det3(m: &[[BigInt; 3]; 3]) -> BigInt {
    &m[0][0] * (&m[1][1] * &m[2][2] - &m[1][2] * &m[2][1])
        - &m[0][1] * (&m[1][0] * &m[2][2] - &m[1][2] * &m[2][0])
        + &m[0][2] * (&m[1][0] * &m[2][1] - &m[1][1] * &m[2][0])
}

/// The determinant of a 4 × 4 matrix of big integers, by its first row.
fn det4(m: &[[BigInt; 4]; 4]) -> BigInt {
    let mut sum = BigInt::from(0);
    for col in 0..4 {
        let rest: Vec<usize> = (0..4).filter(|&c| c != col).collect();
        let minor: [[BigInt; 3]; 3] =
            core::array::from_fn(|r| core::array::from_fn(|k| m[r + 1][rest[k]].clone()));
        let term = &m[0][col] * det3(&minor);
        if col % 2 == 0 {
            sum += term;
        } else {
            sum -= term;
        }
    }
    sum
}

/// The height of the point where the three planes meet, exactly, by Cramer's rule on n · x = −d;
/// `None` when their normals are dependent.
fn height(planes: &[Plane; 3]) -> Option<BigRational> {
    let w = det3(&planes.map(|q| q.n().map(big)));
    let with_d: [[BigInt; 3]; 3] = core::array::from_fn(|k| {
        let n = planes[k].n();
        [big(n[0]), big(n[1]), -big(planes[k].d())]
    });
    (w != BigInt::from(0)).then(|| BigRational::new(det3(&with_d), w))
}

/// When the four moving planes meet, exactly: the time t at which the determinant of the rows
/// (n, d + t s) is zero, t = −nd / ns; `None` when ns = 0.
fn time(m: &[Moving; 4]) -> Option<BigRational> {
    let rows = |last: &dyn Fn(&Moving) -> i128| -> [[BigInt; 4]; 4] {
        core::array::from_fn(|k| {
            let n = m[k].plane().n();
            [big(n[0]), big(n[1]), big(n[2]), big(last(&m[k]))]
        })
    };
    let nd = det4(&rows(&|q| q.plane().d()));
    let ns = det4(&rows(&|q| q.speed()));
    (ns != BigInt::from(0)).then(|| BigRational::new(-nd, ns))
}

/// The three planes x = 0, y = 0 and z = h.
fn axes_at(h: i64) -> [Plane; 3] {
    [
        Plane::through(p(0, 0, 0), p(0, 1, 0), p(0, 0, 1)).unwrap(),
        Plane::through(p(0, 0, 0), p(0, 0, 1), p(1, 0, 0)).unwrap(),
        Plane::through(p(0, 0, h), p(1, 0, h), p(0, 1, h)).unwrap(),
    ]
}

fn tpi(planes: [Plane; 3]) -> Tpi {
    Tpi::new(planes[0], planes[1], planes[2]).unwrap()
}

/// The plane through three corners of the frame, each given by the signs of its coordinates:
/// normals near 2^82 and offsets near 2^122, the reach of planes through grid points.
fn through_corners(corners: [[i64; 3]; 3]) -> Plane {
    let [a, b, c] = corners.map(|[x, y, z]| p(x * M, y * M, z * M));
    Plane::through(a, b, c).unwrap()
}

#[test]
fn points_on_one_vertical_are_ordered_by_height() {
    let low = tpi(axes_at(5));
    let high = tpi(axes_at(7));
    assert_eq!(low.cmp_height(&high), Ok(Ordering::Less));
    assert_eq!(high.cmp_height(&low), Ok(Ordering::Greater));
    assert_eq!(low.cmp_height(&low), Ok(Ordering::Equal));
    // The planes in another order negate the weight and the numerators: the same point.
    let [x, y, z] = axes_at(7);
    let swapped = Tpi::new(y, x, z).unwrap();
    assert_eq!(high.cmp_height(&swapped), Ok(Ordering::Equal));
    assert_eq!(low.cmp_height(&swapped), Ok(Ordering::Less));
}

#[test]
fn heights_at_the_extremes_are_exact() {
    // Two points of corner planes, at heights −M/2 and M/2.
    let low = [
        through_corners([[-1, -1, -1], [-1, 1, -1], [1, -1, 1]]),
        through_corners([[-1, -1, -1], [-1, 1, 1], [1, 1, -1]]),
        through_corners([[-1, -1, 1], [-1, 1, -1], [1, -1, -1]]),
    ];
    let high = [
        through_corners([[-1, -1, -1], [-1, 1, -1], [1, -1, 1]]),
        through_corners([[-1, -1, 1], [1, -1, -1], [1, 1, 1]]),
        through_corners([[-1, 1, 1], [1, -1, 1], [1, 1, -1]]),
    ];
    assert_eq!(
        height(&low),
        Some(BigRational::new(big(-i128::from(M)), big(2)))
    );
    // The same point from its planes in another order: a tie of the widest values, which the
    // exact path in I576 settles.
    let again = [low[1], low[0], low[2]];
    for (a, b) in [(low, high), (high, low), (low, again)] {
        let want = height(&a).unwrap().cmp(&height(&b).unwrap());
        assert_eq!(tpi(a).cmp_height(&tpi(b)), Ok(want));
    }
    assert_eq!(tpi(low).cmp_height(&tpi(again)), Ok(Ordering::Equal));
}

/// The plane x + y + z = 10 moving at `speed`, and the three planes x = 0, y = 0, z = 0 standing
/// still: they meet at the origin when 100 · 10 = t · speed (the plane's normal is 100 (1, 1, 1)).
fn corner_and(speed: i128) -> [Moving; 4] {
    let still = axes_at(0).map(|q| Moving::new(q, 0).unwrap());
    let slant = Plane::through(p(10, 0, 0), p(0, 10, 0), p(0, 0, 10)).unwrap();
    [
        still[0],
        still[1],
        still[2],
        Moving::new(slant, speed).unwrap(),
    ]
}

#[test]
fn a_faster_plane_meets_the_corner_first() {
    let slow = corner_and(100);
    let fast = corner_and(200);
    assert_eq!(
        time(&slow),
        Some(BigRational::from_integer(BigInt::from(10)))
    );
    assert_eq!(Moving::cmp_meeting(&slow, &fast), Ok(Ordering::Greater));
    assert_eq!(Moving::cmp_meeting(&fast, &slow), Ok(Ordering::Less));
    assert_eq!(Moving::cmp_meeting(&slow, &slow), Ok(Ordering::Equal));
}

#[test]
fn planes_that_never_meet_at_one_time_are_degenerate() {
    let still = corner_and(0);
    assert_eq!(time(&still), None);
    assert_eq!(
        Moving::cmp_meeting(&still, &corner_and(100)),
        Err(NumError::Degenerate)
    );
    assert_eq!(
        Moving::cmp_meeting(&corner_and(100), &still),
        Err(NumError::Degenerate)
    );
}

#[test]
fn a_speed_past_the_offsets_budget_is_refused() {
    let q = axes_at(0)[0];
    assert!(Moving::new(q, LIMIT).is_ok());
    assert!(Moving::new(q, -LIMIT).is_ok());
    assert_eq!(Moving::new(q, LIMIT + 1), Err(NumError::BudgetExceeded));
    assert_eq!(Moving::new(q, -LIMIT - 1), Err(NumError::BudgetExceeded));
}

#[test]
fn meetings_at_the_extremes_are_exact() {
    let corner = [-1, -1, -1];
    let planes = [
        through_corners([corner, [-1, -1, 1], [-1, 1, -1]]),
        through_corners([corner, [-1, -1, 1], [1, -1, -1]]),
        through_corners([corner, [-1, 1, -1], [1, -1, -1]]),
        through_corners([[-1, -1, 1], [-1, 1, -1], [1, -1, -1]]),
        through_corners([[-1, -1, 1], [-1, 1, -1], [1, -1, 1]]),
    ];
    let first = [
        Moving::new(planes[0], LIMIT).unwrap(),
        Moving::new(planes[1], LIMIT).unwrap(),
        Moving::new(planes[2], LIMIT).unwrap(),
        Moving::new(planes[3], -LIMIT).unwrap(),
    ];
    let second = [
        Moving::new(planes[0], LIMIT).unwrap(),
        Moving::new(planes[1], LIMIT).unwrap(),
        Moving::new(planes[2], LIMIT).unwrap(),
        Moving::new(planes[4], LIMIT).unwrap(),
    ];
    // The same meeting with its planes in another order: a tie of the widest values, which the
    // exact path in I768 settles.
    let again = [first[1], first[0], first[2], first[3]];
    for (a, b) in [(first, second), (second, first), (first, again)] {
        let want = time(&a).unwrap().cmp(&time(&b).unwrap());
        assert_eq!(Moving::cmp_meeting(&a, &b), Ok(want));
    }
    assert_eq!(Moving::cmp_meeting(&first, &again), Ok(Ordering::Equal));
}

/// The default seed of every randomised test here.
const DEFAULT_SEED: u64 = 0x6251_0000_5eed_0251;

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
fn heights_are_the_exact_order() {
    let mut rng = Rng::for_test("heights_are_the_exact_order");
    let mut checked = 0;
    while checked < 2_000 {
        let planes: [Option<Plane>; 6] = core::array::from_fn(|_| rng.plane());
        let [Some(a), Some(b), Some(c), Some(e), Some(f), Some(g)] = planes else {
            continue;
        };
        let (Ok(x), Ok(y)) = (Tpi::new(a, b, c), Tpi::new(e, f, g)) else {
            continue;
        };
        let want = height(&[a, b, c])
            .unwrap()
            .cmp(&height(&[e, f, g]).unwrap());
        assert_eq!(x.cmp_height(&y), Ok(want));
        // Each against itself through its planes in another order: the exact path's tie.
        assert_eq!(
            x.cmp_height(&Tpi::new(b, a, c).unwrap()),
            Ok(Ordering::Equal)
        );
        checked += 1;
    }
}

#[test]
fn meetings_are_the_exact_order() {
    let mut rng = Rng::for_test("meetings_are_the_exact_order");
    let mut checked = 0;
    while checked < 1_000 {
        let planes: [Option<Plane>; 8] = core::array::from_fn(|_| rng.plane());
        let Some(planes) = planes.into_iter().collect::<Option<Vec<Plane>>>() else {
            continue;
        };
        let moving: Vec<Moving> = planes
            .iter()
            .map(|q| Moving::new(*q, rng.speed()).unwrap())
            .collect();
        let a: [Moving; 4] = core::array::from_fn(|k| moving[k]);
        let b: [Moving; 4] = core::array::from_fn(|k| moving[k + 4]);
        match (time(&a), time(&b)) {
            (Some(ta), Some(tb)) => {
                assert_eq!(Moving::cmp_meeting(&a, &b), Ok(ta.cmp(&tb)));
                let again = [a[1], a[0], a[2], a[3]];
                assert_eq!(Moving::cmp_meeting(&a, &again), Ok(Ordering::Equal));
            }
            _ => assert_eq!(Moving::cmp_meeting(&a, &b), Err(NumError::Degenerate)),
        }
        checked += 1;
    }
}
