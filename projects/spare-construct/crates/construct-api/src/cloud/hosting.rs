//! The commands that host an element on another (CON-A4-F3-P1): a door on a face of a wall, at an
//! offset along that face and up it, through the same edit log as `element.*`, with the same
//! access check (`model.edit`) and the same claims. How a hosted element sits in its record, and
//! where that puts it, is construct-regen's [`hosting`] module, the one the regenerator builds a
//! wall with its doors through, so what a command allows and what is built cannot differ.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `hosted.place` | `element`, `kind` (default `door`), `name`, `host`, `face` (the persistent name of one of the host's sides), `along`, `up` (typed lengths; `up` defaults to 0), `recipe` | A new element on the host's level, hosted on that face at those offsets |
//! | `hosted.move` | `element`, `along`, and `up` (kept when left out) | The element's offsets: it moves along its host |
//!
//! Each is refused, as its result and in words, when the host is missing (`"There is no element
//! <host> in this model to host it."`), when it is not a wall (`"<name> is not a wall: only a wall
//! hosts a door."`), when the face is not one of its sides, when an offset is not a length, and
//! when the element would not fit with what is already hosted there, in the kernel builder's own
//! words ([`construct_regen::kernel::fits`]: `"door D-1: it crosses the wall's end"`), on every
//! edition, so a model drawn on the stand-in holds no door the kernel's builder would refuse.
//!
//! The rules the other commands keep for hosting are here too: deleting an element that still
//! hosts others is refused, naming how many; a hosted element moves along its host, not by
//! `element.move`, and its host is not changed through its references ([`element_rules`], asked
//! before every `element.*` command runs); and an undo that would leave a hosted element without
//! its host is turned back whole ([`undo_rules`], asked by the undo once it has written).
//!
//! Nothing here builds a shape or reads a derived row: the fit is the builder's rules asked
//! without the engine, from the records as they are now. Nothing reads a clock or a random number:
//! the claim's time is the orderer's.

use crate::cloud::elements::write_record;
use construct_model::element::{Element, Placement};
use construct_model::geometry::{Edition, FaceName, Hosted, Recipe};
use construct_model::keys::Place as ModelPlace;
use construct_model::rows::Model;
use construct_params::{Definition, Holder, Kind, Length, Units, Value};
use construct_regen::hosting::{self, Hosting};
use construct_regen::kernel;
use construct_regen::levels;
use construct_regen::parameters::{self as records, At, CATEGORY_KEY};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context, Outcome, claims};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Every command here.
pub const COMMANDS: [&str; 2] = [PLACE, MOVE];
/// Puts a new element on a face of its host.
pub const PLACE: &str = "hosted.place";
/// Moves a hosted element along its host.
pub const MOVE: &str = "hosted.move";
/// The kind a placed element is when its command names none.
const DOOR: &str = "door";

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The arguments of both commands; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    name: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    host: Option<Id>,
    face: Option<String>,
    along: Option<String>,
    up: Option<String>,
    recipe: Option<Recipe>,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

/// A host, checked: its record, its recipe as built, the face an element sits on, and the
/// document's edition, which names its faces and whose builder's rules a fit is asked by.
pub(crate) struct Host {
    pub(crate) record: Element,
    pub(crate) recipe: Recipe,
    pub(crate) face: FaceName,
    pub(crate) edition: Edition,
}

