//! Attacks on pieces G35 and G40 (the filter certificates): the problems Gappa and `FPTaylor` prove
//! are the filters that ship, their cases cover every input and assume only what holds in them,
//! and each module's label names them (chapter 28.6). `kernel-certificates.yml` runs the tools on
//! the committed problems; these tests hold the problems to the committed filters, reading both as
//! text, so a proof is about the code that ships, not about a copy of it.
//!
//! - `each_gappa_problem_states_the_shipped_filter` and
//!   `each_fptaylor_problem_states_the_shipped_filter`: every `let x = a op b;` operation of the
//!   committed `filter`, in order, is `x = rnd(a op b);` in each Gappa problem and
//!   `x = rnd64(a op b);` in the `FPTaylor` problem, and nothing else is; the filter's inputs are
//!   the problems' inputs (a rounded input's copy `rndx(e_v)` of a whole number within its bits),
//!   each bounded by the maximum the filter takes over it, and each maximum by the largest value
//!   its inputs reach as the filter computes them; the constant has the bits of the module's
//!   `ERROR`; and the computed scale is the module's `scale`, rounded product by product, with
//!   each larger-of resolved. In
//!   the `FPTaylor` problem, a sum's operand is weighted by a ratio parameter exactly when its
//!   scale set, in the module's error table, is not the sum's; each parameter is ratios of sets,
//!   the operand's monomials among the sum's, so in [0, 1], all one ratio once their common
//!   factor is cancelled (worked out here from the sets' text), and no two parameters are; and
//!   the variables are the normalised inputs (a rounded input's exact value within 1 + 2^−53, its
//!   copy `rnd64(e_v)`) and the parameters, nothing else.
//! - `the_cases_cover_every_input_and_assume_only_what_holds`: a filter whose scale takes a
//!   larger-of has one problem for each way its larger-ofs can resolve, so whichever the filter
//!   takes, a problem states it; and every hypothesis of each problem is one this test can justify
//!   for the inputs of its case, the case where its S is the largest of those ways: an input's own
//!   bounds, a bound by a larger maximum that the case implies, or a term of the scale at most S.
//! - `the_bounds_are_the_constant_less_its_roundings`: the error bound K both tools check is
//!   exactly C · (1 − (k + 1) · 2^−53) for the module's constant C and roundings k, worked out here
//!   from the module's text, and each Gappa problem bounds the computed bound's relative error by
//!   (k + 1) · 2^−53. Each `FPTaylor` problem bounds the sum of its error terms at once exactly
//!   when `predicate_certificates::EXACT_OPTIMISATION` names its filter, and then the total HOL
//!   Light proves is the largest double at most K; otherwise it bounds each term and names none.
//! - `the_problems_are_current_and_registered`: the committed problems are what the registered
//!   generators write now, the registry generates no other problem, and every generated filter is
//!   held here.
//! - `each_label_names_its_problems`: each module's guarantees say its filter is proven and name
//!   exactly the Gappa problems the registry generates for it, or, for a filter
//!   `predicate_certificates::UNPROVEN` lists, say "checked, not proven" with its open issue; and
//!   each names its `FPTaylor` problem.
//! - `a_single_problem_refuses_a_scale_of_several_monomials`: such a filter (`plane_side`,
//!   `incircle`, `insphere`, `lpi_weight`) gets no single Gappa problem, with the reason: one per
//!   case; `FPTaylor`'s normalised problem needs none.
//! - `each_unproven_filter_is_one_gappa_cannot_state`: an unproven filter's issue is not stale.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "fixtures; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names
)]

use std::collections::{BTreeMap, BTreeSet};

use geom_gen::Generate;
use geom_gen::predicate_certificates::{
    EXACT_OPTIMISATION, UNPROVEN, fptaylor_problem, gappa_case_problem, gappa_cases, gappa_problem,
};
use geom_gen::predicates::{PREDICATES, derivation};
use geom_gen::registry::REGISTRY;

/// One predicate: its committed module, its Gappa problems (file name and text), none while
/// [`UNPROVEN`] gives it an open issue, and its `FPTaylor` problem.
struct Filter {
    name: &'static str,
    module: &'static str,
    gappa: &'static [(&'static str, &'static str)],
    fptaylor: (&'static str, &'static str),
}

