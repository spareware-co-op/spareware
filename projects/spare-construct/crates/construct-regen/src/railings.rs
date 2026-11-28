//! How a railing's recipe is laid out along its path or its stair and filled with its pieces (job
//! 14.2, CON-A4-F5-P3), and what a page shows of it. The regenerator fills a railing's recipe
//! through this one module before it is built, and the server's commands and views read a
//! railing's layout through it too, so a command, a view and a build cannot read a railing
//! differently.
//!
//! What it promises:
//! - A sketched railing is laid out from its own recipe; one on a stair from the stair as the
//!   document stands `at` that state: its record, its layout from its levels (construct-stairs,
//!   through [`crate::stairs::layout_at`]) and how far its base stands above the railing's, so a
//!   stair that is changed, moved or laid out again by a moved level lays its railings out again
//!   ([`filled`]). The filled recipe is construct-railings' `filled`, which
//!   [`crate::railing_plan`] reads. A recipe that is not a railing's is returned as it is.
//! - What a railing reads, for its `reads` row ([`reads`]): its stair and the stair's levels, so
//!   a change to any of them builds it again; the regenerator also builds again every railing
//!   whose row lists an element with a shape that changed.
//! - A railing's layout as the document stands ([`layout_at`]), what a page shows of it
//!   ([`read_only_at`]: rail and handrail lengths, balusters, posts and height, read-only) and
//!   its warnings ([`warnings_at`]), each worked out each time it is read, never stored.
//! - A stair that is not in the model, or cannot be laid out, is the railing's build failure,
//!   naming it (`HostMissing`, `BadHost`); its record stays.
//!
//! Depends on construct-model (the recipe), construct-railings (the railing, its layout, pieces
//! and warnings), construct-stairs (the stair it follows) and the [`crate::levels`] and
//! [`crate::parameters`] readers. Nothing here is interim.

use crate::levels;
use crate::parameters::{At, ParameterView, read_at};
use crate::slabs::shown;
use construct_model::element::Element;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Length, Units};
use construct_railings::{
    Layout, Railing, RailingError, StairHost, Warning, is_railing_recipe, railing_of, read_only,
};
use construct_stairs::{Layout as StairLayout, STAIR_KIND, Stair};
use kit_core::Id;
use kit_store::{Reader, StoreError};
use std::collections::BTreeSet;

fn failure(error: &RailingError) -> GeometryError {
    GeometryError {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

/// The stair a railing follows, laid out, and how far its base stands above the railing's.
struct Host {
    stair: Stair,
    layout: StairLayout,
    shift: Length,
}

impl Host {
    fn borrowed(&self) -> StairHost<'_> {
        StairHost {
            stair: &self.stair,
            layout: &self.layout,
            shift: self.shift,
        }
    }
}

/// The stair `railing` follows, as the document stands `at` that state, for a railing whose base
/// stands at `own` above the project's zero; none for a sketched railing.
async fn host<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    railing: &Railing,
    own: Option<Length>,
    at: At,
) -> Result<Result<Option<Host>, GeometryError>, StoreError> {
    let Some(id) = railing.stair() else {
        return Ok(Ok(None));
    };
    let Some(record) = read_at(reader, project, document, STAIR_KIND, id, at).await? else {
        return Ok(Err(GeometryError {
            code: "HostMissing".to_owned(),
            message: format!("its stair {id} is not in the model"),
        }));
    };
    let (stair, layout) =
        match crate::stairs::layout_at(reader, project, document, &record, at).await? {
            Ok(laid) => laid,
            Err(why) => {
                return Ok(Err(GeometryError {
                    code: "BadHost".to_owned(),
                    message: format!("its stair {}: {why}", record.name),
                }));
            }
        };
    let base = match levels::placed(reader, project, document, &record, at).await? {
        Ok(placed) => placed.elevation,
        Err(e) => return Ok(Err(e)),
    };
    let shift = match (base, own) {
        (Some(base), Some(own)) => base.checked_sub(own),
        _ => Some(Length::from_steps(0)),
    };
    Ok(match shift {
        Some(shift) => Ok(Some(Host {
            stair,
            layout,
            shift,
        })),
        None => Err(GeometryError {
            code: "TooBig".to_owned(),
            message: "its stair is too far above or below it".to_owned(),
        }),
    })
}