/// How a host's side is named to [`host_for`]: by the face's persistent name, or by its role.
pub(crate) enum Which<'a> {
    /// The persistent name of one of the host's sides.
    Name(&'a str),
    /// A side's role: `exterior` or `interior`.
    Role(&'a str),
}

/// A typed length, such as `1200`, `1.2 m` or `1200mm`, read as construct-params reads a length
/// parameter's value, with the metric display settings.
pub(crate) fn length(what: &str, typed: &str) -> Result<Length, String> {
    let id = Id::parse("hosting-offset").map_err(|e| e.to_string())?;
    let definition =
        Definition::new(id, what, Kind::Length, Holder::Instance).map_err(|e| e.to_string())?;
    match definition.parse(typed, &Units::default()) {
        Ok(Value::Length(length)) => Ok(length),
        Ok(_) => Err(format!("{what} takes a length")),
        Err(e) => Err(e.to_string()),
    }
}

/// The edition a document records.
async fn edition_of<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
) -> Result<Option<Edition>, StoreError> {
    let place = ModelPlace::Model {
        project: project.clone(),
        model: document.clone(),
    };
    let model: Option<Model> = read_row(txn, &place.key()).await?;
    Ok(model.map(|model| model.edition))
}

/// The host `host` and its face `face`, checked: in this document, a wall, and the face one of its
/// sides. `what` is what the wall hosts, for the refusal (`a door`). The inner error is the
/// refusal.
pub(crate) async fn host_for<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    host: &Id,
    face: Which<'_>,
    what: &str,
) -> Result<Result<Host, String>, StoreError> {
    let Some((_, record)) = records::read_element(txn, project, document, host).await? else {
        return Ok(Err(format!(
            "There is no element {host} in this model to host it."
        )));
    };
    if !records::has_shape(&record) || !hosting::is_host(&record.recipe) {
        return Ok(Err(format!(
            "{} is not a wall: only a wall hosts {what}.",
            record.name
        )));
    }
    let recipe = match levels::recipe_at(txn, project, document, &record, At::Now).await? {
        Ok(recipe) => recipe,
        Err(e) => {
            return Ok(Err(format!(
                "{} can't be built: {}",
                record.name, e.message
            )));
        }
    };
    let Some(edition) = edition_of(txn, project, document).await? else {
        return Ok(Err("That model is not in this project.".to_owned()));
    };
    let face = match face {
        Which::Name(name) => hosting::side(&edition, &recipe, name),
        Which::Role(role) => hosting::side_by_role(&edition, &recipe, role),
    };
    let face = match face {
        Ok(face) => face,
        Err(why) => return Ok(Err(format!("{}: {why}.", record.name))),
    };
    Ok(Ok(Host {
        record,
        recipe,
        face,
        edition,
    }))
}

/// The floor `host`, checked: in this document, a floor, and its top face, which an opening drawn
/// in its plan is hosted on. The inner error is the refusal, in words.
pub(crate) async fn floor_for<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    host: &Id,
) -> Result<Result<Host, String>, StoreError> {
    let Some((_, record)) = records::read_element(txn, project, document, host).await? else {
        return Ok(Err(format!(
            "There is no element {host} in this model to hold an opening."
        )));
    };
    if !is_floor(&record) {
        return Ok(Err(format!(
            "{} is not a floor: an opening is drawn in a floor's plan.",
            record.name
        )));
    }
    let recipe = match levels::recipe_at(txn, project, document, &record, At::Now).await? {
        Ok(recipe) => recipe,
        Err(e) => {
            return Ok(Err(format!(
                "{} can't be built: {}",
                record.name, e.message
            )));
        }
    };
    let Some(edition) = edition_of(txn, project, document).await? else {
        return Ok(Err("That model is not in this project.".to_owned()));
    };
    let names = match construct_regen::face_names(&edition, &recipe) {
        Ok(names) => names,
        Err(e) => return Ok(Err(format!("{}: {}", record.name, e.message))),
    };
    let Some(face) = names.into_iter().find(|n| n.role == "top") else {
        return Ok(Err(format!("{} has no top face.", record.name)));
    };
    Ok(Ok(Host {
        record,
        recipe,
        face,
        edition,
    }))
}

/// Whether `record` is a floor: its recipe is a floor's.
pub(crate) fn is_floor(record: &Element) -> bool {
    construct_regen::floor_plan::is_floor(&record.recipe)
}

