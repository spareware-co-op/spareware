//! What a local operation may change, and its refusal of anything else (13.14; piece G144).
//!
//! What it does: an [`AllowSet`] is a set of chapter 13.14's [`ChangeClass`]es. Each local
//! operation declares the set it may make, its [`LocalSchema`] beside its [`OpSchema`], and a
//! caller narrows it ([`AllowSet::narrowed`]). [`allowed`] holds the changes an operation made to
//! the set and returns the first it does not allow as a [`LocalDetail::TopologyChangeNotAllowed`],
//! which names the class; the error built from it names the entities.
//!
//! Guarantees:
//!
//! - **Never wider than declared.** [`AllowSet::narrowed`] keeps only what both sets hold, so a
//!   caller can take classes away and never add one the operation does not declare.
//! - **The first refusal, in order.** [`allowed`] reads the changes in the order given, which a
//!   [`History`](crate::history::History) keeps by name, so the same changes give the same refusal.
//! - **One kind.** A change outside the set is `NoSolution`: no result exists within the changes
//!   the caller allows, and its remedy is to allow the class (13.20).
//!
//! Method: chapter 13.14's taxonomy and allow-sets and 13.20's `TopologyChangeNotAllowed`
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`), on chapter 22's error
//! kinds (`projects/engines/geometry/architecture/22-errors-queries-ai.md`).
//!
//! Cost: a set is one word; [`allowed`] is linear in the changes. No allocation but the remedy
//! list. Recursion: none.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::history::{ChangeClass, TopologyChange};
use crate::kind::ErrorKind;
use crate::remedy::Remedy;
use crate::schema::OpSchema;

/// Every class, in 13.14's order.
pub const CLASSES: [ChangeClass; 9] = [
    ChangeClass::VertexSplit,
    ChangeClass::FaceVanished,
    ChangeClass::SelfOverlapCut,
    ChangeClass::ClashTrimmed,
    ChangeClass::OvershootRemoved,
    ChangeClass::FacesMerged,
    ChangeClass::FaceAdded,
    ChangeClass::LoopChanged,
    ChangeClass::BodySplit,
];

/// The bit of a class in a set.
const fn bit(class: ChangeClass) -> u16 {
    match class {
        ChangeClass::VertexSplit => 1,
        ChangeClass::FaceVanished => 1 << 1,
        ChangeClass::SelfOverlapCut => 1 << 2,
        ChangeClass::ClashTrimmed => 1 << 3,
        ChangeClass::OvershootRemoved => 1 << 4,
        ChangeClass::FacesMerged => 1 << 5,
        ChangeClass::FaceAdded => 1 << 6,
        ChangeClass::LoopChanged => 1 << 7,
        ChangeClass::BodySplit => 1 << 8,
    }
}

/// A set of topology-change classes (13.14).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AllowSet(u16);

impl AllowSet {
    /// No class: an operation that never changes topology.
    pub const NONE: AllowSet = AllowSet(0);

    /// Every class.
    pub const ALL: AllowSet = AllowSet(0x1ff);

    /// The set of `classes`.
    #[must_use]
    pub const fn of(classes: &[ChangeClass]) -> AllowSet {
        let mut bits = 0u16;
        let mut i = 0;
        while i < classes.len() {
            // `i < len` holds, so the index is in range; a const fn cannot use `get` in a loop.
            #[allow(
                clippy::indexing_slicing,
                reason = "i is below classes.len(), checked by the loop condition"
            )]
            let class = classes[i];
            bits |= bit(class);
            i = i.saturating_add(1);
        }
        AllowSet(bits)
    }

    /// Whether the set holds `class`.
    #[must_use]
    pub const fn contains(self, class: ChangeClass) -> bool {
        self.0 & bit(class) != 0
    }

    /// What both sets hold: an operation's declared set narrowed by its caller's.
    #[must_use]
    pub const fn narrowed(self, by: AllowSet) -> AllowSet {
        AllowSet(self.0 & by.0)
    }

    /// The set with `class` taken away.
    #[must_use]
    pub const fn without(self, class: ChangeClass) -> AllowSet {
        AllowSet(self.0 & !bit(class))
    }

    /// The classes the set holds, in 13.14's order.
    pub fn classes(self) -> impl Iterator<Item = ChangeClass> {
        CLASSES.into_iter().filter(move |c| self.contains(*c))
    }
}

/// A local operation's schema: the operation's own, and the classes it may make (13.1, 13.14).
#[derive(Clone, Copy, Debug)]
pub struct LocalSchema {
    /// The operation's schema.
    pub schema: &'static OpSchema,
    /// What it may change.
    pub may: AllowSet,
}

/// A refusal of a local operation's changes (13.20).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LocalDetail {
    /// The operation would make a change of `class`, which the call does not allow.
    TopologyChangeNotAllowed {
        /// The class.
        class: ChangeClass,
    },
}

impl LocalDetail {
    /// The base kind: no result exists within the changes allowed.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        match self {
            LocalDetail::TopologyChangeNotAllowed { .. } => ErrorKind::NoSolution,
        }
    }

    /// The remedies: allow the class, or change the operation's own input so it does not make it.
    #[must_use]
    pub fn remedies(&self) -> Vec<Remedy> {
        match self {
            LocalDetail::TopologyChangeNotAllowed { .. } => vec![Remedy::ChangeInput {
                input: "allow",
                to: None,
            }],
        }
    }
}

impl fmt::Display for LocalDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocalDetail::TopologyChangeNotAllowed { class } => {
                write!(
                    f,
                    "the operation would make a change of class {class:?}, which the call does not allow"
                )
            }
        }
    }
}

impl core::error::Error for LocalDetail {}

/// Whether `set` allows every one of `changes`: `Err` with the first change it does not, by class.
///
/// # Errors
///
/// [`LocalDetail::TopologyChangeNotAllowed`] for the first change outside the set, with the change
/// itself, whose names the error reports.
pub fn allowed(
    changes: &[TopologyChange],
    set: AllowSet,
) -> Result<(), (LocalDetail, &TopologyChange)> {
    match changes.iter().find(|c| !set.contains(c.class())) {
        Some(change) => Err((
            LocalDetail::TopologyChangeNotAllowed {
                class: change.class(),
            },
            change,
        )),
        None => Ok(()),
    }
}
