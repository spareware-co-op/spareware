//! The families a document holds (job 7.6, CON-A7-F2-P3, piece A73): a family recipe loaded into
//! the document from the library or as recipe text, reloaded at a newer version with every placed
//! copy kept, placed copies swapped to another type or family, door and window types made by the
//! generator, and a family taken out of the document. Each goes through the same edit log as
//! `family.*`, with the same access check (`model.edit`), and each is one entry, so one undo takes
//! back all it wrote ([`crate::cloud::undo`]). The family's record is construct-regen's
//! [`families::record`], holding construct-families' [`StoredFamily`]; a placed copy names it by
//! the family's permanent id ([`families::FAMILY_ID_ARG`]) and is built from it, so the document
//! builds the same on a server without the library.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `family.load` | `family` (a library family's permanent id) or `recipe` (recipe text) | The family's record |
//! | `family.reload` | `family` (its id in the document), and `recipe` (recipe text; default the library's version of it) | The family's record, and every placed copy of it at the new version, each keeping its place, host, type and values |
//! | `family.swap` | `element`, or `family` and `type` (every copy of that type of it); `to_family` (in the document, or a library family's id, loaded with it), `to_type` | Each copy: of the other family and type, at the same place on the same host |
//! | `family.generate` | `family` (the id), `name` (for a new family), `spec` (below) | A new generated family's record, or the family's record with the new type and every placed copy at the new version |
//! | `family.remove` | `family` | Its record goes |
//!
//! A `spec` is one door or window asked of the generator (construct-doorgen's `Spec`):
//! `template` (`single-swing-door`, `fixed-window` or `sliding-window`), `mark`, `width` and
//! `height` (typed lengths; the outside of the frame, or the clear opening with `clear: true`),
//! and, where its template has them, `frame`, `frame_depth`, `leaf`, `sash`, `sash_depth`,
//! `track_gap`, `glass`, `sill_height` and `wall` (typed lengths), `hand` (`left` or `right`),
//! `frame_material` and `leaf_material` (material ids). The generated recipe is checked by
//! construct-doorgen's `verify` before anything is stored.
//!
//! Every recipe a document takes passes the same checks ([`takes`]): no more than
//! construct-families' `MOST_RECIPE_BYTES` of text, the reader, the checker against the project's
//! categories and materials, every type it names evaluated within the evaluator's default limits,
//! and a wall-hosted family's opening cut as a builder cuts it. Each command is refused, as its
//! result and in words, for an unknown family or type (saying what there is), a recipe that fails
//! those checks (naming the type), a family already in the document (load) or not in it
//! (reload, remove), a reload whose recipe is another family, the same version or another kind,
//! or one that would break a placed copy (a type the new version no longer has, a value it no
//! longer takes, or a copy that would no longer cut its wall cleanly, each naming the copy), a
//! swap to a family of another kind, a generated type whose mark the family already has, and a
//! remove of a family still placed (naming its copies). Nothing is written by a refused command.
//! Another person working on a copy it would write turns it back in their words.
//!
//! The generic commands are not a back door ([`generic_rules`]): a family's record is not made,
//! changed, moved nor deleted by `element.*`. An undo that would leave a placed copy without the
//! version of its family it names is refused ([`undo_rules`]).
//!
//! For the page, without the log: [`read_families`], the document's families and then the
//! library's that it does not hold, as the server's families route lists them. Nothing here reads
//! a clock or a random number.

use crate::cloud::elements::write_record;
use crate::cloud::families::{FamilyView, category_of, kind_of, names_of, type_in, view_of};
use crate::cloud::hosting::{self, Which, fit, length};
use construct_categories::CategoryTable;
use construct_doorgen::{Family as Generator, Hand, Size, Spec, Template, generate, verify};
use construct_families::{
    Hosting as FamilyHosting, Key, LoadRefusal, Recipe as FamilyRecipe, Source, StoredFamily,
    loadable, read_text,
};
use construct_materials::Materials;
use construct_model::element::Element;
use construct_regen::families::{self, Instance, Library};
use construct_regen::hosting::{self as placing, Hosting};
use construct_regen::parameters::{self as records, At, CATEGORY_KEY};
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::{Place, Reader, StoreError, Txn, to_row};
use kit_sync::{Applied, Context, Outcome, claims};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Every command here.
pub const COMMANDS: [&str; 5] = [LOAD, RELOAD, SWAP, GENERATE, REMOVE];
/// Loads a family into the document.
pub const LOAD: &str = "family.load";
/// Replaces a family of the document with a newer version, keeping its placed copies.
pub const RELOAD: &str = "family.reload";
/// Changes placed copies to another type or family of the same kind.
pub const SWAP: &str = "family.swap";
/// Makes a door or window type with the generator, into a new family or one it made before.
pub const GENERATE: &str = "family.generate";
/// Takes a family that is no longer placed out of the document.
pub const REMOVE: &str = "family.remove";

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    family: Option<String>,
    recipe: Option<String>,
    name: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    #[serde(rename = "type")]
    ty: Option<String>,
    to_family: Option<String>,
    to_type: Option<String>,
    spec: Option<SpecArgs>,
}

