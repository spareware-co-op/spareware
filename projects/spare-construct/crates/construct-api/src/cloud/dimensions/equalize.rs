//! `dimension.equalize`: the segments of each group made equal by moving the elements the
//! dimension's references are of, and the groups stored, all in one entry of the edit log, so one
//! undo puts every element back.
//!
//! construct-dimensions' [`equalize`](construct_dimensions::equalize) works out how far each
//! element moves along the dimension (the run's ends stay; segments outside a group keep their
//! length). Each element is moved the way its own commands move it, and checked by the same
//! rules: a layered wall's reference line shifts (its start and end, exact to the step); the old
//! box wall's placement shifts by whole millimetres, which is all it holds; a grid line moves as
//! `grid.move` moves it, checked by construct-grids; a door, a window or an opening moves along its
//! wall as `hosted.move` moves it, checked by the builder's fit with what else the wall hosts.
//! Afterwards every group is measured again and must hold (equal to the step); anything else is
//! refused and nothing is written.
//!
//! Refused by name, naming the reference and what is in the way: another person's claim on an
//! element (they are working on it); a box wall that would move by part of a millimetre; an
//! opening whose wall runs across the dimension, so moving it along its wall does not move it
//! along the dimension; an element that would not fit its host there, in the builder's words; and
//! what the data crate refuses (a reference that cannot be used, an element at two references
//! that would move by two amounts).

use super::{Answer, Run, World, dimension_of, located, measure_now, need, wall_of, words};
use crate::cloud::elements::write_record;
use crate::cloud::hosting::{Which, fit, host_for};
use construct_dimensions::{Dimension, Feature, Move, equalize};
use construct_grids::Datums;
use construct_model::element::{Element, Placement};
use construct_params::{Length, Value};
use construct_regen::hosting::{self, HOST_OP, Hosting};
use construct_regen::wall_plan;
use kit_core::Id;
use kit_ops::{Change, Claim};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, claims};

use super::{Args, Named};

/// One element to write: its piece, the hash it replaces, and the record it becomes.
struct Moved {
    piece: Id,
    before: Id,
    after: Element,
    /// The references it is, for the words of a refusal.
    references: Vec<usize>,
    /// For an opening: what its fit is asked against.
    hosted: Option<crate::cloud::hosting::Host>,
}

/// `Reference 2 (the grid line G-2)`.
fn named(dimension: &Dimension, at: &[usize]) -> String {
    let Some(&first) = at.first() else {
        return "A reference".to_owned();
    };
    let words = dimension
        .references()
        .get(first)
        .map_or_else(String::new, construct_dimensions::Reference::describe);
    format!("Reference {} ({words})", first.saturating_add(1))
}

fn shifted(length: Length, by: i64) -> Option<Length> {
    length.steps().checked_add(by).map(Length::from_steps)
}

/// A layered wall's recipe with its start and end moved by `by`.
fn moved_line(record: &Element, by: [i64; 2]) -> Result<Element, String> {
    let mut after = record.clone();
    for end in ["start", "end"] {
        let point = after
            .recipe
            .args
            .get(end)
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("its {end} cannot be read"))?;
        let mut moved = Vec::new();
        for (at, shift) in point.iter().zip(by) {
            let Some(Value::Length(length)) = at.as_str().and_then(Value::from_stored) else {
                return Err(format!("its {end} cannot be read"));
            };
            let now = shifted(length, shift)
                .filter(|l| {
                    l.steps().unsigned_abs() <= construct_grids::LIMIT.steps().unsigned_abs()
                })
                .ok_or_else(|| "it would move beyond the kernel's grid".to_owned())?;
            moved.push(serde_json::Value::String(Value::Length(now).stored()));
        }
        after
            .recipe
            .args
            .insert(end.to_owned(), serde_json::Value::Array(moved));
    }
    Ok(after)
}

