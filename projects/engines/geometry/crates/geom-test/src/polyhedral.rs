//! The polyhedral corpus of chapter 11.17's item 15 that random grid boxes do not cover (Gate 4,
//! G156): EMBER's figures 24 and 25 as generated boxes, and Manifold's regression models, stored
//! in this crate as the corpus `manifold-models`, each with its exact reference.
//!
//! What it does:
//!
//! - [`figure_24`] and [`figure_25`]: the two worst cases EMBER's paper draws for its arrangement
//!   (Trettner, Nehring-Wirxel and Kobbelt 2022, figures 24 and 25), built from their
//!   descriptions as unit-step boxes. Figure 24 is twenty by twenty beams: twenty along `x`, from
//!   `z` = 0 to 2, and twenty along `y`, from `z` = 1 to 3, crossing at all 400 places, so that
//!   nearly every face is cut by many others. Figure 25 is twenty cubes of side 4 in a row, each
//!   a step along from the last, all between `z` = 0 and 4, so that every cube shares its top and
//!   bottom planes with all the others.
//! - [`unit_cells`]: the exact count of unit cells a selection keeps, from which boxes hold
//!   each cell; [`box_solid`] the flat reference's solid of a box.
//! - [`MODELS`]: Manifold's test models at the commit the register pins, their files unmodified
//!   (`corpora/manifold-models`, Apache-2.0); [`mesh_polygons`] and [`mesh_solid`] give a model
//!   read onto the grid ([`crate::obj`]) as `geom-mesh`'s input and as the flat reference's solid.
//!
//! Guarantees:
//!
//! - **Exact.** Every corner is a grid point; the counts and the reference are exact.
//! - **Unmodified data.** The models are the stored corpus's bytes, compiled in, so every target
//!   reads the same ones; the corpus register's check holds the folder to its digest.
//!
//! Method: EMBER's figures 24 and 25 (<https://doi.org/10.1145/3528223.3530181>) and Manifold's
//! regression models, chapter 11.17's polyhedral verification corpus
//! (`projects/engines/geometry/architecture/11-booleans.md`); the unit-cell count is the
//! exact-volume pattern of csgrs' hypermesh tests that G140's laws follow.
//! Consulted: Manifold, Apache-2.0,
//!   <https://github.com/elalish/manifold/tree/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/test/models>,
//!   for its regression models, stored unmodified as the corpus `manifold-models`. No code copied.
//!
//! Cost: the figures are a few dozen boxes; [`unit_cells`] visits every unit cell of the boxes'
//! span once for each box; a model's polygons and solid are linear in its faces.

// The figures' coordinates are whole numbers below a hundred, which no step of theirs can take out
// of an i64.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "the figures' coordinates are below a hundred"
)]

use geom_mesh::exact::build::{Corner, Polygons, Source};
use geom_mesh::exact::{PropTable, Role};
use geom_num::{Exactness, P3};
use geom_op::NameId;

use crate::flat_reference::{Polygon, ReferenceError, Solid, point};
use crate::obj::Grid;

/// A box from `low` to `high`, each coordinate of `low` below `high`'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    /// The low corner.
    pub low: [i64; 3],
    /// The high corner.
    pub high: [i64; 3],
}

impl Block {
    /// Whether the unit cell whose low corner is `cell` lies in the box.
    #[must_use]
    pub fn holds(&self, cell: [i64; 3]) -> bool {
        cell.iter()
            .zip(self.low.iter().zip(&self.high))
            .all(|(c, (l, h))| l <= c && c < h)
    }
}

/// How many beams figure 24 lays each way.
pub const BEAMS: i64 = 20;

/// EMBER's figure 24: twenty beams along `x` and twenty along `y`, a step wide and two high, three
/// steps apart, the second layer a step above the first, so each pair crosses in a unit cube.
#[must_use]
pub fn figure_24() -> Vec<Block> {
    let length = 3 * BEAMS;
    let along_x = (0..BEAMS).map(|i| Block {
        low: [0, 3 * i, 0],
        high: [length, 3 * i + 1, 2],
    });
    let along_y = (0..BEAMS).map(|j| Block {
        low: [3 * j, 0, 1],
        high: [3 * j + 1, length, 3],
    });
    along_x.chain(along_y).collect()
}

/// How many cubes figure 25 lays in its row.
pub const CUBES: i64 = 20;

/// EMBER's figure 25: twenty cubes of side 4, the `i`-th from `x` = `i`, all from `y` = 0 and
/// `z` = 0 to 4, so each overlaps the next three and all share their top and bottom planes.
#[must_use]
pub fn figure_25() -> Vec<Block> {
    (0..CUBES)
        .map(|i| Block {
            low: [i, 0, 0],
            high: [i + 4, 4, 4],
        })
        .collect()
}

