//! The derivation behind each generated predicate: the bit-range certificate of every
//! intermediate, the width of the exact path, and the filter's error constant, all exact.
//!
//! What it does: [`derive`] takes a description ([`crate::predicate_lang`]) and returns a
//! [`Derivation`]: for every step, a [`StepCert`] (the bound on its exact value over the input
//! domain, its bits, the integer type the exact path computes it in, and the filter's scale,
//! unit bound and unit error for it), the scale expression the filter evaluates at run time, the
//! number of roundings in it, and the error constant, exactly and as the `f64` the filter uses.
//!
//! Guarantees, each by the proof below:
//!
//! 1. **The certificate.** For every input in the domain (grid coordinates |v| ≤ 2^40 − 1 =
//!    `geom_num::grid::COORD_MAX`; rounded inputs |v| ≤ 2^bits − 1), every step's exact value v
//!    has |v| ≤ `bound` < 2^`bits`, and its exact-path type holds every value of that magnitude.
//! 2. **The filter.** With M(g) the largest |x̃| over the computed scale variables x of group g,
//!    S the largest product Π M(g)^e(g) over the monomials e of the result's scale, and ṽ the
//!    result computed in `f64` in the steps' order: |ṽ − v| ≤ E · S, where E is the result's
//!    unit error. The generated filter computes S in `f64`, at least S · (1 − u)^r with r =
//!    [`ScaleExpr::roundings`] (every product's rounding and its operands', since relative
//!    errors multiply), and multiplies it by the constant, one more rounding: k =
//!    [`Derivation::roundings`] = r + 1. The constant is at least E · (1 + 2u)^k, so the computed
//!    bound is at least E · S, and a computed value beyond it in magnitude has the exact value's
//!    sign. If S is zero the exact value is zero.
//! 3. **No overflow, no underflow, no NaN.** Every value the filter computes is a whole number
//!    (inputs are whole numbers or `f64` copies of them, and the correctly rounded sum or product
//!    of whole numbers is whole), so none is subnormal; and every computed magnitude, and the
//!    bound, is checked here to stay below 2^1000, far from `f64::MAX`.
//!
//! Proof. Binary64 arithmetic with rounding to nearest, ties to even, is exact to the standard
//! on every target Rust supports, and Rust never fuses a multiply and an add (RFC 3514). For a
//! sum or product z of two finite `f64`s whose rounded value fl(z) is normal or zero,
//! |fl(z) − z| ≤ u·|z| with u = 2^-53, and fl(z) = z when z is a whole number with |z| ≤ 2^53
//! (the standard model of Higham, *Accuracy and Stability of Numerical Algorithms*, second
//! edition, section 2.2). The argument is short: in the binade [2^e, 2^(e+1)) neighbouring `f64`s
//! are 2^(e − 52) apart, so rounding to nearest moves a value there by at most 2^(e − 53), which
//! is at most u times the value and at most u times its rounded value (which lies in
//! [2^e, 2^(e+1)]); and every whole number up to 2^53 is an `f64`. The same argument gives the
//! bound for an `f64` copy x̃ of a whole number x rounded to nearest: |x̃ − x| ≤ u·|x̃|.
//!
//! Each step v has a scale set S(v) of monomials over the groups, and S(v) also denotes the
//! largest Π M(g)^e(g) over them. By induction over the steps, |ṽ| ≤ B(v) · S(v) and
//! |ṽ − v| ≤ E(v) · S(v), with B(v) and E(v) polynomials in u (the unit bound and unit error):
//!
//! - A scale variable of group g (a translation p.k − q.k, or an input loaded by itself) has
//!   S = {g}, so |ṽ| ≤ M(g): B = 1. A coordinate is a whole number with |v| ≤ 2^40 − 1 ≤ 2^53,
//!   an `f64` exactly (and [`derive`] refuses a grid whose `COORD_MAX` passes 2^53, rather than
//!   assume it): E = 0. A translation of two grid coordinates is a whole number with
//!   |v| ≤ 2 · (2^40 − 1) ≤ 2^53, computed exactly: E = 0 (were its bound past 2^53, its one
//!   rounding would give E = u, as below). A rounded input has E = u. (FPG's translation filter starts the same way, with error ulp(1)/2 because its
//!   inputs are arbitrary doubles; on the grid the translation is exact.)
//! - A sum or difference: S = S(a) ∪ S(b), so S(a), S(b) ≤ S; the operands' sum is at most
//!   P = B(a) + B(b) times S. If both operands are computed exactly and the certificate bounds the
//!   exact value by 2^53, the step is exact: B = P, E = 0. Otherwise the rounding adds u·P:
//!   E = E(a) + E(b) + u·P and B = P·(1 + u).
//! - A product: S = S(a) · S(b) (every pairwise sum of exponents; all M(g) ≥ 0, so the largest
//!   product is the product of the largest), P = B(a) · B(b). With ã = a + α and b̃ = b + β,
//!   ãb̃ − ab = ãβ + αb̃ − αβ, so the propagated error is E(a)·B(b) + B(a)·E(b) + E(a)·E(b). Exact as
//!   for a sum, else E = E(a)·B(b) + B(a)·E(b) + E(a)·E(b) + u·P and B = P·(1 + u).
//!
//! This is Meyer and Pion's forward error analysis at unit input bounds, scaled at run time by
//! the group maxima, with their `Sfe` rules (bound and error of a sum and a product; FPG
//! appendix B), their groups and their "alternatives" (here, a set of monomials whose largest
//! value is taken at run time), computed exactly instead of in upward-rounded floating point. For
//! the bound at run time, by induction over the scale expression, its computed value is at least
//! its exact value times (1 − u)^r: a group maximum is exact (r = 0); a product of non-negative
//! values computed within (1 − u)^a and (1 − u)^b of theirs is within (1 − u)^(a + b) before its
//! own rounding, which loses at most one more factor (1 − u), so r = a + b + 1 (the operands'
//! losses multiply: counting only the longest path would undercount `m_x · m_y` times a rounded
//! square, and a test shows grid maxima where that scale falls more than (1 − u)^2 below its
//! exact value); and the larger of two such values is at least (1 − u)^max(a, b) times the
//! larger exact value, so r = max(a, b). The product with the constant rounds once more,
//! k = r + 1 in all, and (1 − u)(1 + 2u) ≥ 1, so a constant of at least E·(1 + 2u)^k makes the
//! computed bound at least E·S. The constant is the smallest `f64` at least that exact value.
//! Finally, if the computed S is zero then some M(g) in each monomial is zero (a product of whole
//! numbers below 2^1000 that are not zero is not zero), so S is exactly zero and
//! |v| ≤ |ṽ| + E·S ≤ (B + E)·S = 0.
//!
//! The certificate's bounds follow Fortune and Van Wyk's static analysis (*Static Analysis Yields
//! Efficient Exact Integer Arithmetic for Computational Geometry*, ACM Transactions on Graphics
//! 15(3), 1996, <https://doi.org/10.1145/231731.231735>), with exact values instead of bit
//! counts: |a ± b| ≤ |a| + |b| and |ab| = |a||b|, so the bound of each step is the sum or product
//! of its operands' bounds, starting from the domain's. Every bound is a whole number computed
//! with [`crate::bignum::Nat`]; bits is the least b with bound < 2^b, and the exact path computes
//! the step in the narrowest of `i64`, `i128`, `I256`, `I320`, `I384` and `I512` whose magnitude
//! limit 2^(width − 1) − 1 is at least the bound. `GeometricTools`' `BSPrecision` derives widths from an
//! expression in the same spirit (Boost licence;
//! <https://github.com/davideberly/GeometricTools/blob/e09f5b39a33da6b4bfd0ff8dbb64ddcb7fdfc763/GTE/Mathematics/BSPrecision.h>).
//!
//! Method: Meyer and Pion, *FPG: A code generator for fast and certified geometric predicates*,
//! Real Numbers and Computers 2008 (<https://inria.hal.science/inria-00344297>), as Geogram's PCK
//! uses it (Lévy, *Robustness and efficiency of geometric programs: the Predicate Construction
//! Kit (PCK)*, Computer-Aided Design 72, 2016, <https://doi.org/10.1016/j.cad.2015.10.004>;
//! generated headers such as
//! <https://github.com/BrunoLevy/geogram/blob/16843b4804de35f14ef54667950ab4c8acdd0062/src/lib/geogram/numerics/predicates/orient3d.h>,
//! BSD-3-Clause, read for method; no code is ported). Shewchuk's stage-A bounds
//! (<https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c>, `exactinit`) are the
//! comparison in the generated headers.
//!
//! Cost: linear in the number of steps, times the size of the numbers (a few limbs, a few dozen
//! coefficients) and of the scale sets (a few monomials).

