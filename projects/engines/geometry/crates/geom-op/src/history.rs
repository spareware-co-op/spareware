//! History: how every output of an operation relates to its inputs, which inputs it removed and
//! what topology changes it made; the four laws of chapter 20.3; and how histories compose.
//!
//! What it does: a [`History`] holds one [`Record`] per output entity, sorted by name, saying
//! whether the output passed through unchanged, was modified from inputs, generated from inputs
//! (or from nothing) in a role, split from one input by cutting faces, or merged from several
//! (chapter 20.3); the set of inputs it deleted; and the [`TopologyChange`]s of chapter 13.14 it
//! made. [`History::check`] tests the laws every operation's history keeps. [`Relations`] is the
//! same history as three relations between inputs and outputs (modified, generated, deleted),
//! the form in which histories compose ([`Relations::then`]) across a chain of operations.
//!
//! Guarantees:
//!
//! - **Sorted by name, never by report order.** Records are in output-name order, each output
//!   once; changes and every set in them are in name order (20.2). Two operations that report the
//!   same history in different orders hold equal histories.
//! - **The four laws hold** for every history and composition. Law 1 (a deleted input has no
//!   modified output) and law 3 (every output traces to an input or is generated, and every input
//!   a record names is an input of the operation) are checked ([`History::check`],
//!   [`Relations::check`]). Law 2 (an input's generated and modified outputs do not overlap) holds
//!   by construction: a history gives each output one origin, and composition takes the modified
//!   pairs out of the generated ones. Law 4 (histories compose, keeping the others) is
//!   [`Relations::then`]; `geom-test`'s history kit checks laws 2 and 4 on generated chains.
//! - **Every topology change is recorded in the history** (13.14): a vanished face or an overshoot
//!   removed is deleted, faces merged are one `Merged` output of exactly those faces, and a face
//!   added is generated in the role the change gives.
//! - **Composition is associative and keeps the laws.** Composing follows the method note's three
//!   rules, with one more where a part reaches an input by both relations through a chain: there it
//!   is modified, not generated, because it carries on a part of that input (the "two paths" case,
//!   decided here and written into 20.3). Grouping a chain either way gives the same relations.
//!
//! Method: chapter 20.3's history contract and 13.14's topology-change records
//! (`projects/engines/geometry/architecture/20-naming-history.md`,
//! `projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`), after OCCT's
//! history contract as the method note records it, its composition rules derived there
//! (`projects/engines/geometry/methods/occt-history-contract.md`), and Kripac's face history graph
//! (doi:10.1016/S0010-4485(96)00040-1).
//!
//! Cost: making a history sorts its records and changes, `O(n log n)` in their number; a check and
//! a composition are `O(m log m)` in the number of relation pairs, on sorted vectors searched by
//! bisection, never hashed sets. Recursion: none.

use alloc::vec::Vec;
use core::fmt;

use crate::id::NameId;
use crate::name::{NameList, NameSet, PieceKey, Role};

/// How one output relates to the operation's inputs (chapter 20.3).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// An input that passed through unchanged: the output is that input, under its own name.
    Unchanged,
    /// The output is these inputs after a change: moved, trimmed, extended. A one-to-one change
    /// keeps the input's name (20.2).
    Modified {
        /// The inputs it carries on, at least one.
        from: NameSet,
    },
    /// The output is new, made in `role` from these inputs, or from none (a primitive's faces).
    Generated {
        /// The inputs it was made from; empty when it was made from nothing.
        from: NameSet,
        /// Its role.
        role: Role,
    },
    /// The output is one piece of an input that became several: modified from it.
    Split {
        /// The input split.
        of: NameId,
        /// Which piece: the cutting faces and the side.
        piece: PieceKey,
    },
    /// The output is several inputs that became one: modified from each.
    Merged {
        /// The inputs merged, at least two.
        of: NameSet,
    },
}

/// One output and its origin.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Record {
    /// The output's name.
    pub output: NameId,
    /// Where it came from.
    pub origin: Origin,
}

