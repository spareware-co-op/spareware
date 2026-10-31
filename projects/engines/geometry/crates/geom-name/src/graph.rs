//! The name graph: every persistent name once, hash-consed, with the canonical bytes its `NameId`
//! was hashed from.
//!
//! What it does: a [`NameGraph`] holds name nodes keyed by their [`NameId`]. [`NameGraph::intern`]
//! encodes a [`TopoName`], hashes it under `NameScheme v1` and keeps the node; interning the same
//! name again returns the same `NameId` and stores nothing new. A name refers to its parents by
//! `NameId` ([`parents`]), so the nodes form a shared graph rather than nested trees.
//! [`NameGraph::is_weak`] says whether a name is a `Weak` node or is built on one at any depth.
//!
//! Guarantees:
//!
//! - **One encoding per `NameId`.** A node keeps its canonical bytes beside its `NameId`. A second,
//!   different encoding under a `NameId` already held is refused with [`GraphError::Collision`],
//!   never merged with the first (32.19).
//! - **Name order.** Nodes are held and listed in the byte order of their `NameId`s (20.2:
//!   "everything is sorted by name"), whatever order they were interned in.
//! - **Weakness is exact.** A name is weak exactly when it is a `Weak` node or reaches one through
//!   its parents. A parent the graph does not hold is [`GraphError::Missing`], never assumed strong.
//! - **No recursion.** The walk through parents uses an explicit stack and visits each node once.
//!
//! Method: hash-consing, as the shared name graph of chapter 20.1
//! (`projects/engines/geometry/architecture/20-naming-history.md`), with decision 32.19's rule that
//! the graph keeps each node's canonical bytes and refuses a second encoding under one `NameId`
//! (`projects/engines/geometry/architecture/32-decisions.md`); the weakness rule of 20.2.
//!
//! Cost: interning is one encoding and one SHA-256 of a few hundred bytes, then a `BTreeMap`
//! lookup, O(log n). [`NameGraph::is_weak`] visits each reachable node once, O(r log n) for r
//! reachable nodes, with a stack and a visited set of at most r entries. Recursion: none.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::fmt;

use geom_op::NameId;
use geom_op::name::{NameError, PieceKey, RoleArg, TopoName, name_id};

/// Why the graph refused a name or could not answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphError {
    /// A name whose `NameId` the graph already holds with different canonical bytes.
    Collision(NameId),
    /// A name the walk reached that the graph does not hold.
    Missing(NameId),
    /// The name could not be encoded.
    Name(NameError),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::Collision(id) => write!(f, "two different names hash to {id}"),
            GraphError::Missing(id) => write!(f, "the name {id} is not in the graph"),
            GraphError::Name(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for GraphError {}

/// One node: the name and the canonical bytes its `NameId` was hashed from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Node {
    name: TopoName,
    bytes: Vec<u8>,
}

/// The hash-consed name graph.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameGraph {
    nodes: BTreeMap<NameId, Node>,
}

impl NameGraph {
    /// An empty graph.
    #[must_use]
    pub fn new() -> NameGraph {
        NameGraph::default()
    }

    /// Interns `name`: its `NameId`, keeping the node if the graph did not hold it.
    ///
    /// # Errors
    ///
    /// [`GraphError::Collision`] if the graph holds a different name under the same `NameId`, and
    /// [`GraphError::Name`] if the name cannot be encoded.
    pub fn intern(&mut self, name: TopoName) -> Result<NameId, GraphError> {
        let bytes = name.encode().map_err(GraphError::Name)?.into_bytes();
        let id = name_id(&bytes);
        self.insert(id, name, bytes)
    }

    /// Keeps `name`, with canonical `bytes`, under `id`, or checks that the node already there is
    /// the same.
    fn insert(&mut self, id: NameId, name: TopoName, bytes: Vec<u8>) -> Result<NameId, GraphError> {
        match self.nodes.get(&id) {
            Some(node) if node.bytes == bytes => Ok(id),
            Some(_) => Err(GraphError::Collision(id)),
            None => {
                self.nodes.insert(id, Node { name, bytes });
                Ok(id)
            }
        }
    }

    /// The name held under `id`.
    #[must_use]
    pub fn name(&self, id: NameId) -> Option<&TopoName> {
        self.nodes.get(&id).map(|node| &node.name)
    }

    /// The canonical bytes of the name held under `id`.
    #[must_use]
    pub fn bytes(&self, id: NameId) -> Option<&[u8]> {
        self.nodes.get(&id).map(|node| node.bytes.as_slice())
    }

    /// How many names the graph holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the graph holds no name.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Every `NameId` the graph holds, in name order.
    pub fn ids(&self) -> impl Iterator<Item = NameId> + '_ {
        self.nodes.keys().copied()
    }

    /// Whether the name under `id` is weak: a `Weak` node, or built on one at any depth (20.2).
    ///
    /// # Errors
    ///
    /// [`GraphError::Missing`] for the first name the walk reaches that the graph does not hold.
    pub fn is_weak(&self, id: NameId) -> Result<bool, GraphError> {
        let mut stack = Vec::from([id]);
        let mut seen = BTreeSet::from([id]);
        while let Some(next) = stack.pop() {
            let name = self.name(next).ok_or(GraphError::Missing(next))?;
            if matches!(name, TopoName::Weak { .. }) {
                return Ok(true);
            }
            for parent in parents(name) {
                if seen.insert(parent) {
                    stack.push(parent);
                }
            }
        }
        Ok(false)
    }
}

/// The names `name` refers to, in the order its encoding holds them.
#[must_use]
pub fn parents(name: &TopoName) -> Vec<NameId> {
    match name {
        TopoName::Created { role, from, .. } => role
            .args()
            .iter()
            .filter_map(|arg| match arg {
                RoleArg::Name(id) => Some(*id),
                RoleArg::Count(_) => None,
            })
            .chain(from.names().iter().copied())
            .collect(),
        TopoName::Split { of, piece, .. } => {
            let PieceKey::ByTools { tools, .. } = piece;
            core::iter::once(*of)
                .chain(tools.names().iter().copied())
                .collect()
        }
        TopoName::Merged { of, .. } => of.names().to_vec(),
        TopoName::Bounded { by, ends, .. } => {
            by.names().iter().chain(ends.names()).copied().collect()
        }
        TopoName::Weak { base, .. } => Vec::from([*base]),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "a failing test should stop loudly")]

    use super::{GraphError, NameGraph};
    use geom_op::NameId;
    use geom_op::name::{Ordinal, TopoName};

    #[test]
    fn a_second_encoding_under_a_held_name_id_is_refused() {
        let mut graph = NameGraph::new();
        let name = TopoName::Weak {
            base: NameId::from_bytes([1; 16]),
            ord: Ordinal::new(0, 2).unwrap(),
        };
        let id = graph.intern(name.clone()).unwrap();
        let other = TopoName::Weak {
            base: NameId::from_bytes([1; 16]),
            ord: Ordinal::new(1, 2).unwrap(),
        };
        let other_bytes = other.encode().unwrap().into_bytes();
        // Forge the collision a SHA-256 cut to 128 bits makes all but impossible: the other name's
        // bytes under the first name's NameId.
        assert_eq!(
            graph.insert(id, other, other_bytes),
            Err(GraphError::Collision(id))
        );
        assert_eq!(graph.name(id), Some(&name));
        assert_eq!(graph.len(), 1);
    }
}
