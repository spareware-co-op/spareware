//! Fuzz target: `geom-format`'s memo store keeps to its bound, forgets the entry used longest ago
//! first, and answers a call only when the caller's budget covers what the stored run used.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a byte bound and up to
//! [`MAX_OPS`] calls on one store (insert an entry of some size whose run used some work, get under
//! some work budget, forget), each on a key from a small alphabet so that keys repeat. [`check`]
//! runs the calls on a `MemoStore` and, beside it, on a list kept in order of use, written here from
//! chapter 23.8's rule; after every call it compares the answer, the keys in order of use, the
//! bytes held and the count. [`check_with`] takes the rule that picks the entry to forget, so a
//! test shows a wrong rule is caught. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing against chapter 23.8's memo store read independently
//! (`projects/engines/geometry/architecture/23-performance.md`): bounded by bytes, a hit only
//! where a fresh run gives the same answer, the entry used longest ago forgotten first.
//!
//! Cost: at most [`MAX_OPS`] calls on a store of at most [`MAX_KEYS`] entries, each checked
//! against a list of at most as many; at most [`MAX_INPUT_BYTES`] bytes read.

use geom_format::cache::{CacheEntry, CacheKey, MemoStore};
use geom_format::content::ContentId;
use geom_num::Exactness;
use geom_op::{Class, Family, OpError, OpId, OpOutput, OpPath, OpSchema, Precision};
use geom_run::{Budget, Usage};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most calls a case makes.
pub const MAX_OPS: u8 = 24;

/// The keys a case's calls draw from.
pub const MAX_KEYS: u8 = 6;

/// The most bytes [`decode`] reads from any input: 98.
///
/// The bound byte and the call count: 2. Each of at most [`MAX_OPS`] calls, its kind, its key and
/// two argument bytes: 4. So 2 + 24 · 4 = 98. The test `the_input_bound_is_reached_and_never_passed`
/// builds that input.
pub const MAX_INPUT_BYTES: usize = 2 + 24 * 4;

const _: () = assert!(MAX_INPUT_BYTES == 98);

/// One call on the store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Store an entry of `value`, `bytes` in size, whose run used `work` units.
    Insert {
        /// The key, an index into the case's keys.
        key: u8,
        /// The answer stored.
        value: u32,
        /// Its size.
        bytes: u64,
        /// The work its run used.
        work: u64,
    },
    /// Ask for `key`'s answer under a budget of `work` units.
    Get {
        /// The key.
        key: u8,
        /// The caller's work budget.
        work: u64,
    },
    /// Forget `key`'s entry.
    Forget {
        /// The key.
        key: u8,
    },
}

/// One case: a store's bound and the calls made on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The store's byte bound, 0 to 31.
    pub bound: u64,
    /// The calls, in order.
    pub ops: Vec<Op>,
}

/// Decodes `data` into a case, and returns it with the number of bytes read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let bound = u64::from(src.byte() % 32);
    let count = src.byte() % (MAX_OPS + 1);
    let mut ops = Vec::new();
    for _ in 0..count {
        let kind = src.byte();
        let key = src.byte() % MAX_KEYS;
        let (a, b) = (src.byte(), src.byte());
        ops.push(match kind % 4 {
            0 | 1 => Op::Insert {
                key,
                value: u32::from(a),
                bytes: u64::from(b % 16),
                work: u64::from(b / 16),
            },
            2 => Op::Get {
                key,
                work: u64::from(a % 16),
            },
            _ => Op::Forget { key },
        });
    }
    let read = data.len().saturating_sub(src.remaining());
    (Case { bound, ops }, read)
}

/// The harness every fuzzing engine drives: decode `data` into a case, then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// One entry of the reference list: key, answer, size, work used.
pub type Held = (u8, u32, u64, u64);

/// The bytes the list holds, added in its order.
fn total(list: &[Held]) -> u64 {
    list.iter().fold(0, |sum, h| sum.saturating_add(h.2))
}

