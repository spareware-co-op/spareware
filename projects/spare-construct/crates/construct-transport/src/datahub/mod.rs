//! The count data of the prototype's data.js: loaded counts, what a count says about a peak hour, and how counts
//! are matched to model links.
//!
//! Port of the parts of `DataHub` that matrix estimation and the calibration panel use: the count records
//! (`P.data.counts`), the sources and observation methods with their matrix-estimation weights, `D()` (what the
//! project holds and which sources are switched on), `periodVol` (a count as a peak-hour volume), `obsMethod` and
//! `countWeight`, `inUse` with the 200 m rule that keeps RAMM estimates under real counts (`shadowed`), `matches`
//! (counts matched to links), the model-against-counts summary (`calibration`), the regression the criteria use
//! (`regression`), the journey-time check (`jtCheck`), gateway flows from counts (`gatewaysFromCounts`) and the
//! import of your own counts from a CSV file (`importCSV`).
//!
//! Network fetching (the ArcGIS loaders, TomTom, Google) and crash analysis stay outside: the downloaded counts are
//! input, as saved in `P.data.counts`. [`Data::read`] reads them from a project without changing it (the prototype's
//! `D()` also fills in the keys a project lacks; a read here does not; [`ensure_data`] does that when a caller needs it).
//!
//! Counts are read as loosely as the JavaScript reads them: every scalar is a [`Num`].

mod gateways;
mod importer;
mod matching;
mod stats;
mod volume;

pub use gateways::*;
pub use importer::*;
pub use matching::*;
pub use stats::*;
pub use volume::*;

use crate::jsnum::{JsNum, Maybe, Num};
use crate::project::{OrderedMap, Project};
use crate::util::dist;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::cell::OnceCell;
use std::rc::Rc;
use std::collections::HashSet;

/// One count source: its label, short name and map colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    /// Key ("nzta", "at", "user", "ramm").
    pub key: &'static str,
    /// Full name.
    pub label: &'static str,
    /// Short name.
    pub short: &'static str,
    /// Map colour.
    pub colour: &'static str,
}

/// `DataHub.SOURCES`, in its order.
pub const SOURCES: [Source; 4] = [
    Source {
        key: "nzta",
        label: "NZTA state highway counts",
        short: "NZTA",
        colour: "#7048e8",
    },
    Source {
        key: "at",
        label: "Auckland Transport counts",
        short: "AT",
        colour: "#0c8599",
    },
    Source {
        key: "user",
        label: "Your counts",
        short: "Yours",
        colour: "#e8590c",
    },
    Source {
        key: "ramm",
        label: "NZTA road centreline estimates (RAMM)",
        short: "RAMM",
        colour: "#868e96",
    },
];

/// `DataHub.SOURCES[key]`.
pub fn source(key: &str) -> Option<&'static Source> {
    SOURCES.iter().find(|s| s.key == key)
}

/// One count observation method with its matrix-estimation weight (`DataHub.OBS_METHODS`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObsMethod {
    /// Key ("measured_hour", "weekday_factored", "adt_factored", "ramm_estimate", "virtual").
    pub key: &'static str,
    /// Display name.
    pub label: &'static str,
    /// Weight in matrix estimation.
    pub weight: f64,
    /// False: never in validation statistics.
    pub validation: bool,
    /// RAMM estimates: an estimate within this distance (m) of an NZTA or AT count is not used.
    pub override_m: Option<f64>,
}

