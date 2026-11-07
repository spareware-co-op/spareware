//! The report's standards and standing text, held as data (cross-cutting rule 4): scopes, the RR 422 checklist, crash risk
//! standards, the Safe System matrix, pedestrian delay constants, regulatory dates, chapters and data vintage rows.

use super::texts::{DISCLAIMER_ITEMS, DISCLAIMERS_VERSION_TEXT};

/// The periods the report reads from the stored results.
pub const PERIODS: [&str; 3] = ["AM", "PM", "IP"];
/// The most junctions the report assesses without an extent test.
pub const MAX_SITES: usize = 20;

/// `D1_SHORT`: the one-line form of Disclaimer D1 for page footers.
pub const D1_SHORT: &str = "Indicative screening results from Spare Construct, a sketch model; not an approved regional model (Disclaimer D1)";

/// The title of a disclaimer.
pub fn disclaimer_title(id: &str) -> &'static str {
    DISCLAIMER_ITEMS
        .iter()
        .find(|d| d.0 == id)
        .map_or("", |d| d.1)
}

/// `dText(id, fill)`: the text of a disclaimer. D10 names the version date (`fill.version`, else the standing one) and D12 the
/// tools it is prepared for (`fill.tools`, else the bracketed list stays).
pub fn disclaimer_text(id: &str, version: Option<&str>, tools: Option<&str>) -> String {
    let mut t = DISCLAIMER_ITEMS
        .iter()
        .find(|d| d.0 == id)
        .map_or(String::new(), |d| d.2.to_string());
    if id == "D10" {
        t = t.replacen(
            "[version date]",
            version
                .filter(|v| !v.is_empty())
                .unwrap_or(DISCLAIMERS_VERSION_TEXT),
            1,
        );
    }
    if id == "D12"
        && let Some(tools) = tools.filter(|t| !t.is_empty())
        && let Some(a) = t.find('[')
        && let Some(b) = t[a..].find(']')
    {
        t = format!("{}{}{}", &t[..a], tools, &t[a + b + 1..]);
    }
    t
}

/// The disclaimer ids in their order.
pub fn disclaimer_ids() -> Vec<&'static str> {
    DISCLAIMER_ITEMS.iter().map(|d| d.0).collect()
}

/// Attribution strings (print-mechanics).
pub mod attrib {
    /// OpenStreetMap.
    pub const OSM: &str = "\u{a9} OpenStreetMap contributors (ODbL)";
    /// NZTA CAS.
    pub const CAS: &str = "Crash data: NZTA CAS (CC BY 4.0)";
    /// NZTA National Speed Limit Register.
    pub const NSLR: &str = "Speed limits: NZTA NSLR (CC BY 4.0)";
    /// Auckland Transport.
    pub const AT: &str = "Auckland Transport open data";
    /// Auckland Council.
    pub const COUNCIL: &str = "Auckland Council open data (Unitary Plan, AGS23)";
    /// NZTA counts.
    pub const NZTA: &str = "Traffic counts: NZTA (CC BY 4.0)";
}

/// `DataHub.AT_LICENCE`'s fallback when the data hub does not give one.
pub const AT_LICENCE_DEFAULT: &str = "Auckland Transport open data, CC BY 4.0 as stated on the AT open data portal (confirm before publishing)";

/// How a data vintage row's licence is found: a fixed text, or the Auckland Transport licence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Licence {
    /// A fixed text.
    Fixed(&'static str),
    /// The Auckland Transport open data licence.
    At,
}

