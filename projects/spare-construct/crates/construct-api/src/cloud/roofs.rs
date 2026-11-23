//! The commands that make and change roof types and roofs on a rectangle (job 5.3, CON-A4-F4-P3).
//! They go through the same edit log as `element.*`, with the same access check (`model.edit`) and
//! the same claims: a piece another person is working on is refused, and one person's edits land
//! in log order. Each is one entry, so one undo takes back all it wrote ([`crate::cloud::undo`]).
//! They read as the floor commands do ([`crate::cloud::floors`]).
//!
//! Each command checks the change with construct-roofs' own rules (a footprint, its form, its
//! pitch and overhangs, every refusal in its words naming the edges and the job that lifts it),
//! construct-slabs' (a roof type's layers, a footprint's loop), construct-types' (types) and
//! construct-levels' (ties), and a roof with the very reader and plan the builders use
//! ([`construct_regen::levels::recipe_at`] and [`construct_regen::roof_plan::RoofPlan::of`]), so a
//! roof the command takes is one the builders take. A refusal is the command's result and changes
//! nothing. Nothing here reads a clock or a random number: the claim's time is the orderer's.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `roof.type.create` | `type`, `name`, `layers` | A roof type: of the roofs category, its layers in its `@system.roof` block, listing every parameter bound to roofs |
//! | `roof.type.layers` | `type`, `layers` | The roof type: its layers |
//! | `roof.type.rename` | `type`, `name` | The roof type, as `type.rename` |
//! | `roof.type.duplicate` | `type`, `new`, `name` | A copy of the roof type, layers and all, as `type.duplicate` |
//! | `roof.type.delete` | `type` | Its pointer goes, as `type.delete`: refused while a roof is of it |
//! | `roof.defaults` | none (`{}`) | Each of construct-roofs' default roof types the document lacks (by ID, or by name among its roof types), as `roof.type.create` makes one; nothing when it lacks none |
//! | `roof.create` | `element`, `name` (default the ID), `type`, `level`, `offset` (default `0`), `footprint`, `form` (default `flat`), `pitch`, `overhang` (default `0`), `eaves` | A new roof |
//! | `roof.footprint` | `element`, `footprint` | The roof: its footprint |
//! | `roof.form` | `element`, `form`, `pitch`, `eaves` | The roof: its form |
//! | `roof.pitch` | `element`, `pitch`, `edge` | The roof: the pitch of every edge it rises from, or of `edge` |
//! | `roof.overhang` | `element`, `overhang`, `edge` | The roof: the overhang of every edge, or of `edge` |
//! | `roof.level` | `element`, `level`, `offset` | The roof: the level its base is tied to, as `level.tie` |
//! | `roof.retype` | `element`, `type` | The roof: its type, as `element.type` |
//! | `roof.delete` | `element` | The roof goes, as `element.delete` |
//!
//! `layers` is as the floor commands take it (`[{"function":"structure","thickness":"200 mm",
//! "material":"concrete-regular"}, ...]`, from the outer face down, each thickness typed text,
//! measured square to the roof's faces).
//!
//! `footprint` is the roof's one loop in the model's plan, as a floor's boundary is: `{"outer":
//! [{"id":"e0","at":["0","0"]}, ...]}`, each corner's `at` an `[x, y]` of typed text and its `id`
//! the permanent ID of the edge from it to the next corner (left out: `o<n>`), which names the
//! roof's eaves or verge there. A footprint with a hole is refused, in construct-roofs' words: an
//! opening through a roof is job 4.2's.
//!
//! `form` is `flat`, `mono` or `gable`. `pitch` is typed text, a slope (`1:4`, `25%`) or an angle
//! (`30°`), held as typed. `overhang` is typed text: how far the roof runs on past an edge in
//! plan, at 0 or more; `roof.create` and `roof.form` give every edge the same. `eaves` names the
//! edges (IDs) the roof rises from: a mono-pitch roof rises from one and a gable from two opposite
//! ones; left out, a mono-pitch roof rises from the footprint's first edge and a gable from the
//! longer of the two pairs of opposite edges of a rectangle, its ridge along its length. What
//! construct-roofs refuses, it refuses in its words: hips, an uneven gable (`roof.pitch` with
//! `edge`), a pitch on a footprint that is not a rectangle along the plan's axes, an overhang the
//! roof cannot take.
//!
//! `roof.create`: `level` is the level the roof's base stands on (its ID), `offset` above it
//! (typed text). The record: kind `roof`, `level` 0, placement 0, its type and category, its tie,
//! and the recipe `construct/roof` with the footprint and edges in their stored form and `layers`
//! `{"type":"layers"}`, read from its type when it is built. Refused: an element of that ID, a
//! type the document lacks, of another category or without layers, no level or one not in the
//! model, and anything construct-roofs or the builders would refuse, in their words.
//! `roof.footprint` keeps what each edge does by its ID (a new edge has the first edge's overhang
//! and no pitch) and refuses a footprint that lacks an edge the roof rises from, naming it: change
//! the form first. `roof.pitch` refuses a flat roof, and an edge the roof does not rise from.
//! `roof.form` keeps each edge's overhang and, where it is not given, the pitch the roof had.
//!
//! A roof type reads no parameter of the document's table (its layers are in its own block, and
//! what a roof reads back is worked out, never stored), so these commands define and bind none; a
//! new roof type lists every parameter already bound to roofs, as `type.create` does.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a roof,
//! `element.set` of a roof's recipe, `element.delete` of a roof, `type.create` of a roofs type and
//! `type.rename`, `type.duplicate` or `type.delete` of a roof type are refused, naming the command
//! to use.
//!
//! For the page, without the log: [`read_roof_types`], a document's roof types with their layers
//! as these commands take them and the materials the layers are drawn in, which the page's roof
//! tool lists (it sends `roof.defaults` where there are none).
//!
//! Typed text is read with construct-params' metric display settings until a project keeps its
//! own.

