//! Where two lines cross in the plane, exact: the implicit point where the line through two grid
//! points crosses the line through two others, which side of a third line it lies on, and
//! committing it to the grid.
//!
//! Two segments of a 2D drawing or a face's boundary that cross meet at such a point. Like a
//! three-plane point ([`crate::plane::Tpi`]), it is kept as its definition and never rounded inside
//! an operation (chapter 6.3), and every question about it is answered exactly from that
//! definition.
//!
//! Guarantees:
//!
//! - Every result is exact, or an error. A [`Sign`] is decided by integer arithmetic alone, never
//!   by a rounded crossing point, so an arrangement built on these answers is consistent.
//! - Nothing overflows silently. Every step is a checked operation in `i128` or a fixed-width
//!   [`Wide`] integer, and a step that did not fit would be reported as
//!   [`NumError::BudgetExceeded`], never answered; for grid points no step can fail (the budgets
//!   on [`Ssi::homogeneous`] and [`Ssi::orient2d`]).
//! - Deterministic: integers only, except the generated `f64` filters, whose answers are proven to
//!   be the exact sign or no answer (chapter 6.2); no platform maths and no allocation.
//! - **Bounded proofs, and what is only tested** (chapter 28.7; piece G41 collects them here).
//!   Verus proves that the generated exact paths of `ssi_weight` and `ssi_side`, which every
//!   answer here rests on, compute their values exactly and never overflow for every input in
//!   their declared ranges (piece G39: the twins in `verus/generated/`, token for token the shipped
//!   modules), on [`Wide`] arithmetic that is itself proven exact (G37). The routing between
//!   filter and exact path, the homogeneous coordinates' budgets and the rounding to the grid are
//!   tested against exact oracles, not proven (`tests/attack_g29.rs`, `tests/verification.rs`
//!   and this module's tests), and the filters' error bounds are checked against exact
//!   arithmetic until G40 certifies them.
//!
//! Method: the crossing point of two lines in homogeneous integer coordinates, classified against
//! a line by multiplying by the sign of the weight so that nothing is divided, the plane-based
//! method of [`crate::plane`] in two dimensions (Campen and Kobbelt 2010,
//! <https://doi.org/10.1111/j.1467-8659.2009.01609.x>; Attene 2020's indirect predicates,
//! <https://doi.org/10.1016/j.cad.2020.102856>, method only, re-derived here). Each sign goes
//! through its generated filter first and its generated exact path when the filter cannot decide
//! (`ssi_weight` and `ssi_side`, chapter 6.2), then through the hand-written path.
//!
//! Cost: constant time and no allocation: a few `i128` products for the weight and the
//! numerators, two products in [`I256`] for a side, and one restoring long division per
//! coordinate to commit, as in [`crate::plane`].

use crate::error::NumError;
use crate::generated;
use crate::grid::{Coord, P2, Rounded};
use crate::plane::{BUDGET_LIMBS, TOTAL_LIMBS, commit_axes, difference};
use crate::sign::Sign;
use crate::wide::{I256, I320, Wide};

/// The point where the line from `a` to `b` crosses the line from `c` to `d`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ssi {
    /// The first line's two points, then the second line's.
    points: [P2; 4],
}

impl Ssi {
    /// The point where the line from `a` to `b` crosses the line from `c` to `d`.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when the lines meet in no point or in a whole line: they are
    /// parallel or the same line, or a = b, or c = d. Decided exactly: w = (b − a) × (d − c) = 0.
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    pub fn new(a: P2, b: P2, c: P2, d: P2) -> Result<Ssi, NumError> {
        let s = Ssi {
            points: [a, b, c, d],
        };
        match s.offsets() {
            Some((oa, ob)) if oa == ob => Err(NumError::Degenerate),
            Some(_) => Ok(s),
            None => Err(NumError::BudgetExceeded),
        }
    }

    /// The four points, in the order given to [`Ssi::new`].
    #[must_use]
    pub const fn points(&self) -> [P2; 4] {
        self.points
    }

    /// oa = orient2d(c, d, a) and ob = orient2d(c, d, b), in `i128`: how far a and b lie on
    /// either side of the second line.
    fn offsets(&self) -> Option<(i128, i128)> {
        let [a, b, c, d] = self.points;
        Some((cross(c, d, a)?, cross(c, d, b)?))
    }

