//! What a run holds and reports: vehicles, route queues, statistics and signal lights.

use super::MicroRoute;
use std::collections::VecDeque;

/// The kind of a vehicle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A car: 4.6 m long, quick to accelerate, a driver headway between 1.3 and 1.7 s.
    Car,
    /// A heavy vehicle: 12 m long, slow to accelerate, capped at 25 m/s.
    Heavy,
    /// A bus: 12 m long, like a heavy vehicle, and the only kind allowed in a bus lane.
    Bus,
}

impl Kind {
    /// "car", "hv" or "bus".
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Car => "car",
            Kind::Heavy => "hv",
            Kind::Bus => "bus",
        }
    }
}

/// Leave to enter a lane of the next arc, granted to a vehicle approaching the end of its arc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permit {
    /// The next arc.
    pub arc: usize,
    /// The lane of it the vehicle may enter.
    pub lane: usize,
}

/// One vehicle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vehicle {
    /// Number in order of release, from 0.
    pub id: u64,
    /// Index of its route.
    pub route: usize,
    /// How far along its route's arcs it is (index of the arc it is on).
    pub k: usize,
    /// The arc it is on (or will start on, while it waits to enter).
    pub arc: usize,
    /// The lane it is in.
    pub lane: usize,
    /// Position of its front along the arc (m).
    pub s: f64,
    /// Speed (m/s).
    pub v: f64,
    /// Acceleration chosen this step (m/s squared).
    pub acc: f64,
    /// Car, heavy vehicle or bus.
    pub kind: Kind,
    /// Length (m).
    pub len: f64,
    /// Greatest acceleration it uses (m/s squared).
    pub a_max: f64,
    /// Time headway it keeps to the vehicle ahead (s).
    pub headway: f64,
    /// Its own share of the speed limit it drives at (0.9 to 1.1).
    pub speed_factor: f64,
    /// Time it was released (s).
    pub t0: f64,
    /// Free-flow time of the arcs it has been on, so far (s).
    pub ff: f64,
    /// Leave to enter the next arc, once granted.
    pub permit: Option<Permit>,
    /// Time spent nearly stopped (s); resets when it moves.
    pub wait: f64,
    /// It has come to a halt at the line of a stop-controlled approach.
    pub stopped: bool,
    /// It was held at a line for two minutes in a gridlock and is being moved on.
    pub stuck: bool,
}

/// A route while it runs: its arrivals and the vehicles waiting to enter the network.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteState {
    /// The route as given.
    pub route: MicroRoute,
    /// Time of the next release (s).
    pub next: f64,
    /// Released vehicles waiting for room on the first arc, the first to enter at the front.
    pub queue: VecDeque<Vehicle>,
}

/// One sample of the whole network (taken every 5 s of simulated time).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Time (s).
    pub t: f64,
    /// Vehicles on the network.
    pub n: usize,
    /// Of them, vehicles slower than 1.5 m/s.
    pub q: usize,
    /// Average speed of the vehicles on the network (km/h), 0 when empty.
    pub speed: f64,
    /// Trips finished so far.
    pub done: u64,
    /// Vehicles waiting to enter the network.
    pub waiting: usize,
}

/// Running totals of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    /// Car and heavy vehicle trips finished.
    pub done: u64,
    /// Their total travel time (s).
    pub tt: f64,
    /// Their total free-flow time (s).
    pub ff: f64,
    /// Vehicles released (including those turned away from a full queue).
    pub spawned: u64,
    /// Vehicles moved on after two minutes held at a line in a gridlock.
    pub teleports: u64,
    /// Samples, every 5 s, the latest 1,440.
    pub series: Vec<Sample>,
    /// Bus trips finished.
    pub bus_done: u64,
    /// Their total travel time (s).
    pub bus_tt: f64,
}

/// The flow on every arc at one moment, kept once a minute for the comparison with the model.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Time (s).
    pub t: f64,
    /// Vehicles that had left each arc by then.
    pub passed: Vec<f64>,
}

/// What a signal shows to an approach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light {
    /// Green: go.
    Green,
    /// Amber (3 s after green): go only if too close to stop.
    Amber,
    /// Red: stop.
    Red,
}

impl Light {
    /// "G", "A" or "R".
    pub fn as_char(self) -> char {
        match self {
            Light::Green => 'G',
            Light::Amber => 'A',
            Light::Red => 'R',
        }
    }
}
