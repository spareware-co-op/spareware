//! The transport commands as MCP tools, so an AI agent can use every command the app has.
//!
//! Each command becomes one tool. Tool names use underscores for the dots of the command name (`transport.network.add_link`
//! is `transport_network_add_link`), because MCP clients limit tool names to letters, digits, underscore and hyphen. A tool's
//! result is the command's JSON as text; an error is the structured error JSON with `isError` set.

use super::{Session, registry};
use kit_mcp::{Progress, Server, Tool};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

/// The name MCP clients see for a command: the dots replaced by underscores.
pub fn tool_name(command: &str) -> String {
    command.replace('.', "_")
}

/// The command name for a tool name, if it is one of ours.
pub fn command_of(tool: &str) -> Option<&'static str> {
    registry()
        .all()
        .iter()
        .map(|c| c.name)
        .find(|n| tool_name(n) == tool)
}

/// What the progress line of an event says.
fn progress_text(ev: &Value) -> String {
    let s = |k: &str| ev.get(k).and_then(Value::as_str).unwrap_or("");
    match s("event") {
        "run_start" => format!("{} {} peak: starting", s("name"), s("period")),
        "run_done" => format!("{} {} peak: done", s("name"), s("period")),
        _ => {
            let gap = ev
                .get("gap")
                .and_then(Value::as_f64)
                .map_or(String::new(), |g| format!(", gap {:.3}%", g * 100.0));
            let it = ev
                .get("iteration")
                .and_then(Value::as_u64)
                .map_or(String::new(), |i| format!(" iteration {i}"));
            let who = if s("name").is_empty() {
                String::new()
            } else {
                format!("{} {} peak:", s("name"), s("period"))
            };
            let msg = s("message");
            format!("{who}{it}{gap} {msg}").trim().to_string()
        }
    }
}

/// One MCP tool per transport command, all working on the one shared session.
pub fn tools(session: Rc<RefCell<Session>>) -> Vec<Tool> {
    registry()
        .all()
        .iter()
        .map(|cmd| {
            let name = cmd.name;
            let session = Rc::clone(&session);
            let description = if cmd.changes {
                format!("{} (changes the project)", cmd.description)
            } else {
                cmd.description.to_string()
            };
            Tool::new(
                &tool_name(name),
                &description,
                cmd.schema.clone(),
                move |args: &Value, progress: Progress<'_>| {
                    let mut n = 0.0;
                    let mut sink = |ev: &Value| {
                        n += 1.0;
                        progress(n, None, &progress_text(ev));
                    };
                    match session.borrow_mut().run_with(name, args, &mut sink) {
                        Ok(v) => Ok(v.to_string()),
                        Err(e) => Err(e.to_value().to_string()),
                    }
                },
            )
        })
        .collect()
}

/// An MCP server with every transport command as a tool.
pub fn server(session: Rc<RefCell<Session>>, version: &str) -> Server {
    let mut s = Server::new("spare-construct", version).instructions(
        "Spare Construct Transport, headless. One project is open in the session; every tool works on it. Start with \
         transport_project_info (or transport_project_open with a project file's text), then transport_scenario_list. Edits go \
         through one undo history (transport_history_undo) and make results out of date; run the model with transport_run. \
         Read results with transport_results_summary, _arcs, _nodes and the transport_analysis_* tools. Nothing here downloads \
         data: transport_import_overpass_query and transport_landuse_queries give the queries to send, and their answers go \
         back in through transport_import_osm_roads and transport_landuse_apply.",
    );
    for t in tools(session) {
        s.add_tool(t);
    }
    s
}