/// Data vintage rows: loaded key, source, dataset, licence.
pub const VINTAGE: [(&str, &str, &str, Licence); 13] = [
    (
        "osm",
        "OpenStreetMap",
        "Road network (Overpass API)",
        Licence::Fixed("ODbL 1.0"),
    ),
    (
        "nzta",
        "NZTA",
        "State highway traffic monitoring sites (AADT)",
        Licence::Fixed("CC BY 4.0"),
    ),
    (
        "tms",
        "NZTA",
        "TMS daily traffic counts",
        Licence::Fixed("CC BY 4.0"),
    ),
    (
        "at",
        "Auckland Transport",
        "Traffic counts (TrafficService)",
        Licence::At,
    ),
    (
        "cas",
        "NZTA",
        "Crash Analysis System (CAS_Data_Public)",
        Licence::Fixed("CC BY 4.0"),
    ),
    (
        "nslr",
        "NZTA",
        "National Speed Limit Register",
        Licence::Fixed("CC BY 4.0"),
    ),
    (
        "fc",
        "Auckland Transport",
        "Future Connect integrated network",
        Licence::At,
    ),
    (
        "controlled",
        "Auckland Transport",
        "Controlled intersections",
        Licence::At,
    ),
    (
        "aup",
        "Auckland Council",
        "Auckland Unitary Plan zones and overlays",
        Licence::Fixed("Auckland Council open data"),
    ),
    (
        "census",
        "Stats NZ",
        "Census 2023 (SA1 and SA2)",
        Licence::Fixed("CC BY 4.0"),
    ),
    (
        "ags23",
        "Auckland Council",
        "Auckland Growth Scenario 2023 (AGS23)",
        Licence::Fixed("Auckland Council open data"),
    ),
    ("gtfs", "Auckland Transport", "GTFS timetable", Licence::At),
    (
        "user",
        "Project team",
        "Your own counts (CSV)",
        Licence::Fixed("Your data"),
    ),
];

/// An RR 422 scope: its key, label and description.
pub struct Scope {
    /// Key.
    pub key: &'static str,
    /// Label.
    pub label: &'static str,
    /// Text.
    pub text: &'static str,
}

/// NZTA RR 422 Table 6.1 scopes, in order.
pub const SCOPES: [Scope; 4] = [
    Scope {
        key: "simple",
        label: "Simple",
        text: "effects within the site and at its interface with the road network",
    },
    Scope {
        key: "moderate",
        label: "Moderate",
        text: "effects on a small area or neighbourhood",
    },
    Scope {
        key: "broad",
        label: "Broad",
        text: "effects on part or all of a suburb",
    },
    Scope {
        key: "extensive",
        label: "Extensive",
        text: "effects on a district or region",
    },
];

/// The scope with this key.
pub fn scope(key: &str) -> Option<&'static Scope> {
    SCOPES.iter().find(|s| s.key == key)
}

/// `LEVEL[l]`: the words for P, L, S, U.
pub fn level(l: char) -> &'static str {
    match l {
        'P' => "Probable",
        'L' => "Likely",
        'S' => "Suggested",
        _ => "Unlikely",
    }
}

/// A row of RR 422 Table 6.3: what content a scope calls for.
pub struct Rr422Row {
    /// Id.
    pub id: &'static str,
    /// Label.
    pub label: &'static str,
    /// P, L, S or U for each scope.
    pub lv: &'static str,
    /// The report chapter it belongs to.
    pub chapter: &'static str,
    /// How to include it.
    pub how: &'static str,
}

/// `RR422.rows`.
pub const RR422_ROWS: [Rr422Row; 8] = [
    Rr422Row {
        id: "crash",
        label: "Crash records",
        lv: "LPPP",
        chapter: "2",
        how: "Load NZTA CAS crashes (Traffic data tab).",
    },
    Rr422Row {
        id: "flows",
        label: "Traffic flows",
        lv: "LLLP",
        chapter: "2",
        how: "Load NZTA and AT counts, or add your own.",
    },
    Rr422Row {
        id: "times",
        label: "Travel times",
        lv: "UUSL",
        chapter: "B",
        how: "Add journey-time checks with observed times.",
    },
    Rr422Row {
        id: "congestion",
        label: "Congestion",
        lv: "USLP",
        chapter: "4",
        how: "Run the model with junction controls set.",
    },
    Rr422Row {
        id: "vehtrips",
        label: "Vehicle trip generation, distribution and assignment",
        lv: "UPPP",
        chapter: "3",
        how: "Add development zones and run the model.",
    },
    Rr422Row {
        id: "persontrips",
        label: "Person trips and modal split",
        lv: "USLP",
        chapter: "3",
        how: "Set trip reductions for public transport and active modes, or load person-trip rates.",
    },
    Rr422Row {
        id: "sensitivity",
        label: "Sensitivity testing",
        lv: "USLP",
        chapter: "4",
        how: "Run the sensitivity pack (Report tab).",
    },
    Rr422Row {
        id: "safety",
        label: "Safety and environmental appraisal",
        lv: "PPPP",
        chapter: "4",
        how: "Load crashes and run the model; complete the Safe System screening.",
    },
];

