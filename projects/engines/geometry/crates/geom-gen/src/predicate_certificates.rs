//! The problems that machine-check a predicate filter's error bound (chapter 28.6 of the kernel
//! architecture; piece G35): the filter's claim stated for Gappa, whose proof Rocq checks, and for
//! `FPTaylor`, whose certificate HOL Light checks.
//!
//! What it does: a generated filter computes the description's steps in `f64` on the coordinate
//! differences, the scale S from the axis maxima, and the bound `fl(C · scale)`, and answers when
//! the computed value is beyond the bound. [`gappa_problem`] states exactly that computation in
//! Gappa's binary64 with rounding to nearest, beside the same steps evaluated exactly, on
//! whole-number differences bounded by their axis maxima, and asks Gappa to prove that the error is
//! within K · S and the computed bound within a relative error R of C · S, with K = C · (1 − R):
//! together, |error| ≤ K · S ≤ bound, so an answer has the exact sign. [`fptaylor_problem`] states
//! the normalised problem for `FPTaylor`, every difference in [−1, 1] so that S is 1, with the same K
//! as the bound HOL Light compares the certificate's with. Both problems carry Gappa's hints and
//! HOL Light's bound in their text, so the tools' runs need nothing else.
//!
//! Guarantees:
//!
//! - **The shipped steps.** Each problem holds one rounded definition per operation of the filter,
//!   in the generated code's order, with its operands and operation, and the constant and the
//!   scale's product order as the code has them; `tests/certificate_problems.rs` holds the
//!   committed problems to the committed filters statement by statement.
//! - **Exact numbers.** R = (k + 1)u, for the k roundings between the axis maxima and the bound,
//!   bounds the bound's relative error ((1 + u)^k − 1 < (k + 1)u for u = 2^−53), and K = C · (1 −
//!   R) is computed exactly from the constant's bits: both are dyadic, written exactly.
//! - **Refused, never approximated.** A filter the problems cannot state yet, one with rounded
//!   inputs, a scale of several monomials, or a coordinate loaded without a translation, is refused
//!   with the reason; piece G40 extends them to every filter.
//!
//! What the problems leave to the written argument (the method note,
//! `projects/kit/notes/G35-filter-certificates.md`): that the translations are exact (the bit-range
//! certificate bounds them by 2^41 − 2 < 2^53), that no value overflows (it bounds every step
//! below 2^127), and that a zero axis maximum makes the exact value zero.
//!
//! Method: Melquiond and Pion's certified semi-static filters, Gappa proofs of a homogeneous
//! predicate's error as a multiple of the product of its inputs' maxima (*Formally certified
//! floating-point filters for homogeneous geometric predicates*, RAIRO Theoretical Informatics and
//! Applications 41 (2007), doi:10.1051/ita:2007005), with Gappa's `LIN` predicate (`a // b`) in
//! place of their special rounding operators, which current Gappa no longer has; and `FPTaylor`'s
//! symbolic Taylor forms on the normalised problem (Solovyev et al., *Rigorous Estimation of
//! Floating-Point Round-off Errors with Symbolic Taylor Expansions*, ACM TOPLAS 41 (2018),
//! doi:10.1145/3230733).
//!
//! Cost: linear in the description's steps; no allocation beyond the text.

use crate::predicate_derive::{Derivation, Monomial, ScaleExpr};
use crate::predicate_emit::{DESCRIPTIONS_PATH, EmitError};
use crate::predicate_lang::{Op, StepKind, max_name};

/// The path of this module, for the headers.
pub const CERTIFICATES_PATH: &str =
    "projects/engines/geometry/crates/geom-gen/src/predicate_certificates.rs";

fn fail<T>(message: impl Into<String>) -> Result<T, EmitError> {
    Err(EmitError(message.into()))
}

/// A line to `out`.
fn line(out: &mut String, text: &str) {
    out.push_str(text);
    out.push('\n');
}

