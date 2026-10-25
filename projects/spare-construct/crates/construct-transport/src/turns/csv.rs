//! Turning movement CSV: turns.js `Turns.sidraCSV`, `sidraExport` and `turnCountCSV`.

use super::{MV, MovementRow, junction_name, rows_for, site_rows};
use crate::edit::collate::locale_compare_f64;
use crate::graph::{js_sort_by, js_str_lt};
use crate::jsnum::{math_min, math_round};
use crate::project::{Link, Scenario, Settings};
use crate::results::{Res, export_ready};
use crate::util::csv;
use serde_json::{Value, json};
use std::collections::HashMap;

/// turns.js SIDRA_HEAD: the columns of the SIDRA-friendly CSV.
pub const SIDRA_HEAD: [&str; 11] = [
    "Site",
    "Junction",
    "Approach",
    "Approach road",
    "Movement",
    "To road",
    "Light vehicles (veh/h)",
    "Heavy vehicles (veh/h)",
    "Total (veh/h)",
    "Period",
    "Scenario",
];

/// turns.js CODE_HEAD: the extra columns with `codes`.
pub const CODE_HEAD: [&str; 3] = ["Leg", "Turn designation", "Movement ID"];

/// What `Turns.sidraCSV(sc, res, nodeIds, opts)` is given beyond the junctions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidraOpts {
    /// Add the SIDRA leg, turn designation and movement ID columns (South, R2, S-R2).
    pub codes: bool,
    /// Every junction in the results, even when the scenario's extent test lists sites.
    pub all: bool,
    /// For [`sidra_export`]: make the CSV even when the runs are not ready.
    pub force: bool,
}

/// Rounds total and heavy vehicles to whole vehicles; light = total - heavy so each row adds up: (light, heavy, total).
fn split(r: &MovementRow) -> (f64, f64, f64) {
    let t = math_round(r.v);
    let hv = math_min(t, math_round(r.hv));
    (t - hv, hv, t)
}

/// The links at each node (a road that starts and ends at one node counts once), as `linkIndex(sc)`.
fn link_index(sc: &Scenario) -> HashMap<&str, Vec<&Link>> {
    let mut m: HashMap<&str, Vec<&Link>> = HashMap::new();
    for l in sc.links.values() {
        if l.a == l.b {
            m.entry(l.a.as_str()).or_default().push(l);
        } else {
            m.entry(l.a.as_str()).or_default().push(l);
            m.entry(l.b.as_str()).or_default().push(l);
        }
    }
    m
}

/// `Turns.sidraCSV(sc, res, nodeIds, opts)`: the turning movements as CSV for SIDRA, one row per movement (site, junction, approach,
/// approach road, movement, exit road, light, heavy and total vehicles, period, scenario; with `codes` the leg, turn designation and
/// movement ID). `res` is one period's result or several (AM, PM). `node_ids` are node ids and / or junction-site ids; without them the
/// export covers the sites the scenario's extent test included when there are any (`sc.extent.sites`), otherwise every junction in
/// the results, sorted by name; `opts.all` forces every junction.
pub fn sidra_csv(
    sc: Option<&Scenario>,
    res: &[&Res],
    s: &Settings,
    node_ids: Option<&[String]>,
    opts: &SidraOpts,
) -> String {
    let mut head: Vec<Value> = SIDRA_HEAD.iter().map(|h| json!(h)).collect();
    if opts.codes {
        head.extend(CODE_HEAD.iter().map(|h| json!(h)));
    }
    let mut out: Vec<Vec<Value>> = vec![head];
    let Some(sc) = sc else {
        return csv(&out);
    };
    let idx = link_index(sc);
    let ext: Option<Vec<String>> = if node_ids.is_none() && !opts.all {
        sc.extent
            .get()
            .and_then(|e| e.get("sites"))
            .and_then(Value::as_array)
            .filter(|a| !a.is_empty())
            .map(|a| a.iter().map(crate::jsnum::value_to_js_string).collect())
    } else {
        None
    };
    let want: Option<Vec<String>> = node_ids.map(|n| n.to_vec()).or(ext);
    for r in res {
        let is_site = |id: &str| r.sites.as_ref().is_some_and(|m| m.contains_key(id));
        let nm = |id: &str| -> String {
            if is_site(id) {
                r.sites
                    .as_ref()
                    .expect("a site")
                    .get(id)
                    .expect("site")
                    .name
                    .clone()
            } else {
                junction_name(sc, id, Some(idx.get(id).map_or(&[][..], |v| &v[..])))
            }
        };
        let candidates: Vec<String> = match &want {
            Some(w) => w.clone(),
            None => r.turns.keys().map(str::to_string).collect(),
        };
        let mut ids: Vec<String> = candidates
            .into_iter()
            .filter(|id| {
                is_site(id) || (sc.nodes.contains_key(id.as_str()) && r.turns.contains_key(id))
            })
            .collect();
        if want.is_none() {
            let names: HashMap<String, String> = ids.iter().map(|i| (i.clone(), nm(i))).collect();
            js_sort_by(&mut ids, |p, q| {
                let d = locale_compare_f64(&names[p], &names[q]);
                if d != 0.0 {
                    d
                } else if js_str_lt(p, q) {
                    -1.0
                } else if js_str_lt(q, p) {
                    1.0
                } else {
                    0.0
                }
            });
        }
        for id in &ids {
            let rows = if is_site(id) {
                site_rows(sc, Some(r), s, id).rows
            } else {
                rows_for(
                    sc,
                    Some(r),
                    s,
                    id,
                    Some(idx.get(id.as_str()).map_or(&[][..], |v| &v[..])),
                )
                .rows
            };
            let name = nm(id);
            for m in &rows {
                let (lv, hv, t) = split(m);
                let mut row: Vec<Value> = vec![
                    json!(id),
                    json!(name),
                    json!(m.approach_name),
                    json!(m.road),
                    json!(m.mv),
                    json!(m.to_road),
                    json!(lv),
                    json!(hv),
                    json!(t),
                    json!(r.period),
                    json!(sc.name.str_or("")),
                ];
                if opts.codes {
                    row.push(json!(m.leg_name));
                    row.push(json!(m.turn));
                    row.push(json!(m.id));
                }
                out.push(row);
            }
        }
    }
    csv(&out)
}

