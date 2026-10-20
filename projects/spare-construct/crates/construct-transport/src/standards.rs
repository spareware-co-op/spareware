//! Standards as data: the STANDARDS table of engine.js.
//!
//! Every threshold, factor and band the engine uses lives here with its source, edition,
//! link and a verified flag (cross-cutting rule 4): level-of-service bands, practical
//! degrees of saturation, peak flow factor, back of queue, convergence, signal to noise,
//! MBCM discounting and update factors, IPM efficiency bands, bus PCU, gap acceptance,
//! roundabout lane capacities, the extent test, difference plots, TMDG, MSM and design
//! years. The truth tests compare it with expected/<name>.json "standards".
//! Port of engine.js, Engine.STANDARDS, with the constants derived from it (LOS_SIG,
//! LOS_UNSIG, LOS_VC, losOf, losLt) and the settings readers that fall back on it
//! (junctionSet, capMethodOf, convSet), together with the other small settings readers of
//! engine.js's opening section (pcuF, yearOf, growth, growthBasis, ef).
//! verified: true means checked against the cited document; false means taken from Spare
//! Construct's research spec and still to be checked against the source.

use crate::jsnum::{Maybe, json_number, math_max, math_round, plain_number, to_fixed};
use crate::project::{Scenario, Settings};
use crate::util::clamp;
use serde_json::{Map, Value, json};

/* ---------------- the table, typed ---------------- */

/// Where a standard comes from: id, name, edition, link (when there is one) and whether it was checked against the source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    /// Identifier ("hcm6").
    pub id: &'static str,
    /// Name.
    pub name: &'static str,
    /// Edition or basis.
    pub edition: &'static str,
    /// Link to the document.
    pub url: Option<&'static str>,
    /// Checked against the cited document.
    pub verified: bool,
}

/// A level-of-service method: delay band upper limits (s) for A to E by junction type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LosMethod {
    /// Source.
    pub src: Source,
    /// Signals: A to E upper limits (s).
    pub signals: [f64; 5],
    /// Roundabouts: A to E upper limits (s).
    pub roundabout: [f64; 5],
    /// Give-way and stop: A to E upper limits (s).
    pub sign: [f64; 5],
    /// An approach over capacity (DoS > 1) is F.
    pub over_capacity_f: bool,
    /// Text shown with the results.
    pub label: &'static str,
}

/// The level-of-service methods (`STANDARDS.los`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LosTable {
    /// HCM 6 control delay (the default).
    pub hcm6: LosMethod,
    /// SIDRA Delay method.
    pub sidra: LosMethod,
    /// RTA NSW delay bands (option only; "less than" bands).
    pub rta: LosMethod,
}

impl LosTable {
    /// `STANDARDS.los[id]`: the method with this id, if any.
    pub fn get(&self, id: &str) -> Option<&LosMethod> {
        match id {
            "hcm6" => Some(&self.hcm6),
            "sidra" => Some(&self.sidra),
            "rta" => Some(&self.rta),
            _ => None,
        }
    }
}

/// Link volume/capacity bands for map colouring (`linkVC`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkVc {
    /// Source.
    pub src: Source,
    /// A to E upper limits of V/C.
    pub cuts: [f64; 5],
}

/// Practical degree of saturation by junction type (`practicalDoS`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PracticalDos {
    /// Source.
    pub src: Source,
    /// Signals.
    pub signals: f64,
    /// Roundabouts.
    pub roundabout: f64,
    /// Give-way and stop.
    pub sign: f64,
}

/// Peak flow factor and analysis period (`pff`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pff {
    /// Source.
    pub src: Source,
    /// Peak flow factor.
    pub value: f64,
    /// Analysis period (h).
    pub t: f64,
}

/// Back of queue (`queue`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Queue {
    /// Source.
    pub src: Source,
    /// 95th-percentile factor.
    pub f95: f64,
    /// Akcelik kB coefficient.
    pub k_b: f64,
    /// Queue space per vehicle (m).
    pub space: f64,
    /// Note.
    pub note: &'static str,
}

/// Assignment convergence (`conv`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvStandard {
    /// Source.
    pub src: Source,
    /// Relative gap target.
    pub gap: f64,
    /// Flow change counted as stable (fraction).
    pub chg: f64,
    /// Share of links that must be stable.
    pub share: f64,
    /// Consecutive stable iterations.
    pub consec: f64,
    /// Name of the gap measure.
    pub gap_label: &'static str,
    /// Note.
    pub note: &'static str,
}

/// Scenario signal to noise (`noise`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    /// Source.
    pub src: Source,
    /// Minimum signal to noise ratio.
    pub min: f64,
    /// Tighter convergence suggested: gap target.
    pub tighter_gap_tol: f64,
    /// Tighter convergence suggested: iterations.
    pub tighter_max_iter: f64,
}

/// Gap needed for turn volume exports (`sidraGap`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidraGap {
    /// Source.
    pub src: Source,
    /// Largest relative gap.
    pub max: f64,
}

/// One step of a discount schedule: rate r (%) up to year `to` (None = onwards).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiscountStep {
    /// Last year of the step; None for onwards.
    pub to: Option<f64>,
    /// Rate (%).
    pub r: f64,
}

/// A source document kept in the repository.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocSource {
    /// Source.
    pub src: Source,
    /// Path of the copy in the repository.
    pub file: &'static str,
}

