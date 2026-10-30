//! The ordinal order of `NameScheme v1`: how entities that would share a name are told apart by
//! `Weak` names, and the one order their ordinals follow.
//!
//! What it does: when an operation's names still collide, the colliding entities share the
//! collided name as `base` and take `Weak { base, ord }` names, the `n`-th of `of` (chapter 20.2).
//! [`ordinals`] puts the entities in 20.2's order and gives each its [`Ordinal`]; [`weak_names`]
//! makes their `Weak` names. The order is the rule's table, step by step:
//!
//! 1. **Reference**: what the entities are ordered on. [`reference`] picks, from the parent faces
//!    and edges every colliding entity touches, the one whose `NameId` sorts lowest; with none,
//!    the body's own frame. The operation computes each entity's keys on it.
//! 2. **Key**: each entity's least point on the reference ([`WeakOrder::least_point`]).
//! 3. **Ties**: entities sharing their least point are compared by their vertices, each list
//!    sorted the same way, like words in a dictionary ([`WeakOrder::vertices`]), then in a
//!    parameter space by the direction each leaves the shared point
//!    ([`WeakOrder::direction`]).
//! 4. **Comparison**: exact. A comparison the operation cannot decide at section 12.3's floor is
//!    `None`, and a tie still standing after the three steps is not an order; either fails with
//!    [`OrderError::UndecidedOrder`], never a guess.
//!
//! Guarantees:
//!
//! - **The order is the entities', not the input's.** Any permutation of the same entities gives
//!   each entity the same ordinal: the order depends only on the keys the operation computes, never
//!   on the order an algorithm reported its results in (20.2).
//! - **Ordinals are a permutation of `0..of`**, with `of` the number of entities, which must fit
//!   in a `u32` (20.2).
//! - **Undecided or tied is an error.** If any comparison the sort makes is undecided, or two
//!   entities compare equal on every step, the result is [`OrderError::UndecidedOrder`] naming two
//!   such entities by their positions in the input.
//! - **Deterministic and bounded.** A bottom-up merge sort, stable and without recursion, makes at
//!   most about n log2 n comparisons.
//!
//! Method: chapter 20.2's ordinal order (the rule's four steps),
//! `projects/engines/geometry/architecture/20-naming-history.md`; bottom-up merge sort, Knuth, *The
//! Art of Computer Programming*, vol. 3, section 5.2.4 (straight two-way merging,
//! <https://www-cs-faculty.stanford.edu/~knuth/taocp.html>).
//!
//! Cost: O(n log n) comparisons and two buffers of n (position, reference) pairs. Recursion: none.

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use geom_op::NameId;
use geom_op::name::{NameSet, Ordinal, TopoName};

/// Why the colliding entities could not be given ordinals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderError {
    /// Two entities, by their positions in the input, whose order could not be decided exactly, or
    /// that tie on every step of the rule (20.2).
    UndecidedOrder(usize, usize),
    /// More entities than a `u32` ordinal counts.
    TooMany,
}

impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderError::UndecidedOrder(a, b) => {
                write!(f, "the order of entities {a} and {b} cannot be decided")
            }
            OrderError::TooMany => f.write_str("more colliding entities than a u32 counts"),
        }
    }
}

impl core::error::Error for OrderError {}

/// What an operation computes for each colliding entity on the reference: the three comparisons of
/// 20.2's rule. Each returns `None` when it cannot be decided exactly.
pub trait WeakOrder {
    /// Compares the entities' least points on the reference: first parameter, then second (in a
    /// frame: x, then y, then z).
    fn least_point(&self, other: &Self) -> Option<Ordering>;

    /// Compares the entities' vertex lists, each sorted the same way, like words in a dictionary.
    fn vertices(&self, other: &Self) -> Option<Ordering>;

    /// Compares the directions in which the entities leave a shared least point, as section 11.7
    /// decides radial order.
    fn direction(&self, other: &Self) -> Option<Ordering>;
}

