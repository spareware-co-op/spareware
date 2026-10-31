//! The Transform family of chapter 10.2: move, rotate, mirror and scale a body, its entities keeping
//! their names, its geometry changing kind only as chapter 7's typed transforms say.
//!
//! What it does: each function turns its input into a `geom_math::TypedTransform` and hands it to
//! [`moved_copy`], which copies every row of the body with its points, curves, surfaces and face
//! boxes moved, publishes the copy under the same names, checks it and records each entity as
//! modified from itself (20.2: a one-to-one change keeps the name). [`Motion`] says which of the
//! four it was, for the attribute event `Transformed` (8.4).
//!
//! - **Exact where the transform is.** A move by whole steps, quarter turns, mirrors across grid
//!   planes and whole-number uniform scales map grid points to grid points and keep planes given
//!   by integers: an Exact body stays Exact.
//! - **Rounded once, and said so.** Any other rotation or scale rounds each vertex to the grid once
//!   (at most √3/2 of a step), and moves curves and surfaces by their own typed transforms, each
//!   with a certified deviation. The result's bound is the input's, stretched by the largest scale
//!   factor, plus one step if any vertex rounded, plus the largest deviation, rounded up.
//! - **Accepted, or refused.** A result less exact than the input is returned only when the caller
//!   accepts it ([`Moved::accept`]): a ceiling at least the result's bound, in the same step;
//!   otherwise [`MakeDetail::NotAccepted`] names the class the result would have.
//! - **Kinds change only where the checker can follow.** A scale that is not uniform turns circles
//!   into ellipses and analytic surfaces into affine ones, which the checker cannot certify yet:
//!   such a scale of a body with curved geometry is [`MakeDetail::KindNotCertified`] (G177).
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

use geom_curve::{CurveKind, ParamCurve};
use geom_math::{Axis, Box3, GridTransform, RationalTransform, Rotation, Scale, TypedTransform};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::history::{Origin, Record};
use geom_op::{MakeDetail, NameId, NameSet, OpContext, OpError, OpOutput};
use geom_run::WorkMeter;
use geom_surface::{ParamSurface, SurfaceKind};
use geom_topo::ix::Arena;
use geom_topo::order::{NameKey, Names};
use geom_topo::{Body, Builder, Edge, Face, Fin, Ix, Loop, Region, Sense, Shell, Vertex};

use crate::build::{Made, fail, finish, meter, stopped};

/// Which transform a body went through, for the attribute event `Transformed` (8.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// A translation.
    Move,
    /// A rotation.
    Rotate,
    /// A mirror.
    Mirror,
    /// A scale.
    Scale,
}

/// A body to transform, with its exactness record and the least exact result the caller accepts.
#[derive(Clone, Copy, Debug)]
pub struct Moved<'b> {
    /// The body, published in name order.
    pub body: &'b Body,
    /// Its exactness record.
    pub exactness: Exactness,
    /// The class and bound the caller accepts for a result less exact than the body; `None`
    /// accepts none.
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
    run(ctx, input, TypedTransform::Exact(t), (1, 1))
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
    run(ctx, input, t, (1, 1))
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
    run(ctx, input, t, (1, 1))
}

