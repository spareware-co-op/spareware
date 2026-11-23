//! The commands of openings and shafts (job 4.2, CON-A4-F3-P2, piece A66): a hole through a wall,
//! a hole through a floor, and a shaft through every floor in its reach, made, changed in size,
//! place or reach, and deleted. They go through the same edit log as `element.*`, with the same
//! access check (`model.edit`) and the same claims, and each is one entry, so one undo takes back
//! all it wrote ([`crate::cloud::undo`]). How an opening sits in its record is construct-openings'
//! and construct-regen's [`hosting`] (a wall opening is hosted on a side of its wall as a door
//! is; a floor opening on its floor's top face, drawn in its plan), and every fit is the builders'
//! own rules asked without the engine, in their words, so a model drawn on the stand-in holds
//! nothing the kernel's builder would refuse.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `opening.place` | `element`, `host` (a wall), `side` (`exterior` or `interior`, default `exterior`), `along` (from the wall's start to the opening's near edge), `sill` (default 0), `width`, `height`, `name` (default the id) | A new wall opening, hosted on that side of the wall |
//! | `opening.draw` | `element`, `host` (a floor), `outline` (corners `[x, y]` as typed lengths, in the plan), `name` | A new floor opening, hosted on the floor's top |
//! | `opening.size` | `element`, `width` and/or `height` | The wall opening: its size |
//! | `opening.move` | `element`, and any of `along`, `sill`, `host`, `side` | The wall opening: its place on its wall, or on another wall |
//! | `opening.outline` | `element`, `outline` | The floor opening: its outline (its size and place) |
//! | `opening.delete` | `element` | Its pointer goes, and its claim |
//! | `shaft.create` | `element`, `outline`, `level` (its base's level), `offset` (default 0), `top` (its top's level), `top_offset` (default 0), `name` | A new shaft, tied to its two levels |
//! | `shaft.outline` | `element`, `outline` | The shaft: its outline (its size and place) |
//! | `shaft.reach` | `element`, and any of `level`, `offset`, `top`, `top_offset` | The shaft: where it starts and ends |
//! | `shaft.delete` | `element` | Its pointer goes, and its claim |
//!
//! Each is refused, as its result and in words, when the wall or floor is missing or is not one, a
//! length is not one, the element is not of the kind the command is for (the words name the
//! command that is), another person is working on it, or the opening or shaft would not fit. A
//! wall opening's fit is the kernel builder's own words naming it (`"opening O-1: it crosses the
//! wall's end"`); a floor opening's or shaft's is construct-openings' (`"opening O-1: it crosses
//! the edge of floor F-1"`), asked of every floor it would cut with every other opening and shaft
//! of the document, so a shaft that would end inside a floor's thickness, or lie over another
//! opening, is refused where it is made and not as a floor that fails to build. Nothing is
//! written by a refused command.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of an opening
//! or a shaft and `element.set`, `element.move` and `element.delete` of one are refused, naming
//! the command to use; `hosted.place` and `hosted.move` are refused the same way. A floor that
//! holds an opening is not deleted (the hosting rules, [`crate::cloud::hosting`]), and a wall or
//! floor changed under one is built as failed, naming it, as under a door. A shaft's base and top
//! are ties to levels, so a level that one is tied to is not deleted.
//!
//! For the page's properties, without the log: [`floor_cuts`], the outlines that cut a floor, from
//! which a floor's area, volume and mass after the cut are worked out
//! ([`construct_regen::parameters::Context::view_cut`]). Nothing here reads a clock or a random
//! number: the claim's time is the orderer's.

