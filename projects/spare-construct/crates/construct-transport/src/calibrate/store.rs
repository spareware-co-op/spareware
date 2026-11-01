//! What the calibration panel keeps in the project (`P.data.calib`): the role of each count, the screenlines and
//! cordons drawn on the map, and the imported turning counts.

use crate::jsnum::{Maybe, Num};
use crate::project::{OrderedMap, Point, Project};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A screenline, a cordon or a core area (an entry of `P.data.calib.screenlines`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Screenline {
    /// Id.
    #[serde(default)]
    pub id: String,
    /// Name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Num,
    /// "screen" (a line), "cordon" (a closed area, inbound and outbound) or "core" (the area of influence).
    #[serde(default, rename = "type", skip_serializing_if = "Maybe::is_missing")]
    pub kind: Num,
    /// "valid" (held out for validation) or "me" / missing (calibration).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub role: Num,
    /// Motorways count in the totals (`motorways !== false`).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub motorways: Num,
    /// The line or polygon as [lat, lng] points.
    #[serde(default)]
    pub pts: Vec<Point>,
    /// Where a suggested screenline came from.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub src: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Screenline {
    /// `sl.type`.
    pub fn type_text(&self) -> String {
        self.kind.js_string()
    }
    /// `sl.type === t`.
    pub fn is_type(&self, t: &str) -> bool {
        self.kind.as_text_is(t)
    }
    /// `sl.role === 'valid'`.
    pub fn is_valid(&self) -> bool {
        self.role.as_text_is("valid")
    }
}

/// Small helpers on loose values.
pub trait NumText {
    /// `x === s` for text.
    fn as_text_is(&self, s: &str) -> bool;
}

impl NumText for Num {
    fn as_text_is(&self, s: &str) -> bool {
        crate::datahub::is_text(self, s)
    }
}

/// A peak hour found in a turning count (`site.peak[period]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Peak {
    /// Start ("07:30").
    pub start: String,
    /// End ("08:30").
    pub end: String,
    /// Vehicles in the hour.
    pub total: f64,
    /// Peak hour factor, V60 / (4 x V15 max); None without 15-minute counts.
    pub phf: Option<f64>,
    /// Vehicles by movement ("N-L") in the hour.
    pub mv: OrderedMap<f64>,
    /// Heavy vehicles (heavy and buses) by movement.
    pub hv: OrderedMap<f64>,
    /// The count only has leg totals (IN and OUT), no movements.
    pub leg_only: bool,
}

/// Counts by vehicle class in one movement of one 15-minute bin: LV, HV, BUS, CYC in the order they were first met.
pub type ClassCounts = OrderedMap<f64>;

/// Movement counts in one 15-minute bin, by "<leg>-<movement>" ("N-L", "S-IN"), in the order they were first met.
pub type BinCounts = OrderedMap<ClassCounts>;

/// An imported turning count of one site on one day (an entry of `P.data.calib.turnSites`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSite {
    /// Id.
    #[serde(default)]
    pub id: String,
    /// Site reference (the file's site_id).
    #[serde(default, rename = "ref", skip_serializing_if = "Maybe::is_missing")]
    pub reference: Num,
    /// Name.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub name: Num,
    /// Latitude.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lat: Num,
    /// Longitude.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub lng: Num,
    /// Count date (ISO where it could be read, else as written).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub date: Num,
    /// "mcc" (manual classified count), "scats" (detector counts) or "corridor-sidra" (modelled volumes).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub src: Num,
    /// The file it came from.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub file: Num,
    /// "valid", "me" or "off".
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub role: Num,
    /// "measured_hour" or "modelled".
    #[serde(
        default,
        rename = "obs_method",
        skip_serializing_if = "Maybe::is_missing"
    )]
    pub obs_method: Num,
    /// Longest count interval (minutes).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub interval: Num,
    /// Counts by start minute of the 15-minute bin, ascending.
    #[serde(default)]
    pub bins: BTreeMap<u32, BinCounts>,
    /// Road names by leg ("N" to a name).
    #[serde(default)]
    pub leg_roads: OrderedMap<Num>,
    /// Things to know about the count (weekend, school holidays, shared lanes).
    #[serde(default)]
    pub flags: Vec<String>,
    /// When it was imported (ms).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub at: Num,
    /// Peak hours by window ("AM", "IP", "PM").
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub peak: Maybe<OrderedMap<Peak>>,
    /// The peak-hour windows the peaks were found with, as JSON text.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub peak_win: Maybe<String>,
    /// The junction node the site was matched to (null when none).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub node: Maybe<String>,
    /// The junction site the node belongs to (null when none).
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub site: Maybe<String>,
    /// Balanced movement counts by period.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub bal: Maybe<OrderedMap<OrderedMap<f64>>>,
    /// The changes balancing made, by period.
    #[serde(default, skip_serializing_if = "Maybe::is_missing")]
    pub audit: Maybe<Value>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The project's calibration store (`P.data.calib`): count roles, screenlines and turning counts.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibStore {
    /// Count id to role ("valid" or "off"; counts that are not listed are calibration counts).
    #[serde(default)]
    pub roles: OrderedMap<Num>,
    /// Screenlines, cordons and core areas.
    #[serde(default)]
    pub screenlines: Vec<Screenline>,
    /// Turning counts.
    #[serde(default)]
    pub turn_sites: Vec<TurnSite>,
    /// Keys this model does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl CalibStore {
    /// `store()` as a read: what the project holds (empty when it holds nothing; the project is not changed).
    pub fn read(p: &Project) -> CalibStore {
        p.data
            .get()
            .and_then(|d| d.extra.get("calib"))
            .and_then(|v| serde_json::from_value::<CalibStore>(v.clone()).ok())
            .unwrap_or_default()
    }
    /// Puts the store back into the project (`P.data.calib`), making `P.data` if it has none.
    pub fn write(&self, p: &mut Project) {
        if !p.data.is_set() {
            p.data = Maybe::Set(crate::project::ProjectData {
                loaded: Maybe::Missing,
                msm: Maybe::Missing,
                controlled: Maybe::Missing,
                extra: Map::new(),
            });
        }
        let v = serde_json::to_value(self).expect("a store serialises");
        p.data
            .get_mut()
            .expect("just made")
            .extra
            .insert("calib".to_string(), v);
    }
    /// `store().roles[id]`.
    pub fn role(&self, id: &str) -> &Num {
        self.roles.num_at(id)
    }
}
