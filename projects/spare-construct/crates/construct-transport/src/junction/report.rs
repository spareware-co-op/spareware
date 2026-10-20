//! Junction results (docs/spec/junction-los-corrections), movement rows and the result texts.

use super::lanes::LaneRow;
use super::movement::{EntryRes, d_hcm, q95v};
use super::perf::{PerfSet, eff_cap};
use crate::graph::{Arc, Ctl, Ctrl, Graph, approach_name};
use crate::jsnum::{
    math_max, math_max_of, math_min, math_round, number_to_js_string, or_default, plain_number,
    to_fixed, to_precision, value_truthy,
};
use crate::project::Settings;
use crate::standards::{JunctionSet, LosMethod, STANDARDS, los_lt, los_of};
use crate::util::{r1, r3};
use serde_json::{Map, Value, json};

/// `JUNCTION_LABEL`: how the junction results are labelled.
pub const JUNCTION_LABEL: &str = "HCM 6 / Akcelik-based, indicative; not SIDRA output";

const LETTERS: &str = "ABCDEF";
const DASH: &str = "\u{2013}";

/// `CAP_SRC[type]`: the name results give a controller type as the source of an arc's capacity (None for other types).
pub fn cap_src(kind: &str) -> Option<&'static str> {
    match kind {
        "signals" => Some("signals"),
        "priority" => Some("give way"),
        "stop" => Some("stop"),
        "roundabout" => Some("roundabout"),
        _ => None,
    }
}

/// An arc's assigned flow (veh/h, as reported) and its junction delay (s), as `res.arcs[key]` holds them for junctionResult.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ArcFlow {
    /// Flow (veh/h).
    pub v: f64,
    /// Junction delay (s).
    pub nd: f64,
}

fn letter_index(los: &str) -> i32 {
    if los.chars().count() == 1 {
        LETTERS.find(los).map_or(-1, |i| i as i32)
    } else {
        -1
    }
}

/// One movement row of an approach (movement method).
#[derive(Clone, Debug, PartialEq)]
pub struct MovRow {
    /// Movement: "L", "T" or "R".
    pub mv: &'static str,
    /// Demand (veh/h, hourly).
    pub v: f64,
    /// Capacity (veh/h).
    pub cap: f64,
    /// Degree of saturation at the peak flow rate.
    pub dos: f64,
    /// Delay (s).
    pub delay: f64,
    /// Level of service.
    pub los: &'static str,
    /// Back of queue (veh).
    pub q95veh: f64,
    /// Back of queue (m).
    pub q95: f64,
    /// Practical spare capacity (%), None without flow.
    pub psc: Option<f64>,
    /// Rank (1 for major road left and through).
    pub rank: u8,
    /// Conflicting flow (veh/h) and two-stage flag, where the movement has a capacity of its own.
    pub vc: Option<(f64, bool)>,
}

/// The movement part of an approach row (`movRows`): movements, lanes, lane source, and the major road and short-lane figures.
#[derive(Clone, Debug, PartialEq)]
pub struct MovRows {
    /// Movement rows.
    pub mov: Vec<MovRow>,
    /// Lane rows as reported: { d, len, slip, v, cap, dos, blocked? }.
    pub lanes: Vec<Value>,
    /// Lane model source (null when none).
    pub lane_src: Value,
    /// Major road: through capacity (veh/h), right-turn bay (m) and the share of time a right turner blocks.
    pub major: Option<(f64, f64, f64)>,
    /// Short-lane factor, when below 1.
    pub short_lane_factor: Option<f64>,
}

