//! What a runner writes: the scorer's output contract, and our flagged regions beside it (chapter
//! 12.17; piece G160).
//!
//! What it does: the corpus calls a runner as `<binary> <case.json> <mode> <tol> <out.json>` and
//! reads [`Output`]: a status, a time, curves as samples and points on their own. The scorer has no
//! notion of a flagged region, so [`Output::flagged`] is a field it ignores, and [`super::gate`]
//! reads it. [`Output::of_intersection`] is the adapter from the kernel's result type: curves are
//! sampled, an overlap's boundary curves are written as curves, and points and flagged regions go
//! across as they are, back in the case's units.
//!
//! Guarantees:
//!
//! - **A flagged region never reaches the scorer as an answer.** The scorer sees only what was
//!   proven. What it misses, the gate holds to the flagged regions.
//! - **The digest is the answer**: [`Output::digest`] hashes every coordinate's bits and every
//!   flagged region, and leaves out the time, so two runs of one answer agree bit for bit.
//!
//! Method: the runner contract of the ssi-corpus's README,
//! <https://github.com/jyviko/ssi-corpus/blob/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9/README.md>;
//! the item list of chapter 12.2
//! (`projects/engines/geometry/architecture/12-intersections.md`).
//!
//! Cost: linear in the items and samples.

use geom_curve::{CurveKind, ParamCurve};
use geom_intersect::{CurveContact, Intersection, Item, Unresolved};
use geom_math::Box3;
use geom_nurbs::Limit;
use geom_num::P3;
use serde::{Deserialize, Serialize};

use super::SsiError;
use super::case::V3;
use super::place::Placed;
use crate::golden::{Sha256, hex};

/// The status a success reports: SMLib's `SM_SUCCESS`, which the scorer tests for.
pub const SUCCESS: u32 = 1000;

/// The status of an engine that does not answer.
pub const NOT_BUILT: u32 = 1;

/// Samples taken along one curve of an answer.
pub const SAMPLES: u32 = 1024;

/// Why a region is flagged: [`Unresolved`] without its numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// A tangent contact too wide to resolve.
    TangentBand,
    /// A contact of higher order than the tests reach.
    HigherOrderContact,
    /// Close together, neither proven one nor apart.
    NearCoincident,
    /// Passing close, neither proven touching nor apart.
    NearMiss,
    /// The item's bound passes the ceiling.
    BoundExceeded,
    /// A known curve could not be proven the only one here.
    KnownCurveUnproven,
    /// A test could not decide at the floor.
    UndecidedAtFloor,
    /// The shape is not smooth here.
    NonSmooth,
}

impl Why {
    /// The reason `why` names.
    #[must_use]
    pub const fn of(why: Unresolved) -> Why {
        match why {
            Unresolved::TangentBand { .. } => Why::TangentBand,
            Unresolved::HigherOrderContact => Why::HigherOrderContact,
            Unresolved::NearCoincident { .. } => Why::NearCoincident,
            Unresolved::NearMiss { .. } => Why::NearMiss,
            Unresolved::BoundExceeded { .. } => Why::BoundExceeded,
            Unresolved::KnownCurveUnproven => Why::KnownCurveUnproven,
            Unresolved::UndecidedAtFloor { .. } => Why::UndecidedAtFloor,
            Unresolved::NonSmooth { .. } => Why::NonSmooth,
        }
    }
}

/// A flagged region: a box in grid steps, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flag {
    /// Why it is undecided.
    pub why: Why,
    /// The box's least corner.
    pub lo: [i64; 3],
    /// The box's greatest corner.
    pub hi: [i64; 3],
}

impl Flag {
    /// The box's corners, as doubles.
    ///
    /// Coordinates are inside the grid's range, so each is a double exactly.
    #[must_use]
    pub fn corners(&self) -> ([f64; 3], [f64; 3]) {
        (self.lo.map(double), self.hi.map(double))
    }

    /// The box's diagonal, in steps: the size [`super::gate`] totals.
    #[must_use]
    pub fn diagonal(&self) -> f64 {
        let (lo, hi) = self.corners();
        super::case::norm(super::case::sub(hi, lo))
    }
}

/// One curve of an answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    /// Its samples, in the case's units.
    pub xyz: Vec<V3>,
    /// How the surfaces meet along it; `TOUCHING` makes the scorer read it as a point.
    #[serde(rename = "type")]
    pub kind: String,
}

