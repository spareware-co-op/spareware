//! Generic MCP (Model Context Protocol) server over stdio, shared by every Spareware program.
//!
//! The caller supplies tools (name, description, JSON Schema, handler) and this crate speaks
//! JSON-RPC 2.0 with newline-delimited messages. It depends on no program crate. A tool handler
//! must never write to stdout, because stdout is the protocol channel (use stderr for logs).
//!
//! Every protocol detail below is copied from the MCP TypeScript SDK 1.25.2 (the copy installed
//! in `RevitDevMCP/revit-dev-mcp-server/node_modules/@modelcontextprotocol/sdk`, `dist/esm`):
//!
//! - Protocol versions: `LATEST_PROTOCOL_VERSION` and `SUPPORTED_PROTOCOL_VERSIONS` from `types.js`.
//! - `initialize`: `Server._oninitialize` in `server/index.js` answers with the client's version
//!   when it is supported, else the latest one, plus `capabilities`, `serverInfo` and (only when
//!   non-empty) `instructions`. The request params (`protocolVersion`, `capabilities`,
//!   `clientInfo` with `name` and `version`) follow `InitializeRequestParamsSchema` in `types.js`.
//! - Capabilities: `{ "tools": {} }` shape from `ServerCapabilitiesSchema` in `types.js`. The SDK's
//!   `McpServer` (`server/mcp.js`, `setToolRequestHandlers`) sets `listChanged: true`; this crate
//!   sends `false` because the tool list is fixed once serving starts and no
//!   `notifications/tools/list_changed` is ever emitted.
//! - `ping`: empty result `{}`, `Protocol` constructor in `shared/protocol.js`.
//! - `tools/list`: `{ "tools": [ { name, description, inputSchema } ] }` in registration order,
//!   `ListToolsResultSchema` and `ToolSchema` in `types.js`, handler in `server/mcp.js`. No
//!   `nextCursor` is sent, as in the SDK.
//! - `tools/call`: params `{ name, arguments?, _meta? }` from `CallToolRequestParamsSchema` in
//!   `types.js`. A success is `{ content: [ { type: "text", text } ] }` and a tool failure adds
//!   `isError: true` (`createToolError` in `server/mcp.js`, `CallToolResultSchema` in `types.js`).
//!   An unknown tool is a protocol error `-32602` with the text `Tool <name> not found`
//!   (`server/mcp.js`, and the `CallToolResultSchema` comment in `types.js`: errors in finding the
//!   tool are MCP error responses). Note that SDK 1.25.2 itself catches that error and turns it
//!   into an `isError` result; this crate keeps the protocol error as the spec asks.
//! - Progress: `notifications/progress` with `{ progressToken, progress, total?, message? }`,
//!   `ProgressNotificationSchema` in `types.js`; the token is `params._meta.progressToken`
//!   (`RequestMetaSchema`, a string or an integer).
//! - Errors: codes `-32700`, `-32600`, `-32601`, `-32602`, `-32603` from `ErrorCode` in
//!   `types.js`; the text `Method not found` from `_onrequest` in `shared/protocol.js`.
//! - Transport: one JSON message per line, `\r` before `\n` tolerated, replies are the compact
//!   JSON text plus `\n`, from `shared/stdio.js` (`ReadBuffer`, `serializeMessage`) and
//!   `server/stdio.js`. Responses to server requests are ignored because this server sends none.
//!
//! Where the SDK sends nothing (invalid JSON, a message of the wrong shape) this crate answers
//! with the JSON-RPC 2.0 standard error and an `id` of `null` when none can be read.

#![warn(missing_docs)]

use std::io::{self, BufRead, Write};

use serde_json::{Map, Number, Value, json};

/// The newest MCP protocol version this server speaks (`LATEST_PROTOCOL_VERSION` in the SDK).
pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";

/// Every MCP protocol version this server accepts (`SUPPORTED_PROTOCOL_VERSIONS` in the SDK).
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 5] = [
    LATEST_PROTOCOL_VERSION,
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];

const JSONRPC_VERSION: &str = "2.0";
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Callback a tool handler calls to report progress: progress so far, optional total, message.
pub type Progress<'a> = &'a mut dyn FnMut(f64, Option<f64>, &str);

/// What a tool handler returns: `Ok(text)` is a success, `Err(text)` a tool failure.
pub type ToolResult = Result<String, String>;

