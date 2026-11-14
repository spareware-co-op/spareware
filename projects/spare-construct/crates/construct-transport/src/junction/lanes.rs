//! Lanes of an approach: the lane model, sharing flows over lanes and the approach capacity with short lanes.

use super::movement::q95v;
use crate::jsnum::{
    math_max, math_max_of, math_min, or_default, plain_number, to_int32, usize_to_f64,
    value_to_js_string, value_to_number, value_truthy,
};
use serde_json::{Map, Value, json};

/// Flows (or anything else) by movement: left, through, right.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lmv {
    /// Left.
    pub l: f64,
    /// Through.
    pub t: f64,
    /// Right (U-turns count with right).
    pub r: f64,
}

impl Lmv {
    /// The value for movement 'L', 'T' or 'R'.
    #[must_use]
    pub fn get(&self, m: char) -> f64 {
        match m {
            'L' => self.l,
            'T' => self.t,
            _ => self.r,
        }
    }
    /// The value for changing.
    pub fn get_mut(&mut self, m: char) -> &mut f64 {
        match m {
            'L' => &mut self.l,
            'T' => &mut self.t,
            _ => &mut self.r,
        }
    }
    /// As JSON { L, T, R } (non-finite numbers as text, as the truth files hold them).
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({ "L": plain_number(self.l), "T": plain_number(self.t), "R": plain_number(self.r) })
    }
}

/// One lane: its movements ("LT"), a short-lane length (m, 0 for a full lane), a slip lane flag, and a capacity where the method sets one.
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    /// Movements, of L, T and R.
    pub d: String,
    /// Bay length (m); 0 for a full lane.
    pub len: f64,
    /// A left-turn slip lane.
    pub slip: bool,
    /// Capacity (pcu/h) where set (roundabout lanes, major road lanes); NaN otherwise.
    pub c: f64,
    /// A major road lane a right turner blocks.
    pub blk: bool,
}

impl Lane {
    /// A lane with movements d and bay length len.
    #[must_use]
    pub fn new(d: &str, len: f64) -> Lane {
        Lane {
            d: d.to_string(),
            len,
            slip: false,
            c: f64::NAN,
            blk: false,
        }
    }
    /// As the lane model writes it: { d, len } and slip: true on a slip lane.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("d".into(), self.d.clone().into());
        m.insert("len".into(), plain_number(self.len));
        if self.slip {
            m.insert("slip".into(), true.into());
        }
        Value::Object(m)
    }
}

/// An approach's lane model (`laneModel`): lanes, flush median, slip lane and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneModel {
    /// Lanes, left to right.
    pub lanes: Vec<Lane>,
    /// A flush median (two-stage crossings).
    pub median: bool,
    /// A left-turn slip lane.
    pub slip: bool,
    /// Source: "default", "user", or the stored model's src as saved.
    pub lm_src: Value,
}

impl LaneModel {
    /// As JSON { lanes, median, slip, lmSrc }.
    pub fn to_value(&self) -> Value {
        json!({ "lanes": self.lanes.iter().map(Lane::to_value).collect::<Vec<_>>(), "median": self.median,
            "slip": self.slip, "lmSrc": self.lm_src })
    }
}

fn field<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m.get(k),
        _ => None,
    }
}

