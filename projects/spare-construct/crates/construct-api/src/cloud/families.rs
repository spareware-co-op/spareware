//! The commands of a placed family (job 7.1, CON-A4-F8-P1, piece A64): a door or a window made
//! from a family of the library, put on a side of a wall, changed to another type of its family,
//! moved along its wall or to another, flipped to the wall's other side, given values of its own,
//! and deleted. They go through the same edit log as `element.*`, with the same access check
//! (`model.edit`) and the same claims, and each is one entry, so one undo takes back all it wrote
//! ([`crate::cloud::undo`]). How a placed family sits in its record is construct-regen's
//! [`families`] and [`hosting`] (the regenerator builds a wall with its families through them),
//! and the fit is the kernel builder's own rules asked without the engine, as a box door's is
//! ([`crate::cloud::hosting`]), so a model drawn on the stand-in holds no family the kernel's
//! builder would refuse.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `family.place` | `element`, `family` (its permanent id, such as `library.doors-windows.fixed-window`), `type` (its key, such as `w1200x1200`), `host` (a wall), `side` (`exterior` or `interior`), `along` (from the wall's start to the middle of the opening), `up` (default 0), `values` (instance values by parameter key, typed text), `name` (default the id) | A new element, hosted on that side of the wall, in its family's category |
//! | `family.retype` | `element`, `type` | The element: its type |
//! | `family.move` | `element`, and any of `along`, `up`, `host`, `side` | The element: its place on its wall, or on another wall |
//! | `family.flip` | `element` | The element: on the wall's other side, at the same place along it |
//! | `family.set` | `element`, `values` (by parameter key, typed text; `null` takes the value away) | The element: its instance values, such as a window's sill height |
//! | `family.delete` | `element` | Its pointer goes, and its claim |
//!
//! The kind a placed family has is what people call it: `door` for a family of the doors
//! category, `window` for the windows category, `family` for any other. Its category is its
//! family's own (stored as `@category`, so what groups or counts by category reads it there); the
//! generic models line of construct-categories stays only as the fallback for a family whose
//! category the table lacks. The family comes from the library
//! ([`families::Library::baked`], interim until job 7.6 stores families in the document); the
//! record names the family recipe version by its content id, so a later version of the family
//! changes nothing already placed.
//!
//! Each is refused, as its result and in words, when the wall is missing or is not a wall, the
//! side is not one of its sides, the family or type is not in the library, an offset or an
//! instance value is not what it takes (the type's own values are changed by changing the type),
//! the element is not a placed family or is not there, another person is working on it, or the
//! family would not fit its wall, in the kernel builder's own words naming it (`"family F-1: it
//! crosses the wall's end"`), on every edition. Nothing is written by a refused command.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a placed
//! family and `element.set`, `element.move` and `element.delete` of one are refused, naming the
//! command to use; `hosted.place` and `hosted.move` are refused the same way. Claiming and
//! releasing an element is the generic commands' for a family too.
//!
//! A wall that hosts a family is not deleted (the hosting rules, [`crate::cloud::hosting`]), and
//! one shortened or changed under it is built as failed, naming the family, by the kernel's
//! builder, as under a box door. Nothing here reads a clock or a random number: the claim's time
//! is the orderer's.
//!
//! For the page, without the log: [`read_library`], the library's families with their types and
//! the values each instance holds, which the page's family tool lists.

use crate::cloud::elements::write_record;
use crate::cloud::hosting::{self, Host, Which, fit, length};
use construct_categories::{CategoryId, CategoryTable, DOORS, GENERIC_MODELS, WINDOWS};
use construct_families::{ContentId, Key, Recipe as FamilyRecipe};
use construct_model::element::Element;
use construct_model::geometry::Recipe;
use construct_params::Units;
use construct_regen::families::{self, Instance, Library};
use construct_regen::hosting::{self as placing, Hosting};
use construct_regen::parameters::{self as records, CATEGORY_KEY};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context, Outcome, claims};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Every command here.
pub const COMMANDS: [&str; 6] = [PLACE, RETYPE, MOVE, FLIP, SET, DELETE];
/// Puts a new placed family on a side of a wall.
pub const PLACE: &str = "family.place";
/// Changes a placed family to another type of its family.
pub const RETYPE: &str = "family.retype";
/// Moves a placed family along its wall, or to another wall.
pub const MOVE: &str = "family.move";
/// Puts a placed family on the other side of its wall.
pub const FLIP: &str = "family.flip";
/// Sets the values a placed family holds itself.
pub const SET: &str = "family.set";
/// Deletes a placed family.
pub const DELETE: &str = "family.delete";

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
    family: Option<String>,
    #[serde(rename = "type")]
    ty: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    host: Option<Id>,
    side: Option<String>,
    along: Option<String>,
    up: Option<String>,
    values: Option<BTreeMap<String, Option<String>>>,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

