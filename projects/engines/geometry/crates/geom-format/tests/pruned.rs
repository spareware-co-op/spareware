//! Tests of the pruned history record (piece G109), through the public interface.
//!
//! What this file proves:
//!
//! - The steps a `PruneHistory` operation cut, written as a pruned history record through the
//!   record layer and read back, are the same; a second spelling, a step that is not a history
//!   record's payload, and bytes left over are refused; the payload matches its CDDL; each error
//!   reads as a sentence.
//! - Every held reference resolves identically after prune, save and reload: exactly as it did
//!   before the prune, through a split to its pieces and to a deletion; a reference the prune did
//!   not hold answers `HistoryPruned` before and after the save.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "test cases; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_format::history::HistoryRecordError;
use geom_format::names::{names_record, read_names};
use geom_format::payload::PayloadError;
use geom_format::pruned::{PrunedError, encode_pruned, pruned_record, read_pruned};
use geom_format::record::Record;
use geom_name::resolve::Step;
use geom_name::{
    Body, ElementId, EntityKind, Lookup, NameGraph, Reference, Resolution, Signature, Want, prune,
    resolve,
};
use geom_op::{
    History, NameId, NameList, NameSet, OpId, OpPath, Origin, PieceKey, Record as Entry, Role,
    Side, TopoName,
};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn through_file(record: &Record) -> Record {
    let bytes = record.encode(&mut meter()).unwrap();
    Record::decode(&bytes, &mut meter()).unwrap().0
}

fn op(step: &str) -> OpPath {
    OpPath::new(OpId::from_bytes([0x09; 16]))
        .step(step)
        .unwrap()
}

fn set(names: &[NameId]) -> NameSet {
    NameSet::new(names.iter().copied()).unwrap()
}

/// A wall's faces and two operations on them: a cut splits the side face in two and deletes the
/// cap, a fillet modifies the first piece; the top face is untouched throughout.
struct Wall {
    graph: NameGraph,
    side: NameId,
    cap: NameId,
    top: NameId,
    pieces: [NameId; 2],
    filleted: NameId,
    history: Vec<Step>,
}

fn wall() -> Wall {
    let mut graph = NameGraph::new();
    let mut face = |op: &OpPath, role: &str| {
        graph
            .intern(TopoName::Created {
                op: op.clone(),
                role: Role::new(role, []).unwrap(),
                from: NameList::new([]),
            })
            .unwrap()
    };
    let side = face(&op("extrude"), "side");
    let cap = face(&op("extrude"), "cap.end");
    let top = face(&op("extrude"), "top");
    let tool = face(&op("tool"), "face");
    let pieces = [face(&op("cut"), "piece.0"), face(&op("cut"), "piece.1")];
    let filleted = face(&op("fillet"), "face");
    let cut = History::new(
        [
            Entry {
                output: pieces[0],
                origin: Origin::Split {
                    of: side,
                    piece: PieceKey::ByTools {
                        tools: set(&[tool]),
                        side: Side::Back,
                    },
                },
            },
            Entry {
                output: pieces[1],
                origin: Origin::Split {
                    of: side,
                    piece: PieceKey::ByTools {
                        tools: set(&[tool]),
                        side: Side::Front,
                    },
                },
            },
            Entry {
                output: top,
                origin: Origin::Unchanged,
            },
        ],
        set(&[cap]),
        [],
    )
    .unwrap();
    let fillet = History::new(
        [
            Entry {
                output: filleted,
                origin: Origin::Modified {
                    from: set(&[pieces[0]]),
                },
            },
            Entry {
                output: pieces[1],
                origin: Origin::Unchanged,
            },
            Entry {
                output: top,
                origin: Origin::Unchanged,
            },
        ],
        NameSet::default(),
        [],
    )
    .unwrap();
    Wall {
        graph,
        side,
        cap,
        top,
        pieces,
        filleted,
        history: vec![
            Step::Recorded {
                by: op("cut"),
                history: cut,
            },
            Step::Recorded {
                by: op("fillet"),
                history: fillet,
            },
        ],
    }
}

