//! Reading OpenStreetMap tags for the land-use build (landuse.js): numbers with thousands separators, floors from levels or height,
//! school rolls, and the tag sets that classify a building, an amenity or a shop.

use crate::importers::jsparse::parse_float;
use crate::jsnum::{js_trim, math_max, math_min, math_round, value_to_js_string, value_truthy};
use serde_json::Value;

/// A tag's value as text, `String(v ?? '')`: missing and null are "".
pub fn tag_text(t: &Value, k: &str) -> String {
    match t.get(k) {
        None | Some(Value::Null) => String::new(),
        Some(v) => value_to_js_string(v),
    }
}

/// `t[k]` is set and truthy (a non-empty text, a non-zero number, true).
pub fn tag_truthy(t: &Value, k: &str) -> bool {
    t.get(k).is_some_and(value_truthy)
}

/// The tag as text when it is text (`t[k] === 'x'` comparisons and `Set.has(t[k])`).
pub fn tag_str<'a>(t: &'a Value, k: &str) -> Option<&'a str> {
    t.get(k).and_then(Value::as_str)
}

/// landuse.js `num(v)`: `parseFloat(String(v ?? '').replace(',', '.'))`, or None when it is not a finite number.
pub fn num(v: &str) -> Option<f64> {
    let x = parse_float(&v.replacen(',', ".", 1));
    x.is_finite().then_some(x)
}

/// True for `/^\d{1,3}([, ]\d{3})+$/`: a count written with thousands separators ("1,200", "1 450").
fn has_thousands_separators(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if !(1..=3).contains(&i) {
        return false;
    }
    let mut groups = 0;
    while i < b.len() {
        if i + 4 > b.len()
            || !(b[i] == b',' || b[i] == b' ')
            || !b[i + 1..i + 4].iter().all(u8::is_ascii_digit)
        {
            return false;
        }
        i += 4;
        groups += 1;
    }
    groups >= 1
}

/// landuse.js `count(v)`: a count (rolls, capacities, rooms) that may carry thousands separators ("1,200" or "1 450"); None when it
/// is not a finite number.
pub fn count(v: &str) -> Option<f64> {
    let s = js_trim(v);
    if has_thousands_separators(s) {
        num(&s.replace([',', ' '], ""))
    } else {
        num(s)
    }
}

/// `count(t[k])` of a tag.
pub fn count_tag(t: &Value, k: &str) -> Option<f64> {
    count(&tag_text(t, k))
}

/// landuse.js `levelsOf(t, apartments)`: floors from `building:levels`, else height divided by 3 m, else 1 (3 for buildings tagged apartments).
pub fn levels_of(t: &Value, apartments: bool) -> f64 {
    if let Some(l) = num(&tag_text(t, "building:levels"))
        && l >= 1.0
    {
        return math_min(60.0, math_round(l));
    }
    if let Some(h) = num(&tag_text(t, "height"))
        && h >= 4.5
    {
        return math_min(60.0, math_max(1.0, math_round(h / 3.0)));
    }
    if apartments { 3.0 } else { 1.0 }
}

/// True when the text has one of the words as a whole word, ignoring case (`/\b(a|b)\b/i`).
fn has_word(s: &str, words: &[&str]) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let is_w = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let lower: Vec<char> = s.to_lowercase().chars().collect();
    // Lower-casing can change the length for a few characters; the prototype's /i compares by simple case folding
    if lower.len() != chars.len() {
        return false;
    }
    for w in words {
        let wc: Vec<char> = w.chars().collect();
        if wc.len() > lower.len() {
            continue;
        }
        for i in 0..=(lower.len() - wc.len()) {
            if lower[i..i + wc.len()] == wc[..]
                && (i == 0 || !is_w(chars[i - 1]))
                && (i + wc.len() == chars.len() || !is_w(chars[i + wc.len()]))
            {
                return true;
            }
        }
    }
    false
}

