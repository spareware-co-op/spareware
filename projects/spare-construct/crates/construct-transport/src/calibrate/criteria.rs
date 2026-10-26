//! The criteria library of the calibration panel (features/calibration.js): TMDG 2019 values by purpose category A to G,
//! UK TAG M3.1 alongside, the RMS 2013 and ATAP presets, the Spare Construct conventions that decide a status, the
//! observation methods and count roles, and the tests built from them (`Calibration.tests`).
//!
//! Every value carries its source, edition, table and a verified flag; unverified values are the ones to check against
//! the source document before relying on them.

use crate::jsnum::number_to_js_string as num_text;
use crate::util::fmt;

/// A source document the criteria cite (`SOURCES[id]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CriteriaSource {
    /// Key ("tmdg", "tag", "rms", "atap", "ttoc", "corridor").
    pub id: &'static str,
    /// Full name.
    pub name: &'static str,
    /// Short name.
    pub short: &'static str,
    /// Edition.
    pub edition: &'static str,
    /// Where to read it.
    pub url: &'static str,
}

/// `SOURCES`.
pub const SOURCES: [CriteriaSource; 6] = [
    CriteriaSource {
        id: "tmdg",
        name: "NZTA Transport Model Development Guidelines",
        short: "TMDG 2019",
        edition: "2019",
        url: "https://www.nzta.govt.nz/assets/resources/transport-model-development-guidelines/docs/tmd.pdf",
    },
    CriteriaSource {
        id: "tag",
        name: "UK TAG Unit M3.1 Highway Assignment Modelling",
        short: "TAG M3.1",
        edition: "May 2026",
        url: "https://assets.publishing.service.gov.uk/media/6a033d074fb0713aa63ea802/tag-m3-1-highway-assignment-modelling.pdf",
    },
    CriteriaSource {
        id: "rms",
        name: "RMS Traffic Modelling Guidelines (superseded)",
        short: "RMS 2013",
        edition: "2013",
        url: "https://standards.transport.nsw.gov.au/_entity/annotation/23428da1-b835-ed11-9db1-000d3ae011f9",
    },
    CriteriaSource {
        id: "atap",
        name: "ATAP T1 Travel demand modelling, Appendix C",
        short: "ATAP",
        edition: "secondary extracts",
        url: "https://www.atap.gov.au/tools-techniques/travel-demand-modelling/appendix-c",
    },
    CriteriaSource {
        id: "ttoc",
        name: "QLDC TTOC-06 Traffic signals modelling guidelines",
        short: "TTOC-06",
        edition: "QLDC",
        url: "https://www.qldc.govt.nz/media/lekcx4iu/ttoc-06-traffic-signals-modelling-guidelines.pdf",
    },
    CriteriaSource {
        id: "corridor",
        name: "Spare Construct assumption",
        short: "Spare Construct assumption",
        edition: "Spare Construct",
        url: "",
    },
];

/// `SOURCES[id]`.
pub fn criteria_source(id: &str) -> Option<&'static CriteriaSource> {
    SOURCES.iter().find(|s| s.id == id)
}

/// `CATS`: the TMDG purpose categories.
pub const CATS: [char; 7] = ['A', 'B', 'C', 'D', 'E', 'F', 'G'];

/// `CAT_NAMES[cat]`.
pub fn cat_name(c: char) -> &'static str {
    match c {
        'A' => "regional",
        'B' => "strategic network",
        'C' => "urban area",
        'D' => "NZTA project",
        'E' => "small area or corridor",
        'F' => "intersection or short corridor",
        _ => "high-flow multi-lane",
    }
}

/// `STANDARDS[id]`: the names of the criteria sets the panel offers.
pub fn standard_name(id: &str) -> Option<&'static str> {
    match id {
        "tmdg" => Some("NZTA TMDG 2019"),
        "rms" => Some("RMS 2013 (superseded)"),
        "atap" => Some("ATAP (unverified extracts)"),
        _ => None,
    }
}

/// A value of a TMDG table for a category: one number, or a pair (a range, or acceptable and clarification limits).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TmdgValue {
    /// One number.
    One(f64),
    /// A pair.
    Pair(f64, f64),
}

impl TmdgValue {
    /// The number (the first of a pair).
    pub fn one(self) -> f64 {
        match self {
            TmdgValue::One(x) => x,
            TmdgValue::Pair(x, _) => x,
        }
    }
}

use TmdgValue::{One, Pair};

/// One row of `TMDG`: the value per category, the categories still to check against the PDF, and the table it comes from.
pub struct TmdgRow {
    /// Row key.
    pub key: &'static str,
    /// Values by category (categories without a value are not listed).
    pub v: &'static [(char, TmdgValue)],
    /// Categories whose value is still to be checked against the PDF.
    pub vf: &'static str,
    /// Table in TMDG.
    pub table: &'static str,
}

