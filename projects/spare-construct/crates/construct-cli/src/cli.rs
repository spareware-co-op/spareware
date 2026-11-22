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
    show: Show,
    stop_on_error: bool,
    save: bool,
    names: bool,
    rest: Vec<String>,
}

/// How results and progress are printed.
#[derive(Default)]
struct Show {
    progress: bool,
    raw: bool,
    pretty: bool,
}

/// What stops the command line: wrong use (exit 2) or a failure (exit 1).
enum Fail {
    Usage(String),
    Run(String),
}

type Res<T> = Result<T, Fail>;

fn usage<T>(message: impl Into<String>) -> Res<T> {
    Err(Fail::Usage(message.into()))
}

fn parse_opts(args: &[String]) -> Res<Opts> {
    let mut opts = Opts::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        // `--name=value` splits at the first `=`, so `--param=KEY=VALUE` gives `--param` the pair
        // `KEY=VALUE`, as `--param KEY=VALUE` does.
        let (flag, inline) = match arg.split_once('=') {
            Some((name, given)) if name.starts_with("--") => {
                (name.to_string(), Some(given.to_string()))
            }
            _ => (arg.clone(), None),
        };
        let mut value = |name: &str| -> Res<String> {
            if let Some(given) = inline.clone() {
                return Ok(given);
            }
            args.next()
                .cloned()
                .map_or_else(|| usage(format!("{name} needs a value")), Ok)
        };
        match flag.as_str() {
            "--project" => opts.project = Some(value("--project")?),
            "--out" => opts.out = Some(value("--out")?),
            "--params" => opts.params = Some(value("--params")?),
            "--input" => opts.input = Some(value("--input")?),
            "--param" => {
                let pair = value("--param")?;
                match pair.split_once('=') {
                    Some((key, given)) if !key.is_empty() => {
                        opts.kv.push((key.to_string(), given.to_string()));
                    }
                    _ => return usage(format!("--param needs KEY=VALUE, not {pair}")),
                }
            }
            "--progress" => opts.show.progress = true,
            "--raw" => opts.show.raw = true,
            "--pretty" => opts.show.pretty = true,
            "--stop-on-error" => opts.stop_on_error = true,
            "--save" => opts.save = true,
            "--names" => opts.names = true,
            "-h" | "--help" => opts.rest.push("--help".to_string()),
            unknown if unknown.starts_with("--") => {
                return usage(format!("unknown option {unknown}"));
            }
            _ => opts.rest.push(arg.clone()),
        }
    }
    Ok(opts)
}