/// The body scaled by `factors` along x, y and z, about `center`.
///
/// # Errors
///
/// As [`move_by`]; [`MakeDetail::SingularTransform`] for a factor of zero;
/// [`MakeDetail::KindNotCertified`] for factors that are not all equal on a body with curved
/// geometry; [`MakeDetail::NotAccepted`] for a result less exact than the body that the caller
/// does not accept.
pub fn scale(
    ctx: &OpContext<'_>,
    input: Moved<'_>,
    factors: [Scale; 3],
    center: P3,
) -> Result<OpOutput<Made>, OpError> {
    if factors.iter().any(|s| s.num() == 0) {
        return Err(fail(ctx, MakeDetail::SingularTransform));
    }
    let [sx, sy, sz] = factors;
    let uniform = sx == sy && sy == sz;
    let curved = curved(input.body);
    if !uniform && curved {
        return Err(fail(ctx, MakeDetail::KindNotCertified));
    }
    let num = |e| OpError::new(ctx.path().clone(), e);
    // One denominator for the three factors: each numerator over the product of the others'
    // denominators.
    let den = sx
        .den()
        .checked_mul(sy.den())
        .and_then(|d| d.checked_mul(sz.den()))
        .and_then(|d| i64::try_from(d).ok())
        .ok_or_else(|| num(geom_num::NumError::OutOfRange { value: i128::MAX }))?;
    let over = |s: Scale| -> Option<i64> {
        let n = i128::from(s.num()).checked_mul(i128::from(den))?;
        i64::try_from(n.checked_div(i128::from(s.den()))?).ok()
    };
    let m = [
        [over(sx), Some(0), Some(0)],
        [Some(0), over(sy), Some(0)],
        [Some(0), Some(0), over(sz)],
    ]
    .map(|row| row.map(|x| x.ok_or(geom_num::NumError::OutOfRange { value: i128::MAX })));
    let m = m
        .into_iter()
        .map(|row| -> Result<[i64; 3], _> {
            let [a, b, c] = row;
            Ok([a?, b?, c?])
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(num)?;
    let m: [[i64; 3]; 3] = m
        .try_into()
        .map_err(|_| fail(ctx, MakeDetail::BuildFailed))?;
    let stretch = RationalTransform::new(m, [0; 3], den).map_err(num)?;
    let t = about(ctx, center, TypedTransform::from(stretch))?;
    // The largest factor, as a fraction, stretches the input's bound.
    let largest = factors.into_iter().fold((0u64, 1u64), |(n, d), s| {
        let bigger = u128::from(s.num())
            .checked_mul(u128::from(d))
            .zip(u128::from(n).checked_mul(u128::from(s.den())))
            .is_some_and(|(a, b)| a > b);
        if bigger { (s.num(), s.den()) } else { (n, d) }
    });
    run(ctx, input, t, largest)
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

/// Transforms `input` by `t`, whose largest scale factor is `stretch` (a numerator and a
/// denominator), keeping every entity's name.
fn run(
    ctx: &OpContext<'_>,
    input: Moved<'_>,
    t: TypedTransform,
    stretch: (u64, u64),
) -> Result<OpOutput<Made>, OpError> {
    let names = input
        .body
        .names()
        .ok_or_else(|| fail(ctx, MakeDetail::BodyNotNamed))?
        .clone();
    let mut meter = meter(ctx);
    let keep = |name: NameId| -> Option<(NameId, Origin)> {
        Some((
            name,
            Origin::Modified {
                from: NameSet::new([name]).ok()?,
            },
        ))
    };
    let (body, records, exactness) =
        moved_copy(ctx, &mut meter, input, &t, stretch, &names, &keep)?;
    finish(
        ctx,
        &mut meter,
        body,
        records,
        NameSet::new([]).map_err(|_| fail(ctx, MakeDetail::BuildFailed))?,
        exactness,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass copies every arena's rows with their moved geometry; split, each piece would carry the same seven arguments"
)]
/// A copy of `input` moved by `t`, each entity renamed and recorded by `rename` (which gives the
/// output's name and origin from the input's name), and the copy's exactness, accepted.
pub(crate) fn moved_copy(
    ctx: &OpContext<'_>,
    meter: &mut WorkMeter<'_>,
    input: Moved<'_>,
    t: &TypedTransform,
    stretch: (u64, u64),
    names: &Names,
    rename: &dyn Fn(NameId) -> Option<(NameId, Origin)>,
) -> Result<(Body, Vec<Record>, Exactness), OpError> {
    let defect = || fail(ctx, MakeDetail::BuildFailed);
    let num = |e| OpError::new(ctx.path().clone(), e);
    let mirror = t.classify().map_err(num)?.mirror;
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
    meter
        .charge(rows)
        .map_err(|stop| stopped(ctx, meter, stop))?;
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
        let moved = c
            .transformed(t, meter)
            .map_err(|_| fail(ctx, MakeDetail::KindNotCertified))?;
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
        let moved = s
            .transformed(t, meter)
            .map_err(|_| fail(ctx, MakeDetail::KindNotCertified))?;
        deviation = geom_curve::tight::larger(deviation, moved.deviation);
        surfaces.push(moved.value);
    }
    let exactness = record(ctx, input, stretch, rounded, deviation)?;
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
                bound: moved_box(t, *a.faces.bound.get(f)?)?,
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
    out.regions.extend(&regions).map_err(|_| defect())?;
    out.shells.extend(&shells).map_err(|_| defect())?;
    out.faces.extend(&faces).map_err(|_| defect())?;
    out.loops.extend(&loops).map_err(|_| defect())?;
    out.fins.extend(&fins).map_err(|_| defect())?;
    out.edges.extend(&edges).map_err(|_| defect())?;
    out.vertices.extend(&vertices).map_err(|_| defect())?;
    out.surfaces.extend(surfaces).map_err(|_| defect())?;
    out.curves.extend(curves).map_err(|_| defect())?;
    out.pcurves.extend(pcurves).map_err(|_| defect())?;
    out.points.extend(points).map_err(|_| defect())?;
    // Names: each renamed, and its record.
    let mut records = Vec::new();
    let mut rekey = |keys: &[NameKey]| -> Option<Vec<NameKey>> {
        keys.iter()
            .map(|k| {
                let (name, origin) = rename(NameId::from_bytes(k.0))?;
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
        .map_err(|_| defect())?;
    Ok((body, records, exactness))
}

/// Every row of an arena of `n` rows.
fn every<K>(n: u32) -> impl Iterator<Item = Ix<K>> {
    (0..n).filter_map(Ix::new)
}

/// The box holding `bound`'s image under `t`: its eight corners moved, and one step wider each way
/// when a corner rounds.
fn moved_box(t: &TypedTransform, bound: Box3) -> Option<Box3> {
    let (lo, hi) = (bound.min(), bound.max());
    let mut min = [i64::MAX; 3];
    let mut max = [i64::MIN; 3];
    let mut widen = 0i64;
    for i in 0u8..8 {
        let pick =
            |bit: u8, a: geom_num::Coord, b: geom_num::Coord| if i & bit == 0 { a } else { b };
        let corner = P3 {
            x: pick(1, lo.x, hi.x),
            y: pick(2, lo.y, hi.y),
            z: pick(4, lo.z, hi.z),
        };
        let image = t.apply(corner).ok()?;
        if !image.is_exact() {
            widen = 1;
        }
        let p = image.value();
        for (k, c) in [p.x, p.y, p.z].into_iter().enumerate() {
            let (m, n) = (min.get_mut(k)?, c.get());
            *m = (*m).min(n);
            let x = max.get_mut(k)?;
            *x = (*x).max(n);
        }
    }
    let [ax, ay, az] = min.map(|v| v.saturating_sub(widen));
    let [bx, by, bz] = max.map(|v| v.saturating_add(widen));
    Box3::new(P3::new(ax, ay, az).ok()?, P3::new(bx, by, bz).ok()?).ok()
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
    if !(deviation.is_finite() && (0.0..9.0e15).contains(&deviation)) {
        return Err(fail(ctx, MakeDetail::KindNotCertified));
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "the deviation is finite, not negative and below 2^53, checked just before: its whole part is a u64 the double holds exactly"
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
    let unchanged = step.is_some() && bound == old;
    let ceiling = input.accept.and_then(Exactness::bound);
    let would_step = step.or(ceiling.map(|(_, s)| s)).unwrap_or(GridStep::Q0);
    let would = Exactness::Certified {
        bound: Steps(bound),
        step: would_step,
    };
    if unchanged {
        return Ok(input.exactness);
    }
    match ceiling {
        Some((most, s)) if s == would_step && bound <= most.get() => Ok(would),
        _ => Err(fail(ctx, MakeDetail::NotAccepted { would })),
    }
}
