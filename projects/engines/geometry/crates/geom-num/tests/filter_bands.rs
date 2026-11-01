//! Each 2D filter refuses exactly what its own bound does not cover (chapter 28.6: inputs "placed
//! just inside and just outside the f64 filter's error bound"), term by term of its scale.
//!
//! A filter answers when |value| passes ERROR · S, where S is the largest product of group maxima
//! the module's header lists. Each product of that list must count: a filter that lost one would
//! answer where the true bound says it may not know. So for each term this draws inputs where that
//! term is the largest, by a margin, moves one input until the exact value lies between the next
//! term's bound and that term's, and asks the filter. It must refuse; and the same inputs with the
//! value moved past the whole bound must be answered, with the exact sign.
//!
//! The scale here is read from the header's table, which no change to the filter's code touches, and
//! evaluated from the inputs' own group maxima, and the error constant from the module's `ERROR`.
//! The exact value is the module's `value`, which `geom-num`'s other tests hold to big integers.
//!
//! A term no input can make the largest by the margin is named in its [`Spec`], with why: there the
//! filter's decision is the same with it or without it.
//!
//! Seeds: the test prints `seed: <hex>` and runs from a fixed default; `SPAREKIT_SEED=<hex>` replays
//! another (chapter 28.17). Case counts are a twentieth in debug builds.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use geom_num::Sign;
use geom_num::generated::{carrier_order, carrier_side, rational_cmp, rational_orient};
use geom_num::wide::Wide;

/// The default seed (G187).
const DEFAULT_SEED: u64 = 0x6718_7000_5eed_b0b0;

/// A term must lead the next by this much for the two bounds to leave room between them.
const MARGIN: f64 = 1.25;

/// Inputs by name.
type Inputs = BTreeMap<&'static str, i128>;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), written out as the other tests do.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A signed integer whose top bit is `bits`, or 0 for no bits.
    fn with_bits(&mut self, bits: u32) -> i128 {
        if bits == 0 {
            return 0;
        }
        let raw = (u128::from(self.next()) << 64) | u128::from(self.next());
        let v = ((raw >> (128 - bits)) | (1 << (bits - 1))) as i128;
        if self.next() & 1 == 0 { v } else { -v }
    }
}

/// `v`, near enough, as a double.
fn real<const L: usize>(v: &Wide<L>) -> f64 {
    let negative = v.is_negative();
    let m = if negative {
        v.checked_neg().unwrap()
    } else {
        *v
    };
    let mut out = 0.0_f64;
    for limb in m.to_le_limbs().iter().rev() {
        out = out * 18_446_744_073_709_551_616.0 + *limb as f64;
    }
    if negative { -out } else { out }
}

/// A filter and how to reach it.
struct Spec {
    name: &'static str,
    module: &'static str,
    /// Each input, its group's name and its budget in bits, and the group its size is drawn with.
    inputs: &'static [(&'static str, &'static str, u32)],
    /// The inputs that may be moved to set the value, tried in turn.
    moved: &'static [&'static str],
    /// The exact value.
    value: fn(&Inputs) -> f64,
    /// The filter's answer.
    filter: fn(&Inputs) -> Option<Sign>,
    /// The scale variables of each group, read from the inputs.
    variables: fn(&Inputs, &mut BTreeMap<&'static str, f64>),
    /// Terms no input can make the largest by [`MARGIN`], with why.
    idle: &'static [(&'static str, &'static str)],
}

/// The module's `ERROR`.
fn error_of(module: &str) -> f64 {
    let line = module
        .lines()
        .find(|l| l.starts_with("const ERROR: f64"))
        .expect("the module has an error constant");
    let hex = line
        .split("0x")
        .nth(1)
        .and_then(|t| t.split(')').next())
        .expect("the constant is written in hexadecimal");
    f64::from_bits(u64::from_str_radix(&hex.replace('_', ""), 16).unwrap())
}

