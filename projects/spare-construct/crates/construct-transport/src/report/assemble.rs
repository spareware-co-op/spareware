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
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Write as _;

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

// number_chapter for an appendix or an unnumbered chapter: its sections in order, the titled ones
// numbered (an appendix's as A1, A2 ...), a fallback left out when another section is filled
fn number_flat(ch: &Chapter, defs: &[Def], done: &[(usize, String)]) -> Vec<Numbered> {
    let mut out: Vec<Numbered> = Vec::new();
    let filled = done.iter().any(|(d, b)| !b.is_empty() && defs.get(*d).is_some_and(|def| !def.fallback && !def.id.starts_with("report-")));
    let mut k: usize = 0;
    for (d, b) in done {
        let Some(x) = defs.get(*d) else {
            continue;
        };
        if b.is_empty() || (x.fallback && filled) {
            continue;
        }
        let numbered = !ch.num.is_empty() && x.title.as_deref().is_some_and(|t| !t.is_empty()) && x.numbered;
        let num = if numbered {
            // Bound: one per section of the chapter
            k = k.saturating_add(1);
            format!("{}{}{}", ch.num, if ch.app { "" } else { "." }, k)
        } else {
            String::new()
        };
        out.push(Numbered { d: *d, body: b.clone(), num, level: 2, merged: false, kids: None, drop: false });
    }
    out
}

