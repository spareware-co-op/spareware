//! The commands that make and change a document's schedules (CON-A10-F1-P1, P2 and P4), and the
//! read of a schedule's table. They go through the same edit log as `element.*`, with the same
//! access check (`model.edit`) and the same claims: a piece another person is working on is
//! refused, and one person's edits land in log order.
//!
//! A schedule is an element record of the schedules category with no shape
//! ([`construct_schedules::Schedule`]). Each command reads the document's categories, parameters
//! and schedules, makes the change with construct-schedules' own types, checks the result with
//! [`construct_schedules::Schedule::check`] and the name rule of
//! [`construct_schedules::Schedules`], and writes the one record it changed. A refusal is the
//! command's result, in construct-schedules' words, and changes nothing. Nothing here reads a
//! clock or a random number.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `schedule.create` | `schedule` (the new ID), `name`, `category` (its number); optional `fields`, `filters`, `sorting`, `staging` | A schedule record |
//! | `schedule.rename` | `schedule`, `name` | The schedule. A name is unique among the document's schedules, case aside |
//! | `schedule.fields` | `schedule`, `fields` | The schedule's fields, in the order given (the whole list) |
//! | `schedule.filters` | `schedule`, `filters` | The schedule's filters (the whole list; `[]` clears) |
//! | `schedule.sorting` | `schedule`, `sorting` | The schedule's sorting (the whole list; `[]` clears) |
//! | `schedule.itemize` | `schedule`, `itemize` (yes or no) | The schedule: every instance a row, or equal rows folded into one with a count |
//! | `schedule.totals` | `schedule`, and `totals` (the whole list; `[]` clears) and/or `grand` (yes or no) | The schedule's totals per column and its grand total |
//! | `schedule.calculated` | `schedule`, `calculated` (the whole list of `{name, formula, kind}`) | The schedule's calculated fields |
//! | `schedule.cell` | `schedule`, `element`, `field`, `value` (typed text) | The element: the value goes back to it by the element's own command, so it is one step to undo and the element is claimed. Refused in words for a field that is read-only |
//! | `schedule.place` | `placement` (the new ID), `schedule`, `sheet`; optional `x`, `y` (the top left corner on the paper) | A placement record: the schedule stands on the sheet |
//! | `schedule.move` | `placement`, `x`, `y` | The placement |
//! | `schedule.unplace` | `placement` | Its pointer goes |
//! | `schedule.delete` | `schedule` | Its pointer goes, and the pointers of its placements |
//!
//! The values: a field is `name`, `type`, `parameter:<id>`, `quantity:<name>` or `calculated:<name>` (construct-schedules'
//! FORMAT.md). A filter is `{"field", "is", "to"}`: `is` is one of `equals`, `not_equals`, `less`,
//! `less_or_equal`, `greater`, `greater_or_equal`, `contains`, `begins_with`, and `to` is typed text
//! read with construct-params' metric display settings as the field's own kind reads it (`3.2 m`,
//! `12 m2`, `2`, `oak`). A sort key is `{"field", "order"}`, `order` `ascending` (the default) or
//! `descending`; with `"header": true` and/or `"footer": true` the key also groups (a header above and a footer below each group of equal values, nested in the order of the keys). `totals` is a list of `{"field", "do"}`, `do` a list of `sum`, `count`, `min`, `max`; a calculated field is `{"name", "formula", "kind"}` (a formula in the project's formula language over the other fields, `kind` one of `length`, `area`, `volume`, `angle`, `number`, `whole`) and is a field `calculated:<name>`. A schedule's category never changes: a new schedule is made.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a schedule
//! and `element.set`, `element.move` or `element.delete` of one are refused, naming the schedule
//! command to use. A schedule has no shape: nothing builds it
//! (`construct_regen::parameters::has_shape`).
//!
//! Interim: the phases a schedule's `staging` may name are the two every project starts with
//! ([`crate::cloud::views::phases`]), as for views; the job that stores phases through the log
//! replaces them in one place.

use crate::cloud::elements::write_record;
use crate::cloud::views;
use construct_categories::{CategoryId, SCHEDULES, SHEETS, VIEWS};
use construct_levels::parse_length;
use construct_model::element::Element;
use construct_model::geometry::Recipe;
use construct_params::{Kind, Length, Units, Value};
use construct_phases::ElementStaging;
use construct_regen::parameters::{self as records, Context};
use construct_schedules::{
    Basis, Calculated, Comparison, Field, Filter, Item, PLACEMENT_KIND, Placements, QUANTITIES,
    Reading, SCHEDULE_KIND, Schedule, SchedulePlacement, Schedules, Sort, Subject, Table,
    TotalKind, Totals, draw, reference_name, table,
};
use construct_types::ElementType;
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 13] = [
    "schedule.create",
    "schedule.rename",
    "schedule.fields",
    "schedule.filters",
    "schedule.sorting",
    "schedule.itemize",
    "schedule.totals",
    "schedule.calculated",
    "schedule.cell",
    "schedule.place",
    "schedule.move",
    "schedule.unplace",
    "schedule.delete",
];

/// The answer to a command that would make or change a schedule as an element.
pub const BY_SCHEDULE_COMMANDS: &str = "Schedules are made and changed by the schedule commands.";

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// A filter as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilterArgs {
    field: String,
    is: String,
    to: String,
}

/// A sort key as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SortArgs {
    field: String,
    order: Option<String>,
    header: Option<bool>,
    footer: Option<bool>,
}

/// The totals of one column as a person sends them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TotalsArgs {
    field: String,
    #[serde(rename = "do")]
    kinds: Vec<String>,
}

/// A calculated field as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CalculatedArgs {
    name: String,
    formula: String,
    kind: String,
}

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    schedule: Option<Id>,
    name: Option<String>,
    category: Option<u32>,
    fields: Option<Vec<String>>,
    filters: Option<Vec<FilterArgs>>,
    sorting: Option<Vec<SortArgs>>,
    staging: Option<ElementStaging>,
    itemize: Option<bool>,
    grand: Option<bool>,
    totals: Option<Vec<TotalsArgs>>,
    calculated: Option<Vec<CalculatedArgs>>,
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    field: Option<String>,
    value: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    placement: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    sheet: Option<Id>,
    x: Option<String>,
    y: Option<String>,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

fn need<T>(value: Option<T>, what: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("This command needs {what}."))
}

/// A document as a schedule's rules read it: its categories, its parameters and its schedules.
pub struct World {
    context: Context,
    schedules: Schedules,
    placements: Placements,
    records: Vec<Element>,
}

impl World {
    /// What the schedules' fields are read against.
    #[must_use]
    pub fn basis(&self) -> Basis<'_> {
        Basis {
            categories: &self.context.categories,
            params: &self.context.table,
        }
    }

    /// The document's schedules.
    #[must_use]
    pub fn schedules(&self) -> &Schedules {
        &self.schedules
    }

    /// The schedules placed on sheets.
    #[must_use]
    pub fn placements(&self) -> &Placements {
        &self.placements
    }

    fn check(&self, schedule: &Schedule) -> Result<(), String> {
        schedule.check(&self.basis()).map_err(|e| e.to_string())
    }
}

