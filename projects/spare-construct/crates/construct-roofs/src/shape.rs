//! Where a roof is: its eaves outline, the height of each of its layer faces, each layer as the
//! solid the builder asks the engine for, its ridge, its planes and lines, and its exact areas and
//! volumes, every corner a point of the kernel's grid in the roof's own frame.
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
//! **Mono-pitch and gable** (on a rectangle along the axes). In the frame of the eaves edge `E`
//! (`a` across the footprint from `E` toward the opposite edge, `b` along `E` from its start), the
//! footprint is `0 <= a <= W` and `0 <= b <= L`. The bottom face is the plane `z = s a` (mono), or
//! `z = s a` up to the ridge and `z = s' (W - a)` beyond it (gable, `s'` the far edge's slope),
//! with the eaves, high side and verges cut plumb (upright):
//! - the eaves at `a = -o(E)`, the far side at `a = W + o(far)`, the verges at `b = -o(start)` and
//!   `b = L + o(end)`, each `o` that edge's overhang;
//! - a gable's ridge on each face where its two slopes meet: for a face standing `u` and `u'`
//!   upright above the bottom face on each slope, at `a = (s' W + u' - u) / (s + s')`, the nearest
//!   step a half up, and the nearest step to `s a + u` high (worked exactly, [`crate::solve`]'s
//!   `gable_ridge`); at one slope that is midway, `a = W / 2`, at `s W / 2 + u` on every face;
//!   each eaves corner's height the nearest step to the plane's height there, moved up by the
//!   face's whole height. Those are the only corners rounded, each once, so a gable's two slopes
//!   meet at its ridge exactly in every face.
//! - Each layer is the solid between its two faces: its section across the roof (a parallelogram,
//!   or a gable's six-sided chevron) at one verge, lofted straight to the same section at the
//!   other ([`Solid::Loft`]).
//!
//! **Solved** (every other sloped roof, [`crate::solve`]): the bottom face is cut into flat
//! pieces, each in one sloped edge's plane, every corner on the grid; the slopes rise at one
//! pitch, so a face `h` square to the bottom face stands one height above it on every plane, and
//! each layer is the union of its pieces, each piece moved up to the layer's lower face and
//! extruded straight up to its upper face ([`Solid::Unite`]). Pieces meet side to side, so the
//! union is one solid.
//!
//! **Planes and lines** ([`Plane`], [`Line`]) are of the top face: each plane the edge it rises
//! from and its pieces; each line a ridge, hip, valley, eave or verge with its two ends.
//!
//! **Exact readings.** Plan area: the eaves outline's, exact (a solved roof's: its pieces', which
//! are its outline's). Slope area: the top face as laid, the plan area times `sqrt(1 + s^2)`, the
//! nearest square step once, for a mono-pitch roof or a gable as a whole (as version 1 gives it)
//! and for a solved roof plane by plane, its whole their sum; each plane's own is its plan area's,
//! the nearest square step. A flat roof's is its plan area. Each layer's volume is the solid's
//! exactly, from its grid corners: a flat or solved layer's plan area times its upright height, a
//! sloped layer's section area times its length between the verges. An area or volume that falls
//! between two whole square or cubic steps is refused ([`RoofError::NotExact`]); corners on the
//! hundredth of a millimetre never meet it.

use crate::error::RoofError;
use crate::exact::{along, height, on_grid};
use crate::roof::{EdgeSpec, Form, Roof, on_axis};
use crate::solve::{self, Line, LineKind, Solution, gable_ridge};
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
    /// A solved layer: each profile (a flat piece of its lower face, anticlockwise from above)
    /// extruded by `by`, straight up, and the solids united in their order, each sharing a side
    /// with one before it.
    Unite {
        /// The profiles.
        profiles: Vec<Vec<[i64; 3]>>,
        /// The vector each is extruded along.
        by: [i64; 3],
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

/// One plane of a roof's top face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plane {
    /// The footprint edge it rises from; none for a flat roof's one plane.
    pub edge: Option<Id>,
    /// Its flat pieces, each anticlockwise from above, every corner in grid steps.
    pub pieces: Vec<Vec<[i64; 3]>>,
    /// The area it covers in plan, exact.
    pub plan_area: Area,
    /// Its area as laid: the plan area times `sqrt(1 + s^2)`, the nearest square step.
    pub slope_area: Area,
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
    /// face (0) last: `n + 1` of them for `n` layers (on a gable, on its first slope).
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
    /// The planes of its top face, in the footprint's order.
    pub planes: Vec<Plane>,
    /// The lines of its top face: ridges, hips, valleys, eaves and verges, by kind, then by their
    /// ends.
    pub lines: Vec<Line>,
    /// The height of its top face's highest point above its base.
    pub height: Length,
}