/// NZTA Monetised Benefits and Costs Manual conventions (`mbcm`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mbcm {
    /// Source.
    pub src: Source,
    /// Non-commercial discount schedule.
    pub rates: &'static [DiscountStep],
    /// Sensitivity discount rate (%).
    pub sens_rate: f64,
    /// Analysis period (years).
    pub years: f64,
    /// Longest period without NZTA approval (years).
    pub long_years: f64,
    /// Time zero.
    pub time_zero: &'static str,
    /// Commercial activities: discount rate (%).
    pub commercial_rate: f64,
    /// Commercial activities: sensitivity rate (%).
    pub commercial_sens_rate: f64,
    /// The circular that sets the discount rates.
    pub discount: DocSource,
    /// Label of the result.
    pub label: &'static str,
    /// Disclaimer.
    pub disclaimer: &'static str,
    /// Year from which 95th-percentile values apply.
    pub p95_from: f64,
}

/// MBCM update factors to July 2025 values (`mbcmUpdate`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MbcmUpdate {
    /// Source.
    pub src: Source,
    /// Travel time.
    pub travel_time: f64,
    /// Public transport.
    pub pt: f64,
    /// Walking and cycling.
    pub walk_cycle: f64,
    /// Crashes.
    pub crash: f64,
    /// Emissions.
    pub emissions: f64,
    /// Vehicle operating costs.
    pub voc: f64,
    /// Behaviour change.
    pub behaviour_change: f64,
}

/// One efficiency band: ratios below `below` take it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IpmBand {
    /// Band code ("VL").
    pub band: &'static str,
    /// Band name.
    pub label: &'static str,
    /// Upper limit (exclusive); infinity for the top band.
    pub below: f64,
}

/// An Investment Prioritisation Method edition (`ipm[]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IpmEdition {
    /// Source.
    pub src: Source,
    /// A draft edition.
    pub draft: bool,
    /// Note (drafts).
    pub note: Option<&'static str>,
    /// Bands, lowest first.
    pub bands: &'static [IpmBand],
}

/// Passenger car units per bus (`bus`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bus {
    /// Source.
    pub src: Source,
    /// PCU per bus.
    pub pcu: f64,
    /// PCU per bus in the MSM preset.
    pub msm: f64,
    /// The engine's value before the setting existed.
    pub legacy: f64,
}

/// Critical gap tc and follow-up headway tf (s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gap {
    /// Critical gap (s).
    pub tc: f64,
    /// Follow-up headway (s).
    pub tf: f64,
}

/// Movement gap acceptance (`gaps`): HCM 6 base values mirrored for left-hand traffic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gaps {
    /// Source.
    pub src: Source,
    /// Major road right turn.
    pub major_r: Gap,
    /// Minor road left turn.
    pub minor_l: Gap,
    /// Minor road through.
    pub minor_t: Gap,
    /// Minor road right turn.
    pub minor_r: Gap,
    /// Reduction of tc in each stage of a two-stage crossing (s).
    pub stage: f64,
    /// Vehicles of storage in the median.
    pub storage: f64,
    /// Label.
    pub label: &'static str,
}

impl Gaps {
    /// The gap pair by key ("majorR", "minorL", "minorT", "minorR").
    pub fn get(&self, key: &str) -> Option<Gap> {
        match key {
            "majorR" => Some(self.major_r),
            "minorL" => Some(self.minor_l),
            "minorT" => Some(self.minor_t),
            "minorR" => Some(self.minor_r),
            _ => None,
        }
    }
}

/// A roundabout lane equation c = A e^(-B vc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneEq {
    /// A (pcu/h).
    pub a: f64,
    /// B (per pcu/h).
    pub b: f64,
}

/// HCM 6 roundabout entry lane capacities (`rbtLanes`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RbtLanes {
    /// Source.
    pub src: Source,
    /// One entry lane, one circulating lane.
    pub one: LaneEq,
    /// One entry lane, two circulating lanes.
    pub one_of2: LaneEq,
    /// Two entry lanes, one circulating lane.
    pub two_of1: LaneEq,
    /// Two entry lanes, two circulating: left lane.
    pub left2: LaneEq,
    /// Two entry lanes, two circulating: right lane.
    pub right2: LaneEq,
    /// Label.
    pub label: &'static str,
}

/// A JavaScript regular expression of the table: its pattern and flags (the JSON form is `{}`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pattern {
    /// Pattern text.
    pub source: &'static str,
    /// Flags ("i").
    pub flags: &'static str,
}

/// Extent of effects test (`extent`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    /// Source.
    pub src: Source,
    /// 5% threshold.
    pub p5: f64,
    /// 10% threshold.
    pub p10: f64,
    /// Floor (veh/h).
    pub floor: f64,
    /// Basis of the floor.
    pub floor_basis: &'static str,
    /// Basis of the freight set.
    pub freight_basis: &'static str,
    /// Buffer (m).
    pub buffer: f64,
    /// General traffic classes tested at 5%.
    pub gt5: Pattern,
    /// Public transport classes tested at 5%.
    pub pt5: Pattern,
    /// Freight classes tested at 5%.
    pub freight5: Pattern,
    /// General traffic classes tested at 10%.
    pub gt10: Pattern,
}

