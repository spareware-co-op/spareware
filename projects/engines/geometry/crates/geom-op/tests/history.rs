//! Tests of `geom_op::history` through the public interface (piece G88).
//!
//! What this file proves:
//!
//! - A history keeps its records in output-name order, refuses two records of one output, a
//!   modification from nothing and a merge of one, and finds each output's origin.
//! - Its relations are the method note's: unchanged and split, merged and modified outputs are
//!   modified images, generated outputs generated images or made from nothing.
//! - Laws 1 and 3 are found where they break, a fillet's removed-yet-generating edge is allowed,
//!   and each 13.14 change is checked against the records as its row says.
//! - Composition follows chapter 20.3's rules on a hand-worked chain, the "two paths" case
//!   included: modified wins.
//! - A history encodes to exactly the bytes its doc comment states, every origin and every change
//!   class, checked against the independent reference encoder (ciborium-ll).

#![allow(
    clippy::arithmetic_side_effects,
    reason = "test cases; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::{V, hex, reference};
use geom_op::{
    ChangeClass, History, HistoryError, LawBreak, LoopChange, NameId, NameList, NameSet, OpId,
    OpPath, Origin, PieceKey, Record, Role, Side, TopologyChange, Vanish,
};

fn n(x: u8) -> NameId {
    NameId::from_bytes([x; 16])
}

fn set(xs: &[u8]) -> NameSet {
    NameSet::new(xs.iter().map(|&x| n(x))).unwrap()
}

fn role(name: &str) -> Role {
    Role::new(name, []).unwrap()
}

fn rec(output: u8, origin: Origin) -> Record {
    Record {
        output: n(output),
        origin,
    }
}

fn modified(xs: &[u8]) -> Origin {
    Origin::Modified { from: set(xs) }
}

fn generated(xs: &[u8], r: &str) -> Origin {
    Origin::Generated {
        from: set(xs),
        role: role(r),
    }
}

fn split(of: u8, tool: u8, side: Side) -> Origin {
    Origin::Split {
        of: n(of),
        piece: PieceKey::ByTools {
            tools: set(&[tool]),
            side,
        },
    }
}

fn history(records: Vec<Record>, deleted: &[u8]) -> History {
    History::new(records, set(deleted), []).unwrap()
}

#[test]
fn records_are_in_output_name_order_and_found_by_name() {
    let h = History::new(
        [
            rec(9, Origin::Unchanged),
            rec(2, modified(&[1])),
            rec(5, generated(&[], "cap")),
        ],
        set(&[7, 3]),
        [
            TopologyChange::BodySplit {
                lumps: NameList::new([n(9)]),
            },
            TopologyChange::ClashTrimmed { faces: set(&[1]) },
        ],
    )
    .unwrap();
    let outputs: Vec<NameId> = h.records().iter().map(|r| r.output).collect();
    assert_eq!(outputs, [n(2), n(5), n(9)]);
    assert_eq!(h.deleted(), &set(&[3, 7]));
    assert_eq!(
        h.changes()
            .iter()
            .map(TopologyChange::class)
            .collect::<Vec<_>>(),
        [ChangeClass::ClashTrimmed, ChangeClass::BodySplit]
    );
    assert_eq!(h.origin(n(2)), Some(&modified(&[1])));
    assert_eq!(h.origin(n(9)), Some(&Origin::Unchanged));
    assert_eq!(h.origin(n(4)), None);
    assert_eq!(History::default().records(), []);
}

#[test]
fn malformed_records_are_refused() {
    let twice = History::new(
        [rec(2, modified(&[1])), rec(2, Origin::Unchanged)],
        set(&[]),
        [],
    );
    assert_eq!(twice, Err(HistoryError::RepeatedOutput { output: n(2) }));
    let nothing = History::new([rec(2, modified(&[]))], set(&[]), []);
    assert_eq!(
        nothing,
        Err(HistoryError::ModifiedFromNothing { output: n(2) })
    );
    let one = History::new([rec(2, Origin::Merged { of: set(&[1]) })], set(&[]), []);
    assert_eq!(one, Err(HistoryError::MergeOfOne { output: n(2) }));
    // Two inputs merge; a generated output may come from nothing.
    assert!(History::new([rec(2, Origin::Merged { of: set(&[1, 3]) })], set(&[]), []).is_ok());
    assert!(History::new([rec(2, generated(&[], "cap"))], set(&[]), []).is_ok());
    let id = "02".repeat(16);
    assert_eq!(
        HistoryError::RepeatedOutput { output: n(2) }.to_string(),
        format!("the output {id} has two records")
    );
    assert_eq!(
        HistoryError::ModifiedFromNothing { output: n(2) }.to_string(),
        format!("the output {id} is modified from no input; one made from nothing is generated")
    );
    assert_eq!(
        HistoryError::MergeOfOne { output: n(2) }.to_string(),
        format!(
            "the output {id} is merged from fewer than two inputs; one input changed is modified"
        )
    );
}