/// `movRows(A, mv, ...)`: an approach's movement rows: demand, capacity and DoS at the peak flow rate, delay, LoS and Q95, and its lane rows.
#[allow(clippy::too_many_arguments)]
pub fn mov_rows(
    a: &Arc,
    mv: &EntryRes,
    kind: &str,
    th: &[f64; 5],
    rta: bool,
    j: &JunctionSet,
    pf: f64,
    xp: f64,
) -> MovRows {
    let t = j.t;
    let k = j.pff / pf;
    let extra = if kind == "roundabout" {
        None
    } else if a.stop {
        Some(5.0)
    } else {
        Some(2.0)
    };
    let is_maj = a.major;
    let lf = |x: f64| if rta { los_lt(x, th) } else { los_of(x, th) };
    let lanes: &[LaneRow] = mv.lanes.as_deref().unwrap_or(&[]);
    let lane_d = |l: &LaneRow| {
        d_hcm(l.v, l.c, t)
            + match extra {
                None => 5.0 * math_min(1.0, l.x),
                Some(e) => e,
            }
    };
    let d_r = match (is_maj, mv.mov.r) {
        (true, Some(r)) => d_hcm(mv.v.r, r.cm, t) + 2.0,
        _ => 0.0,
    };
    let mj = mv.major;
    let mut rows = Vec::new();
    for m in ['L', 'T', 'R'] {
        let v = or_default(mv.v.get(m), 0.0);
        let mc = mv.mov.get(m);
        let cm = mc.map(|x| x.cm);
        if !(v > 0.05) && cm.is_none() {
            continue;
        }
        let ls: Vec<&LaneRow> = lanes.iter().filter(|l| l.use_.get(m) > 0.0).collect();
        let mut x = if ls.is_empty() {
            0.0
        } else {
            math_max_of(&ls.iter().map(|l| l.x).collect::<Vec<_>>())
        };
        let d;
        let mut q = if ls.is_empty() {
            0.0
        } else {
            math_max_of(&ls.iter().map(|l| q95v(l.v, l.c, t)).collect::<Vec<_>>())
        };
        if is_maj && m == 'R' {
            x = mj.map_or(f64::NAN, |j| j.xr);
            d = d_r;
            q = q95v(v, cm.unwrap_or(0.0), t);
        } else if is_maj {
            let b = ls
                .iter()
                .filter(|l| l.blocked == Some(true))
                .fold(0.0, |s, l| s + l.use_.get(m));
            let p_eff = mj.map_or(f64::NAN, |j| j.p_eff);
            d = if v > 0.0 { b / v * (1.0 - p_eff) * d_r } else { 0.0 };
            q = 0.0;
        } else {
            d = if v > 0.0 {
                ls.iter().fold(0.0, |s, l| s + l.use_.get(m) * lane_d(l)) / v
            } else {
                0.0
            };
        }
        let cap_pcu = if v > 0.05 && x > 0.0 {
            v / x
        } else {
            cm.unwrap_or(0.0)
        };
        rows.push(MovRow {
            mv: match m {
                'L' => "L",
                'T' => "T",
                _ => "R",
            },
            v: r1(v * k),
            cap: math_round(cap_pcu / pf),
            dos: r3(x),
            delay: r1(d),
            los: if x > 1.0 { "F" } else { lf(d) },
            q95veh: r1(q),
            q95: math_round(q * j.queue_space),
            psc: if x > 0.0 {
                Some(math_round((xp / x - 1.0) * 100.0))
            } else {
                None
            },
            rank: mc.map_or(1, |x| x.rank),
            vc: mc.map(|x| (math_round(x.vc / pf), x.two_stage.unwrap_or(false))),
        });
    }
    let lane_rows = lanes
        .iter()
        .map(|l| {
            let mut o = Map::new();
            o.insert("d".into(), l.d.clone().into());
            o.insert("len".into(), plain_number(or_default(l.len, 0.0)));
            o.insert("slip".into(), l.slip.unwrap_or(false).into());
            o.insert("v".into(), plain_number(r1(l.v * k)));
            o.insert("cap".into(), plain_number(math_round(l.c / pf)));
            o.insert("dos".into(), plain_number(r3(l.x)));
            if l.blocked == Some(true) {
                o.insert("blocked".into(), true.into());
            }
            Value::Object(o)
        })
        .collect();
    MovRows {
        mov: rows,
        lanes: lane_rows,
        lane_src: match &a.lm_src {
            Some(v) if value_truthy(v) => v.clone(),
            _ => Value::Null,
        },
        major: if is_maj {
            mj.map(|j| (math_round(j.cap_t / pf), or_default(j.bay, 0.0), r3(1.0 - j.p_eff)))
        } else {
            None
        },
        short_lane_factor: mv.r.filter(|&r| r < 1.0).map(r3),
    }
}

