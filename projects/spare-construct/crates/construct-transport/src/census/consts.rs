//! The data sources, standards, default yields and source choices of the land use feature (landuse-plus.js).

use serde_json::{Value, json};

/// The Stats NZ ArcGIS services root.
pub const STATS: &str = "https://services2.arcgis.com/vKb0s8tBIA3bdocZ/arcgis/rest/services";
/// The data.govt.nz CKAN SQL search address (Ministry of Education directory).
pub const CKAN: &str = "https://catalogue.data.govt.nz/api/3/action/datastore_search_sql";
/// The Ministry of Education directory page.
pub const MOE_URL: &str =
    "https://catalogue.data.govt.nz/dataset/directory-of-educational-institutions";
/// The time limit of one download (ms).
pub const TIMEOUT_MS: f64 = 30000.0;
/// Where trip rates come from when a land use has no source of its own.
pub const DEFAULT_RATE_SOURCE: &str = "Spare Construct indicative default (Settings \u{203a} Trip rates)";

/// A data source with its licence (SRC entries).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    /// Short key ("census").
    pub id: &'static str,
    /// Full name.
    pub name: &'static str,
    /// Vintage text ("" when the source has none).
    pub vintage: &'static str,
    /// Licence text.
    pub licence: &'static str,
    /// Web address ("" when there is none).
    pub url: &'static str,
    /// Note on rounding ("" for none).
    pub rounding: &'static str,
    /// CKAN resource id ("" for none).
    pub resource: &'static str,
}

/// SRC.census: Stats NZ 2023 Census totals by topic, SA1.
pub const CENSUS: Source = Source {
    id: "census",
    name: "Stats NZ 2023 Census totals by topic, SA1",
    vintage: "Census 2023 (7 March 2023)",
    licence: "CC BY 4.0, Stats NZ",
    url: "https://www.stats.govt.nz/",
    rounding: "Stats NZ random-rounds counts to base 3, so sums of SA1 counts can differ slightly from published totals",
    resource: "",
};
/// SRC.employees: Stats NZ business demography, employees by SA2.
pub const EMPLOYEES: Source = Source {
    id: "employees",
    name: "Stats NZ business demography, employees by SA2",
    vintage: "",
    licence: "CC BY 4.0, Stats NZ",
    url: "https://services2.arcgis.com/vKb0s8tBIA3bdocZ/arcgis/rest/services/2025_employees_by_statistical_area_2_2/FeatureServer/0",
    rounding: "",
    resource: "",
};
/// SRC.schools: Ministry of Education directory, schools.
pub const SCHOOLS: Source = Source {
    id: "schools",
    name: "Ministry of Education, Directory of Educational Institutions (schools)",
    vintage: "",
    licence: "CC BY 4.0, Ministry of Education (confirm)",
    url: MOE_URL,
    rounding: "",
    resource: "4b292323-9fcc-41f8-814b-3c7b19cf14b3",
};
/// SRC.ece: Ministry of Education directory, early learning services.
pub const ECE: Source = Source {
    id: "ece",
    name: "Ministry of Education, Directory of Educational Institutions (early learning services)",
    vintage: "",
    licence: "CC BY 4.0, Ministry of Education (confirm)",
    url: MOE_URL,
    rounding: "",
    resource: "a9d65b07-8483-4b05-bdfd-d2abe4f38827",
};
/// SRC.aup: Auckland Unitary Plan base zones.
pub const AUP: Source = Source {
    id: "aup",
    name: "Auckland Unitary Plan (Operative in part), base zones",
    vintage: "",
    licence: "Auckland Council open data, CC BY 4.0 (confirm before publishing)",
    url: "https://services1.arcgis.com/n4yPwebTjJCmXB6W/arcgis/rest/services/Unitary_Plan_Base_Zone/FeatureServer/0",
    rounding: "",
    resource: "",
};
/// SRC.osm: OpenStreetMap building outlines.
pub const OSM: Source = Source {
    id: "osm",
    name: "OpenStreetMap building outlines (LINZ import), classified by Spare Construct",
    vintage: "",
    licence: "ODbL, \u{a9} OpenStreetMap contributors",
    url: "",
    rounding: "",
    resource: "",
};