use crate::cloud::elements::write_record;
use crate::cloud::families::{claim_on, rewrite};
use crate::cloud::hosting::{self, Host, Which, fit, length};
use crate::cloud::levels;
use construct_levels::{Tie, base_of, top_of, write_base, write_top};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_openings::{
    Cutter, Named, Outline, Reach, Shaft, WallOpening, fit_in_floor, floor_opening_recipe,
    floor_opening_record, is_floor_opening, is_shaft, is_void, is_wall_opening, outline_of, reach,
    shaft_recipe, shaft_record, wall_opening_record,
};
use construct_params::Length;
use construct_regen::floor_plan::FloorPlan;
use construct_regen::hosting::{self as placing, Hosting};
use construct_regen::levels as shape_levels;
use construct_regen::parameters::{self as records, At};
use construct_regen::wall_plan::{self, Layered};
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::{Place, Reader, StoreError, Txn, to_row};
use kit_sync::{Applied, Context, Outcome, claims};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 10] = [
    PLACE,
    DRAW,
    SIZE,
    MOVE,
    OUTLINE,
    DELETE,
    SHAFT_CREATE,
    SHAFT_OUTLINE,
    SHAFT_REACH,
    SHAFT_DELETE,
];
/// Makes an opening in a wall.
pub const PLACE: &str = "opening.place";
/// Makes an opening in a floor.
pub const DRAW: &str = "opening.draw";
/// Changes a wall opening's size.
pub const SIZE: &str = "opening.size";
/// Moves a wall opening along its wall, or to another wall.
pub const MOVE: &str = "opening.move";
/// Changes a floor opening's outline.
pub const OUTLINE: &str = "opening.outline";
/// Deletes an opening.
pub const DELETE: &str = "opening.delete";
/// Makes a shaft.
pub const SHAFT_CREATE: &str = "shaft.create";
/// Changes a shaft's outline.
pub const SHAFT_OUTLINE: &str = "shaft.outline";
/// Changes where a shaft starts and ends.
pub const SHAFT_REACH: &str = "shaft.reach";
/// Deletes a shaft.
pub const SHAFT_DELETE: &str = "shaft.delete";

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
    #[serde(default, with = "kit_ops::id::option")]
    host: Option<Id>,
    side: Option<String>,
    along: Option<String>,
    sill: Option<String>,
    width: Option<String>,
    height: Option<String>,
    outline: Option<Vec<[String; 2]>>,
    /// The level a shaft's base stands on: its ID ([`levels::tie_of_new`]).
    level: Option<Json>,
    offset: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    top: Option<Id>,
    top_offset: Option<String>,
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

/// A refusal or a store failure, so the first can be returned with `?` and the second passed on.
type Checked<T> = Result<Result<T, String>, StoreError>;

/// What kind of opening or shaft a record is, for a refusal that names the command to use.
fn what_to_do(record: &Element) -> String {
    let name = &record.name;
    if is_wall_opening(&record.recipe) {
        format!(
            "{name} is an opening in a wall: change its size with opening.size, move it with \
             opening.move, delete it with opening.delete."
        )
    } else if is_floor_opening(&record.recipe) {
        format!(
            "{name} is an opening in a floor: draw it again with opening.outline, delete it with \
             opening.delete."
        )
    } else if is_shaft(&record.recipe) {
        format!(
            "{name} is a shaft: change its outline with shaft.outline, where it starts and ends \
             with shaft.reach, delete it with shaft.delete."
        )
    } else {
        format!("{name} is not an opening or a shaft.")
    }
}

/// Whether `record` is the kind `cmd` is for.
fn is_for(cmd: &str, record: &Element) -> bool {
    match cmd {
        SIZE | MOVE => is_wall_opening(&record.recipe),
        OUTLINE => is_floor_opening(&record.recipe),
        DELETE => is_wall_opening(&record.recipe) || is_floor_opening(&record.recipe),
        SHAFT_OUTLINE | SHAFT_REACH | SHAFT_DELETE => is_shaft(&record.recipe),
        _ => false,
    }
}