/// Difference plot thresholds (`diff`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiffPlot {
    /// Source.
    pub src: Source,
    /// Floor (veh/h).
    pub floor: f64,
    /// Multiple of the 95th-percentile noise.
    pub noise_k: f64,
    /// Labels from this difference.
    pub label: f64,
    /// Labels from this difference at low zoom.
    pub label_low_zoom: f64,
    /// Zoom below which the low-zoom label applies.
    pub low_zoom: f64,
    /// Narrowest bar (px).
    pub w_min: f64,
    /// Widest bar (px).
    pub w_max: f64,
}

/// TMDG model status (`tmdg`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tmdg {
    /// Source.
    pub src: Source,
    /// Standing of the criteria.
    pub standing: &'static str,
}

/// MSM consistency threshold (`msm`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Msm {
    /// Source.
    pub src: Source,
    /// Threshold (%).
    pub pct: f64,
    /// Text when not checked.
    pub not_checked: &'static str,
}

/// Default design year by pathway (`design`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Design {
    /// Source.
    pub src: Source,
    /// Resource consent (years ahead).
    pub consent: f64,
    /// Plan change.
    pub planchange: f64,
    /// Notice of requirement.
    pub nor: f64,
    /// Fast-track approval.
    pub ftaa: f64,
    /// Warn when the design year is more than this far ahead.
    pub warn_ahead: f64,
    /// Share of an MSM 2-hour period in the peak hour.
    pub msm_peak_share: f64,
    /// Basis of that share.
    pub msm_share_basis: &'static str,
}

/// The whole table (`Engine.STANDARDS`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Standards {
    /// Level-of-service methods.
    pub los: LosTable,
    /// Link V/C bands.
    pub link_vc: LinkVc,
    /// Practical degree of saturation.
    pub practical_dos: PracticalDos,
    /// Peak flow factor.
    pub pff: Pff,
    /// Back of queue.
    pub queue: Queue,
    /// Convergence.
    pub conv: ConvStandard,
    /// Signal to noise.
    pub noise: Noise,
    /// Gap for turn volume exports.
    pub sidra_gap: SidraGap,
    /// MBCM.
    pub mbcm: Mbcm,
    /// MBCM update factors.
    pub mbcm_update: MbcmUpdate,
    /// IPM editions.
    pub ipm: &'static [IpmEdition],
    /// Background growth.
    pub growth: Source,
    /// Bus PCU.
    pub bus: Bus,
    /// Gap acceptance.
    pub gaps: Gaps,
    /// Roundabout lanes.
    pub rbt_lanes: RbtLanes,
    /// Extent test.
    pub extent: Extent,
    /// Difference plots.
    pub diff: DiffPlot,
    /// TMDG.
    pub tmdg: Tmdg,
    /// MSM consistency.
    pub msm: Msm,
    /// Design years.
    pub design: Design,
}

/* ---------------- the table's values ---------------- */

const HCM_URL: &str = "https://www.txdot.gov/manuals/des/tsp/chapter-12-roundabout-analysis/12-2-motorized-vehicle-capacity-methodology/12-2-4-reporting-moes/12-2-4-4-level-of-service.html";
const MBCM_URL: &str = "https://www.nzta.govt.nz/assets/resources/monetised-benefits-and-costs-manual/Monetised-benefits-and-costs-manual-volume-1-procedures.pdf";
const TAG_URL: &str =
    "https://assets.publishing.service.gov.uk/media/6a033d074fb0713aa63ea802/tag-m3-1-highway-assignment-modelling.pdf";
const SIDRA_URL: &str = "https://docs.sidrasolutions.com/intersection/intersection/sidra-glossary/";
const TRB_URL: &str = "https://www.trb.org/Main/Blurbs/175169.aspx";

const fn src(
    id: &'static str,
    name: &'static str,
    edition: &'static str,
    url: Option<&'static str>,
    verified: bool,
) -> Source {
    Source {
        id,
        name,
        edition,
        url,
        verified,
    }
}

const IPM_2024: [IpmBand; 4] = [
    IpmBand { band: "VL", label: "Very low", below: 1.0 },
    IpmBand { band: "L", label: "Low", below: 3.0 },
    IpmBand { band: "M", label: "Medium", below: 6.0 },
    IpmBand { band: "H", label: "High", below: f64::INFINITY },
];
const IPM_2027: [IpmBand; 4] = [
    IpmBand { band: "VL", label: "Very low", below: 1.0 },
    IpmBand { band: "L", label: "Low", below: 1.5 },
    IpmBand { band: "M", label: "Medium", below: 3.0 },
    IpmBand { band: "H", label: "High", below: f64::INFINITY },
];
const IPM: [IpmEdition; 2] = [
    IpmEdition {
        src: src(
            "2024-27",
            "Investment Prioritisation Method 2024\u{2013}27 efficiency rating",
            "2024\u{2013}27 NLTP (final)",
            Some("https://www.nzta.govt.nz/assets/P-and-I-Knowledge-Base/docs/2024-27-IPM.pdf"),
            false,
        ),
        draft: false,
        note: None,
        bands: &IPM_2024,
    },
    IpmEdition {
        src: src(
            "2027-30",
            "Investment Prioritisation Method 2027\u{2013}30 efficiency rating",
            "Initial draft, August 2026",
            Some("https://www.nzta.govt.nz/assets/planning-and-investment/nltp/2027/docs/initial-draft-ipm-for-the-2027-30-nltp-august-2026.pdf"),
            false,
        ),
        draft: true,
        note: Some("draft, subject to GPS 2027"),
        bands: &IPM_2027,
    },
];
const MBCM_RATES: [DiscountStep; 3] = [
    DiscountStep { to: Some(30.0), r: 2.0 },
    DiscountStep { to: Some(100.0), r: 1.5 },
    DiscountStep { to: None, r: 1.0 },
];

