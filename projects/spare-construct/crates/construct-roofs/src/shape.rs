//! Where a roof is: its eaves outline, the height of each of its layer faces, each layer as the
//! solid the builder asks the engine for, its ridge, and its exact areas and volumes, every corner
//! a point of the kernel's grid in the roof's own frame.
//!
//! **The frame.** Plan `x` and `y` are the model's, in grid steps; `z = 0` is the roof's base, its
//! level's elevation plus its offset (its built row holds that height). The roof's bottom face
//! crosses every footprint edge at `z = 0`, and its layers stand on it, the top face (layer 0's)
//! highest, as a floor's are read from the top down.
//!
//! **How the layers follow the slope.** A layer's thickness is measured square to its faces, as a
//! rafter's depth, a board's or a tile's is given and as a section through a sloped roof draws
//! it. So on a slope a face `h` square to the bottom face stands `h * sqrt(1 + s^2)` above it
//! measured upright, `s` the slope; that height is rounded to the grid once for each face, the
//! nearest step, a half up ([`crate::exact`]), and every corner of the face moves up by that one
//! whole number of steps, so two layers that meet share their face bit for bit and every face is
//! exactly parallel to the bottom face, as a layered wall's faces are (construct-regen's
//! `wall_plan`). On a flat roof `h` is the height itself and nothing is rounded.
//!
//! **Flat.** The eaves outline is the footprint with each edge moved out by its overhang (an edge
//! along x or y moved square to itself, its corners where the moved edges meet, every one a grid
//! point); each layer is that outline extruded straight up from its lower face to its upper face.
//!
//! **Mono-pitch and gable.** In the frame of the eaves edge `E` (`a` across the footprint from
//! `E` toward the opposite edge, `b` along `E` from its start), the footprint is `0 <= a <= W` and
//! `0 <= b <= L`. The bottom face is the plane `z = s a` (mono), or `z = s a` up to the ridge and
//! `z = s (W - a)` beyond it (gable), with the eaves, high side and verges cut plumb (upright):
//! - the eaves at `a = -o(E)`, the far side at `a = W + o(far)`, the verges at `b = -o(start)` and
//!   `b = L + o(end)`, each `o` that edge's overhang;
//! - the ridge of a gable at `a = W / 2`, the nearest step a half up, and its height the nearest
//!   step to `s W / 2`; each eaves corner's height the nearest step to the plane's height there.
//!   Those are the only corners rounded, each once; every other corner is one of them moved up by
//!   a face's whole height, so a gable's two slopes meet at its ridge exactly in every face.
//! - Each layer is the solid between its two faces: its section across the roof (a parallelogram,
//!   or a gable's six-sided chevron) at one verge, lofted straight to the same section at the
//!   other ([`Solid::Loft`]).
//!
//! **Exact readings.** Plan area: the eaves outline's, exact. Slope area: the top face as laid, the
//! plan area times `sqrt(1 + s^2)`, the nearest square step once; a flat roof's is its plan area.
//! Each layer's volume is the solid's exactly, from its grid corners: a flat layer's plan area
//! times its height, a sloped layer's section area times its length between the verges. An area or
//! volume that falls between two whole square or cubic steps is refused ([`RoofError::NotExact`]);
//! corners on the hundredth of a millimetre never meet it.

use crate::error::RoofError;
use crate::exact::{along, height, on_grid, round_half_up};
use crate::roof::{EdgeSpec, Form, Roof, on_axis};
use construct_params::{Area, Length, Slope, Volume};
use construct_slabs::{Boundary, Corner, Layers, SlabError};
use construct_solver::Point;
use kit_core::Id;

/// The solid one layer is, as the builder asks the engine for it, in grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Solid {
    /// A flat layer: `profile` (level, anticlockwise from above) extruded by `by`, straight up.
    Extrude {
        /// The profile's corners.
        profile: Vec<[i64; 3]>,
        /// The vector it is extruded along.
        by: [i64; 3],
    },
    /// A sloped layer: the loft from its section at one verge to the same section at the other,
    /// corner `i` of each joined to corner `i` of the other.
    Loft {
        /// The two sections, each in an upright plane across the roof.
        sections: [Vec<[i64; 3]>; 2],
    },
}

/// One layer with a thickness, as built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerSolid {
    /// Its index from the top (0 is the top layer).
    pub layer: usize,
    /// Its solid.
    pub solid: Solid,
    /// Its volume, exactly the solid's.
    pub volume: Volume,
}

