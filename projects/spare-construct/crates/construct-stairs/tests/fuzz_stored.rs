//! Reader fuzz target, as construct-roofs' are: whatever a stair's recipe holds, reading it
//! answers with a stair or a refusal and never panics, and a stair it reads writes back the same.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_stairs::Stair;
use std::collections::BTreeMap;

#[test]
fn reading_stored_settings_never_panics() {
    bolero::check!().with_type::<String>().for_each(|text| {
        let Ok(args) = serde_json::from_str::<BTreeMap<String, serde_json::Value>>(text) else {
            return;
        };
        if let Ok(stair) = Stair::from_args(&args) {
            for (key, value) in stair.to_args() {
                assert_eq!(args.get(&key), Some(&value));
            }
        }
    });
}
