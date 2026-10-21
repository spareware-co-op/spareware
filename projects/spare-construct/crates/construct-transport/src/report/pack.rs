//! The ITA data pack: every table of the report as UTF-8 CSV, with the network, settings and project file (`packFiles`).
//! The prototype zips the files; [`super::zip`] writes the same archive.

use super::app_support::assumption_rows;
use super::build::BuildOpts;
use super::ctx::Ctx;
use super::host::ExportOutcome;
use super::js;
use super::pack_csv::{Cell, csv_text, slug};
use super::pack_readme::{project_json, readme};
use super::sections_env::{aup_of, study_bounds};
use super::sections_front::vintage_rows;
use super::shared::{cnum, ctext, cx, j_sum, kit_junction_name, site_name, speed_of};
use super::texts::{COLS, PY};
use super::Rep;
use crate::analysis::{ExtentOpts, dev_distribution, extent_test};
use crate::edit::json::JsonValue;
use crate::jsnum::{math_round, number_to_js_string};
use crate::junction_tools::SidraOpts;
use crate::project::{land_use, road_class};
use crate::results::{iso_day_of_ms};
use crate::util::{clamp, r1, r3};

/// One file of the data pack: its name, its text and where it comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct PackFile {
    /// Path in the archive.
    pub name: String,
    /// The file's text.
    pub data: String,
    /// For a per-scenario file: the combined table it holds rows of.
    pub of: Option<String>,
    /// The SIDRA hand-over file.
    pub handover: bool,
}

/// The files of the data pack and what could not be included.
#[derive(Clone, Debug, PartialEq)]
pub struct PackOutput {
    /// The files, README first.
    pub files: Vec<PackFile>,
    /// What the README lists as not included.
    pub missing: Vec<String>,
}

/// `new Date(ms).toISOString()`: the full UTC date and time.
pub fn iso_full(ms: f64) -> String {
    let day = iso_day_of_ms(ms).unwrap_or_default();
    let t = ms.trunc().rem_euclid(86_400_000.0) as i64;
    format!("{day}T{:02}:{:02}:{:02}.{:03}Z", t / 3_600_000, t / 60_000 % 60, t / 1000 % 60, t % 1000)
}

fn d1_line(name: &str) -> String {
    format!("# Spare Construct ITA data pack, {name}. {}", super::consts::disclaimer_text("D1", None, None))
}

/// `csvFile(name, rows, cols)`: a BOM, Disclaimer D1 as the first row, then the header and rows.
pub fn csv_file(name: &str, cols: &[&str], rows: &[Vec<Cell>]) -> String {
    let mut all: Vec<Vec<Cell>> = vec![vec![Cell::S(d1_line(name))], cols.iter().map(|c| Cell::s(c)).collect()];
    all.extend(rows.iter().cloned());
    format!("\u{feff}{}\r\n", csv_text(&all))
}

fn cols_of(name: &str) -> Vec<&'static str> {
    COLS.iter().find(|(f, _)| *f == name).map(|(_, c)| c.iter().map(|(k, _)| *k).collect()).unwrap_or_default()
}

fn t(s: &str) -> Cell {
    Cell::s(s)
}

fn n(x: f64) -> Cell {
    Cell::N(x)
}