/// The four Census layers (key, address): ind0 and ind2 individuals, dw dwellings, hh households.
pub fn census_layer(k: &str) -> String {
    let (name, n) = match k {
        "ind0" => ("2023_Census_totals_by_topic_for_individuals_by_SA1", 0),
        "ind2" => ("2023_Census_totals_by_topic_for_individuals_by_SA1", 2),
        "dw" => ("2023_Census_totals_by_topic_for_dwellings_by_SA1", 0),
        "hh" => ("2023_Census_totals_by_topic_for_households_by_SA1", 0),
        _ => ("", 0),
    };
    format!("{STATS}/{name}/FeatureServer/{n}")
}

/// STD.ita: the AT ITA Guidelines trip generation standard.
pub mod ita {
    /// Standard id.
    pub const ID: &str = "at-ita-2025-v2";
    /// Standard name.
    pub const NAME: &str = "AT Integrated Transport Assessment Guidelines 2025 v2, Appendix 1";
    /// Address of the guidelines.
    pub const URL: &str = "https://at.govt.nz/media/ezemv4cz/auckland-transport-integrated-transport-assessment-guidelines-2025-vestion-2.pdf";
    /// Not yet verified against the source.
    pub const VERIFIED: bool = false;
    /// The review note.
    pub const REVIEW: &str = "review per AT ITA Guidelines App 1(iv)";
    /// Rates surveyed up to this year need review.
    pub const REVIEW_TO: f64 = 2022.0;
    /// Order of the adjustments.
    pub const SEQUENCE: &str = "gross vehicle trips, less existing site trips, less internal trips, less pass-by and undiverted linked trips, less travel demand management (TDM)";
}

/// STD.rr422: the NZTA Research Report 422 sanity row.
pub mod rr422 {
    /// Standard id.
    pub const ID: &str = "nzta-rr422";
    /// Report name.
    pub const NAME: &str = "NZTA Research Report 422, Trips and parking related to land use (2010)";
    /// Address of the report.
    pub const URL: &str = "https://www.nzta.govt.nz/assets/resources/research/reports/422/docs/422.pdf";
    /// Not yet verified against the source.
    pub const VERIFIED: bool = false;
    /// Peak-hour vehicle trips per household in the report.
    pub const PER_HOUSEHOLD: f64 = 1.0;
    /// The band checked against.
    pub const BAND: [f64; 2] = [0.4, 1.2];
    /// What the band is.
    pub const BAND_LABEL: &str = "Spare Construct band (assumption) around RR 422\u{2019}s about 1 peak-hour vehicle trip per household";
}

