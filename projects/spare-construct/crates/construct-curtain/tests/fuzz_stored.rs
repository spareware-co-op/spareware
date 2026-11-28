//! Fuzz targets for the stored forms. They run as property tests in `cargo test`, and under
//! libFuzzer with `cargo bolero test <name> --package construct-curtain`.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use bolero::check;
use construct_curtain::{CurtainSpec, Edits};

/// Any text at all is refused or read as a type's grid and parts that writes back to exactly that
/// text: reading never panics and the stored form has one spelling.
#[test]
fn stored_type_never_panics() {
    check!().for_each(|bytes: &[u8]| {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return;
        };
        if let Ok(spec) = CurtainSpec::from_text(text) {
            assert_eq!(spec.to_text().unwrap(), text);
        }
    });
}

/// Any JSON at all is refused or read as a wall's edits that write back to exactly that JSON.
#[test]
fn stored_edits_never_panic() {
    check!().for_each(|bytes: &[u8]| {
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            return;
        };
        if let Ok(edits) = Edits::from_json(Some(&json)) {
            assert_eq!(edits.to_json().unwrap(), Some(json));
        }
    });
}
