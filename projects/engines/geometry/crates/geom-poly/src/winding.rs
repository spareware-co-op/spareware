//! Sparse winding vectors and the tests that select pieces: chapter 11.2.
//!
//! What it does: a [`Winding`] holds one winding number per entry of an arrangement, as a sorted
//! list of (entry, count) pairs without zeros, so the outside is the empty list and the length of
//! a list follows how many elements overlap at a point, not how many operands there are. A
//! [`WindingTest`] is the small expression of 11.1: inside an entry, inside at least n entries, and
//! their combinations. [`kept`] reads a piece's front and back vectors through a test: a piece is
//! kept exactly when the test differs across it. [`WindingTest::decided`] reads a test with some
//! entries unknown, which is how a box is found to hold nothing of the result (11.3, step 4).
//!
//! Guarantees:
//!
//! - **One vector, one list.** The pairs stay sorted, with no zero and no repeated entry, whatever
//!   order the crossings came in, so equal vectors compare equal (tested).
//! - **Nothing wraps, nothing aborts.** A count that would leave `i32` is
//!   [`WindingError::Overflow`], and memory refused is [`WindingError::OutOfMemory`].
//! - **No recursion.** A test is held in postfix and evaluated on a stack of 64 answers, so its depth
//!   never threatens a WebAssembly stack (principle 13); a test deeper than that is refused when it
//!   is made.
//! - **A settled test is settled.** [`WindingTest::decided`] answers only where every count of its
//!   open entries gives that answer, so a box it settles holds no boundary of the result.
//!
//! Method: winding-number vectors after EMBER's section 4 (Trettner, Nehring-Wirxel and Kobbelt
//! 2022, <https://doi.org/10.1145/3528223.3530181>), sparse as chapter 11.2 sets out
//! (`projects/engines/geometry/architecture/11-booleans.md`); signed crossings after Jacobson,
//! Kavan and Sorkine-Hornung 2013, <https://doi.org/10.1145/2461912.2461916>.
//!
//! Cost: [`Winding::count`] is a binary search; [`Winding::add`] that and a shift of the list;
//! [`WindingTest::holds`] one step per term, with a look-up for each [`Term::Inside`].

use alloc::vec::Vec;
use core::fmt;

/// An operand's place in an arrangement, in (operand key) order.
pub type Entry = u32;

/// Why a winding vector or a test could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindingError {
    /// A count left `i32`.
    Overflow,
    /// The allocator refused.
    OutOfMemory,
    /// A test's terms do not leave one answer.
    BadTest,
    /// A test is deeper than 64.
    TooDeep,
}

impl fmt::Display for WindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            WindingError::Overflow => "a winding number left its range",
            WindingError::OutOfMemory => "the allocator refused a winding vector's room",
            WindingError::BadTest => "a winding test's terms do not leave one answer",
            WindingError::TooDeep => "a winding test is deeper than 64",
        })
    }
}

impl core::error::Error for WindingError {}

/// One winding number per entry; the entries it does not hold are zero.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Winding {
    pairs: Vec<(Entry, i32)>,
}

impl Winding {
    /// The outside: every count zero.
    #[must_use]
    pub const fn new() -> Self {
        Winding { pairs: Vec::new() }
    }

    /// The (entry, count) pairs, sorted by entry, none zero.
    #[must_use]
    pub fn pairs(&self) -> &[(Entry, i32)] {
        &self.pairs
    }

    /// The winding number of `entry`.
    #[must_use]
    pub fn count(&self, entry: Entry) -> i32 {
        self.pairs
            .binary_search_by_key(&entry, |(e, _)| *e)
            .ok()
            .and_then(|at| self.pairs.get(at))
            .map_or(0, |(_, count)| *count)
    }

    /// Room for `more` entries.
    ///
    /// # Errors
    ///
    /// [`WindingError::OutOfMemory`] if the allocator refuses; the vector is unchanged.
    pub fn reserve(&mut self, more: usize) -> Result<(), WindingError> {
        self.pairs
            .try_reserve(more)
            .map_err(|_| WindingError::OutOfMemory)
    }

    /// Adds `by` to the winding number of `entry`.
    ///
    /// # Errors
    ///
    /// [`WindingError::Overflow`] if the count leaves `i32`, [`WindingError::OutOfMemory`] if the
    /// allocator refuses; the vector is unchanged.
    pub fn add(&mut self, entry: Entry, by: i32) -> Result<(), WindingError> {
        match self.pairs.binary_search_by_key(&entry, |(e, _)| *e) {
            Ok(at) => {
                let sum = self.count(entry).checked_add(by);
                let sum = sum.ok_or(WindingError::Overflow)?;
                self.pairs.remove(at);
                if sum != 0 {
                    self.pairs.insert(at, (entry, sum));
                }
            }
            Err(at) => {
                if by != 0 {
                    self.reserve(1)?;
                    self.pairs.insert(at, (entry, by));
                }
            }
        }
        Ok(())
    }
}

