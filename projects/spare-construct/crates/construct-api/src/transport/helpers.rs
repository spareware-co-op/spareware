//! Small helpers the command files share: order-keeping JSON for editing a record, and the app's number parsing.

use crate::error::{ApiError, ApiResult};
use crate::schema::Params;
use construct_transport::edit::json::JsonValue;
use construct_transport::edit::net::LinkProps;
use construct_transport::edit::{STATUS, name_of};
use construct_transport::jsnum::Maybe;
use construct_transport::project::{CLASSES, Link, Scenario, road_class_exact};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

/// The scenario at an index from `Session::target`, `pick_scenario` or `active_index`. Those indexes come from the same
/// project just before, so a missing one is a bug; it is an error the caller can show, never a crash.
pub fn scenario_at(list: &[Scenario], i: usize) -> ApiResult<&Scenario> {
    list.get(i).ok_or_else(gone)
}

/// [`scenario_at`] for a scenario to change.
pub fn scenario_at_mut(list: &mut [Scenario], i: usize) -> ApiResult<&mut Scenario> {
    list.get_mut(i).ok_or_else(gone)
}

/// The error for a record that a command made or checked a moment before and that is not there after all: a bug, shown
/// rather than a crash.
pub fn lost(what: &str, id: &str) -> ApiError {
    ApiError::new(
        "internal",
        format!("The {what} {id} is not in the scenario."),
    )
}

/// Sets a key of a JSON object, as `o[k] = x` does in the JavaScript. Anything that is not an object is left as it is.
pub fn put(o: &mut Value, k: &str, x: Value) {
    if let Some(m) = o.as_object_mut() {
        m.insert(k.to_string(), x);
    }
}

/// The site with this id in a scenario, or the error that says it is not there.
pub fn site_of<'a>(
    sc: &'a Scenario,
    id: &str,
) -> ApiResult<&'a construct_transport::project::Site> {
    sc.sites
        .get()
        .and_then(|m| m.get(id))
        .ok_or_else(|| lost("site", id))
}

fn gone() -> ApiError {
    ApiError::new("internal", "The scenario is no longer in the project.")
}

/// `roadProps()` of ui.js: the class, lanes each way, speed and status a drawn or imported road takes
/// (`View.template`: arterial, one lane each way, 50 km/h, proposed, unless given).
pub fn road_template(p: Params<'_>) -> ApiResult<LinkProps> {
    let cls = p.opt_str("cls")?.unwrap_or("arterial");
    if road_class_exact(cls).is_none() {
        return Err(ApiError::bad_params(format!(
            "`cls` must be one of: {}",
            CLASSES.map(|c| c.key).join(", ")
        )));
    }
    let lanes = p.opt_f64("lanes")?.unwrap_or(1.0);
    let oneway = p.flag("oneway")?;
    let speed = p.opt_f64("speed")?.unwrap_or(50.0);
    let status = p.opt_str("status")?.unwrap_or("proposed");
    if name_of(&STATUS, status).is_none() {
        return Err(ApiError::bad_params(
            "`status` must be existing, proposed or closed",
        ));
    }
    let mut props = LinkProps::new()
        .lanes(lanes, if oneway { 0.0 } else { lanes })
        .speed(speed)
        .status(status);
    props.0.cls = Maybe::Set(cls.to_string());
    Ok(props)
}

/// A record as order-keeping JSON, so changing one field keeps the order of the others.
pub fn to_jv<T: Serialize>(v: &T) -> ApiResult<JsonValue> {
    let text = serde_json::to_string(v).map_err(|e| ApiError::new("internal", e.to_string()))?;
    JsonValue::parse(&text).map_err(|e| ApiError::new("internal", e))
}

/// A record read back from order-keeping JSON; the error says which field did not fit.
pub fn from_jv<T: DeserializeOwned>(j: &JsonValue) -> ApiResult<T> {
    serde_json::from_str(&j.stringify())
        .map_err(|e| ApiError::bad_params(format!("that value does not fit: {e}")))
}

/// Any serialisable value as JSON (a failure gives null, which cannot happen for the engine's types).
pub fn val<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// The text with a trailing area unit removed: `\s*(m2|m²|sq\.?\s*m|sqm)\.?$`.
fn strip_unit(s: &str) -> &str {
    for c in [s.strip_suffix('.').unwrap_or(s), s] {
        for u in ["m2", "m\u{b2}", "sqm"] {
            if let Some(r) = c.strip_suffix(u) {
                return r;
            }
        }
        if let Some(r) = c.strip_suffix('m') {
            let r = r.trim_end();
            let r = r.strip_suffix('.').unwrap_or(r);
            if let Some(r) = r.strip_suffix("sq") {
                return r;
            }
        }
    }
    s
}

/// `/^[-+]?(\d+\.?\d*|\.\d+)$/.test(s) ? +s : NaN`.
fn plain_number(s: &str) -> f64 {
    let body = s.strip_prefix(['-', '+']).unwrap_or(s);
    let digits = |x: &str| x.chars().all(|c| c.is_ascii_digit());
    let ok = match body.split_once('.') {
        None => !body.is_empty() && digits(body),
        Some((i, f)) => digits(i) && digits(f) && !(i.is_empty() && f.is_empty()),
    };
    if ok {
        construct_transport::jsnum::string_to_number(s)
    } else {
        f64::NAN
    }
}

