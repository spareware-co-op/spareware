//! J1: the SIDRA-style Movement Summary of a junction (`j1`, `j1Html`), the storage the queues are measured against and the
//! results footer.

use super::text::{d1, dos_cell, los_badge, n0, p1, per_name, rq_cell, sgn, x2, ctl_name};
use super::{
    Entry, LABEL, Nr, STORAGE_AMBER, STORAGE_BASIS, STORAGE_RED, Tools, entries, legs_of, moves_of,
    periods_of,
};
use crate::edit::net::{len as net_len, links_at};
use crate::edit::sites;
use crate::jsnum::{math_round, number_to_js_string, plain_number, to_fixed};
use crate::project::{Link, Scenario};
use crate::results::{Approach, Res};
use crate::standards::{STANDARDS, LosMethod};
use crate::util::esc;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

/// `STANDARDS.los[method] || STANDARDS.los.hcm6`.
pub(crate) fn los_method(method: &str) -> &'static LosMethod {
    STANDARDS.los.get(method).unwrap_or(&STANDARDS.los.hcm6)
}

/// `footer(res)`: how the results were made (LoS method, capacity methods, peak flow factor, analysis period, queue space).
pub fn footer(res: &Res) -> String {
    let j = &res.junction;
    let l = los_method(&j.method);
    let mvm = j.cap_method == "movement";
    let pff = math_round(j.pff * 100.0);
    let t = j.t;
    let qs = j.queue_space;
    format!(
        "LoS method: {}; roundabout capacity: {}; give way and stop: {}; signals Webster / Akcelik; PFF {}%; analysis period {} h; queue space {} m. {}.",
        l.src.name,
        if mvm {
            "HCM 6 lane-based entry equations (SIDRA Standard is geometry-sensitive and will differ)"
        } else {
            "HCM 6 entry equation"
        },
        if mvm {
            "HCM 6 movement capacities (gap acceptance, rank impedance)"
        } else {
            "gap acceptance against all major-road flow"
        },
        number_to_js_string(pff),
        number_to_js_string(t),
        to_fixed(qs, 1),
        LABEL
    )
}

/// `upstreamLen(sc, res, key, side)`: the distance back to the upstream stop line from an approach arc: through uncontrolled
/// nodes with two roads (OSM splits roads mid-block) and a splitter island's one-way entry, stopping at a junction, a controlled
/// node, a dead end or a one-way road the wrong way. None when there is nothing upstream or the arc is a zone connector.
pub fn upstream_len(
    sc: &Scenario,
    res: &Res,
    key: &str,
    side: Option<&HashSet<String>>,
) -> Option<f64> {
    if key.is_empty() || key.starts_with("c:") || !res.arcs.contains_key(key) {
        return None;
    }
    let in_arc = |m: &Link, u: &str| {
        if m.b == u {
            format!("{}:ab", m.id)
        } else {
            format!("{}:ba", m.id)
        }
    };
    let out_arc = |m: &Link, u: &str| {
        if m.a == u {
            format!("{}:ab", m.id)
        } else {
            format!("{}:ba", m.id)
        }
    };
    let far = |m: &Link, u: &str| if m.a == u { m.b.clone() } else { m.a.clone() };
    let own = |n: &str| {
        side.is_some_and(|s| s.contains(n))
            || sc.nodes.get(n).is_some_and(|x| x.control.is("roundabout"))
    };
    let mut len = 0.0f64;
    let mut k = key.to_string();
    let mut seen: HashSet<String> = HashSet::new();
    while !k.is_empty() && !seen.contains(&k) && seen.len() < 500 {
        seen.insert(k.clone());
        let i = k.rfind(':').map_or(-1, |p| p as isize);
        let link_id = if i < 0 { &k[..] } else { &k[..i as usize] };
        let (Some(lk), Some(a)) = (sc.links.get(link_id), res.arcs.get(&k)) else {
            break;
        };
        len += if a.len != 0.0 && !a.len.is_nan() {
            a.len
        } else {
            net_len(sc, lk)
        };
        let u = if k.ends_with(":ab") { &lk.a } else { &lk.b };
        let nu = sc.nodes.get(u);
        let ctl = nu.map(|n| n.control.str_or("none"));
        let controlled = !matches!(ctl, Some("none") | Some("priority"));
        let Some(_) = nu else {
            break;
        };
        if controlled || sites::of(sc, u).is_some() {
            break;
        }
        let ctl = ctl.unwrap_or("none");
        let ls: Vec<&Link> = links_at(sc, u)
            .into_iter()
            .filter(|m| !m.status.is("closed") && m.a != m.b)
            .collect();
        let others: Vec<&Link> = ls.iter().copied().filter(|m| m.id != lk.id).collect();
        let mut next: Option<&Link> = None;
        if ctl == "none" && ls.len() == 2 && others.len() == 1 {
            next = Some(others[0]);
        } else if ls.len() == 3 && others.len() == 2 && !res.arcs.contains_key(&in_arc(lk, u)) {
            // a splitter island: lk is the one-way entry
            let x = others.iter().copied().find(|m| {
                res.arcs.contains_key(&in_arc(m, u))
                    && !res.arcs.contains_key(&out_arc(m, u))
                    && own(&far(m, u))
            });
            let m = x.and_then(|x| others.iter().copied().find(|m| m.id != x.id));
            if let Some(m) = m
                && res.arcs.contains_key(&in_arc(m, u))
                && res.arcs.contains_key(&out_arc(m, u))
            {
                next = Some(m);
            }
        }
        let Some(next) = next else {
            break;
        };
        k = in_arc(next, u);
    }
    if len > 0.0 { Some(len) } else { None }
}

