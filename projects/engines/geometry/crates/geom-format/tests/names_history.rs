//! Tests of the names and history records (piece G95), through the public interface.
//!
//! What this file proves:
//!
//! - A name graph with every kind of name written as a names record, through the record layer, and
//!   read back is the same graph, byte for byte; every `NameId` is checked against its bytes.
//! - A names record is refused under another `NameScheme`, with a node whose id is not its hash,
//!   out of order or repeated, or with a name in a second spelling, each at its byte.
//! - A history with every origin and every topology change written as a history record and read
//!   back is the same history at the same path; generated histories round-trip too; a second
//!   spelling or an unknown origin is refused.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test cases; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use geom_format::history::{HistoryRecordError, history_record, read_history};
use geom_format::names::{NamesError, names_record, read_names, read_topo_name};
use geom_format::record::Record;
use geom_name::NameGraph;
use geom_op::{
    EdgeOrVertex, History, LoopChange, NameId, NameList, NameSet, OpId, OpPath, Ordinal, Origin,
    PieceKey, Record as Entry, Role, RoleArg, Side, TopoName, TopologyChange, Vanish,
};
use geom_run::{Budget, WorkMeter};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn n(x: u8) -> NameId {
    NameId::from_bytes([x; 16])
}

fn set(xs: &[u8]) -> NameSet {
    NameSet::new(xs.iter().map(|&x| n(x))).unwrap()
}

fn op() -> OpPath {
    OpPath::new(OpId::from_bytes([0x42; 16]))
        .step("cut")
        .unwrap()
        .item([7; 16])
}

fn role() -> Role {
    Role::new("side", [RoleArg::Name(n(3)), RoleArg::Count(2)]).unwrap()
}

/// One name of every variant.
fn names() -> Vec<TopoName> {
    vec![
        TopoName::Created {
            op: op(),
            role: role(),
            from: NameList::new([n(2), n(1), n(2)]),
        },
        TopoName::Split {
            op: op(),
            of: n(4),
            piece: PieceKey::ByTools {
                tools: set(&[5, 6]),
                side: Side::Front,
            },
        },
        TopoName::Merged {
            op: op(),
            of: set(&[7, 8]),
        },
        TopoName::Bounded {
            kind: EdgeOrVertex::Vertex,
            by: set(&[9, 10, 11]),
            ends: set(&[]),
        },
        TopoName::Weak {
            base: n(12),
            ord: Ordinal::new(1, 3).unwrap(),
        },
    ]
}

fn graph() -> NameGraph {
    let mut graph = NameGraph::new();
    for name in names() {
        graph.intern(name).unwrap();
    }
    graph
}

/// The record through the record layer and back: what a file holds.
fn through_file(record: &Record) -> Record {
    let bytes = record.encode(&mut meter()).unwrap();
    let (back, used) = Record::decode(&bytes, &mut meter()).unwrap();
    assert_eq!(used, bytes.len());
    back
}

#[test]
fn a_name_graph_round_trips_byte_for_byte() {
    let graph = graph();
    let record = through_file(&names_record(&graph).unwrap());
    assert_eq!((record.tag, record.flags, record.schema), (16, 1, 1));
    let back = read_names(&record.payload, 1, &mut meter()).unwrap();
    let ids: Vec<NameId> = graph.ids().collect();
    assert_eq!(back.ids().collect::<Vec<_>>(), ids);
    for id in ids {
        assert_eq!(back.bytes(id), graph.bytes(id));
        assert_eq!(back.name(id), graph.name(id));
    }
    // Each name alone.
    for name in names() {
        let bytes = name.encode().unwrap().into_bytes();
        assert_eq!(read_topo_name(&bytes, &mut meter()).unwrap(), name);
    }
    // An empty graph.
    let empty = names_record(&NameGraph::new()).unwrap();
    assert_eq!(
        read_names(&empty.payload, 1, &mut meter()).unwrap().len(),
        0
    );
}

#[test]
fn a_names_record_is_refused_under_another_scheme() {
    let record = names_record(&graph()).unwrap();
    assert_eq!(
        read_names(&record.payload, 2, &mut meter()).unwrap_err(),
        NamesError::Scheme(2)
    );
}

