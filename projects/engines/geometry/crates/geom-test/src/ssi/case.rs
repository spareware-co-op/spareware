//! A corpus case: two surfaces, the exact intersection and the tolerance it is judged at (chapter
//! 12.17; piece G160).
//!
//! What it does: [`Case`] reads the corpus's JSON schema and writes the same schema back, which is
//! how [`super::catalogue`]'s cases reach the same scorer. [`Implicit`] is a surface's closed form,
//! which gives the distance from a point to it. [`Case::features`] lists what a case's truth is made
//! of, each with its [`Contact`].
//!
//! Guarantees:
//!
//! - **A kind or contact it does not know is an error**, except an [`Implicit`] the adapter has no
//!   need of, which reads as [`Implicit::Other`]; only the scorer uses those.
//! - **Distances are the scorer's formulas** (its `ssi_geom.py`), on the same fields.
//!
//! Method: the case schema of the ssi-corpus's README,
//! <https://github.com/jyviko/ssi-corpus/blob/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9/README.md>.
//!
//! Cost: linear in the case's text.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{SsiError, cos_deg, sin_deg};

/// A point or vector.
pub type V3 = [f64; 3];

/// `a` − `b`.
#[must_use]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a` + `k`·`b`.
#[must_use]
pub fn along(a: V3, k: f64, b: V3) -> V3 {
    [a[0] + k * b[0], a[1] + k * b[1], a[2] + k * b[2]]
}

/// `a`·`b`.
#[must_use]
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `a` × `b`.
#[must_use]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The length of `a`.
#[must_use]
pub fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// `a` at unit length.
#[must_use]
pub fn unit(a: V3) -> V3 {
    along([0.0; 3], 1.0 / norm(a), a)
}

/// A surface as the case states it: angles in degrees, lengths in the case's units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Surface {
    /// origin + u·xaxis + v·yaxis over `u` × `v`.
    Plane {
        /// The origin.
        origin: V3,
        /// The first direction.
        xaxis: V3,
        /// The second direction.
        yaxis: V3,
        /// The range along `xaxis`.
        u: [f64; 2],
        /// The range along `yaxis`.
        v: [f64; 2],
    },
    /// A sphere's patch.
    Sphere {
        /// The centre.
        center: V3,
        /// Longitude zero.
        xaxis: V3,
        /// Longitude 90.
        yaxis: V3,
        /// The radius.
        radius: f64,
        /// The longitudes.
        lon: [f64; 2],
        /// The latitudes.
        lat: [f64; 2],
    },
    /// A cylinder's patch.
    Cylinder {
        /// A point on the axis.
        origin: V3,
        /// Angle zero.
        xaxis: V3,
        /// Angle 90; the axis is `xaxis` × `yaxis`.
        yaxis: V3,
        /// The radius.
        radius: f64,
        /// The angles.
        angle: [f64; 2],
        /// The heights along the axis.
        height: [f64; 2],
    },
    /// A cone's patch.
    Cone {
        /// The apex.
        apex: V3,
        /// Angle zero.
        xaxis: V3,
        /// Angle 90; the axis is `xaxis` × `yaxis`.
        yaxis: V3,
        /// The half angle.
        half_angle_deg: f64,
        /// The heights along the axis.
        height: [f64; 2],
    },
    /// A torus's patch.
    Torus {
        /// The centre.
        center: V3,
        /// Major angle zero.
        xaxis: V3,
        /// Major angle 90; the axis is `xaxis` × `yaxis`.
        yaxis: V3,
        /// The major radius.
        #[serde(rename = "R")]
        big: f64,
        /// The minor radius.
        #[serde(rename = "r")]
        small: f64,
        /// The major angles.
        major: [f64; 2],
        /// The minor angles.
        minor: [f64; 2],
    },
    /// A NURBS surface: control points row by row, `points[i·nv + j]`.
    Nurbs {
        /// What it is a patch of.
        label: String,
        /// The degrees in u and v.
        degree: [usize; 2],
        /// The knots in u.
        knots_u: Vec<f64>,
        /// The knots in v.
        knots_v: Vec<f64>,
        /// Control points in u.
        nu: usize,
        /// Control points in v.
        nv: usize,
        /// The net.
        points: Vec<V3>,
        /// The weights, one per point.
        weights: Vec<f64>,
    },
}