/// The storage of a movement: the bay when every lane it uses is a short lane, otherwise the distance to the upstream junction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Storage {
    /// Storage (m), None for none known.
    pub storage: Option<f64>,
    /// The storage is a bay.
    pub bay: bool,
}

/// `storageOf(ap, mv, L)`: storage for a movement.
pub fn storage_of(ap: &Approach, mv: &str, l: Option<f64>) -> Storage {
    let ls: Vec<&crate::results::LaneRow> = ap
        .lanes
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|x| !x.d.is_empty() && x.d.contains(mv))
        .collect();
    let bay = if !ls.is_empty() && ls.iter().all(|x| x.len > 0.0) {
        ls.iter().fold(f64::NEG_INFINITY, |m, x| crate::jsnum::math_max(m, x.len))
    } else {
        0.0
    };
    let storage = if bay != 0.0 && !bay.is_nan() {
        Some(bay)
    } else {
        l.filter(|v| *v != 0.0 && !v.is_nan())
    };
    Storage {
        storage,
        bay: bay > 0.0,
    }
}

/// One row of the Movement Summary: a movement (or, without movement capacity, a turn) of an approach.
#[derive(Clone, Debug, Default)]
pub struct J1Row {
    /// Movement ids.
    pub ids: Vec<String>,
    /// Turn designations.
    pub turns: Vec<String>,
    /// Movement: L, T, R or U.
    pub mv: String,
    /// Demand flow (veh/h).
    pub v: f64,
    /// Heavy vehicles (%).
    pub hv_pct: f64,
    /// Capacity (veh/h).
    pub cap: Option<f64>,
    /// Degree of saturation.
    pub dos: Option<f64>,
    /// Practical degree of saturation.
    pub xp: Option<f64>,
    /// Average delay (s).
    pub delay: Option<f64>,
    /// Level of service.
    pub los: Option<String>,
    /// 95% back of queue (veh).
    pub q95veh: Option<f64>,
    /// 95% back of queue (m).
    pub q95: Option<f64>,
    /// Practical spare capacity (%).
    pub psc: Option<f64>,
    /// Storage (m).
    pub storage: Option<f64>,
    /// The storage is a right-turn or short-lane bay.
    pub bay: bool,
    /// Storage ratio (a queue over its storage).
    pub rq: Option<f64>,
    /// A movement-capacity row (otherwise a turn row without capacity).
    pub full: bool,
    /// Two-stage crossing.
    pub two_stage: bool,
    /// The road the turn goes to (turn rows only).
    pub to_road: Option<String>,
}

/// The approach's own row of the Movement Summary.
#[derive(Clone, Debug, Default)]
pub struct J1Sub {
    /// Demand flow (veh/h).
    pub v: f64,
    /// Heavy vehicles (%).
    pub hv_pct: f64,
    /// Capacity (veh/h).
    pub cap: f64,
    /// Degree of saturation.
    pub dos: f64,
    /// Practical degree of saturation.
    pub xp: Option<f64>,
    /// Average delay (s).
    pub delay: f64,
    /// Level of service ("\u{2013}" for a free approach).
    pub los: String,
    /// 95% back of queue (veh).
    pub q95veh: f64,
    /// 95% back of queue (m).
    pub q95: f64,
    /// Practical spare capacity (%).
    pub psc: Option<f64>,
    /// Distance back to the upstream stop line (m).
    pub storage: Option<f64>,
    /// Storage ratio.
    pub rq: Option<f64>,
    /// A free (major road) approach.
    pub free: bool,
    /// Arc key.
    pub key: String,
    /// Road name.
    pub name: String,
    /// Through capacity on a major road (veh/h).
    pub cap_t: Option<f64>,
    /// Right-turn bay (m).
    pub bay: Option<f64>,
    /// Share of time right turners block through traffic.
    pub right_block: Option<f64>,
    /// Where the lane model came from: None when the key is missing, Some(None) for null.
    pub lane_src: Option<Option<String>>,
}

