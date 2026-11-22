//! A fuzz target for the stored text. It runs as a property test in `cargo test`, and under
//! libFuzzer with `cargo bolero test stored_text_never_panics --package construct-walls`.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use bolero::check;
use construct_walls::Assembly;

/// Any text at all is refused or read as a wall type that writes back to exactly that text:
/// reading never panics and the stored form has one spelling.
#[test]
fn stored_text_never_panics() {
    check!().for_each(|bytes: &[u8]| {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return;
        };
        if let Ok(wall) = Assembly::from_text(text) {
            assert_eq!(wall.to_text().unwrap(), text);
        }
    });
}
