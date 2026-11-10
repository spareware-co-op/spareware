//! Chunks: every table of a body is cut into fixed runs of 4,096 rows, so chunk boundaries are a
//! pure function of the table's length, never of how memory was shared (24.2).
//!
//! What it does: [`CHUNK_ROWS`] is the run length; [`chunk_count`] says how many chunks a table of
//! `rows` rows takes, and [`chunk`] gives the rows of one. Body records (G94) write one chunk record
//! per run and an index listing their content IDs.
//!
//! Guarantees:
//!
//! - **Fixed runs.** Every chunk but the last holds exactly [`CHUNK_ROWS`] rows; the last holds the
//!   rest, at least one; an empty table has no chunk. The chunks cover the table once, in order.
//! - **No panic, no overflow**, for every `usize` length.
//!
//! Method: chapter 24.2's chunk rule (4,096 rows, proposed and fixed by the specification) in
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`.
//!
//! Cost: constant time, no allocation.

use core::ops::Range;

/// The rows of every chunk but a table's last (24.2, proposed; 31's list of figures to measure).
pub const CHUNK_ROWS: usize = 4096;

/// How many chunks a table of `rows` rows takes.
#[must_use]
pub fn chunk_count(rows: usize) -> usize {
    rows.div_ceil(CHUNK_ROWS)
}

/// The rows of chunk `index` of a table of `rows` rows, or `None` past its last chunk.
#[must_use]
pub fn chunk(rows: usize, index: usize) -> Option<Range<usize>> {
    let start = index.checked_mul(CHUNK_ROWS)?;
    if start >= rows {
        return None;
    }
    Some(start..rows.min(start.saturating_add(CHUNK_ROWS)))
}
