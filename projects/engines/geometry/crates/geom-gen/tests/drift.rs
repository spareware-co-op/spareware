//! The grid-steps table cannot silently miss a constant: every public constant of `geom-num`'s
//! `ladder.rs` and `grid.rs` is named in the generated table, as `` `ladder::NAME` `` or
//! `` `grid::NAME` ``.
//!
//! `geom-num` keeps no list of its ladder constants, so the generator names them by hand
//! (`grid_steps::LADDER`); this test reads `geom-num`'s source, so a constant added there without a
//! row here fails CI. Grid steps need no such test: the generator walks `GridStep::coarser` and
//! `GridStep::finer`, so a new step gets its row and columns by itself.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;

use geom_gen::grid_steps;

/// The names of the top-level `pub const` items (not `pub const fn`) of a source file.
fn public_constants(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|l| l.strip_prefix("pub const "))
        .filter(|rest| !rest.starts_with("fn "))
        .map(|rest| {
            rest.split(|c: char| c == ':' || c.is_whitespace())
                .next()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads geom-num's source from the host, which WASI cannot without a preopened folder"
)]
#[test]
fn every_ladder_and_grid_constant_is_in_the_table() {
    let table = String::from_utf8(grid_steps::generate().unwrap()).unwrap();
    let num_src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("geom-num")
        .join("src");
    let mut missing = Vec::new();
    let mut seen = 0;
    for (module, least) in [("ladder", 7), ("grid", 2)] {
        let source = std::fs::read_to_string(num_src.join(format!("{module}.rs"))).unwrap();
        let names = public_constants(&source);
        assert!(
            names.len() >= least,
            "{module}.rs: found only {names:?}; the scan is broken"
        );
        for name in names {
            seen += 1;
            if !table.contains(&format!("`{module}::{name}`")) {
                missing.push(format!("{module}::{name}"));
            }
        }
    }
    println!("{seen} constants named in the table");
    assert!(
        missing.is_empty(),
        "geom-num constants the grid-steps table does not name: {missing:?}; add them to \
         geom-gen/src/grid_steps.rs and run `cargo run -p geom-gen -- write`"
    );
}

#[test]
fn the_scan_reads_constants_and_skips_functions() {
    let src = "pub const A: u32 = 1;\npub const fn f() {}\n    pub const INNER: u8 = 0;\n\
               pub const B: Steps = Steps(2);\n";
    assert_eq!(public_constants(src), ["A", "B"]);
}
