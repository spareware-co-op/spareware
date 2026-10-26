//! The saved project, typed: the data model of the prototype's model.js.
//!
//! A project holds settings (defaultSettings, settings version 3), scenarios (each with
//! nodes, links, zones and gateways, bus routes, junction sites, a manual OD matrix and
//! results), and the project-wide records the engine reads: the committed-development
//! register, loaded data, matrix estimation factors, land-use factors and validation.
//! Inputs are projects after the prototype's `migrate()`, as tools/transport-truth/inputs
//! holds them; this module does not port `migrate()` itself.
//!
//! Reading is as loose as the JavaScript: every number, flag or scalar is a [`Num`] (any
//! JSON value, read through `+x`, `x | 0`, `!!x` or `x === true` by the engine), keys keep
//! "missing" apart from null ([`Maybe`]), and keys this model does not name are kept in
//! each object's `extra`, so a project read and written back is the same JSON value.
//! Objects keyed by id use [`OrderedMap`], which keeps the file's key order: the engine
//! iterates them in that order (`Object.values(sc.links)`), and results depend on it.

use crate::jsnum::{Maybe, Num, json_number};
use crate::util::{LatLng, poly_len, r7};
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize as DeriveDeserialize, Serialize as DeriveSerialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fmt;

/* ---------------- ordered map ---------------- */

/// A JSON object keyed by text that keeps insertion order, as a JavaScript object does.
///
/// Iteration (`iter`, `keys`, `values`) follows the order keys were first inserted, which
/// for a loaded project is the order in the file. Setting an existing key keeps its place;
/// removing a key and adding it again puts it last (as `delete` then assignment does in
/// JavaScript). Reading JSON with a repeated key keeps the last value at the first key's
/// place, as `JSON.parse` does.
///
/// Read projects from JSON text (`Project::from_json`, `serde_json::from_str` or `from_reader`):
/// a `serde_json::Value` object is sorted by key (this workspace has no `preserve_order`), so
/// deserializing from a `Value` loses the file's order.
///
/// Assumption: keys are not integer-like. JavaScript lists keys such as "12" (array
/// indices, 0 to 2^32 - 2) before all others, in ascending order; this map does not. The
/// prototype's ids are generated with a letter prefix ("n", "l", "z", "s", "j", "r"), so
/// saved projects never hold integer-like ids.
#[derive(Clone)]
pub struct OrderedMap<V> {
    entries: Vec<(String, V)>,
    index: HashMap<String, usize>,
}

impl<V> OrderedMap<V> {
    /// An empty map.
    pub fn new() -> Self {
        OrderedMap {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }
    /// Number of keys.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// True when there are no keys.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// `obj[key]`.
    pub fn get(&self, key: &str) -> Option<&V> {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }
    /// `obj[key]`, for changing.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        match self.index.get(key) {
            Some(&i) => Some(&mut self.entries[i].1),
            None => None,
        }
    }
    /// `key in obj` (own keys only).
    pub fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }
    /// Position of a key in iteration order.
    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.index.get(key).copied()
    }
    /// The entry at a position in iteration order.
    pub fn get_index(&self, i: usize) -> Option<(&str, &V)> {
        self.entries.get(i).map(|(k, v)| (k.as_str(), v))
    }
    /// `obj[key] = value`: a new key goes last, an existing key keeps its place; returns the old value.
    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<V> {
        let key = key.into();
        if let Some(&i) = self.index.get(&key) {
            return Some(std::mem::replace(&mut self.entries[i].1, value));
        }
        self.index.insert(key.clone(), self.entries.len());
        self.entries.push((key, value));
        None
    }
    /// `delete obj[key]`: later keys move up one place; returns the value.
    pub fn remove(&mut self, key: &str) -> Option<V> {
        let i = self.index.remove(key)?;
        let (_, v) = self.entries.remove(i);
        for (k, _) in &self.entries[i..] {
            if let Some(p) = self.index.get_mut(k) {
                *p -= 1;
            }
        }
        Some(v)
    }
    /// Keeps only the entries for which f is true, in order.
    pub fn retain(&mut self, mut f: impl FnMut(&str, &mut V) -> bool) {
        self.entries.retain_mut(|(k, v)| f(k, v));
        self.reindex();
    }
    fn reindex(&mut self) {
        self.index.clear();
        for (i, (k, _)) in self.entries.iter().enumerate() {
            self.index.insert(k.clone(), i);
        }
    }
    /// `Object.entries(obj)`, in order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&str, &V)> + ExactSizeIterator {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }
    /// `Object.entries(obj)`, in order, with values for changing.
    pub fn iter_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (&str, &mut V)> + ExactSizeIterator {
        self.entries.iter_mut().map(|(k, v)| (k.as_str(), v))
    }
    /// `Object.keys(obj)`, in order.
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &str> + ExactSizeIterator {
        self.entries.iter().map(|(k, _)| k.as_str())
    }
    /// `Object.values(obj)`, in order.
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator {
        self.entries.iter().map(|(_, v)| v)
    }
    /// `Object.values(obj)`, in order, for changing.
    pub fn values_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut V> + ExactSizeIterator {
        self.entries.iter_mut().map(|(_, v)| v)
    }
}

impl OrderedMap<Num> {
    /// `obj[key]` as a [`Num`]: missing when the key is absent.
    pub fn num_at(&self, key: &str) -> &Num {
        self.get(key).unwrap_or(&crate::jsnum::MISSING)
    }
}

impl<V> Default for OrderedMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: PartialEq> PartialEq for OrderedMap<V> {
    // Same keys, values and order
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<V: fmt::Debug> fmt::Debug for OrderedMap<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

impl<V> FromIterator<(String, V)> for OrderedMap<V> {
    fn from_iter<I: IntoIterator<Item = (String, V)>>(iter: I) -> Self {
        let mut m = OrderedMap::new();
        for (k, v) in iter {
            m.insert(k, v);
        }
        m
    }
}

impl<V> IntoIterator for OrderedMap<V> {
    type Item = (String, V);
    type IntoIter = std::vec::IntoIter<(String, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a, V> IntoIterator for &'a OrderedMap<V> {
    type Item = (&'a str, &'a V);
    type IntoIter =
        std::iter::Map<std::slice::Iter<'a, (String, V)>, fn(&'a (String, V)) -> (&'a str, &'a V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }
}

impl<V> std::ops::Index<&str> for OrderedMap<V> {
    type Output = V;
    fn index(&self, key: &str) -> &V {
        self.get(key)
            .unwrap_or_else(|| panic!("no key {key:?} in the map"))
    }
}

impl<V: Serialize> Serialize for OrderedMap<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.entries.len()))?;
        for (k, v) in &self.entries {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for OrderedMap<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V2<V>(std::marker::PhantomData<V>);
        impl<'de, V: Deserialize<'de>> Visitor<'de> for V2<V> {
            type Value = OrderedMap<V>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut m = OrderedMap::new();
                while let Some((k, v)) = a.next_entry::<String, V>()? {
                    m.insert(k, v);
                }
                Ok(m)
            }
        }
        d.deserialize_map(V2(std::marker::PhantomData))
    }
}

