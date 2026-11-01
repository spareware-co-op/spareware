//! Rust for the generated predicates: one module per predicate, and the module that holds them.
//!
//! What it does: [`predicate_module`] writes a predicate's module from its [`Derivation`]: a
//! header with the description, the bit-range certificate and the error analysis, the error
//! constant, `filter` (the `f64` evaluation and its decision), and `value` and `exact` (the
//! checked integer evaluation). [`parent_module`] writes the module that declares them and holds
//! the helpers they share. Both are formatted as rustfmt formats them, so `cargo fmt --check`
//! passes on the committed files. [`verus_twin`] writes a Verus-annotated twin of a predicate's
//! exact path (chapter 28.7; piece G36): a model of the grid's types, the description's value over
//! the integers, and `value` written by the same function as the module's, with a `#[verus_spec]`
//! attribute and a `proof!` hint before each product, so with those removed it is the shipped
//! `value` token for token.
//!
//! Guarantees:
//!
//! - **The code is the derivation.** `filter` performs exactly the derivation's steps, in their
//!   order, one `let` per step, so the error analysis is about the code that runs; it computes
//!   the scale by the derivation's scale expression, whose roundings the constant accounts for.
//!   `value` performs the same steps, each in the integer type the certificate gives it (the
//!   narrowest of `i64`, `i128`, `I256`, `I320`, `I384` and `I512` that holds its bound), with
//!   operands widened to that type and checked arithmetic (`checked_add`, `checked_sub`,
//!   `checked_mul`, and `resize` between `Wide`s), so a certificate that was wrong would show as
//!   `None`, never as a wrapped value. A product in a `Wide` whose operands fit `i128` is
//!   `product_i128`, and one with one such operand `checked_mul_i128` (piece G29): the same exact
//!   product or `None`, without widening the narrow operand to every limb. The certificate's rows
//!   name the same steps.
//! - **Width-aware, as chapter 23.3 asks.** A step that fits 63 bits is computed in `i64`, so a
//!   product of two such steps is `i128::from(a).checked_mul(i128::from(b))`: two sign-extended
//!   64-bit operands, whose product the compiler proves cannot overflow `i128` and emits as one
//!   64 × 64 → 128-bit multiply, where a product of two general `i128`s is a long overflow-checked
//!   sequence. The value is the same either way; only the cost differs.
//! - **Only the domain the analysis assumes.** A point input is a `P2` or `P3`, whose coordinates
//!   are within the grid, so no caller can pass a value outside it. A rounded input is passed as
//!   its exact integer, an `i128` (a description's rounded input of more than 127 bits is
//!   refused here: no correctly rounded conversion of a wider integer to `f64` exists in
//!   `geom-num` yet); `filter` converts it to its nearest `f64` and both functions return `None`
//!   for one outside its declared range, before any bound is relied on.
//! - **Names that cannot collide.** Every name the module binds is checked here against every
//!   other and against the module's items, imports and Rust's keywords, so a description that
//!   slipped past the language's rules ([`crate::predicate_lang::reserved`]) is refused, never
//!   emitted as code that shadows a value or does not compile.
//! - **Kernel rules.** No native integer arithmetic and no casts beyond the exact conversion of a
//!   grid coordinate to `f64` and the correctly rounded conversion of a rounded input (each with
//!   its reason written beside it), no `unsafe`, no panics, no allocation; every public item
//!   documented.
//!
//! Method: the generated predicates of Geogram's PCK (`pck.sh` runs FPG's code generator over a
//! `.pck` file for the filter, and the same file instantiated with expansion arithmetic gives the
//! exact path; Lévy 2016, <https://doi.org/10.1016/j.cad.2015.10.004>, and
//! <https://github.com/BrunoLevy/geogram/blob/16843b4804de35f14ef54667950ab4c8acdd0062/src/lib/geogram/numerics/predicates/pck.sh>,
//! BSD-3-Clause, read for method), with the exact path in fixed-width integers chosen per step as
//! chapter 23.3 of the kernel architecture asks
//! (`projects/engines/geometry/architecture/23-performance.md`).
//! Consulted: Geogram PCK, BSD-3-Clause,
//!   <https://github.com/BrunoLevy/geogram/blob/16843b4804de35f14ef54667950ab4c8acdd0062/src/lib/geogram/numerics/predicates/pck.sh>,
//!   for how PCK runs FPG over one description per predicate. No code copied.
//!
//! Cost: linear in the number of steps.

use geom_num::grid::{COORD_BITS, COORD_MAX};

use std::collections::BTreeSet;

use crate::bignum::Nat;
use crate::predicate_certificates::proof_sentence;
use crate::predicate_derive::{Derivation, Monomial, ScaleExpr, StepCert, Width};
use crate::predicate_lang::{Axis, InputKind, Op, RUST_WORDS, Step, StepKind, max_name};

/// The path of the predicate descriptions, for the headers.
pub const DESCRIPTIONS_PATH: &str = "projects/engines/geometry/crates/geom-gen/src/predicates.rs";

/// The path of this emitter, for the headers.
pub const EMITTER_PATH: &str = "projects/engines/geometry/crates/geom-gen/src/predicate_emit.rs";

/// The widest rounded input the emitter takes: an `i128`'s magnitude bits.
const ROUNDED_MAX_BITS: u32 = 127;

/// Why emission failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmitError(pub String);

impl core::fmt::Display for EmitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

fn fail<T>(message: impl Into<String>) -> Result<T, EmitError> {
    Err(EmitError(message.into()))
}

/// How an exact path holds its steps and forms its products: the variants chapter 23.3 compares
/// (piece G42). Every style computes the same value with checked arithmetic, so only the cost
/// differs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The shipped exact paths: each step in the narrowest type its certificate gives it, from
    /// `i64` up, and a product of operands that fit `i128` formed in a wide type by
    /// `product_i128` or `checked_mul_i128` (piece G29).
    WidthAware,
    /// Plain `i128`: each step in `i128`, or in its certificate's wide type when it does not fit
    /// 127 bits, with both operands widened to that type and the general `checked_mul`: no width
    /// analysis beyond each result's type. On WebAssembly every product then goes through
    /// `__multi3`, the library routine Rust's `i128` multiply becomes there.
    Plain,
    /// Width-aware limbs: each step in `i64`, or in a `Wide32` of as many 32-bit limbs as its
    /// operands' certified bits need ([`limbs_held`]), so a limb product is one 32 × 32 → 64-bit
    /// multiply, native on WebAssembly; the result is returned in the shipped path's type.
    Limbs,
}

impl Style {
    /// The type a step certified to fit `width` is held in.
    #[must_use]
    pub const fn held(self, width: Width) -> Width {
        match (self, width) {
            (Style::Plain, Width::I64) => Width::I128,
            (_, w) => w,
        }
    }
}

/// A line to `out`; writing to a `String` cannot fail.
fn line(out: &mut String, text: &str) {
    out.push_str(text);
    out.push('\n');
}

/// A doc line `//! text`, or `//!` alone for an empty one.
fn doc(out: &mut String, text: &str) {
    if text.is_empty() {
        line(out, "//!");
    } else {
        line(out, &format!("//! {text}"));
    }
}

/// A monomial as `x·y^2`, or `1` for the empty one.
#[must_use]
pub fn monomial_text(m: &Monomial, groups: &[String]) -> String {
    let mut parts = Vec::new();
    for (g, &e) in groups.iter().zip(m) {
        match e {
            0 => {}
            1 => parts.push(g.clone()),
            e => parts.push(format!("{g}^{e}")),
        }
    }
    if parts.is_empty() {
        "1".to_owned()
    } else {
        parts.join("·")
    }
}

/// A scale set as `x·y`, or `max{x^3·y, x·y^3}` when it has several monomials.
#[must_use]
pub fn scale_text<'m>(scale: impl IntoIterator<Item = &'m Monomial>, groups: &[String]) -> String {
    let parts: Vec<String> = scale
        .into_iter()
        .map(|m| monomial_text(m, groups))
        .collect();
    if parts.len() == 1 {
        parts.concat()
    } else {
        format!("max{{{}}}", parts.join(", "))
    }
}

/// The scale expression as Rust.
fn scale_rust(e: &ScaleExpr, groups: &[String]) -> Result<String, EmitError> {
    Ok(match e {
        ScaleExpr::Max(g) => match groups.get(*g) {
            Some(name) => max_name(name),
            None => return fail("a scale expression names a missing group"),
        },
        ScaleExpr::One => "1.0".to_owned(),
        ScaleExpr::Mul(l, r) => {
            let right = match &**r {
                ScaleExpr::Mul(..) => format!("({})", scale_rust(r, groups)?),
                other => scale_rust(other, groups)?,
            };
            format!("{} * {right}", scale_rust(l, groups)?)
        }
        ScaleExpr::Larger(l, r) => {
            format!(
                "larger({}, {})",
                scale_rust(l, groups)?,
                scale_rust(r, groups)?
            )
        }
    })
}

/// What a step computes, as written in the certificate: `b.x - a.x`, `ba_x * ca_y`, `a.x`.
#[must_use]
pub fn step_text(step: &Step, steps: &[Step]) -> String {
    let name = |i: &usize| steps.get(*i).map_or("?", |s| s.name.as_str());
    match &step.kind {
        StepKind::LoadCoord { point, axis } => format!("{point}.{}", axis.letter()),
        StepKind::LoadRounded { input } => input.clone(),
        StepKind::Translate {
            minuend,
            subtrahend,
            axis,
        } => format!("{minuend}.{a} - {subtrahend}.{a}", a = axis.letter()),
        StepKind::Op { op, left, right } => {
            format!("{} {} {}", name(left), op.symbol(), name(right))
        }
    }
}

/// The value of an `f64`'s bits as `m · 2^e`, with m the whole significand.
fn f64_exact_text(bits: u64) -> String {
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    let biased = bits.checked_shr(52).unwrap_or(0) & 0x7ff;
    if biased == 0 {
        return format!("{fraction} · 2^-1074");
    }
    let m = fraction | 0x0010_0000_0000_0000;
    let e = i64::try_from(biased).unwrap_or(0).saturating_sub(1_075);
    format!("{m} · 2^{e}")
}

/// A hex literal with `_` every four digits, as clippy's `unreadable_literal` asks.
fn hex_literal(bits: u64) -> String {
    let digits = format!("{bits:016x}");
    let groups: Vec<&str> = (0..4)
        .filter_map(|i: usize| {
            digits.get(i.saturating_mul(4)..i.saturating_mul(4).saturating_add(4))
        })
        .collect();
    format!("0x{}", groups.join("_"))
}

/// The point type of a description's point inputs (`P2` or `P3`), `None` if it has none.
fn point_type(d: &Derivation) -> Result<Option<&'static str>, EmitError> {
    let mut dims = d.description.inputs.iter().filter_map(|i| match i.kind {
        InputKind::Point { dim } => Some(dim),
        InputKind::Rounded { .. } => None,
    });
    let Some(first) = dims.next() else {
        return Ok(None);
    };
    if dims.any(|x| x != first) {
        return fail("every point input must have the same dimension");
    }
    match first {
        2 => Ok(Some("P2")),
        3 => Ok(Some("P3")),
        _ => fail("points have 2 or 3 coordinates"),
    }
}

/// The bits a rounded input declares, if `input` is one the emitter takes.
fn rounded_bits(d: &Derivation, input: &str) -> Result<u32, EmitError> {
    match d.description.input(input).map(|i| &i.kind) {
        Some(InputKind::Rounded { bits, .. }) if *bits <= ROUNDED_MAX_BITS => Ok(*bits),
        Some(InputKind::Rounded { bits, .. }) => fail(format!(
            "`{}`: the rounded input `{input}` has {bits} bits; the emitter takes rounded inputs \
             of at most {ROUNDED_MAX_BITS} bits, as `i128`s, because geom-num has no correctly \
             rounded conversion of a wider integer to f64 yet",
            d.description.name
        )),
        _ => fail(format!("`{input}` is not a rounded input")),
    }
}

/// The parameters: `a: P2`, `b: P2`, `c: P2`, or `nx: i128`, …, `p: P3` with rounded inputs.
fn params(d: &Derivation, ty: Option<&str>) -> Result<Vec<String>, EmitError> {
    let mut parts = Vec::new();
    for i in &d.description.inputs {
        let t = match (&i.kind, ty) {
            (InputKind::Point { .. }, Some(t)) => t,
            (InputKind::Point { .. }, None) => return fail("a point input without a point type"),
            (InputKind::Rounded { .. }, _) => {
                rounded_bits(d, &i.name)?;
                "i128"
            }
        };
        parts.push(format!("{}: {t}", i.name));
    }
    Ok(parts)
}

/// A generated function's first line, `pub fn <name>(<parameters>) -> <ret> {`, as rustfmt lays
/// it out: on one line when it fits [`WIDTH`] columns, and otherwise as rustfmt's default
/// `fn_params_layout`, `Tall`, writes it, one parameter per line, indented four spaces, each
/// followed by a comma, and the return type after the closing parenthesis.
fn signature(out: &mut String, name: &str, parameters: &[String], ret: &str) {
    let one = format!("pub fn {name}({}) -> {ret} {{", parameters.join(", "));
    if one.chars().count() <= WIDTH {
        line(out, &one);
        return;
    }
    line(out, &format!("pub fn {name}("));
    for p in parameters {
        line(out, &format!("    {p},"));
    }
    line(out, &format!(") -> {ret} {{"));
}