/// A generator spec as a command sends it: lengths as typed text.
#[derive(Deserialize)]
struct SpecArgs {
    template: String,
    mark: String,
    width: String,
    height: String,
    #[serde(default)]
    clear: bool,
    frame: Option<String>,
    frame_depth: Option<String>,
    leaf: Option<String>,
    sash: Option<String>,
    sash_depth: Option<String>,
    track_gap: Option<String>,
    glass: Option<String>,
    hand: Option<String>,
    sill_height: Option<String>,
    frame_material: Option<String>,
    leaf_material: Option<String>,
    wall: Option<String>,
}

/// What a door or a window is hosted on, for a refusal.
const WHAT: &str = "a door or a window";

/// Whether a document takes `recipe` ([`loadable`] against the project's categories and
/// materials, within the evaluator's default limits), and for a family hosted in a wall, whether
/// each type's opening is one a builder cuts. The error is the refusal, naming the family.
pub(crate) fn takes(recipe: &FamilyRecipe) -> Result<(), String> {
    let categories = CategoryTable::new();
    let materials = Materials::default_metric();
    let library = construct_families::Library {
        categories: &categories,
        materials: &materials,
    };
    let refused = |e: &dyn std::fmt::Display| format!("{} ({}): {e}.", recipe.name, recipe.id);
    loadable(recipe, &library, construct_families::Limits::DEFAULT)
        .map_err(|e: LoadRefusal| refused(&e))?;
    if matches!(recipe.hosting, FamilyHosting::Wall { .. }) {
        let stored = StoredFamily::new(recipe.clone(), Source::Text);
        for key in stored.type_keys() {
            let placed = families::of_family(&families::recipe(&stored.content, &key), &stored);
            let filled = families::filled(&placed, &stored).map_err(|e| refused(&e.message))?;
            families::opening_of(&Library::default(), &filled).map_err(|e| {
                refused(&format!(
                    "its type {key} does not cut its wall: {}",
                    e.message
                ))
            })?;
        }
    }
    Ok(())
}

/// The family `id` of the document as it is now, if it holds it. The inner error is a record
/// that does not read.
pub(crate) async fn in_document<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    id: &Id,
) -> Result<Result<Option<(Id, StoredFamily)>, String>, StoreError> {
    let Some((hash, record)) = records::read_element(reader, project, document, id).await? else {
        return Ok(Ok(None));
    };
    if !families::is_record(&record) {
        return Ok(Ok(None));
    }
    Ok(families::read_record(&record).map(|family| Some((hash, family))))
}

/// Every family record of the document, by id, read; the inner error is one that does not read.
async fn families_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<BTreeMap<Id, StoredFamily>, String>, StoreError> {
    let mut found = BTreeMap::new();
    for (id, (_, record)) in records::read_document(reader, project, document).await? {
        if families::is_record(&record) {
            match families::read_record(&record) {
                Ok(family) => {
                    found.insert(id, family);
                }
                Err(why) => return Ok(Err(why)),
            }
        }
    }
    Ok(Ok(found))
}

/// The family a placed copy is of, by permanent id: the one in the document it names, or for one
/// written before families were held in documents, the library's family of its version.
fn family_id_of(instance: &Instance) -> Option<Id> {
    instance.in_document.clone().or_else(|| {
        use construct_regen::families::FamilySource;
        Library::baked()
            .recipe(&instance.family)
            .map(|recipe| recipe.id.clone())
    })
}

/// Every placed copy in the document, with its hash and what its recipe says.
async fn copies<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Vec<(Id, Element, Instance)>, StoreError> {
    let mut found = Vec::new();
    for (_, (hash, record)) in records::read_document(reader, project, document).await? {
        if let Ok(instance) = Instance::of(&record.recipe) {
            found.push((hash, record, instance));
        }
    }
    Ok(found)
}

