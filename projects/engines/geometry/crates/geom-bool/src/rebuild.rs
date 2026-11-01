//! The canonical rebuild: merged pieces published as a body (11.3, step 5).
//!
//! What it does: [`rebuild`] takes the pieces a selection keeps, has [`merge`] make them faces,
//! edges and shells, and writes those into arenas: every face on the surface of the polygon it
//! came from, every vertex a grid point or the lowest meeting of its planes, every edge a straight
//! shadow between rounded ends, and regions nested as the shells are.
//!
//! Guarantees:
//!
//! - **The same bytes however the boxes fell.** Every row comes in the order [`merge`] numbers the
//!   shape, which depends on the shape alone (11.3; tested).
//! - **Plane-based.** A vertex on the grid is a grid point. Any other is `Meet`, its lowest valid
//!   triple of the planes around it. Its edges' lines are shadows between rounded ends, as 8.3
//!   allows: the fins decide. An edge's two planes are its two faces' planes, read through its fins,
//!   so the body stores them once.
//! - **Refused, never guessed.** What [`merge`] refuses and what the arenas refuse are a
//!   [`RebuildError`].
//!
//! Method: chapter 11.3's rebuild (`projects/engines/geometry/architecture/11-booleans.md`), into
//! the Parasolid model structure of chapter 8.1, after EMBER (Trettner, Nehring-Wirxel and Kobbelt
//! 2022, <https://doi.org/10.1145/3528223.3530181>).
//!
//! Cost: linear in what [`merge`] returns, after its cost.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use geom_num::grid::COORD_MAX;
use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, P3, Sign};
use geom_poly::merge::{Curves, Merge, MergeError, Merged, Outward, Rules, merge_with};
use geom_poly::order::normals_det;
use geom_run::WorkMeter;
use geom_topo::Line as Shadow;
use geom_topo::{
    Body, BodyKind, Box3, Builder, CurveKind, Edge, Face, Fin, Ix, Loop, PointKind, Region,
    RegionKind, Sense, Shell, SurfaceKind, TopoError, Vertex,
};

/// The surface an input face lies on, and which way its polygon's normal runs on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Carrier {
    /// The surface.
    pub surface: SurfaceKind,
    /// [`Sense::Forward`] when the polygon's normal is the surface's.
    pub sense: Sense,
}

/// Why pieces could not be made into a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebuildError {
    /// The merge refused.
    Merge(MergeError),
    /// The numbers layer refused.
    Num(NumError),
    /// The topology refused.
    Topo(TopoError),
    /// A row past the end of a list: a defect, reported instead of a wrong body.
    Broken,
}

impl fmt::Display for RebuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RebuildError::Merge(e) => write!(f, "{e}"),
            RebuildError::Num(e) => write!(f, "pieces that cannot be rebuilt: {e:?}"),
            RebuildError::Topo(e) => write!(f, "a rebuilt body refused: {e:?}"),
            RebuildError::Broken => f.write_str("a rebuild lost a row"),
        }
    }
}

impl core::error::Error for RebuildError {}

impl From<MergeError> for RebuildError {
    fn from(e: MergeError) -> Self {
        RebuildError::Merge(e)
    }
}

impl From<NumError> for RebuildError {
    fn from(e: NumError) -> Self {
        RebuildError::Num(e)
    }
}

impl From<TopoError> for RebuildError {
    fn from(e: TopoError) -> Self {
        RebuildError::Topo(e)
    }
}

/// A row of an arena, as a typed index.
fn row<K>(at: usize) -> Result<Ix<K>, RebuildError> {
    u32::try_from(at)
        .ok()
        .and_then(Ix::new)
        .ok_or(RebuildError::Num(NumError::BudgetExceeded))
}

/// The lowest three of `planes` that meet in one point, or the point's own.
fn lowest_triple(planes: &[usize], pool: &[Plane], own: &Tpi) -> Result<[Plane; 3], NumError> {
    for (i, a) in planes.iter().enumerate() {
        for (j, b) in planes.iter().enumerate().skip(i.saturating_add(1)) {
            for c in planes.iter().skip(j.saturating_add(1)) {
                let triple = [pool.get(*a), pool.get(*b), pool.get(*c)];
                if let [Some(p), Some(q), Some(r)] = triple
                    && normals_det(p, q, r)? != Sign::Zero
                {
                    return Ok([*p, *q, *r]);
                }
            }
        }
    }
    Ok(*own.planes())
}