/// A gable's ridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ridge {
    /// Where it starts in plan, at one verge, in grid steps.
    pub from: [i64; 2],
    /// Where it ends, at the other verge.
    pub to: [i64; 2],
    /// The height of the top face along it above the base.
    pub height: Length,
}

/// A roof's shape for a set of layers ([`shape`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Its form.
    pub form: Form,
    /// The eaves outline in plan, anticlockwise from above, each corner with the footprint edge
    /// that starts there (the outline's edge beside it).
    pub outline: Vec<(Id, [i64; 2])>,
    /// The upright height of each layer face above the bottom face, top face first and the bottom
    /// face (0) last: `n + 1` of them for `n` layers.
    pub faces: Vec<Length>,
    /// Each layer with a thickness, top first; a layer of none (a membrane) is a face between its
    /// neighbours and has no solid.
    pub solids: Vec<LayerSolid>,
    /// A gable's ridge; none for another form.
    pub ridge: Option<Ridge>,
    /// The area the roof covers in plan, its overhangs included.
    pub plan_area: Area,
    /// The area of its top face as laid on its slopes.
    pub slope_area: Area,
}

/// A sloped roof in the frame of its first eaves edge.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sloped {
    /// The slope it rises at.
    slope: Slope,
    /// The eaves edge's start, its unit direction `t` and the unit direction `n` across the
    /// footprint (to the left of `t`), in plan.
    origin: [i64; 2],
    t: [i64; 2],
    n: [i64; 2],
    /// The verges, along `t`: from `b[0]` to `b[1]`.
    b: [i64; 2],
    /// The bottom face's corners across the roof, `(a, z)`: the eaves and the far side for a mono
    /// roof, the two eaves with the ridge between them for a gable.
    bottom: Vec<(i64, i64)>,
}

/// What a roof's shape is before its layers: its eaves outline and, sloped, its frame. Making one
/// is every check the shape needs, so [`Roof::new`] makes one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    outline: Vec<(Id, [i64; 2])>,
    sloped: Option<Sloped>,
    area: Area,
}

fn add(a: i64, b: i64) -> Result<i64, RoofError> {
    a.checked_add(b).ok_or(RoofError::OutOfRange)
}

fn sub(a: i64, b: i64) -> Result<i64, RoofError> {
    a.checked_sub(b).ok_or(RoofError::OutOfRange)
}

/// The unit direction of the edge from `a` to `b`, along an axis.
fn unit(a: Point, b: Point) -> [i64; 2] {
    [
        b.x.saturating_sub(a.x).signum(),
        b.y.saturating_sub(a.y).signum(),
    ]
}

/// `v` times `k`, on the grid.
fn times(v: [i64; 2], k: i64) -> Result<[i64; 2], RoofError> {
    let [x, y] = v;
    Ok([
        on_grid(i128::from(x).saturating_mul(i128::from(k)))?,
        on_grid(i128::from(y).saturating_mul(i128::from(k)))?,
    ])
}

fn plus(a: [i64; 2], b: [i64; 2]) -> Result<[i64; 2], RoofError> {
    let ([ax, ay], [bx, by]) = (a, b);
    Ok([
        on_grid(i128::from(ax).saturating_add(i128::from(bx)))?,
        on_grid(i128::from(ay).saturating_add(i128::from(by)))?,
    ])
}

/// The item after `at` in a loop of `len`.
fn next(at: usize, len: usize, by: usize) -> usize {
    at.saturating_add(by).checked_rem(len).unwrap_or(0)
}

impl Plan {
    /// The plan of `roof`, as the module header says.
    pub(crate) fn of(roof: &Roof) -> Result<Plan, RoofError> {
        let corners = roof.corners();
        match roof.form() {
            Form::Flat => Self::flat(&corners),
            Form::Mono { eaves } => Self::sloped(&corners, eaves, None),
            Form::Gable {
                eaves: [first, second],
            } => Self::sloped(&corners, first, Some(second)),
        }
    }

