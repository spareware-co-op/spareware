//! Tests of `PruneHistory` (piece G109) through `geom-name`'s public interface.
//!
//! What this file proves:
//!
//! - A pruned step keeps its place and its path, names the operation and the held set, and keeps
//!   exactly the records and deletions walks from held names meet; generated records, topology
//!   changes and the records of names no held walk reaches go.
//! - Every held reference resolves as it did, from every step it could have been made at and for
//!   every want, on generated chains (`geom-test`'s history kit, fixed seed); a reference to any
//!   other name that meets a pruned step answers `HistoryPruned` with the operation, never
//!   `Deleted`.
//! - Pruning again leaves the steps an earlier prune kept as they were, and every reference the
//!   second prune holds resolves as it did after the first; a step pruned in a linked version
//!   stays as it is.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic and indexing, short names for the entities of each case, and a \
              failing test should stop loudly"
)]

use std::collections::BTreeMap;

use geom_name::resolve::Step;
use geom_name::{
    Body, CarrierIx, ElementId, EntityKind, Lookup, NameGraph, Reference, Resolution, Signature,
    Target, Want, prune, resolve,
};
use geom_op::history::{History, Origin, Record, TopologyChange};
use geom_op::name::{NameList, NameSet, PieceKey, Role, Side, TopoName};
use geom_op::{NameId, OpId, OpPath};
use geom_test::history::chain;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

fn op(byte: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([byte; 16]))
}

fn set(names: &[NameId]) -> NameSet {
    NameSet::new(names.iter().copied()).unwrap()
}

fn face(graph: &mut NameGraph, role: &str) -> NameId {
    graph
        .intern(TopoName::Created {
            op: op(0),
            role: Role::new(role, []).unwrap(),
            from: NameList::new([]),
        })
        .unwrap()
}

fn record(output: NameId, origin: Origin) -> Record {
    Record { output, origin }
}

fn split(of: NameId, tool: NameId, side: Side) -> Origin {
    Origin::Split {
        of,
        piece: PieceKey::ByTools {
            tools: set(&[tool]),
            side,
        },
    }
}

fn reference(name: NameId, want: Want) -> Reference {
    Reference {
        doc: None,
        element: ElementId([0xE1; 16]),
        name,
        kind: EntityKind::Face,
        want,
        signature: Signature::default(),
    }
}

/// `name` resolved for `want` against `body`, with `history` since the reference was made.
fn resolved(
    graph: &NameGraph,
    body: &NameSet,
    carriers: &[(NameId, CarrierIx)],
    history: &[Step],
    name: NameId,
    want: Want,
) -> Resolution {
    let lookup = Lookup::Found {
        body: Body {
            names: body,
            carriers,
            history,
            graph,
        },
        version: None,
    };
    resolve(&reference(name, want), &lookup)
}

const WANTS: [Want; 3] = [Want::One, Want::AllPieces, Want::Carrier];

