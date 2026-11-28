//! The commands that make and change a document's schedules (CON-A10-F1-P1, first half), and the
//! read of a schedule's rows. They go through the same edit log as `element.*`, with the same
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
//! | `schedule.delete` | `schedule` | Its pointer goes |
//!
//! The values: a field is `name`, `type`, `parameter:<id>` or `quantity:<name>` (construct-schedules'
//! FORMAT.md). A filter is `{"field", "is", "to"}`: `is` is one of `equals`, `not_equals`, `less`,
//! `less_or_equal`, `greater`, `greater_or_equal`, `contains`, `begins_with`, and `to` is typed text
//! read with construct-params' metric display settings as the field's own kind reads it (`3.2 m`,
//! `12 m2`, `2`, `oak`). A sort key is `{"field", "order"}`, `order` `ascending` (the default) or
//! `descending`. A schedule's category never changes: a new schedule is made.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a schedule
//! and `element.set`, `element.move` or `element.delete` of one are refused, naming the schedule
//! command to use. A schedule has no shape: nothing builds it
//! (`construct_regen::parameters::has_shape`).
//!
//! Interim: the phases a schedule's `staging` may name are the two every project starts with
//! ([`crate::cloud::views::phases`]), as for views; the job that stores phases through the log
//! replaces them in one place.

use crate::cloud::Tools;
use crate::cloud::elements::ElementTool;
use crate::cloud::elements::write_record;
use crate::cloud::tool::Tool;
use crate::cloud::views;
use construct_categories::{CategoryId, SCHEDULES};
use construct_model::element::Element;
use construct_model::geometry::Recipe;
use construct_params::{Units, Value};
use construct_phases::ElementStaging;
use construct_regen::parameters::{self as records, Context};
use construct_schedules::{
    Basis, Comparison, Field, Filter, Quantity, Reading, SCHEDULE_KIND, Schedule, Schedules, Sort,
    Subject, columns_among, rows_among,
};
use construct_types::ElementType;
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 6] = [
    "schedule.create",
    "schedule.rename",
    "schedule.fields",
    "schedule.filters",
    "schedule.sorting",
    "schedule.delete",
];

/// The answer to a command that would make or change a schedule as an element.
pub const BY_SCHEDULE_COMMANDS: &str = "Schedules are made and changed by the schedule commands.";

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The schedule tool ([`Tool`]): schedules and their fields, filters and sorting.
pub struct ScheduleTool;

impl Tool for ScheduleTool {
    const NAME: &'static str = "schedule";
    const COMMANDS: &'static [&'static str] = &COMMANDS;
    const GUARDS: &'static [&'static str] = &[ElementTool::NAME];
    const UNDO_ORDER: u16 = 50;

    async fn apply<T: Txn>(
        txn: &mut T,
        project: &Id,
        context: &kit_sync::Context<'_>,
        edit: &Edit,
        claim_ms: u64,
    ) -> Result<Outcome, StoreError> {
        apply(txn, project, context, edit, claim_ms).await
    }

    async fn guard<T: Txn>(
        txn: &mut T,
        project: &Id,
        edit: &Edit,
    ) -> Result<Result<(), String>, StoreError> {
        generic_rules(txn, project, edit).await
    }

    async fn undo_rules<T: Txn>(
        txn: &mut T,
        project: &Id,
        document: &Id,
        written: &[Element],
        _deleted: &[Id],
        _whole: bool,
    ) -> Result<Result<(), String>, StoreError> {
        undo_rules(txn, project, document, written).await
    }
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

/// A document as a schedule's rules read it: its categories, its parameters, its schedules and
/// the quantities its tools give their elements.
pub struct World {
    context: Context,
    schedules: Schedules,
    records: Vec<Element>,
    /// Every tool's schedule fields ([`Tools::fields`]).
    fields: Vec<Quantity>,
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

    /// The quantities a schedule's fields are read against: every tool's.
    #[must_use]
    pub fn fields(&self) -> &[Quantity] {
        &self.fields
    }