/// `Engine.STANDARDS`: every threshold and factor with its source.
pub const STANDARDS: Standards = Standards {
    los: LosTable {
        hcm6: LosMethod {
            src: src("hcm6", "HCM 6 control delay", "Highway Capacity Manual, 6th edition (2016)", Some(HCM_URL), true),
            signals: [10.0, 20.0, 35.0, 55.0, 80.0],
            roundabout: [10.0, 15.0, 25.0, 35.0, 50.0],
            sign: [10.0, 15.0, 25.0, 35.0, 50.0],
            over_capacity_f: true,
            label: "Level of service: HCM 6 control delay (signals A \u{2264} 10, B \u{2264} 20, C \u{2264} 35, D \u{2264} 55, E \u{2264} 80 s; roundabouts and give-way/stop A \u{2264} 10, B \u{2264} 15, C \u{2264} 25, D \u{2264} 35, E \u{2264} 50 s). An approach over capacity (DoS > 1) is F.",
        },
        sidra: LosMethod {
            src: src("sidra", "SIDRA Delay (roundabouts as signals)", "SIDRA Intersection LoS method \"Delay (SIDRA)\"", Some(SIDRA_URL), false),
            signals: [10.0, 20.0, 35.0, 55.0, 80.0],
            roundabout: [10.0, 20.0, 35.0, 55.0, 80.0],
            sign: [10.0, 15.0, 25.0, 35.0, 50.0],
            over_capacity_f: true,
            label: "Level of service: SIDRA Delay method (roundabouts on the signals scale A \u{2264} 10 \u{2026} E \u{2264} 80 s; give-way/stop A \u{2264} 10 \u{2026} E \u{2264} 50 s). An approach over capacity (DoS > 1) is F.",
        },
        rta: LosMethod {
            src: src("rta", "RTA NSW delay bands (option only)", "RTA NSW Guide to Traffic Generating Developments (2002), Table 4.2", Some("https://standards.transport.nsw.gov.au/"), false),
            signals: [14.0, 28.0, 42.0, 56.0, 70.0],
            roundabout: [14.0, 28.0, 42.0, 56.0, 70.0],
            sign: [14.0, 28.0, 42.0, 56.0, 70.0],
            over_capacity_f: true,
            label: "Level of service: RTA NSW delay bands (A < 14, B < 28, C < 42, D < 56, E < 70 s). An approach over capacity (DoS > 1) is F. Option only; HCM 6 is the default.",
        },
    },
    link_vc: LinkVc {
        src: src("corridor-vc", "Link volume/capacity bands", "Spare Construct assumption", None, false),
        cuts: [0.35, 0.55, 0.75, 0.9, 1.0],
    },
    practical_dos: PracticalDos {
        src: src("sidra-xp", "Practical degree of saturation", "SIDRA Intersection defaults", Some(SIDRA_URL), false),
        signals: 0.90,
        roundabout: 0.85,
        sign: 0.80,
    },
    pff: Pff {
        src: src("ttoc06-pff", "Peak flow factor", "TTOC-06 Traffic Signals Modelling Guidelines; SIDRA default convention", Some("https://www.qldc.govt.nz/media/lekcx4iu/ttoc-06-traffic-signals-modelling-guidelines.pdf"), false),
        value: 0.95,
        t: 0.5,
    },
    queue: Queue {
        src: src("hcm2000-bq", "Back of queue", "HCM 2000 Appendix G (signals, Ak\u{e7}elik form); HCM 6 eq. 20-68 (unsignalised)", Some("https://www.txdot.gov/manuals/des/tsp/chapter-4-traffic-analysis-methodology--tools--and/4-5-moes/4-5-5-queue-length.html"), false),
        f95: 1.6,
        k_b: 0.12,
        space: 7.0,
        note: "The 95th-percentile factor of 1.6 is kept until it is checked against HCM 2000 Appendix G.",
    },
    conv: ConvStandard {
        src: src("tag-m31-conv", "Assignment convergence", "UK TAG M3.1 (May 2026), Table 4", Some(TAG_URL), true),
        gap: 0.001,
        chg: 0.01,
        share: 0.98,
        consec: 4.0,
        gap_label: "%GAP (TAG Delta / relative gap)",
        note: "TAG base models: %GAP under 0.1% and P (links changing flow by under 1%) above 98% for 4 consecutive iterations. NZTA TMDG 2019 asks for convergence to be shown but sets no number.",
    },
    noise: Noise {
        src: src("tag-m31-snr", "Scenario signal to noise", "UK TAG M3.1 (May 2026)", Some(TAG_URL), false),
        min: 10.0,
        tighter_gap_tol: 1e-5,
        tighter_max_iter: 400.0,
    },
    sidra_gap: SidraGap {
        src: src("corridor-sidra-gap", "Gap needed for turn volume exports", "Spare Construct assumption", None, false),
        max: 1e-4,
    },
    mbcm: Mbcm {
        src: src("mbcm-1.7.5", "NZTA Monetised Benefits and Costs Manual", "v1.7.5", Some(MBCM_URL), false),
        rates: &MBCM_RATES,
        sens_rate: 8.0,
        years: 40.0,
        long_years: 60.0,
        time_zero: "1 July",
        commercial_rate: 8.0,
        commercial_sens_rate: 2.0,
        discount: DocSource {
            src: src("nzta-gc-25-01", "NZTA General Circular 25/01: updated discount rates", "Effective 6 January 2025", Some("https://www.nzta.govt.nz/resources/monetised-benefits-and-costs-manual"), true),
            file: "docs/references/25-01.pdf",
        },
        label: "Indicative, IER-equivalent screening estimate; not a funding-approval BCR",
        disclaimer: "The benefit-cost ratio is an indicative screening estimate following NZTA Monetised Benefits and Costs Manual v1.7.5 conventions. It is equivalent to an Indicative Efficiency Rating, not a funding-approval BCR. It is not MBCM-compliant unless the full procedures are applied to a model calibrated and validated to the NZTA Transport Model Development Guidelines and independently reviewed. IPM efficiency bands for 2027-30 are shown from NZTA's initial draft (August 2026) and may change after GPS 2027.",
        p95_from: 10.0,
    },
    mbcm_update: MbcmUpdate {
        src: src("mbcm-update-2025", "MBCM update factors", "July 2025", Some("https://www.nzta.govt.nz/assets/resources/monetised-benefits-and-costs-manual/MBCM-update-factors.pdf"), false),
        travel_time: 1.15,
        pt: 1.15,
        walk_cycle: 1.15,
        crash: 1.25,
        emissions: 1.25,
        voc: 1.01,
        behaviour_change: 1.02,
    },
    ipm: &IPM,
    growth: src("mbcm-arithmetic", "Background traffic growth", "MBCM v1.7.5 arithmetic growth; NZ ITA practice", Some(MBCM_URL), false),
    bus: Bus {
        src: src("corridor-bus-pcu", "Passenger car units per bus", "docs/spec/gtfs-bus (Spare Construct research spec)", None, false),
        pcu: 2.0,
        msm: 2.5,
        legacy: 3.0,
    },
    gaps: Gaps {
        src: src("hcm-twsc-gaps", "Critical gaps and follow-up headways", "HCM 6 ch. 20 (TWSC) base values, two-lane major road; mirrored for left-hand traffic", Some(TRB_URL), false),
        major_r: Gap { tc: 4.1, tf: 2.2 },
        minor_l: Gap { tc: 6.2, tf: 3.3 },
        minor_t: Gap { tc: 6.5, tf: 4.0 },
        minor_r: Gap { tc: 7.1, tf: 3.5 },
        stage: 1.0,
        storage: 1.0,
        label: "Movement capacities: HCM 6 two-way stop control method (rank impedance, shared and short lanes), mirrored for NZ left-hand traffic; indicative, not SIDRA output.",
    },
    rbt_lanes: RbtLanes {
        src: src("hcm6-rbt", "Roundabout lane capacity", "HCM 6 ch. 22 lane-based entry capacity equations; NZ markings left lane L+T, right lane T+R", Some(TRB_URL), false),
        one: LaneEq { a: 1380.0, b: 0.00102 },
        one_of2: LaneEq { a: 1420.0, b: 0.00085 },
        two_of1: LaneEq { a: 1420.0, b: 0.00091 },
        left2: LaneEq { a: 1420.0, b: 0.00085 },
        right2: LaneEq { a: 1350.0, b: 0.00092 },
        label: "HCM 6 roundabout (SIDRA Standard is geometry-sensitive and will differ)",
    },
    extent: Extent {
        src: src("at-ita-4.2.1.1", "Extent of effects (5% / 10% test)", "AT ITA Guidelines 2025 v2, s4.2.1.1", Some("https://at.govt.nz/media/ezemv4cz/auckland-transport-integrated-transport-assessment-guidelines-2025-vestion-2.pdf"), false),
        p5: 0.05,
        p10: 0.10,
        floor: 20.0,
        floor_basis: "Spare Construct assumption",
        freight_basis: "Spare Construct assumption (AT does not define \"freight route\")",
        buffer: 50.0,
        gt5: Pattern {
            source: r"^(motorway|strategic( arterial)?|primary arterial|secondary arterial|arterial \(from road class\))$",
            flags: "i",
        },
        pt5: Pattern { source: r"rapid transit|frequent transit|\b(i?rtn|ftn)\b", flags: "i" },
        freight5: Pattern { source: r"^level (1a|1b|2)$", flags: "i" },
        gt10: Pattern { source: r"^collector", flags: "i" },
    },
    diff: DiffPlot {
        src: src("corridor-diff", "Difference plot thresholds", "Spare Construct assumption", None, false),
        floor: 20.0,
        noise_k: 2.0,
        label: 10.0,
        label_low_zoom: 50.0,
        low_zoom: 15.0,
        w_min: 1.0,
        w_max: 14.0,
    },
    tmdg: Tmdg {
        src: src("tmdg-2019", "NZTA Transport Model Development Guidelines", "2019", Some("https://www.nzta.govt.nz/assets/resources/transport-model-development-guidelines/docs/tmd.pdf"), false),
        standing: "TMDG criteria are not a pass/fail checklist; meeting them does not by itself make a model fit for purpose (TMDG s5.8).",
    },
    msm: Msm {
        src: src("corridor-msm", "MSM consistency threshold", "Spare Construct assumption", None, false),
        pct: 20.0,
        not_checked: "MSM consistency not checked: AT ITA v2 s4.2.1.1 expects consistency with MSM forecasts where appropriate.",
    },
    design: Design {
        src: src("rr422-design-year", "Design year", "NZTA RR 422 s5.5; AT ITA Guidelines 2025 v2 s3.6", Some("https://www.nzta.govt.nz/assets/resources/research/reports/422/docs/422.pdf"), false),
        consent: 5.0,
        planchange: 10.0,
        nor: 10.0,
        ftaa: 5.0,
        warn_ahead: 10.0,
        msm_peak_share: 0.55,
        msm_share_basis: "Spare Construct assumption",
    },
};

