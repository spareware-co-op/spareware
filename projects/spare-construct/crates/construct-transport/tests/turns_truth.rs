//! Turns truth tests: movement tables, arms, names, SIDRA and turn-count CSV and the junction diagram against the prototype.
//!
//! tools/transport-truth/gen-turns.mjs ran turns.js `Turns` on the saved test projects with the engine's recorded results, on
//! made-up junctions that reach the other branches, and on grids of inputs, and wrote expected/turns-*.json; these tests give the
//! Rust port the same inputs and compare every answer (text character for character).

mod edit_support;
mod support;

use construct_transport::edit::json::JsonValue;
use construct_transport::project::{Project, Scenario, Settings};
use construct_transport::results::Res;
use construct_transport::turns::{
    CODE_HEAD, DiagramOpts, LEG_CODES, LEG_NAMES, MV, SIDRA_HEAD, SidraOpts, TURN_CODES,
    approach_name, arms, assign_legs, classify, diagram, empty_text, junction_name, leg_code_of,
    leg_label, movement_id, movements, rank, side_name, sidra_csv, sidra_export, site_rows,
    turn_code, turn_count_csv,
};
use edit_support::{Checker, load_json, read_truth};
use serde_json::{Value, json};

fn s(v: &Value) -> String {
    v.as_str().expect("text").to_string()
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().expect("number"),
        Value::String(t) => match t.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("not a number {other}"),
        },
        other => panic!("not a number {other}"),
    }
}

#[test]
fn the_pure_functions_match_the_prototype() {
    let f = load_json("expected/turns-functions.json");
    let mut ck = Checker::default();
    for (i, r) in f["sideName"].as_array().expect("rows").iter().enumerate() {
        ck.same(
            &format!("sideName[{i}] {}", r[0]),
            &side_name(num(&r[0]), r[1].as_bool().expect("flag")).to_string(),
            &s(&r[2]),
        );
    }
    for (i, r) in f["approachName"]
        .as_array()
        .expect("rows")
        .iter()
        .enumerate()
    {
        ck.same(
            &format!("approachName[{i}] {}", r[0]),
            &approach_name_text(num(&r[0]), r[1].as_bool().expect("flag")),
            &s(&r[2]),
        );
    }
    for (i, r) in f["legCodeOf"].as_array().expect("rows").iter().enumerate() {
        ck.same(
            &format!("legCodeOf[{i}] {}", r[0]),
            &(leg_code_of(num(&r[0])) as u64),
            &r[1].as_u64().expect("code"),
        );
    }
    for (i, r) in f["classify"].as_array().expect("rows").iter().enumerate() {
        ck.same(
            &format!("classify[{i}] {} {}", r[0], r[1]),
            &classify(num(&r[0]), num(&r[1])).to_string(),
            &s(&r[2]),
        );
    }
    for (i, c) in f["assignLegs"]
        .as_array()
        .expect("cases")
        .iter()
        .enumerate()
    {
        let bs: Vec<f64> = c["bearings"]
            .as_array()
            .expect("list")
            .iter()
            .map(num)
            .collect();
        let prio: Vec<f64> = c["prio"]
            .as_array()
            .expect("list")
            .iter()
            .map(num)
            .collect();
        let got: Vec<Value> = assign_legs(&bs, &prio)
            .iter()
            .map(|l| {
                let mut o = json!({ "o": l.o, "err": l.err, "merged": l.merged });
                if let Some(k) = l.into {
                    o["into"] = json!(k);
                }
                o
            })
            .collect();
        ck.check(
            &format!("assignLegs[{i}] {}", c["bearings"]),
            &Value::Array(got),
            &c["out"],
        );
    }
    for (i, r) in f["turnCode"].as_array().expect("rows").iter().enumerate() {
        ck.same(
            &format!("turnCode[{i}]"),
            &turn_code(
                r[0].as_u64().expect("o") as usize,
                r[1].as_u64().expect("o") as usize,
            )
            .to_string(),
            &s(&r[2]),
        );
    }
    for (i, r) in f["movementId"].as_array().expect("rows").iter().enumerate() {
        ck.same(
            &format!("movementId[{i}]"),
            &movement_id(
                r[0].as_u64().expect("o") as usize,
                r[1].as_u64().expect("o") as usize,
            ),
            &s(&r[2]),
        );
    }
    for (i, r) in f["legLabel"].as_array().expect("rows").iter().enumerate() {
        ck.same(
            &format!("legLabel[{i}]"),
            &leg_label(
                r[0].as_u64().expect("o") as usize,
                r[1].as_str().expect("road"),
            ),
            &s(&r[2]),
        );
    }
    for (i, r) in f["rank"].as_array().expect("rows").iter().enumerate() {
        ck.same(
            &format!("rank[{i}]"),
            &rank(r[0].as_str().expect("mv"), r[1].as_bool().expect("major")),
            &num(&r[2]),
        );
    }
    let c = &f["constants"];
    let list = |a: &[&str]| json!(a);
    ck.check("LEG_CODES", &list(&LEG_CODES), &c["LEG_CODES"]);
    ck.check("LEG_NAMES", &list(&LEG_NAMES), &c["LEG_NAMES"]);
    ck.check("TURN_CODES", &list(&TURN_CODES), &c["TURN_CODES"]);
    ck.check("SIDRA_HEAD", &list(&SIDRA_HEAD), &c["SIDRA_HEAD"]);
    ck.check("CODE_HEAD", &list(&CODE_HEAD), &c["CODE_HEAD"]);
    ck.check("MV", &list(&MV), &json!(["L", "T", "R", "U"]));
    ck.finish("turns functions");
}

