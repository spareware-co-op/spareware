//! The projections of the calibration panel's results the generator records (tools/transport-truth/gen-calibrate.mjs,
//! part 2: `countRowOut`, `evOut`, `assessOut`, `siteRowsOut` and the rest), built from the Rust results.
#![allow(dead_code)]

use super::{pn, po};
use construct_transport::calibrate::*;
use construct_transport::datahub::JtCheck;
use construct_transport::jsnum::Num;
use serde_json::{Map, Value, json};

/// `countRowOut`.
pub fn count_row_out(r: &CountRow) -> Value {
    json!({ "id": r.c.id, "link": r.mt.link, "dir": r.mt.dir, "d": pn(r.mt.d), "obs": pn(r.obs), "model": pn(r.model), "how": r.how, "geh": pn(r.geh),
        "method": r.method, "role": r.role, "scored": r.scored, "inCore": r.in_core, "mw": r.mw, "cls": r.link_cls })
}

/// A statistic set as the generator writes it (`plain(statsOf(rows))`).
pub fn cal_stats_out(s: &CalStats) -> Value {
    serde_json::to_value(s).expect("stats")
}

/// `slRowOut`.
pub fn sl_row_out(r: &SlRow) -> Value {
    serde_json::to_value(r).expect("row")
}

/// `slObsOut`.
pub fn sl_obs_out(s: &SlSummary, store: &CalibStore) -> Value {
    let mut m = Map::new();
    m.insert("n".into(), json!(s.n));
    let keys = ["sgeh5", "sgeh75", "sgeh10", "sgeh4", "sw5", "sw10", "sw15"];
    let mut cnt = Map::new();
    for (k, c) in keys.iter().zip(s.cnt.iter()) {
        cnt.insert(k.to_string(), json!(c));
    }
    m.insert("cnt".into(), Value::Object(cnt));
    let rows: Vec<Value> = s
        .rows
        .iter()
        .map(|(i, r)| {
            let mut o = serde_json::to_value(r).expect("row");
            o["slId"] = json!(store.screenlines[*i].id);
            o
        })
        .collect();
    m.insert("rows".into(), Value::Array(rows));
    m.insert("dropped".into(), json!(s.dropped));
    for k in keys {
        m.insert(k.to_string(), po(s.share(k)));
    }
    Value::Object(m)
}

/// `jrOut`.
pub fn jr_out(r: &JourneyRow) -> Value {
    json!({ "name": r.name, "model": po(r.model), "len": po(r.len),
        "obs": r.obs.as_ref().map_or(Value::Null, |(t, l)| json!({ "t": pn(*t), "label": l })),
        "google": r.google.as_ref().map_or(Value::Null, |(t, w)| json!({ "t": pn(*t), "when": w.clone().unwrap_or(Value::Null) })),
        "chk": r.chk.as_ref().map_or(Value::Null, |c: &JtCheck| serde_json::to_value(c).expect("chk")),
        "refObserved": r.ref_observed, "short": r.short, "long": r.long })
}

/// `jtStats` as the generator writes it.
pub fn jt_stats_out(j: &JtStats) -> Value {
    json!({ "n": j.n, "cnt": { "jt1": j.jt1_n, "jt2": j.jt2_n }, "jt1": po(j.jt1), "jt2": po(j.jt2), "predicted": j.predicted })
}

/// `convOut`.
pub fn conv_out(v: &ConvStats) -> Value {
    if v.n == 0 {
        return json!({ "n": 0 });
    }
    json!({ "n": v.n, "gapPct": pn(v.gap_pct), "gap": pn(v.gap), "it": pn(v.it), "converged": v.converged, "gapTol": pn(v.gap_tol), "note": v.note,
        "p4": v.p4, "lastP": po(v.last_p) })
}

fn pair_out(p: &PairVal) -> Value {
    json!({ "ok": p.ok, "text": p.text })
}

/// `meOut`.
pub fn me_out(m: &Option<MeStats>) -> Value {
    match m {
        None => json!({ "n": 0 }),
        Some(m) => {
            json!({ "n": m.n, "totalAbs": pn(m.total_abs), "totalPct": pn(m.total_pct), "cells": pair_out(&m.cells), "ends": pair_out(&m.ends),
            "tld": m.tld.as_ref().map_or(Value::Null, pair_out), "cr": po(m.cr) })
        }
    }
}

/// `turnRowOut`.
pub fn turn_row_out(r: &TurnCheckRow) -> Value {
    let mut o = json!({ "siteId": r.site_id, "leg": r.leg, "mv": r.mv, "key": r.key, "obs": pn(r.obs), "model": po(r.model), "geh": po(r.geh),
        "arm": r.arm, "estimated": r.estimated });
    if !r.estimated {
        o["balanced"] = json!(r.balanced);
    }
    o
}

/// `coreOut`.
pub fn core_out(k: &Core) -> Value {
    json!({ "kind": k.kind, "label": k.label,
        "r": if k.kind == "buffer" { pn(k.r) } else { Value::Null },
        "lines": if k.kind == "buffer" { json!(k.lines.len()) } else { Value::Null },
        "polyN": if k.kind == "user" { json!(k.poly.len()) } else { Value::Null } })
}

