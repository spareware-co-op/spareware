//! Hosting (CON-A4-F3-P1): how an element hosted on another sits in its record, and where that puts
//! it. A door is hosted on a face of a wall, named by the face's persistent name, never by its
//! place in a list (chapter 20.4), at an offset along that face and up it. The regenerator builds a
//! wall together with the doors hosted on it through this module, and the server's commands check
//! a door against its wall through it, so what is refused and what is built cannot differ.
//!
//! The record (FORMAT.md, "Hosted elements"):
//!
//! - `references` holds one reference of kind [`FACE_KIND`] in this document: the host element,
//!   the face's persistent name, `want` `carrier` (an opening cut through the face does not break
//!   what is hosted on it), and the face's role as the `signature`, kept only to suggest a repair.
//! - `props` holds [`ALONG_KEY`] and [`UP_KEY`]: its offset along the face and up it, each a
//!   construct-params stored length (`length:q0:<steps>`).
//! - `placement` is where it sat when a command last wrote it. Where it sits now is read from its
//!   host ([`at`], [`in_model`]): a door moves with its wall, and its record is not rewritten.
//!
//! Where an offset puts it, in the host's frame (the frame its shape is built in): along the
//! wall's axis from its start (`end0`), whichever side it is on; up from its base; across the
//! wall, flush with the side it is hosted on, so a door as thick as its wall fills it. A wall's
//! exterior is the side on its right walking it from start to end (construct-regen's builders).
//! The opening it cuts is the builder's to decide (the kernel's `opening`): its width and height
//! through the wall's whole thickness.
//!
//! An older record, written before hosting was stored this way, is a `door.single` with a face
//! reference and no offsets ([`Hosting::ByPosition`]). It finds its host by that reference and sits
//! where its own placement puts it: its place in its host is the difference of the two placements.
//! It cuts its host as a new one does, but stays where it was placed when its host moves; a
//! `hosted.move` writes its offsets, and from then on it moves with its host. Why: the decision
//! note `projects/spare-construct/notes/CON-A4-F3-P1-hosting.md`.
//!
//! Depends on construct-model (records), construct-params (lengths and their stored text),
//! construct-families (a [`Seat`]) and this crate's builders' face names ([`crate::face_names`]).
//! A layered wall (`wall.layered`, job 4.1) hosts as the box does: along its line from its start,
//! the way it runs, flush with the side its element is on, each to the nearest millimetre where a
//! layer face is not on one ([`crate::wall_plan`]). A plain opening through a wall
//! (construct-openings' `opening.wall`, job 4.2) is hosted as a door is, as thick as its wall. A
//! floor opening (`opening.floor`) is hosted in its floor's plan ([`Hosting::InPlan`]): a face
//! reference to the floor's top and no offsets, its outline in the floor's own frame, so it sits
//! where its floor does. Interim: only a wall hosts on a face, only on its two sides, and a box
//! door sits flush with its side.
//!
//! A placed family (`family.instance`, [`crate::families`], job 6.4) is hosted the same way, on a
//! layered wall, and is seated in it by [`seat`]: its insertion point (the middle of its opening)
//! `along` the wall's line from its start, on the wall's centre plane (the frames of the library's
//! doors and windows are centred on the family's `y = 0`), its origin `up` from the wall's base;
//! and turned about its z axis so that its `+y` points out of the face it is hosted on and the
//! turn is a turn, never a mirror. On a wall along x or y the turn is by quarter turns
//! ([`families::TURN_ARG`], written into the recipe a builder is handed, derived and never
//! stored), its place to the nearest millimetre, as a placement holds it; the opening its wall
//! cuts is cut where the family then is, so the two agree exactly.
//!
//! **A wall at any angle** (piece A83; why: the decision note
//! `projects/spare-construct/notes/CON-A4-F3-P1-any-angle.md`). Everything hosted on a layered wall
//! whose line runs along neither axis is placed in the wall's own frame: `a` along its line from
//! its start, `s` square to it (to its left), up from its base, each a whole number of grid steps
//! worked out exactly (a door's box from the side it is flush with, a plain opening through the
//! wall's thickness, a family's origin on the centre plane). A frame point lands in plan at
//! `start + (a * d + s * d') / |d|`, `d` the line's direction and `d'` it turned a quarter to the
//! left, each coordinate rounded once to the nearest grid step (construct-families' [`Seat`]): the
//! same one rounding the wall's own layer faces take ([`crate::wall_plan`]). [`seat`] hands the
//! builder each such element with its seat ([`SEAT_ARG`], derived and never stored): the wall's
//! start from the element's placement, the way its own x runs (the wall's direction, or its
//! opposite for a family facing the line's right) and its origin in that frame; its placement is
//! the nearest whole millimetre to its origin, a reference for its record and its bounds only. Its
//! own shape and the opening its wall cuts for it are both worked out from that one frame, every
//! corner the nearest grid step to its true place (within half a step, 78.125 nm, on each axis),
//! so the door stands in its hole as the grid allows, never a millimetre apart. A wall along x or
//! y hosts as it always has, byte for byte; no seat is written for it.