    /// The point's homogeneous coordinates: numerators X and denominator w, with the point at
    /// X / w. With oa = orient2d(c, d, a) and ob = orient2d(c, d, b), the point a + t (b − a) lies
    /// on the line from c to d for t = oa / (oa − ob), so X = oa b − ob a and w = oa − ob =
    /// (b − a) × (d − c), never zero.
    ///
    /// Bit budget: with coordinates within ±M (M = 2^40 − 1), each of oa, ob is a 2 × 2
    /// determinant of differences within ±2M, whose largest magnitude lies at a corner of the box,
    /// at most 4M² (82 bits); so |w| ≤ 8M² and |`X_i`| ≤ 8M³ (123 bits), all in `i128`.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step does not fit, which no grid points can cause (the
    /// budget above).
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    pub fn homogeneous(&self) -> Result<([i128; 2], i128), NumError> {
        let [a, b, _, _] = self.points;
        let Some((oa, ob)) = self.offsets() else {
            return Err(NumError::BudgetExceeded);
        };
        let (Some(x), Some(y), Some(w)) = (
            ssi_axis(oa, ob, b.x, a.x),
            ssi_axis(oa, ob, b.y, a.y),
            oa.checked_sub(ob),
        ) else {
            return Err(NumError::BudgetExceeded);
        };
        Ok(([x, y], w))
    }

    /// Which side of the line from `p` to `q` the point lies on: orient2d(p, q, x) for the
    /// crossing point x, in [`crate::predicate::orient2d`]'s convention: `Positive` when p, q, x
    /// turn counter-clockwise, `Zero` exactly when x lies on the line.
    ///
    /// orient2d(p, q, ·) is affine, so orient2d(p, q, X / w) = (oa · orient2d(p, q, b) −
    /// ob · orient2d(p, q, a)) / w, whose sign is the sign of oa · ob₂ − ob · oa₂ (with
    /// oa₂ = orient2d(p, q, a), ob₂ = orient2d(p, q, b)) times the sign of w.
    ///
    /// Bit budget: each orient2d of grid points is at most 4M² (82 bits), so each product is at
    /// most 16M⁴ and their difference at most 32M⁴ < 2^165, in [`I256`]. Total path, unreachable:
    /// should a step not fit anyway, the same difference is computed in [`I320`], where two
    /// products of `i128`s (at most 2^254 each) cannot overflow.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if an orient2d does not fit an `i128` or the total path
    /// failed, neither of which grid points can cause.
    pub fn orient2d(&self, p: P2, q: P2) -> Result<Sign, NumError> {
        self.routed(p, q)
            .map_or_else(|| self.orient2d_hand(p, q), Ok)
    }

    /// [`Ssi::orient2d`] through the generated predicates: `ssi_side` times `ssi_weight`, each
    /// filter first and its generated exact path otherwise. `None` only when a generated exact
    /// path does not fit its certified types.
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    fn routed(&self, p: P2, q: P2) -> Option<Sign> {
        let [a, b, c, d] = self.points;
        let weight = generated::ssi_weight::filter(a, b, c, d)
            .or_else(|| generated::ssi_weight::exact(a, b, c, d))?;
        let side = generated::ssi_side::filter(a, b, c, d, p, q)
            .or_else(|| generated::ssi_side::exact(a, b, c, d, p, q))?;
        Some(side.mul(weight))
    }

    /// [`Ssi::orient2d`] by hand: oa · ob₂ − ob · oa₂ in [`I256`], then in [`I320`], times the
    /// sign of w.
    #[inline(never)]
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    fn orient2d_hand(&self, p: P2, q: P2) -> Result<Sign, NumError> {
        let [a, b, _, _] = self.points;
        let (Some((oa, ob)), Some(oa2), Some(ob2)) =
            (self.offsets(), cross(p, q, a), cross(p, q, b))
        else {
            return Err(NumError::BudgetExceeded);
        };
        let Some(w) = oa.checked_sub(ob) else {
            return Err(NumError::BudgetExceeded);
        };
        let s = match side::<4>(oa, ob2, ob, oa2) {
            Some(s) => Some(s),
            None => side::<5>(oa, ob2, ob, oa2),
        };
        match s {
            Some(s) => Ok(s.mul(Sign::of_i128(w))),
            None => Err(NumError::BudgetExceeded),
        }
    }

    /// The point committed to the grid: each coordinate `X_i` / w rounded half to even, by the same
    /// code that commits every implicit point ([`crate::plane::Tpi::to_grid`]), with whether both
    /// coordinates were exact.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] for the first coordinate outside the frame (two lines can cross
    /// far outside it), and [`NumError::BudgetExceeded`] as for [`Ssi::homogeneous`].
    pub fn to_grid(&self) -> Result<Rounded<P2>, NumError> {
        let ([x, y], w) = self.homogeneous()?;
        let x = [I320::from_i128(x), I320::from_i128(y)];
        let w = I256::from_i128(w);
        commit_axes::<BUDGET_LIMBS, 2>(&x, &w)
            .or_else(|| commit_axes::<TOTAL_LIMBS, 2>(&x, &w))
            .unwrap_or(Err(NumError::BudgetExceeded))
            .map(|([x, y], exact)| Rounded::new(P2 { x, y }, exact))
    }
}

