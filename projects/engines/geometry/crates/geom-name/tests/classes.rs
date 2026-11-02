//! The shared attribute step through the public interface (piece G90): each of section 8.4's
//! events by hand, setting a value as an operation, every refusal, and a property test over
//! generated histories that the step's output values are exactly what the behaviours and the
//! history give, by a model written here from 8.4's table.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    reason = "tests may use native arithmetic and casts"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet};

use geom_name::{AttrError, Mover, Note, Step, apply, set};
use geom_op::{History, NameId, NameSet, Origin, PieceKey, Record, Role, Side};
use geom_test::history::step as history_step;
use geom_topo::attributes::{AttrId, Attributes, Carried, MergeAction, SplitAction, Value};
use geom_topo::definition::{
    Behaviour, ChangeAction, CopyAction, Definition, Definitions, EntityKind, OnTransform,
    TransformAction, TransformKind, ValueSchema,
};
use geom_topo::order::NameKey;
use proptest::prelude::*;

fn n(i: u8) -> NameId {
    NameId::from_bytes([i; 16])
}

fn k(i: u8) -> NameKey {
    NameKey([i; 16])
}

fn attr(owner: &str, name: &str) -> AttrId {
    AttrId {
        owner: owner.to_owned(),
        name: name.to_owned(),
        version: 1,
    }
}

fn v(b: u8) -> Value {
    Value(vec![b, b ^ 0x5a])
}

fn names(ns: &[u8]) -> NameSet {
    NameSet::new(ns.iter().map(|&i| n(i))).unwrap()
}

/// A behaviour that keeps everything it can.
fn keeping() -> Behaviour {
    Behaviour {
        changed: ChangeAction::Keep,
        transformed: OnTransform::all(TransformAction::Keep),
        split: SplitAction::Propagate,
        merged: MergeAction::KeepIfEqual,
        copied: CopyAction::Copy,
    }
}

/// A behaviour that deletes everything.
fn deleting() -> Behaviour {
    Behaviour {
        changed: ChangeAction::Delete,
        transformed: OnTransform::all(TransformAction::Delete),
        split: SplitAction::Delete,
        merged: MergeAction::Delete,
        copied: CopyAction::Delete,
    }
}

fn define(defs: &Definitions, a: &AttrId, schema: ValueSchema, b: Behaviour) -> Definitions {
    defs.define(
        Definition::new(a.clone(), [EntityKind::Face, EntityKind::Edge], schema, b).unwrap(),
    )
    .unwrap()
}

/// Attributes holding `values` for `a` under `b`'s class.
fn holding(a: &AttrId, b: Behaviour, values: &[(u8, u8)]) -> Attributes {
    let mut at = Attributes::new().define(a.clone(), b.class()).unwrap();
    for &(name, value) in values {
        at = at.set(a, k(name), v(value)).unwrap();
    }
    at
}

fn history(records: Vec<(u8, Origin)>, deleted: &[u8]) -> History {
    History::new(
        records.into_iter().map(|(o, origin)| Record {
            output: n(o),
            origin,
        }),
        names(deleted),
        [],
    )
    .unwrap()
}

fn opaque() -> ValueSchema {
    ValueSchema::Opaque {
        id: "acme.fire/rating".to_owned(),
    }
}

/// Moves a value by adding one to every byte; refuses an empty value.
struct AddOne;

impl Mover for AddOne {
    fn moved(&self, _: &ValueSchema, value: &Value) -> Option<Value> {
        if value.0.is_empty() {
            None
        } else {
            Some(Value(value.0.iter().map(|b| b.wrapping_add(1)).collect()))
        }
    }
}

fn values(at: &Attributes, a: &AttrId) -> BTreeMap<NameKey, Carried> {
    at.iter()
        .filter(|(x, _, _)| *x == a)
        .map(|(_, n, c)| (*n, c.clone()))
        .collect()
}

