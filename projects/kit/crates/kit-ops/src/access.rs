//! The permission check every operation goes through, as a question and an answer. A program
//! knows who belongs where and asks; a decider answers. The accounts service answers with Cedar.
//! The trait lives here so that a program's crates can ask without building Cedar, which is too
//! heavy for the crates compiled into a browser.

use serde_json::Value;

/// One question, in the shape Cedar takes. `entities` and `context` are Cedar's JSON forms, and
/// hold what the rows said at the moment of asking.
#[derive(Clone, Debug)]
pub struct Question {
    pub principal: (String, String),
    pub action: String,
    pub resource: (String, String),
    pub context: Value,
    pub entities: Value,
}

/// Answers questions.
pub trait Decider: Send + Sync + 'static {
    /// `Ok(true)` only for a clear yes. A question the rules can't read is an error.
    fn decide(&self, question: &Question) -> Result<bool, String>;
}
