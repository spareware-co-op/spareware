//! Every hard-to-round case in CORE-MATH's worst-case files, against the rigorous reference.
//!
//! The other `math_*` tests carry a sample of each file. These tests read the complete files from
//! a CORE-MATH checkout and are ignored by default, since the larger files hold a million or more
//! inputs and take minutes. To run them, point `CORE_MATH_DIR` at the checkout's
//! `src/binary64` directory (commit `0ce0cbc15935e24c656280e5a46c34e89f301b7a` was used for this
//! port) and run the ignored tests:
//!
//! ```text
//! CORE_MATH_DIR=/path/to/core-math/src/binary64 cargo test -p geom-num --release \
//!     --test math_full_worst_cases -- --ignored
//! ```
//!
//! A file that is not present is reported and skipped. For `sinpi` the images of the file's set (b)
//! under every power of two up to `2^968`, which the file says to add, are checked too.
//!
//! The functions return a `Real` or an error (the module documentation of `geom_num::math`): each
//! call is checked against what it owes, `reference::owed`, from the reference's IEEE 754 value
//! (with a zero `sinpi` or `cospi` as +0, the crate's one-zero rule).

// Test code may use native arithmetic and casts (Cargo.toml); helpers outside `#[test]` functions
// may panic, as clippy.toml allows inside them.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::panic,
    reason = "tests may use native arithmetic, casts and std reductions, and stop loudly"
)]

#[path = "math_reference.rs"]
mod reference;

use std::path::PathBuf;

use geom_num::{NumError, Real};
use reference::{Owed, assert_owed, owed};

/// The outcome of a call of a public maths function, to compare with what it owes.
fn outcome(r: Result<Real, NumError>) -> Owed {
    match r {
        Ok(v) => Owed::Value(v.get()),
        Err(NumError::NotFinite) => Owed::NotFinite,
        Err(NumError::OutsideDomain) => Owed::OutsideDomain,
        Err(e) => panic!("a maths function returned {e:?}"),
    }
}

/// One input from a worst-case file: a hex or decimal literal rounded to nearest as C's `strtod`
/// reads it, or `None` for NaN and infinities (special values are tested elsewhere).
fn parse(token: &str) -> Option<f64> {
    let t = token.trim();
    let lower = t.to_ascii_lowercase();
    if t.is_empty() || lower.contains("nan") || lower.contains("inf") {
        return None;
    }
    if lower.contains("0x") {
        Some(reference::hx_rounded(t))
    } else {
        t.parse().ok()
    }
}

/// The data lines of `<CORE_MATH_DIR>/<function>/<function>.wc`, comments removed, or `None`.
fn lines(function: &str) -> Option<Vec<String>> {
    let dir = std::env::var_os("CORE_MATH_DIR")?;
    let path = PathBuf::from(dir)
        .join(function)
        .join(format!("{function}.wc"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("{} not found; skipped", path.display());
        return None;
    };
    Some(
        text.lines()
            .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
    )
}

/// The inputs of a worst-case file, in order.
fn inputs(lines: &[String]) -> Vec<f64> {
    lines
        .iter()
        .filter_map(|line| line.split_whitespace().next().and_then(parse))
        .collect()
}

/// Checks a univariate function on every input of its file, plus `extra`, and on each input's
/// negation too if `both_signs` (for odd and even functions, whose files list positive inputs
/// only).
fn univariate(
    function: &str,
    ours: fn(f64) -> Result<Real, NumError>,
    oracle: fn(f64) -> f64,
    both_signs: bool,
    extra: fn(&[f64]) -> Vec<f64>,
) {
    let Some(lines) = lines(function) else {
        eprintln!("CORE_MATH_DIR not set or {function}.wc missing; skipped");
        return;
    };
    let listed = inputs(&lines);
    let more = extra(&listed);
    let mut n = 0usize;
    for &x in listed.iter().chain(more.iter()) {
        for x in if both_signs { vec![x, -x] } else { vec![x] } {
            assert_owed(
                outcome(ours(x)),
                owed(&[x], oracle(x)),
                &format!("{function}({x:e} = {:#018x})", x.to_bits()),
            );
            n += 1;
        }
    }
    eprintln!(
        "{function}: {n} inputs ({} added to the file's), all correctly rounded",
        more.len() * if both_signs { 2 } else { 1 }
    );
}

/// No inputs beyond the file's.
fn none(_: &[f64]) -> Vec<f64> {
    Vec::new()
}

/// sinpi.wc's instruction: "for 1 <= e <= 968, add the inputs from set (b) multiplied by 2^e",
/// set (b) being its worst cases in `[2^−1022, 2^−1021)`. These images are the hard cases of
/// `sinpi` up to `[2^−54, 2^−53)` (the proof's step A7 in `sincospi.rs`).
fn sinpi_set_b_images(listed: &[f64]) -> Vec<f64> {
    let lo = f64::MIN_POSITIVE;
    let set_b: Vec<f64> = listed
        .iter()
        .copied()
        .filter(|x| (lo..2.0 * lo).contains(x))
        .collect();
    let mut out = Vec::with_capacity(set_b.len() * 968);
    for e in 1..=968 {
        let scale = f64::from_bits(((1023 + e) as u64) << 52); // 2^e, exact
        out.extend(set_b.iter().map(|x| x * scale));
    }
    out
}

#[test]
#[ignore = "needs CORE_MATH_DIR and minutes"]
fn sinpi_all_worst_cases() {
    univariate(
        "sinpi",
        geom_num::math::sinpi,
        |x| reference::sinpi(x) + 0.0, // a zero as +0
        true,
        sinpi_set_b_images,
    );
}

#[test]
#[ignore = "needs CORE_MATH_DIR and minutes"]
fn cospi_all_worst_cases() {
    univariate(
        "cospi",
        geom_num::math::cospi,
        |x| reference::cospi(x) + 0.0, // a zero as +0
        true,
        none,
    );
}

#[test]
#[ignore = "needs CORE_MATH_DIR and minutes"]
fn exp_all_worst_cases() {
    univariate("exp", geom_num::math::exp, reference::exp, false, none);
}

#[test]
#[ignore = "needs CORE_MATH_DIR and minutes"]
fn ln_all_worst_cases() {
    univariate("log", geom_num::math::ln, reference::ln, false, none);
}

#[test]
#[ignore = "needs CORE_MATH_DIR and minutes"]
fn atan2_all_worst_cases() {
    let Some(lines) = lines("atan2") else {
        eprintln!("CORE_MATH_DIR not set or atan2.wc missing; skipped");
        return;
    };
    let mut n = 0usize;
    for line in &lines {
        let mut parts = line.split(',');
        let (Some(y), Some(x)) = (parts.next().and_then(parse), parts.next().and_then(parse))
        else {
            continue;
        };
        for (y, x) in [(y, x), (-y, x), (y, -x), (-y, -x)] {
            let what = format!("atan2({:#018x}, {:#018x})", y.to_bits(), x.to_bits());
            assert_owed(
                outcome(geom_num::math::atan2(y, x)),
                owed(&[y, x], reference::atan2(y, x)),
                &what,
            );
            n += 1;
        }
    }
    eprintln!("atan2: {n} inputs, all correctly rounded");
}