/// An approach of the Movement Summary: its rows and its own row.
#[derive(Clone, Debug)]
pub struct J1Appr {
    /// Rows by movement.
    pub rows: Vec<J1Row>,
    /// The approach's own row.
    pub sub: J1Sub,
}

/// A leg of the summary (an approach, or several entries on one side), with its subtotal.
#[derive(Clone, Debug, Default)]
pub struct J1Group {
    /// Leg code (8 and up for an approach with no leg).
    pub o: f64,
    /// Leg code letters.
    pub leg: String,
    /// Leg label.
    pub label: String,
    /// Road.
    pub road: String,
    /// Approaches.
    pub appr: Vec<J1Appr>,
    /// The leg total, when the leg has more than one entry.
    pub leg_sub: Option<J1Sub>,
}

/// The intersection row of the Movement Summary.
#[derive(Clone, Debug, Default)]
pub struct J1All {
    /// Demand flow (veh/h).
    pub v: f64,
    /// Heavy vehicles (%).
    pub hv_pct: f64,
    /// Degree of saturation.
    pub dos: f64,
    /// Practical degree of saturation.
    pub xp: Option<f64>,
    /// Average delay (s).
    pub delay: f64,
    /// Level of service.
    pub los: String,
    /// 95% back of queue (veh).
    pub q95veh: f64,
    /// 95% back of queue (m).
    pub q95: f64,
}

/// The Movement Summary of a junction in a result (`j1`).
#[derive(Clone, Debug)]
pub struct J1<'a> {
    /// The junction.
    pub e: Entry<'a>,
    /// Its result.
    pub nr: Nr<'a>,
    /// The legs.
    pub groups: Vec<J1Group>,
    /// The intersection row, none at two-way sign control.
    pub all: Option<J1All>,
    /// Give way and stop control: no intersection level of service.
    pub twsc: bool,
    /// Level of service method.
    pub method: String,
    /// Control.
    pub kind: String,
    /// Period.
    pub period: String,
    /// Year.
    pub year: f64,
    /// The result is out of date.
    pub stale: bool,
    /// Rows are by movement (movement capacity).
    pub movement: bool,
    /// How the results were made.
    pub footer: String,
    /// The result's own footer (`res.meta.status.footer`).
    pub res_footer: String,
}

fn n(x: f64) -> Value {
    plain_number(x)
}

fn on(x: Option<f64>) -> Value {
    x.map_or(Value::Null, plain_number)
}

impl J1Row {
    /// As JSON, with the keys the prototype's row has.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("ids".into(), json!(self.ids));
        m.insert("turns".into(), json!(self.turns));
        m.insert("mv".into(), json!(self.mv));
        m.insert("v".into(), n(self.v));
        m.insert("hvPct".into(), n(self.hv_pct));
        if self.full {
            m.insert("cap".into(), on(self.cap));
            m.insert("dos".into(), on(self.dos));
            m.insert("xp".into(), on(self.xp));
            m.insert("delay".into(), on(self.delay));
            m.insert("los".into(), json!(self.los));
            m.insert("q95veh".into(), on(self.q95veh));
            m.insert("q95".into(), on(self.q95));
            m.insert("psc".into(), on(self.psc));
            m.insert("storage".into(), on(self.storage));
            m.insert("bay".into(), json!(self.bay));
            m.insert("rq".into(), on(self.rq));
            m.insert("full".into(), json!(true));
            m.insert("twoStage".into(), json!(self.two_stage));
        } else {
            m.insert("full".into(), json!(false));
            m.insert("toRoad".into(), json!(self.to_road));
        }
        Value::Object(m)
    }
}

