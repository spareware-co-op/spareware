//! A layered wall on the engine (job 4.1, CON-A4-F2-P1): one solid per layer of a filled
//! `wall.layered` recipe, each the extrusion of its strip in plan ([`crate::wall_plan`]) from the
//! wall's base straight up to its top, less the opening of every door hosted in it.
//!
//! What it builds:
//!
//! - A solid for each layer with a thickness: the parallelogram between the layer's two faces,
//!   corners on the grid by the plan's one rounding rule, extruded by the wall's height through
//!   [`Engine::extrude`]. A layer of no thickness (a membrane) is a face between its neighbours and
//!   has no solid. Each solid knows its layer's material and function: the body's bytes list them,
//!   and each display lists the triangles of each layer as a part ([`Part`]), so a page colours
//!   them by material.
//! - Openings: a door's opening is cut through every layer, by the same rules as the old box's
//!   ([`super::opening`]): the same refusals, in the same words, asked in the wall's own frame
//!   (along its line from its start, across it from its exterior face, up from its base). Each
//!   cutter is extruded once and subtracted from each layer, one door after another in the order
//!   of their keys.
//! - A wall at an angle (piece A83): every door, plain opening and family in it comes seated
//!   (construct-regen's `hosting`, [`crate::hosting::SEAT_ARG`]), its place read from its seat in
//!   the wall's frame exactly, never from its placement, which is only the nearest millimetre to
//!   it; the rules are asked in that frame ([`super::opening::Long`] for its length), and each
//!   cutter's corners are put through the wall's frame as the element's own are, each the nearest
//!   grid step to its true place, so an element and its opening are made from one frame. An
//!   element in it that is not seated (an older door hosted by its position), or seated for
//!   another wall, is refused (`NotSeated`, `SeatMismatch`).
//! - A placed family's opening (job 6.4, [`crate::families`]): the outline its recipe declares,
//!   solved at its type's values, placed where the family stands (its origin and its turn, as the
//!   regenerator seats it: construct-regen's `hosting::seat`) and cut through every layer on the
//!   same path as a door's, by the same rules ([`super::opening::plan_in`]). Where the family's
//!   recipe comes from is the caller's [`FamilySource`].
//! - Wrapping: not built. A layer that wraps at an opening (construct-walls' `Layer::wraps`) is
//!   cut flush with the opening as every other layer is; turning it round the opening's sides is
//!   job 6.1 (CON-A4-F2-P4), and its place is [`build`]'s loop over the cutters.
//! - Joins and clean-ups where walls meet (jobs 5.1 and 6.1): not here; a wall's ends are square.
//! - Names (chapter 20.2): the wall's six roles, `base`, `top`, `exterior`, `interior`, `end0` and
//!   `end1`, are named as the old box's are, under this recipe's operation, so a door hosted on
//!   the exterior keeps its face through every rebuild. `exterior` is the first layer's outer
//!   face, `interior` the last layer's; each layer's piece of the base, the top and the ends
//!   carries the wall's name for it, as the pieces of a face an opening splits do. The face
//!   between layer `k - 1` and layer `k` is `face.<k>`, for `k` from 1. An opening's faces are
//!   named as in the old box, once for all the layers' pieces.
//! - Displays: level 0 the solids with their openings, level 1 before the openings, both in whole
//!   millimetres, each coordinate the nearest millimetre to the engine's grid point (a layer face
//!   at 12.5 mm, or the corner of a wall at an angle, is not on one); a level that rounded anything
//!   strays by 1 mm more than it would. The bounds are rounded outward.
//!
//! Depends on the parent module's units, naming and errors, [`super::opening`] and
//! [`crate::wall_plan`]. Interim: as the parent module's (the engine behind the seam, the body's
//! bytes being the engine's request, names written from the recipe); and the layers' pieces of a
//! face share its one name where chapter 20.2 names each piece `Split` (CON-A3-F1-P2, with the
//! body record).

use super::opening::{self, Cut, Frame, Hosted, Long, OPENING_OP, PLAIN_OP, Placed, Span};
use super::{
    AskedCut, Extrusion, KernelSolid, LayerSolid, NamedFace, Units, WALL_ROLES, as_steps, carrier,
    fault, from_engine, name, op_id,
};
use crate::families::{self, FamilySource};
use crate::hosting;
use crate::wall_plan::{Axis, Layered};
use construct_families::Seat;
use construct_model::geometry::{
    Bounds, Built, Display, FaceName, GeometryError, Mesh, Part, Recipe,
};
use engine_geometry::value::engine::{DisplayMesh, Engine, EngineError, GridPlane, Profile};
use engine_geometry::value::grid::P3;
use engine_geometry::value::op::OpId;
use serde::Serialize;