/// `TMDG`: TMDG 2019 values by category (docs/spec/_validation-rules, read from tmd.pdf).
pub const TMDG_TABLE: [TmdgRow; 17] = [
    TmdgRow { key: "geh5", v: &[('A', One(0.65)), ('B', One(0.75)), ('C', One(0.80)), ('D', One(0.825)), ('E', One(0.85)), ('F', One(0.95)), ('G', One(0.85))], vf: "AB", table: "Individual turning movements and directional link counts (GEH < 5)" },
    TmdgRow { key: "geh75", v: &[('A', One(0.75)), ('B', One(0.80)), ('C', One(0.85)), ('D', One(0.875)), ('E', One(0.90)), ('F', One(1.0)), ('G', One(0.90))], vf: "AB", table: "Individual turning movements and directional link counts (GEH < 7.5)" },
    TmdgRow { key: "geh10", v: &[('A', One(0.85)), ('B', One(0.85)), ('C', One(0.90)), ('D', One(0.925)), ('E', One(0.95)), ('F', One(1.0)), ('G', One(0.95))], vf: "AB", table: "Individual turning movements and directional link counts (GEH < 10)" },
    TmdgRow { key: "geh12", v: &[('A', One(0.95))], vf: "A", table: "Individual counts (GEH < 12, Category A only)" },
    TmdgRow { key: "band", v: &[('B', One(0.70)), ('C', One(0.775)), ('D', One(0.85)), ('E', One(0.90)), ('F', One(0.95)), ('G', One(0.80))], vf: "", table: "Count band criteria (< 400 within 50 veh/h; 400\u{2013}2,000 within 12.5%; > 2,000 within 250 veh/h)" },
    TmdgRow { key: "sgeh5", v: &[('A', One(0.60)), ('B', One(0.75)), ('C', One(0.85)), ('D', One(0.90))], vf: "", table: "Screenline totals (GEH < 5)" },
    TmdgRow { key: "sgeh75", v: &[('A', One(0.75)), ('B', One(0.85)), ('C', One(0.90)), ('D', One(0.95))], vf: "ABCD", table: "Screenline totals (GEH < 7.5)" },
    TmdgRow { key: "sgeh10", v: &[('A', One(0.90)), ('B', One(0.95)), ('C', One(0.95)), ('D', One(1.0))], vf: "ABCD", table: "Screenline totals (GEH < 10)" },
    TmdgRow { key: "sw10", v: &[('A', One(0.70))], vf: "", table: "Screenline totals within 10% (Category A)" },
    TmdgRow { key: "sw15", v: &[('A', One(0.80))], vf: "", table: "Screenline totals within 15% (Category A)" },
    TmdgRow { key: "r2", v: &[('A', One(0.85)), ('B', One(0.90)), ('C', One(0.95)), ('D', One(0.95)), ('E', One(0.95)), ('F', One(0.95)), ('G', One(0.95))], vf: "", table: "Scatter plot, trendline through the origin (R\u{b2})" },
    TmdgRow { key: "slope", v: &[('A', Pair(0.90, 1.10)), ('B', Pair(0.90, 1.10)), ('C', Pair(0.90, 1.10)), ('D', Pair(0.925, 1.075)), ('E', Pair(0.95, 1.05)), ('F', Pair(0.97, 1.03)), ('G', Pair(0.97, 1.03))], vf: "", table: "Scatter plot, trendline through the origin (slope)" },
    TmdgRow { key: "rmse", v: &[('A', Pair(30.0, 40.0)), ('B', Pair(25.0, 35.0)), ('C', Pair(20.0, 30.0)), ('D', Pair(17.5, 27.5)), ('E', Pair(15.0, 25.0))], vf: "", table: "%RMSE (acceptable; requires clarification)" },
    TmdgRow { key: "jt1", v: &[('A', One(0.80)), ('B', One(0.85)), ('C', One(0.85)), ('D', One(0.875)), ('E', One(0.90)), ('F', One(0.90)), ('G', One(0.90))], vf: "", table: "Journey times, tier 1 (within 15% or 1 minute)" },
    TmdgRow { key: "jt2", v: &[('A', One(0.85)), ('B', One(0.90)), ('C', One(0.90)), ('D', One(0.925)), ('E', One(0.95)), ('F', One(1.0)), ('G', One(1.0))], vf: "", table: "Journey times, tier 2 (within 25% or 1.5 minutes)" },
    TmdgRow { key: "meTotal", v: &[('E', Pair(5.0, 10.0)), ('F', Pair(7.0, 15.0))], vf: "", table: "Section 8, total matrix change (high-quality prior; low-quality prior)" },
    TmdgRow { key: "meCR", v: &[('C', One(0.70)), ('D', One(0.75)), ('E', One(0.80))], vf: "", table: "Section 8, trip-length coincidence ratio" },
];

/// `TMDG[key]`.
pub fn tmdg_row(key: &str) -> &'static TmdgRow {
    TMDG_TABLE
        .iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no TMDG row {key:?}"))
}

impl TmdgRow {
    /// `TMDG[key].v[cat]`.
    pub fn at(&self, cat: char) -> Option<TmdgValue> {
        self.v.iter().find(|(c, _)| *c == cat).map(|(_, v)| *v)
    }
}

/// `FIG5`: TMDG Figure 5 applicability by category A to G (e expected, u potentially useful, n not expected) for a test group.
pub fn fig5(group: &str) -> &'static str {
    match group {
        "link" | "turn" | "scatter" => "eeeeeee",
        "band" | "jt" => "uuuuuuu",
        "rmse" => "uuuuunn",
        "screen" => "eeeennn",
        _ => "eeeeeee",
    }
}

