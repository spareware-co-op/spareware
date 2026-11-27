//! A curtain wall type's grid and parts as text, in one spelling (FORMAT.md beside this crate). The
//! text is kept in the type's system block under the owner tag [`OWNER`]; construct-types stores
//! it untouched and never reads inside it. A panel is spelt the same here and in a wall's swapped
//! panels ([`crate::Edits`]).

use crate::error::CurtainError;
use crate::grid::{Justify, Layout};
use crate::length;
use crate::spec::{Crossing, CurtainSpec, DoorPanel, Infill, Mullions, Panel, Profile};
use construct_materials::MaterialId;
use serde::{Deserialize, Serialize};

/// The owner tag of a curtain wall type's system block: its props key is `@system.curtain`.
pub const OWNER: &str = "curtain";

/// The version of the text this crate writes. We may stop writing an old version; we never stop
/// reading it.
pub const FORMAT_VERSION: u32 = 1;

/// The text, field for field, in the order it is written.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    vertical: StoredLayout,
    horizontal: StoredLayout,
    panel: StoredPanel,
    mullions: StoredMullions,
    crossing: Crossing,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "layout", rename_all = "lowercase", deny_unknown_fields)]
enum StoredLayout {
    None,
    Spacing { spacing: String, justify: Justify },
    Count { count: u32 },
    Most { spacing: String },
}

