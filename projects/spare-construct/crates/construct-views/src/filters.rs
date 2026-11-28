//! View filters (CON-A8-F3-P4): a rule on a category and one of its parameters that gives the
//! elements it matches their own graphics and visibility in a view or a view template.
//!
//! A [`Filter`] names a category, a parameter (by its ID), an [`Operator`] and a value, and the
//! [`ElementStyle`] it gives what it matches. An element matches when it is of the filter's
//! category (or of a subcategory of it), reads a value for the parameter, and that value stands in
//! the operator's relation to the filter's value:
//!
//! - `equals`: the same value of the same kind; text is compared exactly.
//! - `contains`: text, a choice or a link holds the filter's text, without regard to case.
//! - `greater_than`: a length, area, volume, angle, number or whole number is greater than the
//!   filter's value (the same kind, compared exactly as the kind's own whole units).
//!
//! An element that reads no value, or one of another kind than the filter's, matches nothing.
//! [`Operator::fits`] says which parameter kinds an operator can be given.
//!
//! **Order decides.** A view's filters are an ordered list. For one element, [`Filters::style_for`]
//! walks the list from the first filter down and takes, for each part of style (pen, pattern,
//! colour, halftone, shown), the first matching filter that names it, so a filter higher in the
//! list wins any part it names and a lower one fills in the parts it leaves. Filters are asked of
//! the element's parameters whenever a view is drawn or listed, never stored against an element,
//! so a change to a parameter changes the result at once.
//!
//! Stored form ([`FILTERS_KEY`] on a view or a template record): one JSON text written only when
//! there is a filter, `{"format":1,"filters":[{"id":1,"name":"...","category":100,
//! "parameter":"<id>","op":"equals","value":"text:...","style":{...}}]}`, in priority order, each
//! `id` a whole number above zero unique in the list, no spare space. The value is
//! construct-params' stored text.

use crate::error::ViewError;
use crate::style::ElementStyle;
use construct_categories::{CategoryId, CategoryTable, VIEWS};
use construct_params::{Kind, Value};
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The record key a view's or a template's filters are stored under.
pub const FILTERS_KEY: &str = "@view.filters";
/// The version of the stored form this crate writes.
pub const FILTERS_FORMAT_VERSION: u32 = 1;

/// How a filter compares an element's value with its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    /// The same value.
    Equals,
    /// A text that holds the filter's text, case aside.
    Contains,
    /// An ordered value above the filter's.
    GreaterThan,
}

impl Operator {
    /// Every operator, in the order a page lists them.
    pub const ALL: [Self; 3] = [Self::Equals, Self::Contains, Self::GreaterThan];

    /// The key an operator is spelt with, in commands, answers and records.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Equals => "equals",
            Self::Contains => "contains",
            Self::GreaterThan => "greater_than",
        }
    }

    /// The operator a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|op| op.key() == key)
    }

    /// Whether a parameter of `kind` can be given this operator.
    #[must_use]
    pub const fn fits(self, kind: Kind) -> bool {
        match self {
            Self::Equals => true,
            Self::Contains => matches!(kind, Kind::Text | Kind::Choice | Kind::Link),
            Self::GreaterThan => matches!(
                kind,
                Kind::Length | Kind::Area | Kind::Volume | Kind::Angle | Kind::Number | Kind::Whole
            ),
        }
    }
}

/// The text of a value that holds text.
fn text_of(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) | Value::Choice(text) | Value::Link(text) => Some(text),
        _ => None,
    }
}

/// Whether `have` is above `bound`: two values of one ordered kind, compared as the kind's own
/// whole units (or exactly, for a number). Anything else is not.
fn above(have: &Value, bound: &Value) -> bool {
    match (have, bound) {
        (Value::Length(a), Value::Length(b)) => a.steps() > b.steps(),
        (Value::Area(a), Value::Area(b)) => a.square_steps() > b.square_steps(),
        (Value::Volume(a), Value::Volume(b)) => a.cubic_steps() > b.cubic_steps(),
        (Value::Angle(a), Value::Angle(b)) => a.units() > b.units(),
        (Value::Number(a), Value::Number(b)) => a > b,
        (Value::Whole(a), Value::Whole(b)) => a > b,
        _ => false,
    }
}

