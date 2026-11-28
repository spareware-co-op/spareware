//! The commands that make and change curtain wall types and curtain walls (job 14.1,
//! CON-A4-F6-P1, piece A84). They go through the same edit log as `element.*`, with the same access
//! check (`model.edit`) and the same claims: a piece another person is working on is refused, and
//! one person's edits land in log order. Each is one entry, so one undo takes back all it wrote
//! ([`crate::cloud::undo`]). They read as the roof commands do ([`crate::cloud::roofs`]).
//!
//! Each command checks the change with construct-curtain's own rules (a type's grid and parts, a
//! wall's lines, segments and panels, every refusal in its words), construct-types' (types) and
//! construct-levels' (ties), and a curtain wall with the very reader and plan the builders use
//! ([`construct_regen::levels::recipe_at`], and the default builder's build of it,
//! [`construct_regen::geometry`], which lays it out and evaluates each door panel's family), so a
//! curtain wall the command takes is one the builders take. An embedded one is fitted in its wall
//! by the kernel builder's own rules for a plain opening ([`construct_regen::kernel::fits`],
//! through [`crate::cloud::hosting`]). A refusal is the command's result and changes nothing.
//! Nothing here reads a clock or a random number: the claim's time is the orderer's.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `curtain.type.create` | `type`, `name`, `spec` | A curtain wall type: of the curtain walls category, its grid and parts in its `@system.curtain` block, listing every parameter bound to curtain walls |
//! | `curtain.type.spec` | `type`, `spec` | The curtain wall type: its grid and parts |
//! | `curtain.type.rename` | `type`, `name` | The type, as `type.rename` |
//! | `curtain.type.duplicate` | `type`, `new`, `name` | A copy of the type, as `type.duplicate` |
//! | `curtain.type.delete` | `type` | Its pointer goes, as `type.delete`: refused while a curtain wall is of it |
//! | `curtain.defaults` | none (`{}`) | Each of construct-curtain's default types the document lacks (by ID, or by name among its curtain wall types); nothing when it lacks none |
//! | `curtain.draw` | `element`, `name` (default the ID), `type`, `start`, `end`, `level`, `offset`, then `top` and `top_offset` or `height` | A new curtain wall on its own line |
//! | `curtain.embed` | `element`, `name`, `type`, `host` (a layered wall), `side` (`exterior` or `interior`, default `exterior`), `along`, `up` (default `0`), `length`, `height` | A new curtain wall embedded in the wall, cutting its own opening there |
//! | `curtain.retype` | `element`, `type` | The curtain wall: its type, as `element.type` |
//! | `curtain.grid.add` | `element`, `direction`, `at` | The curtain wall: a grid line added |
//! | `curtain.grid.remove` | `element`, `direction`, `at` | The curtain wall: a grid line removed, its own or its type's |
//! | `curtain.segment.omit` | `element`, `direction`, `at`, `from` | The curtain wall: a segment left out |
//! | `curtain.segment.restore` | `element`, `direction`, `at`, `from` | The curtain wall: a left-out segment kept again |
//! | `curtain.panel` | `element`, `at` (`[along, up]`), `panel` (`null` for the type's) | The curtain wall: the panel of one cell swapped |
//! | `curtain.delete` | `element` | The curtain wall goes, as `element.delete` |
//!
//! `spec` is a type's grid and parts as a person sends them: `{"vertical":{"layout":"spacing",
//! "spacing":"1.5 m","justify":"centre"},"horizontal":{"layout":"count","count":3},"panel":{"kind":
//! "glazed","thickness":"24 mm","material":"glass-plate"},"mullions":{"interior-vertical":{"width":
//! "50 mm","depth":"150 mm","material":"timber-douglas-fir"}},"crossing":"vertical"}`. A layout is
//! `none`, `spacing` (with `justify` `start`, `centre` or `end`, default `start`), `count` or `most`;
//! a panel `glazed` or `solid` (a `thickness`, an `offset` across, default `0`, and a `material`),
//! `empty`, or `door` (a `family` of the library by its permanent ID and its `type` by its key); a
//! mullion slot left out has none; `crossing` defaults to `vertical`. Every length is typed text.
//!
//! `direction` is `vertical` (a line standing `at` that far along the wall from its start) or
//! `horizontal` (a line `at` that height above its base). `from` names a segment by the crossing it
//! starts at (`0` for the first). `at` of `curtain.panel` is the corner a panel's cell starts at.
//!
//! `curtain.draw`: `start` and `end` are its line in the model's plan, each `[x, y]` as typed text;
//! walking from the start to the end its exterior is on the right. Its base and top are tied as a
//! wall's are (`level`, `offset`, `top`, `top_offset`, or `height`). The record: kind `curtain`,
//! `level` 0, placement 0, its type and category, its ties, and the recipe `construct/wall.curtain`
//! with its line, its height and `curtain` `{"type":"curtain"}`, read from its type when it is
//! built. `curtain.embed`: the curtain wall stands on its host's reference line from `along` its
//! line (from the host's start to the curtain wall's) for `length`, `up` above the host's base and
//! `height` high, hosted on that side as a plain opening is, on its host's level; its recipe holds
//! its `length` and `height`, and its line is its host's (construct-regen's `hosting`), so it moves
//! with its wall. Refused: an element of that ID, a type the document lacks or of another category,
//! a line with no length, a host that is not a layered wall along x or y, an opening that would
//! not fit there (the kernel builder's words), and anything construct-curtain or the builders
//! would refuse, in their words.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` and
//! `hosted.place` of a curtain wall, `element.set` of a curtain wall's recipe, `type.create` of a
//! curtain walls type and `type.rename`, `type.duplicate` or `type.delete` of a curtain wall type
//! are refused, naming the command to use.
//!
//! For the page, without the log: [`read_curtain_types`], a document's curtain wall types with
//! their grid and parts as these commands take them, the materials and the defaults.
//!
//! Typed text is read with construct-params' metric display settings until a project keeps its
//! own.

