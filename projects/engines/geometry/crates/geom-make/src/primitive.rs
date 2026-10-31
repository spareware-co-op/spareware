//! The primitives of chapter 10.2's Make family: box, prism, cylinder, cone, sphere and torus, each
//! the shape 10.2's "Primitive topology" fixes, its faces, edges and vertices named by role.
//!
//! What it does: each function checks its input, writes the solid as a [`Part`] and hands it to
//! [`assemble`], which names, publishes, checks and records it. Lengths are whole grid steps of the
//! frame (`geom_num::Steps`) and every vertex is a grid point; round shapes stand on the z axis
//! through the point given, and other orientations come from the Transform family.
//!
//! - **Box** ([`make_box`]): six faces on planes through its corners, named `min.x`, `max.x`,
//!   `min.y`, `max.y`, `min.z` and `max.z`.
//! - **Prism** ([`prism`]): a strictly convex base polygon, counter-clockwise from above, lifted by
//!   a height: faces `cap.start` (the base), `cap.end` and `side` with the index of its base edge
//!   as a count.
//! - **Cylinder** ([`cylinder`]) and **cone** ([`cone`]): caps on planes perpendicular to the axis
//!   (`cap.start`, `cap.end` for the cylinder) and a `side`, joined by full circles without
//!   vertices; the cone's apex has no entity.
//! - **Sphere** ([`sphere`]): one face, `surface`, with no loops.
//! - **Torus** ([`torus`]): a ring torus, one face `surface`, one vertex on its outer equator, and
//!   two full circles through it, `parallel` and `meridian`.
//!
//! Every edge of a box or prism is named `edge` with its two faces' names, every vertex `corner`
//! with its faces' names; every circle of a cylinder or cone `edge` with its two faces' names;
//! the regions `region.solid` and `region.outside`.
//!
//! Guarantees:
//!
//! - **Outward and counter-clockwise.** Every face's surface normal, with its sense, points out of
//!   the solid, and every loop runs counter-clockwise seen from outside.
//! - **Exact where it can be.** A box and a prism are recorded Exact: their planes are given by
//!   integers through grid corners. Round shapes are recorded Certified with a bound of one step of
//!   the step given, as 10.2 asks: their circles' points come from certified sines and cosines.
//! - **Typed refusals.** A zero length, a base that is not strictly convex or not level, and a
//!   torus that would not be a ring are input errors that name the input and the nearest value that
//!   works; a corner past the grid's range is the numbers layer's `OutOfRange`.
//!
//! Method: chapter 10.2's primitives and their topology, accepted with piece G85
//! (`projects/engines/geometry/architecture/10-operations.md`), on chapter 7's analytic surfaces
//! (`geom_surface::analytic`).
//!
//! Cost: constant per primitive, linear in a prism's corners (quadratic in them for its convexity
//! and edge lookups, at most a few hundred corners in practice). Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the geometry's: o, t and c points, r and h radius and height, x and z axes, a, b and n corners"
)]

use alloc::vec;
use alloc::vec::Vec;

use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_math::Box3;
use geom_num::{Exactness, GridStep, NumError, P3, Steps};
use geom_op::{MakeDetail, NameId, OpContext, OpError, OpOutput, Role, RoleArg};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind, Torus};
use geom_topo::Sense;

use crate::build::{
    Made, Part, PartEdge, PartFace, PartVertex, assemble, created, fail, plain, role_of,
};

/// A box: its least corner and its sizes along x, y and z, in grid steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxInput {
    /// The corner with the least x, y and z.
    pub min: P3,
    /// The sizes along x, y and z.
    pub size: [Steps; 3],
}

/// A prism: a strictly convex base polygon, counter-clockwise seen from above, at one height, and
/// the height it rises by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrismInput {
    /// The base's corners, in order.
    pub base: Vec<P3>,
    /// How far the top is above the base.
    pub height: Steps,
}

