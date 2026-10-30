//! Interval Newton operators for one to four unknowns: Krawczyk's and Hansen and Sengupta's, each
//! of which proves that a box holds exactly one zero of a system of equations, proves that it holds
//! none, or narrows it to where every zero in it lies.
//!
//! What it does: a system `F(x) = 0` of `N` equations in `N` unknowns is written once over
//! [`Scalar`] ([`Residuals`]). One step of [`krawczyk`] or [`hansen_sengupta`] on a box `X`:
//!
//! 1. evaluates `F` and its Jacobian over `X` on dual intervals ([`crate::dual`]), giving
//!    enclosures `F(X)` and `J(X)`; if some `Fᵢ(X)` excludes zero, `X` holds no zero (chapter
//!    12.4's exclusion test) and the step stops there;
//! 2. takes the midpoint `m` of `X`, a double inside it, and an enclosure `F(m)` of `F` there;
//! 3. inverts the midpoint matrix of `J(X)` in doubles by Gaussian elimination with partial
//!    pivoting, giving the preconditioner `C` (any real matrix serves the proofs below; this one
//!    makes `C·J(X)` close to the identity), or refuses with [`NewtonError::Singular`] when a
//!    pivot is zero;
//! 4. forms `A = C·J(X)` and `b = C·F(m)` in outward-rounded interval arithmetic, and the offsets
//!    `D = X − m`;
//! 5. applies its operator: Krawczyk's `K = m − b + (I − A)·D`, or Hansen and Sengupta's interval
//!    Gauss-Seidel sweep on `A·d = −b` over `D`, row by row in order, each row using the rows
//!    before it as already narrowed, a row whose diagonal entry holds zero left as it is.
//!
//! The image strictly inside `X` in every coordinate proves exactly one zero, in the image
//! ([`Verdict::Unique`]); the image missing `X` in some coordinate proves none
//! ([`Verdict::NoZero`]); otherwise the image meets `X` in a smaller box that holds every zero in
//! `X` ([`Verdict::Narrowed`]). Both verdicts that keep a box report an upper bound on
//! ‖I − C·J(X)‖∞, chapter 12.4's contraction factor `r`, from which the root-isolation engine (G53)
//! sets each box's iteration cap. The small solves are written here for fixed sizes; block
//! versions on `geom-linalg` belong to the chained certificates of sketches (G227).
//!
//! The root-isolation engine ([`crate::isolate`]) also runs both operators with the residuals and
//! the Jacobian evaluated in double-double balls ([`crate::ball`], [`Precision::DoubleDouble`]),
//! each result enclosed in an interval before steps 3 to 5, which are the same in either precision.
//! Every guarantee below holds in both: it rests only on the enclosures of steps 1 and 2.
//!
//! Guarantees:
//!
//! - **A unique zero is proven (Krawczyk).** Let `x ∈ X` and let `J̃` be the matrix whose row `i`
//!   is the gradient of `Fᵢ` at a point of the segment from `m` to `x`, so that
//!   `F(x) = F(m) + J̃·(x − m)` (the mean value theorem, row by row, which needs `F` continuously
//!   differentiable on `X`: [`crate::dual`] refuses every box where it is not). Every entry of `J̃`
//!   lies in its entry of `J(X)`, so `g(x) = x − C·F(x) = m − C·F(m) + (I − C·J̃)·(x − m)` lies in
//!   the computed `K`, which encloses that expression for every choice inside its interval
//!   operands. If `K ⊆ int X`, `g` maps `X` into itself and is continuous, so it has a fixed point
//!   (Brouwer). For uniqueness, let `wⱼ` be the width of `Xⱼ`, positive since `Kⱼ` is strictly
//!   inside it. The width of an interval sum is the sum of the widths, and a product with an
//!   interval holding zero is at least as wide as its operand's magnitude times that interval's
//!   width, so the width of `Kᵢ` is at least `Σⱼ |I − A|ᵢⱼ wⱼ`, where `|·|` is the entrywise
//!   magnitude; `Kᵢ ⊂ int Xᵢ` makes that less than `wᵢ`. So the norm `‖v‖_w = maxⱼ |vⱼ|/wⱼ` gives
//!   `‖(I − C·J̃) v‖_w < ‖v‖_w` for every `J̃` in `J(X)` and `v ≠ 0`. Hence `C·J̃` is nonsingular,
//!   so `C` is, and the fixed point is a zero of `F`; and two zeros `x ≠ y` in `X` would give
//!   `x − y = (I − C·J̃)(x − y)` for some such `J̃`, which that inequality forbids. So `X` holds
//!   exactly one zero, and it lies in `K`. The same argument shows that the Jacobian is nonsingular
//!   at every point of `X`: a zero where it is singular, or two zeros, are never reported as one.
//!   The rounding of steps 2 to 5 only widens what the argument uses, so it applies to the
//!   computed `K`. This is the test of Krawczyk (1969) as Moore (1977) and Rump (1983) made it a
//!   proof of existence and uniqueness, here with its uniqueness argument written out in the
//!   weighted norm the strict inclusion itself provides.
//! - **A unique zero is proven (Hansen and Sengupta).** For a box `X`, any `m` in it, a vector `y`
//!   holding `C·F(m)` and an interval matrix holding `C·F′(x)` for every `x` in `X`, Hansen and
//!   Sengupta's operator `X′ = m + Γ(A, −y, X − m)` holds every zero of `F` in `X`, and
//!   `∅ ≠ X′ ⊆ int X` proves that `F` has exactly one zero in `X′` (Neumaier 1990, chapter 5, as
//!   Goldsztejn states it, theorem 1). The computed sweep encloses the exact one: each computed
//!   quotient `qᵢ` holds the exact one, and the narrowed rows feed the later ones as supersets.
//!   [`Verdict::Unique`] is given only when every diagonal entry of `A` excludes zero and every
//!   computed `m + qᵢ` is strictly inside `Xᵢ`. Then each exact quotient is too, so the exact
//!   operator's image is the exact quotients, not empty and inside `int X`.
//! - **No zero is lost.** Every zero of `F` in `X` lies in the enclosure of [`Verdict::Unique`] and
//!   of [`Verdict::Narrowed`]: for Krawczyk because `g` fixes each zero, so each lies in `K`; for
//!   Hansen and Sengupta by the theorem's first part. [`Verdict::NoZero`] is given only when `X`
//!   holds none: some `Fᵢ(X)` excludes zero, or an operator's image misses `X` in a coordinate.
//! - **Singular systems are refused, never certified.** A midpoint Jacobian with a zero pivot is
//!   [`NewtonError::Singular`]. Over any box, a zero where the Jacobian is singular is never
//!   proven unique by Krawczyk's operator (the first guarantee), nor by Hansen and Sengupta's
//!   while the box's diagonal entries hold zero, as they do at a multiple root of one equation.
//! - **Errors, never guesses.** A failing residual is [`NewtonError::Num`] with its error; so is an
//!   enclosure that leaves the finite doubles. Every enclosure is finite and inside the box given.
//! - **Deterministic.** Doubles and outward-rounded intervals only, in a fixed order, through
//!   [`Scalar`]'s operations: the same bits on every target.
//!
//! Method: the Krawczyk operator (R. Krawczyk, "Newton-Algorithmen zur Bestimmung von Nullstellen
//! mit Fehlerschranken", *Computing* 4, 1969, doi:10.1007/BF02234767), as a test of existence (R. E.
//! Moore, "A test for existence of solutions to nonlinear systems", *SIAM J. Numer. Anal.* 14,
//! 1977, doi:10.1137/0714044) and of uniqueness (S. M. Rump, "Solving algebraic problems with high
//! accuracy", in *A New Approach to Scientific Computation*, Academic Press, 1983,
//! doi:10.1016/B978-0-12-428660-3.50010-0; P. Breiding, K. Rose and S. Timme, "Certifying zeros
//! of polynomial systems using interval arithmetic", *ACM Trans. Math. Softw.* 49, 2023, theorem
//! 3.6, <https://arxiv.org/abs/2011.05000>). The Hansen-Sengupta operator (E. Hansen and S.
//! Sengupta, "Bounding solutions of systems of equations using interval analysis", *BIT* 21, 1981,
//! doi:10.1007/BF01933165), in the form of A. Neumaier, *Interval Methods for Systems of
//! Equations*, Cambridge University Press, 1990, chapter 5, doi:10.1017/CBO9780511526473, as A.
//! Goldsztejn states it ("Sensitivity analysis using a fixed point interval iteration", 2008,
//! section 1, <https://arxiv.org/abs/0811.2984>). The preconditioner's inverse by Gaussian
//! elimination with partial pivoting and the triangular solves of G. H. Golub and C. F. Van Loan,
//! *Matrix Computations*, 4th ed., Johns Hopkins University Press, 2013, algorithm 3.4.1 and
//! section 3.1, <https://jhupbooks.press.jhu.edu/title/matrix-computations>, as chapter 12.4 asks.
//! Written for this kernel: the operators must run on its own interval type and maths functions
//! (chapter 21), which no published interval library offers without changing the rounding mode.
//!
//! Cost: two evaluations of the residuals, one on `N` dual intervals and one on `N` point
//! intervals, then O(N³) operations on doubles and intervals for the preconditioner and the
//! products, with `N` at most 4: a few hundred interval operations at most. No allocation, no
//! recursion.

