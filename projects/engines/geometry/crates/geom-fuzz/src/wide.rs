//! Fuzz target: `geom-num`'s wide integers agree with a big-integer oracle on every operation.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a width, an operation and two
//! operands of that width (and, for `resize`, a second width). [`check`] runs the operation on
//! `geom_num::wide::Wide` and on `num-bigint`'s `BigInt`, an arbitrary-precision oracle that
//! shares no code with it, and compares: the exact result when it fits the width and `None`
//! exactly when it does not, for `checked_add`, `checked_sub`, `checked_neg` and `checked_mul`;
//! the order (`Ord`, `==`); the sign (`signum`, `is_negative`, `is_zero`); `resize` to every
//! other width; and the `i128` conversions both ways. [`harness`] is `decode` then `check`, the
//! function every fuzzing engine drives (the crate README sets out the layout).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//!   Each limb is read through a rule byte that makes the values at every width's limits common:
//!   0, 1, −1, the top bit alone, all bits but the top one, a copy of the limb below, or a word
//!   read from the input. So the most negative and most positive values, their neighbours, and
//!   carries that run the whole length come up all the time, not once in 2^64.
//! - **Every width the kernel uses, and the two reserved ones.** `I128` as `Wide<2>`, `I256`,
//!   `I320`, `I384`, `I512`, and `Wide<9>` and `Wide<12>`, the 576- and 768-bit widths chapter 6.2
//!   reserves for later predicates (the board's G26; whether they are named is an owner's decision,
//!   G240), through the same generic code.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a
//!   unit test builds the input that reaches it.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the operation, the width,
//!   the operands and both answers.
//!
//! Method: differential testing against an independent arbitrary-precision implementation, the
//! big-integer oracle chapter 29.3 names for the wide integers
//! (`projects/engines/geometry/architecture/29-sources-licences.md`), here `num-bigint` 0.4.8
//! (<https://docs.rs/num-bigint/0.4.8/num_bigint/struct.BigInt.html>). Two's complement in `L`
//! limbs holds exactly −2^(64L−1) ..= 2^(64L−1) − 1 (the module header of `geom_num::wide`); the
//! oracle reads a `Wide`'s limbs with `BigInt::from_signed_bytes_le`, the same two's-complement
//! reading, computes exactly, and writes a result that fits back with `to_signed_bytes_le`.
//!
//! Cost: one operation on `Wide` and one on `BigInt` per input, at most O(L²) for a product of
//! L ≤ 12 limbs, plus up to seven `resize` targets; at most [`MAX_INPUT_BYTES`] bytes read.

use core::cmp::Ordering;
use core::fmt::Debug;

use geom_num::Sign;
use geom_num::wide::Wide;
use geom_test::{ByteSource, Entropy};
use num_bigint::BigInt;

use crate::Finding;

/// The widths a case may have, in limbs of 64 bits: `I128`, `I256`, `I320`, `I384`, `I512`, and
/// the reserved 576 and 768 bits.
pub const WIDTHS: [usize; 7] = [2, 4, 5, 6, 8, 9, 12];

/// The widest, in limbs.
const MAX_LIMBS: usize = 12;

/// The most bytes [`decode`] reads from any input: 219.
///
/// Three header bytes (the width, the operation and `resize`'s target width), then two operands
/// of at most [`MAX_LIMBS`] limbs, each limb a rule byte and, for a word read from the input, eight
/// bytes more: 3 + 2 · 12 · (1 + 8) = 219. The unit test `the_input_bound_is_reached_and_never_passed`
/// builds that input and checks it reads exactly 219.
pub const MAX_INPUT_BYTES: usize = 219;

// The derivation above, checked when the crate compiles.
const _: () = assert!(MAX_INPUT_BYTES == 3 + 2 * MAX_LIMBS * (1 + 8));

/// An operation the harness checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// `checked_add`.
    Add,
    /// `checked_sub`.
    Sub,
    /// `checked_neg` of the first operand.
    Neg,
    /// `checked_mul`.
    Mul,
    /// `Ord` and `==`.
    Compare,
    /// `signum`, `is_negative` and `is_zero` of the first operand.
    Sign,
    /// `resize` of the first operand to the case's second width.
    Resize,
    /// `from_i128` of the first operand's low 128 bits, and `to_i128` of the first operand.
    I128,
}

