//! Every refusal, as a value the caller can read. Its words are what a person sees.

use kit_core::Id;
use std::fmt;

/// Why a sheet, a viewport, a record of one or a layout was refused. A refusal changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SheetError {
    /// A sheet needs a name with something in it besides spaces.
    EmptyName,
    /// A sheet needs a number with something in it besides spaces.
    EmptyNumber,
    /// A sheet number that is too long or holds a control character.
    BadNumber(String),
    /// Another sheet already has this number (case does not matter).
    DuplicateNumber(String),
    /// A sheet already has this ID.
    DuplicateSheet(Id),
    /// No sheet has this ID.
    UnknownSheet(Id),
    /// A viewport already has this ID.
    DuplicateViewport(Id),
    /// No viewport has this ID.
    UnknownViewport(Id),
    /// No view has this ID.
    UnknownView(Id),
    /// A custom paper size outside what a sheet can be, or that does not fit.
    BadPaper(String),
    /// The view is already placed: on this sheet or another.
    ViewPlaced {
        /// The view.
        view: Id,
        /// The sheet it is on, by number.
        sheet_number: String,
    },
    /// A viewport's position (its centre) is off the paper.
    OffPaper {
        /// The sheet's number.
        sheet_number: String,
        /// The viewport's centre, as text.
        position: String,
        /// The paper, as text.
        paper: String,
    },
    /// A viewport aligned to itself, to one on another sheet, or round a loop.
    BadAlignment(String),
    /// A viewport's position on an axis it is aligned on: it follows the other viewport.
    Aligned(String),
    /// Other viewports are aligned to this one.
    Followed {
        /// The viewport others follow.
        viewport: Id,
        /// One of the viewports that follows it.
        follower: Id,
    },
    /// A view's extent that is negative or too large.
    BadExtent(String),
    /// A sum that does not fit.
    OutOfRange(String),
    /// A record that is not a sheet or a viewport as written.
    Malformed(String),
}

impl fmt::Display for SheetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "A sheet needs a name."),
            Self::EmptyNumber => write!(f, "A sheet needs a number."),
            Self::BadNumber(why) | Self::BadPaper(why) | Self::Malformed(why) => f.write_str(why),
            Self::DuplicateNumber(number) => {
                write!(
                    f,
                    "There is already a sheet numbered {number:?} in this model."
                )
            }
            Self::DuplicateSheet(id) => write!(f, "There is already a sheet {id}."),
            Self::UnknownSheet(id) => write!(f, "There is no sheet {id} in this model."),
            Self::DuplicateViewport(id) => write!(f, "There is already a viewport {id}."),
            Self::UnknownViewport(id) => write!(f, "There is no viewport {id} in this model."),
            Self::UnknownView(id) => write!(f, "There is no view {id} in this model."),
            Self::ViewPlaced { view, sheet_number } => write!(
                f,
                "The view {view} is already placed, on sheet {sheet_number:?}; a view is on one sheet only."
            ),
            Self::OffPaper {
                sheet_number,
                position,
                paper,
            } => write!(
                f,
                "A viewport centred at {position} is off sheet {sheet_number:?}, which is {paper}."
            ),
            Self::BadAlignment(why) => write!(f, "Cannot align: {why}."),
            Self::Aligned(axis) => write!(
                f,
                "The viewport is aligned on {axis}; it follows the viewport it is aligned to. \
                 Take the alignment away to place it by hand."
            ),
            Self::Followed { viewport, follower } => write!(
                f,
                "The viewport {viewport} is followed by {follower}, which is aligned to it."
            ),
            Self::BadExtent(why) | Self::OutOfRange(why) => write!(f, "Out of range: {why}."),
        }
    }
}

impl std::error::Error for SheetError {}