/// A surface's closed form, as the scorer reads it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Implicit {
    /// A plane.
    Plane {
        /// A point on it.
        point: V3,
        /// Its normal.
        normal: V3,
    },
    /// A sphere.
    Sphere {
        /// The centre.
        center: V3,
        /// The radius.
        radius: f64,
    },
    /// A cylinder.
    Cylinder {
        /// A point on the axis.
        point: V3,
        /// The axis.
        axis: V3,
        /// The radius.
        radius: f64,
    },
    /// The upper nappe of a cone.
    Cone {
        /// The apex.
        point: V3,
        /// The axis.
        axis: V3,
        /// The half angle.
        half_angle_deg: f64,
    },
    /// A torus.
    Torus {
        /// The centre.
        point: V3,
        /// The axis.
        axis: V3,
        /// The major radius.
        #[serde(rename = "R")]
        big: f64,
        /// The minor radius.
        #[serde(rename = "r")]
        small: f64,
    },
    /// A kind only the scorer reads: height fields, parametric forms, quadrics.
    #[serde(other)]
    Other,
}

impl Implicit {
    /// How far `p` is from the surface, or `None` for [`Implicit::Other`].
    #[must_use]
    pub fn distance(&self, p: V3) -> Option<f64> {
        // Height along the axis and distance from it.
        let polar = |point: V3, axis: V3| {
            let a = unit(axis);
            let q = sub(p, point);
            let h = dot(q, a);
            (h, norm(along(q, -h, a)))
        };
        Some(match *self {
            Implicit::Plane { point, normal } => dot(sub(p, point), unit(normal)).abs(),
            Implicit::Sphere { center, radius } => (norm(sub(p, center)) - radius).abs(),
            Implicit::Cylinder {
                point,
                axis,
                radius,
            } => (polar(point, axis).1 - radius).abs(),
            Implicit::Cone {
                point,
                axis,
                half_angle_deg,
            } => {
                let (h, rho) = polar(point, axis);
                (rho * cos_deg(half_angle_deg) - h * sin_deg(half_angle_deg)).abs()
            }
            Implicit::Torus {
                point,
                axis,
                big,
                small,
            } => {
                let (h, rho) = polar(point, axis);
                ((rho - big).hypot(h) - small).abs()
            }
            Implicit::Other => return None,
        })
    }
}

/// How a piece of the truth meets the two surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contact {
    /// A transversal crossing.
    Crossing,
    /// Tangent along a curve.
    Tangent,
    /// On the border of a shared region.
    CoincidentBorder,
    /// Crossing at a very small angle.
    NearTangent,
    /// On a patch's boundary.
    Boundary,
    /// A point on its own.
    Isolated,
    /// A branch point or self-crossing.
    Singular,
    /// The boundary of a shared region.
    Coincident,
}

/// A curve of the truth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    /// Whether it closes.
    pub closed: bool,
    /// How the surfaces meet along it.
    pub kind: Contact,
    /// Dense samples.
    pub points: Vec<V3>,
}

/// A region the two surfaces share.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shared {
    /// Dense samples of its boundary.
    pub boundary: Vec<V3>,
}

/// The exact answer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Expected {
    /// The curves.
    pub branches: Vec<Branch>,
    /// The points on their own.
    pub isolated_points: Vec<V3>,
    /// The points where the curves must split.
    pub singular_points: Vec<V3>,
    /// The shared regions.
    pub coincident_regions: Vec<Shared>,
}

/// The classes of the corpus, with Yang, Jia and Yan's two as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// Class 1.
    Transversal,
    /// Class 2.
    TangentCurve,
    /// Class 3.
    IsolatedPoint,
    /// Class 4.
    Singular,
    /// Class 5.
    Boundary,
    /// Class 6.
    Coincident,
    /// Class 7.
    NearDegenerate,
    /// Class 8.
    Periodic,
    /// Class 9.
    HighDegree,
    /// The classes of Yang, Jia and Yan.
    Yang,
}

impl Class {
    /// Every class.
    pub const ALL: [Class; 10] = [
        Class::Transversal,
        Class::TangentCurve,
        Class::IsolatedPoint,
        Class::Singular,
        Class::Boundary,
        Class::Coincident,
        Class::NearDegenerate,
        Class::Periodic,
        Class::HighDegree,
        Class::Yang,
    ];

