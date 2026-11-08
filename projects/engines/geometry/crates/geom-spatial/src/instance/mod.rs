//! The instance tree for huge assemblies: each shape stored once and placed many times, found by
//! a two-level tree, through nested assemblies, in fine and coarse frames (19.12; piece G295).
//!
//! What it does: a building with 2,000 identical doors, a tower of repeated floors or a car of
//! 30,000 parts stores each shape once, as a definition in the caller's [`Definitions`], cached by
//! content hash and kernel edition. An [`Assembly`] holds the caller's instance records (chapter
//! 19.12's one record, `Placed`, read through [`Record`]) and an [`InstanceTree`] over their boxes
//! whose leaves are record indices. A query finds the instances whose boxes meet a region, moves
//! the region into each one's definition frame, and queries that definition's own tree, or goes on
//! into a nested assembly. The modules:
//!
//! - [`record`] has [`Record`] and [`Place`], what the tree reads of a record.
//! - [`definitions`] has [`DefinitionKey`], [`Definitions`], [`Definition`], [`Content`] and
//!   [`MAX_NESTING`].
//! - [`tree`] has [`InstanceTree`], one level's tree.
//! - [`region`] has [`Region`], a query region with rational corners, exact in every frame.
//! - [`enter`] has [`Relation`] between frames, and moves a region through a placement.
//! - [`assembly`] has [`Assembly`], its checks and its queries, with [`Hit`] and [`Path`].
//! - [`error`] has [`InstanceError`].
//!
//! Guarantees:
//!
//! - **Every answer is brute force's, sorted.** Queries return record indices, or paths of them
//!   with part keys, ascending; the tree's shape never shows.
//! - **One record, one tree.** No second instance record (19.12), and the kernel's one box tree
//!   (19.9).
//!
//! Method: the two-level instance tree of chapter 19.12
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`); each module names its
//! own method.
//!
//! Cost: as each module states; about 140 bytes an instance with its record, measured by
//! `benches/instance.rs`. Recursion: none.

pub mod assembly;
pub mod definitions;
pub mod enter;
pub mod error;
pub mod record;
pub mod region;
pub mod tree;

pub use assembly::{Assembly, Hit, Path};
pub use definitions::{Content, Definition, DefinitionKey, Definitions, MAX_NESTING};
pub use enter::Relation;
pub use error::InstanceError;
pub use record::{Place, Record};
pub use region::Region;
pub use tree::InstanceTree;