/// Runs an `opening.*` or `shaft.*` command.
///
/// # Errors
///
/// A store error; a command that cannot be made is the outcome's error instead.
pub async fn apply<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
    claim_ms: u64,
) -> Answer {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    match edit.cmd.as_str() {
        PLACE => return place(txn, project, edit, args).await,
        DRAW => return draw(txn, project, edit, args).await,
        SHAFT_CREATE => return make_shaft(txn, project, edit, args).await,
        _ => {}
    }
    let Some(id) = args.element.clone() else {
        return Ok(Err("Which element?".to_owned()));
    };
    let document = &edit.target;
    let Some((hash, before)) = records::read_element(txn, project, document, &id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    if !is_for(&edit.cmd, &before) {
        return Ok(Err(what_to_do(&before)));
    }
    let claim = match claim_on(txn, project, context, edit, &id, claim_ms).await? {
        Ok(claim) => claim,
        Err(why) => return Ok(Err(why)),
    };
    if matches!(edit.cmd.as_str(), DELETE | SHAFT_DELETE) {
        let pointer = Place::Piece {
            project: project.clone(),
            document: document.clone(),
            piece: id.clone(),
        };
        txn.delete(pointer.key()).await?;
        let claim = claims::release(txn, project, document, &id).await?;
        return Ok(Ok(Applied {
            changes: vec![Change {
                piece: id,
                before: Some(hash),
                after: None,
            }],
            claims: vec![claim],
            ..Applied::default()
        }));
    }
    let after = match edit.cmd.as_str() {
        SIZE => resize(txn, project, document, &before, &args).await?,
        MOVE => move_to(txn, project, document, &before, &args).await?,
        OUTLINE | SHAFT_OUTLINE => redraw(txn, project, document, &before, &args).await?,
        SHAFT_REACH => reach_of(txn, project, document, &before, &args).await?,
        other => Err(format!("Unknown command {other}.")),
    };
    match after {
        Ok(after) => rewrite(txn, project, document, hash, &after, claim).await,
        Err(why) => Ok(Err(why)),
    }
}

/// Why a new element `id` cannot be made, if it cannot.
async fn refused_id<T: Txn>(txn: &mut T, project: &Id, document: &Id, id: &Id) -> Checked<()> {
    if records::read_element(txn, project, document, id)
        .await?
        .is_some()
    {
        return Ok(Err("That element already exists.".to_owned()));
    }
    if id.as_str() == records::TABLE_ID {
        return Ok(Err(
            "Types and the parameter table are changed by their own commands, not as elements."
                .to_owned(),
        ));
    }
    Ok(Ok(()))
}

/// A typed length, as every command here reads one.
fn typed(what: &str, text: &str) -> Result<Length, String> {
    length(what, text)
}

/// Whether a wall's line runs along y: the plan axis its openings are cut along. A wall at an
/// angle has none; the builders refuse an opening in it, in their words.
fn along_y(recipe: &construct_model::geometry::Recipe) -> bool {
    if wall_plan::is_layered(recipe) {
        Layered::of(recipe)
            .ok()
            .and_then(|wall| wall.plan.axis())
            .is_some_and(|axis| axis.index() == 1)
    } else {
        recipe
            .args
            .get("axis")
            .and_then(Json::as_str)
            .is_some_and(|axis| axis == "y")
    }
}

/// Puts `element` on its host's level, as its host's base is tied.
fn on_level_of(element: &mut Element, host: &Element) -> Result<(), String> {
    let base = base_of(host).map_err(|e| e.to_string())?;
    write_base(&mut element.props, base.as_ref());
    Ok(())
}

/// An outline from what a person sent: each corner's typed text read as a length, then checked by
/// construct-openings (the floor's own loop checks), naming `key`.
fn outline_from(sent: &[[String; 2]], named: &Named) -> Result<Outline, String> {
    let mut corners = Vec::with_capacity(sent.len());
    for [x, y] in sent {
        corners.push([
            typed("A corner of the outline", x)?,
            typed("A corner of the outline", y)?,
        ]);
    }
    Outline::new(named, corners).map_err(|e| e.to_string())
}

/// Writes a new record, as the command's whole result.
async fn create<T: Txn>(txn: &mut T, project: &Id, document: &Id, element: &Element) -> Answer {
    let hash = write_record(txn, project, element).await?;
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: element.id.clone(),
    };
    txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: element.id.clone(),
            before: None,
            after: Some(hash),
        }],
        ..Applied::default()
    }))
}

