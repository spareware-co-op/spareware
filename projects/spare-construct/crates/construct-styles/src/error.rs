//! Every refusal this crate makes. A refusal is a value the caller reads; nothing here panics.

use std::fmt;

/// Why a table, a pen, a pattern or a stored form was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleError {
    /// A width or length of zero, or above the largest this crate stores.
    BadLength {
        /// What was given, in whole micrometres.
        micrometres: u32,
    },
    /// A view scale with a denominator of zero, or above the largest this crate stores.
    BadScale {
        /// What was given: the `n` of 1:n.
        denominator: u32,
    },
    /// A pen table with no scales.
    NoScales,
    /// A pen table with more scales than the limit.
    TooManyScales,
    /// Scales that are not in strictly ascending order of denominator (so a scale is repeated or
    /// out of place).
    ScalesNotAscending,
    /// A table with no pens.
    NoPens,
    /// A table with more pens than the limit.
    TooManyPens,
    /// Pen numbers that are zero, repeated or not in ascending order.
    PensNotAscending,
    /// No pen has this number.
    NoSuchPen {
        /// The number asked for.
        pen: u16,
    },
    /// A pen gives a different number of widths from the table's number of scales.
    WidthCount {
        /// The pen.
        pen: u16,
        /// How many scales the table has.
        expected: usize,
        /// How many widths were given.
        found: usize,
    },
    /// A pen would draw thicker on a smaller-scale drawing than on a larger-scale one.
    WidthGrows {
        /// The pen.
        pen: u16,
        /// The position, among the table's scales, of the first width that is thicker than the one
        /// before it.
        scale_index: usize,
    },
    /// A name that is empty, has spaces at an end, has a control character or is too long.
    BadName,
    /// Two pens, or two patterns, with the same name.
    DuplicateName,
    /// Patterns whose IDs are not in strictly ascending order.
    PatternsNotAscending,
    /// No pattern has this ID.
    NoSuchPattern {
        /// The ID asked for.
        id: u32,
    },
    /// A built-in pattern cannot be removed or changed.
    BuiltIn {
        /// The built-in pattern.
        id: u32,
    },
    /// The built-in patterns are missing or are not as this crate defines them.
    BuiltInsChanged,
    /// A pattern with no dash and no dot: it would draw nothing.
    NoMarks,
    /// A pattern whose cycle has no length (only dots): it would draw every mark on one point.
    ZeroCycle,
    /// A pattern with more segments than the limit.
    TooManySegments,
    /// No more pattern IDs can be given out.
    IdsExhausted,
    /// The stored form is not a valid table: the reason as the JSON reader gave it.
    Format(String),
    /// The stored form is of a version this reader does not know.
    Version {
        /// The version found.
        found: u32,
    },
    /// The stored form is valid but is not spelt the one way this crate writes it.
    NotCanonical,
}

impl fmt::Display for StyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadLength { micrometres } => {
                write!(f, "a length of {micrometres} micrometres is not allowed")
            }
            Self::BadScale { denominator } => {
                write!(f, "a scale of 1:{denominator} is not allowed")
            }
            Self::NoScales => f.write_str("a pen table needs at least one scale"),
            Self::TooManyScales => f.write_str("too many scales"),
            Self::ScalesNotAscending => {
                f.write_str("scales must be in strictly ascending order of denominator")
            }
            Self::NoPens => f.write_str("a pen table needs at least one pen"),
            Self::TooManyPens => f.write_str("too many pens"),
            Self::PensNotAscending => {
                f.write_str("pen numbers must be above zero, ascending and not repeated")
            }
            Self::NoSuchPen { pen } => write!(f, "there is no pen {pen}"),
            Self::WidthCount {
                pen,
                expected,
                found,
            } => write!(
                f,
                "pen {pen} needs {expected} widths, one for each scale, but has {found}"
            ),
            Self::WidthGrows { pen, scale_index } => write!(
                f,
                "pen {pen} would be thicker at scale position {scale_index} than at the one before"
            ),
            Self::BadName => f.write_str(
                "a name must be 1 to 64 characters with no control characters or outer spaces",
            ),
            Self::DuplicateName => f.write_str("that name is already used"),
            Self::PatternsNotAscending => {
                f.write_str("pattern IDs must be in strictly ascending order")
            }
            Self::NoSuchPattern { id } => write!(f, "there is no pattern {id}"),
            Self::BuiltIn { id } => write!(f, "pattern {id} is built in and cannot be removed"),
            Self::BuiltInsChanged => f.write_str("the built-in patterns are not as defined"),
            Self::NoMarks => f.write_str("a pattern needs at least one dash or dot"),
            Self::ZeroCycle => f.write_str("a pattern's cycle must have a length above zero"),
            Self::TooManySegments => f.write_str("too many segments in a pattern"),
            Self::IdsExhausted => f.write_str("no more pattern IDs"),
            Self::Format(why) => write!(f, "not a valid stored form: {why}"),
            Self::Version { found } => write!(f, "stored form version {found} is not known"),
            Self::NotCanonical => f.write_str("the stored form is not spelt the way it is written"),
        }
    }
}

impl std::error::Error for StyleError {}