/// `LOS_SIG`: HCM 6 signal delay bands (s).
pub const LOS_SIG: [f64; 5] = STANDARDS.los.hcm6.signals;
/// `LOS_UNSIG`: HCM 6 give-way and stop delay bands (s).
pub const LOS_UNSIG: [f64; 5] = STANDARDS.los.hcm6.sign;
/// `LOS_VC`: link V/C bands.
pub const LOS_VC: [f64; 5] = STANDARDS.link_vc.cuts;

const LETTERS5: [&str; 5] = ["A", "B", "C", "D", "E"];

/// `losOf(x, th)`: the first band A to E whose limit x does not exceed, otherwise F (NaN is F).
pub fn los_of(x: f64, th: &[f64; 5]) -> &'static str {
    for i in 0..5 {
        if x <= th[i] {
            return LETTERS5[i];
        }
    }
    "F"
}

/// `losLt(x, th)`: "less than" bands (RTA): the first band whose limit x is below, otherwise F.
pub fn los_lt(x: f64, th: &[f64; 5]) -> &'static str {
    for i in 0..5 {
        if x < th[i] {
            return LETTERS5[i];
        }
    }
    "F"
}

/* ---------------- the table as JSON ---------------- */

fn obj(s: &Source, rest: Value) -> Value {
    let mut m = Map::new();
    m.insert("id".into(), s.id.into());
    m.insert("name".into(), s.name.into());
    m.insert("edition".into(), s.edition.into());
    if let Some(u) = s.url {
        m.insert("url".into(), u.into());
    }
    m.insert("verified".into(), s.verified.into());
    if let Value::Object(r) = rest {
        m.extend(r);
    }
    Value::Object(m)
}