/// Why a face vanished (13.14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Vanish {
    /// It shrank to a line or a point.
    Collapsed,
    /// It turned inside out.
    InsideOut,
}

/// How a face's loops changed (13.14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoopChange {
    /// A hole in the face closed: its loop is deleted.
    HoleClosed,
    /// The face became several: its pieces are `Split` records.
    FaceSplit,
}

/// The classes of chapter 13.14's topology-change taxonomy, which a local operation declares and a
/// caller may narrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeClass {
    /// A vertex became several, joined by new edges.
    VertexSplit,
    /// A face shrank to nothing or turned inside out, and was removed.
    FaceVanished,
    /// Part of a face that would cross itself was cut away.
    SelfOverlapCut,
    /// Faces moved into each other were trimmed against each other.
    ClashTrimmed,
    /// Faces that passed right through each other were removed.
    OvershootRemoved,
    /// Neighbours ending on one carrier became one face.
    FacesMerged,
    /// A new face keeps the body closed.
    FaceAdded,
    /// A hole closed, or a face became several.
    LoopChanged,
    /// One solid became several.
    BodySplit,
}

/// One topology change a local operation made (13.14), recorded with its history.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TopologyChange {
    /// `vertex` became several, joined by `edges`.
    VertexSplit {
        /// The input vertex.
        vertex: NameId,
        /// The new edges, in name order.
        edges: NameList,
    },
    /// `face` vanished, and was deleted.
    FaceVanished {
        /// The input face.
        face: NameId,
        /// Why.
        why: Vanish,
    },
    /// Part of `face` was cut away along `along`.
    SelfOverlapCut {
        /// The input face.
        face: NameId,
        /// The new edges it was cut along, in name order.
        along: NameList,
    },
    /// `faces` were trimmed against each other.
    ClashTrimmed {
        /// The input faces.
        faces: NameSet,
    },
    /// `faces` passed through each other and were deleted.
    OvershootRemoved {
        /// The input faces.
        faces: NameSet,
    },
    /// `faces` became the one face `into`.
    FacesMerged {
        /// The input faces.
        faces: NameSet,
        /// The output face, `Merged` from exactly them.
        into: NameId,
    },
    /// `face` was added in `role`.
    FaceAdded {
        /// The output face, generated in that role.
        face: NameId,
        /// Its role.
        role: Role,
    },
    /// `face`'s loops changed.
    LoopChanged {
        /// The input face.
        face: NameId,
        /// How.
        change: LoopChange,
    },
    /// One solid became `lumps`.
    BodySplit {
        /// The output solids, in name order.
        lumps: NameList,
    },
}

impl TopologyChange {
    /// The change's class in 13.14's taxonomy.
    #[must_use]
    pub const fn class(&self) -> ChangeClass {
        match self {
            TopologyChange::VertexSplit { .. } => ChangeClass::VertexSplit,
            TopologyChange::FaceVanished { .. } => ChangeClass::FaceVanished,
            TopologyChange::SelfOverlapCut { .. } => ChangeClass::SelfOverlapCut,
            TopologyChange::ClashTrimmed { .. } => ChangeClass::ClashTrimmed,
            TopologyChange::OvershootRemoved { .. } => ChangeClass::OvershootRemoved,
            TopologyChange::FacesMerged { .. } => ChangeClass::FacesMerged,
            TopologyChange::FaceAdded { .. } => ChangeClass::FaceAdded,
            TopologyChange::LoopChanged { .. } => ChangeClass::LoopChanged,
            TopologyChange::BodySplit { .. } => ChangeClass::BodySplit,
        }
    }
}

