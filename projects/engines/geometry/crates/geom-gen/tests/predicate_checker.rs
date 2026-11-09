//! The independent checker of the generated predicates: a second implementation of the
//! description language, the bit-range certificate, the error analysis and the constant's
//! rounding, written separately from geom-gen's `predicate_lang`, `predicate_derive` and
//! `bignum`, with its own big integers (32-bit limbs) and its own polynomials in u. It reads each
//! committed module in `geom-num/src/generated/` and must agree with every number and every line
//! of code:
//!
//! - the description in the header is the registered description;
//! - every certificate row (step, operation, bound, bits, exact-path type) and every
//!   error-analysis row (scale, exactness, unit bound, unit error);
//! - the summary (result, unit error, scale, scale at run time, roundings, constant exactly and
//!   as an `f64`) and the `ERROR` constant in the code;
//! - the code itself, line for line: every line outside comments and attributes is the line the
//!   checker writes for it from its own derivation (the imports, the constant, the three
//!   signatures with their parameters in the description's order and their types, each `let` of
//!   `filter` and of `value`, each group maximum, the call to `decide`, the result, and `exact`'s
//!   body, each laid out as rustfmt lays it out: a signature or a call too long for one line over
//!   several), except the run-time scale, which it reads and evaluates: it must compute exactly the
//!   result's scale, with the roundings the constant counts (a scale computed over several lines,
//!   one term per line, is read as the one expression its lines compute together);
//! - the plain-`i128` variant `plain.rs` (piece G42), line for line outside comments and
//!   attributes: for each registered predicate, in their order, `<predicate>_value` with the same
//!   steps, each held in `i128` or in its certificate's wide type, both operands widened to it and
//!   the general method, so every type it uses is at least as wide as the certificate's;
//! - the width-aware limb variant `limbs.rs` (piece G42), line for line outside comments and
//!   attributes: each step in `i64` where the certificate gives `i64`, otherwise in as many 32-bit
//!   limbs as its operands' certified bits need, by its own rule; the result converted back to the
//!   shipped type; and one bounded proof for each kind of limb step, running the very expression the
//!   code writes for it on operands of every value the certified bits allow;
//! - the parent module `mod.rs`, line for line outside comments and attributes: one `pub mod` per
//!   registered predicate and per variant, and the helpers the proof relies on, as the checker writes them: `decide`
//!   with strict comparisons and the zero-scale case, `largest_magnitude` taking absolute values,
//!   `larger` by comparison, the exact conversion of a coordinate, and the range check and
//!   correctly rounded conversion of a rounded input.
//!
//! It names every value by what it computes, never by its name: a coordinate's load is reused only
//! for the same coordinate, and two values of one name are reported, so a description whose `let`
//! takes a coordinate's name cannot pass. It never panics on a module it cannot read; it reports.
//!
//! It also agrees with the generator's in-memory derivation, refuses a committed module or parent
//! module with a hand-edited constant, bound, bits, unit error, code line, helper or parameter
//! order, and the binary's `check` fails on such a module in a scratch copy of the workspace. The
//! committed files it reads are exactly the registered predicates' (a test ties the list here to
//! `PREDICATES` and, natively, to the generated folder's contents).
//!
//! The checker follows the written method in `predicate_derive.rs` and `predicate_lang.rs`
//! (Meyer and Pion's forward error analysis at unit bounds with group maxima; the certificate's
//! sum and product rules), not their code.

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
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::many_single_char_names
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use geom_gen::predicates::{PREDICATES, Predicate, derivation};

// ---------------------------------------------------------------------------------------------
// Big naturals, base 2^32.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct Big(Vec<u32>);

impl Big {
    fn small(v: u64) -> Big {
        Big(vec![v as u32, (v >> 32) as u32]).trim()
    }

    fn trim(mut self) -> Big {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
        self
    }

    fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    fn add(&self, o: &Big) -> Big {
        let n = self.0.len().max(o.0.len());
        let mut out = Vec::with_capacity(n + 1);
        let mut carry = 0u64;
        for i in 0..n {
            let s = u64::from(*self.0.get(i).unwrap_or(&0))
                + u64::from(*o.0.get(i).unwrap_or(&0))
                + carry;
            out.push(s as u32);
            carry = s >> 32;
        }
        out.push(carry as u32);
        Big(out).trim()
    }

    fn mul(&self, o: &Big) -> Big {
        let mut out = vec![0u64; self.0.len() + o.0.len() + 1];
        for (i, &x) in self.0.iter().enumerate() {
            for (j, &y) in o.0.iter().enumerate() {
                out[i + j] += u64::from(x) * u64::from(y);
                // Normalise as we go so no slot passes 2^64.
                let mut k = i + j;
                while out[k] >> 32 != 0 {
                    let c = out[k] >> 32;
                    out[k] &= 0xffff_ffff;
                    out[k + 1] += c;
                    k += 1;
                }
            }
        }
        Big(out.into_iter().map(|v| v as u32).collect()).trim()
    }

    fn pow2(n: u32) -> Big {
        let mut v = vec![0u32; (n / 32) as usize];
        v.push(1 << (n % 32));
        Big(v)
    }

    fn bits(&self) -> u32 {
        match self.0.last() {
            None => 0,
            Some(&top) => (self.0.len() as u32 - 1) * 32 + (32 - top.leading_zeros()),
        }
    }

    fn le(&self, o: &Big) -> bool {
        if self.0.len() != o.0.len() {
            return self.0.len() < o.0.len();
        }
        for (a, b) in self.0.iter().rev().zip(o.0.iter().rev()) {
            if a != b {
                return a < b;
            }
        }
        true
    }

    /// Bit i.
    fn bit(&self, i: u32) -> bool {
        self.0
            .get((i / 32) as usize)
            .is_some_and(|w| (w >> (i % 32)) & 1 == 1)
    }

    /// The decimal digits, by repeated division by ten.
    fn decimal(&self) -> String {
        if self.is_zero() {
            return "0".to_owned();
        }
        let mut digits = Vec::new();
        let mut n = self.0.clone();
        while !n.is_empty() {
            let mut rem = 0u64;
            for w in n.iter_mut().rev() {
                let cur = (rem << 32) | u64::from(*w);
                *w = (cur / 10) as u32;
                rem = cur % 10;
            }
            digits.push(b'0' + rem as u8);
            while n.last() == Some(&0) {
                n.pop();
            }
        }
        digits.reverse();
        String::from_utf8(digits).unwrap()
    }
}

// ---------------------------------------------------------------------------------------------
// Polynomials in u = 2^-53 with whole coefficients.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct Poly(Vec<Big>);

impl Poly {
    fn of(c: u64) -> Poly {
        Poly(vec![Big::small(c)]).trim()
    }

    fn u() -> Poly {
        Poly(vec![Big::small(0), Big::small(1)])
    }

    fn trim(mut self) -> Poly {
        while self.0.last().is_some_and(Big::is_zero) {
            self.0.pop();
        }
        self
    }

    fn add(&self, o: &Poly) -> Poly {
        let n = self.0.len().max(o.0.len());
        let zero = Big::small(0);
        Poly(
            (0..n)
                .map(|i| {
                    self.0
                        .get(i)
                        .unwrap_or(&zero)
                        .add(o.0.get(i).unwrap_or(&zero))
                })
                .collect(),
        )
        .trim()
    }

    fn mul(&self, o: &Poly) -> Poly {
        if self.0.is_empty() || o.0.is_empty() {
            return Poly(Vec::new());
        }
        let mut out = vec![Big::small(0); self.0.len() + o.0.len()];
        for (i, x) in self.0.iter().enumerate() {
            for (j, y) in o.0.iter().enumerate() {
                out[i + j] = out[i + j].add(&x.mul(y));
            }
        }
        Poly(out).trim()
    }

    /// As the generated headers write it: `4u + 2u^2`, `1 + u`, `0`.
    fn text(&self) -> String {
        let mut parts = Vec::new();
        for (i, c) in self.0.iter().enumerate() {
            if c.is_zero() {
                continue;
            }
            let c = c.decimal();
            parts.push(match (i, c.as_str()) {
                (0, _) => c,
                (1, "1") => "u".to_owned(),
                (1, _) => format!("{c}u"),
                (_, "1") => format!("u^{i}"),
                _ => format!("{c}u^{i}"),
            });
        }
        if parts.is_empty() {
            "0".to_owned()
        } else {
            parts.join(" + ")
        }
    }

    /// The bits of the smallest normal f64 at least the value at u = 2^-53.
    fn f64_at_least(&self) -> u64 {
        let d = (self.0.len() - 1) as u32;
        // value = N / 2^(53d) with N = Σ c_i 2^(53(d − i)).
        let mut n = Big::small(0);
        for (i, c) in self.0.iter().enumerate() {
            n = n.add(&c.mul(&Big::pow2(53 * (d - i as u32))));
        }
        let len = n.bits();
        // The leading bit is 2^(len − 1 − 53d); the significand is the top 53 bits, rounded up.
        let mut top = 0u64;
        for k in 0..53u32 {
            let i = i64::from(len) - 1 - i64::from(k);
            top = (top << 1) | u64::from(i >= 0 && n.bit(i as u32));
        }
        let rest = (0..len.saturating_sub(53)).any(|i| n.bit(i));
        let mut exponent = i64::from(len) - 1 - 53 * i64::from(d);
        if rest {
            top += 1;
            if top == 1 << 53 {
                top = 1 << 52;
                exponent += 1;
            }
        }
        let biased = (exponent + 1023) as u64;
        assert!((1..2047).contains(&biased));
        (biased << 52) | (top - (1 << 52))
    }
}

// ---------------------------------------------------------------------------------------------
// The language, read again: precedence climbing over tokens.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Coord(String, char),
    Name(String),
    Op(char, Box<Node>, Box<Node>),
}

