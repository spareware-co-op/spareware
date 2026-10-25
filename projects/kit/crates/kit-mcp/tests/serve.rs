//! Progress notifications and the line-by-line `serve` loop of the shared MCP server.

use std::cell::RefCell;
use std::io::{self, Cursor, Write};
use std::rc::Rc;

use kit_mcp::{Server, Tool};
use serde_json::{Value, json};

/// A server with a `work` tool that reports three progress steps and then succeeds.
fn server() -> Server {
    let mut server = Server::new("kit-test", "1.2.3");
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

fn parse(lines: &[String]) -> Vec<Value> {
    lines
        .iter()
        .map(|line| serde_json::from_str(line).expect("valid JSON"))
        .collect()
}

fn work_call(id: Value, token: Option<Value>) -> String {
    let mut params = json!({ "name": "work", "arguments": {} });
    if let Some(token) = token {
        params["_meta"] = json!({ "progressToken": token });
    }
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params }).to_string()
}

#[test]
fn a_progress_token_makes_progress_notifications_before_the_reply() {
    let replies = parse(&server().handle_line(&work_call(json!(9), Some(json!("tok-1")))));
    assert_eq!(replies.len(), 4);
    assert_eq!(
        replies[0],
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": { "progressToken": "tok-1", "progress": 1.0, "total": 3.0, "message": "step one" },
        })
    );
    assert_eq!(
        replies[1]["params"],
        json!({ "progressToken": "tok-1", "progress": 2.0, "total": 3.0, "message": "step two" })
    );
    // No total and an empty message are left out.
    assert_eq!(
        replies[2]["params"],
        json!({ "progressToken": "tok-1", "progress": 3.0 })
    );
    assert!(replies[..3].iter().all(|n| n.get("id").is_none()));
    assert_eq!(
        replies[3],
        json!({
            "jsonrpc": "2.0",
            "id": 9,
            "result": { "content": [{ "type": "text", "text": "done" }] },
        })
    );
}

#[test]
fn an_integer_progress_token_is_sent_back_as_an_integer() {
    let replies = parse(&server().handle_line(&work_call(json!(1), Some(json!(42)))));
    assert_eq!(replies.len(), 4);
    assert_eq!(replies[0]["params"]["progressToken"], json!(42));
}

#[test]
fn without_a_progress_token_the_callback_sends_nothing() {
    let replies = parse(&server().handle_line(&work_call(json!(1), None)));
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["result"]["content"][0]["text"], "done");
}

#[test]
fn a_progress_token_of_the_wrong_type_is_ignored() {
    let replies = parse(&server().handle_line(&work_call(json!(1), Some(json!({ "x": 1 })))));
    assert_eq!(replies.len(), 1);
}

#[test]
fn progress_that_is_not_a_number_is_dropped() {
    let mut server = Server::new("kit-test", "1.0");
    server.add_tool(Tool::new(
        "nan",
        "d",
        json!({ "type": "object" }),
        |_, progress| {
            progress(f64::NAN, Some(1.0), "lost");
            progress(1.0, Some(f64::INFINITY), "kept without total");
            Ok(String::new())
        },
    ));
    let call = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "nan", "_meta": { "progressToken": 1 } },
    });
    let replies = parse(&server.handle_line(&call.to_string()));
    assert_eq!(replies.len(), 2);
    assert_eq!(
        replies[0]["params"],
        json!({ "progressToken": 1, "progress": 1.0, "message": "kept without total" })
    );
}

