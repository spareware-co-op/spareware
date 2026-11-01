//! J2 summary and J3 comparison: one line a junction and period, and the changes between two scenarios.

use super::text::{ctl_name, d1, dos_cell, los_badge, n0, per_name, sgn, x2};
use super::{Entry, LABEL, Sc, Tools, entries, legs_of, periods_of_all, xid};
use crate::graph::js_sort_by;
use crate::jsnum::{math_max, math_max_of, number_to_js_string, plain_number};
use crate::project::Project;
use crate::results::Res;
use crate::util::esc;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// One period's summary of a junction: worst DoS, delay, level of service, longest queue and its leg.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    /// Worst degree of saturation.
    pub dos: f64,
    /// Practical degree of saturation; None when no approach has one.
    pub xp: Option<f64>,
    /// Delay (s).
    pub delay: f64,
    /// Basis of the delay.
    pub delay_basis: String,
    /// Level of service.
    pub los: String,
    /// Level of service as shown.
    pub los_text: String,
    /// Give way and stop: no intersection level of service.
    pub twsc: bool,
    /// Longest 95% queue (m).
    pub q95: f64,
    /// Its leg.
    pub q95_leg: String,
    /// The result is out of date.
    pub stale: bool,
}

impl Summary {
    /// As JSON.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("dos".into(), plain_number(self.dos));
        if let Some(x) = self.xp {
            m.insert("xp".into(), plain_number(x));
        }
        m.insert("delay".into(), plain_number(self.delay));
        m.insert("delayBasis".into(), json!(self.delay_basis));
        m.insert("los".into(), json!(self.los));
        m.insert("losText".into(), json!(self.los_text));
        m.insert("twsc".into(), json!(self.twsc));
        m.insert("q95".into(), plain_number(self.q95));
        m.insert("q95Leg".into(), json!(self.q95_leg));
        m.insert("stale".into(), json!(self.stale));
        Value::Object(m)
    }
}

/// `flyOf(r)`: what a table row flies to on the map.
pub(crate) fn fly_of(is_site: bool, id: &str) -> String {
    if is_site {
        format!("junction-site:{id}")
    } else {
        format!("node:{id}")
    }
}

/// A junction across the periods of a scenario (a row of J2).
#[derive(Clone, Debug)]
pub struct J2Row {
    /// Site id or node id.
    pub id: String,
    /// The stable id for exports.
    pub xid: String,
    /// A junction site.
    pub is_site: bool,
    /// Name.
    pub name: String,
    /// Control.
    pub kind: String,
    /// The controller's node.
    pub node: String,
    /// Member nodes.
    pub nodes: Vec<String>,
    /// Summary by period, in the order the periods were read.
    pub p: Vec<(String, Option<Summary>)>,
}

impl J2Row {
    fn get(&self, per: &str) -> Option<&Summary> {
        self.p
            .iter()
            .find(|(k, _)| k == per)
            .and_then(|(_, s)| s.as_ref())
    }
    /// As JSON.
    pub fn to_value(&self) -> Value {
        let mut p = Map::new();
        for (k, s) in &self.p {
            p.insert(k.clone(), s.as_ref().map_or(Value::Null, Summary::to_value));
        }
        json!({
            "id": self.id, "xid": self.xid, "kind": if self.is_site { "site" } else { "node" }, "name": self.name,
            "type": self.kind, "node": self.node, "nodes": self.nodes, "p": Value::Object(p),
        })
    }
}

/// One period's side of a changed junction (J3).
#[derive(Clone, Debug, PartialEq)]
pub struct J3Delta {
    /// Change in degree of saturation.
    pub dos: f64,
    /// As text.
    pub dos_text: String,
    /// Change in delay (s).
    pub delay: f64,
    /// As text.
    pub delay_text: String,
    /// Change in level of service ("\u{2013}" for none).
    pub los: String,
    /// Change in the longest queue (m).
    pub q95: f64,
    /// As text.
    pub q95_text: String,
}

