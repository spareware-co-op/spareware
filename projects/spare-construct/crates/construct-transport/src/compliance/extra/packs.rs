//! Hand-over packs (compliance-extra.js section 19): each CSV carries a manifest, the version stamp and the hand-over disclaimer D12.
//!
//! Also the small number and table helpers the other parts of this file group share (`r1`, `r3`, [`n0`], [`t_table`]). The packs themselves
//! are in the submodules [`tsdg`], [`ssa`] and [`sheets`].

use super::anop::{anop, anop_corridors, corridor_speed};
use super::ctx::{disc, pair, periods_of, res_of, stamp, stamped, today};
use crate::compliance::data::x as xdata;
use crate::compliance::html::{ReportCtx, Th, Tr, kit_table, th, tr};
use crate::compliance::jv::{Jx, NULL, V, csv};
use crate::jsnum::{math_round, truthy_number};

pub mod sheets;
pub mod ssa;
pub mod tsdg;

pub use sheets::{handover_manifest, mbcm_pack, msm_pack, rltp_sheet, sh_pack};
pub use ssa::ssa_pack;
pub use tsdg::{run_design_life, tsdg_pack};

/// `+x || 0`: the number a value reads as, 0 for NaN and 0.
pub fn n0(v: &V) -> f64 {
    let x = v.n();
    if truthy_number(x) { x } else { 0.0 }
}

/// `+x || d`: the number a value reads as, d for NaN and 0.
pub fn or_n(v: &V, d: f64) -> f64 {
    let x = v.n();
    if truthy_number(x) { x } else { d }
}

/// `v || ''` for a loose value.
pub fn or_blank(v: &V) -> V {
    if v.t() { v.clone() } else { V::String(String::new()) }
}

/// `r1(v)`: a value rounded to 1 decimal, an empty cell when it is missing or not finite.
pub fn r1(v: &V) -> V {
    if v.nullish() || !v.n().is_finite() { V::String(String::new()) } else { V::Number(math_round(v.n() * 10.0) / 10.0) }
}

/// `r3(v)`: a value rounded to 3 decimals, an empty cell when it is missing or not finite.
pub fn r3(v: &V) -> V {
    if v.nullish() || !v.n().is_finite() { V::String(String::new()) } else { V::Number(math_round(v.n() * 1000.0) / 1000.0) }
}

/// `r1(x)` for a number.
pub fn r1f(x: f64) -> V {
    r1(&V::Number(x))
}

/// `r3(x)` for a number.
pub fn r3f(x: f64) -> V {
    r3(&V::Number(x))
}

/// A text cell.
pub fn sv(x: &str) -> V {
    V::String(x.to_string())
}

/// `T({ head, rows, empty })` (`Kit.table` with the heading text as given and compact rows).
pub fn t_table(head: &[&str], rows: Vec<Vec<String>>, empty: &str) -> String {
    let h: Vec<Th> = head.iter().map(|x| th(x)).collect();
    let r: Vec<Tr> = rows.into_iter().map(tr).collect();
    kit_table(&h, &r, true, "", empty)
}

/// `x.length` of a loose value the way JavaScript reads it: an array's length, a string's UTF-16 length, otherwise nothing (0).
pub fn js_length(v: &V) -> usize {
    match v {
        V::Array(a) => a.len(),
        V::String(s) => s.encode_utf16().count(),
        _ => 0,
    }
}

/// Distinct scenarios in order (`[a, b, c].filter((x, i, arr) => x && arr.indexOf(x) === i)`), by identity.
pub fn distinct<'a>(list: &[Option<&'a V>]) -> Vec<&'a V> {
    let mut out: Vec<&'a V> = Vec::new();
    for x in list.iter().flatten() {
        if !out.iter().any(|u| std::ptr::eq(*u, *x)) {
            out.push(x);
        }
    }
    out
}

/// `resOK(P)`: the option has results.
pub fn res_ok(p: &V) -> bool {
    !periods_of(pair(p).opt).is_empty()
}

/// `signalNodes(sc)`: the ids of the signalised junctions.
pub fn signal_nodes(sc: &V) -> Vec<String> {
    sc.g("nodes").vals().into_iter().filter(|n| n.g("control").eq_s("signals")).map(|n| n.g("id").s()).collect()
}

/* ---------- the pack builder ---------- */

/// One section of a hand-over CSV.
pub struct Section {
    /// The section title.
    pub title: String,
    /// A note under the title.
    pub note: Option<String>,
    /// The column headings.
    pub head: Vec<String>,
    /// The rows.
    pub rows: Vec<Vec<V>>,
    /// The line printed when there are no rows ("None" when not given).
    pub empty: Option<String>,
}

/// A section of a pack.
pub fn section(title: &str, note: Option<String>, head: &[&str], rows: Vec<Vec<V>>, empty: Option<&str>) -> Section {
    Section { title: title.to_string(), note, head: head.iter().map(|h| h.to_string()).collect(), rows, empty: empty.map(|e| e.to_string()) }
}