/// A document's categories, parameters and schedules as they are now. The inner error is a record
/// that is not as written, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_world<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<World, String>, StoreError> {
    let context = match records::read_context(reader, project, document).await? {
        Ok(context) => context,
        Err(why) => return Ok(Err(why)),
    };
    let all: Vec<Element> = records::read_document(reader, project, document)
        .await?
        .into_values()
        .map(|(_, record)| record)
        .collect();
    let schedules = match Schedules::from_records(&all) {
        Ok(schedules) => schedules,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let placements = match Placements::from_records(&all) {
        Ok(placements) => placements,
        Err(e) => return Ok(Err(e.to_string())),
    };
    Ok(Ok(World {
        context,
        schedules,
        placements,
        records: all,
    }))
}

/// A schedule as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ScheduleSummary {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The category it lists, by number.
    pub category: u32,
    /// That category's name.
    pub category_name: String,
    /// Its fields, by key, in order.
    pub fields: Vec<String>,
    /// Its filters, each as `{field, is, to}` with `to` in its stored spelling.
    pub filters: Vec<FilterView>,
    /// Its sorting, each as `{field, order, header, footer}`.
    pub sorting: Vec<SortView>,
    /// Whether every instance has a row; when not, equal rows are folded into one with a count.
    pub itemize: bool,
    /// Whether the table ends with a grand total.
    pub grand_totals: bool,
    /// The totals of its columns, each as `{field, do}`.
    pub totals: Vec<TotalsView>,
    /// Its calculated fields, each as `{name, formula, kind}`.
    pub calculated: Vec<CalculatedView>,
    /// The sheets it stands on, each with its corner on the paper.
    pub placements: Vec<PlacementView>,
    /// Why it does not stand in the document now (a parameter taken off the category), in words.
    pub problem: Option<String>,
}

/// A schedule placed on a sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlacementView {
    /// The placement's ID.
    pub id: String,
    /// The sheet's ID.
    pub sheet: String,
    /// The corner's distance across the paper, a stored length.
    pub x: String,
    /// The corner's distance up the paper, a stored length.
    pub y: String,
}

/// A filter as it is stored and shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FilterView {
    /// The field's key.
    pub field: String,
    /// The comparison's key.
    pub is: String,
    /// The value in its stored spelling.
    pub to: String,
}

/// A sort key as it is stored and shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SortView {
    /// The field's key.
    pub field: String,
    /// `ascending` or `descending`.
    pub order: &'static str,
    /// Whether each group of this key has a header.
    pub header: bool,
    /// Whether each group of this key has a footer.
    pub footer: bool,
}

/// The totals of one column as stored and shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TotalsView {
    /// The field's key.
    pub field: String,
    /// The totals, by key: `sum`, `count`, `min`, `max`.
    #[serde(rename = "do")]
    pub kinds: Vec<&'static str>,
}

/// A calculated field as stored and shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CalculatedView {
    /// Its name.
    pub name: String,
    /// Its formula, in the formula language's one spelling.
    pub formula: String,
    /// What it gives, as construct-params tags a kind.
    pub kind: &'static str,
}

/// One column of a schedule's table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ColumnView {
    /// The field's key.
    pub field: String,
    /// Its name.
    pub name: String,
    /// The kind of value it holds, as construct-params tags it; none when the category lacks it.
    pub kind: Option<&'static str>,
    /// Whether a value typed in one of its cells goes back to the element (`schedule.cell`); a
    /// read-only field (a quantity, a calculated field, the type's) is shown, not edited.
    pub editable: bool,
}

/// One cell of a row: the exact value in its stored spelling and the same shown with its unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CellView {
    /// The exact value, stored spelling; none when the element has no value there.
    pub value: Option<String>,
    /// The value shown in the document's units.
    pub shown: Option<String>,
}

/// One row of a schedule's table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RowView {
    /// The element's ID.
    pub element: String,
    /// One cell for each column.
    pub cells: Vec<CellView>,
}

/// A field the schedule's category has, which the schedule may list, filter or sort by.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OfferedView {
    /// The field's key.
    pub field: String,
    /// Its name.
    pub name: String,
    /// The kind of value it holds, as construct-params tags it.
    pub kind: &'static str,
    /// The comparisons a filter on it may use, by key, in the order they are listed to a person.
    pub comparisons: Vec<&'static str>,
    /// The name a formula of a calculated field uses for it; none for a field a formula cannot
    /// use (text, a choice...).
    pub formula_name: Option<String>,
}

/// A category a schedule may list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ListableView {
    /// Its number.
    pub id: u32,
    /// Its name.
    pub name: String,
}

/// A schedule with its table, worked out from the document as it is now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ScheduleTable {
    /// The schedule.
    pub schedule: ScheduleSummary,
    /// Every field its category has and every calculated field of its own, in a fixed order: what
    /// the page offers to add, filter, sort and total by.
    pub offered: Vec<OfferedView>,
    /// Its columns, in the order of its fields.
    pub columns: Vec<ColumnView>,
    /// One row for each element it lists, filtered and sorted, whatever the grouping and the
    /// itemizing (the rows the table's items fold and group).
    pub rows: Vec<RowView>,
    /// The table: group headers, rows (folded, with a count, when instances are not itemized) and
    /// group footers, in order.
    pub items: Vec<ItemView>,
    /// The grand total, when the schedule has one.
    pub grand: Option<FooterView>,
}

/// One item of a schedule's table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ItemView {
    /// A group begins.
    Header {
        /// How deep the group is: 0 for the first grouping key.
        level: usize,
        /// The key's field.
        field: String,
        /// The field's name.
        name: String,
        /// The group's value shown; none for the elements that have no value.
        shown: Option<String>,
    },
    /// A row, or the rows of several elements folded into one.
    Row {
        /// The first element the row stands for.
        element: String,
        /// Every element it stands for, in order.
        elements: Vec<String>,
        /// How many.
        count: usize,
        /// One cell for each column.
        cells: Vec<CellView>,
    },
    /// A group ends.
    Footer(FooterView),
}

/// A group's footer or the grand total.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FooterView {
    /// The group's depth; none for the grand total.
    pub level: Option<usize>,
    /// The key's field name; empty for the grand total.
    pub name: String,
    /// The group's value shown.
    pub shown: Option<String>,
    /// How many elements the group holds.
    pub elements: usize,
    /// The totals of the columns that have them.
    pub totals: Vec<TotalView>,
}