/// Before, after and change for one junction and period.
#[derive(Clone, Debug)]
pub struct J3Cell {
    /// The "from" scenario's summary.
    pub a: Option<Summary>,
    /// The "to" scenario's summary.
    pub b: Option<Summary>,
    /// The change, when both exist.
    pub d: Option<J3Delta>,
}

/// A junction in the comparison.
#[derive(Clone, Debug)]
pub struct J3Row {
    /// Id (the "to" scenario's when there is one).
    pub id: String,
    /// Id in the "from" scenario.
    pub from_id: Option<String>,
    /// A junction site.
    pub is_site: bool,
    /// Name.
    pub name: String,
    /// Control.
    pub kind: String,
    /// "new", "gone", "regrouped" or "".
    pub only: String,
    /// By period.
    pub p: Vec<(String, J3Cell)>,
}

/// The comparison of two scenarios (J3).
#[derive(Clone, Debug)]
pub struct J3 {
    /// The junctions, the biggest delay change first.
    pub rows: Vec<J3Row>,
    /// A warning when the scenarios are for different years.
    pub note: String,
}

impl J3 {
    /// As JSON.
    pub fn to_value(&self) -> Value {
        let rows: Vec<Value> = self
            .rows
            .iter()
            .map(|r| {
                let mut p = Map::new();
                for (k, c) in &r.p {
                    p.insert(
                        k.clone(),
                        json!({
                            "a": c.a.as_ref().map_or(Value::Null, Summary::to_value),
                            "b": c.b.as_ref().map_or(Value::Null, Summary::to_value),
                            "d": c.d.as_ref().map_or(Value::Null, |d| json!({
                                "dos": plain_number(d.dos), "dosText": d.dos_text, "delay": plain_number(d.delay),
                                "delayText": d.delay_text, "los": d.los, "q95": plain_number(d.q95), "q95Text": d.q95_text,
                            })),
                        }),
                    );
                }
                json!({
                    "id": r.id, "fromId": r.from_id, "kind": if r.is_site { "site" } else { "node" }, "name": r.name,
                    "type": r.kind, "only": r.only, "p": Value::Object(p),
                })
            })
            .collect();
        json!({ "rows": rows, "note": self.note })
    }
}

/// `roleRank(P, s)`: the end of the comparison a scenario belongs to by its role.
fn role_rank(p: &Project, s: &Sc) -> i32 {
    match s.sc.role.as_str() {
        Some("existing") => 0,
        Some("baseline") => 1,
        Some("mitigation") => 3,
        Some("proposal") => 2,
        _ => {
            if p.base.as_str() == Some(s.sc.id.as_str()) {
                1
            } else {
                2
            }
        }
    }
}

/// `compareOrder(P, active, other, swap)`: which way a comparison runs, as (from, to) positions in the scenario list: from
/// the existing or without-development side to the proposal or mitigation side. Equal ranks measure the active scenario
/// against the other one. `swap` reverses it.
pub fn compare_order(p: &Project, active: &Sc, other: &Sc, swap: bool) -> (usize, usize) {
    let o = if role_rank(p, active) < role_rank(p, other) {
        (active.idx, other.idx)
    } else {
        (other.idx, active.idx)
    };
    if swap { (o.1, o.0) } else { o }
}