/// One filter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    /// Its number in its list, above zero and never reused while it is in the list.
    pub id: u32,
    /// Its name, unique in its list without regard to case.
    pub name: String,
    /// The category (or the one a subcategory belongs to) it applies to.
    pub category: CategoryId,
    /// The parameter it reads.
    pub parameter: Id,
    /// How the parameter's value is compared.
    pub op: Operator,
    /// What it is compared with.
    pub value: Value,
    /// How what matches looks.
    pub style: ElementStyle,
}

impl Filter {
    /// Whether an element of `category` matches, reading its parameters through `read`.
    #[must_use]
    pub fn matches(
        &self,
        categories: &CategoryTable,
        category: CategoryId,
        read: &dyn Fn(&Id) -> Option<Value>,
    ) -> bool {
        let of_it = category == self.category
            || categories.get(category).and_then(|found| found.parent) == Some(self.category);
        if !of_it {
            return false;
        }
        let Some(have) = read(&self.parameter) else {
            return false;
        };
        match self.op {
            Operator::Equals => have == self.value,
            Operator::Contains => text_of(&have)
                .zip(text_of(&self.value))
                .is_some_and(|(held, wanted)| held.to_lowercase().contains(&wanted.to_lowercase())),
            Operator::GreaterThan => above(&have, &self.value),
        }
    }
}

/// An ordered list of filters, first the one that wins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    list: Vec<Filter>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredFilter {
    id: u32,
    name: String,
    category: CategoryId,
    parameter: String,
    op: String,
    value: String,
    style: ElementStyle,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    format: u32,
    filters: Vec<StoredFilter>,
}

/// A filter name as kept: trimmed, and never empty.
fn clean_name(name: &str) -> Result<String, ViewError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ViewError::EmptyFilterName);
    }
    Ok(name.to_owned())
}

impl Filters {
    /// No filters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether there is none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// How many there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Every filter, the winning one first.
    pub fn iter(&self) -> impl Iterator<Item = &Filter> {
        self.list.iter()
    }