/// What a runner writes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
    /// [`SUCCESS`], or why not.
    pub status: u32,
    /// The status, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_name: Option<String>,
    /// How long it took.
    pub time_ms: f64,
    /// The curves.
    pub curves: Vec<Curve>,
    /// The points on their own, in the case's units.
    pub points: Vec<V3>,
    /// The scale the answer was made at: 2^`log2` steps to the case's unit.
    #[serde(default)]
    pub log2: u32,
    /// The flagged regions.
    #[serde(default)]
    pub flagged: Vec<Flag>,
}

/// The middle of `xyz`, in steps.
fn middle(xyz: &Box3) -> V3 {
    let (a, b) = (corner(xyz.min()), corner(xyz.max()));
    [0, 1, 2].map(|i| (double(a[i]) + double(b[i])) / 2.0)
}

/// `p`'s coordinates.
fn corner(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// A coordinate as a double, which holds it exactly: the grid's range is under 2^53.
#[allow(
    clippy::cast_precision_loss,
    reason = "the grid's range is under 2^53, so each coordinate is a double exactly"
)]
fn double(x: i64) -> f64 {
    x as f64
}

/// `curve` as samples, in the case's units.
fn sampled(curve: &CurveKind<3>, placed: &Placed, id: &str) -> Result<Vec<V3>, SsiError> {
    let (lo, hi) = ParamCurve::domain(curve);
    let mut samples = Vec::with_capacity(SAMPLES as usize);
    for i in 0..SAMPLES {
        let t = lo + (hi - lo) * (f64::from(i) / f64::from(SAMPLES - 1));
        let side = if i + 1 == SAMPLES {
            Limit::FromLeft
        } else {
            Limit::FromRight
        };
        let p = ParamCurve::point(curve, t, side).map_err(|e| SsiError::Kernel {
            id: id.to_owned(),
            why: format!("{e:?}"),
        })?;
        samples.push(placed.units(p));
    }
    Ok(samples)
}

impl Output {
    /// An answer with nothing in it yet, at `placed`'s scale.
    #[must_use]
    pub fn empty(placed: &Placed) -> Output {
        Output {
            status: SUCCESS,
            status_name: None,
            time_ms: 0.0,
            curves: Vec::new(),
            points: Vec::new(),
            log2: placed.log2,
            flagged: Vec::new(),
        }
    }

    /// The kernel's `answer` to case `id`, as a runner writes it.
    ///
    /// # Errors
    ///
    /// [`SsiError::Kernel`] for a curve that cannot be evaluated.
    pub fn of_intersection(
        answer: &Intersection,
        placed: &Placed,
        id: &str,
    ) -> Result<Output, SsiError> {
        let mut out = Output::empty(placed);
        for item in answer.items() {
            match item {
                Item::Point { xyz, .. } => out.points.push(placed.units(middle(xyz))),
                Item::Curve { curve, contact, .. } => out.curves.push(Curve {
                    xyz: sampled(curve, placed, id)?,
                    kind: match contact {
                        CurveContact::Crossing => "CROSSING".into(),
                        CurveContact::Tangent { .. } => "TANGENT".into(),
                    },
                }),
                Item::Overlap { region, .. } => {
                    for curve in &region.boundary {
                        out.curves.push(Curve {
                            xyz: sampled(curve, placed, id)?,
                            kind: "COINCIDENT".into(),
                        });
                    }
                }
                Item::Flagged { xyz, why, .. } => {
                    out.flagged.push(Flag {
                        why: Why::of(*why),
                        lo: corner(xyz.min()),
                        hi: corner(xyz.max()),
                    });
                }
            }
        }
        Ok(out)
    }

    /// The hash of the answer: the status, every coordinate's bits and every flagged region, and
    /// not the time.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut h = Sha256::new();
        let mut word = |x: u64| h.write(&x.to_le_bytes());
        word(u64::from(self.status));
        word(u64::from(self.log2));
        for curve in &self.curves {
            word(curve.xyz.len() as u64);
            for x in curve.xyz.iter().flatten() {
                word(x.to_bits());
            }
        }
        word(self.points.len() as u64);
        for x in self.points.iter().flatten() {
            word(x.to_bits());
        }
        word(self.flagged.len() as u64);
        for flag in &self.flagged {
            word(flag.why as u64);
            for x in flag.lo.iter().chain(&flag.hi) {
                word(x.cast_unsigned());
            }
        }
        hex(h.finish())
    }
}
