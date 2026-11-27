//! The modify commands (CON-A3-F4-P1, job 10.2): move, copy, rotate, mirror and array a selection
//! of elements, through the same edit log as every edit, with the same access check
//! (`model.edit`) and the same claims. What each writes is construct-modify's [`plan`], a pure
//! function of the document's records and the request, so a command and a replay agree; each is
//! one entry, so one undo takes back all it wrote ([`crate::cloud::undo`]).
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `modify.move` | `elements`, `by` | Each element moved by `by`; what it hosts follows |
//! | `modify.copy` | `elements`, `by`, `copy` | A copy of each, `by` away, its ID made from the key `copy` |
//! | `modify.rotate` | `elements`, `about`, `angle`, or `from` and `to`; `copy` | Each turned about `about`; with `copy`, a turned copy of each instead |
//! | `modify.mirror` | `elements`, `line`; `copy` | Each mirrored about `line`; with `copy`, a mirrored copy of each instead |
//! | `modify.array` | `elements`, `copy`, `count`, and `spacing` or `span` (linear), or `about` with `angle` or `sweep` (radial) | `count - 1` copies of each, every one a plain element |
//!
//! `elements` is a list of element IDs. A point (`by`, `about`, `spacing`, `span`, `from`, `to`)
//! is `[x, y]` as typed lengths (`["1200 mm", "0"]`); `line` is two points; `angle` and `sweep`
//! are typed angles, any angle (`90°`, `37.5°`); `from` and `to` are two directions, the turn
//! taking the first to the second; `count` is how many in all, the originals counted (a `sweep` of
//! a full turn spreads them evenly round); `copy` is a key the copies' IDs are made from
//! (`<copy>.<n>`; an array's `<copy>.<k>.<n>`), fresh for each command, as a new element's ID is.
//!
//! Every point written is the nearest grid step to where the motion takes it, rounded once
//! (construct-modify's `exact`): exact for a move, a quarter turn, a 3:4:5 turn or a mirror that
//! lands it, within half a step and 2^-94 of a step of the exact turn for any other angle.
//!
//! Refused, as the result and in words, writing nothing: what construct-modify refuses (an
//! element not in the model or of a kind it does not move, a point past the model's limit, a
//! hosted element moved off its host or turned alone, an older door hosted by its position, a
//! copy's ID already taken), another person's claim on an element it rewrites, and anything the
//! builders would refuse of what it writes, in their words (a door, a family or an opening is
//! hosted only on a wall along x or y today, so a wall that hosts one is turned off the axes only
//! when the builder hosts them at any angle): every
//! record written is filled and read by the builders' own readers as regeneration reads it
//! ([`construct_regen::levels::recipe_at`], [`construct_regen::face_names`]; a room by
//! [`construct_regen::rooms::found_at`]), every grid and plane is read again with their names
//! ([`grids::check`]), and every host moved or copied with what it hosts, or that an element was
//! moved or copied along, is asked whether everything on it still fits, by the kernel builder's
//! rules ([`hosting::fit`]). A hosted element whose record is written takes the placement that
//! fit gives it, as `hosted.move` writes one.
//!
//! Geometry only through the kernel's interface as it stands: nothing here builds a shape; the
//! fit is the builder's rules asked without the engine. Nothing reads a clock or a random number:
//! the claim's time is the orderer's. Not here yet (the second half): the page's tools.

use crate::cloud::elements::write_record;
use crate::cloud::grids;
use crate::cloud::hosting::{self, Which};
use construct_model::element::Element;
use construct_model::keys::Place as ModelPlace;
use construct_model::rows::Model;
use construct_modify::array;
use construct_modify::{Copies, Hand, Motion, Plan, Request, Sense, Steps, Turn, hands_of, plan};
use construct_params::{Angle, Definition, Holder, Kind, Units, Value};
use construct_regen::families::{self as placed, FamilySource, Instance};
use construct_regen::hosting::{self as seated, Hosting};
use construct_regen::parameters::{self as records, At, Context, TYPE_KIND};
use construct_types::Types;
use kit_core::Id;
use kit_ops::{Change, Claim, ClaimChange, Edit};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Moves a selection.
pub const MOVE: &str = "modify.move";
/// Copies a selection by a displacement.
pub const COPY: &str = "modify.copy";
/// Turns a selection, or a copy of it, about a point.
pub const ROTATE: &str = "modify.rotate";
/// Mirrors a selection, or a copy of it, about a line.
pub const MIRROR: &str = "modify.mirror";
/// Copies a selection in a row or round a point.
pub const ARRAY: &str = "modify.array";
/// Every command here.
pub const COMMANDS: [&str; 5] = [MOVE, COPY, ROTATE, MIRROR, ARRAY];

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default)]
    elements: Vec<String>,
    by: Option<[String; 2]>,
    about: Option<[String; 2]>,
    angle: Option<String>,
    from: Option<[String; 2]>,
    to: Option<[String; 2]>,
    line: Option<[[String; 2]; 2]>,
    copy: Option<String>,
    count: Option<u32>,
    spacing: Option<[String; 2]>,
    span: Option<[String; 2]>,
    sweep: Option<String>,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