/// What both problems need from a derivation: the step names, which steps are the coordinate
/// differences (and their groups), the result's single scale monomial, and the dyadic K and R.
struct Shape<'d> {
    d: &'d Derivation,
    /// For each step, the group of a coordinate difference, or `None` for an operation.
    group_of: Vec<Option<usize>>,
    /// The result's scale monomial.
    monomial: Monomial,
    /// The roundings between the maxima and the bound.
    roundings: u32,
    /// K = numerator · 2^−exponent.
    k: (u128, u32),
}

/// The shape of `d`, or why the problems cannot state its filter yet.
fn shape(d: &Derivation) -> Result<Shape<'_>, EmitError> {
    let mut group_of = vec![None; d.steps.len()];
    for (g, group) in d.groups.iter().enumerate() {
        for &v in &group.vars {
            match (d.steps.get(v).map(|s| &s.kind), group_of.get_mut(v)) {
                (Some(StepKind::Translate { .. }), Some(slot)) => *slot = Some(g),
                _ => {
                    return fail(format!(
                        "group `{}`'s scale variable is not a translation of grid points; the \
                         problems state translations only (piece G40 extends them)",
                        group.name
                    ));
                }
            }
        }
    }
    for (step, g) in d.steps.iter().zip(&group_of) {
        match (&step.kind, g) {
            (StepKind::Op { .. }, None) | (StepKind::Translate { .. }, Some(_)) => {}
            _ => {
                return fail(format!(
                    "step `{}` is a load or a translation outside every group; the problems state \
                     translations of grid points and operations only (piece G40 extends them)",
                    step.name
                ));
            }
        }
    }
    let Some(result) = d.certs.get(d.result) else {
        return fail("the result step is missing");
    };
    let mut monomials = result.scale.iter();
    let (Some(monomial), None) = (monomials.next(), monomials.next()) else {
        return fail(
            "the result's scale has several monomials, which a single LIN goal cannot state \
             (piece G40 extends the problems)",
        );
    };
    let roundings = d.roundings;
    let k = dyadic_k(d.constant_bits, roundings)?;
    Ok(Shape {
        d,
        group_of,
        monomial: monomial.clone(),
        roundings,
        k,
    })
}

/// K = C · (1 − (k + 1) · 2^−53) for the constant C with `bits`, exactly, as a numerator and a
/// power-of-two exponent: C = M · 2^−p for a normal C (M = 2^52 + fraction, p = 1075 − exponent
/// field), so K = M · (2^53 − k − 1) · 2^−(p + 53), with M · (2^53 − k − 1) < 2^106.
fn dyadic_k(bits: u64, roundings: u32) -> Result<(u128, u32), EmitError> {
    let field = bits >> 52;
    if bits >> 63 != 0 || field == 0 || field >= 0x7ff {
        return fail("the filter's constant is not a positive normal f64");
    }
    let fraction = bits & ((1u64 << 52) - 1);
    let m = u128::from((1u64 << 52) | fraction);
    let Some(p) = 1075u32.checked_sub(u32::try_from(field).unwrap_or(u32::MAX)) else {
        return fail("the filter's constant is 2^22 or more, far above any error bound");
    };
    let factor = (1u128 << 53)
        .checked_sub(u128::from(roundings))
        .and_then(|f| f.checked_sub(1));
    let (Some(mut numerator), Some(mut exponent)) =
        (factor.and_then(|f| m.checked_mul(f)), p.checked_add(53))
    else {
        return fail("K = C · (1 − R) does not fit 128 bits");
    };
    // The same number with the fewest bits.
    while numerator != 0 && numerator.trailing_zeros() > 0 && exponent > 0 {
        numerator >>= 1;
        exponent = exponent.saturating_sub(1);
    }
    Ok((numerator, exponent))
}

/// A positive dyadic `numerator · 2^−exponent` in Gappa's syntax, `<numerator>b-<exponent>`.
fn gappa_dyadic((numerator, exponent): (u128, u32)) -> String {
    format!("{numerator}b-{exponent}")
}