/// Every generated predicate, each with every problem committed for it.
const FILTERS: [Filter; 23] = [
    Filter {
        name: "orient2d",
        module: include_str!("../../geom-num/src/generated/orient2d.rs"),
        gappa: &[(
            "gappa-orient2d.g",
            include_str!("../generated/gappa-orient2d.g"),
        )],
        fptaylor: (
            "fptaylor-orient2d.fpt",
            include_str!("../generated/fptaylor-orient2d.fpt"),
        ),
    },
    Filter {
        name: "orient3d",
        module: include_str!("../../geom-num/src/generated/orient3d.rs"),
        gappa: &[(
            "gappa-orient3d.g",
            include_str!("../generated/gappa-orient3d.g"),
        )],
        fptaylor: (
            "fptaylor-orient3d.fpt",
            include_str!("../generated/fptaylor-orient3d.fpt"),
        ),
    },
    Filter {
        name: "incircle",
        module: include_str!("../../geom-num/src/generated/incircle.rs"),
        gappa: &[
            (
                "gappa-incircle-x.g",
                include_str!("../generated/gappa-incircle-x.g"),
            ),
            (
                "gappa-incircle-y.g",
                include_str!("../generated/gappa-incircle-y.g"),
            ),
        ],
        fptaylor: (
            "fptaylor-incircle.fpt",
            include_str!("../generated/fptaylor-incircle.fpt"),
        ),
    },
    Filter {
        name: "insphere",
        module: include_str!("../../geom-num/src/generated/insphere.rs"),
        gappa: &[
            (
                "gappa-insphere-x.g",
                include_str!("../generated/gappa-insphere-x.g"),
            ),
            (
                "gappa-insphere-y.g",
                include_str!("../generated/gappa-insphere-y.g"),
            ),
            (
                "gappa-insphere-z.g",
                include_str!("../generated/gappa-insphere-z.g"),
            ),
        ],
        fptaylor: (
            "fptaylor-insphere.fpt",
            include_str!("../generated/fptaylor-insphere.fpt"),
        ),
    },
    Filter {
        name: "plane_side",
        module: include_str!("../../geom-num/src/generated/plane_side.rs"),
        gappa: &[
            (
                "gappa-plane-side-d.g",
                include_str!("../generated/gappa-plane-side-d.g"),
            ),
            (
                "gappa-plane-side-z-n.g",
                include_str!("../generated/gappa-plane-side-z-n.g"),
            ),
            (
                "gappa-plane-side-y-n.g",
                include_str!("../generated/gappa-plane-side-y-n.g"),
            ),
            (
                "gappa-plane-side-x-n.g",
                include_str!("../generated/gappa-plane-side-x-n.g"),
            ),
        ],
        fptaylor: (
            "fptaylor-plane-side.fpt",
            include_str!("../generated/fptaylor-plane-side.fpt"),
        ),
    },
    Filter {
        name: "tpi_weight",
        module: include_str!("../../geom-num/src/generated/tpi_weight.rs"),
        gappa: &[(
            "gappa-tpi-weight.g",
            include_str!("../generated/gappa-tpi-weight.g"),
        )],
        fptaylor: (
            "fptaylor-tpi-weight.fpt",
            include_str!("../generated/fptaylor-tpi-weight.fpt"),
        ),
    },
    Filter {
        name: "tpi_side",
        module: include_str!("../../geom-num/src/generated/tpi_side.rs"),
        gappa: &[],
        fptaylor: (
            "fptaylor-tpi-side.fpt",
            include_str!("../generated/fptaylor-tpi-side.fpt"),
        ),
    },
    Filter {
        name: "lpi_weight",
        module: include_str!("../../geom-num/src/generated/lpi_weight.rs"),
        gappa: &[
            (
                "gappa-lpi-weight-z-n.g",
                include_str!("../generated/gappa-lpi-weight-z-n.g"),
            ),
            (
                "gappa-lpi-weight-y-n.g",
                include_str!("../generated/gappa-lpi-weight-y-n.g"),
            ),
            (
                "gappa-lpi-weight-x-n.g",
                include_str!("../generated/gappa-lpi-weight-x-n.g"),
            ),
        ],
        fptaylor: (
            "fptaylor-lpi-weight.fpt",
            include_str!("../generated/fptaylor-lpi-weight.fpt"),
        ),
    },
    Filter {
        name: "lpi_side",
        module: include_str!("../../geom-num/src/generated/lpi_side.rs"),
        gappa: &[],
        fptaylor: (
            "fptaylor-lpi-side.fpt",
            include_str!("../generated/fptaylor-lpi-side.fpt"),
        ),
    },
    Filter {
        name: "ssi_weight",
        module: include_str!("../../geom-num/src/generated/ssi_weight.rs"),
        gappa: &[(
            "gappa-ssi-weight.g",
            include_str!("../generated/gappa-ssi-weight.g"),
        )],
        fptaylor: (
            "fptaylor-ssi-weight.fpt",
            include_str!("../generated/fptaylor-ssi-weight.fpt"),
        ),
    },
    Filter {
        name: "ssi_side",
        module: include_str!("../../geom-num/src/generated/ssi_side.rs"),
        gappa: &[(
            "gappa-ssi-side.g",
            include_str!("../generated/gappa-ssi-side.g"),
        )],
        fptaylor: (
            "fptaylor-ssi-side.fpt",
            include_str!("../generated/fptaylor-ssi-side.fpt"),
        ),
    },
    Filter {
        name: "carrier_side",
        module: include_str!("../../geom-num/src/generated/carrier_side.rs"),
        gappa: &[
            (
                "gappa-carrier-side-n-p.g",
                include_str!("../generated/gappa-carrier-side-n-p.g"),
            ),
            (
                "gappa-carrier-side-d-w.g",
                include_str!("../generated/gappa-carrier-side-d-w.g"),
            ),
        ],
        fptaylor: (
            "fptaylor-carrier-side.fpt",
            include_str!("../generated/fptaylor-carrier-side.fpt"),
        ),
    },
    Filter {
        name: "carrier_order",
        module: include_str!("../../geom-num/src/generated/carrier_order.rs"),
        gappa: &[(
            "gappa-carrier-order.g",
            include_str!("../generated/gappa-carrier-order.g"),
        )],
        fptaylor: (
            "fptaylor-carrier-order.fpt",
            include_str!("../generated/fptaylor-carrier-order.fpt"),
        ),
    },
    Filter {
        name: "rational_cmp",
        module: include_str!("../../geom-num/src/generated/rational_cmp.rs"),
        gappa: &[(
            "gappa-rational-cmp.g",
            include_str!("../generated/gappa-rational-cmp.g"),
        )],
        fptaylor: (
            "fptaylor-rational-cmp.fpt",
            include_str!("../generated/fptaylor-rational-cmp.fpt"),
        ),
    },
    Filter {
        name: "rational_orient",
        module: include_str!("../../geom-num/src/generated/rational_orient.rs"),
        gappa: &[(
            "gappa-rational-orient.g",
            include_str!("../generated/gappa-rational-orient.g"),
        )],
        fptaylor: (
            "fptaylor-rational-orient.fpt",
            include_str!("../generated/fptaylor-rational-orient.fpt"),
        ),
    },
    Filter {
        name: "round_grid",
        module: include_str!("../../geom-num/src/generated/round_grid.rs"),
        gappa: &[],
        fptaylor: (
            "fptaylor-round-grid.fpt",
            include_str!("../generated/fptaylor-round-grid.fpt"),
        ),
    },
    Filter {
        name: "round_rational",
        module: include_str!("../../geom-num/src/generated/round_rational.rs"),
        gappa: &[],
        fptaylor: (
            "fptaylor-round-rational.fpt",
            include_str!("../generated/fptaylor-round-rational.fpt"),
        ),
    },
    Filter {
        name: "round_line",
        module: include_str!("../../geom-num/src/generated/round_line.rs"),
        gappa: &[],
        fptaylor: (
            "fptaylor-round-line.fpt",
            include_str!("../generated/fptaylor-round-line.fpt"),
        ),
    },
    Filter {
        name: "circle_side",
        module: include_str!("../../geom-num/src/generated/circle_side.rs"),
        gappa: &[],
        fptaylor: (
            "fptaylor-circle-side.fpt",
            include_str!("../generated/fptaylor-circle-side.fpt"),
        ),
    },
    Filter {
        name: "circle_line",
        module: include_str!("../../geom-num/src/generated/circle_line.rs"),
        gappa: &[],
        fptaylor: (
            "fptaylor-circle-line.fpt",
            include_str!("../generated/fptaylor-circle-line.fpt"),
        ),
    },
    Filter {
        name: "circle_gap",
        module: include_str!("../../geom-num/src/generated/circle_gap.rs"),
        gappa: &[
            (
                "gappa-circle-gap-x-x.g",
                include_str!("../generated/gappa-circle-gap-x-x.g"),
            ),
            (
                "gappa-circle-gap-y-y.g",
                include_str!("../generated/gappa-circle-gap-y-y.g"),
            ),
            (
                "gappa-circle-gap-t-t.g",
                include_str!("../generated/gappa-circle-gap-t-t.g"),
            ),
        ],
        fptaylor: (
            "fptaylor-circle-gap.fpt",
            include_str!("../generated/fptaylor-circle-gap.fpt"),
        ),
    },
    Filter {
        name: "ellipse_side",
        module: include_str!("../../geom-num/src/generated/ellipse_side.rs"),
        gappa: &[],
        fptaylor: (
            "fptaylor-ellipse-side.fpt",
            include_str!("../generated/fptaylor-ellipse-side.fpt"),
        ),
    },
    Filter {
        name: "ellipse_line",
        module: include_str!("../../geom-num/src/generated/ellipse_line.rs"),
        gappa: &[],
        fptaylor: (
            "fptaylor-ellipse-line.fpt",
            include_str!("../generated/fptaylor-ellipse-line.fpt"),
        ),
    },
];

