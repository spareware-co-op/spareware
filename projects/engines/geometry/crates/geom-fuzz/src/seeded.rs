//! The seeded leg: a harness run on byte strings drawn from a printed seed, half of them hostile.
//!
//! What it does: [`run`] resolves a seed with `geom_test::seed::resolve` (the test's default, or
//! `SPAREKIT_SEED`; printed as `seed: <hex>`), then for each case draws a length in `0..=max_len`
//! and that many bytes from `geom-test`'s `SplitMix64`, and runs the harness on them. Every second
//! case is hostile instead ([`Hostile`], in turn): words at the ends of their domains
//! ([`HOSTILE_WORDS`] and solidcore's [`HOSTILE_F64`]: NaN, infinities, zeros, subnormals, huge and
//! tiny values), a run of bytes repeated (so a decoder reads the same value, point or record again:
//! coincident points, zero lengths and areas), the same with one bit flipped in every second copy
//! (near-coincident points, a sliver), evenly spaced words (collinear and coplanar points, a
//! value at a domain's end after a step), and one byte throughout (all zero, all ones, the sign
//! boundaries). Each starts with a few random bytes, so the words fall on every alignment of the
//! harness's own fields, and is cut to the case's length, so every hostile kind also comes short.
//! The test's count is sized for an optimised build (about two seconds, at most ten for the
//! slowest harnesses); an unoptimised build runs the first tenth of the cases ([`DEBUG_SHARE`]),
//! the same cases, so the debug legs take about as long.
//!
//! Guarantees:
//!
//! - **The same cases every run.** One seed gives the same byte strings on every target, so a pull
//!   request runs a fixed set of cases (28.17) and any failure replays exactly with the printed
//!   seed.
//! - **Every hostile kind is drawn.** The kinds take the hostile cases in turn, so any run of ten
//!   cases or more holds every kind.
//! - **A failure says how to keep it.** [`Failure::Finding`] carries the case number, the seed,
//!   the bytes in hexadecimal and the broken law. Saved as a file in the target's
//!   `tests/__fuzz__/<target>/crashes` folder, the bytes become a case bolero's test mode replays on
//!   every run (28.16).
//! - **Panics are not caught.** A harness that panics fails the test, and the seed line printed
//!   before the first case shows which seed to replay.
//! - **Every seed is printed**, by [`run`] as `geom_test::seed::resolve` prints it and by
//!   [`run_with_seed`] as `geom_test::seed::fixed` does, before the first case.
//!
//! Method: the fixed-seed property runs of section 28.17 of the kernel architecture
//! (`projects/engines/geometry/architecture/28-testing.md`: "Unit and property tests with fixed
//! seeds" on every pull request), over the byte strings a fuzzer would give, so the one harness
//! function serves fixed-seed testing and Kani alike, the arrangement bolero's own test mode uses
//! (<https://github.com/camshaft/bolero>); the hostile share is solidcore's panic fuzz, seeded
//! random cases mixed with a fixed list of hostile values, sized to seconds and run in the normal
//! suite.
//!
//! Cost: `cases` runs of the harness, plus one `SplitMix64` word per eight bytes drawn and a few
//! words per case.
//!
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/a3a8244566081e0777284dc182f77a91bb1a456d/crates/solidcore/tests/support/panic_fuzz.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: its `HOSTILE` list of `f64` values as [`HOSTILE_F64`], and its way of
//!   mixing hostile values into seeded random cases, applied to a harness's bytes rather than to
//!   typed inputs; the integer words, the repeats, the progressions and the fills are ours.
//! SPDX-License-Identifier: Apache-2.0

use core::fmt;
use core::num::NonZeroU64;

use geom_test::seed::{SeedError, fixed, resolve};
use geom_test::{Seed, SplitMix64};

use crate::{Finding, Harness};

