//! The doors, windows and plain openings hosted in a document's walls, and the stretch of their
//! wall each one opens (issues about warnings and wall quantities). What the warnings read to find
//! an opening that crosses a wall joined to its host, and what a wall's quantities read to take
//! its openings out.
//!
//! What it promises:
//! - An [`Insert`] is one element hosted on a wall, placed as the wall's builder places it:
//!   [`crate::hosting::seat`] puts it in its host's frame, and [`crate::kernel::openings`] asks
//!   the builder's own rules, without the engine, where its opening lies. So what is read here is
//!   what is built, and a door the builder would refuse is not read (its wall's build failure says
//!   why; a refused set of inserts gives none).
//! - Every number is a whole number of the building's grid steps ([`construct_params::Length`]),
//!   exact: the plan stretch in the model's own coordinates (the host's placement added), the
//!   foot and the head up from the host's base. Nothing is rounded here.
//! - It reads the document as it stands now ([`crate::parameters::read_document`]), so the same
//!   call after an edit gives the new inserts.
//!
//! Depends on this crate's [`crate::hosting`], [`crate::joins`], [`crate::kernel`] and
//! [`crate::rooms`], and on construct-model's records. Nothing here is interim.

use crate::families;
use crate::hosting::{self, Hosting};
use crate::joins;
use crate::kernel::{self, Opened};
use crate::levels;
use crate::parameters::{self, At};
use crate::rooms::{WALL_KIND, level_of};
use construct_categories::{DOORS, WINDOWS};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{Edition, Hosted as HostedAt, Recipe};
use construct_model::keys::Place;
use construct_model::rows::Model;
use construct_params::Length;
use kit_core::Id;
use kit_store::{Reader, StoreError, read_row};
use std::collections::BTreeMap;

/// One element hosted on a wall, and the opening it cuts in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Insert {
    /// The hosted element's permanent ID.
    pub id: Id,
    /// What it is called in a sentence: `door D-1`, `window W-3`, `opening O-2`.
    pub who: String,
    /// The wall that hosts it.
    pub host: Id,
    /// The level its wall stands on, when the wall's base is tied to one.
    pub level: Option<Id>,
    /// The plan axis the wall runs along: 0 for x, 1 for y.
    pub axis: usize,
    /// Where its opening starts along that axis, in the model's plan, in grid steps.
    pub low: i64,
    /// Where it ends.
    pub high: i64,
    /// Its foot, up from the wall's base, in grid steps.
    pub foot: i64,
    /// Its head, up from the wall's base.
    pub head: i64,
}

/// What a hosted element is called: a door or a window by its category, else by what hosts it.
fn who_of(element: &Element, recipe: &Recipe, noun: &str) -> String {
    let category =
        parameters::stored_category(element).or_else(|| parameters::category_of(recipe).ok());
    let word = match category {
        Some(c) if c == DOORS => "door",
        Some(c) if c == WINDOWS => "window",
        _ => noun,
    };
    format!("{word} {}", element.id)
}

/// The model's grid steps of a placement's whole millimetres, if they fit.
fn steps_of(mm: i64) -> Option<i64> {
    Length::checked_from_mm(mm).map(Length::steps)
}

/// The edition a document records, from its model row.
async fn edition_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Option<Edition>, StoreError> {
    let place = Place::Model {
        project: project.clone(),
        model: document.clone(),
    };
    let model: Option<Model> = read_row(reader, &place.key()).await?;
    Ok(model.map(|m| m.edition))
}

/// Every insert hosted on a wall of the document, as it stands now, sorted by wall then by the
/// insert's ID. A wall that does not build, or whose inserts the builder would refuse, has none.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn in_document<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Vec<Insert>, StoreError> {
    let Some(edition) = edition_of(reader, project, document).await? else {
        return Ok(Vec::new());
    };
    let records = parameters::read_document(reader, project, document).await?;
    let mut found = Vec::new();
    for (_, wall) in records.values() {
        if wall.kind == WALL_KIND {
            found.extend(on_wall(reader, project, document, &edition, &records, wall).await?);
        }
    }
    Ok(found)
}

/// The inserts hosted on `wall`, as they stand now, sorted by ID.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn of_wall<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    wall: &Element,
) -> Result<Vec<Insert>, StoreError> {
    let Some(edition) = edition_of(reader, project, document).await? else {
        return Ok(Vec::new());
    };
    let records = parameters::read_document(reader, project, document).await?;
    on_wall(reader, project, document, &edition, &records, wall).await
}

async fn on_wall<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    edition: &Edition,
    records: &BTreeMap<Id, (Id, Element)>,
    wall: &Element,
) -> Result<Vec<Insert>, StoreError> {
    let at = At::Now;
    let Ok(host_recipe) = joins::recipe_at(reader, project, document, wall, at).await? else {
        return Ok(Vec::new());
    };
    // What is hosted on this wall: each element's recipe filled, and where the wall's builder
    // seats it in the wall's frame.
    let mut seated: Vec<(String, Recipe, Placement, Element)> = Vec::new();
    for (id, (_, element)) in records {
        let Ok(Some(hosting)) = hosting::read(element) else {
            continue;
        };
        if !matches!(hosting, Hosting::OnFace { .. } | Hosting::ByPosition { .. })
            || *hosting.host() != wall.id
        {
            continue;
        }
        let Ok(recipe) = levels::recipe_at(reader, project, document, element, at).await? else {
            continue;
        };
        if let Ok((place, recipe)) =
            hosting::seat(edition, wall, &host_recipe, element, &recipe, &hosting)
        {
            seated.push((id.to_string(), recipe, place, element.clone()));
        }
    }
    if seated.is_empty() {
        return Ok(Vec::new());
    }
    let list: Vec<HostedAt<'_>> = seated
        .iter()
        .map(|(key, recipe, place, _)| HostedAt {
            key,
            recipe,
            at: *place,
        })
        .collect();
    let Ok(opened) = kernel::openings(edition, &host_recipe, &list, &*families::baked()) else {
        return Ok(Vec::new());
    };
    let level = level_of(wall);
    let mut out = Vec::with_capacity(opened.len());
    for Opened {
        key,
        noun,
        axis,
        low,
        high,
        foot,
        head,
    } in opened
    {
        let Some((_, recipe, _, element)) = seated.iter().find(|(k, ..)| *k == key) else {
            continue;
        };
        let origin = [wall.placement.x, wall.placement.y]
            .get(axis)
            .copied()
            .and_then(steps_of);
        let (Some(origin), Ok(id)) = (origin, Id::parse(&key)) else {
            continue;
        };
        let (Some(low), Some(high)) = (low.checked_add(origin), high.checked_add(origin)) else {
            continue;
        };
        out.push(Insert {
            id,
            who: who_of(element, recipe, noun),
            host: wall.id.clone(),
            level: level.clone(),
            axis,
            low,
            high,
            foot,
            head,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}
