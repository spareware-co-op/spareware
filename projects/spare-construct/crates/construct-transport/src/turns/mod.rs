//! Junction outputs: turning movement tables, a junction diagram and SIDRA-friendly CSV, from the assignment's `res.turns`
//! (turns.js `Turns`).
//!
//! Left-hand traffic: arriving traffic keeps to the left of each arm and a right turn crosses the opposing flow. An
//! approach is named after the side traffic comes FROM (heading north is the South approach). Rows carry the NZ / SIDRA leg
//! ("South: Great South Road"), the turn designation ("L2") and movement id ("S-L2"), and the development share when the
//! engine tracked it. Junction sites (`res.sites`) list movements through the whole site. Light vehicles are total less
//! heavy, rounded so they add up. Zone connector movements (development accesses) come from the engine and are arms named
//! after the zone.
//!
//! NZ / SIDRA conventions: a leg's bearing is the arrival heading plus 180 degrees, snapped to 45 degree bins coded S 0, SE 1,
//! E 2, NE 3, N 4, NW 5, W 6, SW 7; two legs never share a code (greedy minimum angular error), and past eight legs the least
//! important (driveways, zone accesses) merge into the nearest leg. The turn designation t = (`o_in` - `o_out`) mod 8 is 0 U, 1 L3, 2
//! L2, 3 L1, 4 T1, 5 R1, 6 R2, 7 R3, as SIDRA's drive-on-left `ConvertMovODToMovTurn`; L / T / R / U come from its first letter.

mod csv;
mod diagram;

pub use csv::{CODE_HEAD, SIDRA_HEAD, SidraExport, SidraOpts, sidra_csv, sidra_export, turn_count_csv};
pub use diagram::{DiagramOpts, diagram};

use crate::edit::collate::locale_compare_f64;
use crate::graph::{js_sort_by, turn_type};
use crate::jsnum::{f64_to_i64, f64_to_usize, math_max, math_min, math_round, or_default, usize_to_f64};
use crate::project::{Link, Node, Scenario, Settings, road_class};
use crate::results::{Res, SiteRes, TurnRow};
use crate::util::{self, LatLng, clamp};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// turns.js MV: the movements, in table order.
pub const MV: [&str; 4] = ["L", "T", "R", "U"];

const N4: [&str; 4] = ["North", "East", "South", "West"];
const N8: [&str; 8] = [
    "North",
    "North-east",
    "East",
    "South-east",
    "South",
    "South-west",
    "West",
    "North-west",
];

/// turns.js `LEG_CODES`: the leg codes by bin (S 0, SE 1, E 2, NE 3, N 4, NW 5, W 6, SW 7).
pub const LEG_CODES: [&str; 8] = ["S", "SE", "E", "NE", "N", "NW", "W", "SW"];
/// turns.js `LEG_NAMES`: the leg names by bin.
pub const LEG_NAMES: [&str; 8] = [
    "South",
    "South-east",
    "East",
    "North-east",
    "North",
    "North-west",
    "West",
    "South-west",
];
/// turns.js `TURN_CODES`: the turn designations by (`o_in` - `o_out`) mod 8.
pub const TURN_CODES: [&str; 8] = ["U", "L3", "L2", "L1", "T1", "R1", "R2", "R3"];

/// `norm(b)`: a heading in [0, 360).
pub(crate) fn norm(b: f64) -> f64 {
    ((b % 360.0) + 360.0) % 360.0
}

/// `Turns.sideName(b, eight)`: the compass side of an arm from its outward bearing, as 4 or 8 points.
#[must_use]
pub fn side_name(b: f64, eight: bool) -> &'static str {
    let i = math_round(norm(b) / if eight { 45.0 } else { 90.0 });
    if i.is_nan() {
        return "undefined";
    }
    // i is a whole number in 0..=8 here; the remainder is below each table's length
    let k = f64_to_usize(i);
    if eight {
        N8.get(k % 8).copied().unwrap_or("undefined")
    } else {
        N4.get(k % 4).copied().unwrap_or("undefined")
    }
}

/// `Turns.approachName(inB, eight)`: the side arriving traffic comes from, given its arrival heading.
#[must_use]
pub fn approach_name(in_b: f64, eight: bool) -> &'static str {
    side_name(in_b + 180.0, eight)
}

/// `Turns.classify(inB, outB)`: the movement (L, T, R or U) from an arrival heading to a departure heading.
#[must_use]
pub fn classify(in_b: f64, out_b: f64) -> &'static str {
    turn_type(in_b, out_b)
}

/// `roadName(L)`: the road's name, or "Unnamed ramp" / "Unnamed arterial road".
fn road_name(l: &Link) -> String {
    match l.name.as_str() {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => {
            if l.cls.is("ramp") {
                "Unnamed ramp".to_string()
            } else {
                format!(
                    "Unnamed {} road",
                    road_class(l.cls.as_str()).label.to_lowercase()
                )
            }
        }
    }
}

/// `binBearing(o)`: the outward bearing of leg code o.
#[must_use]
pub fn bin_bearing(o: usize) -> f64 {
    norm(180.0 - 45.0 * usize_to_f64(o))
}

/// `Turns.legCodeOf(legB)`: the leg code of an outward leg bearing.
#[must_use]
pub fn leg_code_of(leg_b: f64) -> usize {
    let i = math_round(norm(180.0 - leg_b) / 45.0);
    // rem_euclid(8) is in 0..8, which usize holds
    usize::try_from(f64_to_i64(i).rem_euclid(8)).unwrap_or(0)
}