/// A layered wall's body bytes: what the engine was asked for each layer, and the cutter of each
/// opening, once for all the layers. A wall with no openings writes no `subtract`.
#[derive(Serialize)]
struct Asked {
    layers: Vec<AskedLayer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    subtract: Vec<AskedCut>,
}

/// One layer as the engine was asked for it.
#[derive(Serialize)]
struct AskedLayer {
    function: String,
    material: String,
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
}

/// The low and high grid points a mesh reaches, in steps.
pub(super) type Extent = Option<([i64; 3], [i64; 3])>;

/// The persistent names of a layered wall's faces without building it, sorted by name: its six
/// roles and the faces between its layers. An opening's faces are not among them.
///
/// # Errors
///
/// `Naming` when a name cannot be made.
pub(super) fn names(recipe: &Recipe, wall: &Layered) -> Result<Vec<FaceName>, GeometryError> {
    let mut names = own_names(op_id(recipe)?, wall)?;
    names.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(names)
}

/// The names of the wall's own faces, in role order.
fn own_names(op: OpId, wall: &Layered) -> Result<Vec<FaceName>, GeometryError> {
    let mut roles: Vec<String> = WALL_ROLES.iter().map(|r| (*r).to_owned()).collect();
    for k in 1..wall.layers.len() {
        roles.push(format!("face.{k}"));
    }
    roles.iter().map(|role| name(op, role)).collect()
}

/// `a + b`, or `TooBig`.
fn add(a: i64, b: i64) -> Result<i64, GeometryError> {
    a.checked_add(b).ok_or_else(super::too_big)
}

/// `a * b`, or `TooBig`.
fn times(a: i64, b: i64) -> Result<i64, GeometryError> {
    a.checked_mul(b).ok_or_else(super::too_big)
}

/// The grid point of a plan point `[x, y]` in the engine's steps at height `z`.
fn point([x, y]: [i64; 2], z: i64) -> Result<P3, GeometryError> {
    P3::new(x, y, z).map_err(|_| super::too_big())
}

/// A wall, as its openings are asked in it: its frame (along its line from its start, across it
/// from its exterior face toward its interior, up from its base) and how a plan point, in the
/// engine's steps relative to the wall's placement, goes into that frame and back.
struct WallFrame {
    /// Its line's direction, a unit vector along x or y; none for a wall at an angle.
    unit: [i64; 2],
    /// For a wall at an angle, its line as a seat: `line.plan([along, left])` is the plan point
    /// `along` its line from its start and `left` to its left, each coordinate rounded once.
    line: Option<Seat>,
    /// Its line's start, in the engine's steps.
    start: [i64; 2],
    /// Its exterior face's offset to the right of its line, in the engine's steps.
    exterior: i64,
    /// 1 when the interior is to the left of the exterior (the exterior on the line's right, as
    /// drawn), -1 when the wall is flipped.
    inward: i64,
    /// The frame the openings are asked in.
    frame: Frame,
}

impl WallFrame {
    /// The frame of `wall`, which runs along `axis`.
    fn of(wall: &Layered, axis: Axis, units: Units) -> Result<WallFrame, GeometryError> {
        let length = units.from_q0(wall.plan.axis_length().unwrap_or(0))?;
        let (exterior, interior) = wall.outer();
        let thickness = i64::try_from(wall.thickness()).map_err(|_| super::too_big())?;
        let [sx, sy] = wall.plan.start();
        Ok(WallFrame {
            unit: axis.unit(),
            line: None,
            start: [units.from_q0(sx)?, units.from_q0(sy)?],
            exterior: units.from_q0(exterior)?,
            inward: if exterior > interior { 1 } else { -1 },
            frame: Frame {
                along: axis.index(),
                length: Long::Steps(length),
                thickness: units.from_q0(thickness)?,
                height: units.from_q0(wall.height)?,
                seated: false,
            },
        })
    }

    /// The frame of `wall`, which runs at an angle: along its line in the frame's first place and
    /// across it in the second, its length the root of its line's square length, its elements
    /// seated.
    fn at_angle(wall: &Layered, units: Units) -> Result<WallFrame, GeometryError> {
        let (exterior, interior) = wall.outer();
        let thickness = i64::try_from(wall.thickness()).map_err(|_| super::too_big())?;
        let [sx, sy] = wall.plan.start();
        let [dx, dy] = wall.plan.direction();
        let start = [units.from_q0(sx)?, units.from_q0(sy)?];
        let way = [units.from_q0(dx)?, units.from_q0(dy)?];
        let square =
            |v: i64| u128::from(v.unsigned_abs()).saturating_mul(u128::from(v.unsigned_abs()));
        Ok(WallFrame {
            unit: [0, 0],
            line: Some(Seat::new(way, start, [0, 0]).ok_or_else(super::too_big)?),
            start,
            exterior: units.from_q0(exterior)?,
            inward: if exterior > interior { 1 } else { -1 },
            frame: Frame {
                along: 0,
                length: Long::Root(square(way[0]).saturating_add(square(way[1]))),
                thickness: units.from_q0(thickness)?,
                height: units.from_q0(wall.height)?,
                seated: true,
            },
        })
    }

