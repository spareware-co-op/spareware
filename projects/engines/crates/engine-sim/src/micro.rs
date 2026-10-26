//! Microsimulation: vehicles driving along paths of arcs on a fixed time step.
//!
//! Give it a network ([`MicroNet`]: arc lengths and speeds, how each arc ends, the signal
//! timings), the routes vehicles are released on ([`MicroRoute`]) and a random source
//! ([`Rng`]); call [`MicroSim::step`] and read the vehicles back. Vehicles follow the
//! Intelligent Driver Model (car-following with a desired speed, a time headway and a braking
//! limit), queue at red signals, give way at priority junctions by gap acceptance, yield at
//! roundabout entries, take turns at merges and keep left. A vehicle held at a line for two
//! minutes in a gridlock is moved on and counted. Buses use the bus lane where an arc has one.
//!
//! The same inputs and the same random numbers always give the same run, and nothing here
//! knows about roads, maps or any program's data: Spare Construct's traffic simulation builds
//! the network from its graph and runs this; any program with arcs and routes can do the same.
//!
//! Port of the transport prototype's sim.js `Sim` (the movement, signals and junction rules),
//! operation for operation, with JavaScript's `Math.max` and `Math.min` for NaN and signed zero.
//! The one place it differs from the prototype's numbers is the last bit of `Math.log` and
//! `Math.pow`, which Rust takes from the platform's C library.

mod drive;
mod net;
mod rng;
mod run;
mod state;

pub use net::{Control, MicroArc, MicroNet, MicroParams, MicroRoute, SignalPlan};
pub use rng::{Mulberry32, Rng};
pub use run::MicroSim;
pub use state::{Kind, Light, Permit, RouteState, Sample, Snapshot, Stats, Vehicle};

/// `Math.max(a, b)`: NaN if either is NaN, and +0 is above -0 (unlike `f64::max`).
fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.is_sign_positive() || b.is_sign_positive() {
            0.0
        } else {
            -0.0
        };
    }
    if a > b { a } else { b }
}

/// `Math.min(a, b)`: NaN if either is NaN, and -0 is below +0 (unlike `f64::min`).
fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.is_sign_negative() || b.is_sign_negative() {
            -0.0
        } else {
            0.0
        };
    }
    if a < b { a } else { b }
}

/// The unsigned difference between two headings in degrees, 0 to 180 (`U.angDiff`).
fn ang_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 { 360.0 - d } else { d }
}
