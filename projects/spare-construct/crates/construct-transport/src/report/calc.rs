//! The report's calculations (pure, tested): crash risk, crash prediction, pedestrian delay, Safe System scores and the staging
//! trigger.

use super::consts::{CEC_MODELS, cec, hrig, ped};
use super::js;
use crate::edit::json::JsonValue;
use crate::graph::js_sort_by;
use crate::jsmath;
use crate::jsnum::{math_max, or_default};
use crate::util::fmt;
use std::collections::HashSet;

/// `pof(legs)`: the HRIG product of flows from the two-way AADTs [major 1, major 2, minor 1, minor 2] (the second minor leg is 0 at a T).
pub fn pof(legs: &[f64]) -> f64 {
    let g = |i: usize| or_default(legs.get(i).copied().unwrap_or(0.0), 0.0);
    let (a, b, c, d) = (g(0), g(1), g(2), g(3));
    jsmath::pow(((a + b) / 2.0) * ((c + d) / 2.0), 0.4)
}

/// `personalRisk(dsi5, pf)`: DSI equivalents (5 years) per 100 million vehicles through the junction.
pub fn personal_risk(dsi5: f64, pf: f64) -> Option<f64> {
    if pf > 0.0 {
        Some(dsi5 * 1e8 / (pf * hrig::YEARS * 365.0 * hrig::FACTOR))
    } else {
        None
    }
}

/// `collectiveBand(dsi5, fs5)`: the HRIG collective risk band; High with three or more fatal and serious crashes.
pub fn collective_band(dsi5: f64, fs5: f64) -> &'static str {
    if fs5 >= hrig::FS_HIGH {
        return "High";
    }
    hrig::COLLECTIVE
        .iter()
        .find(|(cut, _)| dsi5 >= *cut)
        .map_or("Low", |(_, b)| *b)
}

/// The personal risk band of a site and whether it rests on enough injury crashes.
#[derive(Clone, Debug, PartialEq)]
pub struct PersonalBand {
    /// The band, none without a volume.
    pub band: Option<&'static str>,
    /// The band as printed ('High (indicative)' when it rests on too few crashes).
    pub label: String,
    /// Four or more injury crashes.
    pub reliable: bool,
}

/// `personalBand(pr, injury5)`: High and Medium-high need 4 or more injury crashes in 5 years, otherwise 'indicative'.
pub fn personal_band(pr: Option<f64>, injury5: f64) -> PersonalBand {
    let Some(pr) = pr.filter(|p| p.is_finite()) else {
        return PersonalBand {
            band: None,
            label: "No volume".to_string(),
            reliable: false,
        };
    };
    let band = hrig::PERSONAL
        .iter()
        .find(|(cut, _, gt)| if *gt { pr > *cut } else { pr >= *cut })
        .map_or("Low", |(_, b, _)| *b);
    let reliable = injury5 >= hrig::MIN_INJURY;
    PersonalBand {
        band: Some(band),
        label: if (band == "High" || band == "Medium-high") && !reliable {
            format!("{band} (indicative)")
        } else {
            band.to_string()
        },
        reliable,
    }
}

/// `severityIndex(flags, control, where)`: the CEC severity index of one crash at a site: vulnerable users first, then the vehicle index by control.
pub fn severity_index(flags: &str, control: &str, midblock: bool) -> f64 {
    if flags.contains('p') {
        return if midblock {
            cec::SI_PED_MID
        } else {
            cec::SI_PED_INT
        };
    }
    if flags.contains('c') {
        return if midblock {
            cec::SI_CYC_MID
        } else {
            cec::SI_CYC_INT
        };
    }
    if flags.contains('m') {
        return if midblock {
            cec::SI_MC_MID
        } else {
            cec::SI_MC_INT
        };
    }
    if midblock {
        return cec::SI_MIDBLOCK;
    }
    match control {
        "signals" => cec::SI_SIGNALS,
        "roundabout" => cec::SI_ROUNDABOUT,
        "priority" | "stop" => cec::SI_PRIORITY,
        _ => cec::SI_GENERIC,
    }
}

