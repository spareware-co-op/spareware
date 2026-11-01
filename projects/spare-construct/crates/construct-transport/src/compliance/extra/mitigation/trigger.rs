//! The trigger finder (compliance-extra.js section 7): steps the build-out of a scenario (5% by default), with and without each upgrade (any other
//! scenario, by default the mitigation scenario), and reports the first increment where a criterion fails at a site that passes at 0%, or where the
//! development worsens a site that already fails at 0% (an existing deficiency) by the assumed amount.

use super::super::anop::{self, MODES};
use super::super::ctx::{self, clone_sc, esc, nn, num, sum_dw, sum_gfa};
use super::{h, tstr};
use crate::compliance::data::x as xdata;
use crate::compliance::dates::now_ms;
use crate::compliance::engine;
use crate::compliance::html::{Tr, kit_table, pill_x, th, thn, tr};
use crate::compliance::jv::{Jx, V, jobj};
use crate::compliance::net;
use crate::jsnum::{math_max, math_min, math_round, truthy_number};
use crate::util::fmt;
use std::collections::HashSet;

/// `storageOf(sc, linkId, nodeId)`: the queue storage on an approach: the approach road back to the upstream junction, chained through simple nodes
/// (two roads, no control) up to 3000 m: `{ m, links }`; null for a roundabout circulating link (junction-internal) and for a missing road.
pub fn storage_of(sc: &V, link_id: &str, node_id: &str) -> V {
    let mut l: &V = sc.g("links").g(link_id);
    let mut at = node_id.to_string();
    let (mut m, mut n) = (0.0f64, 0usize);
    if !l.t() || l.g("ring").t() {
        return V::Null;
    }
    let max_chain = xdata("STD.trigger.maxChain").n();
    let mut seen: Vec<String> = Vec::new();
    while l.t() && !seen.contains(&l.g("id").s()) && m < max_chain {
        seen.push(l.g("id").s());
        m += net::len(sc, l);
        n += 1;
        let far = if l.g("a").eq_s(&at) {
            l.g("b")
        } else {
            l.g("a")
        };
        let node = sc.g("nodes").g(&far.s());
        let others: Vec<&V> = net::road_links(sc, &far.s())
            .into_iter()
            .filter(|x| !x.g("id").strict_eq(l.g("id")))
            .collect();
        let control = node.g("control");
        if !node.t() || others.len() != 1 || (control.t() && !control.eq_s("none")) {
            break;
        }
        at = far.s();
        l = others[0];
    }
    jobj! { "m" => m, "links" => n }
}

/// `los = nr.intersectionLos || nr.los` of a junction result (None when neither is there).
fn los_of(nr: &V) -> Option<V> {
    if nr.g("intersectionLos").t() {
        Some(nr.g("intersectionLos").clone())
    } else {
        nr.get("los").cloned()
    }
}

/// `LETTERS.indexOf(v)` of a loose value (-1 for anything that is not a letter grade).
fn los_index(v: &V) -> i32 {
    v.as_str()
        .and_then(|s| ["A", "B", "C", "D", "E", "F"].iter().position(|l| *l == s))
        .map_or(-1, |i| i as i32)
}