use crate::families;
pub use crate::seating::{SEAT_ARG, noun, seat_of, unseated, with_seat};
use crate::seating;
use crate::wall_plan::{self, Axis, Layered};
use construct_families::Turn;
use construct_model::element::{Element, Placement, Reference, Want};
use construct_model::geometry::{Edition, FaceName, GeometryError, Recipe};
use construct_params::{Length, Value};
use kit_core::Id;

/// The `props` key of a hosted element's offset along its host's face.
pub const ALONG_KEY: &str = "@host.along";
/// The `props` key of a hosted element's offset up its host's face.
pub const UP_KEY: &str = "@host.up";
/// The kind of reference a hosted element names its host's face by.
pub const FACE_KIND: &str = "face";
/// The old box wall's operation, which hosts.
pub const HOST_OP: &str = "wall.straight";
/// Every operation that hosts: the old box wall and the layered wall.
pub const HOST_OPS: [&str; 2] = [HOST_OP, wall_plan::OP];
/// The faces of a host an element may sit on, by role.
pub const SIDES: [&str; 2] = ["exterior", "interior"];
/// The operation of an older record that is hosted by position.
const OPENING_OP: &str = "door.single";
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";

/// How an element is hosted, as its record says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hosting {
    /// On a face of its host at an offset along it and up it: it moves with its host.
    OnFace {
        /// The host element.
        host: Id,
        /// The host's face, by its persistent name.
        face: String,
        /// How far along the face from the host's start.
        along: Length,
        /// How far up the face from the host's base.
        up: Length,
    },
    /// An older record: a door with a face reference and no offsets, placed by its position.
    ByPosition {
        /// The host element.
        host: Id,
        /// The host's face, by its persistent name.
        face: String,
    },
    /// An opening through a floor (construct-openings' `opening.floor`): a reference to its
    /// floor's top face and no offsets. Its outline is drawn in its floor's own frame, so it sits
    /// where its floor does and moves with it.
    InPlan {
        /// The floor.
        host: Id,
        /// The floor's top face, by its persistent name.
        face: String,
    },
}

impl Hosting {
    /// The host element.
    #[must_use]
    pub fn host(&self) -> &Id {
        match self {
            Hosting::OnFace { host, .. }
            | Hosting::ByPosition { host, .. }
            | Hosting::InPlan { host, .. } => host,
        }
    }

    /// The host's face, by its persistent name.
    #[must_use]
    pub fn face(&self) -> &str {
        match self {
            Hosting::OnFace { face, .. }
            | Hosting::ByPosition { face, .. }
            | Hosting::InPlan { face, .. } => face,
        }
    }
}

/// How `element` is hosted, if it is.
///
/// # Errors
///
/// Why its record does not read as hosted, in words: one offset without the other, an offset that
/// is not a stored length, or offsets with no face, or more than one, to sit on.
pub fn read(element: &Element) -> Result<Option<Hosting>, String> {
    let faces: Vec<&Reference> = element
        .references
        .iter()
        .filter(|r| r.kind == FACE_KIND && r.doc.is_none())
        .collect();
    let name = &element.name;
    let (along, up) = (element.props.get(ALONG_KEY), element.props.get(UP_KEY));
    if along.is_none() && up.is_none() {
        let own = element.recipe.owner == OWN;
        let opening = own && element.recipe.op == OPENING_OP;
        let in_plan = own && element.recipe.op == construct_openings::FLOOR_OP;
        return Ok(match faces.first() {
            Some(face) if opening => Some(Hosting::ByPosition {
                host: face.element.clone(),
                face: face.name.clone(),
            }),
            Some(face) if in_plan => Some(Hosting::InPlan {
                host: face.element.clone(),
                face: face.name.clone(),
            }),
            _ => None,
        });
    }
    let (Some(along), Some(up)) = (along, up) else {
        return Err(format!("{name}: its offset on its host is half written"));
    };
    let [face] = faces.as_slice() else {
        return Err(format!(
            "{name}: a hosted element names one face of its host, not {}",
            faces.len()
        ));
    };
    let length = |key: &str, text: &str| match Value::from_stored(text) {
        Some(Value::Length(length)) => Ok(length),
        _ => Err(format!("{name}: {key} is {text}, not a stored length")),
    };
    Ok(Some(Hosting::OnFace {
        host: face.element.clone(),
        face: face.name.clone(),
        along: length(ALONG_KEY, along)?,
        up: length(UP_KEY, up)?,
    }))
}

/// Whether an element of `recipe` hosts: a wall, old box or layered ([`HOST_OPS`]).
#[must_use]
pub fn is_host(recipe: &Recipe) -> bool {
    recipe.owner == OWN && HOST_OPS.contains(&recipe.op.as_str())
}

/// The element `element` is hosted on, if its record reads as hosted.
#[must_use]
pub fn host_of(element: &Element) -> Option<Id> {
    read(element)
        .ok()
        .flatten()
        .map(|hosting| hosting.host().clone())
}