use crate::ball::Ball;
use crate::dual::{Dual, jacobian};
use crate::error::NumError;
use crate::interval::{Interval, max2, min2};
use crate::real::Real;
use crate::scalar::Scalar;
use crate::sign::Sign;

/// A system of `N` equations in `N` unknowns, written once over any [`Scalar`] so that the
/// operators can evaluate it at a point, over a box, and with its derivatives.
pub trait Residuals<const N: usize> {
    /// The `N` residuals at `x`; a zero of the system is an `x` where all of them are zero.
    ///
    /// # Errors
    ///
    /// Whatever the operations return: a domain error, an overflow, or a derivative refused over a
    /// box where the residuals are not differentiable.
    fn residuals<S: Scalar>(&self, x: [S; N]) -> Result<[S; N], NumError>;
}

/// What one step of an operator proved about the box it was given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict<const N: usize> {
    /// The box holds no zero of the system.
    NoZero,
    /// The box holds exactly one zero of the system, and it lies in `enclosure`, which is strictly
    /// inside the box in every coordinate.
    Unique {
        /// A box holding the zero.
        enclosure: [Interval; N],
        /// An upper bound on ‖I − C·J(X)‖∞ (module documentation).
        contraction: Real,
    },
    /// Every zero of the system in the box lies in `enclosure`, a box inside the one given; none is
    /// proven to exist.
    Narrowed {
        /// A box holding every zero of the given box.
        enclosure: [Interval; N],
        /// An upper bound on ‖I − C·J(X)‖∞ (module documentation).
        contraction: Real,
    },
}