/// STD.census, STD.split, STD.eduOcc and the other standards of the land use feature.
pub mod std {
    /// A mode share from fewer people than this falls back to the default.
    pub const CENSUS_MIN_N: f64 = 30.0;
    /// What the minimum is.
    pub const CENSUS_MIN_N_LABEL: &str = "Spare Construct assumption: a mode share from fewer than 30 people falls back to the default";
    /// OSM this far above the Census suggests newer homes (share).
    pub const CENSUS_NEWER: f64 = 0.10;
    /// OSM this many dwellings above the Census suggests newer homes.
    pub const CENSUS_NEWER_ABS: f64 = 3.0;
    /// What the newer rule is.
    pub const CENSUS_NEWER_LABEL: &str = "Spare Construct assumption: OSM more than 10% (and 3 dwellings) above the Census suggests homes built since March 2023";
    /// Default mode split: car driver (%).
    pub const SPLIT_DRIVER: f64 = 75.0;
    /// Default mode split: car passenger (%).
    pub const SPLIT_PASSENGER: f64 = 7.0;
    /// Default mode split: public transport (%).
    pub const SPLIT_PT: f64 = 10.0;
    /// Default mode split: walk (%).
    pub const SPLIT_WALK: f64 = 6.0;
    /// Default mode split: cycle (%).
    pub const SPLIT_CYCLE: f64 = 2.0;
    /// What the default split is.
    pub const SPLIT_LABEL: &str = "Spare Construct assumption (indicative Auckland suburban mode share); replace with surveyed or Census data";
    /// Students per car on education trips.
    pub const EDU_OCC: f64 = 1.3;
    /// What the education occupancy is.
    pub const EDU_OCC_LABEL: &str = "Spare Construct assumption: 1.3 students per car on education trips";
    /// Census dwellings this far (m) from any existing zone get a zone of their own.
    pub const CREATE_M: f64 = 1000.0;
    /// Employees over the Census workplace count must lie in this band.
    pub const JOBS_CHECK: [f64; 2] = [0.7, 1.3];
    /// Yield label.
    pub const YIELDS_LABEL: &str = "Spare Construct default yield; check plan rules";
    /// Developable share of zoned land (%).
    pub const YIELDS_NET_PCT: f64 = 70.0;
    /// What the developable share is.
    pub const YIELDS_NET_LABEL: &str = "Spare Construct assumption: 70% of the zoned land is developable (the rest is roads, reserves and stormwater)";
    /// Label of a zone with no default yield.
    pub const YIELDS_NONE: &str = "no Spare Construct default; enter a yield";
    /// Label of land that takes no yield.
    pub const YIELDS_NOT_LAND: &str = "not for development (no yield)";
    /// Label of a yield the user entered.
    pub const YIELDS_USER: &str = "user-entered yield";
    /// Label of an assumed yield.
    pub const YIELDS_ASSUMED: &str = "assumed; not zoned capacity";
    /// Label of a proposed zone.
    pub const YIELDS_PROPOSED: &str = "proposed (user-selected); not operative zoning";
}

/// A default yield for an AUP base zone code: dwellings per developable ha, GFA ratio, GFA mix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct YieldRow {
    /// AUP zone code.
    pub code: u32,
    /// Dwellings per developable hectare.
    pub dph: Option<f64>,
    /// Land use the dwellings become ("residential", "apartment" or "mixed").
    pub homes: Option<&'static str>,
    /// GFA divided by developable land.
    pub far: Option<f64>,
    /// GFA shares by land use, in order.
    pub mix: Option<&'static [(&'static str, f64)]>,
    /// Quantity per developable hectare by land use, in order.
    pub per: Option<&'static [(&'static str, f64)]>,
    /// A structure-plan assumption, not zoned capacity.
    pub assumed: bool,
    /// Note on the yield.
    pub note: Option<&'static str>,
}

const fn y(code: u32) -> YieldRow {
    YieldRow { code, dph: None, homes: None, far: None, mix: None, per: None, assumed: false, note: None }
}

/// YIELDS: default yields per AUP base zone code, in ascending code order (a JavaScript object lists whole-number keys that way).
pub const YIELDS: [YieldRow; 18] = [
    YieldRow { far: Some(0.6), mix: Some(&[("office", 1.0)]), ..y(1) },
    YieldRow { dph: Some(25.0), homes: Some("residential"), assumed: true, note: Some("Future Urban Zone (AUP H18): no urban development is enabled until the land is rezoned, so this is a structure-plan density assumption, not zoned capacity"), ..y(4) },
    YieldRow { far: Some(0.4), mix: Some(&[("industrial", 1.0)]), ..y(5) },
    YieldRow { dph: Some(40.0), homes: Some("apartment"), far: Some(0.6), mix: Some(&[("retail", 0.8), ("office", 0.2)]), ..y(7) },
    YieldRow { dph: Some(120.0), homes: Some("apartment"), ..y(8) },
    YieldRow { dph: Some(120.0), homes: Some("apartment"), far: Some(1.5), mix: Some(&[("retail", 0.5), ("office", 0.5)]), ..y(10) },
    YieldRow { dph: Some(60.0), homes: Some("apartment"), far: Some(0.6), mix: Some(&[("office", 0.5), ("retail", 0.5)]), ..y(12) },
    YieldRow { far: Some(0.5), mix: Some(&[("industrial", 0.5), ("warehouse", 0.5)]), ..y(17) },
    YieldRow { dph: Some(30.0), homes: Some("residential"), ..y(18) },
    YieldRow { dph: Some(15.0), homes: Some("residential"), ..y(19) },
    YieldRow { dph: Some(80.0), homes: Some("apartment"), far: Some(1.2), mix: Some(&[("retail", 0.6), ("office", 0.4)]), ..y(22) },
    YieldRow { dph: Some(250.0), homes: Some("apartment"), far: Some(4.0), mix: Some(&[("office", 0.7), ("retail", 0.3)]), ..y(35) },
    YieldRow { dph: Some(30.0), homes: Some("apartment"), far: Some(0.4), mix: Some(&[("retail", 1.0)]), ..y(44) },
    YieldRow { far: Some(0.5), mix: Some(&[("retail", 0.5), ("warehouse", 0.5)]), ..y(49) },
    YieldRow { far: Some(0.6), mix: Some(&[("medical", 1.0)]), ..y(55) },
    YieldRow { dph: Some(60.0), homes: Some("mixed"), ..y(60) },
    YieldRow { per: Some(&[("school", 120.0)]), ..y(63) },
    YieldRow { per: Some(&[("tertiary", 200.0)]), ..y(64) },
];

