//! Chapters 1 and 2: introduction, scoping, the existing transport and land use environment.

use super::consts::{AT_FREQUENT, PERIODS, RR422_ROWS, SCOPES, attrib, level, level_of, rr422, scope, walk_factor, AT_LICENCE_DEFAULT};
use super::ctx::{Ctx, FigRequest, Head, Row, Tbl, TableOpts, cells, h, hn, note, para};
use super::js;
use super::rfmt;
use super::sections_front::{int, int_f, year_txt};
use super::shared::{
    RoadGroup, assessed_sites, crash_years, cx, ctext, cnum, kit_junction_name, road_groups, speed_of, junction_groups, plural,
};
use super::{Rep, Sel};
use crate::edit::json::JsonValue;
use crate::edit::net::links_at;
use crate::edit::CONTROLS;
use crate::graph::{build_graph, js_sort_by, zone_class};
use crate::jsnum::{JsNum, Maybe, math_max, math_min, number_to_js_string, value_truthy};
use crate::project::{Link, Scenario, road_class};
use crate::util::{self, esc, fmt};
use serde_json::Value;

/// `CONTROLS[t] || t`.
pub(crate) fn control_label(t: &str) -> String {
    for (k, v) in CONTROLS {
        if k == t {
            return v.to_string();
        }
    }
    t.to_string()
}

fn sc_of<'a>(c: &Ctx<'a, '_>, i: Option<usize>) -> usize {
    i.or(c.sel.base).unwrap_or(0)
}

/// `studyBounds(P)`: the study area's bounds, or the development zones' extent with a margin of about 600 m.
pub(crate) fn study_bounds(r: &Rep, sel: &Sel) -> Option<JsonValue> {
    let a = js::get(Some(r.raw), "studyArea")
        .filter(|v| v.truthy())
        .or_else(|| js::path(Some(r.raw), &["report", "bounds"]));
    if let Some(a) = a {
        if let Some(b) = js::get(Some(a), "bounds").filter(|b| b.is_array()) {
            return Some(b.clone());
        }
        if a.is_array() {
            return Some(a.clone());
        }
        if js::get(Some(a), "south").is_some_and(|v| !v.is_null()) {
            let n = |k: &str| JsonValue::Number(js::num(js::get(Some(a), k)));
            let g = |k: &str| js::get(Some(a), k).cloned().unwrap_or(JsonValue::Null);
            let _ = n;
            return Some(JsonValue::Array(vec![
                JsonValue::Array(vec![g("south"), g("west")]),
                JsonValue::Array(vec![g("north"), g("east")]),
            ]));
        }
    }
    let sc = sel.with.or(if r.project.scenarios.is_empty() { None } else { Some(0) })?;
    let pts: Vec<[f64; 2]> = r
        .scenario(sc)
        .zones
        .values()
        .filter(|z| !z.kind.is("gateway"))
        .map(|z| [z.lat.num(), z.lng.num()])
        .collect();
    if pts.is_empty() {
        return None;
    }
    let (mut s, mut w, mut n, mut e) = (90.0f64, 180.0f64, -90.0f64, -180.0f64);
    for [la, ln] in pts {
        s = math_min(s, la);
        n = math_max(n, la);
        w = math_min(w, ln);
        e = math_max(e, ln);
    }
    let pad = 0.006;
    let num = JsonValue::Number;
    Some(JsonValue::Array(vec![
        JsonValue::Array(vec![num(s - pad), num(w - pad * 1.25)]),
        JsonValue::Array(vec![num(n + pad), num(e + pad * 1.25)]),
    ]))
}