/// The old box wall's placement moved by `by`: whole millimetres only.
fn moved_box(record: &Element, by: [i64; 2]) -> Result<Element, String> {
    let whole = |steps: i64| (steps.rem_euclid(6_400) == 0).then(|| steps.div_euclid(6_400));
    let (Some(dx), Some(dy)) = (whole(by[0]), whole(by[1])) else {
        return Err(format!(
            "it is a box wall, which moves by whole millimetres only, and it would have to move by \
             {} and {} across and up",
            Length::from_steps(by[0]),
            Length::from_steps(by[1])
        ));
    };
    let mut after = record.clone();
    let place = |c: i64, d: i64| {
        c.checked_add(d)
            .ok_or_else(|| "it would move beyond the kernel's grid".to_owned())
    };
    after.placement = Placement {
        x: place(record.placement.x, dx)?,
        y: place(record.placement.y, dy)?,
        z: record.placement.z,
    };
    Ok(after)
}

/// The record an element becomes when its references move by `by`, and for an opening the host
/// its fit is asked against.
async fn moved_record<T: Txn>(
    run: &mut Run<'_, T>,
    world: &World,
    feature: Feature,
    record: &Element,
    by: [i64; 2],
) -> Result<Result<(Element, Option<crate::cloud::hosting::Host>), String>, StoreError> {
    match feature {
        Feature::WallFace(_) | Feature::WallLine => Ok(if wall_plan::is_layered(&record.recipe) {
            moved_line(record, by).map(|after| (after, None))
        } else if record.recipe.owner == "construct" && record.recipe.op == HOST_OP {
            moved_box(record, by).map(|after| (after, None))
        } else {
            Err("it is not a wall that moves".to_owned())
        }),
        Feature::GridLine => {
            let all: Vec<&Element> = world.all.values().map(|(_, r)| r).collect();
            let mut datums = match Datums::from_records(all.iter().copied()) {
                Ok(datums) => datums,
                Err(e) => return Ok(Err(e.to_string())),
            };
            let Some(grid) = datums.grid(&record.id) else {
                return Ok(Err("it is not a grid".to_owned()));
            };
            let moved = grid
                .shape()
                .moved(Length::from_steps(by[0]), Length::from_steps(by[1]))
                .and_then(|shape| datums.reshape_grid(&record.id, shape));
            if let Err(e) = moved {
                return Ok(Err(e.to_string()));
            }
            Ok(datums
                .grid(&record.id)
                .map(|grid| (grid.update_record(record), None))
                .ok_or_else(|| "it is not a grid".to_owned()))
        }
        Feature::Centre | Feature::Edge(_) => moved_along(run, record, by).await,
        Feature::Level => Ok(Err("a level does not move in a plan".to_owned())),
    }
}

/// An opening moved along its wall so that it moves `by` along the dimension.
async fn moved_along<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
    by: [i64; 2],
) -> Result<Result<(Element, Option<crate::cloud::hosting::Host>), String>, StoreError> {
    let (host_id, face, along, up) = match hosting::read(record) {
        Ok(Some(Hosting::OnFace {
            host,
            face,
            along,
            up,
        })) => (host, face, along, up),
        Ok(_) => {
            return Ok(Err(
                "it is not hosted at an offset along a wall: move it with hosted.move once first"
                    .to_owned(),
            ));
        }
        Err(why) => return Ok(Err(why)),
    };
    let host = match host_for(
        run.txn,
        run.project,
        run.document,
        &host_id,
        Which::Name(&face),
        "an opening",
    )
    .await?
    {
        Ok(host) => host,
        Err(why) => return Ok(Err(why)),
    };
    let axis = match wall_of(&host.record, &host.recipe) {
        Ok(Some(wall)) => wall.axis,
        Ok(None) => return Ok(Err("its host is not a wall".to_owned())),
        Err(missing) => return Ok(Err(missing.detail)),
    };
    let Some([ux, uy]) = axis else {
        return Ok(Err(
            "its wall runs at an angle, and only a wall along x or y hosts".to_owned(),
        ));
    };
    // Along its wall's direction: a wall across the dimension moves it nowhere along it.
    let w = by[0]
        .checked_mul(ux)
        .zip(by[1].checked_mul(uy))
        .and_then(|(a, b)| a.checked_add(b));
    let Some(w) = w.filter(|w| *w != 0) else {
        return Ok(Err(
            "it moves only along its wall, which runs across the dimension".to_owned(),
        ));
    };
    let Some(along) = shifted(along, w) else {
        return Ok(Err("it would move beyond the kernel's grid".to_owned()));
    };
    let mut after = record.clone();
    hosting::write(&mut after, &host_id, &host.face, along, up);
    Ok(Ok((after, Some(host))))
}