/* ---------------- points ---------------- */

/// A stored [lat, lng] pair (link shape points), written as `JSON.stringify` writes numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point(pub LatLng);

impl Serialize for Point {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut q = s.serialize_seq(Some(2))?;
        q.serialize_element(&json_number(self.0[0]))?;
        q.serialize_element(&json_number(self.0[1]))?;
        q.end()
    }
}

impl<'de> Deserialize<'de> for Point {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PV;
        impl<'de> Visitor<'de> for PV {
            type Value = Point;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a [lat, lng] pair of numbers")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Point, A::Error> {
                use serde::de::Error;
                let lat: f64 = a
                    .next_element()?
                    .ok_or_else(|| A::Error::invalid_length(0, &self))?;
                let lng: f64 = a
                    .next_element()?
                    .ok_or_else(|| A::Error::invalid_length(1, &self))?;
                if a.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(A::Error::invalid_length(3, &self));
                }
                Ok(Point([lat, lng]))
            }
        }
        d.deserialize_seq(PV)
    }
}

/* ---------------- road classes and land uses ---------------- */

/// A road class of model.js CLASSES: mid-block capacity per lane (pcu/h), default speed (km/h), lanes each way, lane width (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadClass {
    /// Key ("arterial").
    pub key: &'static str,
    /// Display name.
    pub label: &'static str,
    /// Rank: 6 motorway down to 2 local.
    pub rank: f64,
    /// Default speed limit (km/h).
    pub speed: f64,
    /// Default lanes each way.
    pub lanes: f64,
    /// Mid-block capacity per lane (pcu/h).
    pub cap: f64,
    /// Lane width (m).
    pub lw: f64,
    /// Minimum drawn width (px).
    pub min_px: f64,
    /// Map colour.
    pub color: &'static str,
}

/// model.js CLASSES, in its order.
pub const CLASSES: [RoadClass; 5] = [
    RoadClass {
        key: "motorway",
        label: "Motorway",
        rank: 6.0,
        speed: 100.0,
        lanes: 2.0,
        cap: 2000.0,
        lw: 3.5,
        min_px: 4.5,
        color: "#3d5ccc",
    },
    RoadClass {
        key: "ramp",
        label: "Ramp",
        rank: 5.0,
        speed: 70.0,
        lanes: 1.0,
        cap: 1700.0,
        lw: 3.5,
        min_px: 3.0,
        color: "#7b93e8",
    },
    RoadClass {
        key: "arterial",
        label: "Arterial",
        rank: 4.0,
        speed: 50.0,
        lanes: 2.0,
        cap: 1200.0,
        lw: 3.5,
        min_px: 3.6,
        color: "#d4501a",
    },
    RoadClass {
        key: "collector",
        label: "Collector",
        rank: 3.0,
        speed: 50.0,
        lanes: 1.0,
        cap: 1000.0,
        lw: 3.3,
        min_px: 2.6,
        color: "#12858a",
    },
    RoadClass {
        key: "local",
        label: "Local",
        rank: 2.0,
        speed: 50.0,
        lanes: 1.0,
        cap: 800.0,
        lw: 3.0,
        min_px: 1.7,
        color: "#8a9490",
    },
];

/// `CLASSES[cls]`: the class with this key, if any.
pub fn road_class_exact(cls: &str) -> Option<&'static RoadClass> {
    CLASSES.iter().find(|c| c.key == cls)
}

/// `CLASSES[cls] || CLASSES.local`: the class with this key, otherwise local.
pub fn road_class(cls: Option<&str>) -> &'static RoadClass {
    cls.and_then(road_class_exact).unwrap_or(&CLASSES[4])
}

/// A land use of model.js LANDUSES: trip rates are per `per` units of `unit`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandUse {
    /// Key ("residential").
    pub key: &'static str,
    /// Display name.
    pub label: &'static str,
    /// Unit of quantity ("dwellings", "m² GFA").
    pub unit: &'static str,
    /// Units per trip-rate unit (1, or 100 for m² GFA).
    pub per: f64,
    /// Display group.
    pub group: &'static str,
}

const fn lu(
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    per: f64,
    group: &'static str,
) -> LandUse {
    LandUse {
        key,
        label,
        unit,
        per,
        group,
    }
}

/// model.js LANDUSES, in its order (the engine sums trip ends in this order).
pub const LANDUSES: [LandUse; 14] = [
    lu("residential", "Houses", "dwellings", 1.0, "Residential"),
    lu("apartment", "Apartments", "dwellings", 1.0, "Residential"),
    lu(
        "retirement",
        "Retirement village",
        "units",
        1.0,
        "Residential",
    ),
    lu("retail", "Retail", "m\u{b2} GFA", 100.0, "Commercial"),
    lu(
        "supermarket",
        "Supermarket",
        "m\u{b2} GFA",
        100.0,
        "Commercial",
    ),
    lu("fastfood", "Fast food", "m\u{b2} GFA", 100.0, "Commercial"),
    lu("office", "Office", "m\u{b2} GFA", 100.0, "Commercial"),
    lu("hotel", "Hotel", "rooms", 1.0, "Commercial"),
    lu("school", "School", "students", 1.0, "Community"),
    lu("childcare", "Childcare", "child places", 1.0, "Community"),
    lu(
        "medical",
        "Medical centre",
        "m\u{b2} GFA",
        100.0,
        "Community",
    ),
    lu(
        "tertiary",
        "Tertiary education",
        "students",
        1.0,
        "Community",
    ),
    lu(
        "industrial",
        "Industrial",
        "m\u{b2} GFA",
        100.0,
        "Industrial",
    ),
    lu(
        "warehouse",
        "Warehouse and logistics",
        "m\u{b2} GFA",
        100.0,
        "Industrial",
    ),
];