#[derive(Clone, Debug)]
enum Kind {
    /// A grid point with this many coordinates.
    Point(u8),
    /// An integer of at most this many bits, in this group, passed exactly.
    Rounded(u32, String),
}

struct Desc {
    inputs: Vec<(String, Kind)>,
    lets: Vec<(String, Node)>,
    result: String,
}

fn lex(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            cur.push(ch);
        } else {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            if !ch.is_whitespace() {
                out.push(ch.to_string());
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Precedence climbing: `+` and `-` bind at 1, `*` at 2, all left-associative.
fn climb(toks: &[String], at: &mut usize, min: u8) -> Result<Node, String> {
    let mut left = atom(toks, at)?;
    while let Some(op) = toks.get(*at) {
        let prec = match op.as_str() {
            "+" | "-" => 1,
            "*" => 2,
            _ => break,
        };
        if prec < min {
            break;
        }
        let op = op.chars().next().ok_or("an empty token")?;
        *at += 1;
        let right = climb(toks, at, prec + 1)?;
        left = Node::Op(op, Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn atom(toks: &[String], at: &mut usize) -> Result<Node, String> {
    let t = toks.get(*at).ok_or("an expression ends early")?.clone();
    *at += 1;
    if t == "(" {
        let inner = climb(toks, at, 1)?;
        if toks.get(*at).map(String::as_str) != Some(")") {
            return Err("an unclosed `(`".to_owned());
        }
        *at += 1;
        return Ok(inner);
    }
    if toks.get(*at).map(String::as_str) == Some(".") {
        let axis = toks
            .get(*at + 1)
            .and_then(|a| a.chars().next())
            .ok_or("an axis is missing")?;
        *at += 2;
        return Ok(Node::Coord(t, axis));
    }
    Ok(Node::Name(t))
}

fn read(text: &str) -> Result<Desc, String> {
    let mut d = Desc {
        inputs: Vec::new(),
        lets: Vec::new(),
        result: String::new(),
    };
    for raw in text.lines() {
        let l = raw.split('#').next().unwrap_or_default().trim();
        if let Some(rest) = l.strip_prefix("input ") {
            let (names, kind) = rest.split_once(':').ok_or("an input without a kind")?;
            let words: Vec<&str> = kind.split_whitespace().collect();
            let kind = match words.as_slice() {
                ["point2"] => Kind::Point(2),
                ["point3"] => Kind::Point(3),
                ["rounded", b, "in", g] => {
                    Kind::Rounded(b.parse().map_err(|_| "bits")?, (*g).to_owned())
                }
                _ => return Err(format!("input kind {kind}")),
            };
            for n in names.split_whitespace() {
                d.inputs.push((n.to_owned(), kind.clone()));
            }
        } else if let Some(rest) = l.strip_prefix("let ") {
            let (name, expr) = rest.split_once('=').ok_or("a let without `=`")?;
            let toks = lex(expr);
            let mut at = 0;
            let node = climb(&toks, &mut at, 1)?;
            if at != toks.len() {
                return Err(format!("text after the expression of `{}`", name.trim()));
            }
            d.lets.push((name.trim().to_owned(), node));
        } else if let Some(rest) = l.strip_prefix("sign ") {
            rest.trim().clone_into(&mut d.result);
        }
    }
    Ok(d)
}

// ---------------------------------------------------------------------------------------------
// The derivation, again.
// ---------------------------------------------------------------------------------------------

/// What the checker derives for one step.
#[derive(Clone, Debug)]
struct Row {
    name: String,
    /// The certificate's "computes" text.
    computes: String,
    /// The filter's statement and the exact path's.
    filter: String,
    exact: String,
    /// The exact path's statement in the plain-`i128` variant (piece G42).
    plain: String,
    /// An operation's operator and operand rows, for the limb variant (piece G42).
    operands: Option<(char, usize, usize)>,
    bound: Big,
    bits: u32,
    width: &'static str,
    /// Scale variable of this group, if one.
    group: Option<String>,
    /// A coordinate's load or a translation: the filter calls `exact_f64`.
    coordinates: bool,
    /// A rounded input's load: the code calls `rounded_f64` and `within`.
    rounded: bool,
    scale: BTreeSet<Vec<u32>>,
    exact_f64: bool,
    b: Poly,
    e: Poly,
}

/// The largest grid coordinate, read from geom-num.
fn m() -> Big {
    Big::small(geom_num::grid::COORD_MAX as u64)
}

/// The type for a step of `bits` bits: the narrowest whose magnitude limit 2^(64·limbs − 1) − 1
/// holds it, with the limb counts read from geom-num's types (`i64` one limb, `i128` two).
fn width(bits: u32) -> &'static str {
    let limbs = [
        ("i64", 1),
        ("i128", 2),
        ("I256", geom_num::I256::ZERO.to_le_limbs().len()),
        ("I320", geom_num::I320::ZERO.to_le_limbs().len()),
        ("I384", geom_num::I384::ZERO.to_le_limbs().len()),
        ("I512", geom_num::I512::ZERO.to_le_limbs().len()),
    ];
    limbs
        .iter()
        .find(|(_, l)| bits < 64 * *l as u32)
        .map_or("too wide", |(n, _)| *n)
}

fn limbs_of(width: &str) -> usize {
    match width {
        "i64" => 1,
        "I256" => 4,
        "I320" => 5,
        "I384" => 6,
        "I512" => 8,
        _ => 2,
    }
}

/// Whether a width is one of the `Wide` types (operands by reference).
fn is_wide(width: &str) -> bool {
    width.starts_with('I')
}

/// The type the plain-`i128` variant (piece G42) holds a step of width `w` in: `i128` for an
/// `i64` step, the same type otherwise.
fn held(w: &'static str) -> &'static str {
    if w == "i64" { "i128" } else { w }
}

/// `name`, held in `from`, as a `to`.
fn conv(name: &str, from: &str, to: &str) -> String {
    if from == to {
        name.to_owned()
    } else if from == "i64" && to == "i128" {
        format!("i128::from({name})")
    } else if from == "i64" {
        format!("{to}::from_i64({name})")
    } else if from == "i128" {
        format!("{to}::from_i128({name})")
    } else {
        format!("{name}.resize::<{}>()?", limbs_of(to))
    }
}

/// A grid coordinate in the exact path (an `i64`), as a `w`.
fn coord_exact(p: &str, a: char, w: &str) -> String {
    conv(&format!("{p}.{a}.get()"), "i64", w)
}

/// `l.method(r)?` in `w`.
fn checked(l: &str, op: char, r: &str, w: &str) -> String {
    let method = match op {
        '+' => "checked_add",
        '-' => "checked_sub",
        _ => "checked_mul",
    };
    let amp = if is_wide(w) { "&" } else { "" };
    format!("{l}.{method}({amp}{r})?")
}

struct Derived {
    rows: Vec<Row>,
    groups: Vec<String>,
    result: usize,
}

struct Walker<'d> {
    d: &'d Desc,
    rows: Vec<Row>,
    lets: BTreeMap<String, usize>,
    /// Each load made so far, by what it loads (`a.x`, or a rounded input's name): reused only
    /// for that same value, never by name.
    loads: BTreeMap<String, usize>,
    groups: Vec<String>,
}

impl Walker<'_> {
    fn unit(&self, g: &str) -> BTreeSet<Vec<u32>> {
        let mut m = vec![0u32; self.groups.len()];
        if let Some(i) = self.groups.iter().position(|x| x == g) {
            m[i] = 1;
        }
        BTreeSet::from([m])
    }

    fn leaf(&mut self, key: String, row: Row) -> usize {
        if let Some(&i) = self.loads.get(&key) {
            return i;
        }
        self.rows.push(row);
        let i = self.rows.len() - 1;
        self.loads.insert(key, i);
        i
    }

    fn visit(
        &mut self,
        node: &Node,
        owner: &str,
        k: &mut usize,
        top: bool,
    ) -> Result<usize, String> {
        let name_for = |k: &mut usize| {
            if top {
                owner.to_owned()
            } else {
                *k += 1;
                format!("{owner}_{k}")
            }
        };
        match node {
            Node::Coord(p, a) => {
                let bound = m();
                let bits = bound.bits();
                let w = width(bits);
                let row = Row {
                    name: format!("{p}_{a}"),
                    computes: format!("{p}.{a}"),
                    filter: format!("exact_f64({p}.{a})"),
                    exact: coord_exact(p, *a, w),
                    plain: coord_exact(p, *a, held(w)),
                    operands: None,
                    bits,
                    width: w,
                    scale: self.unit(&a.to_string()),
                    group: Some(a.to_string()),
                    coordinates: true,
                    rounded: false,
                    exact_f64: bound.le(&Big::pow2(53)),
                    b: Poly::of(1),
                    e: Poly::of(0),
                    bound,
                };
                Ok(self.leaf(format!("{p}.{a}"), row))
            }
            Node::Name(n) => {
                if let Some(&i) = self.lets.get(n) {
                    return Ok(i);
                }
                let Some((_, Kind::Rounded(bits, g))) = self.d.inputs.iter().find(|(x, _)| x == n)
                else {
                    return Err(format!(
                        "`{n}` is neither an earlier let nor a rounded input"
                    ));
                };
                // 2^bits − 1: every bit below set.
                let mut bound = Big::small(0);
                for i in 0..*bits {
                    bound = bound.add(&Big::pow2(i));
                }
                let w = width(*bits);
                let exact = match w {
                    "i128" => format!("within({n}, {bits})?"),
                    "i64" => format!("i64::try_from(within({n}, {bits})?).ok()?"),
                    other => return Err(format!("a rounded input in {other}")),
                };
                let row = Row {
                    name: n.clone(),
                    computes: n.clone(),
                    filter: format!("rounded_f64({n}, {bits})?"),
                    exact,
                    plain: format!("within({n}, {bits})?"),
                    operands: None,
                    bits: *bits,
                    width: w,
                    scale: self.unit(g),
                    group: Some(g.clone()),
                    coordinates: false,
                    rounded: true,
                    // Rounded to nearest: never counted exact, whatever its size.
                    exact_f64: false,
                    b: Poly::of(1),
                    e: Poly::u(),
                    bound,
                };
                Ok(self.leaf(format!("rounded {n}"), row))
            }
            Node::Op(op, l, r) => {
                if let (Node::Coord(p, pa), Node::Coord(q, qa), '-') = (&**l, &**r, *op) {
                    if pa == qa {
                        let name = name_for(k);
                        let bound = m().add(&m());
                        let bits = bound.bits();
                        let w = width(bits);
                        let row = Row {
                            computes: format!("{p}.{pa} - {q}.{qa}"),
                            filter: format!("exact_f64({p}.{pa}) - exact_f64({q}.{qa})"),
                            exact: checked(
                                &coord_exact(p, *pa, w),
                                '-',
                                &coord_exact(q, *qa, w),
                                w,
                            ),
                            plain: checked(
                                &coord_exact(p, *pa, held(w)),
                                '-',
                                &coord_exact(q, *qa, held(w)),
                                held(w),
                            ),
                            operands: None,
                            bits,
                            width: w,
                            scale: self.unit(&pa.to_string()),
                            group: Some(pa.to_string()),
                            coordinates: true,
                            rounded: false,
                            exact_f64: bound.le(&Big::pow2(53)),
                            b: Poly::of(1),
                            e: Poly::of(0),
                            bound,
                            name,
                        };
                        self.rows.push(row);
                        return Ok(self.rows.len() - 1);
                    }
                }
                let li = self.visit(l, owner, k, false)?;
                let ri = self.visit(r, owner, k, false)?;
                let name = if top {
                    owner.to_owned()
                } else {
                    *k += 1;
                    format!("{owner}_{k}")
                };
                let (a, b) = (self.rows[li].clone(), self.rows[ri].clone());
                let (bound, scale, p) = if *op == '*' {
                    let mut s = BTreeSet::new();
                    for x in &a.scale {
                        for y in &b.scale {
                            s.insert(x.iter().zip(y).map(|(p, q)| p + q).collect::<Vec<u32>>());
                        }
                    }
                    (a.bound.mul(&b.bound), s, a.b.mul(&b.b))
                } else {
                    let mut s = a.scale.clone();
                    s.extend(b.scale.iter().cloned());
                    (a.bound.add(&b.bound), s, a.b.add(&b.b))
                };
                let exact_f64 = a.exact_f64 && b.exact_f64 && bound.le(&Big::pow2(53));
                let (bb, e) = if exact_f64 {
                    (p, Poly::of(0))
                } else {
                    let carried = if *op == '*' {
                        a.e.mul(&b.b).add(&a.b.mul(&b.e)).add(&a.e.mul(&b.e))
                    } else {
                        a.e.add(&b.e)
                    };
                    let rounding = p.mul(&Poly::u());
                    (p.mul(&Poly::of(1).add(&Poly::u())), carried.add(&rounding))
                };
                let bits = bound.bits();
                let w = width(bits);
                // A product in a wide type is width-aware: `product_i128` when both operands fit
                // an `i128`, `checked_mul_i128` on the wide one when one does.
                let fits = |x: &str| !is_wide(x);
                let as_i128 = |name: &str, from: &str| conv(name, from, "i128");
                let exact = if *op == '*' && is_wide(w) && fits(a.width) && fits(b.width) {
                    format!(
                        "{w}::product_i128({}, {})?",
                        as_i128(&a.name, a.width),
                        as_i128(&b.name, b.width)
                    )
                } else if *op == '*' && is_wide(w) && (fits(a.width) || fits(b.width)) {
                    let (narrow, wide) = if fits(a.width) { (&a, &b) } else { (&b, &a) };
                    format!(
                        "{}.checked_mul_i128({})?",
                        conv(&wide.name, wide.width, w),
                        as_i128(&narrow.name, narrow.width)
                    )
                } else {
                    checked(
                        &conv(&a.name, a.width, w),
                        *op,
                        &conv(&b.name, b.width, w),
                        w,
                    )
                };
                // The plain-`i128` variant (piece G42): the step held in `i128` or wider, both
                // operands widened to it, and the general method.
                let pw = held(w);
                let plain = checked(
                    &conv(&a.name, held(a.width), pw),
                    *op,
                    &conv(&b.name, held(b.width), pw),
                    pw,
                );
                let row = Row {
                    computes: format!("{} {op} {}", a.name, b.name),
                    filter: format!("{} {op} {}", a.name, b.name),
                    exact,
                    plain,
                    operands: Some((*op, li, ri)),
                    bound,
                    bits,
                    width: w,
                    group: None,
                    coordinates: false,
                    rounded: false,
                    scale,
                    exact_f64,
                    b: bb,
                    e,
                    name,
                };
                self.rows.push(row);
                Ok(self.rows.len() - 1)
            }
        }
    }
}

fn derive_again(text: &str) -> Result<Derived, String> {
    let d = read(text)?;
    // The groups: x, y, z, then rounded inputs' groups, each only if it has a scale variable.
    let mut candidates: Vec<String> = ["x", "y", "z"].iter().map(|s| (*s).to_owned()).collect();
    for (_, k) in &d.inputs {
        if let Kind::Rounded(_, g) = k {
            if !candidates.contains(g) {
                candidates.push(g.clone());
            }
        }
    }
    // A first walk over every group finds which are used; the second numbers them.
    let mut groups = candidates.clone();
    for pass in 0..2 {
        let mut w = Walker {
            d: &d,
            rows: Vec::new(),
            lets: BTreeMap::new(),
            loads: BTreeMap::new(),
            groups: groups.clone(),
        };
        for (name, node) in &d.lets {
            let mut k = 0;
            let i = w.visit(node, name, &mut k, true)?;
            w.lets.insert(name.clone(), i);
        }
        if pass == 0 {
            groups = candidates
                .iter()
                .filter(|g| {
                    w.rows
                        .iter()
                        .any(|r| r.group.as_deref() == Some(g.as_str()))
                })
                .cloned()
                .collect();
            continue;
        }
        // Every value has a name of its own.
        let mut names = BTreeSet::new();
        for r in &w.rows {
            if !names.insert(r.name.clone()) {
                return Err(format!("two values are named `{}`", r.name));
            }
        }
        let result = *w
            .lets
            .get(&d.result)
            .ok_or_else(|| format!("`sign {}` names no let", d.result))?;
        return Ok(Derived {
            rows: w.rows,
            groups,
            result,
        });
    }
    Err("unreachable".to_owned())
}

fn monomial(m: &[u32], groups: &[String]) -> String {
    let parts: Vec<String> = groups
        .iter()
        .zip(m)
        .filter(|(_, e)| **e > 0)
        .map(|(g, e)| {
            if *e == 1 {
                g.clone()
            } else {
                format!("{g}^{e}")
            }
        })
        .collect();
    if parts.is_empty() {
        "1".to_owned()
    } else {
        parts.join("·")
    }
}

fn scale_text(s: &BTreeSet<Vec<u32>>, groups: &[String]) -> String {
    let parts: Vec<String> = s.iter().map(|m| monomial(m, groups)).collect();
    if parts.len() == 1 {
        parts[0].clone()
    } else {
        format!("max{{{}}}", parts.join(", "))
    }
}

// ---------------------------------------------------------------------------------------------
// The run-time scale expression, read from the committed code.
// ---------------------------------------------------------------------------------------------

/// The words of a line of code: maximal runs of letters, digits and `_`.
fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
}

/// `s` with every whole word `word` replaced by `with`.
fn replace_word(s: &str, word: &str, with: &str) -> String {
    let mut out = String::new();
    let mut cur = String::new();
    for ch in s.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            cur.push(ch);
            continue;
        }
        out.push_str(if cur == word { with } else { &cur });
        cur.clear();
        out.push(ch);
    }
    out.pop();
    out
}