/// The operations, in the order a header byte picks them.
const OPS: [Op; 8] = [
    Op::Add,
    Op::Sub,
    Op::Neg,
    Op::Mul,
    Op::Compare,
    Op::Sign,
    Op::Resize,
    Op::I128,
];

/// One case: the width, the operation, `resize`'s target width, and the operands' limbs, least
/// significant first, each exactly `limbs` long.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The width, in limbs: one of [`WIDTHS`].
    pub limbs: usize,
    /// The operation.
    pub op: Op,
    /// `resize`'s target width, in limbs: one of [`WIDTHS`].
    pub target: usize,
    /// The first operand's limbs.
    pub a: Vec<u64>,
    /// The second operand's limbs.
    pub b: Vec<u64>,
}

/// One of `items`, picked by a byte (the modulo's bias is irrelevant for a fuzzer's choice).
fn pick<T: Copy>(byte: u8, items: &[T]) -> Option<T> {
    let n = u8::try_from(items.len()).ok()?;
    items.get(usize::from(byte.checked_rem(n)?)).copied()
}

/// An operand of `limbs` limbs, each through its rule byte.
fn operand(src: &mut ByteSource<'_>, limbs: usize) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::with_capacity(limbs);
    for _ in 0..limbs {
        let below = out.last().copied().unwrap_or(0);
        let limb = match src.byte() % 8 {
            0 => 0,
            1 => 1,
            2 => u64::MAX,
            3 => 1 << 63,
            4 => !(1 << 63),
            5 => below,
            _ => src.word(),
        };
        out.push(limb);
    }
    out
}

/// Decodes `data` into a case, with how many of its bytes were read: at most
/// [`MAX_INPUT_BYTES`]. Past the end of `data`, bytes read as zero and are not counted.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let limbs = pick(src.byte(), &WIDTHS).unwrap_or(2);
    let op = pick(src.byte(), &OPS).unwrap_or(Op::Add);
    let target = pick(src.byte(), &WIDTHS).unwrap_or(2);
    let a = operand(&mut src, limbs);
    let b = operand(&mut src, limbs);
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            limbs,
            op,
            target,
            a,
            b,
        },
        read,
    )
}

/// The harness every fuzzing engine drives: decode `data` into a case, then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// Runs the case's operation on `Wide` and on the oracle, and compares.
///
/// # Errors
///
/// A [`Finding`] if they disagree, or if the case is malformed (a width not in [`WIDTHS`], or
/// operands of the wrong length: a defect in this harness).
pub fn check(case: &Case) -> Result<(), Finding> {
    match case.limbs {
        2 => at::<2>(case),
        4 => at::<4>(case),
        5 => at::<5>(case),
        6 => at::<6>(case),
        8 => at::<8>(case),
        9 => at::<9>(case),
        12 => at::<12>(case),
        _ => Err(malformed(case)),
    }
}

/// A finding for a case this harness cannot have made.
fn malformed(case: &Case) -> Finding {
    Finding::new("the harness decodes only well-formed cases", || {
        format!("case: {case:?}")
    })
}

