//! The seat a builder is handed with an element hosted on a wall at an angle (piece A83,
//! CON-A4-F3-P1): the recipe argument [`SEAT_ARG`], written where the server works out where a
//! hosted element sits (construct-regen's `hosting`, which re-exports these) and read by both
//! builders here ([`crate::kernel`], [`crate::stand_in`]) and by the void builder
//! ([`crate::voids`]), so the browser's builds read it as the server's do. What a seat is:
//! construct-families' [`Seat`], a way, a base and an origin in grid steps, every point it puts
//! rounded once to the grid. Derived from the host, never stored.
//!
//! Depends on construct-model (recipes) and construct-families (the seat). Nothing here is
//! interim.

use construct_families::Seat;
use construct_model::geometry::{GeometryError, Recipe};

/// The argument a builder is handed with an element hosted on a wall at an angle: its seat in the
/// wall's frame (construct-regen's `hosting` says how it is worked out),
/// `{"way": [x, y], "base": [x, y], "at": [x, y]}` in grid steps. Derived from the host, never
/// stored.
pub const SEAT_ARG: &str = "seat";

/// `recipe` with the seat a builder is handed ([`SEAT_ARG`]).
#[must_use]
pub fn with_seat(recipe: &Recipe, seat: &Seat) -> Recipe {
    let mut seated = recipe.clone();
    seated.args.insert(
        SEAT_ARG.to_owned(),
        serde_json::json!({ "way": seat.way(), "base": seat.base(), "at": seat.at() }),
    );
    seated
}

/// The seat `recipe` was handed ([`SEAT_ARG`]), if any.
///
/// # Errors
///
/// `BadSeat` for a seat that is not three pairs of whole grid steps, its way of some length.
pub fn seat_of(recipe: &Recipe) -> Result<Option<Seat>, GeometryError> {
    let Some(value) = recipe.args.get(SEAT_ARG) else {
        return Ok(None);
    };
    let bad = || GeometryError {
        code: "BadSeat".to_owned(),
        message: format!(
            "{SEAT_ARG} must be a way, a base and an origin, each two whole numbers of grid \
             steps, the way of some length"
        ),
    };
    let object = value
        .as_object()
        .filter(|object| object.len() == 3)
        .ok_or_else(bad)?;
    let pair = |name: &str| {
        object
            .get(name)
            .and_then(serde_json::Value::as_array)
            .and_then(|pair| match pair.as_slice() {
                [x, y] => Some([x.as_i64()?, y.as_i64()?]),
                _ => None,
            })
            .ok_or_else(bad)
    };
    Seat::new(pair("way")?, pair("base")?, pair("at")?)
        .map(Some)
        .ok_or_else(bad)
}

/// `recipe` as stored, without the seat it was handed, and that seat, if any.
///
/// # Errors
///
/// [`seat_of`]'s.
pub fn unseated(recipe: &Recipe) -> Result<(Recipe, Option<Seat>), GeometryError> {
    let seat = seat_of(recipe)?;
    let mut stored = recipe.clone();
    stored.args.remove(SEAT_ARG);
    Ok((stored, seat))
}
