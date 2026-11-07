//! Network graph: buildGraph and its helpers from engine.js.
//!
//! Arcs per link direction (lanes, bus lanes, free-flow time, mid-block capacity, entry and
//! exit bearings), junction sites and their internal links, variable speed limits and
//! tolls, the main connected component, zone connectors, the turn table (penalties, bans and
//! movement types), site movement slots and the junction controllers, and the searches on it
//! (shortest-path trees, bus route resolution). Port of engine.js, "graph", "turns" and
//! "shortest paths". The searches run on the shared engine [`engine_sim::network`].
//!
//! The JavaScript graph object `G` is [`Graph`]; its arc objects are [`Arc`] and its junction
//! controllers [`Ctl`]. Arrays keep the prototype's integer types and fill values (-1 for none),
//! objects iterated in the prototype keep its order (nodes, links and zones in file order), and
//! every sort is the JavaScript engine's ([`js_sort_by`]).
//!
//! Coordinates are read as numbers (`Number()` of each); a node or zone saved with text
//! coordinates would make the prototype concatenate text in a few sums, which is not reproduced.

mod build;
mod controls;
mod search;
mod sites;
mod sort;
mod turns;
mod zones;

pub use build::{build_graph, speed_of, toll_of};
pub use controls::{apply_signal, hash_offset, site_control, site_legs};
pub(crate) use sites::map_set as sites_map_set;
pub use sites::{SiteMoveList, SiteMoves, site_flows, site_index, traced_exits};
pub use sort::{js_sort_by, js_str_lt};
pub use turns::{
    MVC, TURN_DEF_LEFT, TURN_DEF_RIGHT, TURN_DEF_RIGHT_SIGNALS, TurnSet, approach_name, turn_set,
    turn_type, u_pen,
};
pub use zones::{connector_map, main_component, zc_idx, zone_class};

use crate::jsnum::Num;
use crate::junction::{EntryRes, MajorState, MoveEntry};
use crate::project::{Node, OrderedMap, Settings, Site, Zone};
use crate::standards::CapMethod;
use crate::util::LatLng;
use engine_sim::EdgeGraph;
use serde_json::Value;
use std::collections::HashMap;

/// How the downstream end of an arc is controlled (`a.ctrl`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ctrl {
    /// Free flow (no junction, a major road, a ring arc or a zone connector).
    Free,
    /// Traffic signals.
    Sig,
    /// A roundabout entry.
    Rbt,
    /// A minor road at a give-way or stop junction.
    Minor,
}

impl Ctrl {
    /// "free", "sig", "rbt" or "minor".
    pub fn as_str(self) -> &'static str {
        match self {
            Ctrl::Free => "free",
            Ctrl::Sig => "sig",
            Ctrl::Rbt => "rbt",
            Ctrl::Minor => "minor",
        }
    }
}