/// What `Turns.assignLegs` gives one leg: its code, the angular error (rounded), whether it merged into another leg, and which.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegAssign {
    /// Leg code 0 to 7.
    pub o: usize,
    /// Angular error from the bin (degrees, rounded).
    pub err: f64,
    /// Merged into the nearest coded leg (past eight legs).
    pub merged: bool,
    /// The leg it merged into.
    pub into: Option<usize>,
}

/// `Turns.assignLegs(bearings, prio)`: leg codes for outward leg bearings; a higher priority decides who keeps a code past eight legs.
#[must_use]
pub fn assign_legs(bearings: &[f64], prio: &[f64]) -> Vec<LegAssign> {
    struct Pair {
        i: usize,
        o: usize,
        e: f64,
    }
    let n = bearings.len();
    let p = |i: usize| prio.get(i).copied().map_or(0.0, |x| or_default(x, 0.0));
    // every index below is below n, so its bearing is there
    let bearing = |i: usize| bearings.get(i).copied().unwrap_or(f64::NAN);
    let mut order: Vec<usize> = (0..n).collect();
    js_sort_by(&mut order, |&a, &b| p(b) - p(a));
    let main: Vec<usize> = order.iter().take(8).copied().collect();
    let mut pairs: Vec<Pair> = Vec::new();
    for &i in &main {
        for o in 0..8 {
            pairs.push(Pair {
                i,
                o,
                e: util::ang_diff(bearing(i), bin_bearing(o)),
            });
        }
    }
    js_sort_by(&mut pairs, |a, b| {
        let d = a.e - b.e;
        if d != 0.0 && !d.is_nan() {
            d
        } else {
            usize_to_f64(a.i) - usize_to_f64(b.i)
        }
    });
    let mut res: Vec<Option<LegAssign>> = vec![None; n];
    let mut used = [false; 8];
    for pr in &pairs {
        let free = used.get(pr.o).is_some_and(|taken| !taken);
        if let Some(slot) = res.get_mut(pr.i)
            && slot.is_none()
            && free
        {
            *slot = Some(LegAssign {
                o: pr.o,
                err: math_round(pr.e),
                merged: false,
                into: None,
            });
            if let Some(taken) = used.get_mut(pr.o) {
                *taken = true;
            }
        }
    }
    // more than eight legs: join the nearest coded leg
    for &i in order.iter().skip(8) {
        // past eight legs main holds eight
        let Some(&first) = main.first() else { break };
        let mut best = first;
        for &j in &main {
            if util::ang_diff(bearing(i), bearing(j))
                < util::ang_diff(bearing(i), bearing(best))
            {
                best = j;
            }
        }
        // the eight main legs take the eight codes (every pair of them is listed), so best has one
        let Some(bo) = res.get(best).copied().flatten().map(|la| la.o) else { continue };
        if let Some(slot) = res.get_mut(i) {
            *slot = Some(LegAssign {
                o: bo,
                err: math_round(util::ang_diff(bearing(i), bin_bearing(bo))),
                merged: true,
                into: Some(best),
            });
        }
    }
    res.into_iter().flatten().collect()
}

/// `turnCode(oIn, oOut)`: the turn designation ("L2") from the arrival and departure leg codes.
#[must_use]
pub fn turn_code(o_in: usize, o_out: usize) -> &'static str {
    // ((oIn - oOut) % 8 + 8) % 8 is (oIn % 8 + 8 - oOut % 8) % 8; Bound: that stays in 1..=15, so nothing saturates
    let k = (o_in % 8).saturating_add(8).saturating_sub(o_out % 8) % 8;
    // k is below 8, the length of TURN_CODES
    TURN_CODES.get(k).copied().unwrap_or("U")
}

/// `movementId(oIn, oOut)`: the movement id ("S-L2").
#[must_use]
pub fn movement_id(o_in: usize, o_out: usize) -> String {
    format!("{}-{}", leg_of(o_in).0, turn_code(o_in, o_out))
}

/// `legLabel(o, road)`: "South: Great South Road".
#[must_use]
pub fn leg_label(o: usize, road: &str) -> String {
    let name = leg_of(o).1;
    if road.is_empty() {
        name.to_string()
    } else {
        format!("{name}: {road}")
    }
}

/// `Turns.rank(mv, major)`: the priority rank of a movement from a major road (1 or 2) or a minor road (2, 3 or 4).
#[must_use]
pub fn rank(mv: &str, major: bool) -> f64 {
    if major {
        if mv == "T" || mv == "L" { 1.0 } else { 2.0 }
    } else if mv == "L" {
        2.0
    } else if mv == "T" {
        3.0
    } else {
        4.0
    }
}

/// `endB(pts, last)`: the bearing over the first or last 20 m of a polyline (as the engine measures arc bearings).
fn end_b(pts: &[LatLng], last: bool) -> f64 {
    let c = util::cum_len(pts);
    // cum_len gives [0] for no points, so there is a last entry
    let len = c.last().copied().unwrap_or(0.0);
    let e = math_min(20.0, len / 2.0);
    // an arm's shape holds both end nodes; a shape without points has no bearing (NaN)
    if last {
        pts.last().map_or(f64::NAN, |end| util::bearing(&util::along(pts, &c, len - e).p, end))
    } else {
        pts.first().map_or(f64::NAN, |start| util::bearing(start, &util::along(pts, &c, e).p))
    }
}