use std::collections::BTreeSet;

use geom_num::wide::{I256, I320, I384, I512};

use crate::bignum::{Nat, U_EXPONENT, UPoly};
use crate::predicate_lang::{Axis, Description, InputKind, Op, Step, StepKind, parse};

/// The integer type the exact path computes a step in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Width {
    /// `i64`.
    I64,
    /// `i128`.
    I128,
    /// `I256`, four limbs.
    I256,
    /// `I320`, five limbs.
    I320,
    /// `I384`, six limbs.
    I384,
    /// `I512`, eight limbs.
    I512,
}

impl Width {
    /// Every width, narrowest first.
    pub const ALL: [Width; 6] = [
        Width::I64,
        Width::I128,
        Width::I256,
        Width::I320,
        Width::I384,
        Width::I512,
    ];

    /// The type's name in Rust.
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Width::I64 => "i64",
            Width::I128 => "i128",
            Width::I256 => "I256",
            Width::I320 => "I320",
            Width::I384 => "I384",
            Width::I512 => "I512",
        }
    }

    /// The number of 64-bit limbs, from `geom-num`'s types themselves (1 for `i64`, 2 for
    /// `i128`).
    #[must_use]
    pub fn limbs(self) -> usize {
        match self {
            Width::I64 => 1,
            Width::I128 => 2,
            Width::I256 => I256::ZERO.to_le_limbs().len(),
            Width::I320 => I320::ZERO.to_le_limbs().len(),
            Width::I384 => I384::ZERO.to_le_limbs().len(),
            Width::I512 => I512::ZERO.to_le_limbs().len(),
        }
    }

    /// The bits of the largest magnitude it holds: every |v| ≤ 2^bits − 1 fits (64 · limbs − 1).
    #[must_use]
    pub fn magnitude_bits(self) -> Option<u32> {
        match self {
            Width::I64 => i64::BITS.checked_sub(1),
            Width::I128 => i128::BITS.checked_sub(1),
            other => u32::try_from(other.limbs())
                .ok()?
                .checked_mul(64)?
                .checked_sub(1),
        }
    }
}

