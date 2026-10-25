//! The `construct` command line: spawns the built binary and checks one command, writing the project back, batch mode, the command
//! list, the options and the exit codes.

use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

fn truth(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tools/transport-truth")
        .join(rel)
}

/// A copy of a truth-test project in a fresh temporary file.
fn temp_project(name: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "construct-cli-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.corridor.json"));
    std::fs::copy(truth(&format!("inputs/{name}.json")), &p).unwrap();
    p
}

struct Out {
    code: i32,
    out: String,
    err: String,
}

fn construct(args: &[&str], stdin: Option<&str>) -> Out {
    let mut c = Command::new(env!("CARGO_BIN_EXE_construct"));
    c.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = c.spawn().expect("the binary starts");
    if let Some(t) = stdin {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(t.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    let o = child.wait_with_output().unwrap();
    Out {
        code: o.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&o.stdout).into_owned(),
        err: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn json_of(o: &Out) -> Value {
    assert_eq!(o.code, 0, "stderr: {}", o.err);
    serde_json::from_str(o.out.trim()).unwrap_or_else(|e| panic!("not JSON ({e}): {}", o.out))
}

#[test]
fn one_command_prints_its_json_result() {
    let p = temp_project("grid");
    let v = json_of(&construct(
        &["transport", "project.info", "--project", path(&p)],
        None,
    ));
    assert_eq!(v["name"], json!("Truth test: grid"));
    assert_eq!(v["scenarios"].as_array().unwrap().len(), 3);
    // the three ways to write a command name
    for name in [
        "transport.project.info",
        "project.info",
        "transport_project_info",
    ] {
        assert_eq!(
            json_of(&construct(
                &["transport", name, "--project", path(&p)],
                None
            ))["name"],
            json!("Truth test: grid")
        );
    }
    // without a project a new empty one is used
    assert_eq!(
        json_of(&construct(&["transport", "network.summary"], None))["nodes"],
        json!(0)
    );
    // a read-only command leaves the file alone
    let before = std::fs::read(&p).unwrap();
    construct(
        &["transport", "network.summary", "--project", path(&p)],
        None,
    );
    assert_eq!(std::fs::read(&p).unwrap(), before);
}

#[test]
fn parameters_come_from_params_param_and_files() {
    let p = temp_project("grid");
    let a = json_of(&construct(
        &[
            "transport",
            "network.list",
            "--project",
            path(&p),
            "--params",
            r#"{"kind":"node","limit":2}"#,
        ],
        None,
    ));
    assert_eq!(a["count"], json!(2));
    let b = json_of(&construct(
        &[
            "transport",
            "network.list",
            "--project",
            path(&p),
            "--param",
            "kind=node",
            "--param",
            "limit=3",
        ],
        None,
    ));
    assert_eq!(b["count"], json!(3));
    // text stays text where the schema says text, even when it looks like a number
    let n = json_of(&construct(
        &["transport", "project.rename", "--param", "name=2026"],
        None,
    ));
    assert_eq!(n["name"], json!("2026"));
    // a file, and standard input
    let f = p.with_extension("params.json");
    std::fs::write(&f, r#"{"kind":"link","limit":4}"#).unwrap();
    let at_file = format!("@{}", path(&f));
    assert_eq!(
        json_of(&construct(
            &[
                "transport",
                "network.list",
                "--project",
                path(&p),
                "--params",
                &at_file
            ],
            None
        ))["count"],
        json!(4)
    );
    assert_eq!(
        json_of(&construct(
            &[
                "transport",
                "network.list",
                "--project",
                path(&p),
                "--params",
                "-"
            ],
            Some(r#"{"kind":"zone"}"#)
        ))["count"],
        json!(6)
    );
    let answer = format!(
        "answer=@{}",
        path(&truth("fixtures/overpass-papakura.json"))
    );
    let imported = json_of(&construct(
        &["transport", "import.osm_roads", "--param", &answer],
        None,
    ));
    assert!(imported["segments"].as_u64().unwrap() > 10);
}

#[test]
fn a_changed_project_is_written_back_or_to_out() {
    let p = temp_project("grid");
    let out = p.with_extension("copy.json");
    let v = json_of(&construct(
        &[
            "transport",
            "network.add_node",
            "--project",
            path(&p),
            "--param",
            "point=[-36.9,174.8]",
            "--out",
            path(&out),
        ],
        None,
    ));
    assert!(v["id"].as_str().unwrap().starts_with('n'));
    // --out wrote the new project; the original is as it was
    assert_eq!(
        std::fs::read(&p).unwrap(),
        std::fs::read(truth("inputs/grid.json")).unwrap()
    );
    assert_eq!(
        json_of(&construct(
            &["transport", "network.summary", "--project", path(&out)],
            None
        ))["nodes"],
        json!(21)
    );
    // without --out the project is written back
    construct(
        &[
            "transport",
            "network.add_node",
            "--project",
            path(&p),
            "--param",
            "point=[-36.9,174.8]",
        ],
        None,
    );
    assert_eq!(
        json_of(&construct(
            &["transport", "network.summary", "--project", path(&p)],
            None
        ))["nodes"],
        json!(21)
    );
    // a run is saved with its results, so a later command reads them
    let run = json_of(&construct(
        &[
            "transport",
            "run",
            "--project",
            path(&p),
            "--param",
            "all=true",
        ],
        None,
    ));
    assert_eq!(run["runs"].as_array().unwrap().len(), 6);
    let list = json_of(&construct(
        &["transport", "results.list", "--project", path(&p)],
        None,
    ));
    assert_eq!(list["results"].as_array().unwrap().len(), 6);
    let sum = json_of(&construct(
        &[
            "transport",
            "results.summary",
            "--project",
            path(&p),
            "--param",
            "scenario=s-opt",
            "--param",
            "period=AM",
        ],
        None,
    ));
    assert!(sum["kpi"]["trips"].as_f64().unwrap() > 0.0);
}

#[test]
fn the_model_run_from_the_command_line_equals_the_prototypes() {
    let p = temp_project("corridor");
    let recorded: Value =
        serde_json::from_str(&std::fs::read_to_string(truth("expected/corridor.json")).unwrap())
            .unwrap();
    let first =
        serde_json::from_str::<Value>(&std::fs::read_to_string(&p).unwrap()).unwrap()["scenarios"]
            [0]["id"]
            .as_str()
            .unwrap()
            .to_string();
    let run = json_of(&construct(
        &[
            "transport",
            "run",
            "--project",
            path(&p),
            "--param",
            &format!("scenario={first}"),
            "--param",
            r#"periods=["AM"]"#,
        ],
        None,
    ));
    let want = &recorded["scenarios"][&first]["periods"]["AM"]["res"]["kpi"];
    let got = &run["runs"][0]["kpi"];
    for k in ["trips", "vht"] {
        let (a, b) = (got[k].as_f64().unwrap(), want[k].as_f64().unwrap());
        assert!(
            (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0),
            "{k}: {a} against {b}"
        );
    }
}

#[test]
fn raw_pretty_and_progress() {
    let p = temp_project("grid");
    let o = construct(
        &[
            "transport",
            "export.links_csv",
            "--project",
            path(&p),
            "--raw",
        ],
        None,
    );
    assert_eq!(o.code, 0);
    assert!(
        o.out.starts_with("link_id,name,class"),
        "{}",
        &o.out[..60.min(o.out.len())]
    );
    let o = construct(
        &["transport", "project.save", "--project", path(&p), "--raw"],
        None,
    );
    assert_eq!(
        serde_json::from_str::<Value>(&o.out).unwrap(),
        serde_json::from_str::<Value>(&std::fs::read_to_string(&p).unwrap()).unwrap()
    );
    let o = construct(
        &[
            "transport",
            "network.summary",
            "--project",
            path(&p),
            "--pretty",
        ],
        None,
    );
    assert!(o.out.lines().count() > 5);
    let o = construct(
        &[
            "transport",
            "run",
            "--project",
            path(&p),
            "--param",
            "scenario=s-dm",
            "--param",
            r#"periods=["AM"]"#,
            "--progress",
        ],
        None,
    );
    assert_eq!(o.code, 0);
    let events: Vec<Value> = o
        .err
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(
        events.iter().any(|e| e["event"] == "run_start")
            && events.iter().any(|e| e["event"] == "progress")
    );
    assert!(events.iter().any(|e| e["event"] == "run_done"));
}

#[test]
fn errors_and_exit_codes() {
    let o = construct(
        &[
            "transport",
            "network.get",
            "--param",
            "kind=node",
            "--param",
            "id=nope",
        ],
        None,
    );
    assert_eq!(o.code, 1);
    let e: Value = serde_json::from_str(o.err.trim()).unwrap();
    assert_eq!(e["error"]["code"], json!("not_found"));
    assert!(
        o.out.is_empty(),
        "nothing on standard output for a failed command"
    );
    let o = construct(&["transport", "no.such.command"], None);
    assert_eq!(o.code, 2);
    assert!(o.err.contains("no command called"), "{}", o.err);
    let o = construct(&["transport", "network.list", "--params", "[1]"], None);
    assert_eq!(o.code, 2);
    let o = construct(
        &[
            "transport",
            "network.list",
            "--project",
            "/no/such/file.json",
        ],
        None,
    );
    assert_eq!(o.code, 1);
    assert!(o.err.contains("cannot read"), "{}", o.err);
    let o = construct(&["transport", "network.summary", "--bogus"], None);
    assert_eq!(o.code, 2);
    let o = construct(&["--help"], None);
    assert_eq!(o.code, 0);
    assert!(o.out.contains("USAGE") && o.out.contains("construct transport batch"));
    let o = construct(&["--version"], None);
    assert!(o.out.starts_with("construct "));
    assert_eq!(construct(&[], None).code, 2);
    // an unknown parameter is named
    let o = construct(
        &["transport", "network.summary", "--param", "colour=red"],
        None,
    );
    assert_eq!(o.code, 1);
    assert!(o.err.contains("unknown parameter"), "{}", o.err);
}

#[test]
fn the_command_list_has_every_command_and_its_schema() {
    let o = construct(&["transport", "commands", "--names"], None);
    let names: Vec<&str> = o.out.lines().collect();
    assert!(names.len() >= 135, "{} commands", names.len());
    assert!(names.contains(&"transport.run") && names.contains(&"transport.network.add_link"));
    let list = json_of(&construct(&["transport", "commands"], None));
    let run = list
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == json!("transport.run"))
        .unwrap();
    assert_eq!(run["params"]["type"], json!("object"));
    assert!(run["params"]["properties"]["periods"].is_object());
    assert!(
        run["description"]
            .as_str()
            .unwrap()
            .contains("Run the model")
    );
}

#[test]
fn batch_runs_json_lines_and_writes_the_project_once() {
    let p = temp_project("grid");
    let input = [
        r#"{"id":1,"command":"network.summary"}"#,
        r#"{"id":"add","command":"transport.network.add_node","params":{"point":[-36.9,174.8]}}"#,
        "",
        r#"{"id":3,"command":"network.get","params":{"kind":"node","id":"nope"}}"#,
        r#"not json"#,
        r#"{"id":5,"command":"network.summary"}"#,
    ]
    .join("\n");
    let o = construct(&["transport", "batch", "--project", path(&p)], Some(&input));
    assert_eq!(o.code, 1, "a failed line makes the exit code 1");
    let lines: Vec<Value> = o
        .out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(
        (
            lines[0]["id"].clone(),
            lines[0]["ok"].clone(),
            lines[0]["result"]["nodes"].clone()
        ),
        (json!(1), json!(true), json!(20))
    );
    assert_eq!(lines[1]["id"], json!("add"));
    assert_eq!(lines[2]["ok"], json!(false));
    assert_eq!(lines[2]["error"]["code"], json!("not_found"));
    assert_eq!(lines[3]["error"]["code"], json!("bad_params"));
    assert_eq!(
        lines[4]["result"]["nodes"],
        json!(21),
        "later lines see the earlier changes"
    );
    // the project was written back
    assert_eq!(
        json_of(&construct(
            &["transport", "network.summary", "--project", path(&p)],
            None
        ))["nodes"],
        json!(21)
    );
    // --stop-on-error stops at the first failure
    let o = construct(&["transport", "batch", "--stop-on-error"], Some(&input));
    assert_eq!(o.out.lines().count(), 3);
    // from a file
    let f = p.with_extension("batch.jsonl");
    std::fs::write(&f, "{\"command\":\"project.info\"}\n").unwrap();
    let o = construct(&["transport", "batch", "--input", path(&f)], None);
    assert_eq!(o.code, 0);
    assert_eq!(
        serde_json::from_str::<Value>(o.out.trim()).unwrap()["ok"],
        json!(true)
    );
}