/// One total of one column.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TotalView {
    /// The column, by its place among the fields.
    pub column: usize,
    /// `sum`, `count`, `min` or `max`.
    pub total: &'static str,
    /// The exact value in its stored spelling; none when no element has a value.
    pub value: Option<String>,
    /// The value shown in the document's units.
    pub shown: Option<String>,
    /// Why it could not be worked out.
    pub problem: Option<String>,
}

fn footer_view(footer: &construct_schedules::Footer) -> FooterView {
    FooterView {
        level: footer.level,
        name: footer.name.clone(),
        shown: footer.shown.clone(),
        elements: footer.elements,
        totals: footer
            .totals
            .iter()
            .map(|total| TotalView {
                column: total.column,
                total: total.kind.key(),
                value: total.value.as_ref().map(Value::stored),
                shown: total.shown.clone(),
                problem: total.problem.clone(),
            })
            .collect(),
    }
}

fn cell_view(cell: construct_schedules::Cell) -> CellView {
    CellView {
        value: cell.value.map(|value| value.stored()),
        shown: cell.shown,
    }
}

/// The table's items and grand total as a page reads them.
fn items_of(made: Table) -> (Vec<ItemView>, Option<FooterView>) {
    let grand = made.grand.as_ref().map(footer_view);
    let items = made
        .items
        .into_iter()
        .map(|item| match item {
            Item::Header(header) => ItemView::Header {
                level: header.level,
                field: header.field.key(),
                name: header.name,
                shown: header.shown,
            },
            Item::Row(row) => ItemView::Row {
                element: row.element.to_string(),
                elements: row.elements.iter().map(ToString::to_string).collect(),
                count: row.count,
                cells: row.cells.into_iter().map(cell_view).collect(),
            },
            Item::Footer(footer) => ItemView::Footer(footer_view(&footer)),
        })
        .collect();
    (items, grand)
}

/// The summary of one schedule.
#[must_use]
pub fn summary(schedule: &Schedule, world: &World) -> ScheduleSummary {
    let basis = world.basis();
    ScheduleSummary {
        id: schedule.id().to_string(),
        name: schedule.name().to_owned(),
        category: schedule.lists().0,
        category_name: basis.category_name(schedule.lists()),
        fields: schedule.fields().iter().map(Field::key).collect(),
        filters: schedule
            .filters()
            .iter()
            .map(|filter| FilterView {
                field: filter.field.key(),
                is: filter.comparison.key().to_owned(),
                to: filter.value.stored(),
            })
            .collect(),
        sorting: schedule
            .sorting()
            .iter()
            .map(|sort| SortView {
                field: sort.field.key(),
                order: if sort.descending {
                    "descending"
                } else {
                    "ascending"
                },
                header: sort.header,
                footer: sort.footer,
            })
            .collect(),
        itemize: schedule.itemize(),
        grand_totals: schedule.grand_totals(),
        totals: schedule
            .totals()
            .iter()
            .map(|totals| TotalsView {
                field: totals.field.key(),
                kinds: totals.kinds.iter().map(|kind| kind.key()).collect(),
            })
            .collect(),
        calculated: schedule
            .calculated()
            .iter()
            .map(|c| CalculatedView {
                name: c.name().to_owned(),
                formula: c.formula().to_owned(),
                kind: c.kind().tag(),
            })
            .collect(),
        placements: world
            .placements
            .of_schedule(schedule.id())
            .into_iter()
            .map(|placed| PlacementView {
                id: placed.id().to_string(),
                sheet: placed.sheet().to_string(),
                x: stored(placed.x()),
                y: stored(placed.y()),
            })
            .collect(),
        problem: schedule.check(&basis).err().map(|e| e.to_string()),
    }
}

/// Every schedule of the world, by name (case aside), then by ID.
#[must_use]
pub fn summaries(world: &World) -> Vec<ScheduleSummary> {
    world
        .schedules
        .ordered()
        .into_iter()
        .map(|schedule| summary(schedule, world))
        .collect()
}

/// The top-level categories a schedule may list, in the category table's order: what a person
/// chooses from when making one.
#[must_use]
pub fn listable(world: &World) -> Vec<ListableView> {
    let basis = world.basis();
    basis
        .categories
        .all()
        .into_iter()
        .filter(|category| category.parent.is_none() && basis.listable(category.id).is_ok())
        .map(|category| ListableView {
            id: category.id.0,
            name: category.name,
        })
        .collect()
}

/// The facts a schedule reads of every element with a category: its name, its type's name, its
/// parameter values and the read-only quantities the product works out of it (a floor's and a
/// roof's from their types and outlines, a wall's from its line, layers, openings and joints, a
/// room's from its loop), each exact and shown in the
/// document's units. Worked out from the records as they are, so the same call after an edit
/// gives the new facts.
///
/// An element whose stored values cannot be read gives a subject with only its name and type: a
/// schedule is not stopped by one bad record.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn subjects<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    world: &World,
) -> Result<Vec<Subject>, StoreError> {
    let context = &world.context;
    let mut types: BTreeMap<Id, Option<ElementType>> = BTreeMap::new();
    let mut found = Vec::new();
    for record in world.records.iter().filter(|r| records::has_shape(r)) {
        let Some(category) = Context::category_of_element(record) else {
            continue;
        };
        let ty = match records::type_of(record) {
            None => None,
            Some(type_id) => {
                if !types.contains_key(&type_id) {
                    let read =
                        records::read_type(reader, project, document, &type_id, context).await?;
                    types.insert(type_id.clone(), read.ok());
                }
                types.get(&type_id).cloned().flatten()
            }
        };
        let cuts = if construct_regen::floor_plan::is_floor(&record.recipe) {
            crate::cloud::openings::floor_cuts(reader, project, document, record).await?
        } else {
            Vec::new()
        };
        let mut readings = BTreeMap::new();
        // A wall's length, area and volume, worked out of the document as it stands now.
        let wall_rows = construct_regen::wall_measure::read_only_view(
            reader,
            project,
            document,
            record,
            &context.units,
        )
        .await?;
        let rows = context
            .view_cut(record, ty.as_ref(), &cuts)
            .map(|view| view.parameters)
            .unwrap_or_default();
        for row in rows.into_iter().chain(wall_rows) {
            let Some(value) = row.value.as_deref().and_then(Value::from_stored) else {
                continue;
            };
            let field = if row.from == construct_regen::slabs::READ_ONLY {
                if !QUANTITIES
                    .iter()
                    .any(|q| q.name == row.name && q.id == row.id)
                {
                    continue;
                }
                Field::Quantity(row.name)
            } else {
                match Id::parse(&row.id) {
                    Ok(id) => Field::Parameter(id),
                    Err(_) => continue,
                }
            };
            readings.insert(
                field.key(),
                Reading {
                    value,
                    shown: row.shown.unwrap_or_default(),
                },
            );
        }
        if construct_regen::rooms::is_room(record) {
            room_readings(
                reader,
                project,
                document,
                record,
                &context.units,
                &mut readings,
            )
            .await?;
        }
        found.push(Subject {
            id: record.id.clone(),
            category,
            name: record.name.clone(),
            type_name: ty.as_ref().map(|ty| ty.name().to_owned()),
            readings,
        });
    }
    found.extend(sheet_and_view_subjects(world));
    Ok(found)
}

