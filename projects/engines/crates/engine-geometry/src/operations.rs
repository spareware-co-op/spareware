//! Every operation the kernel offers, its schema held as data (chapter 22, "Every operation is
//! described as data").
//!
//! What it does: [`OPERATIONS`] lists one [`Operation`] per operation, in name order: its schema
//! and its tier. `geom-gen` writes the API documentation (`generated/api.md`) and the MCP tool
//! descriptions (`generated/mcp-tools.json`) from this list, and CI fails if a committed file has
//! not been regenerated after the list changed, so the interfaces never drift from the kernel and
//! no AI tool is written by hand (22; EXTENDING.md, section 2.2).
//!
//! Every operation piece adds its entry here as part of chapter 30.1's standard bar. The
//! operations of chapter 10.2's families come with phases 3 to 12 (the first, primitives and
//! transforms, with piece G101), so the list starts empty.
//!
//! Guarantees:
//!
//! - **One entry per operation, in name order.** `tests/attack_g100.rs` checks the names are
//!   unique, ascending, and valid MCP tool names, that each schema's version is at least 1, and
//!   that every preview's date is a real date.
//!
//! Method: none; a list of constants (`projects/engines/geometry/architecture/22-errors-queries-ai.md`).
//!
//! Cost: none at run time.

use crate::schema::Operation;

/// Every operation, in name order.
pub static OPERATIONS: &[Operation] = &[];
