//! A small default metric set of wall types for the tests and the first users. Each is made up
//! from common building practice, not taken from any list: an internal timber stud partition
//! lined both sides, a plain concrete wall, and a brick veneer wall over a timber frame with a
//! drained cavity.
//!
//! Materials are construct-materials' default IDs where one exists. Two that a wall needs are
//! not there yet, and these IDs are used for them, for the materials crate to add with sourced
//! values: [`PLASTERBOARD`] and [`BRICK`]. [`crate::Assembly::missing_materials`] lists the
//! layers whose material a library lacks.

use crate::assembly::Assembly;
use crate::error::WallError;
use crate::layer::{Function, Layer};
use crate::wall_type::new_wall_type;
use construct_categories::CategoryTable;
use construct_materials::MaterialId;
use construct_params::Length;
use construct_types::{ElementType, Parameters};
use kit_core::Id;

/// Gypsum plasterboard (not in construct-materials' defaults yet).
pub const PLASTERBOARD: &str = "plasterboard-gypsum";
/// Fired clay brick (not in construct-materials' defaults yet).
pub const BRICK: &str = "brick-clay";
/// Framing timber: construct-materials' default Douglas-fir.
pub const FRAMING_TIMBER: &str = "timber-douglas-fir";
/// Concrete: construct-materials' default regular concrete.
pub const CONCRETE: &str = "concrete-regular";
/// The air in a cavity: construct-materials' default dry air.
pub const AIR: &str = "air-dry";
/// A building wrap: construct-materials' default polyethylene sheet.
pub const WRAP: &str = "membrane-polyethylene";

/// One default wall type, before it is made an element type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultWall {
    /// Its permanent ID.
    pub id: &'static str,
    /// Its name.
    pub name: &'static str,
    /// Its layers and core.
    pub assembly: Assembly,
}

/// A layer of a default type.
fn layer(function: Function, mm: i64, material: &str, wraps: bool) -> Result<Layer, WallError> {
    Ok(Layer::new(
        function,
        Length::from_mm(mm),
        MaterialId::parse(material)?,
        wraps,
    ))
}

/// The default wall types' layers and cores, in a fixed order:
/// - `wall-partition-stud-90`: 13 mm plasterboard, a 90 mm timber stud frame (the core), 13 mm
///   plasterboard; 116 mm. Both linings wrap.
/// - `wall-concrete-200`: 200 mm concrete, all core; 200 mm.
/// - `wall-brick-veneer-90`: 110 mm brick veneer, a 40 mm drained air cavity, a building wrap
///   (0 mm), a 90 mm timber frame (the core), 10 mm plasterboard; 250 mm. The wrap and the
///   lining wrap; the veneer stops at openings on its own lintel and jambs.
///
/// Plasterboard is often 12.5 mm thick. Today's length unit is a whole millimetre, so the
/// linings here are 13 and 10 mm; once piece A39 gives lengths the kernel's finer unit, the
/// partition's linings become 12.5 mm (the test `a39_partition_lining_is_12_5_mm` says so).
///
/// # Errors
/// None for this fixed data; the type is a `Result` because each layer is checked.
pub fn default_walls() -> Result<Vec<DefaultWall>, WallError> {
    use Function::{Finish, Membrane, Structure, Thermal};
    Ok(vec![
        DefaultWall {
            id: "wall-partition-stud-90",
            name: "Partition, 90 timber stud, 13 plasterboard both sides",
            assembly: Assembly::new(
                vec![
                    layer(Finish, 13, PLASTERBOARD, true)?,
                    layer(Structure, 90, FRAMING_TIMBER, false)?,
                    layer(Finish, 13, PLASTERBOARD, true)?,
                ],
                1,
                2,
            )?,
        },
        DefaultWall {
            id: "wall-concrete-200",
            name: "Concrete 200",
            assembly: Assembly::new(vec![layer(Structure, 200, CONCRETE, false)?], 0, 1)?,
        },
        DefaultWall {
            id: "wall-brick-veneer-90",
            name: "Brick veneer, 40 cavity, 90 timber frame, 10 plasterboard",
            assembly: Assembly::new(
                vec![
                    layer(Finish, 110, BRICK, false)?,
                    layer(Thermal, 40, AIR, false)?,
                    layer(Membrane, 0, WRAP, true)?,
                    layer(Structure, 90, FRAMING_TIMBER, false)?,
                    layer(Finish, 10, PLASTERBOARD, true)?,
                ],
                3,
                4,
            )?,
        },
    ])
}

/// The default wall types as element types, in the order of [`default_walls`]. `params` must
/// hold the wall parameters ([`crate::add_parameters`]).
///
/// # Errors
/// Any refusal of [`new_wall_type`].
pub fn default_wall_types(
    params: &Parameters,
    categories: &CategoryTable,
) -> Result<Vec<ElementType>, WallError> {
    default_walls()?
        .into_iter()
        .map(|wall| {
            let id = Id::parse(wall.id)
                .map_err(|_| WallError::Malformed(format!("{:?} is not an id", wall.id)))?;
            new_wall_type(id, wall.name, &wall.assembly, params, categories)
        })
        .collect()
}