/// The constant's bits as a hexadecimal float, `0x1.<fraction>p<exponent>`, exactly: Gappa and
/// `FPTaylor` both read it.
fn hex_float(bits: u64) -> String {
    let fraction = bits & ((1u64 << 52) - 1);
    let field = bits >> 52;
    // The field is 11 bits, so the exponent is between -1023 and 1024.
    let exponent = i64::try_from(field).unwrap_or(0).saturating_sub(1023);
    let mut hex = format!("{fraction:013x}");
    while hex.len() > 1 && hex.ends_with('0') {
        hex.pop();
    }
    format!("0x1.{hex}p{exponent}")
}

/// The product of the maxima in a monomial, in group order, left to right: `m_x * m_y * m_z`.
fn canonical(monomial: &Monomial, d: &Derivation) -> Result<String, EmitError> {
    let mut factors = Vec::new();
    for (group, &e) in d.groups.iter().zip(monomial) {
        for _ in 0..e {
            factors.push(max_name(&group.name));
        }
    }
    if factors.is_empty() {
        return fail("a scale monomial is 1, which no filter of a predicate on points has");
    }
    Ok(factors.join(" * "))
}

/// The scale as the filter computes it, with each product rounded: `rnd(rnd(m_x * m_y) * m_z)`.
fn scale_rounded(e: &ScaleExpr, d: &Derivation) -> Result<String, EmitError> {
    Ok(match e {
        ScaleExpr::Max(g) => match d.groups.get(*g) {
            Some(group) => max_name(&group.name),
            None => return fail("the scale names a missing group"),
        },
        ScaleExpr::Mul(l, r) => format!("rnd({} * {})", scale_rounded(l, d)?, scale_rounded(r, d)?),
        ScaleExpr::One | ScaleExpr::Larger(..) => {
            return fail("the scale is 1 or a larger-of, which the problems do not state yet");
        }
    })
}

/// The scale each step's error is a multiple of, as Gappa's rules form it from the operands'
/// (a product's is the product of its operands', in their order) and as the problem's goal writes
/// it (canonical, in group order): the two differ by the order of the factors, which hints bridge.
fn natural_scales(s: &Shape<'_>) -> Result<Vec<String>, EmitError> {
    let d = s.d;
    let mut natural: Vec<String> = Vec::with_capacity(d.steps.len());
    for ((step, cert), group) in d.steps.iter().zip(&d.certs).zip(&s.group_of) {
        let text = match (&step.kind, group) {
            (_, Some(g)) => match d.groups.get(*g) {
                Some(group) => max_name(&group.name),
                None => return fail("a step names a missing group"),
            },
            (
                StepKind::Op {
                    op: Op::Mul,
                    left,
                    right,
                },
                None,
            ) => {
                let operand = |i: &usize| -> Result<String, EmitError> {
                    let Some(t) = natural.get(*i) else {
                        return fail("an operand follows its step");
                    };
                    Ok(if t.contains(' ') {
                        format!("({t})")
                    } else {
                        t.clone()
                    })
                };
                format!("{} * {}", operand(left)?, operand(right)?)
            }
            (_, None) => {
                // A sum or difference: both operands are brought to the canonical scale.
                let mut monomials = cert.scale.iter();
                let (Some(m), None) = (monomials.next(), monomials.next()) else {
                    return fail(format!(
                        "step `{}` has a scale of several monomials (piece G40)",
                        step.name
                    ));
                };
                canonical(m, d)?
            }
        };
        natural.push(text);
    }
    Ok(natural)
}

