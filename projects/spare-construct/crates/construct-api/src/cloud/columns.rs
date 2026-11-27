//! The commands that make and change column types and columns (job 14.7, CON-A5-F1-P1). They go
//! through the same edit log as `element.*`, with the same access check (`model.edit`) and the
//! same claims: a piece another person is working on is refused, and one person's edits land in log
//! order. Each is one entry, so one undo takes back all it wrote ([`crate::cloud::undo`]), however
//! many columns it placed or changed. They read as the floor commands do ([`crate::cloud::floors`]).
//!
//! Each command checks the change with construct-columns' own rules (a type's section, a column's
//! height), construct-types' (types), construct-levels' (ties) and construct-grids' (crossings),
//! and a column with the very reader and plan the builders use
//! ([`construct_regen::levels::recipe_at`], [`construct_regen::columns::finished`] and
//! [`construct_regen::column_plan::ColumnPlan::of`]), so a column a command takes is one the
//! builders take. A refusal is the command's result, in the data crate's words after the column's
//! name, and changes nothing. Nothing here reads a clock or a random number: the claim's time is
//! the orderer's.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `column.type.create` | `type`, `name`, `section`, `material` | A column type: of the columns category, its section and material in its `@system.column` block |
//! | `column.type.section` | `type`; `section` and/or `material` | The column type: its section and/or its material |
//! | `column.type.rename` | `type`, `name` | The column type, as `type.rename` |
//! | `column.type.duplicate` | `type`, `new`, `name` | A copy of the column type, as `type.duplicate` |
//! | `column.type.delete` | `type` | Its pointer goes, as `type.delete`: refused while a column is of it |
//! | `column.defaults` | none (`{}`) | Each of construct-columns' default column types the document lacks (by ID, or by name among its column types); nothing when it lacks none |
//! | `column.place` | `element`, `name` (default the ID), `type`, `level`, `offset` (default `0`), the top: `top` (`{"level":<ID>,"offset":<text>}`) or `height` (typed text), `at` (`[x, y]`, typed text), `turn` (degrees, default `0`) | A new column at a point |
//! | `column.place.grids` | `type`, `level`, `offset`, the top and `turn` as `column.place`, `columns`: a list of `{"element","name","first","second","cross"}` (two grid IDs; `cross` the crossing, default `0`) | One new column at each of the crossings listed, in one step |
//! | `column.type` | `elements` (IDs), `type` | The columns: their type |
//! | `column.levels` | `elements`; any of `level`, `offset` (the base), `top` or `height` (the top) | The columns: their base and/or their top. `level` alone moves the base to that level at offset 0; `offset` alone changes the offset |
//! | `column.move` | `elements`, `dx`, `dy` (typed text) | The columns, each moved by that much from where it stands now. A column on grid lines is let go of them and stands at the point it was moved to |
//! | `column.turn` | `elements`, `turn` (degrees) | The columns: their turn about their centres |
//! | `column.delete` | `elements` | Their pointers go |
//!
//! `section` is `{"kind":"rect","width":<text>,"depth":<text>}` or `{"kind":"round","diameter":<text>}`;
//! lengths and angles are typed text, read with construct-params' metric units until a project
//! keeps its own: `400`, `0.4 m`, `12.5 mm`, `37.5`. A column's top is at a level plus an offset, or
//! an unconnected `height` above its base; a top at or below its base is refused by name
//! ([`construct_columns::ColumnError::TopBelowBase`]).
//!
//! `column.place`: the record is kind `column`, `level` 0, its type and category, its base tie, its
//! top tie when it has one, and the recipe `construct/column` (construct-columns' FORMAT.md).
//! Refused: an element of that ID, a type the document lacks or one that is not a column type, a
//! level not in the model, a size or a height construct-columns refuses, a point past the grid, and
//! anything the builders would refuse, in their words. `column.place.grids` refuses, naming the
//! column, two grids the model lacks or that do not cross (or cross fewer times than `cross`), and
//! nothing is written when any of its columns is refused. At most [`MOST_AT_ONCE`] columns are
//! placed or changed in one command.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a column,
//! `element.set` of a column's recipe, `element.move` of a column, `type.create` of a columns type
//! and `type.rename`, `type.duplicate` or `type.delete` of a column type are refused, naming the
//! command to use.
//!
//! For the page, without the log: [`read_columns`], a document's column types and columns with
//! what each stands on and reads back (the server's read-only view).

