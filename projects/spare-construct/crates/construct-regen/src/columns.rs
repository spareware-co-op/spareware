//! How a column's recipe reads its section from its type and its position from the grid lines it
//! stands on (job 14.7, CON-A5-F1-P1). The regenerator fills a column's recipe through this one
//! module before it is built, so a column is built from what construct-columns says it is, and a
//! command or a page view asks the same functions, so what is checked and what is built cannot
//! differ.
//!
//! What it promises:
//! - A column's recipe argument written `{"type":"section"}` (construct-columns' `section_arg`) is
//!   filled, before the shape is built, with the section and the material of the element's column
//!   type (its `@system.column` block, construct-columns' `spec_of`) ([`filled`]). Its height is
//!   then read from its levels as every element's is ([`crate::levels::filled`]: a top tied to a
//!   level is the height from the base to it, a stored length). Last, [`finished`] works out the
//!   ring the builders take (construct-columns' `finish`): the section about the column's
//!   position, turned, where a position on two grid lines is where they cross as the document
//!   stands (as regeneration has reached it, for building). A recipe that is not a column's, or
//!   already has its ring, is returned as it is.
//! - A column's `reads` row lists its type and the parameter table ([`reads_type`], through
//!   [`crate::parameters::reads_records`]), its levels ([`crate::levels::levels_read`]) and the
//!   grid lines it stands on ([`grids_read`]), taken from its own recipe so that a column that
//!   failed to build still builds again when a grid is made or moved. A change to a grid builds
//!   the columns that read it again (the regenerator's grid step), so a column on a grid
//!   intersection stays on it when the grid moves.
//! - A failure is the column's build failure, in construct-columns' words under its code:
//!   `NoColumnType`, `BadColumnType`, `BadSize`, `BadGrid`, `NoHeight`, `TopBelowBase`.
//! - What a page shows of a column besides its parameters ([`view_rows`]): its height, section
//!   area, volume and mass (construct-columns' `quantities` and `read_only`), worked out from its
//!   type and its levels each time it is read, never stored, each shown in the document's units
//!   and marked `read-only`.
//!
//! Depends on construct-columns (the rules), construct-grids (the datums), construct-types (the
//! type), construct-model and kit-store, and on [`crate::parameters`] and [`crate::levels`].
//! Nothing here is interim.

use crate::levels;
use crate::parameters::{At, ParameterView, read_at};
use construct_columns::{
    ColumnError, HEIGHT_ARG, RING_ARG, finish, is_column, is_column_recipe, quantities, read_only,
    spec_of,
};
use construct_grids::{Datums, GRID_KIND};
use construct_materials::Materials;
use construct_model::element::Element;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Units, Value};
use construct_types::ElementType;
use kit_core::Id;
use kit_store::{Reader, StoreError};
use std::collections::BTreeSet;

pub use construct_columns::reads_section as reads_type;

fn failure(code: &str, message: String) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message,
    }
}

fn refused(error: &ColumnError) -> GeometryError {
    failure(error.code(), error.to_string())
}

/// The recipe a column is built from, with its section and material read from its column type
/// `ty`. A recipe that reads no type ([`reads_type`]) is returned as it is.
///
/// # Errors
/// A build failure, which is model state like any other: `NoColumnType` for a column that names no
/// type, `BadColumnType` for a type that is not a column type or whose section does not read, in
/// construct-columns' words.
pub fn filled(recipe: &Recipe, ty: Option<&ElementType>) -> Result<Recipe, GeometryError> {
    if !reads_type(recipe) {
        return Ok(recipe.clone());
    }
    let Some(ty) = ty else {
        return Err(failure(
            "NoColumnType",
            "its section is read from its column type, and it names none".to_owned(),
        ));
    };
    let spec =
        spec_of(ty).map_err(|e| failure("BadColumnType", format!("its type {}: {e}", ty.id())))?;
    construct_columns::filled_by_type(recipe, &spec).map_err(|e| refused(&e))
}

/// The grid lines a column's own recipe stands on, by ID, ascending: its `reads` row lists them.
/// Nothing for a recipe that is not a column's.
#[must_use]
pub fn grids_read(recipe: &Recipe) -> BTreeSet<String> {
    construct_columns::grids_read(recipe)
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// The recipe the builders take: `recipe` (filled by the type and with its height read from the
/// levels) with its ring worked out, the grid lines read `at` that state. A recipe that is not a
/// column's, or already has its ring, is returned as it is.
///
/// # Errors
/// A [`StoreError`] when the store fails. The inner error is the build failure: `BadGrid` for a
/// grid the model lacks or two that do not cross, and construct-columns' own refusals, in words.
pub async fn finished<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    recipe: Recipe,
    at: At,
) -> Result<Result<Recipe, GeometryError>, StoreError> {
    if !is_column_recipe(&recipe) || recipe.args.contains_key(RING_ARG) {
        return Ok(Ok(recipe));
    }
    let mut records = Vec::new();
    for id in construct_columns::grids_read(&recipe) {
        match read_at(reader, project, document, GRID_KIND, &id, at).await? {
            Some(record) => records.push(record),
            None => {
                return Ok(Err(failure(
                    "BadGrid",
                    format!("the column stands on grid {id}, which is not in this model"),
                )));
            }
        }
    }
    let datums = match Datums::from_records(&records) {
        Ok(datums) => datums,
        Err(e) => return Ok(Err(failure("BadGrid", e.to_string()))),
    };
    Ok(finish(&recipe, &datums).map_err(|e| refused(&e)))
}

/// Where a read-only value came from, as a page shows it: worked out, never set.
pub const READ_ONLY: &str = "read-only";

/// A column's read-only parameters as a page shows them, as the module header says: its height,
/// section area, volume and mass, from its type `ty` and its levels as the document stands `at`
/// that state. Not a column, or one whose type or levels cannot be read (its built row says why):
/// none.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn view_rows<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    ty: Option<&ElementType>,
    units: &Units,
    at: At,
) -> Result<Vec<ParameterView>, StoreError> {
    if !is_column(element) {
        return Ok(Vec::new());
    }
    let Ok(recipe) = levels::recipe_at(reader, project, document, element, at).await? else {
        return Ok(Vec::new());
    };
    let height = recipe
        .args
        .get(HEIGHT_ARG)
        .and_then(serde_json::Value::as_str)
        .and_then(Value::from_stored);
    let rows = (|| {
        let (Some(ty), Some(Value::Length(height))) = (ty, height) else {
            return None;
        };
        let q = quantities(&spec_of(ty).ok()?, height, &Materials::default_metric()).ok()?;
        read_only(&q).ok()
    })();
    Ok(rows
        .into_iter()
        .flatten()
        .map(|(def, value)| crate::slabs::shown(&def, value, units))
        .collect())
}