/// Why a seeded run failed.
#[derive(Clone, PartialEq, Eq)]
pub enum Failure {
    /// `SPAREKIT_SEED` was set but is not a seed.
    Seed(SeedError),
    /// The harness found a broken law.
    Finding {
        /// The run's label.
        label: String,
        /// The run's seed.
        seed: Seed,
        /// The failing case's number, from 0.
        case: u32,
        /// The failing case's bytes, in lowercase hexadecimal.
        bytes: String,
        /// What the harness found.
        finding: Finding,
    },
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Seed(e) => write!(f, "{e}"),
            Failure::Finding {
                label,
                seed,
                case,
                bytes,
                finding,
            } => write!(
                f,
                "{label}: case {case} failed with seed: {seed}\n{finding}\n\
                 input bytes (hex): {bytes}\nreplay with SPAREKIT_SEED={seed}"
            ),
        }
    }
}

/// The same text as `Display`, so a test that returns `Result<(), Failure>` shows the whole report.
impl fmt::Debug for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for Failure {}

/// Runs `harness` on `cases` byte strings, each of length `0..=max_len`, drawn from the seed
/// `resolve` gives for `label` and `default`.
///
/// # Errors
///
/// [`Failure::Seed`] if `SPAREKIT_SEED` is malformed; [`Failure::Finding`] for the first
/// case the harness rejects.
pub fn run(
    label: &str,
    default: Seed,
    cases: u32,
    max_len: u16,
    harness: Harness,
) -> Result<(), Failure> {
    let seed = resolve(label, default).map_err(Failure::Seed)?;
    run_cases(label, seed, for_this_build(cases), max_len, harness)
}

/// An unoptimised build runs one case in this many of a test's count: its first cases.
pub const DEBUG_SHARE: u32 = 10;

/// The cases [`run`] runs of `cases`, sized for an optimised build: all of them, or in an
/// unoptimised build (`opt-level` 0, `cfg(unoptimised)`, which `build.rs` sets) the first
/// [`DEBUG_SHARE`]th (at least one, when `cases` is not zero). An optimised build with debug
/// assertions on, as the pull requests' test run is (the owner's testing principle 3), runs them
/// all.
#[must_use]
pub fn for_this_build(cases: u32) -> u32 {
    if cfg!(unoptimised) {
        cases
            .checked_div(DEBUG_SHARE)
            .unwrap_or(0)
            .max(cases.min(1))
    } else {
        cases
    }
}

/// The hostile `f64` values: NaN, infinities, zeros, negatives, subnormals, huge and tiny values,
/// as solidcore's panic fuzz lists them (the header's `Source:`).
pub const HOSTILE_F64: [f64; 21] = [
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    0.0,
    -0.0,
    -1.0,
    -1e-3,
    5e-324,
    1e-310,
    f64::MIN_POSITIVE,
    1e-300,
    1e-200,
    1e-15,
    1e-9,
    f64::EPSILON,
    1e15,
    1e100,
    1e300,
    -1e300,
    f64::MAX,
    f64::MIN,
];

/// Integer words at the ends of their domains: zero and one, all ones (−1), the ends of `i64`
/// and their neighbours, the grid's ±2^40 and one inside it, 2^53 (where `f64` stops holding every
/// integer), and the ends of 32 bits.
pub const HOSTILE_WORDS: [u64; 15] = [
    0,
    1,
    u64::MAX,
    0x8000_0000_0000_0000,
    0x8000_0000_0000_0001,
    0x7FFF_FFFF_FFFF_FFFF,
    0x7FFF_FFFF_FFFF_FFFE,
    0x4000_0000_0000_0000,
    0x0000_0100_0000_0000,
    0x0000_00FF_FFFF_FFFF,
    0xFFFF_FF00_0000_0000,
    0x0020_0000_0000_0000,
    0x0000_0000_7FFF_FFFF,
    0x0000_0000_8000_0000,
    0x0000_0000_FFFF_FFFF,
];

