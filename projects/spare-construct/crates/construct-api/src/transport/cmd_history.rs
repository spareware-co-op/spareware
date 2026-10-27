//! Undo and redo.

use super::registry::Registry;
use crate::schema::*;
use serde_json::json;

pub(super) fn register(r: &mut Registry) {
    r.add(
        "transport.history.undo",
        "Undo the latest change, switching to its scenario if it was in another; the labels say what each step was.",
        true,
        none(),
        |s, _| s.step_back(false),
    );
    r.add(
        "transport.history.redo",
        "Redo the step the last undo took back.",
        true,
        none(),
        |s, _| s.step_back(true),
    );
    r.add(
        "transport.history.state",
        "The undo and redo steps that can be taken, oldest first for undo, with their labels.",
        false,
        none(),
        |s, _| {
            let (u, r) = s.step_labels();
            Ok(json!({ "undo": u, "redo": r }))
        },
    );
}
