//! The official data sources and the thresholds the feeds use, as data (`SRC`, `COUNCILS`, `NZ_FEEDS`, `RULES`, `TERMS`, `MATARIKI`, `DEFAULTS`).

use super::load::AT_LICENCE;
use serde_json::{json, Value};

/// The NZTA ArcGIS services folder.
pub const NZTA: &str = "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services";
/// The Auckland Transport ArcGIS services folder.
pub const AT: &str = "https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services";
/// The Auckland Council ArcGIS services folder.
pub const AC: &str = "https://services1.arcgis.com/n4yPwebTjJCmXB6W/arcgis/rest/services";
/// The Mobility Database mirror.
pub const MDB: &str = "https://files.mobilitydatabase.org";
/// The licence text kept with Auckland Council data.
pub const AC_LICENCE: &str = "Auckland Council open data, CC BY 4.0 (confirm before publishing)";

/// One data source: where it is, what it is called and under what licence (`SRC[id]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Src {
    /// Key ("nslr", "controlled", ...).
    pub id: &'static str,
    /// Short label.
    pub label: &'static str,
    /// Full name.
    pub name: &'static str,
    /// Service address ("" for a layer the user names).
    pub url: &'static str,
    /// The source's web page.
    pub page: Option<&'static str>,
    /// Licence.
    pub licence: &'static str,
    /// Attribution line for maps and reports.
    pub attribution: Option<&'static str>,
    /// Note kept with the source.
    pub note: Option<&'static str>,
}

impl Src {
    /// The source as the prototype's object (only the keys it has).
    pub fn to_value(&self) -> Value {
        let mut o = serde_json::Map::new();
        o.insert("label".into(), json!(self.label));
        o.insert("name".into(), json!(self.name));
        o.insert("url".into(), json!(self.url));
        if let Some(p) = self.page {
            o.insert("page".into(), json!(p));
        }
        o.insert("licence".into(), json!(self.licence));
        if let Some(a) = self.attribution {
            o.insert("attribution".into(), json!(a));
        }
        if let Some(n) = self.note {
            o.insert("note".into(), json!(n));
        }
        Value::Object(o)
    }
}