fn approach_name_text(b: f64, eight: bool) -> String {
    approach_name(b, eight).to_string()
}

/// The ordered JSON of a recorded file (object keys in the file's order).
fn ordered(rel: &str) -> JsonValue {
    JsonValue::parse(&read_truth(rel)).expect("JSON")
}

fn typed<T: serde::de::DeserializeOwned>(v: &JsonValue) -> T {
    serde_json::from_str(&v.stringify()).expect("reads as the type")
}

/// What one recorded case needs: the scenario, settings and results to give the Rust functions.
struct Inputs {
    project: Project,
    scenario: usize,
    res: Res,
}

impl Inputs {
    fn sc(&self) -> &Scenario {
        &self.project.scenarios[self.scenario]
    }
    fn s(&self) -> &Settings {
        &self.project.settings
    }
}

fn arms_json(a: &[construct_transport::turns::Arm], coded: bool) -> Value {
    Value::Array(a.iter().map(|x| x.to_value(coded)).collect())
}

fn rows_json(r: &[construct_transport::turns::MovementRow]) -> Value {
    Value::Array(r.iter().map(|x| x.to_value()).collect())
}

fn gate_json(g: &construct_transport::turns::SidraExport) -> Value {
    json!({ "ok": g.ok, "why": g.why, "csv": g.csv })
}