#[test]
fn relations_are_the_method_notes() {
    let h = history(
        vec![
            rec(1, Origin::Unchanged),
            rec(20, modified(&[2])),
            rec(30, split(3, 99, Side::Back)),
            rec(31, split(3, 99, Side::Front)),
            rec(40, Origin::Merged { of: set(&[4, 5]) }),
            rec(60, generated(&[6, 7], "roll")),
            rec(70, generated(&[], "cap")),
        ],
        &[6],
    );
    let r = h.relations();
    assert_eq!(
        r.outputs().collect::<Vec<_>>(),
        [n(1), n(20), n(30), n(31), n(40), n(60), n(70)]
    );
    assert_eq!(
        r.modified().collect::<Vec<_>>(),
        [
            (n(1), n(1)),
            (n(2), n(20)),
            (n(3), n(30)),
            (n(3), n(31)),
            (n(4), n(40)),
            (n(5), n(40)),
        ]
    );
    assert_eq!(
        r.generated().collect::<Vec<_>>(),
        [(n(6), n(60)), (n(7), n(60))]
    );
    assert_eq!(r.from_nothing().collect::<Vec<_>>(), [n(70)]);
    assert_eq!(r.deleted().collect::<Vec<_>>(), [n(6)]);
    // A fillet: the rounded edge is deleted and generates its face. Laws 1 to 3 allow it.
    assert_eq!(h.check(&set(&[1, 2, 3, 4, 5, 6, 7])), Ok(()));
}

#[test]
fn law_1_a_deleted_input_has_no_modified_output() {
    let inputs = set(&[1, 2, 3, 4, 5]);
    for (record, input, output) in [
        (rec(1, Origin::Unchanged), 1, 1),
        (rec(20, modified(&[2])), 2, 20),
        (rec(30, split(3, 9, Side::Back)), 3, 30),
        (rec(40, Origin::Merged { of: set(&[4, 5]) }), 4, 40),
    ] {
        let h = history(vec![record], &[input]);
        assert_eq!(
            h.check(&inputs),
            Err(LawBreak::DeletedButModified {
                input: n(input),
                output: n(output)
            })
        );
    }
    let broken = LawBreak::DeletedButModified {
        input: n(1),
        output: n(2),
    };
    assert_eq!(
        broken.to_string(),
        format!(
            "law 1: the deleted input {} has the modified output {}",
            "01".repeat(16),
            "02".repeat(16)
        )
    );
}

#[test]
fn law_3_everything_named_as_an_input_is_one() {
    let inputs = set(&[1, 2]);
    for (h, name) in [
        (history(vec![rec(20, modified(&[3]))], &[]), 3),
        (history(vec![rec(20, generated(&[1, 4], "r"))], &[]), 4),
        (history(vec![rec(20, split(5, 1, Side::Front))], &[]), 5),
        (history(vec![rec(6, Origin::Unchanged)], &[]), 6),
        (history(vec![rec(1, Origin::Unchanged)], &[7]), 7),
    ] {
        assert_eq!(
            h.check(&inputs),
            Err(LawBreak::NotAnInput { name: n(name) }),
            "{h:?}"
        );
    }
    assert_eq!(
        LawBreak::NotAnInput { name: n(3) }.to_string(),
        format!(
            "law 3: {} is named as an input but is not one",
            "03".repeat(16)
        )
    );
}