impl J1Sub {
    /// As JSON, with the keys the prototype's approach row has (the leg total carries fewer).
    pub fn to_value(&self, leg_total: bool) -> Value {
        let mut m = Map::new();
        m.insert("v".into(), n(self.v));
        m.insert("hvPct".into(), n(self.hv_pct));
        m.insert("cap".into(), n(self.cap));
        m.insert("dos".into(), n(self.dos));
        m.insert("xp".into(), on(self.xp));
        m.insert("delay".into(), n(self.delay));
        m.insert("los".into(), json!(self.los));
        m.insert("q95veh".into(), n(self.q95veh));
        m.insert("q95".into(), n(self.q95));
        m.insert("psc".into(), on(self.psc));
        if !leg_total {
            m.insert("storage".into(), on(self.storage));
            m.insert("rq".into(), on(self.rq));
            m.insert("free".into(), json!(self.free));
            m.insert("key".into(), json!(self.key));
            m.insert("name".into(), json!(self.name));
            if let Some(v) = self.cap_t {
                m.insert("capT".into(), n(v));
            }
            if let Some(v) = self.bay {
                m.insert("bay".into(), n(v));
            }
            if let Some(v) = self.right_block {
                m.insert("rightBlock".into(), n(v));
            }
            match &self.lane_src {
                Some(Some(s)) => {
                    m.insert("laneSrc".into(), json!(s));
                }
                Some(None) => {
                    m.insert("laneSrc".into(), Value::Null);
                }
                None => {}
            }
        }
        Value::Object(m)
    }
}

impl J1<'_> {
    /// As JSON, without the result and the junction's own result: what the prototype's `j1` returns less its object references.
    pub fn to_value(&self) -> Value {
        let groups: Vec<Value> = self
            .groups
            .iter()
            .map(|g| {
                let mut m = Map::new();
                m.insert("o".into(), n(g.o));
                m.insert("leg".into(), json!(g.leg));
                m.insert("label".into(), json!(g.label));
                m.insert("road".into(), json!(g.road));
                m.insert(
                    "appr".into(),
                    Value::Array(
                        g.appr
                            .iter()
                            .map(|x| {
                                json!({
                                    "rows": x.rows.iter().map(J1Row::to_value).collect::<Vec<_>>(),
                                    "sub": x.sub.to_value(false),
                                })
                            })
                            .collect(),
                    ),
                );
                if let Some(ls) = &g.leg_sub {
                    m.insert("leg_sub".into(), ls.to_value(true));
                }
                Value::Object(m)
            })
            .collect();
        let all = match &self.all {
            Some(a) => json!({
                "v": n(a.v), "hvPct": n(a.hv_pct), "dos": n(a.dos), "xp": on(a.xp), "delay": n(a.delay),
                "los": a.los, "q95veh": n(a.q95veh), "q95": n(a.q95),
            }),
            None => Value::Null,
        };
        json!({
            "e": { "id": self.e.id, "kind": self.e.kind_name(), "name": self.e.name },
            "groups": groups,
            "all": all,
            "twsc": self.twsc,
            "method": self.method,
            "type": self.kind,
            "period": self.period,
            "year": n(self.year),
            "stale": self.stale,
            "movement": self.movement,
            "footer": self.footer,
        })
    }
}

/// `j1(sc, res, id, e0)`'s part of the sum of a list of numbers (`reduce((s, r) => s + r.v, 0)`).
fn sum<T>(xs: &[T], f: impl Fn(&T) -> f64) -> f64 {
    xs.iter().fold(0.0, |s, x| s + f(x))
}

/// `Math.max(0, ...xs)`.
fn max0(xs: impl IntoIterator<Item = f64>) -> f64 {
    xs.into_iter().fold(0.0, crate::jsnum::math_max)
}

