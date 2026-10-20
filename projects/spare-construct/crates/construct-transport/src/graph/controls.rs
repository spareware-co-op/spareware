//! Junction controllers: setupControls, makeCtl, applySignal and the site helpers they use.

use super::sites::site_entries;
use super::sort::{js_sort_by, js_str_lt};
use super::{Ctl, CtlSrc, Ctrl, Graph, Leg, SignalTiming, SiteCtl};
use crate::jsnum::{Maybe, Num, math_max, math_max_of, math_min, math_round, or_default, value_to_number};
use crate::junction::prep_moves;
use crate::project::{OrderedMap, Scenario, Settings, Site};
use crate::standards::CapMethod;
use crate::util::ang_diff;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// `hashOffset(s)`: a signal offset (0 to 96 s) from a controller key.
pub fn hash_offset(s: &str) -> i32 {
    let mut h: i32 = 7;
    for c in s.encode_utf16() {
        h = (h as i64 * 31 + c as i64) as i32;
    }
    ((h as i64).abs() % 97) as i32
}

fn ctl_rank(c: &str) -> Option<i32> {
    match c {
        "none" => Some(0),
        "priority" => Some(1),
        "stop" => Some(2),
        "roundabout" => Some(3),
        "signals" => Some(4),
        _ => None,
    }
}

/// `siteControl(sc, s)`: the site's own control, or the strongest of its nodes' controls.
pub fn site_control(sc: &Scenario, s: &Site) -> &'static str {
    const NAMES: [&str; 5] = ["none", "priority", "stop", "roundabout", "signals"];
    if let Some(c) = s.control.as_str()
        && let Some(r) = ctl_rank(c)
        && r > 0
    {
        return NAMES[r as usize];
    }
    let mut best = 0;
    for id in &s.nodes {
        let c = sc.nodes.get(id).map_or("none", |n| n.control.str_or("none"));
        let r = ctl_rank(c).unwrap_or(0);
        if r > best {
            best = r;
        }
    }
    NAMES[best as usize]
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
    s.parse::<u64>().ok().filter(|&v| v < 4294967295).map(|v| v as u32)
}

