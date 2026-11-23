//! Every refusal this crate makes, as a value the caller reads. Each names the element it is about
//! (an opening or a shaft, by its permanent ID) and, where there is one, the element it meets (its
//! floor, another opening), and says why in words a person reads. A refusal changes nothing.
//! [`OpeningError::code`] is the short code a built row and a command's reply carry beside the
//! words.

use std::fmt;

/// What an element is, as a refusal names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum What {
    /// An opening through a wall or a floor.
    Opening,
    /// A shaft.
    Shaft,
}

/// An element a refusal names: what it is and its permanent ID, shown `opening O-1`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Named {
    /// What it is.
    pub what: What,
    /// Its permanent ID.
    pub key: String,
}

impl Named {
    /// The opening `key`.
    #[must_use]
    pub fn opening(key: &str) -> Self {
        Self {
            what: What::Opening,
            key: key.to_owned(),
        }
    }

    /// The shaft `key`.
    #[must_use]
    pub fn shaft(key: &str) -> Self {
        Self {
            what: What::Shaft,
            key: key.to_owned(),
        }
    }
}

impl fmt::Display for Named {
    /// `opening O-1`, or `this opening` for an element whose ID the caller does not have (a
    /// builder, handed a recipe alone).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.what {
            What::Opening => "opening",
            What::Shaft => "shaft",
        };
        if self.key.is_empty() {
            write!(f, "this {what}")
        } else {
            write!(f, "{what} {}", self.key)
        }
    }
}

/// Which end of a shaft.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum End {
    /// Its base.
    Base,
    /// Its top.
    Top,
}

impl fmt::Display for End {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base => write!(f, "base"),
            Self::Top => write!(f, "top"),
        }
    }
}

/// Why an opening or a shaft was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpeningError {
    /// A recipe that is not this kind of opening's, or of a version this reader does not know.
    NotAnOpening {
        /// The element.
        element: Named,
        /// Its recipe's operation and version, as `opening.wall v2`.
        op: String,
    },
    /// A recipe or record not in the stored form (FORMAT.md beside this crate).
    Malformed {
        /// The element.
        element: Named,
        /// What is wrong, in words.
        why: String,
    },
    /// A width or a height of zero or less.
    NoSize {
        /// The element.
        element: Named,
        /// `width` or `height`.
        what: &'static str,
    },
    /// An outline with a curved edge. The engine seam's extrusion takes polygons only, so a curved
    /// opening cannot be cut exactly yet; it is refused until it can (the interim register).
    Curved {
        /// The element.
        element: Named,
    },
    /// An outline of fewer than three corners.
    TooFewCorners {
        /// The element.
        element: Named,
        /// How many it has.
        corners: usize,
    },
    /// Two corners of an outline one after the other in one place: an edge of no length.
    CornersTogether {
        /// The element.
        element: Named,
    },
    /// An outline whose edges cross, touch or run over each other.
    CrossesItself {
        /// The element.
        element: Named,
    },
    /// An opening wholly outside its floor.
    OutsideHost {
        /// The element.
        element: Named,
        /// Its floor, as the words name it (`floor F-1`).
        host: String,
    },
    /// An opening or a shaft across the edge of a floor, or touching it.
    AcrossEdge {
        /// The element.
        element: Named,
        /// The floor, as the words name it (`floor F-1`).
        host: String,
    },
    /// Two openings or shafts overlapping or touching in one floor.
    Overlap {
        /// The first, in the order of their keys.
        element: Named,
        /// The second.
        other: Named,
    },
    /// An opening or a shaft over a hole of its floor's own boundary, or touching one.
    OverHole {
        /// The element.
        element: Named,
        /// The floor, as the words name it (`floor F-1`).
        host: String,
    },
    /// A shaft that takes the whole of a floor.
    TakesFloor {
        /// The shaft.
        element: Named,
        /// The floor, as the words name it (`floor F-1`).
        host: String,
    },
    /// One element handed to its host twice.
    Twice {
        /// The element.
        element: Named,
    },
    /// A shaft whose top is below its base.
    TopBelowBase {
        /// The shaft.
        element: Named,
    },
    /// A shaft whose top is at its base.
    NoHeight {
        /// The shaft.
        element: Named,
    },
    /// A shaft that ends inside a floor's thickness: it would cut part of the floor's layers.
    EndsInFloor {
        /// The shaft.
        element: Named,
        /// The floor, as the words name it (`floor F-1`).
        floor: String,
        /// Which end.
        end: End,
    },
    /// A corner or a length beyond the kernel's grid.
    OutOfRange {
        /// The element.
        element: Named,
    },
}