/// The document's families by name, for a refusal that says what there is.
fn names_in(families: &BTreeMap<Id, StoredFamily>) -> String {
    if families.is_empty() {
        return "none".to_owned();
    }
    let names: Vec<String> = families
        .values()
        .map(|f| format!("{} ({})", f.recipe.name, f.recipe.id))
        .collect();
    names.join(", ")
}

/// Writes `record` at its piece, after `before`: the change.
async fn put<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    record: &Element,
    before: Option<Id>,
) -> Result<Change, StoreError> {
    let hash = write_record(txn, project, record).await?;
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: record.id.clone(),
    };
    txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    Ok(Change {
        piece: record.id.clone(),
        before,
        after: Some(hash),
    })
}

/// The family of the library `id` names, as a document would hold it.
fn from_library(id: &str) -> Result<StoredFamily, String> {
    let library = Library::baked();
    let Some((_, recipe)) = library.family(id) else {
        return Err(format!(
            "There is no family {id} in the library; it has {}.",
            names_of(&library)
        ));
    };
    let source = Source::Library(recipe.id.clone());
    Ok(StoredFamily::new((*recipe).clone(), source))
}

/// The family a `family.load`'s or `family.reload`'s arguments give: recipe text, or a family of
/// the library by its id (`default` when neither is sent). Checked ([`takes`]).
fn given(args: &Args, default: Option<&str>) -> Result<StoredFamily, String> {
    let family = match (&args.recipe, args.family.as_deref().or(default)) {
        (Some(text), _) => {
            let recipe =
                read_text(text).map_err(|e| format!("That recipe can't be loaded: {e}."))?;
            StoredFamily::new(recipe, Source::Text)
        }
        (None, Some(id)) => from_library(id)?,
        (None, None) => {
            return Err(
                "Which family? Name one of the library by its id, or send its recipe.".to_owned(),
            );
        }
    };
    takes(&family.recipe)?;
    Ok(family)
}

/// The family `typed` names in the document, or why not.
fn named<'a>(
    families: &'a BTreeMap<Id, StoredFamily>,
    typed: Option<&str>,
) -> Result<(&'a Id, &'a StoredFamily), String> {
    let Some(typed) = typed else {
        return Err("Which family? Name it by its id.".to_owned());
    };
    Id::parse(typed)
        .ok()
        .and_then(|id| families.get_key_value(&id))
        .ok_or_else(|| {
            format!(
                "There is no family {typed} in this document; it has {}.",
                names_in(families)
            )
        })
}