/// Why an operator could not make its step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewtonError {
    /// The midpoint of the Jacobian's enclosure has a zero pivot, or an inverse that is not
    /// finite, so the system is refused: no preconditioner can be formed.
    Singular,
    /// The residuals failed, or an enclosure left the finite doubles.
    Num(NumError),
}

impl From<NumError> for NewtonError {
    fn from(e: NumError) -> Self {
        NewtonError::Num(e)
    }
}

impl core::fmt::Display for NewtonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            NewtonError::Singular => {
                f.write_str("the Jacobian at the box's midpoint is singular: the system is refused")
            }
            NewtonError::Num(e) => e.fmt(f),
        }
    }
}

/// Which interval Newton operator a step applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operator {
    /// Krawczyk's operator ([`krawczyk`]).
    Krawczyk,
    /// Hansen and Sengupta's operator ([`hansen_sengupta`]).
    HansenSengupta,
}

/// The arithmetic the residuals and their Jacobian are enclosed in before the operator's linear
/// algebra, which is in intervals of doubles either way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Precision {
    /// Intervals of doubles, rounded outward.
    Double,
    /// Double-double balls ([`crate::ball::Ball`]), each result then enclosed in an interval.
    DoubleDouble,
}

/// How a function written over [`Scalar`] is enclosed over a box: its arithmetic, and the
/// conversions between that arithmetic and intervals. Each enclosure it gives holds the exact
/// function over the box, because each conversion encloses its operand.
pub(crate) trait Arithmetic {
    /// The scalar the function runs on.
    type S: Scalar;

