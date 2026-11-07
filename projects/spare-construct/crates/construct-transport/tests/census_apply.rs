//! Land use truth tests: putting the Census, employees, schools, early learning and Unitary Plan zones in a project's zones (`apply` of
//! packs/nz/landuse-plus.js) against the prototype (tools/transport-truth/gen-census.mjs): the report and every zone afterwards.

mod census_support;
mod edit_support;
mod support;

use census_support::{data_for, ids_for};
use construct_transport::census::{ApplyOpts, Bx, apply};
use construct_transport::project::Project;
use edit_support::{Checker, load_json, read_truth};
use serde_json::{Value, json};

fn opts_of(o: &Value) -> (ApplyOpts, Option<Bx>) {
    let b = o.get("bounds").map(|b| {
        Bx::new(
            b["s"].as_f64().unwrap(),
            b["w"].as_f64().unwrap(),
            b["n"].as_f64().unwrap(),
            b["e"].as_f64().unwrap(),
        )
    });
    let rolls = o
        .get("rolls")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    (
        ApplyOpts {
            newer: o.get("newer").and_then(Value::as_str).map(str::to_string),
            create: o.get("create").and_then(Value::as_bool),
            rolls,
            replace: o.get("replace").and_then(Value::as_bool),
        },
        b,
    )
}

#[test]
fn filling_zones_from_the_downloaded_data_matches_the_prototype() {
    let ex = load_json("expected/census-apply.json");
    let ans = census_support::answers();
    let mut ck = Checker::default();
    for case in ex.as_array().expect("cases") {
        let cname = case["name"].as_str().expect("name");
        let text = read_truth(&format!(
            "fixtures/data/{}.json",
            case["project"].as_str().expect("project")
        ));
        let mut p = Project::from_json(&text).expect("the project reads");
        let mut ids = ids_for(case);
        for st in case["steps"].as_array().expect("steps") {
            let (opts, bounds) = opts_of(&st["opts"]);
            let data = data_for(&ans, st.get("data").unwrap_or(&Value::Null));
            let r = apply(&mut p, 0, &mut ids, &data, &opts, bounds.as_ref());
            let label = format!("{cname} / {}", st["name"].as_str().unwrap());
            match r {
                Ok(rep) => {
                    ck.same(&format!("{label}: no error"), &st["error"].is_null(), &true);
                    ck.check(&format!("{label}: report"), &rep.to_value(), &st["report"]);
                }
                Err(e) => {
                    ck.same(&format!("{label}: error"), &json!(e), &st["error"]);
                }
            }
            ck.check(
                &format!("{label}: zones"),
                &serde_json::to_value(&p.scenarios[0].zones).unwrap(),
                &st["zones"],
            );
        }
    }
    ck.finish("filling zones");
}