/// `RR422.name` and the rest of its description.
pub mod rr422 {
    /// Id.
    pub const ID: &str = "rr422-table-6.3";
    /// Name.
    pub const NAME: &str = "NZTA Research Report 422, Table 6.3 (content by scope)";
    /// Edition.
    pub const EDITION: &str = "NZTA RR 422 Integrated transport assessment guidelines (2010)";
    /// Link.
    pub const URL: &str =
        "https://www.nzta.govt.nz/assets/resources/research/reports/422/docs/422.pdf";
    /// Checked against the source.
    pub const VERIFIED: bool = false;
    /// Note.
    pub const NOTE: &str = "Only the rows quoted in Spare Construct's research spec are carried; check the full Table 6.3 in RR 422.";
}

/// `levelOf(row, scope)`: the level a scope gives a row (the first scope's when the scope is not known).
pub fn level_of(row: &Rr422Row, scope_key: &str) -> char {
    let i = SCOPES.iter().position(|s| s.key == scope_key).unwrap_or(0);
    row.lv.as_bytes()[i] as char
}

/// A crash model of the NZTA crash estimation compendium.
pub struct CecModel {
    /// Key.
    pub key: &'static str,
    /// Label.
    pub label: &'static str,
    /// Coefficient.
    pub b0: f64,
    /// Major flow exponent.
    pub b1: f64,
    /// Minor flow exponent.
    pub b2: f64,
    /// A model per approach.
    pub per_approach: bool,
    /// Valid ranges (major, minor), where recorded.
    pub range: Option<([f64; 2], [f64; 2])>,
}

/// `CEC.models`, in order.
pub const CEC_MODELS: [CecModel; 7] = [
    CecModel {
        key: "uncontrolledT",
        label: "Uncontrolled T",
        b0: 2.08e-3,
        b1: 0.19,
        b2: 0.36,
        per_approach: false,
        range: None,
    },
    CecModel {
        key: "priorityCross",
        label: "Priority crossroads",
        b0: 1.13e-3,
        b1: 0.51,
        b2: 0.21,
        per_approach: false,
        range: None,
    },
    CecModel {
        key: "priorityT",
        label: "Priority T",
        b0: 4.68e-5,
        b1: 0.20,
        b2: 0.76,
        per_approach: false,
        range: Some(([5000.0, 26000.0], [1000.0, 5000.0])),
    },
    CecModel {
        key: "signalsCross",
        label: "Signalised crossroads",
        b0: 2.26e-3,
        b1: 0.14,
        b2: 0.46,
        per_approach: false,
        range: None,
    },
    CecModel {
        key: "signalsT",
        label: "Signalised T",
        b0: 1.21e-1,
        b1: 0.12,
        b2: 0.04,
        per_approach: false,
        range: None,
    },
    CecModel {
        key: "rbt1",
        label: "Roundabout, single-lane (per approach)",
        b0: 4.43e-4,
        b1: 0.58,
        b2: 0.0,
        per_approach: true,
        range: None,
    },
    CecModel {
        key: "rbtMulti",
        label: "Roundabout, multi-lane (per approach)",
        b0: 7.95e-4,
        b1: 0.58,
        b2: 0.0,
        per_approach: true,
        range: None,
    },
];

