//! Reader fuzz targets, as construct-stairs' are: whatever a railing's recipe or a rule set holds,
//! reading it answers with a value or a refusal and never panics, and a railing it reads writes
//! back the same.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_railings::{Railing, RuleSet};
use std::collections::BTreeMap;

#[test]
fn reading_stored_settings_never_panics() {
    bolero::check!().with_type::<String>().for_each(|text| {
        let Ok(args) = serde_json::from_str::<BTreeMap<String, serde_json::Value>>(text) else {
            return;
        };
        if let Ok(railing) = Railing::from_args(&args) {
            assert_eq!(railing.to_args(), args);
        }
    });
}

#[test]
fn reading_a_rule_set_never_panics() {
    bolero::check!().with_type::<String>().for_each(|text| {
        let _ = RuleSet::from_json(text);
    });
}
