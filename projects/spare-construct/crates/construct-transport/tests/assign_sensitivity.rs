//! The sensitivity pack on the real assignment: Table SENS-1 for the first scenario of each truth-test
//! project, running `assign` for every test, must equal the table the prototype recorded
//! (expected/<name>.json "sensitivity"), and every run it makes must give the result the prototype's
//! `assign` gave for the same inputs (expected/sensitivity-runs-<name>.json). tests/sensitivity_truth.rs
//! replays the recorded runs; this makes every run itself.

mod recorded;
mod support;

use construct_transport::analysis::{SensOpts, sensitivity};
use construct_transport::assign::assign;
use recorded::{json, project_and_record};
use support::{PROJECTS, assert_close, load_json};

#[test]
fn sensitivity_table_from_the_real_assignment() {
    let mut total_runs = 0;
    for name in PROJECTS {
        let (p, rec) = project_and_record(name);
        let runs = load_json(&format!("expected/sensitivity-runs-{name}.json"));
        let calls = runs["calls"].as_array().expect("recorded runs");
        let sc = p
            .scenario(runs["scenario"].as_str().expect("scenario id"))
            .expect("scenario");
        let mut i = 0usize;
        let table = sensitivity(&p, sc, SensOpts::default(), |s1, s, ao| {
            let call = &calls[i];
            let path = format!(
                "{name}.run[{i}] {} {} {}",
                call["label"], call["period"], call["which"]
            );
            let res = assign(&p, s1, s, ao, None);
            assert_close(&json(&res), &call["res"], &format!("{path}.res"));
            i += 1;
            res
        })
        .unwrap_or_else(|e| panic!("{name}: {}", e.error));
        assert_eq!(i, calls.len(), "{name}: runs made");
        total_runs += i;
        assert_close(
            &json(&table),
            &rec.sensitivity,
            &format!("{name}.sensitivity"),
        );
    }
    assert_eq!(total_runs, 36, "runs made");
}