use crate::cloud::floors::{
    Answer, BoundaryArgs, LayerArgs, Run, layers, length, need, plain_boundary, units, why,
};
use crate::cloud::walls::{MaterialView, materials_view};
use crate::cloud::{elements, levels, params};
use construct_categories::ROOFS;
use construct_levels::{Length, Tie};
use construct_model::element::Element;
use construct_params::{Definition, Holder, Kind, Value};
use construct_regen::levels as placing;
use construct_regen::parameters::{self as records, At, Context, TYPE_KIND};
use construct_regen::roof_plan::{self, RoofPlan};
use construct_roofs::{
    EdgeSpec, Pitch, Roof, default_roof_types, is_roof, layers_of, new_roof_type, roof_of,
    roof_record, with_layers, with_roof,
};
use construct_slabs::{Boundary, Layers, Ring};
use construct_types::{ElementType, TypeError, Types};
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::{Place, Reader, StoreError, Txn, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 14] = [
    "roof.create",
    "roof.footprint",
    "roof.form",
    "roof.pitch",
    "roof.overhang",
    "roof.level",
    "roof.retype",
    "roof.delete",
    "roof.defaults",
    "roof.type.create",
    "roof.type.layers",
    "roof.type.rename",
    "roof.type.duplicate",
    "roof.type.delete",
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
    footprint: Option<BoundaryArgs>,
    form: Option<String>,
    pitch: Option<String>,
    overhang: Option<String>,
    eaves: Option<Vec<String>>,
    edge: Option<String>,
}

/// A roof type of the document: a type of the roofs category with layers in its roof block. The
/// inner error is why not, in construct-types' or construct-roofs' words.
async fn roof_type<T: Txn>(
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

/// A new roof type that lists every parameter bound to roofs, as `type.create` makes one.
fn listed(ty: ElementType, context: &Context) -> Result<ElementType, String> {
    let mut ty = ty;
    for applicable in context.table.parameters_of(&context.categories, ROOFS) {
        let parameter = applicable.definition.id();
        if !ty.has(parameter) {
            ty.add_parameter(&context.params, parameter).map_err(why)?;
        }
    }
    Ok(ty)
}

/// The document's current roof record, or the refusal: it is gone, or it is not a roof.
async fn roof_record_of<T: Txn>(
    run: &mut Run<'_, T>,
    id: Option<Id>,
) -> Result<Result<Element, String>, StoreError> {
    let id = match need(id, "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((_, record)) = run.current(&id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    Ok(if is_roof(&record) {
        Ok(record)
    } else {
        Err(format!("{} is not a roof.", record.name))
    })
}

/// What the builders would make of `record`, read the way they read it: the refusal, after its
/// name, when they would refuse it.
async fn buildable<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
) -> Result<Result<(), String>, StoreError> {
    let filled = placing::recipe_at(run.txn, run.project, run.document, record, At::Now).await?;
    let built = filled.and_then(|filled| RoofPlan::of(&filled).map(|_| ()));
    Ok(built.map_err(|failed| format!("{}: {}", record.name, failed.message)))
}