/// One arc of the graph: a link direction or a zone connector (the prototype's arc object).
///
/// Connector arcs have no link: `link` and `name` are empty, `dir` is 0, `cls` is None, `speed`
/// and `lw` are NaN and `cum` is empty (all undefined in the prototype).
#[derive(Clone, Debug, PartialEq)]
pub struct Arc {
    /// Index in `Graph::arcs`.
    pub i: usize,
    /// Key: "<link id>:ab" or ":ba", or "c:<zone id>:<node id>:o" or ":d" for a connector.
    pub key: String,
    /// Link id.
    pub link: String,
    /// 1 from a to b, -1 from b to a, 0 for a connector.
    pub dir: i8,
    /// A zone connector.
    pub conn: bool,
    /// Start node index.
    pub from: usize,
    /// End node index.
    pub to: usize,
    /// Length (m, at least 1; connectors at least 10).
    pub len: f64,
    /// Lanes.
    pub lanes: i32,
    /// General traffic lanes (lanes less a bus lane).
    pub gl: i32,
    /// One lane is a bus lane.
    pub bus: bool,
    /// The link has lanes both ways.
    pub two: bool,
    /// Free-flow time (s).
    pub ff: f64,
    /// Mid-block capacity (pcu/h; infinite for connectors).
    pub cap_mid: f64,
    /// Road class key as saved.
    pub cls: Option<String>,
    /// Class rank (6 motorway to 2 local; 0 for connectors).
    pub rank: f64,
    /// Speed (km/h).
    pub speed: f64,
    /// Lane width (m).
    pub lw: f64,
    /// Part of a roundabout ring.
    pub ring: bool,
    /// Road name.
    pub name: String,
    /// Shape points in the direction of travel.
    pub pts: Vec<LatLng>,
    /// Cumulative length at each shape point.
    pub cum: Vec<f64>,
    /// Bearing arriving at the end node.
    pub in_b: f64,
    /// Bearing leaving the start node.
    pub out_b: f64,
    /// Control at the end node.
    pub ctrl: Ctrl,
    /// Both ends in one junction site (free-flow time only).
    pub internal: bool,
    /// Time-of-use charge (s; 0 for none).
    pub toll: f64,
    /// Zone of a connector.
    pub zone: Option<String>,
    /// Node of the junction controller it enters.
    pub node: Option<usize>,
    /// Signal phase (0 or 1).
    pub phase: Option<u8>,
    /// Signal cycle (s).
    pub c: Option<f64>,
    /// Effective green ratio.
    pub g_c: Option<f64>,
    /// A major road approach at a give-way or stop junction.
    pub major: bool,
    /// A stop-controlled minor approach.
    pub stop: bool,
    /// Conflicting arcs and their weights (minor and roundabout entries).
    pub conf: Option<Vec<(usize, f64)>>,
    /// Ring arcs arriving at a mapped roundabout node.
    pub ring_in: Option<Vec<usize>>,
    /// Ring arcs leaving a mapped roundabout node.
    pub ring_out: Option<Vec<usize>>,
    /// Gap-acceptance capacity (pcu/h) from the last update (None until the first).
    pub cap_n: Option<f64>,
    /// Major road right-turn state (movement method).
    pub mj: Option<MajorState>,
    /// Movement detail from the last update (movement method).
    pub mv_res: Option<EntryRes>,
    /// Source of the lane model (movement method).
    pub lm_src: Option<Value>,
}

/// Signal timing of a controller.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalTiming {
    /// Every approach in one phase.
    pub single: bool,
    /// Cycle (s).
    pub c: f64,
    /// Lost time (s).
    pub lost: f64,
    /// Green split [phase 0, phase 1].
    pub split: Vec<f64>,
    /// Offset (s) from the controller key.
    pub offset: i32,
}

/// The node or site a controller was made from (`ctl.src`), with the fields the assignment reads.
#[derive(Clone, Debug, PartialEq)]
pub struct CtlSrc {
    /// A site (otherwise a node).
    pub site: bool,
    /// Its id.
    pub id: String,
    /// Its own signal cycle.
    pub cycle: Num,
    /// Keep its timing as set.
    pub fixed_timing: Num,
}

/// One leg of a junction site: arcs entering and leaving on one side.
#[derive(Clone, Debug, PartialEq)]
pub struct Leg {
    /// Index.
    pub k: usize,
    /// Bearing outward from the site (whole degrees).
    pub b: f64,
    /// Most common road name.
    pub name: String,
    /// Highest class rank.
    pub rank: f64,
    /// Arcs entering the site.
    pub ins: Vec<usize>,
    /// Arcs leaving the site.
    pub outs: Vec<usize>,
}

/// The site part of a site's one controller (signals, give-way or stop over several nodes).
#[derive(Clone, Debug, PartialEq)]
pub struct SiteCtl {
    /// Site id.
    pub site: String,
    /// Member node indices.
    pub members: Vec<usize>,
    /// Legs.
    pub legs: Vec<Leg>,
    /// Arcs leaving the site.
    pub exits: Vec<usize>,
    /// Entry arc to leg.
    pub leg_in: HashMap<usize, usize>,
    /// Exit arc to leg.
    pub leg_out: HashMap<usize, usize>,
}

/// A junction controller (the prototype's ctl object), keyed by node.
#[derive(Clone, Debug, PartialEq)]
pub struct Ctl {
    /// Control: "signals", "roundabout", "priority", "stop" (or another saved control text, treated as give way).
    pub kind: String,
    /// Node (a site's representative node).
    pub n: usize,
    /// Entry arcs.
    pub arcs: Vec<usize>,
    /// The node or site it was made from.
    pub src: CtlSrc,
    /// Signal timing (signals).
    pub signal: Option<SignalTiming>,
    /// Major road arcs (give way and stop).
    pub major: Option<Vec<usize>>,
    /// The site, for a site's one controller.
    pub site: Option<SiteCtl>,
    /// Movement-method entries (None for signals and with the arc method).
    pub ent: Option<Vec<MoveEntry>>,
}

