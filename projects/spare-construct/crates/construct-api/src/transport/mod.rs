//! The transport command layer: every action the Transport app offers, as a named command with JSON in and JSON out.
//!
//! One [`Session`] holds the open project, the active scenario, results per scenario and period, the id source and the undo
//! history. A command is a name (`transport.network.add_link`), a one-line description, a JSON Schema for its parameters and a
//! handler; [`registry`] lists them all. The interface, AI agents (over MCP), scripts (the command line) and the browser build
//! (WebAssembly) all call the same commands, so a command does exactly what the same action does in the app.
//!
//! Commands that act on a scenario take an optional `scenario` (id or name) and make it the active one first, as picking it in
//! the top bar does. Commands that change the network or demand take an undo step (labelled as the app labels it) and mark the
//! scenario's results out of date. Compositions the app makes from several engine calls (drawing a road, adding a zone or a
//! gateway, deleting an object, changing a field) are ported from `demo/transport/js/ui.js` and `ui-inspector.js`.

mod cmd_analysis;
mod cmd_calibrate;
mod cmd_history;
mod cmd_import;
mod cmd_network;
mod cmd_project;
mod cmd_run;
mod cmd_scenario;
mod cmd_sim;
mod cmd_sites;
mod cmd_turns;
mod helpers;
pub mod mcp;
mod registry;
mod session;

pub use registry::{Command, Handler, Registry, registry};
pub use session::{BoxRng, Session};