/// A pitch from typed text: an angle when it is written in degrees (`30°`, `30 deg`), otherwise a
/// slope (`1:4`, `25%`), as construct-params reads each.
fn pitch(text: &str) -> Result<Pitch, String> {
    let text = text.trim();
    let kind = if text.ends_with('°') || text.ends_with("deg") {
        Kind::Angle
    } else {
        Kind::Slope
    };
    let id = Id::parse("roof.pitch").map_err(|_| "A pitch is a slope or an angle.".to_owned())?;
    let definition = Definition::new(id, "Pitch", kind, Holder::Instance).map_err(why)?;
    match definition.parse(text, &units()).map_err(why)? {
        Value::Slope(slope) => Ok(Pitch::Slope(slope)),
        Value::Angle(angle) => Ok(Pitch::Angle(angle)),
        _ => Err("A pitch is a slope or an angle.".to_owned()),
    }
}

/// An overhang from typed text.
fn overhang(text: &str) -> Result<Length, String> {
    length(text)
}

/// One edge of a roof being worked out: its ID and what it does.
type Edges = Vec<(Id, EdgeSpec)>;

/// Which edges the roof of this form rises from: those `eaves` names, or the defaults the module
/// header gives. The inner error is the refusal, in words.
fn eaves_of(footprint: &Boundary, form: &str, named: Option<&[String]>) -> Result<Vec<Id>, String> {
    let ids: Vec<Id> = footprint.outer().iter().map(|s| s.id.clone()).collect();
    let named: Option<Vec<Id>> = match named {
        Some(list) => Some(
            list.iter()
                .map(|text| Id::parse(text).map_err(|_| format!("{text:?} is not an edge's ID.")))
                .collect::<Result<_, _>>()?,
        ),
        None => None,
    };
    match form {
        "mono" => match named {
            Some(list) if list.len() == 1 => Ok(list),
            Some(_) => Err("A mono-pitch roof rises from one edge.".to_owned()),
            None => ids
                .first()
                .map(|first| vec![first.clone()])
                .ok_or_else(|| "A roof's footprint has no edge.".to_owned()),
        },
        "gable" => match named {
            Some(list) if list.len() == 2 => Ok(list),
            Some(_) => Err("A gable rises from two opposite edges.".to_owned()),
            None => {
                let corners = footprint.corners(Ring::Outer).unwrap_or_default();
                // The squared length of the edge that starts at corner `at`.
                let squared = |at: usize| -> i128 {
                    let next = at.saturating_add(1).checked_rem(corners.len()).unwrap_or(0);
                    match (corners.get(at), corners.get(next)) {
                        (Some((_, a)), Some((_, b))) => {
                            let dx = i128::from(b.x).saturating_sub(i128::from(a.x));
                            let dy = i128::from(b.y).saturating_sub(i128::from(a.y));
                            dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
                        }
                        _ => 0,
                    }
                };
                // The longer pair of opposite edges: the ridge runs along the roof's length.
                let start = usize::from(squared(0) < squared(1));
                let pair = [start, start.saturating_add(2)];
                let chosen: Vec<Id> = pair.iter().filter_map(|at| ids.get(*at).cloned()).collect();
                if chosen.len() == 2 {
                    Ok(chosen)
                } else {
                    Err("A gable rises from two opposite edges: name them in eaves.".to_owned())
                }
            }
        },
        _ => Ok(Vec::new()),
    }
}

