//! DataHub and calibrate.js function by function against the JavaScript's recorded answers
//! (expected/calibrate-cases.json, from tools/transport-truth/gen-calibrate.mjs): road-name normalising, a count as a
//! peak-hour volume and its method and weight, which counts are in use (with the 200 m rule for RAMM estimates), the
//! regression, the through-origin and OLS fits, the statistics of a set of sites (band edges included), the journey-time
//! check, the matrix change, the base-year view, the matching of counts to links at several radii, the model against the
//! counts, the validation report in each of its states, and `Math.log` bit for bit.

mod calibrate_support;
mod support;

use calibrate_support::{pn, po, pv, store_res};
use construct_transport::assign::assign;
use construct_transport::calibrate::{
    GEH_BANDS, MIN_SITES, RULES, base_view, eligible, js_log, matrix_change, ols, origin, report,
    stats_of, stored_res,
};
use construct_transport::datahub::*;
use construct_transport::jsnum::{JsNum, Maybe, Num, string_to_number};
use construct_transport::project::{Project, Settings, Zone, default_settings};
use construct_transport::results::AssignOpts;
use serde_json::{Map, Value, json};
use support::{as_number, assert_close, load_json};

fn cases() -> Value {
    load_json("expected/calibrate-cases.json")
}

fn num_of(v: &Value) -> f64 {
    as_number(v).unwrap_or_else(|| panic!("not a number: {v}"))
}

#[test]
fn rules_and_constants() {
    let c = cases();
    let rules: Map<String, Value> = RULES
        .iter()
        .map(|(k, r)| (k.to_string(), serde_json::to_value(r).expect("rule")))
        .collect();
    assert_close(&Value::Object(rules), &c["rules"], "RULES");
    let mut om = Map::new();
    for m in OBS_METHODS.iter() {
        let mut o = Map::new();
        o.insert("label".into(), json!(m.label));
        o.insert("weight".into(), json!(m.weight));
        o.insert("validation".into(), json!(m.validation));
        if let Some(x) = m.override_m {
            o.insert("overrideM".into(), json!(x));
        }
        om.insert(m.key.to_string(), Value::Object(o));
    }
    assert_close(&Value::Object(om), &c["obsMethods"], "OBS_METHODS");
    let t = &JT_CRITERIA;
    assert_close(
        &json!({ "id": t.id, "name": t.name, "edition": t.edition, "verified": t.verified,
            "tiers": t.tiers.iter().map(|x| json!({ "id": x.id, "rel": x.rel, "abs": x.abs })).collect::<Vec<_>>(),
            "text": t.text, "googleLabel": t.google_label }),
        &c["jtCriteria"],
        "JT_CRITERIA",
    );
    let labels: Map<String, Value> = REG_LABELS
        .iter()
        .map(|(k, l)| (k.to_string(), json!(l)))
        .collect();
    assert_close(&Value::Object(labels), &c["regLabels"], "REG_LABELS");
    assert_eq!(MIN_SITES, 10);
    let bands: Vec<Value> = GEH_BANDS
        .iter()
        .map(|&(lo, hi)| json!([pn(lo), pn(hi)]))
        .collect();
    assert_eq!(bands.len(), 5);
}

#[test]
fn road_names_normalise_as_recorded() {
    let c = cases();
    let mut n = 0;
    for r in c["norm"].as_array().expect("norm") {
        let s = r["s"].as_str().expect("text");
        assert_eq!(norm(s), r["out"].as_str().expect("out"), "norm({s:?})");
        n += 1;
    }
    assert!(n > 40);
    // loose values: the first truthy of two, as text
    for r in c["normLoose"].as_array().expect("loose") {
        let raw = &r["raw"];
        let want = r["out"].as_str().expect("out");
        let got = match raw {
            Value::String(s) if s == "__undefined__" => norm_of(&Maybe::Missing, &Maybe::Missing),
            Value::Null => norm_of(&Maybe::Null, &Maybe::Null),
            v => norm_of(&Maybe::Set(JsNum::from(v.clone())), &Maybe::Missing),
        };
        assert_eq!(got, want, "norm({raw})");
    }
}

fn settings_with(sv: &Value) -> Settings {
    let mut s = default_settings();
    let mut d = s.data.get().cloned().expect("data settings");
    d.am_share = value_num(&sv["amShare"]);
    d.pm_share = value_num(&sv["pmShare"]);
    d.match_radius = if sv.get("matchRadius").is_some() {
        value_num(&sv["matchRadius"])
    } else {
        Maybe::Missing
    };
    s.data = Maybe::Set(d);
    s
}

