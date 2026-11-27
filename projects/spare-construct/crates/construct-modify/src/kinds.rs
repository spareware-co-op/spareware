//! What each kind of element becomes under an exact motion ([`crate::exact::Motion`]), read and
//! written by the crate that owns its stored form, so a moved record is exactly the record of one
//! drawn there:
//!
//! | Kind | What moves | Read and written by |
//! | --- | --- | --- |
//! | Layered wall (`wall.layered`) | Its line, `start` and `end`; mirrored, its `wall.flipped` turns over so its exterior is where the mirror puts it | its stored lengths (construct-params); construct-walls' parameter |
//! | Floor | Its boundary's corners, each edge keeping its ID | construct-slabs |
//! | Roof | Its footprint's corners, each edge keeping its ID and what it does | construct-roofs |
//! | Room | Its point | construct-rooms |
//! | Shaft, floor opening | Its outline's corners | construct-openings |
//! | Grid line, reference plane | Its geometry; an arc's angles by quarter turns and 45-degree mirrors only | construct-grids |
//! | Block | Its box, by quarter turns and 45-degree mirrors, in whole millimetres | its placement and recipe |
//! | Older box wall or door (`wall.straight`, a `door.single` on no host) | Its placement, moved only, in whole millimetres | its placement |
//!
//! An element hosted on a face of a wall (a door, a placed family, a wall opening) is not moved
//! here: where it sits is read from its host, so it follows its host with its record unchanged,
//! except what the host's turn or mirror changes in it ([`follower`]): a box door or a wall
//! opening names the axis its wall runs along, so a quarter turn that takes x to y changes it; a
//! placed family's hand (an instance choice of left or right, [`hands_of`]) changes in a mirror,
//! because a family is seated turned and never mirrored, so the mirror image of a left-hinged door
//! is the right-hinged one. A floor opening's outline is in its floor's plan, so it is moved with
//! its floor by the same motion.
//!
//! A record's points are in its own frame, the plan less its placement (every kind here but the
//! boxes keeps its placement at the origin): each is moved as the plan point it is and put back in
//! the same frame.

use crate::error::ModifyError;
use crate::exact::{Miss, Motion, Steps};
use construct_families::{Key, ParameterValue, Recipe as FamilyRecipe};
use construct_grids::{Grid, PlaneShape, Point as GridPoint, ReferencePlane, Shape};
use construct_model::element::{Element, Placement};
use construct_params::{Holder, Kind as ParamKind, Length, Value};
use construct_regen::families::{self, Instance};
use construct_regen::hosting::{self, Hosting};
use construct_regen::parameters::has_shape;
use construct_regen::wall_plan;
use construct_slabs::{Boundary, Corner, Ring};
use construct_solver::Point;
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};

/// What the modify tools need to know of a document beyond an element's own record, answered by
/// whoever holds the document: the server's command from its parameters and its families, a test
/// from its fixtures.
pub trait Sense {
    /// Whether the layered wall `wall` is flipped now (its exterior on the left of its line): its
    /// own value of `wall.flipped`, or the default it reads.
    ///
    /// # Errors
    /// Why it cannot be read, in words.
    fn flipped(&self, wall: &Element) -> Result<bool, String>;

    /// The hands of the placed family `family` ([`hands_of`]).
    ///
    /// # Errors
    /// Why its family or its values cannot be read, in words.
    fn hands(&self, family: &Element) -> Result<Vec<Hand>, String>;
}

/// A hand of a placed family: an instance parameter choosing left or right, and the value it takes
/// in the family's mirror image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hand {
    /// The parameter's key in the family recipe.
    pub key: Key,
    /// The value its mirror image holds, in stored text (`choice:right` for a left hand).
    pub mirrored: String,
}

/// The two choices a hand is made of.
const HANDS: [&str; 2] = ["left", "right"];

