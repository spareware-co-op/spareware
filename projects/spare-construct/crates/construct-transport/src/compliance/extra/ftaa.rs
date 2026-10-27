//! The Fast-track package (compliance-extra.js section 8, invitees to package files), its package files, JSON bundle and report section.

use super::ctx::{
    add_months, add_working_days, assumption, assumptions, context, data, disc, disclaimer, esc, is_iso, is_sh, link_name, node_name, num, pair, periods_of, res_of,
    stamp, sum_dw, today,
};
use super::options::util::{Put, cells, label, or_s, tbl, xs};
use super::shared::{econ_of, kpi_p, neg_pp, per_period, pp_text, route_p};
use super::{mitigation, parking, precinct};
use crate::compliance::data::{c as cdata, x as xdata};
use crate::compliance::html::{ReportCtx, rt};
use crate::compliance::jv::{Jx, V, csv, jarr, jobj, strs};
use crate::compliance::{dates, engine, net};
use crate::graph::js_sort_by;
use crate::jsnum::to_fixed;
use crate::util::fmt;

/// `invitees(P)`: who the Fast-track panel invites to comment (s53): the council, NZTA when a state highway is near the site or the author
/// marks a designation, and KiwiRail when the author marks one: `[{ who, basis, auto }]`.
pub fn invitees(p: &V) -> V {
    let cx = data(p);
    let cx = cx.g("ftaa");
    let sc = super::ctx::active_sc(p);
    let a = assumptions(p);
    let buffer = a.g("shBuffer").n();
    let mut out: Vec<V> = vec![jobj! { "who" => "Auckland Council", "basis" => "FTAA s53(2) (relevant local authority)", "auto" => true }];
    // the nearest state highway to a development zone
    let mut near: Option<(f64, &V, &V)> = None;
    let zs = super::ctx::dev_zones(sc);
    for l in sc.g("links").vals() {
        if l.g("status").eq_s("closed") || !is_sh(p, l) {
            continue;
        }
        // point-to-segment distance, so a sparsely digitised motorway is still found
        for z in &zs {
            let d = net::project(sc, l, [z.g("lat").n(), z.g("lng").n()]).map_or(f64::NAN, |x| x.d);
            if d <= buffer && near.is_none_or(|n| d < n.0) {
                near = Some((d, l, z));
            }
        }
    }
    let marked = cx.g("nztaDesignation").t();
    if near.is_some() || marked {
        let why = match (marked, near) {
            (false, Some((d, l, z))) => format!(
                "{} is {} m from {}; within {} m ({})",
                link_name(l), fmt::int(d), if z.g("name").t() { z.g("name").s() } else { "a development zone".to_string() }, a.g("shBuffer").s(), assumption()
            ),
            _ => "marked by the author".to_string(),
        };
        out.push(jobj! { "who" => "NZTA", "basis" => format!("FTAA s53(2)(l): designation on or adjacent to the site ({why})"), "auto" => !marked });
    }
    if cx.g("kiwirailDesignation").t() {
        out.push(jobj! { "who" => "KiwiRail", "basis" => "FTAA s53(2)(l): rail designation on or adjacent to the site (marked by the author)", "auto" => false });
    }
    V::Array(out)
}

