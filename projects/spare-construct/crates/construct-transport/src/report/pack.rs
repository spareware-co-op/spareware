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
use super::{CrashSite, Rep, Sel};
use crate::analysis::{ExtentOpts, dev_distribution, extent_test};
use crate::edit::json::JsonValue;
use crate::jsnum::{f64_to_i64, math_round, number_to_js_string};
use crate::junction_tools::SidraOpts;
use crate::project::{land_use, road_class};
use crate::results::{Res, iso_day_of_ms};
use crate::util::{clamp, r1, r3};

/// The rows of a table.
type Rows = Vec<Vec<Cell>>;

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
#[must_use]
pub fn iso_full(ms: f64) -> String {
    let day = iso_day_of_ms(ms).unwrap_or_default();
    let t = f64_to_i64(ms.trunc().rem_euclid(86_400_000.0));
    format!("{day}T{:02}:{:02}:{:02}.{:03}Z", t / 3_600_000, t / 60_000 % 60, t / 1000 % 60, t % 1000)
}

fn d1_line(name: &str) -> String {
    format!("# Spare Construct ITA data pack, {name}. {}", super::consts::disclaimer_text("D1", None, None))
}

/// `csvFile(name, rows, cols)`: a BOM, Disclaimer D1 as the first row, then the header and rows.
#[must_use]
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

fn add(files: &mut Vec<PackFile>, name: &str, rows: &[Vec<Cell>]) {
    files.push(PackFile { name: name.to_string(), data: csv_file(name, &cols_of(name), rows), of: None, handover: false });
}

// Scenario i's results for period p. The report's periods are those with results, so for them this is always Some.
fn res_of<'a>(rep: &Rep<'a>, i: usize, p: &str) -> Option<&'a Res> {
    rep.results.get(i)?.get(p)
}

// Whether the link's middle shape point lies in the study area's bounds (blank without bounds or shape points)
fn in_bounds(bounds: Option<&JsonValue>, l: &crate::project::Link) -> Cell {
    let Some(b) = bounds else {
        return t("");
    };
    let pts = l.pts.get().filter(|p| !p.is_empty());
    // floor(len / 2) of a non-empty list is always one of its indexes
    let Some(p) = pts.and_then(|p| p.get(p.len() / 2)).map(|pt| pt.0) else {
        return t("");
    };
    let g = |i: usize, j: usize| js::num(js::idx(js::idx(Some(b), i), j));
    Cell::B(p[0] >= g(0, 0) && p[0] <= g(1, 0) && p[1] >= g(0, 1) && p[1] <= g(1, 1))
}

