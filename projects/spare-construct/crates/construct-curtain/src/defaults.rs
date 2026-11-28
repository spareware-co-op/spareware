//! A small default metric set of curtain wall types, our own: what a new document offers until it
//! keeps its own. Each is made by the same checked constructors as any type, under a permanent ID.
//!
//! - `curtain-1500x1000`, "Curtain wall 1500 x 1000": vertical lines every 1.5 m from the middle,
//!   horizontal lines every 1 m from the base, 24 mm glazing, 50 by 150 mm mullions on every line
//!   and border, the vertical mullions running through.
//! - `curtain-storefront`, "Storefront": the fewest equal panels none wider than 1.2 m along the
//!   wall, no horizontal lines, 24 mm glazing, 50 by 100 mm mullions on the vertical lines and
//!   every border, the vertical mullions running through.
//! - `curtain-glazing`, "Glazing": one pane with no grid and no mullions, 12 mm.

use crate::error::CurtainError;
use crate::grid::{Justify, Layout};
use crate::spec::{Crossing, CurtainSpec, Infill, Mullions, Panel, Profile};
use construct_materials::MaterialId;
use construct_params::Length;
use kit_core::Id;

/// One default curtain wall type: its permanent ID, its name and its grid and parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultCurtain {
    /// Its permanent ID.
    pub id: Id,
    /// Its name.
    pub name: &'static str,
    /// Its grid and parts.
    pub spec: CurtainSpec,
}

/// A material of the default library, by its permanent ID.
fn material(id: &str) -> Result<MaterialId, CurtainError> {
    MaterialId::parse(id).map_err(|e| CurtainError::Malformed(e.to_string()))
}

/// A permanent ID of this set.
fn id(text: &str) -> Result<Id, CurtainError> {
    Id::parse(text).map_err(|_| CurtainError::Malformed(format!("{text:?} is not an ID")))
}

/// A mullion of `width` by `depth` whole millimetres, centred on the wall's line.
fn mullion(width: i32, depth: i32) -> Result<Profile, CurtainError> {
    Profile::rectangle(
        Length::from_mm(width),
        Length::from_mm(depth),
        Length::from_mm(0),
        material("timber-douglas-fir")?,
    )
}

/// Glazing `thickness` whole millimetres thick, centred on the wall's line.
fn glazing(thickness: i32) -> Result<Panel, CurtainError> {
    Ok(Panel::Glazed(Infill::new(
        Length::from_mm(thickness),
        Length::from_mm(0),
        material("glass-plate")?,
    )?))
}

/// The default set, in the order the module header lists it.
///
/// # Errors
/// None for this fixed data; the type is a `Result` because each value is checked.
pub fn default_curtains() -> Result<Vec<DefaultCurtain>, CurtainError> {
    let every = Mullions {
        interior_vertical: Some(mullion(50, 150)?),
        interior_horizontal: Some(mullion(50, 150)?),
        border_vertical: Some(mullion(50, 150)?),
        border_horizontal: Some(mullion(50, 150)?),
    };
    let storefront = Mullions {
        interior_vertical: Some(mullion(50, 100)?),
        interior_horizontal: None,
        border_vertical: Some(mullion(50, 100)?),
        border_horizontal: Some(mullion(50, 100)?),
    };
    Ok(vec![
        DefaultCurtain {
            id: id("curtain-1500x1000")?,
            name: "Curtain wall 1500 x 1000",
            spec: CurtainSpec::new(
                Layout::Spacing {
                    spacing: Length::from_mm(1_500),
                    justify: Justify::Centre,
                },
                Layout::Spacing {
                    spacing: Length::from_mm(1_000),
                    justify: Justify::Start,
                },
                glazing(24)?,
                every,
                Crossing::Vertical,
            )?,
        },
        DefaultCurtain {
            id: id("curtain-storefront")?,
            name: "Storefront",
            spec: CurtainSpec::new(
                Layout::Most {
                    spacing: Length::from_mm(1_200),
                },
                Layout::None,
                glazing(24)?,
                storefront,
                Crossing::Vertical,
            )?,
        },
        DefaultCurtain {
            id: id("curtain-glazing")?,
            name: "Glazing",
            spec: CurtainSpec::new(
                Layout::None,
                Layout::None,
                glazing(12)?,
                Mullions::default(),
                Crossing::Vertical,
            )?,
        },
    ])
}