/// A monomial over the groups: the exponent of each group's maximum, in group order.
pub type Monomial = Vec<u32>;

/// The certificate and error analysis of one step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepCert {
    /// |exact value| ≤ this, over the whole input domain.
    pub bound: Nat,
    /// The least b with bound < 2^b.
    pub bits: u32,
    /// The exact path's type for it.
    pub width: Width,
    /// The filter's scale set.
    pub scale: BTreeSet<Monomial>,
    /// Whether the filter computes it exactly.
    pub f64_exact: bool,
    /// |computed| ≤ `unit_bound` · S.
    pub unit_bound: UPoly,
    /// |computed − exact| ≤ `unit_error` · S.
    pub unit_error: UPoly,
}

/// A group of the filter's scale and its scale variables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Its name: an axis letter, or a rounded input's group.
    pub name: String,
    /// Its scale variables, as step indices, in step order.
    pub vars: Vec<usize>,
}

/// How the filter computes the scale S at run time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScaleExpr {
    /// The maximum M(g) of a group, by index.
    Max(usize),
    /// 1.
    One,
    /// A product: one rounding.
    Mul(Box<ScaleExpr>, Box<ScaleExpr>),
    /// The larger of two values: exact.
    Larger(Box<ScaleExpr>, Box<ScaleExpr>),
}

impl ScaleExpr {
    /// The multiplications the filter performs to compute it: one per product.
    #[must_use]
    pub fn multiplications(&self) -> Option<u32> {
        match self {
            ScaleExpr::Max(_) | ScaleExpr::One => Some(0),
            ScaleExpr::Mul(l, r) => l
                .multiplications()?
                .checked_add(r.multiplications()?)?
                .checked_add(1),
            ScaleExpr::Larger(l, r) => l.multiplications()?.checked_add(r.multiplications()?),
        }
    }

    /// The roundings r for which the computed value is at least the exact one times (1 − u)^r
    /// (module documentation): a product's own rounding plus both its operands' counts, since
    /// their relative errors multiply; the larger of two values, the larger of its operands'
    /// counts.
    #[must_use]
    pub fn roundings(&self) -> Option<u32> {
        match self {
            ScaleExpr::Max(_) | ScaleExpr::One => Some(0),
            ScaleExpr::Mul(l, r) => l.roundings()?.checked_add(r.roundings()?)?.checked_add(1),
            ScaleExpr::Larger(l, r) => Some(l.roundings()?.max(r.roundings()?)),
        }
    }