/// `FIG5_OK[cat]`: the groups whose applicability for the category comes from rules read so far (verified).
pub fn fig5_ok(cat: char, group: &str) -> bool {
    match cat {
        'E' => ["link", "turn", "band", "scatter", "rmse", "screen"].contains(&group),
        'F' | 'G' => group == "rmse",
        _ => false,
    }
}

/// `USE[letter]`: "expected", "useful" or "not-expected".
pub fn use_of_letter(c: char) -> &'static str {
    match c {
        'e' => "expected",
        'u' => "useful",
        _ => "not-expected",
    }
}

/// A count band rule (veh/h): an observed flow under `lo` is met within `abs_lo`, from `lo` to `hi` within `rel` of it, above `hi` within `abs_hi`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// Lower limit.
    pub lo: f64,
    /// Upper limit.
    pub hi: f64,
    /// Tolerance below lo (veh/h).
    pub abs_lo: f64,
    /// Relative tolerance between lo and hi.
    pub rel: f64,
    /// Tolerance above hi (veh/h).
    pub abs_hi: f64,
    /// Source key.
    pub src: &'static str,
    /// Table.
    pub table: &'static str,
    /// Checked against the source.
    pub verified: bool,
}

/// `BANDS.tmdg`: the TMDG count band.
pub const BAND_TMDG: Band = Band { lo: 400.0, hi: 2000.0, abs_lo: 50.0, rel: 0.125, abs_hi: 250.0, src: "tmdg", table: "Count band criteria", verified: true };
/// `BANDS.tag`: the TAG M3.1 Table 2 flow band.
pub const BAND_TAG: Band = Band { lo: 700.0, hi: 2700.0, abs_lo: 100.0, rel: 0.15, abs_hi: 400.0, src: "tag", table: "Table 2", verified: true };

/// `inBand(B, m, c)`: the modelled flow m is within the band of the counted flow c.
pub fn in_band(b: &Band, m: f64, c: f64) -> bool {
    (m - c).abs() <= (if c < b.lo { b.abs_lo } else if c <= b.hi { b.rel * c } else { b.abs_hi })
}

/// `tmdgBand(m, c)`.
pub fn tmdg_band(m: f64, c: f64) -> bool {
    in_band(&BAND_TMDG, m, c)
}

/// `tagBand(m, c)`.
pub fn tag_band(m: f64, c: f64) -> bool {
    in_band(&BAND_TAG, m, c)
}

/// UK TAG M3.1 (May 2026) values (`TAG`).
pub mod tag {
    /// Table 2: GEH < 5 or within the flow band, share of counts.
    pub const EITHER_V: f64 = 0.85;
    /// Table 2 criterion text.
    pub const EITHER_CRIT: &str = "GEH < 5 or within the flow band, more than 85%";
    /// Table 1: share of screenlines within the tolerance.
    pub const SCREEN_V: f64 = 0.95;
    /// Table 1: directional totals within this share.
    pub const SCREEN_WITHIN: f64 = 0.05;
    /// Table 1: at least this many links per screenline.
    pub const SCREEN_MIN_LINKS: f64 = 5.0;
    /// Table 1 criterion text.
    pub const SCREEN_CRIT: &str = "directional totals within 5%, all or nearly all (95%)";
    /// Table 3: share of routes within the tolerance.
    pub const JT_V: f64 = 0.85;
    /// Table 3 criterion text.
    pub const JT_CRIT: &str = "within 15% or 60 s, more than 85% of routes";
    /// Table 4: relative gap (%).
    pub const GAP_V: f64 = 0.1;
    /// Table 4 criterion text.
    pub const GAP_CRIT: &str = "relative gap, %";
    /// Table 4: share of links that must change by less than CHG.
    pub const P_SHARE: f64 = 0.98;
    /// Table 4: consecutive iterations.
    pub const P_CONSEC: usize = 4;
    /// Table 4 criterion text.
    pub const P_CRIT: &str = "more than 98% of links change by less than 1%, 4 iterations running";
    /// Table 5: matrix cells slope range.
    pub const CELLS_SLOPE: [f64; 2] = [0.98, 1.02];
    /// Table 5: matrix cells R squared.
    pub const CELLS_R2: f64 = 0.95;
    /// Table 5 criterion text for cells.
    pub const CELLS_CRIT: &str = "slope 0.98\u{2013}1.02, R\u{b2} > 0.95, intercept near 0";
    /// Table 5: trip ends slope range.
    pub const ENDS_SLOPE: [f64; 2] = [0.99, 1.01];
    /// Table 5: trip ends R squared.
    pub const ENDS_R2: f64 = 0.98;
    /// Table 5 criterion text for trip ends.
    pub const ENDS_CRIT: &str = "slope 0.99\u{2013}1.01, R\u{b2} > 0.98";
    /// Table 5: trip length mean and SD within this (%).
    pub const TLD_V: f64 = 5.0;
    /// Table 5 criterion text for trip lengths.
    pub const TLD_CRIT: &str = "mean and SD both within 5%";
    /// Table 5: sector-to-sector within this share.
    pub const SECTORS_V: f64 = 0.05;
}