impl OpeningError {
    /// The short code a built row and a command's reply carry beside the words.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotAnOpening { .. } => "NotAnOpening",
            Self::Malformed { .. } => "BadOpening",
            Self::NoSize { .. } => "EmptyShape",
            Self::Curved { .. } => "CurvedOutline",
            Self::TooFewCorners { .. } | Self::CornersTogether { .. } => "BadOutline",
            Self::CrossesItself { .. } => "OutlineCrossesItself",
            Self::OutsideHost { .. } => "OpeningOutsideHost",
            Self::AcrossEdge { .. } => "OpeningAcrossEdge",
            Self::Overlap { .. } => "OpeningsOverlap",
            Self::OverHole { .. } => "OpeningOverHole",
            Self::TakesFloor { .. } => "ShaftTakesFloor",
            Self::Twice { .. } => "DuplicateOpening",
            Self::TopBelowBase { .. } => "ShaftTopBelowBase",
            Self::NoHeight { .. } => "ShaftNoHeight",
            Self::EndsInFloor { .. } => "ShaftEndsInFloor",
            Self::OutOfRange { .. } => "TooBig",
        }
    }

    /// The element the refusal is about.
    #[must_use]
    pub fn element(&self) -> &Named {
        match self {
            Self::NotAnOpening { element, .. }
            | Self::Malformed { element, .. }
            | Self::NoSize { element, .. }
            | Self::Curved { element }
            | Self::TooFewCorners { element, .. }
            | Self::CornersTogether { element }
            | Self::CrossesItself { element }
            | Self::OutsideHost { element, .. }
            | Self::AcrossEdge { element, .. }
            | Self::Overlap { element, .. }
            | Self::OverHole { element, .. }
            | Self::TakesFloor { element, .. }
            | Self::Twice { element }
            | Self::TopBelowBase { element }
            | Self::NoHeight { element }
            | Self::EndsInFloor { element, .. }
            | Self::OutOfRange { element } => element,
        }
    }
}

impl fmt::Display for OpeningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnOpening { element, op } => {
                write!(
                    f,
                    "{element}: a recipe of {op} is not one this reader knows"
                )
            }
            Self::Malformed { element, why } => write!(f, "{element}: {why}"),
            Self::NoSize { element, what } => write!(f, "{element}: its {what} must be above 0"),
            Self::Curved { element } => write!(
                f,
                "{element}: its outline has a curved edge, and a curved opening cannot be cut \
                 exactly yet: draw it with straight edges"
            ),
            Self::TooFewCorners { element, corners } => write!(
                f,
                "{element}: its outline has {corners} corners; an outline needs at least three"
            ),
            Self::CornersTogether { element } => write!(
                f,
                "{element}: two corners of its outline, one after the other, are in one place"
            ),
            Self::CrossesItself { element } => write!(
                f,
                "{element}: its outline crosses or touches itself; draw it as one loop that \
                 does not"
            ),
            Self::OutsideHost { element, host } => write!(
                f,
                "{element} lies outside {host}: an opening is cut wholly inside the floor \
                 it is in"
            ),
            Self::AcrossEdge { element, host } => write!(
                f,
                "{element} crosses or touches the edge of {host}: it must lie wholly \
                 inside the floor, clear of its edge"
            ),
            Self::Overlap { element, other } => {
                write!(
                    f,
                    "{element} and {other} overlap or touch; move one clear of the other"
                )
            }
            Self::OverHole { element, host } => write!(
                f,
                "{element} overlaps or touches a hole of {host}'s own boundary; move it \
                 clear of the hole"
            ),
            Self::TakesFloor { element, host } => write!(
                f,
                "{element} takes the whole of {host}: a shaft cuts a hole in a floor, not \
                 the floor away"
            ),
            Self::Twice { element } => write!(f, "{element} is in its floor twice"),
            Self::TopBelowBase { element } => write!(
                f,
                "{element}: its top is below its base; tie its top to a level above its base"
            ),
            Self::NoHeight { element } => write!(
                f,
                "{element}: its top is at its base, so it has no height; tie its top to a level \
                 above its base"
            ),
            Self::EndsInFloor {
                element,
                floor,
                end,
            } => write!(
                f,
                "{element}: its {end} lies inside the thickness of {floor}; a shaft passes \
                 wholly through a floor or misses it, so move its {end} to the floor's top or \
                 bottom face or past it"
            ),
            Self::OutOfRange { element } => {
                write!(
                    f,
                    "{element}: a corner or length is beyond the kernel's grid"
                )
            }
        }
    }
}

impl std::error::Error for OpeningError {}