/// A junction site in the graph: the saved site and its member node indices.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphSite {
    /// The saved site.
    pub site: Site,
    /// Member node indices (in the site's order).
    pub members: Vec<usize>,
}

/// The network graph of one scenario and period (the prototype's `G`).
#[derive(Clone, Debug)]
pub struct Graph {
    /// Node ids: network nodes in file order, then "z:<zone id>" per zone.
    pub ids: Vec<String>,
    /// Node index by id.
    pub idx: HashMap<String, usize>,
    /// Node positions.
    pub ll: Vec<LatLng>,
    /// Arcs: link directions in link order, then connectors.
    pub arcs: Vec<Arc>,
    /// Number of nodes.
    pub n: usize,
    /// Number of arcs.
    pub m: usize,
    /// The search structure: rows (`head`, `out`, `to`), turn table (`tOff`, `tpen`, `tTo`) and zone centroids (`isC`).
    pub search: EdgeGraph,
    /// Free-flow time of each arc.
    pub ffc: Vec<f64>,
    /// Link arcs arriving at each node.
    pub into: Vec<Vec<usize>>,
    /// Link arcs leaving each node.
    pub out_r: Vec<Vec<usize>>,
    /// Zones and gateways with finite positions, in file order.
    pub zones: Vec<Zone>,
    /// Centroid node of each zone.
    pub zone_node: Vec<usize>,
    /// Arc index by key.
    pub key_idx: HashMap<String, usize>,
    /// Connector target node ids by zone id.
    pub conns: OrderedMap<Vec<String>>,
    /// Junction controllers in the order they were made.
    pub ctl: Vec<Ctl>,
    /// Controller index at each node (-1 for none).
    pub ctl_at: Vec<i32>,
    /// The saved node behind each node index (None for centroids).
    pub nodes: Vec<Option<Node>>,
    /// Links meeting at each node (connectors not counted).
    pub deg: Vec<i32>,
    /// Turn penalty settings.
    pub turn_s: TurnSet,
    /// Site id of each node in a site.
    pub site_of: Vec<Option<String>>,
    /// Junction sites with their members.
    pub sites: Vec<GraphSite>,
    /// Capacity method.
    pub cap_method: CapMethod,
    /// The settings the graph was built with.
    pub settings: Settings,
    /// Period ("AM", "PM") or None.
    pub period: Option<String>,
    /// Movement of each turn (index into [`MVC`]: 1 L, 2 T, 3 R, 4 U).
    pub tmv: Vec<u8>,
    /// Number of turns.
    pub n_t: usize,
    /// Site movement slots (None without sites).
    pub sm: Option<SiteMoves>,
    /// Traced site movement flows (pcu/h by slot) the movement method reads (`G.smX`).
    pub sm_x: Option<Vec<f64>>,
    /// Build the lane rows of the movement detail (`G.mvDetail !== false`).
    pub mv_detail: bool,
}

impl Graph {
    /// Zone centroid flags (`G.isC`).
    pub fn is_c(&self) -> &[u8] {
        &self.search.stop
    }
    /// Row offsets (`G.head`).
    pub fn head(&self) -> &[i32] {
        &self.search.net.head
    }
    /// Arcs grouped by start node (`G.out`).
    pub fn out(&self) -> &[i32] {
        &self.search.net.out
    }
    /// End node of each arc (`G.to`).
    pub fn to(&self) -> &[i32] {
        &self.search.net.to
    }
    /// Turn offsets per arc (`G.tOff`).
    pub fn t_off(&self) -> &[i32] {
        &self.search.turns.off
    }
    /// Turn penalties, infinity for banned (`G.tpen`).
    pub fn tpen(&self) -> &[f64] {
        &self.search.turns.pen
    }
    /// Arc each turn enters (`G.tTo`).
    pub fn t_to(&self) -> &[i32] {
        &self.search.turns.to
    }
    /// The controller at node n (`G.ctl.get(n)`).
    pub fn ctl_of(&self, n: usize) -> Option<&Ctl> {
        match self.ctl_at.get(n) {
            Some(&k) if k >= 0 => self.ctl.get(k as usize),
            _ => None,
        }
    }
}