/// The widest argument list rustfmt keeps on one line in a call: its default `fn_call_width`, 60%
/// of [`WIDTH`].
const CALL_ARGS_WIDTH: usize = 60;

/// `exact`'s body, `value(<arguments>).map(<sign>)`, as rustfmt lays it out: on one line when the
/// line fits [`WIDTH`] columns and the arguments [`CALL_ARGS_WIDTH`]; otherwise the call's
/// arguments on one line of their own, indented eight spaces and followed by a comma, and the
/// `.map` on the line after the closing parenthesis, as rustfmt breaks a chain whose arguments are
/// all names. A list of arguments too long for that one line is refused.
fn value_call(out: &mut String, arguments: &str, sign: &str) -> Result<(), EmitError> {
    let one = format!("    value({arguments}).map({sign})");
    if one.chars().count() <= WIDTH && arguments.chars().count() <= CALL_ARGS_WIDTH {
        line(out, &one);
        return Ok(());
    }
    let inner = format!("        {arguments},");
    if inner.chars().count() > WIDTH {
        return fail(format!(
            "`exact`'s arguments would pass {WIDTH} columns on a line of their own, which rustfmt \
             lays out over several lines: {arguments}"
        ));
    }
    line(out, "    value(");
    line(out, &inner);
    line(out, "    )");
    line(out, &format!("    .map({sign})"));
    Ok(())
}

/// The lines of the filter that compute its scale. On one line, `let scale = <expression>;`, when
/// that fits [`WIDTH`] columns and the expression is not a `larger` whose arguments pass
/// [`CALL_ARGS_WIDTH`]. Otherwise the scale is the largest of several terms, times their common
/// factor if they have one ([`crate::predicate_derive`]'s scale expression), and it is computed one
/// term per line: the larger of the first two terms (or the first term, then the larger of it and
/// the second, when two would pass [`CALL_ARGS_WIDTH`]), then the larger of the scale so far and
/// each further term, then the common factor times the scale. Those are the operations the one-line
/// expression performs, on the same values, so they give the same `f64` and the same roundings;
/// the header shows the one-line expression they compute. A scale of another form, or a line that
/// would still be too long, is refused.
fn scale_lines(e: &ScaleExpr, groups: &[String]) -> Result<Vec<String>, EmitError> {
    let whole = scale_rust(e, groups)?;
    let one = format!("    let scale = {whole};");
    let call_args_fit = match e {
        ScaleExpr::Larger(l, r) => {
            let args = format!("{}, {}", scale_rust(l, groups)?, scale_rust(r, groups)?);
            args.chars().count() <= CALL_ARGS_WIDTH
        }
        _ => true,
    };
    if one.chars().count() <= WIDTH && call_args_fit {
        return Ok(vec![one]);
    }
    let (common, largest) = match e {
        ScaleExpr::Mul(c, l) if matches!(**l, ScaleExpr::Larger(..)) => (Some(&**c), &**l),
        ScaleExpr::Larger(..) => (None, e),
        _ => {
            return fail(format!(
                "the scale `{whole}` passes {WIDTH} columns and is not the largest of several \
                 terms"
            ));
        }
    };
    // The terms of the left-nested `larger(larger(t1, t2), t3)`, in order.
    let mut terms = Vec::new();
    let mut at = largest;
    while let ScaleExpr::Larger(l, r) = at {
        terms.push(scale_rust(r, groups)?);
        at = l;
    }
    terms.push(scale_rust(at, groups)?);
    terms.reverse();
    let mut lines = Vec::new();
    let mut previous: Option<String> = None;
    for term in terms {
        let Some(p) = previous.take() else {
            previous = Some(term);
            continue;
        };
        if format!("{p}, {term}").chars().count() > CALL_ARGS_WIDTH {
            // Two long terms take a line each: the same values, in the same order.
            if p != "scale" {
                lines.push(format!("    let scale = {p};"));
            }
            lines.push(format!("    let scale = larger(scale, {term});"));
        } else {
            lines.push(format!("    let scale = larger({p}, {term});"));
        }
        previous = Some("scale".to_owned());
    }
    if let Some(c) = common {
        lines.push(format!(
            "    let scale = {} * scale;",
            scale_rust(c, groups)?
        ));
    }
    for l in &lines {
        let args = l
            .strip_prefix("    let scale = larger(")
            .and_then(|a| a.strip_suffix(");"));
        if l.chars().count() > WIDTH || args.is_some_and(|a| a.chars().count() > CALL_ARGS_WIDTH) {
            return fail(format!(
                "a line of the scale would still be laid out over several lines by rustfmt: {l}"
            ));
        }
    }
    Ok(lines)
}

/// The argument list: `a, b, c`.
fn args(d: &Derivation) -> String {
    let parts: Vec<&str> = d
        .description
        .inputs
        .iter()
        .map(|i| i.name.as_str())
        .collect();
    parts.join(", ")
}

/// The names of the group maxima, in group order.
fn group_names(d: &Derivation) -> Vec<String> {
    d.groups.iter().map(|g| g.name.clone()).collect()
}

/// Whether a description has coordinate loads or translations, which call `exact_f64`.
fn has_coordinates(d: &Derivation) -> bool {
    d.steps.iter().any(|s| {
        matches!(
            s.kind,
            StepKind::LoadCoord { .. } | StepKind::Translate { .. }
        )
    })
}

/// Whether a description has rounded inputs, which call `rounded_f64` and `within`.
fn has_rounded(d: &Derivation) -> bool {
    d.steps
        .iter()
        .any(|s| matches!(s.kind, StepKind::LoadRounded { .. }))
}

/// `name`, of type `from`, as a `to`: unchanged, widened from `i64` or `i128`, or resized.
fn widened(name: &str, from: Width, to: Width) -> Result<String, EmitError> {
    Ok(match (from, to) {
        (f, t) if f == t => name.to_owned(),
        (Width::I64, Width::I128) => format!("i128::from({name})"),
        (Width::I64, t) => format!("{}::from_i64({name})", t.rust()),
        (Width::I128, t) if t > Width::I128 => format!("{}::from_i128({name})", t.rust()),
        (f, t) if f > Width::I128 && t > f => format!("{name}.resize::<{}>()?", t.limbs()),
        _ => return fail(format!("`{name}` is wider than the step that uses it")),
    })
}

/// A grid coordinate (an `i64`) in the exact path, as a `to`.
fn coordinate(point: &str, axis: Axis, to: Width) -> Result<String, EmitError> {
    widened(&format!("{point}.{}.get()", axis.letter()), Width::I64, to)
}

/// The checked method of an operation.
const fn method(op: Op) -> &'static str {
    match op {
        Op::Add => "checked_add",
        Op::Sub => "checked_sub",
        Op::Mul => "checked_mul",
    }
}

/// `left.method(right)?` in type `width`: by value for `i64` and `i128`, by reference for a `Wide`.
fn checked(left: &str, op: Op, right: &str, width: Width) -> String {
    if width <= Width::I128 {
        format!("{left}.{}({right})?", method(op))
    } else {
        format!("{left}.{}(&{right})?", method(op))
    }
}

/// The exact path's statement for one step.
fn exact_statement(
    step: &Step,
    cert: &StepCert,
    d: &Derivation,
    style: Style,
) -> Result<String, EmitError> {
    let operand = |i: usize| -> Result<(&str, Width), EmitError> {
        match (d.steps.get(i), d.certs.get(i)) {
            (Some(s), Some(c)) => Ok((s.name.as_str(), style.held(c.width))),
            _ => fail("a step's operand is missing"),
        }
    };
    let w = style.held(cert.width);
    let body = match &step.kind {
        StepKind::LoadCoord { point, axis } => coordinate(point, *axis, w)?,
        StepKind::Translate {
            minuend,
            subtrahend,
            axis,
        } => checked(
            &coordinate(minuend, *axis, w)?,
            Op::Sub,
            &coordinate(subtrahend, *axis, w)?,
            w,
        ),
        StepKind::LoadRounded { input } => {
            let bits = rounded_bits(d, input)?;
            match w {
                Width::I64 => format!("i64::try_from(within({input}, {bits})?).ok()?"),
                Width::I128 => format!("within({input}, {bits})?"),
                _ => return fail("a rounded input wider than i128"),
            }
        }
        StepKind::Op {
            op: Op::Mul,
            left,
            right,
        } if w > Width::I128 && style == Style::WidthAware => {
            let (l, lw) = operand(*left)?;
            let (r, rw) = operand(*right)?;
            wide_product(l, lw, r, rw, w)?
        }
        StepKind::Op { op, left, right } => {
            let (l, lw) = operand(*left)?;
            let (r, rw) = operand(*right)?;
            checked(&widened(l, lw, w)?, *op, &widened(r, rw, w)?, w)
        }
    };
    Ok(format!("    let {} = {body};", step.name))
}

/// `name`, of type `from` (`i64` or `i128`), as an `i128`.
fn as_i128(name: &str, from: Width) -> Result<String, EmitError> {
    match from {
        Width::I64 => Ok(format!("i128::from({name})")),
        Width::I128 => Ok(name.to_owned()),
        _ => fail(format!("`{name}` is wider than an `i128`")),
    }
}

/// A product in a wide type `w`, width-aware (chapter 23.3): two operands that fit `i128` as
/// `W::product_i128(l, r)`, four 64 × 64 → 128-bit limb products; one operand that fits `i128`
/// times the other, widened to `w`, as `x.checked_mul_i128(n)`, two limb products per limb; two
/// wide operands as `checked_mul`. Each gives the exact product or `None`, as `checked_mul` of the
/// widened operands does, so the value is the same and only the cost differs.
fn wide_product(l: &str, lw: Width, r: &str, rw: Width, w: Width) -> Result<String, EmitError> {
    let narrow = |width: Width| width <= Width::I128;
    Ok(match (narrow(lw), narrow(rw)) {
        (true, true) => format!(
            "{}::product_i128({}, {})?",
            w.rust(),
            as_i128(l, lw)?,
            as_i128(r, rw)?
        ),
        (true, false) => format!(
            "{}.checked_mul_i128({})?",
            widened(r, rw, w)?,
            as_i128(l, lw)?
        ),
        (false, true) => format!(
            "{}.checked_mul_i128({})?",
            widened(l, lw, w)?,
            as_i128(r, rw)?
        ),
        (false, false) => checked(&widened(l, lw, w)?, Op::Mul, &widened(r, rw, w)?, w),
    })
}

/// The filter's statement for one step.
fn filter_statement(step: &Step, d: &Derivation) -> Result<String, EmitError> {
    let name = |i: &usize| -> Result<&str, EmitError> {
        match d.steps.get(*i) {
            Some(s) => Ok(s.name.as_str()),
            None => fail("a step's operand is missing"),
        }
    };
    let body = match &step.kind {
        StepKind::LoadCoord { point, axis } => format!("exact_f64({point}.{})", axis.letter()),
        StepKind::Translate {
            minuend,
            subtrahend,
            axis,
        } => format!(
            "exact_f64({minuend}.{a}) - exact_f64({subtrahend}.{a})",
            a = axis.letter()
        ),
        StepKind::LoadRounded { input } => {
            format!("rounded_f64({input}, {})?", rounded_bits(d, input)?)
        }
        StepKind::Op { op, left, right } => {
            format!("{} {} {}", name(left)?, op.symbol(), name(right)?)
        }
    };
    Ok(format!("    let {} = {body};", step.name))
}

/// Whether `name` is lowercase snake case, as every name the module binds must be.
fn snake(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.contains("__")
        && !name.ends_with('_')
}

/// Refuses a derivation whose names the module could not hold (module header): every name bound
/// in `filter` or `value` (parameters, steps, group maxima, `scale`) and the predicate's own name
/// must be snake case, not a Rust keyword or primitive, not one of the module's items or
/// imports, and bound once, except that a rounded input's load deliberately shadows its
/// parameter.
fn check_names(d: &Derivation, imports: &[&str]) -> Result<(), EmitError> {
    let name = &d.description.name;
    let items = ["filter", "value", "exact", "ERROR"];
    let clash = |n: &str| -> Option<&'static str> {
        if !snake(n) {
            Some("not lowercase snake case")
        } else if RUST_WORDS.contains(&n) {
            Some("a Rust keyword or primitive type")
        } else if items.contains(&n) || imports.contains(&n) {
            Some("an item or import of the module")
        } else {
            None
        }
    };
    if let Some(why) = clash(name) {
        return fail(format!("the predicate's name `{name}` is {why}"));
    }
    let mut bound: Vec<String> = d
        .description
        .inputs
        .iter()
        .map(|i| i.name.clone())
        .collect();
    for s in &d.steps {
        let shadows_its_parameter =
            matches!(&s.kind, StepKind::LoadRounded { input } if *input == s.name);
        if !shadows_its_parameter {
            bound.push(s.name.clone());
        }
    }
    bound.extend(d.groups.iter().map(|g| max_name(&g.name)));
    bound.push("scale".to_owned());
    for (i, n) in bound.iter().enumerate() {
        if let Some(why) = clash(n) {
            return fail(format!("`{name}`: the name `{n}` is {why}"));
        }
        if bound.iter().skip(i.saturating_add(1)).any(|m| m == n) {
            return fail(format!("`{name}`: the name `{n}` would be bound twice"));
        }
    }
    Ok(())
}