/// The recipe a railing is built from: laid out along its path or its stair and filled with its
/// pieces, for a railing whose base stands at `elevation`. A recipe that is not a railing's is
/// returned as it is.
///
/// # Errors
/// A [`StoreError`] when the store fails. The inner error is the build failure, which is model
/// state like any other: construct-railings' refusal by its code, or the stair's (`HostMissing`,
/// `BadHost`).
pub async fn filled<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    recipe: Recipe,
    elevation: Option<Length>,
    at: At,
) -> Result<Result<Recipe, GeometryError>, StoreError> {
    if !is_railing_recipe(&recipe) {
        return Ok(Ok(recipe));
    }
    let railing = match railing_of(&recipe) {
        Ok(railing) => railing,
        Err(e) => return Ok(Err(failure(&e))),
    };
    let host = match host(reader, project, document, &railing, elevation, at).await? {
        Ok(host) => host,
        Err(e) => return Ok(Err(e)),
    };
    Ok(
        construct_railings::filled(&recipe, host.as_ref().map(Host::borrowed))
            .map_err(|e| failure(&e)),
    )
}

/// A railing and its layout as the document stands `at` that state. The inner error is why it has
/// none, in words: a level it cannot read, its stair's, or construct-railings' refusal.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn layout_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
) -> Result<Result<(Railing, Layout), String>, StoreError> {
    let own = match levels::placed(reader, project, document, element, at).await? {
        Ok(placed) => placed.elevation,
        Err(why) => return Ok(Err(why.message)),
    };
    let railing = match railing_of(&element.recipe) {
        Ok(railing) => railing,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let host = match host(reader, project, document, &railing, own, at).await? {
        Ok(host) => host,
        Err(e) => return Ok(Err(e.message)),
    };
    Ok(
        construct_railings::laid_out(&element.recipe, host.as_ref().map(Host::borrowed))
            .map_err(|e| e.to_string()),
    )
}

/// Every record a railing's shape reads beyond its own levels, by ID: its stair and the stair's
/// levels, as the document stands `at` that state. None for a sketched railing.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn reads<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
) -> Result<BTreeSet<String>, StoreError> {
    let mut read = BTreeSet::new();
    if !construct_railings::is_railing(element) {
        return Ok(read);
    }
    let Some(stair) = construct_railings::stair_of(&element.recipe) else {
        return Ok(read);
    };
    if let Some(record) = read_at(reader, project, document, STAIR_KIND, &stair, at).await? {
        read.extend(
            levels::levels_read(&record)
                .into_iter()
                .map(|id| id.to_string()),
        );
    }
    read.insert(stair.to_string());
    Ok(read)
}

/// A railing's read-only parameters as a page shows them, as the module header says; none for an
/// element that is not a railing or cannot be laid out.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_only_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    units: &Units,
    at: At,
) -> Result<Vec<ParameterView>, StoreError> {
    if !construct_railings::is_railing(element) {
        return Ok(Vec::new());
    }
    let Ok((railing, layout)) = layout_at(reader, project, document, element, at).await? else {
        return Ok(Vec::new());
    };
    Ok(read_only(&railing, &layout).map_or_else(
        |_| Vec::new(),
        |rows| {
            rows.into_iter()
                .map(|(def, value)| shown(&def, value, units))
                .collect()
        },
    ))
}

/// A railing's warnings as the document stands `at` that state, named by its record's name; none
/// for one that cannot be laid out (its built row says why).
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn warnings_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
) -> Result<Vec<Warning>, StoreError> {
    let Ok((railing, layout)) = layout_at(reader, project, document, element, at).await? else {
        return Ok(Vec::new());
    };
    Ok(
        construct_railings::warnings(&element.id, &element.name, &railing, &layout)
            .unwrap_or_default(),
    )
}
