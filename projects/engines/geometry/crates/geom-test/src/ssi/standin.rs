//! Stand-ins for a surface/surface method: the truth, and the truth perturbed (chapter 12.17; piece
//! G160).
//!
//! What it does: the kernel has no surface/surface method yet, and a gate that has only ever seen
//! one kind of answer proves nothing. So each [`StandIn`] answers a case from its own truth, snapped
//! to the grid: [`StandIn::Truth`] as it is, and each of the others with one thing wrong. 12.17 asks
//! for a test that the gate fails when an answer is perturbed or a flagged region is enlarged;
//! these are those perturbations, and [`StandIn::expected`] names the rule each must break.
//!
//! [`StandIn::Flagged`] is a right answer of the other sort: where a class may flag, it leaves out
//! isolated points and a stretch of a tangent curve, and returns regions that hold them. A long
//! stretch comes back as a chain of boxes, each no wider than the case's band.
//!
//! Guarantees:
//!
//! - **Never mistaken for the kernel.** A report names its engine, and the runner writes the
//!   stand-in's name there.
//! - **The same answer every run**: a stand-in reads nothing but its case.
//!
//! Method: chapter 12.17's mutation test
//! (`projects/engines/geometry/architecture/12-intersections.md`).
//!
//! Cost: linear in the case's samples.

use super::case::{Case, Contact, V3, along, norm, sub, unit};
use super::cover::effective;
use super::gate::{band, floor_reach};
use super::output::{Curve, Flag, Output, Why};
use super::place::Placed;
use super::{SsiError, larger, smaller};

/// Boxes a flagged stretch may take; a stretch that would need more is left answered.
const MOST_BOXES: f64 = 4_000.0;

/// How many tolerances of a tangent curve [`StandIn::Flagged`] leaves out: enough that the scorer
/// sees a gap, which it does not at under two.
const GAP: f64 = 2.5;

/// What answers a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandIn {
    /// The truth.
    Truth,
    /// The truth, moved away from where it is.
    Shifted,
    /// The truth, with a curve that is not there.
    Spurious,
    /// The truth, with its first piece left out and nothing said.
    Dropped,
    /// The truth, with some of it flagged instead of answered.
    Flagged,
    /// [`StandIn::Flagged`], with its regions enlarged.
    Enlarged,
    /// [`StandIn::Flagged`], with its regions moved off the truth.
    Misplaced,
    /// [`StandIn::Flagged`], with reasons that do not fit what is there.
    WrongReason,
}

impl StandIn {
    /// Every stand-in.
    pub const ALL: [StandIn; 8] = [
        StandIn::Truth,
        StandIn::Shifted,
        StandIn::Spurious,
        StandIn::Dropped,
        StandIn::Flagged,
        StandIn::Enlarged,
        StandIn::Misplaced,
        StandIn::WrongReason,
    ];

    /// Its name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            StandIn::Truth => "truth",
            StandIn::Shifted => "shifted",
            StandIn::Spurious => "spurious",
            StandIn::Dropped => "dropped",
            StandIn::Flagged => "flagged",
            StandIn::Enlarged => "enlarged",
            StandIn::Misplaced => "misplaced",
            StandIn::WrongReason => "wrong-reason",
        }
    }

    /// The stand-in called `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<StandIn> {
        StandIn::ALL.into_iter().find(|s| s.name() == name)
    }

    /// The gate rule this stand-in must break, none if it is a right answer.
    #[must_use]
    pub const fn expected(self) -> Option<&'static str> {
        match self {
            StandIn::Truth | StandIn::Flagged => None,
            StandIn::Shifted | StandIn::Spurious | StandIn::Dropped => Some("verdict"),
            StandIn::Enlarged => Some("too-wide"),
            StandIn::Misplaced => Some("no-feature"),
            StandIn::WrongReason => Some("wrong-kind"),
        }
    }

    /// Its answer to `case`, at a run's `accuracy`.
    ///
    /// # Errors
    ///
    /// [`SsiError::Malformed`] for a case with a class the corpus does not have.
    pub fn answer(
        self,
        case: &Case,
        placed: &Placed,
        accuracy: Option<f64>,
    ) -> Result<Output, SsiError> {
        let cov = effective(case, accuracy);
        let mut out = truth(case, placed);
        match self {
            StandIn::Truth => {}
            StandIn::Shifted => {
                let off = along([0.0; 3], 4.0 * cov, unit([1.0, 1.0, 1.0]));
                for p in out.curves.iter_mut().flat_map(|c| &mut c.xyz) {
                    *p = along(*p, 1.0, off);
                }
                for p in &mut out.points {
                    *p = along(*p, 1.0, off);
                }
            }
            StandIn::Spurious => {
                let far = case
                    .features()
                    .iter()
                    .flat_map(|f| f.points.iter())
                    .fold(0.0, |m, p| larger(m, norm(*p)));
                let start = [far + 8.0 * cov; 3];
                out.curves.push(Curve {
                    xyz: vec![start, along(start, 8.0 * cov, [1.0, 0.0, 0.0])],
                    kind: "CROSSING".into(),
                });
            }
            StandIn::Dropped => {
                if out.curves.is_empty() {
                    out.points.clear();
                } else {
                    out.curves.remove(0);
                }
            }
            StandIn::Flagged | StandIn::Enlarged | StandIn::Misplaced | StandIn::WrongReason => {
                if case.class()?.may_flag() {
                    flagged(case, placed, cov, &mut out);
                }
                tweak(self, placed, &mut out);
            }
        }
        Ok(out)
    }
}

