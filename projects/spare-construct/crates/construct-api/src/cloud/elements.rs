//! The commands that change elements. They write an element's input record and the pointer to it,
//! and nothing else: no shape is built or read here, so an edit's cost doesn't depend on how hard
//! the shape is, and nothing here knows what a shape is. `construct_regen` builds them afterwards.
//!
//! The log's `changes` hold each element's record before and after, by content ID. Those records
//! hold the recipe, so the log is enough to rebuild every shape.

use construct_model::element::{Element, Placement, Reference};
use construct_model::geometry::Recipe;
use construct_model::rows::Checkpoint;
use kit_core::Id;
use kit_ops::{Change, Claim, Edit, sha256_hex};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context, Outcome, claims};
use serde::Deserialize;
use std::collections::BTreeMap;

/// The arguments of every command; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    name: Option<String>,
    level: Option<i32>,
    placement: Option<Placement>,
    recipe: Option<Recipe>,
    props: Option<BTreeMap<String, String>>,
    references: Option<Vec<Reference>>,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

fn parse_hash(hash: &str) -> Result<Id, StoreError> {
    Id::parse(hash).map_err(|e| StoreError::Damaged(e.to_string()))
}

async fn write_record<T: Txn>(
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
    let element = Element {
        id: id.clone(),
        document: edit.target.clone(),
        kind,
        name: args.name.unwrap_or_else(|| id.to_string()),
        level: args.level.unwrap_or(0),
        placement,
        recipe,
        props: args.props.unwrap_or_default(),
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

/// Runs an `element.*` command.
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
        until_ms: context.now_ms + claim_ms,
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

    let place = Place::Record {
        project: project.clone(),
        hash: hash.clone(),
    };
    let before: Element = read_row(txn, &place.key())
        .await?
        .ok_or_else(|| StoreError::Damaged("record missing".into()))?;
    let mut after = before;
    match edit.cmd.as_str() {
        "element.move" => {
            let Some(placement) = args.placement else {
                return Ok(Err("A move needs a placement.".to_owned()));
            };
            after.placement = placement;
        }
        "element.set" => {
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
        other => return Ok(Err(format!("Unknown command {other}."))),
    }

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
pub fn checkpoint(edit: &Edit) -> Outcome {
    match serde_json::from_value::<Checkpoint>(edit.args.clone()) {
        Ok(_) => Ok(Applied::default()),
        Err(e) => Err(format!("That edit isn't valid: {e}")),
    }
}
