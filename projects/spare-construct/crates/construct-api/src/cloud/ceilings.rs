//! The commands that make and change ceiling types and ceilings, placed by clicking inside a room's
//! walls or drawn from a sketched outline (job A101, CON-A4-F4-P2). They go through the same edit
//! log as `element.*`, with the same access check (`model.edit`) and the same claims, and read as
//! the floor commands do ([`crate::cloud::floors`]): each is one entry, so one undo takes back all
//! it wrote.
//!
//! A ceiling is a flat layered slab, held and built as a floor is (construct-slabs' `ceiling`),
//! with two differences a person sees: its height is its exposed face, `offset` above its level
//! (the layers stand above it), and it may carry a grid pattern of tiles.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `ceiling.type.create` | `type`, `name`, `layers` | A ceiling type: of the ceilings category, its layers in its `@system.ceiling` block |
//! | `ceiling.type.layers` | `type`, `layers` | The ceiling type: its layers |
//! | `ceiling.type.rename` | `type`, `name` | The ceiling type, as `type.rename` |
//! | `ceiling.type.duplicate` | `type`, `new`, `name` | A copy of the ceiling type, as `type.duplicate` |
//! | `ceiling.type.delete` | `type` | Its pointer goes, as `type.delete`: refused while a ceiling is of it |
//! | `ceiling.defaults` | none (`{}`) | Each of construct-slabs' default ceiling types the document lacks, as `ceiling.type.create` makes one |
//! | `ceiling.create` | `element`, `name` (default the ID), `type`, `level`, `offset` (default `0`), `boundary` or `at`, `grid` (optional) | A new ceiling |
//! | `ceiling.boundary` | `element`, `boundary` or `at` | The ceiling: its outline |
//! | `ceiling.grid` | `element`, `grid`: `{"width","depth","origin":[x, y],"angle":degrees}` | The ceiling: its grid pattern, set or changed |
//! | `ceiling.grid.clear` | `element` | The ceiling: its grid taken away |
//!
//! `boundary` is a floor's ([`crate::cloud::floors`]): the outer loop and the holes (the ceiling's
//! openings, drawn by sketch), each corner's `at` an `[x, y]` of typed text. `at` is instead a
//! point `[x, y]` of typed text inside a room's walls: the outline is the loop of walls round it
//! on the ceiling's level, found by the very search a room uses ([`construct_rooms::find`]) at the
//! walls' faces, with its island walls as holes, and written as an ordinary boundary (it does not
//! follow the walls afterwards). Exactly one of the two is given. A point not enclosed, or on a
//! wall, is refused in the search's words.
//!
//! `grid`: `width` and `depth` are a tile's size along the pattern's x and y axes (typed
//! lengths), `origin` a point a grid line goes through (the lower left corner of the outline's
//! bounding box when left out), `angle` whole degrees from 0 up to 180 (default `0`). A turned
//! grid (any angle but 0 and 90) is kept and drawn, and its tiles are not counted until the
//! kernel's exact rotation lands (construct-slabs' `tiles`, the interim register).
//!
//! `ceiling.create`: `level` is the level the ceiling's exposed face stands over (its ID),
//! `offset` above it. The record: kind `ceiling`, its type and category, its tie, and the recipe
//! `construct/ceiling` as construct-slabs' `ceiling_recipe` writes it. Refused: an element of that
//! ID, a type the document lacks, a type of another category or one without layers, no level or
//! one not in the model, an outline construct-slabs refuses (the loop and the edge named), an area
//! that is no whole number of square grid steps, and anything the builders would refuse, in their
//! words.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a
//! ceiling, `element.set` of a ceiling's recipe, `type.create` of a ceilings type and
//! `type.rename`, `type.duplicate` or `type.delete` of a ceiling type are refused, naming the
//! command to use. A ceiling's level and offset are changed by `level.tie`, as any element's.
//!
//! For the page, without the log: [`read_ceiling_types`], a document's ceiling types with their
//! layers and the materials the layers are drawn in, which the page's ceiling tool lists (it sends
//! `ceiling.defaults` where there are none).
//!
//! Not here: a sloped ceiling. The floors' slope arrow gives the slab's top face a slope and its
//! layers hang plumb below it, while a ceiling's height is its exposed face; a ceiling that
//! slopes is its own piece, with the room volume under it (a room is bounded by a flat ceiling
//! here, and construct-rooms refuses a sloped limit by name).