impl<'a> Tools<'a> {
    /// `j1(sc, res, id, e0)`: the Movement Summary of a junction, looked up by id (or node id) unless the entry is given.
    pub fn j1(
        &self,
        sc: &Scenario,
        res: &'a Res,
        id: &str,
        e0: Option<&Entry<'a>>,
    ) -> Option<J1<'a>> {
        let found;
        let e: &Entry<'a> = match e0 {
            Some(e) => e,
            None => {
                let es = entries(sc, res, false);
                let i = es
                    .iter()
                    .position(|x| x.id == id)
                    .or_else(|| es.iter().position(|x| x.nodes.iter().any(|n| n == id)))?;
                found = es.into_iter().nth(i)?;
                &found
            }
        };
        let nr = e.nr?;
        let set = self.st();
        let method = match res.junction.method.as_str() {
            "" => match nr.method {
                Some(m) if !m.is_empty() => m.to_string(),
                _ => "hcm6".to_string(),
            },
            m => m.to_string(),
        };
        let legs = legs_of(sc, set, res, e);
        let mut by_in: HashMap<&str, usize> = HashMap::new();
        for (li, a) in legs.iter().enumerate() {
            for k in &a.in_keys {
                by_in.insert(k.as_str(), li);
            }
        }
        let mvs = moves_of(sc, set, res, e);
        let mut codes: Vec<f64> = Vec::new();
        let mut groups: Vec<J1Group> = Vec::new();
        let side: HashSet<String> = e.nodes.iter().cloned().collect();
        for ap in nr.appr {
            let a = by_in.get(ap.key.as_str()).map(|&i| &legs[i].arm);
            let code = match a {
                Some(a) => a.o as f64,
                None => 8.0 + groups.len() as f64,
            };
            let gi = match codes.iter().position(|c| *c == code) {
                Some(i) => i,
                None => {
                    codes.push(code);
                    groups.push(J1Group {
                        o: code,
                        leg: a.map_or(String::new(), |a| a.leg.to_string()),
                        label: match a {
                            Some(a) => a.leg_label.clone(),
                            None => format!(
                                "{}: {}",
                                if ap.approach.is_empty() { "Approach" } else { &ap.approach },
                                if ap.name.is_empty() { "Unnamed road" } else { &ap.name }
                            ),
                        },
                        road: match a {
                            Some(a) => a.road.clone(),
                            None => ap.name.clone(),
                        },
                        appr: Vec::new(),
                        leg_sub: None,
                    });
                    groups.len() - 1
                }
            };
            let rows: Vec<&crate::turns::MovementRow> =
                mvs.iter().filter(|r| r.approach == ap.key).collect();
            let v_t = sum(&rows, |r| r.v);
            let hv_t = sum(&rows, |r| r.hv);
            let hv_pct = if v_t > 0.0 {
                hv_t / v_t * 100.0
            } else {
                set.hv_pct.or(0.0)
            };
            let l = upstream_len(sc, res, &ap.key, Some(&side));
            let free = ap.ctrl == "free";
            let mut out: Vec<J1Row> = Vec::new();
            if let Some(movs) = &ap.mov {
                for m in movs {
                    let rs: Vec<&&crate::turns::MovementRow> =
                        rows.iter().filter(|r| r.mv == m.mv).collect();
                    let sv = sum(&rs, |r| r.v);
                    let sh = sum(&rs, |r| r.hv);
                    let so = storage_of(ap, &m.mv, l);
                    out.push(J1Row {
                        ids: rs.iter().map(|r| r.id.clone()).collect(),
                        turns: rs.iter().map(|r| r.turn.to_string()).collect(),
                        mv: m.mv.clone(),
                        v: m.v,
                        hv_pct: if sv > 0.0 { sh / sv * 100.0 } else { hv_pct },
                        cap: Some(m.cap),
                        dos: Some(m.dos),
                        xp: ap.xp,
                        delay: Some(m.delay),
                        los: Some(m.los.clone()),
                        q95veh: Some(m.q95veh),
                        q95: Some(m.q95),
                        psc: m.psc,
                        storage: so.storage,
                        bay: so.bay,
                        rq: so.storage.map(|s| m.q95 / s),
                        full: true,
                        two_stage: m.two_stage.unwrap_or(false),
                        to_road: None,
                    });
                }
            } else {
                for r in &rows {
                    out.push(J1Row {
                        ids: vec![r.id.clone()],
                        turns: vec![r.turn.to_string()],
                        mv: r.mv.clone(),
                        v: r.v,
                        hv_pct: if r.v > 0.0 { r.hv / r.v * 100.0 } else { hv_pct },
                        full: false,
                        to_road: Some(r.to_road.clone()),
                        ..J1Row::default()
                    });
                }
            }
            groups[gi].appr.push(J1Appr {
                rows: out,
                sub: J1Sub {
                    v: ap.v,
                    hv_pct,
                    cap: ap.cap,
                    dos: ap.dos,
                    xp: ap.xp,
                    delay: ap.delay,
                    los: if free { "\u{2013}".to_string() } else { ap.los.clone() },
                    q95veh: ap.q95veh,
                    q95: ap.q95,
                    psc: ap.psc,
                    storage: l,
                    rq: if l.is_some() && !free { l.map(|l| ap.q95 / l) } else { None },
                    free,
                    key: ap.key.clone(),
                    name: ap.name.clone(),
                    cap_t: ap.cap_t,
                    bay: ap.bay,
                    right_block: ap.right_block,
                    lane_src: match &ap.lane_src {
                        crate::jsnum::Maybe::Missing => None,
                        crate::jsnum::Maybe::Null => Some(None),
                        crate::jsnum::Maybe::Set(s) => Some(Some(s.clone())),
                    },
                },
            });
        }
        groups.sort_by(|p, q| p.o.partial_cmp(&q.o).unwrap_or(std::cmp::Ordering::Equal));
        for g in groups.iter_mut() {
            if g.appr.len() > 1 {
                let a: Vec<&J1Sub> = g.appr.iter().map(|x| &x.sub).collect();
                let ctl: Vec<&&J1Sub> = a.iter().filter(|x| !x.free).collect();
                let v = sum(&a, |x| x.v);
                let cv = sum(&ctl, |x| x.v);
                let dly = if cv > 0.0 {
                    sum(&ctl, |x| x.v * x.delay) / cv
                } else {
                    0.0
                };
                let dos = max0(ctl.iter().map(|x| x.dos));
                let psc = if ctl.is_empty() {
                    None
                } else {
                    Some(ctl.iter().fold(f64::INFINITY, |m, x| {
                        crate::jsnum::math_min(m, x.psc.unwrap_or(f64::INFINITY))
                    }))
                };
                g.leg_sub = Some(J1Sub {
                    v,
                    hv_pct: if v > 0.0 {
                        sum(&a, |x| x.v * x.hv_pct) / v
                    } else {
                        0.0
                    },
                    cap: sum(&a, |x| x.cap),
                    dos,
                    xp: a[0].xp,
                    delay: dly,
                    los: if !ctl.is_empty() {
                        super::scale::los_from(dly, dos, &method, nr.kind).to_string()
                    } else {
                        "\u{2013}".to_string()
                    },
                    q95veh: max0(a.iter().map(|x| x.q95veh)),
                    q95: max0(a.iter().map(|x| x.q95)),
                    psc,
                    ..J1Sub::default()
                });
            }
        }
        let twsc = match nr.intersection_los {
            Some(None) => true,
            None => nr.kind == "priority" || nr.kind == "stop",
            Some(Some(_)) => false,
        };
        let subs: Vec<&J1Sub> = groups
            .iter()
            .flat_map(|g| g.appr.iter().map(|x| &x.sub))
            .collect();
        let v_all = sum(&subs, |a| a.v);
        let all = if twsc {
            None
        } else {
            let il = match nr.intersection_los {
                Some(Some(s)) if !s.is_empty() => s,
                _ => nr.los,
            };
            Some(J1All {
                v: v_all,
                hv_pct: if v_all > 0.0 {
                    sum(&subs, |a| a.v * a.hv_pct) / v_all
                } else {
                    0.0
                },
                dos: nr.dos,
                xp: nr.xp.or_else(|| subs.first().and_then(|s| s.xp)),
                delay: nr.avg_delay,
                los: il.to_string(),
                q95veh: max0(subs.iter().map(|a| a.q95veh)),
                q95: max0(subs.iter().map(|a| a.q95)),
            })
        };
        let movement = !subs.is_empty() && nr.appr.iter().any(|a| a.mov.is_some());
        Some(J1 {
            e: e.clone(),
            nr,
            groups,
            all,
            twsc,
            method,
            kind: nr.kind.to_string(),
            period: res.period.clone(),
            year: res.year,
            stale: res.stale,
            movement,
            footer: footer(res),
            res_footer: res.meta.status.footer.clone(),
        })
    }
}

