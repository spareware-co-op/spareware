//! The pre-lodgement linter against the JavaScript's recorded answers (expected/compliance-f4-lint.json, from
//! tools/transport-truth/gen-compliance-f4.mjs).

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly. Tests
// may use native arithmetic and casts (Cargo.toml), and as in the kernel's tests a case table may run
// long and name its values with single letters.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "test code; the rules are for the crate's own code"
)]

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
    for id in ["count-date", "count-source", "peak", "hv", "peds", "base", "sens", "cumulative", "rates", "rates-old", "tdm", "unfunded"] {
        assert!(findings.contains(id), "no case produced the finding {id}");
    }
}
