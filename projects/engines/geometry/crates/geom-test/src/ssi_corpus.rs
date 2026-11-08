//! The surface-surface intersection corpus: 119 pairs of surfaces whose exact intersection is known,
//! read as plain data for the benchmark of `geom-intersect` (chapter 12.17; piece S6).
//!
//! What it does: [`cases`] reads `data/ssi-corpus.json`, one [`Case`] for each pair: its class, the
//! two surfaces as the corpus gives them ([`Surface`]: plane, cylinder, cone, sphere, torus or
//! NURBS, in the corpus' own units and in degrees), the coverage tolerance, the expected topology
//! (branches with their kind and whether they close, isolated points, singular points, coincident
//! regions) and whether `SMLib`'s `GlobalSurfaceIntersect` passes the case ([`Cpp`]). The file is the
//! corpus slimmed: the densely sampled curves are cut to nine points a branch and a region keeps
//! its point count (`data/slim-ssi-corpus.py` makes it from the corpus). Placing a case on the
//! grid and running the kernel over it is the benchmark's work, in `tests/ssi_corpus.rs`; this
//! module holds no kernel type.
//!
//! Guarantees:
//!
//! - **Nothing is guessed.** Unknown fields are refused, and a case file that does not read is an
//!   error, never a shorter corpus.
//! - **A benchmark, not a gate.** Nothing here decides whether a kernel passes; the corpus' pass
//!   count for `SMLib` (81 of 119) is held as data, in [`Case::cpp`].
//! - **The count is the corpus'.** [`cases`] returns 119 cases, each id once.
//!
//! Method: jyviko's exact-ground-truth corpus, as chapter 12.17 names it, the cases' surfaces and
//! topology taken as they are. Where `SMLib` passes each case comes from the text of solidcore issue
//! #17 (the 38 cases it names as not passing), listed in `data/slim-ssi-corpus.py`. The Yang, Jia
//! and Yan cases derive from a CC-BY 4.0 paper: F. Yang, Z. Jia and D.-M. Yan, "Topology
//! Guaranteed B-Spline Surface/Surface Intersection", ACM Transactions on Graphics 42(6), Article
//! 211, 2023, <https://doi.org/10.1145/3618349>; every such case has "Y23" in its class and `y23_`
//! in its id.
//! Source: PORT from ssi-corpus (the 119 cases of `cases/data`), MIT OR Apache-2.0,
//!   <https://github.com/jyviko/ssi-corpus/tree/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9/cases/data>
//!   Copyright (c) 2026 `SolidCraft` contributors
//!   Changed for Spare Kit: slimmed to one file (every branch cut to at most nine points, each
//!   region to its point count, the truth functions left out), each case given `SMLib`'s result from
//!   solidcore issue #17, and read by this module in place of the corpus' Python scorer.
//! SPDX-License-Identifier: MIT OR Apache-2.0
//!
//! Cost: reading the file is one pass over 380 KB.

use std::collections::BTreeSet;

use serde::Deserialize;

/// The corpus file, read at compile time so the reader needs no file access when it runs
/// (wasm32-wasip1 under Wasmtime has none).
const DATA: &str = include_str!("../data/ssi-corpus.json");

/// The number of cases in the corpus.
pub const CASES: usize = 119;

/// How many cases `SMLib`'s `GlobalSurfaceIntersect` passes at the corpus' tolerance 1e-6.
pub const CPP_PASSES: usize = 81;

/// Whether `SMLib` passes a case.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Cpp {
    /// It passes.
    Pass,
    /// It does not: incomplete, wrong topology or an error.
    Fail,
}

