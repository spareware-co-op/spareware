//! The table of definitions: each shape stored once, in its own frame, with its own tree, cached
//! by its content hash and the kernel edition that made it (19.12, 23.8; piece G295).
//!
//! What it does: a [`Definitions`] table belongs to the caller (23.9). Each entry is a
//! [`Definition`]: its [`DefinitionKey`], its frame, its box in that frame and its [`Content`],
//! either the parts of a body or mesh with their own box tree (`geom-bvh`), or a nested
//! [`Assembly`] with its own records and instance tree. [`Definitions::get_or_insert_with`] looks
//! the key up and makes the content only when the key is not cached, so a door placed 2,000 times
//! is built and stored once. Handles are positions in the table, in the order definitions were
//! made, and a record names its definition by handle.
//!
//! Guarantees:
//!
//! - **Built once.** A key already in the table returns its handle and never runs `make`; two
//!   keys differ in content hash or in edition, so a definition made by an older kernel edition is
//!   never reused after an upgrade (23.8).
//! - **Nesting is acyclic and bounded.** An assembly can only name definitions made before it, so
//!   no assembly contains itself, however indirectly; and an assembly nested more than
//!   [`MAX_NESTING`] deep is refused where it is cached, so every walk stops within that depth.
//! - **All or nothing.** A refused insertion leaves the table as it was.
//!
//! Method: chapter 19.12's definitions cached by content hash and kernel edition, and chapter
//! 23.8's content-addressed keys (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`,
//! `projects/engines/geometry/architecture/23-performance.md`).
//!
//! Cost: a lookup is O(log definitions) in an ordered map; an insertion adds the content's own
//! cost, and checks a nested assembly's definitions once each. Recursion: none.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use geom_bvh::BoxTree;
use geom_math::{Box3, FrameRecord};

use super::assembly::Assembly;
use super::enter::Relation;
use super::error::InstanceError;
use super::tree::grid_box;

/// The deepest assemblies nest: a definition's depth is 0 for parts and one more than its
/// deepest placed definition for an assembly, and no assembly may be deeper than this. Sixteen
/// levels hold a building's storeys, rooms, assemblies and their fittings, and a product
/// structure of a car or a plant, with room to spare; the bound keeps every path of a query a
/// fixed size.
pub const MAX_NESTING: usize = 16;

/// The key a definition is cached by (19.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefinitionKey {
    /// Its content hash: chapter 24.2's content ID, the SHA-256 root over its canonical bytes,
    /// computed by the caller (`geom-format` sits above this crate).
    pub content: [u8; 32],
    /// The kernel edition that made it (26.2).
    pub edition: u32,
}

/// What a definition holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content<R> {
    /// A body or a mesh: the box tree of its parts (faces or triangles) in its own frame, each
    /// part keyed as the caller names it.
    Parts {
        /// Its frame.
        frame: FrameRecord,
        /// Its parts' tree.
        tree: BoxTree<3>,
    },
    /// A nested assembly, with its own records and instance tree.
    Assembly(Assembly<R>),
}

/// One definition (module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition<R> {
    key: DefinitionKey,
    bound: Box3,
    depth: u8,
    content: Content<R>,
}

impl<R> Definition<R> {
    /// Its key.
    #[must_use]
    pub const fn key(&self) -> DefinitionKey {
        self.key
    }

    /// Its frame.
    #[must_use]
    pub fn frame(&self) -> &FrameRecord {
        match &self.content {
            Content::Parts { frame, .. } => frame,
            Content::Assembly(a) => a.frame(),
        }
    }

    /// Its box, in its own frame.
    #[must_use]
    pub const fn bound(&self) -> &Box3 {
        &self.bound
    }

    /// Its depth: 0 for parts, one more than its deepest placed definition for an assembly.
    #[must_use]
    pub const fn depth(&self) -> u8 {
        self.depth
    }

    /// What it holds.
    #[must_use]
    pub const fn content(&self) -> &Content<R> {
        &self.content
    }
}