/// NZTA National Speed Limit Register.
pub const SRC_NSLR: Src = Src {
    id: "nslr",
    label: "Legal speed limits",
    name: "NZTA National Speed Limit Register",
    url: "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/SpeedLimitZoneFull__View/FeatureServer/0",
    page: Some("https://www.arcgis.com/home/item.html?id=aa376f1f2f3643bdac4d18855229239c"),
    licence: "CC BY 4.0",
    attribution: Some("Speed limits: NZTA National Speed Limit Register (CC BY 4.0)"),
    note: None,
};
/// AT controlled intersections.
pub const SRC_CONTROLLED: Src = Src {
    id: "controlled",
    label: "Signalised sites",
    name: "AT controlled intersections (RoadingService layer 2)",
    url: "https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/RoadingService/FeatureServer/2",
    page: Some("https://at.govt.nz/about-us/at-data-sources"),
    licence: AT_LICENCE,
    attribution: None,
    note: Some("AT marks this layer 'not refreshed', so check recently built signals on site."),
};
/// AT regulatory signs.
pub const SRC_SIGNS: Src = Src {
    id: "signs",
    label: "Regulatory signs",
    name: "AT signs (RoadingService layer 0): RG 5 stop, RG 6 give way, RG 6R roundabout",
    url: "https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/RoadingService/FeatureServer/0",
    page: Some("https://at.govt.nz/about-us/at-data-sources"),
    licence: AT_LICENCE,
    attribution: None,
    note: None,
};
/// OpenStreetMap turn restrictions.
pub const SRC_OSM: Src = Src {
    id: "osm",
    label: "Turn bans",
    name: "OpenStreetMap turn restrictions (Overpass)",
    url: "https://overpass-api.de/api/interpreter",
    page: Some("https://www.openstreetmap.org/copyright"),
    licence: "ODbL 1.0, \u{a9} OpenStreetMap contributors",
    attribution: None,
    note: None,
};
/// NZTA TMS daily counts.
pub const SRC_TMS: Src = Src {
    id: "tms",
    label: "Daily counts",
    name: "NZTA TMS daily traffic counts",
    url: "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/TMS_Telemetry_Sites/FeatureServer/0",
    page: Some("https://catalogue.data.govt.nz/dataset/tms-daily-traffic-counts-api4"),
    licence: "CC BY 4.0",
    attribution: None,
    note: None,
};
/// TMS peak profiles built from the quarter-hourly archive (a file on the host's own site).
pub const SRC_PROFILES: Src = Src {
    id: "profiles",
    label: "Peak profiles",
    name: "TMS peak profiles built from the NZTA quarter-hourly archive",
    url: "data/tms_profiles.json",
    page: Some("https://www.arcgis.com/home/item.html?id=39cdaa422dbd45c4b41e0e59b49e79e2"),
    licence: "CC BY 4.0",
    attribution: None,
    note: None,
};
/// Auckland Council AGS23 growth scenario.
pub const SRC_AGS: Src = Src {
    id: "ags",
    label: "Growth (Auckland)",
    name: "Auckland Council AGS23 v1.1 (Auckland Growth Scenario)",
    url: "https://services1.arcgis.com/n4yPwebTjJCmXB6W/arcgis/rest/services/Auckland_Growth_Scenario_v1_1/FeatureServer/0",
    page: Some("https://www.knowledgeauckland.org.nz/publications/auckland-growth-scenario-2023-version-11-ags23v11-data/"),
    licence: "Auckland Council terms: fetched live for this study area; never bundled or republished",
    attribution: Some("Auckland Council AGS23 v1.1"),
    note: None,
};
/// The Mobility Database catalogue of GTFS feeds.
pub const SRC_GTFS_CAT: Src = Src {
    id: "gtfsCat",
    label: "Bus feed list",
    name: "Mobility Database catalogue",
    url: "https://files.mobilitydatabase.org/feeds_v2.csv",
    page: Some("https://mobilitydatabase.org/terms-and-conditions"),
    licence: "CC0 (catalogue metadata)",
    attribution: None,
    note: None,
};
/// A GTFS timetable through the Mobility Database mirror ({id} is the feed's id).
pub const SRC_GTFS: Src = Src {
    id: "gtfs",
    label: "Bus timetables",
    name: "GTFS timetable through the Mobility Database mirror",
    url: "https://files.mobilitydatabase.org/{id}/latest.zip",
    page: Some("https://mobilitydatabase.org/terms-and-conditions"),
    licence: "The publisher's own licence (see the feed); Mobility Database terms",
    attribution: None,
    note: None,
};
/// Auckland Council flood plains.
pub const SRC_FLOOD: Src = Src {
    id: "flood",
    label: "Flood plains",
    name: "Auckland Council flood plains",
    url: "https://services1.arcgis.com/n4yPwebTjJCmXB6W/arcgis/rest/services/Flood_Plains/FeatureServer/0",
    page: Some("https://data-aucklandcouncil.opendata.arcgis.com/"),
    licence: AC_LICENCE,
    attribution: None,
    note: None,
};
/// Auckland Council coastal inundation.
pub const SRC_COASTAL: Src = Src {
    id: "coastal",
    label: "Coastal inundation",
    name: "Auckland Council coastal inundation (1% AEP and sea-level-rise layers)",
    url: "https://services1.arcgis.com/n4yPwebTjJCmXB6W/arcgis/rest/services",
    page: Some("https://data-aucklandcouncil.opendata.arcgis.com/"),
    licence: AC_LICENCE,
    attribution: None,
    note: None,
};
/// A council count layer the user names.
pub const SRC_COUNCIL: Src = Src {
    id: "council",
    label: "Council counts",
    name: "Council count layer (ArcGIS)",
    url: "",
    page: None,
    licence: "The council's own licence (check the layer)",
    attribution: None,
    note: None,
};
/// The NZTA Motor Vehicle Register.
pub const SRC_MVR: Src = Src {
    id: "mvr",
    label: "Fleet mix",
    name: "NZTA Motor Vehicle Register",
    url: "https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services",
    page: Some("https://opendata-nzta.opendata.arcgis.com/"),
    licence: "CC BY 4.0",
    attribution: None,
    note: None,
};

/// Every source, in the prototype's order (`SRC`).
pub const SRC: [Src; 13] = [
    SRC_NSLR,
    SRC_CONTROLLED,
    SRC_SIGNS,
    SRC_OSM,
    SRC_TMS,
    SRC_PROFILES,
    SRC_AGS,
    SRC_GTFS_CAT,
    SRC_GTFS,
    SRC_FLOOD,
    SRC_COASTAL,
    SRC_COUNCIL,
    SRC_MVR,
];

/// `SRC[id]`.
pub fn src(id: &str) -> Option<&'static Src> {
    SRC.iter().find(|s| s.id == id)
}

/// `SRC` as the prototype's object, keyed by source.
pub fn src_value() -> Value {
    Value::Object(SRC.iter().map(|s| (s.id.to_string(), s.to_value())).collect())
}