/// A cylinder or a cone: the centre of its base, its radius there and its height, along +z.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoundInput {
    /// The centre of the base.
    pub base: P3,
    /// The radius of the base.
    pub radius: Steps,
    /// The height, to the top (a cylinder) or the apex (a cone).
    pub height: Steps,
    /// The grid step the frame's coordinates count, which the result's bound counts too.
    pub step: GridStep,
}

/// A sphere: its centre and radius.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SphereInput {
    /// The centre.
    pub center: P3,
    /// The radius.
    pub radius: Steps,
    /// The grid step the frame's coordinates count.
    pub step: GridStep,
}

/// A ring torus about the z axis: its centre and its two radii, the minor below the major.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TorusInput {
    /// The centre.
    pub center: P3,
    /// The radius of the circle through the tube's centres.
    pub major: Steps,
    /// The tube's radius.
    pub minor: Steps,
    /// The grid step the frame's coordinates count.
    pub step: GridStep,
}

/// A length in steps as a signed coordinate offset, refused past the grid's reach.
fn length(s: Steps) -> Result<i64, NumError> {
    i64::try_from(s.get()).map_err(|_| NumError::OutOfRange {
        value: i128::from(s.get()),
    })
}

/// `p` moved by `d` steps, refused past the grid's range.
fn shift(p: P3, d: [i64; 3]) -> Result<P3, NumError> {
    let [dx, dy, dz] = d;
    let add = |c: geom_num::Coord, d: i64| {
        c.get().checked_add(d).ok_or(NumError::OutOfRange {
            value: i128::from(c.get()).saturating_add(i128::from(d)),
        })
    };
    P3::new(add(p.x, dx)?, add(p.y, dy)?, add(p.z, dz)?)
}

/// A grid point as doubles.
fn doubles(p: P3) -> [f64; 3] {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a grid coordinate is below 2^40, so the double holds it exactly"
    )]
    [p.x, p.y, p.z].map(|c| c.get() as f64)
}

/// A length in steps as a double, exactly (lengths that pass `length` are below 2^63, and those
/// that reach a primitive's geometry below 2^41).
fn real(s: Steps) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a length the grid's range admits is below 2^41, so the double holds it exactly"
    )]
    let x = s.get() as f64;
    x
}

/// Refuses a zero `value` for the input `input`.
fn positive(ctx: &OpContext<'_>, value: Steps, input: &'static str) -> Result<(), OpError> {
    if value.get() == 0 {
        Err(fail(ctx, MakeDetail::ZeroLength { input }))
    } else {
        Ok(())
    }
}

/// The error for a numbers-layer failure.
fn num(ctx: &OpContext<'_>, e: NumError) -> OpError {
    OpError::new(ctx.path().clone(), e)
}

/// A defect: the geometry or a name could not be formed from values already checked.
fn defect(ctx: &OpContext<'_>) -> OpError {
    fail(ctx, MakeDetail::BuildFailed)
}

/// The box with least corner `input.min` and sizes `input.size`.
///
/// # Errors
///
/// [`MakeDetail::ZeroLength`] for a zero size; the numbers layer's `OutOfRange` for a corner past
/// the grid's range; the meter's stop; a defect as [`MakeDetail::BuildFailed`] or
/// [`MakeDetail::ResultInvalid`].
pub fn make_box(ctx: &OpContext<'_>, input: &BoxInput) -> Result<OpOutput<Made>, OpError> {
    let [sx, sy, sz] = input.size;
    positive(ctx, sx, "size.x")?;
    positive(ctx, sy, "size.y")?;
    positive(ctx, sz, "size.z")?;
    let [dx, dy, dz] = [sx, sy, sz].map(length);
    let d = [
        dx.map_err(|e| num(ctx, e))?,
        dy.map_err(|e| num(ctx, e))?,
        dz.map_err(|e| num(ctx, e))?,
    ];
    // Corner i: bit 0 is x, bit 1 y, bit 2 z.
    let mut corners = Vec::with_capacity(8);
    for i in 0u8..8 {
        let pick = |bit: u8, size: i64| if i & bit == 0 { 0 } else { size };
        let [ax, ay, az] = d;
        corners.push(
            shift(input.min, [pick(1, ax), pick(2, ay), pick(4, az)]).map_err(|e| num(ctx, e))?,
        );
    }
    // Each face's corners counter-clockwise seen from outside.
    let faces: [(&[usize], &str); 6] = [
        (&[0, 2, 3, 1], "min.z"),
        (&[4, 5, 7, 6], "max.z"),
        (&[0, 1, 5, 4], "min.y"),
        (&[2, 6, 7, 3], "max.y"),
        (&[0, 4, 6, 2], "min.x"),
        (&[1, 3, 7, 5], "max.x"),
    ];
    let faces = faces
        .iter()
        .map(|(c, r)| Some((c.to_vec(), plain(r)?)))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| defect(ctx))?;
    let part = polyhedron(ctx, &corners, &faces).ok_or_else(|| defect(ctx))?;
    assemble(ctx, part, Exactness::Exact)
}