use crate::cloud::floors::{Answer, Run, length, need, why};
use crate::cloud::hosting::{self, Which, fit};
use crate::cloud::walls::{MaterialView, materials_view};
use crate::cloud::{elements, levels, params};
use construct_categories::{CURTAIN_WALLS, CategoryId};
use construct_curtain::{
    CURTAIN_KIND, Crossing, Curtain, CurtainSpec, Direction, DoorPanel, Edits, Infill, Justify,
    Layout, Mullions, Panel, Profile, Segment, curtain_recipe, default_curtains, embedded_recipe,
    is_curtain, new_curtain_type, spec_of, with_edits, with_spec,
};
use construct_levels::{Length, Tie, base_of, write_base, write_top};
use construct_materials::MaterialId;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::Value;
use construct_regen::hosting as placing;
use construct_regen::levels as shape_levels;
use construct_regen::parameters::{self as records, At, CATEGORY_KEY, Context, TYPE_KIND};
use construct_types::{ElementType, Instance, SYSTEM_KEY_PREFIX, TypeError, Types};
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::{Place, Reader, StoreError, Txn, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 15] = [
    "curtain.type.create",
    "curtain.type.spec",
    "curtain.type.rename",
    "curtain.type.duplicate",
    "curtain.type.delete",
    "curtain.defaults",
    "curtain.draw",
    "curtain.embed",
    "curtain.retype",
    "curtain.grid.add",
    "curtain.grid.remove",
    "curtain.segment.omit",
    "curtain.segment.restore",
    "curtain.panel",
    "curtain.delete",
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
    spec: Option<SpecArgs>,
    start: Option<[String; 2]>,
    end: Option<[String; 2]>,
    /// The base level: its ID ([`levels::tie_of_new`]).
    level: Option<Json>,
    offset: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    top: Option<Id>,
    top_offset: Option<String>,
    height: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    host: Option<Id>,
    side: Option<String>,
    along: Option<String>,
    up: Option<String>,
    length: Option<String>,
    direction: Option<String>,
    /// A grid line's place, or a panel's corner (`[along, up]`).
    at: Option<Json>,
    from: Option<String>,
}

/// A type's grid and parts as a person sends them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecArgs {
    vertical: LayoutArgs,
    horizontal: LayoutArgs,
    panel: PanelArgs,
    #[serde(default)]
    mullions: MullionArgs,
    crossing: Option<Crossing>,
}

/// A layout as a person sends it.
#[derive(Deserialize)]
#[serde(tag = "layout", rename_all = "lowercase", deny_unknown_fields)]
enum LayoutArgs {
    None,
    Spacing {
        spacing: String,
        justify: Option<Justify>,
    },
    Count {
        count: u32,
    },
    Most {
        spacing: String,
    },
}

/// A panel as a person sends it.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum PanelArgs {
    Glazed {
        thickness: String,
        offset: Option<String>,
        material: String,
    },
    Solid {
        thickness: String,
        offset: Option<String>,
        material: String,
    },
    Empty,
    Door {
        family: String,
        #[serde(rename = "type")]
        ty: String,
    },
}

/// The four mullion slots as a person sends them; one left out has none.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct MullionArgs {
    #[serde(rename = "interior-vertical")]
    interior_vertical: Option<ProfileArgs>,
    #[serde(rename = "interior-horizontal")]
    interior_horizontal: Option<ProfileArgs>,
    #[serde(rename = "border-vertical")]
    border_vertical: Option<ProfileArgs>,
    #[serde(rename = "border-horizontal")]
    border_horizontal: Option<ProfileArgs>,
}

/// A rectangular mullion profile as a person sends it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileArgs {
    width: String,
    depth: String,
    offset: Option<String>,
    material: String,
}

/// A material ID, or the refusal.
fn material(text: &str) -> Result<MaterialId, String> {
    MaterialId::parse(text).map_err(why)
}

