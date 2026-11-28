//! One schedule: its ID, name, the category it lists, its fields in order, its filters, its
//! sorting and grouping, its totals and its calculated fields, written as an element record of
//! the schedules category and read back strictly.

use crate::calc::{self, Calculated, Names, reference_name};
use crate::error::ScheduleError;
use crate::field::{Basis, Field};
use crate::filter::{Comparison, Filter};
use crate::total::{TotalKind, Totals};
use construct_categories::{CategoryId, SCHEDULES};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Definition, Kind, Value};
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
/// The sorting, first key first: a JSON list of `{"field", "order"}`, with `"header":true` and
/// `"footer":true` on a key that groups.
pub const SORTING_KEY: &str = "@schedule.sorting";
/// The schedule's own staging fields, as the phases crate's JSON.
pub const STAGING_KEY: &str = "@staging";
/// `no` when equal rows are folded into one with a count; absent when every instance is listed.
pub const ITEMIZE_KEY: &str = "@schedule.itemize";
/// `yes` when the schedule ends with a grand total; absent when it does not.
pub const GRAND_KEY: &str = "@schedule.grand";
/// The totals of each column that has them: a JSON list of `{"field", "do"}`; absent when none.
pub const TOTALS_KEY: &str = "@schedule.totals";
/// The calculated fields: a JSON list of `{"name", "formula", "kind"}`; absent when none.
pub const CALCULATED_KEY: &str = "@schedule.calculated";

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

/// One key of a schedule's sorting. A key with a header or a footer also groups: the rows that
/// agree on it and on every key before it are one group, with a header above them (the key's
/// field and value), a footer below them (how many elements and their totals), or both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sort {
    /// The field sorted on.
    pub field: Field,
    /// Largest or last first.
    pub descending: bool,
    /// A header above each group of this key.
    pub header: bool,
    /// A footer below each group of this key.
    pub footer: bool,
}

impl Sort {
    /// A sort key that does not group.
    #[must_use]
    pub fn by(field: Field, descending: bool) -> Self {
        Self {
            field,
            descending,
            header: false,
            footer: false,
        }
    }