    /// The scalars holding the box `x`.
    fn lift<const N: usize>(x: [Interval; N]) -> Result<[Self::S; N], NumError>;

    /// An interval holding the scalar `s`.
    fn lower(s: Self::S) -> Result<Interval, NumError>;

    /// Enclosures of the residuals of `system` over `x`.
    ///
    /// Over doubles this is the residuals themselves, with no conversion, so the operators' doubles
    /// path is the code it was before balls came: lifting and lowering the box there cost
    /// Krawczyk's and Hansen and Sengupta's steps 1% to 5% more instructions in the count gate.
    fn residuals<R: Residuals<N>, const N: usize>(
        system: &R,
        x: [Interval; N],
    ) -> Result<[Interval; N], NumError>;

    /// Enclosures of the residuals of `system` over `x`, and of their Jacobian.
    #[allow(
        clippy::type_complexity,
        reason = "the values and the Jacobian of N residuals, as dual::jacobian returns them"
    )]
    fn jacobian<R: Residuals<N>, const N: usize>(
        system: &R,
        x: [Interval; N],
    ) -> Result<([Interval; N], [[Interval; N]; N]), NumError>;

    /// Intervals holding the scalars `s`.
    fn lower_all<const N: usize>(s: [Self::S; N]) -> Result<[Interval; N], NumError> {
        let mut out = [Interval::from(Real::ZERO); N];
        for (o, v) in out.iter_mut().zip(s) {
            *o = Self::lower(v)?;
        }
        Ok(out)
    }
}

/// Intervals of doubles: the scalar is the interval itself.
pub(crate) struct InDoubles;

impl Arithmetic for InDoubles {
    type S = Interval;

    fn lift<const N: usize>(x: [Interval; N]) -> Result<[Interval; N], NumError> {
        Ok(x)
    }

    fn lower(s: Interval) -> Result<Interval, NumError> {
        Ok(s)
    }

    fn residuals<R: Residuals<N>, const N: usize>(
        system: &R,
        x: [Interval; N],
    ) -> Result<[Interval; N], NumError> {
        system.residuals(x)
    }

    fn jacobian<R: Residuals<N>, const N: usize>(
        system: &R,
        x: [Interval; N],
    ) -> Result<([Interval; N], [[Interval; N]; N]), NumError> {
        jacobian(|v| system.residuals(v), x)
    }
}

/// Double-double balls ([`Ball`]).
pub(crate) struct InBalls;

impl Arithmetic for InBalls {
    type S = Ball;

    fn lift<const N: usize>(x: [Interval; N]) -> Result<[Ball; N], NumError> {
        let mut out = [Ball::constant(Real::ZERO); N];
        for (o, v) in out.iter_mut().zip(x) {
            *o = Ball::from_interval(v)?;
        }
        Ok(out)
    }

    fn lower(s: Ball) -> Result<Interval, NumError> {
        s.to_interval()
    }

    fn residuals<R: Residuals<N>, const N: usize>(
        system: &R,
        x: [Interval; N],
    ) -> Result<[Interval; N], NumError> {
        InBalls::lower_all(system.residuals(InBalls::lift(x)?)?)
    }

    fn jacobian<R: Residuals<N>, const N: usize>(
        system: &R,
        x: [Interval; N],
    ) -> Result<([Interval; N], [[Interval; N]; N]), NumError> {
        let (values, rows) = jacobian(
            |v: [Dual<Ball, N>; N]| system.residuals(v),
            InBalls::lift(x)?,
        )?;
        let mut jac = [[Interval::from(Real::ZERO); N]; N];
        for (out, row) in jac.iter_mut().zip(rows) {
            *out = InBalls::lower_all(row)?;
        }
        Ok((InBalls::lower_all(values)?, jac))
    }
}

/// One step of Krawczyk's operator on the box `x` (module documentation).
///
/// # Errors
///
/// [`NewtonError::Singular`] when the midpoint Jacobian cannot be inverted;
/// [`NewtonError::Num`] when the residuals fail or an enclosure overflows.
pub fn krawczyk<R: Residuals<N>, const N: usize>(
    f: &R,
    x: [Interval; N],
) -> Result<Verdict<N>, NewtonError> {
    krawczyk_in::<InDoubles, R, N>(f, x)
}

