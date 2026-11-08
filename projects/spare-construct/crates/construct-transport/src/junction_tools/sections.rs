//! The junctions feature's report sections: the flow-scale test, the movement summaries by turn (Appendix C), the junction
//! comparison and the turning movement diagrams.

use super::figure::FigureOpts;
use super::j1::J1HtmlOpts;
use super::text::per_name;
use super::{FIGURE_STEP, FLOW_SCALE, FLOW_SCALE_FACTOR, LABEL, Sc, Tools, entries};
use crate::jsnum::{math_round, number_to_js_string, to_fixed};
use crate::report::Sel;
use crate::util::esc;

/// A report section the junctions feature registers: its id, chapter, order and title.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionMeta {
    /// Section id.
    pub id: &'static str,
    /// Chapter name or letter.
    pub chapter: &'static str,
    /// Order within the chapter.
    pub order: f64,
    /// Title.
    pub title: &'static str,
}

/// The flow-scale test's percentage over the flows (`PCT`).
pub fn pct() -> f64 {
    math_round((FLOW_SCALE_FACTOR - 1.0) * 100.0)
}

/// The title of the flow-scale section (`PERF_TITLE`).
pub fn perf_title() -> String {
    format!(
        "Junction flow-scale test (+{}%)",
        number_to_js_string(pct())
    )
}

/// The ids of the report sections this feature registers.
pub fn report_section_ids() -> [&'static str; 4] {
    [
        "junctions-performance",
        "junctions-movement-summary",
        "junctions-comparison",
        "junctions-turning",
    ]
}

/// The sections' chapter and order, as `Registry.report` holds them (the flow-scale title is [`perf_title`]).
pub fn report_section_meta(id: &str) -> Option<SectionMeta> {
    Some(match id {
        "junctions-performance" => SectionMeta {
            id: "junctions-performance",
            chapter: "Network performance",
            order: 40.0,
            title: "",
        },
        "junctions-movement-summary" => SectionMeta {
            id: "junctions-movement-summary",
            chapter: "C",
            order: 20.0,
            title: "Movement summaries by turn",
        },
        "junctions-comparison" => SectionMeta {
            id: "junctions-comparison",
            chapter: "Network performance",
            order: 42.0,
            title: "Junction comparison",
        },
        "junctions-turning" => SectionMeta {
            id: "junctions-turning",
            chapter: "Network performance",
            order: 43.0,
            title: "Turning movement diagrams",
        },
        _ => return None,
    })
}

/// What a report section is given besides the project: the scenarios being assessed, or an explicit list of scenarios.
#[derive(Clone, Debug, Default)]
pub struct ReportCtx {
    /// The assessed scenarios (`ctx.sel`).
    pub sel: Option<Sel>,
    /// Scenarios to cover instead (`ctx.scenarios`), by position.
    pub scenarios: Option<Vec<usize>>,
}

/// The scenarios a report section covers and the comparisons it makes: (from, to, heading, kind).
#[derive(Clone, Debug, PartialEq)]
pub struct ReportSelection {
    /// The scenarios that have been run, by position.
    pub list: Vec<usize>,
    /// The comparisons.
    pub pairs: Vec<(usize, usize, &'static str, &'static str)>,
}

impl<'a> Tools<'a> {
    /// `hasRun(s)`: the scenario has results.
    fn has_run(&self, i: Option<usize>) -> bool {
        i.is_some_and(|i| !self.sc(i).periods().is_empty())
    }

