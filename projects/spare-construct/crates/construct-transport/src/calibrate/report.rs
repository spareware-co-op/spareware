//! The validation summary against NZTA TMDG 2019, Category E, with UK TAG M3.1 alongside (`Calibrate.report`).

use super::estimate::{MatrixChange, stats};
use super::sites::{SiteOpts, sites_for};
use super::stats::{GehBand, MIN_SITES, Rule, TMDG, rule_of};
use crate::datahub::Data;
use crate::jsnum::{number_to_js_string, plain_number, to_fixed};
use crate::project::{Project, Scenario};
use crate::results::{Res, plain};
use crate::util::fmt;
use serde::Serialize;
use serde_json::{Map, Value};

/// One validation test of a report (an entry of `report().checks`).
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    /// Rule id ("geh5", "r2", ...).
    pub id: &'static str,
    /// The rule it applies, with its source.
    pub rule: &'static Rule,
    /// The measured value; None when there is none.
    pub value: Option<f64>,
    /// The test is met; None when there is no value.
    pub ok: Option<bool>,
    /// The rule decides the overall status.
    pub required: bool,
    /// The rule is judged (not information only, and enough sites for a required one).
    pub judged: bool,
    /// "Pass", "Fail", "Fail (41 of 52; 45 needed)", "Info", "Indicative" or "N/A".
    pub status: String,
    /// The extra values of this test (got, need, of; clarify; slopeValue), overriding the rule's.
    pub extra: Vec<(&'static str, Value)>,
}

impl Check {
    /// The check as the prototype's object: the rule's fields, the id, value, flags and extras.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("id".to_string(), Value::String(self.id.to_string()));
        if let Value::Object(r) = serde_json::to_value(self.rule).expect("a rule serialises") {
            for (k, v) in r {
                m.insert(k, v);
            }
        }
        m.insert(
            "value".to_string(),
            self.value.map_or(Value::Null, plain_number),
        );
        m.insert("ok".to_string(), self.ok.map_or(Value::Null, Value::Bool));
        m.insert("required".to_string(), Value::Bool(self.required));
        m.insert("judged".to_string(), Value::Bool(self.judged));
        m.insert("status".to_string(), Value::String(self.status.clone()));
        for (k, v) in &self.extra {
            m.insert(k.to_string(), v.clone());
        }
        Value::Object(m)
    }
}

/// What a report adds when there is a result: the OLS fit and the GEH bands.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportFit {
    /// OLS slope.
    #[serde(with = "plain")]
    pub ols_slope: Option<f64>,
    /// OLS intercept.
    #[serde(with = "plain")]
    pub ols_intercept: Option<f64>,
    /// GEH bands.
    pub geh_bands: Vec<GehBand>,
}

/// The validation summary (`Calibrate.report`): the statistics, each test with its verdict, the overall status and the words that go with it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Period.
    pub period: String,
    /// The standard assessed against.
    pub standard: String,
    /// Number of counted sites.
    pub n: usize,
    /// Share with GEH < 5.
    #[serde(with = "plain")]
    pub geh5: Option<f64>,
    /// Share with GEH < 7.5.
    #[serde(with = "plain")]
    pub geh75: Option<f64>,
    /// Share with GEH < 10.
    #[serde(with = "plain")]
    pub geh10: Option<f64>,
    /// Share within the TMDG count band.
    #[serde(with = "plain")]
    pub band: Option<f64>,
    /// Share with GEH < 5 or within the TAG flow band.
    #[serde(with = "plain")]
    pub flow: Option<f64>,
    /// R² through the origin, centred.
    #[serde(with = "plain")]
    pub r2: Option<f64>,
    /// R² through the origin, Excel's.
    #[serde(with = "plain")]
    pub r2_excel: Option<f64>,
    /// OLS R².
    #[serde(with = "plain")]
    pub r2_ols: Option<f64>,
    /// Slope through the origin.
    #[serde(with = "plain")]
    pub slope: Option<f64>,
    /// %RMSE.
    #[serde(with = "plain")]
    pub rmse: Option<f64>,
    /// Σ modelled / Σ counted.
    #[serde(with = "plain")]
    pub ratio: Option<f64>,
    /// Σ counted.
    #[serde(with = "plain")]
    pub tot_obs: f64,
    /// Σ modelled.
    #[serde(with = "plain")]
    pub tot_mod: f64,
    /// The result's year.
    #[serde(with = "plain")]
    pub year: Option<f64>,
    /// The result is out of date.
    pub stale: bool,
    /// The tests.
    #[serde(serialize_with = "ser_checks")]
    pub checks: Vec<Check>,
    /// "none", "indicative", "not base year", "pass" or "fail".
    pub status: String,
    /// True for pass, false for fail, None otherwise.
    pub pass: Option<bool>,
    /// A sentence.
    pub summary: String,
    /// The period's matrix estimation record, as saved.
    pub matrix: Option<Value>,
    /// Warnings.
    pub warnings: Vec<String>,
    /// Notes.
    pub notes: Vec<String>,
    /// The OLS fit and GEH bands (present when there is a result).
    #[serde(flatten)]
    pub fit: Option<ReportFit>,
}

