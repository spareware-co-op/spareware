//! How types and instances sit in construct-model's element record (FORMAT.md beside this crate):
//! the reserved `props` keys, the type recipe, and the one spelling of each entry.
//!
//! A reserved key starts with `@`. A kit-core ID always starts with a letter or a digit, so a
//! reserved key can never be a parameter's ID, and construct-params' `Values::from_props`, which
//! reads only parameter IDs, never reads one.

use crate::error::TypeError;
use construct_categories::CategoryId;
use construct_model::geometry::Recipe;
use kit_core::Id;
use std::collections::{BTreeMap, BTreeSet};

/// The `kind` of every type record.
pub const TYPE_KIND: &str = "type";
/// On a type record: the type's category, as its decimal ID.
pub const CATEGORY_KEY: &str = "@category";
/// On a type record: the IDs of every parameter the type's elements have, ascending, joined by
/// commas (an ID never holds a comma); empty when there are none.
pub const PARAMS_KEY: &str = "@params";
/// On an instance's record: the permanent ID of its type.
pub const TYPE_KEY: &str = "@type";
/// On a type record of version 2: the start of the key of its one system block, followed by the
/// block's owner tag (`@system.wall`); the value is the owner's data.
pub const SYSTEM_KEY_PREFIX: &str = "@system.";

/// What every reserved key starts with.
const RESERVED: char = '@';

/// The recipe of a type record: Spare Construct's own operation `type`, version 1, with no
/// arguments. It builds no shape. Its version is the type record's format version.
#[must_use]
pub fn type_recipe() -> Recipe {
    Recipe::construct(TYPE_KIND, BTreeMap::new())
}

/// The recipe of a type record of version 2, one that carries a system block: as
/// [`type_recipe`] with `op_version` 2.
#[must_use]
pub fn system_type_recipe() -> Recipe {
    Recipe {
        op_version: 2,
        ..type_recipe()
    }
}

/// Whether a `props` key is one of this crate's reserved keys, known or not.
pub(crate) fn is_reserved(key: &str) -> bool {
    key.starts_with(RESERVED)
}

/// A category ID in its one spelling.
pub(crate) fn write_category(id: CategoryId) -> String {
    id.0.to_string()
}

/// Reads a category ID, refusing any spelling but the one written.
pub(crate) fn read_category(text: &str) -> Result<CategoryId, TypeError> {
    match text.parse::<u32>() {
        Ok(n) if n.to_string() == text => Ok(CategoryId(n)),
        _ => Err(TypeError::Malformed(format!(
            "{CATEGORY_KEY} {text:?} is not a category id"
        ))),
    }
}

/// A type's parameter IDs in their one spelling.
pub(crate) fn write_params(params: &BTreeSet<Id>) -> String {
    params.iter().map(Id::as_str).collect::<Vec<_>>().join(",")
}

/// Reads a type's parameter IDs: none, or IDs strictly ascending, joined by commas.
pub(crate) fn read_params(text: &str) -> Result<BTreeSet<Id>, TypeError> {
    let mut params = BTreeSet::new();
    if text.is_empty() {
        return Ok(params);
    }
    for part in text.split(',') {
        let id = read_id(part, PARAMS_KEY)?;
        if params.last().is_some_and(|last| last >= &id) {
            return Err(TypeError::Malformed(format!(
                "{PARAMS_KEY} is not strictly ascending at {part:?}"
            )));
        }
        params.insert(id);
    }
    Ok(params)
}

/// Reads one permanent ID, naming where it was found if it is not one.
pub(crate) fn read_id(text: &str, what: &str) -> Result<Id, TypeError> {
    Id::parse(text).map_err(|_| TypeError::Malformed(format!("{what} {text:?} is not an id")))
}