/// Writes `element` as hosted on `face` of `host` at `along` and `up`: its one face reference and
/// its two offsets. Its other references and `props` stay.
pub fn write(element: &mut Element, host: &Id, face: &FaceName, along: Length, up: Length) {
    element
        .references
        .retain(|r| !(r.kind == FACE_KIND && r.doc.is_none()));
    element.references.push(Reference {
        doc: None,
        element: host.clone(),
        name: face.name.clone(),
        kind: FACE_KIND.to_owned(),
        want: Want::Carrier,
        signature: Some(face.role.clone()),
    });
    element
        .props
        .insert(ALONG_KEY.to_owned(), Value::Length(along).stored());
    element
        .props
        .insert(UP_KEY.to_owned(), Value::Length(up).stored());
}

/// Writes `element`, a floor opening, as in the plan of the floor `host`, on its top face `face`:
/// its one face reference, and no offsets. Its other references and `props` stay.
pub fn write_in_plan(element: &mut Element, host: &Id, face: &FaceName) {
    element
        .references
        .retain(|r| !(r.kind == FACE_KIND && r.doc.is_none()));
    element.props.remove(ALONG_KEY);
    element.props.remove(UP_KEY);
    element.references.push(Reference {
        doc: None,
        element: host.clone(),
        name: face.name.clone(),
        kind: FACE_KIND.to_owned(),
        want: Want::Carrier,
        signature: Some(face.role.clone()),
    });
}

/// The face of `host` named `face`, under `edition`, if it is a side an element may sit on.
///
/// # Errors
///
/// Why not, in words: the host's faces cannot be named, or that face is not one of its sides.
pub fn side(edition: &Edition, host: &Recipe, face: &str) -> Result<FaceName, String> {
    let names = crate::face_names(edition, host).map_err(|e| e.message)?;
    names
        .into_iter()
        .find(|n| n.name == face && SIDES.contains(&n.role.as_str()))
        .ok_or_else(|| {
            format!(
                "the face {face} is not one of its sides; an element sits on a wall's {} or its {}",
                SIDES[0], SIDES[1]
            )
        })
}

/// The side of `host` whose role is `role` (`exterior` or `interior`), under `edition`.
///
/// # Errors
///
/// Why not, in words: the host's faces cannot be named, or the role is not one of its sides.
pub fn side_by_role(edition: &Edition, host: &Recipe, role: &str) -> Result<FaceName, String> {
    let names = crate::face_names(edition, host).map_err(|e| e.message)?;
    names
        .into_iter()
        .find(|n| n.role == role && SIDES.contains(&n.role.as_str()))
        .ok_or_else(|| {
            format!(
                "{role} is not one of its sides; an element sits on a wall's {} or its {}",
                SIDES[0], SIDES[1]
            )
        })
}

/// Where `hosted` sits in `host`'s frame, in whole millimetres: what a builder is handed with the
/// host. `host_recipe` and `hosted_recipe` are their recipes as built, each parameter filled, and
/// `edition` is the document's, which names the host's faces.
///
/// # Errors
///
/// A [`GeometryError`] naming the hosted element: `NotAHost` (only a `wall.straight` hosts on a
/// face), `NotASide` (its face is not one of its host's sides), `MissingArgument` (a thickness
/// that is not whole millimetres), `NotWholeMillimetres` (an offset finer than a placement holds),
/// `TooBig` (beyond a 64-bit number of millimetres).
pub fn at(
    edition: &Edition,
    host: &Element,
    host_recipe: &Recipe,
    hosted: &Element,
    hosted_recipe: &Recipe,
    hosting: &Hosting,
) -> Result<Placement, GeometryError> {
    seat(edition, host, host_recipe, hosted, hosted_recipe, hosting).map(|(at, _)| at)
}

