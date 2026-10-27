//! The result of one assignment, typed: every key `Engine.assign` writes, in its order.

use super::plain;
use crate::jsnum::{Maybe, Num};
use crate::project::OrderedMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What `Engine.assign(sc, { period })` returns: one scenario in one period.
///
/// Keys are written in the order the JavaScript builds them. Fields that are `Option` with
/// "left out" in their description are omitted when `None` (the JavaScript adds them only
/// sometimes); other `Option` fields are written as null. `arc_keys` and `paths` are what
/// `Store.serialise` strips from saved projects, so a result read from a saved project has
/// neither (paths is then missing, not null).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Res {
    /// Period ("AM", "PM").
    pub period: String,
    /// Scenario year (`yearOf(sc, S)`).
    #[serde(with = "plain")]
    pub year: f64,
    /// Time stamp of the run (ms since 1970).
    #[serde(with = "plain")]
    pub ts: f64,
    /// Iterations run.
    #[serde(with = "plain")]
    pub it: f64,
    /// Final relative gap.
    #[serde(with = "plain")]
    pub gap: f64,
    /// Run time (ms).
    #[serde(with = "plain")]
    pub ms: f64,
    /// Set by the interface when the inputs changed after the run.
    pub stale: bool,
    /// The run met the gap target (and stability rule) before the iteration limit.
    pub converged: bool,
    /// The run stopped at the iteration limit.
    pub hit_max: bool,
    /// Gap target.
    #[serde(with = "plain")]
    pub gap_tol: f64,
    /// Iteration limit.
    #[serde(with = "plain")]
    pub max_iter: f64,
    /// The TAG stability rule was on.
    pub stable_rule: bool,
    /// Convergence sentence (convText).
    pub conv_note: String,
    /// Gap, stability and VHT by iteration.
    pub conv: Vec<ConvRow>,
    /// 95th percentile of the change in arc flow over the last five updates (veh/h); null with fewer than six.
    #[serde(with = "plain")]
    pub noise95: Option<f64>,
    /// Background growth factor and its basis.
    pub growth: GrowthInfo,
    /// Junction analysis settings used.
    pub junction: JunctionInfo,
    /// Link results by arc key ("<link id>:ab" / ":ba"), in arc order (connectors left out).
    pub arcs: OrderedMap<ArcRes>,
    /// Junction results by controller node id.
    pub nodes: OrderedMap<NodeRes>,
    /// Turning movements by node id.
    pub turns: OrderedMap<Vec<TurnRow>>,
    /// Bus route results by route id.
    pub routes: OrderedMap<RouteRes>,
    /// Link flows by development class.
    pub cls: ClassFlows,
    /// Development trips by zone and gateway.
    pub dev: DevRes,
    /// Select-link, select-turn or select-zone analysis (left out when none was asked for).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<Select>,
    /// Junction sites by id (left out when the scenario has no sites).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sites: Option<OrderedMap<SiteRes>>,
    /// Movements through each site, entry to exit (left out when the scenario has no sites).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_turns: Option<OrderedMap<Vec<TurnRow>>>,
    /// "paths" (traced along the assignment's paths) or "shares" (left out when no sites).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_turns_basis: Option<String>,
    /// The scenario the run was for.
    pub scenario: ScenarioInfo,
    /// Model status, convergence and data vintages (modelMeta).
    pub meta: ModelMeta,
    /// Network totals.
    pub kpi: Kpi,
    /// Passenger car units per bus used for route preloads (left out without routes).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub bus_pcu: Option<f64>,
    /// Zone ids, the order of O, D and T.
    pub zone_ids: Vec<String>,
    /// Trips from each zone (veh/h).
    #[serde(rename = "O", with = "plain")]
    pub o: Vec<f64>,
    /// Trips to each zone (veh/h).
    #[serde(rename = "D", with = "plain")]
    pub d: Vec<f64>,
    /// Trips from each zone before matrix estimation (left out when it did not apply).
    #[serde(
        rename = "O0",
        default,
        skip_serializing_if = "Option::is_none",
        with = "plain"
    )]
    pub o0: Option<Vec<f64>>,
    /// Trips to each zone before matrix estimation (left out when it did not apply).
    #[serde(
        rename = "D0",
        default,
        skip_serializing_if = "Option::is_none",
        with = "plain"
    )]
    pub d0: Option<Vec<f64>>,
    /// OD matrix row by row, rounded to 0.1 veh/h; null with more than 150 zones.
    #[serde(rename = "T", with = "plain")]
    pub t: Option<Vec<f64>>,
    /// Demand notes.
    pub note: String,
    /// The period used a manual OD matrix.
    pub manual: bool,
    /// Every arc's key by arc index (paths refer to arcs by index); missing in saved projects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_keys: Option<Vec<String>>,
    /// Paths by OD pair with their flows (pcu/h); null with more than 300 zones; missing in saved projects.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub paths: Maybe<Vec<OdPaths>>,
}

