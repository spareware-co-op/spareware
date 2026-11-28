//! A small default metric set of floor types for the tests and the first users, each made up
//! from common building practice, not taken from any list: a plain 200 mm concrete slab, a
//! 150 mm slab under a 50 mm screed, and a timber floor of 240 mm joists under 20 mm sheet
//! flooring.
//!
//! Materials are construct-materials' default IDs where one exists. Two that a floor needs are not
//! there yet, and these IDs are used for them, for the materials crate to add with sourced values:
//! [`SCREED`] and [`PARTICLEBOARD`]. [`Layers::missing_materials`] lists the layers whose material
//! a library lacks, and a floor's mass is not known while one is missing.
//!
//! The joists are one layer of framing timber, as a floor type with no framed layer can only say
//! it: its volume is the whole depth of the joists, so its mass counts them as solid timber, far
//! more than the joists weigh. A framed layer (members at centres, the gaps air or insulation) is
//! a later version of the type.

use crate::ceiling_type::new_ceiling_type;
use crate::error::SlabError;
use crate::floor_type::new_floor_type;
use crate::layers::{FloorLayer, Function, Layers};
use construct_categories::CategoryTable;
use construct_materials::MaterialId;
use construct_params::Length;
use construct_types::ElementType;
use kit_core::Id;

/// A cement and sand screed (not in construct-materials' defaults yet).
pub const SCREED: &str = "screed-cement";
/// Particleboard sheet flooring (not in construct-materials' defaults yet).
pub const PARTICLEBOARD: &str = "particleboard";
/// Concrete: construct-materials' default regular concrete.
pub const CONCRETE: &str = "concrete-regular";
/// Framing timber: construct-materials' default Douglas-fir.
pub const FRAMING_TIMBER: &str = "timber-douglas-fir";

/// One default floor type, before it is made an element type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultFloor {
    /// Its permanent ID.
    pub id: &'static str,
    /// Its name.
    pub name: &'static str,
    /// Its layers.
    pub layers: Layers,
}

/// A layer of a default type.
fn layer(function: Function, mm: i32, material: &str) -> Result<FloorLayer, SlabError> {
    Ok(FloorLayer::new(
        function,
        Length::from_mm(mm),
        MaterialId::parse(material)?,
    ))
}

/// The default floor types' layers, in a fixed order:
/// - `floor-concrete-200`: 200 mm concrete.
/// - `floor-concrete-150-screed-50`: a 50 mm screed (the finish) on 150 mm concrete.
/// - `floor-timber-joist-240`: 20 mm particleboard flooring (the substrate a finish is laid on)
///   on 240 mm timber joists, as one layer.
///
/// # Errors
/// None for this fixed data; the type is a `Result` because each layer is checked.
pub fn default_floors() -> Result<Vec<DefaultFloor>, SlabError> {
    use Function::{Finish, Structure, Substrate};
    Ok(vec![
        DefaultFloor {
            id: "floor-concrete-200",
            name: "Concrete 200",
            layers: Layers::new(vec![layer(Structure, 200, CONCRETE)?])?,
        },
        DefaultFloor {
            id: "floor-concrete-150-screed-50",
            name: "Concrete 150, 50 screed",
            layers: Layers::new(vec![
                layer(Finish, 50, SCREED)?,
                layer(Structure, 150, CONCRETE)?,
            ])?,
        },
        DefaultFloor {
            id: "floor-timber-joist-240",
            name: "Timber joists 240, 20 particleboard",
            layers: Layers::new(vec![
                layer(Substrate, 20, PARTICLEBOARD)?,
                layer(Structure, 240, FRAMING_TIMBER)?,
            ])?,
        },
    ])
}

/// The default floor types as element types, in the order of [`default_floors`].
///
/// # Errors
/// Any refusal of [`new_floor_type`].
pub fn default_floor_types(categories: &CategoryTable) -> Result<Vec<ElementType>, SlabError> {
    default_floors()?
        .into_iter()
        .map(|floor| {
            let id = Id::parse(floor.id)
                .map_err(|_| SlabError::Malformed(format!("{:?} is not an id", floor.id)))?;
            new_floor_type(id, floor.name, &floor.layers, categories)
        })
        .collect()
}

/// Gypsum plaster: construct-materials' default plaster, the finish of every default ceiling.
pub const PLASTER: &str = "plaster-gypsum";

/// The default ceiling types' layers, in a fixed order (each layer from the ceiling's upper face
/// down, so the last is the finish a person sees):
/// - `ceiling-plaster-13`: 13 mm gypsum plaster.
/// - `ceiling-plaster-13-timber-45`: 13 mm gypsum plaster under 45 mm framing timber.
/// - `ceiling-suspended-tile-15`: a 15 mm suspended tile (gypsum plaster stands in for the tile's
///   material until the library holds mineral fibre), for a 600 by 1200 grid.
///
/// # Errors
/// None for this fixed data; the type is a `Result` because each layer is checked.
pub fn default_ceilings() -> Result<Vec<DefaultFloor>, SlabError> {
    use Function::{Finish, Structure};
    Ok(vec![
        DefaultFloor {
            id: "ceiling-plaster-13",
            name: "Plaster 13",
            layers: Layers::new(vec![layer(Finish, 13, PLASTER)?])?,
        },
        DefaultFloor {
            id: "ceiling-plaster-13-timber-45",
            name: "Plaster 13 on timber 45",
            layers: Layers::new(vec![
                layer(Structure, 45, FRAMING_TIMBER)?,
                layer(Finish, 13, PLASTER)?,
            ])?,
        },
        DefaultFloor {
            id: "ceiling-suspended-tile-15",
            name: "Suspended tile 15",
            layers: Layers::new(vec![layer(Finish, 15, PLASTER)?])?,
        },
    ])
}

/// The default ceiling types as element types, in the order of [`default_ceilings`].
///
/// # Errors
/// Any refusal of [`new_ceiling_type`].
pub fn default_ceiling_types(categories: &CategoryTable) -> Result<Vec<ElementType>, SlabError> {
    default_ceilings()?
        .into_iter()
        .map(|ceiling| {
            let id = Id::parse(ceiling.id)
                .map_err(|_| SlabError::Malformed(format!("{:?} is not an id", ceiling.id)))?;
            new_ceiling_type(id, ceiling.name, &ceiling.layers, categories)
        })
        .collect()
}