/// An offset across: typed text, default 0.
fn offset_of(text: Option<&str>) -> Result<Length, String> {
    text.map_or(Ok(Length::from_steps(0)), length)
}

fn layout_from(sent: LayoutArgs) -> Result<Layout, String> {
    Ok(match sent {
        LayoutArgs::None => Layout::None,
        LayoutArgs::Spacing { spacing, justify } => Layout::Spacing {
            spacing: length(&spacing)?,
            justify: justify.unwrap_or(Justify::Start),
        },
        LayoutArgs::Count { count } => Layout::Count { count },
        LayoutArgs::Most { spacing } => Layout::Most {
            spacing: length(&spacing)?,
        },
    })
}

/// A panel from what a person sent: a door's family found in the library by its permanent ID.
fn panel_from(sent: PanelArgs) -> Result<Panel, String> {
    Ok(match sent {
        PanelArgs::Glazed {
            thickness,
            offset,
            material: m,
        } => Panel::Glazed(
            Infill::new(
                length(&thickness)?,
                offset_of(offset.as_deref())?,
                material(&m)?,
            )
            .map_err(why)?,
        ),
        PanelArgs::Solid {
            thickness,
            offset,
            material: m,
        } => Panel::Solid(
            Infill::new(
                length(&thickness)?,
                offset_of(offset.as_deref())?,
                material(&m)?,
            )
            .map_err(why)?,
        ),
        PanelArgs::Empty => Panel::Empty,
        PanelArgs::Door { family, ty } => {
            let library = construct_regen::families::Library::baked();
            let (content, recipe) = library
                .family(&family)
                .ok_or_else(|| format!("There is no family {family} in the library."))?;
            let key = construct_families::Key::parse(&ty)
                .filter(|key| recipe.named_type(key).is_some())
                .ok_or_else(|| format!("Family {family} has no type {ty}."))?;
            Panel::Door(DoorPanel::new(content.as_str(), key.as_str()).map_err(why)?)
        }
    })
}

fn profile_from(sent: Option<ProfileArgs>) -> Result<Option<Profile>, String> {
    sent.map(|p| {
        Profile::rectangle(
            length(&p.width)?,
            length(&p.depth)?,
            offset_of(p.offset.as_deref())?,
            material(&p.material)?,
        )
        .map_err(why)
    })
    .transpose()
}

/// A type's grid and parts from what a person sent, checked by construct-curtain.
fn spec_from(sent: SpecArgs) -> Result<CurtainSpec, String> {
    let mullions = Mullions {
        interior_vertical: profile_from(sent.mullions.interior_vertical)?,
        interior_horizontal: profile_from(sent.mullions.interior_horizontal)?,
        border_vertical: profile_from(sent.mullions.border_vertical)?,
        border_horizontal: profile_from(sent.mullions.border_horizontal)?,
    };
    CurtainSpec::new(
        layout_from(sent.vertical)?,
        layout_from(sent.horizontal)?,
        panel_from(sent.panel)?,
        mullions,
        sent.crossing.unwrap_or(Crossing::Vertical),
    )
    .map_err(why)
}

/// A length as typed text that reads back exactly.
fn typed(l: Length) -> String {
    l.to_string()
}

fn layout_json(layout: Layout) -> Json {
    match layout {
        Layout::None => json!({ "layout": "none" }),
        Layout::Spacing { spacing, justify } => {
            json!({ "layout": "spacing", "spacing": typed(spacing), "justify": justify })
        }
        Layout::Count { count } => json!({ "layout": "count", "count": count }),
        Layout::Most { spacing } => json!({ "layout": "most", "spacing": typed(spacing) }),
    }
}

/// A panel as these commands take it; a door's family by its permanent ID where the library holds
/// it, else by its content id.
fn panel_json(panel: &Panel) -> Json {
    let infill = |kind: &str, i: &Infill| {
        json!({ "kind": kind, "thickness": typed(i.thickness()), "offset": typed(i.offset()),
                "material": i.material().as_str() })
    };
    match panel {
        Panel::Glazed(i) => infill("glazed", i),
        Panel::Solid(i) => infill("solid", i),
        Panel::Empty => json!({ "kind": "empty" }),
        Panel::Door(door) => {
            let library = construct_regen::families::Library::baked();
            let family = library
                .recipes()
                .find(|(content, _)| *content == door.family())
                .map_or_else(
                    || door.family().as_str().to_owned(),
                    |(_, recipe)| recipe.id.as_str().to_owned(),
                );
            json!({ "kind": "door", "family": family, "type": door.ty().as_str() })
        }
    }
}

fn profile_json(profile: Option<&Profile>) -> Option<Json> {
    profile.map(|p| {
        json!({ "width": typed(p.width()), "depth": typed(p.depth()), "offset": typed(p.offset()),
                "material": p.material().as_str() })
    })
}

