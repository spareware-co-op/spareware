//! The report document helpers: tables, figures, disclaimers, the author's text and the context every section is given.

use super::consts::{D1_SHORT, attrib, disclaimer_text, disclaimer_title};
use super::js;
use super::{Rep, Sel};
use crate::edit::json::JsonValue;
use crate::jsnum::{is_js_space, math_max_of, number_to_js_string};
use crate::results::iso_day_of_ms;
use crate::util::esc;
use std::collections::HashMap;

/// A column heading of a report table.
#[derive(Clone, Debug, Default)]
pub struct Head {
    /// Heading HTML.
    pub label: String,
    /// Right-aligned numeric column.
    pub n: bool,
    /// Tooltip.
    pub title: Option<String>,
}

/// A text heading.
pub fn h(label: &str) -> Head {
    Head {
        label: label.to_string(),
        n: false,
        title: None,
    }
}

/// A numeric heading.
pub fn hn(label: &str) -> Head {
    Head {
        label: label.to_string(),
        n: true,
        title: None,
    }
}

/// A row of a report table: cells (HTML), an optional id and class, and the span of the last cell.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// Cells.
    pub cells: Vec<String>,
    /// Element id.
    pub id: Option<String>,
    /// Class.
    pub cls: Option<String>,
    /// Columns the last cell spans.
    pub span: Option<usize>,
}

impl Row {
    /// A plain row.
    pub fn new(cells: Vec<String>) -> Row {
        Row {
            cells,
            ..Row::default()
        }
    }
    /// A row with a class (empty for none).
    pub fn cls(cells: Vec<String>, cls: &str) -> Row {
        Row {
            cells,
            cls: if cls.is_empty() { None } else { Some(cls.to_string()) },
            ..Row::default()
        }
    }
    /// A row with an element id.
    pub fn id(cells: Vec<String>, id: &str) -> Row {
        Row {
            cells,
            id: Some(id.to_string()),
            ..Row::default()
        }
    }
    /// A row whose one cell spans the table.
    pub fn span(cell: String, span: usize) -> Row {
        Row {
            cells: vec![cell],
            span: Some(span),
            ..Row::default()
        }
    }
}

/// Plain cells from text.
pub fn cells<S: AsRef<str>>(v: &[S]) -> Vec<String> {
    v.iter().map(|s| s.as_ref().to_string()).collect()
}

/// A table: headings, rows, a class and the text for no rows.
#[derive(Clone, Debug, Default)]
pub struct Tbl {
    /// Headings.
    pub head: Vec<Head>,
    /// Rows.
    pub rows: Vec<Row>,
    /// Extra class.
    pub cls: String,
    /// Text for no rows ("None." when unset).
    pub empty: Option<String>,
}

impl Tbl {
    /// A table of headings and rows.
    pub fn new(head: Vec<Head>, rows: Vec<Row>) -> Tbl {
        Tbl {
            head,
            rows,
            cls: String::new(),
            empty: None,
        }
    }
    /// The same table with the text it prints for no rows.
    pub fn empty(mut self, text: &str) -> Tbl {
        self.empty = Some(text.to_string());
        self
    }
}

/// `tbl(o)`: a table with a <thead> (it repeats on every printed page).
pub fn tbl(o: &Tbl) -> String {
    let th = if o.head.is_empty() {
        String::new()
    } else {
        format!(
            "<thead><tr>{}</tr></thead>",
            o.head
                .iter()
                .map(|h| format!(
                    "<th{}{}>{}</th>",
                    if h.n { " class=\"n\"" } else { "" },
                    match &h.title {
                        Some(t) if !t.is_empty() => format!(" title=\"{}\"", esc(t)),
                        _ => String::new(),
                    },
                    h.label
                ))
                .collect::<String>()
        )
    };
    let body = if o.rows.is_empty() {
        format!(
            "<tr><td colspan=\"{}\" class=\"muted\">{}</td></tr>",
            o.head.len().max(1),
            o.empty.as_deref().unwrap_or("None.")
        )
    } else {
        o.rows
            .iter()
            .map(|r| {
                format!(
                    "<tr{}{}>{}</tr>",
                    match &r.id {
                        Some(i) if !i.is_empty() => format!(" id=\"{}\"", esc(i)),
                        _ => String::new(),
                    },
                    match &r.cls {
                        Some(c) if !c.is_empty() => format!(" class=\"{}\"", esc(c)),
                        _ => String::new(),
                    },
                    r.cells
                        .iter()
                        .enumerate()
                        .map(|(i, c)| format!(
                            "<td{}{}>{}</td>",
                            if o.head.get(i).is_some_and(|h| h.n) { " class=\"n\"" } else { "" },
                            match r.span {
                                Some(s) if s != 0 && i + 1 == r.cells.len() => format!(" colspan=\"{s}\""),
                                _ => String::new(),
                            },
                            c
                        ))
                        .collect::<String>()
                )
            })
            .collect::<String>()
    };
    format!(
        "<table class=\"rt{}\">{}<tbody>{}</tbody></table>",
        if o.cls.is_empty() { String::new() } else { format!(" {}", esc(&o.cls)) },
        th,
        body
    )
}

