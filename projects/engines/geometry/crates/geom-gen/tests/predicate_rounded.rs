//! Rounded inputs: an input that is an `f64` copy of a wider integer (a plane's 83-bit normal or
//! 125-bit offset, chapter 23.2) carries its rounding into the filter's bound.
//!
//! The predicate here is the side of a plane n · p + d = 0 for a grid point p, with n and d at
//! their full widths, so their `f64` copies are rounded. Its filter is evaluated by an
//! interpreter of the derivation's steps, in the generated code's order and with its constant,
//! on near-zero cases built to make every rounding count: n at 83 bits, p at the grid's extremes,
//! and d chosen so that n · p + d is a small whole number of either sign. Every sign the filter
//! gives must be exact (checked in exact `i128` arithmetic, which holds every value here), and
//! the filter must certify some cases and leave others. The description is the one geom-gen
//! emits as `geom_num::generated::plane_side`; that compiled module is tested against exact
//! arithmetic, independently of this interpreter and of the generator's flattening, in geom-num's
//! `tests/generated_bounds.rs`. Here the emitted module is checked to be what is committed, and
//! a rounded input wider than an `i128` is refused.

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
    clippy::indexing_slicing,
    clippy::many_single_char_names
)]

use geom_gen::predicate_derive::{Derivation, ScaleExpr, derive};
use geom_gen::predicate_emit::predicate_module;
use geom_gen::predicate_lang::{Op, StepKind};
use geom_gen::predicates::{PLANE_SIDE, generate_plane_side};
use geom_test::{Seed, SplitMix64};

const M: i128 = (1 << 40) - 1;

/// The inputs: the exact integers, by name.
struct Inputs {
    n: [i128; 3],
    d: i128,
    p: [i128; 3],
}

/// The filter, interpreted: every step in `f64` in the derivation's order (rounded inputs as
/// their nearest `f64`), the group maxima, the scale by the scale expression, and the decision
/// against the constant. `Some(sign)` as the generated `filter` would give it.
fn filter(d: &Derivation, x: &Inputs) -> Option<i8> {
    let mut v: Vec<f64> = Vec::new();
    for step in &d.steps {
        let value = match &step.kind {
            StepKind::LoadCoord { axis, .. } => x.p[*axis as usize] as f64,
            StepKind::LoadRounded { input } => match input.as_str() {
                "nx" => x.n[0] as f64,
                "ny" => x.n[1] as f64,
                "nz" => x.n[2] as f64,
                "d" => x.d as f64,
                other => panic!("{other}"),
            },
            StepKind::Translate { .. } => panic!("no translations here"),
            StepKind::Op { op, left, right } => {
                let (a, b) = (v[*left], v[*right]);
                match op {
                    Op::Add => a + b,
                    Op::Sub => a - b,
                    Op::Mul => a * b,
                }
            }
        };
        v.push(value);
    }
    let maxima: Vec<f64> = d
        .groups
        .iter()
        .map(|g| {
            g.vars
                .iter()
                .map(|&i| v[i].abs())
                .fold(0.0, |m: f64, a| if m < a { a } else { m })
        })
        .collect();
    let scale = eval(&d.scale_expr, &maxima);
    let bound = f64::from_bits(d.constant_bits) * scale;
    let r = v[d.result];
    if r > bound {
        Some(1)
    } else if r < -bound {
        Some(-1)
    } else if scale == 0.0 {
        Some(0)
    } else {
        None
    }
}

/// The scale expression, evaluated as the generated code evaluates it.
fn eval(e: &ScaleExpr, m: &[f64]) -> f64 {
    match e {
        ScaleExpr::Max(g) => m[*g],
        ScaleExpr::One => 1.0,
        ScaleExpr::Mul(a, b) => eval(a, m) * eval(b, m),
        ScaleExpr::Larger(a, b) => {
            let (a, b) = (eval(a, m), eval(b, m));
            if a < b { b } else { a }
        }
    }
}

/// The exact sign of n · p + d.
fn exact(x: &Inputs) -> i8 {
    let s = x.n[0] * x.p[0] + x.n[1] * x.p[1] + x.n[2] * x.p[2] + x.d;
    s.signum() as i8
}