/// A crash prediction at a junction.
#[derive(Clone, Debug, PartialEq)]
pub struct CecPrediction {
    /// Model key.
    pub model: &'static str,
    /// Model label.
    pub label: &'static str,
    /// Predicted injury crashes a year.
    pub a: f64,
    /// Whether the flows are in the model's valid range.
    pub flag: String,
    /// Flows outside the recorded range.
    pub outside: bool,
}

/// `cecPredict(modelId, qmaj, qmin)`: A = b0 x Qmaj^b1 x Qmin^b2 (injury crashes a year; two-way AADT); flags flows outside the recorded range.
pub fn cec_predict(model_id: &str, qmaj: f64, qmin: f64) -> Option<CecPrediction> {
    let m = CEC_MODELS.iter().find(|m| m.key == model_id)?;
    let a = if m.per_approach {
        m.b0 * jsmath::pow(math_max(0.0, qmaj), m.b1)
    } else {
        m.b0 * jsmath::pow(math_max(0.0, qmaj), m.b1) * jsmath::pow(math_max(0.0, qmin), m.b2)
    };
    let mut flag = String::new();
    if let Some((maj, min)) = m.range {
        let mut out: Vec<String> = Vec::new();
        if qmaj < maj[0] || qmaj > maj[1] {
            out.push(format!(
                "Qmaj {} outside {}\u{2013}{}",
                fmt::int(qmaj),
                fmt::int(maj[0]),
                fmt::int(maj[1])
            ));
        }
        if qmin < min[0] || qmin > min[1] {
            out.push(format!(
                "Qmin {} outside {}\u{2013}{}",
                fmt::int(qmin),
                fmt::int(min[0]),
                fmt::int(min[1])
            ));
        }
        if !out.is_empty() {
            flag = format!("outside model range: {}", out.join("; "));
        }
    } else {
        flag = "range not recorded (check CEC Table 7-3)".to_string();
    }
    Some(CecPrediction {
        model: m.key,
        label: m.label,
        a,
        outside: flag.contains("outside"),
        flag,
    })
}

/// `cecModelFor(control, legs, multiLane)`: the crash model of a junction by control and legs.
pub fn cec_model_for(control: &str, legs: usize, multi_lane: bool) -> Option<&'static str> {
    if control == "roundabout" {
        return Some(if multi_lane { "rbtMulti" } else { "rbt1" });
    }
    let cross = legs >= 4;
    if control == "signals" {
        return Some(if cross { "signalsCross" } else { "signalsT" });
    }
    if control == "priority" || control == "stop" {
        return Some(if cross { "priorityCross" } else { "priorityT" });
    }
    if cross { None } else { Some("uncontrolledT") }
}

/// What `pedDelay` reads of a crossing (loose numbers from the author's table).
#[derive(Clone, Copy, Debug, Default)]
pub struct PedInput {
    /// Signal cycle (s).
    pub c: f64,
    /// Walk green (s).
    pub g: f64,
    /// Crossing length (m).
    pub l: f64,
    /// Conflicting flow (veh/h, both directions).
    pub flow: f64,
    /// Walking speed (m/s); 0 or NaN for the standard.
    pub sp: f64,
    /// Start-up time (s); NaN for the standard.
    pub ts: f64,
}

/// `pedDelay(control, o)`: pedestrian delay (s) at a crossing; None where it cannot be worked out.
pub fn ped_delay(control: &str, o: &PedInput) -> Option<f64> {
    let sp = or_default(o.sp, ped::SP);
    let ts = if o.ts.is_nan() { ped::TS } else { o.ts };
    if control == "signals" {
        let (c, g) = (o.c, o.g);
        return if c > 0.0 && g >= 0.0 && g <= c {
            Some((c - g) * (c - g) / (2.0 * c))
        } else {
            None
        };
    }
    if control == "zebra" {
        return Some(0.0);
    }
    let unc = |flow: f64, l: f64| -> f64 {
        let lam = or_default(flow, 0.0) / 3600.0;
        let tc = or_default(l, 0.0) / sp + ts;
        if lam > 0.0 {
            (jsmath::exp(lam * tc) - lam * tc - 1.0) / lam
        } else {
            0.0
        }
    };
    if control == "refuge" {
        return Some(unc(or_default(o.flow, 0.0) / 2.0, or_default(o.l, 0.0) / 2.0) * 2.0);
    }
    Some(unc(o.flow, o.l))
}

