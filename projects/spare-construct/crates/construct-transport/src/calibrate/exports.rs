//! The calibration panel's CSV exports: counts against the model, the criteria summary, screenline validation and
//! turn validation (`Calibration.countsCSV`, `summaryCSV`, `screenlinesCSV`, `turnsCSV`).

use super::assess::{EvOpts, assess};
use super::criteria::{GEH_BANDS_PANEL, SOURCES, criteria_source, standard_name, tag_band, target_text, tmdg_band};
use super::roles::Ctx;
use super::rows::count_rows;
use super::screen::{sl_crossings, sl_rows};
use super::turncounts::ci_of;
use super::turnrows::site_rows;
use crate::datahub::Count;
use crate::jsnum::{json_number, math_round};
use crate::project::Scenario;
use crate::results::Res;
use crate::util::{csv, r1, r3};
use serde_json::Value;

/// A byte order mark so spreadsheets read the file as UTF-8.
pub const BOM: &str = "\u{feff}";

fn s(x: &str) -> Value {
    Value::String(x.to_string())
}

fn n(x: f64) -> Value {
    json_number(x)
}

fn raw(x: &crate::jsnum::Num) -> Value {
    match x {
        crate::jsnum::Maybe::Set(j) => j.to_value(),
        _ => Value::Null,
    }
}

// `a || b`: the first value that is truthy, else the second as it is
fn or_cell(a: &crate::jsnum::Num, b: &crate::jsnum::Num) -> Value {
    if a.truthy() { raw(a) } else { raw(b) }
}

/// `nDays(c)`: the number of counted days: `n_days`, else the number in a note such as "7 counted days"; None when there is none.
pub fn n_days(c: &Count) -> Option<Value> {
    if !c.n_days.is_nullish() {
        return Some(raw(&c.n_days));
    }
    let note = if c.note.truthy() { c.note.js_string() } else { String::new() };
    let t: Vec<char> = note.chars().collect();
    let pat: Vec<char> = " counted days".chars().collect();
    let mut i = 0;
    while i < t.len() {
        if t[i].is_ascii_digit() {
            let mut j = i;
            while j < t.len() && t[j].is_ascii_digit() {
                j += 1;
            }
            if t[j..].starts_with(&pat[..]) {
                let d: String = t[i..j].iter().collect();
                return Some(n(d.parse::<f64>().unwrap_or(f64::NAN)));
            }
            i = j;
        } else {
            i += 1;
        }
    }
    None
}

/// `countsCSV(sc)`: every counted site against the model in each period that has a result, with GEH, bands, role, screenlines it is on and the match; text with a byte order mark. `res_of` gives the scenario's result for a period.
pub fn counts_csv(ctx: &Ctx, sc: &Scenario, res_of: &dyn Fn(&str) -> Option<Res>) -> String {
    let head = [
        "site_id",
        "source",
        "source_ref",
        "road",
        "dir",
        "period",
        "observed_veh_h",
        "obs_method",
        "count_date",
        "n_days",
        "ci95",
        "role",
        "in_core",
        "modelled_veh_h",
        "diff",
        "diff_pct",
        "geh",
        "geh_band",
        "tmdg_band_pass",
        "tag_band_pass",
        "either_pass",
        "screenline_id",
        "link_id",
        "arc_dir",
        "match_dist_m",
        "road_class",
    ];
    let mut out: Vec<Vec<Value>> = vec![head.iter().map(|h| s(h)).collect()];
    let sls: Vec<&super::store::Screenline> = ctx.store.screenlines.iter().filter(|x| !x.is_type("core")).collect();
    for p in ["AM", "PM"] {
        let Some(res) = res_of(p) else { continue };
        for r in count_rows(ctx, sc, p, Some(&res), false) {
            let c = &r.c;
            let band = GEH_BANDS_PANEL.iter().find(|(lo, hi)| r.geh >= *lo && r.geh < *hi);
            let sl_ids: Vec<&str> = sls
                .iter()
                .filter(|x| sl_crossings(sc, x).iter().any(|k| k.link == r.mt.link))
                .map(|x| x.id.as_str())
                .collect();
            let ci = ci_of(c);
            let tb = tmdg_band(r.model, r.obs);
            let gb = tag_band(r.model, r.obs);
            out.push(vec![
                s(&c.id),
                raw(&c.src),
                if c.reference.truthy() { raw(&c.reference) } else { s("") },
                or_cell(&c.road, &c.name),
                if c.dir.truthy() { raw(&c.dir) } else { s("both") },
                s(p),
                n(r1(r.obs)),
                s(r.method),
                if c.date.truthy() { raw(&c.date) } else { s("") },
                n_days(c).unwrap_or_else(|| s("")),
                ci.map_or_else(|| s(""), |x| x.1),
                s(r.role),
                n(if r.in_core { 1.0 } else { 0.0 }),
                n(r1(r.model)),
                n(r1(r.model - r.obs)),
                if r.obs > 0.0 { n(r1(100.0 * (r.model - r.obs) / r.obs)) } else { s("") },
                n(r3(r.geh)),
                match band {
                    Some((lo, hi)) => {
                        if *hi == f64::INFINITY {
                            s("12+")
                        } else {
                            s(&format!("{}-{}", crate::jsnum::number_to_js_string(*lo), crate::jsnum::number_to_js_string(*hi)))
                        }
                    }
                    None => s(""),
                },
                n(if tb { 1.0 } else { 0.0 }),
                n(if gb { 1.0 } else { 0.0 }),
                n(if r.geh < 5.0 || gb { 1.0 } else { 0.0 }),
                s(&sl_ids.join(";")),
                s(&r.mt.link),
                s(r.mt.dir),
                n(math_round(r.mt.d)),
                r.link_cls.as_deref().map_or_else(|| s(""), s),
            ]);
        }
    }
    format!("{BOM}{}", csv(&out))
}

