//! Counted sites against a result (`Calibrate.sites`), count groups fitted as totals (`Calibrate.parallel`,
//! `Calibrate.units`) and the weights of counts in estimation.

use crate::datahub::{
    Count, Data, Match, Matches, OBS_METHODS, count_weight, geh, is_estimate, matches, model_flow,
    period_vol,
};
use crate::jsnum::math_max;
use crate::project::{Scenario, Settings};
use crate::results::Res;
use crate::standards::pcu_f;
use crate::util::{fmt, r1};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// `SEQ`: the rule for counts in sequence on one path (Spare Construct convention): more than max(1 pcu/h, 1% of the group's modelled flow) rejects the group.
pub const SEQ_MIN_PCU: f64 = 1.0;
/// `SEQ.share`.
pub const SEQ_SHARE: f64 = 0.01;
/// `SEQ.basis`.
pub const SEQ_BASIS: &str = "Spare Construct convention";

/// One counted site matched to the result: the count, where it matched, counted and modelled volume, GEH, the arcs it counts (by index into `arcKeys`) and its weight.
#[derive(Clone, Debug)]
pub struct SiteRow {
    /// The count.
    pub c: Rc<Count>,
    /// Where it was matched.
    pub mt: Match,
    /// Counted volume in the period (veh/h).
    pub obs: f64,
    /// Modelled flow (veh/h).
    pub model: f64,
    /// GEH.
    pub geh: f64,
    /// Indexes of the arcs the site counts (both directions for a two-way count).
    pub arcs: Vec<usize>,
    /// Weight in estimation.
    pub w: f64,
}

/// How [`sites`] picks counts and weighs them (`opts` of `Calibrate.sites`).
#[derive(Clone, Copy, Default)]
pub struct SiteOpts<'a> {
    /// Only these counts take part (`opts.countIds`).
    pub count_ids: Option<&'a HashSet<String>>,
    /// RAMM centreline estimates take part (as in the fit); validation statistics never use them.
    pub estimates: bool,
    /// Weights by observation method (`opts.weights === true`); otherwise every count weighs 1, except estimates (RAMM 0.25, virtual 0).
    pub weights: bool,
    /// A caller's own weights (`opts.weight`), which win over `weights`.
    pub weight: Option<&'a dyn Fn(&Count, &str) -> f64>,
}

/// `idSet(ids)`: the ids as a set.
pub fn id_set(ids: &[String]) -> HashSet<String> {
    ids.iter().cloned().collect()
}

fn weight_of(c: &Count, period: &str, o: &SiteOpts, s: &Settings) -> f64 {
    if let Some(f) = o.weight {
        let w = f(c, period);
        return if w >= 0.0 && w.is_finite() { w } else { 1.0 };
    }
    // Without weights every count weighs 1, except the estimates: RAMM centreline estimates 0.25, NZTA virtual sites 0
    if !o.weights {
        return if is_estimate(c) {
            OBS_METHODS[3].weight
        } else if c.virtual_.truthy() {
            0.0
        } else {
            1.0
        };
    }
    count_weight(c, period, s)
}

/// `Calibrate.sites(sc, res, period, opts)`: the counted sites in use, matched to the result, with the arcs each counts and its weight.
///
/// Same selection and model volume as `DataHub.calibration`. `m` is `DataHub.matches(sc)`. Counts are visited in
/// their order in the project.
pub fn sites(
    res: &Res,
    period: &str,
    opts: &SiteOpts,
    s: &Settings,
    data: &Data,
    m: &Matches,
) -> Vec<SiteRow> {
    let idx: HashMap<&str, usize> = match res.arc_keys.as_ref() {
        Some(k) => k.iter().enumerate().map(|(i, k)| (k.as_str(), i)).collect(),
        None => HashMap::new(),
    };
    let mut rows = Vec::new();
    for c in &data.counts {
        if !data.in_use(c) {
            continue;
        }
        if let Some(only) = opts.count_ids
            && !only.contains(&c.id)
        {
            continue;
        }
        if is_estimate(c) && !opts.estimates {
            continue;
        }
        let Some(mt) = m.get(&c.id) else { continue };
        let Some(pv) = period_vol(c, period, s) else {
            continue;
        };
        let Some(model) = model_flow(res, mt) else {
            continue;
        };
        let keys: Vec<String> = if mt.dir == "both" {
            vec![format!("{}:ab", mt.link), format!("{}:ba", mt.link)]
        } else {
            vec![format!("{}:{}", mt.link, mt.dir)]
        };
        let obs = pv.v;
        rows.push(SiteRow {
            c: c.clone(),
            mt: mt.clone(),
            obs,
            model,
            geh: geh(model, obs),
            arcs: keys
                .iter()
                .filter_map(|k| idx.get(k.as_str()).copied())
                .collect(),
            w: weight_of(c, period, opts, s),
        });
    }
    rows
}