/// Where `element` sits in `host`'s frame, from the records as they are now, and its recipe as its
/// host's builder is handed it ([`hosting::seat`]: a placed family's turned to face out of its
/// side, as the regenerator builds it, so a family on a wall along y fits as it is built). The
/// inner error is the refusal, in the words of what refused it.
async fn frame_at<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    host: &Host,
    element: &Element,
    hosting: &Hosting,
) -> Result<Result<(Recipe, Placement), String>, StoreError> {
    let recipe = match levels::recipe_at(txn, project, document, element, At::Now).await? {
        Ok(recipe) => recipe,
        Err(e) => {
            return Ok(Err(format!(
                "{} can't be built: {}",
                element.name, e.message
            )));
        }
    };
    // An embedded curtain wall is fitted as the plain opening it cuts in its host.
    match hosting::cut_of(
        &host.edition,
        &host.record,
        &host.recipe,
        element,
        &recipe,
        hosting,
    ) {
        Ok(Some((at, cut))) => return Ok(Ok((cut, at))),
        Ok(None) => {}
        Err(e) => return Ok(Err(e.message)),
    }
    let seated = hosting::seat(
        &host.edition,
        &host.record,
        &host.recipe,
        element,
        &recipe,
        hosting,
    );
    Ok(seated
        .map(|(at, recipe)| (recipe, at))
        .map_err(|e| e.message))
}