/// A mono-pitch roof or gable in the frame of its first eaves edge.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sloped {
    /// The slope it rises at from its first eaves edge.
    slope: Slope,
    /// A gable's far eaves' slope.
    far: Option<Slope>,
    /// The eaves edge's start, its unit direction `t` and the unit direction `n` across the
    /// footprint (to the left of `t`), in plan.
    origin: [i64; 2],
    t: [i64; 2],
    n: [i64; 2],
    /// The verges, along `t`: from `b[0]` to `b[1]`.
    b: [i64; 2],
    /// The footprint's width across, `W`.
    wide: i64,
    /// The eaves and the far side across, `(a, z)` on the bottom face.
    low: (i64, i64),
    high: (i64, i64),
}

/// What a roof's shape is before its layers: its eaves outline and, sloped, its frame or its
/// solved bottom face. Making one is every check the shape needs, so [`Roof::new`] makes one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    outline: Vec<(Id, [i64; 2])>,
    sloped: Option<Sloped>,
    solved: Option<Solution>,
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
            Form::Solved { eaves } => Self::solved(&corners, eaves),
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
        let area = outline_area(&outline)?;
        Ok(Plan {
            outline,
            sloped: None,
            solved: None,
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
        let far_slope = match far {
            Some(far) => Some(opposite.pitch.ok_or(RoofError::OutOfRange)?.checked(far)?),
            None => None,
        };
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
        let z_lo = height(slope, a_lo)?;
        let z_hi = match far_slope {
            None => height(slope, a_hi)?,
            Some(far_slope) => height(far_slope, out(opposite)?)?,
        };
        let sloped = Sloped {
            slope,
            far: far_slope,
            origin: [start_point.x, start_point.y],
            t: along,
            n: across,
            b: verges,
            wide,
            low: (a_lo, z_lo),
            high: (a_hi, z_hi),
        };
        // The bottom face rises: a mono-pitch roof to its high side, a gable to its ridge.
        match (far, sloped.chevron(0, 0)?.as_slice()) {
            (None, _) if z_hi <= z_lo => {
                return Err(RoofError::TooShallow {
                    edge: eaves.clone(),
                });
            }
            (Some(far), [_, (ridge, z_ridge), _]) => {
                if *ridge <= 0 || *ridge >= wide || *z_ridge <= z_lo {
                    return Err(RoofError::TooShallow {
                        edge: eaves.clone(),
                    });
                }
                if *z_ridge <= z_hi {
                    return Err(RoofError::TooShallow { edge: far.clone() });
                }
            }
            _ => {}
        }
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
            solved: None,
            area,
        })
    }

    /// A solved roof's plan ([`crate::solve`]), its sloped edges `eaves` at one slope.
    fn solved(corners: &[(Id, Point, EdgeSpec)], eaves: &[Id]) -> Result<Plan, RoofError> {
        let first = eaves.first().ok_or(RoofError::OutOfRange)?;
        let slope = corners
            .iter()
            .find(|(id, _, _)| id == first)
            .and_then(|(_, _, spec)| spec.pitch)
            .ok_or(RoofError::OutOfRange)?
            .checked(first)?;
        let solution = solve::solve(corners, slope)?;
        let area = half_area(solution.twice_area)?;
        Ok(Plan {
            outline: solution.outline.clone(),
            sloped: None,
            solved: Some(solution),
            area,
        })
    }
}