/// A reading of a quantity a sheet or a view has.
fn reading(readings: &mut BTreeMap<String, Reading>, name: &str, value: Value, shown: String) {
    readings.insert(
        Field::Quantity(name.to_owned()).key(),
        Reading { value, shown },
    );
}

/// The facts the sheet list and the view list read: every sheet and every view of the document,
/// each with the quantities construct-schedules names for it (a sheet's number, paper and how many
/// views and schedules stand on it; a view's kind, scale, detail level and the sheet it stands on),
/// and a sheet's parameters as its own record holds them. Records that do not read are left out:
/// a list is not stopped by one bad record, and the commands that read them say why.
fn sheet_and_view_subjects(world: &World) -> Vec<Subject> {
    let context = &world.context;
    let sheets = construct_sheets::Sheets::from_records(&world.records).unwrap_or_default();
    let views = construct_views::Views::from_records(&world.records).ok();
    let mut found = Vec::new();
    for sheet in sheets.ordered() {
        let mut readings = BTreeMap::new();
        let text = |text: &str| (Value::Text(text.to_owned()), text.to_owned());
        let (value, shown) = text(sheet.number());
        reading(&mut readings, "Number", value, shown);
        let (value, shown) = text(&sheet.paper().describe());
        reading(&mut readings, "Paper", value, shown);
        for (name, count) in [
            ("Views placed", sheets.viewports_of(sheet.id()).len()),
            (
                "Schedules placed",
                world.placements.of_sheet(sheet.id()).len(),
            ),
        ] {
            let count = i64::try_from(count).unwrap_or(i64::MAX);
            reading(&mut readings, name, Value::Whole(count), count.to_string());
        }
        // The sheet's own parameters (drawn by, checked by...), as its record holds them.
        let own = world
            .records
            .iter()
            .find(|record| &record.id == sheet.id())
            .and_then(|record| context.own_values(record, None).ok());
        if let Some(values) = own {
            for applicable in context
                .table
                .parameters_of(&context.categories, SHEETS)
                .into_iter()
                .filter(|applicable| applicable.holder == construct_params::Holder::Instance)
            {
                let Some(value) = values.get(applicable.definition.id()) else {
                    continue;
                };
                let shown = applicable
                    .definition
                    .format(value, &context.units)
                    .unwrap_or_default();
                readings.insert(
                    Field::Parameter(applicable.definition.id().clone()).key(),
                    Reading {
                        value: value.clone(),
                        shown,
                    },
                );
            }
        }
        found.push(Subject {
            id: sheet.id().clone(),
            category: SHEETS,
            name: sheet.name().to_owned(),
            type_name: None,
            readings,
        });
    }
    for view in views.iter().flat_map(construct_views::Views::ordered) {
        let mut readings = BTreeMap::new();
        let words = view.kind().key().replace('_', " ");
        reading(&mut readings, "Kind", Value::Text(words.clone()), words);
        let n = view.scale().denominator();
        reading(
            &mut readings,
            "Scale",
            Value::Whole(i64::from(n)),
            format!("1:{n}"),
        );
        let detail = view.detail().key().to_owned();
        reading(
            &mut readings,
            "Detail level",
            Value::Text(detail.clone()),
            detail,
        );
        if let Some((sheet, _)) = sheets.placement_of(view.id()) {
            let number = sheet.number().to_owned();
            reading(
                &mut readings,
                "On sheet",
                Value::Text(number.clone()),
                number,
            );
        }
        found.push(Subject {
            id: view.id().clone(),
            category: VIEWS,
            name: view.name().to_owned(),
            type_name: None,
            readings,
        });
    }
    found
}

/// A room's state, area, perimeter and volume, exact, from its loop as the document stands.
async fn room_readings<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    room: &Element,
    units: &Units,
    readings: &mut BTreeMap<String, Reading>,
) -> Result<(), StoreError> {
    let worked =
        construct_regen::rooms::worked_at(reader, project, document, room, records::At::Now)
            .await?;
    let Ok(worked) = worked else {
        return Ok(());
    };
    let view = construct_regen::rooms::RoomView::of(&worked, units);
    let mut put = |name: &str, value: Value, shown: String| {
        readings.insert(
            Field::Quantity(name.to_owned()).key(),
            Reading { value, shown },
        );
    };
    put(
        "State",
        Value::Text(view.state.to_owned()),
        view.state.to_owned(),
    );
    if worked.0.is_ok()
        && let Some(measures) = worked.1
    {
        put(
            "Area",
            Value::Area(measures.area),
            view.area.unwrap_or_default(),
        );
        put(
            "Perimeter",
            Value::Length(measures.perimeter),
            view.perimeter.unwrap_or_default(),
        );
        put(
            "Volume",
            Value::Volume(measures.volume),
            view.volume.unwrap_or_default(),
        );
    }
    Ok(())
}

/// The table a schedule makes of the document as it is now.
///
/// # Errors
/// A [`StoreError`] when the store fails.
async fn made_table<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    world: &World,
    schedule: &Schedule,
) -> Result<Table, StoreError> {
    let facts = subjects(reader, project, document, world).await?;
    Ok(table(
        schedule,
        &world.basis(),
        &world.context.units,
        &facts,
    ))
}

/// One schedule with its rows, as the document is now: `None` when the document has no such
/// schedule.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is a record that is not as written.
pub async fn read_table<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    id: &Id,
) -> Result<Result<Option<ScheduleTable>, String>, StoreError> {
    let world = match read_world(reader, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    let Some(schedule) = world.schedules.get(id) else {
        return Ok(Ok(None));
    };
    let made = made_table(reader, project, document, &world, schedule).await?;
    let basis = world.basis();
    let rows = made
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Row(row) => Some(row),
            _ => None,
        })
        .flat_map(|row| {
            // A folded row stands for several elements: the flat rows give each its own.
            row.elements.iter().map(|element| RowView {
                element: element.to_string(),
                cells: row.cells.iter().cloned().map(cell_view).collect(),
            })
        })
        .collect();
    let columns = made
        .columns
        .iter()
        .map(|column| ColumnView {
            field: column.field.key(),
            name: column.name.clone(),
            kind: column.kind.map(Kind::tag),
            editable: schedule.edit_target(&basis, &column.field).is_ok(),
        })
        .collect();
    let (items, grand) = items_of(made);
    Ok(Ok(Some(ScheduleTable {
        schedule: summary(schedule, &world),
        offered: schedule
            .offered(&basis)
            .into_iter()
            .map(|(field, definition)| OfferedView {
                field: field.key(),
                name: definition.name().to_owned(),
                kind: definition.kind().tag(),
                comparisons: Comparison::ALL
                    .into_iter()
                    .filter(|comparison| comparison.fits(definition.kind()))
                    .map(Comparison::key)
                    .collect(),
                formula_name: matches!(
                    definition.kind(),
                    Kind::Length
                        | Kind::Area
                        | Kind::Volume
                        | Kind::Angle
                        | Kind::Slope
                        | Kind::Number
                        | Kind::Whole
                )
                .then(|| reference_name(definition.name())),
            })
            .collect(),
        columns,
        rows,
        items,
        grand,
    })))
}

