//! Every refusal, as a value the caller can read. Its words are what a person sees.

use kit_core::Id;
use std::fmt;

/// Why a text note, a text style, a leader or a layout was refused. A refusal changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextError {
    /// A style needs a name with something in it besides spaces.
    EmptyName,
    /// A style name that is too long or holds a control character.
    BadName(String),
    /// Another style already has this name (case does not matter).
    DuplicateName(String),
    /// A style already has this ID.
    DuplicateStyle(Id),
    /// No style has this ID.
    UnknownStyle(Id),
    /// A note already has this ID.
    DuplicateNote(Id),
    /// No note has this ID.
    UnknownNote(Id),
    /// No view has this ID.
    UnknownView(Id),
    /// A font name that is empty, too long or holds a control character.
    BadFont(String),
    /// A size, a width or a line spacing that is not above zero or is too large.
    BadSize(String),
    /// Text that is empty, too long or holds a control character.
    BadText(String),
    /// A rotation outside one turn either way.
    BadRotation(String),
    /// A leader with fewer than two points, or too many.
    BadLeader(String),
    /// A note has no leader of this number.
    UnknownLeader {
        /// The note.
        note: Id,
        /// The leader's number.
        leader: u32,
    },
    /// A style that notes still use.
    StyleInUse {
        /// The style.
        style: Id,
        /// How many notes use it.
        notes: usize,
    },
    /// A view that still holds notes.
    ViewHasNotes {
        /// The view.
        view: Id,
        /// How many notes it holds.
        notes: usize,
    },
    /// The measuring of a font failed (the caller's metrics did not know it).
    Metrics(String),
    /// A sum that does not fit.
    OutOfRange(String),
    /// A record that is not a note or a style as written.
    Malformed(String),
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "A text style needs a name."),
            Self::BadName(why)
            | Self::BadFont(why)
            | Self::BadSize(why)
            | Self::BadText(why)
            | Self::BadRotation(why)
            | Self::BadLeader(why)
            | Self::Metrics(why)
            | Self::Malformed(why) => f.write_str(why),
            Self::DuplicateName(name) => write!(
                f,
                "There is already a text style named {name:?} in this model."
            ),
            Self::DuplicateStyle(id) => write!(f, "There is already a text style {id}."),
            Self::UnknownStyle(id) => write!(f, "There is no text style {id} in this model."),
            Self::DuplicateNote(id) => write!(f, "There is already a text note {id}."),
            Self::UnknownNote(id) => write!(f, "There is no text note {id} in this model."),
            Self::UnknownView(id) => write!(f, "There is no view {id} in this model."),
            Self::UnknownLeader { note, leader } => {
                write!(f, "The text note {note} has no leader {leader}.")
            }
            Self::StyleInUse { style, notes } => write!(
                f,
                "The text style {style} is used by {notes} text note{}; give those notes another \
                 style first.",
                plural(*notes)
            ),
            Self::ViewHasNotes { view, notes } => write!(
                f,
                "The view {view} holds {notes} text note{}; delete them with text.note.delete \
                 first.",
                plural(*notes)
            ),
            Self::OutOfRange(why) => write!(f, "Out of range: {why}."),
        }
    }
}

impl std::error::Error for TextError {}
