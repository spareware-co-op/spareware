//! Attribute definitions: who owns an attribute, which entities may carry it, what its values are,
//! and what a value does at each of the eight events of section 8.4.
//!
//! What it does: a [`Definition`] gives an attribute's identity ([`AttrId`]: owner, name,
//! version), the [`EntityKind`]s that may carry it, its [`ValueSchema`], and its [`Behaviour`]: an
//! action for every event an operation's history can show (changed, transformed by each kind of
//! transform, split, merged, copied). Unchanged, generated and deleted entities need no action:
//! the value stays, starts absent, or goes with its entity. [`Definitions`] holds every definition
//! of a model, in `AttrId` order, and refuses to change one: a new behaviour or value schema is a
//! new version (8.4). `geom-name`'s attribute step applies these behaviours to an operation's
//! history (piece G90).
//!
//! Guarantees:
//!
//! - **A definition never changes.** [`Definitions::define`] accepts a definition already held only
//!   when it is identical; any difference is [`DefinitionError::Redefined`].
//! - **Only movable values move.** A behaviour may choose [`TransformAction::Transform`] only for
//!   a schema the kernel knows how to move (points, directions, frames, lengths and fields, 8.4);
//!   for an opaque schema [`Definition::new`] refuses it with
//!   [`DefinitionError::CannotTransform`], naming the transform.
//! - **Only named entities carry values.** The kinds are the body and its named entities: regions,
//!   faces, edges and vertices. Fins, loops and shells have no names and never carry attributes
//!   (8.4); a definition with no kind is [`DefinitionError::NoKinds`].
//! - **Ordered.** Definitions iterate in `AttrId` order, and each definition's kinds in their order,
//!   the same on every machine (8.2, 21).
//!
//! Method: section 8.4's events and actions, the proposal the owner accepted with the attribute
//! pieces (`projects/engines/geometry/architecture/08-topology.md`), after Parasolid's published
//! attribute classes, taken as a concept only (`projects/kit/notes/G01-evidence/07-persistent-naming.md`,
//! section 5).
//!
//! Cost: a definition is a few words plus its strings; [`Definitions::define`] and
//! [`Definitions::get`] are `O(log n)` in the number of definitions. Nothing recurses.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use core::fmt;

use crate::attributes::{AttrId, Class, MergeAction, SplitAction};

/// An entity that may carry an attribute: the body or one of its named entities (8.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    /// The body itself.
    Body,
    /// A region.
    Region,
    /// A face.
    Face,
    /// An edge.
    Edge,
    /// A vertex.
    Vertex,
}

/// What an attribute's values are (8.4): data the kernel only carries, or one of the kinds of value
/// it knows how to move with its entity (section 7's rules).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueSchema {
    /// Canonical bytes under the owner's schema, named by `id`; carried, never read or moved.
    Opaque {
        /// The owner's name for the schema.
        id: String,
    },
    /// A point.
    Point,
    /// A direction.
    Direction,
    /// A frame.
    Frame,
    /// A length.
    Length,
    /// A field over the entity (16.2).
    Field,
}

impl ValueSchema {
    /// Whether the kernel knows how to move a value of this schema with its entity.
    #[must_use]
    pub const fn movable(&self) -> bool {
        !matches!(self, ValueSchema::Opaque { .. })
    }
}

/// What a value does when its entity is changed from one input and keeps its name (8.4's Changed
/// event).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeAction {
    /// The value stays.
    Keep,
    /// The value goes.
    Delete,
}

/// A kind of transform, as a transform operation reports it (8.4's Transformed event, 10.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransformKind {
    /// A translation.
    Move,
    /// A rotation.
    Rotate,
    /// A mirror.
    Mirror,
    /// A scale.
    Scale,
}

impl TransformKind {
    /// Every kind, in this order.
    pub const ALL: [TransformKind; 4] = [
        TransformKind::Move,
        TransformKind::Rotate,
        TransformKind::Mirror,
        TransformKind::Scale,
    ];
}

impl fmt::Display for TransformKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TransformKind::Move => "move",
            TransformKind::Rotate => "rotate",
            TransformKind::Mirror => "mirror",
            TransformKind::Scale => "scale",
        })
    }
}

/// What a value does when its entity is moved by a transform operation (8.4's Transformed event).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransformAction {
    /// The value stays as it was.
    Keep,
    /// The value moves with the entity, by section 7's rules (movable schemas only).
    Transform,
    /// The value goes.
    Delete,
}

/// The action for each kind of transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OnTransform {
    /// For a translation.
    pub moved: TransformAction,
    /// For a rotation.
    pub rotated: TransformAction,
    /// For a mirror.
    pub mirrored: TransformAction,
    /// For a scale.
    pub scaled: TransformAction,
}

impl OnTransform {
    /// The same action for every kind of transform.
    #[must_use]
    pub const fn all(action: TransformAction) -> OnTransform {
        OnTransform {
            moved: action,
            rotated: action,
            mirrored: action,
            scaled: action,
        }
    }

    /// The action for `kind`.
    #[must_use]
    pub const fn get(&self, kind: TransformKind) -> TransformAction {
        match kind {
            TransformKind::Move => self.moved,
            TransformKind::Rotate => self.rotated,
            TransformKind::Mirror => self.mirrored,
            TransformKind::Scale => self.scaled,
        }
    }
}

