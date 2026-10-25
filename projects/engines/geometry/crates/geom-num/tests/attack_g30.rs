//! Attacks on piece G30: symbolic perturbation ordered by keys (`geom_num::perturb`), the
//! predicates that never answer zero. Every test tries to break a claim of the piece's done-when or
//! of the module's guarantees.
//!
//! What decides what is right is an oracle that shares no code and no method with the module: it
//! builds the perturbed matrix itself, each perturbed entry moved by a concrete ε^(2^e) with
//! ε = 2^−K, and takes the sign of its determinant exactly on `dashu` integers (every perturbed
//! column scaled by 2^L, L = K · 2^m, so every entry is a whole number). For K large enough the
//! lowest power of ε present dominates, which is what the module's walk over masks finds. The
//! module never forms such numbers. The oracle runs on orient2d (6 perturbed entries), and on
//! orient3d and incircle (12) for a smaller count, as its numbers reach a million bits; insphere's
//! 20 entries would need far more, so insphere is held to the laws.
//!
//! - **Ties**: collinear, coplanar, cocircular and cospherical points, and repeated points with
//!   distinct keys, against the oracle; never zero.
//! - **The exact sign wins**: random inputs, where the exact predicate is not zero, give its sign.
//! - **The laws**: every permutation of the keyed arguments multiplies the sign by its parity;
//!   relabelling the keys in an order-preserving way changes nothing; a shared key is refused.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "points are a to e as in the predicates"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use dashu_int::IBig;
use geom_num::grid::{P2, P3};
use geom_num::{NumError, Sign, perturb, predicate};

/// The default seed of every randomised test here (G30, issue 221).
const DEFAULT_SEED: u64 = 0x6730_0000_5eed_0221;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), written out as the other predicate tests do.
struct Rng(u64);

impl Rng {
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        let mut name_hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in name.bytes() {
            name_hash = (name_hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        Rng(seed ^ name_hash)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (i128::from(hi) - i128::from(lo) + 1) as u128;
        (i128::from(lo) + (u128::from(self.next()) % span) as i128) as i64
    }

    /// Distinct keys, in a pseudo-random order.
    fn keys<const N: usize>(&mut self) -> [u64; N] {
        let mut keys = [0u64; N];
        let mut i = 0;
        while i < N {
            let k = self.next() % 1000;
            if !keys[..i].contains(&k) {
                keys[i] = k;
                i += 1;
            }
        }
        keys
    }
}

fn p2([x, y]: [i64; 2]) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: [i64; 3]) -> P3 {
    P3::new(x, y, z).unwrap()
}

// ---------------------------------------------------------------------------------------------
// The oracle: the perturbed determinant with a concrete ε, on dashu integers.
// ---------------------------------------------------------------------------------------------

fn sign_of(v: &IBig) -> Sign {
    match v.signum().cmp(&IBig::ZERO) {
        std::cmp::Ordering::Less => Sign::Negative,
        std::cmp::Ordering::Equal => Sign::Zero,
        std::cmp::Ordering::Greater => Sign::Positive,
    }
}

/// The determinant of a square matrix, by the Leibniz formula.
fn det(m: &[Vec<IBig>]) -> IBig {
    let n = m.len();
    let mut perm: Vec<usize> = (0..n).collect();
    let mut total = IBig::ZERO;
    loop {
        let inversions = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .filter(|&(i, j)| perm[i] > perm[j])
            .count();
        let mut term = IBig::ONE;
        for (row, &col) in m.iter().zip(&perm) {
            term *= &row[col];
        }
        if inversions % 2 == 1 {
            total -= term;
        } else {
            total += term;
        }
        // Next permutation in lexicographic order.
        let Some(i) = (0..n.saturating_sub(1))
            .rev()
            .find(|&i| perm[i] < perm[i + 1])
        else {
            return total;
        };
        let j = (i + 1..n).rev().find(|&j| perm[j] > perm[i]).unwrap();
        perm.swap(i, j);
        perm[i + 1..].reverse();
    }
}