/// Why records cannot make a history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryError {
    /// Two records for one output.
    RepeatedOutput {
        /// The output.
        output: NameId,
    },
    /// A `Modified` record from no input: an output made from nothing is `Generated`.
    ModifiedFromNothing {
        /// The output.
        output: NameId,
    },
    /// A `Merged` record of fewer than two inputs: one input changed is `Modified`.
    MergeOfOne {
        /// The output.
        output: NameId,
    },
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HistoryError::RepeatedOutput { output } => {
                write!(f, "the output {output} has two records")
            }
            HistoryError::ModifiedFromNothing { output } => write!(
                f,
                "the output {output} is modified from no input; one made from nothing is generated"
            ),
            HistoryError::MergeOfOne { output } => write!(
                f,
                "the output {output} is merged from fewer than two inputs; one input changed is \
                 modified"
            ),
        }
    }
}

impl core::error::Error for HistoryError {}

/// Which law of chapter 20.3 (or 13.14's recording rule) a history breaks, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LawBreak {
    /// Law 1: a deleted input has a modified output (or passed through unchanged).
    DeletedButModified {
        /// The input.
        input: NameId,
        /// The output modified from it.
        output: NameId,
    },
    /// Law 3: a record or the deleted set names something that is not an input.
    NotAnInput {
        /// The name.
        name: NameId,
    },
    /// 13.14: a topology change the records and the deleted set do not show.
    ChangeNotRecorded {
        /// The change's class.
        class: ChangeClass,
        /// The entity it concerns that the history does not record as the change says.
        entity: NameId,
    },
}

impl fmt::Display for LawBreak {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LawBreak::DeletedButModified { input, output } => write!(
                f,
                "law 1: the deleted input {input} has the modified output {output}"
            ),
            LawBreak::NotAnInput { name } => {
                write!(f, "law 3: {name} is named as an input but is not one")
            }
            LawBreak::ChangeNotRecorded { class, entity } => write!(
                f,
                "the {class:?} change of {entity} is not recorded in the history (13.14)"
            ),
        }
    }
}

impl core::error::Error for LawBreak {}

/// What an operation did to its inputs (chapter 20.3).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct History {
    records: Vec<Record>,
    deleted: NameSet,
    changes: Vec<TopologyChange>,
}

impl History {
    /// The history of these records, deleted inputs and topology changes, each sorted by name.
    ///
    /// # Errors
    ///
    /// [`HistoryError`] for two records of one output, a `Modified` record from no input, or a
    /// `Merged` record of fewer than two inputs.
    pub fn new(
        records: impl IntoIterator<Item = Record>,
        deleted: NameSet,
        changes: impl IntoIterator<Item = TopologyChange>,
    ) -> Result<History, HistoryError> {
        let mut records: Vec<Record> = records.into_iter().collect();
        records.sort_unstable_by_key(|r| r.output);
        if let Some(output) = records.windows(2).find_map(|w| match w {
            [a, b] if a.output == b.output => Some(a.output),
            _ => None,
        }) {
            return Err(HistoryError::RepeatedOutput { output });
        }
        for record in &records {
            match &record.origin {
                Origin::Modified { from } if from.names().is_empty() => {
                    return Err(HistoryError::ModifiedFromNothing {
                        output: record.output,
                    });
                }
                Origin::Merged { of } if of.names().len() < 2 => {
                    return Err(HistoryError::MergeOfOne {
                        output: record.output,
                    });
                }
                _ => {}
            }
        }
        let mut changes: Vec<TopologyChange> = changes.into_iter().collect();
        changes.sort_unstable();
        Ok(History {
            records,
            deleted,
            changes,
        })
    }

    /// The records, in output-name order.
    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// The inputs deleted.
    #[must_use]
    pub fn deleted(&self) -> &NameSet {
        &self.deleted
    }

    /// The topology changes, in order.
    #[must_use]
    pub fn changes(&self) -> &[TopologyChange] {
        &self.changes
    }

    /// The origin of `output`, if the history has it.
    #[must_use]
    pub fn origin(&self, output: NameId) -> Option<&Origin> {
        self.records
            .binary_search_by(|r| r.output.cmp(&output))
            .ok()
            .and_then(|at| self.records.get(at))
            .map(|r| &r.origin)
    }