impl Rep<'_> {
    /// `Report.packFiles(P)`: the data pack's files, README first.
    #[must_use]
    pub fn pack_files(&self) -> PackOutput {
        let mut ctx: Ctx = self.context(&BuildOpts::default());
        let sel = ctx.sel.clone();
        let bounds = study_bounds(self, &sel);
        let mut files: Vec<PackFile> = Vec::new();
        let mut missing: Vec<String> = Vec::new();
        add(&mut files, "scenarios.csv", &self.scenario_rows());
        let lr = self.link_rows(&sel, bounds.as_ref());
        add(&mut files, "links.csv", &lr);
        let (js_rows, jm) = self.junction_rows();
        let tm = self.turn_rows();
        add(&mut files, "junction_summary.csv", &js_rows);
        add(&mut files, "junction_movements.csv", &jm);
        add(&mut files, "turning_movements.csv", &tm);
        self.by_scenario_files(&mut files, [("links.csv", &lr), ("junction_summary.csv", &js_rows), ("junction_movements.csv", &jm), ("turning_movements.csv", &tm)]);
        add(&mut files, "trip_generation.csv", &self.trip_rows());
        add(&mut files, "distribution.csv", &self.distribution_rows());
        let (cc, cal_sum) = self.calibration_rows(&sel);
        add(&mut files, "counts_calibration.csv", &cc);
        add(&mut files, "calibration_summary.csv", &cal_sum);
        add(&mut files, "route_times.csv", &self.route_rows());
        // crashes
        let sites = super::sections_crash::crash_sites(&mut ctx);
        add(&mut files, "crashes.csv", &self.crash_rows(&sites));
        add(&mut files, "crash_sites.csv", &crash_site_rows(&sites));
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
            self.od_files(o, &mut files, &mut missing);
        }
        // the SIDRA hand-over: the junctions feature's volumes file and SIDRA 11 loader script, the same files as its exports
        if let Some(o) = od_sc {
            self.sidra_files(o, &mut files, &mut missing);
        }
        self.export_files(&mut files, &mut missing);
        files.push(PackFile {
            name: "read_datapack.py".to_string(),
            data: format!("# {}\n{}", super::consts::disclaimer_text("D1", None, None), PY),
            of: None,
            handover: false,
        });
        files.push(self.geojson_file(od_sc.unwrap_or(0), &ctx.period));
        files.push(self.settings_file());
        files.push(PackFile { name: "project.corridor.json".to_string(), data: project_json(self), of: None, handover: false });
        let rd = readme(self, &sel, &files, &missing);
        files.insert(0, PackFile { name: "README.txt".to_string(), data: rd, of: None, handover: false });
        PackOutput { files, missing }
    }

    // scenarios.csv
    fn scenario_rows(&self) -> Rows {
        let mut rows: Rows = Vec::new();
        for i in 0..self.project.scenarios.len() {
            let sc = self.scenario(i);
            for p in self.rep_periods(i) {
                let Some(r) = res_of(self, i, p) else {
                    continue;
                };
                let k = &r.kpi;
                rows.push(vec![
                    Cell::S(sc.id.clone()),
                    Cell::S(sc.name.as_str().unwrap_or("").to_string()),
                    Cell::S(sc.code.as_str().unwrap_or("").to_string()),
                    Cell::S(sc.role.as_str().unwrap_or("").to_string()),
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
        rows
    }

    // links.csv, with the extent comparison against the without scenario for the with and mitigation scenarios
    fn link_rows(&self, sel: &Sel, bounds: Option<&JsonValue>) -> Rows {
        let mut lr: Rows = Vec::new();
        for i in 0..self.project.scenarios.len() {
            for p in self.rep_periods(i) {
                self.period_link_rows(&mut lr, sel, bounds, i, p);
            }
        }
        lr
    }

    // The links.csv rows of scenario i in period p
    fn period_link_rows(&self, lr: &mut Rows, sel: &Sel, bounds: Option<&JsonValue>, i: usize, p: &str) {
        let s_set = self.st();
        let sc = self.scenario(i);
        let a_up = aup_of(self, i);
        let Some(res) = res_of(self, i, p) else {
            return;
        };
        let wo = if (Some(i) == sel.with || Some(i) == sel.mitigation) && sel.without.is_some() {
            sel.without.and_then(|w| res_of(self, w, p))
        } else {
            None
        };
        let ext = wo.and_then(|wr| {
            extent_test(sc, Some(wr), Some(res), ExtentOpts { sc_without: sel.without.map(|w| self.scenario(w)), ..ExtentOpts::default() })
        });
        for l in sc.links.values() {
            for d in ["ab", "ba"] {
                let lanes = if d == "ab" { &l.lanes_ab } else { &l.lanes_ba };
                if !lanes.truthy() {
                    continue;
                }
                let Some(arc) = res.arcs.get(&format!("{}:{d}", l.id)) else {
                    continue;
                };
                let sp = speed_of(l);
                let ext_row = ext.as_ref().and_then(|e| e.rows.iter().find(|q| q.key == format!("{}:{d}", l.id)));
                let fcv = |key: &str| -> Cell {
                    match l.fc.get().and_then(|f| f.get(key)) {
                        Some(v) if crate::jsnum::value_truthy(v) => Cell::from_json(Some(&JsonValue::from_value(v))),
                        _ => t(""),
                    }
                };
                let cap_per = l.cap_per_lane.num();
                let class_cap = road_class(l.cls.as_str()).cap;
                let cap = (if cap_per == 0.0 || cap_per.is_nan() { class_cap } else { cap_per }) * lanes.num();
                lr.push(vec![
                    Cell::S(sc.id.clone()),
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
                    n(math_round(arc.len)),
                    n(math_round(arc.v)),
                    n(math_round(arc.pcu)),
                    Cell::from_num(&s_set.hv_pct),
                    n(math_round(arc.cap)),
                    n(cap),
                    t(if l.cap_per_lane.truthy() { "user" } else { "class default" }),
                    n(arc.vc),
                    n(r1(arc.spd)),
                    match sp.v {
                        Some(v) if v > 0.0 => n(r1(arc.spd / v * 100.0)),
                        _ => t(""),
                    },
                    t(""),
                    n(r1(arc.nd)),
                    ext_row.map_or(t(""), |x| n(math_round(x.without))),
                    ext_row.map_or(t(""), |x| n(math_round(x.d))),
                    match ext_row {
                        Some(x) if x.p.is_some() => n(r1(x.p.unwrap_or(0.0) * 100.0)),
                        _ => t(""),
                    },
                    match ext_row {
                        Some(x) if x.threshold.is_some() => n(x.threshold.unwrap_or(0.0) * 100.0),
                        _ => t(""),
                    },
                    in_bounds(bounds, l),
                ]);
            }
        }
    }

    // junction_summary.csv and junction_movements.csv
    fn junction_rows(&self) -> (Rows, Rows) {
        let (mut js_rows, mut jm): (Rows, Rows) = (Vec::new(), Vec::new());
        for i in 0..self.project.scenarios.len() {
            let sc = self.scenario(i);
            for p in self.rep_periods(i) {
                let Some(res) = res_of(self, i, p) else {
                    continue;
                };
                for (id, nr) in res.nodes.iter() {
                    if nr.r#type == "none" {
                        continue;
                    }
                    let sum = j_sum(nr);
                    let nm = site_name(sc, Some(res), id);
                    let site = nr.site.clone().filter(|x| !x.is_empty()).unwrap_or_else(|| id.to_string());
                    js_rows.push(vec![
                        Cell::S(sc.id.clone()),
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
                        n(r1(sum.delay)),
                        Cell::S(nr.los.clone()),
                        Cell::S(sum.basis.clone()),
                        n(math_round(sum.q95)),
                        Cell::S(sum.leg.clone()),
                    ]);
                    for ap in &nr.appr {
                        let link = sc.links.get(&ap.link);
                        jm.push(vec![
                            Cell::S(sc.id.clone()),
                            t(p),
                            Cell::S(site.clone()),
                            Cell::S(nm.clone()),
                            Cell::S(ap.approach.clone()),
                            Cell::S(ap.name.clone()),
                            Cell::S(ap.link.clone()),
                            n(math_round(ap.v)),
                            n(ap.cap),
                            n(r3(ap.dos)),
                            n(r1(ap.delay)),
                            Cell::S(ap.los.clone()),
                            n(ap.q95veh),
                            n(ap.q95),
                            ap.psc.map_or(t(""), n),
                            link.map_or(t(""), |l| n(math_round(sc.link_len(l)))),
                        ]);
                    }
                }
            }
        }
        (js_rows, jm)
    }

    // turning_movements.csv
    fn turn_rows(&self) -> Rows {
        let s_set = self.st();
        let mut tm: Rows = Vec::new();
        for i in 0..self.project.scenarios.len() {
            let sc = self.scenario(i);
            for p in self.rep_periods(i) {
                let Some(res) = res_of(self, i, p) else {
                    continue;
                };
                for (id, list) in res.turns.iter() {
                    if !sc.nodes.contains_key(id) {
                        continue;
                    }
                    let mv = crate::turns::movements(sc, Some(res), s_set, id);
                    let mut code: Vec<(String, &crate::turns::MovementRow)> = Vec::new();
                    for mrow in &mv {
                        let key = format!("{}>{}", mrow.approach, mrow.to);
                        match code.iter_mut().find(|(name, _)| *name == key) {
                            Some(entry) => entry.1 = mrow,
                            None => code.push((key, mrow)),
                        }
                    }
                    for tr in list {
                        let m = code.iter().find(|(name, _)| *name == format!("{}>{}", tr.from, tr.to)).map(|(_, mrow)| *mrow);
                        tm.push(vec![
                            Cell::S(sc.id.clone()),
                            t(p),
                            Cell::S(id.to_string()),
                            Cell::S(kit_junction_name(sc, id)),
                            Cell::S(tr.from.clone()),
                            Cell::S(tr.to.clone()),
                            Cell::S(tr.mv.clone()),
                            Cell::S(m.map_or(String::new(), |mrow| mrow.leg_name.to_string())),
                            Cell::S(m.map_or(String::new(), |mrow| mrow.turn.to_string())),
                            Cell::S(m.map_or(String::new(), |mrow| mrow.id.clone())),
                            n(r1(tr.v)),
                            n(r1(tr.pcu)),
                            tr.dev.map_or(t(""), |dev| n(r1(dev))),
                        ]);
                    }
                }
            }
        }
        tm
    }

    // One file per scenario and period as well, named with the period and year
    fn by_scenario_files(&self, files: &mut Vec<PackFile>, tables: [(&str, &Rows); 4]) {
        let mut used: Vec<String> = Vec::new();
        let project_slug = slug(self.name());
        for i in 0..self.project.scenarios.len() {
            let sc = self.scenario(i);
            for p in self.rep_periods(i) {
                let Some(res) = res_of(self, i, p) else {
                    continue;
                };
                let y: String = if res.year > 0.0 {
                    number_to_js_string(res.year)
                } else {
                    self.year_of(i).map_or("year".to_string(), number_to_js_string)
                };
                let sname = sc.name.as_str().unwrap_or("");
                let b0 = [
                    if project_slug.is_empty() { "project".to_string() } else { project_slug.clone() },
                    {
                        let by_name = slug(sname);
                        if by_name.is_empty() {
                            let by_id = slug(&sc.id);
                            if by_id.is_empty() { "scenario".to_string() } else { by_id }
                        } else {
                            by_name
                        }
                    },
                    p.to_lowercase(),
                    y.clone(),
                ]
                .join("-");
                let mut base = b0.clone();
                let mut k: u64 = 1;
                // Bound: k counts the files already named, far below u64::MAX
                while used.contains(&base) {
                    k = k.saturating_add(1);
                    base = format!("{b0}-{k}");
                }
                used.push(base.clone());
                for (table, rows) in tables {
                    let name = format!("by-scenario/{base}-{}", table.replace('_', "-"));
                    // Every row starts with the scenario id and the period
                    let sub: Vec<Vec<Cell>> = rows
                        .iter()
                        .filter(|r| r.first() == Some(&Cell::S(sc.id.clone())) && r.get(1) == Some(&t(p)))
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
    }

    // trip_generation.csv
    fn trip_rows(&self) -> Rows {
        let s_set = self.st();
        let mut tg: Rows = Vec::new();
        for i in 0..self.project.scenarios.len() {
            let sc = self.scenario(i);
            for zone in sc.zones.values() {
                for p in ["AM", "PM"] {
                    if zone.kind.is("gateway") {
                        let gf = crate::standards::growth(s_set, Some(sc));
                        let gw = |suffix: &str| -> f64 {
                            let val = zone.gw.get().and_then(|map| map.get(&format!("{p}{suffix}"))).map_or(0.0, crate::jsnum::Maybe::num);
                            (if val == 0.0 || val.is_nan() { 0.0 } else { val }) * gf
                        };
                        tg.push(vec![
                            Cell::S(sc.id.clone()),
                            t(p),
                            Cell::S(zone.id.clone()),
                            Cell::S(zone.name.as_str().unwrap_or("").to_string()),
                            t("external"),
                            t("gateway"),
                            t(""),
                            t(""),
                            t(""),
                            t(""),
                            t(""),
                            t(""),
                            t(""),
                            n(r1(gw("in"))),
                            n(r1(gw("out"))),
                        ]);
                        continue;
                    }
                    let red = clamp(zone.reduction.or(0.0), 0.0, 100.0);
                    let st = crate::demand::stage_of(Some(zone), Some(sc));
                    // !(zone year > scenario year), NaN counting as open as in the JavaScript
                    let zy = zone.year.num();
                    let sy = crate::standards::year_of(Some(sc), s_set);
                    let open = zy <= sy || zy.is_nan() || sy.is_nan();
                    let Some(lu) = zone.lu.get() else {
                        continue;
                    };
                    for (key, qty) in lu.iter() {
                        let rr = s_set.rates.get().and_then(|map| map.get(key)).and_then(|pr| pr.get()).and_then(|pr| pr.get(p)).and_then(|rate| rate.get());
                        let (Some(rr), Some(lu_def)) = (rr, land_use(key)) else {
                            continue;
                        };
                        let qn = qty.num();
                        if qn <= 0.0 || qn.is_nan() {
                            continue;
                        }
                        let gross = qn / lu_def.per * rr.rate.num();
                        let net = if open { gross * (1.0 - red / 100.0) * st } else { 0.0 };
                        let inpct = rr.in_pct.num();
                        tg.push(vec![
                            Cell::S(sc.id.clone()),
                            t(p),
                            Cell::S(zone.id.clone()),
                            Cell::S(zone.name.as_str().unwrap_or("").to_string()),
                            Cell::S(crate::graph::zone_class(Some(zone)).to_string()),
                            Cell::S(key.to_string()),
                            n(qn),
                            t(lu_def.unit),
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
        tg
    }

    // distribution.csv
    fn distribution_rows(&self) -> Rows {
        let mut dist: Rows = Vec::new();
        for i in 0..self.project.scenarios.len() {
            let rl: Vec<&Res> = self.rep_periods(i).iter().filter_map(|p| res_of(self, i, p)).collect();
            if rl.is_empty() {
                continue;
            }
            let dd = dev_distribution(Some(self.scenario(i)), &rl);
            let sc = self.scenario(i);
            for r in &dd.rows {
                let sh = |k: &str| -> Cell { n(r1(r.shares.get(k).copied().unwrap_or(0.0))) };
                dist.push(vec![
                    Cell::S(sc.id.clone()),
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
        dist
    }

    // counts_calibration.csv and calibration_summary.csv
    fn calibration_rows(&self, sel: &Sel) -> (Rows, Rows) {
        let s_set = self.st();
        let (mut cc, mut cal_sum): (Rows, Rows) = (Vec::new(), Vec::new());
        let cal_i = sel.existing.or(sel.base).unwrap_or(0);
        let cal0 = self.scenario(cal_i);
        let data = crate::datahub::Data::read(self.project);
        let raw_counts = js::items(js::get(self.data(), "counts"));
        for p in self.rep_periods(cal_i) {
            let cal = crate::datahub::calibration(cal0, res_of(self, cal_i, p), p, s_set, &data);
            if let Some(cal) = cal {
                for row in &cal.rows {
                    let rawc = raw_counts.iter().find(|c| js::get(Some(c), "id").and_then(JsonValue::as_str) == Some(row.c.id.as_str()));
                    let rg = |key: &str| Cell::or_blank(js::get(rawc, key));
                    cc.push(vec![
                        Cell::S(cal0.id.clone()),
                        t(p),
                        Cell::S(row.c.id.clone()),
                        Cell::from_json(js::get(rawc, "src")),
                        rg("ref"),
                        rg("name"),
                        rg("road"),
                        rg("date"),
                        rg("dur"),
                        rg("adt"),
                        n(r1(row.obs)),
                        n(r1(row.model)),
                        if row.geh.is_nan() { t("") } else { n(r3(row.geh)) },
                        Cell::S(row.how.clone()),
                        Cell::S(row.mt.link.clone()),
                        Cell::S(row.mt.dir.to_string()),
                    ]);
                }
            }
            let rep = crate::calibrate::report(self.project, cal0, p, res_of(self, cal_i, p));
            for check in &rep.checks {
                cal_sum.push(check_row(&cal0.id, p, &check.to_value()));
            }
        }
        (cc, cal_sum)
    }

    // route_times.csv
    fn route_rows(&self) -> Rows {
        let mut rt: Rows = Vec::new();
        for i in 0..self.project.scenarios.len() {
            let sc = self.scenario(i);
            for p in self.rep_periods(i) {
                let Some(res) = res_of(self, i, p) else {
                    continue;
                };
                for (id, x) in res.routes.iter() {
                    rt.push(vec![
                        Cell::S(sc.id.clone()),
                        t(p),
                        Cell::S(id.to_string()),
                        Cell::S(sc.routes.get(id).and_then(|r| r.name.as_str().map(str::to_string)).unwrap_or_default()),
                        n(math_round(x.len)),
                        n(r1(x.t)),
                        n(r1(x.spd.unwrap_or(f64::NAN))),
                    ]);
                }
            }
        }
        rt
    }

    // crashes.csv: each crash with the nearest crash site within 50 m
    fn crash_rows(&self, sites: &[CrashSite]) -> Rows {
        let crashes = js::items(js::get(self.data(), "crashes"));
        let mut near: Vec<Option<(String, f64)>> = vec![None; crashes.len()];
        for site in sites {
            for (k, d) in &site.list {
                if let Some(slot) = near.get_mut(*k)
                    && slot.as_ref().is_none_or(|o| *d < o.1)
                {
                    *slot = Some((site.id.clone(), *d));
                }
            }
        }
        crashes
            .iter()
            .zip(&near)
            .map(|(crash, nearest)| {
                let nn = nearest.as_ref();
                let col = |i: usize| js::idx(Some(crash), i);
                let loc = js::text_or_empty(col(4));
                let mut parts = loc.split(" / ");
                let (l1, l2) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                let flags = js::text_or_empty(col(5));
                let count = |i: usize| cnum(crash, i).map_or(t(""), n);
                vec![
                    Cell::S(ctext(crash, cx::ID)),
                    Cell::from_json(col(3)),
                    Cell::from_json(col(2)),
                    count(cx::FATAL),
                    count(cx::SERIOUS),
                    count(cx::MINOR),
                    match col(6) {
                        None | Some(JsonValue::Null) => t(""),
                        v => Cell::from_json(v),
                    },
                    Cell::S(ctext(crash, cx::CONTROL)),
                    Cell::S(ctext(crash, cx::URBAN)),
                    Cell::B(flags.contains('p')),
                    Cell::B(flags.contains('c')),
                    Cell::B(flags.contains('m')),
                    Cell::S(ctext(crash, cx::LIGHT)),
                    Cell::S(ctext(crash, cx::WEATHER)),
                    Cell::S(l1.to_string()),
                    Cell::S(l2.to_string()),
                    nn.map_or(t(""), |o| Cell::S(o.0.clone())),
                    nn.map_or(t(""), |o| n(math_round(o.1))),
                    Cell::from_json(col(0)),
                    Cell::from_json(col(1)),
                ]
            })
            .collect()
    }

    // od_{period}.csv for the with scenario, or a note in the README where the matrix is not kept
    fn od_files(&self, sc_i: usize, files: &mut Vec<PackFile>, missing: &mut Vec<String>) {
        let sc = self.scenario(sc_i);
        for p in self.rep_periods(sc_i) {
            let Some(res) = res_of(self, sc_i, p) else {
                continue;
            };
            let Some(tm_) = res.t.as_ref().filter(|_| !res.zone_ids.is_empty()) else {
                missing.push(format!("od_{p}.csv: the OD matrix is kept only for 150 zones or fewer"));
                continue;
            };
            let z = res.zone_ids.len();
            let nm = |id: &str| -> String {
                match sc.zones.get(id).and_then(|zone| zone.name.as_str()) {
                    Some(x) if !x.is_empty() => x.to_string(),
                    _ => id.to_string(),
                }
            };
            let mut rows: Rows = vec![
                vec![Cell::S(format!(
                    "# Spare Construct ITA data pack, OD matrix {} {} {} (veh/h). {}",
                    sc.name.as_str().unwrap_or(""),
                    p,
                    number_to_js_string(res.year),
                    super::consts::disclaimer_text("D1", None, None)
                ))],
                std::iter::once(t("from \\ to")).chain(res.zone_ids.iter().map(|id| Cell::S(nm(id)))).collect(),
            ];
            for (i, zid) in res.zone_ids.iter().enumerate() {
                // The matrix holds z rows of z values, so row i starts at i * z; a missing value reads as undefined (NaN)
                let from_zone = i.checked_mul(z).and_then(|start| tm_.get(start..)).unwrap_or(&[]);
                rows.push(std::iter::once(Cell::S(nm(zid))).chain((0..z).map(|j| n(r1(from_zone.get(j).copied().unwrap_or(f64::NAN))))).collect());
            }
            files.push(PackFile { name: format!("od_{p}.csv"), data: format!("\u{feff}{}\r\n", csv_text(&rows)), of: None, handover: false });
        }
    }

    // The junctions feature's volumes file and SIDRA 11 loader script for scenario o
    fn sidra_files(&self, sc_i: usize, files: &mut Vec<PackFile>, missing: &mut Vec<String>) {
        let tools = self.tools();
        let today = iso_day_of_ms(self.now_ms).unwrap_or_default();
        let opts = SidraOpts { scope: "auto".to_string(), force: false, date_iso: today, date_long: long_date(self.now_ms) };
        let sc = tools.sc(sc_i);
        let v = tools.volumes_csv(sc, None, None, &opts);
        let l = v.as_ref().ok().map(|_| tools.loader_py(sc, None, None, &opts));
        match (v, l) {
            (Ok(v), Some(Ok(l))) => {
                if crate::jsnum::js_trim(&v).is_empty() {
                    missing.push("corridor-sidra-volumes.csv: the junctions feature produced no rows".to_string());
                } else {
                    files.push(PackFile {
                        name: "corridor-sidra-volumes.csv".to_string(),
                        data: format!("\u{feff}{}", v.strip_prefix('\u{feff}').unwrap_or(&v)),
                        of: None,
                        handover: true,
                    });
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
    fn export_files(&self, files: &mut Vec<PackFile>, missing: &mut Vec<String>) {
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
                    // The text ends with the line break being cut, so the lengths are at least 2 and 1
                    loop {
                        if text.ends_with("\r\n") {
                            text.truncate(text.len().saturating_sub(2));
                        } else if text.ends_with('\n') {
                            text.truncate(text.len().saturating_sub(1));
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
    }

    // network.geojson: scenario gs in the report's period (or its first period), with the pack's name and disclaimer
    fn geojson_file(&self, gs: usize, period: &str) -> PackFile {
        let gscn = self.scenario(gs);
        let res = res_of(self, gs, period).or_else(|| self.rep_periods(gs).first().and_then(|p| res_of(self, gs, p)));
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
        PackFile { name: "network.geojson".to_string(), data, of: None, handover: false }
    }

    // settings.json: the project's settings without the LINZ key
    fn settings_file(&self) -> PackFile {
        let mut settings = js::get(Some(self.raw), "settings").cloned().unwrap_or(JsonValue::Null);
        if let JsonValue::Object(m) = &mut settings {
            m.remove("linzKey");
        }
        let mut o = JsonValue::object();
        o.set("settings", settings);
        o.set("modelVersion", JsonValue::String(super::shared::model_version(self)));
        o.set("disclaimer", JsonValue::String(super::consts::disclaimer_text("D1", None, None)));
        PackFile { name: "settings.json".to_string(), data: o.stringify_pretty(1), of: None, handover: false }
    }
}

// A calibration_summary.csv row from one calibration check (as JSON)
fn check_row(cal_id: &str, p: &str, check: &serde_json::Value) -> Vec<Cell> {
    let g = |f: &str| check.get(f).cloned();
    let or_blank = |f: &str| match g(f) {
        Some(serde_json::Value::String(s)) => Cell::S(s),
        _ => t(""),
    };
    vec![
        Cell::S(cal_id.to_string()),
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
        Cell::B(check.get("verified").is_some_and(crate::jsnum::value_truthy)),
    ]
}

// crash_sites.csv
fn crash_site_rows(sites: &[CrashSite]) -> Rows {
    sites
        .iter()
        .map(|site| {
            vec![
                Cell::S(site.id.clone()),
                Cell::S(site.name.clone()),
                site.control.as_ref().map_or(Cell::Null, |c| Cell::S(c.clone())),
                n(site.speed),
                n(site.f),
                n(site.s),
                n(site.m),
                n(site.n),
                n(site.injury),
                n(r3(site.dsi_eq)),
                t(site.collective),
                Cell::S(
                    site.legs
                        .iter()
                        .map(|leg| match leg.aadt {
                            Some(a) if a != 0.0 && !a.is_nan() => number_to_js_string(math_round(a)),
                            _ => String::new(),
                        })
                        .collect::<Vec<_>>()
                        .join(" / "),
                ),
                match site.pof {
                    Some(p) if p != 0.0 && !p.is_nan() => n(r1(p)),
                    _ => t(""),
                },
                site.pr.map_or(t(""), |p| n(r1(p))),
                Cell::S(site.pband.label.clone()),
                Cell::B(site.pband.reliable),
            ]
        })
        .collect()
}

/// `new Date().toLocaleDateString('en-NZ', { day: 'numeric', month: 'long', year: 'numeric' })`: "21 October 2026".
pub(crate) fn long_date(ms: f64) -> String {
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let iso = iso_day_of_ms(ms).unwrap_or_default();
    let (Some(year), Some(mm), Some(dd)) = (iso.get(0..4), iso.get(5..7), iso.get(8..10)) else {
        return iso;
    };
    let (month_no, day) = (mm.parse::<usize>().unwrap_or(1), dd.parse::<usize>().unwrap_or(1));
    // An ISO day's month is 01 to 12, so the name is always found
    let month = month_no.checked_sub(1).and_then(|i| MONTHS.get(i)).copied().unwrap_or("");
    format!("{day} {month} {year}")
}
