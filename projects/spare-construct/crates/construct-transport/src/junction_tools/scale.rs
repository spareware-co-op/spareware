//! The +10% flow-scale test (TTOC-06), the turn checks and the level of service tables.

use super::j1::los_method;
use super::text::{Head, TableRow, ctl_name, dos_cell, kit_table, los_badge, x2};
use super::{
    Entry, FLOW_SCALE, FLOW_SCALE_FACTOR, LABEL, Sc, Tools, entries, legs_of, moves_of,
    BALANCE_TOL, U_TURN_VPH,
};
use crate::graph::{Graph, apply_signal, build_graph, site_flows};
use crate::jsnum::{math_max, math_round, number_to_js_string, plain_number, to_fixed};
use crate::junction::{ArcFlow, JunctionResult, PerfSet, arc_time, junction_result, update_node_caps};
use crate::project::{Scenario, Settings};
use crate::results::{Approach, Res};
use crate::standards::{CapMethod, JunctionSet, STANDARDS, junction_set, pcu_f};
use crate::util::{esc, r1, r3};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// `losFrom(d, dos, method, type)`: level of service from delay (and `DoS`) with the method's thresholds.
#[must_use]
pub fn los_from(d: f64, dos: f64, method: &str, kind: &str) -> &'static str {
    let l = los_method(method);
    let th = match kind {
        "signals" => &l.signals,
        "roundabout" => &l.roundabout,
        _ => &l.sign,
    };
    if l.over_capacity_f && dos > 1.0 {
        return "F";
    }
    for (t, grade) in th.iter().zip(["A", "B", "C", "D", "E"]) {
        let hit = if method == "rta" { d < *t } else { d <= *t };
        if hit {
            return grade;
        }
    }
    "F"
}

/// A turn count that does not add up to its approach flow.
#[derive(Clone, Debug, PartialEq)]
pub struct BadTurns {
    /// Arc key.
    pub key: String,
    /// Leg label.
    pub leg: String,
    /// Sum of the turns (veh/h).
    pub turns: f64,
    /// The arc's flow (veh/h).
    pub arc: f64,
}

/// A U-turn flow over the flag.
#[derive(Clone, Debug, PartialEq)]
pub struct UTurn {
    /// Movement id.
    pub id: String,
    /// Flow (veh/h).
    pub v: f64,
    /// Road.
    pub road: String,
}

/// What `checks` finds in a junction's turns.
#[derive(Clone, Debug, PartialEq)]
pub struct Checks {
    /// Every approach's turns add up to its flow.
    pub balanced: bool,
    /// The approaches whose turns do not.
    pub bad: Vec<BadTurns>,
    /// U-turns over the flag.
    pub u_turns: Vec<UTurn>,
}

impl Checks {
    /// As JSON.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({
            "balanced": self.balanced,
            "bad": self.bad.iter().map(|b| json!({ "key": b.key, "leg": b.leg, "turns": plain_number(b.turns), "arc": plain_number(b.arc) })).collect::<Vec<_>>(),
            "uTurns": self.u_turns.iter().map(|u| json!({ "id": u.id, "v": plain_number(u.v), "road": u.road })).collect::<Vec<_>>(),
        })
    }
}

/// `checks(sc, res, e)`: whether the turns balance the approach flows, and the U-turns over the flag.
#[must_use]
pub fn checks(sc: &Scenario, st: &crate::project::Settings, res: &Res, e: &Entry) -> Checks {
    let mvs = moves_of(sc, st, res, e);
    let legs = legs_of(sc, st, res, e);
    let mut bad = Vec::new();
    // a mid-block signal (two roads) records no turns: nothing to balance
    if !mvs.is_empty() {
        for a in &legs {
            for k in &a.in_keys {
                if k.starts_with("c:") {
                    continue;
                }
                let Some(arc) = res.arcs.get(k) else {
                    continue;
                };
                let t = mvs
                    .iter()
                    .filter(|r| &r.approach == k)
                    .fold(0.0, |s, r| s + r.v);
                let v = arc.v;
                if (t - v).abs() > BALANCE_TOL {
                    bad.push(BadTurns {
                        key: k.clone(),
                        leg: a.arm.leg_label.clone(),
                        turns: r1(t),
                        arc: v,
                    });
                }
            }
        }
    }
    Checks {
        balanced: bad.is_empty(),
        bad,
        u_turns: mvs
            .iter()
            .filter(|r| r.mv == "U" && r.v > U_TURN_VPH)
            .map(|r| UTurn {
                id: r.id.clone(),
                v: r.v,
                road: r.road.clone(),
            })
            .collect(),
    }
}