impl<'a> Tools<'a> {
    /// `summary(sc, res, e)`: one period's summary of an entry: worst DoS, delay, level of service, longest Q95 and its leg.
    pub fn summary(&self, sc: &crate::project::Scenario, res: &Res, e: &Entry) -> Option<Summary> {
        let nr = e.nr?;
        let legs = legs_of(sc, self.st(), res, e);
        let mut by_in: HashMap<&str, &str> = HashMap::new();
        for a in &legs {
            for k in &a.in_keys {
                by_in.insert(k.as_str(), a.arm.leg_label.as_str());
            }
        }
        let mut qa: Option<&crate::results::Approach> = None;
        for a in nr.appr {
            if a.ctrl != "free" && qa.is_none_or(|q| or0(a.q95) > or0(q.q95)) {
                qa = Some(a);
            }
        }
        let twsc = matches!(nr.intersection_los, Some(None));
        let xp = nr
            .xp
            .or_else(|| nr.appr.iter().find(|a| a.xp.is_some()).and_then(|a| a.xp));
        Some(Summary {
            dos: nr.dos,
            xp,
            delay: nr.delay,
            delay_basis: if !nr.delay_basis.is_empty() {
                nr.delay_basis.to_string()
            } else if twsc {
                "worst movement".to_string()
            } else {
                "average".to_string()
            },
            los: nr.los.to_string(),
            los_text: if !nr.los_text.is_empty() {
                nr.los_text.to_string()
            } else {
                nr.los.to_string()
            },
            twsc,
            q95: qa.map_or(0.0, |a| or0(a.q95)),
            q95_leg: match qa {
                Some(a) => match by_in.get(a.key.as_str()) {
                    Some(l) => l.to_string(),
                    None => format!("{}: {}", a.approach, a.name),
                },
                None => String::new(),
            },
            stale: res.stale,
        })
    }

    /// `j2(sc, periods)`: each junction across the periods, the biggest delay first.
    pub fn j2(&self, s: Sc<'a>, periods: &[String]) -> Vec<J2Row> {
        let mut by: Vec<J2Row> = Vec::new();
        for p in periods {
            let Some(res) = s.res_of(p) else {
                continue;
            };
            for e in entries(s.sc, res, false) {
                let i = match by.iter().position(|r| r.id == e.id) {
                    Some(i) => i,
                    None => {
                        by.push(J2Row {
                            id: e.id.clone(),
                            xid: xid(&e),
                            is_site: e.is_site,
                            name: e.name.clone(),
                            kind: e.kind.clone(),
                            node: e.node.clone(),
                            nodes: e.nodes.clone(),
                            p: Vec::new(),
                        });
                        by.len() - 1
                    }
                };
                let sm = self.summary(s.sc, res, &e);
                match by[i].p.iter().position(|(k, _)| k == p) {
                    Some(k) => by[i].p[k].1 = sm,
                    None => by[i].p.push((p.clone(), sm)),
                }
            }
        }
        let worst = |r: &J2Row| {
            math_max_of(
                &r.p.iter()
                    .map(|(_, s)| s.as_ref().map_or(0.0, |s| s.delay))
                    .collect::<Vec<_>>(),
            )
        };
        js_sort_by(&mut by, |a, b| worst(b) - worst(a));
        by
    }

