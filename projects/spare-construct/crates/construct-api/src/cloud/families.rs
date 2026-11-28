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
//! category the table lacks. The family is one the document holds (job 7.6,
//! [`crate::cloud::loading`]); `family.place` of a library family the document does not hold yet
//! loads it in the same entry, so every new copy names a family in the document. The record names
//! the family by its permanent id and the recipe version by its content id, so only a reload
//! (`family.reload`) moves a placed copy to a later version. A copy written before job 7.6 names
//! no family in the document and is read from the library by its content id.
//!
//! The commands of the families a document holds (`family.load`, `family.reload`,
//! `family.swap`, `family.generate`, `family.remove`) are [`crate::cloud::loading`]'s, run
//! through [`apply`] here.
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
//! the values each instance holds; the families route lists the document's own first
//! ([`crate::cloud::loading::read_families`]).

use crate::cloud::elements::write_record;
use crate::cloud::hosting::{self, Host, Which, fit, length};
use crate::cloud::levels;
use crate::cloud::loading;
use construct_categories::{CategoryId, CategoryTable, DOORS, GENERIC_MODELS, WINDOWS};
use construct_families::{ContentId, Hosting as FamilyHosting, Key, Recipe as FamilyRecipe};
use construct_levels::Tie;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Definition, Holder, Kind, Units, Value};
use construct_regen::families::{self, Heading, Instance, Library};
use construct_regen::hosting::{self as placing, Hosting};
use construct_regen::parameters::At;
use construct_regen::parameters::{self as records, CATEGORY_KEY};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context, Outcome, claims};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Every command here.
pub const COMMANDS: [&str; 7] = [PLACE, RETYPE, MOVE, FLIP, TURN, SET, DELETE];
/// Puts a new placed family on a side of a wall.
pub const PLACE: &str = "family.place";
/// Changes a placed family to another type of its family.
pub const RETYPE: &str = "family.retype";
/// Moves a placed family along its wall, or to another wall.
pub const MOVE: &str = "family.move";
/// Puts a placed family on the other side of its wall.
pub const FLIP: &str = "family.flip";
/// Turns a placed family that stands on a level.
pub const TURN: &str = "family.turn";
/// Sets the values a placed family holds itself.
pub const SET: &str = "family.set";
/// Deletes a placed family.
pub const DELETE: &str = "family.delete";

/// Whether a command is one of these, or of the families a document holds
/// ([`loading::COMMANDS`]).
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd) || loading::is_command(cmd)
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
    /// A standing family's point, each a length in whole millimetres.
    x: Option<String>,
    y: Option<String>,
    /// A standing family's turn, an angle.
    turn: Option<String>,
    /// A standing family's level: its ID, or an old page's integer.
    level: Option<serde_json::Value>,
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
pub(crate) fn kind_of(category: CategoryId) -> &'static str {
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
pub(crate) fn category_of(recipe: &FamilyRecipe) -> CategoryId {
    if CategoryTable::new().get(recipe.category).is_some() {
        recipe.category
    } else {
        GENERIC_MODELS
    }
}

/// The families of the library by name, for a refusal that says what there is.
pub(crate) fn names_of(library: &Library) -> String {
    let names: Vec<String> = library
        .recipes()
        .map(|(_, recipe)| format!("{} ({})", recipe.name, recipe.id))
        .collect();
    names.join(", ")
}

/// The type `ty` of `recipe`, checked: a key the family names.
pub(crate) fn type_in(recipe: &FamilyRecipe, ty: &str) -> Result<Key, String> {
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
    if loading::is_command(&edit.cmd) {
        return loading::apply(txn, project, context, edit).await;
    }
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
        TURN => turn_to(&before, &args),
        SET => set_values(txn, project, document, &before, &args).await?,
        other => Err(format!("Unknown command {other}.")),
    };
    match after {
        Ok(after) => rewrite(txn, project, document, hash, &after, claim).await,
        Err(why) => Ok(Err(why)),
    }
}