impl MovRows {
    fn extend(&self, m: &mut Map<String, Value>) {
        let rows: Vec<Value> = self
            .mov
            .iter()
            .map(|r| {
                let mut o = Map::new();
                o.insert("mv".into(), r.mv.into());
                o.insert("v".into(), plain_number(r.v));
                o.insert("cap".into(), plain_number(r.cap));
                o.insert("dos".into(), plain_number(r.dos));
                o.insert("delay".into(), plain_number(r.delay));
                o.insert("los".into(), r.los.into());
                o.insert("q95veh".into(), plain_number(r.q95veh));
                o.insert("q95".into(), plain_number(r.q95));
                o.insert("psc".into(), r.psc.map_or(Value::Null, plain_number));
                o.insert("rank".into(), r.rank.into());
                if let Some((vc, ts)) = r.vc {
                    o.insert("vc".into(), plain_number(vc));
                    o.insert("twoStage".into(), ts.into());
                }
                Value::Object(o)
            })
            .collect();
        m.insert("mov".into(), Value::Array(rows));
        m.insert("lanes".into(), Value::Array(self.lanes.clone()));
        m.insert("laneSrc".into(), self.lane_src.clone());
        m.insert("capMethod".into(), "movement".into());
        if let Some((cap_t, bay, rb)) = self.major {
            m.insert("capT".into(), plain_number(cap_t));
            m.insert("bay".into(), plain_number(bay));
            m.insert("rightBlock".into(), plain_number(rb));
        }
        if let Some(f) = self.short_lane_factor {
            m.insert("shortLaneFactor".into(), plain_number(f));
        }
    }
}

/// One approach row of a junction result.
#[derive(Clone, Debug, PartialEq)]
pub struct ApproachRow {
    /// Arc key.
    pub key: String,
    /// Link id.
    pub link: String,
    /// Road name.
    pub name: String,
    /// Arrival bearing (whole degrees).
    pub in_b: f64,
    /// Compass side ("West").
    pub approach: &'static str,
    /// Flow (veh/h).
    pub v: f64,
    /// Capacity (veh/h).
    pub cap: f64,
    /// Capacity (pcu/h).
    pub cap_pcu: f64,
    /// Degree of saturation (PCU ratio at the peak flow rate).
    pub dos: f64,
    /// Practical degree of saturation (None for free approaches).
    pub xp: Option<f64>,
    /// Practical spare capacity (%).
    pub psc: Option<f64>,
    /// "ok", "near" or "over" (None for free approaches).
    pub dos_band: Option<&'static str>,
    /// Delay (s).
    pub delay: f64,
    /// Level of service ("\u{2013}" for free approaches).
    pub los: &'static str,
    /// Level of service from delay alone.
    pub los_delay: &'static str,
    /// Back of queue (m).
    pub q95: f64,
    /// Back of queue (veh).
    pub q95veh: f64,
    /// Signal phase.
    pub phase: Option<u8>,
    /// A major road approach.
    pub major: bool,
    /// Control at the end of the arc.
    pub ctrl: &'static str,
    /// Movement rows (movement method).
    pub mov: Option<MovRows>,
}