/// The body of the module's `filter`, one statement per line.
fn filter_body(module: &str) -> Vec<String> {
    let start = module.find("pub fn filter(").expect("a filter");
    let body = &module[start..];
    let end = body.find("\n}\n").expect("the filter's end");
    body[..end]
        .lines()
        .skip(1)
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect()
}

/// `(name, left, op, right)` of each `let name = left op right;` whose operands are names.
fn filter_operations(module: &str) -> Vec<(String, String, String, String)> {
    filter_body(module)
        .iter()
        .filter_map(|l| {
            let rest = l.strip_prefix("let ")?.strip_suffix(';')?;
            let (name, expr) = rest.split_once(" = ")?;
            let parts: Vec<&str> = expr.split(' ').collect();
            match parts.as_slice() {
                [a, op, b]
                    if ["+", "-", "*"].contains(op)
                        && !a.contains('(')
                        && !b.contains('(')
                        && name != "scale" =>
                {
                    Some((
                        name.to_owned(),
                        (*a).to_owned(),
                        (*op).to_owned(),
                        (*b).to_owned(),
                    ))
                }
                _ => None,
            }
        })
        .collect()
}

/// `(name, left, op, right)` of each `name = <rounding>(left op right);` line of a problem.
fn problem_operations(problem: &str, rounding: &str) -> Vec<(String, String, String, String)> {
    problem
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let (name, expr) = l.split_once(" = ")?;
            let inner = expr
                .strip_prefix(&format!("{rounding}("))?
                .strip_suffix(");")?;
            let parts: Vec<&str> = inner.split(' ').collect();
            match parts.as_slice() {
                [a, op, b] if !a.contains('(') && !b.contains('(') => Some((
                    name.to_owned(),
                    (*a).to_owned(),
                    (*op).to_owned(),
                    (*b).to_owned(),
                )),
                _ => None,
            }
        })
        .filter(|(name, ..)| name != "scale" && name != "bound")
        .collect()
}

/// How the filter takes an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Input {
    /// Exactly: a coordinate, or a difference of two (`exact_f64`).
    Exact,
    /// As the nearest `f64` to an integer of this many bits (`rounded_f64(v, bits)?`).
    Rounded(u32),
}

/// The filter's inputs, in order: `let x = exact_f64(..)..;` and `let x = rounded_f64(.., b)?;`.
fn inputs(module: &str) -> Vec<(String, Input)> {
    filter_body(module)
        .iter()
        .filter_map(|l| {
            let (name, expr) = l.strip_prefix("let ")?.split_once(" = ")?;
            if expr.starts_with("exact_f64(") {
                return Some((name.to_owned(), Input::Exact));
            }
            let args = expr.strip_prefix("rounded_f64(")?.strip_suffix(")?;")?;
            let bits = args.split_once(", ")?.1.parse().ok()?;
            Some((name.to_owned(), Input::Rounded(bits)))
        })
        .collect()
}

/// `(maximum, inputs)` of each `let m = largest_magnitude([..]);`, in the filter's order.
fn maxima(module: &str) -> Vec<(String, Vec<String>)> {
    filter_body(module)
        .iter()
        .filter_map(|l| {
            let rest = l.strip_prefix("let ")?;
            let (name, expr) = rest.split_once(" = ")?;
            let list = expr
                .strip_prefix("largest_magnitude([")?
                .strip_suffix("]);")?;
            Some((
                name.to_owned(),
                list.split(", ").map(str::to_owned).collect(),
            ))
        })
        .collect()
}

/// The maximum the filter takes over `input`.
fn maximum_of(module: &str, input: &str) -> String {
    maxima(module)
        .into_iter()
        .find(|(_, members)| members.iter().any(|m| m == input))
        .unwrap_or_else(|| panic!("no maximum over `{input}`"))
        .0
}