/// `CEC` as data: name, edition, severity indices and the reference speed.
pub mod cec {
    /// Name.
    pub const NAME: &str = "NZTA Crash estimation compendium";
    /// Edition.
    pub const EDITION: &str = "May 2026: Table 10-4 urban (\u{2264} 70 km/h) \"All\" severity indices; Table 7-2 urban 50\u{2013}70 km/h models";
    /// Reference speed (km/h).
    pub const SPEED_REF: f64 = 50.0;
    /// Note on the speed factor.
    pub const SPEED_NOTE: &str = "scaling factor to be verified";
    /// Vehicle severity index at signals.
    pub const SI_SIGNALS: f64 = 0.11;
    /// Roundabout.
    pub const SI_ROUNDABOUT: f64 = 0.10;
    /// Priority.
    pub const SI_PRIORITY: f64 = 0.13;
    /// Midblock vehicle.
    pub const SI_MIDBLOCK: f64 = 0.19;
    /// Generic vehicle.
    pub const SI_GENERIC: f64 = 0.15;
    /// Pedestrian at an intersection.
    pub const SI_PED_INT: f64 = 0.29;
    /// Pedestrian midblock.
    pub const SI_PED_MID: f64 = 0.31;
    /// Cyclist at an intersection.
    pub const SI_CYC_INT: f64 = 0.22;
    /// Cyclist midblock.
    pub const SI_CYC_MID: f64 = 0.26;
    /// Motorcyclist at an intersection.
    pub const SI_MC_INT: f64 = 0.31;
    /// Motorcyclist midblock.
    pub const SI_MC_MID: f64 = 0.39;
}

/// `HRIG` as data.
pub mod hrig {
    /// Name.
    pub const NAME: &str = "NZTA High-risk Intersections Guide";
    /// Edition.
    pub const EDITION: &str = "HRIG Table 4-1 (collective risk) and s4.2.2 (personal risk)";
    /// Years of crash history.
    pub const YEARS: f64 = 5.0;
    /// Fatal and serious crashes that make a site High.
    pub const FS_HIGH: f64 = 3.0;
    /// Injury crashes needed for High and Medium-high personal risk.
    pub const MIN_INJURY: f64 = 4.0;
    /// Exposure factor.
    pub const FACTOR: f64 = 1.7;
    /// Search radius (m).
    pub const RADIUS: f64 = 50.0;
    /// Collective risk bands: lower limit and band, highest first.
    pub const COLLECTIVE: [(f64, &str); 5] = [
        (1.6, "High"),
        (1.1, "Medium-high"),
        (0.6, "Medium"),
        (0.3, "Low-medium"),
        (f64::NEG_INFINITY, "Low"),
    ];
    /// Personal risk bands: limit, band and whether the band starts above (not at) the limit.
    pub const PERSONAL: [(f64, &str, bool); 5] = [
        (32.0, "High", true),
        (16.0, "Medium-high", false),
        (10.0, "Medium", false),
        (6.0, "Low-medium", false),
        (f64::NEG_INFINITY, "Low", false),
    ];
}

/// `SSM`: the Safe System screening matrix.
pub mod ssm {
    /// Name.
    pub const NAME: &str = "Safe System assessment framework";
    /// Edition.
    pub const EDITION: &str = "Austroads AP-R509-16 (2016)";
    /// Label.
    pub const LABEL: &str =
        "Screening aid, not a Safe System Audit; scores not comparable between sites";
    /// Columns: key and label.
    pub const COLS: [(&str, &str); 7] = [
        ("ror", "Run-off-road"),
        ("headon", "Head-on"),
        ("int", "Intersection"),
        ("other", "Other"),
        ("ped", "Pedestrian"),
        ("cyc", "Cyclist"),
        ("mc", "Motorcyclist"),
    ];
    /// Safe System speeds by column key.
    pub fn safe(k: &str) -> f64 {
        match k {
            "ror" => 40.0,
            "headon" => 70.0,
            "int" => 50.0,
            "other" => 50.0,
            _ => 30.0,
        }
    }
    /// Vehicle exposure cut points.
    pub const VEH_EXPOSURE: [f64; 3] = [1000.0, 5000.0, 10000.0];
    /// Vulnerable road user exposure cut points.
    pub const VRU_EXPOSURE: [f64; 3] = [10.0, 50.0, 100.0];
    /// Severity basis.
    pub const SEVERITY_BASIS: &str = "Spare Construct assumption: severity 1 at or below the Safe System speed, 2 up to 20 km/h above, 3 up to 40 km/h above, otherwise 4";
}

