//! Inputs for `geom-topo`'s hot paths (piece G80, decision 32.44): a column filled, copied as a
//! new version and written across a chunk boundary, and a small body's arenas published.
//!
//! Class `typical` only: each [`column_edit`] input is a column of [`ROWS`] values, one full chunk
//! and a little more, a row to write and its new value, and each [`publish`] input is a builder
//! holding [`FACES`] four-sided faces, every link inside its arena. Publishing's refusals (ragged
//! columns, a dangling link, an outside region that is not void) are tested in `geom-topo` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the shape of `geom-topo`'s own arena tests (`tests/arena.rs`), at one scene size, measured as
//! chapter 28.14 sets benchmarks out, `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: [`ROWS`] draws of [`SplitMix64`] per column input, a few dozen rows per builder, once per
//! batch, outside every measured run.

use geom_curve::{CurveKind, Line};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_surface::{Plane, SurfaceKind};
use geom_topo::{
    Builder, CHUNK_ROWS, Edge, Face, Fin, Ix, Loop, Region, RegionKind, Sense, Shell, Vertex,
};

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The rows of a benchmark's column: one full chunk and 64 more, so a write lands in either.
pub const ROWS: usize = CHUNK_ROWS as usize + 64;
/// The faces of a benchmark's builder, four fins each.
pub const FACES: u32 = 32;

/// A column's values, the row a copy writes and the value it writes there.
#[must_use]
pub fn column_edit(class: Class) -> Vec<(Vec<u64>, u32, u64)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("topo::column_edit", class);
    batch_of(|_| {
        let values = (0..ROWS).map(|_| g.next_u64()).collect();
        let row = u32::try_from(g.range(0, ROWS as i64 - 1)).ok()?;
        Some((values, row, g.next_u64()))
    })
}

/// A builder holding [`FACES`] faces of one loop of four fins on one shell of one solid region,
/// each fin its own edge. `None` only if an arena refuses a row, which the tests rule out.
fn builder(g: &mut SplitMix64) -> Option<Builder> {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions
        .push(Region {
            kind: RegionKind::Solid,
            first_shell: Ix::new(0),
        })
        .ok()?;
    a.regions
        .push(Region {
            kind: RegionKind::Void,
            first_shell: None,
        })
        .ok()?;
    let shell = a
        .shells
        .push(Shell {
            region: Ix::new(0)?,
            next: None,
            first_face: Ix::new(0),
        })
        .ok()?;
    let low = P3::new(0, 0, 0).ok()?;
    let high = P3::new(g.range(0, 1 << 20), 1, 1).ok()?;
    let bound = Box3::new(low, high).ok()?;
    let plane = Plane::new(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((0.0, 1.0), (0.0, 1.0)),
    )
    .ok()?;
    let line = Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).ok()?;
    let surface = a.surfaces.push(SurfaceKind::Plane(plane)).ok()?;
    let curve = a.curves.push(CurveKind::Line(line)).ok()?;
    let point = a.points.push(low).ok()?;
    let vertex = a.vertices.push(Vertex { point }).ok()?;
    for f in 0..FACES {
        a.faces
            .push(Face {
                shell,
                next: (f + 1 < FACES).then(|| Ix::new(f + 1)).flatten(),
                surface,
                sense: Sense::Forward,
                first_loop: Ix::new(f),
                bound,
            })
            .ok()?;
        a.loops
            .push(Loop {
                face: Ix::new(f)?,
                next: None,
                first_fin: Ix::new(4 * f),
            })
            .ok()?;
        for n in 0..4 {
            let row = 4 * f + n;
            a.fins
                .push(Fin {
                    loop_: Ix::new(f)?,
                    next: Ix::new(4 * f + (n + 1) % 4)?,
                    prev: Ix::new(4 * f + (n + 3) % 4)?,
                    edge: Ix::new(row)?,
                    sense: Sense::Forward,
                    radial: Ix::new(row)?,
                    pcurve: None,
                })
                .ok()?;
            a.edges
                .push(Edge {
                    first_fin: Ix::new(row),
                    start: Some(vertex),
                    end: Some(vertex),
                    curve,
                })
                .ok()?;
        }
    }
    Some(b)
}

/// Builders for `Builder::publish`.
#[must_use]
pub fn publish(class: Class) -> Vec<Builder> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("topo::publish", class);
    batch_of(|_| builder(&mut g))
}
