//! The seat a builder is handed with an element hosted on a wall at an angle (piece A83,
//! CON-A4-F3-P1): the recipe argument [`SEAT_ARG`], written where the server works out where a
//! hosted element sits (construct-regen's `hosting`, which re-exports these) and read by both
//! builders here ([`crate::kernel`], [`crate::stand_in`]) and by the void builder
//! ([`crate::voids`]), so the browser's builds read it as the server's do. What a seat is:
//! construct-families' [`Seat`], a way, a base and an origin in grid steps, every point it puts
//! rounded once to the grid. Derived from the host, never stored.
//!
//! Where an element hosted on a wall at an angle sits and what its seat is
//! ([`seat_at_angle`]) is worked out here, for the server's regenerator (`hosting::seat` calls it)
//! and, as text in and out ([`seat_json`]), for the browser's packages, so that a page seats what
//! it builds exactly as the server does and never rounds a point itself.
//!
//! Depends on construct-model (recipes), construct-params (lengths), construct-openings (a plain
//! opening's width) and construct-families (the seat). Nothing here is interim.

use crate::families;
use crate::wall_plan::{self, Layered};
use construct_families::Seat;
use construct_model::element::Placement;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Length, Value};

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

/// What a hosted element is called in a refusal: a placed family, a curtain wall, a plain or floor
/// opening, or a door.
#[must_use]
pub fn noun(recipe: &Recipe) -> &'static str {
    if families::is_instance(recipe) {
        "family"
    } else if construct_curtain::is_curtain(recipe) {
        "curtain wall"
    } else if construct_openings::is_void(recipe) {
        "opening"
    } else {
        "door"
    }
}

/// A box hosted on a layered wall `wall`, `key` its element's ID, of the filled recipe `hosted_recipe`: its width
/// along the wall and how thick it is across it, in steps (a plain opening as thick as its wall).
pub(crate) fn size_in(
    wall: &Layered,
    key: &str,
    hosted_recipe: &Recipe,
    refuse: &impl Fn(&str, String) -> GeometryError,
) -> Result<(i64, i64), GeometryError> {
    let too_big = || refuse("TooBig", "it is too far from its host".to_owned());
    let steps = |recipe: &Recipe, arg: &str| {
        recipe
            .args
            .get(arg)
            .and_then(serde_json::Value::as_i64)
            .and_then(Length::checked_from_mm)
            .map(Length::steps)
            .ok_or_else(|| {
                refuse(
                    "MissingArgument",
                    format!("{arg} must be a whole number of millimetres"),
                )
            })
    };
    if construct_openings::is_wall_opening(hosted_recipe) {
        let opening =
            construct_openings::WallOpening::of(key, hosted_recipe).map_err(|e| GeometryError {
                code: e.code().to_owned(),
                message: e.to_string(),
            })?;
        let thickness = i64::try_from(wall.thickness()).map_err(|_| too_big())?;
        Ok((opening.width.steps(), thickness))
    } else {
        Ok((
            steps(hosted_recipe, "width")?,
            steps(hosted_recipe, "thickness")?,
        ))
    }
}

/// An offset along or up a host's face, as whole millimetres, or the refusal that names it.
fn whole_offset(
    what: &str,
    length: Length,
    refuse: &impl Fn(&str, String) -> GeometryError,
) -> Result<i64, GeometryError> {
    length.whole_mm().ok_or_else(|| {
        refuse(
            "NotWholeMillimetres",
            format!(
                "its offset {what} its host's face, {length}, is not a whole number of \
                 millimetres, as every placement is"
            ),
        )
    })
}

