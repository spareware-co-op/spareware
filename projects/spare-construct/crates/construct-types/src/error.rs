//! Every refusal, as a value the caller can read.

use construct_categories::CategoryId;
use construct_params::ParamError;
use kit_core::Id;
use std::fmt;

/// Why a type, an instance or a stored record was refused. A refusal changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeError {
    /// A type needs a name with something in it besides spaces.
    EmptyName,
    /// The category already has a type of this name (case does not matter).
    DuplicateName {
        /// The category.
        category: CategoryId,
        /// The name asked for.
        name: String,
    },
    /// A type already has this ID.
    DuplicateType(Id),
    /// No type has this ID.
    UnknownType(Id),
    /// No category has this ID.
    UnknownCategory(CategoryId),
    /// A parameter of this ID is already defined.
    DuplicateParameter(Id),
    /// No parameter of this ID is defined.
    UnknownParameter(Id),
    /// The type's elements do not have this parameter.
    NotOnType {
        /// The type's name.
        type_name: String,
        /// The parameter's name.
        parameter: String,
    },
    /// The parameter is held by the type, so an instance cannot hold a value of it.
    HeldByType {
        /// The parameter's name.
        parameter: String,
    },
    /// The parameter is held by each instance, so the type cannot hold a value of it.
    HeldByInstance {
        /// The parameter's name.
        parameter: String,
    },
    /// The instance is of another type than the one given.
    NotItsType {
        /// The type the instance names.
        instance_type: Id,
        /// The type it was given with.
        given: Id,
    },
    /// An instance can move only to a type of its own category.
    OtherCategory {
        /// Its type's category.
        from: CategoryId,
        /// The category of the type asked for.
        to: CategoryId,
    },
    /// Instances still use the type, so it cannot be deleted.
    InUse {
        /// The type's name.
        type_name: String,
        /// How many of the instances given use it.
        count: usize,
    },
    /// A value was refused by its parameter's definition.
    Param(ParamError),
    /// A system block's owner tag or data is not well formed; the text says why.
    BadSystem(String),
    /// A stored record or `props` entry is not one this crate writes; the text says why.
    Malformed(String),
}

impl From<ParamError> for TypeError {
    fn from(error: ParamError) -> Self {
        Self::Param(error)
    }
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "a type needs a name"),
            Self::DuplicateName { category, name } => {
                write!(
                    f,
                    "category {} already has a type named {name:?}",
                    category.0
                )
            }
            Self::DuplicateType(id) => write!(f, "a type already has id {id}"),
            Self::UnknownType(id) => write!(f, "no type has id {id}"),
            Self::UnknownCategory(id) => write!(f, "no category has id {}", id.0),
            Self::DuplicateParameter(id) => write!(f, "parameter {id} is already defined"),
            Self::UnknownParameter(id) => write!(f, "no parameter has id {id}"),
            Self::NotOnType {
                type_name,
                parameter,
            } => write!(f, "type {type_name:?} has no parameter {parameter:?}"),
            Self::HeldByType { parameter } => write!(
                f,
                "{parameter:?} is held by the type, so it cannot be set on an instance"
            ),
            Self::HeldByInstance { parameter } => write!(
                f,
                "{parameter:?} is held by each instance, so it cannot be set on the type"
            ),
            Self::NotItsType {
                instance_type,
                given,
            } => write!(f, "the instance is of type {instance_type}, not {given}"),
            Self::OtherCategory { from, to } => write!(
                f,
                "an instance of category {} cannot take a type of category {}",
                from.0, to.0
            ),
            Self::InUse { type_name, count } => write!(
                f,
                "type {type_name:?} cannot be deleted: {count} instances use it"
            ),
            Self::Param(error) => error.fmt(f),
            Self::BadSystem(why) => write!(f, "not a system block: {why}"),
            Self::Malformed(why) => write!(f, "not a stored type or instance: {why}"),
        }
    }
}

impl std::error::Error for TypeError {}
