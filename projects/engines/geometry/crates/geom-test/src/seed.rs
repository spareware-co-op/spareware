//! Seeds: where every random choice in a test comes from, and how a failure is replayed.
//!
//! What it does: a [`Seed`] is a 64-bit value that decides every input a seeded test draws.
//! [`resolve`] picks a test's seed, the value of the environment variable [`ENV_VAR`]
//! (`SPAREKIT_SEED`) when it is set and the test's fixed default otherwise, and prints it as
//! `seed: <hex>` before the test draws anything. [`fixed`] prints a seed that a test must use
//! whatever the environment says, because its assertions depend on that seed (a golden hash, a
//! recorded case).
//!
//! Guarantees:
//!
//! - **Every seed is printed.** [`resolve`] and [`fixed`] print one line that starts `seed: ` and
//!   is followed by exactly 16 lowercase hexadecimal digits, then the test's label, where the seed
//!   came from and how to replay it. Rust's test harness shows a passing test's output only with
//!   `--nocapture`, and always shows a failing test's, so every failure carries its seed (28.5:
//!   "Every generator prints its seed, so any failure replays exactly").
//! - **Any printed seed replays exactly.** Setting `SPAREKIT_SEED` to the digits [`resolve`]
//!   printed makes it return the same seed, and the generators in [`crate::rng`] then give the same
//!   inputs on every target. A seed [`fixed`] printed replays by itself: the test always uses it.
//! - **A seed is never invented here.** It is the test's fixed default or the value of
//!   `SPAREKIT_SEED`; nothing reads a clock or the operating system's random source. Pull requests
//!   run with the fixed defaults; a nightly job sets `SPAREKIT_SEED` to a fresh value, and the
//!   printed line logs it (28.17).
//! - **A malformed override is an error, never ignored.** A run that asked for one seed and silently
//!   got another would replay the wrong case, so [`resolve`] returns [`SeedError`] instead.
//!
//! The override format: one to sixteen hexadecimal digits, in either case, with an optional `0x` or
//! `0X` prefix and nothing else (no spaces, no separators). Every line [`resolve`] prints is in this
//! format.
//!
//! Method: the replay discipline of sections 28.5 and 28.17 of the kernel architecture
//! (`projects/engines/geometry/architecture/28-testing.md`): "Every generator prints its seed, so
//! any failure replays exactly", and "Seeds belong to test generators only, and every one is
//! printed". The environment variable mirrors the seed variables of proptest
//! (`PROPTEST_RNG_SEED`, <https://github.com/proptest-rs/proptest>) and bolero
//! (`BOLERO_RANDOM_SEED`, <https://github.com/camshaft/bolero>), with one name for every Spare Kit
//! test, so one value replays any of them.
//!
//! Cost: constant time; one environment read per [`resolve`], none for [`fixed`].

use core::fmt;

/// The environment variable that overrides every seeded test's default seed.
pub const ENV_VAR: &str = "SPAREKIT_SEED";

/// A 64-bit seed for test data. Its display form is exactly 16 lowercase hexadecimal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Seed(pub u64);

impl Seed {
    /// The default seed of the kernel's tests: "SPAREKIT" in ASCII, the seed of `geom-num`'s
    /// determinism corpus too.
    pub const DEFAULT: Seed = Seed(0x5350_4152_454b_4954);

    /// The seed as a number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Reads a seed in the override format: one to sixteen hexadecimal digits, in either case, with
    /// an optional `0x` or `0X` prefix and nothing else.
    ///
    /// # Errors
    ///
    /// [`SeedError::Empty`], [`SeedError::TooLong`] or [`SeedError::NotHex`].
    pub fn parse(text: &str) -> Result<Seed, SeedError> {
        let digits = text
            .strip_prefix("0x")
            .or_else(|| text.strip_prefix("0X"))
            .unwrap_or(text);
        if digits.is_empty() {
            return Err(SeedError::Empty);
        }
        let mut value: u64 = 0;
        for (index, c) in digits.chars().enumerate() {
            let Some(d) = c.to_digit(16) else {
                return Err(SeedError::NotHex { found: c });
            };
            // The digit at index 16 is the seventeenth.
            if index >= 16 {
                return Err(SeedError::TooLong {
                    digits: digits.chars().count(),
                });
            }
            // At most 16 digits, so the shift never drops a set bit.
            value = (value << 4) | u64::from(d);
        }
        Ok(Seed(value))
    }