/// Compares everything recorded for one scenario and result.
fn check_case(ck: &mut Checker, at: &str, inp: &Inputs, rec: &Value, loose_base: &JsonValue) {
    let sc = inp.sc();
    let st = inp.s();
    let res = Some(&inp.res);
    let none: Vec<construct_transport::results::TurnRow> = Vec::new();
    for (id, n) in rec["nodes"].as_object().expect("nodes") {
        let a = format!("{at}.nodes[{id}]");
        let list = res.and_then(|r| r.turns.get(id)).unwrap_or(&none);
        if id != "missing" {
            ck.same(
                &format!("{a}.has"),
                &res.is_some_and(|r| r.turns.contains_key(id)),
                &n["has"].as_bool().expect("flag"),
            );
            ck.check(
                &format!("{a}.arms"),
                &arms_json(&arms(sc, id, list, None), false),
                &n["arms"],
            );
            ck.check(
                &format!("{a}.armsNoTurns"),
                &arms_json(&arms(sc, id, &[], None), false),
                &n["armsNoTurns"],
            );
        } else {
            ck.check(
                &format!("{a}.arms"),
                &arms_json(&arms(sc, id, &[], None), false),
                &n["arms"],
            );
        }
        ck.check(
            &format!("{a}.rows"),
            &rows_json(&movements(sc, res, st, id)),
            &n["rows"],
        );
        ck.same(
            &format!("{a}.name"),
            &junction_name(sc, id, None),
            &s(&n["name"]),
        );
        ck.same(
            &format!("{a}.empty"),
            &empty_text(Some(sc), res, id),
            &s(&n["empty"]),
        );
        ck.same(
            &format!("{a}.count"),
            &turn_count_csv(Some(sc), res, st, id),
            &s(&n["count"]),
        );
        let d = &n["diagram"];
        ck.same(
            &format!("{a}.diagram.total"),
            &diagram(Some(sc), res, st, id, &DiagramOpts::default()),
            &s(&d["total"]),
        );
        // The other layers and sizes are recorded for some junctions only
        let layer = |l: &str, size: Option<f64>, suffix: Option<&str>| DiagramOpts {
            layer: Some(l.into()),
            size,
            id_suffix: suffix.map(Into::into),
        };
        if let Some(x) = d.get("base") {
            ck.same(
                &format!("{a}.diagram.base"),
                &diagram(Some(sc), res, st, id, &layer("base", Some(300.0), None)),
                &s(x),
            );
        }
        if let Some(x) = d.get("dev") {
            ck.same(
                &format!("{a}.diagram.dev"),
                &diagram(
                    Some(sc),
                    res,
                    st,
                    id,
                    &layer("dev", Some(180.5), Some("-x 1<2>")),
                ),
                &s(x),
            );
        }
        if let Some(x) = d.get("small") {
            ck.same(
                &format!("{a}.diagram.small"),
                &diagram(
                    Some(sc),
                    res,
                    st,
                    id,
                    &DiagramOpts {
                        size: Some(60.0),
                        ..Default::default()
                    },
                ),
                &s(x),
            );
        }
    }
    for (sid, e) in rec["sites"].as_object().expect("sites") {
        let a = format!("{at}.sites[{sid}]");
        let m = site_rows(sc, res, st, sid);
        ck.check(&format!("{a}.rows"), &rows_json(&m.rows), &e["rows"]);
        ck.check(&format!("{a}.arms"), &arms_json(&m.arms, true), &e["arms"]);
        ck.check(
            &format!("{a}.movements"),
            &rows_json(&movements(sc, res, st, sid)),
            &e["movements"],
        );
    }
    // The SIDRA CSV in every form
    let c = &rec["csv"];
    let r = &inp.res;
    let one: [&Res; 1] = [r];
    let ids: Vec<String> = r.turns.keys().map(str::to_string).collect();
    let sites: Vec<String> = r
        .sites
        .as_ref()
        .map(|m| m.keys().map(str::to_string).collect())
        .unwrap_or_default();
    let mut some: Vec<String> = ids.iter().take(2).cloned().collect();
    some.extend(sites.iter().cloned());
    some.push("nope".to_string());
    let codes = SidraOpts {
        codes: true,
        ..Default::default()
    };
    let all_codes = SidraOpts {
        all: true,
        codes: true,
        ..Default::default()
    };
    // (the junctions of the random cases record only some of the forms)
    let mut text = |name: &str, got: String| {
        if let Some(want) = c.get(name) {
            ck.same(&format!("{at}.csv.{name}"), &got, &s(want))
        }
    };
    text(
        "default",
        sidra_csv(Some(sc), &one, st, None, &SidraOpts::default()),
    );
    text("codes", sidra_csv(Some(sc), &one, st, None, &codes));
    text("all", sidra_csv(Some(sc), &one, st, None, &all_codes));
    text(
        "some",
        sidra_csv(Some(sc), &one, st, Some(&some), &SidraOpts::default()),
    );
    text(
        "someCodes",
        sidra_csv(Some(sc), &one, st, Some(&some), &codes),
    );
    text(
        "empty",
        sidra_csv(Some(sc), &one, st, Some(&[]), &SidraOpts::default()),
    );
    text(
        "arrayOne",
        sidra_csv(Some(sc), &one, st, None, &SidraOpts::default()),
    );
    text(
        "nullScenario",
        sidra_csv(None, &one, st, None, &SidraOpts::default()),
    );
    text(
        "noRes",
        sidra_csv(Some(sc), &[], st, None, &SidraOpts::default()),
    );
    // A result with no turns: its key is missing in the prototype
    let mut bare: JsonValue = loose_base.clone();
    bare.set("turns", JsonValue::object());
    bare.set("period", "AM".into());
    bare.remove("sites");
    bare.remove("siteTurns");
    let bare_res: Res = typed(&bare);
    text(
        "noTurns",
        sidra_csv(Some(sc), &[&bare_res], st, None, &SidraOpts::default()),
    );
    let mut gate = |name: &str, g: construct_transport::turns::SidraExport| {
        if let Some(want) = c.get(name) {
            ck.check(&format!("{at}.csv.{name}"), &gate_json(&g), want)
        }
    };
    gate(
        "gate",
        sidra_export(Some(sc), &one, st, None, &SidraOpts::default()),
    );
    gate("gateCodes", sidra_export(Some(sc), &one, st, None, &codes));
    gate(
        "gateNone",
        sidra_export(Some(sc), &[], st, None, &SidraOpts::default()),
    );
    // The extent test's sites
    let ext = c.get("extent").is_some();
    if ext {
        let mut with_extent = sc.clone();
        let list: Vec<Value> = rec["extentSites"].as_array().expect("sites").clone();
        with_extent.extent = construct_transport::jsnum::Maybe::Set(json!({ "sites": list }));
        ck.same(
            &format!("{at}.csv.extent"),
            &sidra_csv(Some(&with_extent), &one, st, None, &SidraOpts::default()),
            &s(&c["extent"]),
        );
        if c.get("extentAll").is_some() {
            ck.same(
                &format!("{at}.csv.extentAll"),
                &sidra_csv(
                    Some(&with_extent),
                    &one,
                    st,
                    None,
                    &SidraOpts {
                        all: true,
                        ..Default::default()
                    },
                ),
                &s(&c["extentAll"]),
            );
        }
    }
    // Loose gap and out of date
    let mut loose = loose_base.clone();
    loose.set("gap", rec["looseGap"].as_f64().expect("gap").into());
    let loose: Res = typed(&loose);
    let mut stale = loose_base.clone();
    stale.set("stale", true.into());
    let stale: Res = typed(&stale);
    let force_codes = SidraOpts {
        force: true,
        codes: true,
        ..Default::default()
    };
    let force = SidraOpts {
        force: true,
        ..Default::default()
    };
    ck.check(
        &format!("{at}.csv.gateLoose"),
        &gate_json(&sidra_export(
            Some(sc),
            &[&loose],
            st,
            None,
            &SidraOpts::default(),
        )),
        &c["gateLoose"],
    );
    if let Some(want) = c.get("gateLooseForce") {
        ck.check(
            &format!("{at}.csv.gateLooseForce"),
            &gate_json(&sidra_export(Some(sc), &[&loose], st, None, &force_codes)),
            want,
        );
    }
    ck.check(
        &format!("{at}.csv.gateStale"),
        &gate_json(&sidra_export(
            Some(sc),
            &[&stale],
            st,
            None,
            &SidraOpts::default(),
        )),
        &c["gateStale"],
    );
    if let Some(want) = c.get("gateStaleForce") {
        ck.check(
            &format!("{at}.csv.gateStaleForce"),
            &gate_json(&sidra_export(Some(sc), &[&stale], st, None, &force)),
            want,
        );
    }
    let first = ids.first().map_or("x", String::as_str);
    if let Some(want) = c.get("gateStaleEmpty") {
        ck.same(
            &format!("{at}.csv.gateStaleEmpty"),
            &empty_text(Some(sc), Some(&stale), first),
            &s(want),
        );
    }
}