/// A panel as it is stored, in a type and in a wall's swaps.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum StoredPanel {
    Glazed {
        thickness: String,
        offset: String,
        material: MaterialId,
    },
    Solid {
        thickness: String,
        offset: String,
        material: MaterialId,
    },
    Empty,
    Door {
        family: String,
        #[serde(rename = "type")]
        ty: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredMullions {
    #[serde(rename = "interior-vertical")]
    interior_vertical: Option<StoredProfile>,
    #[serde(rename = "interior-horizontal")]
    interior_horizontal: Option<StoredProfile>,
    #[serde(rename = "border-vertical")]
    border_vertical: Option<StoredProfile>,
    #[serde(rename = "border-horizontal")]
    border_horizontal: Option<StoredProfile>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Shape {
    Rectangle,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProfile {
    shape: Shape,
    width: String,
    depth: String,
    offset: String,
    material: MaterialId,
}

/// A stored length, or the refusal naming what it is.
fn read(text: &str, what: &str) -> Result<construct_params::Length, CurtainError> {
    length::read(text).ok_or_else(|| {
        CurtainError::Malformed(format!("the {what} {text:?} is not a stored length"))
    })
}

fn layout_to_stored(layout: Layout) -> StoredLayout {
    match layout {
        Layout::None => StoredLayout::None,
        Layout::Spacing { spacing, justify } => StoredLayout::Spacing {
            spacing: length::write(spacing),
            justify,
        },
        Layout::Count { count } => StoredLayout::Count { count },
        Layout::Most { spacing } => StoredLayout::Most {
            spacing: length::write(spacing),
        },
    }
}

fn layout_from_stored(stored: &StoredLayout) -> Result<Layout, CurtainError> {
    Ok(match stored {
        StoredLayout::None => Layout::None,
        StoredLayout::Spacing { spacing, justify } => Layout::Spacing {
            spacing: read(spacing, "spacing")?,
            justify: *justify,
        },
        StoredLayout::Count { count } => Layout::Count { count: *count },
        StoredLayout::Most { spacing } => Layout::Most {
            spacing: read(spacing, "spacing")?,
        },
    })
}

/// A panel as it is stored.
pub(crate) fn panel_to_stored(panel: &Panel) -> StoredPanel {
    match panel {
        Panel::Glazed(infill) => StoredPanel::Glazed {
            thickness: length::write(infill.thickness()),
            offset: length::write(infill.offset()),
            material: infill.material().clone(),
        },
        Panel::Solid(infill) => StoredPanel::Solid {
            thickness: length::write(infill.thickness()),
            offset: length::write(infill.offset()),
            material: infill.material().clone(),
        },
        Panel::Empty => StoredPanel::Empty,
        Panel::Door(door) => StoredPanel::Door {
            family: door.family().as_str().to_owned(),
            ty: door.ty().as_str().to_owned(),
        },
    }
}

/// A panel read from its stored form, checked.
pub(crate) fn panel_from_stored(stored: &StoredPanel) -> Result<Panel, CurtainError> {
    Ok(match stored {
        StoredPanel::Glazed {
            thickness,
            offset,
            material,
        } => Panel::Glazed(Infill::new(
            read(thickness, "panel thickness")?,
            read(offset, "panel offset")?,
            material.clone(),
        )?),
        StoredPanel::Solid {
            thickness,
            offset,
            material,
        } => Panel::Solid(Infill::new(
            read(thickness, "panel thickness")?,
            read(offset, "panel offset")?,
            material.clone(),
        )?),
        StoredPanel::Empty => Panel::Empty,
        StoredPanel::Door { family, ty } => Panel::Door(DoorPanel::new(family, ty)?),
    })
}

fn profile_to_stored(profile: Option<&Profile>) -> Option<StoredProfile> {
    profile.map(|p| StoredProfile {
        shape: Shape::Rectangle,
        width: length::write(p.width()),
        depth: length::write(p.depth()),
        offset: length::write(p.offset()),
        material: p.material().clone(),
    })
}

fn profile_from_stored(stored: Option<&StoredProfile>) -> Result<Option<Profile>, CurtainError> {
    stored
        .map(|p| match p.shape {
            Shape::Rectangle => Profile::rectangle(
                read(&p.width, "mullion width")?,
                read(&p.depth, "mullion depth")?,
                read(&p.offset, "mullion offset")?,
                p.material.clone(),
            ),
        })
        .transpose()
}

impl CurtainSpec {
    /// Its text: compact JSON with the fields in a fixed order, so the same grid and parts always
    /// give the same bytes.
    ///
    /// # Errors
    /// [`CurtainError::Malformed`] only if the JSON writer fails, which it does not for this data.
    pub fn to_text(&self) -> Result<String, CurtainError> {
        let mullions = self.mullions();
        let stored = Stored {
            version: FORMAT_VERSION,
            vertical: layout_to_stored(self.layout(crate::Direction::Vertical)),
            horizontal: layout_to_stored(self.layout(crate::Direction::Horizontal)),
            panel: panel_to_stored(self.panel()),
            mullions: StoredMullions {
                interior_vertical: profile_to_stored(mullions.interior_vertical.as_ref()),
                interior_horizontal: profile_to_stored(mullions.interior_horizontal.as_ref()),
                border_vertical: profile_to_stored(mullions.border_vertical.as_ref()),
                border_horizontal: profile_to_stored(mullions.border_horizontal.as_ref()),
            },
            crossing: self.crossing(),
        };
        serde_json::to_string(&stored).map_err(|e| CurtainError::Malformed(e.to_string()))
    }

    /// Reads the text [`CurtainSpec::to_text`] writes. Anything else is refused, never repaired:
    /// other spacing, field order, a field missing or added, a version this crate does not know,
    /// or a grid or part the checks refuse.
    ///
    /// # Errors
    /// [`CurtainError::Malformed`] for text not in its one spelling; any refusal of the checks
    /// ([`CurtainSpec::new`], [`Profile::rectangle`], [`Infill::new`], [`DoorPanel::new`]).
    pub fn from_text(text: &str) -> Result<CurtainSpec, CurtainError> {
        let stored: Stored =
            serde_json::from_str(text).map_err(|e| CurtainError::Malformed(e.to_string()))?;
        if stored.version != FORMAT_VERSION {
            return Err(CurtainError::Malformed(format!(
                "version {} is not one this crate reads",
                stored.version
            )));
        }
        let mullions = Mullions {
            interior_vertical: profile_from_stored(stored.mullions.interior_vertical.as_ref())?,
            interior_horizontal: profile_from_stored(stored.mullions.interior_horizontal.as_ref())?,
            border_vertical: profile_from_stored(stored.mullions.border_vertical.as_ref())?,
            border_horizontal: profile_from_stored(stored.mullions.border_horizontal.as_ref())?,
        };
        let spec = CurtainSpec::new(
            layout_from_stored(&stored.vertical)?,
            layout_from_stored(&stored.horizontal)?,
            panel_from_stored(&stored.panel)?,
            mullions,
            stored.crossing,
        )?;
        if spec.to_text()? != text {
            return Err(CurtainError::Malformed(
                "the text is not in its one spelling".to_owned(),
            ));
        }
        Ok(spec)
    }
}
