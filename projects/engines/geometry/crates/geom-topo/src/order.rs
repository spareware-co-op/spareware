//! Name order: the key every named entity is sorted by, and the permutation that moves an arena's
//! rows into that order (8.2, accepted in 32.17).
//!
//! What it does: [`NameKey`] is a persistent name's 16 bytes as the caller supplies them (this
//! crate never makes names; `geom-name` does, 20.1). [`Permutation::by_name`] sorts one arena's
//! rows by their keys and gives, for every row, the row it moves to, so that the builder can move
//! the arena and rewrite every index that points into it before a body is published.
//!
//! Guarantees:
//!
//! - **Index order is name order.** After [`Permutation::apply`], row `i` holds the entity with the
//!   `i`-th smallest key, compared as bytes, so a body's arenas depend only on the names it holds,
//!   never on the order an operation produced them in (32.17).
//! - **Names are unique per arena.** Two rows with the same key are refused with
//!   [`OrderError::DuplicateName`], naming the two rows that come first in name order, never
//!   ordered by a guess (20.2: a duplicate the contract does not expect is a bug).
//! - **A bijection.** [`Permutation::new_of`] maps `0..len` onto `0..len`, one to one, and
//!   [`Permutation::old_of`] is its inverse.
//!
//! Method: a sort by key, then by row to keep the order total, as section 23.4's builder sorts its
//! scratch arrays before publishing (`projects/engines/geometry/architecture/08-topology.md`, 8.2;
//! `projects/engines/geometry/architecture/32-decisions.md`, 32.17).
//!
//! Cost: `O(n log n)` comparisons of 16 bytes and two vectors of `n` indices; nothing recurses.

use alloc::vec::Vec;

/// A persistent name's 16 bytes (`NameScheme` v1, 20.1), compared as bytes: the order a named
/// arena is published in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NameKey(pub [u8; 16]);

/// Why an arena's rows could not be put in name order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderError {
    /// Two rows carry the same name: the first such pair in name order, the lower row first.
    DuplicateName {
        /// The lower of the two rows.
        first: u32,
        /// The higher of the two rows.
        second: u32,
    },
    /// More rows than a 32-bit index addresses.
    TooManyRows,
}

/// A one-to-one map of the rows `0..len` onto themselves: where each row of an arena moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permutation {
    /// `new_of_old[old]` is the row the entity at `old` moves to.
    new_of_old: Vec<u32>,
    /// `old_of_new[new]` is the row the entity now at `new` came from.
    old_of_new: Vec<u32>,
}

impl Permutation {
    /// The permutation that sorts `keys` by name: the entity at row `old` moves to the row its key
    /// takes among all of them.
    ///
    /// # Errors
    ///
    /// [`OrderError::DuplicateName`] when two keys are equal; [`OrderError::TooManyRows`] when
    /// there are more keys than a 32-bit index addresses.
    pub fn by_name(keys: &[NameKey]) -> Result<Permutation, OrderError> {
        let n = u32::try_from(keys.len()).map_err(|_| OrderError::TooManyRows)?;
        let mut old_of_new: Vec<u32> = (0..n).collect();
        // Rows are distinct, so the order is total even before duplicates are refused.
        old_of_new.sort_by(|&a, &b| key(keys, a).cmp(&key(keys, b)).then(a.cmp(&b)));
        for pair in old_of_new.windows(2) {
            if let [a, b] = *pair
                && key(keys, a) == key(keys, b)
            {
                return Err(OrderError::DuplicateName {
                    first: a.min(b),
                    second: a.max(b),
                });
            }
        }
        let mut new_of_old: Vec<u32> = (0..n).collect();
        for (new, &old) in (0..n).zip(old_of_new.iter()) {
            if let Some(slot) = usize::try_from(old)
                .ok()
                .and_then(|o| new_of_old.get_mut(o))
            {
                *slot = new;
            }
        }
        Ok(Permutation {
            new_of_old,
            old_of_new,
        })
    }

    /// The number of rows it moves.
    #[must_use]
    pub fn len(&self) -> u32 {
        // Built from at most u32::MAX keys (by_name refuses more).
        u32::try_from(self.new_of_old.len()).unwrap_or(u32::MAX)
    }

    /// Whether it moves no rows at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.new_of_old.is_empty()
    }

    /// The row the entity at `old` moves to, or `None` past the end.
    #[must_use]
    pub fn new_of(&self, old: u32) -> Option<u32> {
        usize::try_from(old)
            .ok()
            .and_then(|o| self.new_of_old.get(o))
            .copied()
    }

    /// The row the entity now at `new` came from, or `None` past the end.
    #[must_use]
    pub fn old_of(&self, new: u32) -> Option<u32> {
        usize::try_from(new)
            .ok()
            .and_then(|n| self.old_of_new.get(n))
            .copied()
    }

    /// Whether every row stays where it is.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        (0u32..).zip(self.new_of_old.iter()).all(|(i, &n)| i == n)
    }

    /// `rows` in their new order: the row now at `i` is `rows[old_of(i)]`. `None` when `rows`
    /// does not hold exactly [`Permutation::len`] rows.
    #[must_use]
    pub fn apply<T: Clone>(&self, rows: &[T]) -> Option<Vec<T>> {
        if rows.len() != self.old_of_new.len() {
            return None;
        }
        self.old_of_new
            .iter()
            .map(|&old| usize::try_from(old).ok().and_then(|o| rows.get(o)).cloned())
            .collect()
    }
}

/// The key at `row`, or the greatest key past the end (never reached: `row < keys.len()`).
fn key(keys: &[NameKey], row: u32) -> NameKey {
    usize::try_from(row)
        .ok()
        .and_then(|r| keys.get(r))
        .copied()
        .unwrap_or(NameKey([u8::MAX; 16]))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn k(b: u8) -> NameKey {
        let mut x = [0u8; 16];
        x[0] = b;
        NameKey(x)
    }

    #[test]
    fn sorts_rows_by_name() {
        let keys = [k(3), k(1), k(2)];
        let p = Permutation::by_name(&keys).unwrap();
        assert_eq!(p.apply(&keys).unwrap(), [k(1), k(2), k(3)]);
        assert_eq!(
            (p.new_of(0), p.new_of(1), p.new_of(2)),
            (Some(2), Some(0), Some(1))
        );
        assert_eq!((p.old_of(0), p.old_of(2)), (Some(1), Some(0)));
        assert!(!p.is_identity());
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn refuses_a_duplicate_name() {
        let keys = [k(5), k(1), k(5)];
        assert_eq!(
            Permutation::by_name(&keys),
            Err(OrderError::DuplicateName {
                first: 0,
                second: 2
            })
        );
    }

    #[test]
    fn empty_and_sorted_inputs() {
        let p = Permutation::by_name(&[]).unwrap();
        assert!(p.is_empty() && p.is_identity());
        let p = Permutation::by_name(&[k(1), k(2)]).unwrap();
        assert!(p.is_identity());
        assert_eq!(p.apply(&[k(1)]), None);
        assert_eq!(p.new_of(2), None);
    }
}