/// `disc(id, fill)`: a standing disclaimer.
pub fn disc(id: &str) -> String {
    disc_with(id, None, None)
}

/// `disc(id, { version, tools })`: a standing disclaimer with its fill-ins.
pub fn disc_with(id: &str, version: Option<&str>, tools: Option<&str>) -> String {
    format!(
        "<div class=\"disc\" data-d=\"{}\"><b>{} {}.</b> {}</div>",
        id,
        id,
        esc(disclaimer_title(id)),
        esc(&disclaimer_text(id, version, tools))
    )
}

/// `note(t, warn)`: a small grey (or amber) note; `t` is HTML.
pub fn note(t: &str, warn: bool) -> String {
    format!("<p class=\"note{}\">{}</p>", if warn { " warn" } else { "" }, t)
}

/// `para(t)`: a paragraph; `t` is HTML.
pub fn para(t: &str) -> String {
    format!("<p>{t}</p>")
}

/// The index of the end of the whitespace run of `s` that starts at byte `i`.
fn ws_end(s: &str, i: usize) -> usize {
    let mut j = i;
    for c in s[i..].chars() {
        if is_js_space(c) {
            j += c.len_utf8();
        } else {
            break;
        }
    }
    j
}

/// `text.split(/\n\s*\n/)`: blocks separated by a blank line (a newline, any whitespace, a newline).
fn split_blank_lines(t: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let b = t.as_bytes();
    while i < b.len() {
        if b[i] == b'\n' {
            let k = ws_end(t, i + 1);
            // the last newline inside the whitespace run that follows
            if let Some(rel) = t[i + 1..k].rfind('\n') {
                let last = i + 1 + rel;
                out.push(&t[start..i]);
                start = last + 1;
                i = start;
                continue;
            }
        }
        i += t[i..].chars().next().map_or(1, char::len_utf8);
    }
    out.push(&t[start..]);
    out
}

/// `/^\s*[-•]\s+/.test(l)` and its match length.
fn bullet_len(l: &str) -> Option<usize> {
    let a = ws_end(l, 0);
    let c = l[a..].chars().next()?;
    if c != '-' && c != '\u{2022}' {
        return None;
    }
    let b = a + c.len_utf8();
    let e = ws_end(l, b);
    if e > b { Some(e) } else { None }
}

/// `authorHtml(text)`: the author's text: blank lines make paragraphs, lines starting '- ' make bullets (everything escaped).
pub fn author_html(text: &str) -> String {
    let t = crate::jsnum::js_trim(text);
    split_blank_lines(t)
        .into_iter()
        .map(|block| {
            let lines: Vec<&str> = block.split('\n').collect();
            if lines.iter().all(|l| bullet_len(l).is_some()) {
                format!(
                    "<ul>{}</ul>",
                    lines
                        .iter()
                        .map(|l| format!("<li>{}</li>", esc(&l[bullet_len(l).unwrap_or(0)..])))
                        .collect::<String>()
                )
            } else {
                format!("<p>{}</p>", lines.iter().map(|l| esc(l)).collect::<Vec<_>>().join("<br>"))
            }
        })
        .collect()
}

/// A map image the report is given for a figure: a data URL, or an object with the image, its attribution and an error.
#[derive(Clone, Debug, PartialEq)]
pub enum MapImage {
    /// A data URL (or blob URL).
    Url(String),
    /// An object: the source, the attribution to print, and the reason the map could not be drawn.
    Object {
        /// Image source.
        src: Option<String>,
        /// Attribution.
        attribution: Option<String>,
        /// Error.
        error: Option<String>,
    },
}