/// `LANDUSES[key]`.
pub fn land_use(key: &str) -> Option<&'static LandUse> {
    LANDUSES.iter().find(|l| l.key == key)
}

/* ---------------- settings ---------------- */

/// Project settings (`P.settings`, model.js defaultSettings): every key the engine reads, unknown keys in `extra`.
///
/// Numbers are read the JavaScript way: `+S.growth || 0` is `growth.or(0.0)`, `S.hvPct / 100`
/// is `hv_pct.num() / 100.0`. [`default_settings`] gives the values a new project starts with.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Settings version (3 = current; 1 and 2 keep older defaults, see model.js SETTINGS_V2).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub sv: Num,
    /// Peak period shown: "AM" or "PM".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub period: Maybe<String>,
    /// Base (count) year.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub base_year: Num,
    /// Project year (a scenario without its own year runs in it).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub year: Num,
    /// Background traffic growth (% a year).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub growth: Num,
    /// "linear" (MBCM arithmetic growth) or "compound".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub growth_mode: Maybe<String>,
    /// Heavy vehicles (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub hv_pct: Num,
    /// Passenger car units per heavy vehicle.
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "pcuHV")]
    pub pcu_hv: Num,
    /// Daily traffic as a multiple of the peak hour.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub peak_to_daily: Num,
    /// Passenger car units per bus on bus-route preloads.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub bus_pcu: Num,
    /// BPR alpha.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub bpr_alpha: Num,
    /// BPR beta.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub bpr_beta: Num,
    /// Saturation flow per lane at signals (pcu/h).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub sat_flow: Num,
    /// Default signal cycle (s).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub cycle: Num,
    /// Lost time per signal phase (s).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lost_per_phase: Num,
    /// Optimise signal timings to the assigned flows.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub signal_opt: Num,
    /// Maximum assignment iterations.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub max_iter: Num,
    /// Convergence target (relative gap).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub gap_tol: Num,
    /// Convergence rule (TAG stability, MSA capacity damping).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub conv: Maybe<ConvSettings>,
    /// Junction analysis (LoS method, peak flow factor, analysis period, queues, capacity method).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub junction: Maybe<JunctionSettings>,
    /// Model status: TMDG category and justification.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub tmdg: Maybe<TmdgSettings>,
    /// Assessment scenarios.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub assess: Maybe<AssessSettings>,
    /// Gravity model deterrence (per minute).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub gravity_beta: Num,
    /// Zone connector speed (km/h).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub connector_speed: Num,
    /// Connectors per zone.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub connectors: Num,
    /// Peak-hour trip rates: land use, then period ("AM", "PM").
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub rates: Maybe<OrderedMap<Maybe<PeriodRates>>>,
    /// Turn penalties.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub turns: Maybe<TurnSettings>,
    /// Economics.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub econ: Maybe<EconSettings>,
    /// Microsimulation display.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub sim: Maybe<SimSettings>,
    /// Count data.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub data: Maybe<DataSettings>,
    /// LINZ basemap key (interface only).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub linz_key: Maybe<String>,
    /// Basemap (interface only).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub basemap: Maybe<String>,
    /// Calibration criteria (`calib.standard`: "tmdg" by default); not in new projects.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub calib: Maybe<CalibSettings>,
    /// Fleet-mix multiplier on light-vehicle CO2 (optional; absent or invalid = 1).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub co2_factor: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `settings.conv`: the TAG M3.1 stability rule and capacity damping.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvSettings {
    /// Stop only when link flows are stable (`stable !== false`).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub stable: Num,
    /// Flow change counted as stable (fraction).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub chg: Num,
    /// Share of links that must be stable.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub share: Num,
    /// Consecutive stable iterations.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub consec: Num,
    /// "msa" (damped give-way and roundabout capacities) or "none".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub cap_damp: Maybe<String>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `settings.junction`: junction analysis.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct JunctionSettings {
    /// Level of service method: "hcm6", "sidra" or "rta".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub los_method: Maybe<String>,
    /// Peak flow factor.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub pff: Num,
    /// Analysis period for overflow delay and queues (h).
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "T")]
    pub t: Num,
    /// Queue space per vehicle (m).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub queue_space: Num,
    /// 95th-percentile queue factor.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub f95: Num,
    /// Capacity method: "arc" (default) or "movement".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub cap_method: Maybe<String>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `settings.tmdg`: the TMDG model category chosen for the study and its justification.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
