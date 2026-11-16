//! A test-only reader of STL meshes onto the grid, and of EMBER's case list: the way in for
//! EMBER's boolean pairs (Gate 4, G156; the fetched corpus `ember-cleared-pairs`), before the
//! kernel's own STL importer (`geom-meshio`, chapter 24.8) exists.
//!
//! What it does: [`read`] reads a binary or ASCII STL file with the `stl_io` crate, which welds
//! corners only where their three `f32` values are bit for bit the same (24.8's rule), into
//! [`Stl`]. [`cases`] reads EMBER's case list (Manifold's `ember-benchmark-cases.json`): each pair
//! of `Thingi10K` file ids with the affine transform EMBER moves each model by. [`placed`] moves a
//! model by its transform exactly, into the exact decimals of `geom_test::obj`, and
//! [`on_one_grid`] puts the models of one pair on the grid at one scale, as `obj::Read::to_grid`
//! does for one model, so the pair's two meshes meet on one grid:
//!
//! - **The transform.** Each corner `p` goes to `M p + t`, `M` the transform's first three columns
//!   and `t` its fourth, every product and sum in exact rationals: an `f32` corner and an `f64`
//!   entry (the value a JSON reader gives the written number, as EMBER's own reader does) are both
//!   exact binary fractions, so the moved corner is one too, and so an exact decimal.
//! - **The scale and the rounding** are `obj::Read::to_grid`'s, over both models together: steps
//!   of `10^-k`, `k` the largest whole number keeping every coordinate of the pair within
//!   `obj::REACH`, each coordinate rounded once to the nearest step, a tie to the even one.
//!
//! Guarantees:
//!
//! - **Exact up to one stated rounding.** No coordinate passes through floating-point arithmetic
//!   after the file and the list: the grid value is the exact moved corner rounded once.
//! - **Refused, never guessed.** A file `stl_io` cannot read, a value that is not finite, a
//!   transform whose last row is not `0 0 0 1`, and more corners or faces than a `u32` counts are
//!   each an [`StlError`]; a case list of another shape is an error with serde's message.
//! - **The same on every target.** Integer and rational arithmetic only, after the file's bits.
//!
//! Method: the STL format as `stl_io` reads it (3D Systems' Stereolithography Interface
//! Specification, 1989, as summarised at <https://www.fabbers.com/tech/STL_Format>); EMBER's case
//! list and its transforms as Manifold's `extras/ember_tests/man_bench.cpp` reads them (Manifold
//! at d056b8fb70de8c8e6d6bee2e639b71c20aa08d01, Apache-2.0: column-major 4 by 4, each model moved
//! by its own, then the first less the second); an `f32` or `f64` value as the exact binary
//! fraction IEEE 754-2019 defines (<https://doi.org/10.1109/IEEESTD.2019.8766229>).
//!
//! Why a crate: `stl_io` 0.11.0 (MIT, no dependencies of its own) is the usual Rust STL reader,
//! and STL's values are `f32` already, so reading them as `f32` loses nothing (unlike OBJ's
//! decimals, which `geom_test::obj` reads itself).
//!
//! Cost: linear in the file for [`read`] and the list for [`cases`]; [`placed`] costs a few
//! rational products a corner, and [`on_one_grid`] `obj::Read::to_grid`'s.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision integers and rationals, which cannot.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision integers and rationals, which cannot overflow"
)]

use core::fmt;
use std::io::Cursor;

use dashu_int::IBig;
use dashu_ratio::RBig;
use serde::Deserialize;

use crate::obj::{Decimal, Grid, Read};

/// A mesh as an STL file gives it, its corners welded only where bit for bit the same.
#[derive(Clone, Debug, PartialEq)]
pub struct Stl {
    /// The corners, in the order the file first gives each.
    pub vertices: Vec<[f32; 3]>,
    /// The triangles, each its corners as indices into `vertices`.
    pub faces: Vec<[u32; 3]>,
}

/// Why a mesh or a transform was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StlError {
    /// `stl_io` could not read the file (its message).
    Read(String),
    /// More corners or faces than a `u32` counts.
    TooMany,
    /// A coordinate or a transform entry that is not a finite number.
    NotFinite,
    /// A transform whose last row is not `0 0 0 1`.
    NotAffine,
}

impl fmt::Display for StlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StlError::Read(e) => write!(f, "not an STL file: {e}"),
            StlError::TooMany => write!(f, "more corners or faces than a u32 counts"),
            StlError::NotFinite => write!(f, "a value that is not finite"),
            StlError::NotAffine => write!(f, "a transform whose last row is not 0 0 0 1"),
        }
    }
}

impl std::error::Error for StlError {}

