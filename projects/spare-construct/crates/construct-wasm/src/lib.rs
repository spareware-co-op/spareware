//! Spare Construct Transport in the browser: the command layer as a WebAssembly module.
//!
//! JavaScript makes a `Session`, calls `session.command(name, paramsJson)` and gets the result as JSON text (or an exception
//! carrying the structured error), and lists every command with `Session.commands()`. The ids and clock come from the
//! browser (`Math.random` and `Date.now`), so the same commands that run on the command line and over MCP run in the page.

use construct_api::Session as ApiSession;
use js_sys::{Date, Error, Function, Math, Reflect};
use wasm_bindgen::prelude::*;

/// A Spare Construct Transport session: the open project, the active scenario, the results and the undo history.
#[wasm_bindgen]
pub struct Session {
    inner: ApiSession,
}

#[wasm_bindgen]
impl Session {
    /// A session with a new empty project.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Session {
        // runs are stamped, and timed, with the browser's clock
        construct_transport::util::set_clock(Date::now);
        Session {
            inner: ApiSession::new(Math::random, Date::now),
        }
    }

    /// Runs one command with its parameters as JSON text and gives the result as JSON text; an error is thrown as an `Error`
    /// whose message says what went wrong and which has `code` and `details` properties.
    pub fn command(&mut self, name: &str, params_json: &str) -> Result<String, JsValue> {
        let params: serde_json_lite::Parsed = serde_json_lite::parse(params_json);
        match params {
            Err(m) => Err(error_value("bad_params", &m, None)),
            Ok(p) => match self.inner.run(name, &p) {
                Ok(v) => Ok(v.to_string()),
                Err(e) => Err(error_value(
                    e.code,
                    &e.message,
                    e.details.as_ref().map(|d| d.to_string()),
                )),
            },
        }
    }

    /// Like `command`, but never throws: it gives `{"ok":true,"result":...}` or `{"ok":false,"error":{...}}` as JSON text.
    pub fn execute(&mut self, name: &str, params_json: &str) -> String {
        self.inner.execute(name, params_json)
    }

    /// Every command as JSON text: name, description, whether it changes the project and the JSON Schema of its parameters.
    pub fn commands() -> String {
        ApiSession::commands_json()
    }

    /// Sets a function called with each progress event as JSON text while a long command runs.
    #[wasm_bindgen(js_name = onProgress)]
    pub fn on_progress(&mut self, callback: Function) {
        self.inner.set_progress(move |ev| {
            let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(&ev.to_string()));
        });
    }

    /// Stops sending progress events.
    #[wasm_bindgen(js_name = clearProgress)]
    pub fn clear_progress(&mut self) {
        self.inner.clear_progress();
    }

    /// A number that goes up each time the project changes.
    pub fn revision(&self) -> f64 {
        self.inner.revision() as f64
    }
}

impl Default for Session {
    fn default() -> Self {
        Session::new()
    }
}

fn error_value(code: &str, message: &str, details: Option<String>) -> JsValue {
    let e = Error::new(message);
    let _ = Reflect::set(&e, &JsValue::from_str("code"), &JsValue::from_str(code));
    if let Some(d) = details {
        let _ = Reflect::set(&e, &JsValue::from_str("details"), &JsValue::from_str(&d));
    }
    e.into()
}

mod serde_json_lite {
    use serde_json::Value;

    /// The parameters as a JSON value; empty text is no parameters.
    pub type Parsed = Result<Value, String>;

    pub fn parse(text: &str) -> Parsed {
        if text.trim().is_empty() {
            return Ok(serde_json::json!({}));
        }
        serde_json::from_str(text).map_err(|e| format!("the parameters are not valid JSON: {e}"))
    }
}
