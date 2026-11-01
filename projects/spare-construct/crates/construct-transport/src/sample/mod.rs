//! The Drury sample project built from downloaded data (a port of the prototype's sample.js).
//!
//! A complete, honestly labelled screening project for Drury, south Auckland: real streets, counts, crashes and existing land use, with an
//! illustrative private plan change and two illustrative network options, set up as an assessment set (S0 2026 base year, S1 2036 do-minimum,
//! S2 2036 with the proposal, S3 2036 with mitigation). [`build`] makes the project; [`analyse`] runs the model, matrix estimation and the tests the
//! report reads. The downloads go through the host's [`crate::census::Net`]; the steps that belong to the feeds, junction tools, compliance and
//! report modules are called through [`SampleHooks`] (a module that is not there is left out and said so in the build notes, as the prototype does).
//!
//! Not ported: the outer time limits (`within`, 150 s for the roads and 180 s for the buildings), which need a clock and are the host's job (a download
//! that runs out of time comes back from the host as a [`crate::census::Failure::Timeout`]); and the menu entry `Sample.drury`, which only draws a busy
//! line, opens the project and says in a toast what loaded (the notes and the failed list it reads are returned by [`build`]).

mod analyse;
mod build;
mod helpers;
mod hooks;

pub use analyse::*;
pub use build::*;
pub use helpers::*;
pub use hooks::*;

use crate::census::Bx;
use crate::jsnum::json_number;
use serde_json::{Value, json};

/// The study box as `[[south, west], [north, east]]`.
pub const BOUNDS: [[f64; 2]; 2] = [[-37.1175, 174.9355], [-37.0885, 174.9850]];
/// What every made-up turning count is labelled.
pub const ILLUSTRATIVE: &str = "ILLUSTRATIVE \u{2014} not a surveyed count";
/// The lodgement date of the sample plan change.
pub const LODGEMENT: &str = "2026-11-16";
/// The base year of the sample.
pub const BASE_YEAR: f64 = 2026.0;
/// The design year of the sample.
pub const DESIGN_YEAR: f64 = 2036.0;
/// Background growth (% of the base year volume a year).
pub const GROWTH: f64 = 1.5;
/// The justification written for the TMDG model category.
pub const JUSTIFY: &str = "Screening-level corridor assessment for a plan change ITA (sample)";
/// The source written on every made-up quantity.
pub const SAMPLE_SRC: &str = "Spare Construct sample (illustrative): made up for the Drury sample, not from a survey, consent or precinct provision";
/// The outer time limit for the road download (ms); the host enforces it.
pub const LIMIT_ROADS_MS: f64 = 150000.0;
/// The outer time limit for the building download (ms); the host enforces it.
pub const LIMIT_BUILDINGS_MS: f64 = 180000.0;
/// The peaks the sample assesses.
pub const PERIODS: [&str; 2] = ["AM", "PM"];
/// The most characters the saved project should have (3 MB).
pub const MAX_SIZE: f64 = 3e6;
/// The build-out steps of the report's staging test (% of the proposal built).
pub const STAGING_STEPS: [f64; 3] = [0.0, 50.0, 100.0];
/// The report's sensitivity tests.
pub const SENS_TESTS: [&str; 2] = ["T1+", "T4+"];
/// Existing land-use zones sit on a grid this many metres wide.
pub const ZONE_CELL: f64 = 700.0;
/// Highways left out of the sample network (residential and living streets).
pub const DROP_HIGHWAY: [&str; 2] = ["residential", "living_street"];

/// The modelled area: the study box and a 25% margin. Roads are cut at its edge and counts and crashes are downloaded for it.
pub fn area() -> Bx {
    let (a, b) = (BOUNDS[0], BOUNDS[1]);
    let h = (b[0] - a[0]) * 0.25;
    let w = (b[1] - a[1]) * 0.25;
    Bx::new(a[0] - h, a[1] - w, b[0] + h, b[1] + w)
}

/// The modelled area as JSON, `{ s, w, n, e }`.
pub fn area_value() -> Value {
    let a = area();
    json!({ "s": json_number(a.s), "w": json_number(a.w), "n": json_number(a.n), "e": json_number(a.e) })
}