/// Options of [`j1_html`].
#[derive(Clone, Copy, Debug)]
pub struct J1HtmlOpts {
    /// Print the heading (the junction, control and period).
    pub head: bool,
}

impl Default for J1HtmlOpts {
    fn default() -> Self {
        J1HtmlOpts { head: true }
    }
}

const J1_HEAD: [&str; 12] = [
    "Mov ID",
    "Turn",
    "Demand flow (veh/h)",
    "HV (%)",
    "Capacity (veh/h)",
    "DoS (v/c)",
    "Average delay (s)",
    "LoS",
    "95% back of queue (veh)",
    "(m)",
    "Practical spare capacity (%)",
    "Storage ratio",
];

/// A row's cells in the order the table prints them: the fields the table reads from a summary row.
struct Cell<'r> {
    v: f64,
    hv_pct: f64,
    cap: Option<f64>,
    dos: Option<f64>,
    free: bool,
    xp: Option<f64>,
    delay: Option<f64>,
    los: Option<&'r str>,
    q95veh: Option<f64>,
    q95: Option<f64>,
    psc: Option<f64>,
    /// None for undefined, Some(None) for null.
    rq: Option<Option<f64>>,
    bay: bool,
}

fn cells(r: &Cell, lab: &str, turn: &str, bold: bool) -> String {
    let td = |x: &str, num: bool| format!("<td{}>{}</td>", if num { " class=\"n\"" } else { "" }, x);
    let dash = "\u{2013}";
    let mut s = format!(
        "<tr{}>",
        if bold {
            " style=\"font-weight:600;background:var(--panel-2)\""
        } else {
            ""
        }
    );
    s.push_str(&td(lab, false));
    s.push_str(&td(turn, false));
    s.push_str(&td(&n0(Some(r.v)), true));
    s.push_str(&td(&p1(Some(r.hv_pct)), true));
    s.push_str(&td(&r.cap.map_or(dash.to_string(), |c| n0(Some(c))), true));
    s.push_str(&td(
        &r.dos.map_or(dash.to_string(), |d| {
            dos_cell(Some(d), if r.free { None } else { r.xp })
        }),
        true,
    ));
    s.push_str(&td(&r.delay.map_or(dash.to_string(), |d| d1(Some(d))), true));
    s.push_str(&td(
        &r.los.map_or(dash.to_string(), |l| los_badge(Some(l))),
        false,
    ));
    s.push_str(&td(
        &match r.q95veh {
            Some(q) if !r.free => p1(Some(q)),
            _ => dash.to_string(),
        },
        true,
    ));
    s.push_str(&td(
        &match r.q95 {
            Some(q) if !r.free => n0(Some(q)),
            _ => dash.to_string(),
        },
        true,
    ));
    s.push_str(&td(
        &r.psc.map_or(dash.to_string(), |p| {
            let t = sgn(Some(p), "", 0);
            if t == dash { "0".to_string() } else { t }
        }),
        true,
    ));
    s.push_str(&td(
        &match r.rq {
            Some(q) => rq_cell(q, r.bay),
            None => dash.to_string(),
        },
        true,
    ));
    s.push_str("</tr>");
    s
}

