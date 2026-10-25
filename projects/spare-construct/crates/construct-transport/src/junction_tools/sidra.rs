//! The SIDRA hand-off (docs/spec/sidra-export): turning volumes by junction, the volumes CSV, the SIDRA 11 loader script and the
//! printable volume grids.

use super::csv::{Cell, at_site_cell, blank, split, txt, write_csv};
use super::export_text::{GRIDS, LOADER};
use super::{
    D1, D12, Entry, Sc, SIDRA_GROWTH_PCT, SIDRA_PEAK_FLOW_PERIOD, SIDRA_PHF_PERIOD, SIDRA_SETUP,
    SIDRA_TYPE_NAME, SIDRA_UNIT_TIME, TURN_ORDER, Tools, entries, js_str, legs_of, moves_of, nc_text,
    per_name, sidra_site_type, xid, LABEL,
};
use crate::edit::json::JsonValue;
use crate::jsnum::{math_max, math_min, number_to_js_string, value_to_number};
use crate::project::Scenario;
use crate::results::Res;
use crate::turns::{LEG_CODES, LEG_NAMES, leg_label, sidra_export, SidraOpts as TurnsSidraOpts};
use crate::util::{esc, r7};
use serde_json::Value;

/// The columns of the SIDRA volumes CSV.
pub const VOL_HEAD: [&str; 16] = [
    "site_id",
    "site_name",
    "at_site_no",
    "lat",
    "lng",
    "site_type",
    "period",
    "origin_leg",
    "dest_leg",
    "turn",
    "lv_veh_h",
    "hv_veh_h",
    "pff_pct",
    "unit_time_min",
    "peak_flow_period_min",
    "growth_pct",
];

/// The columns of the turning-count layout the volumes CSV is converted to.
pub const COUNT_HEAD: [&str; 11] = [
    "site_id",
    "site_name",
    "lat",
    "lng",
    "date",
    "time_start",
    "interval_min",
    "from_leg",
    "movement",
    "class",
    "count",
];

fn window_start(period: &str) -> &'static str {
    match period {
        "AM" => "07:00",
        "IP" => "11:00",
        "PM" => "16:00",
        "SAT" => "11:00",
        _ => "",
    }
}

/// What the SIDRA exports are asked for: which junctions, whether to go past the gate, and the date to print.
#[derive(Clone, Debug)]
pub struct SidraOpts {
    /// Junctions to export: "auto" (sites from the extent test, else every controlled junction), "controlled" or "all".
    pub scope: String,
    /// Export a run that is not converged enough, marked so.
    pub force: bool,
    /// Today as YYYY-MM-DD (the loader script's date).
    pub date_iso: String,
    /// Today as "21 October 2026" (the volume grids' date).
    pub date_long: String,
}

impl Default for SidraOpts {
    fn default() -> Self {
        SidraOpts {
            scope: "auto".to_string(),
            force: false,
            date_iso: String::new(),
            date_long: String::new(),
        }
    }
}

/// One turning volume for SIDRA: a movement of a junction in a period, light and heavy vehicles.
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeRow {
    /// Stable junction id.
    pub site_id: String,
    /// Junction name.
    pub site_name: String,
    /// AT site number(s), or null (printed as nothing).
    pub at_site_no: Value,
    /// Latitude (7 decimals).
    pub lat: Option<f64>,
    /// Longitude (7 decimals).
    pub lng: Option<f64>,
    /// SIDRA site type.
    pub site_type: f64,
    /// Period.
    pub period: String,
    /// Origin leg code (the leg's position in S, SE, E, NE, N, NW, W, SW; -1 when none).
    pub origin_leg: f64,
    /// Destination leg code.
    pub dest_leg: f64,
    /// Turn designation.
    pub turn: String,
    /// Light vehicles (veh/h).
    pub lv_veh_h: f64,
    /// Heavy vehicles (veh/h).
    pub hv_veh_h: f64,
    /// Peak flow factor (%).
    pub pff_pct: f64,
    /// Unit time for volumes (min).
    pub unit_time_min: f64,
    /// Peak flow period (min).
    pub peak_flow_period_min: f64,
    /// Growth (%).
    pub growth_pct: f64,
    /// NOT CONVERGED text, or empty.
    pub export_note: String,
    /// Origin leg label.
    pub from: String,
    /// Destination leg label.
    pub to: String,
    /// Year.
    pub year: f64,
    /// Where the peak flow factor came from.
    pub pff_src: &'static str,
    /// The entry's own id.
    pub id: String,
}