pub struct TmdgSettings {
    /// Category ("E").
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub category: Num,
    /// The author's justification.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub justification: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `settings.assess`: assessment scenarios.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessSettings {
    /// Share of an MSM 2-hour period in the peak hour.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub msm_peak_share: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Trip rates of one land use by period ("AM", "PM").
pub type PeriodRates = OrderedMap<Maybe<TripRate>>;

/// One trip rate: vehicle trips per land-use unit in the peak hour and the share arriving.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct TripRate {
    /// Vehicle trips per unit (per dwelling, per 100 m² GFA, ...).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub rate: Num,
    /// Share of the trips arriving (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub in_pct: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `settings.turns`: turn penalties (s); `uturn` is "ban" or a penalty (a number, or a number as text).
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSettings {
    /// Left turn (s).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub left: Num,
    /// Right turn (s).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub right: Num,
    /// Right turn at signals (s).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub right_signals: Num,
    /// U-turns: "ban" or a penalty (s).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub uturn: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `settings.econ`: indicative economics.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconSettings {
    /// "mbcm" (MBCM v1.7.5, GC 25/01 discounting) or "uniform" (older projects).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub method: Maybe<String>,
    /// "non-commercial" or "commercial".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub activity: Maybe<String>,
    /// Value of time (NZ$/h).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub vot: Num,
    /// True when vot is at the MBCM base date and needs the July 2025 update factor.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub vot_update: Num,
    /// Equivalent hours a year (uniform method).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub annual_hours: Num,
    /// Equivalent hours a year per modelled period ("AM", "PM", "IP").
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub period_hours: Maybe<OrderedMap<Num>>,
    /// Discount schedule: an array of { to, r } (to null = onwards), or a single rate (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub rates: Maybe<Value>,
    /// Sensitivity discount rate (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub sens_rate: Num,
    /// Discount rate of the uniform method (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub discount: Num,
    /// Analysis period (years).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub years: Num,
    /// Justification for more than 40 years.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub long_life_note: Num,
    /// NZTA's written approval for more than 60 years.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub approval: Num,
    /// Benefit growth (% a year; null = the background growth rate).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub benefit_growth: Num,
    /// Use 95th-percentile values.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub p95: Num,
    /// Capital cost per lane-km by road class (NZ$m).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lane_km: Maybe<OrderedMap<Num>>,
    /// Capital cost per new or upgraded junction by control (NZ$m).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub node: Maybe<OrderedMap<Num>>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `settings.sim`: microsimulation display.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimSettings {
    /// Demand scale.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub scale: Num,
    /// Playback speed.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub speed: Num,
    /// Maximum vehicles.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub max_veh: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `settings.data`: count data.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSettings {
    /// AM peak hour as a share of daily traffic (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub am_share: Num,
    /// PM peak hour as a share of daily traffic (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub pm_share: Num,
    /// Count-to-link match radius (m).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub match_radius: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `settings.calib`: calibration criteria.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
pub struct CalibSettings {
    /// "tmdg", "tag", "rms" or "atap".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub standard: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// model.js SV: the current settings version.
pub const SETTINGS_VERSION: f64 = 3.0;

fn n(x: f64) -> Num {
    Num::number(x)
}

fn s(t: &str) -> Maybe<String> {
    Maybe::Set(t.to_string())
}

fn txt(t: &str) -> Num {
    Maybe::Set(crate::jsnum::JsNum::Text(t.to_string()))
}

fn rate_pair(am: (f64, f64), pm: (f64, f64)) -> Maybe<PeriodRates> {
    let r = |(rate, in_pct): (f64, f64)| {
        Maybe::Set(TripRate {
            rate: n(rate),
            in_pct: n(in_pct),
            extra: Map::new(),
        })
    };
    Maybe::Set(
        [("AM".to_string(), r(am)), ("PM".to_string(), r(pm))]
            .into_iter()
            .collect(),
    )
}

fn num_map(items: &[(&str, f64)]) -> Maybe<OrderedMap<Num>> {
    Maybe::Set(items.iter().map(|&(k, v)| (k.to_string(), n(v))).collect())
}

/// model.js `defaultSettings()`: the settings of a new project, every nested object included, keys in its order.
pub fn default_settings() -> Settings {
    let rates: OrderedMap<Maybe<PeriodRates>> = [
        ("residential", (0.85, 20.0), (0.95, 65.0)),
        ("apartment", (0.45, 20.0), (0.50, 60.0)),
        ("retail", (1.5, 60.0), (4.5, 50.0)),
        ("office", (2.0, 85.0), (1.8, 15.0)),
        ("industrial", (0.6, 70.0), (0.6, 30.0)),
        ("school", (0.45, 50.0), (0.05, 50.0)),
        ("supermarket", (3.0, 60.0), (9.0, 50.0)),
        ("childcare", (0.8, 55.0), (0.7, 45.0)),
        ("medical", (2.5, 75.0), (3.0, 35.0)),
        ("retirement", (0.2, 40.0), (0.3, 55.0)),
        ("hotel", (0.4, 30.0), (0.4, 60.0)),
        ("warehouse", (0.5, 65.0), (0.5, 35.0)),
        ("fastfood", (4.0, 50.0), (12.0, 50.0)),
        ("tertiary", (0.25, 85.0), (0.2, 20.0)),
    ]
    .into_iter()
    .map(|(k, am, pm)| (k.to_string(), rate_pair(am, pm)))
    .collect();
    Settings {
        sv: n(SETTINGS_VERSION),
        period: s("AM"),
        base_year: n(2026.0),
        year: n(2026.0),
        growth: n(1.5),
        growth_mode: s("linear"),
        hv_pct: n(6.0),
        pcu_hv: n(2.3),
        peak_to_daily: n(10.0),
        bus_pcu: n(2.0),
        bpr_alpha: n(0.15),
        bpr_beta: n(4.0),
        sat_flow: n(1800.0),
        cycle: n(100.0),
        lost_per_phase: n(5.0),
        signal_opt: true.into(),
        max_iter: n(200.0),
        gap_tol: n(0.0001),
        conv: Maybe::Set(ConvSettings {
            stable: true.into(),
            chg: n(0.01),
            share: n(0.98),
            consec: n(4.0),
            cap_damp: s("msa"),
            extra: Map::new(),
        }),
        junction: Maybe::Set(JunctionSettings {
            los_method: s("hcm6"),
            pff: n(0.95),
            t: n(0.5),
            queue_space: n(7.0),
            f95: n(1.6),
            cap_method: s("arc"),
            extra: Map::new(),
        }),
        tmdg: Maybe::Set(TmdgSettings {
            category: txt("E"),
            justification: txt(""),
            extra: Map::new(),
        }),
        assess: Maybe::Set(AssessSettings {
            msm_peak_share: n(0.55),
            extra: Map::new(),
        }),
        gravity_beta: n(0.10),
        connector_speed: n(25.0),
        connectors: n(2.0),
        rates: Maybe::Set(rates),
        turns: Maybe::Set(TurnSettings {
            left: n(2.0),
            right: n(6.0),
            right_signals: n(4.0),
            uturn: txt("ban"),
            extra: Map::new(),
        }),
        econ: Maybe::Set(EconSettings {
            method: s("mbcm"),
            activity: s("non-commercial"),
            vot: n(32.0),
            vot_update: false.into(),
            annual_hours: n(1200.0),
            period_hours: num_map(&[("AM", 600.0), ("PM", 600.0), ("IP", 0.0)]),
            rates: Maybe::Set(
                serde_json::json!([{ "to": 30, "r": 2 }, { "to": 100, "r": 1.5 }, { "to": null, "r": 1 }]),
            ),
            sens_rate: n(8.0),
            discount: n(2.0),
            years: n(40.0),
            long_life_note: txt(""),
            approval: txt(""),
            benefit_growth: Maybe::Null,
            p95: false.into(),
            lane_km: num_map(&[
                ("motorway", 12.0),
                ("ramp", 8.0),
                ("arterial", 5.0),
                ("collector", 3.5),
                ("local", 2.5),
            ]),
            node: num_map(&[
                ("signals", 1.5),
                ("roundabout", 3.0),
                ("priority", 0.2),
                ("stop", 0.2),
                ("none", 0.0),
            ]),
            extra: Map::new(),
        }),
        sim: Maybe::Set(SimSettings {
            scale: n(1.0),
            speed: n(6.0),
            max_veh: n(6000.0),
            extra: Map::new(),
        }),
        data: Maybe::Set(DataSettings {
            am_share: n(8.5),
            pm_share: n(9.5),
            match_radius: n(40.0),
            extra: Map::new(),
        }),
        linz_key: s(""),
        basemap: s(""),
        calib: Maybe::Missing,
        co2_factor: Maybe::Missing,
        extra: Map::new(),
    }
}

/* ---------------- project ---------------- */

/// A saved project (`Store.P`): settings, scenarios and the project-wide records the engine reads.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    /// File format version (1).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub v: Num,
    /// Project id.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub id: Maybe<String>,
    /// Project name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Maybe<String>,
    /// Settings.
    pub settings: Settings,
    /// Scenarios, in the order the project lists them.
    pub scenarios: Vec<Scenario>,
    /// Id of the scenario being edited (`Store.sc`).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub active: Maybe<String>,
    /// Id of the base scenario.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub base: Maybe<String>,
    /// Map view { c: [lat, lng], z } (interface only).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub view: Maybe<Value>,
    /// The committed-development register (reg-receiving-env).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub register: Maybe<Vec<RegisterRow>>,
    /// Loaded data: vintages, MSM forecasts, AT controlled intersections.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub data: Maybe<ProjectData>,
    /// Matrix estimation factors and fit records (`P.me`).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub me: Maybe<MatrixEstimation>,
    /// Per-land-use trip-end factors fitted to counts: period, then land use (or "gateways"), then factor.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lu_factors: Maybe<OrderedMap<Maybe<OrderedMap<Num>>>>,
    /// Validation records by period.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub validation: Maybe<OrderedMap<Maybe<ValidationRecord>>>,
    /// The last assessment set the wizard made.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub assessment: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `P.data`: data loaded into the project (the engine reads `loaded`, `msm` and `controlled`).
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
pub struct ProjectData {
    /// Vintages by source ("nzta", "at", "user", "fc", "controlled", "cas"): { at, source, n, licence, label }.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub loaded: Maybe<OrderedMap<Value>>,
    /// MSM forecasts { src, date, validated, links: { linkId: { growth } } }.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub msm: Maybe<Value>,
    /// AT controlled-intersection points [{ lat, lng, no, id }] (Sites.points).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub controlled: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `P.me`: matrix estimation. `info` holds each period's fit record; every other key is a period
/// ("AM", "PM") holding factors keyed "<origin zone id>><destination zone id>".
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
pub struct MatrixEstimation {
    /// Fit records by period.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub info: Maybe<OrderedMap<Maybe<MeInfo>>>,
    /// Factors by period, in file order (the engine applies them in this order).
    #[serde(flatten)]
    pub factors: OrderedMap<Maybe<OrderedMap<Num>>>,
}

/// One period's matrix estimation record (`P.me.info[period]`).
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeInfo {
    /// Year the fit was made for.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub base_year: Num,
    /// Additive adjustments (veh/h) keyed "<origin>><destination>", carried into other years.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub delta: Maybe<Value>,
    /// Date of the fit.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub date: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One period's validation record (`P.validation[period]`), written by the validation step.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
pub struct ValidationRecord {
    /// Independent counts met the targets (`pass === true`).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub pass: Num,
    /// The counts were independent of calibration.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub independent: Num,
    /// TMDG category the record was made for.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub category: Num,
    /// Standard the record was made for ("tmdg", "tag", "rms", "atap").
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub standard: Num,
    /// Date of the record.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub date: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A row of the committed-development register (model.js Register).
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRow {
    /// Row id ("preset:mill-road-1").
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub id: Maybe<String>,
    /// Consent or plan reference.
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "ref")]
    pub reference: Num,
    /// Name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Num,
    /// Type: "consent", "ftaa", "planchange", "precinct" or "network".
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "type")]
    pub row_type: Maybe<String>,
    /// Yield.
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "yield")]
    pub yield_: Maybe<Value>,
    /// Trip rates.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub rates: Maybe<Value>,
    /// Consent status text (granted, approved, operative ...).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub status: Num,
    /// "likely", "possible" or "unlikely".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub likely: Num,
    /// Network projects: "funded", "programmed" or "unfunded".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub funding: Maybe<String>,
    /// Source.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub source: Num,
    /// Date.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub date: Num,
    /// As-at date.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub as_at: Num,
    /// Still to be verified.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub verify: Num,
    /// A preset row.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub preset: Num,
    /// Zone ids it covers.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub zones: Maybe<Vec<String>>,
    /// Link ids it covers.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub links: Maybe<Vec<String>>,
    /// The author's override: true or false, otherwise the rules decide.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub include: Num,
    /// Note.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub note: Num,
    /// Designation record.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub designation: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/* ---------------- scenario ---------------- */

/// A scenario: the network, demand and assessment fields (model.js newScenario, Scenarios).
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    /// Scenario id.
    pub id: String,
    /// Name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Maybe<String>,
    /// Notes.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub notes: Num,
    /// Nodes by id, in file order.
    #[serde(default)]
    pub nodes: OrderedMap<Node>,
    /// Links by id, in file order.
    #[serde(default)]
    pub links: OrderedMap<Link>,
    /// Zones and gateways by id, in file order.
    #[serde(default)]
    pub zones: OrderedMap<Zone>,
    /// Bus routes by id, in file order.
    #[serde(default)]
    pub routes: OrderedMap<Route>,
    /// Manual OD matrices by period (null when none).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub od_manual: Maybe<OrderedMap<Maybe<OdMatrix>>>,
    /// Results by period, as saved (paths stripped).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub res: Maybe<Value>,
    /// Junction sites by id.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub sites: Maybe<OrderedMap<Site>>,
    /// Scenario year (missing: the project year).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub year: Num,
    /// Role: "existing", "baseline", "proposal", "mitigation", "sensitivity" or "permitted".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub role: Maybe<String>,
    /// Share of the development zones built (%), staging.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub buildout: Num,
    /// Periods assessed (["AM", "PM"]).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub periods: Maybe<Vec<String>>,
    /// Scenario code ("S0", "S1" ...).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub code: Maybe<String>,
    /// Assessment set id.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub set_id: Maybe<String>,
    /// What the wizard put in: "existing", "committed" or "mitigation".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub network: Maybe<String>,
    /// For a sensitivity copy, the scenario it copies.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub sens_of: Maybe<String>,
    /// Study extent.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub extent: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A manual OD matrix for one period: zone ids and trips m[origin][destination] (veh/h).
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
pub struct OdMatrix {
    /// Zone ids, the order of the rows and columns.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub zones: Maybe<Vec<String>>,
    /// Trips by row (origin) and column (destination).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub m: Maybe<Vec<Maybe<Vec<Num>>>>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A node (Net.addNode): position, junction control and per-junction data.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Node id.
    pub id: String,
    /// Latitude (degrees).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lat: Num,
    /// Longitude (degrees).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lng: Num,
    /// Control: "none", "priority", "stop", "roundabout" or "signals".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub control: Maybe<String>,
    /// OpenStreetMap node id.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub osm: Maybe<Value>,
    /// U-turns allowed here (`allowU === true`).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub allow_u: Num,
    /// Banned turns, "<from link id>><to link id>".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub bans: Maybe<Value>,
    /// Lane model per approach link id: { lanes: [{ d, len } or "L+T"], median, slip, src }.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lanes: Maybe<OrderedMap<Value>>,
    /// Gap-acceptance overrides { majorR: { tc, tf }, ... }.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub gaps: Maybe<Value>,
    /// Fixed signal cycle (s).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub cycle: Num,
    /// Keep the signal timing as set (no optimisation).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub fixed_timing: Num,
    /// Signal green split [phase 0, phase 1].
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub split: Maybe<Value>,
    /// New or upgraded junction (capital cost).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub upgrade: Num,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A link (Net.addLink): two nodes, shape points, class, lanes each way and attributes.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    /// Link id.
    pub id: String,
    /// Node at the start.
    pub a: String,
    /// Node at the end.
    pub b: String,
    /// Road name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Maybe<String>,
    /// Road class key (CLASSES; unknown reads as local).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub cls: Maybe<String>,
    /// Lanes from a to b.
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "lanesAB")]
    pub lanes_ab: Num,
    /// Lanes from b to a.
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "lanesBA")]
    pub lanes_ba: Num,
    /// Speed limit (km/h).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub speed: Num,
    /// Capacity per lane (pcu/h; null = the class value).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub cap_per_lane: Num,
    /// "existing", "proposed" or "closed".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub status: Maybe<String>,
    /// One lane each way is a bus lane (with 2 or more lanes).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub bus_lane: Num,
    /// Cycling provision: "none", "painted" or "separated".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub cycle: Maybe<String>,
    /// Shape points between the end nodes.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub pts: Maybe<Vec<Point>>,
    /// Part of a roundabout ring.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub ring: Num,
    /// Time-of-use charge (s): a number, or { AM, PM }.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub toll: Maybe<Value>,
    /// Variable speed limit { v, desc, AM, PM }.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub var_speed: Maybe<Value>,
    /// Stage: "existing", "committed", "unfunded", "proposal" or "mitigation".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub stage: Maybe<String>,
    /// Committed projects: "funded", "programmed" or "unfunded".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub funding: Maybe<String>,
    /// Future Connect classes { gt, pt, freight, gt10, pt10, freight10, user, src }.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub fc: Maybe<Value>,
    /// OpenStreetMap way segment key.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub osm: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A zone (land use) or gateway (external counts).
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    /// Zone id.
    pub id: String,
    /// "zone" or "gateway".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub kind: Maybe<String>,
    /// Name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Maybe<String>,
    /// Latitude (degrees).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lat: Num,
    /// Longitude (degrees).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lng: Num,
    /// Land use quantities by LANDUSES key.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lu: Maybe<OrderedMap<Num>>,
    /// Connectors (null = the settings value).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub connectors: Num,
    /// Class: "existing", "committed" or "development".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub role: Maybe<String>,
    /// Opening year: no trips before it.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub year: Num,
    /// Share built (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub buildout: Num,
    /// Existing land use (`existing === true`).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub existing: Num,
    /// Where the zone came from.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub src: Maybe<Value>,
    /// Node a gateway (or pinned zone) connects to.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub node: Maybe<String>,
    /// Gateway counts { AMin, AMout, PMin, PMout } (veh/h).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub gw: Maybe<OrderedMap<Num>>,
    /// Source of gateway counts (set: counted, so land-use factors do not apply).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub gw_src: Maybe<Value>,
    /// Trip-generation table ({ on, ... }).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub tg: Maybe<Value>,
    /// Trip-generation loads { AMin, AMout, PMin, PMout } used when tg.on.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub tg_load: Maybe<OrderedMap<Num>>,
    /// Extra trips { AMin, AMout, PMin, PMout } on top of land use (the saved key is "extra").
    #[serde(default, skip_serializing_if = "Maybe::is_missing", rename = "extra")]
    pub extra_trips: Maybe<OrderedMap<Num>>,
    /// Trips replaced by public transport and active modes (%).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub reduction: Num,
    /// Register row id.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub reg: Maybe<String>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A bus route: waypoints and buses an hour.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    /// Route id.
    pub id: String,
    /// Name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Maybe<String>,
    /// Map colour.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub color: Maybe<String>,
    /// Buses an hour (when the period has no own frequency).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub freq: Num,
    /// Buses an hour by period (GTFS) { AM, PM }.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub freqs: Maybe<Value>,
    /// Waypoint node ids.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub wp: Maybe<Vec<String>>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A junction site (Sites.detect, Sites.merge): nodes that act as one intersection.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    /// Site id (the key it is stored under).
    pub id: String,
    /// Member node ids, sorted.
    pub nodes: Vec<String>,
    /// Control (null: the strongest of the nodes' controls).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub control: Maybe<String>,
    /// Signal cycle (s).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub cycle: Num,
    /// Signal phases.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub phases: Maybe<Value>,
    /// Name ("Great South Road / Waihoehoe Road").
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Maybe<String>,
    /// AT controlled-intersection site number(s).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub at_site_no: Maybe<Value>,
    /// Keep the signal timing as set.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub fixed_timing: Num,
    /// Lane model per approach link id.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lanes: Maybe<OrderedMap<Value>>,
    /// Where the site came from.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub src: Maybe<SiteSource>,
    /// Gap-acceptance overrides.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub gaps: Maybe<Value>,
    /// Signal green split.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub split: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `site.src`: "auto" (made by Sites.detect) or "user" (made by hand), the date and the rule.
