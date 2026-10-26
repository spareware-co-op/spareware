//! Fuzz target: every variant of every generated exact path returns the exact value (chapter 23.3;
//! piece G42).
//!
//! What it does: [`decode`] turns a fuzzer's bytes into the inputs the predicates take: six 2D and
//! five 3D grid points from `geom-test`'s [`GridSampler`], and twelve plane-normal coordinates of
//! 83 bits and four plane offsets of 125 bits, each an extreme, zero, a small value or a value of a
//! random bit length. [`check`] evaluates each registered predicate's description (geom-gen's
//! `PREDICATES`, read by its `predicate_lang::parse`) on those inputs in big integers, and compares
//! the shipped `value`, the plain-`i128` variant (`geom_num::generated::plain`) and the width-aware
//! limb variant (`geom_num::generated::limbs`) with that exact value. [`harness`] is `decode` then
//! `check`, the function every fuzzing engine drives (the crate README sets out the layout).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to inputs inside
//!   every predicate's domain: grid points (magnitude at most 2^40 − 1), aimed at the frame's
//!   extremes and at degenerate configurations, and rounded inputs within their declared bits.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes; longer inputs only add
//!   bytes nobody reads.
//! - **An independent oracle.** The exact value is the description's own arithmetic in
//!   `num-bigint`'s arbitrary-precision integers, read from the description's text, not from the
//!   generated code or the fixed-width integers it uses.
//! - **A wrong or missing value is a [`Finding`]** naming the predicate, the variant, the inputs,
//!   what the variant returned and the exact value. Every input is in the domain, so a `None` is a
//!   finding too.
//!
//! Method: differential testing against an exact oracle, as chapter 23.3 asks of every variant
//! ("the differential fuzzer checks every variant against a rational oracle"),
//! `projects/engines/geometry/architecture/23-performance.md`, with `num-bigint`'s integers; the
//! descriptions' integer values need no rational.
//!
//! Cost: per input, the registered descriptions parsed and evaluated once each in big integers,
//! and three exact-path calls per predicate; at most [`MAX_INPUT_BYTES`] bytes read.

use core::fmt::Debug;

use geom_gen::predicate_lang::{Axis, Expr, InputKind, Op, parse};
use geom_gen::predicates::PREDICATES;
use geom_num::generated::{
    incircle, insphere, limbs, lpi_side, lpi_weight, orient2d, orient3d, plain, plane_side,
    ssi_side, ssi_weight, tpi_side, tpi_weight,
};
use geom_num::grid::{P2, P3};
use geom_num::wide::Wide;
use geom_test::grid::GridSampler;
use geom_test::{ByteSource, Entropy};
use num_bigint::BigInt;

use crate::Finding;

/// The most bytes [`decode`] reads from any input: 620, an upper bound.
///
/// The points read what `geom_fuzz::predicates` derives for the sampler: at most 26 bytes for the
/// first 2D point, 24 for the second and 23 for each later one, so (26 + 24 + 4 · 23) = 142 for
/// six; and at most 37, 35 and 34 for the 3D points, so (37 + 35 + 3 · 34) = 174 for five. A
/// rounded input reads a class byte, a sign byte, and for a random value a length byte and two
/// eight-byte words: at most 19, and 16 · 19 = 304 for the sixteen. 142 + 174 + 304 = 620; a unit
/// test checks that no input reads more.
pub const MAX_INPUT_BYTES: usize = 620;

/// The inputs one byte string decodes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inputs {
    /// The 2D points, taken in order by the descriptions that need them.
    pub p2: [P2; 6],
    /// The 3D points.
    pub p3: [P3; 5],
    /// Plane-normal coordinates, |v| ≤ 2^83 − 1.
    pub normals: [i128; 12],
    /// Plane offsets, |v| ≤ 2^125 − 1.
    pub offsets: [i128; 4],
}

