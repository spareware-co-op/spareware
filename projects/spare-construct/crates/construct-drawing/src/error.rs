//! Every refusal of this crate, as a value the caller can read.

use std::fmt;

/// Why a drawing was not made, read, looked up or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawingError {
    /// A number of the drawing does not fit its type: a coordinate past 128 bits.
    TooBig,
    /// A point whose denominator is not positive or which is not in lowest terms.
    BadPoint,
    /// A line or region that is not one a drawing holds: a segment of no length, an arc whose ends
    /// are not equally far from its centre, a loop of fewer than three points.
    BadShape(String),
    /// The stored bytes are not a drawing of this version, or not in their one canonical form.
    Format(String),
    /// The engine or the kernel builder refused: its code and its words.
    Geometry {
        /// The builder's code, such as `EngineUnsupported`.
        code: String,
        /// Its message.
        message: String,
    },
    /// A cut edge lies on a plane none of the element's named faces lies on.
    UnnamedFace(String),
    /// The object styles refused a lookup: its words.
    Style(String),
}

impl fmt::Display for DrawingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooBig => write!(f, "a number of the drawing is past 128 bits"),
            Self::BadPoint => write!(f, "a point not in lowest terms over a positive denominator"),
            Self::BadShape(what) => write!(f, "not a shape a drawing holds: {what}"),
            Self::Format(what) => write!(f, "not a stored drawing: {what}"),
            Self::Geometry { code, message } => write!(f, "{code}: {message}"),
            Self::UnnamedFace(what) => write!(f, "a cut edge on no named face: {what}"),
            Self::Style(what) => write!(f, "the styles refused: {what}"),
        }
    }
}

impl std::error::Error for DrawingError {}