/// The case at width `L`.
fn at<const L: usize>(case: &Case) -> Result<(), Finding> {
    let (Ok(a), Ok(b)) = (
        <[u64; L]>::try_from(case.a.as_slice()),
        <[u64; L]>::try_from(case.b.as_slice()),
    ) else {
        return Err(malformed(case));
    };
    let (a, b) = (Wide::<L>::from_le_limbs(a), Wide::<L>::from_le_limbs(b));
    let (x, y) = (value(&case.a), value(&case.b));
    match case.op {
        // `BigInt`'s checked operations are exact and never return `None`: it grows instead.
        Op::Add => agree::<L>(case, "checked_add", a.checked_add(&b), x.checked_add(&y)),
        Op::Sub => agree::<L>(case, "checked_sub", a.checked_sub(&b), x.checked_sub(&y)),
        Op::Neg => agree::<L>(
            case,
            "checked_neg",
            a.checked_neg(),
            BigInt::ZERO.checked_sub(&x),
        ),
        Op::Mul => agree::<L>(case, "checked_mul", a.checked_mul(&b), x.checked_mul(&y)),
        Op::Compare => {
            let (got, want) = (a.cmp(&b), x.cmp(&y));
            same(case, "Ord::cmp", &got, &want)?;
            same(case, "==", &(a == b), &(want == Ordering::Equal))
        }
        Op::Sign => {
            let want = match x.sign() {
                num_bigint::Sign::Minus => Sign::Negative,
                num_bigint::Sign::NoSign => Sign::Zero,
                num_bigint::Sign::Plus => Sign::Positive,
            };
            same(case, "signum", &a.signum(), &want)?;
            same(
                case,
                "is_negative",
                &a.is_negative(),
                &(want == Sign::Negative),
            )?;
            same(case, "is_zero", &a.is_zero(), &(want == Sign::Zero))
        }
        Op::Resize => match case.target {
            2 => agree::<2>(case, "resize", a.resize::<2>(), Some(x.clone())),
            4 => agree::<4>(case, "resize", a.resize::<4>(), Some(x.clone())),
            5 => agree::<5>(case, "resize", a.resize::<5>(), Some(x.clone())),
            6 => agree::<6>(case, "resize", a.resize::<6>(), Some(x.clone())),
            8 => agree::<8>(case, "resize", a.resize::<8>(), Some(x.clone())),
            9 => agree::<9>(case, "resize", a.resize::<9>(), Some(x.clone())),
            12 => agree::<12>(case, "resize", a.resize::<12>(), Some(x.clone())),
            _ => Err(malformed(case)),
        },
        Op::I128 => {
            let low = i128_of(&case.a);
            agree::<L>(
                case,
                "from_i128",
                Some(Wide::<L>::from_i128(low)),
                Some(BigInt::from(low)),
            )?;
            let want = i128::try_from(&x).ok();
            same(case, "to_i128", &a.to_i128(), &want)
        }
    }
}

/// The value of two's-complement limbs, least significant first.
fn value(limbs: &[u64]) -> BigInt {
    let bytes: Vec<u8> = limbs.iter().flat_map(|limb| limb.to_le_bytes()).collect();
    BigInt::from_signed_bytes_le(&bytes)
}

/// The `i128` whose two's complement is the first two limbs.
fn i128_of(limbs: &[u64]) -> i128 {
    let mut bytes = [0u8; 16];
    for (chunk, limb) in bytes.chunks_mut(8).zip(limbs) {
        chunk.copy_from_slice(&limb.to_le_bytes());
    }
    i128::from_le_bytes(bytes)
}

/// The limbs of `r` in `L` limbs, or `None` if it does not fit: its two's complement, sign-extended.
fn limbs_of<const L: usize>(r: &BigInt) -> Option<[u64; L]> {
    let mut bytes = r.to_signed_bytes_le();
    let fill = if r.sign() == num_bigint::Sign::Minus {
        0xff
    } else {
        0
    };
    let width = L.checked_mul(8)?;
    if bytes.len() > width {
        return None;
    }
    bytes.resize(width, fill);
    let mut out = [0u64; L];
    for (limb, chunk) in out.iter_mut().zip(bytes.chunks_exact(8)) {
        *limb = u64::from_le_bytes(<[u8; 8]>::try_from(chunk).ok()?);
    }
    Some(out)
}

/// Checks a checked operation's answer against the exact result: the result's limbs when it fits
/// `M` limbs, `None` exactly when it does not. `exact` is the oracle's answer, which is never
/// `None` (`BigInt`'s checked operations grow instead of failing); a `None` is a finding against
/// this harness.
fn agree<const M: usize>(
    case: &Case,
    name: &str,
    got: Option<Wide<M>>,
    exact: Option<BigInt>,
) -> Result<(), Finding> {
    let Some(exact) = exact else {
        return Err(malformed(case));
    };
    let want = limbs_of::<M>(&exact);
    same(case, name, &got.map(Wide::to_le_limbs), &want).map_err(|f| Finding {
        law: f.law,
        detail: format!(
            "{}
exact result: {exact}",
            f.detail
        ),
    })
}

