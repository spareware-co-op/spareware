//! A box hosted in a wall at an angle (piece A83, CON-A4-F3-P1): a box door (`door.single`), or
//! any box this builder makes, handed with its seat (construct-regen's `hosting`,
//! [`crate::seating::SEAT_ARG`]): its plan rectangle laid along its own x (its width along, its
//! thickness across, as a box along x is), each corner put through its seat, so each is the
//! nearest grid step to where the wall's frame puts it, and extruded straight up by its height.
//! The opening its wall cuts for it is worked out from the same frame (the parent's `layered`
//! module), so the box stands in its hole as the grid allows: every corner of both within half a
//! step of its true place.
//!
//! - Displays: both levels the box, each coordinate the nearest millimetre to the engine's grid
//!   point; a level that rounded anything strays by 1 mm.
//! - Bounds: the mesh's reach, rounded outward. Outline: its four corners, each the nearest
//!   millimetre.
//! - Body: what the engine was asked, as for a box along an axis.
//! - Names: a box's roles laid along x, so its faces are named as they are along an axis.
//! - It hosts nothing.
//!
//! Depends on the parent module's units, shapes, naming and errors, and the layered wall's mesh
//! helpers. Interim: as the parent module's.

use super::layered::{Extent, millimetres, side};
use super::{Asked, Extrusion, KernelSolid, NamedFace, Units, carrier, fault, from_engine, name};
use super::{as_steps, op_id};
use construct_families::Seat;
use construct_model::geometry::{Bounds, Built, Display, GeometryError, Recipe};
use construct_params::Length;
use engine_geometry::value::engine::{Engine, Profile};
use engine_geometry::value::grid::P3;

/// The box `shape` laid along x, its plan corners counter-clockwise from above put through
/// `seat`, at its base, in the engine's steps; and the box laid so.
fn corners(
    shape: &Extrusion,
    seat: &Seat,
    units: Units,
) -> Result<(Extrusion, Vec<P3>), GeometryError> {
    let laid = shape.along_x();
    let mut outer = Vec::with_capacity(4);
    for [x, y] in laid.corners() {
        let steps = |mm: i64| {
            Length::checked_from_mm(mm)
                .map(Length::steps)
                .ok_or_else(super::too_big)
        };
        let [px, py] = seat
            .plan([steps(x)?, steps(y)?])
            .ok_or_else(super::too_big)?;
        outer.push(
            P3::new(units.from_q0(px)?, units.from_q0(py)?, 0).map_err(|_| super::too_big())?,
        );
    }
    Ok((laid, outer))
}

/// Builds the box `shape` of `recipe`, seated at `seat`, on `engine`.
///
/// # Errors
///
/// `TooBig` beyond the grid; the engine's own refusals.
pub(super) fn build<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    shape: &Extrusion,
    seat: &Seat,
    units: Units,
) -> Result<Built, GeometryError> {
    let (laid, outer) = corners(shape, seat, units)?;
    let by = [0, 0, units.steps(laid.height)?];
    let asked = Asked {
        extrude: as_steps(&outer),
        by,
        subtract: Vec::new(),
    };
    let outline = outer
        .iter()
        .map(|p| [units.nearest_mm(p.x.get()).0, units.nearest_mm(p.y.get()).0])
        .collect::<Vec<_>>();
    let solid = engine
        .extrude(
            &Profile {
                outer,
                holes: Vec::new(),
            },
            by,
        )
        .map_err(from_engine)?;
    let drawn = engine.mesh(&solid).map_err(from_engine)?;
    let mut extent: Extent = None;
    let (mesh, rounded) = millimetres(&drawn, units, &mut extent)?;
    let (lo, hi) = extent.ok_or_else(|| fault("EmptyShape", "the engine's mesh has no points"))?;
    let op = op_id(recipe)?;
    let mut names = laid
        .roles()
        .map(|role| name(op, role))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort_by(|a, b| a.name.cmp(&b.name));
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation: i64::from(rounded),
            mesh: mesh.clone(),
            outline: outline.clone(),
            parts: Vec::new(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&asked).unwrap_or_default(),
        bounds: Bounds {
            min: lo.map(|s| units.floor_mm(s)),
            max: hi.map(|s| units.ceil_mm(s)),
        },
        names,
        displays,
    })
}

/// The engine's solid of the box `shape` of `recipe`, seated at `seat`, with the plane of each
/// named face, for a drawing to cut (the parent's `Kernel::solid`): its base and top, and each
/// side on the plan edge that sweeps it.
///
/// # Errors
///
/// As [`build`].
pub(super) fn solid<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    shape: &Extrusion,
    seat: &Seat,
    units: Units,
) -> Result<KernelSolid<E::Solid>, GeometryError> {
    let (laid, outer) = corners(shape, seat, units)?;
    let height = units.steps(laid.height)?;
    let plan: Vec<[i64; 2]> = outer.iter().map(|p| [p.x.get(), p.y.get()]).collect();
    let solid = engine
        .extrude(
            &Profile {
                outer,
                holes: Vec::new(),
            },
            [0, 0, height],
        )
        .map_err(from_engine)?;
    let op = op_id(recipe)?;
    let mut faces = vec![
        NamedFace {
            plane: carrier([0, 0, -1], 0)?,
            name: name(op, laid.caps[0])?,
        },
        NamedFace {
            plane: carrier([0, 0, 1], i128::from(height))?,
            name: name(op, laid.caps[1])?,
        },
    ];
    for (i, role) in laid.sides.into_iter().enumerate() {
        let next = i.checked_add(1).and_then(|n| n.checked_rem(plan.len()));
        let (Some(&from), Some(&to)) = (plan.get(i), next.and_then(|n| plan.get(n))) else {
            return Err(super::too_big());
        };
        faces.push(NamedFace {
            plane: side(from, to)?,
            name: name(op, role)?,
        });
    }
    Ok(KernelSolid { solid, faces })
}
