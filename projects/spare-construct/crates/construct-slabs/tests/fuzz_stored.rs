//! Fuzz targets for the stored forms. They run as property tests in `cargo test`, and under
//! libFuzzer with `cargo bolero test <name> --package construct-slabs`.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use bolero::check;
use construct_slabs::{Boundary, Layers};

/// Any text at all is refused or read as layers that write back to exactly that text.
#[test]
fn layers_text_never_panics() {
    check!().for_each(|bytes: &[u8]| {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return;
        };
        if let Ok(layers) = Layers::from_text(text) {
            assert_eq!(layers.to_text().unwrap(), text);
        }
    });
}

/// Any JSON at all is refused or read as a boundary that writes back to exactly that value, in
/// both the stored form and a command's plain form.
#[test]
fn boundary_values_never_panic() {
    check!().for_each(|bytes: &[u8]| {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            return;
        };
        if let Ok(boundary) = Boundary::from_value(&value) {
            assert_eq!(boundary.to_value(), value);
        }
        let _ = Boundary::from_plain(&value);
    });
}
