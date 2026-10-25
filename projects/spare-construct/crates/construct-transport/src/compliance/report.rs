//! The Compliance report sections, exports and CSV stamping rules (features/compliance.js section 16), and the dispatch over both modules'
//! sections and exports.

use super::access::{access_check, operators, sh_check};
use super::authority::authorities;
use super::checklist::{checklist, checklist_csv};
use super::clocks::clocks;
use super::data::c as table;
use super::dates::{long_date_v};
use super::docs::{disclaimer, sign_state, stamp_csv};
use super::e27::{e27, ita_scope, scoping_memo};
use super::engine;
use super::expert::{appendix_g, jws_csv, supersession};
use super::fds::fds_check;
use super::html::{ReportCtx, p_, report_table};
use super::jv::{Jx, V, csv, st};
use super::record::{active_sc, num, numf, recheck, regime, stamp, state};
use super::{extra, record};
use crate::util::{esc, fmt};

fn t(head: &[&str], rows: Vec<Vec<String>>) -> String {
    report_table(head, &rows)
}

fn d_(id: &str, p: &V) -> String {
    format!("<div class=\"disc\" data-d=\"{id}\" data-disclaimer=\"{id}\"><b>{id} {}.</b> {}</div>", esc(&table("DISCLAIMERS").g(id).g("title").sn()), esc(&disclaimer(id, Some(p), None)))
}

fn e(v: &V) -> String {
    esc(&v.sn())
}

fn acc_status(a: &V) -> &'static str {
    match a.g("worst").s().as_str() {
        "ok" => "Complies",
        "NC" => "Non-complying",
        "RD" => "Restricted discretionary",
        _ => "Not checked: AUP controls not loaded",
    }
}

fn nvalue(v: &V) -> String {
    num(v, 0)
}

/// The ids of the Compliance report sections (`REPORT`).
pub const REPORT_IDS: &[&str] = &[
    "report-regulatory", "compliance-model-status", "compliance-e27", "compliance-discretion", "compliance-scope", "compliance-access", "compliance-network-scope", "compliance-sh", "compliance-fds",
    "compliance-operators", "compliance-standards", "compliance-expert", "compliance-timeline", "compliance-checklist",
];