/// RMS 2013 preset values (`RMS`; the document is marked superseded).
pub mod rms {
    /// Link GEH <= 5 share.
    pub const GEH5: f64 = 0.95;
    /// Link GEH <= 10 share.
    pub const GEH10: f64 = 1.0;
    /// Turn GEH <= 5 share.
    pub const TGEH5: f64 = 0.85;
    /// Turn GEH <= 10 share.
    pub const TGEH10: f64 = 1.0;
    /// Screenline GEH < 4 share.
    pub const SGEH4: f64 = 1.0;
    /// R squared.
    pub const R2: f64 = 0.9;
    /// %RMSE.
    pub const RMSE: f64 = 30.0;
    /// Journey times share.
    pub const JT: f64 = 0.95;
}

/// ATAP preset values (`ATAP`, from secondary extracts: to verify).
pub mod atap {
    /// Links within 20% share.
    pub const WITHIN20: f64 = 0.85;
    /// Screenlines within 10% share.
    pub const SW10: f64 = 1.0;
    /// R squared.
    pub const R2: f64 = 0.85;
    /// Slope range.
    pub const SLOPE: [f64; 2] = [0.9, 1.1];
    /// %RMSE.
    pub const RMSE: f64 = 30.0;
}

/// `CORRIDOR`: the Spare Construct conventions that decide a status or a flag.
pub mod corridor {
    /// Fewer observations than this: "Indicative", never pass or fail.
    pub const MIN_N: usize = 10;
    /// Hold-out: every 4th eligible count is a validation count (about 25%).
    pub const HOLD_EVERY: usize = 4;
    /// Trip-end factor bounds.
    pub const TEF_BOUNDS: [f64; 2] = [0.5, 2.0];
    /// ME cell intercept flagged above this share of the mean prior cell.
    pub const ME_INTERCEPT: f64 = 0.01;
    /// ME sector cells tested only with a prior of this many trips or more.
    pub const ME_SECTOR_PRIOR: f64 = 50.0;
    /// Model DoS above this: a stop-line count understates demand.
    pub const DOS_V: f64 = 0.95;
    /// Turning-count balancing: gaps above max(abs, rel) are a midblock source or sink (veh/h).
    pub const BALANCE_ABS: f64 = 50.0;
    /// Turning-count balancing: relative gap.
    pub const BALANCE_REL: f64 = 0.1;
    /// Stress test: demand factors.
    pub const STRESS_FACTORS: [f64; 2] = [1.1, 1.2];
    /// Stress test: flag a DoS rising more than this times the demand increase.
    pub const STRESS_RISE: f64 = 2.0;
}

/// A count observation method of the calibration panel with its matrix-estimation weight (`METHODS`; no RAMM estimates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelMethod {
    /// Key.
    pub key: &'static str,
    /// Display name.
    pub label: &'static str,
    /// Short name.
    pub short: &'static str,
    /// Weight in matrix estimation.
    pub weight: f64,
}

/// `METHODS`.
pub const METHODS: [PanelMethod; 4] = [
    PanelMethod { key: "measured_hour", label: "Measured hour", short: "measured", weight: 1.0 },
    PanelMethod { key: "weekday_factored", label: "Weekday mean \u{d7} peak share", short: "weekday", weight: 0.7 },
    PanelMethod { key: "adt_factored", label: "Daily \u{d7} peak share", short: "factored", weight: 0.4 },
    PanelMethod { key: "virtual", label: "NZTA estimate (virtual site)", short: "virtual", weight: 0.0 },
];

/// `METHODS[key]`.
pub fn panel_method(key: &str) -> Option<&'static PanelMethod> {
    METHODS.iter().find(|m| m.key == key)
}

/// `ROLES[key]`: the names of the count roles (me, valid, gateway, off).
pub fn role_name(key: &str) -> Option<&'static str> {
    match key {
        "me" => Some("Calibration"),
        "valid" => Some("Validation"),
        "gateway" => Some("Gateway"),
        "off" => Some("Not used"),
        _ => None,
    }
}

// `A.5` as the prototype writes it in text: a number with no trailing zeros
fn n(x: f64) -> String {
    num_text(x)
}

/// A test of the criteria library (`Calibration.tests()` entry): what it measures, how it is judged and where the criterion comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct Test {
    /// Test id ("geh5", "tgeh5", "r2", "meTotal", ...).
    pub id: String,
    /// Group ("link", "turn", "band", "tag", "scatter", "rmse", "screen", "jt", "conv", "me").
    pub group: &'static str,
    /// The evidence it reads ("link", "turn", "screen", "jt", "conv", "me").
    pub on: &'static str,
    /// The statistic it reads.
    pub stat: &'static str,
    /// "share", "min", "max", "range", "rmse", "bool", "pair" or "none".
    pub kind: &'static str,
    /// Minimum (share and min kinds).
    pub min: Option<f64>,
    /// Maximum (max and rmse kinds).
    pub max: Option<f64>,
    /// Lower limit (range kind).
    pub lo: Option<f64>,
    /// Upper limit (range kind).
    pub hi: Option<f64>,
    /// Limit above which %RMSE cannot be explained (rmse kind).
    pub clarify: Option<f64>,
    /// Display name.
    pub label: String,
    /// The criterion in words.
    pub crit: String,
    /// Table in the source.
    pub table: String,
    /// Source key.
    pub src: &'static str,
    /// "expected", "useful", "check" or "not-expected".
    pub use_: &'static str,
    /// Value checked against the source.
    pub verified: bool,
    /// Applicability for the category checked against the source (None where not asked).
    pub use_verified: Option<bool>,
}