/// One surface, as the corpus gives it: lengths in the corpus' units, angles in degrees, each frame
/// by its x and y axes.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Surface {
    /// The plane `origin + u·xaxis + v·yaxis`.
    Plane {
        /// A point of the plane.
        origin: [f64; 3],
        /// The direction of u.
        xaxis: [f64; 3],
        /// The direction of v.
        yaxis: [f64; 3],
        /// The range of u.
        u: [f64; 2],
        /// The range of v.
        v: [f64; 2],
    },
    /// A cylinder about the frame's z axis.
    Cylinder {
        /// A point of the axis.
        origin: [f64; 3],
        /// The direction of angle zero.
        xaxis: [f64; 3],
        /// The direction of angle ninety degrees.
        yaxis: [f64; 3],
        /// The radius.
        radius: f64,
        /// The range of the angle.
        angle: [f64; 2],
        /// The range of the height along the axis.
        height: [f64; 2],
    },
    /// A single-nappe cone about the frame's z axis, its apex at the origin.
    Cone {
        /// The apex.
        apex: [f64; 3],
        /// The direction of angle zero.
        xaxis: [f64; 3],
        /// The direction of angle ninety degrees.
        yaxis: [f64; 3],
        /// The half-angle at the apex.
        half_angle_deg: f64,
        /// The range of the height along the axis.
        height: [f64; 2],
    },
    /// A sphere about the frame's origin.
    Sphere {
        /// The centre.
        center: [f64; 3],
        /// The direction of longitude zero.
        xaxis: [f64; 3],
        /// The direction of longitude ninety degrees.
        yaxis: [f64; 3],
        /// The radius.
        radius: f64,
        /// The range of the longitude.
        lon: [f64; 2],
        /// The range of the latitude.
        lat: [f64; 2],
    },
    /// A torus about the frame's z axis.
    Torus {
        /// The centre.
        center: [f64; 3],
        /// The direction of angle zero.
        xaxis: [f64; 3],
        /// The direction of angle ninety degrees.
        yaxis: [f64; 3],
        /// The major radius.
        #[serde(rename = "R")]
        major_radius: f64,
        /// The minor radius.
        #[serde(rename = "r")]
        minor_radius: f64,
        /// The range of the angle about the axis.
        major: [f64; 2],
        /// The range of the angle about the tube, zero at the outer equator.
        minor: [f64; 2],
    },
    /// A tensor-product NURBS surface, its net row by row in u.
    Nurbs {
        /// What the corpus calls it.
        label: String,
        /// The degrees in u and v.
        degree: [usize; 2],
        /// The knots in u.
        knots_u: Vec<f64>,
        /// The knots in v.
        knots_v: Vec<f64>,
        /// The control points in u.
        nu: usize,
        /// The control points in v.
        nv: usize,
        /// The net, point (i, j) at i·nv + j.
        points: Vec<[f64; 3]>,
        /// The weights in the same order, absent for a polynomial surface.
        weights: Option<Vec<f64>>,
    },
}

/// One branch of the exact intersection.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Branch {
    /// What the corpus calls it: `crossing`, `tangent`, `near_tangent`, `boundary` or
    /// `coincident_border`.
    pub kind: String,
    /// Whether it is a closed loop.
    pub closed: bool,
    /// How many points the corpus samples it at.
    pub samples: usize,
    /// At most nine of them, evenly spaced by index, both ends included.
    pub points: Vec<[f64; 3]>,
}

/// One case.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Its name.
    pub id: String,
    /// Its class, such as `1 transversal` or `7 near-degenerate`.
    pub class: String,
    /// What it is, in words.
    pub title: String,
    /// The distance within which the corpus' scorer holds a curve to the truth.
    pub cov_tol: f64,
    /// The two surfaces.
    pub surfaces: Vec<Surface>,
    /// The branches of the exact intersection.
    pub branches: Vec<Branch>,
    /// Where the surfaces touch at a point without crossing.
    pub isolated_points: Vec<[f64; 3]>,
    /// Where several branches meet.
    pub singular_points: Vec<[f64; 3]>,
    /// How many regions the surfaces share.
    pub coincident_regions: usize,
    /// Whether `SMLib` passes it.
    pub cpp: Cpp,
}

/// Why the corpus did not read.
#[derive(Debug)]
pub enum CorpusError {
    /// The file is not the corpus' form.
    Json(serde_json::Error),
    /// A count or an id is not the corpus'.
    Shape(&'static str),
}

/// The 119 cases, in the corpus' order of ids.
///
/// # Errors
///
/// [`CorpusError::Json`] if the file does not read; [`CorpusError::Shape`] if it does not hold
/// [`CASES`] cases with distinct ids and two surfaces each, [`CPP_PASSES`] of them `SMLib`'s passes.
pub fn cases() -> Result<Vec<Case>, CorpusError> {
    let cases: Vec<Case> = serde_json::from_str(DATA).map_err(CorpusError::Json)?;
    if cases.len() != CASES {
        return Err(CorpusError::Shape("the corpus is not 119 cases"));
    }
    let ids: BTreeSet<&str> = cases.iter().map(|c| c.id.as_str()).collect();
    if ids.len() != CASES {
        return Err(CorpusError::Shape("two cases share an id"));
    }
    if cases.iter().any(|c| c.surfaces.len() != 2) {
        return Err(CorpusError::Shape("a case does not have two surfaces"));
    }
    if cases.iter().filter(|c| c.cpp == Cpp::Pass).count() != CPP_PASSES {
        return Err(CorpusError::Shape("SMLib does not pass 81 cases"));
    }
    Ok(cases)
}