/// `opening.place`: a new opening in a wall.
async fn place<T: Txn>(txn: &mut T, project: &Id, edit: &Edit, args: Args) -> Answer {
    let document = &edit.target;
    let (Some(id), Some(host_id), Some(along), Some(width), Some(height)) =
        (args.element, args.host, args.along, args.width, args.height)
    else {
        return Ok(Err(
            "An opening in a wall needs an id, a wall, a position along it, a width and a height."
                .to_owned(),
        ));
    };
    if let Err(why) = refused_id(txn, project, document, &id).await? {
        return Ok(Err(why));
    }
    let sizes = (|| {
        Ok::<_, String>((
            typed("The position along the wall", &along)?,
            typed("The sill height", args.sill.as_deref().unwrap_or("0"))?,
            typed("The width", &width)?,
            typed("The height", &height)?,
        ))
    })();
    let (along, sill, width, height) = match sizes {
        Ok(sizes) => sizes,
        Err(why) => return Ok(Err(why)),
    };
    let host = match hosting::host_for(
        txn,
        project,
        document,
        &host_id,
        Which::Role(args.side.as_deref().unwrap_or("exterior")),
        "an opening",
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let opening = match WallOpening::new(
        &Named::opening(id.as_str()),
        width,
        height,
        along_y(&host.recipe),
    ) {
        Ok(opening) => opening,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let name = args.name.unwrap_or_else(|| id.to_string());
    let mut element = wall_opening_record(&id, document, &name, &opening);
    if let Err(why) = on_level_of(&mut element, &host.record) {
        return Ok(Err(why));
    }
    placing::write(&mut element, &host_id, &host.face, along, sill);
    element.placement = match fit(txn, project, document, &host, &element).await? {
        Ok(placement) => placement,
        Err(why) => return Ok(Err(why)),
    };
    create(txn, project, document, &element).await
}

/// `opening.draw`: a new opening in a floor.
async fn draw<T: Txn>(txn: &mut T, project: &Id, edit: &Edit, args: Args) -> Answer {
    let document = &edit.target;
    let (Some(id), Some(host_id), Some(sent)) = (args.element, args.host, args.outline) else {
        return Ok(Err(
            "An opening in a floor needs an id, a floor and an outline.".to_owned(),
        ));
    };
    if let Err(why) = refused_id(txn, project, document, &id).await? {
        return Ok(Err(why));
    }
    let outline = match outline_from(&sent, &Named::opening(id.as_str())) {
        Ok(outline) => outline,
        Err(why) => return Ok(Err(why)),
    };
    let host = match hosting::floor_for(txn, project, document, &host_id).await? {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let name = args.name.unwrap_or_else(|| id.to_string());
    let mut element = floor_opening_record(&id, document, &name, &outline);
    if let Err(why) = on_level_of(&mut element, &host.record) {
        return Ok(Err(why));
    }
    placing::write_in_plan(&mut element, &host_id, &host.face);
    element.placement = host.record.placement;
    if let Err(why) = cuts_hold(txn, project, document, &element).await? {
        return Ok(Err(why));
    }
    create(txn, project, document, &element).await
}

/// `shaft.create`: a new shaft.
async fn make_shaft<T: Txn>(txn: &mut T, project: &Id, edit: &Edit, args: Args) -> Answer {
    let document = &edit.target;
    let (Some(id), Some(sent), Some(top)) = (args.element, args.outline, args.top) else {
        return Ok(Err(
            "A shaft needs an id, an outline, the level it starts on and the level it ends on."
                .to_owned(),
        ));
    };
    if let Err(why) = refused_id(txn, project, document, &id).await? {
        return Ok(Err(why));
    }
    let outline = match outline_from(&sent, &Named::shaft(id.as_str())) {
        Ok(outline) => outline,
        Err(why) => return Ok(Err(why)),
    };
    let base = match levels::tie_of_new(txn, project, document, args.level.as_ref()).await? {
        Ok(Some(base)) => base,
        Ok(None) => {
            return Ok(Err(
                "A shaft starts on a level: this command needs a level.".to_owned(),
            ));
        }
        Err(why) => return Ok(Err(why)),
    };
    if let Err(why) = levels::level_now(txn, project, document, &top).await? {
        return Ok(Err(why));
    }
    let offsets = (|| {
        let up =
            |text: Option<&str>| text.map_or(Ok(Length::from_steps(0)), |t| typed("An offset", t));
        Ok::<_, String>((up(args.offset.as_deref())?, up(args.top_offset.as_deref())?))
    })();
    let (offset, top_offset) = match offsets {
        Ok(offsets) => offsets,
        Err(why) => return Ok(Err(why)),
    };
    let name = args.name.unwrap_or_else(|| id.to_string());
    let base = Tie { offset, ..base };
    let top = Tie {
        level: top,
        offset: top_offset,
    };
    let element = shaft_record(&id, document, &name, &outline, &base, &top);
    if let Err(why) = levels::top_is_above_base(txn, project, document, &element).await? {
        return Ok(Err(why));
    }
    if let Err(why) = cuts_hold(txn, project, document, &element).await? {
        return Ok(Err(why));
    }
    create(txn, project, document, &element).await
}

/// How a wall opening is hosted: on a face of a wall, with its offsets.
fn hosted_on(element: &Element) -> Result<(Id, String, Length, Length), String> {
    match placing::read(element)? {
        Some(Hosting::OnFace {
            host,
            face,
            along,
            up,
        }) => Ok((host, face, along, up)),
        _ => Err(format!(
            "{} is not hosted on a face of a wall.",
            element.name
        )),
    }
}

/// `opening.size`: another width and height.
async fn resize<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: &Element,
    args: &Args,
) -> Checked<Element> {
    if args.width.is_none() && args.height.is_none() {
        return Ok(Err("A new size needs a width, a height or both.".to_owned()));
    }
    let old = match WallOpening::of(before.id.as_str(), &before.recipe) {
        Ok(old) => old,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let size = (|| {
        let width = match &args.width {
            Some(text) => typed("The width", text)?,
            None => old.width,
        };
        let height = match &args.height {
            Some(text) => typed("The height", text)?,
            None => old.height,
        };
        WallOpening::new(
            &Named::opening(before.id.as_str()),
            width,
            height,
            old.along_y,
        )
        .map_err(|e| e.to_string())
    })();
    let new = match size {
        Ok(new) => new,
        Err(why) => return Ok(Err(why)),
    };
    if new == old {
        return Ok(Err(format!("{} is already that size.", before.name)));
    }
    let mut after = before.clone();
    after.recipe = new.recipe();
    let (host_id, face, _, _) = match hosted_on(&after) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let host = match hosting::host_for(
        txn,
        project,
        document,
        &host_id,
        Which::Name(&face),
        "an opening",
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    placed_in(txn, project, document, &host, after).await
}

/// `opening.move`: another place on its wall, or on another wall.
async fn move_to<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: &Element,
    args: &Args,
) -> Checked<Element> {
    if args.along.is_none() && args.sill.is_none() && args.host.is_none() && args.side.is_none() {
        return Ok(Err(
            "A move needs a position along the wall, a sill height, another wall or another side."
                .to_owned(),
        ));
    }
    let (host_id, face, along, up) = match hosted_on(before) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let place = (|| {
        let along = match &args.along {
            Some(text) => typed("The position along the wall", text)?,
            None => along,
        };
        let up = match &args.sill {
            Some(text) => typed("The sill height", text)?,
            None => up,
        };
        Ok::<_, String>((along, up))
    })();
    let (along, up) = match place {
        Ok(place) => place,
        Err(why) => return Ok(Err(why)),
    };
    let new_host = args.host.clone().unwrap_or_else(|| host_id.clone());
    // Another wall, or another side of its own: the side asked for; else the side it is on.
    let role = match &args.side {
        Some(side) => Some(side.clone()),
        None if args.host.is_some() => before
            .references
            .iter()
            .find(|r| r.kind == placing::FACE_KIND)
            .and_then(|r| r.signature.clone()),
        None => None,
    };
    let host = match &role {
        Some(role) => {
            hosting::host_for(
                txn,
                project,
                document,
                &new_host,
                Which::Role(role),
                "an opening",
            )
            .await?
        }
        None => {
            hosting::host_for(
                txn,
                project,
                document,
                &new_host,
                Which::Name(&face),
                "an opening",
            )
            .await?
        }
    };
    let host = match host {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let mut after = before.clone();
    placing::write(&mut after, &new_host, &host.face, along, up);
    placed_in(txn, project, document, &host, after).await
}

/// `after` on `host`: its placement worked out, if it fits there with everything else hosted.
async fn placed_in<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    host: &Host,
    mut after: Element,
) -> Checked<Element> {
    match fit(txn, project, document, host, &after).await? {
        Ok(placement) => {
            after.placement = placement;
            Ok(Ok(after))
        }
        Err(why) => Ok(Err(why)),
    }
}

/// `opening.outline` and `shaft.outline`: another outline, which is another size and place.
async fn redraw<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: &Element,
    args: &Args,
) -> Checked<Element> {
    let Some(sent) = &args.outline else {
        return Ok(Err("Which outline? Give its corners.".to_owned()));
    };
    let named = construct_openings::named(before.id.as_str(), &before.recipe);
    let outline = match outline_from(sent, &named) {
        Ok(outline) => outline,
        Err(why) => return Ok(Err(why)),
    };
    let mut after = before.clone();
    after.recipe = if is_shaft(&before.recipe) {
        shaft_recipe(&outline)
    } else {
        floor_opening_recipe(&outline)
    };
    if after.recipe == before.recipe {
        return Ok(Err(format!("{} already has that outline.", before.name)));
    }
    if let Err(why) = cuts_hold(txn, project, document, &after).await? {
        return Ok(Err(why));
    }
    Ok(Ok(after))
}

/// `shaft.reach`: where it starts and ends.
async fn reach_of<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: &Element,
    args: &Args,
) -> Checked<Element> {
    if args.level.is_none()
        && args.offset.is_none()
        && args.top.is_none()
        && args.top_offset.is_none()
    {
        return Ok(Err(
            "A new reach needs a level or an offset for its start, its end or both.".to_owned(),
        ));
    }
    let ties = match (base_of(before), top_of(before)) {
        (Ok(Some(base)), Ok(Some(top))) => (base, top),
        (Err(e), _) | (_, Err(e)) => return Ok(Err(e.to_string())),
        _ => return Ok(Err(format!("{} is not tied to two levels.", before.name))),
    };
    let (mut base, mut top) = ties;
    if args.level.is_some() {
        base = match levels::tie_of_new(txn, project, document, args.level.as_ref()).await? {
            Ok(Some(tie)) => tie,
            Ok(None) => {
                return Ok(Err("A shaft starts on a level: name one.".to_owned()));
            }
            Err(why) => return Ok(Err(why)),
        };
    }
    if let Some(level) = &args.top {
        if let Err(why) = levels::level_now(txn, project, document, level).await? {
            return Ok(Err(why));
        }
        top = Tie {
            level: level.clone(),
            offset: Length::from_steps(0),
        };
    }
    let offsets = (|| {
        if let Some(text) = &args.offset {
            base.offset = typed("An offset", text)?;
        }
        if let Some(text) = &args.top_offset {
            top.offset = typed("An offset", text)?;
        }
        Ok::<_, String>(())
    })();
    if let Err(why) = offsets {
        return Ok(Err(why));
    }
    let mut after = before.clone();
    write_base(&mut after.props, Some(&base));
    write_top(&mut after.props, Some(&top));
    if after.props == before.props {
        return Ok(Err(format!(
            "{} already starts and ends there.",
            before.name
        )));
    }
    if let Err(why) = levels::top_is_above_base(txn, project, document, &after).await? {
        return Ok(Err(why));
    }
    if let Err(why) = cuts_hold(txn, project, document, &after).await? {
        return Ok(Err(why));
    }
    Ok(Ok(after))
}

/// A floor as openings and shafts are fitted in it: its loops in grid steps, and what would cut it.
struct Fitted {
    outer: Vec<[i64; 2]>,
    holes: Vec<Vec<[i64; 2]>>,
    cutters: Vec<Cutter>,
    /// The shafts among the cutters, as the floor's builder is handed them.
    shafts: Vec<Guest>,
}

/// A shaft as the builder of a floor it passes through is handed it: its key, its filled recipe and
/// where it sits in the floor's frame, in whole millimetres.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Guest {
    /// The shaft's permanent ID.
    pub key: String,
    /// Its recipe with its height filled.
    pub recipe: Recipe,
    /// Its place in the floor's frame.
    pub placement: Placement,
}