/// Reads an STL file, binary or ASCII.
///
/// # Errors
///
/// [`StlError::Read`] for a file `stl_io` refuses, [`StlError::TooMany`] past a `u32`.
pub fn read(bytes: &[u8]) -> Result<Stl, StlError> {
    let mesh =
        stl_io::read_stl(&mut Cursor::new(bytes)).map_err(|e| StlError::Read(e.to_string()))?;
    let index = |i: usize| u32::try_from(i).map_err(|_| StlError::TooMany);
    let faces = mesh
        .faces
        .iter()
        .map(|t| {
            let [a, b, c] = t.vertices;
            Ok([index(a)?, index(b)?, index(c)?])
        })
        .collect::<Result<Vec<_>, StlError>>()?;
    u32::try_from(mesh.vertices.len()).map_err(|_| StlError::TooMany)?;
    Ok(Stl {
        vertices: mesh.vertices.iter().map(|v| v.0).collect(),
        faces,
    })
}

/// One column of a transform, as the case list writes it.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    /// Its first entry.
    pub x: f64,
    /// Its second.
    pub y: f64,
    /// Its third.
    pub z: f64,
    /// Its fourth.
    pub w: f64,
}

/// A 4 by 4 transform, column by column: the first three columns the linear part, the fourth the
/// translation.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    /// The first column.
    pub col0: Column,
    /// The second.
    pub col1: Column,
    /// The third.
    pub col2: Column,
    /// The fourth.
    pub col3: Column,
}

/// One pair of EMBER's case list.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// The first model's `Thingi10K` file id.
    pub id_a: u64,
    /// The second's.
    pub id_b: u64,
    /// What moves the first.
    pub transform_a: Transform,
    /// What moves the second.
    pub transform_b: Transform,
}

/// Reads EMBER's case list, in its order: the place of a case is its pair's number.
///
/// # Errors
///
/// serde's message for a list of another shape.
pub fn cases(json: &str) -> Result<Vec<Case>, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// An `f32` or `f64` value as the exact rational it is.
fn exact(value: f64) -> Result<RBig, StlError> {
    RBig::try_from(value).map_err(|_| StlError::NotFinite)
}

/// An exact binary fraction as the exact decimal it is: `n / 2^a` is `n 5^a × 10^-a`.
fn decimal(q: &RBig) -> Result<Decimal, StlError> {
    let a = q.denominator().trailing_zeros().unwrap_or(0);
    if q.denominator() != &(dashu_int::UBig::ONE << a) {
        // Unreachable for binary fractions; refused rather than rounded.
        return Err(StlError::NotFinite);
    }
    Ok(Decimal {
        digits: q.numerator() * IBig::from(5_u8).pow(a),
        exponent: -i64::try_from(a).map_err(|_| StlError::TooMany)?,
    })
}

/// `stl` moved by `by`, exactly, as exact decimals.
///
/// # Errors
///
/// [`StlError::NotAffine`] for a last row other than `0 0 0 1`, [`StlError::NotFinite`] for a
/// value that is not finite.
pub fn placed(stl: &Stl, by: &Transform) -> Result<Read, StlError> {
    let columns = [by.col0, by.col1, by.col2, by.col3];
    if columns.map(|c| c.w) != [0.0, 0.0, 0.0, 1.0] {
        return Err(StlError::NotAffine);
    }
    let entries: [fn(&Column) -> f64; 3] = [|c| c.x, |c| c.y, |c| c.z];
    let rows: Vec<[RBig; 4]> = entries
        .iter()
        .map(|entry| {
            let [c0, c1, c2, c3] = columns.each_ref().map(entry);
            Ok([exact(c0)?, exact(c1)?, exact(c2)?, exact(c3)?])
        })
        .collect::<Result<_, StlError>>()?;
    let mut vertices = Vec::with_capacity(stl.vertices.len());
    for [x, y, z] in &stl.vertices {
        let p = [
            exact(f64::from(*x))?,
            exact(f64::from(*y))?,
            exact(f64::from(*z))?,
        ];
        let mut moved = Vec::with_capacity(3);
        for [m0, m1, m2, t] in &rows {
            let [px, py, pz] = &p;
            moved.push(decimal(&(m0 * px + m1 * py + m2 * pz + t))?);
        }
        let [a, b, c]: [Decimal; 3] = moved.try_into().map_err(|_| StlError::TooMany)?;
        vertices.push([a, b, c]);
    }
    Ok(Read {
        vertices,
        faces: stl.faces.iter().map(|f| f.to_vec()).collect(),
    })
}

