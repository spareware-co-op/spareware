//! The integrated transport assessment report, written from the project and its results (port of packs/nz/report.js).
//!
//! The report is a document: [`Rep::html`] gives the same HTML the prototype prints or downloads for the same project, results and
//! date, with every section's numbers, tables, figure captions, numbering and text, and [`Rep::pack_files`] the files of the ITA
//! data pack (the prototype zips them; [`zip`] writes the same STORE-method ZIP without a library). Spare Construct's own sections are
//! here with the junction tools' four ([`crate::junction_tools`]); sections other features add, and what the other features answer
//! (compliance, land use, the data hub's crash analysis, registered exports) come through a [`ReportHost`].
//!
//! Where the JavaScript reads the store (`Store.P`, `sc.res`) a [`Rep`] holds the project, its JSON with the keys in file order
//! (the author's inputs and the datasets are read from there), each scenario's results and the host. The prototype's staging and
//! sensitivity runs, the Report tab and the checklist are interface and are not ported.

mod app_support;
mod assemble;
mod author;
mod build;
mod calc;
mod consts;
mod ctx;
mod doc;
mod host;
mod js;
mod pack;
mod pack_csv;
mod pack_readme;
mod rfmt;
mod sections_app;
mod sections_assess;
mod sections_crash;
mod sections_end;
mod sections_env;
mod sections_front;
mod sections_prop;
mod shared;
mod texts;
mod zip;

pub use author::Rs;
pub use build::{BuildOpts, Built, TocEntry, chapter_of, fix_tables, number_foreign};
pub use calc::*;
pub use consts::*;
pub use ctx::{
    ChapterState, Ctx, FigRequest, Head, MapImage, Row, SectionState, TableBody, TableOpts, Tbl,
    author_html, cells, disc, disc_with, fig_html, fill_figures, h, hn, note, para, tbl, tno_rest,
    tno_token,
};
pub use doc::{css_str, iso_minute};
pub use host::{
    AboutEntry, ExportOutcome, ForeignSection, LinkRiskInfo, NoHost, NodeRiskInfo, Regime,
    ReportHost,
};
pub use pack::{PackFile, PackOutput, csv_file, iso_full};
pub use sections_end::{Change, conclusion_text, network_diff};
pub use sections_env::CrashSite;
pub use texts::{COLS, CSP, CSS, DISCLAIMER_ITEMS, DISCLAIMERS_VERSION, DOC_SCRIPT, FILES};
pub use zip::{crc32, zip};

use crate::edit::json::JsonValue;
use crate::junction_tools::{Sc, Tools};
use crate::project::{Project, Scenario, Settings};
use crate::results::Results;
use crate::standards::year_of;

/// The scenarios a report assesses (`Report.pick`): by position in the project's scenario list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sel {
    /// With the proposal.
    pub with: Option<usize>,
    /// Without the proposal (same year as `with`), if there is one.
    pub without: Option<usize>,
    /// With the mitigation, if there is one.
    pub mitigation: Option<usize>,
    /// The existing (base year) network.
    pub existing: Option<usize>,
    /// The project's base scenario.
    pub base: Option<usize>,
}