/// Where `element`, hosted on `host`, sits in the model, if it fits there with everything else
/// hosted on it, asked by the kernel builder's own rules. The inner error is the refusal: the
/// builder's own words when it does not fit.
pub(crate) async fn fit<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    host: &Host,
    element: &Element,
) -> Result<Result<Placement, String>, StoreError> {
    let hosting = match hosting::read(element) {
        Ok(Some(hosting)) => hosting,
        Ok(None) => return Ok(Err(format!("{} is not hosted.", element.name))),
        Err(why) => return Ok(Err(why)),
    };
    let mut placed: Vec<(String, Recipe, Placement)> = Vec::new();
    for (id, (_, other)) in records::read_document(txn, project, document).await? {
        if id == element.id {
            continue;
        }
        let Ok(Some(theirs)) = hosting::read(&other) else {
            continue;
        };
        if *theirs.host() != host.record.id {
            continue;
        }
        match frame_at(txn, project, document, host, &other, &theirs).await? {
            Ok((recipe, at)) => placed.push((id.to_string(), recipe, at)),
            Err(why) => return Ok(Err(why)),
        }
    }
    let (recipe, at) = match frame_at(txn, project, document, host, element, &hosting).await? {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    placed.push((element.id.to_string(), recipe, at));
    let list: Vec<Hosted<'_>> = placed
        .iter()
        .map(|(key, recipe, at)| Hosted {
            key,
            recipe,
            at: *at,
        })
        .collect();
    if let Err(e) = kernel::fits(&host.edition, &host.recipe, &list) {
        return Ok(Err(e.message));
    }
    Ok(hosting::in_model(host.record.placement, at).map_err(|e| e.message))
}

/// Runs a `hosted.*` command.
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
        PLACE => place(txn, project, edit, args).await,
        MOVE => move_along(txn, project, context, edit, args, claim_ms).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// `hosted.place`: a new element on a face of its host.
async fn place<T: Txn>(txn: &mut T, project: &Id, edit: &Edit, args: Args) -> Answer {
    let document = &edit.target;
    let (Some(id), Some(host_id), Some(face), Some(along), Some(recipe)) =
        (args.element, args.host, args.face, args.along, args.recipe)
    else {
        return Ok(Err(
            "A hosted element needs an id, a host, a face of it, an offset along it and a recipe."
                .to_owned(),
        ));
    };
    if records::read_element(txn, project, document, &id)
        .await?
        .is_some()
    {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let kind = args.kind.unwrap_or_else(|| DOOR.to_owned());
    if kind == records::TYPE_KIND
        || kind == records::TABLE_KIND
        || kind == construct_regen::families::RECORD_KIND
        || id.as_str() == records::TABLE_ID
    {
        return Ok(Err(
            "Types and the parameter table are changed by their own commands, not as elements."
                .to_owned(),
        ));
    }
    let category = match records::category_of(&recipe) {
        Ok(category) => category,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let along = match length("The offset along its host", &along) {
        Ok(length) => length,
        Err(why) => return Ok(Err(why)),
    };
    let up = match length("The offset up its host", args.up.as_deref().unwrap_or("0")) {
        Ok(length) => length,
        Err(why) => return Ok(Err(why)),
    };
    let host = match host_for(
        txn,
        project,
        document,
        &host_id,
        Which::Name(&face),
        "a door",
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let mut props = BTreeMap::new();
    props.insert(CATEGORY_KEY.to_owned(), records::write_category(category));
    // On its host's level: tied as its host's base is, and on no level by number.
    let base = match construct_levels::base_of(&host.record) {
        Ok(base) => base,
        Err(e) => return Ok(Err(e.to_string())),
    };
    construct_levels::write_base(&mut props, base.as_ref());
    let mut element = Element {
        id: id.clone(),
        document: document.clone(),
        kind,
        name: args.name.unwrap_or_else(|| id.to_string()),
        level: 0,
        placement: host.record.placement,
        recipe,
        props,
        references: Vec::new(),
    };
    hosting::write(&mut element, &host_id, &host.face, along, up);
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

/// `hosted.move`: a hosted element's offsets, so it moves along its host. An older record, hosted
/// by its position, gets its offsets here and moves with its host from then on.
async fn move_along<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
    args: Args,
    claim_ms: u64,
) -> Answer {
    let document = &edit.target;
    let Some(id) = args.element else {
        return Ok(Err("Which element?".to_owned()));
    };
    let Some(along) = args.along else {
        return Ok(Err(
            "A move along a host needs an offset along it.".to_owned()
        ));
    };
    let Some((hash, before)) = records::read_element(txn, project, document, &id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    let hosting = match hosting::read(&before) {
        Ok(Some(hosting)) => hosting,
        Ok(None) => {
            return Ok(Err(format!(
                "{} is not hosted: move it with element.move.",
                before.name
            )));
        }
        Err(why) => return Ok(Err(why)),
    };
    let along = match length("The offset along its host", &along) {
        Ok(length) => length,
        Err(why) => return Ok(Err(why)),
    };
    let holder =
        claims::held_by_another(txn, project, document, &id, &edit.person, context.now_ms).await?;
    if let Some(claim) = holder {
        return Ok(Err(format!("{} is working on this.", claim.name)));
    }
    let host = match host_for(
        txn,
        project,
        document,
        hosting.host(),
        Which::Name(hosting.face()),
        "a door",
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let up = match (&args.up, &hosting) {
        (Some(typed), _) => length("The offset up its host", typed),
        (None, Hosting::OnFace { up, .. }) => Ok(*up),
        (None, Hosting::InPlan { .. }) => Err(format!(
            "{} is drawn in its floor's plan: change its outline to move it.",
            before.name
        )),
        (None, Hosting::ByPosition { .. }) => {
            match frame_at(txn, project, document, &host, &before, &hosting).await? {
                Ok((_, at)) => Length::checked_from_mm(at.z)
                    .ok_or_else(|| format!("{} is too far from its host.", before.name)),
                Err(why) => Err(why),
            }
        }
    };
    let up = match up {
        Ok(up) => up,
        Err(why) => return Ok(Err(why)),
    };
    let mut after = before.clone();
    hosting::write(&mut after, hosting.host(), &host.face, along, up);
    after.placement = match fit(txn, project, document, &host, &after).await? {
        Ok(placement) => placement,
        Err(why) => return Ok(Err(why)),
    };
    let person: Option<Named> = read_row(txn, &Place::Person(edit.person.clone()).key()).await?;
    let name = person.map_or_else(|| edit.person.to_string(), |person| person.name);
    let mine = Claim {
        person: edit.person.clone(),
        name,
        // milliseconds since 1970 plus a claim's length: far below u64::MAX
        until_ms: context.now_ms.saturating_add(claim_ms),
    };
    let new_hash = write_record(txn, project, &after).await?;
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: id.clone(),
    };
    txn.put(pointer.key(), to_row(&new_hash.as_str())?).await?;
    let claim = claims::take(txn, project, document, &id, mine).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: id,
            before: Some(hash),
            after: Some(new_hash),
        }],
        claims: vec![claim],
        ..Applied::default()
    }))
}

