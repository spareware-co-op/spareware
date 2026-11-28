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
//!   (construct-regen's `hosting`, [`crate::seating::SEAT_ARG`]), its place read from its seat in
//!   the wall's frame exactly, never from its placement, which is only the nearest millimetre to
//!   it; the rules are asked in that frame (`opening::Long` for its length). Each layer is then
//!   made without a subtraction: cut along the wall at its ends and at every opening's jambs into
//!   stretches, each stretch's heights outside the openings in runs (a stretch beside the openings
//!   its whole height, one across an opening the pieces under and over it), each run a cell,
//!   extruded and united, each sharing part of a side with one before it and none touching the
//!   rest only along an edge (the engine's union keeps its pieces and refuses that). A cell's
//!   corners are the wall's frame points on the layer's two faces at those jambs, each put through
//!   the wall's frame and so rounded once, as the element's own corners are (a door flush with a
//!   face has its jambs' corners there, exactly the layer's); the layer's corners at the wall's
//!   ends are its strip's.
//!   So every vertex of the cut layer is a grid point, as the engine meshes only those (a
//!   subtraction of a cutter at an angle would leave vertices where a rounded jamb meets a
//!   rounded face, which no grid point holds). The body lists each layer's cells and each
//!   opening's place in the frame. An element in such a wall that is not seated (an older door
//!   hosted by its position), or seated for another wall, is refused (`NotSeated`,
//!   `SeatMismatch`).
//! - A placed family's opening (job 6.4, [`crate::families`]): the outline its recipe declares,
//!   solved at its type's values, placed where the family stands (its origin and its turn, as the
//!   regenerator seats it: construct-regen's `hosting::seat`) and cut through every layer on the
//!   same path as a door's, by the same rules ([`super::opening::plan_in`]). Where the family's
//!   recipe comes from is the caller's [`FamilySource`].
//! - Wrapping at inserts (job 6.1, CON-A4-F2-P4): where the filled recipe's `wrap` wraps a side
//!   at inserts ([`crate::wall_plan::Wrap`]), each door's and placed family's opening is cut from
//!   each layer set back by construct-walls' wrapping rule ([`construct_walls::turned`]): grown
//!   along the wall at both jambs, and up at its head and down at its sill where they are inside
//!   the wall, by the thicknesses of the wrapping layers that turn in front of it; and each
//!   wrapping layer's turned part is built as boxes round the opening (two jambs the full grown
//!   height, a head and a sill between them), across the layers it covers, each listed in the
//!   body and the displays as a piece of that layer. A plain opening is a hole and is cut flush.
//!   A wall that wraps nothing at inserts is built exactly as before.
//! - Joins (job 5.1): a wall whose filled recipe has `joins` ([`crate::wall_plan::Joints`]) is
//!   built of each layer's joined pieces in place of its strips, each piece extruded the same way
//!   and listed in the body as a layer of its own, in order; each opening is cut from every piece
//!   its cutter's plan box reaches, and the display's outline is the joined outline. The pieces
//!   are worked out before building (construct-walls' join rule, filled by construct-regen's
//!   `joins`, with crosses, joins at any angle and wrapped free ends from job 6.1), so this
//!   builder never reads another wall.
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
    AskedCut, Extrusion, KernelSolid, LayerSolid, NamedFace, Opened, Units, WALL_ROLES, as_steps,
    carrier, fault, from_engine, name, op_id,
};
use crate::families::{self, FamilySource};
use crate::seating;
use crate::wall_plan::{Axis, Layered, Strip};
use construct_families::Seat;
use construct_model::geometry::{
    Bounds, Built, Display, FaceName, GeometryError, Mesh, Part, Recipe,
};
use construct_walls::WrapLayer;
use engine_geometry::value::engine::{DisplayMesh, Engine, EngineError, GridPlane, Profile};
use engine_geometry::value::grid::P3;
use engine_geometry::value::op::OpId;
use serde::Serialize;
use std::collections::BTreeSet;

/// A layered wall's body bytes: what the engine was asked for each layer, and the cutter of each
/// opening, once for all the layers. A wall with no openings writes no `subtract`.
#[derive(Serialize)]
struct Asked {
    layers: Vec<AskedLayer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    subtract: Vec<AskedCut>,
    /// In a wall at an angle, each opening's place in the wall's frame, in the order of their keys.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    openings: Vec<AskedOpening>,
    /// Where a wall wraps at inserts: each layer's own cutters at each insert, set back.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    grown: Vec<AskedGrown>,
}

/// One layer's own cutter at an insert where the wall wraps.
#[derive(Serialize)]
struct AskedGrown {
    layer: usize,
    key: String,
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
}

/// One layer as the engine was asked for it: its strip, and in a wall at an angle with openings
/// the cells it is made of, each extruded and united in order.
#[derive(Serialize)]
struct AskedLayer {
    function: String,
    material: String,
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cells: Vec<AskedCell>,
}