/// The bound on |exact value| of a step, from the module's bit-range table.
fn table_bound(module: &str, step: &str) -> u128 {
    let row = module
        .lines()
        .find(|l| l.starts_with(&format!("//! | `{step}` | ")))
        .unwrap_or_else(|| panic!("no row for `{step}`"));
    row.split(" | ").nth(3).unwrap().parse().unwrap()
}

/// The module's `ERROR` bits.
fn error_bits(module: &str) -> u64 {
    let line = module
        .lines()
        .find(|l| l.starts_with("const ERROR: f64 = f64::from_bits(0x"))
        .unwrap();
    let hex = line
        .trim_start_matches("const ERROR: f64 = f64::from_bits(0x")
        .trim_end_matches(");")
        .replace('_', "");
    u64::from_str_radix(&hex, 16).unwrap()
}

/// The bits of a hexadecimal float `0x1.<hex>p<exponent>`, read back.
fn hex_float_bits(text: &str) -> u64 {
    let rest = text.strip_prefix("0x1.").unwrap();
    let (digits, exponent) = rest.split_once('p').unwrap();
    let mut fraction = u64::from_str_radix(digits, 16).unwrap();
    fraction <<= 4 * (13 - digits.len());
    let exponent: i64 = exponent.parse().unwrap();
    (((exponent + 1023) as u64) << 52) | fraction
}

/// The module's roundings k, from its header's table.
fn roundings(module: &str) -> u32 {
    let line = module
        .lines()
        .find(|l| l.starts_with("//! | Roundings k | "))
        .unwrap();
    line.trim_start_matches("//! | Roundings k | ")
        .trim_end_matches(" |")
        .parse()
        .unwrap()
}

/// The value `text = ...;` binds in a problem.
fn binding<'p>(problem: &'p str, name: &str) -> &'p str {
    problem
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{name} = "))?.strip_suffix(';'))
        .unwrap_or_else(|| panic!("no `{name} =` line"))
}

/// The hypotheses of a problem, between `{` and `->`, without their `/\`.
fn hypotheses(problem: &str) -> Vec<String> {
    let start = problem.find("\n{\n").unwrap() + 3;
    let end = problem.find("\n->\n").unwrap();
    problem[start..end]
        .lines()
        .map(|l| l.trim().trim_end_matches(" /\\").to_owned())
        .collect()
}

/// The module's scale, as its filter writes it: maxima, products and larger-ofs.
#[derive(Clone, Debug)]
enum Scale {
    Max(String),
    Product(Vec<Scale>),
    Larger(Box<Scale>, Box<Scale>),
}

/// `text` split at each top-level occurrence of `separator`, outside every bracket.
fn split_top(text: &str, separator: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if depth == 0 && rest.starts_with(separator) {
            parts.push(std::mem::take(&mut current));
            rest = &rest[separator.len()..];
            continue;
        }
        let c = rest.chars().next().unwrap();
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        current.push(c);
        rest = &rest[c.len_utf8()..];
    }
    parts.push(current);
    parts
}

/// The scale expression `text` parsed.
fn parse_scale(text: &str) -> Scale {
    let factors = split_top(text, " * ");
    if factors.len() > 1 {
        return Scale::Product(factors.iter().map(|f| parse_scale(f)).collect());
    }
    if let Some(inner) = text
        .strip_prefix("larger(")
        .and_then(|t| t.strip_suffix(')'))
    {
        let sides = split_top(inner, ", ");
        assert_eq!(sides.len(), 2, "{text}");
        return Scale::Larger(
            Box::new(parse_scale(&sides[0])),
            Box::new(parse_scale(&sides[1])),
        );
    }
    assert!(text.starts_with("m_") && !text.contains(' '), "{text}");
    Scale::Max(text.to_owned())
}

/// Each way the scale's larger-ofs can resolve: the scale then, as Gappa writes it computed (each
/// product rounded, left to right), and its maxima as factors.
fn variants(s: &Scale) -> Vec<(String, Vec<String>)> {
    match s {
        Scale::Max(m) => vec![(m.clone(), vec![m.clone()])],
        Scale::Larger(a, b) => {
            let mut all = variants(a);
            all.extend(variants(b));
            all
        }
        Scale::Product(factors) => {
            let mut acc: Vec<(String, Vec<String>)> = variants(&factors[0]);
            for f in &factors[1..] {
                let mut next = Vec::new();
                for (text, maxima) in &acc {
                    for (t, m) in variants(f) {
                        let mut all = maxima.clone();
                        all.extend(m);
                        next.push((format!("rnd({text} * {t})"), all));
                    }
                }
                acc = next;
            }
            acc
        }
    }
}

/// The module's scale's variants.
fn module_variants(module: &str) -> Vec<(String, Vec<String>)> {
    let scale = filter_body(module)
        .iter()
        .find_map(|l| {
            l.strip_prefix("let scale = ")
                .and_then(|s| s.strip_suffix(';'))
                .map(str::to_owned)
        })
        .unwrap();
    variants(&parse_scale(&scale))
}

/// A multiset of maxima as exponents, in the filter's order of its maxima.
fn exponents(module: &str, factors: &[String]) -> Vec<i64> {
    maxima(module)
        .iter()
        .map(|(m, _)| factors.iter().filter(|f| *f == m).count() as i64)
        .collect()
}

/// The problem's computed scale's variant: its rounded text and its maxima.
fn variant_of(module: &str, problem: &str) -> (String, Vec<String>) {
    let scale = binding(problem, "scale");
    module_variants(module)
        .into_iter()
        .find(|(t, _)| t == scale)
        .unwrap_or_else(|| panic!("`scale = {scale}` is no way the module's scale resolves"))
}