/// A term of a [`WindingTest`], in postfix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term {
    /// Whether the point is inside this entry: its winding number is above zero.
    Inside(Entry),
    /// Whether the point is inside at least this many entries.
    AtLeast(u32),
    /// Whether both answers below are true.
    And,
    /// Whether either answer below is true.
    Or,
    /// The opposite of the answer below.
    Not,
}

/// A test on a winding vector, 11.1's small expression, in postfix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindingTest {
    terms: Vec<Term>,
}

impl WindingTest {
    /// The test of `terms`.
    ///
    /// # Errors
    ///
    /// [`WindingError::BadTest`] if a term finds too few answers or more than one answer is left,
    /// [`WindingError::TooDeep`] if a term would pile up more than 64.
    pub fn new(terms: Vec<Term>) -> Result<Self, WindingError> {
        let mut depth = 0u32;
        for term in &terms {
            let (needs, leaves) = match term {
                Term::Inside(_) | Term::AtLeast(_) => (0, depth.saturating_add(1)),
                Term::And | Term::Or => (2, depth.saturating_sub(1)),
                Term::Not => (1, depth),
            };
            if depth < needs {
                return Err(WindingError::BadTest);
            }
            if leaves > 64 {
                return Err(WindingError::TooDeep);
            }
            depth = leaves;
        }
        if depth == 1 {
            Ok(WindingTest { terms })
        } else {
            Err(WindingError::BadTest)
        }
    }

    /// Inside any entry.
    #[must_use]
    pub fn unite() -> Self {
        WindingTest {
            terms: alloc::vec![Term::AtLeast(1)],
        }
    }

    /// Inside every one of `count` entries.
    #[must_use]
    pub fn intersect(count: u32) -> Self {
        WindingTest {
            terms: alloc::vec![Term::AtLeast(count)],
        }
    }

    /// Inside `target` and no other entry of the arrangement: at most one entry, and that one.
    #[must_use]
    pub fn subtract(target: Entry) -> Self {
        WindingTest {
            terms: alloc::vec![Term::Inside(target), Term::AtLeast(2), Term::Not, Term::And],
        }
    }

    /// Whether `winding` passes.
    #[must_use]
    pub fn holds(&self, winding: &Winding) -> bool {
        self.decided(winding, &[]) == Some(true)
    }

    /// What the test says of every point where `known` gives each count but those of `open`, which
    /// may be anything, or `None` if the answer turns on them: Kleene's three-valued reading.
    #[must_use]
    pub fn decided(&self, known: &Winding, open: &[Entry]) -> Option<bool> {
        let mut stack = [None; 64];
        let mut depth = 0usize;
        for term in &self.terms {
            let top = depth.saturating_sub(1);
            let second = top.saturating_sub(1);
            let read = |at: usize| stack.get(at).copied().flatten();
            let (at, answer) = match term {
                Term::Inside(entry) => (depth, inside(known, open, *entry)),
                Term::AtLeast(n) => (depth, at_least(known, open, *n)),
                Term::Not => (top, read(top).map(|answer: bool| !answer)),
                Term::And => (second, both(read(second), read(top))),
                Term::Or => (second, either(read(second), read(top))),
            };
            stack
                .iter_mut()
                .skip(at)
                .take(1)
                .for_each(|slot| *slot = answer);
            depth = at.saturating_add(1);
        }
        stack.first().copied().flatten()
    }
}

/// Whether a point is inside `entry`, unless `entry` is open.
fn inside(known: &Winding, open: &[Entry], entry: Entry) -> Option<bool> {
    (!open.contains(&entry)).then(|| known.count(entry) > 0)
}

/// Whether a point is inside at least `n` entries, unless the open ones can change that.
fn at_least(known: &Winding, open: &[Entry], n: u32) -> Option<bool> {
    let settled = known
        .pairs
        .iter()
        .filter(|(entry, count)| count.is_positive() && !open.contains(entry))
        .count();
    let n = usize::try_from(n).unwrap_or(usize::MAX);
    if settled >= n {
        Some(true)
    } else if settled.saturating_add(open.len()) < n {
        Some(false)
    } else {
        None
    }
}

/// Kleene's and.
fn both(a: Option<bool>, b: Option<bool>) -> Option<bool> {
    match (a, b) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), Some(true)) => Some(true),
        _ => None,
    }
}

/// Kleene's or.
fn either(a: Option<bool>, b: Option<bool>) -> Option<bool> {
    match (a, b) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        _ => None,
    }
}

/// What a test makes of a piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// The test is the same on both sides: no boundary of the result.
    No,
    /// Inside behind, outside in front: kept as it faces.
    AsIs,
    /// Inside in front, outside behind: kept facing the other way.
    Flipped,
}

/// What `test` makes of a piece with these vectors.
#[must_use]
pub fn kept(test: &WindingTest, front: &Winding, back: &Winding) -> Kept {
    match (test.holds(front), test.holds(back)) {
        (false, true) => Kept::AsIs,
        (true, false) => Kept::Flipped,
        _ => Kept::No,
    }
}
