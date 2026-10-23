//! Protocol tests for the shared MCP server: messages in, JSON-RPC replies out.

use kit_mcp::{LATEST_PROTOCOL_VERSION, SUPPORTED_PROTOCOL_VERSIONS, Server, Tool};
use serde_json::{Value, json};

/// A server with four tools: echo, fail, count (keeps state) and work (reports progress).
fn server() -> Server {
    let mut server = Server::new("kit-test", "1.2.3").instructions("Call the tools politely.");
    server.add_tool(Tool::new(
        "echo",
        "Returns the text argument.",
        json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"],
        }),
        |args, _| Ok(args["text"].as_str().unwrap_or("").to_string()),
    ));
    server.add_tool(Tool::new(
        "fail",
        "Always fails.",
        json!({ "type": "object" }),
        |_, _| Err("it broke".to_string()),
    ));
    let mut calls = 0;
    server.add_tool(Tool::new(
        "count",
        "Counts its own calls and shows the arguments it got.",
        json!({ "type": "object" }),
        move |args, _| {
            calls += 1;
            Ok(format!("call {calls} with {args}"))
        },
    ));
    server.add_tool(Tool::new(
        "work",
        "Reports progress three times.",
        json!({ "type": "object" }),
        |_, progress| {
            progress(1.0, Some(3.0), "step one");
            progress(2.0, Some(3.0), "step two");
            progress(3.0, None, "");
            Ok("done".to_string())
        },
    ));
    server
}

/// Sends one message and parses every line the server answers with.
fn send(server: &mut Server, message: Value) -> Vec<Value> {
    send_raw(server, &message.to_string())
}

/// Sends one raw line and parses every line the server answers with.
fn send_raw(server: &mut Server, line: &str) -> Vec<Value> {
    server
        .handle_line(line)
        .iter()
        .map(|reply| {
            assert!(
                !reply.contains('\n'),
                "a reply must be a single line: {reply}"
            );
            serde_json::from_str(reply).expect("replies are valid JSON")
        })
        .collect()
}

/// Sends one message and returns its only reply.
fn ask(server: &mut Server, message: Value) -> Value {
    let mut replies = send(server, message);
    assert_eq!(replies.len(), 1, "expected exactly one reply: {replies:?}");
    replies.remove(0)
}

fn initialize_message(version: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": version,
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "0.1" },
        },
    })
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
}

#[test]
fn initialize_answers_a_supported_version_with_the_same_version() {
    for version in SUPPORTED_PROTOCOL_VERSIONS {
        let reply = ask(&mut server(), initialize_message(version));
        assert_eq!(
            reply,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "kit-test", "version": "1.2.3" },
                    "instructions": "Call the tools politely.",
                },
            })
        );
    }
}

#[test]
fn initialize_answers_an_unsupported_version_with_the_latest() {
    let reply = ask(&mut server(), initialize_message("1999-01-01"));
    assert_eq!(reply["result"]["protocolVersion"], LATEST_PROTOCOL_VERSION);
    assert_eq!(LATEST_PROTOCOL_VERSION, "2025-11-25");
}

#[test]
fn protocol_versions_are_the_sdk_ones() {
    assert_eq!(
        SUPPORTED_PROTOCOL_VERSIONS,
        [
            "2025-11-25",
            "2025-06-18",
            "2025-03-26",
            "2024-11-05",
            "2024-10-07"
        ]
    );
}

#[test]
fn initialize_leaves_out_empty_or_missing_instructions() {
    let mut bare = Server::new("bare", "0.1");
    let reply = ask(&mut bare, initialize_message("2025-06-18"));
    assert!(reply["result"].get("instructions").is_none());
    let mut empty = Server::new("empty", "0.1").instructions("");
    let reply = ask(&mut empty, initialize_message("2025-06-18"));
    assert!(reply["result"].get("instructions").is_none());
}

#[test]
fn initialize_with_bad_params_is_invalid_params() {
    let bad_params = [
        json!({}),
        json!({ "capabilities": {}, "clientInfo": { "name": "c", "version": "1" } }),
        json!({ "protocolVersion": 5, "capabilities": {}, "clientInfo": { "name": "c", "version": "1" } }),
        json!({ "protocolVersion": "2025-06-18", "clientInfo": { "name": "c", "version": "1" } }),
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "c" } }),
    ];
    for params in bad_params {
        let reply = ask(
            &mut server(),
            json!({ "jsonrpc": "2.0", "id": 7, "method": "initialize", "params": params }),
        );
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["error"]["code"], -32602, "{params}");
    }
    let reply = ask(
        &mut server(),
        json!({ "jsonrpc": "2.0", "id": 8, "method": "initialize" }),
    );
    assert_eq!(reply["error"]["code"], -32602);
}

#[test]
fn initialized_notification_gets_no_reply() {
    let replies = send(
        &mut server(),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    );
    assert!(replies.is_empty());
}

#[test]
fn unknown_and_cancelled_notifications_are_ignored() {
    let mut server = server();
    for method in [
        "notifications/cancelled",
        "notifications/roots/list_changed",
        "whatever",
    ] {
        let replies = send(
            &mut server,
            json!({ "jsonrpc": "2.0", "method": method, "params": { "requestId": 3 } }),
        );
        assert!(replies.is_empty(), "{method}");
    }
}

#[test]
fn ping_answers_an_empty_object() {
    let reply = ask(
        &mut server(),
        json!({ "jsonrpc": "2.0", "id": "p-1", "method": "ping" }),
    );
    assert_eq!(
        reply,
        json!({ "jsonrpc": "2.0", "id": "p-1", "result": {} })
    );
}