impl ApproachRow {
    /// As JSON.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("key".into(), self.key.clone().into());
        m.insert("link".into(), self.link.clone().into());
        m.insert("name".into(), self.name.clone().into());
        m.insert("inB".into(), plain_number(self.in_b));
        m.insert("approach".into(), self.approach.into());
        m.insert("v".into(), plain_number(self.v));
        m.insert("cap".into(), plain_number(self.cap));
        m.insert("capPcu".into(), plain_number(self.cap_pcu));
        m.insert("dos".into(), plain_number(self.dos));
        m.insert("xp".into(), self.xp.map_or(Value::Null, plain_number));
        m.insert("psc".into(), self.psc.map_or(Value::Null, plain_number));
        m.insert("dosBand".into(), self.dos_band.map_or(Value::Null, Value::from));
        m.insert("delay".into(), plain_number(self.delay));
        m.insert("los".into(), self.los.into());
        m.insert("losDelay".into(), self.los_delay.into());
        m.insert("q95".into(), plain_number(self.q95));
        m.insert("q95veh".into(), plain_number(self.q95veh));
        if let Some(p) = self.phase {
            m.insert("phase".into(), p.into());
        }
        m.insert("major".into(), self.major.into());
        m.insert("ctrl".into(), self.ctrl.into());
        if let Some(mr) = &self.mov {
            mr.extend(&mut m);
        }
        Value::Object(m)
    }
}

/// The worst approach of a junction.
#[derive(Clone, Debug, PartialEq)]
pub struct WorstLeg {
    /// Arc key.
    pub key: String,
    /// Road name.
    pub name: String,
    /// Compass side.
    pub approach: &'static str,
    /// Delay (s).
    pub delay: f64,
    /// Degree of saturation.
    pub dos: f64,
    /// Level of service.
    pub los: &'static str,
}

/// One junction's approaches and summary (`junctionResult`).
#[derive(Clone, Debug, PartialEq)]
pub struct JunctionResult {
    /// Control type.
    pub kind: String,
    /// Approach rows.
    pub appr: Vec<ApproachRow>,
    /// Delay (s): the worst movement at give way and stop, else the volume-weighted mean.
    pub delay: f64,
    /// "worst movement" or "average".
    pub delay_basis: &'static str,
    /// Volume-weighted mean delay (s).
    pub avg_delay: f64,
    /// Worst approach delay (s).
    pub worst_delay: f64,
    /// The worst approach.
    pub worst_leg: Option<WorstLeg>,
    /// Highest degree of saturation.
    pub dos: f64,
    /// Total flow (veh/h).
    pub vol: f64,
    /// Level of service (give way and stop: the worst movement's).
    pub los: &'static str,
    /// Intersection level of service (None at give way and stop).
    pub intersection_los: Option<&'static str>,
    /// "worst movement" or "intersection delay".
    pub los_basis: &'static str,
    /// Level of service as shown ("F (side road, South approach)").
    pub los_text: String,
    /// Practical degree of saturation.
    pub xp: f64,
    /// Level-of-service method.
    pub method: &'static str,
    /// Capacity method.
    pub cap_method: &'static str,
    /// Label of the movement method (movement method, not signals).
    pub cap_label: Option<&'static str>,
    /// Signal cycle (s, whole).
    pub c: Option<f64>,
    /// Signal green split, to 3 decimals.
    pub split: Option<Vec<f64>>,
    /// Site id and its member node ids (a site's one controller).
    pub site: Option<(String, Vec<String>)>,
}