/// `DataHub.OBS_METHODS`, in its order (weights from docs/spec/count-roles, a Spare Construct suggestion; RAMM estimates from docs/spec/local-road-counts).
pub const OBS_METHODS: [ObsMethod; 5] = [
    ObsMethod {
        key: "measured_hour",
        label: "Measured peak hour",
        weight: 1.0,
        validation: true,
        override_m: None,
    },
    ObsMethod {
        key: "weekday_factored",
        label: "TMS weekday mean \u{d7} peak share",
        weight: 0.7,
        validation: true,
        override_m: None,
    },
    ObsMethod {
        key: "adt_factored",
        label: "ADT or AADT \u{d7} peak share",
        weight: 0.4,
        validation: true,
        override_m: None,
    },
    ObsMethod {
        key: "ramm_estimate",
        label: "RAMM centreline estimate (NZTA RCA data)",
        weight: 0.25,
        validation: false,
        override_m: Some(200.0),
    },
    ObsMethod {
        key: "virtual",
        label: "NZTA estimate (virtual site)",
        weight: 0.0,
        validation: false,
        override_m: None,
    },
];

/// Where the observation-method weights come from (`OBS_METHODS.source`).
pub const OBS_METHODS_SOURCE: &str = "docs/spec/count-roles (weights: Spare Construct suggestion); docs/spec/local-road-counts (RAMM estimates)";

/// `DataHub.OBS_METHODS[key]` for the five methods (the prototype's object also holds a `source` text and a `verified` flag, which are not methods).
pub fn obs_method_of_key(key: &str) -> Option<&'static ObsMethod> {
    OBS_METHODS.iter().find(|m| m.key == key)
}

/// `DIR_BEARING[dir]`: the heading of a count direction (NB 0, EB 90, SB 180, WB 270); NaN for anything else (undefined in the prototype).
pub fn dir_bearing(dir: &str) -> f64 {
    match dir {
        "NB" => 0.0,
        "EB" => 90.0,
        "SB" => 180.0,
        "WB" => 270.0,
        _ => f64::NAN,
    }
}

/// True when the value is the text `s` (`x === s` for a value that may be anything).
pub fn is_text(x: &Num, s: &str) -> bool {
    matches!(x, Maybe::Set(JsNum::Text(t)) if t == s)
}

/// `x || y` read as text (`String(x || y || '')`): the first of the two that is truthy, as the prototype's text, else "".
pub fn or_text(a: &Num, b: &Num) -> String {
    if a.truthy() {
        a.js_string()
    } else if b.truthy() {
        b.js_string()
    } else {
        String::new()
    }
}

/// One traffic count (an entry of `P.data.counts`): an NZTA or AT count, a user count or a RAMM estimate.
///
/// Every field the model reads is kept as the file held it; keys not named here are kept in `extra`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Count {
    /// Count id ("nzta:...", "at:...", "user:...").
    #[serde(default)]
    pub id: String,
    /// Source: "nzta", "at", "user" or "ramm".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub src: Num,
    /// Source reference.
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "ref")]
    pub reference: Num,
    /// Name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Num,
    /// Road name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub road: Num,
    /// Latitude.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lat: Num,
    /// Longitude.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lng: Num,
    /// Daily traffic (AADT or ADT).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub adt: Num,
    /// Heavy vehicles (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub hv: Num,
    /// Direction: "NB", "EB", "SB" or "WB".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub dir: Num,
    /// The count is for one direction.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub directional: Num,
    /// An NZTA virtual site (an estimate, not a count).
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "virtual")]
    pub virtual_: Num,
    /// A ramp site.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub ramp: Num,
    /// Switched off by the user.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub off: Num,
    /// User count: AM peak hour (veh/h).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub am: Num,
    /// User count: PM peak hour (veh/h).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub pm: Num,
    /// AT count: peak-hour flow (veh/h).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub peak: Num,
    /// AT count: start of the peak hour ("07:45").
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub peak_time: Num,
    /// "ramm_estimate" for a RAMM centreline estimate.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub kind: Num,
    /// An importer's own observation method.
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "obs_method")]
    pub obs_method: Num,
    /// NZTA TMS Tue to Thu term-time mean: { target: { mean, ok, ci95, n } }.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub tms: Maybe<Value>,
    /// The site's own peak shares of daily traffic by period ({ AM, PM }, fractions).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub share: Maybe<Value>,
    /// Role mirrored from the calibration panel.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub role: Num,
    /// Count date.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub date: Num,
    /// Note.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub note: Num,
    /// Number of counted days.
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "n_days")]
    pub n_days: Num,
    /// 95% confidence interval.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub ci95: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Count {
    /// A count with only an id and a source; the rest unset (for building counts in code).
    pub fn new(id: &str, src: &str) -> Count {
        let blank: Count = serde_json::from_value(Value::Object(Map::new())).expect("an empty count reads");
        Count {
            id: id.to_string(),
            src: Maybe::Set(JsNum::Text(src.to_string())),
            ..blank
        }
    }
    /// `c.src === s`.
    pub fn src_is(&self, s: &str) -> bool {
        is_text(&self.src, s)
    }
    /// `[c.lat, c.lng]` as the arithmetic reads them.
    pub fn ll(&self) -> [f64; 2] {
        [self.lat.num(), self.lng.num()]
    }
}