/// The models of one pair on the grid at one scale, `obj::Read::to_grid`'s over all of them, in
/// their order; `None` if every coordinate is zero or a count passes a `u32`.
#[must_use]
pub fn on_one_grid(models: &[Read]) -> Option<Vec<Grid>> {
    let mut all = Read {
        vertices: Vec::new(),
        faces: Vec::new(),
    };
    let mut starts = Vec::with_capacity(models.len());
    for model in models {
        let start = u32::try_from(all.vertices.len()).ok()?;
        starts.push((all.vertices.len(), model.vertices.len(), model.faces.len()));
        all.vertices.extend(model.vertices.iter().cloned());
        for face in &model.faces {
            all.faces.push(
                face.iter()
                    .map(|i| i.checked_add(start))
                    .collect::<Option<Vec<u32>>>()?,
            );
        }
    }
    let grid = all.to_grid()?;
    let mut out = Vec::with_capacity(models.len());
    let mut face_at = 0_usize;
    for (start, count, faces) in starts {
        let offset = u32::try_from(start).ok()?;
        let end = start.checked_add(count)?;
        let face_end = face_at.checked_add(faces)?;
        out.push(Grid {
            points: grid.points.get(start..end)?.to_vec(),
            faces: grid
                .faces
                .get(face_at..face_end)?
                .iter()
                .map(|f| f.iter().map(|i| i.checked_sub(offset)).collect())
                .collect::<Option<Vec<Vec<u32>>>>()?,
            scale: grid.scale,
        });
        face_at = face_end;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "test code: a failure should stop loudly"
    )]

    use super::*;

    /// A binary STL of `triangles`, each three corners.
    fn binary(triangles: &[[[f32; 3]; 3]]) -> Vec<u8> {
        let mut out = vec![0_u8; 80];
        out.extend(u32::try_from(triangles.len()).unwrap().to_le_bytes());
        for t in triangles {
            out.extend([0_u8; 12]);
            for p in t {
                for x in p {
                    out.extend(x.to_le_bytes());
                }
            }
            out.extend([0_u8; 2]);
        }
        out
    }

    const IDENTITY: Transform = Transform {
        col0: Column {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        col1: Column {
            x: 0.0,
            y: 1.0,
            z: 0.0,
            w: 0.0,
        },
        col2: Column {
            x: 0.0,
            y: 0.0,
            z: 1.0,
            w: 0.0,
        },
        col3: Column {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
    };

    #[test]
    fn corners_are_welded_only_when_bit_for_bit_the_same() {
        let bytes = binary(&[
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-0.0, 0.0, 0.0]],
        ]);
        let stl = read(&bytes).unwrap();
        // -0.0 is not 0.0 bit for bit, so four corners.
        assert_eq!(stl.vertices.len(), 4);
        assert_eq!(stl.faces, vec![[0, 1, 2], [1, 2, 3]]);
        assert!(matches!(read(&[1, 2, 3]), Err(StlError::Read(_))));
    }

    #[test]
    fn a_transform_moves_exactly_and_the_pair_shares_one_scale() {
        let stl = read(&binary(&[[
            [0.5, 0.0, 0.0],
            [0.0, 0.1, 0.0],
            [0.0, 0.0, 3.0],
        ]]))
        .unwrap();
        let mut by = IDENTITY;
        by.col3 = Column {
            x: 2.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        };
        by.col0.x = 0.25;
        let moved = placed(&stl, &by).unwrap();
        // 0.25 × 0.5 + 2, exactly.
        assert_eq!(
            moved.vertices[0][0],
            Decimal {
                digits: IBig::from(2125),
                exponent: -3
            }
        );
        // f32's 0.1 is 13421773 / 2^27 exactly, never rounded to 0.1.
        let tenth = &moved.vertices[1][1];
        assert_eq!(
            RBig::from_parts(
                tenth.digits.clone(),
                dashu_int::UBig::from(10_u8).pow(usize::try_from(-tenth.exponent).unwrap())
            ),
            RBig::from_parts(IBig::from(13_421_773), dashu_int::UBig::ONE << 27)
        );
        let still = placed(&stl, &IDENTITY).unwrap();
        let grids = on_one_grid(&[moved.clone(), still.clone()]).unwrap();
        let alone = moved.to_grid().unwrap();
        assert_eq!(grids[0].scale, grids[1].scale);
        assert_eq!(
            grids[0].scale,
            alone.scale.min(still.to_grid().unwrap().scale)
        );
        assert_eq!(grids[0].faces, vec![vec![0, 1, 2]]);
        assert_eq!(grids[1].faces, vec![vec![0, 1, 2]]);
        assert_eq!(grids[0].points.len(), 3);
        let mut projective = IDENTITY;
        projective.col0.w = 1.0;
        assert_eq!(placed(&stl, &projective), Err(StlError::NotAffine));
        let mut infinite = IDENTITY;
        infinite.col1.y = f64::INFINITY;
        assert_eq!(placed(&stl, &infinite), Err(StlError::NotFinite));
    }

    #[test]
    fn the_case_list_reads_as_ember_writes_it() {
        let column = |x: f64, w: f64| format!(r#"{{"x": {x}, "y": 0, "z": 0, "w": {w}}}"#);
        let transform = format!(
            r#"{{"col0": {}, "col1": {}, "col2": {}, "col3": {}}}"#,
            column(1.0, 0.0),
            column(0.0, 0.0),
            column(0.0, 0.0),
            column(5.0, 1.0)
        );
        let json = format!(
            r#"[{{"id_a": 7, "id_b": 9, "transform_a": {transform}, "transform_b": {transform}}}]"#
        );
        let read = cases(&json).unwrap();
        assert_eq!((read[0].id_a, read[0].id_b), (7, 9));
        assert_eq!(read[0].transform_a.col3.x, 5.0);
        assert!(cases(&json.replace("id_b", "id_c")).is_err());
    }
}