/// A finding unless `got` is `want`.
fn same<T: PartialEq + Debug>(case: &Case, name: &str, got: &T, want: &T) -> Result<(), Finding> {
    if got == want {
        Ok(())
    } else {
        Err(Finding::new(
            &format!(
                "Wide<{}>::{name} agrees with exact big-integer arithmetic (num-bigint): the                  result when it fits, None exactly when it does not",
                case.limbs
            ),
            || {
                format!(
                    "case: {case:?}
a = {}
b = {}
got: {got:?}
want: {want:?}",
                    value(&case.a),
                    value(&case.b)
                )
            },
        ))
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;
    use geom_test::seed::fixed;
    use geom_test::{Seed, SplitMix64};

    #[test]
    fn empty_and_tiny_inputs_are_cases() {
        harness(&[]).unwrap();
        harness(&[0]).unwrap();
        harness(&[0xff; 3]).unwrap();
        harness(&[0x80; 400]).unwrap();
        assert_eq!(decode(&[]).1, 0);
    }

    /// The input of [`MAX_INPUT_BYTES`]'s derivation: the widest width (12 limbs, the last of
    /// [`WIDTHS`]), then every limb of both operands read as a word (rule 6).
    fn worst_case() -> Vec<u8> {
        let mut out = vec![6, 0, 0];
        for _ in 0..2 * MAX_LIMBS {
            out.push(6);
            out.extend([0xa5; 8]);
        }
        out
    }

    #[test]
    fn the_input_bound_is_reached_and_never_passed() {
        assert_eq!(WIDTHS.iter().max(), Some(&MAX_LIMBS));
        let worst = worst_case();
        assert_eq!(worst.len(), MAX_INPUT_BYTES);
        assert_eq!(decode(&worst).0.limbs, MAX_LIMBS);
        assert_eq!(decode(&worst).1, MAX_INPUT_BYTES);
        let mut longer = worst.clone();
        longer.extend([0x5a; 64]);
        assert_eq!(decode(&longer).1, MAX_INPUT_BYTES);
        assert_eq!(decode(&longer).0, decode(&worst).0);
        let mut rng = SplitMix64::new(fixed("wide input bound", Seed(0x219)));
        let mut data = vec![0u8; 2 * MAX_INPUT_BYTES];
        for _ in 0..20_000 {
            rng.fill_bytes(&mut data);
            assert!(decode(&data).1 <= MAX_INPUT_BYTES);
        }
    }

    /// The oracle's conversions are exact: limbs to value and back at every width, at the limits.
    #[test]
    fn the_oracle_reads_and_writes_twos_complement() {
        for limbs in WIDTHS {
            let min: Vec<u64> = (0..limbs)
                .map(|i| if i + 1 == limbs { 1 << 63 } else { 0 })
                .collect();
            let max: Vec<u64> = (0..limbs)
                .map(|i| if i + 1 == limbs { !(1 << 63) } else { u64::MAX })
                .collect();
            let bits = 64 * limbs - 1;
            assert_eq!(value(&min), -(BigInt::from(1) << bits));
            assert_eq!(value(&max), (BigInt::from(1) << bits) - 1);
            assert_eq!(value(&vec![u64::MAX; limbs]), BigInt::from(-1));
        }
        assert_eq!(limbs_of::<2>(&BigInt::from(-1)), Some([u64::MAX; 2]));
        assert_eq!(limbs_of::<2>(&(BigInt::from(1) << 127_usize)), None);
        assert_eq!(
            limbs_of::<2>(&-(BigInt::from(1) << 127_usize)),
            Some([0, 1 << 63])
        );
        assert_eq!(limbs_of::<2>(&(-(BigInt::from(1) << 127_usize) - 1)), None);
        assert_eq!(i128_of(&[u64::MAX, u64::MAX, 7]), -1);
    }

    /// The check catches a wide integer that is wrong only at the limit: a check that cannot fail
    /// proves nothing (28.4, fault injection).
    #[test]
    fn the_check_catches_a_wrong_answer() {
        let case = Case {
            limbs: 2,
            op: Op::Add,
            target: 2,
            a: vec![u64::MAX, !(1 << 63)],
            b: vec![1, 0],
        };
        // MAX + 1 overflows: the right answer is None.
        check(&case).unwrap();
        let wrong = agree::<2>(
            &case,
            "checked_add",
            Some(Wide::from_le_limbs([0, 1 << 63])),
            Some(value(&case.a) + value(&case.b)),
        );
        assert!(wrong.is_err());
        let e = wrong.unwrap_err();
        assert!(e.law.contains("Wide<2>::checked_add"), "{e}");
        assert!(e.detail.contains("exact result"), "{e}");
    }
}
