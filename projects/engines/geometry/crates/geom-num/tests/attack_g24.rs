//! Attacks on piece G24: `libm`, the one published crate inside the kernel, held to its written
//! audit (`projects/kit/notes/G24-libm-audit.md`), and `Real`'s total order with its tie rules
//! (chapters 21 and 29.20 of the kernel architecture).
//!
//! The claims attacked:
//!
//! - **The audit covers every `libm` function the kernel calls**: every `libm::<name>` in
//!   `geom-num`'s source is one of the 21 functions the audit's section 2 lists, and each of them
//!   appears there; a new call fails here until the audit covers it.
//! - **The audit is of the bytes that ship**: `Cargo.lock` pins `libm` 0.2.16 at the checksum the
//!   audit names, so a version or checksum change fails here until a new audit is written; and
//!   `deny-no-std.toml`'s record names the audit's path (the merge gate checks the file exists).
//! - **No audited function misbehaves on any binade**: each, through `geom_num::math` and
//!   `geom_num::math::unproven`, on seeded inputs over every binade of the doubles, both signs,
//!   and the arguments that reach `libm`'s `rem_pio2_large`, the code with the most unchecked
//!   indexing. In a debug build that indexing is checked (the audit, section 4), so an index out
//!   of range would panic here. Each result is also checked against a property it must have.
//! - **`Real`'s total order and its tie rules**: equality is equality of bits; −0 sorts before +0;
//!   `total_max` and `total_min` return the larger and smaller in that order, and on a tie, which
//!   is always the same bits, the first argument; every special value (zeros, subnormals, the
//!   extremes) is ordered as its sign-magnitude bits are.
//!
//! Seeds: the randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use geom_num::math::{self, unproven};
use geom_num::{NumError, Real};
use geom_test::rng::SplitMix64;
use geom_test::rust_source::scan;
use geom_test::seed::{Seed, resolve};

/// The 21 `libm` functions the audit's section 2 lists.
const AUDITED: [&str; 21] = [
    "acos", "asin", "atan", "atan2", "cbrt", "ceil", "cos", "exp", "fabs", "floor", "fma", "hypot",
    "log", "log10", "log2", "pow", "rint", "sin", "sqrt", "tan", "trunc",
];

/// The audit.
const AUDIT: &str = include_str!("../../../../../kit/notes/G24-libm-audit.md");

/// The workspace's lock file.
const LOCK: &str = include_str!("../../../../../../Cargo.lock");

/// The vetted allow-list and its records.
const ALLOW_LIST: &str = include_str!("../../../deny-no-std.toml");

/// The audited release's checksum, as `Cargo.lock` writes it.
const CHECKSUM: &str = "b6d2cec3eae94f9f509c767b45932f1ada8350c4bdb85af2fcab4a3c14807981";

/// Every `libm::<name>` in `text`.
fn libm_calls(text: &str) -> BTreeSet<String> {
    text.match_indices("libm::")
        .map(|(at, _)| {
            text[at + "libm::".len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect()
        })
        .collect()
}

/// Every `libm` function `geom-num`'s kernel code calls is audited, and the audit lists each. Test
/// code (`#[cfg(test)]` modules, found by `geom_test::rust_source`) may call others, which never
/// ship.
#[test]
#[cfg_attr(
    target_os = "wasi",
    ignore = "reads the crate's folder; wasm32-wasip1 under wasmtime is given no folder to read"
)]
fn attack_the_audit_covers_every_libm_function_the_kernel_calls() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut folders = vec![src];
    let mut called = BTreeSet::new();
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let scanned = scan(&text).unwrap();
                let kernel: Vec<&str> = text
                    .lines()
                    .enumerate()
                    .filter(|(i, _)| !scanned.is_test_line(i + 1))
                    .map(|(_, line)| line)
                    .collect();
                called.extend(libm_calls(&kernel.join(
                    "
",
                )));
            }
        }
    }
    let audited: BTreeSet<String> = AUDITED.iter().map(|s| (*s).to_owned()).collect();
    let unaudited: Vec<&String> = called.difference(&audited).collect();
    assert!(
        unaudited.is_empty(),
        "libm functions called but not audited: {unaudited:?}"
    );
    let section = AUDIT
        .split("## 2. What the kernel calls")
        .nth(1)
        .and_then(|s| s.split("## 3.").next())
        .expect("the audit's section 2");
    for name in AUDITED {
        assert!(
            section.contains(&format!("`{name}`")),
            "the audit's section 2 lists `{name}`"
        );
        assert!(
            called.contains(name),
            "`{name}` is audited but no longer called: trim the list"
        );
    }
}