/// The HTML of a Compliance report section (`REPORT[id].render(P)`), None when the id is not one of this module's. A section with nothing to say gives an empty string.
pub fn report(id: &str, p: &V) -> Option<String> {
    Some(match id {
        "report-regulatory" => {
            let s = stamp(p, &V::Null);
            let a = authorities(p, &V::Null);
            let sg = sign_state(p);
            format!("{}{}{}\n        <h3>Authorities</h3>{}{}{}{}{}{}",
                if sg.g("watermark").t() { format!("<p class=\"warn\"><b>{}</b></p>", e(sg.g("watermark"))) } else { String::new() },
                t(&["Item", "Value"], s.g("lines").items().iter().map(|l| vec![e(l.ix(0)), e(l.ix(1))]).collect()),
                if recheck(p).g("show").t() { format!("<p class=\"warn\"><b>{}.</b></p>", e(table("REGIME.recheck"))) } else { String::new() },
                t(&["Role", "Body", "Basis"], a.g("rows").items().iter().map(|r| vec![e(r.g("role")), e(r.g("name")), e(r.g("basis"))]).collect()),
                if a.g("transitionalText").t() { p_(&a.g("transitionalText").s()) } else { String::new() },
                if a.g("requiring").t() { p_(&a.g("requiring").s()) } else { String::new() },
                p_(&a.g("arcf").s()), d_("D10", p), d_("D11", p))
        }
        "compliance-model-status" => {
            let m: Vec<V> = ["AM", "PM"].iter().map(|per| engine::model_status(p, per)).collect();
            let s = p.g("settings").g("tmdg");
            let std = engine::standards_table();
            let cat = if s.g("category").t() { s.g("category").s() } else { "E".into() }.to_uppercase();
            let just = if s.g("justification").t() { s.g("justification").s() } else { "[author to justify the TMDG category (Settings \u{203a} Calibration and validation \u{203a} Why this category)]".into() };
            format!("{}{}\n        {}{}{}{}",
                t(&["Period", "Status", "TMDG category", "Date"], m.iter().map(|x| vec![x.g("period").sn(), e(x.g("label")), e(x.g("category")), esc(&if x.g("date").t() { x.g("date").s() } else { "\u{2013}".into() })]).collect()),
                p_(&format!("TMDG category {cat}. Justification: {just}")), p_(&std.g("tmdg").g("standing").s()),
                if p.g("data").g("msm").g("links").t() { String::new() } else { p_(&std.g("msm").g("notChecked").s()) }, d_("D6", p), d_("D7", p))
        }
        "compliance-e27" => {
            let ee = e27(p);
            let a: Vec<V> = state(p).g("access").items().iter().map(|x| access_check(p, x, active_sc(p))).collect();
            let rows: Vec<Vec<String>> = ee.g("rows").items().iter().filter(|r| r.g("q").n() > 0.0 || r.g("exceeded").t()).map(|r| vec![r.g("id").sn(), format!("{} {}", num(r.g("thr"), 0), e(r.g("unit"))), num(r.g("q"), 0), if r.g("exceeded").t() { "Yes".into() } else { "No".into() }, r.g("tier").sn(), e(r.g("consequence"))]).collect();
            let infr: Vec<Vec<String>> = a.iter().flat_map(|x| x.g("flags").items().iter().map(|f| vec![e(x.g("name")), e(f.g("rule")), f.g("status").sn(), e(f.g("text"))]).collect::<Vec<_>>()).collect();
            format!("{}{}{}\n        {}\n        {}{}{}{}<p class=\"note\"><em>{}</em></p>\n        {}{}",
                if ee.g("redirect").t() { p_(&format!("{}.", ee.g("redirect").g("text").s())) } else { String::new() },
                if ee.g("exempt").t() { p_(&format!("{}.", ee.g("exempt").g("text").s())) } else { String::new() },
                if ee.g("pc120").t() { format!("{}<p class=\"note\">{}</p>", p_(&ee.g("pc120").g("text").s()), e(ee.g("pc120").g("status"))) } else { String::new() },
                t(&["Rule", "Threshold", "Proposal quantity", "Exceeded", "Tier", "Consequence"], rows),
                p_(&format!("{}: {}", ee.g("catchAll").g("id").s(), ee.g("catchAll").g("why").s())),
                if ee.g("tier").t() { p_(&ee.g("consequence").s()) } else { String::new() },
                ee.g("notes").items().iter().map(|n| p_(&n.s())).collect::<String>(), p_(&ee.g("wording").s()), e(ee.g("version")),
                if a.iter().any(|x| x.g("flags").jlen() > 0) { format!("<h3>Access infringements (E27.6.4)</h3>{}", t(&["Access", "Rule", "Status", "Finding"], infr)) } else { String::new() }, d_("D13", p))
        }
        "compliance-discretion" => {
            let ee = e27(p);
            let r = regime(p);
            let tier = ee.g("tier").n();
            let a = if ee.g("tier").t() && tier == 1.0 { p_(&format!("{}. {}", table("E27.tier1Scope").s(), ee.g("consequence").s())) } else if ee.g("tier").t() && tier == 2.0 { p_(&format!("{}.", table("E27.tier2Scope").s())) } else { p_("No E27.6.1 matters of discretion are triggered.") };
            format!("{}{}{}\n        {}\n        <h3>Objectives and policies (E27.2, E27.3)</h3>{}\n        <p class=\"note\">The objectives and policies are paraphrased; quote the plan text in the evaluation.</p>",
                a, if ee.g("supplementary").t() { p_(&format!("Network level-of-service sections are {}.", table("E27.supplementary").s())) } else { String::new() }, p_(&format!("{}.", table("E27.s104C").s())),
                if r.g("side").eq_s("transition") { p_("For a district land use consent lodged from 30 October 2026, the effects listed in s104(1A) are disregarded (s104(1A), s92(2AA)), and Planning Act and NEA instruments are not relied on (s104(1C)).") } else { String::new() },
                t(&["Provision", "Applies", "Evidence"], ee.g("matrix").items().iter().map(|m| vec![format!("{} {}", m.g("ref").sn(), e(m.g("text"))), if m.g("applies").t() { "Yes".into() } else { format!("No ({})", e(m.g("why"))) }, esc(&m.g("evidence").items().iter().map(|x| x.s()).collect::<Vec<_>>().join("; "))]).collect()))
        }
        "compliance-scope" => {
            let i = ita_scope(p);
            let memo = scoping_memo(p, active_sc(p));
            let pa = i.g("paragraph");
            format!("{}{}\n        {}\n        <h3>Proportionality statement</h3>{}{}\n        {}{}\n        <h3>Scoping memo</h3>{}{}",
                i.g("aup").items().iter().map(|x| p_(&x.s())).collect::<String>(),
                if i.g("at").jlen() > 0 { format!("<h3>AT ITA Guidelines 2025 v2: ITA likely</h3><ul>{}</ul>", i.g("at").items().iter().map(|x| format!("<li>{}</li>", esc(&x.s()))).collect::<String>()) } else { String::new() },
                if i.g("nzta").t() { format!("<h3>{} Table 1</h3>{}{}{}", e(table("NZTA_T1.label")), p_(&i.g("nzta").g("text").s()), if i.g("depth").t() { p_(&i.g("depth").g("text").s()) } else { String::new() }, p_(&i.g("nzta").g("order").s())) } else { String::new() },
                p_(&pa.g("text").s()),
                if pa.g("confirmed").t() { p_(&format!("Confirmed by {}{}.", if pa.g("by").t() { pa.g("by").s() } else { "the author".into() }, if pa.g("date").t() { format!(" on {}", long_date_v(pa.g("date"))) } else { String::new() })) } else { "<p class=\"warn\">[Not yet confirmed by the author.]</p>".to_string() },
                t(&["Driver", "Value"], i.g("drivers").items().iter().map(|d| vec![e(d.ix(0)), e(d.ix(1))]).collect()),
                p_(&format!("Recommended level: {} ({} bands).", i.g("level").s(), record::assumption())),
                t(&["Item", "Proposed", "Agreed by", "Date"], memo.items().iter().map(|m| vec![e(m.g("label")), e(m.g("value")), e(m.g("by")), e(m.g("date"))]).collect()),
                d_("D13", p))
        }
        "compliance-access" => {
            let a: Vec<V> = state(p).g("access").items().iter().map(|x| access_check(p, x, active_sc(p))).collect();
            if a.is_empty() {
                return Some(String::new());
            }
            let rows = a.iter().map(|x| {
                let mut parts: Vec<String> = x.g("flags").items().iter().map(|f| esc(&format!("{}: {}", f.g("rule").s(), f.g("text").s()))).collect();
                if x.g("uncheckedText").t() {
                    parts.push(format!("<i>{}</i>", e(x.g("uncheckedText"))));
                }
                vec![e(x.g("name")), esc(acc_status(x)), if parts.is_empty() { "No breach found".into() } else { parts.join("<br>") }]
            }).collect();
            format!("{}{}{}", t(&["Access", "Status", "Findings"], rows), p_(&format!("{}.", table("ACCESS.nearBasis").s())), d_("D13", p))
        }
        "compliance-network-scope" => {
            let ee = e27(p);
            if !ee.g("supplementary").t() {
                return Some(String::new());
            }
            let scope = table("E27.tier1Scope").s();
            let short = super::rx::rx(r"^[^:]+: ", "").replace_with(&scope, "");
            format!("{}{}", p_(&format!("E27 tier 1: only TA1, T1A, T2A or T3A is exceeded, so the matters of discretion are {short} (E27.8.1(4A), E27.8.2(3A)).")),
                p_(&format!("The network level-of-service sections that follow (extent of effects, junction performance) are marked \"{}\". {}.", table("E27.supplementary").s(), table("E27.s104C").s())))
        }
        "compliance-sh" => {
            let x = sh_check(p, active_sc(p));
            if !x.g("inArea").t() && !x.g("involved").t() {
                return Some(String::new());
            }
            let rows = x.g("rows");
            let d12 = super::rx::rx(r"\[[^\]]*\]", "").replace_with(&d_("D12", p), "an NZTA access application");
            format!("{}\n        {}\n        {}\n        {}{}\n        {}{}",
                if x.g("triggers").jlen() > 0 { format!("<ul>{}</ul>", x.g("triggers").items().iter().map(|v| format!("<li>{}</li>", esc(&v.s()))).collect::<String>()) } else { p_("No NZTA trigger case found.") },
                t(&["Approval", "Needed"], x.g("approvals").items().iter().map(|a| vec![e(a.g("label")), if a.g("needed").t() { "Yes".into() } else if a.g("check").t() { "Check".into() } else { "No".into() }]).collect()),
                if rows.jlen() > 0 {
                    t(&["Link", "AM with (veh/h)", "AM without (veh/h)", "PM with (veh/h)", "PM without (veh/h)", "With (veh/day)", "HCV %", "Pavement"],
                        rows.items().iter().take(40).map(|r| vec![esc(&format!("{} {}", r.g("ref").sn(), r.g("name").sn())), nvalue(r.g("per").g("AM").g("with")), nvalue(r.g("per").g("AM").g("without")), nvalue(r.g("per").g("PM").g("with")), nvalue(r.g("per").g("PM").g("without")), nvalue(r.g("dayWith")), num(r.g("hvPct"), 1),
                            if r.g("pavement").t() { format!("{}{}{}", e(r.g("pavement").g("text")), if r.g("pavement").g("flagged").t() { ": pavement assessment needed" } else { "" }, if r.g("pavement").g("proxy").t() { " (total-traffic proxy)" } else { "" }) } else { "\u{2013}".into() }]).collect()) + &p_(&x.g("pavementBasis").s())
                } else { String::new() },
                if x.g("tech").jlen() > 0 { t(&["Access", "Required SISD (m)", "Basis", "Movements a day", "Access type"], x.g("tech").items().iter().map(|a| vec![e(a.g("name")), e(a.g("sisd").g("m")), e(a.g("sisd").g("basis")), format!("{} ({})", nvalue(a.g("daily")), e(a.g("volBasis"))), e(a.g("type").g("text"))]).collect()) } else { String::new() },
                if x.g("noise").t() { p_("A noise-sensitive activity is within 100 m of a state highway: assess reverse sensitivity and acoustic design (NZTA PPM Managing effects s5.2.2).") } else { String::new() },
                p_(&x.g("labels").items().iter().map(|v| v.s()).collect::<Vec<_>>().join("; ")), d12)
        }
        "compliance-fds" => {
            let x = fds_check(p);
            let c = state(p);
            if !x.g("applies").t() {
                return Some(String::new());
            }
            format!("<p><b>{}</b></p>{}{}{}", e(x.g("heading")), p_(&x.g("paragraph").s()),
                if x.g("fundingPrompt").t() { p_(&if c.g("fds").g("fundingGap").t() { c.g("fds").g("fundingGap").s() } else { format!("[{}]", x.g("fundingPrompt").s()) }) } else { String::new() },
                if c.g("fds").g("pair").t() { t(&["Measure", "With", "Without", "Change"], c.g("fds").g("pair").g("rows").items().iter().map(|r| vec![e(r.g("label")), num(r.g("with"), r.g("dp").n() as usize), num(r.g("without"), r.g("dp").n() as usize), if r.g("d").nullish() { "\u{2013}".into() } else { fmt::signed(r.g("d").n(), r.g("dp").n() as usize) }]).collect()) } else { String::new() })
        }
        "compliance-operators" => {
            let o = operators(p);
            format!("{}{}{}\n        {}{}{}", p_(&o.g("label").s()), if o.g("note").t() { format!("<p class=\"warn\">{}</p>", e(o.g("note"))) } else { String::new() },
                t(&["Operator", "Links in area", "Links over threshold", "Junctions over threshold", "Affected"], o.g("rows").items().iter().map(|r| vec![e(r.g("name")), num(r.g("assets"), 0), numf(r.g("links").jlen() as f64, 0), numf(r.g("nodes").jlen() as f64, 0), if r.g("affected").t() { "Yes".into() } else { "No".into() }]).collect()),
                if regime(p).g("side").eq_s("transition") { p_("For applications lodged from 30 October 2026, AUP cross-references to s95A(9) have no target (repealed); s95B(10) remains.") } else { String::new() }, d_("D11", p), d_("D13", p))
        }
        "compliance-standards" => {
            let sup = supersession(p, &V::Null);
            format!("{}{}{}", sup.items().iter().map(|s| format!("<p class=\"warn\">{}</p>", e(s.g("text")))).collect::<String>(),
                t(&["Document", "Edition", "Effective", "Next known change", "Checked against source"], table("STANDARDS").items().iter().map(|s| vec![e(s.g("doc")), e(s.g("edition")), esc(&if s.g("effective").t() { super::dates::long_date(&s.g("effective").s()) } else { "\u{2013}".into() }),
                    esc(&if s.g("next").t() { format!("{}{}", if s.g("next").g("date").t() { format!("{}: ", super::dates::long_date(&s.g("next").g("date").s())) } else { String::new() }, s.g("next").g("what").s()) } else { "\u{2013}".into() }), if s.g("verified").t() { "Yes".into() } else { "No".into() }]).collect()),
                p_(&table("PLANNING_ONLY").s()))
        }
        "compliance-expert" => {
            let g = appendix_g(p);
            let roles = table("ROLES");
            format!("{}{}{}\n        <h3>Sign-off</h3>{}{}",
                if g.g("watermark").t() { format!("<p class=\"warn\"><b>{}</b></p>", e(g.g("watermark"))) } else { String::new() },
                if g.g("prompt").t() { format!("<p class=\"warn\">{}</p>", e(g.g("prompt"))) } else { String::new() },
                g.g("sections").items().iter().map(|s| format!("<h3>{}</h3><p>{}</p>", esc(&s.ix(0).s()), esc(&s.ix(1).s()).replace('\n', "<br>"))).collect::<String>(),
                if g.g("signoffs").jlen() > 0 { t(&["Role", "Name", "Qualifications", "Date"], g.g("signoffs").items().iter().map(|s| vec![roles.g(&s.g("role").s()).sn(), e(s.g("name")), e(s.g("quals")), esc(&long_date_v(s.g("date")))]).collect()) } else { p_("Not signed off.") }, d_("D2", p))
        }
        "compliance-timeline" => {
            let k = clocks(p);
            if k.jlen() > 0 {
                t(&["Event", "Date", "Rule", "Due", "What"], k.items().iter().flat_map(|c| c.g("rows").items().iter().map(|r| vec![e(c.g("label")), esc(&long_date_v(c.g("date"))), e(r.g("id")), esc(&long_date_v(r.g("date"))), e(r.g("text"))]).collect::<Vec<_>>()).collect())
                    + &p_("Working days exclude weekends, public holidays and the statutory Christmas period (RMA s2; Legislation Act 2019 s13 for the FTAA).")
            } else {
                p_("No statutory events recorded.")
            }
        }
        "compliance-checklist" => {
            let all = checklist(p);
            let l: Vec<&V> = all.items().iter().filter(|i| i.g("applies").t()).collect();
            let ck_state = |s: &str| match s { "done" => "Done", "todo" => "To do", "warn" => "Check", "na" => "Not applicable", o => o }.to_string();
            format!("{}<p class=\"note\">Items that do not apply to the {} pathway are left out ({} of {}).</p>",
                t(&["ID", "Item", "Source", "Status", "Spare Construct finding or note"], l.iter().map(|i| {
                    let src = format!("{}{}", i.g("source").s(), if i.g("refNote").t() { format!(" (checked: {})", i.g("refNote").s()) } else { String::new() });
                    let note: Vec<String> = [i.g("text").s(), i.g("note").s()].into_iter().filter(|s| !s.is_empty()).collect();
                    vec![i.g("id").sn(), e(i.g("item")), esc(&src), esc(&ck_state(&i.g("state").s())), esc(&note.join(" \u{b7} "))]
                }).collect()),
                esc(&regime(p).g("pathwayLabel").s().to_lowercase()), all.jlen() - l.len(), table("CHECKLIST").jlen())
        }
        _ => return None,
    })
}