type Scale = (BTreeSet<Vec<u32>>, u32);

/// The monomials an expression of `m_<group>`, `1.0`, `*` and `larger(…, …)` computes the
/// largest of, and its roundings r, with the computed value at least the exact one times
/// (1 − u)^r: each `*` rounds once and carries both its operands' roundings, since relative errors
/// multiply; `larger` carries the larger of its operands'.
fn scale_expr(s: &str, groups: &[String]) -> Result<Scale, String> {
    fn product(toks: &[String], at: &mut usize, groups: &[String]) -> Result<Scale, String> {
        let (mut set, mut k) = factor(toks, at, groups)?;
        while toks.get(*at).map(String::as_str) == Some("*") {
            *at += 1;
            let (s2, k2) = factor(toks, at, groups)?;
            let mut out = BTreeSet::new();
            for a in &set {
                for b in &s2 {
                    out.insert(a.iter().zip(b).map(|(x, y)| x + y).collect());
                }
            }
            set = out;
            k = k + k2 + 1;
        }
        Ok((set, k))
    }
    fn expect(toks: &[String], at: &mut usize, t: &str) -> Result<(), String> {
        if toks.get(*at).map(String::as_str) == Some(t) {
            *at += 1;
            Ok(())
        } else {
            Err(format!("expected `{t}` in the scale"))
        }
    }
    fn factor(toks: &[String], at: &mut usize, groups: &[String]) -> Result<Scale, String> {
        let t = toks.get(*at).ok_or("the scale ends early")?.clone();
        *at += 1;
        if t == "larger" {
            expect(toks, at, "(")?;
            let (a, ka) = product(toks, at, groups)?;
            expect(toks, at, ",")?;
            let (b, kb) = product(toks, at, groups)?;
            expect(toks, at, ")")?;
            let mut s = a;
            s.extend(b);
            return Ok((s, ka.max(kb)));
        }
        if t == "(" {
            let r = product(toks, at, groups)?;
            expect(toks, at, ")")?;
            return Ok(r);
        }
        let mut m = vec![0u32; groups.len()];
        if t != "1.0" {
            let g = t
                .strip_prefix("m_")
                .ok_or_else(|| format!("`{t}` in the scale is not a group maximum"))?;
            let i = groups
                .iter()
                .position(|x| x == g)
                .ok_or_else(|| format!("`{t}` in the scale names no group"))?;
            m[i] = 1;
        }
        Ok((BTreeSet::from([m]), 0))
    }
    // Tokens: names (with `_`), `1.0`, `*`, `(`, `)`, `,`.
    let mut toks = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            cur.push(ch);
        } else {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            if !ch.is_whitespace() {
                toks.push(ch.to_string());
            }
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    let mut at = 0;
    let r = product(&toks, &mut at, groups)?;
    if at != toks.len() {
        return Err(format!("text after the scale expression `{s}`"));
    }
    Ok(r)
}

