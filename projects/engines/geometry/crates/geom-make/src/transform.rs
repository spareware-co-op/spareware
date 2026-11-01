//! The Transform family of chapter 10.2: move, rotate, mirror and scale a body, its entities keeping
//! their names, its geometry changing kind only as chapter 7's typed transforms say.
//!
//! What it does: each function turns its input into a `geom_math::TypedTransform` and hands it to
//! [`moved_copy`], which copies every row of the body with its points, curves, surfaces and face
//! boxes moved, publishes the copy under the same names, checks it and records each entity as
//! modified from itself (20.2: a one-to-one change keeps the name). [`Motion`], in the result, says
//! which of the four it was, for the attribute event `Transformed` (8.4).
//!
//! Guarantees:
//!
//! - **Exact where the transform is.** A move by whole steps, quarter turns, mirrors across grid
//!   planes and whole-number uniform scales map grid points to grid points and keep planes given
//!   by integers: an Exact body stays Exact.
//! - **Rounded once, and said so.** Any other rotation or scale rounds each vertex to the grid once
//!   (at most √3/2 of a step), and moves curves and surfaces by their own typed transforms, each
//!   with a certified deviation. The result's bound is the input's, stretched by the most the
//!   transform can stretch a length, plus one step if any vertex rounded, plus the largest
//!   deviation, rounded up.
//! - **The class never rises.** The result keeps the input's class or a less exact one: an
//!   Approximate body stays Approximate whatever its bound does (principle 3).
//! - **Accepted, or refused.** A result less exact than the input is returned only when the caller
//!   accepts it ([`Moved::accept`]): a class at least as loose and a ceiling at least the result's
//!   bound; otherwise [`MakeDetail::NotAccepted`] names the class the result would have.
//! - **Kinds change only where the checker can follow.** A transform that does not keep circles
//!   turns them into ellipses and analytic surfaces into affine ones, which the checker cannot
//!   certify yet: on a body with curved geometry it is [`MakeDetail::KindNotCertified`] (G177).
//! - **Mirrors keep the outside outside.** A mirror reverses orientation, so every face's sense
//!   and every loop's direction is reversed with it.
//!
//! Method: chapter 7's typed transforms and their kind changes
//! (`projects/engines/geometry/architecture/07-geometry-types.md`), applied as `geom-math`,
//! `geom-curve` and `geom-surface` define them; chapter 20.2's naming rule for one-to-one changes.
//!
//! Cost: linear in the body's rows, one meter unit each, plus each curve's and surface's transform
//! and the checker. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': t the transform, a the arenas, b the builder, f, l, n, e, r, s and v rows"
)]

use alloc::vec::Vec;

use geom_curve::{CurveError, CurveKind, ParamCurve};
use geom_math::{
    Axis, Box3, GridTransform, RationalTransform, Rotation, Scale, TransformKind, TypedTransform,
};
use geom_num::interval::Interval;
use geom_num::{Exactness, GridStep, NumError, P3, Steps};
use geom_op::history::{Origin, Record};
use geom_op::{MakeDetail, NameId, NameSet, OpContext, OpError, OpErrorDetail, OpOutput, TopoName};
use geom_run::WorkMeter;
use geom_surface::{ParamSurface, SurfaceKind};
use geom_topo::ix::Arena;
use geom_topo::order::{NameKey, Names};
use geom_topo::{
    Body, Builder, Edge, Face, Fin, Ix, Loop, Region, Sense, Shell, TopoError, Vertex,
};

use crate::build::{Made, Motion, PHASE, charge_rows, fail, finish, unbuilt};

/// The most a rotation held in doubles can stretch a length: 2^−34 (`geom_num::ladder::ANGULAR`,
/// the `ε_ang` such a rotation keeps lengths within), as 1 + 2^−34 written over 2^34.
const RIGID: (u64, u64) = ((1 << 34) + 1, 1 << 34);

/// The denominator of the stretch of a general transform held in doubles.
const STRETCH_DEN: f64 = 1024.0;

/// A body to transform, with its exactness record and the least exact result the caller accepts.
#[derive(Clone, Copy, Debug)]
pub struct Moved<'b> {
    /// The body, published in name order.
    pub body: &'b Body,
    /// Its exactness record.
    pub exactness: Exactness,
    /// The class and bound the caller accepts for a result less exact than the body; `None`
    /// accepts none. An Exact body's record holds no grid step, so a rounded result counts its
    /// bound in the step given here.
    pub accept: Option<Exactness>,
}

