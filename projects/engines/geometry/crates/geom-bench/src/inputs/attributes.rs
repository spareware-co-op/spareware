//! Inputs for the attribute step's hot path (piece G90, decision 32.44): one attribute over a small
//! body's entities, carried across an operation that leaves some unchanged, changes some and
//! deletes the rest.
//!
//! Class `typical` only: each input is a [`Scene`] of [`ENTITIES`] values of one attribute that
//! keeps everything it can, with a third of its entities unchanged, a third changed and a third
//! deleted. The step's refusals (no definition, a class that differs) are tested in `geom-name`
//! itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the first scene of `geom-name`'s `tests/classes.rs`, at one size, measured as chapter
//! 28.14 sets benchmarks out, `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few hundred small allocations per input, once per batch, outside every measured run.

use geom_op::{History, NameId, NameSet, Origin, Record};
use geom_topo::attributes::{AttrId, Attributes, MergeAction, SplitAction, Value};
use geom_topo::definition::{
    Behaviour, ChangeAction, CopyAction, Definition, Definitions, EntityKind, OnTransform,
    TransformAction, ValueSchema,
};
use geom_topo::order::NameKey;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// The entities of one scene.
pub const ENTITIES: u8 = 24;

/// What `geom_name::apply` takes.
#[derive(Clone, Debug)]
pub struct Scene {
    /// The attribute's definition.
    pub defs: Definitions,
    /// The values before the operation.
    pub before: Attributes,
    /// What the operation did.
    pub history: History,
}

/// A behaviour that keeps everything it can.
const fn keeping() -> Behaviour {
    Behaviour {
        changed: ChangeAction::Keep,
        transformed: OnTransform::all(TransformAction::Keep),
        split: SplitAction::Propagate,
        merged: MergeAction::KeepIfEqual,
        copied: CopyAction::Copy,
    }
}

/// Entity `i`'s name, drawn from `base`.
fn name(base: u64, i: u8) -> [u8; 16] {
    let mut k = [i; 16];
    let (high, _) = k.split_at_mut(8);
    high.copy_from_slice(&base.to_be_bytes());
    k
}

/// A scene with fresh names and values.
fn scene(g: &mut SplitMix64) -> Option<Scene> {
    let a = AttrId {
        owner: "kit.bench".to_owned(),
        name: "rating".to_owned(),
        version: 1,
    };
    let schema = ValueSchema::Opaque {
        id: "kit.bench/rating".to_owned(),
    };
    let behaviour = keeping();
    let defs = Definitions::new()
        .define(Definition::new(a.clone(), [EntityKind::Face], schema, behaviour).ok()?)
        .ok()?;
    let base = g.next_u64();
    let mut before = Attributes::new().define(a.clone(), behaviour.class())?;
    let mut records = Vec::new();
    let mut deleted = Vec::new();
    for i in 0..ENTITIES {
        let bytes = name(base, i);
        let id = NameId::from_bytes(bytes);
        before = before.set(
            &a,
            NameKey(bytes),
            Value(g.next_u64().to_be_bytes().to_vec()),
        )?;
        match i % 3 {
            0 => records.push(Record {
                output: id,
                origin: Origin::Unchanged,
            }),
            1 => records.push(Record {
                output: id,
                origin: Origin::Modified {
                    from: NameSet::new([id]).ok()?,
                },
            }),
            _ => deleted.push(id),
        }
    }
    let history = History::new(records, NameSet::new(deleted).ok()?, []).ok()?;
    Some(Scene {
        defs,
        before,
        history,
    })
}

/// Scenes for `geom_name::apply`.
#[must_use]
pub fn apply(class: Class) -> Vec<Scene> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("attributes::apply", class);
    batch_of(|_| scene(&mut g))
}