/// The scale set of a step, as the module's error-analysis table writes it (`x·n1`, or
/// `max{y·n1, x·n1}`).
fn table_scale(module: &str, step: &str) -> String {
    module
        .lines()
        .filter(|l| l.starts_with(&format!("//! | `{step}` | ")))
        .find_map(|l| {
            let cells: Vec<&str> = l.split(" | ").collect();
            ["yes", "no"]
                .contains(cells.get(3)?)
                .then(|| cells[2].to_owned())
        })
        .unwrap_or_else(|| panic!("no error-analysis row for `{step}`"))
}

/// The monomials of a scale set as the module writes it.
fn monomials(scale: &str) -> BTreeSet<&str> {
    match scale.strip_prefix("max{").and_then(|s| s.strip_suffix('}')) {
        Some(list) => list.split(", ").collect(),
        None => BTreeSet::from([scale]),
    }
}

/// One operation of an `FPTaylor` problem: `(name, left, op, right)` and each operand's ratio
/// parameter, if it has one (`ratio3 * w1_3`).
type Weighted = ((String, String, String, String), [Option<String>; 2]);

/// Each `name = rnd64(<operand> op <operand>);` line of an `FPTaylor` problem.
fn fptaylor_operations(problem: &str) -> Vec<Weighted> {
    problem
        .lines()
        .filter_map(|l| {
            let (name, expr) = l.trim().split_once(" = ")?;
            let inner = expr.strip_prefix("rnd64(")?.strip_suffix(");")?;
            let t: Vec<&str> = inner.split(' ').collect();
            let operand = |at: usize| -> Option<(Option<String>, String, usize)> {
                match (t.get(at), t.get(at + 1), t.get(at + 2)) {
                    (Some(r), Some(&"*"), Some(v)) if r.starts_with("ratio") => {
                        Some((Some((*r).to_owned()), (*v).to_owned(), at + 3))
                    }
                    (Some(v), _, _) => Some((None, (*v).to_owned(), at + 1)),
                    _ => None,
                }
            };
            let (lr, a, next) = operand(0)?;
            let op = *t.get(next)?;
            let (rr, b, end) = operand(next + 1)?;
            (end == t.len() && ["+", "-", "*"].contains(&op))
                .then(|| ((name.to_owned(), a, op.to_owned(), b), [lr, rr]))
        })
        .collect()
}

/// The ratio parameters an `FPTaylor` problem's header defines: the name and the
/// `(operand's scale set, sum's scale set)` pairs of each `// ratioN = <set> / <set> = ...` line.
fn ratio_lines(problem: &str) -> Vec<(String, Vec<(String, String)>)> {
    problem
        .lines()
        .filter_map(|l| {
            let (name, ratios) = l.strip_prefix("// ")?.split_once(" = ")?;
            let pairs = ratios
                .split(" = ")
                .map(|r| {
                    r.split_once(" / ")
                        .map(|(a, b)| (a.to_owned(), b.to_owned()))
                })
                .collect::<Option<Vec<_>>>()?;
            name.starts_with("ratio").then(|| (name.to_owned(), pairs))
        })
        .collect()
}

/// A monomial as the module writes it (`x^2·n1`, or `1`), as each maximum's power.
fn powers(monomial: &str) -> BTreeMap<String, u32> {
    monomial
        .split('·')
        .filter(|f| *f != "1")
        .map(|f| match f.split_once('^') {
            Some((m, e)) => (m.to_owned(), e.parse().unwrap()),
            None => (f.to_owned(), 1),
        })
        .collect()
}

/// A scale set as each monomial's powers, in order.
type Powers = BTreeSet<Vec<(String, u32)>>;

/// A ratio of two scale sets with their common factor cancelled: each monomial's powers less the
/// least power of each maximum over the sum's set, which holds the operand's.
fn cancelled(operand: &str, sum: &str) -> (Powers, Powers) {
    let all: Vec<BTreeMap<String, u32>> = monomials(sum).into_iter().map(powers).collect();
    let names: BTreeSet<&String> = all.iter().flat_map(BTreeMap::keys).collect();
    let common: BTreeMap<&String, u32> = names
        .into_iter()
        .map(|m| {
            (
                m,
                all.iter()
                    .map(|p| p.get(m).copied().unwrap_or(0))
                    .min()
                    .unwrap(),
            )
        })
        .collect();
    let less = |set: &str| {
        monomials(set)
            .into_iter()
            .map(|m| {
                powers(m)
                    .into_iter()
                    .map(|(f, e)| {
                        let c = common.get(&f).copied().unwrap_or(0);
                        (f, e - c)
                    })
                    .filter(|(_, e)| *e > 0)
                    .collect()
            })
            .collect()
    };
    (less(operand), less(sum))
}

/// The lines of a problem's `Variables` section.
fn variables(problem: &str) -> Vec<String> {
    let start = problem.find("\nVariables\n").unwrap() + "\nVariables\n".len();
    problem[start..]
        .lines()
        .take_while(|l| !l.is_empty())
        .map(|l| l.trim().to_owned())
        .collect()
}

/// The value `  name = ...;` binds in an `FPTaylor` problem's definitions.
fn binding_indented<'p>(problem: &'p str, name: &str) -> &'p str {
    problem
        .lines()
        .find_map(|l| l.strip_prefix(&format!("  {name} = "))?.strip_suffix(';'))
        .unwrap_or_else(|| panic!("no `{name} =` line"))
}

/// 1 + 2^−53, as the `FPTaylor` problems write it: a rounded input's exact value over its
/// maximum is at most this.
fn one_plus_u() -> String {
    format!("{}/{}", (1u64 << 53) + 1, 1u64 << 53)
}

