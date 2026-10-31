//! Walls and doors as plain boxes. This is the placeholder for Spare Construct's real elements:
//! the geometry kernel's shapes will take the place of [`Shape`], and these commands will move to
//! Spare Construct once it has a server of its own. Lengths are whole millimetres, so a hash never
//! depends on rounding.

use kit_accounts::rows::Person;
use kit_core::Id;
use kit_ops::{Change, Claim, Edit, sha256_hex};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context, Outcome, claims};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Cells are 5 m squares of a level of a model.
const CELL_MM: i64 = 5000;

/// One element's data.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Element {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    #[serde(with = "kit_ops::id")]
    pub model: Id,
    pub kind: String,
    pub name: String,
    pub level: i32,
    pub x: i64,
    pub y: i64,
    #[serde(with = "kit_ops::id")]
    pub shape: Id,
    pub props: BTreeMap<String, String>,
}

/// A box.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Shape {
    pub w: i64,
    pub d: i64,
    pub h: i64,
}

/// Every cell a box touches, as (x, y).
#[must_use]
pub fn cells(element: &Element, shape: &Shape) -> Vec<(i64, i64)> {
    cells_between(
        element.x,
        element.y,
        element.x + shape.w,
        element.y + shape.d,
    )
}

/// Every cell of a rectangle.
#[must_use]
pub fn cells_between(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<(i64, i64)> {
    let mut found = Vec::new();
    for x in x0.div_euclid(CELL_MM)..=x1.div_euclid(CELL_MM) {
        for y in y0.div_euclid(CELL_MM)..=y1.div_euclid(CELL_MM) {
            found.push((x, y));
        }
    }
    found
}

/// The arguments of every command; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    name: Option<String>,
    level: Option<i32>,
    x: Option<i64>,
    y: Option<i64>,
    w: Option<i64>,
    d: Option<i64>,
    h: Option<i64>,
    props: Option<BTreeMap<String, String>>,
}

fn area(element: &Element, project: &Id, cell: (i64, i64)) -> Place {
    Place::Area {
        project: project.clone(),
        model: element.model.clone(),
        level: element.level,
        x: cell.0,
        y: cell.1,
        element: element.id.clone(),
    }
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
    txn.put(
        Place::Record {
            project: project.clone(),
            hash: hash.clone(),
        }
        .key(),
        bytes,
    )
    .await?;
    Ok(hash)
}

async fn write_cells<T: Txn>(
    txn: &mut T,
    project: &Id,
    element: &Element,
    shape: &Shape,
    hash: &Id,
) -> Result<(), StoreError> {
    for cell in cells(element, shape) {
        txn.put(area(element, project, cell).key(), to_row(&hash.as_str())?)
            .await?;
    }
    Ok(())
}

async fn clear_cells<T: Txn>(
    txn: &mut T,
    project: &Id,
    element: &Element,
    shape: &Shape,
) -> Result<(), StoreError> {
    for cell in cells(element, shape) {
        txn.delete(area(element, project, cell).key()).await?;
    }
    Ok(())
}