    /// The history as relations between inputs and outputs.
    #[must_use]
    pub fn relations(&self) -> Relations {
        let mut outputs = Vec::new();
        let mut modified = Vec::new();
        let mut generated = Vec::new();
        let mut from_nothing = Vec::new();
        for record in &self.records {
            let o = record.output;
            outputs.push(o);
            match &record.origin {
                Origin::Unchanged => modified.push((o, o)),
                Origin::Modified { from: inputs } | Origin::Merged { of: inputs } => {
                    modified.extend(inputs.names().iter().map(|&i| (i, o)));
                }
                Origin::Split { of, .. } => modified.push((*of, o)),
                Origin::Generated { from, .. } => {
                    if from.names().is_empty() {
                        from_nothing.push(o);
                    }
                    generated.extend(from.names().iter().map(|&i| (i, o)));
                }
            }
        }
        Relations {
            outputs: sorted(outputs),
            modified: sorted(modified),
            generated: sorted(generated),
            from_nothing: sorted(from_nothing),
            deleted: self.deleted.names().to_vec(),
        }
    }

    /// Check the history of an operation with these inputs against laws 1 and 3 of chapter 20.3
    /// (law 2 holds by construction) and 13.14's recording rule.
    ///
    /// # Errors
    ///
    /// The first [`LawBreak`] found: laws in order, then the changes in order.
    pub fn check(&self, inputs: &NameSet) -> Result<(), LawBreak> {
        self.relations().check(inputs)?;
        for change in &self.changes {
            self.check_change(change)?;
        }
        Ok(())
    }

    /// 13.14's rule for one change: it shows in the records or the deleted set as its row says.
    fn check_change(&self, change: &TopologyChange) -> Result<(), LawBreak> {
        let class = change.class();
        let missing = |entity: NameId| LawBreak::ChangeNotRecorded { class, entity };
        let deleted = |face: NameId| self.deleted.names().binary_search(&face).is_ok();
        match change {
            TopologyChange::FaceVanished { face, .. } => {
                if !deleted(*face) {
                    return Err(missing(*face));
                }
            }
            TopologyChange::OvershootRemoved { faces } => {
                if let Some(face) = faces.names().iter().find(|&&f| !deleted(f)) {
                    return Err(missing(*face));
                }
            }
            TopologyChange::FacesMerged { faces, into } => {
                if !matches!(self.origin(*into), Some(Origin::Merged { of }) if of == faces) {
                    return Err(missing(*into));
                }
            }
            TopologyChange::FaceAdded { face, role } => {
                if !matches!(self.origin(*face), Some(Origin::Generated { role: r, .. }) if r == role)
                {
                    return Err(missing(*face));
                }
            }
            TopologyChange::VertexSplit { .. }
            | TopologyChange::SelfOverlapCut { .. }
            | TopologyChange::ClashTrimmed { .. }
            | TopologyChange::LoopChanged { .. }
            | TopologyChange::BodySplit { .. } => {}
        }
        Ok(())
    }
}

/// `values` sorted, each once.
fn sorted<T: Ord>(mut values: Vec<T>) -> Vec<T> {
    values.sort_unstable();
    values.dedup();
    values
}

/// Whether the sorted `values` hold `value`.
fn holds<T: Ord>(values: &[T], value: &T) -> bool {
    values.binary_search(value).is_ok()
}

/// The outputs paired with `input` in the sorted `pairs`.
fn images(pairs: &[(NameId, NameId)], input: NameId) -> &[(NameId, NameId)] {
    let start = pairs.partition_point(|&(i, _)| i < input);
    let end = pairs.partition_point(|&(i, _)| i <= input);
    pairs.get(start..end).unwrap_or_default()
}

