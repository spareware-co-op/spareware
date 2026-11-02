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
//! A term no input can make the largest by the margin is named in its [`Spec`], with why. Such a
//! term is not what decides, but a filter whose scale grew past the true one would still refuse what
//! it should answer: so each filter is also asked with some groups at 0, 1 or 2, where a sum or a
//! quotient in place of a product shows, and must answer what lies past its bound.
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
    clippy::too_many_lines,
    reason = "the drawing, placing and checking of one case read best in one piece"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use geom_num::Sign;
use geom_num::generated::{
    carrier_order, carrier_side, circle_gap, circle_line, circle_side, ellipse_line, ellipse_side,
    rational_cmp, rational_orient, round_grid, round_line, round_rational,
};
use geom_num::grid::P2;
use geom_num::wide::Wide;

/// The default seed (G187).
const DEFAULT_SEED: u64 = 0x6718_7000_5eed_b0b0;

/// Why a mixed term never leads: `x · y` is at most the larger of `x²` and `y²`.
const MEAN: &str = "the geometric mean of two other terms, so never the largest of them";

/// Why `circle_gap`'s squares of differences never lead where it matters: with x² above t², the
/// value is at least 2x − 1, far past the bound.
const SQUARE: &str = "above t^2 it leaves the value past the bound whatever the scale";

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