/// `hvOf(v, pcu, S)`: heavy vehicles from PCU; where PCU carries no heavy-vehicle information (PCU per heavy vehicle of 1 or
/// less, or no pcu value) the heavy-vehicle share setting is used instead.
fn hv_of(v: f64, pcu: Option<f64>, s: &Settings) -> f64 {
    // `!(v > 0)`: NaN gives 0 too
    if v <= 0.0 || v.is_nan() {
        return 0.0;
    }
    let pcu_hv = s.pcu_hv.num();
    match pcu {
        Some(pcu) if pcu_hv > 1.0 => clamp(v * (pcu / v - 1.0) / (pcu_hv - 1.0), 0.0, v),
        _ => clamp(v * s.hv_pct.or(0.0) / 100.0, 0.0, v),
    }
}

/// One arm of a junction: a road, a zone connector or (for a site) a leg, with its outward bearing and name.
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the prototype's arm object (written by to_value), whose four flags isIn, isOut, conn and legMerged are independent and read across the crate"
)]
pub struct Arm {
    /// Link id, "c:ZONE:NODE" for a zone connector, "legK" for a site leg.
    pub id: String,
    /// Link id (None for a connector or a site leg).
    pub link: Option<String>,
    /// Class rank of the link (None when there is no link).
    pub link_rank: Option<f64>,
    /// A zone connector.
    pub conn: bool,
    /// Road name ("Unnamed collector road", the zone's name).
    pub road: String,
    /// Outward bearing (degrees).
    pub b: f64,
    /// Key of the arc arriving at the junction along this arm.
    pub in_key: String,
    /// Key of the arc leaving along this arm.
    pub out_key: String,
    /// Traffic arrives along it.
    pub is_in: bool,
    /// Traffic leaves along it.
    pub is_out: bool,
    /// Outward bearing (the side the arm is on).
    pub side: f64,
    /// Name of the approach ("North", "South-east", "North (Great South Road)").
    pub name: String,
    /// Leg code 0 to 7 (set by [`code_arms`]).
    pub o: usize,
    /// Leg code letters ("S").
    pub leg: &'static str,
    /// Leg name ("South").
    pub leg_name: &'static str,
    /// Leg label ("South: Great South Road").
    pub leg_label: String,
    /// Merged into another leg (past eight legs).
    pub leg_merged: bool,
    /// Position among the arms, clockwise from north.
    pub order: usize,
    /// Site leg number (None for a node's arms).
    pub k: Option<f64>,
    /// Site leg: arc keys entering.
    pub ins: Vec<String>,
    /// Site leg: arc keys leaving.
    pub outs: Vec<String>,
}

fn nv(x: f64) -> Value {
    if x.is_finite() {
        json!(x)
    } else if x.is_nan() {
        json!("NaN")
    } else if x > 0.0 {
        json!("Infinity")
    } else {
        json!("-Infinity")
    }
}

impl Arm {
    /// The arm as JSON (numbers that JSON cannot hold are "NaN", "Infinity" or "-Infinity"); the code keys are there once coded.
    #[must_use]
    pub fn to_value(&self, coded: bool) -> Value {
        let mut o = serde_json::Map::new();
        o.insert("id".into(), json!(self.id));
        o.insert("linkId".into(), json!(self.link));
        o.insert("conn".into(), json!(self.conn));
        o.insert("road".into(), json!(self.road));
        o.insert("b".into(), nv(self.b));
        if self.k.is_none() {
            o.insert("inKey".into(), json!(self.in_key));
            o.insert("outKey".into(), json!(self.out_key));
        }
        o.insert("isIn".into(), json!(self.is_in));
        o.insert("isOut".into(), json!(self.is_out));
        o.insert("side".into(), nv(self.side));
        o.insert("name".into(), json!(self.name));
        if let Some(k) = self.k {
            o.insert("k".into(), nv(k));
            o.insert("ins".into(), json!(self.ins));
            o.insert("outs".into(), json!(self.outs));
        }
        if coded {
            o.insert("o".into(), json!(self.o));
            o.insert("leg".into(), json!(self.leg));
            o.insert("legName".into(), json!(self.leg_name));
            o.insert("legLabel".into(), json!(self.leg_label));
            if self.k.is_none() {
                o.insert("legMerged".into(), json!(self.leg_merged));
                o.insert("order".into(), json!(self.order));
            }
        }
        Value::Object(o)
    }
}

/// An ordered set of text keys, as a JavaScript `Set` iterates them.
struct KeySet {
    list: Vec<String>,
    seen: HashSet<String>,
}

impl KeySet {
    fn new() -> KeySet {
        KeySet {
            list: Vec::new(),
            seen: HashSet::new(),
        }
    }
    fn add(&mut self, k: &str) {
        if self.seen.insert(k.to_string()) {
            self.list.push(k.to_string());
        }
    }
    fn has(&self, k: &str) -> bool {
        self.seen.contains(k)
    }
}