/// Where `hosted` sits in `host`'s frame ([`at`]), and its recipe as its host's builder is handed
/// it: a placed family's with its turn ([`families::TURN_ARG`]), anything else's as it is.
///
/// On a layered wall at an angle the recipe carries its seat ([`SEAT_ARG`], this module's header),
/// a placed family's in place of a turn; a seat or a turn a stored record holds is never handed
/// on.
///
/// # Errors
///
/// As [`at`]; for a placed family also `NotAHost` on the old box wall (a family is hosted on a
/// layered wall).
#[allow(
    clippy::too_many_lines,
    reason = "every way an element is seated on its host, in the order its record is read"
)]
pub fn seat(
    edition: &Edition,
    host: &Element,
    host_recipe: &Recipe,
    hosted: &Element,
    hosted_recipe: &Recipe,
    hosting: &Hosting,
) -> Result<(Placement, Recipe), GeometryError> {
    let key = hosted.id.as_str();
    let what = noun(hosted_recipe);
    // What is derived is never read from a record: its seat is the host's to give.
    let mut stored = hosted_recipe.clone();
    stored.args.remove(SEAT_ARG);
    let hosted_recipe = &stored;
    let refuse = |code: &str, why: String| GeometryError {
        code: code.to_owned(),
        message: format!("{what} {key}: {why}"),
    };
    let too_big = || refuse("TooBig", "it is too far from its host".to_owned());
    let (along, up, face) = match hosting {
        Hosting::ByPosition { .. } => {
            let less = |a: i64, b: i64| a.checked_sub(b).ok_or_else(too_big);
            let (own, host) = (hosted.placement, host.placement);
            let at = Placement {
                x: less(own.x, host.x)?,
                y: less(own.y, host.y)?,
                z: less(own.z, host.z)?,
            };
            return Ok((at, hosted_recipe.clone()));
        }
        Hosting::InPlan { .. } => {
            if !crate::floor_plan::is_floor(host_recipe) {
                return Err(refuse(
                    "NotAHost",
                    format!(
                        "{} is a {}, not a floor: an opening drawn in plan is hosted on a floor",
                        host.name, host_recipe.op
                    ),
                ));
            }
            return Ok((Placement { x: 0, y: 0, z: 0 }, hosted_recipe.clone()));
        }
        Hosting::OnFace {
            along, up, face, ..
        } => (*along, *up, face),
    };
    if !is_host(host_recipe) {
        return Err(refuse(
            "NotAHost",
            format!(
                "{} is a {}, not a wall: only a {} or a {} hosts",
                host.name, host_recipe.op, HOST_OPS[0], HOST_OPS[1]
            ),
        ));
    }
    let role = side(edition, host_recipe, face).map_err(|why| refuse("NotASide", why))?;
    if families::is_instance(hosted_recipe) {
        if !wall_plan::is_layered(host_recipe) {
            return Err(refuse(
                "NotAHost",
                format!(
                    "{} is a {}: a family is hosted only on a {}",
                    host.name,
                    host_recipe.op,
                    wall_plan::OP
                ),
            ));
        }
        let wall = Layered::of(host_recipe).map_err(|e| refuse(&e.code, e.message))?;
        let Some(axis) = wall.plan.axis() else {
            return seating::seat_at_angle(&wall, key, hosted_recipe, &role.role, along, up);
        };
        let (at, turn) = family_on_layered(&wall, axis, &role, along, up, &refuse)?;
        return Ok((at, families::turned(hosted_recipe, turn)));
    }
    if wall_plan::is_layered(host_recipe) {
        let wall = Layered::of(host_recipe).map_err(|e| refuse(&e.code, e.message))?;
        let Some(axis) = wall.plan.axis() else {
            return seating::seat_at_angle(&wall, key, hosted_recipe, &role.role, along, up);
        };
        let at = on_layered(
            &wall,
            axis,
            hosted,
            hosted_recipe,
            &role,
            along,
            up,
            &refuse,
        )?;
        return Ok((at, hosted_recipe.clone()));
    }
    let whole = |recipe: &Recipe, arg: &str| {
        recipe
            .args
            .get(arg)
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| {
                refuse(
                    "MissingArgument",
                    format!("{arg} must be a whole number of millimetres"),
                )
            })
    };
    let thickness = whole(host_recipe, "thickness")?;
    // A plain opening goes through the whole wall, so it is as thick as its wall.
    let own = if construct_openings::is_wall_opening(hosted_recipe) {
        thickness
    } else {
        whole(hosted_recipe, "thickness")?
    };
    let mm = |what: &str, length: Length| {
        length.whole_mm().ok_or_else(|| {
            refuse(
                "NotWholeMillimetres",
                format!(
                    "its offset {what} its host's face, {length}, is not a whole number of \
                     millimetres, as every placement is"
                ),
            )
        })
    };
    let (along, up) = (mm("along", along)?, mm("up", up)?);
    let flush = thickness.checked_sub(own).ok_or_else(too_big)?;
    let along_y = host_recipe
        .args
        .get("axis")
        .and_then(serde_json::Value::as_str)
        == Some("y");
    // Along x the exterior is the side at y = 0; along y, the side at x = the wall's thickness.
    let across = match (role.role.as_str(), along_y) {
        ("exterior", false) | ("interior", true) => 0,
        _ => flush,
    };
    let at = if along_y {
        Placement {
            x: across,
            y: along,
            z: up,
        }
    } else {
        Placement {
            x: along,
            y: across,
            z: up,
        }
    };
    Ok((at, hosted_recipe.clone()))
}