/// The bits of a plane normal's coordinate and of a plane offset, as the descriptions declare
/// them (chapter 23.2).
const NORMAL_BITS: u32 = 83;
const OFFSET_BITS: u32 = 125;

/// A value of magnitude at most 2^bits − 1 (bits ≤ 127): an extreme, zero, a small value or one
/// of a random bit length, each with a sign from the input.
fn rounded(e: &mut dyn Entropy, bits: u32) -> i128 {
    let class = e.byte();
    let negative = e.byte() & 1 == 1;
    let max = 1i128
        .checked_shl(bits)
        .and_then(|m| m.checked_sub(1))
        .unwrap_or(0);
    let magnitude = match class % 8 {
        0..=2 => max,
        3 => 0,
        4 => i128::from(e.byte() % 4),
        _ => {
            let keep = u32::from(e.byte())
                .checked_rem(bits.saturating_add(1))
                .unwrap_or(0);
            let word = (u128::from(e.word()) << 64) | u128::from(e.word());
            let shifted = word.checked_shr(128u32.saturating_sub(keep)).unwrap_or(0);
            i128::try_from(shifted).unwrap_or(0) & max
        }
    };
    // |magnitude| ≤ 2^127 − 1, so its negation always fits.
    if negative {
        magnitude.checked_neg().unwrap_or(magnitude)
    } else {
        magnitude
    }
}

/// Decodes `data` into the harness's inputs, with how many of its bytes were read: at most
/// [`MAX_INPUT_BYTES`]. Past the end of `data`, bytes read as zero and are not counted.
#[must_use]
pub fn decode(data: &[u8]) -> (Inputs, usize) {
    let mut source = ByteSource::new(data);
    let mut sampler = GridSampler::new();
    let p2 = core::array::from_fn(|_| sampler.p2(&mut source));
    let p3 = core::array::from_fn(|_| sampler.p3(&mut source));
    let normals = core::array::from_fn(|_| rounded(&mut source, NORMAL_BITS));
    let offsets = core::array::from_fn(|_| rounded(&mut source, OFFSET_BITS));
    let read = data.len().saturating_sub(source.remaining());
    (
        Inputs {
            p2,
            p3,
            normals,
            offsets,
        },
        read,
    )
}

/// The harness every fuzzing engine drives: decode `data`, then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// One argument of a predicate, bound to an input in the description's order.
#[derive(Clone, Copy, Debug)]
enum Arg {
    P2(P2),
    P3(P3),
    Rounded(i128),
}

/// A value of any exact-path result type, exactly.
trait Exact {
    fn big(&self) -> BigInt;
}

impl Exact for i64 {
    fn big(&self) -> BigInt {
        BigInt::from(*self)
    }
}

impl Exact for i128 {
    fn big(&self) -> BigInt {
        BigInt::from(*self)
    }
}

impl<const L: usize> Exact for Wide<L> {
    fn big(&self) -> BigInt {
        let mut bytes = Vec::with_capacity(L.saturating_mul(8));
        for limb in self.to_le_limbs() {
            bytes.extend_from_slice(&limb.to_le_bytes());
        }
        BigInt::from_signed_bytes_le(&bytes)
    }
}

/// The arguments of one call, read by position.
struct Args(Vec<Arg>);

impl Args {
    fn p2(&self, i: usize) -> Result<P2, Finding> {
        match self.0.get(i) {
            Some(Arg::P2(p)) => Ok(*p),
            other => Err(mismatch(i, other)),
        }
    }

    fn p3(&self, i: usize) -> Result<P3, Finding> {
        match self.0.get(i) {
            Some(Arg::P3(p)) => Ok(*p),
            other => Err(mismatch(i, other)),
        }
    }

    fn r(&self, i: usize) -> Result<i128, Finding> {
        match self.0.get(i) {
            Some(Arg::Rounded(v)) => Ok(*v),
            other => Err(mismatch(i, other)),
        }
    }
}

