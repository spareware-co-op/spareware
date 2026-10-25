//! The registry of transport commands: stable names, one-line descriptions, parameter schemas and handlers.

use super::Session;
use crate::error::ApiResult;
use crate::schema::Params;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Where a long-running command sends its progress events (`{ event, ... }`) while it runs.
pub type Sink<'a> = &'a mut dyn FnMut(&Value);

/// What a command does when called: reads its parameters, works on the session and gives JSON back.
#[derive(Clone, Copy)]
pub enum Handler {
    /// A command that finishes at once.
    Plain(fn(&mut Session, &Params) -> ApiResult),
    /// A command that reports progress while it runs.
    Streaming(fn(&mut Session, &Params, Sink<'_>) -> ApiResult),
}

/// One command of the transport API.
pub struct Command {
    /// Stable dot-form name, for example transport.network.add_link.
    pub name: &'static str,
    /// One line saying what the command does.
    pub description: &'static str,
    /// True when the command can change the project (it then counts towards "the project changed").
    pub changes: bool,
    /// JSON Schema of the parameters.
    pub schema: Value,
    /// The function that runs the command.
    pub handler: Handler,
}

/// Every command, in the order they were added.
pub struct Registry {
    list: Vec<Command>,
    by_name: HashMap<&'static str, usize>,
}

impl Registry {
    fn new() -> Registry {
        Registry {
            list: Vec::new(),
            by_name: HashMap::new(),
        }
    }
    /// Adds a command; a name used twice is a bug and panics when the registry is first built.
    pub fn add(
        &mut self,
        name: &'static str,
        description: &'static str,
        changes: bool,
        schema: Value,
        handler: fn(&mut Session, &Params) -> ApiResult,
    ) {
        self.push(name, description, changes, schema, Handler::Plain(handler));
    }
    /// Adds a command that reports progress through a sink while it runs.
    pub fn add_streaming(
        &mut self,
        name: &'static str,
        description: &'static str,
        changes: bool,
        schema: Value,
        handler: fn(&mut Session, &Params, Sink<'_>) -> ApiResult,
    ) {
        self.push(
            name,
            description,
            changes,
            schema,
            Handler::Streaming(handler),
        );
    }
    fn push(
        &mut self,
        name: &'static str,
        description: &'static str,
        changes: bool,
        schema: Value,
        handler: Handler,
    ) {
        let i = self.list.len();
        assert!(
            self.by_name.insert(name, i).is_none(),
            "command {name} is registered twice"
        );
        self.list.push(Command {
            name,
            description,
            changes,
            schema,
            handler,
        });
    }
    /// The command with this name.
    pub fn get(&self, name: &str) -> Option<&Command> {
        self.by_name.get(name).map(|&i| &self.list[i])
    }
    /// Every command in order.
    pub fn all(&self) -> &[Command] {
        &self.list
    }
    /// Every command as JSON: name, description, changes and the parameter schema.
    pub fn describe(&self) -> Value {
        Value::Array(
            self.list
                .iter()
                .map(|c| json!({ "name": c.name, "description": c.description, "changes": c.changes, "params": c.schema }))
                .collect(),
        )
    }
}

/// The registry of every transport command, built once.
pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut r = Registry::new();
        super::cmd_project::register(&mut r);
        super::cmd_scenario::register(&mut r);
        super::cmd_network::register(&mut r);
        super::cmd_sites::register(&mut r);
        super::cmd_import::register(&mut r);
        super::cmd_run::register(&mut r);
        super::cmd_analysis::register(&mut r);
        super::cmd_turns::register(&mut r);
        super::cmd_sim::register(&mut r);
        super::cmd_history::register(&mut r);
        super::cmd_calibrate::register(&mut r);
        r
    })
}