/// The body moved by `offset` steps.
///
/// # Errors
///
/// [`MakeDetail::BodyNotNamed`]; the numbers layer's `OutOfRange` for an offset or an image past
/// the grid's range; the meter's stop; a defect.
pub fn move_by(
    ctx: &OpContext<'_>,
    input: Moved<'_>,
    offset: [i64; 3],
) -> Result<OpOutput<Made>, OpError> {
    let t = GridTransform::translation(offset).map_err(|e| OpError::new(ctx.path().clone(), e))?;
    run(ctx, input, TypedTransform::Exact(t), Motion::Move)
}

/// The body rotated by `rotation` about the line through `center` along `axis`.
///
/// # Errors
///
/// As [`move_by`]; [`MakeDetail::NotAccepted`] for a rotation other than quarter turns of an Exact
/// body, unless the caller accepts the result's bound.
pub fn rotate(
    ctx: &OpContext<'_>,
    input: Moved<'_>,
    axis: Axis,
    rotation: Rotation,
    center: P3,
) -> Result<OpOutput<Made>, OpError> {
    let turn = TypedTransform::rotation(axis, rotation)
        .map_err(|e| OpError::new(ctx.path().clone(), e))?;
    let t = about(ctx, center, turn)?;
    run(ctx, input, t, Motion::Rotate)
}

/// The body mirrored across the plane through `at` perpendicular to `axis`.
///
/// # Errors
///
/// As [`move_by`].
pub fn mirror(
    ctx: &OpContext<'_>,
    input: Moved<'_>,
    axis: Axis,
    at: P3,
) -> Result<OpOutput<Made>, OpError> {
    let t = about(ctx, at, TypedTransform::Exact(GridTransform::mirror(axis)))?;
    run(ctx, input, t, Motion::Mirror)
}

/// The body scaled by `factors` along x, y and z, about `center`.
///
/// # Errors
///
/// As [`move_by`] (a factor is never zero: [`Scale`] refuses it);
/// [`MakeDetail::KindNotCertified`] for factors that are not all equal on a body with curved
/// geometry; [`MakeDetail::NotAccepted`] for a result less exact than the body that the caller
/// does not accept.
pub fn scale(
    ctx: &OpContext<'_>,
    input: Moved<'_>,
    factors: [Scale; 3],
    center: P3,
) -> Result<OpOutput<Made>, OpError> {
    let diagonal = diagonal(factors).map_err(|e| OpError::new(ctx.path().clone(), e))?;
    let t = about(ctx, center, TypedTransform::from(diagonal))?;
    run(ctx, input, t, Motion::Scale)
}

/// The transform scaling x, y and z by `factors`: each numerator over the product of the others'
/// denominators, one denominator for the three.
fn diagonal(factors: [Scale; 3]) -> Result<RationalTransform, NumError> {
    let [sx, sy, sz] = factors;
    let too_big = NumError::OutOfRange { value: i128::MAX };
    let den = sx
        .den()
        .checked_mul(sy.den())
        .and_then(|d| d.checked_mul(sz.den()))
        .and_then(|d| i64::try_from(d).ok())
        .ok_or(too_big)?;
    let over = |s: Scale| -> Result<i64, NumError> {
        i128::from(s.num())
            .checked_mul(i128::from(den))
            .and_then(|n| n.checked_div(i128::from(s.den())))
            .and_then(|n| i64::try_from(n).ok())
            .ok_or(too_big)
    };
    let m = [[over(sx)?, 0, 0], [0, over(sy)?, 0], [0, 0, over(sz)?]];
    RationalTransform::new(m, [0; 3], den)
}

/// `t` about `center`: moved there from the origin, applied, and moved back.
fn about(ctx: &OpContext<'_>, center: P3, t: TypedTransform) -> Result<TypedTransform, OpError> {
    let c = [center.x.get(), center.y.get(), center.z.get()];
    let num = |e| OpError::new(ctx.path().clone(), e);
    let neg = c.map(i64::wrapping_neg);
    let to = TypedTransform::Exact(GridTransform::translation(neg).map_err(num)?);
    let back = TypedTransform::Exact(GridTransform::translation(c).map_err(num)?);
    let composed = t
        .compose(&to)
        .and_then(|inner| back.compose(&inner))
        // A defect: a grid point moved to the origin and back stays within every transform's
        // bound, at most 2^103 for a rational's translation against its 2^126.
        .map_err(|_| fail(ctx, MakeDetail::BuildFailed))?;
    Ok(composed.simplified())
}