/// An outline's corners in grid steps.
fn steps_of(outline: &Outline) -> Vec<[i64; 2]> {
    outline
        .corners()
        .iter()
        .map(|corner| corner.map(Length::steps))
        .collect()
}

/// The cutter a shaft makes of a floor from `bottom` up to `top`: its outline moved to the floor's
/// frame, if it passes wholly through it; none when it misses the floor or cannot be read as built
/// (its own build failure says why, unless it is the element the command writes, `mine`). The inner
/// error is a shaft that ends inside the floor's thickness, or one of `mine` that does not read.
async fn shaft_cut<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    floor: &Element,
    record: &Element,
    (bottom, top): (Length, Length),
    mine: bool,
) -> Result<Result<Option<(Cutter, Guest)>, String>, StoreError> {
    let id = record.id.as_str();
    let named = Named::shaft(id);
    let own = match shape_levels::recipe_at(reader, project, document, record, At::Now).await? {
        Ok(own) => own,
        Err(e) if mine => return Ok(Err(format!("{}: {}", record.name, e.message))),
        Err(_) => return Ok(Ok(None)),
    };
    let shaft = match Shaft::of(id, &own) {
        Ok(shaft) => shaft,
        Err(e) if mine => return Ok(Err(e.to_string())),
        Err(_) => return Ok(Ok(None)),
    };
    let base = match shape_levels::placed(reader, project, document, record, At::Now).await? {
        Ok(placed) => placed.elevation,
        Err(_) => None,
    };
    let Some(base) = base else {
        return Ok(Ok(None));
    };
    let Some(end) = base.checked_add(shaft.height) else {
        return Ok(Ok(None));
    };
    match reach(base, end, bottom, top) {
        Reach::Misses => Ok(Ok(None)),
        Reach::EndsInside(which) => {
            let refused = construct_openings::OpeningError::EndsInFloor {
                element: named,
                floor: format!("floor {}", floor.id),
                end: which,
            };
            Ok(Err(refused.to_string()))
        }
        Reach::Through => {
            let by = (
                Length::checked_from_mm(record.placement.x.saturating_sub(floor.placement.x)),
                Length::checked_from_mm(record.placement.y.saturating_sub(floor.placement.y)),
            );
            let (Some(dx), Some(dy)) = by else {
                return Ok(Ok(None));
            };
            let guest = Guest {
                key: id.to_owned(),
                recipe: own,
                placement: Placement {
                    x: record.placement.x.saturating_sub(floor.placement.x),
                    y: record.placement.y.saturating_sub(floor.placement.y),
                    z: 0,
                },
            };
            Ok(
                match shaft.outline.steps_moved(&named, [dx.steps(), dy.steps()]) {
                    Ok(corners) => Ok(Some((
                        Cutter {
                            element: named,
                            corners,
                        },
                        guest,
                    ))),
                    Err(e) if mine => Err(e.to_string()),
                    Err(_) => Ok(None),
                },
            )
        }
    }
}