/// The runs of ASCII digits in a string as numbers (`s.match(/\d+/g).map(Number)`).
fn digit_runs(s: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(cur.parse::<f64>().expect("digits"));
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push(cur.parse::<f64>().expect("digits"));
    }
    out
}

/// landuse.js `students(t)`: a school's roll: the capacity tag, else 350 (years 1 to 8 or primary), 800 (College or High, or years up
/// to 9 and above) from the year levels or the name, else 400.
pub fn students(t: &Value) -> f64 {
    let c = count_tag(t, "capacity")
        .or_else(|| count_tag(t, "students"))
        .or_else(|| count_tag(t, "capacity:pupils"));
    if let Some(c) = c
        && c > 0.0
    {
        return math_round(c);
    }
    let yrs_src = ["MOE:years", "grades", "school:years"]
        .iter()
        .find(|k| tag_truthy(t, k))
        .map(|k| tag_text(t, k))
        .unwrap_or_default();
    let yrs = digit_runs(&yrs_src);
    let (lo, hi) = if yrs.is_empty() {
        (None, None)
    } else {
        (
            Some(yrs.iter().copied().fold(f64::INFINITY, math_min)),
            Some(yrs.iter().copied().fold(f64::NEG_INFINITY, math_max)),
        )
    };
    let name = if tag_truthy(t, "name") {
        tag_text(t, "name")
    } else {
        String::new()
    };
    let isced = tag_str(t, "isced:level");
    if has_word(&name, &["college", "high"])
        || (lo.is_some_and(|l| l >= 7.0) && hi.is_some_and(|h| h >= 9.0))
        || isced == Some("2;3")
        || isced == Some("3")
    {
        return 800.0;
    }
    if hi.is_some_and(|h| h <= 8.0)
        || has_word(&name, &["primary", "intermediate", "kura"])
        || isced == Some("1")
    {
        return 350.0;
    }
    400.0
}

/// Building values that are not buildings people use (landuse.js IGNORE).
pub const IGNORE: [&str; 25] = [
    "garage",
    "garages",
    "shed",
    "carport",
    "roof",
    "greenhouse",
    "hut",
    "cabin",
    "service",
    "construction",
    "abandoned",
    "ruins",
    "hangar",
    "farm_auxiliary",
    "barn",
    "cowshed",
    "stable",
    "sty",
    "glasshouse",
    "tank",
    "silo",
    "storage_tank",
    "water_tower",
    "bunker",
    "bridge",
];
/// More building values that are ignored (landuse.js IGNORE continued).
pub const IGNORE2: [&str; 5] = ["container", "shelter", "toilets", "kiosk_atm", "no"];
/// Buildings that generate trips but have no land use (churches, halls, clubrooms; landuse.js OTHER_B).
pub const OTHER_B: [&str; 22] = [
    "church",
    "chapel",
    "cathedral",
    "mosque",
    "temple",
    "synagogue",
    "shrine",
    "religious",
    "pavilion",
    "public",
    "civic",
    "government",
    "sports_hall",
    "sports_centre",
    "grandstand",
    "stadium",
    "transportation",
    "train_station",
    "marae",
    "fire_station",
    "police",
    "museum",
];
/// Amenities that are community or other uses (landuse.js OTHER_A).
pub const OTHER_A: [&str; 16] = [
    "place_of_worship",
    "community_centre",
    "library",
    "toilets",
    "shelter",
    "townhall",
    "social_facility",
    "fire_station",
    "police",
    "arts_centre",
    "theatre",
    "social_centre",
    "events_venue",
    "bus_station",
    "parking",
    "fuel",
];
/// Residential building values (landuse.js RES_B).
pub const RES_B: [&str; 5] = [
    "house",
    "detached",
    "semidetached_house",
    "bungalow",
    "static_caravan",
];
/// Retail building values (landuse.js RETAIL_B).
pub const RETAIL_B: [&str; 3] = ["retail", "kiosk", "shop"];
/// Industrial building values (landuse.js IND_B).
pub const IND_B: [&str; 4] = ["industrial", "factory", "manufacture", "workshop"];
/// Shops that are workshops (landuse.js SHOP_IND).
pub const SHOP_IND: [&str; 3] = ["car_repair", "tyres", "motorcycle_repair"];
/// Trade and bulky-goods shops (landuse.js SHOP_BULKY).
pub const SHOP_BULKY: [&str; 21] = [
    "car",
    "car_parts",
    "hardware",
    "trade",
    "garden_centre",
    "furniture",
    "bathroom_furnishing",
    "electrical",
    "tools",
    "building_materials",
    "doityourself",
    "flooring",
    "kitchen",
    "appliance",
    "boat",
    "motorcycle",
    "caravan",
    "agrarian",
    "tiles",
    "bed",
    "lighting",
];
/// Shops with a yard that adds no floor area (landuse.js SHOP_YARD).
pub const SHOP_YARD: [&str; 8] = [
    "car",
    "garden_centre",
    "trade",
    "building_materials",
    "boat",
    "caravan",
    "motorcycle",
    "agrarian",
];