/// The prism over `input.base`, `input.height` high.
///
/// # Errors
///
/// [`MakeDetail::TooFewCorners`], [`MakeDetail::BaseNotLevel`] and [`MakeDetail::BaseNotConvex`]
/// for a base that is not a level, strictly convex polygon of at least three corners,
/// counter-clockwise from above; [`MakeDetail::ZeroLength`] for a zero height; the numbers
/// layer's `OutOfRange`; the meter's stop; a defect.
pub fn prism(ctx: &OpContext<'_>, input: &PrismInput) -> Result<OpOutput<Made>, OpError> {
    let n = input.base.len();
    if n < 3 {
        let corners = u64::try_from(n).unwrap_or(u64::MAX);
        return Err(fail(ctx, MakeDetail::TooFewCorners { corners }));
    }
    let z = input.base.first().map(|p| p.z);
    if input.base.iter().any(|p| Some(p.z) != z) {
        return Err(fail(ctx, MakeDetail::BaseNotLevel));
    }
    if !convex(&input.base) {
        return Err(fail(ctx, MakeDetail::BaseNotConvex));
    }
    positive(ctx, input.height, "height")?;
    let h = length(input.height).map_err(|e| num(ctx, e))?;
    let mut corners = input.base.clone();
    for p in &input.base {
        corners.push(shift(*p, [0, 0, h]).map_err(|e| num(ctx, e))?);
    }
    let mut faces = Vec::with_capacity(n.saturating_add(2));
    // The base seen from below runs the other way; the top runs as given.
    let bottom: Vec<usize> = core::iter::once(0).chain((1..n).rev()).collect();
    let top: Vec<usize> = (n..n.saturating_mul(2)).collect();
    faces.push((bottom, plain("cap.start").ok_or_else(|| defect(ctx))?));
    faces.push((top, plain("cap.end").ok_or_else(|| defect(ctx))?));
    for k in 0..n {
        let after = k
            .checked_add(1)
            .and_then(|a| a.checked_rem(n))
            .ok_or_else(|| defect(ctx))?;
        let quad = vec![k, after, after.saturating_add(n), k.saturating_add(n)];
        let index = u64::try_from(k).map_err(|_| defect(ctx))?;
        let role = Role::new("side", [RoleArg::Count(index)]).map_err(|_| defect(ctx))?;
        faces.push((quad, role));
    }
    let part = polyhedron(ctx, &corners, &faces).ok_or_else(|| defect(ctx))?;
    assemble(ctx, part, Exactness::Exact)
}