    fn check(&self, schedule: &Schedule) -> Result<(), String> {
        schedule
            .check_among(&self.basis(), &self.fields)
            .map_err(|e| e.to_string())
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
    Ok(Ok(World {
        context,
        schedules,
        records: all,
        fields: Tools::fields(),
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
    /// Its sorting, each as `{field, order}`.
    pub sorting: Vec<SortView>,
    /// Why it does not stand in the document now (a parameter taken off the category), in words.
    pub problem: Option<String>,
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
}

/// A category a schedule may list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ListableView {
    /// Its number.
    pub id: u32,
    /// Its name.
    pub name: String,
}

/// A schedule with its rows, worked out from the document as it is now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ScheduleTable {
    /// The schedule.
    pub schedule: ScheduleSummary,
    /// Every field its category has, in a fixed order: what the page offers to add, filter and
    /// sort by.
    pub offered: Vec<OfferedView>,
    /// Its columns, in the order of its fields.
    pub columns: Vec<ColumnView>,
    /// Its rows, filtered and sorted.
    pub rows: Vec<RowView>,
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
            })
            .collect(),
        problem: schedule
            .check_among(&basis, world.fields())
            .err()
            .map(|e| e.to_string()),
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
                if !world
                    .fields()
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
    Ok(found)
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
    let facts = subjects(reader, project, document, &world).await?;
    let basis = world.basis();
    let table = ScheduleTable {
        schedule: summary(schedule, &world),
        offered: basis
            .fields_of_among(world.fields(), schedule.lists())
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
            })
            .collect(),
        columns: columns_among(schedule, &basis, world.fields())
            .into_iter()
            .map(|column| ColumnView {
                field: column.field.key(),
                name: column.name,
                kind: column.kind.map(construct_params::Kind::tag),
            })
            .collect(),
        rows: rows_among(schedule, &basis, world.fields(), &facts)
            .into_iter()
            .map(|row| RowView {
                element: row.element.to_string(),
                cells: row
                    .cells
                    .into_iter()
                    .map(|cell| CellView {
                        value: cell.value.map(|value| value.stored()),
                        shown: cell.shown,
                    })
                    .collect(),
            })
            .collect(),
    };
    Ok(Ok(Some(table)))
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
/// number is compared with a number and a length with a length.
fn filters_of(
    given: &[FilterArgs],
    category: CategoryId,
    basis: &Basis<'_>,
    fields: &[Quantity],
) -> Result<Vec<Filter>, String> {
    let units = Units::default();
    let mut filters = Vec::new();
    for one in given {
        let field = field_of(&one.field)?;
        let comparison = Comparison::parse(&one.is).map_err(|e| e.to_string())?;
        let definition = basis
            .require_among(fields, category, &field)
            .map_err(|e| e.to_string())?;
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
            })
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
        "schedule.delete" => delete(&mut run, args).await,
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
}

/// The schedule a `schedule.create` asks for, before it is checked against the document.
fn make(args: &Args, world: &World) -> Result<Schedule, String> {
    let id = need(args.schedule.clone(), "a schedule")?;
    let name = need(args.name.as_deref(), "a name")?;
    let category = CategoryId(need(args.category, "a category")?);
    let mut schedule =
        Schedule::new(id, name, category, staging_of(args)?).map_err(|e| e.to_string())?;
    if let Some(keys) = &args.fields {
        schedule = schedule.with_fields(fields_of(keys)?);
    }
    if let Some(given) = &args.filters {
        schedule =
            schedule.with_filters(filters_of(given, category, &world.basis(), world.fields())?);
    }
    if let Some(given) = &args.sorting {
        schedule = schedule.with_sorting(sorting_of(given)?);
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
            let filters = filters_of(given, schedule.lists(), &world.basis(), world.fields())?;
            Ok(schedule.with_filters(filters))
        }
        Part::Sorting => {
            let given = need(args.sorting.as_deref(), "sorting")?;
            Ok(schedule.with_sorting(sorting_of(given)?))
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

async fn delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.schedule, "a schedule") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (hash, _) = match run.schedule_of(&id).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = run.claim(&id).await? {
        return Ok(Err(refusal));
    }
    let pointer = Place::Piece {
        project: run.project.clone(),
        document: run.document.clone(),
        piece: id.clone(),
    };
    run.txn.delete(pointer.key()).await?;
    let released = claims::release(run.txn, run.project, run.document, &id).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: id,
            before: Some(hash),
            after: None,
        }],
        claims: vec![released],
        ..Applied::default()
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
/// and `element.set`, `element.move` or `element.delete` of one are refused, naming the command
/// to use. Anything else, and a command that does not read (or an element that is not there), is
/// left to the command, which answers it as it always has.
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
                        "schedule.rename, schedule.fields, schedule.filters or schedule.sorting",
                    ),
                }),
                Some(_) if by_recipe => Err(use_instead("A schedule is made", "schedule.create")),
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// Whether the schedules an undo wrote still stand in the document: each reads as a schedule and
/// its category, fields, filters and sorting are the document's now. A parameter taken off a category
/// since is not the undo's doing: only the schedules the undo wrote are held to the present fields.
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
    Ok(Ok(()))
}