/// Where an element hosted on the layered wall `wall` at an angle sits and the recipe its builder
/// is handed (this module's header): its origin `[a, s]` in the wall's frame, steps along the line
/// from its start and to its left; its own x running the way the line runs (`sign` 1) or against
/// it (-1); `up` whole millimetres up. Placed at the nearest whole millimetre to its origin, as a
/// placement holds it, and seated ([`SEAT_ARG`]) so that every point of it is worked out from the
/// wall's frame and rounded once.
fn seated_in(
    wall: &Layered,
    [a, s]: [i64; 2],
    sign: i64,
    up: i64,
    recipe: &Recipe,
    refuse: &impl Fn(&str, String) -> GeometryError,
) -> Result<(Placement, Recipe), GeometryError> {
    let too_big = || refuse("TooBig", "it is too far from its host".to_owned());
    let start = wall.plan.start();
    let line = Seat::new(wall.plan.direction(), start, [0, 0]).ok_or_else(too_big)?;
    let origin = line.plan([a, s]).ok_or_else(too_big)?;
    let [x, y] = origin.map(|c| wall_plan::nearest_mm(c).0);
    let less = |at: i64, mm: i64| {
        mm.checked_mul(Length::STEPS_PER_MM)
            .and_then(|steps| at.checked_sub(steps))
            .ok_or_else(too_big)
    };
    let times = |v: i64| v.checked_mul(sign).ok_or_else(too_big);
    let [dx, dy] = wall.plan.direction();
    let seat = Seat::new(
        [times(dx)?, times(dy)?],
        [less(start[0], x)?, less(start[1], y)?],
        [times(a)?, times(s)?],
    )
    .ok_or_else(too_big)?;
    Ok((Placement { x, y, z: up }, with_seat(recipe, &seat)))
}

/// Where a box door, a plain opening or any other box sits on the side `role` of a layered wall
/// `wall` at an angle, `along` its line from its start and `up` from its base, and the recipe its
/// builder is handed: its origin at its box's corner on the face of that side at `along`, the box
/// reaching `along` the line by its width and across from that face toward the other side by its
/// own thickness (a plain opening by the wall's), seated there ([`seated_in`]).
fn at_angle(
    wall: &Layered,
    key: &str,
    hosted_recipe: &Recipe,
    (role, along, up): (&str, Length, Length),
    refuse: &impl Fn(&str, String) -> GeometryError,
) -> Result<(Placement, Recipe), GeometryError> {
    let too_big = || refuse("TooBig", "it is too far from its host".to_owned());
    let (_, own) = size_in(wall, key, hosted_recipe, refuse)?;
    let up = whole_offset("up", up, refuse)?;
    whole_offset("along", along, refuse)?;
    let (exterior, interior) = wall.outer();
    let (on, other) = if role == "interior" {
        (interior, exterior)
    } else {
        (exterior, interior)
    };
    // To the left of the line is the negative of to its right.
    let neg = |v: i64| v.checked_neg().ok_or_else(too_big);
    let (face, back) = (neg(on)?, neg(other)?);
    let low = if back > face {
        face
    } else {
        face.checked_sub(own).ok_or_else(too_big)?
    };
    seated_in(wall, [along.steps(), low], 1, up, hosted_recipe, refuse)
}

/// Where a placed family sits on the side `role` of a layered wall `wall` at an angle, `along` its
/// line from its start to its insertion point and `up` from its base, and the recipe its builder
/// is handed: its origin on the wall's centre plane (to a whole step, as along an axis), its `+y`
/// out of that side and its `+x` a quarter turn clockwise from that, so it is turned, never
/// mirrored: against the line when it faces the line's right, along it when it faces its left.
/// Seated there ([`seated_in`]) in place of a turn.
fn family_at_angle(
    wall: &Layered,
    role: &str,
    along: Length,
    up: Length,
    recipe: &Recipe,
    refuse: &impl Fn(&str, String) -> GeometryError,
) -> Result<(Placement, Recipe), GeometryError> {
    let too_big = || refuse("TooBig", "it is too far from its host".to_owned());
    let up = whole_offset("up", up, refuse)?;
    whole_offset("along", along, refuse)?;
    let (exterior, interior) = wall.outer();
    let centre = exterior
        .checked_add(interior)
        .map(|twice| twice.div_euclid(2))
        .ok_or_else(too_big)?;
    // 1 when the family faces the line's right, -1 its left.
    let right_of_exterior: i64 = if exterior > interior { 1 } else { -1 };
    let right = if role == "interior" {
        right_of_exterior.checked_neg().ok_or_else(too_big)?
    } else {
        right_of_exterior
    };
    let mut unturned = recipe.clone();
    unturned.args.remove(families::TURN_ARG);
    let left = centre.checked_neg().ok_or_else(too_big)?;
    let sign = right.checked_neg().ok_or_else(too_big)?;
    seated_in(wall, [along.steps(), left], sign, up, &unturned, refuse)
}