/// What `family.place` has read and checked of a family before it asks where it goes.
struct Prepared {
    id: Id,
    recipe: Arc<FamilyRecipe>,
    /// The record's recipe: the family version, the type and the values given.
    placed: Recipe,
    category: CategoryId,
    /// The record of the library's family this entry loads, when the document did not hold it yet:
    /// the first change, so it is regenerated before the copy that reads it.
    loaded: Option<Change>,
}

/// `family.place`: a new placed family: on a side of a wall (a door, a window, casework or a
/// fixture hung on it), or standing on a level at a point.
async fn place<T: Txn>(txn: &mut T, project: &Id, edit: &Edit, args: Args) -> Answer {
    let document = &edit.target;
    let (Some(id), Some(family), Some(ty)) =
        (args.element.clone(), args.family.clone(), args.ty.clone())
    else {
        return Ok(Err(
            "A placed family needs an id, a family and a type of it.".to_owned(),
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
    // The document's family, or the library's loaded with this entry (its record the first
    // change, so it is regenerated before the copy that reads it).
    let (stored, loaded) = match loading::family_for(txn, project, document, &family).await? {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let recipe = &stored.recipe;
    let key = match type_in(recipe, &ty) {
        Ok(key) => key,
        Err(why) => return Ok(Err(why)),
    };
    let mut placed = families::of_family(&families::recipe(&stored.content, &key), &stored);
    match values_in(recipe, &args.values.clone().unwrap_or_default()) {
        Ok(values) => {
            for (key, stored) in values {
                placed = families::with_value(&placed, &key, &stored);
            }
        }
        Err(why) => return Ok(Err(why)),
    }
    let category = category_of(recipe);
    let hosting = recipe.hosting.clone();
    let prepared = Prepared {
        id,
        recipe: Arc::new(stored.recipe.clone()),
        placed,
        category,
        loaded,
    };
    match hosting {
        FamilyHosting::Unhosted => stand(txn, project, edit, &args, prepared).await,
        FamilyHosting::Wall { .. } | FamilyHosting::Face => {
            hang(txn, project, edit, &args, prepared).await
        }
        FamilyHosting::Floor => Ok(Err(format!(
            "{} is hosted on a floor, which is not placed yet.",
            prepared.recipe.name
        ))),
    }
}

/// `family.place` of a family hosted on a wall: on a side of the wall, `along` it from its start.
async fn hang<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
    args: &Args,
    prepared: Prepared,
) -> Answer {
    let document = &edit.target;
    let (Some(host_id), Some(side), Some(along)) =
        (args.host.clone(), args.side.clone(), args.along.clone())
    else {
        return Ok(Err(
            "A placed family needs an id, a family, a type of it, a wall, a side of the wall and \
             a position along it."
                .to_owned(),
        ));
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
    let host = match hosting::host_for(
        txn,
        project,
        document,
        &host_id,
        Which::Role(&side),
        what_hosts(&prepared.recipe),
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let mut props = BTreeMap::new();
    props.insert(
        CATEGORY_KEY.to_owned(),
        records::write_category(prepared.category),
    );
    // On its wall's level: tied as the wall's base is, and on no level by number.
    let base = match construct_levels::base_of(&host.record) {
        Ok(base) => base,
        Err(e) => return Ok(Err(e.to_string())),
    };
    construct_levels::write_base(&mut props, base.as_ref());
    let mut element = Element {
        id: prepared.id.clone(),
        document: document.clone(),
        kind: kind_of(prepared.category).to_owned(),
        name: args.name.clone().unwrap_or_else(|| prepared.id.to_string()),
        level: 0,
        placement: host.record.placement,
        recipe: prepared.placed,
        props,
        references: Vec::new(),
    };
    placing::write(&mut element, &host_id, &host.face, along, up);
    element.placement = match fit(txn, project, document, &host, &element).await? {
        Ok(placement) => placement,
        Err(why) => return Ok(Err(why)),
    };
    write_new(txn, project, document, &element, prepared.loaded).await
}

/// What a wall hosts, as a refusal says it: the family's own words for what is hung on it.
fn what_hosts(recipe: &FamilyRecipe) -> &'static str {
    if recipe.hosting == FamilyHosting::Face {
        "casework or a fixture"
    } else {
        "a door or a window"
    }
}

/// A new element's record written and its pointer put: the whole result of placing it.
async fn write_new<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    element: &Element,
    loaded: Option<Change>,
) -> Answer {
    let hash = write_record(txn, project, element).await?;
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: element.id.clone(),
    };
    txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    let mut changes: Vec<Change> = loaded.into_iter().collect();
    changes.push(Change {
        piece: element.id.clone(),
        before: None,
        after: Some(hash),
    });
    Ok(Ok(Applied {
        changes,
        ..Applied::default()
    }))
}

/// A typed point's one coordinate in whole millimetres, as a placement holds it.
fn whole_mm(what: &str, typed: &str) -> Result<i64, String> {
    let at = length(what, typed)?;
    at.whole_mm().ok_or_else(|| {
        format!("{what}, {at}, is not a whole number of millimetres, as every placement is.")
    })
}

/// A typed angle, with the metric display settings, as a heading. Interim: only whole quarter
/// turns are built ([`Heading::quarter_turn`]), so any other angle is refused here in words.
fn heading_in(typed: &str) -> Result<Heading, String> {
    let id = Id::parse("family-turn").map_err(|e| e.to_string())?;
    let definition = Definition::new(id, "The turn", Kind::Angle, Holder::Instance)
        .map_err(|e| e.to_string())?;
    let heading = match definition.parse(typed, &units()) {
        Ok(Value::Angle(angle)) => Heading::of(angle),
        Ok(_) => return Err("The turn takes an angle.".to_owned()),
        Err(e) => return Err(e.to_string()),
    };
    heading.quarter_turn().map(|_| heading)
}

/// The level a standing family stands on and its offset above it, as `level` and `offset` say
/// (a level's ID or an old page's number; a length): the tie, or none for no level. The inner
/// error is the refusal, in words.
async fn tie_in<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    level: Option<&serde_json::Value>,
    offset: Option<&str>,
) -> Result<Result<Option<Tie>, String>, StoreError> {
    let base = match levels::tie_of_new(txn, project, document, level).await? {
        Ok(base) => base,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(match (base, offset) {
        (base, None) => Ok(base),
        (Some(tie), Some(text)) => {
            length("The offset from the level", text).map(|offset| Some(Tie { offset, ..tie }))
        }
        (None, Some(_)) => Err("An offset is measured from a level, and none is given.".to_owned()),
    })
}

/// `family.place` of a family that stands on a level: at the point `x`, `y`, turned by `turn`,
/// on the level `level` at `up` above it.
async fn stand<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
    args: &Args,
    prepared: Prepared,
) -> Answer {
    let document = &edit.target;
    let name = &prepared.recipe.name;
    if args.host.is_some() || args.side.is_some() || args.along.is_some() {
        return Ok(Err(format!(
            "{name} stands on a level: it takes a point (x and y), a turn and a level, not a wall."
        )));
    }
    let (Some(x), Some(y)) = (&args.x, &args.y) else {
        return Ok(Err(format!(
            "{name} needs the point it stands at: x and y."
        )));
    };
    let (x, y) = match (whole_mm("The point's x", x), whole_mm("The point's y", y)) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(why), _) | (_, Err(why)) => return Ok(Err(why)),
    };
    let heading = match args.turn.as_deref().map(heading_in).transpose() {
        Ok(heading) => heading.unwrap_or_default(),
        Err(why) => return Ok(Err(why)),
    };
    let base = match tie_in(
        txn,
        project,
        document,
        args.level.as_ref(),
        args.up.as_deref(),
    )
    .await?
    {
        Ok(base) => base,
        Err(why) => return Ok(Err(why)),
    };
    let mut props = BTreeMap::new();
    props.insert(
        CATEGORY_KEY.to_owned(),
        records::write_category(prepared.category),
    );
    construct_levels::write_base(&mut props, base.as_ref());
    heading.write(&mut props);
    let element = Element {
        id: prepared.id.clone(),
        document: document.clone(),
        kind: kind_of(prepared.category).to_owned(),
        name: args.name.clone().unwrap_or_else(|| prepared.id.to_string()),
        level: 0,
        placement: Placement { x, y, z: 0 },
        recipe: prepared.placed,
        props,
        references: Vec::new(),
    };
    write_new(txn, project, document, &element, prepared.loaded).await
}