/// The result's scale from the module's table: each term as the group names it multiplies.
fn terms_of(module: &str) -> Vec<(String, Vec<String>)> {
    let result = module
        .lines()
        .find_map(|l| l.strip_prefix("//! | Result | `"))
        .and_then(|l| l.split('`').next())
        .expect("the module names its result");
    let row = format!("//! | `{result}` | ");
    let scale = module
        .lines()
        .filter(|l| l.starts_with(&row))
        .find_map(|l| {
            let cell = l[row.len()..].split(" | ").next()?;
            (!cell.starts_with('`')).then_some(cell)
        })
        .expect("the table gives the result's scale");
    let inner = scale
        .strip_prefix("max{")
        .and_then(|t| t.strip_suffix('}'))
        .unwrap_or(scale);
    inner
        .split(", ")
        .map(|term| {
            let mut groups = Vec::new();
            for factor in term.split('·') {
                let (group, power) = factor.split_once('^').unwrap_or((factor, "1"));
                for _ in 0..power.parse::<u32>().unwrap() {
                    groups.push(group.to_owned());
                }
            }
            (term.to_owned(), groups)
        })
        .collect()
}

/// The larger of two finite doubles.
fn larger(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// The largest |v| of each group.
fn maxima(spec: &Spec, inputs: &Inputs) -> BTreeMap<&'static str, f64> {
    let mut out: BTreeMap<&'static str, f64> = BTreeMap::new();
    for (name, group, _) in spec.inputs {
        let v = (inputs[name] as f64).abs();
        let m = out.entry(group).or_insert(0.0);
        *m = larger(*m, v);
    }
    (spec.variables)(inputs, &mut out);
    out
}

/// Each term's value.
fn evaluated(terms: &[(String, Vec<String>)], m: &BTreeMap<&'static str, f64>) -> Vec<f64> {
    terms
        .iter()
        .map(|(_, groups)| groups.iter().fold(1.0, |p, g| p * m[g.as_str()]))
        .collect()
}

/// `inputs` with `name` moved until the value is near `target`, by Newton's steps: `None` if it will
/// not come, or leaves its budget.
fn placed(
    spec: &Spec,
    inputs: &Inputs,
    name: &'static str,
    target: f64,
    near: f64,
) -> Option<Inputs> {
    let bits = spec.inputs.iter().find(|(n, ..)| *n == name)?.2;
    let top = (1_i128 << bits) - 1;
    let mut at = inputs.clone();
    for _ in 0..12 {
        let v = (spec.value)(&at);
        if (v - target).abs() <= near {
            return Some(at);
        }
        let here = at[name];
        let step = (here.abs() >> 24).max(1);
        let mut next = at.clone();
        next.insert(name, (here + step).clamp(-top, top));
        let slope = ((spec.value)(&next) - v) / (next[name] - here).max(1) as f64;
        if slope == 0.0 || !slope.is_finite() {
            return None;
        }
        let moved = here as f64 + (target - v) / slope;
        if moved.abs() >= top as f64 {
            return None;
        }
        at.insert(name, moved as i128);
    }
    None
}