/// The finding for an argument the harness's call table reads as another kind than the
/// description declares: a defect in this harness, reported rather than hidden.
fn mismatch(i: usize, found: Option<&Arg>) -> Finding {
    Finding {
        law: "the harness calls each predicate with its description's inputs, in order".into(),
        detail: format!("argument {i}: {found:?}"),
    }
}

/// The shipped `value` of module `$m` and both variants' `$v`, each exactly, on the same
/// arguments.
macro_rules! three {
    ($m:ident, $v:ident; $($arg:expr),+ $(,)?) => {
        [
            $m::value($($arg),+).map(|v| v.big()),
            plain::$v($($arg),+).map(|v| v.big()),
            limbs::$v($($arg),+).map(|v| v.big()),
        ]
    };
}

/// The three results of predicate `name` on `a`.
#[allow(
    clippy::many_single_char_names,
    reason = "one-letter names are the arguments' positions, kept short to keep each call on a line"
)]
fn call(name: &str, a: &Args) -> Result<[Option<BigInt>; 3], Finding> {
    let (p, q, r) = (|i| a.p2(i), |i| a.p3(i), |i| a.r(i));
    Ok(match name {
        "orient2d" => three!(orient2d, orient2d_value; p(0)?, p(1)?, p(2)?),
        "orient3d" => three!(orient3d, orient3d_value; q(0)?, q(1)?, q(2)?, q(3)?),
        "incircle" => three!(incircle, incircle_value; p(0)?, p(1)?, p(2)?, p(3)?),
        "insphere" => three!(insphere, insphere_value; q(0)?, q(1)?, q(2)?, q(3)?, q(4)?),
        "plane_side" => three!(plane_side, plane_side_value; r(0)?, r(1)?, r(2)?, r(3)?, q(4)?),
        "tpi_weight" => three!(
            tpi_weight, tpi_weight_value;
            r(0)?, r(1)?, r(2)?, r(3)?, r(4)?, r(5)?, r(6)?, r(7)?, r(8)?
        ),
        "tpi_side" => three!(
            tpi_side, tpi_side_value;
            r(0)?, r(1)?, r(2)?, r(3)?, r(4)?, r(5)?, r(6)?, r(7)?, r(8)?, r(9)?, r(10)?,
            r(11)?, r(12)?, r(13)?, r(14)?, r(15)?
        ),
        "lpi_weight" => three!(lpi_weight, lpi_weight_value; r(0)?, r(1)?, r(2)?, q(3)?, q(4)?),
        "lpi_side" => three!(
            lpi_side, lpi_side_value;
            r(0)?, r(1)?, r(2)?, r(3)?, r(4)?, r(5)?, r(6)?, r(7)?, q(8)?, q(9)?
        ),
        "ssi_weight" => three!(ssi_weight, ssi_weight_value; p(0)?, p(1)?, p(2)?, p(3)?),
        "ssi_side" => three!(ssi_side, ssi_side_value; p(0)?, p(1)?, p(2)?, p(3)?, p(4)?, p(5)?),
        other => {
            return Err(Finding {
                law: "the harness calls every registered predicate".into(),
                detail: format!("{other} is registered but has no call in this harness"),
            });
        }
    })
}

/// The coordinate of an argument named by the description.
fn coordinate(bound: &[(String, Arg)], point: &str, axis: Axis) -> Option<BigInt> {
    let (_, arg) = bound.iter().find(|(n, _)| n == point)?;
    let c = match (arg, axis) {
        (Arg::P2(p), Axis::X) => p.x.get(),
        (Arg::P2(p), Axis::Y) => p.y.get(),
        (Arg::P3(p), Axis::X) => p.x.get(),
        (Arg::P3(p), Axis::Y) => p.y.get(),
        (Arg::P3(p), Axis::Z) => p.z.get(),
        _ => return None,
    };
    Some(BigInt::from(c))
}