/// How many of a document's current records are hosted on `host`.
async fn hosted_on<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    host: &Id,
) -> Result<usize, StoreError> {
    let all = records::read_document(txn, project, document).await?;
    Ok(all
        .values()
        .filter(|(_, record)| hosting::host_of(record).as_ref() == Some(host))
        .count())
}

/// "1 element", "2 elements".
fn elements(n: usize) -> String {
    if n == 1 {
        "1 element".to_owned()
    } else {
        format!("{n} elements")
    }
}

/// What the hosting rules of an `element.*` command read of its arguments.
#[derive(Deserialize)]
struct ElementArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    references: Option<serde_json::Value>,
}

/// The hosting rules of the `element.*` commands, asked before the command runs: an element that
/// still hosts others is not deleted, naming how many; one hosted on a face moves along its host
/// with `hosted.move`, not by `element.move`; and its host is not changed through its references
/// by `element.set`. The inner error is the refusal. An edit these rules have nothing to say of
/// (another command, arguments that do not read, an element that is not there) is left to the
/// command, which answers it as it always has.
///
/// # Errors
///
/// A [`StoreError`] when the store fails.
pub async fn element_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
) -> Result<Result<(), String>, StoreError> {
    let document = &edit.target;
    let Ok(ElementArgs {
        element: Some(id),
        references,
    }) = serde_json::from_value(edit.args.clone())
    else {
        return Ok(Ok(()));
    };
    let Some((_, before)) = records::read_element(txn, project, document, &id).await? else {
        return Ok(Ok(()));
    };
    let on_face = matches!(hosting::read(&before), Ok(Some(Hosting::OnFace { .. })));
    let references = references.is_some();
    Ok(match edit.cmd.as_str() {
        "element.delete" => match hosted_on(txn, project, document, &before.id).await? {
            0 => Ok(()),
            n => Err(format!(
                "{} still hosts {}: delete them, or host them elsewhere, first.",
                before.name,
                elements(n)
            )),
        },
        "element.move" if on_face => Err(format!(
            "{} is hosted: move it along its host with hosted.move.",
            before.name
        )),
        "element.set" if on_face && references => Err(format!(
            "{}'s host is set by hosted.place and hosted.move, not by its references.",
            before.name
        )),
        _ => Ok(()),
    })
}

/// The hosting rule of an undo or a redo, asked once it has written `written` and deleted the
/// pieces `deleted`: no element of the document it wrote, or hosted on a piece it deleted, is
/// left without its host. The inner error is the refusal, without the undo's own opening words.
///
/// # Errors
///
/// A [`StoreError`] when the store fails.
pub(crate) async fn undo_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    written: &[Element],
    deleted: &[Id],
) -> Result<Result<(), String>, StoreError> {
    let all = records::read_document(txn, project, document).await?;
    let wrote: BTreeSet<&Id> = written.iter().map(|record| &record.id).collect();
    let gone: BTreeSet<&Id> = deleted.iter().collect();
    for (id, (_, record)) in &all {
        let Some(host) = hosting::host_of(record) else {
            continue;
        };
        if (wrote.contains(id) || gone.contains(&host)) && !all.contains_key(&host) {
            return Ok(Err(format!(
                "{} would be left without its host, {host}.",
                record.name
            )));
        }
    }
    Ok(Ok(()))
}
