//! One schedule: its ID, name, the category it lists, its fields in order, its filters and its
//! sorting, written as an element record of the schedules category and read back strictly.

use crate::error::ScheduleError;
use crate::field::{Basis, Field, QUANTITIES, Quantity};
use crate::filter::{Comparison, Filter};
use construct_categories::{CategoryId, SCHEDULES};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::Value;
use construct_phases::ElementStaging;
use kit_core::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The `kind` of every schedule record, and its recipe's operation.
pub const SCHEDULE_KIND: &str = "schedule";
/// The record's category, as every record carries it (construct-types' key).
pub const CATEGORY_KEY: &str = "@category";
/// The category the schedule lists, as its number.
pub const LISTS_KEY: &str = "@schedule.lists";
/// The fields, in order: a JSON list of field keys.
pub const FIELDS_KEY: &str = "@schedule.fields";
/// The filters: a JSON list of `{"field", "is", "to"}`, `to` a value in construct-params' stored
/// spelling.
pub const FILTERS_KEY: &str = "@schedule.filters";
/// The sorting, first key first: a JSON list of `{"field", "order"}`.
pub const SORTING_KEY: &str = "@schedule.sorting";
/// The schedule's own staging fields, as the phases crate's JSON.
pub const STAGING_KEY: &str = "@staging";

/// Where a schedule record sits: a schedule has no shape, so nowhere.
const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a schedule record: Spare Construct's own operation `schedule`, version 1, with
/// no arguments. It builds no shape. Its version is the record's format version.
#[must_use]
pub fn schedule_recipe() -> Recipe {
    Recipe::construct(SCHEDULE_KIND, BTreeMap::new())
}

/// A name with something in it besides spaces, trimmed.
pub(crate) fn clean_name(name: &str) -> Result<String, ScheduleError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ScheduleError::EmptyName);
    }
    Ok(name.to_owned())
}

/// One key of a schedule's sorting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sort {
    /// The field sorted on.
    pub field: Field,
    /// Largest or last first.
    pub descending: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredFilter {
    field: String,
    is: String,
    to: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSort {
    field: String,
    order: String,
}

/// A schedule: a stored query over a document's elements. It holds no rows: they are worked out
/// from the elements as they are now ([`crate::rows`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    id: Id,
    name: String,
    lists: CategoryId,
    fields: Vec<Field>,
    filters: Vec<Filter>,
    sorting: Vec<Sort>,
    staging: ElementStaging,
}

impl Schedule {
    /// A schedule of a category with no fields, filters or sorting yet.
    ///
    /// # Errors
    /// [`ScheduleError::EmptyName`] if the name is only spaces.
    pub fn new(
        id: Id,
        name: &str,
        lists: CategoryId,
        staging: ElementStaging,
    ) -> Result<Self, ScheduleError> {
        Ok(Self {
            id,
            name: clean_name(name)?,
            lists,
            fields: Vec::new(),
            filters: Vec::new(),
            sorting: Vec::new(),
            staging,
        })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Its name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The category it lists.
    #[must_use]
    pub fn lists(&self) -> CategoryId {
        self.lists
    }

    /// Its fields, in order.
    #[must_use]
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// Its filters; a row is listed when all hold.
    #[must_use]
    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }

    /// Its sorting, first key first.
    #[must_use]
    pub fn sorting(&self) -> &[Sort] {
        &self.sorting
    }

    /// Its staging fields.
    #[must_use]
    pub fn staging(&self) -> &ElementStaging {
        &self.staging
    }

    /// The schedule under another name.
    ///
    /// # Errors
    /// [`ScheduleError::EmptyName`].
    pub fn renamed(mut self, name: &str) -> Result<Self, ScheduleError> {
        self.name = clean_name(name)?;
        Ok(self)
    }

    /// The schedule with these fields.
    #[must_use]
    pub fn with_fields(mut self, fields: Vec<Field>) -> Self {
        self.fields = fields;
        self
    }

    /// The schedule with these filters.
    #[must_use]
    pub fn with_filters(mut self, filters: Vec<Filter>) -> Self {
        self.filters = filters;
        self
    }

    /// The schedule with this sorting.
    #[must_use]
    pub fn with_sorting(mut self, sorting: Vec<Sort>) -> Self {
        self.sorting = sorting;
        self
    }

    /// The schedule with these staging fields.
    #[must_use]
    pub fn with_staging(mut self, staging: ElementStaging) -> Self {
        self.staging = staging;
        self
    }

    /// Whether the schedule stands in a document: its category can be listed, every field,
    /// filter and sort key is one the category has, no field or sort key is repeated, and every
    /// filter's value suits its field.
    ///
    /// # Errors
    /// The first thing that does not stand, as a [`ScheduleError`].
    pub fn check(&self, basis: &Basis<'_>) -> Result<(), ScheduleError> {
        self.check_among(basis, &QUANTITIES)
    }

    /// [`Schedule::check`], a category's quantities being those among `quantities`.
    ///
    /// # Errors
    /// The first thing that does not stand, as a [`ScheduleError`].
    pub fn check_among(
        &self,
        basis: &Basis<'_>,
        quantities: &[Quantity],
    ) -> Result<(), ScheduleError> {
        basis.listable(self.lists)?;
        for (at, field) in self.fields.iter().enumerate() {
            basis.require_among(quantities, self.lists, field)?;
            if self.fields.iter().take(at).any(|before| before == field) {
                return Err(ScheduleError::DuplicateField(field.key()));
            }
        }
        for filter in &self.filters {
            let definition = basis.require_among(quantities, self.lists, &filter.field)?;
            filter.check(&definition)?;
        }
        for (at, sort) in self.sorting.iter().enumerate() {
            basis.require_among(quantities, self.lists, &sort.field)?;
            if self
                .sorting
                .iter()
                .take(at)
                .any(|before| before.field == sort.field)
            {
                return Err(ScheduleError::DuplicateSort(sort.field.key()));
            }
        }
        Ok(())
    }