#[test]
fn counts_as_period_volumes_methods_and_weights() {
    let c = cases();
    let mut n = 0;
    for rec in c["periodVol"].as_array().expect("periodVol") {
        let s = settings_with(&rec["settings"]);
        let cnt: Count = serde_json::from_value(rec["count"].clone()).expect("count");
        let id = cnt.id.clone();
        for per in ["AM", "PM"] {
            let want = &rec["periods"][per];
            let got = period_vol(&cnt, per, &s).map_or(
                Value::Null,
                |p| json!({ "v": pn(p.v), "how": p.how, "basis": p.basis }),
            );
            assert_close(
                &got,
                &want["pv"],
                &format!("periodVol({id}, {per}) with {}", rec["settings"]),
            );
            assert_eq!(
                obs_method(&cnt, per, &s),
                want["method"].as_str().expect("method"),
                "obsMethod({id}, {per})"
            );
            assert_close(
                &pn(count_weight(&cnt, per, &s)),
                &want["weight"],
                &format!("countWeight({id}, {per})"),
            );
            n += 1;
        }
        assert_eq!(
            is_estimate(&cnt),
            rec["isEstimate"].as_bool().expect("isEstimate"),
            "isEstimate({id})"
        );
        assert_eq!(
            validation_ok(&cnt),
            rec["validationOK"].as_bool().expect("validationOK"),
            "validationOK({id})"
        );
    }
    assert!(n > 400, "{n}");
}

#[test]
fn counts_in_use_and_shadowed() {
    let c = cases();
    for rec in c["inUse"].as_array().expect("inUse") {
        let mut p: Project = construct_transport::project::new_project("t", "t", "s");
        let mut extra = Map::new();
        extra.insert("counts".into(), rec["counts"].clone());
        if let Some(u) = rec["use"].as_object() {
            extra.insert("use".into(), Value::Object(u.clone()));
        }
        p.data = Maybe::Set(construct_transport::project::ProjectData {
            loaded: Maybe::Missing,
            msm: Maybe::Missing,
            controlled: Maybe::Missing,
            extra,
        });
        let d = Data::read(&p);
        assert_eq!(d.counts.len(), 19);
        for cnt in &d.counts {
            let want = rec["inUse"][&cnt.id].as_bool().expect("inUse");
            assert_eq!(d.in_use(cnt), want, "inUse({}) with {}", cnt.id, rec["use"]);
            let want = rec["shadowed"][&cnt.id].as_bool().expect("shadowed");
            assert_eq!(
                d.shadowed(cnt),
                want,
                "shadowed({}) with {}",
                cnt.id,
                rec["use"]
            );
        }
    }
}

fn xy(v: &Value) -> (Vec<f64>, Vec<f64>) {
    let f = |k: &str| {
        v[k].as_array()
            .expect(k)
            .iter()
            .map(num_of)
            .collect::<Vec<f64>>()
    };
    (f("x"), f("y"))
}

#[test]
fn regressions_and_fits() {
    let c = cases();
    for (k, rec) in c["regression"]
        .as_array()
        .expect("regression")
        .iter()
        .enumerate()
    {
        let (x, y) = xy(rec);
        let r = regression(&x, &y);
        let got = json!({ "n": r.n, "b0": po(r.b0), "r2OriginCentred": po(r.r2_origin_centred), "r2OriginExcel": po(r.r2_origin_excel),
            "olsA": po(r.ols_a), "olsB": po(r.ols_b), "r2Ols": po(r.r2_ols) });
        assert_close(&got, &rec["out"], &format!("regression[{k}]"));
    }
    for (k, rec) in c["origin"].as_array().expect("origin").iter().enumerate() {
        let (x, y) = xy(rec);
        assert_close(
            &serde_json::to_value(origin(&x, &y)).expect("origin"),
            &rec["origin"],
            &format!("origin[{k}]"),
        );
        assert_close(
            &serde_json::to_value(ols(&x, &y)).expect("ols"),
            &rec["ols"],
            &format!("ols[{k}]"),
        );
    }
    for (k, rec) in c["stats"].as_array().expect("stats").iter().enumerate() {
        let rows: Vec<(f64, f64, f64)> = rec["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|r| {
                let (o, m) = (num_of(&r[0]), num_of(&r[1]));
                (o, m, geh(m, o))
            })
            .collect();
        assert_close(
            &serde_json::to_value(stats_of(&rows)).expect("stats"),
            &rec["out"],
            &format!("stats[{k}]"),
        );
    }
}