fn leg_index(code: &str) -> f64 {
    LEG_CODES.iter().position(|c| *c == code).map_or(-1.0, |i| i as f64)
}

fn num_cell(x: Option<f64>) -> Cell {
    x.map_or_else(blank, Cell::Num)
}

impl VolumeRow {
    /// The value of a column as a CSV cell (`r[k]`).
    fn cell(&self, k: &str) -> Cell {
        match k {
            "site_id" => txt(&self.site_id),
            "site_name" => txt(&self.site_name),
            "at_site_no" => at_site_cell_of(&self.at_site_no),
            "lat" => num_cell(self.lat),
            "lng" => num_cell(self.lng),
            "site_type" => Cell::Num(self.site_type),
            "period" => txt(&self.period),
            "origin_leg" => Cell::Num(self.origin_leg),
            "dest_leg" => Cell::Num(self.dest_leg),
            "turn" => txt(&self.turn),
            "lv_veh_h" => Cell::Num(self.lv_veh_h),
            "hv_veh_h" => Cell::Num(self.hv_veh_h),
            "pff_pct" => Cell::Num(self.pff_pct),
            "unit_time_min" => Cell::Num(self.unit_time_min),
            "peak_flow_period_min" => Cell::Num(self.peak_flow_period_min),
            "growth_pct" => Cell::Num(self.growth_pct),
            "export_note" => txt(&self.export_note),
            _ => blank(),
        }
    }

    /// The row as the loader script holds it: an object with the CSV's columns, in order.
    fn json(&self) -> JsonValue {
        JsonValue::from_pairs(VOL_HEAD.iter().map(|k| {
            let v = match *k {
                "site_id" => JsonValue::from(self.site_id.as_str()),
                "site_name" => JsonValue::from(self.site_name.as_str()),
                "at_site_no" => {
                    if self.at_site_no.is_null() {
                        JsonValue::from("")
                    } else {
                        JsonValue::from_value(&self.at_site_no)
                    }
                }
                "lat" => self.lat.map_or(JsonValue::from(""), JsonValue::from),
                "lng" => self.lng.map_or(JsonValue::from(""), JsonValue::from),
                "site_type" => JsonValue::from(self.site_type),
                "period" => JsonValue::from(self.period.as_str()),
                "origin_leg" => JsonValue::from(self.origin_leg),
                "dest_leg" => JsonValue::from(self.dest_leg),
                "turn" => JsonValue::from(self.turn.as_str()),
                "lv_veh_h" => JsonValue::from(self.lv_veh_h),
                "hv_veh_h" => JsonValue::from(self.hv_veh_h),
                "pff_pct" => JsonValue::from(self.pff_pct),
                "unit_time_min" => JsonValue::from(self.unit_time_min),
                "peak_flow_period_min" => JsonValue::from(self.peak_flow_period_min),
                _ => JsonValue::from(self.growth_pct),
            };
            (k.to_string(), v)
        }))
    }

    /// The row as JSON for comparison: every column and the private fields, as the prototype's row object.
    pub fn to_value(&self) -> Value {
        use crate::jsnum::plain_number;
        let mut m = serde_json::Map::new();
        for k in VOL_HEAD {
            let v = match k {
                "site_id" => Value::from(self.site_id.clone()),
                "site_name" => Value::from(self.site_name.clone()),
                "at_site_no" => {
                    if self.at_site_no.is_null() {
                        Value::from("")
                    } else {
                        self.at_site_no.clone()
                    }
                }
                "lat" => self.lat.map_or(Value::from(""), plain_number),
                "lng" => self.lng.map_or(Value::from(""), plain_number),
                "site_type" => plain_number(self.site_type),
                "period" => Value::from(self.period.clone()),
                "origin_leg" => plain_number(self.origin_leg),
                "dest_leg" => plain_number(self.dest_leg),
                "turn" => Value::from(self.turn.clone()),
                "lv_veh_h" => plain_number(self.lv_veh_h),
                "hv_veh_h" => plain_number(self.hv_veh_h),
                "pff_pct" => plain_number(self.pff_pct),
                "unit_time_min" => plain_number(self.unit_time_min),
                "peak_flow_period_min" => plain_number(self.peak_flow_period_min),
                _ => plain_number(self.growth_pct),
            };
            m.insert(k.to_string(), v);
        }
        m.insert("export_note".into(), Value::from(self.export_note.clone()));
        m.insert("_from".into(), Value::from(self.from.clone()));
        m.insert("_to".into(), Value::from(self.to.clone()));
        m.insert("_year".into(), plain_number(self.year));
        m.insert("_pffSrc".into(), Value::from(self.pff_src));
        m.insert("_id".into(), Value::from(self.id.clone()));
        Value::Object(m)
    }
}