    /// The monomials whose largest value this computes, exactly.
    #[must_use]
    pub fn monomials(&self, groups: usize) -> BTreeSet<Monomial> {
        match self {
            ScaleExpr::Max(g) => {
                let mut m = vec![0u32; groups];
                if let Some(e) = m.get_mut(*g) {
                    *e = 1;
                }
                BTreeSet::from([m])
            }
            ScaleExpr::One => BTreeSet::from([vec![0u32; groups]]),
            ScaleExpr::Mul(l, r) => product(&l.monomials(groups), &r.monomials(groups)),
            ScaleExpr::Larger(l, r) => {
                let mut s = l.monomials(groups);
                s.extend(r.monomials(groups));
                s
            }
        }
    }
}

/// Everything derived for one predicate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Derivation {
    /// The description's text.
    pub text: String,
    /// The parsed description.
    pub description: Description,
    /// The steps, in evaluation order.
    pub steps: Vec<Step>,
    /// Each step's certificate, in the same order.
    pub certs: Vec<StepCert>,
    /// The result's step.
    pub result: usize,
    /// The groups, axes first (x, y, z), then rounded inputs' groups in declaration order.
    pub groups: Vec<Group>,
    /// How the filter computes S.
    pub scale_expr: ScaleExpr,
    /// The roundings between the group maxima and the computed bound: the scale's, plus one for
    /// the product with the constant.
    pub roundings: u32,
    /// The constant, exactly: the result's unit error times (1 + 2u)^roundings.
    pub constant: UPoly,
    /// The bits of the constant as the filter's `f64`: the smallest `f64` at least `constant`,
    /// or 0 (the value 0.0) if the constant is zero.
    pub constant_bits: u64,
}

/// Why a derivation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeriveError(pub String);

impl core::fmt::Display for DeriveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

fn fail<T>(message: impl Into<String>) -> Result<T, DeriveError> {
    Err(DeriveError(message.into()))
}

/// An arithmetic step that cannot fail for the generator's numbers did fail.
fn overflow(what: &str) -> DeriveError {
    DeriveError(format!("exact arithmetic overflowed computing {what}"))
}

/// 2^1000: every computed magnitude and the computed bound stay below it.
const FINITE_LIMIT_LOG2: u32 = 1_000;

/// Derives everything for the description `text`.
///
/// # Errors
///
/// A [`DeriveError`] if the description does not parse, or a step needs a wider type than
/// `I512`, or a computed value or the bound could come near `f64::MAX`, or exact arithmetic
/// overflows (which no description of the kernel's size causes).
pub fn derive(text: &str) -> Result<Derivation, DeriveError> {
    grid_is_exact_in_f64(Description::coord_max())?;
    let description = parse(text).map_err(|e| DeriveError(e.to_string()))?;
    let steps = description
        .steps()
        .map_err(|e| DeriveError(e.to_string()))?;
    let result = description
        .result_step(&steps)
        .map_err(|e| DeriveError(e.to_string()))?;
    let groups = groups(&description, &steps)?;
    let mut certs: Vec<StepCert> = Vec::with_capacity(steps.len());
    for step in &steps {
        let cert = certify(&description, step, &certs, &groups)?;
        certs.push(cert);
    }
    let Some(root) = certs.get(result) else {
        return fail("the result step is missing");
    };
    let scale_expr = scale_expr(&root.scale, groups.len())?;
    let roundings = scale_expr
        .roundings()
        .and_then(|k| k.checked_add(1))
        .ok_or_else(|| overflow("the roundings"))?;
    let growth = UPoly::from_coeffs(vec![1, 2])
        .pow(roundings)
        .ok_or_else(|| overflow("(1 + 2u)^k"))?;
    let constant = root
        .unit_error
        .mul(&growth)
        .ok_or_else(|| overflow("the constant"))?;
    let constant_bits = if constant.is_zero() {
        0
    } else {
        constant
            .round_up_to_f64()
            .ok_or_else(|| DeriveError("the constant is not a normal f64".to_owned()))?
    };
    let derivation = Derivation {
        text: text.to_owned(),
        description,
        steps,
        certs,
        result,
        groups,
        scale_expr,
        roundings,
        constant,
        constant_bits,
    };
    check_finite(&derivation)?;
    Ok(derivation)
}

/// The grid's coordinates are `f64`s exactly, |v| ≤ `coord_max` ≤ 2^53, as the filter's loads of
/// coordinates (`exact_f64`, with E = 0) and translations (exact operands) assume. If the grid ever
/// grew past that, the generator would stop here rather than write an unsound filter.
fn grid_is_exact_in_f64(coord_max: i64) -> Result<(), DeriveError> {
    let coord_max = u128::try_from(coord_max).map_err(|_| overflow("COORD_MAX"))?;
    if Nat::from_u128(coord_max) > exact_limit()? {
        return fail(format!(
            "grid coordinates up to {coord_max} are not all f64s exactly (2^53 is the limit)"
        ));
    }
    Ok(())
}

