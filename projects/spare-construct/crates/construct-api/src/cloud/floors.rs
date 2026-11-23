//! The commands that make and change floor types and floors drawn from a boundary (job 4.3,
//! CON-A4-F4-P1). They go through the same edit log as `element.*`, with the same access check
//! (`model.edit`) and the same claims: a piece another person is working on is refused, and one
//! person's edits land in log order. Each is one entry, so one undo takes back all it wrote
//! ([`crate::cloud::undo`]). They read as the wall commands do ([`crate::cloud::walls`]).
//!
//! Each command checks the change with construct-slabs' own rules (a floor type's layers, a
//! boundary's loops and edges), construct-types' (types) and construct-levels' (ties), and a
//! floor with the very reader and plan the builders use ([`construct_regen::levels::recipe_at`]
//! and [`construct_regen::floor_plan::FloorPlan::of`]), so a floor the command takes is one the
//! builders take. A refusal is the command's result, in the data crate's words, and changes
//! nothing; one about a boundary names the loop and the edge. Nothing here reads a clock or a
//! random number: the claim's time is the orderer's.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `floor.type.create` | `type`, `name`, `layers` | A floor type: of the floors category, its layers in its `@system.floor` block, listing every parameter bound to floors |
//! | `floor.type.layers` | `type`, `layers` | The floor type: its layers |
//! | `floor.type.rename` | `type`, `name` | The floor type, as `type.rename` |
//! | `floor.type.duplicate` | `type`, `new`, `name` | A copy of the floor type, layers and all, as `type.duplicate` |
//! | `floor.type.delete` | `type` | Its pointer goes, as `type.delete`: refused while a floor is of it |
//! | `floor.defaults` | none (`{}`) | Each of construct-slabs' default floor types the document lacks (by ID, or by name among its floor types), as `floor.type.create` makes one; nothing when it lacks none |
//! | `floor.create` | `element`, `name` (default the ID), `type`, `level`, `offset` (default `0`), `boundary` | A new floor |
//! | `floor.boundary` | `element`, `boundary` | The floor: its boundary |
//!
//! `layers` is the layers from the top face down, as construct-slabs takes them:
//! `[{"function":"finish","thickness":"50 mm","material":"screed-cement"}, ...]`; `function` is one
//! of construct-slabs' stored names, `thickness` typed text.
//!
//! `boundary` is the floor's outer loop and its holes in the model's plan, each a list of corners:
//! `{"outer":[{"id":"e0","at":["0","0"]}, ...],"holes":[[...], ...]}`, each corner's `at` an
//! `[x, y]` of typed text and its `id` the permanent ID of the edge from it to the next corner
//! (left out: `o<n>` on the outer loop, `h<k>-<n>` on hole `k`), which names the floor's side
//! face there; `holes` may be left out. A boundary drawn again with the same edge IDs keeps its
//! sides' names.
//!
//! `floor.create`: `level` is the level the floor's top face stands on (its ID), `offset` above it
//! (typed text). The record: kind `floor`, `level` 0, placement 0, its type and category, its tie,
//! and the recipe `construct/floor` with the boundary in its stored form and `layers`
//! `{"type":"layers"}`, read from its type when it is built. Refused: an element of that ID, a
//! type the document lacks, a type of another category or one without layers, no level or one not
//! in the model, a boundary construct-slabs refuses (the loop and the edge named), an area that
//! is no whole number of square grid steps, and anything the builders would refuse, in their
//! words. `floor.boundary` refuses the same of the boundary, and an element that is not a floor.
//!
//! A floor type reads no parameter of the document's table (its layers are in its own block, and
//! what a floor reads back is worked out, never stored), so these commands define and bind none;
//! a new floor type lists every parameter already bound to floors, as `type.create` does.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a floor,
//! `element.set` of a floor's recipe, `type.create` of a floors type and `type.rename`,
//! `type.duplicate` or `type.delete` of a floor type are refused, naming the command to use.
//!
//! For the page, without the log: [`read_floor_types`], a document's floor types with their layers
//! as these commands take them ([`layers_args`]) and the materials the layers are drawn in, which
//! the page's floor tool lists (it sends `floor.defaults` where there are none).
//!
//! Typed text is read with construct-params' metric display settings until a project keeps its
//! own. A floor's level and offset are changed by `level.tie`, as any element's. Later, with
//! their places: a sketch with constraints for its boundary (job 5.2); arc edges, once the
//! kernel's sweep takes them (the interim register).

