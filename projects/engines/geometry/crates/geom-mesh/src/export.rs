//! Exact meshes written at each format's precision (19.5, 24.10).
//!
//! What it does: [`triangles`] commits a mesh to the grid and splits its faces into triangles.
//! [`export`] writes those as OBJ, ASCII STL or a 3MF model part, in exact decimals of millimetres,
//! or as binary STL or binary glTF, in `f32` relative to a chunk origin. It then checks what it
//! wrote with exact predicates, and says how far from the mesh the file can be.
//!
//! Guarantees:
//!
//! - **Text carries grid points exactly.** A grid step is a whole number of quarter-nanometres, so
//!   a coordinate is a decimal of at most eight places in millimetres, written in full.
//! - **`f32` is rounded once, on one lattice.** Every value of a chunk is rounded to the nearest
//!   multiple of the power of two that fits the chunk's largest value in 24 bits. Each such multiple
//!   is an `f32`, so the error is at most half a unit in the last place at the chunk's extent. That
//!   is what 19.5 asks, at every value and not only the largest, and it makes the written points
//!   small integers, which the kernel's own predicates can check.
//! - **The bound is proven and stated.** [`Export::bound`] adds the lattice's half step to the commit
//!   of three-plane points to the grid, in quarter-nanometres.
//! - **Rounding that breaks the mesh is refused.** A triangle that turned over or closed up, or two
//!   that came to touch, are [`Flaw`]s, with their faces. [`export`] returns
//!   [`ExportError::RoundingFlip`] unless told to accept them, and then lists them.
//! - **Deterministic.** The bytes depend on the mesh and the options alone, and no floating-point
//!   sum is done: an `f32` is built from its integer.
//!
//! Method: 19.5's export and 24.10's steps 3 to 5
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`,
//! `24-file-format-exchange.md`), for OBJ (Wavefront's appendix to the Advanced Visualizer manual),
//! STL (3D Systems, 1989), the 3MF Core Specification and glTF 2.0
//! (<https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html>). A face that is not a triangle goes
//! through this crate's constrained Delaunay triangulation in the plan its normal leans on least.
//! The zip a 3MF package needs is the exchange layer's, not this one's.
//!
//! Cost: O(n log n) for n triangles: one triangulation per face that is not a triangle, and a query
//! of a box tree per triangle for the check, charged to the meter per test.

#![allow(
    clippy::many_single_char_names,
    reason = "points, the axes i and j, and a triangle's corners, as chapter 19 names them"
)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use core::fmt::Write as _;

use geom_bvh::{Aabb, BoxTree, BvhError, Entry};
use geom_num::grid::{GridStep, P2, P3, Steps};
use geom_num::plane::Tpi;
use geom_num::predicate::orient2d;
use geom_num::{Exactness, NumError, Sign};
use geom_run::{Exec, Stopped, WorkMeter};

use crate::cdt::{CdtError, Site, triangulate};
use crate::contact::{Contact, triangles as contact};
use crate::exact::build::span;
use crate::exact::{ExactMesh, MeshFault, Parts, Vertex, at};

/// A format to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Wavefront OBJ, in exact decimals.
    Obj,
    /// ASCII STL, in exact decimals.
    AsciiStl,
    /// The model part of a 3MF package, in exact decimals.
    ThreeMf,
    /// Binary STL, `f32` relative to a chunk origin.
    BinaryStl,
    /// Binary glTF, `f32` relative to a chunk origin, which is the node's translation.
    Glb,
}

/// What rounding did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flaw {
    /// A triangle of this face turned over, or closed up.
    Flipped {
        /// The face.
        face: u32,
    },
    /// Two triangles that were apart now touch.
    Touching {
        /// Their faces, the lower first.
        faces: [u32; 2],
    },
}

/// Why a mesh could not be written.
#[derive(Clone, Debug, PartialEq)]
pub enum ExportError {
    /// Rounding broke the mesh here. The remedies are a finer unit, splitting the faces, or
    /// accepting it, which is recorded with the file.
    RoundingFlip {
        /// What broke, in order.
        flaws: Vec<Flaw>,
    },
    /// A face could not be split into triangles.
    Cdt(CdtError),
    /// The mesh does not fit together.
    Fault(MeshFault),
    /// The numbers layer refused.
    Num(NumError),
    /// The box tree refused.
    Bvh(BvhError),
    /// The meter stopped the run.
    Stopped(Stopped),
    /// More triangles or vertices than the format's 32-bit indices can name.
    Full,
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::RoundingFlip { flaws } => write!(
                f,
                "rounding broke the mesh in {} place(s): use a finer unit, split the faces, or accept it",
                flaws.len()
            ),
            ExportError::Cdt(e) => write!(f, "a face would not triangulate: {e}"),
            ExportError::Fault(e) => write!(f, "{e}"),
            ExportError::Num(e) => write!(f, "a mesh that cannot be written: {e:?}"),
            ExportError::Bvh(e) => write!(f, "{e}"),
            ExportError::Stopped(_) => f.write_str("the meter stopped the export"),
            ExportError::Full => f.write_str("more than the format's indices can name"),
        }
    }
}

impl core::error::Error for ExportError {}

impl From<CdtError> for ExportError {
    fn from(e: CdtError) -> Self {
        ExportError::Cdt(e)
    }
}

impl From<MeshFault> for ExportError {
    fn from(e: MeshFault) -> Self {
        ExportError::Fault(e)
    }
}

impl From<NumError> for ExportError {
    fn from(e: NumError) -> Self {
        ExportError::Num(e)
    }
}

impl From<BvhError> for ExportError {
    fn from(e: BvhError) -> Self {
        ExportError::Bvh(e)
    }
}

impl From<Stopped> for ExportError {
    fn from(e: Stopped) -> Self {
        ExportError::Stopped(e)
    }
}

/// A mesh on the grid, in triangles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Triangles {
    /// The vertices, as the mesh numbers them.
    pub vertices: Vec<P3>,
    /// Corners of each triangle, counter-clockwise from outside.
    pub corners: Vec<[u32; 3]>,
    /// The face each triangle came from.
    pub faces: Vec<u32>,
    /// Whether any vertex was off the grid, and so rounded.
    pub rounded: bool,
}

/// A written mesh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    /// The file.
    pub bytes: Vec<u8>,
    /// How far any written vertex can be from the mesh, in quarter-nanometres.
    pub bound: u64,
    /// `Exact` when nothing was rounded, `Certified` by `bound` otherwise.
    pub class: Exactness,
    /// What rounding broke, when that was accepted.
    pub flaws: Vec<Flaw>,
}

/// Half a grid step, rounded up, in quarter-nanometres: as far as a three-plane point moves to the
/// grid in each axis.
fn half_step(step: GridStep) -> u64 {
    u64::from(step.quarter_nm()).div_ceil(2)
}

/// Quarter-nanometres in a millimetre.
const QUARTER_NM_PER_MM: u128 = 4_000_000;

/// Which two axes a face's plan uses, and whether the normal runs up the third.
fn plan_axes(n: [i128; 3]) -> ([usize; 2], bool) {
    let [x, y, z] = n;
    let (a, b, c) = (x.unsigned_abs(), y.unsigned_abs(), z.unsigned_abs());
    if a >= b && a >= c {
        ([1, 2], x > 0)
    } else if b >= c {
        ([2, 0], y > 0)
    } else {
        ([0, 1], z > 0)
    }
}

/// `p` in the plan of axes `i` and `j`.
fn planned(p: P3, [i, j]: [usize; 2]) -> Result<P2, ExportError> {
    let parts = [p.x.get(), p.y.get(), p.z.get()];
    let u = parts.get(i).ok_or(MeshFault::Shape)?;
    let v = parts.get(j).ok_or(MeshFault::Shape)?;
    Ok(P2::new(*u, *v)?)
}

/// The outward normal of face `f`.
fn outward(parts: &Parts, f: u32) -> Result<[i128; 3], MeshFault> {
    let facing = *at(&parts.face_plane, f)?;
    let n = at(&parts.planes, facing.row)?.n();
    Ok(if facing.flipped {
        n.map(i128::wrapping_neg)
    } else {
        n
    })
}

/// The triangles inside the rings of a triangulation's constrained edges: those an odd number of
/// constraints in from the outside.
fn inside(triangles: &[[u32; 3]], constraints: &[[u32; 2]]) -> Result<Vec<usize>, ExportError> {
    let key = |a: u32, b: u32| (a.min(b), a.max(b));
    let mut edges: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for (t, corners) in triangles.iter().enumerate() {
        let [a, b, c] = *corners;
        for (u, v) in [(a, b), (b, c), (c, a)] {
            edges.entry(key(u, v)).or_default().push(t);
        }
    }
    let walled = |edge: (u32, u32)| {
        let (a, b) = edge;
        constraints.binary_search(&[a, b]).is_ok()
    };
    let mut depth: Vec<Option<bool>> = alloc::vec![None; triangles.len()];
    let mut work: Vec<usize> = Vec::new();
    for (edge, holders) in &edges {
        if let [t] = holders.as_slice() {
            let slot = depth.get_mut(*t).ok_or(MeshFault::Shape)?;
            if slot.is_none() || walled(*edge) {
                *slot = Some(walled(*edge));
                work.push(*t);
            }
        }
    }
    while let Some(t) = work.pop() {
        let here = depth.get(t).copied().flatten().ok_or(MeshFault::Shape)?;
        let [a, b, c] = *triangles.get(t).ok_or(MeshFault::Shape)?;
        for (u, v) in [(a, b), (b, c), (c, a)] {
            let edge = key(u, v);
            let beyond = here != walled(edge);
            for &other in edges.get(&edge).ok_or(MeshFault::Shape)? {
                let slot = depth.get_mut(other).ok_or(MeshFault::Shape)?;
                if other != t && slot.is_none() {
                    *slot = Some(beyond);
                    work.push(other);
                }
            }
        }
    }
    Ok(depth
        .iter()
        .enumerate()
        .filter(|(_, d)| **d == Some(true))
        .map(|(t, _)| t)
        .collect())
}

/// The triangles of a face of several corners, as vertices of the mesh.
fn split(
    points: &[P3],
    rings: &[Vec<u32>],
    (face, n): (u32, [i128; 3]),
    meter: &mut WorkMeter<'_>,
    out: &mut Vec<[u32; 3]>,
) -> Result<(), ExportError> {
    let (axes, up) = plan_axes(n);
    let mut sites: Vec<Site<u32>> = Vec::new();
    let mut seen: BTreeMap<(i64, i64), u32> = BTreeMap::new();
    let mut constraints = Vec::new();
    for ring in rings {
        let mut numbers = Vec::with_capacity(ring.len());
        for &v in ring {
            let at_plan = planned(*at(points, v)?, axes)?;
            let place = (at_plan.x.get(), at_plan.y.get());
            let fresh = u32::try_from(sites.len()).map_err(|_| ExportError::Full)?;
            let number = *seen.entry(place).or_insert(fresh);
            if number == fresh {
                sites.push(Site {
                    at: at_plan,
                    key: v,
                });
            }
            numbers.push(number);
        }
        let after = numbers.iter().cycle().skip(1);
        for (&a, &b) in numbers.iter().zip(after) {
            if a != b {
                constraints.push([a, b]);
            }
        }
    }
    let cdt = triangulate(&sites, &constraints, None, meter)?;
    let vertex = |t: u32| -> Result<u32, ExportError> {
        at(&cdt.vertices, t)?
            .key
            .ok_or_else(|| ExportError::RoundingFlip {
                flaws: alloc::vec![Flaw::Flipped { face }],
            })
    };
    for t in inside(&cdt.triangles, &cdt.constraints)? {
        let [a, b, c] = *cdt.triangles.get(t).ok_or(MeshFault::Shape)?;
        let (a, b, c) = (vertex(a)?, vertex(b)?, vertex(c)?);
        out.push(if up { [a, b, c] } else { [a, c, b] });
    }
    Ok(())
}

/// `mesh` on the grid, in triangles. Three-plane vertices round once.
///
/// # Errors
///
/// An [`ExportError`]: a three-plane vertex outside the frame, a face the rounding made cross
/// itself, or a stopped meter.
pub fn triangles(mesh: &ExactMesh, meter: &mut WorkMeter<'_>) -> Result<Triangles, ExportError> {
    let parts = mesh.parts();
    let mut vertices = Vec::with_capacity(parts.verts.len());
    let mut rounded = false;
    for v in &parts.verts {
        meter.charge(1)?;
        vertices.push(match *v {
            Vertex::Grid(p) => p,
            Vertex::Planes([a, b, c]) => {
                let row = |r: u32| at(&parts.planes, r).copied();
                rounded = true;
                Tpi::new(row(a)?, row(b)?, row(c)?)?.to_grid()?.value()
            }
        });
    }
    let mut corners = Vec::new();
    let mut faces = Vec::new();
    for f in 0..u32::try_from(mesh.faces()).map_err(|_| ExportError::Full)? {
        meter.charge(1)?;
        let (first, last) = span(&parts.face_loops, f)?;
        let mut rings: Vec<Vec<u32>> = Vec::new();
        for looped in first..last {
            let (start, end) = span(&parts.loop_start, looped)?;
            let ring = parts
                .he_vert
                .get(start as usize..end as usize)
                .ok_or(MeshFault::Shape)?;
            rings.push(ring.to_vec());
        }
        let before = corners.len();
        match rings.as_slice() {
            [ring] if ring.len() == 3 => {
                if let &[a, b, c] = ring.as_slice() {
                    corners.push([a, b, c]);
                }
            }
            _ => {
                let facing = (f, outward(parts, f)?);
                split(&vertices, &rings, facing, meter, &mut corners)?;
            }
        }
        faces.resize(
            faces
                .len()
                .saturating_add(corners.len().saturating_sub(before)),
            f,
        );
    }
    Ok(Triangles {
        vertices,
        corners,
        faces,
        rounded,
    })
}

/// The decimal places of a millimetre that hold any grid step: one quarter-nanometre is 25 of the
/// last.
const MILLIMETRE_PLACES: u32 = 8;

/// The same in metres.
const METRE_PLACES: u32 = 11;

/// `steps` grid steps in units of 10^(`shown` − 8) mm, as an exact decimal.
fn places(steps: i64, step: GridStep, shown: u32) -> String {
    let digits = u128::from(steps.unsigned_abs())
        .saturating_mul(u128::from(step.quarter_nm()))
        .saturating_mul(25);
    let scale = 10_u128.pow(shown);
    let whole = digits.checked_div(scale).unwrap_or(0);
    let part = digits.checked_rem(scale).unwrap_or(0);
    let mut out = String::new();
    if steps < 0 {
        out.push('-');
    }
    out.push_str(&whole.to_string());
    if part != 0 {
        let mut kept = part;
        let mut width = shown as usize;
        while kept % 10 == 0 {
            kept /= 10;
            width = width.saturating_sub(1);
        }
        let text = kept.to_string();
        out.push('.');
        for _ in text.len()..width {
            out.push('0');
        }
        out.push_str(&text);
    }
    out
}

/// `steps` grid steps as an exact decimal of millimetres, in full.
#[must_use]
pub fn decimal(steps: i64, step: GridStep) -> String {
    places(steps, step, MILLIMETRE_PLACES)
}

/// One point's text, its three coordinates split by `between`.
fn point_text(p: P3, step: GridStep, between: &str) -> String {
    let mut out = decimal(p.x.get(), step);
    out.push_str(between);
    out.push_str(&decimal(p.y.get(), step));
    out.push_str(between);
    out.push_str(&decimal(p.z.get(), step));
    out
}

fn write_obj(t: &Triangles, step: GridStep) -> String {
    let mut out = String::new();
    for p in &t.vertices {
        let _ = writeln!(out, "v {}", point_text(*p, step, " "));
    }
    for corner in &t.corners {
        let [a, b, c] = corner.map(|v| v.saturating_add(1));
        let _ = writeln!(out, "f {a} {b} {c}");
    }
    out
}

fn write_ascii_stl(t: &Triangles, step: GridStep) -> Result<String, ExportError> {
    let mut out = String::from("solid mesh\n");
    for corner in &t.corners {
        // A normal of zeros: readers take it from the order of the corners, which is exact.
        out.push_str(" facet normal 0 0 0\n  outer loop\n");
        for v in corner {
            let _ = writeln!(
                out,
                "   vertex {}",
                point_text(*at(&t.vertices, *v)?, step, " ")
            );
        }
        out.push_str("  endloop\n endfacet\n");
    }
    out.push_str("endsolid mesh\n");
    Ok(out)
}

fn write_three_mf(t: &Triangles, step: GridStep) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <model unit=\"millimeter\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n\
         <resources><object id=\"1\" type=\"model\"><mesh><vertices>\n",
    );
    for p in &t.vertices {
        let (x, y, z) = (
            decimal(p.x.get(), step),
            decimal(p.y.get(), step),
            decimal(p.z.get(), step),
        );
        let _ = writeln!(out, "<vertex x=\"{x}\" y=\"{y}\" z=\"{z}\"/>");
    }
    out.push_str("</vertices><triangles>\n");
    for &[a, b, c] in &t.corners {
        let _ = writeln!(out, "<triangle v1=\"{a}\" v2=\"{b}\" v3=\"{c}\"/>");
    }
    out.push_str(
        "</triangles></mesh></object></resources>\n\
         <build><item objectid=\"1\"/></build>\n</model>\n",
    );
    out
}

/// A chunk's lattice: the origin, and the power of two of millimetres that is one unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lattice {
    /// The chunk's origin, the middle of its box.
    pub origin: P3,
    /// One unit is 2^`exponent` mm.
    pub exponent: i32,
}

/// The value a chunk writes for `steps` from its origin, in units of its lattice: the nearest,
/// ties to even.
fn on_lattice(steps: i64, step: GridStep, exponent: i32) -> Result<i32, ExportError> {
    let top = i128::from(steps)
        .checked_mul(i128::from(step.quarter_nm()))
        .ok_or(ExportError::Full)?;
    let mut bottom = QUARTER_NM_PER_MM;
    let mut magnitude = top.unsigned_abs();
    let shift = exponent.unsigned_abs();
    if exponent > 0 {
        bottom = bottom.checked_shl(shift).ok_or(ExportError::Full)?;
    } else {
        magnitude = magnitude
            .checked_mul(1_u128.checked_shl(shift).ok_or(ExportError::Full)?)
            .ok_or(ExportError::Full)?;
    }
    let units = magnitude.checked_div(bottom).ok_or(ExportError::Full)?;
    let left = magnitude.checked_rem(bottom).ok_or(ExportError::Full)?;
    let twice = left.saturating_mul(2);
    let up = twice > bottom || (twice == bottom && units % 2 == 1);
    let units = units.saturating_add(u128::from(up));
    let units = i32::try_from(units).map_err(|_| ExportError::Full)?;
    Ok(if top < 0 { units.wrapping_neg() } else { units })
}

/// The lattice that holds `points`: the finest that keeps each value at or under 2^24 units, and a
/// unit of that is the last place of an `f32` at the largest value.
fn lattice(points: &[P3], step: GridStep) -> Result<Lattice, ExportError> {
    let parts = |p: &P3| [p.x.get(), p.y.get(), p.z.get()];
    let mut middle = [0_i64; 3];
    let mut reach = 0_u128;
    for (axis, slot) in middle.iter_mut().enumerate() {
        let along = |p: &P3| parts(p).get(axis).copied().unwrap_or(0);
        let low = points.iter().map(along).min().unwrap_or(0);
        let high = points.iter().map(along).max().unwrap_or(0);
        *slot = low.wrapping_add(high.wrapping_sub(low) / 2);
        reach = reach.max(u128::from(high.abs_diff(*slot).max(low.abs_diff(*slot))));
    }
    let [x, y, z] = middle;
    let origin = P3::new(x, y, z)?;
    let largest = reach.saturating_mul(u128::from(step.quarter_nm()));
    let limit = QUARTER_NM_PER_MM << 24;
    let width = |v: u128| i32::try_from(u128::BITS.saturating_sub(v.leading_zeros())).unwrap_or(0);
    let mut exponent = width(largest).saturating_sub(width(limit));
    let fits = |e: i32| {
        let shift = e.unsigned_abs();
        if e >= 0 {
            limit.checked_shl(shift).is_some_and(|top| largest <= top)
        } else {
            largest.checked_shl(shift).is_some_and(|v| v <= limit)
        }
    };
    while !fits(exponent) {
        exponent = exponent.saturating_add(1);
    }
    Ok(Lattice { origin, exponent })
}

/// Half a unit of a lattice, rounded up, in quarter-nanometres.
fn half_unit(exponent: i32) -> u64 {
    let halves = 1_i32.saturating_sub(exponent);
    let shift = halves.unsigned_abs();
    let half = if halves > 0 {
        1_u128
            .checked_shl(shift)
            .map(|by| QUARTER_NM_PER_MM.div_ceil(by))
    } else {
        QUARTER_NM_PER_MM.checked_shl(shift)
    };
    half.and_then(|v| u64::try_from(v).ok()).unwrap_or(u64::MAX)
}

/// The `f32` of `units` × 2^`exponent`, which `units`, at most 2^24, makes exact, built from its
/// parts.
fn to_f32(units: i32, exponent: i32) -> f32 {
    if units == 0 {
        return f32::from_bits(0);
    }
    let magnitude = units.unsigned_abs();
    let width = u32::BITS.saturating_sub(magnitude.leading_zeros());
    let fraction = if width <= 24 {
        magnitude << 24_u32.saturating_sub(width)
    } else {
        magnitude >> width.saturating_sub(24)
    };
    let top = width.cast_signed().saturating_sub(1);
    let field = top
        .saturating_add(exponent)
        .saturating_add(127)
        .cast_unsigned();
    let sign = u32::from(units < 0) << 31;
    f32::from_bits(sign | (field << 23) | (fraction & 0x007f_ffff))
}

/// `points` in units of `lattice` from its origin, as points on the kernel's own grid.
fn lattice_points(points: &[P3], place: &Lattice, step: GridStep) -> Result<Vec<P3>, ExportError> {
    let mut out = Vec::with_capacity(points.len());
    for p in points {
        let unit = |c: i64, o: i64| on_lattice(c.wrapping_sub(o), step, place.exponent);
        out.push(P3::new(
            i64::from(unit(p.x.get(), place.origin.x.get())?),
            i64::from(unit(p.y.get(), place.origin.y.get())?),
            i64::from(unit(p.z.get(), place.origin.z.get())?),
        )?);
    }
    Ok(out)
}

fn write_binary_stl(t: &Triangles, points: &[P3], exponent: i32) -> Result<Vec<u8>, ExportError> {
    let count = u32::try_from(t.corners.len()).map_err(|_| ExportError::Full)?;
    let mut out = alloc::vec![0_u8; 80];
    out.extend_from_slice(&count.to_le_bytes());
    for corner in &t.corners {
        out.extend_from_slice(&[0_u8; 12]);
        for v in corner {
            let p = at(points, *v)?;
            for c in [p.x.get(), p.y.get(), p.z.get()] {
                let units = i32::try_from(c).map_err(|_| ExportError::Full)?;
                out.extend_from_slice(&to_f32(units, exponent).to_le_bytes());
            }
        }
        out.extend_from_slice(&[0_u8; 2]);
    }
    Ok(out)
}

/// `units` × 2^`exponent` as an exact decimal.
fn dyadic(units: i64, exponent: i32) -> String {
    let shift = exponent.unsigned_abs();
    let magnitude = u128::from(units.unsigned_abs());
    let mut out = String::new();
    if units < 0 {
        out.push('-');
    }
    let scaled = |by: u128| 1_u128.checked_shl(shift).map(|s| by.saturating_mul(s));
    if exponent >= 0 {
        out.push_str(&scaled(magnitude).unwrap_or(0).to_string());
        return out;
    }
    let scale = scaled(1).unwrap_or(1);
    let cut = |v: u128| {
        (
            v.checked_div(scale).unwrap_or(0),
            v.checked_rem(scale).unwrap_or(0),
        )
    };
    let (whole, mut part) = cut(magnitude);
    out.push_str(&whole.to_string());
    if part != 0 {
        out.push('.');
    }
    while part != 0 {
        let (digit, left) = cut(part.saturating_mul(10));
        out.push_str(&digit.to_string());
        part = left;
    }
    out
}

/// Binary glTF. Its positions are the lattice's, in millimetres, so its one node scales them to
/// glTF's metres and moves them to the origin, whose decimals are exact.
fn write_glb(
    t: &Triangles,
    points: &[P3],
    place: &Lattice,
    step: GridStep,
    bound: u64,
) -> Result<Vec<u8>, ExportError> {
    let mut low = [i64::MAX; 3];
    let mut high = [i64::MIN; 3];
    let mut binary: Vec<u8> = Vec::new();
    for p in points {
        let parts = [p.x.get(), p.y.get(), p.z.get()];
        for ((l, h), c) in low.iter_mut().zip(&mut high).zip(parts) {
            *l = (*l).min(c);
            *h = (*h).max(c);
            let units = i32::try_from(c).map_err(|_| ExportError::Full)?;
            binary.extend_from_slice(&to_f32(units, place.exponent).to_le_bytes());
        }
    }
    let positions = binary.len();
    for corner in &t.corners {
        for v in corner {
            binary.extend_from_slice(&v.to_le_bytes());
        }
    }
    let indices = binary.len().saturating_sub(positions);
    let list = |values: [i64; 3]| {
        let [x, y, z] = values.map(|c| dyadic(c, place.exponent));
        alloc::format!("[{x},{y},{z}]")
    };
    let o = place.origin;
    let metres = |c: i64| places(c, step, METRE_PLACES);
    let count = t.corners.len().saturating_mul(3);
    let mut json = alloc::format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"Spare Kit\"}},\"scene\":0,\
         \"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0,\"translation\":[{},{},{}],\
         \"scale\":[0.001,0.001,0.001],\"extras\":{{\"bound_quarter_nm\":{bound}}}}}],\
         \"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}},\"indices\":1}}]}}],\
         \"accessors\":[{{\"bufferView\":0,\"componentType\":5126,\"count\":{},\"type\":\"VEC3\",\
         \"min\":{},\"max\":{}}},{{\"bufferView\":1,\"componentType\":5125,\"count\":{count},\
         \"type\":\"SCALAR\"}}],\"bufferViews\":[{{\"buffer\":0,\"byteLength\":{positions}}},\
         {{\"buffer\":0,\"byteOffset\":{positions},\"byteLength\":{indices}}}],\
         \"buffers\":[{{\"byteLength\":{}}}]}}",
        metres(o.x.get()),
        metres(o.y.get()),
        metres(o.z.get()),
        points.len(),
        list(low),
        list(high),
        binary.len(),
    );
    while !json.len().is_multiple_of(4) {
        json.push(' ');
    }
    while !binary.len().is_multiple_of(4) {
        binary.push(0);
    }
    let sizes = |n: usize| u32::try_from(n).map_err(|_| ExportError::Full);
    let total = sizes(json.len().saturating_add(binary.len()).saturating_add(28))?;
    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2_u32.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&sizes(json.len())?.to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(json.as_bytes());
    out.extend_from_slice(&sizes(binary.len())?.to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&binary);
    Ok(out)
}

/// The box of a triangle.
fn box_of(points: &[P3], corner: [u32; 3]) -> Result<([i64; 3], [i64; 3]), ExportError> {
    let [a, b, c] = corner;
    let (p, q, r) = (*at(points, a)?, *at(points, b)?, *at(points, c)?);
    let axis = |x: i64, y: i64, z: i64| (x.min(y).min(z), x.max(y).max(z));
    let (low_x, high_x) = axis(p.x.get(), q.x.get(), r.x.get());
    let (low_y, high_y) = axis(p.y.get(), q.y.get(), r.y.get());
    let (low_z, high_z) = axis(p.z.get(), q.z.get(), r.z.get());
    Ok(([low_x, low_y, low_z], [high_x, high_y, high_z]))
}

/// Whether two triangles touch in `points`.
fn meet(points: &[P3], a: [u32; 3], b: [u32; 3]) -> Result<bool, ExportError> {
    let place = |t: [u32; 3]| -> Result<[P3; 3], ExportError> {
        let [x, y, z] = t;
        Ok([*at(points, x)?, *at(points, y)?, *at(points, z)?])
    };
    Ok(match contact(place(a)?, place(b)?) {
        // A triangle with no area is a flip, listed once on its own.
        Ok(Contact::Apart) | Err(NumError::Degenerate) => false,
        Ok(_) => true,
        Err(e) => return Err(e.into()),
    })
}

/// What moving the vertices from `before` to `after` broke: each triangle against its turn in the
/// plan of its face's normal, then each pair that were apart. Two triangles that share a corner
/// always touch, so their folding shows as a flip.
fn recheck(
    mesh: &ExactMesh,
    t: &Triangles,
    before: &[P3],
    after: &[P3],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Flaw>, ExportError> {
    let parts = mesh.parts();
    let mut flaws = Vec::new();
    for (corner, &f) in t.corners.iter().zip(&t.faces) {
        meter.charge(1)?;
        let (axes, up) = plan_axes(outward(parts, f)?);
        let [a, b, c] = *corner;
        let turn = orient2d(
            planned(*at(after, a)?, axes)?,
            planned(*at(after, b)?, axes)?,
            planned(*at(after, c)?, axes)?,
        )?;
        let wanted = if up { Sign::Positive } else { Sign::Negative };
        if turn != wanted {
            flaws.push(Flaw::Flipped { face: f });
        }
    }
    let mut entries = Vec::with_capacity(t.corners.len());
    for (corner, key) in t.corners.iter().zip(0_u64..) {
        let (low, high) = box_of(after, *corner)?;
        entries.push(Entry {
            key,
            bbox: Aabb::new(low, high)?,
        });
    }
    let tree = BoxTree::build(&entries, exec, meter)?;
    for (entry, corner) in entries.iter().zip(&t.corners) {
        for other in tree.overlapping(&entry.bbox, meter)? {
            if other <= entry.key {
                continue;
            }
            let at_other = usize::try_from(other).map_err(|_| ExportError::Full)?;
            let far = *t.corners.get(at_other).ok_or(MeshFault::Shape)?;
            if corner.iter().any(|v| far.contains(v)) {
                continue;
            }
            meter.charge(1)?;
            if meet(after, *corner, far)? && !meet(before, *corner, far)? {
                let here = usize::try_from(entry.key).map_err(|_| ExportError::Full)?;
                let (x, y) = (t.faces.get(here), t.faces.get(at_other));
                let (&x, &y) = x.zip(y).ok_or(MeshFault::Shape)?;
                flaws.push(Flaw::Touching {
                    faces: [x.min(y), x.max(y)],
                });
            }
        }
    }
    flaws.sort_unstable();
    flaws.dedup();
    Ok(flaws)
}

/// `mesh` written in `format`, a grid step being `step`.
///
/// # Errors
///
/// [`ExportError::RoundingFlip`] for what rounding broke, unless `accept` is set; what
/// [`triangles`] returns; [`ExportError::Full`] for more than the format can index.
pub fn export(
    mesh: &ExactMesh,
    format: Format,
    step: GridStep,
    accept: bool,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Export, ExportError> {
    let t = triangles(mesh, meter)?;
    let on_grid = if t.rounded { half_step(step) } else { 0 };
    let text = |s: String| s.into_bytes();
    let (bytes, bound, rounded, moved): (Vec<u8>, u64, bool, Option<Vec<P3>>) = match format {
        Format::Obj => (text(write_obj(&t, step)), on_grid, t.rounded, None),
        Format::AsciiStl => (text(write_ascii_stl(&t, step)?), on_grid, t.rounded, None),
        Format::ThreeMf => (text(write_three_mf(&t, step)), on_grid, t.rounded, None),
        Format::BinaryStl | Format::Glb => {
            let place = lattice(&t.vertices, step)?;
            let points = lattice_points(&t.vertices, &place, step)?;
            let bound = on_grid.saturating_add(half_unit(place.exponent));
            let bytes = if format == Format::Glb {
                write_glb(&t, &points, &place, step, bound)?
            } else {
                write_binary_stl(&t, &points, place.exponent)?
            };
            (bytes, bound, true, Some(points))
        }
    };
    let flaws = if rounded {
        let after = moved.as_deref().unwrap_or(&t.vertices);
        recheck(mesh, &t, &t.vertices, after, exec, meter)?
    } else {
        Vec::new()
    };
    if !flaws.is_empty() && !accept {
        return Err(ExportError::RoundingFlip { flaws });
    }
    Ok(Export {
        bytes,
        bound,
        class: if rounded {
            Exactness::Certified {
                bound: Steps(bound.div_ceil(u64::from(step.quarter_nm()))),
                step,
            }
        } else {
            Exactness::Exact
        },
        flaws,
    })
}
