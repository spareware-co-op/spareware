//! Movement capacities: gap acceptance by rank at priority junctions and HCM 6 lane equations at roundabouts.
//!
//! Priority junctions (HCM 6 two-way stop control, mirrored for left-hand traffic): rank 1
//! major T and L; rank 2 major R (opposed by the opposing T + L) and minor L; rank 3 minor T;
//! rank 4 minor R. Potential capacity `c_p` = `v_c` e^(-v_c `t_c` / 3600) / (1 - e^(-v_c `t_f` / 3600));
//! rank impedance `c_m` = `c_p` times the product of p0 (rank 4 through HCM's p'). A flush median
//! makes minor T and R two-stage. Flows are PCU at the peak flow rate.

use super::lanes::{Alloc, ApproachCap, Lane, LaneRow, Lmv, alloc_lanes, approach_cap, lane_rows_value};
use crate::graph::{Priority, js_sort_by};
use crate::jsmath;
use crate::jsnum::{
    math_max, math_max_of, math_min, or_default, plain_number, usize_to_f64, value_to_number, value_truthy,
};
use crate::standards::{Gap, STANDARDS};
use crate::util::ang_diff;
use serde_json::{Map, Value, json};

/// `cpot(vc, tc, tf)`: potential capacity (pcu/h) against conflicting flow vc; 3600 / tf when vc is 0.01 or less.
#[must_use]
pub fn cpot(vc: f64, tc: f64, tf: f64) -> f64 {
    if vc > 0.01 {
        vc * jsmath::exp(-vc * tc / 3600.0) / (1.0 - jsmath::exp(-vc * tf / 3600.0))
    } else {
        3600.0 / tf
    }
}

/// `p0of(v, c)`: probability of a queue-free state, max(0, 1 - v / c) (0 when c is not positive).
#[must_use]
pub fn p0of(v: f64, c: f64) -> f64 {
    if c > 0.0 { math_max(0.0, 1.0 - v / c) } else { 0.0 }
}

/// `pPrime(p)`: HCM's rank-4 impedance adjustment p' = 0.65 p - p / (p + 3) + 0.6 sqrt(p), within 0 to 1.
#[must_use]
pub fn p_prime(p: f64) -> f64 {
    math_min(
        1.0,
        math_max(0.0, 0.65 * p - p / (p + 3.0) + 0.6 * math_max(0.0, p).sqrt()),
    )
}

/// `twoStage(cI, cII, cm, vL, k)`: two-stage gap acceptance (HCM 6 / Brilon): stage capacities, one-stage capacity, major right turners vL using the median, storage k.
#[must_use]
pub fn two_stage(c_i: f64, c_ii: f64, cm: f64, v_l: f64, k: f64) -> f64 {
    let a = if k > 0.0 {
        1.0 - 0.32 * jsmath::exp(-1.3 * k.sqrt())
    } else {
        1.0
    };
    let den = c_ii - v_l - cm;
    let y = (c_i - cm) / den;
    if !y.is_finite() || den.abs() < 1e-6 {
        return math_max(0.0, a * (c_ii - v_l));
    }
    if (y - 1.0).abs() < 1e-6 {
        return math_max(0.0, a / (k + 1.0) * (k * (c_ii - v_l) + cm));
    }
    math_max(
        0.0,
        a / (jsmath::pow(y, k + 1.0) - 1.0) * (y * (jsmath::pow(y, k) - 1.0) * (c_ii - v_l) + (y - 1.0) * cm),
    )
}

/// `q95v(v, c, T)`: 95th-percentile queue (veh) of an unsignalised lane (HCM 6 eq. 20-68).
#[must_use]
pub fn q95v(v: f64, c: f64, t: f64) -> f64 {
    if v <= 0.0 || v.is_nan() || c <= 0.0 || c.is_nan() {
        return 0.0;
    }
    let x = v / c;
    math_max(
        0.0,
        900.0 * t * ((x - 1.0) + ((x - 1.0) * (x - 1.0) + (3600.0 / c) * x / (150.0 * t)).sqrt()) * c
            / 3600.0,
    )
}