/// One row of `res.conv`: an iteration's gap, stability share P and VHT.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConvRow {
    /// Iteration.
    #[serde(with = "plain")]
    pub it: f64,
    /// Relative gap, 3 significant digits.
    #[serde(with = "plain")]
    pub gap: f64,
    /// Share of arcs whose flow changed by less than the stability threshold; null on the first.
    #[serde(rename = "P", with = "plain")]
    pub p: Option<f64>,
    /// Vehicle hours (h), 0.1.
    #[serde(with = "plain")]
    pub vht: f64,
}

/// `res.growth`: the background growth factor and the text of its basis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrowthInfo {
    /// Factor from the base year to the scenario year.
    #[serde(with = "plain")]
    pub factor: f64,
    /// "linear, 2.0% of 2026 volume per year".
    pub basis: String,
}

/// `res.junction`: the junction analysis settings the run used.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JunctionInfo {
    /// Level of service method ("hcm6", "sidra", "rta").
    pub method: String,
    /// The method's label.
    pub label: String,
    /// Peak flow factor.
    #[serde(with = "plain")]
    pub pff: f64,
    /// Analysis period (h).
    #[serde(rename = "T", with = "plain")]
    pub t: f64,
    /// Queue space per vehicle (m).
    #[serde(with = "plain")]
    pub queue_space: f64,
    /// 95th-percentile queue factor.
    #[serde(with = "plain")]
    pub f95: f64,
    /// JUNCTION_LABEL.
    pub honest: String,
    /// "arc" or "movement".
    pub cap_method: String,
    /// Movement capacity label (left out with the arc method).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_label: Option<String>,
    /// Roundabout lane capacity label (left out with the arc method).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rbt_label: Option<String>,
}

/// One arc's result (`res.arcs[key]`); flows in veh/h unless pcu is said.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArcRes {
    /// Flow (veh/h), 0.1.
    #[serde(with = "plain")]
    pub v: f64,
    /// Flow with bus preloads (pcu/h), 0.1.
    #[serde(with = "plain")]
    pub pcu: f64,
    /// Capacity (pcu/h), whole.
    #[serde(with = "plain")]
    pub cap: f64,
    /// Volume over capacity, 0.001.
    #[serde(with = "plain")]
    pub vc: f64,
    /// Travel time (s), 0.1; charges are not time.
    #[serde(with = "plain")]
    pub t: f64,
    /// Free-flow time (s), 0.1.
    #[serde(with = "plain")]
    pub ff: f64,
    /// Speed (km/h), 0.1.
    #[serde(with = "plain")]
    pub spd: f64,
    /// Junction delay at the downstream end (s), 0.1.
    #[serde(with = "plain")]
    pub nd: f64,
    /// Length (m), whole.
    #[serde(with = "plain")]
    pub len: f64,
    /// Link V/C band letter.
    pub los: String,
    /// Buses an hour preloaded.
    #[serde(with = "plain")]
    pub bus: f64,
    /// Mid-block capacity (pcu/h), whole.
    #[serde(with = "plain")]
    pub cap_mid: f64,
    /// What sets the capacity: "mid-block", "signals", "give way", "stop", "roundabout" or "junction".
    pub cap_src: String,
    /// The controlling junction's node id (left out when the mid-block sets the capacity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_node: Option<String>,
    /// Time-of-use charge (s), 0.1 (left out when none).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub toll: Option<f64>,
}

