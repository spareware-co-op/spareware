//! Spare Kit geometry kernel: threads for the parallel helper.
//!
//! One job: Threads for `geom-run`'s helper: rayon on native targets, Web Workers in a browser.
//!
//! Layer: 13, Facade (chapter 27's proposed layer table): it sits beside `engine-geometry`, which
//! reaches it only behind its `threads` feature.
//!
//! Invariants this crate keeps:
//!
//! - It only runs tasks: it implements `geom-run`'s `Pool` trait, and the fixed chunks, ordered
//!   results, ordered errors and order-free combines are the helper's, so every result is the same
//!   bits with this pool, on any number of threads, as with no pool at all (23.4, 21).
//! - Every task it is handed runs exactly once, on any thread and in any order, and the pool is
//!   re-entrant: a task may call the helper again without blocking a thread, so one pool serves a
//!   program's per-element work and an operation's inner work (23.4).
//! - It never panics, except to pass on a task's panic (23.4).
//! - In a browser it starts Web Workers only where shared memory is allowed; elsewhere the kernel
//!   runs serially, with the same answer (23.4, 29.4).
//! - It is the one `geom-*` crate that ships and may use `std`, because threads need it (the
//!   development crates use it too, and never ship); it still forbids unsafe code and carries
//!   `geom-num`'s lint posture (27).
//! - The only published crates it may name are rayon and, for Web Workers, wasm-bindgen-rayon, each
//!   by its row of section 29.4. It names rayon, on native targets only ([`native`], piece G33);
//!   wasm-bindgen-rayon waits for piece G456, and until then WebAssembly runs serially.
//!
//! Sections: 23.4 and 29.4; open question OQ-23.4; chapters 21 and 27 ("Proposed rule changes"), in
//! the kernel architecture (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Licences of the published crates it names, each record checked by the map (`depends`):
//!
//! - MIT OR Apache-2.0: `rayon`.
//!
//! Its name, folder and the facade's edge to it are chapter 27's, accepted by owner action G08
//! (section 32.13) in place of 23.4's `engine-geometry-pool` beside the facade in
//! `projects/engines/crates/`.
#![forbid(unsafe_code)]

#[cfg(not(target_family = "wasm"))]
pub mod native;