/// Half of twice an area: refused when it falls between two square steps.
fn half_area(twice: i128) -> Result<Area, RoofError> {
    if twice.checked_rem(2) != Some(0) {
        return Err(RoofError::NotExact);
    }
    Ok(Area::from_square_steps(
        twice.checked_div(2).ok_or(RoofError::OutOfRange)?,
    ))
}

/// The area of an eaves outline, checked as a construct-slabs boundary.
fn outline_area(outline: &[(Id, [i64; 2])]) -> Result<Area, RoofError> {
    let first = outline.first().map(|(id, _)| id.clone());
    let boundary = outline_boundary(outline).map_err(|e| match e {
        SlabError::Crossing { first, .. } => RoofError::OverhangFolds { edge: first },
        SlabError::OutOfRange => RoofError::OutOfRange,
        _ => first.map_or(RoofError::OutOfRange, |edge| RoofError::OverhangFolds {
            edge,
        }),
    })?;
    Ok(construct_slabs::area(&boundary)?)
}

impl Sloped {
    /// The plan point `a` across and `b` along the frame.
    fn plan(&self, a: i64, b: i64) -> Result<[i64; 2], RoofError> {
        plus(plus(self.origin, times(self.n, a)?)?, times(self.t, b)?)
    }

    /// A face's corners across the roof, `(a, z)`, the face standing `up` steps upright above the
    /// bottom face on the first slope and `far_up` on a gable's far one: the eaves and the far
    /// side for a mono-pitch roof, the two eaves with the ridge between them for a gable.
    fn chevron(&self, up: i64, far_up: i64) -> Result<Vec<(i64, i64)>, RoofError> {
        let (a_lo, z_lo) = self.low;
        let (a_hi, z_hi) = self.high;
        Ok(match self.far {
            None => vec![(a_lo, add(z_lo, up)?), (a_hi, add(z_hi, up)?)],
            Some(far) => {
                let ridge = gable_ridge(self.slope, far, self.wide, up, far_up)?;
                vec![(a_lo, add(z_lo, up)?), ridge, (a_hi, add(z_hi, far_up)?)]
            }
        })
    }

    /// The section across the roof at `b` between the faces `lower` and `upper` ([`chevron`]s):
    /// the lower face's corners across, then the upper face's back.
    fn section(
        &self,
        b: i64,
        lower: &[(i64, i64)],
        upper: &[(i64, i64)],
    ) -> Result<Vec<[i64; 3]>, RoofError> {
        let mut out = Vec::with_capacity(lower.len().saturating_add(upper.len()));
        for &(a, z) in lower.iter().chain(upper.iter().rev()) {
            let [x, y] = self.plan(a, b)?;
            out.push([x, y, z]);
        }
        Ok(out)
    }