/// A typed point: `[x, y]` as two lengths, in grid steps.
fn point(sent: Option<&[String; 2]>, what: &str) -> Result<Steps, String> {
    let [x, y] = sent.ok_or_else(|| format!("This command needs {what}."))?;
    Ok([
        hosting::length(what, x)?.steps(),
        hosting::length(what, y)?.steps(),
    ])
}

/// A typed angle, read as construct-params reads an angle parameter's value.
fn angle(typed: &str, what: &str) -> Result<Angle, String> {
    let id = Id::parse("modify-angle").map_err(|e| e.to_string())?;
    let definition =
        Definition::new(id, what, Kind::Angle, Holder::Instance).map_err(|e| e.to_string())?;
    match definition.parse(typed, &Units::default()) {
        Ok(Value::Angle(angle)) => Ok(angle),
        Ok(_) => Err(format!("{what} takes an angle")),
        Err(e) => Err(e.to_string()),
    }
}

/// The turn a rotate asks for: by any angle, or from one direction to another.
fn turn(args: &Args) -> Result<Turn, String> {
    match (&args.angle, &args.from, &args.to) {
        (Some(typed), None, None) => Ok(Turn::of_angle(angle(typed, "The angle")?)),
        (None, Some(from), Some(to)) => Turn::between(
            point(Some(from), "a direction from")?,
            point(Some(to), "a direction to")?,
        )
        .map_err(why),
        _ => Err(
            "A turn is an angle, or a direction from and a direction to, one or the other."
                .to_owned(),
        ),
    }
}

/// A refusal of construct-modify's, in its words.
fn why(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// The motions of an array's copies.
fn array_motions(args: &Args) -> Result<Vec<Motion>, String> {
    let count = args
        .count
        .ok_or_else(|| "An array needs a count, the originals counted.".to_owned())?;
    let radial = args.about.is_some();
    match (radial, &args.spacing, &args.span, &args.angle, &args.sweep) {
        (false, Some(spacing), None, None, None) => {
            array::linear(point(Some(spacing), "a spacing")?, count).map_err(why)
        }
        (false, None, Some(span), None, None) => {
            array::linear_span(point(Some(span), "a span")?, count).map_err(why)
        }
        (true, None, None, Some(typed), None) => array::radial(
            point(args.about.as_ref(), "a centre")?,
            angle(typed, "The angle")?,
            count,
        )
        .map_err(why),
        (true, None, None, None, Some(typed)) => array::radial_sweep(
            point(args.about.as_ref(), "a centre")?,
            angle(typed, "The sweep")?,
            count,
        )
        .map_err(why),
        _ => Err(
            "An array is linear, by a spacing or a span, or radial, about a centre by an angle \
             or a sweep: one of the four."
                .to_owned(),
        ),
    }
}

/// What a command asks for.
fn request(cmd: &str, args: &Args) -> Result<Request, String> {
    let copies = |motions: Vec<Motion>| -> Result<Request, String> {
        let key = args
            .copy
            .clone()
            .ok_or_else(|| "Copies need a key their IDs are made from (copy).".to_owned())?;
        Ok(Request::Copy(Copies { key, motions }))
    };
    let one = |motion: Motion| match &args.copy {
        Some(_) => copies(vec![motion]),
        None => Ok(Request::Move(motion)),
    };
    match cmd {
        MOVE => {
            if args.copy.is_some() {
                return Err(format!("{MOVE} moves: {COPY} copies."));
            }
            Ok(Request::Move(Motion::by(point(
                args.by.as_ref(),
                "a displacement",
            )?)))
        }
        COPY => copies(vec![Motion::by(point(args.by.as_ref(), "a displacement")?)]),
        ROTATE => {
            let about = point(args.about.as_ref(), "a centre")?;
            one(Motion::turn_about(about, &turn(args)?))
        }
        MIRROR => {
            let [a, b] = args
                .line
                .as_ref()
                .ok_or_else(|| "This command needs a mirror line.".to_owned())?;
            let mirror = Motion::mirror(
                point(Some(a), "a mirror line")?,
                point(Some(b), "a mirror line")?,
            )
            .map_err(why)?;
            one(mirror)
        }
        ARRAY => copies(array_motions(args)?),
        other => Err(format!("Unknown command {other}.")),
    }
}

/// What the plan reads of the document beyond a record: a wall's side from its parameters, and a
/// placed family's hands from the library it is placed from.
struct Answers<'a> {
    context: &'a Context,
    types: &'a Types,
    families: &'a dyn FamilySource,
}