#[test]
fn unchanged_changed_and_deleted_entities() {
    let a = attr("kit.geom", "rating");
    for (b, kept) in [(keeping(), true), (deleting(), false)] {
        let defs = define(&Definitions::new(), &a, opaque(), b);
        let before = holding(&a, b, &[(1, 10), (2, 20), (3, 30)]);
        // 1 unchanged, 2 changed keeping its name, 3 deleted.
        let h = history(
            vec![
                (1, Origin::Unchanged),
                (2, Origin::Modified { from: names(&[2]) }),
            ],
            &[3],
        );
        let (after, notes) = apply(&defs, &before, &h, &Step::default()).unwrap();
        let got = values(&after, &a);
        assert_eq!(
            got.get(&k(1)),
            Some(&Carried::One(v(10))),
            "unchanged stays"
        );
        assert_eq!(got.contains_key(&k(2)), kept, "changed: {b:?}");
        assert_eq!(got.get(&k(3)), None, "deleted goes");
        assert_eq!(notes, []);
        assert_eq!(after.class(&a), Some(b.class()));
    }
}

#[test]
fn transformed_values_keep_move_or_go_by_kind() {
    let a = attr("kit.geom", "datum");
    for kind in TransformKind::ALL {
        for action in [
            TransformAction::Keep,
            TransformAction::Transform,
            TransformAction::Delete,
        ] {
            let mut b = keeping();
            b.transformed = OnTransform::all(TransformAction::Delete);
            match kind {
                TransformKind::Move => b.transformed.moved = action,
                TransformKind::Rotate => b.transformed.rotated = action,
                TransformKind::Mirror => b.transformed.mirrored = action,
                TransformKind::Scale => b.transformed.scaled = action,
            }
            let defs = define(&Definitions::new(), &a, ValueSchema::Point, b);
            let before = holding(&a, b, &[(1, 10)]);
            let h = history(vec![(1, Origin::Modified { from: names(&[1]) })], &[]);
            let step = Step {
                transform: Some((kind, &AddOne)),
                copies: &[],
            };
            let (after, notes) = apply(&defs, &before, &h, &step).unwrap();
            let got = values(&after, &a).get(&k(1)).cloned();
            let want = match action {
                TransformAction::Keep => Some(Carried::One(v(10))),
                TransformAction::Transform => {
                    Some(Carried::One(Value(vec![11, (0x0a ^ 0x5a) + 1])))
                }
                TransformAction::Delete => None,
            };
            assert_eq!(got, want, "{kind} {action:?}");
            assert_eq!(notes, []);
        }
    }
    // A value the mover cannot move is dropped, with a note.
    let mut b = keeping();
    b.transformed = OnTransform::all(TransformAction::Transform);
    let defs = define(&Definitions::new(), &a, ValueSchema::Length, b);
    let before = Attributes::new()
        .define(a.clone(), b.class())
        .unwrap()
        .set(&a, k(1), Value(vec![]))
        .unwrap();
    let h = history(vec![(1, Origin::Modified { from: names(&[1]) })], &[]);
    let step = Step {
        transform: Some((TransformKind::Rotate, &AddOne)),
        copies: &[],
    };
    let (after, notes) = apply(&defs, &before, &h, &step).unwrap();
    assert_eq!(values(&after, &a), BTreeMap::new());
    assert_eq!(
        notes,
        [Note::NotMoved {
            attr: a.clone(),
            entity: n(1)
        }]
    );
}

#[test]
fn split_pieces_get_the_value_or_none() {
    let a = attr("kit.geom", "rating");
    for (split, kept) in [(SplitAction::Propagate, true), (SplitAction::Delete, false)] {
        let mut b = keeping();
        b.split = split;
        let defs = define(&Definitions::new(), &a, opaque(), b);
        let before = holding(&a, b, &[(1, 10)]);
        let piece = |side| PieceKey::ByTools {
            tools: names(&[90]),
            side,
        };
        let h = history(
            vec![
                (
                    5,
                    Origin::Split {
                        of: n(1),
                        piece: piece(Side::Front),
                    },
                ),
                (
                    6,
                    Origin::Split {
                        of: n(1),
                        piece: piece(Side::Back),
                    },
                ),
            ],
            &[],
        );
        let (after, _) = apply(&defs, &before, &h, &Step::default()).unwrap();
        let got = values(&after, &a);
        assert_eq!(got.contains_key(&k(5)), kept);
        assert_eq!(got.contains_key(&k(6)), kept);
        assert_eq!(got.get(&k(1)), None, "the split input is not an output");
    }
}