    /// The top face's planes and lines, from its [`chevron`] `top`, with the edges the roof rises
    /// from.
    fn planes(
        &self,
        top: &[(i64, i64)],
        eaves: &[Id],
    ) -> Result<(Vec<Plane>, Vec<Line>), RoofError> {
        let [b0, b1] = self.b;
        let point = |(a, z): (i64, i64), b: i64| -> Result<[i64; 3], RoofError> {
            let [x, y] = self.plan(a, b)?;
            Ok([x, y, z])
        };
        let mut planes = Vec::new();
        let mut lines = Vec::new();
        for (k, pair) in top.windows(2).enumerate() {
            let [lo, hi] = pair else {
                continue;
            };
            let piece = vec![
                point(*lo, b0)?,
                point(*lo, b1)?,
                point(*hi, b1)?,
                point(*hi, b0)?,
            ];
            let plan_area = Area::from_square_steps(
                i128::from(sub(b1, b0)?)
                    .checked_mul(i128::from(sub(hi.0, lo.0)?))
                    .ok_or(RoofError::OutOfRange)?,
            );
            let (edge, slope) = if k == 0 {
                (eaves.first().cloned(), self.slope)
            } else {
                (eaves.get(1).cloned(), self.far.unwrap_or(self.slope))
            };
            let laid = along(
                u128::try_from(plan_area.square_steps()).map_err(|_| RoofError::OutOfRange)?,
                slope,
            )?;
            planes.push(Plane {
                edge,
                pieces: vec![piece],
                plan_area,
                slope_area: Area::from_square_steps(
                    i128::try_from(laid).map_err(|_| RoofError::OutOfRange)?,
                ),
            });
            for b in [b0, b1] {
                lines.push(line(LineKind::Verge, point(*lo, b)?, point(*hi, b)?));
            }
        }
        if let (Some(first), Some(last)) = (top.first(), top.last()) {
            lines.push(line(LineKind::Eave, point(*first, b0)?, point(*first, b1)?));
            let far_kind = if self.far.is_some() {
                LineKind::Eave
            } else {
                LineKind::Verge
            };
            lines.push(line(far_kind, point(*last, b0)?, point(*last, b1)?));
        }
        if let [_, ridge, _] = top {
            lines.push(line(
                LineKind::Ridge,
                point(*ridge, b0)?,
                point(*ridge, b1)?,
            ));
        }
        Ok((planes, sorted(lines)))
    }
}

/// Twice the area of a polygon `(a, z)`: exact.
fn twice_area_across(points: &[(i64, i64)]) -> i128 {
    let mut twice = 0_i128;
    for (&(a0, z0), &(a1, z1)) in points.iter().zip(points.iter().cycle().skip(1)) {
        let (a0, z0, a1, z1) = (
            i128::from(a0),
            i128::from(z0),
            i128::from(a1),
            i128::from(z1),
        );
        twice = twice.saturating_add(a0.saturating_mul(z1).saturating_sub(a1.saturating_mul(z0)));
    }
    twice
}

/// A line with its ends, the lower first.
fn line(kind: LineKind, a: [i64; 3], b: [i64; 3]) -> Line {
    let (from, to) = if a <= b { (a, b) } else { (b, a) };
    Line { kind, from, to }
}