    /// `reportSel(P, ctx)`: the scenarios a report section covers: the report's assessed set (ctx.sel, else the report's
    /// choice: existing, without, with, mitigation), so a sensitivity or option run the author did not choose stays out.
    /// Comparisons run without to with and with to mitigation (existing to with when there is no same-year without).
    pub fn report_sel(&self, ctx: &ReportCtx) -> ReportSelection {
        let sel: Option<Sel> = if ctx.scenarios.is_some() {
            None
        } else {
            Some(ctx.sel.clone().unwrap_or_else(|| crate::report::pick(self)))
        };
        let Some(sel) = sel else {
            let all: Vec<usize> = match &ctx.scenarios {
                Some(l) => l.clone(),
                None => (0..self.project.scenarios.len()).collect(),
            };
            let list: Vec<usize> = all.into_iter().filter(|i| self.has_run(Some(*i))).collect();
            let b = list
                .iter()
                .copied()
                .find(|i| {
                    self.project.base.as_str() == Some(self.project.scenarios[*i].id.as_str())
                })
                .or_else(|| list.first().copied());
            let pairs = match b {
                Some(b) => list
                    .iter()
                    .copied()
                    .filter(|s| *s != b)
                    .map(|s| (b, s, "Change from the base scenario", "base"))
                    .collect(),
                None => Vec::new(),
            };
            return ReportSelection { list, pairs };
        };
        let mut list: Vec<usize> = Vec::new();
        for x in [sel.existing, sel.without, sel.with, sel.mitigation]
            .into_iter()
            .flatten()
        {
            if !list.contains(&x) && self.has_run(Some(x)) {
                list.push(x);
            }
        }
        let mut pairs = Vec::new();
        if self.has_run(sel.without) && self.has_run(sel.with) {
            pairs.push((
                sel.without.expect("run"),
                sel.with.expect("run"),
                "Effect of the development",
                "development",
            ));
        } else if self.has_run(sel.existing) && self.has_run(sel.with) && sel.existing != sel.with {
            pairs.push((
                sel.existing.expect("run"),
                sel.with.expect("run"),
                "Change from the existing network",
                "existing",
            ));
        }
        if self.has_run(sel.with) && self.has_run(sel.mitigation) {
            pairs.push((
                sel.with.expect("run"),
                sel.mitigation.expect("run"),
                "Effect of the mitigation",
                "mitigation",
            ));
        }
        ReportSelection { list, pairs }
    }

    /// `keyJunctions(sc, n)`: the junctions worth a full table: the extent-of-effects sites, else the worst by delay (at most n).
    pub fn key_junctions(&self, s: Sc<'a>, n: usize) -> Vec<String> {
        let periods = s.periods();
        let res = s
            .res_of("AM")
            .or_else(|| s.res_of("PM"))
            .or_else(|| periods.first().and_then(|p| s.res_of(p)));
        let Some(res) = res else {
            return Vec::new();
        };
        let ext: Option<Vec<String>> =
            s.sc.extent
                .get()
                .and_then(|e| e.get("sites"))
                .and_then(|x| x.as_array())
                .filter(|a| !a.is_empty())
                .map(|a| a.iter().map(super::js_str).collect());
        let es = entries(s.sc, res, false);
        let mut chosen: Vec<&super::Entry> = match &ext {
            Some(ext) => es
                .iter()
                .filter(|e| ext.contains(&e.id) || e.nodes.iter().any(|x| ext.contains(x)))
                .collect(),
            None => {
                let mut v: Vec<&super::Entry> = es.iter().collect();
                crate::graph::js_sort_by(&mut v, |a, b| {
                    or0(b.nr.map_or(0.0, |x| x.delay)) - or0(a.nr.map_or(0.0, |x| x.delay))
                });
                v
            }
        };
        chosen.truncate(n);
        chosen.into_iter().map(|e| e.id.clone()).collect()
    }

    /// `reportScs(P, ctx)`: the scenarios a section covers.
    fn report_scs(&self, ctx: &ReportCtx) -> Vec<usize> {
        self.report_sel(ctx).list
    }

    /// The report section with this id (`render(P, ctx)`): the HTML, or an empty text when there is nothing to print.
    pub fn report_section(&self, id: &str, ctx: &ReportCtx) -> Option<String> {
        Some(match id {
            "junctions-performance" => self.sec_performance(ctx),
            "junctions-movement-summary" => self.sec_movement_summary(ctx),
            "junctions-comparison" => self.sec_comparison(ctx),
            "junctions-turning" => self.sec_turning(ctx),
            _ => return None,
        })
    }

