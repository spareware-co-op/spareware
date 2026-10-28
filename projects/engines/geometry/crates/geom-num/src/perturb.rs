//! Symbolic perturbation, ordered by persistent keys: orient2d, orient3d, incircle and insphere
//! with no zero answer, for the algorithms that need every sign to be non-zero (Delaunay and
//! constrained triangulations, ray parity; chapter 6.2).
//!
//! Each point carries a key, its persistent name, which the caller supplies. Every perturbed
//! coordinate of every point (and, for incircle and insphere, its lifted coordinate) is moved by
//! an infinitesimal ε^(2^e), where e is the entry's place in one global order: by key, then by
//! column (x, y, z, then the lift). The perturbed predicate is the sign of the determinant of
//! the perturbed points, which is never zero for points with distinct keys. Where the exact sign
//! is not zero, it is the answer, unchanged; the perturbation decides only exact ties.
//!
//! Guarantees:
//!
//! - Never zero, never a guess: every answer is `Positive` or `Negative`, decided by exact integer
//!   arithmetic, or an error. Points that share a key cannot be told apart by any perturbation, so
//!   they are refused with [`NumError::Degenerate`].
//! - The exact sign wins: where [`crate::predicate`]'s predicate is not zero, the perturbed one
//!   returns its sign unchanged.
//! - Consistent: the answers are the signs of one perturbed configuration of points, so every
//!   predicate agrees with every other on the same keyed points; swapping two arguments flips the
//!   sign, and every permutation multiplies it by its parity, as for the exact predicates.
//! - Keys, never addresses: the order is the keys' order alone, so the answers are the same on
//!   every run and every target, whatever the points' places in memory (Geogram's default,
//!   ordering by address, is exactly what chapter 6.2 rules out).
//! - Deterministic: integers only, no floating point, no allocation, no recursion.
//! - Bounded proofs, and what is only tested: Kani proves (piece G41, chapter 28.7) that
//!   `distinct` refuses exactly the keys that repeat, for every two to five `u64` keys; that the
//!   lift |p|² of every frame point, in 2D and 3D, fits an `i128` (no step overflows); and that
//!   `bit_of` gives the bit for every index below 32 and refuses the rest. The cascade over the
//!   masks (up to 2^20 of them, each an exact determinant in [`I256`]) is beyond a bounded proof:
//!   its steps are [`I256`]'s, whose arithmetic Kani and Verus prove (`wide`), its bit budget is
//!   derived below, and it is tested against exact rationals on every tie class.
//!
//! Method: Simulation of Simplicity (Edelsbrunner and Mücke 1990,
//! <https://doi.org/10.1145/77635.77639>), entry j of point i moved by ε^(2^e) with e increasing
//! along a global order of the points, with the lifted coordinate perturbed as an entry of its own
//! (as Devillers and Teillaud perturb the lift for Delaunay, <https://doi.org/10.1016/j.comgeo.2010.09.010>),
//! so the lifted predicates always reach a non-zero term. The perturbed determinant is a polynomial
//! in ε. By multilinearity in the rows, each term chooses, in each row, either the row itself or
//! one of its perturbed entries, so it belongs to a set S of perturbed entries in distinct rows
//! and distinct columns, with degree Σ 2^e over S and coefficient the determinant with the rows of
//! S replaced by the unit vectors of their columns. Degrees are sums of distinct powers of two, so
//! they order the sets S exactly as the sets' bit masks order them: the first non-zero coefficient,
//! walking the masks upwards from 1, decides the sign as ε goes to 0. Mask 0 is the unperturbed
//! determinant. Consulted for the cascade's shape: Geogram's `_SOS` predicates (BSD-3-Clause), no
//! code copied.
//!
//! Consulted: Geogram's PCK `_SOS` predicates, BSD-3-Clause,
//!   <https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/numerics/PCK.h#L143-L172>,
//!   for the shape of a cascade over keyed points, and its address ordering as the trap to avoid.
//!   No code copied.
//!
//! Bit budget: the absolute matrix has rows (coordinates, lift, 1) with coordinates within
//! ±M (M = 2^40 − 1) and the lift at most 3M² < 2^82 in 3D (2M² < 2^81 in 2D); a coefficient's
//! matrix replaces rows by unit vectors, so its entries are among these, 0 and 1. A determinant is
//! a sum of at most 5! = 120 products of one entry per column, each at most 2^82 · (2^40)³ = 2^202
//! (insphere's lift and three coordinate columns), so every partial sum stays below 2^209, in
//! [`I256`] (limit 2^255). Every step is checked, so a step that did not fit would be
//! [`NumError::BudgetExceeded`], never a wrong sign; no grid points can cause it.
//!
//! Cost: on the exact path, the predicate itself. Only on an exact tie, a walk over the masks of
//! the N·(N − 1) perturbed entries (6 for orient2d, 12 for orient3d and incircle, 20 for
//! insphere) until a coefficient is not zero, each candidate an exact N × N determinant by
//! cofactors (at most 120 products in [`I256`]); for points in general position but for the tie,
//! the first few masks decide, and even five coincident points with distinct keys are decided
//! within 2^16 masks.

