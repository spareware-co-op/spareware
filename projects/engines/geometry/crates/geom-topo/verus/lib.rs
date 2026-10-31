//! `geom-topo`'s Verus crate (chapter 28.7 of the kernel architecture, "Topology, by a model";
//! piece G111): a model of the arenas and Euler operators, with Verus's specifications and proofs,
//! for `verus` to verify. Cargo never builds it.
//!
//! - `rings`: fins in cycles, and the moves that keep them cycles.
//! - `euler`: a body's loops and vertex fans as rings, and the operators, each proved to keep every
//!   fin's partner, every loop and Euler's formula.
//!
//! Verify: `verus --crate-type=lib projects/engines/geometry/crates/geom-topo/verus/lib.rs`, with
//! the Verus release `kernel-dev.yml` pins; its `verus` job does this on every pull request.

#![feature(proc_macro_hygiene)]

pub mod euler;
pub mod rings;
