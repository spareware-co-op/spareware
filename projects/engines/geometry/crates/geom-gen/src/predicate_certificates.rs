//! The problems that machine-check a predicate filter's error bound (chapter 28.6 of the kernel
//! architecture; pieces G35 and G40): the filter's claim stated for Gappa, whose proof Rocq checks,
//! and for `FPTaylor`, whose certificate HOL Light checks; and the label each generated module
//! carries, proven or "checked, not proven".
//!
//! What it does: a generated filter computes the description's steps in `f64` on its inputs
//! (coordinates, their differences, and the `f64` copies of wider integers), the scale S from the
//! groups' maxima, and the bound `fl(C · scale)`, and answers when the computed value is beyond the
//! bound. [`gappa_problem`] states exactly that computation in Gappa's binary64 with rounding to
//! nearest, beside the same steps evaluated exactly, on whole-number inputs bounded by their
//! groups' maxima, and asks Gappa to prove that the error is within K · S and the computed bound
//! within a relative error R of C · S, with K = C · (1 − R): together, |error| ≤ K · S ≤ bound, so
//! an answer has the exact sign. Where the result's scale has several monomials (the larger of
//! `m_x^3 m_y` and `m_x m_y^3`, say), one problem cannot state it: [`gappa_case_problem`] states
//! one problem per case ([`gappa_cases`]), the case where one group's maximum is the largest when
//! the monomials share one degree, and otherwise where one monomial is the largest of the result's,
//! each with S that monomial and the hypotheses the case implies. [`fptaylor_problem`] states the
//! normalised problem for `FPTaylor`, each step divided by its scale S(v), the largest of its
//! monomials at the group maxima, so every input is in [−1, 1] and the result's scale is 1, with
//! a ratio parameter in [0, 1] where a sum's operand has a smaller scale set; one problem covers
//! every input, with the same K, which HOL Light compares the certificate's bound with.
//! [`proof_sentence`] is the label: proven, naming the problems, when the Gappa problems state the
//! filter in every case, and otherwise "checked, not proven" with the open issue [`UNPROVEN`]
//! gives it. Every problem carries Gappa's hints and HOL Light's bound in its text, so the tools'
//! runs need nothing else.
//!
//! Guarantees:
//!
//! - **The shipped steps.** Each problem holds one rounded definition per operation of the filter,
//!   in the generated code's order, with its operands and operation, and the constant and the
//!   scale's product order as the code has them; `tests/certificate_problems.rs` holds the
//!   committed problems to the committed filters statement by statement.
//! - **Every input covered, nothing assumed that fails.** A filter's cases together cover every
//!   input, one problem for each way its scale's larger-ofs resolve, and each hypothesis holds for
//!   every input of its case; `tests/certificate_problems.rs` justifies each one independently.
//! - **Exact numbers.** R = (k + 1)u, for the k roundings between the maxima and the bound, bounds
//!   the bound's relative error ((1 + u)^k − 1 < (k + 1)u for u = 2^−53), and K = C · (1 − R) is
//!   computed exactly from the constant's bits: both are dyadic, written exactly. A rounded input's
//!   largest copy is its bound rounded to the nearest `f64`, exactly ([`crate::bignum::Nat`]).
//! - **Refused, never approximated.** A filter the Gappa problems cannot state (a scale term formed
//!   from more than two inputs, say) is refused with the reason, and its label says "checked, not
//!   proven" with its issue; with no issue in [`UNPROVEN`] its module cannot be generated.
//! - **Ratios, never more.** Each ratio parameter of an `FPTaylor` problem is S(operand) / S(sum)
//!   for the pairs of scale sets named in the problem's header, each the same ratio once their
//!   common factor is cancelled, and every use of it is one of those pairs: the problem's box
//!   holds every input's normalised values, never fewer.
//!
//! What the problems leave to the written argument (the method note,
//! `projects/kit/notes/G35-filter-certificates.md`): that the translations are exact (the bit-range
//! certificate bounds them by 2^41 − 2 < 2^53), that no value overflows (it bounds every step
//! below 2^127), that a zero S makes the exact value zero, that a rounded input's copy, written
//! `rndx` (53 bits, no exponent bounds), is binary64's rounding of a whole number below 2^1024,
//! and that `FPTaylor`'s normalised problem bounds the filter's error at every scale (each step's
//! value over its scale is the problem's value, at the ratios the maxima give; a parameter two
//! pairs share has one value, since a common factor cancels unless it is zero, and then that
//! pair's sum is zero, whatever the parameter).
//!
//! Method: Melquiond and Pion's certified semi-static filters, Gappa proofs of a homogeneous
//! predicate's error as a multiple of the product of its inputs' maxima (*Formally certified
//! floating-point filters for homogeneous geometric predicates*, RAIRO Theoretical Informatics and
//! Applications 41 (2007), <https://doi.org/10.1051/ita:2007005>), with Gappa's `LIN` predicate
//! (`a // b`) in place of their special rounding operators, which current Gappa no longer has, and
//! a case split for a scale that takes a larger-of; and `FPTaylor`'s symbolic Taylor forms on the
//! normalised problem (Solovyev et al., *Rigorous Estimation of Floating-Point Round-off Errors
//! with Symbolic Taylor Expansions*, ACM TOPLAS 41 (2018), <https://doi.org/10.1145/3230733>).
//!
//! Cost: linear in the description's steps times the ways its scale's larger-ofs resolve (one per
//! case); no allocation beyond the text.

use std::collections::BTreeSet;

use crate::predicate_derive::{Derivation, Monomial, ScaleExpr};
use crate::predicate_emit::{DESCRIPTIONS_PATH, EmitError, scale_text};
use crate::predicate_lang::{Op, StepKind, max_name};

/// The path of this module, for the headers.
pub const CERTIFICATES_PATH: &str =
    "projects/engines/geometry/crates/geom-gen/src/predicate_certificates.rs";

