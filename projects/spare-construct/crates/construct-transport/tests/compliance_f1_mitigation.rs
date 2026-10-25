//! Effects, the mitigation register, condition parameters, queue storage and the failing-site test of the trigger finder against the
//! JavaScript's recorded answers (expected/compliance-f1-mitigation.json, from tools/transport-truth/gen-compliance-f1.mjs).

mod compliance_f1_support;
mod compliance_support;
mod support;

use compliance_f1_support::{args, norm_uids, obj, ok_of, run_export, run_report};
use compliance_support::{check, variant};
use construct_transport::compliance::dates::set_clock;
use construct_transport::compliance::extra::{ctx, mitigation};
use construct_transport::compliance::jv::{Jx, V};
use std::collections::HashSet;

/// The register rows after a seeding, with the random ids normalised.
fn rows_of(p: &V, key: &str) -> V {
    let mut r = ctx::data(p).g(key).clone();
    norm_uids(&mut r);
    r
}

fn count(n: usize) -> V {
    V::Number(n as f64)
}

#[test]
fn mitigation_register() {
    set_clock(Some(1792540800000.0));
    let all = compliance_support::expected("f1-mitigation");
    let cases = all["cases"].as_array().expect("cases");
    assert!(cases.len() > 700, "{} cases", cases.len());
    let mut n = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let call = case["call"].as_str().unwrap();
        let a = args(case);
        let want = ok_of(case);
        if call == "ordinal" {
            let x = a.g("n").n();
            check(id, &V::String(mitigation::ordinal(x)), want);
            n += 1;
            continue;
        }
        let mut p = variant(case["fixture"].as_str().unwrap(), &case["sets"]);
        let got: V = match call {
            "effects" => {
                if a.has("sc") {
                    // the scenario is compared by identity with the do-minimum, so it is passed as the project's own
                    mitigation::effects(&p, Some(p.g("scenarios").ix(a.g("sc").n() as usize)))
                } else {
                    mitigation::effects(&p, None)
                }
            }
            "mitigationTable" => mitigation::mitigation_table(&p),
            "conditionTable" => mitigation::condition_table(&p),
            "mitigationCSV" => V::String(mitigation::mitigation_csv(&p)),
            "report" => run_report(case, &p),
            "export" => run_export(case, &p),
            "seedMitigation" => {
                let n1 = mitigation::seed_mitigation(&mut p);
                let rows1 = rows_of(&p, "mitigation");
                let n2 = mitigation::seed_mitigation(&mut p);
                obj(vec![("n1", count(n1)), ("rows1", rows1), ("n2", count(n2)), ("rows2", rows_of(&p, "mitigation"))])
            }
            "seedConditions" => {
                let n1 = mitigation::seed_conditions(&mut p);
                let rows1 = rows_of(&p, "conditions");
                let n2 = mitigation::seed_conditions(&mut p);
                obj(vec![("n1", count(n1)), ("rows1", rows1), ("n2", count(n2)), ("rows2", rows_of(&p, "conditions"))])
            }
            "seedBoth" => {
                let na = mitigation::seed_mitigation(&mut p);
                let nb = mitigation::seed_conditions(&mut p);
                let (mut table, mut cond) = (mitigation::mitigation_table(&p), mitigation::condition_table(&p));
                norm_uids(&mut table);
                norm_uids(&mut cond);
                obj(vec![("a", count(na)), ("b", count(nb)), ("mitigation", rows_of(&p, "mitigation")), ("conditions", rows_of(&p, "conditions")), ("table", table), ("cond", cond)])
            }
            "failsAt" => {
                let sc = p.g("scenarios").ix(a.g("sc").n() as usize);
                let per = a.g("period").s();
                let res = sc.g("res").g(&per);
                if !sc.t() || !res.t() {
                    V::Null
                } else {
                    let only: Option<HashSet<String>> = if a.g("only").is_arr() { Some(a.g("only").items().iter().map(|x| x.s()).collect()) } else { None };
                    mitigation::fails_at(sc, res, a.g("crit"), only.as_ref())
                }
            }
            "storageOf" => {
                let mut o = V::object();
                for sc in p.g("scenarios").items() {
                    for nd in sc.g("nodes").vals() {
                        for l in sc.g("links").vals() {
                            if l.g("a").strict_eq(nd.g("id")) || l.g("b").strict_eq(nd.g("id")) {
                                o.set(format!("{}|{}|{}", sc.g("id").s(), nd.g("id").s(), l.g("id").s()), mitigation::storage_of(sc, &l.g("id").s(), &nd.g("id").s()));
                            }
                        }
                    }
                }
                o.set("missing", mitigation::storage_of(p.g("scenarios").ix(0), "nope", "x"));
                o
            }
            other => panic!("{id}: unknown call {other}"),
        };
        check(id, &got, want);
        n += 1;
    }
    assert_eq!(n, cases.len());
}
