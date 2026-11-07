//! The junction CSVs: junction_movements.csv, turning_movements.csv and one junction's turning count.

use super::j1::{J1Row, J1Sub};
use super::{Entry, STORAGE_RED, Sc, Tools, entries, js_str, moves_of, nc_text, per_name, xid};
use crate::jsnum::{math_min, math_round, number_to_js_string};
use crate::results::export_ready;
use crate::util::{csv_cell, r1, r3, r7};
use serde_json::Value;

/// One CSV cell as the prototype's `U.csv` writes it: a number prints as `String(x)`, text with the formula rule.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Cell {
    /// A number.
    Num(f64),
    /// Text (a leading = + - @ tab or CR that is not a number gets an apostrophe).
    Text(String),
    /// A loose JSON value (`String(v)`, the formula rule only for text).
    Val(Value),
}

/// A blank cell.
pub(crate) fn blank() -> Cell {
    Cell::Text(String::new())
}

/// A number, or a blank for none.
pub(crate) fn opt(x: Option<f64>) -> Cell {
    x.map_or_else(blank, Cell::Num)
}

/// Text.
pub(crate) fn txt(s: &str) -> Cell {
    Cell::Text(s.to_string())
}

fn cell_text(c: &Cell) -> String {
    match c {
        Cell::Num(x) => number_to_js_string(*x),
        Cell::Text(s) => csv_cell(&Value::String(s.clone())),
        Cell::Val(v) => csv_cell(v),
    }
}

/// `U.csv(rows)`: cells joined with commas, rows with CRLF.
pub(crate) fn write_csv(rows: &[Vec<Cell>]) -> String {
    rows.iter()
        .map(|r| r.iter().map(cell_text).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn heads(h: &[&str]) -> Vec<Cell> {
    h.iter().map(|x| txt(x)).collect()
}

/// The columns of junction_movements.csv.
pub const JM_HEAD: [&str; 25] = [
    "scenario_id",
    "year",
    "period",
    "site_id",
    "junction",
    "control",
    "cycle_s",
    "pff",
    "approach_leg",
    "approach_road",
    "movement",
    "demand_veh_h",
    "hv_pct",
    "capacity_veh_h",
    "dos",
    "practical_dos",
    "psc_pct",
    "delay_s",
    "los",
    "los_method",
    "q95_veh",
    "q95_m",
    "queue_space_m",
    "storage_m",
    "exceeds_storage",
];

/// The columns of turning_movements.csv.
pub const TM_HEAD: [&str; 19] = [
    "scenario_id",
    "period",
    "year",
    "site_id",
    "site_name",
    "at_site_no",
    "lat",
    "lng",
    "control",
    "from_leg",
    "from_road",
    "to_leg",
    "to_road",
    "turn",
    "lv_veh_h",
    "hv_veh_h",
    "total_veh_h",
    "hv_pct",
    "pcu_h",
];

/// What a summary row gives the junction_movements.csv: the fields `row` reads.
struct RowView<'a> {
    v: Option<f64>,
    hv_pct: Option<f64>,
    cap: Option<f64>,
    dos: Option<f64>,
    xp: Option<f64>,
    psc: Option<f64>,
    delay: Option<f64>,
    los: Option<&'a str>,
    q95veh: Option<f64>,
    q95: Option<f64>,
    storage: Option<f64>,
    rq: Option<f64>,
}

fn view_row(r: &J1Row) -> RowView<'_> {
    RowView {
        v: Some(r.v),
        hv_pct: Some(r.hv_pct),
        cap: r.cap,
        dos: r.dos,
        xp: r.xp,
        psc: r.psc,
        delay: r.delay,
        los: r.los.as_deref(),
        q95veh: r.q95veh,
        q95: r.q95,
        storage: r.storage,
        rq: r.rq,
    }
}

fn view_sub(s: &J1Sub) -> RowView<'_> {
    RowView {
        v: Some(s.v),
        hv_pct: Some(s.hv_pct),
        cap: Some(s.cap),
        dos: Some(s.dos),
        xp: s.xp,
        psc: s.psc,
        delay: Some(s.delay),
        los: Some(&s.los),
        q95veh: Some(s.q95veh),
        q95: Some(s.q95),
        storage: s.storage,
        rq: s.rq,
    }
}

