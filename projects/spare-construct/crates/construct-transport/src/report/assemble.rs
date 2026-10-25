//! The report's section list and `build`: render every section in its chapter, number it, add the disclaimers every table must
//! carry, number the tables and figures and write the contents list.

use super::build::{
    BuildOpts, Built, TocEntry, chapter_of, fix_tables, has_assumption, has_source_line, has_word, number_figcaptions, number_foreign,
};
use super::consts::{Chapter, chapters};
use super::ctx::{ChapterState, Ctx, SectionState, disc};
use super::host::ForeignSection;
use super::{Rep, pick};
use super::{sections_app as app, sections_assess as assess, sections_crash as crash, sections_end as end, sections_env as env};
use super::{sections_front as front, sections_prop as prop};
use crate::jsnum::{is_js_space, js_trim};
use crate::junction_tools::{ReportCtx, perf_title, report_section_ids, report_section_meta};
use crate::util::esc;
use std::collections::HashMap;

type RenderFn = for<'a, 'r> fn(&mut Ctx<'a, 'r>) -> String;

/// How a section's HTML is made.
#[derive(Clone, Copy)]
enum Kind {
    /// One of the report's own sections.
    Own(RenderFn),
    /// One of the junction tools' sections, by id.
    Junction(&'static str),
    /// Another feature's section, already rendered: its place in the host's list.
    Foreign(usize),
}

/// A section as `Registry.report` holds it.
struct Def {
    id: String,
    chapter: String,
    order: f64,
    at: Option<&'static str>,
    title: Option<String>,
    tag: Option<&'static str>,
    landscape: bool,
    fallback: bool,
    numbered: bool,
    kind: Kind,
}

#[allow(clippy::too_many_arguments)]
fn own(
    id: &str,
    chapter: &str,
    order: f64,
    at: Option<&'static str>,
    title: Option<&str>,
    tag: &'static str,
    landscape: bool,
    fallback: bool,
    numbered: bool,
    f: RenderFn,
) -> Def {
    Def {
        id: id.to_string(),
        chapter: chapter.to_string(),
        order,
        at,
        title: title.map(str::to_string),
        tag: Some(tag),
        landscape,
        fallback,
        numbered,
        kind: Kind::Own(f),
    }
}

/// Spare Construct's own sections, in the order the prototype registers them.
fn own_defs() -> Vec<Def> {
    let t = |s: &'static str| Some(s);
    vec![
        own("report-cover", "front", 1.0, None, None, "mixed", false, false, true, front::cover),
        own("report-contents", "front", 2.0, None, t("Contents"), "auto", false, false, false, front::contents),
        own("report-regulatory", "front", 3.0, None, t("Regulatory context"), "auto", false, false, true, front::regulatory),
        own("report-doc-control", "front", 4.0, None, t("Document control"), "author", false, false, true, front::doc_control),
        own("report-data-vintage", "front", 5.0, None, t("Data sources and vintage"), "auto", false, false, true, front::data_vintage),
        own("report-model-status", "front", 6.0, None, t("Model purpose and status"), "mixed", false, false, true, front::model_status),
        own("report-scope", "front", 7.0, None, t("Scope and proportionality"), "mixed", false, false, true, front::scope_section),
        own("report-limitations", "front", 8.0, None, t("Limitations"), "mixed", false, false, true, front::limitations),
        own("report-exec", "front", 9.0, None, t("Executive summary"), "mixed", false, false, true, front::exec),
        own("report-purpose", "1", 10.0, t("1.1"), t("Purpose and statutory context"), "author", false, false, true, env::purpose),
        own("report-reason", "1", 20.0, t("1.2"), t("Reason for consent"), "mixed", false, false, true, env::reason),
        own("report-engagement", "1", 30.0, t("1.3"), t("Scoping and engagement log"), "author", false, false, true, env::engagement),
        own("report-rr422", "1", 40.0, t("1.4"), t("ITA scope checklist"), "auto", false, false, true, env::rr422_section),
        own("report-locality", "2", 10.0, t("2.1"), t("Site and locality"), "mixed", false, false, true, env::locality),
        own("report-zoning", "2", 20.0, t("2.2"), t("Zoning"), "mixed", false, false, true, env::zoning),
        own("report-roads", "2", 30.0, t("2.3"), t("Road network"), "auto", false, false, true, env::roads),
        own("report-active", "2", 40.0, t("2.4"), t("Walking, cycling and public transport"), "mixed", false, false, true, env::active),
        own("report-volumes", "2", 50.0, t("2.5"), t("Traffic volumes"), "auto", false, false, true, env::volumes),
        own("report-speeds", "2", 60.0, t("2.6"), t("Speeds and existing performance"), "auto", true, false, true, env::speeds),
        own("report-crashes", "2", 70.0, t("2.7"), t("Crash history"), "mixed", true, false, true, crash::crashes),
        own("report-receiving", "2", 80.0, t("2.8"), t("Future receiving environment"), "auto", false, false, true, crash::receiving),
        own("report-landuse", "3", 10.0, t("3.1"), t("Land-use schedule"), "auto", false, false, true, prop::landuse),
        own("report-access", "3", 20.0, t("3.2"), t("Access"), "author", false, false, true, prop::access),
        own("report-accessibility", "3", 30.0, t("3.3"), t("Accessibility and parking"), "mixed", false, false, true, prop::accessibility),
        own("report-staging-plan", "3", 40.0, t("3.4"), t("Staging and sequencing"), "mixed", false, false, true, prop::staging_plan),
        own("report-construction", "3", 50.0, t("3.5"), t("Construction traffic"), "author", false, false, true, prop::construction),
        own("report-tripgen", "3", 60.0, t("3.6"), t("Trip generation, distribution and assignment"), "auto", true, false, true, prop::tripgen),
        own("report-scenarios", "4", 10.0, t("4.1"), t("Assessment scenarios"), "auto", false, false, true, assess::scenarios),
        own("report-modelling", "4", 20.0, t("4.2"), t("Model description and calibration summary"), "auto", false, false, true, assess::modelling),
        own("report-walk", "4", 30.0, t("4.2.1"), t("Walking, cycling and public transport levels of service"), "mixed", false, false, true, assess::walk),
        own("report-extent", "4", 38.0, t("4.2.2"), t("Extent of effects"), "auto", true, false, true, assess::extent),
        own("report-junctions", "4", 39.0, t("4.2.3"), t("Junction performance"), "auto", true, false, true, assess::junctions),
        own("report-safety", "4", 60.0, t("4.3"), t("Road user safety"), "mixed", true, false, true, assess::safety),
        own("report-alignment", "4", 70.0, t("4.4"), t("Network and policy alignment"), "mixed", false, false, true, assess::alignment),
        own("report-climate", "4", 75.0, t("4.5"), t("Climate adaptation"), "author", false, true, true, assess::climate),
        own("report-sensitivity", "4", 80.0, t("4.6"), t("Sensitivity tests"), "auto", false, false, true, assess::sensitivity),
        own("report-economics", "4", 84.0, t("4.7"), t("Indicative economics"), "auto", false, false, true, assess::economics),
        own("report-mitigation", "5", 10.0, t("5.1"), t("Mitigation register"), "mixed", true, false, true, end::mitigation),
        own("report-staging", "5", 20.0, t("5.2"), t("Staging triggers"), "auto", false, false, true, end::staging),
        own("report-conditions", "5", 30.0, t("5.3"), t("Conditions and monitoring"), "mixed", false, false, true, end::conditions),
        own("report-conclusion", "6", 10.0, t("6.1"), t("Conclusion"), "mixed", false, false, true, end::conclusion),
        own("report-app-data", "A", 5.0, None, t("Data sources, licences and basis"), "auto", false, false, true, app::app_data),
        own("report-app-settings", "A", 10.0, None, t("Model settings"), "auto", false, false, true, app::app_settings),
        own("report-app-rates", "A", 20.0, None, t("Trip rates"), "auto", false, false, true, app::app_rates),
        own("report-app-zones", "A", 30.0, None, t("Zones and gateways"), "auto", true, false, true, app::app_zones),
        own("report-app-network", "A", 40.0, None, t("Junction controls and network edits"), "auto", true, false, true, app::app_network),
        own("report-app-assumptions", "A", 50.0, None, t("Standards and assumptions register"), "auto", false, false, true, app::app_assumptions),
        own("report-app-calibration", "B", 10.0, None, t("Calibration and validation criteria"), "auto", false, false, true, app::app_calibration),
        own("report-app-convergence", "B", 20.0, None, t("Convergence"), "auto", false, false, true, app::app_convergence),
        own("report-app-counts", "B", 30.0, None, t("Count data and journey times"), "auto", false, false, true, app::app_counts),
        own("report-app-movements", "C", 10.0, None, t("Movement summaries by approach"), "auto", true, false, true, app::app_movements),
        own("report-app-crashes", "D", 10.0, None, t("Crash listing"), "auto", true, false, true, app::app_crashes),
        own("report-app-anop", "E", 10.0, None, t("ANOP tool inputs"), "auto", false, true, true, app::app_anop),
        own("report-app-datapack", "F", 10.0, None, t("Data pack manifest"), "auto", false, false, true, app::app_datapack),
        own("report-app-expert", "G", 10.0, None, t("Expert disclosure and sign-off"), "mixed", false, false, true, app::app_expert),
    ]
}

/// Sections printed inside one of the report's own (`MERGED`): left out of the document while their host is there.
const MERGED: [&str; 4] = ["compliance-scope", "compliance-model-status", "landuse-tripgen", "landuse-distribution"];

/// A section after numbering.
struct Numbered {
    /// Position in the definitions.
    d: usize,
    body: String,
    num: String,
    level: u8,
    merged: bool,
    kids: Option<Vec<usize>>,
    drop: bool,
}

fn depth_of(num: &str) -> usize {
    num.split('.').count()
}

fn same(a: Option<&str>, b: Option<&str>) -> bool {
    js_trim(a.unwrap_or("")).to_lowercase() == js_trim(b.unwrap_or("")).to_lowercase()
}

/// `numberChapter(ch, done)`: the sections of a chapter with their numbers and levels.
///
/// Numbers in chapters 1 to 6 mirror the AT ITA Guidelines 2025 v2: the report's own sections keep their fixed numbers. Another
/// feature's section is numbered as a sub-section of the section before it ('4.2.3.1'); with an order of 100 or more it becomes a
/// numbered section after the report's ('4.8'). A sub-section with the same title as its parent is printed under the parent's
/// heading. A placeholder (fallback) with sub-sections gives its number to the first of them. Appendices are numbered in order.
fn number_chapter(ch: &Chapter, defs: &[Def], done: &[(usize, String)]) -> Vec<Numbered> {
    let mut out: Vec<Numbered> = Vec::new();
    if ch.app || ch.num.is_empty() {
        let filled = done.iter().any(|(d, b)| !b.is_empty() && !defs[*d].fallback && !defs[*d].id.starts_with("report-"));
        let mut k = 0;
        for (d, b) in done {
            let x = &defs[*d];
            if b.is_empty() || (x.fallback && filled) {
                continue;
            }
            let numbered = !ch.num.is_empty() && x.title.as_deref().is_some_and(|t| !t.is_empty()) && x.numbered;
            let num = if numbered {
                k += 1;
                format!("{}{}{}", ch.num, if ch.app { "" } else { "." }, k)
            } else {
                String::new()
            };
            out.push(Numbered { d: *d, body: b.clone(), num, level: 2, merged: false, kids: None, drop: false });
        }
        return out;
    }
    let mut top = 0.0f64;
    for (d, b) in done {
        if let Some(at) = defs[*d].at
            && !b.is_empty()
        {
            let n = at.split('.').nth(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
            top = top.max(n);
        }
    }
    let mut parent: Option<usize> = None;
    for (d, b) in done {
        let x = &defs[*d];
        if b.is_empty() {
            continue;
        }
        if let Some(at) = x.at {
            out.push(Numbered {
                d: *d,
                body: b.clone(),
                num: at.to_string(),
                level: if depth_of(at) > 2 { 3 } else { 2 },
                merged: false,
                kids: Some(Vec::new()),
                drop: false,
            });
            parent = Some(out.len() - 1);
            continue;
        }
        if x.order >= 100.0 {
            parent = None;
            let num = if x.title.as_deref().is_some_and(|t| !t.is_empty()) {
                top += 1.0;
                format!("{}.{}", ch.num, top)
            } else {
                String::new()
            };
            out.push(Numbered { d: *d, body: b.clone(), num, level: 2, merged: false, kids: None, drop: false });
            continue;
        }
        let Some(p) = parent else {
            out.push(Numbered { d: *d, body: b.clone(), num: String::new(), level: 2, merged: false, kids: None, drop: false });
            continue;
        };
        let pd = &defs[out[p].d];
        let merged = same(x.title.as_deref(), pd.title.as_deref()) && !pd.fallback;
        out.push(Numbered { d: *d, body: b.clone(), num: String::new(), level: 3, merged, kids: None, drop: false });
        let idx = out.len() - 1;
        if let Some(k) = out[p].kids.as_mut() {
            k.push(idx);
        }
    }
    // a placeholder with sub-sections: the first sub-section takes its number, the others follow under it
    for p in 0..out.len() {
        let Some(kids) = out[p].kids.clone() else {
            continue;
        };
        let kids: Vec<usize> = kids.into_iter().filter(|k| !out[*k].merged).collect();
        let (pnum, plevel, pfallback) = (out[p].num.clone(), out[p].level, defs[out[p].d].fallback);
        if pfallback && !kids.is_empty() {
            out[p].drop = true;
            for (i, k) in kids.iter().enumerate() {
                out[*k].num = if i > 0 { format!("{pnum}.{i}") } else { pnum.clone() };
                out[*k].level = if i > 0 { 3 } else { plevel };
            }
            continue;
        }
        for (i, k) in kids.iter().enumerate() {
            out[*k].num = format!("{pnum}.{}", i + 1);
        }
    }
    out.into_iter().filter(|x| !x.drop).collect()
}

/// `body.replace(/^\s*<h[1-4][^>]*>\s*TITLE\s*<\/h[1-4]>/, '')`: the section's own heading leaves the body (the report prints its own).
pub(crate) fn strip_leading_title(body: &str, title: &str) -> String {
    let lit = esc(title);
    let a = {
        let mut i = 0;
        for c in body.chars() {
            if is_js_space(c) {
                i += c.len_utf8();
            } else {
                break;
            }
        }
        i
    };
    let b = body.as_bytes();
    if !(b.get(a) == Some(&b'<') && b.get(a + 1) == Some(&b'h') && b.get(a + 2).is_some_and(|c| (b'1'..=b'4').contains(c))) {
        return body.to_string();
    }
    let Some(gt) = body[a + 3..].find('>') else {
        return body.to_string();
    };
    let after_open = a + 3 + gt + 1;
    let ws_after = |from: usize| -> usize {
        let mut i = from;
        for c in body[from..].chars() {
            if is_js_space(c) {
                i += c.len_utf8();
            } else {
                break;
            }
        }
        i
    };
    let max_start = ws_after(after_open);
    // greedy whitespace, then the literal; back off through the whitespace for a literal that starts with some
    let mut start = max_start;
    loop {
        if body[start..].starts_with(&lit) {
            let end = start + lit.len();
            let max_end = ws_after(end);
            let mut e = max_end;
            loop {
                let rest = &body[e..];
                let rb = rest.as_bytes();
                if rest.starts_with("</h") && rb.get(3).is_some_and(|c| (b'1'..=b'4').contains(c)) && rb.get(4) == Some(&b'>') {
                    return body[e + 5..].to_string();
                }
                if e == end {
                    break;
                }
                e -= body[..e].chars().next_back().map_or(1, char::len_utf8);
            }
        }
        if start == after_open {
            break;
        }
        start -= body[..start].chars().next_back().map_or(1, char::len_utf8);
    }
    body.to_string()
}

/// `/^(appendix|annex)\s+[a-z]\s*[:·.–-]\s*/i` removed from a title.
fn shown_title(title: &str) -> String {
    let low = title.to_ascii_lowercase();
    let rest = if let Some(r) = low.strip_prefix("appendix") {
        Some(title.len() - r.len())
    } else {
        low.strip_prefix("annex").map(|r| title.len() - r.len())
    };
    let Some(mut i) = rest else {
        return title.to_string();
    };
    let ws = |s: &str, from: usize| -> usize {
        let mut j = from;
        for c in s[from..].chars() {
            if is_js_space(c) {
                j += c.len_utf8();
            } else {
                break;
            }
        }
        j
    };
    let j = ws(title, i);
    if j == i {
        return title.to_string();
    }
    i = j;
    let Some(c) = title[i..].chars().next().filter(char::is_ascii_alphabetic) else {
        return title.to_string();
    };
    i += c.len_utf8();
    i = ws(title, i);
    let Some(sep) = title[i..].chars().next().filter(|c| matches!(c, ':' | '\u{b7}' | '.' | '\u{2013}' | '-')) else {
        return title.to_string();
    };
    i += sep.len_utf8();
    i = ws(title, i);
    title[i..].to_string()
}

/// `/<th\b[^>]*>[\s\S]*?<\/th>/g` joined with a space: the table headings.
fn heads_of(body: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut from = 0;
    while let Some(i) = body[from..].find("<th") {
        let a = from + i;
        let after = body[a + 3..].chars().next();
        if after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            from = a + 3;
            continue;
        }
        let Some(gt) = body[a..].find('>') else {
            break;
        };
        let Some(close) = body[a + gt + 1..].find("</th>") else {
            break;
        };
        let end = a + gt + 1 + close + 5;
        out.push(&body[a..end]);
        from = end;
    }
    out.join(" ")
}

/// `body.replace(/%%TNO(?::([^%]*))?%%/g, ...)`: table numbers.
fn number_tables(body: &str, f: &mut dyn FnMut(Option<&str>) -> String) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(i) = rest.find("%%TNO") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 5..];
        if let Some(t) = tail.strip_prefix("%%") {
            out.push_str(&f(None));
            rest = t;
        } else if let Some(r) = tail.strip_prefix(':')
            && let Some(e) = r.find('%')
            && r[e..].starts_with("%%")
        {
            out.push_str(&f(Some(&r[..e])));
            rest = &r[e + 2..];
        } else {
            out.push_str("%%TNO");
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

impl<'a> Rep<'a> {
    /// `context(P, opts)`: the context every section is given.
    pub fn context<'r>(&'r self, opts: &BuildOpts) -> Ctx<'a, 'r> {
        let sel = pick(&self.tools());
        let periods: Vec<String> = match sel.with {
            Some(w) if !self.rep_periods(w).is_empty() => self.rep_periods(w).iter().map(|p| p.to_string()).collect(),
            _ => vec!["AM".to_string(), "PM".to_string()],
        };
        Ctx {
            r: self,
            sel,
            period: opts.period.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| self.st().period.get().cloned().unwrap_or_default()),
            periods,
            map_images: opts.map_images.clone(),
            requests: Vec::new(),
            chapter: ChapterState { id: "4".to_string(), num: "4".to_string() },
            section: SectionState::default(),
            counters: (0, 0),
            refs: Vec::new(),
            crash_cache: None,
        }
    }

    /// `Report.build(P, opts)`: the report body, the figures it asks for and its contents.
    pub fn build(&self, opts: &BuildOpts) -> Built {
        let mut ctx = self.context(opts);
        let foreign: Vec<ForeignSection> = self.host.foreign_sections();
        // the registry list: the junction tools' sections, the report's own, then the other features' (by order, then by arrival)
        let mut defs: Vec<Def> = Vec::new();
        for id in report_section_ids() {
            let Some(m) = report_section_meta(id) else {
                continue;
            };
            defs.push(Def {
                id: id.to_string(),
                chapter: m.chapter.to_string(),
                order: m.order,
                at: None,
                title: Some(if id == "junctions-performance" { perf_title() } else { m.title.to_string() }),
                tag: None,
                landscape: false,
                fallback: false,
                numbered: true,
                kind: Kind::Junction(id),
            });
        }
        defs.extend(own_defs());
        for (i, f) in foreign.iter().enumerate() {
            defs.push(Def {
                id: f.id.clone(),
                chapter: f.chapter.clone(),
                order: f.order,
                at: None,
                title: f.title.clone(),
                tag: None,
                landscape: false,
                fallback: false,
                numbered: true,
                kind: Kind::Foreign(i),
            });
        }
        let mut sorted: Vec<usize> = (0..defs.len()).collect();
        sorted.sort_by(|a, b| defs[*a].order.partial_cmp(&defs[*b].order).unwrap_or(std::cmp::Ordering::Equal));
        let all = chapters();
        let mut groups: HashMap<&str, Vec<usize>> = all.iter().map(|c| (c.id, Vec::new())).collect();
        for i in sorted {
            let d = &defs[i];
            if MERGED.contains(&d.id.as_str()) {
                continue;
            }
            let ch = chapter_of(&d.chapter, d.title.as_deref().unwrap_or(""));
            let key = all.iter().find(|c| c.id == ch).map_or("4", |c| c.id);
            groups.entry(key).or_default().push(i);
        }
        let mut chapter_html: Vec<(&Chapter, String)> = Vec::new();
        let mut toc: Vec<TocEntry> = Vec::new();
        for ch in &all {
            let mut list = groups.remove(ch.id).unwrap_or_default();
            if list.is_empty() {
                continue;
            }
            // chapters 1 to 6: on an equal order the report's own section comes first, so another feature's section follows it
            if !ch.num.is_empty() && !ch.app {
                list.sort_by(|a, b| {
                    defs[*a]
                        .order
                        .partial_cmp(&defs[*b].order)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| (defs[*b].at.is_some() as i32).cmp(&(defs[*a].at.is_some() as i32)))
                });
            }
            ctx.chapter = ChapterState { id: ch.id.to_string(), num: ch.num.to_string() };
            ctx.counters = (0, 0);
            let mut done: Vec<(usize, String)> = Vec::new();
            for &i in &list {
                let d = &defs[i];
                ctx.section = SectionState { id: d.id.clone(), num: String::new(), title: d.title.clone().unwrap_or_default() };
                let body = match d.kind {
                    Kind::Own(f) => f(&mut ctx),
                    Kind::Junction(id) => self
                        .tools()
                        .report_section(id, &ReportCtx { sel: Some(ctx.sel.clone()), scenarios: None })
                        .unwrap_or_default(),
                    Kind::Foreign(k) => foreign[k].html.clone(),
                };
                done.push((i, if js_trim(&body).is_empty() { String::new() } else { body }));
            }
            let mut parts: Vec<String> = Vec::new();
            for x in number_chapter(ch, &defs, &done) {
                let d = &defs[x.d];
                let is_own = matches!(d.kind, Kind::Own(_));
                let mut body = x.body.clone();
                if let Some(t) = d.title.as_deref().filter(|t| !t.is_empty()) {
                    body = strip_leading_title(&body, t);
                }
                // numbered tables and figures: the report's own come captioned, other features' are captioned here
                if !ch.num.is_empty() {
                    body = if is_own {
                        number_figcaptions(&body)
                    } else {
                        number_foreign(&body, d.title.as_deref().unwrap_or(""))
                    };
                }
                // provenance: a table from another feature without a source line says where it comes from
                if !is_own && body.contains("<table") && !has_source_line(&body) {
                    body.push_str(&format!(
                        "<p class=\"src\">Source: Spare Construct's {} feature, from this project's inputs, loaded data and model runs (data vintage in the front matter; inputs in Appendix A).</p>",
                        esc(d.id.split('-').next().unwrap_or(""))
                    ));
                }
                // every junction table carries D3, including tables from other features
                if body.contains("<table") && has_word(&body, "DoS") && has_word(&body, "LoS") && !body.contains("data-d=\"D3\"") {
                    body.push_str(&disc("D3"));
                }
                // D4 on every economic table, D5 on every ANOP table and D13 wherever a 'Spare Construct assumption' is used
                let heads = heads_of(&body);
                if has_word(&heads, "BCR") && !body.contains("data-d=\"D4\"") {
                    body.push_str(&disc("D4"));
                }
                if has_word(&heads, "ANOP") && !body.contains("data-d=\"D5\"") {
                    body.push_str(&disc("D5"));
                }
                if has_assumption(&body) && !body.contains("data-d=\"D13\"") {
                    body.push_str(&disc("D13"));
                }
                let pre = if ch.num.is_empty() { String::new() } else { format!("{}{}", ch.num, if ch.app { "" } else { "." }) };
                let has_num = !ch.num.is_empty();
                let mut tcount = ctx.counters.0;
                let mut fcount = ctx.counters.1;
                body = number_tables(&body, &mut |id| {
                    if has_num {
                        tcount += 1;
                        format!("Table {pre}{tcount}{}: ", match id {
                            Some(i) if !i.is_empty() => format!(" ({})", esc(i)),
                            _ => String::new(),
                        })
                    } else {
                        String::new()
                    }
                });
                body = body.replace("%%FNO%%", "\u{0}FNO\u{0}");
                {
                    let mut out = String::with_capacity(body.len());
                    for (k, seg) in body.split("\u{0}FNO\u{0}").enumerate() {
                        if k > 0 {
                            if has_num {
                                fcount += 1;
                                out.push_str(&format!("Figure {pre}{fcount}: "));
                            } else {
                                out.push_str("Figure: ");
                            }
                        }
                        out.push_str(seg);
                    }
                    body = out;
                }
                ctx.counters = (tcount, fcount);
                let tag = d.tag.map_or(String::new(), |t| format!("<span class=\"tag noprint\">{}</span>", esc(t)));
                let shown = match d.title.as_deref() {
                    Some(t) if ch.app && !t.is_empty() => shown_title(t),
                    Some(t) => t.to_string(),
                    None => String::new(),
                };
                let hl = if x.level == 3 { "h3" } else { "h2" };
                let head = if d.title.as_deref().is_some_and(|t| !t.is_empty()) && !x.merged {
                    format!(
                        "<{hl} class=\"sec-h{}\" id=\"s-{}\">{}{}{}</{hl}>",
                        if x.level == 3 { " sub" } else { "" },
                        esc(&d.id),
                        if x.num.is_empty() { String::new() } else { format!("<span class=\"no\">{}</span> ", esc(&x.num)) },
                        esc(&shown),
                        tag
                    )
                } else {
                    String::new()
                };
                if d.title.as_deref().is_some_and(|t| !t.is_empty()) && !x.merged && d.numbered && d.id != "report-contents" {
                    toc.push(TocEntry { ch: ch.id.to_string(), num: x.num.clone(), title: shown.clone(), id: d.id.clone(), level: x.level });
                }
                parts.push(format!(
                    "<section class=\"sec{}{}{}\" data-sec=\"{}\">{}{}</section>",
                    if d.landscape { " landscape" } else { "" },
                    if d.id == "report-cover" { " cover-sec" } else { "" },
                    if x.merged { " merged" } else { "" },
                    esc(&d.id),
                    head,
                    body
                ));
            }
            if parts.is_empty() {
                continue;
            }
            let title = if ch.id == "front" {
                String::new()
            } else {
                format!(
                    "<h1 class=\"ch\" id=\"ch-{}\">{}{} {}</h1>",
                    esc(ch.id),
                    if ch.app { if ch.id == "P" { "Annex " } else { "Appendix " } } else { "" },
                    esc(ch.num),
                    esc(ch.title)
                )
            };
            chapter_html.push((
                ch,
                format!(
                    "<div class=\"chapter{}\" data-ch=\"{}\">{}{}</div>",
                    if ch.id == "front" { " front" } else { "" },
                    esc(ch.id),
                    title,
                    parts.join("")
                ),
            ));
        }
        let contents = format!(
            "<ol class=\"toc\">{}</ol>",
            all.iter()
                .filter(|ch| ch.id != "front" && chapter_html.iter().any(|(c, _)| c.id == ch.id))
                .map(|ch| format!(
                    "<li><a href=\"#ch-{}\"><b>{}{} {}</b></a><ol>{}</ol></li>",
                    esc(ch.id),
                    if ch.app { if ch.id == "P" { "Annex " } else { "Appendix " } } else { "" },
                    esc(ch.num),
                    esc(ch.title),
                    toc.iter()
                        .filter(|t| t.ch == ch.id)
                        .map(|t| format!(
                            "<li{}><a href=\"#s-{}\">{} {}</a></li>",
                            if t.level == 3 { " class=\"sub\"" } else { "" },
                            esc(&t.id),
                            esc(&t.num),
                            esc(&t.title)
                        ))
                        .collect::<String>()
                ))
                .collect::<String>()
        );
        let joined: String = chapter_html.iter().map(|(_, h)| h.as_str()).collect();
        let body = fix_tables(&joined.replacen("%%CONTENTS%%", &contents, 1));
        Built {
            body,
            requests: ctx.requests.clone(),
            chapters: chapter_html.iter().map(|(c, _)| c.id.to_string()).collect(),
            toc,
            sel: ctx.sel.clone(),
        }
    }
}