/// Everything of the document that would cut `floor`, as the records are now with `after` written
/// over them: the floor openings drawn in it and the shafts that pass wholly through it. `None`
/// when the floor cannot be read as built (its own build failure says why, and an opening is
/// not the cause). The inner error is a shaft that ends inside the floor's thickness, or an
/// outline of `after` that does not read, in words.
async fn fitted<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    floor: &Element,
    after: Option<&Element>,
) -> Result<Result<Option<Fitted>, String>, StoreError> {
    let Ok(filled) = shape_levels::recipe_at(reader, project, document, floor, At::Now).await?
    else {
        return Ok(Ok(None));
    };
    let Ok(plan) = FloorPlan::of(&filled) else {
        return Ok(Ok(None));
    };
    let Ok(placed) = shape_levels::placed(reader, project, document, floor, At::Now).await? else {
        return Ok(Ok(None));
    };
    let (Some(top), Some(total)) = (placed.elevation, plan.total()) else {
        return Ok(Ok(None));
    };
    let Some(bottom) = top.checked_sub(Length::from_steps(total)) else {
        return Ok(Ok(None));
    };
    let mut all: BTreeMap<Id, Element> = records::read_document(reader, project, document)
        .await?
        .into_iter()
        .map(|(id, (_, record))| (id, record))
        .collect();
    if let Some(after) = after {
        all.insert(after.id.clone(), after.clone());
    }
    let mut cutters = Vec::new();
    let mut shafts = Vec::new();
    for (id, record) in &all {
        let mine = after.is_some_and(|after| after.id == *id);
        let named = construct_openings::named(id.as_str(), &record.recipe);
        if is_floor_opening(&record.recipe) {
            let on_this = matches!(
                placing::read(record),
                Ok(Some(Hosting::InPlan { host, .. })) if host == floor.id
            );
            if !on_this {
                continue;
            }
            match outline_of(id.as_str(), &record.recipe) {
                Ok(outline) => cutters.push(Cutter {
                    element: named,
                    corners: steps_of(&outline),
                }),
                Err(e) if mine => return Ok(Err(e.to_string())),
                Err(_) => {}
            }
        } else if is_shaft(&record.recipe) {
            let heights = (bottom, top);
            match shaft_cut(reader, project, document, floor, record, heights, mine).await? {
                Ok(Some((cutter, guest))) => {
                    cutters.push(cutter);
                    shafts.push(guest);
                }
                Ok(None) => {}
                Err(why) => return Ok(Err(why)),
            }
        }
    }
    let ring = |loop_: &[construct_regen::floor_plan::PlanCorner]| -> Vec<[i64; 2]> {
        loop_.iter().map(|corner| corner.at).collect()
    };
    Ok(Ok(Some(Fitted {
        outer: ring(plan.outer()),
        holes: plan.holes().iter().map(|hole| ring(hole)).collect(),
        cutters,
        shafts,
    })))
}