/// Runs a command here.
///
/// # Errors
///
/// A store error; a command that cannot be made is the outcome's error instead.
pub async fn apply<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
) -> Answer {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    let document = &edit.target;
    let families = match families_of(txn, project, document).await? {
        Ok(families) => families,
        Err(why) => return Ok(Err(why)),
    };
    match edit.cmd.as_str() {
        LOAD => {
            let family = match given(&args, None) {
                Ok(family) => family,
                Err(why) => return Ok(Err(why)),
            };
            load(txn, project, document, &families, &family).await
        }
        RELOAD => {
            let (id, old) = match named(&families, args.family.as_deref()) {
                Ok(found) => found,
                Err(why) => return Ok(Err(why)),
            };
            let library_id = match &old.source {
                Source::Library(id) => Some(id.to_string()),
                _ => None,
            };
            if args.recipe.is_none() && library_id.is_none() {
                return Ok(Err(format!(
                    "{} did not come from the library: send its new recipe.",
                    old.recipe.name
                )));
            }
            let lookup = Args {
                recipe: args.recipe.clone(),
                family: library_id.clone(),
                ..Args::default()
            };
            let new = match given(&lookup, library_id.as_deref()) {
                Ok(new) => new,
                Err(why) => return Ok(Err(why)),
            };
            let (id, old) = (id.clone(), old.clone());
            reload(txn, project, context, edit, &id, &old, new).await
        }
        SWAP => swap(txn, project, context, edit, &families, &args).await,
        GENERATE => generated(txn, project, context, edit, &families, &args).await,
        REMOVE => remove(txn, project, document, &families, &args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// `family.load`: `family` into the document, unless it holds that family already.
async fn load<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    families: &BTreeMap<Id, StoredFamily>,
    family: &StoredFamily,
) -> Answer {
    let id = &family.recipe.id;
    if let Some(held) = families.get(id) {
        return Ok(Err(format!(
            "{} ({id}) is already in this document: bring in another version with family.reload.",
            held.recipe.name
        )));
    }
    if let Some((_, other)) = records::read_element(txn, project, document, id).await? {
        return Ok(Err(format!(
            "{id} is already the id of {} in this document.",
            other.name
        )));
    }
    let change = put(
        txn,
        project,
        document,
        &families::record(document, family),
        None,
    )
    .await?;
    Ok(Ok(Applied {
        changes: vec![change],
        ..Applied::default()
    }))
}

/// The family `typed` names, for a copy to be made of: the document's, or a family of the
/// library loaded with the command (its record is then the first change).
pub(crate) async fn family_for<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    typed: &str,
) -> Result<Result<(StoredFamily, Option<Change>), String>, StoreError> {
    if let Ok(id) = Id::parse(typed) {
        match in_document(txn, project, document, &id).await? {
            Ok(Some((_, family))) => return Ok(Ok((family, None))),
            Ok(None) => {}
            Err(why) => return Ok(Err(why)),
        }
    }
    let family = match from_library(typed).and_then(|family| takes(&family.recipe).map(|()| family))
    {
        Ok(family) => family,
        Err(why) => return Ok(Err(why)),
    };
    if let Some((_, other)) =
        records::read_element(txn, project, document, &family.recipe.id).await?
    {
        return Ok(Err(format!(
            "{} is already the id of {} in this document.",
            family.recipe.id, other.name
        )));
    }
    let change = put(
        txn,
        project,
        document,
        &families::record(document, &family),
        None,
    )
    .await?;
    Ok(Ok((family, Some(change))))
}

/// Another person's live claim on `piece`, in their words.
async fn held<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
    piece: &Id,
) -> Result<Option<String>, StoreError> {
    let holder = claims::held_by_another(
        txn,
        project,
        &edit.target,
        piece,
        &edit.person,
        context.now_ms,
    )
    .await?;
    Ok(holder.map(|claim| format!("{} is working on this.", claim.name)))
}

/// A placed copy's host, the side it is on, and its offsets.
fn hosted_on(element: &Element) -> Result<(Id, String), String> {
    match placing::read(element)? {
        Some(Hosting::OnFace { host, face, .. }) => Ok((host, face)),
        _ => Err(format!(
            "{} is not hosted on a face of a wall.",
            element.name
        )),
    }
}

/// `after`, already written, seated again on its wall with everything else now there: its record
/// as it is to stay (its placement worked out again), or the refusal.
async fn refit<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    mut after: Element,
) -> Result<Result<Element, String>, StoreError> {
    let (host_id, face) = match hosted_on(&after) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let host = match hosting::host_for(txn, project, document, &host_id, Which::Name(&face), WHAT)
        .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    Ok(match fit(txn, project, document, &host, &after).await? {
        Ok(placement) => {
            after.placement = placement;
            Ok(after)
        }
        Err(why) => Err(why),
    })
}

/// Writes each copy in `rewritten` (its hash before, and its record after), then seats each again
/// on its wall with all of them written, so each is judged with the others as they will be. The
/// changes, or the refusal naming the copy (`because` says what was asked).
async fn write_copies<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    rewritten: Vec<(Id, Element)>,
    because: &str,
) -> Result<Result<Vec<Change>, String>, StoreError> {
    for (_, after) in &rewritten {
        put(txn, project, document, after, None).await?;
    }
    let mut changes = Vec::with_capacity(rewritten.len());
    for (before, after) in rewritten {
        let name = after.name.clone();
        let seated = match refit(txn, project, document, after).await? {
            Ok(seated) => seated,
            Err(why) => {
                return Ok(Err(format!(
                    "{name} would no longer cut its wall cleanly {because}: {why}"
                )));
            }
        };
        changes.push(put(txn, project, document, &seated, Some(before)).await?);
    }
    Ok(Ok(changes))
}