/// The generated-file header both problems open with, in their comment syntax.
fn header(out: &mut String, comment: &str, generator: &str, d: &Derivation, about: &[&str]) {
    let name = &d.description.name;
    let lines = [
        format!(
            "Generated by geom-gen, generator `{generator}` (seed: none, it uses no randomness), \
             from the"
        ),
        format!("description of `{name}` in `{DESCRIPTIONS_PATH}`,"),
        format!("by `{CERTIFICATES_PATH}`."),
        "Do not edit: change the description or the generator and run `cargo run -p geom-gen -- \
         write`."
            .to_owned(),
        String::new(),
    ];
    for l in lines
        .iter()
        .map(String::as_str)
        .chain(about.iter().copied())
    {
        if l.is_empty() {
            line(out, comment);
        } else {
            line(out, &format!("{comment} {l}"));
        }
    }
}

impl Shape<'_> {
    /// The names of a step's two operands, computed or (with `exact`) exact: an exact operation's
    /// name is `e_` and its own; a coordinate difference is exact either way.
    fn operands(
        &self,
        left: usize,
        right: usize,
        exact: bool,
    ) -> Result<(String, String), EmitError> {
        let pick = |i: usize| -> Result<String, EmitError> {
            let (Some(step), Some(group)) = (self.d.steps.get(i), self.group_of.get(i)) else {
                return fail("an operand is missing");
            };
            Ok(if exact && group.is_none() {
                format!("e_{}", step.name)
            } else {
                step.name.clone()
            })
        };
        Ok((pick(left)?, pick(right)?))
    }
}

/// The Gappa problem of `d`'s filter (module documentation), as text.
///
/// # Errors
///
/// [`EmitError`] if the filter is not one the problems state yet (module documentation).
pub fn gappa_problem(d: &Derivation, generator: &str) -> Result<String, EmitError> {
    let sh = shape(d)?;
    let name = &d.description.name;
    let mut out = String::new();
    header(
        &mut out,
        "#",
        generator,
        d,
        &[
            &format!(
                "{name}'s f64 filter (geom_num::generated::{name}::filter) for Gappa (chapter 28.6;"
            ),
            "piece G35). Gappa proves the goal below for binary64 with rounding to nearest, on",
            "whole-number coordinate differences, each at most its axis maximum in magnitude, and",
            "writes a proof that Rocq checks. The goal: the computed value is within K * S of the",
            "exact one, and the computed bound within a relative error R of C * S, where",
            "K = C * (1 - R); so whenever the filter answers, its sign is the exact one.",
        ],
    );
    gappa_definitions(&mut out, &sh)?;
    gappa_formula(&mut out, &sh)?;
    gappa_hints(&mut out, &sh)?;
    Ok(out)
}

/// The rounding operator, the filter's operations as computed and exactly, the scale, the
/// constant and the bound.
fn gappa_definitions(out: &mut String, sh: &Shape<'_>) -> Result<(), EmitError> {
    let d = sh.d;
    line(out, "");
    line(out, "@rnd = float<ieee_64, ne>;");
    line(out, "");
    line(
        out,
        "# The filter's operations, as the generated code computes them.",
    );
    for step in &d.steps {
        if let StepKind::Op { op, left, right } = &step.kind {
            let (lhs, rhs) = sh.operands(*left, *right, false)?;
            line(
                out,
                &format!("{} = rnd({lhs} {} {rhs});", step.name, op.symbol()),
            );
        }
    }
    line(out, "");
    line(out, "# The same operations, exactly.");
    for step in &d.steps {
        if let StepKind::Op { op, left, right } = &step.kind {
            let (lhs, rhs) = sh.operands(*left, *right, true)?;
            line(
                out,
                &format!("e_{} = {lhs} {} {rhs};", step.name, op.symbol()),
            );
        }
    }
    line(out, "");
    line(
        out,
        "# The scale S, the scale as the filter computes it, the constant C and the bound.",
    );
    line(out, &format!("S = {};", canonical(&sh.monomial, d)?));
    line(
        out,
        &format!("scale = {};", scale_rounded(&d.scale_expr, d)?),
    );
    line(out, &format!("C = {};", hex_float(d.constant_bits)));
    line(out, "bound = rnd(C * scale);");
    line(out, "");
    Ok(())
}