    /// A flat roof's plan: the footprint with each edge moved out by its overhang.
    fn flat(corners: &[(Id, Point, EdgeSpec)]) -> Result<Plan, RoofError> {
        let len = corners.len();
        let zero = Length::from_steps(0);
        let overhangs = corners.iter().any(|(_, _, spec)| spec.overhang > zero);
        let edge = |at: usize| -> Option<(&Id, Point, Point, i64)> {
            let (id, a, spec) = corners.get(at)?;
            let (_, b, _) = corners.get(next(at, len, 1))?;
            Some((id, *a, *b, spec.overhang.steps()))
        };
        if overhangs {
            for at in 0..len {
                if let Some((id, a, b, _)) = edge(at)
                    && !on_axis(a, b)
                {
                    return Err(RoofError::OverhangOffAxis { edge: id.clone() });
                }
            }
        }
        let mut outline = Vec::with_capacity(len);
        for at in 0..len {
            let (Some((id, a, b, o)), Some((_, pa, pb, po))) =
                (edge(at), edge(next(at, len, len.saturating_sub(1))))
            else {
                return Err(RoofError::OutOfRange);
            };
            let corner = [a.x, a.y];
            if !overhangs {
                outline.push((id.clone(), corner));
                continue;
            }
            // Each edge moves out to its right (the footprint runs anticlockwise).
            let ([tx, ty], [px, py]) = (unit(a, b), unit(pa, pb));
            let (out, before) = ([ty, tx.saturating_neg()], [py, px.saturating_neg()]);
            let moved = if [tx, ty] == [px, py] {
                if o != po {
                    return Err(RoofError::OverhangFolds { edge: id.clone() });
                }
                plus(corner, times(out, o)?)?
            } else {
                plus(plus(corner, times(before, po)?)?, times(out, o)?)?
            };
            outline.push((id.clone(), moved));
        }
        // Each moved edge runs the way its edge runs, and the outline is a simple loop.
        for (at, (id, [x0, y0])) in outline.iter().enumerate() {
            let (Some((_, [x1, y1])), Some((_, a, b, _))) =
                (outline.get(next(at, len, 1)), edge(at))
            else {
                return Err(RoofError::OutOfRange);
            };
            let [tx, ty] = unit(a, b);
            let run = i128::from(x1.saturating_sub(*x0))
                .saturating_mul(i128::from(tx))
                .saturating_add(i128::from(y1.saturating_sub(*y0)).saturating_mul(i128::from(ty)));
            if run <= 0 {
                return Err(RoofError::OverhangFolds { edge: id.clone() });
            }
        }
        let first = outline.first().map(|(id, _)| id.clone());
        let boundary = outline_boundary(&outline).map_err(|e| match e {
            SlabError::Crossing { first, .. } => RoofError::OverhangFolds { edge: first },
            SlabError::OutOfRange => RoofError::OutOfRange,
            _ => first.map_or(RoofError::OutOfRange, |edge| RoofError::OverhangFolds {
                edge,
            }),
        })?;
        let area = construct_slabs::area(&boundary)?;
        Ok(Plan {
            outline,
            sloped: None,
            area,
        })
    }

    /// A sloped roof's plan in the frame of `eaves`, a gable's far eaves `far`.
    fn sloped(
        corners: &[(Id, Point, EdgeSpec)],
        eaves: &Id,
        far: Option<&Id>,
    ) -> Result<Plan, RoofError> {
        let at = corners
            .iter()
            .position(|(id, _, _)| id == eaves)
            .ok_or(RoofError::OutOfRange)?;
        let corner = |by: usize| corners.get(next(at, 4, by)).ok_or(RoofError::OutOfRange);
        let ((_, start_point, own), (_, end_point, end), (_, _, opposite), (_, last_point, start)) =
            (corner(0)?, corner(1)?, corner(2)?, corner(3)?);
        let slope = own.pitch.ok_or(RoofError::OutOfRange)?.checked(eaves)?;
        let along = unit(*start_point, *end_point);
        let across = [along[1].saturating_neg(), along[0]];
        let dot = |p: Point, d: [i64; 2]| -> Result<i64, RoofError> {
            on_grid(
                i128::from(p.x.saturating_sub(start_point.x))
                    .saturating_mul(i128::from(d[0]))
                    .saturating_add(
                        i128::from(p.y.saturating_sub(start_point.y))
                            .saturating_mul(i128::from(d[1])),
                    ),
            )
        };
        let (long, wide) = (dot(*end_point, along)?, dot(*last_point, across)?);
        if long <= 0 || wide <= 0 {
            return Err(RoofError::OutOfRange);
        }
        let out = |spec: &EdgeSpec| {
            spec.overhang
                .steps()
                .checked_neg()
                .ok_or(RoofError::OutOfRange)
        };
        let verges = [out(start)?, add(long, end.overhang.steps())?];
        let (a_lo, a_hi) = (out(own)?, add(wide, opposite.overhang.steps())?);
        let bottom = match far {
            None => mono_bottom(slope, a_lo, a_hi, eaves)?,
            Some(far) => gable_bottom(slope, wide, (a_lo, out(opposite)?, a_hi), [eaves, far])?,
        };
        let sloped = Sloped {
            slope,
            origin: [start_point.x, start_point.y],
            t: along,
            n: across,
            b: verges,
            bottom,
        };
        // The eaves outline, in the footprint's order: corner k after the eaves edge's start.
        let mut outline = Vec::with_capacity(4);
        for (j, (id, _, _)) in corners.iter().enumerate() {
            let (a, b) = match next(j, 4, 4_usize.saturating_sub(at)) {
                0 => (a_lo, verges[0]),
                1 => (a_lo, verges[1]),
                2 => (a_hi, verges[1]),
                _ => (a_hi, verges[0]),
            };
            outline.push((id.clone(), sloped.plan(a, b)?));
        }
        let area = Area::from_square_steps(
            i128::from(sub(verges[1], verges[0])?)
                .checked_mul(i128::from(sub(a_hi, a_lo)?))
                .ok_or(RoofError::OutOfRange)?,
        );
        Ok(Plan {
            outline,
            sloped: Some(sloped),
            area,
        })
    }
}