/// `family.reload` (and a generated type added): `new` in place of `old`, the family `id` of the
/// document, and every placed copy of it at the new version, keeping its place, host, type and
/// values; refused, naming the copy, when the new version would break one.
async fn reload<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
    id: &Id,
    old: &StoredFamily,
    new: StoredFamily,
) -> Answer {
    let document = &edit.target;
    if new.recipe.id != *id {
        return Ok(Err(format!(
            "That recipe is family {}, not {id}.",
            new.recipe.id
        )));
    }
    if new.content == old.content {
        return Ok(Err(format!(
            "{} is already this version in this document.",
            old.recipe.name
        )));
    }
    let (was, is) = (
        kind_of(category_of(&old.recipe)),
        kind_of(category_of(&new.recipe)),
    );
    if was != is {
        return Ok(Err(format!(
            "{} is a {was} family and that recipe is a {is} one: a reload keeps the family's kind.",
            old.recipe.name
        )));
    }
    let Some((before, _)) = records::read_element(txn, project, document, id).await? else {
        return Ok(Err(format!("There is no family {id} in this document.")));
    };
    let mut changes = vec![
        put(
            txn,
            project,
            document,
            &families::record(document, &new),
            Some(before),
        )
        .await?,
    ];
    let mut rewritten = Vec::new();
    for (hash, record, instance) in copies(txn, project, document).await? {
        if family_id_of(&instance).as_ref() != Some(id) {
            continue;
        }
        if let Some(why) = held(txn, project, context, edit, &record.id).await? {
            return Ok(Err(why));
        }
        if new.recipe.named_type(&instance.ty).is_none() {
            return Ok(Err(format!(
                "{} no longer has type {}, which {} is: keep that type in the new version, or \
                 change {} to another type first.",
                new.recipe.name, instance.ty, record.name, record.name
            )));
        }
        for key in instance.values.keys() {
            if let Err(why) = new.recipe.check_instance_key(key) {
                return Ok(Err(format!("{}: {why}", record.name)));
            }
        }
        let mut after = record.clone();
        after.recipe = families::of_family(&record.recipe, &new);
        after.props.insert(
            CATEGORY_KEY.to_owned(),
            records::write_category(category_of(&new.recipe)),
        );
        rewritten.push((hash, after));
    }
    match write_copies(txn, project, document, rewritten, "at the new version").await? {
        Ok(more) => changes.extend(more),
        Err(why) => return Ok(Err(why)),
    }
    Ok(Ok(Applied {
        changes,
        ..Applied::default()
    }))
}