/// Whether `base` is a strictly convex polygon running counter-clockwise seen from above: every
/// corner turns left, strictly, and the boundary winds once (its corners' angles sum to one turn,
/// which for left turns throughout means no corner repeats and the sides do not cross).
fn convex(base: &[P3]) -> bool {
    let n = base.len();
    let at = |k: usize| {
        base.get(k.checked_rem(n)?)
            .map(|p| [i128::from(p.x.get()), i128::from(p.y.get())])
    };
    let turn = |k: usize| -> Option<i128> {
        let ([ax, ay], [bx, by], [cx, cy]) =
            (at(k)?, at(k.checked_add(1)?)?, at(k.checked_add(2)?)?);
        bx.checked_sub(ax)?
            .checked_mul(cy.checked_sub(ay)?)?
            .checked_sub(by.checked_sub(ay)?.checked_mul(cx.checked_sub(ax)?)?)
    };
    // Left turns throughout, and the sides' directions turn through one full turn: each side's
    // direction is above the first's in angle order once, so the number of times the direction
    // crosses the first side's line from below to above is exactly one.
    let left = (0..n).all(|k| turn(k).is_some_and(|t| t > 0));
    let side = |k: usize| -> Option<[i128; 2]> {
        let ([ax, ay], [bx, by]) = (at(k)?, at(k.checked_add(1)?)?);
        Some([bx.checked_sub(ax)?, by.checked_sub(ay)?])
    };
    let winds_once = side(0).is_some_and(|[fx, fy]| {
        let cross = |[x, y]: [i128; 2]| {
            fx.checked_mul(y)
                .zip(fy.checked_mul(x))
                .map(|(a, b)| a.checked_sub(b))
        };
        // Count the sides whose direction is on the right of the first side's (cross < 0), then
        // turns back to the left: a convex polygon passes from left to right exactly once.
        let signs: Option<Vec<i128>> = (0..n).map(|k| cross(side(k)?).flatten()).collect();
        signs.is_some_and(|s| {
            s.windows(2)
                .filter(|w| matches!(w, [a, b] if *a >= 0 && *b < 0))
                .count()
                <= 1
        })
    });
    left && winds_once
}