// ---------------------------------------------------------------------------------------------
// The code, as the checker writes it.
// ---------------------------------------------------------------------------------------------

/// The lines of `source` that are code: not blank, not a comment, not an attribute (a one-line
/// `#[…]` or a `#[allow(` … `)]` block).
fn code_lines(source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_attribute = false;
    for l in source.lines() {
        let t = l.trim();
        if in_attribute {
            in_attribute = t != ")]";
            continue;
        }
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        if t.starts_with("#[") {
            in_attribute = !t.ends_with(']');
            continue;
        }
        out.push(l);
    }
    out
}

/// A line of the expected code: exactly this text, or the run-time scale, which is read and
/// evaluated rather than written.
enum Expect {
    Line(String),
    Scale,
}

/// The `f64` constant's literal, as clippy's `unreadable_literal` asks.
fn literal(bits: u64) -> String {
    let h = format!("{bits:016x}");
    format!("0x{}_{}_{}_{}", &h[0..4], &h[4..8], &h[8..12], &h[12..16])
}

/// A function's first line or lines: on one line when it fits 100 columns, otherwise one
/// parameter per line, as rustfmt's default layout writes it.
fn signature_lines(name: &str, params: &[String], ret: &str) -> Vec<String> {
    let one = format!("pub fn {name}({}) -> {ret} {{", params.join(", "));
    if one.chars().count() <= 100 {
        return vec![one];
    }
    let mut lines = vec![format!("pub fn {name}(")];
    lines.extend(params.iter().map(|p| format!("    {p},")));
    lines.push(format!(") -> {ret} {{"));
    lines
}

/// A description's parameters, in its order: a point as `P2` or `P3`, a rounded input as `i128`.
fn parameters(d: &Desc) -> Vec<String> {
    d.inputs
        .iter()
        .map(|(n, k)| match k {
            Kind::Point(2) => format!("{n}: P2"),
            Kind::Point(_) => format!("{n}: P3"),
            Kind::Rounded(..) => format!("{n}: i128"),
        })
        .collect()
}

/// The code lines of the plain-`i128` variant (piece G42), as the checker writes them from its
/// own derivation of every registered predicate, in their order: the imports, then for each
/// predicate `<predicate>_value`, each step in the type [`held`] gives it.
fn expected_plain() -> Result<Vec<Expect>, String> {
    let mut parts = Vec::new();
    for p in &PREDICATES {
        parts.push((p.comparison.name, read(p.text)?, derive_again(p.text)?));
    }
    let mut points: BTreeSet<&str> = BTreeSet::new();
    let mut wides: BTreeSet<&str> = BTreeSet::new();
    for (_, d, x) in &parts {
        for (_, k) in &d.inputs {
            match k {
                Kind::Point(2) => points.insert("P2"),
                Kind::Point(_) => points.insert("P3"),
                Kind::Rounded(..) => false,
            };
        }
        wides.extend(x.rows.iter().map(|r| held(r.width)).filter(|w| is_wide(w)));
    }
    let mut out = Vec::new();
    let mut push = |s: String| out.push(Expect::Line(s));
    let points: Vec<&str> = points.into_iter().collect();
    match points.as_slice() {
        [] => {}
        [one] => push(format!("use crate::grid::{one};")),
        many => push(format!("use crate::grid::{{{}}};", many.join(", "))),
    }
    push(WIDE_IMPORT.to_owned());
    if parts
        .iter()
        .any(|(_, _, x)| x.rows.iter().any(|r| r.rounded))
    {
        push("use super::within;".to_owned());
    }
    for (name, d, x) in &parts {
        let root = &x.rows[x.result];
        let ret = format!("Option<{}>", held(root.width));
        for l in signature_lines(&format!("{name}_value"), &parameters(d), &ret) {
            push(l);
        }
        for r in &x.rows {
            push(format!("    let {} = {};", r.name, r.plain));
        }
        push(format!("    Some({})", root.name));
        push("}".to_owned());
    }
    let wides: Vec<&str> = wides.into_iter().collect();
    Ok(settle_wide_import(out, &wides))
}

/// The code lines of a predicate module, as the checker writes them from its own derivation.
fn expected_module(d: &Desc, x: &Derived, constant: u64, larger: bool) -> Vec<Expect> {
    let mut out = Vec::new();
    let mut push = |s: String| out.push(Expect::Line(s));
    let point = d.inputs.iter().find_map(|(_, k)| match k {
        Kind::Point(2) => Some("P2"),
        Kind::Point(_) => Some("P3"),
        Kind::Rounded(..) => None,
    });
    if let Some(p) = point {
        push(format!("use crate::grid::{p};"));
    }
    push("use crate::sign::Sign;".to_owned());
    let wides: BTreeSet<&str> = x
        .rows
        .iter()
        .map(|r| r.width)
        .filter(|w| is_wide(w))
        .collect();
    let wides: Vec<&str> = wides.into_iter().collect();
    // The import names exactly the wide types the code names, so it is settled once the code
    // lines are known (below).
    push(WIDE_IMPORT.to_owned());
    let mut helpers = vec!["decide", "largest_magnitude"];
    if x.rows.iter().any(|r| r.coordinates) {
        helpers.push("exact_f64");
    }
    if larger {
        helpers.push("larger");
    }
    if x.rows.iter().any(|r| r.rounded) {
        helpers.push("rounded_f64");
        helpers.push("within");
    }
    helpers.sort_unstable();
    push(format!("use super::{{{}}};", helpers.join(", ")));
    push(format!(
        "const ERROR: f64 = f64::from_bits({});",
        literal(constant)
    ));
    let params = parameters(d);
    let signature = |name: &str, ret: &str| signature_lines(name, &params, ret);
    let args: Vec<&str> = d.inputs.iter().map(|(n, _)| n.as_str()).collect();
    let root = &x.rows[x.result];
    for l in signature("filter", "Option<Sign>") {
        push(l);
    }
    for r in &x.rows {
        push(format!("    let {} = {};", r.name, r.filter));
    }
    for g in &x.groups {
        let vars: Vec<&str> = x
            .rows
            .iter()
            .filter(|r| r.group.as_deref() == Some(g.as_str()))
            .map(|r| r.name.as_str())
            .collect();
        push(format!(
            "    let m_{g} = largest_magnitude([{}]);",
            vars.join(", ")
        ));
    }
    out.push(Expect::Scale);
    let mut push = |s: String| out.push(Expect::Line(s));
    push(format!("    decide({}, ERROR, scale)", root.name));
    push("}".to_owned());
    for l in signature("value", &format!("Option<{}>", root.width)) {
        push(l);
    }
    for r in &x.rows {
        push(format!("    let {} = {};", r.name, r.exact));
    }
    push(format!("    Some({})", root.name));
    push("}".to_owned());
    for l in signature("exact", "Option<Sign>") {
        push(l);
    }
    let sign = match root.width {
        "i64" => "Sign::of_i64",
        "i128" => "Sign::of_i128",
        _ => "|v| v.signum()",
    };
    // On one line when it fits 100 columns and its arguments 60 (rustfmt's `fn_call_width`),
    // otherwise the arguments on a line of their own and `.map` after the call, as rustfmt breaks
    // the chain.
    let args = args.join(", ");
    let one = format!("    value({args}).map({sign})");
    if one.chars().count() <= 100 && args.chars().count() <= 60 {
        push(one);
    } else {
        push("    value(".to_owned());
        push(format!("        {args},"));
        push("    )".to_owned());
        push(format!("    .map({sign})"));
    }
    push("}".to_owned());
    settle_wide_import(out, &wides)
}

/// Where [`expected_module`] puts the wide-integer import until the code lines are known.
const WIDE_IMPORT: &str = "<wide import>";

/// Replaces the [`WIDE_IMPORT`] line with an import of the candidate wide types that the code
/// lines name as a whole word (none, one, or a braced list), or drops it when they name none.
fn settle_wide_import(out: Vec<Expect>, candidates: &[&str]) -> Vec<Expect> {
    let code: Vec<&str> = out
        .iter()
        .filter_map(|e| match e {
            Expect::Line(l) if l != WIDE_IMPORT => Some(l.as_str()),
            _ => None,
        })
        .collect();
    let named = |ty: &str| {
        code.iter().any(|l| {
            l.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|word| word == ty)
        })
    };
    let used: Vec<&str> = candidates.iter().copied().filter(|ty| named(ty)).collect();
    let import = match used.as_slice() {
        [] => None,
        [one] => Some(format!("use crate::wide::{one};")),
        many => Some(format!("use crate::wide::{{{}}};", many.join(", "))),
    };
    out.into_iter()
        .filter_map(|e| match e {
            Expect::Line(l) if l == WIDE_IMPORT => import.clone().map(Expect::Line),
            other => Some(other),
        })
        .collect()
}

