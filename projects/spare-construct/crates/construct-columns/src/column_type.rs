//! A column type as an element type of construct-types: of the columns category, its section and
//! material in its system block under the owner tag [`OWNER`]. The sizes of the section are the
//! type's values: every column of the type reads them, and changing the type's section changes
//! every column of it. A column instance holds no size of its own; what it reads back (height,
//! area, volume, mass) is worked out ([`crate::read_only`]), never stored.

use crate::error::ColumnError;
use crate::stored::{ColumnSpec, OWNER};
use construct_categories::{COLUMNS, CategoryTable};
use construct_types::{ElementType, SystemData};
use kit_core::Id;

/// A new column type of the columns category, with `spec` in its system block.
///
/// # Errors
/// [`ColumnError::Type`] for a name construct-types refuses.
pub fn new_column_type(
    id: Id,
    name: &str,
    spec: &ColumnSpec,
    categories: &CategoryTable,
) -> Result<ElementType, ColumnError> {
    let ty = ElementType::new(id, name, COLUMNS, categories)?;
    with_spec(ty, spec)
}

/// The same column type with `spec`: how a changed type's result is kept.
///
/// # Errors
/// [`ColumnError::NotAColumnType`] if `ty` is not of the columns category.
pub fn with_spec(ty: ElementType, spec: &ColumnSpec) -> Result<ElementType, ColumnError> {
    if ty.category() != COLUMNS {
        return Err(ColumnError::NotAColumnType {
            category: ty.category(),
        });
    }
    let block = SystemData::new(OWNER, &spec.to_text()?)?;
    Ok(ty.with_system(block))
}

/// A column type's section and material, read from its system block.
///
/// # Errors
/// [`ColumnError::NotAColumnType`] if `ty` is not of the columns category;
/// [`ColumnError::NoColumnSection`] if it has no column block; [`ColumnError::Malformed`] if the
/// text is not in its one spelling; the sizes' refusals.
pub fn spec_of(ty: &ElementType) -> Result<ColumnSpec, ColumnError> {
    if ty.category() != COLUMNS {
        return Err(ColumnError::NotAColumnType {
            category: ty.category(),
        });
    }
    match ty.system() {
        Some(block) if block.owner() == OWNER => ColumnSpec::from_text(block.data()),
        _ => Err(ColumnError::NoColumnSection {
            ty: ty.id().clone(),
        }),
    }
}