    /// Whether the key groups: it has a header or a footer.
    #[must_use]
    pub fn groups(&self) -> bool {
        self.header || self.footer
    }
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_false(value: &bool) -> bool {
    !*value
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
    #[serde(default, skip_serializing_if = "is_false")]
    header: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    footer: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredTotals {
    field: String,
    #[serde(rename = "do")]
    kinds: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCalculated {
    name: String,
    formula: String,
    kind: String,
}

/// A schedule: a stored query over a document's elements. It holds no rows: they are worked out
/// from the elements as they are now ([`crate::rows`], [`crate::table`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    id: Id,
    name: String,
    lists: CategoryId,
    fields: Vec<Field>,
    filters: Vec<Filter>,
    sorting: Vec<Sort>,
    itemize: bool,
    grand: bool,
    totals: Vec<Totals>,
    calculated: Vec<Calculated>,
    staging: ElementStaging,
}

impl Schedule {
    /// A schedule of a category with no fields, filters or sorting yet, every instance itemized.
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
            itemize: true,
            grand: false,
            totals: Vec::new(),
            calculated: Vec::new(),
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

    /// Whether every instance has a row (the default). Off, rows that agree in every column are
    /// folded into one with a count.
    #[must_use]
    pub fn itemize(&self) -> bool {
        self.itemize
    }

    /// Whether the table ends with a grand total.
    #[must_use]
    pub fn grand_totals(&self) -> bool {
        self.grand
    }

    /// The totals of its columns.
    #[must_use]
    pub fn totals(&self) -> &[Totals] {
        &self.totals
    }

    /// Its calculated fields, in the order they may use each other.
    #[must_use]
    pub fn calculated(&self) -> &[Calculated] {
        &self.calculated
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

    /// The schedule with every instance itemized or with equal rows folded.
    #[must_use]
    pub fn with_itemize(mut self, itemize: bool) -> Self {
        self.itemize = itemize;
        self
    }

    /// The schedule with a grand total or without.
    #[must_use]
    pub fn with_grand_totals(mut self, grand: bool) -> Self {
        self.grand = grand;
        self
    }

    /// The schedule with these totals.
    #[must_use]
    pub fn with_totals(mut self, totals: Vec<Totals>) -> Self {
        self.totals = totals;
        self
    }

    /// The schedule with these calculated fields.
    #[must_use]
    pub fn with_calculated(mut self, calculated: Vec<Calculated>) -> Self {
        self.calculated = calculated;
        self
    }

    /// The schedule with these staging fields.
    #[must_use]
    pub fn with_staging(mut self, staging: ElementStaging) -> Self {
        self.staging = staging;
        self
    }

    /// The definition of one of this schedule's fields, or `None` when neither the category nor
    /// the schedule's calculated fields have it.
    #[must_use]
    pub fn definition(&self, basis: &Basis<'_>, field: &Field) -> Option<Definition> {
        match field {
            Field::Calculated(name) => self
                .calculated
                .iter()
                .find(|c| c.name() == name)
                .and_then(Calculated::definition),
            _ => basis.definition(self.lists, field),
        }
    }

    /// The definition of a field the schedule must have.
    ///
    /// # Errors
    /// [`ScheduleError::NoSuchField`].
    pub fn require(&self, basis: &Basis<'_>, field: &Field) -> Result<Definition, ScheduleError> {
        self.definition(basis, field)
            .ok_or_else(|| ScheduleError::NoSuchField {
                category: basis.category_name(self.lists),
                field: field.key(),
            })
    }

    /// Every field the schedule can list, filter, sort or total: the category's, then its own
    /// calculated fields.
    #[must_use]
    pub fn offered(&self, basis: &Basis<'_>) -> Vec<(Field, Definition)> {
        let mut fields = basis.fields_of(self.lists);
        for calculated in &self.calculated {
            if let Some(definition) = calculated.definition() {
                fields.push((calculated.field(), definition));
            }
        }
        fields
    }

    /// What the calculated field at `at` may name in its formula: the category's fields and the
    /// calculated fields before it, each by [`reference_name`].
    pub(crate) fn names_before(&self, basis: &Basis<'_>, at: usize) -> Names {
        let mut names: Names = basis
            .fields_of(self.lists)
            .into_iter()
            .map(|(field, definition)| {
                (reference_name(definition.name()), field, definition.kind())
            })
            .collect();
        for calculated in self.calculated.iter().take(at) {
            names.push((
                reference_name(calculated.name()),
                calculated.field(),
                calculated.kind(),
            ));
        }
        names
    }

    fn check_calculated(&self, basis: &Basis<'_>) -> Result<(), ScheduleError> {
        for (at, calculated) in self.calculated.iter().enumerate() {
            let taken = |why: &str| ScheduleError::Calculated {
                name: calculated.name().to_owned(),
                why: why.to_owned(),
            };
            let spelling = reference_name(calculated.name());
            let others = self.names_before(basis, at);
            let same_name = |other: &str| other.to_lowercase() == calculated.name().to_lowercase();
            if basis
                .fields_of(self.lists)
                .iter()
                .any(|(_, definition)| same_name(definition.name()))
                || self
                    .calculated
                    .iter()
                    .enumerate()
                    .any(|(before, other)| before != at && same_name(other.name()))
            {
                return Err(taken("another field already has that name."));
            }
            if spelling.is_empty() || others.iter().any(|(spelt, _, _)| *spelt == spelling) {
                return Err(taken(
                    "a formula would read its name as another field's: choose a name with letters or digits that no field has.",
                ));
            }
            calc::check(calculated, &others)?;
        }
        Ok(())
    }

    fn check_totals(&self, basis: &Basis<'_>) -> Result<(), ScheduleError> {
        for (at, totals) in self.totals.iter().enumerate() {
            let definition = self.require(basis, &totals.field)?;
            if !self.fields.contains(&totals.field) {
                return Err(ScheduleError::TotalNotListed(definition.name().to_owned()));
            }
            if self.totals.iter().take(at).any(|b| b.field == totals.field) {
                return Err(ScheduleError::DuplicateTotal(definition.name().to_owned()));
            }
            if totals.kinds.is_empty() {
                return Err(ScheduleError::NoTotals(definition.name().to_owned()));
            }
            for (n, kind) in totals.kinds.iter().enumerate() {
                if totals.kinds.iter().take(n).any(|b| b == kind) {
                    return Err(ScheduleError::DuplicateTotal(definition.name().to_owned()));
                }
                if !kind.fits(definition.kind()) {
                    return Err(ScheduleError::TotalNotFor {
                        field: definition.name().to_owned(),
                        total: kind.key().to_owned(),
                        holds: definition.kind(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Whether the schedule stands in a document: its category can be listed, its calculated
    /// fields have names and formulas that stand, every field, filter, sort key and total is one
    /// the schedule has, no field, sort key or total is repeated, every filter's value suits its
    /// field, and every total suits what its column holds.
    ///
    /// # Errors
    /// The first thing that does not stand, as a [`ScheduleError`].
    pub fn check(&self, basis: &Basis<'_>) -> Result<(), ScheduleError> {
        basis.listable(self.lists)?;
        self.check_calculated(basis)?;
        for (at, field) in self.fields.iter().enumerate() {
            self.require(basis, field)?;
            if self.fields.iter().take(at).any(|before| before == field) {
                return Err(ScheduleError::DuplicateField(field.key()));
            }
        }
        for filter in &self.filters {
            let definition = self.require(basis, &filter.field)?;
            filter.check(&definition)?;
        }
        for (at, sort) in self.sorting.iter().enumerate() {
            self.require(basis, &sort.field)?;
            if self
                .sorting
                .iter()
                .take(at)
                .any(|before| before.field == sort.field)
            {
                return Err(ScheduleError::DuplicateSort(sort.field.key()));
            }
        }
        self.check_totals(basis)
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
                header: sort.header,
                footer: sort.footer,
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
        // The entries a schedule that does none of this does not have: an old schedule is the
        // same bytes it always was.
        if !self.itemize {
            props.insert(ITEMIZE_KEY.to_owned(), "no".to_owned());
        }
        if self.grand {
            props.insert(GRAND_KEY.to_owned(), "yes".to_owned());
        }
        if !self.totals.is_empty() {
            let totals: Vec<StoredTotals> = self
                .totals
                .iter()
                .map(|totals| StoredTotals {
                    field: totals.field.key(),
                    kinds: totals.kinds.iter().map(|k| k.key().to_owned()).collect(),
                })
                .collect();
            props.insert(
                TOTALS_KEY.to_owned(),
                serde_json::to_string(&totals).unwrap_or_default(),
            );
        }
        if !self.calculated.is_empty() {
            let calculated: Vec<StoredCalculated> = self
                .calculated
                .iter()
                .map(|c| StoredCalculated {
                    name: c.name().to_owned(),
                    formula: c.formula().to_owned(),
                    kind: c.kind().tag().to_owned(),
                })
                .collect();
            props.insert(
                CALCULATED_KEY.to_owned(),
                serde_json::to_string(&calculated).unwrap_or_default(),
            );
        }
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
        let filters = record
            .props
            .get(FILTERS_KEY)
            .ok_or_else(|| malformed("holds no list of filters"))
            .and_then(|text| read_filters(text).map_err(|why| malformed(&why)))?;
        let sorting = record
            .props
            .get(SORTING_KEY)
            .ok_or_else(|| malformed("holds no sorting"))
            .and_then(|text| read_sorting(text).map_err(|why| malformed(&why)))?;
        let mut schedule = Self::new(record.id.clone(), &record.name, lists, staging)
            .map_err(|_| malformed("has no name"))?
            .with_fields(fields)
            .with_filters(filters)
            .with_sorting(sorting)
            .with_itemize(
                record
                    .props
                    .get(ITEMIZE_KEY)
                    .is_none_or(|text| text != "no"),
            )
            .with_grand_totals(
                record
                    .props
                    .get(GRAND_KEY)
                    .is_some_and(|text| text == "yes"),
            );
        if let Some(text) = record.props.get(TOTALS_KEY) {
            schedule = schedule.with_totals(read_totals(text).map_err(|why| malformed(&why))?);
        }
        if let Some(text) = record.props.get(CALCULATED_KEY) {
            schedule =
                schedule.with_calculated(read_calculated(text).map_err(|why| malformed(&why))?);
        }
        if schedule.to_record(&record.document) != *record {
            return Err(malformed("is not as a schedule record is written"));
        }
        Ok(schedule)
    }
}

/// The totals a record holds, or why they do not read, as the end of a sentence about the record.
fn read_totals(text: &str) -> Result<Vec<Totals>, String> {
    let stored: Vec<StoredTotals> =
        serde_json::from_str(text).map_err(|_| "holds totals that do not read".to_owned())?;
    let mut totals = Vec::new();
    for one in stored {
        let field = Field::parse(&one.field)
            .map_err(|e| format!("holds totals on a field that does not read ({e})"))?;
        let kinds = one
            .kinds
            .iter()
            .map(|key| TotalKind::parse(key))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("holds a total that does not read ({e})"))?;
        totals.push(Totals { field, kinds });
    }
    Ok(totals)
}

/// The calculated fields a record holds, or why they do not read.
fn read_calculated(text: &str) -> Result<Vec<Calculated>, String> {
    let stored: Vec<StoredCalculated> = serde_json::from_str(text)
        .map_err(|_| "holds calculated fields that do not read".to_owned())?;
    let mut calculated = Vec::new();
    for one in stored {
        let kind = Kind::from_tag(&one.kind)
            .ok_or_else(|| "holds a calculated field of no known kind".to_owned())?;
        calculated.push(
            Calculated::new(&one.name, &one.formula, kind)
                .map_err(|e| format!("holds a calculated field that does not read ({e})"))?,
        );
    }
    Ok(calculated)
}

/// The filters a record holds, or why they do not read.
fn read_filters(text: &str) -> Result<Vec<Filter>, String> {
    let stored: Vec<StoredFilter> =
        serde_json::from_str(text).map_err(|_| "holds no list of filters".to_owned())?;
    let mut filters = Vec::new();
    for one in stored {
        let field = Field::parse(&one.field)
            .map_err(|e| format!("holds a filter on a field that does not read ({e})"))?;
        let comparison = Comparison::parse(&one.is)
            .map_err(|e| format!("holds a filter that does not read ({e})"))?;
        let value = Value::from_stored(&one.to)
            .filter(|value| value.stored() == one.to)
            .ok_or_else(|| "holds a filter value in no one spelling".to_owned())?;
        filters.push(Filter {
            field,
            comparison,
            value,
        });
    }
    Ok(filters)
}

/// The sorting a record holds, or why it does not read.
fn read_sorting(text: &str) -> Result<Vec<Sort>, String> {
    let stored: Vec<StoredSort> =
        serde_json::from_str(text).map_err(|_| "holds no sorting".to_owned())?;
    let mut sorting = Vec::new();
    for one in stored {
        let field = Field::parse(&one.field)
            .map_err(|e| format!("holds a sort on a field that does not read ({e})"))?;
        let descending = match one.order.as_str() {
            "ascending" => false,
            "descending" => true,
            _ => return Err("holds a sort that is neither ascending nor descending".to_owned()),
        };
        sorting.push(Sort {
            field,
            descending,
            header: one.header,
            footer: one.footer,
        });
    }
    Ok(sorting)
}