/// The curves and points of `case`'s truth, snapped to the grid, in the case's units.
fn truth(case: &Case, placed: &Placed) -> Output {
    let mut out = Output::empty(placed);
    for f in case.features() {
        let name = match f.contact {
            Contact::Crossing | Contact::Boundary => "CROSSING",
            Contact::Tangent | Contact::NearTangent => "TANGENT",
            Contact::CoincidentBorder | Contact::Coincident => "COINCIDENT",
            Contact::Isolated => {
                out.points.extend(f.points.iter().map(|p| placed.snapped(*p)));
                continue;
            }
            Contact::Singular => continue,
        };
        out.curves.push(Curve {
            xyz: f.points.iter().map(|p| placed.snapped(*p)).collect(),
            kind: name.into(),
        });
    }
    out
}

/// A box of half-width `half` steps about `p`, which is in the case's units.
#[allow(
    clippy::cast_possible_truncation,
    reason = "a coordinate in steps is inside the grid's range, and floor and ceil make it whole"
)]
fn around(p: V3, half: f64, why: Why, placed: &Placed) -> Flag {
    let at = placed.steps(p);
    Flag {
        why,
        lo: at.map(|x| (x - half).floor() as i64),
        hi: at.map(|x| (x + half).ceil() as i64),
    }
}

/// The half-width of a box whose corner stays within `reach` of what it holds.
fn half_for(reach: f64) -> f64 {
    larger(1.0, (reach / 3.0_f64.sqrt() * 0.9).floor())
}

/// Leaves out what a method may flag, and flags it: isolated points, singular points, and a stretch
/// of the first tangent curve.
fn flagged(case: &Case, placed: &Placed, cov: f64, out: &mut Output) {
    let floor = half_for(floor_reach(placed));
    out.points.clear();
    for p in &case.expected.isolated_points {
        out.flagged.push(around(*p, floor, Why::TangentBand, placed));
    }
    for p in &case.expected.singular_points {
        out.flagged.push(around(*p, floor, Why::NonSmooth, placed));
    }
    let tangent = case
        .expected
        .branches
        .iter()
        .position(|b| matches!(b.kind, Contact::Tangent | Contact::NearTangent));
    if let Some(index) = tangent {
        stretch(index, placed, cov, out);
    }
}

/// Cuts a stretch out of `out`'s curve `index`, and covers it with a chain of boxes.
fn stretch(index: usize, placed: &Placed, cov: f64, out: &mut Output) {
    let half = half_for(band(placed));
    let xyz = out.curves[index].xyz.clone();
    let mut at = Vec::with_capacity(xyz.len());
    let mut walked = 0.0;
    for (i, p) in xyz.iter().enumerate() {
        if i > 0 {
            walked += norm(sub(*p, xyz[i - 1]));
        }
        at.push(walked);
    }
    let length = smaller(GAP * 2.0 * cov, walked);
    let spacing = half / placed.scale();
    if length / spacing > MOST_BOXES || walked == 0.0 {
        return;
    }
    let (from, to) = ((walked - length) / 2.0, (walked + length) / 2.0);
    let on = |s: f64| {
        let i = at.partition_point(|a| *a < s).clamp(1, xyz.len() - 1);
        let (a, b) = (at[i - 1], at[i]);
        let t = if b > a { (s - a) / (b - a) } else { 0.0 };
        along(xyz[i - 1], t, sub(xyz[i], xyz[i - 1]))
    };
    let mut s = from;
    while s < to {
        out.flagged.push(around(on(s), half, Why::TangentBand, placed));
        s += spacing;
    }
    out.flagged.push(around(on(to), half, Why::TangentBand, placed));
    let keep = |wanted: &dyn Fn(f64) -> bool| -> Vec<V3> {
        xyz.iter()
            .zip(&at)
            .filter(|(_, a)| wanted(**a))
            .map(|(p, _)| *p)
            .collect()
    };
    let before = keep(&|a| a < from);
    let after = keep(&|a| a > to);
    let kind = out.curves[index].kind.clone();
    out.curves.remove(index);
    for piece in [before, after] {
        if piece.len() > 1 {
            out.curves.push(Curve {
                xyz: piece,
                kind: kind.clone(),
            });
        }
    }
}

/// Puts one thing wrong with a flagged answer.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the band is far under the grid's range, and ceil makes it whole"
)]
fn tweak(stand_in: StandIn, placed: &Placed, out: &mut Output) {
    let wide = (band(placed) * 3.0).ceil() as i64;
    for flag in &mut out.flagged {
        match stand_in {
            StandIn::Enlarged => {
                flag.lo = flag.lo.map(|x| x - wide);
                flag.hi = flag.hi.map(|x| x + wide);
            }
            StandIn::Misplaced => {
                flag.lo = flag.lo.map(|x| x + 4 * wide);
                flag.hi = flag.hi.map(|x| x + 4 * wide);
            }
            StandIn::WrongReason => {
                flag.why = match flag.why {
                    Why::NonSmooth => Why::NearCoincident,
                    _ => Why::NonSmooth,
                };
            }
            _ => {}
        }
    }
}