impl Sense for Answers<'_> {
    fn flipped(&self, wall: &Element) -> Result<bool, String> {
        let type_id = records::type_of(wall).ok_or_else(|| "it has no wall type".to_owned())?;
        let ty = self
            .types
            .get(&type_id)
            .ok_or_else(|| format!("no type has id {type_id}"))?;
        let instance = self.context.instance(wall, ty)?;
        construct_walls::placement(&self.context.params, ty, &instance)
            .map(|(_, flipped)| flipped)
            .map_err(why)
    }

    fn hands(&self, family: &Element) -> Result<Vec<Hand>, String> {
        let instance = Instance::of(&family.recipe).map_err(|e| e.message)?;
        let recipe = self
            .families
            .recipe(&instance.family)
            .ok_or_else(|| format!("its family {} is not in the library", instance.family))?;
        hands_of(&recipe, &instance)
    }
}

/// The edition a document records.
async fn edition_of<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
) -> Result<Result<construct_model::geometry::Edition, String>, StoreError> {
    let place = ModelPlace::Model {
        project: project.clone(),
        model: document.clone(),
    };
    let model: Option<Model> = read_row(txn, &place.key()).await?;
    Ok(model
        .map(|model| model.edition)
        .ok_or_else(|| "That model is not in this project.".to_owned()))
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
) -> Answer {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    let selection = match args
        .elements
        .iter()
        .map(|text| Id::parse(text).map_err(|_| format!("{text:?} is not an element's ID.")))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(selection) => selection,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let request = match request(&edit.cmd, &args) {
        Ok(request) => request,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let document = &edit.target;
    let (planned, hashes) = match plan_for(txn, project, document, &selection, &request).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };

    // Another person's claim on anything it rewrites refuses it, as it refuses any edit.
    for record in &planned.changed {
        let holder = claims::held_by_another(
            txn,
            project,
            document,
            &record.id,
            &edit.person,
            context.now_ms,
        )
        .await?;
        if let Some(claim) = holder {
            return Ok(Err(format!("{} is working on this.", claim.name)));
        }
    }
    let person: Option<Named> = read_row(txn, &Place::Person(edit.person.clone()).key()).await?;
    let name = person.map_or_else(|| edit.person.to_string(), |person| person.name);

    // Written in the staged transaction, so a refusal below leaves nothing.
    let mut changes = Vec::new();
    let mut taken: Vec<ClaimChange> = Vec::new();
    let made: BTreeSet<Id> = planned
        .made
        .iter()
        .map(|record| record.id.clone())
        .collect();
    let written: Vec<Element> = planned
        .changed
        .iter()
        .chain(&planned.made)
        .cloned()
        .collect();
    for record in &written {
        let before = hashes.get(&record.id).cloned();
        let after = put(txn, project, document, record).await?;
        changes.push(Change {
            piece: record.id.clone(),
            before,
            after: Some(after),
        });
        if !made.contains(&record.id) {
            let mine = Claim {
                person: edit.person.clone(),
                name: name.clone(),
                // milliseconds since 1970 plus a claim's length: far below u64::MAX
                until_ms: context.now_ms.saturating_add(claim_ms),
            };
            taken.push(claims::take(txn, project, document, &record.id, mine).await?);
        }
    }

    // What the builders would make of each record written, read as they read it.
    let edition = match edition_of(txn, project, document).await? {
        Ok(edition) => edition,
        Err(refusal) => return Ok(Err(refusal)),
    };
    for record in &written {
        if let Err(refusal) = buildable(txn, project, document, &edition, record).await? {
            return Ok(Err(refusal));
        }
    }
    if let Err(refusal) = grids::check(txn, project, document, &written).await? {
        return Ok(Err(refusal));
    }
    // Everything on each host still fits it, and what was written there sits where it fits.
    let rewritten: BTreeSet<Id> = written.iter().map(|record| record.id.clone()).collect();
    for host in &planned.hosts {
        match seat(txn, project, document, host, &rewritten).await? {
            Ok(placed) => {
                for (id, hash) in placed {
                    if let Some(change) = changes.iter_mut().find(|change| change.piece == id) {
                        change.after = Some(hash);
                    }
                }
            }
            Err(refusal) => return Ok(Err(refusal)),
        }
    }
    Ok(Ok(Applied {
        changes,
        claims: taken,
        ..Applied::default()
    }))
}