/// A type's grid and parts as `curtain.type.create` and `curtain.type.spec` take them, each
/// length as typed text that reads back exactly; an empty mullion slot is left out.
#[must_use]
pub fn spec_args(spec: &CurtainSpec) -> Json {
    let m = spec.mullions();
    let mut mullions = serde_json::Map::new();
    for (key, profile) in [
        ("interior-vertical", m.interior_vertical.as_ref()),
        ("interior-horizontal", m.interior_horizontal.as_ref()),
        ("border-vertical", m.border_vertical.as_ref()),
        ("border-horizontal", m.border_horizontal.as_ref()),
    ] {
        if let Some(p) = profile_json(profile) {
            mullions.insert(key.to_owned(), p);
        }
    }
    json!({
        "vertical": layout_json(spec.layout(Direction::Vertical)),
        "horizontal": layout_json(spec.layout(Direction::Horizontal)),
        "panel": panel_json(spec.panel()),
        "mullions": mullions,
        "crossing": spec.crossing(),
    })
}

/// A curtain wall type of the document: a type of the curtain walls category with its grid and
/// parts in its block. The inner error is why not, in construct-types' or construct-curtain's
/// words.
async fn curtain_type<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    context: &Context,
    id: &Id,
) -> Result<Result<ElementType, String>, StoreError> {
    let ty = match records::read_type(txn, project, document, id, context).await? {
        Ok(ty) if ty.category() != CURTAIN_WALLS => {
            let refusal = TypeError::OtherCategory {
                from: CURTAIN_WALLS,
                to: ty.category(),
            };
            return Ok(Err(why(refusal)));
        }
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(spec_of(&ty).map(|_| ty).map_err(why))
}

/// A new type that lists every parameter bound to curtain walls, as `type.create` makes one.
fn listed(ty: ElementType, context: &Context) -> Result<ElementType, String> {
    let mut ty = ty;
    for applicable in context
        .table
        .parameters_of(&context.categories, CURTAIN_WALLS)
    {
        let parameter = applicable.definition.id();
        if !ty.has(parameter) {
            ty.add_parameter(&context.params, parameter).map_err(why)?;
        }
    }
    Ok(ty)
}

/// The document's current curtain wall record, or the refusal: it is gone, or it is not one.
async fn curtain_record_of<T: Txn>(
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
    Ok(if is_curtain(&record.recipe) {
        Ok(record)
    } else {
        Err(format!("{} is not a curtain wall.", record.name))
    })
}

/// The recipe a curtain wall's own builder is handed, as the regenerator hands it: filled, and an
/// embedded one seated on its host's line. The inner error is the refusal, after its name.
async fn seated<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
) -> Result<Result<Recipe, String>, StoreError> {
    let named = |message: String| format!("{}: {message}", record.name);
    let filled =
        match shape_levels::recipe_at(run.txn, run.project, run.document, record, At::Now).await? {
            Ok(filled) => filled,
            Err(failed) => return Ok(Err(named(failed.message))),
        };
    if !construct_curtain::is_embedded(&filled) {
        return Ok(Ok(filled));
    }
    let hosting = match placing::read(record) {
        Ok(Some(hosting)) => hosting,
        Ok(None) => return Ok(Err(named("it is embedded in no wall".to_owned()))),
        Err(why) => return Ok(Err(why)),
    };
    let host = match hosting::host_for(
        run.txn,
        run.project,
        run.document,
        hosting.host(),
        Which::Name(hosting.face()),
        "a curtain wall",
    )
    .await?
    {
        Ok(host) => host,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(placing::seat(
        &host.edition,
        &host.record,
        &host.recipe,
        record,
        &filled,
        &hosting,
    )
    .map(|(_, seated)| seated)
    .map_err(|e| e.message))
}

/// What the builders would make of `record`, read the way they read it: built by the default
/// builder ([`construct_regen::geometry`]), which lays it out and evaluates every door panel's
/// family. The inner error is the refusal, after its name.
async fn buildable<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
) -> Result<Result<(), String>, StoreError> {
    let recipe = match seated(run, record).await? {
        Ok(recipe) => recipe,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(construct_regen::geometry()
        .build(&recipe)
        .map(|_| ())
        .map_err(|failed| format!("{}: {}", record.name, failed.message)))
}

/// A curtain wall's layout inputs, from its record as it is now: its length, height, type's grid
/// and parts, and own edits. The inner error is the refusal, after its name.
async fn curtain_of<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
) -> Result<Result<Curtain, String>, StoreError> {
    let filled =
        match shape_levels::recipe_at(run.txn, run.project, run.document, record, At::Now).await? {
            Ok(filled) => filled,
            Err(failed) => return Ok(Err(format!("{}: {}", record.name, failed.message))),
        };
    Ok(construct_regen::curtains::curtain_of(&filled).map_err(|e| format!("{}: {e}", record.name)))
}

/// A direction from its word.
fn direction_of(text: Option<&str>) -> Result<Direction, String> {
    let text = need(text, "a direction (vertical or horizontal)")?;
    Direction::from_word(text)
        .ok_or_else(|| format!("{text:?} is not a direction: vertical or horizontal."))
}

