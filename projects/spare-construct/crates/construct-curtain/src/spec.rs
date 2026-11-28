//! What a curtain wall type says, as data: its grid in each direction ([`Layout`]), the panel each
//! cell holds unless a wall swaps it ([`Panel`]), the mullions on its interior grid lines and on
//! its borders ([`Mullions`], each a [`Profile`]), and which mullion runs through where two cross
//! ([`Crossing`]). Every value here has passed its checks, so a type's layout can be made for any
//! wall it is drawn as.
//!
//! The frame every size is given in: along the wall from its start, across it (positive toward
//! its exterior, the right hand walking from its start to its end, as a wall's), and up from its
//! base. A mullion's `width` is its size along the face it stands on (along the wall for a
//! vertical mullion, up the wall for a horizontal one), its `depth` its size across the wall, and
//! its `offset` where its middle lies across, from the wall's line. A panel's `thickness` is
//! across, centred on its `offset`.

use crate::error::CurtainError;
use crate::grid::{Direction, Layout};
use crate::length::{self, ZERO};
use construct_families::{ContentId, Key};
use construct_materials::MaterialId;
use construct_params::Length;
use serde::{Deserialize, Serialize};

/// The largest a mullion's width or depth, a panel's thickness or an offset across may be: 2 m.
pub const MOST_SIZE: Length = Length::from_mm(2_000);

/// Checks a size that stands centred: above zero, at most [`MOST_SIZE`], halving on the grid.
fn centred(what: &str, value: Length) -> Result<(), CurtainError> {
    if value <= ZERO {
        return Err(CurtainError::NotPositive {
            what: what.to_owned(),
            value,
        });
    }
    if value > MOST_SIZE {
        return Err(CurtainError::TooLarge {
            what: what.to_owned(),
            value,
            most: MOST_SIZE,
        });
    }
    if length::half(value).is_none() {
        return Err(CurtainError::OddSteps {
            what: what.to_owned(),
            value,
        });
    }
    Ok(())
}

/// Checks an offset across: at most [`MOST_SIZE`] either way.
fn across(what: &str, value: Length) -> Result<(), CurtainError> {
    let size = if value < ZERO {
        length::negated(value).ok_or(CurtainError::OutOfRange)?
    } else {
        value
    };
    if size > MOST_SIZE {
        return Err(CurtainError::TooLarge {
            what: what.to_owned(),
            value,
            most: MOST_SIZE,
        });
    }
    Ok(())
}

/// From `offset - size / 2` to `offset + size / 2`: where a centred size lies across.
fn span(offset: Length, size: Length) -> Result<[Length; 2], CurtainError> {
    let half = length::half(size).ok_or(CurtainError::OutOfRange)?;
    Ok([
        length::difference(offset, half).ok_or(CurtainError::OutOfRange)?,
        length::sum(offset, half).ok_or(CurtainError::OutOfRange)?,
    ])
}

/// A mullion's cross-section: a rectangle, its width along the face it stands on and its depth
/// across the wall, its middle `offset` across from the wall's line, and its material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    width: Length,
    depth: Length,
    offset: Length,
    material: MaterialId,
}

impl Profile {
    /// A rectangular profile.
    ///
    /// # Errors
    /// [`CurtainError::NotPositive`], [`CurtainError::TooLarge`] or [`CurtainError::OddSteps`] for
    /// a width or a depth at or below 0, above [`MOST_SIZE`] or not halving on the grid;
    /// [`CurtainError::TooLarge`] for an offset beyond [`MOST_SIZE`] either way.
    pub fn rectangle(
        width: Length,
        depth: Length,
        offset: Length,
        material: MaterialId,
    ) -> Result<Profile, CurtainError> {
        centred("mullion width", width)?;
        centred("mullion depth", depth)?;
        across("mullion offset", offset)?;
        Ok(Profile {
            width,
            depth,
            offset,
            material,
        })
    }