    /// The schedule as its element record in `document`.
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), SCHEDULES.0.to_string());
        props.insert(LISTS_KEY.to_owned(), self.lists.0.to_string());
        // Plain lists of strings: serialising one cannot fail, and a record that held nothing
        // here would be refused when read.
        let keys: Vec<String> = self.fields.iter().map(Field::key).collect();
        props.insert(
            FIELDS_KEY.to_owned(),
            serde_json::to_string(&keys).unwrap_or_default(),
        );
        let filters: Vec<StoredFilter> = self
            .filters
            .iter()
            .map(|filter| StoredFilter {
                field: filter.field.key(),
                is: filter.comparison.key().to_owned(),
                to: filter.value.stored(),
            })
            .collect();
        props.insert(
            FILTERS_KEY.to_owned(),
            serde_json::to_string(&filters).unwrap_or_default(),
        );
        let sorting: Vec<StoredSort> = self
            .sorting
            .iter()
            .map(|sort| StoredSort {
                field: sort.field.key(),
                order: if sort.descending {
                    "descending"
                } else {
                    "ascending"
                }
                .to_owned(),
            })
            .collect();
        props.insert(
            SORTING_KEY.to_owned(),
            serde_json::to_string(&sorting).unwrap_or_default(),
        );
        props.insert(
            STAGING_KEY.to_owned(),
            serde_json::to_string(&self.staging).unwrap_or_default(),
        );
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: SCHEDULE_KIND.to_owned(),
            name: self.name.clone(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: schedule_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a schedule from its element record. Anything [`Schedule::to_record`] would not
    /// write is refused, never repaired, so one schedule has one byte string. Whether the
    /// schedule stands in the document ([`Schedule::check`]) is a different question: a
    /// parameter that has gone leaves the schedule readable.
    ///
    /// # Errors
    /// [`ScheduleError::Malformed`], naming the record.
    pub fn from_record(record: &Element) -> Result<Self, ScheduleError> {
        let malformed = |why: &str| {
            ScheduleError::Malformed(format!("The schedule record {} {why}.", record.id))
        };
        if record.kind != SCHEDULE_KIND || record.recipe != schedule_recipe() {
            return Err(malformed(
                "is not of kind schedule, with the schedule recipe",
            ));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
            return Err(malformed("has a level, a placement or references"));
        }
        let lists = record
            .props
            .get(LISTS_KEY)
            .and_then(|text| text.parse::<u32>().ok())
            .map(CategoryId)
            .ok_or_else(|| malformed("lists no category"))?;
        let staging = record
            .props
            .get(STAGING_KEY)
            .and_then(|text| serde_json::from_str::<ElementStaging>(text).ok())
            .ok_or_else(|| malformed("holds no staging fields"))?;
        let keys: Vec<String> = record
            .props
            .get(FIELDS_KEY)
            .and_then(|text| serde_json::from_str(text).ok())
            .ok_or_else(|| malformed("holds no list of fields"))?;
        let fields = keys
            .iter()
            .map(|key| Field::parse(key))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| malformed(&format!("holds a field that does not read ({e})")))?;
        let stored: Vec<StoredFilter> = record
            .props
            .get(FILTERS_KEY)
            .and_then(|text| serde_json::from_str(text).ok())
            .ok_or_else(|| malformed("holds no list of filters"))?;
        let mut filters = Vec::new();
        for one in stored {
            let field = Field::parse(&one.field).map_err(|e| {
                malformed(&format!(
                    "holds a filter on a field that does not read ({e})"
                ))
            })?;
            let comparison = Comparison::parse(&one.is)
                .map_err(|e| malformed(&format!("holds a filter that does not read ({e})")))?;
            let value = Value::from_stored(&one.to)
                .filter(|value| value.stored() == one.to)
                .ok_or_else(|| malformed("holds a filter value in no one spelling"))?;
            filters.push(Filter {
                field,
                comparison,
                value,
            });
        }
        let stored: Vec<StoredSort> = record
            .props
            .get(SORTING_KEY)
            .and_then(|text| serde_json::from_str(text).ok())
            .ok_or_else(|| malformed("holds no sorting"))?;
        let mut sorting = Vec::new();
        for one in stored {
            let field = Field::parse(&one.field).map_err(|e| {
                malformed(&format!("holds a sort on a field that does not read ({e})"))
            })?;
            let descending = match one.order.as_str() {
                "ascending" => false,
                "descending" => true,
                _ => {
                    return Err(malformed(
                        "holds a sort that is neither ascending nor descending",
                    ));
                }
            };
            sorting.push(Sort { field, descending });
        }
        let schedule = Self::new(record.id.clone(), &record.name, lists, staging)
            .map_err(|_| malformed("has no name"))?
            .with_fields(fields)
            .with_filters(filters)
            .with_sorting(sorting);
        if schedule.to_record(&record.document) != *record {
            return Err(malformed("is not as a schedule record is written"));
        }
        Ok(schedule)
    }
}