/// Where a placed family sits on the side `role` of a layered wall `wall` running along `axis`,
/// `along` its line from its start to the family's insertion point and `up` from its base, in
/// whole millimetres in the wall's frame, and how far it is turned: its origin on the wall's
/// centre plane, its `+y` out of that side, its `+x` a quarter turn clockwise from that (so the
/// family is turned, never mirrored). Each coordinate is the nearest millimetre, as a placement
/// holds it.
fn family_on_layered(
    wall: &Layered,
    axis: Axis,
    role: &FaceName,
    along: Length,
    up: Length,
    refuse: &impl Fn(&str, String) -> GeometryError,
) -> Result<(Placement, Turn), GeometryError> {
    let whole = |what: &str, length: Length| {
        length.whole_mm().ok_or_else(|| {
            refuse(
                "NotWholeMillimetres",
                format!(
                    "its offset {what} its host's face, {length}, is not a whole number of \
                     millimetres, as every placement is"
                ),
            )
        })
    };
    let up = whole("up", up)?;
    whole("along", along)?;
    let too_big = || refuse("TooBig", "it is too far from its host".to_owned());
    let (exterior, interior) = wall.outer();
    // The centre plane, to a whole step (half a step at most from it, far under a millimetre).
    let centre = exterior
        .checked_add(interior)
        .map(|twice| twice.div_euclid(2))
        .ok_or_else(too_big)?;
    let [ux, uy] = axis.unit();
    let right = [uy, ux.checked_neg().ok_or_else(too_big)?];
    // The exterior is on the line's right as drawn, on its left when the wall is flipped.
    let outward_of_exterior: i64 = if exterior > interior { 1 } else { -1 };
    let outward = if role.role == "interior" {
        outward_of_exterior.checked_neg().ok_or_else(too_big)?
    } else {
        outward_of_exterior
    };
    let [nx, ny] = right.map(|c| c.saturating_mul(outward));
    let turn =
        Turn::taking_x_to([ny, nx.checked_neg().ok_or_else(too_big)?]).ok_or_else(too_big)?;
    let start = wall.plan.start();
    let mut origin = [0_i64; 2];
    for (slot, ((s, u), r)) in origin.iter_mut().zip(start.iter().zip([ux, uy]).zip(right)) {
        let point = along
            .steps()
            .checked_mul(u)
            .and_then(|a| centre.checked_mul(r).and_then(|c| a.checked_add(c)))
            .and_then(|d| d.checked_add(*s))
            .ok_or_else(too_big)?;
        *slot = wall_plan::nearest_mm(point).0;
    }
    let [x, y] = origin;
    Ok((Placement { x, y, z: up }, turn))
}

/// Where an element sits on the side `role` of a layered wall (its filled recipe `wall`), `along`
/// its line from its start and `up` from its base: its box's lowest corner in the wall's frame,
/// in whole millimetres. Along the line it starts `along` from the wall's start the way the wall
/// runs (so on a wall running toward -x its box ends there); across, it is flush with that side
/// and reaches toward the other; each to the nearest millimetre, as a placement holds it, since a
/// layer face need not lie on one (the opening it cuts reaches 100 mm past both sides, so this
/// moves no face of the opening across the wall). A wall at an angle hosts through [`seating::seat_at_angle`].
#[allow(
    clippy::too_many_arguments,
    reason = "the host and the hosted element with its filled recipe, and where it sits"
)]
fn on_layered(
    wall: &Layered,
    axis: Axis,
    hosted: &Element,
    hosted_recipe: &Recipe,
    role: &FaceName,
    along: Length,
    up: Length,
    refuse: &impl Fn(&str, String) -> GeometryError,
) -> Result<Placement, GeometryError> {
    let too_big = || refuse("TooBig", "it is too far from its host".to_owned());
    let (width, own) = seating::size_in(wall, hosted.id.as_str(), hosted_recipe, refuse)?;
    let whole = |what: &str, length: Length| {
        length.whole_mm().ok_or_else(|| {
            refuse(
                "NotWholeMillimetres",
                format!(
                    "its offset {what} its host's face, {length}, is not a whole number of \
                     millimetres, as every placement is"
                ),
            )
        })
    };
    let up = whole("up", up)?;
    whole("along", along)?;
    let (exterior, interior) = wall.outer();
    let (on, other) = if role.role == "interior" {
        (interior, exterior)
    } else {
        (exterior, interior)
    };
    let (a, c) = (axis.index(), usize::from(axis.index() == 0));
    let pick = |v: [i64; 2], i: usize| v.get(i).copied().unwrap_or_default();
    let start = wall.plan.start();
    let [ux, uy] = axis.unit();
    // The right of the line, across it: (uy, -ux); its component on the across axis.
    let right = pick([uy, ux.checked_neg().unwrap_or_default()], c);
    let add = |p: i64, q: i64| p.checked_add(q).ok_or_else(too_big);
    let times = |p: i64, q: i64| p.checked_mul(q).ok_or_else(too_big);
    let low_along = if pick(axis.unit(), a) > 0 {
        add(pick(start, a), along.steps())?
    } else {
        add(pick(start, a), times(add(along.steps(), width)?, -1)?)?
    };
    let face = add(pick(start, c), times(on, right)?)?;
    let back = add(pick(start, c), times(other, right)?)?;
    let low_across = if back > face {
        face
    } else {
        face.checked_sub(own).ok_or_else(too_big)?
    };
    let mut plan = [0_i64; 2];
    for (axis, value) in [(a, low_along), (c, low_across)] {
        if let Some(slot) = plan.get_mut(axis) {
            *slot = wall_plan::nearest_mm(value).0;
        }
    }
    let [x, y] = plan;
    Ok(Placement { x, y, z: up })
}

