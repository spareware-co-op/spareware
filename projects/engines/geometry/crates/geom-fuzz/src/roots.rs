//! Fuzz target: the root engine misses no root and proves none twice, and Brent's method brackets
//! its zero, on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a polynomial in one unknown,
//! the product of x − rᵢ for up to [`MAX_ROOTS`] roots on a grid of quarters (repeated roots
//! included), a domain, the Newton operator to use, and a bracket's half-width. [`check`] holds
//! three laws, decided exactly from the roots (the sign of the polynomial at a double is the
//! product of the signs of x − rᵢ, each an exact comparison):
//!
//! - **The engine misses no root and proves none twice** (`geom_num::isolate`, piece G53, with
//!   piece G52's interval Newton operators): every zero it returns holds a root, and every simple
//!   root strictly inside the domain lies in exactly one returned zero or in a flagged region.
//! - **Brent's zero is exact or brackets a sign change** no wider than 4u|x| + tol
//!   (`geom_num::brent::zero`, piece G51).
//! - **Quadrature answers or refuses** (`geom_num::quadrature`, G51): no panic and no hang on the
//!   polynomial over the domain, whose stated exactness ("up to the rounding of the rule's entries
//!   and of the sum") gives no number to check here; its own tests check it against exact
//!   integrals.
//!
//! [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Roots are quarters in [−8, 8], so repeated roots, roots on the
//!   domain's ends and roots on the engine's split lines come up often.
//! - **A panic is a failure; a broken law is a [`Finding`]**; every typed error is legal.
//! - **No hang.** Each call runs under a work meter of [`WORK`] units, or an evaluation cap.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//!
//! Method: the engine's and Brent's stated guarantees (`geom_num::isolate`, `geom_num::brent`),
//! checked against the exact roots, as `geom-num`'s `tests/attack_g53.rs` does on seeded systems,
//! driven by coverage-guided fuzzing (chapter 28.5,
//! `projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: one isolation, one Brent search and one quadrature of a polynomial of degree at most
//! [`MAX_ROOTS`], each bounded; at most [`MAX_INPUT_BYTES`] bytes read.

// The lint is for fixed-width arithmetic in kernel code. Here it is products of signs (each −1,
// 0 or 1), byte folds below 256, and the oracle's IEEE doubles.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "products of signs, byte folds below 256, and IEEE doubles in the oracle"
)]

use geom_num::NumError;
use geom_num::brent;
use geom_num::interval::Interval;
use geom_num::isolate::{Options, System, isolate};
use geom_num::newton::{Operator, Residuals};
use geom_num::quadrature::gauss_legendre;
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most roots a case has.
pub const MAX_ROOTS: usize = 5;

/// The work units the engine may spend.
pub const WORK: u64 = 2_000_000;

/// The most bytes [`decode`] reads from any input: 10.
///
/// A count byte, [`MAX_ROOTS`] root bytes, two domain bytes, an operator byte and a half-width
/// byte: 1 + 5 + 2 + 1 + 1 = 10. The unit test `the_input_bound_is_reached_and_never_passed`
/// builds an input that reads exactly this many.
pub const MAX_INPUT_BYTES: usize = 1 + MAX_ROOTS + 4;

/// One input.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// The roots, quarters in [−8, 8].
    pub roots: Vec<f64>,
    /// The domain, quarters in [−8, 8], `lo < hi` (else the engine refuses it).
    pub domain: (f64, f64),
    /// Whether the engine uses the Hansen–Sengupta operator, else Krawczyk's.
    pub hansen: bool,
    /// The half-width of Brent's bracket around the first root, a quarter step or more.
    pub half: f64,
}

/// A quarter in [−8, 8] from a byte.
fn quarter(b: u8) -> f64 {
    f64::from(i16::from(b) % 65 - 32) / 4.0
}

/// Any bytes as a case, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(bytes);
    let n = usize::from(src.byte()) % MAX_ROOTS + 1;
    let roots = (0..n).map(|_| quarter(src.byte())).collect();
    let (a, b) = (quarter(src.byte()), quarter(src.byte()));
    let hansen = src.byte() & 1 == 1;
    let half = f64::from(src.byte() % 16 + 1) / 8.0;
    let read = bytes.len().saturating_sub(src.remaining());
    (
        Case {
            roots,
            domain: if a < b { (a, b) } else { (b, a) },
            hansen,
            half,
        },
        read,
    )
}