/// `family.swap`: the copies asked for, of another type or family of the same kind.
#[allow(
    clippy::too_many_lines,
    reason = "each thing a swap is checked against, in the order its arguments are read"
)]
async fn swap<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
    families: &BTreeMap<Id, StoredFamily>,
    args: &Args,
) -> Answer {
    let document = &edit.target;
    let (Some(to_family), Some(to_type)) = (&args.to_family, &args.to_type) else {
        return Ok(Err(
            "A swap needs the family and the type to change to (to_family, to_type).".to_owned(),
        ));
    };
    // The copies: one element, or every copy of one type of one family.
    let all = copies(txn, project, document).await?;
    let chosen: Vec<(Id, Element, Instance)> = match (&args.element, &args.family, &args.ty) {
        (Some(element), None, None) => {
            let Some(found) = all.into_iter().find(|(_, record, _)| record.id == *element) else {
                return Ok(Err(
                    match records::read_element(txn, project, document, element).await? {
                        Some((_, record)) => format!("{} is not a placed family.", record.name),
                        None => "That element no longer exists.".to_owned(),
                    },
                ));
            };
            vec![found]
        }
        (None, Some(family), Some(ty)) => {
            let Ok(id) = Id::parse(family) else {
                return Ok(Err(format!("{family} is not a family's id.")));
            };
            if !families.contains_key(&id)
                && !all
                    .iter()
                    .any(|(_, _, i)| family_id_of(i) == Some(id.clone()))
            {
                return Ok(Err(format!(
                    "There is no family {family} in this document; it has {}.",
                    names_in(families)
                )));
            }
            let chosen: Vec<_> = all
                .into_iter()
                .filter(|(_, _, i)| family_id_of(i).as_ref() == Some(&id) && i.ty.as_str() == ty)
                .collect();
            if chosen.is_empty() {
                return Ok(Err(format!("No copy of {family} is of type {ty}.")));
            }
            chosen
        }
        _ => {
            return Ok(Err(
                "Which copies? Name one element, or a family and one of its types.".to_owned(),
            ));
        }
    };
    let (target, loaded) = match family_for(txn, project, document, to_family).await? {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let key = match type_in(&target.recipe, to_type) {
        Ok(key) => key,
        Err(why) => return Ok(Err(why)),
    };
    let to_kind = kind_of(category_of(&target.recipe));
    let mut rewritten = Vec::with_capacity(chosen.len());
    for (hash, record, instance) in chosen {
        if let Some(why) = held(txn, project, context, edit, &record.id).await? {
            return Ok(Err(why));
        }
        if record.kind != to_kind {
            return Ok(Err(format!(
                "{} is a {}: it is swapped only for another {} family, and {} is a {to_kind} one.",
                record.name, record.kind, record.kind, target.recipe.name
            )));
        }
        if instance.family == target.content && instance.ty == key {
            return Ok(Err(format!(
                "{} is already {} of type {key}.",
                record.name, target.recipe.name
            )));
        }
        let mut recipe = families::of_family(&families::recipe(&target.content, &key), &target);
        // The values it holds that the other family takes; any other goes.
        for (value, stored) in &instance.values {
            if target.recipe.check_instance_key(value).is_ok() {
                recipe = families::with_value(&recipe, value, stored);
            }
        }
        let mut after = record.clone();
        after.recipe = recipe;
        after.props.insert(
            CATEGORY_KEY.to_owned(),
            records::write_category(category_of(&target.recipe)),
        );
        rewritten.push((hash, after));
    }
    let mut changes: Vec<Change> = loaded.into_iter().collect();
    let because = format!("as {} {key}", target.recipe.name);
    match write_copies(txn, project, document, rewritten, &because).await? {
        Ok(more) => changes.extend(more),
        Err(why) => return Ok(Err(why)),
    }
    Ok(Ok(Applied {
        changes,
        ..Applied::default()
    }))
}

/// A typed length of a spec, if sent.
fn spec_length(
    what: &str,
    typed: Option<&String>,
) -> Result<Option<construct_params::Length>, String> {
    typed.map(|typed| length(what, typed)).transpose()
}

/// A material id of a spec, if sent.
fn spec_material(what: &str, typed: Option<&String>) -> Result<Option<Id>, String> {
    typed
        .map(|typed| Id::parse(typed).map_err(|_| format!("{what} {typed} is not a material's id")))
        .transpose()
}

/// The spec a command sends, read.
fn spec_of(sent: &SpecArgs) -> Result<Spec, String> {
    let Some(template) = Template::ALL
        .into_iter()
        .find(|t| t.stem() == sent.template)
    else {
        let stems: Vec<&str> = Template::ALL.iter().map(|t| t.stem()).collect();
        return Err(format!(
            "There is no template {}; the generator has {}.",
            sent.template,
            stems.join(", ")
        ));
    };
    let width = length("The width", &sent.width)?;
    let height = length("The height", &sent.height)?;
    let size = if sent.clear {
        Size::Clear { width, height }
    } else {
        Size::Overall { width, height }
    };
    let mut spec = Spec::new(sent.mark.clone(), template, size);
    spec.frame = spec_length("The frame", sent.frame.as_ref())?;
    spec.frame_depth = spec_length("The frame depth", sent.frame_depth.as_ref())?;
    spec.leaf = spec_length("The leaf", sent.leaf.as_ref())?;
    spec.sash = spec_length("The sash", sent.sash.as_ref())?;
    spec.sash_depth = spec_length("The sash depth", sent.sash_depth.as_ref())?;
    spec.track_gap = spec_length("The track gap", sent.track_gap.as_ref())?;
    spec.glass = spec_length("The glass", sent.glass.as_ref())?;
    spec.sill_height = spec_length("The sill height", sent.sill_height.as_ref())?;
    spec.wall = spec_length("The wall", sent.wall.as_ref())?;
    spec.frame_material = spec_material("The frame material", sent.frame_material.as_ref())?;
    spec.leaf_material = spec_material("The leaf material", sent.leaf_material.as_ref())?;
    spec.hand = match sent.hand.as_deref() {
        None => None,
        Some("left") => Some(Hand::Left),
        Some("right") => Some(Hand::Right),
        Some(other) => return Err(format!("The hand is left or right, not {other}.")),
    };
    Ok(spec)
}

/// `family.generate`: a spec made into a type by the generator, checked by its `verify`, as a new
/// family of the document or a type added to one it generated before.
async fn generated<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
    families: &BTreeMap<Id, StoredFamily>,
    args: &Args,
) -> Answer {
    let document = &edit.target;
    let (Some(family), Some(sent)) = (&args.family, &args.spec) else {
        return Ok(Err(
            "A generated type needs the family's id and a spec to make it from.".to_owned(),
        ));
    };
    let Ok(id) = Id::parse(family) else {
        return Ok(Err(format!("{family} is not a family's id.")));
    };
    let spec = match spec_of(sent) {
        Ok(spec) => spec,
        Err(why) => return Ok(Err(why)),
    };
    let template = spec.template;
    let categories = CategoryTable::new();
    let materials = Materials::default_metric();
    let library = construct_families::Library {
        categories: &categories,
        materials: &materials,
    };
    let existing = families.get(&id);
    let name = match (existing, &args.name) {
        (Some(held), _) => held.recipe.name.clone(),
        (None, Some(name)) => name.clone(),
        (None, None) => {
            return Ok(Err(format!(
                "A new generated family needs a name; {family} is not in this document."
            )));
        }
    };
    let generator = Generator {
        id: id.clone(),
        name,
        template,
    };
    let made = match generate(&generator, std::slice::from_ref(&spec), &library) {
        Ok(made) => made,
        Err(refusal) => return Ok(Err(format!("The generator refused it: {refusal}."))),
    };
    let source = Source::Generated(template.stem().to_owned());
    let recipe = match existing {
        None => made.recipe,
        Some(held) => match added(held, &made.recipe, &source) {
            Ok(recipe) => recipe,
            Err(why) => return Ok(Err(why)),
        },
    };
    if let Err(refusal) = verify(&recipe, template) {
        return Ok(Err(format!(
            "The generated family does not check: {refusal}."
        )));
    }
    if let Err(why) = takes(&recipe) {
        return Ok(Err(why));
    }
    let new = StoredFamily::new(recipe, source);
    match existing {
        None => load(txn, project, document, families, &new).await,
        Some(old) => {
            let old = old.clone();
            reload(txn, project, context, edit, &id, &old, new).await
        }
    }
}

