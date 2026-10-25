//! The command line: argument reading, the single-command, batch, command-list and MCP modes, and writing the project back.

use construct_api::Session;
use construct_api::transport::{mcp, registry};
use serde_json::{Map, Value, json};
use std::cell::RefCell;
use std::io::{self, BufRead, Read, Write};
use std::rc::Rc;

const USAGE: &str = "\
construct: Spare Construct on the command line

USAGE
  construct transport <command> [options]   run one Transport command and print its JSON result
  construct transport batch [options]       run JSON-line commands from standard input (or --input FILE)
  construct transport commands [--names]    list every command with its parameter schema
  construct mcp [options]                   serve every command as MCP tools over standard input and output

A command is written transport.network.add_link, network.add_link or transport_network_add_link.

OPTIONS
  --project FILE      open this project file first (a .corridor.json); without it a new empty project is used
  --params JSON       the command's parameters as a JSON object, or @FILE to read them from a file, or - for standard input
  --param KEY=VALUE   set one top-level parameter (repeatable); VALUE is JSON where it parses, else text; @FILE reads a file
  --out FILE          write the project here when the command changed it (default: write back to --project)
  --raw               print the text of a result's text, svg or csv field instead of the JSON
  --pretty            print the JSON result indented
  --progress          print progress events to standard error, one JSON object per line
  --input FILE        batch: read the lines from this file instead of standard input
  --stop-on-error     batch: stop at the first command that fails
  --save              mcp: write the project to --project or --out when the session ends, if it changed
  --names             commands: print only the names
  -h, --help          this text
  --version           the version

EXIT CODES
  0 done   1 a command failed (its error is printed to standard error as JSON)   2 wrong use of the command line
";

/// Everything the option parser reads.
#[derive(Default)]
struct Opts {
    project: Option<String>,
    out: Option<String>,
    params: Option<String>,
    kv: Vec<(String, String)>,
    input: Option<String>,
    progress: bool,
    raw: bool,
    pretty: bool,
    stop_on_error: bool,
    save: bool,
    names: bool,
    rest: Vec<String>,
}

/// What stops the command line: wrong use (exit 2) or a failure (exit 1).
enum Fail {
    Usage(String),
    Run(String),
}

type Res<T> = Result<T, Fail>;

fn usage<T>(m: impl Into<String>) -> Res<T> {
    Err(Fail::Usage(m.into()))
}

fn parse_opts(args: &[String]) -> Res<Opts> {
    let mut o = Opts::default();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") && !["--param"].contains(&f) => {
                (f.to_string(), Some(v.to_string()))
            }
            _ => (a.clone(), None),
        };
        let mut value = |name: &str| -> Res<String> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            i += 1;
            args.get(i)
                .cloned()
                .map_or_else(|| usage(format!("{name} needs a value")), Ok)
        };
        match flag.as_str() {
            "--project" => o.project = Some(value("--project")?),
            "--out" => o.out = Some(value("--out")?),
            "--params" => o.params = Some(value("--params")?),
            "--input" => o.input = Some(value("--input")?),
            "--param" => {
                let v = if a == "--param" {
                    value("--param")?
                } else {
                    a["--param=".len()..].to_string()
                };
                match v.split_once('=') {
                    Some((k, x)) if !k.is_empty() => o.kv.push((k.to_string(), x.to_string())),
                    _ => return usage(format!("--param needs KEY=VALUE, not {v}")),
                }
            }
            "--progress" => o.progress = true,
            "--raw" => o.raw = true,
            "--pretty" => o.pretty = true,
            "--stop-on-error" => o.stop_on_error = true,
            "--save" => o.save = true,
            "--names" => o.names = true,
            "-h" | "--help" => o.rest.push("--help".to_string()),
            f if f.starts_with("--") => return usage(format!("unknown option {f}")),
            _ => o.rest.push(a.clone()),
        }
        i += 1;
    }
    Ok(o)
}

/// Runs the command line and gives the exit code.
pub fn run(args: Vec<String>) -> i32 {
    match dispatch(&args) {
        Ok(code) => code,
        Err(Fail::Usage(m)) => {
            eprintln!("construct: {m}\nRun construct --help for the options.");
            2
        }
        Err(Fail::Run(m)) => {
            eprintln!("{m}");
            1
        }
    }
}

