//! What the page shows of a document's categories, parameter kinds, project parameters and types,
//! so a person can manage them. Each is a read of the same records the commands check against and
//! the regenerator builds from ([`crate::parameters`]); nothing here is kept anywhere, and nothing
//! here changes a record (the commands in construct-api do).
//!
//! What it promises:
//! - [`Catalog`]: every parameter kind and every category of the document, read from the data
//!   crates, so a page lists them and never holds a list of its own.
//! - [`TableView`]: every parameter of the document, bound or not, with its kind, whether an
//!   instance or its type holds it, its choices, its default and the categories it is bound to.
//! - [`TypesView`]: the types of a document (or of one category), each with the parameters it
//!   lists and the values it holds, shown in the document's units, and how many elements are of it.
//! - Values are shown as the document's units format them
//!   ([`construct_params::Definition::format`]): the page never does unit arithmetic. The order is
//!   the same every time: by name (case ignored), then ID.
//!
//! Depends on construct-categories, construct-params, construct-types and [`crate::parameters`].
//! Nothing here is interim.

use crate::parameters::{CategoryView, Context, TYPE_KIND, read_document, type_of};
use construct_categories::CategoryId;
use construct_model::element::Element;
use construct_params::{Holder, Kind};
use construct_types::{ElementType, Types};
use kit_core::Id;
use kit_store::{Reader, StoreError};
use serde::Serialize;
use std::collections::BTreeMap;

/// The kinds of parameter and the categories of a document, for a page's lists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Catalog {
    /// Every kind of parameter, in construct-params' own order.
    pub kinds: Vec<KindView>,
    /// Every category, the built-in tree in order, then the document's own.
    pub categories: Vec<CategoryEntry>,
}

/// A kind of parameter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KindView {
    /// Its tag, such as `length`: what a definition's `kind` says.
    pub tag: &'static str,
    /// What people read, such as `a length`.
    pub label: String,
}

/// A category as a list shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CategoryEntry {
    /// Its permanent ID.
    pub id: u32,
    /// Its name.
    pub name: String,
    /// The top-level category it is under; `None` for a top-level category. Parameters bind to
    /// top-level categories.
    pub parent: Option<u32>,
}

/// A document's project parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TableView {
    /// Every parameter, by name.
    pub parameters: Vec<DefinitionView>,
}

/// One project parameter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DefinitionView {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its kind's tag.
    pub kind: &'static str,
    /// Its kind as people read it.
    pub kind_label: String,
    /// `instance` or `type`: who holds its value.
    pub holder: &'static str,
    /// The list a choice parameter takes; empty for any other.
    pub choices: Vec<ChoiceView>,
    /// The default its binding gives, shown in the document's units; `None` when it has none.
    pub default: Option<String>,
    /// The categories it is bound to; empty while it is bound nowhere.
    pub categories: Vec<CategoryView>,
}

/// One entry of a choice parameter's list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChoiceView {
    /// What is stored.
    pub key: String,
    /// What people see.
    pub label: String,
}

/// A document's types.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TypesView {
    /// The category asked about, if one was.
    pub category: Option<CategoryView>,
    /// The types, by name.
    pub types: Vec<TypeEntry>,
}

/// One type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TypeEntry {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its category.
    pub category: Option<CategoryView>,
    /// How many elements of the document are of it.
    pub elements: usize,
    /// Every parameter its elements have, by name.
    pub parameters: Vec<TypeParameter>,
}

/// One parameter of a type, with the value the type holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TypeParameter {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its kind's tag.
    pub kind: &'static str,
    /// `instance` or `type`: who holds its value. Only a type's own can be set on the type.
    pub holder: &'static str,
    /// The value the type holds, in its stored text.
    pub value: Option<String>,
    /// The same, shown in the document's units.
    pub shown: Option<String>,
    /// `type` when the type holds one, `default` when its elements read the default, `unset`
    /// when there is none, `instance` when each element holds its own.
    pub from: &'static str,
}

const fn holder_word(holder: Holder) -> &'static str {
    match holder {
        Holder::Instance => "instance",
        Holder::Type => "type",
    }
}

impl Context {
    /// The kinds of parameter and the categories of this document.
    #[must_use]
    pub fn catalog(&self) -> Catalog {
        Catalog {
            kinds: Kind::ALL
                .into_iter()
                .map(|kind| KindView {
                    tag: kind.tag(),
                    label: kind.to_string(),
                })
                .collect(),
            categories: self
                .categories
                .all()
                .into_iter()
                .map(|category| CategoryEntry {
                    id: category.id.0,
                    name: category.name,
                    parent: category.parent.map(|parent| parent.0),
                })
                .collect(),
        }
    }

    fn category_view(&self, id: CategoryId) -> Option<CategoryView> {
        self.categories.get(id).map(|found| CategoryView {
            id: id.0,
            name: found.name,
        })
    }