/// How the limb variant holds a step, by the checker's own rule (piece G42): `None` for an
/// `i64`, otherwise the number of 32-bit limbs.
fn limbs_of_row(x: &Derived, i: usize) -> Option<usize> {
    let r = &x.rows[i];
    if r.width == "i64" {
        return None;
    }
    // Magnitude below 2^b: a product's b is the sum of its operands' bits, a sum's one more than
    // the larger; it needs b + 1 bits with the sign, in whole 32-bit limbs, at least two.
    let b = match r.operands {
        Some(('*', l, rr)) => x.rows[l].bits + x.rows[rr].bits,
        Some((_, l, rr)) => x.rows[l].bits.max(x.rows[rr].bits) + 1,
        None => r.bits,
    };
    Some(((b + 1).div_ceil(32) as usize).max(2))
}

/// `name`, held as `from` limbs (`None` for `i64`), as `n` limbs.
fn in_limbs(name: &str, from: Option<usize>, n: usize) -> String {
    match from {
        None => format!("Wide32::<{n}>::from_i64({name})"),
        Some(m) if m == n => name.to_owned(),
        Some(_) => format!("{name}.resize::<{n}>()?"),
    }
}

/// The limb variant's expression for an operation in `n` limbs.
fn limb_expr(op: char, l: &str, lh: Option<usize>, r: &str, rh: Option<usize>, n: usize) -> String {
    match (op, lh, rh) {
        ('*', None, None) => format!("Wide32::<{n}>::product_i64({l}, {r})?"),
        ('*', Some(_), None) => format!("Wide32::<{n}>::product_by_i64(&{l}, {r})?"),
        ('*', None, Some(_)) => format!("Wide32::<{n}>::product_by_i64(&{r}, {l})?"),
        ('*', Some(_), Some(_)) => format!("Wide32::<{n}>::product(&{l}, &{r})?"),
        (op, lh, rh) => {
            let method = if op == '+' {
                "checked_add"
            } else {
                "checked_sub"
            };
            format!("{}.{method}(&{})?", in_limbs(l, lh, n), in_limbs(r, rh, n))
        }
    }
}

/// One kind of limb step, as the checker counts them; ordered as the module lists its proofs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum LimbKind {
    Rounded(u32, usize),
    Operation(char, (Option<usize>, u32), (Option<usize>, u32), usize),
    Back(usize, u32, usize),
}

/// A held operand in a proof's name, and as a symbolic value.
fn kind_name(h: Option<usize>, bits: u32) -> String {
    match h {
        None => format!("i64x{bits}"),
        Some(n) => format!("w{n}x{bits}"),
    }
}

fn kind_value(h: Option<usize>, bits: u32) -> String {
    match h {
        None => format!("small({bits})"),
        Some(n) => format!("big::<{n}>({bits})"),
    }
}

/// The code lines of the limb variant (piece G42), as the checker writes them from its own
/// derivation of every registered predicate, in their order: the imports, each predicate's
/// `<predicate>_value`, and the proof module.
fn expected_limbs() -> Result<Vec<Expect>, String> {
    let mut parts = Vec::new();
    for p in &PREDICATES {
        parts.push((p.comparison.name, read(p.text)?, derive_again(p.text)?));
    }
    let mut points: BTreeSet<&str> = BTreeSet::new();
    let mut wides: BTreeSet<&str> = BTreeSet::new();
    for (_, d, x) in &parts {
        for (_, k) in &d.inputs {
            match k {
                Kind::Point(2) => points.insert("P2"),
                Kind::Point(_) => points.insert("P3"),
                Kind::Rounded(..) => false,
            };
        }
        let root = x.rows[x.result].width;
        if is_wide(root) {
            wides.insert(root);
        }
    }
    let mut out = Vec::new();
    let mut push = |s: String| out.push(Expect::Line(s));
    let points: Vec<&str> = points.into_iter().collect();
    match points.as_slice() {
        [] => {}
        [one] => push(format!("use crate::grid::{one};")),
        many => push(format!("use crate::grid::{{{}}};", many.join(", "))),
    }
    push(WIDE_IMPORT.to_owned());
    push("use crate::wide32::Wide32;".to_owned());
    if parts
        .iter()
        .any(|(_, _, x)| x.rows.iter().any(|r| r.rounded))
    {
        push("use super::within;".to_owned());
    }
    let mut kinds: BTreeSet<LimbKind> = BTreeSet::new();
    for (name, d, x) in &parts {
        let root = &x.rows[x.result];
        let ret = format!("Option<{}>", root.width);
        for l in signature_lines(&format!("{name}_value"), &parameters(d), &ret) {
            push(l);
        }
        for (i, r) in x.rows.iter().enumerate() {
            let h = limbs_of_row(x, i);
            let statement = match (h, r.operands) {
                // An i64 step is the shipped statement, whose operands are i64 too.
                (None, _) => r.exact.clone(),
                (Some(n), None) => {
                    kinds.insert(LimbKind::Rounded(r.bits, n));
                    format!("Wide32::<{n}>::from_i128(within({}, {})?)?", r.name, r.bits)
                }
                (Some(n), Some((op, li, ri))) => {
                    let (lh, rh) = (limbs_of_row(x, li), limbs_of_row(x, ri));
                    kinds.insert(LimbKind::Operation(
                        op,
                        (lh, x.rows[li].bits),
                        (rh, x.rows[ri].bits),
                        n,
                    ));
                    limb_expr(op, &x.rows[li].name, lh, &x.rows[ri].name, rh, n)
                }
            };
            push(format!("    let {} = {statement};", r.name));
        }
        match (limbs_of_row(x, x.result), root.width) {
            (None, _) => push(format!("    Some({})", root.name)),
            (Some(n), "i128") => {
                kinds.insert(LimbKind::Back(n, root.bits, 2));
                push(format!("    {}.to_i128()", root.name));
            }
            (Some(n), w) => {
                let to = limbs_of(w);
                kinds.insert(LimbKind::Back(n, root.bits, to));
                push(format!("    {}.to_wide::<{to}>()", root.name));
            }
        }
        push("}".to_owned());
    }
    for l in [
        "mod proofs {",
        "    use super::*;",
        "    fn small(bits: u32) -> i64 {",
        "        let v: i64 = kani::any();",
        "        kani::assume(v.unsigned_abs().checked_shr(bits) == Some(0));",
        "        v",
        "    }",
        "    fn wide_input(bits: u32) -> i128 {",
        "        let v: i128 = kani::any();",
        "        kani::assume(v.unsigned_abs().checked_shr(bits) == Some(0));",
        "        v",
        "    }",
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
    ] {
        push(l.to_owned());
    }
    for kind in &kinds {
        match kind {
            LimbKind::Rounded(bits, n) => {
                push(format!("    fn rounded_{bits}_in_{n}() {{"));
                push(format!("        let v = wide_input({bits});"));
                push(format!(
                    "        assert!(Wide32::<{n}>::from_i128(v).is_some());"
                ));
            }
            LimbKind::Operation(op, (lh, lb), (rh, rb), n) => {
                let word = match op {
                    '+' => "add",
                    '-' => "sub",
                    _ => "mul",
                };
                push(format!(
                    "    fn {word}_{}_{}_in_{n}() {{",
                    kind_name(*lh, *lb),
                    kind_name(*rh, *rb)
                ));
                push(format!(
                    "        let (l, r) = ({}, {});",
                    kind_value(*lh, *lb),
                    kind_value(*rh, *rb)
                ));
                push(format!(
                    "        let step = || -> Option<Wide32<{n}>> {{ Some({}) }};",
                    limb_expr(*op, "l", *lh, "r", *rh, *n)
                ));
                push("        assert!(step().is_some());".to_owned());
            }
            LimbKind::Back(n, bits, to) => {
                let target = if *to == 2 {
                    "i128".to_owned()
                } else {
                    to.to_string()
                };
                push(format!("    fn back_w{n}x{bits}_to_{target}() {{"));
                push(format!("        let v = big::<{n}>({bits});"));
                push(if *to == 2 {
                    "        assert!(v.to_i128().is_some());".to_owned()
                } else {
                    format!("        assert!(v.to_wide::<{to}>().is_some());")
                });
            }
        }
        push("    }".to_owned());
    }
    push("}".to_owned());
    let wides: Vec<&str> = wides.into_iter().collect();
    Ok(settle_wide_import(out, &wides))
}

/// The limb variant's code lines against [`expected_limbs`]'s, line for line (piece G42).
fn verify_limbs(limbs: &str) -> Vec<String> {
    let want = match expected_limbs() {
        Ok(w) => w,
        Err(e) => return vec![format!("a description: {e}")],
    };
    let got = code_lines(limbs);
    let mut bad = Vec::new();
    if got.len() != want.len() {
        bad.push(format!(
            "{} lines of code in limbs.rs, {} expected",
            got.len(),
            want.len()
        ));
    }
    for (g, w) in got.iter().zip(&want) {
        match w {
            Expect::Line(w) if g == w => {}
            Expect::Line(w) => bad.push(format!("limbs.rs: `{g}`, expected `{w}`")),
            Expect::Scale => bad.push("limbs.rs: a scale where none belongs".to_owned()),
        }
    }
    bad
}

/// The plain-`i128` variant's code lines against [`expected_plain`]'s, line for line (piece G42).
fn verify_plain(plain: &str) -> Vec<String> {
    let want = match expected_plain() {
        Ok(w) => w,
        Err(e) => return vec![format!("a description: {e}")],
    };
    let got = code_lines(plain);
    let mut bad = Vec::new();
    if got.len() != want.len() {
        bad.push(format!(
            "{} lines of code in plain.rs, {} expected",
            got.len(),
            want.len()
        ));
    }
    for (g, w) in got.iter().zip(&want) {
        match w {
            Expect::Line(w) if g == w => {}
            Expect::Line(w) => bad.push(format!("plain.rs: `{g}`, expected `{w}`")),
            Expect::Scale => bad.push("plain.rs: a scale where none belongs".to_owned()),
        }
    }
    bad
}