/// A `DoS` at one approach (or movement) of a junction, before or after the flow rises.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleHit {
    /// Degree of saturation.
    pub dos: f64,
    /// Practical degree of saturation.
    pub xp: Option<f64>,
    /// Arc key.
    pub key: String,
    /// Movement ("" for the approach).
    pub mv: String,
    /// `DoS` before the flows rise (the crossing only).
    pub before: Option<f64>,
    /// Where: the leg and movement.
    pub where_: String,
}

impl ScaleHit {
    fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("dos".into(), plain_number(self.dos));
        m.insert("xp".into(), self.xp.map_or(Value::Null, plain_number));
        m.insert("key".into(), json!(self.key));
        m.insert("mv".into(), json!(self.mv));
        if let Some(b) = self.before {
            m.insert("before".into(), plain_number(b));
        }
        m.insert("where".into(), json!(self.where_));
        Value::Object(m)
    }
}

/// One junction's row of the flow-scale test.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowScaleRow {
    /// Site id or node id.
    pub id: String,
    /// A junction site.
    pub is_site: bool,
    /// Name.
    pub name: String,
    /// Control.
    pub kind: String,
    /// Worst `DoS` as run.
    pub dos: f64,
    /// Worst `DoS` at the raised flows.
    pub dos_f: f64,
    /// Practical `DoS`.
    pub xp: Option<f64>,
    /// The junction crosses its practical `DoS`.
    pub crosses: bool,
    /// Where it first crosses.
    pub first: Option<ScaleHit>,
    /// Where it is already over.
    pub already: Option<ScaleHit>,
    /// The result in words.
    pub text: String,
}

impl FlowScaleRow {
    /// As JSON.
    pub fn to_value(&self) -> Value {
        json!({
            "id": self.id, "kind": if self.is_site { "site" } else { "node" }, "name": self.name, "type": self.kind,
            "dos": plain_number(self.dos), "dosF": plain_number(self.dos_f),
            "xp": self.xp.map_or(Value::Null, plain_number), "crosses": self.crosses,
            "first": self.first.as_ref().map_or(Value::Null, ScaleHit::to_value),
            "already": self.already.as_ref().map_or(Value::Null, ScaleHit::to_value),
            "text": self.text,
        })
    }
}

/// The flow-scale test of a result.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowScale {
    /// The factor on the flows.
    pub factor: f64,
    /// Period.
    pub period: String,
    /// One row per junction.
    pub rows: Vec<FlowScaleRow>,
    /// The basis in words.
    pub basis: String,
}

impl FlowScale {
    /// The junctions that cross their practical `DoS`.
    #[must_use]
    pub fn crossing(&self) -> Vec<&FlowScaleRow> {
        self.rows.iter().filter(|r| r.crosses).collect()
    }
    /// As JSON.
    pub fn to_value(&self) -> Value {
        json!({
            "factor": plain_number(self.factor), "period": self.period,
            "rows": self.rows.iter().map(FlowScaleRow::to_value).collect::<Vec<_>>(),
            "crossing": self.crossing().iter().map(|r| r.to_value()).collect::<Vec<_>>(),
            "basis": self.basis,
        })
    }
}

/// One approach or movement row of a junction result, for the flow-scale comparison.
#[derive(Clone, Debug)]
struct El {
    key: String,
    mv: String,
    dos: f64,
    xp: Option<f64>,
}

