//! A wall type as an element type of construct-types, and what each wall instance chooses.
//!
//! A wall type is an [`ElementType`] of the walls category whose system block, owner
//! [`OWNER`], holds its [`Assembly`] as text. Its elements have the two instance parameters
//! below, held by each instance, so every wall type lists them and changing a wall to another
//! wall type keeps both values (construct-types keeps every value the new type also has). The
//! chosen location line therefore stays on the wall's reference line, and the faces of the new
//! layers are laid out round it.
//!
//! - [`LOCATION_LINE`]: a choice of the six [`LocationLine`] keys, default the wall centreline.
//! - [`FLIPPED`]: yes or no, default no; yes puts the exterior on the other side of the line.

use crate::assembly::Assembly;
use crate::error::WallError;
use crate::offsets::{LocationLine, Offsets};
use crate::stored::OWNER;
use construct_categories::{CategoryTable, WALLS};
use construct_params::{Choice, Definition, Holder, Kind, Value};
use construct_types::{ElementType, Instance, Parameters, SystemData, TypeError, resolve};
use kit_core::Id;

/// The permanent ID of the location line parameter, held by each wall instance.
pub const LOCATION_LINE: &str = "wall.location-line";
/// The permanent ID of the flipped parameter, held by each wall instance.
pub const FLIPPED: &str = "wall.flipped";

/// One of this crate's fixed parameter IDs.
fn fixed_id(text: &str) -> Result<Id, WallError> {
    Id::parse(text).map_err(|_| WallError::Malformed(format!("{text:?} is not an id")))
}

/// The value of the location line parameter that chooses `line`.
#[must_use]
pub fn location_line_value(line: LocationLine) -> Value {
    Value::Choice(line.key().to_owned())
}

/// Adds the two wall instance parameters, with their defaults, to a caller's parameters.
///
/// # Errors
/// [`WallError::Type`] if either ID is already defined.
pub fn add_parameters(params: &mut Parameters) -> Result<(), WallError> {
    let choices = LocationLine::ALL
        .iter()
        .map(|line| Choice::new(line.key(), line.label()))
        .collect();
    let line = Definition::choice(
        fixed_id(LOCATION_LINE)?,
        "Location line",
        Holder::Instance,
        choices,
    )
    .map_err(TypeError::from)?;
    let flipped = Definition::new(fixed_id(FLIPPED)?, "Flipped", Kind::YesNo, Holder::Instance)
        .map_err(TypeError::from)?;
    params.add(line, Some(location_line_value(LocationLine::WallCentre)))?;
    params.add(flipped, Some(Value::YesNo(false)))?;
    Ok(())
}

/// A new wall type: of the walls category, with the two wall instance parameters and `assembly`
/// in its system block. `params` must hold the wall parameters ([`add_parameters`]).
///
/// # Errors
/// [`WallError::Type`] for a name or parameter construct-types refuses.
pub fn new_wall_type(
    id: Id,
    name: &str,
    assembly: &Assembly,
    params: &Parameters,
    categories: &CategoryTable,
) -> Result<ElementType, WallError> {
    let mut ty = ElementType::new(id, name, WALLS, categories)?;
    ty.add_parameter(params, &fixed_id(LOCATION_LINE)?)?;
    ty.add_parameter(params, &fixed_id(FLIPPED)?)?;
    with_assembly(ty, assembly)
}

/// The same wall type with `assembly` as its layers and core: how an edit's result is kept.
///
/// # Errors
/// [`WallError::NotAWallType`] if `ty` is not of the walls category.
pub fn with_assembly(ty: ElementType, assembly: &Assembly) -> Result<ElementType, WallError> {
    if ty.category() != WALLS {
        return Err(WallError::NotAWallType {
            category: ty.category(),
        });
    }
    let block = SystemData::new(OWNER, &assembly.to_text()?)?;
    Ok(ty.with_system(block))
}

/// A wall type's layers and core, read from its system block.
///
/// # Errors
/// [`WallError::NotAWallType`] if `ty` is not of the walls category; [`WallError::Malformed`]
/// if it has no wall block or its text is not in its one spelling; any refusal of
/// [`Assembly::new`].
pub fn assembly_of(ty: &ElementType) -> Result<Assembly, WallError> {
    if ty.category() != WALLS {
        return Err(WallError::NotAWallType {
            category: ty.category(),
        });
    }
    match ty.system() {
        Some(block) if block.owner() == OWNER => Assembly::from_text(block.data()),
        _ => Err(WallError::Malformed(format!(
            "a wall type carries a {OWNER:?} system block"
        ))),
    }
}

/// The location line a wall instance of type `ty` has chosen and whether it is flipped, each its
/// own value or else the default.
///
/// # Errors
/// [`WallError::Type`] if `ty` is not the instance's type or lacks the wall parameters;
/// [`WallError::Malformed`] if a value is not one the wall parameters take.
pub fn placement(
    params: &Parameters,
    ty: &ElementType,
    instance: &Instance,
) -> Result<(LocationLine, bool), WallError> {
    let line = resolve(params, ty, instance, &fixed_id(LOCATION_LINE)?)?;
    let line = match line.value {
        Some(Value::Choice(key)) => LocationLine::from_key(key),
        _ => None,
    }
    .ok_or_else(|| WallError::Malformed(format!("{LOCATION_LINE} is not a location line")))?;
    let flipped = match resolve(params, ty, instance, &fixed_id(FLIPPED)?)?.value {
        Some(Value::YesNo(flipped)) => *flipped,
        _ => return Err(WallError::Malformed(format!("{FLIPPED} is not yes or no"))),
    };
    Ok((line, flipped))
}

/// Where a wall instance's layer faces and location lines lie from its reference line: its
/// type's layers laid out round its chosen location line, on its chosen side.
///
/// # Errors
/// Any refusal of [`placement`], [`assembly_of`] or [`Assembly::offsets`].
pub fn offsets_of(
    params: &Parameters,
    ty: &ElementType,
    instance: &Instance,
) -> Result<Offsets, WallError> {
    let (line, flipped) = placement(params, ty, instance)?;
    assembly_of(ty)?.offsets(line, flipped)
}