/// The folder the problems are written into, as the headers name it.
pub const PROBLEMS_DIR: &str = "projects/engines/geometry/crates/geom-gen/generated";

/// Each filter the Gappa problems cannot state yet, by predicate name, with its open issue: its
/// module says "checked, not proven" and names the issue. A term of these filters' scales is
/// formed from more than two inputs beside terms of other groups (`d4 * w` against
/// `n4x * mx_x`), so no hypothesis on the inputs, or on products of two, bounds it by the case's
/// S; Gappa's `LIN` rules relate scales only through products of maxima, and the ratio of two
/// such terms needs a division by a maximum that may be zero. The issue sets out the way through.
pub const UNPROVEN: [(&str, u32); 9] = [
    ("tpi_side", 292),
    ("lpi_side", 292),
    ("round_grid", 292),
    ("round_rational", 292),
    ("round_line", 292),
    ("circle_side", 292),
    ("circle_line", 292),
    ("ellipse_side", 292),
    ("ellipse_line", 292),
];

/// The filters whose `FPTaylor` problem bounds the sum of its error terms at once (`opt-exact`),
/// by predicate name; every other bounds each error term on its own (`opt-approx`). HOL Light
/// checks a bound on each term as one inequality per term, and the sum as one inequality over
/// the whole box, split into cells. Term by term, eight filters checked in at most three minutes
/// and `lpi_side` in 87 (run 40643928468), but the 150 terms of `insphere` and the 142 of
/// `tpi_side` had reached 34 and 69 when their jobs stopped at 350 minutes; the sum for
/// `insphere` checked in 75 minutes (run 40603306636). For these, HOL Light proves the sum at
/// most the largest double at most K, not `FPTaylor`'s own bound: every cell that proves the
/// smaller bound proves the larger, so the larger never needs more cells, and it is the bound
/// the filter needs.
pub const EXACT_OPTIMISATION: [&str; 2] = ["insphere", "tpi_side"];

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
    /// Each step's scale monomial: its only one, or, in a case, the one that bounds the rest.
    monomials: Vec<Monomial>,
    /// The case, for a result whose scale has several monomials.
    case: Option<Case>,
    /// The roundings between the maxima and the bound.
    roundings: u32,
    /// K = numerator · 2^−exponent.
    k: (u128, u32),
}

/// A case of a filter whose result's scale has several monomials (piece G40). The cases of a
/// filter cover every input, one problem each ([`gappa_cases`]).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Case {
    /// The inputs where this group's maximum is the largest, for a scale whose monomials all have
    /// one degree: each step's monomial is then the one [`dominant`] gives.
    Group(usize),
    /// The inputs where this monomial is the largest of the result's, for a scale of mixed
    /// degrees (a plane's offset beside its products): each of the result's other monomials is
    /// bounded by it through the steps that compute it (`gappa_formula`).
    Term(Monomial),
}

/// The degree of a monomial.
fn degree(m: &Monomial) -> u32 {
    m.iter().fold(0u32, |a, e| a.saturating_add(*e))
}

/// The least monomial that is at least every one of `set` whenever group `g`'s maximum is the
/// largest: of the set's degree, with each other group's exponent the least the set has for it
/// and the rest of the degree on `g`. Each member is reached from it by moving exponents off `g`,
/// which cannot increase it in the case. For a product it is the product of its factors' (the
/// least of a sum of exponents is the sum of the least), so the steps' monomials stay consistent.
/// `None` if the set is empty or not of one degree.
fn dominant(set: &BTreeSet<Monomial>, g: usize) -> Option<Monomial> {
    let first = set.iter().next()?;
    let total = degree(first);
    if set.iter().any(|m| degree(m) != total) {
        return None;
    }
    let mut top: Monomial = (0..first.len())
        .map(|h| {
            if h == g {
                0
            } else {
                set.iter()
                    .filter_map(|m| m.get(h).copied())
                    .min()
                    .unwrap_or(0)
            }
        })
        .collect();
    let rest = total.checked_sub(degree(&top))?;
    *top.get_mut(g)? = rest;
    Some(top)
}

