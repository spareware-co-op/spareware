//! A small default metric set of column types for the tests and the first users, each made up from
//! common building practice, not taken from any list: a 400 by 400 mm concrete column, a 450 mm
//! round concrete column and a 140 by 140 mm timber post.
//!
//! Materials are construct-materials' default IDs.

use crate::column_type::new_column_type;
use crate::error::ColumnError;
use crate::section::Section;
use crate::stored::ColumnSpec;
use construct_categories::CategoryTable;
use construct_materials::MaterialId;
use construct_params::Length;
use construct_types::ElementType;
use kit_core::Id;

/// Concrete, construct-materials' ID.
pub const CONCRETE: &str = "concrete-regular";
/// Framing timber, construct-materials' ID.
pub const TIMBER: &str = "timber-douglas-fir";

/// One default column type: the ID it is made under, its name and what it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultColumn {
    /// The type's permanent ID.
    pub id: &'static str,
    /// The name people see.
    pub name: &'static str,
    /// Its section and material.
    pub spec: ColumnSpec,
}

/// The default column types, in the order they are listed.
///
/// # Errors
/// A refusal of a size or an ID, which these constants never meet.
pub fn default_columns() -> Result<Vec<DefaultColumn>, ColumnError> {
    Ok(vec![
        DefaultColumn {
            id: "column-concrete-400x400",
            name: "Concrete 400 x 400",
            spec: ColumnSpec {
                section: Section::rectangular(Length::from_mm(400), Length::from_mm(400))?,
                material: MaterialId::parse(CONCRETE)?,
            },
        },
        DefaultColumn {
            id: "column-concrete-round-450",
            name: "Concrete round 450",
            spec: ColumnSpec {
                section: Section::round(Length::from_mm(450))?,
                material: MaterialId::parse(CONCRETE)?,
            },
        },
        DefaultColumn {
            id: "column-timber-140x140",
            name: "Timber 140 x 140",
            spec: ColumnSpec {
                section: Section::rectangular(Length::from_mm(140), Length::from_mm(140))?,
                material: MaterialId::parse(TIMBER)?,
            },
        },
    ])
}

/// The default column types as element types of the columns category.
///
/// # Errors
/// [`ColumnError::Type`] for a name construct-types refuses.
pub fn default_column_types(categories: &CategoryTable) -> Result<Vec<ElementType>, ColumnError> {
    default_columns()?
        .into_iter()
        .map(|column| {
            let id = Id::parse(column.id)
                .map_err(|_| ColumnError::Malformed(format!("{:?} is not an id", column.id)))?;
            new_column_type(id, column.name, &column.spec, categories)
        })
        .collect()
}