fn show(items: &[(&str, bool)]) -> Vec<(String, bool)> {
    items.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn fig(id: &str, title: &str, mode: &str, scenario_id: &str, bounds: Option<JsonValue>, show: Vec<(String, bool)>) -> FigRequest {
    FigRequest {
        id: id.to_string(),
        title: title.to_string(),
        mode: mode.to_string(),
        scenario_id: scenario_id.to_string(),
        base_scenario_id: None,
        period: None,
        bounds,
        show,
        source: None,
    }
}

/// `report-purpose`.
pub(crate) fn purpose(c: &mut Ctx) -> String {
    c.author("purpose", "Describe the proposal, who it is for, the consent or approval sought and the statutory tests it answers (for example RMA s88(2)(b) and Sch 4; s168(3A) for a notice of requirement; FTAA Sch 5 cl 5\u{2013}7).")
}

/// `report-reason`.
pub(crate) fn reason(c: &mut Ctx) -> String {
    format!(
        "{}{}",
        c.author("reason", "List the AUP E27 triggers (Table E27.6.1.1 thresholds, tier and exemptions), the access, parking and loading infringements and the activity status."),
        note("The E27 screening table is added here by the compliance feature when it is installed. Never read an E27.6.1 exceedance by itself as requiring an ITA.", false)
    )
}

/// `report-engagement`.
pub(crate) fn engagement(c: &mut Ctx) -> String {
    let rows: Vec<Row> = c
        .r
        .rs
        .engagement
        .iter()
        .map(|r| {
            let g = |k: &str| esc(&js::text(r.get(k)));
            Row::new(vec![g("date"), g("party"), g("topic"), g("outcome")])
        })
        .collect();
    c.table(
        "Scoping and engagement log",
        Tbl::new(vec![h("Date"), h("Party"), h("Topic"), h("Outcome")], rows)
            .empty("No engagement recorded yet. Add meetings and agreements in the Report tab."),
        &TableOpts::default(),
    )
}

/// `hasRes(P, pred)`: some scenario has a result for AM, PM or IP the predicate accepts.
fn has_res(r: &Rep, pred: impl Fn(&crate::results::Res) -> bool) -> bool {
    (0..r.project.scenarios.len()).any(|i| PERIODS.iter().any(|p| r.results[i].get(p).is_some_and(&pred)))
}

/// Whether a row of RR 422 Table 6.3 is covered (`r.done(P)`).
pub(crate) fn rr422_done(r: &Rep, id: &str) -> bool {
    let data = r.data();
    let len = |k: &str| !js::items(js::get(data, k)).is_empty();
    match id {
        "crash" => len("crashes"),
        "flows" => len("counts"),
        "times" => js::items(js::get(data, "jt")).iter().any(|j| js::truthy(js::get(Some(j), "g"))),
        "congestion" => has_res(r, |x| !x.nodes.is_empty()),
        "vehtrips" => has_res(r, |x| x.kpi.trips > 0.0),
        "persontrips" => r.project.scenarios.iter().any(|s| {
            s.zones.values().any(|z| {
                z.reduction.num() > 0.0
                    || z.extra.get("persons").is_some_and(value_truthy)
                    || z.extra.get("modeShare").is_some_and(value_truthy)
            })
        }),
        "sensitivity" => js::truthy(js::path(Some(r.raw), &["report", "sens", "rows"])),
        "safety" => len("crashes") && has_res(r, |_| true),
        _ => false,
    }
}

/// `report-rr422`.
pub(crate) fn rr422_section(c: &mut Ctx) -> String {
    let r = c.r;
    let sc = r.rs.scope.as_str();
    let rows: Vec<Row> = RR422_ROWS
        .iter()
        .map(|row| {
            let lv: Vec<String> = SCOPES
                .iter()
                .map(|k| {
                    let l = level_of(row, k.key);
                    format!(
                        "<span class=\"lv lv-{}{}\">{}</span>",
                        l,
                        if k.key == sc { " on" } else { "" },
                        level(l)
                    )
                })
                .collect();
            let ok = rr422_done(r, row.id);
            let l = if sc.is_empty() { None } else { Some(level_of(row, sc)) };
            let mut cs = vec![esc(row.label)];
            cs.extend(lv);
            cs.push(if ok {
                "<span class=\"ok\">Included</span>".to_string()
            } else if l == Some('P') {
                format!("<span class=\"bad\">Missing (Probable)</span> {}", esc(row.how))
            } else {
                format!("<span class=\"muted\">Not included</span> {}", esc(row.how))
            });
            Row::cls(cs, if !ok && l == Some('P') { "miss" } else { "" })
        })
        .collect();
    let mut head = vec![h("Item")];
    head.extend(SCOPES.iter().map(|k| h(k.label)));
    head.push(h("Status"));
    format!(
        "{}\n      {}",
        match scope(sc) {
            Some(s) => para(&format!("Scope chosen: <b>{}</b>. The chosen scope's column is outlined.", s.label)),
            None => note("Choose the scope in the Report tab to check the content against it.", true),
        },
        c.table(
            "RR 422 Table 6.3 content checklist",
            Tbl::new(head, rows),
            &TableOpts {
                source: Some(format!("{}. {}", esc(rr422::EDITION), esc(rr422::NOTE))),
                ..TableOpts::default()
            }
        )
    )
}

/// `report-locality`.
pub(crate) fn locality(c: &mut Ctx) -> String {
    let sc = sc_of(c, c.sel.existing);
    let bounds = study_bounds(c.r, &c.sel);
    let id = c.r.scenario(sc).id.clone();
    format!(
        "{}\n      {}",
        c.author("locality", "Describe the site, its surroundings and the study area."),
        c.figure(fig("locality", "Locality and study area", "class", &id, bounds, show(&[("counts", false), ("crashes", false), ("connectors", false)])))
    )
}

/// `report-zoning`.
pub(crate) fn zoning(c: &mut Ctx) -> String {
    format!(
        "{}{}",
        c.author("zoning", "Describe the operative AUP zoning (and any proposed zoning, such as PC120) of the site and its surroundings."),
        note("The AUP zone map is added by the land-use feature when Auckland Unitary Plan zones are loaded.", false)
    )
}

/// `fcText(L, dec)`: the Future Connect classes of a link.
fn fc_text(l: Option<&Link>, dec: bool) -> String {
    let Some(f) = l.and_then(|l| l.fc.get()).filter(|f| value_truthy(f)) else {
        return fmt::DASH.to_string();
    };
    let k = if dec { "10" } else { "" };
    ["gt", "pt", "freight", "cycle"]
        .iter()
        .map(|n| {
            let v = f.get(format!("{n}{k}"));
            let t = match v {
                Some(x) if value_truthy(x) => crate::jsnum::value_to_js_string(x),
                _ => fmt::DASH.to_string(),
            };
            esc(&t)
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

/// AUP arterial overlay and vehicle access restriction per link (`aupOf`).
pub(crate) struct Aup {
    /// The layer is loaded.
    pub loaded: bool,
    art: serde_json::Map<String, Value>,
    vmap: std::collections::HashMap<String, String>,
}

impl Aup {
    /// `art(L)`: true, false, or None when the layer is not loaded.
    pub fn art(&self, l: &Link) -> Option<bool> {
        let ex = |k: &str| l.extra.get(k);
        let on = self.art.get(&l.id).is_some_and(value_truthy)
            || ex("aupArterial").is_some_and(|v| v.as_bool() == Some(true))
            || ex("aup").is_some_and(|v| v.as_bool() == Some(true));
        if on {
            Some(true)
        } else if self.loaded || ex("aupArterial").is_some_and(|v| v.as_bool() == Some(false)) {
            Some(false)
        } else {
            None
        }
    }
    /// `vr(L)`: the access restriction type, empty when none, None when the layer is not loaded.
    pub fn vr(&self, l: &Link) -> Option<String> {
        if let Some(v) = self.vmap.get(&l.id).filter(|v| !v.is_empty()) {
            return Some(v.clone());
        }
        for k in ["var", "vehicleAccessRestriction"] {
            if let Some(v) = l.extra.get(k).filter(|v| value_truthy(v)) {
                return Some(crate::jsnum::value_to_js_string(v));
            }
        }
        if self.loaded { Some(String::new()) } else { None }
    }
}

/// `aupOf(P, sc)`.
pub(crate) fn aup_of(r: &Rep, sc: usize) -> Aup {
    let comp = js::get(Some(r.raw), "compliance");
    let net = js::get(comp, "net");
    let ov = js::get(comp, "overlays").filter(|o| o.truthy());
    let art = js::get(net, "aupArterial")
        .filter(|a| a.truthy() && a.is_object())
        .map(|a| a.to_value())
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default();
    let loaded = ov.is_some_and(|o| !js::items(js::get(Some(o), "sources")).is_empty()) || !art.is_empty();
    let mut vmap = std::collections::HashMap::new();
    if let Some(o) = ov {
        let var = js::get(Some(o), "var");
        if !js::items(var).is_empty() {
            let items = JsonValue::Array(
                js::items(var)
                    .iter()
                    .map(|v| {
                        JsonValue::from_pairs([
                            ("tag", js::get(Some(v), "type").cloned().unwrap_or(JsonValue::Null)),
                            ("pts", js::get(Some(v), "pts").cloned().unwrap_or(JsonValue::Null)),
                        ])
                    })
                    .collect(),
            );
            if let Some(m) = r.host.conflate(sc, &items) {
                vmap = m;
            }
        }
    }
    Aup { loaded, art, vmap }
}

fn uniq(v: impl Iterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in v {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// `g0nslr(groups)`: some link's speed comes from the NSLR.
fn g0nslr(groups: &[RoadGroup]) -> bool {
    groups.iter().any(|g| g.links.iter().any(|l| speed_of(l).src == "NSLR"))
}

/// `report-roads`.
pub(crate) fn roads(c: &mut Ctx) -> String {
    let r = c.r;
    let sc = sc_of(c, c.sel.existing);
    let scn = r.scenario(sc);
    let groups = road_groups(scn);
    let a = aup_of(r, sc);
    let rows: Vec<Row> = groups
        .iter()
        .take(40)
        .map(|g| {
            let l = g.links.iter().copied().find(|l| l.fc.get().is_some_and(value_truthy)).or(g.links.first().copied());
            let sp = uniq(g.links.iter().map(|l| {
                let s = speed_of(l);
                format!("{} ({})", s.v.map_or(fmt::DASH.to_string(), number_to_js_string), s.src)
            }))
            .join(", ");
            let lanes = g
                .links
                .iter()
                .map(|l| math_max(l.lanes_ab.int32() as f64, l.lanes_ba.int32() as f64))
                .fold(f64::NEG_INFINITY, math_max);
            let ar: Vec<Option<bool>> = g.links.iter().map(|l| a.art(l)).collect();
            let aup = if ar.contains(&Some(true)) {
                "Yes"
            } else if ar.contains(&Some(false)) {
                "No"
            } else {
                fmt::DASH
            };
            let vr = uniq(g.links.iter().filter_map(|l| a.vr(l)).filter(|v| !v.is_empty()));
            Row::new(vec![
                esc(&g.name),
                esc(road_class(Some(g.cls.as_str())).label),
                rfmt::num(g.km, 2),
                fc_text(l, false),
                fc_text(l, true),
                aup.to_string(),
                esc(&if !vr.is_empty() {
                    vr.join(", ")
                } else if a.loaded {
                    "None".to_string()
                } else {
                    fmt::DASH.to_string()
                }),
                esc(&sp),
                number_to_js_string(lanes),
            ])
        })
        .collect();
    let loaded = r.loaded();
    let has = |k: &str| js::get(loaded, k).is_some_and(JsonValue::truthy);
    let source = format!(
        "{}{}{}{}",
        attrib::OSM,
        if has("fc") { format!("; Future Connect: {}", esc(AT_LICENCE_DEFAULT)) } else { String::new() },
        if has("nslr") || g0nslr(&groups) { format!("; {}", attrib::NSLR) } else { String::new() },
        if a.loaded { format!("; AUP arterial and vehicle access restriction controls: {}", esc(attrib::COUNCIL)) } else { String::new() }
    );
    let nt = format!(
        "{}Local roads are left out. '\u{2013}' means the layer is not loaded for this study area (AUP controls: Compliance tab).",
        if groups.len() > 40 { format!("The 40 longest of {} roads are shown. ", groups.len()) } else { String::new() }
    );
    format!(
        "{}\n      {}",
        c.table(
            "Table E1 \u{b7} Road network",
            Tbl::new(
                vec![
                    h("Road"),
                    h("Class"),
                    hn("Length km"),
                    h("Future Connect GT / PT / freight / cycle (current)"),
                    h("Future Connect (first decade)"),
                    h("AUP arterial"),
                    h("Vehicle access restriction"),
                    h("Speed limit km/h (source)"),
                    hn("Lanes per direction"),
                ],
                rows
            )
            .empty("No named arterial or collector roads in this scenario."),
            &TableOpts::src_note(&source, &nt)
        ),
        if has("fc") {
            String::new()
        } else {
            note("Load AT Future Connect (Traffic data) to fill the Future Connect columns; outside Auckland, tag classes by hand.", false)
        }
    )
}

/// The public transport stops of the project and where they come from (`stopsOf`).
struct Stops {
    list: Vec<JsonValue>,
    /// Escaped source.
    src: String,
    /// Unescaped source text.
    text: String,
}

fn stops_of(r: &Rep) -> Stops {
    let data = r.data();
    let own = js::path(data, &["pt", "stops"])
        .filter(|v| v.truthy())
        .or_else(|| js::get(data, "stops"));
    if let Some(JsonValue::Array(a)) = own
        && !a.is_empty()
    {
        return Stops {
            list: a.clone(),
            src: esc(attrib::AT),
            text: attrib::AT.to_string(),
        };
    }
    let g = js::path(Some(r.raw), &["feeds", "gtfs"]);
    if let Some(g) = g
        && let Some(JsonValue::Array(stops)) = js::get(Some(g), "stops")
        && !stops.is_empty()
    {
        let who = js::get(Some(g), "publisher")
            .filter(|v| v.truthy())
            .or_else(|| js::path(Some(g), &["feed", "provider"]).filter(|v| v.truthy()))
            .map_or("the operator".to_string(), JsonValue::js_string);
        let ds = js::text(js::get(Some(g), "date"));
        let dt = if ds.len() == 8 && ds.bytes().all(|b| b.is_ascii_digit()) {
            format!("{}-{}-{}", &ds[..4], &ds[4..6], &ds[6..8])
        } else {
            String::new()
        };
        let lic = js::path(Some(g), &["feed", "licence"])
            .filter(|v| v.truthy())
            .map_or("the publisher's licence".to_string(), JsonValue::js_string);
        let at = super::ctx::day_of(js::get(Some(g), "at"));
        let text = format!(
            "GTFS timetable from {}{} ({}), loaded {}",
            who,
            if dt.is_empty() { String::new() } else { format!(", service day {dt}") },
            lic,
            if at.is_empty() { "date not recorded".to_string() } else { at }
        );
        let list = stops
            .iter()
            .map(|s| {
                let mut o = s.clone();
                let buses = match js::get(Some(s), "busesAM") {
                    Some(b) if !b.is_null() => b.clone(),
                    _ => js::get(Some(s), "am").cloned().unwrap_or(JsonValue::Null),
                };
                o.set("busesAM", buses);
                o
            })
            .collect();
        return Stops {
            list,
            src: esc(&text),
            text,
        };
    }
    Stops {
        list: Vec::new(),
        src: String::new(),
        text: String::new(),
    }
}

/// `fcPtNear(sc, s)`: the Future Connect PT class of the nearest link (within 50 m) that has one.
fn fc_pt_near(sc: &Scenario, s: &JsonValue) -> Option<String> {
    let at = [js::num(js::get(Some(s), "lat")), js::num(js::get(Some(s), "lng"))];
    let mut best: Option<String> = None;
    let mut bd = 50.0;
    for l in sc.links.values() {
        let Some(pt) = l.fc.get().and_then(|f| f.get("pt")).filter(|v| value_truthy(v)) else {
            continue;
        };
        if l.status.is("closed") {
            continue;
        }
        for q in sc.geom(l) {
            let d = util::dist(&q, &at);
            if d < bd {
                bd = d;
                best = Some(crate::jsnum::value_to_js_string(pt));
            }
        }
    }
    best
}

/// `siteCentre(sc)`: the mean position of the development zones.
fn site_centre(sc: &Scenario) -> Option<[f64; 2]> {
    let z: Vec<&crate::project::Zone> = sc.zones.values().filter(|z| zone_class(Some(z)) == "development").collect();
    if z.is_empty() {
        return None;
    }
    let n = z.len() as f64;
    Some([
        z.iter().fold(0.0, |s, x| s + x.lat.num()) / n,
        z.iter().fold(0.0, |s, x| s + x.lng.num()) / n,
    ])
}

fn anop_cycle(l: &Link, aadt: Option<f64>) -> &'static str {
    if l.cycle.is("separated") {
        "B"
    } else if l.cycle.is("painted") {
        "E"
    } else {
        let sp = l.speed.num();
        if sp <= 30.0 && !(aadt.is_some_and(|a| a > 3000.0)) {
            "D"
        } else if sp <= 50.0 {
            "E"
        } else {
            "F"
        }
    }
}

fn at_facility(cycle: Option<&str>) -> &'static str {
    match cycle {
        Some("separated") => "Protected cycle lane",
        Some("painted") => "On-road cycle lane",
        _ => "Mixed traffic",
    }
}

/// `report-active`.
pub(crate) fn active(c: &mut Ctx) -> String {
    let r = c.r;
    let sc = sc_of(c, c.sel.existing);
    let scn = r.scenario(sc);
    let rb = r.results[sc].get("AM").or_else(|| r.results[sc].get("PM"));
    let ctr = site_centre(r.scenario(sc_of(c, c.sel.with.or(Some(sc)))));
    let st = stops_of(r);
    let bounds = study_bounds(r, &c.sel);
    let sc_id = scn.id.clone();
    let pt = if !st.list.is_empty() && ctr.is_some() {
        let ctr = ctr.expect("centre");
        let mut ds: Vec<(&JsonValue, f64)> = st
            .list
            .iter()
            .map(|s| (s, util::dist(&ctr, &[js::num(js::get(Some(s), "lat")), js::num(js::get(Some(s), "lng"))]) * walk_factor::VALUE))
            .filter(|(_, d)| *d <= 1200.0)
            .collect();
        js_sort_by(&mut ds, |a, b| a.1 - b.1);
        ds.truncate(30);
        let rows: Vec<Row> = ds
            .iter()
            .map(|(s, d)| {
                let g = |k: &str| js::get(Some(s), k);
                let code = js::text_or_empty(g("code"));
                let mode = js::text_or_empty(g("mode"));
                let routes = match g("routes") {
                    Some(JsonValue::Array(a)) => a.iter().map(|x| if x.is_null() { String::new() } else { x.js_string() }).collect::<Vec<_>>().join(", "),
                    _ => String::new(),
                };
                let buses = match g("busesAM") {
                    Some(b) if !b.is_null() && !matches!(b, JsonValue::String(t) if t.is_empty()) => rfmt::num(b.to_number(), 1),
                    _ => fmt::DASH.to_string(),
                };
                let fc = js::text_or_empty(g("fcPt"));
                let fc = if !fc.is_empty() { fc } else { fc_pt_near(scn, s).unwrap_or_else(|| fmt::DASH.to_string()) };
                Row::new(vec![
                    esc(&js::text(g("name"))),
                    esc(&if code.is_empty() { js::text(g("id")) } else { code }),
                    esc(&if mode.is_empty() { "Bus".to_string() } else { mode }),
                    rfmt::q(*d),
                    rfmt::num(d / 80.0, 1),
                    esc(&routes),
                    buses,
                    esc(&fc),
                    (if *d <= 400.0 { "400 m" } else if *d <= 800.0 { "800 m" } else { "1,200 m" }).to_string(),
                ])
            })
            .collect();
        c.table(
            "Table PT-1 \u{b7} Public transport stops near the site",
            Tbl::new(
                vec![h("Stop or station"), h("Code"), h("Mode"), hn("Walk m"), hn("Walk min"), h("Routes"), hn("Buses/h AM"), h("Future Connect PT"), h("Within")],
                rows,
            )
            .empty("No stops within 1,200 m."),
            &TableOpts {
                source: Some(st.src.clone()),
                note: Some(format!("{}. Walk time at 80 m/min. {}", esc(walk_factor::BASIS), esc(AT_FREQUENT))),
                d: vec!["D13"],
                ..TableOpts::default()
            },
        )
    } else {
        note(&format!("Public transport stops are not loaded for this study area, so Table PT-1 is not filled. Load the bus timetable (GTFS) or AT bus and train stops (data feeds). {}", esc(AT_FREQUENT)), false)
    };
    let groups = road_groups(scn);
    let ptd = {
        let v = r.st().peak_to_daily.num();
        if v == 0.0 || v.is_nan() { 10.0 } else { v }
    };
    let cy: Vec<Row> = groups
        .iter()
        .take(30)
        .map(|g| {
            let l = g.links[0];
            let aadt = rb.map(|rb| {
                let a = rb.arcs.get(&format!("{}:ab", l.id));
                let b = rb.arcs.get(&format!("{}:ba", l.id));
                (a.map_or(0.0, |x| x.v) + b.map_or(0.0, |x| x.v)) * ptd
            });
            let lts = crate::results::lts(l, rb, r.st());
            Row::new(vec![
                esc(&g.name),
                esc(at_facility(l.cycle.as_str())),
                if lts != 0 { format!("LTS {}", lts) } else { "Not allowed".to_string() },
                anop_cycle(l, aadt).to_string(),
            ])
        })
        .collect();
    let figs = if ctr.is_some() {
        let mut f = fig(
            "pt-catchments",
            &format!("Public transport stops and the {} m walking catchments of the site", ["400", "800", "1,200"].join(" / ")),
            "class",
            &sc_id,
            bounds.clone(),
            show(&[("counts", false), ("crashes", false), ("report-catchments", true), ("feeds-bus-stops", true)]),
        );
        f.source = Some(format!("{}Catchments: {}", if st.text.is_empty() { String::new() } else { format!("{}. ", st.text) }, walk_factor::BASIS));
        c.figure(f)
    } else {
        String::new()
    };
    let cy_table = c.table(
        "Table CY-1 \u{b7} Cycling facilities and level of traffic stress",
        Tbl::new(vec![h("Section"), h("AT facility type"), h("Spare Construct LTS"), h("ANOP cycle LoS (indicative)")], cy).empty("No named roads."),
        &TableOpts {
            note: Some("ANOP cycle LoS from the indicative mapping in Spare Construct spec (off-road cycleway A; protected lane B; shared path B/C; trail or shared zone C; LATM D; on-road lane E; mixed traffic D/E/F by speed and volume).".to_string()),
            d: vec!["D5"],
            ..TableOpts::default()
        },
    );
    let mut lts_fig = fig(
        "cycle-lts",
        &format!("Cycle network: level of traffic stress, {}", scn.name.as_str().unwrap_or("")),
        "lts",
        &sc_id,
        bounds,
        show(&[("counts", false), ("crashes", false)]),
    );
    lts_fig.period = rb.map(|x| x.period.clone());
    format!(
        "{}{}{}\n      {}\n      {}",
        pt,
        figs,
        cy_table,
        c.figure(lts_fig),
        c.author("active", "Describe walking and cycling routes, crossings and public transport services serving the site.")
    )
}

/// A count row for the traffic volumes table (`countRows`).
struct CountRow<'a> {
    c: &'a JsonValue,
    t: Option<String>,
    am: bool,
    growth: Option<f64>,
    dur: String,
    one_day: bool,
}

fn one_day(dur: &str) -> bool {
    let d = dur.trim_start_matches(crate::jsnum::is_js_space).to_lowercase();
    let after_ws = |s: &str| s.trim_start_matches(crate::jsnum::is_js_space).to_string();
    if let Some(rest) = d.strip_prefix('1') {
        let r = after_ws(rest);
        if r.starts_with('d') {
            return true;
        }
    }
    if let Some(rest) = d.strip_prefix("24") {
        let r = after_ws(rest);
        if r.starts_with('h') {
            return true;
        }
    }
    false
}

fn count_rows<'a>(r: &Rep<'a>) -> Vec<CountRow<'a>> {
    let cs = js::items(js::get(r.data(), "counts"));
    cs.iter()
        .filter(|c| !js::truthy(js::get(Some(c), "off")))
        .map(|c| {
            let peak = js::num(js::get(Some(c), "peak"));
            let pt = js::get(Some(c), "peakTime").filter(|v| v.truthy());
            let t = match pt {
                Some(p) if peak > 0.0 && p.js_string() != "00:00" => Some(p.js_string()),
                _ => None,
            };
            let am = t.as_ref().is_some_and(|t| {
                let h: String = t.chars().take(2).collect();
                crate::jsnum::string_to_number(&h) < 12.0
            });
            let hist: Option<Vec<f64>> = js::get(Some(c), "hist")
                .filter(|h| h.truthy())
                .map(|h| js::items(Some(h)).iter().map(JsonValue::to_number).filter(|v| *v > 0.0).collect());
            let growth = hist.as_ref().filter(|h| h.len() >= 2).map(|h| (h[h.len() - 1] - h[0]) / h[0] / (h.len() as f64 - 1.0) * 100.0);
            let dur = match js::get(Some(c), "dur").filter(|v| v.truthy()) {
                Some(d) => d.js_string(),
                None => {
                    if js::get(Some(c), "src").and_then(|v| v.as_str()) == Some("nzta") {
                        if js::truthy(js::get(Some(c), "virtual")) {
                            "NZTA estimate".to_string()
                        } else if js::text_or_empty(js::get(Some(c), "note")).to_lowercase().contains("continuous") {
                            "Continuous".to_string()
                        } else {
                            "Periodic".to_string()
                        }
                    } else {
                        String::new()
                    }
                }
            };
            CountRow {
                c,
                t,
                am,
                growth,
                one_day: one_day(&dur),
                dur,
            }
        })
        .collect()
}

fn src_short(v: Option<&JsonValue>) -> String {
    let key = js::text(v);
    match crate::datahub::source(&key) {
        Some(s) if v.is_some_and(JsonValue::is_object_like) == false => s.short.to_string(),
        _ => key,
    }
}

/// `peakStatement(P)`: the peak hours derived from the loaded counts.
pub(crate) fn peak_statement(r: &Rep) -> String {
    let rows = count_rows(r);
    let hours: Vec<&CountRow> = rows.iter().filter(|x| x.t.is_some()).collect();
    let all = rows.len();
    if all == 0 {
        return "No traffic counts are loaded, so no peak hour can be derived. The model uses one-hour AM and PM peaks.".to_string();
    }
    let years: Vec<f64> = rows
        .iter()
        .filter_map(|x| {
            let d = js::get(Some(x.c), "date").filter(|v| v.truthy())?;
            let y: String = d.js_string().chars().take(4).collect();
            let n = crate::jsnum::string_to_number(&y);
            if n != 0.0 && !n.is_nan() { Some(n) } else { None }
        })
        .collect();
    let yr = if years.is_empty() {
        "dates not recorded".to_string()
    } else {
        format!(
            "{}\u{2013}{}",
            number_to_js_string(years.iter().copied().fold(f64::INFINITY, f64::min)),
            number_to_js_string(years.iter().copied().fold(f64::NEG_INFINITY, f64::max))
        )
    };
    let mode = |list: &[&CountRow]| -> Option<(String, usize)> {
        let mut m: Vec<(String, usize)> = Vec::new();
        for x in list {
            let t = x.t.clone().unwrap_or_default();
            match m.iter().position(|(k, _)| *k == t) {
                Some(i) => m[i].1 += 1,
                None => m.push((t, 1)),
            }
        }
        js_sort_by(&mut m, |a, b| b.1 as f64 - a.1 as f64);
        m.into_iter().next()
    };
    let hh = |t: &str| {
        let hr = crate::jsnum::string_to_number(&t.chars().take(2).collect::<String>());
        let n = number_to_js_string(hr + 1.0);
        format!("{}\u{2013}{}:{}", t, if n.len() < 2 { format!("0{n}") } else { n }, t.chars().skip(3).collect::<String>())
    };
    let am_list: Vec<&CountRow> = hours.iter().copied().filter(|x| x.am).collect();
    let pm_list: Vec<&CountRow> = hours.iter().copied().filter(|x| !x.am).collect();
    let am = mode(&am_list);
    let pm = mode(&pm_list);
    let src = uniq(rows.iter().map(|x| src_short(js::get(Some(x.c), "src")))).join(" and ");
    if am.is_none() && pm.is_none() {
        return format!(
            "None of the {} ({}, {}) reports a peak hour, so the model's one-hour AM and PM peaks are assumed.",
            plural(all as f64, "loaded count", "loaded counts"),
            esc(&src),
            yr
        );
    }
    let part = |m: &Option<(String, usize)>, n: usize, w: &str| match m {
        Some((t, k)) => format!("{w} peak hour {} (the busiest hour at {k} of {n} counts with a {w} peak)", hh(t)),
        None => format!("no {w} peak hour reported"),
    };
    format!(
        "Weekday {} and {}, derived from {} of {} {} counts that report a peak hour ({}).",
        part(&am, am_list.len(), "AM"),
        part(&pm, pm_list.len(), "PM"),
        plural(hours.len() as f64, "count", "counts"),
        all,
        esc(&src),
        yr
    )
}

/// `report-volumes`.
pub(crate) fn volumes(c: &mut Ctx) -> String {
    let r = c.r;
    let rows: Vec<Row> = count_rows(r)
        .iter()
        .map(|cr| {
            let k = cr.c;
            let g = |key: &str| js::get(Some(k), key);
            let src = src_short(g("src"));
            let is_nzta = g("src").and_then(|v| v.as_str()) == Some("nzta");
            let nzta_at = js::path(r.loaded(), &["nzta", "at"]).filter(|a| a.truthy());
            let date = match g("date").filter(|d| d.truthy()) {
                Some(d) => d.js_string(),
                None if is_nzta => format!(
                    "Latest NZTA AADT year{}",
                    nzta_at.map_or(String::new(), |a| format!(" (loaded {})", super::ctx::day_of(Some(a))))
                ),
                None => fmt::DASH.to_string(),
            };
            let hv = g("hv").filter(|v| !v.is_null());
            let peak = js::num(g("peak"));
            let adt = js::num(g("adt"));
            let dir = js::text_or_empty(g("dir"));
            Row::new(vec![
                esc(&js::text_or_empty(g("road"))),
                esc(&js::text(g("name"))),
                format!(
                    "{} {}{}",
                    esc(&src),
                    esc(&js::text_or_empty(g("ref"))),
                    if js::truthy(g("virtual")) { " (estimate)" } else { "" }
                ),
                esc(&date),
                format!(
                    "{}{}",
                    esc(&if cr.dur.is_empty() { fmt::DASH.to_string() } else { cr.dur.clone() }),
                    if cr.one_day { " <b class=\"warn\" title=\"One day only\">24H</b>" } else { "" }
                ),
                rfmt::adt(adt),
                esc(&if !dir.is_empty() {
                    format!("{dir} only")
                } else if js::truthy(g("directional")) {
                    fmt::DASH.to_string()
                } else {
                    "two-way".to_string()
                }),
                hv.map_or(fmt::DASH.to_string(), |v| rfmt::pct(v.to_number())),
                match (&cr.t, cr.am) {
                    (Some(t), true) => format!("{}, {}", esc(t), rfmt::veh(peak)),
                    _ => fmt::DASH.to_string(),
                },
                match (&cr.t, cr.am) {
                    (Some(t), false) => format!("{}, {}", esc(t), rfmt::veh(peak)),
                    _ => fmt::DASH.to_string(),
                },
                if cr.t.is_some() && g("adt").is_some_and(JsonValue::truthy) {
                    rfmt::pct(peak / adt * 100.0)
                } else {
                    fmt::DASH.to_string()
                },
                cr.growth.map_or(fmt::DASH.to_string(), rfmt::pct),
            ])
        })
        .collect();
    let id = c.r.scenario(sc_of(c, c.sel.existing)).id.clone();
    let bounds = study_bounds(r, &c.sel);
    let mut f = fig("counts", "Traffic count sites", "class", &id, bounds, show(&[("counts", true), ("crashes", false)]));
    f.source = Some(format!("{}; {}", attrib::NZTA, attrib::AT));
    format!(
        "{}\n      {}\n      {}",
        para(&esc(&peak_statement(r))),
        c.table(
            "Table E2 \u{b7} Traffic volumes",
            Tbl::new(
                vec![
                    h("Road"),
                    h("Location"),
                    h("Source / site ref"),
                    h("Count date"),
                    h("Duration"),
                    hn("ADT/AADT vpd"),
                    h("Direction"),
                    hn("HV %"),
                    h("AM peak start, veh/h"),
                    h("PM peak start, veh/h"),
                    hn("Peak % of daily"),
                    hn("Linear growth %/yr"),
                ],
                rows
            )
            .empty("No counts loaded. Load NZTA and AT counts in the Traffic data tab."),
            &TableOpts::src_note(
                &format!("{}; {}", attrib::NZTA, esc(attrib::AT)),
                "ADT to the nearest 10 vpd. Growth is linear, from the NZTA five-year AADT history where available. 24H flags a one-day count."
            )
        ),
        c.figure(f)
    )
}

/// `j2Table(P, c, sc, title)`: one scenario's junction table.
pub(crate) fn j2_table(c: &mut Ctx, sc: usize, title: &str) -> String {
    let r = c.r;
    let sel = c.sel.clone();
    let rows = super::shared::junction_rows(r, &sel, sc, None);
    let pers = r.rep_periods(sc);
    let mut ids: Vec<String> = Vec::new();
    for x in &rows {
        if !ids.contains(&x.id) {
            ids.push(x.id.clone());
        }
    }
    let scn = r.scenario(sc);
    if rows.is_empty() {
        return note(&format!("{} has no junction results. Run the model.", esc(r.sc(sc).name())), false);
    }
    let ri = match c.refs.iter().position(|(k, _)| *k == scn.id) {
        Some(i) => i,
        None => {
            c.refs.push((scn.id.clone(), Vec::new()));
            c.refs.len() - 1
        }
    };
    let mut body: Vec<Row> = Vec::new();
    for id in &ids {
        let reff = match c.refs[ri].1.iter().find(|(k, _)| k == id) {
            Some((_, v)) => v.clone(),
            None => {
                let v = format!("J{}", c.refs[ri].1.len() + 1);
                c.refs[ri].1.push((id.clone(), v.clone()));
                v
            }
        };
        let first = rows.iter().find(|x| x.id == *id).expect("a row");
        let ctl = match &first.s {
            Some(s) => control_label(&s.kind),
            None => fmt::DASH.to_string(),
        };
        let mut cs = vec![reff, esc(&first.name), esc(&ctl)];
        for p in &pers {
            let s = rows.iter().find(|x| x.id == *id && x.p == *p).and_then(|x| x.s.as_ref());
            match s {
                Some(s) => {
                    cs.push(format!(
                        "{}{}",
                        rfmt::dos(s.dos),
                        if s.dos > (if s.xp == 0.0 || s.xp.is_nan() { 1.0 } else { s.xp }) { " <b class=\"bad\">!</b>" } else { "" }
                    ));
                    cs.push(rfmt::delay(s.delay));
                    cs.push(esc(&s.los_text));
                    cs.push(format!(
                        "{}{}",
                        rfmt::q(s.q95),
                        if s.leg.is_empty() { String::new() } else { format!(" <span class=\"muted\">({})</span>", esc(&s.leg)) }
                    ));
                }
                None => {
                    for _ in 0..4 {
                        cs.push(fmt::DASH.to_string());
                    }
                }
            }
        }
        body.push(Row::id(cs, &format!("j-{}-{}", scn.id, id)));
    }
    let mut head = vec![h("Ref"), h("Junction"), h("Control")];
    for p in &pers {
        head.push(hn(&format!("{p} worst DoS")));
        head.push(hn(&format!("{p} delay s")));
        head.push(h(&format!("{p} LoS")));
        head.push(h(&format!("{p} longest Q95 m (leg)")));
    }
    let r0 = pers.first().and_then(|p| r.results[sc].get(p));
    c.table(
        &format!(
            "{} \u{b7} {} ({})",
            title,
            esc(r.sc(sc).name()),
            r.year_of(sc).map_or("null".to_string(), number_to_js_string)
        ),
        Tbl::new(head, body),
        &TableOpts {
            junction: true,
            los_label: Some(r0.map_or(String::new(), |x| x.junction.label.clone())),
            note: Some(format!(
                "Delay: volume-weighted average for signals and roundabouts, worst movement for give-way and stop. ! = over the practical DoS (signals 0.90, roundabouts 0.85, give-way 0.80). Queues are whole metres. Sites: {}",
                esc(&assessed_sites(r, sel.with).basis)
            )),
            ..TableOpts::default()
        },
    )
}

/// `report-speeds`.
pub(crate) fn speeds(c: &mut Ctx) -> String {
    let r = c.r;
    let sc = sc_of(c, c.sel.existing);
    let scn = r.scenario(sc);
    let pers = r.rep_periods(sc);
    struct Part<'a> {
        name: String,
        v: Option<f64>,
        src: String,
        links: Vec<&'a Link>,
    }
    let mut secs: Vec<Part> = Vec::new();
    for g in road_groups(scn).iter().take(30) {
        // one section per road and speed limit, so '% of limit' is always against that section's own limit
        let mut by: Vec<(String, Option<f64>, String, Vec<&Link>, f64)> = Vec::new();
        for l in &g.links {
            let s = speed_of(l);
            let k = format!("{}|{}", s.v.map_or("null".to_string(), number_to_js_string), s.src);
            let i = match by.iter().position(|x| x.0 == k) {
                Some(i) => i,
                None => {
                    by.push((k, s.v, s.src.clone(), Vec::new(), 0.0));
                    by.len() - 1
                }
            };
            by[i].3.push(l);
            by[i].4 += scn.link_len(l) / 1000.0;
        }
        js_sort_by(&mut by, |a, b| b.4 - a.4);
        let multi = by.len() > 1;
        for x in by {
            secs.push(Part {
                name: if multi {
                    format!("{} ({} km at {} km/h)", g.name, rfmt::num(x.4, 2), x.1.map_or("?".to_string(), number_to_js_string))
                } else {
                    g.name.clone()
                },
                v: x.1,
                src: x.2,
                links: x.3,
            });
        }
    }
    let rows: Vec<Row> = secs
        .iter()
        .take(40)
        .map(|g| {
            let mod_: Vec<Option<f64>> = pers
                .iter()
                .map(|p| {
                    let res = r.results[sc].get(p).expect("period");
                    let (mut s, mut n) = (0.0, 0.0);
                    for l in &g.links {
                        for d in ["ab", "ba"] {
                            if let Some(a) = res.arcs.get(&format!("{}:{}", l.id, d))
                                && a.v > 0.0
                            {
                                s += a.spd * a.v;
                                n += a.v;
                            }
                        }
                    }
                    if n != 0.0 { Some(s / n) } else { None }
                })
                .collect();
            let obs = g
                .links
                .iter()
                .map(|l| l.extra.get("obsSpeed").map_or(f64::NAN, crate::jsnum::value_to_number))
                .find(|v| *v > 0.0);
            let lim_v = g.v;
            let mut cs = vec![
                esc(&g.name),
                format!("{} ({})", lim_v.map_or(fmt::DASH.to_string(), |v| rfmt::num(v, 0)), esc(&g.src)),
            ];
            cs.extend(mod_.iter().map(|v| v.map_or(fmt::DASH.to_string(), |x| rfmt::num(x, 0))));
            cs.extend(mod_.iter().map(|v| match (v, lim_v) {
                (Some(x), Some(l)) if l > 0.0 => rfmt::pct(x / l * 100.0),
                _ => fmt::DASH.to_string(),
            }));
            cs.push(match obs {
                Some(o) if o != 0.0 => rfmt::num(o, 0),
                _ => fmt::DASH.to_string(),
            });
            cs.push(fmt::DASH.to_string());
            cs.push(fmt::DASH.to_string());
            Row::new(cs)
        })
        .collect();
    let mut head = vec![h("Section"), h("Limit km/h (source)")];
    head.extend(pers.iter().map(|p| hn(&format!("Modelled {p} km/h"))));
    head.extend(pers.iter().map(|p| hn(&format!("{p} % of limit"))));
    head.push(hn("Observed mean km/h"));
    head.push(h("ANOP LoS"));
    head.push(h("85th percentile"));
    let t1 = c.table(
        "Table E3 \u{b7} Speeds",
        Tbl::new(head, rows).empty("No results for the existing scenario. Run it."),
        &TableOpts {
            note: Some("Modelled speeds are flow-weighted means from the existing scenario (estimated, not observed). Observed speeds and 85th percentiles come from surveys or AT speed data when loaded; ANOP LoS from the AT ANOP tool.".to_string()),
            d: vec!["D5"],
            ..TableOpts::default()
        },
    );
    let figs = if let Some(p0) = pers.first() {
        let bands = js::path(Some(r.raw), &["settings", "display", "speedBands"]).filter(|v| v.truthy());
        let hcm = bands.map_or("anop".to_string(), JsonValue::js_string) == "hcm";
        let mut f = fig(
            &format!("speed-{}-{}", scn.id, p0),
            &format!(
                "Modelled speed as a share of the {}, {}, {} peak hour {} (indicative)",
                if hcm { "free-flow speed" } else { "speed limit" },
                r.sc(sc).name(),
                p0,
                r.year_of(sc).map_or("null".to_string(), number_to_js_string)
            ),
            "speed",
            &scn.id,
            study_bounds(r, &c.sel),
            show(&[("counts", false), ("crashes", false)]),
        );
        f.period = Some(p0.to_string());
        f.source = Some(
            if scn.links.values().any(|l| speed_of(l).src == "NSLR") {
                attrib::NSLR.to_string()
            } else {
                "Speed limits: coded speeds (NSLR not loaded)".to_string()
            },
        );
        c.figure(f)
    } else {
        String::new()
    };
    let t2 = j2_table(c, sc, "Table J2 \u{b7} Existing junction performance");
    format!("{}\n      {}\n      {}", t1, figs, t2)
}

