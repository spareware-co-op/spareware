//! Link and junction performance: arc travel time, effective capacity and the gap-acceptance capacity update.

use super::movement::{MajorState, d_hcm};
use super::moves::movement_caps;
use crate::graph::{Arc, Ctrl, Graph};
use crate::jsnum::{math_max, math_min, or_default};
use crate::project::Settings;
use crate::standards::{CapMethod, junction_set};

/// The settings the performance functions read, once: BPR alpha and beta, saturation flow, and the junction block's peak flow factor and analysis period.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerfSet {
    /// BPR alpha (`S.bprAlpha`).
    pub bpr_alpha: f64,
    /// BPR beta (`S.bprBeta`).
    pub bpr_beta: f64,
    /// Saturation flow per lane (`S.satFlow`).
    pub sat_flow: f64,
    /// Peak flow factor (1 when the junction block is missing or out of range).
    pub pff: f64,
    /// Analysis period (h; 1 when missing).
    pub t: f64,
}

impl PerfSet {
    /// Reads the settings as arcTime does (`S.junction.pff` within 0 to 1, `S.junction.T` above 0, else 1).
    pub fn new(s: &Settings) -> PerfSet {
        let j = junction_set(s);
        PerfSet {
            bpr_alpha: s.bpr_alpha.num(),
            bpr_beta: s.bpr_beta.num(),
            sat_flow: s.sat_flow.num(),
            pff: j.pff,
            t: j.t,
        }
    }
}

/// `arcTime(a, x, S)`: travel time (s) on an arc carrying x pcu/h: BPR running time plus delay at the downstream junction.
///
/// Signals: Webster's uniform delay and the HCM overflow delay; give way, stop and roundabout
/// entries: HCM 6 control delay on the gap-acceptance capacity, plus 5 s at a stop, 5 s times
/// the degree of saturation (at most 1) at a roundabout and 2 s at give way. Junction delay uses
/// the peak flow rate x / PFF and the analysis period T. Connectors and internal site links run
/// at free-flow time; a major approach with right turners adds their delay (movement method).
pub fn arc_time(a: &Arc, x: f64, p: &PerfSet) -> f64 {
    if a.conn || a.internal {
        return a.ff;
    }
    let mut t = a.ff * (1.0 + p.bpr_alpha * (x / a.cap_mid).powf(p.bpr_beta));
    if a.ctrl == Ctrl::Free {
        return match &a.mj {
            Some(mj) => t + major_delay(mj, x, p),
            None => t,
        };
    }
    let (pff, tt) = (p.pff, p.t);
    let q = x / pff;
    if a.ctrl == Ctrl::Sig {
        let g_c = a.g_c.unwrap_or(f64::NAN);
        let c = math_max(1.0, a.gl as f64 * p.sat_flow * g_c);
        let xx = q / c;
        let g = g_c;
        let d1 = 0.5 * a.c.unwrap_or(f64::NAN) * (1.0 - g) * (1.0 - g) / (1.0 - math_min(1.0, xx) * g);
        let d2 = 900.0 * tt * ((xx - 1.0) + ((xx - 1.0) * (xx - 1.0) + 4.0 * xx / (c * tt)).sqrt());
        t += d1 + d2;
    } else {
        let c = math_max(1.0, or_default(a.cap_n.unwrap_or(f64::NAN), a.cap_mid));
        let xx = q / c;
        let h = 3600.0 / c;
        let extra = if a.stop {
            5.0
        } else if a.ctrl == Ctrl::Rbt {
            5.0 * math_min(1.0, xx)
        } else {
            2.0
        };
        t += h
            + 900.0 * tt * ((xx - 1.0) + ((xx - 1.0) * (xx - 1.0) + h * xx / (450.0 * tt)).sqrt())
            + extra;
    }
    t
}

