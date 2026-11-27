//! What a section or an elevation sees of one solid beyond its plane: the shadow of a flat-faced
//! solid seen along a grid axis between two depths, made from exact cuts (job 8.1; note A12's
//! build order, the outlines of step 5).
//!
//! What it does: [`sweep`] takes one solid the kernel builder made (its solid and the plane of
//! every face), the way a view looks (north, south, east or west) and two depths along it, and
//! gives loops whose union is the solid's shadow on the view between those depths: the region a
//! person sees it fill. The engine seam has no projection, so the shadow is made from what it does
//! have, the exact cut of a solid by a plane square to a grid axis (`Engine::section`):
//!
//! - **Events.** A flat-faced solid's cut by a plane square to the view changes its shape only
//!   where the plane passes a corner of the solid. Every corner lies where three of its faces'
//!   planes meet, so the depths where any three of the planes meet in one point are the only
//!   places the cut can change; between two of them the cut keeps its edges, each in one face, and
//!   each corner of the cut slides along the line where its two faces meet.
//! - **Slices.** Between each two neighbouring depths (the two given ones included) the solid is
//!   cut once, at a depth strictly between them (a whole step where there is one), so never at a
//!   corner. Neighbouring slices whose cuts have the same faces in the same order are one stretch.
//! - **The shadow of a stretch** from depth `a` to depth `b`: its cut moved to `a` (each corner
//!   where its two faces meet the plane at `a`, exactly), and for each edge of the cut in a face
//!   turned toward the viewer, the four-sided piece of that face between `a` and `b` (its corners
//!   where the face and its neighbours meet the planes at `a` and `b`). Seen along the view, a
//!   solid's shadow between two depths is the shadow of its faces turned toward the viewer and of
//!   its cut at the nearer depth; a face square to the view is covered by the cut at its own
//!   depth, which is where a stretch starts.
//!
//! Each side of a loop carries what it lies on ([`Mark`]): a face, where a plane at one depth
//! crosses it, or the edge where two faces meet, the path of a corner of the cut. The union of the
//! loops (the crate's exact overlay) is the shadow, and its boundary is the outline.
//!
//! What it needs: every face of the solid named with its plane (the kernel builder's
//! `KernelSolid::faces` names every one), flat faces only, and an engine that cuts a plane square
//! to a grid axis at any depth, as the seam's `section` does.
//!
//! Promises: exact (fractions of any size); a cut is never asked for at a corner; every corner a
//! cut gives is checked to lie where its two faces meet its plane, or the answer is
//! [`DrawingError::Overlay`]; the same solid and depths give the same loops in the same order.
//!
//! Cost: the events try every three distinct face planes (cubic in the faces: tens for a wall, a
//! door or a roof), and the engine cuts the solid once for each slice.
//!
//! Interim: the kernel's own exact view predicates, sections and hidden lines (geom-draw, pieces
//! G388, G389 and G394) replace this module through engine-geometry once they are built; the seam
//! has no projection today and no other engine answers one. It sits behind `crate::section`,
//! crate-private, so replacing it changes this one file; the tests (`tests/section.rs`) are written
//! against the crate's public types. The interim register (`geom-solidcore/tests/interim.json`,
//! programs) holds its row.

use std::collections::BTreeSet;

use construct_model::geometry::GeometryError;
use construct_regen::kernel::{Kernel, KernelSolid};
use engine_geometry::value::engine::{Cut, CutEdge, CutPoint, Engine, GridPlane};

use crate::DrawingError;
use crate::drawing::ViewPoint;
use crate::overlay::Q;
use crate::view::Looking;

/// What one side of a swept loop lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Mark {
    /// The edge where two faces meet, by their planes, the lesser first.
    Edge(GridPlane, GridPlane),
    /// A face, by its plane, where a plane at one depth crosses it.
    Face(GridPlane),
}

impl Mark {
    fn edge(a: GridPlane, b: GridPlane) -> Self {
        if a <= b {
            Self::Edge(a, b)
        } else {
            Self::Edge(b, a)
        }
    }
}

/// One solid's shadow between two depths.
pub(crate) struct Swept {
    /// Loops whose union is the shadow, in view coordinates: outlines counter-clockwise, holes
    /// clockwise, each vertex with the mark of the side leaving it.
    pub(crate) rings: Vec<Vec<(ViewPoint, Mark)>>,
    /// The depths, along the view, of the nearest and the farthest of the solid between the two
    /// depths given, when any of it is there.
    pub(crate) reach: Option<(Q, Q)>,
}