/// landuse.js `shopUse(s)`: workshops count as industrial, trade and bulky-goods shops as warehouse, others as retail.
pub fn shop_use(s: Option<&str>) -> &'static str {
    match s {
        Some(s) if SHOP_IND.contains(&s) => "industrial",
        Some(s) if SHOP_BULKY.contains(&s) => "warehouse",
        _ => "retail",
    }
}

/// landuse.js LU_CLASS: the OpenStreetMap landuse values the build reads.
pub fn lu_class(v: Option<&str>) -> Option<&'static str> {
    match v? {
        "residential" => Some("residential"),
        "commercial" => Some("commercial"),
        "retail" => Some("retail"),
        "industrial" => Some("industrial"),
        "education" => Some("education"),
        _ => None,
    }
}

/// landuse.js WEIGHT: dwelling-equivalents per unit (dwelling, m2 GFA, student, child): AM and PM vehicle trips per unit relative to a
/// house, from the indicative default rates. Used only to weight zone centres and merges. In the prototype's key order.
pub const WEIGHT: [(&str, f64); 13] = [
    ("residential", 1.0),
    ("apartment", 0.53),
    ("retail", 0.033),
    ("office", 0.021),
    ("industrial", 0.0067),
    ("school", 0.28),
    ("childcare", 0.83),
    ("medical", 0.03),
    ("supermarket", 0.06),
    ("tertiary", 0.25),
    ("hotel", 0.44),
    ("warehouse", 0.0056),
    ("fastfood", 0.089),
];

/// The weight of a land-use key.
pub fn weight(k: &str) -> f64 {
    WEIGHT
        .iter()
        .find(|(key, _)| *key == k)
        .map_or(f64::NAN, |(_, w)| *w)
}

/// Amenities that are schools (landuse.js EDU_A).
pub const EDU_A: [&str; 1] = ["school"];
/// Amenities that are tertiary campuses (landuse.js TERT_A).
pub const TERT_A: [&str; 2] = ["college", "university"];
/// Amenities that are early childhood centres (landuse.js CHILD_A).
pub const CHILD_A: [&str; 2] = ["kindergarten", "childcare"];
/// Amenities that are clinics (landuse.js MED_A).
pub const MED_A: [&str; 3] = ["doctors", "clinic", "dentist"];
/// Building values that are schools (landuse.js EDU_B).
pub const EDU_B: [&str; 1] = ["school"];
/// Building values that are tertiary buildings (landuse.js TERT_B).
pub const TERT_B: [&str; 2] = ["university", "college"];
/// Building values that are early childhood buildings (landuse.js CHILD_B).
pub const CHILD_B: [&str; 2] = ["kindergarten", "childcare"];

/// `Set.has(v)` for a tag that may be missing.
pub fn in_set(set: &[&str], v: Option<&str>) -> bool {
    v.is_some_and(|v| set.contains(&v))
}