/// Whether a placed family stands on a level: it is hosted on nothing.
fn is_standing(element: &Element) -> bool {
    matches!(placing::read(element), Ok(None))
}

/// `family.move` of a family that stands on a level: another point, another level, another offset
/// above it; any of them.
async fn stand_to<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    before: &Element,
    args: &Args,
) -> Result<Result<Element, String>, StoreError> {
    if args.host.is_some() || args.side.is_some() || args.along.is_some() {
        return Ok(Err(format!(
            "{} stands on a level: move it to another point (x and y), level or offset, not along a wall.",
            before.name
        )));
    }
    if args.x.is_none() && args.y.is_none() && args.up.is_none() && args.level.is_none() {
        return Ok(Err(
            "A move needs a point (x and y), a level or an offset above it.".to_owned(),
        ));
    }
    let mut after = before.clone();
    if let Some(typed) = &args.x {
        match whole_mm("The point's x", typed) {
            Ok(mm) => after.placement.x = mm,
            Err(why) => return Ok(Err(why)),
        }
    }
    if let Some(typed) = &args.y {
        match whole_mm("The point's y", typed) {
            Ok(mm) => after.placement.y = mm,
            Err(why) => return Ok(Err(why)),
        }
    }
    if args.up.is_some() || args.level.is_some() {
        let current = match construct_levels::base_of(before) {
            Ok(current) => current,
            Err(e) => return Ok(Err(e.to_string())),
        };
        // Its level as it is, unless another is named; its offset as it is, unless one is given
        // (a new level starts at none).
        let level = match (&args.level, &current) {
            (Some(level), _) => Some(level.clone()),
            (None, Some(tie)) => Some(serde_json::Value::String(tie.level.to_string())),
            (None, None) => None,
        };
        let offset = match (&args.up, &args.level, &current) {
            (Some(up), _, _) => Some(up.clone()),
            (None, None, Some(tie)) => Some(tie.offset.to_string()),
            _ => None,
        };
        let base = match tie_in(txn, project, document, level.as_ref(), offset.as_deref()).await? {
            Ok(base) => base,
            Err(why) => return Ok(Err(why)),
        };
        construct_levels::write_base(&mut after.props, base.as_ref());
    }
    if after == *before {
        return Ok(Err(format!("{} is already there.", before.name)));
    }
    Ok(Ok(after))
}