/// The hands of a placed family of `recipe` holding `instance`'s values: each parameter each
/// instance holds that is a choice of exactly `left` and `right`, with the other of the two from
/// what it holds now (its own value, else its default). A hand worked out by a formula is the
/// formula's and is left out.
///
/// # Errors
/// A hand that holds neither `left` nor `right`, in words.
pub fn hands_of(recipe: &FamilyRecipe, instance: &Instance) -> Result<Vec<Hand>, String> {
    let mut hands = Vec::new();
    for parameter in &recipe.parameters {
        let definition = &parameter.definition;
        let keys: BTreeSet<&str> = definition
            .choices
            .iter()
            .map(|choice| choice.key.as_str())
            .collect();
        if definition.kind != ParamKind::Choice
            || definition.holder != Holder::Instance
            || keys != BTreeSet::from(HANDS)
        {
            continue;
        }
        let now = match (instance.values.get(&parameter.key), &parameter.value) {
            (Some(own), _) => own.clone(),
            (None, ParameterValue::Default(default)) => default.clone(),
            (None, ParameterValue::Formula(_)) => continue,
        };
        let other = match Value::from_stored(&now) {
            Some(Value::Choice(hand)) if hand == HANDS[0] => HANDS[1],
            Some(Value::Choice(hand)) if hand == HANDS[1] => HANDS[0],
            _ => {
                return Err(format!(
                    "its {} holds {now}, which is neither left nor right",
                    parameter.key.as_str()
                ));
            }
        };
        hands.push(Hand {
            key: parameter.key.clone(),
            mirrored: Value::Choice(other.to_owned()).stored(),
        });
    }
    Ok(hands)
}

/// What an element is, to the modify tools.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum What {
    /// A layered wall.
    Layered,
    /// A floor.
    Floor,
    /// A roof.
    Roof,
    /// A room.
    Room,
    /// A shaft, or an opening drawn in a floor's plan: an outline.
    Outline,
    /// A grid line.
    Grid,
    /// A reference plane.
    Plane,
    /// A block.
    Block,
    /// An older box: a `wall.straight`, or a `door.single` on no host.
    Older,
    /// An element on a face of its host, at an offset along it and up it.
    OnFace(Hosting),
    /// A floor opening, hosted in the plan of its floor.
    InPlan(Hosting),
}

/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";

/// What `record` is, or why the modify tools do not move it.
///
/// # Errors
/// [`ModifyError::NotMoved`] for a record of no shape or a kind not listed; the hosting reader's
/// refusal of a record that does not read as hosted; [`ModifyError::ByPosition`] for an older door
/// hosted by its position.
pub(crate) fn what(record: &Element) -> Result<What, ModifyError> {
    let not_moved = || ModifyError::NotMoved {
        element: record.name.clone(),
        kind: record.kind.clone(),
    };
    if !has_shape(record) {
        return Err(not_moved());
    }
    let hosted = hosting::read(record).map_err(|why| ModifyError::Refused {
        element: record.name.clone(),
        why,
    })?;
    match hosted {
        Some(Hosting::ByPosition { .. }) => {
            return Err(ModifyError::ByPosition {
                element: record.name.clone(),
            });
        }
        Some(on @ Hosting::OnFace { .. }) => return Ok(What::OnFace(on)),
        Some(in_plan @ Hosting::InPlan { .. }) => return Ok(What::InPlan(in_plan)),
        None => {}
    }
    let recipe = &record.recipe;
    Ok(if wall_plan::is_layered(recipe) {
        What::Layered
    } else if construct_slabs::is_floor(record) {
        What::Floor
    } else if construct_roofs::is_roof(record) {
        What::Roof
    } else if construct_rooms::is_room(record) {
        What::Room
    } else if construct_openings::is_shaft(recipe) || construct_openings::is_floor_opening(recipe)
    {
        What::Outline
    } else if record.kind == construct_grids::GRID_KIND {
        What::Grid
    } else if record.kind == construct_grids::PLANE_KIND {
        What::Plane
    } else if recipe.owner == OWN && recipe.op == "block" {
        What::Block
    } else if recipe.owner == OWN && ["wall.straight", "door.single"].contains(&recipe.op.as_str())
    {
        What::Older
    } else {
        return Err(not_moved());
    })
}

/// The record's frame: its placement, in grid steps.
fn origin(record: &Element) -> Result<Steps, ModifyError> {
    let steps = |mm: i64| {
        Length::checked_from_mm(mm)
            .map(Length::steps)
            .ok_or(ModifyError::TooLarge)
    };
    Ok([steps(record.placement.x)?, steps(record.placement.y)?])
}