    /// Its width, along the face it stands on.
    #[must_use]
    pub fn width(&self) -> Length {
        self.width
    }

    /// Its depth, across the wall.
    #[must_use]
    pub fn depth(&self) -> Length {
        self.depth
    }

    /// Where its middle lies across, from the wall's line toward its exterior.
    #[must_use]
    pub fn offset(&self) -> Length {
        self.offset
    }

    /// Its material.
    #[must_use]
    pub fn material(&self) -> &MaterialId {
        &self.material
    }

    /// Where it lies across: from its inner face to its outer face.
    ///
    /// # Errors
    /// [`CurtainError::OutOfRange`] never, for a checked profile.
    pub fn across(&self) -> Result<[Length; 2], CurtainError> {
        span(self.offset, self.depth)
    }
}

/// A panel that fills its cell: a glazed or solid slab, its thickness centred on its offset
/// across, of its material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Infill {
    thickness: Length,
    offset: Length,
    material: MaterialId,
}

impl Infill {
    /// A slab of `thickness`, its middle `offset` across, of `material`.
    ///
    /// # Errors
    /// As [`Profile::rectangle`], for the thickness and the offset.
    pub fn new(
        thickness: Length,
        offset: Length,
        material: MaterialId,
    ) -> Result<Infill, CurtainError> {
        centred("panel thickness", thickness)?;
        across("panel offset", offset)?;
        Ok(Infill {
            thickness,
            offset,
            material,
        })
    }

    /// Its thickness, across the wall.
    #[must_use]
    pub fn thickness(&self) -> Length {
        self.thickness
    }

    /// Where its middle lies across.
    #[must_use]
    pub fn offset(&self) -> Length {
        self.offset
    }

    /// Its material.
    #[must_use]
    pub fn material(&self) -> &MaterialId {
        &self.material
    }

    /// Where it lies across: from its inner face to its outer face.
    ///
    /// # Errors
    /// [`CurtainError::OutOfRange`] never, for a checked slab.
    pub fn across(&self) -> Result<[Length; 2], CurtainError> {
        span(self.offset, self.thickness)
    }
}

/// A door from the family library standing in a cell: the family recipe version by its content
/// id and one of its types by its key. The door is sized to its panel (its family's `width` and
/// `height` parameters set to the panel's), stands on the panel's foot at its middle, and is
/// centred across on the wall's line, as a door hosted in a wall is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoorPanel {
    family: ContentId,
    ty: Key,
}

impl DoorPanel {
    /// A door panel of the family `family` (a content id, `sha256:...`) and its type `ty`.
    ///
    /// # Errors
    /// [`CurtainError::BadDoor`] for a family that is not a content id or a type that is not a
    /// key.
    pub fn new(family: &str, ty: &str) -> Result<DoorPanel, CurtainError> {
        let family = ContentId::parse(family).ok_or_else(|| {
            CurtainError::BadDoor(format!(
                "{family:?} is not a family recipe's content id (sha256:...)"
            ))
        })?;
        let ty = Key::parse(ty)
            .ok_or_else(|| CurtainError::BadDoor(format!("{ty:?} is not a type's key")))?;
        Ok(DoorPanel { family, ty })
    }

    /// The family recipe version.
    #[must_use]
    pub fn family(&self) -> &ContentId {
        &self.family
    }

    /// The type, by its key in the family recipe.
    #[must_use]
    pub fn ty(&self) -> &Key {
        &self.ty
    }
}

/// What a cell holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Panel {
    /// Glass.
    Glazed(Infill),
    /// An opaque slab.
    Solid(Infill),
    /// Nothing: an open cell between its mullions.
    Empty,
    /// A door of the family library.
    Door(DoorPanel),
}

/// A panel's kind, as a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelKind {
    /// Glass.
    Glazed,
    /// An opaque slab.
    Solid,
    /// Nothing.
    Empty,
    /// A door.
    Door,
}