#[derive(Clone, Debug, PartialEq, DeriveSerialize, DeriveDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSource {
    /// "auto" or "user".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub nodes: Maybe<String>,
    /// Date made (YYYY-MM-DD).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub at: Maybe<String>,
    /// Rule set used.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub rule: Maybe<String>,
    /// Source of the AT site number.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub at_site_no: Maybe<String>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/* ---------------- new projects and network edits ---------------- */

/// model.js `newScenario(name)`, with the id given (the JavaScript makes a random one); year is left unset.
pub fn new_scenario(id: &str, name: &str) -> Scenario {
    Scenario {
        id: id.to_string(),
        name: Maybe::Set(name.to_string()),
        notes: Maybe::Set(crate::jsnum::JsNum::Text(String::new())),
        nodes: OrderedMap::new(),
        links: OrderedMap::new(),
        zones: OrderedMap::new(),
        routes: OrderedMap::new(),
        od_manual: Maybe::Null,
        res: Maybe::Set(Value::Object(Map::new())),
        sites: Maybe::Missing,
        year: Maybe::Missing,
        role: Maybe::Missing,
        buildout: Maybe::Missing,
        periods: Maybe::Missing,
        code: Maybe::Missing,
        set_id: Maybe::Missing,
        network: Maybe::Missing,
        sens_of: Maybe::Missing,
        extent: Maybe::Missing,
        extra: Map::new(),
    }
}