/// The part of a solid whose faces are the convex polygons `faces` over `corners`, each listed
/// counter-clockwise from outside with its role; one edge for each pair of corners a face side
/// joins, named by its two faces, and one vertex per corner, named by its faces.
fn polyhedron(ctx: &OpContext<'_>, corners: &[P3], faces: &[(Vec<usize>, Role)]) -> Option<Part> {
    let op = ctx.path();
    let face_names: Vec<NameId> = faces
        .iter()
        .map(|(_, r)| created(op, r))
        .collect::<Option<_>>()?;
    let mut ends: Vec<(usize, usize)> = Vec::new();
    let mut edge_faces: Vec<Vec<NameId>> = Vec::new();
    let mut part_faces = Vec::with_capacity(faces.len());
    for ((ring, role), &name) in faces.iter().zip(&face_names) {
        let n = ring.len();
        let mut fins = Vec::with_capacity(n);
        for k in 0..n {
            let a = *ring.get(k)?;
            let b = *ring.get(k.checked_add(1)?.checked_rem(n)?)?;
            let key = (a.min(b), a.max(b));
            let e = if let Some(e) = ends.iter().position(|&x| x == key) {
                e
            } else {
                ends.push(key);
                edge_faces.push(Vec::new());
                ends.len().checked_sub(1)?
            };
            edge_faces.get_mut(e)?.push(name);
            let sense = if a < b {
                Sense::Forward
            } else {
                Sense::Reversed
            };
            fins.push((u32::try_from(e).ok()?, sense));
        }
        let points: Vec<P3> = ring
            .iter()
            .map(|&c| corners.get(c).copied())
            .collect::<Option<_>>()?;
        let (o, along, across) = (
            doubles(*points.first()?),
            doubles(*points.get(1)?),
            doubles(*points.last()?),
        );
        let minus = |p: [f64; 3]| {
            let [px, py, pz] = p;
            let [ox, oy, oz] = o;
            [px - ox, py - oy, pz - oz]
        };
        let plane = Plane::new(o, minus(along), minus(across), ((0.0, 1.0), (0.0, 1.0))).ok()?;
        part_faces.push(PartFace {
            surface: SurfaceKind::Plane(plane),
            sense: Sense::Forward,
            loops: vec![fins],
            bound: bound(&points)?,
            role: role.clone(),
        });
    }
    let edges = ends
        .iter()
        .zip(&edge_faces)
        .map(|(&(a, b), names)| {
            let (p, q) = (*corners.get(a)?, *corners.get(b)?);
            let (from, to) = (doubles(p), doubles(q));
            let [fx, fy, fz] = from;
            let [tx, ty, tz] = to;
            let line = Line::new(from, [tx - fx, ty - fy, tz - fz], (0.0, 1.0)).ok()?;
            Some(PartEdge {
                ends: Some((u32::try_from(a).ok()?, u32::try_from(b).ok()?)),
                curve: CurveKind::Line(line),
                role: role_of("edge", names.iter().copied())?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let vertices = corners
        .iter()
        .enumerate()
        .map(|(c, &at)| {
            let incident = faces
                .iter()
                .zip(&face_names)
                .filter(|((ring, _), _)| ring.contains(&c))
                .map(|(_, &name)| name);
            Some(PartVertex {
                at,
                role: role_of("corner", incident)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Part {
        vertices,
        edges,
        faces: part_faces,
    })
}

/// The box of `points`.
fn bound(points: &[P3]) -> Option<Box3> {
    let first = *points.first()?;
    let pick = |f: fn(i64, i64) -> i64| {
        let [x, y, z] = points.iter().fold(
            [first.x.get(), first.y.get(), first.z.get()],
            |[x, y, z], p| [f(x, p.x.get()), f(y, p.y.get()), f(z, p.z.get())],
        );
        P3::new(x, y, z).ok()
    };
    Box3::new(pick(i64::min)?, pick(i64::max)?).ok()
}

/// The box from `p` moved back by `below` to `p` moved on by `above`, each not negative.
fn box_around(p: P3, below: [i64; 3], above: [i64; 3]) -> Result<Box3, NumError> {
    let back = below.map(|d| {
        d.checked_neg().ok_or(NumError::OutOfRange {
            value: i128::from(d),
        })
    });
    let [bx, by, bz] = back;
    Box3::new(shift(p, [bx?, by?, bz?])?, shift(p, above)?)
}

/// The plane z = `z` through `at`, its normal up (`+z`) or down.
fn level(at: [f64; 3], up: bool) -> Option<SurfaceKind> {
    let (a, b) = if up {
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    } else {
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0])
    };
    Plane::new(at, a, b, ((0.0, 1.0), (0.0, 1.0)))
        .ok()
        .map(SurfaceKind::Plane)
}

/// The full circle about `center` of `radius` with `normal`, starting along `x`.
fn circle(center: [f64; 3], radius: f64, x: [f64; 3], normal: [f64; 3]) -> Option<CurveKind<3>> {
    let frame = Frame::new(x, normal).ok()?;
    Circle::full(center, radius, frame)
        .ok()
        .map(CurveKind::Circle)
}

/// The certified record of a round primitive: one step of `step`.
const fn round_record(step: GridStep) -> Exactness {
    Exactness::Certified {
        bound: Steps(1),
        step,
    }
}

/// The two faces' names of a circle that joins them, as the circle's role.
fn joining(ctx: &OpContext<'_>, a: &Role, b: &Role) -> Option<Role> {
    let op = ctx.path();
    role_of("edge", [created(op, a)?, created(op, b)?])
}

/// The cylinder on `input.base`, `input.radius` wide and `input.height` high.
///
/// # Errors
///
/// [`MakeDetail::ZeroLength`] for a zero radius or height; the numbers layer's `OutOfRange`; the
/// meter's stop; a defect.
pub fn cylinder(ctx: &OpContext<'_>, input: &RoundInput) -> Result<OpOutput<Made>, OpError> {
    positive(ctx, input.radius, "radius")?;
    positive(ctx, input.height, "height")?;
    let (r, h) = (
        length(input.radius).map_err(|e| num(ctx, e))?,
        length(input.height).map_err(|e| num(ctx, e))?,
    );
    let top = shift(input.base, [0, 0, h]).map_err(|e| num(ctx, e))?;
    let side_box = box_around(input.base, [r, r, 0], [r, r, h]).map_err(|e| num(ctx, e))?;
    let bottom_box = box_around(input.base, [r, r, 0], [r, r, 0]).map_err(|e| num(ctx, e))?;
    let top_box = box_around(top, [r, r, 0], [r, r, 0]).map_err(|e| num(ctx, e))?;
    let part = (|| {
        let (o, t, rr, hh) = (
            doubles(input.base),
            doubles(top),
            real(input.radius),
            real(input.height),
        );
        let (start, end, side) = (plain("cap.start")?, plain("cap.end")?, plain("side")?);
        let x = [1.0, 0.0, 0.0];
        let z = [0.0, 0.0, 1.0];
        let wall = Cylinder::new(o, rr, Frame::standard().ok()?, ((0.0, 2.0), (0.0, hh))).ok()?;
        Some(Part {
            vertices: Vec::new(),
            edges: vec![
                PartEdge {
                    ends: None,
                    curve: circle(o, rr, x, z)?,
                    role: joining(ctx, &start, &side)?,
                },
                PartEdge {
                    ends: None,
                    curve: circle(t, rr, x, z)?,
                    role: joining(ctx, &end, &side)?,
                },
            ],
            faces: vec![
                PartFace {
                    surface: level(o, false)?,
                    sense: Sense::Forward,
                    loops: vec![vec![(0, Sense::Reversed)]],
                    bound: bottom_box,
                    role: start,
                },
                PartFace {
                    surface: level(t, true)?,
                    sense: Sense::Forward,
                    loops: vec![vec![(1, Sense::Forward)]],
                    bound: top_box,
                    role: end,
                },
                PartFace {
                    surface: SurfaceKind::Cylinder(wall),
                    sense: Sense::Forward,
                    loops: vec![vec![(0, Sense::Forward)], vec![(1, Sense::Reversed)]],
                    bound: side_box,
                    role: side,
                },
            ],
        })
    })()
    .ok_or_else(|| defect(ctx))?;
    assemble(ctx, part, round_record(input.step))
}

/// The cone on `input.base`, `input.radius` wide there, narrowing to its apex `input.height`
/// above it.
///
/// # Errors
///
/// [`MakeDetail::ZeroLength`] for a zero radius or height; the numbers layer's `OutOfRange`; the
/// meter's stop; a defect.
pub fn cone(ctx: &OpContext<'_>, input: &RoundInput) -> Result<OpOutput<Made>, OpError> {
    positive(ctx, input.radius, "radius")?;
    positive(ctx, input.height, "height")?;
    let (r, h) = (
        length(input.radius).map_err(|e| num(ctx, e))?,
        length(input.height).map_err(|e| num(ctx, e))?,
    );
    let side_box = box_around(input.base, [r, r, 0], [r, r, h]).map_err(|e| num(ctx, e))?;
    let base_box = box_around(input.base, [r, r, 0], [r, r, 0]).map_err(|e| num(ctx, e))?;
    let part = (|| {
        let (o, rr, hh) = (doubles(input.base), real(input.radius), real(input.height));
        let (start, side) = (plain("cap.start")?, plain("side")?);
        let wall = Cone::new(
            o,
            rr,
            -rr / hh,
            Frame::standard().ok()?,
            ((0.0, 2.0), (0.0, hh)),
        )
        .ok()?;
        Some(Part {
            vertices: Vec::new(),
            edges: vec![PartEdge {
                ends: None,
                curve: circle(o, rr, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])?,
                role: joining(ctx, &start, &side)?,
            }],
            faces: vec![
                PartFace {
                    surface: level(o, false)?,
                    sense: Sense::Forward,
                    loops: vec![vec![(0, Sense::Reversed)]],
                    bound: base_box,
                    role: start,
                },
                PartFace {
                    surface: SurfaceKind::Cone(wall),
                    sense: Sense::Forward,
                    loops: vec![vec![(0, Sense::Forward)]],
                    bound: side_box,
                    role: side,
                },
            ],
        })
    })()
    .ok_or_else(|| defect(ctx))?;
    assemble(ctx, part, round_record(input.step))
}

/// The sphere about `input.center` of `input.radius`.
///
/// # Errors
///
/// [`MakeDetail::ZeroLength`] for a zero radius; the numbers layer's `OutOfRange`; the meter's
/// stop; a defect.
pub fn sphere(ctx: &OpContext<'_>, input: &SphereInput) -> Result<OpOutput<Made>, OpError> {
    positive(ctx, input.radius, "radius")?;
    let r = length(input.radius).map_err(|e| num(ctx, e))?;
    let ball_box = box_around(input.center, [r, r, r], [r, r, r]).map_err(|e| num(ctx, e))?;
    let part = (|| {
        let ball = Sphere::new(
            doubles(input.center),
            real(input.radius),
            Frame::standard().ok()?,
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .ok()?;
        Some(Part {
            vertices: Vec::new(),
            edges: Vec::new(),
            faces: vec![PartFace {
                surface: SurfaceKind::Sphere(ball),
                sense: Sense::Forward,
                loops: Vec::new(),
                bound: ball_box,
                role: plain("surface")?,
            }],
        })
    })()
    .ok_or_else(|| defect(ctx))?;
    assemble(ctx, part, round_record(input.step))
}

/// The ring torus about `input.center` with radii `input.major` and `input.minor`.
///
/// # Errors
///
/// [`MakeDetail::ZeroLength`] for a zero radius; [`MakeDetail::MinorNotBelowMajor`] unless the
/// minor radius is below the major; the numbers layer's `OutOfRange`; the meter's stop; a defect.
pub fn torus(ctx: &OpContext<'_>, input: &TorusInput) -> Result<OpOutput<Made>, OpError> {
    positive(ctx, input.major, "major")?;
    positive(ctx, input.minor, "minor")?;
    if input.minor.get() >= input.major.get() {
        let major = input.major.get();
        return Err(fail(ctx, MakeDetail::MinorNotBelowMajor { major }));
    }
    let (big, small) = (
        length(input.major).map_err(|e| num(ctx, e))?,
        length(input.minor).map_err(|e| num(ctx, e))?,
    );
    let out = big
        .checked_add(small)
        .ok_or_else(|| num(ctx, NumError::OutOfRange { value: i128::MAX }))?;
    let corner = shift(input.center, [out, 0, 0]).map_err(|e| num(ctx, e))?;
    let ring_box =
        box_around(input.center, [out, out, small], [out, out, small]).map_err(|e| num(ctx, e))?;
    let part = (|| {
        let c = doubles(input.center);
        let (rb, rs) = (real(input.major), real(input.minor));
        let [cx, cy, cz] = c;
        let ring = Torus::new(c, rb, rs, Frame::standard().ok()?, ((0.0, 2.0), (0.0, 2.0))).ok()?;
        Some(Part {
            vertices: vec![PartVertex {
                at: corner,
                role: plain("vertex")?,
            }],
            edges: vec![
                PartEdge {
                    ends: Some((0, 0)),
                    curve: circle(c, rb + rs, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])?,
                    role: plain("parallel")?,
                },
                PartEdge {
                    ends: Some((0, 0)),
                    curve: circle([cx + rb, cy, cz], rs, [1.0, 0.0, 0.0], [0.0, -1.0, 0.0])?,
                    role: plain("meridian")?,
                },
            ],
            faces: vec![PartFace {
                surface: SurfaceKind::Torus(ring),
                sense: Sense::Forward,
                loops: vec![vec![
                    (0, Sense::Forward),
                    (1, Sense::Forward),
                    (0, Sense::Reversed),
                    (1, Sense::Reversed),
                ]],
                bound: ring_box,
                role: plain("surface")?,
            }],
        })
    })()
    .ok_or_else(|| defect(ctx))?;
    assemble(ctx, part, round_record(input.step))
}