/// The code lines of the parent module, as the checker writes them: one `pub mod` per predicate,
/// then the helpers, each exactly what the proof in `predicate_derive.rs` relies on.
fn expected_parent(names: &[&str], coordinates: bool, rounded: bool) -> Vec<String> {
    let mut sorted = names.to_vec();
    sorted.sort_unstable();
    let mut out: Vec<String> = sorted.iter().map(|n| format!("pub mod {n};")).collect();
    let mut lines = |ls: &[&str]| out.extend(ls.iter().map(|l| (*l).to_owned()));
    if coordinates {
        lines(&["use crate::grid::Coord;"]);
    }
    lines(&["use crate::sign::Sign;"]);
    if coordinates {
        // A coordinate is a whole number of at most 2^40 − 1 in magnitude: the conversion is
        // exact, and nothing else may happen to it.
        lines(&[
            "const fn exact_f64(c: Coord) -> f64 {",
            "    c.get() as f64",
            "}",
        ]);
    }
    lines(&[
        // The larger of two non-negative values, by comparison (no `max`, which may pick either
        // zero).
        "const fn larger(a: f64, b: f64) -> f64 {",
        "    if a < b { b } else { a }",
        "}",
        // M(g): the largest absolute value. Without `abs` the maximum of signed values would
        // undercount the scale.
        "fn largest_magnitude<const N: usize>(values: [f64; N]) -> f64 {",
        "    values.iter().map(|v| v.abs()).reduce(larger).unwrap_or(0.0)",
        "}",
        // The decision: a sign only when the value is strictly beyond the bound (a value equal to
        // the bound proves nothing), zero only for a zero scale, the bound computed with one
        // rounding, which the constant counts.
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
        "    (above | below | (scale == 0.0)).then_some(sign)",
        "}",
    ]);
    if rounded {
        lines(&[
            // The declared range, |v| ≤ 2^bits − 1, inclusive, within which the bounds hold.
            "fn within(v: i128, bits: u32) -> Option<i128> {",
            "    let limit = 1u128.checked_shl(bits)?.checked_sub(1)?;",
            "    if v.unsigned_abs() <= limit {",
            "        Some(v)",
            "    } else {",
            "        None",
            "    }",
            "}",
            // The nearest f64, ties to even: Rust's integer-to-float conversion, the rounding
            // whose error u · |x̃| the analysis gives a rounded input.
            "fn rounded_f64(v: i128, bits: u32) -> Option<f64> {",
            "    within(v, bits).map(|v| v as f64)",
            "}",
        ]);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// The check of one committed module, and of the parent.
// ---------------------------------------------------------------------------------------------

/// The cells of a Markdown table row `//! | a | b | … |`.
fn cells(line: &str) -> Vec<String> {
    line.trim_start_matches("//!")
        .trim()
        .trim_matches('|')
        .split(" | ")
        .map(|c| c.trim().to_owned())
        .collect()
}

/// Checks a committed module against the description, independently; returns what failed.
fn verify(text: &str, module: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let (desc, d) = match read(text).and_then(|desc| derive_again(text).map(|d| (desc, d))) {
        Ok(both) => both,
        Err(e) => return vec![format!("the description: {e}")],
    };
    let groups = &d.groups;

    // The description, verbatim.
    let header: Vec<&str> = module
        .lines()
        .skip_while(|l| *l != "//! ```text")
        .skip(1)
        .take_while(|l| *l != "//! ```")
        .map(|l| l.strip_prefix("//! ").unwrap_or(""))
        .collect();
    if header != text.lines().collect::<Vec<_>>() {
        bad.push("the description in the header differs".to_owned());
    }

    // The two tables of steps.
    let rows: Vec<Vec<String>> = module
        .lines()
        .filter(|l| l.starts_with("//! | `"))
        .map(cells)
        .collect();
    let (certificate, analysis) = rows.split_at(rows.len().min(d.rows.len()));
    if certificate.len() != d.rows.len() || analysis.len() < d.rows.len() {
        bad.push(format!(
            "{} certificate rows and {} analysis rows for {} steps",
            certificate.len(),
            analysis.len(),
            d.rows.len()
        ));
        return bad;
    }
    for ((r, c), a) in d.rows.iter().zip(certificate).zip(analysis) {
        let want_c = [
            format!("`{}`", r.name),
            format!("`{}`", r.computes),
            r.bound.decimal(),
            r.bits.to_string(),
            format!("`{}`", r.width),
        ];
        if c.as_slice() != want_c.as_slice() {
            bad.push(format!("certificate row {c:?}, derived {want_c:?}"));
        }
        let want_a = [
            format!("`{}`", r.name),
            scale_text(&r.scale, groups),
            if r.exact_f64 { "yes" } else { "no" }.to_owned(),
            r.b.text(),
            r.e.text(),
        ];
        if a.as_slice() != want_a.as_slice() {
            bad.push(format!("analysis row {a:?}, derived {want_a:?}"));
        }
    }

    // The run-time scale: one line binds `scale`, or several in a row, each after the first
    // using the one before it; together they compute the result's scale.
    let root = &d.rows[d.result];
    let code = code_lines(module);
    let binds = |l: &&str| l.starts_with("    let scale = ");
    let Some(first) = code.iter().position(binds) else {
        bad.push("no line binds `scale`".to_owned());
        return bad;
    };
    let run = code[first..].iter().take_while(|l| binds(l)).count();
    if code.iter().filter(|l| binds(l)).count() != run {
        bad.push("the lines that bind `scale` are not in one run".to_owned());
        return bad;
    }
    let mut scale_src = String::new();
    for (k, l) in code[first..first + run].iter().enumerate() {
        let rhs = l
            .trim_start_matches("    let scale = ")
            .trim_end_matches(';');
        let uses = words(rhs).filter(|w| *w == "scale").count();
        if uses != usize::from(k > 0) {
            bad.push(format!(
                "scale line {} uses `scale` {uses} time(s): `{l}`",
                k + 1
            ));
            return bad;
        }
        scale_src = if k == 0 {
            rhs.to_owned()
        } else {
            replace_word(rhs, "scale", &scale_src)
        };
    }
    let scale_src = scale_src.as_str();
    let (computed, scale_roundings) = match scale_expr(scale_src, groups) {
        Ok(s) => s,
        Err(e) => {
            bad.push(format!("the run-time scale: {e}"));
            return bad;
        }
    };
    if computed != root.scale {
        bad.push(format!(
            "the run-time scale computes {computed:?}, not the result's scale {:?}",
            root.scale
        ));
    }

    // The constant: E · (1 + 2u)^k, k the scale's roundings plus the product with the constant.
    let k = scale_roundings + 1;
    let mut constant = root.e.clone();
    let grow = Poly(vec![Big::small(1), Big::small(2)]);
    for _ in 0..k {
        constant = constant.mul(&grow);
    }
    let bits = constant.f64_at_least();

    // Every line of code, the run of scale lines read as one.
    let expected = expected_module(&desc, &d, bits, scale_src.contains("larger("));
    let mut code = code;
    code.drain(first + 1..first + run);
    if code.len() != expected.len() {
        bad.push(format!(
            "{} lines of code, {} expected",
            code.len(),
            expected.len()
        ));
    }
    for (got, want) in code.iter().zip(&expected) {
        match want {
            Expect::Line(w) if *got != w.as_str() => {
                bad.push(format!("code `{got}`, expected `{w}`"));
            }
            Expect::Scale if !got.starts_with("    let scale = ") => {
                bad.push(format!("code `{got}`, expected the scale"));
            }
            _ => {}
        }
    }

    let fraction = bits & ((1 << 52) - 1);
    let exponent = (bits >> 52) as i64 - 1075;
    let summary: BTreeMap<String, String> = module
        .lines()
        .filter(|l| l.starts_with("//! | ") && !l.starts_with("//! | `"))
        .map(cells)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].clone(), c[1].clone()))
        .collect();
    let want = [
        ("Result", format!("`{}`", root.name)),
        ("Unit error E", root.e.text()),
        ("Scale S", scale_text(&root.scale, groups)),
        ("Scale at run time", format!("`{scale_src}`")),
        ("Roundings k", k.to_string()),
        ("Constant, exactly", constant.text()),
    ];
    for (key, value) in want {
        if summary.get(key) != Some(&value) {
            bad.push(format!(
                "summary `{key}` is {:?}, derived {value:?}",
                summary.get(key)
            ));
        }
    }
    let f64_row = summary
        .get("Constant as an `f64`")
        .cloned()
        .unwrap_or_default();
    let exact_form = format!("`{bits:#018x}` = {} · 2^{exponent}", fraction | (1 << 52));
    if !f64_row.starts_with(&exact_form) {
        bad.push(format!(
            "summary constant {f64_row:?}, derived {exact_form:?}"
        ));
    }
    bad
}

/// Checks the committed parent module against the registered predicates; returns what failed.
fn verify_parent(parent: &str) -> Vec<String> {
    let mut names: Vec<&str> = PREDICATES.iter().map(|p| p.comparison.name).collect();
    // The variant modules (piece G42).
    names.push("limbs");
    names.push("plain");
    let mut coordinates = false;
    let mut rounded = false;
    for p in &PREDICATES {
        let Ok(d) = derive_again(p.text) else {
            return vec![format!("{}: the description does not read", p.generator)];
        };
        coordinates |= d.rows.iter().any(|r| r.coordinates);
        rounded |= d.rows.iter().any(|r| r.rounded);
    }
    let want = expected_parent(&names, coordinates, rounded);
    let got = code_lines(parent);
    let mut bad = Vec::new();
    if got.len() != want.len() {
        bad.push(format!(
            "{} lines of code in mod.rs, {} expected",
            got.len(),
            want.len()
        ));
    }
    for (g, w) in got.iter().zip(&want) {
        if *g != w.as_str() {
            bad.push(format!("mod.rs: `{g}`, expected `{w}`"));
        }
    }
    bad
}

// ---------------------------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------------------------

/// The parent module, built into the test so it also runs on wasm32.
const PARENT: &str = include_str!("../../geom-num/src/generated/mod.rs");

