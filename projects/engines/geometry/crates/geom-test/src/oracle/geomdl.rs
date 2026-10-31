//! The first oracle: geomdl (NURBS-Python, MIT), out of process, on NURBS (chapter 28.10; piece
//! G113).
//!
//! What it does: [`jobs`] draws a night's seeded curves and what to do with each: evaluate a point,
//! the basis functions at a parameter, or a knot insertion. Each job is one JSON line, with every
//! double written by [`super::exact::float`], so geomdl reads the bits the kernel read. The harness
//! in the oracle repository answers one JSON line per job. [`judge`] compares an answer with
//! `geom-nurbs`, and settles any difference by the exact rational reference
//! ([`crate::nurbs_reference`]): a difference the kernel is within its own bound of the truth on is
//! geomdl's rounding; anything else is ours.
//!
//! Guarantees:
//!
//! - **The job is what the kernel ran**: inputs round-trip bit for bit, and the kernel's answer
//!   comes from the same parsed job.
//! - **Refusals are compared**: a job the kernel refuses and geomdl answers is a difference, of kind
//!   `refused`, never a pass.
//! - **Truth settles it**: an oracle is never trusted; [`Compared::Differ`]'s `settled` says what the
//!   exact reference found.
//!
//! Method: The NURBS Book's A2.2, A3.1, A4.1 and A5.1, which geomdl's comments and `geom-nurbs` both
//! cite; chapter 28.10's comparison
//! (`projects/engines/geometry/architecture/28-testing.md`). The bound is the one
//! `geom-nurbs` states and `tests/nurbs_reference.rs` holds it to; geomdl states none, and it runs
//! the same algorithms in doubles, so it is held to the same bound.
//!
//! Cost: linear in the jobs; the exact reference runs only on a difference.

use core::num::NonZeroU64;

use dashu_ratio::RBig;
use geom_nurbs::{Basis, Curve, Limit};
use geom_run::{Budget, WorkMeter};
use serde::{Deserialize, Serialize};

use super::compare::{Allowance, within};
use super::exact::{float, read_float};
use super::triage::Signature;
use crate::nurbs_reference::Spline;
use crate::rng::SplitMix64;
use crate::seed::Seed;

/// The oracle's id in the catalogue.
pub const ID: &str = "geomdl";

/// What to do with a curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    /// The point at `at`.
    Point,
    /// The nonzero basis functions at `at`.
    Basis,
    /// The curve with `at` inserted once: its knots and homogeneous points.
    Insert,
}

impl Op {
    /// The operation's name in a signature.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Op::Point => "nurbs.point",
            Op::Basis => "nurbs.basis",
            Op::Insert => "nurbs.insert",
        }
    }
}

/// One job, every double as [`float`] wrote it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    /// Its number.
    pub id: u64,
    /// Its input family: `degree-<p>-<d>d` and `-rational`.
    pub family: String,
    /// What to do.
    pub op: Op,
    /// The degree.
    pub degree: usize,
    /// The knots.
    pub knots: Vec<String>,
    /// The control points, not weighted.
    pub points: Vec<Vec<String>>,
    /// The weights, for a rational curve.
    pub weights: Option<Vec<String>>,
    /// The parameter.
    pub at: String,
}

/// One answer: `values` flat, in a job's stated order, each a decimal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    /// The job's number.
    pub id: u64,
    /// The values; `None` if the oracle refused the job.
    pub values: Option<Vec<String>>,
}

fn write(x: f64) -> String {
    float(x).unwrap_or_default()
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    NonZeroU64::new(n).map_or(0, |n| rng.below(n))
}

/// A fraction with a short binary expansion, so every input is exactly what is written.
fn fraction(rng: &mut SplitMix64, top: u64, bits: u32) -> f64 {
    let value = below(rng, top.saturating_mul(1 << bits));
    // In range: `value` is below 2^53 for every `top` and `bits` this file uses.
    #[allow(clippy::cast_precision_loss, reason = "below 2^53, so exact")]
    let x = value as f64;
    x / f64::from(1_u32 << bits)
}

/// A clamped knot vector of `count` control points of degree `degree` on [0, 1].
fn knots(rng: &mut SplitMix64, degree: usize, count: usize) -> Vec<f64> {
    let inside = count.saturating_sub(degree).saturating_sub(1);
    let mut interior: Vec<f64> = (0..inside)
        .map(|_| {
            let sixteenth = below(rng, 15).saturating_add(1);
            #[allow(clippy::cast_precision_loss, reason = "at most 15")]
            let x = sixteenth as f64;
            x / 16.0
        })
        .collect();
    interior.sort_by(f64::total_cmp);
    let order = degree.saturating_add(1);
    let mut all = vec![0.0; order];
    all.extend(interior);
    all.extend(vec![1.0; order]);
    all
}

/// Whether no interior knot of `knots` repeats more than `degree` times.
fn fits(knots: &[f64], degree: usize) -> bool {
    knots
        .iter()
        .filter(|k| **k > 0.0 && **k < 1.0)
        .all(|k| knots.iter().filter(|x| *x == k).count() <= degree)
}