/// `Turns.arms(sc, nodeId, list, links)`: each road (or zone connector) touching the node, with its outward bearing from the node.
///
/// `list` is the node's turning movements (its arcs make connectors and closed roads count); `links` limits the roads (default
/// every road at the node). Names follow the side of the arm; 8-point names are used when two approaches share a quadrant, the
/// road name is added if that still clashes, and any still-equal names are numbered clockwise. Arms come back clockwise from
/// north, starting half a sector west of north.
#[must_use]
pub fn arms(sc: &Scenario, node_id: &str, list: &[TurnRow], links: Option<&[&Link]>) -> Vec<Arm> {
    let Some(n) = sc.nodes.get(node_id) else {
        return Vec::new();
    };
    let mut used = KeySet::new();
    for t in list {
        used.add(&t.from);
        used.add(&t.to);
    }
    let at_node: Vec<&Link>;
    let ls: &[&Link] = if let Some(l) = links { l } else {
        at_node = crate::edit::net::links_at(sc, node_id);
        &at_node
    };
    let mut out: Vec<Arm> = Vec::new();
    link_arms(sc, node_id, &used, ls, &mut out);
    connector_arms(sc, n, node_id, &used, &mut out);
    let eight = name_arms(&mut out);
    // Clockwise from North, starting half a sector west of North so the North arm comes first
    let off = if eight { 22.5 } else { 45.0 };
    js_sort_by(&mut out, |p, q| norm(p.b + off) - norm(q.b + off));
    out
}

// An arm with its road, bearing and arc keys; the names, codes and order are filled in later.
fn new_arm(id: String, road: String, b: f64, (in_key, out_key): (String, String), (is_in, is_out): (bool, bool)) -> Arm {
    Arm {
        id,
        link: None,
        link_rank: None,
        conn: false,
        road,
        b,
        in_key,
        out_key,
        is_in,
        is_out,
        side: 0.0,
        name: String::new(),
        o: 0,
        leg: "",
        leg_name: "",
        leg_label: String::new(),
        leg_merged: false,
        order: 0,
        k: None,
        ins: Vec::new(),
        outs: Vec::new(),
    }
}

// The road arms of `arms`: each link at the node with both ends there, a closed one only when its arcs carry turns.
fn link_arms(sc: &Scenario, node_id: &str, used: &KeySet, ls: &[&Link], out: &mut Vec<Arm>) {
    for l in ls {
        if l.a == l.b || !sc.nodes.contains_key(&l.a) || !sc.nodes.contains_key(&l.b) {
            continue;
        }
        let at_a = l.a == node_id;
        let (in_key, out_key) = if at_a {
            (format!("{}:ba", l.id), format!("{}:ab", l.id))
        } else {
            (format!("{}:ab", l.id), format!("{}:ba", l.id))
        };
        if l.status.is("closed") && !used.has(&in_key) && !used.has(&out_key) {
            continue;
        }
        let g = sc.geom(l);
        let pts: Vec<LatLng> = if at_a {
            g
        } else {
            g.into_iter().rev().collect()
        };
        let (lin, lout) = if at_a {
            (l.lanes_ba.int32(), l.lanes_ab.int32())
        } else {
            (l.lanes_ab.int32(), l.lanes_ba.int32())
        };
        let is_in = lin > 0 || used.has(&in_key);
        let is_out = lout > 0 || used.has(&out_key);
        out.push(Arm {
            link: Some(l.id.clone()),
            link_rank: Some(road_class(l.cls.as_str()).rank),
            ..new_arm(l.id.clone(), road_name(l), end_b(&pts, false), (in_key, out_key), (is_in, is_out))
        });
    }
}

// The zone connector arms of `arms`: one per zone connector at the node among the turns' arc keys ("c:ZONE:NODE:o").
fn connector_arms(sc: &Scenario, node: &Node, node_id: &str, used: &KeySet, out: &mut Vec<Arm>) {
    let mut seen: HashSet<String> = HashSet::new();
    for key in &used.list {
        if key.is_empty() || !key.starts_with("c:") {
            continue;
        }
        // const [, zid, nid] = k.split(':')
        let mut parts = key.split(':');
        parts.next();
        let zid = parts.next();
        let nid = parts.next();
        let (Some(zid), Some(nid)) = (zid, nid) else {
            continue;
        };
        let id = format!("c:{zid}:{nid}");
        let Some(zone) = sc.zones.get(zid) else {
            continue;
        };
        if nid != node_id || seen.contains(&id) {
            continue;
        }
        seen.insert(id.clone());
        let road = match zone.name.as_str() {
            Some(zname) if !zname.is_empty() => zname.to_string(),
            _ => "Zone".to_string(),
        };
        let flags = (used.has(&format!("{id}:o")), used.has(&format!("{id}:d")));
        out.push(Arm {
            conn: true,
            ..new_arm(
                id.clone(),
                road,
                util::bearing(&node.ll(), &zone.ll()),
                (format!("{id}:o"), format!("{id}:d")),
                flags,
            )
        });
    }
}

