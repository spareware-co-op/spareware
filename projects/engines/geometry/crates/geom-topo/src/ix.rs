//! Typed 32-bit indices into a body's arenas, and the names of the arenas.
//!
//! What it does: [`Ix<K>`] is the index of one row of the arena whose rows are of kind `K`:
//! `Ix<Face>` ([`crate::FaceIx`]) indexes faces, `Ix<SurfaceKind>` ([`crate::SurfaceIx`]) the
//! surface pool, and so on (chapter 8.2). [`Kind`] says which arena a kind's rows live in, and
//! [`Arena`] names the eleven arenas: seven of topology and four geometry pools.
//!
//! Guarantees:
//!
//! - **Typed.** An `Ix<Face>` can only index the face arena; mixing kinds is a compile error, not
//!   a wrong row.
//! - **Thirty-two bits, and an absent link costs nothing.** `Ix<K>` and `Option<Ix<K>>` are both
//!   four bytes: the row is stored plus one, so zero is free for `None`. Rows run from 0 to
//!   [`Ix::MAX_ROW`], `u32::MAX − 1`, so an arena holds at most `u32::MAX` rows.
//! - **Ordered as rows, never as addresses.** Equality, order and hashing are those of the row, so
//!   sorting indices sorts them in arena order, which is persistent-name order (8.2; 32.17). An
//!   index holds no pointer and nothing about where memory lies.
//! - **Valid inside one body version only.** An index is a position, not a name; references and
//!   files hold persistent names (8.2, 20.1).
//!
//! Bounded proofs, and what is only tested: Kani proves (piece G112, chapter 28.7) that every
//! row but `u32::MAX` has an index that gives it back, `u32::MAX` none, through both constructors,
//! and that indices compare as their rows, for every `u32`. Hashing and printing are tested.
//!
//! Method: one arena per entity kind addressed by a `u32` newtype index, never by address, the
//! design Fornjot ended on after losing determinism to address identity
//! (`projects/kit/notes/G01-evidence/03-rust-kernels-crates.md`, sections 2 and 5); the free
//! value for `None` is `core::num::NonZeroU32`'s niche
//! (<https://doc.rust-lang.org/core/num/type.NonZeroU32.html>).
//!
//! Cost: every operation is constant time and allocates nothing.

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::num::NonZeroU32;

/// The arenas of a body: seven of topology (chapter 8.1) and four geometry pools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arena {
    /// Regions: connected volumes, solid or void.
    Region,
    /// Shells: connected sets of faces bounding a region.
    Shell,
    /// Faces: bounded pieces of one surface.
    Face,
    /// Loops: closed boundaries of a face.
    Loop,
    /// Fins: the uses of an edge by a loop, in one direction.
    Fin,
    /// Edges: bounded pieces of one curve.
    Edge,
    /// Vertices: points of the topology.
    Vertex,
    /// The surface pool: the surfaces faces lie on.
    Surface,
    /// The curve pool: the 3D curves edges lie on.
    Curve,
    /// The pcurve pool: the curves fins lie on, in their face's parameter space.
    Pcurve,
    /// The point pool: the points vertices lie at.
    Point,
}

impl Arena {
    /// Every arena, in this order: the topology from region to vertex, then the pools.
    pub const ALL: [Arena; 11] = [
        Arena::Region,
        Arena::Shell,
        Arena::Face,
        Arena::Loop,
        Arena::Fin,
        Arena::Edge,
        Arena::Vertex,
        Arena::Surface,
        Arena::Curve,
        Arena::Pcurve,
        Arena::Point,
    ];

    /// The arena's name, as the errors write it: `region`, `shell`, and so on.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Arena::Region => "region",
            Arena::Shell => "shell",
            Arena::Face => "face",
            Arena::Loop => "loop",
            Arena::Fin => "fin",
            Arena::Edge => "edge",
            Arena::Vertex => "vertex",
            Arena::Surface => "surface",
            Arena::Curve => "curve",
            Arena::Pcurve => "pcurve",
            Arena::Point => "point",
        }
    }
}