/// Whether a generated function needs the `many_single_char_names` allowance: more than four
/// one-letter names in scope, clippy's default threshold, counting the inputs and the steps the
/// function binds (a one-letter step, such as `ssi_weight`'s `w`, is in scope too).
fn many_single_char_names(d: &Derivation) -> bool {
    let inputs = d
        .description
        .inputs
        .iter()
        .filter(|i| i.name.chars().count() == 1)
        .count();
    let steps = d
        .steps
        .iter()
        .filter(|s| s.name.chars().count() == 1)
        .count();
    inputs.saturating_add(steps) > 4
}

/// The allowances a generated function needs: one-letter point names past clippy's threshold,
/// the description's step names (`steps`), which differ by a letter (`cd_x`, `ce_x`), more
/// than seven parameters, and a body past clippy's hundred lines (`body_lines`).
fn allowances(out: &mut String, d: &Derivation, steps: bool, body_lines: usize) {
    let mut lints: Vec<&str> = Vec::new();
    let mut reasons: Vec<&str> = Vec::new();
    if many_single_char_names(d) {
        lints.push("clippy::many_single_char_names");
        reasons.push("one-letter names are the description's points");
    }
    if steps {
        lints.push("clippy::similar_names");
        reasons.push("each name is a step of the description, named by the language's rule");
    }
    if d.description.inputs.len() > 7 {
        lints.push("clippy::too_many_arguments");
        reasons.push("one parameter per input of the description");
    }
    if body_lines > 100 {
        lints.push("clippy::too_many_lines");
        reasons.push("one line per step of the description");
    }
    let reason = match reasons.as_slice() {
        [] => return,
        [one] => *one,
        _ => "names, parameters and lines follow the description: one per input and step",
    };
    line(out, "#[allow(");
    for lint in lints {
        line(out, &format!("    {lint},"));
    }
    line(out, &format!("    reason = \"{reason}\""));
    line(out, ")]");
}

/// The widest doc line: rustfmt's default `max_width`.
const WIDTH: usize = 100;

/// A paragraph of comment lines starting with `lead` (`//!` or `///`), its words wrapped so that
/// no line passes [`WIDTH`] columns unless one word does; `first` begins the first line and
/// `rest` every other (`- ` and two spaces for a list item).
fn para(out: &mut String, lead: &str, first: &str, rest: &str, text: &str) {
    let mut current = format!("{lead} {first}");
    let mut empty = true;
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        let needed = current
            .chars()
            .count()
            .saturating_add(usize::from(!empty))
            .saturating_add(word.chars().count());
        if !empty && needed > WIDTH {
            line(out, &current);
            current = format!("{lead} {rest}");
            empty = true;
        }
        if !empty {
            current.push(' ');
        }
        current.push_str(word);
        empty = false;
    }
    line(out, current.trim_end());
}

/// A `//!` paragraph.
fn doc_para(out: &mut String, text: &str) {
    para(out, "//!", "", "", text);
}

/// A `///` paragraph.
fn item_para(out: &mut String, text: &str) {
    para(out, "///", "", "", text);
}

/// The number of `f64` additions, subtractions and multiplications the filter's steps perform
/// (a translation is one subtraction; a load is a conversion, not counted here).
fn arithmetic_steps(d: &Derivation) -> usize {
    d.steps
        .iter()
        .filter(|s| matches!(s.kind, StepKind::Translate { .. } | StepKind::Op { .. }))
        .count()
}

/// The certificate and error-analysis header of a predicate module.
#[allow(
    clippy::too_many_lines,
    reason = "one straight sequence of the header's sections, in the order they are written"
)]
fn header(
    out: &mut String,
    d: &Derivation,
    generator: &str,
    constant_name: &str,
    serves: &str,
) -> Result<(), EmitError> {
    let groups = group_names(d);
    doc_para(
        out,
        &format!(
            "Generated by geom-gen, generator `{generator}` (seed: none, it uses no randomness), \
             from the description `{constant_name}` in `{DESCRIPTIONS_PATH}`. Do not edit: change \
             the description or the generator and run `cargo run -p geom-gen -- write`. CI runs \
             `cargo run -p geom-gen -- check` and fails on any difference."
        ),
    );
    doc(out, "");
    doc_para(
        out,
        &format!(
            "The `f64` filter and the exact path of {serves}. The proof and the guarantees common \
             to every generated predicate are in [`super`]."
        ),
    );
    doc(out, "");
    doc_para(
        out,
        &format!(
            "Guarantees: when `filter` returns `Some(sign)`, `sign` is the sign of the exact value \
             for every input in the domain below, and it returns `None` when it cannot tell, never \
             a guess; `value` returns the exact value for every input in the domain, each step in \
             the type the certificate below shows holds it and every step a checked method, so a \
             step that did not fit would give `None`, never a wrapped value; and `exact` is that \
             value's sign. {}",
            proof_sentence(d)?
        ),
    );
    doc(out, "");
    doc_para(
        out,
        "Method: Meyer and Pion's semi-static filters (FPG, \
         <https://inria.hal.science/inria-00344297>), with the error analysis derived exactly, as \
         [`super`] sets out, and the exact path in integers.",
    );
    doc(out, "");
    doc(out, "The description:");
    doc(out, "");
    doc(out, "```text");
    for l in d.text.lines() {
        doc(out, l);
    }
    doc(out, "```");
    doc(out, "");
    doc(out, "# Bit-range certificate");
    doc(out, "");
    let mut domain = Vec::new();
    if point_type(d)?.is_some() {
        domain.push(format!(
            "Every point input is a grid point, each coordinate with |v| ≤ 2^{COORD_BITS} − 1 = \
             {COORD_MAX} (`grid::COORD_MAX`)."
        ));
    }
    if has_rounded(d) {
        domain.push(
            "Every rounded input is an integer with |v| ≤ 2^bits − 1 for the bits it declares, \
             passed exactly as an `i128`; `filter` and `value` return `None` for one outside that \
             range, and `filter` uses its nearest `f64`."
                .to_owned(),
        );
    }
    doc_para(
        out,
        &format!(
            "{} Over every such input, each step's exact value v has |v| at most its bound, which \
             is below 2^bits, and the exact path computes it in the type shown, which holds every \
             magnitude up to 2^(64 · limbs − 1) − 1 (`i64`: 2^63 − 1; `i128`: 2^127 − 1).",
            domain.join(" ")
        ),
    );
    doc(out, "");
    doc(
        out,
        "| Step | Computes | Bound on \\|v\\| | Bits | Exact path |",
    );
    doc(out, "| --- | --- | ---: | ---: | --- |");
    for (step, cert) in d.steps.iter().zip(&d.certs) {
        let bound = cert
            .bound
            .to_decimal()
            .ok_or_else(|| EmitError("a bound's decimal".to_owned()))?;
        doc(
            out,
            &format!(
                "| `{}` | `{}` | {bound} | {} | `{}` |",
                step.name,
                step_text(step, &d.steps),
                cert.bits,
                cert.width.rust()
            ),
        );
    }
    doc(out, "");
    doc(out, "# Filter error analysis");
    doc(out, "");
    let mut group_list = Vec::new();
    for g in &d.groups {
        let vars: Vec<String> = g
            .vars
            .iter()
            .filter_map(|&v| d.steps.get(v).map(|s| format!("`{}`", s.name)))
            .collect();
        group_list.push(format!("{} ({})", g.name, vars.join(", ")));
    }
    doc_para(
        out,
        &format!(
            "Groups and their scale variables: {}. M(g) is the largest |v| among group g's \
             computed scale variables, and S the largest product of group maxima over a step's \
             scale. For each step, |computed| ≤ B · S and |computed − exact| ≤ E · S, with \
             u = 2^-53 (the proof is in [`super`]).",
            group_list.join("; ")
        ),
    );
    doc(out, "");
    doc(out, "| Step | Scale | Exact in f64 | B | E |");
    doc(out, "| --- | --- | --- | --- | --- |");
    for (step, cert) in d.steps.iter().zip(&d.certs) {
        doc(
            out,
            &format!(
                "| `{}` | {} | {} | {} | {} |",
                step.name,
                scale_text(&cert.scale, &groups),
                if cert.f64_exact { "yes" } else { "no" },
                cert.unit_bound,
                cert.unit_error
            ),
        );
    }
    doc(out, "");
    let Some((root_step, root)) = d.steps.get(d.result).zip(d.certs.get(d.result)) else {
        return fail("the result step is missing");
    };
    let scale_roundings = d
        .scale_expr
        .roundings()
        .ok_or_else(|| EmitError("the scale's roundings".to_owned()))?;
    doc_para(
        out,
        &format!(
            "The filter computes the scale S at run time at least (1 − u)^{scale_roundings} times \
             its exact value: {scale_roundings} rounding(s), each product counting its own and \
             both its operands', each `larger` the larger of its operands'. It computes the bound \
             as the constant times S, one more rounding: k = {scale_roundings} + 1. The constant \
             is E · (1 + 2u)^k, rounded up to an `f64`."
        ),
    );
    doc(out, "");
    doc(out, "| Quantity | Value |");
    doc(out, "| --- | --- |");
    doc(out, &format!("| Result | `{}` |", root_step.name));
    doc(out, &format!("| Unit error E | {} |", root.unit_error));
    doc(
        out,
        &format!("| Scale S | {} |", scale_text(&root.scale, &groups)),
    );
    doc(
        out,
        &format!(
            "| Scale at run time | `{}` |",
            scale_rust(&d.scale_expr, &groups)?
        ),
    );
    doc(out, &format!("| Roundings k | {} |", d.roundings));
    doc(out, &format!("| Constant, exactly | {} |", d.constant));
    doc(
        out,
        &format!(
            "| Constant as an `f64` | `{:#018x}` = {} ≈ {:e} |",
            d.constant_bits,
            f64_exact_text(d.constant_bits),
            f64::from_bits(d.constant_bits)
        ),
    );
    Ok(())
}

/// One predicate's module, its header saying it is the predicate `<name>`.
///
/// # Errors
///
/// As [`predicate_module_serving`].
pub fn predicate_module(
    d: &Derivation,
    generator: &str,
    constant_name: &str,
) -> Result<String, EmitError> {
    let serves = format!("the predicate `{}`", d.description.name);
    predicate_module_serving(d, generator, constant_name, &serves)
}