#[test]
fn the_rounded_inputs_carry_their_error_into_the_bound() {
    let d = derive(PLANE_SIDE).unwrap();
    let names: Vec<&str> = d.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["x", "y", "z", "n", "d"]);
    // A rounded input's own error is u, and the bound grows with it: the leaf of nx.
    let nx = d.steps.iter().position(|s| s.name == "nx").unwrap();
    assert_eq!(d.certs[nx].unit_error.to_string(), "u");
    assert_eq!(d.certs[nx].bits, 83);
    // 3 (2^83 − 1)(2^40 − 1) + 2^125 − 1 < 1.75 · 2^125: 126 bits, in i128.
    assert_eq!(d.certs[d.result].bits, 126);
    println!(
        "plane_side: unit error {}, constant {:e}",
        d.certs[d.result].unit_error,
        f64::from_bits(d.constant_bits)
    );

    let seed = Seed(0x6727_0000_0000_0123);
    println!("seed: {seed}");
    let mut rng = SplitMix64::new(seed);
    // A magnitude below 2^bits, and a random sign.
    let magnitude = |bits: u32, rng: &mut SplitMix64| -> i128 {
        let v = (i128::from(rng.next_u64()) << 64) | i128::from(rng.next_u64());
        v & ((1 << bits) - 1)
    };
    let signed = |v: i128, rng: &mut SplitMix64| if rng.next_u64() & 1 == 1 { -v } else { v };
    let cases = if cfg!(debug_assertions) {
        20_000
    } else {
        1_000_000
    };
    let (mut certified, mut left) = (0u64, 0u64);
    for i in 0..cases {
        // n at its full 83 bits: the top bit set, the rest random.
        let n: [i128; 3] = std::array::from_fn(|_| {
            let v = magnitude(82, &mut rng) | (1 << 82);
            signed(v, &mut rng)
        });
        // p at the grid's extremes, or anywhere on it.
        let p: [i128; 3] = std::array::from_fn(|_| match rng.next_u64() % 4 {
            0 => M,
            1 => -M,
            _ => {
                let v = magnitude(40, &mut rng).min(M);
                signed(v, &mut rng)
            }
        });
        // d makes the sum a small whole number t of either sign, up to 2^k for k up to 90.
        let k = (rng.next_u64() % 91) as u32;
        let t = if i % 7 == 0 {
            0
        } else {
            let v = magnitude(k, &mut rng);
            signed(v, &mut rng)
        };
        let dot = n[0] * p[0] + n[1] * p[1] + n[2] * p[2];
        let x = Inputs { n, d: t - dot, p };
        assert!(x.d.unsigned_abs() < 1 << 125);
        let truth = exact(&x);
        match filter(&d, &x) {
            Some(s) => {
                assert_eq!(s, truth, "n {n:?} p {p:?} d {}", x.d);
                certified += 1;
            }
            None => left += 1,
        }
    }
    println!("certified {certified}, left to the exact path {left}");
    assert!(certified > 0 && left > 0);
}

/// The emitted module takes each rounded input as its exact `i128`, converts it with
/// `rounded_f64` in the filter and checks its range with `within` in the exact path; it is the
/// committed `plane_side.rs`. A rounded input wider than an `i128` is refused, since geom-num has
/// no correctly rounded conversion of a wider integer to `f64` yet.
#[test]
fn the_emitter_carries_rounded_inputs_into_the_code() {
    let module = String::from_utf8(generate_plane_side().unwrap()).unwrap();
    assert_eq!(
        module.as_bytes(),
        include_bytes!("../../geom-num/src/generated/plane_side.rs")
    );
    assert!(
        module.contains(
            "pub fn filter(nx: i128, ny: i128, nz: i128, d: i128, p: P3) -> Option<Sign> {"
        )
    );
    assert!(module.contains("    let nx = rounded_f64(nx, 83)?;"));
    assert!(module.contains("    let d = rounded_f64(d, 125)?;"));
    assert!(module.contains("    let nx = within(nx, 83)?;"));
    assert!(module.contains("    let m_n = largest_magnitude([nx, ny, nz]);"));
    let wide = PLANE_SIDE.replace("rounded 125 in d", "rounded 200 in d");
    let d = derive(&wide).unwrap();
    let e = predicate_module(&d, "predicate-plane-side", "PLANE_SIDE").unwrap_err();
    assert!(e.0.contains("at most 127 bits"), "{e:?}");
}
