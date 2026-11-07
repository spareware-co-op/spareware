//! The pre-lodgement linter against the JavaScript's recorded answers (expected/compliance-f4-lint.json, from
//! tools/transport-truth/gen-compliance-f4.mjs).

mod compliance_f4_support;
mod compliance_support;
mod support;

use compliance_f4_support::{check_ok, load, project, set_clock};
use construct_transport::compliance;
use construct_transport::compliance::jv::Jx;

#[test]
fn lint_matches_the_prototype() {
    set_clock();
    let all = load("f4-lint");
    let cases = all.g("cases").items();
    assert!(cases.len() > 250, "{} cases", cases.len());
    let mut findings = std::collections::BTreeSet::new();
    for case in cases {
        let p = project(case);
        let got = compliance::lint(&p);
        for f in got.items() {
            findings.insert(f.g("id").s());
        }
        check_ok(case, &got);
    }
    // every finding the linter can make was made by some case
    for id in [
        "count-date",
        "count-source",
        "peak",
        "hv",
        "peds",
        "base",
        "sens",
        "cumulative",
        "rates",
        "rates-old",
        "tdm",
        "unfunded",
    ] {
        assert!(findings.contains(id), "no case produced the finding {id}");
    }
}
