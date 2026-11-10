//! The replay hash of piece G90: the shared attribute step run over a fixed corpus, its outputs
//! hashed with SHA-256, must give the same digest on every target (chapters 21 and 28.13; the
//! golden store of piece G19).
//!
//! The corpus, from a fixed `SplitMix64` stream: 200 operations, each a history of up to 24
//! outputs (unchanged, changed, split, merged, modified from several, generated in a copy role or
//! another, from nothing), deleted inputs, three attributes with random behaviours (one of an
//! unknown owner, one a point moved by a transform in a third of the operations), values on up to
//! 32 input names; and every output value and note of the step, written as bytes. Any target that
//! orders a name, merges a value or drops a note differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use geom_name::{Mover, Note, Step, apply};
use geom_op::{History, NameId, NameSet, Origin, PieceKey, Record, Role, Side};
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};
use geom_topo::attributes::{AttrId, Attributes, Carried, MergeAction, SplitAction, Value};
use geom_topo::definition::{
    Behaviour, ChangeAction, CopyAction, Definition, Definitions, EntityKind, OnTransform,
    TransformAction, TransformKind, ValueSchema,
};
use geom_topo::order::NameKey;

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-name/attributes";

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

fn n(i: u64) -> NameId {
    let mut b = [0u8; 16];
    b[8..].copy_from_slice(&i.to_be_bytes());
    NameId::from_bytes(b)
}

/// Reverses a value's bytes: a stand-in for moving a point, deterministic and checkable.
struct Reverse;

impl Mover for Reverse {
    fn moved(&self, _: &ValueSchema, value: &Value) -> Option<Value> {
        (value.0.len() > 1).then(|| Value(value.0.iter().rev().copied().collect()))
    }
}

fn behaviour(rng: &mut SplitMix64, movable: bool) -> Behaviour {
    let t = |rng: &mut SplitMix64| match below(rng, 3) {
        0 => TransformAction::Keep,
        1 if movable => TransformAction::Transform,
        _ => TransformAction::Delete,
    };
    Behaviour {
        changed: if rng.coin() {
            ChangeAction::Keep
        } else {
            ChangeAction::Delete
        },
        transformed: OnTransform {
            moved: t(rng),
            rotated: t(rng),
            mirrored: t(rng),
            scaled: t(rng),
        },
        split: if rng.coin() {
            SplitAction::Propagate
        } else {
            SplitAction::Delete
        },
        merged: match below(rng, 3) {
            0 => MergeAction::KeepIfEqual,
            1 => MergeAction::Combine,
            _ => MergeAction::Delete,
        },
        copied: if rng.coin() {
            CopyAction::Copy
        } else {
            CopyAction::Delete
        },
    }
}

fn some_inputs(rng: &mut SplitMix64, k: u64) -> NameSet {
    let set: BTreeSet<u64> = (0..k).map(|_| below(rng, 32)).collect();
    NameSet::new(set.into_iter().map(n)).unwrap()
}