/// One cell of a layer in a wall at an angle, as the engine was asked for it.
#[derive(Serialize)]
struct AskedCell {
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
}

/// An opening's place in a wall at an angle's frame, in steps: along and up.
#[derive(Serialize)]
struct AskedOpening {
    key: String,
    along: [i64; 2],
    up: [i64; 2],
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

    /// A placed family that hangs against the wall's face, checked to lie along the wall: the
    /// boxes of its solids, turned as it stands, from where its origin is, must not cross the
    /// wall's ends. It cuts nothing, so nothing else is asked of it.
    fn along_the_wall(
        &self,
        hosted: &Hosted<'_>,
        families: &dyn FamilySource,
        units: Units,
    ) -> Result<(), GeometryError> {
        let a = self.frame.along;
        let sign = self.unit.get(a).copied().unwrap_or(1);
        let origin = self.to_frame([hosted.at.x.get(), hosted.at.y.get()])?;
        let along = origin.first().copied().unwrap_or_default();
        self.lie_along(hosted, families, units, (a, sign), along)
    }

    /// Whether the family `hosted`, its origin `along` the wall from its start, lies along it:
    /// each box of its solids reaches along `axis` of its own frame, which runs `sign` times the
    /// way the wall does, and no box may cross either end of the wall. Used by a wall along an
    /// axis ([`Self::along_the_wall`]) and a wall at an angle (its seat gives the way).
    fn lie_along(
        &self,
        hosted: &Hosted<'_>,
        families: &dyn FamilySource,
        units: Units,
        (axis, sign): (usize, i64),
        along: i64,
    ) -> Result<(), GeometryError> {
        let key = hosted.key;
        let named = |e: GeometryError| fault(&e.code, format!("family {key}: {}", e.message));
        let instance = families::Instance::of(hosted.recipe).map_err(named)?;
        let resolved = families::resolve(families, &instance).map_err(named)?;
        let boxes = construct_families::extents(
            &resolved.recipe,
            &resolved.values,
            construct_families::Limits::DEFAULT,
            instance.turn,
        )
        .map_err(|e| named(families::evaluation_refused(&resolved.recipe, &e)))?;
        let (mut low, mut high) = (i64::MAX, i64::MIN);
        for b in &boxes {
            for edge in [b.low.get(axis), b.high.get(axis)].into_iter().flatten() {
                let at = units.from_q0(edge.steps()).map_err(named)?;
                let reach = times(at, sign)?;
                low = low.min(reach);
                high = high.max(reach);
            }
        }
        let (start, end) = (add(along, low)?, add(along, high)?);
        if start < 0 || self.frame.length.against(end) == std::cmp::Ordering::Greater {
            return Err(fault(
                "OpeningPastEnd",
                format!("family {key}: it crosses the wall's end"),
            ));
        }
        Ok(())
    }

    /// A cut's cutter in plan, as the engine is asked for it: its corners at its foot, and the
    /// vector up to its head.
    fn cutter(&self, cut: &Cut) -> Result<(Profile, [i64; 3]), GeometryError> {
        let (lo, hi) = cut.frame_box();
        self.frame_solid(lo, hi)
    }