/// A council count preset (`COUNCILS[key]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CouncilPreset {
    /// Key ("hamilton", "qldc").
    pub key: &'static str,
    /// Name.
    pub label: &'static str,
    /// Layer address.
    pub url: &'static str,
    /// Only the layer's latest counts.
    pub latest: bool,
    /// Counts older than this many years are left out.
    pub max_age_years: Option<f64>,
}

/// `COUNCILS`.
pub const COUNCILS: [CouncilPreset; 2] = [
    CouncilPreset {
        key: "hamilton",
        label: "Hamilton City Council",
        url: "https://services1.arcgis.com/R6s0QqCMQdwKY6yp/arcgis/rest/services/Hamilton%20City%20Traffic%20Counts/FeatureServer/0",
        latest: false,
        max_age_years: None,
    },
    CouncilPreset {
        key: "qldc",
        label: "Queenstown Lakes District Council",
        url: "https://services1.arcgis.com/9YyqaQtDdDR8tupG/arcgis/rest/services/Traffic_by_Site_Latest_Counts/FeatureServer/0",
        latest: true,
        max_age_years: Some(10.0),
    },
];

/// `COUNCILS[key]`.
pub fn council(key: &str) -> Option<&'static CouncilPreset> {
    COUNCILS.iter().find(|c| c.key == key)
}

/// `COUNCILS` as the prototype's object.
pub fn councils_value() -> Value {
    Value::Object(
        COUNCILS
            .iter()
            .map(|c| {
                let mut o = serde_json::Map::new();
                o.insert("label".into(), json!(c.label));
                o.insert("url".into(), json!(c.url));
                if c.latest {
                    o.insert("latest".into(), json!(true));
                }
                if let Some(a) = c.max_age_years {
                    o.insert("maxAgeYears".into(), json!(a));
                }
                (c.key.to_string(), Value::Object(o))
            })
            .collect(),
    )
}

/// A Mobility Database feed for New Zealand, with a rough coverage box used only when the catalogue cannot be read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NzFeed {
    /// The feed's id in the Mobility Database.
    pub id: &'static str,
    /// The operator.
    pub provider: &'static str,
    /// Coverage [south, west, north, east].
    pub box_: [f64; 4],
}

/// `NZ_FEEDS`.
pub const NZ_FEEDS: [NzFeed; 6] = [
    NzFeed { id: "mdb-1029", provider: "Auckland Transport", box_: [-37.45, 174.15, -36.05, 175.6] },
    NzFeed { id: "mdb-1132", provider: "Metlink (Greater Wellington)", box_: [-41.65, 174.55, -40.6, 176.1] },
    NzFeed { id: "mdb-1313", provider: "Metro (Christchurch)", box_: [-43.95, 171.9, -43.2, 173.1] },
    NzFeed { id: "mdb-983", provider: "Otago Regional Council", box_: [-46.3, 168.3, -44.6, 170.9] },
    NzFeed { id: "tld-6738", provider: "Waikato Regional Council", box_: [-38.6, 174.6, -37.25, 176.3] },
    NzFeed { id: "mdb-1071", provider: "BayBus (Bay of Plenty)", box_: [-38.4, 175.8, -37.4, 177.3] },
];

/// `NZ_FEEDS` as the prototype's list.
pub fn nz_feeds_value() -> Value {
    Value::Array(NZ_FEEDS.iter().map(|f| json!({ "id": f.id, "provider": f.provider, "box": f.box_ })).collect())
}