/// Whether `body` has a face on anything but a plane, or an edge on anything but a line.
fn curved(body: &Body) -> bool {
    let a = body.arenas();
    let surfaces = (0..a.rows(Arena::Face))
        .filter_map(Ix::new)
        .filter_map(|f| a.faces.surface.get(f).and_then(|s| a.surfaces.get(*s)));
    let curves = (0..a.rows(Arena::Edge))
        .filter_map(Ix::new)
        .filter_map(|e| a.edges.curve.get(e).and_then(|c| a.curves.get(*c)));
    surfaces
        .into_iter()
        .any(|s| !matches!(s, SurfaceKind::Plane(_)))
        || curves.into_iter().any(|c| !matches!(c, CurveKind::Line(_)))
}

/// At least the most `t` stretches any length, as a numerator and a denominator.
///
/// A length-keeping transform held exactly stretches nothing; one held in doubles keeps lengths
/// within `ε_ang` ([`RIGID`]). Anything else is bounded by the larger of its linear part's largest
/// absolute row sum and column sum: the spectral norm is at most the geometric mean of the two
/// (Golub and Van Loan, Matrix Computations, 4th ed., 2.3.3), and on a diagonal matrix they are
/// its largest entry exactly. A matrix in doubles sums outward, in intervals, and rounds up to a
/// multiple of 1/[`STRETCH_DEN`].
fn stretch_of(t: &TypedTransform) -> Result<(u64, u64), NumError> {
    let keeps = t.classify()?.keeps_lengths();
    match t {
        TypedTransform::Exact(_) | TypedTransform::Rotation(_) => Ok((1, 1)),
        TypedTransform::Rational(_) if keeps => Ok((1, 1)),
        TypedTransform::Certified(_) if keeps => Ok(RIGID),
        TypedTransform::Rational(r) => {
            let m = r
                .matrix()
                .map(|row| row.map(|v| u128::from(v.unsigned_abs())));
            let [[a, b, c], [d, e, f], [g, h, i]] = m;
            let most = [
                [a, b, c],
                [d, e, f],
                [g, h, i],
                [a, d, g],
                [b, e, h],
                [c, f, i],
            ]
            .into_iter()
            .map(|[x, y, z]| x.saturating_add(y).saturating_add(z))
            .fold(0, u128::max);
            let too_big = NumError::OutOfRange { value: i128::MAX };
            let num = u64::try_from(most).map_err(|_| too_big)?;
            Ok((num, r.denominator().unsigned_abs()))
        }
        TypedTransform::Certified(c) => {
            let [[a, b, cc, _], [d, e, f, _], [g, h, i, _]] = c.rows();
            let mut most: f64 = 0.0;
            for [x, y, z] in [
                [a, b, cc],
                [d, e, f],
                [g, h, i],
                [a, d, g],
                [b, e, h],
                [cc, f, i],
            ] {
                let sum = Interval::point(x.abs())?
                    .add(Interval::point(y.abs())?)?
                    .add(Interval::point(z.abs())?)?;
                most = geom_curve::tight::larger(most, sum.hi());
            }
            let scaled = geom_num::math::ceil(most * STRETCH_DEN)?.get();
            if !(0.0..9.0e15).contains(&scaled) {
                return Err(NumError::OutOfRange { value: i128::MAX });
            }
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "scaled is a whole number from 0 to 9e15, checked just above; STRETCH_DEN is 1024"
            )]
            Ok((scaled as u64, STRETCH_DEN as u64))
        }
    }
}

/// Transforms `input` by `t`, the `motion` named, keeping every entity's name.
fn run(
    ctx: &OpContext<'_>,
    input: Moved<'_>,
    t: TypedTransform,
    motion: Motion,
) -> Result<OpOutput<Made>, OpError> {
    let names = input
        .body
        .names()
        .ok_or_else(|| fail(ctx, MakeDetail::BodyNotNamed))?
        .clone();
    let keep = |name: NameId| -> Option<Renamed> {
        Some((
            name,
            Origin::Modified {
                from: NameSet::new([name]).ok()?,
            },
            None,
        ))
    };
    ctx.run(|meter| {
        let copy = moved_copy(ctx, meter, input, &t, &names, &keep)?;
        settle(ctx, copy, Some(motion))
    })
}

/// An entity's new name, its origin, and the name itself when the copy made it (a transform keeps
/// its input's name and makes none).
pub(crate) type Renamed = (NameId, Origin, Option<TopoName>);