/// One junction's result (`res.nodes[id]`, junctionResult).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRes {
    /// Control: "signals", "roundabout", "priority" or "stop".
    pub r#type: String,
    /// Approaches.
    pub appr: Vec<Approach>,
    /// Reported delay (s): the volume-weighted average, or the worst movement at give-way and stop.
    #[serde(with = "plain")]
    pub delay: f64,
    /// "average" or "worst movement".
    pub delay_basis: String,
    /// Volume-weighted average delay (s).
    #[serde(with = "plain")]
    pub avg_delay: f64,
    /// Worst movement delay (s).
    #[serde(with = "plain")]
    pub worst_delay: f64,
    /// The worst approach; null when every approach is free.
    pub worst_leg: Option<WorstLeg>,
    /// Largest degree of saturation.
    #[serde(with = "plain")]
    pub dos: f64,
    /// Total approach volume (veh/h), whole.
    #[serde(with = "plain")]
    pub vol: f64,
    /// Level of service.
    pub los: String,
    /// Intersection level of service; null at give-way and stop junctions.
    pub intersection_los: Option<String>,
    /// "worst movement" or "intersection delay".
    pub los_basis: String,
    /// "C" or "F (side road, South approach)".
    pub los_text: String,
    /// Practical degree of saturation.
    #[serde(with = "plain")]
    pub xp: f64,
    /// Level of service method.
    pub method: String,
    /// "arc" or "movement".
    pub cap_method: String,
    /// Capacity label (movement method); null otherwise and at signals.
    pub cap_label: Option<String>,
    /// Signal cycle (s); null away from signals.
    #[serde(rename = "C", with = "plain")]
    pub c: Option<f64>,
    /// Signal green split; null away from signals.
    #[serde(with = "plain")]
    pub split: Option<Vec<f64>>,
    /// Junction site id (left out for a single node).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// Member node ids of the site (left out for a single node).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<String>>,
}

/// One approach of a junction (a row of `appr`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approach {
    /// Arc key.
    pub key: String,
    /// Link id.
    pub link: String,
    /// Road name.
    pub name: String,
    /// Bearing in (degrees), whole.
    #[serde(with = "plain")]
    pub in_b: f64,
    /// "North", "South east" ...
    pub approach: String,
    /// Flow (veh/h).
    #[serde(with = "plain")]
    pub v: f64,
    /// Capacity (veh/h), whole.
    #[serde(with = "plain")]
    pub cap: f64,
    /// Capacity (pcu/h), whole.
    #[serde(with = "plain")]
    pub cap_pcu: f64,
    /// Degree of saturation at the peak flow rate, 0.001.
    #[serde(with = "plain")]
    pub dos: f64,
    /// Practical degree of saturation; null when free.
    #[serde(with = "plain")]
    pub xp: Option<f64>,
    /// Practical spare capacity (%); null when free or without flow.
    #[serde(with = "plain")]
    pub psc: Option<f64>,
    /// "ok", "near" or "over"; null when free.
    pub dos_band: Option<String>,
    /// Delay (s), 0.1.
    #[serde(with = "plain")]
    pub delay: f64,
    /// Level of service ("F" over capacity, "–" when free).
    pub los: String,
    /// Level of service from delay alone.
    pub los_delay: String,
    /// Back of queue, 95th percentile (m), whole.
    #[serde(with = "plain")]
    pub q95: f64,
    /// Back of queue, 95th percentile (vehicles), 0.1.
    #[serde(with = "plain")]
    pub q95veh: f64,
    /// Signal phase (left out away from signals).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub phase: Option<f64>,
    /// A major-road approach.
    pub major: bool,
    /// Arc control: "free", "sig", "minor", "rbt" ...
    pub ctrl: String,
    /// Movement rows (left out unless the movement method applies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mov: Option<Vec<MovRow>>,
    /// Lane rows (left out unless the movement method applies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanes: Option<Vec<LaneRow>>,
    /// Where the lane model came from ("default", "user" ...); null when unknown (left out unless the movement method applies).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lane_src: Maybe<String>,
    /// "movement" (left out unless the movement method applies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_method: Option<String>,
    /// Through capacity on a major approach (veh/h) (left out elsewhere).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub cap_t: Option<f64>,
    /// Right-turn bay storage (vehicles) on a major approach (left out elsewhere).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub bay: Option<f64>,
    /// Share of time right turners block the through lane on a major approach (left out elsewhere).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub right_block: Option<f64>,
    /// Short-lane factor, when below 1 (left out otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub short_lane_factor: Option<f64>,
}