/// The grid box around `points`, a step wider where a point is off the grid.
fn bound_of(points: &[(P3, bool)]) -> Result<Box3, NumError> {
    let mut low = [COORD_MAX; 3];
    let mut high = [-COORD_MAX; 3];
    for (p, exact) in points {
        let margin = i64::from(!*exact);
        for ((l, h), v) in low
            .iter_mut()
            .zip(&mut high)
            .zip([p.x.get(), p.y.get(), p.z.get()])
        {
            *l = (*l).min(v.saturating_sub(margin)).max(-COORD_MAX);
            *h = (*h).max(v.saturating_add(margin)).min(COORD_MAX);
        }
    }
    let [ax, ay, az] = low;
    let [bx, by, bz] = high;
    Box3::new(P3::new(ax, ay, az)?, P3::new(bx, by, bz)?)
}

/// A coordinate as a double, which holds it exactly: it is below 2^40.
fn double(v: geom_num::Coord) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a coordinate is below 2^40, which a double holds"
    )]
    let x = v.get() as f64;
    x
}

/// An edge's shadow, between its rounded ends. An edge shorter than the grid, whose ends round to
/// one point, stands a step long from the end that is on the grid: group 2 holds a grid vertex to
/// its line's end exactly and a meeting to within a step.
fn shadow(from: (P3, bool), to: (P3, bool)) -> Result<CurveKind<3>, NumError> {
    let ((p, from_exact), (q, _)) = (from, to);
    let origin = [double(p.x), double(p.y), double(p.z)];
    let direction = [
        double(q.x) - double(p.x),
        double(q.y) - double(p.y),
        double(q.z) - double(p.z),
    ];
    let line = if direction != [0.0; 3] {
        Shadow::new(origin, direction, (0.0, 1.0))
    } else if from_exact {
        Shadow::new(origin, [1.0, 0.0, 0.0], (0.0, 1.0))
    } else {
        Shadow::new(origin, [1.0, 0.0, 0.0], (-1.0, 0.0))
    };
    line.map(CurveKind::Line).map_err(|_| NumError::Degenerate)
}

/// The planes around each vertex, by face number, ascending.
fn planes_around(merged: &Merged) -> Vec<Vec<usize>> {
    let mut around: Vec<Vec<usize>> = alloc::vec![Vec::new(); merged.vertices.len()];
    for fin in &merged.fins {
        for at in [fin.from, fin.to] {
            around
                .iter_mut()
                .skip(at)
                .take(1)
                .for_each(|list| list.push(fin.face));
        }
    }
    for list in &mut around {
        list.sort_unstable();
        list.dedup();
    }
    around
}

