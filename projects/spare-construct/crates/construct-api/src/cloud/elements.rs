//! The commands that change elements. They write an element's input record and the pointer to it,
//! and nothing else: no shape is built or read here, so an edit's cost doesn't depend on how hard
//! the shape is, and nothing here knows what a shape is. `construct_regen` builds them afterwards.
//!
//! The log's `changes` hold each element's record before and after, by content ID. Those records
//! hold the recipe, so the log is enough to rebuild every shape.
//!
//! A new element is tied to the level its `level` argument names ([`levels::tie_of_new`]); its
//! integer `level` is written 0, on no level by number.
//!
//! Every record written here carries its category under `@category`, from its kind
//! (construct-categories): a new kind with no category is refused. A new element may name its
//! type (`type`); a parameter's value and the reserved `@` keys are never set through `props`
//! here, only by the parameter commands ([`crate::cloud::params`]), which check them. Types and
//! the parameter table are records with no shape and their own commands, so these refuse them.

use crate::cloud::grids;
use crate::cloud::levels;
use crate::cloud::tool::Tool;
use construct_model::element::{Element, Placement, Reference};
use construct_model::geometry::Recipe;
use construct_model::rows::Checkpoint;
use construct_regen::parameters::{self as records, CATEGORY_KEY, TYPE_KEY};
use construct_types::TypeError;
use kit_core::Id;
use kit_ops::{Change, Claim, Edit, sha256_hex};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context, Outcome, claims};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 6] = [
    "element.create",
    "element.move",
    "element.set",
    "element.delete",
    "element.claim",
    "element.release",
];
/// The commands here that are steps a person can undo: a claim and its release are not.
pub const STEPS: [&str; 4] = [
    "element.create",
    "element.move",
    "element.set",
    "element.delete",
];

/// The element tool ([`Tool`]): the generic commands, which reach any tool's records, so the tools
/// whose records they could reach guard them.
pub struct ElementTool;

impl Tool for ElementTool {
    const NAME: &'static str = "element";
    const COMMANDS: &'static [&'static str] = &COMMANDS;
    const STEPS: &'static [&'static str] = &STEPS;

    async fn apply<T: Txn>(
        txn: &mut T,
        project: &Id,
        context: &kit_sync::Context<'_>,
        edit: &Edit,
        claim_ms: u64,
    ) -> Result<Outcome, StoreError> {
        apply(txn, project, context, edit, claim_ms).await
    }
}

/// The arguments of every command; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    name: Option<String>,
    /// The level a new element is tied to: its ID, or an old page's integer ([`levels::tie_of_new`]).
    level: Option<serde_json::Value>,
    placement: Option<Placement>,
    recipe: Option<Recipe>,
    props: Option<BTreeMap<String, String>>,
    references: Option<Vec<Reference>>,
    /// The type a new element is of.
    #[serde(default, rename = "type", with = "kit_ops::id::option")]
    type_id: Option<Id>,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

fn parse_hash(hash: &str) -> Result<Id, StoreError> {
    Id::parse(hash).map_err(|e| StoreError::Damaged(e.to_string()))
}

pub(crate) async fn write_record<T: Txn>(
    txn: &mut T,
    project: &Id,
    element: &Element,
) -> Result<Id, StoreError> {
    let bytes = to_row(element)?;
    let hash = parse_hash(&sha256_hex(&bytes))?;
    let place = Place::Record {
        project: project.clone(),
        hash: hash.clone(),
    };
    txn.put(place.key(), bytes).await?;
    Ok(hash)
}