fn t0(id: &str, group: &'static str, on: &'static str, stat: &'static str, kind: &'static str) -> Test {
    Test {
        id: id.to_string(),
        group,
        on,
        stat,
        kind,
        min: None,
        max: None,
        lo: None,
        hi: None,
        clarify: None,
        label: String::new(),
        crit: String::new(),
        table: String::new(),
        src: "tmdg",
        use_: "expected",
        verified: true,
        use_verified: None,
    }
}

fn tag_t(id: &str, group: &'static str, on: &'static str, stat: &'static str, kind: &'static str) -> Test {
    Test {
        src: "tag",
        use_: "check",
        ..t0(id, group, on, stat, kind)
    }
}

/// `bandCrit(B)`: a band rule in words.
pub fn band_crit(b: &Band) -> String {
    format!(
        "within {} veh/h (< {}), {}% ({}\u{2013}{}), {} veh/h (> {})",
        n(b.abs_lo),
        fmt::int(b.lo),
        fmt::num(b.rel * 100.0, if (b.rel * 100.0) % 1.0 != 0.0 { 1 } else { 0 }),
        fmt::int(b.lo),
        fmt::int(b.hi),
        n(b.abs_hi),
        fmt::int(b.hi)
    )
}

/// `TAGT.band(on)`: TAG Table 2 flow band on links or turning movements.
pub fn tag_band_test(on: &'static str) -> Test {
    let turn = on == "turn";
    Test {
        label: if turn { "Turn TAG flow band" } else { "TAG flow band" }.to_string(),
        crit: band_crit(&BAND_TAG),
        table: BAND_TAG.table.to_string(),
        use_: "useful",
        ..tag_t(if turn { "tTagBand" } else { "tagBand" }, "tag", on, "tag", "none")
    }
}

/// `TAGT.either(on)`: TAG Table 2, GEH < 5 or within the flow band, on links or turning movements.
pub fn tag_either_test(on: &'static str) -> Test {
    let turn = on == "turn";
    Test {
        min: Some(tag::EITHER_V),
        label: if turn { "Turn TAG either (GEH < 5 or flow band)" } else { "TAG either (GEH < 5 or flow band)" }.to_string(),
        crit: format!("{} of {}", tag::EITHER_CRIT, if turn { "turning movements" } else { "counts" }),
        table: "Table 2".to_string(),
        ..tag_t(if turn { "tTagEither" } else { "tagEither" }, "tag", on, "either", "share")
    }
}

/// `TAGT.screen()`: TAG Table 1, screenline totals within 5%.
pub fn tag_screen_test() -> Test {
    Test {
        min: Some(tag::SCREEN_V),
        label: "Screenline within 5% (TAG)".to_string(),
        crit: tag::SCREEN_CRIT.to_string(),
        table: "Table 1".to_string(),
        ..tag_t("tagSl", "tag", "screen", "sw5", "share")
    }
}

/// `TAGT.jt()`: TAG Table 3, journey times.
pub fn tag_jt_test() -> Test {
    Test {
        min: Some(tag::JT_V),
        label: "Journey times (TAG)".to_string(),
        crit: tag::JT_CRIT.to_string(),
        table: "Table 3".to_string(),
        ..tag_t("tagJt", "tag", "jt", "jt1", "share")
    }
}

/// The convergence and matrix-estimation rows (`commonTests(cat)`), shown with every standard; `prior_high` is the prior quality setting (anything but "low").
pub fn common_tests(cat: char, prior_high: bool) -> Vec<Test> {
    let me = tmdg_row("meTotal").at(cat);
    let cr = tmdg_row("meCR").at(cat);
    let me_max = me.map(|v| match v {
        Pair(a, b) => {
            if prior_high {
                a
            } else {
                b
            }
        }
        One(a) => a,
    });
    vec![
        Test {
            max: Some(tag::GAP_V),
            label: "%GAP".to_string(),
            crit: tag::GAP_CRIT.to_string(),
            table: "Table 4".to_string(),
            ..tag_t("gap", "conv", "conv", "gapPct", "max")
        },
        Test {
            label: format!("P \u{d7} {}", tag::P_CONSEC),
            crit: tag::P_CRIT.to_string(),
            table: "Table 4".to_string(),
            ..tag_t("p4", "conv", "conv", "p4", "bool")
        },
        Test {
            max: me_max,
            label: "Matrix total change".to_string(),
            crit: format!("{}-quality prior, %", if prior_high { "high" } else { "low" }),
            table: tmdg_row("meTotal").table.to_string(),
            use_: "check",
            ..t0("meTotal", "me", "me", "totalAbs", if me.is_some() { "max" } else { "none" })
        },
        Test {
            label: "Matrix cells, slope and R\u{b2}".to_string(),
            crit: tag::CELLS_CRIT.to_string(),
            table: "Table 5".to_string(),
            ..tag_t("meCells", "me", "me", "cells", "pair")
        },
        Test {
            label: "Trip ends, slope and R\u{b2}".to_string(),
            crit: tag::ENDS_CRIT.to_string(),
            table: "Table 5".to_string(),
            ..tag_t("meEnds", "me", "me", "ends", "pair")
        },
        Test {
            label: "Trip-length mean and SD".to_string(),
            crit: tag::TLD_CRIT.to_string(),
            table: "Table 5".to_string(),
            ..tag_t("meTld", "me", "me", "tld", "pair")
        },
        Test {
            min: cr.map(|v| v.one()),
            label: "Trip-length coincidence ratio".to_string(),
            crit: "\u{3a3}min / \u{3a3}max over distance bands".to_string(),
            table: tmdg_row("meCR").table.to_string(),
            use_: "check",
            ..t0("meCR", "me", "me", "cr", if cr.is_some() { "min" } else { "none" })
        },
    ]
}