/// Runs the command line and gives the exit code.
pub fn run(args: &[String]) -> i32 {
    match dispatch(args) {
        Ok(code) => code,
        Err(Fail::Usage(message)) => {
            eprintln!("construct: {message}\nRun construct --help for the options.");
            2
        }
        Err(Fail::Run(message)) => {
            eprintln!("{message}");
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
    let opts = parse_opts(args.get(1..).unwrap_or_default())?;
    if opts.rest.iter().any(|arg| arg == "--help") {
        print!("{USAGE}");
        return Ok(0);
    }
    match first {
        Some("transport") => transport(&opts),
        Some("mcp") => serve_mcp(&opts),
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
fn open_session(opts: &Opts) -> Res<(Session, u64)> {
    let mut session = Session::system();
    if let Some(path) = &opts.project {
        let text = read_file(path)?;
        session
            .run("transport.project.open", &json!({ "text": text }))
            .map_err(|e| fail_json(&e))?;
    }
    let rev = session.revision();
    Ok((session, rev))
}

/// Writes the project to `--out` (or back to `--project`) when it changed.
fn persist(session: &Session, rev: u64, opts: &Opts) -> Res<()> {
    if session.revision() == rev {
        return Ok(());
    }
    if let Some(path) = opts.out.as_ref().or(opts.project.as_ref()) {
        std::fs::write(path, session.project().serialise())
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

fn print_json(v: &Value, show: &Show) {
    let text = if show.pretty {
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

fn build_params(command: &str, opts: &Opts) -> Res<Value> {
    let mut obj = Map::new();
    if let Some(p) = &opts.params {
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
    for (k, v) in &opts.kv {
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

fn transport(opts: &Opts) -> Res<i32> {
    let Some(sub) = opts.rest.first() else {
        return usage("transport needs a command, or batch, or commands");
    };
    match sub.as_str() {
        "commands" => {
            if opts.names {
                for c in registry().all() {
                    println!("{}", c.name);
                }
            } else {
                print_json(&registry().describe(), &opts.show);
            }
            Ok(0)
        }
        "batch" => batch(opts),
        _ => single(sub, opts),
    }
}

/// The text of a result's text, svg or csv field, for --raw.
fn raw_text(v: &Value) -> Option<&str> {
    ["text", "svg", "csv"]
        .iter()
        .find_map(|k| v.get(k)?.as_str())
}

fn single(cmd: &str, opts: &Opts) -> Res<i32> {
    let name = command_name(cmd)?;
    if let Some(extra) = opts.rest.get(1) {
        return usage(format!("unexpected argument {extra}"));
    }
    let params = build_params(&name, opts)?;
    let (mut session, rev) = open_session(opts)?;
    if opts.show.progress {
        session.set_progress(|event| eprintln!("{event}"));
    }
    match session.run(&name, &params) {
        Ok(result) => {
            match (opts.show.raw, raw_text(&result)) {
                (true, Some(text)) => {
                    let out = io::stdout();
                    let mut handle = out.lock();
                    let _ = handle.write_all(text.as_bytes());
                    let _ = handle.flush();
                }
                _ => print_json(&result, &opts.show),
            }
            persist(&session, rev, opts)?;
            Ok(0)
        }
        Err(e) => Err(fail_json(&e)),
    }
}

fn batch(opts: &Opts) -> Res<i32> {
    if let Some(extra) = opts.rest.get(1) {
        return usage(format!("unexpected argument {extra}"));
    }
    let (mut session, rev) = open_session(opts)?;
    let lines: Vec<String> = match &opts.input {
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
            Ok(request) => {
                let id = request.get("id").cloned().unwrap_or(Value::Null);
                let cmd = request
                    .get("command")
                    .or_else(|| request.get("name"))
                    .and_then(Value::as_str);
                match cmd {
                    None => (id, Err("the line has no `command`".to_string())),
                    Some(text) => match command_name(text) {
                        Err(Fail::Usage(message) | Fail::Run(message)) => (id, Err(message)),
                        Ok(name) => {
                            let params =
                                request.get("params").cloned().unwrap_or_else(|| json!({}));
                            match session.run(&name, &params) {
                                Ok(result) => (id, Ok(result)),
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
            Ok(result) => {
                reply.insert("ok".into(), json!(true));
                reply.insert("result".into(), result);
            }
            Err(message) => {
                failed = true;
                reply.insert("ok".into(), json!(false));
                let error = serde_json::from_str::<Value>(&message)
                    .unwrap_or_else(|_| json!({ "code": "bad_params", "message": message }));
                reply.insert("error".into(), error);
            }
        }
        print_json(&Value::Object(reply), &Show::default());
        if failed && opts.stop_on_error {
            break;
        }
    }
    persist(&session, rev, opts)?;
    Ok(i32::from(failed))
}

fn serve_mcp(opts: &Opts) -> Res<i32> {
    if let Some(extra) = opts.rest.first() {
        return usage(format!("unexpected argument {extra}"));
    }
    let (session, rev) = open_session(opts)?;
    let session = Rc::new(RefCell::new(session));
    let mut server = mcp::server(&session, env!("CARGO_PKG_VERSION"));
    server
        .serve_stdio()
        .map_err(|e| Fail::Run(format!("the MCP server stopped: {e}")))?;
    if opts.save || opts.out.is_some() {
        persist(&session.borrow(), rev, opts)?;
    }
    Ok(0)
}
