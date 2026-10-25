//! Speed limit validator and the cost benefit disclosure statement (compliance-extra.js section 9), with their CSV export and report section.

pub mod statement;

pub use self::statement::{cbds, crash_history, section_crashes};

use super::ctx::{active_sc, authority_of, data, disclaimer, esc, esc_s, link_name, nn, pair, posted, stamped};
use super::options::util::{cells, int32, label, or_s, tbl, unique};
use crate::compliance::data::x as xdata;
use crate::compliance::html::{ReportCtx, note, rt};
use crate::compliance::jv::{Jx, V, csv, jobj};
use crate::compliance::net;
use crate::jsnum::{math_round, number_to_js_string};
use crate::util::{self, fmt};

/// `minLenFor(v)`: the Schedule 1 minimum length (m) of a section with this limit (km/h).
pub fn min_len_for(v: f64) -> f64 {
    let t = xdata("STD.speed.minLen");
    for r in t.items() {
        if v <= r.ix(0).n() {
            return r.ix(1).n();
        }
    }
    t.items().last().map_or(f64::NAN, |r| r.ix(1).n())
}

/// `limitOf(L)`: the proposed limit (km/h) of a road: the author's entered limit, otherwise its posted limit.
pub fn limit_of(l: &V) -> f64 {
    let cx = l.g("cx");
    if cx.t() {
        if let Some(x) = nn(cx.g("limit")) {
            if x != 0.0 {
                return x;
            }
        }
    }
    posted(l)
}

/// `speedSections(sc)`: the road sections to validate: links sharing `cx.section`, otherwise connected links with the same name and limit (one section each).
pub fn speed_sections(sc: &V) -> Vec<Vec<&V>> {
    let links: Vec<&V> = sc
        .g("links")
        .vals()
        .into_iter()
        .filter(|l| !l.g("status").eq_s("closed") && l.g("cx").t() && (l.g("cx").g("ruleClass").t() || nn(l.g("cx").g("limit")).is_some() || l.g("cx").g("variable").t()))
        .collect();
    let key = |l: &V| -> String {
        if l.g("cx").g("section").t() {
            format!("s:{}", l.g("cx").g("section").s())
        } else {
            format!("n:{}|{}", if l.g("name").t() { l.g("name").s() } else { l.g("id").s() }, number_to_js_string(limit_of(l)))
        }
    };
    let mut groups: Vec<(String, Vec<&V>)> = Vec::new();
    for l in links {
        let k = key(l);
        match groups.iter_mut().find(|g| g.0 == k) {
            Some(g) => g.1.push(l),
            None => groups.push((k, vec![l])),
        }
    }
    let mut out: Vec<Vec<&V>> = Vec::new();
    for (k, ls) in groups {
        if k.starts_with("s:") {
            out.push(ls);
            continue;
        }
        let mut left: Vec<String> = unique(ls.iter().map(|l| l.g("id").s()).collect());
        while !left.is_empty() {
            // connected components by shared nodes
            let seed = left.remove(0);
            let mut comp: Vec<&V> = Vec::new();
            let mut st: Vec<String> = vec![seed];
            while let Some(id) = st.pop() {
                let l = sc.g("links").g(&id);
                comp.push(l);
                for o in &ls {
                    let oid = o.g("id").s();
                    if left.contains(&oid) && (o.g("a").strict_eq(l.g("a")) || o.g("a").strict_eq(l.g("b")) || o.g("b").strict_eq(l.g("a")) || o.g("b").strict_eq(l.g("b"))) {
                        left.retain(|x| *x != oid);
                        st.push(oid);
                    }
                }
            }
            out.push(comp);
        }
    }
    out
}