#[test]
fn a_pruned_step_keeps_what_held_walks_meet() {
    let mut graph = NameGraph::new();
    let [a, b, c, d, e, f, g] = ["a", "b", "c", "d", "e", "f", "g"].map(|r| face(&mut graph, r));
    let cap = Role::new("cap", []).unwrap();
    // Step 1 splits a into b and c by e, keeps e, deletes f while generating g from it.
    let first = History::new(
        [
            record(b, split(a, e, Side::Back)),
            record(c, split(a, e, Side::Front)),
            record(e, Origin::Unchanged),
            record(
                g,
                Origin::Generated {
                    from: set(&[f]),
                    role: cap.clone(),
                },
            ),
        ],
        set(&[f]),
        [TopologyChange::FaceAdded {
            face: g,
            role: cap,
        }],
    )
    .unwrap();
    // Step 2 modifies b into d, deletes c, keeps e and g.
    let second = History::new(
        [
            record(d, Origin::Modified { from: set(&[b]) }),
            record(e, Origin::Unchanged),
            record(g, Origin::Unchanged),
        ],
        set(&[c]),
        [],
    )
    .unwrap();
    let history = [
        Step::Recorded {
            by: op(1),
            history: first.clone(),
        },
        Step::Recorded {
            by: op(2),
            history: second,
        },
    ];
    let held = set(&[a]);
    let pruned = prune(&history, &held, &op(9));
    let kept = |by: u8, records: Vec<Record>, deleted: &[NameId]| Step::Kept {
        by: op(by),
        history: History::new(records, set(deleted), []).unwrap(),
        pruned_by: op(9),
        held: held.clone(),
    };
    assert_eq!(
        pruned,
        [
            kept(
                1,
                vec![
                    record(b, split(a, e, Side::Back)),
                    record(c, split(a, e, Side::Front))
                ],
                &[]
            ),
            kept(
                2,
                vec![record(d, Origin::Modified { from: set(&[b]) })],
                &[c]
            ),
        ]
    );
    let body = set(&[d, e, g]);
    let carriers = [(d, CarrierIx(1)), (e, CarrierIx(2)), (g, CarrierIx(3))];
    let before = |from: usize, name, want| resolved(&graph, &body, &carriers, &history[from..], name, want);
    let after = |from: usize, name, want| resolved(&graph, &body, &carriers, &pruned[from..], name, want);
    // The held face resolves as it did, from either step.
    for want in WANTS {
        for from in 0..=2 {
            assert_eq!(after(from, a, want), before(from, a, want), "{from} {want:?}");
        }
    }
    assert_eq!(
        after(0, a, Want::One),
        Resolution::FollowedHistory {
            target: Target::One(d),
            weak: false
        }
    );
    // A face no reference holds: in the body, exact as before; deleted by step 1, now pruned.
    assert_eq!(after(0, e, Want::One), before(0, e, Want::One));
    assert_eq!(before(0, f, Want::One), Resolution::Deleted { by: op(1) });
    assert_eq!(
        after(0, f, Want::One),
        Resolution::HistoryPruned { by: Some(op(9)) }
    );
    assert_eq!(
        after(1, b, Want::AllPieces),
        Resolution::HistoryPruned { by: Some(op(9)) }
    );
    // Nothing is held: every step keeps nothing, and every walk is pruned.
    let none = prune(&history, &NameSet::default(), &op(9));
    assert!(none.iter().all(|s| matches!(s, Step::Kept { history, .. } if *history == History::default())));
    assert_eq!(
        resolved(&graph, &body, &carriers, &none, a, Want::One),
        Resolution::HistoryPruned { by: Some(op(9)) }
    );
    // A step pruned in a linked version, and a step an earlier prune kept, stay as they are.
    let linked = [
        Step::Pruned { by: None },
        pruned[0].clone(),
        Step::Recorded {
            by: op(3),
            history: first,
        },
    ];
    let again = prune(&linked, &set(&[b]), &op(10));
    assert_eq!(again[..2], linked[..2]);
    assert!(matches!(&again[2], Step::Kept { by, pruned_by, .. } if *by == op(3) && *pruned_by == op(10)));
}

fn config(cases: u32) -> Config {
    Config {
        cases,
        rng_seed: RngSeed::Fixed(0x0000_0109),
        failure_persistence: None,
        ..Config::default()
    }
}

fn inputs(count: u8) -> Vec<NameId> {
    (1..=count).map(|x| NameId::from_bytes([x; 16])).collect()
}

/// A generated chain's names, each interned as a face of its own, so the graph classifies every
/// name and none is weak.
#[derive(Default)]
struct Interned {
    graph: NameGraph,
    names: BTreeMap<NameId, NameId>,
}

impl Interned {
    fn name(&mut self, n: NameId) -> NameId {
        let graph = &mut self.graph;
        *self.names.entry(n).or_insert_with(|| {
            graph
                .intern(TopoName::Created {
                    op: OpPath::new(OpId::from_bytes(n.to_bytes())),
                    role: Role::new("face", []).unwrap(),
                    from: NameList::new([]),
                })
                .unwrap()
        })
    }

    fn set(&mut self, s: &NameSet) -> NameSet {
        let names: Vec<NameId> = s.names().iter().map(|&n| self.name(n)).collect();
        NameSet::new(names).unwrap()
    }

    fn origin(&mut self, origin: &Origin) -> Origin {
        match origin {
            Origin::Unchanged => Origin::Unchanged,
            Origin::Modified { from } => Origin::Modified {
                from: self.set(from),
            },
            Origin::Generated { from, role } => Origin::Generated {
                from: self.set(from),
                role: role.clone(),
            },
            Origin::Split {
                of,
                piece: PieceKey::ByTools { tools, side },
            } => Origin::Split {
                of: self.name(*of),
                piece: PieceKey::ByTools {
                    tools: self.set(tools),
                    side: *side,
                },
            },
            Origin::Merged { of } => Origin::Merged { of: self.set(of) },
        }
    }