/// `failsAt(sc, res, crit, only)`: the junctions of a result that fail the criteria `{ dos, los, queue, minStorage }` (degree of saturation above the
/// limit, level of service worse than the limit, a 95th-percentile queue longer than the storage back to the upstream junction), `only` limiting the
/// junctions looked at: `[{ id, why: [text], dos, los, delay }]`.
pub fn fails_at(sc: &V, res: &V, crit: &V, only: Option<&HashSet<String>>) -> V {
    let mut out: Vec<V> = Vec::new();
    let min_st = if !crit.g("minStorage").nullish() {
        crit.g("minStorage").n()
    } else {
        xdata("STD.trigger.minStorage").n()
    };
    for (id, nr) in res.g("nodes").entries() {
        if only.is_some_and(|o| !o.contains(id)) {
            continue;
        }
        let mut why: Vec<V> = Vec::new();
        let los = los_of(nr);
        if !crit.g("dos").nullish() && nr.g("dos").n() > crit.g("dos").n() {
            why.push(V::String(format!(
                "DoS {} > {}",
                num(nr.g("dos"), 2),
                crit.g("dos").s()
            )));
        }
        if let Some(l) = los.as_ref().filter(|l| l.t()) {
            if crit.g("los").t() && los_index(l) > los_index(crit.g("los")) {
                why.push(V::String(format!(
                    "LoS {} worse than {}",
                    l.s(),
                    crit.g("los").s()
                )));
            }
        }
        if crit.g("queue").t() {
            for a in nr.g("appr").items() {
                let st = storage_of(sc, &a.g("link").s(), id);
                if st.nullish() || st.g("m").n() < min_st || !(a.g("q95").n() > st.g("m").n()) {
                    continue;
                }
                let name = if a.g("name").t() {
                    a.g("name").s()
                } else {
                    ctx::link_name(sc.g("links").g(&a.g("link").s()))
                };
                why.push(V::String(format!(
                    "95th-percentile queue {} m > {} m storage back to the upstream junction on {}",
                    fmt::int(a.g("q95").n()),
                    fmt::int(st.g("m").n()),
                    name
                )));
                break;
            }
        }
        if !why.is_empty() {
            let mut o = jobj! { "id" => id, "why" => why };
            if let Some(v) = nr.get("dos") {
                o.set("dos", v.clone());
            }
            if let Some(l) = los {
                o.set("los", l);
            }
            if let Some(v) = nr.get("delay") {
                o.set("delay", v.clone());
            }
            out.push(o);
        }
    }
    V::Array(out)
}

/// `anopFitOf(scB, rB, scJ, rJ, c)`: the ANOP fit of one corridor for a project result against the do-minimum result ("High", "Moderate", "Low"; None
/// when nothing could be assessed).
pub fn anop_fit_of(sc_b: &V, r_b: &V, sc_j: &V, r_j: &V, c: &V) -> Option<String> {
    let lb = anop::anop_los(sc_b, r_b, c);
    let lj = anop::anop_los(sc_j, r_j, c);
    let lu = if c.g("landUse").t() {
        c.g("landUse").s()
    } else {
        "other".to_string()
    };
    let modes: Vec<V> = MODES
        .iter()
        .map(|m| {
            let hs = anop::hier_of(sc_j, c, m);
            let pf = anop::anop_pref(m, &hs.g("h").sn(), &lu);
            jobj! { "base" => lb.g(m).g("los"), "pref" => if pf.g("pref").t() { pf.g("pref").clone() } else { V::Null }, "proj" => lj.g(m).g("los") }
        })
        .collect();
    anop::anop_fit(&modes)
}

/// `ordinal(n)`: a number with its English ordinal ending ("1st", "22nd", "1,013th").
pub fn ordinal(n: f64) -> String {
    let s = ["th", "st", "nd", "rd"];
    let v = n % 100.0;
    let idx = |x: f64| -> Option<usize> {
        if x >= 0.0 && x < 4.0 && x == x.trunc() {
            Some(x as usize)
        } else {
            None
        }
    };
    fmt::int(n) + s[idx((v - 20.0) % 10.0).or_else(|| idx(v)).unwrap_or(0)]
}

/* =====================================================================================
The finder
===================================================================================== */

/// One scenario the finder steps: the scenario with no upgrade, then each scenario tested as an upgrade.
struct Variant<'a> {
    id: V,
    label: String,
    sc: &'a V,
    upgrade: bool,
}

/// A site that fails at 0% build-out (an existing deficiency).
struct Existing {
    id: V,
    key: String,
    name: String,
    period: V,
    why: String,
    at0: V,
    worsened_at: Option<f64>,
}

/// One evaluated build-out step: the scenario clone, the new failures, every junction's metrics and the ANOP fits.
struct Step {
    c: V,
    fails: Vec<V>,
    metrics: V,
    fits: V,
}

/// What a step needs to be evaluated.
struct Run<'a> {
    p: &'a V,
    periods: &'a [V],
    crit: &'a V,
    only: Option<&'a HashSet<String>>,
    opts: &'a V,
    base: f64,
    anop_on: bool,
    pr_base: &'a V,
    cors: &'a V,
}