    /// The unit vector to the right of the line.
    fn right(&self) -> [i64; 2] {
        let [ux, uy] = self.unit;
        [uy, ux.checked_neg().unwrap_or(0)]
    }

    /// A plan point, relative to the wall's placement, in the frame: along and across.
    fn to_frame(&self, [x, y]: [i64; 2]) -> Result<[i64; 2], GeometryError> {
        let [sx, sy] = self.start;
        let (px, py) = (
            x.checked_sub(sx).ok_or_else(super::too_big)?,
            y.checked_sub(sy).ok_or_else(super::too_big)?,
        );
        let [ux, uy] = self.unit;
        let [rx, ry] = self.right();
        let along = add(times(px, ux)?, times(py, uy)?)?;
        let right = add(times(px, rx)?, times(py, ry)?)?;
        let across = times(
            self.exterior
                .checked_sub(right)
                .ok_or_else(super::too_big)?,
            self.inward,
        )?;
        Ok([along, across])
    }

    /// A frame point, along and across, in plan relative to the wall's placement; at an angle,
    /// each coordinate the nearest grid step to the true point.
    fn to_plan(&self, [along, across]: [i64; 2]) -> Result<[i64; 2], GeometryError> {
        let right = self
            .exterior
            .checked_sub(times(across, self.inward)?)
            .ok_or_else(super::too_big)?;
        if let Some(line) = &self.line {
            let left = right.checked_neg().ok_or_else(super::too_big)?;
            return line.plan([along, left]).ok_or_else(super::too_big);
        }
        let [sx, sy] = self.start;
        let [ux, uy] = self.unit;
        let [rx, ry] = self.right();
        Ok([
            add(sx, add(times(along, ux)?, times(right, rx)?)?)?,
            add(sy, add(times(along, uy)?, times(right, ry)?)?)?,
        ])
    }

    /// The plan axis across the wall: the one its line does not run along.
    fn across_axis(&self) -> usize {
        usize::from(self.frame.along == 0)
    }

