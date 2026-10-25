//! `construct mcp`: spawns the server, speaks MCP over its standard input and output (initialize, the list of tools, a call that runs
//! the model, an error) and checks the model's answer against the prototype's recorded run.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

fn truth(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tools/transport-truth")
        .join(rel)
}

struct Client {
    child: Child,
    to: Option<ChildStdin>,
    from: BufReader<ChildStdout>,
    next: u64,
}

impl Client {
    fn start(args: &[&str]) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_construct"))
            .arg("mcp")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the server starts");
        let to = child.stdin.take();
        let from = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            to,
            from,
            next: 1,
        };
        c.call("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } }));
        c.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        c
    }
    fn send(&mut self, v: Value) {
        let to = self.to.as_mut().unwrap();
        writeln!(to, "{v}").unwrap();
        to.flush().unwrap();
    }
    fn line(&mut self) -> Value {
        let mut l = String::new();
        self.from.read_line(&mut l).unwrap();
        serde_json::from_str(l.trim()).unwrap_or_else(|e| panic!("not JSON ({e}): {l:?}"))
    }
    /// A request and its reply (the notifications that came first are returned too).
    fn call(&mut self, method: &str, params: Value) -> (Value, Vec<Value>) {
        let id = self.next;
        self.next += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut notes = Vec::new();
        loop {
            let v = self.line();
            if v.get("id") == Some(&json!(id)) {
                return (v, notes);
            }
            notes.push(v);
        }
    }
    fn tool(&mut self, name: &str, args: Value) -> Value {
        self.call("tools/call", json!({ "name": name, "arguments": args }))
            .0
    }
    /// Closes the server's input, as a client going away does, and waits for it to stop.
    fn finish(mut self) -> bool {
        self.to = None;
        self.child.wait().unwrap().success()
    }
}

fn text_of(reply: &Value) -> Value {
    serde_json::from_str(
        reply["result"]["content"][0]["text"]
            .as_str()
            .expect("text content"),
    )
    .expect("JSON text")
}

#[test]
fn an_agent_can_list_the_tools_and_run_the_model() {
    let project = truth("inputs/grid.json");
    let mut c = Client::start(&["--project", project.to_str().unwrap()]);
    let (init, _) = c.call("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } }));
    assert_eq!(
        init["result"]["serverInfo"]["name"],
        json!("spare-construct")
    );
    assert_eq!(init["result"]["protocolVersion"], json!("2025-06-18"));
    assert!(init["result"]["capabilities"]["tools"].is_object());
    assert!(
        init["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("transport_project_info")
    );

    let (list, _) = c.call("tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 135, "{} tools", tools.len());
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"transport_run") && names.contains(&"transport_network_add_link"));
    assert!(
        names
            .iter()
            .all(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')),
        "tool names use letters, digits and underscores"
    );
    let run = tools
        .iter()
        .find(|t| t["name"] == json!("transport_run"))
        .unwrap();
    assert_eq!(run["inputSchema"]["type"], json!("object"));
    assert!(
        run["description"]
            .as_str()
            .unwrap()
            .contains("Run the model")
    );

    // run the model for the second scenario's AM peak, then read the whole result back
    let r = c.tool(
        "transport_run",
        json!({ "scenario": "s-opt", "periods": ["AM"] }),
    );
    assert!(
        r["result"]
            .get("isError")
            .is_none_or(|e| e == &json!(false)),
        "{r}"
    );
    assert_eq!(text_of(&r)["runs"].as_array().unwrap().len(), 1);
    let got = text_of(&c.tool(
        "transport_results_get",
        json!({ "scenario": "s-opt", "period": "AM" }),
    ));
    let recorded: Value =
        serde_json::from_str(&std::fs::read_to_string(truth("expected/grid.json")).unwrap())
            .unwrap();
    let want = &recorded["scenarios"]["s-opt"]["periods"]["AM"]["res"];
    for k in ["trips", "vkt", "vht", "dly", "spd", "co2", "over"] {
        let (a, b) = (
            got["kpi"][k].as_f64().unwrap(),
            want["kpi"][k].as_f64().unwrap(),
        );
        assert!(
            (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0),
            "{k}: {a} against {b}"
        );
    }
    assert_eq!(got["it"].as_f64(), want["it"].as_f64());
    assert_eq!(
        got["arcs"].as_object().unwrap().len(),
        want["arcs"].as_object().unwrap().len()
    );

    // an error is a tool result with isError and the structured error as its text
    let e = c.tool(
        "transport_network_get",
        json!({ "kind": "node", "id": "nope" }),
    );
    assert_eq!(e["result"]["isError"], json!(true));
    assert_eq!(text_of(&e)["code"], json!("not_found"));
    // an unknown tool is an error too
    let u = c.tool("transport_nothing", json!({}));
    assert!(
        u.get("error").is_some() || u["result"]["isError"] == json!(true),
        "{u}"
    );
    let (p, _) = c.call("ping", json!({}));
    assert_eq!(p["result"], json!({}));
    assert!(c.finish(), "the server stops when its input closes");
}

#[test]
fn the_server_reports_progress_when_asked() {
    let project = truth("inputs/grid.json");
    let mut c = Client::start(&["--project", project.to_str().unwrap()]);
    let (r, notes) = c.call(
        "tools/call",
        json!({ "name": "transport_run", "arguments": { "scenario": "s-dm", "periods": ["AM"] }, "_meta": { "progressToken": "run1" } }),
    );
    assert!(r["result"].is_object());
    let progress: Vec<&Value> = notes
        .iter()
        .filter(|n| n["method"] == json!("notifications/progress"))
        .collect();
    assert!(
        progress.len() >= 3,
        "{} progress notifications",
        progress.len()
    );
    assert_eq!(progress[0]["params"]["progressToken"], json!("run1"));
    let nums: Vec<f64> = progress
        .iter()
        .map(|n| n["params"]["progress"].as_f64().unwrap())
        .collect();
    assert!(
        nums.windows(2).all(|w| w[1] > w[0]),
        "progress goes up: {nums:?}"
    );
    assert!(progress.iter().any(|n| {
        n["params"]["message"]
            .as_str()
            .unwrap()
            .contains("iteration")
    }));
    c.finish();
}

#[test]
fn the_project_is_saved_on_exit_only_when_asked() {
    let dir = std::env::temp_dir().join(format!("construct-mcp-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("p.corridor.json");
    std::fs::copy(truth("inputs/grid.json"), &p).unwrap();
    let before = std::fs::read(&p).unwrap();
    let mut c = Client::start(&["--project", p.to_str().unwrap()]);
    c.tool(
        "transport_network_add_node",
        json!({ "point": [-36.9, 174.8] }),
    );
    c.finish();
    assert_eq!(
        std::fs::read(&p).unwrap(),
        before,
        "not written without --save"
    );
    let mut c = Client::start(&["--project", p.to_str().unwrap(), "--save"]);
    c.tool(
        "transport_network_add_node",
        json!({ "point": [-36.9, 174.8] }),
    );
    c.finish();
    assert_ne!(std::fs::read(&p).unwrap(), before, "written with --save");
}
