//! Why a modify was refused, in words a person reads, each naming the element it is about. A
//! refusal writes nothing: the command that asked is turned back whole.

use construct_params::{Angle, Definition, Holder, Kind, Units, Value};
use kit_core::Id;
use std::fmt;

/// A refusal of the modify tools.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModifyError {
    /// No element was named.
    Nothing,
    /// An element named is not in the model.
    Missing(Id),
    /// An element of a kind the modify tools do not move: a type, a level, a view, a sheet.
    NotMoved {
        /// The element's name.
        element: String,
        /// Its kind.
        kind: String,
    },
    /// A point of an element would lie between the length grid's steps.
    OffGrid {
        /// The element's name.
        element: String,
    },
    /// A number grew past what is held exactly.
    TooLarge,
    /// An angle that is not a whole number of quarter turns.
    AngleOffGrid {
        /// The angle asked for.
        angle: Angle,
        /// The quarter turn below it.
        below: Angle,
        /// The quarter turn above it.
        above: Angle,
    },
    /// A turn from one direction to another whose lengths do not multiply to a whole number.
    TurnNotWhole,
    /// A direction of no length.
    NoDirection,
    /// A mirror line given by one point twice.
    LineOnePoint,
    /// A block turned or mirrored other than by a quarter turn or about a line at 45 degrees.
    BoxTurns {
        /// The element's name.
        element: String,
    },
    /// An older box wall or door, which runs only along x or y as drawn: it moves, and does not
    /// turn or mirror.
    OlderBox {
        /// The element's name.
        element: String,
    },
    /// An element placed in whole millimetres moved to a point that is not one.
    NotWholeMm {
        /// The element's name.
        element: String,
    },
    /// An arc grid turned or mirrored other than by a quarter turn or about a line at 45 degrees.
    ArcTurns {
        /// The element's name.
        element: String,
    },
    /// A hosted element moved alone, off the line of its host.
    OffItsHost {
        /// The element's name.
        element: String,
        /// Its host's name.
        host: String,
    },
    /// A hosted element turned or mirrored alone.
    TurnsWithHost {
        /// The element's name.
        element: String,
        /// Its host's name.
        host: String,
    },
    /// A hosted element moved alone on a host that runs at an angle.
    HostAtAngle {
        /// The element's name.
        element: String,
        /// Its host's name.
        host: String,
    },
    /// An older door, hosted by where it was placed and not by offsets.
    ByPosition {
        /// The element's name.
        element: String,
    },
    /// The crate that owns an element's stored form refused the result, in its own words.
    Refused {
        /// The element's name.
        element: String,
        /// Why, in that crate's words.
        why: String,
    },
    /// The key a command names for its copies makes an ID that is not one.
    CopyKey {
        /// The key.
        key: String,
    },
    /// A copy's ID that an element of the model already has.
    Taken {
        /// The ID.
        id: Id,
    },
    /// A copy, or an array, that would put each copy on its original.
    InPlace,
    /// An array of fewer than two.
    TooFew,
    /// An array that would make more records than one command writes.
    TooMany {
        /// The most records one command makes.
        most: usize,
    },
    /// A linear array spread over a span that does not split evenly on the length grid.
    Span {
        /// How many it was asked for, the originals counted.
        count: u32,
        /// The nearest smaller count that splits evenly, if any.
        below: Option<u32>,
        /// The nearest larger count that splits evenly, if any.
        above: Option<u32>,
    },
    /// A radial array spread over a sweep that does not split into whole angle units.
    Sweep {
        /// How many it was asked for.
        count: u32,
    },
    /// A radial array whose copies come round onto the original.
    RoundAgain {
        /// How many it was asked for.
        count: u32,
        /// The most it can be at its angle.
        most: u32,
    },
}

/// An angle as a person reads it: whole degrees as they are (`90°`), any other in
/// construct-params' display of angles.
fn angle_text(angle: Angle) -> String {
    let degrees = angle.degrees();
    if degrees.is_integer() {
        return format!("{}°", degrees.numer());
    }
    let shown = Id::parse("modify-angle").ok().and_then(|id| {
        let definition = Definition::new(id, "Angle", Kind::Angle, Holder::Instance).ok()?;
        definition
            .format(&Value::Angle(angle), &Units::default())
            .ok()
    });
    shown.unwrap_or_else(|| format!("{} angle units", angle.units()))
}

