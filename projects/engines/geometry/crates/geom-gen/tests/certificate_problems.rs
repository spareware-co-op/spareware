//! Attacks on piece G35 (the filter-certificate spike): the problems Gappa and `FPTaylor` prove are
//! the filters that ship (chapter 28.6). `kernel-certificates.yml` runs the tools on the committed
//! problems; these tests hold the problems to the committed filters, reading both as text, so a
//! proof is about the code that ships, not about a copy of it.
//!
//! - `each_problem_states_the_shipped_filter`: every `let x = a op b;` operation of the committed
//!   `filter`, in order, is `x = rnd(a op b);` in the Gappa problem and `x = rnd64(a op b);` in the
//!   `FPTaylor` problem, and nothing else is; the coordinate differences are the problems' inputs, each
//!   bounded by the maximum the filter takes over it; the Gappa problem's constant has the bits of
//!   the module's `ERROR` and its computed scale rounds each product of the module's `scale`.
//! - `the_bounds_are_the_constant_less_its_roundings`: the error bound K both tools check is exactly
//!   C · (1 − (k + 1) · 2^−53) for the module's constant C and roundings k, worked out here from
//!   the module's text, and the Gappa problem bounds the computed bound's relative error by
//!   (k + 1) · 2^−53.
//! - `the_problems_are_current`: the committed problems are what the generators write now.
//! - `the_problems_refuse_what_they_cannot_state`: a filter of rounded inputs (`plane_side`) and
//!   one whose scale has several monomials (`incircle`, `insphere`) are refused, with the reason.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
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

use geom_gen::predicate_certificates::{fptaylor_problem, gappa_problem};
use geom_gen::predicates::{
    PREDICATES, derivation, generate_fptaylor_orient2d, generate_fptaylor_orient3d,
    generate_gappa_orient2d, generate_gappa_orient3d,
};