/// `numberChapter(ch, done)`: the sections of a chapter with their numbers and levels.
///
/// Numbers in chapters 1 to 6 mirror the AT ITA Guidelines 2025 v2: the report's own sections keep their fixed numbers. Another
/// feature's section is numbered as a sub-section of the section before it ('4.2.3.1'); with an order of 100 or more it becomes a
/// numbered section after the report's ('4.8'). A sub-section with the same title as its parent is printed under the parent's
/// heading. A placeholder (fallback) with sub-sections gives its number to the first of them. Appendices are numbered in order.
fn number_chapter(ch: &Chapter, defs: &[Def], done: &[(usize, String)]) -> Vec<Numbered> {
    // Every position in done is one of the definitions, and every position kept in out is one of out's
    let mut out: Vec<Numbered> = Vec::new();
    if ch.app || ch.num.is_empty() {
        return number_flat(ch, defs, done);
    }
    let mut top = 0.0f64;
    for (d, b) in done {
        if let Some(at) = defs.get(*d).and_then(|def| def.at)
            && !b.is_empty()
        {
            let n = at.split('.').nth(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
            top = top.max(n);
        }
    }
    let mut parent: Option<usize> = None;
    for (d, b) in done {
        let Some(x) = defs.get(*d) else {
            continue;
        };
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
            parent = out.len().checked_sub(1);
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
        let pd = out.get(p).and_then(|row| defs.get(row.d));
        let merged = pd.is_some_and(|pd| same(x.title.as_deref(), pd.title.as_deref()) && !pd.fallback);
        out.push(Numbered { d: *d, body: b.clone(), num: String::new(), level: 3, merged, kids: None, drop: false });
        let idx = out.len().saturating_sub(1);
        if let Some(k) = out.get_mut(p).and_then(|row| row.kids.as_mut()) {
            k.push(idx);
        }
    }
    // a placeholder with sub-sections: the first sub-section takes its number, the others follow under it
    for p in 0..out.len() {
        let Some(row) = out.get(p) else {
            continue;
        };
        let Some(kids) = row.kids.clone() else {
            continue;
        };
        let (pnum, plevel, pfallback) = (row.num.clone(), row.level, defs.get(row.d).is_some_and(|def| def.fallback));
        let kids: Vec<usize> = kids.into_iter().filter(|kid| out.get(*kid).is_some_and(|sub| !sub.merged)).collect();
        if pfallback && !kids.is_empty() {
            if let Some(row) = out.get_mut(p) {
                row.drop = true;
            }
            for (i, kid) in kids.iter().enumerate() {
                if let Some(sub) = out.get_mut(*kid) {
                    sub.num = if i > 0 { format!("{pnum}.{i}") } else { pnum.clone() };
                    sub.level = if i > 0 { 3 } else { plevel };
                }
            }
            continue;
        }
        for (i, kid) in kids.iter().enumerate() {
            if let Some(sub) = out.get_mut(*kid) {
                // Bound: i counts sub-sections, far below usize::MAX
                sub.num = format!("{pnum}.{}", i.saturating_add(1));
            }
        }
    }
    out.into_iter().filter(|x| !x.drop).collect()
}

/// `body.replace(/^\s*<h[1-4][^>]*>\s*TITLE\s*<\/h[1-4]>/, '')`: the section's own heading leaves the body (the report prints its own).
pub(crate) fn strip_leading_title(body: &str, title: &str) -> String {
    let lit = esc(title);
    let a = ws_end(body, 0);
    let Some([b'<', b'h', b'1'..=b'4', ..]) = body.as_bytes().get(a..) else {
        return body.to_string();
    };
    // Bound: positions are within the body, so the sums stay far below usize::MAX; the three bytes matched are ASCII,
    // so a + 3 is a character boundary
    let open_from = a.saturating_add(3);
    let Some(gt) = body.get(open_from..).and_then(|s| s.find('>')) else {
        return body.to_string();
    };
    let after_open = open_from.saturating_add(gt).saturating_add(1);
    let max_start = ws_end(body, after_open);
    // greedy whitespace, then the literal; back off through the whitespace for a literal that starts with some
    let mut start = max_start;
    loop {
        if body.get(start..).is_some_and(|s| s.starts_with(&lit)) {
            let end = start.saturating_add(lit.len());
            let max_end = ws_end(body, end);
            let mut e = max_end;
            loop {
                let rest = body.get(e..).unwrap_or("");
                if let [b'<', b'/', b'h', b'1'..=b'4', b'>', ..] = rest.as_bytes() {
                    // the five bytes are ASCII, so what follows starts on a character boundary
                    return rest.get(5..).unwrap_or("").to_string();
                }
                if e == end {
                    break;
                }
                e = e.saturating_sub(prev_char_len(body, e));
            }
        }
        if start == after_open {
            break;
        }
        start = start.saturating_sub(prev_char_len(body, start));
    }
    body.to_string()
}

// The byte position after the JavaScript white space that starts at `from` (from itself when there is none)
fn ws_end(s: &str, from: usize) -> usize {
    let rest = s.get(from..).unwrap_or("");
    from.saturating_add(rest.len().saturating_sub(rest.trim_start_matches(is_js_space).len()))
}

// The length in bytes of the character that ends at `at` (1 when there is none)
fn prev_char_len(s: &str, at: usize) -> usize {
    s.get(..at).and_then(|head| head.chars().next_back()).map_or(1, char::len_utf8)
}

/// `/^(appendix|annex)\s+[a-z]\s*[:·.–-]\s*/i` removed from a title.
fn shown_title(title: &str) -> String {
    let low = title.to_ascii_lowercase();
    // ASCII lower-casing keeps every byte's place, so a position in low is the same position in title
    let rest = if let Some(r) = low.strip_prefix("appendix") {
        Some(title.len().saturating_sub(r.len()))
    } else {
        low.strip_prefix("annex").map(|r| title.len().saturating_sub(r.len()))
    };
    let Some(mut i) = rest else {
        return title.to_string();
    };
    let j = ws_end(title, i);
    if j == i {
        return title.to_string();
    }
    i = j;
    let Some(c) = title.get(i..).and_then(|s| s.chars().next()).filter(char::is_ascii_alphabetic) else {
        return title.to_string();
    };
    i = ws_end(title, i.saturating_add(c.len_utf8()));
    let Some(sep) = title.get(i..).and_then(|s| s.chars().next()).filter(|c| matches!(c, ':' | '\u{b7}' | '.' | '\u{2013}' | '-')) else {
        return title.to_string();
    };
    i = ws_end(title, i.saturating_add(sep.len_utf8()));
    // i is always at a character boundary here
    title.get(i..).unwrap_or(title).to_string()
}

/// `/<th\b[^>]*>[\s\S]*?<\/th>/g` joined with a space: the table headings.
fn heads_of(body: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut from = 0;
    // Bound: every position is within the body, so the sums stay far below usize::MAX; each match is ASCII, so every
    // position is a character boundary
    while let Some(i) = body.get(from..).and_then(|s| s.find("<th")) {
        let a = from.saturating_add(i);
        let after = body.get(a.saturating_add(3)..).and_then(|s| s.chars().next());
        if after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            from = a.saturating_add(3);
            continue;
        }
        let Some(gt) = body.get(a..).and_then(|s| s.find('>')) else {
            break;
        };
        let open_end = a.saturating_add(gt).saturating_add(1);
        let Some(close) = body.get(open_end..).and_then(|s| s.find("</th>")) else {
            break;
        };
        let end = open_end.saturating_add(close).saturating_add(5);
        if let Some(cell) = body.get(a..end) {
            out.push(cell);
        }
        from = end;
    }
    out.join(" ")
}