/// A land-use quantity of the committed growth or the proposal.
pub type Quantity = (&'static str, f64);

/// Committed growth (in S1 to S3): a precinct with its illustrative yield, opening year and build-out share.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Committed {
    /// Short key.
    pub key: &'static str,
    /// The register row id.
    pub reg: &'static str,
    /// The precinct ("I451").
    pub precinct: &'static str,
    /// The zone name.
    pub name: &'static str,
    /// Where the zone is, `[lat, lng]`.
    pub ll: [f64; 2],
    /// Land use quantities, in the order the zone lists them.
    pub lu: &'static [Quantity],
    /// First homes year.
    pub year: f64,
    /// Share built by the design year (%).
    pub buildout: f64,
}

/// Committed growth of the two precincts.
pub const COMMITTED: [Committed; 2] = [
    Committed {
        key: "i451",
        reg: "sample:i451",
        precinct: "I451",
        name: "Drury East (I451) \u{b7} committed growth (illustrative yield)",
        ll: [-37.0962, 174.9675],
        lu: &[("residential", 420.0), ("apartment", 80.0)],
        year: 2027.0,
        buildout: 70.0,
    },
    Committed {
        key: "i450",
        reg: "sample:i450",
        precinct: "I450",
        name: "Drury Centre (I450) \u{b7} committed growth (illustrative yield)",
        ll: [-37.1008, 174.9612],
        lu: &[("apartment", 200.0), ("retail", 6000.0), ("office", 5000.0)],
        year: 2029.0,
        buildout: 60.0,
    },
];

/// A proposed development zone (S2 and S3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Proposal {
    /// Short key.
    pub key: &'static str,
    /// The zone name.
    pub name: &'static str,
    /// Where the zone is, `[lat, lng]`.
    pub ll: [f64; 2],
    /// Land use quantities, in the order the zone lists them.
    pub lu: &'static [Quantity],
    /// Opening year.
    pub year: f64,
    /// Carries the full trip-generation schedule.
    pub tg: bool,
}

/// The proposal: the plan change area and the business area.
pub const PROPOSAL: [Proposal; 2] = [
    Proposal {
        key: "pc",
        name: "Appleby Road plan change area (illustrative)",
        ll: [-37.1012, 174.9775],
        lu: &[
            ("residential", 320.0),
            ("apartment", 120.0),
            ("retail", 1500.0),
            ("childcare", 60.0),
        ],
        year: 2030.0,
        tg: true,
    },
    Proposal {
        key: "biz",
        name: "Fitzgerald Road business area (illustrative)",
        ll: [-37.1088, 174.9640],
        lu: &[("industrial", 20000.0), ("office", 1500.0)],
        year: 2031.0,
        tg: false,
    },
];

/// The full trip-generation schedule on the plan change area: person-trip rates with a mode split for the homes and the childcare centre, vehicle rates
/// for the shops, opening hours, directional splits, internal and pass-by shares. Every rate is a sample value with no survey behind it.
pub fn tg() -> Value {
    let src = "Illustrative sample rate (not surveyed): replace with RTA, TDB or RR 453 rates";
    json!({
        "on": true,
        "rows": {
            "residential": { "basis": "person", "person": { "AM": 1.6, "PM": 1.8 }, "inPct": { "AM": 22, "PM": 63 }, "hours": "24 h", "source": src },
            "apartment": { "basis": "person", "person": { "AM": 0.9, "PM": 1.0 }, "inPct": { "AM": 20, "PM": 60 }, "hours": "24 h", "source": src },
            "retail": { "basis": "vehicle", "veh": { "AM": 1.5, "PM": 4.5 }, "inPct": { "AM": 60, "PM": 50 }, "freight": { "AM": 0.05, "PM": 0.05 }, "hvPct": 8,
                "hours": "7am\u{2013}9pm", "source": "Illustrative sample rate (not surveyed): Spare Construct default retail rate" },
            "childcare": { "basis": "person", "person": { "AM": 1.2, "PM": 1.1 }, "inPct": { "AM": 55, "PM": 45 }, "hours": "7am\u{2013}6pm",
                "source": "Illustrative sample rate (not surveyed): replace with a surveyed childcare rate" },
        },
        "split": { "driver": 72, "passenger": 9, "pt": 9, "walk": 7, "cycle": 3 },
        "internal": 5, "passby": 10,
    })
}

/// Illustrative turning-count volumes (veh/h in the peak hour) by approach (where traffic comes FROM) and movement, as `[approach, [[movement, veh/h]]]` in the
/// prototype's order.
pub type TurnVolumes = [(&'static str, [(&'static str, f64); 3]); 4];

