//! Assignment from engine.js.
//!
//! Edge-based shortest paths, bus route preloads, conjugate Frank-Wolfe user equilibrium
//! with junction capacities, the TAG stability rule and MSA capacity damping, signal
//! optimisation, class and development flows, select-link tracking and the result object.
//! Port of engine.js, "assignment": [`assign`] runs one scenario in one period and gives the
//! [`Res`](crate::results::Res) the prototype's `Engine.assign` gives.
//!
//! The prototype's `assign` is async only to let the page draw between iterations; this one is
//! synchronous and reports progress through a callback instead. Floating-point sums are made in
//! the same order as the JavaScript's, so the iteration count and gaps match.

// The loops and comparisons mirror the JavaScript's, index for index, so that the sums come out in the same order and
// NaN takes the same branches (`!(a > b)` is not `a <= b`)
#![allow(clippy::needless_range_loop, clippy::neg_cmp_op_on_partial_ord)]

mod build;
mod paths;
mod run;
mod sites;

use crate::demand::demand;
use crate::graph::build_graph;
use crate::project::{Project, Scenario, Settings};
use crate::results::{AssignOpts, Res};
use run::Run;
use std::time::Instant;

/// `Engine.assign(sc, opts)`: the equilibrium assignment of a scenario in a period, with junction delay and capacity, and its results.
///
/// `p` is the open project (`Store.P`: its matrix estimation, fitted factors, validation and data
/// records), `s` the settings to run with (`opts.settings`, or the project's). The options are
/// `period` (else the settings' period), `gapTol` and `maxIter` (when above 0), `demandFactor`
/// (a factor on the whole matrix, when above 0), `select` (`{ arc: key }`, `{ turn: [fromKey,
/// toKey] }` or `{ zone: id }`, tracked through the assignment) and `arcPenalty` (extra seconds
/// by arc key or link id). `progress` is called as `(iteration, gap, message)` every second
/// iteration, and with "Optimising signal timings" when the signals are re-timed.
///
/// Every OD pair carries a class, development if either end is a development zone, committed if
/// either end is committed (and neither is development), otherwise existing; each class has its
/// own link flows (and development its own turn flows) on the same costs, averaged with the same
/// steps. The period must be given by the options or the settings (a settings object without one
/// reads as "AM", where the prototype would read undefined).
pub fn assign(
    p: &Project,
    sc: &Scenario,
    s: &Settings,
    opts: &AssignOpts,
    progress: Option<&mut dyn FnMut(u32, f64, &str)>,
) -> Res {
    let t0 = Instant::now();
    let period = match opts.period.as_deref() {
        Some(x) if !x.is_empty() => x,
        _ => s.period.as_str().unwrap_or("AM"),
    };
    let g = build_graph(sc, s, Some(period));
    let mut dem = demand(p, sc, &g, period, s);
    let df = if opts.demand_factor.num() > 0.0 { opts.demand_factor.num() } else { 1.0 };
    if df != 1.0 {
        for v in dem.t.iter_mut() {
            *v *= df;
        }
    }
    let mut run = Run::new(sc, s, period, opts, g, &dem, progress);
    run.solve();
    let ms = t0.elapsed().as_millis() as f64;
    build::build_res(p, sc, s, &dem, df, &run, ms)
}
