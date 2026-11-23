//! The commands that make and change wall types and draw layered walls (job 4.1, CON-A4-F2-P1).
//! They go through the same edit log as `element.*`, with the same access check (`model.edit`)
//! and the same claims: a piece another person is working on is refused, and one person's edits
//! land in log order. Each is one entry, so one undo takes back all it wrote
//! ([`crate::cloud::undo`]).
//!
//! Each command checks the change with construct-walls' own rules (a wall type's layers and
//! core), construct-types' (types and instances) and construct-levels' (ties), and a drawn wall
//! with the very reader and plan the builders use ([`construct_regen::levels::recipe_at`] and
//! [`construct_regen::wall_plan::Layered::of`]), so a wall the command takes is one the builders
//! take. A refusal is the command's result, in the data crate's words, and changes nothing.
//! Nothing here reads a clock or a random number: the claim's time is the orderer's.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `wall.type.create` | `type`, `name`, `assembly` | A wall type: of the walls category, its layers and core in its `@system.wall` block, listing every parameter bound to walls. If the document's parameter table does not yet define and bind `wall.location-line` and `wall.flipped` to walls, the table too, in the same entry, and every other type of the walls category then lists them |
//! | `wall.type.layers` | `type`, `assembly` | The wall type: its layers and core |
//! | `wall.type.rename` | `type`, `name` | The wall type, as `type.rename` |
//! | `wall.type.duplicate` | `type`, `new`, `name` | A copy of the wall type, layers and all, as `type.duplicate` |
//! | `wall.type.delete` | `type` | Its pointer goes, as `type.delete`: refused while a wall is of it |
//! | `wall.draw` | `element`, `name` (default the ID), `type`, `start`, `end`, `level`, `offset`, then `top` and `top_offset` or `height`, and `line`, `flipped` | A new layered wall |
//!
//! `assembly` is the layers from the exterior face to the interior face and the core between its
//! two boundaries, as construct-walls takes them: `{"core":{"exterior":1,"interior":2},
//! "layers":[{"function":"finish","thickness":"12.5 mm","material":"plasterboard-gypsum",
//! "wraps":true}, ...]}`; `function` is one of construct-walls' stored names, `thickness` typed
//! text, `wraps` no when left out.
//!
//! `wall.draw`: `start` and `end` are the reference line's two points in the model's plan, each
//! `[x, y]` as typed text (`["0", "4 m"]`); walking from `start` to `end`, the exterior is on the
//! right. `level` ties its base to a level (its ID; left out, on no level) `offset` above it
//! (typed text, default `0`). Its top is tied to the level `top`, `top_offset` above it, or it is
//! `height` (typed text) high, one or the other. `line` is the location line on its reference
//! line (construct-walls' keys, default the wall centreline) and `flipped` puts its exterior on
//! the left; each, when given, is the instance's own value. The record: kind `wall`, `level` 0,
//! placement 0, its type and category, its ties, and the recipe `construct/wall.layered` with
//! `start` and `end` as stored lengths, `height` a stored length or `{"tie":"top"}`, and `layers`
//! `{"wall":"type"}`, read from its type when it is built. Refused: an element of that ID, a line
//! with no length, a type the document lacks, a type of another category or one without layers,
//! a top at or below the base, both a top and a height or neither, and anything the builders
//! would refuse, in their words.
//!
//! Typed text is read with construct-params' metric display settings until a project keeps its
//! own. Later, with its place: changing a drawn wall's ends, its location line and its side (a
//! command beside `wall.draw`).
//!
//! The generic commands do not go behind these ([`generic_rules`]), as they do not behind the
//! floor commands: `element.create` of a layered wall, `element.set` of a layered wall's recipe
//! (or of a recipe that would make one), `type.create` of a walls type, and `type.rename`,
//! `type.duplicate` or `type.delete` of a wall type (a walls type holding its layers) are refused,
//! naming the command to use. A walls type without layers, which only a document made before this
//! rule can hold and which the wall commands refuse, is renamed, copied and deleted by the type
//! commands as before; a straight wall (`construct/wall.straight`) is no layered wall and is made
//! and changed by the element commands as before.
//!
//! For the page and the seed, without the log: [`default_type_args`], the `wall.type.create`
//! arguments of construct-walls' default wall types, which the page sends where a document has no
//! wall types and the seed sends for its models; and [`read_wall_types`], a document's wall types
//! with their layers as these commands take them, and the materials the layers are drawn in.

