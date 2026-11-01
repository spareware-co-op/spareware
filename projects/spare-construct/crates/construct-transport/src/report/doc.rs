//! The document around the body: the page shell, the footer text, the style sheet and script, and the file names.

use super::Rep;
use super::build::{BuildOpts, Built};
use super::consts::{D1_SHORT, disclaimer_text};
use super::shared::model_version;
use super::texts::{CSP, CSS, DOC_SCRIPT};
use crate::results::iso_day_of_ms;
use crate::util::esc;

/// `cssStr(s)`: a CSS string literal. '<' and '>' become CSS escapes, so a scenario name such as '</style>' cannot close the
/// <style> element; control characters are dropped.
pub fn css_str(s: &str) -> String {
    let s = s.replace('\\', "\\\\").replace('"', "\\\"");
    // /[\r\n\f]+/g becomes a space, then the other control characters go
    let mut t = String::with_capacity(s.len());
    let mut in_run = false;
    for c in s.chars() {
        if matches!(c, '\r' | '\n' | '\u{c}') {
            if !in_run {
                t.push(' ');
                in_run = true;
            }
            continue;
        }
        in_run = false;
        t.push(c);
    }
    let t: String = t
        .chars()
        .filter(|c| !(('\u{0}'..='\u{1f}').contains(c) || *c == '\u{7f}'))
        .collect();
    let t = t
        .replace('<', "\\3C ")
        .replace('>', "\\3E ")
        .replace('&', "\\26 ");
    format!("\"{t}\"")
}

/// `new Date(ms).toISOString().slice(0, 16).replace('T', ' ')`: the UTC date and time to the minute.
pub fn iso_minute(ms: f64) -> String {
    let day = iso_day_of_ms(ms).unwrap_or_default();
    let t = ms.trunc().rem_euclid(86_400_000.0);
    let h = (t / 3_600_000.0).floor();
    let m = ((t - h * 3_600_000.0) / 60_000.0).floor();
    format!("{day} {:02}:{:02}", h as i64, m as i64)
}

impl<'a> Rep<'a> {
    /// `fileBase(P)`: the project's name as a file-name part.
    pub fn file_base(&self) -> String {
        let name = self.project.name.as_str().unwrap_or("");
        let name = if name.is_empty() { "project" } else { name };
        let kept: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
            .collect();
        let t = crate::jsnum::js_trim(&kept);
        let mut out = String::new();
        let mut in_ws = false;
        for c in t.chars() {
            if crate::jsnum::is_js_space(c) {
                if !in_ws {
                    out.push('-');
                    in_ws = true;
                }
            } else {
                in_ws = false;
                out.push(c.to_ascii_lowercase());
            }
        }
        if out.is_empty() {
            "project".to_string()
        } else {
            out
        }
    }

    /// `ymd()`: today's date as YYYYMMDD.
    pub fn ymd(&self) -> String {
        iso_day_of_ms(self.now_ms)
            .unwrap_or_default()
            .replace('-', "")
    }

    /// `reportName(P)`: the report's file name.
    pub fn report_name(&self) -> String {
        format!("{}-ITA-report-{}.html", self.file_base(), self.ymd())
    }

    /// `packName(P)`: the data pack's file name.
    pub fn pack_name(&self) -> String {
        format!("{}-ITA-datapack-{}.zip", self.file_base(), self.ymd())
    }

    /// `Report.html(P, opts)`: a complete standalone HTML document.
    pub fn html(&self, opts: &BuildOpts) -> String {
        self.doc(&self.build(opts))
    }