/// The units a typed value is read in until a project keeps its own: metric.
fn units() -> Units {
    Units::default()
}

/// The kind a placed family of `category` has.
fn kind_of(category: CategoryId) -> &'static str {
    if category == DOORS {
        "door"
    } else if category == WINDOWS {
        "window"
    } else {
        "family"
    }
}

/// The category of a placed family of `recipe`: its family's, or generic models where the family's
/// is not one the table has.
fn category_of(recipe: &FamilyRecipe) -> CategoryId {
    if CategoryTable::new().get(recipe.category).is_some() {
        recipe.category
    } else {
        GENERIC_MODELS
    }
}

/// The families of the library by name, for a refusal that says what there is.
fn names_of(library: &Library) -> String {
    let names: Vec<String> = library
        .recipes()
        .map(|(_, recipe)| format!("{} ({})", recipe.name, recipe.id))
        .collect();
    names.join(", ")
}

/// The family `family` of the library, found by its permanent id.
fn family_in(library: &Library, family: &str) -> Result<(ContentId, Arc<FamilyRecipe>), String> {
    library.family(family).ok_or_else(|| {
        format!(
            "There is no family {family} in the library; it has {}.",
            names_of(library)
        )
    })
}

/// The type `ty` of `recipe`, checked: a key the family names.
fn type_in(recipe: &FamilyRecipe, ty: &str) -> Result<Key, String> {
    let names = || {
        let all: Vec<String> = recipe
            .named_types()
            .iter()
            .map(|t| format!("{} ({})", t.name(), t.key()))
            .collect();
        all.join(", ")
    };
    let key = Key::parse(ty).ok_or_else(|| {
        format!(
            "{ty} is not a type of {}; its types are {}.",
            recipe.name,
            names()
        )
    })?;
    if recipe.named_type(&key).is_none() {
        return Err(format!(
            "{} has no type {ty}; its types are {}.",
            recipe.name,
            names()
        ));
    }
    Ok(key)
}

/// The instance values sent, as stored text by key: each typed text read as its parameter reads
/// it. `None` for a key means it is taken away.
fn values_in(
    recipe: &FamilyRecipe,
    sent: &BTreeMap<String, Option<String>>,
) -> Result<Vec<(Key, String)>, String> {
    let mut read = Vec::with_capacity(sent.len());
    for (name, typed) in sent {
        let key = Key::parse(name)
            .ok_or_else(|| format!("{name} is not the key of one of {}'s values.", recipe.name))?;
        let stored = if let Some(typed) = typed {
            recipe.instance_value(&key, typed, &units())?
        } else {
            // taking a value away: the parameter must be one an instance holds
            recipe.check_instance_key(&key)?;
            String::new()
        };
        read.push((key, stored));
    }
    Ok(read)
}

/// The claim this person takes on an element, or who else holds it.
pub(crate) async fn claim_on<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
    piece: &Id,
    claim_ms: u64,
) -> Result<Result<Claim, String>, StoreError> {
    let document = &edit.target;
    let holder =
        claims::held_by_another(txn, project, document, piece, &edit.person, context.now_ms)
            .await?;
    if let Some(claim) = holder {
        return Ok(Err(format!("{} is working on this.", claim.name)));
    }
    let person: Option<Named> = read_row(txn, &Place::Person(edit.person.clone()).key()).await?;
    let name = person.map_or_else(|| edit.person.to_string(), |person| person.name);
    Ok(Ok(Claim {
        person: edit.person.clone(),
        name,
        // milliseconds since 1970 plus a claim's length: far below u64::MAX
        until_ms: context.now_ms.saturating_add(claim_ms),
    }))
}

/// Writes `after` over `before` (its hash) and takes the claim: the whole result of a command that
/// changes a placed family.
pub(crate) async fn rewrite<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: Id,
    after: &Element,
    claim: Claim,
) -> Answer {
    let hash = write_record(txn, project, after).await?;
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: after.id.clone(),
    };
    txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    let claim = claims::take(txn, project, document, &after.id, claim).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: after.id.clone(),
            before: Some(before),
            after: Some(hash),
        }],
        claims: vec![claim],
        ..Applied::default()
    }))
}