#[test]
fn the_saved_projects_match_the_prototype() {
    let mut ck = Checker::default();
    let mut cases_run = 0usize;
    for name in ["corridor", "grid"] {
        let project =
            Project::from_json(&read_truth(&format!("inputs/{name}.json"))).expect("a project");
        let results = ordered(&format!("expected/{name}.json"));
        let ex = load_json(&format!("expected/turns-{name}.json"));
        for (k, case) in ex["cases"].as_array().expect("cases").iter().enumerate() {
            let sid = s(&case["scenario"]);
            let period = s(&case["period"]);
            let scenario = project
                .scenarios
                .iter()
                .position(|x| x.id == sid)
                .expect("scenario");
            let at = format!("{name}.cases[{k}]({sid}, {period})");
            if period == "both" {
                let periods = results
                    .get("scenarios")
                    .and_then(|x| x.get(&sid))
                    .and_then(|x| x.get("periods"))
                    .expect("periods");
                let list: Vec<Res> = periods
                    .as_object()
                    .expect("periods")
                    .values()
                    .map(|p| typed(p.get("res").expect("res")))
                    .collect();
                let refs: Vec<&Res> = list.iter().collect();
                let sc = &project.scenarios[scenario];
                let st = &project.settings;
                let c = &case["csv"];
                ck.same(
                    &format!("{at}.both"),
                    &sidra_csv(Some(sc), &refs, st, None, &SidraOpts::default()),
                    &s(&c["both"]),
                );
                ck.same(
                    &format!("{at}.bothCodes"),
                    &sidra_csv(
                        Some(sc),
                        &refs,
                        st,
                        None,
                        &SidraOpts {
                            codes: true,
                            ..Default::default()
                        },
                    ),
                    &s(&c["bothCodes"]),
                );
                ck.check(
                    &format!("{at}.bothGate"),
                    &gate_json(&sidra_export(
                        Some(sc),
                        &refs,
                        st,
                        None,
                        &SidraOpts::default(),
                    )),
                    &c["bothGate"],
                );
                let mut second = periods
                    .as_object()
                    .expect("periods")
                    .values()
                    .nth(1)
                    .expect("second")
                    .get("res")
                    .expect("res")
                    .clone();
                second.set("gap", 0.5.into());
                let second: Res = typed(&second);
                let mixed = [&list[0], &second];
                ck.check(
                    &format!("{at}.bothLoose"),
                    &gate_json(&sidra_export(
                        Some(sc),
                        &mixed,
                        st,
                        None,
                        &SidraOpts::default(),
                    )),
                    &c["bothLoose"],
                );
                let mut third = periods
                    .as_object()
                    .expect("periods")
                    .values()
                    .nth(1)
                    .expect("second")
                    .get("res")
                    .expect("res")
                    .clone();
                third.set("gap", 0.5.into());
                third.set("period", "".into());
                let third: Res = typed(&third);
                let mixed = [&list[0], &third];
                ck.check(
                    &format!("{at}.bothForce"),
                    &gate_json(&sidra_export(
                        Some(sc),
                        &mixed,
                        st,
                        None,
                        &SidraOpts {
                            force: true,
                            ..Default::default()
                        },
                    )),
                    &c["bothForce"],
                );
                continue;
            }
            let res_json = results
                .get("scenarios")
                .and_then(|x| x.get(&sid))
                .and_then(|x| x.get("periods"))
                .and_then(|x| x.get(&period))
                .and_then(|x| x.get("res"))
                .expect("res");
            let res: Res = typed(res_json);
            let inp = Inputs {
                project: project.clone(),
                scenario,
                res,
            };
            check_case(&mut ck, &at, &inp, case, res_json);
            cases_run += 1;
        }
    }
    println!("scenario-period cases checked: {cases_run}");
    ck.finish("turns on the saved projects");
}