/// Where `motion` takes the point `p` of a record framed at `origin`, in the same frame.
fn moved_point(
    motion: &Motion,
    origin: Steps,
    p: Steps,
    name: &str,
) -> Result<Steps, ModifyError> {
    let [ox, oy] = origin;
    let [x, y] = p;
    let plan = [
        x.checked_add(ox).ok_or(ModifyError::TooLarge)?,
        y.checked_add(oy).ok_or(ModifyError::TooLarge)?,
    ];
    let [mx, my] = motion.apply(plan).map_err(|miss| match miss {
        Miss::OffGrid => ModifyError::OffGrid {
            element: name.to_owned(),
        },
        Miss::TooLarge => ModifyError::TooLarge,
    })?;
    Ok([
        mx.checked_sub(ox).ok_or(ModifyError::TooLarge)?,
        my.checked_sub(oy).ok_or(ModifyError::TooLarge)?,
    ])
}

/// The refusal of the crate that owns a record's form, in its words.
fn refused(record: &Element) -> impl Fn(&dyn std::fmt::Display) -> ModifyError + '_ {
    |why| ModifyError::Refused {
        element: record.name.clone(),
        why: why.to_string(),
    }
}

/// The record `record` becomes under `motion`: moved as itself, standing in the plan. A hosted
/// element is not moved here ([`follower`] when its host moves; the selection's own rule when it
/// moves alone).
///
/// # Errors
/// [`ModifyError::OffGrid`] for a point that would leave the grid; the kind's own refusal of
/// what it would become; [`ModifyError::BoxTurns`], [`ModifyError::OlderBox`],
/// [`ModifyError::NotWholeMm`] and [`ModifyError::ArcTurns`] as the table in the module header
/// says; [`ModifyError::NotMoved`] for a hosted element or one of a kind not listed.
pub fn moved(record: &Element, motion: &Motion, sense: &dyn Sense) -> Result<Element, ModifyError> {
    match what(record)? {
        What::Layered => layered(record, motion, sense),
        What::Floor => floor(record, motion),
        What::Roof => roof(record, motion),
        What::Room => room(record, motion),
        What::Outline | What::InPlan(_) => outline(record, motion),
        What::Grid => grid(record, motion),
        What::Plane => plane(record, motion),
        What::Block => block(record, motion),
        What::Older => older(record, motion),
        What::OnFace(_) => Err(ModifyError::NotMoved {
            element: record.name.clone(),
            kind: record.kind.clone(),
        }),
    }
}

/// A point as a layered wall's recipe holds it: `[x, y]`, two stored lengths.
fn stored_point(value: Option<&Json>) -> Option<Steps> {
    let [x, y] = value?.as_array()?.as_slice() else {
        return None;
    };
    let length = |v: &Json| match Value::from_stored(v.as_str()?) {
        Some(Value::Length(length)) => Some(length.steps()),
        _ => None,
    };
    Some([length(x)?, length(y)?])
}

/// A point written as a layered wall's recipe holds it.
fn write_point([x, y]: Steps) -> Json {
    Json::from(vec![
        Value::Length(Length::from_steps(x)).stored(),
        Value::Length(Length::from_steps(y)).stored(),
    ])
}

fn layered(record: &Element, motion: &Motion, sense: &dyn Sense) -> Result<Element, ModifyError> {
    let name = &record.name;
    let frame = origin(record)?;
    let mut out = record.clone();
    for end in ["start", "end"] {
        let p = stored_point(record.recipe.args.get(end)).ok_or_else(|| ModifyError::Refused {
            element: name.clone(),
            why: format!("its {end} is not two stored lengths"),
        })?;
        let moved = moved_point(motion, frame, p, name)?;
        out.recipe.args.insert(end.to_owned(), write_point(moved));
    }
    if motion.is_mirror() {
        let flipped = sense.flipped(record).map_err(|why| ModifyError::Refused {
            element: name.clone(),
            why,
        })?;
        out.props.insert(
            construct_walls::FLIPPED.to_owned(),
            Value::YesNo(!flipped).stored(),
        );
    }
    Ok(out)
}

/// The corners of a loop of `boundary` moved by `motion`, each with the ID of its edge.
fn ring(
    record: &Element,
    boundary: &Boundary,
    which: Ring,
    motion: &Motion,
) -> Result<Vec<Corner>, ModifyError> {
    let frame = origin(record)?;
    let corners = boundary
        .corners(which)
        .ok_or_else(|| ModifyError::Refused {
            element: record.name.clone(),
            why: format!("{which} has an edge that is not a straight line"),
        })?;
    corners
        .into_iter()
        .map(|(id, at)| {
            let [x, y] = moved_point(motion, frame, [at.x, at.y], &record.name)?;
            Ok(Corner {
                id: Some(id),
                at: Point { x, y },
            })
        })
        .collect()
}