/// `family.turn`: a family that stands on a level, turned to `turn` about its z axis.
fn turn_to(before: &Element, args: &Args) -> Result<Element, String> {
    if !is_standing(before) {
        return Err(format!(
            "{} is hosted on a wall, which turns it: flip it to the wall's other side with family.flip.",
            before.name
        ));
    }
    let Some(typed) = &args.turn else {
        return Err("Which turn? Give the angle it should face, such as 90 degrees.".to_owned());
    };
    let heading = heading_in(typed)?;
    let current = Heading::read(before)?;
    let (new, old) = (heading.quarter_turn()?, current.quarter_turn()?);
    if new == old {
        return Err(format!("{} already faces that way.", before.name));
    }
    let mut after = before.clone();
    heading.write(&mut after.props);
    Ok(after)
}

/// A placed family's instance and its family recipe: the document's family it names, or for a
/// copy written before families were held in documents, the library's version it names.
async fn instance_of<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    element: &Element,
) -> Result<Result<(Instance, Arc<FamilyRecipe>), String>, StoreError> {
    let named =
        |e: construct_model::geometry::GeometryError| format!("{}: {}", element.name, e.message);
    let recipe = match records::recipe_at(txn, project, document, element, At::Now).await? {
        Ok(recipe) => recipe,
        Err(e) => return Ok(Err(named(e))),
    };
    let instance = match Instance::of(&recipe) {
        Ok(instance) => instance,
        Err(e) => return Ok(Err(named(e))),
    };
    let library = Library::baked();
    Ok(families::resolve(&*library, &instance)
        .map(|resolved| (instance, resolved.recipe))
        .map_err(|e| e.message))
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

/// What the wall `element` is hosted on hosts, as a refusal says it, by its family's hosting; the
/// door and window words when its family is not known.
async fn what_of<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    element: &Element,
) -> Result<&'static str, StoreError> {
    Ok(instance_of(txn, project, document, element)
        .await?
        .map_or("a door or a window", |(_, recipe)| what_hosts(&recipe)))
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
    let (instance, recipe) = match instance_of(txn, project, document, before).await? {
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
    if is_standing(before) {
        return stand_to(txn, project, document, before, args).await;
    }
    if args.along.is_none() && args.up.is_none() && args.host.is_none() && args.side.is_none() {
        return Ok(Err(
            "A move needs a position along the wall, a height, another wall or another side."
                .to_owned(),
        ));
    }
    let what = what_of(txn, project, document, before).await?;
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
            hosting::host_for(txn, project, document, &new_host, Which::Role(&role), what).await?
        }
        None => {
            hosting::host_for(txn, project, document, &new_host, Which::Name(&face), what).await?
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
    let what = what_of(txn, project, document, before).await?;
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
        what,
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
    let host = match hosting::host_for(txn, project, document, &host_id, Which::Role(other), what)
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
    let (_, recipe) = match instance_of(txn, project, document, before).await? {
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
    if is_standing(&after) {
        return Ok(Ok(after));
    }
    let what = what_of(txn, project, document, &after).await?;
    let (host_id, face, _, _) = match hosted_on(&after) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let host = match hosting::host_for(txn, project, document, &host_id, Which::Name(&face), what)
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
    if let Err(refused) = loading::generic_rules(txn, project, edit).await? {
        return Ok(Err(refused));
    }
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
    /// What a placed one is put on: `unhosted` (it stands on a level), `wall` (a door or a window,
    /// cutting an opening), `face` (casework or a fixture hung against a wall's face) or `floor`.
    pub hosting: String,
    /// Its types, in the order its recipe names them.
    pub types: Vec<TypeView>,
    /// The values each placed one holds, with their defaults.
    pub values: Vec<ValueView>,
    /// Whether the document holds it; a library family it does not hold is loaded when one is
    /// first placed.
    pub in_document: bool,
    /// Where the document's family came from (construct-families' `Source`, written:
    /// `library:<id>`, `text` or `generated:<template>`); empty for one it does not hold.
    pub source: String,
    /// How many copies of it are placed in the document.
    pub placed: u32,
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

/// What a family is placed on, as the page names it.
fn hosting_name(hosting: &FamilyHosting) -> &'static str {
    match hosting {
        FamilyHosting::Unhosted => "unhosted",
        FamilyHosting::Wall { .. } => "wall",
        FamilyHosting::Floor => "floor",
        FamilyHosting::Face => "face",
    }
}

/// The library's families, with their types and the values each instance holds, in the order of
/// their content ids. Interim as the baked library is (construct-regen's `families`).
#[must_use]
pub fn read_library() -> Vec<FamilyView> {
    let library = Library::baked();
    library
        .recipes()
        .map(|(content, recipe)| view_of(content, recipe))
        .collect()
}

/// The family `recipe`, of version `content`, as the page lists it: not in the document, none
/// placed, until the caller says otherwise.
pub(crate) fn view_of(content: &ContentId, recipe: &FamilyRecipe) -> FamilyView {
    let table = CategoryTable::new();
    let category = category_of(recipe);
    FamilyView {
        id: recipe.id.to_string(),
        content: content.as_str().to_owned(),
        name: recipe.name.clone(),
        kind: kind_of(category).to_owned(),
        category: category.0,
        category_name: table.get(category).map(|c| c.name).unwrap_or_default(),
        hosting: hosting_name(&recipe.hosting).to_owned(),
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
        in_document: false,
        source: String::new(),
        placed: 0,
    }
}
