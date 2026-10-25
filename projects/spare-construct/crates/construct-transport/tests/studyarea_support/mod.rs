//! Shared by the study area truth tests: the recorded answers to every request, the made-up projects and the box of a recorded case.
//! tools/transport-truth/gen-studyarea.mjs made the answers (fixtures/data/studyarea-exchanges.json) and ran the prototype on them.
#![allow(dead_code)] // each test file uses its own part of this module

use construct_transport::edit::Ids;
use construct_transport::project::Project;
use construct_transport::studyarea::latlng_bounds;
use engine_sim::Mulberry32;
use serde_json::Value;

/// The clock every case ran at: 2026-10-21 00:00 UTC.
pub const NOW: f64 = 1_792_540_800_000.0;

/// The answers every recorded request got, by request.
pub fn exchanges() -> Value {
    crate::support::load_json("fixtures/data/studyarea-exchanges.json")
}

/// An id generator for a case that makes no ids (the load cases): the clock is what matters.
pub fn fixed_ids() -> Ids<Mulberry32> {
    Ids::new(Mulberry32::new(1), NOW)
}

/// The made-up project a case starts from (fixtures/data/<name>.json).
pub fn project(name: &str) -> Project {
    Project::from_json(&crate::support::read_truth(&format!("fixtures/data/{name}.json"))).expect("the project reads")
}

/// A box recorded as `[[south, west], [north, east]]`.
pub fn corners(v: &Value) -> construct_transport::census::Bx {
    let n = |i: usize, j: usize| v[i][j].as_f64().expect("a number");
    latlng_bounds(&[[n(0, 0), n(0, 1)], [n(1, 0), n(1, 1)]])
}