#[test]
fn merges_keep_equal_values_combine_or_delete() {
    let a = attr("kit.geom", "rating");
    let merged = |of: &[u8]| Origin::Merged { of: names(of) };
    for action in [
        MergeAction::KeepIfEqual,
        MergeAction::Combine,
        MergeAction::Delete,
    ] {
        let mut b = keeping();
        b.merged = action;
        let defs = define(&Definitions::new(), &a, opaque(), b);
        // 7 = 1 + 2, both 10; 8 = 3 + 4, 30 and 40; 9 = 5 + 6, only 5 carries one; and a
        // modified record from two inputs is a merge too: 11 = 1 + 3.
        let before = holding(&a, b, &[(1, 10), (2, 10), (3, 30), (4, 40), (5, 50)]);
        let h = history(
            vec![
                (7, merged(&[1, 2])),
                (8, merged(&[3, 4])),
                (9, merged(&[5, 6])),
                (
                    11,
                    Origin::Modified {
                        from: names(&[1, 3]),
                    },
                ),
            ],
            &[],
        );
        let (after, notes) = apply(&defs, &before, &h, &Step::default()).unwrap();
        let got = values(&after, &a);
        match action {
            MergeAction::KeepIfEqual => {
                assert_eq!(got.get(&k(7)), Some(&Carried::One(v(10))));
                assert_eq!(got.get(&k(8)), None);
                assert_eq!(got.get(&k(9)), None);
                assert_eq!(got.get(&k(11)), None);
                let dropped: Vec<NameId> = notes
                    .iter()
                    .map(|nt| match nt {
                        Note::Dropped { into, .. } => *into,
                        Note::NotMoved { .. } => panic!("{nt:?}"),
                    })
                    .collect();
                assert_eq!(dropped, [n(8), n(9), n(11)]);
            }
            MergeAction::Combine => {
                let each = |pairs: &[(u8, u8)]| {
                    Carried::Combined(pairs.iter().map(|&(x, y)| (k(x), v(y))).collect())
                };
                assert_eq!(got.get(&k(7)), Some(&each(&[(1, 10), (2, 10)])));
                assert_eq!(got.get(&k(8)), Some(&each(&[(3, 30), (4, 40)])));
                assert_eq!(got.get(&k(9)), Some(&each(&[(5, 50)])));
                assert_eq!(got.get(&k(11)), Some(&each(&[(1, 10), (3, 30)])));
                assert_eq!(notes, []);
            }
            MergeAction::Delete => {
                assert_eq!(got, BTreeMap::new());
                assert_eq!(notes, []);
            }
        }
    }
    // Merging combined values flattens them by name.
    let mut b = keeping();
    b.merged = MergeAction::Combine;
    let defs = define(&Definitions::new(), &a, opaque(), b);
    let before = holding(&a, b, &[(1, 10), (2, 20), (3, 30)]);
    let h1 = history(vec![(7, merged(&[1, 2])), (3, Origin::Unchanged)], &[]);
    let (mid, _) = apply(&defs, &before, &h1, &Step::default()).unwrap();
    let h2 = history(vec![(8, merged(&[7, 3]))], &[]);
    let (after, _) = apply(&defs, &mid, &h2, &Step::default()).unwrap();
    let want: BTreeMap<NameKey, Value> = [(k(1), v(10)), (k(2), v(20)), (k(3), v(30))].into();
    assert_eq!(
        values(&after, &a).get(&k(8)),
        Some(&Carried::Combined(want))
    );
}

#[test]
fn copies_carry_the_value_or_not_and_other_new_entities_start_without() {
    let a = attr("kit.geom", "rating");
    let instance = Role::new("instance", []).unwrap();
    let side = Role::new("side", []).unwrap();
    for (copied, kept) in [(CopyAction::Copy, true), (CopyAction::Delete, false)] {
        let mut b = keeping();
        b.copied = copied;
        let defs = define(&Definitions::new(), &a, opaque(), b);
        let before = holding(&a, b, &[(1, 10), (2, 20)]);
        let made = |from: &[u8], role: &Role| Origin::Generated {
            from: names(from),
            role: role.clone(),
        };
        let h = history(
            vec![
                (1, Origin::Unchanged),
                (5, made(&[1], &instance)),
                (6, made(&[1], &side)),
                (7, made(&[1, 2], &instance)),
                (8, made(&[], &instance)),
            ],
            &[],
        );
        let copies = [instance.clone()];
        let step = Step {
            transform: None,
            copies: &copies,
        };
        let (after, _) = apply(&defs, &before, &h, &step).unwrap();
        let got = values(&after, &a);
        assert_eq!(got.contains_key(&k(5)), kept, "a copy of one input");
        assert_eq!(got.get(&k(6)), None, "generated in another role");
        assert_eq!(
            got.get(&k(7)),
            None,
            "generated from two inputs is not a copy"
        );
        assert_eq!(got.get(&k(8)), None, "generated from nothing");
        // Without the copy role declared, the same history copies nothing.
        let (plain, _) = apply(&defs, &before, &h, &Step::default()).unwrap();
        assert_eq!(values(&plain, &a).get(&k(5)), None);
    }
}