/// `el(nr)`: the approaches (or movements) of a result, by key and movement, in order.
fn el_list(appr: &[Approach]) -> Vec<(String, El)> {
    let mut m: Vec<(String, El)> = Vec::new();
    let mut set = |k: String, v: El| {
        if let Some(slot) = m.iter_mut().find(|(x, _)| *x == k) {
            slot.1 = v;
        } else {
            m.push((k, v));
        }
    };
    for a in appr {
        if a.ctrl == "free" && a.mov.is_none() {
            continue;
        }
        if let Some(movs) = &a.mov {
            for mv in movs {
                set(
                    format!("{}|{}", a.key, mv.mv),
                    El { key: a.key.clone(), mv: mv.mv.clone(), dos: mv.dos, xp: a.xp },
                );
            }
        } else {
            set(
                a.key.clone(),
                El { key: a.key.clone(), mv: String::new(), dos: a.dos, xp: a.xp },
            );
        }
    }
    m
}

/// `el` for a re-evaluated junction result (an approach row of `junctionResult`).
fn el_list_after(appr: &[crate::junction::ApproachRow]) -> Vec<(String, El)> {
    let mut m: Vec<(String, El)> = Vec::new();
    let mut set = |k: String, v: El| {
        if let Some(slot) = m.iter_mut().find(|(x, _)| *x == k) {
            slot.1 = v;
        } else {
            m.push((k, v));
        }
    };
    for a in appr {
        if a.ctrl == "free" && a.mov.is_none() {
            continue;
        }
        if let Some(mr) = &a.mov {
            for mv in &mr.mov {
                set(
                    format!("{}|{}", a.key, mv.mv),
                    El { key: a.key.clone(), mv: mv.mv.to_string(), dos: mv.dos, xp: a.xp },
                );
            }
        } else {
            set(
                a.key.clone(),
                El { key: a.key.clone(), mv: String::new(), dos: a.dos, xp: a.xp },
            );
        }
    }
    m
}

/// `xT` of `flowScale`: the run's turn flows times `factor`, in the graph's turn slots (movement capacity only).
fn scaled_turns(graph: &Graph, res: &Res, factor: f64) -> Vec<f64> {
    let mut xs = vec![0.0f64; graph.n_t];
    let (head, out, off) = (graph.head(), graph.out(), graph.t_off());
    for list in res.turns.values() {
        for turn in list {
            let (Some(&from), Some(&to)) = (graph.key_idx.get(&turn.from), graph.key_idx.get(&turn.to)) else {
                continue;
            };
            // the arc, its end node's range of out-arcs and its turn offset all come from the graph the keys index
            let Some(node) = graph.arcs.get(from).map(|x| x.to) else {
                continue;
            };
            let (Some(&h0), Some(&h1), Some(&base)) =
                (head.get(node), node.checked_add(1).and_then(|n1| head.get(n1)), off.get(from))
            else {
                continue;
            };
            for k in h0..h1 {
                let Some(&o) = usize::try_from(k).ok().and_then(|ku| out.get(ku)) else {
                    continue;
                };
                if usize::try_from(o).ok() == Some(to) {
                    // t_off[a] + k - h0 (Bound: a turn slot index, within i32 as the graph stores it)
                    let ix = base
                        .checked_add(k)
                        .and_then(|s| s.checked_sub(h0))
                        .and_then(|i| usize::try_from(i).ok());
                    if let Some(slot) = ix.and_then(|i| xs.get_mut(i)) {
                        *slot = turn.pcu * factor;
                    }
                }
            }
        }
    }
    xs
}

/// Puts the run's cycle and green splits back on every signalised controller of the graph.
fn keep_signal_timings(graph: &mut Graph, res: &Res) {
    for ctl in &mut graph.ctl {
        if ctl.kind != "signals" {
            continue;
        }
        // a controller's node is in the graph it was built with
        let Some(id) = graph.ids.get(ctl.n) else {
            continue;
        };
        let Some(nr) = res.nodes.get(id) else {
            continue;
        };
        let (Some(c), Some(split)) = (nr.c, nr.split.as_ref()) else {
            continue;
        };
        if c == 0.0 || c.is_nan() {
            continue;
        }
        if let Some(t) = ctl.signal.as_mut() {
            t.c = c;
            t.split.clone_from(split);
        }
        apply_signal(ctl, &mut graph.arcs);
    }
}