fn cell_of_row(r: &J1Row) -> Cell<'_> {
    Cell {
        v: r.v,
        hv_pct: r.hv_pct,
        cap: r.cap,
        dos: r.dos,
        free: false,
        xp: r.xp,
        delay: r.delay,
        los: r.los.as_deref(),
        q95veh: r.q95veh,
        q95: r.q95,
        psc: r.psc,
        rq: if r.full { Some(r.rq) } else { None },
        bay: r.bay,
    }
}

fn cell_of_sub(s: &J1Sub, leg_total: bool) -> Cell<'_> {
    Cell {
        v: s.v,
        hv_pct: s.hv_pct,
        cap: Some(s.cap),
        dos: Some(s.dos),
        free: s.free,
        xp: s.xp,
        delay: Some(s.delay),
        los: Some(&s.los),
        q95veh: Some(s.q95veh),
        q95: Some(s.q95),
        psc: s.psc,
        rq: if leg_total { None } else { Some(s.rq) },
        bay: false,
    }
}

impl J1<'_> {
    /// The result's own footer as a note, or nothing.
    fn res_footer_html(&self) -> String {
        if self.res_footer.is_empty() {
            String::new()
        } else {
            format!("<p class=\"note\">{}</p>", esc(&self.res_footer))
        }
    }
}