/// The turning-count volumes of a peak (`TURN_VOL[period]`).
pub fn turn_vol(period: &str) -> TurnVolumes {
    if period == "AM" {
        [
            ("N", [("L", 180.0), ("T", 420.0), ("R", 60.0)]),
            ("S", [("L", 50.0), ("T", 520.0), ("R", 260.0)]),
            ("E", [("L", 300.0), ("T", 40.0), ("R", 380.0)]),
            ("W", [("L", 70.0), ("T", 30.0), ("R", 60.0)]),
        ]
    } else {
        [
            ("N", [("L", 290.0), ("T", 560.0), ("R", 80.0)]),
            ("S", [("L", 70.0), ("T", 430.0), ("R", 210.0)]),
            ("E", [("L", 200.0), ("T", 50.0), ("R", 260.0)]),
            ("W", [("L", 60.0), ("T", 40.0), ("R", 90.0)]),
        ]
    }
}

/// The share of the peak-hour volume in each 15 minutes; bins 3 to 6 make the peak hour.
pub const BIN: [f64; 8] = [0.17, 0.2, 0.23, 0.26, 0.27, 0.24, 0.2, 0.18];
/// The first minute of the day of each peak's turning count (07:00 and 16:00).
pub fn window(period: &str) -> f64 {
    if period == "AM" {
        7.0 * 60.0
    } else {
        16.0 * 60.0
    }
}

/// Why a step stopped: the user cancelled, or it failed with a message.
#[derive(Clone, Debug, PartialEq)]
pub enum SampleError {
    /// The build was cancelled (the prototype's AbortError, "Sample cancelled").
    Cancelled,
    /// The step failed. `stage` is "roads" when the roads could not be downloaded (the prototype's `sampleStage`).
    Failed {
        /// What went wrong, in words.
        message: String,
        /// The stage that failed, when it is one the caller treats specially.
        stage: Option<String>,
    },
}

impl SampleError {
    /// A failure with a message and no stage.
    pub fn msg(message: impl Into<String>) -> SampleError {
        SampleError::Failed {
            message: message.into(),
            stage: None,
        }
    }
    /// The error's message (`e.message`).
    pub fn message(&self) -> String {
        match self {
            SampleError::Cancelled => "Sample cancelled".to_string(),
            SampleError::Failed { message, .. } => message.clone(),
        }
    }
    /// True for a cancel (`isAbort(e)`).
    pub fn is_cancel(&self) -> bool {
        matches!(self, SampleError::Cancelled)
    }
}

impl std::fmt::Display for SampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for SampleError {}

impl From<String> for SampleError {
    fn from(message: String) -> Self {
        SampleError::msg(message)
    }
}

/// A source that did not answer: left out of the project, named here and in the scenario notes.
#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
    /// Which source ("nzta", "census", "gtfs").
    pub id: String,
    /// What it is called.
    pub label: String,
    /// Why it is missing.
    pub why: String,
}

impl Failed {
    /// The record as JSON, `{ id, label, why }`.
    pub fn to_value(&self) -> Value {
        json!({ "id": self.id, "label": self.label, "why": self.why })
    }
}

/// Which optional modules the page has loaded (the prototype's `MODS()`): a step whose module is not loaded is left out and said so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Modules {
    /// The land use feature (Census, employees, schools, zoning, trip-generation tables).
    pub lp: bool,
    /// The calibration feature (count roles, turning counts, matrix estimation, validation).
    pub cal: bool,
    /// The calibration engine (`Calibrate`); matrix estimation needs it as well as the feature.
    pub cb: bool,
    /// The junction tools (lane coding).
    pub jn: bool,
    /// The compliance feature.
    pub co: bool,
    /// The compliance extras (precinct, mitigation register).
    pub cx: bool,
    /// The data feeds (speed limits, TMS, flood hazards, bus timetables).
    pub fe: bool,
    /// Existing land use from OpenStreetMap buildings.
    pub elu: bool,
    /// The report (staging and sensitivity tests).
    pub rp: bool,
}

impl Default for Modules {
    /// Everything loaded.
    fn default() -> Self {
        Modules {
            lp: true,
            cal: true,
            cb: true,
            jn: true,
            co: true,
            cx: true,
            fe: true,
            elu: true,
            rp: true,
        }
    }
}

impl Modules {
    /// Nothing loaded.
    pub fn none() -> Modules {
        Modules {
            lp: false,
            cal: false,
            cb: false,
            jn: false,
            co: false,
            cx: false,
            fe: false,
            elu: false,
            rp: false,
        }
    }
}