impl JunctionResult {
    /// As JSON, with the keys the prototype's result object has.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("type".into(), self.kind.clone().into());
        m.insert(
            "appr".into(),
            Value::Array(self.appr.iter().map(ApproachRow::to_value).collect()),
        );
        m.insert("delay".into(), plain_number(self.delay));
        m.insert("delayBasis".into(), self.delay_basis.into());
        m.insert("avgDelay".into(), plain_number(self.avg_delay));
        m.insert("worstDelay".into(), plain_number(self.worst_delay));
        m.insert(
            "worstLeg".into(),
            match &self.worst_leg {
                Some(w) => json!({ "key": w.key, "name": w.name, "approach": w.approach, "delay": plain_number(w.delay),
                    "dos": plain_number(w.dos), "los": w.los }),
                None => Value::Null,
            },
        );
        m.insert("dos".into(), plain_number(self.dos));
        m.insert("vol".into(), plain_number(self.vol));
        m.insert("los".into(), self.los.into());
        m.insert(
            "intersectionLos".into(),
            self.intersection_los.map_or(Value::Null, Value::from),
        );
        m.insert("losBasis".into(), self.los_basis.into());
        m.insert("losText".into(), self.los_text.clone().into());
        m.insert("xp".into(), plain_number(self.xp));
        m.insert("method".into(), self.method.into());
        m.insert("capMethod".into(), self.cap_method.into());
        m.insert(
            "capLabel".into(),
            self.cap_label.map_or(Value::Null, Value::from),
        );
        m.insert("C".into(), self.c.map_or(Value::Null, plain_number));
        m.insert(
            "split".into(),
            self.split.as_ref().map_or(Value::Null, |s| {
                Value::Array(s.iter().map(|&x| plain_number(x)).collect())
            }),
        );
        if let Some((site, nodes)) = &self.site {
            m.insert("site".into(), site.clone().into());
            m.insert("nodes".into(), json!(nodes));
        }
        Value::Object(m)
    }
}

