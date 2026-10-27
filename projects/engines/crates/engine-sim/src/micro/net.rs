//! What the simulation is given: the network, the vehicle routes and the run settings.
//!
//! The network is plain numbers: arc lengths and speeds, how each arc ends (free, signals,
//! give way or roundabout entry), which arcs conflict with it, and the signal timings. It holds
//! nothing about roads, maps or any program's data, so any program with a network can run it.

/// How the downstream end of an arc is controlled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Free flow: no junction, a major road, a ring arc or an uncontrolled join.
    Free,
    /// Traffic signals: the arc is in the given phase (0 or 1) of the given timing plan.
    Signal {
        /// Index into [`MicroNet::signals`].
        plan: usize,
        /// Signal phase of the approach (0 for the main road, 1 for the cross road).
        phase: u8,
    },
    /// A minor road giving way (or stopping) to the arcs it conflicts with.
    Minor,
    /// A roundabout entry giving way to the circulating arcs it conflicts with.
    Roundabout,
}

/// The timing of one signal controller: a cycle split into two green phases with lost time between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignalPlan {
    /// Cycle length (s).
    pub cycle: f64,
    /// Lost time in the whole cycle (s), shared equally between the two phases.
    pub lost: f64,
    /// Share of the effective green each phase gets.
    pub split: [f64; 2],
    /// Offset (s) of this controller's cycle from time zero.
    pub offset: f64,
}

/// One arc of the network: a road direction vehicles drive along.
#[derive(Clone, Debug, PartialEq)]
pub struct MicroArc {
    /// Length (m).
    pub len: f64,
    /// Speed limit (km/h).
    pub speed: f64,
    /// Free-flow travel time (s); each vehicle adds up the free-flow times of the arcs it drives.
    pub ff: f64,
    /// General traffic lanes (the lanes less a bus lane).
    pub lanes: usize,
    /// One extra lane at the left edge (lane 0) that only buses may use.
    pub bus_lane: bool,
    /// A virtual arc (such as a zone connector) that no vehicle ever occupies; it has no lanes.
    pub virtual_arc: bool,
    /// Node this arc ends at.
    pub to: usize,
    /// Bearing arriving at the end of the arc (degrees).
    pub in_bearing: f64,
    /// Bearing leaving the start of the arc (degrees).
    pub out_bearing: f64,
    /// How the end of the arc is controlled.
    pub control: Control,
    /// A major road approach at a give-way or stop junction (it has priority).
    pub major: bool,
    /// A stop sign: vehicles must come to a halt before going.
    pub stop: bool,
    /// Arcs that must be clear before a vehicle on this arc may enter the junction.
    pub conflicts: Vec<usize>,
}

/// The network a run drives on.
#[derive(Clone, Debug, PartialEq)]
pub struct MicroNet {
    /// The arcs, in the order vehicles are processed.
    pub arcs: Vec<MicroArc>,
    /// Arcs arriving at each node (counted per node index).
    pub node_in: Vec<usize>,
    /// Arcs leaving each node.
    pub node_out: Vec<usize>,
    /// Signal timings, indexed by [`Control::Signal`].
    pub signals: Vec<SignalPlan>,
}

/// A route vehicles are released on: a list of arcs and how often a vehicle starts.
#[derive(Clone, Debug, PartialEq)]
pub struct MicroRoute {
    /// Arcs in driving order (none of them virtual).
    pub arcs: Vec<usize>,
    /// Vehicles per second (before the demand scale for cars; buses leave on a fixed headway).
    pub rate: f64,
    /// A bus route: one bus every 1 / rate seconds at a random first start, on the bus lane where there is one.
    pub bus: bool,
}

/// The settings of a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MicroParams {
    /// Time step (s).
    pub dt: f64,
    /// Demand scale applied to the arrival rate of car routes.
    pub scale: f64,
    /// Vehicles allowed on the network at once; releases wait while it is reached.
    pub max_vehicles: f64,
    /// Share (0 to 1) of non-bus vehicles that are heavy vehicles.
    pub heavy_share: f64,
}

impl MicroParams {
    /// The usual settings: a 0.5 s time step with the given demand scale, vehicle limit and heavy vehicle share.
    pub fn new(scale: f64, max_vehicles: f64, heavy_share: f64) -> Self {
        MicroParams {
            dt: 0.5,
            scale,
            max_vehicles,
            heavy_share,
        }
    }
}