/// Runs a `family.*` command.
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
    if edit.cmd == PLACE {
        return place(txn, project, edit, args).await;
    }
    let Some(id) = args.element.clone() else {
        return Ok(Err("Which element?".to_owned()));
    };
    let document = &edit.target;
    let Some((hash, before)) = records::read_element(txn, project, document, &id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    if !families::is_instance(&before.recipe) {
        return Ok(Err(format!(
            "{} is not a placed family: {}.",
            before.name,
            match edit.cmd.as_str() {
                DELETE => "delete it with element.delete",
                MOVE => "move it with element.move or hosted.move",
                _ => "change it with element.set",
            }
        )));
    }
    let claim = match claim_on(txn, project, context, edit, &id, claim_ms).await? {
        Ok(claim) => claim,
        Err(why) => return Ok(Err(why)),
    };
    if edit.cmd == DELETE {
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
        RETYPE => retype(txn, project, document, &before, &args).await?,
        MOVE => move_to(txn, project, document, &before, &args).await?,
        FLIP => flip(txn, project, document, &before).await?,
        SET => set_values(txn, project, document, &before, &args).await?,
        other => Err(format!("Unknown command {other}.")),
    };
    match after {
        Ok(after) => rewrite(txn, project, document, hash, &after, claim).await,
        Err(why) => Ok(Err(why)),
    }
}

/// `family.place`: a new placed family on a side of a wall.
#[allow(
    clippy::too_many_lines,
    reason = "each thing a placed family is checked against, in the order its arguments are read"
)]
async fn place<T: Txn>(txn: &mut T, project: &Id, edit: &Edit, args: Args) -> Answer {
    let document = &edit.target;
    let (Some(id), Some(family), Some(ty), Some(host_id), Some(side), Some(along)) = (
        args.element,
        args.family,
        args.ty,
        args.host,
        args.side,
        args.along,
    ) else {
        return Ok(Err(
            "A placed family needs an id, a family, a type of it, a wall, a side of the wall and \
             a position along it."
                .to_owned(),
        ));
    };
    if records::read_element(txn, project, document, &id)
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
    let library = Library::baked();
    let (content, recipe) = match family_in(&library, &family) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let key = match type_in(&recipe, &ty) {
        Ok(key) => key,
        Err(why) => return Ok(Err(why)),
    };
    let along = match length("The position along the wall", &along) {
        Ok(length) => length,
        Err(why) => return Ok(Err(why)),
    };
    let up = match length(
        "The height above the wall's base",
        args.up.as_deref().unwrap_or("0"),
    ) {
        Ok(length) => length,
        Err(why) => return Ok(Err(why)),
    };
    let mut placed = families::recipe(&content, &key);
    match values_in(&recipe, &args.values.unwrap_or_default()) {
        Ok(values) => {
            for (key, stored) in values {
                placed = families::with_value(&placed, &key, &stored);
            }
        }
        Err(why) => return Ok(Err(why)),
    }
    let host = match hosting::host_for(
        txn,
        project,
        document,
        &host_id,
        Which::Role(&side),
        "a door or a window",
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let category = category_of(&recipe);
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), records::write_category(category));
    // On its wall's level: tied as the wall's base is, and on no level by number.
    let base = match construct_levels::base_of(&host.record) {
        Ok(base) => base,
        Err(e) => return Ok(Err(e.to_string())),
    };
    construct_levels::write_base(&mut props, base.as_ref());
    let mut element = Element {
        id: id.clone(),
        document: document.clone(),
        kind: kind_of(category).to_owned(),
        name: args.name.unwrap_or_else(|| id.to_string()),
        level: 0,
        placement: host.record.placement,
        recipe: placed,
        props,
        references: Vec::new(),
    };
    placing::write(&mut element, &host_id, &host.face, along, up);
    element.placement = match fit(txn, project, document, &host, &element).await? {
        Ok(placement) => placement,
        Err(why) => return Ok(Err(why)),
    };
    let hash = write_record(txn, project, &element).await?;
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: id.clone(),
    };
    txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: id,
            before: None,
            after: Some(hash),
        }],
        ..Applied::default()
    }))
}

/// A placed family's instance and its family recipe, from the library.
fn instance_of(element: &Element) -> Result<(Instance, Arc<FamilyRecipe>), String> {
    let instance =
        Instance::of(&element.recipe).map_err(|e| format!("{}: {}", element.name, e.message))?;
    let library = Library::baked();
    let recipe = families::resolve(&*library, &instance)
        .map(|resolved| resolved.recipe)
        .map_err(|e| e.message)?;
    Ok((instance, recipe))
}