/// model.js `newProject(name)`, with the project and base-case ids given: one "Base case" scenario, default settings.
pub fn new_project(id: &str, name: &str, scenario_id: &str) -> Project {
    let sc = new_scenario(scenario_id, "Base case");
    Project {
        v: Num::number(1.0),
        id: Maybe::Set(id.to_string()),
        name: Maybe::Set(name.to_string()),
        settings: default_settings(),
        scenarios: vec![sc],
        active: Maybe::Set(scenario_id.to_string()),
        base: Maybe::Set(scenario_id.to_string()),
        view: Maybe::Set(serde_json::json!({ "c": [-36.8485, 174.7633], "z": 14 })),
        register: Maybe::Missing,
        data: Maybe::Missing,
        me: Maybe::Missing,
        lu_factors: Maybe::Missing,
        validation: Maybe::Missing,
        assessment: Maybe::Missing,
        extra: Map::new(),
    }
}

impl Node {
    /// The fields of `Net.addNode(sc, ll)`: position rounded to 7 decimals, control "none".
    pub fn new(id: &str, ll: LatLng) -> Node {
        Node {
            id: id.to_string(),
            lat: Num::number(r7(ll[0])),
            lng: Num::number(r7(ll[1])),
            control: Maybe::Set("none".to_string()),
            osm: Maybe::Missing,
            allow_u: Maybe::Missing,
            bans: Maybe::Missing,
            lanes: Maybe::Missing,
            gaps: Maybe::Missing,
            cycle: Maybe::Missing,
            fixed_timing: Maybe::Missing,
            split: Maybe::Missing,
            upgrade: Maybe::Missing,
            extra: Map::new(),
        }
    }
    /// `[n.lat, n.lng]` as the engine's arithmetic reads it (`Number()` of each).
    pub fn ll(&self) -> LatLng {
        [self.lat.num(), self.lng.num()]
    }
}