/// What a value does when its entity is generated as a copy of one input, which the operation's
/// schema declares, as for a pattern's instances (8.4's Copied event).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CopyAction {
    /// The copy carries the value.
    Copy,
    /// The copy starts without it.
    Delete,
}

/// An attribute's behaviour: its action at every event of section 8.4 that has a choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Behaviour {
    /// Changed: modified from one input, keeping its name.
    pub changed: ChangeAction,
    /// Transformed, by each kind of transform.
    pub transformed: OnTransform,
    /// Split from one input.
    pub split: SplitAction,
    /// Merged from several inputs.
    pub merged: MergeAction,
    /// Generated from one input as its copy.
    pub copied: CopyAction,
}

impl Behaviour {
    /// The split and merge actions, as `Attributes` holds them.
    #[must_use]
    pub const fn class(&self) -> Class {
        Class {
            split: self.split,
            merge: self.merged,
        }
    }
}

/// Why a definition is refused.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DefinitionError {
    /// No kind of entity may carry it.
    NoKinds,
    /// Its behaviour moves values by this kind of transform, and its schema is opaque.
    CannotTransform(TransformKind),
    /// Another definition is already held under the same identity: a definition never changes.
    Redefined(AttrId),
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefinitionError::NoKinds => f.write_str("no kind of entity may carry the attribute"),
            DefinitionError::CannotTransform(kind) => write!(
                f,
                "the attribute's values are opaque, so it cannot move them under a {kind}"
            ),
            DefinitionError::Redefined(attr) => write!(
                f,
                "{}/{} version {} is already defined otherwise; a new behaviour is a new version",
                attr.owner, attr.name, attr.version
            ),
        }
    }
}

impl core::error::Error for DefinitionError {}

/// An attribute's definition (8.4).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Definition {
    attr: AttrId,
    kinds: BTreeSet<EntityKind>,
    schema: ValueSchema,
    behaviour: Behaviour,
}

impl Definition {
    /// The definition of `attr`, carried by `kinds`, with values of `schema`, behaving as
    /// `behaviour`.
    ///
    /// # Errors
    ///
    /// [`DefinitionError::NoKinds`] if `kinds` is empty; [`DefinitionError::CannotTransform`] for
    /// the first transform kind whose action moves values of an opaque schema.
    pub fn new(
        attr: AttrId,
        kinds: impl IntoIterator<Item = EntityKind>,
        schema: ValueSchema,
        behaviour: Behaviour,
    ) -> Result<Definition, DefinitionError> {
        let kinds: BTreeSet<EntityKind> = kinds.into_iter().collect();
        if kinds.is_empty() {
            return Err(DefinitionError::NoKinds);
        }
        if !schema.movable()
            && let Some(kind) = TransformKind::ALL
                .into_iter()
                .find(|&k| behaviour.transformed.get(k) == TransformAction::Transform)
        {
            return Err(DefinitionError::CannotTransform(kind));
        }
        Ok(Definition {
            attr,
            kinds,
            schema,
            behaviour,
        })
    }

    /// The attribute's identity.
    #[must_use]
    pub const fn attr(&self) -> &AttrId {
        &self.attr
    }

    /// Whether an entity of `kind` may carry it.
    #[must_use]
    pub fn allows(&self, kind: EntityKind) -> bool {
        self.kinds.contains(&kind)
    }

    /// The kinds that may carry it, in order.
    pub fn kinds(&self) -> impl Iterator<Item = EntityKind> + '_ {
        self.kinds.iter().copied()
    }

    /// Its values' schema.
    #[must_use]
    pub const fn schema(&self) -> &ValueSchema {
        &self.schema
    }

    /// Its behaviour.
    #[must_use]
    pub const fn behaviour(&self) -> &Behaviour {
        &self.behaviour
    }
}

/// Every attribute definition of a model, by identity (8.4: a file holds every definition its
/// bodies use).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Definitions {
    by_attr: BTreeMap<AttrId, Definition>,
}

impl Definitions {
    /// No definitions.
    #[must_use]
    pub const fn new() -> Definitions {
        Definitions {
            by_attr: BTreeMap::new(),
        }
    }

    /// With `definition` held. Holding an identical definition again changes nothing.
    ///
    /// # Errors
    ///
    /// [`DefinitionError::Redefined`] if another definition is held under the same identity.
    pub fn define(&self, definition: Definition) -> Result<Definitions, DefinitionError> {
        if let Some(held) = self.by_attr.get(definition.attr())
            && held != &definition
        {
            return Err(DefinitionError::Redefined(definition.attr.clone()));
        }
        let mut next = self.clone();
        next.by_attr.insert(definition.attr.clone(), definition);
        Ok(next)
    }

    /// The definition of `attr`, if held.
    #[must_use]
    pub fn get(&self, attr: &AttrId) -> Option<&Definition> {
        self.by_attr.get(attr)
    }

    /// Every definition, in `AttrId` order.
    pub fn iter(&self) -> impl Iterator<Item = &Definition> + '_ {
        self.by_attr.values()
    }
}