/// The limb variant (piece G42), built into the test so it also runs on wasm32.
const LIMBS: &str = include_str!("../../geom-num/src/generated/limbs.rs");

/// The plain-`i128` variant (piece G42), built into the test so it also runs on wasm32.
const PLAIN: &str = include_str!("../../geom-num/src/generated/plain.rs");

/// The committed modules, built into the test so it also runs on wasm32. A test ties this list to
/// `PREDICATES`.
const COMMITTED: [(&str, &str); 23] = [
    (
        "orient2d",
        include_str!("../../geom-num/src/generated/orient2d.rs"),
    ),
    (
        "orient3d",
        include_str!("../../geom-num/src/generated/orient3d.rs"),
    ),
    (
        "incircle",
        include_str!("../../geom-num/src/generated/incircle.rs"),
    ),
    (
        "insphere",
        include_str!("../../geom-num/src/generated/insphere.rs"),
    ),
    (
        "plane_side",
        include_str!("../../geom-num/src/generated/plane_side.rs"),
    ),
    (
        "tpi_weight",
        include_str!("../../geom-num/src/generated/tpi_weight.rs"),
    ),
    (
        "tpi_side",
        include_str!("../../geom-num/src/generated/tpi_side.rs"),
    ),
    (
        "lpi_weight",
        include_str!("../../geom-num/src/generated/lpi_weight.rs"),
    ),
    (
        "lpi_side",
        include_str!("../../geom-num/src/generated/lpi_side.rs"),
    ),
    (
        "ssi_weight",
        include_str!("../../geom-num/src/generated/ssi_weight.rs"),
    ),
    (
        "ssi_side",
        include_str!("../../geom-num/src/generated/ssi_side.rs"),
    ),
    (
        "carrier_side",
        include_str!("../../geom-num/src/generated/carrier_side.rs"),
    ),
    (
        "carrier_order",
        include_str!("../../geom-num/src/generated/carrier_order.rs"),
    ),
    (
        "rational_cmp",
        include_str!("../../geom-num/src/generated/rational_cmp.rs"),
    ),
    (
        "rational_orient",
        include_str!("../../geom-num/src/generated/rational_orient.rs"),
    ),
    (
        "round_grid",
        include_str!("../../geom-num/src/generated/round_grid.rs"),
    ),
    (
        "round_rational",
        include_str!("../../geom-num/src/generated/round_rational.rs"),
    ),
    (
        "round_line",
        include_str!("../../geom-num/src/generated/round_line.rs"),
    ),
    (
        "circle_side",
        include_str!("../../geom-num/src/generated/circle_side.rs"),
    ),
    (
        "circle_line",
        include_str!("../../geom-num/src/generated/circle_line.rs"),
    ),
    (
        "circle_gap",
        include_str!("../../geom-num/src/generated/circle_gap.rs"),
    ),
    (
        "ellipse_side",
        include_str!("../../geom-num/src/generated/ellipse_side.rs"),
    ),
    (
        "ellipse_line",
        include_str!("../../geom-num/src/generated/ellipse_line.rs"),
    ),
];

fn predicate(name: &str) -> &'static Predicate {
    PREDICATES
        .iter()
        .find(|p| p.comparison.name == name)
        .unwrap()
}

/// Every committed module, and the parent module, agree with the checker's own derivation, line
/// by line.
#[test]
fn the_checker_agrees_with_every_committed_module() {
    for (name, module) in COMMITTED {
        let p = predicate(name);
        let bad = verify(p.text, module);
        assert!(bad.is_empty(), "{name}:\n{}", bad.join("\n"));
        let d = derive_again(p.text).unwrap();
        println!(
            "{name}: {} steps agree; the result has {} bits",
            d.rows.len(),
            d.rows[d.result].bits
        );
    }
    let bad = verify_parent(PARENT);
    assert!(bad.is_empty(), "mod.rs:\n{}", bad.join("\n"));
    let bad = verify_plain(PLAIN);
    assert!(bad.is_empty(), "plain.rs:\n{}", bad.join("\n"));
    let bad = verify_limbs(LIMBS);
    assert!(bad.is_empty(), "limbs.rs:\n{}", bad.join("\n"));
}

/// The modules read here are exactly the registered predicates, in their order; and, natively,
/// the generated folder holds exactly these files, the parent module and its `.gitattributes`.
#[test]
fn the_committed_list_is_every_registered_predicate() {
    let listed: Vec<&str> = COMMITTED.iter().map(|(n, _)| *n).collect();
    let registered: Vec<&str> = PREDICATES.iter().map(|p| p.comparison.name).collect();
    assert_eq!(listed, registered);
    if cfg!(target_family = "wasm") {
        return;
    }
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../geom-num/src/generated");
    let mut files: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    let mut want: Vec<String> = registered.iter().map(|n| format!("{n}.rs")).collect();
    want.push("mod.rs".to_owned());
    want.push("plain.rs".to_owned());
    want.push("limbs.rs".to_owned());
    want.push(".gitattributes".to_owned());
    want.sort();
    assert_eq!(files, want);
}

/// The checker agrees with the generator's in-memory derivation too: every bound, width, unit
/// bound and unit error, and the constant, exactly and as an `f64`.
#[test]
fn the_checker_agrees_with_the_generator() {
    for p in &PREDICATES {
        let g = derivation(p).unwrap();
        let c = derive_again(p.text).unwrap();
        assert_eq!(g.steps.len(), c.rows.len(), "{}", p.generator);
        for ((step, cert), row) in g.steps.iter().zip(&g.certs).zip(&c.rows) {
            assert_eq!(step.name, row.name);
            assert_eq!(
                cert.bound.to_decimal().unwrap(),
                row.bound.decimal(),
                "{}",
                row.name
            );
            assert_eq!(cert.bits, row.bits, "{}", row.name);
            assert_eq!(cert.width.rust(), row.width, "{}", row.name);
            assert_eq!(cert.f64_exact, row.exact_f64, "{}", row.name);
            assert_eq!(cert.unit_bound.to_string(), row.b.text(), "{}", row.name);
            assert_eq!(cert.unit_error.to_string(), row.e.text(), "{}", row.name);
        }
        assert_eq!(g.result, c.result);
        let mut constant = c.rows[c.result].e.clone();
        for _ in 0..g.roundings {
            constant = constant.mul(&Poly(vec![Big::small(1), Big::small(2)]));
        }
        assert_eq!(g.constant.to_string(), constant.text(), "{}", p.generator);
        assert_eq!(g.constant_bits, constant.f64_at_least(), "{}", p.generator);
    }
}

/// The checker's own arithmetic, on cases worked by hand: orient2d's unit error is 4u + 2u^2
/// (two products, each u, then the difference's rounding u · (2 + 2u)), and rounding it up gives
/// the next double above 4u.
#[test]
fn the_checkers_arithmetic_on_hand_worked_cases() {
    let d = derive_again(predicate("orient2d").text).unwrap();
    assert_eq!(d.rows[d.result].e.text(), "4u + 2u^2");
    assert_eq!(
        d.rows[d.result].bound.decimal(),
        "9671406556899441211605000"
    );
    assert_eq!(d.rows[0].width, "i64");
    assert_eq!(
        Poly::u().f64_at_least(),
        (1.0f64 / 9_007_199_254_740_992.0).to_bits()
    );
    let four_u = Poly(vec![Big::small(0), Big::small(4)]);
    assert_eq!(
        four_u.f64_at_least(),
        (4.0f64 / 9_007_199_254_740_992.0).to_bits()
    );
    let above = Poly(vec![Big::small(0), Big::small(4), Big::small(2)]);
    assert_eq!(above.f64_at_least(), four_u.f64_at_least() + 1);
    assert_eq!(Big::pow2(100).decimal(), "1267650600228229401496703205376");
    let (set, k) = scale_expr(
        "m_x * m_y * larger(m_x * m_x, m_y * m_y)",
        &["x".to_owned(), "y".to_owned()],
    )
    .unwrap();
    assert_eq!(set, BTreeSet::from([vec![3, 1], vec![1, 3]]));
    // m_x · m_y rounds once, the larger square once, and their product once: the operands'
    // roundings multiply, so three, not the two on the longest path.
    assert_eq!(k, 3);
}

/// A description whose `let` takes a coordinate's name is refused by the checker's own
/// derivation, which names values by what they compute, and a module whose result is called
/// `scale` is reported, not a panic.
#[test]
fn the_checker_names_values_by_what_they_compute_and_reports() {
    let collide = "predicate p\ninput a b: point2\nlet a_x = b.x * b.y\nlet v = a.x * a_x - a.y\n\
                   sign v\n";
    let Err(e) = derive_again(collide) else {
        panic!("the checker derived a description whose let takes a coordinate's name");
    };
    assert!(e.contains("two values are named `a_x`"), "{e}");
    let module = COMMITTED[0].1;
    let renamed = module
        .replace("let det = ", "let scale = ")
        .replace("decide(det,", "decide(scale,")
        .replace("Some(det)", "Some(scale)");
    let bad = verify(predicate("orient2d").text, &renamed);
    assert!(
        !bad.is_empty(),
        "a module whose result is called `scale` was accepted"
    );
    println!("{}", bad.join("\n"));
}