/// How a placed family is hosted: on a face of a wall.
fn hosted_on(
    element: &Element,
) -> Result<
    (
        Id,
        String,
        construct_params::Length,
        construct_params::Length,
    ),
    String,
> {
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

/// `family.retype`: another type of the family.
async fn retype<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: &Element,
    args: &Args,
) -> Result<Result<Element, String>, StoreError> {
    let Some(ty) = &args.ty else {
        return Ok(Err(
            "Which type? A family is changed to a type of its own.".to_owned()
        ));
    };
    let (instance, recipe) = match instance_of(before) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let key = match type_in(&recipe, ty) {
        Ok(key) => key,
        Err(why) => return Ok(Err(why)),
    };
    if key == instance.ty {
        return Ok(Err(format!("{} is already of type {key}.", before.name)));
    }
    let mut after = before.clone();
    after.recipe = families::with_type(&before.recipe, &key);
    refit(txn, project, document, after).await
}

/// `family.move`: another place on its wall, or on another wall.
async fn move_to<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: &Element,
    args: &Args,
) -> Result<Result<Element, String>, StoreError> {
    if args.along.is_none() && args.up.is_none() && args.host.is_none() && args.side.is_none() {
        return Ok(Err(
            "A move needs a position along the wall, a height, another wall or another side."
                .to_owned(),
        ));
    }
    let (host_id, face, along, up) = match hosted_on(before) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let along = match &args.along {
        Some(typed) => match length("The position along the wall", typed) {
            Ok(length) => length,
            Err(why) => return Ok(Err(why)),
        },
        None => along,
    };
    let up = match &args.up {
        Some(typed) => match length("The height above the wall's base", typed) {
            Ok(length) => length,
            Err(why) => return Ok(Err(why)),
        },
        None => up,
    };
    // Another wall, or another side of its own: the side asked for; else the side it is on.
    let new_host = args.host.clone().unwrap_or_else(|| host_id.clone());
    let role = match &args.side {
        Some(side) => Some(side.clone()),
        None if args.host.is_some() => before
            .references
            .iter()
            .find(|r| r.kind == placing::FACE_KIND)
            .and_then(|r| r.signature.clone()),
        None => None,
    };
    let host = match role {
        Some(role) => {
            hosting::host_for(
                txn,
                project,
                document,
                &new_host,
                Which::Role(&role),
                "a door or a window",
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
                "a door or a window",
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
    place_in(txn, project, document, &host, after).await
}

/// `family.flip`: the wall's other side, at the same place along it.
async fn flip<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: &Element,
) -> Result<Result<Element, String>, StoreError> {
    let (host_id, face, along, up) = match hosted_on(before) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let current = match hosting::host_for(
        txn,
        project,
        document,
        &host_id,
        Which::Name(&face),
        "a door or a window",
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let other = placing::SIDES
        .iter()
        .find(|side| **side != current.face.role)
        .copied()
        .unwrap_or("interior");
    let host = match hosting::host_for(
        txn,
        project,
        document,
        &host_id,
        Which::Role(other),
        "a door or a window",
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let mut after = before.clone();
    placing::write(&mut after, &host_id, &host.face, along, up);
    place_in(txn, project, document, &host, after).await
}

/// `family.set`: the values the instance holds.
async fn set_values<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: &Element,
    args: &Args,
) -> Result<Result<Element, String>, StoreError> {
    let Some(sent) = args.values.as_ref().filter(|sent| !sent.is_empty()) else {
        return Ok(Err(
            "Which values? Name each one by its key, with what it should be.".to_owned(),
        ));
    };
    let (_, recipe) = match instance_of(before) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let values = match values_in(&recipe, sent) {
        Ok(values) => values,
        Err(why) => return Ok(Err(why)),
    };
    let mut after = before.clone();
    for (key, stored) in values {
        after.recipe = families::with_value(&after.recipe, &key, &stored);
    }
    if after.recipe == before.recipe {
        return Ok(Err(format!("{} already has those values.", before.name)));
    }
    refit(txn, project, document, after).await
}

/// `after`, checked against the wall it is on now: where it sits, or why not.
async fn refit<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    after: Element,
) -> Result<Result<Element, String>, StoreError> {
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
        "a door or a window",
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    place_in(txn, project, document, &host, after).await
}

/// `after` on `host`: its placement worked out, if it fits there with everything else hosted.
async fn place_in<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    host: &Host,
    mut after: Element,
) -> Result<Result<Element, String>, StoreError> {
    match fit(txn, project, document, host, &after).await? {
        Ok(placement) => {
            after.placement = placement;
            Ok(Ok(after))
        }
        Err(why) => Ok(Err(why)),
    }
}

/// The part of a generic command's arguments these rules read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    recipe: Option<Recipe>,
}