/// The code that runs a tool: it gets the arguments object and a progress callback.
pub type ToolHandler = Box<dyn FnMut(&Value, Progress<'_>) -> ToolResult>;

/// One tool the server offers, with the handler that runs when a client calls it.
pub struct Tool {
    /// The unique name clients use to call the tool.
    pub name: String,
    /// A plain-English description shown to the client and the model.
    pub description: String,
    /// A JSON Schema object (`"type": "object"` at the root) describing the arguments.
    pub input_schema: Value,
    /// The code that runs the tool: it gets the arguments object and a progress callback.
    pub handler: ToolHandler,
}

impl Tool {
    /// Builds a tool, boxing the handler for the caller.
    pub fn new(
        name: &str,
        description: &str,
        input_schema: Value,
        handler: impl FnMut(&Value, Progress<'_>) -> ToolResult + 'static,
    ) -> Tool {
        Tool {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
            handler: Box::new(handler),
        }
    }
}

/// An MCP server: its identity, optional instructions and the tools it offers, in order.
pub struct Server {
    name: String,
    version: String,
    instructions: Option<String>,
    tools: Vec<Tool>,
}

/// The parts of a well-formed JSON-RPC message: `id` is absent for a notification.
struct Shape<'a> {
    id: Option<&'a Value>,
    method: &'a str,
    params: Option<&'a Map<String, Value>>,
}

/// A JSON-RPC error on its way to becoming an error response.
struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn invalid_params(message: impl Into<String>) -> RpcError {
        RpcError {
            code: INVALID_PARAMS,
            message: message.into(),
        }
    }
}

impl Server {
    /// Creates a server that reports the given name and version in the `initialize` reply.
    pub fn new(name: &str, version: &str) -> Server {
        Server {
            name: name.to_string(),
            version: version.to_string(),
            instructions: None,
            tools: Vec::new(),
        }
    }

    /// Sets the instructions text sent to clients in the `initialize` reply.
    pub fn instructions(mut self, text: &str) -> Server {
        self.instructions = Some(text.to_string());
        self
    }

    /// Adds a tool after the ones already added, so that is also its place in `tools/list`.
    ///
    /// # Panics
    ///
    /// Panics when a tool with the same name was already added, as the SDK throws.
    pub fn add_tool(&mut self, tool: Tool) {
        assert!(
            !self.tools.iter().any(|t| t.name == tool.name),
            "Tool {} is already registered",
            tool.name
        );
        self.tools.push(tool);
    }

    /// Handles one incoming line and returns the lines to write back, in order: any progress
    /// notifications sent while a tool ran, then the response (none for notifications).
    ///
    /// The returned lines have no trailing newline.
    pub fn handle_line(&mut self, line: &str) -> Vec<String> {
        let mut lines = Vec::new();
        self.handle_line_with(line, &mut |reply| lines.push(reply));
        lines
    }

    /// Handles one incoming line and hands each outgoing line to `emit` the moment it exists,
    /// so progress notifications can reach the client while a long tool is still running.
    pub fn handle_line_with(&mut self, line: &str, emit: &mut dyn FnMut(String)) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(message) => self.handle_message(message, emit),
            Err(error) => emit(error_line(
                &Value::Null,
                PARSE_ERROR,
                &format!("Parse error: {error}"),
            )),
        }
    }

    /// Reads lines until end of input, writing and flushing each reply as soon as it exists.
    pub fn serve<R: BufRead, W: Write>(&mut self, mut input: R, mut output: W) -> io::Result<()> {
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            if input.read_until(b'\n', &mut buffer)? == 0 {
                return Ok(());
            }
            // Bad UTF-8 becomes replacement characters and is then answered as a parse error.
            let line = String::from_utf8_lossy(&buffer);
            let mut failure: Option<io::Error> = None;
            self.handle_line_with(&line, &mut |reply: String| {
                if failure.is_none() {
                    failure = write_line(&mut output, reply).err();
                }
            });
            if let Some(error) = failure {
                return Err(error);
            }
        }
    }

    /// Serves the process's standard input and output until standard input closes.
    pub fn serve_stdio(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        self.serve(stdin.lock(), stdout.lock())
    }

    /// Checks the shape of one parsed message and routes it.
    fn handle_message(&mut self, message: Value, emit: &mut dyn FnMut(String)) {
        let Value::Object(message) = message else {
            emit(error_line(
                &Value::Null,
                INVALID_REQUEST,
                "Invalid Request: a message must be a JSON object",
            ));
            return;
        };
        if !message.contains_key("method")
            && (message.contains_key("result") || message.contains_key("error"))
        {
            return; // A response from the client: this server sends no requests, so ignore it.
        }
        let shape = match check_shape(&message) {
            Ok(shape) => shape,
            Err(text) => {
                let id = message
                    .get("id")
                    .filter(|v| is_id(v))
                    .unwrap_or(&Value::Null);
                emit(error_line(id, INVALID_REQUEST, text));
                return;
            }
        };
        // No id means a notification (initialized, cancelled, anything else): nothing to do or send.
        let Some(id) = shape.id else {
            return;
        };
        let line = match self.dispatch(shape.method, shape.params, emit) {
            Ok(result) => result_line(id, result),
            Err(error) => error_line(id, error.code, &error.message),
        };
        emit(line);
    }

    /// Runs the method of a request and returns its result or its JSON-RPC error.
    fn dispatch(
        &mut self,
        method: &str,
        params: Option<&Map<String, Value>>,
        emit: &mut dyn FnMut(String),
    ) -> Result<Value, RpcError> {
        match method {
            "initialize" => self.initialize(params),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(self.list_tools()),
            "tools/call" => self.call_tool(params, emit),
            _ => Err(RpcError {
                code: METHOD_NOT_FOUND,
                message: "Method not found".to_string(),
            }),
        }
    }

    /// Answers `initialize`: the client's protocol version when supported, else the latest.
    fn initialize(&self, params: Option<&Map<String, Value>>) -> Result<Value, RpcError> {
        let bad = |what: &str| {
            RpcError::invalid_params(format!("Invalid initialize request params: {what}"))
        };
        let params = params.ok_or_else(|| bad("params are required"))?;
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("protocolVersion must be a string"))?;
        if params.get("capabilities").is_none_or(|c| !c.is_object()) {
            return Err(bad("capabilities must be an object"));
        }
        let client_named = params
            .get("clientInfo")
            .is_some_and(|c| c["name"].is_string() && c["version"].is_string());
        if !client_named {
            return Err(bad(
                "clientInfo must be an object with a name and a version",
            ));
        }
        let protocol_version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
            requested
        } else {
            LATEST_PROTOCOL_VERSION
        };
        let mut result = json!({
            "protocolVersion": protocol_version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": self.name, "version": self.version },
        });
        if let Some(text) = self.instructions.as_deref().filter(|t| !t.is_empty()) {
            result["instructions"] = json!(text);
        }
        Ok(result)
    }

    /// Answers `tools/list` with every tool in the order it was added.
    fn list_tools(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                })
            })
            .collect();
        json!({ "tools": tools })
    }

    /// Answers `tools/call`: runs the handler, sending progress notifications while it runs.
    fn call_tool(
        &mut self,
        params: Option<&Map<String, Value>>,
        emit: &mut dyn FnMut(String),
    ) -> Result<Value, RpcError> {
        let bad = |what: &str| {
            RpcError::invalid_params(format!("Invalid tools/call request params: {what}"))
        };
        let params = params.ok_or_else(|| bad("params are required"))?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("name must be a string"))?;
        let no_arguments = Value::Object(Map::new());
        let arguments = match params.get("arguments") {
            None => &no_arguments,
            Some(value) if value.is_object() => value,
            Some(_) => return Err(bad("arguments must be an object")),
        };
        let token = params
            .get("_meta")
            .and_then(|meta| meta.get("progressToken"))
            .filter(|token| is_id(token));
        let tool = self
            .tools
            .iter_mut()
            .find(|tool| tool.name == name)
            .ok_or_else(|| RpcError::invalid_params(format!("Tool {name} not found")))?;
        let mut report = |progress: f64, total: Option<f64>, message: &str| {
            // Without a progress token the client did not ask for notifications.
            if let Some(line) = token.and_then(|t| progress_line(t, progress, total, message)) {
                emit(line);
            }
        };
        Ok(tool_result((tool.handler)(arguments, &mut report)))
    }
}

