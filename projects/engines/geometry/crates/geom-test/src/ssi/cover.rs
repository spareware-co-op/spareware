//! Our own reading of coverage: which of a case's truth an answer misses, and what an answer holds
//! that the truth does not (chapter 12.17; piece G160).
//!
//! What it does: [`compare`] measures distances the way the scorer's `score_case` does: each truth
//! feature, thinned to about a thousand samples, against the answer's polylines and points, and each
//! answer curve, thinned the same way, against the truth. A sample further than the effective
//! tolerance ([`effective`]) is missing or spurious. The result names the samples, because the
//! scorer's verdict does not say where a gap is, and the gate needs that to hold it to a flagged
//! region.
//!
//! Guarantees:
//!
//! - **The scorer's tolerance**: [`effective`] is its `cov_eff`, including the widening for a case
//!   that states its conditioning.
//! - **The scorer's samples**: the same thinning, so the two readings look at the same points.
//!
//! Method: `score_case` in the corpus's `scoring/score.py`,
//! <https://github.com/jyviko/ssi-corpus/blob/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9/scoring/score.py>,
//! without its spatial hash: this runs on hundreds of cases, not thousands, and a nearest-segment
//! scan cannot be wrong in the ways a hash can.
//!
//! Cost: samples × segments, for each direction.

use super::case::{Case, Contact, V3, along, dot, norm, sub};
use super::output::Output;
use super::{larger, smaller};

/// A case's tolerance as the scorer applies it: widened where a case states its conditioning, by
/// twice the run's accuracy times that factor, up to a half.
#[must_use]
pub fn effective(case: &Case, accuracy: Option<f64>) -> f64 {
    match (case.ill_cond, accuracy) {
        (Some(ill), Some(accuracy)) if accuracy > 0.0 => {
            larger(case.cov_tol, smaller(0.5, 2.0 * accuracy * ill))
        }
        _ => case.cov_tol,
    }
}

/// Every `len / 1000`th sample of `points`, as the scorer thins them.
fn thinned(points: &[V3]) -> impl Iterator<Item = &V3> {
    points.iter().step_by((points.len() / 1000).max(1))
}

/// How far `q` is from the segment `a`–`b`.
fn to_segment(q: V3, a: V3, b: V3) -> f64 {
    let d = sub(b, a);
    let long = dot(d, d);
    let t = if long > 0.0 {
        larger(0.0, smaller(1.0, dot(sub(q, a), d) / long))
    } else {
        0.0
    };
    norm(sub(q, along(a, t, d)))
}

/// A set of polylines and points to measure against.
struct Nearest {
    segments: Vec<(V3, V3)>,
}

impl Nearest {
    /// The set of `curves` and `points`.
    fn of<'a>(curves: impl Iterator<Item = &'a [V3]>, points: &[V3]) -> Nearest {
        let mut segments = Vec::new();
        for curve in curves {
            if let [only] = curve {
                segments.push((*only, *only));
            }
            segments.extend(curve.windows(2).map(|w| (w[0], w[1])));
        }
        segments.extend(points.iter().map(|p| (*p, *p)));
        Nearest { segments }
    }

    /// How far `q` is from the set; infinite if it is empty.
    fn distance(&self, q: V3) -> f64 {
        self.segments
            .iter()
            .fold(f64::INFINITY, |d, (a, b)| smaller(d, to_segment(q, *a, *b)))
    }
}

/// A piece of the truth the answer does not reach.
#[derive(Clone, Debug, PartialEq)]
pub struct Missing {
    /// The feature's place in [`Case::features`].
    pub feature: usize,
    /// How the surfaces meet there.
    pub contact: Contact,
    /// The samples further than the tolerance from the answer.
    pub samples: Vec<V3>,
}

/// What [`compare`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    /// What the answer misses.
    pub missing: Vec<Missing>,
    /// How many of the answer's curves and points leave the truth.
    pub spurious: usize,
}

/// `out` against `case`'s truth, at tolerance `cov`.
#[must_use]
pub fn compare(case: &Case, out: &Output, cov: f64) -> Seen {
    let answer = Nearest::of(out.curves.iter().map(|c| c.xyz.as_slice()), &out.points);
    let features = case.features();
    let judged = |contact: Contact| contact != Contact::Singular;
    let mut seen = Seen::default();
    for (feature, f) in features.iter().enumerate() {
        if !judged(f.contact) {
            continue;
        }
        let samples: Vec<V3> = thinned(f.points)
            .filter(|q| answer.distance(**q) > cov)
            .copied()
            .collect();
        if !samples.is_empty() {
            seen.missing.push(Missing {
                feature,
                contact: f.contact,
                samples,
            });
        }
    }
    let truth = Nearest::of(
        features
            .iter()
            .filter(|f| judged(f.contact) && f.contact != Contact::Isolated)
            .map(|f| f.points),
        &case.expected.isolated_points,
    );
    seen.spurious = out
        .curves
        .iter()
        .filter(|c| thinned(&c.xyz).any(|q| truth.distance(*q) > cov))
        .count()
        + out.points.iter().filter(|p| truth.distance(**p) > cov).count();
    seen
}