/// `boundary` with every corner moved by `motion`, checked again by construct-slabs.
fn moved_boundary(
    record: &Element,
    boundary: &Boundary,
    motion: &Motion,
) -> Result<Boundary, ModifyError> {
    let outer = ring(record, boundary, Ring::Outer, motion)?;
    let holes = (0..boundary.holes().len())
        .map(|at| ring(record, boundary, Ring::Hole(at), motion))
        .collect::<Result<Vec<_>, _>>()?;
    Boundary::from_corners(&outer, &holes).map_err(|e| refused(record)(&e))
}

fn floor(record: &Element, motion: &Motion) -> Result<Element, ModifyError> {
    let boundary = construct_slabs::boundary_of(&record.recipe).map_err(|e| refused(record)(&e))?;
    let moved = moved_boundary(record, &boundary, motion)?;
    construct_slabs::with_boundary(record.clone(), &moved).map_err(|e| refused(record)(&e))
}

fn roof(record: &Element, motion: &Motion) -> Result<Element, ModifyError> {
    let roof = construct_roofs::roof_of(&record.recipe).map_err(|e| refused(record)(&e))?;
    let footprint = moved_boundary(record, roof.footprint(), motion)?;
    let edges: BTreeMap<_, _> = roof.edges().iter().cloned().collect();
    let moved =
        construct_roofs::Roof::new(footprint, &edges).map_err(|e| refused(record)(&e))?;
    construct_roofs::with_roof(record.clone(), &moved).map_err(|e| refused(record)(&e))
}

fn room(record: &Element, motion: &Motion) -> Result<Element, ModifyError> {
    let at = construct_rooms::at_of(&record.recipe).map_err(|e| refused(record)(&e))?;
    let limit = construct_rooms::limit_of(record).map_err(|e| refused(record)(&e))?;
    let [x, y] = moved_point(motion, origin(record)?, at, &record.name)?;
    let mut out = record.clone();
    out.recipe =
        construct_rooms::room_recipe([Length::from_steps(x), Length::from_steps(y)], &limit)
            .map_err(|e| refused(record)(&e))?;
    Ok(out)
}

fn outline(record: &Element, motion: &Motion) -> Result<Element, ModifyError> {
    let key = record.id.as_str();
    let outline =
        construct_openings::outline_of(key, &record.recipe).map_err(|e| refused(record)(&e))?;
    let frame = origin(record)?;
    let corners = outline
        .corners()
        .iter()
        .map(|[x, y]| {
            let [mx, my] = moved_point(motion, frame, [x.steps(), y.steps()], &record.name)?;
            Ok([Length::from_steps(mx), Length::from_steps(my)])
        })
        .collect::<Result<Vec<_>, ModifyError>>()?;
    let named = construct_openings::named(key, &record.recipe);
    let moved = construct_openings::Outline::new(&named, corners).map_err(|e| refused(record)(&e))?;
    let mut out = record.clone();
    out.recipe
        .args
        .insert(construct_openings::OUTLINE_ARG.to_owned(), moved.to_value());
    Ok(out)
}

/// A datum's point moved: a datum's frame is the plan.
fn grid_point(record: &Element, motion: &Motion, p: GridPoint) -> Result<GridPoint, ModifyError> {
    let [x, y] = moved_point(motion, [0, 0], [p.x.steps(), p.y.steps()], &record.name)?;
    Ok(GridPoint::new(Length::from_steps(x), Length::from_steps(y)))
}

fn grid(record: &Element, motion: &Motion) -> Result<Element, ModifyError> {
    let grid = Grid::from_record(record).map_err(|e| refused(record)(&e))?;
    let shape = match *grid.shape() {
        Shape::Line { from, to } => Shape::line(
            grid_point(record, motion, from)?,
            grid_point(record, motion, to)?,
        ),
        Shape::Arc {
            centre,
            radius,
            from,
            to,
        } => {
            let square = motion.square().ok_or_else(|| ModifyError::ArcTurns {
                element: record.name.clone(),
            })?;
            // A mirror runs the arc the other way round, so its ends change places.
            let (from, to) = if square.mirrored {
                (square.angle(to), square.angle(from))
            } else {
                (square.angle(from), square.angle(to))
            };
            Shape::arc(grid_point(record, motion, centre)?, radius, from, to)
        }
    }
    .map_err(|e| refused(record)(&e))?;
    let moved = grid.reshaped(shape).map_err(|e| refused(record)(&e))?;
    Ok(moved.update_record(record))
}