/// What [`sidra_export`] returns.
#[derive(Clone, Debug, PartialEq)]
pub struct SidraExport {
    /// The CSV is ready to use.
    pub ok: bool,
    /// Why not (or what to check when forced).
    pub why: String,
    /// The CSV (None when refused).
    pub csv: Option<String>,
}

/// `Turns.sidraExport(sc, res, nodeIds, opts)`: the export gate (docs/spec/convergence): turn volumes for SIDRA need a well-converged
/// run (gap 0.01% or better) that is not out of date. With `opts.force` the CSV is made anyway and `why` still explains the problem.
pub fn sidra_export(
    sc: Option<&Scenario>,
    res: &[&Res],
    s: &Settings,
    node_ids: Option<&[String]>,
    opts: &SidraOpts,
) -> SidraExport {
    if res.is_empty() {
        return SidraExport {
            ok: false,
            why: "Run the model first.".to_string(),
            csv: None,
        };
    }
    let bad = res
        .iter()
        .map(|r| (*r, export_ready(Some(r))))
        .find(|(_, c)| !c.ok);
    let why = match bad {
        Some((r, c)) => format!(
            "{} result: {}",
            if r.period.is_empty() { "A" } else { &r.period },
            c.why
        ),
        None if res.iter().any(|r| r.stale) => {
            "Results are out of date. Run the model again.".to_string()
        }
        None => String::new(),
    };
    if !why.is_empty() && !opts.force {
        return SidraExport {
            ok: false,
            why,
            csv: None,
        };
    }
    SidraExport {
        ok: why.is_empty(),
        why,
        csv: Some(sidra_csv(sc, res, s, node_ids, opts)),
    }
}

/// `Turns.turnCountCSV(sc, res, nodeId)`: a classic turning-count sheet for one junction: a row per approach with its left, through,
/// right and U-turn counts, totals and heavy vehicles, and a total row.
pub fn turn_count_csv(
    sc: Option<&Scenario>,
    res: Option<&Res>,
    s: &Settings,
    node_id: &str,
) -> String {
    let m = match sc {
        Some(sc) => rows_for(sc, res, s, node_id, None),
        None => super::Movements {
            rows: Vec::new(),
            arms: Vec::new(),
        },
    };
    let jname = match sc {
        Some(sc) if sc.nodes.contains_key(node_id) => junction_name(sc, node_id, None),
        _ => "Unknown junction".to_string(),
    };
    let period = match res {
        Some(r) if !r.period.is_empty() => format!("{} peak hour", r.period),
        _ => String::new(),
    };
    let mut out: Vec<Vec<Value>> = vec![
        vec![json!("Junction"), json!(jname)],
        vec![
            json!("Scenario"),
            json!(sc.map_or("", |s| s.name.str_or(""))),
        ],
        vec![json!("Period"), json!(period)],
        vec![
            json!("Units"),
            json!("veh/h (heavy vehicles included in totals)"),
        ],
        vec![],
        {
            let mut h = vec![json!("Approach"), json!("Road")];
            h.extend(MV.iter().map(|m| json!(m)));
            h.push(json!("Total"));
            h.push(json!("Heavy vehicles"));
            h
        },
    ];
    // sum: L, T, R, U, total, heavy
    let mut sum = [0.0f64; 6];
    for a in m.arms.iter().filter(|x| x.is_in) {
        let mut c = [0.0f64; 6];
        for r in &m.rows {
            if r.approach == a.in_key {
                let (_, hv, t) = split(r);
                let k = MV.iter().position(|x| *x == r.mv).expect("a movement");
                c[k] += t;
                c[4] += t;
                c[5] += hv;
            }
        }
        for k in 0..6 {
            sum[k] += c[k];
        }
        let mut row = vec![json!(a.name), json!(a.road)];
        row.extend(c.iter().map(|x| json!(x)));
        out.push(row);
    }
    let mut total = vec![json!("Total"), json!("")];
    total.extend(sum.iter().map(|x| json!(x)));
    out.push(total);
    csv(&out)
}