    /// The seed from `SPAREKIT_SEED`, if it is set.
    ///
    /// # Errors
    ///
    /// [`SeedError::NotUnicode`] if the variable is set but not valid Unicode, and the
    /// errors of [`Seed::parse`] if it is not a seed.
    pub fn from_env() -> Result<Option<Seed>, SeedError> {
        match std::env::var_os(ENV_VAR) {
            None => Ok(None),
            Some(value) => match value.to_str() {
                Some(text) => Seed::parse(text).map(Some),
                None => Err(SeedError::NotUnicode),
            },
        }
    }
}

impl fmt::Display for Seed {
    /// Exactly 16 lowercase hexadecimal digits, the form [`Seed::parse`] reads.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// Where a resolved seed came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The test's fixed default.
    Default,
    /// The environment variable `SPAREKIT_SEED`.
    Env,
    /// Fixed by the test, which `SPAREKIT_SEED` does not override.
    Fixed,
}

/// Why `SPAREKIT_SEED` could not be read as a seed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedError {
    /// The variable is set but is not valid Unicode.
    NotUnicode,
    /// No digits.
    Empty,
    /// More than 16 hexadecimal digits: the value does not fit in 64 bits.
    TooLong {
        /// How many characters followed the optional prefix.
        digits: usize,
    },
    /// A character that is not a hexadecimal digit.
    NotHex {
        /// The first such character.
        found: char,
    },
}

impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{ENV_VAR} is not a seed: ")?;
        match self {
            SeedError::NotUnicode => write!(f, "it is not valid Unicode"),
            SeedError::Empty => write!(f, "it has no digits"),
            SeedError::TooLong { digits } => {
                write!(f, "{digits} digits, but a seed has at most 16")
            }
            SeedError::NotHex { found } => write!(f, "{found:?} is not a hexadecimal digit"),
        }?;
        write!(
            f,
            " (expected one to sixteen hexadecimal digits, optionally prefixed by 0x)"
        )
    }
}

impl std::error::Error for SeedError {}

/// The seed a test uses, with where it came from: `SPAREKIT_SEED` if set, otherwise `default`.
/// Prints nothing; [`resolve`] is the form tests use.
///
/// # Errors
///
/// As [`Seed::from_env`].
pub fn choose(default: Seed) -> Result<(Seed, Source), SeedError> {
    Ok(match Seed::from_env()? {
        Some(seed) => (seed, Source::Env),
        None => (default, Source::Default),
    })
}

/// The line [`resolve`] or [`fixed`] prints for a seed: `seed: <16 hex digits> (<label>; <source>;
/// replay with SPAREKIT_SEED=<16 hex digits>)`, or, for [`Source::Fixed`], `seed: <16 hex digits>
/// (<label>; fixed by the test; SPAREKIT_SEED does not apply)`.
#[must_use]
pub fn line(seed: Seed, source: Source, label: &str) -> String {
    let from = match source {
        Source::Default => "the test's default",
        Source::Env => "from SPAREKIT_SEED",
        Source::Fixed => {
            return format!("seed: {seed} ({label}; fixed by the test; {ENV_VAR} does not apply)");
        }
    };
    format!("seed: {seed} ({label}; {from}; replay with {ENV_VAR}={seed})")
}

/// Prints `seed`, which the test uses whatever `SPAREKIT_SEED` says, as [`line()`] gives it with
/// [`Source::Fixed`], and returns it. For tests whose assertions depend on one seed: a golden hash,
/// a recorded case, a law checked on a stream known to reach a rare branch.
#[must_use]
pub fn fixed(label: &str, seed: Seed) -> Seed {
    println!("{}", line(seed, Source::Fixed, label));
    seed
}