impl Run<'_> {
    /// `evalStep(v, k)`: one build-out step: failing sites, every junction's metrics and (optionally) the ANOP fits.
    fn eval_step(&self, v_sc: &V, k: f64) -> Step {
        let c = clone_sc(v_sc, &[("buildout", V::Number(self.base * k / 100.0))]);
        let (mut fails, mut metrics, mut fits) = (Vec::new(), V::object(), V::object());
        for per in self.periods {
            let ps = per.s();
            let mut ao = jobj! { "period" => per };
            for key in ["gapTol", "maxIter"] {
                if self.opts.has(key) {
                    ao.set(key, self.opts.g(key).clone());
                }
            }
            let r = engine::assign(self.p, &c, None, &ao);
            for f in fails_at(&c, &r, self.crit, self.only).items() {
                let mut o = f.clone();
                o.set("period", per.clone());
                o.set("key", V::String(format!("{}|{}", f.g("id").s(), ps)));
                o.set("name", V::String(ctx::node_name(&c, &f.g("id").s())));
                fails.push(o);
            }
            for (id, nr) in r.g("nodes").entries() {
                let mut m = V::object();
                for k in ["dos", "delay"] {
                    if let Some(v) = nr.get(k) {
                        m.set(k, v.clone());
                    }
                }
                if let Some(l) = los_of(nr) {
                    m.set("los", l);
                }
                metrics.set(format!("{id}|{ps}"), m);
            }
            let rb = if self.anop_on {
                ctx::res_of(self.pr_base, &ps)
            } else {
                None
            };
            if let Some(rb) = rb {
                for cor in self.cors.items() {
                    // `try { ... } catch { }`: a corridor the ANOP assessment cannot read is left out
                    let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        anop_fit_of(self.pr_base, rb, &c, &r, cor)
                    }));
                    if let Ok(fit) = attempt {
                        fits.set(
                            format!("{}|{}", cor.g("id").s(), ps),
                            jobj! { "fit" => fit, "name" => cor.g("name"), "period" => per },
                        );
                    }
                }
            }
        }
        Step {
            c,
            fails,
            metrics,
            fits,
        }
    }
}

/// `why.join('; ')` of a failure's reasons.
fn join_why(why: &V) -> String {
    why.items()
        .iter()
        .map(|w| w.sn())
        .collect::<Vec<_>>()
        .join("; ")
}

/// `triggerFinder(P, opts)`: steps the build-out of a scenario for the periods (default AM and PM) and reports, for the scenario without an upgrade
/// and with each upgrade, the first increment where a criterion fails (`opts`: `{ scId, upgrades [scenario ids], step, periods, dos, los, queue,
/// minStorage, anop, worsenDos, worsenDelay, nodes [ids], gapTol, maxIter, save }`). The result is kept in the project's record (`complianceExtra.triggers`)
/// unless `save` is false. `{ at, scId, scName, step, periods, criteria, basis, rows, ms, label, copy }`.
pub fn trigger_finder(p: &mut V, opts: &V) -> V {
    let res = find(p, opts);
    // `data(P)` also fills the record's defaults in, saved or not
    let record = ctx::data_mut(p);
    if !opts.g("save").strict_eq(&V::Bool(false)) {
        record.set("triggers", res.clone());
    }
    res
}