    /// A box of the wall's frame, its lowest and highest corners as a cut's frame box holds them,
    /// as the engine is asked for it: its corners at its foot, and the vector up to its head.
    fn frame_solid(
        &self,
        lo: [i64; 3],
        hi: [i64; 3],
    ) -> Result<(Profile, [i64; 3]), GeometryError> {
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
    // A family that hangs against the wall's face cuts nothing: it only has to lie along the wall.
    let (faced, cutting): (Vec<&Hosted<'_>>, Vec<&Hosted<'_>>) = hosted.iter().partition(|h| {
        families::is_instance(h.recipe) && families::sits_on_face(families, h.recipe)
    });
    for h in faced {
        frame.along_the_wall(h, families, units)?;
    }
    let (placed, doors): (Vec<&Hosted<'_>>, Vec<&Hosted<'_>>) = cutting
        .into_iter()
        .partition(|h| families::is_instance(h.recipe));
    if placed.is_empty() && doors.is_empty() {
        return Ok(None);
    }
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
    let (recipe, seat) = seating::unseated(hosted.recipe).map_err(named)?;
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
        if families::is_instance(h.recipe) && families::sits_on_face(families, &seat.recipe) {
            // Hung against the wall's face: it cuts nothing, it only lies along the wall.
            let on = Hosted {
                key: h.key,
                recipe: &seat.recipe,
                at: h.at,
            };
            frame.lie_along(&on, families, units, (0, seat.sign), along)?;
            continue;
        }
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

/// Where each opening the doors and placed families `hosted` make lies in `wall`, in plan and up
/// the wall, in the building's grid steps ([`Opened`]), asked without the engine and by the same
/// rules ([`cuts`]); none when nothing is hosted. Plan numbers are relative to the wall's
/// placement, as its line is.
///
/// # Errors
///
/// The openings' refusals, each naming the door and why; `NotAHost` for a wall at an angle.
pub(super) fn opened(
    wall: &Layered,
    hosted: &[Hosted<'_>],
    units: Units,
    band: i64,
    families: &dyn FamilySource,
) -> Result<Vec<Opened>, GeometryError> {
    let Some((frame, cuts)) = cuts(wall, hosted, units, band, families)? else {
        return Ok(Vec::new());
    };
    let axis = frame.frame.along;
    let sign = frame.unit.get(axis).copied().unwrap_or(1);
    let origin = frame.start.get(axis).copied().unwrap_or_default();
    cuts.iter()
        .map(|cut| {
            let ([from, to], [foot, head]) = cut.stretch();
            // The line's unit vector along the axis is +1 or -1: along grows toward the wall's end,
            // which is the lower plan number when the wall runs the negative way.
            let at = |along: i64| add(origin, times(sign, along)?);
            let (a, b) = (at(from)?, at(to)?);
            Ok(Opened {
                key: cut.key().to_owned(),
                noun: cut.noun(),
                axis,
                low: units.to_q0(a.min(b), false)?,
                high: units.to_q0(a.max(b), true)?,
                foot: units.to_q0(foot, false)?,
                head: units.to_q0(head, true)?,
            })
        })
        .collect()
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

/// The plan outline of a polygon whose corners are in the building grid's steps, in whole
/// millimetres, each corner the nearest millimetre.
fn outline(corners: &[[i64; 2]], units: Units) -> Result<Vec<[i64; 2]>, GeometryError> {
    let mut out = Vec::with_capacity(corners.len());
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

/// A cell of a layer as the engine is asked for it: its corners at its foot and the vector up to
/// its head.
type Cell = (Vec<P3>, [i64; 3]);

/// Where a cell of a layer in a wall at an angle is cut along the wall: its start, a jamb `along`
/// steps from the start, or its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Break {
    Start,
    At(i64),
    End,
}

/// The cells of `layer` of the wall at an angle `wall`, whose line is `line` (in the engine's
/// steps), with the openings `cuts` taken out (the module header): each a cell's corners at its
/// foot, anticlockwise from above, and the vector up to its head, in the order they are united.
fn layer_cells(
    wall: &Layered,
    layer: &crate::wall_plan::Strip,
    line: &Seat,
    cuts: &[Cut],
    units: Units,
) -> Result<Vec<Cell>, GeometryError> {
    let spans: Vec<Span> = cuts.iter().map(Cut::span).collect();
    let mut along: Vec<Break> = spans
        .iter()
        .flat_map(|s| [Break::At(s.start), Break::At(s.end)])
        .chain([Break::Start, Break::End])
        .collect();
    along.sort_unstable();
    along.dedup();
    let height = units.from_q0(wall.height)?;
    let mut up: Vec<i64> = spans
        .iter()
        .flat_map(|s| [s.foot.max(0), s.head])
        .chain([0, height])
        .collect();
    up.sort_unstable();
    up.dedup();
    // The layer's corner on its face `offset` steps right of the line at a break: its strip's at
    // the wall's ends, the frame point on that face at a jamb.
    let corner = |at: Break, offset: i64| -> Result<[i64; 2], GeometryError> {
        match at {
            Break::Start | Break::End => {
                let face = wall.plan.face(offset)?;
                let [x, y] = if at == Break::Start { face[0] } else { face[1] };
                Ok([units.from_q0(x)?, units.from_q0(y)?])
            }
            Break::At(a) => {
                let left = units
                    .from_q0(offset)?
                    .checked_neg()
                    .ok_or_else(super::too_big)?;
                line.plan([a, left]).ok_or_else(super::too_big)
            }
        }
    };
    let value = |at: Break| match at {
        Break::Start => i64::MIN,
        Break::At(a) => a,
        Break::End => i64::MAX,
    };
    let open = |k: usize, b: usize| -> bool {
        let pick = |v: &[Break], i: usize| v.get(i).copied().map(value);
        let (Some(a0), Some(a1)) = (pick(&along, k), pick(&along, k.saturating_add(1))) else {
            return false;
        };
        let (Some(&z0), Some(&z1)) = (up.get(b), up.get(b.saturating_add(1))) else {
            return false;
        };
        spans
            .iter()
            .any(|s| s.start <= a0 && a1 <= s.end && s.foot.max(0) <= z0 && z1 <= s.head)
    };
    // Each stretch along the wall, its runs of heights outside the openings: a column the whole
    // height beside every opening, the pieces under and over one in its stretch.
    let mut pieces: Vec<(usize, i64, i64)> = Vec::new();
    for k in 0..along.len().saturating_sub(1) {
        let mut run: Option<(i64, i64)> = None;
        for b in 0..up.len().saturating_sub(1) {
            let (Some(&z0), Some(&z1)) = (up.get(b), up.get(b.saturating_add(1))) else {
                continue;
            };
            if open(k, b) {
                if let Some((low, high)) = run.take() {
                    pieces.push((k, low, high));
                }
            } else {
                run = Some(run.map_or((z0, z1), |(low, _)| (low, z1)));
            }
        }
        if let Some((low, high)) = run {
            pieces.push((k, low, high));
        }
    }
    let mut cells = Vec::with_capacity(pieces.len());
    for at in joined_order(&pieces) {
        let Some(&(k, z0, z1)) = pieces.get(at) else {
            continue;
        };
        let at = |i: usize| along.get(i).copied().ok_or_else(super::too_big);
        let (a0, a1) = (at(k)?, at(k.saturating_add(1))?);
        let mut ring = vec![
            corner(a0, layer.exterior)?,
            corner(a1, layer.exterior)?,
            corner(a1, layer.interior)?,
            corner(a0, layer.interior)?,
        ];
        if area_twice(&ring) < 0 {
            ring.reverse();
        }
        let outer = ring
            .into_iter()
            .map(|xy| point(xy, z0))
            .collect::<Result<Vec<_>, _>>()?;
        let rise = z1.checked_sub(z0).ok_or_else(super::too_big)?;
        cells.push((outer, [0, 0, rise]));
    }
    Ok(cells)
}

/// The order pieces of a cut layer are united in, each `(stretch, low, high)`: from the first,
/// each next the least piece that shares part of a side with one already taken (a neighbouring
/// stretch, overlapping heights), so the union stays one solid and no piece touches it only along
/// an edge; a piece none reaches comes after, least first.
fn joined_order(pieces: &[(usize, i64, i64)]) -> Vec<usize> {
    let shares = |a: (usize, i64, i64), b: (usize, i64, i64)| {
        a.0.abs_diff(b.0) == 1 && a.1.max(b.1) < a.2.min(b.2)
    };
    let mut order = Vec::with_capacity(pieces.len());
    let mut taken = BTreeSet::new();
    while let Some(first) = (0..pieces.len()).find(|i| !taken.contains(i)) {
        let mut reach = BTreeSet::from([first]);
        while let Some(next) = reach.pop_first() {
            if !taken.insert(next) {
                continue;
            }
            order.push(next);
            let Some(&piece) = pieces.get(next) else {
                continue;
            };
            for (i, &other) in pieces.iter().enumerate() {
                if !taken.contains(&i) && shares(piece, other) {
                    reach.insert(i);
                }
            }
        }
    }
    order
}

/// Twice the signed area of a plan ring, positive when it runs anticlockwise from above.
fn area_twice(ring: &[[i64; 2]]) -> i128 {
    ring.iter()
        .enumerate()
        .map(|(i, a)| {
            let next = i.checked_add(1).and_then(|n| n.checked_rem(ring.len()));
            let b = next.and_then(|n| ring.get(n)).copied().unwrap_or(*a);
            i128::from(a[0])
                .saturating_mul(i128::from(b[1]))
                .saturating_sub(i128::from(a[1]).saturating_mul(i128::from(b[0])))
        })
        .fold(0_i128, i128::saturating_add)
}

/// The cells `cells` extruded and united in their order, as one solid.
fn united<E: Engine>(engine: &mut E, cells: Vec<Cell>) -> Result<E::Solid, GeometryError> {
    let mut made: Option<E::Solid> = None;
    for (outer, up) in cells {
        let piece = engine
            .extrude(
                &Profile {
                    outer,
                    holes: Vec::new(),
                },
                up,
            )
            .map_err(from_engine)?;
        made = Some(match made {
            None => piece,
            Some(so_far) => engine.unite(&so_far, &piece).map_err(from_engine)?,
        });
    }
    made.ok_or_else(|| fault("EmptyShape", "a layer with no cell outside its openings"))
}

/// The plan pieces of layer `at` of `wall`, in the building grid's steps: its joined pieces when
/// the wall's joints are made, else its strip.
fn pieces_of(
    wall: &Layered,
    at: usize,
    layer: &Strip,
) -> Result<Vec<Vec<[i64; 2]>>, GeometryError> {
    if let Some(joints) = &wall.joined {
        return joints
            .layers
            .get(at)
            .cloned()
            .ok_or_else(|| fault("BadJoins", format!("layer {at} has no joined pieces")));
    }
    let (corners, _) = wall.plan.strip(layer.exterior, layer.interior)?;
    Ok(vec![corners.to_vec()])
}

/// The low and high corners in plan of a polygon of grid points.
fn plan_box(points: &[P3]) -> Option<([i64; 2], [i64; 2])> {
    let mut points = points.iter().map(|p| [p.x.get(), p.y.get()]);
    let first = points.next()?;
    Some(points.fold((first, first), |(lo, hi), [x, y]| {
        ([lo[0].min(x), lo[1].min(y)], [hi[0].max(x), hi[1].max(y)])
    }))
}

/// Whether two plan boxes share an area.
fn overlap(a: Option<([i64; 2], [i64; 2])>, b: Option<([i64; 2], [i64; 2])>) -> bool {
    let (Some((alo, ahi)), Some((blo, bhi))) = (a, b) else {
        return false;
    };
    alo[0] < bhi[0] && blo[0] < ahi[0] && alo[1] < bhi[1] && blo[1] < ahi[1]
}

/// An opening's cutter as the engine made it, with its plan box.
struct Cutter<'c, S> {
    cut: &'c Cut,
    solid: S,
    reach: Option<([i64; 2], [i64; 2])>,
}

/// One piece of a layer on the engine: its extrusion's mesh, and its mesh less every opening
/// whose cutter reaches it, when one does.
fn piece<E: Engine>(
    engine: &mut E,
    profile: &Profile,
    by: [i64; 3],
    cutters: &[&Cutter<'_, E::Solid>],
) -> Result<(DisplayMesh, Option<DisplayMesh>), GeometryError> {
    let mut solid = engine.extrude(profile, by).map_err(from_engine)?;
    let plain = engine.mesh(&solid).map_err(from_engine)?;
    let reach = plan_box(&profile.outer);
    let mut cut_any = false;
    for cutter in cutters.iter().filter(|c| overlap(c.reach, reach)) {
        let named = |e: EngineError| cutter.cut.engine_error(from_engine(e));
        solid = engine.subtract(&solid, &cutter.solid).map_err(named)?;
        cut_any = true;
    }
    let cut = if cut_any {
        Some(engine.mesh(&solid).map_err(from_engine)?)
    } else {
        None
    };
    Ok((plain, cut))
}

/// What a layered wall's pieces make at both levels of detail, as they are added.
struct Levels {
    fine: Mesh,
    coarse: Mesh,
    fine_parts: Vec<Part>,
    coarse_parts: Vec<Part>,
    rounded_fine: bool,
    rounded_coarse: bool,
    /// Only the finest level's reach makes the bounds; the coarse level's is not kept.
    extent: Extent,
    coarse_extent: Extent,
}

impl Levels {
    fn new() -> Levels {
        Levels {
            fine: empty(),
            coarse: empty(),
            fine_parts: Vec::new(),
            coarse_parts: Vec::new(),
            rounded_fine: false,
            rounded_coarse: false,
            extent: None,
            coarse_extent: None,
        }
    }

    /// Adds a piece of `layer`, its plan outline `shape` in whole millimetres.
    fn add(
        &mut self,
        layer: &Strip,
        shape: &[[i64; 2]],
        (plain, cut): (DisplayMesh, Option<DisplayMesh>),
        units: Units,
    ) -> Result<(), GeometryError> {
        // A piece with no openings has the same mesh at both levels.
        let (cut_mesh, r0) = millimetres(cut.as_ref().unwrap_or(&plain), units, &mut self.extent)?;
        let (plain, r1) = millimetres(&plain, units, &mut self.coarse_extent)?;
        self.rounded_coarse |= r1;
        self.rounded_fine |= r0;
        let part = |triangles: [u32; 2]| Part {
            material: layer.material.clone(),
            function: layer.function.clone(),
            triangles,
            outline: shape.to_vec(),
        };
        self.fine_parts
            .push(part(append(&mut self.fine, &cut_mesh)?));
        self.coarse_parts
            .push(part(append(&mut self.coarse, &plain)?));
        Ok(())
    }
}

/// The cells of `layer` of the wall at an angle `wall` with openings (this module's header), or
/// the refusal that a wall joined to others is not cut in cells yet.
fn cells_of(
    wall: &Layered,
    layer: &Strip,
    line: &Seat,
    cuts: &[Cut],
    units: Units,
) -> Result<Vec<Cell>, GeometryError> {
    if wall.joined.is_some() {
        return Err(fault(
            "JoinedAtAnAngle",
            "a wall at an angle with openings is not joined to other walls yet (interim: its              layers are cut in cells from its own strip)",
        ));
    }
    layer_cells(wall, layer, line, cuts, units)
}

/// What the body records of one piece of `layer`: its outline, its height and its cells.
fn asked_layer(layer: &Strip, outer: &[P3], by: [i64; 3], cells: &[Cell]) -> AskedLayer {
    AskedLayer {
        function: layer.function.clone(),
        material: layer.material.clone(),
        extrude: as_steps(outer),
        by,
        cells: cells
            .iter()
            .map(|(corners, up)| AskedCell {
                extrude: as_steps(corners),
                by: *up,
            })
            .collect(),
    }
}

/// One piece of a layer on the engine: with openings in a wall at an angle it is its `cells`
/// united, no subtraction; else [`piece`].
fn cut_piece<E: Engine>(
    engine: &mut E,
    profile: &Profile,
    by: [i64; 3],
    cutters: &[&Cutter<'_, E::Solid>],
    cells: Vec<Cell>,
) -> Result<(DisplayMesh, Option<DisplayMesh>), GeometryError> {
    if cells.is_empty() {
        return piece(engine, profile, by, cutters);
    }
    let strip = engine.extrude(profile, by).map_err(from_engine)?;
    let plain = engine.mesh(&strip).map_err(from_engine)?;
    let joined = united(engine, cells)?;
    let cut = engine.mesh(&joined).map_err(from_engine)?;
    Ok((plain, Some(cut)))
}

/// What the body records before its layers: each cutter, and for a wall at an angle each
/// opening's place in the wall's frame.
fn asked_of(cut_list: &[Cut], tools: &[(Profile, [i64; 3])], at_an_angle: bool) -> Asked {
    Asked {
        layers: Vec::new(),
        subtract: cut_list
            .iter()
            .zip(tools)
            .map(|(cut, (tool, by))| AskedCut {
                key: cut.key().to_owned(),
                extrude: as_steps(&tool.outer),
                by: *by,
            })
            .collect(),
        openings: if at_an_angle {
            cut_list
                .iter()
                .map(|cut| {
                    let span = cut.span();
                    AskedOpening {
                        key: cut.key().to_owned(),
                        along: [span.start, span.end],
                        up: [span.foot, span.head],
                    }
                })
                .collect()
        } else {
            Vec::new()
        },
        grown: Vec::new(),
    }
}

/// How a wall's layers turn into its inserts (job 6.1): each layer's boxes to cut at each insert,
/// by the cut's index, and each turned part with the layer it belongs to, all as boxes of the
/// wall's frame (lowest and highest corners, as a cut's frame box holds them).
struct Reveal {
    /// For each layer, its boxes: the index of the cut each stands for, and its corners.
    cutters: Vec<Vec<FrameBox>>,
    /// Each turned part: its layer, and its corners.
    turns: Vec<FrameBox>,
}

/// A box of a wall's frame with the index it belongs to (a cut's, or a layer's): its lowest and
/// highest corners, as a cut's frame box holds them.
type FrameBox = (usize, [i64; 3], [i64; 3]);

/// `v` with its entry `at` set to `to`.
fn with(mut v: [i64; 3], at: usize, to: i64) -> [i64; 3] {
    if let Some(slot) = v.get_mut(at) {
        *slot = to;
    }
    v
}

/// How `wall`, framed as `frame`, turns its layers into the inserts among `cuts`; `None` when it
/// wraps nothing at inserts, or no insert is cut.
fn reveal(
    wall: &Layered,
    frame: &WallFrame,
    cuts: &[Cut],
    units: Units,
) -> Result<Option<Reveal>, GeometryError> {
    let Some(wrap) = &wall.wrap else {
        return Ok(None);
    };
    let (exterior, interior) = wrap.inserts.sides();
    if !(exterior || interior) || !cuts.iter().any(Cut::is_insert) {
        return Ok(None);
    }
    let mut layers = Vec::with_capacity(wall.layers.len());
    for (layer, &wraps) in wall.layers.iter().zip(&wrap.layers) {
        layers.push(WrapLayer {
            thickness: i64::try_from(layer.thickness()).map_err(|_| super::too_big())?,
            wraps,
        });
    }
    let Some(turned) = construct_walls::turned(&layers, wrap.core, exterior, interior) else {
        return Ok(None);
    };
    if !turned.turns_any() {
        return Ok(None);
    }
    let e = |steps: i64| units.from_q0(steps);
    let (along, across) = (frame.frame.along, frame.across_axis());
    let (thickness, height) = (frame.frame.thickness, frame.frame.height);
    let mut out = Reveal {
        cutters: vec![Vec::new(); wall.layers.len()],
        turns: Vec::new(),
    };
    for (at, cut) in cuts.iter().enumerate().filter(|(_, c)| c.is_insert()) {
        let (lo, hi) = cut.frame_box();
        let pick = |v: [i64; 3], i: usize| v.get(i).copied().unwrap_or_default();
        let (a0, a1, z0, z1) = (pick(lo, along), pick(hi, along), pick(lo, 2), pick(hi, 2));
        let (sill, head) = (z0 > 0, z1 < height);
        // The opening grown by `g`: along at both jambs, and at its head and sill where they are
        // inside the wall.
        let grown = |g: i64| -> Result<(i64, i64, i64, i64), GeometryError> {
            let less = |a: i64, b: i64| a.checked_sub(b).ok_or_else(super::too_big);
            let more = |a: i64, b: i64| a.checked_add(b).ok_or_else(super::too_big);
            Ok((
                less(a0, g)?,
                more(a1, g)?,
                if sill { less(z0, g)?.max(0) } else { z0 },
                if head { more(z1, g)?.min(height) } else { z1 },
            ))
        };
        let frame_box = |(b0, b1): (i64, i64), (c0, c1): (i64, i64), (h0, h1): (i64, i64)| {
            let lo = with(with(with(lo, along, b0), across, c0), 2, h0);
            let hi = with(with(with(hi, along, b1), across, c1), 2, h1);
            (lo, hi)
        };
        for (k, parts) in turned.setbacks.iter().enumerate() {
            let split = parts.len() > 1;
            for (n, part) in parts.iter().enumerate() {
                let (g0, g1, h0, h1) = grown(e(part.setback)?)?;
                // One part spans the whole wall, so its faces are never the layer's own; of two,
                // each runs from its own side's face to where the two meet.
                let (c0, c1) = match (split, n) {
                    (false, _) => (0, thickness),
                    (true, 0) => (0, e(part.to)?),
                    (true, _) => (e(part.from)?, thickness),
                };
                let (blo, bhi) = frame_box((g0, g1), (c0, c1), (h0, h1));
                if let Some(slot) = out.cutters.get_mut(k) {
                    slot.push((at, blo, bhi));
                }
            }
        }
        for turn in &turned.turns {
            let (near, far) = (e(turn.near)?, e(turn.far)?);
            let span = (e(turn.from)?, e(turn.to)?);
            let (n0, n1, nz0, nz1) = grown(near)?;
            let (f0, f1, fz0, fz1) = grown(far)?;
            // A cutter may reach past the wall's base or top to cut it cleanly; a turned part is
            // the wall's own, so it stops at them.
            let (fz0, fz1) = (fz0.max(0), fz1.min(height));
            let mut parts = vec![
                frame_box((f0, n0), span, (fz0, fz1)),
                frame_box((n1, f1), span, (fz0, fz1)),
            ];
            if head && nz1 < fz1 {
                parts.push(frame_box((n0, n1), span, (nz1, fz1)));
            }
            if sill && fz0 < nz0 {
                parts.push(frame_box((n0, n1), span, (fz0, nz0)));
            }
            for (plo, phi) in parts {
                out.turns.push((turn.layer, plo, phi));
            }
        }
    }
    Ok(Some(out))
}

/// Each layer's own cutters at the inserts `reveal` sets back, made on `engine` and listed in
/// `asked`.
fn own_cutters<'c, E: Engine>(
    engine: &mut E,
    frame: &WallFrame,
    reveal: &Reveal,
    cut_list: &'c [Cut],
    asked: &mut Asked,
) -> Result<Vec<Vec<Cutter<'c, E::Solid>>>, GeometryError> {
    let mut own = Vec::with_capacity(reveal.cutters.len());
    for (layer, boxes) in reveal.cutters.iter().enumerate() {
        let mut mine = Vec::with_capacity(boxes.len());
        for &(at, lo, hi) in boxes {
            let cut = cut_list.get(at).ok_or_else(super::too_big)?;
            let (tool, up) = frame.frame_solid(lo, hi)?;
            asked.grown.push(AskedGrown {
                layer,
                key: cut.key().to_owned(),
                extrude: as_steps(&tool.outer),
                by: up,
            });
            let named = |e: EngineError| cut.engine_error(from_engine(e));
            mine.push(Cutter {
                cut,
                solid: engine.extrude(&tool, up).map_err(named)?,
                reach: plan_box(&tool.outer),
            });
        }
        own.push(mine);
    }
    Ok(own)
}

/// Each wrapping layer's turned parts round each insert, built on `engine`, listed in `asked` and
/// added to `levels` as pieces of their layers.
fn add_turns<E: Engine>(
    engine: &mut E,
    frame: &WallFrame,
    reveal: &Reveal,
    wall: &Layered,
    asked: &mut Asked,
    levels: &mut Levels,
    units: Units,
) -> Result<(), GeometryError> {
    for &(at, lo, hi) in &reveal.turns {
        let layer = wall.layers.get(at).ok_or_else(super::too_big)?;
        let (tool, up) = frame.frame_solid(lo, hi)?;
        asked.layers.push(asked_layer(layer, &tool.outer, up, &[]));
        let shape: Vec<[i64; 2]> = tool
            .outer
            .iter()
            .map(|p| [units.nearest_mm(p.x.get()).0, units.nearest_mm(p.y.get()).0])
            .collect();
        let made = piece(engine, &tool, up, &[])?;
        levels.add(layer, &shape, made, units)?;
    }
    Ok(())
}

/// Each piece of each layer of `wall` with a thickness, extruded on `engine` and cut by its
/// cutters (`own`, a layer's own at its inserts where the wall wraps there, else `cutters`),
/// listed in `asked` and added to `levels`.
fn add_layers<E: Engine>(
    engine: &mut E,
    wall: &Layered,
    (cutters, own): (&[Cutter<'_, E::Solid>], &[Vec<Cutter<'_, E::Solid>>]),
    (line, cut_list): (Option<&Seat>, &[Cut]),
    asked: &mut Asked,
    levels: &mut Levels,
    units: Units,
) -> Result<(), GeometryError> {
    let by = [0, 0, units.from_q0(wall.height)?];
    for (at, layer) in wall.layers.iter().enumerate() {
        if layer.thickness() == 0 {
            continue;
        }
        let mine: Vec<&Cutter<'_, E::Solid>> = match own.get(at) {
            Some(own) => {
                let plain = cutters.iter().filter(|c| !c.cut.is_insert());
                own.iter().chain(plain).collect()
            }
            None => cutters.iter().collect(),
        };
        for corners in pieces_of(wall, at, layer)? {
            let outer = corners
                .iter()
                .map(|&[x, y]| point([units.from_q0(x)?, units.from_q0(y)?], 0))
                .collect::<Result<Vec<P3>, _>>()?;
            let cells = match line {
                Some(line) => cells_of(wall, layer, line, cut_list, units)?,
                None => Vec::new(),
            };
            asked.layers.push(asked_layer(layer, &outer, by, &cells));
            let profile = Profile {
                outer,
                holes: Vec::new(),
            };
            let made = cut_piece(engine, &profile, by, &mine, cells)?;
            levels.add(layer, &outline(&corners, units)?, made, units)?;
        }
    }
    Ok(())
}

/// Builds a layered wall with the doors `hosted` in it, on `engine`: each piece of each layer
/// extruded, every opening cut from each piece its cutter reaches.
///
/// # Errors
///
/// The plan's and the openings' refusals, and the engine's own (a door's with its key in front).
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
    let (frame, cut_list) = match cuts(wall, hosted, units, band, families)? {
        None => (None, Vec::new()),
        Some((frame, cuts)) => (Some(frame), cuts),
    };
    // A wall at an angle is asked in its seats; only a wall along an axis has cutters made in its
    // frame, and wraps at inserts (wraps in a wall at an angle are not built yet).
    let line = frame.as_ref().and_then(|f| f.line.as_ref());
    let tools = match &frame {
        Some(frame) if line.is_none() => cut_list
            .iter()
            .map(|cut| frame.cutter(cut))
            .collect::<Result<Vec<_>, _>>()?,
        _ => Vec::new(),
    };
    let reveal = match &frame {
        Some(frame) if line.is_none() => reveal(wall, frame, &cut_list, units)?,
        _ => None,
    };
    let mut asked = asked_of(&cut_list, &tools, line.is_some());
    let mut cutters = Vec::with_capacity(tools.len());
    for (cut, (tool, up)) in cut_list.iter().zip(&tools) {
        let named = |e: EngineError| cut.engine_error(from_engine(e));
        cutters.push(Cutter {
            cut,
            solid: engine.extrude(tool, *up).map_err(named)?,
            reach: plan_box(&tool.outer),
        });
    }
    // Where the wall wraps at inserts, each layer's own cutters at its inserts, set back.
    let own = match (&frame, &reveal) {
        (Some(frame), Some(reveal)) => own_cutters(engine, frame, reveal, &cut_list, &mut asked)?,
        _ => Vec::new(),
    };
    let mut levels = Levels::new();
    add_layers(
        engine,
        wall,
        (&cutters, &own),
        (line, &cut_list),
        &mut asked,
        &mut levels,
        units,
    )?;
    if let (Some(frame), Some(reveal)) = (&frame, &reveal) {
        add_turns(engine, frame, reveal, wall, &mut asked, &mut levels, units)?;
    }
    let (lo, hi) = levels
        .extent
        .ok_or_else(|| fault("EmptyShape", "a wall with no layer of any thickness"))?;
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
    let whole = if let Some(joints) = &wall.joined {
        outline(&joints.outline, units)?
    } else {
        outline(&wall.plan.strip(exterior, interior)?.0, units)?
    };
    let displays = vec![
        Display {
            lod: 0,
            deviation: i64::from(levels.rounded_fine),
            mesh: levels.fine,
            outline: whole.clone(),
            parts: levels.fine_parts,
        },
        Display {
            lod: 1,
            deviation: opening::deviation(&cut_list, units)
                .saturating_add(i64::from(levels.rounded_coarse)),
            mesh: levels.coarse,
            outline: whole,
            parts: levels.coarse_parts,
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
