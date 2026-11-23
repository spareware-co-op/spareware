//! The sheet parameters every project has: drawn by, checked by, approved by, issue date,
//! revision and the rest of what a title block reads.
//!
//! They are construct-params' own: text parameters of the instance, with permanent IDs fixed here,
//! bound to the sheets category. A sheet holds their values in its record like any element holds
//! its parameters, set by the ordinary `element.param` command, so a title block reads them with
//! the same reader as everything else and nothing about a sheet's parameters is special except
//! that these definitions are in every project's table once a sheet is made.

use crate::error::SheetError;
use construct_categories::{CategoryTable, SHEETS};
use construct_params::{Definition, Holder, Kind, ProjectParams};
use kit_core::Id;

/// The standard sheet parameters: ID and the name people see. The issue date is text in the form
/// `2026-11-24`, as the project's own issue date is.
pub const STANDARD: [(&str, &str); 8] = [
    ("sheet.drawn-by", "Drawn by"),
    ("sheet.checked-by", "Checked by"),
    ("sheet.approved-by", "Approved by"),
    ("sheet.issue-date", "Sheet issue date"),
    ("sheet.revision", "Current revision"),
    ("sheet.revision-date", "Current revision date"),
    ("sheet.revision-description", "Current revision description"),
    ("sheet.discipline", "Sheet discipline"),
];

/// The standard definitions, in the order of [`STANDARD`].
///
/// # Errors
/// [`SheetError::Malformed`] only if a fixed ID or name were refused, which a test rules out.
pub fn standard_definitions() -> Result<Vec<Definition>, SheetError> {
    let bad = |why: String| SheetError::Malformed(why);
    STANDARD
        .iter()
        .map(|(id, name)| {
            let id = Id::parse(id).map_err(|e| bad(e.to_string()))?;
            Definition::new(id, name, Kind::Text, Holder::Instance).map_err(|e| bad(e.to_string()))
        })
        .collect()
}

/// The table with every standard sheet parameter defined and bound to the sheets category. A
/// table that has them already comes back equal, so a command writes the table only when it
/// changed. A user's own parameter of the same ID is not touched.
///
/// # Errors
/// [`SheetError::Malformed`] with the table's own words if a standard ID is already another kind
/// of parameter, or the sheets category cannot be bound.
pub fn with_standard(
    table: &ProjectParams,
    categories: &CategoryTable,
) -> Result<ProjectParams, SheetError> {
    let mut table = table.clone();
    for definition in standard_definitions()? {
        let id = definition.id().clone();
        table = table
            .add_project_parameter(definition)
            .map_err(|e| SheetError::Malformed(e.to_string()))?;
        let bound = table
            .binding(&id)
            .is_some_and(|(_, bound)| bound.contains(&SHEETS));
        if !bound {
            table = table
                .bind(categories, &id, &[SHEETS], Holder::Instance, None)
                .map_err(|e| SheetError::Malformed(e.to_string()))?;
        }
    }
    Ok(table)
}