/// `roadLen(sc, Ls)`: the road length (m) of a section: the two one-way carriageways of a divided road (same name, opposite heading, within 60 m)
/// count once, as half their summed length.
pub fn road_len(sc: &V, ls: &[&V]) -> f64 {
    let pair_m = xdata("PAIR_M").n();
    let one_way = |l: &V| (int32(l.g("lanesAB")) > 0.0) != (int32(l.g("lanesBA")) > 0.0);
    let head = |l: &V| -> f64 {
        let g = net::geom(sc, l).unwrap_or_default();
        if g.is_empty() {
            return f64::NAN;
        }
        let b = util::bearing(&g[0], &g[g.len() - 1]);
        if int32(l.g("lanesAB")) > 0.0 { b } else { (b + 180.0) % 360.0 }
    };
    let mut len = 0.0;
    for &l in ls {
        let ll = net::len(sc, l);
        if !one_way(l) {
            len += ll;
            continue;
        }
        let g = net::geom(sc, l).unwrap_or_default();
        if g.is_empty() {
            len += f64::NAN;
            continue;
        }
        let mid = util::along(&g, &util::cum_len(&g), util::poly_len(&g) / 2.0).p;
        let b = head(l);
        let name_of = |x: &V| or_s(x.g("name"), "");
        let paired = ls.iter().any(|&o| {
            !std::ptr::eq(o, l) && one_way(o) && name_of(o).strict_eq(&name_of(l)) && util::ang_diff(b, head(o)) > 135.0 && net::project(sc, o, mid).is_some_and(|x| x.d <= pair_m)
        });
        len += if paired { ll / 2.0 } else { ll };
    }
    len
}

/// `currentOf(P, L)`: the current (existing) limit of a road: from the do-minimum where the road exists there.
pub fn current_of(p: &V, l: &V) -> f64 {
    let b = pair(p).base.g("links").g(&l.g("id").s());
    posted(if b.t() { b } else { l })
}

/// `speedCheck(P, sc)`: the speed limit validation of each road section: `[{ ids, name, ruleClass, classLabel, limit, current, len, minLen, exception, flags, ok, decision, onf }]`.
pub fn speed_check(p: &V, sc: Option<&V>) -> V {
    let sc = sc.unwrap_or_else(|| active_sc(p));
    let sp = xdata("STD.speed");
    let mut rows: Vec<V> = Vec::new();
    for ls in speed_sections(sc) {
        let l = ls[0];
        let empty = V::object();
        let x = if l.g("cx").t() { l.g("cx") } else { &empty };
        let limit = limit_of(l);
        let cls = sp.g("classes").g(&x.g("ruleClass").s());
        let len = road_len(sc, &ls);
        let mut flags: Vec<V> = Vec::new();
        let ls_txt = number_to_js_string(limit);
        if !cls.t() {
            flags.push(jobj! { "id" => "class", "text" => "No Rule class: choose one from Schedule 3 Tables 1-3 (road inspector \u{203a} Speed limit).", "rule" => "cl 4.5" });
        } else if limit < cls.g("min").n() || limit > cls.g("max").n() {
            let range = if cls.g("min").n() == cls.g("max").n() { cls.g("min").s() } else { format!("{}\u{2013}{}", cls.g("min").s(), cls.g("max").s()) };
            flags.push(jobj! { "id" => "range", "text" => format!("{ls_txt} km/h is outside the Schedule 3 range for {} ({range} km/h).", cls.g("label").s()), "rule" => "cl 4.5, Schedule 3" });
        }
        if cls.t() && cls.g("crashHistory").t() && !x.g("crashHistory").t() {
            flags.push(jobj! { "id" => "isz", "text" => format!("{}s need a history of high-risk crash types: record it.", cls.g("label").s()), "rule" => "Schedule 3 Table 3" });
        }
        let min = min_len_for(limit);
        let exc: String = if x.g("exception").t() && sp.g("exceptions").g(&x.g("exception").s()).t() {
            x.g("exception").s()
        } else if limit < 40.0 {
            "below40".to_string()
        } else if x.g("school").t() {
            "school".to_string()
        } else if x.g("variable").t() {
            "variable".to_string()
        } else {
            String::new()
        };
        if len < min && exc.is_empty() {
            flags.push(jobj! {
                "id" => "length",
                "text" => format!("The {ls_txt} km/h section is {} m long, shorter than the Schedule 1 minimum of {} m.", fmt::int(len), fmt::int(min)), "rule" => "cl 4.6, Schedule 1",
            });
        }
        let auth = authority_of(p, l, sc);
        if sp.g("nztaOnly").items().iter().any(|v| v.n() == limit) && !auth.g("nzta").t() {
            flags.push(jobj! {
                "id" => "nzta",
                "text" => format!("{ls_txt} km/h may be set only by NZTA, on roads designed, built and maintained for it; this road is controlled by {}.", auth.g("name").s()), "rule" => "cl 4.3(2)",
            });
        }
        if x.g("variable").t() && !sp.g("varReasons").g(&x.g("varReason").s()).t() {
            flags.push(jobj! { "id" => "variable", "text" => "A variable limit needs a cl 4.9(1)(b) reason (or Director approval).", "rule" => "cl 4.9(1)(b)" });
        }
        let onf: Vec<String> = if x.g("onf").t() {
            let want = x.g("onf").s().to_lowercase();
            sp.g("onf").entries().into_iter().filter(|(_, v)| v.items().iter().any(|n| n.s().to_lowercase() == want)).map(|(k, _)| sp.g("classes").g(k).g("label").s()).collect()
        } else {
            Vec::new()
        };
        let ok = flags.is_empty();
        rows.push(jobj! {
            "ids" => V::Array(ls.iter().map(|q| q.g("id").clone()).collect()), "name" => link_name(l), "ruleClass" => or_s(x.g("ruleClass"), ""),
            "classLabel" => if cls.t() { cls.g("label").clone() } else { V::String("Not set".into()) }, "limit" => limit, "current" => current_of(p, l), "len" => len, "minLen" => min,
            "exception" => if !exc.is_empty() { sp.g("exceptions").g(&exc).clone() } else { V::String(String::new()) },
            "flags" => V::Array(flags), "ok" => ok, "decision" => auth.g("decision"),
            "onf" => if !onf.is_empty() { format!("{} ({})", onf.join(" or "), label("onf")) } else { String::new() },
        });
    }
    V::Array(rows)
}