/// Every edge of `footprint` with the form's pitch on the edges it rises from and `overhangs`
/// kept (an edge not listed has none), as the module header says. The inner error is the refusal,
/// in words.
fn edges_for(
    footprint: &Boundary,
    overhangs: &BTreeMap<Id, Length>,
    form: &str,
    pitch: Option<Pitch>,
    named: Option<&[String]>,
) -> Result<Edges, String> {
    match form {
        "flat" => {
            if pitch.is_some() {
                return Err(
                    "A flat roof does not slope: leave the pitch out, or choose mono or gable."
                        .to_owned(),
                );
            }
            if named.is_some_and(|list| !list.is_empty()) {
                return Err(
                    "A flat roof rises from no edge: leave eaves out, or choose mono or gable."
                        .to_owned(),
                );
            }
        }
        "mono" | "gable" => {}
        _ => return Err("A roof's form is flat, mono or gable.".to_owned()),
    }
    let eaves = eaves_of(footprint, form, named)?;
    let pitch = if form == "flat" {
        None
    } else {
        Some(pitch.ok_or_else(|| {
            "A roof that slopes needs a pitch, such as 1:4, 25% or 30°.".to_owned()
        })?)
    };
    let mut edges: Edges = footprint
        .outer()
        .iter()
        .map(|segment| {
            let overhang = overhangs
                .get(&segment.id)
                .copied()
                .unwrap_or(Length::from_steps(0));
            let pitch = pitch.filter(|_| eaves.contains(&segment.id));
            (segment.id.clone(), EdgeSpec { overhang, pitch })
        })
        .collect();
    // An edge named that the footprint lacks is construct-roofs' to refuse, in its words.
    let lacking: Vec<Id> = eaves
        .iter()
        .filter(|id| !edges.iter().any(|(own, _)| own == *id))
        .cloned()
        .collect();
    for id in lacking {
        edges.push((
            id,
            EdgeSpec {
                overhang: Length::from_steps(0),
                pitch,
            },
        ));
    }
    Ok(edges)
}

/// A roof from a footprint and its edges, checked by construct-roofs; its refusal comes after the
/// roof's name.
fn roof_from(footprint: Boundary, edges: &Edges, name: &str) -> Result<Roof, String> {
    let specs: BTreeMap<Id, EdgeSpec> = edges.iter().cloned().collect();
    Roof::new(footprint, &specs).map_err(|e| format!("{name}: {e}"))
}

/// A footprint from what a person sent: each corner's typed text read as a length, then checked
/// by construct-slabs. A refusal names the loop and the edge, after `name`.
fn footprint(sent: BoundaryArgs, name: &str) -> Result<Boundary, String> {
    if !sent.constraints.is_empty() {
        return Err(format!(
            "{name}'s footprint: a roof's footprint is drawn by its corners; only a floor's outline is sketched with constraints."
        ));
    }
    let plain = plain_boundary(sent)?;
    Boundary::from_plain(&plain).map_err(|e| format!("{name}'s footprint: {e}"))
}

/// What each edge of `roof` does, as a map.
fn specs_of(roof: &Roof) -> Edges {
    roof.edges().to_vec()
}