/// The claim this person takes on a piece, or the name of whoever else holds it.
async fn claim_of<T: Txn>(
    run: &mut Run<'_, T>,
    piece: &Id,
) -> Result<Result<Claim, String>, StoreError> {
    if let Some(claim) = claims::held_by_another(
        run.txn,
        run.project,
        run.document,
        piece,
        &run.edit.person,
        run.now_ms,
    )
    .await?
    {
        return Ok(Err(claim.name));
    }
    let person: Option<Named> =
        read_row(run.txn, &Place::Person(run.edit.person.clone()).key()).await?;
    let name = person.map_or_else(|| run.edit.person.to_string(), |person| person.name);
    Ok(Ok(Claim {
        person: run.edit.person.clone(),
        name,
        // milliseconds since 1970 plus a claim's length: far below u64::MAX
        until_ms: run.now_ms.saturating_add(run.claim_ms),
    }))
}

/// Writes one record over its piece, taking this person's claim on it.
async fn write_one<T: Txn>(
    run: &mut Run<'_, T>,
    record: &Element,
    before: Option<Id>,
    claim: Claim,
    applied: &mut Applied,
) -> Result<(), StoreError> {
    let hash = write_record(run.txn, run.project, record).await?;
    let pointer = Place::Piece {
        project: run.project.clone(),
        document: run.document.clone(),
        piece: record.id.clone(),
    };
    run.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
    applied
        .claims
        .push(claims::take(run.txn, run.project, run.document, &record.id, claim).await?);
    applied.changes.push(Change {
        piece: record.id.clone(),
        before,
        after: Some(hash),
    });
    Ok(())
}

/// The records each move makes, or the refusal of the first that cannot be made.
async fn records_of<T: Txn>(
    run: &mut Run<'_, T>,
    world: &World,
    dimension: &Dimension,
    moves: &[Move],
) -> Result<Result<Vec<Moved>, String>, StoreError> {
    let mut found = Vec::new();
    for one in moves {
        let who = named(dimension, &one.references);
        let Some((hash, record)) = world.record(&one.element).cloned() else {
            return Ok(Err(format!("{who} cannot move: its element is not there.")));
        };
        let Some(feature) = one
            .references
            .first()
            .and_then(|&at| dimension.references().get(at))
            .map(construct_dimensions::Reference::feature)
        else {
            continue;
        };
        match moved_record(run, world, feature, &record, one.by).await? {
            Ok((after, hosted)) => found.push(Moved {
                piece: one.element.clone(),
                before: hash,
                after,
                references: one.references.clone(),
                hosted,
            }),
            Err(why) => return Ok(Err(format!("{who} cannot move: {why}."))),
        }
    }
    Ok(Ok(found))
}