#![allow(
    clippy::many_single_char_names,
    reason = "points are a to e and matrix entries single letters, as in the predicates' formulas"
)]

use crate::error::NumError;
use crate::grid::{P2, P3};
use crate::predicate;
use crate::sign::Sign;
use crate::wide::I256;

/// [`orient2d`](crate::predicate::orient2d) with every sign non-zero: which way the keyed points
/// a, b, c turn, ties broken by symbolic perturbation in the keys' order.
///
/// # Errors
///
/// [`NumError::Degenerate`] when two points share a key. [`NumError::BudgetExceeded`] only if a
/// checked step failed, which no grid points can cause (the module's bit budget).
pub fn orient2d(a: (P2, u64), b: (P2, u64), c: (P2, u64)) -> Result<Sign, NumError> {
    distinct(&[a.1, b.1, c.1])?;
    match predicate::orient2d(a.0, b.0, c.0)? {
        Sign::Zero => {
            let rows = [a, b, c].map(|(p, key)| ([x(p.x), x(p.y), 1], key));
            tie(rows, det3)
        }
        sign => Ok(sign),
    }
}

/// [`orient3d`](crate::predicate::orient3d) with every sign non-zero, ties broken by symbolic
/// perturbation in the keys' order.
///
/// # Errors
///
/// As for [`orient2d`].
pub fn orient3d(a: (P3, u64), b: (P3, u64), c: (P3, u64), d: (P3, u64)) -> Result<Sign, NumError> {
    distinct(&[a.1, b.1, c.1, d.1])?;
    match predicate::orient3d(a.0, b.0, c.0, d.0)? {
        Sign::Zero => {
            let rows = [a, b, c, d].map(|(p, key)| ([x(p.x), x(p.y), x(p.z), 1], key));
            // The absolute determinant is −orient3d (chapter 6.2's sign convention).
            tie(rows, det4).map(Sign::negate)
        }
        sign => Ok(sign),
    }
}

/// [`incircle`](crate::predicate::incircle) with every sign non-zero, ties broken by symbolic
/// perturbation in the keys' order, the lifted coordinate perturbed as an entry of its own.
///
/// # Errors
///
/// As for [`orient2d`].
pub fn incircle(a: (P2, u64), b: (P2, u64), c: (P2, u64), d: (P2, u64)) -> Result<Sign, NumError> {
    distinct(&[a.1, b.1, c.1, d.1])?;
    match predicate::incircle(a.0, b.0, c.0, d.0)? {
        Sign::Zero => {
            let mut rows = [([0i128; 4], 0u64); 4];
            for (row, (p, key)) in rows.iter_mut().zip([a, b, c, d]) {
                let lift = lift(&[x(p.x), x(p.y)]).ok_or(NumError::BudgetExceeded)?;
                *row = ([x(p.x), x(p.y), lift, 1], key);
            }
            tie(rows, det4)
        }
        sign => Ok(sign),
    }
}

/// [`insphere`](crate::predicate::insphere) with every sign non-zero, ties broken by symbolic
/// perturbation in the keys' order, the lifted coordinate perturbed as an entry of its own.
///
/// # Errors
///
/// As for [`orient2d`].
pub fn insphere(
    a: (P3, u64),
    b: (P3, u64),
    c: (P3, u64),
    d: (P3, u64),
    e: (P3, u64),
) -> Result<Sign, NumError> {
    distinct(&[a.1, b.1, c.1, d.1, e.1])?;
    match predicate::insphere(a.0, b.0, c.0, d.0, e.0)? {
        Sign::Zero => {
            let mut rows = [([0i128; 5], 0u64); 5];
            for (row, (p, key)) in rows.iter_mut().zip([a, b, c, d, e]) {
                let lift = lift(&[x(p.x), x(p.y), x(p.z)]).ok_or(NumError::BudgetExceeded)?;
                *row = ([x(p.x), x(p.y), x(p.z), lift, 1], key);
            }
            // The absolute determinant is −insphere (chapter 6.2's sign convention).
            tie(rows, det5).map(Sign::negate)
        }
        sign => Ok(sign),
    }
}