fn _unused(_: Head, _: &[&str], _: Maybe<JsNum>) -> Vec<String> {
    let _ = (cells::<&str>, int, int_f, year_txt, cx::ID, ctext, cnum, kit_junction_name, crash_years, junction_groups, links_at);
    Vec::new()
}

/// A crash site assessed in the report (`crashSites`): the counts within 50 m, the leg volumes and the risk bands.
#[derive(Clone, Debug)]
pub struct CrashSite {
    /// First node.
    pub id: String,
    /// Assessed nodes.
    pub ids: Vec<String>,
    /// Member nodes.
    pub nodes: Vec<String>,
    /// Name (with the zones it gives access to).
    pub name: String,
    /// Zones the junction gives access to.
    pub access: Vec<String>,
    /// Control, when set.
    pub control: Option<String>,
    /// Speed limit (km/h), 0 when unknown.
    pub speed: f64,
    /// Fatal.
    pub f: f64,
    /// Serious.
    pub s: f64,
    /// Minor.
    pub m: f64,
    /// Non-injury.
    pub n: f64,
    /// Injury crashes.
    pub injury: f64,
    /// Pedestrian injury crashes.
    pub p: f64,
    /// Cyclist injury crashes.
    pub c: f64,
    /// Motorcyclist injury crashes.
    pub mc: f64,
    /// DSI equivalents over five years.
    pub dsi_eq: f64,
    /// The crashes within 50 m: crash index and distance.
    pub list: Vec<(usize, f64)>,
    /// Legs with their volumes.
    pub legs: Vec<CrashLeg>,
    /// Product of flows.
    pub pof: Option<f64>,
    /// Personal risk.
    pub pr: Option<f64>,
    /// Personal band.
    pub pband: super::calc::PersonalBand,
    /// Collective band.
    pub collective: &'static str,
    /// A volume is estimated from the model.
    pub estimated: bool,
    /// Speed note.
    pub speed_flag: String,
}