fn plane(record: &Element, motion: &Motion) -> Result<Element, ModifyError> {
    let plane = ReferencePlane::from_record(record).map_err(|e| refused(record)(&e))?;
    let shape = match *plane.shape() {
        PlaneShape::Vertical { from, to } => PlaneShape::Vertical {
            from: grid_point(record, motion, from)?,
            to: grid_point(record, motion, to)?,
        },
        // A horizontal plane has no place in plan: a motion in plan leaves it as it is.
        horizontal @ PlaneShape::Horizontal { .. } => horizontal,
    };
    let moved = plane.reshaped(shape).map_err(|e| refused(record)(&e))?;
    Ok(moved.update_record(record))
}

/// Grid steps in a millimetre.
const STEPS_PER_MM: i64 = Length::STEPS_PER_MM;

/// `steps` as whole millimetres, or the refusal naming `record`.
fn whole_mm(record: &Element, steps: i64) -> Result<i64, ModifyError> {
    Length::from_steps(steps)
        .whole_mm()
        .ok_or_else(|| ModifyError::NotWholeMm {
            element: record.name.clone(),
        })
}

/// A whole number of millimetres a box's recipe holds.
fn size_arg(record: &Element, arg: &str) -> Result<i64, ModifyError> {
    record
        .recipe
        .args
        .get(arg)
        .and_then(Json::as_i64)
        .ok_or_else(|| ModifyError::Refused {
            element: record.name.clone(),
            why: format!("its {arg} is not a whole number of millimetres"),
        })
}

fn block(record: &Element, motion: &Motion) -> Result<Element, ModifyError> {
    if motion.square().is_none() {
        return Err(ModifyError::BoxTurns {
            element: record.name.clone(),
        });
    }
    let (width, depth) = (size_arg(record, "width")?, size_arg(record, "depth")?);
    let mm = |v: i64| v.checked_mul(STEPS_PER_MM).ok_or(ModifyError::TooLarge);
    let corner = [mm(record.placement.x)?, mm(record.placement.y)?];
    let far = [
        mm(record.placement.x.checked_add(width).ok_or(ModifyError::TooLarge)?)?,
        mm(record.placement.y.checked_add(depth).ok_or(ModifyError::TooLarge)?)?,
    ];
    let [ax, ay] = moved_point(motion, [0, 0], corner, &record.name)?;
    let [bx, by] = moved_point(motion, [0, 0], far, &record.name)?;
    let size = |a: i64, b: i64| {
        a.checked_sub(b)
            .map(i64::abs)
            .ok_or(ModifyError::TooLarge)
    };
    let mut out = record.clone();
    out.placement = Placement {
        x: whole_mm(record, ax.min(bx))?,
        y: whole_mm(record, ay.min(by))?,
        z: record.placement.z,
    };
    out.recipe.args.insert(
        "width".to_owned(),
        Json::from(whole_mm(record, size(ax, bx)?)?),
    );
    out.recipe.args.insert(
        "depth".to_owned(),
        Json::from(whole_mm(record, size(ay, by)?)?),
    );
    Ok(out)
}

fn older(record: &Element, motion: &Motion) -> Result<Element, ModifyError> {
    let [dx, dy] = motion.translation().ok_or_else(|| ModifyError::OlderBox {
        element: record.name.clone(),
    })?;
    let (dx, dy) = (whole_mm(record, dx)?, whole_mm(record, dy)?);
    let mut out = record.clone();
    out.placement.x = record
        .placement
        .x
        .checked_add(dx)
        .ok_or(ModifyError::TooLarge)?;
    out.placement.y = record
        .placement
        .y
        .checked_add(dy)
        .ok_or(ModifyError::TooLarge)?;
    Ok(out)
}

/// The argument of a box door's recipe naming the axis it runs along.
const AXIS: &str = "axis";