fn at_site_cell_of(v: &Value) -> Cell {
    if v.is_null() { blank() } else { Cell::Val(v.clone()) }
}

/// The gate for turn-volume exports (`Turns.sidraExport`): whether the runs are converged and fresh enough, and if not, why.
#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    /// Fit to export.
    pub ok: bool,
    /// Why not.
    pub why: String,
}

impl<'a> Tools<'a> {
    /// `gate(sc, resList)`: the export gate for these results.
    pub fn gate(&self, sc: &Scenario, res_list: &[&Res]) -> Gate {
        let r = sidra_export(
            Some(sc),
            res_list,
            self.st(),
            Some(&[]),
            &TurnsSidraOpts::default(),
        );
        Gate {
            ok: r.ok,
            why: r.why,
        }
    }

    /// `bothRes(sc)`: the stored results in period order.
    pub fn both_res(&self, s: Sc<'a>) -> Vec<&'a Res> {
        s.periods().iter().filter_map(|p| s.res_of(p)).collect()
    }

    /// `targets(sc, res, ids, scope)`: the junctions that go to SIDRA: the ids given, else the scope: 'auto' (the sites of the
    /// extent test, else every controlled junction), 'controlled' or 'all'.
    pub fn targets(
        &self,
        sc: &Scenario,
        res: &'a Res,
        ids: Option<&[String]>,
        scope: &str,
    ) -> Vec<Entry<'a>> {
        let all = entries(sc, res, true);
        if let Some(ids) = ids.filter(|i| !i.is_empty()) {
            return all
                .into_iter()
                .filter(|e| ids.contains(&e.id) || e.nodes.iter().any(|n| ids.contains(n)))
                .collect();
        }
        let ext: Option<Vec<String>> = sc
            .extent
            .get()
            .and_then(|e| e.get("sites"))
            .and_then(|s| s.as_array())
            .filter(|a| !a.is_empty())
            .map(|a| {
                a.iter()
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        other => js_str(other),
                    })
                    .collect()
            });
        if scope == "auto"
            && let Some(ext) = ext
        {
            return all
                .into_iter()
                .filter(|e| ext.contains(&e.id) || e.nodes.iter().any(|n| ext.contains(n)))
                .collect();
        }
        if scope == "all" {
            all
        } else {
            all.into_iter()
                .filter(|e| e.nr.is_some() || e.kind != "none")
                .collect()
        }
    }

    /// `phfOf(sc, e, period)`: the peak hour factor of an imported turning count at any node of the junction, when there is one.
    pub fn phf_of(&self, sc: &Scenario, e: &Entry, period: &str) -> Option<f64> {
        for id in &e.nodes {
            if let Some(f) = self.host.count_phf(sc, id, period)
                && f > 0.25
                && f <= 1.0
            {
                return Some(f);
            }
        }
        None
    }

    /// `volumeRows(sc, resList, ids, opts)`: every movement of the target junctions as SIDRA volumes.
    pub fn volume_rows(
        &self,
        sc: &Scenario,
        res_list: &[&'a Res],
        ids: Option<&[String]>,
        scope: &str,
    ) -> Vec<VolumeRow> {
        let mut rows = Vec::new();
        for res in res_list {
            let res: &'a Res = res;
            let j_pff = res.junction.pff;
            let s_pff = self.st().junction.get().map_or(f64::NAN, |j| j.pff.num());
            let pff_base = if j_pff != 0.0 && !j_pff.is_nan() {
                j_pff
            } else if s_pff != 0.0 && !s_pff.is_nan() {
                s_pff
            } else {
                1.0
            };
            let pff0 = crate::jsnum::math_round(pff_base * 100.0);
            let nc = nc_text(res);
            for e in self.targets(sc, res, ids, scope) {
                // pff_pct: 100 x the PHF of a turning count at this junction in this period, else the model's PFF
                let phf = self.phf_of(sc, &e, &res.period);
                let pff = match phf {
                    Some(f) => crate::jsnum::math_round(f * 100.0),
                    None => pff0,
                };
                for m in moves_of(sc, self.st(), res, &e) {
                    let (lv, hv, t) = split(m.v, m.hv);
                    if t == 0.0 {
                        continue;
                    }
                    let od = leg_index(m.leg);
                    let dd = leg_index(m.to_leg);
                    rows.push(VolumeRow {
                        site_id: xid(&e),
                        site_name: e.name.clone(),
                        at_site_no: e.at_site_no.clone(),
                        lat: e.ll.map(|l| r7(l[0])),
                        lng: e.ll.map(|l| r7(l[1])),
                        site_type: sidra_site_type(&e.kind).unwrap_or(2.0),
                        period: res.period.clone(),
                        origin_leg: od,
                        dest_leg: dd,
                        turn: m.turn.to_string(),
                        lv_veh_h: lv,
                        hv_veh_h: hv,
                        pff_pct: pff,
                        unit_time_min: SIDRA_UNIT_TIME,
                        peak_flow_period_min: if phf.is_some() {
                            SIDRA_PHF_PERIOD
                        } else {
                            SIDRA_PEAK_FLOW_PERIOD
                        },
                        growth_pct: SIDRA_GROWTH_PCT,
                        export_note: nc.clone(),
                        from: m.leg_label.clone(),
                        to: leg_label(dd as usize, &m.to_road),
                        year: res.year,
                        pff_src: if phf.is_some() { "turning count PHF" } else { "model setting" },
                        id: e.id.clone(),
                    });
                }
            }
        }
        rows
    }

    /// `volumesCSV(sc, resList, ids, opts)`: the SIDRA volumes CSV; a forced export of an unconverged run gains a last column,
    /// export_note, saying NOT CONVERGED on its rows. The error is the gate's reason.
    pub fn volumes_csv(
        &self,
        s: Sc<'a>,
        res_list: Option<&[&'a Res]>,
        ids: Option<&[String]>,
        opts: &SidraOpts,
    ) -> Result<String, String> {
        let list: Vec<&'a Res> = match res_list {
            Some(l) => l.to_vec(),
            None => self.both_res(s),
        };
        if !opts.force {
            let g = self.gate(s.sc, &list);
            if !g.ok {
                return Err(g.why);
            }
        }
        let rows = self.volume_rows(s.sc, &list, ids, &opts.scope);
        let noted = rows.iter().any(|r| !r.export_note.is_empty());
        let mut head: Vec<&str> = VOL_HEAD.to_vec();
        if noted {
            head.push("export_note");
        }
        let mut out: Vec<Vec<Cell>> = vec![head.iter().map(|h| txt(h)).collect()];
        for r in &rows {
            out.push(head.iter().map(|k| r.cell(k)).collect());
        }
        Ok(write_csv(&out))
    }

    /// `legLanes(sc, res, e)`: the lanes (1 to 3) on each leg's approach, by leg code, so the loader can add legs with roughly
    /// the right number of lanes.
    fn leg_lanes(&self, sc: &Scenario, res: &Res, e: &Entry) -> Vec<(i64, f64)> {
        let mut out: Vec<(i64, f64)> = Vec::new();
        for a in legs_of(sc, self.st(), res, e) {
            let mut n = 1.0f64;
            for k in &a.in_keys {
                let i = k.rfind(':').map_or(-1, |p| p as isize);
                let id = if i < 0 { &k[..] } else { &k[..i as usize] };
                if let Some(l) = sc.links.get(id) {
                    let lanes = if k.ends_with(":ab") { l.lanes_ab.int32() } else { l.lanes_ba.int32() };
                    n = math_max(n, lanes as f64);
                }
            }
            let v = math_max(1.0, math_min(3.0, n));
            let key = a.arm.o as i64;
            match out.iter().position(|(k, _)| *k == key) {
                Some(p) => out[p].1 = v,
                None => out.push((key, v)),
            }
        }
        out
    }

    /// `loaderPy(sc, resList, ids, opts)`: load_corridor_volumes.py, a SIDRA INTERSECTION 11 .NET API loader (pythonnet) with the
    /// New Zealand setup and a --dry-run. The error is the gate's reason.
    pub fn loader_py(
        &self,
        s: Sc<'a>,
        res_list: Option<&[&'a Res]>,
        ids: Option<&[String]>,
        opts: &SidraOpts,
    ) -> Result<String, String> {
        let list: Vec<&'a Res> = match res_list {
            Some(l) => l.to_vec(),
            None => self.both_res(s),
        };
        if !opts.force {
            let g = self.gate(s.sc, &list);
            if !g.ok {
                return Err(g.why);
            }
        }
        let sc = s.sc;
        let rows = self.volume_rows(sc, &list, ids, &opts.scope);
        // legs by junction: the lanes on each leg's approach
        let mut legs: Vec<(String, Vec<(i64, f64)>)> = Vec::new();
        for res in &list {
            for e in self.targets(sc, res, ids, &opts.scope) {
                let x = xid(&e);
                let new = self.leg_lanes(sc, res, &e);
                let p = match legs.iter().position(|(k, _)| *k == x) {
                    Some(p) => p,
                    None => {
                        legs.push((x.clone(), Vec::new()));
                        legs.len() - 1
                    }
                };
                for (k, v) in new {
                    match legs[p].1.iter().position(|(a, _)| *a == k) {
                        Some(i) => legs[p].1[i].1 = v,
                        None => legs[p].1.push((k, v)),
                    }
                }
            }
        }
        let rows_json = JsonValue::Array(rows.iter().map(VolumeRow::json).collect());
        let legs_json = JsonValue::from_pairs(legs.iter().map(|(k, lanes)| {
            let mut sorted = lanes.clone();
            sorted.sort_by_key(|(a, _)| *a);
            (
                k.clone(),
                JsonValue::from_pairs(
                    sorted
                        .iter()
                        .map(|(a, v)| (a.to_string(), JsonValue::from(*v))),
                ),
            )
        }));
        // JSON inside a Python raw string: no ''' can occur because ' is written as '
        let js = |x: &JsonValue| py_unicode(&x.stringify().replace('\'', "\\u0027"));
        let py_str = |t: &str| py_unicode(&JsonValue::from(t).stringify());
        // Project data in a comment: one line, no control characters, so nothing from a shared project file can become code
        let cm = |t: &str| -> String {
            t.chars()
                .map(|c| {
                    let u = c as u32;
                    if u <= 0x1f || (0x7f..=0x9f).contains(&u) || u == 0x2028 || u == 0x2029 {
                        ' '
                    } else {
                        c
                    }
                })
                .collect()
        };
        let year = list
            .iter()
            .map(|r| number_to_js_string(r.year))
            .find(|y| y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or_default();
        let sites = {
            let mut seen: Vec<&str> = Vec::new();
            for r in &rows {
                if !seen.contains(&r.site_id.as_str()) {
                    seen.push(&r.site_id);
                }
            }
            seen.len()
        };
        let periods = {
            let mut seen: Vec<&str> = Vec::new();
            for r in &rows {
                if !seen.contains(&r.period.as_str()) {
                    seen.push(&r.period);
                }
            }
            let ok: Vec<&str> = seen
                .into_iter()
                .filter(|p| (1..=12).contains(&p.chars().count()) && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                .collect();
            if ok.is_empty() { "none".to_string() } else { ok.join(", ") }
        };
        let nc = if opts.force {
            let mut parts: Vec<String> = Vec::new();
            for r in &list {
                let t = nc_text(r);
                if !t.is_empty() {
                    let p = format!("{}: {}", per_name(&r.period), t);
                    if !parts.contains(&p) {
                        parts.push(p);
                    }
                }
            }
            parts.join("; ")
        } else {
            String::new()
        };
        // D1 and D12 as a comment block (wrapped, '# ' lines) and as a string the script prints once when it runs
        let wrap_c = |t: &str| -> String {
            let w = 112usize;
            let mut out: Vec<String> = Vec::new();
            let mut line = String::new();
            for word in split_ws(t) {
                if !line.is_empty() && line.encode_utf16().count() + word.encode_utf16().count() + 1 > w {
                    out.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else if line.is_empty() {
                    line = word.to_string();
                } else {
                    line = format!("{line} {word}");
                }
            }
            if !line.is_empty() {
                out.push(line);
            }
            out.iter().map(|l| format!("#   {l}")).collect::<Vec<_>>().join("\n")
        };
        let notice = format!("D1 Screening status: {D1}\nD12 Hand-over: {D12}");
        let name = s.name_or_id();
        let nc_part = if nc.is_empty() {
            String::new()
        } else {
            format!(
                "\n# {}\n# These volumes come from a run that has not converged far enough for SIDRA. Do not rely on them.\n#",
                cm(&nc)
            )
        };
        let vals: [String; 15] = [
            wrap_c(D1),
            wrap_c(D12),
            nc_part,
            opts.date_iso.clone(),
            cm(&name),
            year,
            cm(&periods),
            rows.len().to_string(),
            sites.to_string(),
            number_to_js_string(SIDRA_SETUP),
            py_str(&name),
            py_str(&nc),
            py_str(&notice),
            js(&rows_json),
            js(&legs_json),
        ];
        let mut out = String::new();
        for (i, seg) in LOADER.iter().enumerate() {
            out.push_str(seg);
            if let Some(v) = vals.get(i) {
                out.push_str(v);
            }
        }
        Ok(out)
    }

    /// `gridsHtml(sc, resList, ids, opts)`: printable volume grids per site and period: rows are legs, columns turn by class.
    pub fn grids_html(
        &self,
        s: Sc<'a>,
        res_list: Option<&[&'a Res]>,
        ids: Option<&[String]>,
        opts: &SidraOpts,
    ) -> String {
        let list: Vec<&'a Res> = match res_list {
            Some(l) => l.to_vec(),
            None => self.both_res(s),
        };
        let sc = s.sc;
        let rows = self.volume_rows(sc, &list, ids, &opts.scope);
        // groups by site and period, in the order first met
        let mut by: Vec<(String, Vec<&VolumeRow>)> = Vec::new();
        for r in &rows {
            let k = format!("{}|{}", r.site_id, r.period);
            match by.iter().position(|(x, _)| *x == k) {
                Some(p) => by[p].1.push(r),
                None => by.push((k, vec![r])),
            }
        }
        let g0 = self.gate(sc, &list);
        let parts: String = by
            .iter()
            .map(|(_, rs)| {
                let r = rs[0];
                let turns: Vec<&str> = TURN_ORDER
                    .iter()
                    .copied()
                    .filter(|t| rs.iter().any(|x| x.turn == *t))
                    .collect();
                let mut legs: Vec<f64> = Vec::new();
                for x in rs {
                    if !legs.contains(&x.origin_leg) {
                        legs.push(x.origin_leg);
                    }
                }
                legs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let cell = |o: f64, t: &str, lv: bool| -> String {
                    let x = rs
                        .iter()
                        .filter(|y| y.origin_leg == o && y.turn == t)
                        .fold(0.0, |s, y| s + if lv { y.lv_veh_h } else { y.hv_veh_h });
                    if x != 0.0 { number_to_js_string(x) } else { String::new() }
                };
                let body: String = legs
                    .iter()
                    .map(|&o| {
                        let lab = match rs.iter().find(|x| x.origin_leg == o) {
                            Some(x) if !x.from.is_empty() => x.from.clone(),
                            _ => leg_name_at(o),
                        };
                        let tot = rs
                            .iter()
                            .filter(|x| x.origin_leg == o)
                            .fold(0.0, |s, x| s + x.lv_veh_h + x.hv_veh_h);
                        format!(
                            "<tr><th>{}</th>{}<td><b>{}</b></td></tr>",
                            esc(&lab),
                            turns
                                .iter()
                                .map(|t| format!("<td>{}</td><td>{}</td>", cell(o, t, true), cell(o, t, false)))
                                .collect::<String>(),
                            number_to_js_string(tot)
                        )
                    })
                    .collect();
                let tot = rs.iter().fold(0.0, |s, x| s + x.lv_veh_h + x.hv_veh_h);
                let sum = |t: &str, lv: bool| {
                    let v = rs
                        .iter()
                        .filter(|x| x.turn == t)
                        .fold(0.0, |s, x| s + if lv { x.lv_veh_h } else { x.hv_veh_h });
                    if v != 0.0 { number_to_js_string(v) } else { String::new() }
                };
                let when_h = if r.period == "IP" {
                    "inter-peak average hour".to_string()
                } else {
                    format!("{} hour", per_name(&r.period))
                };
                format!(
                    "<section><h2>{} \u{b7} {}{}</h2>\n        <p class=\"meta\">Site {}{} \u{b7} {} \u{b7} PFF {}% over {} min ({}) \u{b7} vehicles per hour</p>\n        <table><tr><th rowspan=\"2\">Approach leg</th>{}<th rowspan=\"2\">Total</th></tr><tr>{}</tr>{}\n        <tr><th>Total</th>{}<td><b>{}</b></td></tr></table></section>",
                    esc(&r.site_name),
                    esc(&when_h),
                    if r.year != 0.0 && !r.year.is_nan() {
                        format!(" {}", esc(&number_to_js_string(r.year)))
                    } else {
                        String::new()
                    },
                    esc(&r.site_id),
                    if r.at_site_no.is_null() {
                        String::new()
                    } else {
                        format!(" \u{b7} AT site {}", esc(&js_str(&r.at_site_no)))
                    },
                    esc(sidra_type_name(r.site_type)),
                    esc(&number_to_js_string(r.pff_pct)),
                    esc(&number_to_js_string(r.peak_flow_period_min)),
                    esc(r.pff_src),
                    turns.iter().map(|t| format!("<th colspan=\"2\">{t}</th>")).collect::<String>(),
                    turns.iter().map(|_| "<th>LV</th><th>HV</th>").collect::<String>(),
                    body,
                    turns
                        .iter()
                        .map(|t| format!("<td>{}</td><td>{}</td>", sum(t, true), sum(t, false)))
                        .collect::<String>(),
                    number_to_js_string(tot)
                )
            })
            .collect();
        let name = s.sc.name.as_str().unwrap_or("");
        let g = &GRIDS;
        let warn = if g0.ok {
            String::new()
        } else {
            format!("<p class=\"warn\">{}</p>", esc(&g0.why))
        };
        let parts = if parts.is_empty() {
            "<p>No turning volumes. Run the model first.</p>".to_string()
        } else {
            parts
        };
        let vals: [String; 10] = [
            esc(name),
            esc(LABEL),
            esc(D1),
            esc(D12),
            esc(name),
            esc(&opts.date_long),
            esc(D1),
            esc(D12),
            warn,
            parts,
        ];
        let mut out = String::new();
        for (i, seg) in g.iter().enumerate() {
            out.push_str(seg);
            if let Some(v) = vals.get(i) {
                out.push_str(v);
            }
        }
        out
    }
}

fn leg_name_at(o: f64) -> String {
    if o >= 0.0 && o.fract() == 0.0 && (o as usize) < LEG_NAMES.len() {
        LEG_NAMES[o as usize].to_string()
    } else {
        // `Turns.LEG_NAMES[o]` of an unknown code is undefined: esc(undefined) is empty
        String::new()
    }
}

fn sidra_type_name(t: f64) -> &'static str {
    if t >= 0.0 && t.fract() == 0.0 && (t as usize) < SIDRA_TYPE_NAME.len() {
        SIDRA_TYPE_NAME[t as usize]
    } else {
        "give-way"
    }
}

/// `.replace(/[\u007f-￿]/g, c => '\\u' + hex4)`: every UTF-16 code unit from U+007F becomes a `\uXXXX` escape.
fn py_unicode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut buf = [0u16; 2];
    for c in s.chars() {
        if (c as u32) < 0x7f {
            out.push(c);
        } else {
            for u in c.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{:04x}", u));
            }
        }
    }
    out
}

