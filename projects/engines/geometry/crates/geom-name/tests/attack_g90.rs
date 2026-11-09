//! Attacks on every claim of piece G90, "Attribute definitions and classes" (chapter 30):
//! definitions with owner, version, entity kinds, value schema and class; values keyed by
//! persistent name; the one shared step that applies classes from an operation's history (8.4's
//! eight events); setting a value as a logged operation; values of an unknown owner carried
//! through edits.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "tests may use native arithmetic and casts"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use geom_name::{Step, apply, set};
use geom_op::{History, NameId, NameSet, Origin, Record};
use geom_test::history::{chain, check_laws};
use geom_topo::attributes::{AttrId, Attributes, Carried, MergeAction, SplitAction, Value};
use geom_topo::definition::{
    Behaviour, ChangeAction, CopyAction, Definition, Definitions, EntityKind, OnTransform,
    TransformAction, ValueSchema,
};
use geom_topo::order::NameKey;
use proptest::prelude::*;

fn n(i: u8) -> NameId {
    NameId::from_bytes([i; 16])
}

fn unknown() -> (AttrId, Definitions, Behaviour) {
    let a = AttrId {
        owner: "com.example.fire".to_owned(),
        name: "rating".to_owned(),
        version: 3,
    };
    let b = Behaviour {
        changed: ChangeAction::Keep,
        transformed: OnTransform::all(TransformAction::Keep),
        split: SplitAction::Propagate,
        merged: MergeAction::KeepIfEqual,
        copied: CopyAction::Copy,
    };
    let schema = ValueSchema::Opaque {
        id: "com.example.fire/rating-v3".to_owned(),
    };
    let d = Definition::new(a.clone(), [EntityKind::Face], schema, b).unwrap();
    (a, Definitions::new().define(d).unwrap(), b)
}

fn carried(at: &Attributes) -> BTreeMap<NameKey, Carried> {
    at.iter().map(|(_, n, c)| (*n, c.clone())).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Values of an owner this program does not know are carried through a chain of edits: after
    /// every step, an output that is the same face (unchanged or changed), a piece of it, or a
    /// merge of faces that all carried the same value, carries that value, byte for byte.
    #[test]
    fn values_of_an_unknown_owner_are_carried_through_edits(
        steps in chain((0..10u8).map(n).collect(), 4),
        value in prop::collection::vec(any::<u8>(), 1..12),
    ) {
        let (a, defs, b) = unknown();
        let mut at = Attributes::new().define(a.clone(), b.class()).unwrap();
        for i in 0..10u8 {
            at = at.set(&a, NameKey(n(i).to_bytes()), Value(value.clone())).unwrap();
        }
        for (history, inputs) in &steps {
            check_laws(history, inputs).unwrap();
            let before = carried(&at);
            let (after, _) = apply(&defs, &at, history, &Step::default()).unwrap();
            let now = carried(&after);
            for record in history.records() {
                let had = |x: &NameId| before.contains_key(&NameKey(x.to_bytes()));
                let should = match &record.origin {
                    Origin::Unchanged => had(&record.output),
                    Origin::Modified { from } if from.names().len() == 1 => had(&from.names()[0]),
                    Origin::Split { of, .. } => had(of),
                    Origin::Modified { from } => from.names().iter().all(had),
                    Origin::Merged { of } => of.names().iter().all(had),
                    Origin::Generated { .. } => false,
                };
                let key = NameKey(record.output.to_bytes());
                if should {
                    prop_assert_eq!(now.get(&key), Some(&Carried::One(Value(value.clone()))));
                } else {
                    prop_assert!(!now.contains_key(&key));
                }
            }
            at = after;
        }
    }
}

#[test]
fn the_step_reads_the_history_never_the_order_it_was_reported_in() {
    let (a, defs, b) = unknown();
    let mut at = Attributes::new().define(a.clone(), b.class()).unwrap();
    for i in 0..6u8 {
        at = at
            .set(&a, NameKey(n(i).to_bytes()), Value(vec![i]))
            .unwrap();
    }
    let records = vec![
        Record {
            output: n(0),
            origin: Origin::Unchanged,
        },
        Record {
            output: n(1),
            origin: Origin::Modified {
                from: NameSet::new([n(1)]).unwrap(),
            },
        },
        Record {
            output: n(9),
            origin: Origin::Merged {
                of: NameSet::new([n(2), n(3)]).unwrap(),
            },
        },
    ];
    let forward = History::new(records.clone(), NameSet::new([n(4)]).unwrap(), []).unwrap();
    let backward =
        History::new(records.into_iter().rev(), NameSet::new([n(4)]).unwrap(), []).unwrap();
    assert_eq!(
        apply(&defs, &at, &forward, &Step::default()),
        apply(&defs, &at, &backward, &Step::default())
    );
    // And the same inputs always give the same output.
    assert_eq!(
        apply(&defs, &at, &forward, &Step::default()),
        apply(&defs, &at, &forward, &Step::default())
    );
}

#[test]
fn setting_a_value_is_logged_with_a_history_that_keeps_the_laws() {
    let (a, defs, _) = unknown();
    let body = NameSet::new((0..5u8).map(n)).unwrap();
    let (after, history) = set(
        &defs,
        &Attributes::new(),
        &body,
        n(3),
        EntityKind::Face,
        &a,
        Some(Value(vec![1, 2, 3])),
    )
    .unwrap();
    check_laws(&history, &body).unwrap();
    // Applying its own history to its output changes nothing: every entity is unchanged.
    let (again, notes) = apply(&defs, &after, &history, &Step::default()).unwrap();
    assert_eq!(again, after);
    assert_eq!(notes, []);
}
