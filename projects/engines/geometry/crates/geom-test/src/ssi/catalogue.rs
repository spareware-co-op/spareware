//! The intersection failure catalogues, rebuilt with inputs of our own (chapter 12.17; piece G160).
//!
//! What it does: [`all`] builds one case for each intersection failure the vetted kernels publish:
//! `SMLib`'s issues #1 and #16, the loops its search drops near tangency, the single candidate per
//! patch pair, its fixed tolerances, truncated traces, and the seam and pole defects of its known
//! issues and `OpenCascade`'s own list. Every input is new: the shapes are chosen here, and each truth
//! is a closed form computed here. Nothing is taken from those projects but the failure's
//! description, and each case's `notes` says which it rebuilds. The cases use the corpus's schema,
//! so the corpus's scorer and [`super::gate`] judge them as they do the corpus.
//!
//! Guarantees:
//!
//! - **Closed-form truth**: every sample of every case comes from the case's formula, and
//!   `tests/ssi.rs` holds each to both surfaces.
//! - **Our own inputs**: no fixture of any outside project is read, here or in the tests.
//!
//! Method: the failure lists in the evidence review
//! (`projects/kit/notes/G01-evidence/13-vetting-smlib.md`) and chapter 12.17's "What we take"
//! (`projects/engines/geometry/architecture/12-intersections.md`); the closed forms are elementary
//! geometry, each stated in its case's `formula`.
//!
//! Cost: linear in the samples; a few thousand per case.

use core::f64::consts::PI;
use std::fs;
use std::path::Path;

use geom_num::{Real, math};

use super::case::{
    Branch, Case, Contact, Expected, Implicit, Shared, Surface, V3, along, cross, unit,
};
use super::{SsiError, cos_deg, cos_half, sin_deg, sin_half};

/// Samples along one curve.
const SAMPLES: u32 = 1_500;

/// `t` from 0 to 1 in [`SAMPLES`] steps.
fn steps() -> impl Iterator<Item = f64> {
    (0..=SAMPLES).map(|i| f64::from(i) / f64::from(SAMPLES))
}

/// A crossing curve from `f` over [0, 1].
fn curve(kind: Contact, closed: bool, f: impl Fn(f64) -> V3) -> Branch {
    Branch {
        closed,
        kind,
        points: steps().map(f).collect(),
    }
}

/// The line from `a` to `b`.
fn line(kind: Contact, a: V3, b: V3) -> Branch {
    curve(kind, false, |t| {
        along(a, t, [b[0] - a[0], b[1] - a[1], b[2] - a[2]])
    })
}

/// A full plane patch's data: `origin` + u·`x` + v·`y`.
fn plane(origin: V3, x: V3, y: V3, u: [f64; 2], v: [f64; 2]) -> (Surface, Implicit) {
    (
        Surface::Plane {
            origin,
            xaxis: x,
            yaxis: y,
            u,
            v,
        },
        Implicit::Plane {
            point: origin,
            normal: cross(x, y),
        },
    )
}

/// A whole sphere, poles along z.
fn sphere(center: V3, radius: f64) -> (Surface, Implicit) {
    (
        Surface::Sphere {
            center,
            xaxis: [1.0, 0.0, 0.0],
            yaxis: [0.0, 1.0, 0.0],
            radius,
            lon: [0.0, 360.0],
            lat: [-90.0, 90.0],
        },
        Implicit::Sphere { center, radius },
    )
}

/// A whole cylinder about the axis `x` × `y`.
fn cylinder(origin: V3, x: V3, y: V3, radius: f64, height: [f64; 2]) -> (Surface, Implicit) {
    (
        Surface::Cylinder {
            origin,
            xaxis: x,
            yaxis: y,
            radius,
            angle: [0.0, 360.0],
            height,
        },
        Implicit::Cylinder {
            point: origin,
            axis: cross(x, y),
            radius,
        },
    )
}