/// `evOut`.
pub fn ev_out(ev: &Evidence, store: &CalibStore, with_all: bool) -> Value {
    json!({ "period": ev.period, "set": ev.set, "requested": ev.requested, "area": ev.area, "core": core_out(&ev.core), "infoOnly": ev.info_only,
        "fallback": ev.fallback, "validN": ev.valid_n,
        "rows": ev.rows.iter().map(|r| json!(r.c.id)).collect::<Vec<_>>(),
        "all": ev.all.iter().map(|r| if with_all { count_row_out(r) } else { json!(r.c.id) }).collect::<Vec<_>>(),
        "link": cal_stats_out(&ev.link), "measured": cal_stats_out(&ev.measured), "turn": cal_stats_out(&ev.turn),
        "turnRows": ev.turn_rows.iter().map(turn_row_out).collect::<Vec<_>>(), "screen": sl_obs_out(&ev.screen, store),
        "jt": jt_stats_out(&ev.jt), "jtRows": ev.jt_rows.iter().map(jr_out).collect::<Vec<_>>(), "conv": conv_out(&ev.conv), "me": me_out(&ev.me),
        "baseYear": ev.base_year, "year": po(ev.year), "hasRes": ev.has_res })
}

fn val_out(v: &Option<Val>) -> Value {
    match v {
        None => Value::Null,
        Some(Val::Num(x)) => pn(*x),
        Some(Val::Bool(b)) => json!(b),
        Some(Val::Pair(p)) => pair_out(p),
    }
}

/// `evalOut`.
pub fn eval_out(e: &Eval) -> Value {
    let mut o = Map::new();
    o.insert("id".into(), json!(e.t.id));
    o.insert("n".into(), json!(e.n));
    o.insert("value".into(), val_out(&e.value));
    o.insert("got".into(), e.got.map_or(Value::Null, |x| json!(x)));
    o.insert("need".into(), e.need.map_or(Value::Null, |x| json!(x)));
    o.insert("ok".into(), e.ok.map_or(Value::Null, |x| json!(x)));
    o.insert("status".into(), json!(e.status));
    o.insert("shown".into(), json!(e.shown));
    if e.predicted {
        o.insert("predicted".into(), json!(true));
    }
    if let Some(c) = e.clarify {
        o.insert("clarify".into(), json!(c));
    }
    if let Some(v) = &e.verdict {
        o.insert("verdict".into(), json!(v));
    }
    if e.indicative {
        o.insert("indicative".into(), json!(true));
    }
    Value::Object(o)
}

/// `testOut`.
pub fn test_out(t: &Test) -> Value {
    json!({ "id": t.id, "group": t.group, "on": t.on, "stat": t.stat, "kind": t.kind, "min": po(t.min), "max": po(t.max), "lo": po(t.lo), "hi": po(t.hi),
        "clarify": po(t.clarify), "label": t.label, "crit": t.crit, "table": t.table, "src": t.src, "use": t.use_, "verified": t.verified,
        "useVerified": t.use_verified.map_or(Value::Null, |b| json!(b)) })
}

/// `assessOut`.
pub fn assess_out(a: &Assessment) -> Value {
    json!({ "period": a.period, "standard": a.standard, "category": a.category.to_string(), "status": a.status, "pass": a.pass.map_or(Value::Null, |b| json!(b)),
        "line": a.line, "also": a.also.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
        "fails": a.fails.iter().map(|f| json!({ "cat": f.cat.to_string(), "test": f.test, "got": f.got, "target": f.target })).collect::<Vec<_>>(),
        "rows": a.rows.iter().map(eval_out).collect::<Vec<_>>() })
}

/// `siteRowsOut`.
pub fn site_rows_out(r: &SiteRowsResult, site_id: &str) -> Value {
    let vol = r.vol.as_ref().map_or(Value::Null, |v| {
        let mv: Map<String, Value> = v.mv.iter().map(|(k, x)| (k.to_string(), pn(*x))).collect();
        let mut o = json!({ "mv": mv, "estimated": v.estimated,
            "furness": v.furness.as_ref().map_or(Value::Null, |f| {
                let m: Map<String, Value> = v.legs.iter().enumerate().map(|(i, li)| {
                    let row: Map<String, Value> = v.legs.iter().enumerate().map(|(j, lj)| (lj.clone(), pn(f.m[i][j]))).collect();
                    (li.clone(), Value::Object(row))
                }).collect();
                json!({ "m": m, "it": f.it, "err": pn(f.err), "converged": f.converged, "outsScaled": f.outs_scaled })
            }) });
        if !v.estimated {
            o["balanced"] = json!(v.balanced);
        }
        o
    });
    json!({ "siteId": site_id, "node": r.node, "rows": r.rows.iter().map(turn_row_out).collect::<Vec<_>>(),
        "flags": if r.vol.is_some() { json!(r.flags) } else { Value::Null }, "note": r.note,
        "kind": r.view.as_ref().map_or(Value::Null, |v| json!(v.kind)),
        "members": r.view.as_ref().map_or(Value::Null, |v| json!(v.members)),
        "arms": r.view.as_ref().map_or(Value::Null, |v| Value::Array(v.arms.iter().map(|a| json!({ "inKey": a.in_key, "outKey": a.out_key, "b": pn(a.b), "road": a.road, "links": a.links })).collect())),
        "vrows": r.view.as_ref().map_or(Value::Null, |v| Value::Array(v.rows.iter().map(|m| json!({ "approach": m.approach, "to": m.to, "mv": m.mv, "turn": m.turn, "v": pn(m.v) })).collect())),
        "vol": vol })
}

/// A number field of a loose value, for the few places a record holds one.
pub fn num_value(n: &Num) -> Value {
    match n {
        construct_transport::jsnum::Maybe::Set(j) => j.to_value(),
        _ => Value::Null,
    }
}