/// (b − a) × (c − a) = orient2d(a, b, c) for grid points, in `i128`: `None` if a step does not
/// fit, which coordinates within the frame cannot cause.
fn cross(a: P2, b: P2, c: P2) -> Option<i128> {
    let (bx, by, cx, cy) = (
        difference(b.x, a.x)?,
        difference(b.y, a.y)?,
        difference(c.x, a.x)?,
        difference(c.y, a.y)?,
    );
    bx.checked_mul(cy)?.checked_sub(by.checked_mul(cx)?)
}

/// One numerator of [`Ssi::homogeneous`], oa · `b_i` − ob · `a_i`, in `i128`: `None` if a step does
/// not fit, which grid points cannot cause.
fn ssi_axis(oa: i128, ob: i128, bc: Coord, ac: Coord) -> Option<i128> {
    oa.checked_mul(i128::from(bc.get()))?
        .checked_sub(ob.checked_mul(i128::from(ac.get()))?)
}

/// The sign of oa · ob₂ − ob · oa₂ in `Wide<L>`: `None` if a step does not fit.
fn side<const L: usize>(oa: i128, ob2: i128, ob: i128, oa2: i128) -> Option<Sign> {
    let left = Wide::<L>::product_i128(oa, ob2)?;
    let right = Wide::<L>::product_i128(ob, oa2)?;
    Some(left.checked_sub(&right)?.signum())
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "tests may use native arithmetic and casts, and name points as the formulas do"
)]
mod tests {
    //! The hand-written path on its own. `Ssi::orient2d` answers through the generated
    //! predicates for every crossing of grid points, so no public call reaches `orient2d_hand`
    //! or `side` behind it. These tests call them directly: the hand-written sign against the
    //! side of the crossing point itself, formed by division in exact rationals, and `side` in
    //! both widths against `num-bigint` at the extremes of `i128`.

    use num_bigint::BigInt;
    use num_rational::BigRational;

    use super::{Ssi, side};
    use crate::error::NumError;
    use crate::grid::{COORD_MAX, P2};
    use crate::sign::Sign;

    const M: i64 = COORD_MAX;

    /// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), as the crate's other tests write it.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// A coordinate: an edge of the grid, a step inside it, a small value, or any value.
        fn coord(&mut self) -> i64 {
            match self.next() % 8 {
                0 => M,
                1 => -M,
                2 => M - 1,
                3 => 1 - M,
                4 | 5 => (self.next() % 7) as i64 - 3,
                _ => (self.next() % (2 * M as u64 + 1)) as i64 - M,
            }
        }