/// The bytes a hostile fill repeats: zero, all ones, and each side of a byte's sign.
pub const FILL_BYTES: [u8; 5] = [0x00, 0xFF, 0x7F, 0x80, 0x01];

/// The longest run a repeated case repeats.
pub const MAX_PERIOD: u64 = 48;

/// The most random bytes before a hostile case's pattern, so its words fall on every alignment.
pub const MAX_PREFIX: u64 = 8;

/// The kinds of hostile case, which take the hostile cases in turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hostile {
    /// Words of [`HOSTILE_WORDS`] and [`HOSTILE_F64`], eight bytes each, little-endian.
    Extremes,
    /// A run of random bytes repeated.
    Repeats,
    /// A run repeated with its last byte's low bit flipped in every second copy.
    NearRepeats,
    /// Words of one, two, four or eight bytes, each a step on from the last.
    Progression,
    /// One byte of [`FILL_BYTES`] throughout.
    Fill,
}

/// Every kind, in the order they take the hostile cases.
pub const HOSTILE_KINDS: [Hostile; 5] = [
    Hostile::Extremes,
    Hostile::Repeats,
    Hostile::NearRepeats,
    Hostile::Progression,
    Hostile::Fill,
];

/// The kind of case `case`: `None` for a random one (every even case), else the hostile kind
/// whose turn it is.
#[must_use]
pub fn kind_of(case: u32) -> Option<Hostile> {
    if case & 1 == 0 {
        return None;
    }
    let turn = usize::try_from(case >> 1).unwrap_or(0);
    HOSTILE_KINDS
        .get(turn.checked_rem(HOSTILE_KINDS.len()).unwrap_or(0))
        .copied()
}

/// One of `items`, drawn from `rng` (`items` is never empty here).
fn pick<T: Copy + Default>(rng: &mut SplitMix64, items: &[T]) -> T {
    let n = NonZeroU64::new(u64::try_from(items.len()).unwrap_or(0)).unwrap_or(NonZeroU64::MIN);
    usize::try_from(rng.below(n))
        .ok()
        .and_then(|i| items.get(i))
        .copied()
        .unwrap_or_default()
}

/// A number in `0..=max`.
fn upto(rng: &mut SplitMix64, max: u64) -> u64 {
    rng.below(NonZeroU64::new(max.saturating_add(1)).unwrap_or(NonZeroU64::MAX))
}

/// A hostile word: one of [`HOSTILE_WORDS`] or of [`HOSTILE_F64`]'s bits.
fn hostile_word(rng: &mut SplitMix64) -> u64 {
    if rng.next_u64() & 1 == 0 {
        pick(rng, &HOSTILE_WORDS)
    } else {
        pick(rng, &HOSTILE_F64.map(f64::to_bits))
    }
}

/// Writes a hostile case of `kind` and length `len` into `bytes`, from `rng`.
pub fn hostile(kind: Hostile, rng: &mut SplitMix64, len: usize, bytes: &mut Vec<u8>) {
    bytes.clear();
    let prefix = usize::try_from(upto(rng, MAX_PREFIX)).unwrap_or(0);
    bytes.resize(prefix, 0);
    rng.fill_bytes(bytes);
    match kind {
        Hostile::Extremes => {
            while bytes.len() < len {
                bytes.extend(hostile_word(rng).to_le_bytes());
            }
        }
        Hostile::Repeats | Hostile::NearRepeats => {
            let period = usize::try_from(upto(rng, MAX_PERIOD.saturating_sub(1)))
                .unwrap_or(0)
                .saturating_add(1);
            let mut run = vec![0; period];
            rng.fill_bytes(&mut run);
            let mut copy = 0_u32;
            while bytes.len() < len {
                if kind == Hostile::NearRepeats && copy & 1 == 1 {
                    let mut near = run.clone();
                    if let Some(last) = near.last_mut() {
                        *last ^= 1;
                    }
                    bytes.extend(near);
                } else {
                    bytes.extend(run.iter().copied());
                }
                copy = copy.wrapping_add(1);
            }
        }
        Hostile::Progression => {
            let width = pick(rng, &[1_usize, 2, 4, 8]);
            let start = hostile_word(rng);
            let random = rng.next_u64();
            let step = pick(rng, &[0, 1, 2, u64::MAX, 1 << 20, random]);
            let mut value = start;
            while bytes.len() < len {
                bytes.extend(value.to_le_bytes().into_iter().take(width));
                value = value.wrapping_add(step);
            }
        }
        Hostile::Fill => {
            let byte = pick(rng, &FILL_BYTES);
            bytes.clear();
            bytes.resize(len, byte);
        }
    }
    bytes.truncate(len);
}