fn corpus() -> (Vec<u8>, usize) {
    let mut rng = SplitMix64::new(geom_test::seed::fixed(
        "geom-name attributes golden",
        Seed::DEFAULT,
    ));
    let copy = Role::new("instance", []).unwrap();
    let other = Role::new("side", []).unwrap();
    let mut bytes = Vec::new();
    let mut values_out = 0;
    for round in 0..200u64 {
        let attrs = [
            (
                AttrId {
                    owner: "kit.geom".into(),
                    name: "rating".into(),
                    version: 1,
                },
                ValueSchema::Opaque {
                    id: "kit/rating".into(),
                },
            ),
            (
                AttrId {
                    owner: "org.unknown".into(),
                    name: "x".into(),
                    version: 7,
                },
                ValueSchema::Opaque { id: "?".into() },
            ),
            (
                AttrId {
                    owner: "kit.geom".into(),
                    name: "datum".into(),
                    version: 2,
                },
                ValueSchema::Point,
            ),
        ];
        let mut defs = Definitions::new();
        let mut before = Attributes::new();
        for (a, schema) in &attrs {
            let b = behaviour(&mut rng, schema.movable());
            defs = defs
                .define(Definition::new(a.clone(), [EntityKind::Face], schema.clone(), b).unwrap())
                .unwrap();
            before = before.define(a.clone(), b.class()).unwrap();
            for _ in 0..below(&mut rng, 33) {
                let name = NameKey(n(below(&mut rng, 32)).to_bytes());
                let len = below(&mut rng, 4) as usize;
                let value = Value((0..len).map(|_| below(&mut rng, 4) as u8).collect());
                before = before.set(a, name, value).unwrap();
            }
        }
        let outputs: BTreeSet<u64> = (0..below(&mut rng, 25))
            .map(|_| below(&mut rng, 48))
            .collect();
        let records = outputs
            .into_iter()
            .map(|o| {
                let origin = match below(&mut rng, 7) {
                    0 => Origin::Unchanged,
                    1 => Origin::Modified {
                        from: NameSet::new([n(below(&mut rng, 32))]).unwrap(),
                    },
                    2 => Origin::Modified {
                        from: some_inputs(&mut rng, 3),
                    },
                    3 => Origin::Split {
                        of: n(below(&mut rng, 32)),
                        piece: PieceKey::ByTools {
                            tools: some_inputs(&mut rng, 2),
                            side: if rng.coin() { Side::Front } else { Side::Back },
                        },
                    },
                    4 => {
                        let mut of = some_inputs(&mut rng, 4);
                        while of.names().len() < 2 {
                            of = some_inputs(&mut rng, 4);
                        }
                        Origin::Merged { of }
                    }
                    5 => Origin::Generated {
                        from: some_inputs(&mut rng, 1),
                        role: copy.clone(),
                    },
                    _ => Origin::Generated {
                        from: some_inputs(&mut rng, 2),
                        role: other.clone(),
                    },
                };
                Record {
                    output: n(o),
                    origin,
                }
            })
            .collect::<Vec<_>>();
        let mut records = records;
        for r in &mut records {
            if let Origin::Modified { from } = &r.origin
                && from.names().is_empty()
            {
                r.origin = Origin::Unchanged;
            }
        }
        let history = History::new(records, some_inputs(&mut rng, 3), []).unwrap();
        let kind = TransformKind::ALL[below(&mut rng, 4) as usize];
        let copies = [copy.clone()];
        let step = Step {
            transform: (round % 3 == 0).then_some((kind, &Reverse as &dyn Mover)),
            copies: if rng.coin() { &copies } else { &[] },
        };
        let (after, notes) = apply(&defs, &before, &history, &step).unwrap();
        for (a, name, c) in after.iter() {
            values_out += 1;
            bytes.extend_from_slice(a.owner.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(a.name.as_bytes());
            bytes.extend_from_slice(&a.version.to_le_bytes());
            bytes.extend_from_slice(&name.0);
            match c {
                Carried::One(v) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&(v.0.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(&v.0);
                }
                Carried::Combined(each) => {
                    bytes.push(2);
                    bytes.extend_from_slice(&(each.len() as u32).to_le_bytes());
                    for (k, v) in each {
                        bytes.extend_from_slice(&k.0);
                        bytes.extend_from_slice(&(v.0.len() as u32).to_le_bytes());
                        bytes.extend_from_slice(&v.0);
                    }
                }
            }
        }
        for note in notes {
            let (tag, a, x) = match note {
                Note::Dropped { attr, into } => (3u8, attr, into),
                Note::NotMoved { attr, entity } => (4u8, attr, entity),
            };
            bytes.push(tag);
            bytes.extend_from_slice(a.name.as_bytes());
            bytes.extend_from_slice(&x.to_bytes());
        }
        bytes.push(0xff);
    }
    (bytes, values_out)
}

#[test]
fn attributes_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let (bytes, values) = corpus();
        (sha256(&bytes), (bytes, values))
    });
    let (bytes, values) = &checked.value;
    println!(
        "geom-name attributes corpus: {values} output values, {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert!(*values > 500, "only {values} output values");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target orders a name, merges a value or drops a \
             note differently (compare this log's digest with another target's), or a definition \
             changed in place, which chapter 26.2 forbids"
        )
    });
}
