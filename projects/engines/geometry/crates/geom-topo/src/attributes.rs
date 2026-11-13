//! Attributes: data hung on a body or on one of its named entities, keyed by persistent name, with
//! the class that says what a value does when its entity is split or merged (8.4).
//!
//! What it does: an [`Attributes`] holds each attribute's [`Class`] and every value, keyed by the
//! entity's [`NameKey`] and the attribute's [`AttrId`]. Values are canonical bytes ([`Value`]),
//! carried whether or not this kernel understands them. [`Attributes::split`] and
//! [`Attributes::merge`] give the values an operation's split and merged entities carry, by each
//! attribute's class; the other events of section 8.4 (changed, transformed, copied, generated)
//! come with the history records (pieces G88 and G90).
//!
//! Guarantees:
//!
//! - **Keyed by name, never by index.** Values follow names, so they survive every re-indexing a
//!   publish makes (8.2), and only named entities carry them (8.4).
//! - **Ordered.** Every map iterates by key: attribute ids, then names, as bytes, the same on every
//!   machine (8.2, 21).
//! - **Values are bytes.** Two values are equal exactly when their bytes are (8.4); a merge that
//!   keeps a value only when every input carries an equal one compares bytes.
//! - **Immutable.** Every operation returns a new [`Attributes`]; nothing is changed in place, as
//!   for bodies (8.2).
//!
//! Method: Parasolid's published attribute classes, taken as a concept only and rebuilt on
//! persistent names (`projects/engines/geometry/architecture/08-topology.md`, 8.4;
//! `projects/kit/notes/G01-evidence/07-persistent-naming.md`, section 5).
//!
//! Cost: a lookup or an insert is `O(log n)` in the number of values; a split or a merge visits
//! the values of its inputs only. Nothing recurses.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::order::NameKey;

/// An attribute's identity: its owner (`kit.geom` for the kernel's own, otherwise a program's or
/// plugin's id), its name and its version (8.4). A new class or value schema is a new version.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttrId {
    /// Who defines it.
    pub owner: String,
    /// Its name within the owner's attributes.
    pub name: String,
    /// Its version.
    pub version: u32,
}

/// A value: canonical bytes (8.4), carried whether or not this kernel understands them.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Value(pub Vec<u8>);

/// What a value does when its entity is split from one input (8.4's Split event).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SplitAction {
    /// Every piece gets the value.
    Propagate,
    /// The pieces start without it.
    Delete,
}

/// What a value does when its entity is merged from several inputs (8.4's Merged event).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MergeAction {
    /// Kept only when every input carries an equal value; otherwise dropped, with a diagnostic.
    KeepIfEqual,
    /// The merged entity carries each input's value, keyed by that input's name.
    Combine,
    /// The merged entity starts without it.
    Delete,
}

/// An attribute's class: what its values do at a split and at a merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Class {
    /// At a split.
    pub split: SplitAction,
    /// At a merge.
    pub merge: MergeAction,
}

/// What an entity carries for one attribute: one value, or, after a combining merge, each input's
/// value keyed by that input's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Carried {
    /// One value.
    One(Value),
    /// Each merged input's value, by the input's name.
    Combined(BTreeMap<NameKey, Value>),
}

/// Why a merge dropped a value: its inputs carried different values under
/// [`MergeAction::KeepIfEqual`] (8.4: "otherwise dropped with a diagnostic").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dropped {
    /// The attribute.
    pub attr: AttrId,
    /// The merged entity.
    pub into: NameKey,
}

/// Every attribute's class and every value, of one body version.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attributes {
    /// Each attribute's class.
    classes: BTreeMap<AttrId, Class>,
    /// Every value, by attribute and then by the name of the entity carrying it.
    values: BTreeMap<AttrId, BTreeMap<NameKey, Carried>>,
}

impl Attributes {
    /// No attributes.
    #[must_use]
    pub fn new() -> Attributes {
        Attributes::default()
    }

    /// With `attr` defined with `class`. A definition never changes (8.4): defining an attribute
    /// already defined with another class returns `None`.
    #[must_use]
    pub fn define(&self, attr: AttrId, class: Class) -> Option<Attributes> {
        match self.classes.get(&attr) {
            Some(c) if *c != class => None,
            _ => {
                let mut next = self.clone();
                next.classes.insert(attr, class);
                Some(next)
            }
        }
    }

    /// The class of `attr`, if it is defined.
    #[must_use]
    pub fn class(&self, attr: &AttrId) -> Option<Class> {
        self.classes.get(attr).copied()
    }