/// One step of Hansen and Sengupta's operator on the box `x` (module documentation).
///
/// # Errors
///
/// [`NewtonError::Singular`] when the midpoint Jacobian cannot be inverted;
/// [`NewtonError::Num`] when the residuals fail or an enclosure overflows.
pub fn hansen_sengupta<R: Residuals<N>, const N: usize>(
    f: &R,
    x: [Interval; N],
) -> Result<Verdict<N>, NewtonError> {
    hansen_sengupta_in::<InDoubles, R, N>(f, x)
}

/// One step of `operator` on the box `x`, with the residuals enclosed in `precision`.
pub(crate) fn step<R: Residuals<N>, const N: usize>(
    operator: Operator,
    precision: Precision,
    f: &R,
    x: [Interval; N],
) -> Result<Verdict<N>, NewtonError> {
    match (operator, precision) {
        (Operator::Krawczyk, Precision::Double) => krawczyk_in::<InDoubles, R, N>(f, x),
        (Operator::Krawczyk, Precision::DoubleDouble) => krawczyk_in::<InBalls, R, N>(f, x),
        (Operator::HansenSengupta, Precision::Double) => {
            hansen_sengupta_in::<InDoubles, R, N>(f, x)
        }
        (Operator::HansenSengupta, Precision::DoubleDouble) => {
            hansen_sengupta_in::<InBalls, R, N>(f, x)
        }
    }
}

/// [`krawczyk`] with the residuals enclosed in the arithmetic `A`.
fn krawczyk_in<A: Arithmetic, R: Residuals<N>, const N: usize>(
    f: &R,
    x: [Interval; N],
) -> Result<Verdict<N>, NewtonError> {
    let Some(lin) = Linear::of::<A, R>(f, x)? else {
        return Ok(Verdict::NoZero);
    };
    let mut image = lin.mid;
    for (((k, m), b), row) in image.iter_mut().zip(lin.mid).zip(lin.b).zip(lin.rest) {
        let mut acc = m.sub(b)?;
        for (r, d) in row.iter().zip(lin.offsets) {
            acc = r.mul_add(d, acc)?;
        }
        *k = acc;
    }
    let inside = image.iter().zip(x).all(|(k, xi)| strictly_inside(*k, xi));
    if inside {
        return Ok(Verdict::Unique {
            enclosure: image,
            contraction: lin.contraction,
        });
    }
    Ok(narrowed(image, x, lin.contraction))
}

/// [`hansen_sengupta`] with the residuals enclosed in the arithmetic `A`.
fn hansen_sengupta_in<A: Arithmetic, R: Residuals<N>, const N: usize>(
    f: &R,
    x: [Interval; N],
) -> Result<Verdict<N>, NewtonError> {
    let Some(lin) = Linear::of::<A, R>(f, x)? else {
        return Ok(Verdict::NoZero);
    };
    let mut d = lin.offsets;
    let mut candidates = lin.mid;
    let mut inside = true;
    for (i, (((row, b), m), (xi, candidate))) in lin
        .a
        .iter()
        .zip(lin.b)
        .zip(lin.mid)
        .zip(x.iter().zip(candidates.iter_mut()))
        .enumerate()
    {
        let mut acc = b.neg();
        let mut diagonal = b;
        for (j, (a, dj)) in row.iter().zip(d).enumerate() {
            if j == i {
                diagonal = *a;
            } else {
                acc = a.neg().mul_add(dj, acc)?;
            }
        }
        let Some(Sign::Positive | Sign::Negative) = diagonal.certain_sign() else {
            inside = false;
            continue;
        };
        let q = acc.div(diagonal)?;
        *candidate = m.add(q)?;
        inside &= strictly_inside(*candidate, *xi);
        for (j, dj) in d.iter_mut().enumerate() {
            if j == i {
                let Some(met) = meet(q, *dj) else {
                    return Ok(Verdict::NoZero);
                };
                *dj = met;
            }
        }
    }
    if inside {
        return Ok(Verdict::Unique {
            enclosure: candidates,
            contraction: lin.contraction,
        });
    }
    let mut image = lin.mid;
    for ((y, m), dj) in image.iter_mut().zip(lin.mid).zip(d) {
        *y = m.add(dj)?;
    }
    Ok(narrowed(image, x, lin.contraction))
}