/// One predicate's module; `serves` says, in its header, what the module evaluates and what calls
/// it (Markdown, for the sentence "The `f64` filter and the exact path of …").
///
/// # Errors
///
/// An [`EmitError`] for a description this emitter does not write (point inputs of mixed
/// dimensions, a rounded input of more than 127 bits), or whose names the module could not hold.
#[allow(
    clippy::too_many_lines,
    reason = "one straight sequence of the module's items, in the order they are written"
)]
pub fn predicate_module_serving(
    d: &Derivation,
    generator: &str,
    constant_name: &str,
    serves: &str,
) -> Result<String, EmitError> {
    let ty = point_type(d)?;
    let name = &d.description.name;
    let groups = group_names(d);
    let scale = scale_rust(&d.scale_expr, &groups)?;

    // Imports, which the name check needs.
    let mut wides: Vec<&str> = d
        .certs
        .iter()
        .filter(|c| c.width > Width::I128)
        .map(|c| c.width.rust())
        .collect();
    wides.sort_unstable();
    wides.dedup();
    let mut helpers = vec!["decide"];
    if has_coordinates(d) {
        helpers.push("exact_f64");
    }
    if scale.contains("larger(") {
        helpers.push("larger");
    }
    helpers.push("largest_magnitude");
    if has_rounded(d) {
        helpers.push("rounded_f64");
        helpers.push("within");
    }
    let mut imports: Vec<&str> = vec!["Sign"];
    imports.extend(ty);
    imports.extend(wides.iter().copied());
    imports.extend(helpers.iter().copied());
    check_names(d, &imports)?;

    let mut out = String::new();
    header(&mut out, d, generator, constant_name, serves)?;
    doc(&mut out, "");
    let magnitudes = d
        .groups
        .iter()
        .map(|g| g.vars.len())
        .fold(0usize, usize::saturating_add);
    let multiplications = d
        .scale_expr
        .multiplications()
        .and_then(|m| m.checked_add(1))
        .ok_or_else(|| EmitError("the bound's multiplications".to_owned()))?;
    let conversions = d
        .steps
        .iter()
        .filter(|s| matches!(s.kind, StepKind::LoadRounded { .. }))
        .count();
    let converted = if conversions == 0 {
        String::new()
    } else {
        format!(", after {conversions} rounded input(s) converted to `f64`")
    };
    doc_para(
        &mut out,
        &format!(
            "Cost: the filter is {} `f64` operations{converted}, then {magnitudes} magnitudes and \
             their maxima and the bound's {multiplications} multiplication(s); the exact path, run \
             only when the filter cannot decide, is {} checked integer operations.",
            arithmetic_steps(d),
            arithmetic_steps(d)
        ),
    );
    line(&mut out, "");

    if let Some(t) = ty {
        line(&mut out, &format!("use crate::grid::{t};"));
    }
    line(&mut out, "use crate::sign::Sign;");
    line(&mut out, WIDE_IMPORTS);
    line(&mut out, "");
    line(&mut out, &format!("use super::{{{}}};", helpers.join(", ")));
    line(&mut out, "");

    // The constant.
    let Some((root_step, root)) = d.steps.get(d.result).zip(d.certs.get(d.result)) else {
        return fail("the result step is missing");
    };
    item_para(
        &mut out,
        &format!(
            "The filter's error constant: E · (1 + 2u)^{} with E = {}, the result's unit error \
             (header), rounded up to an `f64`: {}.",
            d.roundings,
            root.unit_error,
            f64_exact_text(d.constant_bits)
        ),
    );
    line(
        &mut out,
        &format!(
            "const ERROR: f64 = f64::from_bits({});",
            hex_literal(d.constant_bits)
        ),
    );
    line(&mut out, "");

    // The filter.
    let parameters = params(d, ty)?;
    item_para(
        &mut out,
        &format!(
            "`{name}`'s sign from its `f64` filter, or `None` when the filter's bound cannot \
             certify it."
        ),
    );
    line(&mut out, "///");
    let zero = if has_rounded(d) {
        "`Some(Sign::Zero)` only when the scale is zero, which makes the exact value zero; `None` \
         also for a rounded input outside its declared range."
    } else {
        "`Some(Sign::Zero)` only when the scale is zero, which makes the exact value zero."
    };
    item_para(&mut out, zero);
    line(&mut out, "#[inline]");
    line(&mut out, "#[must_use]");
    let scale_code = scale_lines(&d.scale_expr, &groups)?;
    let filter_lines = d
        .steps
        .len()
        .saturating_add(d.groups.len())
        .saturating_add(scale_code.len())
        .saturating_add(1);
    allowances(&mut out, d, true, filter_lines);
    signature(&mut out, "filter", &parameters, "Option<Sign>");
    for step in &d.steps {
        line(&mut out, &filter_statement(step, d)?);
    }
    for g in &d.groups {
        let vars: Vec<&str> = g
            .vars
            .iter()
            .filter_map(|&v| d.steps.get(v).map(|s| s.name.as_str()))
            .collect();
        line(
            &mut out,
            &format!(
                "    let {} = largest_magnitude([{}]);",
                max_name(&g.name),
                vars.join(", ")
            ),
        );
    }
    for l in &scale_code {
        line(&mut out, l);
    }
    line(
        &mut out,
        &format!("    decide({}, ERROR, scale)", root_step.name),
    );
    line(&mut out, "}");
    line(&mut out, "");

    // The exact path.
    value_item(&mut out, d, &parameters, false)?;
    item_para(
        &mut out,
        &format!("`{name}`'s exact sign: the sign of [`value`], or `None` as for [`value`]."),
    );
    line(&mut out, "#[inline]");
    line(&mut out, "#[must_use]");
    allowances(&mut out, d, false, 1);
    signature(&mut out, "exact", &parameters, "Option<Sign>");
    let sign = match root.width {
        Width::I64 => "Sign::of_i64",
        Width::I128 => "Sign::of_i128",
        _ => "|v| v.signum()",
    };
    value_call(&mut out, &args(d), sign)?;
    line(&mut out, "}");
    // rustfmt would rewrap a longer line of code, and the committed file would then fail
    // `cargo fmt --check`: refuse it here, naming the line.
    if let Some(long) = out
        .lines()
        .find(|l| !l.trim_start().starts_with("//") && l.chars().count() > WIDTH)
    {
        return fail(format!(
            "`{name}`: a line of code would pass {WIDTH} columns, which rustfmt rewraps: {long}"
        ));
    }
    Ok(wide_imports(&out, &wides))
}

/// The placeholder line [`predicate_module_serving`] writes where the module's wide-integer import
/// goes, replaced by [`wide_imports`] once the module's code is known.
const WIDE_IMPORTS: &str = "// geom-gen: wide imports";

/// Replaces the [`WIDE_IMPORTS`] line with an import of exactly the wide-integer types the code
/// names, so a module imports no type it does not use. A step's certified width can exceed the
/// type its code names, when the step's value is formed by a method of a narrower type, so the
/// candidates (every width a step needs) are filtered by whether the code outside doc comments
/// names them as a whole word.
fn wide_imports(module: &str, candidates: &[&str]) -> String {
    let code: Vec<&str> = module
        .lines()
        .filter(|l| !l.trim_start().starts_with("//") && *l != WIDE_IMPORTS)
        .collect();
    let named = |ty: &str| {
        code.iter().any(|l| {
            l.match_indices(ty).any(|(at, _)| {
                let before = l.get(..at).and_then(|head| head.chars().next_back());
                let after = l
                    .get(at..)
                    .and_then(|tail| tail.strip_prefix(ty))
                    .and_then(|tail| tail.chars().next());
                !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                    && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
            })
        })
    };
    let used: Vec<&str> = candidates.iter().copied().filter(|ty| named(ty)).collect();
    let import = match used.as_slice() {
        [] => None,
        [one] => Some(format!("use crate::wide::{one};")),
        many => Some(format!("use crate::wide::{{{}}};", many.join(", "))),
    };
    let mut out = String::with_capacity(module.len());
    for l in module.split_inclusive('\n') {
        if l.trim_end_matches('\n') == WIDE_IMPORTS {
            if let Some(import) = &import {
                out.push_str(import);
                out.push('\n');
            }
        } else {
            out.push_str(l);
        }
    }
    out
}

/// The exact path's item, `value`, as the module ships it. With `twin`, also its Verus
/// annotations (chapter 28.7): a `#[verus_spec]` attribute after its other attributes, saying that
/// for inputs on the grid it returns `Some` of the description's value over the integers, and
/// before each product a `proof!` block bounding the product by its operands' certified bounds.
/// Apart from those, the text is the same either way, so the twin's `value` is the shipped one token
/// for token (`tests/verus_twin.rs` checks the committed files).
fn value_item(
    out: &mut String,
    d: &Derivation,
    parameters: &[String],
    twin: bool,
) -> Result<(), EmitError> {
    value_fn(out, d, parameters, twin, Style::WidthAware, "value")
}

/// The exact path's function, named `fn_name`, in `style`; with `twin`, its Verus annotations, as
/// [`value_item`] describes.
fn value_fn(
    out: &mut String,
    d: &Derivation,
    parameters: &[String],
    twin: bool,
    style: Style,
    fn_name: &str,
) -> Result<(), EmitError> {
    let name = &d.description.name;
    let Some((root_step, root)) = d.steps.get(d.result).zip(d.certs.get(d.result)) else {
        return fail("the result step is missing");
    };
    let range = if has_rounded(d) {
        "; `None` also for a rounded input outside its declared range"
    } else {
        ""
    };
    let text = match style {
        Style::Limbs => return fail("the limb variant is written by `limbs_fn`"),
        Style::WidthAware => format!(
            "`{name}`'s exact value: each step in the type its certificate gives it, with checked \
             arithmetic; `None` only if a step overflowed, which the certificate rules out{range}."
        ),
        Style::Plain => format!(
            "`{name}`'s exact value in plain `i128` (chapter 23.3): each step in `i128`, or in its \
             certificate's wide type with both operands widened to it, with checked arithmetic; \
             `None` only if a step overflowed, which the certificate rules out{range}."
        ),
    };
    item_para(out, &text);
    line(out, "#[inline]");
    line(out, "#[must_use]");
    allowances(out, d, true, d.steps.len().saturating_add(1));
    if twin {
        verus_spec(out, d)?;
    }
    signature(
        out,
        fn_name,
        parameters,
        &format!("Option<{}>", style.held(root.width).rust()),
    );
    let mut widths_seen = Vec::new();
    for (step, cert) in d.steps.iter().zip(&d.certs) {
        if twin && cert.width > Width::I128 && !widths_seen.contains(&cert.width) {
            widths_seen.push(cert.width);
            line(
                out,
                &format!("    proof! {{ lemma_half_base_{}(); }}", cert.width.limbs()),
            );
        }
        if let (true, StepKind::LoadRounded { input }) = (twin, &step.kind) {
            let bits = rounded_bits(d, input)?;
            let limit = Nat::pow2(bits)
                .and_then(|p| p.to_decimal())
                .ok_or_else(|| EmitError("a range's decimal".to_owned()))?;
            line(
                out,
                &format!(
                    "    proof! {{ assert(1u128 << {bits}u32 == {limit}u128) by (bit_vector); }}"
                ),
            );
        }
        if let (
            true,
            StepKind::Op {
                op: Op::Mul,
                left,
                right,
            },
        ) = (twin, &step.kind)
        {
            line(out, &product_proof(d, *left, *right)?);
        }
        line(out, &exact_statement(step, cert, d, style)?);
        if twin && matches!(step.kind, StepKind::Op { .. }) {
            let v = if cert.width > Width::I128 {
                format!("{}.value()", step.name)
            } else {
                format!("{} as int", step.name)
            };
            line(
                out,
                &format!(
                    "    proof! {{ assert(-bound_of_{n}() <= {v} <= bound_of_{n}()); }}",
                    n = step.name
                ),
            );
        }
    }
    line(out, &format!("    Some({})", root_step.name));
    line(out, "}");
    line(out, "");
    Ok(())
}

/// The coordinates of a point type, in order.
fn axes_of(ty: &str) -> &'static [&'static str] {
    if ty == "P3" {
        &["x", "y", "z"]
    } else {
        &["x", "y"]
    }
}

/// The twin's `#[verus_spec]`: every coordinate of every point input on the grid, and the
/// result `Some` of the description's value over the integers (`spec_value`). With rounded inputs,
/// the result is `Some` of that value exactly when every rounded input is within its declared
/// range (`in_range`, the specification of the twin of `within`), and `None` otherwise.
fn verus_spec(out: &mut String, d: &Derivation) -> Result<(), EmitError> {
    let ty = point_type(d)?;
    let mut grid = Vec::new();
    let mut ranges = Vec::new();
    for input in &d.description.inputs {
        match (&input.kind, ty) {
            (InputKind::Point { .. }, Some(t)) => {
                for axis in axes_of(t) {
                    grid.push(format!("{}.{axis}.on_grid()", input.name));
                }
            }
            (InputKind::Point { .. }, None) => return fail("a point input without a point type"),
            (InputKind::Rounded { .. }, _) => ranges.push(format!(
                "in_range({}, {})",
                input.name,
                rounded_bits(d, &input.name)?
            )),
        }
    }
    line(out, "#[verus_spec(r =>");
    if !grid.is_empty() {
        line(out, "    requires");
        for g in &grid {
            line(out, &format!("        {g},"));
        }
    }
    let Some(root) = d.certs.get(d.result) else {
        return fail("the result step is missing");
    };
    let result = if root.width > Width::I128 {
        "v.value()"
    } else {
        "v as int"
    };
    line(out, "    ensures");
    if ranges.is_empty() {
        line(
            out,
            &format!(
                "        r matches Some(v) && {result} == spec_value({}),",
                args(d)
            ),
        );
    } else {
        let all = ranges.join(" && ");
        line(out, "        match r {");
        line(
            out,
            &format!(
                "            Some(v) => {all} && {result} == spec_value({}),",
                args(d)
            ),
        );
        line(out, &format!("            None => !({all}),"));
        line(out, "        },");
    }
    line(out, ")]");
    Ok(())
}

/// Whether the twin states step `i`'s certified bound right after computing it: every operation
/// does, so that each later step reasons from its operands' bounds alone, not back through the
/// whole computation (a long exact path otherwise defeats the solver).
fn bounded(d: &Derivation, i: usize) -> bool {
    d.steps
        .get(i)
        .is_some_and(|s| matches!(s.kind, StepKind::Op { .. }))
}

/// The twin's hint before a product: the product of the steps `left` and `right` is bounded by
/// the product of their certified bounds (`lemma_product_bound`), which Verus's linear arithmetic
/// cannot see alone. Each bound is the twin's spec function `bound_of_<step>` ([`bound_specs`]),
/// since a bound can be wider than any Rust integer literal outside the `verus!` block.
fn product_proof(d: &Derivation, left: usize, right: usize) -> Result<String, EmitError> {
    let operand = |i: usize| -> Result<(String, String), EmitError> {
        match (d.steps.get(i), d.certs.get(i)) {
            (Some(s), Some(c)) => Ok((
                if c.width > Width::I128 {
                    format!("{}.value()", s.name)
                } else {
                    format!("{} as int", s.name)
                },
                format!("bound_of_{}()", s.name),
            )),
            _ => fail("a step's operand is missing"),
        }
    };
    let (l, lb) = operand(left)?;
    let (r, rb) = operand(right)?;
    // In the order the code multiplies: a narrow left factor times a wide right one is written
    // `right.checked_mul_i128(left)` ([`wide_product`]), whose value is right · left.
    let narrow = |i: usize| d.certs.get(i).is_some_and(|c| c.width <= Width::I128);
    if narrow(left) && !narrow(right) {
        return Ok(format!(
            "    proof! {{ lemma_product_bound({r}, {l}, {rb}, {lb}); }}"
        ));
    }
    Ok(format!(
        "    proof! {{ lemma_product_bound({l}, {r}, {lb}, {rb}); }}"
    ))
}