#[test]
fn tools_list_gives_the_tools_in_the_order_added() {
    let reply = ask(
        &mut server(),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    );
    let tools = reply["result"]["tools"].as_array().expect("a tools array");
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["echo", "fail", "count", "work"]);
    assert_eq!(tools[0]["description"], "Returns the text argument.");
    assert_eq!(
        tools[0]["inputSchema"],
        json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"],
        })
    );
    assert_eq!(tools[1]["inputSchema"], json!({ "type": "object" }));
    assert!(reply["result"].get("nextCursor").is_none());
}

#[test]
fn tools_list_with_no_tools_is_an_empty_list() {
    let reply = ask(
        &mut Server::new("empty", "0.1"),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    );
    assert_eq!(reply["result"], json!({ "tools": [] }));
}

#[test]
fn tools_call_success_returns_text_content() {
    let reply = ask(&mut server(), call(3, "echo", json!({ "text": "hello" })));
    assert_eq!(
        reply,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": { "content": [{ "type": "text", "text": "hello" }] },
        })
    );
}

#[test]
fn tools_call_handler_error_is_a_normal_result_with_is_error() {
    let reply = ask(&mut server(), call(4, "fail", json!({})));
    assert!(reply.get("error").is_none());
    assert_eq!(
        reply["result"],
        json!({
            "content": [{ "type": "text", "text": "it broke" }],
            "isError": true,
        })
    );
}

#[test]
fn tools_call_without_arguments_gives_the_handler_an_empty_object() {
    let reply = ask(
        &mut server(),
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "count" } }),
    );
    assert_eq!(reply["result"]["content"][0]["text"], "call 1 with {}");
}

#[test]
fn handlers_keep_their_state_between_calls() {
    let mut server = server();
    ask(&mut server, call(1, "count", json!({})));
    let reply = ask(&mut server, call(2, "count", json!({ "a": 1 })));
    assert_eq!(
        reply["result"]["content"][0]["text"],
        "call 2 with {\"a\":1}"
    );
}

#[test]
fn tools_call_unknown_tool_is_invalid_params() {
    let reply = ask(&mut server(), call(6, "nope", json!({})));
    assert_eq!(
        reply,
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "error": { "code": -32602, "message": "Tool nope not found" },
        })
    );
}

#[test]
fn tools_call_with_bad_params_is_invalid_params() {
    let bad_calls = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call" }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {} }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": 9 } }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "echo", "arguments": [] } }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "echo", "arguments": null } }),
    ];
    for message in bad_calls {
        let reply = ask(&mut server(), message.clone());
        assert_eq!(reply["id"], 1);
        assert_eq!(reply["error"]["code"], -32602, "{message}");
    }
}

#[test]
fn unknown_method_is_method_not_found_and_keeps_the_id() {
    let reply = ask(
        &mut server(),
        json!({ "jsonrpc": "2.0", "id": "abc", "method": "resources/list" }),
    );
    assert_eq!(
        reply,
        json!({
            "jsonrpc": "2.0",
            "id": "abc",
            "error": { "code": -32601, "message": "Method not found" },
        })
    );
}

#[test]
fn invalid_json_is_a_parse_error_with_a_null_id() {
    let replies = send_raw(&mut server(), "{ not json");
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["jsonrpc"], "2.0");
    assert_eq!(replies[0]["id"], Value::Null);
    assert_eq!(replies[0]["error"]["code"], -32700);
}

#[test]
fn a_message_of_the_wrong_shape_is_an_invalid_request() {
    let cases = [
        ("42", Value::Null),
        ("[]", Value::Null),
        ("\"ping\"", Value::Null),
        ("{}", Value::Null),
        (r#"{"id":1,"method":"ping"}"#, json!(1)),
        (r#"{"jsonrpc":"1.0","id":2,"method":"ping"}"#, json!(2)),
        (r#"{"jsonrpc":"2.0","id":3,"method":7}"#, json!(3)),
        (r#"{"jsonrpc":"2.0","id":4}"#, json!(4)),
        (
            r#"{"jsonrpc":"2.0","id":5,"method":"ping","params":[1]}"#,
            json!(5),
        ),
        (
            r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#,
            Value::Null,
        ),
        (r#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#, Value::Null),
        (r#"{"jsonrpc":"2.0","id":[1],"method":"ping"}"#, Value::Null),
        (
            r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":3}"#,
            Value::Null,
        ),
    ];
    for (line, id) in cases {
        let replies = send_raw(&mut server(), line);
        assert_eq!(replies.len(), 1, "{line}");
        assert_eq!(replies[0]["error"]["code"], -32600, "{line}");
        assert_eq!(replies[0]["id"], id, "{line}");
    }
}

#[test]
fn responses_from_the_client_are_ignored() {
    let mut server = server();
    assert!(send_raw(&mut server, r#"{"jsonrpc":"2.0","id":1,"result":{}}"#).is_empty());
    assert!(
        send_raw(
            &mut server,
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-1,"message":"x"}}"#
        )
        .is_empty()
    );
}

#[test]
fn blank_lines_are_skipped_and_carriage_returns_tolerated() {
    let mut server = server();
    assert!(send_raw(&mut server, "").is_empty());
    assert!(send_raw(&mut server, "   \r").is_empty());
    let replies = send_raw(
        &mut server,
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\r",
    );
    assert_eq!(
        replies,
        [json!({ "jsonrpc": "2.0", "id": 1, "result": {} })]
    );
}