/// `packCSV(P, title, tool, sections)`: a hand-over CSV: the manifest, project, date and version stamp, then each section, then the footer.
pub fn pack_csv(p: &V, title: &str, tool: &str, sections: &[Section]) -> String {
    let mut rows: Vec<Vec<V>> = vec![
        vec![V::String(format!("# {title}"))],
        vec![sv("# Manifest"), V::String(sections.iter().map(|x| x.title.as_str()).collect::<Vec<_>>().join(" | "))],
        vec![sv("# Project"), or_blank(p.g("name"))],
        vec![sv("# Prepared"), V::String(today())],
        vec![sv("# Version stamp"), V::String(stamp(p))],
    ];
    for sec in sections {
        rows.push(vec![]);
        rows.push(vec![V::String(format!("## {}", sec.title))]);
        if let Some(n) = &sec.note {
            if !n.is_empty() {
                rows.push(vec![V::String(format!("# {n}"))]);
            }
        }
        rows.push(sec.head.iter().map(|h| V::String(h.clone())).collect());
        for r in &sec.rows {
            rows.push(r.clone());
        }
        if sec.rows.is_empty() {
            rows.push(vec![V::String(sec.empty.clone().filter(|e| !e.is_empty()).unwrap_or_else(|| "None".to_string()))]);
        }
    }
    stamped(&csv(&rows), p, Some(tool))
}

/* ---------- (3) ANOP tool inputs ---------- */

/// `anopInputsCSV(P)`: speed as a percentage of the posted limit and the 85th/median ratio by mode, corridor and direction (CSV).
pub fn anop_inputs_csv(p: &V) -> String {
    let pr = pair(p);
    let mut rows: Vec<Vec<V>> = Vec::new();
    for c in anop_corridors(p).items() {
        let ids: Vec<String> = c.g("links").items().iter().map(|x| x.s()).collect();
        for per in periods_of(pr.opt) {
            // one row per direction (spec anop: speed per direction)
            let r = res_of(pr.opt, per).unwrap_or(&NULL);
            let g = corridor_speed(pr.opt, r, &ids, false);
            let b = corridor_speed(pr.opt, r, &ids, true);
            let rel = if c.g("rel").t() { c.g("rel") } else { &NULL };
            let mut put = |mode: &str, x: &V, rv: &V| {
                let dirs: Vec<&V> = if x.t() && x.g("dirs").t() { x.g("dirs").items().iter().collect() } else { vec![&NULL] };
                for d in dirs {
                    let dv = if d.t() { d } else { &NULL };
                    rows.push(vec![
                        c.g("name").clone(),
                        if d.t() { d.g("name").clone() } else { sv("\u{2013}") },
                        sv(per),
                        sv(mode),
                        r1(if dv.t() { dv.g("pct") } else { &NULL }),
                        r1(if dv.t() { dv.g("spd") } else { &NULL }),
                        r1(if dv.t() { dv.g("limit") } else { &NULL }),
                        if x.t() { x.g("limitSrc").clone() } else { sv("") },
                        if rv.t() { rv.clone() } else { sv("N/A") },
                    ]);
                }
            };
            put("General traffic", &g, rel.g("gt"));
            put("Freight", &g, rel.g("freight"));
            put("Public transport", &b, rel.g("pt"));
        }
    }
    let note = format!(
        "{} Speed = \u{3a3}length / \u{3a3}(link time + junction delay) per direction. Reliability is N/A unless observed (TomTom or AT data). Where no NSLR limit is loaded the posted limit falls back to the modelled link speed ({}; see the source column).",
        xdata("STD.labels.anop").s(),
        xdata("ASSUMPTION").s()
    );
    pack_csv(
        p,
        "AT ANOP tool inputs",
        "the AT ANOP tool",
        &[section(
            "Average speed and reliability by mode, corridor and direction",
            Some(note),
            &[
                "Spare Construct",
                "Direction",
                "Period",
                "Mode",
                "Speed (% of posted)",
                "Average speed (km/h)",
                "Posted limit (km/h, length-weighted)",
                "Posted limit source",
                "85th/median journey time",
            ],
            rows,
            None,
        )],
    )
}

/* ---------- report section and exports ---------- */

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-anop-inputs"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, _ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id != "compliance-extra-anop-inputs" {
        return None;
    }
    let a = anop(p);
    if a.g("rows").items().is_empty() {
        return Some(String::new());
    }
    Some(format!(
        "<p>The ANOP tool inputs are in <code>anop.csv</code> (Export \u{203a} Compliance+: ANOP tool inputs), in the v10.2 input sheet column order: {} corridor-period rows (the worse direction). Per-direction speeds are in the ANOP speeds export.</p>{}",
        a.g("rows").items().len(),
        disc("D5", p, None)
    ))
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &[
    "compliance-extra-anop-inputs",
    "compliance-extra-tsdg",
    "compliance-extra-ssa",
    "compliance-extra-sh",
    "compliance-extra-mbcm",
    "compliance-extra-rltp",
    "compliance-extra-msm",
];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    Some(match id {
        "compliance-extra-anop-inputs" => anop_inputs_csv(p),
        "compliance-extra-tsdg" => tsdg_pack(p),
        "compliance-extra-ssa" => ssa_pack(p),
        "compliance-extra-sh" => sh_pack(p),
        "compliance-extra-mbcm" => mbcm_pack(p),
        "compliance-extra-rltp" => rltp_sheet(p),
        "compliance-extra-msm" => msm_pack(p),
        _ => return None,
    })
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    Some(match id {
        "compliance-extra-anop-inputs" | "compliance-extra-tsdg" | "compliance-extra-ssa" | "compliance-extra-sh" | "compliance-extra-mbcm" => res_ok(p),
        "compliance-extra-rltp" | "compliance-extra-msm" => true,
        _ => return None,
    })
}