/// `junctionResult(G, ctl, xt, arcsRes, S, J, pf)`: one junction's approaches and summary.
///
/// v and cap are veh/h (cap = PCU capacity / pf); dos is the PCU ratio at the peak flow rate.
/// An approach over capacity (DoS > 1) is F whatever its delay; signals and roundabouts report
/// the volume-weighted mean delay and its LoS; give way and stop report the worst movement.
/// Back of queue (95th percentile): signals per lane Q1 + Q2 (HCM 2000 / Akcelik) times f95;
/// unsignalised and roundabout entries by HCM 6 eq. 20-68. arcs_res holds each arc's flow and
/// junction delay by arc index.
#[allow(clippy::too_many_arguments)]
pub fn junction_result(
    g: &Graph,
    ctl: &Ctl,
    xt: &[f64],
    arcs_res: &[ArcFlow],
    s: &Settings,
    j: &JunctionSet,
    pf: f64,
) -> JunctionResult {
    let p = PerfSet::new(s);
    let los: &LosMethod = j.los();
    let xps = &STANDARDS.practical_dos;
    let kind = ctl.kind.as_str();
    let is_sig = kind == "signals";
    let is_rbt = kind == "roundabout";
    let twsc = !is_sig && !is_rbt;
    let th = if is_sig {
        &los.signals
    } else if is_rbt {
        &los.roundabout
    } else {
        &los.sign
    };
    let rta = j.method == "rta";
    let lf = |x: f64| if rta { los_lt(x, th) } else { los_of(x, th) };
    let xp = if is_sig {
        xps.signals
    } else if is_rbt {
        xps.roundabout
    } else {
        xps.sign
    };
    let movement = j.cap_method == crate::standards::CapMethod::Movement;
    let mut appr: Vec<ApproachRow> = Vec::new();
    let (mut sv, mut sd, mut worst_dos) = (0.0f64, 0.0f64, 0.0f64);
    let mut w: Option<usize> = None;
    for &ai in &ctl.arcs {
        let a = &g.arcs[ai];
        let r = arcs_res[ai];
        let nd = math_max(0.0, r.nd);
        let mv = if movement && !a.ring { a.mv_res.as_ref() } else { None };
        let mjr = matches!(mv, Some(m) if a.major && a.mj.is_some() && m.v.r > 0.0);
        let free = a.ctrl == Ctrl::Free && !mjr;
        let cap_p = if mjr {
            mv.map_or(f64::NAN, |m| m.cap)
        } else {
            eff_cap(a, &p)
        };
        let flow_p = xt[ai] / j.pff;
        let dos = flow_p / cap_p;
        let cap_v = cap_p / pf;
        let gl = math_max(1.0, a.gl as f64);
        let mut qv = 0.0;
        if let Some(m) = mv.filter(|_| !free && a.ctrl != Ctrl::Sig) {
            let lanes = m.lanes.as_deref().unwrap_or(&[]);
            qv = math_max(
                0.0,
                math_max_of(&lanes.iter().map(|l| q95v(l.v, l.c, j.t)).collect::<Vec<_>>()),
            );
        } else if a.ctrl == Ctrl::Sig {
            let c_l = cap_v / gl;
            let v_l = flow_p / pf / gl;
            let x = dos;
            let g_c = a.g_c.unwrap_or(f64::NAN);
            let c = a.c.unwrap_or(f64::NAN);
            let s_l = p.sat_flow / pf;
            let q1 = v_l * c / 3600.0 * (1.0 - g_c) / (1.0 - math_min(1.0, x) * g_c);
            let k_b = STANDARDS.queue.k_b * crate::jsmath::pow(s_l * g_c * c / 3600.0, 0.7);
            let c_t = math_max(1e-9, c_l * j.t);
            let q2 = 0.25 * c_l * j.t * ((x - 1.0) + ((x - 1.0) * (x - 1.0) + 8.0 * k_b * x / c_t).sqrt());
            qv = (q1 + q2) * j.f95;
        } else if !free {
            let c = math_max(1.0, cap_v);
            let x = dos;
            let t = j.t;
            qv = 900.0 * t * ((x - 1.0) + ((x - 1.0) * (x - 1.0) + (3600.0 / c) * x / (150.0 * t)).sqrt()) * c
                / 3600.0
                / gl;
        }
        let los_delay = if free { DASH } else { lf(nd) };
        let los_v = if free {
            DASH
        } else if los.over_capacity_f && dos > 1.0 {
            "F"
        } else {
            los_delay
        };
        let mov = match mv {
            Some(m) if !free => Some(mov_rows(a, m, kind, th, rta, j, pf, xp)),
            _ => None,
        };
        appr.push(ApproachRow {
            key: a.key.clone(),
            link: a.link.clone(),
            name: a.name.clone(),
            in_b: math_round(a.in_b),
            approach: approach_name(a.in_b, false),
            v: r.v,
            cap: math_round(cap_v),
            cap_pcu: math_round(cap_p),
            dos: r3(dos),
            xp: if free { None } else { Some(xp) },
            psc: if free || !(dos > 0.0) {
                None
            } else {
                Some(math_round((xp / dos - 1.0) * 100.0))
            },
            dos_band: if free {
                None
            } else if dos <= xp {
                Some("ok")
            } else if dos <= 1.0 {
                Some("near")
            } else {
                Some("over")
            },
            delay: r1(nd),
            los: los_v,
            los_delay,
            q95: math_round(math_max(0.0, qv) * j.queue_space),
            q95veh: r1(math_max(0.0, qv)),
            phase: a.phase,
            major: a.major,
            ctrl: a.ctrl.as_str(),
            mov,
        });
        if free {
            continue;
        }
        sv += r.v;
        sd += r.v * nd;
        if dos > worst_dos {
            worst_dos = dos;
        }
        let row = appr.len() - 1;
        let take = match w {
            None => true,
            Some(wi) => {
                let wr = &appr[wi];
                letter_index(los_v) > letter_index(wr.los) || (los_v == wr.los && nd > wr.delay)
            }
        };
        if take {
            w = Some(row);
        }
    }
    let avg = if sv > 0.0 { sd / sv } else { 0.0 };
    let wr = w.map(|i| &appr[i]);
    let worst_d = wr.map_or(0.0, |w| w.delay);
    let los_all = if twsc {
        wr.map_or("A", |w| w.los)
    } else {
        lf(avg)
    };
    let los_text = if twsc {
        format!(
            "{} ({}{})",
            los_all,
            if wr.is_some_and(|w| w.major) {
                "major road right turn"
            } else {
                "side road"
            },
            wr.map_or(String::new(), |w| format!(", {} approach", w.approach))
        )
    } else {
        los_all.to_string()
    };
    let worst_leg = wr.map(|w| WorstLeg {
        key: w.key.clone(),
        name: w.name.clone(),
        approach: w.approach,
        delay: w.delay,
        dos: w.dos,
        los: w.los,
    });
    let vol = math_round(appr.iter().fold(0.0, |s, a| s + a.v));
    let sig = ctl.signal.as_ref();
    JunctionResult {
        kind: ctl.kind.clone(),
        appr,
        delay: r1(if twsc { worst_d } else { avg }),
        delay_basis: if twsc { "worst movement" } else { "average" },
        avg_delay: r1(avg),
        worst_delay: r1(worst_d),
        worst_leg,
        dos: r3(worst_dos),
        vol,
        los: los_all,
        intersection_los: if twsc { None } else { Some(los_all) },
        los_basis: if twsc {
            "worst movement"
        } else {
            "intersection delay"
        },
        los_text,
        xp,
        method: j.method,
        cap_method: j.cap_method.as_str(),
        cap_label: if movement {
            if is_rbt {
                Some(STANDARDS.rbt_lanes.label)
            } else if is_sig {
                None
            } else {
                Some(STANDARDS.gaps.label)
            }
        } else {
            None
        },
        c: if is_sig {
            Some(math_round(sig.map_or(f64::NAN, |s| s.c)))
        } else {
            None
        },
        split: if is_sig {
            Some(sig.map_or(Vec::new(), |s| s.split.iter().map(|&x| r3(x)).collect()))
        } else {
            None
        },
        site: ctl
            .site
            .as_ref()
            .map(|st| (st.site.clone(), st.members.iter().map(|&n| g.ids[n].clone()).collect())),
    }
}

