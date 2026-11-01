//! The author's inputs to the report (`P.report`), read without changing the project (`rs(P)` with its `clean`).

use super::consts::scope;
use super::js;
use crate::edit::json::JsonValue;

/// The author's report inputs after `clean`: wrong types fall back to the defaults, and an unknown RR 422 scope counts as not chosen.
#[derive(Clone, Debug)]
pub struct Rs {
    /// Project and author details (an object).
    pub details: JsonValue,
    /// The RR 422 scope key, or empty.
    pub scope: String,
    /// Sign-off (an object).
    pub signoff: JsonValue,
    /// Document revisions.
    pub revisions: Vec<JsonValue>,
    /// Engagement log.
    pub engagement: Vec<JsonValue>,
    /// The author's text by box id (an object).
    pub text: JsonValue,
    /// The scenario choice (an object of scenario ids).
    pub sel: JsonValue,
    /// Mitigation register entries (an object).
    pub mitigation: JsonValue,
    /// Safe System likelihoods (an object).
    pub ssm: JsonValue,
    /// Pedestrian crossings.
    pub crossings: Vec<JsonValue>,
    /// Pedestrian delay constants (an object).
    pub ped: JsonValue,
    /// The stored staging run (an object), when there is one.
    pub staging: Option<JsonValue>,
    /// The stored sensitivity pack (an object), when there is one.
    pub sens: Option<JsonValue>,
}

fn obj_or_empty(v: Option<&JsonValue>) -> JsonValue {
    match v {
        Some(x) if x.is_object() => x.clone(),
        _ => JsonValue::object(),
    }
}

fn objects(v: Option<&JsonValue>) -> Vec<JsonValue> {
    js::items(v)
        .iter()
        .filter(|x| x.is_object() || x.is_array())
        .cloned()
        .collect()
}

/// `arrs(o, keys)`: a copy of an object where the named keys that are set but not arrays become empty arrays; None for a
/// value that is not an object.
fn arrs(o: &JsonValue, keys: &[&str]) -> Option<JsonValue> {
    if !o.is_object() {
        return None;
    }
    let mut x = o.clone();
    for k in keys {
        if let Some(v) = x.get(k)
            && !v.is_null()
            && !v.is_array()
        {
            x.set(*k, JsonValue::Array(Vec::new()));
        }
    }
    Some(x)
}

impl Rs {
    /// `rs(P)`: the author's inputs from the project's JSON.
    pub fn read(raw: &JsonValue) -> Rs {
        let r = js::get(Some(raw), "report").filter(|r| r.is_object());
        let g = |k: &str| js::get(r, k);
        let scope_key = match g("scope") {
            Some(JsonValue::String(s)) if scope(s).is_some() => s.clone(),
            _ => String::new(),
        };
        let staging = g("staging").filter(|s| s.truthy()).and_then(|s| {
            let mut x = arrs(s, &["rows", "periods", "existing", "existingWhy", "steps"])?;
            if let Some(t) = x.get("trigger").filter(|t| t.truthy()).cloned() {
                x.set(
                    "trigger",
                    arrs(&t, &["sites", "names", "why", "how"]).unwrap_or(JsonValue::Null),
                );
            }
            Some(x)
        });
        let sens = g("sens")
            .filter(|s| s.truthy())
            .and_then(|s| arrs(s, &["rows", "sentences", "flags"]));
        Rs {
            details: obj_or_empty(g("details")),
            scope: scope_key,
            signoff: obj_or_empty(g("signoff")),
            revisions: objects(g("revisions")),
            engagement: objects(g("engagement")),
            text: obj_or_empty(g("text")),
            sel: obj_or_empty(g("sel")),
            mitigation: obj_or_empty(g("mitigation")),
            ssm: obj_or_empty(g("ssm")),
            crossings: objects(g("crossings")),
            ped: match g("ped") {
                Some(p) if p.is_object() => p.clone(),
                _ => {
                    let mut o = JsonValue::object();
                    o.set("Sp", JsonValue::from(super::consts::ped::SP));
                    o.set("ts", JsonValue::from(super::consts::ped::TS));
                    o
                }
            },
            staging,
            sens,
        }
    }

    /// `details[k]`.
    pub fn detail(&self, k: &str) -> Option<&JsonValue> {
        self.details.get(k)
    }

    /// `text[id]`: the author's text for a box, as the loose value.
    pub fn text_of(&self, id: &str) -> Option<&JsonValue> {
        self.text.get(id)
    }

    /// `!!String(text[id] || '').trim()`: the box has text.
    pub fn has_text(&self, id: &str) -> bool {
        match self.text.get(id) {
            Some(v) if v.truthy() => !crate::jsnum::js_trim(&v.js_string()).is_empty(),
            _ => false,
        }
    }

    /// `signedOff(P)`: a named reviewer has ticked the report off.
    pub fn signed_off(&self) -> bool {
        self.signoff
            .get("reviewed")
            .is_some_and(|v| matches!(v, JsonValue::Bool(true)))
            && !crate::jsnum::js_trim(&js::text_or_empty(self.signoff.get("reviewer"))).is_empty()
    }
}