/// What a case states beside its surfaces.
struct About {
    id: &'static str,
    class: &'static str,
    title: &'static str,
    rebuilds: &'static str,
    formula: &'static str,
    cov_tol: f64,
    feature_size: Option<f64>,
}

/// A case.
fn case(about: &About, a: (Surface, Implicit), b: (Surface, Implicit), expected: Expected) -> Case {
    Case {
        id: about.id.to_owned(),
        class: about.class.to_owned(),
        title: about.title.to_owned(),
        surfaces: [a.0, b.0],
        truth: [a.1, b.1],
        surface_labels: ["first".to_owned(), "second".to_owned()],
        expected,
        cov_tol: about.cov_tol,
        notes: format!("Rebuilds, with our own inputs: {}", about.rebuilds),
        formula: about.formula.to_owned(),
        feature_size: about.feature_size,
        ill_cond: None,
        source: None,
    }
}

/// The curves of a case with nothing else in it.
fn curves(branches: Vec<Branch>) -> Expected {
    Expected {
        branches,
        ..Expected::default()
    }
}

const X: V3 = [1.0, 0.0, 0.0];
const Y: V3 = [0.0, 1.0, 0.0];
const Z: V3 = [0.0, 0.0, 1.0];

/// `SMLib` #16: wrong results and errors on tangent cylinders.
fn tangent_cylinders() -> Vec<Case> {
    let about = |id, title, formula| About {
        id,
        class: "2 tangent curve",
        title,
        rebuilds: "SMLib issue #16, tangent cylinders",
        formula,
        cov_tol: 1e-2,
        feature_size: None,
    };
    let tall = [-2.0, 2.0];
    vec![
        case(
            &about(
                "f16_cylinders_external",
                "two equal cylinders side by side, touching along a line",
                "x = 0, y = 0, |z| <= 2",
            ),
            cylinder([-1.0, 0.0, 0.0], X, Y, 1.0, tall),
            cylinder([1.0, 0.0, 0.0], X, Y, 1.0, tall),
            curves(vec![line(
                Contact::Tangent,
                [0.0, 0.0, -2.0],
                [0.0, 0.0, 2.0],
            )]),
        ),
        case(
            &about(
                "f16_cylinders_internal",
                "a cylinder inside a wider one, touching along a line",
                "x = 2, y = 0, |z| <= 2",
            ),
            cylinder([0.0; 3], X, Y, 2.0, tall),
            cylinder([1.0, 0.0, 0.0], X, Y, 1.0, tall),
            curves(vec![line(
                Contact::Tangent,
                [2.0, 0.0, -2.0],
                [2.0, 0.0, 2.0],
            )]),
        ),
        case(
            &about(
                "f16_cylinders_staggered",
                "two equal cylinders at different heights, touching along part of a line",
                "x = 0, y = 0, 0 <= z <= 1",
            ),
            cylinder([-1.0, 0.0, 0.0], X, Y, 1.0, [-2.0, 1.0]),
            cylinder([1.0, 0.0, 0.0], X, Y, 1.0, [0.0, 3.0]),
            curves(vec![line(Contact::Tangent, [0.0; 3], [0.0, 0.0, 1.0])]),
        ),
    ]
}