impl Link {
    /// The fields of `Net.addLink(sc, a, b, pts, { cls })`: the class's lanes and speed, existing, no bus lane or cycleway.
    ///
    /// cls None is local; an unknown key is kept on the link and reads as local, as in the JavaScript.
    pub fn new(id: &str, a: &str, b: &str, pts: &[LatLng], cls: Option<&str>) -> Link {
        let t = road_class(cls);
        Link {
            id: id.to_string(),
            a: a.to_string(),
            b: b.to_string(),
            name: Maybe::Set(String::new()),
            cls: Maybe::Set(cls.unwrap_or("local").to_string()),
            lanes_ab: Num::number(t.lanes),
            lanes_ba: Num::number(t.lanes),
            speed: Num::number(t.speed),
            cap_per_lane: Maybe::Null,
            status: Maybe::Set("existing".to_string()),
            bus_lane: false.into(),
            cycle: Maybe::Set("none".to_string()),
            pts: Maybe::Set(pts.iter().map(|p| Point([r7(p[0]), r7(p[1])])).collect()),
            ring: Maybe::Missing,
            toll: Maybe::Missing,
            var_speed: Maybe::Missing,
            stage: Maybe::Missing,
            funding: Maybe::Missing,
            fc: Maybe::Missing,
            osm: Maybe::Missing,
            extra: Map::new(),
        }
    }
    /// `CLASSES[L.cls] || CLASSES.local`.
    pub fn class(&self) -> &'static RoadClass {
        road_class(self.cls.as_str())
    }
}

impl Zone {
    /// A gateway or zone's `[z.lat, z.lng]` as the engine's arithmetic reads it.
    pub fn ll(&self) -> LatLng {
        [self.lat.num(), self.lng.num()]
    }
}

