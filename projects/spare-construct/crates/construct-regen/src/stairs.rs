//! How a stair's recipe is laid out from the height between its levels and filled with its pieces
//! (job 6.7, CON-A4-F5-P1), and what a page shows of it. The regenerator fills a stair's recipe
//! through this one module before it is built, and the server's commands and views read a stair's
//! layout through it too, so a command, a view and a build cannot read a stair differently.
//!
//! What it promises:
//! - A stair's recipe reads its rise with `{"tie":"top"}`, which [`crate::levels::filled`] fills
//!   with the height from its base to its top tie as a construct-params stored length; [`filled`]
//!   then lays it out by its rule set and replaces its settings with the pieces the builders take
//!   (construct-stairs' `filled`; [`crate::stair_plan`] reads the result). A recipe that is not a
//!   stair's, or whose rise is not read yet, is returned as it is. Its `reads` row lists its base
//!   and top levels (construct-levels' ties, [`crate::levels::levels_read`]), so a moved level
//!   lays the stair out again; its record never changes.
//! - A stair's layout as the document stands `at` a state ([`layout_at`]): its settings and its
//!   [`Layout`], or the refusal in construct-stairs' words (a limit that a moved level now breaks
//!   names the limit and the nearest heights that work).
//! - What a page shows of a stair ([`read_only_at`]): its rise, risers, riser height, treads,
//!   going, total run, pitch and width (construct-stairs' `read_only`), worked out each time it is
//!   read, never stored, each shown in the document's units and marked `read-only`, as a floor's
//!   and a roof's are; none where it cannot be laid out (its built row and the stairs view say
//!   why).
//!
//! Depends on construct-model (the recipe), construct-stairs (the stair, its layout and its
//! pieces), construct-params (units) and the [`crate::levels`] reader. Nothing here is interim.

use crate::levels;
use crate::parameters::{At, ParameterView};
use crate::slabs::shown;
use construct_model::element::Element;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::Units;
use construct_stairs::{Layout, Stair, StairError, is_stair_recipe, laid_out, read_only, rise_of};
use kit_core::Id;
use kit_store::{Reader, StoreError};

fn failure(error: &StairError) -> GeometryError {
    GeometryError {
        code: "BadStair".to_owned(),
        message: error.to_string(),
    }
}

/// The recipe a stair is built from: laid out from its rise and filled with its pieces. A recipe
/// that is not a stair's, or whose rise is not read yet, is returned as it is.
///
/// # Errors
/// A build failure, which is model state like any other: `BadStair` for a stair construct-stairs
/// refuses (its settings, a limit of its rule set, its headroom), in its words.
pub fn filled(recipe: &Recipe) -> Result<Recipe, GeometryError> {
    if !is_stair_recipe(recipe) {
        return Ok(recipe.clone());
    }
    match rise_of(recipe) {
        Ok(Some(_)) => construct_stairs::filled(recipe).map_err(|e| failure(&e)),
        Ok(None) | Err(_) => Ok(recipe.clone()),
    }
}

/// A stair's settings and layout as the document stands `at` that state: its rise read from its
/// levels. The inner error is why it has none, in words: a level it cannot read, or construct-
/// stairs' refusal.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn layout_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
) -> Result<Result<(Stair, Layout), String>, StoreError> {
    Ok(
        match levels::placed(reader, project, document, element, at).await? {
            Ok(placed) => match levels::filled(&element.recipe, &placed) {
                Ok(recipe) => laid_out(&recipe).map_err(|e| e.to_string()),
                Err(why) => Err(why.message),
            },
            Err(why) => Err(why.message),
        },
    )
}

/// A stair's read-only parameters as a page shows them, as the module header says; none for an
/// element that is not a stair or cannot be laid out.
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
    if !construct_stairs::is_stair(element) {
        return Ok(Vec::new());
    }
    let Ok((_, layout)) = layout_at(reader, project, document, element, at).await? else {
        return Ok(Vec::new());
    };
    Ok(read_only(&layout).map_or_else(
        |_| Vec::new(),
        |rows| {
            rows.into_iter()
                .map(|(def, value)| shown(&def, value, units))
                .collect()
        },
    ))
}