/// The night's `count` jobs from `seed`.
#[must_use]
pub fn jobs(seed: Seed, count: u64) -> Vec<Job> {
    let mut rng = SplitMix64::new(seed);
    let mut out = Vec::new();
    for id in 0..count {
        let degree = usize::try_from(below(&mut rng, 4))
            .unwrap_or(0)
            .saturating_add(1);
        let points_count = degree
            .saturating_add(1)
            .saturating_add(usize::try_from(below(&mut rng, 5)).unwrap_or(0));
        let dimension = usize::try_from(below(&mut rng, 2))
            .unwrap_or(0)
            .saturating_add(2);
        let rational = below(&mut rng, 2) == 0;
        let mut drawn = knots(&mut rng, degree, points_count);
        while !fits(&drawn, degree) {
            drawn = knots(&mut rng, degree, points_count);
        }
        let points: Vec<Vec<String>> = (0..points_count)
            .map(|_| {
                (0..dimension)
                    .map(|_| write(fraction(&mut rng, 16, 6) - 8.0))
                    .collect()
            })
            .collect();
        let weights = rational.then(|| {
            (0..points_count)
                .map(|_| write(0.25 + fraction(&mut rng, 4, 4)))
                .collect()
        });
        let at = 1.0 / 64.0 + fraction(&mut rng, 1, 6) * (62.0 / 64.0);
        let op = match below(&mut rng, 3) {
            0 => Op::Point,
            1 => Op::Basis,
            _ => Op::Insert,
        };
        let rational_name = if rational { "-rational" } else { "" };
        out.push(Job {
            id,
            family: format!("degree-{degree}-{dimension}d{rational_name}"),
            op,
            degree,
            knots: drawn.into_iter().map(write).collect(),
            points,
            weights,
            at: write(at),
        });
    }
    out
}

/// A job's doubles.
struct Parsed {
    knots: Vec<f64>,
    points: Vec<Vec<f64>>,
    weights: Option<Vec<f64>>,
    at: f64,
}

fn parse(job: &Job) -> Result<Parsed, String> {
    let list = |texts: &[String]| {
        texts
            .iter()
            .map(|t| read_float(t).ok_or_else(|| format!("job {}: {t:?} is not a number", job.id)))
            .collect::<Result<Vec<f64>, String>>()
    };
    Ok(Parsed {
        knots: list(&job.knots)?,
        points: job
            .points
            .iter()
            .map(|p| list(p))
            .collect::<Result<_, _>>()?,
        weights: job.weights.as_deref().map(list).transpose()?,
        at: read_float(&job.at).ok_or_else(|| format!("job {}: bad parameter", job.id))?,
    })
}