/// [`sites`] for a scenario: matches the counts first.
pub fn sites_for(
    sc: &Scenario,
    res: &Res,
    period: &str,
    opts: &SiteOpts,
    s: &Settings,
    data: &Data,
) -> Vec<SiteRow> {
    sites(res, period, opts, s, data, &matches(sc, s, data))
}

/// A count group to fit as a total (`opts.groups`): counts that are parallel on the network.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    /// Group id (text or a number); the group's position when None.
    pub id: Option<Value>,
    /// Display name; the id when None or empty.
    pub label: Option<String>,
    /// Ids of the counts in the group.
    pub count_ids: Vec<String>,
}

fn id_text(id: &Option<Value>, gi: usize) -> String {
    match id {
        Some(v) if !v.is_null() => crate::jsnum::value_to_js_string(v),
        _ => gi.to_string(),
    }
}

fn id_value(id: &Option<Value>, gi: usize) -> Value {
    match id {
        Some(v) if !v.is_null() => v.clone(),
        _ => Value::from(gi as u64),
    }
}

/// A group that passed the parallel check: the group and its counts' ids (`out.ok` of `Calibrate.parallel`).
#[derive(Clone, Debug)]
pub struct OkGroup {
    /// The group.
    pub g: Group,
    /// Its position in the groups given.
    pub gi: usize,
    /// Indexes of its members among the rows.
    pub members: Vec<usize>,
    /// Ids of the counts that matched.
    pub ids: Vec<String>,
}

/// A group left out because its counts lie in sequence on one path (`out.rejected` of `Calibrate.parallel`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejected {
    /// Group id.
    pub id: Value,
    /// Display name.
    pub label: String,
    /// Flow (veh/h) passing more than one of the group's counts on the same path, 0.1.
    #[serde(with = "crate::results::plain")]
    pub shared: f64,
    /// Ids of the group's counts.
    pub count_ids: Vec<String>,
    /// Why.
    pub reason: String,
}

/// The result of the parallel check made once on the prior run.
#[derive(Clone, Debug, Default)]
pub struct ParallelCheck {
    /// Groups that are parallel.
    pub ok: Vec<OkGroup>,
    /// Groups left out.
    pub rejected: Vec<Rejected>,
}

/// `Calibrate.parallel(res, rows, groups)`: checks that each group's counts are parallel (no path crosses two of them carrying more than max(1 pcu/h, 1% of the group's modelled flow)).
pub fn parallel(res: &Res, rows: &[SiteRow], groups: &[Group], s: &Settings) -> ParallelCheck {
    let mut out = ParallelCheck::default();
    if groups.is_empty() {
        return out;
    }
    let mut by_id: HashMap<&str, usize> = HashMap::new();
    for (k, r) in rows.iter().enumerate() {
        by_id.insert(r.c.id.as_str(), k);
    }
    struct G {
        gi: usize,
        members: Vec<usize>,
    }
    let mut gs: Vec<G> = Vec::new();
    for (gi, g) in groups.iter().enumerate() {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut members = Vec::new();
        for id in &g.count_ids {
            if seen.insert(id.as_str())
                && let Some(&k) = by_id.get(id.as_str())
            {
                members.push(k);
            }
        }
        if !members.is_empty() {
            gs.push(G { gi, members });
        }
    }
    // arc index to [[group, member row]]
    let mut at: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for x in &gs {
        for &k in &x.members {
            for &a in &rows[k].arcs {
                at.entry(a).or_default().push((x.gi, k));
            }
        }
    }
    let mut shared: HashMap<usize, f64> = HashMap::new();
    let mut flow: HashMap<usize, f64> = HashMap::new();
    for x in &gs {
        flow.insert(
            x.gi,
            x.members.iter().fold(0.0f64, |s, &k| s + rows[k].model),
        );
    }
    if let Some(paths) = res.paths.get() {
        for od in paths {
            for p in &od.list {
                // group to the set of its member rows this path crosses, in the order the groups were first met
                let mut hit: Vec<(usize, Vec<usize>)> = Vec::new();
                for a in &p.arcs {
                    if let Some(l) = at.get(a) {
                        for &(gi, k) in l {
                            match hit.iter_mut().find(|(g, _)| *g == gi) {
                                Some((_, set)) => {
                                    if !set.contains(&k) {
                                        set.push(k);
                                    }
                                }
                                None => hit.push((gi, vec![k])),
                            }
                        }
                    }
                }
                for (gi, set) in hit {
                    if set.len() > 1 {
                        *shared.entry(gi).or_insert(0.0) += p.w;
                    }
                }
            }
        }
    }
    let pf = pcu_f(s);
    for x in &gs {
        let g = &groups[x.gi];
        let sh = shared.get(&x.gi).copied().unwrap_or(0.0) / pf;
        let lim = math_max(
            SEQ_MIN_PCU,
            SEQ_SHARE * flow.get(&x.gi).copied().unwrap_or(0.0),
        );
        let ids: Vec<String> = x.members.iter().map(|&k| rows[k].c.id.clone()).collect();
        if sh > lim {
            out.rejected.push(Rejected {
                id: id_value(&g.id, x.gi),
                label: match g.label.as_deref() {
                    Some(l) if !l.is_empty() => l.to_string(),
                    _ => id_text(&g.id, x.gi),
                },
                shared: r1(sh),
                count_ids: ids,
                reason: format!(
                    "Counts in sequence: {} veh/h pass more than one of its counts on the same path, so the group total counts them twice (groups must be parallel; {}).",
                    fmt::int(sh),
                    SEQ_BASIS
                ),
            });
        } else {
            out.ok.push(OkGroup {
                g: g.clone(),
                gi: x.gi,
                members: x.members.clone(),
                ids,
            });
        }
    }
    out
}