/// A leg of a crash site.
#[derive(Clone, Debug)]
pub struct CrashLeg {
    /// First link id.
    pub id: String,
    /// Road name.
    pub name: String,
    /// Two-way AADT.
    pub aadt: Option<f64>,
    /// Source of the volume.
    pub src: Option<String>,
}

/// A leg of a junction: its links and name (`externalLegs`).
pub(crate) struct ExtLeg<'a> {
    pub links: Vec<&'a Link>,
    pub name: String,
}

/// `externalLegs(sc, members)`: the links that leave a junction of one or more nodes (ring links inside it are left out), with
/// the two one-way carriageways of one road on one side merged into one leg.
pub(crate) fn external_legs<'a>(sc: &'a Scenario, members: &[String]) -> Vec<ExtLeg<'a>> {
    let inside: std::collections::HashSet<&str> = members.iter().map(String::as_str).collect();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let ns: Vec<&crate::project::Node> = members.iter().filter_map(|id| sc.nodes.get(id)).collect();
    let n = ns.len() as f64;
    let ctr = [ns.iter().fold(0.0, |s, x| s + x.lat.num()) / n, ns.iter().fold(0.0, |s, x| s + x.lng.num()) / n];
    let one_way = |l: &Link| !(l.lanes_ab.int32() > 0 && l.lanes_ba.int32() > 0);
    struct G<'a> {
        links: Vec<&'a Link>,
        name: String,
        nm: String,
        b: f64,
    }
    let mut legs: Vec<G> = Vec::new();
    for id in members {
        for l in links_at(sc, id) {
            if l.status.is("closed") || seen.contains(l.id.as_str()) || (inside.contains(l.a.as_str()) && inside.contains(l.b.as_str())) {
                continue;
            }
            seen.insert(&l.id);
            let far = sc.nodes.get(if inside.contains(l.a.as_str()) { &l.b } else { &l.a });
            let b = far.map_or(0.0, |f| util::bearing(&ctr, &f.ll()));
            let nm = crate::jsnum::js_trim(l.name.as_str().unwrap_or("")).to_lowercase();
            let pos = legs.iter().position(|g| {
                !nm.is_empty() && g.nm == nm && one_way(l) && g.links.iter().all(|x| one_way(x)) && util::ang_diff(g.b, b) < 45.0
            });
            match pos {
                Some(p) => legs[p].links.push(l),
                None => legs.push(G {
                    links: vec![l],
                    name: l.name.as_str().unwrap_or("").to_string(),
                    nm,
                    b,
                }),
            }
        }
    }
    legs.into_iter().map(|g| ExtLeg { links: g.links, name: g.name }).collect()
}