/// A coordinate as an `i128`.
fn x(c: crate::grid::Coord) -> i128 {
    i128::from(c.get())
}

/// The lifted coordinate |p|², `None` if a step does not fit, which grid points cannot cause.
fn lift(coords: &[i128]) -> Option<i128> {
    coords
        .iter()
        .try_fold(0i128, |sum, &c| sum.checked_add(c.checked_mul(c)?))
}

/// [`NumError::Degenerate`] if two keys are equal.
fn distinct(keys: &[u64]) -> Result<(), NumError> {
    for (i, k) in keys.iter().enumerate() {
        if keys
            .iter()
            .skip(i.saturating_add(1))
            .any(|other| other == k)
        {
            return Err(NumError::Degenerate);
        }
    }
    Ok(())
}

/// The sign of an exact tie's perturbed determinant: rows are the absolute matrix (perturbed
/// entries first, the constant 1 last), in the arguments' order, each with its key. The perturbed
/// entries are ranked by key, then by column; the walk over masks follows the module's method.
fn tie<const N: usize>(
    rows: [([i128; N], u64); N],
    det: fn(&[[i128; N]; N]) -> Option<I256>,
) -> Result<Sign, NumError> {
    // The rows in key order: `order[rank]` is the argument the rank names.
    let mut order: [usize; N] = core::array::from_fn(|i| i);
    order.sort_unstable_by_key(|&i| rows.get(i).map_or(0, |r| r.1));
    let matrix: [[i128; N]; N] = rows.map(|(row, _)| row);
    // The perturbed entries, in ε order: (argument, column) for rank 0's columns, then rank 1's...
    let perturbed = N.saturating_sub(1);
    let entries: usize = N.saturating_mul(perturbed);
    let masks = 1u32.checked_shl(u32::try_from(entries).map_err(|_| NumError::BudgetExceeded)?);
    let masks = masks.ok_or(NumError::BudgetExceeded)?;
    for mask in 1..masks {
        let mut m = matrix;
        let (mut rows_used, mut cols_used) = (0u32, 0u32);
        let mut valid = true;
        for (bit, (&arg, column)) in order
            .iter()
            .flat_map(|arg| (0..perturbed).map(move |column| (arg, column)))
            .enumerate()
        {
            let selected = mask
                .checked_shr(u32::try_from(bit).map_err(|_| NumError::BudgetExceeded)?)
                .is_some_and(|m| m & 1 == 1);
            if !selected {
                continue;
            }
            let (row_bit, col_bit) = (bit_of(arg)?, bit_of(column)?);
            if rows_used & row_bit != 0 || cols_used & col_bit != 0 {
                valid = false;
                break;
            }
            rows_used |= row_bit;
            cols_used |= col_bit;
            if let Some(row) = m.get_mut(arg) {
                *row = core::array::from_fn(|j| i128::from(j == column));
            }
        }
        if !valid {
            continue;
        }
        match det(&m).ok_or(NumError::BudgetExceeded)?.signum() {
            Sign::Zero => {}
            sign => return Ok(sign),
        }
    }
    // Unreachable for distinct keys: choosing, in key order, column k for the argument of rank k
    // leaves the constant column for the last row, a coefficient of ±1.
    Err(NumError::BudgetExceeded)
}

/// The bit of index `i` in a mask of at most 32.
fn bit_of(i: usize) -> Result<u32, NumError> {
    u32::try_from(i)
        .ok()
        .and_then(|i| 1u32.checked_shl(i))
        .ok_or(NumError::BudgetExceeded)
}

/// a · b in [`I256`], from `i128`s.
fn product(a: i128, b: &I256) -> Option<I256> {
    I256::from_i128(a).checked_mul(b)
}

/// The 2 × 2 determinant ad − bc in [`I256`].
fn det2(a: i128, b: i128, c: i128, d: i128) -> Option<I256> {
    product(a, &I256::from_i128(d))?.checked_sub(&product(b, &I256::from_i128(c))?)
}

/// A 3 × 3 determinant by cofactors along the first row.
fn det3(m: &[[i128; 3]; 3]) -> Option<I256> {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    product(a, &det2(e, f, h, i)?)?
        .checked_sub(&product(b, &det2(d, f, g, i)?)?)?
        .checked_add(&product(c, &det2(d, e, g, h)?)?)
}

