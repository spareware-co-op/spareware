//! Junction controllers: setupControls, makeCtl, applySignal and the site helpers they use.
//!
//! Arc and node indexes come from the graph itself, so every lookup finds its entry; an index
//! that did not (which the JavaScript would throw on) is skipped.

use super::sites::site_entries;
use super::sort::{js_sort_by, js_str_lt};
use super::zones::uf_find as find;
use super::{Ctl, CtlSrc, Ctrl, Graph, Leg, Priority, SignalTiming, SiteCtl};
use crate::jsnum::{Maybe, Num, math_max, math_max_of, math_min, math_round, or_default, value_to_number};
use crate::junction::prep_moves;
use crate::project::{OrderedMap, Scenario, Settings, Site};
use crate::standards::CapMethod;
use crate::util::ang_diff;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// `hashOffset(s)`: a signal offset (0 to 96 s) from a controller key.
#[must_use]
pub fn hash_offset(s: &str) -> i32 {
    let mut h: i32 = 7;
    for c in s.encode_utf16() {
        // (h * 31 + c) | 0: the JavaScript's 32-bit wrap
        h = h.wrapping_mul(31).wrapping_add(i32::from(c));
    }
    (i64::from(h).abs() % 97) as i32
}

const CTL_NAMES: [&str; 5] = ["none", "priority", "stop", "roundabout", "signals"];

fn ctl_rank(c: &str) -> Option<usize> {
    CTL_NAMES.iter().position(|&name| name == c)
}

/// `siteControl(sc, s)`: the site's own control, or the strongest of its nodes' controls.
#[must_use]
pub fn site_control(sc: &Scenario, s: &Site) -> &'static str {
    let name = |r: usize| CTL_NAMES.get(r).copied().unwrap_or("none");
    if let Some(c) = s.control.as_str()
        && let Some(r) = ctl_rank(c)
        && r > 0
    {
        return name(r);
    }
    let mut best = 0;
    for id in &s.nodes {
        let c = sc.nodes.get(id).map_or("none", |n| n.control.str_or("none"));
        let r = ctl_rank(c).unwrap_or(0);
        if r > best {
            best = r;
        }
    }
    name(best)
}

// nameKey(s): lower case, runs of anything but a-z and 0-9 as one space, trimmed
fn name_key(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in s.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap {
                out.push(' ');
                gap = false;
            }
            out.push(c);
        } else {
            gap = true;
        }
    }
    if gap {
        out.push(' ');
    }
    out.trim_matches(' ').to_string()
}

// A canonical array index ("0", "17"): JavaScript lists such keys first, in ascending order
fn array_index(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0')) {
        return None;
    }
    s.parse::<u64>()
        .ok()
        .filter(|&v| v < 4_294_967_295)
        .and_then(|v| u32::try_from(v).ok())
}