/// The audit is of the release `Cargo.lock` pins, and the allow-list's record names it.
#[test]
fn attack_the_audit_is_of_the_bytes_that_ship() {
    let lock = LOCK.replace("\r\n", "\n");
    let entry = lock
        .split("[[package]]\n")
        .find(|p| p.starts_with("name = \"libm\"\n"))
        .expect("libm in Cargo.lock");
    assert!(entry.contains("version = \"0.2.16\"\n"), "{entry}");
    assert!(entry.contains("source = \"registry+https://github.com/rust-lang/crates.io-index\"\n"));
    assert!(
        entry.contains(&format!("checksum = \"{CHECKSUM}\"")),
        "Cargo.lock's libm is not the audited release: write a new audit\n{entry}"
    );
    assert!(
        AUDIT.contains(CHECKSUM),
        "the audit names the checksum it audited"
    );
    assert!(
        ALLOW_LIST.contains("audited: projects/kit/notes/G24-libm-audit.md"),
        "deny-no-std.toml's record for libm names the audit"
    );
}

/// The test's generator, from the printed seed.
fn rng(label: &str) -> SplitMix64 {
    SplitMix64::new(resolve(label, Seed(0x6724_0000_5eed_0249)).unwrap())
}

/// A double of binade `e` (the biased exponent, 0 for the subnormals) with a random significand
/// and sign.
fn in_binade(rng: &mut SplitMix64, e: u64) -> f64 {
    let bits = rng.next_u64();
    f64::from_bits((bits & (1 << 63)) | (e << 52) | (bits & ((1 << 52) - 1)))
}

/// A result's value, or `None` for an error of one of the kinds the maths module documents.
fn value(r: Result<Real, NumError>) -> Option<f64> {
    match r {
        Ok(v) => Some(v.get()),
        Err(NumError::NotFinite | NumError::OutsideDomain) => None,
        Err(e) => panic!("an error the maths module does not document: {e:?}"),
    }
}

/// Every audited function on every binade, both signs: no panic (indexing is checked in debug),
/// and each result has a property it must have.
#[test]
fn attack_every_audited_function_on_every_binade() {
    let mut rng = rng("every binade");
    let rounds = if cfg!(debug_assertions) { 4 } else { 32 };
    for e in 0..2047_u64 {
        for _ in 0..rounds {
            let x = in_binade(&mut rng, e);
            let binade = rng.next_u64() % 2047;
            let y = in_binade(&mut rng, binade);
            check_one(x, y);
        }
    }
    // The arguments of sin, cos and tan that `rem_pio2_large` reduces: from 2^20 · π/2 up, and
    // the multiples of π/2 near which the reduction needs every bit of its table.
    for k in 0..4096_u64 {
        let big = in_binade(&mut rng, 1043 + k % 980).abs();
        check_one(big, -big);
        let near = (k as f64 + 1.0) * core::f64::consts::FRAC_PI_2 * 1e6;
        check_one(near, near.next_up());
    }
}

