//! Link and junction performance from engine.js.
//!
//! BPR link times, signal timing (Webster and Akcelik delay), gap-acceptance capacity at
//! give-way and stop junctions (per arc, and per movement with the lane model), roundabout
//! entry capacity, junction results, level of service and back of queue. Port of engine.js,
//! "link and junction performance", "movement-based capacity" and "junction results".
//! Turn penalties and bans are part of the graph ([`crate::graph`]).
//!
//! The movement method (settings.junction.capMethod "movement"): priority junctions by the
//! HCM 6 two-way stop control method mirrored for left-hand traffic (rank impedance, two-stage
//! crossings, shared and short lanes), roundabouts by HCM 6 lane equations; each entry's
//! capacity is charged to its arc, and the movement detail stays on the arc for the junction
//! tables. `Math.exp` and `Math.pow` are the platform's here and may differ from V8's in the
//! last bit, well inside the truth tests' tolerance.

mod lanes;
mod movement;
mod moves;
mod perf;
mod report;

pub use lanes::{
    Alloc, ApproachCap, Lane, LaneModel, LaneRow, Lmv, alloc_lanes, approach_cap, lane_model,
};
pub use movement::{
    CapOpts, EntGaps, EntryRes, MajorRes, MajorState, MoveCap, MoveEntry, Movs, RbtRes, cpot,
    d_hcm, p_prime, p0of, prio_geom, priority_caps, q95v, rbt_caps, two_stage,
};
pub(crate) use moves::prep_moves;
pub use moves::{ctl_flows, movement_caps, site_exits, site_mv};
pub use perf::{PerfSet, arc_cap, arc_time, eff_cap, major_delay, update_node_caps};
pub use report::{
    ApproachRow, ArcFlow, ConvInfo, JUNCTION_LABEL, JunctionResult, MovRow, MovRows, WorstLeg,
    cap_src, conv_text, delay_text, junction_result, mov_rows,
};
