//! Where a typed value in a schedule's cell goes back to.
//!
//! A cell is the value of one element's field. Editing it writes the element, through the same
//! command a person uses on the element itself, so it is one step to undo and the element is
//! claimed like any edit. Only two fields are the element's to change: its name, and a parameter
//! the element holds itself. Everything else a schedule shows is worked out or held elsewhere, and
//! refuses in words: the type's name (the type is changed on the element), a parameter the type
//! holds (it is one value for every element of the type, changed on the type), a quantity (worked
//! out of the element's shape) and a calculated field (worked out of other fields).

use crate::error::ScheduleError;
use crate::field::{Basis, Field};
use crate::schedule::Schedule;
use construct_params::{Definition, Holder};

/// What a cell's typed value is written to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditTarget {
    /// The element's name.
    Name,
    /// An instance parameter of the element, by its definition.
    Parameter(Definition),
}

impl Schedule {
    /// Where an edit of one of this schedule's fields goes, or why the field is read-only.
    ///
    /// # Errors
    /// [`ScheduleError::NoSuchField`] for a field the schedule does not have, and
    /// [`ScheduleError::ReadOnly`] for one that is worked out or held elsewhere, with the reason.
    pub fn edit_target(
        &self,
        basis: &Basis<'_>,
        field: &Field,
    ) -> Result<EditTarget, ScheduleError> {
        let definition = self.require(basis, field)?;
        let name = definition.name().to_owned();
        let read_only = |why: &str| ScheduleError::ReadOnly {
            field: name.clone(),
            why: why.to_owned(),
        };
        match field {
            Field::Name => Ok(EditTarget::Name),
            Field::Type => Err(read_only(
                "it is the name of the element's type: change the element's type, or rename the type.",
            )),
            Field::Quantity(_) => Err(read_only(
                "it is worked out from the element, so it is changed by changing the element.",
            )),
            Field::Calculated(_) => Err(read_only(
                "it is worked out from other fields by its formula: change those fields, or the formula.",
            )),
            Field::Parameter(id) => {
                let holder = basis
                    .params
                    .parameters_of(basis.categories, self.lists())
                    .into_iter()
                    .find(|applicable| applicable.definition.id() == id)
                    .map(|applicable| applicable.holder);
                match holder {
                    Some(Holder::Instance) => Ok(EditTarget::Parameter(definition)),
                    Some(Holder::Type) => Err(read_only(
                        "its value is held by the element's type, one value for every element of that type: change it on the type.",
                    )),
                    None => Err(ScheduleError::NoSuchField {
                        category: basis.category_name(self.lists()),
                        field: field.key(),
                    }),
                }
            }
        }
    }
}