async fn create<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
    args: Args,
) -> Result<Outcome, StoreError> {
    let (Some(id), Some(kind), Some(placement), Some(recipe)) =
        (args.element, args.kind, args.placement, args.recipe)
    else {
        return Ok(Err(
            "A new element needs an id, a kind, a placement and a recipe.".to_owned(),
        ));
    };
    let pointer = Place::Piece {
        project: project.clone(),
        document: edit.target.clone(),
        piece: id.clone(),
    };
    if txn.get(&pointer.key()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    if kind == records::TYPE_KIND
        || kind == records::TABLE_KIND
        || kind == construct_regen::families::RECORD_KIND
        || id.as_str() == records::TABLE_ID
    {
        return Ok(Err(NOT_AN_ELEMENT.to_owned()));
    }
    if grids::is_datum(&kind, &recipe) {
        return Ok(Err(grids::BY_COMMAND.to_owned()));
    }
    let category = match records::category_of(&recipe) {
        Ok(category) => category,
        Err(e) => return Ok(Err(e.to_string())),
    };
    if kind == construct_levels::LEVEL_KIND || category == construct_categories::LEVELS {
        return Ok(Err(levels::BY_LEVEL_COMMANDS.to_owned()));
    }
    let tie = match levels::tie_of_new(txn, project, &edit.target, args.level.as_ref()).await? {
        Ok(tie) => tie,
        Err(why) => return Ok(Err(why)),
    };
    let mut props = args.props.unwrap_or_default();
    if let Err(why) = props_allowed(txn, project, &edit.target, &props).await? {
        return Ok(Err(why));
    }
    if let Some(type_id) = &args.type_id {
        let context = match records::read_context(txn, project, &edit.target).await? {
            Ok(context) => context,
            Err(why) => return Ok(Err(why)),
        };
        let ty = match records::read_type(txn, project, &edit.target, type_id, &context).await? {
            Ok(ty) => ty,
            Err(why) => return Ok(Err(why)),
        };
        if ty.category() != category {
            let refusal = TypeError::OtherCategory {
                from: category,
                to: ty.category(),
            };
            return Ok(Err(refusal.to_string()));
        }
        props.insert(TYPE_KEY.to_owned(), type_id.to_string());
    }
    props.insert(CATEGORY_KEY.to_owned(), records::write_category(category));
    construct_levels::write_base(&mut props, tie.as_ref());
    let element = Element {
        id: id.clone(),
        document: edit.target.clone(),
        kind,
        name: args.name.unwrap_or_else(|| id.to_string()),
        // On no level by number: the tie in `props` is its level.
        level: 0,
        placement,
        recipe,
        props,
        references: args.references.unwrap_or_default(),
    };
    let hash = write_record(txn, project, &element).await?;
    txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    let change = Change {
        piece: id,
        before: None,
        after: Some(hash),
    };
    Ok(Ok(Applied {
        changes: vec![change],
        ..Applied::default()
    }))
}

/// Why a command refuses a record of no shape.
const NOT_AN_ELEMENT: &str =
    "Types, levels and the parameter table are changed by their own commands, not as elements.";

/// Refuses `props` that set a reserved `@` key or a defined parameter's value: those are set by
/// the parameter commands, which check them. Any other key is plain text, kept as it is.
async fn props_allowed<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    props: &BTreeMap<String, String>,
) -> Result<Result<(), String>, StoreError> {
    if props.keys().any(|key| key.starts_with('@')) {
        return Ok(Err(
            "A key starting with @ is kept by the server, not set by hand.".to_owned(),
        ));
    }
    if props.is_empty() {
        return Ok(Ok(()));
    }
    let context = match records::read_context(txn, project, document).await? {
        Ok(context) => context,
        Err(why) => return Ok(Err(why)),
    };
    let defined = props
        .keys()
        .find(|key| Id::parse(key).is_ok_and(|id| context.params.definition(&id).is_some()));
    Ok(match defined {
        Some(key) => Err(format!(
            "{key} is a parameter: set it with element.param, which checks its value."
        )),
        None => Ok(()),
    })
}

/// Why an `element.*` command on an existing record refuses it before anything else is read: a
/// record of no shape (a type, a level, the parameter table) is changed by its own commands, and
/// `element.delete` does not delete a grid or a reference plane, which their own commands delete
/// ([`grids::delete_refusal`]). A grid's move or change is refused in [`edited`].
fn refusal(before: &Element, cmd: &str) -> Option<String> {
    // Letting go of a claim is allowed on any piece, a type or the parameter table too: the
    // commands that make those take a claim on them, and nothing else gives it back early.
    if cmd != "element.release" && !records::has_shape(before) {
        return Some(NOT_AN_ELEMENT.to_owned());
    }
    if cmd == "element.delete" {
        return grids::delete_refusal(before);
    }
    None
}