    /// `doc(P, b)`: the page around a built report body.
    pub fn doc(&self, b: &Built) -> String {
        let draft = !self.rs.signed_off();
        let runs: Vec<f64> = (0..self.project.scenarios.len())
            .flat_map(|i| {
                self.rep_periods(i)
                    .into_iter()
                    .filter_map(move |p| self.results[i].get(p).map(|r| r.ts))
                    .collect::<Vec<_>>()
            })
            .filter(|t| *t != 0.0 && !t.is_nan())
            .collect();
        let ts = if runs.is_empty() {
            None
        } else {
            Some(crate::jsnum::math_max_of(&runs))
        };
        let version = model_version(self);
        let foot = format!(
            "{} \u{b7} {} \u{b7} run {} \u{b7} {}{}",
            D1_SHORT,
            version,
            ts.map_or("not run".to_string(), iso_minute),
            b.sel
                .with
                .map_or(String::new(), |i| self.sc(i).name().to_string()),
            if draft { " \u{b7} DRAFT" } else { "" }
        );
        // the regulatory version stamp (pathway, regime and the editions relied on) goes in every page footer under D1
        let vs = self.host.stamp_footer(self.project);
        let why = self
            .host
            .export_block(self.project)
            .filter(|w| !w.is_empty());
        let foot_css = format!(
            "{}{}",
            css_str(&foot),
            if vs.is_empty() {
                String::new()
            } else {
                format!(" \"\\A\" {}", css_str(&vs))
            }
        );
        let bar = match &why {
            Some(w) => format!(
                "<button class=\"primary\" type=\"button\" disabled title=\"{0}\">Print or save as PDF</button><button type=\"button\" disabled title=\"{0}\">Download HTML</button><span class=\"warn\"><b>Printing and saving are blocked.</b> {0} Fix it in the Compliance+ tab (scope linter), then open the report again.</span>",
                esc(w)
            ),
            None => format!(
                "<button class=\"primary\" data-print type=\"button\">Print or save as PDF</button><button data-save type=\"button\">Download HTML</button><span class=\"muted\">{}In the print dialog choose A4 and turn on background graphics. Print from Chrome or Edge 131 or later: other browsers leave out the page footer that carries Disclaimer D1, the version stamp and the page numbers (both are also printed at the end of the report).</span>",
                if draft {
                    "DRAFT: a reviewer has not signed this report off yet. "
                } else {
                    ""
                }
            ),
        };
        let block_css = match &why {
            Some(w) => format!(
                "@media print{{main,.rp-watermark{{display:none!important}}body::before{{content:{};display:block;padding:20mm;font:12pt Arial,sans-serif}}}}",
                css_str(&format!("Printing is blocked: {w}"))
            ),
            None => String::new(),
        };
        format!(
            "<!doctype html>\n<html lang=\"en-NZ\"><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{name} \u{b7} Integrated Transport Assessment{draft_t}</title>\n<meta name=\"generator\" content=\"{gen}\"><style>{css}\n:root{{--foot-l:{foot_l}}}@page{{@bottom-left{{content:{foot_css};white-space:pre-wrap;font-size:6.5pt}}}}{block_css}</style></head>\n<body data-file=\"{file}\" data-draft=\"{d01}\"{blocked}>\n<div class=\"bar noprint\">{bar}</div>\n{wm}\n<main>{body}<footer class=\"doc-foot\"><p>{d1s}. {d1}</p>{vs_p}<p>{ver}{draft_f}</p></footer></main>\n<script>{script}</script></body></html>",
            csp = CSP,
            name = esc(self.name()),
            draft_t = if draft { " (DRAFT)" } else { "" },
            gen = esc(&version),
            css = CSS,
            foot_l = css_str(&foot),
            foot_css = foot_css,
            block_css = block_css,
            file = esc(&self.report_name()),
            d01 = if draft { 1 } else { 0 },
            blocked = if why.is_some() { " data-blocked=\"1\"" } else { "" },
            bar = bar,
            wm = if draft { "<div class=\"rp-watermark\" aria-hidden=\"true\">DRAFT</div>" } else { "" },
            body = b.body,
            d1s = esc(D1_SHORT),
            d1 = esc(&disclaimer_text("D1", None, None)),
            vs_p = if vs.is_empty() { String::new() } else { format!("<p>{}</p>", esc(&vs)) },
            ver = esc(&version),
            draft_f = if draft { " \u{b7} DRAFT" } else { "" },
            script = DOC_SCRIPT,
        )
    }
}