/// A point of space, exactly, in grid steps.
type Point3 = [Q; 3];

fn geometry(e: GeometryError) -> DrawingError {
    DrawingError::Geometry {
        code: e.code,
        message: e.message,
    }
}

fn broken(what: &str) -> DrawingError {
    DrawingError::Overlay(what.to_owned())
}

/// The index after `i` round a loop of `n`.
fn after(i: usize, n: usize) -> usize {
    i.checked_add(1).and_then(|j| j.checked_rem(n)).unwrap_or(0)
}

/// The index before `i` round a loop of `n`.
fn before(i: usize, n: usize) -> usize {
    i.checked_sub(1).unwrap_or_else(|| n.saturating_sub(1))
}

/// `a · b`, exact, for whole vectors.
fn dot(a: [i128; 3], b: [i128; 3]) -> Result<i128, DrawingError> {
    let mut sum: i128 = 0;
    for (x, y) in a.into_iter().zip(b) {
        sum = x
            .checked_mul(y)
            .and_then(|t| sum.checked_add(t))
            .ok_or(DrawingError::TooBig)?;
    }
    Ok(sum)
}

fn row(normal: [i128; 3]) -> [Q; 3] {
    normal.map(Q::whole)
}

/// The determinant of a three by three matrix.
#[allow(
    clippy::many_single_char_names,
    reason = "the matrix's entries by the letters its rule is written in"
)]
fn det3(m: &[[Q; 3]; 3]) -> Q {
    let [[a, b, c], [d, e, f], [g, h, i]] = m;
    let minor = |p: &Q, q: &Q, r: &Q, s: &Q| p.times(s).minus(&q.times(r));
    a.times(&minor(e, f, h, i))
        .minus(&b.times(&minor(d, f, g, i)))
        .plus(&c.times(&minor(d, e, g, h)))
}