/// `band4(v, cuts)`: a Safe System exposure score 1 to 4.
pub fn band4(v: Option<f64>, cuts: &[f64; 3]) -> Option<u8> {
    let v = v.filter(|x| x.is_finite())?;
    Some(if v < cuts[0] {
        1
    } else if v <= cuts[1] {
        2
    } else if v <= cuts[2] {
        3
    } else {
        4
    })
}

/// `sevHint(speed, safe)`: a Safe System severity score 1 to 4 from the posted speed.
pub fn sev_hint(speed: f64, safe: f64) -> Option<u8> {
    if !(speed > 0.0) {
        return None;
    }
    Some(if speed <= safe {
        1
    } else if speed <= safe + 20.0 {
        2
    } else if speed <= safe + 40.0 {
        3
    } else {
        4
    })
}

/// `WORSEN.dos`: an existing deficiency that worsens by this much is a staging trigger.
pub const WORSEN_DOS: f64 = 0.05;
/// `WORSEN.basis`.
pub const WORSEN_BASIS: &str = "Spare Construct assumption: an existing deficiency that worsens by 0.05 DoS or more is a staging trigger";

/// The first build-out increment where an assessed junction fails.
#[derive(Clone, Debug)]
pub struct Trigger {
    /// Position of the triggering row (in the rows as given), none when no site triggers.
    pub row: Option<usize>,
    /// The junctions that trigger.
    pub sites: Vec<String>,
    /// Why each triggers.
    pub how: Vec<(String, String)>,
    /// The last row before the trigger (in the rows as given).
    pub last_pass: Option<usize>,
    /// Junctions failing at 0%.
    pub existing: Vec<String>,
}

fn crit_of(r: &JsonValue, id: &str) -> Vec<String> {
    if let Some(c) = js::get(js::get(Some(r), "crit"), id).filter(|c| c.truthy()) {
        return js::items(Some(c))
            .iter()
            .map(JsonValue::js_string)
            .collect();
    }
    let fails = js::items(js::get(Some(r), "fails"));
    if fails.iter().any(|f| f.as_str() == Some(id)) {
        vec!["fail".to_string()]
    } else {
        Vec::new()
    }
}