/// [`run`] from exactly `seed`, ignoring `SPAREKIT_SEED`, with the seed printed by
/// `geom_test::seed::fixed`: for replaying one seed from a tool, and for tests whose assertions
/// depend on the seed.
///
/// # Errors
///
/// [`Failure::Finding`] for the first case the harness rejects.
pub fn run_with_seed(
    label: &str,
    seed: Seed,
    cases: u32,
    max_len: u16,
    harness: Harness,
) -> Result<(), Failure> {
    run_cases(label, fixed(label, seed), cases, max_len, harness)
}

/// The cases of [`run`] and [`run_with_seed`], from a seed already printed.
fn run_cases(
    label: &str,
    seed: Seed,
    cases: u32,
    max_len: u16,
    harness: Harness,
) -> Result<(), Failure> {
    let mut rng = SplitMix64::new(seed);
    // max_len + 1 is at least 1 and at most 65,536, so `new` cannot return `None`.
    let lengths = NonZeroU64::new(u64::from(max_len) + 1).unwrap_or(NonZeroU64::MIN);
    let mut bytes = Vec::with_capacity(usize::from(max_len));
    for case in 0..cases {
        // At most `max_len`, which is a u16, so the conversion cannot fail.
        let len = usize::try_from(rng.below(lengths)).unwrap_or(0);
        if let Some(kind) = kind_of(case) {
            hostile(kind, &mut rng, len, &mut bytes);
        } else {
            bytes.clear();
            bytes.resize(len, 0);
            rng.fill_bytes(&mut bytes);
        }
        if let Err(finding) = harness(&bytes) {
            return Err(Failure::Finding {
                label: label.to_owned(),
                seed,
                case,
                bytes: hex(&bytes),
                finding,
            });
        }
    }
    Ok(())
}

/// Bytes in lowercase hexadecimal, two digits each.
fn hex(bytes: &[u8]) -> String {
    format!("{}", Hex(bytes))
}

/// Bytes shown in lowercase hexadecimal, two digits each, written straight into the formatter.
struct Hex<'a>(&'a [u8]);