fn find(p: &V, opts: &V) -> V {
    let pr = ctx::pair(p);
    let sc: &V = ctx::sc_by_id(p, opts.g("scId")).unwrap_or(pr.opt);
    let t_ = xdata("STD.trigger");
    let step = {
        let s = opts.g("step").n();
        math_max(
            1.0,
            math_min(
                50.0,
                if truthy_number(s) {
                    s
                } else {
                    t_.g("step").n()
                },
            ),
        )
    };
    let pick = |k: &str, d: &V| {
        if opts.has(k) {
            opts.g(k).clone()
        } else {
            d.clone()
        }
    };
    let num_or = |k: &str, d: &V| {
        V::Number(if nn(opts.g(k)).is_some() {
            opts.g(k).n()
        } else {
            d.n()
        })
    };
    let crit = jobj! {
        "dos" => pick("dos", t_.g("dos")), "los" => pick("los", t_.g("los")),
        "queue" => if opts.has("queue") { opts.g("queue").t() } else { t_.g("queue").t() },
        "minStorage" => num_or("minStorage", t_.g("minStorage")),
        "anop" => if opts.has("anop") { opts.g("anop").t() } else { t_.g("anop").t() },
        "worsen" => jobj! { "dos" => num_or("worsenDos", t_.g("worsen").g("dos")), "delay" => num_or("worsenDelay", t_.g("worsen").g("delay")) },
    };
    let periods: Vec<V> = if opts.g("periods").t() && opts.g("periods").jlen() > 0 {
        opts.g("periods").items().to_vec()
    } else {
        let have = ctx::periods_of(sc);
        if !have.is_empty() {
            have.into_iter()
                .filter(|x| *x != "IP")
                .map(|x| V::String(x.to_string()))
                .collect()
        } else {
            vec![V::String("AM".into()), V::String("PM".into())]
        }
    };
    let up_ids: Vec<V> = if opts.g("upgrades").t() {
        opts.g("upgrades").items().to_vec()
    } else {
        match pr.mit.filter(|m| !std::ptr::eq(*m, sc)) {
            Some(m) => vec![m.g("id").clone()],
            None => Vec::new(),
        }
    };
    let mut ups: Vec<&V> = Vec::new();
    for id in &up_ids {
        if let Some(s) = ctx::sc_by_id(p, id) {
            if !std::ptr::eq(s, sc) && !ups.iter().any(|u| std::ptr::eq(*u, s)) {
                ups.push(s);
            }
        }
    }
    let only: Option<HashSet<String>> = if opts.g("nodes").t() && opts.g("nodes").jlen() > 0 {
        Some(opts.g("nodes").items().iter().map(|n| n.s()).collect())
    } else {
        None
    };
    // the ANOP criterion compares each step with the do-minimum, so it needs a do-minimum other than the stepped scenario
    let anop_on = crit.g("anop").t() && pr.base.t() && !std::ptr::eq(pr.base, sc);
    let cors = if anop_on {
        anop::anop_corridors(p)
    } else {
        V::Array(Vec::new())
    };
    let mut variants: Vec<Variant> = vec![Variant {
        id: sc.g("id").clone(),
        label: format!("{} (without upgrade)", tstr(sc, "name")),
        sc,
        upgrade: false,
    }];
    variants.extend(ups.iter().map(|u| Variant {
        id: u.g("id").clone(),
        label: format!("{} (with upgrade)", tstr(u, "name")),
        sc: u,
        upgrade: true,
    }));
    let mut steps: Vec<f64> = Vec::new();
    let mut k = 0.0f64;
    while k <= 100.0 + 1e-9 {
        steps.push(math_round(k * 100.0) / 100.0);
        k += step;
    }
    if steps.last() != Some(&100.0) {
        steps.push(100.0);
    }
    let base = if sc.g("buildout").entered() {
        sc.g("buildout").n()
    } else {
        100.0
    };
    let run = Run {
        p,
        periods: &periods,
        crit: &crit,
        only: only.as_ref(),
        opts,
        base,
        anop_on,
        pr_base: pr.base,
        cors: &cors,
    };
    let t0 = now_ms();
    let fit_rank = xdata("FIT_RANK");
    let mut rows: Vec<V> = Vec::new();
    for v in &variants {
        let mut existing: Option<Vec<Existing>> = None;
        let mut fit0 = V::Null;
        let mut first = V::Null;
        let mut last_pass: Option<(f64, f64, f64)> = None;
        let mut last: Option<(f64, V)> = None;
        let mut trace: Vec<V> = Vec::new();
        for &k in &steps {
            let s = run.eval_step(v.sc, k);
            last = Some((k, s.metrics.clone()));
            let (dw, gfa) = (sum_dw(&s.c), sum_gfa(&s.c));
            if existing.is_none() {
                let mut map: Vec<Existing> = Vec::new();
                for f in &s.fails {
                    let e = Existing {
                        id: f.g("id").clone(),
                        key: f.g("key").s(),
                        name: f.g("name").s(),
                        period: f.g("period").clone(),
                        why: join_why(f.g("why")),
                        at0: s.metrics.g(&f.g("key").s()).clone(),
                        worsened_at: None,
                    };
                    // a Map keeps the first place of a key and the last value
                    match map.iter_mut().find(|x| x.key == e.key) {
                        Some(slot) => *slot = e,
                        None => map.push(e),
                    }
                }
                existing = Some(map);
                fit0 = s.fits.clone();
            }
            let ex = existing.as_mut().expect("set above");
            let mut new_f: Vec<V> = s
                .fails
                .iter()
                .filter(|f| !ex.iter().any(|e| e.key == f.g("key").s()))
                .cloned()
                .collect();
            // an existing deficiency counts once the development worsens it (change in DoS or in delay, Spare Construct assumption)
            let (w_dos, w_delay) = (
                crit.g("worsen").g("dos").n(),
                crit.g("worsen").g("delay").n(),
            );
            for e in ex.iter_mut() {
                let m = s.metrics.g(&e.key);
                if !m.t() || !e.at0.t() {
                    continue;
                }
                if m.g("dos").n() - e.at0.g("dos").n() >= w_dos
                    || m.g("delay").n() - e.at0.g("delay").n() >= w_delay
                {
                    if e.worsened_at.is_none() {
                        e.worsened_at = Some(k);
                    }
                    let mut o = jobj! { "id" => &e.id, "key" => e.key.as_str(), "name" => e.name.as_str(), "period" => &e.period, "worsened" => true };
                    if let Some(x) = m.get("dos") {
                        o.set("dos", x.clone());
                    }
                    if let Some(x) = m.get("los") {
                        o.set("los", x.clone());
                    }
                    o.set(
                        "why",
                        V::Array(vec![V::String(format!(
                            "existing deficiency worsened by the development: DoS {} \u{2192} {}, delay {} \u{2192} {} s",
                            num(e.at0.g("dos"), 2),
                            num(m.g("dos"), 2),
                            num(e.at0.g("delay"), 0),
                            num(m.g("delay"), 0)
                        ))]),
                    );
                    new_f.push(o);
                }
            }
            // the ANOP fit drops a class against its 0% value
            if anop_on {
                for (key, f) in s.fits.entries() {
                    let f0 = fit0.g(key);
                    if f0.t() && f0.g("fit").t() && f.g("fit").t() {
                        let (r1, r0) = (fit_rank.g(&f.g("fit").s()), fit_rank.g(&f0.g("fit").s()));
                        if r1.is_num() && r0.is_num() && r1.n() < r0.n() {
                            new_f.push(jobj! { "id" => format!("anop:{key}"), "key" => format!("anop:{key}"), "name" => format!("{} (ANOP corridor)", f.g("name").s()), "period" => f.g("period"), "anop" => true,
                                "why" => vec![format!("ANOP fit drops from {} to {}", f0.g("fit").s(), f.g("fit").s())] });
                        }
                    }
                }
            }
            trace.push(jobj! { "pct" => k, "dwellings" => dw, "gfa" => gfa, "fails" => new_f.len(), "keys" => new_f.iter().map(|f| f.g("key").clone()).collect::<Vec<V>>(),
                "worst" => new_f.first().cloned().unwrap_or(V::Null) });
            if let Some(w) = new_f.first() {
                first = jobj! { "pct" => k, "dwellings" => dw, "gfa" => gfa, "before" => last_pass.map_or(0.0, |l| l.1).floor() + 1.0, "sites" => new_f.clone(), "node" => w.g("id"), "key" => w.g("key"),
                "name" => w.g("name"), "period" => w.g("period"), "why" => join_why(w.g("why")), "worsened" => w.g("worsened").t() };
                break;
            }
            last_pass = Some((k, dw, gfa));
        }
        // existing deficiencies with their 0% and full build-out values (one more run per period when the search stopped early)
        let ex = existing.as_ref().expect("a build-out step ran");
        if !ex.is_empty() && last.as_ref().is_some_and(|l| l.0 != 100.0) {
            last = Some((100.0, run.eval_step(v.sc, 100.0).metrics));
        }
        let existing_v: Vec<V> = ex
            .iter()
            .map(|e| {
                let at100 = last.as_ref().map_or(V::Null, |l| if l.1.g(&e.key).t() { l.1.g(&e.key).clone() } else { V::Null });
                jobj! { "id" => &e.id, "name" => e.name.as_str(), "period" => &e.period, "why" => e.why.as_str(), "at0" => if e.at0.t() { e.at0.clone() } else { V::Null }, "at100" => at100,
                    "worsenedAt" => e.worsened_at }
            })
            .collect();
        let text = if first.t() {
            format!(
                "First fails at {}% build-out ({} dwellings): {}, {} peak ({}).",
                first.g("pct").s(),
                fmt::int(first.g("dwellings").n()),
                first.g("name").s(),
                first.g("period").s(),
                first.g("why").s()
            )
        } else {
            format!(
                "No assessed site fails up to 100% build-out ({} dwellings).",
                fmt::int(trace.last().map_or(0.0, |t| t.g("dwellings").n()))
            )
        };
        let last_pass_v = match last_pass {
            Some((pct, dwellings, gfa)) => {
                jobj! { "pct" => pct, "dwellings" => dwellings, "gfa" => gfa }
            }
            None => V::Null,
        };
        rows.push(jobj! { "variant" => &v.id, "label" => v.label.as_str(), "upgrade" => v.upgrade, "first" => first, "lastPass" => last_pass_v, "existing" => existing_v, "trace" => trace, "text" => text });
    }
    // Resolved Y/N (mitigation-staging Table M2): does each upgrade clear the trigger of the variant without it?
    let w0 = rows.first().map(|r| r.g("first").clone()).filter(|f| f.t());
    for r in rows.iter_mut().skip(1) {
        let resolved = match &w0 {
            None => {
                jobj! { "state" => V::Null, "text" => "Nothing to resolve: no trigger without the upgrade" }
            }
            Some(w0) => {
                let t = r
                    .g("trace")
                    .items()
                    .iter()
                    .find(|x| x.g("pct").n() == w0.g("pct").n());
                let f = r.g("first");
                let (state, text) = if !f.t() {
                    (
                        "Y",
                        format!(
                            "Yes: no assessed site fails up to 100% build-out with {}",
                            r.g("label").s()
                        ),
                    )
                } else if f.g("pct").n() > w0.g("pct").n() {
                    (
                        "Y",
                        format!(
                            "Yes: the first failure moves from {}% to {}% build-out",
                            w0.g("pct").s(),
                            f.g("pct").s()
                        ),
                    )
                } else if f.g("pct").n() < w0.g("pct").n() {
                    (
                        "N",
                        format!(
                            "No: fails earlier, at {}% ({}, {})",
                            f.g("pct").s(),
                            f.g("name").s(),
                            f.g("period").s()
                        ),
                    )
                } else if t
                    .is_some_and(|t| !t.g("keys").items().iter().any(|k| k.strict_eq(w0.g("key"))))
                {
                    (
                        "Partly",
                        format!(
                            "Partly: {} is resolved, but {} fails at the same build-out ({}%)",
                            w0.g("name").s(),
                            f.g("name").s(),
                            f.g("pct").s()
                        ),
                    )
                } else {
                    (
                        "N",
                        format!(
                            "No: {} still fails at {}% build-out",
                            w0.g("name").s(),
                            w0.g("pct").s()
                        ),
                    )
                };
                jobj! { "state" => state, "text" => text }
            }
        };
        r.set("resolved", resolved);
    }
    let crit_dos = crit.g("dos").s();
    let queue_part = if crit.g("queue").t() {
        format!(
            ", or a 95th-percentile queue longer than the storage back to the upstream junction (storage under {} m is not tested: usually junction-internal coding)",
            crit.g("minStorage").s()
        )
    } else {
        String::new()
    };
    let anop_part = if anop_on {
        "; or an ANOP corridor fit drops a class"
    } else if crit.g("anop").t() {
        "; the ANOP criterion needs a do-minimum other than the stepped scenario"
    } else {
        ""
    };
    let basis = format!(
        "{} criteria: worst DoS above {}, LoS worse than {}{}; an existing deficiency counts when the development raises its DoS by {} or its delay by {} s{}",
        ctx::assumption(),
        crit_dos,
        crit.g("los").s(),
        queue_part,
        crit.g("worsen").g("dos").s(),
        crit.g("worsen").g("delay").s(),
        anop_part
    );
    let mut res = jobj! { "at" => crate::edit::ids::iso_string(now_ms()) };
    if sc.has("id") {
        res.set("scId", sc.g("id").clone());
    }
    if sc.has("name") {
        res.set("scName", sc.g("name").clone());
    }
    res.set("step", V::Number(step));
    res.set("periods", V::Array(periods.clone()));
    res.set("criteria", crit.clone());
    res.set("basis", V::String(basis));
    res.set("rows", V::Array(rows.clone()));
    res.set("ms", V::Number(now_ms() - t0));
    res.set("label", xdata("STD.labels.junction").clone());
    let w = rows.first().map(|r| r.g("first")).filter(|f| f.t());
    let cx = ctx::data(p);
    let m = cx
        .g("mitigation")
        .items()
        .iter()
        .find(|x| x.g("measure").t());
    res.set(
        "copy",
        V::String(match w {
            Some(w) => format!(
                "Prior to occupation of the {} dwelling, {} (parameter only: the condition wording is for the planner).",
                ordinal(w.g("before").n()),
                m.map_or("<measure>".to_string(), |m| m.g("measure").s())
            ),
            None => String::new(),
        }),
    );
    res
}