#[test]
fn every_value_needs_its_definition_and_its_class() {
    let a = attr("acme", "rating");
    let b = keeping();
    let before = holding(&a, b, &[(1, 10)]);
    let h = history(vec![(1, Origin::Unchanged)], &[]);
    assert_eq!(
        apply(&Definitions::new(), &before, &h, &Step::default()),
        Err(AttrError::Undefined(a.clone()))
    );
    let defs = define(&Definitions::new(), &a, opaque(), deleting());
    assert_eq!(
        apply(&defs, &before, &h, &Step::default()),
        Err(AttrError::ClassMismatch(a.clone()))
    );
    // Attributes with no values need no definitions.
    let (after, _) = apply(
        &Definitions::new(),
        &Attributes::new(),
        &h,
        &Step::default(),
    )
    .unwrap();
    assert_eq!(after, Attributes::new());
}

#[test]
fn setting_a_value_is_an_operation_that_changes_no_entity() {
    let a = attr("kit.geom", "continuity");
    let b = keeping();
    let defs = define(&Definitions::new(), &a, opaque(), b);
    let body = names(&[1, 2, 3]);
    let (after, h) = set(
        &defs,
        &Attributes::new(),
        &body,
        n(2),
        EntityKind::Edge,
        &a,
        Some(v(7)),
    )
    .unwrap();
    assert_eq!(values(&after, &a), [(k(2), Carried::One(v(7)))].into());
    assert_eq!(h.records().len(), 3);
    assert!(h.records().iter().all(|r| r.origin == Origin::Unchanged));
    assert_eq!(h.deleted().names(), []);
    h.check(&body).unwrap();
    // Changing it, then removing it.
    let (changed, _) = set(&defs, &after, &body, n(2), EntityKind::Edge, &a, Some(v(8))).unwrap();
    assert_eq!(values(&changed, &a), [(k(2), Carried::One(v(8)))].into());
    let (removed, _) = set(&defs, &changed, &body, n(2), EntityKind::Edge, &a, None).unwrap();
    assert_eq!(values(&removed, &a), BTreeMap::new());
    // Refusals.
    let other = attr("kit.geom", "missing");
    assert_eq!(
        set(&defs, &after, &body, n(2), EntityKind::Edge, &other, None),
        Err(AttrError::Undefined(other))
    );
    assert_eq!(
        set(
            &defs,
            &after,
            &body,
            n(2),
            EntityKind::Vertex,
            &a,
            Some(v(1))
        ),
        Err(AttrError::WrongKind {
            attr: a.clone(),
            kind: EntityKind::Vertex
        })
    );
    assert_eq!(
        set(&defs, &after, &body, n(9), EntityKind::Edge, &a, Some(v(1))),
        Err(AttrError::NotInBody(n(9)))
    );
    let mismatched = holding(&a, deleting(), &[]);
    assert_eq!(
        set(
            &defs,
            &mismatched,
            &body,
            n(2),
            EntityKind::Edge,
            &a,
            Some(v(1))
        ),
        Err(AttrError::ClassMismatch(a.clone()))
    );
}

#[test]
fn errors_and_steps_print_what_they_are() {
    let a = attr("acme", "rating");
    let cases = [
        (
            AttrError::Undefined(a.clone()),
            "acme/rating version 1 has no definition",
        ),
        (
            AttrError::ClassMismatch(a.clone()),
            "acme/rating version 1 is held with a class its definition does not give",
        ),
        (
            AttrError::WrongKind {
                attr: a.clone(),
                kind: EntityKind::Face,
            },
            "a Face may not carry acme/rating",
        ),
    ];
    for (e, text) in cases {
        assert_eq!(e.to_string(), text);
    }
    assert!(
        AttrError::NotInBody(n(1))
            .to_string()
            .ends_with("is not a named entity of the body")
    );
    let step = Step {
        transform: Some((TransformKind::Mirror, &AddOne)),
        copies: &[],
    };
    assert_eq!(
        format!("{step:?}"),
        "Step { transform: Some(Mirror), copies: [] }"
    );
}

