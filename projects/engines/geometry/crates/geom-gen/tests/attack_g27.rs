//! Attacks on piece G27 (issue #186): the predicate generator, its description language, the
//! filters and exact paths it emits into `geom-num`, and the routing of the explicit-point
//! predicates through them.
//!
//! Each test fails if the piece is wrong in the way its name says. The emitted filters are run by
//! a small interpreter of the emitted Rust (`Program`), which evaluates every `let` in order with
//! Rust's shadowing rule and the `decide` of the generated `mod.rs`; it is first checked against
//! the compiled filters, so a disagreement elsewhere is the generator's, not the interpreter's.
//!
//! - `the_interpreter_runs_the_emitted_filters_as_compiled`: a guard for the interpreter.
//! - `a_result_named_like_a_filter_local_keeps_the_filters_sign`: a description whose result is
//!   named `m_x`, `m_y` or `scale` (names the emitted filter binds itself) must be refused, or its
//!   filter must give the same signs as the same determinant under another name.
//! - `a_coordinate_is_never_read_from_a_let_of_the_same_name`: `a.x` must be the coordinate, even
//!   when an earlier `let` is called `a_x`.
//! - `rust_keywords_and_reserved_names_are_refused`: a name that makes the emitted module fail to
//!   compile must be refused by the language, because `geom-gen` cannot run once `geom-num` does
//!   not build.
//! - `each_module_header_counts_the_bounds_multiplications`: the cost line of each generated
//!   module.
//! - `orient2d_on_exact_zeros_reports_its_time_against_before`: a guard that the public body
//!   before this piece and the public function now both answer `Zero` on exactly collinear
//!   points, and a printed wall-clock comparison of the two, which cannot fail; release builds
//!   only. Principle 12's 5% on these inputs is judged on counts, by `geom-bench`'s
//!   `predicate/orient2d/degenerate` in `kernel-bench.yml` (the test's documentation). Its
//!   baseline is the public body before this piece, behind the same calls as the public function
//!   now (the fixer replaced a replica that already had the new, cheaper `i64` differences).
//! - `orient2d_filter_is_exact_just_inside_and_just_outside_its_bound`: chapter 28.6's inputs
//!   placed at the bound, a guard.

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
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]

use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

use geom_gen::predicate_derive::{Derivation, derive};
use geom_gen::predicate_emit::predicate_module;
use geom_gen::predicate_lang::{Op, StepKind};
use geom_gen::predicates::{
    generate_incircle, generate_insphere, generate_orient2d, generate_orient3d,
};
use geom_num::generated::{incircle, insphere, orient2d, orient3d};
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::{NumError, Sign, predicate};
use geom_test::{Seed, SplitMix64};

const M: i64 = COORD_MAX;

// ---------------------------------------------------------------------------------------------
// Random points.
// ---------------------------------------------------------------------------------------------

fn rng(label: &str, seed: u64) -> SplitMix64 {
    let seed = Seed(seed);
    println!("{label}: seed: {seed}");
    SplitMix64::new(seed)
}

/// Uniform in lo..=hi.
fn range(r: &mut SplitMix64, lo: i64, hi: i64) -> i64 {
    let span = (hi - lo) as u64 + 1;
    lo + (r.next_u64() % span) as i64
}

/// A coordinate anywhere on the grid, at an edge, or within 2^k of zero for a random k.
fn coord(r: &mut SplitMix64) -> i64 {
    match r.next_u64() % 6 {
        0 => M,
        1 => -M,
        2 | 3 => {
            let k = range(r, 0, 40);
            range(r, -(1 << k) + 1, (1 << k) - 1).clamp(-M, M)
        }
        _ => range(r, -M, M),
    }
}

fn cases(release: usize) -> usize {
    if cfg!(debug_assertions) {
        release / 20
    } else {
        release
    }
}

// ---------------------------------------------------------------------------------------------
// An interpreter of an emitted filter.
// ---------------------------------------------------------------------------------------------

fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        let in_number = ch == '.' && cur.starts_with(|c: char| c.is_ascii_digit());
        if ch.is_ascii_alphanumeric() || ch == '_' || in_number {
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

/// The lines of the body of `pub fn <name>(`, up to its closing brace.
fn body<'a>(module: &'a str, name: &str) -> Vec<&'a str> {
    let start = format!("pub fn {name}(");
    let mut lines = module.lines().skip_while(|l| !l.starts_with(&start));
    lines.next().unwrap_or_else(|| panic!("no fn {name}"));
    lines.take_while(|l| *l != "}").collect()
}

