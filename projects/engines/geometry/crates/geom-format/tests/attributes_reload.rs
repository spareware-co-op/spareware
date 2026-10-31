//! Tests of the attributes record and of references across a save (piece G95).
//!
//! What this file proves:
//!
//! - Definitions of every value schema and behaviour, and values both single and combined, written
//!   as an attributes record and read back are the same, byte for byte; a reader that does not
//!   know an attribute's owner carries its values through a split by its class alone.
//! - A value without its definition, a definition refused by its rules, a second spelling and the
//!   values of one attribute split into two groups are refused; a definition that copies its
//!   values round-trips; the payload matches its CDDL; each error reads as a sentence.
//! - Every stored reference resolves identically after its names and history are saved and
//!   reloaded: exactly, through a split to its pieces, ambiguously, and to a deletion.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "test cases; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use geom_format::attributes::{
    AttributesError, attributes_record, encode_attributes, read_attributes,
};
use geom_format::history::{history_record, read_history};
use geom_format::names::{names_record, read_names};
use geom_format::payload::PayloadError;
use geom_format::record::Record;
use geom_name::resolve::Step;
use geom_name::{
    Body, DocRef, ElementId, EntityKind as RefKind, Lookup, NameGraph, Reference, Signature, Want,
    resolve,
};
use geom_op::{
    History, NameId, NameList, NameSet, OpId, OpPath, Origin, PieceKey, Record as Entry, Role,
    Side, TopoName,
};
use geom_run::{Budget, WorkMeter};
use geom_topo::attributes::{AttrId, Attributes, Carried, MergeAction, SplitAction, Value};
use geom_topo::definition::{
    Behaviour, ChangeAction, CopyAction, Definition, Definitions, EntityKind, OnTransform,
    TransformAction, ValueSchema,
};
use geom_topo::order::NameKey;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn through_file(record: &Record) -> Record {
    let bytes = record.encode(&mut meter()).unwrap();
    Record::decode(&bytes, &mut meter()).unwrap().0
}

fn attr(owner: &str, name: &str) -> AttrId {
    AttrId {
        owner: owner.to_owned(),
        name: name.to_owned(),
        version: 1,
    }
}

fn behaviour(split: SplitAction, merged: MergeAction, transform: TransformAction) -> Behaviour {
    Behaviour {
        changed: ChangeAction::Keep,
        transformed: OnTransform {
            moved: transform,
            rotated: TransformAction::Keep,
            mirrored: TransformAction::Delete,
            scaled: TransformAction::Keep,
        },
        split,
        merged,
        copied: CopyAction::Delete,
    }
}

/// Definitions of every value schema, and values single and combined.
fn model() -> (Definitions, Attributes) {
    let fire = attr("org.example.fire", "rating");
    let mark = attr("kit.construct", "mark");
    let defs = [
        Definition::new(
            fire.clone(),
            [EntityKind::Face, EntityKind::Body],
            ValueSchema::Opaque {
                id: "fire-rating-v1".to_owned(),
            },
            behaviour(
                SplitAction::Propagate,
                MergeAction::Combine,
                TransformAction::Keep,
            ),
        )
        .unwrap(),
        Definition::new(
            mark.clone(),
            [EntityKind::Vertex],
            ValueSchema::Point,
            behaviour(
                SplitAction::Delete,
                MergeAction::KeepIfEqual,
                TransformAction::Transform,
            ),
        )
        .unwrap(),
    ];
    let mut definitions = Definitions::default();
    let mut values = Attributes::new();
    for d in defs {
        values = values.defined_as(d.attr().clone(), d.behaviour().class());
        definitions = definitions.define(d).unwrap();
    }
    for (schema, (n, a)) in [
        ValueSchema::Direction,
        ValueSchema::Frame,
        ValueSchema::Length,
        ValueSchema::Field,
    ]
    .into_iter()
    .zip([(3_u8, "dir"), (4, "frame"), (5, "len"), (6, "field")])
    {
        let d = Definition::new(
            attr("kit.construct", a),
            [EntityKind::Edge, EntityKind::Region],
            schema,
            behaviour(
                SplitAction::Propagate,
                MergeAction::Delete,
                TransformAction::Delete,
            ),
        )
        .unwrap();
        values = values.defined_as(d.attr().clone(), d.behaviour().class());
        definitions = definitions.define(d).unwrap();
        let _ = n;
    }
    let combined: BTreeMap<NameKey, Value> = [
        (NameKey([1; 16]), Value(vec![60])),
        (NameKey([2; 16]), Value(vec![90])),
    ]
    .into_iter()
    .collect();
    let values = values.rebuilt([
        (
            fire.clone(),
            NameKey([7; 16]),
            Carried::One(Value(vec![1, 2, 3])),
        ),
        (fire, NameKey([8; 16]), Carried::Combined(combined)),
        (mark, NameKey([9; 16]), Carried::One(Value(vec![]))),
    ]);
    (definitions, values)
}

