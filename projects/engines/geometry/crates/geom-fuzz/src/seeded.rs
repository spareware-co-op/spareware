//! The seeded leg: a harness run on byte strings drawn from a printed seed.
//!
//! What it does: [`run`] resolves a seed with `geom_test::seed::resolve` (the test's default, or
//! `SPAREKIT_SEED`; printed as `seed: <hex>`), then for each case draws a length in `0..=max_len`
//! and that many bytes from `geom-test`'s `SplitMix64`, and runs the harness on them.
//!
//! Guarantees:
//!
//! - **The same cases every run.** One seed gives the same byte strings on every target, so a pull
//!   request runs a fixed set of cases (28.17) and any failure replays exactly with the printed
//!   seed.
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
//! seeds" on every pull request, "fresh, logged seeds" nightly), over the byte strings a fuzzer
//! would give, so the one harness function serves fuzzing and fixed-seed testing alike, the
//! arrangement bolero's own test mode uses (<https://github.com/camshaft/bolero>).
//!
//! Cost: `cases` runs of the harness, plus one `SplitMix64` word per eight bytes drawn and one or
//! two words per length.

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
    run_cases(label, seed, cases, max_len, harness)
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
        bytes.clear();
        bytes.resize(len, 0);
        rng.fill_bytes(&mut bytes);
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
    fn hex_is_two_lowercase_digits_a_byte() {
        assert_eq!(hex(&[0x00, 0x0f, 0xab, 0xff]), "000fabff");
        assert_eq!(hex(&[]), "");
    }
}
