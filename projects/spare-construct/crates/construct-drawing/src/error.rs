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
    /// are not equally far from its centre, a loop of fewer than three points, a region's edge
    /// with no cut line of its element, a hatch family with no direction.
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
    /// A region's material is not in the materials given: its id.
    NoMaterial(String),
    /// A material's cut fill names no fill pattern in the table given: the name.
    NoFillPattern(String),
    /// A hatch would draw more lines than its budget: the count, made before any line, and the
    /// most allowed (chapter 14.6 of the geometry architecture: no silent caps).
    Budget {
        /// The lines the hatch would draw.
        lines: u64,
        /// The most it may draw.
        most: u64,
    },
    /// The interim 2D overlay found its own rule broken (a boundary that does not close, a hole in
    /// no outline): its words. A bug to report, never an answer.
    Overlay(String),
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
            Self::NoMaterial(id) => write!(f, "no material {id} in the materials given"),
            Self::NoFillPattern(name) => write!(f, "no fill pattern named {name}"),
            Self::Budget { lines, most } => {
                write!(f, "a hatch of {lines} lines, past the most allowed, {most}")
            }
            Self::Overlay(what) => write!(f, "the 2D overlay broke its own rule: {what}"),
        }
    }
}

impl std::error::Error for DrawingError {}