fn nums(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|&x| json_number(x)).collect())
}

fn los_value(l: &LosMethod) -> Value {
    obj(
        &l.src,
        json!({ "signals": nums(&l.signals), "roundabout": nums(&l.roundabout), "sign": nums(&l.sign),
            "overCapacityF": l.over_capacity_f, "label": l.label }),
    )
}

fn lane_eq(q: &LaneEq) -> Value {
    json!({ "A": json_number(q.a), "B": json_number(q.b) })
}

fn gap(g: &Gap) -> Value {
    json!({ "tc": json_number(g.tc), "tf": json_number(g.tf) })
}

impl Standards {
    /// The table as JSON, as tools/transport-truth writes `Engine.STANDARDS` (expected/<name>.json "standards").
    ///
    /// That is `JSON.stringify(Engine.STANDARDS)` except for the one non-finite number: the top
    /// IPM bands' `below: Infinity` is written "Infinity" (JSON.stringify writes null). The extent
    /// test's regular expressions are written `{}`, as JSON.stringify writes them.
    pub fn to_value(&self) -> Value {
        let n = json_number;
        let p = &self.mbcm;
        let ipm: Vec<Value> = self
            .ipm
            .iter()
            .map(|e| {
                let bands: Vec<Value> = e
                    .bands
                    .iter()
                    .map(|b| json!({ "band": b.band, "label": b.label, "below": plain_number(b.below) }))
                    .collect();
                let mut rest = json!({ "draft": e.draft, "bands": bands });
                if let Some(note) = e.note {
                    rest["note"] = note.into();
                }
                obj(&e.src, rest)
            })
            .collect();
        let rates: Vec<Value> = p
            .rates
            .iter()
            .map(|r| json!({ "to": r.to.map_or(Value::Null, n), "r": n(r.r) }))
            .collect();
        let x = &self.extent;
        json!({
            "los": { "hcm6": los_value(&self.los.hcm6), "sidra": los_value(&self.los.sidra), "rta": los_value(&self.los.rta) },
            "linkVC": obj(&self.link_vc.src, json!({ "cuts": nums(&self.link_vc.cuts) })),
            "practicalDoS": obj(&self.practical_dos.src, json!({ "signals": n(self.practical_dos.signals),
                "roundabout": n(self.practical_dos.roundabout), "sign": n(self.practical_dos.sign) })),
            "pff": obj(&self.pff.src, json!({ "value": n(self.pff.value), "T": n(self.pff.t) })),
            "queue": obj(&self.queue.src, json!({ "f95": n(self.queue.f95), "kB": n(self.queue.k_b), "space": n(self.queue.space), "note": self.queue.note })),
            "conv": obj(&self.conv.src, json!({ "gap": n(self.conv.gap), "chg": n(self.conv.chg), "share": n(self.conv.share),
                "consec": n(self.conv.consec), "gapLabel": self.conv.gap_label, "note": self.conv.note })),
            "noise": obj(&self.noise.src, json!({ "min": n(self.noise.min),
                "tighter": { "gapTol": n(self.noise.tighter_gap_tol), "maxIter": n(self.noise.tighter_max_iter) } })),
            "sidraGap": obj(&self.sidra_gap.src, json!({ "max": n(self.sidra_gap.max) })),
            "mbcm": obj(&p.src, json!({ "rates": rates, "sensRate": n(p.sens_rate), "years": n(p.years), "longYears": n(p.long_years),
                "timeZero": p.time_zero, "commercial": { "rate": n(p.commercial_rate), "sensRate": n(p.commercial_sens_rate) },
                "discount": obj(&p.discount.src, json!({ "file": p.discount.file })), "label": p.label, "disclaimer": p.disclaimer,
                "p95From": n(p.p95_from) })),
            "mbcmUpdate": obj(&self.mbcm_update.src, json!({ "travelTime": n(self.mbcm_update.travel_time), "pt": n(self.mbcm_update.pt),
                "walkCycle": n(self.mbcm_update.walk_cycle), "crash": n(self.mbcm_update.crash), "emissions": n(self.mbcm_update.emissions),
                "voc": n(self.mbcm_update.voc), "behaviourChange": n(self.mbcm_update.behaviour_change) })),
            "ipm": ipm,
            "growth": obj(&self.growth, json!({})),
            "bus": obj(&self.bus.src, json!({ "pcu": n(self.bus.pcu), "msm": n(self.bus.msm), "legacy": n(self.bus.legacy) })),
            "gaps": obj(&self.gaps.src, json!({ "majorR": gap(&self.gaps.major_r), "minorL": gap(&self.gaps.minor_l),
                "minorT": gap(&self.gaps.minor_t), "minorR": gap(&self.gaps.minor_r), "stage": n(self.gaps.stage),
                "storage": n(self.gaps.storage), "label": self.gaps.label })),
            "rbtLanes": obj(&self.rbt_lanes.src, json!({ "one": lane_eq(&self.rbt_lanes.one), "oneOf2": lane_eq(&self.rbt_lanes.one_of2),
                "twoOf1": lane_eq(&self.rbt_lanes.two_of1), "left2": lane_eq(&self.rbt_lanes.left2), "right2": lane_eq(&self.rbt_lanes.right2),
                "label": self.rbt_lanes.label })),
            "extent": obj(&x.src, json!({ "p5": n(x.p5), "p10": n(x.p10), "floor": n(x.floor), "floorBasis": x.floor_basis,
                "freightBasis": x.freight_basis, "buffer": n(x.buffer), "gt5": {}, "pt5": {}, "freight5": {}, "gt10": {} })),
            "diff": obj(&self.diff.src, json!({ "floor": n(self.diff.floor), "noiseK": n(self.diff.noise_k), "label": n(self.diff.label),
                "labelLowZoom": n(self.diff.label_low_zoom), "lowZoom": n(self.diff.low_zoom), "wMin": n(self.diff.w_min), "wMax": n(self.diff.w_max) })),
            "tmdg": obj(&self.tmdg.src, json!({ "standing": self.tmdg.standing })),
            "msm": obj(&self.msm.src, json!({ "pct": n(self.msm.pct), "notChecked": self.msm.not_checked })),
            "design": obj(&self.design.src, json!({ "consent": n(self.design.consent), "planchange": n(self.design.planchange),
                "nor": n(self.design.nor), "ftaa": n(self.design.ftaa), "warnAhead": n(self.design.warn_ahead),
                "msmPeakShare": n(self.design.msm_peak_share), "msmShareBasis": self.design.msm_share_basis })),
        })
    }
}