use crate::cloud::floors::{
    Answer, BoundaryArgs, LayerArgs, Run, layers, layers_args, length, need, plain_boundary, why,
};
use crate::cloud::walls::{MaterialView, materials_view};
use crate::cloud::{levels, params};
use construct_categories::{CEILINGS, CategoryId};
use construct_levels::{Tie, base_of};
use construct_model::element::Element;
use construct_regen::floor_plan::FloorPlan;
use construct_regen::levels as placing;
use construct_regen::parameters::{self as records, At, Context, TYPE_KIND};
use construct_slabs::{
    Boundary, Grid, Point, Ring, area, ceiling_boundary, ceiling_layers_of, ceiling_record,
    default_ceiling_types, grid_of, is_ceiling, new_ceiling_type, with_ceiling_boundary,
    with_ceiling_layers, with_grid,
};
use construct_types::{ElementType, TypeError, Types};
use kit_core::Id;
use kit_ops::Edit;
use kit_store::{Place, Reader, StoreError, Txn, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

/// Every command here.
pub const COMMANDS: [&str; 10] = [
    "ceiling.create",
    "ceiling.boundary",
    "ceiling.grid",
    "ceiling.grid.clear",
    "ceiling.defaults",
    "ceiling.type.create",
    "ceiling.type.layers",
    "ceiling.type.rename",
    "ceiling.type.duplicate",
    "ceiling.type.delete",
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
    /// A point inside a room's walls, typed text.
    at: Option<[String; 2]>,
    grid: Option<GridArgs>,
}

/// A grid pattern as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GridArgs {
    width: String,
    depth: String,
    #[serde(default)]
    origin: Option<[String; 2]>,
    #[serde(default)]
    angle: Option<i64>,
}

/// A ceiling type of the document: a type of the ceilings category with layers in its ceiling
/// block. The inner error is why not, in construct-types' or construct-slabs' words.
async fn ceiling_type<T: Txn>(
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
    Ok(ceiling_layers_of(&ty).map(|_| ty).map_err(why))
}

/// A new ceiling type that lists every parameter bound to ceilings, as `type.create` makes one.
fn listed(ty: ElementType, context: &Context) -> Result<ElementType, String> {
    let mut ty = ty;
    for applicable in context.table.parameters_of(&context.categories, CEILINGS) {
        let parameter = applicable.definition.id();
        if !ty.has(parameter) {
            ty.add_parameter(&context.params, parameter).map_err(why)?;
        }
    }
    Ok(ty)
}