/// An emitted `filter`, parsed once: its `let`s in order, its `decide` and its constant.
struct Program {
    error: f64,
    lets: Vec<(String, Vec<String>)>,
    decide: [String; 3],
}

impl Program {
    fn parse(module: &str) -> Program {
        let line = module
            .lines()
            .find(|l| l.starts_with("const ERROR: f64 = f64::from_bits(0x"))
            .expect("an ERROR constant");
        let hex = line
            .trim_start_matches("const ERROR: f64 = f64::from_bits(0x")
            .trim_end_matches(");")
            .replace('_', "");
        let error = f64::from_bits(u64::from_str_radix(&hex, 16).unwrap());
        let mut lets = Vec::new();
        let mut decide = None;
        for l in body(module, "filter") {
            let l = l.trim();
            if let Some(rest) = l.strip_prefix("let ") {
                let (name, expr) = rest.split_once(" = ").unwrap();
                lets.push((name.to_owned(), tokens(expr.strip_suffix(';').unwrap())));
            } else if let Some(rest) = l.strip_prefix("decide(") {
                let args: Vec<String> = rest
                    .strip_suffix(')')
                    .unwrap()
                    .split(", ")
                    .map(str::to_owned)
                    .collect();
                decide = Some([args[0].clone(), args[1].clone(), args[2].clone()]);
            } else {
                panic!("a line the interpreter does not know: {l}");
            }
        }
        Program {
            error,
            lets,
            decide: decide.expect("a decide"),
        }
    }

    /// The filter's answer on these points (by input name), with Rust's rules: statements in
    /// order, a later `let` of a name shadowing an earlier one.
    fn run(&self, points: &BTreeMap<&str, Vec<i64>>) -> Option<Sign> {
        let mut env: BTreeMap<String, f64> = BTreeMap::new();
        for (name, toks) in &self.lets {
            let mut e = Eval {
                toks,
                at: 0,
                env: &env,
                points,
            };
            let v = e.expr();
            assert_eq!(e.at, toks.len(), "{name}: {toks:?}");
            env.insert(name.clone(), v);
        }
        assert_eq!(self.decide[1], "ERROR");
        decide(env[&self.decide[0]], self.error, env[&self.decide[2]])
    }
}

/// `decide` as the generated `mod.rs` writes it.
fn decide(value: f64, error: f64, scale: f64) -> Option<Sign> {
    let bound = error * scale;
    if value > bound {
        Some(Sign::Positive)
    } else if value < -bound {
        Some(Sign::Negative)
    } else if scale == 0.0 {
        Some(Sign::Zero)
    } else {
        None
    }
}

struct Eval<'a> {
    toks: &'a [String],
    at: usize,
    env: &'a BTreeMap<String, f64>,
    points: &'a BTreeMap<&'a str, Vec<i64>>,
}

impl Eval<'_> {
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.at).map(String::as_str)
    }

    fn take(&mut self) -> String {
        let t = self.toks[self.at].clone();
        self.at += 1;
        t
    }

    fn expect(&mut self, t: &str) {
        let got = self.take();
        assert_eq!(got, t);
    }

    fn expr(&mut self) -> f64 {
        let mut v = self.term();
        loop {
            match self.peek() {
                Some("+") => {
                    self.at += 1;
                    v += self.term();
                }
                Some("-") => {
                    self.at += 1;
                    v -= self.term();
                }
                _ => return v,
            }
        }
    }

    fn term(&mut self) -> f64 {
        let mut v = self.atom();
        while self.peek() == Some("*") {
            self.at += 1;
            v *= self.atom();
        }
        v
    }

    fn atom(&mut self) -> f64 {
        let t = self.take();
        match t.as_str() {
            "(" => {
                let v = self.expr();
                self.expect(")");
                v
            }
            "exact_f64" => {
                self.expect("(");
                let point = self.take();
                self.expect(".");
                let axis = match self.take().as_str() {
                    "x" => 0,
                    "y" => 1,
                    "z" => 2,
                    other => panic!("axis {other}"),
                };
                self.expect(")");
                self.points[point.as_str()][axis] as f64
            }
            "largest_magnitude" => {
                self.expect("(");
                self.expect("[");
                let mut m = 0.0f64;
                loop {
                    let v = self.expr().abs();
                    if m < v {
                        m = v;
                    }
                    match self.take().as_str() {
                        "," => {}
                        "]" => break,
                        other => panic!("in a list: {other}"),
                    }
                }
                self.expect(")");
                m
            }
            "larger" => {
                self.expect("(");
                let a = self.expr();
                self.expect(",");
                let b = self.expr();
                self.expect(")");
                if a < b { b } else { a }
            }
            n if n.starts_with(|c: char| c.is_ascii_digit()) => n.parse().unwrap(),
            name => *self
                .env
                .get(name)
                .unwrap_or_else(|| panic!("`{name}` is not bound")),
        }
    }
}