/// The rules of the generic commands for a placed family, asked before one runs: it is made by
/// `family.place`, and changed, moved and deleted by the family commands, not by `element.create`,
/// `element.set`, `element.move` nor `element.delete`; `hosted.place` and `hosted.move` are not
/// for a family either. The inner error is the refusal, naming the command to use. An edit these
/// rules have nothing to say of (another command, arguments that do not read, an element that is
/// not there or is not a placed family) is left to the command, which answers it as it always has.
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
    let a_family = args.recipe.as_ref().is_some_and(families::is_instance);
    if matches!(edit.cmd.as_str(), "element.create" | "hosted.place") && a_family {
        return Ok(Err(
            "A placed family is made with family.place, which seats it on its wall.".to_owned(),
        ));
    }
    let what = match edit.cmd.as_str() {
        "element.set" | "element.move" | "element.delete" | "hosted.move" => edit.cmd.as_str(),
        _ => return Ok(Ok(())),
    };
    let Some(id) = &args.element else {
        return Ok(Ok(()));
    };
    let Some((_, record)) = records::read_element(txn, project, &edit.target, id).await? else {
        return Ok(Ok(()));
    };
    if !families::is_instance(&record.recipe) {
        return Ok(Ok(()));
    }
    let name = &record.name;
    Ok(Err(match what {
        "element.delete" => format!("{name} is a placed family: delete it with family.delete."),
        "element.set" => format!(
            "{name} is a placed family: change its type with family.retype or its values with family.set."
        ),
        _ => format!(
            "{name} is a placed family: move it along its wall or to another wall with family.move."
        ),
    }))
}

/// A family of the library as the page lists it.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct FamilyView {
    /// The family's permanent id: what `family.place` names it by.
    pub id: String,
    /// The version of its recipe the library holds, by content id: what a placed one's record
    /// names.
    pub content: String,
    /// Its name.
    pub name: String,
    /// What a placed one is called: `door`, `window` or `family`.
    pub kind: String,
    /// Its category's id, the family's own.
    pub category: u32,
    /// Its category's name.
    pub category_name: String,
    /// Its types, in the order its recipe names them.
    pub types: Vec<TypeView>,
    /// The values each placed one holds, with their defaults.
    pub values: Vec<ValueView>,
}

/// A type of a family.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct TypeView {
    /// Its key, what `family.place` and `family.retype` name it by.
    pub key: String,
    /// Its name.
    pub name: String,
}

/// A value each placed instance holds.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ValueView {
    /// Its key, what `family.set` names it by.
    pub key: String,
    /// Its name.
    pub name: String,
    /// Its kind's tag, such as `length` or `choice`.
    pub kind: String,
    /// Its default as a person reads it.
    pub shown: String,
    /// The choices of a choice value, each its key and its label.
    pub choices: Vec<ChoiceView>,
}

/// One choice of a choice value.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ChoiceView {
    /// What is stored.
    pub key: String,
    /// What people see and type.
    pub label: String,
}

/// The library's families, with their types and the values each instance holds, in the order of
/// their content ids: what the page's family tool offers. Interim as the library is (job 7.6).
#[must_use]
pub fn read_library() -> Vec<FamilyView> {
    let library = Library::baked();
    let table = CategoryTable::new();
    library
        .recipes()
        .map(|(content, recipe)| {
            let category = category_of(recipe);
            FamilyView {
                id: recipe.id.to_string(),
                content: content.as_str().to_owned(),
                name: recipe.name.clone(),
                kind: kind_of(category).to_owned(),
                category: category.0,
                category_name: table.get(category).map(|c| c.name).unwrap_or_default(),
                types: recipe
                    .named_types()
                    .iter()
                    .map(|t| TypeView {
                        key: t.key().to_string(),
                        name: t.name().to_owned(),
                    })
                    .collect(),
                values: recipe
                    .instance_parameters(&units())
                    .into_iter()
                    .map(|p| ValueView {
                        key: p.key.to_string(),
                        name: p.name,
                        kind: p.kind.tag().to_owned(),
                        shown: p.shown,
                        choices: p
                            .choices
                            .into_iter()
                            .map(|(key, label)| ChoiceView { key, label })
                            .collect(),
                    })
                    .collect(),
            }
        })
        .collect()
}