/// The element after an `element.move` or `element.set`, or why the edit cannot be made.
fn edited(mut after: Element, cmd: &str, args: Args) -> Result<Element, String> {
    // A grid or a reference plane, or a recipe that makes one, is the grid commands' to change.
    if grids::is_datum(&after.kind, &after.recipe)
        || args
            .recipe
            .as_ref()
            .is_some_and(|recipe| grids::is_datum("", recipe))
    {
        return Err(grids::BY_COMMAND.to_owned());
    }
    let mut changed_recipe = false;
    match cmd {
        "element.move" => {
            let Some(placement) = args.placement else {
                return Err("A move needs a placement.".to_owned());
            };
            after.placement = placement;
        }
        "element.set" => {
            changed_recipe = args.recipe.is_some();
            if let Some(name) = args.name {
                after.name = name;
            }
            if let Some(recipe) = args.recipe {
                after.recipe = recipe;
            }
            if let Some(references) = args.references {
                after.references = references;
            }
            after.props.extend(args.props.unwrap_or_default());
        }
        other => return Err(format!("Unknown command {other}.")),
    }
    // Its category follows its kind. A recipe changed to a kind with no category, or to one of
    // another category than its type's, is refused; an older record of a kind no category
    // holds, edited otherwise, is kept as it was.
    match records::category_of(&after.recipe) {
        Ok(category) => {
            if let Some(stored) = records::stored_category(&after)
                && after.props.contains_key(TYPE_KEY)
                && stored != category
            {
                let refusal = TypeError::OtherCategory {
                    from: stored,
                    to: category,
                };
                return Err(refusal.to_string());
            }
            after
                .props
                .insert(CATEGORY_KEY.to_owned(), records::write_category(category));
        }
        Err(e) if changed_recipe => return Err(e.to_string()),
        Err(_) => {}
    }
    Ok(after)
}

/// Runs an `element.*` command.
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
) -> Result<Outcome, StoreError> {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    if edit.cmd == "element.create" {
        return create(txn, project, edit, args).await;
    }

    let Some(id) = args.element.clone() else {
        return Ok(Err("Which element?".to_owned()));
    };
    let document = &edit.target;
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: id.clone(),
    };
    let hash: Option<String> = read_row(txn, &pointer.key()).await?;
    let Some(hash) = hash else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    let hash = parse_hash(&hash)?;
    let place = Place::Record {
        project: project.clone(),
        hash: hash.clone(),
    };
    let before: Element = read_row(txn, &place.key())
        .await?
        .ok_or_else(|| StoreError::Damaged("record missing".into()))?;
    if let Some(why) = refusal(&before, &edit.cmd) {
        return Ok(Err(why));
    }
    if let Some(props) = &args.props
        && let Err(why) = props_allowed(txn, project, document, props).await?
    {
        return Ok(Err(why));
    }
    let holder =
        claims::held_by_another(txn, project, document, &id, &edit.person, context.now_ms).await?;
    if let Some(claim) = holder {
        return Ok(Err(format!("{} is working on this.", claim.name)));
    }
    let person: Option<Named> = read_row(txn, &Place::Person(edit.person.clone()).key()).await?;
    let name = person.map_or_else(|| edit.person.to_string(), |person| person.name);
    let mine = Claim {
        person: edit.person.clone(),
        name,
        // milliseconds since 1970 plus a claim's length: far below u64::MAX
        until_ms: context.now_ms.saturating_add(claim_ms),
    };

    match edit.cmd.as_str() {
        "element.claim" => {
            let claim = claims::take(txn, project, document, &id, mine).await?;
            return Ok(Ok(Applied {
                claims: vec![claim],
                ..Applied::default()
            }));
        }
        "element.release" => {
            let claim = claims::release(txn, project, document, &id).await?;
            return Ok(Ok(Applied {
                claims: vec![claim],
                ..Applied::default()
            }));
        }
        "element.delete" => {
            txn.delete(pointer.key()).await?;
            let claim = claims::release(txn, project, document, &id).await?;
            let change = Change {
                piece: id,
                before: Some(hash),
                after: None,
            };
            return Ok(Ok(Applied {
                changes: vec![change],
                claims: vec![claim],
                ..Applied::default()
            }));
        }
        _ => {}
    }

    let after = match edited(before, edit.cmd.as_str(), args) {
        Ok(after) => after,
        Err(why) => return Ok(Err(why)),
    };

    let new_hash = write_record(txn, project, &after).await?;
    txn.put(pointer.key(), to_row(&new_hash.as_str())?).await?;
    let claim = claims::take(txn, project, document, &id, mine).await?;
    let change = Change {
        piece: id,
        before: Some(hash),
        after: Some(new_hash),
    };
    Ok(Ok(Applied {
        changes: vec![change],
        claims: vec![claim],
        ..Applied::default()
    }))
}

/// Records what regeneration built. It changes nothing: it is in the log so that every browser
/// watching the document, on any server, gets it in order with the edits it covers.
///
/// # Errors
///
/// The edit's arguments do not read as a checkpoint.
pub fn checkpoint(edit: &Edit) -> Outcome {
    match serde_json::from_value::<Checkpoint>(edit.args.clone()) {
        Ok(_) => Ok(Applied::default()),
        Err(e) => Err(format!("That edit isn't valid: {e}")),
    }
}