/// A mono-pitch roof's bottom face across it, `(a, z)`: its eaves at `a_lo` and its high side at
/// `a_hi`, each height the nearest step to the plane's.
fn mono_bottom(
    slope: Slope,
    a_lo: i64,
    a_hi: i64,
    eaves: &Id,
) -> Result<Vec<(i64, i64)>, RoofError> {
    let (z_lo, z_hi) = (height(slope, a_lo)?, height(slope, a_hi)?);
    if z_hi <= z_lo {
        return Err(RoofError::TooShallow {
            edge: eaves.clone(),
        });
    }
    Ok(vec![(a_lo, z_lo), (a_hi, z_hi)])
}

/// A gable's bottom face across it, `(a, z)`: its first eaves at `a_lo`, its ridge midway over the
/// footprint `wide` steps across, its far eaves at `a_hi`, `far_out` steps past the far edge
/// (negative, as `a_lo` is); each height the nearest step to its plane's.
fn gable_bottom(
    slope: Slope,
    wide: i64,
    (a_lo, far_out, a_hi): (i64, i64, i64),
    [eaves, far]: [&Id; 2],
) -> Result<Vec<(i64, i64)>, RoofError> {
    let ridge = on_grid(round_half_up(i128::from(wide), 2)?)?;
    let twice_run = i128::from(slope.run())
        .checked_mul(2)
        .ok_or(RoofError::OutOfRange)?;
    let rise = i128::from(slope.rise())
        .checked_mul(i128::from(wide))
        .ok_or(RoofError::OutOfRange)?;
    let z_ridge = on_grid(round_half_up(rise, twice_run)?)?;
    let (z_lo, z_hi) = (height(slope, a_lo)?, height(slope, far_out)?);
    if ridge <= 0 || ridge >= wide || z_ridge <= z_lo {
        return Err(RoofError::TooShallow {
            edge: eaves.clone(),
        });
    }
    if z_ridge <= z_hi {
        return Err(RoofError::TooShallow { edge: far.clone() });
    }
    Ok(vec![(a_lo, z_lo), (ridge, z_ridge), (a_hi, z_hi)])
}

impl Sloped {
    /// The plan point `a` across and `b` along the frame.
    fn plan(&self, a: i64, b: i64) -> Result<[i64; 2], RoofError> {
        plus(plus(self.origin, times(self.n, a)?)?, times(self.t, b)?)
    }

    /// The section across the roof at `b` between faces `low` and `high` (upright heights above
    /// the bottom face): the lower face's corners across, then the upper face's back.
    fn section(&self, b: i64, low: i64, high: i64) -> Result<Vec<[i64; 3]>, RoofError> {
        let mut out = Vec::with_capacity(self.bottom.len().saturating_mul(2));
        let lifted = |(a, z): (i64, i64), up: i64| -> Result<[i64; 3], RoofError> {
            let [x, y] = self.plan(a, b)?;
            Ok([x, y, on_grid(i128::from(z).saturating_add(i128::from(up)))?])
        };
        for &corner in &self.bottom {
            out.push(lifted(corner, low)?);
        }
        for &corner in self.bottom.iter().rev() {
            out.push(lifted(corner, high)?);
        }
        Ok(out)
    }

