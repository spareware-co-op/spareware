//! Inputs for `geom-mesh`'s hot paths (pieces G153 and G146, decision 32.44): a small terrain, a
//! scatter of survey points with breaklines across it; and a small height field, every face on its
//! own plane, for the exact mesh's build and check.
//!
//! For point location (piece G147), that field closed into a solid, with points in its box; for
//! booleans (piece G148), two small prisms that cross; for slicing (piece G149), that solid and
//! level planes; for hulls, points of a lattice. For
//! the distance comparisons of piece G151, grid points in a room a million steps across, each
//! with a clearance of up to as much; for triangle contact, pairs of triangles in that room whose
//! planes mostly cut each other.
//!
//! Class `typical` only: each input is a [`Terrain`] of [`POINTS`] distinct grid points and
//! [`BREAKLINE`] constraints spread across them, or polygons of [`FACES`] triangles. The refusals
//! are tested in `geom-mesh` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the board's scatter with breaklines (chapter 19.2's target) at a size a count run can
//! afford, measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: one pass over the points per input, once per batch, outside every measured run.

use std::collections::BTreeSet;

use geom_mesh::Site;
use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::{ExactMesh, PropTable, Role};
use geom_num::Exactness;
use geom_num::grid::{P2, P3};
use geom_num::plane::Plane;
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};

use super::{Class, batch_of, p3, rng, within};
use crate::rng::SplitMix64;

/// The points of one terrain.
pub const POINTS: u16 = 96;

/// The breaklines of one terrain.
pub const BREAKLINE: u16 = 8;

/// The points of a height field's side.
const SIDE: u16 = 8;

/// The triangles of a height field: two to each of 7 × 7 cells.
pub const FACES: usize = 98;

/// The points of one [`Probe`].
pub const PROBES: usize = 64;

/// What `geom_mesh::boolean::boolean` takes.
#[derive(Clone, Debug)]
pub struct Pair {
    /// One solid.
    pub first: ExactMesh,
    /// The other, of the same ground plan and other heights.
    pub second: ExactMesh,
}

/// The planes of one [`Layers`].
pub const LAYERS: usize = 8;

/// What `geom_mesh::slice::slice` takes.
#[derive(Clone, Debug)]
pub struct Layers {
    /// A closed solid.
    pub solid: ExactMesh,
    /// Level planes, rising, across its top.
    pub planes: Vec<Plane>,
}

/// What `geom_mesh::classify` takes.
#[derive(Clone, Debug)]
pub struct Probe {
    /// A closed solid.
    pub solid: ExactMesh,
    /// Points in and around it.
    pub points: Vec<P3>,
}

/// What `geom_mesh::triangulate` takes.
#[derive(Clone, Debug)]
pub struct Terrain {
    /// The survey points, keyed by their order.
    pub sites: Vec<Site<u32>>,
    /// The breaklines, between points spread across the scatter.
    pub constraints: Vec<[u32; 2]>,
}

/// One terrain from `rng`.
fn terrain(r: &mut SplitMix64) -> Option<Terrain> {
    let mut seen = BTreeSet::new();
    while seen.len() < usize::from(POINTS) {
        let (x, y) = (r.next_u64() >> 44, r.next_u64() >> 44);
        seen.insert((i64::try_from(x).ok()?, i64::try_from(y).ok()?));
    }
    let mut sites = Vec::with_capacity(usize::from(POINTS));
    for (&(x, y), key) in seen.iter().zip(0_u32..) {
        sites.push(Site {
            at: P2::new(x, y).ok()?,
            key,
        });
    }
    let step = u32::from(POINTS / BREAKLINE);
    let constraints = (0..u32::from(BREAKLINE))
        .map(|i| {
            let start = i.saturating_mul(step);
            [start, start.saturating_add(step).saturating_sub(1)]
        })
        .collect();
    Some(Terrain { sites, constraints })
}

/// Terrains for `geom_mesh::triangulate`.
#[must_use]
pub fn cdt(class: Class) -> Vec<Terrain> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::cdt", class);
    batch_of(|_| terrain(&mut g))
}