fn ser_checks<S: serde::Serializer>(c: &[Check], s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut q = s.serialize_seq(Some(c.len()))?;
    for x in c {
        q.serialize_element(&x.to_value())?;
    }
    q.end()
}

// `x === y` for the result's year against the settings' base year (strict: text "2026" is not 2026)
fn strict_year(year: f64, base: &crate::jsnum::Num) -> bool {
    matches!(base, crate::jsnum::Maybe::Set(crate::jsnum::JsNum::Number(b)) if *b == year)
}

/// `Calibrate.report(sc, period)`: the validation summary of a scenario's result against TMDG 2019 Category E, TAG M3.1 alongside.
///
/// `res` is the scenario's result for the period (`Store.res(sc, period)`, None when it has not been run).
/// Fewer than 10 counted sites give "indicative", never pass or fail; a result for another year than the base year is
/// "not base year". The counts used to estimate the matrix and to validate it are the same, which the notes say.
pub fn report(p: &Project, sc: &Scenario, period: &str, res: Option<&Res>) -> Report {
    let s = &p.settings;
    let info =
        p.me.get()
            .and_then(|m| m.info.get())
            .and_then(|i| i.get(period))
            .and_then(|i| i.get());
    let matrix = info.map(|i| serde_json::to_value(i).expect("a record serialises"));
    let mut out = Report {
        period: period.to_string(),
        standard: format!("{TMDG}, Category E (small area / corridor)"),
        n: 0,
        geh5: None,
        geh75: None,
        geh10: None,
        band: None,
        flow: None,
        r2: None,
        r2_excel: None,
        r2_ols: None,
        slope: None,
        rmse: None,
        ratio: None,
        tot_obs: 0.0,
        tot_mod: 0.0,
        year: res.map(|r| r.year),
        stale: res.is_some_and(|r| r.stale),
        checks: Vec::new(),
        status: "none".to_string(),
        pass: None,
        summary: String::new(),
        matrix,
        warnings: Vec::new(),
        notes: Vec::new(),
        fit: None,
    };
    let Some(res) = res else {
        out.notes
            .push(format!("Run the model for the {period} peak first."));
        return out;
    };
    let data = Data::read(p);
    let rows = sites_for(sc, res, period, &SiteOpts::default(), s, &data);
    let st = stats(&rows);
    out.n = st.n;
    out.geh5 = st.pass;
    out.geh75 = st.pass75;
    out.geh10 = st.pass10;
    out.band = st.band;
    out.flow = st.flow;
    out.r2 = st.r2;
    out.r2_excel = st.r2_excel;
    out.r2_ols = st.r2_ols;
    out.slope = st.slope;
    out.rmse = st.rmse;
    out.ratio = if st.tot_obs > 0.0 {
        Some(st.tot_mod / st.tot_obs)
    } else {
        None
    };
    out.tot_obs = st.tot_obs;
    out.tot_mod = st.tot_mod;
    out.fit = Some(ReportFit {
        ols_slope: st.ols_slope,
        ols_intercept: st.ols_intercept,
        geh_bands: st.geh_bands.clone(),
    });
    if st.n == 0 {
        out.notes.push(format!(
            "No counts match the network for the {period} peak. Load counts for this area first."
        ));
        return out;
    }
    let few = st.n < MIN_SITES;
    fn chk(
        out: &mut Report,
        few: bool,
        id: &'static str,
        value: Option<f64>,
        ok: bool,
        extra: Vec<(&'static str, Value)>,
    ) {
        let r = rule_of(id);
        let judged = r.use_ != "info" && !(few && r.use_ == "required");
        let status = if value.is_none() {
            "N/A"
        } else if r.use_ == "info" {
            "Info"
        } else if few && r.use_ == "required" {
            "Indicative"
        } else if ok {
            "Pass"
        } else {
            "Fail"
        };
        out.checks.push(Check {
            id,
            rule: r,
            value,
            ok: value.map(|_| ok),
            required: r.use_ == "required",
            judged,
            status: status.to_string(),
            extra,
        });
    }
    // Shares also say how many sites were needed, e.g. 'Fail (41 of 52; 45 needed)'
    fn share(out: &mut Report, few: bool, st_n: usize, id: &'static str, v: Option<f64>) {
        let nf = st_n as f64;
        let need = (rule_of(id).min.unwrap_or(f64::NAN) * nf - 1e-9).ceil();
        let got = crate::jsnum::math_round(v.unwrap_or(0.0) * nf);
        chk(
            out,
            few,
            id,
            v,
            got >= need,
            vec![
                ("got", plain_number(got)),
                ("need", plain_number(need)),
                ("of", Value::from(st_n as u64)),
            ],
        );
        let c = out.checks.last_mut().expect("just pushed");
        if c.status == "Fail" {
            c.status = format!(
                "Fail ({} of {}; {} needed)",
                number_to_js_string(got),
                st_n,
                number_to_js_string(need)
            );
        }
    }
    share(&mut out, few, st.n, "geh5", st.pass);
    share(&mut out, few, st.n, "geh75", st.pass75);
    share(&mut out, few, st.n, "geh10", st.pass10);
    chk(
        &mut out,
        few,
        "r2",
        st.r2,
        st.r2
            .is_some_and(|x| x > rule_of("r2").min.unwrap_or(f64::NAN)),
        vec![],
    );
    let sl = rule_of("slope");
    chk(
        &mut out,
        few,
        "slope",
        st.slope,
        st.slope
            .is_some_and(|x| x >= sl.min.unwrap_or(f64::NAN) && x <= sl.max.unwrap_or(f64::NAN)),
        vec![],
    );
    share(&mut out, few, st.n, "band", st.band);
    share(&mut out, few, st.n, "flow", st.flow);
    let rm = rule_of("rmse");
    chk(
        &mut out,
        few,
        "rmse",
        st.rmse,
        st.rmse.is_some_and(|x| x < rm.max.unwrap_or(f64::NAN)),
        vec![(
            "clarify",
            Value::Bool(st.rmse.is_some_and(|x| {
                x >= rm.max.unwrap_or(f64::NAN) && x <= rm.clarify.unwrap_or(f64::NAN)
            })),
        )],
    );
    let mc: Option<MatrixChange> = info
        .and_then(|i| i.extra.get("matrixChange"))
        .filter(|v| !v.is_null())
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    if let Some(mc) = mc {
        let mt = rule_of("matrixTotal");
        chk(
            &mut out,
            few,
            "matrixTotal",
            Some(mc.total_pct),
            mc.total_pct.abs() <= mt.max.unwrap_or(f64::NAN),
            vec![],
        );
        let ce = rule_of("cells");
        let cs = ce.slope.expect("cells slope");
        let cells_ok = match mc.cell_r2 {
            None => true,
            Some(r2) => {
                r2 > ce.min.unwrap_or(f64::NAN)
                    && match mc.cell_slope {
                        None => true,
                        Some(x) => x >= cs[0] && x <= cs[1],
                    }
            }
        };
        chk(
            &mut out,
            few,
            "cells",
            mc.cell_r2,
            cells_ok,
            vec![(
                "slopeValue",
                mc.cell_slope.map_or(Value::Null, plain_number),
            )],
        );
        let te = rule_of("tripEnds");
        chk(
            &mut out,
            few,
            "tripEnds",
            mc.trip_end_r2,
            mc.trip_end_r2
                .is_none_or(|x| x > te.min.unwrap_or(f64::NAN)),
            vec![("slopeValue", mc.row_slope.map_or(Value::Null, plain_number))],
        );
        if mc.total_pct.abs() > mt.max.unwrap_or(f64::NAN) {
            out.warnings.push(format!(
                "Matrix estimation changed the matrix total by {}, more than {}%. Check the trip rates and gateway counts before relying on the factors.",
                fmt::signed_pct(mc.total_pct / 100.0, 1),
                number_to_js_string(mt.max.unwrap_or(f64::NAN))
            ));
        }
        if let Some(t) = mc.trip_end_r2
            && t <= te.min.unwrap_or(f64::NAN)
        {
            out.warnings.push(format!(
                "Matrix estimation changed the trip ends a lot (R\u{b2} {}, TAG M3.1 Table 5 asks for more than {}). The prior matrix is probably wrong in places; fix it rather than let the counts force it.",
                to_fixed(t, 3),
                number_to_js_string(te.min.unwrap_or(f64::NAN))
            ));
        }
        out.notes.push("The same counts were used to estimate the matrix and to validate it. TMDG and TAG M3.1 expect validation against independent counts, so hold some counts back for an independent check.".to_string());
        out.notes.push(
            "On NZTA projects matrix estimation needs the peer reviewer's agreement (TMDG)."
                .to_string(),
        );
    }
    // Counts are base-year traffic: comparing them with a forecast is not validation
    let base_year = !(res.year > 0.0) || strict_year(res.year, &s.base_year);
    if !base_year {
        out.warnings.push(format!(
            "These results are for {}, but the counts are base-year ({}) traffic. Validate a base-year scenario; this comparison is for information only.",
            number_to_js_string(res.year),
            s.base_year.js_string()
        ));
    }
    // Statistics from a run that did not converge mix assignment noise into the fit
    if !res.converged {
        let note = if res.conv_note.is_empty() {
            "The assignment did not converge."
        } else {
            res.conv_note.as_str()
        };
        out.warnings.push(format!(
            "{note} Re-run the model to convergence before reporting these statistics."
        ));
    }
    if out.stale {
        out.warnings
            .push("The results are out of date. Run the model again before reporting.".to_string());
    }
    if few {
        out.warnings.push(format!(
            "Only {} count site{} match the network. With fewer than {} the statistics are indicative only, so no pass or fail is given.",
            st.n,
            if st.n == 1 { "" } else { "s" },
            MIN_SITES
        ));
    }
    out.notes.push("Meeting the criteria does not by itself show the model is fit for purpose (TAG M3.1 3.2.2). The model specification agreed with the peer reviewer takes precedence over these defaults.".to_string());
    let req: Vec<&Check> = out.checks.iter().filter(|c| c.required).collect();
    out.status = if few {
        "indicative"
    } else if !base_year {
        "not base year"
    } else if req.iter().all(|c| c.ok == Some(true)) {
        "pass"
    } else {
        "fail"
    }
    .to_string();
    out.pass = match out.status.as_str() {
        "pass" => Some(true),
        "fail" => Some(false),
        _ => None,
    };
    let failed: Vec<&str> = req
        .iter()
        .filter(|c| c.ok == Some(false))
        .map(|c| c.rule.label)
        .collect();
    let pc = |x: Option<f64>| fmt::pct(x.unwrap_or(f64::NAN), 1);
    out.summary = if out.status == "indicative" {
        format!(
            "Indicative only: {} count sites ({} peak, {} with GEH < 5).",
            st.n,
            period,
            pc(st.pass)
        )
    } else if out.status == "not base year" {
        format!(
            "Not assessed: the {} results are compared with base-year counts ({} peak, {} with GEH < 5).",
            number_to_js_string(res.year),
            period,
            pc(st.pass)
        )
    } else {
        format!(
            "Assessed against TMDG 2019 Category E, {} peak, {} count sites: {}.",
            period,
            st.n,
            if out.pass == Some(true) {
                "all expected tests pass".to_string()
            } else {
                format!("fails {}", failed.join(", "))
            }
        )
    };
    out
}
