//! One field of one arena, as a column cut into fixed chunks that body versions share.
//!
//! What it does: a [`Column<K, V>`] holds one field (of type `V`) of every row of the arena of
//! `K`, in row order: all face boxes together, all fin links together (structure of arrays,
//! chapter 23.9). The column is cut into chunks of [`CHUNK_ROWS`] rows, each held behind a shared
//! pointer, so cloning a column (a new body version) copies only the list of chunks, and changing
//! a row copies only the chunk that holds it, if another version still shares it (copy on write).
//!
//! Guarantees:
//!
//! - **Fixed chunks, cut by row number only.** Row r is in chunk r / [`CHUNK_ROWS`] at place
//!   r mod [`CHUNK_ROWS`]. Every chunk but the last is full, and no chunk is empty, so where a
//!   chunk starts never depends on how the column was built or edited (23.9, 24.2).
//! - **Versions never see each other's edits.** After a clone, [`Column::set`] and
//!   [`Column::push`] on either column leave the other's values unchanged; a write copies the chunk
//!   it lands in when that chunk is shared, and only that chunk.
//! - **Values are all that count.** Equality, iteration and every answer depend on the rows'
//!   values in row order only. Which chunks are shared is a memory detail that [`Column::memory`]
//!   reports for measurement and never reaches a result (23.8, 23.9).
//! - **Large allocations are fallible.** A chunk's rows, and the list of chunks, are reserved with
//!   `try_reserve`, so a refusal returns [`TopoError::OutOfMemory`] instead of aborting (23.10).
//!   Only a chunk's small shared header is allocated infallibly, as the standard library's `Arc`
//!   allows no other way.
//! - **At most `u32::MAX` rows,** the most a 32-bit index addresses: one more is
//!   [`TopoError::Full`].
//!
//! Method: structure of arrays as Manifold stores its halfedges
//! (`projects/kit/notes/G01-evidence/01-manifold-mesh-booleans.md`, section 2), cut into the fixed
//! chunks of chapter 23.9 and shared between versions through `Arc` chunks, as report 03 advises
//! for immutable snapshots (`projects/kit/notes/G01-evidence/03-rust-kernels-crates.md`, section 5;
//! `Arc::make_mut`, <https://doc.rust-lang.org/alloc/sync/struct.Arc.html#method.make_mut>). No
//! vetted crate does this under `no_std` with fixed chunk boundaries: persistent vectors such as
//! imbl's and rpds's cut their trees where edits happened, which chapter 24.2 rules out, and only
//! `libm` may ship in a kernel crate (`deny-no-std.toml`).
//!
//! Cost: [`Column::get`] is constant time. [`Column::set`] is constant time, plus a copy of one
//! chunk (at most [`CHUNK_ROWS`] rows) when that chunk is shared. [`Column::push`] is amortised
//! constant time, plus at most one chunk copy. Cloning is linear in the number of chunks, one
//! pointer each. [`Column::filled`] allocates one chunk, however many rows it fills. Nothing
//! recurses.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use core::mem::size_of;

use crate::error::TopoError;
use crate::ix::{Ix, Kind};

/// The rows in every chunk but a column's last: 4,096, the run length chapter 24.2 cuts a body's
/// tables into, so chunks in memory and chunks in a file are the same rows.
pub const CHUNK_ROWS: u32 = 4096;

/// [`CHUNK_ROWS`] as a `usize`.
const CHUNK: usize = 4096;

/// One field of every row of the arena of `K`, in row order, cut into chunks shared between body
/// versions.
pub struct Column<K, V> {
    /// Every chunk but the last holds [`CHUNK`] rows; the last holds 1 to [`CHUNK`]; none is empty.
    chunks: Vec<Arc<Vec<V>>>,
    /// The number of rows.
    len: u32,
    kind: PhantomData<fn() -> K>,
}

/// What a column, an arena set or a body holds in memory, for measurement only (chapter 23.9).
///
/// `bytes` counts the heap memory the chunks reach: each chunk's rows (its capacity times the size
/// of a value), each chunk's shared header (two counters and the chunk's vector) and each column's
/// list of chunk pointers. A chunk shared with another version, or shared by several places of one
/// column, is counted at each place. Memory a value owns itself (a NURBS kind's control points) and
/// the allocator's own overhead are not counted. `unique_*` count only the chunks no other version
/// shares: after an edit, the chunks the edit copied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Memory {
    /// The chunks.
    pub chunks: u64,
    /// The chunks held by this column alone.
    pub unique_chunks: u64,
    /// The heap bytes the chunks reach, as above.
    pub bytes: u64,
    /// The heap bytes of the chunks held by this column alone.
    pub unique_bytes: u64,
}

impl Memory {
    /// The sum of two reports.
    #[must_use]
    pub fn plus(self, other: Memory) -> Memory {
        Memory {
            chunks: self.chunks.saturating_add(other.chunks),
            unique_chunks: self.unique_chunks.saturating_add(other.unique_chunks),
            bytes: self.bytes.saturating_add(other.bytes),
            unique_bytes: self.unique_bytes.saturating_add(other.unique_bytes),
        }
    }
}