        fn point(&mut self) -> [i64; 2] {
            [self.coord(), self.coord()]
        }
    }

    fn p2(c: [i64; 2]) -> P2 {
        P2::new(c[0], c[1]).unwrap()
    }

    fn sign_of(v: &BigInt) -> Sign {
        match v.sign() {
            num_bigint::Sign::Minus => Sign::Negative,
            num_bigint::Sign::NoSign => Sign::Zero,
            num_bigint::Sign::Plus => Sign::Positive,
        }
    }

    fn count(seen: &mut [usize; 3], sign: Sign) {
        seen[match sign {
            Sign::Negative => 0,
            Sign::Zero => 1,
            Sign::Positive => 2,
        }] += 1;
    }

    fn rational(v: i64) -> BigRational {
        BigRational::from_integer(BigInt::from(v))
    }

    /// (u − o) × (v − o) in exact rationals, for a point v that need not be on the grid.
    fn turn(o: [i64; 2], u: [i64; 2], v: &[BigRational; 2]) -> BigRational {
        (rational(u[0]) - rational(o[0])) * (&v[1] - rational(o[1]))
            - (rational(u[1]) - rational(o[1])) * (&v[0] - rational(o[0]))
    }

    /// The point where the line a b crosses the line c d, in exact rationals: a + t (b − a) with
    /// t = orient2d(c, d, a) / (orient2d(c, d, a) − orient2d(c, d, b)); `None` when the lines are
    /// parallel or one of them is a single point.
    fn crossing(pts: [[i64; 2]; 4]) -> Option<[BigRational; 2]> {
        let [a, b, c, d] = pts;
        let oa = turn(c, d, &a.map(rational));
        let ob = turn(c, d, &b.map(rational));
        if oa == ob {
            return None;
        }
        let t = &oa / (&oa - &ob);
        Some([0, 1].map(|k| rational(a[k]) + &t * (rational(b[k]) - rational(a[k]))))
    }

    /// The oracle itself, on a point known by eye: the diagonals of the square from (0, 0) to
    /// (4, 4) cross at (2, 2), and parallel lines nowhere.
    #[test]
    fn the_rational_oracle_finds_a_crossing_known_by_eye() {
        let x = crossing([[0, 0], [4, 4], [0, 4], [4, 0]]);
        assert_eq!(x, Some([rational(2), rational(2)]));
        assert_eq!(crossing([[0, 0], [4, 4], [1, 0], [5, 4]]), None);
        // The x axis walked rightwards has (2, 2) on its left.
        let x = x.unwrap();
        assert_eq!(sign_of(turn([0, 0], [1, 0], &x).numer()), Sign::Positive);
    }

    /// `orient2d_hand`, the path `Ssi::orient2d` takes when the generated one gives nothing,
    /// which no grid points cause: the side of the crossing point itself, for the crossing's own
    /// lines (`Zero`), lines one step off them and random lines; on an ordinary crossing, on
    /// nearly parallel lines at the edge of the grid (weights of ±1 between products near 2^80),
    /// on the frame's diagonals and on random crossings. The public function gives the same
    /// sign.
    #[test]
    fn the_hand_written_side_of_a_crossing_point_is_exact() {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(0x6741_0000_5eed_0004);
        println!("orient2d_hand: seed: {seed:016x}");
        let mut rng = Rng(seed);
        let mut cases = vec![
            [[0, 0], [4, 4], [0, 4], [4, 0]],
            [[0, 0], [M, M - 1], [-3, -7], [M - 4, M - 9]],
            [[0, 0], [M, M - 1], [M - 4, M - 9], [-3, -7]],
            [[-M, -M], [M, M], [-M, M], [M, -M]],
            [[-M, -M], [M, M - 1], [M, -M], [-M, M - 1]],
        ];
        while cases.len() < 400 {
            cases.push([rng.point(), rng.point(), rng.point(), rng.point()]);
        }
        let mut tally = [0usize; 3];
        let mut points = 0;
        for pts in cases {
            let [a, b, c, d] = pts;
            let made = Ssi::new(p2(a), p2(b), p2(c), p2(d));
            let Some(x) = crossing(pts) else {
                assert_eq!(made, Err(NumError::Degenerate), "{pts:?}");
                continue;
            };
            points += 1;
            let s = made.unwrap();
            let mut check = |p: [i64; 2], q: [i64; 2]| {
                let expected = sign_of(turn(p, q, &x).numer());
                let (p, q) = (p2(p), p2(q));
                assert_eq!(s.orient2d_hand(p, q), Ok(expected), "{pts:?} {p:?} {q:?}");
                assert_eq!(s.orient2d(p, q), Ok(expected), "{pts:?} {p:?} {q:?}");
                count(&mut tally, expected);
                expected
            };
            // The point lies on both of its own lines, whichever way each is walked.
            for (p, q) in [(a, b), (b, a), (c, d), (d, c)] {
                assert_eq!(check(p, q), Sign::Zero);
            }
            // A line from a to a point one step off b has the point beside it, unless that
            // step keeps it on the line: the oracle says which.
            for step in [[0, 1], [1, 0], [0, -1], [-1, 0]] {
                let near = [b[0] + step[0], b[1] + step[1]];
                if near[0].abs() <= M && near[1].abs() <= M {
                    check(a, near);
                }
            }
            for _ in 0..8 {
                check(rng.point(), rng.point());
            }
        }
        assert!(points >= 300, "only {points} pairs of lines crossed");
        assert!(tally.iter().all(|&n| n > 300), "{tally:?}");
    }

    /// `side` in both widths: the sign of oa · ob₂ − ob · oa₂, against `num-bigint`, at every
    /// combination of the extremes of `i128`. Even the narrow width holds every one: each
    /// product is at most 2^254 in magnitude and of the two only one can be 2^254, so the
    /// difference stays within ±(2^255 − 2^127).
    #[test]
    fn side_is_exact_in_both_widths_at_the_extremes_of_i128() {
        let levels = [
            i128::MIN,
            i128::MIN + 1,
            -3,
            -1,
            0,
            1,
            2,
            i128::MAX - 1,
            i128::MAX,
        ];
        let mut tally = [0usize; 3];
        for oa in levels {
            for ob2 in levels {
                for ob in levels {
                    for oa2 in levels {
                        let exact = BigInt::from(oa) * BigInt::from(ob2)
                            - BigInt::from(ob) * BigInt::from(oa2);
                        let expected = sign_of(&exact);
                        let what = format!("{oa} {ob2} {ob} {oa2}");
                        assert_eq!(side::<4>(oa, ob2, ob, oa2), Some(expected), "{what}");
                        assert_eq!(side::<5>(oa, ob2, ob, oa2), Some(expected), "{what}");
                        count(&mut tally, expected);
                    }
                }
            }
        }
        assert_eq!(
            tally[0], tally[2],
            "swapping the two products flips the sign"
        );
        assert!(tally.iter().all(|&n| n > 0), "{tally:?}");
    }
}