/// What an element hosted on a face of a wall becomes when its host moves by `motion`: `None`
/// when nothing in its record changes, which is most moves (where it sits is read from its host).
/// A box door or a wall opening names the axis its wall runs along, so a motion that takes x to y
/// changes it; a placed family changes hand in a mirror ([`hands_of`]).
///
/// # Errors
/// A wall opening whose recipe does not read, and a family whose hands cannot be read, in words.
pub fn follower(
    record: &Element,
    motion: &Motion,
    sense: &dyn Sense,
) -> Result<Option<Element>, ModifyError> {
    let mut out = record.clone();
    let mut changed = false;
    let recipe = &record.recipe;
    if motion.square().is_some_and(crate::exact::Square::swaps_axes) {
        if construct_openings::is_wall_opening(recipe) {
            let key = record.id.as_str();
            let opening = construct_openings::WallOpening::of(key, recipe)
                .map_err(|e| refused(record)(&e))?;
            let named = construct_openings::named(key, recipe);
            let turned = construct_openings::WallOpening::new(
                &named,
                opening.width,
                opening.height,
                !opening.along_y,
            )
            .map_err(|e| refused(record)(&e))?;
            out.recipe = turned.recipe();
            changed = true;
        } else if recipe.owner == OWN && recipe.op == "door.single" {
            if recipe.args.get(AXIS).and_then(Json::as_str) == Some("y") {
                out.recipe.args.remove(AXIS);
            } else {
                out.recipe
                    .args
                    .insert(AXIS.to_owned(), Json::from("y"));
            }
            changed = true;
        }
    }
    if motion.is_mirror() && families::is_instance(recipe) {
        let hands = sense.hands(record).map_err(|why| ModifyError::Refused {
            element: record.name.clone(),
            why,
        })?;
        for hand in hands {
            out.recipe = families::with_value(&out.recipe, &hand.key, &hand.mirrored);
            changed = true;
        }
    }
    Ok(changed.then_some(out))
}

/// The direction a host runs from its start, as a unit step along the plan's x or y axis: a
/// layered wall's from its line, an older box wall's from its axis. `None` for a layered wall at an
/// angle.
///
/// # Errors
/// A host whose line does not read, in words.
pub(crate) fn host_direction(host: &Element) -> Result<Option<Steps>, ModifyError> {
    let recipe = &host.recipe;
    if wall_plan::is_layered(recipe) {
        let ends = (
            stored_point(recipe.args.get("start")),
            stored_point(recipe.args.get("end")),
        );
        let (Some(start), Some(end)) = ends else {
            return Err(ModifyError::Refused {
                element: host.name.clone(),
                why: "its line is not two stored points".to_owned(),
            });
        };
        let plan = wall_plan::Plan::new(start, end).map_err(|e| ModifyError::Refused {
            element: host.name.clone(),
            why: e.message,
        })?;
        return Ok(plan.axis().map(wall_plan::Axis::unit));
    }
    Ok(Some(
        if recipe.args.get(AXIS).and_then(Json::as_str) == Some("y") {
            [0, 1]
        } else {
            [1, 0]
        },
    ))
}

/// `record`, hosted on a face of `host` at `along`, moved alone by `by`: the same along its
/// host by the part of `by` along it.
///
/// # Errors
/// [`ModifyError::OffItsHost`] for a move with any part across its host;
/// [`ModifyError::HostAtAngle`] for a host at an angle; [`ModifyError::TooLarge`].
pub(crate) fn along_host(
    record: &Element,
    host: &Element,
    along: Length,
    by: Steps,
) -> Result<Element, ModifyError> {
    let Some([ux, uy]) = host_direction(host)? else {
        return Err(ModifyError::HostAtAngle {
            element: record.name.clone(),
            host: host.name.clone(),
        });
    };
    let [dx, dy] = by;
    let across = dx
        .checked_mul(uy)
        .and_then(|a| dy.checked_mul(ux).and_then(|b| a.checked_sub(b)))
        .ok_or(ModifyError::TooLarge)?;
    if across != 0 {
        return Err(ModifyError::OffItsHost {
            element: record.name.clone(),
            host: host.name.clone(),
        });
    }
    let step = dx
        .checked_mul(ux)
        .and_then(|a| dy.checked_mul(uy).and_then(|b| a.checked_add(b)))
        .ok_or(ModifyError::TooLarge)?;
    let moved = along
        .checked_add(Length::from_steps(step))
        .ok_or(ModifyError::TooLarge)?;
    let mut out = record.clone();
    out.props.insert(
        hosting::ALONG_KEY.to_owned(),
        Value::Length(moved).stored(),
    );
    Ok(out)
}