#[test]
fn definitions_and_values_round_trip() {
    let (definitions, values) = model();
    let record = through_file(&attributes_record(&definitions, &values).unwrap());
    assert_eq!((record.tag, record.flags, record.schema), (19, 1, 1));
    let (d, v) = read_attributes(&record.payload, &mut meter()).unwrap();
    assert_eq!(d, definitions);
    assert_eq!(v, values);
    let (empty_d, empty_v) = (Definitions::default(), Attributes::new());
    let empty = attributes_record(&empty_d, &empty_v).unwrap();
    assert_eq!(
        read_attributes(&empty.payload, &mut meter()).unwrap(),
        (empty_d, empty_v)
    );
}

#[test]
fn a_reader_that_does_not_know_the_owner_carries_values_by_their_class() {
    let (definitions, values) = model();
    let payload = attributes_record(&definitions, &values).unwrap().payload;
    let (_, read) = read_attributes(&payload, &mut meter()).unwrap();
    // Nothing here knows `org.example.fire`; its class propagates the value to both pieces.
    let pieces = [NameKey([20; 16]), NameKey([21; 16])];
    let split = read.split(NameKey([7; 16]), &pieces);
    let fire = attr("org.example.fire", "rating");
    for piece in pieces {
        assert_eq!(
            split.get(&fire, piece),
            Some(&Carried::One(Value(vec![1, 2, 3])))
        );
    }
}

#[test]
fn a_bad_attributes_record_is_refused() {
    let (definitions, values) = model();
    // A value whose attribute is not defined.
    let stray = values.defined_as(
        attr("x", "stray"),
        geom_topo::attributes::Class {
            split: SplitAction::Delete,
            merge: MergeAction::Delete,
        },
    );
    let stray = stray.rebuilt(
        values
            .iter()
            .map(|(a, n, c)| (a.clone(), *n, c.clone()))
            .chain([(
                attr("x", "stray"),
                NameKey([5; 16]),
                Carried::One(Value(vec![])),
            )]),
    );
    let payload = attributes_record(&definitions, &stray).unwrap().payload;
    assert_eq!(
        read_attributes(&payload, &mut meter()).unwrap_err(),
        AttributesError::Undefined(attr("x", "stray"))
    );
    let payload = attributes_record(&definitions, &values).unwrap().payload;
    let mut longer = payload.clone();
    longer.push(0);
    assert!(read_attributes(&longer, &mut meter()).is_err());
    // An opaque value schema that would move its values: the first definition's moved action
    // (keep 0) becomes transform 1.
    let fire = payload
        .windows(4)
        .position(|w| w == [0x85, 0x00, 0x84, 0x00])
        .unwrap();
    let mut moving = payload;
    moving[fire + 3] = 0x01;
    assert!(matches!(
        read_attributes(&moving, &mut meter()).unwrap_err(),
        AttributesError::Definition(_)
    ));
}

#[test]
fn the_attributes_payload_matches_its_cddl() {
    let (definitions, values) = model();
    let schema = [
        include_str!("../../../format/cddl/attributes.cddl"),
        include_str!("../../../format/cddl/names.cddl"),
    ]
    .join("\n");
    let payload = attributes_record(&definitions, &values).unwrap().payload;
    cddl_cat::validate_cbor_bytes("attributes", &schema, &payload)
        .unwrap_or_else(|e| panic!("{e:?}"));
}

// References across a save.