/// `tmdgTests(cat)`: every test of TMDG 2019 for a purpose category, then the TAG, RMS and convergence rows beside them.
pub fn tmdg_tests(cat: char, prior_high: bool) -> Vec<Test> {
    let i = CATS.iter().position(|&c| c == cat).expect("a category A to G");
    let use_of = |g: &str| use_of_letter(fig5(g).as_bytes()[i] as char);
    let fig5ok = |g: &str| fig5_ok(cat, g);
    let v = |key: &str| tmdg_row(key).at(cat);
    let vf = |key: &str| !tmdg_row(key).vf.contains(cat);
    let tb = |key: &str| tmdg_row(key).table.to_string();
    let share = |id: &str, group: &'static str, on: &'static str, key: &str, stat: &'static str, label: &str, crit: &str| -> Test {
        Test {
            min: v(key).map(|x| x.one()),
            label: label.to_string(),
            crit: crit.to_string(),
            table: tb(key),
            verified: vf(key),
            use_: use_of(group),
            use_verified: Some(fig5ok(group)),
            ..t0(id, group, on, stat, if v(key).is_none() { "none" } else { "share" })
        }
    };
    let sl = use_of("screen");
    let band = band_crit(&BAND_TMDG);
    let mut out = vec![
        share("geh5", "link", "link", "geh5", "geh5", "Link GEH < 5", "GEH < 5, % of directional counts"),
        share("geh75", "link", "link", "geh75", "geh75", "Link GEH < 7.5", "GEH < 7.5"),
        share("geh10", "link", "link", "geh10", "geh10", "Link GEH < 10", "GEH < 10"),
    ];
    if cat == 'A' {
        out.push(share("geh12", "link", "link", "geh12", "geh12", "Link GEH < 12", "GEH < 12"));
    }
    out.push(share("band", "band", "link", "band", "band", "TMDG count band", &band));
    out.push(tag_band_test("link"));
    out.push(tag_either_test("link"));
    out.push(share("tgeh5", "turn", "turn", "geh5", "geh5", "Turn GEH < 5", "GEH < 5, % of turning movements"));
    out.push(share("tgeh75", "turn", "turn", "geh75", "geh75", "Turn GEH < 7.5", "GEH < 7.5"));
    out.push(share("tgeh10", "turn", "turn", "geh10", "geh10", "Turn GEH < 10", "GEH < 10"));
    out.push(share("tband", "band", "turn", "band", "band", "Turn TMDG count band", &format!("turning movements {band}")));
    out.push(tag_band_test("turn"));
    out.push(tag_either_test("turn"));
    out.push(Test { use_: sl, ..share("sgeh5", "screen", "screen", "sgeh5", "sgeh5", "Screenline GEH < 5", "GEH < 5 on directional totals") });
    out.push(Test { use_: sl, ..share("sgeh75", "screen", "screen", "sgeh75", "sgeh75", "Screenline GEH < 7.5", "GEH < 7.5 on directional totals") });
    out.push(Test { use_: sl, ..share("sgeh10", "screen", "screen", "sgeh10", "sgeh10", "Screenline GEH < 10", "GEH < 10 on directional totals") });
    if cat == 'A' {
        out.push(share("sw10", "screen", "screen", "sw10", "sw10", "Screenline within 10%", "directional totals within 10%"));
        out.push(share("sw15", "screen", "screen", "sw15", "sw15", "Screenline within 15%", "directional totals within 15%"));
    }
    out.push(tag_screen_test());
    out.push(Test {
        min: Some(rms::SGEH4),
        label: "Screenline GEH < 4 (RMS)".to_string(),
        crit: "every directional total".to_string(),
        src: "rms",
        table: "screenline criteria".to_string(),
        verified: true,
        use_: "useful",
        ..t0("rmsSl", "tag", "screen", "sgeh4", "share")
    });
    out.push(Test {
        label: "R\u{b2} (OLS)".to_string(),
        crit: "ordinary least squares, for information".to_string(),
        table: "reported beside the TMDG test".to_string(),
        use_: "useful",
        ..t0("r2Ols", "scatter", "link", "r2Ols", "none")
    });
    out.push(Test {
        min: v("r2").map(|x| x.one()),
        label: "R\u{b2} through the origin".to_string(),
        crit: "centred on the mean (the TMDG test)".to_string(),
        table: tb("r2"),
        verified: vf("r2"),
        use_: use_of("scatter"),
        use_verified: Some(fig5ok("scatter")),
        ..t0("r2", "scatter", "link", "r2", "min")
    });
    out.push(Test {
        label: "R\u{b2} through the origin (Excel)".to_string(),
        crit: "what Excel shows for a trendline through zero".to_string(),
        table: "for information".to_string(),
        use_: "useful",
        ..t0("r2Excel", "scatter", "link", "r2Excel", "none")
    });
    let slope = v("slope").map(|x| match x {
        Pair(a, b) => (a, b),
        One(a) => (a, f64::NAN),
    });
    out.push(Test {
        lo: slope.map(|s| s.0),
        hi: slope.map(|s| s.1),
        label: "Slope b0".to_string(),
        crit: "trendline through the origin".to_string(),
        table: tb("slope"),
        verified: vf("slope"),
        use_: use_of("scatter"),
        use_verified: Some(fig5ok("scatter")),
        ..t0("slope", "scatter", "link", "b0", "range")
    });
    let rm = v("rmse").map(|x| match x {
        Pair(a, b) => (a, b),
        One(a) => (a, f64::NAN),
    });
    out.push(Test {
        max: rm.map(|r| r.0),
        clarify: rm.map(|r| r.1),
        label: "%RMSE".to_string(),
        crit: "100\u{b7}\u{221a}(\u{3a3}(M\u{2212}C)\u{b2}/(N\u{2212}1)) / (\u{3a3}C/N)".to_string(),
        table: tb("rmse"),
        verified: vf("rmse"),
        use_: use_of("rmse"),
        use_verified: Some(fig5ok("rmse")),
        ..t0("rmse", "rmse", "link", "rmse", if rm.is_some() { "rmse" } else { "none" })
    });
    out.push(Test {
        min: v("jt1").map(|x| x.one()),
        label: "Journey times, tier 1".to_string(),
        crit: "within 15% or 1 minute, % of routes".to_string(),
        table: tb("jt1"),
        use_: use_of("jt"),
        ..t0("jt1", "jt", "jt", "jt1", "share")
    });
    out.push(Test {
        min: v("jt2").map(|x| x.one()),
        label: "Journey times, tier 2".to_string(),
        crit: "within 25% or 1.5 minutes, % of routes".to_string(),
        table: tb("jt2"),
        use_: use_of("jt"),
        ..t0("jt2", "jt", "jt", "jt2", "share")
    });
    out.extend(common_tests(cat, prior_high));
    out
}