#[test]
fn each_gappa_problem_states_the_shipped_filter() {
    for p in &FILTERS {
        let name = p.name;
        let module = p.module;
        let shipped = filter_operations(module);
        assert!(shipped.len() > 2, "{name}: the filter's operations");
        let ins = inputs(module);
        for (file, problem) in p.gappa {
            let at = format!("{name}, {file}");
            assert_eq!(problem_operations(problem, "rnd"), shipped, "{at}");
            for (v, kind) in &ins {
                match kind {
                    Input::Exact => {
                        assert!(problem.contains(&format!("@FIX({v}, 0)")), "{at}: {v}");
                    }
                    Input::Rounded(bits) => {
                        let b = (1u128 << bits) - 1;
                        assert_eq!(table_bound(module, v), b, "{at}: {v}'s bits");
                        assert_eq!(binding(problem, v), format!("rndx(e_{v})"), "{at}");
                        assert!(problem.contains(&format!("@FIX(e_{v}, 0)")), "{at}: {v}");
                        assert!(
                            problem.contains(&format!("e_{v} in [-{b}, {b}]")),
                            "{at}: {v}"
                        );
                    }
                }
            }
            // Each input bounded by its maximum, and each maximum by the largest value its inputs
            // reach as the filter computes them (a rounded bound rounds to its nearest f64).
            let mut bounded = 0;
            for (m, members) in maxima(module) {
                let mut top = 0u128;
                for v in &members {
                    assert!(
                        problem.contains(&format!("{v} // {m} in [-1, 1]")),
                        "{at}: {v} by {m}"
                    );
                    bounded += 1;
                    let bound = table_bound(module, v);
                    let computed = match ins.iter().find(|(n, _)| n == v).unwrap().1 {
                        Input::Exact => bound,
                        Input::Rounded(_) => bound as f64 as u128,
                    };
                    top = top.max(computed);
                }
                let range = hypotheses(problem)
                    .into_iter()
                    .find_map(|h| h.strip_prefix(&format!("{m} in [")).map(str::to_owned))
                    .unwrap_or_else(|| panic!("{at}: no range for {m}"));
                assert!(
                    range.ends_with(&format!(", {top}]")),
                    "{at}: {m} in [{range}"
                );
            }
            assert_eq!(bounded, ins.len(), "{at}: every input has its maximum");
            // The constant, the computed scale and S.
            assert_eq!(
                hex_float_bits(binding(problem, "C")),
                error_bits(module),
                "{at}: C"
            );
            let (_, factors) = variant_of(module, problem);
            let order: Vec<String> = maxima(module).into_iter().map(|(m, _)| m).collect();
            let mut sorted = factors.clone();
            sorted.sort_by_key(|f| order.iter().position(|m| m == f));
            assert_eq!(binding(problem, "S"), sorted.join(" * "), "{at}: S");
            assert_eq!(binding(problem, "bound"), "rnd(C * scale)", "{at}");
        }
    }
}

/// `FPTaylor`'s normalised problem: the same operations, each sum's operand of a smaller scale set
/// weighted by the ratio of its set to the sum's, and nothing else.
#[test]
fn each_fptaylor_problem_states_the_shipped_filter() {
    for p in &FILTERS {
        let name = p.name;
        let module = p.module;
        let shipped = filter_operations(module);
        let ins = inputs(module);
        let (file, problem) = p.fptaylor;
        let at = format!("{name}, {file}");
        let ops = fptaylor_operations(problem);
        let plain: Vec<_> = ops.iter().map(|(o, _)| o.clone()).collect();
        assert_eq!(plain, shipped, "{at}");
        let ratios = ratio_lines(problem);
        let mut distinct = BTreeSet::new();
        for (r, pairs) in &ratios {
            for (a, b) in pairs {
                // S(a) ≤ S(b), so the ratio is in [0, 1]: b's monomials hold a's.
                assert!(
                    monomials(a).is_subset(&monomials(b)) && a != b,
                    "{at}: {r} = {a} / {b}"
                );
                // A parameter's pairs are one ratio once their common factor is cancelled.
                assert_eq!(
                    cancelled(a, b),
                    cancelled(&pairs[0].0, &pairs[0].1),
                    "{at}: {r}"
                );
            }
            assert!(
                distinct.insert(cancelled(&pairs[0].0, &pairs[0].1)),
                "{at}: {r} twice"
            );
        }
        let mut used = BTreeSet::new();
        for ((step, left, op, right), weights) in &ops {
            let own = table_scale(module, step);
            for (operand, weight) in [left, right].into_iter().zip(weights) {
                let theirs = table_scale(module, operand);
                let want = if op == "*" || theirs == own {
                    None
                } else {
                    let r = ratios
                        .iter()
                        .find(|(_, pairs)| pairs.iter().any(|(a, b)| *a == theirs && *b == own));
                    Some(
                        r.unwrap_or_else(|| panic!("{at}: `{step}`'s `{operand}` has no ratio"))
                            .0
                            .clone(),
                    )
                };
                assert_eq!(*weight, want, "{at}: `{step}`'s operand `{operand}`");
                used.extend(weight.clone());
            }
        }
        assert_eq!(used.len(), ratios.len(), "{at}: every ratio is used");
        // The variables: each input, normalised, and each ratio.
        let mut want: Vec<String> = Vec::new();
        for (v, kind) in &ins {
            match kind {
                Input::Exact => want.push(format!("float64 {v} in [-1, 1];")),
                Input::Rounded(_) => {
                    let q = one_plus_u();
                    want.push(format!("real e_{v} in [-{q}, {q}];"));
                    assert_eq!(
                        binding_indented(problem, v),
                        format!("rnd64(e_{v})"),
                        "{at}"
                    );
                }
            }
        }
        want.extend(ratios.iter().map(|(r, ..)| format!("real {r} in [0, 1];")));
        let mut have = variables(problem);
        have.sort();
        want.sort();
        assert_eq!(have, want, "{at}: the variables");
    }
}