/// A map figure a section asks for (`ctx.figure`).
#[derive(Clone, Debug, PartialEq)]
pub struct FigRequest {
    /// Figure id.
    pub id: String,
    /// Caption (HTML-free text; escaped when printed).
    pub title: String,
    /// Colour mode.
    pub mode: String,
    /// Scenario drawn.
    pub scenario_id: String,
    /// Scenario compared with (difference plots).
    pub base_scenario_id: Option<String>,
    /// Period drawn.
    pub period: Option<String>,
    /// Map bounds [[south, west], [north, east]], as the project holds them.
    pub bounds: Option<JsonValue>,
    /// Layers to show, in order.
    pub show: Vec<(String, bool)>,
    /// Source line.
    pub source: Option<String>,
}

impl FigRequest {
    /// As JSON, with the keys the prototype's request has (left out when unset).
    pub fn to_value(&self) -> serde_json::Value {
        use serde_json::{Map, Value, json};
        let mut m = Map::new();
        m.insert("id".into(), json!(self.id));
        m.insert("title".into(), json!(self.title));
        m.insert("mode".into(), json!(self.mode));
        m.insert("scenarioId".into(), json!(self.scenario_id));
        if let Some(b) = &self.base_scenario_id {
            m.insert("baseScenarioId".into(), json!(b));
        }
        if let Some(p) = &self.period {
            m.insert("period".into(), json!(p));
        }
        if let Some(b) = &self.bounds {
            m.insert("bounds".into(), b.to_value());
        }
        let mut s = Map::new();
        for (k, v) in &self.show {
            s.insert(k.clone(), json!(v));
        }
        m.insert("show".into(), Value::Object(s));
        if let Some(src) = &self.source {
            m.insert("source".into(), json!(src));
        }
        Value::Object(m)
    }
}

/// `figHtml(f, img, tno)`: one figure: the drawn map, or a placeholder until it is drawn.
pub fn fig_html(f: &FigRequest, img: Option<&MapImage>, tno: &str) -> String {
    let src = match img {
        Some(MapImage::Url(u)) => Some(u.as_str()),
        Some(MapImage::Object { src, .. }) => src.as_deref(),
        None => None,
    }
    .filter(|s| !s.is_empty());
    let ok_src = src.filter(|s| {
        s.starts_with("blob:")
            || ["png", "jpeg", "webp"]
                .iter()
                .any(|k| s.starts_with(&format!("data:image/{k};base64,")))
    });
    let (attribution, error) = match img {
        Some(MapImage::Object { attribution, error, .. }) => (attribution.as_deref(), error.as_deref()),
        _ => (None, None),
    };
    let attr = [attribution, f.source.as_deref()]
        .iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" \u{b7} ");
    let attr = if attr.is_empty() { attrib::OSM.to_string() } else { attr };
    let body = match ok_src {
        Some(s) => format!("<img src=\"{}\" alt=\"{}\">", esc(s), esc(&f.title)),
        None => format!(
            "<div class=\"fig-ph\">{}</div>",
            match error.filter(|e| !e.is_empty()) {
                Some(e) => format!("This map could not be drawn: {}.", esc(e)),
                None => "Map figure. It is drawn when you open the report from Spare Construct (Report \u{203a} Open the report), with scale bar, north arrow, legend and attribution.".to_string(),
            }
        ),
    };
    format!(
        "<figure class=\"fig\" id=\"{}\">{}<figcaption><span class=\"tno\">{}</span>{}</figcaption><p class=\"src\">{}</p></figure>",
        esc(&format!("f-{}", f.id)),
        body,
        tno,
        esc(&f.title),
        esc(&attr)
    )
}

/// `fillFigures(body, requests, images)`: puts drawn maps into a built report body (figures keep their numbers).
pub fn fill_figures(body: &str, requests: &[FigRequest], images: &HashMap<String, MapImage>) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    let open = "<figure class=\"fig\" id=\"";
    while let Some(i) = rest.find(open) {
        out.push_str(&rest[..i]);
        let after = &rest[i + open.len()..];
        let (Some(q), Some(end)) = (after.find('"'), after.find("</figure>")) else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        };
        // the figure element runs to the first </figure> after the id attribute, as the prototype's lazy pattern reads it
        let fid = &after[..q];
        let whole = &rest[i..i + open.len() + end + "</figure>".len()];
        let req = requests.iter().find(|f| esc(&format!("f-{}", f.id)) == fid);
        match req.and_then(|f| images.get(&f.id).map(|im| (f, im))) {
            Some((f, im)) => {
                let tno = whole
                    .find("<span class=\"tno\">")
                    .and_then(|a| {
                        let s = &whole[a + "<span class=\"tno\">".len()..];
                        s.find('<').map(|b| s[..b].to_string())
                    })
                    .unwrap_or_default();
                out.push_str(&fig_html(f, Some(im), &tno));
            }
            None => out.push_str(whole),
        }
        rest = &rest[i + open.len() + end + "</figure>".len()..];
    }
    out.push_str(rest);
    out
}