/// One height field from `rng`: two triangles to a cell, at random heights.
fn field(r: &mut SplitMix64) -> Option<Polygons> {
    let side = u32::from(SIDE);
    let mut vertices = Vec::with_capacity(usize::from(SIDE).saturating_mul(usize::from(SIDE)));
    for y in 0..i64::from(SIDE) {
        for x in 0..i64::from(SIDE) {
            let z = i64::try_from(r.next_u64() >> 52).ok()?;
            let at = P3::new(x.saturating_mul(4096), y.saturating_mul(4096), z).ok()?;
            vertices.push(Corner::Grid(at));
        }
    }
    let mut corners = Vec::with_capacity(FACES.saturating_mul(3));
    for y in 0..side.saturating_sub(1) {
        for x in 0..side.saturating_sub(1) {
            let a = y.saturating_mul(side).saturating_add(x);
            let (b, d) = (a.saturating_add(1), a.saturating_add(side));
            let c = d.saturating_add(1);
            corners.extend([a, b, c, a, c, d]);
        }
    }
    let faces = u32::try_from(FACES).ok()?;
    Some(Polygons {
        vertices,
        corners,
        loop_start: (0..=faces).map(|f| f.saturating_mul(3)).collect(),
        face_loops: (0..=faces).collect(),
        face_run: alloc_zeros(FACES),
        runs: vec![Source {
            name: NameId::from_bytes([1; 16]),
            role: Role::Face,
            flags: 0,
        }],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev: Vec::new(),
        class: Exactness::Exact,
    })
}

/// `n` zeros.
fn alloc_zeros(n: usize) -> Vec<u32> {
    vec![0; n]
}

/// Polygons for `geom_mesh::exact::build::build`.
#[must_use]
pub fn exact_build(class: Class) -> Vec<Polygons> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::exact_build", class);
    batch_of(|_| field(&mut g))
}

/// One closed solid from `rng`: a height field's triangles over a flat base, with four walls.
fn solid(r: &mut SplitMix64) -> Option<ExactMesh> {
    let mut polygons = field(r)?;
    let side = u32::from(SIDE);
    let low = side.saturating_mul(side);
    let flat: Vec<Corner> = polygons
        .vertices
        .iter()
        .map(|corner| match corner {
            Corner::Grid(at) => P3::new(at.x.get(), at.y.get(), -4096).map(Corner::Grid),
            other => Ok(*other),
        })
        .collect::<Result<_, _>>()
        .ok()?;
    polygons.vertices.extend(flat);
    let at = |x: u32, y: u32| y.saturating_mul(side).saturating_add(x);
    let mut face = |vertices: &[u32]| {
        polygons.corners.extend_from_slice(vertices);
        let end = u32::try_from(polygons.corners.len()).ok()?;
        polygons.loop_start.push(end);
        Some(())
    };
    for y in 0..side.saturating_sub(1) {
        for x in 0..side.saturating_sub(1) {
            let a = low.saturating_add(at(x, y));
            let b = low.saturating_add(at(x.saturating_add(1), y));
            let d = low.saturating_add(at(x, y.saturating_add(1)));
            let c = low.saturating_add(at(x.saturating_add(1), y.saturating_add(1)));
            face(&[a, c, b])?;
            face(&[a, d, c])?;
        }
    }
    let end = side.saturating_sub(1);
    for i in 0..end {
        let next = i.saturating_add(1);
        for (a, b) in [
            (at(i, 0), at(next, 0)),
            (at(end, i), at(end, next)),
            (at(next, end), at(i, end)),
            (at(0, next), at(0, i)),
        ] {
            face(&[low.saturating_add(a), low.saturating_add(b), b, a])?;
        }
    }
    let faces = polygons.loop_start.len().saturating_sub(1);
    polygons.face_loops = (0..=u32::try_from(faces).ok()?).collect();
    polygons.face_run = alloc_zeros(faces);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    build(&polygons, &Exec::serial(), &mut meter).ok()
}

/// A closed solid and [`PROBES`] points in its box for `geom_mesh::classify`.
#[must_use]
pub fn locate(class: Class) -> Vec<Probe> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::locate", class);
    let reach = i64::from(SIDE).saturating_mul(4096);
    batch_of(|_| {
        let solid = solid(&mut g)?;
        let mut points = Vec::with_capacity(PROBES);
        for _ in 0..PROBES {
            let x = within(&mut g, reach).abs();
            let y = within(&mut g, reach).abs();
            points.push(P3::new(x, y, within(&mut g, 8192)).ok()?);
        }
        Some(Probe { solid, points })
    })
}

