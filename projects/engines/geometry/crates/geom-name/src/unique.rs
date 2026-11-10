//! The duplicate-name detector: a name an operation gives twice, which its contract does not
//! expect, is a bug that tests must catch (chapter 20.2).
//!
//! What it does: [`first_duplicate`] finds the lowest name that appears more than once in a list.
//! [`expect_unique`] is what an operation calls on the names it made before publishing them: in a
//! build with debug assertions, every test build, it refuses a list with a duplicate; in a release
//! build it checks nothing, because there a collision still becomes a `Weak` name, never a random
//! suffix (20.2), and the operation's own naming has already done that.
//!
//! Guarantees:
//!
//! - **Exact.** [`first_duplicate`] returns `Some` exactly when a name appears twice, and the
//!   lowest such name, whatever the list's order.
//! - **On in every test build.** Under `debug_assertions`, [`expect_unique`] is
//!   [`first_duplicate`]'s answer as an error; the kernel's test legs build with debug assertions on.
//!
//! Method: sort and compare neighbours, the duplicate test any sorted sequence allows; chapter
//! 20.2's rule that an unexpected duplicate name is a bug caught by tests,
//! `projects/engines/geometry/architecture/20-naming-history.md`.
//!
//! Cost: O(n log n) time and one copy of the list. Recursion: none.

use alloc::vec::Vec;
use core::fmt;

use geom_op::NameId;

/// A name given to two entities, which the operation's contract does not expect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuplicateName(pub NameId);

impl fmt::Display for DuplicateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the name {} was given twice", self.0)
    }
}

impl core::error::Error for DuplicateName {}

/// The lowest name that appears more than once in `names`, if any.
#[must_use]
pub fn first_duplicate(names: &[NameId]) -> Option<NameId> {
    let mut sorted: Vec<NameId> = names.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).find_map(|pair| match pair {
        [a, b] if a == b => Some(*a),
        _ => None,
    })
}

/// Refuses `names` if one appears twice: the check of a build with debug assertions.
///
/// # Errors
///
/// [`DuplicateName`] with the lowest repeated name.
#[cfg(debug_assertions)]
pub fn expect_unique(names: &[NameId]) -> Result<(), DuplicateName> {
    first_duplicate(names).map_or(Ok(()), |name| Err(DuplicateName(name)))
}

/// Accepts `names` unchecked: a release build, where a collision has already become a `Weak` name.
///
/// # Errors
///
/// None in a release build; see the build with debug assertions.
#[cfg(not(debug_assertions))]
pub fn expect_unique(_names: &[NameId]) -> Result<(), DuplicateName> {
    Ok(())
}