#[test]
fn the_made_up_junctions_match_the_prototype() {
    let mut ck = Checker::default();
    let file = ordered("expected/turns-synthetic.json");
    let ex = load_json("expected/turns-synthetic.json");
    // A full result to start from: the made-up ones set only what Turns reads
    let corridor = ordered("expected/corridor.json");
    let base = corridor
        .get("scenarios")
        .and_then(JsonValue::as_object)
        .and_then(|m| m.values().next())
        .and_then(|x| x.get("periods"))
        .and_then(JsonValue::as_object)
        .and_then(|m| m.values().next())
        .and_then(|x| x.get("res"))
        .expect("a result")
        .clone();
    let cases = file
        .get("cases")
        .and_then(JsonValue::as_array)
        .expect("cases");
    for (k, case) in cases.iter().enumerate() {
        let name = case
            .get("name")
            .and_then(JsonValue::as_str)
            .expect("name")
            .to_string();
        let project: Project = typed(case.get("project").expect("project"));
        let mut full = base.clone();
        full.remove("sites");
        full.remove("siteTurns");
        for (key, v) in case
            .get("res")
            .and_then(JsonValue::as_object)
            .expect("res")
            .iter()
        {
            full.set(key, v.clone());
        }
        let res: Res = typed(&full);
        let inp = Inputs {
            project,
            scenario: 0,
            res,
        };
        let at = format!("synthetic[{k}]({name})");
        check_case(&mut ck, &at, &inp, &ex["cases"][k], &full);
    }
    ck.finish("turns on made-up junctions");
}
