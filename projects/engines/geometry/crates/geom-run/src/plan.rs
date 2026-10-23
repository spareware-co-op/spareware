//! Chunk plans: where a problem of a given size is cut into chunks.
//!
//! A [`ChunkPlan`] holds one number, the chunk size, chosen as a constant at each call site for the
//! cost of one item (for example 64 items for boolean cells and 4,096 for point evaluations).
//! Chunk `i` of a problem of `len` items covers items `i · size .. min((i + 1) · size, len)`.
//! Nothing else enters: not the thread count, the core count, cache sizes or timing. So every
//! executor, on every machine, cuts the same problem at the same places (architecture chapter
//! 23.4, "Fixed chunks").
//!
//! Method: fixed-size static chunking, the boundaries of Rust's own [`slice::chunks`]
//! (<https://doc.rust-lang.org/core/primitive.slice.html#method.chunks>) and of the chunk-size form
//! of OpenMP's worksharing-loop `schedule` clause (OpenMP API 5.2, section 11.5.3,
//! <https://www.openmp.org/spec-html/5.2/openmp.html>). The tests check the boundaries against
//! `slice::chunks` for every size and length they try.
//!
//! Cost: a plan is one machine word. [`ChunkPlan::count`], [`ChunkPlan::chunk`] and each step of
//! [`Chunks`] are a fixed number of integer operations; nothing allocates.
//!
//! Invariants:
//!
//! - Every chunk holds at least one item; only the last chunk of a problem may hold fewer than the
//!   chunk size.
//! - Chunks are contiguous, in index order, and together cover every item exactly once.
//! - No arithmetic here can overflow, whatever the length and size, including `usize::MAX`.

use core::num::NonZeroUsize;
use core::ops::Range;

/// How a problem is cut into chunks: a fixed chunk size, never derived from the thread count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkPlan {
    size: NonZeroUsize,
}

impl ChunkPlan {
    /// A plan whose chunk size is the compile-time constant `N`, for the constant each call site
    /// chooses: `const PLAN: ChunkPlan = ChunkPlan::fixed::<64>();`. `N = 0` does not compile.
    #[must_use]
    pub const fn fixed<const N: usize>() -> Self {
        const { assert!(N > 0, "a chunk holds at least one item") };
        let size = match NonZeroUsize::new(N) {
            Some(size) => size,
            // Never taken: the assertion above rejects `N = 0` at compile time.
            None => NonZeroUsize::MIN,
        };
        Self { size }
    }

    /// A plan with the given chunk size, for sizes known only at run time (such as tests that force
    /// chunk sizes of 1, 7 and 4,096, chapter 23.4).
    #[must_use]
    pub const fn new(size: NonZeroUsize) -> Self {
        Self { size }
    }

    /// The number of items in every chunk but possibly the last.
    #[must_use]
    pub const fn size(self) -> NonZeroUsize {
        self.size
    }

    /// The number of chunks for a problem of `len` items: `len / size` rounded up, so 0 for an
    /// empty problem.
    #[must_use]
    pub const fn count(self, len: usize) -> usize {
        len.div_ceil(self.size.get())
    }

    /// Chunk `index` of a problem of `len` items, or `None` when the problem has no such chunk
    /// (`index >= self.count(len)`).
    #[must_use]
    pub const fn chunk(self, len: usize, index: usize) -> Option<Chunk> {
        let size = self.size.get();
        let Some(start) = index.checked_mul(size) else {
            return None;
        };
        if start >= len {
            return None;
        }
        // `end = min(start + size, len)`, exactly: when `start + size` passes `usize::MAX`, the
        // true sum is above `len` too, so the saturated sum gives `len` as the true one would.
        let past = start.saturating_add(size);
        let end = if past < len { past } else { len };
        Some(Chunk { index, start, end })
    }

    /// Every chunk of a problem of `len` items, in index order.
    #[must_use]
    pub const fn chunks(self, len: usize) -> Chunks {
        Chunks {
            plan: self,
            len,
            next: 0,
            count: self.count(len),
        }
    }
}

/// One chunk of a problem: its index and the range of item indices it covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk {
    index: usize,
    start: usize,
    end: usize,
}

impl Chunk {
    /// The chunk's position among the problem's chunks, from 0.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    /// The index of the chunk's first item.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// One past the index of the chunk's last item.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    /// The number of items in the chunk; at least 1 for every chunk a plan makes.
    #[must_use]
    pub const fn len(self) -> usize {
        // Exact: only `ChunkPlan::chunk` makes a chunk, always with `start < end`, so this never
        // saturates.
        self.end.saturating_sub(self.start)
    }

    /// Whether the chunk holds no items. Never true for a chunk a plan makes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// The item indices the chunk covers, `start..end`.
    #[must_use]
    pub const fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

/// The chunks of a problem in index order, from [`ChunkPlan::chunks`].
#[derive(Clone, Debug)]
pub struct Chunks {
    plan: ChunkPlan,
    len: usize,
    next: usize,
    count: usize,
}

impl Iterator for Chunks {
    type Item = Chunk;

    fn next(&mut self) -> Option<Chunk> {
        let chunk = self.plan.chunk(self.len, self.next)?;
        // A chunk exists, so `index < count <= usize::MAX`: this never saturates.
        self.next = chunk.index.saturating_add(1);
        Some(chunk)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.count.saturating_sub(self.next);
        (left, Some(left))
    }
}

impl ExactSizeIterator for Chunks {}

impl core::iter::FusedIterator for Chunks {}