/// `dHCM(v, c, T)`: control delay (s) of an unsignalised lane without the stop or give-way constant (HCM 6 eq. 20-64).
#[must_use]
pub fn d_hcm(v: f64, c: f64, t: f64) -> f64 {
    let c = math_max(1.0, c);
    let x = v / c;
    let hw = 3600.0 / c;
    hw + 900.0 * t * ((x - 1.0) + ((x - 1.0) * (x - 1.0) + hw * x / (450.0 * t)).sqrt())
}

/// Options of the capacity functions: analysis period T (h), queue space (m) and whether to build lane rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CapOpts {
    /// Analysis period (h).
    pub t: f64,
    /// Queue space per vehicle (m).
    pub qs: f64,
    /// Build the lane rows.
    pub detail: bool,
}

impl CapOpts {
    /// `{ T, qs, detail }` read as the functions read it: T 0.25 and qs the standard queue space when 0 or not a number.
    #[must_use]
    pub fn new(t: f64, qs: f64, detail: bool) -> CapOpts {
        CapOpts {
            t: or_default(t, 0.25),
            qs: or_default(qs, STANDARDS.queue.space),
            detail,
        }
    }
}

/// An entry's gap values: STANDARDS.gaps with the entry's overrides over them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntGaps {
    /// Major road right turn.
    pub major_r: Gap,
    /// Minor road left turn.
    pub minor_l: Gap,
    /// Minor road through.
    pub minor_t: Gap,
    /// Minor road right turn.
    pub minor_r: Gap,
}

impl EntGaps {
    /// As JSON { majorR: { tc, tf }, ... }.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let g = |x: &Gap| json!({ "tc": plain_number(x.tc), "tf": plain_number(x.tf) });
        json!({ "majorR": g(&self.major_r), "minorL": g(&self.minor_l), "minorT": g(&self.minor_t), "minorR": g(&self.minor_r) })
    }
}

/// One movement's capacity: conflicting flow, potential and movement capacity, queue-free probability, rank and two-stage flag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveCap {
    /// Conflicting flow (pcu/h).
    pub vc: f64,
    /// Potential capacity (pcu/h).
    pub cp: f64,
    /// Movement capacity (pcu/h).
    pub cm: f64,
    /// Probability of a queue-free state.
    pub p0: f64,
    /// Rank (2 to 4).
    pub rank: u8,
    /// Two-stage crossing (minor T and R only).
    pub two_stage: Option<bool>,
}

impl MoveCap {
    /// As JSON.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("vc".into(), plain_number(self.vc));
        m.insert("cp".into(), plain_number(self.cp));
        m.insert("cm".into(), plain_number(self.cm));
        m.insert("p0".into(), plain_number(self.p0));
        m.insert("rank".into(), self.rank.into());
        if let Some(t) = self.two_stage {
            m.insert("twoStage".into(), t.into());
        }
        Value::Object(m)
    }
}

/// Movement capacities by movement (`res.mov`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Movs {
    /// Left.
    pub l: Option<MoveCap>,
    /// Through.
    pub t: Option<MoveCap>,
    /// Right.
    pub r: Option<MoveCap>,
}

impl Movs {
    /// The movement's capacity, if computed.
    #[must_use]
    pub fn get(&self, m: char) -> Option<&MoveCap> {
        match m {
            'L' => self.l.as_ref(),
            'T' => self.t.as_ref(),
            _ => self.r.as_ref(),
        }
    }
    /// As JSON { L?, T?, R? }.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        for (k, v) in [("L", &self.l), ("T", &self.t), ("R", &self.r)] {
            if let Some(v) = v {
                m.insert(k.into(), v.to_value());
            }
        }
        Value::Object(m)
    }
}

/// The major road part of an entry's result: right turners and the through lanes they block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MajorRes {
    /// Degree of saturation of the right turners.
    pub xr: f64,
    /// Degree of saturation of the through and left lanes.
    pub xtl: f64,
    /// Through capacity (pcu/h).
    pub cap_t: f64,
    /// Right turners' queue-free probability.
    pub p0: f64,
    /// Chance a through lane is not blocked.
    pub p_eff: f64,
    /// Share of right turners a bay stores (1 with a long enough bay, 0 when sharing a lane).
    pub rb: f64,
    /// Right-turn bay length (m; 0 for none).
    pub bay: f64,
    /// Right turners' Q95 (veh).
    pub q95r: f64,
    /// Share of the approach turning right.
    pub s_r: f64,
    /// Share of the approach in lanes a right turner blocks.
    pub sh_tl: f64,
}