/// The polynomial Π (x − rᵢ) of the roots given, in any scalar.
pub struct Product<'a>(pub &'a [f64]);

impl Residuals<1> for Product<'_> {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let [x] = x;
        let mut acc = S::constant(Real::new(1.0)?);
        for &r in self.0 {
            acc = acc.mul(x.sub(S::constant(Real::new(r)?))?)?;
        }
        Ok([acc])
    }
}

impl System<1, 1> for Product<'_> {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok(x)
    }
}

/// The exact sign of Π (x − rᵢ) at x: −1, 0 or 1.
fn sign(roots: &[f64], x: f64) -> i32 {
    roots.iter().fold(1, |s, &r| {
        s * if x > r {
            1
        } else if x < r {
            -1
        } else {
            0
        }
    })
}

fn found(law: &str, detail: impl FnOnce() -> String) -> Result<(), Finding> {
    Err(Finding::new(law, detail))
}

/// The three laws on the case (module documentation).
///
/// # Errors
///
/// A [`Finding`] for the first law broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    let p = Product(&case.roots);
    let (lo, hi) = case.domain;

    // The engine misses no root and proves none twice.
    if let Ok(domain) = Interval::new(lo, hi) {
        let options = Options {
            operator: if case.hansen {
                Operator::HansenSengupta
            } else {
                Operator::Krawczyk
            },
            ..Options::DEFAULT
        };
        let mut meter = WorkMeter::new(Budget::of_work(WORK));
        if let Ok(found_) = isolate(&p, [domain], options, &mut meter) {
            let holds = |b: &[Interval; 1], r: f64| b.iter().all(|i| i.lo() <= r && r <= i.hi());
            for z in &found_.zeros {
                if !case.roots.iter().any(|&r| holds(&z.enclosure, r)) {
                    return found("every zero the engine proves holds a root", || {
                        format!("{case:?}: {z:?}")
                    });
                }
            }
            for &r in &case.roots {
                let simple = case.roots.iter().filter(|&&s| s == r).count() == 1;
                if !(simple && lo < r && r < hi) {
                    continue;
                }
                let zeros = found_
                    .zeros
                    .iter()
                    .filter(|z| holds(&z.enclosure, r))
                    .count();
                let flagged = found_.flagged.iter().any(|f| holds(&f.region, r));
                if zeros > 1 || (zeros == 0 && !flagged) {
                    return found(
                        "every simple root lies in exactly one zero or a flagged region",
                        || {
                            format!(
                                "{case:?}: root {r} in {zeros} zeros, flagged {flagged}: {found_:?}"
                            )
                        },
                    );
                }
            }
        }
    }

    // Brent's zero is exact or brackets a sign change no wider than 4u|x| + tol.
    if let Some(&r) = case.roots.first()
        && let (Ok(a), Ok(b), Ok(tol)) = (
            Real::new(r - case.half),
            Real::new(r + case.half),
            Real::new(1.0 / 1024.0),
        )
    {
        let f = |x: Real| p.residuals([x]).map(|[y]| y);
        if let Ok(z) = brent::zero(f, a, b, tol, 200) {
            let (x, other) = (z.x.get(), z.other.get());
            let width_ok = (x - other).abs() <= 4.0 * f64::EPSILON / 2.0 * x.abs() + 1.0 / 1024.0;
            let exact = sign(&case.roots, x) == 0;
            let bracket = sign(&case.roots, x) * sign(&case.roots, other) <= 0 && width_ok;
            if !(exact || bracket) {
                return found(
                    "Brent's zero is exact or brackets a sign change within 4u|x| + tol",
                    || format!("{case:?}: {z:?}"),
                );
            }
        }
    }

    // Quadrature answers or refuses.
    if let (Ok(a), Ok(b)) = (Real::new(lo), Real::new(hi)) {
        let f = |x: Real| p.residuals([x]).map(|[y]| y);
        let _ = gauss_legendre(f, a, b, MAX_ROOTS);
    }
    Ok(())
}

/// [`decode`], then [`check`].
///
/// # Errors
///
/// As [`check`].
pub fn harness(bytes: &[u8]) -> Result<(), Finding> {
    check(&decode(bytes).0)
}