fn dispatch(args: &[String]) -> Res<i32> {
    let first = args.first().map(String::as_str);
    if args.is_empty() || matches!(first, Some("-h" | "--help" | "help")) {
        print!("{USAGE}");
        return Ok(if args.is_empty() { 2 } else { 0 });
    }
    if first == Some("--version") {
        println!("construct {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    let o = parse_opts(&args[1..])?;
    if o.rest.iter().any(|x| x == "--help") {
        print!("{USAGE}");
        return Ok(0);
    }
    match first {
        Some("transport") => transport(o),
        Some("mcp") => serve_mcp(o),
        Some(other) => usage(format!(
            "unknown command {other}; the programs are transport and mcp"
        )),
        None => usage("no command given"),
    }
}

// ------------------------------------------------------------------ session and files

fn read_file(path: &str) -> Res<String> {
    std::fs::read_to_string(path).map_err(|e| Fail::Run(format!("cannot read {path}: {e}")))
}

fn fail_json(e: &construct_api::ApiError) -> Fail {
    Fail::Run(json!({ "error": e.to_value() }).to_string())
}

/// A session with the project file open, and the revision to compare against to know whether the project changed.
fn open_session(o: &Opts) -> Res<(Session, u64)> {
    let mut s = Session::system();
    if let Some(path) = &o.project {
        let text = read_file(path)?;
        s.run("transport.project.open", &json!({ "text": text }))
            .map_err(|e| fail_json(&e))?;
    }
    let rev = s.revision();
    Ok((s, rev))
}

/// Writes the project to `--out` (or back to `--project`) when it changed.
fn persist(s: &Session, rev: u64, o: &Opts) -> Res<()> {
    if s.revision() == rev {
        return Ok(());
    }
    if let Some(path) = o.out.as_ref().or(o.project.as_ref()) {
        std::fs::write(path, s.project().serialise())
            .map_err(|e| Fail::Run(format!("cannot write {path}: {e}")))?;
    }
    Ok(())
}

fn command_name(text: &str) -> Res<String> {
    let reg = registry();
    let t = text.trim();
    for cand in [t.to_string(), format!("transport.{t}")] {
        if reg.get(&cand).is_some() {
            return Ok(cand);
        }
    }
    if let Some(c) = mcp::command_of(t) {
        return Ok(c.to_string());
    }
    usage(format!(
        "there is no command called {t}; run construct transport commands --names to list them"
    ))
}

fn print_json(v: &Value, o: &Opts) {
    let text = if o.pretty {
        serde_json::to_string_pretty(v)
    } else {
        serde_json::to_string(v)
    }
    .unwrap_or_default();
    let out = io::stdout();
    let mut h = out.lock();
    let _ = writeln!(h, "{text}");
    let _ = h.flush();
}

// ------------------------------------------------------------------ parameters

fn schema_type(command: &str, key: &str) -> Option<String> {
    registry().get(command).and_then(|c| {
        c.schema
            .get("properties")?
            .get(key)?
            .get("type")?
            .as_str()
            .map(str::to_string)
    })
}

fn stdin_text() -> Res<String> {
    let mut t = String::new();
    io::stdin()
        .read_to_string(&mut t)
        .map_err(|e| Fail::Run(format!("cannot read standard input: {e}")))?;
    Ok(t)
}

fn build_params(command: &str, o: &Opts) -> Res<Value> {
    let mut obj = Map::new();
    if let Some(p) = &o.params {
        let text = if p == "-" {
            stdin_text()?
        } else if let Some(path) = p.strip_prefix('@') {
            read_file(path)?
        } else {
            p.clone()
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(m)) => obj = m,
            Ok(Value::Null) => {}
            Ok(_) => return usage("--params must be a JSON object"),
            Err(e) => return usage(format!("--params is not valid JSON: {e}")),
        }
    }
    for (k, v) in &o.kv {
        let text = match v.strip_prefix('@') {
            Some(path) => read_file(path)?,
            None => v.clone(),
        };
        let val = if schema_type(command, k).as_deref() == Some("string") {
            Value::String(text)
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        obj.insert(k.clone(), val);
    }
    Ok(Value::Object(obj))
}

// ------------------------------------------------------------------ the transport program

fn transport(o: Opts) -> Res<i32> {
    let Some(sub) = o.rest.first().cloned() else {
        return usage("transport needs a command, or batch, or commands");
    };
    match sub.as_str() {
        "commands" => {
            if o.names {
                for c in registry().all() {
                    println!("{}", c.name);
                }
            } else {
                print_json(&registry().describe(), &o);
            }
            Ok(0)
        }
        "batch" => batch(o),
        _ => single(&sub, o),
    }
}

/// The text of a result's text, svg or csv field, for --raw.
fn raw_text(v: &Value) -> Option<&str> {
    ["text", "svg", "csv"]
        .iter()
        .find_map(|k| v.get(k)?.as_str())
}

fn single(cmd: &str, o: Opts) -> Res<i32> {
    let name = command_name(cmd)?;
    if o.rest.len() > 1 {
        return usage(format!("unexpected argument {}", o.rest[1]));
    }
    let params = build_params(&name, &o)?;
    let (mut s, rev) = open_session(&o)?;
    if o.progress {
        s.set_progress(|ev| eprintln!("{ev}"));
    }
    match s.run(&name, &params) {
        Ok(v) => {
            match (o.raw, raw_text(&v)) {
                (true, Some(t)) => {
                    let out = io::stdout();
                    let mut h = out.lock();
                    let _ = h.write_all(t.as_bytes());
                    let _ = h.flush();
                }
                _ => print_json(&v, &o),
            }
            persist(&s, rev, &o)?;
            Ok(0)
        }
        Err(e) => Err(fail_json(&e)),
    }
}

fn batch(o: Opts) -> Res<i32> {
    if o.rest.len() > 1 {
        return usage(format!("unexpected argument {}", o.rest[1]));
    }
    let (mut s, rev) = open_session(&o)?;
    let lines: Vec<String> = match &o.input {
        Some(path) => read_file(path)?.lines().map(str::to_string).collect(),
        None => io::stdin().lock().lines().map_while(Result::ok).collect(),
    };
    let mut failed = false;
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (id, out) = match serde_json::from_str::<Value>(&line) {
            Err(e) => (Value::Null, Err(format!("the line is not valid JSON: {e}"))),
            Ok(v) => {
                let id = v.get("id").cloned().unwrap_or(Value::Null);
                let cmd = v
                    .get("command")
                    .or_else(|| v.get("name"))
                    .and_then(Value::as_str);
                match cmd {
                    None => (id, Err("the line has no `command`".to_string())),
                    Some(c) => match command_name(c) {
                        Err(Fail::Usage(m) | Fail::Run(m)) => (id, Err(m)),
                        Ok(name) => {
                            let params = v.get("params").cloned().unwrap_or_else(|| json!({}));
                            match s.run(&name, &params) {
                                Ok(r) => (id, Ok(r)),
                                Err(e) => (id, Err(e.to_value().to_string())),
                            }
                        }
                    },
                }
            }
        };
        let mut reply = Map::new();
        if !id.is_null() {
            reply.insert("id".into(), id);
        }
        match out {
            Ok(r) => {
                reply.insert("ok".into(), json!(true));
                reply.insert("result".into(), r);
            }
            Err(m) => {
                failed = true;
                reply.insert("ok".into(), json!(false));
                let e = serde_json::from_str::<Value>(&m)
                    .unwrap_or_else(|_| json!({ "code": "bad_params", "message": m }));
                reply.insert("error".into(), e);
            }
        }
        print_json(&Value::Object(reply), &Opts::default());
        if failed && o.stop_on_error {
            break;
        }
    }
    persist(&s, rev, &o)?;
    Ok(if failed { 1 } else { 0 })
}

fn serve_mcp(o: Opts) -> Res<i32> {
    if let Some(extra) = o.rest.first() {
        return usage(format!("unexpected argument {extra}"));
    }
    let (s, rev) = open_session(&o)?;
    let session = Rc::new(RefCell::new(s));
    let mut server = mcp::server(Rc::clone(&session), env!("CARGO_PKG_VERSION"));
    server
        .serve_stdio()
        .map_err(|e| Fail::Run(format!("the MCP server stopped: {e}")))?;
    if o.save || o.out.is_some() {
        persist(&session.borrow(), rev, &o)?;
    }
    Ok(0)
}