/// `PED`: pedestrian delay constants.
pub mod ped {
    /// Name.
    pub const NAME: &str = "Pedestrian crossing delay";
    /// Edition.
    pub const EDITION: &str = "HCM (NCHRP 26518 ch. 8) forms, as stated in Spare Construct spec";
    /// Walking speed (m/s).
    pub const SP: f64 = 1.2;
    /// Start-up time (s).
    pub const TS: f64 = 3.0;
}

/// Public transport frequent service definition.
pub const AT_FREQUENT: &str = "Auckland Transport defines a frequent service as every 15 minutes or better, 7am to 7pm, seven days a week.";

/// `WALK_FACTOR`.
pub mod walk_factor {
    /// Network walk distance over straight-line distance.
    pub const VALUE: f64 = 1.3;
    /// Basis.
    pub const BASIS: &str = "Spare Construct assumption: network walk distance \u{2248} 1.3 \u{d7} straight-line distance until the walk network is loaded";
}

/// Regulatory dates and the stamp list (as at 8 Oct 2026).
pub mod reg {
    /// Law and guidance as at.
    pub const AS_AT: &str = "2026-10-08";
    /// The amendment date: consents lodged before it are processed under the earlier regime.
    pub const AMENDMENT: &str = "2026-10-30";
    /// The regime before the amendment.
    pub const PRE: &str =
        "RMA as it stood immediately before 30 September 2026 (Planning Act 2026 Sch 1 cl 18, 20)";
    /// The regime after.
    pub const POST: &str =
        "RMA as amended by the Planning Act 2026, as at lodgement (Sch 1 cl 19, 21)";
    /// Fast-track.
    pub const FTAA: &str =
        "Fast-track Approvals Act 2024 as at lodgement (FTAA Sch 1 cl 16\u{2013}24)";
    /// Versions relied on.
    pub const STAMPS: [&str; 8] = [
        "RMA (regime above)",
        "Auckland Unitary Plan (operative text, July 2026)",
        "AT ITA Guidelines 2025 Version 2",
        "NZTA PPM ITA Guide G00907 (July 2026)",
        "NZTA Transport Model Development Guidelines (2019)",
        "NZTA Monetised Benefits and Costs Manual v1.7.5",
        "Land Transport Rule: Setting of Speed Limits 2024",
        "TCD Rule (amendment from 1 Nov 2026)",
    ];
    /// Authorities: body and role.
    pub const AUTHORITIES: [(&str, &str); 5] = [
        (
            "Auckland Council",
            "Road controlling authority for Auckland roads since 7 May 2026 (LGACA s47)",
        ),
        (
            "Local board",
            "Specified road controlling authority powers on local and collector roads (LGACA s47C, Sch 4)",
        ),
        (
            "Auckland Transport",
            "Transport CCO, focused on public transport",
        ),
        (
            "NZTA",
            "State highways, where affected (approvals cannot be granted through Fast-track, FTAA s42(4))",
        ),
        (
            "KiwiRail",
            "Rail corridor and level crossings, where affected",
        ),
    ];
}