fn with_change(records: Vec<Record>, deleted: &[u8], change: TopologyChange) -> History {
    History::new(records, set(deleted), [change]).unwrap()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one case per row of 13.14's table, side by side"
)]
fn each_topology_change_is_checked_against_the_records() {
    let inputs = set(&[1, 2, 3]);
    let missing = |class, entity: u8| {
        Err(LawBreak::ChangeNotRecorded {
            class,
            entity: n(entity),
        })
    };
    let vanished = TopologyChange::FaceVanished {
        face: n(1),
        why: Vanish::Collapsed,
    };
    assert_eq!(
        with_change(vec![], &[1], vanished.clone()).check(&inputs),
        Ok(())
    );
    assert_eq!(
        with_change(vec![], &[], vanished).check(&inputs),
        missing(ChangeClass::FaceVanished, 1)
    );
    let overshoot = TopologyChange::OvershootRemoved {
        faces: set(&[1, 2]),
    };
    assert_eq!(
        with_change(vec![], &[1, 2], overshoot.clone()).check(&inputs),
        Ok(())
    );
    assert_eq!(
        with_change(vec![], &[1], overshoot).check(&inputs),
        missing(ChangeClass::OvershootRemoved, 2)
    );
    let merged = TopologyChange::FacesMerged {
        faces: set(&[1, 2]),
        into: n(9),
    };
    let merge = |of: &[u8]| rec(9, Origin::Merged { of: set(of) });
    assert_eq!(
        with_change(vec![merge(&[1, 2])], &[], merged.clone()).check(&inputs),
        Ok(())
    );
    assert_eq!(
        with_change(vec![merge(&[1, 3])], &[], merged.clone()).check(&inputs),
        missing(ChangeClass::FacesMerged, 9)
    );
    assert_eq!(
        with_change(vec![rec(9, modified(&[1]))], &[], merged).check(&inputs),
        missing(ChangeClass::FacesMerged, 9)
    );
    let added = TopologyChange::FaceAdded {
        face: n(9),
        role: role("rim"),
    };
    assert_eq!(
        with_change(vec![rec(9, generated(&[1], "rim"))], &[], added.clone()).check(&inputs),
        Ok(())
    );
    assert_eq!(
        with_change(vec![rec(9, generated(&[1], "cap"))], &[], added.clone()).check(&inputs),
        missing(ChangeClass::FaceAdded, 9)
    );
    assert_eq!(
        with_change(vec![rec(9, modified(&[1]))], &[], added.clone()).check(&inputs),
        missing(ChangeClass::FaceAdded, 9)
    );
    assert_eq!(
        with_change(vec![], &[], added).check(&inputs),
        missing(ChangeClass::FaceAdded, 9)
    );
    // The classes with no record of their own to check.
    for change in [
        TopologyChange::VertexSplit {
            vertex: n(1),
            edges: NameList::new([n(8)]),
        },
        TopologyChange::SelfOverlapCut {
            face: n(1),
            along: NameList::new([n(8)]),
        },
        TopologyChange::ClashTrimmed { faces: set(&[1]) },
        TopologyChange::LoopChanged {
            face: n(1),
            change: LoopChange::HoleClosed,
        },
        TopologyChange::BodySplit {
            lumps: NameList::new([n(8)]),
        },
    ] {
        assert_eq!(with_change(vec![], &[], change).check(&inputs), Ok(()));
    }
    // A law break comes before a change's.
    assert_eq!(
        History::new(
            [rec(1, Origin::Unchanged)],
            set(&[1]),
            [TopologyChange::FaceVanished {
                face: n(2),
                why: Vanish::InsideOut
            }]
        )
        .unwrap()
        .check(&inputs),
        Err(LawBreak::DeletedButModified {
            input: n(1),
            output: n(1)
        })
    );
    assert_eq!(
        LawBreak::ChangeNotRecorded {
            class: ChangeClass::FaceAdded,
            entity: n(9)
        }
        .to_string(),
        format!(
            "the FaceAdded change of {} is not recorded in the history (13.14)",
            "09".repeat(16)
        )
    );
}