/// Every controller of the graph re-evaluated at the flows `xt`, by node id.
fn rescaled_nodes(
    graph: &Graph,
    xt: &[f64],
    set: &Settings,
    jset: &JunctionSet,
    pf: f64,
) -> HashMap<String, JunctionResult> {
    let perf = PerfSet::new(set);
    let mut nodes: HashMap<String, JunctionResult> = HashMap::new();
    for ctl in &graph.ctl {
        // a controller's node and entry arcs are in the graph it was built with
        let Some(id) = graph.ids.get(ctl.n) else {
            continue;
        };
        let mut arcs_res = vec![ArcFlow::default(); graph.m];
        for &ai in &ctl.arcs {
            let (Some(a), Some(&x)) = (graph.arcs.get(ai), xt.get(ai)) else {
                continue;
            };
            let run = a.ff
                * (1.0
                    + set.bpr_alpha.num()
                        * crate::jsmath::pow(x / a.cap_mid, set.bpr_beta.num()));
            if let Some(slot) = arcs_res.get_mut(ai) {
                *slot = ArcFlow {
                    v: x / pf,
                    nd: arc_time(a, x, &perf) - run,
                };
            }
        }
        nodes.insert(
            id.clone(),
            junction_result(graph, ctl, xt, &arcs_res, set, jset, pf),
        );
    }
    nodes
}

/// One junction's row of the flow-scale test, from its approaches before and after the flows rise.
fn scale_row(
    entry: &Entry,
    before: &[(String, El)],
    after: &HashMap<String, El>,
    by_in: &HashMap<&str, &str>,
) -> FlowScaleRow {
    let name = |x: &El| {
        let base = by_in.get(x.key.as_str()).map_or(x.key.clone(), std::string::ToString::to_string);
        if x.mv.is_empty() {
            base
        } else {
            format!("{} ({})", base, x.mv)
        }
    };
    let (mut w0, mut w1) = (0.0f64, 0.0f64);
    let mut first: Option<ScaleHit> = None;
    let mut already: Option<ScaleHit> = None;
    for (k, b) in before {
        let a = after.get(k).unwrap_or(b);
        w0 = math_max(w0, b.dos);
        w1 = math_max(w1, a.dos);
        if let Some(bxp) = b.xp
            && b.dos > bxp
        {
            if already.as_ref().is_none_or(|x| b.dos > x.dos) {
                already = Some(ScaleHit {
                    dos: b.dos,
                    xp: b.xp,
                    key: b.key.clone(),
                    mv: b.mv.clone(),
                    before: None,
                    where_: name(b),
                });
            }
        } else if b.xp.is_some()
            && a.dos > a.xp.unwrap_or(0.0)
            && first.as_ref().is_none_or(|x| a.dos > x.dos)
        {
            first = Some(ScaleHit {
                dos: a.dos,
                xp: a.xp,
                key: a.key.clone(),
                mv: a.mv.clone(),
                before: Some(b.dos),
                where_: name(a),
            });
        }
    }
    let xp = entry.nr.and_then(|nr| nr.appr.iter().find(|a| a.xp.is_some()).and_then(|a| a.xp));
    let text = scale_text(first.as_ref(), already.as_ref());
    FlowScaleRow {
        id: entry.id.clone(),
        is_site: entry.is_site,
        name: entry.name.clone(),
        kind: entry.kind.clone(),
        dos: r3(w0),
        dos_f: r3(w1),
        xp,
        crosses: first.is_some(),
        first,
        already,
        text,
    }
}

/// The flow-scale result in words.
fn scale_text(first: Option<&ScaleHit>, already: Option<&ScaleHit>) -> String {
    match (first, already) {
        (Some(fi), al) => format!(
            "Crosses practical DoS at {}: {} \u{2192} {}{}",
            fi.where_,
            x2(fi.before),
            x2(Some(fi.dos)),
            match al {
                Some(al) => format!(
                    "; already above it at {} ({})",
                    al.where_,
                    x2(Some(al.dos))
                ),
                None => String::new(),
            }
        ),
        (None, Some(al)) => format!(
            "Already above practical DoS at {} ({})",
            al.where_,
            x2(Some(al.dos))
        ),
        (None, None) => "Stays at or below practical DoS".to_string(),
    }
}