/// The staging a new schedule is made with: the one asked for, else created in the new phase, in
/// no design option.
fn staging_of(args: &Args) -> Result<ElementStaging, String> {
    if let Some(staging) = &args.staging {
        return Ok(staging.clone());
    }
    let created = Id::parse(views::NEW_PHASE).map_err(|e| e.to_string())?;
    ElementStaging::new(&views::phases()?, &views::options(), created, None, None)
        .map_err(|e| e.to_string())
}

fn field_of(key: &str) -> Result<Field, String> {
    Field::parse(key).map_err(|e| e.to_string())
}

fn fields_of(keys: &[String]) -> Result<Vec<Field>, String> {
    keys.iter().map(|key| field_of(key)).collect()
}

/// Filters from typed text: each value is read as its field's own kind reads typed text, so a
/// number is compared with a number and a length with a length. A filter may be on a calculated
/// field of the schedule, which must already be set on it.
fn filters_of(
    given: &[FilterArgs],
    schedule: &Schedule,
    basis: &Basis<'_>,
) -> Result<Vec<Filter>, String> {
    let units = Units::default();
    let mut filters = Vec::new();
    for one in given {
        let field = field_of(&one.field)?;
        let comparison = Comparison::parse(&one.is).map_err(|e| e.to_string())?;
        let definition = schedule.require(basis, &field).map_err(|e| e.to_string())?;
        let value = definition.parse(&one.to, &units).map_err(|e| {
            format!(
                "The filter on {} cannot compare it with {:?}: {e}",
                definition.name(),
                one.to
            )
        })?;
        filters.push(Filter {
            field,
            comparison,
            value,
        });
    }
    Ok(filters)
}

fn sorting_of(given: &[SortArgs]) -> Result<Vec<Sort>, String> {
    given
        .iter()
        .map(|one| {
            let descending = match one.order.as_deref() {
                None | Some("ascending") => false,
                Some("descending") => true,
                Some(other) => {
                    return Err(
                        construct_schedules::ScheduleError::UnknownOrder(other.to_owned())
                            .to_string(),
                    );
                }
            };
            Ok(Sort {
                field: field_of(&one.field)?,
                descending,
                header: one.header.unwrap_or(false),
                footer: one.footer.unwrap_or(false),
            })
        })
        .collect()
}