// Names: 4-point unless two approaches share a quadrant, then 8-point, then with the road, then numbered clockwise.
// Gives whether 8-point names are used.
fn name_arms(out: &mut [Arm]) -> bool {
    let clash = |out: &[Arm], eight: bool| -> bool {
        let mut sides: HashSet<&str> = HashSet::new();
        out.iter().filter(|arm| arm.is_in).any(|arm| {
            let side = side_name(arm.b, eight);
            sides.contains(side) || !sides.insert(side)
        })
    };
    let eight = clash(&*out, false);
    for arm in out.iter_mut() {
        arm.side = arm.b;
        arm.name = side_name(arm.b, eight).to_string();
    }
    let dupes = |out: &[Arm]| -> HashMap<String, usize> {
        let mut cnt: HashMap<String, usize> = HashMap::new();
        for arm in out.iter().filter(|arm| arm.is_in) {
            let count = cnt.entry(arm.name.clone()).or_insert(0);
            // Bound: at most one per arm
            *count = count.saturating_add(1);
        }
        cnt
    };
    if eight && clash(&*out, true) {
        let cnt = dupes(&*out);
        for arm in out.iter_mut() {
            if cnt.get(&arm.name).copied().unwrap_or(0) > 1 {
                arm.name = format!("{} ({})", arm.name, arm.road);
            }
        }
    }
    // Still the same (two same-named ramps from one side): number them clockwise so every approach is distinct
    let cnt = dupes(&*out);
    let mut seq: HashMap<String, usize> = HashMap::new();
    let mut by_bearing: Vec<usize> = (0..out.len()).collect();
    // every index is one of out's
    let bearing_at = |idx: usize| out.get(idx).map_or(f64::NAN, |arm| norm(arm.b));
    js_sort_by(&mut by_bearing, |&ia, &ib| bearing_at(ia) - bearing_at(ib));
    for idx in by_bearing {
        let Some(arm) = out.get_mut(idx) else { continue };
        if arm.is_in && cnt.get(&arm.name).copied().unwrap_or(0) > 1 {
            let key = arm.name.clone();
            let nth = seq.entry(key).or_insert(0);
            // Bound: at most one per arm
            *nth = nth.saturating_add(1);
            let nth = *nth;
            arm.name = format!("{} {}", arm.name, nth);
        }
    }
    eight
}

/// `codeArms(A)`: puts the SIDRA leg code, leg name, leg label and merge flag on each arm.
pub fn code_arms(a: &mut [Arm]) {
    let pr: Vec<f64> = a
        .iter()
        .map(|x| {
            if x.conn {
                0.0
            } else {
                x.link_rank.unwrap_or(1.0)
            }
        })
        .collect();
    let bs: Vec<f64> = a.iter().map(|x| x.b).collect();
    // one code per arm, in the arms' order
    for (x, c) in a.iter_mut().zip(assign_legs(&bs, &pr)) {
        let (leg, name) = leg_of(c.o);
        x.o = c.o;
        x.leg = leg;
        x.leg_name = name;
        x.leg_label = leg_label(c.o, &x.road);
        x.leg_merged = c.merged;
    }
}

// The leg code letters and leg name of leg code o (assign_legs gives 0 to 7; anything else reads as JavaScript's undefined)
fn leg_of(o: usize) -> (&'static str, &'static str) {
    (
        LEG_CODES.get(o).copied().unwrap_or("undefined"),
        LEG_NAMES.get(o).copied().unwrap_or("undefined"),
    )
}

/// One row of a movement table: a turn from one arm to another with its flow, heavy-vehicle split and SIDRA codes.
#[derive(Clone, Debug, PartialEq)]
pub struct MovementRow {
    /// Arc key arriving.
    pub approach: String,
    /// Approach name.
    pub approach_name: String,
    /// Approach road.
    pub road: String,
    /// Movement: L, T, R or U.
    pub mv: String,
    /// Arc key leaving.
    pub to: String,
    /// Exit road.
    pub to_road: String,
    /// Exit name.
    pub to_name: String,
    /// Flow (veh/h).
    pub v: f64,
    /// Flow (pcu/h).
    pub pcu: f64,
    /// Heavy vehicles (veh/h).
    pub hv: f64,
    /// Light vehicles (veh/h).
    pub lv: f64,
    /// Share of the approach's flow.
    pub share: f64,
    /// The approach's outward bearing.
    pub side: f64,
    /// Leg code letters.
    pub leg: &'static str,
    /// Leg name.
    pub leg_name: &'static str,
    /// Leg label.
    pub leg_label: String,
    /// Exit leg code letters.
    pub to_leg: &'static str,
    /// Turn designation ("L2").
    pub turn: &'static str,
    /// Movement id ("S-L2").
    pub id: String,
    /// Development traffic in the movement.
    pub dev: f64,
    /// Traffic without development.
    pub base: f64,
    /// The junction site the row is for.
    pub site: Option<String>,
}

impl MovementRow {
    /// The row as JSON, keys as the prototype's row object.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut o = serde_json::Map::new();
        o.insert("approach".into(), json!(self.approach));
        o.insert("approachName".into(), json!(self.approach_name));
        o.insert("road".into(), json!(self.road));
        o.insert("mv".into(), json!(self.mv));
        o.insert("to".into(), json!(self.to));
        o.insert("toRoad".into(), json!(self.to_road));
        o.insert("toName".into(), json!(self.to_name));
        for (k, x) in [
            ("v", self.v),
            ("pcu", self.pcu),
            ("hv", self.hv),
            ("lv", self.lv),
            ("share", self.share),
            ("side", self.side),
        ] {
            o.insert(k.into(), nv(x));
        }
        o.insert("leg".into(), json!(self.leg));
        o.insert("legName".into(), json!(self.leg_name));
        o.insert("legLabel".into(), json!(self.leg_label));
        o.insert("toLeg".into(), json!(self.to_leg));
        o.insert("turn".into(), json!(self.turn));
        o.insert("id".into(), json!(self.id));
        o.insert("dev".into(), nv(self.dev));
        o.insert("base".into(), nv(self.base));
        if let Some(s) = &self.site {
            o.insert("site".into(), json!(s));
        }
        Value::Object(o)
    }
}