/// `held`, a family the generator made from `source`'s template, with the type row of `made` (the
/// same family generated from one more spec) added: refused when it was not generated so, has
/// been changed since, or already has a type of that mark.
fn added(
    held: &StoredFamily,
    made: &FamilyRecipe,
    source: &Source,
) -> Result<FamilyRecipe, String> {
    let name = &held.recipe.name;
    if held.source != *source {
        return Err(format!(
            "{name} was not made by the generator from that template ({}): generate into a new \
             family.",
            held.source
        ));
    }
    let (Some(table), Some(new_table)) = (&held.recipe.type_table, &made.type_table) else {
        return Err(format!(
            "{name} has no type table to add a generated type to."
        ));
    };
    let mut base = held.recipe.clone();
    let mut other = made.clone();
    for recipe in [&mut base, &mut other] {
        if let Some(table) = &mut recipe.type_table {
            table.rows.clear();
        }
    }
    if base != other || table.columns != new_table.columns {
        return Err(format!(
            "{name} is not as the generator makes it from its template: generate into a new family."
        ));
    }
    let mut recipe = held.recipe.clone();
    let mut keys: BTreeSet<Key> = held.type_keys().into_iter().collect();
    if let Some(rows) = recipe.type_table.as_mut().map(|t| &mut t.rows) {
        for row in &new_table.rows {
            if !keys.insert(row.key.clone()) {
                return Err(format!("{name} already has a type {}.", row.key));
            }
            rows.push(row.clone());
        }
    }
    Ok(recipe)
}

/// `family.remove`: a family no copy is placed of, out of the document.
async fn remove<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    families: &BTreeMap<Id, StoredFamily>,
    args: &Args,
) -> Answer {
    let (id, family) = match named(families, args.family.as_deref()) {
        Ok(found) => found,
        Err(why) => return Ok(Err(why)),
    };
    let placed: Vec<String> = copies(txn, project, document)
        .await?
        .into_iter()
        .filter(|(_, _, instance)| instance.in_document.as_ref() == Some(id))
        .map(|(_, record, _)| record.name)
        .collect();
    if !placed.is_empty() {
        return Ok(Err(format!(
            "{} is still placed ({} copies: {}): delete them or swap them to another family first.",
            family.recipe.name,
            placed.len(),
            placed.join(", ")
        )));
    }
    let Some((hash, _)) = records::read_element(txn, project, document, id).await? else {
        return Ok(Err(format!("There is no family {id} in this document.")));
    };
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: id.clone(),
    };
    txn.delete(pointer.key()).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: id.clone(),
            before: Some(hash),
            after: None,
        }],
        ..Applied::default()
    }))
}