/// `delayText(d, dos)`: delay for display, never above 300 s; over-capacity approaches say so ("> 300 s · over capacity (DoS 1.42)"). NaN stands for a missing delay.
pub fn delay_text(d: f64, dos: f64) -> String {
    if !d.is_finite() {
        return DASH.to_string();
    }
    let t = if d > 300.0 {
        "> 300 s".to_string()
    } else {
        format!("{} s", number_to_js_string(math_round(d)))
    };
    if dos > 1.0 {
        format!("{} \u{b7} over capacity (DoS {})", t, to_fixed(dos, 2))
    } else {
        t
    }
}

/// The convergence record convText reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvInfo {
    /// Converged.
    pub converged: bool,
    /// Stopped at the iteration limit.
    pub hit_max: bool,
    /// Relative gap reached.
    pub gap: f64,
    /// Iterations run.
    pub it: f64,
    /// Gap target.
    pub gap_tol: f64,
    /// The stability rule is on.
    pub stable: bool,
    /// Share of links that must be stable.
    pub share: f64,
    /// Consecutive stable iterations.
    pub consec: f64,
}

/// `convText(o)`: "Converged: gap 0.01% (target 0.01%), flows stable (P > 98%) for 4 iterations, 37 iterations." and the not-converged forms.
pub fn conv_text(o: &ConvInfo) -> String {
    let pct = |g: f64| {
        let s = if g * 100.0 >= 0.005 || !(g > 0.0) {
            to_fixed(g * 100.0, 2)
        } else {
            to_precision(g * 100.0, 2)
        };
        format!("{s}%")
    };
    let js = number_to_js_string;
    if o.converged {
        let st = if o.stable {
            format!(
                ", flows stable (P > {}%) for {} iterations",
                js(math_round(o.share * 100.0)),
                js(o.consec)
            )
        } else {
            String::new()
        };
        return format!(
            "Converged: gap {} (target {}){}, {} iterations.",
            pct(o.gap),
            pct(o.gap_tol),
            st,
            js(o.it)
        );
    }
    if o.hit_max {
        let tail = if o.stable && o.gap <= o.gap_tol {
            "; flows not yet stable"
        } else {
            ""
        };
        return format!(
            "Not converged: reached {} iterations at gap {} (target {}){}.",
            js(o.it),
            pct(o.gap),
            pct(o.gap_tol),
            tail
        );
    }
    format!("Not converged: gap {} (target {}).", pct(o.gap), pct(o.gap_tol))
}