/// The sign of the perturbed determinant of the absolute matrix whose rows are `rows` (the
/// perturbed entries, then the constant 1), each row with its key: entry c of the point of key
/// rank r moved by ε^(2^(r·P + c)), ε = 2^−k.
fn oracle(rows: &[(Vec<i64>, u64)], k: u32) -> Sign {
    let n = rows.len();
    let p = n - 1;
    let m = u32::try_from(n * p).unwrap();
    let l = u64::from(k) << m;
    let mut ranks: Vec<usize> = (0..n).collect();
    ranks.sort_by_key(|&i| rows[i].1);
    let mut matrix: Vec<Vec<IBig>> = Vec::with_capacity(n);
    for (i, (entries, _)) in rows.iter().enumerate() {
        let rank = ranks.iter().position(|&r| r == i).unwrap();
        let mut row = Vec::with_capacity(n);
        for (c, &v) in entries.iter().enumerate() {
            let e = (rank * p + c) as u32;
            let shift = l - u64::from(k) * (1u64 << e);
            row.push((IBig::from(v) << l as usize) + (IBig::ONE << shift as usize));
        }
        row.push(IBig::ONE);
        matrix.push(row);
    }
    sign_of(&det(&matrix))
}

fn rows2(points: &[([i64; 2], u64)], lifted: bool) -> Vec<(Vec<i64>, u64)> {
    points
        .iter()
        .map(|&([x, y], key)| {
            let mut v = vec![x, y];
            if lifted {
                v.push(x * x + y * y);
            }
            (v, key)
        })
        .collect()
}