/// `String(s).split(/\s+/)` for text without empty parts at the ends.
fn split_ws(t: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut in_ws = false;
    let mut first = true;
    for (i, c) in t.char_indices() {
        let ws = crate::jsnum::is_js_space(c);
        if ws && !in_ws {
            if !(first && i == 0) {
                out.push(&t[start..i]);
            } else {
                out.push("");
            }
            in_ws = true;
        } else if !ws && in_ws {
            start = i;
            in_ws = false;
        }
        first = false;
    }
    if !in_ws {
        out.push(&t[start..]);
    } else {
        out.push("");
    }
    out
}

/* ---------------- the volumes CSV read back as turning counts ---------------- */

/// A value read from a volumes CSV: a number for the numeric columns, text otherwise.
#[derive(Clone, Debug, PartialEq)]
pub enum CsvValue {
    /// A number (`+text`, NaN when the text is not a number).
    Num(f64),
    /// Text.
    Text(String),
}

/// A volumes CSV row: the columns in the order of the header.
pub type CsvRow = Vec<(String, CsvValue)>;

/// `parseVolumesCSV(text)`: reads the volumes CSV back, with numbers where the column is numeric.
pub fn parse_volumes_csv(text: &str) -> Vec<CsvRow> {
    const NUM: [&str; 11] = [
        "lat",
        "lng",
        "site_type",
        "origin_leg",
        "dest_leg",
        "lv_veh_h",
        "hv_veh_h",
        "pff_pct",
        "unit_time_min",
        "peak_flow_period_min",
        "growth_pct",
    ];
    let mut rows = crate::calibrate::parse_csv(text);
    let head: Vec<String> = if rows.is_empty() { Vec::new() } else { rows.remove(0) };
    rows.iter()
        .map(|r| {
            let mut o: CsvRow = Vec::new();
            for (i, h) in head.iter().enumerate() {
                let cell = r.get(i);
                let v = if NUM.contains(&h.as_str()) && cell.is_none_or(|c| !c.is_empty()) {
                    CsvValue::Num(match cell {
                        Some(c) => crate::jsnum::string_to_number(c),
                        None => f64::NAN,
                    })
                } else {
                    CsvValue::Text(cell.cloned().unwrap_or_default())
                };
                match o.iter().position(|(k, _)| k == h) {
                    Some(p) => o[p].1 = v,
                    None => o.push((h.clone(), v)),
                }
            }
            o
        })
        .collect()
}