/// Each proven predicate: its committed module, Gappa problem and `FPTaylor` problem.
const CASES: [(&str, &str, &str, &str); 2] = [
    (
        "orient2d",
        include_str!("../../geom-num/src/generated/orient2d.rs"),
        include_str!("../generated/gappa-orient2d.g"),
        include_str!("../generated/fptaylor-orient2d.fpt"),
    ),
    (
        "orient3d",
        include_str!("../../geom-num/src/generated/orient3d.rs"),
        include_str!("../generated/gappa-orient3d.g"),
        include_str!("../generated/fptaylor-orient3d.fpt"),
    ),
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

/// The coordinate differences of the filter, `let x = exact_f64(..) - exact_f64(..);`.
fn differences(module: &str) -> Vec<String> {
    filter_body(module)
        .iter()
        .filter(|l| l.contains("exact_f64("))
        .map(|l| {
            l.strip_prefix("let ")
                .unwrap()
                .split_once(" = ")
                .unwrap()
                .0
                .to_owned()
        })
        .collect()
}

/// `(maximum, differences)` of each `let m = largest_magnitude([..]);`.
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

#[test]
fn each_problem_states_the_shipped_filter() {
    for (name, module, gappa, fptaylor) in CASES {
        let shipped = filter_operations(module);
        assert!(shipped.len() > 2, "{name}: the filter's operations");
        assert_eq!(problem_operations(gappa, "rnd"), shipped, "{name}: Gappa");
        assert_eq!(
            problem_operations(fptaylor, "rnd64"),
            shipped,
            "{name}: FPTaylor"
        );
        // The inputs: the coordinate differences, each bounded by its maximum.
        let diffs = differences(module);
        for d in &diffs {
            assert!(gappa.contains(&format!("@FIX({d}, 0)")), "{name}: {d}");
            assert!(
                fptaylor.contains(&format!("float64 {d} in [-1, 1];")),
                "{name}: {d}"
            );
        }
        let mut bounded = 0;
        for (m, members) in maxima(module) {
            for d in members {
                assert!(
                    gappa.contains(&format!("{d} // {m} in [-1, 1]")),
                    "{name}: {d} by {m}"
                );
                bounded += 1;
            }
        }
        assert_eq!(
            bounded,
            diffs.len(),
            "{name}: every difference has its maximum"
        );
        // The constant and the computed scale.
        assert_eq!(
            hex_float_bits(binding(gappa, "C")),
            error_bits(module),
            "{name}: C"
        );
        let scale = filter_body(module)
            .iter()
            .find_map(|l| {
                l.strip_prefix("let scale = ")
                    .and_then(|s| s.strip_suffix(';'))
                    .map(str::to_owned)
            })
            .unwrap();
        let mut factors = scale.split(" * ");
        let mut rounded = factors.next().unwrap().to_owned();
        for f in factors {
            rounded = format!("rnd({rounded} * {f})");
        }
        assert_eq!(binding(gappa, "scale"), rounded, "{name}: the scale");
        assert_eq!(
            binding(gappa, "bound"),
            "rnd(C * scale)",
            "{name}: the bound"
        );
    }
}

#[test]
fn the_bounds_are_the_constant_less_its_roundings() {
    for (name, module, gappa, fptaylor) in CASES {
        let bits = error_bits(module);
        let k = u128::from(roundings(module));
        // C = M · 2^−p for the normal constant.
        let m = u128::from((bits & ((1 << 52) - 1)) | (1 << 52));
        let p = 1075 - (bits >> 52) as u32;
        let want = m * ((1u128 << 53) - k - 1); // K · 2^(p + 53)
        // Gappa's K, `<n>b-<e>`, and FPTaylor's, `&<n> * &2 ipow (-- &<e>)`.
        let goal = gappa.lines().find(|l| l.contains(" // S in [-")).unwrap();
        let gappa_k = goal
            .split(" // S in [-")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        let (n, e) = gappa_k.split_once("b-").unwrap();
        let (n, e): (u128, u32) = (n.parse().unwrap(), e.parse().unwrap());
        assert_eq!(n << (p + 53 - e), want, "{name}: Gappa's K");
        let hol = fptaylor
            .lines()
            .find_map(|l| l.strip_prefix("// hol-light-bound: &"))
            .unwrap();
        let (hn, he) = hol.split_once(" * &2 ipow (-- &").unwrap();
        let (hn, he): (u128, u32) = (
            hn.parse().unwrap(),
            he.trim_end_matches(')').parse().unwrap(),
        );
        assert_eq!((hn, he), (n, e), "{name}: HOL Light's K is Gappa's");
        assert!(
            goal.contains(&format!(
                "bound -/ C * S in [-{}b-53, {}b-53]",
                k + 1,
                k + 1
            )),
            "{name}: R"
        );
    }
}

#[test]
fn the_problems_are_current() {
    let generated = [
        generate_gappa_orient2d().unwrap(),
        generate_fptaylor_orient2d().unwrap(),
        generate_gappa_orient3d().unwrap(),
        generate_fptaylor_orient3d().unwrap(),
    ];
    let committed = [CASES[0].2, CASES[0].3, CASES[1].2, CASES[1].3];
    for (g, c) in generated.iter().zip(committed) {
        assert_eq!(String::from_utf8(g.clone()).unwrap(), c);
    }
}

#[test]
fn the_problems_refuse_what_they_cannot_state() {
    for name in ["plane_side", "incircle", "insphere"] {
        let p = PREDICATES
            .iter()
            .find(|p| p.comparison.name == name)
            .unwrap();
        let d = derivation(p).unwrap();
        let gappa = gappa_problem(&d, "test").unwrap_err();
        let fptaylor = fptaylor_problem(&d, "test").unwrap_err();
        let why = if name == "plane_side" { "G40" } else { "one problem per case" };
        assert!(gappa.0.contains(why), "{name}: {}", gappa.0);
        assert_eq!(gappa, fptaylor, "{name}");
    }
}