/// A `usize` as a `u64` (every target the kernel builds for has `usize` of at most 64 bits).
fn wide(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The chunk `chunk` itself, copied first if another column or version shares it, with room for
/// `extra` more rows.
fn unique<V: Clone>(chunk: &mut Arc<Vec<V>>, extra: usize) -> Result<&mut Vec<V>, TopoError> {
    if Arc::get_mut(chunk).is_none() {
        let mut copy = Vec::new();
        copy.try_reserve_exact(chunk.len().saturating_add(extra))?;
        copy.extend_from_slice(chunk);
        *chunk = Arc::new(copy);
    }
    // The chunk is held here alone now, so this never copies.
    Ok(Arc::make_mut(chunk))
}

impl<K, V> Column<K, V> {
    /// An empty column.
    #[must_use]
    pub const fn new() -> Self {
        Column {
            chunks: Vec::new(),
            len: 0,
            kind: PhantomData,
        }
    }

    /// The number of rows.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.len
    }

    /// Whether the column has no rows.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of chunks: the rows divided by [`CHUNK_ROWS`], rounded up.
    #[must_use]
    pub const fn chunk_count(&self) -> u32 {
        self.len.div_ceil(CHUNK_ROWS)
    }

    /// The value at `ix`, or `None` past the last row.
    #[must_use]
    pub fn get(&self, ix: Ix<K>) -> Option<&V> {
        let row = ix.to_usize();
        self.chunks.get(row / CHUNK)?.get(row % CHUNK)
    }

    /// Every value, in row order.
    pub fn iter(&self) -> impl Iterator<Item = &V> + '_ {
        self.chunks.iter().flat_map(|chunk| chunk.iter())
    }

    /// Every chunk's values, in row order: chunk n holds rows n·[`CHUNK_ROWS`] onwards.
    pub fn chunks(&self) -> impl Iterator<Item = &[V]> + '_ {
        self.chunks.iter().map(|chunk| chunk.as_slice())
    }

    /// Takes from `other` each chunk at the same place that `same` says holds this column's rows,
    /// given the place and both chunks: the rows stay as they are and the two columns share the
    /// chunk (23.9).
    pub(crate) fn share_with(&mut self, other: &Self, same: impl Fn(usize, &[V], &[V]) -> bool) {
        for (place, (mine, theirs)) in self.chunks.iter_mut().zip(&other.chunks).enumerate() {
            if same(place, mine, theirs) {
                *mine = Arc::clone(theirs);
            }
        }
    }

    /// Whether chunk `chunk` of this column is the very chunk `other` holds there: then their rows
    /// are equal without reading them, and a new version of a body need not hash it again (23.8).
    /// `false` for a chunk either column lacks. A hint of equality only: equal rows in two chunks
    /// answer `false`. This is the kernel's one use of an address: it answers for clones of one
    /// chunk whatever the addresses are, and orders nothing.
    #[must_use]
    pub fn shares_chunk(&self, other: &Self, chunk: usize) -> bool {
        match (self.chunks.get(chunk), other.chunks.get(chunk)) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// What the column holds in memory; see [`Memory`]. A measurement only: which chunks are
    /// shared depends on which other versions are alive, never on the column's values.
    #[must_use]
    pub fn memory(&self) -> Memory {
        let header = 2_usize
            .saturating_mul(size_of::<usize>())
            .saturating_add(size_of::<Vec<V>>());
        let list = wide(
            self.chunks
                .capacity()
                .saturating_mul(size_of::<Arc<Vec<V>>>()),
        );
        let mut memory = Memory {
            chunks: 0,
            unique_chunks: 0,
            bytes: list,
            unique_bytes: list,
        };
        for chunk in &self.chunks {
            let bytes = wide(
                chunk
                    .capacity()
                    .saturating_mul(size_of::<V>())
                    .saturating_add(header),
            );
            let alone = Arc::strong_count(chunk) == 1;
            memory = memory.plus(Memory {
                chunks: 1,
                unique_chunks: u64::from(alone),
                bytes,
                unique_bytes: if alone { bytes } else { 0 },
            });
        }
        memory
    }
}

impl<K: Kind, V: Clone> Column<K, V> {
    /// A column of `rows` copies of `value`, every full chunk sharing one allocation until it is
    /// written to.
    ///
    /// # Errors
    ///
    /// [`TopoError::OutOfMemory`] if an allocation is refused.
    pub fn filled(value: V, rows: u32) -> Result<Self, TopoError> {
        let mut column = Column::new();
        let full = rows / CHUNK_ROWS;
        let last = rows % CHUNK_ROWS;
        let count = rows.div_ceil(CHUNK_ROWS) as usize;
        column.chunks.try_reserve_exact(count)?;
        if full > 0 {
            let mut chunk = Vec::new();
            chunk.try_reserve_exact(CHUNK)?;
            chunk.resize(CHUNK, value.clone());
            let chunk = Arc::new(chunk);
            column.chunks.resize(full as usize, chunk);
        }
        if last > 0 {
            let mut chunk = Vec::new();
            chunk.try_reserve_exact(last as usize)?;
            chunk.resize(last as usize, value);
            column.chunks.push(Arc::new(chunk));
        }
        column.len = rows;
        Ok(column)
    }