/// The volumes the turning-count layout is made from: the CSV text, or the rows.
pub enum VolumeSource<'a> {
    /// The volumes CSV.
    Text(&'a str),
    /// Rows from [`Tools::volume_rows`].
    Rows(&'a [VolumeRow]),
}

struct CountRow {
    site_id: Cell,
    site_name: Cell,
    lat: Cell,
    lng: Cell,
    period: String,
    origin_leg: f64,
    turn: Cell,
    lv: f64,
    hv: f64,
}

fn count_rows(src: &VolumeSource) -> Vec<CountRow> {
    match src {
        VolumeSource::Rows(rows) => rows
            .iter()
            .map(|r| CountRow {
                site_id: txt(&r.site_id),
                site_name: txt(&r.site_name),
                lat: num_cell(r.lat),
                lng: num_cell(r.lng),
                period: r.period.clone(),
                origin_leg: r.origin_leg,
                turn: txt(&r.turn),
                lv: r.lv_veh_h,
                hv: r.hv_veh_h,
            })
            .collect(),
        VolumeSource::Text(t) => parse_volumes_csv(t)
            .into_iter()
            .map(|row| {
                let get = |k: &str| row.iter().find(|(h, _)| h == k).map(|(_, v)| v.clone());
                let cell = |k: &str| match get(k) {
                    Some(CsvValue::Num(x)) => Cell::Num(x),
                    Some(CsvValue::Text(s)) => Cell::Text(s),
                    None => blank(),
                };
                let num = |k: &str| match get(k) {
                    Some(CsvValue::Num(x)) => x,
                    _ => f64::NAN,
                };
                CountRow {
                    site_id: cell("site_id"),
                    site_name: cell("site_name"),
                    lat: cell("lat"),
                    lng: cell("lng"),
                    period: match get("period") {
                        Some(CsvValue::Text(s)) => s,
                        Some(CsvValue::Num(x)) => number_to_js_string(x),
                        None => String::new(),
                    },
                    origin_leg: num("origin_leg"),
                    turn: cell("turn"),
                    lv: num("lv_veh_h"),
                    hv: num("hv_veh_h"),
                }
            })
            .collect(),
    }
}

impl<'a> Tools<'a> {
    /// `asCountCSV(src, date)`: the volumes in Spare Construct's turning-count layout (long: site_id, lat, lng, time_start,
    /// interval_min, from_leg, movement, class, count), so it re-imports as a one-hour count at the start of each period's
    /// peak-hour window.
    pub fn as_count_csv(&self, src: &VolumeSource, date: &str) -> String {
        let windows: Option<Value> = self
            .st()
            .calib
            .get()
            .and_then(|c| c.extra.get("windows"))
            .cloned();
        let mut out: Vec<Vec<Cell>> = vec![COUNT_HEAD.iter().map(|h| txt(h)).collect()];
        for r in count_rows(src) {
            for (cls, v) in [("LV", r.lv), ("HV", r.hv)] {
                if !(v > 0.0) {
                    continue;
                }
                let w0 = windows
                    .as_ref()
                    .and_then(|w| w.get(&r.period))
                    .and_then(|p| p.get(0))
                    .filter(|x| crate::jsnum::value_truthy(x))
                    .map(js_str);
                let start = match w0 {
                    Some(t) => t,
                    None => {
                        let d = window_start(&r.period);
                        if d.is_empty() { "07:00".to_string() } else { d.to_string() }
                    }
                };
                let leg = if r.origin_leg >= 0.0 && r.origin_leg.fract() == 0.0 && (r.origin_leg as usize) < LEG_CODES.len() {
                    txt(LEG_CODES[r.origin_leg as usize])
                } else {
                    blank()
                };
                out.push(vec![
                    r.site_id.clone(),
                    r.site_name.clone(),
                    r.lat.clone(),
                    r.lng.clone(),
                    txt(date),
                    txt(&start),
                    Cell::Num(60.0),
                    leg,
                    r.turn.clone(),
                    txt(cls),
                    Cell::Num(v),
                ]);
            }
        }
        write_csv(&out)
    }
}

/// Keeps the helper used by the other exports linked.
#[allow(dead_code)]
fn _unused(e: &Entry) -> Cell {
    at_site_cell(e)
}

#[allow(dead_code)]
fn _num(v: &Value) -> f64 {
    value_to_number(v)
}