/// The HTML of any compliance report section, from either module (`REPORT` of both scripts); None when the id is not one of theirs.
pub fn report_section(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    report(id, p).or_else(|| extra::report(id, p, ctx))
}

/// The ids of the exports of the Compliance module (`EXPORTS`).
pub const EXPORT_IDS: &[&str] = &["compliance-checklist", "compliance-scoping", "compliance-register", "compliance-timeline", "compliance-jws"];

/// Exports that carry author text are refused while the scope linter has a blocking finding (`GUARD`, `authorText`).
pub fn export_guarded(id: &str) -> bool {
    ["report-html", "report-datapack", "compliance-extra-ftaa", "compliance-extra-planchange", "compliance-scoping", "compliance-jws", "compliance-extra-mitigation"].contains(&id)
}

/// The file name of an export (`filename(c)` with `c.fileBase`), None for an unknown id. The Fast-track package is a zip only when a zip writer exists, otherwise a json bundle.
pub fn export_filename(id: &str, file_base: &str) -> Option<String> {
    Some(match id {
        "compliance-checklist" => format!("{file_base}-regulatory-checklist.csv"),
        "compliance-scoping" => format!("{file_base}-scoping-memo.csv"),
        "compliance-register" => format!("{file_base}-register.csv"),
        "compliance-timeline" => format!("{file_base}-statutory-timeline.csv"),
        "compliance-jws" => format!("{file_base}-jws-inputs.csv"),
        _ => {
            let key = id.strip_prefix("compliance-extra-")?;
            let ext = table_ext(key);
            format!("{file_base}-{}.{}", if key == "anop" { "anop".to_string() } else { format!("cx-{key}") }, ext)
        }
    })
}