/// A history as relations between inputs and outputs: the form in which histories compose
/// (chapter 20.3; the method note's three relations). An unchanged output is its own modified
/// image. Every list is sorted and holds each item once.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Relations {
    outputs: Vec<NameId>,
    modified: Vec<(NameId, NameId)>,
    generated: Vec<(NameId, NameId)>,
    from_nothing: Vec<NameId>,
    deleted: Vec<NameId>,
}

impl Relations {
    /// The outputs, in name order.
    pub fn outputs(&self) -> impl Iterator<Item = NameId> + '_ {
        self.outputs.iter().copied()
    }

    /// The pairs (input, output) where the output is modified from the input, in order.
    pub fn modified(&self) -> impl Iterator<Item = (NameId, NameId)> + '_ {
        self.modified.iter().copied()
    }

    /// The pairs (input, output) where the output is generated from the input, in order.
    pub fn generated(&self) -> impl Iterator<Item = (NameId, NameId)> + '_ {
        self.generated.iter().copied()
    }

    /// The outputs generated from nothing, in name order.
    pub fn from_nothing(&self) -> impl Iterator<Item = NameId> + '_ {
        self.from_nothing.iter().copied()
    }

    /// The inputs deleted, in name order.
    pub fn deleted(&self) -> impl Iterator<Item = NameId> + '_ {
        self.deleted.iter().copied()
    }

    /// Check laws 1 and 3 of chapter 20.3 for an operation, or a chain, with these inputs (law 2
    /// holds by construction).
    ///
    /// # Errors
    ///
    /// The first [`LawBreak`] found, laws in order.
    pub fn check(&self, inputs: &NameSet) -> Result<(), LawBreak> {
        // Law 1: a deleted input has no modified outputs.
        if let Some(&(input, output)) = self.modified.iter().find(|(i, _)| holds(&self.deleted, i))
        {
            return Err(LawBreak::DeletedButModified { input, output });
        }
        // Law 3: every output traces to an input or is generated, and everything named as an
        // input is one.
        let named = self
            .modified
            .iter()
            .chain(&self.generated)
            .map(|&(i, _)| i)
            .chain(self.deleted.iter().copied());
        for name in named {
            if !holds(inputs.names(), &name) {
                return Err(LawBreak::NotAnInput { name });
            }
        }
        Ok(())
    }

    /// The relations of this step followed by `next`, whose inputs are this step's outputs (law
    /// 4). An output of the chain is modified from an input when it is a modified image of one of
    /// its modified images; generated from it when it traces to it through any other path; made
    /// from nothing when it traces to no input; and an input is deleted by the chain when this
    /// step deleted it, or when it had modified images here and none has a modified image after
    /// `next`.
    #[must_use]
    pub fn then(&self, next: &Relations) -> Relations {
        let mut modified = Vec::new();
        let mut traced = Vec::new();
        for &(i, m) in &self.modified {
            for &(_, o) in images(&next.modified, m) {
                modified.push((i, o));
                traced.push((i, o));
            }
            for &(_, o) in images(&next.generated, m) {
                traced.push((i, o));
            }
        }
        for &(i, m) in &self.generated {
            for &(_, o) in images(&next.modified, m)
                .iter()
                .chain(images(&next.generated, m))
            {
                traced.push((i, o));
            }
        }
        let modified = sorted(modified);
        // Two paths to one part: modified wins (20.3).
        let mut generated = sorted(traced);
        let reached = sorted(generated.iter().map(|&(_, o)| o).collect());
        generated.retain(|pair| !holds(&modified, pair));
        let from_nothing = next
            .outputs
            .iter()
            .copied()
            .filter(|o| !holds(&reached, o))
            .collect();
        let kept = sorted(modified.iter().map(|&(i, _)| i).collect());
        let mut deleted = self.deleted.clone();
        deleted.extend(
            self.modified
                .iter()
                .map(|&(i, _)| i)
                .filter(|i| !holds(&kept, i)),
        );
        Relations {
            outputs: next.outputs.clone(),
            modified,
            generated,
            from_nothing,
            deleted: sorted(deleted),
        }
    }
}