/// `PATHWAY_LABEL()`: the pathway names (model.js PATHWAYS plus the report's own).
pub const PATHWAY_LABELS: [(&str, &str); 6] = [
    ("consent", "Resource consent"),
    ("planchange", "Plan change"),
    ("nor", "Notice of requirement or designation"),
    ("ftaa", "Fast-track application"),
    ("funding", "NLTF funding (business case)"),
    ("other", "Other"),
];

/// A chapter of the document: id, number, title and the names that map to it.
pub struct Chapter {
    /// Id.
    pub id: &'static str,
    /// Number or letter.
    pub num: &'static str,
    /// An appendix or annex.
    pub app: bool,
    /// Title.
    pub title: &'static str,
    /// Aliases.
    pub alias: &'static [&'static str],
}

/// `CHAPTERS`.
pub const CHAPTERS: [Chapter; 16] = [
    Chapter {
        id: "front",
        num: "",
        app: false,
        title: "Front matter",
        alias: &[
            "front",
            "front matter",
            "cover",
            "executive summary",
            "summary",
            "limitations",
        ],
    },
    Chapter {
        id: "1",
        num: "1",
        app: false,
        title: "Introduction",
        alias: &[
            "introduction",
            "reason for consent",
            "scope",
            "scoping",
            "statutory context",
            "e27",
            "e27 screening",
            "triggers",
        ],
    },
    Chapter {
        id: "2",
        num: "2",
        app: false,
        title: "Transport and land use environment",
        alias: &[
            "existing environment",
            "environment",
            "land use environment",
            "road network",
            "crash history",
            "traffic volumes",
            "speeds",
            "zoning",
            "receiving environment",
        ],
    },
    Chapter {
        id: "3",
        num: "3",
        app: false,
        title: "Description of proposal",
        alias: &[
            "proposal",
            "description of proposal",
            "trip generation",
            "trip distribution",
            "distribution",
            "assignment",
            "access",
            "parking",
            "staging and sequencing",
            "construction traffic",
            "land use",
        ],
    },
    Chapter {
        id: "4",
        num: "4",
        app: false,
        title: "Transport assessment",
        alias: &[
            "assessment",
            "transport assessment",
            "network performance",
            "modelling",
            "junctions",
            "junction performance",
            "safety",
            "road user safety",
            "sensitivity",
            "economics",
            "network alignment",
            "policy alignment",
            "climate",
            "climate adaptation",
            "state highway",
            "state highway effects",
            "anop",
            "anop fit",
            "freight",
        ],
    },
    Chapter {
        id: "5",
        num: "5",
        app: false,
        title: "Mitigation, conditions and monitoring",
        alias: &[
            "mitigation",
            "conditions",
            "monitoring",
            "staging triggers",
            "affected parties",
            "notification",
        ],
    },
    Chapter {
        id: "6",
        num: "6",
        app: false,
        title: "Conclusion",
        alias: &["conclusion"],
    },
    Chapter {
        id: "P",
        num: "P",
        app: true,
        title: "Pathway annexes",
        alias: &[
            "pathway",
            "pathway annexes",
            "annex",
            "plan change",
            "notice of requirement",
            "fast-track",
            "funding",
            "speed limit",
            "cbds",
            "growth funding",
            "time-of-use",
        ],
    },
    Chapter {
        id: "A",
        num: "A",
        app: true,
        title: "Model inputs and standards register",
        alias: &[
            "appendix a",
            "model inputs",
            "inputs",
            "settings",
            "standards register",
        ],
    },
    Chapter {
        id: "B",
        num: "B",
        app: true,
        title: "Calibration and validation",
        alias: &[
            "appendix b",
            "calibration",
            "validation",
            "calibration and validation",
        ],
    },
    Chapter {
        id: "C",
        num: "C",
        app: true,
        title: "Full movement tables",
        alias: &["appendix c", "movement tables", "movements", "sidra tables"],
    },
    Chapter {
        id: "D",
        num: "D",
        app: true,
        title: "Crash listing",
        alias: &["appendix d", "crash listing"],
    },
    Chapter {
        id: "E",
        num: "E",
        app: true,
        title: "ANOP tool inputs",
        alias: &["appendix e", "anop inputs", "anop tool inputs"],
    },
    Chapter {
        id: "F",
        num: "F",
        app: true,
        title: "Data pack manifest",
        alias: &["appendix f", "data pack", "data pack manifest"],
    },
    Chapter {
        id: "G",
        num: "G",
        app: true,
        title: "Expert disclosure and sign-off",
        alias: &["appendix g", "expert disclosure", "expert"],
    },
    Chapter {
        id: "H",
        num: "H",
        app: true,
        title: "Statutory timeline",
        alias: &["appendix h", "statutory timeline", "timeline", "rfi clock"],
    },
];