/// `ftaaTimeline(P)`: the Fast-track dates: the comment clock from the invitation (s54, s55(2)), the draft conditions clock (s67) and the lapse and
/// development contributions dates from the approval: `[{ step, date, rule, basis? }]`.
pub fn ftaa_timeline(p: &V) -> V {
    let cx = data(p);
    let f = cx.g("ftaa");
    let s = xdata("STD.ftaa");
    let (s54, s55, s67, lapse, dc) = (s.g("s54").n(), s.g("s55").n(), s.g("s67").s(), s.g("lapseYears").n(), s.g("dcMonths").n());
    let mut rows: Vec<V> = Vec::new();
    if is_iso(f.g("invitation")) {
        let d54 = add_working_days(f.g("invitation"), s54, "ftaa");
        let d55 = if d54.g("date").t() { add_working_days(d54.g("date"), s55, "ftaa") } else { jobj! { "date" => V::Null } };
        rows.push(jobj! { "step" => "Invitation to comment sent (s53)", "date" => f.g("invitation"), "rule" => "FTAA s53" });
        rows.push(jobj! { "step" => "Comments due", "date" => d54.g("date"), "rule" => format!("FTAA s54: {} working days", s.g("s54").s()), "basis" => d54.g("basis") });
        let mut r = jobj! { "step" => "Applicant response to comments due", "date" => d55.g("date"), "rule" => format!("FTAA s55(2): {} working days", s.g("s55").s()) };
        if let Some(b) = d55.get("basis") {
            r.set("basis", b.clone());
        }
        rows.push(r);
    } else {
        rows.push(jobj! { "step" => "Invitation to comment", "date" => V::Null, "rule" => "FTAA s53-55", "basis" => "Enter the invitation date to work out the s54 and s55(2) dates." });
    }
    rows.push(jobj! { "step" => "Comments on draft conditions", "date" => V::Null, "rule" => format!("FTAA s67: no more than {s67} working days"), "basis" => "Counted from when the panel issues draft conditions." });
    if is_iso(f.g("approval")) {
        rows.push(jobj! {
            "step" => "Approval lapses (unless the panel sets another period)", "date" => add_months(f.g("approval"), 12.0 * lapse),
            "rule" => format!("FTAA Sch 5 cl 26: {} years by default", s.g("lapseYears").s()),
        });
        rows.push(jobj! {
            "step" => "Development contributions policy window ends", "date" => add_months(f.g("approval"), dc),
            "rule" => format!("LGA s102A(3): {} months from approval", s.g("dcMonths").s()),
        });
    }
    V::Array(rows)
}

/// The short wording of the benefit-cost ratio's status (`D4short`).
fn d4_short() -> String {
    xs("D4short")
}

/// `MATRIX_RESULT[result]`: the wording of a result the author recorded for an objective or policy, None when it is not one of them.
fn matrix_result(result: &V) -> Option<String> {
    let t = xdata("MATRIX_RESULT");
    let k = result.s();
    if t.has(&k) && t.g(&k).t() { Some(t.g(&k).s()) } else { None }
}