#[test]
fn every_change_has_its_class() {
    let pairs = [
        (
            TopologyChange::VertexSplit {
                vertex: n(1),
                edges: NameList::new([]),
            },
            ChangeClass::VertexSplit,
        ),
        (
            TopologyChange::FaceVanished {
                face: n(1),
                why: Vanish::Collapsed,
            },
            ChangeClass::FaceVanished,
        ),
        (
            TopologyChange::SelfOverlapCut {
                face: n(1),
                along: NameList::new([]),
            },
            ChangeClass::SelfOverlapCut,
        ),
        (
            TopologyChange::ClashTrimmed { faces: set(&[]) },
            ChangeClass::ClashTrimmed,
        ),
        (
            TopologyChange::OvershootRemoved { faces: set(&[]) },
            ChangeClass::OvershootRemoved,
        ),
        (
            TopologyChange::FacesMerged {
                faces: set(&[]),
                into: n(1),
            },
            ChangeClass::FacesMerged,
        ),
        (
            TopologyChange::FaceAdded {
                face: n(1),
                role: role("r"),
            },
            ChangeClass::FaceAdded,
        ),
        (
            TopologyChange::LoopChanged {
                face: n(1),
                change: LoopChange::FaceSplit,
            },
            ChangeClass::LoopChanged,
        ),
        (
            TopologyChange::BodySplit {
                lumps: NameList::new([]),
            },
            ChangeClass::BodySplit,
        ),
    ];
    for (change, class) in pairs {
        assert_eq!(change.class(), class);
    }
}

#[test]
fn a_hand_worked_chain_composes_by_chapter_20_3s_rules() {
    // Step a, over inputs 1..=6: 1 unchanged; 2 modified to 20; 3 deleted, generating 30; 4 split
    // into 40 and 41; 5 modified to 50, which also generates 51 (a pipe kept its spine); 6
    // deleted; 70 made from nothing.
    let a = history(
        vec![
            rec(1, Origin::Unchanged),
            rec(20, modified(&[2])),
            rec(30, generated(&[3], "roll")),
            rec(40, split(4, 9, Side::Back)),
            rec(41, split(4, 9, Side::Front)),
            rec(50, modified(&[5])),
            rec(51, generated(&[5], "wall")),
            rec(70, generated(&[], "cap")),
        ],
        &[3, 6],
    );
    // Step b, over a's outputs: 1 deleted; 20 merged with 30 into 100; 40 deleted, 41 unchanged;
    // 50 and 51 merged into 110 (two paths to input 5); 70 modified to 120; 130 from nothing.
    let b = history(
        vec![
            rec(100, Origin::Merged { of: set(&[20, 30]) }),
            rec(41, Origin::Unchanged),
            rec(110, Origin::Merged { of: set(&[50, 51]) }),
            rec(120, modified(&[70])),
            rec(130, generated(&[], "lid")),
        ],
        &[1, 40],
    );
    assert_eq!(a.check(&set(&[1, 2, 3, 4, 5, 6])), Ok(()));
    assert_eq!(b.check(&set(&[1, 20, 30, 40, 41, 50, 51, 70])), Ok(()));
    let chain = a.relations().then(&b.relations());
    assert_eq!(
        chain.outputs().collect::<Vec<_>>(),
        [n(41), n(100), n(110), n(120), n(130)]
    );
    // Modified: images of modified images.
    assert_eq!(
        chain.modified().collect::<Vec<_>>(),
        [(n(2), n(100)), (n(4), n(41)), (n(5), n(110))]
    );
    // Generated: every other path; 5 -> 110 also arrives through 51, and modified wins.
    assert_eq!(chain.generated().collect::<Vec<_>>(), [(n(3), n(100))]);
    // From nothing: 70's descendant and b's own new output.
    assert_eq!(chain.from_nothing().collect::<Vec<_>>(), [n(120), n(130)]);
    // Deleted: a's deletions, and 1, whose only modified image b deleted; 4 keeps a piece.
    assert_eq!(chain.deleted().collect::<Vec<_>>(), [n(1), n(3), n(6)]);
    assert_eq!(chain.check(&set(&[1, 2, 3, 4, 5, 6])), Ok(()));
    // An empty step after a history keeps nothing: every input with a modified image is deleted.
    let none = a.relations().then(&History::default().relations());
    assert_eq!(none.outputs().count(), 0);
    assert_eq!(none.modified().count(), 0);
    assert_eq!(none.generated().count(), 0);
    assert_eq!(
        none.deleted().collect::<Vec<_>>(),
        [n(1), n(2), n(3), n(4), n(5), n(6)]
    );
}