/// The point where the planes `rows[k] · p = rhs[k]` meet, if they meet in one point (Cramer's
/// rule, exactly).
fn solve(rows: &[[Q; 3]; 3], rhs: &[Q; 3]) -> Result<Option<Point3>, DrawingError> {
    let d = det3(rows);
    if d.is_zero() {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(3);
    for k in 0..3 {
        let replaced: Vec<[Q; 3]> = rows
            .iter()
            .zip(rhs)
            .map(|(r, b)| {
                let mut r = r.clone();
                if let Some(c) = r.get_mut(k) {
                    *c = b.clone();
                }
                r
            })
            .collect();
        let m: [[Q; 3]; 3] = replaced
            .try_into()
            .map_err(|_| broken("a matrix of the wrong size"))?;
        out.push(det3(&m).over(&d)?);
    }
    let p: Point3 = out
        .try_into()
        .map_err(|_| broken("a point of the wrong size"))?;
    Ok(Some(p))
}

/// Where the faces on `f` and `g` meet the plane square to the view at depth `t` (`v · p = t`).
fn corner(f: &GridPlane, g: &GridPlane, v: [i128; 3], t: &Q) -> Result<Point3, DrawingError> {
    let rows = [row(f.normal), row(g.normal), row(v)];
    let rhs = [Q::whole(f.offset), Q::whole(g.offset), t.clone()];
    solve(&rows, &rhs)?
        .ok_or_else(|| broken("two faces of a cut's corner that do not meet its plane in a point"))
}

/// A cut's point, exactly.
fn point_of(p: &CutPoint) -> Result<Point3, DrawingError> {
    let [x, y, z] = p.num;
    Ok([
        Q::fraction(x, p.den)?,
        Q::fraction(y, p.den)?,
        Q::fraction(z, p.den)?,
    ])
}

/// A depth strictly between `a` and `b` (`a` below `b`): the least whole step above `a` where one
/// is below `b`, else halfway.
fn between(a: &Q, b: &Q) -> Result<Q, DrawingError> {
    let next = Q::whole(a.floor()?.checked_add(1).ok_or(DrawingError::TooBig)?);
    if &next < b {
        Ok(next)
    } else {
        Ok(a.plus(b).times(&Q::fraction(1, 2)?))
    }
}

/// The plane square to the view at depth `t` (`v · p = t`), its normal at the viewer, as the seam
/// takes it: whole coefficients.
fn depth_plane(v: [i128; 3], t: &Q) -> Result<GridPlane, DrawingError> {
    let (num, den) = t.parts()?;
    let mut normal = [0_i128; 3];
    for (n, c) in normal.iter_mut().zip(v) {
        *n = c
            .checked_neg()
            .and_then(|c| c.checked_mul(den))
            .ok_or(DrawingError::TooBig)?;
    }
    Ok(GridPlane {
        normal,
        offset: num.checked_neg().ok_or(DrawingError::TooBig)?,
    })
}

/// A cut's shape as its faces in order round each loop, each loop turned to its least rotation,
/// the loops sorted: two cuts with the same key have corners on the same lines.
fn key_of(cut: &Cut) -> Vec<Vec<GridPlane>> {
    let mut key: Vec<Vec<GridPlane>> = cut
        .loops
        .iter()
        .map(|ring| {
            let faces: Vec<GridPlane> = ring.edges.iter().map(|e| e.face).collect();
            (0..faces.len())
                .map(|k| {
                    let mut turned = faces.clone();
                    turned.rotate_left(k);
                    turned
                })
                .min()
                .unwrap_or_default()
        })
        .collect();
    key.sort();
    key
}

/// The view point of the element-frame point `p` moved by `by` grid steps, as two fractions.
fn project(looking: Looking, p: &Point3, by: [i64; 3]) -> (Q, Q) {
    let along = |axis: [i128; 3]| {
        let mut sum = Q::whole(0);
        for ((c, a), b) in p.iter().zip(axis).zip(by) {
            sum = sum.plus(&c.plus(&Q::whole(i128::from(b))).times(&Q::whole(a)));
        }
        sum
    };
    (along(looking.right()), along(looking.up()))
}

/// Twice the signed area of a loop of view points given as fractions.
fn twice_area(ring: &[(Q, Q)]) -> Q {
    let mut sum = Q::whole(0);
    for (i, (x0, y0)) in ring.iter().enumerate() {
        if let Some((x1, y1)) = ring.get(after(i, ring.len())) {
            sum = sum.plus(&x0.times(y1).minus(&y0.times(x1)));
        }
    }
    sum
}

/// A run of slices of one shape.
struct Stretch {
    from: Q,
    to: Q,
    /// The depth it was cut at, strictly between.
    at: Q,
    cut: Cut,
    key: Vec<Vec<GridPlane>>,
}

/// The shadow of `solid`, placed `by` grid steps from the origin, seen `looking` between the
/// depths `near` and `far` along the view (`v · p` of the project's points), built by `kernel`.
///
/// # Errors
///
/// [`DrawingError::Geometry`] for the engine's refusals of a cut, [`DrawingError::Overlay`] for a
/// cut whose corners are not where its faces meet, [`DrawingError::TooBig`] for a number past what
/// the seam's planes or a view point hold.
pub(crate) fn sweep<E: Engine + Send + 'static>(
    kernel: &Kernel<E>,
    looking: Looking,
    by: [i64; 3],
    solid: &KernelSolid<E::Solid>,
    (near, far): (&Q, &Q),
) -> Result<Swept, DrawingError> {
    let v = looking.direction();
    // The depths in the element's own frame, where its solid is.
    let shift = Q::whole(dot(v, by.map(i128::from))?);
    let (near, far) = (near.minus(&shift), far.minus(&shift));
    let mut stops = vec![near.clone()];
    stops.extend(events(looking, solid, &near, &far)?);
    stops.push(far);
    let stretches = stretches(kernel, v, solid, &stops)?;
    let mut rings = Vec::new();
    for stretch in &stretches {
        for ring in &stretch.cut.loops {
            shadow_of(looking, by, stretch, &ring.edges, &mut rings)?;
        }
    }
    let reach = match (stretches.first(), stretches.last()) {
        (Some(first), Some(last)) => Some((first.from.plus(&shift), last.to.plus(&shift))),
        _ => None,
    };
    Ok(Swept { rings, reach })
}

/// The depths strictly between `near` and `far` (in the solid's own frame) where three of the
/// solid's face planes meet in one point: the only depths where its cut can change, in order.
fn events<S>(
    looking: Looking,
    solid: &KernelSolid<S>,
    near: &Q,
    far: &Q,
) -> Result<BTreeSet<Q>, DrawingError> {
    let axis = looking.axis();
    let sign = Q::whole(looking.direction().get(axis).copied().unwrap_or(1));
    let planes: Vec<GridPlane> = solid
        .faces
        .iter()
        .map(|f| f.plane)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut events = BTreeSet::new();
    for (i, a) in planes.iter().enumerate() {
        for (j, b) in planes.iter().enumerate().skip(i.saturating_add(1)) {
            for c in planes.iter().skip(j.saturating_add(1)) {
                let rows = [row(a.normal), row(b.normal), row(c.normal)];
                let rhs = [Q::whole(a.offset), Q::whole(b.offset), Q::whole(c.offset)];
                if let Some(p) = solve(&rows, &rhs)? {
                    let depth = p
                        .get(axis)
                        .ok_or_else(|| broken("a point of no such axis"))?
                        .times(&sign);
                    if near < &depth && &depth < far {
                        events.insert(depth);
                    }
                }
            }
        }
    }
    Ok(events)
}