#[test]
fn handle_line_with_hands_over_each_line_while_the_tool_is_still_running() {
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let tool_log = Rc::clone(&log);
    let mut server = Server::new("kit-test", "1.0");
    server.add_tool(Tool::new(
        "slow",
        "d",
        json!({ "type": "object" }),
        move |_, progress| {
            tool_log
                .borrow_mut()
                .push("handler: before step 1".to_string());
            progress(1.0, None, "one");
            tool_log
                .borrow_mut()
                .push("handler: between steps".to_string());
            progress(2.0, None, "two");
            tool_log.borrow_mut().push("handler: finished".to_string());
            Ok("ok".to_string())
        },
    ));
    let call = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "slow", "_meta": { "progressToken": "t" } },
    });
    let emit_log = Rc::clone(&log);
    server.handle_line_with(&call.to_string(), &mut |line| {
        let value: Value = serde_json::from_str(&line).unwrap();
        let what = value["method"].as_str().map_or("response", |_| "progress");
        emit_log.borrow_mut().push(format!("emit: {what}"));
    });
    assert_eq!(
        *log.borrow(),
        [
            "handler: before step 1",
            "emit: progress",
            "handler: between steps",
            "emit: progress",
            "handler: finished",
            "emit: response",
        ]
    );
}

#[test]
fn serve_answers_several_lines_in_order() {
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "c", "version": "1" } },
    });
    let mut input = String::new();
    input.push_str(&format!("{init}\n"));
    input.push_str("{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n");
    input.push('\n');
    input.push_str("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\r\n");
    input.push_str(&format!("{}\n", work_call(json!(3), Some(json!("p")))));
    input.push_str("this is not json\n");
    input.push_str("{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"nope\"}\n");
    // The last line has no newline at the end.
    input.push_str("{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"ping\"}");

    let mut output = Vec::new();
    server().serve(Cursor::new(input), &mut output).unwrap();

    let text = String::from_utf8(output).unwrap();
    assert!(text.ends_with('\n'), "every reply ends with a newline");
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let replies = parse(&lines);
    // initialize, tools/list, 3 progress + call result, parse error, method not found, ping.
    assert_eq!(replies.len(), 9, "{text}");
    assert_eq!(replies[0]["id"], 1);
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(replies[1]["id"], 2);
    assert_eq!(replies[1]["result"]["tools"][0]["name"], "work");
    for progress in &replies[2..5] {
        assert_eq!(progress["method"], "notifications/progress");
    }
    assert_eq!(replies[5]["id"], 3);
    assert_eq!(replies[5]["result"]["content"][0]["text"], "done");
    assert_eq!(replies[6]["error"]["code"], -32700);
    assert_eq!(replies[6]["id"], Value::Null);
    assert_eq!(replies[7]["id"], 4);
    assert_eq!(replies[7]["error"]["code"], -32601);
    assert_eq!(
        replies[8],
        json!({ "jsonrpc": "2.0", "id": 5, "result": {} })
    );
}

#[test]
fn serve_survives_bytes_that_are_not_utf8() {
    let mut input = vec![0xff, 0xfe, b'\n'];
    input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n");
    let mut output = Vec::new();
    server().serve(Cursor::new(input), &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let replies = parse(&lines);
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["error"]["code"], -32700);
    assert_eq!(replies[1]["result"], json!({}));
}

#[test]
fn serve_with_no_input_writes_nothing() {
    let mut output = Vec::new();
    server()
        .serve(Cursor::new(Vec::new()), &mut output)
        .unwrap();
    assert!(output.is_empty());
}

/// A writer that counts flushes, to show each reply is flushed as it is written.
struct CountingWriter {
    bytes: Vec<u8>,
    flushes: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}

#[test]
fn serve_flushes_every_reply() {
    let mut writer = CountingWriter {
        bytes: Vec::new(),
        flushes: 0,
    };
    let input = format!(
        "{}\n{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}}\n",
        work_call(json!(1), Some(json!(1)))
    );
    server().serve(Cursor::new(input), &mut writer).unwrap();
    // Three progress notifications, the call result and the ping result.
    assert_eq!(writer.flushes, 5);
}

/// A writer that always fails.
struct BrokenWriter;

impl Write for BrokenWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn serve_returns_the_write_error() {
    let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n";
    let error = server()
        .serve(Cursor::new(input), BrokenWriter)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
}

#[test]
#[should_panic(expected = "Tool work is already registered")]
fn adding_two_tools_with_the_same_name_panics() {
    let mut server = server();
    server.add_tool(Tool::new(
        "work",
        "again",
        json!({ "type": "object" }),
        |_, _| Ok(String::new()),
    ));
}