/// One movement row of an approach (movRows).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovRow {
    /// "L", "T" or "R".
    pub mv: String,
    /// Demand (veh/h, hourly), 0.1.
    #[serde(with = "plain")]
    pub v: f64,
    /// Capacity (veh/h), whole.
    #[serde(with = "plain")]
    pub cap: f64,
    /// Degree of saturation, 0.001.
    #[serde(with = "plain")]
    pub dos: f64,
    /// Delay (s), 0.1.
    #[serde(with = "plain")]
    pub delay: f64,
    /// Level of service.
    pub los: String,
    /// Back of queue (vehicles), 0.1.
    #[serde(with = "plain")]
    pub q95veh: f64,
    /// Back of queue (m), whole.
    #[serde(with = "plain")]
    pub q95: f64,
    /// Practical spare capacity (%); null without saturation.
    #[serde(with = "plain")]
    pub psc: Option<f64>,
    /// Gap-acceptance rank.
    #[serde(with = "plain")]
    pub rank: f64,
    /// Conflicting flow (veh/h) (left out for movements without gap acceptance).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub vc: Option<f64>,
    /// Two-stage crossing (left out for movements without gap acceptance).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub two_stage: Option<bool>,
}

/// One lane row of an approach (movRows).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaneRow {
    /// Movements the lane carries ("LT", "R" ...).
    pub d: String,
    /// Short-lane length (m); 0 for a full lane.
    #[serde(with = "plain")]
    pub len: f64,
    /// A slip lane.
    pub slip: bool,
    /// Flow (veh/h), 0.1.
    #[serde(with = "plain")]
    pub v: f64,
    /// Capacity (veh/h), whole.
    #[serde(with = "plain")]
    pub cap: f64,
    /// Degree of saturation, 0.001.
    #[serde(with = "plain")]
    pub dos: f64,
    /// Blocked by queued right turners (left out otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<bool>,
}

/// The worst approach of a junction (`worstLeg`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorstLeg {
    /// Arc key.
    pub key: String,
    /// Road name.
    pub name: String,
    /// "North" ...
    pub approach: String,
    /// Delay (s).
    #[serde(with = "plain")]
    pub delay: f64,
    /// Degree of saturation.
    #[serde(with = "plain")]
    pub dos: f64,
    /// Level of service.
    pub los: String,
}

/// One turning movement (`res.turns[node]` and `res.siteTurns[site]` rows).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnRow {
    /// Arc key in.
    pub from: String,
    /// Arc key out.
    pub to: String,
    /// Movement: "L", "T", "R", "U" ...
    pub mv: String,
    /// Flow (veh/h), 0.1.
    #[serde(with = "plain")]
    pub v: f64,
    /// Flow (pcu/h), 0.1.
    #[serde(with = "plain")]
    pub pcu: f64,
    /// Development traffic in the movement (veh/h) (left out when none).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub dev: Option<f64>,
}

/// One bus route's result (`res.routes[id]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteRes {
    /// Length (m), whole.
    #[serde(with = "plain")]
    pub len: f64,
    /// Run time with stops (s), whole.
    #[serde(with = "plain")]
    pub t: f64,
    /// Average speed (km/h), 0.1; null for a route of no length.
    #[serde(with = "plain")]
    pub spd: Option<f64>,
    /// Buses an hour.
    #[serde(with = "plain")]
    pub freq: f64,
}

/// `res.cls`: link flows by development class (existing, committed, development).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassFlows {
    /// Class names, ZCLASS.
    pub names: Vec<String>,
    /// Trips by class (veh/h), 0.1.
    #[serde(with = "plain")]
    pub trips: Vec<f64>,
    /// [existing, committed, development] flows (veh/h) by arc key, where any is committed or development.
    #[serde(with = "plain")]
    pub arcs: OrderedMap<Vec<f64>>,
}

/// `res.dev`: development trips, by development zone and by gateway.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevRes {
    /// Development trips (veh/h), 0.1.
    #[serde(with = "plain")]
    pub trips: f64,
    /// Trips from (O) and to (D) each development zone.
    pub zones: OrderedMap<DevZone>,
    /// Development trips arriving (in) and leaving (out) through each gateway.
    pub gateways: OrderedMap<DevGateway>,
    /// Development trips through gateways (veh/h), 0.1.
    #[serde(with = "plain")]
    pub via_gateways: f64,
}

/// A development zone's trips (veh/h).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DevZone {
    /// From the zone.
    #[serde(rename = "O", with = "plain")]
    pub o: f64,
    /// To the zone.
    #[serde(rename = "D", with = "plain")]
    pub d: f64,
}

/// Development trips through a gateway (veh/h).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DevGateway {
    /// Arriving through it.
    #[serde(rename = "in", with = "plain")]
    pub r#in: f64,
    /// Leaving through it.
    #[serde(with = "plain")]
    pub out: f64,
}