/// `presetTests(std)`: the tests of the RMS 2013 preset ("rms") or the ATAP preset (anything else), with the convergence and matrix rows.
pub fn preset_tests(std: &str, prior_high: bool) -> Vec<Test> {
    if std == "rms" {
        let r = |t: Test| Test {
            src: "rms",
            table: "RMS 2013 calibration criteria".to_string(),
            verified: true,
            ..t
        };
        let mut v = vec![
            r(Test { min: Some(rms::GEH5), label: "Link GEH \u{2264} 5".into(), crit: "95% of links".into(), ..t0("geh5", "link", "link", "geh5le", "share") }),
            r(Test { min: Some(rms::GEH10), label: "Link GEH \u{2264} 10".into(), crit: "all links".into(), ..t0("geh10", "link", "link", "geh10le", "share") }),
            r(Test { min: Some(rms::TGEH5), label: "Turn GEH \u{2264} 5".into(), crit: "85% of turns".into(), ..t0("tgeh5", "turn", "turn", "geh5le", "share") }),
            r(Test { min: Some(rms::TGEH10), label: "Turn GEH \u{2264} 10".into(), crit: "all turns".into(), ..t0("tgeh10", "turn", "turn", "geh10le", "share") }),
            r(Test { min: Some(rms::SGEH4), label: "Screenline GEH < 4".into(), crit: "every directional total".into(), ..t0("rmsSl", "screen", "screen", "sgeh4", "share") }),
            r(Test { min: Some(rms::R2), label: "R\u{b2} through the origin".into(), crit: format!("more than {}", n(rms::R2)), ..t0("r2", "scatter", "link", "r2", "min") }),
            r(Test { max: Some(rms::RMSE), label: "%RMSE".into(), crit: format!("{}% or less", n(rms::RMSE)), ..t0("rmse", "rmse", "link", "rmse", "max") }),
            r(Test { min: Some(rms::JT), label: "Journey times".into(), crit: "95% of routes within 15% or 1 minute".into(), ..t0("jt1", "jt", "jt", "jt1", "share") }),
            tag_either_test("link"),
        ];
        v.extend(common_tests('E', prior_high));
        return v;
    }
    let a = |t: Test| Test {
        src: "atap",
        table: "Appendix C (secondary extract)".to_string(),
        verified: false,
        ..t
    };
    let mut v = vec![
        a(Test { min: Some(atap::WITHIN20), label: "Hourly links within \u{b1}20%".into(), crit: "85% of links (share is a Spare Construct assumption)".into(), ..t0("within20", "link", "link", "within20", "share") }),
        a(Test { min: Some(atap::SW10), label: "Screenlines within \u{b1}10%".into(), crit: "every directional total".into(), ..t0("sw10", "screen", "screen", "sw10", "share") }),
        a(Test { min: Some(atap::R2), label: "R\u{b2} through the origin".into(), crit: format!("more than {}", n(atap::R2)), ..t0("r2", "scatter", "link", "r2", "min") }),
        a(Test { lo: Some(atap::SLOPE[0]), hi: Some(atap::SLOPE[1]), label: "Slope b0".into(), crit: format!("{} to {}", n(atap::SLOPE[0]), n(atap::SLOPE[1])), ..t0("slope", "scatter", "link", "b0", "range") }),
        a(Test { max: Some(atap::RMSE), label: "%RMSE".into(), crit: format!("under {}%", n(atap::RMSE)), ..t0("rmse", "rmse", "link", "rmse", "max") }),
        tag_either_test("link"),
    ];
    v.extend(common_tests('E', prior_high));
    v
}