/// What `request` writes in the document as it stands, and each current record's content ID. The
/// inner error is the refusal, in construct-modify's words.
async fn plan_for<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    selection: &[Id],
    request: &Request,
) -> Result<Result<(Plan, BTreeMap<Id, Id>), String>, StoreError> {
    let all = records::read_document(txn, project, document).await?;
    let hashes: BTreeMap<Id, Id> = all
        .iter()
        .map(|(id, (hash, _))| (id.clone(), hash.clone()))
        .collect();
    let model: BTreeMap<Id, Element> = all
        .into_iter()
        .map(|(id, (_, record))| (id, record))
        .collect();
    let parameters = match records::read_context(txn, project, document).await? {
        Ok(parameters) => parameters,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let type_records: Vec<Element> = model
        .values()
        .filter(|record| record.kind == TYPE_KIND)
        .cloned()
        .collect();
    let types = match Types::from_records(&type_records, &parameters.params, &parameters.categories)
    {
        Ok(types) => types,
        Err(e) => return Ok(Err(why(e))),
    };
    let library = placed::baked();
    let answers = Answers {
        context: &parameters,
        types: &types,
        families: &*library,
    };
    Ok(plan(&model, selection, request, &answers)
        .map(|planned| (planned, hashes))
        .map_err(why))
}

/// Writes a record and points its piece at it: its content ID.
async fn put<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    record: &Element,
) -> Result<Id, StoreError> {
    let hash = write_record(txn, project, record).await?;
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: record.id.clone(),
    };
    txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    Ok(hash)
}

/// The refusal when the builders would refuse `record`, read the way they read it: its recipe
/// filled as regeneration fills it and read by the builder of the document's edition, or for a
/// room its loop found. A grid or plane is read by [`grids::check`], and an opening or a shaft by
/// its host's fit, so neither is asked here.
async fn buildable<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    edition: &construct_model::geometry::Edition,
    record: &Element,
) -> Result<Result<(), String>, StoreError> {
    if construct_grids::is_datum_kind(&record.kind) || construct_openings::is_void(&record.recipe) {
        return Ok(Ok(()));
    }
    if construct_rooms::is_room(record) {
        let found =
            construct_regen::rooms::found_at(txn, project, document, record, At::Now).await?;
        return Ok(found.map(|_| ()).map_err(|failed| failed.message));
    }
    let filled =
        construct_regen::levels::recipe_at(txn, project, document, record, At::Now).await?;
    Ok(filled
        .and_then(|filled| construct_regen::face_names(edition, &filled).map(|_| ()))
        .map_err(|failed| format!("{}: {}", record.name, failed.message)))
}

/// Asks whether everything hosted on `host` still fits it, by the kernel builder's rules, and
/// seats each of them that this command `rewrote` where it fits: the content ID of each record
/// whose placement that changed, rewritten. The inner error is the refusal, in the builder's
/// words.
async fn seat<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    host: &Id,
    rewrote: &BTreeSet<Id>,
) -> Result<Result<Vec<(Id, Id)>, String>, StoreError> {
    let all = records::read_document(txn, project, document).await?;
    let on: Vec<(Id, Element)> = all
        .into_iter()
        .filter(|(_, (_, record))| seated::host_of(record).as_ref() == Some(host))
        .map(|(id, (_, record))| (id, record))
        .collect();
    // The host is named by the face the first element hosted by offsets sits on; one hosted by
    // its position is still asked with the rest, by its position.
    let mut asked = None;
    for (id, record) in &on {
        match seated::read(record) {
            Ok(Some(Hosting::OnFace { face, .. })) => {
                asked = Some((id.clone(), Some(face)));
                break;
            }
            Ok(Some(Hosting::InPlan { .. })) => {
                asked = Some((id.clone(), None));
                break;
            }
            Ok(_) => {}
            Err(refusal) => return Ok(Err(refusal)),
        }
    }
    let Some((whole, face)) = asked else {
        return Ok(Ok(Vec::new()));
    };
    let found = match &face {
        Some(face) => {
            hosting::host_for(txn, project, document, host, Which::Name(face), "a door").await?
        }
        None => hosting::floor_for(txn, project, document, host).await?,
    };
    let found = match found {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut placed = Vec::new();
    for (id, record) in &on {
        // One asks the whole host; each this command wrote is seated where it fits.
        if *id != whole && !rewrote.contains(id) {
            continue;
        }
        let at = match hosting::fit(txn, project, document, &found, record).await? {
            Ok(at) => at,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if rewrote.contains(id) && at != record.placement {
            let mut seated = record.clone();
            seated.placement = at;
            placed.push((id.clone(), put(txn, project, document, &seated).await?));
        }
    }
    Ok(Ok(placed))
}