/// The logical formula: whole-number differences and maxima, each maximum at least 1 and at most
/// the differences' bound, each difference at most its axis maximum in magnitude; then the goal.
fn gappa_formula(out: &mut String, sh: &Shape<'_>) -> Result<(), EmitError> {
    let d = sh.d;
    let mut hypotheses: Vec<String> = Vec::new();
    for (step, g) in d.steps.iter().zip(&sh.group_of) {
        if g.is_some() {
            hypotheses.push(format!("@FIX({}, 0)", step.name));
        }
    }
    for group in &d.groups {
        let m = max_name(&group.name);
        let Some(bound) = group
            .vars
            .iter()
            .filter_map(|v| d.certs.get(*v))
            .find_map(|c| c.bound.to_decimal())
        else {
            return fail(format!("group `{}` has no scale variable", group.name));
        };
        hypotheses.push(format!("@FIX({m}, 0)"));
        hypotheses.push(format!("{m} in [1, {bound}]"));
    }
    for (step, g) in d.steps.iter().zip(&sh.group_of) {
        if let Some(group) = g.and_then(|g| d.groups.get(g)) {
            hypotheses.push(format!(
                "{} // {} in [-1, 1]",
                step.name,
                max_name(&group.name)
            ));
        }
    }
    let Some(result) = d.steps.get(d.result) else {
        return fail("the result step is missing");
    };
    line(out, "{");
    let count = hypotheses.len();
    for (i, h) in hypotheses.iter().enumerate() {
        let tail = if i.saturating_add(1) == count {
            ""
        } else {
            " /\\"
        };
        line(out, &format!("  {h}{tail}"));
    }
    line(out, "->");
    let error = gappa_dyadic(sh.k);
    let relative = format!("{}b-53", sh.roundings.saturating_add(1));
    line(
        out,
        &format!(
            "  {res} - e_{res} // S in [-{error}, {error}] /\\ bound -/ C * S in \
             [-{relative}, {relative}]",
            res = result.name
        ),
    );
    line(out, "}");
    line(out, "");
    Ok(())
}

/// The hints: each product's error as its operands' errors times the other operand, then each
/// order of a product of maxima tied to the canonical one.
fn gappa_hints(out: &mut String, sh: &Shape<'_>) -> Result<(), EmitError> {
    let d = sh.d;
    line(
        out,
        "# The error of a product as its operands' errors times the other operand.",
    );
    for step in &d.steps {
        if let StepKind::Op {
            op: Op::Mul,
            left,
            right,
        } = &step.kind
        {
            let (a, b) = sh.operands(*left, *right, false)?;
            let (ea, eb) = sh.operands(*left, *right, true)?;
            let hint = match (a == ea, b == eb) {
                (true, true) => continue,
                (true, false) => format!("{a} * {b} - {a} * {eb} -> {a} * ({b} - {eb});"),
                (false, true) => format!("{a} * {b} - {ea} * {b} -> ({a} - {ea}) * {b};"),
                (false, false) => {
                    format!("{a} * {b} - {ea} * {eb} -> {a} * ({b} - {eb}) + ({a} - {ea}) * {eb};")
                }
            };
            line(out, &hint);
        }
    }
    line(out, "");
    line(
        out,
        "# Products of maxima are one scale whatever the order of their factors; Gappa's rules",
    );
    line(
        out,
        "# match scales as written, so each order a step forms is tied to the canonical one.",
    );
    let natural = natural_scales(sh)?;
    let mut seen: Vec<(String, String)> = Vec::new();
    for ((step, cert), nat) in d.steps.iter().zip(&d.certs).zip(&natural) {
        if !matches!(step.kind, StepKind::Op { op: Op::Mul, .. }) {
            continue;
        }
        let mut monomials = cert.scale.iter();
        let (Some(m), None) = (monomials.next(), monomials.next()) else {
            return fail(format!(
                "step `{}` has several scale monomials (piece G40)",
                step.name
            ));
        };
        let canon = canonical(m, d)?;
        if *nat != canon && !seen.iter().any(|(a, b)| *a == *nat && *b == canon) {
            line(out, &format!("{nat} -> {canon};"));
            line(out, &format!("{canon} -> {nat};"));
            seen.push((nat.clone(), canon));
        }
    }
    Ok(())
}