/// A fitting unit: a group total, or a single count not in any group.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    /// Counted volume (veh/h).
    pub obs: f64,
    /// Modelled flow (veh/h).
    pub model: f64,
    /// Arc indexes it counts.
    pub arcs: Vec<usize>,
    /// Weight.
    pub w: f64,
    /// Ids of its counts.
    pub ids: Vec<String>,
    /// The group's id, for a group total.
    pub group: Option<Value>,
}

/// `Calibrate.units(rows, chk)`: fitting units from one result's sites and a parallel check made once on the prior run.
///
/// Each accepted group is one unit (its total), rejected groups are left out, other counts are units of their own.
/// Rows are matched to the check by count id, so the same check serves every pass.
pub fn units(rows: &[SiteRow], chk: Option<&ParallelCheck>) -> Vec<Unit> {
    let single = |r: &SiteRow| Unit {
        obs: r.obs,
        model: r.model,
        arcs: r.arcs.clone(),
        w: r.w,
        ids: vec![r.c.id.clone()],
        group: None,
    };
    let chk = match chk {
        Some(c) if !(c.ok.is_empty() && c.rejected.is_empty()) => c,
        _ => return rows.iter().map(single).collect(),
    };
    let mut by_id: HashMap<&str, &SiteRow> = HashMap::new();
    for r in rows {
        by_id.insert(r.c.id.as_str(), r);
    }
    let mut used: HashSet<&str> = HashSet::new();
    let mut list = Vec::new();
    let gone: HashSet<&str> = chk
        .rejected
        .iter()
        .flat_map(|r| r.count_ids.iter().map(String::as_str))
        .collect();
    for x in &chk.ok {
        let rs: Vec<&SiteRow> = x
            .ids
            .iter()
            .filter(|id| !used.contains(id.as_str()) && by_id.contains_key(id.as_str()))
            .map(|id| by_id[id.as_str()])
            .collect();
        if rs.is_empty() {
            continue;
        }
        for r in &rs {
            used.insert(r.c.id.as_str());
        }
        let mut arcs: Vec<usize> = Vec::new();
        for r in &rs {
            for &a in &r.arcs {
                if !arcs.contains(&a) {
                    arcs.push(a);
                }
            }
        }
        list.push(Unit {
            obs: rs.iter().fold(0.0f64, |s, r| s + r.obs),
            model: rs.iter().fold(0.0f64, |s, r| s + r.model),
            arcs,
            w: rs.iter().fold(0.0f64, |s, r| s + r.w) / rs.len() as f64,
            ids: rs.iter().map(|r| r.c.id.clone()).collect(),
            group: Some(id_value(&x.g.id, x.gi)),
        });
    }
    for r in rows {
        if !used.contains(r.c.id.as_str()) && !gone.contains(r.c.id.as_str()) {
            list.push(single(r));
        }
    }
    list
}