#[test]
fn a_node_whose_id_is_not_its_hash_is_refused() {
    let graph = graph();
    let mut payload = names_record(&graph).unwrap().payload;
    // The first node's id starts at byte 3: the array head, the pair's head, the bytes' head.
    payload[3] ^= 1;
    let error = read_names(&payload, 1, &mut meter()).unwrap_err();
    assert!(
        matches!(
            error,
            NamesError::WrongId { offset: 1, .. } | NamesError::Order { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn nodes_out_of_order_or_repeated_are_refused() {
    let graph = graph();
    let ids: Vec<NameId> = graph.ids().collect();
    let node = |id: NameId| {
        let mut e = geom_op::cbor::Encoder::new();
        e.array(2).unwrap();
        e.bytes(&id.to_bytes()).unwrap();
        e.item(&graph.name(id).unwrap().encode().unwrap()).unwrap();
        e.finish().unwrap().into_bytes()
    };
    for order in [[1_usize, 0], [0, 0]] {
        let mut payload = vec![0x82];
        for at in order {
            payload.extend(node(ids[at]));
        }
        let error = read_names(&payload, 1, &mut meter()).unwrap_err();
        assert!(matches!(error, NamesError::Order { .. }), "{error:?}");
    }
}

#[test]
fn a_second_spelling_of_a_name_is_refused() {
    // A merge whose set is out of order: [2, op, [8, 7]].
    let canonical = TopoName::Merged {
        op: op(),
        of: set(&[7, 8]),
    }
    .encode()
    .unwrap()
    .into_bytes();
    let mut swapped = canonical.clone();
    let seven = swapped.windows(16).position(|w| w == [7; 16]).unwrap();
    let eight = swapped.windows(16).position(|w| w == [8; 16]).unwrap();
    swapped[seven..seven + 16].copy_from_slice(&[8; 16]);
    swapped[eight..eight + 16].copy_from_slice(&[7; 16]);
    let error = read_topo_name(&swapped, &mut meter()).unwrap_err();
    assert!(
        matches!(error, NamesError::NotCanonical { .. } | NamesError::Name(_)),
        "{error:?}"
    );
    // An unknown variant.
    assert!(matches!(
        read_topo_name(&[0x82, 0x09, 0x00], &mut meter()).unwrap_err(),
        NamesError::Payload(_)
    ));
}

fn rec(output: u8, origin: Origin) -> Entry {
    Entry {
        output: n(output),
        origin,
    }
}

/// A history with every origin and every topology change.
fn full_history() -> History {
    History::new(
        [
            rec(1, Origin::Unchanged),
            rec(20, Origin::Modified { from: set(&[2]) }),
            rec(
                30,
                Origin::Generated {
                    from: set(&[3]),
                    role: role(),
                },
            ),
            rec(
                40,
                Origin::Split {
                    of: n(4),
                    piece: PieceKey::ByTools {
                        tools: set(&[9]),
                        side: Side::Back,
                    },
                },
            ),
            rec(50, Origin::Merged { of: set(&[5, 6]) }),
        ],
        set(&[3]),
        [
            TopologyChange::VertexSplit {
                vertex: n(1),
                edges: NameList::new([n(60), n(61)]),
            },
            TopologyChange::FaceVanished {
                face: n(3),
                why: Vanish::InsideOut,
            },
            TopologyChange::SelfOverlapCut {
                face: n(2),
                along: NameList::new([n(62)]),
            },
            TopologyChange::ClashTrimmed { faces: set(&[2]) },
            TopologyChange::OvershootRemoved { faces: set(&[3]) },
            TopologyChange::FacesMerged {
                faces: set(&[5, 6]),
                into: n(50),
            },
            TopologyChange::FaceAdded {
                face: n(30),
                role: role(),
            },
            TopologyChange::LoopChanged {
                face: n(4),
                change: LoopChange::FaceSplit,
            },
            TopologyChange::BodySplit {
                lumps: NameList::new([n(70)]),
            },
        ],
    )
    .unwrap()
}

#[test]
fn a_history_round_trips_at_its_path() {
    let history = full_history();
    let record = through_file(&history_record(&op(), &history).unwrap());
    assert_eq!((record.tag, record.flags, record.schema), (17, 1, 1));
    let (path, back) = read_history(&record.payload, &mut meter()).unwrap();
    assert_eq!(path, op());
    assert_eq!(back, history);
    let empty = history_record(&op(), &History::default()).unwrap();
    assert_eq!(
        read_history(&empty.payload, &mut meter()).unwrap(),
        (op(), History::default())
    );
}

#[test]
fn a_broken_history_is_refused() {
    let payload = history_record(&op(), &full_history()).unwrap().payload;
    // Bytes after the history.
    let mut longer = payload.clone();
    longer.push(0);
    assert!(read_history(&longer, &mut meter()).is_err());
    // An origin kind that does not exist: the first record's origin [0] becomes [5].
    let at = payload
        .windows(3)
        .position(|w| w == [0x82, 0x50, 0x01])
        .unwrap()
        + 2
        + 16;
    let mut unknown = payload.clone();
    assert_eq!(unknown[at..at + 2], [0x81, 0x00]);
    unknown[at + 1] = 0x05;
    assert!(matches!(
        read_history(&unknown, &mut meter()).unwrap_err(),
        HistoryRecordError::Names(NamesError::Payload(_))
    ));
    // Two records of one output.
    let mut twice = payload;
    let second = twice
        .windows(17)
        .position(|w| w[0] == 0x50 && w[1..] == [20; 16])
        .unwrap();
    twice[second + 1..second + 17].copy_from_slice(&[1; 16]);
    assert!(read_history(&twice, &mut meter()).is_err());
}

fn config(cases: u32) -> Config {
    Config {
        cases,
        rng_seed: RngSeed::Fixed(0x0000_6795),
        failure_persistence: None,
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(config(128))]

    #[test]
    fn generated_histories_round_trip(
        (history, _) in geom_test::history::step(0, (1..=20).map(n).collect()),
    ) {
        let record = history_record(&op(), &history).unwrap();
        let (path, back) = read_history(&record.payload, &mut meter()).unwrap();
        prop_assert_eq!(path, op());
        prop_assert_eq!(back, history);
    }
}
