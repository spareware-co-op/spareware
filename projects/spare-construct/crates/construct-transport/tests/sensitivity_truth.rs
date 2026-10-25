//! The sensitivity pack against the JavaScript: every assignment run it makes is replayed from
//! expected/sensitivity-runs-<name>.json (gen-sensitivity-runs.mjs). The closure checks each run's
//! inputs (the scenario and settings copies the tests change, and the options) against the
//! recorded ones, in order, and returns the recorded result; the table must equal
//! expected/<name>.json "sensitivity".

mod recorded;
mod support;

use construct_transport::analysis::{SENS, SENS_DEFAULT, SensOpts, sensitivity};
use construct_transport::results::Res;
use recorded::{json, project_and_record};
use serde::Deserialize;
use serde_json::Value;
use support::{PROJECTS, assert_close, read_truth};

#[derive(Deserialize)]
struct RunsFile {
    scenario: String,
    calls: Vec<Call>,
}

#[derive(Deserialize)]
struct Call {
    label: String,
    test: Option<String>,
    which: String,
    period: String,
    sc: Value,
    settings: Value,
    opts: Value,
    res: Res,
}

#[test]
fn sensitivity_table_from_the_recorded_runs() {
    for name in PROJECTS {
        let (p, rec) = project_and_record(name);
        let text = read_truth(&format!("expected/sensitivity-runs-{name}.json"));
        let runs: RunsFile = serde_json::from_str(&text).expect("runs file");
        let sc = p.scenario(&runs.scenario).expect("scenario");
        let mut i = 0;
        let mut progress: Vec<String> = Vec::new();
        let mut on_progress =
            |k: usize, n: usize, label: &str| progress.push(format!("{k}/{n} {label}"));
        let opts = SensOpts {
            on_progress: Some(&mut on_progress),
            ..SensOpts::default()
        };
        let table = sensitivity(&p, sc, opts, |s1, s, ao| {
            let call = runs
                .calls
                .get(i)
                .unwrap_or_else(|| panic!("{name}: more runs than recorded"));
            let path = format!(
                "{name}.run[{i}] {} {} {}",
                call.label, call.period, call.which
            );
            assert_close(&json(s1), &call.sc, &format!("{path}.sc"));
            assert_close(&json(s), &call.settings, &format!("{path}.settings"));
            assert_close(&json(ao), &call.opts, &format!("{path}.opts"));
            i += 1;
            call.res.clone()
        })
        .unwrap_or_else(|e| panic!("{name}: {}", e.error));
        assert_eq!(i, runs.calls.len(), "{name}: runs made");
        // the runs come in the JavaScript's order, with its progress labels
        let want: Vec<String> = runs
            .calls
            .iter()
            .enumerate()
            .map(|(k, c)| {
                let tail = if c.which == "without" {
                    " (without)"
                } else {
                    ""
                };
                format!(
                    "{}/{} {}: {}{}",
                    k + 1,
                    runs.calls.len(),
                    c.label,
                    c.period,
                    tail
                )
            })
            .collect();
        assert_eq!(progress, want, "{name}: progress");
        let tests: Vec<Option<&str>> = runs.calls.iter().map(|c| c.test.as_deref()).collect();
        assert_eq!(tests[0], None, "{name}: the central run comes first");
        assert_close(
            &json(&table),
            &rec.sensitivity,
            &format!("{name}.sensitivity"),
        );
    }
}

#[test]
fn sens_tables_as_in_engine_js() {
    let ids: Vec<&str> = SENS.iter().map(|t| t.id).collect();
    assert_eq!(
        ids,
        [
            "T1+", "T1-", "T2", "T3+", "T3-", "T4+", "T4-", "T5", "D+", "D-", "T6"
        ]
    );
    assert_eq!(
        SENS_DEFAULT,
        ["T1+", "T1-", "T2", "T3+", "T3-", "T4+", "T4-", "T5"]
    );
}

#[test]
fn t6_needs_a_mode_share() {
    let (p, _) = project_and_record("grid");
    let sc = &p.scenarios[0];
    let opts = SensOpts {
        tests: Some(vec!["T6".to_string()]),
        ..SensOpts::default()
    };
    let r = sensitivity(&p, sc, opts, |_, _, _| {
        unreachable!("no runs without a mode share")
    });
    assert_eq!(
        json(&r.unwrap_err()),
        serde_json::json!({ "error": "T6 needs the alternative mode share (% of vehicle trips replaced)." })
    );
}
