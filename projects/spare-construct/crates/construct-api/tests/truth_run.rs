//! `transport.run` and the results commands against the JavaScript's recorded answers (tools/transport-truth).

mod support;

use construct_api::Session;
use serde_json::{Value, json};
use support::{PROJECTS, diff_json, input_text, load_expected};

/// A session with a truth-test project open.
fn open(name: &str) -> Session {
    let mut s = Session::seeded(11, 1_790_000_000_000.0);
    s.run(
        "transport.project.open",
        &json!({ "text": input_text(name) }),
    )
    .expect("the project opens");
    s
}

fn ok(s: &mut Session, name: &str, params: Value) -> Value {
    s.run(name, &params)
        .unwrap_or_else(|e| panic!("{name} {params}: {e}"))
}

#[test]
fn run_gives_every_recorded_result() {
    let mut runs = 0;
    for name in PROJECTS {
        let mut s = open(name);
        let expected = load_expected(name);
        let out = ok(&mut s, "transport.run", json!({ "all": true }));
        assert_eq!(
            out["skipped"].as_array().map(Vec::len),
            Some(0),
            "{name}: nothing skipped"
        );
        for sc in expected["scenarios"].as_object().unwrap().keys() {
            for period in ["AM", "PM"] {
                let want = &expected["scenarios"][sc]["periods"][period]["res"];
                let got =
                    serde_json::to_value(s.result(sc, period).expect("a result is held")).unwrap();
                let (diffs, total) = diff_json(&got, want, &format!("{name}.{sc}.{period}.res"));
                assert_eq!(total, 0, "{:#?}", diffs.iter().take(5).collect::<Vec<_>>());
                runs += 1;
            }
        }
    }
    assert_eq!(runs, 10);
}