/// The twin's spec functions `bound_of_<step>`, one for each step that is a factor of a product or
/// an operation ([`bounded`]), in the order the steps come: the step's certified bound,
/// |value| ≤ it, in decimal.
fn bound_specs(d: &Derivation) -> Result<Vec<String>, EmitError> {
    let mut factors: Vec<usize> = (0..d.steps.len()).filter(|&i| bounded(d, i)).collect();
    for step in &d.steps {
        if let StepKind::Op {
            op: Op::Mul,
            left,
            right,
        } = &step.kind
        {
            for i in [*left, *right] {
                if !factors.contains(&i) {
                    factors.push(i);
                }
            }
        }
    }
    factors.sort_unstable();
    let mut out = Vec::new();
    for i in factors {
        let (Some(step), Some(cert)) = (d.steps.get(i), d.certs.get(i)) else {
            return fail("a step's operand is missing");
        };
        let bound = cert
            .bound
            .to_decimal()
            .ok_or_else(|| EmitError("a bound's decimal".to_owned()))?;
        out.push(format!("/// `{}`'s certified bound.", step.name));
        out.push(format!(
            "pub open spec fn bound_of_{}() -> int {{",
            step.name
        ));
        out.push(format!("    {bound}int"));
        out.push("}".to_owned());
        out.push(String::new());
    }
    Ok(out)
}

/// The twin's integer value of one step, as Verus's `spec_value` computes it: the description's
/// arithmetic over the mathematical integers, where nothing overflows.
fn spec_expression(step: &Step, d: &Derivation) -> Result<String, EmitError> {
    let name = |i: usize| -> Result<&str, EmitError> {
        match d.steps.get(i) {
            Some(s) => Ok(s.name.as_str()),
            None => fail("a step's operand is missing"),
        }
    };
    Ok(match &step.kind {
        StepKind::LoadCoord { point, axis } => format!("{point}.{}@", axis.letter()),
        StepKind::Translate {
            minuend,
            subtrahend,
            axis,
        } => format!("{minuend}.{a}@ - {subtrahend}.{a}@", a = axis.letter()),
        StepKind::LoadRounded { input } => format!("{input} as int"),
        // A narrow left factor times a wide right one in the order the code multiplies them
        // (`product_proof`), so the solver need not reorder a product.
        StepKind::Op {
            op: Op::Mul,
            left,
            right,
        } if d.certs.get(*left).is_some_and(|c| c.width <= Width::I128)
            && d.certs.get(*right).is_some_and(|c| c.width > Width::I128) =>
        {
            format!("{} * {}", name(*right)?, name(*left)?)
        }
        StepKind::Op { op, left, right } => {
            format!("{} {} {}", name(*left)?, op.symbol(), name(*right)?)
        }
    })
}

/// A Verus-annotated twin of a predicate's exact path (chapter 28.7; pieces G36, G38 and G39): a
/// module of `geom-num`'s Verus crate (`verus/lib.rs`), which holds the model of the grid's types
/// (`model`), the twin of a rounded input's range check (`rounded`) and the wide integers' own
/// twin (`wide`). The module holds the description's value over the integers (`spec_value`), each
/// factor's and operation's certified bound (`bound_of_<step>`), and `value` exactly as the
/// shipped module (`shipped`, a repository path) writes it, with its annotations
/// ([`value_item`]).
///
/// # Errors
///
/// An [`EmitError`] for a description the twin does not cover: a step wider than `I384` (the
/// widest budgeted width), a wide step made from an `i64` operand (`from_i64` has no proof yet),
/// or no inputs.
#[allow(
    clippy::too_many_lines,
    reason = "one straight sequence of the twin's refusals, header and items, in the order they are written"
)]
pub fn verus_twin(d: &Derivation, generator: &str, shipped: &str) -> Result<String, EmitError> {
    let name = &d.description.name;
    if d.certs.iter().any(|c| c.width > Width::I384) {
        return fail(format!(
            "`{name}`: the twin covers exact paths up to I384, the widest budgeted width (chapter \
             6.2); the wide twin states the half base of 4, 5 and 6 limbs only"
        ));
    }
    for (step, cert) in d.steps.iter().zip(&d.certs) {
        if let StepKind::Op { left, right, .. } = &step.kind {
            for i in [*left, *right] {
                if cert.width > Width::I128 && d.certs.get(i).is_some_and(|c| c.width == Width::I64)
                {
                    return fail(format!(
                        "`{name}`: the step `{}` widens an i64 to I256, and `from_i64` has no \
                         proof yet",
                        step.name
                    ));
                }
            }
        }
    }
    let ty = point_type(d)?;
    if ty.is_none() && !has_rounded(d) {
        return fail(format!("`{name}`: a predicate with no inputs"));
    }
    let Some(root_step) = d.steps.get(d.result) else {
        return fail("the result step is missing");
    };
    let wide = d.certs.iter().any(|c| c.width > Width::I128);
    let parameters = params(d, ty)?;
    let proves = if has_rounded(d) {
        format!(
            "It proves that for every input whose coordinates are on the grid (|v| ≤ \
             2^{COORD_BITS} − 1), `value` returns `Some` of the description's value over the \
             mathematical integers exactly when every rounded input is within its declared range, \
             and `None` otherwise: no checked step overflows, and the result is the value the \
             description defines."
        )
    } else {
        format!(
            "It proves that for every input whose coordinates are on the grid (|v| ≤ \
             2^{COORD_BITS} − 1), `value` returns `Some` of the description's value over the \
             mathematical integers: no checked step overflows, and the result is the value the \
             description defines."
        )
    };
    let mut out = String::new();
    let paragraphs = [
        format!(
            "Generated by geom-gen, generator `{generator}` (seed: none, it uses no randomness), \
             from the description `{}` in `{DESCRIPTIONS_PATH}`. Do not edit: change the \
             description or the generator and run `cargo run -p geom-gen -- write`. CI runs `cargo \
             run -p geom-gen -- check` and fails on any difference.",
            name.to_uppercase()
        ),
        format!(
            "A Verus-annotated twin of `{name}`'s exact path, `value` in `{shipped}` (chapter 28.7 \
             of the kernel architecture, \"Predicates, by generated twins\"; pieces G36, G38 and \
             G39). {proves}"
        ),
        "The twin's `value`, with its `#[verus_spec]` attribute and its `proof!` blocks removed, \
         is token for token the shipped one: the same emitter code writes both, and geom-gen's \
         `tests/verus_twin.rs` compares the committed files. It is a module of `geom-num`'s \
         Verus crate, `verus/lib.rs`, whose `model` module models `geom_num::grid`'s types and \
         whose `wide` module is the wide integers' own proven twin."
            .to_owned(),
        "Verify: `verus --crate-type=lib projects/engines/geometry/crates/geom-num/verus/lib.rs`, \
         with the Verus release `kernel-dev.yml` pins; its `verus` job does this on every pull \
         request."
            .to_owned(),
    ];
    for (k, text) in paragraphs.iter().enumerate() {
        if k > 0 {
            line(&mut out, "//!");
        }
        doc_para(&mut out, text);
    }
    line(&mut out, "");
    line(&mut out, "use vstd::prelude::*;");
    line(&mut out, "");
    line(&mut out, "use crate::model::*;");
    if has_rounded(d) {
        line(&mut out, "use crate::rounded::*;");
    }
    if wide {
        line(&mut out, "use crate::wide::*;");
    }
    line(&mut out, "");
    line(&mut out, "verus! {");
    line(&mut out, "");
    line(
        &mut out,
        &format!(
            "/// `{name}`'s value over the integers, step by step as its description defines it."
        ),
    );
    line(
        &mut out,
        &format!(
            "pub open spec fn spec_value({}) -> int {{",
            parameters.join(", ")
        ),
    );
    for step in &d.steps {
        line(
            &mut out,
            &format!("    let {} = {};", step.name, spec_expression(step, d)?),
        );
    }
    line(&mut out, &format!("    {}", root_step.name));
    line(&mut out, "}");
    line(&mut out, "");
    for l in bound_specs(d)? {
        line(&mut out, &l);
    }
    line(&mut out, "} // verus!");
    line(&mut out, "");
    value_item(&mut out, d, &parameters, true)?;
    out.truncate(out.trim_end().len());
    out.push('\n');
    Ok(out)
}

/// The file of one exact-path variant (chapter 23.3; piece G42): for each predicate, in the order
/// given, its exact value as `<predicate>_value`, written in `style`. The module sits beside the
/// predicate modules (`plain.rs` for [`Style::Plain`]) and shares their helpers; the shipped
/// [`Style::WidthAware`] paths are the predicate modules themselves, so that style is refused here.
///
/// # Errors
///
/// An [`EmitError`] for [`Style::WidthAware`], for a description [`predicate_module`] refuses, or
/// for a line of code past [`WIDTH`] columns.
pub fn variant_module(
    derivations: &[&Derivation],
    generator: &str,
    style: Style,
) -> Result<String, EmitError> {
    let (file, what) = match style {
        Style::Plain => (
            "plain",
            "the plain-`i128` variant: each step in `i128`, or in its certificate's wide type when \
             it does not fit 127 bits, with both operands widened to that type and the general \
             `checked_mul`, so no product is narrower than its result's type. On WebAssembly \
             every product then goes through `__multi3`, the library routine Rust's `i128` \
             multiply becomes there",
        ),
        Style::Limbs => (
            "limbs",
            "the width-aware limb variant: each step in `i64` when its certificate fits 63 bits, \
             otherwise in a `Wide32` of as many 32-bit limbs as its operands' certified bits need, \
             so every limb product is one 32 × 32 → 64-bit multiply, native on WebAssembly, and \
             operands of different widths multiply without being widened first",
        ),
        Style::WidthAware => {
            return fail("the width-aware exact paths are the predicate modules themselves");
        }
    };
    let mut out = String::new();
    variant_header(&mut out, generator, what, style);
    let mut points: Vec<&str> = Vec::new();
    let mut wides: Vec<&str> = Vec::new();
    for d in derivations {
        points.extend(point_type(d)?);
        if style == Style::Limbs {
            // The limb variant names a wide type only as the shipped path's result.
            let root = d.certs.get(d.result).map(|c| c.width);
            wides.extend(root.filter(|w| *w > Width::I128).map(Width::rust));
        } else {
            wides.extend(
                d.certs
                    .iter()
                    .map(|c| style.held(c.width))
                    .filter(|w| *w > Width::I128)
                    .map(Width::rust),
            );
        }
    }
    points.sort_unstable();
    points.dedup();
    wides.sort_unstable();
    wides.dedup();
    match points.as_slice() {
        [] => {}
        [one] => line(&mut out, &format!("use crate::grid::{one};")),
        many => line(
            &mut out,
            &format!("use crate::grid::{{{}}};", many.join(", ")),
        ),
    }
    line(&mut out, WIDE_IMPORTS);
    if style == Style::Limbs {
        line(&mut out, "use crate::wide32::Wide32;");
    }
    if derivations.iter().any(|d| has_rounded(d)) {
        line(&mut out, "");
        line(&mut out, "use super::within;");
    }
    for d in derivations {
        let mut imports: Vec<&str> = Vec::new();
        imports.extend(point_type(d)?);
        imports.extend(wides.iter().copied());
        if has_rounded(d) {
            imports.push("within");
        }
        if style == Style::Limbs {
            imports.push("Wide32");
        }
        check_names(d, &imports)?;
        // Each function ends with an empty line, so only the first needs one before it.
        if !out.ends_with("\n\n") {
            line(&mut out, "");
        }
        let ty = point_type(d)?;
        let parameters = params(d, ty)?;
        let name = format!("{}_value", d.description.name);
        if style == Style::Limbs {
            limbs_fn(&mut out, d, &parameters, &name)?;
        } else {
            value_fn(&mut out, d, &parameters, false, style, &name)?;
        }
    }
    if style == Style::Limbs {
        proofs_module(&mut out, derivations)?;
    }
    // value_fn ends each function with an empty line: the file ends with the last brace.
    while out.ends_with("\n\n") {
        out.pop();
    }
    if let Some(long) = out
        .lines()
        .find(|l| !l.trim_start().starts_with("//") && l.chars().count() > WIDTH)
    {
        return fail(format!(
            "`{file}`: a line of code would pass {WIDTH} columns, which rustfmt rewraps: {long}"
        ));
    }
    Ok(wide_imports(&out, &wides))
}