/// `res.select`: a select-link, select-turn or select-zone analysis, or why it could not run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Select {
    /// "zone", "arc" or "turn".
    pub kind: String,
    /// What was selected: a zone id, an arc key or [from key, to key] (left out with an error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Value>,
    /// Selected flows by arc key (veh/h) (left out with an error).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub flows: Option<OrderedMap<f64>>,
    /// Selected turning movements by node id (left out with an error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turns: Option<OrderedMap<Vec<SelectTurn>>>,
    /// OD pairs over 0.05 veh/h, largest first, at most 2,000 (left out with an error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ods: Option<Vec<OdFlow>>,
    /// OD pairs that use the selection (left out with an error).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub od_pairs: Option<f64>,
    /// Total selected trips (veh/h) (left out with an error).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub tot: Option<f64>,
    /// SELECT_LABEL (left out with an error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Why the selection is not in the network (left out otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A selected turning movement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelectTurn {
    /// Arc key in.
    pub from: String,
    /// Arc key out.
    pub to: String,
    /// Movement.
    pub mv: String,
    /// Flow (veh/h), 0.1.
    #[serde(with = "plain")]
    pub v: f64,
}

/// An OD pair and its trips (veh/h).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OdFlow {
    /// Origin zone id.
    pub o: String,
    /// Destination zone id.
    pub d: String,
    /// Trips (veh/h).
    #[serde(with = "plain")]
    pub q: f64,
}

/// One junction site's result (`res.sites[id]`, siteResults).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteRes {
    /// Site id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Member node ids.
    pub nodes: Vec<String>,
    /// Representative node id.
    pub rep: String,
    /// Control.
    pub r#type: String,
    /// AT controlled-intersection site number(s), or null.
    pub at_site_no: Value,
    /// One controller for the whole site.
    pub one: bool,
    /// Legs: the arcs entering and leaving on each side.
    pub legs: Vec<SiteLeg>,
    /// The approaches from outside the site together (roundabout sites; left out otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<SiteResult>,
}

/// A leg of a junction site.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteLeg {
    /// Leg number.
    #[serde(with = "plain")]
    pub k: f64,
    /// Road name.
    pub name: String,
    /// Bearing outward from the site (degrees), whole.
    #[serde(with = "plain")]
    pub b: f64,
    /// Arc keys entering the site on this leg.
    pub ins: Vec<String>,
    /// Arc keys leaving the site on this leg.
    pub outs: Vec<String>,
}

/// A site's combined result (roundabout sites with one controller per entry node).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteResult {
    /// Control.
    pub r#type: String,
    /// The approaches from outside the site.
    pub appr: Vec<Approach>,
    /// Average delay (s), 0.1.
    #[serde(with = "plain")]
    pub delay: f64,
    /// Average delay (s), 0.1.
    #[serde(with = "plain")]
    pub avg_delay: f64,
    /// "average".
    pub delay_basis: String,
    /// Largest degree of saturation, 0.001.
    #[serde(with = "plain")]
    pub dos: f64,
    /// Total approach volume (veh/h), whole.
    #[serde(with = "plain")]
    pub vol: f64,
    /// Level of service.
    pub los: String,
    /// Intersection level of service.
    pub intersection_los: String,
    /// Level of service text.
    pub los_text: String,
}

/// Paths of one OD pair with their flows (`res.paths` rows).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OdPaths {
    /// Origin zone index (into zoneIds).
    pub i: usize,
    /// Destination zone index.
    pub j: usize,
    /// The pair's paths with flow over 0.01 pcu/h.
    pub list: Vec<PathFlow>,
}

/// One path: arc indexes (into arcKeys) in order, and its flow w (pcu/h).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathFlow {
    /// Arc indexes.
    pub arcs: Vec<usize>,
    /// Flow (pcu/h).
    #[serde(with = "plain")]
    pub w: f64,
}

/// `res.scenario`: the scenario the run was for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioInfo {
    /// Scenario id.
    pub id: String,
    /// Role, or null.
    pub role: Option<String>,
    /// Staging (% built) as saved, or null.
    #[serde(default)]
    pub buildout: Num,
    /// Demand factor applied (1 unless a sensitivity test scales the matrix).
    #[serde(with = "plain")]
    pub demand_factor: f64,
}

