//! The committed-development register: model.js `Register`.
//!
//! Rows list committed developments and network projects (consents, fast-track approvals, plan changes,
//! precinct yields, funded or unfunded roads). [`in_baseline`] says whether a row belongs in the future
//! baseline (do-minimum) and why (AT ITA Guidelines s3.6.1); [`table`] is the report's register table.

use super::{FUNDING, REGISTER_TYPES, name_of};
use crate::jsnum::{JsNum, Maybe, Num};
use crate::project::{Project, RegisterRow};
use serde_json::{Map, Value, json};

pub use crate::project::in_baseline;

fn text(t: &str) -> Num {
    Maybe::Set(JsNum::Text(t.to_string()))
}

/// A saved scalar as JSON: None when the key is missing (JSON.stringify leaves it out), null when null.
pub fn num_json(n: &Num) -> Option<Value> {
    match n {
        Maybe::Missing => None,
        Maybe::Null => Some(Value::Null),
        Maybe::Set(v) => Some(v.to_value()),
    }
}

/// `Register.presets(asAt)`: the South Auckland rows of Spare Construct's regulatory spec, each tagged "verify" with its
/// as-at date (default 2026-10-08). They have not been checked against primary sources.
pub fn presets(as_at: &str) -> Vec<RegisterRow> {
    let src =
        "Spare Construct regulatory spec (reg-receiving-env), not checked against primary sources";
    // The row every preset starts from; the callers then set their own keys
    let row = |id: &str, name: &str, row_type: &str| RegisterRow {
        id: Maybe::Set(format!("preset:{id}")),
        reference: text(""),
        name: text(name),
        row_type: Maybe::Set(row_type.to_string()),
        yield_: Maybe::Null,
        rates: Maybe::Null,
        status: text(""),
        likely: text("possible"),
        funding: Maybe::Null,
        source: text(src),
        date: text(as_at),
        as_at: text(as_at),
        verify: true.into(),
        preset: true.into(),
        zones: Maybe::Set(Vec::new()),
        links: Maybe::Set(Vec::new()),
        include: Maybe::Missing,
        note: Maybe::Missing,
        designation: Maybe::Missing,
        extra: Map::new(),
    };
    /* Mill Road: NZTA designation 6786 (Mill Road Redoubt Road Spare Construct Project). Its lapse-date extension under RMA
    s184 is listed as confirmed, plan updated 21/09/2026, in Auckland Council's AUP plan modification schedule. The
    designation is on record; funding is not. */
    let mill = json!({
        "id": "6786",
        "name": "Mill Road Redoubt Road Spare Construct Project",
        "authority": "New Zealand Transport Agency",
        "lapseExtension": "confirmed",
        "confirmed": "2026-09-21",
        "basis": "RMA s184 extension of the lapse date, confirmed and plan updated 21/09/2026",
        "source": "Auckland Council, Auckland Unitary Plan plan modification schedule (2026)",
        "verified": true,
    });
    let mill_note = "Designation 6786 (NZTA): lapse-date extension confirmed 21 Sep 2026 (AUP plan modification schedule).";
    let mut r1 = row("mill-road-1", "Mill Road Stage 1", "network");
    r1.status = text("Fast-track");
    r1.funding = Maybe::Set("unfunded".to_string());
    r1.designation = Maybe::Set(mill.clone());
    r1.note = text(&format!(
        "Fast-track; construction funding not committed. {mill_note}"
    ));
    let mut r2 = row("mill-road-2-3", "Mill Road Stages 2\u{2013}3", "network");
    r2.funding = Maybe::Set("unfunded".to_string());
    r2.designation = Maybe::Set(mill);
    r2.note = text(mill_note);
    let mut r3 = row(
        "westfield-pukekohe",
        "Westfield\u{2013}Pukekohe four-tracking",
        "network",
    );
    r3.note = text("Funding status to be confirmed.");
    let mut r4 = row("sunfield", "Sunfield", "ftaa");
    r4.status = text("Secondary-source status; High Court appeal");
    let r5 = row("drury-metro", "Drury Metropolitan Centre", "precinct");
    vec![r1, r2, r3, r4, r5]
}

/// The default as-at date of [`presets`].
pub const PRESET_AS_AT: &str = "2026-10-08";

/// `Register.table(P)`: report section 2.8, every row with "Included in S1" and the reason. Each row is
/// { id, name, type, status, funding, likely, source, asAt, verify, included, reason }.
pub fn table(p: &Project) -> Vec<Value> {
    let Some(rows) = p.register.get() else {
        return Vec::new();
    };
    rows.iter()
        .map(|r| {
            let (included, reason) = in_baseline(Some(r));
            let mut o = Map::new();
            if let Some(v) = r.id.get() {
                o.insert("id".into(), json!(v));
            }
            if let Some(v) = num_json(&r.name) {
                o.insert("name".into(), v);
            }
            // REGISTER_TYPES[r.type] || r.type
            match &r.row_type {
                Maybe::Set(t) => {
                    o.insert(
                        "type".into(),
                        json!(name_of(&REGISTER_TYPES, t).unwrap_or(t)),
                    );
                }
                Maybe::Null => {
                    o.insert("type".into(), Value::Null);
                }
                Maybe::Missing => {}
            }
            let or_empty = |n: &Num| -> Value {
                if n.truthy() {
                    num_json(n).unwrap_or(Value::Null)
                } else {
                    json!("")
                }
            };
            o.insert("status".into(), or_empty(&r.status));
            // r.funding ? FUNDING[r.funding] || r.funding : ''
            let funding = match r.funding.as_str() {
                Some(f) if !f.is_empty() => name_of(&FUNDING, f).unwrap_or(f).to_string(),
                _ => String::new(),
            };
            o.insert("funding".into(), json!(funding));
            o.insert("likely".into(), or_empty(&r.likely));
            o.insert("source".into(), or_empty(&r.source));
            // r.asAt || r.date || ''
            let as_at = if r.as_at.truthy() {
                or_empty(&r.as_at)
            } else {
                or_empty(&r.date)
            };
            o.insert("asAt".into(), as_at);
            o.insert("verify".into(), json!(r.verify.truthy()));
            o.insert("included".into(), json!(included));
            o.insert("reason".into(), json!(reason));
            Value::Object(o)
        })
        .collect()
}