    fn sec_performance(&self, ctx: &ReportCtx) -> String {
        let scs = self.report_scs(ctx);
        if scs.is_empty() {
            return String::new();
        }
        let pct = number_to_js_string(pct());
        let mut scale = String::new();
        for &i in &scs {
            let s = self.sc(i);
            for p in s.periods() {
                let r = self.flow_scale_of(s, &p);
                if let Some(r) = r {
                    let crossing = r.crossing();
                    if !crossing.is_empty() {
                        scale.push_str(&format!(
                            "<li>{}, {}: {}</li>",
                            esc(s.name()),
                            esc(&per_name(&p)),
                            crossing
                                .iter()
                                .map(|x| esc(&format!(
                                    "{} ({})",
                                    x.name,
                                    x.first.as_ref().map_or("undefined", |f| f.where_.as_str())
                                )))
                                .collect::<Vec<_>>()
                                .join("; ")
                        ));
                    }
                }
            }
        }
        format!(
            "<h3>{}</h3>{}\n        {}\n        <p class=\"note\">{}: {}. Junctions are re-evaluated at flows \u{d7} {} with each run's signal timings; routes are not re-assigned. {}.</p>",
            esc(&perf_title()),
            self.supp_note(),
            if scale.is_empty() {
                format!(
                    "<p>No junction crosses its practical degree of saturation when flows rise by {pct}%.</p>"
                )
            } else {
                format!(
                    "<p>Junctions whose degree of saturation crosses the practical limit when flows rise by {pct}%:</p><ul>{scale}</ul>"
                )
            },
            esc(FLOW_SCALE.name),
            esc(FLOW_SCALE.edition),
            to_fixed(FLOW_SCALE_FACTOR, 2),
            LABEL
        )
    }

    fn sec_movement_summary(&self, ctx: &ReportCtx) -> String {
        let scs = self.report_scs(ctx);
        if scs.is_empty() {
            return String::new();
        }
        let mut body = String::new();
        for &i in &scs {
            let s = self.sc(i);
            for id in self.key_junctions(s, 8) {
                for p in s.periods() {
                    let Some(res) = s.res_of(&p) else {
                        continue;
                    };
                    let name = entries(s.sc, res, false)
                        .into_iter()
                        .find(|e| e.id == id)
                        .map(|e| e.name)
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| id.clone());
                    let d = self.j1(s.sc, res, &id, None);
                    body.push_str(&format!(
                        "<h4>{} \u{b7} {} \u{b7} {}</h4>{}",
                        esc(s.name()),
                        esc(&name),
                        esc(&per_name(&p)),
                        super::j1::j1_html(self, d.as_ref(), J1HtmlOpts { head: false })
                    ));
                }
            }
        }
        format!(
            "<h3>Movement summaries by turn</h3><p>SIDRA-style Movement Summary tables, turn by turn, for the key junctions (the extent-of-effects sites, or the most delayed). {}.</p>\n        {}",
            LABEL, body
        )
    }

    fn sec_comparison(&self, ctx: &ReportCtx) -> String {
        let pairs: Vec<_> = self
            .report_sel(ctx)
            .pairs
            .into_iter()
            .filter(|x| x.3 != "development")
            .collect();
        if pairs.is_empty() {
            return String::new();
        }
        let mut out = String::from("<h3>Junction comparison</h3>");
        for (a, b, what, _) in pairs {
            let (sa, sb) = (self.sc(a), self.sc(b));
            let html = self.j3_html(Some(sa), Some(sb), None);
            out.push_str(&format!(
                "<h4>{}: {} to {}</h4>{}",
                esc(what),
                esc(sa.name()),
                esc(sb.name()),
                strip_fly(&html)
            ));
        }
        out
    }

    fn sec_turning(&self, ctx: &ReportCtx) -> String {
        let scs = self.report_scs(ctx);
        if scs.is_empty() {
            return String::new();
        }
        let mut body = String::new();
        for &i in &scs {
            let s = self.sc(i);
            for id in self.key_junctions(s, 8) {
                body.push_str(&format!(
                    "<figure style=\"display:inline-block;margin:6px;break-inside:avoid\">{}<figcaption>{}</figcaption></figure>",
                    self.figure(s.sc, s.res_of("AM"), s.res_of("PM"), &id, FigureOpts::default()),
                    esc(s.name())
                ));
            }
        }
        format!(
            "<h3>Turning movement diagrams</h3><p>Modelled peak-hour turning volumes, AM (PM), rounded to the nearest {} veh/h. {}.</p>\n        {}",
            number_to_js_string(FIGURE_STEP),
            LABEL,
            body
        )
    }
}

/// `.replace(/ data-jx-fly="[^"]*"/g, '')`.
fn strip_fly(h: &str) -> String {
    let pat = " data-jx-fly=\"";
    let mut out = String::with_capacity(h.len());
    let mut rest = h;
    while let Some(i) = rest.find(pat) {
        out.push_str(&rest[..i]);
        let after = &rest[i + pat.len()..];
        match after.find('"') {
            Some(j) => rest = &after[j + 1..],
            None => {
                out.push_str(&rest[i..]);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

fn or0(x: f64) -> f64 {
    if x.is_nan() || x == 0.0 { 0.0 } else { x }
}
