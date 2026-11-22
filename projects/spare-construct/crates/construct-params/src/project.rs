//! Project parameters: the parameters a project has, each bound to a chosen set of categories as
//! instance or type, so every element of those categories has it (CON-A2-F2-P3, CON-A2-F2-P5).
//!
//! A [`ProjectParams`] holds definitions (shared ones, from a [`SharedParameter`], or ones only
//! this project has) and a binding for each parameter that is bound. Every function is pure and
//! returns a new table; the caller, later the edit log, decides when to keep it. This crate keeps
//! no index of elements, so [`ProjectParams::unbind`] is given the holders of values and counts
//! from them.
//!
//! The binding rules, each tested:
//! - A parameter is bound to categories by their ID in a [`CategoryTable`], and at least one.
//! - A subcategory cannot be bound. An element belongs to a top-level category; a subcategory only
//!   names a part a drawing draws differently, so a value on it would belong to no element. The
//!   refusal names the parent to bind instead.
//! - A category can hold an instance parameter whatever it is. A type parameter needs a category
//!   whose elements have types: [`can_hold`] says no to imported files and analytical stand-ins.
//! - A parameter is bound as instance or as type, once: binding it again adds categories only if
//!   the holder is the same, because changing the holder would strand the values already kept.
//! - A binding may carry a default value, checked against the parameter's kind; a wrong kind is
//!   refused. construct-types' `Parameters::add` is fed from these bindings. Binding the same
//!   categories with the same default again changes nothing.
//! - Unbinding reports how many holders of values are in the categories and have a value, so the
//!   caller can warn that those values will be dropped, and then drop them.
//!
//! [`ProjectParams::parameters_of`] answers which parameters an element of a category has. The
//! stored form is in FORMAT.md.

use crate::definition::{Definition, Holder, Kind};
use crate::error::ParamError;
use crate::info;
use crate::shared::{SharedError, SharedParameter, difference};
use crate::value::Value;
use crate::values::Values;
use construct_categories::{CategoryId, CategoryKind, CategoryTable};
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Why a project parameter, a binding or a stored table was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectError {
    /// The ID belongs to the project information's built-in parameters.
    ReservedId(Id),
    /// The ID is already a parameter of another kind.
    KindConflict {
        /// The parameter's ID.
        id: Id,
        /// The name of the parameter the project already has.
        name: String,
        /// The kind it has.
        existing: Kind,
        /// The kind that was offered.
        incoming: Kind,
    },
    /// The ID is already a parameter that differs in another field.
    Conflict {
        /// The parameter's ID.
        id: Id,
        /// The name of the parameter the project already has.
        name: String,
        /// Which field differs.
        field: &'static str,
    },
    /// Another parameter of the project has this name (case does not matter).
    DuplicateName(String),
    /// The project has no parameter with this ID.
    UnknownParameter(Id),
    /// A binding needs at least one category.
    NoCategories,
    /// No category has this ID.
    UnknownCategory(CategoryId),
    /// A subcategory cannot be bound; bind its parent.
    SubcategoryNotBindable {
        /// The subcategory.
        category: CategoryId,
        /// The category to bind instead.
        parent: CategoryId,
    },
    /// The category's elements have no types, so it cannot hold a type parameter.
    CannotHold {
        /// The category.
        category: CategoryId,
        /// What sort of category it is.
        kind: CategoryKind,
        /// The holder asked for.
        holder: Holder,
    },
    /// The parameter is already bound as the other holder.
    HolderConflict {
        /// The parameter's ID.
        id: Id,
        /// The name of the parameter.
        name: String,
        /// How it is bound.
        existing: Holder,
        /// How it was asked to be bound.
        requested: Holder,
    },
    /// The parameter is not bound to this category.
    NotBound {
        /// The parameter's ID.
        id: Id,
        /// The name of the parameter.
        name: String,
        /// The category.
        category: CategoryId,
    },
    /// The default is not a value the parameter takes.
    BadDefault(ParamError),
    /// The stored table is not a version this build reads.
    UnsupportedFormat(u32),
    /// The stored table is not valid; the text says why.
    Malformed(String),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedId(id) => write!(f, "{} is a project information parameter", id.as_str()),
            Self::KindConflict {
                id,
                name,
                existing,
                incoming,
            } => write!(
                f,
                "{} is already {name:?}, {existing}, and cannot also be {incoming}",
                id.as_str()
            ),
            Self::Conflict { id, name, field } => write!(
                f,
                "{} is already {name:?} and its {field} differs",
                id.as_str()
            ),
            Self::DuplicateName(name) => write!(f, "the project already has a parameter {name:?}"),
            Self::UnknownParameter(id) => {
                write!(f, "the project has no parameter {}", id.as_str())
            }
            Self::NoCategories => write!(f, "a binding needs at least one category"),
            Self::UnknownCategory(id) => write!(f, "no category has id {}", id.0),
            Self::SubcategoryNotBindable { category, parent } => write!(
                f,
                "category {} is a subcategory; bind its parent, category {}",
                category.0, parent.0
            ),
            Self::CannotHold {
                category,
                kind,
                holder,
            } => write!(
                f,
                "category {} ({kind:?}) has no {holder:?} parameters",
                category.0
            ),
            Self::HolderConflict {
                id,
                name,
                existing,
                requested,
            } => write!(
                f,
                "{} ({name:?}) is bound as {existing:?}, not {requested:?}",
                id.as_str()
            ),
            Self::NotBound { id, name, category } => write!(
                f,
                "{} ({name:?}) is not bound to category {}",
                id.as_str(),
                category.0
            ),
            Self::BadDefault(error) => write!(f, "default: {error}"),
            Self::UnsupportedFormat(v) => {
                write!(f, "project parameter table format {v} is not supported")
            }
            Self::Malformed(why) => write!(f, "project parameter table is not valid: {why}"),
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<SharedError> for ProjectError {
    fn from(error: SharedError) -> Self {
        // Only a parameter's own refusals reach here; the project turns them into its own.
        match error {
            SharedError::KindConflict {
                id,
                name,
                existing,
                incoming,
            } => Self::KindConflict {
                id,
                name,
                existing,
                incoming,
            },
            SharedError::Conflict { id, name, field } => Self::Conflict { id, name, field },
            SharedError::DuplicateName { name, .. } => Self::DuplicateName(name),
            SharedError::UnsupportedFormat(v) => Self::UnsupportedFormat(v),
            SharedError::BadUuid(text)
            | SharedError::BadGroup(text)
            | SharedError::Malformed(text) => Self::Malformed(text),
        }
    }
}

/// Whether elements of a category of this sort can hold a parameter held as `holder`. An instance
/// parameter fits every category. A type parameter needs elements that have types, which imported
/// files and analytical stand-ins do not.
#[must_use]
pub fn can_hold(kind: CategoryKind, holder: Holder) -> bool {
    match holder {
        Holder::Instance => true,
        Holder::Type => !matches!(kind, CategoryKind::Import | CategoryKind::Analytical),
    }
}

/// A parameter of the project, and the group it is filed under if it is a shared one.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    definition: Definition,
    group: Option<String>,
}