/// A moved copy before its check: the body, its records, the names it made, its exactness, and,
/// when the transform rounded a corner or moved curved geometry by a deviation, the piece that
/// widens what the checker certifies for it.
pub(crate) struct Copied {
    body: Body,
    records: Vec<Record>,
    names: Vec<TopoName>,
    exactness: Exactness,
    awaits: Option<&'static str>,
}

/// Checks `copy` and returns it: a checker failure is a defect when the transform was exact, and
/// [`MakeDetail::NotCertifiedYet`] when it rounded, since the checker certifies exact data only
/// where groups 2 and 3 follow it today.
pub(crate) fn settle(
    ctx: &OpContext<'_>,
    copy: Copied,
    motion: Option<Motion>,
) -> Result<OpOutput<Made>, OpError> {
    finish(
        ctx,
        copy.body,
        copy.records,
        copy.names,
        motion,
        copy.exactness,
    )
    .map_err(|e| match (e.detail(), copy.awaits) {
        (OpErrorDetail::Make(MakeDetail::ResultInvalid), Some(piece)) => {
            fail(ctx, MakeDetail::NotCertifiedYet { piece }).with_message(e.message())
        }
        _ => e,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass copies every arena's rows with their moved geometry; split, each piece would carry the same seven arguments"
)]
/// A copy of `input` moved by `t`, each entity renamed and recorded by `rename` (which gives the
/// output's name, its origin and any name it made from the input's name), and the copy's
/// exactness, accepted.
pub(crate) fn moved_copy(
    ctx: &OpContext<'_>,
    meter: &mut WorkMeter<'_>,
    input: Moved<'_>,
    t: &TypedTransform,
    names: &Names,
    rename: &dyn Fn(NameId) -> Option<Renamed>,
) -> Result<Copied, OpError> {
    let defect = || fail(ctx, MakeDetail::BuildFailed);
    let num = |e| OpError::new(ctx.path().clone(), e);
    let not_moved = |meter: &WorkMeter<'_>, e: CurveError| match e {
        CurveError::Stopped(stop) => ctx.stopped(meter, stop, PHASE),
        _ => fail(ctx, MakeDetail::KindNotCertified),
    };
    let class = t.classify().map_err(num)?;
    if matches!(class.kind, TransformKind::Degenerate) {
        return Err(fail(ctx, MakeDetail::SingularTransform));
    }
    if !class.keeps_circles() && curved(input.body) {
        return Err(fail(ctx, MakeDetail::KindNotCertified));
    }
    let mirror = class.mirror;
    let stretch = stretch_of(t).map_err(num)?;
    let a = input.body.arenas();
    let rows: u64 = [
        Arena::Region,
        Arena::Shell,
        Arena::Face,
        Arena::Loop,
        Arena::Fin,
        Arena::Edge,
        Arena::Vertex,
    ]
    .into_iter()
    .try_fold(0u64, |n, k| n.checked_add(u64::from(a.rows(k))))
    .ok_or_else(defect)?;
    charge_rows(ctx, meter, rows)?;
    let mut b = Builder::new();
    let out = &mut b.arenas;
    // Points: rounded once each.
    let mut rounded = false;
    let mut points = Vec::new();
    for p in every(a.rows(Arena::Vertex)).filter_map(|v| a.vertices.point.get(v)) {
        let image = t
            .apply(*a.points.get(*p).ok_or_else(defect)?)
            .map_err(num)?;
        rounded |= !image.is_exact();
        points.push(image.value());
    }
    // Curves and surfaces: by their own typed transforms, each with its deviation.
    let mut deviation: f64 = 0.0;
    let mut curves = Vec::new();
    for e in every(a.rows(Arena::Edge)) {
        let c = a
            .edges
            .curve
            .get(e)
            .and_then(|c| a.curves.get(*c))
            .ok_or_else(defect)?;
        let moved = c.transformed(t, meter).map_err(|e| not_moved(meter, e))?;
        deviation = geom_curve::tight::larger(deviation, moved.deviation);
        curves.push(moved.value);
    }
    let mut surfaces = Vec::new();
    for f in every(a.rows(Arena::Face)) {
        let s = a
            .faces
            .surface
            .get(f)
            .and_then(|s| a.surfaces.get(*s))
            .ok_or_else(defect)?;
        let moved = s.transformed(t, meter).map_err(|e| not_moved(meter, e))?;
        deviation = geom_curve::tight::larger(deviation, moved.deviation);
        surfaces.push(moved.value);
    }
    let mut bounds = Vec::new();
    for f in every(a.rows(Arena::Face)) {
        let bound = *a.faces.bound.get(f).ok_or_else(defect)?;
        bounds.push(moved_box(t, bound).map_err(num)?);
    }
    let exactness = record(ctx, input, stretch, rounded, deviation)?;
    let awaits =
        (rounded || deviation > 0.0).then_some(if curved(input.body) { "G177" } else { "G134" });
    // Rows: copied, each pointing at its moved geometry; a mirror reverses faces and loops.
    let flip = |s: Sense| match s {
        Sense::Forward => Sense::Reversed,
        Sense::Reversed => Sense::Forward,
    };
    let regions: Vec<Region> = every(a.rows(Arena::Region))
        .map(|r| {
            Some(Region {
                kind: *a.regions.kind.get(r)?,
                first_shell: *a.regions.first_shell.get(r)?,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(defect)?;
    let shells: Vec<Shell> = every(a.rows(Arena::Shell))
        .map(|s| {
            Some(Shell {
                region: *a.shells.region.get(s)?,
                next: *a.shells.next.get(s)?,
                first_face: *a.shells.first_face.get(s)?,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(defect)?;
    let faces: Vec<Face> = every(a.rows(Arena::Face))
        .enumerate()
        .map(|(i, f)| {
            let sense = *a.faces.sense.get(f)?;
            Some(Face {
                shell: *a.faces.shell.get(f)?,
                next: *a.faces.next.get(f)?,
                surface: Ix::new(u32::try_from(i).ok()?)?,
                sense: if mirror { flip(sense) } else { sense },
                first_loop: *a.faces.first_loop.get(f)?,
                bound: *bounds.get(i)?,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(defect)?;
    let loops: Vec<Loop> = every(a.rows(Arena::Loop))
        .map(|l| {
            Some(Loop {
                face: *a.loops.face.get(l)?,
                next: *a.loops.next.get(l)?,
                first_fin: *a.loops.first_fin.get(l)?,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(defect)?;
    let fins: Vec<Fin> = every(a.rows(Arena::Fin))
        .map(|n| {
            let (next, prev) = (*a.fins.next.get(n)?, *a.fins.prev.get(n)?);
            let sense = *a.fins.sense.get(n)?;
            Some(Fin {
                loop_: *a.fins.loop_.get(n)?,
                next: if mirror { prev } else { next },
                prev: if mirror { next } else { prev },
                edge: *a.fins.edge.get(n)?,
                sense: if mirror { flip(sense) } else { sense },
                radial: *a.fins.radial.get(n)?,
                pcurve: *a.fins.pcurve.get(n)?,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(defect)?;
    let edges: Vec<Edge> = every(a.rows(Arena::Edge))
        .enumerate()
        .map(|(i, e)| {
            Some(Edge {
                first_fin: *a.edges.first_fin.get(e)?,
                start: *a.edges.start.get(e)?,
                end: *a.edges.end.get(e)?,
                curve: Ix::new(u32::try_from(i).ok()?)?,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(defect)?;
    let vertices: Vec<Vertex> = (0..points.len())
        .map(|i| {
            Some(Vertex {
                point: Ix::new(u32::try_from(i).ok()?)?,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(defect)?;
    let pcurves: Vec<CurveKind<2>> = every(a.rows(Arena::Pcurve))
        .filter_map(|p| a.pcurves.get(p).cloned())
        .collect();
    let fill = || -> Result<(), TopoError> {
        out.regions.extend(&regions)?;
        out.shells.extend(&shells)?;
        out.faces.extend(&faces)?;
        out.loops.extend(&loops)?;
        out.fins.extend(&fins)?;
        out.edges.extend(&edges)?;
        out.vertices.extend(&vertices)?;
        out.surfaces.extend(surfaces)?;
        out.curves.extend(curves)?;
        out.pcurves.extend(pcurves)?;
        out.points.extend(points)
    };
    fill().map_err(|e| unbuilt(ctx, meter, e))?;
    // Names: each renamed, and its record.
    let mut records = Vec::new();
    let mut made = Vec::new();
    let mut rekey = |keys: &[NameKey]| -> Option<Vec<NameKey>> {
        keys.iter()
            .map(|k| {
                let (name, origin, node) = rename(NameId::from_bytes(k.0))?;
                made.extend(node);
                records.push(Record {
                    output: name,
                    origin,
                });
                Some(NameKey(name.to_bytes()))
            })
            .collect()
    };
    let renamed = Names {
        regions: rekey(&names.regions).ok_or_else(defect)?,
        faces: rekey(&names.faces).ok_or_else(defect)?,
        edges: rekey(&names.edges).ok_or_else(defect)?,
        vertices: rekey(&names.vertices).ok_or_else(defect)?,
    };
    let body = b
        .publish_named(&renamed, input.body.kind(), input.body.outside(), meter)
        .map_err(|e| unbuilt(ctx, meter, e))?;
    Ok(Copied {
        body,
        records,
        names: made,
        exactness,
        awaits,
    })
}

/// Every row of an arena of `n` rows.
fn every<K>(n: u32) -> impl Iterator<Item = Ix<K>> {
    (0..n).filter_map(Ix::new)
}

/// The box holding `bound`'s image under `t`: its eight corners moved, and one step wider each way
/// when a corner rounds.
fn moved_box(t: &TypedTransform, bound: Box3) -> Result<Box3, NumError> {
    let (lo, hi) = (bound.min(), bound.max());
    let mut min = [i64::MAX; 3];
    let mut max = [i64::MIN; 3];
    let mut widen = 0i64;
    for x in [lo.x, hi.x] {
        for y in [lo.y, hi.y] {
            for z in [lo.z, hi.z] {
                let image = t.apply(P3 { x, y, z })?;
                if !image.is_exact() {
                    widen = 1;
                }
                let p = image.value();
                for ((low, high), c) in min.iter_mut().zip(&mut max).zip([p.x, p.y, p.z]) {
                    *low = (*low).min(c.get());
                    *high = (*high).max(c.get());
                }
            }
        }
    }
    let [ax, ay, az] = min.map(|v| v.saturating_sub(widen));
    let [bx, by, bz] = max.map(|v| v.saturating_add(widen));
    Box3::new(P3::new(ax, ay, az)?, P3::new(bx, by, bz)?)
}

/// The record of the moved body, and whether the caller accepts it: the input's bound stretched by
/// `stretch`, plus one step if a vertex `rounded`, plus `deviation` rounded up.
fn record(
    ctx: &OpContext<'_>,
    input: Moved<'_>,
    stretch: (u64, u64),
    rounded: bool,
    deviation: f64,
) -> Result<Exactness, OpError> {
    let defect = || fail(ctx, MakeDetail::BuildFailed);
    let (old, step) = match input.exactness.bound() {
        Some((b, s)) => (b.get(), Some(s)),
        None => (0, None),
    };
    let (num, den) = stretch;
    let stretched = u128::from(old)
        .checked_mul(u128::from(num))
        .and_then(|x| x.checked_add(u128::from(den).checked_sub(1)?))
        .and_then(|x| x.checked_div(u128::from(den)))
        .ok_or_else(defect)?;
    if !(0.0..9.0e15).contains(&deviation) {
        return Err(fail(ctx, MakeDetail::KindNotCertified));
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "the deviation is not negative and below 2^53, checked just before: its whole part is a u64 the double holds exactly"
    )]
    let (whole, back) = {
        let whole = deviation as u64;
        (whole, whole as f64)
    };
    // Rounded up: one more where a fraction remains.
    let dev = whole
        .checked_add(u64::from(back < deviation))
        .ok_or_else(defect)?;
    let bound = stretched
        .checked_add(u128::from(rounded))
        .and_then(|x| x.checked_add(u128::from(dev)))
        .and_then(|x| u64::try_from(x).ok())
        .ok_or_else(defect)?;
    if bound == 0 && step.is_none() {
        return Ok(Exactness::Exact);
    }
    let would_step = step
        .or(input.accept.and_then(Exactness::bound).map(|(_, s)| s))
        .unwrap_or(GridStep::Q0);
    let would = if matches!(input.exactness, Exactness::Approximate { .. }) {
        Exactness::Approximate {
            deviation: Steps(bound),
            step: would_step,
        }
    } else {
        Exactness::Certified {
            bound: Steps(bound),
            step: would_step,
        }
    };
    if step.is_some() && bound <= old {
        return Ok(would);
    }
    let accepted = input.accept.is_some_and(|accept| {
        accept.rank() >= would.rank()
            && accept
                .bound()
                .is_some_and(|(most, s)| would.within(most, s).is_ok())
    });
    if accepted {
        Ok(would)
    } else {
        Err(fail(ctx, MakeDetail::NotAccepted { would }))
    }
}