impl fmt::Display for Arena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A kind of row with an arena of its own: an entity of chapter 8.1, or a pool's geometry.
pub trait Kind {
    /// The arena this kind's rows live in.
    const ARENA: Arena;
}

/// The index of one row of the arena of `K`: 32 bits, typed by the arena it indexes.
pub struct Ix<K> {
    /// The row plus one, so `Option<Ix<K>>` uses zero for `None`.
    stored: NonZeroU32,
    kind: PhantomData<fn() -> K>,
}

impl<K> Ix<K> {
    /// The largest row an index holds, `u32::MAX − 1`.
    pub const MAX_ROW: u32 = u32::MAX - 1;

    /// The index of `row`, or `None` for `u32::MAX`, which no index holds.
    #[must_use]
    pub const fn new(row: u32) -> Option<Ix<K>> {
        match NonZeroU32::MIN.checked_add(row) {
            Some(stored) => Some(Ix {
                stored,
                kind: PhantomData,
            }),
            None => None,
        }
    }

    /// The index of `row`, or `None` past [`Ix::MAX_ROW`].
    #[must_use]
    pub fn from_usize(row: usize) -> Option<Ix<K>> {
        u32::try_from(row).ok().and_then(Ix::new)
    }

    /// The row.
    #[must_use]
    pub const fn row(self) -> u32 {
        // `stored` is at least one, so this never wraps.
        self.stored.get().wrapping_sub(1)
    }

    /// The row, as a `usize` (every target the kernel builds for has `usize` of 32 bits or more).
    #[must_use]
    pub const fn to_usize(self) -> usize {
        self.row() as usize
    }
}

impl<K> Clone for Ix<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for Ix<K> {}

impl<K> PartialEq for Ix<K> {
    fn eq(&self, other: &Self) -> bool {
        self.stored == other.stored
    }
}

impl<K> Eq for Ix<K> {}

impl<K> PartialOrd for Ix<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K> Ord for Ix<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.stored.cmp(&other.stored)
    }
}

impl<K> Hash for Ix<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.row().hash(state);
    }
}

impl<K: Kind> fmt::Debug for Ix<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", K::ARENA, self.row())
    }
}

impl<K: Kind> fmt::Display for Ix<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", K::ARENA, self.row())
    }
}

#[cfg(kani)]
// cargo-mutants reads this attribute from the source and skips the module (its harnesses are
// proven by Kani, and no test build compiles them); no build evaluates it, since the module
// exists only under Kani, where `test` is off.
#[cfg_attr(test, mutants::skip)]
mod proofs {
    use super::*;

    /// A kind for the proofs.
    struct Row;

    impl Kind for Row {
        const ARENA: Arena = Arena::Face;
    }

    /// Every row but `u32::MAX` has an index, which gives the row back; `u32::MAX` has none.
    #[kani::proof]
    fn every_row_round_trips() {
        let row: u32 = kani::any();
        match Ix::<Row>::new(row) {
            Some(ix) => {
                assert!(row <= Ix::<Row>::MAX_ROW);
                assert_eq!(ix.row(), row);
                assert_eq!(ix.to_usize(), row as usize);
                assert_eq!(Ix::<Row>::from_usize(row as usize), Some(ix));
            }
            None => assert_eq!(row, u32::MAX),
        }
    }

    /// Indices compare as their rows, and an absent index costs no space.
    #[kani::proof]
    fn order_is_row_order() {
        let (a, b): (u32, u32) = (kani::any(), kani::any());
        kani::assume(a < u32::MAX && b < u32::MAX);
        let (x, y) = (Ix::<Row>::new(a), Ix::<Row>::new(b));
        assert_eq!(x.cmp(&y), a.cmp(&b));
        assert_eq!(x == y, a == b);
        assert_eq!(core::mem::size_of::<Option<Ix<Row>>>(), 4);
    }
}