/// The last chapter (Appendix I), which the array above leaves out of its fixed size.
pub const CHAPTER_I: Chapter = Chapter {
    id: "I",
    num: "I",
    app: true,
    title: "Regulatory checklist and other material",
    alias: &[
        "appendix i",
        "appendices",
        "regulatory checklist",
        "other material",
    ],
};

/// Every chapter, in order.
pub fn chapters() -> Vec<&'static Chapter> {
    let mut v: Vec<&'static Chapter> = CHAPTERS.iter().collect();
    v.push(&CHAPTER_I);
    v
}

/// Labels for the settings keys the appendix prints (`KEY_LABELS`).
pub fn key_label(k: &str) -> Option<&'static str> {
    Some(match k {
        "sv" => "Settings version",
        "period" => "Active period",
        "baseYear" => "Base year",
        "year" => "Default forecast year",
        "growth" => "Background growth (% a year)",
        "growthMode" => "Growth method",
        "hvPct" => "Heavy vehicles (%)",
        "pcuHV" => "PCU per heavy vehicle",
        "peakToDaily" => "Peak hour to daily factor",
        "bprAlpha" => "BPR \u{3b1}",
        "bprBeta" => "BPR \u{3b2}",
        "satFlow" => "Saturation flow (pcu/h per lane)",
        "cycle" => "Default signal cycle (s)",
        "lostPerPhase" => "Lost time per phase (s)",
        "signalOpt" => "Optimise signal splits",
        "maxIter" => "Maximum iterations",
        "gapTol" => "Convergence target (relative gap)",
        "conv.stable" => "TAG stability rule",
        "conv.chg" => "Stability: flow change",
        "conv.share" => "Stability: share of links",
        "conv.consec" => "Stability: iterations",
        "conv.capDamp" => "Capacity damping",
        "junction.losMethod" => "LoS method",
        "junction.pff" => "Peak flow factor",
        "junction.T" => "Analysis period T (h)",
        "junction.queueSpace" => "Queue space (m/veh)",
        "junction.f95" => "95th-percentile queue factor",
        "junction.capMethod" => "Junction capacity method",
        "tmdg.category" => "TMDG model category",
        "tmdg.justification" => "TMDG justification",
        "gravityBeta" => "Gravity distribution \u{3b2} (per minute)",
        "connectorSpeed" => "Zone connector speed (km/h)",
        "connectors" => "Connectors per zone",
        "turns.left" => "Left-turn penalty (s)",
        "turns.right" => "Right-turn penalty (s)",
        "turns.rightSignals" => "Right-turn penalty at signals (s)",
        "turns.uturn" => "U-turns",
        "econ.method" => "Economics method",
        "econ.vot" => "Value of time ($/veh\u{b7}h)",
        "econ.years" => "Analysis period (years)",
        "econ.discount" => "Discount rate (uniform method, %)",
        "econ.sensRate" => "Sensitivity discount rate (%)",
        "econ.annualHours" => "Annualisation hours (uniform method)",
        "data.amShare" => "AM peak share of daily (%)",
        "data.pmShare" => "PM peak share of daily (%)",
        "data.matchRadius" => "Count matching radius (m)",
        _ => return None,
    })
}