/* =====================================================================================
Tables
===================================================================================== */

/// `mvalH(x)`: a junction's values at a build-out as table text ("DoS 0.95, 12 s, LoS D"), an en dash when there are none.
fn mval_h(x: &V) -> String {
    if x.t() {
        format!(
            "DoS {}, {} s, LoS {}",
            num(x.g("dos"), 2),
            num(x.g("delay"), 0),
            esc(&if x.g("los").t() {
                x.g("los").clone()
            } else {
                V::String("\u{2013}".into())
            })
        )
    } else {
        "\u{2013}".to_string()
    }
}

/// `triggerTables(t)`: the trigger finder's results as report tables: the variants with their first failing build-out, then the existing deficiencies.
pub fn trigger_tables(t: &V) -> String {
    let head = [
        th("Variant"),
        th("First failing build-out"),
        thn("Dwellings"),
        th("Site and reason"),
        th("Resolved by the upgrade"),
    ];
    let rows: Vec<Tr> = t
        .g("rows")
        .items()
        .iter()
        .map(|r| {
            let f = r.g("first");
            tr([
                esc(r.g("label")),
                if f.t() {
                    format!("{}%", f.g("pct").s())
                } else {
                    "None".to_string()
                },
                if f.t() {
                    fmt::int(f.g("dwellings").n())
                } else {
                    "\u{2013}".to_string()
                },
                if f.t() {
                    format!(
                        "{} {}: {}",
                        esc(f.g("name")),
                        esc(f.g("period")),
                        esc(f.g("why"))
                    )
                } else {
                    esc(r.g("text"))
                },
                if r.g("resolved").t() {
                    esc(r.g("resolved").g("text"))
                } else if r.g("upgrade").t() {
                    "\u{2013}".to_string()
                } else {
                    String::new()
                },
            ])
        })
        .collect();
    let main = kit_table(&head, &rows, true, "", "");
    let mut ex: Vec<Tr> = Vec::new();
    for r in t.g("rows").items() {
        for e in r.g("existing").items() {
            ex.push(tr([
                esc(r.g("label")),
                esc(e.g("name")),
                esc(e.g("period")),
                mval_h(e.g("at0")),
                mval_h(e.g("at100")),
                if !e.g("worsenedAt").nullish() {
                    pill_x(&format!("worsened from {}%", e.g("worsenedAt").s()), "bad")
                } else {
                    "not beyond the threshold".to_string()
                },
            ]));
        }
    }
    if ex.is_empty() {
        return main;
    }
    let head2 = [
        "Variant",
        "Junction",
        "Period",
        "At 0%",
        "At 100%",
        "Development effect",
    ]
    .map(th);
    format!(
        "{}{}{}",
        main,
        h("Existing deficiencies (fail at 0% build-out)"),
        kit_table(&head2, &ex, true, "", "")
    )
}