/// `siteLegs(G, inc, exits)`: the legs of a site, its entering and leaving arcs grouped by road name (within 90 degrees) or bearing (35 degrees).
#[must_use]
pub fn site_legs(g: &Graph, inc: &[usize], exits: &[usize]) -> Vec<Leg> {
    struct It<'a> {
        i: usize,
        inn: bool,
        b: f64,
        nm: String,
        name: &'a str,
        rank: f64,
    }
    let item = |i: usize, inn: bool| {
        g.arcs.get(i).map(|a| It {
            i,
            inn,
            b: if inn {
                ((a.in_b + 180.0) % 360.0 + 360.0) % 360.0
            } else {
                ((a.out_b % 360.0) + 360.0) % 360.0
            },
            nm: name_key(&a.name),
            name: &a.name,
            rank: a.rank,
        })
    };
    let it: Vec<It> = inc
        .iter()
        .filter_map(|&i| item(i, true))
        .chain(exits.iter().filter_map(|&i| item(i, false)))
        .collect();
    let mut parent: Vec<usize> = (0..it.len()).collect();
    for (ka, ia) in it.iter().enumerate() {
        for (kb, ib) in it.iter().enumerate().skip(ka.saturating_add(1)) {
            let diff = ang_diff(ia.b, ib.b);
            if (!ia.nm.is_empty() && ia.nm == ib.nm && diff <= 90.0) || diff <= 35.0 {
                let ra = find(&mut parent, ka);
                let rb = find(&mut parent, kb);
                if ra != rb
                    && let Some(slot) = parent.get_mut(rb)
                {
                    *slot = ra;
                }
            }
        }
    }
    let mut groups: Vec<(usize, Vec<&It>)> = Vec::new();
    for (k, x) in it.iter().enumerate() {
        let r = find(&mut parent, k);
        match groups.iter_mut().find(|g| g.0 == r) {
            Some(gr) => gr.1.push(x),
            None => groups.push((r, vec![x])),
        }
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(k, (_, gr))| {
            let (mut sx, mut sy) = (0.0f64, 0.0f64);
            for x in &gr {
                sx += crate::jsmath::sin(x.b * std::f64::consts::PI / 180.0);
                sy += crate::jsmath::cos(x.b * std::f64::consts::PI / 180.0);
            }
            // cnt in object key order: array-index names first (ascending), then insertion order
            let mut cnt: Vec<(String, f64)> = Vec::new();
            for x in &gr {
                let n = x.name;
                if n.is_empty() {
                    continue;
                }
                match cnt.iter_mut().find(|c| c.0 == n) {
                    Some(c) => c.1 += 1.0,
                    None => cnt.push((n.to_string(), 1.0)),
                }
            }
            let mut keys: Vec<(String, f64)> = cnt.iter().filter(|c| array_index(&c.0).is_some()).cloned().collect();
            keys.sort_by_key(|c| array_index(&c.0));
            keys.extend(cnt.iter().filter(|c| array_index(&c.0).is_none()).cloned());
            js_sort_by(&mut keys, |a, b| b.1 - a.1);
            let ranks: Vec<f64> = gr.iter().map(|x| or_default(x.rank, 0.0)).collect();
            Leg {
                k,
                b: math_round(
                    ((sx.atan2(sy) * 180.0 / std::f64::consts::PI) + 360.0) % 360.0,
                ),
                name: keys.first().map_or(String::new(), |c| c.0.clone()),
                rank: math_max_of(&ranks),
                ins: gr.iter().filter(|x| x.inn).map(|x| x.i).collect(),
                outs: gr.iter().filter(|x| !x.inn).map(|x| x.i).collect(),
            }
        })
        .collect()
}