fn pts<const D: usize, const N: usize>(
    names: [&str; N],
    p: [[i64; D]; N],
) -> BTreeMap<&str, Vec<i64>> {
    names.into_iter().zip(p.map(|q| q.to_vec())).collect()
}

fn p2([x, y]: [i64; 2]) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: [i64; 3]) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// Guard: the interpreter, run on the generator's own output for the four predicates, gives
/// exactly what the compiled filters in `geom-num` give.
#[test]
fn the_interpreter_runs_the_emitted_filters_as_compiled() {
    let o2 = Program::parse(&String::from_utf8(generate_orient2d().unwrap()).unwrap());
    let o3 = Program::parse(&String::from_utf8(generate_orient3d().unwrap()).unwrap());
    let ic = Program::parse(&String::from_utf8(generate_incircle().unwrap()).unwrap());
    let is = Program::parse(&String::from_utf8(generate_insphere().unwrap()).unwrap());
    let mut r = rng("interpreter", 0x6727_a77a_c000_0001);
    for _ in 0..cases(20_000) {
        let q2: [[i64; 2]; 4] = std::array::from_fn(|_| [coord(&mut r), coord(&mut r)]);
        let q3: [[i64; 3]; 5] =
            std::array::from_fn(|_| [coord(&mut r), coord(&mut r), coord(&mut r)]);
        let [a, b, c, d] = q2.map(p2);
        assert_eq!(
            o2.run(&pts(["a", "b", "c"], [q2[0], q2[1], q2[2]])),
            orient2d::filter(a, b, c),
            "{q2:?}"
        );
        assert_eq!(
            ic.run(&pts(["a", "b", "c", "d"], q2)),
            incircle::filter(a, b, c, d),
            "{q2:?}"
        );
        let [a, b, c, d, e] = q3.map(p3);
        assert_eq!(
            o3.run(&pts(["a", "b", "c", "d"], [q3[0], q3[1], q3[2], q3[3]])),
            orient3d::filter(a, b, c, d),
            "{q3:?}"
        );
        assert_eq!(
            is.run(&pts(["a", "b", "c", "d", "e"], q3)),
            insphere::filter(a, b, c, d, e),
            "{q3:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The emitted filter's own names.
// ---------------------------------------------------------------------------------------------

/// orient2d's description with its result called `name`.
fn orient2d_named(name: &str) -> String {
    format!(
        "predicate orient2d\ninput a b c: point2\nlet ba_x = b.x - a.x\nlet ba_y = b.y - a.y\n\
         let ca_x = c.x - a.x\nlet ca_y = c.y - a.y\nlet {name} = ba_x * ca_y - ba_y * ca_x\n\
         sign {name}\n"
    )
}

/// The emitted filter binds `m_<group>` and `scale` after every step and then calls
/// `decide(<result>, ERROR, scale)`. A result called `m_x`, `m_y` or `scale` is shadowed by the
/// filter's own `let` before `decide` reads it, so the filter decides on a group maximum or on the
/// scale instead of the determinant: `Positive` for every input whose scale is not zero. Neither
/// the language nor the emitter refuses these names, and the independent checker in
/// `tests/predicate_checker.rs` accepts the `m_x` module (its statement count and its `decide` line
/// both match). The description is orient2d's own, so the right answers are the compiled filter's.
#[test]
fn a_result_named_like_a_filter_local_keeps_the_filters_sign() {
    let clockwise = [[0, 0], [0, 10], [10, 0]];
    let counter = [[0, 0], [10, 0], [0, 10]];
    let far = [[-M, -M], [M, M - 1], [M, -M]];
    let mut wrong = Vec::new();
    for name in ["m_x", "m_y", "scale"] {
        let Ok(d) = derive(&orient2d_named(name)) else {
            continue;
        };
        let Ok(module) = predicate_module(&d, "predicate-attack", "ATTACK") else {
            continue;
        };
        let program = Program::parse(&module);
        for q in [clockwise, counter, far] {
            let [a, b, c] = q.map(p2);
            let got = program.run(&pts(["a", "b", "c"], q));
            let want = orient2d::filter(a, b, c);
            if got != want {
                wrong.push(format!(
                    "result `{name}`: the emitted filter gives {got:?} on {q:?}, the determinant's \
                     sign is {want:?}"
                ));
            }
        }
        // The same fact, read off the text: the result is bound twice in `filter`.
        let bound_twice = body(&module, "filter")
            .iter()
            .filter(|l| l.starts_with(&format!("    let {name} = ")))
            .count();
        if bound_twice > 1 {
            wrong.push(format!(
                "result `{name}` is bound {bound_twice} times in `filter`"
            ));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

// ---------------------------------------------------------------------------------------------
// Coordinates and lets with the same name.
// ---------------------------------------------------------------------------------------------

/// The exact value of a derivation's result, step by step in `i128` (every value here is small).
fn derived_value(d: &Derivation, points: &BTreeMap<&str, Vec<i64>>) -> i128 {
    let mut v: Vec<i128> = Vec::new();
    for s in &d.steps {
        let x = match &s.kind {
            StepKind::LoadCoord { point, axis } => {
                i128::from(points[point.as_str()][*axis as usize])
            }
            StepKind::Translate {
                minuend,
                subtrahend,
                axis,
            } => {
                i128::from(points[minuend.as_str()][*axis as usize])
                    - i128::from(points[subtrahend.as_str()][*axis as usize])
            }
            StepKind::LoadRounded { .. } => panic!("no rounded inputs here"),
            StepKind::Op { op, left, right } => match op {
                Op::Add => v[*left] + v[*right],
                Op::Sub => v[*left] - v[*right],
                Op::Mul => v[*left] * v[*right],
            },
        };
        v.push(x);
    }
    v[d.result]
}

/// The flattener loads a coordinate `p.x` as the step named `p_x`, and reuses any earlier step of
/// that name (`Flattener::load`), whatever it computes. A `let a_x = …` before the first use of
/// `a.x` is such a step, so `a.x` then reads the `let`, not the coordinate: the derivation, the
/// certificate, the filter and the exact path all compute another polynomial than the one written.
/// In the first case `a.y` is still read, so the emitted module compiles without even an unused
/// parameter warning. The name check in `Description::steps` only catches the other order. The
/// checker's `Walker::leaf` dedups by name the same way, so it agrees.
#[test]
fn a_coordinate_is_never_read_from_a_let_of_the_same_name() {
    let cases = [
        (
            "predicate collide\ninput a b: point2\nlet a_x = b.x * b.y\nlet v = a.x * a_x - a.y\n\
             sign v\n",
            "a.x · (b.x · b.y) − a.y",
        ),
        (
            "predicate collide\ninput a b c: point2\nlet a_x = b.x - c.x\nlet v = a.x * a_x\n\
             sign v\n",
            "a.x · (b.x − c.x)",
        ),
    ];
    let points = pts(["a", "b", "c"], [[-5, 1], [3, 11], [7, 2]]);
    let truth = |i: usize| -> i128 {
        let [a, b, c] = [&points["a"], &points["b"], &points["c"]];
        if i == 0 {
            i128::from(a[0] * b[0] * b[1] - a[1])
        } else {
            i128::from(a[0] * (b[0] - c[0]))
        }
    };
    let mut wrong = Vec::new();
    for (i, (text, formula)) in cases.iter().enumerate() {
        let Ok(d) = derive(text) else {
            continue;
        };
        let got = derived_value(&d, &points);
        if got != truth(i) {
            wrong.push(format!(
                "`{formula}` at a = (-5, 1), b = (3, 11), c = (7, 2) is {}, the derivation \
                 computes {got} (steps: {:?})",
                truth(i),
                d.steps.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()
            ));
        }
        if let Ok(module) = predicate_module(&d, "predicate-attack", "ATTACK") {
            let filter = body(&module, "filter").join("\n");
            if !filter.contains("exact_f64(a.x)") {
                wrong.push(format!(
                    "`{formula}`: the emitted filter never reads a.x:\n{filter}"
                ));
            }
            let program = Program::parse(&module);
            let got = program.run(&points);
            let want = Some(Sign::of_i128(truth(i)));
            if got != want {
                wrong.push(format!(
                    "`{formula}`: the emitted filter gives {got:?}, the exact sign is {want:?}"
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

// ---------------------------------------------------------------------------------------------
// Names the emitted Rust cannot hold.
// ---------------------------------------------------------------------------------------------

/// The language's `is_name` refuses only its own seven keywords. A Rust keyword (`type`, `match`,
/// `loop`, `fn`, `ref`, `self`) as a `let` or predicate name gives `let type = …;` or
/// `pub mod type;`, and `ERROR`, `None` or `Some` as a `let` give a refutable pattern: in each case
/// the emitter writes a module that does not compile. Then `geom-num` does not build, and since
/// `geom-gen` depends on `geom-num`, `cargo run -p geom-gen -- write` cannot run to repair it
/// (the bootstrap caveat in the builder's notes). Such names must be refused before anything is
/// written.
#[test]
fn rust_keywords_and_reserved_names_are_refused() {
    let mut accepted = Vec::new();
    for name in [
        "type", "match", "loop", "fn", "ref", "self", "ERROR", "None", "Some",
    ] {
        let text = format!(
            "predicate p\ninput a b: point2\nlet {name} = a.x * b.y - a.y * b.x\nsign {name}\n"
        );
        let emitted =
            derive(&text).is_ok_and(|d| predicate_module(&d, "predicate-attack", "ATTACK").is_ok());
        if emitted {
            accepted.push(format!("a `let` named `{name}`"));
        }
    }
    for name in ["type", "fn", "mod"] {
        let text =
            format!("predicate {name}\ninput a b: point2\nlet v = a.x * b.y - a.y * b.x\nsign v\n");
        let emitted =
            derive(&text).is_ok_and(|d| predicate_module(&d, "predicate-attack", "ATTACK").is_ok());
        if emitted {
            accepted.push(format!("a predicate named `{name}`"));
        }
    }
    assert!(
        accepted.is_empty(),
        "emitted although the module cannot compile: {}",
        accepted.join(", ")
    );
}

// ---------------------------------------------------------------------------------------------
// The modules' cost lines.
// ---------------------------------------------------------------------------------------------

const COMMITTED: [(&str, &str); 4] = [
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
];

/// Each module's header said "the bound's k multiplication(s)", with k the roundings
/// (`Derivation::roundings`), not the number of multiplications. incircle's bound
/// `m_x * m_y * larger(m_y * m_y, m_x * m_x)` times `ERROR` is five multiplications, not three;
/// insphere's is seven, not four. A module header must state its cost correctly.
#[test]
fn each_module_header_counts_the_bounds_multiplications() {
    let mut wrong = Vec::new();
    for (name, module) in COMMITTED {
        let doc: Vec<&str> = module
            .lines()
            .filter_map(|l| l.strip_prefix("//!"))
            .map(str::trim)
            .collect();
        let doc = doc.join(" ");
        let at = doc.find("and the bound's ").expect("a cost line") + "and the bound's ".len();
        let stated: usize = doc[at..]
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let scale = module
            .lines()
            .find(|l| l.starts_with("    let scale = "))
            .unwrap();
        let actual = scale.matches('*').count() + 1;
        if stated != actual {
            wrong.push(format!(
                "{name}: the header says {stated} multiplication(s); `{}` and `ERROR * scale` are \
                 {actual}",
                scale.trim()
            ));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

// ---------------------------------------------------------------------------------------------
// Principle 12: no speed regression over 5%, judged on counts in geom-bench; reported here.
// ---------------------------------------------------------------------------------------------

/// `predicate::orient2d` before this piece, as a caller in another crate reached it: a call to a
/// function that is not inlined (`old_public_orient2d`).
#[inline(never)]
fn before_orient2d(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
    old_public_orient2d(a, b, c)
}

/// The public body before this piece, the same operations as `orient2d`, `orient2d_fast`, `diff`
/// and `det2` in predicate.rs at commit f3ba674: differences taken in `i128` (`diff`), and the
/// cross product with `i128::checked_mul` on those (`det2`), whose overflow check the compiler
/// cannot remove. The total path behind it, which no grid input reaches, is the error it would
/// report.
///
/// (Fixer's note: the first version of this test replicated the old body with differences in
/// `i64` widened afterwards, which is the new generated exact path, not the old one, and so
/// measured the new code against itself plus a call; the old body is restored here.)
#[inline(never)]
fn old_public_orient2d(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
    let [a, b, c] = [a, b, c].map(|p| [p.x.get(), p.y.get()]);
    let fast = old_orient2d_fast(a, b, c).map(Sign::of_i128);
    fast.ok_or(NumError::BudgetExceeded)
}

/// `orient2d_fast` before this piece: `D2::sub(b, a)?.cross(D2::sub(c, a)?)`.
fn old_orient2d_fast(a: [i64; 2], b: [i64; 2], c: [i64; 2]) -> Option<i128> {
    let diff = |p: i64, q: i64| i128::from(p).checked_sub(i128::from(q));
    let det2 =
        |a: i128, b: i128, c: i128, d: i128| a.checked_mul(d)?.checked_sub(b.checked_mul(c)?);
    let (bx, by) = (diff(b[0], a[0])?, diff(b[1], a[1])?);
    let (cx, cy) = (diff(c[0], a[0])?, diff(c[1], a[1])?);
    det2(bx, by, cx, cy)
}

#[inline(never)]
fn now_orient2d(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
    predicate::orient2d(a, b, c)
}

fn time(inputs: &[[P2; 3]], f: fn(P2, P2, P2) -> Result<Sign, NumError>) -> f64 {
    let start = Instant::now();
    let mut acc = 0i64;
    for &[a, b, c] in inputs {
        acc += i64::from(f(black_box(a), black_box(b), black_box(c)).unwrap().as_i8());
    }
    black_box(acc);
    start.elapsed().as_nanos() as f64 / inputs.len() as f64
}

/// G01 section 3, principle 12: "speed regressions over 5% block a merge". Chapter 23.2: in a
/// building many predicate calls are exactly zero, and a filter can never certify a zero, so on
/// such calls the filter is pure overhead. The builder's own benchmark measured the public
/// orient2d at about 1.55 to 1.65 times its old cost on degenerate data.
///
/// Where the 5% is judged: on counts, never here. `geom-bench`'s `predicate/orient2d/degenerate`
/// runs the public orient2d on exactly collinear points (its `tests/inputs.rs` checks every answer
/// is `Zero`), and `kernel-bench.yml`'s count gates fail a pull request that raises it by more than
/// 5% against its merge base: instructions under Callgrind on Linux x86-64 and aarch64, where the
/// route skips the filter, and WebAssembly fuel on wasm32, where the filter runs first (chapters
/// 23.13 and 28.14). This test only prints a wall-clock comparison with the public body before
/// this piece, on points of one lattice line (every value exactly zero), each route timed in turn
/// in every round and keeping its best round, both behind the same two calls a caller in another
/// crate makes. It cannot fail on time: at about 4 ns a call, a shared runner's noise moves the
/// ratio by more than 5% (it read 1.26 on an unchanged route, issue #371). It fails only if either
/// route answers anything but `Zero`. Release builds only: debug timings mean nothing.
#[cfg_attr(debug_assertions, ignore = "a timing: release builds only")]
#[test]
fn orient2d_on_exact_zeros_reports_its_time_against_before() {
    let mut r = rng("orient2d exact zeros", 0x6727_a77a_c000_0002);
    let inputs: Vec<[P2; 3]> = (0..1 << 14)
        .map(|_| {
            let a = [
                range(&mut r, -(1 << 39), 1 << 39),
                range(&mut r, -(1 << 39), 1 << 39),
            ];
            let u = [
                range(&mut r, -(1 << 19), 1 << 19),
                range(&mut r, -(1 << 19), 1 << 19),
            ];
            let at = |k: i64| [a[0] + k * u[0], a[1] + k * u[1]];
            let (j, k) = (
                range(&mut r, -(1 << 19), 1 << 19),
                range(&mut r, -(1 << 19), 1 << 19),
            );
            [a, at(j), at(k)].map(p2)
        })
        .collect();
    for &[a, b, c] in &inputs {
        assert_eq!(predicate::orient2d(a, b, c), Ok(Sign::Zero));
        assert_eq!(before_orient2d(a, b, c), Ok(Sign::Zero));
    }
    let (mut before, mut now) = (f64::INFINITY, f64::INFINITY);
    for _ in 0..60 {
        let t = time(&inputs, before_orient2d);
        if t < before {
            before = t;
        }
        let t = time(&inputs, now_orient2d);
        if t < now {
            now = t;
        }
    }
    println!(
        "orient2d on exact zeros: before {before:.2} ns, now {now:.2} ns, ratio {:.3} (a report: \
         geom-bench's predicate/orient2d/degenerate holds the 5% on counts)",
        now / before
    );
}

// ---------------------------------------------------------------------------------------------
// Chapter 28.6: inputs just inside and just outside the bound.
// ---------------------------------------------------------------------------------------------

/// (g, x, y) with a·x + b·y = g = gcd(a, b).
fn egcd(a: i128, b: i128) -> (i128, i128, i128) {
    if b == 0 {
        (a, 1, 0)
    } else {
        let (g, x, y) = egcd(b, a % b);
        (g, y, x - (a / b) * y)
    }
}

/// Guard, and chapter 28.6's "placed just inside and just outside the f64 filter's error bound":
/// a, b and c with det[b − a; c − a] = t exactly, where |t| is between a quarter of and twice the
/// filter's bound and b − a spans up to 2^39 on each axis, shifted towards the grid's edges. Every
/// sign the filter gives is t's, and both outcomes occur in the band.
#[test]
fn orient2d_filter_is_exact_just_inside_and_just_outside_its_bound() {
    let mut r = rng("orient2d at its bound", 0x6727_a77a_c000_0003);
    let error = f64::from_bits(
        derive(geom_gen::predicates::ORIENT2D)
            .unwrap()
            .constant_bits,
    );
    let (mut certified, mut left) = (0u64, 0u64);
    let mut done = 0;
    while done < cases(400_000) {
        let bx = range(&mut r, 1 << 37, 1 << 39) * if r.next_u64() & 1 == 0 { 1 } else { -1 };
        let by = range(&mut r, 1 << 37, 1 << 39) * if r.next_u64() & 1 == 0 { 1 } else { -1 };
        let (g, x, y) = egcd(i128::from(bx), i128::from(by));
        if g.abs() != 1 {
            continue;
        }
        // bx·s − by·q = 1.
        let (s, q) = (x * g, -y * g);
        assert_eq!(i128::from(bx) * s - i128::from(by) * q, 1);
        let scale = (bx.unsigned_abs() as f64) * (by.unsigned_abs() as f64);
        let bound = error * scale;
        let f = 0.25 + 1.75 * ((r.next_u64() >> 11) as f64 / (1u64 << 53) as f64);
        let t = ((bound * f) as i128) * if r.next_u64() & 1 == 0 { 1 } else { -1 };
        // c − a = (q·t, s·t) + k·(bx, by), with k putting c − a's x in 0..|bx|.
        let bxi = i128::from(bx);
        let cx = (q * t).rem_euclid(bxi);
        let cy = (t + i128::from(by) * cx) / bxi;
        assert_eq!(bxi * cy - i128::from(by) * cx, t);
        let (cx, cy) = (cx as i64, cy as i64);
        // a near −(b − a)/2, then shifted as far towards an edge as the grid allows.
        let room = M - (1 << 38) - (1 << 39) - 2;
        let shift = [range(&mut r, -room, room), range(&mut r, -room, room)];
        let a = [-bx / 2 + shift[0], -by / 2 + shift[1]];
        let b = [a[0] + bx, a[1] + by];
        let c = [a[0] + cx, a[1] + cy];
        if ![a, b, c].iter().flatten().all(|v| (-M..=M).contains(v)) {
            continue;
        }
        let [pa, pb, pc] = [a, b, c].map(p2);
        assert_eq!(orient2d::value(pa, pb, pc), Some(t), "{a:?} {b:?} {c:?}");
        match orient2d::filter(pa, pb, pc) {
            Some(sign) => {
                assert_eq!(sign, Sign::of_i128(t), "{a:?} {b:?} {c:?}: t = {t}");
                certified += 1;
            }
            None => left += 1,
        }
        assert_eq!(predicate::orient2d(pa, pb, pc), Ok(Sign::of_i128(t)));
        done += 1;
    }
    println!("at the bound: certified {certified}, left {left}");
    assert!(certified > 0 && left > 0);
}
