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
    /// A total that is not one of `sum`, `count`, `min`, `max`.
    UnknownTotal(String),
    /// A field totalled twice.
    DuplicateTotal(String),
    /// A field given in the totals with nothing to total.
    NoTotals(String),
    /// A total on a field the schedule does not list: a total is read from the column.
    TotalNotListed(String),
    /// A total that makes no sense for what the field holds (a sum of text).
    TotalNotFor {
        /// The field's name.
        field: String,
        /// The total, by its key.
        total: String,
        /// What the field holds.
        holds: Kind,
    },
    /// A calculated field that cannot stand: a name taken, a formula that does not read, a unit
    /// that does not fit or a field it names that is not there.
    Calculated {
        /// The calculated field's name.
        name: String,
        /// Why, in words.
        why: String,
    },
    /// A cell that is not edited here.
    ReadOnly {
        /// The field's name.
        field: String,
        /// Why, in words.
        why: String,
    },
    /// The schedule does not list this element.
    NotListed(Id),
    /// A schedule already stands on this sheet (or the placement's ID is taken).
    DuplicatePlacement(Id, Id),
    /// No placement has this ID.
    UnknownPlacement(Id),
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
            Self::UnknownTotal(key) => {
                write!(f, "{key:?} is not a total: sum, count, min or max.")
            }
            Self::DuplicateTotal(field) => write!(f, "The field {field} is totalled twice."),
            Self::NoTotals(field) => write!(f, "The totals of {field} name no total."),
            Self::TotalNotListed(field) => write!(
                f,
                "{field} is not a column of this schedule, so it has no totals: add the field first."
            ),
            Self::TotalNotFor {
                field,
                total,
                holds,
            } => write!(
                f,
                "{field} holds {holds}, so its {total} cannot be worked out{}.",
                if total == "count" {
                    ""
                } else {
                    " (count works on any field)"
                }
            ),
            Self::Calculated { name, why } => {
                write!(f, "The calculated field {name}: {why}")
            }
            Self::ReadOnly { field, why } => write!(f, "{field} cannot be edited here: {why}"),
            Self::NotListed(id) => {
                write!(f, "The schedule does not list the element {id}.")
            }
            Self::DuplicatePlacement(schedule, sheet) => write!(
                f,
                "The schedule {schedule} already stands on the sheet {sheet}; move it instead."
            ),
            Self::UnknownPlacement(id) => {
                write!(f, "No schedule is placed as {id}.")
            }
        }
    }
}

impl std::error::Error for ScheduleError {}
