//! The toolchain pins: `rust-toolchain.toml` and every workflow's `RUST_TOOLCHAIN` name one
//! compiler.
//!
//! What it does: reads the channel, components and targets of `rust-toolchain.toml`, and the
//! `RUST_TOOLCHAIN` line of each workflow that has one, and reports where they disagree.
//!
//! What it guarantees: the channel is an exact release (three numbers, never `stable`), so a
//! golden hash never moves because the compiler did (chapter 28, the toolchain guard); it lists
//! rustfmt and clippy and the two WebAssembly targets the kernel tests on; every workflow that pins
//! Rust pins that release, in exactly one `RUST_TOOLCHAIN` line, and installs it only through
//! `${{ env.RUST_TOOLCHAIN }}`; and the workflows that must pin it do. rustup lets the toolchain
//! file override the compiler a workflow installs, so a workflow pin that drifted would otherwise
//! be ignored without a word, and a toolchain change made in the file alone would not rerun
//! `kernel.yml`'s golden hashes: this check fails until all of them change together.
//!
//! Method: the `toml` crate, through `manifest`, for the toolchain file
//! (<https://docs.rs/toml/0.9.6/toml/>), and a line scan of each workflow.
//!
//! Cost: linear in the length of the files.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use crate::check::{Rule, Violation};
use crate::manifest::{keys, strings};

/// The toolchain file's name, at the repository root.
pub const TOOLCHAIN_FILE: &str = "rust-toolchain.toml";

/// The workflows that build or test the kernel, which must pin the toolchain: the merge gate's
/// (G12) among them.
pub const PINNED_WORKFLOWS: [&str; 3] = ["kernel.yml", "kernel-build.yml", "kernel-gate.yml"];

/// The components and targets the toolchain file must list.
const COMPONENTS: [&str; 2] = ["rustfmt", "clippy"];
const TARGETS: [&str; 2] = ["wasm32-wasip1", "wasm32-unknown-unknown"];

fn fault(file: &str, detail: String) -> Violation {
    Violation {
        krate: file.to_owned(),
        rule: Rule::ToolchainPin,
        detail,
    }
}

/// The text inside the first pair of quotes of a workflow's value, or the value itself when
/// unquoted.
fn unquote(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix(['"', '\''])
        .and_then(|v| v.strip_suffix(['"', '\'']))
        .unwrap_or(value)
}

/// Whether `channel` is an exact release such as `1.99.0`.
fn is_exact_release(channel: &str) -> bool {
    let parts: Vec<&str> = channel.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Checks the toolchain file against the workflows, given as (file name, text).
pub fn pins(toolchain_file: &str, workflows: &[(&str, &str)]) -> Vec<Violation> {
    let mut found = Vec::new();
    let Some(keys) = keys(toolchain_file) else {
        return vec![fault(TOOLCHAIN_FILE, "it is not TOML".to_owned())];
    };
    let channel = keys
        .get("toolchain.channel")
        .and_then(|c| c.as_str())
        .unwrap_or_default()
        .to_owned();
    if !is_exact_release(&channel) {
        found.push(fault(
            TOOLCHAIN_FILE,
            format!("channel {channel:?} is not an exact release such as \"1.99.0\""),
        ));
    }
    for (key, needed) in [
        ("toolchain.components", &COMPONENTS),
        ("toolchain.targets", &TARGETS),
    ] {
        let listed = keys.get(key).and_then(strings).unwrap_or_default();
        for item in needed.iter().filter(|item| !listed.iter().any(|l| l == *item)) {
            found.push(fault(TOOLCHAIN_FILE, format!("{key} lacks {item:?}")));
        }
    }
    for required in PINNED_WORKFLOWS {
        let pins_it = workflows
            .iter()
            .any(|(name, text)| *name == required && text.contains("RUST_TOOLCHAIN"));
        if !pins_it {
            found.push(fault(
                required,
                "this workflow must pin the toolchain in RUST_TOOLCHAIN".to_owned(),
            ));
        }
    }
    for (name, text) in workflows
        .iter()
        .filter(|(_, text)| text.contains("RUST_TOOLCHAIN"))
    {
        let pins: Vec<&str> = text
            .lines()
            .filter_map(|l| l.trim_start().strip_prefix("RUST_TOOLCHAIN:"))
            .map(unquote)
            .collect();
        if pins != [channel.as_str()] {
            found.push(fault(
                name,
                format!(
                    "RUST_TOOLCHAIN lines {pins:?}; there must be one, equal to {TOOLCHAIN_FILE}'s \
                     channel {channel:?}: change them together"
                ),
            ));
        }
        for line in text
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("toolchain:"))
        {
            if line != "toolchain: ${{ env.RUST_TOOLCHAIN }}" {
                found.push(fault(
                    name,
                    format!(
                        "{line:?}: install the pinned toolchain, ${{{{ env.RUST_TOOLCHAIN }}}}"
                    ),
                ));
            }
        }
    }
    found.sort();
    found
}