/// `RULES.nslr.samples`: where along a link the register is read (fractions of its length).
pub const NSLR_SAMPLES: [f64; 3] = [0.25, 0.5, 0.75];
/// `RULES.nslr.futureDay`: the day a future scenario year's limits apply from.
pub const NSLR_FUTURE_DAY: &str = "07-01";
/// `RULES.periods.AM`: the AM peak window, hours.
pub const PERIOD_AM: [f64; 2] = [7.0, 9.0];
/// `RULES.periods.PM`: the PM peak window, hours.
pub const PERIOD_PM: [f64; 2] = [16.0, 18.0];
/// `RULES.control.siteM`: a signal site belongs to a junction within this distance (m).
pub const CONTROL_SITE_M: f64 = 40.0;
/// `RULES.control.signM`: a sign belongs to a junction within this distance (m).
pub const CONTROL_SIGN_M: f64 = 25.0;
/// `RULES.control.tailM`: the part of an approach (m) a sign is matched to.
pub const CONTROL_TAIL_M: f64 = 30.0;
/// `RULES.control.pedCycle`: the cycle time of a signalised pedestrian crossing (s).
pub const CONTROL_PED_CYCLE: f64 = 60.0;
/// `RULES.control.pedCycleRange`.
pub const CONTROL_PED_CYCLE_RANGE: &str = "60\u{2013}90 s";
/// `RULES.tms.minShare`: days under this share of the 28-day median are dropped.
pub const TMS_MIN_SHARE: f64 = 0.5;
/// `RULES.tms.medianDays`: the window of the median (days).
pub const TMS_MEDIAN_DAYS: f64 = 28.0;
/// `RULES.tms.aadtDays`: valid days needed for an AADT.
pub const TMS_AADT_DAYS: f64 = 300.0;
/// `RULES.tms.termWeeks`: school-term weeks of Tuesday to Thursday data wanted (fewest, most).
pub const TMS_TERM_WEEKS: [f64; 2] = [8.0, 12.0];
/// `RULES.tms.z`: the multiplier of the 95% confidence interval.
pub const TMS_Z: f64 = 1.96;
/// `RULES.tms.days`: days of daily counts to download.
pub const TMS_DAYS: f64 = 400.0;
/// `RULES.tms.chunk`: sites asked for in one query.
pub const TMS_CHUNK: usize = 8;
/// `RULES.tms.maxPages`: pages of one TMS query.
pub const TMS_MAX_PAGES: usize = 120;
/// `RULES.tms.hcvRule`.
pub const TMS_HCV_RULE: &str = "NZTA length classes: under 5.5 m light, over 11 m heavy, 5.5\u{2013}11 m split 50:50 between light and heavy";
/// `RULES.ags.gatewayKm`: growth beyond a gateway is read within this distance (km).
pub const AGS_GATEWAY_KM: f64 = 10.0;
/// `RULES.ags.gatewayDeg`: the sector beyond a gateway, plus and minus this many degrees.
pub const AGS_GATEWAY_DEG: f64 = 60.0;
/// `RULES.ags.years`: the years the scenario covers.
pub const AGS_YEARS: [f64; 2] = [2022.0, 2052.0];
/// `RULES.ags.groups`: which growth a land use group follows.
pub const AGS_GROUPS: [(&str, &str); 4] = [("Residential", "hh"), ("Commercial", "emp"), ("Industrial", "emp"), ("Community", "pop")];
/// `RULES.gtfs.bufferM`: the study area is widened by this much for stops (m).
pub const GTFS_BUFFER_M: f64 = 500.0;
/// `RULES.gtfs.linkM`: a stop snaps to a road within this distance (m).
pub const GTFS_LINK_M: f64 = 60.0;
/// `RULES.fleetDays`: how long a fleet mix is kept (days).
pub const FLEET_DAYS: f64 = 90.0;
/// `RULES.fleetTimeout`: the fleet query's least time limit (s).
pub const FLEET_TIMEOUT: f64 = 90.0;

/// `RULES` as the prototype's object (regular expressions of the count field guesses are written as their source text).
pub fn rules_value() -> Value {
    json!({
        "nslr": { "samples": NSLR_SAMPLES, "futureDay": NSLR_FUTURE_DAY, "source": "docs/spec/nslr-speeds (modal value of three samples along the link)", "verified": false },
        "periods": { "AM": PERIOD_AM, "PM": PERIOD_PM, "source": "Spare Construct assumption: two-hour peak windows", "verified": false },
        "control": { "siteM": CONTROL_SITE_M, "signM": CONTROL_SIGN_M, "tailM": CONTROL_TAIL_M, "pedCycle": CONTROL_PED_CYCLE, "pedCycleRange": CONTROL_PED_CYCLE_RANGE,
            "source": "docs/spec/auto-junction-control; distances are Spare Construct assumptions", "verified": false },
        "tms": { "minShare": TMS_MIN_SHARE, "medianDays": TMS_MEDIAN_DAYS, "aadtDays": TMS_AADT_DAYS, "termWeeks": TMS_TERM_WEEKS, "z": TMS_Z, "days": TMS_DAYS, "chunk": TMS_CHUNK, "maxPages": TMS_MAX_PAGES,
            "hcvRule": TMS_HCV_RULE, "source": "docs/spec/tms-daily; NZTA TMS", "verified": false },
        "ags": { "gatewayKm": AGS_GATEWAY_KM, "gatewayDeg": AGS_GATEWAY_DEG, "years": AGS_YEARS,
            "groups": AGS_GROUPS.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<String, Value>>(),
            "source": "docs/spec/ags23-growth (Spare Construct convention). The \u{b1}60\u{b0} sector beyond a gateway and population growth for community uses are Spare Construct assumptions", "verified": false },
        "gtfs": { "bufferM": GTFS_BUFFER_M, "linkM": GTFS_LINK_M, "source": "docs/spec/gtfs-bus; snapping a stop to a road within 60 m is a Spare Construct assumption", "verified": false },
        "counts": { "adt": ["/^adt$/i", "/aadt/i", "/average.*daily/i", "/^vpd$/i"], "peakVol": ["/peak.*(traffic|vol)/i", "/peak_?hour_?vol/i"], "peakTime": ["/peak_?hour$/i", "/peak_?time/i"],
            "hcv": ["/heavy|hcv|pc_?hv/i"], "date": ["/count_?date/i", "/^date$/i", "/survey_?date/i"], "name": ["/road_?name/i", "/^name$/i", "/street/i", "/location/i", "/site_?desc/i", "/description/i"],
            "dir": ["/^dir/i", "/direction/i"], "latest": ["/^latest$/i"], "year": "/^year_?(\\d{4})$/i", "source": "docs/spec/local-road-counts", "verified": false },
        "fleetDays": FLEET_DAYS, "fleetTimeout": FLEET_TIMEOUT,
    })
}