/// The header of a variant module: where it comes from, what `what` (the variant, from its
/// name to the end of a sentence) computes, and its guarantees, method and cost.
fn variant_header(out: &mut String, generator: &str, what: &str, style: Style) {
    let limbs = style == Style::Limbs;
    doc_para(
        out,
        &format!(
            "Generated by geom-gen, generator `{generator}` (seed: none, it uses no randomness), \
             from the descriptions in `{DESCRIPTIONS_PATH}` and `{EMITTER_PATH}`. Do not edit: \
             change the generator and run `cargo run -p geom-gen -- write`. CI runs `cargo run -p \
             geom-gen -- check` and fails on any difference."
        ),
    );
    doc(out, "");
    doc_para(
        out,
        &format!(
            "The exact paths of the generated predicates in {what} (chapter 23.3 of the kernel \
             architecture, `projects/engines/geometry/architecture/23-performance.md`; piece \
             G42). For each predicate, `<predicate>_value` performs the steps of the predicate \
             module's `value`, in their order and with their names, each held in the type above, \
             so it returns the same value, {returned}. The benchmarks compare the two on every \
             target, and the tests check that they agree on every input they draw and at the \
             grid's extremes.",
            returned = if limbs {
                "converted back to the shipped path's type"
            } else {
                "as an `i128` where the shipped path returns an `i64`"
            }
        ),
    );
    doc(out, "");
    doc_para(
        out,
        &format!(
            "Guarantees: as each predicate module's `value`. Every step is a checked method, the \
             certificate in the predicate module's header bounds every step within the type it is \
             held in here (a type at least as wide as the one the certificate names), so a step \
             never returns `None` for an input in the domain; nothing depends on overflow checks \
             being on. geom-gen's independent checker re-derives every line from its own \
             derivation{proofs}.",
            proofs = if limbs {
                ", and the bounded proofs at the end of the module show, for every kind of limb \
                 step, that it never returns `None` on any operand its certified bits allow"
            } else {
                ""
            }
        ),
    );
    doc(out, "");
    if limbs {
        doc_para(
            out,
            "Method: each predicate's steps as its module's certificate derives them, each held in \
             as many 32-bit limbs as its operands' certified bits need, chapter 23.3's width-aware \
             limbs, `projects/engines/geometry/architecture/23-performance.md`, with the \
             schoolbook limb arithmetic of `crate::wide32`.",
        );
        doc(out, "");
        doc_para(
            out,
            "Cost: the predicate module's steps; a product of operands of A and B limbs is A · B \
             32 × 32 → 64-bit limb multiplies, and the result's conversion back is linear in its \
             limbs.",
        );
    } else {
        doc_para(
            out,
            "Method: each predicate's steps as its module's certificate derives them, held as \
             chapter 23.3 describes the baseline of its comparison of exact-path variants, \
             `projects/engines/geometry/architecture/23-performance.md`.",
        );
        doc(out, "");
        doc_para(
            out,
            "Cost: the predicate module's `value`, with each product at the width of its result.",
        );
    }
    line(out, "");
}

/// How the limb variant holds a step (piece G42).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Held {
    /// An `i64`.
    I64,
    /// A `Wide32` of this many 32-bit limbs.
    Limbs(usize),
}

/// The 32-bit limbs that hold every value of magnitude below 2^`bits`: ⌈(bits + 1)/32⌉, at least
/// two.
fn limbs_for(bits: u32) -> Result<usize, EmitError> {
    let limbs = bits
        .checked_add(32)
        .and_then(|b| b.checked_div(32))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| EmitError(format!("{bits} bits in limbs")))?;
    Ok(limbs.max(2))
}

/// The certified bits of step `i`.
fn bits_of(d: &Derivation, i: usize) -> Result<u32, EmitError> {
    d.certs
        .get(i)
        .map(|c| c.bits)
        .ok_or_else(|| EmitError("a step's operand is missing".to_owned()))
}

/// How the limb variant holds each step, in step order (piece G42): in `i64` where the certificate
/// gives `i64`; otherwise a rounded input of b bits in ⌈(b + 1)/32⌉ limbs, a product of operands of
/// a and b certified bits in ⌈(a + b + 1)/32⌉, and a sum or difference in ⌈(max(a, b) + 2)/32⌉. A
/// product's magnitude is below 2^(a + b) and a sum's below 2^(max(a, b) + 1), so each holds the
/// step's certified bound; sizing from the operands' bits rather than from the step's own bound
/// lets one proof of each kind of step ([`limb_proofs`]) cover every step of that kind.
///
/// # Errors
///
/// An [`EmitError`] for a coordinate or translation the certificate does not fit in `i64`, or a
/// step whose operand is missing.
pub fn limbs_held(d: &Derivation) -> Result<Vec<Held>, EmitError> {
    let mut held = Vec::new();
    for (step, cert) in d.steps.iter().zip(&d.certs) {
        if cert.width == Width::I64 {
            held.push(Held::I64);
            continue;
        }
        let bits = match &step.kind {
            StepKind::LoadRounded { input } => Some(rounded_bits(d, input)?),
            StepKind::Op {
                op: Op::Mul,
                left,
                right,
            } => bits_of(d, *left)?.checked_add(bits_of(d, *right)?),
            StepKind::Op { left, right, .. } => {
                bits_of(d, *left)?.max(bits_of(d, *right)?).checked_add(1)
            }
            _ => return fail("a coordinate or translation wider than i64"),
        }
        .ok_or_else(|| EmitError("a step's bits".to_owned()))?;
        held.push(Held::Limbs(limbs_for(bits)?));
    }
    Ok(held)
}

/// `name`, held as `from`, as a `Wide32<n>`.
fn as_limbs(name: &str, from: Held, n: usize) -> String {
    match from {
        Held::I64 => format!("Wide32::<{n}>::from_i64({name})"),
        Held::Limbs(m) if m == n => name.to_owned(),
        Held::Limbs(_) => format!("{name}.resize::<{n}>()?"),
    }
}

/// The limb variant's expression for an operation held in `n` limbs, on operands `l` and `r`
/// held as `lh` and `rh`: a product by [`Wide32`]'s width-aware products, with neither operand
/// widened; a sum or difference on both operands brought to `n` limbs.
fn limb_operation(op: Op, l: &str, lh: Held, r: &str, rh: Held, n: usize) -> String {
    match (op, lh, rh) {
        (Op::Mul, Held::I64, Held::I64) => format!("Wide32::<{n}>::product_i64({l}, {r})?"),
        (Op::Mul, Held::Limbs(_), Held::I64) => {
            format!("Wide32::<{n}>::product_by_i64(&{l}, {r})?")
        }
        (Op::Mul, Held::I64, Held::Limbs(_)) => {
            format!("Wide32::<{n}>::product_by_i64(&{r}, {l})?")
        }
        (Op::Mul, Held::Limbs(_), Held::Limbs(_)) => {
            format!("Wide32::<{n}>::product(&{l}, &{r})?")
        }
        (op, lh, rh) => format!(
            "{}.{}(&{})?",
            as_limbs(l, lh, n),
            method(op),
            as_limbs(r, rh, n)
        ),
    }
}

/// The limb variant's statement for step `i`.
fn limbs_statement(d: &Derivation, held: &[Held], index: usize) -> Result<String, EmitError> {
    let (Some(step), Some(&step_held)) = (d.steps.get(index), held.get(index)) else {
        return fail("a step is missing");
    };
    let operand = |at: usize| -> Result<(&str, Held), EmitError> {
        match (d.steps.get(at), held.get(at)) {
            (Some(found), Some(&how)) => Ok((found.name.as_str(), how)),
            _ => fail("a step's operand is missing"),
        }
    };
    let body = match (&step.kind, step_held) {
        (StepKind::LoadCoord { point, axis }, Held::I64) => coordinate(point, *axis, Width::I64)?,
        (
            StepKind::Translate {
                minuend,
                subtrahend,
                axis,
            },
            Held::I64,
        ) => checked(
            &coordinate(minuend, *axis, Width::I64)?,
            Op::Sub,
            &coordinate(subtrahend, *axis, Width::I64)?,
            Width::I64,
        ),
        (StepKind::LoadRounded { input }, Held::I64) => format!(
            "i64::try_from(within({input}, {})?).ok()?",
            rounded_bits(d, input)?
        ),
        (StepKind::LoadRounded { input }, Held::Limbs(n)) => format!(
            "Wide32::<{n}>::from_i128(within({input}, {})?)?",
            rounded_bits(d, input)?
        ),
        (StepKind::Op { op, left, right }, Held::I64) => {
            let ((left_name, left_held), (right_name, right_held)) =
                (operand(*left)?, operand(*right)?);
            if left_held != Held::I64 || right_held != Held::I64 {
                return fail("an i64 step with a wider operand");
            }
            checked(left_name, *op, right_name, Width::I64)
        }
        (StepKind::Op { op, left, right }, Held::Limbs(n)) => {
            let ((left_name, left_held), (right_name, right_held)) =
                (operand(*left)?, operand(*right)?);
            limb_operation(*op, left_name, left_held, right_name, right_held, n)
        }
        _ => return fail("a coordinate or translation wider than i64"),
    };
    Ok(format!("    let {} = {body};", step.name))
}

/// The limb variant's `<predicate>_value` (piece G42): the steps held as [`limbs_held`] says, and
/// the result returned in the shipped path's type.
fn limbs_fn(
    out: &mut String,
    d: &Derivation,
    parameters: &[String],
    fn_name: &str,
) -> Result<(), EmitError> {
    let name = &d.description.name;
    let held = limbs_held(d)?;
    let (Some(root_step), Some(root), Some(&root_held)) = (
        d.steps.get(d.result),
        d.certs.get(d.result),
        held.get(d.result),
    ) else {
        return fail("the result step is missing");
    };
    let range = if has_rounded(d) {
        "; `None` also for a rounded input outside its declared range"
    } else {
        ""
    };
    item_para(
        out,
        &format!(
            "`{name}`'s exact value in width-aware limbs (chapter 23.3): each step in `i64`, or in \
             a `Wide32` of as many 32-bit limbs as its operands' certified bits need, with checked \
             arithmetic, and the result in the shipped path's type; `None` only if a step \
             overflowed, which the certificate rules out{range}."
        ),
    );
    line(out, "#[inline]");
    line(out, "#[must_use]");
    allowances(out, d, true, d.steps.len().saturating_add(1));
    signature(
        out,
        fn_name,
        parameters,
        &format!("Option<{}>", root.width.rust()),
    );
    for i in 0..d.steps.len() {
        line(out, &limbs_statement(d, &held, i)?);
    }
    let tail = match (root_held, root.width) {
        (Held::I64, Width::I64) => format!("    Some({})", root_step.name),
        (Held::Limbs(_), Width::I128) => format!("    {}.to_i128()", root_step.name),
        (Held::Limbs(_), w) if w > Width::I128 => {
            format!("    {}.to_wide::<{}>()", root_step.name, w.limbs())
        }
        _ => return fail("the result's held type and shipped type disagree"),
    };
    line(out, &tail);
    line(out, "}");
    line(out, "");
    Ok(())
}

/// One kind of limb step, proven once for every step of that kind (piece G42): the operation, how
/// each operand is held with its certified bits, and the limbs the result is held in.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LimbProof {
    /// A rounded input of `bits` bits brought into `limbs` limbs.
    Rounded {
        /// The input's declared bits.
        bits: u32,
        /// The limbs it is held in.
        limbs: usize,
    },
    /// An operation whose result is held in `limbs` limbs.
    Operation {
        /// The operation.
        op: char,
        /// The left operand's held type and certified bits.
        left: (Held, u32),
        /// The right operand's held type and certified bits.
        right: (Held, u32),
        /// The limbs the result is held in.
        limbs: usize,
    },
    /// The result, held in `limbs` limbs with `bits` certified bits, returned as the shipped type
    /// of `to` 64-bit limbs (2 for `i128`).
    Back {
        /// The limbs it is held in.
        limbs: usize,
        /// Its certified bits.
        bits: u32,
        /// The shipped type's 64-bit limbs.
        to: usize,
    },
}

/// The operator's character, for a proof's name.
const fn op_char(op: Op) -> char {
    match op {
        Op::Add => '+',
        Op::Sub => '-',
        Op::Mul => '*',
    }
}

/// The kinds of limb step a predicate's limb variant performs (piece G42).
///
/// # Errors
///
/// As [`limbs_held`].
pub fn limb_proofs(d: &Derivation) -> Result<BTreeSet<LimbProof>, EmitError> {
    let held = limbs_held(d)?;
    let mut proofs = BTreeSet::new();
    for (i, step) in d.steps.iter().enumerate() {
        let Some(&Held::Limbs(limbs)) = held.get(i) else {
            continue;
        };
        match &step.kind {
            StepKind::LoadRounded { input } => {
                proofs.insert(LimbProof::Rounded {
                    bits: rounded_bits(d, input)?,
                    limbs,
                });
            }
            StepKind::Op { op, left, right } => {
                let side = |j: usize| -> Result<(Held, u32), EmitError> {
                    match held.get(j) {
                        Some(&h) => Ok((h, bits_of(d, j)?)),
                        None => fail("a step's operand is missing"),
                    }
                };
                proofs.insert(LimbProof::Operation {
                    op: op_char(*op),
                    left: side(*left)?,
                    right: side(*right)?,
                    limbs,
                });
            }
            _ => return fail("a coordinate or translation wider than i64"),
        }
    }
    if let (Some(&Held::Limbs(limbs)), Some(root)) = (held.get(d.result), d.certs.get(d.result)) {
        proofs.insert(LimbProof::Back {
            limbs,
            bits: root.bits,
            to: root.width.limbs(),
        });
    }
    Ok(proofs)
}

/// A held operand's part of a proof's name: `i64x41`, or `w3x82` for three limbs.
fn held_name(h: Held, bits: u32) -> String {
    match h {
        Held::I64 => format!("i64x{bits}"),
        Held::Limbs(n) => format!("w{n}x{bits}"),
    }
}