/// The values 8.4's table gives one output, written from the table, for comparison with the step.
fn model(
    b: Behaviour,
    before: &BTreeMap<NameKey, Carried>,
    record: &Record,
    copies: &[Role],
) -> Option<Carried> {
    let of = |x: &NameId| before.get(&NameKey(x.to_bytes())).cloned();
    let merge = |inputs: &[NameId]| -> Option<Carried> {
        let carried: Vec<(NameKey, Carried)> = inputs
            .iter()
            .filter_map(|x| of(x).map(|c| (NameKey(x.to_bytes()), c)))
            .collect();
        if carried.is_empty() {
            return None;
        }
        match b.merged {
            MergeAction::Delete => None,
            MergeAction::KeepIfEqual => {
                let first = &carried[0].1;
                (carried.len() == inputs.len() && carried.iter().all(|(_, c)| c == first))
                    .then(|| first.clone())
            }
            MergeAction::Combine => {
                let mut each = BTreeMap::new();
                for (x, c) in carried {
                    match c {
                        Carried::One(val) => {
                            each.insert(x, val);
                        }
                        Carried::Combined(inner) => each.extend(inner),
                    }
                }
                Some(Carried::Combined(each))
            }
        }
    };
    match &record.origin {
        Origin::Unchanged => of(&record.output),
        Origin::Modified { from } if from.names().len() == 1 => match b.changed {
            ChangeAction::Keep => of(&from.names()[0]),
            ChangeAction::Delete => None,
        },
        Origin::Modified { from } => merge(from.names()),
        Origin::Merged { of: inputs } => merge(inputs.names()),
        Origin::Split { of: input, .. } => match b.split {
            SplitAction::Propagate => of(input),
            SplitAction::Delete => None,
        },
        Origin::Generated { from, role } => {
            if from.names().len() == 1 && copies.contains(role) {
                match b.copied {
                    CopyAction::Copy => of(&from.names()[0]),
                    CopyAction::Delete => None,
                }
            } else {
                None
            }
        }
    }
}