/// `TERMS.source`.
pub const TERMS_SOURCE: &str = "Ministry of Education school term dates, https://www.education.govt.nz/school/school-terms-and-holidays/ (as entered by Spare Construct; check before relying on them)";

/// `TERMS.dates`: NZ state school terms by year (start and end of each term, YYYY-MM-DD). Tuesday to Thursday calibration days must fall in term.
pub const TERM_DATES: [(u32, [(&str, &str); 4]); 5] = [
    (2022, [("2022-02-07", "2022-04-14"), ("2022-05-02", "2022-07-08"), ("2022-07-25", "2022-09-30"), ("2022-10-17", "2022-12-20")]),
    (2023, [("2023-02-07", "2023-04-06"), ("2023-04-24", "2023-06-30"), ("2023-07-17", "2023-09-22"), ("2023-10-09", "2023-12-20")]),
    (2024, [("2024-02-07", "2024-04-12"), ("2024-04-29", "2024-07-05"), ("2024-07-22", "2024-09-27"), ("2024-10-14", "2024-12-20")]),
    (2025, [("2025-02-07", "2025-04-11"), ("2025-04-28", "2025-07-04"), ("2025-07-21", "2025-09-26"), ("2025-10-13", "2025-12-19")]),
    (2026, [("2026-02-06", "2026-04-02"), ("2026-04-20", "2026-07-03"), ("2026-07-20", "2026-09-25"), ("2026-10-12", "2026-12-18")]),
];

/// `TERMS.dates[year]`.
pub fn term_dates(year: i64) -> Option<&'static [(&'static str, &'static str); 4]> {
    TERM_DATES.iter().find(|(y, _)| *y as i64 == year).map(|(_, t)| t)
}

/// `TERMS` as the prototype's object.
pub fn terms_value() -> Value {
    json!({
        "source": TERMS_SOURCE, "verified": false,
        "dates": TERM_DATES.iter().map(|(y, t)| (y.to_string(), json!(t.iter().map(|(s, e)| json!([s, e])).collect::<Vec<_>>()))).collect::<serde_json::Map<String, Value>>(),
    })
}

/// `MATARIKI.source`.
pub const MATARIKI_SOURCE: &str = "Te K\u{101}hui o Matariki Public Holiday Act 2022, Schedule 1";

/// `MATARIKI.dates`: the Matariki public holiday by year (MM-DD); later years are not entered.
pub const MATARIKI_DATES: [(u32, &str); 9] = [
    (2022, "06-24"),
    (2023, "07-14"),
    (2024, "06-28"),
    (2025, "06-20"),
    (2026, "07-10"),
    (2027, "06-25"),
    (2028, "07-14"),
    (2029, "07-06"),
    (2030, "06-21"),
];

/// `MATARIKI` as the prototype's object.
pub fn matariki_value() -> Value {
    json!({
        "source": MATARIKI_SOURCE, "verified": false,
        "dates": MATARIKI_DATES.iter().map(|(y, d)| (y.to_string(), json!(d))).collect::<serde_json::Map<String, Value>>(),
    })
}

/// `DEFAULTS` of the feeds settings as the prototype's object (timeout in seconds, nslrFuture, tmsDays, otherScenarios).
pub fn defaults_value() -> Value {
    json!({ "timeout": 30, "nslrFuture": true, "tmsDays": TMS_DAYS, "otherScenarios": false })
}