#[test]
fn the_cases_cover_every_input_and_assume_only_what_holds() {
    for p in FILTERS.iter().filter(|p| !p.gappa.is_empty()) {
        let name = p.name;
        let module = p.module;
        let all = module_variants(module);
        // One problem for each way the scale resolves, so the problems' cases, each where its S is
        // the largest of the ways, cover every input.
        let mut scales: Vec<&str> = p.gappa.iter().map(|(_, t)| binding(t, "scale")).collect();
        scales.sort_unstable();
        let mut ways: Vec<&str> = all.iter().map(|(t, _)| t.as_str()).collect();
        ways.sort_unstable();
        ways.dedup();
        assert_eq!(scales, ways, "{name}: one problem per way");
        assert_eq!(p.gappa.len(), all.len(), "{name}: no way twice");
        let ins = inputs(module);
        let names: Vec<String> = maxima(module).into_iter().map(|(m, _)| m).collect();
        for (file, problem) in p.gappa {
            let at = format!("{name}, {file}");
            let (_, s_factors) = variant_of(module, problem);
            let s = exponents(module, &s_factors);
            // The case: S is at least every other way. A bound of an input of maximum `own` by a
            // larger maximum `top` follows when some way's monomial is S with k factors `top`
            // swapped for `own`, k ≥ 1: then S ≥ it gives top ≥ own (S's maxima are positive).
            let implied = |own: &str, top: &str| {
                let (i, j) = (
                    names.iter().position(|m| m == own).unwrap(),
                    names.iter().position(|m| m == top).unwrap(),
                );
                all.iter().any(|(_, w)| {
                    let w = exponents(module, w);
                    let diff: Vec<i64> = s.iter().zip(&w).map(|(a, b)| a - b).collect();
                    let k = diff[j];
                    k >= 1
                        && diff[i] == -k
                        && diff
                            .iter()
                            .enumerate()
                            .all(|(n, e)| n == i || n == j || *e == 0)
                })
            };
            // A term of the scale: some way's monomial, so at most S in this case.
            let a_term = |vars: &[&str]| {
                let mut m: Vec<String> = vars.iter().map(|v| maximum_of(module, v)).collect();
                m.sort();
                all.iter().any(|(_, w)| {
                    let mut w = w.clone();
                    w.sort();
                    w == m
                })
            };
            for h in hypotheses(problem) {
                let fix = h.strip_prefix("@FIX(").and_then(|r| r.strip_suffix(", 0)"));
                let justified = if let Some(v) = fix {
                    // Whole numbers: the inputs, a rounded input's exact value, the maxima.
                    ins.iter().any(|(n, kind)| match kind {
                        Input::Exact => n == v,
                        Input::Rounded(_) => v == format!("e_{n}"),
                    }) || names.iter().any(|m| m == v)
                } else if let Some((lhs, range)) = h.split_once(" // ") {
                    let vars: Vec<&str> = lhs.split(" * ").collect();
                    match (vars.as_slice(), range) {
                        // An input by its own maximum, or by a larger one the case implies.
                        ([v], r) if r.starts_with("m_") => {
                            let (m, r) = r.split_once(" in ").unwrap();
                            r == "[-1, 1]" && {
                                let own = maximum_of(module, v);
                                own == m || implied(&own, m)
                            }
                        }
                        // An input, or a product of two, that forms a term of the scale.
                        (vars, "S in [-1, 1]") => a_term(vars),
                        _ => false,
                    }
                } else if let Some((v, range)) = h.split_once(" in [") {
                    let (lo, _) = range.split_once(", ").unwrap();
                    if let Some(n) = v.strip_prefix("e_") {
                        // A rounded input's range, checked in the other test.
                        ins.iter()
                            .any(|(i, k)| i == n && matches!(k, Input::Rounded(_)))
                    } else {
                        // A maximum's range: at least 1 only where a zero one makes S zero, so
                        // every way zero and the exact value zero (the method note); its upper
                        // end is checked in the other test.
                        names.iter().any(|m| m == v)
                            && (lo == "0" || (lo == "1" && s_factors.iter().any(|f| f == v)))
                    }
                } else {
                    false
                };
                assert!(justified, "{at}: the hypothesis `{h}` is not justified");
            }
        }
    }
}

#[test]
fn the_bounds_are_the_constant_less_its_roundings() {
    for p in &FILTERS {
        let name = p.name;
        let module = p.module;
        let bits = error_bits(module);
        let k = u128::from(roundings(module));
        // C = M · 2^−p for the normal constant.
        let m = u128::from((bits & ((1 << 52) - 1)) | (1 << 52));
        let p_exp = 1075 - (bits >> 52) as u32;
        let want = m * ((1u128 << 53) - k - 1); // K · 2^(p + 53)
        for (file, gappa) in p.gappa {
            // Gappa's K, `<n>b-<e>`.
            let goal = gappa
                .lines()
                .find(|l| l.contains(" // S in [-") && l.contains(" - e_"))
                .unwrap();
            let gappa_k = goal
                .split(" // S in [-")
                .nth(1)
                .unwrap()
                .split(',')
                .next()
                .unwrap();
            let (n, e) = gappa_k.split_once("b-").unwrap();
            let (n, e): (u128, u32) = (n.parse().unwrap(), e.parse().unwrap());
            assert_eq!(n << (p_exp + 53 - e), want, "{name}, {file}: Gappa's K");
            assert!(
                goal.contains(&format!(
                    "bound -/ C * S in [-{}b-53, {}b-53]",
                    k + 1,
                    k + 1
                )),
                "{name}, {file}: R"
            );
        }
        // FPTaylor's, `&<n> * &2 ipow (-- &<e>)`, is the same K.
        let (file, fptaylor) = p.fptaylor;
        let hol = fptaylor
            .lines()
            .find_map(|l| l.strip_prefix("// hol-light-bound: &"))
            .unwrap();
        let (hn, he) = hol.split_once(" * &2 ipow (-- &").unwrap();
        let (hn, he): (u128, u32) = (
            hn.parse().unwrap(),
            he.trim_end_matches(')').parse().unwrap(),
        );
        assert_eq!(
            hn << (p_exp + 53 - he),
            want,
            "{name}, {file}: HOL Light's K"
        );
        // The optimisation, and for the exact one the total HOL Light proves: the largest
        // double h = m · 2^(e − 52) at most K, so m · 2^(e + p + 1) ≤ K · 2^(p + 53) < (m + 1) ·
        // 2^(e + p + 1).
        let header = |key: &str| {
            fptaylor
                .lines()
                .find_map(|l| l.strip_prefix(&format!("// {key}: ")))
        };
        let exact = EXACT_OPTIMISATION.contains(&name);
        let mode = if exact { "exact" } else { "approx" };
        assert_eq!(
            header("fptaylor-optimisation"),
            Some(mode),
            "{name}, {file}"
        );
        match header("hol-light-total") {
            Some(total) => {
                assert!(exact, "{name}, {file}: a total without the exact problem");
                let (fraction, e) = total
                    .strip_prefix("0x1.")
                    .and_then(|t| t.split_once('p'))
                    .unwrap();
                let digits = format!("{fraction:0<13}");
                let m = u128::from_str_radix(&digits, 16).unwrap() | (1 << 52);
                let shift =
                    u32::try_from(e.parse::<i64>().unwrap() + i64::from(p_exp) + 1).unwrap();
                assert!(m << shift <= want, "{name}, {file}: the total is above K");
                assert!(
                    (m + 1) << shift > want,
                    "{name}, {file}: a larger double is at most K"
                );
            }
            None => assert!(!exact, "{name}, {file}: no total for the exact problem"),
        }
    }
}