impl<'a> Rep<'a> {
    /// `Report.packFiles(P)`: the data pack's files, README first.
    pub fn pack_files(&self) -> PackOutput {
        let mut ctx: Ctx = self.context(&BuildOpts::default());
        let sel = ctx.sel.clone();
        let s_set = self.st();
        let nscs = self.project.scenarios.len();
        let bounds = study_bounds(self, &sel);
        let in_bounds = |l: &crate::project::Link| -> Cell {
            let Some(b) = bounds.as_ref() else {
                return t("");
            };
            let pts = l.pts.get().filter(|p| !p.is_empty());
            let Some(p) = pts.map(|p| p[(p.len() as f64 / 2.0).floor() as usize].0) else {
                return t("");
            };
            let g = |i: usize, j: usize| js::num(js::idx(js::idx(Some(b), i), j));
            Cell::B(p[0] >= g(0, 0) && p[0] <= g(1, 0) && p[1] >= g(0, 1) && p[1] <= g(1, 1))
        };
        let mut files: Vec<PackFile> = Vec::new();
        let mut missing: Vec<String> = Vec::new();
        let add = |files: &mut Vec<PackFile>, name: &str, rows: &[Vec<Cell>]| {
            files.push(PackFile { name: name.to_string(), data: csv_file(name, &cols_of(name), rows), of: None, handover: false });
        };
        // scenarios
        let mut rows: Vec<Vec<Cell>> = Vec::new();
        for i in 0..nscs {
            let s = self.scenario(i);
            for p in self.rep_periods(i) {
                let r = self.results[i].get(p).expect("results");
                let k = &r.kpi;
                rows.push(vec![
                    Cell::S(s.id.clone()),
                    Cell::S(s.name.as_str().unwrap_or("").to_string()),
                    Cell::S(s.code.as_str().unwrap_or("").to_string()),
                    Cell::S(s.role.as_str().unwrap_or("").to_string()),
                    n(r.year),
                    t(p),
                    n(r1(k.trips)),
                    n(r1(k.vkt)),
                    n(r1(k.vht)),
                    n(r1(k.dly)),
                    n(r1(k.spd)),
                    n(r3(k.co2)),
                    n(r.it),
                    n(r.gap),
                    Cell::B(r.converged),
                    if r.ts != 0.0 && !r.ts.is_nan() { Cell::S(iso_full(r.ts)) } else { t("") },
                    Cell::B(r.stale),
                ]);
            }
        }
        add(&mut files, "scenarios.csv", &rows);
        // links, with the extent comparison against the without scenario for the with and mitigation scenarios
        let mut lr: Vec<Vec<Cell>> = Vec::new();
        for i in 0..nscs {
            let s = self.scenario(i);
            for p in self.rep_periods(i) {
                let a_up = aup_of(self, i);
                let r = self.results[i].get(p).expect("results");
                let wo = if (Some(i) == sel.with || Some(i) == sel.mitigation) && sel.without.is_some() {
                    sel.without.and_then(|w| self.results[w].get(p))
                } else {
                    None
                };
                let ext = wo.and_then(|wr| {
                    extent_test(s, Some(wr), Some(r), ExtentOpts { sc_without: sel.without.map(|w| self.scenario(w)), ..ExtentOpts::default() })
                });
                for l in s.links.values() {
                    for d in ["ab", "ba"] {
                        let lanes = if d == "ab" { &l.lanes_ab } else { &l.lanes_ba };
                        if !lanes.truthy() {
                            continue;
                        }
                        let Some(a) = r.arcs.get(&format!("{}:{d}", l.id)) else {
                            continue;
                        };
                        let sp = speed_of(l);
                        let x = ext.as_ref().and_then(|e| e.rows.iter().find(|q| q.key == format!("{}:{d}", l.id)));
                        let fcv = |k: &str| -> Cell {
                            match l.fc.get().and_then(|f| f.get(k)) {
                                Some(v) if crate::jsnum::value_truthy(v) => Cell::from_json(Some(&JsonValue::from_value(v))),
                                _ => t(""),
                            }
                        };
                        let cap_per = l.cap_per_lane.num();
                        let class_cap = road_class(l.cls.as_str()).cap;
                        let cap = (if cap_per == 0.0 || cap_per.is_nan() { class_cap } else { cap_per }) * lanes.num();
                        lr.push(vec![
                            Cell::S(s.id.clone()),
                            t(p),
                            Cell::S(l.id.clone()),
                            Cell::S(d.to_uppercase()),
                            Cell::S(l.name.as_str().unwrap_or("").to_string()),
                            Cell::S(if d == "ab" { l.a.clone() } else { l.b.clone() }),
                            Cell::S(if d == "ab" { l.b.clone() } else { l.a.clone() }),
                            match &l.cls {
                                crate::jsnum::Maybe::Set(c) => Cell::S(c.clone()),
                                _ => Cell::Null,
                            },
                            fcv("gt"),
                            fcv("pt"),
                            fcv("freight"),
                            fcv("cycle"),
                            a_up.art(l).map_or(t(""), Cell::B),
                            Cell::from_num(lanes),
                            sp.v.map_or(Cell::Null, n),
                            Cell::S(sp.src.clone()),
                            n(math_round(a.len)),
                            n(math_round(a.v)),
                            n(math_round(a.pcu)),
                            Cell::from_num(&s_set.hv_pct),
                            n(math_round(a.cap)),
                            n(cap),
                            t(if l.cap_per_lane.truthy() { "user" } else { "class default" }),
                            n(a.vc),
                            n(r1(a.spd)),
                            match sp.v {
                                Some(v) if v > 0.0 => n(r1(a.spd / v * 100.0)),
                                _ => t(""),
                            },
                            t(""),
                            n(r1(a.nd)),
                            x.map_or(t(""), |x| n(math_round(x.without))),
                            x.map_or(t(""), |x| n(math_round(x.d))),
                            match x {
                                Some(x) if x.p.is_some() => n(r1(x.p.unwrap_or(0.0) * 100.0)),
                                _ => t(""),
                            },
                            match x {
                                Some(x) if x.threshold.is_some() => n(x.threshold.unwrap_or(0.0) * 100.0),
                                _ => t(""),
                            },
                            in_bounds(l),
                        ]);
                    }
                }
            }
        }
        add(&mut files, "links.csv", &lr);
        // junctions
        let (mut js_rows, mut jm, mut tm): (Vec<Vec<Cell>>, Vec<Vec<Cell>>, Vec<Vec<Cell>>) = (Vec::new(), Vec::new(), Vec::new());
        for i in 0..nscs {
            let s = self.scenario(i);
            for p in self.rep_periods(i) {
                let r = self.results[i].get(p).expect("results");
                for (id, nr) in r.nodes.iter() {
                    if nr.r#type == "none" {
                        continue;
                    }
                    let j = j_sum(nr);
                    let nm = site_name(s, Some(r), id);
                    let site = nr.site.clone().filter(|x| !x.is_empty()).unwrap_or_else(|| id.to_string());
                    js_rows.push(vec![
                        Cell::S(s.id.clone()),
                        t(p),
                        Cell::S(site.clone()),
                        Cell::S(nm.clone()),
                        Cell::S(nr.r#type.clone()),
                        match nr.c {
                            Some(c) if c != 0.0 && !c.is_nan() => n(c),
                            _ => t(""),
                        },
                        n(math_round(if nr.vol == 0.0 || nr.vol.is_nan() { 0.0 } else { nr.vol })),
                        n(r3(nr.dos)),
                        n(r1(j.delay)),
                        Cell::S(nr.los.clone()),
                        Cell::S(j.basis.clone()),
                        n(math_round(j.q95)),
                        Cell::S(j.leg.clone()),
                    ]);
                    for a in &nr.appr {
                        let l = s.links.get(&a.link);
                        jm.push(vec![
                            Cell::S(s.id.clone()),
                            t(p),
                            Cell::S(site.clone()),
                            Cell::S(nm.clone()),
                            Cell::S(a.approach.clone()),
                            Cell::S(a.name.clone()),
                            Cell::S(a.link.clone()),
                            n(math_round(a.v)),
                            n(a.cap),
                            n(r3(a.dos)),
                            n(r1(a.delay)),
                            Cell::S(a.los.clone()),
                            n(a.q95veh),
                            n(a.q95),
                            a.psc.map_or(t(""), n),
                            l.map_or(t(""), |l| n(math_round(s.link_len(l)))),
                        ]);
                    }
                }
                for (id, list) in r.turns.iter() {
                    if !s.nodes.contains_key(id) {
                        continue;
                    }
                    let mv = crate::turns::movements(s, Some(r), s_set, id);
                    let mut code: Vec<(String, &crate::turns::MovementRow)> = Vec::new();
                    for m in &mv {
                        let key = format!("{}>{}", m.approach, m.to);
                        match code.iter_mut().find(|(k, _)| *k == key) {
                            Some(e) => e.1 = m,
                            None => code.push((key, m)),
                        }
                    }
                    for tr in list {
                        let m = code.iter().find(|(k, _)| *k == format!("{}>{}", tr.from, tr.to)).map(|(_, m)| *m);
                        tm.push(vec![
                            Cell::S(s.id.clone()),
                            t(p),
                            Cell::S(id.to_string()),
                            Cell::S(kit_junction_name(s, id)),
                            Cell::S(tr.from.clone()),
                            Cell::S(tr.to.clone()),
                            Cell::S(tr.mv.clone()),
                            Cell::S(m.map_or(String::new(), |m| m.leg_name.to_string())),
                            Cell::S(m.map_or(String::new(), |m| m.turn.to_string())),
                            Cell::S(m.map_or(String::new(), |m| m.id.clone())),
                            n(r1(tr.v)),
                            n(r1(tr.pcu)),
                            tr.dev.map_or(t(""), |d| n(r1(d))),
                        ]);
                    }
                }
            }
        }
        add(&mut files, "junction_summary.csv", &js_rows);
        add(&mut files, "junction_movements.csv", &jm);
        add(&mut files, "turning_movements.csv", &tm);
        // one file per scenario and period as well, named with the period and year
        let mut used: Vec<String> = Vec::new();
        let project_slug = slug(self.name());
        for i in 0..nscs {
            let s = self.scenario(i);
            for p in self.rep_periods(i) {
                let res = self.results[i].get(p).expect("results");
                let y: String = if res.year > 0.0 {
                    number_to_js_string(res.year)
                } else {
                    self.year_of(i).map_or("year".to_string(), number_to_js_string)
                };
                let sname = s.name.as_str().unwrap_or("");
                let b0 = [
                    if project_slug.is_empty() { "project".to_string() } else { project_slug.clone() },
                    {
                        let a = slug(sname);
                        if !a.is_empty() {
                            a
                        } else {
                            let b = slug(&s.id);
                            if b.is_empty() { "scenario".to_string() } else { b }
                        }
                    },
                    p.to_lowercase(),
                    y.clone(),
                ]
                .join("-");
                let mut base = b0.clone();
                let mut k = 1;
                while used.contains(&base) {
                    k += 1;
                    base = format!("{b0}-{k}");
                }
                used.push(base.clone());
                for (table, rows) in [("links.csv", &lr), ("junction_summary.csv", &js_rows), ("junction_movements.csv", &jm), ("turning_movements.csv", &tm)] {
                    let name = format!("by-scenario/{base}-{}", table.replace('_', "-"));
                    let sub: Vec<Vec<Cell>> = rows
                        .iter()
                        .filter(|r| r[0] == Cell::S(s.id.clone()) && r[1] == t(p))
                        .cloned()
                        .collect();
                    let comment = format!(
                        "# Spare Construct ITA data pack, {name}: {sname}, {p} peak hour, {y} (the columns of {table}). {}",
                        super::consts::disclaimer_text("D1", None, None)
                    );
                    let mut all: Vec<Vec<Cell>> = vec![vec![Cell::S(comment)], cols_of(table).iter().map(|c| t(c)).collect()];
                    all.extend(sub);
                    files.push(PackFile { name, data: format!("\u{feff}{}\r\n", csv_text(&all)), of: Some(table.to_string()), handover: false });
                }
            }
        }
        // trip generation and distribution
        let mut tg: Vec<Vec<Cell>> = Vec::new();
        for i in 0..nscs {
            let s = self.scenario(i);
            for p in ["AM", "PM"] {
                for z in s.zones.values() {
                    if z.kind.is("gateway") {
                        let gf = crate::standards::growth(s_set, Some(s));
                        let g = |suffix: &str| -> f64 {
                            let v = z.gw.get().and_then(|m| m.get(&format!("{p}{suffix}"))).map_or(0.0, |x| x.num());
                            (if v == 0.0 || v.is_nan() { 0.0 } else { v }) * gf
                        };
                        tg.push(vec![
                            Cell::S(s.id.clone()),
                            t(p),
                            Cell::S(z.id.clone()),
                            Cell::S(z.name.as_str().unwrap_or("").to_string()),
                            t("external"),
                            t("gateway"),
                            t(""),
                            t(""),
                            t(""),
                            t(""),
                            t(""),
                            t(""),
                            t(""),
                            n(r1(g("in"))),
                            n(r1(g("out"))),
                        ]);
                        continue;
                    }
                    let red = clamp(z.reduction.or(0.0), 0.0, 100.0);
                    let st = crate::demand::stage_of(Some(z), Some(s));
                    let open = !(z.year.num() > crate::standards::year_of(Some(s), s_set));
                    let Some(lu) = z.lu.get() else {
                        continue;
                    };
                    for (k, q) in lu.iter() {
                        let rr = s_set.rates.get().and_then(|m| m.get(k)).and_then(|pr| pr.get()).and_then(|pr| pr.get(p)).and_then(|r| r.get());
                        let (Some(rr), Some(l)) = (rr, land_use(k)) else {
                            continue;
                        };
                        let qn = q.num();
                        if !(qn > 0.0) {
                            continue;
                        }
                        let gross = qn / l.per * rr.rate.num();
                        let net = if open { gross * (1.0 - red / 100.0) * st } else { 0.0 };
                        let inpct = rr.in_pct.num();
                        tg.push(vec![
                            Cell::S(s.id.clone()),
                            t(p),
                            Cell::S(z.id.clone()),
                            Cell::S(z.name.as_str().unwrap_or("").to_string()),
                            Cell::S(crate::graph::zone_class(Some(z)).to_string()),
                            Cell::S(k.to_string()),
                            n(qn),
                            t(l.unit),
                            Cell::from_num(&rr.rate),
                            Cell::from_num(&rr.in_pct),
                            n(r1(gross)),
                            n(red),
                            n(r1(st * 100.0)),
                            n(r1(net * inpct / 100.0)),
                            n(r1(net * (1.0 - inpct / 100.0))),
                        ]);
                    }
                }
            }
        }
        add(&mut files, "trip_generation.csv", &tg);
        let mut dist: Vec<Vec<Cell>> = Vec::new();
        for i in 0..nscs {
            let rl: Vec<&crate::results::Res> = self.rep_periods(i).iter().filter_map(|p| self.results[i].get(p)).collect();
            if rl.is_empty() {
                continue;
            }
            let dd = dev_distribution(Some(self.scenario(i)), &rl);
            let s = self.scenario(i);
            for r in &dd.rows {
                let sh = |k: &str| -> Cell { n(r1(r.shares.get(k).copied().unwrap_or(0.0))) };
                dist.push(vec![
                    Cell::S(s.id.clone()),
                    Cell::S(r.id.clone()),
                    Cell::S(r.name.clone()),
                    sh("AMin"),
                    sh("AMout"),
                    sh("PMin"),
                    sh("PMout"),
                    r.vph.get("AM").map_or(t(""), |v| n(*v)),
                    r.vph.get("PM").map_or(t(""), |v| n(*v)),
                ]);
            }
        }
        add(&mut files, "distribution.csv", &dist);
        // calibration
        let (mut cc, mut cs): (Vec<Vec<Cell>>, Vec<Vec<Cell>>) = (Vec::new(), Vec::new());
        let cal_i = sel.existing.or(sel.base).unwrap_or(0);
        let cal0 = self.scenario(cal_i);
        let data = crate::datahub::Data::read(self.project);
        let raw_counts = js::items(js::get(self.data(), "counts"));
        for p in self.rep_periods(cal_i) {
            let cal = crate::datahub::calibration(cal0, self.results[cal_i].get(p), p, s_set, &data);
            if let Some(cal) = cal {
                for x in &cal.rows {
                    let rawc = raw_counts.iter().find(|k| js::get(Some(k), "id").and_then(JsonValue::as_str) == Some(x.c.id.as_str()));
                    let rg = |k: &str| Cell::or_blank(js::get(rawc, k));
                    cc.push(vec![
                        Cell::S(cal0.id.clone()),
                        t(p),
                        Cell::S(x.c.id.clone()),
                        Cell::from_json(js::get(rawc, "src")),
                        rg("ref"),
                        rg("name"),
                        rg("road"),
                        rg("date"),
                        rg("dur"),
                        rg("adt"),
                        n(r1(x.obs)),
                        n(r1(x.model)),
                        if x.geh.is_nan() { t("") } else { n(r3(x.geh)) },
                        Cell::S(x.how.clone()),
                        Cell::S(x.mt.link.clone()),
                        Cell::S(x.mt.dir.to_string()),
                    ]);
                }
            }
            let rep = crate::calibrate::report(self.project, cal0, p, self.results[cal_i].get(p));
            for k in &rep.checks {
                let k = k.to_value();
                let g = |f: &str| k.get(f).cloned();
                let or_blank = |f: &str| match g(f) {
                    Some(serde_json::Value::String(s)) => Cell::S(s),
                    _ => t(""),
                };
                cs.push(vec![
                    Cell::S(cal0.id.clone()),
                    t(p),
                    Cell::from_json(g("id").map(|v| JsonValue::from_value(&v)).as_ref()),
                    Cell::from_json(g("label").map(|v| JsonValue::from_value(&v)).as_ref()),
                    or_blank("target"),
                    match g("value") {
                        Some(serde_json::Value::Number(x)) => n(r3(x.as_f64().unwrap_or(f64::NAN))),
                        Some(serde_json::Value::String(s)) => match s.as_str() {
                            "NaN" => n(f64::NAN),
                            "Infinity" => n(f64::INFINITY),
                            "-Infinity" => n(f64::NEG_INFINITY),
                            _ => Cell::S(s),
                        },
                        Some(serde_json::Value::Bool(b)) => Cell::B(b),
                        _ => t(""),
                    },
                    or_blank("status"),
                    or_blank("source"),
                    Cell::B(k.get("verified").is_some_and(crate::jsnum::value_truthy)),
                ]);
            }
        }
        add(&mut files, "counts_calibration.csv", &cc);
        add(&mut files, "calibration_summary.csv", &cs);
        let mut rt: Vec<Vec<Cell>> = Vec::new();
        for i in 0..nscs {
            let s = self.scenario(i);
            for p in self.rep_periods(i) {
                for (id, x) in self.results[i].get(p).expect("results").routes.iter() {
                    rt.push(vec![
                        Cell::S(s.id.clone()),
                        t(p),
                        Cell::S(id.to_string()),
                        Cell::S(s.routes.get(id).and_then(|r| r.name.as_str().map(str::to_string)).unwrap_or_default()),
                        n(math_round(x.len)),
                        n(r1(x.t)),
                        n(r1(x.spd.unwrap_or(f64::NAN))),
                    ]);
                }
            }
        }
        add(&mut files, "route_times.csv", &rt);
        // crashes
        let sites = super::sections_crash::crash_sites(&mut ctx);
        let mut near: Vec<Option<(String, f64)>> = Vec::new();
        let crashes = js::items(js::get(self.data(), "crashes"));
        near.resize(crashes.len(), None);
        for st in &sites {
            for (k, d) in &st.list {
                if near[*k].as_ref().is_none_or(|o| *d < o.1) {
                    near[*k] = Some((st.id.clone(), *d));
                }
            }
        }
        let cr_rows: Vec<Vec<Cell>> = crashes
            .iter()
            .enumerate()
            .map(|(idx, k)| {
                let nn = near[idx].as_ref();
                let g = |i: usize| js::idx(Some(k), i);
                let loc = js::text_or_empty(g(4));
                let mut parts = loc.split(" / ");
                let (l1, l2) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                let f = js::text_or_empty(g(5));
                let q = |i: usize| cnum(k, i).map_or(t(""), n);
                vec![
                    Cell::S(ctext(k, cx::ID)),
                    Cell::from_json(g(3)),
                    Cell::from_json(g(2)),
                    q(cx::FATAL),
                    q(cx::SERIOUS),
                    q(cx::MINOR),
                    match g(6) {
                        None | Some(JsonValue::Null) => t(""),
                        v => Cell::from_json(v),
                    },
                    Cell::S(ctext(k, cx::CONTROL)),
                    Cell::S(ctext(k, cx::URBAN)),
                    Cell::B(f.contains('p')),
                    Cell::B(f.contains('c')),
                    Cell::B(f.contains('m')),
                    Cell::S(ctext(k, cx::LIGHT)),
                    Cell::S(ctext(k, cx::WEATHER)),
                    Cell::S(l1.to_string()),
                    Cell::S(l2.to_string()),
                    nn.map_or(t(""), |o| Cell::S(o.0.clone())),
                    nn.map_or(t(""), |o| n(math_round(o.1))),
                    Cell::from_json(g(0)),
                    Cell::from_json(g(1)),
                ]
            })
            .collect();
        add(&mut files, "crashes.csv", &cr_rows);
        let cs_rows: Vec<Vec<Cell>> = sites
            .iter()
            .map(|s| {
                vec![
                    Cell::S(s.id.clone()),
                    Cell::S(s.name.clone()),
                    s.control.as_ref().map_or(Cell::Null, |c| Cell::S(c.clone())),
                    n(s.speed),
                    n(s.f),
                    n(s.s),
                    n(s.m),
                    n(s.n),
                    n(s.injury),
                    n(r3(s.dsi_eq)),
                    t(s.collective),
                    Cell::S(
                        s.legs
                            .iter()
                            .map(|l| match l.aadt {
                                Some(a) if a != 0.0 && !a.is_nan() => number_to_js_string(math_round(a)),
                                _ => String::new(),
                            })
                            .collect::<Vec<_>>()
                            .join(" / "),
                    ),
                    match s.pof {
                        Some(p) if p != 0.0 && !p.is_nan() => n(r1(p)),
                        _ => t(""),
                    },
                    s.pr.map_or(t(""), |p| n(r1(p))),
                    Cell::S(s.pband.label.to_string()),
                    Cell::B(s.pband.reliable),
                ]
            })
            .collect();
        add(&mut files, "crash_sites.csv", &cs_rows);
        let vint: Vec<Vec<Cell>> = vintage_rows(self).into_iter().map(|r| r.into_iter().map(Cell::S).collect()).collect();
        add(&mut files, "provenance.csv", &vint);
        let ass: Vec<Vec<Cell>> = assumption_rows()
            .into_iter()
            .map(|r| vec![t(r.group), Cell::S(r.name), Cell::S(r.edition), t(if r.corridor { "Spare Construct assumption" } else { "Published source" }), Cell::B(r.verified)])
            .collect();
        add(&mut files, "assumptions.csv", &ass);
        // OD matrices, SIDRA volumes, network, settings and the project file
        let od_sc = sel.with;
        if let Some(o) = od_sc {
            let sc = self.scenario(o);
            for p in self.rep_periods(o) {
                let r = self.results[o].get(p).expect("results");
                let Some(tm_) = r.t.as_ref().filter(|_| !r.zone_ids.is_empty()) else {
                    missing.push(format!("od_{p}.csv: the OD matrix is kept only for 150 zones or fewer"));
                    continue;
                };
                let z = r.zone_ids.len();
                let nm = |id: &str| -> String {
                    match sc.zones.get(id).and_then(|z| z.name.as_str()) {
                        Some(x) if !x.is_empty() => x.to_string(),
                        _ => id.to_string(),
                    }
                };
                let mut rows: Vec<Vec<Cell>> = vec![
                    vec![Cell::S(format!(
                        "# Spare Construct ITA data pack, OD matrix {} {} {} (veh/h). {}",
                        sc.name.as_str().unwrap_or(""),
                        p,
                        number_to_js_string(r.year),
                        super::consts::disclaimer_text("D1", None, None)
                    ))],
                    std::iter::once(t("from \\ to")).chain(r.zone_ids.iter().map(|id| Cell::S(nm(id)))).collect(),
                ];
                for i in 0..z {
                    rows.push(std::iter::once(Cell::S(nm(&r.zone_ids[i]))).chain((0..z).map(|j| n(r1(tm_[i * z + j])))).collect());
                }
                files.push(PackFile { name: format!("od_{p}.csv"), data: format!("\u{feff}{}\r\n", csv_text(&rows)), of: None, handover: false });
            }
        }
        // the SIDRA hand-over: the junctions feature's volumes file and SIDRA 11 loader script, the same files as its exports
        if let Some(o) = od_sc {
            let tools = self.tools();
            let today = iso_day_of_ms(self.now_ms).unwrap_or_default();
            let opts = SidraOpts { scope: "auto".to_string(), force: false, date_iso: today, date_long: long_date(self.now_ms) };
            let sc = tools.sc(o);
            let v = tools.volumes_csv(sc, None, None, &opts);
            let l = v.as_ref().ok().map(|_| tools.loader_py(sc, None, None, &opts));
            match (v, l) {
                (Ok(v), Some(Ok(l))) => {
                    if !crate::jsnum::js_trim(&v).is_empty() {
                        files.push(PackFile {
                            name: "corridor-sidra-volumes.csv".to_string(),
                            data: format!("\u{feff}{}", v.strip_prefix('\u{feff}').unwrap_or(&v)),
                            of: None,
                            handover: true,
                        });
                    } else {
                        missing.push("corridor-sidra-volumes.csv: the junctions feature produced no rows".to_string());
                    }
                    if !crate::jsnum::js_trim(&l).is_empty() {
                        files.push(PackFile { name: "load_corridor_volumes.py".to_string(), data: l, of: None, handover: false });
                    }
                }
                (Err(e), _) | (_, Some(Err(e))) => {
                    missing.push(format!("corridor-sidra-volumes.csv and load_corridor_volumes.py: {e}"));
                }
                _ => {}
            }
        }
        // anop.csv and screenlines.csv come from the features that make them (their registered exports)
        for (name, id, feature) in [("anop.csv", "compliance-extra-anop", "Compliance+"), ("screenlines.csv", "calibration-screenlines", "Calibration")] {
            match self.host.registered_export(id) {
                None => missing.push(format!("{name}: made by the {feature} feature, which is not installed")),
                Some(ExportOutcome::Disabled(label)) => missing.push(format!("{name}: nothing to export yet ({feature} \u{203a} {label})")),
                Some(ExportOutcome::Failed(msg)) => missing.push(format!("{name}: {msg}")),
                Some(ExportOutcome::Text(text)) => {
                    let mut text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_string();
                    if crate::jsnum::js_trim(&text).is_empty() {
                        missing.push(format!("{name}: {feature} produced no rows"));
                        continue;
                    }
                    loop {
                        if text.ends_with("\r\n") {
                            text.truncate(text.len() - 2);
                        } else if text.ends_with('\n') {
                            text.truncate(text.len() - 1);
                        } else {
                            break;
                        }
                    }
                    files.push(PackFile {
                        name: name.to_string(),
                        data: format!("\u{feff}{}\r\n{}\r\n", csv_text(&[vec![Cell::S(d1_line(name))]]), text),
                        of: None,
                        handover: false,
                    });
                }
            }
        }
        files.push(PackFile {
            name: "read_datapack.py".to_string(),
            data: format!("# {}\n{}", super::consts::disclaimer_text("D1", None, None), PY),
            of: None,
            handover: false,
        });
        {
            let gs = od_sc.unwrap_or(0);
            let gscn = self.scenario(gs);
            let res = self.results[gs].get(&self.period_name(&ctx.period)).or_else(|| self.rep_periods(gs).first().and_then(|p| self.results[gs].get(p)));
            let gj = crate::importers::export_geojson(gscn, res);
            let data = match JsonValue::parse(&gj) {
                Ok(o) => {
                    let mut out = JsonValue::object();
                    out.set("type", o.get("type").cloned().unwrap_or(JsonValue::Null));
                    out.set("name", JsonValue::String(format!("{}: {}", self.name(), gscn.name.as_str().unwrap_or(""))));
                    out.set("disclaimer", JsonValue::String(super::consts::disclaimer_text("D1", None, None)));
                    out.set("modelVersion", JsonValue::String(super::shared::model_version(self)));
                    if let JsonValue::Object(m) = &o {
                        for (k, v) in m.iter() {
                            out.set(k.to_string(), v.clone());
                        }
                    }
                    out.stringify()
                }
                Err(_) => gj,
            };
            files.push(PackFile { name: "network.geojson".to_string(), data, of: None, handover: false });
        }
        {
            let mut settings = js::get(Some(self.raw), "settings").cloned().unwrap_or(JsonValue::Null);
            if let JsonValue::Object(m) = &mut settings {
                m.remove("linzKey");
            }
            let mut o = JsonValue::object();
            o.set("settings", settings);
            o.set("modelVersion", JsonValue::String(super::shared::model_version(self)));
            o.set("disclaimer", JsonValue::String(super::consts::disclaimer_text("D1", None, None)));
            files.push(PackFile { name: "settings.json".to_string(), data: o.stringify_pretty(1), of: None, handover: false });
        }
        files.push(PackFile { name: "project.corridor.json".to_string(), data: project_json(self), of: None, handover: false });
        let rd = readme(self, &sel, &files, &missing);
        files.insert(0, PackFile { name: "README.txt".to_string(), data: rd, of: None, handover: false });
        PackOutput { files, missing }
    }

    fn period_name(&self, p: &str) -> String {
        p.to_string()
    }
}

/// `new Date().toLocaleDateString('en-NZ', { day: 'numeric', month: 'long', year: 'numeric' })`: "21 October 2026".
pub(crate) fn long_date(ms: f64) -> String {
    let iso = iso_day_of_ms(ms).unwrap_or_default();
    if iso.len() < 10 {
        return iso;
    }
    let (y, m, d) = (&iso[0..4], iso[5..7].parse::<usize>().unwrap_or(1), iso[8..10].parse::<usize>().unwrap_or(1));
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    format!("{d} {} {y}", MONTHS[m - 1])
}