/// The group of a scale variable step, if it is one.
fn group_of(description: &Description, step: &Step) -> Option<String> {
    match &step.kind {
        StepKind::LoadCoord { axis, .. } | StepKind::Translate { axis, .. } => {
            Some(axis.letter().to_owned())
        }
        StepKind::LoadRounded { input } => match description.input(input).map(|i| &i.kind) {
            Some(InputKind::Rounded { group, .. }) => Some(group.clone()),
            _ => None,
        },
        StepKind::Op { .. } => None,
    }
}

/// The groups in their order, with their scale variables.
fn groups(description: &Description, steps: &[Step]) -> Result<Vec<Group>, DeriveError> {
    let mut order: Vec<String> = [Axis::X, Axis::Y, Axis::Z]
        .iter()
        .map(|a| a.letter().to_owned())
        .collect();
    for input in &description.inputs {
        if let InputKind::Rounded { group, .. } = &input.kind {
            if !order.contains(group) {
                order.push(group.clone());
            }
        }
    }
    let mut out = Vec::new();
    for name in order {
        let vars: Vec<usize> = steps
            .iter()
            .enumerate()
            .filter(|(_, s)| group_of(description, s).as_deref() == Some(name.as_str()))
            .map(|(i, _)| i)
            .collect();
        if !vars.is_empty() {
            out.push(Group { name, vars });
        }
    }
    if out.is_empty() {
        return fail("the description has no scale variable");
    }
    Ok(out)
}

/// The narrowest width holding every magnitude up to `bound`.
fn width_for(bits: u32) -> Result<Width, DeriveError> {
    for w in Width::ALL {
        if bits <= w.magnitude_bits().ok_or_else(|| overflow("a width"))? {
            return Ok(w);
        }
    }
    fail(format!("a step needs {bits} bits, more than I512 holds"))
}

/// The monomial of a single group's maximum.
fn unit_monomial(groups: &[Group], name: &str) -> Result<BTreeSet<Monomial>, DeriveError> {
    let Some(index) = groups.iter().position(|g| g.name == name) else {
        return fail(format!("no group `{name}`"));
    };
    let mut m = vec![0u32; groups.len()];
    if let Some(e) = m.get_mut(index) {
        *e = 1;
    }
    Ok(BTreeSet::from([m]))
}

/// Every pairwise product of two monomial sets.
fn product(a: &BTreeSet<Monomial>, b: &BTreeSet<Monomial>) -> BTreeSet<Monomial> {
    let mut out = BTreeSet::new();
    for x in a {
        for y in b {
            out.insert(x.iter().zip(y).map(|(p, q)| p.saturating_add(*q)).collect());
        }
    }
    out
}

/// 2^53: whole numbers up to it in magnitude are `f64`s.
fn exact_limit() -> Result<Nat, DeriveError> {
    Nat::pow2(U_EXPONENT).ok_or_else(|| overflow("2^53"))
}