/// The box an operator's `image` and `x` share, as [`Verdict::Narrowed`], or [`Verdict::NoZero`]
/// when they share nothing in some coordinate.
fn narrowed<const N: usize>(
    image: [Interval; N],
    x: [Interval; N],
    contraction: Real,
) -> Verdict<N> {
    let mut enclosure = x;
    for (e, k) in enclosure.iter_mut().zip(image) {
        let Some(met) = meet(k, *e) else {
            return Verdict::NoZero;
        };
        *e = met;
    }
    Verdict::Narrowed {
        enclosure,
        contraction,
    }
}

/// The linearisation both operators share: steps 1 to 4 of the module documentation.
struct Linear<const N: usize> {
    /// The midpoint `m`, as point intervals.
    mid: [Interval; N],
    /// `X − m`.
    offsets: [Interval; N],
    /// `A = C·J(X)`.
    a: [[Interval; N]; N],
    /// `I − A`.
    rest: [[Interval; N]; N],
    /// `b = C·F(m)`.
    b: [Interval; N],
    /// An upper bound on ‖I − A‖∞.
    contraction: Real,
}

impl<const N: usize> Linear<N> {
    /// The linearisation of `f` over `x`, its residuals enclosed in the arithmetic `A`, or `None`
    /// when `F(X)` excludes zero in some row.
    fn of<A: Arithmetic, R: Residuals<N>>(
        system: &R,
        x: [Interval; N],
    ) -> Result<Option<Linear<N>>, NewtonError> {
        const {
            assert!(
                1 <= N && N <= 4,
                "the fixed-size operators take 1 to 4 unknowns"
            );
        };
        let (values, jac) = A::jacobian(system, x)?;
        let excluded = values
            .iter()
            .any(|v| matches!(v.certain_sign(), Some(Sign::Positive | Sign::Negative)));
        if excluded {
            return Ok(None);
        }
        let mut mid = x;
        for m in &mut mid {
            *m = Interval::from(Real::new(midpoint(*m))?);
        }
        let at_mid = A::residuals(system, mid)?;
        let mut centre = [[0.0; N]; N];
        for (crow, jrow) in centre.iter_mut().zip(jac) {
            for (c, j) in crow.iter_mut().zip(jrow) {
                *c = midpoint(j);
            }
        }
        let inv = inverse(centre).ok_or(NewtonError::Singular)?;
        let zero = Interval::from(Real::ZERO);
        let mut a = [[zero; N]; N];
        let mut b = [zero; N];
        for ((arow, bi), crow) in a.iter_mut().zip(b.iter_mut()).zip(inv) {
            for ((cik, jrow), fm) in crow.iter().zip(jac).zip(at_mid) {
                let cik = Interval::from(Real::new(*cik)?);
                for (aij, j) in arow.iter_mut().zip(jrow) {
                    *aij = cik.mul_add(j, *aij)?;
                }
                *bi = cik.mul_add(fm, *bi)?;
            }
        }
        let one = Interval::from(Real::ONE);
        let mut rest = a;
        let mut contraction = 0.0_f64;
        for (i, row) in rest.iter_mut().enumerate() {
            let mut sum = 0.0_f64;
            for (j, r) in row.iter_mut().enumerate() {
                *r = if i == j { one.sub(*r)? } else { r.neg() };
                // Each sum rounded to nearest is within one step of the exact sum, so the step
                // above it is at least the exact sum: an upper bound, never an underestimate.
                sum = (sum + max2(-r.lo(), r.hi())).next_up();
            }
            contraction = max2(contraction, sum);
        }
        let mut offsets = x;
        for (o, m) in offsets.iter_mut().zip(mid) {
            *o = o.sub(m)?;
        }
        Ok(Some(Linear {
            mid,
            offsets,
            a,
            rest,
            b,
            contraction: Real::new(contraction)?,
        }))
    }
}