/// One hand edit of a committed module: the text to replace, its replacement, and what it
/// pretends.
fn edits(module: &str) -> Vec<(String, String, &'static str)> {
    let mut out = Vec::new();
    let code = module
        .lines()
        .find(|l| l.starts_with("const ERROR: f64 = f64::from_bits(0x"))
        .unwrap();
    // A constant one ulp smaller: a hand-tightened bound.
    let hex: String = code
        .trim_start_matches("const ERROR: f64 = f64::from_bits(0x")
        .trim_end_matches(");")
        .replace('_', "");
    let bits = u64::from_str_radix(&hex, 16).unwrap() - 1;
    let smaller = format!("const ERROR: f64 = f64::from_bits({});", literal(bits));
    out.push((code.to_owned(), smaller, "a constant one ulp smaller"));
    // A certificate row claiming one bit fewer.
    let row = module.lines().rfind(|l| l.starts_with("//! | `")).unwrap();
    let mut c = cells(row);
    let first_row = module.lines().find(|l| l.starts_with("//! | `")).unwrap();
    let mut first = cells(first_row);
    let bits: u32 = first[3].parse().unwrap();
    first[3] = (bits - 1).to_string();
    out.push((
        first_row.to_owned(),
        format!("//! | {} |", first.join(" | ")),
        "a certificate row with one bit fewer",
    ));
    // An analysis row with a smaller unit error.
    "u".clone_into(&mut c[4]);
    out.push((
        row.to_owned(),
        format!("//! | {} |", c.join(" | ")),
        "a unit error made smaller",
    ));
    // A filter statement changed: a sum where the description has a product or a difference.
    let stmt = module
        .lines()
        .find(|l| {
            l.starts_with("    let ")
                && (l.contains(" - ") || l.contains(" * "))
                && !l.contains("exact_f64")
                && !l.contains("scale")
        })
        .unwrap();
    out.push((
        stmt.to_owned(),
        stmt.replace(" - ", " + ").replace(" * ", " + "),
        "a filter statement",
    ));
    // An exact-path step put in a narrower type.
    if let Some(wide) = module.lines().find(|l| l.contains("I256::from_i128(")) {
        out.push((
            wide.to_owned(),
            wide.replace("I256::from_i128(", "i128::from("),
            "an exact step narrowed",
        ));
    }
    // Two parameters swapped in `exact`'s signature, and its body calling `value` in another
    // order; a signature or a call laid out over several lines has its parameters, or its
    // arguments, on the lines after its first.
    let lines: Vec<&str> = module.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("pub fn exact("))
        .unwrap();
    let sig = lines[at];
    if sig == "pub fn exact(" {
        let (first, second) = (lines[at + 1], lines[at + 2]);
        out.push((
            format!("{first}\n{second}\n"),
            format!("{second}\n{first}\n"),
            "parameters swapped",
        ));
    } else {
        let inside = sig
            .trim_start_matches("pub fn exact(")
            .split(") ->")
            .next()
            .unwrap();
        let params: Vec<&str> = inside.split(", ").collect();
        let mut swapped = params.clone();
        swapped.swap(0, 1);
        out.push((
            sig.to_owned(),
            sig.replace(inside, &swapped.join(", ")),
            "parameters swapped",
        ));
    }
    let at = lines
        .iter()
        .position(|l| l.starts_with("    value("))
        .unwrap();
    let (call, args) = if lines[at] == "    value(" {
        let inner = lines[at + 1];
        (inner, inner.trim().trim_end_matches(','))
    } else {
        let call = lines[at];
        let args = call
            .trim_start_matches("    value(")
            .split(')')
            .next()
            .unwrap();
        (call, args)
    };
    let names: Vec<&str> = args.split(", ").collect();
    let mut reordered = names.clone();
    reordered.swap(0, 1);
    out.push((
        call.to_owned(),
        call.replace(args, &reordered.join(", ")),
        "exact calls value in another order",
    ));
    // A term of a scale computed over several lines dropped: the first line's second term.
    if let Some(l) = lines
        .iter()
        .find(|l| l.starts_with("    let scale = larger(") && !l.contains("(scale"))
        && lines
            .iter()
            .filter(|l| l.starts_with("    let scale = "))
            .count()
            > 1
    {
        let args = l
            .trim_start_matches("    let scale = larger(")
            .trim_end_matches(");");
        let (left, _) = args.split_once(", ").unwrap();
        out.push((
            (*l).to_owned(),
            format!("    let scale = larger({left}, {left});"),
            "a scale term dropped",
        ));
    }
    out
}

/// One hand edit of the plain-`i128` variant each (piece G42): a narrower type than the variant
/// holds, an unchecked product, a step dropped and a range check skipped.
fn plain_edits() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "    let ba_x = i128::from(b.x.get()).checked_sub(i128::from(a.x.get()))?;",
            "    let ba_x = b.x.get().checked_sub(a.x.get())?;",
            "a translation held in i64",
        ),
        (
            "    let det_1 = ba_x.checked_mul(ca_y)?;",
            "    let det_1 = ba_x.wrapping_mul(ca_y);",
            "a product that wraps",
        ),
        (
            "    let det_2 = ba_y.checked_mul(ca_x)?;\n",
            "",
            "a step dropped",
        ),
        (
            "    let nx = within(nx, 83)?;",
            "    let nx = Some(nx)?;",
            "a rounded input's range check skipped",
        ),
    ]
}

/// One hand edit of the limb variant each (piece G42): a product in too few limbs, an unchecked
/// product, a proof over narrower operands than the code's, and a proof dropped.
fn limbs_edits() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "    let det_1 = Wide32::<3>::product_i64(ba_x, ca_y)?;",
            "    let det_1 = Wide32::<2>::product_i64(ba_x, ca_y)?;",
            "a product in two limbs",
        ),
        (
            "    let det_2 = Wide32::<3>::product_i64(ba_y, ca_x)?;",
            "    let det_2 = Wide32::<3>::from_i64(ba_y.wrapping_mul(ca_x));",
            "a product that wraps",
        ),
        (
            "        let (l, r) = (small(41), small(41));",
            "        let (l, r) = (small(40), small(41));",
            "a proof over a narrower operand",
        ),
        (
            "        assert!(v.to_i128().is_some());\n",
            "",
            "a proof's assertion dropped",
        ),
    ]
}

/// One hand edit of the parent module, each breaking what the proof relies on.
fn parent_edits() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "    let above = value > bound;",
            "    let above = value >= bound;",
            "a sign on the bound itself",
        ),
        (
            "    let below = value < -bound;",
            "    let below = value <= -bound;",
            "a sign on the negated bound itself",
        ),
        (
            "values.iter().map(|v| v.abs())",
            "values.iter().map(|v| *v)",
            "maxima of signed values",
        ),
        (
            "    if a < b { b } else { a }",
            "    if a < b { a } else { b }",
            "the smaller, not the larger",
        ),
        (
            "(above | below | (scale == 0.0))",
            "(above | below | (scale <= 1.0))",
            "zero for a scale that is not zero",
        ),
        (
            "    if v.unsigned_abs() <= limit {",
            "    if v.unsigned_abs() <= limit.saturating_mul(2) {",
            "a range twice the declared one",
        ),
    ]
}

/// A hand-edited constant, certificate row, unit error, filter statement, exact-path type,
/// parameter order or helper in a committed module, or a hand-edited line of either variant, is
/// refused by the checker, whatever produced it.
#[test]
fn the_checker_refuses_hand_edited_modules() {
    for (name, module) in COMMITTED {
        let p = predicate(name);
        for (from, to, what) in edits(module) {
            let edited = module.replacen(&from, &to, 1);
            assert_ne!(edited, module, "{name}: {what}: the edit changed nothing");
            let bad = verify(p.text, &edited);
            assert!(!bad.is_empty(), "{name}: {what}: the checker accepted it");
        }
    }
    for (from, to, what) in limbs_edits() {
        let edited = LIMBS.replacen(from, to, 1);
        assert_ne!(edited, LIMBS, "limbs.rs: {what}: the edit changed nothing");
        assert!(
            !verify_limbs(&edited).is_empty(),
            "limbs.rs: {what}: the checker accepted it"
        );
    }
    for (from, to, what) in plain_edits() {
        let edited = PLAIN.replacen(from, to, 1);
        assert_ne!(edited, PLAIN, "plain.rs: {what}: the edit changed nothing");
        assert!(
            !verify_plain(&edited).is_empty(),
            "plain.rs: {what}: the checker accepted it"
        );
    }
    for (from, to, what) in parent_edits() {
        let edited = PARENT.replacen(from, to, 1);
        assert_ne!(edited, PARENT, "mod.rs: {what}: the edit changed nothing");
        assert!(
            !verify_parent(&edited).is_empty(),
            "mod.rs: {what}: the checker accepted it"
        );
    }
}

/// A scratch folder, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Scratch {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("geom-gen-{label}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// CI's `cargo run -p geom-gen -- check`, on a scratch copy of the workspace's outputs, fails
/// with exit code 1 on each hand edit of a generated module or of the parent module, naming the
/// file, and passes again once the file is restored.
#[cfg_attr(
    target_family = "wasm",
    ignore = "uses the host's file system and runs the binary"
)]
#[test]
fn check_fails_on_a_hand_edited_constant_or_certificate() {
    let s = Scratch::new("predicate-tamper");
    let write = std::process::Command::new(env!("CARGO_BIN_EXE_geom-gen"))
        .args(["write", "--root"])
        .arg(&s.0)
        .output()
        .unwrap();
    assert_eq!(write.status.code(), Some(0));
    let check_fails_naming = |file: &str, what: &str| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_geom-gen"))
            .args(["check", "--root"])
            .arg(&s.0)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{file}: {what}");
        assert!(stderr.contains(file), "{stderr}");
        assert!(stderr.contains("DIFFERS"), "{stderr}");
    };
    for p in &PREDICATES {
        let path = s.0.join(p.output);
        let original = std::fs::read_to_string(&path).unwrap();
        for (from, to, what) in edits(&original) {
            std::fs::write(&path, original.replacen(&from, &to, 1)).unwrap();
            check_fails_naming(p.output, what);
        }
        std::fs::write(&path, &original).unwrap();
    }
    let parent = s.0.join(geom_gen::predicates::MODULE_OUTPUT);
    let original = std::fs::read_to_string(&parent).unwrap();
    for (from, to, what) in parent_edits() {
        std::fs::write(&parent, original.replacen(from, to, 1)).unwrap();
        check_fails_naming(geom_gen::predicates::MODULE_OUTPUT, what);
    }
    std::fs::write(&parent, &original).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_geom-gen"))
        .args(["check", "--root"])
        .arg(&s.0)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
}