#[test]
fn the_problems_are_current_and_registered() {
    let committed: Vec<(&str, &str)> = FILTERS
        .iter()
        .flat_map(|p| p.gappa.iter().copied().chain([p.fptaylor]))
        .collect();
    let mut registered = 0;
    for g in REGISTRY.generators {
        if !(g.name.starts_with("gappa-") || g.name.starts_with("fptaylor-")) {
            continue;
        }
        registered += 1;
        let file = g.output.rsplit('/').next().unwrap();
        let text = committed
            .iter()
            .find(|(f, _)| *f == file)
            .unwrap_or_else(|| panic!("{file} is registered but not held here"))
            .1;
        let Generate::Plain(generate) = g.generate else {
            panic!("{}: a problem uses no randomness", g.name);
        };
        assert_eq!(
            String::from_utf8(generate().unwrap()).unwrap(),
            text,
            "{file}"
        );
    }
    assert_eq!(
        registered,
        committed.len(),
        "every problem held here is registered"
    );
    // Every generated predicate is held here.
    let mut held: Vec<&str> = FILTERS.iter().map(|p| p.name).collect();
    let mut generated: Vec<&str> = PREDICATES.iter().map(|p| p.comparison.name).collect();
    held.sort_unstable();
    generated.sort_unstable();
    assert_eq!(held, generated, "every generated filter");
}

/// The module's Guarantees paragraph, as one line.
fn guarantees(module: &str) -> String {
    let start = module.find("//! Guarantees: ").unwrap();
    module[start..]
        .lines()
        .take_while(|l| *l != "//!")
        .map(|l| l.trim_start_matches("//! "))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn each_label_names_its_problems() {
    for p in &FILTERS {
        let name = p.name;
        let label = guarantees(p.module);
        // The problems it names are those the registry generates for it.
        let named: BTreeSet<&str> = label
            .split('`')
            .filter(|w| {
                w.starts_with("gappa-")
                    && std::path::Path::new(w)
                        .extension()
                        .is_some_and(|e| e == "g")
            })
            .collect();
        let stem = format!("gappa-{}", name.replace('_', "-"));
        let registered: BTreeSet<&str> = REGISTRY
            .generators
            .iter()
            .filter(|g| g.name == stem || g.name.starts_with(&format!("{stem}-")))
            .map(|g| g.output.rsplit('/').next().unwrap())
            .collect();
        let held: BTreeSet<&str> = p.gappa.iter().map(|(f, _)| *f).collect();
        assert_eq!(named, registered, "{name}");
        assert_eq!(named, held, "{name}");
        assert!(
            label.contains(&format!("`{}`", p.fptaylor.0)),
            "{name}: {label}"
        );
        if let Some((_, issue)) = UNPROVEN.iter().find(|(n, _)| *n == name) {
            // Checked, not proven: no Gappa problem, and the open issue named.
            assert!(
                label.contains("Its filter's bound is checked, not proven (chapter 28.6)"),
                "{name}: {label}"
            );
            assert!(label.contains(&format!("(issue #{issue})")), "{name}");
            assert!(named.is_empty(), "{name}");
        } else {
            assert!(
                label.contains("Its filter's bound is proven (chapter 28.6)"),
                "{name}: {label}"
            );
            assert!(!label.contains("not proven"), "{name}: {label}");
            assert!(!named.is_empty(), "{name}");
        }
    }
}

#[test]
fn a_single_problem_refuses_a_scale_of_several_monomials() {
    for name in ["plane_side", "incircle", "insphere", "lpi_weight"] {
        let p = PREDICATES
            .iter()
            .find(|p| p.comparison.name == name)
            .unwrap();
        let d = derivation(p).unwrap();
        let gappa = gappa_problem(&d, "test").unwrap_err();
        assert!(
            gappa.0.contains("one problem per case"),
            "{name}: {}",
            gappa.0
        );
        // FPTaylor's normalised problem needs no cases.
        assert!(fptaylor_problem(&d, "test").is_ok(), "{name}");
    }
}

/// An unproven filter's issue is still needed: some problem of its cases is refused, with why.
#[test]
fn each_unproven_filter_is_one_gappa_cannot_state() {
    for (name, issue) in UNPROVEN {
        let p = PREDICATES
            .iter()
            .find(|p| p.comparison.name == name)
            .unwrap();
        let d = derivation(p).unwrap();
        let cases = gappa_cases(&d).unwrap();
        let refused = if cases.is_empty() {
            gappa_problem(&d, "test").err()
        } else {
            cases
                .iter()
                .find_map(|c| gappa_case_problem(&d, "test", c).err())
        };
        assert!(
            refused.is_some(),
            "{name}: Gappa states it; issue #{issue} is stale"
        );
    }
}