/// For each step, the group of a scale variable (a coordinate, a translation of grid points or a
/// rounded input), or `None` for an operation; or why the problems cannot state the filter.
fn groups_of(d: &Derivation) -> Result<Vec<Option<usize>>, EmitError> {
    let mut group_of = vec![None; d.steps.len()];
    for (g, group) in d.groups.iter().enumerate() {
        for &v in &group.vars {
            match (d.steps.get(v).map(|s| &s.kind), group_of.get_mut(v)) {
                (
                    Some(
                        StepKind::Translate { .. }
                        | StepKind::LoadCoord { .. }
                        | StepKind::LoadRounded { .. },
                    ),
                    Some(slot),
                ) => *slot = Some(g),
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
            (StepKind::Op { .. }, None)
            | (
                StepKind::Translate { .. }
                | StepKind::LoadCoord { .. }
                | StepKind::LoadRounded { .. },
                Some(_),
            ) => {}
            _ => {
                return fail(format!(
                    "step `{}` is a load or a translation outside every group, which no filter has",
                    step.name
                ));
            }
        }
    }
    Ok(group_of)
}

/// The shape of `d` (in `case`, for a scale of several monomials), or why the problems cannot
/// state its filter yet.
fn shape(d: &Derivation, case: Option<Case>) -> Result<Shape<'_>, EmitError> {
    let group_of = groups_of(d)?;
    let Some(result_set) = d.certs.get(d.result).map(|c| &c.scale) else {
        return fail("the result step is missing");
    };
    let mut monomials = Vec::with_capacity(d.certs.len());
    for (step, cert) in d.steps.iter().zip(&d.certs) {
        let mut set = cert.scale.iter();
        let m = match (set.next(), set.next(), &case) {
            (Some(m), None, _) => m.clone(),
            (Some(_), Some(_), Some(Case::Group(g))) => match dominant(&cert.scale, *g) {
                Some(m) => m,
                None => {
                    return fail(format!(
                        "step `{}`: no monomial of its scale dominates the others in this case",
                        step.name
                    ));
                }
            },
            // Every monomial of the step's scale is one of the result's, so the case bounds it.
            (Some(_), Some(_), Some(Case::Term(t))) if cert.scale.is_subset(result_set) => {
                t.clone()
            }
            (Some(_), Some(_), Some(Case::Term(_))) => {
                return fail(format!(
                    "step `{}` has a scale monomial the result's scale does not have, which a \
                     case of the result's monomials does not bound",
                    step.name
                ));
            }
            _ => {
                return fail(format!(
                    "step `{}` has a scale of several monomials, which a single LIN goal cannot \
                     state: one problem per case of the largest maximum",
                    step.name
                ));
            }
        };
        monomials.push(m);
    }
    let Some(monomial) = monomials.get(d.result).cloned() else {
        return fail("the result step is missing");
    };
    let roundings = d.roundings;
    let k = dyadic_k(d.constant_bits, roundings)?;
    Ok(Shape {
        d,
        group_of,
        monomial,
        monomials,
        case,
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

/// Each way the scale's larger-ofs can resolve: the scale then, as the filter computes it, with
/// each product rounded (`rnd(rnd(m_x * m_y) * m_z)`), and its monomial.
fn scale_variants(expr: &ScaleExpr, d: &Derivation) -> Result<Vec<(String, Monomial)>, EmitError> {
    let groups = d.groups.len();
    Ok(match expr {
        ScaleExpr::Max(index) => {
            let Some(group) = d.groups.get(*index) else {
                return fail("the scale names a missing group");
            };
            let mut unit = vec![0u32; groups];
            if let Some(slot) = unit.get_mut(*index) {
                *slot = 1;
            }
            vec![(max_name(&group.name), unit)]
        }
        ScaleExpr::One => vec![("1".to_owned(), vec![0u32; groups])],
        ScaleExpr::Mul(lhs, rhs) => {
            let right = scale_variants(rhs, d)?;
            let mut all = Vec::new();
            for (lt, lm) in scale_variants(lhs, d)? {
                for (rt, rm) in &right {
                    let product = lm
                        .iter()
                        .zip(rm)
                        .map(|(left, right)| left.saturating_add(*right))
                        .collect();
                    all.push((format!("rnd({lt} * {rt})"), product));
                }
            }
            all
        }
        ScaleExpr::Larger(lhs, rhs) => {
            let mut all = scale_variants(lhs, d)?;
            all.extend(scale_variants(rhs, d)?);
            all
        }
    })
}

/// The scale as the problem states it: the scale as the filter computes it, with each product
/// rounded, its larger-ofs resolved, in a case, the way whose monomial is the case's S. The
/// filter's computed scale is at least every way's (a larger-of returns the larger computed side,
/// and rounding and multiplying by a non-negative maximum are monotone), so its bound is at least
/// the one the problem states (the method note).
fn scale_rounded(sh: &Shape<'_>) -> Result<String, EmitError> {
    let d = sh.d;
    let all = scale_variants(&d.scale_expr, d)?;
    let chosen = match (all.as_slice(), &sh.case) {
        ([(text, m)], _) if *m == sh.monomial => Some(text.clone()),
        (_, Some(_)) => all
            .iter()
            .find(|(_, m)| *m == sh.monomial)
            .map(|(text, _)| text.clone()),
        _ => None,
    };
    match chosen {
        Some(text) if text != "1" => Ok(text),
        Some(_) => fail("the scale is 1, which no filter on points has"),
        None => fail("the scale takes a larger-of, which only a case resolves"),
    }
}

/// The scale each step's error is a multiple of, as Gappa's rules form it from the operands'
/// (a product's is the product of its operands', in their order) and as the problem's goal writes
/// it (canonical, in group order): the two differ by the order of the factors, which hints bridge.
fn natural_scales(s: &Shape<'_>) -> Result<Vec<String>, EmitError> {
    let d = s.d;
    let mut natural: Vec<String> = Vec::with_capacity(d.steps.len());
    for (step, group) in d.steps.iter().zip(&s.group_of) {
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
                let Some(m) = s.monomials.get(natural.len()) else {
                    return fail(format!("step `{}` has no monomial", step.name));
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
    /// or rounded input's name is `e_` and its own; a coordinate or a coordinate difference is
    /// exact either way.
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
            let rounded = matches!(step.kind, StepKind::LoadRounded { .. });
            Ok(if exact && (group.is_none() || rounded) {
                format!("e_{}", step.name)
            } else {
                step.name.clone()
            })
        };
        Ok((pick(left)?, pick(right)?))
    }
}

/// The file names of `d`'s Gappa problems: `gappa-<name>.g`, or one `gappa-<name>-<case>.g` per
/// case of [`gappa_cases`], with the name's `_` written `-`.
///
/// # Errors
///
/// [`EmitError`] if the result step is missing.
pub fn gappa_files(d: &Derivation) -> Result<Vec<String>, EmitError> {
    let stem = format!("gappa-{}", d.description.name.replace('_', "-"));
    let cases = gappa_cases(d)?;
    if cases.is_empty() {
        return Ok(vec![format!("{stem}.g")]);
    }
    Ok(cases.iter().map(|c| format!("{stem}-{c}.g")).collect())
}

/// The sentence a generated module's guarantees end with: whether its filter's bound is proven,
/// and by what, or "checked, not proven" with its open issue (chapter 28.6). Proven means the
/// Gappa problems, the proof of record, state the filter, every case of it, so
/// `kernel-certificates.yml` proves them on every change and every night, and fails if a proof
/// does not go through. `FPTaylor`'s problem, checked by HOL Light, is the independent second
/// check, and alone it does not make a filter proven: its certificate is about `FPTaylor`'s error
/// model, and the step to binary64 is written (the method note). `tests/certificate_problems.rs`
/// holds the registered problems to these files.
///
/// # Errors
///
/// [`EmitError`] if the Gappa problems cannot state the filter and [`UNPROVEN`] gives it no issue,
/// or `FPTaylor`'s problem cannot state it.
pub fn proof_sentence(d: &Derivation) -> Result<String, EmitError> {
    let name = &d.description.name;
    let fptaylor = format!("fptaylor-{}.fpt", name.replace('_', "-"));
    fptaylor_problem(d, "")?;
    let cases = gappa_cases(d)?;
    let stated = if cases.is_empty() {
        gappa_problem(d, "").map(|_| ())
    } else {
        cases
            .iter()
            .try_for_each(|c| gappa_case_problem(d, "", c).map(|_| ()))
    };
    if let Err(why) = stated {
        let Some((_, issue)) = UNPROVEN.iter().find(|(n, _)| n == name) else {
            return fail(format!(
                "the problems cannot state {name}'s filter ({}), and UNPROVEN gives it no open \
                 issue",
                why.0
            ));
        };
        return Ok(format!(
            "Its filter's bound is checked, not proven (chapter 28.6): derived exactly, \
             re-derived by geom-gen's independent checker, and tested; `FPTaylor` bounds it and \
             HOL Light checks its certificate (`{fptaylor}`, in `{PROBLEMS_DIR}`), but that \
             certificate is about `FPTaylor`'s error model, and Gappa's problems, the proof of \
             record, cannot state this filter yet (issue #{issue}); `kernel-certificates.yml` \
             runs the check on every change and every night."
        ));
    }
    let files: Vec<String> = gappa_files(d)?.iter().map(|f| format!("`{f}`")).collect();
    let gappa = if cases.is_empty() {
        format!("Gappa proves it for binary64 ({}", files.join(""))
    } else {
        format!(
            "Gappa proves it for binary64 in one problem per case, which together cover every \
             input ({}",
            files.join(", ")
        )
    };
    let proofs = if cases.is_empty() {
        "the proof"
    } else {
        "each proof"
    };
    Ok(format!(
        "Its filter's bound is proven (chapter 28.6): {gappa}, in `{PROBLEMS_DIR}`), and Rocq \
         checks {proofs}; `FPTaylor` bounds it independently, and HOL Light checks its \
         certificate (`{fptaylor}`); `kernel-certificates.yml` runs them on every change and \
         every night."
    ))
}

/// The Gappa problem of `d`'s filter (module documentation), as text.
///
/// # Errors
///
/// [`EmitError`] if the filter is not one the problems state yet (module documentation).
pub fn gappa_problem(d: &Derivation, generator: &str) -> Result<String, EmitError> {
    gappa_problem_in(d, generator, None)
}

/// The cases `d`'s Gappa problems split into, by name, in order: none when the result's scale has
/// one monomial (one problem covers every input); each group's name when the result's monomials
/// all have one degree and, whichever group's maximum is the largest, one of them dominates the
/// rest (in-circle's axes); and otherwise each of the result's monomials, its groups' names joined
/// by `-` (`x-n` for the maximum of x times that of n).
///
/// # Errors
///
/// [`EmitError`] if the result step is missing.
pub fn gappa_cases(d: &Derivation) -> Result<Vec<String>, EmitError> {
    let Some(set) = d.certs.get(d.result).map(|c| &c.scale) else {
        return fail("the result step is missing");
    };
    if set.len() < 2 {
        return Ok(Vec::new());
    }
    if group_cases(set, d.groups.len()) {
        return Ok(d.groups.iter().map(|g| g.name.clone()).collect());
    }
    Ok(set.iter().map(|m| term_name(m, d)).collect())
}

/// Whether the cases of the largest group maximum state a scale set: its monomials have one
/// degree, and for each group the monomial [`dominant`] gives is one of them. Otherwise (a
/// plane's normal beside the axes, whose largest maximum makes no monomial the largest) each
/// monomial is a case of its own.
fn group_cases(set: &BTreeSet<Monomial>, groups: usize) -> bool {
    one_degree(set) && (0..groups).all(|g| dominant(set, g).is_some_and(|m| set.contains(&m)))
}

/// Whether every monomial of `set` has one degree.
fn one_degree(set: &BTreeSet<Monomial>) -> bool {
    let mut degrees = set.iter().map(degree);
    let first = degrees.next();
    degrees.all(|e| Some(e) == first)
}

/// A monomial's name as a case: its groups' names, each as often as its exponent, joined by `-`.
fn term_name(m: &Monomial, d: &Derivation) -> String {
    let mut names = Vec::new();
    for (group, &e) in d.groups.iter().zip(m) {
        for _ in 0..e {
            names.push(group.name.as_str());
        }
    }
    names.join("-")
}

/// The Gappa problem of one case of `d`'s filter, whose result's scale has several monomials:
/// `case` is one of [`gappa_cases`]'s names, the case where that group's maximum is the largest,
/// or that monomial the largest of the result's. The cases' problems together cover every input.
///
/// # Errors
///
/// [`EmitError`] if `case` is not one of the filter's cases, or the filter is not one the problems
/// state yet (module documentation).
pub fn gappa_case_problem(
    d: &Derivation,
    generator: &str,
    case: &str,
) -> Result<String, EmitError> {
    let Some(set) = d.certs.get(d.result).map(|c| &c.scale) else {
        return fail("the result step is missing");
    };
    let found = if set.len() < 2 {
        None
    } else if group_cases(set, d.groups.len()) {
        d.groups
            .iter()
            .position(|group| group.name == case)
            .map(Case::Group)
    } else {
        set.iter()
            .find(|m| term_name(m, d) == case)
            .cloned()
            .map(Case::Term)
    };
    let Some(found) = found else {
        return fail(format!("`{case}` is not one of the filter's cases"));
    };
    gappa_problem_in(d, generator, Some(found))
}

/// The Gappa problem, in `case` if one is given.
fn gappa_problem_in(
    d: &Derivation,
    generator: &str,
    case: Option<Case>,
) -> Result<String, EmitError> {
    let sh = shape(d, case)?;
    let name = &d.description.name;
    let case_lines = match &sh.case {
        Some(Case::Group(g)) => match d.groups.get(*g) {
            Some(group) => vec![format!(
                "This is the case where m_{} is the largest maximum; each other case has its own problem.",
                group.name
            )],
            None => return fail("the case names a missing group"),
        },
        Some(Case::Term(t)) => vec![
            format!(
                "This is the case where {} is the largest term of the scale; each other term has",
                canonical(t, d)?
            ),
            "its own problem.".to_owned(),
        ],
        None => Vec::new(),
    };
    let first = format!(
        "{name}'s f64 filter (geom_num::generated::{name}::filter) for Gappa (chapter 28.6;"
    );
    let loads = d.steps.iter().any(|s| {
        matches!(
            s.kind,
            StepKind::LoadCoord { .. } | StepKind::LoadRounded { .. }
        )
    });
    let mut about: Vec<&str> = vec![&first];
    if loads {
        about.extend([
            "pieces G35 and G40). Gappa proves the goal below for binary64 with rounding to",
            "nearest, on whole-number coordinates (or their differences) and the f64 copies of",
            "whole-number inputs, each at most its group's maximum in magnitude, and writes a",
            "proof that Rocq checks. The goal: the computed value is within K * S of the exact",
            "one, and the computed bound within a relative error R of C * S, where",
            "K = C * (1 - R); so whenever the filter answers, its sign is the exact one.",
        ]);
    } else {
        about.extend([
            "pieces G35 and G40). Gappa proves the goal below for binary64 with rounding to",
            "nearest, on whole-number coordinate differences, each at most its axis maximum in",
            "magnitude, and writes a proof that Rocq checks. The goal: the computed value is within",
            "K * S of the exact one, and the computed bound within a relative error R of C * S,",
            "where K = C * (1 - R); so whenever the filter answers, its sign is the exact one.",
        ]);
    }
    if rounded_inputs(d) {
        about.extend([
            "A rounded input's copy is written rndx: rounding to 53 bits without exponent bounds,",
            "which is binary64's rounding on every whole number below 2^1024 (the method note).",
        ]);
    }
    about.extend(case_lines.iter().map(String::as_str));
    let mut out = String::new();
    header(&mut out, "#", generator, d, &about);
    gappa_definitions(&mut out, &sh)?;
    gappa_formula(&mut out, &sh)?;
    gappa_hints(&mut out, &sh)?;
    Ok(out)
}

/// Whether the filter takes rounded inputs.
fn rounded_inputs(d: &Derivation) -> bool {
    d.steps
        .iter()
        .any(|s| matches!(s.kind, StepKind::LoadRounded { .. }))
}

/// The rounding operators, the rounded inputs' copies, the filter's operations as computed and
/// exactly, the scale, the constant and the bound.
fn gappa_definitions(out: &mut String, sh: &Shape<'_>) -> Result<(), EmitError> {
    let d = sh.d;
    line(out, "");
    line(out, "@rnd = float<ieee_64, ne>;");
    if rounded_inputs(d) {
        line(out, "@rndx = float<53, ne>;");
        line(out, "");
        line(
            out,
            "# The rounded inputs' f64 copies, as the generated code converts them.",
        );
        for step in &d.steps {
            if matches!(step.kind, StepKind::LoadRounded { .. }) {
                line(out, &format!("{0} = rndx(e_{0});", step.name));
            }
        }
    }
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
    line(out, &format!("scale = {};", scale_rounded(sh)?));
    line(out, &format!("C = {};", hex_float(d.constant_bits)));
    line(out, "bound = rnd(C * scale);");
    line(out, "");
    Ok(())
}

/// A term case's own hypotheses: each other monomial of the result's scale is at most the case's,
/// S, so each value the filter forms as that monomial, an input or a product of two inputs, is at
/// most S in magnitude, `v // S` (|v| is at most the product of its inputs' group maxima, which is
/// that monomial). The steps above them inherit the bound through Gappa's rules.
fn term_bounds(sh: &Shape<'_>, t: &Monomial) -> Result<Vec<String>, EmitError> {
    let d = sh.d;
    let Some(result_set) = d.certs.get(d.result).map(|c| &c.scale) else {
        return fail("the result step is missing");
    };
    let mut bounds = Vec::new();
    let mut covered: BTreeSet<&Monomial> = BTreeSet::new();
    for ((step, cert), g) in d.steps.iter().zip(&d.certs).zip(&sh.group_of) {
        let mut set = cert.scale.iter();
        let (Some(m), None) = (set.next(), set.next()) else {
            continue;
        };
        if m == t || !result_set.contains(m) {
            continue;
        }
        let value = match (&step.kind, g) {
            (_, Some(_)) => step.name.clone(),
            (
                StepKind::Op {
                    op: Op::Mul,
                    left,
                    right,
                },
                None,
            ) => {
                let input = |i: &usize| sh.group_of.get(*i).is_some_and(Option::is_some);
                if !(input(left) && input(right)) {
                    return fail(format!(
                        "step `{}` forms a term of the scale from more than two inputs, which no \
                         term case states yet",
                        step.name
                    ));
                }
                let (lhs, rhs) = sh.operands(*left, *right, false)?;
                format!("{lhs} * {rhs}")
            }
            _ => continue,
        };
        bounds.push(format!("{value} // S in [-1, 1]"));
        covered.insert(m);
    }
    if let Some(m) = result_set.iter().find(|m| *m != t && !covered.contains(m)) {
        return fail(format!(
            "no input or product of two inputs forms the scale's term {}, so the case cannot \
             bound it",
            canonical(m, d)?
        ));
    }
    Ok(bounds)
}

/// The logical formula: whole-number inputs (a rounded input's exact value, within its bits) and
/// maxima, each maximum at most the largest magnitude its group's inputs can have as the filter
/// computes them, and at least 1 (or 0, for a group outside a term case's monomial); each input as
/// computed at most its group's maximum in magnitude; and the case's own bounds. Then the goal.
fn gappa_formula(out: &mut String, sh: &Shape<'_>) -> Result<(), EmitError> {
    let d = sh.d;
    let mut hypotheses: Vec<String> = Vec::new();
    for ((step, cert), g) in d.steps.iter().zip(&d.certs).zip(&sh.group_of) {
        if g.is_none() {
            continue;
        }
        if matches!(step.kind, StepKind::LoadRounded { .. }) {
            let Some(bound) = cert.bound.to_decimal() else {
                return fail(format!("step `{}`'s bound has no decimal form", step.name));
            };
            hypotheses.push(format!("@FIX(e_{}, 0)", step.name));
            hypotheses.push(format!("e_{} in [-{bound}, {bound}]", step.name));
        } else {
            hypotheses.push(format!("@FIX({}, 0)", step.name));
        }
    }
    for (gi, group) in d.groups.iter().enumerate() {
        let m = max_name(&group.name);
        // The largest magnitude an input of the group can have as the filter computes it: its
        // bound, or a rounded input's bound rounded to the nearest f64.
        let mut top = None;
        for &v in &group.vars {
            let (Some(step), Some(cert)) = (d.steps.get(v), d.certs.get(v)) else {
                return fail(format!("group `{}` names a missing step", group.name));
            };
            let computed = if matches!(step.kind, StepKind::LoadRounded { .. }) {
                cert.bound.nearest_f64()
            } else {
                Some(cert.bound.clone())
            };
            let Some(computed) = computed else {
                return fail(format!("step `{}`'s bound is beyond every f64", step.name));
            };
            top = top.max(Some(computed));
        }
        let Some(bound) = top.and_then(|t| t.to_decimal()) else {
            return fail(format!("group `{}` has no scale variable", group.name));
        };
        // A term case's monomial is at least 1 (a zero one makes every term zero, the method
        // note), so its groups' maxima are; another group's maximum may be 0.
        let least = match &sh.case {
            Some(Case::Term(t)) if t.get(gi).copied().unwrap_or(0) == 0 => 0,
            _ => 1,
        };
        hypotheses.push(format!("@FIX({m}, 0)"));
        hypotheses.push(format!("{m} in [{least}, {bound}]"));
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
    match &sh.case {
        // Every input is also at most the largest maximum in magnitude.
        Some(Case::Group(c)) => {
            let Some(top) = d.groups.get(*c) else {
                return fail("the case names a missing group");
            };
            for (step, g) in d.steps.iter().zip(&sh.group_of) {
                if g.is_some_and(|g| g != *c) {
                    hypotheses.push(format!(
                        "{} // {} in [-1, 1]",
                        step.name,
                        max_name(&top.name)
                    ));
                }
            }
        }
        Some(Case::Term(t)) => hypotheses.extend(term_bounds(sh, t)?),
        None => {}
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
    for ((step, m), nat) in d.steps.iter().zip(&sh.monomials).zip(&natural) {
        if !matches!(step.kind, StepKind::Op { op: Op::Mul, .. }) {
            continue;
        }
        let canon = canonical(m, d)?;
        if *nat != canon && !seen.iter().any(|(a, b)| *a == *nat && *b == canon) {
            line(out, &format!("{nat} -> {canon};"));
            line(out, &format!("{canon} -> {nat};"));
            seen.push((nat.clone(), canon));
        }
    }
    // The bound's relative error is stated against C * S, and Gappa matches products as written:
    // where the filter multiplies the maxima out in another order than S's, the scale's exact
    // form is tied to S.
    let exact = scale_rounded(sh)?.replace("rnd(", "(");
    let mut factors = canonical(&sh.monomial, d)?
        .split(" * ")
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .into_iter();
    let first = factors.next().unwrap_or_default();
    let written = factors.fold(first, |acc, f| format!("({acc} * {f})"));
    if exact != written {
        line(out, "");
        line(out, "# The scale as the filter multiplies it out is S.");
        line(out, &format!("C * S -> C * {exact};"));
    }
    Ok(())
}

/// The `FPTaylor` problem of `d`'s filter (module documentation), as text: the normalised problem,
/// each step divided by its scale S(v), with a ratio parameter in [0, 1] for each sum's operand
/// whose scale set is not the sum's, and, in a structured comment, the K that HOL Light compares
/// the certificate's bound with. One problem covers every input: it is the derivation's own
/// induction (S(v) the largest of its monomials), so it needs no cases.
///
/// # Errors
///
/// [`EmitError`] if a scale variable is not an input the problems state, or a ratio parameter's
/// name is a step's.
pub fn fptaylor_problem(d: &Derivation, generator: &str) -> Result<String, EmitError> {
    let group_of = groups_of(d)?;
    let (numerator, exponent) = dyadic_k(d.constant_bits, d.roundings)?;
    let ratios = ratios(d)?;
    let about = fptaylor_about(d, &ratios);
    let about: Vec<&str> = about.iter().map(String::as_str).collect();
    let mut out = String::new();
    header(&mut out, "//", generator, d, &about);
    line(
        &mut out,
        &format!("// hol-light-bound: &{numerator} * &2 ipow (-- &{exponent})"),
    );
    if exact_optimisation(d) {
        line(&mut out, "// fptaylor-optimisation: exact");
        let total = hex_float(below((numerator, exponent))?);
        line(&mut out, &format!("// hol-light-total: {total}"));
    } else {
        line(&mut out, "// fptaylor-optimisation: approx");
    }
    line(&mut out, "");
    line(&mut out, "Variables");
    for (step, g) in d.steps.iter().zip(&group_of) {
        match (&step.kind, g) {
            (StepKind::LoadRounded { .. }, Some(_)) => line(
                &mut out,
                &format!("  real e_{} in [-{ONE_PLUS_U}, {ONE_PLUS_U}];", step.name),
            ),
            (_, Some(_)) => line(&mut out, &format!("  float64 {} in [-1, 1];", step.name)),
            (_, None) => {}
        }
    }
    for i in 0..ratios.len() {
        line(&mut out, &format!("  real {} in [0, 1];", ratio_name(i)));
    }
    line(&mut out, "");
    line(&mut out, "Definitions");
    for step in &d.steps {
        if matches!(step.kind, StepKind::LoadRounded { .. }) {
            line(&mut out, &format!("  {0} = rnd64(e_{0});", step.name));
        }
    }
    let mut result = None;
    for (i, (step, cert)) in d.steps.iter().zip(&d.certs).enumerate() {
        let StepKind::Op { op, left, right } = &step.kind else {
            continue;
        };
        let operand = |o: &usize| -> Result<String, EmitError> {
            let (Some(s), Some(c)) = (d.steps.get(*o), d.certs.get(*o)) else {
                return fail("an operand is missing");
            };
            if matches!(op, Op::Mul) || c.scale == cert.scale {
                return Ok(s.name.clone());
            }
            match ratios
                .iter()
                .position(|r| r.iter().any(|(a, b)| *a == c.scale && *b == cert.scale))
            {
                Some(r) => Ok(format!("{} * {}", ratio_name(r), s.name)),
                None => fail("a sum's operand has no ratio"),
            }
        };
        let text = format!(
            "  {} = rnd64({} {} {});",
            step.name,
            operand(left)?,
            op.symbol(),
            operand(right)?
        );
        if i == d.result {
            result = Some(text);
        } else {
            line(&mut out, &text);
        }
    }
    let Some(result) = result else {
        return fail("the result is not an operation");
    };
    line(&mut out, "");
    line(&mut out, "Expressions");
    line(&mut out, &result);
    Ok(out)
}

/// The `FPTaylor` problem's description, after the generated-file header: what it states, and
/// each ratio parameter as the ratios of scale sets it stands for.
fn fptaylor_about(d: &Derivation, ratios: &[Ratio]) -> Vec<String> {
    let name = &d.description.name;
    let mut about = vec![
        format!("{name}'s f64 filter (geom_num::generated::{name}::filter) for FPTaylor (chapter"),
        "28.6; pieces G35 and G40), normalised: each step divided by its scale S(v), the largest"
            .to_owned(),
        "of its scale's monomials at the group maxima, so every input is in [-1, 1] and the"
            .to_owned(),
        "result's scale is 1. The error model fl(t) = t(1 + e) is homogeneous, so a bound B here"
            .to_owned(),
        "bounds the error by B * S at every scale (the method note).".to_owned(),
    ];
    if exact_optimisation(d) {
        about.extend([
            "FPTaylor bounds the sum of the error terms at once (opt-exact); HOL Light proves the"
                .to_owned(),
            "sum at most hol-light-total below, the largest double at most K, when FPTaylor's own"
                .to_owned(),
            "bound is at most that, and otherwise checks FPTaylor's own bound and compares it with K."
                .to_owned(),
        ]);
    } else {
        about.extend([
            "FPTaylor bounds each error term on its own (opt-approx); HOL Light checks each term's"
                .to_owned(),
            "bound, and the job compares their sum with K below.".to_owned(),
        ]);
    }
    if rounded_inputs(d) {
        about.extend([
            "A rounded input's exact value e_v is at most its maximum times 1 + 2^-53 (a value"
                .to_owned(),
            "beyond m + ulp(m)/2 rounds above the f64 m), and its copy v = rnd64(e_v).".to_owned(),
        ]);
    }
    if !ratios.is_empty() {
        about.extend([
            "A sum whose operand's scale is not the sum's takes the operand's normalised value"
                .to_owned(),
            "times S(operand) / S(sum), a ratio in [0, 1], exactly; each such ratio of two scale"
                .to_owned(),
            "sets is one parameter, shared by the ratios equal to it once their common factor is"
                .to_owned(),
            "cancelled (where that factor is 0, the sum is 0 whatever the parameter):".to_owned(),
        ]);
        let groups: Vec<String> = d.groups.iter().map(|g| g.name.clone()).collect();
        for (i, pairs) in ratios.iter().enumerate() {
            let texts: Vec<String> = pairs
                .iter()
                .map(|(operand, sum)| {
                    format!(
                        "{} / {}",
                        scale_text(operand, &groups),
                        scale_text(sum, &groups)
                    )
                })
                .collect();
            about.push(format!("{} = {}", ratio_name(i), texts.join(" = ")));
        }
    }
    about
}

/// Whether `d`'s `FPTaylor` problem bounds the sum of its error terms at once
/// ([`EXACT_OPTIMISATION`]).
fn exact_optimisation(d: &Derivation) -> bool {
    EXACT_OPTIMISATION.contains(&d.description.name.as_str())
}

/// The bits of the largest `f64` at most the positive dyadic `numerator · 2^−exponent`: its top
/// 53 bits, the rest cut off. Refused outside the normal range, which no error bound reaches.
fn below((numerator, exponent): (u128, u32)) -> Result<u64, EmitError> {
    let width = u128::BITS.saturating_sub(numerator.leading_zeros());
    let significand = match width.checked_sub(53) {
        Some(cut) => numerator.checked_shr(cut),
        None => 53u32
            .checked_sub(width)
            .and_then(|up| numerator.checked_shl(up)),
    };
    // The leading bit's place is width − 1 − exponent; the field adds the bias, 1023.
    let field = i64::from(width)
        .checked_sub(1)
        .and_then(|w| w.checked_sub(i64::from(exponent)))
        .and_then(|e| e.checked_add(1023));
    match (
        width,
        significand.and_then(|m| u64::try_from(m).ok()),
        field.and_then(|f| u64::try_from(f).ok()),
    ) {
        (1.., Some(m), Some(f)) if (1..0x7ff).contains(&f) => {
            Ok((f << 52) | (m & ((1u64 << 52) - 1)))
        }
        _ => fail("K is not a positive normal f64's size"),
    }
}

/// 1 + 2^−53 as `FPTaylor` reads a rational constant: a rounded input's exact value is at most
/// its group's maximum times this.
const ONE_PLUS_U: &str = "9007199254740993/9007199254740992";

/// The name of the `i`-th ratio parameter (from 0): `ratio1`, `ratio2`, and so on.
fn ratio_name(i: usize) -> String {
    format!("ratio{}", i.saturating_add(1))
}

/// A sum's operand's scale set and the sum's.
type Sets = (BTreeSet<Monomial>, BTreeSet<Monomial>);

/// A ratio parameter of an `FPTaylor` problem: the pairs of scale sets it stands for, the
/// operand's over the sum's, each the same ratio once their common factor is cancelled.
type Ratio = Vec<Sets>;

/// Both sets of `pair` divided by the largest monomial that divides every one of the sum's (and
/// so the operand's, which the sum's holds): S(a) / S(b) = S(a / g) / S(b / g) when g's maxima
/// are not zero, since S multiplies through by a common factor.
fn cancelled((operand, sum): &Sets) -> Sets {
    let mut monomials = sum.iter();
    let first = monomials.next().cloned().unwrap_or_default();
    let common: Monomial = monomials.fold(first, |g, m| {
        g.iter().zip(m).map(|(a, b)| *a.min(b)).collect()
    });
    let less = |set: &BTreeSet<Monomial>| -> BTreeSet<Monomial> {
        set.iter()
            .map(|m| {
                m.iter()
                    .zip(&common)
                    .map(|(e, c)| e.saturating_sub(*c))
                    .collect()
            })
            .collect()
    };
    (less(operand), less(sum))
}

/// The ratios of scales the normalised problem needs, in order of first use: for each sum or
/// difference, each operand whose scale set is not the step's, as `(operand's, step's)`, once
/// each, and pairs that are the same ratio once their common factor is cancelled
/// ([`cancelled`]) as one parameter. S(operand) ≤ S(step), the step's set holding the operand's,
/// so each ratio is in [0, 1]; and a product's scale set is its operands' products, whose largest
/// is the product of their largest, so a product needs none. Refused if a name would clash with
/// a step's.
fn ratios(d: &Derivation) -> Result<Vec<Ratio>, EmitError> {
    let mut out: Vec<Ratio> = Vec::new();
    for (step, cert) in d.steps.iter().zip(&d.certs) {
        let StepKind::Op {
            op: Op::Add | Op::Sub,
            left,
            right,
        } = &step.kind
        else {
            continue;
        };
        for o in [left, right] {
            let Some(c) = d.certs.get(*o) else {
                return fail("an operand is missing");
            };
            if !c.scale.is_subset(&cert.scale) {
                return fail(format!(
                    "step `{}`'s scale does not hold its operand's",
                    step.name
                ));
            }
            let pair = (c.scale.clone(), cert.scale.clone());
            if c.scale == cert.scale || out.iter().any(|r| r.contains(&pair)) {
                continue;
            }
            let key = cancelled(&pair);
            match out
                .iter_mut()
                .find(|r| r.first().is_some_and(|p| cancelled(p) == key))
            {
                Some(r) => r.push(pair),
                None => out.push(vec![pair]),
            }
        }
    }
    if let Some(step) = d
        .steps
        .iter()
        .find(|s| (0..out.len()).any(|i| s.name == ratio_name(i)))
    {
        return fail(format!("step `{}` has a ratio parameter's name", step.name));
    }
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
        // K is C less 4u · C, 3.5 of C's units in the last place and a little more, so the
        // largest double at most K is C less 4 of them; and a double is itself.
        let k = dyadic_k(0x3cec_0000_0000_0007, 3).unwrap_or((0, 0));
        assert_eq!(
            below(k).map(hex_float),
            Ok("0x1.c000000000003p-49".to_owned())
        );
        assert_eq!(
            below((0x1c_0000_0000_0007, 101)).map(hex_float),
            Ok("0x1.c000000000007p-49".to_owned())
        );
        assert!(below((0, 1)).is_err());
    }

    /// A filter the problems cannot state is never labelled proven: without an open issue in
    /// [`UNPROVEN`] its module cannot be generated at all.
    #[test]
    fn an_unstated_filter_needs_its_issue() {
        // Mixed degrees, and a term of three inputs, which no term case states.
        let d = crate::predicate_derive::derive(
            "predicate three\ninput a b: point2\nlet s = a.x * a.y * b.x + b.y\nsign s\n",
        )
        .unwrap();
        assert_eq!(gappa_cases(&d).unwrap(), ["y", "x-x-y"]);

        let why = gappa_case_problem(&d, "test", "y").unwrap_err();
        assert!(why.0.contains("more than two inputs"), "{}", why.0);
        let refused = proof_sentence(&d).unwrap_err();
        assert!(
            refused.0.contains("UNPROVEN gives it no open issue"),
            "{}",
            refused.0
        );
    }
}