/// The caller's table of definitions (module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definitions<R> {
    by_key: BTreeMap<DefinitionKey, u32>,
    items: Vec<Definition<R>>,
}

impl<R> Default for Definitions<R> {
    fn default() -> Self {
        Definitions {
            by_key: BTreeMap::new(),
            items: Vec::new(),
        }
    }
}

impl<R> Definitions<R> {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Definitions::default()
    }

    /// How many definitions it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The handle of the definition cached under `key`, if any.
    #[must_use]
    pub fn find(&self, key: &DefinitionKey) -> Option<u32> {
        self.by_key.get(key).copied()
    }

    /// The definition with handle `handle`, if any.
    #[must_use]
    pub fn get(&self, handle: u32) -> Option<&Definition<R>> {
        self.items.get(usize::try_from(handle).ok()?)
    }

    /// The box of the definition with handle `handle` in the steps of `frame`, the frame of an
    /// assembly that places it: rounded outward from a finer frame, exact from the same or a
    /// coarser one. A record's box is made from it (`Placed::exact` moves it by the placement).
    ///
    /// # Errors
    ///
    /// [`InstanceError::NoDefinition`] for a handle the table does not hold;
    /// [`InstanceError::Frame`] for frames with no exact shift between them;
    /// [`InstanceError::Num`] for a box beyond `frame`'s range.
    pub fn bound_in(&self, handle: u32, frame: &FrameRecord) -> Result<Box3, InstanceError> {
        let d = self
            .get(handle)
            .ok_or(InstanceError::NoDefinition(handle))?;
        Relation::between(frame, d.frame())?.outward(d.bound())
    }

    /// The handle of the definition cached under `key`; when there is none, `make` makes its
    /// content from the table as it stands, and the definition is cached under `key`.
    ///
    /// # Errors
    ///
    /// `make`'s error; [`InstanceError::Empty`] for content with no geometry;
    /// [`InstanceError::Num`] for parts whose box leaves the grid's range;
    /// [`InstanceError::Stale`] for an assembly that names a handle this table holds under another
    /// key, and [`InstanceError::NoDefinition`] for one it does not hold;
    /// [`InstanceError::TooDeep`] for an assembly nested deeper than [`MAX_NESTING`];
    /// [`InstanceError::TooManyRecords`] past `u32::MAX` definitions; [`InstanceError::OutOfMemory`].
    /// On every error the table is unchanged.
    pub fn get_or_insert_with(
        &mut self,
        key: DefinitionKey,
        make: impl FnOnce(&Self) -> Result<Content<R>, InstanceError>,
    ) -> Result<u32, InstanceError> {
        if let Some(handle) = self.find(&key) {
            return Ok(handle);
        }
        let handle = u32::try_from(self.items.len()).map_err(|_| InstanceError::TooManyRecords)?;
        let content = make(self)?;
        let (bound, depth) = match &content {
            Content::Parts { tree, .. } => {
                let root = tree.nodes().first().ok_or(InstanceError::Empty)?;
                (grid_box(&root.bbox())?, 0)
            }
            Content::Assembly(a) => {
                let bound = a.bound()?.ok_or(InstanceError::Empty)?;
                let mut deepest = 0u8;
                for (child, key) in a.placed() {
                    let d = self.get(child).ok_or(InstanceError::NoDefinition(child))?;
                    if d.key != key {
                        return Err(InstanceError::Stale(child));
                    }
                    deepest = deepest.max(d.depth);
                }
                let depth = deepest.checked_add(1).ok_or(InstanceError::TooDeep)?;
                if usize::from(depth) > MAX_NESTING {
                    return Err(InstanceError::TooDeep);
                }
                (bound, depth)
            }
        };
        self.items
            .try_reserve(1)
            .map_err(|_| InstanceError::OutOfMemory)?;
        self.items.push(Definition {
            key,
            bound,
            depth,
            content,
        });
        self.by_key.insert(key, handle);
        Ok(handle)
    }
}