/// The kept steps of `history` as a pruned history record holds them.
fn kept_steps(history: &[Step]) -> (OpPath, NameSet, Vec<(OpPath, History)>) {
    let mut steps = Vec::new();
    let mut prune = None;
    for step in history {
        let Step::Kept {
            by,
            history,
            pruned_by,
            held,
        } = step
        else {
            panic!("{step:?} is not a kept step");
        };
        prune = Some((pruned_by.clone(), held.clone()));
        steps.push((by.clone(), history.clone()));
    }
    let (pruned_by, held) = prune.unwrap();
    (pruned_by, held, steps)
}

#[test]
fn a_pruned_history_round_trips() {
    let w = wall();
    let pruned = prune(&w.history, &set(&[w.side, w.cap]), &op("prune"));
    let (pruned_by, held, steps) = kept_steps(&pruned);
    let record = through_file(&pruned_record(&pruned_by, &held, &steps).unwrap());
    assert_eq!((record.tag, record.flags, record.schema), (21, 1, 1));
    assert_eq!(
        read_pruned(&record.payload, &mut meter()).unwrap(),
        (pruned_by.clone(), held.clone(), steps)
    );
    let empty = encode_pruned(&pruned_by, &NameSet::default(), &[]).unwrap();
    assert_eq!(
        read_pruned(&empty, &mut meter()).unwrap(),
        (pruned_by, NameSet::default(), vec![])
    );
}

#[test]
fn a_broken_pruned_history_is_refused() {
    let w = wall();
    let pruned = prune(&w.history, &set(&[w.side]), &op("prune"));
    let (pruned_by, held, steps) = kept_steps(&pruned);
    let payload = encode_pruned(&pruned_by, &held, &steps).unwrap();
    let mut longer = payload.clone();
    longer.push(0);
    assert!(matches!(
        read_pruned(&longer, &mut meter()).unwrap_err(),
        PrunedError::Payload(PayloadError::Shape { .. })
    ));
    // Not three items: the outer array's head 0x83 made 0x82.
    let mut two = payload.clone();
    assert_eq!(two[0], 0x83);
    two[0] = 0x82;
    assert!(matches!(
        read_pruned(&two, &mut meter()).unwrap_err(),
        PrunedError::Payload(PayloadError::Shape { offset: 0, .. })
    ));
    // A step that is not a history: its four-item head made a three-item one.
    let first_step = payload.windows(2).rposition(|w| w == [0x82, 0x84]).unwrap() + 1;
    let mut bad = payload.clone();
    bad[first_step] = 0x83;
    assert!(matches!(
        read_pruned(&bad, &mut meter()).unwrap_err(),
        PrunedError::Step(HistoryRecordError::Names(_))
    ));
    // The step count spelled with a long head (0x98 0x02 for 0x82): a second spelling.
    let mut long = payload[..first_step - 1].to_vec();
    long.extend([0x98, 0x02]);
    long.extend(&payload[first_step..]);
    assert!(matches!(
        read_pruned(&long, &mut meter()).unwrap_err(),
        PrunedError::Payload(PayloadError::Cbor(_))
    ));
}

#[test]
fn the_pruned_errors_read_as_sentences() {
    assert_eq!(
        PrunedError::NotCanonical.to_string(),
        "the pruned history is not in its canonical encoding"
    );
    assert_eq!(
        PrunedError::Payload(PayloadError::Shape {
            offset: 3,
            expected: "an item"
        })
        .to_string(),
        "not a pruned history record: an item expected at byte 3"
    );
    assert_eq!(
        PrunedError::Step(HistoryRecordError::NotCanonical).to_string(),
        "not a pruned step: the history is not in its canonical encoding"
    );
}