/// The rules of the generic commands for a family's record: it is made by `family.load`, changed
/// by `family.reload` and taken out by `family.remove`, never by `element.create`, `element.set`,
/// `element.move` nor `element.delete`. The inner error is the refusal, naming the command to use.
///
/// # Errors
///
/// A [`StoreError`] when the store fails.
pub async fn generic_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
) -> Result<Result<(), String>, StoreError> {
    #[derive(Deserialize, Default)]
    struct Generic {
        #[serde(default, with = "kit_ops::id::option")]
        element: Option<Id>,
        kind: Option<String>,
        recipe: Option<construct_model::geometry::Recipe>,
    }
    let Ok(args) = serde_json::from_value::<Generic>(edit.args.clone()) else {
        return Ok(Ok(()));
    };
    let a_record = args.kind.as_deref() == Some(families::RECORD_KIND)
        || args
            .recipe
            .as_ref()
            .is_some_and(|recipe| recipe.op == families::RECORD_KIND);
    if matches!(edit.cmd.as_str(), "element.create" | "element.set") && a_record {
        return Ok(Err(
            "A family is brought into a document with family.load, which checks its recipe."
                .to_owned(),
        ));
    }
    if !matches!(
        edit.cmd.as_str(),
        "element.set" | "element.move" | "element.delete"
    ) {
        return Ok(Ok(()));
    }
    let Some(id) = &args.element else {
        return Ok(Ok(()));
    };
    let Some((_, record)) = records::read_element(txn, project, &edit.target, id).await? else {
        return Ok(Ok(()));
    };
    if !families::is_record(&record) {
        return Ok(Ok(()));
    }
    Ok(Err(format!(
        "{} is a family of this document: bring in another version with family.reload, or take \
         it out with family.remove.",
        record.name
    )))
}

/// What an undo or a redo may not leave: a placed copy without the version of its family it
/// names. Asked when it wrote or took away a family's record or a copy of one; the inner error is
/// the refusal.
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
    let touched: BTreeSet<&Id> = written
        .iter()
        .filter(|r| families::is_record(r) || families::in_document(&r.recipe).is_some())
        .map(|r| &r.id)
        .chain(deleted)
        .collect();
    if touched.is_empty() {
        return Ok(Ok(()));
    }
    for (_, record, instance) in copies(txn, project, document).await? {
        let Some(family) = &instance.in_document else {
            continue;
        };
        if !touched.contains(family) && !touched.contains(&record.id) {
            continue;
        }
        match records::read_family_at(txn, project, document, family, At::Now).await? {
            Ok(stored) if stored.content == instance.family => {}
            Ok(stored) => {
                return Ok(Err(format!(
                    "{} names another version of {} than the document would hold.",
                    record.name, stored.recipe.name
                )));
            }
            Err(_) => {
                return Ok(Err(format!(
                    "{} would be left without its family, {family}.",
                    record.name
                )));
            }
        }
    }
    Ok(Ok(()))
}

/// The document's families, then the library's it does not hold, as the server lists them: each
/// with its types, the values each placed copy holds, where it came from and how many copies are
/// placed. The inner error is a family record that does not read.
///
/// # Errors
///
/// A [`StoreError`] when the store fails.
pub async fn read_families<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<Vec<FamilyView>, String>, StoreError> {
    let held = match families_of(reader, project, document).await? {
        Ok(held) => held,
        Err(why) => return Ok(Err(why)),
    };
    let mut placed: BTreeMap<Id, u32> = BTreeMap::new();
    for (_, _, instance) in copies(reader, project, document).await? {
        if let Some(id) = family_id_of(&instance) {
            let count = placed.entry(id).or_insert(0);
            *count = count.saturating_add(1);
        }
    }
    let count = |id: &Id| placed.get(id).copied().unwrap_or(0);
    let mut views: Vec<FamilyView> = held
        .values()
        .map(|family| {
            let mut view = view_of(&family.content, &family.recipe);
            view.source = family.source.write();
            view.in_document = true;
            view.placed = count(&family.recipe.id);
            view
        })
        .collect();
    let library = Library::baked();
    for (content, recipe) in library.recipes() {
        if !held.contains_key(&recipe.id) {
            let mut view = view_of(content, recipe);
            view.placed = count(&recipe.id);
            views.push(view);
        }
    }
    Ok(Ok(views))
}
