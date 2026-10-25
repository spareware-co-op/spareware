//! Climate hazard assessment table (compliance-extra.js section 18). The hazard rows come from the project's stored hazard feed
//! (`feeds.hazards[<scenario id>].rows`); the live feed is not part of this port.

use super::ctx::{active_sc, esc, stamped};
use super::packs::t_table;
use crate::compliance::html::{ReportCtx, rt};
use crate::compliance::jv::{Jx, V, arr, csv, jobj, st};
use crate::util::fmt;

/// `climate(P, sc)`: the roads in mapped hazard areas for a scenario (the active one when None): `{ rows, loaded, source, note }`; each row is the
/// feed's row with `vest` (a proposed road that will be vested) and `assess` (what to do about it).
pub fn climate(p: &V, sc: Option<&V>) -> V {
    let sc = sc.unwrap_or_else(|| active_sc(p));
    let f = p.g("feeds").g("hazards").g(&sc.g("id").s());
    let rows: Option<&V> = if f.t() && f.g("rows").is_arr() { Some(f.g("rows")) } else { None };
    let out: Vec<V> = rows
        .map(|r| r.items())
        .unwrap_or(&[])
        .iter()
        .map(|r| {
            let l = sc.g("links").g(&r.g("link").s());
            let vest = if l.t() { Some(l.g("status").eq_s("proposed")) } else { None };
            let assess = if r.g("only").t() {
                "Only access: confirm an alternative route in a flood event or raise the road above the design flood level"
            } else if vest == Some(true) {
                "Proposed road (asset to vest): design levels for the climate-adjusted event"
            } else {
                "Relied-on road: note the hazard and the detour"
            };
            let mut o = if r.is_obj() { r.clone() } else { V::object() };
            match vest {
                Some(v) => o.set("vest", V::Bool(v)),
                None => {
                    o.remove("vest");
                }
            }
            o.set("assess", st(assess));
            o
        })
        .collect();
    jobj! {
        "rows" => arr(out),
        "loaded" => rows.is_some(),
        "source" => "Auckland Council flood plains and coastal inundation (feeds lane)",
        "note" => "Length is the part of each road centreline inside the hazard area (AT ITA s4.5).",
    }
}

/// `climateCSV(P)`: the hazard table as a stamped CSV.
pub fn climate_csv(p: &V) -> String {
    let c = climate(p, None);
    let s = |x: &str| st(x);
    let mut rows: Vec<Vec<V>> = vec![
        vec![s("Climate adaptation (AT ITA s4.5)"), c.g("note").clone()],
        ["Link", "Future Connect role", "Length in hazard (m)", "Hazard layer", "Only access", "Assessment"].iter().map(|h| s(h)).collect(),
    ];
    for r in c.g("rows").items() {
        rows.push(vec![r.g("name").clone(), r.g("fc").clone(), r.g("len").clone(), r.g("layer").clone(), s(if r.g("only").t() { "Y" } else { "N" }), r.g("assess").clone()]);
    }
    stamped(&csv(&rows), p, None)
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-climate"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id != "compliance-extra-climate" {
        return None;
    }
    let x = climate(p, None);
    if !x.g("loaded").t() {
        return Some(String::new());
    }
    let rows: Vec<Vec<String>> = x
        .g("rows")
        .items()
        .iter()
        .map(|r| {
            vec![
                esc(r.g("name")),
                esc(r.g("fc")),
                if r.has("len") { fmt::int(r.g("len").n()) } else { fmt::DASH.to_string() },
                esc(r.g("layer")),
                if r.g("only").t() { "Yes" } else { "No" }.to_string(),
                esc(r.g("assess")),
            ]
        })
        .collect();
    let tbl = t_table(&["Link", "FC role", "Length in hazard (m)", "Hazard layer", "Only access", "Assessment"], rows, "No assessed road lies in a mapped hazard area.");
    Some(rt(ctx, "Assessed roads in flood plains and coastal inundation areas", &tbl, &format!("{} Source: {}.", esc(x.g("note")), esc(x.g("source")))))
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-climate"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    (id == "compliance-extra-climate").then(|| climate_csv(p))
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    (id == "compliance-extra-climate").then(|| climate(p, None).g("loaded").t())
}