/// One prism from `x` along x: a cell of side 4096 over a flat base, its top in two triangles at
/// random heights.
fn prism(r: &mut SplitMix64, x: i64) -> Option<ExactMesh> {
    let mut vertices = Vec::with_capacity(8);
    for (i, j) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let at = |z: i64| P3::new(x + i * 4096, j * 4096, z).ok().map(Corner::Grid);
        let top = 4096 + i64::try_from(r.next_u64() >> 52).ok()?;
        vertices.push(at(top)?);
        vertices.push(at(0)?);
    }
    let faces: [&[u32]; 8] = [
        &[0, 2, 6],
        &[0, 6, 4],
        &[1, 7, 3],
        &[1, 5, 7],
        &[1, 3, 2, 0],
        &[3, 7, 6, 2],
        &[7, 5, 4, 6],
        &[5, 1, 0, 4],
    ];
    let mut corners = Vec::new();
    let mut loop_start = vec![0];
    for face in faces {
        corners.extend_from_slice(face);
        loop_start.push(u32::try_from(corners.len()).ok()?);
    }
    let polygons = Polygons {
        vertices,
        corners,
        loop_start,
        face_loops: (0..=8).collect(),
        face_run: alloc_zeros(8),
        runs: vec![Source {
            name: NameId::from_bytes([u8::from(x != 0); 16]),
            role: Role::Face,
            flags: 0,
        }],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev: Vec::new(),
        class: Exactness::Exact,
    };
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    build(&polygons, &Exec::serial(), &mut meter).ok()
}

/// Two small closed solids that cross, for `geom_mesh::boolean::boolean`.
#[must_use]
pub fn boolean(class: Class) -> Vec<Pair> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::boolean", class);
    batch_of(|_| {
        Some(Pair {
            first: prism(&mut g, 0)?,
            second: prism(&mut g, 2048)?,
        })
    })
}

/// Points of a small lattice, many on one plane, for `geom_mesh::hull::Hull::of`.
#[must_use]
pub fn hull(class: Class) -> Vec<Vec<P3>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::hull", class);
    batch_of(|_| {
        let mut points = Vec::with_capacity(PROBES);
        for _ in 0..PROBES {
            points.push(p3(
                within(&mut g, 16),
                within(&mut g, 16),
                within(&mut g, 16),
            ));
        }
        Some(points)
    })
}

/// A closed solid and level planes across it for `geom_mesh::slice::slice`.
#[must_use]
pub fn slice(class: Class) -> Vec<Layers> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::slice", class);
    batch_of(|_| {
        let mut planes = Vec::with_capacity(LAYERS);
        for k in 0..LAYERS {
            let height = i128::try_from(k)
                .ok()?
                .saturating_mul(512)
                .saturating_add(256);
            planes.push(Plane::new([0, 0, 1], height.saturating_neg()).ok()?);
        }
        Some(Layers {
            solid: solid(&mut g)?,
            planes,
        })
    })
}

/// The reach of the distance benchmarks' coordinates, in grid steps.
const ROOM: i64 = 1 << 20;

/// A point in the room.
fn point(g: &mut SplitMix64) -> P3 {
    p3(within(g, ROOM), within(g, ROOM), within(g, ROOM))
}

/// `N` points and a clearance.
fn points_and_clearance<const N: usize>(name: &str, class: Class) -> Vec<([P3; N], u64)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(name, class);
    batch_of(|_| {
        let c = u64::try_from(within(&mut g, ROOM).abs()).ok()?;
        Some((core::array::from_fn(|_| point(&mut g)), c))
    })
}

/// Pairs of points and clearances for `geom_num::distance::point_point`.
#[must_use]
pub fn point_point(class: Class) -> Vec<([P3; 2], u64)> {
    points_and_clearance("mesh::point_point", class)
}

/// Lines, points and clearances for `geom_num::distance::point_line`.
#[must_use]
pub fn point_line(class: Class) -> Vec<([P3; 3], u64)> {
    points_and_clearance("mesh::point_line", class)
}

/// Planes, points and clearances for `geom_num::distance::point_plane`.
#[must_use]
pub fn point_plane(class: Class) -> Vec<([P3; 4], u64)> {
    points_and_clearance("mesh::point_plane", class)
}

/// Two lines and a clearance for `geom_num::distance::line_line`.
#[must_use]
pub fn line_line(class: Class) -> Vec<([P3; 4], u64)> {
    points_and_clearance("mesh::line_line", class)
}

/// Four points, for the products of dots a closest-feature test compares.
#[must_use]
pub fn closest_dots(class: Class) -> Vec<[P3; 4]> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::closest_dots", class);
    batch_of(|_| Some(core::array::from_fn(|_| point(&mut g))))
}

/// Pairs of triangles for `geom_mesh::contact::triangles`.
#[must_use]
pub fn triangles(class: Class) -> Vec<[[P3; 3]; 2]> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::triangles", class);
    batch_of(|_| {
        Some(core::array::from_fn(|_| {
            core::array::from_fn(|_| point(&mut g))
        }))
    })
}

/// Triangles and points near them for `geom_mesh::envelope::TriangleEnvelope`.
#[must_use]
pub fn envelope(class: Class) -> Vec<([P3; 3], P3)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("mesh::envelope", class);
    batch_of(|_| {
        let t = core::array::from_fn(|_| point(&mut g));
        Some((t, point(&mut g)))
    })
}
