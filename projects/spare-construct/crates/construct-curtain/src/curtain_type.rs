//! A curtain wall type as an element type of construct-types: of the curtain walls category, its
//! grid and parts in its system block under the owner tag [`OWNER`] ([`CurtainSpec::to_text`]). A
//! curtain wall instance holds no parameter of its own here: what it changes of its type's grid is
//! in its recipe ([`crate::Edits`]), and what it reads back is worked out ([`crate::read_only`]),
//! never stored.

use crate::error::CurtainError;
use crate::spec::CurtainSpec;
use crate::stored::OWNER;
use construct_categories::{CURTAIN_WALLS, CategoryTable};
use construct_types::{ElementType, SystemData};
use kit_core::Id;

/// A new curtain wall type of the curtain walls category, with `spec` in its system block.
///
/// # Errors
/// [`CurtainError::Type`] for a name construct-types refuses.
pub fn new_curtain_type(
    id: Id,
    name: &str,
    spec: &CurtainSpec,
    categories: &CategoryTable,
) -> Result<ElementType, CurtainError> {
    let ty = ElementType::new(id, name, CURTAIN_WALLS, categories)?;
    with_spec(ty, spec)
}

/// The same curtain wall type with `spec` as its grid and parts: how a changed type is kept.
///
/// # Errors
/// [`CurtainError::NotACurtainType`] if `ty` is not of the curtain walls category.
pub fn with_spec(ty: ElementType, spec: &CurtainSpec) -> Result<ElementType, CurtainError> {
    if ty.category() != CURTAIN_WALLS {
        return Err(CurtainError::NotACurtainType {
            category: ty.category(),
        });
    }
    let block = SystemData::new(OWNER, &spec.to_text()?)?;
    Ok(ty.with_system(block))
}

/// A curtain wall type's grid and parts, read from its system block.
///
/// # Errors
/// [`CurtainError::NotACurtainType`] if `ty` is not of the curtain walls category;
/// [`CurtainError::NoCurtainBlock`] if it has no curtain block; the refusals of
/// [`CurtainSpec::from_text`].
pub fn spec_of(ty: &ElementType) -> Result<CurtainSpec, CurtainError> {
    if ty.category() != CURTAIN_WALLS {
        return Err(CurtainError::NotACurtainType {
            category: ty.category(),
        });
    }
    match ty.system() {
        Some(block) if block.owner() == OWNER => CurtainSpec::from_text(block.data()),
        _ => Err(CurtainError::NoCurtainBlock {
            ty: ty.id().clone(),
        }),
    }
}