use crate::cloud::elements::write_record;
use crate::cloud::{levels, params};
use construct_categories::{CategoryId, WALLS};
use construct_levels::{Length, Tie, parse_length, write_base, write_top};
use construct_materials::{MaterialId, Materials};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{ProjectParams, Units, Value};
use construct_regen::levels as placing;
use construct_regen::parameters::{self as records, At, CATEGORY_KEY, Context, TYPE_KIND};
use construct_regen::wall_plan::{self, Layered};
use construct_regen::walls::type_arg;
use construct_types::{ElementType, Instance, SYSTEM_KEY_PREFIX, TypeError, Types};
use construct_walls::{
    Assembly, FLIPPED, Function, LOCATION_LINE, Layer, LocationLine, Side, WallError, assembly_of,
    default_walls, location_line_value, new_wall_type, parameter_definitions, with_assembly,
};
use kit_core::Id;
use kit_ops::{Change, Claim, ClaimChange, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 6] = [
    "wall.draw",
    "wall.type.create",
    "wall.type.layers",
    "wall.type.rename",
    "wall.type.duplicate",
    "wall.type.delete",
];

/// The kind of a drawn wall's record.
const WALL_KIND: &str = "wall";

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    name: Option<String>,
    #[serde(default, rename = "type", with = "kit_ops::id::option")]
    type_id: Option<Id>,
    assembly: Option<AssemblyArgs>,
    start: Option<[String; 2]>,
    end: Option<[String; 2]>,
    /// The base level: its ID, or an old page's integer ([`levels::tie_of_new`]).
    level: Option<serde_json::Value>,
    offset: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    top: Option<Id>,
    top_offset: Option<String>,
    height: Option<String>,
    line: Option<String>,
    flipped: Option<bool>,
}

/// A wall type's layers and core as a person sends them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssemblyArgs {
    core: CoreArgs,
    layers: Vec<LayerArgs>,
}

/// The core's two boundaries: each the count of layers on its exterior side.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoreArgs {
    exterior: usize,
    interior: usize,
}

/// One layer as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerArgs {
    function: Function,
    /// Typed text.
    thickness: String,
    material: String,
    #[serde(default)]
    wraps: bool,
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