impl Scenario {
    /// `Net.addNode(sc, ll)`: adds a node (last in order) and returns it for further fields.
    pub fn add_node(&mut self, id: &str, ll: LatLng) -> &mut Node {
        self.nodes.insert(id, Node::new(id, ll));
        self.nodes.get_mut(id).expect("just inserted")
    }
    /// `Net.addLink(sc, a, b, pts, { cls })`: adds a link (last in order) and returns it for further fields.
    pub fn add_link(
        &mut self,
        id: &str,
        a: &str,
        b: &str,
        pts: &[LatLng],
        cls: Option<&str>,
    ) -> &mut Link {
        self.links.insert(id, Link::new(id, a, b, pts, cls));
        self.links.get_mut(id).expect("just inserted")
    }
    /// `Net.geom(sc, L)`: [node a, shape points..., node b]. Both end nodes must exist (the JavaScript throws otherwise).
    pub fn geom(&self, l: &Link) -> Vec<LatLng> {
        let a = self
            .nodes
            .get(&l.a)
            .unwrap_or_else(|| panic!("link {} has no node {}", l.id, l.a));
        let b = self
            .nodes
            .get(&l.b)
            .unwrap_or_else(|| panic!("link {} has no node {}", l.id, l.b));
        let pts = l.pts.get().map_or(&[][..], Vec::as_slice);
        let mut g = Vec::with_capacity(pts.len() + 2);
        g.push(a.ll());
        g.extend(pts.iter().map(|p| p.0));
        g.push(b.ll());
        g
    }
    /// `Net.len(sc, L)`: link length along its shape (m).
    pub fn link_len(&self, l: &Link) -> f64 {
        poly_len(&self.geom(l))
    }
}

impl Project {
    /// Reads a saved project from JSON text.
    pub fn from_json(text: &str) -> serde_json::Result<Project> {
        serde_json::from_str(text)
    }
    /// The project as a JSON value (every key it was read with, `extra` included).
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("a project always serialises")
    }
    /// `Store.serialise(P)`: JSON text without the keys "paths", "arcKeys" and "settingsNotice" at any depth.
    pub fn serialise(&self) -> String {
        let mut v = self.to_value();
        strip_keys(&mut v, &["paths", "arcKeys", "settingsNotice"]);
        serde_json::to_string(&v).expect("a JSON value always serialises")
    }
    /// The scenario with this id.
    pub fn scenario(&self, id: &str) -> Option<&Scenario> {
        self.scenarios.iter().find(|s| s.id == id)
    }
    /// `Store.sc`: the active scenario, otherwise the first.
    pub fn active_scenario(&self) -> Option<&Scenario> {
        self.active
            .as_str()
            .and_then(|id| self.scenario(id))
            .or(self.scenarios.first())
    }
    /// `Store.baseSc`: the base scenario, otherwise the first.
    pub fn base_scenario(&self) -> Option<&Scenario> {
        self.base
            .as_str()
            .and_then(|id| self.scenario(id))
            .or(self.scenarios.first())
    }
    /// A register row by id (`(P.register || []).find(r => r.id === id)`).
    pub fn register_row(&self, id: &str) -> Option<&RegisterRow> {
        self.register
            .get()
            .and_then(|rows| rows.iter().find(|r| r.id.is(id)))
    }
}

fn strip_keys(v: &mut Value, keys: &[&str]) {
    match v {
        Value::Object(m) => {
            m.retain(|k, _| !keys.contains(&k.as_str()));
            for x in m.values_mut() {
                strip_keys(x, keys);
            }
        }
        Value::Array(a) => {
            for x in a {
                strip_keys(x, keys);
            }
        }
        _ => {}
    }
}

/* ---------------- assessment rules the engine calls ---------------- */

const LINK_STAGES: [&str; 5] = [
    "existing",
    "committed",
    "unfunded",
    "proposal",
    "mitigation",
];

/// `Scenarios.linkStage(L)`: the link's stage, or "proposal" for a proposed link and "existing" otherwise.
pub fn link_stage(l: &Link) -> &str {
    if let Some(s) = l.stage.as_str()
        && LINK_STAGES.contains(&s)
    {
        return s;
    }
    if l.status.is("proposed") {
        "proposal"
    } else {
        "existing"
    }
}

/// `Scenarios.unfunded(L)`: stage "unfunded", or committed with funding "unfunded" (sensitivity scenarios only).
pub fn link_unfunded(l: &Link) -> bool {
    let s = link_stage(l);
    s == "unfunded" || (s == "committed" && l.funding.is("unfunded"))
}

/// `Scenarios.programmed(L)`: committed, programmed but not funded.
pub fn link_programmed(l: &Link) -> bool {
    link_stage(l) == "committed" && l.funding.is("programmed")
}

/// `Register.inBaseline(row)`: whether a register row belongs in the future baseline (do-minimum), and why (AT ITA s3.6.1).
pub fn in_baseline(row: Option<&RegisterRow>) -> (bool, String) {
    let Some(row) = row else {
        return (false, "Not in the register.".to_string());
    };
    if row.include.is_true() || row.include.is_false() {
        let note = if row.note.truthy() {
            format!(": {}", row.note.js_string())
        } else {
            String::new()
        };
        return (row.include.is_true(), format!("Author's decision{}.", note));
    }
    if row.row_type.is("network") {
        if row.funding.is("funded") {
            return (true, "Funded network project.".to_string());
        }
        let why = if row.funding.is("programmed") {
            "Programmed but not funded: sensitivity scenarios only (AT ITA s3.6.1)."
        } else if row.funding.is("unfunded") {
            "Unfunded: sensitivity scenarios only (AT ITA s3.6.1)."
        } else {
            "Funding status not recorded: confirm it before including it."
        };
        return (false, why.to_string());
    }
    // /grant|approv|operative/i.test(row.status || '')
    let status = if row.status.truthy() {
        row.status.js_string()
    } else {
        String::new()
    };
    let low = status.to_ascii_lowercase();
    if !(low.contains("grant") || low.contains("approv") || low.contains("operative")) {
        let shown = if status.is_empty() {
            "status not recorded".to_string()
        } else {
            status
        };
        return (false, format!("Not granted ({}).", shown));
    }
    if !(matches!(&row.likely, Maybe::Set(crate::jsnum::JsNum::Text(t)) if t == "likely")) {
        let rated = if row.likely.truthy() {
            row.likely.js_string()
        } else {
            "not assessed".to_string()
        };
        return (
            false,
            format!("Granted, but implementation is rated {}.", rated),
        );
    }
    (true, "Granted and likely to be implemented.".to_string())
}