use crate::cloud::floors::{Answer, Run, length, need, units, why};
use crate::cloud::{levels, params};
use construct_categories::{COLUMNS, CategoryId};
use construct_columns::{
    Column, ColumnError, ColumnSpec, Position, Section, Top, column_of, column_record,
    default_column_types, is_column, new_column_type, spec_of, with_column, with_spec,
};
use construct_levels::{Length, Tie};
use construct_materials::MaterialId;
use construct_model::element::Element;
use construct_model::keys::Place as ModelPlace;
use construct_model::rows::{BuiltElement, BuiltResult};
use construct_params::{Angle, Definition, Holder, Kind, Units, Value};
use construct_regen::column_plan::ColumnPlan;
use construct_regen::columns as building;
use construct_regen::levels as placing;
use construct_regen::parameters::{self as records, At, Context, TYPE_KIND};
use construct_types::{ElementType, TypeError, Types};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row};
use kit_sync::{Context as Ordered, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeSet;

/// Every command here.
pub const COMMANDS: [&str; 13] = [
    "column.type.create",
    "column.type.section",
    "column.type.rename",
    "column.type.duplicate",
    "column.type.delete",
    "column.defaults",
    "column.place",
    "column.place.grids",
    "column.type",
    "column.levels",
    "column.move",
    "column.turn",
    "column.delete",
];

/// The most columns one command places or changes.
pub const MOST_AT_ONCE: usize = 2_000;

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// A column type's section as a person sends it.
#[derive(Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum SectionArgs {
    Rect { width: String, depth: String },
    Round { diameter: String },
}

/// A column's top as a person sends it: the level it reaches, and an offset from it.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct TopArgs {
    level: Json,
    offset: Option<String>,
}

/// One column of a `column.place.grids`: its ID, its name and the two grid lines it stands on.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct GridColumn {
    #[serde(with = "kit_ops::id")]
    element: Id,
    name: Option<String>,
    #[serde(with = "kit_ops::id")]
    first: Id,
    #[serde(with = "kit_ops::id")]
    second: Id,
    #[serde(default)]
    cross: usize,
}

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    name: Option<String>,
    #[serde(default, rename = "type", with = "kit_ops::id::option")]
    type_id: Option<Id>,
    section: Option<SectionArgs>,
    material: Option<String>,
    level: Option<Json>,
    offset: Option<String>,
    top: Option<TopArgs>,
    height: Option<String>,
    at: Option<[String; 2]>,
    turn: Option<String>,
    elements: Option<Vec<String>>,
    columns: Option<Vec<GridColumn>>,
    dx: Option<String>,
    dy: Option<String>,
}

/// Typed text as an angle, in degrees.
fn angle(text: &str) -> Result<Angle, String> {
    let id = Id::parse("turn").map_err(why)?;
    let definition = Definition::new(id, "Turn", Kind::Angle, Holder::Instance).map_err(why)?;
    match definition.parse(text, &Units::metric()).map_err(why)? {
        Value::Angle(angle) => Ok(angle),
        _ => Err("A turn is an angle.".to_owned()),
    }
}

/// A section from what a person sent, its sizes read as typed lengths and checked.
fn section(sent: &SectionArgs) -> Result<Section, String> {
    match sent {
        SectionArgs::Rect { width, depth } => {
            Section::rectangular(length(width)?, length(depth)?).map_err(why)
        }
        SectionArgs::Round { diameter } => Section::round(length(diameter)?).map_err(why),
    }
}

