//! A fuzz target for the object styles' stored form. It runs as a property test in `cargo test`,
//! and under libFuzzer with `cargo bolero test object_style_bytes_never_panic --package
//! construct-styles`.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use bolero::check;
use construct_categories::CategoryTable;
use construct_styles::{LineStyles, ObjectStyles};

/// Any bytes at all are refused or read as a table that writes back to exactly those bytes:
/// reading never panics and a stored form has one spelling.
#[test]
fn object_style_bytes_never_panic() {
    let categories = CategoryTable::new();
    let lines = LineStyles::default_metric();
    check!().for_each(|bytes: &[u8]| {
        if let Ok(styles) = ObjectStyles::from_bytes(bytes, &categories, &lines) {
            assert_eq!(styles.to_bytes().unwrap(), bytes);
        }
    });
}