/// Checks the JSON-RPC shape of a message and returns its id, method and params.
fn check_shape(message: &Map<String, Value>) -> Result<Shape<'_>, &'static str> {
    if message.get("jsonrpc").and_then(Value::as_str) != Some(JSONRPC_VERSION) {
        return Err("Invalid Request: jsonrpc must be \"2.0\"");
    }
    let id = message.get("id");
    if id.is_some_and(|id| !is_id(id)) {
        return Err("Invalid Request: id must be a string or an integer");
    }
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Err("Invalid Request: method must be a string");
    };
    let params = match message.get("params") {
        None => None,
        Some(Value::Object(params)) => Some(params),
        Some(_) => return Err("Invalid Request: params must be an object"),
    };
    Ok(Shape { id, method, params })
}

/// A request id or progress token is a string or an integer.
fn is_id(value: &Value) -> bool {
    value.is_string() || value.is_i64() || value.is_u64()
}

/// Builds the `tools/call` result for a handler's outcome.
fn tool_result(outcome: ToolResult) -> Value {
    match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
        Err(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
    }
}

/// Builds a successful JSON-RPC response line.
fn result_line(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": JSONRPC_VERSION, "id": id, "result": result }).to_string()
}

/// Builds a JSON-RPC error response line.
fn error_line(id: &Value, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": { "code": code, "message": message },
    })
    .to_string()
}

/// Builds a `notifications/progress` line, or nothing when the progress is not a finite number.
fn progress_line(
    token: &Value,
    progress: f64,
    total: Option<f64>,
    message: &str,
) -> Option<String> {
    let mut params = Map::new();
    params.insert("progressToken".to_string(), token.clone());
    params.insert(
        "progress".to_string(),
        Value::Number(Number::from_f64(progress)?),
    );
    if let Some(total) = total.and_then(Number::from_f64) {
        params.insert("total".to_string(), Value::Number(total));
    }
    if !message.is_empty() {
        params.insert("message".to_string(), Value::String(message.to_string()));
    }
    Some(
        json!({
            "jsonrpc": JSONRPC_VERSION,
            "method": "notifications/progress",
            "params": params,
        })
        .to_string(),
    )
}

/// Writes one reply followed by a newline and flushes it.
fn write_line<W: Write>(output: &mut W, reply: String) -> io::Result<()> {
    let mut bytes = reply.into_bytes();
    bytes.push(b'\n');
    output.write_all(&bytes)?;
    output.flush()
}