/// The seed a test uses (`SPAREKIT_SEED` if set, otherwise `default`), printed as [`line()`] gives it
/// before the test draws anything.
///
/// # Errors
///
/// As [`Seed::from_env`]. The error is returned, not replaced by the default, so a run that
/// asked for a seed never silently runs another.
pub fn resolve(label: &str, default: Seed) -> Result<Seed, SeedError> {
    let (seed, source) = choose(default)?;
    println!("{}", line(seed, source, label));
    Ok(seed)
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

    #[test]
    fn display_is_sixteen_lowercase_hex_digits() {
        assert_eq!(Seed(0).to_string(), "0000000000000000");
        assert_eq!(Seed(u64::MAX).to_string(), "ffffffffffffffff");
        assert_eq!(Seed::DEFAULT.to_string(), "53504152454b4954");
        assert_eq!(&0x5350_4152_454b_4954u64.to_be_bytes(), b"SPAREKIT");
    }

    #[test]
    fn parse_reads_what_display_writes() {
        for v in [0, 1, 0xdead_beef, Seed::DEFAULT.0, u64::MAX, 1 << 63] {
            let s = Seed(v);
            assert_eq!(Seed::parse(&s.to_string()), Ok(s));
            assert_eq!(Seed::parse(&format!("0x{s}")), Ok(s));
            assert_eq!(Seed::parse(&format!("0X{v:X}")), Ok(s));
        }
        assert_eq!(Seed::parse("7"), Ok(Seed(7)));
        assert_eq!(Seed::parse("00000000000000ff"), Ok(Seed(255)));
    }

    #[test]
    fn parse_refuses_anything_else() {
        assert_eq!(Seed::parse(""), Err(SeedError::Empty));
        assert_eq!(Seed::parse("0x"), Err(SeedError::Empty));
        assert_eq!(
            Seed::parse("10000000000000000"),
            Err(SeedError::TooLong { digits: 17 })
        );
        assert_eq!(Seed::parse("12g4"), Err(SeedError::NotHex { found: 'g' }));
        assert_eq!(Seed::parse(" 12"), Err(SeedError::NotHex { found: ' ' }));
        assert_eq!(Seed::parse("12\n"), Err(SeedError::NotHex { found: '\n' }));
        assert_eq!(Seed::parse("-1"), Err(SeedError::NotHex { found: '-' }));
        assert_eq!(Seed::parse("1_000"), Err(SeedError::NotHex { found: '_' }));
        assert_eq!(Seed::parse("0x0x1"), Err(SeedError::NotHex { found: 'x' }));
        // A full-width digit is a digit to Unicode but not to the seed format.
        assert_eq!(
            Seed::parse("\u{ff11}"),
            Err(SeedError::NotHex { found: '\u{ff11}' })
        );
    }

    #[test]
    fn line_starts_with_the_seed_and_names_the_replay() {
        let l = line(Seed(0xabc), Source::Default, "orient2d swaps");
        assert_eq!(
            l,
            "seed: 0000000000000abc (orient2d swaps; the test's default; \
             replay with SPAREKIT_SEED=0000000000000abc)"
        );
        assert!(line(Seed(1), Source::Env, "x").contains("from SPAREKIT_SEED"));
        assert_eq!(
            line(Seed(0xabc), Source::Fixed, "golden"),
            "seed: 0000000000000abc (golden; fixed by the test; SPAREKIT_SEED does not apply)"
        );
        assert_eq!(fixed("golden", Seed(9)), Seed(9));
    }

    #[test]
    fn errors_name_the_variable_and_the_format() {
        let e = SeedError::NotHex { found: 'g' }.to_string();
        assert!(e.starts_with("SPAREKIT_SEED is not a seed"));
        assert!(e.contains("'g'"));
        assert!(e.contains("sixteen hexadecimal digits"));
    }
}