/// The larger of two doubles neither of which is NaN.
fn larger(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// The kernel's values for a job, flat, with the size each value is measured against; `None` if the
/// kernel refused it.
fn kernel<const D: usize>(job: &Job, input: &Parsed) -> Option<(Vec<f64>, f64)> {
    let basis = Basis::new(job.degree, input.knots.clone()).ok()?;
    if job.op == Op::Basis {
        let mut out = vec![0.0; job.degree.saturating_add(1)];
        basis.eval(input.at, Limit::FromRight, &mut out).ok()?;
        return Some((out, 1.0));
    }
    let weights = input.weights.as_deref().unwrap_or(&[]);
    let mut homogeneous = Vec::with_capacity(input.points.len());
    for (i, point) in input.points.iter().enumerate() {
        let w = weights.get(i).copied().unwrap_or(1.0);
        let mut h = [0.0; D];
        for (slot, x) in h.iter_mut().zip(point) {
            *slot = x * w;
        }
        homogeneous.push(h);
    }
    let curve = Curve::from_homogeneous(basis, homogeneous, input.weights.clone()).ok()?;
    let size = input
        .points
        .iter()
        .flatten()
        .fold(1.0_f64, |most, x| larger(most, x.abs()));
    if job.op == Op::Point {
        return Some((curve.point(input.at, Limit::FromRight).ok()?.to_vec(), size));
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let new = curve.insert_knot(input.at, 1, &mut meter).ok()?.value;
    let mut out = new.basis().knots().to_vec();
    out.extend(new.homogeneous().iter().flatten());
    Some((out, size))
}

/// `γ_n` as a double, rounded up: n·u / (1 − n·u), u = 2^−53.
fn gamma(n: usize) -> f64 {
    #[allow(clippy::cast_precision_loss, reason = "a small count")]
    let nu = n as f64 * f64::EPSILON / 2.0;
    (nu / (1.0 - nu)).next_up()
}

/// The bound of a value of a job: `geom-nurbs`'s statement, measured on the largest coordinate.
fn bound(job: &Job, size: f64) -> f64 {
    let p = job.degree;
    let steps = if job.weights.is_some() {
        p.saturating_mul(14).saturating_add(3)
    } else {
        p.saturating_mul(7).saturating_add(1)
    };
    let weights = job.weights.as_ref().map_or(1.0, |list| {
        let read = |t: &String| read_float(t).unwrap_or(1.0);
        let most = list.iter().map(read).fold(0.0_f64, larger);
        let least = list
            .iter()
            .map(read)
            .fold(f64::MAX, |a, b| if b < a { b } else { a });
        most / least
    });
    (gamma(steps) * size * weights).next_up()
}

/// What a job's comparison found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compared {
    /// Within the allowance.
    Agree,
    /// A difference.
    Differ {
        /// Its signature.
        signature: Signature,
        /// What the exact reference found: whether the kernel is within its bound of the truth.
        settled: String,
    },
}

/// The difference `job` raised, of `kind`, with `settled`.
fn difference(job: &Job, kind: &str, settled: String) -> Compared {
    Compared::Differ {
        signature: Signature {
            operation: job.op.name().to_owned(),
            oracle: ID.to_owned(),
            kind: kind.to_owned(),
            family: job.family.clone(),
        },
        settled,
    }
}

/// `x` exactly.
fn exact(x: f64) -> Result<RBig, String> {
    RBig::try_from(x).map_err(|_| format!("{x} is not finite"))
}

/// The exact curve of a job.
fn reference(job: &Job, input: &Parsed) -> Result<Spline, String> {
    let error = |e| format!("the reference: {e:?}");
    match &input.weights {
        None => {
            let rows: Vec<&[f64]> = input.points.iter().map(Vec::as_slice).collect();
            Spline::new(job.degree, &input.knots, &rows).map_err(error)
        }
        Some(weights) => {
            let weighted: Vec<Vec<f64>> = input
                .points
                .iter()
                .zip(weights)
                .map(|(p, w)| p.iter().map(|x| x * w).collect())
                .collect();
            let rows: Vec<&[f64]> = weighted.iter().map(Vec::as_slice).collect();
            Spline::new_rational(job.degree, &input.knots, &rows, weights).map_err(error)
        }
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals, which cannot overflow"
)]
/// Settles a point difference against the exact curve: which side is within the bound of the truth.
fn settle(
    job: &Job,
    input: &Parsed,
    ours: &[f64],
    theirs: &[f64],
    limit: f64,
) -> Result<String, String> {
    let spline = reference(job, input)?;
    let truth = spline
        .point(&exact(input.at)?)
        .map_err(|e| format!("the reference: {e:?}"))?;
    let limit = exact(limit)?;
    let far = |side: &[f64]| -> Result<bool, String> {
        for (x, t) in side.iter().zip(&truth) {
            let gap = exact(*x)? - t.clone();
            let gap = if gap < RBig::ZERO { -gap } else { gap };
            if gap > limit {
                return Ok(true);
            }
        }
        Ok(false)
    };
    Ok(match (far(ours)?, far(theirs)?) {
        (false, true) => "theirs: the kernel is within its bound of the truth, and geomdl is not",
        (true, false) => "ours: geomdl is within the bound of the truth, and the kernel is not",
        (true, true) => "ours: neither is within the bound of the truth",
        (false, false) => "both within the bound of the truth",
    }
    .to_owned())
}

/// Judges one answer.
///
/// # Errors
///
/// A job or answer that cannot be read: the harness is broken, not the kernel.
pub fn judge(job: &Job, answer: &Answer) -> Result<Compared, String> {
    if job.id != answer.id {
        return Err(format!("answer {} for job {}", answer.id, job.id));
    }
    let input = parse(job)?;
    let dimension = job.points.first().map_or(0, Vec::len);
    let ours = if dimension == 2 {
        kernel::<2>(job, &input)
    } else {
        kernel::<3>(job, &input)
    };
    let theirs = match &answer.values {
        None => None,
        Some(values) => Some(
            values
                .iter()
                .map(|t| read_float(t).ok_or_else(|| format!("job {}: bad answer {t:?}", job.id)))
                .collect::<Result<Vec<f64>, String>>()?,
        ),
    };
    let (ours, size, theirs) = match (ours, theirs) {
        (None, None) => return Ok(Compared::Agree),
        (None, Some(_)) | (Some(_), None) => {
            return Ok(difference(
                job,
                "refused",
                "one side refused the job".to_owned(),
            ));
        }
        (Some((ours, size)), Some(theirs)) => (ours, size, theirs),
    };
    if ours.len() != theirs.len() {
        return Ok(difference(
            job,
            "shape",
            format!("{} values and {}", ours.len(), theirs.len()),
        ));
    }
    let stated = bound(job, size);
    let allowance = Allowance {
        ours: stated,
        theirs: stated,
        rounding: 0.0,
        fit: stated,
    };
    if ours
        .iter()
        .zip(&theirs)
        .all(|(a, b)| within(*a, *b, &allowance))
    {
        return Ok(Compared::Agree);
    }
    let settled = if job.op == Op::Point {
        settle(job, &input, &ours, &theirs, stated)?
    } else {
        "beyond the allowance; the exact reference settles a point only".to_owned()
    };
    Ok(difference(job, "value", settled))
}