fn rows3(points: &[([i64; 3], u64)], lifted: bool) -> Vec<(Vec<i64>, u64)> {
    points
        .iter()
        .map(|&([x, y, z], key)| {
            let mut v = vec![x, y, z];
            if lifted {
                v.push(x * x + y * y + z * z);
            }
            (v, key)
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Calls with keys attached.
// ---------------------------------------------------------------------------------------------

fn o2(p: &[([i64; 2], u64); 3]) -> Result<Sign, NumError> {
    perturb::orient2d(
        (p2(p[0].0), p[0].1),
        (p2(p[1].0), p[1].1),
        (p2(p[2].0), p[2].1),
    )
}

fn ic(p: &[([i64; 2], u64); 4]) -> Result<Sign, NumError> {
    let k = |i: usize| (p2(p[i].0), p[i].1);
    perturb::incircle(k(0), k(1), k(2), k(3))
}

fn o3(p: &[([i64; 3], u64); 4]) -> Result<Sign, NumError> {
    let k = |i: usize| (p3(p[i].0), p[i].1);
    perturb::orient3d(k(0), k(1), k(2), k(3))
}

fn is(p: &[([i64; 3], u64); 5]) -> Result<Sign, NumError> {
    let k = |i: usize| (p3(p[i].0), p[i].1);
    perturb::insphere(k(0), k(1), k(2), k(3), k(4))
}

/// Every permutation of 0..n with its parity, in lexicographic order.
fn permutations(n: usize) -> Vec<(Vec<usize>, bool)> {
    let mut out = Vec::new();
    let mut perm: Vec<usize> = (0..n).collect();
    loop {
        let inversions = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .filter(|&(i, j)| perm[i] > perm[j])
            .count();
        out.push((perm.clone(), inversions % 2 == 1));
        let Some(i) = (0..n.saturating_sub(1))
            .rev()
            .find(|&i| perm[i] < perm[i + 1])
        else {
            return out;
        };
        let j = (i + 1..n).rev().find(|&j| perm[j] > perm[i]).unwrap();
        perm.swap(i, j);
        perm[i + 1..].reverse();
    }
}

/// The permutation law for one input: every reordering gives the sign times its parity.
fn laws<T: Copy, const N: usize>(points: [T; N], f: impl Fn(&[T; N]) -> Result<Sign, NumError>) {
    let base = f(&points).unwrap();
    assert_ne!(base, Sign::Zero, "a perturbed sign is never zero");
    for (perm, odd) in permutations(N) {
        let moved: [T; N] = core::array::from_fn(|i| points[perm[i]]);
        let want = if odd { base.negate() } else { base };
        assert_eq!(f(&moved).unwrap(), want, "permutation {perm:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Degenerate inputs.
// ---------------------------------------------------------------------------------------------

fn collinear2(rng: &mut Rng) -> [[i64; 2]; 3] {
    let a = [rng.range(-1000, 1000), rng.range(-1000, 1000)];
    let u = [rng.range(-50, 50), rng.range(-50, 50)];
    let (s, t) = (rng.range(-3, 3), rng.range(-3, 3));
    [
        a,
        [a[0] + s * u[0], a[1] + s * u[1]],
        [a[0] + t * u[0], a[1] + t * u[1]],
    ]
}

fn cocircular(rng: &mut Rng) -> [[i64; 2]; 4] {
    // Integer points on x² + y² = 25 (and repeats), moved by a random offset.
    const ON: [[i64; 2]; 12] = [
        [5, 0],
        [4, 3],
        [3, 4],
        [0, 5],
        [-3, 4],
        [-4, 3],
        [-5, 0],
        [-4, -3],
        [-3, -4],
        [0, -5],
        [3, -4],
        [4, -3],
    ];
    let o = [rng.range(-1000, 1000), rng.range(-1000, 1000)];
    core::array::from_fn(|_| {
        let p = ON[rng.range(0, 11) as usize];
        [p[0] + o[0], p[1] + o[1]]
    })
}

fn coplanar(rng: &mut Rng) -> [[i64; 3]; 4] {
    let a: [i64; 3] = core::array::from_fn(|_| rng.range(-1000, 1000));
    let u: [i64; 3] = core::array::from_fn(|_| rng.range(-20, 20));
    let v: [i64; 3] = core::array::from_fn(|_| rng.range(-20, 20));
    core::array::from_fn(|_| {
        let (s, t) = (rng.range(-3, 3), rng.range(-3, 3));
        core::array::from_fn(|i| a[i] + s * u[i] + t * v[i])
    })
}

fn cospherical(rng: &mut Rng) -> [[i64; 3]; 5] {
    // Integer points on x² + y² + z² = 9 (and repeats), moved by a random offset.
    const ON: [[i64; 3]; 10] = [
        [3, 0, 0],
        [-3, 0, 0],
        [0, 3, 0],
        [0, -3, 0],
        [0, 0, 3],
        [0, 0, -3],
        [1, 2, 2],
        [2, -1, 2],
        [-2, 2, 1],
        [2, 2, -1],
    ];
    let o: [i64; 3] = core::array::from_fn(|_| rng.range(-1000, 1000));
    core::array::from_fn(|_| {
        let p = ON[rng.range(0, 9) as usize];
        core::array::from_fn(|i| p[i] + o[i])
    })
}

// ---------------------------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------------------------

#[test]
fn orient2d_ties_match_the_oracle_and_obey_the_laws() {
    let mut rng = Rng::for_test("orient2d_ties_match_the_oracle_and_obey_the_laws");
    for _ in 0..300 {
        let pts = collinear2(&mut rng);
        let keys = rng.keys::<3>();
        let keyed: [([i64; 2], u64); 3] = core::array::from_fn(|i| (pts[i], keys[i]));
        assert_eq!(
            predicate::orient2d(p2(pts[0]), p2(pts[1]), p2(pts[2])),
            Ok(Sign::Zero)
        );
        assert_eq!(
            o2(&keyed).unwrap(),
            oracle(&rows2(&keyed, false), 128),
            "{keyed:?}"
        );
        laws(keyed, o2);
    }
}

#[test]
fn incircle_and_orient3d_ties_match_the_oracle() {
    let mut rng = Rng::for_test("incircle_and_orient3d_ties_match_the_oracle");
    for _ in 0..if cfg!(debug_assertions) { 2 } else { 12 } {
        let pts = cocircular(&mut rng);
        let keys = rng.keys::<4>();
        let keyed: [([i64; 2], u64); 4] = core::array::from_fn(|i| (pts[i], keys[i]));
        assert_eq!(
            ic(&keyed).unwrap(),
            oracle(&rows2(&keyed, true), 192),
            "{keyed:?}"
        );
        let pts = coplanar(&mut rng);
        let keys = rng.keys::<4>();
        let keyed: [([i64; 3], u64); 4] = core::array::from_fn(|i| (pts[i], keys[i]));
        // The absolute determinant is −orient3d.
        assert_eq!(
            o3(&keyed).unwrap(),
            oracle(&rows3(&keyed, false), 160).negate(),
            "{keyed:?}"
        );
    }
}

#[test]
fn every_tie_obeys_the_laws_and_is_never_zero() {
    let mut rng = Rng::for_test("every_tie_obeys_the_laws_and_is_never_zero");
    for _ in 0..if cfg!(debug_assertions) { 10 } else { 100 } {
        let keys = rng.keys::<4>();
        let c = cocircular(&mut rng);
        laws(core::array::from_fn(|i| (c[i], keys[i])), ic);
        let q = coplanar(&mut rng);
        laws(core::array::from_fn(|i| (q[i], keys[i])), o3);
        let keys5 = rng.keys::<5>();
        let s = cospherical(&mut rng);
        laws(core::array::from_fn(|i| (s[i], keys5[i])), is);
    }
}

#[test]
fn coincident_points_with_distinct_keys_are_decided() {
    let a2 = [7, -3];
    let a3 = [7, -3, 11];
    assert_ne!(o2(&[(a2, 3), (a2, 1), (a2, 2)]).unwrap(), Sign::Zero);
    assert_ne!(
        ic(&[(a2, 4), (a2, 1), (a2, 3), (a2, 2)]).unwrap(),
        Sign::Zero
    );
    assert_ne!(
        o3(&[(a3, 4), (a3, 1), (a3, 3), (a3, 2)]).unwrap(),
        Sign::Zero
    );
    assert_ne!(
        is(&[(a3, 5), (a3, 1), (a3, 4), (a3, 2), (a3, 3)]).unwrap(),
        Sign::Zero
    );
    let keyed = [(a2, 3), (a2, 1), (a2, 2)];
    assert_eq!(o2(&keyed).unwrap(), oracle(&rows2(&keyed, false), 128));
    laws([(a3, 5), (a3, 1), (a3, 4), (a3, 2), (a3, 3)], is);
}

#[test]
fn the_exact_sign_wins_wherever_it_is_not_zero() {
    let mut rng = Rng::for_test("the_exact_sign_wins_wherever_it_is_not_zero");
    let c = |rng: &mut Rng| rng.range(-(1 << 40) + 1, (1 << 40) - 1);
    for _ in 0..2000 {
        let [a, b, cc, d, e]: [[i64; 3]; 5] =
            core::array::from_fn(|_| [c(&mut rng), c(&mut rng), c(&mut rng)]);
        let keys = rng.keys::<5>();
        let (a2, b2, c2, d2) = ([a[0], a[1]], [b[0], b[1]], [cc[0], cc[1]], [d[0], d[1]]);
        let exact = predicate::orient2d(p2(a2), p2(b2), p2(c2)).unwrap();
        if exact != Sign::Zero {
            assert_eq!(
                o2(&[(a2, keys[0]), (b2, keys[1]), (c2, keys[2])]).unwrap(),
                exact
            );
        }
        let exact = predicate::incircle(p2(a2), p2(b2), p2(c2), p2(d2)).unwrap();
        if exact != Sign::Zero {
            assert_eq!(
                ic(&[(a2, keys[0]), (b2, keys[1]), (c2, keys[2]), (d2, keys[3])]).unwrap(),
                exact
            );
        }
        let exact = predicate::orient3d(p3(a), p3(b), p3(cc), p3(d)).unwrap();
        if exact != Sign::Zero {
            assert_eq!(
                o3(&[(a, keys[0]), (b, keys[1]), (cc, keys[2]), (d, keys[3])]).unwrap(),
                exact
            );
        }
        let exact = predicate::insphere(p3(a), p3(b), p3(cc), p3(d), p3(e)).unwrap();
        if exact != Sign::Zero {
            let keyed = [
                (a, keys[0]),
                (b, keys[1]),
                (cc, keys[2]),
                (d, keys[3]),
                (e, keys[4]),
            ];
            assert_eq!(is(&keyed).unwrap(), exact);
        }
    }
}

#[test]
fn only_the_keys_order_matters_and_a_shared_key_is_refused() {
    let mut rng = Rng::for_test("only_the_keys_order_matters_and_a_shared_key_is_refused");
    for _ in 0..200 {
        let pts = collinear2(&mut rng);
        let keys = rng.keys::<3>();
        let keyed: [([i64; 2], u64); 3] = core::array::from_fn(|i| (pts[i], keys[i]));
        let relabelled: [([i64; 2], u64); 3] =
            core::array::from_fn(|i| (pts[i], keys[i] * 3 + 1000));
        assert_eq!(
            o2(&keyed),
            o2(&relabelled),
            "an order-preserving relabelling changes nothing"
        );
        let shared = [(pts[0], 5), (pts[1], 5), (pts[2], 9)];
        assert_eq!(o2(&shared), Err(NumError::Degenerate));
    }
    let a = [1, 2, 3];
    assert_eq!(
        is(&[(a, 1), (a, 2), (a, 3), (a, 4), (a, 1)]),
        Err(NumError::Degenerate)
    );
}