/// `Onboarding.parseNum`: "12,000" is 12000, "5 000 m2" and "5000sqm" are 5000; empty or null is None; anything else is NaN.
pub fn parse_num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => Some(n.as_f64().filter(|x| x.is_finite()).unwrap_or(f64::NAN)),
        Value::Null => None,
        other => {
            let raw = match other {
                Value::String(s) => s.clone(),
                o => construct_transport::jsnum::value_to_js_string(o),
            };
            let lower = construct_transport::jsnum::js_trim(&raw).to_lowercase();
            let s: String = strip_unit(&lower)
                .chars()
                .filter(|c| !c.is_whitespace() && *c != ',')
                .collect();
            if s.is_empty() {
                None
            } else {
                Some(plain_number(&s))
            }
        }
    }
}

/// A position component rounded to 7 decimals, as the app stores positions (`U.r7`).
pub fn r7(x: f64) -> f64 {
    construct_transport::util::r7(x)
}

const BAD_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

fn child<'a>(o: &'a JsonValue, k: &str) -> Option<&'a JsonValue> {
    match o {
        JsonValue::Object(m) => m.get(k),
        JsonValue::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get(i)),
        _ => None,
    }
}

fn child_mut<'a>(o: &'a mut JsonValue, k: &str) -> Option<&'a mut JsonValue> {
    match o {
        JsonValue::Object(m) => m.get_mut(k),
        JsonValue::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get_mut(i)),
        _ => None,
    }
}

/// `Kit.getPath(o, k)`: the value at a dotted path ("lu.residential", "econ.rates.0.r"); None where a step is missing.
pub fn kit_get_path<'a>(o: &'a JsonValue, k: &str) -> Option<&'a JsonValue> {
    let parts: Vec<&str> = k.split('.').collect();
    if parts.iter().any(|p| BAD_KEYS.contains(p)) {
        return None;
    }
    let mut t = o;
    for p in parts {
        t = child(t, p)?;
    }
    Some(t)
}

/// `Kit.setPath(o, k, v)`: sets the value at a dotted path. A step that is missing or empty becomes an object; a step that holds
/// a plain value stops the write (false), as does a path through `__proto__`, `constructor` or `prototype`.
pub fn kit_set_path(o: &mut JsonValue, k: &str, v: JsonValue) -> bool {
    let parts: Vec<&str> = k.split('.').collect();
    if parts.iter().any(|p| BAD_KEYS.contains(p))
        || !matches!(o, JsonValue::Object(_) | JsonValue::Array(_))
    {
        return false;
    }
    // split always gives at least one part
    let Some((last, steps)) = parts.split_last() else {
        return false;
    };
    let mut cur = o;
    for p in steps {
        let empty = child(cur, p).is_none_or(|c| !c.truthy());
        if empty {
            match cur {
                JsonValue::Object(_) => cur.set(*p, JsonValue::object()),
                JsonValue::Array(items) => match p.parse::<usize>() {
                    Ok(i) if i == items.len() => items.push(JsonValue::object()),
                    Ok(i) => match items.get_mut(i) {
                        Some(slot) => *slot = JsonValue::object(),
                        None => return false,
                    },
                    Err(_) => return false,
                },
                _ => return false,
            }
        }
        match child_mut(cur, p) {
            Some(next) if matches!(next, JsonValue::Object(_) | JsonValue::Array(_)) => cur = next,
            _ => return false,
        }
    }
    match cur {
        JsonValue::Object(_) => {
            cur.set(*last, v);
            true
        }
        JsonValue::Array(items) => match last.parse::<usize>() {
            Ok(i) if i == items.len() => {
                items.push(v);
                true
            }
            Ok(i) => match items.get_mut(i) {
                Some(slot) => {
                    *slot = v;
                    true
                }
                None => false,
            },
            Err(_) => false,
        },
        _ => false,
    }
}

/// Road fields given as JSON (without id, ends and shape points) as the props of a new road; a class must be one of the road classes.
pub fn link_props(o: Option<&Map<String, Value>>) -> ApiResult<LinkProps> {
    let mut m = o.cloned().unwrap_or_default();
    for k in ["id", "a", "b", "pts"] {
        m.remove(k);
    }
    for k in ["id", "a", "b"] {
        m.insert(k.to_string(), json!(""));
    }
    let l: Link = serde_json::from_value(Value::Object(m))
        .map_err(|e| ApiError::bad_params(format!("`props` does not fit a road: {e}")))?;
    if let Some(c) = l.cls.as_str()
        && road_class_exact(c).is_none()
    {
        return Err(ApiError::bad_params(format!(
            "`cls` must be one of: {}",
            CLASSES.map(|c| c.key).join(", ")
        )));
    }
    Ok(LinkProps(l))
}