impl fmt::Display for ModifyError {
    #[allow(
        clippy::too_many_lines,
        reason = "one sentence for each refusal, in the order they are listed"
    )]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nothing => write!(f, "Which elements? Name at least one."),
            Self::Missing(id) => write!(f, "There is no element {id} in this model."),
            Self::NotMoved { element, kind } => write!(
                f,
                "{element} is a {kind}, which the modify tools do not move: change it with its own \
                 commands."
            ),
            Self::OffGrid { element } => write!(
                f,
                "{element} would not land on the length grid: a point of it would lie between two \
                 of its steps."
            ),
            Self::TooLarge => write!(
                f,
                "That motion grows past the numbers held exactly: make it in fewer steps."
            ),
            Self::AngleOffGrid {
                angle,
                below,
                above,
            } => write!(
                f,
                "A turn of {} does not land on the length grid: only a whole number of quarter \
                 turns does, and the nearest are {} and {}. To turn by another angle, turn from \
                 one direction to another whose lengths are whole (3 to 4 to 5, say).",
                angle_text(*angle),
                angle_text(*below),
                angle_text(*above)
            ),
            Self::TurnNotWhole => write!(
                f,
                "That turn does not land on the length grid: the two directions' lengths do not \
                 multiply to a whole number (3 to 4 to 5 does)."
            ),
            Self::NoDirection => write!(f, "A direction needs a length."),
            Self::LineOnePoint => write!(
                f,
                "A mirror line runs through two different points."
            ),
            Self::BoxTurns { element } => write!(
                f,
                "{element} is a box along x and y: it turns by quarter turns and mirrors about \
                 lines along x or y or at 45 degrees to them."
            ),
            Self::OlderBox { element } => write!(
                f,
                "{element} is an older box, drawn along x or y: it moves, and is drawn again to \
                 turn or mirror it."
            ),
            Self::NotWholeMm { element } => write!(
                f,
                "{element} is placed in whole millimetres, and this would put it between two."
            ),
            Self::ArcTurns { element } => write!(
                f,
                "{element} is an arc: it turns by quarter turns and mirrors about lines along x or \
                 y or at 45 degrees to them, so its angles stay exact."
            ),
            Self::OffItsHost { element, host } => write!(
                f,
                "{element} is hosted on {host}: alone it moves only along {host}, and this would \
                 take it off. Move {host} with it, or move it along {host}."
            ),
            Self::TurnsWithHost { element, host } => write!(
                f,
                "{element} is hosted on {host}: it turns and mirrors with {host}, not alone."
            ),
            Self::HostAtAngle { element, host } => write!(
                f,
                "{element} is hosted on {host}, which runs at an angle: move {host} with it."
            ),
            Self::ByPosition { element } => write!(
                f,
                "{element} is hosted by where it was placed, as older doors were: move it along \
                 its host once with hosted.move, and it moves with its host from then on."
            ),
            Self::Refused { element, why } => write!(f, "{element}: {why}"),
            Self::CopyKey { key } => write!(
                f,
                "The copies' key {key:?} does not make an ID for each copy: give a shorter key of \
                 letters, digits, '.', '_' and '-'."
            ),
            Self::Taken { id } => write!(
                f,
                "An element {id} is already in this model: give the copies another key."
            ),
            Self::InPlace => write!(f, "That would put every copy on its original."),
            Self::TooFew => write!(
                f,
                "An array makes at least two, the originals counted."
            ),
            Self::TooMany { most } => write!(
                f,
                "That would make more than {most} elements at once: make the array in parts."
            ),
            Self::Span {
                count,
                below,
                above,
            } => {
                write!(
                    f,
                    "That span does not split into {} equal steps on the length grid",
                    count.saturating_sub(1)
                )?;
                match (below, above) {
                    (Some(b), Some(a)) => write!(f, ": {b} or {a} in all would."),
                    (Some(n), None) | (None, Some(n)) => write!(f, ": {n} in all would."),
                    (None, None) => write!(f, "."),
                }
            }
            Self::Sweep { count } => write!(
                f,
                "That sweep does not split into {} equal turns of whole angle units.",
                count.saturating_sub(1)
            ),
            Self::RoundAgain { count, most } => write!(
                f,
                "{count} at that angle come round onto the original: at most {most} fit."
            ),
        }
    }
}

impl std::error::Error for ModifyError {}
