//! Why bytes or text were refused as SKG's deterministic CBOR, and where.
//!
//! What it does: [`CborError`] carries the byte offset of the head at fault and a
//! [`CborErrorKind`]; [`TextError`] carries the line and column of the text at fault and a
//! [`TextErrorKind`]. Every refusal of the decoder and of the text form is one of these, never a
//! panic (24.7).
//!
//! Guarantees:
//!
//! - **One kind per rule.** Each rule of 24.3's canonical form that bytes can break has its own
//!   kind, so a test can tell which rule refused an input.
//!
//! Method: none; it defines error types and their `Display` and `Error` implementations.
//!
//! Cost: a few machine words; nothing allocates.

use core::fmt;

use geom_run::Stopped;

/// Why bytes were refused as SKG's deterministic CBOR, and the offset of the item at fault.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CborError {
    /// The byte offset, from the start of the input, of the head of the item at fault, or of
    /// the first byte past the end for [`CborErrorKind::Truncated`].
    pub offset: usize,
    /// Which rule the item breaks.
    pub kind: CborErrorKind,
}

/// The rule of SKG's CBOR subset (24.3, RFC 8949 section 4.2.1) an input breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CborErrorKind {
    /// The input ends inside an item.
    Truncated,
    /// Additional information 28, 29 or 30, which RFC 8949 reserves.
    ReservedInfo,
    /// An indefinite length or a "break" byte; deterministic CBOR has neither.
    Indefinite,
    /// An integer, length, count or tag number not written in the shortest head that holds it.
    NonShortestHead,
    /// A float written wider than the shortest of half, single and double precision that holds
    /// its value exactly.
    NonShortestFloat,
    /// A NaN or an infinity, in a float item or in a column of doubles (tag 86).
    NotFinite,
    /// A simple value other than `false`, `true` and `null`.
    UnsupportedSimple,
    /// A tag outside the subset's four: 2 and 3 (bignums), 79 (`i64` column) and 86 (`f64`
    /// column).
    UnsupportedTag,
    /// A tag whose content is not a byte string.
    TagContent,
    /// A bignum whose value fits in 64 bits, or whose bytes start with a zero byte.
    NonCanonicalBignum,
    /// A column whose byte string is not a whole number of 8-byte values.
    ColumnLength,
    /// A text string that is not UTF-8.
    InvalidUtf8,
    /// A map key not after the previous key in the bytewise order of their encodings.
    UnsortedKeys,
    /// A map key equal to the previous key.
    DuplicateKey,
    /// Containers and tags nested deeper than the caller's limit.
    TooDeep,
    /// Bytes after the one item the input holds.
    TrailingBytes,
    /// The work meter stopped: over its budget, cancelled, or a large allocation refused.
    Stopped(Stopped),
}

impl fmt::Display for CborErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CborErrorKind::Truncated => f.write_str("the input ends inside an item"),
            CborErrorKind::ReservedInfo => f.write_str("reserved additional information"),
            CborErrorKind::Indefinite => f.write_str("an indefinite length or a break"),
            CborErrorKind::NonShortestHead => f.write_str("a head longer than its value needs"),
            CborErrorKind::NonShortestFloat => f.write_str("a float wider than its value needs"),
            CborErrorKind::NotFinite => f.write_str("a NaN or an infinity"),
            CborErrorKind::UnsupportedSimple => {
                f.write_str("a simple value other than false, true and null")
            }
            CborErrorKind::UnsupportedTag => f.write_str("a tag outside 2, 3, 79 and 86"),
            CborErrorKind::TagContent => f.write_str("a tag whose content is not a byte string"),
            CborErrorKind::NonCanonicalBignum => {
                f.write_str("a bignum that fits in 64 bits or has a leading zero byte")
            }
            CborErrorKind::ColumnLength => {
                f.write_str("a column that is not a whole number of 8-byte values")
            }
            CborErrorKind::InvalidUtf8 => f.write_str("a text string that is not UTF-8"),
            CborErrorKind::UnsortedKeys => f.write_str("map keys out of order"),
            CborErrorKind::DuplicateKey => f.write_str("a repeated map key"),
            CborErrorKind::TooDeep => f.write_str("nesting deeper than the limit"),
            CborErrorKind::TrailingBytes => f.write_str("bytes after the item"),
            CborErrorKind::Stopped(s) => write!(f, "{s}"),
        }
    }
}

impl fmt::Display for CborError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "not SKG's deterministic CBOR at byte {}: {}",
            self.offset, self.kind
        )
    }
}

impl core::error::Error for CborError {}

/// Why text was refused as the diagnostic form of SKG's CBOR, and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextError {
    /// The line of the text at fault, counted from 1.
    pub line: u32,
    /// The column of the text at fault, in characters, counted from 1.
    pub column: u32,
    /// What is wrong there.
    pub kind: TextErrorKind,
}

/// What is wrong with text given as the diagnostic form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextErrorKind {
    /// A character no item can start with, or one that does not belong where it stands.
    Unexpected,
    /// The text ends inside an item, or holds no item.
    UnexpectedEnd,
    /// An integer outside −2^64 to 2^64 − 1, or a tag number above 2^64 − 1.
    IntegerRange,
    /// A float that is not a hexadecimal float, or one no double holds exactly.
    InexactFloat,
    /// A malformed escape in a text string, or one naming a lone surrogate.
    BadEscape,
    /// An odd number of hexadecimal digits, or a character that is not one, in `h'...'`.
    BadHex,
    /// A comment that is never closed.
    OpenComment,
    /// Text after the one item.
    Trailing,
    /// The item the text describes breaks a rule of SKG's CBOR subset; the kind says which.
    Cbor(CborErrorKind),
}

impl fmt::Display for TextErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextErrorKind::Unexpected => f.write_str("an unexpected character"),
            TextErrorKind::UnexpectedEnd => f.write_str("the text ends inside an item"),
            TextErrorKind::IntegerRange => f.write_str("an integer out of range"),
            TextErrorKind::InexactFloat => {
                f.write_str("a float that is not a hexadecimal float a double holds exactly")
            }
            TextErrorKind::BadEscape => f.write_str("a malformed escape"),
            TextErrorKind::BadHex => f.write_str("malformed hexadecimal bytes"),
            TextErrorKind::OpenComment => f.write_str("a comment that is never closed"),
            TextErrorKind::Trailing => f.write_str("text after the item"),
            TextErrorKind::Cbor(kind) => write!(f, "{kind}"),
        }
    }
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "not SKG's diagnostic text at line {}, column {}: {}",
            self.line, self.column, self.kind
        )
    }
}

impl core::error::Error for TextError {}
