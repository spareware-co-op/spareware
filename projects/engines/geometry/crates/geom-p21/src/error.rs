//! Why a Part 21 text was refused or could not be written: one typed error for the reader, with the
//! line and the byte offset, and one for the writer.
//!
//! What it does: [`Error`] is every refusal of the reader ([`crate::read`]) and of the collected
//! exchange ([`crate::exchange`]): an [`ErrorKind`] with the line (counted from 1) and the byte
//! offset where reading stopped. [`WriteError`] is every refusal of the canonical writer
//! ([`crate::write`]).
//!
//! Guarantees:
//!
//! - **A refusal is a value.** No kind carries text copied from the file, so an error costs no
//!   allocation and is `Copy`.
//! - **The place is inside the input.** `offset` is the number of bytes read when the reader
//!   stopped, never more than the input's length; `line` is one more than the line feeds among
//!   them.
//!
//! Method: none; an error enum and its `Display` and `Error` implementations.
//!
//! Cost: constant time and no allocation.

use core::fmt;

use geom_format::source::SourceError;
use geom_run::Stopped;

/// What was wrong with a Part 21 text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The byte source failed.
    Source(SourceError),
    /// The caller's work meter stopped the read: over its budget, or cancelled.
    Stopped(Stopped),
    /// The text does not start with `ISO-10303-21;`.
    NotP21,
    /// The first section is not `HEADER;`.
    MissingHeader,
    /// The text ends before `END-ISO-10303-21;`.
    UnexpectedEnd,
    /// A `/*` with no `*/` after it.
    UnterminatedComment,
    /// A `'` with no closing `'` after it.
    UnterminatedString,
    /// This byte was expected and another was found.
    Expected(u8),
    /// A keyword was expected.
    ExpectedKeyword,
    /// A byte that starts no parameter.
    UnexpectedByte(u8),
    /// A keyword that opens no section here: a second `HEADER`, or a name that is no section.
    UnexpectedSection,
    /// A number that is not `[+-]digits[.digits][E[+-]digits]` with a digit in its mantissa.
    BadNumber,
    /// An integer beyond `i64`.
    IntegerOutOfRange,
    /// A real beyond `f64`'s finite range.
    RealOutOfRange,
    /// A `#` that is not followed by digits within `u64`.
    BadInstanceName,
    /// A `.` that does not open `.NAME.`.
    BadEnumeration,
    /// A `"..."` that is not a first digit `0` to `3` and upper-case hexadecimal digits after it.
    BadBinary,
    /// A string that is not UTF-8, or an `\X2\` or `\X4\` run that is not whole groups of
    /// hexadecimal digits naming Unicode characters ([`crate::Options::lossy_text`] replaces them
    /// instead).
    BadText,
    /// An instance with no record: `#1=();`.
    EmptyInstance,
    /// Lists nested deeper than [`crate::MAX_DEPTH`].
    TooDeep,
    /// One record or instance holds more items than [`crate::Options::items`].
    TooManyItems,
    /// One record or instance holds more text than [`crate::Options::text_bytes`].
    TooMuchText,
    /// The text is longer than [`crate::exchange::Totals::bytes`].
    TooLarge,
    /// More instances than [`crate::exchange::Totals::instances`].
    TooManyInstances,
    /// More records than [`crate::exchange::Totals::records`].
    TooManyRecords,
    /// More parameters than [`crate::exchange::Totals::params`].
    TooManyParams,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Source(e) => write!(f, "{e}"),
            ErrorKind::Stopped(e) => write!(f, "{e}"),
            ErrorKind::NotP21 => f.write_str("not a Part 21 file (no ISO-10303-21; at its start)"),
            ErrorKind::MissingHeader => f.write_str("the first section is not HEADER"),
            ErrorKind::UnexpectedEnd => f.write_str("the file ends without END-ISO-10303-21;"),
            ErrorKind::UnterminatedComment => f.write_str("unterminated comment"),
            ErrorKind::UnterminatedString => f.write_str("unterminated string"),
            ErrorKind::Expected(c) => write!(f, "expected `{}`", char::from(*c)),
            ErrorKind::ExpectedKeyword => f.write_str("expected a keyword"),
            ErrorKind::UnexpectedByte(c) if matches!(c, b' '..=b'~') => {
                write!(f, "unexpected `{}`", char::from(*c))
            }
            ErrorKind::UnexpectedByte(c) => write!(f, "unexpected byte {c:#04X}"),
            ErrorKind::UnexpectedSection => f.write_str("a keyword that opens no section here"),
            ErrorKind::BadNumber => f.write_str("bad number"),
            ErrorKind::IntegerOutOfRange => f.write_str("integer out of range"),
            ErrorKind::RealOutOfRange => f.write_str("real out of range"),
            ErrorKind::BadInstanceName => f.write_str("bad instance name"),
            ErrorKind::BadEnumeration => f.write_str("bad enumeration"),
            ErrorKind::BadBinary => f.write_str("bad binary"),
            ErrorKind::BadText => f.write_str("a string that is not Unicode text"),
            ErrorKind::EmptyInstance => f.write_str("an instance with no record"),
            ErrorKind::TooDeep => f.write_str("lists nested too deeply"),
            ErrorKind::TooManyItems => f.write_str("too many items in one record"),
            ErrorKind::TooMuchText => f.write_str("too much text in one record"),
            ErrorKind::TooLarge => f.write_str("the file is too large"),
            ErrorKind::TooManyInstances => f.write_str("too many instances"),
            ErrorKind::TooManyRecords => f.write_str("too many records"),
            ErrorKind::TooManyParams => f.write_str("too many parameters"),
        }
    }
}

/// A refusal of the reader: what was wrong and where reading stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Error {
    /// What was wrong.
    pub kind: ErrorKind,
    /// The line reading stopped on, counted from 1: one more than the line feeds read.
    pub line: u64,
    /// The bytes read when reading stopped.
    pub offset: u64,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Part 21 error at line {} (byte {}): {}",
            self.line, self.offset, self.kind
        )
    }
}

impl core::error::Error for Error {}

/// A refusal of the canonical writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WriteError {
    /// The sink failed; the number is the program's.
    Sink(u32),
    /// A call out of the file's order: a record outside its section, a section inside another, or
    /// anything after the end.
    OutOfOrder,
    /// An instance whose id is not above the one written before it.
    InstanceOrder {
        /// The id refused.
        id: u64,
        /// The id written before it.
        last: u64,
    },
    /// An instance with no record.
    EmptyInstance,
    /// A real that is infinite or not a number.
    NonFiniteReal,
    /// A record or typed-value name that is not `[!]` and then `[A-Z_][A-Z0-9_]*`.
    BadName,
    /// An enumeration that is not `[A-Z_][A-Z0-9_]*`.
    BadEnumeration,
    /// A binary that is not a first digit `0` to `3` and upper-case hexadecimal digits after it.
    BadBinary,
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Sink(code) => write!(f, "the sink failed ({code})"),
            WriteError::OutOfOrder => f.write_str("a call out of the file's order"),
            WriteError::InstanceOrder { id, last } => {
                write!(f, "instance #{id} does not follow #{last}")
            }
            WriteError::EmptyInstance => f.write_str("an instance with no record"),
            WriteError::NonFiniteReal => f.write_str("a real that is not finite"),
            WriteError::BadName => f.write_str("a name Part 21 cannot write"),
            WriteError::BadEnumeration => f.write_str("an enumeration Part 21 cannot write"),
            WriteError::BadBinary => f.write_str("a binary Part 21 cannot write"),
        }
    }
}

impl core::error::Error for WriteError {}