fn totals_of(given: &[TotalsArgs]) -> Result<Vec<Totals>, String> {
    given
        .iter()
        .map(|one| {
            Ok(Totals {
                field: field_of(&one.field)?,
                kinds: one
                    .kinds
                    .iter()
                    .map(|key| TotalKind::parse(key).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect()
}

fn calculated_of(given: &[CalculatedArgs]) -> Result<Vec<Calculated>, String> {
    given
        .iter()
        .map(|one| {
            let kind = Kind::from_tag(&one.kind).ok_or_else(|| {
                format!(
                    "The calculated field {}: {:?} is not a kind: length, area, volume, angle, number or whole.",
                    one.name, one.kind
                )
            })?;
            Calculated::new(&one.name, &one.formula, kind).map_err(|e| e.to_string())
        })
        .collect()
}

/// What a command is run against: the store, where, who, and when the orderer says it is.
struct Run<'a, T: Txn> {
    txn: &'a mut T,
    project: &'a Id,
    document: &'a Id,
    edit: &'a Edit,
    now_ms: u64,
    claim_ms: u64,
}

impl<T: Txn> Run<'_, T> {
    /// The claim this person takes on a piece, or who else holds it.
    async fn claim(&mut self, piece: &Id) -> Result<Result<Claim, String>, StoreError> {
        let holder = claims::held_by_another(
            self.txn,
            self.project,
            self.document,
            piece,
            &self.edit.person,
            self.now_ms,
        )
        .await?;
        if let Some(claim) = holder {
            return Ok(Err(format!("{} is working on this.", claim.name)));
        }
        let person: Option<Named> =
            read_row(self.txn, &Place::Person(self.edit.person.clone()).key()).await?;
        let name = person.map_or_else(|| self.edit.person.to_string(), |person| person.name);
        Ok(Ok(Claim {
            person: self.edit.person.clone(),
            name,
            // milliseconds since 1970 plus a claim's length: far below u64::MAX
            until_ms: self.now_ms.saturating_add(self.claim_ms),
        }))
    }

    /// The document's current record of a piece, and its hash.
    async fn current(&mut self, id: &Id) -> Result<Option<(Id, Element)>, StoreError> {
        records::read_element(self.txn, self.project, self.document, id).await
    }

    /// The document's categories, parameters and schedules as they are now.
    async fn world(&mut self) -> Result<Result<World, String>, StoreError> {
        read_world(self.txn, self.project, self.document).await
    }

    /// The schedule of this ID as it is now, with its record's hash.
    async fn schedule_of(&mut self, id: &Id) -> Result<Result<(Id, Schedule), String>, StoreError> {
        Ok(match self.current(id).await? {
            Some((hash, record)) if record.kind == SCHEDULE_KIND => Schedule::from_record(&record)
                .map(|schedule| (hash, schedule))
                .map_err(|e| e.to_string()),
            _ => Err(construct_schedules::ScheduleError::UnknownSchedule(id.clone()).to_string()),
        })
    }

    /// The placement of this ID as it is now, with its record's hash.
    async fn placement_of(
        &mut self,
        id: &Id,
    ) -> Result<Result<(Id, SchedulePlacement), String>, StoreError> {
        Ok(match self.current(id).await? {
            Some((hash, record)) if record.kind == PLACEMENT_KIND => {
                SchedulePlacement::from_record(&record)
                    .map(|placement| (hash, placement))
                    .map_err(|e| e.to_string())
            }
            _ => Err(construct_schedules::ScheduleError::UnknownPlacement(id.clone()).to_string()),
        })
    }

    /// Writes one record whose piece this person claims, as the command's whole result.
    async fn put_claimed(&mut self, record: &Element, before: Option<Id>) -> Answer {
        let claim = match self.claim(&record.id).await? {
            Ok(claim) => claim,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let hash = write_record(self.txn, self.project, record).await?;
        let pointer = Place::Piece {
            project: self.project.clone(),
            document: self.document.clone(),
            piece: record.id.clone(),
        };
        self.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        let claim = claims::take(self.txn, self.project, self.document, &record.id, claim).await?;
        Ok(Ok(Applied {
            changes: vec![Change {
                piece: record.id.clone(),
                before,
                after: Some(hash),
            }],
            claims: vec![claim],
            ..Applied::default()
        }))
    }

    /// Takes pieces away (each with the hash it had) as one command's result, this person's claim
    /// taken on every one first: another person's claim on any of them refuses it all.
    async fn remove_claimed(&mut self, pieces: Vec<(Id, Id)>) -> Answer {
        for (piece, _) in &pieces {
            if let Err(refusal) = self.claim(piece).await? {
                return Ok(Err(refusal));
            }
        }
        let mut changes = Vec::with_capacity(pieces.len());
        let mut released = Vec::with_capacity(pieces.len());
        for (piece, hash) in pieces {
            let pointer = Place::Piece {
                project: self.project.clone(),
                document: self.document.clone(),
                piece: piece.clone(),
            };
            self.txn.delete(pointer.key()).await?;
            released.push(claims::release(self.txn, self.project, self.document, &piece).await?);
            changes.push(Change {
                piece,
                before: Some(hash),
                after: None,
            });
        }
        Ok(Ok(Applied {
            changes,
            claims: released,
            ..Applied::default()
        }))
    }
}

/// Runs one of these commands.
///
/// # Errors
///
/// A store error; a command that cannot be made is the outcome's error instead.
pub async fn apply<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Ordered<'_>,
    edit: &Edit,
    claim_ms: u64,
) -> Result<Outcome, StoreError> {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    let mut run = Run {
        txn,
        project,
        document: &edit.target,
        edit,
        now_ms: context.now_ms,
        claim_ms,
    };
    match edit.cmd.as_str() {
        "schedule.create" => create(&mut run, args).await,
        "schedule.rename" => change(&mut run, args, Part::Rename).await,
        "schedule.fields" => change(&mut run, args, Part::Fields).await,
        "schedule.filters" => change(&mut run, args, Part::Filters).await,
        "schedule.sorting" => change(&mut run, args, Part::Sorting).await,
        "schedule.itemize" => change(&mut run, args, Part::Itemize).await,
        "schedule.totals" => change(&mut run, args, Part::Totals).await,
        "schedule.calculated" => change(&mut run, args, Part::Calculated).await,
        "schedule.place" => place(&mut run, args).await,
        "schedule.move" => move_placement(&mut run, args).await,
        "schedule.unplace" => unplace(&mut run, args).await,
        "schedule.delete" => delete(&mut run, args).await,
        // A cell is written as the command of the element it goes back to: `cell_edit` turns it
        // into that command before anything runs, so it never reaches here.
        "schedule.cell" => Ok(Err(
            "A cell is edited through the element's own command.".to_owned()
        )),
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// Which part of a schedule a change command sets.
#[derive(Clone, Copy)]
enum Part {
    Rename,
    Fields,
    Filters,
    Sorting,
    Itemize,
    Totals,
    Calculated,
}

/// The schedule a `schedule.create` asks for, before it is checked against the document.
fn make(args: &Args, world: &World) -> Result<Schedule, String> {
    let id = need(args.schedule.clone(), "a schedule")?;
    let name = need(args.name.as_deref(), "a name")?;
    let category = CategoryId(need(args.category, "a category")?);
    let mut schedule =
        Schedule::new(id, name, category, staging_of(args)?).map_err(|e| e.to_string())?;
    // The calculated fields first: a filter may be on one.
    if let Some(given) = &args.calculated {
        schedule = schedule.with_calculated(calculated_of(given)?);
    }
    if let Some(keys) = &args.fields {
        schedule = schedule.with_fields(fields_of(keys)?);
    }
    if let Some(given) = &args.filters {
        let filters = filters_of(given, &schedule, &world.basis())?;
        schedule = schedule.with_filters(filters);
    }
    if let Some(given) = &args.sorting {
        schedule = schedule.with_sorting(sorting_of(given)?);
    }
    if let Some(given) = &args.totals {
        schedule = schedule.with_totals(totals_of(given)?);
    }
    if let Some(itemize) = args.itemize {
        schedule = schedule.with_itemize(itemize);
    }
    if let Some(grand) = args.grand {
        schedule = schedule.with_grand_totals(grand);
    }
    Ok(schedule)
}

async fn create<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let schedule = match make(&args, &world) {
        Ok(schedule) => schedule,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(schedule.id()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut all = world.schedules.clone();
    if let Err(e) = all.add(schedule.clone()) {
        return Ok(Err(e.to_string()));
    }
    if let Err(refusal) = world.check(&schedule) {
        return Ok(Err(refusal));
    }
    run.put_claimed(&schedule.to_record(run.document), None)
        .await
}

/// The schedule after a change command, or why it cannot be.
fn changed_by(
    what: Part,
    args: &Args,
    schedule: Schedule,
    world: &World,
) -> Result<Schedule, String> {
    if args.category.is_some() {
        return Err("A schedule's category is not changed: make another schedule.".to_owned());
    }
    match what {
        Part::Rename => {
            let name = need(args.name.as_deref(), "a name")?;
            schedule.renamed(name).map_err(|e| e.to_string())
        }
        Part::Fields => {
            let keys = need(args.fields.as_deref(), "fields")?;
            Ok(schedule.with_fields(fields_of(keys)?))
        }
        Part::Filters => {
            let given = need(args.filters.as_deref(), "filters")?;
            let filters = filters_of(given, &schedule, &world.basis())?;
            Ok(schedule.with_filters(filters))
        }
        Part::Sorting => {
            let given = need(args.sorting.as_deref(), "sorting")?;
            Ok(schedule.with_sorting(sorting_of(given)?))
        }
        Part::Itemize => {
            let itemize = need(args.itemize, "itemize, yes or no")?;
            Ok(schedule.with_itemize(itemize))
        }
        Part::Totals => {
            if args.totals.is_none() && args.grand.is_none() {
                return Err("This command needs totals, grand, or both.".to_owned());
            }
            let mut changed = schedule;
            if let Some(given) = &args.totals {
                changed = changed.with_totals(totals_of(given)?);
            }
            if let Some(grand) = args.grand {
                changed = changed.with_grand_totals(grand);
            }
            Ok(changed)
        }
        Part::Calculated => {
            let given = need(args.calculated.as_deref(), "calculated fields")?;
            Ok(schedule.with_calculated(calculated_of(given)?))
        }
    }
}

async fn change<T: Txn>(run: &mut Run<'_, T>, args: Args, what: Part) -> Answer {
    let id = match need(args.schedule.clone(), "a schedule") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (hash, old) = match run.schedule_of(&id).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = match changed_by(what, &args, old.clone(), &world) {
        Ok(changed) => changed,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if changed == old {
        // Nothing changed: no record is written and no step is made.
        return Ok(Ok(Applied::default()));
    }
    let mut all = world.schedules.clone();
    if let Err(e) = all.replace(changed.clone()) {
        return Ok(Err(e.to_string()));
    }
    if let Err(refusal) = world.check(&changed) {
        return Ok(Err(refusal));
    }
    run.put_claimed(&changed.to_record(run.document), Some(hash))
        .await
}

/// Deletes a schedule and, in the same step, every placement of it on a sheet.
async fn delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.schedule, "a schedule") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (hash, _) = match run.schedule_of(&id).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut pieces = Vec::new();
    for placed in world.placements.of_schedule(&id) {
        if let Some((placed_hash, _)) = run.current(placed.id()).await? {
            pieces.push((placed.id().clone(), placed_hash));
        }
    }
    pieces.push((id, hash));
    run.remove_claimed(pieces).await
}

/// The sheet a command is about, from the document's records.
fn sheet_of(world: &World, id: &Id) -> Result<construct_sheets::Sheet, String> {
    construct_sheets::Sheets::from_records(&world.records)
        .map_err(|e| e.to_string())?
        .sheet(id)
        .cloned()
        .ok_or_else(|| construct_sheets::SheetError::UnknownSheet(id.clone()).to_string())
}

/// A top left corner on the paper, or why it is not.
fn on_paper(x: Length, y: Length, sheet: &construct_sheets::Sheet) -> Result<(), String> {
    let (wide, tall) = (sheet.paper().width(), sheet.paper().height());
    let zero = Length::from_steps(0);
    if x < zero || y < zero || x > wide || y > tall {
        return Err(format!(
            "A schedule's top left corner is on the paper: across 0 to {wide}, up 0 to {tall}."
        ));
    }
    Ok(())
}

/// How far in from the paper's edges a schedule is first put: 10 mm.
const MARGIN: Length = Length::from_mm(10);

/// A typed length in the metric settings.
fn typed(text: &str) -> Result<Length, String> {
    parse_length(text, &Units::default()).map_err(|e| e.to_string())
}

async fn place<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let placed = (|| {
        let id = need(args.placement.clone(), "a placement")?;
        let schedule = need(args.schedule.clone(), "a schedule")?;
        let sheet_id = need(args.sheet.clone(), "a sheet")?;
        if world.schedules.get(&schedule).is_none() {
            return Err(construct_schedules::ScheduleError::UnknownSchedule(schedule).to_string());
        }
        let sheet = sheet_of(&world, &sheet_id)?;
        let x = args.x.as_deref().map(typed).transpose()?.unwrap_or(MARGIN);
        let y = match args.y.as_deref() {
            Some(text) => typed(text)?,
            None => Length::from_steps(
                sheet
                    .paper()
                    .height()
                    .steps()
                    .saturating_sub(MARGIN.steps()),
            ),
        };
        on_paper(x, y, &sheet)?;
        let placement = SchedulePlacement::new(id, schedule, sheet_id, x, y);
        let mut all = world.placements.clone();
        all.add(placement.clone()).map_err(|e| e.to_string())?;
        Ok(placement)
    })();
    let placement = match placed {
        Ok(placement) => placement,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(placement.id()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    run.put_claimed(&placement.to_record(run.document), None)
        .await
}

async fn move_placement<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.placement.clone(), "a placement") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (hash, old) = match run.placement_of(&id).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let moved = (|| {
        if args.x.is_none() && args.y.is_none() {
            return Err("This command needs x, y, or both.".to_owned());
        }
        let x = args.x.as_deref().map(typed).transpose()?.unwrap_or(old.x());
        let y = args.y.as_deref().map(typed).transpose()?.unwrap_or(old.y());
        on_paper(x, y, &sheet_of(&world, old.sheet())?)?;
        Ok(old.clone().moved(x, y))
    })();
    let moved = match moved {
        Ok(moved) => moved,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if moved == old {
        return Ok(Ok(Applied::default()));
    }
    run.put_claimed(&moved.to_record(run.document), Some(hash))
        .await
}

async fn unplace<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.placement.clone(), "a placement") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (hash, _) = match run.placement_of(&id).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.remove_claimed(vec![(id, hash)]).await
}

/// Whether a command is `schedule.cell`, which the project's commands turn into an element's own
/// command before they run it ([`cell_edit`]).
#[must_use]
pub fn is_cell(cmd: &str) -> bool {
    cmd == "schedule.cell"
}

/// The element's own command a `schedule.cell` stands for, or why the cell is not edited.
///
/// A cell is one element's value of one field of a schedule. The value goes back to the element
/// through the command a person would use on the element: its parameter by `element.param`, its
/// name by `element.set` (`sheet.set` for a sheet, `view.rename` for a view). So it is one step to
/// undo, the element is claimed like any edit, and the element's own rules (a wall's, a formula
/// that would fail) judge it. A field that is read-only is refused in words by
/// [`construct_schedules::Schedule::edit_target`], and an element the schedule does not list is
/// refused.
///
/// # Errors
/// A store error; a cell that cannot be edited is the inner error, in words.
pub async fn cell_edit<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
) -> Result<Result<Edit, String>, StoreError> {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    let (Some(schedule), Some(element), Some(key), Some(value)) =
        (args.schedule, args.element, args.field, args.value)
    else {
        return Ok(Err(
            "This command needs a schedule, an element, a field and a value.".to_owned(),
        ));
    };
    let world = match read_world(txn, project, &edit.target).await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some(listing) = world.schedules.get(&schedule) else {
        return Ok(Err(construct_schedules::ScheduleError::UnknownSchedule(
            schedule,
        )
        .to_string()));
    };
    let target = field_of(&key).and_then(|field| {
        listing
            .edit_target(&world.basis(), &field)
            .map_err(|e| e.to_string())
    });
    let target = match target {
        Ok(target) => target,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some(record) = world.records.iter().find(|record| record.id == element) else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    let listed = if matches!(record.kind.as_str(), "sheet") {
        Some(SHEETS)
    } else if record.kind == "view" {
        Some(VIEWS)
    } else {
        Context::category_of_element(record)
    };
    if !listed.is_some_and(|category| world.basis().lists(listing.lists(), category)) {
        return Ok(Err(
            construct_schedules::ScheduleError::NotListed(element).to_string()
        ));
    }
    let (cmd, args) = match target {
        construct_schedules::EditTarget::Name => match record.kind.as_str() {
            "sheet" => (
                "sheet.set",
                serde_json::json!({"sheet": element.to_string(), "name": value}),
            ),
            "view" => (
                "view.rename",
                serde_json::json!({"view": element.to_string(), "name": value}),
            ),
            _ => (
                "element.set",
                serde_json::json!({"element": element.to_string(), "name": value}),
            ),
        },
        construct_schedules::EditTarget::Parameter(definition) => (
            "element.param",
            serde_json::json!({
                "element": element.to_string(),
                "parameter": definition.id().to_string(),
                "value": value,
            }),
        ),
    };
    Ok(Ok(Edit {
        cmd: cmd.to_owned(),
        args,
        ..edit.clone()
    }))
}

/// The part of a generic command the rules below read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<Recipe>,
}

/// Whether a recipe is a schedule's.
fn a_schedule_recipe(recipe: &Recipe) -> bool {
    records::category_of(recipe).is_ok_and(|category| category == SCHEDULES)
}

/// The generic commands do not go behind the schedule commands: `element.create` of a schedule
/// or of a placed schedule and `element.set`, `element.move` or `element.delete` of one are
/// refused, naming the command to use. Anything else, and a command that does not read (or an
/// element that is not there), is left to the command, which answers it as it always has.
///
/// # Errors
///
/// A [`StoreError`] when the store fails.
pub async fn generic_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
) -> Result<Result<(), String>, StoreError> {
    let Ok(args) = serde_json::from_value::<GenericArgs>(edit.args.clone()) else {
        return Ok(Ok(()));
    };
    let by_recipe = args.recipe.as_ref().is_some_and(a_schedule_recipe);
    let use_instead = |what: &str, command: &str| format!("{what}: use {command}.");
    Ok(match edit.cmd.as_str() {
        "element.create" if args.kind.as_deref() == Some(PLACEMENT_KIND) => Err(use_instead(
            "A schedule is placed on a sheet",
            "schedule.place",
        )),
        "element.create" if by_recipe || args.kind.as_deref() == Some(SCHEDULE_KIND) => {
            Err(use_instead("A schedule is made", "schedule.create"))
        }
        "element.set" | "element.move" | "element.delete" => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, &edit.target, id).await? {
                Some((_, record)) if record.kind == SCHEDULE_KIND => Err(match edit.cmd.as_str() {
                    "element.delete" => use_instead("A schedule is deleted", "schedule.delete"),
                    _ => use_instead(
                        "A schedule is changed",
                        "schedule.rename, schedule.fields, schedule.filters, schedule.sorting, schedule.itemize, schedule.totals or schedule.calculated",
                    ),
                }),
                Some((_, record)) if record.kind == PLACEMENT_KIND => {
                    Err(match edit.cmd.as_str() {
                        "element.delete" => {
                            use_instead("A placed schedule is taken off", "schedule.unplace")
                        }
                        _ => use_instead("A placed schedule is moved", "schedule.move"),
                    })
                }
                Some(_) if by_recipe => Err(use_instead("A schedule is made", "schedule.create")),
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// Whether the schedules an undo wrote still stand in the document: each reads as a schedule and
/// its category, fields, filters and sorting are the document's now; and each placement it wrote
/// has its schedule and its sheet. A parameter taken off a category since is not the undo's doing:
/// only the schedules the undo wrote are held to the present fields.
///
/// # Errors
/// A store error; the inner error names the schedule and what it would lack.
pub async fn undo_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    written: &[Element],
) -> Result<Result<(), String>, StoreError> {
    let world = match read_world(txn, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    let checked = written
        .iter()
        .filter(|record| record.kind == SCHEDULE_KIND)
        .filter_map(|record| world.schedules.get(&record.id));
    for schedule in checked {
        if let Err(why) = world.check(schedule) {
            return Ok(Err(format!("{}: {why}", schedule.name())));
        }
    }
    let placed = written
        .iter()
        .filter(|record| record.kind == PLACEMENT_KIND)
        .filter_map(|record| world.placements.get(&record.id));
    for placement in placed {
        if world.schedules.get(placement.schedule()).is_none()
            || sheet_of(&world, placement.sheet()).is_err()
        {
            return Ok(Err(
                "A schedule placed on a sheet needs both the schedule and the sheet.".to_owned(),
            ));
        }
    }
    Ok(Ok(()))
}

/// One schedule placed on a sheet, drawn from its live table: where its top left corner is on the
/// paper, how big the table is, and the text and lines that make it, each at an exact place from
/// the corner, across and down.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlacedSchedule {
    /// The placement's ID.
    pub placement: String,
    /// The schedule's ID.
    pub schedule: String,
    /// The schedule's name.
    pub name: String,
    /// The corner's distance across the paper, a stored length.
    pub x: String,
    /// The corner's distance up the paper, a stored length.
    pub y: String,
    /// How wide the drawing is, a stored length.
    pub width: String,
    /// How tall the drawing is, a stored length.
    pub height: String,
    /// The text.
    pub texts: Vec<DrawnTextView>,
    /// The lines.
    pub rules: Vec<DrawnRuleView>,
    /// Why it is not drawn, in words (the schedule no longer stands in the document).
    pub problem: Option<String>,
}

/// A piece of drawn text, its place from the table's top left corner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DrawnTextView {
    /// Across, a stored length.
    pub x: String,
    /// Down to the top of its row, a stored length.
    pub y: String,
    /// The text.
    pub text: String,
    /// Whether it is set heavier.
    pub bold: bool,
}

/// A drawn line, its ends from the table's top left corner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DrawnRuleView {
    /// Where it starts across, a stored length.
    pub x1: String,
    /// Where it starts down.
    pub y1: String,
    /// Where it ends across.
    pub x2: String,
    /// Where it ends down.
    pub y2: String,
}

fn stored(length: Length) -> String {
    Value::Length(length).stored()
}

/// The schedules placed on a sheet, each drawn from its table as the document is now.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is a record that is not as written.
pub async fn placed_on<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    sheet: &Id,
) -> Result<Result<Vec<PlacedSchedule>, String>, StoreError> {
    let world = match read_world(reader, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    let mut shown = Vec::new();
    for placement in world.placements.of_sheet(sheet) {
        let Some(schedule) = world.schedules.get(placement.schedule()) else {
            continue;
        };
        let mut one = PlacedSchedule {
            placement: placement.id().to_string(),
            schedule: schedule.id().to_string(),
            name: schedule.name().to_owned(),
            x: stored(placement.x()),
            y: stored(placement.y()),
            width: stored(Length::from_steps(0)),
            height: stored(Length::from_steps(0)),
            texts: Vec::new(),
            rules: Vec::new(),
            problem: schedule.check(&world.basis()).err().map(|e| e.to_string()),
        };
        if one.problem.is_none() {
            let made = made_table(reader, project, document, &world, schedule).await?;
            let drawn = draw(&made, schedule.name());
            one.width = stored(drawn.width);
            one.height = stored(drawn.height);
            one.texts = drawn
                .texts
                .into_iter()
                .map(|t| DrawnTextView {
                    x: stored(t.x),
                    y: stored(t.y),
                    text: t.text,
                    bold: t.bold,
                })
                .collect();
            one.rules = drawn
                .rules
                .into_iter()
                .map(|r| DrawnRuleView {
                    x1: stored(r.x1),
                    y1: stored(r.y1),
                    x2: stored(r.x2),
                    y2: stored(r.y2),
                })
                .collect();
        }
        shown.push(one);
    }
    Ok(Ok(shown))
}
