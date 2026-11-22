//! Every refusal, as a value the caller can read and show.

use crate::definition::Kind;
use crate::units::Unit;
use std::fmt;

/// An arithmetic step on values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Adding two values.
    Add,
    /// Taking one value from another.
    Subtract,
    /// Multiplying two values.
    Multiply,
}

impl Op {
    fn verb(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Multiply => "multiply",
        }
    }
}

/// Why a value, a text or a definition was refused. Each names the parameter it was meant for,
/// and where two kinds met, both of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamError {
    /// A value of one kind was given to a parameter of another.
    WrongKind {
        /// The parameter's name.
        parameter: String,
        /// The parameter's kind.
        expected: Kind,
        /// The value's kind.
        found: Kind,
    },
    /// Typed text carried a unit of another kind, such as `m2` for a length.
    WrongUnit {
        /// The parameter's name.
        parameter: String,
        /// The parameter's kind.
        expected: Kind,
        /// The unit as it was typed.
        unit: String,
        /// The kind that unit measures.
        found: Kind,
    },
    /// Typed text carried a unit this crate does not read.
    UnknownUnit {
        /// The parameter's name.
        parameter: String,
        /// The unit as it was typed.
        unit: String,
    },
    /// Typed text held no number where one was needed.
    NotANumber {
        /// The parameter's name.
        parameter: String,
        /// The text.
        text: String,
    },
    /// A whole number was needed and the text has a fraction.
    NotWhole {
        /// The parameter's name.
        parameter: String,
        /// The text.
        text: String,
    },
    /// The number is too large to store, or is a slope with no ratio (a vertical one).
    OutOfRange {
        /// The parameter's name.
        parameter: String,
        /// The text.
        text: String,
    },
    /// The text is neither yes nor no.
    NotYesOrNo {
        /// The parameter's name.
        parameter: String,
        /// The text.
        text: String,
    },
    /// The choice is not in the parameter's list.
    NotAChoice {
        /// The parameter's name.
        parameter: String,
        /// The text or key given.
        text: String,
    },
    /// The text is not a permanent ID, for an element or material link.
    BadId {
        /// The parameter's name.
        parameter: String,
        /// The text.
        text: String,
    },
    /// The text is not an http or https web address.
    BadLink {
        /// The parameter's name.
        parameter: String,
        /// The text.
        text: String,
    },
    /// Stored text that is not a value in its one spelling.
    BadStored {
        /// The parameter's name.
        parameter: String,
        /// The stored text.
        text: String,
    },
    /// A definition that breaks a rule, as made or as read back.
    BadDefinition {
        /// The parameter's name, as far as it is known.
        parameter: String,
        /// Which rule.
        reason: &'static str,
    },
    /// A display format whose unit is not for its kind, or with more than nine places.
    BadFormat {
        /// The kind it was set for.
        kind: Kind,
        /// Its unit.
        unit: Unit,
        /// Its places.
        places: u8,
    },
    /// Arithmetic on two kinds that do not go together.
    Mismatch {
        /// The step.
        op: Op,
        /// The left value's kind.
        left: Kind,
        /// The right value's kind.
        right: Kind,
    },
    /// Arithmetic whose answer is too large to store.
    Overflow {
        /// The step.
        op: Op,
        /// The kind of the answer.
        kind: Kind,
    },
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongKind {
                parameter,
                expected,
                found,
            } => write!(f, "{parameter} takes {expected}, not {found}"),
            Self::WrongUnit {
                parameter,
                expected,
                unit,
                found,
            } => write!(
                f,
                "{parameter} takes {expected}, and {unit:?} is a unit of {found}"
            ),
            Self::UnknownUnit { parameter, unit } => {
                write!(f, "{parameter}: {unit:?} is not a unit this reads")
            }
            Self::NotANumber { parameter, text } => {
                write!(f, "{parameter}: {text:?} is not a number")
            }
            Self::NotWhole { parameter, text } => {
                write!(f, "{parameter} takes a whole number, not {text:?}")
            }
            Self::OutOfRange { parameter, text } => {
                write!(f, "{parameter}: {text:?} is out of range")
            }
            Self::NotYesOrNo { parameter, text } => {
                write!(f, "{parameter} takes yes or no, not {text:?}")
            }
            Self::NotAChoice { parameter, text } => {
                write!(f, "{parameter}: {text:?} is not one of its choices")
            }
            Self::BadId { parameter, text } => {
                write!(f, "{parameter}: {text:?} is not a permanent ID")
            }
            Self::BadLink { parameter, text } => {
                write!(
                    f,
                    "{parameter}: {text:?} is not an http or https web address"
                )
            }
            Self::BadStored { parameter, text } => {
                write!(f, "{parameter}: {text:?} is not a stored value")
            }
            Self::BadDefinition { parameter, reason } => {
                write!(f, "parameter {parameter:?}: {reason}")
            }
            Self::BadFormat { kind, unit, places } => write!(
                f,
                "{kind} can't be shown in {} to {places} places",
                unit.name()
            ),
            Self::Mismatch { op, left, right } => {
                write!(f, "can't {} {left} and {right}", op.verb())
            }
            Self::Overflow { op, kind } => {
                write!(f, "{kind} from that {} is too large to store", op.verb())
            }
        }
    }
}

impl std::error::Error for ParamError {}