    /// `j2Html(sc, periods, opts)`: the junction summary table; `fly` false leaves out the map links.
    pub fn j2_html(&self, s: Sc<'a>, periods: &[String], fly: bool) -> String {
        let rows = self.j2(s, periods);
        let ps: Vec<&String> = periods.iter().filter(|p| s.res_of(p).is_some()).collect();
        if rows.is_empty() {
            return "<p class=\"empty\">No controlled junctions with results. Set junction controls (give way, stop, roundabout or signals), then run the model.</p>".to_string();
        }
        let grp: String = ps
            .iter()
            .map(|p| {
                let r = s.res_of(p);
                format!(
                    "<th colspan=\"4\" style=\"text-align:center\">{} {}{}</th>",
                    esc(&per_name(p)),
                    esc(&r.map_or(String::new(), |r| year_text(r.year))),
                    if r.is_some_and(|r| r.stale) {
                        " <span class=\"pill stale\">out of date</span>"
                    } else {
                        ""
                    }
                )
            })
            .collect();
        let sub: String = ps
            .iter()
            .map(|_| "<th class=\"n\">Worst DoS</th><th class=\"n\">Delay (s)</th><th>LoS</th><th>Longest Q95 (m) and leg</th>")
            .collect();
        let body: String = rows
            .iter()
            .map(|r| {
                let cells: String = ps
                    .iter()
                    .map(|p| match r.get(p) {
                        None => "<td class=\"n\">\u{2013}</td><td class=\"n\">\u{2013}</td><td>\u{2013}</td><td>\u{2013}</td>".to_string(),
                        Some(sm) => format!(
                            "<td class=\"n\">{}</td><td class=\"n\">{}{}</td><td>{}{}</td><td>{} m{}</td>",
                            dos_cell(Some(sm.dos), sm.xp),
                            d1(Some(sm.delay)),
                            if sm.twsc { "\u{2020}" } else { "" },
                            los_badge(Some(&sm.los)),
                            if sm.twsc { "\u{2020}" } else { "" },
                            n0(Some(sm.q95)),
                            if sm.q95_leg.is_empty() {
                                String::new()
                            } else {
                                format!(" <span class=\"note\">{}</span>", esc(&sm.q95_leg))
                            }
                        ),
                    })
                    .collect();
                format!(
                    "<tr class=\"click\" {}><td>{}{}</td><td>{}</td>{}</tr>",
                    if fly {
                        format!("data-jx-fly=\"{}\"", esc(&fly_of(r.is_site, &r.id)))
                    } else {
                        String::new()
                    },
                    esc(&r.name),
                    if r.is_site { " <span class=\"pill\">site</span>" } else { "" },
                    esc(&ctl_name(&r.kind)),
                    cells
                )
            })
            .collect();
        format!(
            "{}<div class=\"tbl-wrap\"><table class=\"t compact\"><tr><th rowspan=\"2\">Junction</th><th rowspan=\"2\">Control</th>{}</tr><tr>{}</tr>{}</table></div>\n      <p class=\"note\">Delay is the volume-weighted average control delay (signals, roundabouts). \u{2020} Give way and stop: the worst movement, as there is no intersection level of service. DoS colours: green at or below practical DoS, amber above it, red over capacity. {}.</p>",
            self.supp_note(),
            grp,
            sub,
            body,
            LABEL
        )
    }