/// Where an element sits on the side `role` (`exterior` or `interior`) of the layered wall `wall`,
/// when that wall runs at an angle, `along` its line from its start and `up` from its base, and
/// the recipe its builder is handed: `recipe` with its seat ([`SEAT_ARG`]; a seat it already holds
/// is never read). `key` is the element's ID, for the words of a refusal. The placement is in the
/// wall's frame, whole millimetres: [`crate::hosting::seat`] and the browser's package
/// ([`seat_json`]) both work it out here, so the page seats what it builds as the server does.
///
/// # Errors
///
/// A [`GeometryError`] naming the element: `MissingArgument` (a size that is not whole
/// millimetres), `NotWholeMillimetres` (an offset finer than a placement holds), `TooBig`.
pub fn seat_at_angle(
    wall: &Layered,
    key: &str,
    recipe: &Recipe,
    role: &str,
    along: Length,
    up: Length,
) -> Result<(Placement, Recipe), GeometryError> {
    let mut stored = recipe.clone();
    stored.args.remove(SEAT_ARG);
    let what = noun(&stored);
    let refuse = |code: &str, why: String| GeometryError {
        code: code.to_owned(),
        message: format!("{what} {key}: {why}"),
    };
    if families::is_instance(&stored) {
        family_at_angle(wall, role, along, up, &stored, &refuse)
    } else {
        at_angle(wall, key, &stored, (role, along, up), &refuse)
    }
}

/// [`seat_at_angle`] for the browser's package: the filled recipe of the wall, the element's
/// recipe and ID, the side's role, and the two offsets as stored text (`length:q0:...`), all as
/// text. The answer is `{"seat": {"placement": {x, y, z}, "recipe": ...}}` (the placement in the
/// wall's frame, whole millimetres), or `{"failed": {code, message}}` as a build failure is
/// answered; a wall along an axis is `NotAtAnAngle`, since it seats as it always has.
#[must_use]
pub fn seat_json(wall: &str, recipe: &str, key: &str, role: &str, along: &str, up: &str) -> String {
    let failed = |code: &str, message: String| {
        serde_json::json!({ "failed": { "code": code, "message": message } }).to_string()
    };
    let (Ok(wall), Ok(recipe)) = (
        serde_json::from_str::<Recipe>(wall),
        serde_json::from_str::<Recipe>(recipe),
    ) else {
        return failed("BadRecipe", "a recipe to seat is not a recipe".to_owned());
    };
    let length = |text: &str| match Value::from_stored(text) {
        Some(Value::Length(length)) => Some(length),
        _ => None,
    };
    let (Some(along), Some(up)) = (length(along), length(up)) else {
        return failed("BadOffset", "an offset must be a stored length".to_owned());
    };
    let wall = match Layered::of(&wall) {
        Ok(wall) if wall.plan.axis().is_none() => wall,
        Ok(_) => {
            return failed("NotAtAnAngle", "the wall runs along an axis".to_owned());
        }
        Err(e) => return failed(&e.code, e.message),
    };
    match seat_at_angle(&wall, key, &recipe, role, along, up) {
        Ok((at, seated)) => {
            serde_json::json!({ "seat": { "placement": at, "recipe": seated } }).to_string()
        }
        Err(e) => failed(&e.code, e.message),
    }
}