    /// Every project parameter with where it is bound.
    ///
    /// # Errors
    /// The refusal of a stored default, in words.
    pub fn table_view(&self) -> Result<TableView, String> {
        let mut parameters = Vec::new();
        for def in self.table.definitions() {
            let (holder, bound) = self
                .table
                .binding(def.id())
                .unwrap_or_else(|| (def.holder(), Vec::new()));
            let default = match self.table.default_of(def.id()) {
                Some(value) => Some(def.format(value, &self.units).map_err(|e| e.to_string())?),
                None => None,
            };
            parameters.push(DefinitionView {
                id: def.id().to_string(),
                name: def.name().to_owned(),
                kind: def.kind().tag(),
                kind_label: def.kind().to_string(),
                holder: holder_word(holder),
                choices: def
                    .choices()
                    .iter()
                    .map(|choice| ChoiceView {
                        key: choice.key.clone(),
                        label: choice.label.clone(),
                    })
                    .collect(),
                default,
                categories: bound
                    .into_iter()
                    .filter_map(|id| self.category_view(id))
                    .collect(),
            });
        }
        Ok(TableView { parameters })
    }

    /// The types given (those of `category` when one is), as a page shows them. `counts` says how
    /// many elements are of each type.
    ///
    /// # Errors
    /// The refusal of a stored value, in words.
    pub fn types_view(
        &self,
        category: Option<CategoryId>,
        types: &Types,
        counts: &BTreeMap<Id, usize>,
    ) -> Result<TypesView, String> {
        let mut shown: Vec<&ElementType> = match category {
            Some(category) => types.in_category(category),
            None => types.iter().collect(),
        };
        shown.sort_by_cached_key(|ty| (ty.name().to_lowercase(), ty.id().clone()));
        let mut entries = Vec::new();
        for ty in shown {
            entries.push(self.type_entry(ty, counts.get(ty.id()).copied().unwrap_or(0))?);
        }
        Ok(TypesView {
            category: category.and_then(|id| self.category_view(id)),
            types: entries,
        })
    }

    fn type_entry(&self, ty: &ElementType, elements: usize) -> Result<TypeEntry, String> {
        let mut parameters = Vec::new();
        for id in ty.params() {
            let Some(def) = self.params.definition(id) else {
                return Err(format!("no parameter has id {id}"));
            };
            let (value, from) = match (def.holder(), ty.value(id)) {
                (Holder::Instance, _) => (None, "instance"),
                (Holder::Type, Some(own)) => (Some(own), "type"),
                (Holder::Type, None) => match self.params.default_of(id) {
                    Some(default) => (Some(default), "default"),
                    None => (None, "unset"),
                },
            };
            let shown = match value {
                Some(value) => Some(def.format(value, &self.units).map_err(|e| e.to_string())?),
                None => None,
            };
            parameters.push(TypeParameter {
                id: id.to_string(),
                name: def.name().to_owned(),
                kind: def.kind().tag(),
                holder: holder_word(def.holder()),
                value: value.map(construct_params::Value::stored),
                shown,
                from,
            });
        }
        parameters.sort_by_cached_key(|p| (p.name.to_lowercase(), p.id.clone()));
        Ok(TypeEntry {
            id: ty.id().to_string(),
            name: ty.name().to_owned(),
            category: self.category_view(ty.category()),
            elements,
            parameters,
        })
    }
}

/// A document's project parameters, read against its context.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is why the stored table cannot be read.
pub async fn read_table_view<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<TableView, String>, StoreError> {
    let context = match crate::parameters::read_context(reader, project, document).await? {
        Ok(context) => context,
        Err(why) => return Ok(Err(why)),
    };
    Ok(context.table_view())
}

/// The kinds and categories of a document.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is why its parameter table cannot be
/// read.
pub async fn read_catalog<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<Catalog, String>, StoreError> {
    let context = crate::parameters::read_context(reader, project, document).await?;
    Ok(context.map(|context| context.catalog()))
}

/// A document's types (those of the category with this ID when it is given), with how many
/// elements are of each.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is why the stored types or values
/// cannot be read, or that no category has the ID.
pub async fn read_types_view<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    category: Option<u32>,
) -> Result<Result<TypesView, String>, StoreError> {
    let category = category.map(CategoryId);
    let context = match crate::parameters::read_context(reader, project, document).await? {
        Ok(context) => context,
        Err(why) => return Ok(Err(why)),
    };
    if let Some(id) = category.filter(|id| context.categories.get(*id).is_none()) {
        return Ok(Err(construct_categories::CategoryError::UnknownCategory(
            id,
        )
        .to_string()));
    }
    let all = read_document(reader, project, document).await?;
    let mut counts: BTreeMap<Id, usize> = BTreeMap::new();
    let mut records: Vec<Element> = Vec::new();
    for (_, record) in all.into_values() {
        if record.kind == TYPE_KIND {
            records.push(record);
        } else if let Some(id) = type_of(&record) {
            let count = counts.entry(id).or_insert(0);
            *count = count.saturating_add(1);
        }
    }
    let types = match Types::from_records(&records, &context.params, &context.categories) {
        Ok(types) => types,
        Err(e) => return Ok(Err(e.to_string())),
    };
    Ok(context.types_view(category, &types, &counts))
}