#[test]
fn the_pruned_payload_matches_its_cddl() {
    let w = wall();
    let pruned = prune(&w.history, &set(&[w.side, w.top]), &op("prune"));
    let (pruned_by, held, steps) = kept_steps(&pruned);
    let schema = [
        include_str!("../../../format/cddl/pruned.cddl"),
        include_str!("../../../format/cddl/history.cddl"),
        include_str!("../../../format/cddl/names.cddl"),
    ]
    .join("\n");
    let payload = encode_pruned(&pruned_by, &held, &steps).unwrap();
    cddl_cat::validate_cbor_bytes("pruned", &schema, &payload).unwrap_or_else(|e| panic!("{e:?}"));
}

fn reference(name: NameId, want: Want) -> Reference {
    Reference {
        doc: None,
        element: ElementId([1; 16]),
        name,
        kind: EntityKind::Face,
        want,
        signature: Signature(vec![]),
    }
}

/// The names of `graph` and the kept steps `pruned`, saved through the record layer and read back.
fn save_and_reload(graph: &NameGraph, pruned: &[Step]) -> (NameGraph, Vec<Step>) {
    let names = through_file(&names_record(graph).unwrap());
    let (pruned_by, held, steps) = kept_steps(pruned);
    let stored = through_file(&pruned_record(&pruned_by, &held, &steps).unwrap());
    let graph = read_names(&names.payload, 1, &mut meter()).unwrap();
    let (pruned_by, held, steps) = read_pruned(&stored.payload, &mut meter()).unwrap();
    let steps = steps
        .into_iter()
        .map(|(by, history)| Step::Kept {
            by,
            history,
            pruned_by: pruned_by.clone(),
            held: held.clone(),
        })
        .collect();
    (graph, steps)
}

#[test]
fn every_held_reference_resolves_identically_after_prune_save_and_reload() {
    let w = wall();
    let live = set(&[w.filleted, w.pieces[1], w.top]);
    let resolved = |graph: &NameGraph, history: &[Step], name, want| {
        let body = Body {
            names: &live,
            carriers: &[],
            history,
            graph,
        };
        resolve(
            &reference(name, want),
            &Lookup::Found {
                body,
                version: None,
            },
        )
    };
    let held = set(&[w.side, w.cap, w.pieces[0]]);
    let pruned = prune(&w.history, &held, &op("prune"));
    let (graph_back, reloaded) = save_and_reload(&w.graph, &pruned);
    assert_eq!(reloaded, pruned);
    // Every held reference, from every step it could have been made at: the same before the
    // prune, after it, and after the save.
    for from in 0..=w.history.len() {
        for &name in held.names() {
            for want in [Want::One, Want::AllPieces] {
                let before = resolved(&w.graph, &w.history[from..], name, want);
                assert_eq!(resolved(&w.graph, &pruned[from..], name, want), before);
                assert_eq!(resolved(&graph_back, &reloaded[from..], name, want), before);
            }
        }
    }
    // Through the split to its pieces, and to the deletion.
    assert!(matches!(
        resolved(&graph_back, &reloaded, w.side, Want::AllPieces),
        Resolution::FollowedHistory { .. }
    ));
    assert_eq!(
        resolved(&graph_back, &reloaded, w.cap, Want::One),
        Resolution::Deleted { by: op("cut") }
    );
    // A prune that does not hold the cap: the cap's reference, deleted before, is pruned after,
    // and stays pruned through the save; the faces in the body stay exact.
    let pruned = prune(&w.history, &set(&[w.side]), &op("prune"));
    let (graph_back, reloaded) = save_and_reload(&w.graph, &pruned);
    let pruned_answer = Resolution::HistoryPruned {
        by: Some(op("prune")),
    };
    assert_eq!(
        resolved(&w.graph, &w.history, w.cap, Want::One),
        Resolution::Deleted { by: op("cut") }
    );
    assert_eq!(resolved(&w.graph, &pruned, w.cap, Want::One), pruned_answer);
    assert_eq!(
        resolved(&graph_back, &reloaded, w.cap, Want::One),
        pruned_answer
    );
    assert_eq!(
        resolved(&graph_back, &reloaded, w.top, Want::One),
        resolved(&w.graph, &w.history, w.top, Want::One)
    );
}