/// Writes `merged` into a builder: the arenas, then which region is the outside.
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the arenas in their order, each short, sharing the rows they fill"
)]
fn arenas(
    merged: &Merged,
    carriers: &[Carrier],
    open: bool,
) -> Result<(Builder, Ix<Region>), RebuildError> {
    let mut builder = Builder::new();
    let a = &mut builder.arenas;

    // Solid regions, one per outer shell, then the outside. A sheet bounds nothing, so all its
    // shells lie in the outside.
    let outers: Vec<usize> = (0..merged.shells.len())
        .filter(|s| !open && merged.shells.get(*s).is_some_and(|shell| shell.outer))
        .collect();
    let region_of: Vec<usize> = merged
        .shells
        .iter()
        .enumerate()
        .map(|(s, shell)| {
            let held = shell.holder.unwrap_or(s);
            outers
                .iter()
                .position(|o| *o == held)
                .unwrap_or(outers.len())
        })
        .collect();
    for region in 0..outers.len() {
        let first = region_of.iter().position(|r| *r == region);
        a.regions.push(Region {
            kind: RegionKind::Solid,
            first_shell: first.map(row).transpose()?,
        })?;
    }
    let outside = a.regions.push(Region {
        kind: RegionKind::Void,
        first_shell: (open && !merged.shells.is_empty())
            .then(|| row(0))
            .transpose()?,
    })?;
    for (s, region) in region_of.iter().enumerate() {
        let next =
            (s.saturating_add(1)..region_of.len()).find(|t| region_of.get(*t) == Some(region));
        let first_face = merged.shells.get(s).and_then(|shell| shell.faces.first());
        a.shells.push(Shell {
            region: row(*region)?,
            next: next.map(row).transpose()?,
            first_face: first_face.copied().map(row).transpose()?,
        })?;
    }

    // Vertices, each a grid point or its lowest meeting.
    let planes: Vec<Plane> = merged
        .faces
        .iter()
        .map(|f| merged.sheets.get(f.sheet).map(|s| s.face))
        .collect::<Option<_>>()
        .ok_or(RebuildError::Broken)?;
    let around = planes_around(merged);
    let coefficients = |plane: &Plane| {
        let [x, y, z] = plane.n();
        [x, y, z, plane.d()]
    };
    let mut pool: Vec<([i128; 4], Plane)> = Vec::new();
    let mut triples: Vec<Option<[Plane; 3]>> = Vec::with_capacity(merged.vertices.len());
    let mut rounded = Vec::with_capacity(merged.vertices.len());
    for (point, list) in merged.vertices.iter().zip(&around) {
        let near = point.to_grid()?;
        rounded.push((near.value(), near.is_exact()));
        if near.is_exact() {
            triples.push(None);
        } else {
            let triple = lowest_triple(list, &planes, point)?;
            pool.extend(triple.iter().map(|plane| (coefficients(plane), *plane)));
            triples.push(Some(triple));
        }
    }
    pool.sort_unstable_by_key(|(key, _)| *key);
    pool.dedup_by_key(|(key, _)| *key);
    for (_, plane) in &pool {
        a.planes.push(*plane)?;
    }
    let pool_row = |plane: &Plane| {
        pool.binary_search_by_key(&coefficients(plane), |(key, _)| *key)
            .map_err(|_| RebuildError::Broken)
            .and_then(row)
    };
    for (triple, (near, _)) in triples.iter().zip(&rounded) {
        let point = match triple {
            Some([p, q, r]) => PointKind::Meet([pool_row(p)?, pool_row(q)?, pool_row(r)?]),
            None => PointKind::Grid(*near),
        };
        let point = a.points.push(point)?;
        a.vertices.push(Vertex { point })?;
    }

    // Faces, loops and fins, ring by ring: a fin's row is its place in the rings.
    let mut fin_rows = alloc::vec![0usize; merged.fins.len()];
    let mut next_fin = 0usize;
    for face in &merged.faces {
        for ring in &face.rings {
            for fin in ring {
                fin_rows
                    .iter_mut()
                    .skip(*fin)
                    .take(1)
                    .for_each(|slot| *slot = next_fin);
                next_fin = next_fin.saturating_add(1);
            }
        }
    }
    let row_of = |fin: usize| fin_rows.get(fin).copied().ok_or(RebuildError::Broken);
    let mut surfaces: BTreeMap<usize, usize> = BTreeMap::new();
    let mut next_loop = 0usize;
    for (f, face) in merged.faces.iter().enumerate() {
        let sheet = merged.sheets.get(face.sheet).ok_or(RebuildError::Broken)?;
        let carrier = carriers.get(sheet.polygon).ok_or(RebuildError::Broken)?;
        let surface = if let Some(held) = surfaces.get(&sheet.polygon) {
            *held
        } else {
            let new = usize::try_from(a.surfaces.len()).unwrap_or(usize::MAX);
            a.surfaces.push(carrier.surface.clone())?;
            surfaces.insert(sheet.polygon, new);
            new
        };
        let mut corners = Vec::new();
        for fin in face.rings.iter().flatten() {
            let at = merged.fins.get(*fin).ok_or(RebuildError::Broken)?.from;
            corners.push(*rounded.get(at).ok_or(RebuildError::Broken)?);
        }
        let shell = merged.shells.get(face.shell).ok_or(RebuildError::Broken)?;
        let place = shell.faces.iter().position(|g| *g == f);
        let later = place.and_then(|p| shell.faces.get(p.saturating_add(1)));
        let turned = sheet.turned != (carrier.sense == Sense::Reversed);
        a.faces.push(Face {
            shell: row(face.shell)?,
            next: later.copied().map(row).transpose()?,
            surface: row(surface)?,
            sense: if turned {
                Sense::Reversed
            } else {
                Sense::Forward
            },
            first_loop: face.rings.first().map(|_| row(next_loop)).transpose()?,
            bound: bound_of(&corners)?,
        })?;
        for (l, ring) in face.rings.iter().enumerate() {
            let at = next_loop.saturating_add(l);
            a.loops.push(Loop {
                face: row(f)?,
                next: (l.saturating_add(1) < face.rings.len())
                    .then(|| row(at.saturating_add(1)))
                    .transpose()?,
                first_fin: ring
                    .first()
                    .copied()
                    .map(row_of)
                    .transpose()?
                    .map(row)
                    .transpose()?,
            })?;
        }
        next_loop = next_loop.saturating_add(face.rings.len());
    }

    // Each fin's neighbours around its edge.
    let mut radial_next = alloc::vec![0usize; merged.fins.len()];
    for edge in &merged.edges {
        let after = edge.fins.iter().cycle().skip(1);
        for (at, next) in edge.fins.iter().zip(after) {
            radial_next
                .iter_mut()
                .skip(*at)
                .take(1)
                .for_each(|slot| *slot = *next);
        }
    }
    let mut loop_row = 0usize;
    for face in &merged.faces {
        for ring in &face.rings {
            let next = ring.iter().cycle().skip(1);
            let prev = ring.iter().cycle().skip(ring.len().saturating_sub(1));
            for ((at, after), before) in ring.iter().zip(next).zip(prev) {
                let fin = merged.fins.get(*at).ok_or(RebuildError::Broken)?;
                let radial = radial_next.get(*at).ok_or(RebuildError::Broken)?;
                a.fins.push(Fin {
                    loop_: row(loop_row)?,
                    next: row(row_of(*after)?)?,
                    prev: row(row_of(*before)?)?,
                    edge: row(fin.edge)?,
                    sense: if fin.from < fin.to {
                        Sense::Forward
                    } else {
                        Sense::Reversed
                    },
                    radial: row(row_of(*radial)?)?,
                    pcurve: None,
                })?;
            }
            loop_row = loop_row.saturating_add(1);
        }
    }

    // Edges, with their shadows.
    for edge in &merged.edges {
        let end = |at: usize| rounded.get(at).copied().ok_or(RebuildError::Broken);
        let curve = a.curves.push(shadow(end(edge.low)?, end(edge.high)?)?)?;
        a.edges.push(Edge {
            first_fin: edge
                .fins
                .first()
                .copied()
                .map(row_of)
                .transpose()?
                .map(row)
                .transpose()?,
            start: Some(row(edge.low)?),
            end: Some(row(edge.high)?),
            curve,
        })?;
    }
    Ok((builder, outside))
}