/// `laneModel(raw, gl)`: the stored lane model (lanes of L T R U; "L+T" style accepted) or NZ default markings (one lane LTR; two or more LT, T..., TR).
#[must_use]
pub fn lane_model(raw: Option<&Value>, gl: f64) -> LaneModel {
    let raw = raw.filter(|r| value_truthy(r));
    // max(1) leaves a positive number, which usize holds
    let gl = usize::try_from(to_int32(gl).max(1)).unwrap_or(1);
    let mut lanes: Vec<Lane> = if gl == 1 {
        vec![Lane::new("LTR", 0.0)]
    } else {
        let mut v = vec![Lane::new("LT", 0.0)];
        for _ in 0..gl.saturating_sub(2) {
            v.push(Lane::new("T", 0.0));
        }
        v.push(Lane::new("TR", 0.0));
        v
    };
    let mut src = Value::String("default".into());
    if let Some(r) = raw
        && let Some(Value::Array(ls)) = field(r, "lanes")
        && !ls.is_empty()
    {
        let parsed: Vec<(String, f64)> = ls
            .iter()
            .map(|l| {
                // (l && typeof l === 'object' ? (l.d ?? l.mv) : l) || ''
                let x: Option<&Value> = match l {
                    Value::Object(m) => match m.get("d") {
                        Some(d) if !d.is_null() => Some(d),
                        _ => m.get("mv"),
                    },
                    Value::Array(_) | Value::Null => None,
                    other => Some(other),
                };
                let text = match x {
                    Some(v) if value_truthy(v) => value_to_js_string(v),
                    _ => String::new(),
                };
                let d: String = text
                    .to_uppercase()
                    .chars()
                    .map(|c| if c == 'U' { 'R' } else { c })
                    .filter(|c| matches!(c, 'L' | 'T' | 'R'))
                    .collect();
                let len = match l {
                    Value::Object(m) => m.get("len").map_or(f64::NAN, value_to_number),
                    _ => f64::NAN,
                };
                (d, math_max(0.0, or_default(len, 0.0)))
            })
            .filter(|(d, _)| !d.is_empty())
            .collect();
        if !parsed.is_empty() {
            lanes = parsed
                .into_iter()
                .map(|(d, len)| {
                    let mut u = String::new();
                    for c in d.chars() {
                        if !u.contains(c) {
                            u.push(c);
                        }
                    }
                    Lane::new(&u, len)
                })
                .collect();
            src = match field(r, "src") {
                Some(s) if value_truthy(s) => s.clone(),
                _ => Value::String("user".into()),
            };
        }
    }
    let slip = raw.is_some_and(|r| field(r, "slip").is_some_and(value_truthy));
    if slip {
        lanes = lanes
            .into_iter()
            .map(|mut l| {
                let d = l.d.replacen('L', "", 1);
                if !d.is_empty() {
                    l.d = d;
                }
                l
            })
            .filter(|l| !l.d.is_empty())
            .collect();
        let mut s = Lane::new("L", 0.0);
        s.slip = true;
        lanes.insert(0, s);
    }
    LaneModel {
        lanes,
        median: raw.is_some_and(|r| field(r, "median").is_some_and(value_truthy)),
        slip,
        lm_src: src,
    }
}

/// How flows were spread over lanes: each lane's degree of saturation and its flows by movement.
#[derive(Clone, Debug, PartialEq)]
pub struct Alloc {
    /// Degree of saturation of each lane (sum of v / c).
    pub x: Vec<f64>,
    /// Flows by movement in each lane.
    pub use_: Vec<Lmv>,
}

impl Alloc {
    /// As JSON { X, use }.
    pub fn to_value(&self) -> Value {
        json!({ "X": self.x.iter().map(|&x| plain_number(x)).collect::<Vec<_>>(), "use": self.use_.iter().map(Lmv::to_value).collect::<Vec<_>>() })
    }
}

/// `allocLanes(lanes, v, cap)`: spreads movement flows over lanes so lane degrees of saturation are as equal as the markings allow (movements with fewest lane choices first).
///
/// cap(lane, m) is the capacity of movement m in that lane. A movement the markings leave out
/// uses the nearest lane (left turns the first, right turns the last, through any).
pub fn alloc_lanes(lanes: &[Lane], flows: &Lmv, cap: &dyn Fn(&Lane, char) -> f64) -> Alloc {
    let n_lanes = lanes.len();
    if let [lane] = lanes {
        let mut sat = 0.0;
        if flows.l > 0.0 {
            sat += flows.l / math_max(1e-6, cap(lane, 'L'));
        }
        if flows.t > 0.0 {
            sat += flows.t / math_max(1e-6, cap(lane, 'T'));
        }
        if flows.r > 0.0 {
            sat += flows.r / math_max(1e-6, cap(lane, 'R'));
        }
        return Alloc {
            x: vec![sat],
            use_: vec![*flows],
        };
    }
    let mut sat = vec![0.0f64; n_lanes];
    let mut use_ = vec![Lmv::default(); n_lanes];
    let mut mvs: Vec<(char, Vec<usize>)> = ['L', 'T', 'R']
        .into_iter()
        .filter(|&mv| flows.get(mv) > 0.0)
        .map(|mv| {
            let mut ls: Vec<usize> = lanes
                .iter()
                .enumerate()
                .filter(|(_, lane)| lane.d.contains(mv))
                .map(|(li, _)| li)
                .collect();
            if ls.is_empty() {
                ls = match mv {
                    'L' => vec![0],
                    'R' => vec![n_lanes.saturating_sub(1)],
                    _ => (0..n_lanes).collect(),
                };
            }
            (mv, ls)
        })
        .collect();
    crate::graph::js_sort_by(&mut mvs, |a, b| usize_to_f64(a.1.len()) - usize_to_f64(b.1.len()));
    // Every index in ls is a lane's, and sat and use_ have one entry per lane, so the lookups below always succeed
    for (mv, ls) in &mvs {
        let mv = *mv;
        let vm = flows.get(mv);
        if let [li] = ls.as_slice() {
            let li = *li;
            let Some(lane) = lanes.get(li) else {
                continue;
            };
            if let Some(lu) = use_.get_mut(li) {
                *lu.get_mut(mv) += vm;
            }
            if let Some(lx) = sat.get_mut(li) {
                *lx += vm / math_max(1e-6, cap(lane, mv));
            }
            continue;
        }
        // exact water-filling: raise the lowest lanes together to the level h where the sum of (h - X_i) c_i is v
        let mut order: Vec<(usize, f64, f64)> = ls
            .iter()
            .filter_map(|&li| Some((li, *sat.get(li)?, math_max(1e-6, cap(lanes.get(li)?, mv)))))
            .collect();
        crate::graph::js_sort_by(&mut order, |pa, pb| pa.1 - pb.1);
        let (mut sc, mut sxc, mut level, mut top) = (0.0f64, 0.0f64, 0.0f64, 0usize);
        while let Some(&(_, xk, ck)) = order.get(top) {
            sc += ck;
            sxc += xk * ck;
            level = (vm + sxc) / sc;
            // Bound: top is below order.len(), so top + 1 cannot overflow
            let Some(next) = order.get(top.saturating_add(1)) else {
                break;
            };
            if level <= next.1 {
                break;
            }
            top = top.saturating_add(1);
        }
        for &(li, xj, cj) in order.iter().take(top.saturating_add(1)) {
            let add = (level - xj) * cj;
            if let Some(lu) = use_.get_mut(li) {
                *lu.get_mut(mv) += add;
            }
            if let Some(lx) = sat.get_mut(li) {
                *lx = level;
            }
        }
    }
    Alloc { x: sat, use_ }
}