    /// `j3(from, to, periods)`: the same layout with changes from scenario `from` to `to`. Junctions are matched by id, then by
    /// member nodes, then by the most shared nodes (flagged 'regrouped'); one left without a partner is 'new' or 'gone'
    /// unless it still shares nodes with one in the other scenario.
    pub fn j3(&self, from: Sc<'a>, to: Sc<'a>, periods: &[String]) -> J3 {
        let a_rows = self.j2(from, periods);
        let b_rows = self.j2(to, periods);
        let mut used = vec![false; a_rows.len()];
        let share = |a: &J2Row, b: &J2Row| a.nodes.iter().filter(|n| b.nodes.contains(n)).count();
        let mut got: Vec<Option<(Option<usize>, &'static str)>> = vec![None; b_rows.len()];
        for (bi, b) in b_rows.iter().enumerate() {
            let a = a_rows
                .iter()
                .enumerate()
                .find(|(i, x)| !used[*i] && x.id == b.id)
                .map(|(i, _)| i)
                .or_else(|| {
                    a_rows
                        .iter()
                        .enumerate()
                        .find(|(i, x)| !used[*i] && x.xid == b.xid)
                        .map(|(i, _)| i)
                });
            if let Some(a) = a {
                used[a] = true;
                got[bi] = Some((Some(a), ""));
            }
        }
        let mut pairs: Vec<(Option<usize>, Option<usize>, &'static str)> = Vec::new();
        for (bi, b) in b_rows.iter().enumerate() {
            if got[bi].is_none() {
                let (mut a, mut best) = (None, 0usize);
                for (i, x) in a_rows.iter().enumerate() {
                    let k = if used[i] { 0 } else { share(x, b) };
                    if k > best {
                        best = k;
                        a = Some(i);
                    }
                }
                if let Some(i) = a {
                    used[i] = true;
                }
                let how = if a.is_some() || a_rows.iter().any(|x| share(x, b) > 0) {
                    "regrouped"
                } else {
                    ""
                };
                got[bi] = Some((a, how));
            }
            let (a, how) = got[bi].expect("matched");
            pairs.push((a, Some(bi), how));
        }
        for (i, a) in a_rows.iter().enumerate() {
            if !used[i] {
                let how = if b_rows.iter().any(|y| share(a, y) > 0) {
                    "regrouped"
                } else {
                    ""
                };
                pairs.push((Some(i), None, how));
            }
        }
        let mut rows: Vec<J3Row> = Vec::new();
        for (a, b, how) in pairs {
            let ar = a.map(|i| &a_rows[i]);
            let br = b.map(|i| &b_rows[i]);
            let base = br.or(ar).expect("a junction");
            let mut p: Vec<(String, J3Cell)> = Vec::new();
            for per in periods {
                let x = ar.and_then(|r| r.get(per));
                let y = br.and_then(|r| r.get(per));
                let d = match (x, y) {
                    (Some(x), Some(y)) => Some(J3Delta {
                        dos: y.dos - x.dos,
                        dos_text: sgn(Some(y.dos - x.dos), "", 2),
                        delay: y.delay - x.delay,
                        delay_text: sgn(Some(y.delay - x.delay), " s", 1),
                        los: if x.los == y.los {
                            "\u{2013}".to_string()
                        } else {
                            format!("{} to {}", x.los, y.los)
                        },
                        q95: y.q95 - x.q95,
                        q95_text: sgn(Some(y.q95 - x.q95), " m", 0),
                    }),
                    _ => None,
                };
                p.push((
                    per.clone(),
                    J3Cell {
                        a: x.cloned(),
                        b: y.cloned(),
                        d,
                    },
                ));
            }
            rows.push(J3Row {
                id: base.id.clone(),
                from_id: ar.map(|r| r.id.clone()),
                is_site: base.is_site,
                name: base.name.clone(),
                kind: base.kind.clone(),
                only: if !how.is_empty() {
                    how.to_string()
                } else if ar.is_none() {
                    "new".to_string()
                } else if br.is_none() {
                    "gone".to_string()
                } else {
                    String::new()
                },
                p,
            });
        }
        let first_year = |s: Sc| {
            periods
                .iter()
                .filter_map(|p| s.res_of(p).map(|r| r.year))
                .find(|y| *y != 0.0 && !y.is_nan())
        };
        let (ya, yb) = (first_year(from), first_year(to));
        js_sort_by(&mut rows, |p, q| {
            let worst = |r: &J3Row| {
                let mut m = 0.0f64;
                for (_, c) in &r.p {
                    m = math_max(m, c.d.as_ref().map_or(0.0, |d| d.delay));
                }
                m
            };
            worst(q) - worst(p)
        });
        J3 {
            rows,
            note: match (ya, yb) {
                (Some(a), Some(b)) if a != b => format!(
                    "The two scenarios are modelled for different years ({} and {}), so the changes mix background growth with the scheme.",
                    number_to_js_string(a),
                    number_to_js_string(b)
                ),
                _ => String::new(),
            },
        }
    }

    /// `j3Html(from, to, periods, opts)`: the comparison as a table with its notes.
    pub fn j3_html(
        &self,
        from: Option<Sc<'a>>,
        to: Option<Sc<'a>>,
        periods: Option<&[String]>,
    ) -> String {
        let (Some(from), Some(to)) = (from, to) else {
            return "<p class=\"note\">Add a second scenario and run both to compare junctions.</p>".to_string();
        };
        let periods: Vec<String> = match periods {
            Some(p) => p.to_vec(),
            None => periods_of_all(&[from, to]),
        };
        let c = self.j3(from, to, &periods);
        let ps: Vec<&String> = periods
            .iter()
            .filter(|p| from.res_of(p).is_some() && to.res_of(p).is_some())
            .collect();
        if ps.is_empty() {
            return format!(
                "<p class=\"note\">Run both {} and {} for the same peak to compare junctions.</p>",
                esc(from.name()),
                esc(to.name())
            );
        }
        let grp: String = ps
            .iter()
            .map(|p| {
                format!(
                    "<th colspan=\"4\" style=\"text-align:center\">{}</th>",
                    esc(&per_name(p))
                )
            })
            .collect();
        let sub: String = ps
            .iter()
            .map(|_| "<th class=\"n\">Worst DoS</th><th class=\"n\">Delay (s)</th><th>LoS</th><th class=\"n\">Longest Q95 (m)</th>")
            .collect();
        let pill = |only: &str| match only {
            "new" => " <span class=\"pill\">new</span>",
            "gone" => " <span class=\"pill\">removed</span>",
            "regrouped" => {
                " <span class=\"pill\" title=\"Grouped into a junction site differently in the two scenarios\">regrouped</span>"
            }
            _ => "",
        };
        let body: String = c
            .rows
            .iter()
            .map(|r| {
                let cells: String = ps
                    .iter()
                    .map(|p| {
                        let c = &r.p.iter().find(|(k, _)| k == *p).expect("period").1;
                        let dash = "\u{2013}";
                        format!(
                            "<td class=\"n\">{} \u{2192} {}{}</td><td class=\"n\">{} \u{2192} {}{}</td><td>{}</td><td class=\"n\">{}</td>",
                            c.a.as_ref().map_or(dash.to_string(), |s| x2(Some(s.dos))),
                            c.b.as_ref().map_or(dash.to_string(), |s| dos_cell(Some(s.dos), s.xp)),
                            c.d.as_ref().map_or(String::new(), |d| format!(" <span class=\"note\">{}</span>", d.dos_text)),
                            c.a.as_ref().map_or(dash.to_string(), |s| d1(Some(s.delay))),
                            c.b.as_ref().map_or(dash.to_string(), |s| d1(Some(s.delay))),
                            c.d.as_ref().map_or(String::new(), |d| format!(" <span class=\"note\">{}</span>", d.delay_text)),
                            c.d.as_ref().map_or(dash.to_string(), |d| esc(&d.los)),
                            c.d.as_ref().map_or(dash.to_string(), |d| d.q95_text.clone())
                        )
                    })
                    .collect();
                format!(
                    "<tr class=\"click\" data-jx-fly=\"{}\"><td>{}{}</td>{}</tr>",
                    esc(&fly_of(r.is_site, &r.id)),
                    esc(&r.name),
                    pill(&r.only),
                    cells
                )
            })
            .collect();
        format!(
            "{}<div class=\"tbl-wrap\"><table class=\"t compact\"><tr><th rowspan=\"2\">Junction</th>{}</tr><tr>{}</tr>{}</table></div>\n      {}<p class=\"note\">From {} to {}: each change is {} minus {}. LoS changes read 'B to D'; '\u{2013}' means unchanged. {}.</p>",
            self.supp_note(),
            grp,
            sub,
            body,
            if c.note.is_empty() {
                String::new()
            } else {
                format!("<p class=\"note warn\">{}</p>", esc(&c.note))
            },
            esc(from.name()),
            esc(to.name()),
            esc(to.name()),
            esc(from.name()),
            LABEL
        )
    }
}

/// `String(year)` for a printed year; an unset (0) year prints nothing, as `year || ''`.
pub(crate) fn year_text(y: f64) -> String {
    if y != 0.0 && !y.is_nan() {
        number_to_js_string(y)
    } else {
        String::new()
    }
}

fn or0(x: f64) -> f64 {
    if x.is_nan() || x == 0.0 { 0.0 } else { x }
}