    /// Sets the value at `ix`, copying its chunk first if another version shares it.
    ///
    /// # Errors
    ///
    /// [`TopoError::NoSuchRow`] past the last row; [`TopoError::OutOfMemory`] if the copy's
    /// allocation is refused. On an error the column is unchanged.
    pub fn set(&mut self, ix: Ix<K>, value: V) -> Result<(), TopoError> {
        let row = ix.to_usize();
        let missing = TopoError::NoSuchRow {
            arena: K::ARENA,
            row: ix.row(),
        };
        let chunk = self.chunks.get_mut(row / CHUNK).ok_or(missing)?;
        if chunk.get(row % CHUNK).is_none() {
            return Err(missing);
        }
        unique(chunk, 0)?
            .get_mut(row % CHUNK)
            .map(|slot| *slot = value)
            .ok_or(missing)
    }

    /// Adds `value` as a new last row and returns its index.
    ///
    /// # Errors
    ///
    /// [`TopoError::Full`] if the column already holds `u32::MAX` rows;
    /// [`TopoError::OutOfMemory`] if an allocation is refused. On an error the column is unchanged.
    pub fn push(&mut self, value: V) -> Result<Ix<K>, TopoError> {
        let ix = Ix::new(self.len).ok_or(TopoError::Full(K::ARENA))?;
        match self.chunks.last_mut() {
            Some(last) if last.len() < CHUNK => {
                let rows = unique(last, 1)?;
                rows.try_reserve(1)?;
                rows.push(value);
            }
            _ => {
                let mut chunk = Vec::new();
                chunk.try_reserve(1)?;
                chunk.push(value);
                self.chunks.try_reserve(1)?;
                self.chunks.push(Arc::new(chunk));
            }
        }
        self.len = ix.row().saturating_add(1);
        Ok(ix)
    }

    /// Adds the values of `values` as new last rows, in order, reserving each chunk's rows exactly
    /// rather than doubling: the form a builder that knows its counts uses.
    ///
    /// Takes as many values as the iterator's length says at the start, and stops early if it ends
    /// sooner; the chunk rules hold either way.
    ///
    /// # Errors
    ///
    /// [`TopoError::Full`] if the rows would pass `u32::MAX`, before anything is added;
    /// [`TopoError::OutOfMemory`] if an allocation is refused, after which the column holds the
    /// rows added before the refusal.
    pub fn extend<I, J>(&mut self, values: I) -> Result<(), TopoError>
    where
        I: IntoIterator<Item = V, IntoIter = J>,
        J: ExactSizeIterator<Item = V>,
    {
        let mut values = values.into_iter();
        let mut left = values.len();
        if left > u32::MAX.saturating_sub(self.len) as usize {
            return Err(TopoError::Full(K::ARENA));
        }
        while left > 0 {
            let added = match self.chunks.last_mut() {
                Some(last) if last.len() < CHUNK => {
                    let take = CHUNK.saturating_sub(last.len()).min(left);
                    let rows = unique(last, take)?;
                    rows.try_reserve_exact(take)?;
                    let before = rows.len();
                    rows.extend(values.by_ref().take(take));
                    rows.len().saturating_sub(before)
                }
                _ => {
                    let take = left.min(CHUNK);
                    let mut chunk = Vec::new();
                    chunk.try_reserve_exact(take)?;
                    chunk.extend(values.by_ref().take(take));
                    if chunk.is_empty() {
                        break;
                    }
                    self.chunks.try_reserve(1)?;
                    let added = chunk.len();
                    self.chunks.push(Arc::new(chunk));
                    added
                }
            };
            if added == 0 {
                break;
            }
            left = left.saturating_sub(added);
            self.len = self
                .len
                .saturating_add(u32::try_from(added).unwrap_or(u32::MAX));
        }
        Ok(())
    }
}

impl<K, V> Default for Column<K, V> {
    fn default() -> Self {
        Column::new()
    }
}

impl<K, V> Clone for Column<K, V> {
    /// Shares every chunk: the copy costs one pointer per chunk, and either column's later writes
    /// copy the chunks they land in.
    fn clone(&self) -> Self {
        Column {
            chunks: self.chunks.clone(),
            len: self.len,
            kind: PhantomData,
        }
    }
}

impl<K, V: PartialEq> PartialEq for Column<K, V> {
    /// Equal when the rows' values are equal in row order, whatever memory either shares.
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl<K, V: Eq> Eq for Column<K, V> {}

impl<K, V: fmt::Debug> fmt::Debug for Column<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