/// The table token a title carries: `Table E1 · Road network` has id E1 and the rest 'Road network'.
fn table_id(title: &str) -> Option<(String, usize)> {
    let t = title;
    let mut i = ws_end(t, 0);
    if !t[i..].starts_with("Table") {
        return None;
    }
    i += 5;
    let j = ws_end(t, i);
    if j == i {
        return None;
    }
    i = j;
    let mut chars = t[i..].char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    let mut k = i + 1;
    for (off, c) in t[i + 1..].char_indices() {
        if c.is_ascii_alphanumeric() || c == '-' {
            k = i + 1 + off + 1;
        } else {
            break;
        }
    }
    let id = t[i..k].to_string();
    let mut m = ws_end(t, k);
    if !t[m..].starts_with('\u{b7}') {
        return None;
    }
    m += '\u{b7}'.len_utf8();
    m = ws_end(t, m);
    Some((id, m))
}

/// `tnoToken(title)`: the placeholder for a table's number.
pub fn tno_token(title: &str) -> String {
    match table_id(title) {
        Some((id, _)) => format!("%%TNO:{id}%%"),
        None => "%%TNO%%".to_string(),
    }
}

/// `tnoRest(title)`: the title without its 'Table X · ' lead.
pub fn tno_rest(title: &str) -> String {
    match table_id(title) {
        Some((_, end)) => title[end..].to_string(),
        None => title.to_string(),
    }
}

/// How a table's body is given: built HTML, or a table to build.
pub enum TableBody {
    /// HTML.
    Html(String),
    /// Headings and rows.
    Tbl(Tbl),
}

impl From<Tbl> for TableBody {
    fn from(t: Tbl) -> TableBody {
        TableBody::Tbl(t)
    }
}

impl From<String> for TableBody {
    fn from(t: String) -> TableBody {
        TableBody::Html(t)
    }
}

/// What `ctx.table` is given besides the title and the body.
#[derive(Clone, Debug, Default)]
pub struct TableOpts {
    /// The source line (HTML); the section's own when unset.
    pub source: Option<String>,
    /// A note under the table (HTML).
    pub note: Option<String>,
    /// A junction table (carries the junction label and D3).
    pub junction: bool,
    /// The level of service label that follows the junction label.
    pub los_label: Option<String>,
    /// E27 tier 1 marker: None for the default (junction tables carry it), Some(false) to leave it off, Some(true) to force it.
    pub sup: Option<bool>,
    /// Disclaimers printed under it.
    pub d: Vec<&'static str>,
    /// Element id.
    pub id: Option<String>,
}

impl TableOpts {
    /// Options with a note.
    pub fn note(n: &str) -> TableOpts {
        TableOpts {
            note: Some(n.to_string()),
            ..TableOpts::default()
        }
    }
    /// Options with a source and a note.
    pub fn src_note(src: &str, n: &str) -> TableOpts {
        TableOpts {
            source: Some(src.to_string()),
            note: Some(n.to_string()),
            ..TableOpts::default()
        }
    }
}

/// The chapter being written.
#[derive(Clone, Debug, Default)]
pub struct ChapterState {
    /// Chapter id.
    pub id: String,
    /// Chapter number or letter.
    pub num: String,
}

/// The section being written.
#[derive(Clone, Debug, Default)]
pub struct SectionState {
    /// Section id.
    pub id: String,
    /// Section number.
    pub num: String,
    /// Section title.
    pub title: String,
}

/// The context every section is given: the assessed scenarios, the numbering counters and the helpers that print tables and figures.
pub struct Ctx<'a, 'r> {
    /// The report's environment.
    pub r: &'r Rep<'a>,
    /// The scenarios being assessed.
    pub sel: Sel,
    /// The period the economics use.
    pub period: String,
    /// The periods of the assessed scenario.
    pub periods: Vec<String>,
    /// Maps already drawn, by figure id.
    pub map_images: HashMap<String, MapImage>,
    /// Figures the sections asked for.
    pub requests: Vec<FigRequest>,
    /// The chapter being written.
    pub chapter: ChapterState,
    /// The section being written.
    pub section: SectionState,
    /// Table and figure counters of the chapter.
    pub counters: (u32, u32),
    /// Junction references by scenario id: node id to 'J1'.
    pub refs: Vec<(String, Vec<(String, String)>)>,
    /// The crash sites, worked out once per build.
    pub(super) crash_cache: Option<Vec<super::sections_env::CrashSite>>,
}