/// Writes the moved records: walls and grids first, then the openings, each of which is asked to
/// fit once its wall and the other openings are where they go, trying each again until all fit or
/// none more does. The inner error is the refusal of the first that does not fit.
async fn write_moves<T: Txn>(
    run: &mut Run<'_, T>,
    old: &Dimension,
    moved: Vec<Moved>,
    held_claims: Vec<Claim>,
    applied: &mut Applied,
) -> Result<Result<(), String>, StoreError> {
    let mut pending = Vec::new();
    for (one, claim) in moved.into_iter().zip(held_claims) {
        if one.hosted.is_none() {
            write_one(run, &one.after, Some(one.before.clone()), claim, applied).await?;
        } else {
            pending.push((one, claim));
        }
    }
    while !pending.is_empty() {
        let before_count = pending.len();
        let mut waiting = Vec::new();
        let mut refusal = None;
        for (mut one, claim) in pending {
            let Some(host) = &one.hosted else { continue };
            match fit(run.txn, run.project, run.document, host, &one.after).await? {
                Ok(placement) => {
                    one.after.placement = placement;
                    write_one(run, &one.after, Some(one.before.clone()), claim, applied).await?;
                }
                Err(why) => {
                    refusal.get_or_insert_with(|| {
                        format!("{} cannot move: {why}.", named(old, &one.references))
                    });
                    waiting.push((one, claim));
                }
            }
        }
        if waiting.len() == before_count {
            return Ok(Err(refusal.unwrap_or_else(|| {
                "Cannot make the segments equal: an opening would not fit its wall.".to_owned()
            })));
        }
        pending = waiting;
    }
    Ok(Ok(()))
}

/// Runs `dimension.equalize`.
pub(super) async fn apply<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match dimension_of(world, args.dimension.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let groups = match need(args.groups.clone(), "groups of segments to make equal") {
        Ok(groups) if !groups.is_empty() => groups,
        Ok(_) => {
            return Ok(Err(
                "This command needs groups of segments to make equal: dimension.equal with none \
                 clears them."
                    .to_owned(),
            ));
        }
        Err(refusal) => return Ok(Err(refusal)),
    };
    let held = match old.clone().with_equalities(groups.clone()) {
        Ok(held) => held,
        Err(e) => return Ok(Err(words(&e))),
    };
    let references: Vec<&construct_dimensions::Reference> = old.references().iter().collect();
    let table = located(run.txn, run.project, run.document, &references).await?;
    let wanted = match equalize(old, &groups, &table) {
        Ok(wanted) => wanted,
        Err(e) => return Ok(Err(words(&e))),
    };
    let moved = match records_of(run, world, old, &wanted).await? {
        Ok(moved) => moved,
        Err(refusal) => return Ok(Err(refusal)),
    };
    // Every claim first: another person's on any element turns the whole command back.
    let mut held_claims = Vec::new();
    for one in &moved {
        match claim_of(run, &one.piece).await? {
            Ok(claim) => held_claims.push(claim),
            Err(name) => {
                return Ok(Err(format!(
                    "{} cannot move: {name} is working on it.",
                    named(old, &one.references)
                )));
            }
        }
    }
    let dimension_claim = if held == *old {
        None
    } else {
        match claim_of(run, old.id()).await? {
            Ok(claim) => Some(claim),
            Err(name) => return Ok(Err(format!("{name} is working on this."))),
        }
    };
    let mut applied = Applied::default();
    if let Err(refusal) = write_moves(run, old, moved, held_claims, &mut applied).await? {
        return Ok(Err(refusal));
    }
    if let Some(claim) = dimension_claim {
        write_one(
            run,
            &held.to_record(run.document),
            Some(hash.clone()),
            claim,
            &mut applied,
        )
        .await?;
    }
    // Measured again as written: every group holds, or nothing is.
    let measured = measure_now(run, &held).await?;
    for state in &measured.equalities {
        if state.holds != Some(true) {
            let segments: Vec<String> = state
                .segments
                .iter()
                .map(|s| s.saturating_add(1).to_string())
                .collect();
            return Ok(Err(format!(
                "Cannot make the segments equal: segments {} would still differ once moved.",
                segments.join(", ")
            )));
        }
    }
    Ok(Ok(applied))
}