    /// With `entity` carrying `value` for `attr`, replacing what it carried. `None` when `attr` is
    /// not defined: every value has a class.
    #[must_use]
    pub fn set(&self, attr: &AttrId, entity: NameKey, value: Value) -> Option<Attributes> {
        self.classes.get(attr)?;
        let mut next = self.clone();
        next.values
            .entry(attr.clone())
            .or_default()
            .insert(entity, Carried::One(value));
        Some(next)
    }

    /// Without `entity`'s value for `attr`.
    #[must_use]
    pub fn remove(&self, attr: &AttrId, entity: NameKey) -> Attributes {
        let mut next = self.clone();
        if let Some(by_name) = next.values.get_mut(attr) {
            by_name.remove(&entity);
            if by_name.is_empty() {
                next.values.remove(attr);
            }
        }
        next
    }

    /// What `entity` carries for `attr`.
    #[must_use]
    pub fn get(&self, attr: &AttrId, entity: NameKey) -> Option<&Carried> {
        self.values
            .get(attr)
            .and_then(|by_name| by_name.get(&entity))
    }

    /// The same classes with exactly these values, as an attribute step computes them afresh from
    /// a history (piece G90). A value whose attribute has no class here is left out: every value
    /// has a class.
    #[must_use]
    pub fn rebuilt(
        &self,
        values: impl IntoIterator<Item = (AttrId, NameKey, Carried)>,
    ) -> Attributes {
        let mut next = Attributes {
            classes: self.classes.clone(),
            values: BTreeMap::new(),
        };
        for (attr, entity, carried) in values {
            if self.classes.contains_key(&attr) {
                next.values.entry(attr).or_default().insert(entity, carried);
            }
        }
        next
    }

    /// The same attributes with `attr`'s class set to `class`, whatever it was: for a definition
    /// already checked against the class held (piece G90).
    #[must_use]
    pub fn defined_as(&self, attr: AttrId, class: Class) -> Attributes {
        let mut next = self.clone();
        next.classes.insert(attr, class);
        next
    }

    /// Every (attribute, entity, carried) triple, attributes then names in order.
    pub fn iter(&self) -> impl Iterator<Item = (&AttrId, &NameKey, &Carried)> {
        self.values
            .iter()
            .flat_map(|(a, by_name)| by_name.iter().map(move |(n, c)| (a, n, c)))
    }

    /// After `of` was split into `pieces`: each attribute's value on `of` goes to every piece
    /// ([`SplitAction::Propagate`]) or to none ([`SplitAction::Delete`]); `of` no longer carries it.
    #[must_use]
    pub fn split(&self, of: NameKey, pieces: &[NameKey]) -> Attributes {
        let mut next = self.clone();
        for (attr, by_name) in &mut next.values {
            let Some(carried) = by_name.remove(&of) else {
                continue;
            };
            if self.class(attr).map(|c| c.split) == Some(SplitAction::Propagate) {
                for &p in pieces {
                    by_name.insert(p, carried.clone());
                }
            }
        }
        next.values.retain(|_, by_name| !by_name.is_empty());
        next
    }

    /// After the entities `of` were merged into `into`: for each attribute, by its class, `into`
    /// keeps a value every input carries equally ([`MergeAction::KeepIfEqual`]; any difference,
    /// or an input without it, drops it and is reported), carries each input's value by name
    /// ([`MergeAction::Combine`]), or nothing ([`MergeAction::Delete`]). The inputs no longer
    /// carry it.
    #[must_use]
    pub fn merge(&self, of: &[NameKey], into: NameKey) -> (Attributes, Vec<Dropped>) {
        let mut next = self.clone();
        let mut dropped = Vec::new();
        for (attr, by_name) in &mut next.values {
            let carried: Vec<(NameKey, Carried)> = of
                .iter()
                .filter_map(|&n| by_name.remove(&n).map(|c| (n, c)))
                .collect();
            if carried.is_empty() {
                continue;
            }
            let action = self.class(attr).map_or(MergeAction::Delete, |c| c.merge);
            match action {
                MergeAction::Delete => {}
                MergeAction::KeepIfEqual => {
                    let first = carried.first().map(|(_, c)| c.clone());
                    let all_equal = carried.len() == of.len()
                        && carried.iter().all(|(_, c)| Some(c) == first.as_ref());
                    match first {
                        Some(c) if all_equal => {
                            by_name.insert(into, c);
                        }
                        _ => dropped.push(Dropped {
                            attr: attr.clone(),
                            into,
                        }),
                    }
                }
                MergeAction::Combine => {
                    let mut each = BTreeMap::new();
                    for (n, c) in carried {
                        match c {
                            Carried::One(v) => {
                                each.insert(n, v);
                            }
                            Carried::Combined(inner) => each.extend(inner),
                        }
                    }
                    by_name.insert(into, Carried::Combined(each));
                }
            }
        }
        next.values.retain(|_, by_name| !by_name.is_empty());
        (next, dropped)
    }
}