use crate::cloud::elements::write_record;
use crate::cloud::walls::{MaterialView, materials_view};
use crate::cloud::{levels, params};
use construct_categories::{CategoryId, FLOORS};
use construct_levels::{Length, Tie, parse_length};
use construct_materials::MaterialId;
use construct_model::element::Element;
use construct_params::Units;
use construct_regen::floor_plan::{self, FloorPlan};
use construct_regen::levels as placing;
use construct_regen::parameters::{self as records, At, Context, TYPE_KIND};
use construct_slabs::{
    Boundary, FloorLayer, Function, Layers, SlabError, area, default_floor_types, floor_record,
    is_floor, layers_of, new_floor_type, with_boundary, with_layers,
};
use construct_types::{ElementType, TypeError, Types};
use kit_core::Id;
use kit_ops::{Change, Claim, ClaimChange, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

/// Every command here.
pub const COMMANDS: [&str; 8] = [
    "floor.create",
    "floor.boundary",
    "floor.defaults",
    "floor.type.create",
    "floor.type.layers",
    "floor.type.rename",
    "floor.type.duplicate",
    "floor.type.delete",
];

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
    layers: Option<Vec<LayerArgs>>,
    /// The level: its ID ([`levels::tie_of_new`]).
    level: Option<Json>,
    offset: Option<String>,
    boundary: Option<BoundaryArgs>,
}

/// One layer as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerArgs {
    function: Function,
    /// Typed text.
    thickness: String,
    material: String,
}

/// A boundary as a person sends it: its outer loop and its holes, each a list of corners.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundaryArgs {
    outer: Vec<CornerArgs>,
    #[serde(default)]
    holes: Vec<Vec<CornerArgs>>,
}

/// One corner as a person sends it: where it is, as typed text, and the ID of the edge that
/// starts there, if given.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CornerArgs {
    #[serde(default)]
    id: Option<String>,
    at: [String; 2],
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

/// A floor type's layers from what a person sent, checked by construct-slabs.
fn layers(sent: Vec<LayerArgs>) -> Result<Layers, String> {
    let mut layers = Vec::with_capacity(sent.len());
    for (at, layer) in sent.into_iter().enumerate() {
        let thickness = parse_length(&layer.thickness, &units())
            .map_err(|e| format!("layer {at}'s thickness: {e}"))?;
        let material = MaterialId::parse(&layer.material).map_err(|e| why(SlabError::from(e)))?;
        layers.push(FloorLayer::new(layer.function, thickness, material));
    }
    Layers::new(layers).map_err(why)
}