/// Why the floors `after` would cut (a floor opening's floor; every floor, for a shaft) would not
/// be cut by it with everything else of the document, in construct-openings' words; or nothing.
async fn cuts_hold<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    after: &Element,
) -> Checked<()> {
    let floors: Vec<Element> = if is_shaft(&after.recipe) {
        records::read_document(reader, project, document)
            .await?
            .into_values()
            .map(|(_, record)| record)
            .filter(hosting::is_floor)
            .collect()
    } else {
        let Ok(Some(Hosting::InPlan { host, .. })) = placing::read(after) else {
            return Ok(Ok(()));
        };
        match records::read_element(reader, project, document, &host).await? {
            Some((_, floor)) => vec![floor],
            None => return Ok(Ok(())),
        }
    };
    for floor in &floors {
        let found = match fitted(reader, project, document, floor, Some(after)).await? {
            Ok(Some(found)) => found,
            Ok(None) => continue,
            Err(why) => return Ok(Err(why)),
        };
        if let Err(e) = fit_in_floor(
            &format!("floor {}", floor.id),
            &found.outer,
            &found.holes,
            &found.cutters,
        ) {
            return Ok(Err(e.to_string()));
        }
    }
    Ok(Ok(()))
}

/// The outlines that cut the floor `floor`, as the document is now, in its own frame and grid
/// steps: the floor openings drawn in it and the shafts that pass wholly through it, those the
/// builders cut. What a page's area, volume and mass of a floor are worked out after. None for a
/// floor nothing cuts, and for one whose cuts the builders refuse (it is built as failed and its
/// own failure says so).
///
/// # Errors
///
/// A store error.
pub async fn floor_cuts<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    floor: &Element,
) -> Result<Vec<Vec<[i64; 2]>>, StoreError> {
    let view = floor_view(reader, project, document, floor).await?;
    Ok(view.cuts.into_iter().map(|cut| cut.corners).collect())
}

