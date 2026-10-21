//! One generator: its name, the one output file it owns, its seed if it has one, and the pure
//! function that produces the file's bytes.
//!
//! What it does: [`Generator`] describes a generator; [`Generator::run`] produces its output in
//! memory; [`Generator::seed_line`] is the line every command prints for it.
//!
//! Guarantees:
//!
//! - **A seed is explicit, and printed.** A generator that uses randomness is
//!   [`Generate::Seeded`], with its seed fixed beside it in the registry, and its function receives
//!   that seed. One that uses none is [`Generate::Plain`], and its function receives no seed. Every
//!   command prints every generator's line, `seed: <16 hex digits>` or `seed: none`, before it runs
//!   any generator.
//! - **No hidden seed: a checked rule, not a type rule.** Rust's types cannot stop a generator's
//!   function from building its own seed, because `geom_test::Seed` and `geom_test::SplitMix64` are
//!   public. So `tests/purity.rs` reads the source of every generator module and fails if one names
//!   a seed, builds a generator or another source of randomness, or reads the environment, a file
//!   or a clock; only `registry.rs` may write a seed, and only as a `Generate::Seeded` entry.
//! - **Pure.** The function is a plain `fn` with no captured state: the same bytes every call.
//!
//! Method: the committed-output pattern of chapter 27's proposed `geom-gen` rule
//! (`projects/engines/geometry/architecture/27-crates-folders.md`, "Generated code and data have
//! one home"), with the seed discipline of sections 28.5 and 28.17
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: whatever the generator's function costs.

use core::fmt;

use geom_test::Seed;

/// How a generator produces its bytes.
#[derive(Clone, Copy, Debug)]
pub enum Generate {
    /// Uses no randomness.
    Plain(fn() -> Result<Vec<u8>, GenError>),
    /// Uses randomness, all of it drawn from `geom-test`'s `SplitMix64` seeded with this seed.
    Seeded(Seed, fn(Seed) -> Result<Vec<u8>, GenError>),
}

/// A generator whose output is committed.
#[derive(Clone, Copy, Debug)]
pub struct Generator {
    /// A short unique name, in kebab case.
    pub name: &'static str,
    /// The one file it owns: a path relative to the workspace root, with `/` separators and no
    /// `.` or `..` components.
    pub output: &'static str,
    /// One line on what it generates.
    pub about: &'static str,
    /// How it produces the file's bytes.
    pub generate: Generate,
}

impl Generator {
    /// Its seed, if it uses randomness.
    #[must_use]
    pub fn seed(&self) -> Option<Seed> {
        match self.generate {
            Generate::Plain(_) => None,
            Generate::Seeded(seed, _) => Some(seed),
        }
    }

    /// Its output, produced in memory.
    ///
    /// # Errors
    ///
    /// Whatever its function returns.
    pub fn run(&self) -> Result<Vec<u8>, GenError> {
        match self.generate {
            Generate::Plain(f) => f(),
            Generate::Seeded(seed, f) => f(seed),
        }
    }

    /// The line every command prints for it: `<name>: seed: <16 hex digits>`, or
    /// `<name>: seed: none (uses no randomness)`.
    #[must_use]
    pub fn seed_line(&self) -> String {
        match self.seed() {
            Some(seed) => format!("{}: seed: {seed}", self.name),
            None => format!("{}: seed: none (uses no randomness)", self.name),
        }
    }
}

/// Why a generator, the registry or a file operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenError {
    /// A generator could not produce its output.
    Generator {
        /// The generator.
        name: String,
        /// What went wrong.
        message: String,
    },
    /// The registry breaks a rule of `files::validate`: a name, a folder or an output path that is
    /// not allowed, or two outputs that would be one file.
    Registry(String),
    /// A file could not be read or written.
    Io {
        /// The file.
        path: String,
        /// The error.
        message: String,
    },
}

impl GenError {
    /// A generator error for `name`.
    #[must_use]
    pub fn generator(name: &str, message: impl Into<String>) -> GenError {
        GenError::Generator {
            name: name.to_owned(),
            message: message.into(),
        }
    }
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenError::Generator { name, message } => write!(f, "generator {name}: {message}"),
            GenError::Registry(m) => write!(f, "registry: {m}"),
            GenError::Io { path, message } => write!(f, "{path}: {message}"),
        }
    }
}

impl std::error::Error for GenError {}