impl<'a> Tools<'a> {
    /// The scenarios an export covers: the ones given, else every scenario (`scList(P, which)`).
    fn sc_list(&self, which: Option<&[Sc<'a>]>) -> Vec<Sc<'a>> {
        match which {
            Some(w) => w.to_vec(),
            None => self.scs(),
        }
    }

    /// `junctionMovementsCSV(P, which, periods)`: the movement summary of every junction, scenario and period (docs/spec/sidra-tables).
    pub fn junction_movements_csv(
        &self,
        which: Option<&[Sc<'a>]>,
        periods: Option<&[String]>,
    ) -> String {
        let mut out: Vec<Vec<Cell>> = vec![heads(&JM_HEAD)];
        for s in self.sc_list(which) {
            let ps: Vec<String> = match periods {
                Some(p) => p.to_vec(),
                None => s.periods(),
            };
            for p in ps {
                let Some(res) = s.res_of(&p) else {
                    continue;
                };
                let j = &res.junction;
                for e in entries(s.sc, res, false) {
                    let Some(d) = self.j1(s.sc, res, &e.id, Some(&e)) else {
                        continue;
                    };
                    let base: Vec<Cell> = vec![
                        txt(&s.sc.id),
                        Cell::Num(res.year),
                        txt(&p),
                        txt(&xid(&e)),
                        txt(&e.name),
                        txt(&e.kind),
                        match d.nr.c {
                            Some(c) if c != 0.0 && !c.is_nan() => Cell::Num(c),
                            _ => blank(),
                        },
                        Cell::Num(j.pff),
                    ];
                    let method = if j.method.is_empty() {
                        d.method.clone()
                    } else {
                        j.method.clone()
                    };
                    let row =
                        |out: &mut Vec<Vec<Cell>>, leg: &str, road: &str, mv: &str, r: &RowView| {
                            let mut c = base.clone();
                            c.push(txt(leg));
                            c.push(txt(road));
                            c.push(txt(mv));
                            c.push(opt(r.v.map(math_round)));
                            c.push(opt(r.hv_pct.map(r1)));
                            c.push(opt(r.cap.map(math_round)));
                            c.push(opt(r.dos.map(r3)));
                            c.push(opt(r.xp));
                            c.push(opt(r.psc));
                            c.push(opt(r.delay.map(r1)));
                            c.push(match r.los {
                                Some(l) if !l.is_empty() && l != "\u{2013}" => txt(l),
                                _ => blank(),
                            });
                            c.push(txt(&method));
                            c.push(opt(r.q95veh));
                            c.push(opt(r.q95));
                            c.push(Cell::Num(j.queue_space));
                            c.push(opt(r.storage.map(math_round)));
                            c.push(match r.rq {
                                Some(q) => txt(if q > STORAGE_RED { "yes" } else { "no" }),
                                None => blank(),
                            });
                            out.push(c);
                        };
                    for g in &d.groups {
                        for x in &g.appr {
                            for r in &x.rows {
                                let mv = r.turns.join("+");
                                let mv = if mv.is_empty() { r.mv.clone() } else { mv };
                                row(&mut out, &g.leg, &g.road, &mv, &view_row(r));
                            }
                            row(&mut out, &g.leg, &g.road, "APPROACH", &view_sub(&x.sub));
                        }
                    }
                    if let Some(a) = &d.all {
                        let mut c = base.clone();
                        c.push(blank());
                        c.push(blank());
                        c.push(txt("ALL"));
                        c.push(Cell::Num(math_round(a.v)));
                        c.push(Cell::Num(r1(a.hv_pct)));
                        c.push(blank());
                        c.push(Cell::Num(r3(a.dos)));
                        c.push(opt(a.xp));
                        c.push(blank());
                        c.push(Cell::Num(r1(a.delay)));
                        c.push(txt(&a.los));
                        c.push(txt(&method));
                        c.push(Cell::Num(a.q95veh));
                        c.push(Cell::Num(a.q95));
                        c.push(Cell::Num(j.queue_space));
                        c.push(blank());
                        c.push(blank());
                        out.push(c);
                    }
                }
            }
        }
        write_csv(&out)
    }

    /// `turningMovementsCSV(P, which, periods, opts)`: every junction with turning traffic; site movements for sites. Turn-volume
    /// exports need the SIDRA gap: an unconverged run gives its reason as the error unless `force`, and a forced file gains a
    /// last column, export_note, that says NOT CONVERGED on every row from such a run.
    pub fn turning_movements_csv(
        &self,
        which: Option<&[Sc<'a>]>,
        periods: Option<&[String]>,
        force: bool,
    ) -> Result<String, String> {
        let mut runs: Vec<(Sc<'a>, String)> = Vec::new();
        for s in self.sc_list(which) {
            let ps: Vec<String> = match periods {
                Some(p) => p.to_vec(),
                None => s.periods(),
            };
            for p in ps {
                if s.res_of(&p).is_some() {
                    runs.push((s, p));
                }
            }
        }
        let bad: Vec<&(Sc<'a>, String)> = runs
            .iter()
            .filter(|(s, p)| !export_ready(s.res_of(p)).ok)
            .collect();
        if !bad.is_empty() && !force {
            let (s, p) = bad[0];
            return Err(format!(
                "{}, {}: {}",
                s.name_or_id(),
                per_name(p),
                export_ready(s.res_of(p)).why
            ));
        }
        let mut head = heads(&TM_HEAD);
        if !bad.is_empty() {
            head.push(txt("export_note"));
        }
        let mut out: Vec<Vec<Cell>> = vec![head];
        for (s, p) in &runs {
            let res = s.res_of(p).expect("result");
            let nc: Vec<Cell> = if bad.is_empty() {
                Vec::new()
            } else {
                vec![txt(&nc_text(res))]
            };
            for e in entries(s.sc, res, true) {
                for m in moves_of(s.sc, self.st(), res, &e) {
                    let (lv, hv, t) = split(m.v, m.hv);
                    let mut c = vec![
                        txt(&s.sc.id),
                        txt(p),
                        Cell::Num(res.year),
                        txt(&xid(&e)),
                        txt(&e.name),
                        at_site_cell(&e),
                        e.ll.map_or_else(blank, |l| Cell::Num(r7(l[0]))),
                        e.ll.map_or_else(blank, |l| Cell::Num(r7(l[1]))),
                        txt(&e.kind),
                        txt(m.leg),
                        txt(&m.road),
                        txt(m.to_leg),
                        txt(&m.to_road),
                        txt(m.turn),
                        Cell::Num(lv),
                        Cell::Num(hv),
                        Cell::Num(t),
                        Cell::Num(if t > 0.0 { r1(hv / t * 100.0) } else { 0.0 }),
                        Cell::Num(r1(m.pcu)),
                    ];
                    c.extend(nc.iter().cloned());
                    out.push(c);
                }
            }
        }
        Ok(write_csv(&out))
    }
}

/// `split(r)`: light vehicles, heavy vehicles and the total of a movement, whole vehicles that add up.
pub(crate) fn split(v: f64, hv_v: f64) -> (f64, f64, f64) {
    let t = math_round(v);
    let hv = math_min(t, math_round(hv_v));
    (t - hv, hv, t)
}

/// `e.atSiteNo ?? ''`.
pub(crate) fn at_site_cell(e: &Entry) -> Cell {
    if e.at_site_no.is_null() {
        blank()
    } else {
        Cell::Val(e.at_site_no.clone())
    }
}

/// One junction's turning count with its convergence marker.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnCountFile {
    /// The file's text.
    pub text: String,
    /// NOT CONVERGED text, or empty.
    pub nc: String,
}

impl<'a> Tools<'a> {
    /// `turnCountFile(sc, res, nodeId)`: one junction's turning count (`Turns.turnCountCSV`); a run short of the turn-volume
    /// export gap starts with a 'Convergence, NOT CONVERGED' row, so the file says so wherever it goes.
    pub fn turn_count_file(
        &self,
        sc: &crate::project::Scenario,
        res: Option<&crate::results::Res>,
        node_id: &str,
    ) -> TurnCountFile {
        let nc = res.map_or_else(String::new, nc_text);
        let text = crate::turns::turn_count_csv(Some(sc), res, self.st(), node_id);
        TurnCountFile {
            text: if nc.is_empty() {
                text
            } else {
                format!(
                    "{}\r\n{}",
                    write_csv(&[vec![txt("Convergence"), txt(&nc)]]),
                    text
                )
            },
            nc,
        }
    }
}

/// `String(v)` for a CSV cell of a loose value (kept for the exports that print site numbers).
#[allow(dead_code)]
pub(crate) fn loose(v: &Value) -> String {
    js_str(v)
}
