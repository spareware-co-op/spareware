//! The assignment against the JavaScript's recorded answers: for every scenario and period of both
//! truth-test projects, `assign` must give the whole result object `Engine.assign` gave (every
//! arc, junction, turn, site, class and total, the iteration count, the gaps and convergence
//! rows, the paths), with only the time stamp and run time left out of the comparison.

mod recorded;
mod support;

use construct_transport::assign::assign;
use construct_transport::results::AssignOpts;
use recorded::json;
use support::{PROJECTS, diff_json, load_expected, load_project};

#[test]
fn assign_matches_every_recorded_result() {
    let mut runs = 0;
    let mut failures: Vec<String> = Vec::new();
    for name in PROJECTS {
        let p = load_project(name);
        let expected = load_expected(name);
        for sc in &p.scenarios {
            for period in ["AM", "PM"] {
                let opts = AssignOpts {
                    period: Some(period.to_string()),
                    ..AssignOpts::default()
                };
                let res = assign(&p, sc, &p.settings, &opts, None);
                let want = &expected["scenarios"][&sc.id]["periods"][period]["res"];
                assert!(want.is_object(), "{name}.{}.{period}: no recorded result", sc.id);
                let path = format!("{name}.{}.{period}.res", sc.id);
                let (diffs, total) = diff_json(&json(&res), want, &path);
                if total > 0 {
                    let mut msg = format!("{total} difference(s) under {path}:");
                    for d in diffs.iter().take(8) {
                        msg.push_str(&format!("\n  {}\n      expected {}\n      actual   {}", d.path, d.expected, d.actual));
                    }
                    failures.push(msg);
                }
                runs += 1;
            }
        }
    }
    assert_eq!(runs, 10);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