/// Where a parameter is bound: how it is held, and the categories whose elements have it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Binding {
    holder: Holder,
    default: Option<Value>,
    categories: BTreeSet<CategoryId>,
}

/// One parameter an element of a category has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applicable<'a> {
    /// What the parameter is.
    pub definition: &'a Definition,
    /// Whether its value is on each element or on the type.
    pub holder: Holder,
    /// The value an element or type has when it has set none, if the binding gives one.
    pub default: Option<&'a Value>,
    /// The group of a shared parameter; `None` for one only this project has.
    pub group: Option<&'a str>,
}

/// A holder of parameter values, as the caller knows them: an element, or a type for a type
/// parameter, with its category and its values.
#[derive(Clone, Copy, Debug)]
pub struct HolderValues<'a> {
    /// The category of the element or type.
    pub category: CategoryId,
    /// Its values.
    pub values: &'a Values,
}

/// The result of unbinding: the new table, and how many holders had a value that goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unbound {
    /// The table with the parameter no longer bound to those categories.
    pub params: ProjectParams,
    /// How many of the given holders were in those categories and had a value for the parameter.
    /// The caller removes those values (with `Values::remove`) when it keeps the new table.
    pub dropped: usize,
}

/// A project's parameters and where they are bound.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectParams {
    entries: BTreeMap<Id, Entry>,
    bindings: BTreeMap<Id, Binding>,
}