/// Runs `spec`, and says what is wrong with each term not excused that was not tested, or with one
/// excused that was.
fn run(spec: &Spec, rng: &mut Rng, cases: u64) -> Vec<String> {
    let error = error_of(spec.module);
    let terms = terms_of(spec.module);
    let mut groups: Vec<(&str, u32)> = Vec::new();
    for (_, g, bits) in spec.inputs {
        match groups.iter_mut().find(|(n, _)| n == g) {
            Some(entry) => entry.1 = entry.1.max(*bits),
            None => groups.push((g, *bits)),
        }
    }
    let mut refused = vec![0_u64; terms.len()];
    let mut answered = vec![0_u64; terms.len()];
    let mut leaked = vec![0_u64; terms.len()];
    for _ in 0..cases {
        // Sizes drawn for each group, so that each term leads in some draws.
        let sizes: BTreeMap<&str, u32> = groups
            .iter()
            .map(|(g, bits)| (*g, 1 + (rng.next() % u64::from(*bits)) as u32))
            .collect();
        let mut inputs = Inputs::new();
        for (name, group, bits) in spec.inputs {
            let size = sizes[group].min(*bits);
            let v = if rng.next() % 3 == 0 {
                rng.with_bits(size)
            } else {
                let less = 1 + (rng.next() % u64::from(size)) as u32;
                rng.with_bits(less)
            };
            inputs.insert(name, v);
        }
        let which = (rng.next() % spec.moved.len() as u64) as usize;
        let name = spec.moved[which];
        let at = maxima(spec, &inputs);
        let values = evaluated(&terms, &at);
        let top = values.iter().copied().fold(0.0, larger);
        if top == 0.0 {
            continue;
        }
        let bound = error * top;
        let sign = if rng.next() & 1 == 0 { 1.0 } else { -1.0 };
        // First near the bound's middle: the terms may move, so they are read again after.
        let Some(first) = placed(spec, &inputs, name, sign * 0.8 * bound, 0.02 * bound) else {
            continue;
        };
        let values = evaluated(&terms, &maxima(spec, &first));
        let top = values.iter().copied().fold(0.0, larger);
        let leader = values.iter().position(|v| *v == top).unwrap();
        let next = values
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != leader)
            .map(|(_, v)| *v)
            .fold(0.0, larger);
        if top < MARGIN * next {
            continue;
        }
        let bound = error * top;
        let share = (0.5 + 0.5 * next / top).clamp(0.55, 0.9);
        let Some(inside) = placed(spec, &first, name, sign * share * bound, 0.02 * bound) else {
            continue;
        };
        let values = evaluated(&terms, &maxima(spec, &inside));
        let held = values.iter().copied().fold(0.0, larger);
        if values[leader] != held || held < MARGIN * next {
            continue;
        }
        match (spec.filter)(&inside) {
            None => refused[leader] += 1,
            Some(_) => leaked[leader] += 1,
        }
        // Past the whole bound, it answers, and rightly.
        if let Some(outside) = placed(
            spec,
            &first,
            name,
            sign * 2.5 * error * held,
            0.25 * error * held,
        ) {
            let v = (spec.value)(&outside);
            let expected = if v > 0.0 {
                Sign::Positive
            } else {
                Sign::Negative
            };
            if (spec.filter)(&outside) == Some(expected) {
                answered[leader] += 1;
            }
        }
    }
    let mut problems = Vec::new();
    for (k, (term, _)) in terms.iter().enumerate() {
        let idle = spec.idle.iter().any(|(t, _)| t == term);
        let name = spec.name;
        let (no, leak, past) = (refused[k], leaked[k], answered[k]);
        println!("{name} {term}: refused {no}, leaked {leak}, answered {past}, idle {idle}");
        if idle {
            if no > 0 {
                problems.push(format!("{name}: {term} is not idle"));
            }
            continue;
        }
        if no == 0 {
            problems.push(format!("{name}: no input tested {term}"));
        }
        // The bound covers the filter's whole error and the rounding of its inputs usually takes
        // only part of it, so some answers near it are right: the most that can.
        if leak > no {
            problems.push(format!("{name}: the filter answered inside {term}'s bound"));
        }
        if past == 0 {
            problems.push(format!("{name}: nothing answered past {term}"));
        }
    }
    problems
}

fn get<const N: usize>(inputs: &Inputs, names: [&str; N]) -> [i128; N] {
    names.map(|n| inputs[n])
}