#[test]
fn journey_time_checks() {
    let c = cases();
    for (k, rec) in c["jtCheck"].as_array().expect("jtCheck").iter().enumerate() {
        let got = jt_check(num_of(&rec["model"]), num_of(&rec["obs"]));
        let got = got.map_or(Value::Null, |x| serde_json::to_value(x).expect("check"));
        assert_close(&got, &rec["out"], &format!("jtCheck[{k}]"));
    }
}

#[test]
fn matrix_changes() {
    let c = cases();
    for (k, rec) in c["matrixChange"]
        .as_array()
        .expect("matrixChange")
        .iter()
        .enumerate()
    {
        let z = rec["Z"].as_u64().expect("Z") as usize;
        let f = |key: &str| {
            rec[key]
                .as_array()
                .expect(key)
                .iter()
                .map(num_of)
                .collect::<Vec<f64>>()
        };
        let mc = matrix_change(z, &f("prior"), &f("est"));
        assert_close(
            &serde_json::to_value(mc).expect("mc"),
            &rec["out"],
            &format!("matrixChange[{k}]"),
        );
    }
}

#[test]
fn eligible_zones() {
    let c = cases();
    for rec in c["eligible"].as_array().expect("eligible") {
        let z = &rec["z"];
        let want = rec["out"].as_bool().expect("out");
        let got = match z {
            Value::Null => eligible(None),
            Value::String(s) if s == "__undefined__" => eligible(None),
            v => {
                let mut o = v.as_object().expect("zone").clone();
                o.insert("id".into(), json!("z"));
                let zone: Zone = serde_json::from_value(Value::Object(o)).expect("zone");
                eligible(Some(&zone))
            }
        };
        assert_eq!(got, want, "eligible({z})");
    }
}