/// The default yield row of a zone code (`YIELDS[code]`).
pub fn yield_row(code: f64) -> Option<&'static YieldRow> {
    YIELDS.iter().find(|r| f64::from(r.code) == code)
}

/// NO_YIELD: zoned land that takes no yield (roads, water, transport corridors, open space, cemeteries and coastal zones).
pub const NO_YIELD: [u32; 17] = [25, 26, 27, 30, 31, 32, 33, 34, 37, 39, 40, 41, 45, 53, 59, 61, 62];
/// GFA_USES: the land uses a GFA ratio can give.
pub const GFA_USES: [&str; 5] = ["retail", "office", "industrial", "warehouse", "medical"];
/// AUP_NAMES: the zone names the feature knows, by code.
pub const AUP_NAMES: [(u32, &str); 18] = [
    (19, "Residential - Single House"),
    (18, "Residential - Mixed Housing Suburban"),
    (60, "Residential - Mixed Housing Urban"),
    (8, "Residential - Terrace Housing and Apartment Building"),
    (35, "Business - City Centre"),
    (10, "Business - Metropolitan Centre"),
    (22, "Business - Town Centre"),
    (7, "Business - Local Centre"),
    (44, "Business - Neighbourhood Centre"),
    (12, "Business - Mixed Use"),
    (49, "Business - General Business"),
    (1, "Business - Business Park"),
    (17, "Business - Light Industry"),
    (5, "Business - Heavy Industry"),
    (63, "Special Purpose - School"),
    (64, "Special Purpose - Tertiary Education"),
    (55, "Special Purpose - Healthcare Facility and Hospital"),
    (4, "Future Urban"),
];

/// `AUP_NAMES[code]`.
pub fn aup_name(code: f64) -> Option<&'static str> {
    AUP_NAMES.iter().find(|(c, _)| f64::from(*c) == code).map(|(_, n)| *n)
}

/// PERIODS: the two peak periods.
pub const PERIODS: [&str; 2] = ["AM", "PM"];
/// MODES: the five travel modes, in order.
pub const MODES: [&str; 5] = ["driver", "passenger", "pt", "walk", "cycle"];
/// MODE_LABEL: the display name of a travel mode.
pub fn mode_label(m: &str) -> &'static str {
    match m {
        "driver" => "Car driver",
        "passenger" => "Car passenger",
        "pt" => "Public transport",
        "walk" => "Walk",
        "cycle" => "Cycle",
        _ => "",
    }
}
/// GROUP_OF: the Census travel topic a land use uses (r usual residence, w workplace, e education).
pub fn group_of(k: &str) -> &'static str {
    match k {
        "residential" | "apartment" | "retirement" => "r",
        "school" | "childcare" | "tertiary" => "e",
        _ => "w",
    }
}
/// MODE_SRC: the Census table a travel topic comes from.
pub fn mode_src(g: &str) -> &'static str {
    match g {
        "r" => "Census 2023 main means of travel to work, by usual residence",
        "w" => "Census 2023 main means of travel to work, by workplace",
        _ => "Census 2023 main means of travel to education, by education address",
    }
}
/// JOB_KEYS: the land uses employees are placed by.
pub const JOB_KEYS: [&str; 7] = ["office", "retail", "industrial", "warehouse", "supermarket", "fastfood", "medical"];
/// NO_YIELD_GROUPS: land groups that take no yield, matched ignoring case.
pub const NO_YIELD_GROUPS: [&str; 2] = ["public open space", "coastal"];
/// Zone codes that are not land (road, water, strategic transport, coastal marine).
pub const NOT_LAND: [u32; 4] = [25, 26, 27, 30];