    /// A door hosted in the wall, moved into the frame: its box's two plan corners, read from its
    /// placement and its size, taken along and across, and its lowest corner put where the
    /// openings read it (along in the frame's along axis's place, across in the other's). What is
    /// not a door, or a door that does not read, is left as it is, for the openings to refuse in
    /// their own words.
    fn door<'a>(&self, hosted: &Hosted<'a>, units: Units) -> Result<Hosted<'a>, GeometryError> {
        let named =
            |e: GeometryError| fault(&e.code, format!("door {}: {}", hosted.key, e.message));
        // Its plan's size along x and along y, in steps; a plain opening reaches through the
        // wall's whole thickness, so only its place along the wall is read.
        let ([wide, deep], plain) = if hosted.recipe.op == PLAIN_OP {
            let Ok(open) = construct_openings::WallOpening::of(hosted.key, hosted.recipe) else {
                return Ok(*hosted);
            };
            let width = units.from_q0(open.width.steps())?;
            let thickness = self.frame.thickness;
            let size = if open.along_y {
                [thickness, width]
            } else {
                [width, thickness]
            };
            (size, true)
        } else if hosted.recipe.op == OPENING_OP {
            let Ok(door) = Extrusion::door(&hosted.recipe.args) else {
                return Ok(*hosted);
            };
            let [wide, deep] = door.size;
            (
                [
                    units.steps(wide).map_err(named)?,
                    units.steps(deep).map_err(named)?,
                ],
                false,
            )
        } else {
            return Ok(*hosted);
        };
        let (x, y) = (hosted.at.x.get(), hosted.at.y.get());
        let (low, high) = ([x, y], [add(x, wide)?, add(y, deep)?]);
        let ([low_along, low_across], [high_along, high_across]) =
            (self.to_frame(low)?, self.to_frame(high)?);
        let mut frame = [0_i64; 2];
        if let Some(slot) = frame.get_mut(self.frame.along) {
            *slot = low_along.min(high_along);
        }
        if let Some(slot) = frame.get_mut(self.across_axis()) {
            *slot = if plain {
                0
            } else {
                low_across.min(high_across)
            };
        }
        let [first, second] = frame;
        Ok(Hosted {
            key: hosted.key,
            recipe: hosted.recipe,
            at: P3::new(first, second, hosted.at.z.get()).map_err(|_| super::too_big())?,
        })
    }

    /// A placed family hosted in the wall, its opening placed in the frame: the outline its
    /// recipe declares at its type's values ([`families::opening_of`]), along the wall from where
    /// its origin is, the way its turn takes its x, and up from its origin's height. Its origin
    /// must lie within the wall's thickness, and its x along the wall.
    fn family<'a>(
        &self,
        hosted: &Hosted<'a>,
        families: &dyn FamilySource,
        units: Units,
    ) -> Result<Placed<'a>, GeometryError> {
        let key = hosted.key;
        let named = |e: GeometryError| fault(&e.code, format!("family {key}: {}", e.message));
        let refused = |code: &str, why: &str| fault(code, format!("family {key}: {why}"));
        let (instance, span) = families::opening_of(families, hosted.recipe).map_err(named)?;
        let [wx, wy] = instance.turn.plan([1, 0]).unwrap_or_default();
        let [ux, uy] = self.unit;
        // +1 when its x runs the way the wall does, -1 when against it, 0 when across it.
        let way = add(times(wx, ux)?, times(wy, uy)?)?;
        if way == 0 {
            return Err(refused(
                "OpeningAcrossWall",
                "it is turned across its wall's axis",
            ));
        }
        let [along, across] = self.to_frame([hosted.at.x.get(), hosted.at.y.get()])?;
        let thickness = self.frame.thickness;
        if across <= 0 || across >= thickness {
            return Err(refused(
                "OpeningNotInWall",
                "its origin is not within the wall's thickness",
            ));
        }
        let steps = |length: construct_params::Length| units.from_q0(length.steps()).map_err(named);
        let (x0, x1) = (
            add(along, times(way, steps(span.x[0])?)?)?,
            add(along, times(way, steps(span.x[1])?)?)?,
        );
        let z = hosted.at.z.get();
        Ok(Placed {
            key,
            noun: "family",
            op: op_id(hosted.recipe)?,
            span: Span {
                start: x0.min(x1),
                end: x0.max(x1),
                foot: add(z, steps(span.z[0])?)?,
                head: add(z, steps(span.z[1])?)?,
            },
        })
    }

    /// A cut's cutter in plan, as the engine is asked for it: its corners at its foot, and the
    /// vector up to its head.
    fn cutter(&self, cut: &Cut) -> Result<(Profile, [i64; 3]), GeometryError> {
        let (lo, hi) = cut.frame_box();
        let pick = |v: [i64; 3], i: usize| v.get(i).copied().unwrap_or_default();
        let (along, across) = (self.frame.along, self.across_axis());
        let (a0, a1) = (pick(lo, along), pick(hi, along));
        let (c0, c1) = (pick(lo, across), pick(hi, across));
        let (z0, z1) = (pick(lo, 2), pick(hi, 2));
        let mut outer = Vec::with_capacity(4);
        for corner in [[a0, c0], [a1, c0], [a1, c1], [a0, c1]] {
            outer.push(point(self.to_plan(corner)?, z0)?);
        }
        let up = z1.checked_sub(z0).ok_or_else(super::too_big)?;
        Ok((
            Profile {
                outer,
                holes: Vec::new(),
            },
            [0, 0, up],
        ))
    }
}