impl<'a, 'r> Ctx<'a, 'r> {
    /// `ctx.table(title, body, opts)`: a numbered, sourced table.
    pub fn table(&self, title: &str, body: impl Into<TableBody>, o: &TableOpts) -> String {
        let t = match body.into() {
            TableBody::Html(s) => s,
            TableBody::Tbl(b) => tbl(&b),
        };
        let src = match &o.source {
            Some(s) if !s.is_empty() => s.clone(),
            _ => self.source_of(),
        };
        let jl = if o.junction {
            format!(
                "<p class=\"src\">{}. {}</p>{}",
                esc(crate::junction_tools::LABEL),
                o.los_label.as_deref().map_or(String::new(), esc),
                disc("D3")
            )
        } else {
            String::new()
        };
        // E27 tier 1: junction results are outside the matters of discretion, so their tables are marked (Compliance.supplementary)
        let sup = if (o.junction && o.sup != Some(false)) || o.sup == Some(true) {
            self.r.tools().supp_text()
        } else {
            String::new()
        };
        format!(
            "<figure class=\"tbl\"{}{}><figcaption><span class=\"tno\">{}</span>{}<button class=\"noprint copy\" data-copy type=\"button\">Copy table</button></figcaption>{}{}",
            if o.junction { " data-junction=\"1\"" } else { "" },
            match &o.id {
                Some(i) if !i.is_empty() => format!(" id=\"{}\"", esc(i)),
                _ => String::new(),
            },
            tno_token(title),
            tno_rest(title),
            if sup.is_empty() { String::new() } else { format!("<p class=\"sup\">{} (AUP E27 tier 1).</p>", esc(&sup)) },
            t
        ) + &format!(
            "<p class=\"src\">Source: {}</p>{}{}{}</figure>",
            src,
            match &o.note {
                Some(n) if !n.is_empty() => format!("<p class=\"src\">{n}</p>"),
                _ => String::new(),
            },
            jl,
            o.d.iter().map(|id| disc(id)).collect::<String>()
        )
    }

    /// `ctx.figure(f)`: the drawn map, or a placeholder until the report is opened.
    pub fn figure(&mut self, f: FigRequest) -> String {
        let html = fig_html(&f, self.map_images.get(&f.id), "%%FNO%%");
        self.requests.push(f);
        html
    }

    /// `ctx.author(id, prompt)`: the author's own text for a box; an empty box prints the prompt as 'to be completed'.
    pub fn author(&self, id: &str, prompt: &str) -> String {
        match self.r.rs.text_of(id) {
            Some(t) if t.truthy() && !crate::jsnum::js_trim(&t.js_string()).is_empty() => {
                format!("<div class=\"author\">{}</div>", author_html(&t.js_string()))
            }
            _ => format!("<div class=\"author empty\"><b>Author to complete.</b> {prompt}</div>"),
        }
    }