/// The CSV header of the trip generation export.
pub const CSV_HEAD: [&str; 26] = [
    "zone_id", "activity", "quantity", "unit", "period", "rate", "rate_unit", "rate_source", "rate_year", "person_trips", "pct_car_driver", "pct_car_pass", "pct_pt", "pct_walk", "pct_cycle",
    "occupancy", "veh_trips_gross", "less_existing", "less_internal", "less_passby", "less_tdm", "veh_trips_net", "pct_in", "veh_in", "veh_out", "hv_trips",
];

/// A source choice: which source wins for an item, and why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Choice {
    /// Item id.
    pub id: &'static str,
    /// Item text.
    pub item: &'static str,
    /// Source used.
    pub used: &'static str,
    /// Source it replaces.
    pub instead: &'static str,
    /// Why.
    pub why: &'static str,
}

/// CHOICES: which source wins for each item and why.
pub const CHOICES: [Choice; 7] = [
    Choice { id: "dwellings", item: "Dwellings (houses, apartments)", used: "Census 2023", instead: "OpenStreetMap",
        why: "The Census counts occupied private dwellings (random-rounded to base 3). Building outlines miss units in one building and include outbuildings, so they are used only to place the Census dwellings inside each SA1. Separate houses become houses and joined dwellings apartments." },
    Choice { id: "people", item: "Population, households, vehicles, travel to work and education", used: "Census 2023", instead: "\u{2013}", why: "Only the Census has these. Mode shares feed the person-trip tables." },
    Choice { id: "jobs", item: "Employees", used: "Stats NZ employees by SA2 (2025)", instead: "Census 2023 employed by workplace",
        why: "Newer than the Census. Each SA2 total is kept as a hard control and placed in zones by commercial and industrial floor area; the Census workplace count is a cross-check." },
    Choice { id: "schools", item: "School rolls", used: "Ministry of Education directory", instead: "OpenStreetMap", why: "The directory lists every registered school with its roll and roll date; OpenStreetMap rolls are tags or defaults (350 to 800)." },
    Choice { id: "ece", item: "Early learning places", used: "Ministry of Education directory", instead: "OpenStreetMap", why: "Licensed places for each service, where OpenStreetMap only gives a default of 40 children. Home-based networks are left out (no site of their own)." },
    Choice { id: "gfa", item: "Floor area (retail, office, industrial, warehouse, supermarket, medical, hotel, fast food)", used: "OpenStreetMap", instead: "\u{2013}", why: "No free official floor-area source: building footprint \u{d7} levels, indicative." },
    Choice { id: "zoning", item: "Zoning", used: "Auckland Unitary Plan base zones", instead: "\u{2013}", why: "Operative base zones at each zone centre, for context and default yields." },
];

/// The Census VAR codes in the 2023 release: a topic prefix, the base VAR number and a category list.
const WORK_CATS: [(&str, i32, &str); 12] = [
    ("home", 0, "Work at home"), ("notToday", 1, "Did not go to work today"), ("drivePriv", 2, "Drive a private car, truck or van"), ("driveCo", 3, "Drive a company car, truck or van"),
    ("pass", 4, "Passenger in a car, truck, van or company bus"), ("bus", 5, "Public bus"), ("train", 6, "Train"), ("bike", 7, "Bicycle"), ("walk", 8, "Walk or jog"), ("ferry", 9, "Ferry"), ("other", 10, "Other"), ("stated", 13, "Total stated"),
];
const EDU_CATS: [(&str, i32, &str); 11] = [
    ("home", 0, "Study at home"), ("drive", 1, "Drive a car, truck or van"), ("pass", 2, "Passenger in a car, truck or van"), ("bike", 3, "Bicycle"), ("walk", 4, "Walk or jog"),
    ("schoolBus", 5, "School bus"), ("bus", 6, "Public bus"), ("train", 7, "Train"), ("ferry", 8, "Ferry"), ("other", 9, "Other"), ("stated", 12, "Total Stated"),
];