/// Checks the store against chapter 23.8's rule: forget the entry used longest ago first.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    check_with(case, |_| 0)
}

const SCHEMA: OpSchema = OpSchema {
    name: "fuzz.cache",
    version: 1,
    family: Family::Make,
    doc: "The cache target's operation.",
    inputs: &[],
    outputs: &[],
    roles: &[],
    errors: &[],
    diagnostics: &[],
    classes: &[Class::Exact],
    charges: &[],
    batch: false,
    name_free: false,
    example: "none",
};

/// The key of the case's key `i`: one operation, input `i`.
fn key(i: u8) -> Result<CacheKey, Finding> {
    let path = OpPath::new(OpId::from_bytes([1; 16]));
    CacheKey::new(
        0,
        &SCHEMA,
        &path,
        Precision::Final,
        &[ContentId::of_payload(&[i])],
    )
    .map_err(|e| Finding::new("a key encodes", || format!("{e}")))
}

/// [`check`] with `forget` in place of the rule: given the list in order of use, it says which
/// entry goes first. How a test shows that a wrong rule is caught.
///
/// # Errors
///
/// As for [`check`].
pub fn check_with<F>(case: &Case, forget: F) -> Result<(), Finding>
where
    F: Fn(&[Held]) -> usize,
{
    let keys: Vec<CacheKey> = (0..MAX_KEYS).map(key).collect::<Result<_, _>>()?;
    let at = |k: u8| keys.get(usize::from(k)).copied();
    let mut store: MemoStore<u32> = MemoStore::new(case.bound);
    let mut list: Vec<Held> = Vec::new();
    for (step, op) in case.ops.iter().enumerate() {
        let broke =
            |law: &'static str| Finding::new(law, || format!("call {step}, {op:?}; case {case:?}"));
        match *op {
            Op::Insert {
                key,
                value,
                bytes,
                work,
            } => {
                let k = at(key).ok_or_else(|| broke("keys are in the alphabet"))?;
                let entry = CacheEntry::new(
                    Ok(OpOutput::new(value, Exactness::Exact)),
                    Budget::UNLIMITED,
                    Usage::new(u128::from(work), 0),
                )
                .ok_or_else(|| broke("a result is stored"))?;
                let stored = store.insert(k, entry, bytes);
                if stored != (bytes <= case.bound) {
                    return Err(broke("an entry is stored exactly when it fits the bound"));
                }
                if stored {
                    list.retain(|h| h.0 != key);
                    while case.bound.saturating_sub(total(&list)) < bytes {
                        let i = forget(&list);
                        if i >= list.len() {
                            break;
                        }
                        list.remove(i);
                    }
                    list.push((key, value, bytes, work));
                }
            }
            Op::Get { key, work } => {
                let k = at(key).ok_or_else(|| broke("keys are in the alphabet"))?;
                let got = store
                    .get(&k, Budget::new(work, u64::MAX))
                    .map(|a| a.as_ref().map(|o| o.result).map_err(OpError::kind));
                let found = list.iter().position(|h| h.0 == key);
                let want = match found {
                    Some(i) if list.get(i).is_some_and(|h| h.3 <= work) => {
                        let h = list.remove(i);
                        list.push(h);
                        Some(Ok(h.1))
                    }
                    _ => None,
                };
                if got != want {
                    return Err(broke(
                        "a get answers the stored value exactly when the budget covers its work",
                    ));
                }
            }
            Op::Forget { key } => {
                let k = at(key).ok_or_else(|| broke("keys are in the alphabet"))?;
                store.forget(&k);
                list.retain(|h| h.0 != key);
            }
        }
        let order: Vec<CacheKey> = store.keys().copied().collect();
        let want: Option<Vec<CacheKey>> = list.iter().map(|h| at(h.0)).collect();
        if Some(order) != want {
            return Err(broke("the store's keys are in order of use"));
        }
        let held: u64 = total(&list);
        if store.held() != held || store.len() != list.len() || held > case.bound {
            return Err(broke("the store holds what it was given, within its bound"));
        }
    }
    Ok(())
}