/// Turns a data crate's refusal into the command's result.
fn why(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// How typed lengths are read: construct-params' metric display settings, until a project keeps
/// its own.
fn units() -> Units {
    Units::default()
}

/// A typed length, or construct-levels' refusal of the text.
fn length(text: &str) -> Result<Length, String> {
    parse_length(text, &units()).map_err(why)
}

/// One of construct-walls' fixed parameter IDs.
fn fixed(text: &str) -> Result<Id, String> {
    Id::parse(text).map_err(|_| format!("{text:?} is not an ID."))
}

/// A wall type's layers and core from what a person sent, checked by construct-walls.
fn assembly(sent: AssemblyArgs) -> Result<Assembly, String> {
    let mut layers = Vec::with_capacity(sent.layers.len());
    for (at, layer) in sent.layers.into_iter().enumerate() {
        let thickness = parse_length(&layer.thickness, &units())
            .map_err(|e| format!("layer {at}'s thickness: {e}"))?;
        let material = MaterialId::parse(&layer.material).map_err(|e| why(WallError::from(e)))?;
        layers.push(Layer::new(layer.function, thickness, material, layer.wraps));
    }
    Assembly::new(layers, sent.core.exterior, sent.core.interior).map_err(why)
}

/// The document's parameter table with the two wall instance parameters defined and bound to
/// walls, each held by the instance with its default; `None` when it has them already. A table
/// that defines either ID as another parameter is refused: its wall types could not read it.
fn with_wall_parameters(context: &Context) -> Result<Option<ProjectParams>, String> {
    let mut table = context.table.clone();
    let mut changed = false;
    for (definition, default) in parameter_definitions().map_err(why)? {
        let id = definition.id().clone();
        match table.definition(&id) {
            None => {
                table = table
                    .add_project_parameter(definition.clone())
                    .map_err(why)?;
                changed = true;
            }
            Some(found) if *found == definition => {}
            Some(_) => {
                return Err(format!(
                    "{id} is another parameter in this model, so a wall type cannot read it."
                ));
            }
        }
        let bound = table
            .binding(&id)
            .is_some_and(|(_, categories)| categories.contains(&WALLS));
        if !bound {
            table = table
                .bind(
                    &context.categories,
                    &id,
                    &[WALLS],
                    definition.holder(),
                    Some(default),
                )
                .map_err(why)?;
            changed = true;
        }
    }
    Ok(changed.then_some(table))
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

    /// Writes a record and points its piece at it; the change, from what it was.
    async fn put(&mut self, record: &Element) -> Result<Change, StoreError> {
        let before = self.current(&record.id).await?.map(|(hash, _)| hash);
        let hash = write_record(self.txn, self.project, record).await?;
        let pointer = Place::Piece {
            project: self.project.clone(),
            document: self.document.clone(),
            piece: record.id.clone(),
        };
        self.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        Ok(Change {
            piece: record.id.clone(),
            before,
            after: Some(hash),
        })
    }

    /// Writes one record whose piece this person claims, as the command's whole result.
    async fn put_claimed(&mut self, record: &Element) -> Answer {
        let claim = match self.claim(&record.id).await? {
            Ok(claim) => claim,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let change = self.put(record).await?;
        let claim = claims::take(self.txn, self.project, self.document, &record.id, claim).await?;
        Ok(Ok(Applied {
            changes: vec![change],
            claims: vec![claim],
            ..Applied::default()
        }))
    }

    /// Every type of the document, read against `context`, as construct-types keeps them.
    async fn types(&mut self, context: &Context) -> Result<Result<Types, String>, StoreError> {
        let all = records::read_document(self.txn, self.project, self.document).await?;
        let records: Vec<Element> = all
            .into_values()
            .map(|(_, record)| record)
            .filter(|record| record.kind == TYPE_KIND)
            .collect();
        Ok(Types::from_records(&records, &context.params, &context.categories).map_err(why))
    }
}

/// A wall type of the document: a type of the walls category with layers in its wall block. The
/// inner error is why not, in construct-types' or construct-walls' words.
async fn wall_type<T: Txn>(
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
    Ok(assembly_of(&ty).map(|_| ty).map_err(why))
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
    // Renaming, copying and deleting a wall type are the type commands' own, once it is known to
    // be a wall type: the same rules, the same words.
    if let Some(as_type) = edit.cmd.strip_prefix("wall.")
        && matches!(as_type, "type.rename" | "type.duplicate" | "type.delete")
    {
        let id = match need(args.type_id, "a wall type") {
            Ok(id) => id,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if let Err(refusal) = wall_type(txn, project, &edit.target, &document, &id).await? {
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
        "wall.type.create" => create_type(&mut run, &document, args).await,
        "wall.type.layers" => set_layers(&mut run, &document, args).await,
        "wall.draw" => draw(&mut run, &document, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

async fn create_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = (|| {
        let id = need(args.type_id, "a wall type")?;
        let name = need(args.name, "a name")?;
        let assembly = assembly(need(args.assembly, "the layers and core")?)?;
        Ok::<_, String>((id, name, assembly))
    })();
    let (id, name, assembly) = match sent {
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let grown = match with_wall_parameters(context) {
        Ok(grown) => grown,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let after = match &grown {
        Some(table) => match Context::new(table.clone()) {
            Ok(after) => after,
            Err(refusal) => return Ok(Err(refusal)),
        },
        None => context.clone(),
    };
    // A new wall type lists every parameter bound to walls, as `type.create` does.
    let made = (|| {
        let mut ty =
            new_wall_type(id, &name, &assembly, &after.params, &after.categories).map_err(why)?;
        for applicable in after.table.parameters_of(&after.categories, WALLS) {
            let parameter = applicable.definition.id();
            if !ty.has(parameter) {
                ty.add_parameter(&after.params, parameter).map_err(why)?;
            }
        }
        Ok::<_, String>(ty)
    })();
    let ty = match made {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut types = match run.types(&after).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let others: Vec<ElementType> = types.in_category(WALLS).into_iter().cloned().collect();
    let record = ty.to_record(run.document);
    if let Err(e) = types.add(ty) {
        return Ok(Err(why(e)));
    }

    // Every piece it writes is claimed first, so another person's claim turns it all back.
    let mut pieces = vec![record.id.clone()];
    if grown.is_some() {
        pieces.push(records::table_id());
    }
    let mut held = Vec::new();
    for piece in pieces {
        match run.claim(&piece).await? {
            Ok(claim) => held.push((piece, claim)),
            Err(refusal) => return Ok(Err(refusal)),
        }
    }
    let mut changes = vec![run.put(&record).await?];
    if let Some(table) = &grown {
        let table = match records::table_record(run.document, table) {
            Ok(table) => table,
            Err(refusal) => return Ok(Err(refusal)),
        };
        changes.push(run.put(&table).await?);
        // Every other wall type lists the two parameters from now on, as `parameter.bind` does.
        let wall_parameters = [fixed(LOCATION_LINE), fixed(FLIPPED)];
        for other in others {
            let mut listed = other.clone();
            for parameter in &wall_parameters {
                let parameter = match parameter {
                    Ok(parameter) => parameter,
                    Err(refusal) => return Ok(Err(refusal.clone())),
                };
                if !listed.has(parameter)
                    && let Err(e) = listed.add_parameter(&after.params, parameter)
                {
                    return Ok(Err(why(e)));
                }
            }
            if listed != other {
                changes.push(run.put(&listed.to_record(run.document)).await?);
            }
        }
    }
    let mut taken: Vec<ClaimChange> = Vec::new();
    for (piece, claim) in held {
        taken.push(claims::take(run.txn, run.project, run.document, &piece, claim).await?);
    }
    Ok(Ok(Applied {
        changes,
        claims: taken,
        ..Applied::default()
    }))
}

async fn set_layers<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = (|| {
        let id = need(args.type_id, "a wall type")?;
        let assembly = assembly(need(args.assembly, "the layers and core")?)?;
        Ok::<_, String>((id, assembly))
    })();
    let (id, assembly) = match sent {
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match wall_type(run.txn, run.project, run.document, context, &id).await? {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = match with_assembly(ty, &assembly) {
        Ok(changed) => changed,
        Err(e) => return Ok(Err(why(e))),
    };
    run.put_claimed(&changed.to_record(run.document)).await
}

/// A point from two typed lengths.
fn point(sent: Option<[String; 2]>, what: &str) -> Result<[Length; 2], String> {
    let [x, y] = need(sent, what)?;
    Ok([length(&x)?, length(&y)?])
}

/// A point as a recipe holds it: `[x, y]` as two stored lengths.
fn stored_point([x, y]: [Length; 2]) -> serde_json::Value {
    json!([Value::Length(x).stored(), Value::Length(y).stored()])
}

async fn draw<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let id = match need(args.element.clone(), "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let name = args.name.clone().unwrap_or_else(|| id.to_string());
    let type_id = match need(args.type_id.clone(), "a wall type") {
        Ok(type_id) => type_id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match records::read_type(run.txn, run.project, run.document, &type_id, context).await?
    {
        Ok(ty) if ty.category() != WALLS => {
            let refusal = TypeError::OtherCategory {
                from: WALLS,
                to: ty.category(),
            };
            return Ok(Err(why(refusal)));
        }
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(e) = assembly_of(&ty) {
        return Ok(Err(why(e)));
    }
    let shaped = (|| {
        let start = point(args.start.clone(), "a start")?;
        let end = point(args.end.clone(), "an end")?;
        if start == end {
            return Err(format!(
                "{name}'s start and end are the same point: a wall has a length."
            ));
        }
        Ok::<_, String>((start, end, own_values(context, &ty, &args)?))
    })();
    let (start, end, mut props) = match shaped {
        Ok(shaped) => shaped,
        Err(refusal) => return Ok(Err(refusal)),
    };
    props.insert(CATEGORY_KEY.to_owned(), records::write_category(WALLS));
    let height = match ties(run, &args, &name, &mut props).await? {
        Ok(height) => height,
        Err(refusal) => return Ok(Err(refusal)),
    };

    let mut recipe_args = BTreeMap::new();
    recipe_args.insert("start".to_owned(), stored_point(start));
    recipe_args.insert("end".to_owned(), stored_point(end));
    recipe_args.insert("height".to_owned(), height);
    recipe_args.insert("layers".to_owned(), type_arg());
    let record = Element {
        id: id.clone(),
        document: run.document.clone(),
        kind: WALL_KIND.to_owned(),
        name: name.clone(),
        // On no level by number: the tie in `props` is its level.
        level: 0,
        // Its line is in the model's plan, so its frame is the model's.
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct(wall_plan::OP, recipe_args),
        props,
        references: Vec::new(),
    };
    if let Err(refusal) =
        levels::top_is_above_base(run.txn, run.project, run.document, &record).await?
    {
        return Ok(Err(refusal));
    }
    // What the builders would make of it, read the way they read it.
    let built =
        match placing::recipe_at(run.txn, run.project, run.document, &record, At::Now).await? {
            Ok(filled) => Layered::of(&filled).map(|_| ()),
            Err(failed) => Err(failed),
        };
    if let Err(failed) = built {
        return Ok(Err(format!("{name}: {}", failed.message)));
    }

    let hash = write_record(run.txn, run.project, &record).await?;
    let pointer = Place::Piece {
        project: run.project.clone(),
        document: run.document.clone(),
        piece: id.clone(),
    };
    run.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: id,
            before: None,
            after: Some(hash),
        }],
        ..Applied::default()
    }))
}

/// A new wall's ties, written into its `props`: its base on a level or on none, and its top on a
/// level; and its recipe's `height`, `{"tie":"top"}` or the stored length it was given. The inner
/// error is the refusal, in words.
async fn ties<T: Txn>(
    run: &mut Run<'_, T>,
    args: &Args,
    name: &str,
    props: &mut BTreeMap<String, String>,
) -> Result<Result<serde_json::Value, String>, StoreError> {
    let base =
        match levels::tie_of_new(run.txn, run.project, run.document, args.level.as_ref()).await? {
            Ok(base) => base,
            Err(refusal) => return Ok(Err(refusal)),
        };
    let base = match (base, args.offset.as_deref()) {
        (base, None) => base,
        (Some(tie), Some(text)) => match length(text) {
            Ok(offset) => Some(Tie { offset, ..tie }),
            Err(refusal) => return Ok(Err(refusal)),
        },
        (None, Some(_)) => {
            return Ok(Err(
                "A base offset is measured from a level, and none is given.".to_owned(),
            ));
        }
    };
    write_base(props, base.as_ref());
    Ok(match (&args.top, &args.height, &args.top_offset) {
        (Some(top), None, offset) => {
            if let Err(refusal) = levels::level_now(run.txn, run.project, run.document, top).await?
            {
                return Ok(Err(refusal));
            }
            offset
                .as_deref()
                .map_or(Ok(Length::from_steps(0)), length)
                .map(|offset| {
                    let tie = Tie {
                        level: top.clone(),
                        offset,
                    };
                    write_top(props, Some(&tie));
                    placing::top_arg()
                })
        }
        (None, Some(_), Some(_)) => {
            Err("A top offset is measured from a top level, and none is given.".to_owned())
        }
        (None, Some(text), None) => match length(text) {
            Ok(height) if height > Length::from_steps(0) => {
                Ok(json!(Value::Length(height).stored()))
            }
            Ok(height) => Err(format!(
                "{name} would be {height} high: a top is above its base."
            )),
            Err(refusal) => Err(refusal),
        },
        (Some(_), Some(_), _) => {
            Err("A wall's top is tied to a level or given as a height, not both.".to_owned())
        }
        (None, None, _) => Err("A wall needs its top tied to a level or a height.".to_owned()),
    })
}

/// A new wall's own values as its record's `props`: its type, and its location line and side
/// when they are given, each checked by construct-types against its type.
fn own_values(
    context: &Context,
    ty: &ElementType,
    args: &Args,
) -> Result<BTreeMap<String, String>, String> {
    let mut instance = Instance::new(ty);
    if let Some(key) = &args.line {
        let line = LocationLine::from_key(key)
            .ok_or_else(|| format!("{key:?} is not a location line."))?;
        instance
            .set(
                &context.params,
                ty,
                &fixed(LOCATION_LINE)?,
                location_line_value(line),
            )
            .map_err(why)?;
    }
    if let Some(flipped) = args.flipped {
        instance
            .set(&context.params, ty, &fixed(FLIPPED)?, Value::YesNo(flipped))
            .map_err(why)?;
    }
    Ok(instance.to_props())
}

/// A wall type's layers and core as `wall.type.create` and `wall.type.layers` take them: from the
/// exterior face, each thickness as typed text that reads back exactly.
#[must_use]
pub fn assembly_args(assembly: &Assembly) -> serde_json::Value {
    let layers: Vec<serde_json::Value> = assembly
        .layers()
        .iter()
        .map(|layer| {
            json!({ "function": layer.function, "thickness": layer.thickness.to_string(),
                    "material": layer.material.as_str(), "wraps": layer.wraps })
        })
        .collect();
    json!({
        "core": {
            "exterior": assembly.boundary(Side::Exterior),
            "interior": assembly.boundary(Side::Interior),
        },
        "layers": layers,
    })
}

/// The `wall.type.create` arguments that make construct-walls' default wall types, in their order,
/// each under its permanent ID: what the seed sends for its models, and what the page sends where a
/// document has no wall types, so that both go through the log.
///
/// # Errors
/// None for this fixed data; construct-walls checks each layer, in its words.
pub fn default_type_args() -> Result<Vec<serde_json::Value>, String> {
    Ok(default_walls()
        .map_err(why)?
        .iter()
        .map(|wall| {
            json!({ "type": wall.id, "name": wall.name, "assembly": assembly_args(&wall.assembly) })
        })
        .collect())
}

/// A document's wall types, as the page lists them for its wall tool: each with its layers, the
/// materials those are drawn in, and the defaults to make where there are none.
#[derive(Serialize, Debug)]
pub struct WallTypesView {
    /// Every type of the walls category with layers, by name, then ID.
    pub types: Vec<WallTypeView>,
    /// The materials a layer may be made of, each with the colours a view draws it in.
    pub materials: Vec<MaterialView>,
    /// [`default_type_args`].
    pub defaults: Vec<serde_json::Value>,
}

/// One wall type.
#[derive(Serialize, Debug)]
pub struct WallTypeView {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its whole thickness, as typed text.
    pub thickness: String,
    /// Its layers and core, as [`assembly_args`] gives them.
    pub assembly: serde_json::Value,
}

/// A material, as a view draws it.
#[derive(Serialize, Debug)]
pub struct MaterialView {
    /// Its ID, as a layer names it.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its colour seen in projection, `#rrggbb`.
    pub surface: String,
    /// Its colour where a view cuts it, `#rrggbb`.
    pub cut: String,
}

/// A colour's red, green and blue as `#rrggbb`.
fn hex(red: u8, green: u8, blue: u8) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// The materials a layer may be made of, each with the colours a view draws it in:
/// construct-materials' default library until a project keeps its own. The wall types' and the
/// floor types' views both list them.
#[must_use]
pub fn materials_view() -> Vec<MaterialView> {
    Materials::default_metric()
        .materials()
        .iter()
        .map(|material| {
            let (surface, cut) = (
                &material.drawing.surface.colour,
                &material.drawing.cut.colour,
            );
            MaterialView {
                id: material.id.as_str().to_owned(),
                name: material.name.clone(),
                surface: hex(surface.0, surface.1, surface.2),
                cut: hex(cut.0, cut.1, cut.2),
            }
        })
        .collect()
}

/// A document's wall types as they are now, and the materials: construct-materials' default
/// library until a project keeps its own. The inner error is a record the document's rules can't
/// read, in their words.
///
/// # Errors
/// A store error.
pub async fn read_wall_types<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<WallTypesView, String>, StoreError> {
    let context = match records::read_context(reader, project, document).await? {
        Ok(context) => context,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let all = records::read_document(reader, project, document).await?;
    let records: Vec<Element> = all
        .into_values()
        .map(|(_, record)| record)
        .filter(|record| record.kind == TYPE_KIND)
        .collect();
    let types = match Types::from_records(&records, &context.params, &context.categories) {
        Ok(types) => types,
        Err(e) => return Ok(Err(why(e))),
    };
    let mut listed: Vec<WallTypeView> = types
        .in_category(WALLS)
        .into_iter()
        .filter_map(|ty| {
            let assembly = assembly_of(ty).ok()?;
            Some(WallTypeView {
                id: ty.id().to_string(),
                name: ty.name().to_owned(),
                thickness: assembly.total().to_string(),
                assembly: assembly_args(&assembly),
            })
        })
        .collect();
    listed.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let materials = materials_view();
    let defaults = match default_type_args() {
        Ok(defaults) => defaults,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(Ok(WallTypesView {
        types: listed,
        materials,
        defaults,
    }))
}

/// The words for a wall command that should have been used.
fn use_instead(what: &str, command: &str) -> String {
    format!("{what} with {command}, which checks it as a wall.")
}

/// The part of a generic command's arguments these rules read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    recipe: Option<Recipe>,
    #[serde(default, rename = "type", with = "kit_ops::id::option")]
    type_id: Option<Id>,
    category: Option<CategoryId>,
}

/// Whether a record is a wall type the wall commands take: a type of the walls category holding
/// its layers in its wall block.
fn is_layered_type(record: &Element) -> bool {
    record.kind == TYPE_KIND
        && records::stored_category(record) == Some(WALLS)
        && record
            .props
            .contains_key(&format!("{SYSTEM_KEY_PREFIX}{}", construct_walls::OWNER))
}

/// The wall rules of the generic commands, asked before one runs, as the floor rules are
/// ([`crate::cloud::floors::generic_rules`]): a layered wall is drawn by `wall.draw`, not made by
/// `element.create` nor changed by `element.set` of its recipe; a wall type is made by
/// `wall.type.create`, not `type.create`, and renamed, copied and deleted by `wall.type.rename`,
/// `wall.type.duplicate` and `wall.type.delete`. The inner error is the refusal, naming the
/// command to use. An edit these rules have nothing to say of (another command, arguments that do
/// not read, an element that is not there, a straight wall, a walls type without layers) is left
/// to the command, which answers it as it always has.
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
    let a_layered_recipe = args.recipe.as_ref().is_some_and(wall_plan::is_layered);
    Ok(match edit.cmd.as_str() {
        "element.create" if a_layered_recipe => {
            Err(use_instead("A layered wall is drawn", "wall.draw"))
        }
        "element.set" if args.recipe.is_some() => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record)) if wall_plan::is_layered(&record.recipe) => Err(use_instead(
                    &format!("{} is a layered wall, drawn", record.name),
                    "wall.draw",
                )),
                Some(_) if a_layered_recipe => {
                    Err(use_instead("A layered wall is drawn", "wall.draw"))
                }
                _ => Ok(()),
            }
        }
        "type.create" if args.category == Some(WALLS) => {
            Err(use_instead("A wall type is made", "wall.type.create"))
        }
        cmd @ ("type.rename" | "type.duplicate" | "type.delete") => {
            let Some(id) = &args.type_id else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record)) if is_layered_type(&record) => {
                    let what = match cmd {
                        "type.rename" => "A wall type is renamed",
                        "type.duplicate" => "A wall type is copied",
                        _ => "A wall type is deleted",
                    };
                    Err(use_instead(what, &format!("wall.{cmd}")))
                }
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}