fn op(step: &str) -> OpPath {
    OpPath::new(OpId::from_bytes([0x21; 16]))
        .step(step)
        .unwrap()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one scenario, built, saved, reloaded and compared in order"
)]
fn every_stored_reference_resolves_identically_after_save_and_reload() {
    // A face made by an extrusion, and a cap it deletes later.
    let mut graph = NameGraph::new();
    let role = |r: &str| Role::new(r, []).unwrap();
    let face = graph
        .intern(TopoName::Created {
            op: op("extrude"),
            role: role("side"),
            from: NameList::new([]),
        })
        .unwrap();
    let cap = graph
        .intern(TopoName::Created {
            op: op("extrude"),
            role: role("cap.end"),
            from: NameList::new([]),
        })
        .unwrap();
    // The cutting tool's face is a name of the graph too: resolution reads every parent.
    let tool = graph
        .intern(TopoName::Created {
            op: op("tool"),
            role: role("face"),
            from: NameList::new([]),
        })
        .unwrap();
    let tools = NameSet::new([tool]).unwrap();
    let pieces: Vec<NameId> = [Side::Back, Side::Front]
        .into_iter()
        .map(|side| {
            graph
                .intern(TopoName::Split {
                    op: op("cut"),
                    of: face,
                    piece: PieceKey::ByTools {
                        tools: tools.clone(),
                        side,
                    },
                })
                .unwrap()
        })
        .collect();
    let history = History::new(
        pieces.iter().map(|&p| Entry {
            output: p,
            origin: Origin::Split {
                of: face,
                piece: PieceKey::ByTools {
                    tools: tools.clone(),
                    side: if p == pieces[0] {
                        Side::Back
                    } else {
                        Side::Front
                    },
                },
            },
        }),
        NameSet::new([cap]).unwrap(),
        [],
    )
    .unwrap();
    let live = NameSet::new(pieces.iter().copied()).unwrap();
    let references: Vec<Reference> = [
        (face, Want::AllPieces),
        (face, Want::One),
        (cap, Want::One),
        (pieces[0], Want::One),
    ]
    .into_iter()
    .map(|(name, want)| Reference {
        doc: None::<DocRef>,
        element: ElementId([1; 16]),
        name,
        kind: RefKind::Face,
        want,
        signature: Signature(vec![]),
    })
    .collect();
    let before: Vec<_> = {
        let steps = [Step::Recorded {
            by: op("cut"),
            history: history.clone(),
        }];
        let body = Body {
            names: &live,
            carriers: &[],
            history: &steps,
            graph: &graph,
        };
        references
            .iter()
            .map(|r| {
                resolve(
                    r,
                    &Lookup::Found {
                        body,
                        version: None,
                    },
                )
            })
            .collect()
    };
    // Save the names and the history, and read them back.
    let names = through_file(&names_record(&graph).unwrap());
    let stored = through_file(&history_record(&op("cut"), &history).unwrap());
    let graph_back = read_names(&names.payload, 1, &mut meter()).unwrap();
    let (by, history_back) = read_history(&stored.payload, &mut meter()).unwrap();
    let steps = [Step::Recorded {
        by,
        history: history_back,
    }];
    let body = Body {
        names: &live,
        carriers: &[],
        history: &steps,
        graph: &graph_back,
    };
    let after: Vec<_> = references
        .iter()
        .map(|r| {
            resolve(
                r,
                &Lookup::Found {
                    body,
                    version: None,
                },
            )
        })
        .collect();
    assert_eq!(after, before);
    // The four outcomes really differ: pieces, ambiguous, deleted, exact.
    let kinds: std::collections::BTreeSet<String> = before
        .iter()
        .map(|r| {
            format!("{r:?}")
                .split([' ', '{', '('])
                .next()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(kinds.len(), 4, "{before:?}");
}

#[test]
fn a_definition_that_copies_round_trips() {
    let mut b = behaviour(
        SplitAction::Propagate,
        MergeAction::Combine,
        TransformAction::Keep,
    );
    b.copied = CopyAction::Copy;
    let d = Definition::new(
        attr("kit.construct", "copied"),
        [EntityKind::Face],
        ValueSchema::Length,
        b,
    )
    .unwrap();
    let definitions = Definitions::default().define(d).unwrap();
    let payload = encode_attributes(&definitions, &Attributes::new()).unwrap();
    assert_eq!(
        read_attributes(&payload, &mut meter()).unwrap().0,
        definitions
    );
}

#[test]
fn the_values_of_one_attribute_are_one_group() {
    let (definitions, _) = model();
    let fire = attr("org.example.fire", "rating");
    let defined = |keys: &[u8]| {
        let mut v = Attributes::new();
        for d in definitions.iter() {
            v = v.defined_as(d.attr().clone(), d.behaviour().class());
        }
        v.rebuilt(
            keys.iter()
                .map(|&k| (fire.clone(), NameKey([k; 16]), Carried::One(Value(vec![k])))),
        )
    };
    let encode = |keys: &[u8]| encode_attributes(&definitions, &defined(keys)).unwrap();
    // [definitions, values]: with no values the payload ends in an empty array; with one value
    // that array holds one group, the tail after the definitions.
    let none = encode(&[]);
    let group = |k: u8| encode(&[k])[none.len()..].to_vec();
    let both = encode(&[7, 8]);
    assert_eq!(
        read_attributes(&both, &mut meter()).unwrap().1,
        defined(&[7, 8])
    );
    // The same two values written as two groups of one attribute are not the canonical encoding.
    let mut split = none[..none.len() - 1].to_vec();
    split.push(0x82);
    split.extend(group(7));
    split.extend(group(8));
    assert_ne!(split, both);
    assert_eq!(
        read_attributes(&split, &mut meter()).unwrap_err(),
        AttributesError::NotCanonical
    );
}

#[test]
fn the_attributes_errors_read_as_sentences() {
    let shape = PayloadError::Shape {
        offset: 3,
        expected: "an item",
    };
    assert_eq!(
        AttributesError::Payload(shape).to_string(),
        "not an attributes record: an item expected at byte 3"
    );
    assert_eq!(
        AttributesError::NotCanonical.to_string(),
        "the attributes are not in their canonical encoding"
    );
}