/// `ftaaPackage(P)`: the Fast-track package: `{ matrix, aee, conditions, s85, s84a, invitees, timeline, reviewer, notes, conditionsLabel, ftaa }`.
pub fn ftaa_package(p: &V) -> V {
    let c = context(p);
    let pr = pair(p);
    let cx = data(p);
    let (opt, base) = (pr.opt, pr.base);
    let after = pr.mit.unwrap_or(opt);
    let ps: Vec<&str> = periods_of(opt).into_iter().filter(|x| res_of(base, x).is_some()).collect();
    let mut matrix: Vec<V> = Vec::new();
    let e = crate::compliance::e27(p);
    let result = if e.g("redirect").t() {
        "Not applicable (precinct)".to_string()
    } else if e.g("exempt").t() {
        format!("Not applicable (exempt, {})", if e.g("exempt").g("rule").t() { e.g("exempt").g("rule").s() } else { "E27.6.1(2)".to_string() })
    } else if e.g("tier").t() {
        "Infringes (restricted discretionary)".to_string()
    } else {
        "Complies".to_string()
    };
    matrix.push(jobj! { "rule" => "AUP E27.6.1 trip generation", "req" => "Table E27.6.1.1 thresholds", "result" => result, "evidence" => "Compliance tab \u{203a} E27 screening", "auto" => true });
    let pk = parking::parking(p);
    if pk.g("rows").jlen() > 0 {
        matrix.push(jobj! {
            "rule" => "AUP E27.6.2 parking, cycle and loading", "req" => "Tables E27.6.2.3-E27.6.2.7", "result" => if pk.g("allComply").t() { "Complies" } else { "Infringes" },
            "evidence" => "Compliance+ \u{203a} Parking (Table P1)",
        });
    }
    let pc = precinct::precinct_check(p, None);
    if pc.t() {
        matrix.push(jobj! {
            "rule" => format!("AUP {} staging", pc.g("precinct").g("rule").s()), "req" => pc.g("precinct").g("table"),
            "result" => if pc.g("breach").t() { format!("Infringes ({})", pc.g("status").s()) } else { "Complies".to_string() }, "evidence" => "Compliance+ \u{203a} Precinct",
        });
    }
    // objectives and policies need the author's judgment: 'To assess (author)' until a result is recorded (data.ftaa.matrix)
    let recorded = if cx.g("ftaa").g("matrix").t() { cx.g("ftaa").g("matrix").clone() } else { V::object() };
    for pol in cdata("POLICY").items() {
        let r0 = recorded.g(&pol.g("ref").s());
        let r = if r0.t() { r0.clone() } else { V::object() };
        let mut ev: Vec<String> = pol.g("evidence").items().iter().filter(|x| x.t()).map(|x| x.s()).collect();
        if r.g("note").t() {
            ev.push(r.g("note").s());
        }
        matrix.push(jobj! {
            "rule" => format!("AUP {}", pol.g("ref").s()), "ref" => pol.g("ref"), "req" => pol.g("text"),
            "result" => matrix_result(r.g("result")).unwrap_or_else(|| "To assess (author)".to_string()), "authorNote" => or_s(r.g("note"), ""),
            "evidence" => ev.join("; "), "author" => true,
        });
    }
    if matrix.is_empty() {
        matrix.push(jobj! { "rule" => "AUP E27 rules and standards", "req" => "Set the site and pathway in the Compliance tab", "result" => "Not assessed", "evidence" => fmt::DASH });
    }
    let mut aee: Vec<V> = [
        ("Description of the activity and its transport components", "FTAA Sch 5 cl 6(1)(a)"),
        ("Assessment of actual and potential transport effects", "FTAA Sch 5 cl 6-7"),
        ("Mitigation and monitoring", "FTAA Sch 5 cl 6(1)"),
        ("Persons affected and consultation", "FTAA Sch 5 cl 6(1)"),
    ]
    .iter()
    .map(|(t, r)| jobj! { "title" => *t, "ref" => *r })
    .collect();
    if c.g("pathway").eq_s("ftaa-nor") {
        aee.push(jobj! { "title" => "Alternatives", "ref" => "FTAA Sch 5 cl 12(1)(g)" });
        aee.push(jobj! { "title" => "Reasonable necessity", "ref" => "FTAA Sch 5 cl 12(1)(h)" });
    }
    let dv = neg_pp(&per_period(base, opt, &|s, pd| kpi_p("vht", s, pd), &ps, 1, ""));
    let ps_after: Vec<&str> = periods_of(after).into_iter().filter(|x| res_of(base, x).is_some()).collect();
    let d_pt = per_period(base, after, &|s, pd| route_p(s, pd), &ps_after, 1, "");
    let ec = econ_of(p, base, opt);
    let mut hot: Vec<String> = Vec::new();
    if let Some(risk) = engine::link_risk(p, base) {
        let mut ents: Vec<(&str, &V)> = risk.entries().into_iter().filter(|(_, r)| r.g("dsi").n() > 0.0).collect();
        js_sort_by(&mut ents, |a, b| b.1.g("dsi").n() - a.1.g("dsi").n());
        hot = ents.iter().take(3).map(|(id, r)| format!("{} ({} DSI)", link_name(base.g("links").g(id)), r.g("dsi").s())).collect();
    }
    let mut worst: Vec<(String, &str, &V)> = Vec::new();
    for &pd in &periods_of(after) {
        for (id, nr) in res_of(after, pd).map_or_else(Vec::new, |r| r.g("nodes").entries()) {
            worst.push((id.to_string(), pd, nr));
        }
    }
    js_sort_by(&mut worst, |a, b| b.2.g("dos").n() - a.2.g("dos").n());
    worst.truncate(3);
    let junction = label("junction");
    let bcr = if ec.t() && !ec.g("mismatch").t() && !ec.g("bcr").nullish() {
        V::String(format!("{} ({})", if ec.g("bcrText").t() { ec.g("bcrText").s() } else { num(ec.g("bcr"), 1) }, d4_short()))
    } else if ec.t() && ec.g("mismatch").t() {
        ec.g("note").clone()
    } else {
        V::String("Not available".into())
    };
    let ftaa_cx = cx.g("ftaa");
    let benefits = vec![
        jarr![
            "Travel time saved",
            if !ps.is_empty() { format!("{} ({} \u{2192} {})", pp_text(&dv, 1, "veh\u{b7}h per modelled peak hour"), base.g("name").s(), opt.g("name").s()) } else { "Run the do-minimum and the option".to_string() }
        ],
        jarr!["Reliability", or_s(ftaa_cx.g("reliability"), "Not modelled: enter from observed data")],
        jarr!["Jobs reachable", or_s(ftaa_cx.g("jobs"), "Enter from the accessibility analysis (policy alignment table)")],
        jarr!["Houses enabled", format!("{} dwellings in {}", fmt::int(sum_dw(opt)), opt.g("name").s())],
        jarr!["DSI change", "Not modelled in Spare Construct: enter from the safety assessment"],
        jarr!["Indicative BCR", bcr],
    ];
    let pt_min = {
        let mut by = V::object();
        for (pd, v) in d_pt.g("by").entries() {
            by.put(pd, v.n() / 60.0);
        }
        jobj! { "by" => by }
    };
    let impacts = vec![
        jarr![
            "Worst junctions after mitigation",
            if !worst.is_empty() {
                format!(
                    "{} ({junction})",
                    worst
                        .iter()
                        .map(|(id, pd, nr)| format!(
                            "{} {} DoS {}, LoS {}",
                            node_name(after, id), pd, num(nr.g("dos"), 2),
                            if nr.g("intersectionLos").t() { nr.g("intersectionLos").s() } else { super::options::util::fld_s(nr, "los") }
                        ))
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            } else {
                "Not run".to_string()
            }
        ],
        jarr!["DSI hotspots (CAS)", if !hot.is_empty() { hot.join("; ") } else { "CAS not loaded or no DSI crashes".to_string() }],
        jarr![
            "Public transport delay",
            if !d_pt.g("by").keys().is_empty() { pp_text(&pt_min, 1, "min per modelled peak hour, summed over the modelled bus routes") } else { fmt::DASH.to_string() }
        ],
    ];
    let s85 = jobj! {
        "benefits" => V::Array(benefits), "impacts" => V::Array(impacts),
        "note" => format!("FTAA s85(3): regional or national benefits are weighed against residual adverse impacts after mitigation, so both are shown. Junction results: {junction}."),
        "junctionLabel" => junction.as_str(),
    };
    let mut s84a: Vec<V> = Vec::new();
    let trig_rows = if cx.g("triggers").t() { cx.g("triggers").g("rows") } else { &V::Null };
    for r in trig_rows.items() {
        let first = r.g("first");
        s84a.push(jobj! {
            "stage" => r.g("label"),
            "trigger" => if first.t() { format!("Before {} dwellings ({}% build-out)", fmt::int(first.g("before").n()), first.g("pct").s()) } else { "No trigger up to full build-out".to_string() },
            "measure" => if r.g("upgrade").t() { "With the upgrade" } else { "Without the upgrade" },
        });
    }
    for m in mitigation::mitigation_table(p).items() {
        if m.g("trigger").t() {
            s84a.push(jobj! { "stage" => m.g("ref"), "trigger" => m.g("trigger"), "measure" => m.g("measure") });
        }
    }
    if pc.t() {
        for r in pc.g("rows").items() {
            s84a.push(jobj! {
                "stage" => format!("{} row ({})", pc.g("precinct").g("id").s(), r.g("row").s()), "trigger" => format!("{} dwellings", fmt::int(r.g("dw").n())),
                "measure" => r.g("up").items().iter().map(|u| format!("{} ({})", u.g("name").s(), u.g("stateLabel").s())).collect::<Vec<_>>().join("; "),
            });
        }
    }
    let sens = if p.g("report").g("sens").g("rows").t() { p.g("report").g("sens").clone() } else { V::Null };
    let loaded = if p.g("data").g("loaded").t() { p.g("data").g("loaded").clone() } else { V::object() };
    let labels = xdata("SET_LABELS");
    let assumptions_rows: Vec<V> = assumptions(p)
        .entries()
        .iter()
        .map(|(k, v)| {
            let l = labels.g(k);
            jarr![if l.t() { l.ix(0).clone() } else { V::String(k.to_string()) }, *v, if l.t() { l.ix(1).clone() } else { V::String(String::new()) }, assumption()]
        })
        .collect();
    let sources: Vec<V> = loaded
        .entries()
        .iter()
        .map(|(k, v)| {
            let at = if v.g("at").t() {
                let ms = dates::to_ms(v.g("at"), dates::now_ms());
                if ms.is_finite() { crate::edit::ids::iso_string(ms)[..10].to_string() } else { String::new() }
            } else {
                String::new()
            };
            let lic = if v.g("licence").t() { v.g("licence").clone() } else { v.g("source").clone() };
            jarr![
                *k, at, if v.t() && !v.g("n").nullish() { v.g("n").clone() } else { V::String(String::new()) }, if v.t() && lic.t() { lic } else { V::String(String::new()) }
            ]
        })
        .collect();
    jobj! {
        "matrix" => V::Array(matrix), "aee" => V::Array(aee), "conditions" => mitigation::condition_table(p), "s85" => s85, "s84a" => V::Array(s84a), "invitees" => invitees(p),
        "timeline" => ftaa_timeline(p),
        "reviewer" => jobj! {
            "assumptions" => V::Array(assumptions_rows), "sources" => V::Array(sources), "sensitivity" => sens,
            "model" => "project.json (the full Spare Construct project: network, zones, settings, counts and results)",
        },
        "notes" => strs([
            "GRPA approvals sit outside Fast-track: NZTA consent under GRPA s91 and s93 (crossing places) and s176 consent on designated land cannot be granted by the panel (FTAA s42(4)). Apply to NZTA separately.".to_string(),
            format!("Lapse: a Fast-track approval lapses {} years after it commences unless the panel sets another period (FTAA Sch 5 cl 26).", xdata("STD.ftaa.lapseYears").s()),
        ]),
        "conditionsLabel" => label("s84a"), "ftaa" => c.g("ftaa"),
    }
}

/// `Store.serialise(P)`: the project as JSON text without the cached paths, arc keys and settings notice.
pub fn serialise(p: &V) -> String {
    fn strip(v: &V) -> V {
        match v {
            V::Array(a) => V::Array(a.iter().map(strip).collect()),
            V::Object(_) => {
                let mut o = V::object();
                for (k, x) in v.entries() {
                    if k != "paths" && k != "arcKeys" && k != "settingsNotice" {
                        o.set(k, strip(x));
                    }
                }
                o
            }
            other => other.clone(),
        }
    }
    strip(p).stringify()
}

/// `ftaaFiles(P)`: the Fast-track package as files, file name to content, in the order the package lists them.
pub fn ftaa_files(p: &V) -> Vec<(String, String)> {
    let f = ftaa_package(p);
    let s84a_label = label("s84a");
    let mut files: Vec<(String, String)> = Vec::new();
    let mut m = vec![cells!["Rule", "Requirement", "Comply / infringe", "Evidence"]];
    m.extend(f.g("matrix").items().iter().map(|r| cells![r.g("rule"), r.g("req"), r.g("result"), r.g("evidence")]));
    files.push(("compliance-matrix.csv".into(), csv(&m)));
    let mut a = vec![cells!["Section", "Reference"]];
    a.extend(f.g("aee").items().iter().map(|s| cells![s.g("title"), s.g("ref")]));
    files.push(("aee-outline.csv".into(), csv(&a)));
    let mut cd = vec![cells!["Ref", "Measure", "Metric", "Threshold", "Survey", "Frequency", "Response", "Nexus", "Label"]];
    cd.extend(f.g("conditions").items().iter().map(|r| {
        cells![r.g("ref"), r.g("measure"), r.g("metric"), r.g("threshold"), r.g("survey"), r.g("frequency"), r.g("response"), r.g("basis"), s84a_label.as_str()]
    }));
    files.push(("conditions.csv".into(), csv(&cd)));
    let mut s = vec![cells!["Side", "Item", "Value"]];
    s.extend(f.g("s85").g("benefits").items().iter().map(|r| cells!["Benefit", r.ix(0), r.ix(1)]));
    s.extend(f.g("s85").g("impacts").items().iter().map(|r| cells!["Residual impact", r.ix(0), r.ix(1)]));
    s.push(cells!["Label", "Junction results", label("junction")]);
    files.push(("s85-panel.csv".into(), csv(&s)));
    let mut st = vec![cells!["Stage", "Trigger", "Measure"]];
    st.extend(f.g("s84a").items().iter().map(|r| cells![r.g("stage"), r.g("trigger"), r.g("measure")]));
    files.push(("s84a-staging.csv".into(), csv(&st)));
    let mut iv = vec![cells!["Invitee", "Basis"]];
    iv.extend(f.g("invitees").items().iter().map(|i| cells![i.g("who"), i.g("basis")]));
    files.push(("invitees.csv".into(), csv(&iv)));
    let mut tl = vec![cells!["Step", "Date", "Rule", "Basis"]];
    tl.extend(f.g("timeline").items().iter().map(|t| cells![t.g("step"), or_s(t.g("date"), ""), t.g("rule"), or_s(t.g("basis"), "")]));
    files.push(("timeline.csv".into(), csv(&tl)));
    files.push(("notes.txt".into(), f.g("notes").items().iter().map(|n| n.s()).collect::<Vec<_>>().join("\n")));
    let rv = f.g("reviewer");
    let mut asm = vec![cells!["Assumption", "Value", "Unit", "Basis"]];
    asm.extend(rv.g("assumptions").items().iter().map(|r| r.items().to_vec()));
    files.push(("reviewer/assumptions.csv".into(), csv(&asm)));
    let mut src = vec![cells!["Data", "Loaded", "Records", "Licence or source"]];
    src.extend(rv.g("sources").items().iter().map(|r| r.items().to_vec()));
    files.push(("reviewer/sources.csv".into(), csv(&src)));
    // Engine.sensitivity gives dVht as a fraction of the central run's VHT: printed as a percentage
    let sens = rv.g("sensitivity");
    let sens_text = if sens.t() {
        let mut rows = vec![cells!["Test", "Input", "Trips", "VHT (veh\u{b7}h)", "VHT change (% of the central run)", "Sentence"]];
        rows.extend(sens.g("rows").items().iter().map(|r| {
            let lab = if r.g("label").t() { r.g("label").clone() } else if r.g("test").t() { r.g("test").clone() } else { r.g("id").clone() };
            let dv = r.g("dVht");
            cells![
                lab, or_s(r.g("input"), ""), super::packs::r1(r.g("trips")), super::packs::r1(r.g("vht")),
                if !dv.nullish() && dv.n().is_finite() { format!("{}%", to_fixed(dv.n() * 100.0, 1)) } else { String::new() }, or_s(r.g("sentence"), "")
            ]
        }));
        csv(&rows)
    } else {
        "Sensitivity tests have not been run: run the sensitivity pack (Report tab) before lodging.".to_string()
    };
    files.push(("reviewer/sensitivity.csv".into(), sens_text));
    files.push(("reviewer/model-inputs.json".into(), serialise(p)));
    files
}

/// `ftaaBundle(P)`: the Fast-track package as one JSON text: its format, the date, the project, the version stamp, the disclaimers, the manifest and the files.
pub fn ftaa_bundle(p: &V) -> String {
    let files = ftaa_files(p);
    let mut fo = V::object();
    for (k, v) in &files {
        fo.put(k, v.as_str());
    }
    let b = jobj! {
        "format" => "corridor-ftaa-package", "version" => 1, "prepared" => today(), "project" => or_s(p.g("name"), ""), "stamp" => stamp(p),
        "disclaimers" => jobj! { "D1" => disclaimer("D1", p, None), "D3" => disclaimer("D3", p, None), "D12" => disclaimer("D12", p, Some("a Fast-track application")) },
        "manifest" => strs(files.iter().map(|(k, _)| k)), "files" => fo,
    };
    b.stringify_pretty(1)
}

/// The README of the Fast-track ZIP (the text `ftaaZip` adds first): the project, date, version stamp, disclaimers D1, D3 and D12 and the manifest, lines joined by CRLF.
pub fn ftaa_readme(p: &V) -> String {
    let files = ftaa_files(p);
    let mut lines: Vec<String> = vec![
        format!("Spare Construct Fast-track package ({}), prepared {}.", if p.g("name").t() { p.g("name").s() } else { "project".to_string() }, today()),
        format!("Version stamp: {}", stamp(p)),
        String::new(),
        format!("D1. {}", disclaimer("D1", p, None)),
        format!("D3. {}", disclaimer("D3", p, None)),
        format!("D12. {}", disclaimer("D12", p, Some("a Fast-track application"))),
        String::new(),
        "Manifest:".to_string(),
    ];
    lines.extend(files.iter().map(|(k, _)| format!("  {k}")));
    lines.join("\r\n")
}

/// `ftaaZip(P)`: the Fast-track export. The prototype builds a ZIP with the report lane's `Report.zip` (README.txt then every file of
/// [`ftaa_files`]) and falls back to the JSON bundle when that is not loaded; this gives the bundle, so the export is named ".json".
pub fn ftaa_zip(p: &V) -> String {
    ftaa_bundle(p)
}

fn report_ftaa(p: &V, ctx: Option<&dyn ReportCtx>) -> String {
    let f = ftaa_package(p);
    if !f.g("ftaa").t() {
        return String::new();
    }
    let kv = |k: &str, rows: &mut Vec<Vec<String>>, side: &str| {
        for r in f.g("s85").g(k).items() {
            rows.push(vec![side.to_string(), esc(r.ix(0)), esc(r.ix(1))]);
        }
    };
    let mut rows: Vec<Vec<String>> = Vec::new();
    kv("benefits", &mut rows, "Benefit");
    kv("impacts", &mut rows, "Residual impact");
    let mut out = rt(ctx, "s85(3) regional benefits and residual impacts", &tbl(&["Side", "Item", "Spare Construct estimate"], rows, ""), &esc(f.g("s85").g("note")));
    out.push_str(&disc("D3", p, None));
    out.push_str("\n        ");
    let mrows: Vec<Vec<String>> = f.g("matrix").items().iter().map(|r| vec![esc(r.g("rule")), esc(r.g("req")), esc(r.g("result")), esc(r.g("evidence"))]).collect();
    out.push_str(&rt(ctx, "Compliance matrix (FTAA Sch 5 cl 5(1)(h), (3))", &tbl(&["Rule", "Requirement", "Result", "Evidence"], mrows, ""), ""));
    out.push_str("\n        ");
    let irows: Vec<Vec<String>> = f.g("invitees").items().iter().map(|i| vec![esc(i.g("who")), esc(i.g("basis"))]).collect();
    out.push_str(&rt(ctx, "s53 invitees", &tbl(&["Invitee", "Basis"], irows, ""), ""));
    for n in f.g("notes").items() {
        out.push_str(&format!("<p class=\"note\">{}</p>", esc(n)));
    }
    out
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-ftaa"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id == "compliance-extra-ftaa" { Some(report_ftaa(p, ctx)) } else { None }
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-ftaa"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's. The Fast-track export is the JSON bundle
/// (see [`ftaa_zip`]).
pub fn export_build(id: &str, p: &V) -> Option<String> {
    if id == "compliance-extra-ftaa" { Some(ftaa_zip(p)) } else { None }
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, _p: &V) -> Option<bool> {
    if id == "compliance-extra-ftaa" { Some(true) } else { None }
}