/// `tagPeer(t)`: the TAG M3.1 test beside a row (Table 2 for link and turn rows, Table 1 for screenlines, Table 3 for journey times), None where TAG sets no criterion.
pub fn tag_peer(t: &Test) -> Option<Test> {
    if t.src == "tag" {
        return Some(t.clone());
    }
    if (t.on == "link" || t.on == "turn") && ["link", "turn", "band"].contains(&t.group) {
        return Some(tag_either_test(t.on));
    }
    if t.on == "screen" {
        return Some(tag_screen_test());
    }
    if t.on == "jt" {
        return Some(tag_jt_test());
    }
    None
}

/// `targetText(t)`: the target of a test in words ("> 85%", "0.95 to 1.05", "< 15% (15\u{2013}25% explain)").
pub fn target_text(t: &Test) -> String {
    match t.kind {
        "share" => {
            let m = t.min.unwrap_or(f64::NAN);
            if m >= 1.0 {
                "100%".to_string()
            } else {
                format!("> {}%", fmt::num(m * 100.0, if (m * 100.0) % 1.0 != 0.0 { 1 } else { 0 }))
            }
        }
        "min" => format!("> {}", n(t.min.unwrap_or(f64::NAN))),
        "range" => format!("{} to {}", n(t.lo.unwrap_or(f64::NAN)), n(t.hi.unwrap_or(f64::NAN))),
        "rmse" => format!(
            "< {}% ({}\u{2013}{}% explain)",
            n(t.max.unwrap_or(f64::NAN)),
            n(t.max.unwrap_or(f64::NAN)),
            n(t.clarify.unwrap_or(f64::NAN))
        ),
        "max" => format!(
            "< {}{}",
            n(t.max.unwrap_or(f64::NAN)),
            if t.id == "gap" || t.group == "me" { "%" } else { "" }
        ),
        "bool" => "yes".to_string(),
        "pair" => "see criterion".to_string(),
        _ => "\u{2013}".to_string(),
    }
}

/// `tooltip(t)`: where a test's criterion comes from and whether it has been checked against the source.
pub fn tooltip(t: &Test) -> String {
    let s = criteria_source(t.src).unwrap_or(&SOURCES[0]);
    let mut out = format!(
        "{} ({}), {}. {}",
        s.name,
        s.edition,
        t.table,
        if t.verified {
            "Value checked against the source."
        } else {
            "Value not yet checked against the source document: verify before relying on it."
        }
    );
    if t.use_verified == Some(false) && t.src == "tmdg" && t.use_ != "not-expected" {
        out.push_str(" Applicability for this category is a Spare Construct reading of TMDG Figure 5, to verify.");
    }
    if t.src == "corridor" {
        out.push_str(" Spare Construct assumption.");
    }
    if !s.url.is_empty() {
        out.push(' ');
        out.push_str(s.url);
    }
    out
}

/// `need(target, n)`: counts needed for a share target. TMDG and TAG targets are strict ("E > 85"), so 17 of 20 (85.0%) is not enough; 100% means all.
pub fn need(target: f64, n: usize) -> usize {
    if target >= 1.0 {
        n
    } else {
        n.min(((target * n as f64 + 1e-9).floor() as usize) + 1)
    }
}

/// `SCHOOL_TERMS.years[year]`: New Zealand school terms (Ministry of Education), each as (first day, last day) in ISO; a count inside the holidays is flagged. Term 1 uses the latest permitted start. To be checked against education.govt.nz each year (the prototype marks them unverified).
pub fn school_terms(year: i64) -> Option<[(&'static str, &'static str); 4]> {
    Some(match year {
        2023 => [("2023-02-07", "2023-04-06"), ("2023-04-24", "2023-06-30"), ("2023-07-17", "2023-09-22"), ("2023-10-09", "2023-12-20")],
        2024 => [("2024-02-07", "2024-04-12"), ("2024-04-29", "2024-07-05"), ("2024-07-22", "2024-09-27"), ("2024-10-14", "2024-12-20")],
        2025 => [("2025-02-07", "2025-04-11"), ("2025-04-28", "2025-07-04"), ("2025-07-21", "2025-09-26"), ("2025-10-13", "2025-12-19")],
        2026 => [("2026-02-06", "2026-04-02"), ("2026-04-20", "2026-07-03"), ("2026-07-20", "2026-09-25"), ("2026-10-12", "2026-12-18")],
        _ => return None,
    })
}

/// `CORRIDOR.dos.src`'s source: QLDC TTOC-06 (a model DoS above this makes a stop-line count understate demand).
pub const DOS_SOURCE: &str = "ttoc";

/// `GEH_BANDS` of the panel: the bands of the criteria library, lower limit included, upper excluded.
pub const GEH_BANDS_PANEL: [(f64, f64); 5] = [(0.0, 5.0), (5.0, 7.5), (7.5, 10.0), (10.0, 12.0), (12.0, f64::INFINITY)];