/// The solid cut once between each two neighbouring `stops`, neighbouring cuts of one shape made
/// one stretch; slices that cut nothing are left out.
fn stretches<E: Engine + Send + 'static>(
    kernel: &Kernel<E>,
    v: [i128; 3],
    solid: &KernelSolid<E::Solid>,
    stops: &[Q],
) -> Result<Vec<Stretch>, DrawingError> {
    let mut stretches: Vec<Stretch> = Vec::new();
    for pair in stops.windows(2) {
        let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        if a >= b {
            continue;
        }
        let at = between(a, b)?;
        let cut = kernel
            .section(&solid.solid, &depth_plane(v, &at)?)
            .map_err(geometry)?;
        if cut.loops.is_empty() {
            continue;
        }
        let key = key_of(&cut);
        match stretches.last_mut() {
            Some(last) if &last.to == a && last.key == key => last.to = b.clone(),
            _ => stretches.push(Stretch {
                from: a.clone(),
                to: b.clone(),
                at,
                cut,
                key,
            }),
        }
    }
    Ok(stretches)
}

/// The shadow of one loop of `stretch`'s cut (its `edges`), added to `rings`: the loop moved to the
/// stretch's nearer end, and the piece of each face turned toward the viewer between its two ends.
fn shadow_of(
    looking: Looking,
    by: [i64; 3],
    stretch: &Stretch,
    edges: &[CutEdge],
    rings: &mut Vec<Vec<(ViewPoint, Mark)>>,
) -> Result<(), DrawingError> {
    let v = looking.direction();
    let n = edges.len();
    let mut nearer = Vec::with_capacity(n);
    let mut farther = Vec::with_capacity(n);
    for (i, edge) in edges.iter().enumerate() {
        let prev = edges
            .get(before(i, n))
            .ok_or_else(|| broken("a cut's loop with no edge before one"))?;
        if corner(&prev.face, &edge.face, v, &stretch.at)? != point_of(&edge.start)? {
            return Err(broken(
                "a corner of the engine's cut that is not where its two faces meet its plane",
            ));
        }
        let at = |t: &Q| corner(&prev.face, &edge.face, v, t);
        nearer.push(project(looking, &at(&stretch.from)?, by));
        farther.push(project(looking, &at(&stretch.to)?, by));
    }
    // The cut moved to the stretch's nearer end.
    let mut first = Vec::with_capacity(n);
    for ((x, y), edge) in nearer.iter().zip(edges) {
        first.push((Q::point(x, y)?, Mark::Face(edge.face)));
    }
    rings.push(first);
    // The faces turned toward the viewer, each between the stretch's two ends.
    for (i, edge) in edges.iter().enumerate() {
        if dot(edge.face.normal, v)? >= 0 {
            continue;
        }
        let j = after(i, n);
        let (Some(next), Some(prev)) = (edges.get(j), edges.get(before(i, n))) else {
            return Err(broken("a cut's loop with no edge beside one"));
        };
        let (Some(ni), Some(nj), Some(fi), Some(fj)) =
            (nearer.get(i), nearer.get(j), farther.get(i), farther.get(j))
        else {
            return Err(broken("a corner of a cut's loop that is not there"));
        };
        let face = Mark::Face(edge.face);
        let ahead = Mark::edge(edge.face, next.face);
        let behind = Mark::edge(prev.face, edge.face);
        let quad = [ni.clone(), nj.clone(), fj.clone(), fi.clone()];
        let ordered = match twice_area(&quad).sign() {
            std::cmp::Ordering::Equal => continue,
            std::cmp::Ordering::Greater => [(ni, face), (nj, ahead), (fj, face), (fi, behind)],
            std::cmp::Ordering::Less => [(ni, behind), (fi, face), (fj, ahead), (nj, face)],
        };
        let mut piece = Vec::with_capacity(4);
        for ((x, y), mark) in ordered {
            piece.push((Q::point(x, y)?, mark));
        }
        rings.push(piece);
    }
    Ok(())
}