/// Where an element at `at` in the frame of a host placed at `host` sits in the model.
///
/// # Errors
///
/// `TooBig` when that is beyond a 64-bit number of millimetres.
pub fn in_model(host: Placement, at: Placement) -> Result<Placement, GeometryError> {
    let add = |a: i64, b: i64| {
        a.checked_add(b).ok_or_else(|| GeometryError {
            code: "TooBig".to_owned(),
            message: "a hosted element is beyond the model's reach".to_owned(),
        })
    };
    Ok(Placement {
        x: add(host.x, at.x)?,
        y: add(host.y, at.y)?,
        z: add(host.z, at.z)?,
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::arithmetic_side_effects,
        reason = "test code"
    )]

    use super::*;
    use crate::stand_in::Boxes;
    use construct_model::geometry::Geometry;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn i(text: &str) -> Id {
        Id::parse(text).unwrap()
    }

    fn element(id: &str, op: &str, args: serde_json::Value, at: [i64; 3]) -> Element {
        Element {
            id: i(id),
            document: i("doc"),
            kind: "door".to_owned(),
            name: id.to_owned(),
            level: 1,
            placement: Placement {
                x: at[0],
                y: at[1],
                z: at[2],
            },
            recipe: Recipe::construct(op, serde_json::from_value(args).unwrap()),
            props: BTreeMap::new(),
            references: Vec::new(),
        }
    }

    fn wall(axis: &str) -> Element {
        let args = json!({ "length": 4000, "thickness": 200, "height": 2700, "axis": axis });
        element("W-1", "wall.straight", args, [1000, 2000, 0])
    }

    fn door() -> Element {
        let args = json!({ "width": 900, "thickness": 100, "height": 2100 });
        element("D-1", "door.single", args, [0, 0, 0])
    }

    fn face(role: &str) -> FaceName {
        crate::stand_in::names(&wall("x").recipe)
            .unwrap()
            .into_iter()
            .find(|n| n.role == role)
            .unwrap()
    }

    /// A hosted element's record holds its host's face and its offsets, and reads back as written;
    /// writing it again replaces its face reference and keeps everything else it holds.
    #[test]
    fn a_hosted_record_reads_back_as_written() {
        let mut d = door();
        d.props.insert("note".to_owned(), "kept".to_owned());
        let along = Length::from_steps(80_000);
        write(
            &mut d,
            &i("W-1"),
            &face("exterior"),
            along,
            Length::from_mm(0),
        );
        assert_eq!(d.props[ALONG_KEY], "length:q0:80000");
        assert_eq!(d.props[UP_KEY], "length:q0:0");
        assert_eq!(d.references.len(), 1);
        assert_eq!(d.references[0].name, "box.exterior");
        assert_eq!(d.references[0].kind, "face");
        assert_eq!(d.references[0].want, Want::Carrier);
        assert_eq!(d.references[0].signature.as_deref(), Some("exterior"));
        let read_back = read(&d).unwrap().unwrap();
        assert_eq!(
            read_back,
            Hosting::OnFace {
                host: i("W-1"),
                face: "box.exterior".to_owned(),
                along,
                up: Length::from_mm(0),
            }
        );
        write(&mut d, &i("W-2"), &face("interior"), along, along);
        assert_eq!(d.references.len(), 1);
        assert_eq!(host_of(&d), Some(i("W-2")));
        assert_eq!(d.props["note"], "kept");
    }

    /// An older door, a face reference and no offsets, finds its host by that reference and sits by
    /// its position; anything else with no offsets is not hosted; a record half written does not read.
    #[test]
    fn an_older_door_is_hosted_by_its_reference_and_its_position() {
        let mut d = door();
        assert_eq!(read(&d).unwrap(), None);
        d.references.push(Reference {
            doc: None,
            element: i("W-1"),
            name: "box.exterior".to_owned(),
            kind: "face".to_owned(),
            want: Want::Carrier,
            signature: None,
        });
        assert_eq!(
            read(&d).unwrap(),
            Some(Hosting::ByPosition {
                host: i("W-1"),
                face: "box.exterior".to_owned(),
            })
        );
        let mut block = d.clone();
        block.recipe.op = "block".to_owned();
        assert_eq!(read(&block).unwrap(), None);
        let mut half = d.clone();
        half.props
            .insert(ALONG_KEY.to_owned(), "length:q0:0".to_owned());
        assert!(read(&half).unwrap_err().contains("half written"));
        half.props.insert(UP_KEY.to_owned(), "900".to_owned());
        assert!(read(&half).unwrap_err().contains("not a stored length"));
        half.references.clear();
        half.props
            .insert(UP_KEY.to_owned(), "length:q0:0".to_owned());
        assert!(read(&half).unwrap_err().contains("not 0"));
        assert_eq!(host_of(&half), None);
    }

    /// An offset puts a door along its wall from the wall's start and up from its base, flush with
    /// the side it is on: along x the exterior is at y = 0, along y at x = the wall's thickness.
    /// An older door's place is the difference of the two placements.
    #[test]
    fn an_offset_puts_a_door_flush_with_its_side() {
        let edition = Boxes.edition();
        let (along, up) = (Length::from_mm(1200), Length::from_mm(300));
        for (axis, role, expected) in [
            ("x", "exterior", [1200, 0, 300]),
            ("x", "interior", [1200, 100, 300]),
            ("y", "exterior", [100, 1200, 300]),
            ("y", "interior", [0, 1200, 300]),
        ] {
            let host = wall(axis);
            let mut d = door();
            write(&mut d, &host.id, &face(role), along, up);
            let hosting = read(&d).unwrap().unwrap();
            let got = at(&edition, &host, &host.recipe, &d, &d.recipe, &hosting).unwrap();
            assert_eq!([got.x, got.y, got.z], expected, "{axis} {role}");
            let placed = in_model(host.placement, got).unwrap();
            assert_eq!([placed.x - got.x, placed.y - got.y], [1000, 2000]);
        }
        let host = wall("x");
        let mut old = door();
        old.placement = Placement {
            x: 2500,
            y: 1950,
            z: 0,
        };
        let hosting = Hosting::ByPosition {
            host: host.id.clone(),
            face: "box.exterior".to_owned(),
        };
        let got = at(&edition, &host, &host.recipe, &old, &old.recipe, &hosting).unwrap();
        assert_eq!([got.x, got.y, got.z], [1500, -50, 0]);
    }

    fn q0(steps: i64) -> String {
        Value::Length(Length::from_steps(steps)).stored()
    }

    /// The default partition (12.5, 90, 12.5 mm), its line from `start` to `end` in whole
    /// millimetres, its exterior face `exterior` steps right of the line and its others `side`
    /// times 80,000, 656,000 and 736,000 steps less (a flipped wall's `side` is -1).
    fn layered(start: [i64; 2], end: [i64; 2], exterior: i64, side: i64) -> Element {
        let mm = |v: i64| q0(v * 6400);
        let face = [0, 80_000, 656_000, 736_000].map(|d| exterior - side * d);
        let layer = |f: &str, a: i64, b: i64| json!({ "function": f, "material": "m", "exterior": q0(a), "interior": q0(b) });
        let args = json!({
            "start": [mm(start[0]), mm(start[1])],
            "end": [mm(end[0]), mm(end[1])],
            "height": mm(2700),
            "layers": [
                layer("finish", face[0], face[1]),
                layer("structure", face[1], face[2]),
                layer("finish", face[2], face[3]),
            ],
        });
        element("W-1", "wall.layered", args, [1000, 2000, 0])
    }

    /// On a layered wall a door sits along its line from its start the way the wall runs, flush
    /// with its side to the nearest millimetre (57.5 mm rounds away from zero), up from its base,
    /// on all four axes and on a flipped wall (its exterior on the line's left).
    #[test]
    fn a_door_on_a_layered_wall_sits_flush_with_its_side() {
        let edition = Boxes.edition();
        let (along, up) = (Length::from_mm(1200), Length::from_mm(300));
        // The door is 900 wide and 100 thick.
        for (start, end, (exterior, side), role, expected) in [
            (
                [0, 0],
                [4000, 0],
                (368_000, 1),
                "exterior",
                [1200, -58, 300],
            ),
            (
                [0, 0],
                [4000, 0],
                (368_000, 1),
                "interior",
                [1200, -43, 300],
            ),
            (
                [4000, 0],
                [0, 0],
                (368_000, 1),
                "exterior",
                [1900, -43, 300],
            ),
            (
                [0, 0],
                [0, 4000],
                (368_000, 1),
                "exterior",
                [-43, 1200, 300],
            ),
            (
                [0, 0],
                [0, 4000],
                (368_000, 1),
                "interior",
                [-58, 1200, 300],
            ),
            (
                [0, 4000],
                [0, 0],
                (368_000, 1),
                "interior",
                [-43, 1900, 300],
            ),
            (
                [0, 0],
                [4000, 0],
                (-368_000, -1),
                "exterior",
                [1200, -43, 300],
            ),
            ([0, 0], [4000, 0], (0, 1), "exterior", [1200, 0, 300]),
        ] {
            let host = layered(start, end, exterior, side);
            let mut d = door();
            let face = crate::stand_in::names(&host.recipe)
                .unwrap()
                .into_iter()
                .find(|n| n.role == role)
                .unwrap();
            write(&mut d, &host.id, &face, along, up);
            let hosting = read(&d).unwrap().unwrap();
            let got = at(&edition, &host, &host.recipe, &d, &d.recipe, &hosting).unwrap();
            assert_eq!(
                [got.x, got.y, got.z],
                expected,
                "{start:?} {end:?} {exterior} {side} {role}"
            );
        }
        let mut d = door();
        let host = layered([0, 0], [4000, 0], 368_000, 1);
        write(
            &mut d,
            &host.id,
            &face("exterior"),
            Length::from_steps(80_000),
            up,
        );
        let hosting = read(&d).unwrap().unwrap();
        let fine = at(&edition, &host, &host.recipe, &d, &d.recipe, &hosting).unwrap_err();
        assert_eq!(fine.code, "NotWholeMillimetres");
        assert!(is_host(&host.recipe) && is_host(&wall("x").recipe));
        assert!(!is_host(&door().recipe));
    }

    /// On a wall at an angle a door is seated in the wall's frame (piece A83): on a 3:4:5 wall its
    /// origin, 1200 mm along and on the exterior face 57.5 mm right of the line, lands exactly at
    /// (766, 925.5) mm, so it is placed at the nearest millimetre, (766, 926), and its seat holds
    /// the rest; the recipe it is handed is its own with that seat, and nothing else.
    #[test]
    fn a_door_on_a_wall_at_an_angle_is_seated_in_its_frame() {
        let edition = Boxes.edition();
        let (along, up) = (Length::from_mm(1200), Length::from_mm(300));
        let angled = layered([0, 0], [3000, 4000], 368_000, 1);
        let mut d = door();
        write(&mut d, &angled.id, &face("exterior"), along, up);
        let hosting = read(&d).unwrap().unwrap();
        let (got, seated) =
            seat(&edition, &angled, &angled.recipe, &d, &d.recipe, &hosting).unwrap();
        assert_eq!([got.x, got.y, got.z], [766, 926, 300]);
        assert_eq!(
            seated.args[SEAT_ARG],
            json!({
                "way": [19_200_000, 25_600_000],
                "base": [-4_902_400, -5_926_400],
                "at": [7_680_000, -368_000],
            })
        );
        let (stored, held) = unseated(&seated).unwrap();
        assert_eq!(stored, d.recipe);
        assert_eq!(held.unwrap().plan([0, 0]), Some([0, -3200]));
    }

    /// The browser's package seats what it is handed as the server does ([`seating::seat_json`]):
    /// the same placement in the wall's frame and the same seated recipe, for a door and for a
    /// placed family, and a wall along an axis is no business of it.
    #[test]
    fn the_package_seats_as_the_server_does() {
        let edition = Boxes.edition();
        let (along, up) = (Length::from_mm(1200), Length::from_mm(300));
        let angled = layered([0, 0], [3000, 4000], 368_000, 1);
        let mut d = door();
        write(&mut d, &angled.id, &face("interior"), along, up);
        let hosting = read(&d).unwrap().unwrap();
        let (got, seated) =
            seat(&edition, &angled, &angled.recipe, &d, &d.recipe, &hosting).unwrap();
        let text = |recipe: &Recipe| serde_json::to_string(recipe).unwrap();
        let answer: serde_json::Value = serde_json::from_str(&seating::seat_json(
            &text(&angled.recipe),
            &text(&d.recipe),
            d.id.as_str(),
            "interior",
            &Value::Length(along).stored(),
            &Value::Length(up).stored(),
        ))
        .unwrap();
        assert_eq!(answer["seat"]["placement"], json!(got));
        assert_eq!(answer["seat"]["recipe"], json!(seated));
        let along_x = layered([0, 0], [4000, 0], 368_000, 1);
        let answer = seating::seat_json(
            &text(&along_x.recipe),
            &text(&d.recipe),
            "D-1",
            "exterior",
            &Value::Length(along).stored(),
            &Value::Length(up).stored(),
        );
        assert!(answer.contains("NotAtAnAngle"), "{answer}");
    }

    /// Each case where an offset puts nothing anywhere is a failure naming the door and why.
    #[test]
    fn an_offset_that_puts_nothing_says_why() {
        let edition = Boxes.edition();
        let host = wall("x");
        let mut d = door();
        write(
            &mut d,
            &host.id,
            &face("exterior"),
            Length::from_steps(80_000),
            Length::from_mm(0),
        );
        let hosting = read(&d).unwrap().unwrap();
        let code = |host: &Element, hosting: &Hosting| {
            at(&edition, host, &host.recipe, &d, &d.recipe, hosting).unwrap_err()
        };
        let fine = code(&host, &hosting);
        assert_eq!(fine.code, "NotWholeMillimetres");
        assert!(fine.message.starts_with("door D-1: "), "{}", fine.message);
        let mut whole = d.clone();
        write(
            &mut whole,
            &host.id,
            &face("top"),
            Length::from_mm(1),
            Length::from_mm(0),
        );
        let top = read(&whole).unwrap().unwrap();
        assert_eq!(
            at(&edition, &host, &host.recipe, &whole, &whole.recipe, &top)
                .unwrap_err()
                .code,
            "NotASide"
        );
        let mut block = host.clone();
        block.recipe.op = "block".to_owned();
        assert_eq!(code(&block, &hosting).code, "NotAHost");
        let other = Edition {
            engine: "elsewhere".to_owned(),
            ..edition.clone()
        };
        let named = at(
            &other,
            &host,
            &host.recipe,
            &whole,
            &whole.recipe,
            &read(&whole).unwrap().unwrap(),
        );
        assert_eq!(named.unwrap_err().code, "NotASide");
        let far = Placement {
            x: i64::MAX,
            y: 0,
            z: 0,
        };
        assert_eq!(in_model(far, far).unwrap_err().code, "TooBig");
    }
}
