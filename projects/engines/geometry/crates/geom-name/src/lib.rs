//! Spare Kit geometry kernel: persistent names and references.
//!
//! One job: The name graph and its hash-consing; references, their resolution and the walk through
//! history; surface groups and placement references.
//!
//! Layer: 3, Topology (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every name is derived from the operation that made its entity and is hash-consed; nothing is
//!   named by position, a random suffix or a memory address (20.2).
//! - Everything is sorted by name, never by the order an algorithm reported it; ordinals follow
//!   `NameScheme` v1's one fixed order, and an order that cannot be decided fails with
//!   `UndecidedOrder`, never a guess (20.2).
//! - A reference resolves exactly, follows recorded history, or returns one value of the closed
//!   `Resolution` type with its reason; there is no geometric guessing (20.4).
//! - History that was pruned resolves to `HistoryPruned`, never `Deleted` (20.4, 24.4).
//! - Attribute classes are applied in one shared, pure step from the history; no operation writes
//!   attribute code of its own (8.4).
//!
//! Sections: 8.4, 17.12, 20 and 24.4, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G87, G89, G90, G109, G374.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