/// One frame of the expression walk: an expression to evaluate, or an operator to apply to the
/// two values on top of the value stack.
enum Work<'e> {
    Eval(&'e Expr),
    Apply(Op),
}

/// The exact value of `expr`, with an explicit stack rather than recursion: a rounded input or an
/// earlier `let` by name, a coordinate, or an operation on two values.
fn evaluate(expr: &Expr, bound: &[(String, Arg)], lets: &[(String, BigInt)]) -> Option<BigInt> {
    let mut work = vec![Work::Eval(expr)];
    let mut values: Vec<BigInt> = Vec::new();
    while let Some(item) = work.pop() {
        match item {
            Work::Eval(Expr::Coord(point, axis)) => values.push(coordinate(bound, point, *axis)?),
            Work::Eval(Expr::Name(name)) => {
                let value = match lets.iter().rev().find(|(n, _)| n == name) {
                    Some((_, v)) => v.clone(),
                    None => match bound.iter().find(|(n, _)| n == name) {
                        Some((_, Arg::Rounded(v))) => BigInt::from(*v),
                        _ => return None,
                    },
                };
                values.push(value);
            }
            Work::Eval(Expr::Bin(l, op, r)) => {
                work.push(Work::Apply(*op));
                work.push(Work::Eval(r));
                work.push(Work::Eval(l));
            }
            Work::Apply(op) => {
                let r = values.pop()?;
                let l = values.pop()?;
                // Big integers never overflow: `checked_*` is `num-bigint`'s exact operation.
                values.push(match op {
                    Op::Add => l.checked_add(&r)?,
                    Op::Sub => l.checked_sub(&r)?,
                    Op::Mul => l.checked_mul(&r)?,
                });
            }
        }
    }
    values.pop()
}

/// Evaluates each registered predicate's description on `inputs` exactly, and checks that the
/// shipped exact path and both variants return that value.
///
/// # Errors
///
/// A [`Finding`] for the first variant that returns `None` or another value, or a description
/// this harness cannot bind or evaluate.
pub fn check(inputs: &Inputs) -> Result<(), Finding> {
    for p in &PREDICATES {
        let d = parse(p.text).map_err(|e| Finding {
            law: "every registered description reads".into(),
            detail: format!("{}: {e:?}", p.comparison.name),
        })?;
        // Bind the description's inputs, in order, to the decoded inputs.
        let (mut i2, mut i3, mut normal, mut offset) = (0usize, 0usize, 0usize, 0usize);
        let mut bound: Vec<(String, Arg)> = Vec::new();
        for input in &d.inputs {
            let arg = match input.kind {
                InputKind::Point { dim: 2 } => inputs.p2.get(i2).map(|&p| Arg::P2(p)),
                InputKind::Point { .. } => inputs.p3.get(i3).map(|&p| Arg::P3(p)),
                InputKind::Rounded { bits, .. } if bits == NORMAL_BITS => {
                    inputs.normals.get(normal).map(|&v| Arg::Rounded(v))
                }
                InputKind::Rounded { bits, .. } if bits == OFFSET_BITS => {
                    inputs.offsets.get(offset).map(|&v| Arg::Rounded(v))
                }
                InputKind::Rounded { .. } => None,
            };
            match input.kind {
                InputKind::Point { dim: 2 } => i2 = i2.saturating_add(1),
                InputKind::Point { .. } => i3 = i3.saturating_add(1),
                InputKind::Rounded { bits, .. } if bits == NORMAL_BITS => {
                    normal = normal.saturating_add(1);
                }
                InputKind::Rounded { .. } => offset = offset.saturating_add(1),
            }
            let Some(arg) = arg else {
                return Err(Finding {
                    law: "the harness decodes every input a description declares".into(),
                    detail: format!("{}: input {} ({:?})", d.name, input.name, input.kind),
                });
            };
            bound.push((input.name.clone(), arg));
        }
        // The exact value: every `let`, in order, in big integers.
        let mut lets: Vec<(String, BigInt)> = Vec::new();
        for l in &d.lets {
            let Some(v) = evaluate(&l.expr, &bound, &lets) else {
                return Err(Finding {
                    law: "every description evaluates on its inputs".into(),
                    detail: format!("{}: let {}", d.name, l.name),
                });
            };
            lets.push((l.name.clone(), v));
        }
        let exact = lets
            .iter()
            .rev()
            .find(|(n, _)| *n == d.result)
            .map(|(_, v)| v.clone());
        let args = Args(bound.iter().map(|(_, a)| *a).collect());
        compare(&d.name, &bound, &call(&d.name, &args)?, exact.as_ref())?;
    }
    Ok(())
}