/// The pitch the roof rises at: its first sloping edge's, as it was given.
fn pitch_of(roof: &Roof) -> Option<Pitch> {
    roof.edges().iter().find_map(|(_, spec)| spec.pitch)
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
    // Renaming, copying and deleting a roof type are the type commands' own, once it is known to
    // be a roof type: the same rules, the same words.
    if let Some(as_type) = edit.cmd.strip_prefix("roof.")
        && matches!(as_type, "type.rename" | "type.duplicate" | "type.delete")
    {
        let id = match need(args.type_id, "a roof type") {
            Ok(id) => id,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if let Err(refusal) = roof_type(txn, project, &edit.target, &document, &id).await? {
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
        "roof.type.create" => create_type(&mut run, &document, args).await,
        "roof.type.layers" => set_layers(&mut run, &document, args).await,
        "roof.defaults" => defaults(&mut run, &document).await,
        "roof.create" => create(&mut run, &document, args).await,
        "roof.footprint" => set_footprint(&mut run, args).await,
        "roof.form" => set_form(&mut run, args).await,
        "roof.pitch" => set_pitch(&mut run, args).await,
        "roof.overhang" => set_overhang(&mut run, args).await,
        "roof.level" | "roof.retype" | "roof.delete" => {
            delegate(&mut run, &document, context, args).await
        }
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// `roof.level`, `roof.retype` and `roof.delete`: the roof checked as a roof, then the generic
/// command of the same job runs, with the same rules and the same words.
async fn delegate<T: Txn>(
    run: &mut Run<'_, T>,
    document: &Context,
    context: &Ordered<'_>,
    args: Args,
) -> Result<Outcome, StoreError> {
    if let Err(refusal) = roof_record_of(run, args.element.clone()).await? {
        return Ok(Err(refusal));
    }
    let mut generic = run.edit.clone();
    match run.edit.cmd.as_str() {
        "roof.level" => {
            if args.level.as_ref().is_none_or(Json::is_null) {
                return Ok(Err(
                    "A roof stands on a level: this command needs a level.".to_owned()
                ));
            }
            "level.tie".clone_into(&mut generic.cmd);
            return levels::apply(run.txn, run.project, context, &generic, run.claim_ms).await;
        }
        "roof.retype" => {
            let id = match need(args.type_id, "a roof type") {
                Ok(id) => id,
                Err(refusal) => return Ok(Err(refusal)),
            };
            if let Err(refusal) =
                roof_type(run.txn, run.project, run.document, document, &id).await?
            {
                return Ok(Err(refusal));
            }
            "element.type".clone_into(&mut generic.cmd);
            return params::apply(run.txn, run.project, context, &generic, run.claim_ms).await;
        }
        _ => {}
    }
    "element.delete".clone_into(&mut generic.cmd);
    elements::apply(run.txn, run.project, context, &generic, run.claim_ms).await
}

async fn create_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = (|| {
        let id = need(args.type_id, "a roof type")?;
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
    let made = new_roof_type(id, &name, &layers, &context.categories)
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
        let id = need(args.type_id, "a roof type")?;
        let layers = layers(need(args.layers, "the layers")?)?;
        Ok::<_, String>((id, layers))
    })();
    let (id, layers): (Id, Layers) = match sent {
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match roof_type(run.txn, run.project, run.document, context, &id).await? {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = match with_layers(ty, &layers) {
        Ok(changed) => changed,
        Err(e) => return Ok(Err(why(e))),
    };
    run.put_claimed(&changed.to_record(run.document)).await
}

/// Each default roof type the document lacks: none of its ID is in the document and no roof type
/// of the document has its name. None lacking writes nothing, which is no step.
async fn defaults<T: Txn>(run: &mut Run<'_, T>, context: &Context) -> Answer {
    let defaults = match default_roof_types(&context.categories) {
        Ok(defaults) => defaults,
        Err(e) => return Ok(Err(why(e))),
    };
    let mut types: Types = match run.types(context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut made = Vec::new();
    for ty in defaults {
        let named = types
            .in_category(ROOFS)
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
    let type_id = match need(args.type_id, "a roof type") {
        Ok(type_id) => type_id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match records::read_type(run.txn, run.project, run.document, &type_id, context).await?
    {
        Ok(ty) if ty.category() != ROOFS => {
            let refusal = TypeError::OtherCategory {
                from: ROOFS,
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
    let made = (|| {
        let footprint = need(args.footprint, "a footprint").and_then(|f| footprint(f, &name))?;
        let eaves_sent = args.eaves.as_deref();
        let pitch = args.pitch.as_deref().map(pitch).transpose()?;
        let overhang = args.overhang.as_deref().map(overhang).transpose()?;
        let overhangs: BTreeMap<Id, Length> = overhang
            .map(|by| {
                footprint
                    .outer()
                    .iter()
                    .map(|s| (s.id.clone(), by))
                    .collect()
            })
            .unwrap_or_default();
        let form = args.form.as_deref().unwrap_or("flat");
        let edges = edges_for(&footprint, &overhangs, form, pitch, eaves_sent)?;
        roof_from(footprint, &edges, &name)
    })();
    let roof = match made {
        Ok(roof) => roof,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let record = roof_record(&id, run.document, &name, &type_id, &tie, &roof);
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
        changes: vec![Change {
            piece: id,
            before: None,
            after: Some(hash),
        }],
        ..Applied::default()
    }))
}

/// A new roof's tie: its base on the level `level` names, `offset` above it. The inner error is
/// the refusal, in words.
async fn tie<T: Txn>(
    run: &mut Run<'_, T>,
    level: Option<&Json>,
    offset: Option<&str>,
) -> Result<Result<Tie, String>, StoreError> {
    let base = match levels::tie_of_new(run.txn, run.project, run.document, level).await? {
        Ok(Some(base)) => base,
        Ok(None) => {
            return Ok(Err(
                "A roof stands on a level: this command needs a level.".to_owned()
            ));
        }
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(match offset {
        None => Ok(base),
        Some(text) => length(text).map(|offset| Tie { offset, ..base }),
    })
}

/// Writes `record` after the builders' check, as the command's whole result.
async fn put_roof<T: Txn>(run: &mut Run<'_, T>, record: &Element) -> Answer {
    if let Err(refusal) = buildable(run, record).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(record).await
}

async fn set_footprint<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let before = match roof_record_of(run, args.element.clone()).await? {
        Ok(record) => record,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let name = before.name.clone();
    let made = (|| {
        let old = roof_of(&before.recipe).map_err(why)?;
        let footprint = need(args.footprint, "a footprint").and_then(|f| footprint(f, &name))?;
        let own: Vec<Id> = footprint.outer().iter().map(|s| s.id.clone()).collect();
        if let Some((id, _)) = old
            .edges()
            .iter()
            .find(|(id, spec)| spec.pitch.is_some() && !own.contains(id))
        {
            return Err(format!(
                "{name} rises from edge {id}, which the new footprint lacks: change its form with roof.form first."
            ));
        }
        // What an edge does stays with its ID; a new edge has the first edge's overhang.
        let first = old
            .edges()
            .first()
            .map_or(Length::from_steps(0), |(_, spec)| spec.overhang);
        let edges: Edges = own
            .into_iter()
            .map(|id| {
                let spec = old.edge(&id).copied().unwrap_or(EdgeSpec {
                    overhang: first,
                    pitch: None,
                });
                (id, spec)
            })
            .collect();
        let roof = roof_from(footprint, &edges, &name)?;
        with_roof(before.clone(), &roof).map_err(why)
    })();
    match made {
        Ok(record) => put_roof(run, &record).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn set_form<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let before = match roof_record_of(run, args.element.clone()).await? {
        Ok(record) => record,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let name = before.name.clone();
    let made = (|| {
        let old = roof_of(&before.recipe).map_err(why)?;
        let form = need(args.form.as_deref(), "a form")?;
        // The pitch it had stays unless a new one is given.
        let pitch = match args.pitch.as_deref() {
            Some(text) => Some(pitch(text)?),
            None if form == "flat" => None,
            None => pitch_of(&old),
        };
        let overhangs: BTreeMap<Id, Length> = old
            .edges()
            .iter()
            .map(|(id, spec)| (id.clone(), spec.overhang))
            .collect();
        let edges = edges_for(
            old.footprint(),
            &overhangs,
            form,
            pitch,
            args.eaves.as_deref(),
        )?;
        let roof = roof_from(old.footprint().clone(), &edges, &name)?;
        with_roof(before.clone(), &roof).map_err(why)
    })();
    match made {
        Ok(record) => put_roof(run, &record).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The ID of the edge `text` names, among `roof`'s.
fn edge_named(roof: &Roof, text: &str) -> Result<Id, String> {
    Id::parse(text)
        .ok()
        .filter(|id| roof.edge(id).is_some())
        .ok_or_else(|| format!("{text:?} is not an edge of this roof."))
}

async fn set_pitch<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let before = match roof_record_of(run, args.element.clone()).await? {
        Ok(record) => record,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let name = before.name.clone();
    let made = (|| {
        let old = roof_of(&before.recipe).map_err(why)?;
        let new = pitch(need(args.pitch.as_deref(), "a pitch")?)?;
        let only = args
            .edge
            .as_deref()
            .map(|text| edge_named(&old, text))
            .transpose()?;
        let mut edges = specs_of(&old);
        if pitch_of(&old).is_none() {
            return Err(format!(
                "{name} is flat, so it has no pitch to change: give it a form with roof.form."
            ));
        }
        if let Some(id) = &only
            && old.edge(id).is_some_and(|spec| spec.pitch.is_none())
        {
            return Err(format!(
                "{name} does not rise from edge {id}: change its form with roof.form."
            ));
        }
        for (id, spec) in &mut edges {
            if spec.pitch.is_some() && only.as_ref().is_none_or(|only| only == id) {
                spec.pitch = Some(new);
            }
        }
        let roof = roof_from(old.footprint().clone(), &edges, &name)?;
        with_roof(before.clone(), &roof).map_err(why)
    })();
    match made {
        Ok(record) => put_roof(run, &record).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn set_overhang<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let before = match roof_record_of(run, args.element.clone()).await? {
        Ok(record) => record,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let name = before.name.clone();
    let made = (|| {
        let old = roof_of(&before.recipe).map_err(why)?;
        let new = overhang(need(args.overhang.as_deref(), "an overhang")?)?;
        let only = args
            .edge
            .as_deref()
            .map(|text| edge_named(&old, text))
            .transpose()?;
        let mut edges = specs_of(&old);
        for (id, spec) in &mut edges {
            if only.as_ref().is_none_or(|only| only == id) {
                spec.overhang = new;
            }
        }
        let roof = roof_from(old.footprint().clone(), &edges, &name)?;
        with_roof(before.clone(), &roof).map_err(why)
    })();
    match made {
        Ok(record) => put_roof(run, &record).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The words for a roof command that should have been used.
fn use_instead(what: &str, command: &str) -> String {
    format!("{what} with {command}, which checks it as a roof.")
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
    category: Option<construct_categories::CategoryId>,
}

/// The roof rules of the generic commands, asked before one runs: a roof is made by `roof.create`,
/// not `element.create`; its footprint, form, pitch and overhangs are changed by the roof
/// commands, not by `element.set` of its recipe; it is deleted by `roof.delete`, not
/// `element.delete`; a roof type is made by `roof.type.create`, not `type.create`, and renamed,
/// copied and deleted by `roof.type.rename`, `roof.type.duplicate` and `roof.type.delete`. The
/// inner error is the refusal, naming the command to use. An edit these rules have nothing to say
/// of (another command, arguments that do not read, an element that is not there) is left to the
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
    let a_roof_recipe = args.recipe.as_ref().is_some_and(roof_plan::is_roof);
    Ok(match edit.cmd.as_str() {
        "element.create"
            if a_roof_recipe || args.kind.as_deref() == Some(construct_roofs::ROOF_KIND) =>
        {
            Err(use_instead("A roof is made", "roof.create"))
        }
        "element.set" if args.recipe.is_some() => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record)) if is_roof(&record) => Err(format!(
                    "{}'s footprint, form, pitch and overhangs are changed with roof.footprint, roof.form, roof.pitch or roof.overhang, which check them as a roof.",
                    record.name
                )),
                Some(_) if a_roof_recipe => Err(use_instead("A roof is made", "roof.create")),
                _ => Ok(()),
            }
        }
        "element.delete" => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record)) if is_roof(&record) => Err(format!(
                    "{} is a roof: delete it with roof.delete.",
                    record.name
                )),
                _ => Ok(()),
            }
        }
        "type.create" if args.category == Some(ROOFS) => {
            Err(use_instead("A roof type is made", "roof.type.create"))
        }
        cmd @ ("type.rename" | "type.duplicate" | "type.delete") => {
            let Some(id) = &args.type_id else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record))
                    if record.kind == TYPE_KIND
                        && records::stored_category(&record) == Some(ROOFS) =>
                {
                    let what = match cmd {
                        "type.rename" => "A roof type is renamed",
                        "type.duplicate" => "A roof type is copied",
                        _ => "A roof type is deleted",
                    };
                    Err(use_instead(what, &format!("roof.{cmd}")))
                }
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// A roof type's layers as `roof.type.create` and `roof.type.layers` take them: from the outer
/// face down, each thickness as typed text that reads back exactly.
#[must_use]
pub fn layers_args(layers: &Layers) -> Json {
    crate::cloud::floors::layers_args(layers)
}

/// A document's roof types, as the page lists them for its roof tool: each with its layers, and
/// the materials those are drawn in.
#[derive(Serialize, Debug)]
pub struct RoofTypesView {
    /// Every type of the roofs category with layers, by name, then ID.
    pub types: Vec<RoofTypeView>,
    /// The materials a layer may be made of, each with the colours a view draws it in.
    pub materials: Vec<MaterialView>,
}

/// One roof type.
#[derive(Serialize, Debug)]
pub struct RoofTypeView {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its whole thickness, as typed text.
    pub thickness: String,
    /// Its layers from the outer face down, as [`layers_args`] gives them.
    pub layers: Json,
}

/// A document's roof types as they are now, and the materials: construct-materials' default
/// library until a project keeps its own. A type of the roofs category without layers is not
/// listed (no roof can be of it). The inner error is a record the document's rules can't read, in
/// their words.
///
/// # Errors
/// A store error.
pub async fn read_roof_types<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<RoofTypesView, String>, StoreError> {
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
    let mut listed: Vec<RoofTypeView> = types
        .in_category(ROOFS)
        .into_iter()
        .filter_map(|ty| {
            let layers = layers_of(ty).ok()?;
            Some(RoofTypeView {
                id: ty.id().to_string(),
                name: ty.name().to_owned(),
                thickness: layers.total().to_string(),
                layers: layers_args(&layers),
            })
        })
        .collect();
    listed.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    Ok(Ok(RoofTypesView {
        types: listed,
        materials: materials_view(),
    }))
}