/// A 4 × 4 determinant by cofactors along the first row.
fn det4(m: &[[i128; 4]; 4]) -> Option<I256> {
    let [[a0, a1, a2, a3], r1, r2, r3] = *m;
    let minor = |drop: usize| {
        let pick = |r: [i128; 4]| -> [i128; 3] {
            let [p, q, s, t] = r;
            match drop {
                0 => [q, s, t],
                1 => [p, s, t],
                2 => [p, q, t],
                _ => [p, q, s],
            }
        };
        det3(&[pick(r1), pick(r2), pick(r3)])
    };
    product(a0, &minor(0)?)?
        .checked_sub(&product(a1, &minor(1)?)?)?
        .checked_add(&product(a2, &minor(2)?)?)?
        .checked_sub(&product(a3, &minor(3)?)?)
}

/// A 5 × 5 determinant by cofactors along the first row.
fn det5(m: &[[i128; 5]; 5]) -> Option<I256> {
    let [[a0, a1, a2, a3, a4], r1, r2, r3, r4] = *m;
    let minor = |drop: usize| {
        let pick = |r: [i128; 5]| -> [i128; 4] {
            let [p, q, s, t, u] = r;
            match drop {
                0 => [q, s, t, u],
                1 => [p, s, t, u],
                2 => [p, q, t, u],
                3 => [p, q, s, u],
                _ => [p, q, s, t],
            }
        };
        det4(&[pick(r1), pick(r2), pick(r3), pick(r4)])
    };
    product(a0, &minor(0)?)?
        .checked_sub(&product(a1, &minor(1)?)?)?
        .checked_add(&product(a2, &minor(2)?)?)?
        .checked_sub(&product(a3, &minor(3)?)?)?
        .checked_add(&product(a4, &minor(4)?)?)
}

#[cfg(test)]
mod tests {
    use super::{det3, det4, det5};
    use crate::wide::I256;

    #[test]
    fn the_determinants_are_exact_on_known_matrices() {
        assert_eq!(
            det3(&[[2, 0, 0], [0, 3, 0], [0, 0, 4]]),
            Some(I256::from_i128(24))
        );
        assert_eq!(
            det3(&[[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
            Some(I256::from_i128(0))
        );
        assert_eq!(
            det4(&[[1, 0, 0, 0], [0, 0, 1, 0], [0, 1, 0, 0], [0, 0, 0, 1]]),
            Some(I256::from_i128(-1))
        );
        assert_eq!(
            det5(&[
                [0, 1, 0, 0, 0],
                [1, 0, 0, 0, 0],
                [0, 0, 1, 0, 0],
                [0, 0, 0, 1, 0],
                [0, 0, 0, 0, 7]
            ]),
            Some(I256::from_i128(-7))
        );
    }
}

/// Bounded proofs (chapter 28.7; piece G41): the helpers the module header names.
#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use super::*;
    use crate::grid::COORD_MAX;

    /// A symbolic frame coordinate, as an `i128`.
    fn frame() -> i128 {
        let v: i64 = kani::any();
        kani::assume(v >= -COORD_MAX && v <= COORD_MAX);
        i128::from(v)
    }

    /// `distinct` refuses exactly the keys that repeat, for every two to five keys.
    #[kani::proof]
    #[kani::unwind(6)]
    fn distinct_refuses_exactly_repeated_keys() {
        let keys: [u64; 5] = kani::any();
        let n: usize = kani::any();
        kani::assume((2..=5).contains(&n));
        let used = &keys[..n];
        let repeated = (0..n).any(|i| (i + 1..n).any(|j| used[i] == used[j]));
        assert!(distinct(used).is_err() == repeated);
    }

    /// The lift of every frame point fits: |p|² < 3 · 2^80 in 3D, so no step overflows, and it is
    /// never negative; the 2D lift is the 3D one with z = 0.
    #[kani::proof]
    #[kani::unwind(4)]
    fn the_lift_of_every_frame_point_fits() {
        let (x, y, z) = (frame(), frame(), frame());
        let l3 = lift(&[x, y, z]);
        assert!(l3.is_some_and(|l| l >= 0 && l < 3 << 80));
        assert!(lift(&[x, y]) == lift(&[x, y, 0]));
    }

    /// `bit_of` gives 2^i for every index below 32 and refuses the rest.
    #[kani::proof]
    fn bit_of_covers_exactly_a_mask_of_32() {
        let i: usize = kani::any();
        match bit_of(i) {
            Ok(b) => assert!(i < 32 && b == 1u32 << i),
            Err(e) => assert!(i >= 32 && e == NumError::BudgetExceeded),
        }
    }
}