/// Lines by kind, then by their ends.
fn sorted(mut lines: Vec<Line>) -> Vec<Line> {
    lines.sort_by_key(|line| (line.kind, line.from, line.to));
    lines.dedup();
    lines
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

/// Each face's upright height above the bottom face on `slope` (`None`: flat), from `layers`.
fn uprights(layers: &Layers, slope: Option<Slope>) -> Result<Vec<Length>, RoofError> {
    let total = layers.total();
    let mut faces = Vec::with_capacity(layers.depths().len());
    for depth in layers.depths() {
        let square = total.checked_sub(*depth).ok_or(RoofError::OutOfRange)?;
        let upright = match slope {
            None => square.steps(),
            Some(slope) => {
                let up = along(u128::from(square.steps().unsigned_abs()), slope)?;
                on_grid(i128::try_from(up).map_err(|_| RoofError::OutOfRange)?)?
            }
        };
        faces.push(Length::from_steps(upright));
    }
    Ok(faces)
}

/// `p` moved up by `z`.
fn lifted(p: [i64; 3], z: i64) -> Result<[i64; 3], RoofError> {
    Ok([p[0], p[1], add(p[2], z)?])
}

/// The shape of `roof` with `layers`, as the module header says.
///
/// # Errors
/// [`RoofError::OutOfRange`] for a corner beyond the kernel's grid; [`RoofError::NotExact`] for a
/// volume between two cubic steps; [`RoofError::TooShallow`] for a gable whose ridge, on a face,
/// leaves the span between its eaves.
#[allow(
    clippy::too_many_lines,
    reason = "every form's layers, planes and readings, worked in the one order the module header gives"
)]
pub fn shape(roof: &Roof, layers: &Layers) -> Result<Shape, RoofError> {
    let plan = Plan::of(roof)?;
    let slope = match (&plan.sloped, &plan.solved) {
        (Some(sloped), _) => Some(sloped.slope),
        (_, Some(solved)) => Some(solved.slope),
        _ => None,
    };
    let faces = uprights(layers, slope)?;
    let far_faces = match plan.sloped.as_ref().and_then(|s| s.far) {
        Some(far) => uprights(layers, Some(far))?,
        None => faces.clone(),
    };
    let eaves = roof.form().eaves();
    let mut solids = Vec::new();
    let pairs = faces.windows(2).zip(far_faces.windows(2));
    for (at, (pair, far_pair)) in pairs.enumerate() {
        let ([high, low], [far_high, far_low]) = (pair, far_pair) else {
            continue;
        };
        let (high, low) = (high.steps(), low.steps());
        if high <= low {
            continue;
        }
        let (solid, volume) = if let Some(sloped) = &plan.sloped {
            let lower = sloped.chevron(low, far_low.steps())?;
            let upper = sloped.chevron(high, far_high.steps())?;
            for chevron in [&lower, &upper] {
                if let [(a_lo, z_lo), (ridge, z_ridge), (a_hi, z_hi)] = chevron.as_slice()
                    && (ridge <= a_lo || ridge >= a_hi || z_ridge <= z_lo || z_ridge <= z_hi)
                {
                    return Err(RoofError::TooShallow {
                        edge: eaves.first().cloned().ok_or(RoofError::OutOfRange)?,
                    });
                }
            }
            let sections = [
                sloped.section(sloped.b[0], &lower, &upper)?,
                sloped.section(sloped.b[1], &lower, &upper)?,
            ];
            let across: Vec<(i64, i64)> = lower.iter().chain(upper.iter().rev()).copied().collect();
            let length = i128::from(sub(sloped.b[1], sloped.b[0])?);
            let twice = twice_area_across(&across)
                .checked_mul(length)
                .ok_or(RoofError::OutOfRange)?;
            if twice.checked_rem(2) != Some(0) {
                return Err(RoofError::NotExact);
            }
            let volume =
                Volume::from_cubic_steps(twice.checked_div(2).ok_or(RoofError::OutOfRange)?);
            (Solid::Loft { sections }, volume)
        } else {
            let up = sub(high, low)?;
            let volume = plan
                .area
                .checked_mul(Length::from_steps(up))
                .ok_or(RoofError::OutOfRange)?;
            let solid = match &plan.solved {
                None => Solid::Extrude {
                    profile: plan
                        .outline
                        .iter()
                        .map(|(_, [x, y])| [*x, *y, low])
                        .collect(),
                    by: [0, 0, up],
                },
                Some(solved) => {
                    let mut profiles = Vec::with_capacity(solved.pieces.len());
                    for (_, piece) in &solved.pieces {
                        profiles.push(
                            piece
                                .iter()
                                .map(|p| lifted(*p, low))
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                    }
                    Solid::Unite {
                        profiles,
                        by: [0, 0, up],
                    }
                }
            };
            (solid, volume)
        };
        solids.push(LayerSolid {
            layer: at,
            solid,
            volume,
        });
    }
    let top = faces.first().copied().unwrap_or(Length::from_steps(0));
    let far_top = far_faces.first().copied().unwrap_or(top);
    let (ridge, slope_area, planes, lines, peak) = if let Some(sloped) = &plan.sloped {
        let area = u128::try_from(plan.area.square_steps()).map_err(|_| RoofError::OutOfRange)?;
        let laid = i128::try_from(along(area, sloped.slope)?).map_err(|_| RoofError::OutOfRange)?;
        let chevron = sloped.chevron(top.steps(), far_top.steps())?;
        let ridge = match chevron.as_slice() {
            [_, (a, z), _] => Some(Ridge {
                from: sloped.plan(*a, sloped.b[0])?,
                to: sloped.plan(*a, sloped.b[1])?,
                height: Length::from_steps(*z),
            }),
            _ => None,
        };
        let peak = chevron.iter().map(|(_, z)| *z).max().unwrap_or(0);
        let (planes, lines) = sloped.planes(&chevron, &eaves)?;
        // An uneven gable's slopes are laid at two pitches: its whole is its planes' sum.
        let laid = match sloped.far {
            Some(far) if far != sloped.slope => {
                let mut sum = Area::from_square_steps(0);
                for plane in &planes {
                    sum = sum
                        .checked_add(plane.slope_area)
                        .ok_or(RoofError::OutOfRange)?;
                }
                sum
            }
            _ => Area::from_square_steps(laid),
        };
        (ridge, laid, planes, lines, peak)
    } else if let Some(solved) = &plan.solved {
        let (planes, lines, peak) = solved_planes(roof, solved, top.steps())?;
        let mut laid = Area::from_square_steps(0);
        for plane in &planes {
            laid = laid
                .checked_add(plane.slope_area)
                .ok_or(RoofError::OutOfRange)?;
        }
        (None, laid, planes, lines, peak)
    } else {
        let z = top.steps();
        let piece: Vec<[i64; 3]> = plan.outline.iter().map(|(_, [x, y])| [*x, *y, z]).collect();
        let mut lines = Vec::with_capacity(piece.len());
        for (a, b) in piece.iter().zip(piece.iter().cycle().skip(1)) {
            lines.push(line(LineKind::Eave, *a, *b));
        }
        let plane = Plane {
            edge: None,
            pieces: vec![piece],
            plan_area: plan.area,
            slope_area: plan.area,
        };
        (None, plan.area, vec![plane], sorted(lines), z)
    };
    Ok(Shape {
        form: roof.form().clone(),
        outline: plan.outline,
        faces,
        solids,
        ridge,
        plan_area: plan.area,
        slope_area,
        planes,
        lines,
        height: Length::from_steps(peak),
    })
}

/// A solved roof's top face: its planes in the footprint's order, its lines, and its highest
/// point, each the bottom face's moved up by `top`.
fn solved_planes(
    roof: &Roof,
    solved: &Solution,
    top: i64,
) -> Result<(Vec<Plane>, Vec<Line>, i64), RoofError> {
    let mut planes = Vec::new();
    let mut peak = i64::MIN;
    for (at, (id, _)) in roof.edges().iter().enumerate() {
        let mut pieces = Vec::new();
        let mut twice = 0_i128;
        for (edge, piece) in &solved.pieces {
            if *edge != at {
                continue;
            }
            let lifted_piece = piece
                .iter()
                .map(|p| lifted(*p, top))
                .collect::<Result<Vec<_>, _>>()?;
            for p in &lifted_piece {
                peak = peak.max(p[2]);
            }
            twice = twice
                .checked_add(solve_twice_area(&lifted_piece)?)
                .ok_or(RoofError::OutOfRange)?;
            pieces.push(lifted_piece);
        }
        if pieces.is_empty() {
            continue;
        }
        let plan_area = half_area(twice)?;
        let laid = along(
            u128::try_from(plan_area.square_steps()).map_err(|_| RoofError::OutOfRange)?,
            solved.slope,
        )?;
        planes.push(Plane {
            edge: Some(id.clone()),
            pieces,
            plan_area,
            slope_area: Area::from_square_steps(
                i128::try_from(laid).map_err(|_| RoofError::OutOfRange)?,
            ),
        });
    }
    let mut lines = Vec::with_capacity(solved.lines.len());
    for l in &solved.lines {
        lines.push(line(l.kind, lifted(l.from, top)?, lifted(l.to, top)?));
    }
    Ok((planes, sorted(lines), peak.max(top)))
}

/// Twice a piece's plan area.
fn solve_twice_area(piece: &[[i64; 3]]) -> Result<i128, RoofError> {
    let mut sum = 0_i128;
    for (a, b) in piece.iter().zip(piece.iter().cycle().skip(1)) {
        let term = i128::from(a[0])
            .checked_mul(i128::from(b[1]))
            .and_then(|t| t.checked_sub(i128::from(b[0]).checked_mul(i128::from(a[1]))?))
            .ok_or(RoofError::OutOfRange)?;
        sum = sum.checked_add(term).ok_or(RoofError::OutOfRange)?;
    }
    Ok(sum)
}
