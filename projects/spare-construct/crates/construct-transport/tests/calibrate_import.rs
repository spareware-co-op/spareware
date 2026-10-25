//! Your own counts from a CSV file (`DataHub.importCSV`) against the JavaScript's recorded answers
//! (expected/calibrate-import.json, from tools/transport-truth/gen-calibrate.mjs): column names in any case and with
//! aliases, count directions read from text, rows with bad coordinates, volumes that are blank, zero or text, quoted
//! cells, short rows, a byte-order mark, the errors, and replacing the project's earlier own counts while keeping the
//! counts of other sources. Run on a project with counts of every source and on a bare project with no data.

mod calibrate_support;
mod support;

use construct_transport::datahub::{dir_of, import_csv};
use construct_transport::edit::IdSource;
use construct_transport::project::Project;
use serde_json::{Map, Value, json};
use support::{assert_close, load_json};

/// The generator's stand-in for `U.uid`: prefix + "x" + a counter.
struct Ids {
    n: usize,
    made: Vec<String>,
}

impl IdSource for Ids {
    fn uid(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}x{}", self.n);
        self.n += 1;
        self.made.push(id.clone());
        id
    }
    fn now_ms(&self) -> f64 {
        1_792_540_800_000.0
    }
}

/// What the generator records of the project's data after an import.
fn data_out(p: &Project) -> Value {
    let Some(d) = p.data.get() else { return Value::Null };
    let counts = d.extra.get("counts").and_then(Value::as_array).cloned().unwrap_or_default();
    let user: Vec<Value> = counts.iter().filter(|c| c["src"] == "user").cloned().collect();
    let others: Vec<Value> = counts.iter().filter(|c| c["src"] != "user").map(|c| c["id"].clone()).collect();
    let loaded: Map<String, Value> = d.loaded.get().map(|l| l.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()).unwrap_or_default();
    json!({ "counts": user, "others": others, "crashes": d.extra.get("crashes"), "loaded": loaded, "jt": d.extra.get("jt"), "use": d.extra.get("use") })
}

#[test]
fn importing_counts() {
    let all = load_json("expected/calibrate-import.json");
    let cases = all["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 44);
    let (mut imported, mut errors) = (0, 0);
    for c in cases {
        let (kind, name) = (c["kind"].as_str().expect("kind"), c["name"].as_str().expect("name"));
        let label = format!("{kind}: {name}");
        let mut p = Project::from_json(all[kind].as_str().expect("project text")).expect("project");
        let mut ids = Ids { n: 0, made: Vec::new() };
        let r = import_csv(&mut p, c["text"].as_str().expect("text"), &mut ids);
        match (&r, c["error"].as_str()) {
            (Ok(n), None) => {
                assert_eq!(*n as u64, c["n"].as_u64().expect("n"), "{label}: how many");
                imported += 1;
            }
            (Err(e), Some(want)) => {
                assert_eq!(e, want, "{label}: the error");
                errors += 1;
            }
            (r, want) => panic!("{label}: got {r:?}, expected error {want:?}"),
        }
        assert_close(&json!(ids.made), &c["ids"], &format!("{label}: ids"));
        let got = data_out(&p);
        // a project that had no data and failed to read keeps none; the recorded `use` of a project made by the prototype is complete either way
        assert_close(&got, &c["data"], &format!("{label}: data"));
    }
    assert_eq!((imported, errors), (34, 10));
}

#[test]
fn count_directions() {
    let all = load_json("expected/calibrate-import.json");
    let c = all["cases"].as_array().expect("cases").iter().find(|c| c["name"] == "direction words" && c["kind"] == "bare").expect("case");
    let words = ["NB", "northbound", "eb sb", "XNB", "nb.", "a_nb", "WB-1", "Eastbound", "(SB)", "sbnb", "NB/SB", " wb ", "", "N B"];
    for (w, count) in words.iter().zip(c["data"]["counts"].as_array().expect("counts")) {
        assert_eq!(dir_of(w).map_or(Value::Null, |d| json!(d)), count["dir"], "dirOf({w:?})");
    }
}