/// `siteLegs(G, inc, exits)`: the legs of a site, its entering and leaving arcs grouped by road name (within 90 degrees) or bearing (35 degrees).
pub fn site_legs(g: &Graph, inc: &[usize], exits: &[usize]) -> Vec<Leg> {
    struct It {
        i: usize,
        inn: bool,
        b: f64,
        nm: String,
    }
    let mut it: Vec<It> = Vec::new();
    for &i in inc {
        it.push(It {
            i,
            inn: true,
            b: ((g.arcs[i].in_b + 180.0) % 360.0 + 360.0) % 360.0,
            nm: String::new(),
        });
    }
    for &i in exits {
        it.push(It {
            i,
            inn: false,
            b: ((g.arcs[i].out_b % 360.0) + 360.0) % 360.0,
            nm: String::new(),
        });
    }
    for x in &mut it {
        x.nm = name_key(&g.arcs[x.i].name);
    }
    let mut p: Vec<usize> = (0..it.len()).collect();
    fn find(p: &mut [usize], mut k: usize) -> usize {
        while p[k] != k {
            p[k] = p[p[k]];
            k = p[k];
        }
        k
    }
    for a in 0..it.len() {
        for b in a + 1..it.len() {
            let d = ang_diff(it[a].b, it[b].b);
            if (!it[a].nm.is_empty() && it[a].nm == it[b].nm && d <= 90.0) || d <= 35.0 {
                let x = find(&mut p, a);
                let y = find(&mut p, b);
                if x != y {
                    p[y] = x;
                }
            }
        }
    }
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for k in 0..it.len() {
        let r = find(&mut p, k);
        match groups.iter_mut().find(|g| g.0 == r) {
            Some(gr) => gr.1.push(k),
            None => groups.push((r, vec![k])),
        }
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(k, (_, gr))| {
            let (mut sx, mut sy) = (0.0f64, 0.0f64);
            for &x in &gr {
                sx += (it[x].b * std::f64::consts::PI / 180.0).sin();
                sy += (it[x].b * std::f64::consts::PI / 180.0).cos();
            }
            // cnt in object key order: array-index names first (ascending), then insertion order
            let mut cnt: Vec<(String, f64)> = Vec::new();
            for &x in &gr {
                let n = &g.arcs[it[x].i].name;
                if n.is_empty() {
                    continue;
                }
                match cnt.iter_mut().find(|c| &c.0 == n) {
                    Some(c) => c.1 += 1.0,
                    None => cnt.push((n.clone(), 1.0)),
                }
            }
            let mut keys: Vec<(String, f64)> = cnt.iter().filter(|c| array_index(&c.0).is_some()).cloned().collect();
            keys.sort_by_key(|c| array_index(&c.0));
            keys.extend(cnt.iter().filter(|c| array_index(&c.0).is_none()).cloned());
            js_sort_by(&mut keys, |a, b| b.1 - a.1);
            let ranks: Vec<f64> = gr.iter().map(|&x| or_default(g.arcs[it[x].i].rank, 0.0)).collect();
            Leg {
                k,
                b: math_round(
                    ((sx.atan2(sy) * 180.0 / std::f64::consts::PI) + 360.0) % 360.0,
                ),
                name: keys.first().map_or(String::new(), |c| c.0.clone()),
                rank: math_max_of(&ranks),
                ins: gr.iter().filter(|&&x| it[x].inn).map(|&x| it[x].i).collect(),
                outs: gr.iter().filter(|&&x| !it[x].inn).map(|&x| it[x].i).collect(),
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
    for n in 0..g.n {
        if g.is_c()[n] != 0 {
            continue;
        }
        let sid = g.site_of[n].clone();
        if let Some(sid) = &sid
            && one.contains_key(sid)
        {
            continue;
        }
        // Internal links of a (roundabout) site run free; only ring arcs and arcs from outside are entries
        let inc: Vec<usize> = if sid.is_none() {
            g.into[n].clone()
        } else {
            g.into[n]
                .iter()
                .copied()
                .filter(|&i| !g.arcs[i].internal || g.arcs[i].ring)
                .collect()
        };
        if inc.is_empty() {
            continue;
        }
        let node = sc
            .nodes
            .get(&g.ids[n])
            .expect("a network node is a saved node");
        let kind = node.control.str_or("none").to_string();
        if kind == "none" {
            continue;
        }
        let mut links: HashSet<&str> = HashSet::new();
        for &i in &inc {
            links.insert(&g.arcs[i].link);
        }
        for &i in &g.out_r[n] {
            links.insert(&g.arcs[i].link);
        }
        if links.len() < if kind == "signals" { 2 } else { 3 } {
            continue;
        }
        // Motorway merges and diverges run free; the downstream link's capacity governs them
        if kind != "signals"
            && inc.iter().chain(g.out_r[n].iter()).all(|&i| g.arcs[i].rank >= 5.0)
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
    for (_, &(k, kind)) in one.iter() {
        let site = g.sites[k].site.clone();
        let members = g.sites[k].members.clone();
        let (inc, exits) = site_entries(g, &members);
        if inc.len() < 2 {
            continue;
        }
        let mut sorted = members.clone();
        js_sort_by(&mut sorted, |&a, &b| {
            let d = (g.deg[b] - g.deg[a]) as f64;
            if d != 0.0 {
                d
            } else if js_str_lt(&g.ids[a], &g.ids[b]) {
                -1.0
            } else {
                1.0
            }
        });
        let rep = sorted[0];
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
    if g.cap_method == CapMethod::Movement {
        for k in 0..g.ctl.len() {
            prep_moves(sc, g, k);
        }
    }
}

// G.ctl.set(n, ctl): a new node goes last, an existing node keeps its place
fn set_ctl(g: &mut Graph, ctl: Ctl) {
    let n = ctl.n;
    if g.ctl_at[n] >= 0 {
        g.ctl[g.ctl_at[n] as usize] = ctl;
    } else {
        g.ctl_at[n] = g.ctl.len() as i32;
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
        None => g.ids[n].clone(),
    };
    if kind == "signals" {
        let mut r = inc[0];
        for &i in &inc {
            if g.arcs[i].rank > g.arcs[r].rank {
                r = i;
            }
        }
        let ref_in_b = g.arcs[r].in_b;
        let mut phases: HashSet<u8> = HashSet::new();
        for &i in &inc {
            let d = ang_diff(g.arcs[i].in_b, ref_in_b);
            let ph = if math_min(d, 180.0 - d) < 45.0 { 0 } else { 1 };
            g.arcs[i].phase = Some(ph);
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
        let c = or_default(num_of(&src.cycle), s.cycle.num());
        let timing = SignalTiming {
            single,
            c,
            lost: s.lost_per_phase.num() * 2.0,
            split,
            offset: hash_offset(&key),
        };
        for &i in &inc {
            let a = &mut g.arcs[i];
            a.ctrl = Ctrl::Sig;
            a.node = Some(n);
            if single {
                a.phase = Some(0);
            }
        }
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
        let ring_in: Vec<usize> = inc.iter().copied().filter(|&i| g.arcs[i].ring).collect();
        let ring_out: Vec<usize> = g.out_r[n].iter().copied().filter(|&i| g.arcs[i].ring).collect();
        for &i in &inc {
            let a = &mut g.arcs[i];
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
        let pr = |i: usize| {
            if g.arcs[i].cls.as_deref() == Some("ramp") {
                3.5
            } else {
                g.arcs[i].rank
            }
        };
        let major: Vec<usize> = if let Some(st) = &site {
            // A give-way or stop site: the major road is the top-ranked pair of legs (both carriageways of each)
            let prl = |l: &Leg| math_max(math_max_of(&l.ins.iter().map(|&i| pr(i)).collect::<Vec<_>>()), -1.0);
            let with_in: Vec<&Leg> = st.legs.iter().filter(|l| !l.ins.is_empty()).collect();
            let max_rank = math_max_of(&with_in.iter().map(|l| prl(l)).collect::<Vec<_>>());
            let top: Vec<&Leg> = with_in.iter().copied().filter(|l| prl(l) == max_rank).collect();
            let ml: Vec<&Leg> = if top.len() <= 2 && top.len() < with_in.len() {
                top
            } else {
                let mut bp: Option<(&Leg, &Leg)> = None;
                let mut bd = -1.0;
                for p in 0..top.len() {
                    for q in p + 1..top.len() {
                        let d = ang_diff(top[p].b, top[q].b);
                        if d > bd {
                            bd = d;
                            bp = Some((top[p], top[q]));
                        }
                    }
                }
                match bp {
                    Some((a, b)) if bd > 100.0 => vec![a, b],
                    _ => top.first().copied().into_iter().collect(),
                }
            };
            ml.iter().flat_map(|l| l.ins.iter().copied()).collect()
        } else {
            // Ramps give way at their terminals with local roads
            let mut max_rank = 0.0;
            for &i in &inc {
                max_rank = math_max(max_rank, pr(i));
            }
            let top: Vec<usize> = inc.iter().copied().filter(|&i| pr(i) == max_rank).collect();
            if top.len() <= 2 && top.len() < inc.len() {
                top
            } else {
                let mut bp: Option<(usize, usize)> = None;
                let mut bd = -1.0;
                for p in 0..top.len() {
                    for q in p + 1..top.len() {
                        let d = ang_diff(g.arcs[top[p]].in_b, g.arcs[top[q]].in_b);
                        if d > bd {
                            bd = d;
                            bp = Some((top[p], top[q]));
                        }
                    }
                }
                match bp {
                    Some((a, b)) if bd > 100.0 => vec![a, b],
                    _ => top.first().copied().into_iter().collect(),
                }
            }
        };
        let ms: HashSet<usize> = major.iter().copied().collect();
        for &i in &inc {
            let a = &mut g.arcs[i];
            a.node = Some(n);
            if ms.contains(&i) {
                a.ctrl = Ctrl::Free;
                a.major = true;
            } else {
                a.ctrl = Ctrl::Minor;
                a.stop = kind == "stop";
                a.conf = Some(major.iter().map(|&j| (j, 1.0)).collect());
            }
        }
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
        let a = &mut arcs[i];
        a.c = Some(t.c);
        let ph = a.phase.unwrap_or(0) as usize;
        let sp = t.split.get(ph).copied().unwrap_or(f64::NAN);
        a.g_c = Some(math_max(0.08, gr * sp / t.c));
    }
}