    /// `runText(...scs)`: the scenarios with their year and run date, as escaped HTML.
    pub fn run_text(&self, scs: &[Option<usize>]) -> String {
        scs.iter()
            .flatten()
            .map(|&i| {
                let s = self.r.sc(i);
                let ts: Vec<f64> = self
                    .r
                    .rep_periods(i)
                    .iter()
                    .filter_map(|p| s.res_of(p).map(|r| r.ts))
                    .filter(|t| *t != 0.0 && !t.is_nan())
                    .collect();
                let year = self.r.year_of(i);
                format!(
                    "{} ({}{})",
                    esc(s.name()),
                    year.map_or("\u{2013}".to_string(), number_to_js_string),
                    if ts.is_empty() {
                        ", not run".to_string()
                    } else {
                        format!(", run {}", esc(&iso_day_of_ms(math_max_of(&ts)).unwrap_or_default()))
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// `sourceOf(c)`: the provenance of a table that names no source: the section's own, with the scenarios and run dates it reads.
    pub fn source_of(&self) -> String {
        let s = &self.sel;
        let id = self.section.id.as_str();
        let model = |xs: &[Option<usize>]| {
            let t = self.run_text(xs);
            format!(
                "Spare Construct model, {}; estimated, not observed",
                if t.is_empty() { "no scenario".to_string() } else { t }
            )
        };
        let user = "Author entries in the Report tab (user)";
        let settings = "Project settings (settings.json in the data pack); Spare Construct's indicative defaults where unchanged";
        let ex = s.existing.or(s.base);
        let rs = &self.r.rs;
        match id {
            "report-regulatory" => "Pathway and lodgement date: user entries (Compliance or Report tab). Regime, versions and authorities: Spare Construct's regulatory data as at 8 October 2026 (Planning Act 2026 Sch 1 cl 18\u{2013}21; LGACA 2009 s47, s47C)".to_string(),
            "report-doc-control" | "report-engagement" | "report-app-expert" => user.to_string(),
            "report-data-vintage" => "Spare Construct's download log for this project: the query date and record count kept when each dataset was loaded".to_string(),
            "report-model-status" | "report-modelling" | "report-app-calibration" => format!(
                "Calibrate.report against NZTA TMDG 2019 for {}; derived from the loaded counts and the model",
                self.run_text(&[ex])
            ),
            "report-active" => format!(
                "{} (cycle facility tags); level of traffic stress and ANOP mapping: Spare Construct (estimated), {}",
                esc(attrib::OSM),
                self.run_text(&[ex])
            ),
            "report-speeds" => format!("{}. Speed limits: NZTA NSLR where loaded, else the coded speed", model(&[ex])),
            "report-crashes" => esc(attrib::CAS),
            "report-receiving" => "Project register (user); each row's source and date as listed".to_string(),
            "report-tripgen" | "report-app-movements" | "report-junctions" | "report-extent" => model(&[s.without, s.with, s.mitigation]),
            "report-scenarios" => "Project scenarios (user): years, roles, land use and network as set in Scenarios".to_string(),
            "report-walk" => format!(
                "Crossings entered by the author in the Report tab (user); delay by the formulas below ({}, estimated)",
                esc(super::consts::ped::EDITION)
            ),
            "report-sensitivity" => format!(
                "Spare Construct sensitivity pack (Engine.sensitivity), {} run {}; estimated",
                esc(&js::text(js::get(rs.sens.as_ref(), "scName"))),
                esc(&day_of(js::get(rs.sens.as_ref(), "at")))
            ),
            "report-economics" => format!(
                "Engine.econ, NZTA MBCM v1.7.5 conventions (indicative screening); {}",
                model(&[s.with, s.mitigation])
            ),
            "report-mitigation" => {
                let t = self.run_text(&[s.mitigation]);
                format!(
                    "Network comparison of {} with the proposal (Spare Construct); costs: Spare Construct indicative rates; other columns: {}",
                    if t.is_empty() { "the mitigation scenario".to_string() } else { t },
                    user.to_lowercase()
                )
            }
            "report-staging" => format!(
                "Spare Construct staging test (Report tab), {} run {}; estimated",
                esc(&js::text(js::get(rs.staging.as_ref(), "scName"))),
                esc(&day_of(js::get(rs.staging.as_ref(), "at")))
            ),
            "report-app-settings" | "report-app-rates" => settings.to_string(),
            "report-app-zones" => "Project zones and gateways (user; existing land use where tagged; gateway flows from counts where the count basis says so, otherwise Spare Construct estimates)".to_string(),
            "report-app-network" => format!("Project network: {} import and user edits", esc(attrib::OSM)),
            "report-app-assumptions" => "Spare Construct's standards data; each item's source and edition as listed".to_string(),
            "report-app-counts" => format!(
                "{}; {}; journey times: Google Routes typical times as stored with the project (predicted, not observed); modelled: Spare Construct (estimated)",
                esc(attrib::NZTA),
                esc(attrib::AT)
            ),
            "report-app-datapack" => "Spare Construct's data pack column dictionary".to_string(),
            _ => format!(
                "Spare Construct, from this project's inputs and model runs ({})",
                {
                    let t = self.run_text(&[s.with]);
                    if t.is_empty() { "not run".to_string() } else { t }
                }
            ),
        }
    }
}

/// `day(t)` for a loose stored time: the date as YYYY-MM-DD, or empty.
pub(crate) fn day_of(t: Option<&JsonValue>) -> String {
    match t {
        None | Some(JsonValue::Null) => String::new(),
        Some(v) if !v.truthy() => String::new(),
        Some(v) => crate::results::js_iso_day(&v.to_value()).unwrap_or_default(),
    }
}

/// The footer line the page boxes print under Disclaimer D1.
pub fn footer_d1() -> &'static str {
    D1_SHORT
}