    fn history(&mut self, history: &History) -> History {
        let records: Vec<Record> = history
            .records()
            .iter()
            .map(|r| Record {
                output: self.name(r.output),
                origin: self.origin(&r.origin),
            })
            .collect();
        let deleted = self.set(history.deleted());
        History::new(records, deleted, []).unwrap()
    }
}

/// A generated chain, interned: its recorded steps, every name it holds, the body after it and
/// carriers for most of the body.
struct Model {
    interned: Interned,
    history: Vec<Step>,
    names: Vec<NameId>,
    body: NameSet,
    carriers: Vec<(NameId, CarrierIx)>,
}

fn model(steps: &[(History, NameSet)]) -> Model {
    let mut interned = Interned::default();
    let histories: Vec<History> = steps.iter().map(|(h, _)| interned.history(h)).collect();
    let mut names: Vec<NameId> = interned.set(&steps[0].1).names().to_vec();
    for h in &histories {
        names.extend(h.records().iter().map(|r| r.output));
    }
    names.sort_unstable();
    names.dedup();
    let body = NameSet::new(histories.last().unwrap().records().iter().map(|r| r.output)).unwrap();
    let carriers: Vec<(NameId, CarrierIx)> = body
        .names()
        .iter()
        .filter(|n| n.to_bytes()[0] % 5 != 0)
        .map(|&n| (n, CarrierIx(u32::from(n.to_bytes()[0] % 3))))
        .collect();
    let history = (0_u8..)
        .zip(histories)
        .map(|(k, history)| Step::Recorded { by: op(k), history })
        .collect();
    Model {
        interned,
        history,
        names,
        body,
        carriers,
    }
}

/// The names of `names` whose bit in `mask` is set.
fn chosen(names: &[NameId], mask: u64) -> NameSet {
    NameSet::new(
        names
            .iter()
            .enumerate()
            .filter(|&(i, _)| mask >> (i % 64) & 1 == 1)
            .map(|(_, &n)| n),
    )
    .unwrap()
}

proptest! {
    #![proptest_config(config(256))]

    /// Every held name resolves as before from every step and for every want; every other name
    /// that is not in the body, with steps to walk, answers `HistoryPruned` with the operation.
    #[test]
    fn every_held_reference_resolves_as_it_did(
        steps in (1_u8..7, 1_u8..6).prop_flat_map(|(c, len)| chain(inputs(c), len)),
        mask in any::<u64>(),
    ) {
        let m = model(&steps);
        let held = chosen(&m.names, mask);
        let pruned = prune(&m.history, &held, &op(200));
        prop_assert_eq!(pruned.len(), m.history.len());
        let graph = &m.interned.graph;
        for from in 0..=m.history.len() {
            for &name in &m.names {
                for want in WANTS {
                    let before = resolved(graph, &m.body, &m.carriers, &m.history[from..], name, want);
                    let after = resolved(graph, &m.body, &m.carriers, &pruned[from..], name, want);
                    let held_here = held.names().binary_search(&name).is_ok();
                    let in_body = m.body.names().binary_search(&name).is_ok();
                    if held_here || in_body || from == m.history.len() {
                        prop_assert_eq!(after, before);
                    } else {
                        prop_assert_eq!(after, Resolution::HistoryPruned { by: Some(op(200)) });
                    }
                }
            }
        }
    }

    /// Pruning again keeps what the first prune kept, and every name the second holds resolves
    /// as it did after the first, from every step and for every want.
    #[test]
    fn pruning_again_prunes_only_what_is_new(
        steps in (1_u8..7, 2_u8..6).prop_flat_map(|(c, len)| chain(inputs(c), len)),
        cut in any::<prop::sample::Index>(),
        masks in (any::<u64>(), any::<u64>()),
    ) {
        let m = model(&steps);
        let at = 1 + cut.index(m.history.len() - 1);
        let first = chosen(&m.names, masks.0);
        let second = chosen(&m.names, masks.1);
        let mut once = prune(&m.history[..at], &first, &op(201));
        once.extend_from_slice(&m.history[at..]);
        let twice = prune(&once, &second, &op(202));
        prop_assert_eq!(&twice[..at], &once[..at]);
        let graph = &m.interned.graph;
        for from in 0..=m.history.len() {
            for &name in second.names() {
                for want in WANTS {
                    prop_assert_eq!(
                        resolved(graph, &m.body, &m.carriers, &twice[from..], name, want),
                        resolved(graph, &m.body, &m.carriers, &once[from..], name, want)
                    );
                }
            }
        }
    }
}