/// `j1Html(D, opts)`: the Movement Summary as a table, with its notes and the footer. None gives the "no junction results" note.
pub fn j1_html(t: &Tools, d: Option<&J1>, opts: J1HtmlOpts) -> String {
    let Some(d) = d else {
        return "<p class=\"note\">No junction results here. Run the model, then pick a junction with a give-way, stop, roundabout or signal control.</p>".to_string();
    };
    let th: String = J1_HEAD
        .iter()
        .enumerate()
        .map(|(i, h)| format!("<th{}>{}</th>", if i >= 2 { " class=\"n\"" } else { "" }, h))
        .collect();
    let mut body: Vec<String> = Vec::new();
    for g in &d.groups {
        body.push(format!(
            "<tr><td colspan=\"12\" style=\"font-weight:700;padding-top:8px\">{}</td></tr>",
            esc(&g.label)
        ));
        for x in &g.appr {
            for r in &x.rows {
                let ids = r.ids.join(", ");
                let turns = r.turns.join(", ");
                body.push(cells(
                    &cell_of_row(r),
                    &esc(if ids.is_empty() { "\u{2013}" } else { &ids }),
                    &esc(if turns.is_empty() { &r.mv } else { &turns }),
                    false,
                ));
            }
            let s = &x.sub;
            let extra = if g.appr.len() > 1 {
                format!(" ({})", esc(if s.name.is_empty() { "entry" } else { &s.name }))
            } else {
                String::new()
            };
            body.push(cells(
                &cell_of_sub(s, false),
                &format!("Approach{extra}"),
                if s.free { "major road" } else { "" },
                true,
            ));
            if let Some(ct) = s.cap_t {
                let bay = match s.bay {
                    Some(b) if b != 0.0 && !b.is_nan() => {
                        format!(" with a {} m right-turn bay", n0(Some(b)))
                    }
                    _ => ", no right-turn bay".to_string(),
                };
                body.push(format!(
                    "<tr><td colspan=\"12\" class=\"note\">Major road: through capacity {} veh/h{}; right turners block through traffic {}% of the time.</td></tr>",
                    n0(Some(ct)),
                    bay,
                    p1(Some(s.right_block.unwrap_or(0.0) * 100.0))
                ));
            }
        }
        if let Some(ls) = &g.leg_sub {
            body.push(cells(&cell_of_sub(ls, true), "Leg total", "", true));
        }
    }
    match &d.all {
        Some(a) => {
            let c = Cell {
                v: a.v,
                hv_pct: a.hv_pct,
                cap: None,
                dos: Some(a.dos),
                free: false,
                xp: a.xp,
                delay: Some(a.delay),
                los: Some(&a.los),
                q95veh: Some(a.q95veh),
                q95: Some(a.q95),
                psc: None,
                rq: None,
                bay: false,
            };
            body.push(cells(&c, "Intersection (all vehicles)", "", true));
        }
        None => {
            let lt = if !d.nr.los_text.is_empty() {
                d.nr.los_text
            } else if !d.nr.los.is_empty() {
                d.nr.los
            } else {
                "\u{2013}"
            };
            body.push(format!(
                "<tr><td colspan=\"12\" class=\"note\">Two-way sign control: no intersection level of service. Worst movement: {}.</td></tr>",
                esc(lt)
            ));
        }
    }
    let head = if opts.head {
        format!(
            "<h4 style=\"margin:6px 0\">{} \u{b7} {} \u{b7} {} {}{}</h4>",
            esc(&d.e.name),
            esc(&ctl_name(&d.kind)),
            esc(&per_name(&d.period)),
            esc(&if d.year != 0.0 && !d.year.is_nan() { number_to_js_string(d.year) } else { String::new() }),
            if d.stale { " <span class=\"pill stale\">out of date</span>" } else { "" }
        )
    } else {
        String::new()
    };
    let cyc = match d.nr.c {
        Some(c) if c != 0.0 && !c.is_nan() => format!(
            "<p class=\"note\">Cycle time {} s; green split {}.</p>",
            n0(Some(c)),
            d.nr.split
                .unwrap_or(&[])
                .iter()
                .map(|s| format!("{}%", n0(Some(s * 100.0))))
                .collect::<Vec<_>>()
                .join(" / ")
        ),
        _ => String::new(),
    };
    // a very short storage to a red queue usually means the next node is part of this junction (an ungrouped divided road)
    let short = d.groups.iter().any(|g| {
        g.appr.iter().any(|x| {
            x.sub.storage.is_some_and(|s| s < 30.0) && x.sub.rq.is_some_and(|r| r > STORAGE_RED)
        })
    });
    let near = if short {
        "<p class=\"note\">An approach has less than 30 m to the next junction node. If those nodes are one junction (a divided road or an offset T), group them into a site with Merge junctions or Find junction sites, so the queue is measured to the real upstream junction.</p>"
    } else {
        ""
    };
    let lvl = if d.movement {
        ""
    } else {
        "<p class=\"note\">Approach-level results: capacity, DoS, delay and queue are per approach. Switch to movement capacity in Settings \u{203a} Junction analysis for per-movement rows.</p>"
    };
    format!(
        "{}{}<div class=\"tbl-wrap\"><table class=\"t compact\"><tr>{}</tr>{}</table></div>{}{}{}\n      <p class=\"note\">{} Storage ratio: amber above {}, red above {} ({}).</p>{}",
        head,
        t.supp_note(),
        th,
        body.join(""),
        cyc,
        lvl,
        near,
        esc(&d.footer),
        number_to_js_string(STORAGE_AMBER),
        number_to_js_string(STORAGE_RED),
        STORAGE_BASIS,
        d.res_footer_html()
    )
}

/// `periodsOf` for a results holder, re-exported for the sections.
pub(crate) fn periods(res: &crate::results::Results) -> Vec<String> {
    periods_of(res)
}
