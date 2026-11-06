//! What the oracle tests share: the stand-in for a correct reference (piece G113).
//!
//! A reference here is the kernel's own answer to a job, written the way an outside oracle writes
//! it, and a flawed one is the same answer moved: the two stand-ins the tests run the tool against.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
// Each test file that includes this module uses some of it.
#![allow(dead_code)]

use geom_nurbs::{Basis, Curve, Limit};
use geom_run::{Budget, WorkMeter};
use geom_test::oracle::exact::float;
use geom_test::oracle::geomdl::{Answer, Job, Op};

/// The kernel's own answer to a job, the way a correct oracle would give it.
pub fn honest(job: &Job) -> Answer {
    let read = |t: &String| t.parse::<f64>().unwrap();
    let knots: Vec<f64> = job.knots.iter().map(read).collect();
    let basis = Basis::new(job.degree, knots).unwrap();
    let values: Vec<f64> = match job.op {
        Op::Basis => {
            let mut out = vec![0.0; job.degree + 1];
            basis
                .eval(read(&job.at), Limit::FromRight, &mut out)
                .unwrap();
            out
        }
        _ => curve_values(job, basis),
    };
    Answer {
        id: job.id,
        values: Some(values.iter().map(|x| float(*x).unwrap()).collect()),
    }
}

fn curve_values(job: &Job, basis: Basis) -> Vec<f64> {
    let read = |t: &String| t.parse::<f64>().unwrap();
    let weights: Option<Vec<f64>> = job.weights.as_ref().map(|w| w.iter().map(read).collect());
    if job.points[0].len() == 2 {
        values::<2>(job, basis, weights)
    } else {
        values::<3>(job, basis, weights)
    }
}

fn values<const D: usize>(job: &Job, basis: Basis, weights: Option<Vec<f64>>) -> Vec<f64> {
    let read = |t: &String| t.parse::<f64>().unwrap();
    let homogeneous: Vec<[f64; D]> = job
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let w = weights.as_ref().map_or(1.0, |w| w[i]);
            let mut h = [0.0; D];
            for (slot, x) in h.iter_mut().zip(p) {
                *slot = read(x) * w;
            }
            h
        })
        .collect();
    let curve = Curve::from_homogeneous(basis, homogeneous, weights).unwrap();
    let at = read(&job.at);
    if job.op == Op::Point {
        return curve.point(at, Limit::FromRight).unwrap().to_vec();
    }
    let changed = curve
        .insert_knot(at, 1, &mut WorkMeter::new(Budget::UNLIMITED))
        .unwrap();
    let new = changed.value;
    let mut out = new.basis().knots().to_vec();
    out.extend(new.homogeneous().iter().flatten());
    out
}

/// `honest(job)` with its first value moved by `delta`: a reference with a flaw.
pub fn flawed(job: &Job, delta: f64) -> Answer {
    let mut answer = honest(job);
    let values = answer.values.as_mut().unwrap();
    let moved = values[0].parse::<f64>().unwrap() + delta;
    values[0] = float(moved).unwrap();
    answer
}