/// `schoolSpeeds(sc)`: school speed records from the NSLR variable layer: `[{ id, name, v, cat, windows, len }]`.
pub fn school_speeds(sc: &V) -> V {
    let re = crate::compliance::rx::rx("school", "i");
    V::Array(
        sc.g("links")
            .vals()
            .into_iter()
            .filter(|l| {
                let vs = l.g("varSpeed");
                vs.t() && re.test(&(if vs.g("desc").t() { vs.g("desc").s() } else { vs.g("v").s() }))
            })
            .map(|l| {
                let vs = l.g("varSpeed");
                let v = vs.g("v").n();
                jobj! {
                    "id" => l.g("id"), "name" => link_name(l), "v" => v,
                    "cat" => if v <= 30.0 { "Category 1 (30 km/h)" } else if v <= 60.0 { "Category 2 (60 km/h or less)" } else { "Check category" },
                    "windows" => or_s(vs.g("desc"), fmt::DASH), "len" => net::len(sc, l),
                }
            })
            .collect(),
    )
}

/* =====================================================================================
   CSV export
   ===================================================================================== */

/// `speedCSV(P)`: the speed limit validation and the stored cost benefit disclosure statement, with the version stamp.
pub fn speed_csv(p: &V) -> String {
    let mut rows: Vec<Vec<V>> = vec![
        cells!["Speed limit validation (Setting of Speed Limits Rule 2024)"],
        cells![
            "Road", "Rule class", "Current limit", "Proposed limit", "Section length (m, a divided road counted once)", "Schedule 1 minimum (m)", "Exception", "Decision-maker", "Flags",
            "ONF suggestion"
        ],
    ];
    for r in speed_check(p, None).items() {
        let flags = r.g("flags").items().iter().map(|f| format!("{} ({})", f.g("text").s(), f.g("rule").s())).collect::<Vec<_>>().join(" | ");
        rows.push(cells![r.g("name"), r.g("classLabel"), r.g("current"), r.g("limit"), math_round(r.g("len").n()), r.g("minLen"), r.g("exception"), r.g("decision"), flags, r.g("onf")]);
    }
    let cx = data(p);
    let cb = cx.g("speed").g("cbds");
    if cb.t() && cb.g("ok").t() {
        rows.push(vec![]);
        rows.push(cells![format!("Cost benefit disclosure statement: {}", cb.g("name").s()), label("cbds")]);
        for h in cb.g("headings").items() {
            rows.push(cells![h.g("title"), h.g("rule")]);
            for r in h.g("rows").items() {
                let mut row = cells![""];
                row.extend(r.items().iter().cloned());
                rows.push(row);
            }
        }
        rows.push(cells!["Decision-maker", cb.g("decision")]);
        for w in cb.g("warn").items() {
            rows.push(cells!["Warning", w]);
        }
        rows.push(cells!["Watermark", label("cbds")]);
        rows.push(cells!["D9", disclaimer("D9", p, None)]);
    }
    stamped(&csv(&rows), p, None)
}