/// The `FPTaylor` problem of `d`'s filter (module documentation), as text: the normalised problem
/// and, in a structured comment, the bound HOL Light compares the certificate's with.
///
/// # Errors
///
/// [`EmitError`] if the filter is not one the problems state yet (module documentation).
pub fn fptaylor_problem(d: &Derivation, generator: &str) -> Result<String, EmitError> {
    let sh = shape(d)?;
    let name = &d.description.name;
    let (numerator, exponent) = sh.k;
    let mut out = String::new();
    header(
        &mut out,
        "//",
        generator,
        d,
        &[
            &format!(
                "{name}'s f64 filter (geom_num::generated::{name}::filter) for FPTaylor (chapter"
            ),
            "28.6; piece G35), normalised: each coordinate difference divided by its axis maximum,",
            "so every variable is in [-1, 1] and the scale S is 1. The error model",
            "fl(t) = t(1 + e) is homogeneous, so a bound B here bounds the error by B * S at every",
            "scale (the method note). HOL Light checks FPTaylor's certificate, and compares its",
            "bound with K below.",
        ],
    );
    line(
        &mut out,
        &format!("// hol-light-bound: &{numerator} * &2 ipow (-- &{exponent})"),
    );
    line(&mut out, "");
    line(&mut out, "Variables");
    for (step, g) in d.steps.iter().zip(&sh.group_of) {
        if g.is_some() {
            line(&mut out, &format!("  float64 {} in [-1, 1];", step.name));
        }
    }
    line(&mut out, "");
    line(&mut out, "Definitions");
    let ops: Vec<_> = d
        .steps
        .iter()
        .enumerate()
        .filter_map(|(i, step)| match &step.kind {
            StepKind::Op { op, left, right } => Some((i, step, *op, *left, *right)),
            _ => None,
        })
        .collect();
    for &(i, step, op, left, right) in &ops {
        if i == d.result {
            continue;
        }
        let (Some(lhs), Some(rhs)) = (d.steps.get(left), d.steps.get(right)) else {
            return fail("an operand is missing");
        };
        line(
            &mut out,
            &format!(
                "  {} = rnd64({} {} {});",
                step.name,
                lhs.name,
                op.symbol(),
                rhs.name
            ),
        );
    }
    line(&mut out, "");
    line(&mut out, "Expressions");
    let Some(&(_, step, op, left, right)) = ops.iter().find(|(i, ..)| *i == d.result) else {
        return fail("the result is not an operation");
    };
    let (Some(lhs), Some(rhs)) = (d.steps.get(left), d.steps.get(right)) else {
        return fail("an operand is missing");
    };
    line(
        &mut out,
        &format!(
            "  {} = rnd64({} {} {});",
            step.name,
            lhs.name,
            op.symbol(),
            rhs.name
        ),
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_constants_round_trip() {
        // orient3d's constant, 7881299347898375 * 2^-101.
        assert_eq!(hex_float(0x3cec_0000_0000_0007), "0x1.c000000000007p-49");
        assert_eq!(hex_float(0x3cc0_0000_0000_0003), "0x1.0000000000003p-51");
        // K = C * (1 - 4u) for orient3d, exactly.
        let (n, e) = dyadic_k(0x3cec_0000_0000_0007, 3).unwrap_or((0, 0));
        assert_eq!(
            (n, e),
            (7_881_299_347_898_375 * ((1u128 << 53) - 4) / 4, 152)
        );
    }
}