/// What [`rows_for`] and [`site_rows`] give: the rows and the arms (legs) they use.
#[derive(Clone, Debug, PartialEq)]
pub struct Movements {
    /// The movement rows.
    pub rows: Vec<MovementRow>,
    /// The arms (a node's arms, or a site's legs).
    pub arms: Vec<Arm>,
}

fn pcu_factor(s: &Settings) -> f64 {
    1.0 + s.hv_pct.num() / 100.0 * (s.pcu_hv.num() - 1.0)
}

fn mv_index(mv: &str) -> usize {
    MV.iter().position(|m| *m == mv).unwrap_or(usize::MAX)
}

fn is_mv(mv: &str) -> bool {
    MV.contains(&mv)
}

/// `rowsFor(sc, res, nodeId, links)`: a junction's movements from `res.turns[node]`, arms coded, rows sorted by arm, movement
/// and turn angle. With no turns for the node the arms alone come back (named, not coded).
#[must_use]
pub fn rows_for(
    sc: &Scenario,
    res: Option<&Res>,
    s: &Settings,
    node_id: &str,
    links: Option<&[&Link]>,
) -> Movements {
    let list = res.and_then(|r| r.turns.get(node_id));
    let Some(list) = list.filter(|_| sc.nodes.contains_key(node_id)) else {
        return Movements {
            rows: Vec::new(),
            arms: arms(sc, node_id, &[], links),
        };
    };
    let pf = pcu_factor(s);
    let mut node_arms = arms(sc, node_id, list, links);
    code_arms(&mut node_arms);
    let mut by_in: HashMap<&str, usize> = HashMap::new();
    let mut by_out: HashMap<&str, usize> = HashMap::new();
    for (idx, arm) in node_arms.iter_mut().enumerate() {
        arm.order = idx;
    }
    for (idx, arm) in node_arms.iter().enumerate() {
        by_in.insert(arm.in_key.as_str(), idx);
        by_out.insert(arm.out_key.as_str(), idx);
    }
    let mut in_b: HashMap<String, Option<f64>> = HashMap::new();
    let mut rows: Vec<SortRow> = Vec::new();
    for tr in list {
        let (Some(&ai), Some(&xi)) = (by_in.get(tr.from.as_str()), by_out.get(tr.to.as_str())) else {
            continue; // arc no longer at this node (stale result) or malformed
        };
        let Some(arr_b) = arriving(sc, &node_arms, &by_in, &mut in_b, &tr.from) else {
            continue;
        };
        // both indexes come from the arms just listed
        let (Some(ar), Some(to_arm)) = (node_arms.get(ai), node_arms.get(xi)) else {
            continue;
        };
        rows.push(node_row(tr, ar, to_arm, arr_b, pf, s));
    }
    let mut tot: HashMap<String, f64> = HashMap::new();
    for sr in &rows {
        *tot.entry(sr.row.approach.clone()).or_insert(0.0) += sr.row.v;
    }
    for sr in &mut rows {
        // every row's approach has a total, made just above
        let sum = tot.get(&sr.row.approach).copied().unwrap_or(0.0);
        sr.row.share = if sum > 0.0 { sr.row.v / sum } else { 0.0 };
    }
    js_sort_by(&mut rows, |pa, pb| {
        let d = usize_to_f64(pa.o) - usize_to_f64(pb.o);
        if d != 0.0 {
            return d;
        }
        let d = usize_to_f64(mv_index(&pa.row.mv)) - usize_to_f64(mv_index(&pb.row.mv));
        if d != 0.0 && !d.is_nan() {
            return d;
        }
        pa.t - pb.t
    });
    Movements {
        rows: rows.into_iter().map(|sr| sr.row).collect(),
        arms: node_arms,
    }
}

// A movement row of a node with what it is sorted by: the approach's position and the turn angle.
struct SortRow {
    row: MovementRow,
    o: usize,
    t: f64,
}

// The arrival heading of arc `key` at the node: a connector's from its arm, a road's over the last 20 m of its link;
// None for a link that is gone or has a missing end node. Kept in `cache` by key.
fn arriving(
    sc: &Scenario,
    node_arms: &[Arm],
    by_in: &HashMap<&str, usize>,
    cache: &mut HashMap<String, Option<f64>>,
    key: &str,
) -> Option<f64> {
    if let Some(v) = cache.get(key) {
        return *v;
    }
    let b = if key.starts_with("c:") {
        by_in.get(key).and_then(|&i| node_arms.get(i)).map(|arm| norm(arm.b + 180.0))
    } else {
        let i = key.rfind(':').unwrap_or(0);
        let lid = &key[..i];
        match sc.links.get(lid) {
            Some(l) if sc.nodes.contains_key(&l.a) && sc.nodes.contains_key(&l.b) => {
                let g = sc.geom(l);
                let pts: Vec<LatLng> = if key.ends_with(":ab") {
                    g
                } else {
                    g.into_iter().rev().collect()
                };
                Some(end_b(&pts, true))
            }
            _ => None,
        }
    };
    cache.insert(key.to_string(), b);
    b
}

