//! A small default metric set of roof types for the tests and the first users, each made up from
//! common building practice, not taken from any list: a sheet steel roof on 150 mm timber rafters
//! with an underlay, and a concrete roof slab of 200 mm under 100 mm of insulation. Every material
//! is one of construct-materials' defaults, each of which has a published density with its source,
//! so a default roof's mass is known.
//!
//! The rafters are one layer of framing timber, as a type with no framed layer can only say it:
//! its volume is the whole depth of the rafters, so its mass counts them as solid timber, far more
//! than the rafters weigh. A framed layer (members at centres, the gaps air or insulation) is a
//! later version of the type, as for a floor's joists.

use crate::error::RoofError;
use crate::roof_type::new_roof_type;
use construct_categories::CategoryTable;
use construct_materials::MaterialId;
use construct_params::Length;
use construct_slabs::{FloorLayer, Function, Layers};
use construct_types::ElementType;
use kit_core::Id;

/// Sheet steel: construct-materials' default carbon steel.
pub const STEEL: &str = "steel-aisi-1025";
/// A roof underlay: construct-materials' default polyethylene sheet.
pub const UNDERLAY: &str = "membrane-polyethylene";
/// Framing timber: construct-materials' default Douglas-fir.
pub const FRAMING_TIMBER: &str = "timber-douglas-fir";
/// Insulation: construct-materials' default foam glass.
pub const INSULATION: &str = "insulation-foam-glass";
/// Concrete: construct-materials' default regular concrete.
pub const CONCRETE: &str = "concrete-regular";

/// One default roof type, before it is made an element type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultRoof {
    /// Its permanent ID.
    pub id: &'static str,
    /// Its name.
    pub name: &'static str,
    /// Its layers, from the top (outer) face down.
    pub layers: Layers,
}

/// A layer of a default type.
fn layer(function: Function, thickness: Length, material: &str) -> Result<FloorLayer, RoofError> {
    Ok(FloorLayer::new(
        function,
        thickness,
        MaterialId::parse(material)?,
    ))
}

/// The default roof types' layers, in a fixed order:
/// - `roof-steel-rafter-150`: 0.55 mm sheet steel (the finish), an underlay (a membrane, 0
///   thick), on 150 mm timber rafters, as one layer.
/// - `roof-concrete-200-insulated`: 100 mm foam glass insulation on a 200 mm concrete slab.
///
/// # Errors
/// None for this fixed data; the type is a `Result` because each layer is checked.
pub fn default_roofs() -> Result<Vec<DefaultRoof>, RoofError> {
    use Function::{Finish, Membrane, Structure, Thermal};
    // 0.55 mm is 3,520 steps of the length grid.
    let sheet = Length::from_steps(3_520);
    Ok(vec![
        DefaultRoof {
            id: "roof-steel-rafter-150",
            name: "Sheet steel on 150 rafters",
            layers: Layers::new(vec![
                layer(Finish, sheet, STEEL)?,
                layer(Membrane, Length::from_mm(0), UNDERLAY)?,
                layer(Structure, Length::from_mm(150), FRAMING_TIMBER)?,
            ])?,
        },
        DefaultRoof {
            id: "roof-concrete-200-insulated",
            name: "Concrete 200, 100 insulation",
            layers: Layers::new(vec![
                layer(Thermal, Length::from_mm(100), INSULATION)?,
                layer(Structure, Length::from_mm(200), CONCRETE)?,
            ])?,
        },
    ])
}

/// The default roof types as element types, in the order of [`default_roofs`].
///
/// # Errors
/// Any refusal of [`new_roof_type`].
pub fn default_roof_types(categories: &CategoryTable) -> Result<Vec<ElementType>, RoofError> {
    default_roofs()?
        .into_iter()
        .map(|roof| {
            let id = Id::parse(roof.id)
                .map_err(|_| RoofError::Malformed(format!("{:?} is not an id", roof.id)))?;
            new_roof_type(id, roof.name, &roof.layers, categories)
        })
        .collect()
}