fn behaviour() -> impl Strategy<Value = Behaviour> {
    (
        prop_oneof![Just(ChangeAction::Keep), Just(ChangeAction::Delete)],
        prop_oneof![Just(SplitAction::Propagate), Just(SplitAction::Delete)],
        prop_oneof![
            Just(MergeAction::KeepIfEqual),
            Just(MergeAction::Combine),
            Just(MergeAction::Delete)
        ],
        prop_oneof![Just(CopyAction::Copy), Just(CopyAction::Delete)],
    )
        .prop_map(|(changed, split, merged, copied)| Behaviour {
            changed,
            transformed: OnTransform::all(TransformAction::Keep),
            split,
            merged,
            copied,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Over generated valid histories, every attribute's output values are exactly what its
    /// behaviour and the history give, for values of a known and of an unknown owner alike.
    #[test]
    fn the_step_gives_exactly_what_classes_and_history_give(
        (h, _ins) in history_step(0, (0..12u8).map(n).collect()),
        known in behaviour(),
        unknown in behaviour(),
        held in prop::collection::vec((0..12u8, 0..3u8, any::<bool>()), 0..24),
        copy_role in any::<bool>(),
    ) {
        let a = attr("kit.geom", "rating");
        let u = attr("acme.unknown", "data");
        let defs = define(&define(&Definitions::new(), &a, opaque(), known), &u, opaque(), unknown);
        let mut before = Attributes::new()
            .define(a.clone(), known.class())
            .unwrap()
            .define(u.clone(), unknown.class())
            .unwrap();
        for (name, value, which) in held {
            let at = if which { &a } else { &u };
            before = before.set(at, k(name), v(value)).unwrap();
        }
        // Copy every role the generator uses when asked: a pattern's schema.
        let roles: Vec<Role> = h
            .records()
            .iter()
            .filter_map(|r| match &r.origin {
                Origin::Generated { role, .. } => Some(role.clone()),
                _ => None,
            })
            .collect::<BTreeSet<Role>>()
            .into_iter()
            .collect();
        let copies: &[Role] = if copy_role { &roles } else { &[] };
        let step = Step { transform: None, copies };
        let (after, _) = apply(&defs, &before, &h, &step).unwrap();
        for (at, b) in [(&a, known), (&u, unknown)] {
            let was = values(&before, at);
            let mut want = BTreeMap::new();
            for record in h.records() {
                if let Some(c) = model(b, &was, record, copies) {
                    want.insert(NameKey(record.output.to_bytes()), c);
                }
            }
            prop_assert_eq!(values(&after, at), want);
        }
        // Deleted inputs never carry a value afterwards, unless an output reuses the name.
        let outputs: BTreeSet<NameId> = h.records().iter().map(|r| r.output).collect();
        for d in h.deleted().names() {
            if !outputs.contains(d) {
                prop_assert!(after.iter().all(|(_, x, _)| *x != NameKey(d.to_bytes())));
            }
        }
    }
}

#[test]
fn a_combined_value_moves_whole_or_not_at_all() {
    let a = attr("kit.geom", "datum");
    let mut b = keeping();
    b.merged = MergeAction::Combine;
    b.transformed = OnTransform::all(TransformAction::Transform);
    let defs = define(&Definitions::new(), &a, ValueSchema::Point, b);
    let before = holding(&a, b, &[(1, 10), (2, 20)]);
    let merged = history(vec![(7, Origin::Merged { of: names(&[1, 2]) })], &[]);
    let (combined, _) = apply(&defs, &before, &merged, &Step::default()).unwrap();
    let moved = history(vec![(7, Origin::Modified { from: names(&[7]) })], &[]);
    let step = Step {
        transform: Some((TransformKind::Move, &AddOne)),
        copies: &[],
    };
    let (after, notes) = apply(&defs, &combined, &moved, &step).unwrap();
    let want: BTreeMap<NameKey, Value> = [
        (k(1), Value(vec![11, (0x0a ^ 0x5a) + 1])),
        (k(2), Value(vec![21, (0x14 ^ 0x5a) + 1])),
    ]
    .into();
    assert_eq!(
        values(&after, &a).get(&k(7)),
        Some(&Carried::Combined(want))
    );
    assert_eq!(notes, []);
    // One part that cannot move drops the whole value.
    let mut each = BTreeMap::new();
    each.insert(k(1), v(10));
    each.insert(k(2), Value(vec![]));
    let odd = combined.rebuilt([(a.clone(), k(7), Carried::Combined(each))]);
    let (after, notes) = apply(&defs, &odd, &moved, &step).unwrap();
    assert_eq!(values(&after, &a), BTreeMap::new());
    assert_eq!(
        notes,
        [Note::NotMoved {
            attr: a.clone(),
            entity: n(7)
        }]
    );
}

#[test]
fn setting_one_entity_leaves_the_others_values() {
    let a = attr("kit.geom", "rating");
    let other = attr("kit.geom", "colour");
    let b = keeping();
    let defs = define(
        &define(&Definitions::new(), &a, opaque(), b),
        &other,
        opaque(),
        b,
    );
    let body = names(&[1, 2, 3]);
    let before = holding(&a, b, &[(1, 10)]);
    let before = before
        .define(other.clone(), b.class())
        .unwrap()
        .set(&other, k(2), v(99))
        .unwrap();
    let (after, _) = set(
        &defs,
        &before,
        &body,
        n(2),
        EntityKind::Face,
        &a,
        Some(v(20)),
    )
    .unwrap();
    assert_eq!(
        values(&after, &a),
        [(k(1), Carried::One(v(10))), (k(2), Carried::One(v(20)))].into()
    );
    assert_eq!(values(&after, &other), [(k(2), Carried::One(v(99)))].into());
}

#[test]
fn history_and_name_errors_print_what_they_are() {
    let e = AttrError::History(geom_op::HistoryError::RepeatedOutput { output: n(4) });
    assert_eq!(
        e.to_string(),
        geom_op::HistoryError::RepeatedOutput { output: n(4) }.to_string()
    );
    let names_error = NameSet::new([n(1), n(1)]).unwrap_err();
    assert_eq!(
        AttrError::Names(names_error).to_string(),
        names_error.to_string()
    );
}