/// The cuts the doors and placed families `hosted` make in `wall`, with the frame they were asked
/// in; none when nothing is hosted. A family's recipe comes from `families`.
fn cuts(
    wall: &Layered,
    hosted: &[Hosted<'_>],
    units: Units,
    band: i64,
    families: &dyn FamilySource,
) -> Result<Option<(WallFrame, Vec<Cut>)>, GeometryError> {
    if hosted.is_empty() {
        return Ok(None);
    }
    let Some(axis) = wall.plan.axis() else {
        return seated_cuts(wall, hosted, units, band, families).map(Some);
    };
    let frame = WallFrame::of(wall, axis, units)?;
    let (placed, doors): (Vec<&Hosted<'_>>, Vec<&Hosted<'_>>) =
        hosted.iter().partition(|h| families::is_instance(h.recipe));
    let framed = doors
        .into_iter()
        .map(|h| frame.door(h, units))
        .collect::<Result<Vec<_>, _>>()?;
    let placed = placed
        .into_iter()
        .map(|h| frame.family(h, families, units))
        .collect::<Result<Vec<_>, _>>()?;
    let cuts = opening::plan_in(frame.frame, &framed, &placed, units, band)?;
    Ok(Some((frame, cuts)))
}

/// An element hosted in a wall at an angle, read from its seat: its recipe as stored, the frame
/// point its seat puts its origin at (along the wall's line from its start and to its left, in
/// the engine's steps) and which way its own x runs along the line (1 with it, -1 against it).
struct InFrame {
    recipe: construct_model::geometry::Recipe,
    origin: [i64; 2],
    sign: i64,
}

/// What `hosted`, in the wall at an angle `wall`, is seated as ([`InFrame`]).
///
/// # Errors
///
/// `NotSeated` for an element with no seat; `SeatMismatch` for one seated for another wall (its
/// way is not this wall's line or its opposite, or its base and placement are not this wall's
/// start); `BadSeat` for a seat that does not read.
fn in_frame(wall: &Layered, hosted: &Hosted<'_>, units: Units) -> Result<InFrame, GeometryError> {
    let who = if families::is_instance(hosted.recipe) {
        format!("family {}", hosted.key)
    } else {
        opening::who(hosted.recipe, hosted.key)
    };
    let named = |e: GeometryError| fault(&e.code, format!("{who}: {}", e.message));
    let (recipe, seat) = hosting::unseated(hosted.recipe).map_err(named)?;
    let seat = seat.ok_or_else(|| {
        fault(
            "NotSeated",
            format!(
                "{who}: it is not seated in its wall, which runs at an angle; an element is placed \
                 in such a wall by its offsets along it and up it (hosted.move gives them)"
            ),
        )
    })?;
    let mismatch = || {
        fault(
            "SeatMismatch",
            format!("{who}: it is seated for another wall than the one it is built in"),
        )
    };
    let d = wall.plan.direction();
    let back = [d[0].checked_neg(), d[1].checked_neg()];
    let sign = if seat.way() == d {
        1
    } else if [Some(seat.way()[0]), Some(seat.way()[1])] == back {
        -1
    } else {
        return Err(mismatch());
    };
    // Its base and its placement make the wall's start: it was seated in this wall where it is.
    let per = construct_params::Length::STEPS_PER_MM;
    let placed = [hosted.at.x.get(), hosted.at.y.get()].map(|steps| {
        units
            .millimetres(steps)
            .ok()
            .and_then(|mm| mm.checked_mul(per))
    });
    let start = wall.plan.start();
    for ((base, placed), start) in seat.base().into_iter().zip(placed).zip(start) {
        if placed.and_then(|p| base.checked_add(p)) != Some(start) {
            return Err(mismatch());
        }
    }
    let [a, s] = seat.at();
    let origin = [
        units.from_q0(a.checked_mul(sign).ok_or_else(super::too_big)?)?,
        units.from_q0(s.checked_mul(sign).ok_or_else(super::too_big)?)?,
    ];
    Ok(InFrame {
        recipe,
        origin,
        sign,
    })
}

/// The cuts the elements `hosted` make in `wall`, a wall at an angle, with its frame: each read
/// from its seat ([`in_frame`]) into the frame (along in the first place, across in the second,
/// up), its recipe as stored, and the openings' rules asked there.
fn seated_cuts(
    wall: &Layered,
    hosted: &[Hosted<'_>],
    units: Units,
    band: i64,
    families: &dyn FamilySource,
) -> Result<(WallFrame, Vec<Cut>), GeometryError> {
    let frame = WallFrame::at_angle(wall, units)?;
    let seated = hosted
        .iter()
        .map(|h| in_frame(wall, h, units))
        .collect::<Result<Vec<_>, _>>()?;
    // Across the wall from its exterior face toward its interior, of a point `left` of its line.
    let across = |left: i64| {
        frame
            .exterior
            .checked_add(left)
            .and_then(|c| c.checked_mul(frame.inward))
            .ok_or_else(super::too_big)
    };
    let mut framed = Vec::new();
    let mut placed = Vec::new();
    for (h, seat) in hosted.iter().zip(&seated) {
        let [along, left] = seat.origin;
        let z = h.at.z.get();
        if families::is_instance(h.recipe) {
            let key = h.key;
            let named = |e: GeometryError| fault(&e.code, format!("family {key}: {}", e.message));
            let (_, span) = families::opening_of(families, h.recipe).map_err(named)?;
            if across(left)? <= 0 || across(left)? >= frame.frame.thickness {
                return Err(fault(
                    "OpeningNotInWall",
                    format!("family {key}: its origin is not within the wall's thickness"),
                ));
            }
            let steps =
                |length: construct_params::Length| units.from_q0(length.steps()).map_err(named);
            let at = |x: construct_params::Length| -> Result<i64, GeometryError> {
                along
                    .checked_add(
                        steps(x)?
                            .checked_mul(seat.sign)
                            .ok_or_else(super::too_big)?,
                    )
                    .ok_or_else(super::too_big)
            };
            let (x0, x1) = (at(span.x[0])?, at(span.x[1])?);
            placed.push(Placed {
                key,
                noun: "family",
                op: op_id(&seat.recipe)?,
                span: Span {
                    start: x0.min(x1),
                    end: x0.max(x1),
                    foot: z
                        .checked_add(steps(span.z[0])?)
                        .ok_or_else(super::too_big)?,
                    head: z
                        .checked_add(steps(span.z[1])?)
                        .ok_or_else(super::too_big)?,
                },
            });
            continue;
        }
        if seat.sign != 1 {
            return Err(fault(
                "SeatMismatch",
                format!(
                    "{}: a box is seated the way its wall runs",
                    opening::who(h.recipe, h.key)
                ),
            ));
        }
        // A door's box reaches across from its seat's origin by its thickness; a plain opening
        // goes through the whole wall, so only its place along it is read.
        let side = if seat.recipe.op == PLAIN_OP {
            0
        } else {
            let deep = Extrusion::door(&seat.recipe.args)
                .ok()
                .map(|door| door.long_and_across().1)
                .map_or(Ok(0), |mm| units.steps(mm))?;
            let far = left.checked_add(deep).ok_or_else(super::too_big)?;
            across(left)?.min(across(far)?)
        };
        framed.push(Hosted {
            key: h.key,
            recipe: &seat.recipe,
            at: P3::new(along, side, z).map_err(|_| super::too_big())?,
        });
    }
    let cuts = opening::plan_in(frame.frame, &framed, &placed, units, band)?;
    Ok((frame, cuts))
}

/// Whether the doors `hosted` fit in `wall` by the openings' rules, asked without the engine.
///
/// # Errors
///
/// The openings' refusals, each naming the door and why; for a wall at an angle, [`in_frame`]'s.
pub(super) fn fits(
    wall: &Layered,
    hosted: &[Hosted<'_>],
    units: Units,
    band: i64,
    families: &dyn FamilySource,
) -> Result<(), GeometryError> {
    cuts(wall, hosted, units, band, families).map(|_| ())
}

/// The engine's mesh in whole millimetres, each coordinate the nearest millimetre, and whether
/// any was rounded; the lowest and highest grid steps it reaches widen `extent`.
pub(super) fn millimetres(
    mesh: &DisplayMesh,
    units: Units,
    extent: &mut Extent,
) -> Result<(Mesh, bool), GeometryError> {
    let mut positions = Vec::with_capacity(mesh.positions.len().saturating_mul(3));
    let mut rounded = false;
    for point in &mesh.positions {
        let mut steps = [0_i64; 3];
        for (slot, &c) in steps.iter_mut().zip(point) {
            *slot = Units::whole_steps(c)?;
        }
        for &s in &steps {
            let (mm, exact) = units.nearest_mm(s);
            rounded |= !exact;
            positions.push(mm);
        }
        *extent = Some(match *extent {
            None => (steps, steps),
            Some((mut lo, mut hi)) => {
                for ((l, h), &s) in lo.iter_mut().zip(hi.iter_mut()).zip(&steps) {
                    *l = (*l).min(s);
                    *h = (*h).max(s);
                }
                (lo, hi)
            }
        });
    }
    let count = mesh.positions.len();
    let mut indices = Vec::with_capacity(mesh.triangles.len().saturating_mul(3));
    for &index in mesh.triangles.iter().flatten() {
        if !usize::try_from(index).is_ok_and(|i| i < count) {
            return Err(fault("EngineRefused", "mesh: a triangle past its points"));
        }
        indices.push(index);
    }
    Ok((Mesh { positions, indices }, rounded))
}

/// A count of a mesh's items, of three numbers each, as a display's triangle numbers are.
fn thirds(len: usize) -> Result<u32, GeometryError> {
    u32::try_from(len.checked_div(3).unwrap_or(0))
        .map_err(|_| fault("TooBig", "a mesh with too many points or triangles"))
}

/// Puts `part` after what `into` holds, its points after `into`'s and its triangles numbered so,
/// and gives its first triangle and how many it has.
pub(super) fn append(into: &mut Mesh, part: &Mesh) -> Result<[u32; 2], GeometryError> {
    let base = thirds(into.positions.len())?;
    let first = thirds(into.indices.len())?;
    into.positions.extend_from_slice(&part.positions);
    for &i in &part.indices {
        into.indices.push(
            i.checked_add(base)
                .ok_or_else(|| fault("TooBig", "a mesh with too many points"))?,
        );
    }
    Ok([first, thirds(part.indices.len())?])
}

/// The plan outline of a strip whose corners are in the building grid's steps, in whole
/// millimetres, each corner the nearest millimetre.
fn outline(corners: &[[i64; 2]; 4], units: Units) -> Result<Vec<[i64; 2]>, GeometryError> {
    let mut out = Vec::with_capacity(4);
    for &[x, y] in corners {
        let (x, _) = units.nearest_mm(units.from_q0(x)?);
        let (y, _) = units.nearest_mm(units.from_q0(y)?);
        out.push([x, y]);
    }
    Ok(out)
}

/// An empty mesh.
pub(super) fn empty() -> Mesh {
    Mesh {
        positions: Vec::new(),
        indices: Vec::new(),
    }
}

/// Builds a layered wall with the doors `hosted` in it, on `engine`.
///
/// # Errors
///
/// The plan's and the openings' refusals, and the engine's own (a door's with its key in front).
#[allow(
    clippy::too_many_lines,
    reason = "one wall's layers, cuts, meshes and names, in the order the record lists them"
)]
pub(super) fn build<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    wall: &Layered,
    hosted: &[Hosted<'_>],
    units: Units,
    band: i64,
    families: &dyn FamilySource,
) -> Result<Built, GeometryError> {
    let op = op_id(recipe)?;
    let (cut_list, tools) = match cuts(wall, hosted, units, band, families)? {
        None => (Vec::new(), Vec::new()),
        Some((frame, cuts)) => {
            let tools = cuts
                .iter()
                .map(|cut| frame.cutter(cut))
                .collect::<Result<Vec<_>, _>>()?;
            (cuts, tools)
        }
    };
    let mut asked = Asked {
        layers: Vec::new(),
        subtract: cut_list
            .iter()
            .zip(&tools)
            .map(|(cut, (tool, by))| AskedCut {
                key: cut.key().to_owned(),
                extrude: as_steps(&tool.outer),
                by: *by,
            })
            .collect(),
    };
    let by = [0, 0, units.from_q0(wall.height)?];
    let mut cutters = Vec::with_capacity(tools.len());
    for (cut, (tool, up)) in cut_list.iter().zip(&tools) {
        let named = |e: EngineError| cut.engine_error(from_engine(e));
        cutters.push((cut, engine.extrude(tool, *up).map_err(named)?));
    }
    let (mut fine, mut coarse) = (empty(), empty());
    let (mut fine_parts, mut coarse_parts) = (Vec::new(), Vec::new());
    let (mut rounded_fine, mut rounded_coarse) = (false, false);
    // Only the finest level's reach makes the bounds; the coarse level's is not kept.
    let (mut extent, mut coarse_extent): (Extent, Extent) = (None, None);
    for layer in &wall.layers {
        if layer.thickness() == 0 {
            continue;
        }
        let (corners, _) = wall.plan.strip(layer.exterior, layer.interior)?;
        let outer = corners
            .iter()
            .map(|&[x, y]| point([units.from_q0(x)?, units.from_q0(y)?], 0))
            .collect::<Result<Vec<P3>, _>>()?;
        asked.layers.push(AskedLayer {
            function: layer.function.clone(),
            material: layer.material.clone(),
            extrude: as_steps(&outer),
            by,
        });
        let profile = Profile {
            outer,
            holes: Vec::new(),
        };
        let mut solid = engine.extrude(&profile, by).map_err(from_engine)?;
        let plain = engine.mesh(&solid).map_err(from_engine)?;
        for (cut, cutter) in &cutters {
            let named = |e: EngineError| cut.engine_error(from_engine(e));
            solid = engine.subtract(&solid, cutter).map_err(named)?;
        }
        // A layer with no openings has the same mesh at both levels.
        let (cut_mesh, r0) = if cutters.is_empty() {
            millimetres(&plain, units, &mut extent)?
        } else {
            millimetres(
                &engine.mesh(&solid).map_err(from_engine)?,
                units,
                &mut extent,
            )?
        };
        let (plain, r1) = millimetres(&plain, units, &mut coarse_extent)?;
        rounded_coarse |= r1;
        rounded_fine |= r0;
        let shape = outline(&corners, units)?;
        let part = |triangles: [u32; 2]| Part {
            material: layer.material.clone(),
            function: layer.function.clone(),
            triangles,
            outline: shape.clone(),
        };
        fine_parts.push(part(append(&mut fine, &cut_mesh)?));
        coarse_parts.push(part(append(&mut coarse, &plain)?));
    }
    let (lo, hi) =
        extent.ok_or_else(|| fault("EmptyShape", "a wall with no layer of any thickness"))?;
    let bounds = Bounds {
        min: lo.map(|s| units.floor_mm(s)),
        max: hi.map(|s| units.ceil_mm(s)),
    };
    let mut names = own_names(op, wall)?;
    for cut in &cut_list {
        names.extend(cut.names(op)?);
    }
    names.sort_by(|a, b| a.name.cmp(&b.name));
    let (exterior, interior) = wall.outer();
    let (whole, _) = wall.plan.strip(exterior, interior)?;
    let whole = outline(&whole, units)?;
    let displays = vec![
        Display {
            lod: 0,
            deviation: i64::from(rounded_fine),
            mesh: fine,
            outline: whole.clone(),
            parts: fine_parts,
        },
        Display {
            lod: 1,
            deviation: opening::deviation(&cut_list, units)
                .saturating_add(i64::from(rounded_coarse)),
            mesh: coarse,
            outline: whole,
            parts: coarse_parts,
        },
    ];
    Ok(Built {
        body: serde_json::to_vec(&asked).unwrap_or_default(),
        bounds,
        names,
        displays,
    })
}

/// The plane the side of a strip stands on, from the plan edge `from` to `to` (engine steps) that
/// sweeps it: the outside is on the right of an edge of a counter-clockwise outline, so the
/// normal is the edge turned a quarter right (`Kernel::solid` does the same for the old box).
pub(super) fn side(from: [i64; 2], to: [i64; 2]) -> Result<GridPlane, GeometryError> {
    let less = |a: i64, b: i64| {
        i128::from(a)
            .checked_sub(i128::from(b))
            .ok_or_else(super::too_big)
    };
    let normal = [less(to[1], from[1])?, less(from[0], to[0])?, 0];
    let offset = normal[0]
        .checked_mul(i128::from(from[0]))
        .and_then(|a| {
            normal[1]
                .checked_mul(i128::from(from[1]))
                .and_then(|b| a.checked_add(b))
        })
        .ok_or_else(super::too_big)?;
    carrier(normal, offset)
}

/// One solid for each layer with a thickness, with the plane of each named face, for a drawing to
/// cut: the strip of the layer extruded from the wall's base to its top, with no opening cut in
/// it (a door's opening is [`build`]'s; a drawing of the wall's cut shows the wall whole until
/// doors are families, job 6.4). A face is named as [`build`] names it: the wall's `base` and
/// `top`, its `end0` and `end1`, and the layer's two sides, `exterior` or `face.<k>` and
/// `interior` or `face.<k + 1>`, so two layers that meet name their shared face alike.
///
/// # Errors
///
/// The plan's refusals (`Degenerate`, `TooBig`), `Naming`, and the engine's own.
pub(super) fn solids<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    wall: &Layered,
    units: Units,
) -> Result<Vec<LayerSolid<E::Solid>>, GeometryError> {
    let op = op_id(recipe)?;
    let height = units.from_q0(wall.height)?;
    let last = wall.layers.len().saturating_sub(1);
    let mut found = Vec::new();
    for (k, layer) in wall.layers.iter().enumerate() {
        if layer.thickness() == 0 {
            continue;
        }
        let (corners, _) = wall.plan.strip(layer.exterior, layer.interior)?;
        let mut steps = Vec::with_capacity(4);
        for [x, y] in corners {
            steps.push([units.from_q0(x)?, units.from_q0(y)?]);
        }
        let outer = steps
            .iter()
            .map(|&xy| point(xy, 0))
            .collect::<Result<Vec<P3>, _>>()?;
        let solid = engine
            .extrude(
                &Profile {
                    outer,
                    holes: Vec::new(),
                },
                [0, 0, height],
            )
            .map_err(from_engine)?;
        let outside = if k == 0 {
            "exterior".to_owned()
        } else {
            format!("face.{k}")
        };
        let inside = if k == last {
            "interior".to_owned()
        } else {
            format!("face.{}", k.saturating_add(1))
        };
        // The strip's corners run counter-clockwise; whether its first edge is the layer's
        // exterior face or its start end depends on which way the layer's faces turn.
        let exterior_face = wall.plan.face(layer.exterior)?;
        let first_is_exterior =
            corners.first() == exterior_face.first() && corners.get(1) == exterior_face.get(1);
        let roles = if first_is_exterior {
            [outside, "end1".to_owned(), inside, "end0".to_owned()]
        } else {
            ["end0".to_owned(), inside, "end1".to_owned(), outside]
        };
        let mut faces = vec![
            NamedFace {
                plane: carrier([0, 0, -1], 0)?,
                name: name(op, "base")?,
            },
            NamedFace {
                plane: carrier([0, 0, 1], i128::from(height))?,
                name: name(op, "top")?,
            },
        ];
        for (i, role) in roles.iter().enumerate() {
            let from = steps.get(i).copied().ok_or_else(super::too_big)?;
            let next = i.checked_add(1).and_then(|n| n.checked_rem(4)).unwrap_or(0);
            let to = steps.get(next).copied().ok_or_else(super::too_big)?;
            faces.push(NamedFace {
                plane: side(from, to)?,
                name: name(op, role)?,
            });
        }
        found.push(LayerSolid {
            function: layer.function.clone(),
            material: Some(layer.material.clone()),
            solid: KernelSolid { solid, faces },
        });
    }
    if found.is_empty() {
        return Err(fault("EmptyShape", "a wall with no layer of any thickness"));
    }
    Ok(found)
}