const SPECS: &[Spec] = &[
    Spec {
        name: "carrier_side",
        module: include_str!("../src/generated/carrier_side.rs"),
        inputs: &[
            ("a", "n", 41),
            ("b", "n", 41),
            ("c", "d", 82),
            ("x", "p", 124),
            ("y", "p", 124),
            ("w", "w", 83),
        ],
        moved: &["c", "x", "y"],
        value: |i| {
            let [a, b, c, x, y, w] = get(i, ["a", "b", "c", "x", "y", "w"]);
            real(&carrier_side::value(a, b, c, x, y, w).unwrap())
        },
        filter: |i| {
            let [a, b, c, x, y, w] = get(i, ["a", "b", "c", "x", "y", "w"]);
            carrier_side::filter(a, b, c, x, y, w)
        },
        variables: |_, _| {},
        idle: &[],
    },
    Spec {
        name: "carrier_order",
        module: include_str!("../src/generated/carrier_order.rs"),
        inputs: &[
            ("a", "n", 41),
            ("b", "n", 41),
            ("xp", "p", 124),
            ("yp", "p", 124),
            ("xq", "p", 124),
            ("yq", "p", 124),
            ("wp", "w", 83),
            ("wq", "w", 83),
        ],
        moved: &["xq", "yq", "xp", "yp"],
        value: |i| {
            let [a, b, xp, yp, xq, yq, wp, wq] =
                get(i, ["a", "b", "xp", "yp", "xq", "yq", "wp", "wq"]);
            real(&carrier_order::value(a, b, xp, yp, xq, yq, wp, wq).unwrap())
        },
        filter: |i| {
            let [a, b, xp, yp, xq, yq, wp, wq] =
                get(i, ["a", "b", "xp", "yp", "xq", "yq", "wp", "wq"]);
            carrier_order::filter(a, b, xp, yp, xq, yq, wp, wq)
        },
        variables: |_, _| {},
        idle: &[],
    },
    Spec {
        name: "rational_cmp",
        module: include_str!("../src/generated/rational_cmp.rs"),
        inputs: &[
            ("p", "c", 124),
            ("q", "c", 124),
            ("wp", "w", 83),
            ("wq", "w", 83),
        ],
        moved: &["p", "q"],
        value: |i| {
            let [p, q, wp, wq] = get(i, ["p", "q", "wp", "wq"]);
            real(&rational_cmp::value(p, q, wp, wq).unwrap())
        },
        filter: |i| {
            let [p, q, wp, wq] = get(i, ["p", "q", "wp", "wq"]);
            rational_cmp::filter(p, q, wp, wq)
        },
        variables: |_, _| {},
        idle: &[],
    },
    Spec {
        name: "rational_orient",
        module: include_str!("../src/generated/rational_orient.rs"),
        inputs: &[
            ("xa", "p", 124),
            ("ya", "p", 124),
            ("xb", "p", 124),
            ("yb", "p", 124),
            ("xc", "p", 124),
            ("yc", "p", 124),
            ("wa", "w", 83),
            ("wb", "w", 83),
            ("wc", "w", 83),
        ],
        moved: &["xa", "ya", "xb", "yb", "xc", "yc"],
        value: |i| {
            let [xa, ya, xb, yb, xc, yc, wa, wb, wc] =
                get(i, ["xa", "ya", "xb", "yb", "xc", "yc", "wa", "wb", "wc"]);
            real(&rational_orient::value(xa, ya, xb, yb, xc, yc, wa, wb, wc).unwrap())
        },
        filter: |i| {
            let [xa, ya, xb, yb, xc, yc, wa, wb, wc] =
                get(i, ["xa", "ya", "xb", "yb", "xc", "yc", "wa", "wb", "wc"]);
            rational_orient::filter(xa, ya, xb, yb, xc, yc, wa, wb, wc)
        },
        variables: |_, _| {},
        idle: &[],
    },
];

#[test]
fn each_term_of_each_scale_counts() {
    let seed = std::env::var("SPAREKIT_SEED")
        .ok()
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(DEFAULT_SEED);
    println!("seed: {seed:x}");
    let cases = if cfg!(debug_assertions) {
        20_000
    } else {
        400_000
    };
    let mut rng = Rng(seed);
    let mut problems = Vec::new();
    for spec in SPECS {
        problems.extend(run(spec, &mut rng, cases));
    }
    assert_eq!(problems, Vec::<String>::new());
}