#[test]
fn math_log_bit_for_bit() {
    let c = cases();
    let list = c["log"].as_array().expect("log");
    assert!(list.len() > 2000);
    let mut bad = Vec::new();
    for p in list {
        let x = num_of(&p[0]);
        let want = num_of(&p[1]);
        let got = js_log(x);
        let same = (got.is_nan() && want.is_nan()) || got.to_bits() == want.to_bits();
        if !same {
            bad.push(format!(
                "log({x:e}): got {got:e} ({:#x}), want {want:e} ({:#x})",
                got.to_bits(),
                want.to_bits()
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} differ, first: {:?}",
        bad.len(),
        &bad[..bad.len().min(5)]
    );
}

fn match_out(m: &Matches) -> Value {
    let mut o = Map::new();
    for (id, x) in m.iter() {
        o.insert(id.to_string(), json!({ "link": x.link, "dir": x.dir, "d": pn(x.d), "score": pn(x.score), "named": x.named }));
    }
    Value::Object(o)
}

fn cal_out(c: Option<CalibrationSummary>) -> Value {
    let Some(c) = c else { return Value::Null };
    let mut m = Map::new();
    m.insert("n".into(), json!(c.n));
    m.insert(
        "rows".into(),
        Value::Array(
            c.rows
                .iter()
                .map(|r| json!({ "id": r.c.id, "link": r.mt.link, "dir": r.mt.dir, "obs": pn(r.obs), "model": pn(r.model), "geh": pn(r.geh), "how": r.how }))
                .collect(),
        ),
    );
    if let Some(s) = &c.stats {
        if let Value::Object(x) = serde_json::to_value(s).expect("stats") {
            for (k, v) in x {
                m.insert(k, v);
            }
        }
        m.insert("slope".into(), po(s.b0));
        m.insert("r2Ols".into(), po(s.r2));
        let labels: Map<String, Value> = REG_LABELS
            .iter()
            .map(|(k, l)| (k.to_string(), json!(l)))
            .collect();
        m.insert("labels".into(), Value::Object(labels));
    }
    Value::Object(m)
}

fn with_counts(p: &Project, counts: Vec<Value>) -> Project {
    let mut q = p.clone();
    if let Some(d) = q.data.get_mut() {
        d.extra.insert("counts".into(), Value::Array(counts));
    }
    q
}

#[test]
fn matches_calibration_base_views_and_reports() {
    let c = cases();
    let mut n_reports = 0;
    for rec in c["project"].as_array().expect("project") {
        let name = rec["fixture"].as_str().expect("fixture");
        let p = Project::from_json(rec["projectText"].as_str().expect("text")).expect("project");
        let sc = &p.scenarios[0];
        let data = Data::read(&p);
        assert_close(
            &match_out(&matches(sc, &p.settings, &data)),
            &rec["matches"],
            &format!("{name}.matches"),
        );
        for (key, r) in [("matchesR15", 15.0), ("matchesR120", 120.0)] {
            let mut s = p.settings.clone();
            let mut d = s.data.get().cloned().expect("data");
            d.match_radius = Num::number(r);
            s.data = Maybe::Set(d);
            assert_close(
                &match_out(&matches(sc, &s, &data)),
                &rec[key],
                &format!("{name}.{key}"),
            );
        }
        {
            let mut q = p.clone();
            let sc2 = &mut q.scenarios[0];
            let ids: Vec<String> = sc2.links.keys().map(String::from).collect();
            sc2.links.get_mut(&ids[1]).expect("link").status = Maybe::Set("closed".into());
            sc2.links.get_mut(&ids[2]).expect("link").lanes_ba = Num::number(0.0);
            sc2.links.get_mut(&ids[3]).expect("link").cls = Maybe::Set("ramp".into());
            assert_close(
                &match_out(&matches(&q.scenarios[0], &q.settings, &data)),
                &rec["matchesEdited"],
                &format!("{name}.matchesEdited"),
            );
        }
        // the model against the counts
        let mut q = p.clone();
        for period in ["AM", "PM"] {
            let res = assign(
                &q,
                &q.scenarios[0],
                &q.settings,
                &AssignOpts {
                    period: Some(period.into()),
                    ..AssignOpts::default()
                },
                None,
            );
            store_res(&mut q.scenarios[0], period, &res);
            let cs = calibration(&q.scenarios[0], Some(&res), period, &q.settings, &data);
            assert_close(
                &cal_out(cs),
                &rec["calibration"][period],
                &format!("{name}.calibration.{period}"),
            );
        }
        // base views
        for bv in rec["baseViews"].as_array().expect("baseViews") {
            let mut q = p.clone();
            let bn = bv["name"].as_str().expect("name");
            {
                let sc = &mut q.scenarios[0];
                match bn {
                    "proposed-link" => {
                        let id = sc.links.keys().next().expect("link").to_string();
                        sc.links.get_mut(&id).expect("link").status = Maybe::Set("proposed".into());
                    }
                    "late-zone" | "zone-year-ok" => {
                        let id = sc
                            .zones
                            .values()
                            .find(|z| !z.kind.is("gateway"))
                            .expect("zone")
                            .id
                            .clone();
                        let z = sc.zones.get_mut(&id).expect("zone");
                        z.year = Num::number(if bn == "late-zone" { 2030.0 } else { 2020.0 });
                        z.existing = Num::from(true);
                    }
                    "scenario-year" => sc.year = Num::number(2026.0),
                    "scenario-year-text" => {
                        sc.year = Num::number(2031.0);
                        sc.name = Maybe::Null;
                    }
                    _ => {}
                }
            }
            let v = base_view(&q.scenarios[0], &q.settings);
            let got = json!({ "same": v.same, "id": v.view.id, "name": v.view.name.as_str().map_or(Value::Null, |s| json!(s)),
                "year": if v.view.year.is_set() { pn(v.view.year.num()) } else { Value::Null },
                "zones": v.view.zones.keys().collect::<Vec<_>>(), "links": v.view.links.keys().collect::<Vec<_>>(),
                "resKeys": match v.view.res.get() { Some(Value::Object(m)) => m.keys().cloned().collect::<Vec<String>>(), _ => vec![] } });
            let mut want = bv["out"].clone();
            // the scenario's own name text is kept as text; a null name reads as an empty one
            if want["name"].is_null() {
                want["name"] = json!(" (base year)");
            }
            let mut got = got;
            if got["name"].is_null() {
                got["name"] = json!(" (base year)");
            }
            assert_close(&got, &want, &format!("{name}.baseView.{bn}"));
        }
        // reports in each state
        let none = report(&p, &p.scenarios[0], "AM", None);
        assert_close(
            &serde_json::to_value(&none).expect("report"),
            &rec["reportNoRes"],
            &format!("{name}.reportNoRes"),
        );
        let mut r = assign(
            &p,
            &p.scenarios[0],
            &p.settings,
            &AssignOpts {
                period: Some("AM".into()),
                ..AssignOpts::default()
            },
            None,
        );
        let run = report(&p, &p.scenarios[0], "AM", Some(&r));
        assert_close(
            &serde_json::to_value(&run).expect("report"),
            &rec["reportRun"],
            &format!("{name}.reportRun"),
        );
        let user_counts: Vec<Value> = match &p.data.get().expect("data").extra["counts"] {
            Value::Array(a) => a.iter().filter(|x| x["src"] == "user").cloned().collect(),
            _ => vec![],
        };
        let few = with_counts(&p, user_counts.iter().take(5).cloned().collect());
        let rf = report(&few, &few.scenarios[0], "AM", Some(&r));
        assert_close(
            &serde_json::to_value(&rf).expect("report"),
            &rec["reportFew"],
            &format!("{name}.reportFew"),
        );
        r.stale = true;
        r.converged = false;
        r.conv_note = "Stopped at 20 iterations (gap 0.5%).".to_string();
        let rs = report(&few, &few.scenarios[0], "AM", Some(&r));
        assert_close(
            &serde_json::to_value(&rs).expect("report"),
            &rec["reportStale"],
            &format!("{name}.reportStale"),
        );
        let one = with_counts(&p, user_counts.iter().take(5).take(1).cloned().collect());
        let ro = report(&one, &one.scenarios[0], "AM", Some(&r));
        assert_close(
            &serde_json::to_value(&ro).expect("report"),
            &rec["reportOne"],
            &format!("{name}.reportOne"),
        );
        r.stale = false;
        r.converged = true;
        r.conv_note = String::new();
        let mut tb = one.clone();
        tb.settings.base_year = Maybe::Set(JsNum::Text("2026".into()));
        let rt = report(&tb, &tb.scenarios[0], "AM", Some(&r));
        assert_close(
            &serde_json::to_value(&rt).expect("report"),
            &rec["reportTextBase"],
            &format!("{name}.reportTextBase"),
        );
        n_reports += 6;
        let _ = (
            stored_res(&p.scenarios[0], "AM"),
            pv(&[]),
            string_to_number("1"),
        );
    }
    assert_eq!(n_reports, 18);
}

#[test]
fn geh_bands_and_the_count_needed() {
    let c = cases();
    let geh_rows = c["geh"].as_array().expect("geh");
    let band_rows = c["bands"].as_array().expect("bands");
    assert_eq!(geh_rows.len(), 357);
    assert_eq!(band_rows.len(), 357);
    for (g, b) in geh_rows.iter().zip(band_rows) {
        let (m, obs) = (num_of(&g[0]), num_of(&g[1]));
        assert_close(
            &json!(construct_transport::datahub::geh(m, obs)),
            &g[2],
            &format!("geh({m}, {obs})"),
        );
        assert_eq!(
            construct_transport::calibrate::tmdg_band(m, obs),
            b[2].as_bool().expect("bool"),
            "tmdgBand({m}, {obs})"
        );
        assert_eq!(
            construct_transport::calibrate::tag_band(m, obs),
            b[3].as_bool().expect("bool"),
            "tagBand({m}, {obs})"
        );
    }
    for g in c["gehOdd"].as_array().expect("gehOdd") {
        let (m, obs) = (num_of(&g[0]), num_of(&g[1]));
        assert_close(
            &json!(pn(construct_transport::datahub::geh(m, obs))),
            &g[2],
            &format!("geh({m}, {obs})"),
        );
        assert_eq!(
            construct_transport::calibrate::tmdg_band(m, obs),
            g[3].as_bool().expect("bool"),
            "tmdgBand({m}, {obs})"
        );
        assert_eq!(
            construct_transport::calibrate::tag_band(m, obs),
            g[4].as_bool().expect("bool"),
            "tagBand({m}, {obs})"
        );
    }
    for n in c["need"].as_array().expect("need") {
        let got = construct_transport::calibrate::need(num_of(&n[0]), num_of(&n[1]) as usize);
        assert_eq!(got as f64, num_of(&n[2]), "need({}, {})", n[0], n[1]);
    }
}