/// What cuts a floor, as a page draws it and builds it: the outline of each opening and shaft that
/// cuts it, and the shafts as its builder is handed them.
#[derive(Serialize, Debug, Clone, PartialEq, Default)]
pub struct FloorCuts {
    /// Each outline that cuts the floor, in its own frame and grid steps.
    pub cuts: Vec<CutView>,
    /// The shafts among them, for a page that builds the floor with what cuts it.
    pub shafts: Vec<Guest>,
}

/// One outline that cuts a floor.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct CutView {
    /// The opening's or shaft's permanent ID.
    pub key: String,
    /// Its corners, `[x, y]` in grid steps, in the floor's frame.
    pub corners: Vec<[i64; 2]>,
}

/// [`FloorCuts`] of the floor `floor`, as the document is now. Empty for a floor nothing cuts, and
/// for one whose cuts the builders refuse (it is built as failed and its own failure says so).
///
/// # Errors
///
/// A store error.
pub async fn floor_view<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    floor: &Element,
) -> Result<FloorCuts, StoreError> {
    let Ok(Some(found)) = fitted(reader, project, document, floor, None).await? else {
        return Ok(FloorCuts::default());
    };
    let Ok(cutting) = fit_in_floor(
        &format!("floor {}", floor.id),
        &found.outer,
        &found.holes,
        &found.cutters,
    ) else {
        return Ok(FloorCuts::default());
    };
    let cuts: Vec<CutView> = cutting
        .into_iter()
        .filter_map(|at| found.cutters.get(at))
        .map(|cutter| CutView {
            key: cutter.element.key.clone(),
            corners: cutter.corners.clone(),
        })
        .collect();
    let shafts = found
        .shafts
        .into_iter()
        .filter(|guest| cuts.iter().any(|cut| cut.key == guest.key))
        .collect();
    Ok(FloorCuts { cuts, shafts })
}

/// The part of a generic command's arguments these rules read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    recipe: Option<construct_model::geometry::Recipe>,
}

/// The rules of the generic commands for openings and shafts, asked before one runs: they are made
/// by `opening.place`, `opening.draw` and `shaft.create`, and changed, moved and deleted by the
/// commands of their own, not by `element.create`, `element.set`, `element.move` nor
/// `element.delete`; `hosted.place` and `hosted.move` are not for them either. The inner error is
/// the refusal, naming the command to use. An edit these rules have nothing to say of (another
/// command, arguments that do not read, an element that is not there or is not one of these) is
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
    if matches!(edit.cmd.as_str(), "element.create" | "hosted.place")
        && args.recipe.as_ref().is_some_and(is_void)
    {
        return Ok(Err(
            "An opening in a wall is made with opening.place, one in a floor with opening.draw \
             and a shaft with shaft.create."
                .to_owned(),
        ));
    }
    if !matches!(
        edit.cmd.as_str(),
        "element.set" | "element.move" | "element.delete" | "hosted.move"
    ) {
        return Ok(Ok(()));
    }
    let Some(id) = &args.element else {
        return Ok(Ok(()));
    };
    let Some((_, record)) = records::read_element(txn, project, &edit.target, id).await? else {
        return Ok(Ok(()));
    };
    if !is_void(&record.recipe) {
        return Ok(Ok(()));
    }
    Ok(Err(what_to_do(&record)))
}