/// `DataHub.isEstimate(c)`: a RAMM centreline estimate (`obs_method`, `kind` or `src` says so).
pub fn is_estimate(c: &Count) -> bool {
    is_text(&c.obs_method, "ramm_estimate") || is_text(&c.kind, "ramm_estimate") || c.src_is("ramm")
}

/// A journey-time check (an entry of `P.data.jt`): two nodes and the observed or predicted times.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JourneyCheck {
    /// Id.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub id: Num,
    /// Name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Num,
    /// Start node id.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub from: Num,
    /// End node id.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub to: Num,
    /// Observed times by period ({ AM, PM, src }, seconds).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub obs: Maybe<Value>,
    /// Google typical times by period ({ AM: { t, when }, PM }).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub g: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// What a project holds as data (`D()`): counts, which sources are switched on, load records and journey checks.
///
/// A snapshot: read it with [`Data::read`], use it for any number of questions, read again after the project's
/// counts change.
#[derive(Debug)]
pub struct Data {
    /// `P.data.counts`, in order.
    pub counts: Vec<Rc<Count>>,
    /// `D().use`: the defaults (nzta, at, user and ramm on, virtual off) overridden by the project's own switches.
    pub use_: OrderedMap<Num>,
    /// `P.data.loaded`: when each source was loaded.
    pub loaded: OrderedMap<Value>,
    /// `P.data.jt`: journey-time checks.
    pub jt: Vec<JourneyCheck>,
    shadow: OnceCell<HashSet<String>>,
}