/// `res.meta` (modelMeta): model status, convergence, data vintages and the bases of the run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMeta {
    /// Model status.
    pub status: ModelStatus,
    /// Convergence.
    pub convergence: Convergence,
    /// Data vintages by source ("nzta", "at", "user", "fc", "controlled", "cas").
    pub vintages: OrderedMap<Vintage>,
    /// Base year as saved in the settings (left out when the settings have none).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub base_year: Num,
    /// Scenario year.
    #[serde(with = "plain")]
    pub year: f64,
    /// Growth basis text.
    pub growth_basis: String,
    /// Level of service method.
    pub los_method: String,
    /// Capacity method.
    pub cap_method: String,
}

/// `meta.convergence`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Convergence {
    /// Converged.
    pub converged: bool,
    /// Stopped at the iteration limit.
    pub hit_max: bool,
    /// Final gap.
    #[serde(with = "plain")]
    pub gap: f64,
    /// Gap target.
    #[serde(with = "plain")]
    pub gap_tol: f64,
    /// Iterations.
    #[serde(with = "plain")]
    pub it: f64,
    /// Convergence sentence.
    pub note: String,
}

/// A data vintage: where data came from, when it was loaded and how much.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vintage {
    /// Source name.
    pub source: Value,
    /// Date loaded (YYYY-MM-DD).
    pub date: Option<String>,
    /// Records loaded, or null.
    pub n: Value,
    /// Licence (left out when none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<Value>,
    /// Crash data window (crash data only; null when not given).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub window: Maybe<Value>,
}

/// modelStatus: Uncalibrated, Calibrated or Validated, with the TMDG category and its basis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelStatus {
    /// "uncalibrated", "calibrated" or "validated".
    pub status: String,
    /// "Uncalibrated", "Calibrated" or "Validated".
    pub short: String,
    /// The status in a sentence.
    pub label: String,
    /// Period.
    pub period: String,
    /// TMDG category.
    pub category: String,
    /// Calibration criteria ("tmdg", "tag", "rms", "atap").
    pub standard: String,
    /// The author's justification of the category.
    pub justification: String,
    /// Date of the validation or calibration record, or null.
    pub date: Value,
    /// Why an earlier validation no longer counts (left out when none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale: Option<String>,
    /// Footer text for reports.
    pub footer: String,
    /// TMDG's standing note.
    pub standing: String,
}

/// `res.kpi`: network totals for the period.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kpi {
    /// Trips (veh/h).
    #[serde(with = "plain")]
    pub trips: f64,
    /// Vehicle kilometres.
    #[serde(with = "plain")]
    pub vkt: f64,
    /// Vehicle hours.
    #[serde(with = "plain")]
    pub vht: f64,
    /// Delay (vehicle hours).
    #[serde(with = "plain")]
    pub dly: f64,
    /// Average speed (km/h).
    #[serde(with = "plain")]
    pub spd: f64,
    /// CO2 (t).
    #[serde(with = "plain")]
    pub co2: f64,
    /// Arcs over capacity.
    #[serde(with = "plain")]
    pub over: f64,
    /// Largest V/C.
    #[serde(rename = "maxVC", with = "plain")]
    pub max_vc: f64,
    /// Trips with no route (veh/h).
    #[serde(with = "plain")]
    pub unserved: f64,
    /// The junction with the longest delay; null without junctions.
    pub worst: Option<KpiWorst>,
    /// Lane kilometres.
    #[serde(with = "plain")]
    pub lane_km: f64,
    /// Hours of charges paid (left out without charges).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "plain")]
    pub charge_h: Option<f64>,
}

/// The options `Engine.assign(sc, opts)` reads besides the settings (`opts.settings`, passed
/// separately) and the progress callback; values as loose as the JavaScript's (`+x > 0` tests).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignOpts {
    /// Gap target (used when above 0; otherwise the settings').
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub gap_tol: Num,
    /// Iteration limit (used when above 0; otherwise the settings').
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub max_iter: Num,
    /// Period ("AM", "PM"); None: the settings' period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    /// Factor on the whole OD matrix (used when above 0; otherwise 1).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub demand_factor: Num,
    /// Select-link, select-turn or select-zone: { arc: key }, { turn: [from key, to key] } or { zone: id }.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<Value>,
    /// Extra charge (s) by arc key or link id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_penalty: Option<Value>,
}

/// `kpi.worst`: the junction with the longest delay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KpiWorst {
    /// Node id.
    pub id: String,
    /// Delay (s).
    #[serde(with = "plain")]
    pub delay: f64,
    /// Level of service.
    pub los: String,
}
