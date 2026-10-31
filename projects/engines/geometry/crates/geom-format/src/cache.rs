//! Cache keys and the reference memo store: an operation's result named by everything it depends
//! on, so that a hit is the same bits as running the operation again (23.8).
//!
//! What it does: [`CacheKey::new`] is the SHA-256 of the canonical encoding of `[edition, name,
//! version, path, precision, [input content IDs]]`: the kernel edition, the operation's name and
//! version from its schema, its `OpPath` (every name the operation creates holds it, 20.2), its
//! precision (a preview and a final result never share a key, 23.6) and the content ID of each
//! input, in the operation's order. [`CacheKey::name_free`] is the same without the path, for the
//! operations whose schema says their geometry does not depend on names; its inputs are then the
//! content IDs of their geometry without names. [`CacheEntry`] holds a result with the budget its
//! run had and what the run used. [`MemoStore`] is the caller's store of entries: bounded by bytes,
//! it forgets the entry used longest ago first, and it is a value the caller owns. [`restamp`]
//! turns a name-free entry into the entry of one call: it fills in the path and the input names
//! and recomputes every `NameId`, through a result's own [`Restamp`].
//!
//! Guarantees:
//!
//! - **A hit is the same bits as a fresh run.** [`CacheEntry::answer`] answers only when a fresh
//!   run under the caller's budget would give exactly the stored answer: a stored result, or a
//!   stored deterministic error, when the budget covers everything the run used (a run stops only
//!   when a charge passes its cap, and charges only add); a stored budget failure only under the
//!   same budget, since a smaller one can stop the run sooner with other numbers. Every other call
//!   misses, and the caller's own run gives the exact `BudgetExceeded`.
//! - **What is never stored.** [`CacheEntry::new`] refuses `Cancelled`, `OutOfMemory` and a
//!   batch's total: none is the operation's deterministic answer (23.10).
//! - **Name-free only where the schema says so.** [`CacheKey::name_free`] gives no key for an
//!   operation whose schema does not set `name_free`. A body enters such a key by
//!   `BodyVersion::geometry_id`.
//! - **A restamped hit is the same bits as a fresh run, names included.** Names go through
//!   `Stamp`, a body is published again by its new names, so its arenas take the new order. A
//!   stored error holds paths and names only the run knows, so [`restamp`] gives no entry for one:
//!   the call misses, and the caller's own run gives it.
//! - **One encoding.** A key is `geom-op`'s canonical encoding and SHA-256 (32.20), and its path is
//!   written by [`OpPath::encode`], as names write it. Every field changes the key; a key with a
//!   path and a name-free key differ in length, so they never collide by construction.
//! - **Bounded, and deterministic.** A [`MemoStore`] never holds more than its byte bound. Which
//!   entry it forgets depends only on the order of the calls made on it, never on time, addresses
//!   or a hash seed. It holds no global state (principle 7).
//!
//! Method: chapter 23.8's `CacheKey`, `CacheEntry` and memo store
//! (`projects/engines/geometry/architecture/23-performance.md`), with the content IDs of 24.2
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`) and SHA-256 by NIST
//! FIPS 180-4, <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>, through `geom_op::hash`. 23.8 fixes
//! only that the store is bounded by bytes and owned by the caller; forgetting the entry used
//! longest ago first is this piece's choice, deterministic because its clock counts calls.
//!
//! Cost: a key is one encoding and one SHA-256 pass over it: linear in the path's labels and the
//! number of inputs. A store lookup, insertion or eviction is O(log n) in the entries held.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use geom_op::cbor::{EncodeError, Encoder};
use geom_op::hash::sha256;
use geom_op::{
    ErrorKind, NameError, NameId, OpError, OpErrorDetail, OpOutput, OpPath, OpSchema, Precision,
    RunDetail, Stamp, TopoName,
};
use geom_run::{Budget, Usage, WorkMeter};
use geom_topo::order::{NameKey, Names};
use geom_topo::{Body, Builder, TopoError};

use crate::content::ContentId;

/// The key of one call of one operation (23.8): 32 bytes of SHA-256.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CacheKey([u8; 32]);

impl CacheKey {
    /// The key of `schema`'s operation called at `path` with `precision` on `inputs`, in the
    /// operation's order, by the kernel of `edition`.
    ///
    /// # Errors
    ///
    /// [`EncodeError`] as the encoder gives it: only for a path or input list too long to count.
    pub fn new(
        edition: u64,
        schema: &OpSchema,
        path: &OpPath,
        precision: Precision,
        inputs: &[ContentId],
    ) -> Result<CacheKey, EncodeError> {
        key(edition, schema, Some(path), precision, inputs)
    }

    /// The name-free key: as [`CacheKey::new`] without the path, called on `inputs`, the content
    /// IDs of its inputs' geometry without names (23.8). `None` unless `schema` says its geometry
    /// does not depend on names.
    ///
    /// # Errors
    ///
    /// As for [`CacheKey::new`].
    pub fn name_free(
        edition: u64,
        schema: &OpSchema,
        precision: Precision,
        inputs: &[ContentId],
    ) -> Result<Option<CacheKey>, EncodeError> {
        schema
            .name_free
            .then(|| key(edition, schema, None, precision, inputs))
            .transpose()
    }

    /// The key's 32 bytes.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Encodes and hashes a key; `path` is left out of a name-free key.
fn key(
    edition: u64,
    schema: &OpSchema,
    path: Option<&OpPath>,
    precision: Precision,
    inputs: &[ContentId],
) -> Result<CacheKey, EncodeError> {
    let mut e = Encoder::new();
    e.array(if path.is_some() { 6 } else { 5 })?;
    e.uint(edition)?;
    e.text(schema.name)?;
    e.uint(u64::from(schema.version))?;
    if let Some(path) = path {
        path.encode(&mut e)?;
    }
    match precision {
        Precision::Final => e.uint(0)?,
        Precision::Preview { bound, step } => {
            e.array(3)?;
            e.uint(1)?;
            e.uint(bound.get())?;
            e.uint(u64::from(step.quarter_nm()))?;
        }
    }
    e.array(u64::try_from(inputs.len()).map_err(|_| EncodeError::TooLong)?)?;
    for input in inputs {
        e.item(&input.item()?)?;
    }
    Ok(CacheKey(sha256(e.finish()?.as_bytes())))
}

/// What a store holds for one key: the operation's answer, the budget its run had, and what the
/// run used (23.8, 23.10).
#[derive(Clone, Debug, PartialEq)]
pub struct CacheEntry<R> {
    answer: Result<OpOutput<R>, OpError>,
    budget: Budget,
    used: Usage,
}

impl<R> CacheEntry<R> {
    /// The entry of a run under `budget` that used `used` and gave `answer`; `None` when the
    /// answer is not the operation's to store: `Cancelled`, `OutOfMemory`, or a batch's total.
    #[must_use]
    pub fn new(answer: Result<OpOutput<R>, OpError>, budget: Budget, used: Usage) -> Option<Self> {
        if let Err(error) = &answer {
            let batch = matches!(error.detail(), OpErrorDetail::Run(RunDetail::BatchTotal(_)));
            let fleeting = matches!(error.kind(), ErrorKind::Cancelled | ErrorKind::OutOfMemory);
            if batch || fleeting {
                return None;
            }
        }
        Some(CacheEntry {
            answer,
            budget,
            used,
        })
    }

    /// The stored answer, if a fresh run under `budget` would give exactly it; `None` otherwise,
    /// and the caller runs the operation.
    #[must_use]
    pub fn answer(&self, budget: Budget) -> Option<&Result<OpOutput<R>, OpError>> {
        let stopped = matches!(&self.answer, Err(e) if e.kind() == ErrorKind::BudgetExceeded);
        let fits = if stopped {
            budget == self.budget
        } else {
            self.used.work <= u128::from(budget.work)
                && self.used.logical_bytes <= u128::from(budget.logical_bytes)
        };
        fits.then_some(&self.answer)
    }

    /// What the run used.
    #[must_use]
    pub const fn used(&self) -> Usage {
        self.used
    }
}

/// Why a name-free entry could not be restamped.
#[derive(Clone, Debug, PartialEq)]
pub enum StampError {
    /// A name holds something other than blanks and names stamped before it.
    Name(NameError),
    /// A body's new names could not be published.
    Topo(TopoError),
}

impl fmt::Display for StampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StampError::Name(e) => write!(f, "a name could not be stamped: {e}"),
            StampError::Topo(e) => write!(f, "a restamped body could not be published: {e}"),
        }
    }
}

impl core::error::Error for StampError {}

impl From<NameError> for StampError {
    fn from(e: NameError) -> Self {
        StampError::Name(e)
    }
}

impl From<TopoError> for StampError {
    fn from(e: TopoError) -> Self {
        StampError::Topo(e)
    }
}

/// A result a name-free entry can hold: it makes itself again with every name stamped.
pub trait Restamp: Sized {
    /// This result as a run with `stamp`'s path and names would make it.
    ///
    /// # Errors
    ///
    /// A [`StampError`] for a name that is not blank, or names that cannot be published.
    fn restamp(&self, stamp: &mut Stamp, meter: &mut WorkMeter<'_>) -> Result<Self, StampError>;
}

/// A body and the names of what it holds, in the order they were made: every `NameKey` in the
/// body is the `NameId` of one of them or a name of the inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedBody {
    /// The names, each after those it refers to.
    pub names: Vec<TopoName>,
    /// The body, published by name.
    pub body: Body,
}

/// Each of `keys` stamped.
fn stamped_keys(keys: &[NameKey], stamp: &Stamp) -> Result<Vec<NameKey>, NameError> {
    keys.iter()
        .map(|key| Ok(NameKey(stamp.id(NameId::from_bytes(key.0))?.to_bytes())))
        .collect()
}

impl Restamp for NamedBody {
    fn restamp(&self, stamp: &mut Stamp, meter: &mut WorkMeter<'_>) -> Result<Self, StampError> {
        let names = self
            .names
            .iter()
            .map(|name| stamp.name(name))
            .collect::<Result<Vec<TopoName>, NameError>>()?;
        let empty = Names::default();
        let keys = self.body.names().unwrap_or(&empty);
        let keys = Names {
            regions: stamped_keys(&keys.regions, stamp)?,
            faces: stamped_keys(&keys.faces, stamp)?,
            edges: stamped_keys(&keys.edges, stamp)?,
            vertices: stamped_keys(&keys.vertices, stamp)?,
        };
        let builder = Builder {
            arenas: self.body.arenas().clone(),
        };
        let body = builder.publish_named(&keys, self.body.kind(), self.body.outside(), meter)?;
        Ok(NamedBody { names, body })
    }
}

/// 23.8's `restamp`: the entry a call at `path` on input names `inputs` would have stored, from
/// `entry`, a name-free entry whose names are blank; `None` for a stored error, which misses.
///
/// # Errors
///
/// A [`StampError`] if the result's names are not blank, or a warning refers to a name the result
/// does not hold.
pub fn restamp<R: Restamp>(
    entry: &CacheEntry<R>,
    path: &OpPath,
    inputs: &[NameId],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<CacheEntry<R>>, StampError> {
    let Ok(output) = &entry.answer else {
        return Ok(None);
    };
    let mut stamp = Stamp::new(path, inputs)?;
    let mut answer = OpOutput::new(output.result.restamp(&mut stamp, meter)?, output.exactness);
    for diagnostic in &output.diagnostics {
        let mut diagnostic = diagnostic.clone();
        for entity in &mut diagnostic.entities {
            *entity = stamp.id(*entity)?;
        }
        answer = answer.with_diagnostic(diagnostic);
    }
    Ok(Some(CacheEntry {
        answer: Ok(answer),
        budget: entry.budget,
        used: entry.used,
    }))
}

/// One entry held, with its size and when it was last used.
#[derive(Clone, Debug)]
struct Slot<R> {
    entry: CacheEntry<R>,
    bytes: u64,
    at: u128,
}

/// The reference memo store (23.8): entries by key, at most `bound` bytes of them by the sizes
/// their callers give, the entry used longest ago forgotten first. The caller creates and owns it.
#[derive(Clone, Debug)]
pub struct MemoStore<R> {
    bound: u64,
    held: u64,
    clock: u128,
    slots: BTreeMap<CacheKey, Slot<R>>,
    order: BTreeMap<u128, CacheKey>,
}

impl<R> MemoStore<R> {
    /// An empty store holding at most `bound` bytes.
    #[must_use]
    pub const fn new(bound: u64) -> Self {
        MemoStore {
            bound,
            held: 0,
            clock: 0,
            slots: BTreeMap::new(),
            order: BTreeMap::new(),
        }
    }

    /// The bytes the store may hold.
    #[must_use]
    pub const fn bound(&self) -> u64 {
        self.bound
    }

    /// The bytes it holds: the sum of its entries' sizes.
    #[must_use]
    pub const fn held(&self) -> u64 {
        self.held
    }

    /// How many entries it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The next use's time. A `u128` count never saturates: 2^128 uses are out of reach.
    fn tick(&mut self) -> u128 {
        self.clock = self.clock.saturating_add(1);
        self.clock
    }

    /// The answer stored for `key`, if [`CacheEntry::answer`] gives one under `budget`; a hit
    /// makes the entry the most recently used.
    pub fn get(&mut self, key: &CacheKey, budget: Budget) -> Option<&Result<OpOutput<R>, OpError>> {
        let now = self.tick();
        let slot = self.slots.get_mut(key)?;
        slot.entry.answer(budget)?;
        self.order.remove(&slot.at);
        slot.at = now;
        self.order.insert(now, *key);
        slot.entry.answer(budget)
    }

    /// Stores `entry` under `key` as `bytes` bytes, in place of any entry it had, forgetting the
    /// entries used longest ago until it fits. Returns `false`, storing nothing, when `bytes`
    /// alone is over the bound.
    pub fn insert(&mut self, key: CacheKey, entry: CacheEntry<R>, bytes: u64) -> bool {
        if bytes > self.bound {
            return false;
        }
        self.forget(&key);
        while self.bound.saturating_sub(self.held) < bytes {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some(slot) = self.slots.remove(&oldest) {
                self.held = self.held.saturating_sub(slot.bytes);
            }
        }
        let at = self.tick();
        self.order.insert(at, key);
        self.slots.insert(key, Slot { entry, bytes, at });
        self.held = self.held.saturating_add(bytes);
        true
    }

    /// Forgets `key`'s entry, if it holds one.
    pub fn forget(&mut self, key: &CacheKey) {
        if let Some(slot) = self.slots.remove(key) {
            self.order.remove(&slot.at);
            self.held = self.held.saturating_sub(slot.bytes);
        }
    }

    /// The keys held, the one used longest ago first.
    pub fn keys(&self) -> impl Iterator<Item = &CacheKey> {
        self.order.values()
    }
}