/// A typed length from a JSON argument that must be text.
fn length_arg(sent: Option<&Json>, what: &str) -> Result<Length, String> {
    let text = need(sent, what)?
        .as_str()
        .ok_or_else(|| format!("{what} is typed text, such as \"1.5 m\"."))?;
    length(text)
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
    // Renaming, copying and deleting a curtain wall type are the type commands' own, once it is
    // known to be one: the same rules, the same words.
    if let Some(as_type) = edit.cmd.strip_prefix("curtain.")
        && matches!(as_type, "type.rename" | "type.duplicate" | "type.delete")
    {
        let id = match need(args.type_id, "a curtain wall type") {
            Ok(id) => id,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if let Err(refusal) = curtain_type(txn, project, &edit.target, &document, &id).await? {
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
        "curtain.type.create" => create_type(&mut run, &document, args).await,
        "curtain.type.spec" => set_spec(&mut run, &document, args).await,
        "curtain.defaults" => defaults(&mut run, &document).await,
        "curtain.draw" => draw(&mut run, &document, args).await,
        "curtain.embed" => embed(&mut run, &document, args).await,
        "curtain.retype" | "curtain.delete" => delegate(&mut run, &document, context, args).await,
        "curtain.grid.add"
        | "curtain.grid.remove"
        | "curtain.segment.omit"
        | "curtain.segment.restore"
        | "curtain.panel" => change_grid(&mut run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// `curtain.retype` and `curtain.delete`: the curtain wall checked as one, then the generic command
/// of the same job runs, with the same rules and the same words.
async fn delegate<T: Txn>(
    run: &mut Run<'_, T>,
    document: &Context,
    context: &Ordered<'_>,
    args: Args,
) -> Result<Outcome, StoreError> {
    if let Err(refusal) = curtain_record_of(run, args.element.clone()).await? {
        return Ok(Err(refusal));
    }
    let mut generic = run.edit.clone();
    if run.edit.cmd == "curtain.retype" {
        let id = match need(args.type_id, "a curtain wall type") {
            Ok(id) => id,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if let Err(refusal) =
            curtain_type(run.txn, run.project, run.document, document, &id).await?
        {
            return Ok(Err(refusal));
        }
        "element.type".clone_into(&mut generic.cmd);
        return params::apply(run.txn, run.project, context, &generic, run.claim_ms).await;
    }
    "element.delete".clone_into(&mut generic.cmd);
    elements::apply(run.txn, run.project, context, &generic, run.claim_ms).await
}

async fn create_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = (|| {
        let id = need(args.type_id, "a curtain wall type")?;
        let name = need(args.name, "a name")?;
        let spec = spec_from(need(args.spec, "the grid and parts")?)?;
        Ok::<_, String>((id, name, spec))
    })();
    let (id, name, spec) = match sent {
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let made = new_curtain_type(id, &name, &spec, &context.categories)
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

async fn set_spec<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let sent = (|| {
        let id = need(args.type_id, "a curtain wall type")?;
        let spec = spec_from(need(args.spec, "the grid and parts")?)?;
        Ok::<_, String>((id, spec))
    })();
    let (id, spec) = match sent {
        Ok(sent) => sent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let ty = match curtain_type(run.txn, run.project, run.document, context, &id).await? {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = match with_spec(ty, &spec) {
        Ok(changed) => changed,
        Err(e) => return Ok(Err(why(e))),
    };
    run.put_claimed(&changed.to_record(run.document)).await
}

/// Each default curtain wall type the document lacks: none of its ID is in the document and no
/// curtain wall type of the document has its name. None lacking writes nothing, which is no step.
async fn defaults<T: Txn>(run: &mut Run<'_, T>, context: &Context) -> Answer {
    let defaults = match default_curtains() {
        Ok(defaults) => defaults,
        Err(e) => return Ok(Err(why(e))),
    };
    let mut types: Types = match run.types(context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut made = Vec::new();
    for default in defaults {
        let named = types
            .in_category(CURTAIN_WALLS)
            .iter()
            .any(|other| other.name() == default.name);
        if named || run.current(&default.id).await?.is_some() {
            continue;
        }
        let ty = new_curtain_type(default.id, default.name, &default.spec, &context.categories)
            .map_err(why)
            .and_then(|ty| listed(ty, context));
        let ty = match ty {
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

/// A point from two typed lengths.
fn point(sent: Option<[String; 2]>, what: &str) -> Result<[Length; 2], String> {
    let [x, y] = need(sent, what)?;
    Ok([length(&x)?, length(&y)?])
}

/// A new curtain wall's record of type `ty`: its type's values and its category in `props`, and
/// `recipe`.
fn new_record(id: &Id, document: &Id, name: &str, ty: &ElementType, recipe: Recipe) -> Element {
    let mut props = Instance::new(ty).to_props();
    props.insert(
        CATEGORY_KEY.to_owned(),
        records::write_category(CURTAIN_WALLS),
    );
    Element {
        id: id.clone(),
        document: document.clone(),
        kind: CURTAIN_KIND.to_owned(),
        name: name.to_owned(),
        // On no level by number: the tie in `props` is its level.
        level: 0,
        // Its line is in the model's plan, so its frame is the model's.
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe,
        props,
        references: Vec::new(),
    }
}

/// The ID, name and type of a new curtain wall. The inner error is the refusal.
async fn new_of<T: Txn>(
    run: &mut Run<'_, T>,
    context: &Context,
    args: &Args,
) -> Result<Result<(Id, String, ElementType), String>, StoreError> {
    let id = match need(args.element.clone(), "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let name = args.name.clone().unwrap_or_else(|| id.to_string());
    let type_id = match need(args.type_id.clone(), "a curtain wall type") {
        Ok(type_id) => type_id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(
        curtain_type(run.txn, run.project, run.document, context, &type_id)
            .await?
            .map(|ty| (id, name, ty)),
    )
}

/// Writes a new record, as the command's whole result.
async fn create<T: Txn>(run: &mut Run<'_, T>, record: &Element) -> Answer {
    let hash = elements::write_record(run.txn, run.project, record).await?;
    let pointer = Place::Piece {
        project: run.project.clone(),
        document: run.document.clone(),
        piece: record.id.clone(),
    };
    run.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: record.id.clone(),
            before: None,
            after: Some(hash),
        }],
        ..Applied::default()
    }))
}

async fn draw<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let (id, name, ty) = match new_of(run, context, &args).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let line = (|| {
        let start = point(args.start.clone(), "a start")?;
        let end = point(args.end.clone(), "an end")?;
        if start == end {
            return Err(format!(
                "{name}'s start and end are the same point: a curtain wall has a length."
            ));
        }
        Ok::<_, String>((start, end))
    })();
    let (start, end) = match line {
        Ok(line) => line,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut props = BTreeMap::new();
    let height = match ties(run, &args, &name, &mut props).await? {
        Ok(height) => height,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let recipe = match curtain_recipe(start, end, height, &Edits::new()) {
        Ok(recipe) => recipe,
        Err(e) => return Ok(Err(why(e))),
    };
    let mut record = new_record(&id, run.document, &name, &ty, recipe);
    record.props.extend(props);
    if let Err(refusal) =
        levels::top_is_above_base(run.txn, run.project, run.document, &record).await?
    {
        return Ok(Err(refusal));
    }
    if let Err(refusal) = buildable(run, &record).await? {
        return Ok(Err(refusal));
    }
    create(run, &record).await
}

/// A new curtain wall's ties, written into `props`: its base on a level or on none, and its top on
/// a level; and its recipe's height, `{"tie":"top"}` or the stored length it was given. The inner
/// error is the refusal, in words.
async fn ties<T: Txn>(
    run: &mut Run<'_, T>,
    args: &Args,
    name: &str,
    props: &mut BTreeMap<String, String>,
) -> Result<Result<Json, String>, StoreError> {
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
                    shape_levels::top_arg()
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
        (Some(_), Some(_), _) => Err(
            "A curtain wall's top is tied to a level or given as a height, not both.".to_owned(),
        ),
        (None, None, _) => {
            Err("A curtain wall needs its top tied to a level or a height.".to_owned())
        }
    })
}

async fn embed<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let (id, name, ty) = match new_of(run, context, &args).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let sizes = (|| {
        Ok::<_, String>((
            need(args.host.clone(), "a wall to embed it in")?,
            length(need(
                args.along.as_deref(),
                "where it starts along its wall",
            )?)?,
            length(args.up.as_deref().unwrap_or("0"))?,
            length(need(args.length.as_deref(), "a length")?)?,
            length(need(args.height.as_deref(), "a height")?)?,
        ))
    })();
    let (host_id, along, up, own, height) = match sizes {
        Ok(sizes) => sizes,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let host = match hosting::host_for(
        run.txn,
        run.project,
        run.document,
        &host_id,
        Which::Role(args.side.as_deref().unwrap_or("exterior")),
        "a curtain wall",
    )
    .await?
    {
        Ok(host) => host,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let recipe = match embedded_recipe(own, height, &Edits::new()) {
        Ok(recipe) => recipe,
        Err(e) => return Ok(Err(format!("{name}: {e}"))),
    };
    let mut record = new_record(&id, run.document, &name, &ty, recipe);
    // It stands on its host's level, as a plain opening does.
    match base_of(&host.record) {
        Ok(base) => write_base(&mut record.props, base.as_ref()),
        Err(e) => return Ok(Err(why(e))),
    }
    placing::write(&mut record, &host_id, &host.face, along, up);
    if let Err(refusal) = fit(run.txn, run.project, run.document, &host, &record).await? {
        return Ok(Err(refusal));
    }
    if let Err(refusal) = buildable(run, &record).await? {
        return Ok(Err(refusal));
    }
    create(run, &record).await
}

/// The grid and panel commands: the wall's edits changed by construct-curtain's own edit, checked
/// as the builders take it, and written.
async fn change_grid<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let record = match curtain_record_of(run, args.element.clone()).await? {
        Ok(record) => record,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let curtain = match curtain_of(run, &record).await? {
        Ok(curtain) => curtain,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = (|| {
        let line = || -> Result<(Direction, Length), String> {
            Ok((
                direction_of(args.direction.as_deref())?,
                length_arg(args.at.as_ref(), "where the grid line stands")?,
            ))
        };
        let segment = || -> Result<Segment, String> {
            let (direction, at) = line()?;
            let from = length(need(
                args.from.as_deref(),
                "the crossing the segment starts at",
            )?)?;
            Ok(Segment {
                direction,
                at,
                from,
            })
        };
        let next = match run.edit.cmd.as_str() {
            "curtain.grid.add" => {
                let (direction, at) = line()?;
                curtain.with_line(direction, at)
            }
            "curtain.grid.remove" => {
                let (direction, at) = line()?;
                curtain.without_line(direction, at)
            }
            "curtain.segment.omit" => curtain.with_segment_omitted(segment()?),
            "curtain.segment.restore" => curtain.with_segment_restored(segment()?),
            _ => {
                let corner = need(args.at.as_ref(), "the corner the panel's cell starts at")?;
                let pair = corner
                    .as_array()
                    .filter(|pair| pair.len() == 2)
                    .ok_or("A panel's corner is [along, up], each typed text.")?;
                let at = [
                    length_arg(pair.first(), "along")?,
                    length_arg(pair.get(1), "up")?,
                ];
                // Read from the edit itself, where a panel left out and one written null differ.
                let sent = need(
                    run.edit.args.get("panel").cloned(),
                    "a panel, or null for its type's",
                )?;
                let panel = if sent.is_null() {
                    None
                } else {
                    let sent: PanelArgs = serde_json::from_value(sent)
                        .map_err(|e| format!("That panel isn't valid: {e}"))?;
                    Some(panel_from(sent)?)
                };
                curtain.with_panel(at, panel)
            }
        };
        next.map_err(|e| format!("{}: {e}", record.name))
    })();
    let changed = match changed {
        Ok(changed) => changed,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut next = record.clone();
    next.recipe = match with_edits(&record.recipe, changed.edits()) {
        Ok(recipe) => recipe,
        Err(e) => return Ok(Err(why(e))),
    };
    if let Err(refusal) = buildable(run, &next).await? {
        return Ok(Err(refusal));
    }
    run.put_claimed(&next).await
}

/// The words for a curtain command that should have been used.
fn use_instead(what: &str, command: &str) -> String {
    format!("{what} with {command}, which checks it as a curtain wall.")
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

/// Whether a record is a curtain wall type: of the curtain walls category, holding its grid and
/// parts in its block.
fn is_curtain_type(record: &Element) -> bool {
    record.kind == TYPE_KIND
        && records::stored_category(record) == Some(CURTAIN_WALLS)
        && record
            .props
            .contains_key(&format!("{SYSTEM_KEY_PREFIX}{}", construct_curtain::OWNER))
}

/// The curtain wall rules of the generic commands, asked before one runs, as the wall rules are
/// ([`crate::cloud::walls::generic_rules`]): a curtain wall is drawn by `curtain.draw` or
/// `curtain.embed`, not made by `element.create` or `hosted.place` nor its recipe changed by
/// `element.set`; a
/// curtain wall type is made by `curtain.type.create`, not `type.create`, and renamed, copied and
/// deleted by `curtain.type.rename`, `curtain.type.duplicate` and `curtain.type.delete`. The inner
/// error is the refusal, naming the command to use. An edit these rules have nothing to say of is
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
    let document = &edit.target;
    let a_curtain_recipe = args.recipe.as_ref().is_some_and(is_curtain);
    Ok(match edit.cmd.as_str() {
        "element.create" if a_curtain_recipe => {
            Err(use_instead("A curtain wall is drawn", "curtain.draw"))
        }
        "hosted.place" if a_curtain_recipe => Err(use_instead(
            "A curtain wall is embedded in a wall",
            "curtain.embed",
        )),
        "element.set" if args.recipe.is_some() => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record)) if is_curtain(&record.recipe) => Err(use_instead(
                    &format!("{}'s grid is changed", record.name),
                    "the curtain.grid, curtain.segment and curtain.panel commands",
                )),
                Some(_) if a_curtain_recipe => {
                    Err(use_instead("A curtain wall is drawn", "curtain.draw"))
                }
                _ => Ok(()),
            }
        }
        "type.create" if args.category == Some(CURTAIN_WALLS) => Err(use_instead(
            "A curtain wall type is made",
            "curtain.type.create",
        )),
        cmd @ ("type.rename" | "type.duplicate" | "type.delete") => {
            let Some(id) = &args.type_id else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, document, id).await? {
                Some((_, record)) if is_curtain_type(&record) => {
                    let what = match cmd {
                        "type.rename" => "A curtain wall type is renamed",
                        "type.duplicate" => "A curtain wall type is copied",
                        _ => "A curtain wall type is deleted",
                    };
                    Err(use_instead(what, &format!("curtain.{cmd}")))
                }
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// The `curtain.type.create` arguments that make construct-curtain's default types, in their
/// order, each under its permanent ID.
///
/// # Errors
/// None for this fixed data; construct-curtain checks each value, in its words.
pub fn default_type_args() -> Result<Vec<Json>, String> {
    Ok(default_curtains()
        .map_err(why)?
        .iter()
        .map(|d| json!({ "type": d.id.as_str(), "name": d.name, "spec": spec_args(&d.spec) }))
        .collect())
}

/// A document's curtain wall types, as the page lists them for its curtain wall tool.
#[derive(Serialize, Debug)]
pub struct CurtainTypesView {
    /// Every type of the curtain walls category with its grid and parts, by name, then ID.
    pub types: Vec<CurtainTypeView>,
    /// The materials a panel or a mullion may be made of, each with the colours a view draws it
    /// in.
    pub materials: Vec<MaterialView>,
    /// [`default_type_args`].
    pub defaults: Vec<Json>,
}

/// One curtain wall type.
#[derive(Serialize, Debug)]
pub struct CurtainTypeView {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its grid and parts, as [`spec_args`] gives them.
    pub spec: Json,
}

/// A document's curtain wall types as they are now, the materials (construct-materials' default
/// library until a project keeps its own) and the defaults. The inner error is a record the
/// document's rules can't read, in their words.
///
/// # Errors
/// A store error.
pub async fn read_curtain_types<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<CurtainTypesView, String>, StoreError> {
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
    let mut listed: Vec<CurtainTypeView> = types
        .in_category(CURTAIN_WALLS)
        .into_iter()
        .filter_map(|ty| {
            let spec = spec_of(ty).ok()?;
            Some(CurtainTypeView {
                id: ty.id().to_string(),
                name: ty.name().to_owned(),
                spec: spec_args(&spec),
            })
        })
        .collect();
    listed.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let defaults = match default_type_args() {
        Ok(defaults) => defaults,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(Ok(CurtainTypesView {
        types: listed,
        materials: materials_view(),
        defaults,
    }))
}

/// Where an embedded curtain wall stands and what it cuts, as the regenerator seats it
/// ([`construct_regen::hosting`]): what a page needs to build it and its host, and a drawing to
/// cut it, without working out where a wall's line is itself.
#[derive(Clone, Debug)]
pub struct Seat {
    /// Where its record stands in the model: its host's placement and its own height above it.
    pub placement: Placement,
    /// Its filled recipe with its line written in (the host's reference line, from where it
    /// starts for its length).
    pub recipe: Recipe,
    /// What it cuts in its host: where the opening is in the host's frame, and its recipe
    /// (`opening.wall`).
    pub cut: (Placement, Recipe),
}

/// The seat of the curtain wall `record`, whose filled recipe is `filled`: none for a curtain wall
/// that is not embedded. The inner error is why it cannot be seated, in words.
///
/// # Errors
/// A store error.
pub async fn read_seat<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    record: &Element,
    filled: &Recipe,
) -> Result<Result<Option<Seat>, String>, StoreError> {
    if !construct_curtain::is_embedded(filled) {
        return Ok(Ok(None));
    }
    let hosting = match placing::read(record) {
        Ok(Some(hosting)) => hosting,
        Ok(None) => return Ok(Err("it is hosted on no wall".to_owned())),
        Err(e) => return Ok(Err(e)),
    };
    let Some((_, host)) = records::read_element(reader, project, document, hosting.host()).await?
    else {
        return Ok(Err(format!(
            "its wall {} is not in this model",
            hosting.host()
        )));
    };
    let host_recipe =
        match shape_levels::recipe_at(reader, project, document, &host, At::Now).await? {
            Ok(recipe) => recipe,
            Err(e) => return Ok(Err(e.message)),
        };
    let place = construct_model::keys::Place::Model {
        project: project.clone(),
        model: document.clone(),
    };
    let model: Option<construct_model::rows::Model> =
        kit_store::read_row(reader, &place.key()).await?;
    let Some(model) = model else {
        return Ok(Err("this document has no edition".to_owned()));
    };
    let edition = model.edition;
    let seated = placing::seat(&edition, &host, &host_recipe, record, filled, &hosting)
        .and_then(|(at, recipe)| Ok((placing::in_model(host.placement, at)?, recipe)));
    let (placement, recipe) = match seated {
        Ok(found) => found,
        Err(e) => return Ok(Err(e.message)),
    };
    let cut = match placing::cut_of(&edition, &host, &host_recipe, record, filled, &hosting) {
        Ok(Some(cut)) => cut,
        Ok(None) => return Ok(Err("it cuts no opening".to_owned())),
        Err(e) => return Ok(Err(e.message)),
    };
    Ok(Ok(Some(Seat {
        placement,
        recipe,
        cut,
    })))
}