/// What the builders would make of `record`, read the way they read it: the refusal, after its
/// name, when they would refuse it.
async fn buildable<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
) -> Result<Result<(), String>, StoreError> {
    let filled = placing::recipe_at(run.txn, run.project, run.document, record, At::Now).await?;
    let built = filled.and_then(|filled| FloorPlan::of(&filled).map(|_| ()));
    Ok(built.map_err(|failed| format!("{}: {}", record.name, failed.message)))
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
    // Renaming, copying and deleting a ceiling type are the type commands' own, once it is known
    // to be a ceiling type: the same rules, the same words.
    if let Some(as_type) = edit.cmd.strip_prefix("ceiling.")
        && matches!(as_type, "type.rename" | "type.duplicate" | "type.delete")
    {
        let id = match need(args.type_id, "a ceiling type") {
            Ok(id) => id,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if let Err(refusal) = ceiling_type(txn, project, &edit.target, &document, &id).await? {
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
        "ceiling.type.create" => create_type(&mut run, &document, args).await,
        "ceiling.type.layers" => set_layers(&mut run, &document, args).await,
        "ceiling.defaults" => defaults(&mut run, &document).await,
        "ceiling.create" => create(&mut run, &document, args).await,
        "ceiling.boundary" => set_boundary(&mut run, args).await,
        "ceiling.grid" => set_grid(&mut run, args, false).await,
        "ceiling.grid.clear" => set_grid(&mut run, args, true).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

async fn create_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = (|| {
        let id = need(args.type_id, "a ceiling type")?;
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
    let made = new_ceiling_type(id, &name, &layers, &context.categories)
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
        let id = need(args.type_id, "a ceiling type")?;
        let layers = layers(need(args.layers, "the layers")?)?;
        Ok::<_, String>((id, layers))
    })();
    let (id, layers) = match sent {
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match ceiling_type(run.txn, run.project, run.document, context, &id).await? {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = match with_ceiling_layers(ty, &layers) {
        Ok(changed) => changed,
        Err(e) => return Ok(Err(why(e))),
    };
    run.put_claimed(&changed.to_record(run.document)).await
}

/// Each default ceiling type the document lacks: none of its ID is in the document and no ceiling
/// type of the document has its name. None lacking writes nothing, which is no step.
async fn defaults<T: Txn>(run: &mut Run<'_, T>, context: &Context) -> Answer {
    let defaults = match default_ceiling_types(&context.categories) {
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
            .in_category(CEILINGS)
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

/// A ceiling's outline from what a person sent: `boundary`, each corner's typed text read as a
/// length and checked by construct-slabs with its area held exactly, or `at`, the loop of walls
/// round a point of the level `level`. The inner error is the refusal, in words, after `name`.
async fn outline<T: Txn>(
    run: &mut Run<'_, T>,
    name: &str,
    level: &Id,
    boundary: Option<BoundaryArgs>,
    at: Option<[String; 2]>,
) -> Result<Result<Boundary, String>, StoreError> {
    let plain = match (boundary, at) {
        (Some(_), Some(_)) => {
            return Ok(Err(
                "A ceiling's outline is a boundary or a point in a room, not both.".to_owned(),
            ));
        }
        (None, None) => {
            return Ok(Err(
                "This command needs a boundary or a point in a room.".to_owned()
            ));
        }
        (Some(sent), None) => match plain_boundary(sent) {
            Ok(plain) => plain,
            Err(refusal) => return Ok(Err(refusal)),
        },
        (None, Some([x, y])) => {
            let point = match (length(&x), length(&y)) {
                (Ok(x), Ok(y)) => [x.steps(), y.steps()],
                (Err(refusal), _) | (_, Err(refusal)) => return Ok(Err(refusal)),
            };
            let found = construct_regen::rooms::loop_at(
                run.txn,
                run.project,
                run.document,
                level,
                point,
                At::Now,
            )
            .await?;
            match found {
                Ok(Some(found)) => {
                    let ring = |corners: &[construct_rooms::Corner]| -> Vec<Json> {
                        corners.iter().map(|c| json!({ "at": c.at })).collect()
                    };
                    json!({
                        "outer": ring(&found.outer),
                        "holes": found.holes.iter().map(|h| ring(h)).collect::<Vec<_>>(),
                    })
                }
                Ok(None) => {
                    return Ok(Err(
                        "No closed loop of walls on that level round that point.".to_owned(),
                    ));
                }
                Err(refusal) => return Ok(Err(refusal)),
            }
        }
    };
    let checked = Boundary::from_plain(&plain).and_then(|b| area(&b).map(|_| b));
    Ok(checked.map_err(|e| format!("{name}'s boundary: {e}")))
}

/// A grid from what a person sent, its origin at the lower left of `boundary`'s bounding box when
/// it names none.
fn grid_of_args(sent: GridArgs, boundary: &Boundary) -> Result<Grid, String> {
    let width = length(&sent.width)?;
    let depth = length(&sent.depth)?;
    let origin = match sent.origin {
        Some([x, y]) => Point {
            x: length(&x)?.steps(),
            y: length(&y)?.steps(),
        },
        None => {
            let corners = boundary.corners(Ring::Outer).unwrap_or_default();
            Point {
                x: corners.iter().map(|(_, p)| p.x).min().unwrap_or(0),
                y: corners.iter().map(|(_, p)| p.y).min().unwrap_or(0),
            }
        }
    };
    Grid::new(width, depth, origin, sent.angle.unwrap_or(0)).map_err(why)
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
    let type_id = match need(args.type_id, "a ceiling type") {
        Ok(type_id) => type_id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match records::read_type(run.txn, run.project, run.document, &type_id, context).await?
    {
        Ok(ty) if ty.category() != CEILINGS => {
            let refusal = TypeError::OtherCategory {
                from: CEILINGS,
                to: ty.category(),
            };
            return Ok(Err(why(refusal)));
        }
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(e) = ceiling_layers_of(&ty) {
        return Ok(Err(why(e)));
    }
    let tie = match tie(run, args.level.as_ref(), args.offset.as_deref()).await? {
        Ok(tie) => tie,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let boundary = match outline(run, &name, &tie.level, args.boundary, args.at).await? {
        Ok(boundary) => boundary,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let grid = match args
        .grid
        .map(|sent| grid_of_args(sent, &boundary))
        .transpose()
    {
        Ok(grid) => grid,
        Err(refusal) => return Ok(Err(format!("{name}: {refusal}"))),
    };
    let record = ceiling_record(
        &id,
        run.document,
        &name,
        &type_id,
        &tie,
        &boundary,
        grid.as_ref(),
    );
    if let Err(refusal) = buildable(run, &record).await? {
        return Ok(Err(refusal));
    }
    let hash = crate::cloud::elements::write_record(run.txn, run.project, &record).await?;
    let pointer = Place::Piece {
        project: run.project.clone(),
        document: run.document.clone(),
        piece: id.clone(),
    };
    run.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    Ok(Ok(Applied {
        changes: vec![kit_ops::Change {
            piece: id,
            before: None,
            after: Some(hash),
        }],
        ..Applied::default()
    }))
}

/// A new ceiling's tie: its exposed face on the level `level` names, `offset` above it. The inner
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
                "A ceiling stands over a level: this command needs a level.".to_owned(),
            ));
        }
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(match offset {
        None => Ok(base),
        Some(text) => length(text).map(|offset| Tie { offset, ..base }),
    })
}

/// The ceiling `element` names, as the document has it, or the refusal.
async fn ceiling_of<T: Txn>(
    run: &mut Run<'_, T>,
    element: Option<Id>,
) -> Result<Result<Element, String>, StoreError> {
    let id = match need(element, "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((_, before)) = run.current(&id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    Ok(if is_ceiling(&before) {
        Ok(before)
    } else {
        Err(format!("{} is not a ceiling.", before.name))
    })
}

async fn set_boundary<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let before = match ceiling_of(run, args.element.clone()).await? {
        Ok(before) => before,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let name = before.name.clone();
    let level = match base_of(&before) {
        Ok(Some(tie)) => tie.level,
        Ok(None) => return Ok(Err(format!("{name} stands over no level."))),
        Err(e) => return Ok(Err(format!("{name}: {e}"))),
    };
    let boundary = match outline(run, &name, &level, args.boundary, args.at).await? {
        Ok(boundary) => boundary,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let record = match with_ceiling_boundary(before, &boundary) {
        Ok(record) => record,
        Err(e) => return Ok(Err(why(e))),
    };
    if let Err(refusal) = buildable(run, &record).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(&record).await
}

async fn set_grid<T: Txn>(run: &mut Run<'_, T>, args: Args, clear: bool) -> Answer {
    let before = match ceiling_of(run, args.element.clone()).await? {
        Ok(before) => before,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let name = before.name.clone();
    let grid = if clear {
        None
    } else {
        let built = ceiling_boundary(&before.recipe)
            .map_err(why)
            .and_then(|boundary| {
                let sent = args
                    .grid
                    .ok_or_else(|| "This command needs a grid.".to_owned())?;
                grid_of_args(sent, &boundary)
            });
        match built {
            Ok(grid) => Some(grid),
            Err(refusal) => return Ok(Err(format!("{name}: {refusal}"))),
        }
    };
    if clear && matches!(grid_of(&before.recipe), Ok(None)) {
        return Ok(Err(format!("{name} has no grid to clear.")));
    }
    let record = match with_grid(before, grid.as_ref()) {
        Ok(record) => record,
        Err(e) => return Ok(Err(why(e))),
    };
    run.put_claimed(&record).await
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

/// The words for a ceiling command that should have been used.
fn use_instead(what: &str, command: &str) -> String {
    format!("{what} with {command}, which checks it as a ceiling.")
}

/// The ceiling rules of the generic commands, asked before one runs: a ceiling is made by
/// `ceiling.create`, not `element.create`; its outline and grid are changed by `ceiling.boundary`,
/// `ceiling.grid` and `ceiling.grid.clear`, not by `element.set` of its recipe; a ceiling type is
/// made by `ceiling.type.create`, not `type.create`, and renamed, copied and deleted by
/// `ceiling.type.rename`, `ceiling.type.duplicate` and `ceiling.type.delete`. The inner error is
/// the refusal, naming the command to use; an edit these rules have nothing to say of is left to
/// the command, which answers it as it always has.
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
    let a_ceiling_recipe = args.recipe.as_ref().is_some_and(|recipe| {
        recipe.owner == "construct" && recipe.op == construct_slabs::CEILING_OP
    });
    Ok(match edit.cmd.as_str() {
        "element.create"
            if a_ceiling_recipe || args.kind.as_deref() == Some(construct_slabs::CEILING_KIND) =>
        {
            Err(use_instead("A ceiling is made", "ceiling.create"))
        }
        "element.set" if args.recipe.is_some() => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record)) if is_ceiling(&record) => Err(format!(
                    "{}'s outline and grid are changed with ceiling.boundary, ceiling.grid or ceiling.grid.clear, which check it as a ceiling.",
                    record.name
                )),
                Some(_) if a_ceiling_recipe => {
                    Err(use_instead("A ceiling is made", "ceiling.create"))
                }
                _ => Ok(()),
            }
        }
        "type.create" if args.category == Some(CEILINGS) => {
            Err(use_instead("A ceiling type is made", "ceiling.type.create"))
        }
        cmd @ ("type.rename" | "type.duplicate" | "type.delete") => {
            let Some(id) = &args.type_id else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record))
                    if record.kind == TYPE_KIND
                        && records::stored_category(&record) == Some(CEILINGS) =>
                {
                    let what = match cmd {
                        "type.rename" => "A ceiling type is renamed",
                        "type.duplicate" => "A ceiling type is copied",
                        _ => "A ceiling type is deleted",
                    };
                    Err(use_instead(what, &format!("ceiling.{cmd}")))
                }
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// A document's ceiling types, as the page lists them for its ceiling tool: each with its layers,
/// and the materials those are drawn in.
#[derive(Serialize, Debug)]
pub struct CeilingTypesView {
    /// Every type of the ceilings category with layers, by name, then ID.
    pub types: Vec<CeilingTypeView>,
    /// The materials a layer may be made of, each with the colours a view draws it in.
    pub materials: Vec<MaterialView>,
}

/// One ceiling type.
#[derive(Serialize, Debug)]
pub struct CeilingTypeView {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its whole thickness, as typed text.
    pub thickness: String,
    /// Its layers from the upper face down, as [`layers_args`] gives them.
    pub layers: Json,
}

/// A document's ceiling types as they are now, and the materials: construct-materials' default
/// library until a project keeps its own. A type of the ceilings category without layers is not
/// listed (no ceiling can be of it). The inner error is a record the document's rules can't read,
/// in their words.
///
/// # Errors
/// A store error.
pub async fn read_ceiling_types<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<CeilingTypesView, String>, StoreError> {
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
    let mut listed: Vec<CeilingTypeView> = types
        .in_category(CEILINGS)
        .into_iter()
        .filter_map(|ty| {
            let layers = ceiling_layers_of(ty).ok()?;
            Some(CeilingTypeView {
                id: ty.id().to_string(),
                name: ty.name().to_owned(),
                thickness: layers.total().to_string(),
                layers: layers_args(&layers),
            })
        })
        .collect();
    listed.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    Ok(Ok(CeilingTypesView {
        types: listed,
        materials: materials_view(),
    }))
}