/// The order of `a` and `b` under 20.2's rule: least point, then vertices, then direction.
/// `None` if a step is undecided or the entities tie on all three.
fn compare<K: WeakOrder>(a: &K, b: &K) -> Option<Ordering> {
    let steps: [fn(&K, &K) -> Option<Ordering>; 3] = [
        WeakOrder::least_point,
        WeakOrder::vertices,
        WeakOrder::direction,
    ];
    for step in steps {
        match step(a, b)? {
            Ordering::Equal => {}
            decided => return Some(decided),
        }
    }
    None
}

/// The ordinal of each entity, in input order: its place in 20.2's order, of the number of
/// entities.
///
/// # Errors
///
/// [`OrderError::UndecidedOrder`] if a comparison is undecided or two entities tie, and
/// [`OrderError::TooMany`] for more than `u32::MAX` entities.
pub fn ordinals<K: WeakOrder>(entities: &[K]) -> Result<Vec<Ordinal>, OrderError> {
    let of = u32::try_from(entities.len()).map_err(|_| OrderError::TooMany)?;
    let mut placed: Vec<(usize, Ordinal)> = sorted(entities)?
        .into_iter()
        .zip(Ordinal::all(of))
        .collect();
    placed.sort_unstable_by_key(|&(entity, _)| entity);
    Ok(placed.into_iter().map(|(_, ord)| ord).collect())
}

/// The `Weak` name of each entity, in input order: `base` and its ordinal.
///
/// # Errors
///
/// As [`ordinals`].
pub fn weak_names<K: WeakOrder>(base: NameId, entities: &[K]) -> Result<Vec<TopoName>, OrderError> {
    Ok(ordinals(entities)?
        .into_iter()
        .map(|ord| TopoName::Weak { base, ord })
        .collect())
}

/// The reference of 20.2's rule: of the names every colliding entity touches, the lowest; `None`
/// when they share none (order on the body's frame instead).
#[must_use]
pub fn reference(touched: &[NameSet]) -> Option<NameId> {
    let (first, rest) = touched.split_first()?;
    first.names().iter().copied().find(|name| {
        rest.iter()
            .all(|set| set.names().binary_search(name).is_ok())
    })
}

/// The positions of `entities` in 20.2's order: a bottom-up merge sort that fails on the first
/// comparison it cannot decide.
///
/// Every tie is found: two entities that tie on every step are next to each other in the order,
/// and a merge compares every pair that ends up next to each other across its two runs, while
/// pairs inside a run were compared by the merges that made it.
fn sorted<K: WeakOrder>(entities: &[K]) -> Result<Vec<usize>, OrderError> {
    let mut from: Vec<(usize, &K)> = entities.iter().enumerate().collect();
    let mut into: Vec<(usize, &K)> = Vec::with_capacity(from.len());
    let mut width = 1_usize;
    while width < from.len() {
        into.clear();
        for run in from.chunks(width.saturating_mul(2)) {
            let (left, right) = run.split_at(width.min(run.len()));
            merge(left, right, &mut into)?;
        }
        core::mem::swap(&mut from, &mut into);
        width = width.saturating_mul(2);
    }
    Ok(from.into_iter().map(|(position, _)| position).collect())
}

/// Merges the sorted runs `left` and `right` onto `into`, the left one first among equals.
fn merge<'k, K: WeakOrder>(
    left: &[(usize, &'k K)],
    right: &[(usize, &'k K)],
    into: &mut Vec<(usize, &'k K)>,
) -> Result<(), OrderError> {
    let (mut i, mut j) = (0, 0);
    while let (Some(&a), Some(&b)) = (left.get(i), right.get(j)) {
        match compare(a.1, b.1) {
            Some(Ordering::Greater) => {
                into.push(b);
                j = j.saturating_add(1);
            }
            Some(_) => {
                into.push(a);
                i = i.saturating_add(1);
            }
            None => return Err(OrderError::UndecidedOrder(a.0.min(b.0), a.0.max(b.0))),
        }
    }
    into.extend_from_slice(left.get(i..).unwrap_or(&[]));
    into.extend_from_slice(right.get(j..).unwrap_or(&[]));
    Ok(())
}
