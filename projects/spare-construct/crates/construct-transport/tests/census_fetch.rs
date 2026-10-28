//! Land use truth tests: the downloads of packs/nz/landuse-plus.js (Census, employees, schools, early learning, Unitary Plan zones) against the prototype
//! with the network stubbed (tools/transport-truth/gen-census.mjs): the same requests in the same order, the same progress messages, the same results
//! and the same words for every way a download can fail.

mod census_support;
mod edit_support;
mod support;

use census_support::{Replay, exchanges};
use construct_transport::census::{Bx, Downloads, Poly};
use construct_transport::jsnum::plain_number;
use edit_support::{Checker, load_json};
use serde_json::{Value, json};

fn plain(x: f64) -> Value {
    plain_number(x)
}

fn poly_part(p: &Poly) -> Value {
    json!({ "bb": p.bb.iter().map(|x| plain(*x)).collect::<Vec<_>>(), "nRings": p.rings.len(), "nPts": p.rings.iter().map(Vec::len).sum::<usize>() })
}

fn merge(a: Value, b: Value) -> Value {
    let mut m = a.as_object().cloned().unwrap_or_default();
    m.extend(b.as_object().cloned().unwrap_or_default());
    Value::Object(m)
}

#[test]
fn every_download_asks_what_the_prototype_asked_and_words_failures_the_same() {
    let ex = load_json("expected/census-fetch.json");
    let table = exchanges();
    let mut ck = Checker::default();
    for case in ex.as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let b = &case["bounds"];
        let bounds = Bx::new(b["s"].as_f64().unwrap(), b["w"].as_f64().unwrap(), b["n"].as_f64().unwrap(), b["e"].as_f64().unwrap());
        let mut net = Replay::new(&case["log"], &table);
        let mut dl = Downloads::default();
        let mut status: Vec<String> = Vec::new();
        let what = case["what"].as_str().expect("what");
        let run = |dl: &mut Downloads, net: &mut Replay, status: &mut Vec<String>| -> Value {
            let mut st = |t: &str| status.push(t.to_string());
            let r: Result<Value, String> = match what {
                "census" => dl.census(net, &bounds, &mut st).map(|c| {
                    json!({
                        "sa1": c.sa1.iter().map(|a| merge(json!({ "code": a.code, "v": Value::Object(a.v.clone()) }), poly_part(&a.poly))).collect::<Vec<_>>(),
                        "fields": Value::Object(c.fields.iter().map(|r| (r.key.clone(), match (&r.name, r.by) { (None, "alias") => json!({ "by": r.by }), _ => json!({ "name": r.name, "by": r.by }) })).collect()),
                        "missing": c.missing, "errors": c.errors, "partial": c.partial,
                    })
                }),
                "employees" => dl.employees(net, &bounds, &mut st).map(|e| {
                    json!({
                        "year": plain(e.year), "field": e.field,
                        "sa2": e.sa2.iter().map(|a| merge(json!({ "code": a.code, "name": a.name, "n": a.n.map(plain), "year": plain(a.year), "growth": a.growth.map(plain) }), poly_part(&a.poly))).collect::<Vec<_>>(),
                    })
                }),
                "moe-schools" | "moe-ece" => dl.moe(net, &what[4..], &bounds, &mut st).map(|l| Value::Array(l.iter().map(|m| m.to_value()).collect())),
                "aup" => dl.aup(net, &bounds, &mut st).map(|a| {
                    json!({
                        "zones": a.zones.iter().map(|z| merge(json!({ "code": plain(z.code), "name": z.name, "group": z.group, "status": z.status }), poly_part(&z.poly))).collect::<Vec<_>>(),
                        "names": Value::Object(a.names.iter().map(|(k, v)| (k.clone(), json!(v))).collect()),
                    })
                }),
                other => panic!("unknown download {other}"),
            };
            match r {
                Ok(v) => json!({ "result": v }),
                Err(e) => json!({ "error": e }),
            }
        };
        let first = run(&mut dl, &mut net, &mut status);
        ck.check(&format!("{name}: first answer"), &first, &case["first"]);
        ck.same(&format!("{name}: requests made by the first call"), &net.used(), &case["firstRequests"].as_u64().unwrap().try_into().unwrap());
        ck.check(&format!("{name}: messages of the first call"), &json!(status[..status.len().min(case["firstStatus"].as_u64().unwrap() as usize)]), &json!(case["status"].as_array().unwrap()[..case["firstStatus"].as_u64().unwrap() as usize]));
        if !case["second"].is_null() {
            let second = run(&mut dl, &mut net, &mut status);
            ck.check(&format!("{name}: second answer"), &second, &case["second"]);
        }
        ck.check(&format!("{name}: every message"), &json!(status), &case["status"]);
        ck.same(&format!("{name}: every request"), &net.done(), &true);
    }
    ck.finish("downloads");
}