/// The certificate of one step, from those before it.
fn certify(
    description: &Description,
    step: &Step,
    before: &[StepCert],
    groups: &[Group],
) -> Result<StepCert, DeriveError> {
    let coord_max = u128::try_from(Description::coord_max()).map_err(|_| overflow("COORD_MAX"))?;
    let limit = exact_limit()?;
    let leaf = |bound: Nat, exact: bool| -> Result<StepCert, DeriveError> {
        let bits = bound.bit_len().ok_or_else(|| overflow("bits"))?;
        let group = group_of(description, step).ok_or_else(|| overflow("a group"))?;
        let f64_exact = exact && bound <= limit;
        Ok(StepCert {
            width: width_for(bits)?,
            bits,
            bound,
            scale: unit_monomial(groups, &group)?,
            f64_exact,
            unit_bound: UPoly::constant(1),
            unit_error: if f64_exact { UPoly::zero() } else { UPoly::u() },
        })
    };
    match &step.kind {
        StepKind::LoadCoord { .. } => leaf(Nat::from_u128(coord_max), true),
        StepKind::Translate { .. } => {
            let bound = Nat::from_u128(coord_max)
                .add(&Nat::from_u128(coord_max))
                .ok_or_else(|| overflow("a translation's bound"))?;
            leaf(bound, true)
        }
        StepKind::LoadRounded { input } => {
            let Some(InputKind::Rounded { bits, .. }) = description.input(input).map(|i| &i.kind)
            else {
                return fail(format!("`{input}` is not a rounded input"));
            };
            let bound = Nat::ones(*bits).ok_or_else(|| overflow("a rounded input's bound"))?;
            leaf(bound, false)
        }
        StepKind::Op { op, left, right } => {
            let (Some(a), Some(b)) = (before.get(*left), before.get(*right)) else {
                return fail(format!("step `{}` uses a later step", step.name));
            };
            let (bound, scale, proposed) = match op {
                Op::Add | Op::Sub => {
                    let mut scale = a.scale.clone();
                    scale.extend(b.scale.iter().cloned());
                    (
                        a.bound.add(&b.bound),
                        scale,
                        a.unit_bound.add(&b.unit_bound),
                    )
                }
                Op::Mul => (
                    a.bound.mul(&b.bound),
                    product(&a.scale, &b.scale),
                    a.unit_bound.mul(&b.unit_bound),
                ),
            };
            let bound = bound.ok_or_else(|| overflow("a bound"))?;
            let p = proposed.ok_or_else(|| overflow("a unit bound"))?;
            let bits = bound.bit_len().ok_or_else(|| overflow("bits"))?;
            let f64_exact = a.f64_exact && b.f64_exact && bound <= limit;
            let (unit_bound, unit_error) = if f64_exact {
                (p, UPoly::zero())
            } else {
                let propagated = match op {
                    Op::Add | Op::Sub => a.unit_error.add(&b.unit_error),
                    Op::Mul => a
                        .unit_error
                        .mul(&b.unit_bound)
                        .and_then(|x| x.add(&a.unit_bound.mul(&b.unit_error)?))
                        .and_then(|x| x.add(&a.unit_error.mul(&b.unit_error)?)),
                }
                .ok_or_else(|| overflow("a unit error"))?;
                let error = propagated
                    .add(&p.times_u())
                    .ok_or_else(|| overflow("a unit error"))?;
                let grown = p
                    .times_one_plus_u()
                    .ok_or_else(|| overflow("a unit bound"))?;
                (grown, error)
            };
            Ok(StepCert {
                width: width_for(bits)?,
                bits,
                bound,
                scale,
                f64_exact,
                unit_bound,
                unit_error,
            })
        }
    }
}

/// The scale expression for a scale set: the common factor of every monomial, times the largest
/// of what remains of each (module header).
fn scale_expr(scale: &BTreeSet<Monomial>, groups: usize) -> Result<ScaleExpr, DeriveError> {
    let Some(first) = scale.iter().next() else {
        return fail("an empty scale");
    };
    let mut common = first.clone();
    for m in scale {
        for (c, e) in common.iter_mut().zip(m) {
            *c = (*c).min(*e);
        }
    }
    let residuals: Vec<Monomial> = scale
        .iter()
        .map(|m| {
            m.iter()
                .zip(&common)
                .map(|(e, c)| e.saturating_sub(*c))
                .collect()
        })
        .collect();
    let chain = |m: &Monomial| -> Option<ScaleExpr> {
        let mut out: Option<ScaleExpr> = None;
        for (g, &e) in m.iter().enumerate() {
            for _ in 0..e {
                out = Some(match out {
                    None => ScaleExpr::Max(g),
                    Some(prev) => ScaleExpr::Mul(Box::new(prev), Box::new(ScaleExpr::Max(g))),
                });
            }
        }
        out
    };
    let largest = if residuals.len() > 1 {
        let mut out: Option<ScaleExpr> = None;
        for r in &residuals {
            let term = chain(r).unwrap_or(ScaleExpr::One);
            out = Some(match out {
                None => term,
                Some(prev) => ScaleExpr::Larger(Box::new(prev), Box::new(term)),
            });
        }
        out
    } else {
        None
    };
    let expr = match (chain(&common), largest) {
        (Some(c), Some(l)) => ScaleExpr::Mul(Box::new(c), Box::new(l)),
        (Some(c), None) => c,
        (None, Some(l)) => l,
        (None, None) => ScaleExpr::One,
    };
    if &expr.monomials(groups) != scale {
        return fail("the scale expression does not compute the scale set");
    }
    Ok(expr)
}