/// One Census item: the key, the VAR code in the 2023 release and the alias text the layer's field is found by.
#[derive(Clone, Debug, PartialEq)]
pub struct VarSpec {
    /// Item key ("pop", "r.drivePriv", "veh.0").
    pub key: String,
    /// VAR code in the 2023 release.
    pub code: String,
    /// Alias text of the layer's field.
    pub alias: String,
}

fn cats(pfx: &str, topic: &str, base: i32, list: &[(&str, i32, &str)]) -> Vec<VarSpec> {
    list.iter()
        .map(|(k, o, c)| VarSpec { key: format!("{pfx}.{k}"), code: format!("VAR_2_{}", base + o), alias: format!("{topic} ({c})") })
        .collect()
}

fn var(key: &str, code: &str, alias: &str) -> VarSpec {
    VarSpec { key: key.into(), code: code.into(), alias: alias.into() }
}

/// CENSUS_VARS: the items of each Census layer (ind0, ind2, dw, hh), in the order the feature reads them.
pub fn census_vars() -> Vec<(&'static str, Vec<VarSpec>)> {
    let mut ind2 = vec![var("empWork", "VAR_2_677", "Industry, by workplace address (Total)")];
    ind2.extend(cats("r", "Main means of travel to work, by usual residence address", 773, &WORK_CATS));
    ind2.extend(cats("w", "Main means of travel to work, by workplace address", 815, &WORK_CATS));
    ind2.extend(cats("e", "Main means of travel to education, by education address", 195, &EDU_CATS));
    let mut hh = vec![var("hh", "VAR_4_3", "Count of households in occupied private dwellings (Total)")];
    for (i, c) in ["No motor vehicle", "One motor vehicle", "Two motor vehicles", "Three motor vehicles", "Four motor vehicles", "Five or more motor vehicles"].iter().enumerate() {
        hh.push(VarSpec { key: format!("veh.{i}"), code: format!("VAR_4_{}", 136 + i), alias: format!("Number of motor vehicles ({c})") });
    }
    hh.push(var("veh.stated", "VAR_4_144", "Number of motor vehicles (Total stated)"));
    vec![
        ("ind0", vec![var("pop", "VAR_1_3", "Var1: Census usually resident population count (Total)")]),
        ("ind2", ind2),
        ("dw", vec![
            var("occ", "VAR_3_54", "Dwelling occupancy status (Occupied Dwelling)"),
            var("dwTot", "VAR_3_59", "Dwelling occupancy status (Total)"),
            var("sep", "VAR_3_82", "Dwelling type (Separate house)"),
            var("joined", "VAR_3_83", "Dwelling type (Joined dwelling)"),
            var("otherPriv", "VAR_3_84", "Dwelling type (Other private dwelling)"),
        ]),
        ("hh", hh),
    ]
}

/// The specs of one Census layer (`CENSUS_VARS[lk]`).
pub fn census_vars_of(lk: &str) -> Vec<VarSpec> {
    census_vars().into_iter().find(|(k, _)| *k == lk).map(|(_, v)| v).unwrap_or_default()
}

fn source_json(s: &Source) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("id".into(), json!(s.id));
    o.insert("name".into(), json!(s.name));
    for (k, v) in [("vintage", s.vintage), ("licence", s.licence), ("url", s.url), ("rounding", s.rounding), ("resource", s.resource)] {
        if !v.is_empty() {
            o.insert(k.into(), json!(v));
        }
    }
    Value::Object(o)
}