fn name_v(x: u8) -> V {
    V::B(vec![x; 16])
}

fn set_v(xs: &[u8]) -> V {
    V::Set(xs.iter().map(|&x| name_v(x)).collect())
}

fn list_v(xs: &[u8]) -> V {
    V::Sorted(xs.iter().map(|&x| name_v(x)).collect())
}

fn role_v(name: &str) -> V {
    V::A(vec![V::T(name.to_owned())])
}

#[test]
fn a_history_encodes_as_its_doc_comment_states() {
    let op = OpPath::new(OpId::from_bytes([9; 16]));
    let h = History::new(
        [
            rec(14, Origin::Merged { of: set(&[6, 7]) }),
            rec(10, Origin::Unchanged),
            rec(11, modified(&[1, 2])),
            rec(12, generated(&[3], "cap")),
            rec(13, split(4, 5, Side::Front)),
        ],
        set(&[8]),
        [
            TopologyChange::BodySplit {
                lumps: NameList::new([n(37), n(36)]),
            },
            TopologyChange::VertexSplit {
                vertex: n(30),
                edges: NameList::new([n(32), n(31)]),
            },
            TopologyChange::FaceVanished {
                face: n(20),
                why: Vanish::Collapsed,
            },
            TopologyChange::FaceVanished {
                face: n(21),
                why: Vanish::InsideOut,
            },
            TopologyChange::SelfOverlapCut {
                face: n(22),
                along: NameList::new([n(23)]),
            },
            TopologyChange::ClashTrimmed {
                faces: set(&[24, 25]),
            },
            TopologyChange::OvershootRemoved { faces: set(&[26]) },
            TopologyChange::FacesMerged {
                faces: set(&[27, 28]),
                into: n(29),
            },
            TopologyChange::FaceAdded {
                face: n(33),
                role: role("rim"),
            },
            TopologyChange::LoopChanged {
                face: n(34),
                change: LoopChange::HoleClosed,
            },
            TopologyChange::LoopChanged {
                face: n(35),
                change: LoopChange::FaceSplit,
            },
        ],
    )
    .unwrap();
    let record = |output: u8, origin: Vec<V>| V::A(vec![name_v(output), V::A(origin)]);
    let expected = V::A(vec![
        V::A(vec![name_v(9)]),
        V::A(vec![
            record(10, vec![V::U(0)]),
            record(11, vec![V::U(1), set_v(&[1, 2])]),
            record(12, vec![V::U(2), set_v(&[3]), role_v("cap")]),
            record(
                13,
                vec![
                    V::U(3),
                    name_v(4),
                    V::A(vec![V::U(0), set_v(&[5]), V::U(1)]),
                ],
            ),
            record(14, vec![V::U(4), set_v(&[6, 7])]),
        ]),
        set_v(&[8]),
        V::A(vec![
            V::A(vec![V::U(0), name_v(30), list_v(&[31, 32])]),
            V::A(vec![V::U(1), name_v(20), V::U(0)]),
            V::A(vec![V::U(1), name_v(21), V::U(1)]),
            V::A(vec![V::U(2), name_v(22), list_v(&[23])]),
            V::A(vec![V::U(3), set_v(&[24, 25])]),
            V::A(vec![V::U(4), set_v(&[26])]),
            V::A(vec![V::U(5), set_v(&[27, 28]), name_v(29)]),
            V::A(vec![V::U(6), name_v(33), role_v("rim")]),
            V::A(vec![V::U(7), name_v(34), V::U(0)]),
            V::A(vec![V::U(7), name_v(35), V::U(1)]),
            V::A(vec![V::U(8), list_v(&[36, 37])]),
        ]),
    ]);
    assert_eq!(
        hex(h.encode(&op).unwrap().as_bytes()),
        hex(&reference(&expected))
    );
    let empty = V::A(vec![
        V::A(vec![name_v(9)]),
        V::A(vec![]),
        V::Set(vec![]),
        V::A(vec![]),
    ]);
    assert_eq!(
        hex(History::default().encode(&op).unwrap().as_bytes()),
        hex(&reference(&empty))
    );
}