fn table_ext(key: &str) -> &'static str {
    if key == "ftaa" { "json" } else { "csv" }
}

/// Whether an export is offered for the project (`enabled()`), None for an unknown id.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    Some(match id {
        "compliance-checklist" | "compliance-scoping" => true,
        "compliance-register" => p.g("register").jlen() > 0,
        "compliance-timeline" => clocks(p).jlen() > 0,
        "compliance-jws" => state(p).g("diff").g("result").t(),
        _ => return extra::export_enabled(id, p),
    })
}

/// The text of an export (`build(P)`): `Ok(text)`, or `Err(why)` when the scope linter blocks an export that carries author text; None for an unknown id.
pub fn export_build(id: &str, p: &V, now: &V) -> Option<Result<String, String>> {
    if export_guarded(id) {
        if let Some(why) = extra::lint::export_block(p) {
            return Some(Err(why));
        }
    }
    let c = state(p);
    Some(Ok(match id {
        "compliance-checklist" => checklist_csv(p, now),
        "compliance-scoping" => {
            let mut rows: Vec<Vec<V>> = vec![["Item", "Proposed", "Agreed by", "Date", "Locked"].iter().map(|s| st(*s)).collect()];
            for m in scoping_memo(p, active_sc(p)).items() {
                rows.push(vec![m.g("label").clone(), m.g("value").clone(), m.g("by").clone(), m.g("date").clone(), st(if m.g("locked").t() { "yes" } else { "no" })]);
            }
            stamp_csv(&csv(&rows), p, None)
        }
        "compliance-register" => {
            let mut rows: Vec<Vec<V>> = vec![["Name", "Type", "Status", "Likelihood", "Funding", "Source", "As at", "Verify", "Included in S1", "Reason"].iter().map(|s| st(*s)).collect()];
            for r in engine::register_table(p).items() {
                rows.push(vec![r.g("name").clone(), r.g("type").clone(), r.g("status").clone(), r.g("likely").clone(), r.g("funding").clone(), r.g("source").clone(), r.g("asAt").clone(), st(if r.g("verify").t() { "verify" } else { "" }), st(if r.g("included").t() { "yes" } else { "no" }), r.g("reason").clone()]);
            }
            stamp_csv(&csv(&rows), p, None)
        }
        "compliance-timeline" => {
            let mut rows: Vec<Vec<V>> = vec![["Event", "Event date", "Rule", "Due", "What"].iter().map(|s| st(*s)).collect()];
            for k in clocks(p).items() {
                for r in k.g("rows").items() {
                    rows.push(vec![k.g("label").clone(), k.g("date").clone(), r.g("id").clone(), r.g("date").clone(), r.g("text").clone()]);
                }
            }
            stamp_csv(&csv(&rows), p, None)
        }
        "compliance-jws" => jws_csv(p, c.g("diff").g("result")),
        _ => {
            let short = id.strip_prefix("compliance-extra-")?;
            let _ = short;
            return extra::export_build(id, p).map(Ok);
        }
    }))
}

/// `stamps(d)`: whether an export registered by another part of the app gets the disclaimer, watermark and version stamp footer (CSV exports on the
/// human-readable list, or any that ask with `stamp: true`; machine hand-over files stay byte-clean).
pub fn stamps_export(id: &str, ext: &str, stamp_flag: Option<bool>) -> bool {
    let st_t = table("STAMP");
    stamp_flag != Some(false) && ext.eq_ignore_ascii_case("csv") && !st_t.g("handover").has(id) && (stamp_flag == Some(true) || st_t.g("tables").items().iter().any(|x| x.eq_s(id)))
}

/// What `stampExports` does to the output of a registered export: the CSV with its footer rows (and D12 for a hand-over tool), after remembering the outputs' regime.
pub fn stamp_export_text(p: &mut V, text: &str, handover: Option<&str>, now: &V) -> String {
    record::record_outputs(p, now);
    stamp_csv(text, p, handover)
}

/// The label of a model.js stage, for callers that print the checklist's group labels.
pub fn group_label(group: &str) -> String {
    let g = table("CK_GROUPS").g(group);
    if g.t() { g.s() } else { group.to_string() }
}