/// The unit cells of the boxes' span that `keep` keeps, given which boxes hold each.
#[must_use]
pub fn unit_cells(blocks: &[Block], keep: impl Fn(&[bool]) -> bool) -> i128 {
    let (mut low, mut high) = ([i64::MAX; 3], [i64::MIN; 3]);
    for b in blocks {
        for ((l, h), (bl, bh)) in low.iter_mut().zip(&mut high).zip(b.low.iter().zip(&b.high)) {
            *l = (*l).min(*bl);
            *h = (*h).max(*bh);
        }
    }
    let [x0, y0, z0] = low;
    let [x1, y1, z1] = high;
    let mut kept = 0_i128;
    let mut held = Vec::with_capacity(blocks.len());
    for x in x0..x1 {
        for y in y0..y1 {
            for z in z0..z1 {
                held.clear();
                held.extend(blocks.iter().map(|b| b.holds([x, y, z])));
                kept = kept.saturating_add(i128::from(keep(&held)));
            }
        }
    }
    kept
}

/// The flat reference's solid of a box.
///
/// # Errors
///
/// As `Solid::cuboid`, for a box with no extent.
pub fn box_solid(block: &Block) -> Result<Solid, ReferenceError> {
    let ([x0, y0, z0], [x1, y1, z1]) = (block.low, block.high);
    Solid::cuboid(&point(x0, y0, z0), &point(x1, y1, z1))
}

/// The folder of the stored corpus, under this crate.
macro_rules! model {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!(
                "../corpora/manifold-models/test/models/",
                $name,
                ".obj"
            )),
        )
    };
}

/// Manifold's test models (`test/models` at the commit the register pins), each its name and its OBJ text, in the
/// folder's byte order.
pub const MODELS: [(&str, &str); 30] = [
    model!("Cray_left"),
    model!("Cray_right"),
    model!("Generic_Twin_7081.1.t0_left"),
    model!("Generic_Twin_7081.1.t0_right"),
    model!("Generic_Twin_7863.1.t0_left"),
    model!("Generic_Twin_7863.1.t0_right"),
    model!("Havocglass8_left"),
    model!("Havocglass8_right"),
    model!("Offset1"),
    model!("Offset2"),
    model!("Offset3"),
    model!("Offset4"),
    model!("dedupe_stale_duplicate"),
    model!("degenerate_split_quad_first"),
    model!("degenerate_split_quad_second"),
    model!("degenerate_split_quad_shared_a"),
    model!("degenerate_split_quad_shared_b"),
    model!("degenerate_split_quad_shared_c"),
    model!("degenerate_swap_cycle_first"),
    model!("degenerate_swap_cycle_second"),
    model!("degenerate_swap_loop_a"),
    model!("degenerate_swap_loop_b"),
    model!("degenerate_swap_loop_c"),
    model!("hull-body"),
    model!("hull-mask"),
    model!("openscad-nonmanifold-crash"),
    model!("r1"),
    model!("r2"),
    model!("self_intersectA"),
    model!("self_intersectB"),
];

/// A model on the grid, moved by `by`, as `geom-mesh`'s builder takes it: one face per OBJ face,
/// its corners in the file's order, every face of one run named `name`.
///
/// # Errors
///
/// A corner that leaves the grid's frame once moved, or more faces or corners than a `u32` counts.
pub fn mesh_polygons(grid: &Grid, by: [i64; 3], name: u8) -> Result<Polygons, String> {
    let vertices = grid
        .points
        .iter()
        .map(|p| {
            let at = |k: usize| {
                p.get(k)
                    .zip(by.get(k))
                    .and_then(|(x, d)| x.checked_add(*d))
                    .ok_or("a coordinate past i64")
            };
            P3::new(at(0)?, at(1)?, at(2)?)
                .map(Corner::Grid)
                .map_err(|e| format!("{e:?}"))
        })
        .collect::<Result<Vec<Corner>, String>>()?;
    let mut corners = Vec::new();
    let mut loop_start = vec![0_u32];
    for face in &grid.faces {
        corners.extend_from_slice(face);
        loop_start.push(u32::try_from(corners.len()).map_err(|e| e.to_string())?);
    }
    let faces = u32::try_from(grid.faces.len()).map_err(|e| e.to_string())?;
    Ok(Polygons {
        vertices,
        corners,
        loop_start,
        face_loops: (0..=faces).collect(),
        face_run: vec![0; grid.faces.len()],
        runs: vec![Source {
            name: NameId::from_bytes([name; 16]),
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

/// A model on the grid, moved by `by`, as the flat reference's solid.
///
/// # Errors
///
/// As [`Polygon::new`] and [`Solid::new`]: a face that rounded to no area or out of one plane, or
/// edges that no longer pair.
pub fn mesh_solid(grid: &Grid, by: [i64; 3]) -> Result<Solid, ReferenceError> {
    let [dx, dy, dz] = by;
    let faces = grid
        .faces
        .iter()
        .map(|face| {
            let corners = face
                .iter()
                .map(|&i| {
                    let [x, y, z] = grid
                        .points
                        .get(usize::try_from(i).map_err(|_| ReferenceError::NoArea)?)
                        .copied()
                        .ok_or(ReferenceError::NoArea)?;
                    Ok(point(
                        x.saturating_add(dx),
                        y.saturating_add(dy),
                        z.saturating_add(dz),
                    ))
                })
                .collect::<Result<Vec<_>, ReferenceError>>()?;
            Polygon::new(corners)
        })
        .collect::<Result<Vec<Polygon>, ReferenceError>>()?;
    Solid::new(faces)
}