/* ---------------- settings read with the table's fallbacks ---------------- */

/// The junction capacity method (`settings.junction.capMethod`): per arc (the default) or per movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapMethod {
    /// One gap-acceptance capacity per minor arc and roundabout entry.
    Arc,
    /// Per-movement gap acceptance with the lane model.
    Movement,
}

impl CapMethod {
    /// "arc" or "movement".
    pub fn as_str(self) -> &'static str {
        match self {
            CapMethod::Arc => "arc",
            CapMethod::Movement => "movement",
        }
    }
}

/// `capMethodOf(S)`: movement only when `settings.junction.capMethod` is exactly "movement".
pub fn cap_method_of(s: &Settings) -> CapMethod {
    match s.junction.get() {
        Some(j) if j.cap_method.is("movement") => CapMethod::Movement,
        _ => CapMethod::Arc,
    }
}

/// Junction analysis settings with their fallbacks (`junctionSet`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JunctionSet {
    /// Level-of-service method: "hcm6", "sidra" or "rta".
    pub method: &'static str,
    /// Peak flow factor (1 when missing or out of range).
    pub pff: f64,
    /// Analysis period (h; 1 when missing).
    pub t: f64,
    /// Queue space per vehicle (m).
    pub queue_space: f64,
    /// 95th-percentile queue factor.
    pub f95: f64,
    /// Capacity method.
    pub cap_method: CapMethod,
}