/* =====================================================================================
   Report section
   ===================================================================================== */

/// `cbdsHtml(cb)`: a cost benefit disclosure statement as the watermarked block of the report.
pub fn cbds_html(cb: &V) -> String {
    let mut s = format!(
        "<div class=\"watermarked\" data-watermark=\"{}\"><p class=\"note\"><b>{}</b> \u{b7} {} \u{b7} decision-maker {}</p>",
        esc(cb.g("watermark")),
        esc(cb.g("watermark")),
        esc(cb.g("name")),
        esc(cb.g("decision"))
    );
    for x in cb.g("headings").items() {
        let rows: Vec<Vec<String>> = x.g("rows").items().iter().map(|r| vec![esc(r.ix(0)), esc(r.ix(1))]).collect();
        s.push_str(&format!("<h4>{} <span class=\"note\">{}</span></h4>", esc(x.g("title")), esc(x.g("rule"))));
        s.push_str(&tbl(&["Item", "Value"], rows, ""));
        if x.g("basis").t() {
            s.push_str(&format!("<p class=\"note\">{}</p>", esc(x.g("basis"))));
        }
    }
    for w in cb.g("warn").items() {
        s.push_str(&note(&esc(w), true));
    }
    s.push_str(&format!("<p class=\"note disclaimer\" data-disclaimer=\"D9\"><b>D9.</b> {}</p></div>", esc(cb.g("disclaimer"))));
    s
}

fn report_speed(p: &V, ctx: Option<&dyn ReportCtx>) -> String {
    let rows = speed_check(p, None);
    let cx = data(p);
    let cb = cx.g("speed").g("cbds");
    let cb_ok = cb.t() && cb.g("ok").t();
    if rows.jlen() == 0 && !cb_ok {
        return String::new();
    }
    let mut out = String::new();
    if rows.jlen() > 0 {
        let trs: Vec<Vec<String>> = rows
            .items()
            .iter()
            .map(|r| {
                vec![
                    esc(r.g("name")),
                    esc(r.g("classLabel")),
                    format!("{} \u{2192} {}", super::ctx::num(r.g("current"), 0), super::ctx::num(r.g("limit"), 0)),
                    fmt::int(r.g("len").n()),
                    if r.g("flags").jlen() > 0 { esc_s(&r.g("flags").items().iter().map(|f| f.g("text").s()).collect::<Vec<_>>().join(" ")) } else { "None".to_string() },
                ]
            })
            .collect();
        out.push_str(&rt(ctx, "Speed limit validation", &tbl(&["Road", "Rule class", "Limit", "Section (m)", "Flags"], trs, ""), "A divided road counts once in the section length."));
    }
    if cb_ok {
        out.push_str(&cbds_html(cb));
    }
    out
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-speed"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id == "compliance-extra-speed" { Some(report_speed(p, ctx)) } else { None }
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-speed"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    if id == "compliance-extra-speed" { Some(speed_csv(p)) } else { None }
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    if id == "compliance-extra-speed" { Some(speed_check(p, None).jlen() > 0 || data(p).g("speed").g("cbds").t()) } else { None }
}
