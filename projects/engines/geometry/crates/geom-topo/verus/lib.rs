//! `geom-topo`'s Verus crate (chapter 28.7 of the kernel architecture, "Topology, by a model";
//! piece G111): a model of the Euler operators, with Verus's specifications and proofs, for `verus`
//! to verify. Cargo never builds it.
//!
//! - `euler`: loops as cyclic sequences of fin uses, and the operators that keep them closed.
//!
//! Verify: `verus --crate-type=lib projects/engines/geometry/crates/geom-topo/verus/lib.rs`, with
//! the Verus release `kernel-dev.yml` pins; its `verus` job does this on every pull request.

#![feature(proc_macro_hygiene)]

pub mod euler;