/// A column type of the document: a type of the columns category with a section in its block. The
/// inner error is why not, in construct-types' or construct-columns' words.
async fn column_type<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    context: &Context,
    id: &Id,
) -> Result<Result<ElementType, String>, StoreError> {
    let ty = match records::read_type(txn, project, document, id, context).await? {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(spec_of(&ty).map(|_| ty).map_err(why))
}

/// A new column type that lists every parameter bound to columns, as `type.create` makes one.
fn listed(ty: ElementType, context: &Context) -> Result<ElementType, String> {
    let mut ty = ty;
    for applicable in context.table.parameters_of(&context.categories, COLUMNS) {
        let parameter = applicable.definition.id();
        if !ty.has(parameter) {
            ty.add_parameter(&context.params, parameter).map_err(why)?;
        }
    }
    Ok(ty)
}

/// The IDs a command lists, each an ID, none twice, at least one and at most [`MOST_AT_ONCE`].
fn listed_ids(sent: Option<Vec<String>>) -> Result<Vec<Id>, String> {
    let sent = need(sent, "the columns (`elements`)")?;
    if sent.is_empty() {
        return Err("This command needs at least one column.".to_owned());
    }
    if sent.len() > MOST_AT_ONCE {
        return Err(format!(
            "This command takes at most {MOST_AT_ONCE} columns, and {} were sent.",
            sent.len()
        ));
    }
    let mut seen = BTreeSet::new();
    let mut ids = Vec::with_capacity(sent.len());
    for text in sent {
        let id = Id::parse(&text).map_err(|_| format!("{text:?} is not an element's ID."))?;
        if !seen.insert(id.clone()) {
            return Err(format!("{id} is listed twice."));
        }
        ids.push(id);
    }
    Ok(ids)
}

impl<T: Txn> Run<'_, T> {
    /// What the builders would make of `record`, read the way they read it: the refusal, after
    /// its name, when they would refuse it.
    async fn column_buildable(
        &mut self,
        record: &Element,
    ) -> Result<Result<(), String>, StoreError> {
        let filled =
            placing::recipe_at(self.txn, self.project, self.document, record, At::Now).await?;
        let refused = |failed: construct_model::geometry::GeometryError| {
            format!("{}: {}", record.name, failed.message)
        };
        let filled = match filled {
            Ok(filled) => filled,
            Err(failed) => return Ok(Err(refused(failed))),
        };
        let done =
            building::finished(self.txn, self.project, self.document, filled, At::Now).await?;
        Ok(done
            .and_then(|done| ColumnPlan::of(&done).map(|_| ()))
            .map_err(refused))
    }

    /// The column a record is, or the refusal that it is not one.
    async fn the_column(&mut self, id: &Id) -> Result<Result<Element, String>, StoreError> {
        Ok(match self.current(id).await? {
            None => Err(format!("{id} no longer exists.")),
            Some((_, record)) if !is_column(&record) => {
                Err(format!("{} is not a column.", record.name))
            }
            Some((_, record)) => Ok(record),
        })
    }

    /// Writes `made` (new columns) and `changed` (columns that were there), each changed piece
    /// claimed first so another person's claim turns it all back, as the command's whole result.
    async fn put_columns(&mut self, made: &[Element], changed: &[Element]) -> Answer {
        let mut held: Vec<(Id, Claim)> = Vec::with_capacity(changed.len());
        for record in changed {
            match self.claim(&record.id).await? {
                Ok(claim) => held.push((record.id.clone(), claim)),
                Err(refusal) => return Ok(Err(refusal)),
            }
        }
        let mut written: Vec<Change> = Vec::with_capacity(made.len().saturating_add(changed.len()));
        for record in made.iter().chain(changed) {
            written.push(self.put(record).await?);
        }
        let mut taken = Vec::with_capacity(held.len());
        for (piece, claim) in held {
            taken.push(
                kit_sync::claims::take(self.txn, self.project, self.document, &piece, claim)
                    .await?,
            );
        }
        Ok(Ok(kit_sync::Applied {
            changes: written,
            claims: taken,
            ..kit_sync::Applied::default()
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
    let document = match records::read_context(txn, project, &edit.target).await? {
        Ok(document) => document,
        Err(refusal) => return Ok(Err(refusal)),
    };
    // Renaming, copying and deleting a column type are the type commands' own, once it is known to
    // be a column type: the same rules, the same words.
    if let Some(as_type) = edit.cmd.strip_prefix("column.")
        && matches!(as_type, "type.rename" | "type.duplicate" | "type.delete")
    {
        let id = match need(args.type_id, "a column type") {
            Ok(id) => id,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if let Err(refusal) = column_type(txn, project, &edit.target, &document, &id).await? {
            return Ok(Err(refusal));
        }
        let mut by_type = edit.clone();
        as_type.clone_into(&mut by_type.cmd);
        return params::apply(txn, project, context, &by_type, claim_ms).await;
    }
    let mut run = Run {
        txn,
        project,
        document: &edit.target,
        edit,
        now_ms: context.now_ms,
        claim_ms,
    };
    match edit.cmd.as_str() {
        "column.type.create" => create_type(&mut run, &document, args).await,
        "column.type.section" => set_section(&mut run, &document, args).await,
        "column.defaults" => defaults(&mut run, &document).await,
        "column.place" => place(&mut run, &document, args).await,
        "column.place.grids" => place_grids(&mut run, &document, args).await,
        "column.type" => set_type(&mut run, &document, args).await,
        "column.levels" => set_levels(&mut run, args).await,
        "column.move" => move_columns(&mut run, args).await,
        "column.turn" => turn_columns(&mut run, args).await,
        "column.delete" => delete_columns(&mut run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// A type's section and material from what a person sent.
fn spec_from(
    section_sent: Option<&SectionArgs>,
    material: Option<&str>,
) -> Result<ColumnSpec, String> {
    let section_sent = need(section_sent, "the section")?;
    let material = need(material, "the material")?;
    Ok(ColumnSpec {
        section: section(section_sent)?,
        material: MaterialId::parse(material).map_err(|e| why(ColumnError::from(e)))?,
    })
}

/// Every type of the document, read against `context`, as construct-types keeps them.
async fn types_of<T: Txn>(
    run: &mut Run<'_, T>,
    context: &Context,
) -> Result<Result<Types, String>, StoreError> {
    run.types(context).await
}

async fn create_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = (|| {
        let id = need(args.type_id, "a column type")?;
        let name = need(args.name, "a name")?;
        let spec = spec_from(args.section.as_ref(), args.material.as_deref())?;
        Ok::<_, String>((id, name, spec))
    })();
    let (id, name, spec) = match sent {
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let made = new_column_type(id, &name, &spec, &context.categories)
        .map_err(why)
        .and_then(|ty| listed(ty, context));
    let ty = match made {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut types = match types_of(run, context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(e) = types.add(ty.clone()) {
        return Ok(Err(why(e)));
    }
    run.put_types(&[ty]).await
}

async fn set_section<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let id = match need(args.type_id, "a column type") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if args.section.is_none() && args.material.is_none() {
        return Ok(Err("This command needs a section or a material.".to_owned()));
    }
    let ty = match column_type(run.txn, run.project, run.document, context, &id).await? {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = (|| {
        let old = spec_of(&ty).map_err(why)?;
        let section = match &args.section {
            Some(sent) => section(sent)?,
            None => old.section,
        };
        let material = match &args.material {
            Some(text) => MaterialId::parse(text).map_err(|e| why(ColumnError::from(e)))?,
            None => old.material,
        };
        with_spec(ty, &ColumnSpec { section, material }).map_err(why)
    })();
    let changed = match changed {
        Ok(changed) => changed,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.put_claimed(&changed.to_record(run.document)).await
}

/// Each default column type the document lacks: none of its ID is in the document and no column
/// type of the document has its name. None lacking writes nothing, which is no step.
async fn defaults<T: Txn>(run: &mut Run<'_, T>, context: &Context) -> Answer {
    let defaults = match default_column_types(&context.categories) {
        Ok(defaults) => defaults,
        Err(e) => return Ok(Err(why(e))),
    };
    let mut types = match types_of(run, context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut made = Vec::new();
    for ty in defaults {
        let named = types
            .in_category(COLUMNS)
            .iter()
            .any(|other| other.name() == ty.name());
        if named || run.current(ty.id()).await?.is_some() {
            continue;
        }
        let ty = match listed(ty, context) {
            Ok(ty) => ty,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if let Err(e) = types.add(ty.clone()) {
            return Ok(Err(why(e)));
        }
        made.push(ty);
    }
    run.put_types(&made).await
}

/// The type of a new column: in the document and a column type.
async fn the_type<T: Txn>(
    run: &mut Run<'_, T>,
    context: &Context,
    type_id: Option<Id>,
) -> Result<Result<Id, String>, StoreError> {
    let type_id = match need(type_id, "a column type") {
        Ok(type_id) => type_id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match records::read_type(run.txn, run.project, run.document, &type_id, context).await?
    {
        Ok(ty) if ty.category() != COLUMNS => {
            let refusal = TypeError::OtherCategory {
                from: COLUMNS,
                to: ty.category(),
            };
            return Ok(Err(why(refusal)));
        }
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(spec_of(&ty).map(|_| type_id).map_err(why))
}

/// A level and an offset typed as a person sends them: the level's ID and the offset text.
async fn tie_of<T: Txn>(
    run: &mut Run<'_, T>,
    level: Option<&Json>,
    offset: Option<&str>,
    what: &str,
) -> Result<Result<Tie, String>, StoreError> {
    let base = match levels::tie_of_new(run.txn, run.project, run.document, level).await? {
        Ok(Some(base)) => base,
        Ok(None) => return Ok(Err(format!("This command needs {what}."))),
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(match offset {
        None => Ok(base),
        Some(text) => length(text).map(|offset| Tie { offset, ..base }),
    })
}

/// How a new column's top is given: a level and an offset, or an unconnected height.
async fn top_of_new<T: Txn>(
    run: &mut Run<'_, T>,
    top: Option<&TopArgs>,
    height: Option<&str>,
) -> Result<Result<Top, String>, StoreError> {
    match (top, height) {
        (Some(_), Some(_)) => Ok(Err(
            "A column's top is a level or a height, not both.".to_owned()
        )),
        (None, None) => Ok(Err(
            "This command needs the column's top: a level (`top`) or a `height`.".to_owned(),
        )),
        (Some(top), None) => Ok(
            match tie_of(run, Some(&top.level), top.offset.as_deref(), "a top level").await? {
                Ok(tie) => Ok(Top::Level(tie)),
                Err(refusal) => Err(refusal),
            },
        ),
        (None, Some(text)) => Ok(length(text).map(Top::Height)),
    }
}

/// A new column's shared settings: its type, base tie, top and turn.
struct Shared {
    type_id: Id,
    base: Tie,
    top: Top,
    turn: Angle,
}

async fn shared<T: Txn>(
    run: &mut Run<'_, T>,
    context: &Context,
    args: &Args,
) -> Result<Result<Shared, String>, StoreError> {
    let type_id = match the_type(run, context, args.type_id.clone()).await? {
        Ok(type_id) => type_id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let base = match tie_of(
        run,
        args.level.as_ref(),
        args.offset.as_deref(),
        "a base level",
    )
    .await?
    {
        Ok(base) => base,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let top = match top_of_new(run, args.top.as_ref(), args.height.as_deref()).await? {
        Ok(top) => top,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let turn = match args.turn.as_deref().map(angle).transpose() {
        Ok(turn) => turn.unwrap_or(Angle::from_units(0)),
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(Ok(Shared {
        type_id,
        base,
        top,
        turn,
    }))
}

async fn place<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let id = match need(args.element.clone(), "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let name = args.name.clone().unwrap_or_else(|| id.to_string());
    let shared = match shared(run, context, &args).await? {
        Ok(shared) => shared,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let at = match need(args.at.as_ref(), "where it stands (`at`)").and_then(|at| {
        Ok(Position::Point {
            x: length(&at[0])?,
            y: length(&at[1])?,
        })
    }) {
        Ok(at) => at,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let record = column_record(
        &id,
        run.document,
        &name,
        &shared.type_id,
        &shared.base,
        &Column {
            position: at,
            turn: shared.turn,
            top: shared.top,
        },
    );
    if let Err(refusal) = run.column_buildable(&record).await? {
        return Ok(Err(refusal));
    }
    run.put_columns(&[record], &[]).await
}

async fn place_grids<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = match need(args.columns.clone(), "the columns (`columns`)") {
        Ok(sent) if sent.is_empty() => {
            return Ok(Err("This command needs at least one column.".to_owned()));
        }
        Ok(sent) if sent.len() > MOST_AT_ONCE => {
            return Ok(Err(format!(
                "This command takes at most {MOST_AT_ONCE} columns, and {} were sent.",
                sent.len()
            )));
        }
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let shared = match shared(run, context, &args).await? {
        Ok(shared) => shared,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut made: Vec<Element> = Vec::with_capacity(sent.len());
    let mut seen = BTreeSet::new();
    for one in sent {
        if !seen.insert(one.element.clone()) {
            return Ok(Err(format!("{} is listed twice.", one.element)));
        }
        if run.current(&one.element).await?.is_some() {
            return Ok(Err(format!("{} already exists.", one.element)));
        }
        let name = one.name.clone().unwrap_or_else(|| one.element.to_string());
        let record = column_record(
            &one.element,
            run.document,
            &name,
            &shared.type_id,
            &shared.base,
            &Column {
                position: Position::Grids {
                    first: one.first,
                    second: one.second,
                    cross: one.cross,
                },
                turn: shared.turn,
                top: shared.top.clone(),
            },
        );
        if let Err(refusal) = run.column_buildable(&record).await? {
            return Ok(Err(refusal));
        }
        made.push(record);
    }
    run.put_columns(&made, &[]).await
}

/// Every column of `ids`, read as it stands now, or the first refusal.
async fn the_columns<T: Txn>(
    run: &mut Run<'_, T>,
    ids: &[Id],
) -> Result<Result<Vec<Element>, String>, StoreError> {
    let mut found = Vec::with_capacity(ids.len());
    for id in ids {
        match run.the_column(id).await? {
            Ok(record) => found.push(record),
            Err(refusal) => return Ok(Err(refusal)),
        }
    }
    Ok(Ok(found))
}

/// Checks each of `changed` as the builders would, then writes them as one step.
async fn put_checked<T: Txn>(run: &mut Run<'_, T>, changed: Vec<Element>) -> Answer {
    for record in &changed {
        if let Err(refusal) = run.column_buildable(record).await? {
            return Ok(Err(refusal));
        }
    }
    run.put_columns(&[], &changed).await
}

async fn set_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let ids = match listed_ids(args.elements.clone()) {
        Ok(ids) => ids,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let type_id = match the_type(run, context, args.type_id.clone()).await? {
        Ok(type_id) => type_id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let columns = match the_columns(run, &ids).await? {
        Ok(columns) => columns,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = columns
        .into_iter()
        .map(|mut record| {
            record
                .props
                .insert(construct_types::TYPE_KEY.to_owned(), type_id.to_string());
            record
        })
        .collect();
    put_checked(run, changed).await
}

async fn set_levels<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let ids = match listed_ids(args.elements.clone()) {
        Ok(ids) => ids,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let has_base = args.level.is_some() || args.offset.is_some();
    let has_top = args.top.is_some() || args.height.is_some();
    if !has_base && !has_top {
        return Ok(Err(
            "This command needs a base (`level`, `offset`) or a top (`top`, `height`).".to_owned(),
        ));
    }
    let top = if has_top {
        match top_of_new(run, args.top.as_ref(), args.height.as_deref()).await? {
            Ok(top) => Some(top),
            Err(refusal) => return Ok(Err(refusal)),
        }
    } else {
        None
    };
    let new_base = if args.level.is_some() {
        match tie_of(
            run,
            args.level.as_ref(),
            args.offset.as_deref(),
            "a base level",
        )
        .await?
        {
            Ok(base) => Some(base),
            Err(refusal) => return Ok(Err(refusal)),
        }
    } else {
        None
    };
    let offset_only = match (&args.level, &args.offset) {
        (None, Some(text)) => match length(text) {
            Ok(offset) => Some(offset),
            Err(refusal) => return Ok(Err(refusal)),
        },
        _ => None,
    };
    let columns = match the_columns(run, &ids).await? {
        Ok(columns) => columns,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut changed = Vec::with_capacity(columns.len());
    for record in columns {
        let mut record = record;
        let mut column = match column_of(&record) {
            Ok(column) => column,
            Err(e) => return Ok(Err(format!("{}: {e}", record.name))),
        };
        if let Some(base) = &new_base {
            construct_levels::write_base(&mut record.props, Some(base));
        } else if let Some(offset) = offset_only {
            match construct_levels::base_of(&record) {
                Ok(Some(base)) => {
                    construct_levels::write_base(&mut record.props, Some(&Tie { offset, ..base }));
                }
                Ok(None) => {
                    return Ok(Err(format!("{} stands on no level.", record.name)));
                }
                Err(e) => return Ok(Err(format!("{}: {e}", record.name))),
            }
        }
        if let Some(top) = &top {
            column.top = top.clone();
        }
        match with_column(record, &column) {
            Ok(record) => changed.push(record),
            Err(e) => return Ok(Err(why(e))),
        }
    }
    put_checked(run, changed).await
}

/// Where a column stands now, as a point in grid steps: its own point, or where its grids cross.
async fn stands_at<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
    column: &Column,
) -> Result<Result<[i64; 2], String>, StoreError> {
    if let Position::Point { x, y } = &column.position {
        return Ok(Ok([x.steps(), y.steps()]));
    }
    let mut found = Vec::new();
    for id in column.position.grids() {
        match records::read_element(run.txn, run.project, run.document, id).await? {
            Some((_, grid)) => found.push(grid),
            None => {
                return Ok(Err(format!(
                    "{}: it stands on grid {id}, which is not in this model.",
                    record.name
                )));
            }
        }
    }
    let datums = match construct_grids::Datums::from_records(&found) {
        Ok(datums) => datums,
        Err(e) => return Ok(Err(format!("{}: {e}", record.name))),
    };
    Ok(column
        .position
        .resolve(&datums)
        .map_err(|e| format!("{}: {e}", record.name)))
}

async fn move_columns<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let ids = match listed_ids(args.elements.clone()) {
        Ok(ids) => ids,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let by = (|| {
        let dx = length(&need(args.dx.clone(), "dx")?)?;
        let dy = length(&need(args.dy.clone(), "dy")?)?;
        Ok::<_, String>((dx, dy))
    })();
    let (dx, dy) = match by {
        Ok(by) => by,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let columns = match the_columns(run, &ids).await? {
        Ok(columns) => columns,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut changed = Vec::with_capacity(columns.len());
    for record in columns {
        let mut column = match column_of(&record) {
            Ok(column) => column,
            Err(e) => return Ok(Err(format!("{}: {e}", record.name))),
        };
        let [x, y] = match stands_at(run, &record, &column).await? {
            Ok(at) => at,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let moved = |from: i64, by: Length| {
            from.checked_add(by.steps())
                .map(Length::from_steps)
                .ok_or_else(|| format!("{}: {}", record.name, ColumnError::OutOfRange))
        };
        column.position = match (moved(x, dx), moved(y, dy)) {
            (Ok(x), Ok(y)) => Position::Point { x, y },
            (Err(refusal), _) | (_, Err(refusal)) => return Ok(Err(refusal)),
        };
        match with_column(record, &column) {
            Ok(record) => changed.push(record),
            Err(e) => return Ok(Err(why(e))),
        }
    }
    put_checked(run, changed).await
}

async fn turn_columns<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let ids = match listed_ids(args.elements.clone()) {
        Ok(ids) => ids,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let turn = match need(args.turn.as_deref(), "a turn").and_then(angle) {
        Ok(turn) => turn,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let columns = match the_columns(run, &ids).await? {
        Ok(columns) => columns,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut changed = Vec::with_capacity(columns.len());
    for record in columns {
        let mut column = match column_of(&record) {
            Ok(column) => column,
            Err(e) => return Ok(Err(format!("{}: {e}", record.name))),
        };
        column.turn = turn;
        match with_column(record, &column) {
            Ok(record) => changed.push(record),
            Err(e) => return Ok(Err(why(e))),
        }
    }
    put_checked(run, changed).await
}

async fn delete_columns<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let ids = match listed_ids(args.elements.clone()) {
        Ok(ids) => ids,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let columns = match the_columns(run, &ids).await? {
        Ok(columns) => columns,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut held = Vec::with_capacity(columns.len());
    for record in &columns {
        match run.claim(&record.id).await? {
            Ok(claim) => held.push((record.id.clone(), claim)),
            Err(refusal) => return Ok(Err(refusal)),
        }
    }
    let mut changes = Vec::with_capacity(columns.len());
    for record in &columns {
        let before = run.current(&record.id).await?.map(|(hash, _)| hash);
        let pointer = Place::Piece {
            project: run.project.clone(),
            document: run.document.clone(),
            piece: record.id.clone(),
        };
        run.txn.delete(pointer.key()).await?;
        changes.push(Change {
            piece: record.id.clone(),
            before,
            after: None,
        });
    }
    let mut taken = Vec::with_capacity(held.len());
    for (piece, claim) in held {
        taken
            .push(kit_sync::claims::take(run.txn, run.project, run.document, &piece, claim).await?);
    }
    Ok(Ok(kit_sync::Applied {
        changes,
        claims: taken,
        ..kit_sync::Applied::default()
    }))
}

/// The words for a generic command that should have been a column command.
fn use_instead(what: &str, command: &str) -> String {
    format!("{what} with {command}, which checks it as a column.")
}

/// The part of a generic command's arguments these rules read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<construct_model::geometry::Recipe>,
    #[serde(default, rename = "type", with = "kit_ops::id::option")]
    type_id: Option<Id>,
    category: Option<CategoryId>,
}

/// The column rules of the generic commands, asked before one runs: a column is made by
/// `column.place`, not `element.create`; its recipe is changed by the column commands, not by
/// `element.set`; it is moved by `column.move`, not `element.move`; a column type is made by
/// `column.type.create`, not `type.create`, and renamed, copied and deleted by
/// `column.type.rename`, `column.type.duplicate` and `column.type.delete`. The inner error is the
/// refusal, naming the command to use. An edit these rules have nothing to say of is left to the
/// command, which answers it as it always has.
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
    let document = &edit.target;
    let a_column_recipe = args
        .recipe
        .as_ref()
        .is_some_and(construct_columns::is_column_recipe);
    Ok(match edit.cmd.as_str() {
        "element.create"
            if a_column_recipe || args.kind.as_deref() == Some(construct_columns::COLUMN_KIND) =>
        {
            Err(use_instead("A column is made", "column.place"))
        }
        "element.set" | "element.move" | "element.delete"
            if edit.cmd != "element.set" || args.recipe.is_some() =>
        {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record)) if is_column(&record) && edit.cmd == "element.move" => Err(
                    use_instead(&format!("{} is moved", record.name), "column.move"),
                ),
                Some((_, record)) if is_column(&record) && edit.cmd == "element.set" => {
                    Err(use_instead(
                        &format!("{} is changed", record.name),
                        "the column commands",
                    ))
                }
                Some(_) if a_column_recipe => Err(use_instead("A column is made", "column.place")),
                _ => Ok(()),
            }
        }
        "type.create" if args.category == Some(COLUMNS) => {
            Err(use_instead("A column type is made", "column.type.create"))
        }
        cmd @ ("type.rename" | "type.duplicate" | "type.delete") => {
            let Some(id) = &args.type_id else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record))
                    if record.kind == TYPE_KIND
                        && records::stored_category(&record) == Some(COLUMNS) =>
                {
                    let what = match cmd {
                        "type.rename" => "A column type is renamed",
                        "type.duplicate" => "A column type is copied",
                        _ => "A column type is deleted",
                    };
                    Err(use_instead(what, &format!("column.{cmd}")))
                }
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// A column type as a page lists it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct ColumnTypeView {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// `rect` or `round`.
    pub section: &'static str,
    /// A rectangle's width, as typed text; none for a round section.
    pub width: Option<String>,
    /// A rectangle's depth, as typed text; none for a round section.
    pub depth: Option<String>,
    /// A round section's diameter, as typed text; none for a rectangle.
    pub diameter: Option<String>,
    /// Its material's ID.
    pub material: String,
}

/// One column as a page lists it, worked out from the document as it stands now, never stored.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct ColumnView {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its type's ID, if it names one.
    #[serde(rename = "type")]
    pub type_id: Option<String>,
    /// The level its base is tied to, and its offset as typed text.
    pub base: Option<TieView>,
    /// The level its top is tied to, and its offset; none for an unconnected height.
    pub top: Option<TieView>,
    /// Its unconnected height as typed text; none when its top is tied to a level.
    pub height: Option<String>,
    /// Where it stands.
    pub at: AtView,
    /// Its turn, in degrees as typed text.
    pub turn: String,
    /// `built`, or `failed` when the builder could not make it.
    pub state: &'static str,
    /// Why it did not build, in the builder's words; empty when it built.
    pub words: String,
    /// What it reads back (height, section area, volume, mass), each shown in the document's units.
    pub quantities: Vec<QuantityView>,
}

/// A tie to a level, as a page shows it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct TieView {
    /// The level's ID.
    pub level: String,
    /// The offset from it.
    pub offset: String,
}

/// Where a column stands, as a page shows it.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(tag = "on", rename_all = "lowercase")]
pub enum AtView {
    /// At a point.
    Point {
        /// Along x, as typed text.
        x: String,
        /// Along y, as typed text.
        y: String,
    },
    /// Where two grid lines cross.
    Grids {
        /// The first grid's ID.
        first: String,
        /// The second grid's ID.
        second: String,
        /// Which crossing.
        cross: usize,
    },
}

/// One read-only quantity.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct QuantityView {
    /// Its parameter's ID, such as `column.volume`.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its value shown in the document's units.
    pub shown: Option<String>,
}

/// A document's column types and columns, for the server's read-only view.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct ColumnsView {
    /// Every type of the columns category with a section, by name, then ID.
    pub types: Vec<ColumnTypeView>,
    /// Every column, by ID.
    pub columns: Vec<ColumnView>,
}

fn type_view(ty: &ElementType, spec: &ColumnSpec) -> ColumnTypeView {
    let (section, width, depth, diameter) = match spec.section {
        Section::Rectangular { width, depth } => (
            "rect",
            Some(width.to_string()),
            Some(depth.to_string()),
            None,
        ),
        Section::Round { diameter } => ("round", None, None, Some(diameter.to_string())),
    };
    ColumnTypeView {
        id: ty.id().to_string(),
        name: ty.name().to_owned(),
        section,
        width,
        depth,
        diameter,
        material: spec.material.as_str().to_owned(),
    }
}

fn tie_view(tie: &Tie) -> TieView {
    TieView {
        level: tie.level.to_string(),
        offset: tie.offset.to_string(),
    }
}

/// An angle in degrees as typed text, exactly.
fn degrees(angle: Angle) -> String {
    let degrees = angle.degrees();
    if *degrees.denom() == 1 {
        degrees.numer().to_string()
    } else {
        format!("{}/{}", degrees.numer(), degrees.denom())
    }
}

/// A document's column types and columns as they are now: construct-materials' default library
/// until a project keeps its own. A type of the columns category without a section is not listed.
/// A column that cannot be read is listed with the builder's words as its state. The inner error
/// is a record the document's rules can't read, in their words.
///
/// # Errors
/// A store error.
pub async fn read_columns<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<ColumnsView, String>, StoreError> {
    let context = match records::read_context(reader, project, document).await? {
        Ok(context) => context,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let all = records::read_document(reader, project, document).await?;
    let type_records: Vec<Element> = all
        .values()
        .map(|(_, record)| record.clone())
        .filter(|record| record.kind == TYPE_KIND)
        .collect();
    let types = match Types::from_records(&type_records, &context.params, &context.categories) {
        Ok(types) => types,
        Err(e) => return Ok(Err(why(e))),
    };
    let mut listed: Vec<ColumnTypeView> = types
        .in_category(COLUMNS)
        .into_iter()
        .filter_map(|ty| Some(type_view(ty, &spec_of(ty).ok()?)))
        .collect();
    listed.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let shown = units();
    let mut columns = Vec::new();
    for (_, record) in all.values().filter(|(_, record)| is_column(record)) {
        let ty = record
            .props
            .get(construct_types::TYPE_KEY)
            .and_then(|text| Id::parse(text).ok())
            .and_then(|id| types.get(&id));
        columns.push(column_view(reader, project, document, record, ty, &shown).await?);
    }
    columns.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Ok(ColumnsView {
        types: listed,
        columns,
    }))
}

/// One column as a page lists it: its record, its built row and what it reads back.
async fn column_view<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    record: &Element,
    ty: Option<&ElementType>,
    shown: &Units,
) -> Result<ColumnView, StoreError> {
    {
        let rows =
            building::view_rows(reader, project, document, record, ty, shown, At::Now).await?;
        let built: Option<BuiltElement> = read_row(
            reader,
            &ModelPlace::Built {
                project: project.clone(),
                document: document.clone(),
                element: record.id.clone(),
            }
            .key(),
        )
        .await?;
        let (state, words) = match built.map(|built| built.result) {
            Some(BuiltResult::Shape { .. }) => ("built", String::new()),
            Some(BuiltResult::Failed { code, message }) => ("failed", format!("{code}: {message}")),
            None => ("failed", "it is not built yet".to_owned()),
        };
        let Ok(column) = column_of(record) else {
            return Ok(ColumnView {
                id: record.id.to_string(),
                name: record.name.clone(),
                type_id: None,
                base: None,
                top: None,
                height: None,
                at: AtView::Point {
                    x: String::new(),
                    y: String::new(),
                },
                turn: String::new(),
                state: "failed",
                words: "its record is not a column's as this server writes it".to_owned(),
                quantities: Vec::new(),
            });
        };
        let (top, height) = match &column.top {
            Top::Level(tie) => (Some(tie_view(tie)), None),
            Top::Height(height) => (None, Some(height.to_string())),
        };
        Ok(ColumnView {
            id: record.id.to_string(),
            name: record.name.clone(),
            type_id: records::type_of(record).map(|id| id.to_string()),
            base: construct_levels::base_of(record)
                .ok()
                .flatten()
                .as_ref()
                .map(tie_view),
            top,
            height,
            at: match &column.position {
                Position::Point { x, y } => AtView::Point {
                    x: x.to_string(),
                    y: y.to_string(),
                },
                Position::Grids {
                    first,
                    second,
                    cross,
                } => AtView::Grids {
                    first: first.to_string(),
                    second: second.to_string(),
                    cross: *cross,
                },
            },
            turn: degrees(column.turn),
            state,
            words,
            quantities: rows
                .into_iter()
                .map(|row| QuantityView {
                    id: row.id,
                    name: row.name,
                    shown: row.shown,
                })
                .collect(),
        })
    }
}