/// A double inside `x`, at or next to its centre: `lo/2 + hi/2`, which cannot overflow, moved into
/// `x` when halving a subnormal bound rounded it out (for `x = [2^−1074, 2^−1074]` the sum is 0).
pub(crate) fn midpoint(x: Interval) -> f64 {
    let m = 0.5 * x.lo() + 0.5 * x.hi();
    if m < x.lo() {
        x.lo()
    } else if m > x.hi() {
        x.hi()
    } else {
        m
    }
}

/// The interval `a` and `b` share, or `None` when they share nothing.
fn meet(a: Interval, b: Interval) -> Option<Interval> {
    Interval::new(max2(a.lo(), b.lo()), min2(a.hi(), b.hi())).ok()
}

/// Whether `k` lies in the interior of `x`: strictly above its lower bound and below its upper.
fn strictly_inside(k: Interval, x: Interval) -> bool {
    k.lo() > x.lo() && k.hi() < x.hi()
}

/// Entry `j` of a row; every index used here is below `N`, so the default is never taken.
fn entry<const N: usize>(row: &[f64; N], j: usize) -> f64 {
    row.get(j).copied().unwrap_or(0.0)
}

/// The inverse of `matrix` by Gaussian elimination with partial pivoting, `P·m = L·U` (Golub and Van
/// Loan, algorithm 3.4.1: at step `k` the row at or below `k` with the largest magnitude in column
/// `k`, the first on a tie, is swapped up), then `U·X = L⁻¹·P` solved by back substitution
/// (section 3.1), or `None` when a pivot is zero or an entry of the inverse is not finite.
fn inverse<const N: usize>(matrix: [[f64; N]; N]) -> Option<[[f64; N]; N]> {
    let mut u = matrix;
    let mut rhs = [[0.0; N]; N];
    for (i, row) in rhs.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            if i == j {
                *v = 1.0;
            }
        }
    }
    for k in 0..N {
        let mut pivot = (k, 0.0);
        for (r, row) in u.iter().enumerate().skip(k) {
            let size = entry(row, k).abs();
            if size > pivot.1 {
                pivot = (r, size);
            }
        }
        if pivot.1 == 0.0 {
            return None;
        }
        swap(&mut u, k, pivot.0);
        swap(&mut rhs, k, pivot.0);
        let (urow, rrow) = (row(&u, k), row(&rhs, k));
        let diagonal = entry(&urow, k);
        for (ur, rr) in u.iter_mut().zip(rhs.iter_mut()).skip(k).skip(1) {
            let factor = entry(ur, k) / diagonal;
            for (v, w) in ur.iter_mut().zip(urow) {
                *v -= factor * w;
            }
            for (v, w) in rr.iter_mut().zip(rrow) {
                *v -= factor * w;
            }
        }
    }
    let mut solved = rhs;
    for k in (0..N).rev() {
        let urow = row(&u, k);
        let mut acc = row(&rhs, k);
        for (j, sj) in solved.iter().enumerate().skip(k).skip(1) {
            let ukj = entry(&urow, j);
            for (a, v) in acc.iter_mut().zip(sj) {
                *a -= ukj * v;
            }
        }
        let diagonal = entry(&urow, k);
        for a in &mut acc {
            *a /= diagonal;
        }
        for (i, si) in solved.iter_mut().enumerate() {
            if i == k {
                *si = acc;
            }
        }
    }
    solved
        .iter()
        .flatten()
        .all(|v| v.is_finite())
        .then_some(solved)
}

/// Row `i` of `m`, copied; every index used here is below `N`, so the default is never taken.
fn row<const N: usize>(m: &[[f64; N]; N], i: usize) -> [f64; N] {
    m.get(i).copied().unwrap_or([0.0; N])
}

/// `m` with rows `k` and `p` exchanged (nothing changes when they are equal).
fn swap<const N: usize>(m: &mut [[f64; N]; N], k: usize, p: usize) {
    let (rk, rp) = (row(m, k), row(m, p));
    for (i, r) in m.iter_mut().enumerate() {
        if i == k {
            *r = rp;
        } else if i == p {
            *r = rk;
        }
    }
}