/// `effCap(a, S)`: capacity (pcu/h) of an arc: infinite for a connector, the signal or gap-acceptance capacity where lower than mid-block, else mid-block.
pub fn eff_cap(a: &Arc, p: &PerfSet) -> f64 {
    if a.conn {
        return f64::INFINITY;
    }
    match a.ctrl {
        Ctrl::Sig => math_min(
            a.cap_mid,
            a.gl as f64 * p.sat_flow * a.g_c.unwrap_or(f64::NAN),
        ),
        Ctrl::Minor | Ctrl::Rbt => {
            math_min(a.cap_mid, or_default(a.cap_n.unwrap_or(f64::NAN), a.cap_mid))
        }
        Ctrl::Free => a.cap_mid,
    }
}

/// `arcCap(a, xt)`: one gap-acceptance capacity per arc (arc method): a minor arc against all major flow, a roundabout entry against circulating flow; None for other arcs.
pub fn arc_cap(a: &Arc, xt: &[f64]) -> Option<f64> {
    match a.ctrl {
        Ctrl::Minor => {
            let mut vc = 0.0;
            for &(j, f) in a.conf.iter().flatten() {
                vc += xt[j] * f;
            }
            vc = math_max(vc, 1.0);
            let (tc, tf) = if a.stop { (6.5, 3.5) } else { (6.0, 3.2) };
            let cp = vc * (-vc * tc / 3600.0).exp() / (1.0 - (-vc * tf / 3600.0).exp());
            Some(math_max(
                80.0,
                math_min(a.cap_mid, cp * (1.0 + 0.6 * (a.gl as f64 - 1.0))),
            ))
        }
        Ctrl::Rbt => {
            let mut vc = 0.0;
            if let Some(ro) = &a.ring_out {
                for &j in ro {
                    vc += xt[j];
                }
                vc = math_max(0.0, vc - xt[a.i]);
            } else {
                for &(j, f) in a.conf.iter().flatten() {
                    vc += xt[j] * f;
                }
            }
            let per = if a.gl > 1 {
                1420.0 * (-0.00091 * vc).exp()
            } else {
                1380.0 * (-0.00102 * vc).exp()
            };
            Some(math_min(a.cap_mid, math_max(50.0, per * a.gl as f64)))
        }
        _ => None,
    }
}

/// `updateNodeCaps(G, xt, w, xT)`: refreshes the gap-acceptance capacities from arc flows xt (and turn flows xT for the movement method).
///
/// w below 1 damps the update (MSA: capN moves w of the way to the new value): these
/// capacities make the costs non-separable, and undamped updates make the assignment oscillate.
pub fn update_node_caps(g: &mut Graph, xt: &[f64], w: f64, x_t: Option<&[f64]>) {
    if g.cap_method == CapMethod::Movement {
        match x_t {
            Some(x) => movement_caps(g, xt, w, x),
            None => {
                let zeros = vec![0.0; g.n_t];
                movement_caps(g, xt, w, &zeros);
            }
        }
        return;
    }
    for a in &mut g.arcs {
        if let Some(cap) = arc_cap(a, xt) {
            a.cap_n = Some(match a.cap_n {
                Some(old) if w < 1.0 && old > 0.0 => old + (cap - old) * w,
                _ => cap,
            });
        }
    }
}

/// `majorDelay(mj, x, S)`: delay (s) a major approach adds at x pcu/h for its right turners and the through traffic they block.
pub fn major_delay(mj: &MajorState, x: f64, p: &PerfSet) -> f64 {
    if !(mj.s_r > 0.0) || !(mj.cm_r > 0.0) {
        return 0.0;
    }
    let v_r = mj.s_r * x / p.pff;
    let p0 = math_max(0.0, 1.0 - v_r / mj.cm_r);
    let p_e = p0 + (1.0 - p0) * mj.rb;
    let d_r = d_hcm(v_r, mj.cm_r, p.t) + 2.0;
    mj.s_r * d_r + mj.sh_tl * (1.0 - p_e) * d_r
}