/// The roundabout part of an entry's result: circulating flow and lanes, and each lane's capacity.
#[derive(Clone, Debug, PartialEq)]
pub struct RbtRes {
    /// Circulating flow (pcu/h at the peak rate).
    pub vc: f64,
    /// Circulating lanes.
    pub nc: f64,
    /// Capacity of each entry lane (pcu/h).
    pub lane_cap: Vec<f64>,
}

/// One entry's movement result (`e.res`, kept on the arc as `mvRes`).
#[derive(Clone, Debug, PartialEq)]
pub struct EntryRes {
    /// Movement capacities.
    pub mov: Movs,
    /// Degree of saturation.
    pub x: f64,
    /// Capacity (pcu/h).
    pub cap: f64,
    /// Short-lane factor (minor and roundabout entries).
    pub r: Option<f64>,
    /// Largest Q95 of the short lanes (when short lanes count).
    pub q95short: Option<f64>,
    /// Lane rows (None when detail is off).
    pub lanes: Option<Vec<LaneRow>>,
    /// Movement flows (pcu/h at the peak rate).
    pub v: Lmv,
    /// Major road detail.
    pub major: Option<MajorRes>,
    /// Roundabout detail.
    pub rbt: Option<RbtRes>,
}

impl EntryRes {
    fn from_cap(mov: Movs, a: ApproachCap, v: Lmv) -> EntryRes {
        EntryRes {
            mov,
            x: a.x,
            cap: a.cap,
            r: Some(a.r),
            q95short: a.q95short,
            lanes: a.lanes,
            v,
            major: None,
            rbt: None,
        }
    }
    /// As JSON, with the keys the prototype's result object has.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("mov".into(), self.mov.to_value());
        m.insert("X".into(), plain_number(self.x));
        m.insert("cap".into(), plain_number(self.cap));
        if let Some(r) = self.r {
            m.insert("r".into(), plain_number(r));
        }
        if let Some(q) = self.q95short {
            m.insert("q95short".into(), plain_number(q));
        }
        m.insert("lanes".into(), lane_rows_value(self.lanes.as_deref()));
        m.insert("v".into(), self.v.to_value());
        if let Some(j) = &self.major {
            for (k, x) in [
                ("XR", j.xr),
                ("XTL", j.xtl),
                ("capT", j.cap_t),
                ("p0", j.p0),
                ("pEff", j.p_eff),
                ("rb", j.rb),
                ("bay", j.bay),
                ("q95R", j.q95r),
                ("sR", j.s_r),
                ("shTL", j.sh_tl),
            ] {
                m.insert(k.into(), plain_number(x));
            }
        }
        if let Some(b) = &self.rbt {
            m.insert("vc".into(), plain_number(b.vc));
            m.insert("nc".into(), plain_number(b.nc));
            m.insert(
                "laneCap".into(),
                Value::Array(b.lane_cap.iter().map(|&c| plain_number(c)).collect()),
            );
        }
        Value::Object(m)
    }
}

/// A major road arc's right-turn state, damped across updates (`a.mj`): right-turn share, blocked share, right-turn capacity, bay share.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MajorState {
    /// Share turning right.
    pub s_r: f64,
    /// Share in lanes a right turner blocks.
    pub sh_tl: f64,
    /// Right-turn capacity (pcu/h).
    pub cm_r: f64,
    /// Share of right turners the bay stores.
    pub rb: f64,
}

impl MajorState {
    /// As JSON { sR, shTL, cmR, rb }.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({ "sR": plain_number(self.s_r), "shTL": plain_number(self.sh_tl), "cmR": plain_number(self.cm_r), "rb": plain_number(self.rb) })
    }
}