impl Data {
    /// Reads the project's data the way `D()` shows it (missing keys read as empty; nothing is written to the project).
    pub fn read(p: &Project) -> Data {
        let d = p.data.get();
        let extra = d.map(|d| &d.extra);
        let counts = match extra.and_then(|e| e.get("counts")) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|v| serde_json::from_value::<Count>(v.clone()).ok().map(Rc::new))
                .collect(),
            _ => Vec::new(),
        };
        let mut use_: OrderedMap<Num> = [
            ("nzta", true),
            ("at", true),
            ("user", true),
            ("virtual", false),
            ("ramm", true),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), Num::from(v)))
        .collect();
        if let Some(Value::Object(u)) = extra.and_then(|e| e.get("use")) {
            for (k, v) in u {
                use_.insert(k.clone(), value_num(v));
            }
        }
        let loaded = d.and_then(|d| d.loaded.get()).cloned().unwrap_or_default();
        let jt = match extra.and_then(|e| e.get("jt")) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|v| serde_json::from_value::<JourneyCheck>(v.clone()).ok())
                .collect(),
            _ => Vec::new(),
        };
        Data {
            counts,
            use_,
            loaded,
            jt,
            shadow: OnceCell::new(),
        }
    }
    /// A data set of these counts with the default switches (for building inputs in code).
    pub fn with_counts(counts: Vec<Count>) -> Data {
        let mut p = crate::project::new_project("p", "p", "s");
        let mut extra = Map::new();
        extra.insert(
            "counts".to_string(),
            serde_json::to_value(&counts).expect("counts serialise"),
        );
        p.data = Maybe::Set(crate::project::ProjectData {
            loaded: Maybe::Missing,
            msm: Maybe::Missing,
            controlled: Maybe::Missing,
            extra,
        });
        Data::read(&p)
    }
    /// `D().use[key]`: a source's switch (missing reads as off).
    pub fn use_of(&self, key: &str) -> &Num {
        self.use_.num_at(key)
    }
    /// The vintage time of a source (`(d.loaded[key] || {}).at`), as text for a cache key; not used by the model itself.
    pub fn loaded_at(&self, key: &str) -> Option<&Value> {
        self.loaded.get(key).and_then(|v| v.get("at"))
    }
    /// `DataHub.shadowed(c)`: a RAMM estimate within 200 m of an NZTA or AT count that is switched on (never an estimate, never virtual).
    pub fn shadowed(&self, c: &Count) -> bool {
        if !is_estimate(c) {
            return false;
        }
        self.shadow
            .get_or_init(|| {
                let real: Vec<&Count> = self
                    .counts
                    .iter()
                    .filter(|x| {
                        (x.src_is("nzta") || x.src_is("at"))
                            && !x.off.truthy()
                            && !x.virtual_.truthy()
                            && !is_estimate(x)
                            && x.lat.num().is_finite()
                            && x.lng.num().is_finite()
                    })
                    .map(|x| x.as_ref())
                    .collect();
                let r = obs_method_of_key("ramm_estimate")
                    .and_then(|m| m.override_m)
                    .unwrap_or(0.0);
                let mut set = HashSet::new();
                for e in &self.counts {
                    if is_estimate(e)
                        && real.iter().any(|x| {
                            (x.lat.num() - e.lat.num()).abs() < 0.003
                                && (x.lng.num() - e.lng.num()).abs() < 0.004
                                && dist(&x.ll(), &e.ll()) <= r
                        })
                    {
                        set.insert(e.id.clone());
                    }
                }
                set
            })
            .contains(&c.id)
    }
    /// `DataHub.inUse(c)`: switched on (not off, its source on, virtual sites only when virtual is on, RAMM estimates only when ramm is not off and no real count is within 200 m).
    pub fn in_use(&self, c: &Count) -> bool {
        let u = |k: &str| self.use_of(k);
        // u[c.src]: the key is String(c.src)
        let src_on = u(&c.src.js_string()).truthy();
        !c.off.truthy()
            && src_on
            && (!c.virtual_.truthy() || u("virtual").truthy())
            && !(is_estimate(c) && (u("ramm").is_false() || self.shadowed(c)))
    }
}

/// A saved JSON value as a [`Num`]: null is `Maybe::Null`, anything else a set value.
pub fn value_num(v: &Value) -> Num {
    match v {
        Value::Null => Maybe::Null,
        other => Maybe::Set(JsNum::from(other.clone())),
    }
}

/// `DataHub.validationOK(c)`: a count that may stand in validation statistics: not virtual, not a RAMM estimate, and its own method (if it names one) is not barred from validation.
pub fn validation_ok(c: &Count) -> bool {
    let method_ok = match c.obs_method.as_text() {
        Some(m) if c.obs_method.truthy() => match obs_method_of_key(m) {
            Some(om) => om.validation,
            None => true,
        },
        _ => true,
    };
    !c.virtual_.truthy() && !is_estimate(c) && method_ok
}

/// Reading a text value out of a [`Num`].
pub trait AsText {
    /// The text, when the value is text.
    fn as_text(&self) -> Option<&str>;
}

impl AsText for Num {
    fn as_text(&self) -> Option<&str> {
        match self {
            Maybe::Set(JsNum::Text(t)) => Some(t),
            _ => None,
        }
    }
}