// One movement row of a node, from approach `ar` (arriving on heading `arr_b`) to exit arm `to_arm`.
fn node_row(t: &TurnRow, ar: &Arm, to_arm: &Arm, arr_b: f64, pf: f64, s: &Settings) -> SortRow {
    let v = math_max(0.0, or_default(t.v, 0.0));
    let pcu = if t.pcu.is_finite() {
        math_max(0.0, t.pcu)
    } else {
        v * pf
    };
    let hv = hv_of(v, Some(pcu), s);
    let turn = util::turn(arr_b, to_arm.b);
    let dev = math_min(v, math_max(0.0, or_default(t.dev.unwrap_or(f64::NAN), 0.0)));
    let mv = if is_mv(&t.mv) {
        t.mv.clone()
    } else {
        classify(arr_b, to_arm.b).to_string()
    };
    SortRow {
        row: MovementRow {
            approach: t.from.clone(),
            approach_name: ar.name.clone(),
            road: ar.road.clone(),
            mv,
            to: t.to.clone(),
            to_road: to_arm.road.clone(),
            to_name: to_arm.name.clone(),
            v,
            pcu,
            hv,
            lv: v - hv,
            share: 0.0,
            side: ar.side,
            leg: ar.leg,
            leg_name: ar.leg_name,
            leg_label: ar.leg_label.clone(),
            to_leg: to_arm.leg,
            turn: turn_code(ar.o, to_arm.o),
            id: movement_id(ar.o, to_arm.o),
            dev,
            base: v - dev,
            site: None,
        },
        o: ar.order,
        t: turn,
    }
}

/// `siteRows(sc, res, siteId)`: the movements of a junction site (`res.sites`, `res.siteTurns` from the engine): the legs are the
/// site's legs (both carriageways of a divided road on one side are one leg) and the movements run from entry to exit through the
/// site. Without such a site both lists are empty.
#[must_use]
pub fn site_rows(sc: &Scenario, res: Option<&Res>, s: &Settings, site_id: &str) -> Movements {
    let Some(site) = res
        .and_then(|r| r.sites.as_ref())
        .and_then(|m| m.get(site_id))
    else {
        return Movements {
            rows: Vec::new(),
            arms: Vec::new(),
        };
    };
    let empty: Vec<TurnRow> = Vec::new();
    let list: &[TurnRow] = res
        .and_then(|r| r.site_turns.as_ref())
        .and_then(|m| m.get(site_id))
        .map_or(&empty[..], |v| &v[..]);
    let pf = pcu_factor(s);
    let legs = site_arms(sc, site);
    let mut in_leg: HashMap<&str, usize> = HashMap::new();
    let mut out_leg: HashMap<&str, usize> = HashMap::new();
    for (idx, leg) in legs.iter().enumerate() {
        for k in &leg.ins {
            in_leg.insert(k.as_str(), idx);
        }
        for k in &leg.outs {
            out_leg.insert(k.as_str(), idx);
        }
    }
    let mut rows: Vec<MovementRow> = Vec::new();
    for tr in list {
        let (Some(&ai), Some(&xi)) = (in_leg.get(tr.from.as_str()), out_leg.get(tr.to.as_str()))
        else {
            continue;
        };
        // both indexes come from the legs just listed
        let (Some(ar), Some(to_arm)) = (legs.get(ai), legs.get(xi)) else {
            continue;
        };
        rows.push(site_row(tr, ar, to_arm, pf, s, site_id));
    }
    let mut tot: HashMap<&'static str, f64> = HashMap::new();
    for row in &rows {
        *tot.entry(row.leg).or_insert(0.0) += row.v;
    }
    for row in &mut rows {
        // every row's leg has a total, made just above
        let sum = tot.get(row.leg).copied().unwrap_or(0.0);
        row.share = if sum > 0.0 { row.v / sum } else { 0.0 };
    }
    let leg_ix = |code: &str| {
        LEG_CODES
            .iter()
            .position(|lc| *lc == code)
            .map_or(-1.0, usize_to_f64)
    };
    js_sort_by(&mut rows, |pa, pb| {
        let d = leg_ix(pa.leg) - leg_ix(pb.leg);
        if d != 0.0 {
            return d;
        }
        usize_to_f64(mv_index(&pa.mv)) - usize_to_f64(mv_index(&pb.mv))
    });
    Movements { rows, arms: legs }
}

// The legs of a junction site as arms, coded: priority by the highest road class among each leg's links.
fn site_arms(sc: &Scenario, site: &SiteRes) -> Vec<Arm> {
    let mut a: Vec<Arm> = site
        .legs
        .iter()
        .map(|l| Arm {
            id: format!("leg{}", crate::jsnum::number_to_js_string(l.k)),
            link: None,
            link_rank: None,
            conn: false,
            road: if l.name.is_empty() {
                "Unnamed road".to_string()
            } else {
                l.name.clone()
            },
            b: l.b,
            in_key: String::new(),
            out_key: String::new(),
            is_in: !l.ins.is_empty(),
            is_out: !l.outs.is_empty(),
            side: 0.0,
            name: String::new(),
            o: 0,
            leg: "",
            leg_name: "",
            leg_label: String::new(),
            leg_merged: false,
            order: 0,
            k: Some(l.k),
            ins: l.ins.clone(),
            outs: l.outs.clone(),
        })
        .collect();
    let pr: Vec<f64> = a
        .iter()
        .map(|x| {
            let mut m = 0.0f64;
            for k in x.ins.iter().chain(x.outs.iter()) {
                let lid = &k[..k.rfind(':').unwrap_or(0)];
                let r = sc
                    .links
                    .get(lid)
                    .map_or(1.0, |l| road_class(l.cls.as_str()).rank);
                m = math_max(m, r);
            }
            m
        })
        .collect();
    let bs: Vec<f64> = a.iter().map(|x| x.b).collect();
    // one code per leg, in the legs' order
    for (x, c) in a.iter_mut().zip(assign_legs(&bs, &pr)) {
        let (leg, name) = leg_of(c.o);
        x.o = c.o;
        x.leg = leg;
        x.leg_name = name;
        x.leg_label = leg_label(c.o, &x.road);
        x.name = name.to_string();
        x.side = x.b;
    }
    a
}