/// The constants as JSON in the prototype's shape (SRC, STD, YIELDS, CHOICES, CENSUS_VARS, MODES and CSV_HEAD), for checking against its own source.
pub fn constants() -> Value {
    let mut layers = serde_json::Map::new();
    for k in ["ind0", "ind2", "dw", "hh"] {
        layers.insert(k.into(), json!(census_layer(k)));
    }
    let mut census = source_json(&CENSUS);
    census["layers"] = Value::Object(layers);
    let yields: serde_json::Map<String, Value> = YIELDS
        .iter()
        .map(|r| {
            let mut o = serde_json::Map::new();
            if let Some(d) = r.dph {
                o.insert("dph".into(), json!(d));
            }
            if let Some(h) = r.homes {
                o.insert("homes".into(), json!(h));
            }
            if let Some(f) = r.far {
                o.insert("far".into(), json!(f));
            }
            for (key, list) in [("mix", r.mix), ("per", r.per)] {
                if let Some(l) = list {
                    o.insert(key.into(), Value::Object(l.iter().map(|(k, v)| (k.to_string(), json!(v))).collect()));
                }
            }
            if r.assumed {
                o.insert("assumed".into(), json!(true));
            }
            if let Some(n) = r.note {
                o.insert("note".into(), json!(n));
            }
            (r.code.to_string(), Value::Object(o))
        })
        .collect();
    let vars: serde_json::Map<String, Value> = census_vars()
        .into_iter()
        .map(|(lk, specs)| {
            let items: serde_json::Map<String, Value> = specs.into_iter().map(|s| (s.key, json!({ "code": s.code, "alias": s.alias }))).collect();
            (lk.to_string(), Value::Object(items))
        })
        .collect();
    json!({
        "SRC": { "census": census, "employees": source_json(&EMPLOYEES), "schools": source_json(&SCHOOLS), "ece": source_json(&ECE), "aup": source_json(&AUP), "osm": source_json(&OSM) },
        "STD": {
            "ita": { "id": ita::ID, "name": ita::NAME, "url": ita::URL, "verified": ita::VERIFIED, "review": ita::REVIEW, "reviewTo": ita::REVIEW_TO, "sequence": ita::SEQUENCE },
            "rr422": { "id": rr422::ID, "name": rr422::NAME, "url": rr422::URL, "verified": rr422::VERIFIED, "perHousehold": rr422::PER_HOUSEHOLD, "band": rr422::BAND, "bandLabel": rr422::BAND_LABEL },
            "census": { "minN": std::CENSUS_MIN_N, "minNLabel": std::CENSUS_MIN_N_LABEL, "newer": std::CENSUS_NEWER, "newerAbs": std::CENSUS_NEWER_ABS, "newerLabel": std::CENSUS_NEWER_LABEL },
            "split": { "driver": std::SPLIT_DRIVER, "passenger": std::SPLIT_PASSENGER, "pt": std::SPLIT_PT, "walk": std::SPLIT_WALK, "cycle": std::SPLIT_CYCLE, "label": std::SPLIT_LABEL },
            "eduOcc": { "v": std::EDU_OCC, "label": std::EDU_OCC_LABEL },
            "createM": std::CREATE_M,
            "jobsCheck": std::JOBS_CHECK,
            "yields": { "label": std::YIELDS_LABEL, "netPct": std::YIELDS_NET_PCT, "verified": false, "netLabel": std::YIELDS_NET_LABEL, "none": std::YIELDS_NONE,
                "notLand": std::YIELDS_NOT_LAND, "user": std::YIELDS_USER, "assumed": std::YIELDS_ASSUMED, "proposed": std::YIELDS_PROPOSED },
        },
        "YIELDS": yields,
        "CHOICES": CHOICES.iter().map(|c| json!({ "id": c.id, "item": c.item, "use": c.used, "instead": c.instead, "why": c.why })).collect::<Vec<_>>(),
        "CENSUS_VARS": vars,
        "MODES": MODES,
        "CSV_HEAD": CSV_HEAD,
    })
}

/// CARD_USES: the land uses the zone card offers first.
pub const LANDUSES_CARD_USES: [&str; 6] = ["residential", "apartment", "retail", "office", "industrial", "school"];