    /// Twice the area of the section between faces `low` and `high`, in `(a, z)`: exact.
    fn twice_section_area(&self, low: i64, high: i64) -> i128 {
        let points: Vec<(i128, i128)> = self
            .bottom
            .iter()
            .map(|&(a, z)| (i128::from(a), i128::from(z).saturating_add(i128::from(low))))
            .chain(self.bottom.iter().rev().map(|&(a, z)| {
                (
                    i128::from(a),
                    i128::from(z).saturating_add(i128::from(high)),
                )
            }))
            .collect();
        let mut twice = 0_i128;
        for (at, &(a0, z0)) in points.iter().enumerate() {
            let (a1, z1) = points
                .get(next(at, points.len(), 1))
                .copied()
                .unwrap_or((a0, z0));
            twice =
                twice.saturating_add(a0.saturating_mul(z1).saturating_sub(a1.saturating_mul(z0)));
        }
        twice
    }
}

/// The eaves outline as a construct-slabs boundary, so its own checks judge it.
fn outline_boundary(outline: &[(Id, [i64; 2])]) -> Result<Boundary, SlabError> {
    let corners: Vec<Corner> = outline
        .iter()
        .map(|(id, [x, y])| Corner {
            id: Some(id.clone()),
            at: Point { x: *x, y: *y },
        })
        .collect();
    Boundary::from_corners(&corners, &[])
}

/// The shape of `roof` with `layers`, as the module header says.
///
/// # Errors
/// [`RoofError::OutOfRange`] for a corner beyond the kernel's grid; [`RoofError::NotExact`] for a
/// volume between two cubic steps.
pub fn shape(roof: &Roof, layers: &Layers) -> Result<Shape, RoofError> {
    let plan = Plan::of(roof)?;
    let total = layers.total();
    // Each face's height above the bottom face, measured square to it, and then upright.
    let mut faces = Vec::with_capacity(layers.depths().len());
    for depth in layers.depths() {
        let square = total.checked_sub(*depth).ok_or(RoofError::OutOfRange)?;
        let upright = match &plan.sloped {
            None => square.steps(),
            Some(sloped) => {
                let up = along(u128::from(square.steps().unsigned_abs()), sloped.slope)?;
                on_grid(i128::try_from(up).map_err(|_| RoofError::OutOfRange)?)?
            }
        };
        faces.push(Length::from_steps(upright));
    }
    let mut solids = Vec::new();
    for (at, pair) in faces.windows(2).enumerate() {
        let [high, low] = pair else {
            continue;
        };
        let (high, low) = (high.steps(), low.steps());
        if high <= low {
            continue;
        }
        let (solid, volume) = match &plan.sloped {
            None => {
                let profile = plan
                    .outline
                    .iter()
                    .map(|(_, [x, y])| [*x, *y, low])
                    .collect();
                let up = sub(high, low)?;
                let volume = plan
                    .area
                    .checked_mul(Length::from_steps(up))
                    .ok_or(RoofError::OutOfRange)?;
                (
                    Solid::Extrude {
                        profile,
                        by: [0, 0, up],
                    },
                    volume,
                )
            }
            Some(sloped) => {
                let sections = [
                    sloped.section(sloped.b[0], low, high)?,
                    sloped.section(sloped.b[1], low, high)?,
                ];
                let length = i128::from(sub(sloped.b[1], sloped.b[0])?);
                let twice = sloped
                    .twice_section_area(low, high)
                    .checked_mul(length)
                    .ok_or(RoofError::OutOfRange)?;
                if twice.checked_rem(2) != Some(0) {
                    return Err(RoofError::NotExact);
                }
                let volume =
                    Volume::from_cubic_steps(twice.checked_div(2).ok_or(RoofError::OutOfRange)?);
                (Solid::Loft { sections }, volume)
            }
        };
        solids.push(LayerSolid {
            layer: at,
            solid,
            volume,
        });
    }
    let top = faces.first().copied().unwrap_or(Length::from_steps(0));
    let (ridge, slope_area) = match &plan.sloped {
        None => (None, plan.area),
        Some(sloped) => {
            let area =
                u128::try_from(plan.area.square_steps()).map_err(|_| RoofError::OutOfRange)?;
            let laid =
                i128::try_from(along(area, sloped.slope)?).map_err(|_| RoofError::OutOfRange)?;
            let ridge = match sloped.bottom.as_slice() {
                [_, (a, z), _] => Some(Ridge {
                    from: sloped.plan(*a, sloped.b[0])?,
                    to: sloped.plan(*a, sloped.b[1])?,
                    height: Length::from_steps(add(*z, top.steps())?),
                }),
                _ => None,
            };
            (ridge, Area::from_square_steps(laid))
        }
    };
    Ok(Shape {
        form: roof.form().clone(),
        outline: plan.outline,
        faces,
        solids,
        ridge,
        plan_area: plan.area,
        slope_area,
    })
}