/// Every computed magnitude and the computed bound stay below 2^1000 (guarantee 3).
fn check_finite(d: &Derivation) -> Result<(), DeriveError> {
    let limit = Nat::pow2(FINITE_LIMIT_LOG2).ok_or_else(|| overflow("2^1000"))?;
    // The largest computed value of each group's scale variables.
    let mut maxima: Vec<Nat> = Vec::new();
    for g in &d.groups {
        let mut m = Nat::zero();
        for &v in &g.vars {
            let Some(cert) = d.certs.get(v) else {
                return fail("a scale variable is missing");
            };
            // A rounded copy can round up to the next power of two.
            let computed = if cert.f64_exact {
                cert.bound.clone()
            } else {
                Nat::pow2(cert.bits).ok_or_else(|| overflow("a power of two"))?
            };
            if computed > m {
                m = computed;
            }
        }
        maxima.push(m);
    }
    let largest = |scale: &BTreeSet<Monomial>| -> Result<Nat, DeriveError> {
        let mut best = Nat::zero();
        for mono in scale {
            let mut v = Nat::from_u128(1);
            for (m, &e) in maxima.iter().zip(mono) {
                for _ in 0..e {
                    v = v.mul(m).ok_or_else(|| overflow("a scale"))?;
                }
            }
            if v > best {
                best = v;
            }
        }
        Ok(best)
    };
    for (step, cert) in d.steps.iter().zip(&d.certs) {
        let b = cert
            .unit_bound
            .at_one()
            .ok_or_else(|| overflow("a unit bound at one"))?;
        let v = Nat::from_u128(b)
            .mul(&largest(&cert.scale)?)
            .ok_or_else(|| overflow("a magnitude"))?;
        if v >= limit {
            return fail(format!("step `{}` could reach 2^1000 in f64", step.name));
        }
    }
    let Some(root) = d.certs.get(d.result) else {
        return fail("the result step is missing");
    };
    let c = d
        .constant
        .at_one()
        .ok_or_else(|| overflow("the constant at one"))?;
    let bound = Nat::from_u128(c)
        .mul(&largest(&root.scale)?)
        .ok_or_else(|| overflow("the bound"))?;
    if bound >= limit {
        return fail("the filter's bound could reach 2^1000");
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    const ORIENT: &str = "predicate orient2d\n\
        input a b c: point2\n\
        let ba_x = b.x - a.x\n\
        let ba_y = b.y - a.y\n\
        let ca_x = c.x - a.x\n\
        let ca_y = c.y - a.y\n\
        let det = ba_x * ca_y - ba_y * ca_x\n\
        sign det\n";

    #[test]
    fn orient2d_by_hand() {
        let d = derive(ORIENT).unwrap();
        let det = &d.certs[d.result];
        // 2 · (2^41 − 2)^2 = 2^83 − 2^43 + 8: 83 bits.
        assert_eq!(det.bits, 83);
        assert_eq!(det.width, Width::I128);
        // The translations, 41 bits, in `i64`; the products of two of them in `i128`.
        assert_eq!(d.certs[0].width, Width::I64);
        assert_eq!(d.scale_expr.multiplications(), Some(1));
        assert_eq!(det.unit_error.to_string(), "4u + 2u^2");
        assert_eq!(d.roundings, 2);
        assert_eq!(d.scale_expr.roundings(), Some(1));
        // C = (4u + 2u^2)(1 + 2u)^2 = 4u + 18u^2 + 24u^3 + 8u^4.
        assert_eq!(d.constant.to_string(), "4u + 18u^2 + 24u^3 + 8u^4");
        let c = f64::from_bits(d.constant_bits);
        assert!(c > 4.0 * f64::EPSILON / 2.0 && c < 4.000_001 * f64::EPSILON / 2.0);
    }

    #[test]
    fn a_lifted_scale_takes_the_largest_square() {
        let d = derive(
            "predicate lifted\ninput a b: point2\nlet dx = a.x - b.x\nlet dy = a.y - b.y\n\
             let l = dx * dx + dy * dy\nlet v = l * dx * dy\nsign v\n",
        )
        .unwrap();
        let root = &d.certs[d.result];
        assert_eq!(root.scale, BTreeSet::from([vec![3, 1], vec![1, 3]]));
        // m_x · m_y · larger(m_x · m_x, m_y · m_y): three roundings carried into the result (the
        // outer product's, m_x · m_y's and the larger square's), and four multiplications in all.
        assert_eq!(d.scale_expr.roundings(), Some(3));
        assert_eq!(d.scale_expr.multiplications(), Some(4));
        assert_eq!(d.roundings, 4);
    }

    /// The f64 value of a scale expression, as the generated filter computes it.
    fn computed(expr: &ScaleExpr, maxima: &[f64]) -> f64 {
        match expr {
            ScaleExpr::Max(group) => maxima[*group],
            ScaleExpr::One => 1.0,
            ScaleExpr::Mul(left, right) => computed(left, maxima) * computed(right, maxima),
            ScaleExpr::Larger(left, right) => {
                let (first, second) = (computed(left, maxima), computed(right, maxima));
                if first < second { second } else { first }
            }
        }
    }

    /// The exact value of a scale expression.
    fn exactly(expr: &ScaleExpr, maxima: &[u128]) -> Nat {
        match expr {
            ScaleExpr::Max(group) => Nat::from_u128(maxima[*group]),
            ScaleExpr::One => Nat::from_u128(1),
            ScaleExpr::Mul(left, right) => {
                exactly(left, maxima).mul(&exactly(right, maxima)).unwrap()
            }
            ScaleExpr::Larger(left, right) => exactly(left, maxima).max(exactly(right, maxima)),
        }
    }

    /// A whole `f64` as a natural number.
    fn whole(v: f64) -> Nat {
        let bits = v.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as u32;
        let significand = (bits & ((1 << 52) - 1)) | (1 << 52);
        Nat::from_u128(u128::from(significand))
            .shl(exponent - 1075)
            .unwrap()
    }

    /// A product's roundings carry both operands' (relative errors multiply), not only the
    /// longest path's. For in-circle's run-time scale `m_x * m_y * larger(m_y * m_y, m_x * m_x)`,
    /// these maxima, translations of grid coordinates (below 2^41 − 2), give a computed scale more
    /// than (1 − u)^2 below the exact one, which a count of the longest path (two) would not
    /// allow, and within (1 − u)^3, the count here.
    #[test]
    fn a_products_roundings_carry_both_operands() {
        let d = derive(crate::predicates::INCIRCLE).unwrap();
        assert_eq!(d.scale_expr.roundings(), Some(3));
        assert_eq!(d.roundings, 4);
        let (mx, my) = (1_156_899_745_123u128, 1_163_609_645_421u128);
        let translation_max = 2 * geom_num::grid::COORD_MAX as u128;
        assert!(mx <= translation_max && my <= translation_max);
        let s = computed(&d.scale_expr, &[mx as f64, my as f64]);
        let exact = exactly(&d.scale_expr, &[mx, my]);
        assert_eq!(
            exact,
            Nat::from_u128(mx * my)
                .mul(&Nat::from_u128((mx * mx).max(my * my)))
                .unwrap()
        );
        // s < exact · (1 − u)^2, as s · 2^106 < exact · (2^53 − 1)^2; and
        // s ≥ exact · (1 − u)^3, as s · 2^159 ≥ exact · (2^53 − 1)^3.
        let one_less = Nat::from_u128((1 << 53) - 1);
        let s = whole(s);
        let two = exact.mul(&one_less).unwrap().mul(&one_less).unwrap();
        assert!(s.shl(106).unwrap() < two);
        assert!(s.shl(159).unwrap() >= two.mul(&one_less).unwrap());
    }

    /// A grid whose coordinates are not all `f64`s exactly is refused: the loads and translations
    /// of the filter assume they are.
    #[test]
    fn a_grid_past_two_to_the_53_is_refused() {
        assert!(grid_is_exact_in_f64((1 << 40) - 1).is_ok());
        assert!(grid_is_exact_in_f64(1 << 53).is_ok());
        let e = grid_is_exact_in_f64((1 << 53) + 1).unwrap_err();
        assert!(e.0.contains("not all f64s exactly"), "{e}");
    }

    #[test]
    fn widths_follow_the_bits() {
        assert_eq!(width_for(41).unwrap(), Width::I64);
        assert_eq!(width_for(63).unwrap(), Width::I64);
        assert_eq!(width_for(64).unwrap(), Width::I128);
        assert_eq!(width_for(127).unwrap(), Width::I128);
        assert_eq!(width_for(128).unwrap(), Width::I256);
        assert_eq!(width_for(255).unwrap(), Width::I256);
        assert_eq!(width_for(256).unwrap(), Width::I320);
        assert_eq!(width_for(383).unwrap(), Width::I384);
        assert_eq!(width_for(511).unwrap(), Width::I512);
        assert!(width_for(512).is_err());
    }

    #[test]
    fn rounded_inputs_carry_their_error() {
        let d = derive(
            "predicate side\ninput n: rounded 83 in n\ninput d: rounded 125 in d\n\
             input p: point3\nlet v = n * p.x + d\nsign v\n",
        )
        .unwrap();
        let n = &d.certs[0];
        assert_eq!(n.unit_error, UPoly::u());
        assert_eq!(n.bits, 83);
        assert_eq!(
            d.groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            ["x", "n", "d"]
        );
    }
}