/// `Report.pick(P)`: the scenarios the report assesses: the author's choice (`P.report.sel`), then the scenario roles, then the
/// base and active scenarios. With and without are compared only in the same year.
pub fn pick(t: &Tools) -> Sel {
    let scs = &t.project.scenarios;
    let st = &t.project.settings;
    let chosen = t
        .raw
        .and_then(|r| r.get("report"))
        .filter(|r| r.is_object())
        .and_then(|r| r.get("sel"))
        .filter(|s| s.is_object());
    let by_choice = |k: &str| -> Option<usize> {
        let id = chosen.and_then(|c| c.get(k)).and_then(|v| v.as_str())?;
        scs.iter().position(|s| s.id == id)
    };
    let by_id = |id: Option<&str>| -> Option<usize> {
        let id = id?;
        scs.iter().position(|s| s.id == id)
    };
    let role = |r: &str| -> Vec<usize> {
        scs.iter()
            .enumerate()
            .filter(|(_, s)| s.role.is(r))
            .map(|(i, _)| i)
            .collect()
    };
    let yr = |i: usize| -> f64 { year_of(Some(&scs[i]), st) };
    let latest = |list: Vec<usize>| -> Option<usize> {
        let mut l = list;
        crate::graph::js_sort_by(&mut l, |a, b| yr(*b) - yr(*a));
        l.first().copied()
    };
    let base = by_id(t.project.base.as_str()).or(if scs.is_empty() { None } else { Some(0) });
    let w = by_choice("with")
        .or_else(|| latest(role("proposal")))
        .or_else(|| {
            if t.project.active.as_str() != t.project.base.as_str() {
                by_id(t.project.active.as_str())
            } else {
                None
            }
        })
        .or_else(|| (0..scs.len()).find(|i| Some(*i) != base))
        .or(base);
    let wo = by_choice("without")
        .or_else(|| {
            let w = w?;
            role("baseline").into_iter().find(|i| yr(*i) == yr(w))
        })
        .or_else(|| {
            let (w, b) = (w?, base?);
            if w != b && yr(b) == yr(w) {
                Some(b)
            } else {
                None
            }
        });
    let m = by_choice("mitigation").or_else(|| {
        let w = w?;
        role("mitigation").into_iter().find(|i| yr(*i) == yr(w))
    });
    let ex = by_choice("existing")
        .or_else(|| role("existing").first().copied())
        .or(base);
    Sel {
        with: w,
        without: if wo == w { None } else { wo },
        mitigation: if m == w { None } else { m },
        existing: ex,
        base,
    }
}

/// The report's environment: the project, its JSON, each scenario's results, the other features and the clock.
pub struct Rep<'a> {
    /// The project.
    pub project: &'a Project,
    /// The project's JSON with its keys in file order: the author's inputs and the datasets are read from it.
    pub raw: &'a JsonValue,
    /// Each scenario's results, in the project's order.
    pub results: &'a [Results],
    /// The other features.
    pub host: &'a dyn ReportHost,
    /// The time the report is made (`Date.now()`), in milliseconds since 1970.
    pub now_ms: f64,
    /// The author's inputs (`rs(P)`).
    pub rs: Rs,
}

impl<'a> Rep<'a> {
    /// A report environment for a project, its JSON and results.
    pub fn new(
        project: &'a Project,
        raw: &'a JsonValue,
        results: &'a [Results],
        host: &'a dyn ReportHost,
        now_ms: f64,
    ) -> Rep<'a> {
        Rep {
            project,
            raw,
            results,
            host,
            now_ms,
            rs: Rs::read(raw),
        }
    }

    /// The junction tools over the same project, results and host.
    pub fn tools(&self) -> Tools<'a> {
        Tools::new(self.project, self.results, self.host).with_raw(self.raw)
    }

    /// Scenario i with its results.
    pub fn sc(&self, i: usize) -> Sc<'a> {
        self.tools().sc(i)
    }

    /// `P.settings` (`Store.S`).
    pub fn st(&self) -> &'a Settings {
        &self.project.settings
    }

    /// Scenario i.
    pub fn scenario(&self, i: usize) -> &'a Scenario {
        &self.project.scenarios[i]
    }

    /// The report's `periodsOf(sc)`: AM, PM, IP that have results (not the junction tools' order).
    pub fn rep_periods(&self, i: usize) -> Vec<&'static str> {
        PERIODS
            .iter()
            .copied()
            .filter(|p| self.results[i].get(p).is_some())
            .collect()
    }

    /// `yearOf(sc)`: `+Engine.yearOf(sc) || null`.
    pub fn year_of(&self, i: usize) -> Option<f64> {
        let y = year_of(Some(self.scenario(i)), self.st());
        if y == 0.0 || y.is_nan() {
            None
        } else {
            Some(y)
        }
    }

    /// `P.data`.
    pub fn data(&self) -> Option<&'a JsonValue> {
        js::get(Some(self.raw), "data")
    }

    /// `P.data.loaded`.
    pub fn loaded(&self) -> Option<&'a JsonValue> {
        js::get(self.data(), "loaded")
    }

    /// `P.name`.
    pub fn name(&self) -> &'a str {
        self.project.name.as_str().unwrap_or("")
    }
}