/// `body.replace(/%%TNO(?::([^%]*))?%%/g, ...)`: table numbers.
fn number_tables(body: &str, f: &mut dyn FnMut(Option<&str>) -> String) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some((before, tail)) = rest.split_once("%%TNO") {
        out.push_str(before);
        if let Some(after) = tail.strip_prefix("%%") {
            out.push_str(&f(None));
            rest = after;
        } else if let Some(r) = tail.strip_prefix(':')
            && let Some((id, after_pct)) = r.split_once('%')
            && let Some(after) = after_pct.strip_prefix('%')
        {
            // the id runs to the first '%', which must start "%%"
            out.push_str(&f(Some(id)));
            rest = after;
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
    #[must_use]
    pub fn context<'r>(&'r self, opts: &BuildOpts) -> Ctx<'a, 'r> {
        let sel = pick(&self.tools());
        let periods: Vec<String> = match sel.with {
            Some(w) if !self.rep_periods(w).is_empty() => self.rep_periods(w).iter().map(std::string::ToString::to_string).collect(),
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
    #[must_use]
    pub fn build(&self, opts: &BuildOpts) -> Built {
        let mut ctx = self.context(opts);
        let foreign: Vec<ForeignSection> = self.host.foreign_sections();
        let defs = registry_defs(&foreign);
        let all = chapters();
        let mut groups = chapter_groups(&defs, &all);
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
                    order_of(&defs, *a)
                        .partial_cmp(&order_of(&defs, *b))
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| i32::from(has_at(&defs, *b)).cmp(&i32::from(has_at(&defs, *a))))
                });
            }
            ctx.chapter = ChapterState { id: ch.id.to_string(), num: ch.num.to_string() };
            ctx.counters = (0, 0);
            let done = self.render_sections(&mut ctx, &defs, &list, &foreign);
            let mut parts: Vec<String> = Vec::new();
            for x in number_chapter(ch, &defs, &done) {
                // Every numbered section's position is one of the definitions
                let Some(d) = defs.get(x.d) else {
                    continue;
                };
                parts.push(section_html(&mut ctx.counters, ch, d, &x, &mut toc));
            }
            if parts.is_empty() {
                continue;
            }
            chapter_html.push((ch, chapter_div(ch, &parts)));
        }
        let contents = contents_html(&all, &chapter_html, &toc);
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

    // The HTML of each section in a chapter's list, in order: an empty body for a section with nothing to say
    fn render_sections(&self, ctx: &mut Ctx<'_, '_>, defs: &[Def], list: &[usize], foreign: &[ForeignSection]) -> Vec<(usize, String)> {
        let mut done: Vec<(usize, String)> = Vec::new();
        for &i in list {
            // The list holds positions in the definitions
            let Some(d) = defs.get(i) else {
                continue;
            };
            ctx.section = SectionState { id: d.id.clone(), num: String::new(), title: d.title.clone().unwrap_or_default() };
            let body = match d.kind {
                Kind::Own(f) => f(ctx),
                Kind::Junction(id) => self
                    .tools()
                    .report_section(id, &ReportCtx { sel: Some(ctx.sel.clone()), scenarios: None })
                    .unwrap_or_default(),
                // A foreign section's place is its position in the host's list
                Kind::Foreign(k) => foreign.get(k).map(|s| s.html.clone()).unwrap_or_default(),
            };
            done.push((i, if js_trim(&body).is_empty() { String::new() } else { body }));
        }
        done
    }
}

// A definition's order (NaN for a position that is not one, which never happens)
fn order_of(defs: &[Def], i: usize) -> f64 {
    defs.get(i).map_or(f64::NAN, |d| d.order)
}

// Whether a definition has a fixed number
fn has_at(defs: &[Def], i: usize) -> bool {
    defs.get(i).is_some_and(|d| d.at.is_some())
}

// The registry list: the junction tools' sections, the report's own, then the other features' (by order, then by arrival)
fn registry_defs(foreign: &[ForeignSection]) -> Vec<Def> {
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
    defs
}