impl<'a> Tools<'a> {
    /// `flowScale(sc, res, f)`: junctions only re-evaluated at the flows times f with the run's signal timings (no re-assignment).
    #[must_use]
    pub fn flow_scale(&self, sc: &Scenario, res: &'a Res, f: f64) -> Option<FlowScale> {
        let set = self.st();
        let mut graph = build_graph(sc, set, None);
        let pf = pcu_f(set);
        let jset = junction_set(set);
        let mut xt = vec![0.0f64; graph.m];
        for (slot, arc) in xt.iter_mut().zip(&graph.arcs) {
            if let Some(r) = res.arcs.get(&arc.key) {
                *slot = r.pcu * f;
            }
        }
        let x_t: Option<Vec<f64>> = if graph.cap_method == CapMethod::Movement {
            Some(scaled_turns(&graph, res, f))
        } else {
            None
        };
        // keep the run's signal timings
        keep_signal_timings(&mut graph, res);
        graph.mv_detail = true;
        // junction sites read the movements traced along the run's paths (res.siteTurns), scaled with the flows
        // (serialising the stored movements, plain data, to JSON cannot fail)
        if let Some(st) = &res.site_turns
            && let Ok(v) = serde_json::to_value(st)
            && let Some(sm) = site_flows(&graph, Some(&v), f)
        {
            graph.sm_x = Some(sm);
        }
        update_node_caps(&mut graph, &xt, 1.0, x_t.as_deref());
        let nodes = rescaled_nodes(&graph, &xt, set, &jset, pf);
        let mut rows: Vec<FlowScaleRow> = Vec::new();
        for entry in entries(sc, res, false) {
            let before = entry.nr.map_or(Vec::new(), |nr| el_list(nr.appr));
            let mut after: HashMap<String, El> = HashMap::new();
            let mut ids: Vec<&String> = entry.nodes.iter().collect();
            ids.push(&entry.node);
            for id in ids {
                if let Some(nr) = nodes.get(id) {
                    for (key, el) in el_list_after(&nr.appr) {
                        if before.iter().any(|(x, _)| *x == key) {
                            after.insert(key, el);
                        }
                    }
                }
            }
            if before.is_empty() {
                continue;
            }
            let legs = legs_of(sc, set, res, &entry);
            let mut by_in: HashMap<&str, &str> = HashMap::new();
            for leg in &legs {
                for key in &leg.in_keys {
                    by_in.insert(key.as_str(), leg.arm.leg_label.as_str());
                }
            }
            rows.push(scale_row(&entry, &before, &after, &by_in));
        }
        crate::graph::js_sort_by(&mut rows, |p, q| {
            let by_cross = f64::from(u8::from(q.crosses)) - f64::from(u8::from(p.crosses));
            if by_cross != 0.0 {
                return by_cross;
            }
            let by_already = f64::from(u8::from(q.already.is_some())) - f64::from(u8::from(p.already.is_some()));
            if by_already != 0.0 {
                return by_already;
            }
            q.dos_f - p.dos_f
        });
        Some(FlowScale {
            factor: f,
            period: res.period.clone(),
            rows,
            basis: format!(
                "{}: {}. Junctions re-evaluated at flows \u{d7} {} with the run's signal timings; routes are not re-assigned.",
                FLOW_SCALE.name,
                FLOW_SCALE.edition,
                to_fixed(f, 2)
            ),
        })
    }