/// `summaryCSV(sc)`: the criteria table of each period that has a result: each test with its target, what was achieved and its status, then the overall line.
pub fn summary_csv(ctx: &Ctx, sc: &Scenario, res_of: &dyn Fn(&str) -> Option<Res>) -> String {
    let mut out: Vec<Vec<Value>> = vec![["criterion", "standard", "category", "period", "target", "achieved", "n", "status"].iter().map(|h| s(h)).collect()];
    for p in ["AM", "PM"] {
        let Some(res) = res_of(p) else { continue };
        let a = assess(ctx, sc, p, Some(&res), &EvOpts::default());
        for r in &a.rows {
            let src = criteria_source(r.t.src).unwrap_or(&SOURCES[0]);
            out.push(vec![
                s(&r.t.label),
                s(&format!("{}{}", src.short, if r.t.table.is_empty() { String::new() } else { format!(", {}", r.t.table) })),
                if a.standard == "tmdg" { s(&a.category.to_string()) } else { s("") },
                s(p),
                s(&target_text(&r.t)),
                s(&r.shown),
                n(r.n as f64),
                s(&r.status),
            ]);
        }
        out.push(vec![
            s("Overall"),
            if a.standard == "tmdg" { s("TMDG 2019") } else { standard_name(&a.standard).map_or(Value::Null, s) },
            s(&a.category.to_string()),
            s(p),
            s(""),
            s(&a.status),
            n(a.ev.link.n as f64),
            s(&a.line),
        ]);
    }
    format!("{BOM}{}", csv(&out))
}

/// `screenlinesCSV(sc)`: each screenline and cordon in each period that has a result, with and without motorways.
pub fn screenlines_csv(ctx: &Ctx, sc: &Scenario, res_of: &dyn Fn(&str) -> Option<Res>) -> String {
    let head = [
        "screenline",
        "type",
        "role",
        "period",
        "direction",
        "motorways",
        "crossing_arcs",
        "counted_arcs",
        "counted_veh_h",
        "modelled_counted_veh_h",
        "modelled_all_veh_h",
        "diff",
        "diff_pct",
        "geh",
        "completeness",
        "status",
    ];
    let mut out: Vec<Vec<Value>> = vec![head.iter().map(|h| s(h)).collect()];
    for sl in ctx.store.screenlines.iter().filter(|x| !x.is_type("core")) {
        for p in ["AM", "PM"] {
            let res = res_of(p);
            for mw in [true, false] {
                if !mw && !sl_crossings(sc, sl).iter().any(|x| x.mw) {
                    continue;
                }
                for r in sl_rows(ctx, sc, sl, p, res.as_ref(), mw) {
                    out.push(vec![
                        raw(&sl.name),
                        raw(&sl.kind),
                        raw(&sl.role),
                        s(p),
                        s(&r.label),
                        s(if mw { "with" } else { "without" }),
                        n(r.n as f64),
                        n(r.counted as f64),
                        n(r1(r.c)),
                        n(r1(r.mc)),
                        n(r1(r.m)),
                        n(r1(r.d)),
                        r.pct.map_or_else(|| s(""), |x| n(r1(100.0 * x))),
                        r.geh.map_or_else(|| s(""), |x| n(r3(x))),
                        n(r3(r.complete)),
                        s(&r.text),
                    ]);
                }
            }
        }
    }
    format!("{BOM}{}", csv(&out))
}

/// `turnsCSV(sc)`: each turning-count site's movements in each period against the model.
pub fn turns_csv(ctx: &Ctx, sc: &Scenario, res_of: &dyn Fn(&str) -> Option<Res>) -> String {
    let head = [
        "site",
        "site_ref",
        "date",
        "period",
        "peak_hour",
        "phf",
        "leg",
        "movement",
        "road",
        "counted_veh_h",
        "modelled_veh_h",
        "geh",
        "tmdg_band_pass",
        "tag_band_pass",
        "estimated",
        "balanced",
        "role",
        "flags",
    ];
    let mut out: Vec<Vec<Value>> = vec![head.iter().map(|h| s(h)).collect()];
    for st in &ctx.store.turn_sites {
        for p in ["AM", "PM"] {
            let res = res_of(p);
            let r = site_rows(ctx, sc, st, p, res.as_ref());
            let pk = st.peak.get().and_then(|k| k.get(p));
            for x in &r.rows {
                let mut flags: Vec<String> = st.flags.clone();
                flags.extend(r.flags.iter().cloned());
                out.push(vec![
                    raw(&st.name),
                    raw(&st.reference),
                    raw(&st.date),
                    s(p),
                    pk.map_or_else(|| s(""), |k| s(&format!("{}-{}", k.start, k.end))),
                    pk.and_then(|k| k.phf).map_or_else(|| s(""), n),
                    s(&x.leg),
                    s(&x.mv),
                    x.arm.as_deref().map_or_else(|| s(""), s),
                    n(r1(x.obs)),
                    x.model.map_or_else(|| s(""), |m| n(r1(m))),
                    x.geh.map_or_else(|| s(""), |g| n(r3(g))),
                    x.model.map_or_else(|| s(""), |m| n(if tmdg_band(m, x.obs) { 1.0 } else { 0.0 })),
                    x.model.map_or_else(|| s(""), |m| n(if tag_band(m, x.obs) { 1.0 } else { 0.0 })),
                    n(if x.estimated { 1.0 } else { 0.0 }),
                    n(if x.balanced { 1.0 } else { 0.0 }),
                    raw(&st.role),
                    s(&flags.join("; ")),
                ]);
            }
        }
    }
    format!("{BOM}{}", csv(&out))
}