/// `stagingTrigger(rows)` from build-out rows [{ pct, dwellings, gfa, fails: [site ids], crit: { id: ['dos', 'los', 'queue'] },
/// dos: { id: worst DoS } }]. A site triggers at the first increment where it fails a criterion it passed at 0% (so a junction
/// whose queue already exceeds its storage still triggers when it first exceeds practical DoS or reaches LoS E/F), or where a
/// criterion it already failed at 0% gets materially worse (DoS up by 0.05 or more).
pub fn staging_trigger(rows: &[JsonValue]) -> Trigger {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    js_sort_by(&mut order, |a, b| {
        js::num(js::get(Some(&rows[*a]), "pct")) - js::num(js::get(Some(&rows[*b]), "pct"))
    });
    let r0i = order
        .iter()
        .copied()
        .find(|i| js::num(js::get(Some(&rows[*i]), "pct")) == 0.0);
    let empty = JsonValue::object();
    let r0 = r0i.map_or(&empty, |i| &rows[i]);
    let base: Vec<String> = unique(
        js::items(js::get(Some(r0), "fails"))
            .iter()
            .map(JsonValue::js_string),
    );
    let dos0 = js::get(Some(r0), "dos").filter(|d| d.truthy());
    let mut last_pass: Option<usize> = None;
    for &ri in &order {
        let r = &rows[ri];
        if Some(ri) == r0i {
            last_pass = Some(ri);
            continue;
        }
        let mut nf: Vec<String> = Vec::new();
        let mut how: Vec<(String, String)> = Vec::new();
        for idv in js::items(js::get(Some(r), "fails")) {
            let id = idv.js_string();
            let was = crit_of(r0, &id);
            let now = crit_of(r, &id);
            let fresh: Vec<&String> = now.iter().filter(|k| !was.contains(k)).collect();
            let rd = js::get(Some(r), "dos").filter(|d| d.truthy());
            let (d_now, d_was) = (js::get(rd, &id), js::get(dos0, &id));
            let worse = !was.is_empty()
                && rd.is_some()
                && d_was.is_some_and(|v| !v.is_null())
                && d_now.is_some_and(|v| !v.is_null())
                && js::num(d_now) - js::num(d_was) >= WORSEN_DOS - 1e-9;
            if !fresh.is_empty() || worse {
                nf.push(id.clone());
                let text = if !fresh.is_empty() {
                    if !was.is_empty() {
                        format!(
                            "new criterion failed: {}",
                            fresh
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    } else {
                        "new failure".to_string()
                    }
                } else {
                    format!(
                        "existing deficiency worse by DoS {}",
                        fmt::num(js::num(d_now) - js::num(d_was), 2)
                    )
                };
                match how.iter().position(|(k, _)| *k == id) {
                    Some(p) => how[p].1 = text,
                    None => how.push((id, text)),
                }
            }
        }
        if !nf.is_empty() {
            return Trigger {
                row: Some(ri),
                sites: nf,
                how,
                last_pass,
                existing: base,
            };
        }
        last_pass = Some(ri);
    }
    Trigger {
        row: None,
        sites: Vec::new(),
        how: Vec::new(),
        last_pass,
        existing: base,
    }
}

fn unique(it: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for s in it {
        if seen.insert(s.clone()) {
            out.push(s);
        }
    }
    out
}

/// `ordinal(n)`: 1st, 2nd, 3rd, 4th.
pub fn ordinal(n: f64) -> String {
    let v = n % 100.0;
    let s = ["th", "st", "nd", "rd"];
    let pick = |i: f64| -> Option<&'static str> {
        if i >= 0.0 && i.fract() == 0.0 && (i as usize) < 4 {
            Some(s[i as usize])
        } else {
            None
        }
    };
    let suffix = pick((v - 20.0) % 10.0).or_else(|| pick(v)).unwrap_or(s[0]);
    format!("{}{}", crate::jsnum::number_to_js_string(n), suffix)
}

/// `triggerText(trig, measure)`: the staging condition in words.
pub fn trigger_text(rows: &[JsonValue], trig: &Trigger, measure: Option<&str>) -> String {
    let Some(ri) = trig.row else {
        return String::new();
    };
    let row = &rows[ri];
    let m = measure
        .filter(|m| !m.is_empty())
        .unwrap_or("<the mitigation measure>");
    let lp = trig.last_pass.map(|i| &rows[i]);
    let n = |v: Option<&JsonValue>, k: &str| js::num(js::get(v, k));
    if js::num(js::get(Some(row), "dwellings")) > 0.0 {
        let d = lp.map_or(0.0, |l| n(Some(l), "dwellings")).floor();
        return format!(
            "Prior to occupation of the {} dwelling, {}.",
            ordinal(d + 1.0),
            m
        );
    }
    if js::num(js::get(Some(row), "gfa")) > 0.0 {
        return format!(
            "Prior to occupation of more than {} m\u{b2} GFA, {}.",
            fmt::int(lp.map_or(0.0, |l| n(Some(l), "gfa"))),
            m
        );
    }
    format!(
        "Before the development passes {}% build-out, {}.",
        lp.map_or("0".to_string(), |l| crate::jsnum::number_to_js_string(n(
            Some(l),
            "pct"
        ))),
        m
    )
}

/// `tsv(rows)`: tab-separated text of a table's rows (arrays of cell text), each cell on one line and trimmed.
pub fn tsv(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|r| {
            r.iter()
                .map(|c| {
                    collapse_breaks(c)
                        .trim_matches(crate::jsnum::is_js_space)
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// `s.replace(/[\t\r\n]+/g, ' ')`.
fn collapse_breaks(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_run = false;
    for c in s.chars() {
        if matches!(c, '\t' | '\r' | '\n') {
            if !in_run {
                out.push(' ');
                in_run = true;
            }
        } else {
            out.push(c);
            in_run = false;
        }
    }
    out
}