/// Draws with some groups at 0, 1 or 2 and the rest of any size, with the value past the bound, or
/// as it falls. Past 2.1 bounds the filter cannot refuse: what it computes is within one bound of the
/// value, so a single refusal there is a scale grown past the true one, which shows only in some
/// draws and is the count returned first. As the value falls past 1.5 bounds rounding may refuse a
/// stray input: for each set of groups held small, how many it did not answer of how many it should.
fn with_small_groups(
    spec: &Spec,
    rng: &mut Rng,
    cases: u64,
    error: f64,
    terms: &[(String, Vec<String>)],
    groups: &[(&str, u32)],
) -> (u64, BTreeMap<u64, (u64, u64)>) {
    let mut sure = 0;
    let mut tally: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
    for _ in 0..cases {
        let sizes: BTreeMap<&str, Option<u32>> = groups
            .iter()
            .map(|(g, bits)| {
                // All its bits or a sixteenth of them: a term grown past the true scale shows
                // where the sizes are far apart.
                let size = if rng.next() & 1 == 0 {
                    *bits
                } else {
                    1 + *bits / 16
                };
                (*g, (rng.next() % 2 != 0).then_some(size))
            })
            .collect();
        let held = groups.iter().enumerate().fold(0, |set, (k, (g, _))| {
            if sizes[g].is_none() {
                set | (1_u64 << k)
            } else {
                set
            }
        });
        let (missed, past_bound) = tally.entry(held).or_default();
        let mut inputs = Inputs::new();
        for (name, group, bits) in spec.inputs {
            let v = match sizes[group] {
                Some(size) => rng.with_bits(size.min(*bits)),
                None => {
                    i128::from((rng.next() % 3) as u8) * if rng.next() & 1 == 0 { 1 } else { -1 }
                }
            };
            inputs.insert(name, v);
        }
        let top = |at: &Inputs| {
            evaluated(terms, &maxima(spec, at))
                .into_iter()
                .fold(0.0, larger)
        };
        let bound = error * top(&inputs);
        if bound == 0.0 {
            continue;
        }
        let v = (spec.value)(&inputs);
        // Past twice the bound nothing the filter's rounding does can stop it answering.
        if v.abs() > 2.1 * bound {
            *past_bound += 1;
            if (spec.filter)(&inputs).is_none() {
                sure += 1;
            }
        }
        let which = (rng.next() % spec.moved.len() as u64) as usize;
        let sign = if rng.next() & 1 == 0 { 1.0 } else { -1.0 };
        let Some(at) = placed(
            spec,
            &inputs,
            spec.moved[which],
            sign * 1.5 * bound,
            0.1 * bound,
        ) else {
            continue;
        };
        if ((top(&at) * error) / bound - 1.0).abs() < 0.01 {
            *past_bound += 1;
            if (spec.filter)(&at).is_none() {
                *missed += 1;
            }
        }
    }
    (sure, tally)
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
    let (sure, tally) = with_small_groups(spec, rng, 2 * cases, error, &terms, &groups);
    let mut problems = Vec::new();
    let past_bound = tally.values().fold(0, |all, (_, past)| all + past);
    println!("{}: {past_bound} past the bound, {sure} refused", spec.name);
    if past_bound == 0 || sure > 0 {
        problems.push(format!(
            "{}: {sure} of {past_bound} past the bound refused",
            spec.name
        ));
    }
    for (held, (missed, past)) in &tally {
        // Past the bound the filter's rounding refuses at most a stray input near 1.5 bounds.
        if *past >= 8 && *missed * 4 > *past {
            problems.push(format!(
                "{}: with the groups of {held:b} small, {missed} of {past} not answered",
                spec.name
            ));
        }
    }
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

/// The grid point named `x` and `y` among the inputs.
fn grid(inputs: &Inputs, x: &str, y: &str) -> P2 {
    P2::new(inputs[x] as i64, inputs[y] as i64).unwrap()
}

/// `v`, added to what `group` holds.
fn widen(m: &mut BTreeMap<&'static str, f64>, group: &'static str, v: i128) {
    let held = m.entry(group).or_insert(0.0);
    *held = larger(*held, (v as f64).abs());
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
    Spec {
        name: "round_grid",
        module: include_str!("../src/generated/round_grid.rs"),
        inputs: &[
            ("z_x", "x", 40),
            ("z_y", "y", 40),
            ("p_x", "x", 40),
            ("p_y", "y", 40),
            ("cx", "c", 124),
            ("cy", "c", 124),
            ("w", "w", 83),
        ],
        moved: &["cx", "cy", "z_x", "z_y"],
        value: |i| {
            let [cx, cy, w] = get(i, ["cx", "cy", "w"]);
            let (z, p) = (grid(i, "z_x", "z_y"), grid(i, "p_x", "p_y"));
            real(&round_grid::value(z, p, cx, cy, w).unwrap())
        },
        filter: |i| {
            let [cx, cy, w] = get(i, ["cx", "cy", "w"]);
            let (z, p) = (grid(i, "z_x", "z_y"), grid(i, "p_x", "p_y"));
            round_grid::filter(z, p, cx, cy, w)
        },
        variables: |i, m| {
            widen(m, "x", i["z_x"] - i["p_x"]);
            widen(m, "y", i["z_y"] - i["p_y"]);
        },
        idle: &[],
    },
    Spec {
        name: "round_rational",
        module: include_str!("../src/generated/round_rational.rs"),
        inputs: &[
            ("x", "p", 124),
            ("y", "p", 124),
            ("v", "v", 83),
            ("q_x", "x", 40),
            ("q_y", "y", 40),
            ("cx", "c", 124),
            ("cy", "c", 124),
            ("w", "w", 83),
        ],
        moved: &["cx", "cy", "x", "y"],
        value: |i| {
            let [x, y, v, cx, cy, w] = get(i, ["x", "y", "v", "cx", "cy", "w"]);
            let q = grid(i, "q_x", "q_y");
            real(&round_rational::value(x, y, v, q, cx, cy, w).unwrap())
        },
        filter: |i| {
            let [x, y, v, cx, cy, w] = get(i, ["x", "y", "v", "cx", "cy", "w"]);
            let q = grid(i, "q_x", "q_y");
            round_rational::filter(x, y, v, q, cx, cy, w)
        },
        variables: |_, _| {},
        idle: &[("y·p·v·w", MEAN), ("x·p·v·w", MEAN)],
    },
    Spec {
        name: "round_line",
        module: include_str!("../src/generated/round_line.rs"),
        inputs: &[
            ("a", "n", 41),
            ("b", "n", 41),
            ("d", "d", 82),
            ("p_x", "x", 40),
            ("p_y", "y", 40),
            ("cx", "c", 124),
            ("cy", "c", 124),
            ("w", "w", 83),
        ],
        moved: &["d", "cx", "cy"],
        value: |i| {
            let [a, b, d, cx, cy, w] = get(i, ["a", "b", "d", "cx", "cy", "w"]);
            let p = grid(i, "p_x", "p_y");
            real(&round_line::value(a, b, d, p, cx, cy, w).unwrap())
        },
        filter: |i| {
            let [a, b, d, cx, cy, w] = get(i, ["a", "b", "d", "cx", "cy", "w"]);
            let p = grid(i, "p_x", "p_y");
            round_line::filter(a, b, d, p, cx, cy, w)
        },
        variables: |_, _| {},
        idle: &[("n·d·c·w", MEAN), ("y·n^2·c·w", MEAN), ("x·n^2·c·w", MEAN)],
    },
    Spec {
        name: "circle_side",
        module: include_str!("../src/generated/circle_side.rs"),
        inputs: &[
            ("x", "p", 124),
            ("y", "p", 124),
            ("v", "v", 83),
            ("k_x", "x", 40),
            ("k_y", "y", 40),
            ("r", "r", 41),
        ],
        moved: &["x", "y", "r"],
        value: |i| {
            let [x, y, v, r] = get(i, ["x", "y", "v", "r"]);
            real(&circle_side::value(x, y, v, grid(i, "k_x", "k_y"), r).unwrap())
        },
        filter: |i| {
            let [x, y, v, r] = get(i, ["x", "y", "v", "r"]);
            circle_side::filter(x, y, v, grid(i, "k_x", "k_y"), r)
        },
        variables: |_, _| {},
        idle: &[("y·p·v", MEAN), ("x·p·v", MEAN)],
    },
    Spec {
        name: "circle_line",
        module: include_str!("../src/generated/circle_line.rs"),
        inputs: &[
            ("a", "n", 41),
            ("b", "n", 41),
            ("d", "d", 82),
            ("k_x", "x", 40),
            ("k_y", "y", 40),
            ("r", "r", 41),
        ],
        moved: &["d", "r"],
        value: |i| {
            let [a, b, d, r] = get(i, ["a", "b", "d", "r"]);
            real(&circle_line::value(a, b, d, grid(i, "k_x", "k_y"), r).unwrap())
        },
        filter: |i| {
            let [a, b, d, r] = get(i, ["a", "b", "d", "r"]);
            circle_line::filter(a, b, d, grid(i, "k_x", "k_y"), r)
        },
        variables: |_, _| {},
        idle: &[("y·n·d", MEAN), ("x·n·d", MEAN), ("x·y·n^2", MEAN)],
    },
    Spec {
        name: "circle_gap",
        module: include_str!("../src/generated/circle_gap.rs"),
        inputs: &[
            ("a_x", "ax", 40),
            ("a_y", "ay", 40),
            ("b_x", "ax", 40),
            ("b_y", "ay", 40),
            ("t", "t", 42),
        ],
        moved: &["t", "a_x", "a_y"],
        value: |i| {
            let (a, b) = (grid(i, "a_x", "a_y"), grid(i, "b_x", "b_y"));
            circle_gap::value(a, b, i["t"]).unwrap() as f64
        },
        filter: |i| {
            let (a, b) = (grid(i, "a_x", "a_y"), grid(i, "b_x", "b_y"));
            circle_gap::filter(a, b, i["t"])
        },
        variables: |i, m| {
            widen(m, "x", i["a_x"] - i["b_x"]);
            widen(m, "y", i["a_y"] - i["b_y"]);
        },
        idle: &[
            ("t^2", "circle_gap_answers_only_past_t_squared"),
            ("y^2", SQUARE),
            ("x^2", SQUARE),
        ],
    },
    Spec {
        name: "ellipse_side",
        module: include_str!("../src/generated/ellipse_side.rs"),
        inputs: &[
            ("x", "a", 124),
            ("y", "a", 124),
            ("w", "w", 83),
            ("k_x", "x", 40),
            ("k_y", "y", 40),
            ("mx", "m", 41),
            ("my", "m", 41),
            ("p", "r", 22),
            ("q", "r", 22),
        ],
        moved: &["x", "y"],
        value: |i| {
            let [x, y, w, mx, my, p, q] = get(i, ["x", "y", "w", "mx", "my", "p", "q"]);
            real(&ellipse_side::value(x, y, w, grid(i, "k_x", "k_y"), mx, my, p, q).unwrap())
        },
        filter: |i| {
            let [x, y, w, mx, my, p, q] = get(i, ["x", "y", "w", "mx", "my", "p", "q"]);
            ellipse_side::filter(x, y, w, grid(i, "k_x", "k_y"), mx, my, p, q)
        },
        variables: |_, _| {},
        idle: &[
            ("y·a·w·m^2·r^2", MEAN),
            ("x·a·w·m^2·r^2", MEAN),
            ("x·y·w^2·m^2·r^2", MEAN),
        ],
    },
    Spec {
        name: "ellipse_line",
        module: include_str!("../src/generated/ellipse_line.rs"),
        inputs: &[
            ("a", "n", 41),
            ("b", "n", 41),
            ("d", "d", 82),
            ("k_x", "x", 40),
            ("k_y", "y", 40),
            ("mx", "m", 41),
            ("my", "m", 41),
            ("p", "r", 22),
            ("q", "r", 22),
        ],
        moved: &["d"],
        value: |i| {
            let [a, b, d, mx, my, p, q] = get(i, ["a", "b", "d", "mx", "my", "p", "q"]);
            real(&ellipse_line::value(a, b, d, grid(i, "k_x", "k_y"), mx, my, p, q).unwrap())
        },
        filter: |i| {
            let [a, b, d, mx, my, p, q] = get(i, ["a", "b", "d", "mx", "my", "p", "q"]);
            ellipse_line::filter(a, b, d, grid(i, "k_x", "k_y"), mx, my, p, q)
        },
        variables: |_, _| {},
        idle: &[
            ("y·n·d·r^2", MEAN),
            ("x·n·d·r^2", MEAN),
            ("x·y·n^2·r^2", MEAN),
        ],
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

/// `circle_gap`'s value moves by 2t for a step of t, far more than its bound, so no input can be
/// moved into the band: a right triangle's sides, nearly, are drawn until one falls in it. With
/// x = y neither x² nor y² is more than half of t², so the bound is t²'s and nothing else's.
#[test]
fn circle_gap_answers_only_past_t_squared() {
    let module = include_str!("../src/generated/circle_gap.rs");
    let error = error_of(module);
    let mut rng = Rng(DEFAULT_SEED);
    let (mut refused, mut leaked, mut answered) = (0, 0, 0);
    let origin = P2::new(0, 0).unwrap();
    for _ in 0..if cfg!(debug_assertions) {
        2_000_000
    } else {
        20_000_000
    } {
        let d = (1 << 39) + (rng.next() >> 26) as i64 % (1 << 38);
        let t = (d as f64 * core::f64::consts::SQRT_2).round() as i128;
        let square = t * t;
        let v = 2 * i128::from(d) * i128::from(d) - square;
        let share = v.abs() as f64 / (error * square as f64);
        if !(0.6..0.9).contains(&share) && !(1.5..3.0).contains(&share) {
            continue;
        }
        let far = P2::new(d, d).unwrap();
        let answer = circle_gap::filter(far, origin, t);
        if share < 1.0 {
            // Rounding takes up part of the bound, so a right answer may come early.
            if answer.is_none() {
                refused += 1;
            } else {
                leaked += 1;
            }
        } else if share > 2.1 {
            let sign = if v > 0 {
                Sign::Positive
            } else {
                Sign::Negative
            };
            assert_eq!(answer, Some(sign), "{d} {t}");
            answered += 1;
        }
    }
    println!("circle_gap: refused {refused}, leaked {leaked}, answered {answered}");
    assert!(
        refused > leaked && answered > 0,
        "{refused} {leaked} {answered}"
    );
}
