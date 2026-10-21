//! Errors a command gives back: always structured, never a panic.

use serde_json::{Value, json};
use std::fmt;

/// What went wrong with a command: a stable code, a sentence for a person and optional details.
#[derive(Clone, Debug, PartialEq)]
pub struct ApiError {
    /// Stable machine-readable code: unknown_command, bad_params, not_found, invalid_state, engine_error or internal.
    pub code: &'static str,
    /// What happened and what to do about it, in plain English.
    pub message: String,
    /// More facts about the error (for example the allowed values), when there are any.
    pub details: Option<Value>,
}

impl ApiError {
    /// An error with this code and message.
    pub fn new(code: &'static str, message: impl Into<String>) -> ApiError {
        ApiError {
            code,
            message: message.into(),
            details: None,
        }
    }
    /// The command name is not in the registry.
    pub fn unknown_command(name: &str) -> ApiError {
        ApiError::new(
            "unknown_command",
            format!("There is no command called {name}. List the commands to see every name."),
        )
    }
    /// The parameters are missing, of the wrong type or out of range.
    pub fn bad_params(message: impl Into<String>) -> ApiError {
        ApiError::new("bad_params", message)
    }
    /// A scenario, road, junction, zone, route or other thing named in the parameters does not exist.
    pub fn not_found(message: impl Into<String>) -> ApiError {
        ApiError::new("not_found", message)
    }
    /// The project is not in a state the command can work from (for example no results yet).
    pub fn invalid_state(message: impl Into<String>) -> ApiError {
        ApiError::new("invalid_state", message)
    }
    /// The calculation engine refused the edit or the input.
    pub fn engine(message: impl Into<String>) -> ApiError {
        ApiError::new("engine_error", message)
    }
    /// Adds details to the error.
    pub fn with_details(mut self, details: Value) -> ApiError {
        self.details = Some(details);
        self
    }
    /// The error as JSON: { code, message, details? }.
    pub fn to_value(&self) -> Value {
        match &self.details {
            Some(d) => json!({ "code": self.code, "message": self.message, "details": d }),
            None => json!({ "code": self.code, "message": self.message }),
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<construct_transport::edit::EditError> for ApiError {
    fn from(e: construct_transport::edit::EditError) -> ApiError {
        ApiError::engine(e.0)
    }
}

/// The result type of every command.
pub type ApiResult<T = Value> = Result<T, ApiError>;