/// The checks on one input pair.
fn check_one(x: f64, y: f64) {
    for (name, v) in [
        ("sin", value(unproven::sin(x))),
        ("cos", value(unproven::cos(x))),
        ("sinpi", value(unproven::sinpi(x))),
        ("cospi", value(unproven::cospi(x))),
    ] {
        let v = v.unwrap_or_else(|| panic!("{name}({x:e}) of a finite argument"));
        assert!(v.abs() <= 1.0, "{name}({x:e}) = {v:e}");
    }
    let _ = value(unproven::tan(x));
    for (name, v) in [("asin", unproven::asin(x)), ("acos", unproven::acos(x))] {
        match value(v) {
            Some(v) => assert!(
                x.abs() <= 1.0 && v.abs() <= core::f64::consts::PI,
                "{name}({x:e})"
            ),
            None => assert!(x.abs() > 1.0, "{name}({x:e}) refused inside its domain"),
        }
    }
    let atan = value(unproven::atan(x)).unwrap();
    assert!(atan.abs() <= core::f64::consts::FRAC_PI_2);
    let atan2 = value(unproven::atan2(y, x)).unwrap();
    assert!(atan2.abs() <= core::f64::consts::PI);
    for (name, v) in [
        ("ln", unproven::ln(x)),
        ("log2", unproven::log2(x)),
        ("log10", unproven::log10(x)),
    ] {
        assert_eq!(value(v).is_some(), x > 0.0, "{name}({x:e})");
    }
    if let Some(v) = value(unproven::exp(x)) {
        assert!(v >= 0.0, "exp({x:e}) = {v:e}");
    }
    let _ = value(unproven::pow(x, y));
    let _ = value(unproven::pow(x.abs(), y));
    if let Some(h) = value(unproven::hypot(x, y)) {
        assert!(h >= x.abs().max(y.abs()), "hypot({x:e}, {y:e}) = {h:e}");
    }
    let c = value(unproven::cbrt(x)).unwrap();
    assert!(c == 0.0 || c.signum() == x.signum(), "cbrt({x:e}) = {c:e}");
    // The functions the module takes as exact: sqrt, fma, abs, floor, ceil and round to even.
    match value(math::sqrt(x)) {
        Some(r) => assert!(x >= 0.0 || x == 0.0, "sqrt({x:e}) = {r:e}"),
        None => assert!(x < 0.0, "sqrt({x:e}) refused"),
    }
    assert_eq!(value(math::abs(x)), Some(x.abs()));
    let (floor, ceil, even) = (
        value(math::floor(x)).unwrap(),
        value(math::ceil(x)).unwrap(),
        value(math::round_ties_even(x)).unwrap(),
    );
    assert!(
        floor <= x && x <= ceil && ceil - floor <= 1.0,
        "{x:e}: {floor:e} {ceil:e}"
    );
    assert!(even == floor || even == ceil, "{x:e}: {even:e}");
    if let Some(f) = value(math::fma(x, 1.0, 0.0)) {
        assert_eq!(f.to_bits(), (x + 0.0).to_bits(), "fma({x:e}, 1, 0)");
    }
}

// ---------------------------------------------------------------------------------------------
// Real: total order and tie rules.
// ---------------------------------------------------------------------------------------------

/// The special values, in their total order.
fn specials() -> Vec<f64> {
    vec![
        f64::MIN,
        -1.0,
        -f64::MIN_POSITIVE,
        -f64::from_bits(1),
        -0.0,
        0.0,
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        1.0,
        f64::MAX,
    ]
}

/// The order is totalOrder: as the sign-magnitude bits; equality is equality of bits.
#[test]
fn attack_real_orders_by_total_order() {
    let reals: Vec<Real> = specials()
        .into_iter()
        .map(|v| Real::new(v).unwrap())
        .collect();
    for (i, a) in reals.iter().enumerate() {
        for (j, b) in reals.iter().enumerate() {
            assert_eq!(a.cmp(b), i.cmp(&j), "{:e} against {:e}", a.get(), b.get());
            assert_eq!(a == b, a.get().to_bits() == b.get().to_bits());
        }
    }
    for refused in [f64::NAN, -f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(Real::new(refused), Err(NumError::NotFinite));
    }
}

/// `total_max` and `total_min`: the larger and the smaller in totalOrder, so −0 is below +0; on a
/// tie, which is the same bits, the first argument (the same value either way).
#[test]
fn attack_real_min_and_max_follow_their_tie_rules() {
    let (neg_zero, zero) = (Real::new(-0.0).unwrap(), Real::new(0.0).unwrap());
    assert_eq!(neg_zero.total_max(zero).get().to_bits(), 0.0_f64.to_bits());
    assert_eq!(zero.total_max(neg_zero).get().to_bits(), 0.0_f64.to_bits());
    assert_eq!(
        neg_zero.total_min(zero).get().to_bits(),
        (-0.0_f64).to_bits()
    );
    assert_eq!(
        zero.total_min(neg_zero).get().to_bits(),
        (-0.0_f64).to_bits()
    );
    let reals: Vec<Real> = specials()
        .into_iter()
        .map(|v| Real::new(v).unwrap())
        .collect();
    for a in &reals {
        for b in &reals {
            assert_eq!(a.total_max(*b), (*a).max(*b));
            assert_eq!(a.total_min(*b), (*a).min(*b));
            assert_eq!(
                a.total_max(*b).get().to_bits(),
                b.total_max(*a).get().to_bits()
            );
        }
    }
}
