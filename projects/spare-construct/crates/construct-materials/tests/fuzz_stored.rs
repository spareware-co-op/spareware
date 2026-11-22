//! A fuzz target for the stored form. It runs as a property test in `cargo test`, and under
//! libFuzzer with `cargo bolero test stored_bytes_never_panic --package construct-materials`.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use bolero::check;
use construct_materials::Materials;

/// Any bytes at all are refused or read as a library that writes back to exactly those bytes:
/// reading never panics and a stored form has one spelling.
#[test]
fn stored_bytes_never_panic() {
    check!().for_each(|bytes: &[u8]| {
        if let Ok(library) = Materials::from_bytes(bytes) {
            assert_eq!(library.to_bytes().unwrap(), bytes);
        }
    });
}