/// A symbolic operand in a proof: an `i64` or a `Wide32` of magnitude below 2^bits.
fn symbolic(h: Held, bits: u32) -> String {
    match h {
        Held::I64 => format!("small({bits})"),
        Held::Limbs(n) => format!("big::<{n}>({bits})"),
    }
}

/// The proofs of the limb variant's steps, one per kind of step: each runs the very expression the
/// variant's code writes for that kind, on symbolic operands of every value the certified bits
/// allow, and asserts it never returns `None` (chapter 23.3: "Kani proves each generated limb step
/// free of overflow within those ranges").
fn proofs_module(out: &mut String, derivations: &[&Derivation]) -> Result<(), EmitError> {
    let mut proofs = BTreeSet::new();
    for d in derivations {
        proofs.extend(limb_proofs(d)?);
    }
    // The last function ends with an empty line.
    item_para(
        out,
        "Bounded proofs (chapter 23.3; piece G42): every kind of limb step the functions above \
         perform, on every operand its certified bits allow, never returns `None`.",
    );
    for l in PROOF_HELPERS {
        line(out, l);
    }
    for proof in &proofs {
        line(out, "");
        match proof {
            LimbProof::Rounded { bits, limbs } => {
                line(out, "    #[kani::proof]");
                line(out, &format!("    fn rounded_{bits}_in_{limbs}() {{"));
                line(out, &format!("        let v = wide_input({bits});"));
                line(
                    out,
                    &format!("        assert!(Wide32::<{limbs}>::from_i128(v).is_some());"),
                );
                line(out, "    }");
            }
            LimbProof::Operation {
                op,
                left,
                right,
                limbs,
            } => {
                let word = match op {
                    '+' => "add",
                    '-' => "sub",
                    _ => "mul",
                };
                let operation = match op {
                    '+' => Op::Add,
                    '-' => Op::Sub,
                    _ => Op::Mul,
                };
                line(out, "    #[kani::proof]");
                line(
                    out,
                    &format!(
                        "    fn {word}_{}_{}_in_{limbs}() {{",
                        held_name(left.0, left.1),
                        held_name(right.0, right.1)
                    ),
                );
                line(
                    out,
                    &format!(
                        "        let (l, r) = ({}, {});",
                        symbolic(left.0, left.1),
                        symbolic(right.0, right.1)
                    ),
                );
                let expression = limb_operation(operation, "l", left.0, "r", right.0, *limbs);
                line(
                    out,
                    &format!(
                        "        let step = || -> Option<Wide32<{limbs}>> {{ Some({expression}) }};"
                    ),
                );
                line(out, "        assert!(step().is_some());");
                line(out, "    }");
            }
            LimbProof::Back { limbs, bits, to } => {
                line(out, "    #[kani::proof]");
                let target = if *to == 2 {
                    "i128".to_owned()
                } else {
                    format!("{to}")
                };
                line(
                    out,
                    &format!("    fn back_w{limbs}x{bits}_to_{target}() {{"),
                );
                line(out, &format!("        let v = big::<{limbs}>({bits});"));
                if *to == 2 {
                    line(out, "        assert!(v.to_i128().is_some());");
                } else {
                    line(
                        out,
                        &format!("        assert!(v.to_wide::<{to}>().is_some());"),
                    );
                }
                line(out, "    }");
            }
        }
    }
    line(out, "}");
    Ok(())
}

/// The opening of the limb variant's proof module and the symbolic operands its proofs share.
const PROOF_HELPERS: &[&str] = &[
    "#[cfg(kani)]",
    "// cargo-mutants reads this attribute from the source and skips the module (its harnesses are",
    "// proven by Kani, and no test build compiles them); no build evaluates it, since the module",
    "// exists only under Kani, where `test` is off.",
    "#[cfg_attr(test, mutants::skip)]",
    "mod proofs {",
    "    use super::*;",
    "",
    "    /// A symbolic `i64` of magnitude below 2^bits.",
    "    fn small(bits: u32) -> i64 {",
    "        let v: i64 = kani::any();",
    "        kani::assume(v.unsigned_abs().checked_shr(bits) == Some(0));",
    "        v",
    "    }",
    "",
    "    /// A symbolic `i128` of magnitude below 2^bits.",
    "    fn wide_input(bits: u32) -> i128 {",
    "        let v: i128 = kani::any();",
    "        kani::assume(v.unsigned_abs().checked_shr(bits) == Some(0));",
    "        v",
    "    }",
    "",
    "    /// A symbolic `Wide32<N>` of magnitude below 2^bits: between −(2^bits − 1) and 2^bits − 1.",
    "    fn big<const N: usize>(bits: u32) -> Wide32<N> {",
    "        let v = Wide32::<N>::from_le_limbs(kani::any());",
    "        let mut limit = [0u32; N];",
    "        for (k, limb) in limit.iter_mut().enumerate() {",
    "            let low = u32::try_from(k).ok().and_then(|k| k.checked_mul(32));",
    "            let shift = low.map(|low| low.saturating_add(32).saturating_sub(bits));",
    "            *limb = shift.and_then(|s| u32::MAX.checked_shr(s)).unwrap_or(0);",
    "        }",
    "        let limit = Wide32::<N>::from_le_limbs(limit);",
    "        let floor = limit.checked_neg();",
    "        kani::assume(v <= limit && floor.is_some_and(|f| v >= f));",
    "        v",
    "    }",
];

/// The variant modules the parent module declares beside the predicate modules (piece G42).
pub const VARIANT_MODULES: [&str; 2] = ["limbs", "plain"];

/// One row of the parent module's table, for one predicate: what its module serves, and
/// Shewchuk's stage-A bound and FPG's constant for the same predicate, as published.
pub struct Comparison {
    /// The predicate.
    pub name: &'static str,
    /// What the module evaluates and what calls it, for its header (Markdown).
    pub serves: &'static str,
    /// Shewchuk's stage-A bound, as `exactinit` writes it, or a dash.
    pub shewchuk: &'static str,
    /// FPG's constant as Geogram's generated filter has it, or a dash.
    pub fpg: &'static str,
}

/// Writes header blocks: `P:` a paragraph, `I:` a list item (both wrapped), `T:` a line as it
/// is, and `""` a blank doc line. `{generator}`, `{emitter}`, `{bits}`, `{bits+1}` are replaced.
fn header_blocks(out: &mut String, blocks: &[&str], generator: &str) {
    for block in blocks {
        let text = block
            .replace("{generator}", generator)
            .replace("{emitter}", EMITTER_PATH)
            .replace("{bits+1}", &COORD_BITS.saturating_add(1).to_string())
            .replace("{bits}", &COORD_BITS.to_string());
        if let Some(p) = text.strip_prefix("P:") {
            doc_para(out, p);
        } else if let Some(item) = text.strip_prefix("I:") {
            para(out, "//!", "- ", "  ", item);
        } else if let Some(t) = text.strip_prefix("T:") {
            doc(out, t);
        } else {
            doc(out, &text);
        }
    }
}

/// The parent module: declarations, the helpers and the proof.
///
/// # Errors
///
/// An [`EmitError`] if a derivation cannot be summarised.
pub fn parent_module(
    derivations: &[(&Derivation, &Comparison)],
    generator: &str,
) -> Result<String, EmitError> {
    let mut out = String::new();
    header_blocks(&mut out, PARENT_HEADER_TOP, generator);
    doc(
        &mut out,
        "| Predicate | Steps | Result's width | Constant | Shewchuk, stage A (on his permanent) | \
         FPG in Geogram (on its row maxima) |",
    );
    doc(&mut out, "| --- | ---: | --- | --- | --- | --- |");
    for (d, c) in derivations {
        let Some(root) = d.certs.get(d.result) else {
            return fail("the result step is missing");
        };
        if c.name != d.description.name {
            return fail(format!(
                "comparison row `{}` given for `{}`",
                c.name, d.description.name
            ));
        }
        doc(
            &mut out,
            &format!(
                "| [`{}`] | {} | `{}` | {:e} · {} | {} | {} |",
                d.description.name,
                d.steps.len(),
                root.width.rust(),
                f64::from_bits(d.constant_bits),
                scale_text(&root.scale, &group_names(d)),
                c.shewchuk,
                c.fpg
            ),
        );
    }
    header_blocks(&mut out, PARENT_HEADER_BOTTOM, generator);
    line(&mut out, "");
    let mut names: Vec<&str> = derivations
        .iter()
        .map(|(d, _)| d.description.name.as_str())
        .collect();
    names.extend(VARIANT_MODULES);
    names.sort_unstable();
    for n in names {
        line(&mut out, &format!("pub mod {n};"));
    }
    let coordinates = derivations.iter().any(|(d, _)| has_coordinates(d));
    let rounded = derivations.iter().any(|(d, _)| has_rounded(d));
    line(&mut out, "");
    if coordinates {
        line(&mut out, "use crate::grid::Coord;");
    }
    line(&mut out, "use crate::sign::Sign;");
    if coordinates {
        line(&mut out, "");
        line(
            &mut out,
            &format!(
                "/// A grid coordinate as an `f64`, exactly: |v| ≤ 2^{COORD_BITS} − 1 ≤ 2^53, so \
                 every such value is"
            ),
        );
        line(
            &mut out,
            "/// an `f64` (geom-gen refuses a grid where it is not).",
        );
        for text in EXACT_F64 {
            line(&mut out, text);
        }
    }
    for text in SHARED_HELPERS {
        line(&mut out, text);
    }
    if rounded {
        for text in ROUNDED_HELPERS {
            line(&mut out, text);
        }
    }
    Ok(out)
}

/// The parent module's header, before the comparison table.
const PARENT_HEADER_TOP: &[&str] = &[
    "P:Generated by geom-gen, generator `{generator}` (seed: none, it uses no randomness), from \
     `{emitter}`. Do not edit: change the generator and run `cargo run -p geom-gen -- write`. CI \
     runs `cargo run -p geom-gen -- check` and fails on any difference.",
    "",
    "P:The generated filters and exact paths of the kernel's predicates (chapter 6.2 of the \
     kernel architecture, `projects/engines/geometry/architecture/06-numbers.md`): one module per \
     predicate. The explicit-point predicates of [`crate::predicate`] call their module's \
     `filter` first and, when it cannot certify the sign, its `exact` path, except where the \
     filter is measured not to pay (the routing in [`crate::predicate`]'s documentation); each \
     module's header says what it evaluates and what calls it.",
    "",
    "P:Each predicate module holds:",
    "",
    "I:its description, as written in geom-gen, and the derivation from it: a bit-range \
     certificate for every intermediate (the bound on its exact value over every input in the \
     domain, its bits and the integer type that holds it) and the filter's error analysis (each \
     intermediate's scale, unit bound and unit error, and the error constant, exactly);",
    "I:`filter`: the description evaluated in `f64`, step by step, and its decision: \
     `Some(sign)` when the computed value exceeds the bound in magnitude, `Some(Sign::Zero)` when \
     the scale is zero, `None` otherwise;",
    "I:`value` and `exact`: the description evaluated exactly, each step in the narrowest of \
     `i64`, `i128`, `I256`, `I320`, `I384` and `I512` that its certificate allows, with checked \
     arithmetic.",
    "",
    "P:Guarantees:",
    "",
    "I:**A filter never gives a wrong sign.** When `filter` returns `Some(sign)`, `sign` is the \
     sign of the exact value, for every input in the domain (the proof below). It returns `None` \
     when it cannot tell, never a guess.",
    "I:**The exact path is exact.** `value` returns the exact value for every input in the \
     domain: the certificate bounds every intermediate within its type, and a step that did not \
     fit would give `None`, never a wrapped value, since every step is a checked method. Nothing \
     depends on overflow checks being on.",
    "I:**Proven, or checked and labelled so** (chapter 28.6). The bounds are derived exactly by \
     the written proof below and re-derived by an independent checker in geom-gen's tests (its \
     own parser, big integers and analysis), which also reads this committed code, this module's \
     helpers included; the filters are tested against the exact paths on millions of random and \
     near-degenerate inputs at the grid's extremes and on inputs placed just inside and just \
     outside each bound. On top of that, each module's guarantees say whether its filter's bound \
     is machine-checked: proven where Gappa's proofs, checked by Rocq, cover every input \
     (`kernel-certificates.yml`, pieces G35 and G40), and \"checked, not proven\" with its open \
     issue otherwise.",
    "I:**Deterministic.** `f64` addition, subtraction and multiplication are correctly rounded on \
     every target and Rust never fuses them (RFC 3514); the conversion of a rounded input rounds \
     to nearest, ties to even, on every target; the maxima use comparisons only. The same inputs \
     give the same answer on every target, and the answer is the exact sign whenever it is \
     given, so the filter can never change a result.",
    "I:**No overflow, no underflow, no NaN, no panic, no allocation.** Every value the filter \
     computes is a whole number below 2^1000 in magnitude (checked by the generator), so none is \
     subnormal or infinite.",
    "",
    "P:Proof. For every input in the domain, each step's computed value ṽ and exact value v \
     satisfy |ṽ| ≤ B · S and |ṽ − v| ≤ E · S, where S is the largest product of group maxima \
     over the step's scale set, by induction over the steps. A grid coordinate is an `f64` \
     exactly, and a translation of two is a whole number of magnitude at most 2^{bits+1} − 2 ≤ \
     2^53, so it is exact: B = 1, E = 0. A rounded input's nearest `f64` x̃ has \
     |x̃ − x| ≤ u · |x̃|: B = 1, E = u. A sum or difference of a and b has scale S(a) ∪ S(b) and \
     P = B(a) + B(b); a product has scale S(a) · S(b) and P = B(a) · B(b). A step is exact when \
     its operands are and the certificate bounds it by 2^53: then B = P and E = 0. Otherwise \
     B = P · (1 + u) and E is the propagated error plus u · P, the propagated error being \
     E(a) + E(b) for a sum or difference and E(a) · B(b) + B(a) · E(b) + E(a) · E(b) for a \
     product. Each rounded product of non-negative values loses at most a factor (1 − u) beyond \
     the losses of both its operands, and `larger` at most the larger of its operands' losses, so \
     the computed scale is at least S · (1 − u)^r, r counting every product's rounding with its \
     operands' and each `larger` as its larger operand's; the product by the constant makes \
     k = r + 1 (each module's header gives both). The constant is at least E · (1 + 2u)^k and \
     (1 − u)(1 + 2u) ≥ 1, so the computed bound is at least E · S; a computed value beyond it has \
     the exact sign. A computed \
     scale of zero is exactly zero, and then |v| ≤ (B + E) · S = 0. The full statement, with \
     references, is in geom-gen's `predicate_derive.rs`.",
    "",
    "P:The constants, beside Shewchuk's stage-A bounds (`exactinit` in `predicates.c`; \
     ε = 2^-53, scaled by a permanent computed at run time) and the constants of FPG's filters in \
     Geogram (scaled by maxima of differences that FPG treats as rounded, so twice as large for \
     orient2d):",
    "",
];

