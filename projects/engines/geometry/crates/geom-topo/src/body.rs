//! The body: one shape, immutable once published.
//!
//! What it does: a [`Body`] is a kind ([`BodyKind`]), the index of its infinite outside region,
//! and its [`Arenas`]. Only [`crate::Builder::publish`] makes one, after checking that every link
//! points inside the body; nothing changes a body afterwards. An edit starts a builder from the
//! body ([`crate::Builder::edit`]) and publishes a new version, which shares every chunk the edit
//! did not touch with the old one (8.2, 23.9).
//!
//! Guarantees:
//!
//! - **Immutable.** No method takes `&mut self`; a published body's values never change.
//! - **No dangling index.** Every index a body's columns hold is a row of the arena it points into,
//!   the columns of each arena hold equally many rows, and the outside region is a void region of
//!   the body ([`crate::Builder::publish`]).
//! - **Values are all that count.** Two bodies are equal when their kinds, outside regions and
//!   arenas' values are; which chunks they share is a memory detail ([`Body::memory`]).
//!
//! Bytes per wall body, measured (23.9) by `tests/attack_g80.rs`: a plain wall, the solid and
//! the outside region, one shell, six planar faces each with one loop of four fins, twelve line
//! edges and eight vertices, built with exact reservations, holds 16,804 heap bytes in 29 chunks
//! on 64-bit targets and 15,088 on wasm32, beside the body's own 1,008 bytes (508 on wasm32). The
//! geometry pools are most of it: six planes at 1,008 bytes a row and twelve lines at 592 (536 on
//! wasm32), each row the size of its kind's largest variant. The topology's own rows take 1,308
//! bytes and the eight points 192; each of the 29 chunks adds 72 bytes of header and chunk
//! pointers (36 on wasm32). Memory a kind owns on the heap (none for planes and lines) is not
//! counted.
//!
//! Method: Parasolid's body kinds and its infinite void region
//! (`projects/kit/notes/G01-evidence/02-occt-parasolid-acis.md`, section 8), made immutable after
//! building as chapter 8.2 asks, with versions sharing `Arc` chunks as report 03 advises
//! (`projects/kit/notes/G01-evidence/03-rust-kernels-crates.md`, section 5).
//!
//! Cost: every accessor is constant time; cloning is linear in the number of chunks;
//! [`Body::memory`] is linear in the number of chunks.

use alloc::sync::Arc;

use crate::arenas::Arenas;
use crate::column::Memory;
use crate::entity::RegionIx;
use crate::order::Names;

/// What a body is made of (8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BodyKind {
    /// A closed volume.
    Solid,
    /// Open surfaces.
    Sheet,
    /// Curves.
    Wire,
    /// Points.
    Acorn,
    /// Mixed and non-manifold.
    General,
}

/// One shape, immutable once published.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    kind: BodyKind,
    outside: RegionIx,
    arenas: Arenas,
    /// The names of its named entities, in name order, when it was published by name (G81);
    /// shared, so cloning a body copies one pointer for them.
    names: Option<Arc<Names>>,
}

impl Body {
    /// A body of checked parts; only the builder calls this, after its checks.
    pub(crate) const fn checked(kind: BodyKind, outside: RegionIx, arenas: Arenas) -> Body {
        Body {
            kind,
            outside,
            arenas,
            names: None,
        }
    }

    /// The same body, carrying `names`; only the builder calls this, with names in name order.
    pub(crate) fn named(self, names: Names) -> Body {
        Body {
            names: Some(Arc::new(names)),
            ..self
        }
    }

    /// The names of its named entities, each list in its arena's order, which is name order; `None`
    /// for a body published without names.
    #[must_use]
    pub fn names(&self) -> Option<&Names> {
        self.names.as_deref()
    }

    /// What the body is made of.
    #[must_use]
    pub const fn kind(&self) -> BodyKind {
        self.kind
    }

    /// The infinite void region outside the body.
    #[must_use]
    pub const fn outside(&self) -> RegionIx {
        self.outside
    }

    /// The body's arenas, to read.
    #[must_use]
    pub const fn arenas(&self) -> &Arenas {
        &self.arenas
    }

    /// The heap memory the body's chunks hold; see [`Memory`]. A measurement only.
    #[must_use]
    pub fn memory(&self) -> Memory {
        self.arenas.memory()
    }
}