/// `junctionSet(S)`: the junction block with fallbacks (a settings object without it runs as before v2).
pub fn junction_set(s: &Settings) -> JunctionSet {
    let j = s.junction.get();
    let num = |f: fn(&crate::project::JunctionSettings) -> f64| j.map_or(f64::NAN, f);
    let method = match j.and_then(|j| j.los_method.as_str()) {
        Some("sidra") => "sidra",
        Some("rta") => "rta",
        _ => "hcm6",
    };
    let pff = num(|j| j.pff.num());
    let t = num(|j| j.t.num());
    let qs = num(|j| j.queue_space.num());
    let f95 = num(|j| j.f95.num());
    JunctionSet {
        method,
        pff: if pff > 0.0 && pff <= 1.0 { pff } else { 1.0 },
        t: if t > 0.0 { t } else { 1.0 },
        queue_space: if qs > 0.0 { qs } else { STANDARDS.queue.space },
        f95: if f95 > 0.0 { f95 } else { STANDARDS.queue.f95 },
        cap_method: cap_method_of(s),
    }
}

impl JunctionSet {
    /// The level-of-service method's table entry.
    pub fn los(&self) -> &'static LosMethod {
        match self.method {
            "sidra" => &STANDARDS.los.sidra,
            "rta" => &STANDARDS.los.rta,
            _ => &STANDARDS.los.hcm6,
        }
    }
}

/// Options of an assignment run that override the settings' convergence target and iterations (`opts.gapTol`, `opts.maxIter`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConvOpts {
    /// Gap target, if given.
    pub gap_tol: Option<f64>,
    /// Iterations, if given.
    pub max_iter: Option<f64>,
}

/// Convergence settings (`convSet`): the TAG stability rule and MSA capacity damping unless switched off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvSet {
    /// Relative gap target.
    pub gap_tol: f64,
    /// Iteration budget.
    pub max_iter: f64,
    /// Stop only when link flows are stable too.
    pub stable: bool,
    /// Damp give-way and roundabout capacities by MSA.
    pub msa: bool,
    /// Flow change counted as stable (fraction).
    pub chg: f64,
    /// Share of links that must be stable.
    pub share: f64,
    /// Consecutive stable iterations.
    pub consec: f64,
}

/// `convSet(S, opts)`.
pub fn conv_set(s: &Settings, opts: &ConvOpts) -> ConvSet {
    let d = &STANDARDS.conv;
    let c = s.conv.get();
    let og = opts.gap_tol.unwrap_or(f64::NAN);
    let om = opts.max_iter.unwrap_or(f64::NAN);
    let sg = s.gap_tol.num();
    let sm = s.max_iter.num();
    let chg = c.map_or(f64::NAN, |c| c.chg.num());
    let share = c.map_or(f64::NAN, |c| c.share.num());
    let consec = c.map_or(f64::NAN, |c| c.consec.num());
    ConvSet {
        gap_tol: if og > 0.0 {
            og
        } else if sg > 0.0 {
            sg
        } else {
            1e-4
        },
        max_iter: if om > 0.0 {
            math_round(om)
        } else if sm > 0.0 {
            math_round(sm)
        } else {
            200.0
        },
        stable: !c.is_some_and(|c| c.stable.is_false()),
        msa: match c.map(|c| &c.cap_damp) {
            Some(Maybe::Set(v)) => v == "msa",
            _ => true,
        },
        chg: if chg > 0.0 { chg } else { d.chg },
        share: if share > 0.0 { share } else { d.share },
        consec: if consec > 0.0 {
            math_round(consec)
        } else {
            d.consec
        },
    }
}

/// `pcuF(S)`: passenger car units per vehicle, 1 + hvPct / 100 x (pcuHV - 1).
pub fn pcu_f(s: &Settings) -> f64 {
    1.0 + s.hv_pct.num() / 100.0 * (s.pcu_hv.num() - 1.0)
}

/// `yearOf(sc, S)`: the scenario's year where set (and positive), otherwise the project year.
pub fn year_of(sc: Option<&Scenario>, s: &Settings) -> f64 {
    match sc {
        Some(sc) if sc.year.num() > 0.0 => sc.year.num(),
        _ => s.year.num(),
    }
}

/// `growth(S, sc)`: background growth factor from the base year to the scenario year (linear unless growthMode is "compound").
///
/// Compound growth uses `powf`, which may differ from V8's Math.pow in the last bit.
pub fn growth(s: &Settings, sc: Option<&Scenario>) -> f64 {
    let dy = year_of(sc, s) - s.base_year.num();
    let g = s.growth.or(0.0) / 100.0;
    if s.growth_mode.is("compound") {
        (1.0 + g).powf(dy)
    } else {
        math_max(0.0, 1.0 + g * dy)
    }
}

/// `growthBasis(S)`: the basis the report prints ("linear, 1.5% of 2026 volume per year").
pub fn growth_basis(s: &Settings) -> String {
    let g = to_fixed(s.growth.or(0.0), 1);
    let by = s.base_year.js_string();
    if s.growth_mode.is("compound") {
        format!("compound, {g}% a year from {by}")
    } else {
        format!("linear, {g}% of {by} volume per year")
    }
}

/// `ef(v, S)`: CO2 grams per vehicle-km at average speed v (km/h), heavy vehicles 3.5 times a car, light part times settings.co2Factor.
pub fn ef(v: f64, s: &Settings) -> f64 {
    let v = clamp(v, 5.0, 130.0);
    let car = 3300.0 / v + 95.0 + 0.011 * v * v;
    let hv = s.hv_pct.num() / 100.0;
    let f = s.co2_factor.num();
    let lf = if f > 0.0 && f.is_finite() { f } else { 1.0 };
    car * lf * (1.0 - hv) + car * 3.5 * hv
}