// One movement row of a junction site, from entry leg `ar` to exit leg `to_arm`.
fn site_row(t: &TurnRow, ar: &Arm, to_arm: &Arm, pf: f64, s: &Settings, site_id: &str) -> MovementRow {
    let v = math_max(0.0, or_default(t.v, 0.0));
    let pcu = if t.pcu.is_finite() { t.pcu } else { v * pf };
    let hv = hv_of(v, Some(pcu), s);
    let dev = math_min(v, math_max(0.0, or_default(t.dev.unwrap_or(f64::NAN), 0.0)));
    MovementRow {
        approach: t.from.clone(),
        approach_name: ar.leg_name.to_string(),
        road: ar.road.clone(),
        mv: if is_mv(&t.mv) {
            t.mv.clone()
        } else {
            "T".to_string()
        },
        to: t.to.clone(),
        to_road: to_arm.road.clone(),
        to_name: to_arm.leg_name.to_string(),
        v,
        pcu,
        hv,
        lv: v - hv,
        share: 0.0,
        side: ar.b,
        leg: ar.leg,
        leg_name: ar.leg_name,
        leg_label: ar.leg_label.clone(),
        to_leg: to_arm.leg,
        turn: turn_code(ar.o, to_arm.o),
        id: movement_id(ar.o, to_arm.o),
        dev,
        base: v - dev,
        site: Some(site_id.to_string()),
    }
}

/// `Turns.movements(sc, res, id)`: the movement rows for a node, or for a junction site when the id is a site id in `res.sites`.
#[must_use]
pub fn movements(sc: &Scenario, res: Option<&Res>, s: &Settings, id: &str) -> Vec<MovementRow> {
    let is_site = res
        .and_then(|r| r.sites.as_ref())
        .is_some_and(|m| m.contains_key(id));
    if is_site {
        site_rows(sc, res, s, id).rows
    } else {
        rows_for(sc, res, s, id, None).rows
    }
}

/// `Turns.junctionName(sc, nodeId, links)`: the junction's own name, or the roads that meet there, highest class first
/// ("Great South Road / Waihoehoe Road"); "Unnamed junction" with none, "Unknown junction" for a node that is not there.
#[must_use]
pub fn junction_name(sc: &Scenario, node_id: &str, links: Option<&[&Link]>) -> String {
    let Some(n) = sc.nodes.get(node_id) else {
        return "Unknown junction".to_string();
    };
    if let Some(v) = n.extra.get("name")
        && crate::jsnum::value_truthy(v)
    {
        return crate::jsnum::value_to_js_string(v);
    }
    let at_node: Vec<&Link>;
    let all: &[&Link] = if let Some(l) = links { l } else {
        at_node = crate::edit::net::links_at(sc, node_id);
        &at_node
    };
    let mut ls: Vec<&Link> = all
        .iter()
        .copied()
        .filter(|l| !l.status.is("closed") && l.name.as_str().is_some_and(|n| !n.is_empty()))
        .collect();
    js_sort_by(&mut ls, |p, q| {
        let d = road_class(q.cls.as_str()).rank - road_class(p.cls.as_str()).rank;
        if d != 0.0 && !d.is_nan() {
            d
        } else {
            locale_compare_f64(p.name.as_str().unwrap_or(""), q.name.as_str().unwrap_or(""))
        }
    });
    let mut names: Vec<&str> = Vec::new();
    for l in ls {
        let n = l.name.as_str().unwrap_or("");
        if !names.contains(&n) {
            names.push(n);
        }
    }
    if names.is_empty() {
        "Unnamed junction".to_string()
    } else {
        names.into_iter().take(3).collect::<Vec<_>>().join(" / ")
    }
}

/// `Turns.emptyText(sc, res, nodeId)`: why a junction has nothing to show, and what to do about it.
#[must_use]
pub fn empty_text(sc: Option<&Scenario>, res: Option<&Res>, node_id: &str) -> String {
    let Some(res) = res else {
        return "Run the model to see turning volumes.".to_string();
    };
    if res.stale {
        return "Results are out of date. Run the model again.".to_string();
    }
    if !res.turns.contains_key(node_id) {
        return match sc {
            Some(sc) if crate::edit::net::links_at(sc, node_id).len() < 3 => {
                "Not a junction (fewer than three roads), so no turns.".to_string()
            }
            _ => "No traffic turns here in this period.".to_string(),
        };
    }
    "No traffic turns here in this period.".to_string()
}