    /// The filter with this number.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&Filter> {
        self.list.iter().find(|filter| filter.id == id)
    }

    /// Every parameter any filter reads.
    #[must_use]
    pub fn parameters(&self) -> BTreeSet<Id> {
        self.list
            .iter()
            .map(|filter| filter.parameter.clone())
            .collect()
    }

    fn name_free(&self, name: &str, except: Option<u32>) -> Result<(), ViewError> {
        let wanted = name.to_lowercase();
        if self
            .list
            .iter()
            .any(|filter| Some(filter.id) != except && filter.name.to_lowercase() == wanted)
        {
            return Err(ViewError::DuplicateFilterName(name.to_owned()));
        }
        Ok(())
    }

    /// These filters with `filter` added last (the lowest priority), under the next free number.
    /// The number in `filter` is ignored; the new number is given back.
    ///
    /// # Errors
    /// [`ViewError::EmptyFilterName`], [`ViewError::DuplicateFilterName`] or
    /// [`ViewError::OutOfRange`] when there is no number left.
    pub fn added(&self, filter: Filter) -> Result<(Self, u32), ViewError> {
        let name = clean_name(&filter.name)?;
        self.name_free(&name, None)?;
        let id = self
            .list
            .iter()
            .map(|one| one.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| ViewError::OutOfRange("there is no filter number left".to_owned()))?;
        let mut list = self.list.clone();
        list.push(Filter { id, name, ..filter });
        Ok((Self { list }, id))
    }

    /// These filters with the one of `filter`'s number replaced, keeping its place.
    ///
    /// # Errors
    /// [`ViewError::UnknownFilter`], [`ViewError::EmptyFilterName`] or
    /// [`ViewError::DuplicateFilterName`].
    pub fn replaced(&self, filter: Filter) -> Result<Self, ViewError> {
        let name = clean_name(&filter.name)?;
        self.name_free(&name, Some(filter.id))?;
        let mut list = self.list.clone();
        let slot = list
            .iter_mut()
            .find(|one| one.id == filter.id)
            .ok_or(ViewError::UnknownFilter(filter.id))?;
        *slot = Filter { name, ..filter };
        Ok(Self { list })
    }

    /// These filters without the one of this number.
    ///
    /// # Errors
    /// [`ViewError::UnknownFilter`].
    pub fn removed(&self, id: u32) -> Result<Self, ViewError> {
        if self.get(id).is_none() {
            return Err(ViewError::UnknownFilter(id));
        }
        Ok(Self {
            list: self
                .list
                .iter()
                .filter(|filter| filter.id != id)
                .cloned()
                .collect(),
        })
    }

    /// These filters with the one of this number moved to place `to` (0 is the first, the one that
    /// wins).
    ///
    /// # Errors
    /// [`ViewError::UnknownFilter`], or [`ViewError::FilterPosition`] for a place the list does
    /// not have.
    pub fn moved(&self, id: u32, to: usize) -> Result<Self, ViewError> {
        let from = self
            .list
            .iter()
            .position(|filter| filter.id == id)
            .ok_or(ViewError::UnknownFilter(id))?;
        if to >= self.list.len() {
            return Err(ViewError::FilterPosition {
                to,
                len: self.list.len(),
            });
        }
        let mut list = self.list.clone();
        let one = list.remove(from);
        list.insert(to, one);
        Ok(Self { list })
    }

    /// How the filters say an element of `category` looks: the first matching filter's parts, then
    /// what later matching filters add for the parts it leaves out. Nothing matching is an empty
    /// style.
    #[must_use]
    pub fn style_for(
        &self,
        categories: &CategoryTable,
        category: CategoryId,
        read: &dyn Fn(&Id) -> Option<Value>,
    ) -> ElementStyle {
        self.list
            .iter()
            .filter(|filter| filter.matches(categories, category, read))
            .fold(ElementStyle::default(), |won, filter| won.or(&filter.style))
    }

    /// Asks whether the filters stand against `categories`: each filter's category exists and is
    /// one a view draws. Whether the parameters exist is the document's to ask.
    ///
    /// # Errors
    /// [`ViewError::UnknownCategory`] or [`ViewError::NotStyled`].
    pub fn check(&self, categories: &CategoryTable) -> Result<(), ViewError> {
        for filter in &self.list {
            let found = categories
                .get(filter.category)
                .ok_or(ViewError::UnknownCategory(filter.category))?;
            if found.id == VIEWS || found.parent == Some(VIEWS) {
                return Err(ViewError::NotStyled(filter.category));
            }
        }
        Ok(())
    }

    /// The stored text, or `None` for no filters (the record then has no key).
    pub(crate) fn to_text(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let filters = self
            .list
            .iter()
            .map(|filter| StoredFilter {
                id: filter.id,
                name: filter.name.clone(),
                category: filter.category,
                parameter: filter.parameter.to_string(),
                op: filter.op.key().to_owned(),
                value: filter.value.stored(),
                style: filter.style.clone(),
            })
            .collect();
        // Plain structs of strings and numbers: serialising one cannot fail, and a record that
        // held nothing here would be refused when read.
        Some(
            serde_json::to_string(&Stored {
                format: FILTERS_FORMAT_VERSION,
                filters,
            })
            .unwrap_or_default(),
        )
    }

    /// Reads the stored text; the caller compares the record it makes of the result with the one
    /// it was given, so any other spelling is refused there.
    pub(crate) fn from_text(text: &str) -> Result<Self, String> {
        let stored: Stored = serde_json::from_str(text)
            .map_err(|why| format!("holds filters that do not read: {why}"))?;
        if stored.format != FILTERS_FORMAT_VERSION {
            return Err(format!(
                "holds filters of version {}, which is not known",
                stored.format
            ));
        }
        let mut list: Vec<Filter> = Vec::new();
        for one in stored.filters {
            let parameter = Id::parse(&one.parameter)
                .map_err(|_| "holds a filter of a parameter with no ID".to_owned())?;
            let op = Operator::from_key(&one.op).ok_or_else(|| {
                "holds a filter with an operator that is not one of ours".to_owned()
            })?;
            let value = Value::from_stored(&one.value)
                .ok_or_else(|| "holds a filter value that is not in its one spelling".to_owned())?;
            if one.id == 0 || list.iter().any(|before| before.id == one.id) {
                return Err("holds filters whose numbers are not unique and above zero".to_owned());
            }
            if one.name.trim() != one.name || one.name.is_empty() {
                return Err("holds a filter with a name that is not as written".to_owned());
            }
            list.push(Filter {
                id: one.id,
                name: one.name,
                category: one.category,
                parameter,
                op,
                value,
                style: one.style,
            });
        }
        if list.is_empty() {
            return Err("holds filters with no filter in them".to_owned());
        }
        Ok(Self { list })
    }
}