/// Checks that each of the three results is the exact value.
fn compare(
    name: &str,
    bound: &[(String, Arg)],
    got: &[Option<BigInt>; 3],
    exact: Option<&BigInt>,
) -> Result<(), Finding> {
    for (variant, value) in ["shipped value", "plain i128", "limbs"].iter().zip(got) {
        if value.is_none() || value.as_ref() != exact {
            return Err(Finding {
                law: format!(
                    "every exact path returns the description's exact value (chapter 23.3: each \
                     variant equals the big-integer oracle); here {name}'s {variant}"
                ),
                detail: format!("inputs: {bound:?}\nreturned: {value:?}\nexact: {exact:?}"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;
    use geom_test::seed::fixed;
    use geom_test::{Seed, SplitMix64};

    /// No input reads more than the bound, and the empty input decodes and holds.
    #[test]
    fn decoding_reads_at_most_the_bound() {
        let mut g = SplitMix64::new(fixed("variants: decoding", Seed(0x0642_f022_0000_0001)));
        let mut most = 0;
        for _ in 0..20_000 {
            let len = (g.next_u64() % 1_200) as usize;
            let data: Vec<u8> = (0..len).map(|_| g.next_u64() as u8).collect();
            let (_, read) = decode(&data);
            most = most.max(read);
            assert!(read <= MAX_INPUT_BYTES, "read {read} of {len}");
        }
        assert!(most > 300, "the decoder read at most {most} bytes");
        check(&decode(&[]).0).unwrap();
        // Every byte 0xff: every rounded input of a random bit length, the longest reads.
        let (_, read) = decode(&[0xff; 2 * MAX_INPUT_BYTES]);
        assert!(read <= MAX_INPUT_BYTES);
    }

    /// The check catches a variant that is wrong or refuses: a check that cannot fail would prove
    /// nothing (chapter 28.4, fault injection). Each of the three positions is tried.
    #[test]
    fn a_wrong_or_missing_value_is_a_finding() {
        let inputs = decode(&[7; 64]).0;
        check(&inputs).unwrap();
        let d = parse(PREDICATES[0].text).unwrap();
        let bound: Vec<(String, Arg)> = d
            .inputs
            .iter()
            .zip(inputs.p2)
            .map(|(i, p)| (i.name.clone(), Arg::P2(p)))
            .collect();
        let mut lets = Vec::new();
        for l in &d.lets {
            let v = evaluate(&l.expr, &bound, &lets).unwrap();
            lets.push((l.name.clone(), v));
        }
        let exact = lets.last().unwrap().1.clone();
        let args = Args(bound.iter().map(|(_, a)| *a).collect());
        let got = call(&d.name, &args).unwrap();
        compare(&d.name, &bound, &got, Some(&exact)).unwrap();
        for position in 0..3 {
            let mut wrong = got.clone();
            wrong[position] = Some(&exact + 1);
            assert!(compare(&d.name, &bound, &wrong, Some(&exact)).is_err());
            let mut missing = got.clone();
            missing[position] = None;
            assert!(compare(&d.name, &bound, &missing, Some(&exact)).is_err());
        }
    }
}
