//! A fuzz target for the stored form. It runs as a property test in `cargo test`, and under
//! libFuzzer with `cargo bolero test stored_bytes_never_panic --package construct-styles`.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use bolero::check;
use construct_styles::LineStyles;

/// Any bytes at all are refused or read as styles that write back to exactly those bytes: reading
/// never panics and a stored form has one spelling.
#[test]
fn stored_bytes_never_panic() {
    check!().for_each(|bytes: &[u8]| {
        if let Ok(styles) = LineStyles::from_bytes(bytes) {
            assert_eq!(styles.to_bytes().unwrap(), bytes);
        }
    });
}