impl fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
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

    /// Rejects any input whose first byte is 0x2a.
    fn picky(data: &[u8]) -> Result<(), Finding> {
        match data.first() {
            Some(0x2a) => Err(Finding {
                law: "first byte is not 42".into(),
                detail: String::new(),
            }),
            _ => Ok(()),
        }
    }

    #[test]
    fn a_failure_reports_case_seed_and_bytes() {
        let r = run_with_seed("picky", Seed(1), 100_000, 4, picky);
        let Err(Failure::Finding {
            label,
            case,
            bytes,
            finding,
            ..
        }) = r
        else {
            panic!("the picky harness must fail on some case: {r:?}");
        };
        assert_eq!(label, "picky");
        assert!(bytes.starts_with("2a"), "{bytes}");
        assert!(bytes.len() <= 8);
        assert_eq!(finding.law, "first byte is not 42");
        // The same seed fails on the same case with the same bytes.
        let again = run_with_seed("picky", Seed(1), case + 1, 4, picky);
        let Err(Failure::Finding {
            case: case2,
            bytes: bytes2,
            ..
        }) = again
        else {
            panic!("{again:?}");
        };
        assert_eq!((case, &bytes), (case2, &bytes2));
    }

    #[test]
    fn lengths_cover_zero_to_max() {
        let mut seen = [false; 5];
        let record = |data: &[u8]| -> Result<(), Finding> {
            // Lengths are reported through a failure on the one length not yet seen.
            Err(Finding {
                law: format!("len {}", data.len()),
                detail: String::new(),
            })
        };
        for case_count in 1..200 {
            if let Err(Failure::Finding { finding, .. }) =
                run_with_seed("lengths", Seed(case_count), 1, 4, record)
            {
                let n: usize = finding.law[4..].parse().unwrap();
                seen[n] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "{seen:?}");
    }

    #[test]
    fn every_second_case_is_hostile_and_every_kind_takes_its_turn() {
        let kinds: Vec<Option<Hostile>> = (0..10).map(kind_of).collect();
        assert!(kinds.iter().step_by(2).all(Option::is_none));
        let hostile: Vec<Hostile> = kinds.iter().skip(1).step_by(2).flatten().copied().collect();
        assert_eq!(hostile, HOSTILE_KINDS);
        assert_eq!(for_this_build(0), 0);
        if cfg!(unoptimised) {
            assert_eq!((for_this_build(5), for_this_build(1_000)), (1, 100));
        } else {
            assert_eq!(for_this_build(1_000), 1_000);
        }
    }

    #[test]
    fn each_hostile_kind_has_its_shape() {
        let mut rng = SplitMix64::new(Seed(7));
        let mut bytes = Vec::new();
        for len in [0, 1, 9, 100, 256] {
            for kind in HOSTILE_KINDS {
                hostile(kind, &mut rng, len, &mut bytes);
                assert_eq!(bytes.len(), len, "{kind:?}");
            }
        }
        let words: Vec<u64> = HOSTILE_WORDS
            .iter()
            .copied()
            .chain(HOSTILE_F64.map(f64::to_bits))
            .collect();
        for _ in 0..50 {
            hostile(Hostile::Extremes, &mut rng, 200, &mut bytes);
            // Some alignment of eight bytes after the prefix reads hostile words only.
            let aligned = (0..=MAX_PREFIX as usize).any(|start| {
                bytes[start..]
                    .chunks_exact(8)
                    .all(|w| words.contains(&u64::from_le_bytes(w.try_into().unwrap())))
            });
            assert!(aligned, "{bytes:02x?}");
            hostile(Hostile::Fill, &mut rng, 64, &mut bytes);
            assert!(FILL_BYTES.contains(&bytes[0]) && bytes.iter().all(|b| *b == bytes[0]));
            hostile(Hostile::Repeats, &mut rng, 200, &mut bytes);
            let repeats = (1..=MAX_PERIOD as usize).any(|p| {
                (0..=MAX_PREFIX as usize).any(|s| (s + p..200).all(|i| bytes[i] == bytes[i - p]))
            });
            assert!(repeats, "{bytes:02x?}");
            hostile(Hostile::NearRepeats, &mut rng, 200, &mut bytes);
            let near = (1..=MAX_PERIOD as usize).any(|p| {
                (0..=MAX_PREFIX as usize).any(|s| {
                    (s + 2 * p..200).all(|i| bytes[i] == bytes[i - 2 * p])
                        && (s + p..200).all(|i| (bytes[i] ^ bytes[i - p]) & !1 == 0)
                })
            });
            assert!(near, "{bytes:02x?}");
        }
    }

    #[test]
    fn hex_is_two_lowercase_digits_a_byte() {
        assert_eq!(hex(&[0x00, 0x0f, 0xab, 0xff]), "000fabff");
        assert_eq!(hex(&[]), "");
    }
}