/// The parent module's header, after the comparison table.
const PARENT_HEADER_BOTTOM: &[&str] = &[
    "",
    "P:Method: Meyer and Pion's almost-static filters, which chapter 6.2 calls semi-static \
     (*FPG: A code generator for fast and certified geometric predicates*, Real Numbers and \
     Computers 2008, <https://inria.hal.science/inria-00344297>): a forward error analysis at \
     unit input bounds, with groups of variables and translated inputs, gives a constant that the \
     run-time maxima of the groups scale. The groups here are the axes; Geogram's orient2d groups \
     each row of its determinant instead (`max1` over `a11` and `a12`), equally sound, so its \
     maxima are not the same quantities as these. Geogram's PCK generates its filters with FPG from \
     one description per predicate (Lévy 2016, <https://doi.org/10.1016/j.cad.2015.10.004>; \
     BSD-3-Clause, read for method at commit 16843b4804de35f14ef54667950ab4c8acdd0062). Here the \
     analysis is exact (whole polynomials in u, no upward rounding), the translations are exact \
     because grid differences are below 2^53, which halves FPG's orient2d constant, rounded \
     inputs (`f64` copies of wider integers) carry their rounding into the bound (chapter 23.2), \
     and FPG's under- and overflow guards are unnecessary because every value is a whole number \
     far from the ends of the `f64` range. The exact path replaces Geogram's expansion \
     arithmetic with fixed-width integers chosen per step from the certificate (Fortune and Van \
     Wyk 1996, <https://doi.org/10.1145/231731.231735>; chapter 23.3).",
    "",
    "P:Cost: per predicate, a few dozen `f64` operations for the filter (each module's header \
     gives its count), then, only when it cannot certify the sign, the exact path: `i64` \
     differences, single 64 × 64 → 128-bit products of them and `i128` sums for the orientation \
     tests, with `I256` products of `i128` factors for in-circle and in-sphere. Chapter 23.2: on \
     data where most answers are exactly zero, the filter mostly falls through, so whether to \
     filter is measured; a zero scale (every point on one axis line, say) still answers at once. \
     The benchmark is `generated_route_against_the_hand_written_fast_path` in `predicate.rs`'s \
     tests.",
    "",
    "P:Licences: the code here is MPL-2.0, generated by geom-gen; no outside code is copied into \
     it. Geogram (BSD-3-Clause, Inria) and FPG's paper were read for method only.",
];

/// `exact_f64`, after its first doc lines (which state the grid's bound).
const EXACT_F64: &[&str] = &[
    "#[allow(",
    "    clippy::cast_precision_loss,",
    "    reason = \"a grid coordinate is at most 2^53 in magnitude, so the conversion is exact\"",
    ")]",
    "#[inline]",
    "const fn exact_f64(c: Coord) -> f64 {",
    "    c.get() as f64",
    "}",
];

/// The helpers every filter uses.
const SHARED_HELPERS: &[&str] = &[
    "",
    "/// The larger of two magnitudes. Both are non-negative and not NaN (absolute values of finite",
    "/// whole numbers, or products of them), so a comparison decides it on every target, with none",
    "/// of `f64::max`'s freedom over the sign of zero.",
    "#[inline]",
    "const fn larger(a: f64, b: f64) -> f64 {",
    "    if a < b { b } else { a }",
    "}",
    "",
    "/// The largest |v| among `values`: a group's maximum M(g). Exact: `abs` and comparisons only.",
    "#[inline]",
    "fn largest_magnitude<const N: usize>(values: [f64; N]) -> f64 {",
    "    values.iter().map(|v| v.abs()).reduce(larger).unwrap_or(0.0)",
    "}",
    "",
    "/// A filter's decision: the sign of `value` when `|value| > error · scale`, the bound computed",
    "/// with one rounding (counted in each constant); `Some(Sign::Zero)` when the scale is zero,",
    "/// which makes the exact value zero; otherwise `None`.",
    "#[inline]",
    "#[allow(",
    "    clippy::needless_bitwise_bool,",
    "    reason = \"every comparison is made, so the decision compiles to selects, not branches\"",
    ")]",
    "fn decide(value: f64, error: f64, scale: f64) -> Option<Sign> {",
    "    let bound = error * scale;",
    "    let above = value > bound;",
    "    let below = value < -bound;",
    "    let sign = if above {",
    "        Sign::Positive",
    "    } else if below {",
    "        Sign::Negative",
    "    } else {",
    "        Sign::Zero",
    "    };",
    "    // Every comparison is made (`|`, not `||`), so the decision compiles to selects, not to",
    "    // branches on the sign, which the caller branches on anyway.",
    "    (above | below | (scale == 0.0)).then_some(sign)",
    "}",
];

/// The helpers of filters and exact paths with rounded inputs.
const ROUNDED_HELPERS: &[&str] = &[
    "",
    "/// `v` itself if |v| ≤ 2^bits − 1, the range a description declares for a rounded input (bits",
    "/// at most 127), within which its certificate and its error analysis hold; otherwise `None`.",
    "#[inline]",
    "fn within(v: i128, bits: u32) -> Option<i128> {",
    "    let limit = 1u128.checked_shl(bits)?.checked_sub(1)?;",
    "    if v.unsigned_abs() <= limit {",
    "        Some(v)",
    "    } else {",
    "        None",
    "    }",
    "}",
    "",
    "/// A rounded input's nearest `f64`, ties to even, the same on every target (the Rust",
    "/// Reference, numeric casts): |x̃ − x| ≤ u · |x̃|, the error the analysis gives it. `None` outside",
    "/// the declared range, as [`within`].",
    "#[allow(",
    "    clippy::cast_precision_loss,",
    "    reason = \"the conversion rounds to nearest, ties to even: the rounding the bound counts\"",
    ")]",
    "#[inline]",
    "fn rounded_f64(v: i128, bits: u32) -> Option<f64> {",
    "    within(v, bits).map(|v| v as f64)",
    "}",
];

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
    use crate::predicate_derive::derive;
    use crate::predicate_lang::{GENERATED_NAMES, reserved};

    const ORIENT: &str = "predicate orient2d\n\
        input a b c: point2\n\
        let ba_x = b.x - a.x\n\
        let ba_y = b.y - a.y\n\
        let ca_x = c.x - a.x\n\
        let ca_y = c.y - a.y\n\
        let det = ba_x * ca_y - ba_y * ca_x\n\
        sign det\n";

    /// The language reserves every lowercase name the emitted code declares, imports or binds
    /// besides the description's own, so the two lists cannot drift apart: each `fn` of the parent
    /// module's helpers and each item and local of a predicate module.
    #[test]
    fn the_language_reserves_every_name_the_code_uses() {
        let d = derive(ORIENT).unwrap();
        let module = predicate_module(&d, "predicate-test", "TEST").unwrap();
        let comparison = Comparison {
            name: "orient2d",
            serves: "the test",
            shewchuk: "-",
            fpg: "-",
        };
        let rounded = derive(
            "predicate side\ninput n: rounded 83 in n\ninput p: point3\nlet v = n * p.x\nsign v\n",
        )
        .unwrap();
        let side = Comparison {
            name: "side",
            serves: "the test",
            shewchuk: "-",
            fpg: "-",
        };
        let parent =
            parent_module(&[(&d, &comparison), (&rounded, &side)], "predicate-module").unwrap();
        let mut declared: Vec<String> = Vec::new();
        for l in parent.lines().chain(module.lines()) {
            let t = l.trim_start();
            for prefix in ["fn ", "const fn ", "pub fn ", "let "] {
                if let Some(rest) = t.strip_prefix(prefix) {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    declared.push(name);
                }
            }
        }
        let steps: Vec<&str> = d.steps.iter().map(|s| s.name.as_str()).collect();
        let groups = ["x", "y", "z"];
        for name in &declared {
            if steps.contains(&name.as_str())
                || name == "bound"
                || name == "above"
                || name == "below"
                || name == "sign"
                || name == "limit"
            {
                // The description's own steps, and `decide`'s and `within`'s locals, which live
                // in functions that bind no description name.
                continue;
            }
            assert!(
                reserved(name, &groups).is_some(),
                "the emitted code binds `{name}`, which the language does not reserve"
            );
        }
        for name in GENERATED_NAMES {
            assert!(
                declared.iter().any(|d| d == name),
                "{name} is reserved but unused"
            );
        }
    }

    /// The width-aware exact path: translations in `i64`, their products widened to `i128` one
    /// operand at a time, and the root's sign from `Sign::of_i128`.
    #[test]
    fn the_exact_path_is_width_aware() {
        let d = derive(ORIENT).unwrap();
        let module = predicate_module(&d, "predicate-test", "TEST").unwrap();
        assert!(module.contains("    let ba_x = b.x.get().checked_sub(a.x.get())?;"));
        assert!(
            module.contains("    let det_1 = i128::from(ba_x).checked_mul(i128::from(ca_y))?;")
        );
        assert!(module.contains("    let det = det_1.checked_sub(det_2)?;"));
        assert!(module.contains("    value(a, b, c).map(Sign::of_i128)"));
        let doc: Vec<&str> = module
            .lines()
            .filter_map(|l| l.strip_prefix("//! "))
            .collect();
        assert!(doc.join(" ").contains("the bound's 2 multiplication(s)"));
    }

    /// A rounded input is a parameter of type `i128`, converted by `rounded_f64` in the filter and
    /// checked by `within` in the exact path; one of more than 127 bits is refused.
    #[test]
    fn rounded_inputs_are_emitted_up_to_127_bits() {
        let d = derive(
            "predicate side\ninput n: rounded 83 in n\ninput off: rounded 125 in w\n\
             input p: point3\nlet v = n * p.x + off\nsign v\n",
        )
        .unwrap();
        let module = predicate_module(&d, "predicate-test", "TEST").unwrap();
        assert!(module.contains("pub fn filter(n: i128, off: i128, p: P3) -> Option<Sign> {"));
        assert!(module.contains("    let n = rounded_f64(n, 83)?;"));
        assert!(module.contains("    let n = within(n, 83)?;"));
        assert!(module.contains("    let off = within(off, 125)?;"));
        assert!(module.contains(
            "use super::{decide, exact_f64, larger, largest_magnitude, rounded_f64, within};"
        ));
        let wide = derive(
            "predicate side\ninput n: rounded 200 in n\ninput p: point3\nlet v = n * p.x\nsign v\n",
        )
        .unwrap();
        let e = predicate_module(&wide, "predicate-test", "TEST").unwrap_err();
        assert!(e.0.contains("at most 127 bits"), "{e:?}");
    }

    /// The emitter's own name check, past the language: a derivation whose step is renamed to a
    /// filter local, a helper, a keyword or another step's name is refused.
    #[test]
    fn the_emitter_refuses_names_the_code_cannot_hold() {
        for bad in ["m_x", "scale", "decide", "type", "ERROR", "ca_y"] {
            let mut d = derive(ORIENT).unwrap();
            let root = d.result;
            d.steps[root].name = bad.to_owned();
            let e = predicate_module(&d, "predicate-test", "TEST").unwrap_err();
            assert!(e.0.contains(bad), "{bad}: {e:?}");
        }
        let mut d = derive(ORIENT).unwrap();
        d.description.name = "fn".to_owned();
        assert!(predicate_module(&d, "predicate-test", "TEST").is_err());
    }
}