/// A lane row of the movement detail: movements, bay length, slip, flow, degree of saturation, capacity, flows by movement, blocked.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneRow {
    /// Movements.
    pub d: String,
    /// Bay length (m).
    pub len: f64,
    /// A slip lane (approach rows; None on major road rows).
    pub slip: Option<bool>,
    /// Flow (pcu/h).
    pub v: f64,
    /// Degree of saturation.
    pub x: f64,
    /// Capacity (pcu/h).
    pub c: f64,
    /// Flows by movement.
    pub use_: Lmv,
    /// A through lane a right turner blocks (major road rows; None elsewhere).
    pub blocked: Option<bool>,
}

impl LaneRow {
    /// As JSON { d, len, slip?, v, X, c, use, blocked? }.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("d".into(), self.d.clone().into());
        m.insert("len".into(), plain_number(self.len));
        if let Some(s) = self.slip {
            m.insert("slip".into(), s.into());
        }
        m.insert("v".into(), plain_number(self.v));
        m.insert("X".into(), plain_number(self.x));
        m.insert("c".into(), plain_number(self.c));
        m.insert("use".into(), self.use_.to_value());
        if let Some(b) = self.blocked {
            m.insert("blocked".into(), b.into());
        }
        Value::Object(m)
    }
}

/// An approach's capacity (`approachCap`): degree of saturation, capacity, short-lane factor, short lanes' Q95 and the lane rows.
#[derive(Clone, Debug, PartialEq)]
pub struct ApproachCap {
    /// Degree of saturation.
    pub x: f64,
    /// Capacity (pcu/h).
    pub cap: f64,
    /// Short-lane factor: 1 separate operation, 0 shared.
    pub r: f64,
    /// Largest Q95 of the short lanes (only when short lanes count).
    pub q95short: Option<f64>,
    /// Lane rows (None when detail is off).
    pub lanes: Option<Vec<LaneRow>>,
}

// The lanes with each short lane's movements merged into the full lane beside it (the first full lane
// for a short lane before them all, the last for one after)
fn merge_short(lanes: &[Lane], short_i: &[usize], full: &[usize]) -> Vec<Lane> {
    let mut merged: Vec<Lane> = full.iter().filter_map(|&li| lanes.get(li).cloned()).collect();
    for &si in short_i {
        // full is not empty here (some lanes are not short), and every index is a lane's
        let (Some(short), Some(&first), Some(&last)) = (lanes.get(si), full.first(), full.last())
        else {
            continue;
        };
        let jj: usize = if si < first {
            0
        } else if si > last {
            merged.len().saturating_sub(1)
        } else {
            full.iter().position(|&f| f > si).unwrap_or(0)
        };
        let Some(target) = merged.get_mut(jj) else {
            continue;
        };
        let joined = format!("{}{}", target.d, short.d);
        let mut u = String::new();
        for c in joined.chars() {
            if !u.contains(c) {
                u.push(c);
            }
        }
        target.d = u;
    }
    merged
}