/// `SMLib` #1: a mirrored operand dropped without a word.
fn mirrored() -> Vec<Case> {
    let root3 = 3.0_f64.sqrt();
    let mirror = [0.6, 0.8, 0.0];
    let across = [-0.8, 0.6, 0.0];
    let circle = move |t: f64| {
        along(
            along([0.0; 3], root3 * cos_half(2.0 * t), across),
            root3 * sin_half(2.0 * t),
            Z,
        )
    };
    let spheres = case(
        &About {
            id: "f1_mirrored_spheres",
            class: "1 transversal",
            title: "a sphere and its image in a slanted mirror",
            rebuilds: "SMLib issue #1, a mirrored operand dropped",
            formula: "a circle of radius sqrt(3) in the mirror plane",
            cov_tol: 1e-3,
            feature_size: None,
        },
        sphere(along([0.0; 3], 1.0, mirror), 2.0),
        sphere(along([0.0; 3], -1.0, mirror), 2.0),
        curves(vec![curve(Contact::Crossing, true, circle)]),
    );
    // Two equal cylinders whose axes are mirror images in x = 0, at 30 degrees either side of z.
    let (sine, cosine) = (0.5, root3 / 2.0);
    let (first, second) = ([cosine, 0.0, -sine], [cosine, 0.0, sine]);
    let half = |x_scale: f64, z_scale: f64, side: f64| {
        curve(Contact::Crossing, false, move |t| {
            [
                x_scale * sin_half(t) * side,
                cos_half(t),
                z_scale * sin_half(t) * side,
            ]
        })
    };
    let cylinders = case(
        &About {
            id: "f1_mirrored_cylinders",
            class: "4 singular",
            title: "a cylinder and its mirror image, axes crossing: two ellipses meeting twice",
            rebuilds: "SMLib issue #1, a mirrored operand dropped",
            formula: "the ellipses (0, cos t, 2 sin t) and (2/sqrt(3) sin t, cos t, 0)",
            cov_tol: 1e-3,
            feature_size: None,
        },
        cylinder([0.0; 3], first, Y, 1.0, [-3.0, 3.0]),
        cylinder([0.0; 3], second, Y, 1.0, [-3.0, 3.0]),
        Expected {
            branches: vec![
                half(0.0, 1.0 / sine, 1.0),
                half(0.0, 1.0 / sine, -1.0),
                half(1.0 / cosine, 0.0, 1.0),
                half(1.0 / cosine, 0.0, -1.0),
            ],
            singular_points: vec![[0.0, 1.0, 0.0], [0.0, -1.0, 0.0]],
            ..Expected::default()
        },
    );
    let corners = [
        [0.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
    ];
    let boundary = corners
        .iter()
        .zip(corners.iter().skip(1))
        .flat_map(|(a, b)| line(Contact::Coincident, *a, *b).points)
        .collect();
    let planes = case(
        &About {
            id: "f1_mirrored_planes",
            class: "6 coincident",
            title: "two faces in one plane, one the other's mirror image, overlapping",
            rebuilds: "SMLib issue #1, a mirrored operand dropped",
            formula: "the rectangle 0 <= x <= 1, |y| <= 1, z = 0",
            cov_tol: 1e-3,
            feature_size: None,
        },
        plane([0.0; 3], X, Y, [-1.0, 1.0], [-1.0, 1.0]),
        plane([1.0, 0.0, 0.0], Y, X, [-1.0, 1.0], [-1.0, 1.0]),
        Expected {
            coincident_regions: vec![Shared { boundary }],
            ..Expected::default()
        },
    );
    vec![spheres, cylinders, planes]
}

/// `SMLib` drops candidates within 5 and 2 degrees of tangent without a signal, and an interior loop
/// rests on one sampled candidate per pair of leaf patches.
fn loops() -> Vec<Case> {
    let shallow = |id, angle: f64| {
        let (r, h) = (sin_deg(angle), cos_deg(angle));
        case(
            &About {
                id,
                class: "7 near-degenerate",
                title: "a plane cutting a small loop near a sphere's top, at a shallow angle",
                rebuilds: "SMLib's candidates dropped within 5 and 2 degrees of tangent",
                formula: "a circle of radius sin(angle) at height cos(angle)",
                cov_tol: 1e-3,
                feature_size: Some(2.0 * r),
            },
            plane([0.0, 0.0, h], X, Y, [-1.0, 1.0], [-1.0, 1.0]),
            sphere([0.0; 3], 1.0),
            curves(vec![curve(Contact::Crossing, true, move |t| {
                [r * cos_half(2.0 * t), r * sin_half(2.0 * t), h]
            })]),
        )
    };
    // A plane x = 1/2 through a ring torus, nearer its axis than its hole's edge: two ovals.
    let oval = |side: f64| {
        curve(Contact::Crossing, true, move |t| {
            let rho = 2.0 + cos_half(2.0 * t);
            [0.5, side * (rho * rho - 0.25).sqrt(), sin_half(2.0 * t)]
        })
    };
    let ovals = case(
        &About {
            id: "f_two_ovals",
            class: "1 transversal",
            title: "a plane through a torus's hole: two ovals",
            rebuilds: "SMLib's one sampled candidate per pair of leaf patches",
            formula: "(1/2, +-sqrt((2 + cos p)^2 - 1/4), sin p)",
            cov_tol: 1e-3,
            feature_size: None,
        },
        plane([0.5, 0.0, 0.0], Y, Z, [-4.0, 4.0], [-2.0, 2.0]),
        (
            Surface::Torus {
                center: [0.0; 3],
                xaxis: X,
                yaxis: Y,
                big: 2.0,
                small: 1.0,
                major: [0.0, 360.0],
                minor: [0.0, 360.0],
            },
            Implicit::Torus {
                point: [0.0; 3],
                axis: Z,
                big: 2.0,
                small: 1.0,
            },
        ),
        curves(vec![oval(1.0), oval(-1.0)]),
    );
    vec![
        shallow("f5_loop_3deg", 3.0),
        shallow("f2_loop_1deg5", 1.5),
        ovals,
    ]
}

/// `SMLib`'s tolerances are fixed fractions of a surface's size, and its truncated traces return
/// success.
fn sizes() -> Vec<Case> {
    let scaled = |id, r: f64| {
        let h = r / 2.0;
        let rho = r * 3.0_f64.sqrt() / 2.0;
        case(
            &About {
                id,
                class: "1 transversal",
                title: "a plane and a sphere, at a size far from one",
                rebuilds: "SMLib's tolerances fixed at 1/1000 of a surface's size",
                formula: "a circle of radius sqrt(3)/2 r at height r/2",
                cov_tol: r / 1_024.0,
                feature_size: None,
            },
            plane([0.0, 0.0, h], X, Y, [-r, r], [-r, r]),
            sphere([0.0; 3], r),
            curves(vec![curve(Contact::Crossing, true, move |t| {
                [rho * cos_half(2.0 * t), rho * sin_half(2.0 * t), h]
            })]),
        )
    };
    // The plane x = 1/2 + z tan 1 degree through a tall cylinder: a long, thin arc over its top.
    let slope = sin_deg(1.0) / cos_deg(1.0);
    let rim = 0.5 - 30.0 * slope;
    let reach = math::atan2((1.0 - rim * rim).sqrt(), rim).map_or(0.0, Real::get) / PI;
    let normal = unit([1.0, 0.0, -slope]);
    let up = cross(normal, Y);
    let long = case(
        &About {
            id: "f_long_arc",
            class: "1 transversal",
            title: "a plane a degree off a tall cylinder's axis: one long, thin arc",
            rebuilds: "SMLib's truncated traces reported as success",
            formula: "(cos t, sin t, (cos t - 1/2) / tan 1 degree), rim to rim",
            cov_tol: 1e-3,
            feature_size: None,
        },
        plane([0.5, 0.0, 0.0], Y, up, [-2.0, 2.0], [-32.0, 32.0]),
        cylinder([0.0; 3], X, Y, 1.0, [-30.0, 30.0]),
        curves(vec![curve(Contact::Crossing, false, move |t| {
            let at = reach * (2.0 * t - 1.0);
            [cos_half(at), sin_half(at), (cos_half(at) - 0.5) / slope]
        })]),
    );
    vec![
        scaled("f_scale_small", 1.0 / 256.0),
        scaled("f_scale_large", 256.0),
        long,
    ]
}

/// The seam and pole defects of `SMLib`'s known issues, and a branch point its tracer follows only
/// one way out of.
fn seams() -> Vec<Case> {
    let tall = [-2.0, 2.0];
    let seam_line = case(
        &About {
            id: "f_seam_line",
            class: "8 periodic/seam/pole",
            title: "a plane through a cylinder's axis and along its seam",
            rebuilds: "SMLib's seam-healing defects on closed surfaces",
            formula: "the lines x = +-1, y = 0, |z| <= 2",
            cov_tol: 1e-3,
            feature_size: None,
        },
        plane([0.0; 3], X, Z, [-2.0, 2.0], [-3.0, 3.0]),
        cylinder([0.0; 3], X, Y, 1.0, tall),
        curves(vec![
            line(Contact::Crossing, [1.0, 0.0, -2.0], [1.0, 0.0, 2.0]),
            line(Contact::Crossing, [-1.0, 0.0, -2.0], [-1.0, 0.0, 2.0]),
        ]),
    );
    let pole_circle = case(
        &About {
            id: "f_pole_circle",
            class: "8 periodic/seam/pole",
            title: "a plane through a sphere's poles and along its seam",
            rebuilds: "SMLib's seam-healing defects, at the poles",
            formula: "the great circle y = 0",
            cov_tol: 1e-3,
            feature_size: None,
        },
        plane([0.0; 3], X, Z, [-2.0, 2.0], [-2.0, 2.0]),
        sphere([0.0; 3], 1.0),
        curves(vec![curve(Contact::Crossing, true, move |t| {
            [cos_half(2.0 * t), 0.0, sin_half(2.0 * t)]
        })]),
    );
    let half = 30.0;
    let (s, c) = (sin_deg(half), cos_deg(half));
    let ruling = |side: f64| line(Contact::Crossing, [0.0; 3], [side * 2.0 * s / c, 0.0, 2.0]);
    let cone = case(
        &About {
            id: "f_apex_rulings",
            class: "4 singular",
            title: "a plane through a cone's apex and axis: two rulings from one point",
            rebuilds: "SMLib's tracer, which follows only the first branch out of a singular point",
            formula: "the rulings from the apex, x = +-z tan 30 degrees, y = 0",
            cov_tol: 1e-3,
            feature_size: None,
        },
        (
            Surface::Cone {
                apex: [0.0; 3],
                xaxis: X,
                yaxis: Y,
                half_angle_deg: half,
                height: [0.0, 2.0],
            },
            Implicit::Cone {
                point: [0.0; 3],
                axis: Z,
                half_angle_deg: half,
            },
        ),
        plane([0.0; 3], X, Z, [-3.0, 3.0], [-1.0, 3.0]),
        Expected {
            branches: vec![ruling(1.0), ruling(-1.0)],
            singular_points: vec![[0.0; 3]],
            ..Expected::default()
        },
    );
    vec![seam_line, pole_circle, cone]
}

/// Every case of the catalogue.
#[must_use]
pub fn all() -> Vec<Case> {
    let mut cases = tangent_cylinders();
    cases.extend(mirrored());
    cases.extend(loops());
    cases.extend(sizes());
    cases.extend(seams());
    cases
}

/// Writes every case into `dir`, one file each, as the corpus lays out its own.
///
/// # Errors
///
/// [`SsiError::File`] if a file cannot be written.
pub fn write(dir: &Path) -> Result<(), SsiError> {
    let failed = |path: &Path, why: String| SsiError::File {
        path: path.display().to_string(),
        why,
    };
    for case in all() {
        let path = dir.join(format!("{}.json", case.id));
        let text = serde_json::to_string(&case).map_err(|e| failed(&path, e.to_string()))?;
        fs::write(&path, text).map_err(|e| failed(&path, e.to_string()))?;
    }
    Ok(())
}
