//! Every refusal, as a value the caller can read. Its words are what a person sees.

use construct_categories::CategoryId;
use construct_params::Kind;
use kit_core::Id;
use std::fmt;

/// Why a schedule, a record of one, or a change to one was refused. A refusal changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    /// A schedule needs a name with something in it besides spaces.
    EmptyName,
    /// The document already has a schedule of this name (case does not matter).
    DuplicateName(String),
    /// A schedule already has this ID.
    DuplicateSchedule(Id),
    /// No schedule has this ID.
    UnknownSchedule(Id),
    /// No category has this ID.
    UnknownCategory(CategoryId),
    /// The category exists, but nothing in it can be listed (views, sheets, tags and the like).
    NotListable {
        /// The category's name.
        category: String,
    },
    /// A field's key is not one of the spellings `name`, `type`, `parameter:<id>`,
    /// `quantity:<name>`.
    BadField(String),
    /// The category has no such field: no parameter of that definition applies to it, or it works
    /// out no such quantity.
    NoSuchField {
        /// The category's name.
        category: String,
        /// The field, as it was named.
        field: String,
    },
    /// A field listed twice.
    DuplicateField(String),
    /// A field sorted on twice.
    DuplicateSort(String),
    /// A filter compares a value of one kind with a field of another: a number with text, say.
    FilterKind {
        /// The field's name.
        field: String,
        /// What the field holds.
        holds: Kind,
        /// What the filter's value is.
        given: Kind,
    },
    /// A filter's comparison makes no sense for what the field holds (`contains` on a length).
    ComparisonNotFor {
        /// The field's name.
        field: String,
        /// The comparison, in its key.
        comparison: String,
        /// What the field holds.
        holds: Kind,
    },
    /// A comparison that is not one of the eight.
    UnknownComparison(String),
    /// A sorting order that is neither `ascending` nor `descending`.
    UnknownOrder(String),
    /// A filter's value the field's definition does not take (a choice it does not list).
    BadFilterValue {
        /// The field's name.
        field: String,
        /// The definition's own words.
        why: String,
    },
    /// A record that is not a schedule as written.
    Malformed(String),
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "A schedule needs a name."),
            Self::DuplicateName(name) => {
                write!(
                    f,
                    "There is already a schedule named {name:?} in this model."
                )
            }
            Self::DuplicateSchedule(id) => write!(f, "There is already a schedule {id}."),
            Self::UnknownSchedule(id) => write!(f, "There is no schedule {id} in this model."),
            Self::UnknownCategory(id) => write!(f, "There is no category {}.", id.0),
            Self::NotListable { category } => {
                write!(
                    f,
                    "A schedule cannot list {category}: it holds no model elements."
                )
            }
            Self::BadField(key) => write!(
                f,
                "{key:?} is not a field. A field is name, type, parameter:<id> or quantity:<name>."
            ),
            Self::NoSuchField { category, field } => {
                write!(f, "{category} have no field {field}.")
            }
            Self::DuplicateField(field) => write!(f, "The field {field} is listed twice."),
            Self::DuplicateSort(field) => write!(f, "The field {field} is sorted on twice."),
            Self::FilterKind {
                field,
                holds,
                given,
            } => write!(
                f,
                "{field} holds {holds}, so a filter on it cannot compare it with {given}."
            ),
            Self::ComparisonNotFor {
                field,
                comparison,
                holds,
            } => write!(
                f,
                "{field} holds {holds}, so a filter on it cannot use {comparison}."
            ),
            Self::UnknownComparison(key) => write!(
                f,
                "{key:?} is not a comparison: equals, not_equals, less, less_or_equal, greater, \
                 greater_or_equal, contains or begins_with."
            ),
            Self::UnknownOrder(key) => {
                write!(f, "{key:?} is not an order: ascending or descending.")
            }
            Self::BadFilterValue { field, why } => {
                write!(f, "The filter on {field} has a value it cannot take: {why}")
            }
            Self::Malformed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for ScheduleError {}