/// One entry of a junction for the movement method (the prototype's `ent` item).
#[derive(Clone, Debug, PartialEq)]
pub struct MoveEntry {
    /// Arc index.
    pub i: usize,
    /// Standing at a give-way or stop junction: a major road approach, a stop-controlled minor approach, or neither.
    pub priority: Priority,
    /// Arrival bearing.
    pub in_b: f64,
    /// Leg ("g<k>" at a site, else the link id).
    pub leg: String,
    /// General traffic lanes (at least 1).
    pub gl: f64,
    /// Mid-block capacity per lane (pcu/h).
    pub s_lane: f64,
    /// Gap overrides as saved (site or node gaps), if any.
    pub gaps: Option<Value>,
    /// Lanes.
    pub lanes: Vec<Lane>,
    /// Flush median.
    pub median: bool,
    /// Slip lane.
    pub slip: bool,
    /// Lane model source.
    pub lm_src: Value,
    /// Full lanes with a through movement (at least 1).
    pub thru: f64,
    /// Turns out of the arc (node controllers): turn index, movement and the leg it goes to.
    pub mv_t: Option<Vec<(usize, &'static str, String)>>,
    /// Circulating lanes (roundabouts).
    pub nc: Option<f64>,
    /// Circulating flow (roundabouts; pcu/h at the peak rate).
    pub vc: Option<f64>,
    /// Movement flows (pcu/h at the peak rate).
    pub v: Lmv,
    /// Left-turn flows by the leg they turn into.
    pub l_to: Vec<(String, f64)>,
    /// The result of the last update.
    pub res: Option<EntryRes>,
    /// Gap values (set by `prio_geom`).
    pub g: Option<EntGaps>,
    /// Near-side major approach (minor entries).
    pub near: Option<usize>,
    /// Far-side major approach (minor entries).
    pub far: Option<usize>,
    /// Opposite minor approach (minor entries).
    pub opp_minor: Option<usize>,
    /// Opposing major approach (major entries).
    pub opp: Option<usize>,
}

impl MoveEntry {
    /// A major road approach.
    #[must_use]
    pub fn major(&self) -> bool {
        self.priority == Priority::Major
    }
    /// Stop control.
    #[must_use]
    pub fn stop(&self) -> bool {
        self.priority == Priority::Stop
    }
    /// Left-turn flow into a leg (`Lto[leg] || 0`).
    #[must_use]
    pub fn l_to(&self, leg: &str) -> f64 {
        self.l_to
            .iter()
            .find(|x| x.0 == leg)
            .map_or(0.0, |x| or_default(x.1, 0.0))
    }
}

fn gap_of(base: Gap, over: Option<&Value>, key: &str) -> Gap {
    // { ...GP[k], ...((e.gaps && e.gaps[k]) || {}) }: only an object override adds keys
    let mut g = base;
    if let Some(Value::Object(o)) = over.filter(|v| value_truthy(v)).and_then(|v| v.get(key)) {
        if let Some(x) = o.get("tc") {
            g.tc = value_to_number(x);
        }
        if let Some(x) = o.get("tf") {
            g.tf = value_to_number(x);
        }
    }
    g
}

/// `prioGeom(ent, GP)`: which major stream is near and far for each minor approach, the opposite minor, the opposing major, and each entry's gap values.
pub fn prio_geom(ent: &mut [MoveEntry]) -> (Vec<usize>, Vec<usize>) {
    let gp = &STANDARDS.gaps;
    let majors: Vec<usize> = ent
        .iter()
        .enumerate()
        .filter(|(_, en)| en.major())
        .map(|(i, _)| i)
        .collect();
    let minors: Vec<usize> = ent
        .iter()
        .enumerate()
        .filter(|(_, en)| !en.major())
        .map(|(i, _)| i)
        .collect();
    // Every index here is an entry's, so the NaN fallback never applies
    let bearing = |ent: &[MoveEntry], x: usize| ent.get(x).map_or(f64::NAN, |en| en.in_b);
    let far = |ent: &[MoveEntry], a: usize, list: &[usize]| -> Option<usize> {
        let ib = ent.get(a)?.in_b;
        let mut c: Vec<usize> = list
            .iter()
            .copied()
            .filter(|&x| x != a && ang_diff(bearing(ent, x), ib) > 120.0)
            .collect();
        js_sort_by(&mut c, |&p, &q| {
            ang_diff(bearing(ent, q), ib) - ang_diff(bearing(ent, p), ib)
        });
        c.first().copied()
    };
    let pick = |ent: &[MoveEntry], h: f64| -> Option<usize> {
        let mut it = majors.iter().copied();
        let mut b = it.next()?;
        for m in it {
            if ang_diff(bearing(ent, m), h) < ang_diff(bearing(ent, b), h) {
                b = m;
            }
        }
        Some(b)
    };
    for e in 0..ent.len() {
        let Some(entry) = ent.get_mut(e) else {
            continue;
        };
        let over = entry.gaps.as_ref();
        let gaps = EntGaps {
            major_r: gap_of(gp.major_r, over, "majorR"),
            minor_l: gap_of(gp.minor_l, over, "minorL"),
            minor_t: gap_of(gp.minor_t, over, "minorT"),
            minor_r: gap_of(gp.minor_r, over, "minorR"),
        };
        entry.g = Some(gaps);
        let ib = entry.in_b;
        if entry.major() {
            let opp = far(ent, e, &majors);
            if let Some(entry) = ent.get_mut(e) {
                entry.opp = opp;
            }
            continue;
        }
        let mut n = pick(ent, ib - 90.0);
        let mut f = pick(ent, ib + 90.0);
        if let (Some(nn), Some(ff)) = (n, f)
            && nn == ff
        {
            let nb = bearing(ent, nn);
            if ang_diff(nb, ib - 90.0) <= ang_diff(nb, ib + 90.0) {
                f = None;
            } else {
                n = None;
            }
        }
        let opp_minor = far(ent, e, &minors);
        if let Some(entry) = ent.get_mut(e) {
            entry.near = n;
            entry.far = f;
            entry.opp_minor = opp_minor;
        }
    }
    (majors, minors)
}

// Flow of movement mv on approach m (0 when there is none).
fn flow_of(ent: &[MoveEntry], m: Option<usize>, mv: char) -> f64 {
    m.and_then(|i| ent.get(i)).map_or(0.0, |e| e.v.get(mv))
}

// Left-turn flow of approach m into a leg (0 when there is none).
fn left_into(ent: &[MoveEntry], m: Option<usize>, leg: &str) -> f64 {
    m.and_then(|i| ent.get(i)).map_or(0.0, |e| e.l_to(leg))
}

// Queue-free probability of approach m's right turners (1 when there are none).
fn p0_right(mov: &[Movs], m: Option<usize>) -> f64 {
    m.and_then(|i| mov.get(i))
        .and_then(|mv| mv.r.as_ref())
        .map_or(1.0, |r| r.p0)
}

// Conflicting flow of a minor approach's first stage (the near-side major stream).
fn stage_one(ent: &[MoveEntry], e: &MoveEntry) -> f64 {
    let n = e.near;
    let ln = left_into(ent, n, &e.leg);
    2.0 * flow_of(ent, n, 'R') + flow_of(ent, n, 'T') + 0.5 * ln + math_max(0.0, flow_of(ent, n, 'L') - ln)
}

// Rank 2: major R, opposed by the opposing through and left flows.
// prio_geom sets every entry's gaps and its indexes are entries', so the lookups in these rank
// functions always succeed.
fn major_right_caps(ent: &[MoveEntry], majors: &[usize], mov: &mut [Movs]) {
    for &mi in majors {
        let Some(e) = ent.get(mi) else {
            continue;
        };
        let Some(gaps) = e.g else {
            continue;
        };
        let vc = e.opp.and_then(|p| ent.get(p)).map_or(0.0, |op| op.v.t + op.v.l);
        let gap = gaps.major_r;
        let cp = cpot(vc, gap.tc, gap.tf);
        if let Some(slot) = mov.get_mut(mi) {
            slot.r = Some(MoveCap {
                vc,
                cp,
                cm: cp,
                p0: p0of(e.v.r, cp),
                rank: 2,
                two_stage: None,
            });
        }
    }
}

// Rank 2: minor L.
fn minor_left_caps(ent: &[MoveEntry], minors: &[usize], mov: &mut [Movs]) {
    for &ai in minors {
        let Some(e) = ent.get(ai) else {
            continue;
        };
        let Some(gaps) = e.g else {
            continue;
        };
        let near = e.near;
        let thru = near
            .and_then(|ni| ent.get(ni))
            .map_or(1.0, |ne| or_default(ne.thru, 1.0));
        let vc = flow_of(ent, near, 'T') / math_max(1.0, thru) + 0.5 * left_into(ent, near, &e.leg);
        let gap = gaps.minor_l;
        let cp = cpot(vc, gap.tc, gap.tf);
        if let Some(slot) = mov.get_mut(ai) {
            slot.l = Some(MoveCap {
                vc,
                cp,
                cm: cp,
                p0: p0of(e.v.l, cp),
                rank: 2,
                two_stage: None,
            });
        }
    }
}

// Rank 3: minor T.
fn minor_through_caps(ent: &[MoveEntry], minors: &[usize], mov: &mut [Movs]) {
    let gp = &STANDARDS.gaps;
    let st = gp.stage;
    let k2 = gp.storage;
    for &ai in minors {
        let Some(e) = ent.get(ai) else {
            continue;
        };
        let Some(gaps) = e.g else {
            continue;
        };
        let far = e.far;
        let vc_i = stage_one(ent, e);
        let vc_ii = 2.0 * flow_of(ent, far, 'R') + flow_of(ent, far, 'T') + flow_of(ent, far, 'L');
        let gap = gaps.minor_t;
        let cp = cpot(vc_i + vc_ii, gap.tc, gap.tf);
        let f_i = p0_right(mov, e.near);
        let f_ii = p0_right(mov, far);
        let mut cm = cp * f_i * f_ii;
        if e.median {
            cm = two_stage(
                cpot(vc_i, gap.tc - st, gap.tf) * f_i,
                cpot(vc_ii, gap.tc - st, gap.tf) * f_ii,
                cm,
                flow_of(ent, far, 'R'),
                k2,
            );
        }
        if let Some(slot) = mov.get_mut(ai) {
            slot.t = Some(MoveCap {
                vc: vc_i + vc_ii,
                cp,
                cm,
                p0: p0of(e.v.t, cm),
                rank: 3,
                two_stage: Some(e.median),
            });
        }
    }
}

// Rank 4: minor R.
fn minor_right_caps(ent: &[MoveEntry], minors: &[usize], mov: &mut [Movs]) {
    let gp = &STANDARDS.gaps;
    let st = gp.stage;
    let k2 = gp.storage;
    for &ai in minors {
        let Some(e) = ent.get(ai) else {
            continue;
        };
        let Some(gaps) = e.g else {
            continue;
        };
        let far = e.far;
        let o2 = e.opp_minor;
        let vc_i = stage_one(ent, e);
        let vc_ii = 2.0 * flow_of(ent, far, 'R')
            + flow_of(ent, far, 'T')
            + 0.5 * flow_of(ent, far, 'L')
            + 0.5 * o2.and_then(|oi| ent.get(oi)).map_or(0.0, |oe| oe.v.l + oe.v.t);
        let gap = gaps.minor_r;
        let cp = cpot(vc_i + vc_ii, gap.tc, gap.tf);
        let f_i = p0_right(mov, e.near);
        let o2_mov: Option<Movs> = o2.and_then(|oi| mov.get(oi)).copied();
        let p2 = f_i * p0_right(mov, far) * o2_mov.and_then(|om| om.t).map_or(1.0, |mc| mc.p0);
        let fac = p_prime(p2) * o2_mov.and_then(|om| om.l).map_or(1.0, |mc| mc.p0);
        let mut cm = cp * fac;
        if e.median {
            cm = two_stage(
                cpot(vc_i, gap.tc - st, gap.tf) * f_i,
                cpot(vc_ii, gap.tc - st, gap.tf) * (if f_i > 0.0 { fac / f_i } else { fac }),
                cm,
                flow_of(ent, far, 'R'),
                k2,
            );
        }
        if let Some(slot) = mov.get_mut(ai) {
            slot.r = Some(MoveCap {
                vc: vc_i + vc_ii,
                cp,
                cm,
                p0: p0of(e.v.r, cm),
                rank: 4,
                two_stage: Some(e.median),
            });
        }
    }
}

/// `priorityCaps(ent, o)`: movement capacities at a give-way or stop junction; sets each entry's `res`.
pub fn priority_caps(ent: &mut [MoveEntry], o: &CapOpts) {
    let (majors, minors) = prio_geom(ent);
    let mut mov: Vec<Movs> = vec![Movs::default(); ent.len()];
    major_right_caps(ent, &majors, &mut mov);
    minor_left_caps(ent, &minors, &mut mov);
    minor_through_caps(ent, &minors, &mut mov);
    minor_right_caps(ent, &minors, &mut mov);
    for &ai in &minors {
        let (Some(e), Some(&ma)) = (ent.get_mut(ai), mov.get(ai)) else {
            continue;
        };
        let cap = move |_: &Lane, mv: char| ma.get(mv).map_or(f64::NAN, |mc| mc.cm);
        let ac = approach_cap(&e.lanes, &e.v, &cap, o.t, o.qs, o.detail);
        let v = e.v;
        e.res = Some(EntryRes::from_cap(ma, ac, v));
    }
    for &mi in &majors {
        // major_right_caps gives every major approach its right-turn capacity
        let res = ent
            .get(mi)
            .zip(mov.get(mi))
            .and_then(|(e, &movs)| major_result(e, movs, o));
        if let (Some(res), Some(e)) = (res, ent.get_mut(mi)) {
            e.res = Some(res);
        }
    }
}

// A major approach's through and left lanes as alloc_lanes takes them: right turns taken out, and
// the capacity of a lane a right turner blocks scaled by the chance it is not blocked.
fn major_through_lanes(e: &MoveEntry, has_bay: bool, p_eff: f64) -> Vec<Lane> {
    let tl: Vec<&Lane> = e.lanes.iter().filter(|l| l.d != "R").collect();
    let last_t = tl.len().checked_sub(1);
    let fallback = Lane::new("LT", 0.0);
    let src: Vec<&Lane> = if tl.is_empty() { vec![&fallback] } else { tl };
    src.iter()
        .enumerate()
        .map(|(i, l)| {
            let blocked = l.d.contains('R') || (has_bay && Some(i) == last_t);
            let d = l.d.replacen('R', "", 1);
            Lane {
                d: if d.is_empty() { "T".to_string() } else { d },
                len: 0.0,
                slip: false,
                c: e.s_lane * if blocked { math_max(0.02, p_eff) } else { 1.0 },
                blk: blocked,
            }
        })
        .collect()
}

// A major approach's lane rows: the through and left lanes, then the right-turn bays.
fn major_lane_rows(tl_l: &[Lane], al: &Alloc, ex: &[&Lane], v_r: f64, xr: f64, cm_r: f64) -> Vec<LaneRow> {
    // alloc_lanes gives one entry of x and use_ per lane, so the zip walks every lane
    let mut rows: Vec<LaneRow> = tl_l
        .iter()
        .zip(al.use_.iter().zip(&al.x))
        .map(|(l, (&lu, &lx))| LaneRow {
            d: l.d.clone(),
            len: 0.0,
            slip: None,
            v: lu.l + lu.t,
            x: lx,
            c: l.c,
            use_: lu,
            blocked: Some(l.blk),
        })
        .collect();
    for l in ex {
        let share = v_r / usize_to_f64(ex.len());
        rows.push(LaneRow {
            d: "R".to_string(),
            len: l.len,
            slip: None,
            v: share,
            x: xr,
            c: cm_r,
            use_: Lmv {
                l: 0.0,
                t: 0.0,
                r: share,
            },
            blocked: None,
        });
    }
    rows
}

// A major approach's result: right turners against the opposing flow, and the through and left
// lanes they block while waiting (None without a right-turn capacity, which never happens).
fn major_result(e: &MoveEntry, movs: Movs, o: &CapOpts) -> Option<EntryRes> {
    let rr = movs.r?;
    let v_r = e.v.r;
    let tot = e.v.l + e.v.t + v_r;
    let p0 = p0of(v_r, rr.cm);
    let ex: Vec<&Lane> = e.lanes.iter().filter(|l| l.d == "R").collect();
    let shared = ex.is_empty();
    let mut rb = if shared { 0.0 } else { 1.0 };
    let mut q95r = 0.0;
    let bay = if !ex.is_empty() && ex.iter().all(|l| l.len > 0.0) {
        math_max_of(&ex.iter().map(|l| l.len).collect::<Vec<_>>())
    } else {
        0.0
    };
    if crate::jsnum::truthy_number(bay) {
        q95r = q95v(v_r, rr.cm * usize_to_f64(ex.len()), o.t);
        rb = if q95r > 1e-9 {
            math_min(1.0, (bay / o.qs).floor() / q95r)
        } else {
            1.0
        };
    }
    let p_eff = p0 + (1.0 - p0) * rb;
    let has_bay = crate::jsnum::truthy_number(bay);
    let tl_l = major_through_lanes(e, has_bay, p_eff);
    let al = alloc_lanes(
        &tl_l,
        &Lmv {
            l: e.v.l,
            t: e.v.t,
            r: 0.0,
        },
        &|l: &Lane, _| l.c,
    );
    let xtl = math_max(0.0, math_max_of(&al.x));
    let xr = if v_r > 0.0 {
        v_r / (rr.cm * math_max(1.0, usize_to_f64(ex.len())))
    } else {
        0.0
    };
    let x = math_max(xtl, xr);
    let mut sh_tl = 0.0;
    // alloc_lanes gives one use_ entry per lane, so the zip walks every lane
    for (l, lu) in tl_l.iter().zip(&al.use_) {
        if l.blk {
            sh_tl += lu.l + lu.t;
        }
    }
    let cap_t = tl_l
        .iter()
        .fold(0.0, |s, l| s + if l.d.contains('T') { l.c } else { 0.0 });
    let lanes = o.detail.then(|| major_lane_rows(&tl_l, &al, &ex, v_r, xr, rr.cm));
    Some(EntryRes {
        mov: movs,
        x,
        cap: if tot > 0.0 && x > 0.0 { tot / x } else { cap_t + rr.cm },
        r: None,
        q95short: None,
        lanes,
        v: e.v,
        major: Some(MajorRes {
            xr,
            xtl,
            cap_t,
            p0,
            p_eff,
            rb,
            bay,
            q95r,
            s_r: if tot > 0.0 { v_r / tot } else { 0.0 },
            sh_tl: if tot > 0.0 { sh_tl / tot } else { 0.0 },
        }),
        rbt: None,
    })
}

/// `rbtCaps(ent, o)`: roundabout entries by HCM 6 lane equations against each entry's circulating flow `vc` and lanes `nc`; sets each entry's `res`.
///
/// A slip lane has no circulating conflict (its capacity is the mid-block lane capacity).
pub fn rbt_caps(ent: &mut [MoveEntry], o: &CapOpts) {
    let rl = &STANDARDS.rbt_lanes;
    for e in ent.iter_mut() {
        let ne = e.lanes.iter().filter(|l| !l.slip).count();
        let nc = e.nc.unwrap_or(f64::NAN);
        let vc = e.vc.unwrap_or(f64::NAN);
        let mut j = 0usize;
        let lanes: Vec<Lane> = e
            .lanes
            .iter()
            .map(|l| {
                let mut l = l.clone();
                if l.slip {
                    l.c = e.s_lane;
                    return l;
                }
                let q = if ne == 1 {
                    if nc >= 2.0 { rl.one_of2 } else { rl.one }
                } else if nc <= 1.0 {
                    rl.two_of1
                } else if j == ne.saturating_sub(1) {
                    rl.right2
                } else {
                    rl.left2
                };
                // Bound: j counts lanes, at most ne
                j = j.saturating_add(1);
                l.c = q.a * jsmath::exp(-q.b * vc);
                l
            })
            .collect();
        let a = approach_cap(&lanes, &e.v, &|l: &Lane, _| l.c, o.t, o.qs, o.detail);
        let mut res = EntryRes::from_cap(Movs::default(), a, e.v);
        res.rbt = Some(RbtRes {
            vc,
            nc,
            lane_cap: lanes.iter().map(|l| l.c).collect(),
        });
        e.res = Some(res);
    }
}