// The definitions of each chapter, by order (a stable sort, so then by arrival); merged sections are left out
fn chapter_groups(defs: &[Def], all: &[&'static Chapter]) -> HashMap<&'static str, Vec<usize>> {
    let mut sorted: Vec<usize> = (0..defs.len()).collect();
    sorted.sort_by(|a, b| order_of(defs, *a).partial_cmp(&order_of(defs, *b)).unwrap_or(Ordering::Equal));
    let mut groups: HashMap<&'static str, Vec<usize>> = all.iter().map(|c| (c.id, Vec::new())).collect();
    for i in sorted {
        let Some(d) = defs.get(i) else {
            continue;
        };
        if MERGED.contains(&d.id.as_str()) {
            continue;
        }
        let ch = chapter_of(&d.chapter, d.title.as_deref().unwrap_or(""));
        let key = all.iter().find(|c| c.id == ch).map_or("4", |c| c.id);
        groups.entry(key).or_default().push(i);
    }
    groups
}

// A numbered section's HTML: its heading, its body with the disclaimers and numbered captions, and its contents entry
fn section_html(counters: &mut (u32, u32), ch: &Chapter, d: &Def, x: &Numbered, toc: &mut Vec<TocEntry>) -> String {
    let is_own = matches!(d.kind, Kind::Own(_));
    let mut body = x.body.clone();
    if let Some(title) = d.title.as_deref().filter(|title| !title.is_empty()) {
        body = strip_leading_title(&body, title);
    }
    // numbered tables and figures: the report's own come captioned, other features' are captioned here
    if !ch.num.is_empty() {
        body = if is_own {
            number_figcaptions(&body)
        } else {
            number_foreign(&body, d.title.as_deref().unwrap_or(""))
        };
    }
    body = with_disclaimers(body, d, is_own);
    body = number_captions(&body, ch, counters);
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
    format!(
        "<section class=\"sec{}{}{}\" data-sec=\"{}\">{}{}</section>",
        if d.landscape { " landscape" } else { "" },
        if d.id == "report-cover" { " cover-sec" } else { "" },
        if x.merged { " merged" } else { "" },
        esc(&d.id),
        head,
        body
    )
}

// The source line and disclaimers a section's tables must carry
fn with_disclaimers(mut body: String, d: &Def, is_own: bool) -> String {
    // provenance: a table from another feature without a source line says where it comes from
    if !is_own && body.contains("<table") && !has_source_line(&body) {
        // Writing to a String cannot fail
        let _ = write!(
            body,
            "<p class=\"src\">Source: Spare Construct's {} feature, from this project's inputs, loaded data and model runs (data vintage in the front matter; inputs in Appendix A).</p>",
            esc(d.id.split('-').next().unwrap_or(""))
        );
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
    body
}

// Table and figure numbers in a section's body, counting on from the chapter's counters
fn number_captions(body: &str, ch: &Chapter, counters: &mut (u32, u32)) -> String {
    let pre = if ch.num.is_empty() { String::new() } else { format!("{}{}", ch.num, if ch.app { "" } else { "." }) };
    let has_num = !ch.num.is_empty();
    let (mut tcount, mut fcount) = *counters;
    // Bound: the counters count a chapter's tables and figures, far below u32::MAX
    let body = number_tables(body, &mut |id| {
        if has_num {
            tcount = tcount.saturating_add(1);
            format!("Table {pre}{tcount}{}: ", match id {
                Some(i) if !i.is_empty() => format!(" ({})", esc(i)),
                _ => String::new(),
            })
        } else {
            String::new()
        }
    });
    let body = body.replace("%%FNO%%", "\u{0}FNO\u{0}");
    let mut out = String::with_capacity(body.len());
    for (k, seg) in body.split("\u{0}FNO\u{0}").enumerate() {
        if k > 0 {
            if has_num {
                fcount = fcount.saturating_add(1);
                // Writing to a String cannot fail
                let _ = write!(out, "Figure {pre}{fcount}: ");
            } else {
                out.push_str("Figure: ");
            }
        }
        out.push_str(seg);
    }
    *counters = (tcount, fcount);
    out
}

// A chapter's HTML: its heading (none for the front matter) and its sections
fn chapter_div(ch: &Chapter, parts: &[String]) -> String {
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
    format!(
        "<div class=\"chapter{}\" data-ch=\"{}\">{}{}</div>",
        if ch.id == "front" { " front" } else { "" },
        esc(ch.id),
        title,
        parts.join("")
    )
}

// The contents list: each printed chapter after the front matter, with its sections
fn contents_html(all: &[&Chapter], chapter_html: &[(&Chapter, String)], toc: &[TocEntry]) -> String {
    let mut items = String::new();
    for ch in all.iter().filter(|ch| ch.id != "front" && chapter_html.iter().any(|(c, _)| c.id == ch.id)) {
        let mut subs = String::new();
        for entry in toc.iter().filter(|entry| entry.ch == ch.id) {
            // Writing to a String cannot fail
            let _ = write!(
                subs,
                "<li{}><a href=\"#s-{}\">{} {}</a></li>",
                if entry.level == 3 { " class=\"sub\"" } else { "" },
                esc(&entry.id),
                esc(&entry.num),
                esc(&entry.title)
            );
        }
        // Writing to a String cannot fail
        let _ = write!(
            items,
            "<li><a href=\"#ch-{}\"><b>{}{} {}</b></a><ol>{}</ol></li>",
            esc(ch.id),
            if ch.app { if ch.id == "P" { "Annex " } else { "Appendix " } } else { "" },
            esc(ch.num),
            esc(ch.title),
            subs
        );
    }
    format!("<ol class=\"toc\">{items}</ol>")
}