async fn create<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
    args: Args,
) -> Result<Outcome, StoreError> {
    let (Some(id), Some(kind), Some(x), Some(y), Some(w), Some(d), Some(h)) = (
        args.element,
        args.kind,
        args.x,
        args.y,
        args.w,
        args.d,
        args.h,
    ) else {
        return Ok(Err(
            "A new element needs an id, a kind, a position and a size.".to_owned(),
        ));
    };
    if w <= 0 || d <= 0 || h <= 0 {
        return Ok(Err("A box needs a size above zero.".to_owned()));
    }
    let pointer = Place::Element {
        project: project.clone(),
        model: edit.target.clone(),
        element: id.clone(),
    };
    if txn.get(&pointer.key()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let shape = Shape { w, d, h };
    let shape_bytes = to_row(&shape)?;
    let shape_hash = parse_hash(&sha256_hex(&shape_bytes))?;
    let place = Place::Shape {
        project: project.clone(),
        hash: shape_hash.clone(),
    };
    txn.put(place.key(), shape_bytes).await?;
    let element = Element {
        id: id.clone(),
        model: edit.target.clone(),
        kind,
        name: args.name.unwrap_or_else(|| id.to_string()),
        level: args.level.unwrap_or(0),
        x,
        y,
        shape: shape_hash,
        props: args.props.unwrap_or_default(),
    };
    let hash = write_record(txn, project, &element).await?;
    write_cells(txn, project, &element, &shape, &hash).await?;
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
    let model = &edit.target;
    let pointer = Place::Element {
        project: project.clone(),
        model: model.clone(),
        element: id.clone(),
    };
    let hash: Option<String> = read_row(txn, &pointer.key()).await?;
    let Some(hash) = hash else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    let hash = parse_hash(&hash)?;
    let holder =
        claims::held_by_another(txn, project, model, &id, &edit.person, context.now_ms).await?;
    if let Some(claim) = holder {
        return Ok(Err(format!("{} is working on this.", claim.name)));
    }
    let person: Option<Person> = read_row(txn, &Place::Person(edit.person.clone()).key()).await?;
    let name = person.map_or_else(|| edit.person.to_string(), |person| person.name);
    let mine = Claim {
        person: edit.person.clone(),
        name,
        until_ms: context.now_ms + claim_ms,
    };

    match edit.cmd.as_str() {
        "element.claim" => {
            let claim = claims::take(txn, project, model, &id, mine).await?;
            return Ok(Ok(Applied {
                claims: vec![claim],
                ..Applied::default()
            }));
        }
        "element.release" => {
            let claim = claims::release(txn, project, model, &id).await?;
            return Ok(Ok(Applied {
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
    let place = Place::Shape {
        project: project.clone(),
        hash: before.shape.clone(),
    };
    let shape: Shape = read_row(txn, &place.key())
        .await?
        .ok_or_else(|| StoreError::Damaged("shape missing".into()))?;
    let mut after = before.clone();
    match edit.cmd.as_str() {
        "element.move" => {
            let (Some(x), Some(y)) = (args.x, args.y) else {
                return Ok(Err("A move needs x and y.".to_owned()));
            };
            after.x = x;
            after.y = y;
        }
        "element.set" => {
            if let Some(name) = args.name {
                after.name = name;
            }
            after.props.extend(args.props.unwrap_or_default());
        }
        "element.delete" => {
            clear_cells(txn, project, &before, &shape).await?;
            txn.delete(pointer.key()).await?;
            let claim = claims::release(txn, project, model, &id).await?;
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
        other => return Ok(Err(format!("Unknown command {other}."))),
    }

    let new_hash = write_record(txn, project, &after).await?;
    clear_cells(txn, project, &before, &shape).await?;
    write_cells(txn, project, &after, &shape, &new_hash).await?;
    txn.put(pointer.key(), to_row(&new_hash.as_str())?).await?;
    let claim = claims::take(txn, project, model, &id, mine).await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn door(x: i64, y: i64) -> Element {
        Element {
            id: Id::parse("d").unwrap(),
            model: Id::parse("m").unwrap(),
            kind: "door".to_owned(),
            name: "D".to_owned(),
            level: 2,
            x,
            y,
            shape: Id::parse("s").unwrap(),
            props: BTreeMap::new(),
        }
    }

    #[test]
    fn a_box_across_a_cell_line_is_in_both_cells() {
        let shape = Shape {
            w: 900,
            d: 100,
            h: 2100,
        };
        assert_eq!(cells(&door(100, 100), &shape), vec![(0, 0)]);
        assert_eq!(cells(&door(4500, 100), &shape), vec![(0, 0), (1, 0)]);
    }

    #[test]
    fn negative_positions_get_their_own_cells() {
        let shape = Shape {
            w: 100,
            d: 100,
            h: 100,
        };
        assert_eq!(cells(&door(-200, -200), &shape), vec![(-1, -1)]);
    }
}