/// A boundary from what a person sent: each corner's typed text read as a length, then checked by
/// construct-slabs ([`Boundary::from_plain`]), and its area held exactly. A refusal names the
/// loop and the edge, after `name`.
fn boundary(sent: BoundaryArgs, name: &str) -> Result<Boundary, String> {
    let corners = |list: Vec<CornerArgs>| -> Result<Vec<Json>, String> {
        list.into_iter()
            .map(|corner| {
                let [x, y] = corner.at;
                let at = json!([length(&x)?.steps(), length(&y)?.steps()]);
                Ok(match corner.id {
                    Some(id) => json!({ "id": id, "at": at }),
                    None => json!({ "at": at }),
                })
            })
            .collect()
    };
    let outer = corners(sent.outer)?;
    let holes = sent
        .holes
        .into_iter()
        .map(corners)
        .collect::<Result<Vec<_>, _>>()?;
    let plain = json!({ "outer": outer, "holes": holes });
    let checked = Boundary::from_plain(&plain).and_then(|b| area(&b).map(|_| b));
    checked.map_err(|e| format!("{name}'s boundary: {e}"))
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

    /// Writes new floor types, each piece claimed first so another person's claim turns it all
    /// back, as the command's whole result.
    async fn put_types(&mut self, made: &[ElementType]) -> Answer {
        let mut held = Vec::with_capacity(made.len());
        for ty in made {
            match self.claim(ty.id()).await? {
                Ok(claim) => held.push((ty.id().clone(), claim)),
                Err(refusal) => return Ok(Err(refusal)),
            }
        }
        let mut changes = Vec::with_capacity(made.len());
        for ty in made {
            changes.push(self.put(&ty.to_record(self.document)).await?);
        }
        let mut taken: Vec<ClaimChange> = Vec::with_capacity(held.len());
        for (piece, claim) in held {
            taken.push(claims::take(self.txn, self.project, self.document, &piece, claim).await?);
        }
        Ok(Ok(Applied {
            changes,
            claims: taken,
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

    /// What the builders would make of `record`, read the way they read it: the refusal, after
    /// its name, when they would refuse it.
    async fn buildable(&mut self, record: &Element) -> Result<Result<(), String>, StoreError> {
        let filled =
            placing::recipe_at(self.txn, self.project, self.document, record, At::Now).await?;
        let built = filled.and_then(|filled| FloorPlan::of(&filled).map(|_| ()));
        Ok(built.map_err(|failed| format!("{}: {}", record.name, failed.message)))
    }
}

/// A floor type of the document: a type of the floors category with layers in its floor block.
/// The inner error is why not, in construct-types' or construct-slabs' words.
async fn floor_type<T: Txn>(
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
    Ok(layers_of(&ty).map(|_| ty).map_err(why))
}

/// A new floor type that lists every parameter bound to floors, as `type.create` makes one.
fn listed(ty: ElementType, context: &Context) -> Result<ElementType, String> {
    let mut ty = ty;
    for applicable in context.table.parameters_of(&context.categories, FLOORS) {
        let parameter = applicable.definition.id();
        if !ty.has(parameter) {
            ty.add_parameter(&context.params, parameter).map_err(why)?;
        }
    }
    Ok(ty)
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
    // Renaming, copying and deleting a floor type are the type commands' own, once it is known to
    // be a floor type: the same rules, the same words.
    if let Some(as_type) = edit.cmd.strip_prefix("floor.")
        && matches!(as_type, "type.rename" | "type.duplicate" | "type.delete")
    {
        let id = match need(args.type_id, "a floor type") {
            Ok(id) => id,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if let Err(refusal) = floor_type(txn, project, &edit.target, &document, &id).await? {
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
        "floor.type.create" => create_type(&mut run, &document, args).await,
        "floor.type.layers" => set_layers(&mut run, &document, args).await,
        "floor.defaults" => defaults(&mut run, &document).await,
        "floor.create" => create(&mut run, &document, args).await,
        "floor.boundary" => set_boundary(&mut run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

async fn create_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = (|| {
        let id = need(args.type_id, "a floor type")?;
        let name = need(args.name, "a name")?;
        let layers = layers(need(args.layers, "the layers")?)?;
        Ok::<_, String>((id, name, layers))
    })();
    let (id, name, layers) = match sent {
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let made = new_floor_type(id, &name, &layers, &context.categories)
        .map_err(why)
        .and_then(|ty| listed(ty, context));
    let ty = match made {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut types = match run.types(context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(e) = types.add(ty.clone()) {
        return Ok(Err(why(e)));
    }
    run.put_types(&[ty]).await
}

async fn set_layers<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = (|| {
        let id = need(args.type_id, "a floor type")?;
        let layers = layers(need(args.layers, "the layers")?)?;
        Ok::<_, String>((id, layers))
    })();
    let (id, layers) = match sent {
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match floor_type(run.txn, run.project, run.document, context, &id).await? {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = match with_layers(ty, &layers) {
        Ok(changed) => changed,
        Err(e) => return Ok(Err(why(e))),
    };
    run.put_claimed(&changed.to_record(run.document)).await
}

/// Each default floor type the document lacks: none of its ID is in the document and no floor
/// type of the document has its name. None lacking writes nothing, which is no step.
async fn defaults<T: Txn>(run: &mut Run<'_, T>, context: &Context) -> Answer {
    let defaults = match default_floor_types(&context.categories) {
        Ok(defaults) => defaults,
        Err(e) => return Ok(Err(why(e))),
    };
    let mut types = match run.types(context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut made = Vec::new();
    for ty in defaults {
        let named = types
            .in_category(FLOORS)
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

async fn create<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let id = match need(args.element, "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let name = args.name.unwrap_or_else(|| id.to_string());
    let type_id = match need(args.type_id, "a floor type") {
        Ok(type_id) => type_id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match records::read_type(run.txn, run.project, run.document, &type_id, context).await?
    {
        Ok(ty) if ty.category() != FLOORS => {
            let refusal = TypeError::OtherCategory {
                from: FLOORS,
                to: ty.category(),
            };
            return Ok(Err(why(refusal)));
        }
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(e) = layers_of(&ty) {
        return Ok(Err(why(e)));
    }
    let tie = match tie(run, args.level.as_ref(), args.offset.as_deref()).await? {
        Ok(tie) => tie,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let boundary = match need(args.boundary, "a boundary").and_then(|sent| boundary(sent, &name)) {
        Ok(boundary) => boundary,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let record = floor_record(&id, run.document, &name, &type_id, &tie, &boundary);
    if let Err(refusal) = run.buildable(&record).await? {
        return Ok(Err(refusal));
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

/// A new floor's tie: its top face on the level `level` names, `offset` above it. The inner
/// error is the refusal, in words.
async fn tie<T: Txn>(
    run: &mut Run<'_, T>,
    level: Option<&Json>,
    offset: Option<&str>,
) -> Result<Result<Tie, String>, StoreError> {
    let base = match levels::tie_of_new(run.txn, run.project, run.document, level).await? {
        Ok(Some(base)) => base,
        Ok(None) => {
            return Ok(Err(
                "A floor stands on a level: this command needs a level.".to_owned(),
            ));
        }
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(match offset {
        None => Ok(base),
        Some(text) => length(text).map(|offset| Tie { offset, ..base }),
    })
}

async fn set_boundary<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match need(args.element, "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((_, before)) = run.current(&id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    if !is_floor(&before) {
        return Ok(Err(format!("{} is not a floor.", before.name)));
    }
    let name = before.name.clone();
    let boundary = match need(args.boundary, "a boundary").and_then(|sent| boundary(sent, &name)) {
        Ok(boundary) => boundary,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let record = match with_boundary(before, &boundary) {
        Ok(record) => record,
        Err(e) => return Ok(Err(why(e))),
    };
    if let Err(refusal) = run.buildable(&record).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record).await
}

/// The words for a floor command that should have been used.
fn use_instead(what: &str, command: &str) -> String {
    format!("{what} with {command}, which checks it as a floor.")
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

/// The floor rules of the generic commands, asked before one runs: a floor is made by
/// `floor.create`, not `element.create`; its boundary is changed by `floor.boundary`, not by
/// `element.set` of its recipe; a floor type is made by `floor.type.create`, not `type.create`,
/// and renamed, copied and deleted by `floor.type.rename`, `floor.type.duplicate` and
/// `floor.type.delete`. The inner error is the refusal, naming the command to use. An edit these
/// rules have nothing to say of (another command, arguments that do not read, an element that is
/// not there) is left to the command, which answers it as it always has.
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
    let a_floor_recipe = args.recipe.as_ref().is_some_and(floor_plan::is_floor);
    Ok(match edit.cmd.as_str() {
        "element.create"
            if a_floor_recipe || args.kind.as_deref() == Some(construct_slabs::FLOOR_KIND) =>
        {
            Err(use_instead("A floor is made", "floor.create"))
        }
        "element.set" if args.recipe.is_some() => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record)) if is_floor(&record) => Err(use_instead(
                    &format!("{}'s boundary is changed", record.name),
                    "floor.boundary",
                )),
                Some(_) if a_floor_recipe => Err(use_instead("A floor is made", "floor.create")),
                _ => Ok(()),
            }
        }
        "type.create" if args.category == Some(FLOORS) => {
            Err(use_instead("A floor type is made", "floor.type.create"))
        }
        cmd @ ("type.rename" | "type.duplicate" | "type.delete") => {
            let Some(id) = &args.type_id else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record))
                    if record.kind == TYPE_KIND
                        && records::stored_category(&record) == Some(FLOORS) =>
                {
                    let what = match cmd {
                        "type.rename" => "A floor type is renamed",
                        "type.duplicate" => "A floor type is copied",
                        _ => "A floor type is deleted",
                    };
                    Err(use_instead(what, &format!("floor.{cmd}")))
                }
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// A floor type's layers as `floor.type.create` and `floor.type.layers` take them: from the top
/// face down, each thickness as typed text that reads back exactly.
#[must_use]
pub fn layers_args(layers: &Layers) -> Json {
    Json::Array(
        layers
            .layers()
            .iter()
            .map(|layer| {
                json!({ "function": layer.function, "thickness": layer.thickness.to_string(),
                        "material": layer.material.as_str() })
            })
            .collect(),
    )
}

/// A document's floor types, as the page lists them for its floor tool: each with its layers, and
/// the materials those are drawn in.
#[derive(Serialize, Debug)]
pub struct FloorTypesView {
    /// Every type of the floors category with layers, by name, then ID.
    pub types: Vec<FloorTypeView>,
    /// The materials a layer may be made of, each with the colours a view draws it in.
    pub materials: Vec<MaterialView>,
}

/// One floor type.
#[derive(Serialize, Debug)]
pub struct FloorTypeView {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its whole thickness, as typed text.
    pub thickness: String,
    /// Its layers from the top face down, as [`layers_args`] gives them.
    pub layers: Json,
}

/// A document's floor types as they are now, and the materials: construct-materials' default
/// library until a project keeps its own. A type of the floors category without layers is not
/// listed (no floor can be of it). The inner error is a record the document's rules can't read, in
/// their words.
///
/// # Errors
/// A store error.
pub async fn read_floor_types<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<FloorTypesView, String>, StoreError> {
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
    let mut listed: Vec<FloorTypeView> = types
        .in_category(FLOORS)
        .into_iter()
        .filter_map(|ty| {
            let layers = layers_of(ty).ok()?;
            Some(FloorTypeView {
                id: ty.id().to_string(),
                name: ty.name().to_owned(),
                thickness: layers.total().to_string(),
                layers: layers_args(&layers),
            })
        })
        .collect();
    listed.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    Ok(Ok(FloorTypesView {
        types: listed,
        materials: materials_view(),
    }))
}