impl PanelKind {
    /// Its stored and typed word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            PanelKind::Glazed => "glazed",
            PanelKind::Solid => "solid",
            PanelKind::Empty => "empty",
            PanelKind::Door => "door",
        }
    }
}

impl Panel {
    /// Its kind.
    #[must_use]
    pub fn kind(&self) -> PanelKind {
        match self {
            Panel::Glazed(_) => PanelKind::Glazed,
            Panel::Solid(_) => PanelKind::Solid,
            Panel::Empty => PanelKind::Empty,
            Panel::Door(_) => PanelKind::Door,
        }
    }

    /// Its slab, a glazed or solid panel's.
    #[must_use]
    pub fn infill(&self) -> Option<&Infill> {
        match self {
            Panel::Glazed(infill) | Panel::Solid(infill) => Some(infill),
            Panel::Empty | Panel::Door(_) => None,
        }
    }
}

/// Which mullion runs through where a vertical and a horizontal mullion cross; the other stops at
/// its faces. Where the one that runs through has no mullion at a crossing, the other runs
/// through there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Crossing {
    /// The vertical mullions run through.
    Vertical,
    /// The horizontal mullions run through.
    Horizontal,
}

impl Crossing {
    /// The direction that runs through.
    #[must_use]
    pub fn direction(self) -> Direction {
        match self {
            Crossing::Vertical => Direction::Vertical,
            Crossing::Horizontal => Direction::Horizontal,
        }
    }
}

/// The mullions of a type: one profile, or none, for each of four kinds of line.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Mullions {
    /// On the interior vertical grid lines.
    pub interior_vertical: Option<Profile>,
    /// On the interior horizontal grid lines.
    pub interior_horizontal: Option<Profile>,
    /// On the two vertical borders, at the wall's start and its end.
    pub border_vertical: Option<Profile>,
    /// On the two horizontal borders, at the wall's base and its top.
    pub border_horizontal: Option<Profile>,
}

impl Mullions {
    /// The profile on a line of `direction`, a border or an interior line.
    #[must_use]
    pub fn of(&self, direction: Direction, border: bool) -> Option<&Profile> {
        match (direction, border) {
            (Direction::Vertical, false) => self.interior_vertical.as_ref(),
            (Direction::Horizontal, false) => self.interior_horizontal.as_ref(),
            (Direction::Vertical, true) => self.border_vertical.as_ref(),
            (Direction::Horizontal, true) => self.border_horizontal.as_ref(),
        }
    }
}

/// A curtain wall type's grid and parts, every one checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurtainSpec {
    vertical: Layout,
    horizontal: Layout,
    panel: Panel,
    mullions: Mullions,
    crossing: Crossing,
}

impl CurtainSpec {
    /// A type's grid and parts.
    ///
    /// # Errors
    /// [`Layout::check`]'s refusals for either layout.
    pub fn new(
        vertical: Layout,
        horizontal: Layout,
        panel: Panel,
        mullions: Mullions,
        crossing: Crossing,
    ) -> Result<CurtainSpec, CurtainError> {
        vertical.check(Direction::Vertical)?;
        horizontal.check(Direction::Horizontal)?;
        Ok(CurtainSpec {
            vertical,
            horizontal,
            panel,
            mullions,
            crossing,
        })
    }

    /// The layout of its lines of `direction`.
    #[must_use]
    pub fn layout(&self, direction: Direction) -> Layout {
        match direction {
            Direction::Vertical => self.vertical,
            Direction::Horizontal => self.horizontal,
        }
    }

    /// The panel each cell holds unless a wall swaps it.
    #[must_use]
    pub fn panel(&self) -> &Panel {
        &self.panel
    }

    /// Its mullions.
    #[must_use]
    pub fn mullions(&self) -> &Mullions {
        &self.mullions
    }

    /// Which mullion runs through where two cross.
    #[must_use]
    pub fn crossing(&self) -> Crossing {
        self.crossing
    }
}