/// `setupControls(sc, G, S)`: one controller per controlled node, or one over a site (signals, give way, stop); movement entries with the movement method.
pub(crate) fn setup_controls(sc: &Scenario, g: &mut Graph, s: &Settings) {
    // Sites with one controller (signals, give-way, stop); roundabout sites keep one controller per entry node
    let mut one: OrderedMap<(usize, &'static str)> = OrderedMap::new();
    for (k, gs) in g.sites.iter().enumerate() {
        let t = site_control(sc, &gs.site);
        if gs.members.len() > 1 && (t == "signals" || t == "priority" || t == "stop") {
            one.insert(gs.site.id.clone(), (k, t));
        }
    }
    node_controls(sc, g, s, &one);
    site_controls(g, s, &one);
    if g.cap_method == CapMethod::Movement {
        for k in 0..g.ctl.len() {
            prep_moves(sc, g, k);
        }
    }
}

// setupControls' first part: a controller at each controlled node outside the one-controller sites
fn node_controls(sc: &Scenario, g: &mut Graph, s: &Settings, one: &OrderedMap<(usize, &'static str)>) {
    for n in 0..g.n {
        if g.is_c().get(n).is_none_or(|&c| c != 0) {
            continue;
        }
        let sid = g.site_of.get(n).cloned().flatten();
        if let Some(sid) = &sid
            && one.contains_key(sid)
        {
            continue;
        }
        let into = g.into.get(n).map_or(&[][..], Vec::as_slice);
        let out_r = g.out_r.get(n).map_or(&[][..], Vec::as_slice);
        // Internal links of a (roundabout) site run free; only ring arcs and arcs from outside are entries
        let inc: Vec<usize> = if sid.is_none() {
            into.to_vec()
        } else {
            into.iter()
                .copied()
                .filter(|&i| g.arcs.get(i).is_some_and(|a| !a.internal || a.ring))
                .collect()
        };
        if inc.is_empty() {
            continue;
        }
        let Some(node) = g.ids.get(n).and_then(|id| sc.nodes.get(id)) else {
            continue;
        };
        let kind = node.control.str_or("none").to_string();
        if kind == "none" {
            continue;
        }
        let links: HashSet<&str> = inc
            .iter()
            .chain(out_r)
            .filter_map(|&i| g.arcs.get(i))
            .map(|a| a.link.as_str())
            .collect();
        if links.len() < if kind == "signals" { 2 } else { 3 } {
            continue;
        }
        // Motorway merges and diverges run free; the downstream link's capacity governs them
        if kind != "signals"
            && inc
                .iter()
                .chain(out_r)
                .all(|&i| g.arcs.get(i).is_some_and(|a| a.rank >= 5.0))
        {
            continue;
        }
        let src = CtlSrc {
            site: false,
            id: node.id.clone(),
            cycle: node.cycle.clone(),
            fixed_timing: node.fixed_timing.clone(),
        };
        let split = node.split.clone();
        make_ctl(g, s, &kind, n, inc, src, &split, None);
    }
}

// setupControls' second part: one controller over each one-controller site, at its representative node
fn site_controls(g: &mut Graph, s: &Settings, one: &OrderedMap<(usize, &'static str)>) {
    for (_, &(k, kind)) in one.iter() {
        let Some(gs) = g.sites.get(k) else {
            continue;
        };
        let site = gs.site.clone();
        let members = gs.members.clone();
        let (inc, exits) = site_entries(g, &members);
        if inc.len() < 2 {
            continue;
        }
        let mut sorted = members.clone();
        let deg = |x: usize| f64::from(g.deg.get(x).copied().unwrap_or(0));
        let id = |x: usize| g.ids.get(x).map_or("", String::as_str);
        js_sort_by(&mut sorted, |&a, &b| {
            // Degrees are small whole numbers, so their difference is exact as a double
            let d = deg(b) - deg(a);
            if d != 0.0 {
                d
            } else if js_str_lt(id(a), id(b)) {
                -1.0
            } else {
                1.0
            }
        });
        let Some(&rep) = sorted.first() else {
            continue;
        };
        let legs = site_legs(g, &inc, &exits);
        if kind != "signals" && legs.len() < 3 {
            continue;
        }
        let src = CtlSrc {
            site: true,
            id: site.id.clone(),
            cycle: site.cycle.clone(),
            fixed_timing: site.fixed_timing.clone(),
        };
        let sc_ctl = SiteCtl {
            site: site.id.clone(),
            members,
            legs,
            exits,
            leg_in: HashMap::new(),
            leg_out: HashMap::new(),
        };
        make_ctl(g, s, kind, rep, inc, src, &site.split, Some(sc_ctl));
    }
}

// G.ctl.set(n, ctl): a new node goes last, an existing node keeps its place
fn set_ctl(g: &mut Graph, ctl: Ctl) {
    let n = ctl.n;
    let at = g.ctl_at.get(n).copied().unwrap_or(-1);
    if let Ok(k) = usize::try_from(at) {
        if let Some(slot) = g.ctl.get_mut(k) {
            *slot = ctl;
        }
    } else {
        // Bound: one controller per node at most, and node counts are far below i32::MAX
        if let Some(slot) = g.ctl_at.get_mut(n) {
            *slot = i32::try_from(g.ctl.len()).unwrap_or(i32::MAX);
        }
        g.ctl.push(ctl);
    }
}

/// `makeCtl(G, S, type, n, inc, src, site)`: one controller over the entry arcs inc at node n (or at a site's representative node).
#[allow(clippy::too_many_arguments)]
fn make_ctl(
    g: &mut Graph,
    s: &Settings,
    kind: &str,
    n: usize,
    inc: Vec<usize>,
    src: CtlSrc,
    split: &Maybe<Value>,
    site: Option<SiteCtl>,
) {
    let mut site = site;
    if let Some(st) = &mut site {
        for l in &st.legs {
            for &i in &l.ins {
                st.leg_in.insert(i, l.k);
            }
            for &i in &l.outs {
                st.leg_out.insert(i, l.k);
            }
        }
    }
    let key = match &site {
        Some(st) => st.site.clone(),
        None => g.ids.get(n).cloned().unwrap_or_default(),
    };
    if kind == "signals" {
        let timing = signal_timing(g, s, n, &inc, &src.cycle, split, &key);
        let ctl = Ctl {
            kind: kind.to_string(),
            n,
            arcs: inc,
            src,
            signal: Some(timing),
            major: None,
            site,
            ent: None,
        };
        apply_signal(&ctl, &mut g.arcs);
        set_ctl(g, ctl);
    } else if kind == "roundabout" {
        roundabout_entries(g, n, &inc);
        set_ctl(
            g,
            Ctl {
                kind: kind.to_string(),
                n,
                arcs: inc,
                src,
                signal: None,
                major: None,
                site,
                ent: None,
            },
        );
    } else {
        let major = priority_entries(g, kind, n, &inc, site.as_ref());
        set_ctl(
            g,
            Ctl {
                kind: kind.to_string(),
                n,
                arcs: inc,
                src,
                signal: None,
                major: Some(major),
                site,
                ent: None,
            },
        );
    }
}

// makeCtl for signals: each entry's phase (against the top-ranked entry's bearing) and the timing
fn signal_timing(
    g: &mut Graph,
    s: &Settings,
    n: usize,
    inc: &[usize],
    cycle: &Num,
    split: &Maybe<Value>,
    key: &str,
) -> SignalTiming {
    let rank = |i: usize| g.arcs.get(i).map_or(f64::NAN, |a| a.rank);
    let mut top = inc.first().copied().unwrap_or(0);
    for &i in inc {
        if rank(i) > rank(top) {
            top = i;
        }
    }
    let ref_in_b = g.arcs.get(top).map_or(f64::NAN, |a| a.in_b);
    let mut phases: HashSet<u8> = HashSet::new();
    for &i in inc {
        let Some(arc) = g.arcs.get_mut(i) else {
            continue;
        };
        let diff = ang_diff(arc.in_b, ref_in_b);
        let ph = u8::from(math_min(diff, 180.0 - diff) >= 45.0);
        arc.phase = Some(ph);
        phases.insert(ph);
    }
    let single = phases.len() == 1;
    let split = if single {
        vec![0.75, 0.25]
    } else {
        match split {
            Maybe::Set(Value::Array(a)) if a.len() == 2 => a.iter().map(value_to_number).collect(),
            _ => vec![0.5, 0.5],
        }
    };
    let timing = SignalTiming {
        single,
        c: or_default(num_of(cycle), s.cycle.num()),
        lost: s.lost_per_phase.num() * 2.0,
        split,
        offset: hash_offset(key),
    };
    for &i in inc {
        let Some(a) = g.arcs.get_mut(i) else {
            continue;
        };
        a.ctrl = Ctrl::Sig;
        a.node = Some(n);
        if single {
            a.phase = Some(0);
        }
    }
    timing
}

// makeCtl for a roundabout: ring arcs run free; each entry gives way to the circulating flow
fn roundabout_entries(g: &mut Graph, n: usize, inc: &[usize]) {
    let is_ring = |i: &usize| g.arcs.get(*i).is_some_and(|a| a.ring);
    let ring_in: Vec<usize> = inc.iter().copied().filter(is_ring).collect();
    let ring_out: Vec<usize> = g
        .out_r
        .get(n)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .copied()
        .filter(is_ring)
        .collect();
    for &i in inc {
        let Some(a) = g.arcs.get_mut(i) else {
            continue;
        };
        a.node = Some(n);
        if a.ring {
            a.ctrl = Ctrl::Free;
            continue;
        }
        a.ctrl = Ctrl::Rbt;
        // Mapped roundabouts: circulating flow past this entry = ring flow leaving the node minus this entry's own flow
        if !ring_in.is_empty() && !ring_out.is_empty() {
            a.ring_out = Some(ring_out.clone());
            a.ring_in = Some(ring_in.clone());
            a.conf = Some(ring_in.iter().map(|&j| (j, 1.0)).collect());
        } else {
            a.conf = Some(inc.iter().copied().filter(|&j| j != i).map(|j| (j, 0.5)).collect());
        }
    }
}

// The pair of a list farthest apart in bearing, if more than 100 degrees apart; otherwise the first
fn widest_pair<T: Copy>(top: &[T], bearing: impl Fn(T) -> f64) -> Vec<T> {
    let mut bp: Option<(T, T)> = None;
    let mut bd = -1.0;
    for (k, &p) in top.iter().enumerate() {
        for &q in top.iter().skip(k.saturating_add(1)) {
            let d = ang_diff(bearing(p), bearing(q));
            if d > bd {
                bd = d;
                bp = Some((p, q));
            }
        }
    }
    match bp {
        Some((a, b)) if bd > 100.0 => vec![a, b],
        _ => top.first().copied().into_iter().collect(),
    }
}

// makeCtl for give way and stop: the major road's entries run free; the others give way to them.
// Returns the major road's entries.
fn priority_entries(g: &mut Graph, kind: &str, n: usize, inc: &[usize], site: Option<&SiteCtl>) -> Vec<usize> {
    let pr = |i: usize| match g.arcs.get(i) {
        Some(a) if a.cls.as_deref() == Some("ramp") => 3.5,
        Some(a) => a.rank,
        None => f64::NAN,
    };
    let major: Vec<usize> = if let Some(st) = site {
        // A give-way or stop site: the major road is the top-ranked pair of legs (both carriageways of each)
        let prl = |l: &Leg| math_max(math_max_of(&l.ins.iter().map(|&i| pr(i)).collect::<Vec<_>>()), -1.0);
        let with_in: Vec<&Leg> = st.legs.iter().filter(|l| !l.ins.is_empty()).collect();
        let max_rank = math_max_of(&with_in.iter().map(|l| prl(l)).collect::<Vec<_>>());
        let top: Vec<&Leg> = with_in.iter().copied().filter(|l| prl(l) == max_rank).collect();
        let ml: Vec<&Leg> = if top.len() <= 2 && top.len() < with_in.len() {
            top
        } else {
            widest_pair(&top, |l| l.b)
        };
        ml.iter().flat_map(|l| l.ins.iter().copied()).collect()
    } else {
        // Ramps give way at their terminals with local roads
        let mut max_rank = 0.0;
        for &i in inc {
            max_rank = math_max(max_rank, pr(i));
        }
        let top: Vec<usize> = inc.iter().copied().filter(|&i| pr(i) == max_rank).collect();
        if top.len() <= 2 && top.len() < inc.len() {
            top
        } else {
            widest_pair(&top, |i| g.arcs.get(i).map_or(f64::NAN, |a| a.in_b))
        }
    };
    let ms: HashSet<usize> = major.iter().copied().collect();
    for &i in inc {
        let Some(a) = g.arcs.get_mut(i) else {
            continue;
        };
        a.node = Some(n);
        if ms.contains(&i) {
            a.ctrl = Ctrl::Free;
            a.priority = Priority::Major;
        } else {
            a.ctrl = Ctrl::Minor;
            // Each arc enters one controller, so this is its first and only priority
            a.priority = if kind == "stop" { Priority::Stop } else { Priority::Neither };
            a.conf = Some(major.iter().map(|&j| (j, 1.0)).collect());
        }
    }
    major
}

fn num_of(v: &Num) -> f64 {
    v.num()
}

/// `applySignal(ctl, G)`: each signal approach's cycle and effective green ratio (at least 0.08) from the controller's timing.
pub fn apply_signal(ctl: &Ctl, arcs: &mut [super::Arc]) {
    let Some(t) = &ctl.signal else {
        return;
    };
    let gr = t.c - t.lost;
    for &i in &ctl.arcs {
        let Some(a) = arcs.get_mut(i) else {
            continue;
        };
        a.c = Some(t.c);
        let ph = usize::from(a.phase.unwrap_or(0));
        let sp = t.split.get(ph).copied().unwrap_or(f64::NAN);
        a.g_c = Some(math_max(0.08, gr * sp / t.c));
    }
}