/// `approachCap(lanes, v, cap, T, qs, detail)`: capacity (pcu/h) of a set of lanes; short lanes interpolate between separate and shared operation by min(1, `N_s` / Q95).
///
/// `N_s` is floor(bay length / queue space qs). detail false leaves out the lane rows (the
/// assignment's iterations do not need them).
pub fn approach_cap(
    lanes: &[Lane],
    flows: &Lmv,
    cap: &dyn Fn(&Lane, char) -> f64,
    period: f64,
    qs: f64,
    detail: bool,
) -> ApproachCap {
    let tot = flows.l + flows.t + flows.r;
    let sep = alloc_lanes(lanes, flows, cap);
    let mut xs = 0.0;
    for &lx in &sep.x {
        if lx > xs {
            xs = lx;
        }
    }
    let best = |lane: &Lane| {
        let mut best_c = 0.0;
        for mv in lane.d.chars() {
            best_c = math_max(best_c, cap(lane, mv));
        }
        best_c
    };
    let c_sep = if tot > 0.0 && xs > 0.0 {
        tot / xs
    } else {
        lanes.iter().fold(0.0, |acc, lane| acc + best(lane))
    };
    let lanes_out = |al: &Alloc, scale: f64| -> Option<Vec<LaneRow>> {
        if !detail {
            return None;
        }
        // alloc_lanes gives one entry of x and use_ per lane, so the zip walks every lane
        Some(
            lanes
                .iter()
                .zip(al.use_.iter().zip(&al.x))
                .map(|(lane, (&lu, &lx))| {
                    let lv = lu.l + lu.t + lu.r;
                    LaneRow {
                        d: lane.d.clone(),
                        len: or_default(lane.len, 0.0),
                        slip: Some(lane.slip),
                        v: lv,
                        x: lx * scale,
                        c: if lv > 0.0 && lx > 0.0 { lv / lx } else { best(lane) },
                        use_: lu,
                        blocked: None,
                    }
                })
                .collect(),
        )
    };
    let n_short = lanes.iter().filter(|lane| lane.len > 0.0).count();
    if n_short == 0 || n_short == lanes.len() || tot <= 0.0 {
        return ApproachCap {
            x: if tot > 0.0 { xs } else { 0.0 },
            cap: c_sep,
            r: 1.0,
            q95short: None,
            lanes: lanes_out(&sep, 1.0),
        };
    }
    let short_i: Vec<usize> = lanes
        .iter()
        .enumerate()
        .filter(|(_, lane)| lane.len > 0.0)
        .map(|(li, _)| li)
        .collect();
    // !(len > 0): NaN lengths count as full lanes
    let full: Vec<usize> = lanes
        .iter()
        .enumerate()
        .filter(|(_, lane)| lane.len <= 0.0 || lane.len.is_nan())
        .map(|(li, _)| li)
        .collect();
    let merged = merge_short(lanes, &short_i, &full);
    let sh = alloc_lanes(&merged, flows, cap);
    let xh = math_max(0.0, math_max_of(&sh.x));
    let c_sh = if xh > 0.0 { tot / xh } else { c_sep };
    let (mut r, mut q95s) = (1.0f64, 0.0f64);
    for &si in &short_i {
        // short_i holds lane indexes, and sep has one entry per lane
        let (Some(&su), Some(&sx), Some(short)) = (sep.use_.get(si), sep.x.get(si), lanes.get(si))
        else {
            continue;
        };
        let lv = su.l + su.t + su.r;
        let sc = if lv > 0.0 && sx > 0.0 { lv / sx } else { 0.0 };
        let q95 = q95v(lv, sc, period);
        let ns = (short.len / qs).floor();
        q95s = math_max(q95s, q95);
        r = math_min(r, if q95 > 1e-9 { math_min(1.0, ns / q95) } else { 1.0 });
    }
    let cap_a = c_sh + (c_sep - c_sh) * r;
    let x = tot / cap_a;
    ApproachCap {
        x,
        cap: cap_a,
        r,
        q95short: Some(q95s),
        lanes: lanes_out(&sep, if xs > 0.0 { x / xs } else { 1.0 }),
    }
}

impl ApproachCap {
    /// As JSON { X, cap, r, q95short?, lanes }.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("X".into(), plain_number(self.x));
        m.insert("cap".into(), plain_number(self.cap));
        m.insert("r".into(), plain_number(self.r));
        if let Some(q) = self.q95short {
            m.insert("q95short".into(), plain_number(q));
        }
        m.insert("lanes".into(), lane_rows_value(self.lanes.as_deref()));
        Value::Object(m)
    }
}

/// Lane rows as JSON, null when absent.
pub(crate) fn lane_rows_value(l: Option<&[LaneRow]>) -> Value {
    match l {
        Some(rows) => Value::Array(rows.iter().map(LaneRow::to_value).collect()),
        None => Value::Null,
    }
}