    /// The class a case names, by the first word of its `class` field.
    #[must_use]
    pub fn of(name: &str) -> Option<Class> {
        Some(match name.split(' ').next()? {
            "1" => Class::Transversal,
            "2" => Class::TangentCurve,
            "3" => Class::IsolatedPoint,
            "4" => Class::Singular,
            "5" => Class::Boundary,
            "6" => Class::Coincident,
            "7" => Class::NearDegenerate,
            "8" => Class::Periodic,
            "9" => Class::HighDegree,
            "Y23" => Class::Yang,
            _ => return None,
        })
    }

    /// Whether 12.17 lets this class return a flagged region. The transversal, seam-and-pole,
    /// coincident, patch-boundary and high-degree classes may not.
    #[must_use]
    pub const fn may_flag(self) -> bool {
        !matches!(
            self,
            Class::Transversal
                | Class::Periodic
                | Class::Coincident
                | Class::Boundary
                | Class::HighDegree
        )
    }
}

/// One piece of a case's truth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feature<'a> {
    /// How the surfaces meet there.
    pub contact: Contact,
    /// Its samples; one for a point.
    pub points: &'a [V3],
}

/// One corpus case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Case {
    /// Its name, which is its file's.
    pub id: String,
    /// Its class, as the corpus writes it.
    pub class: String,
    /// One line on what it is.
    pub title: String,
    /// The two surfaces.
    pub surfaces: [Surface; 2],
    /// Their closed forms.
    pub truth: [Implicit; 2],
    /// What each surface is called.
    pub surface_labels: [String; 2],
    /// The exact answer.
    pub expected: Expected,
    /// How close an answer must come.
    pub cov_tol: f64,
    /// Notes.
    pub notes: String,
    /// The closed form of the answer, if it has one.
    pub formula: String,
    /// The size of the smallest feature, where it is the point of the case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_size: Option<f64>,
    /// How far a residual moves the answer, where the case is ill-conditioned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ill_cond: Option<f64>,
    /// Where it comes from: the attribution of a case derived from a paper.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Case {
    /// Reads a case from JSON text.
    ///
    /// # Errors
    ///
    /// [`SsiError::Malformed`] if it does not fit the schema.
    pub fn parse(text: &str) -> Result<Case, SsiError> {
        serde_json::from_str(text).map_err(|e| SsiError::Malformed {
            what: "a case".into(),
            why: e.to_string(),
        })
    }

    /// Reads the case at `path`.
    ///
    /// # Errors
    ///
    /// [`SsiError::File`] if it cannot be read; [`SsiError::Malformed`] as [`Case::parse`].
    pub fn read(path: &Path) -> Result<Case, SsiError> {
        let text = fs::read_to_string(path).map_err(|e| SsiError::File {
            path: path.display().to_string(),
            why: e.to_string(),
        })?;
        Case::parse(&text).map_err(|e| match e {
            SsiError::Malformed { why, .. } => SsiError::Malformed {
                what: path.display().to_string(),
                why,
            },
            other => other,
        })
    }

    /// Its class.
    ///
    /// # Errors
    ///
    /// [`SsiError::Malformed`] for a class the corpus does not have.
    pub fn class(&self) -> Result<Class, SsiError> {
        Class::of(&self.class).ok_or_else(|| SsiError::Malformed {
            what: self.id.clone(),
            why: format!("no class {}", self.class),
        })
    }

    /// Everything the truth is made of: branches, region boundaries, isolated points and singular
    /// points, in that order.
    #[must_use]
    pub fn features(&self) -> Vec<Feature<'_>> {
        let e = &self.expected;
        let branches = e.branches.iter().map(|b| Feature {
            contact: b.kind,
            points: &b.points,
        });
        let regions = e.coincident_regions.iter().map(|r| Feature {
            contact: Contact::Coincident,
            points: &r.boundary,
        });
        let isolated = e.isolated_points.iter().map(|p| Feature {
            contact: Contact::Isolated,
            points: core::slice::from_ref(p),
        });
        let singular = e.singular_points.iter().map(|p| Feature {
            contact: Contact::Singular,
            points: core::slice::from_ref(p),
        });
        branches.chain(regions).chain(isolated).chain(singular).collect()
    }
}