    /// `scaleHtml(sc, res, opts)`: the flow-scale test as a note and a table. None for no result.
    #[must_use]
    pub fn scale_html(&self, sc: &Scenario, res: Option<&'a Res>) -> String {
        let Some(res) = res else {
            return "<p class=\"note\">Run the model to test junctions at higher flows.</p>".to_string();
        };
        let Some(r) = self.flow_scale(sc, res, FLOW_SCALE_FACTOR) else {
            return "<p class=\"note\">No controlled junctions to test.</p>".to_string();
        };
        if r.rows.is_empty() {
            return "<p class=\"note\">No controlled junctions to test.</p>".to_string();
        }
        let pct = number_to_js_string(math_round((r.factor - 1.0) * 100.0));
        let rows: Vec<TableRow> = r
            .rows
            .iter()
            .map(|x| TableRow {
                cells: vec![
                    esc(&x.name),
                    esc(&ctl_name(&x.kind)),
                    x2(Some(x.dos)),
                    dos_cell(Some(x.dos_f), x.xp),
                    x2(x.xp),
                    if x.crosses {
                        format!("<b style=\"color:var(--warn)\">{}</b>", esc(&x.text))
                    } else if x.already.is_some() {
                        format!("<span style=\"color:var(--danger)\">{}</span>", esc(&x.text))
                    } else {
                        esc(&x.text)
                    },
                ],
                fly: Some(super::summary::fly_of(x.is_site, &x.id)),
            })
            .collect();
        let crossing = r.crossing().len();
        let head = [
            Head::text("Junction"),
            Head::text("Control"),
            Head::num(&format!("Worst DoS ({})", esc(&res.period))),
            Head::num(&format!("At +{pct}%")),
            Head::num("Practical DoS"),
            Head::text("Result"),
        ];
        let table = kit_table(&head, &rows).replace("data-fly=", "data-jx-fly=");
        format!(
            "{}<p>{}</p>\n      {}\n      <p class=\"note\">{} {}.</p>",
            self.supp_note(),
            if crossing > 0 {
                format!(
                    "<b>{} junction{}</b> cross practical DoS when flows rise by {}%.",
                    crossing,
                    if crossing == 1 { "" } else { "s" },
                    pct
                )
            } else {
                format!("No junction crosses practical DoS when flows rise by {pct}%.")
            },
            table,
            esc(&r.basis),
            LABEL
        )
    }
}

/// `losTable(method)`: the level of service definitions as a table with their source.
#[must_use]
pub fn los_table(method: &str) -> String {
    let l = los_method(method);
    let op = if method == "rta" { "<" } else { "\u{2264}" };
    let n = |x: f64| number_to_js_string(x);
    let mut rows: Vec<TableRow> = Vec::new();
    for (i, lt) in ["A", "B", "C", "D", "E"].iter().enumerate() {
        let cell = |th: &[f64; 5]| {
            format!(
                "{}{} {}",
                match i.checked_sub(1).and_then(|p| th.get(p)) {
                    Some(prev) => format!("> {} and ", n(*prev)),
                    None => String::new(),
                },
                op,
                // i runs over the five grades A to E, so th[i] is always there
                th.get(i).map_or_else(String::new, |x| n(*x))
            )
        };
        rows.push(TableRow {
            cells: vec![los_badge(Some(lt)), cell(&l.signals), cell(&l.roundabout), cell(&l.sign)],
            fly: None,
        });
    }
    rows.push(TableRow {
        cells: vec![
            los_badge(Some("F")),
            format!("> {}, or DoS > 1", n(l.signals[4])),
            format!("> {}, or DoS > 1", n(l.roundabout[4])),
            format!("> {}, or DoS > 1", n(l.sign[4])),
        ],
        fly: None,
    });
    let xp = &STANDARDS.practical_dos;
    format!(
        "{}\n      <p class=\"note\">{}, {}{}. Practical degree of saturation: signals {}, roundabouts {}, give way and stop {} ({}). Give way and stop junctions report the worst movement, with no intersection LoS.</p>",
        kit_table(
            &[
                Head::text("LoS"),
                Head::text("Signals: control delay (s)"),
                Head::text("Roundabouts (s)"),
                Head::text("Give way and stop (s)")
            ],
            &rows
        ),
        esc(l.src.name),
        esc(l.src.edition),
        if l.src.verified { "" } else { " (not yet checked against the source)" },
        n(xp.signals),
        n(xp.roundabout),
        n(xp.sign),
        esc(xp.src.edition)
    )
}

/// Convenience: [`Sc`]'s flow-scale test for a period, as the report sections call it.
impl<'a> Tools<'a> {
    /// `resOf(sc, p) ? flowScale(sc, resOf(sc, p)) : null` at the default factor.
    #[must_use]
    pub fn flow_scale_of(&self, s: Sc<'a>, p: &str) -> Option<FlowScale> {
        s.res_of(p)
            .and_then(|r| self.flow_scale(s.sc, r, FLOW_SCALE_FACTOR))
    }
}