/// The body `pieces` bound, every surface from `carriers`, by polygon index.
///
/// # Errors
///
/// A [`RebuildError`]: what [`merge`] refuses, or what the arenas refuse.
pub fn rebuild(
    pieces: &[Outward],
    carriers: &[Carrier],
    mode: Merge,
    meter: &mut WorkMeter<'_>,
) -> Result<Body, RebuildError> {
    rebuild_with(pieces, carriers, mode, Rules::default(), meter)
}

/// [`rebuild`], under `rules`: an open merge gives a sheet.
///
/// # Errors
///
/// As [`rebuild`].
pub fn rebuild_with(
    pieces: &[Outward],
    carriers: &[Carrier],
    mode: Merge,
    rules: Rules<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Body, RebuildError> {
    let merged = merge_with(pieces, mode, rules, meter)?;
    let (builder, outside) = arenas(&merged, carriers, rules.open)?;
    let kind = if rules.open {
        BodyKind::Sheet
    } else if merged.edges.iter().all(|e| e.fins.len() == 2) {
        BodyKind::Solid
    } else {
        BodyKind::General
    };
    Ok(builder.publish(kind, outside, meter)?)
}

/// The wire body of `curves`: edges with no fins, each vertex a grid point or its own meeting.
///
/// # Errors
///
/// A [`RebuildError`]: what the arenas refuse.
pub fn wire(curves: &Curves, meter: &mut WorkMeter<'_>) -> Result<Body, RebuildError> {
    let mut builder = Builder::new();
    let a = &mut builder.arenas;
    let outside = a.regions.push(Region {
        kind: RegionKind::Void,
        first_shell: None,
    })?;
    let mut rounded = Vec::with_capacity(curves.vertices.len());
    let mut stored: Vec<Plane> = Vec::new();
    for point in &curves.vertices {
        let near = point.to_grid()?;
        rounded.push((near.value(), near.is_exact()));
        let kind = if near.is_exact() {
            PointKind::Grid(near.value())
        } else {
            let mut rows = [row(0)?, row(0)?, row(0)?];
            for (slot, plane) in rows.iter_mut().zip(point.planes()) {
                let at = stored.iter().position(|held| held == plane);
                let at = at.unwrap_or_else(|| {
                    stored.push(*plane);
                    stored.len().saturating_sub(1)
                });
                *slot = row(at)?;
            }
            PointKind::Meet(rows)
        };
        let point = a.points.push(kind)?;
        a.vertices.push(Vertex { point })?;
    }
    for plane in &stored {
        a.planes.push(*plane)?;
    }
    for (low, high) in &curves.edges {
        let end = |at: usize| rounded.get(at).copied().ok_or(RebuildError::Broken);
        let curve = a.curves.push(shadow(end(*low)?, end(*high)?)?)?;
        a.edges.push(Edge {
            first_fin: None,
            start: Some(row(*low)?),
            end: Some(row(*high)?),
            curve,
        })?;
    }
    Ok(builder.publish(BodyKind::Wire, outside, meter)?)
}