/// The table's format this build writes (FORMAT.md). Earlier formats are read for ever.
const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireParameter {
    group: Option<String>,
    definition: Definition,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBinding {
    #[serde(with = "kit_ops::id")]
    parameter: Id,
    holder: Holder,
    default: Option<String>,
    categories: Vec<CategoryId>,
}

/// The stored form: field order here is the byte order.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: u32,
    parameters: Vec<WireParameter>,
    bindings: Vec<WireBinding>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl ProjectParams {
    /// A project with no parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The definition of the parameter with this ID.
    #[must_use]
    pub fn definition(&self, id: &Id) -> Option<&Definition> {
        self.entries.get(id).map(|entry| &entry.definition)
    }

    /// How many parameters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Where a parameter is bound: how it is held and its categories in ID order; `None` if it is
    /// not bound.
    #[must_use]
    pub fn binding(&self, id: &Id) -> Option<(Holder, Vec<CategoryId>)> {
        self.bindings
            .get(id)
            .map(|b| (b.holder, b.categories.iter().copied().collect()))
    }

    /// The default value of a bound parameter's binding; `None` if it has none or is not bound.
    #[must_use]
    pub fn default_of(&self, id: &Id) -> Option<&Value> {
        self.bindings.get(id).and_then(|b| b.default.as_ref())
    }

    fn insert(&self, definition: Definition, group: Option<String>) -> Result<Self, ProjectError> {
        if info::is_builtin_id(definition.id()) {
            return Err(ProjectError::ReservedId(definition.id().clone()));
        }
        if let Some(existing) = self.entries.get(definition.id()) {
            return match difference(
                &existing.definition,
                existing.group.as_deref(),
                &definition,
                group.as_deref(),
            ) {
                None => Ok(self.clone()),
                Some("kind") => Err(ProjectError::KindConflict {
                    id: definition.id().clone(),
                    name: existing.definition.name().to_owned(),
                    existing: existing.definition.kind(),
                    incoming: definition.kind(),
                }),
                Some(field) => Err(ProjectError::Conflict {
                    id: definition.id().clone(),
                    name: existing.definition.name().to_owned(),
                    field,
                }),
            };
        }
        if self
            .entries
            .values()
            .any(|other| same_name(other.definition.name(), definition.name()))
        {
            return Err(ProjectError::DuplicateName(definition.name().to_owned()));
        }
        let mut next = self.clone();
        next.entries
            .insert(definition.id().clone(), Entry { definition, group });
        Ok(next)
    }

    /// The table with a parameter only this project has. Adding the same one again changes
    /// nothing.
    ///
    /// # Errors
    /// [`ProjectError::ReservedId`] for a project information ID, [`ProjectError::KindConflict`] or
    /// [`ProjectError::Conflict`] if the ID is already a different parameter, or
    /// [`ProjectError::DuplicateName`].
    pub fn add_project_parameter(&self, definition: Definition) -> Result<Self, ProjectError> {
        self.insert(definition, None)
    }

    /// The table with a shared parameter, as a project takes one in from a shared parameter file.
    /// Adding the same one again changes nothing; the same UUID with another kind is refused.
    ///
    /// # Errors
    /// As [`ProjectParams::add_project_parameter`].
    pub fn add_shared_parameter(&self, parameter: &SharedParameter) -> Result<Self, ProjectError> {
        self.insert(
            parameter.definition().clone(),
            Some(parameter.group().to_owned()),
        )
    }

    /// Binds the parameter to `targets`, as `holder`: every element of those categories has it.
    /// Categories it was already bound to stay. `default` is the value an element or type has when
    /// it has set none (the resolution order is instance, then type, then this); it must be a
    /// value the parameter takes, and it replaces any default the binding had, so `None` clears
    /// it. See the module's rules.
    ///
    /// # Errors
    /// [`ProjectError::UnknownParameter`], [`ProjectError::NoCategories`],
    /// [`ProjectError::UnknownCategory`], [`ProjectError::SubcategoryNotBindable`],
    /// [`ProjectError::CannotHold`], [`ProjectError::HolderConflict`] or
    /// [`ProjectError::BadDefault`] (a default of the wrong kind, or not one of a choice's keys).
    pub fn bind(
        &self,
        categories: &CategoryTable,
        id: &Id,
        targets: &[CategoryId],
        holder: Holder,
        default: Option<Value>,
    ) -> Result<Self, ProjectError> {
        let entry = self
            .entries
            .get(id)
            .ok_or_else(|| ProjectError::UnknownParameter(id.clone()))?;
        if let Some(value) = &default {
            entry
                .definition
                .check(value)
                .map_err(ProjectError::BadDefault)?;
        }
        if targets.is_empty() {
            return Err(ProjectError::NoCategories);
        }
        for &target in targets {
            let category = categories
                .get(target)
                .ok_or(ProjectError::UnknownCategory(target))?;
            if let Some(parent) = category.parent {
                return Err(ProjectError::SubcategoryNotBindable {
                    category: target,
                    parent,
                });
            }
            if !can_hold(category.kind, holder) {
                return Err(ProjectError::CannotHold {
                    category: target,
                    kind: category.kind,
                    holder,
                });
            }
        }
        let mut next = self.clone();
        let binding = next.bindings.entry(id.clone()).or_insert_with(|| Binding {
            holder,
            default: None,
            categories: BTreeSet::new(),
        });
        if binding.holder != holder {
            return Err(ProjectError::HolderConflict {
                id: id.clone(),
                name: entry.definition.name().to_owned(),
                existing: binding.holder,
                requested: holder,
            });
        }
        binding.default = default;
        binding.categories.extend(targets.iter().copied());
        Ok(next)
    }

    /// Unbinds the parameter from `targets`, and reports how many of `holders` (elements, or types
    /// for a type parameter) are in those categories and have a value for it: those values are
    /// dropped when the caller keeps the new table. The parameter's definition stays.
    ///
    /// # Errors
    /// [`ProjectError::UnknownParameter`], [`ProjectError::NoCategories`] or
    /// [`ProjectError::NotBound`] naming the first category it is not bound to.
    pub fn unbind(
        &self,
        id: &Id,
        targets: &[CategoryId],
        holders: &[HolderValues<'_>],
    ) -> Result<Unbound, ProjectError> {
        let entry = self
            .entries
            .get(id)
            .ok_or_else(|| ProjectError::UnknownParameter(id.clone()))?;
        if targets.is_empty() {
            return Err(ProjectError::NoCategories);
        }
        let mut next = self.clone();
        let not_bound = |category| ProjectError::NotBound {
            id: id.clone(),
            name: entry.definition.name().to_owned(),
            category,
        };
        let binding = next
            .bindings
            .get_mut(id)
            .ok_or_else(|| not_bound(targets.first().copied().unwrap_or(CategoryId(0))))?;
        for &target in targets {
            if !binding.categories.contains(&target) {
                return Err(not_bound(target));
            }
        }
        for target in targets {
            binding.categories.remove(target);
        }
        if binding.categories.is_empty() {
            next.bindings.remove(id);
        }
        let dropped = holders
            .iter()
            .filter(|h| targets.contains(&h.category) && h.values.get(id).is_some())
            .count();
        Ok(Unbound {
            params: next,
            dropped,
        })
    }

    /// The parameters an element of `category` has, in one stable order: by name (case ignored),
    /// then ID. A subcategory gives its parent's, since an element is in a top-level category; an
    /// unknown category gives none.
    #[must_use]
    pub fn parameters_of(
        &self,
        categories: &CategoryTable,
        category: CategoryId,
    ) -> Vec<Applicable<'_>> {
        let Some(found) = categories.get(category) else {
            return Vec::new();
        };
        let top = found.parent.unwrap_or(found.id);
        let mut have: Vec<Applicable<'_>> = self
            .bindings
            .iter()
            .filter(|(_, binding)| binding.categories.contains(&top))
            .filter_map(|(id, binding)| {
                self.entries.get(id).map(|entry| Applicable {
                    definition: &entry.definition,
                    holder: binding.holder,
                    default: binding.default.as_ref(),
                    group: entry.group.as_deref(),
                })
            })
            .collect();
        have.sort_by_cached_key(|a| {
            (
                a.definition.name().to_lowercase(),
                a.definition.id().clone(),
            )
        });
        have
    }

    /// The table's stored bytes: the same table always gives the same bytes (FORMAT.md).
    ///
    /// # Errors
    /// [`ProjectError::Malformed`] if the text cannot be written, which a valid table never hits.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ProjectError> {
        let wire = Wire {
            format: FORMAT,
            parameters: self
                .entries
                .values()
                .map(|e| WireParameter {
                    group: e.group.clone(),
                    definition: e.definition.clone(),
                })
                .collect(),
            bindings: self
                .bindings
                .iter()
                .map(|(id, b)| WireBinding {
                    parameter: id.clone(),
                    holder: b.holder,
                    default: b.default.as_ref().map(Value::stored),
                    categories: b.categories.iter().copied().collect(),
                })
                .collect(),
        };
        serde_json::to_vec(&wire).map_err(|e| ProjectError::Malformed(e.to_string()))
    }

    /// Reads a stored table against the project's categories. Every rule of the adding and binding
    /// functions is checked again, and the bytes must be the one spelling
    /// [`ProjectParams::to_bytes`] writes.
    ///
    /// # Errors
    /// [`ProjectError::UnsupportedFormat`] for a format this build does not know, otherwise
    /// [`ProjectError::Malformed`] or the rule that an entry broke.
    pub fn from_bytes(bytes: &[u8], categories: &CategoryTable) -> Result<Self, ProjectError> {
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|e| ProjectError::Malformed(e.to_string()))?;
        if wire.format != FORMAT {
            return Err(ProjectError::UnsupportedFormat(wire.format));
        }
        let mut table = Self::new();
        for entry in wire.parameters {
            table = table.insert(entry.definition, entry.group)?;
        }
        for binding in wire.bindings {
            let default = match (&binding.default, table.entries.get(&binding.parameter)) {
                (Some(text), Some(entry)) => Some(
                    entry
                        .definition
                        .read(text)
                        .map_err(ProjectError::BadDefault)?,
                ),
                _ => None,
            };
            table = table.bind(
                categories,
                &binding.parameter,
                &binding.categories,
                binding.holder,
                default,
            )?;
        }
        if table.to_bytes()? != bytes {
            return Err(ProjectError::Malformed(
                "not in its one spelling (order, repeats, spacing or escapes)".to_owned(),
            ));
        }
        Ok(table)
    }
}
