//! Attacks on every claim piece G89 makes about references and how they resolve (chapter 20.4).
//!
//! What this file proves:
//!
//! - **Every outcome, as 20.4 sets it out**: `Exact`, `FollowedHistory`, `Ambiguous`,
//!   `WeakChanged`, `Deleted`, `HistoryPruned`, and `Unresolved` with each reason resolution itself
//!   gives (`ElementGone`, `LinkMissing`, `LinkOlder`, `NoCarrier`, `NameUnknown`,
//!   `HistoryBroken`), for references that want one entity, every piece or the carrier.
//! - **The walk follows modified images only**: through modifications, splits and merges over
//!   several steps, to the step that deleted the last descendant, and never through a generated
//!   output.
//! - **Pruned is never Deleted**: a walk that needs a pruned stretch is `HistoryPruned`, and a name
//!   deleted before the stretch stays `Deleted`.
//! - **Weak names** resolve with `weak` set, and a weak name gone while its base still names
//!   entities is `WeakChanged`.
//! - **Links** resolve inside the version supplied, and an older version without the name is
//!   `LinkOlder`.
//! - **No geometric guessing (20.4 step 5)**: every resolution in this file is repeated with other
//!   signatures, kinds and elements and comes out the same, and `src/resolve.rs`'s code names a
//!   reference's signature, kind and element nowhere but in their declarations.
//! - **The walk agrees with the history laws**: on generated chains of histories (`geom-test`'s
//!   history kit, fixed seed), every input resolves as the chain's composed relations
//!   (`Relations::then`, law 4) say, for each want, and every target is in the body.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic and indexing, short names for the entities of each case, and a               failing test should stop loudly"
)]

use std::collections::{BTreeMap, BTreeSet};

use geom_name::{
    Body, CarrierIx, DocRef, ElementId, EntityKind, Lookup, NameGraph, Reference, Resolution,
    Signature, Step, Target, UnresolvedReason, Want, resolve,
};
use geom_op::history::{History, Origin, Record};
use geom_op::name::{NameList, NameSet, Ordinal, PieceKey, Role, Side, TopoName};
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

/// A face made by step `step` in `role`, from `from`: weak if any of `from` is.
fn face_from(graph: &mut NameGraph, step: u8, role: &str, from: &[NameId]) -> NameId {
    graph
        .intern(TopoName::Created {
            op: op(step),
            role: Role::new(role, []).unwrap(),
            from: NameList::new(from.iter().copied()),
        })
        .unwrap()
}

/// A face made by step `step` in `role`, from nothing.
fn face(graph: &mut NameGraph, step: u8, role: &str) -> NameId {
    face_from(graph, step, role, &[])
}

/// The `n`-th of `of` entities that share `base`.
fn weak(graph: &mut NameGraph, base: NameId, n: u32, of: u32) -> NameId {
    graph
        .intern(TopoName::Weak {
            base,
            ord: Ordinal::new(n, of).unwrap(),
        })
        .unwrap()
}

fn unchanged(output: NameId) -> Record {
    Record {
        output,
        origin: Origin::Unchanged,
    }
}

fn modified(output: NameId, from: &[NameId]) -> Record {
    Record {
        output,
        origin: Origin::Modified { from: set(from) },
    }
}

fn generated(output: NameId, from: &[NameId]) -> Record {
    Record {
        output,
        origin: Origin::Generated {
            from: set(from),
            role: Role::new("cap", []).unwrap(),
        },
    }
}

fn split(output: NameId, of: NameId, tool: NameId, side: Side) -> Record {
    Record {
        output,
        origin: Origin::Split {
            of,
            piece: PieceKey::ByTools {
                tools: set(&[tool]),
                side,
            },
        },
    }
}

fn merged(output: NameId, of: &[NameId]) -> Record {
    Record {
        output,
        origin: Origin::Merged { of: set(of) },
    }
}

/// Step `by`, which recorded `records` and deleted `deleted`.
fn recorded(by: u8, records: Vec<Record>, deleted: &[NameId]) -> Step {
    Step::Recorded {
        by: op(by),
        history: History::new(records, set(deleted), []).unwrap(),
    }
}

fn pruned(by: Option<u8>) -> Step {
    Step::Pruned { by: by.map(op) }
}

/// A reference into its own document, with an empty signature.
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

fn carriers_of(carriers: &[(NameId, u32)]) -> Vec<(NameId, CarrierIx)> {
    let mut out: Vec<(NameId, CarrierIx)> =
        carriers.iter().map(|&(n, c)| (n, CarrierIx(c))).collect();
    out.sort_unstable();
    out
}

/// `resolve`, after checking that no other signature, kind or element changes what it gives (20.4
/// step 5).
fn checked(reference: &Reference, lookup: &Lookup<'_>) -> Resolution {
    let result = resolve(reference, lookup);
    let signatures = [
        Signature::default(),
        Signature(vec![0; 64]),
        Signature((0..=255).collect()),
        Signature(b"plane z=3000 normal +z".to_vec()),
    ];
    let kinds = [
        EntityKind::Body,
        EntityKind::Face,
        EntityKind::Edge,
        EntityKind::Vertex,
    ];
    let elements = [ElementId([0; 16]), ElementId([0xFF; 16]), reference.element];
    for signature in &signatures {
        for &kind in &kinds {
            for &element in &elements {
                let other = Reference {
                    signature: signature.clone(),
                    kind,
                    element,
                    ..reference.clone()
                };
                assert_eq!(resolve(&other, lookup), result, "{other:?}");
            }
        }
    }
    result
}

/// Resolves `reference` against a body of `names`, with `carriers` and `history`, inside the
/// linked version `version`.
fn linked(
    reference: &Reference,
    graph: &NameGraph,
    (names, carriers): (&[NameId], &[(NameId, u32)]),
    history: &[Step],
    version: Option<u64>,
) -> Resolution {
    let names = set(names);
    let carriers = carriers_of(carriers);
    let lookup = Lookup::Found {
        body: Body {
            names: &names,
            carriers: &carriers,
            history,
            graph,
        },
        version,
    };
    checked(reference, &lookup)
}

/// Resolves `reference` against a body of `names`, with `carriers` and `history`.
fn resolve_in(
    reference: &Reference,
    graph: &NameGraph,
    names: &[NameId],
    carriers: &[(NameId, u32)],
    history: &[Step],
) -> Resolution {
    linked(reference, graph, (names, carriers), history, None)
}

fn exact(target: Target) -> Resolution {
    Resolution::Exact {
        target,
        weak: false,
    }
}

fn followed(target: Target) -> Resolution {
    Resolution::FollowedHistory {
        target,
        weak: false,
    }
}

fn unresolved(reason: UnresolvedReason) -> Resolution {
    Resolution::Unresolved { reason }
}

/// Step 1: a name in the body is `Exact`, for each want, whatever the history since holds.
#[test]
fn a_name_in_the_body_is_exact_for_each_want() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let b = face(&mut g, 1, "side");
    let body = [a, b];
    let carriers = [(a, 7)];
    for history in [vec![], vec![pruned(Some(5))]] {
        let r = |want| resolve_in(&reference(a, want), &g, &body, &carriers, &history);
        assert_eq!(r(Want::One), exact(Target::One(a)));
        assert_eq!(r(Want::AllPieces), exact(Target::Pieces(set(&[a]))));
        assert_eq!(r(Want::Carrier), exact(Target::Carrier(CarrierIx(7))));
    }
    // An entity with no carrier, such as a vertex.
    assert_eq!(
        resolve_in(&reference(b, Want::Carrier), &g, &body, &carriers, &[]),
        unresolved(UnresolvedReason::NoCarrier)
    );
}

/// Step 2: a modified name is followed to its image, for each want.
#[test]
fn a_modified_name_is_followed() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let b = face(&mut g, 1, "side");
    let a2 = face(&mut g, 2, "top");
    let history = [recorded(2, vec![modified(a2, &[a]), unchanged(b)], &[])];
    let body = [a2, b];
    let r = |want| resolve_in(&reference(a, want), &g, &body, &[(a2, 3)], &history);
    assert_eq!(r(Want::One), followed(Target::One(a2)));
    assert_eq!(r(Want::AllPieces), followed(Target::Pieces(set(&[a2]))));
    assert_eq!(r(Want::Carrier), followed(Target::Carrier(CarrierIx(3))));
}

/// Step 2: a split name is `Ambiguous` for a reference that wants one, every piece for one that
/// wants them all, and the carrier the pieces share for one that wants the carrier.
#[test]
fn a_split_name_is_ambiguous_for_one_and_followed_for_all_pieces() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let t = face(&mut g, 2, "cut");
    let p = face(&mut g, 2, "back");
    let q = face(&mut g, 2, "front");
    let history = [recorded(
        2,
        vec![split(p, a, t, Side::Back), split(q, a, t, Side::Front)],
        &[],
    )];
    let body = [p, q];
    let r = |want, carriers: &[(NameId, u32)]| {
        resolve_in(&reference(a, want), &g, &body, carriers, &history)
    };
    let pieces = set(&[p, q]);
    assert_eq!(
        r(Want::One, &[]),
        Resolution::Ambiguous {
            candidates: pieces.clone()
        }
    );
    assert_eq!(
        r(Want::AllPieces, &[]),
        followed(Target::Pieces(pieces.clone()))
    );
    assert_eq!(
        r(Want::Carrier, &[(p, 4), (q, 4)]),
        followed(Target::Carrier(CarrierIx(4)))
    );
    assert_eq!(
        r(Want::Carrier, &[(p, 4), (q, 5)]),
        Resolution::Ambiguous { candidates: pieces }
    );
    assert_eq!(
        r(Want::Carrier, &[(p, 4)]),
        unresolved(UnresolvedReason::NoCarrier)
    );
    assert_eq!(
        r(Want::Carrier, &[(q, 5)]),
        unresolved(UnresolvedReason::NoCarrier)
    );
}

/// A piece with no carrier makes a carrier reference `NoCarrier` whichever piece it is, even when
/// the others lie on different carriers: the outcome never depends on the names' order.
#[test]
fn a_piece_with_no_carrier_is_no_carrier_in_any_order() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let t = face(&mut g, 2, "cut");
    let u = face(&mut g, 2, "cut again");
    let pieces = [
        face(&mut g, 2, "back"),
        face(&mut g, 2, "front"),
        face(&mut g, 2, "far front"),
    ];
    let history = [recorded(
        2,
        vec![
            split(pieces[0], a, t, Side::Back),
            split(pieces[1], a, t, Side::Front),
            split(pieces[2], a, u, Side::Front),
        ],
        &[],
    )];
    for missing in 0..3 {
        let carriers: Vec<(NameId, u32)> = (0..3)
            .filter(|&i| i != missing)
            .map(|i| (pieces[i], u32::try_from(i).unwrap()))
            .collect();
        assert_eq!(
            resolve_in(
                &reference(a, Want::Carrier),
                &g,
                &pieces,
                &carriers,
                &history
            ),
            unresolved(UnresolvedReason::NoCarrier),
            "piece {missing} has no carrier"
        );
    }
}

/// Merges are followed: the merged entity is the image of each input.
#[test]
fn a_merged_name_is_followed_to_the_merge() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let b = face(&mut g, 1, "side");
    let m = face(&mut g, 2, "merged");
    let history = [recorded(2, vec![merged(m, &[a, b])], &[])];
    for name in [a, b] {
        assert_eq!(
            resolve_in(&reference(name, Want::One), &g, &[m], &[], &history),
            followed(Target::One(m))
        );
    }
}

/// The walk crosses several steps: a split, then one piece deleted and the other modified, or both
/// merged again.
#[test]
fn the_walk_crosses_several_steps() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let t = face(&mut g, 2, "cut");
    let p = face(&mut g, 2, "back");
    let q = face(&mut g, 2, "front");
    let q2 = face(&mut g, 3, "front");
    let m = face(&mut g, 3, "merged");
    let cut = recorded(
        2,
        vec![split(p, a, t, Side::Back), split(q, a, t, Side::Front)],
        &[],
    );
    let history = [cut.clone(), recorded(3, vec![modified(q2, &[q])], &[p])];
    assert_eq!(
        resolve_in(&reference(a, Want::One), &g, &[q2], &[], &history),
        followed(Target::One(q2))
    );
    let history = [cut.clone(), recorded(3, vec![merged(m, &[p, q])], &[])];
    assert_eq!(
        resolve_in(&reference(a, Want::One), &g, &[m], &[], &history),
        followed(Target::One(m))
    );
    // Steps that pass the pieces through unchanged change nothing.
    let history = [
        cut,
        recorded(3, vec![unchanged(p), unchanged(q)], &[]),
        recorded(4, vec![unchanged(p), unchanged(q)], &[]),
    ];
    assert_eq!(
        resolve_in(&reference(a, Want::AllPieces), &g, &[p, q], &[], &history),
        followed(Target::Pieces(set(&[p, q])))
    );
}

/// A walk left with no descendant is `Deleted`, by the step that removed the last of them.
#[test]
fn a_walk_with_no_descendant_is_deleted_by_the_last_step() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let b = face(&mut g, 1, "side");
    let t = face(&mut g, 2, "cut");
    let p = face(&mut g, 2, "back");
    let q = face(&mut g, 2, "front");
    let history = [
        recorded(
            2,
            vec![
                split(p, a, t, Side::Back),
                split(q, a, t, Side::Front),
                unchanged(b),
            ],
            &[],
        ),
        recorded(3, vec![unchanged(q), unchanged(b)], &[p]),
        recorded(4, vec![unchanged(b)], &[q]),
        recorded(5, vec![unchanged(b)], &[]),
    ];
    for want in [Want::One, Want::AllPieces, Want::Carrier] {
        assert_eq!(
            resolve_in(&reference(a, want), &g, &[b], &[(b, 1)], &history),
            Resolution::Deleted { by: op(4) }
        );
    }
    let history = [recorded(2, vec![unchanged(b)], &[a])];
    assert_eq!(
        resolve_in(&reference(a, Want::One), &g, &[b], &[], &history),
        Resolution::Deleted { by: op(2) }
    );
}

/// Generated outputs are new entities, never the referenced one in a new form: never followed.
#[test]
fn generated_outputs_are_not_followed() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let a2 = face(&mut g, 2, "top");
    let cap = face(&mut g, 2, "cap");
    let history = [recorded(2, vec![generated(cap, &[a])], &[a])];
    for want in [Want::One, Want::AllPieces] {
        assert_eq!(
            resolve_in(&reference(a, want), &g, &[cap], &[], &history),
            Resolution::Deleted { by: op(2) }
        );
    }
    let history = [recorded(
        2,
        vec![modified(a2, &[a]), generated(cap, &[a])],
        &[],
    )];
    assert_eq!(
        resolve_in(&reference(a, Want::One), &g, &[a2, cap], &[], &history),
        followed(Target::One(a2))
    );
}

/// A walk that needs a pruned stretch is `HistoryPruned`, whatever the later steps hold; a name
/// deleted before the stretch stays `Deleted` (24.4).
#[test]
fn pruned_history_is_never_deleted() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let b = face(&mut g, 1, "side");
    let a2 = face(&mut g, 2, "top");
    let r = |history: &[Step]| resolve_in(&reference(a, Want::One), &g, &[b], &[], history);
    assert_eq!(
        r(&[pruned(Some(5)), recorded(6, vec![unchanged(b)], &[a])]),
        Resolution::HistoryPruned { by: Some(op(5)) }
    );
    assert_eq!(
        r(&[recorded(2, vec![unchanged(b)], &[a]), pruned(Some(5))]),
        Resolution::Deleted { by: op(2) }
    );
    assert_eq!(
        r(&[
            recorded(2, vec![modified(a2, &[a]), unchanged(b)], &[]),
            pruned(Some(5)),
            recorded(6, vec![unchanged(b)], &[a2]),
        ]),
        Resolution::HistoryPruned { by: Some(op(5)) }
    );
    // A linked version shared without its history.
    assert_eq!(r(&[pruned(None)]), Resolution::HistoryPruned { by: None });
}

/// Links (20.4): a missing element or link is `Unresolved`, and a reference into another document
/// resolves inside the version supplied, `LinkOlder` when that version is older and does not yet
/// hold the name.
#[test]
fn links_resolve_inside_the_version_supplied() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let a2 = face(&mut g, 2, "top");
    let mut r = reference(a, Want::One);
    assert_eq!(
        checked(&r, &Lookup::ElementGone),
        unresolved(UnresolvedReason::ElementGone)
    );
    assert_eq!(
        checked(&r, &Lookup::LinkMissing),
        unresolved(UnresolvedReason::LinkMissing)
    );
    let history = [recorded(2, vec![modified(a2, &[a])], &[])];
    r.doc = Some(DocRef {
        id: [0xD0; 16],
        version: 5,
    });
    let at =
        |r: &Reference, names: &[NameId], version| linked(r, &g, (names, &[]), &history, version);
    assert_eq!(
        at(&r, &[a2], Some(4)),
        unresolved(UnresolvedReason::LinkOlder)
    );
    assert_eq!(
        at(&r, &[a2], Some(0)),
        unresolved(UnresolvedReason::LinkOlder)
    );
    // An older version that holds the name resolves in it.
    assert_eq!(at(&r, &[a], Some(4)), exact(Target::One(a)));
    // The same version, a newer one, or none said.
    for version in [Some(5), Some(6), Some(u64::MAX), None] {
        assert_eq!(at(&r, &[a2], version), followed(Target::One(a2)));
    }
    // A reference into its own document has no version to compare.
    r.doc = None;
    assert_eq!(at(&r, &[a2], Some(4)), followed(Target::One(a2)));
}

/// Weak names, and names built on them, resolve with `weak` set (20.2, 20.4 step 3).
#[test]
fn weak_names_resolve_with_weak_set() {
    let mut g = NameGraph::new();
    let base = face(&mut g, 1, "slot");
    let w0 = weak(&mut g, base, 0, 2);
    let w1 = weak(&mut g, base, 1, 2);
    let weakly = |target| Resolution::Exact { target, weak: true };
    assert_eq!(
        resolve_in(&reference(w0, Want::One), &g, &[w0, w1], &[], &[]),
        weakly(Target::One(w0))
    );
    // Built on a weak name.
    let fillet = face_from(&mut g, 2, "fillet", &[w0]);
    assert_eq!(
        resolve_in(&reference(fillet, Want::One), &g, &[fillet, w1], &[], &[]),
        weakly(Target::One(fillet))
    );
    // A plain name followed to a result built on a weak one.
    let a = face(&mut g, 1, "top");
    let a2 = face_from(&mut g, 2, "top", &[w1]);
    let history = [recorded(2, vec![modified(a2, &[a]), unchanged(w1)], &[])];
    assert_eq!(
        resolve_in(&reference(a, Want::One), &g, &[a2, w1], &[], &history),
        Resolution::FollowedHistory {
            target: Target::One(a2),
            weak: true
        }
    );
    // A weak name whose history leads on is followed, still weak.
    let e = face(&mut g, 3, "slot");
    let history = [recorded(3, vec![modified(e, &[w0]), unchanged(w1)], &[])];
    assert_eq!(
        resolve_in(&reference(w0, Want::One), &g, &[e, w1], &[], &history),
        Resolution::FollowedHistory {
            target: Target::One(e),
            weak: true
        }
    );
}

/// A weak name left with no live descendant while names that share its base are in the body is
/// `WeakChanged`, with those names; with none, it resolves as any other name would.
#[test]
fn a_weak_name_gone_while_its_base_names_entities_is_weak_changed() {
    let mut g = NameGraph::new();
    let base = face(&mut g, 1, "slot");
    let w0 = weak(&mut g, base, 0, 3);
    let w1 = weak(&mut g, base, 1, 3);
    let w2 = weak(&mut g, base, 2, 3);
    let n0 = weak(&mut g, base, 0, 2);
    let n1 = weak(&mut g, base, 1, 2);
    let other = face(&mut g, 1, "rib");
    let x = weak(&mut g, other, 0, 2);
    let gone = face(&mut g, 4, "gone");
    let deleting = [recorded(
        4,
        vec![modified(n0, &[w0]), modified(n1, &[w1]), unchanged(x)],
        &[w2],
    )];
    let leading = [recorded(4, vec![modified(gone, &[w2])], &[])];
    let r = |body: &[NameId], history: &[Step]| {
        resolve_in(&reference(w2, Want::One), &g, body, &[], history)
    };
    let changed = |names: &[NameId]| Resolution::WeakChanged {
        candidates: set(names),
    };
    // No recorded step holds it; deleted by a step; led to a name the body does not hold.
    assert_eq!(r(&[n0, n1, x], &[]), changed(&[n0, n1]));
    assert_eq!(r(&[n0, n1, x], &deleting), changed(&[n0, n1]));
    assert_eq!(r(&[n0, x], &leading), changed(&[n0]));
    // The base itself is a candidate.
    assert_eq!(r(&[base, x], &[]), changed(&[base]));
    // Nothing shares its base.
    assert_eq!(r(&[x], &[]), unresolved(UnresolvedReason::NameUnknown));
    assert_eq!(r(&[x], &deleting), Resolution::Deleted { by: op(4) });
    assert_eq!(
        r(&[x], &leading),
        unresolved(UnresolvedReason::HistoryBroken)
    );
    // A name built on a weak one has no base of its own.
    let fillet = face_from(&mut g, 2, "fillet", &[w2]);
    assert_eq!(
        resolve_in(&reference(fillet, Want::One), &g, &[n0, n1], &[], &[]),
        unresolved(UnresolvedReason::NameUnknown)
    );
}

/// The history leads to names the body does not hold: `HistoryBroken`, neither preferred.
#[test]
fn a_history_the_body_does_not_match_is_history_broken() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let a2 = face(&mut g, 2, "top");
    let z = face(&mut g, 2, "other");
    for history in [
        [recorded(2, vec![modified(a2, &[a])], &[])],
        // Passed through unchanged, and still not in the body.
        [recorded(2, vec![unchanged(a)], &[])],
    ] {
        assert_eq!(
            resolve_in(&reference(a, Want::One), &g, &[z], &[], &history),
            unresolved(UnresolvedReason::HistoryBroken)
        );
    }
    // Some descendants held and some not.
    let t = face(&mut g, 2, "cut");
    let p = face(&mut g, 2, "back");
    let q = face(&mut g, 2, "front");
    let history = [recorded(
        2,
        vec![split(p, a, t, Side::Back), split(q, a, t, Side::Front)],
        &[],
    )];
    assert_eq!(
        resolve_in(&reference(a, Want::AllPieces), &g, &[p, z], &[], &history),
        unresolved(UnresolvedReason::HistoryBroken)
    );
}

/// A name nothing knows is `NameUnknown`, never guessed.
#[test]
fn unknown_names_are_never_guessed() {
    let mut g = NameGraph::new();
    let a = face(&mut g, 1, "top");
    let b = face(&mut g, 1, "side");
    let t = face(&mut g, 2, "cut");
    let p = face(&mut g, 2, "back");
    let q = face(&mut g, 2, "front");
    let stranger = NameId::from_bytes([0x5A; 16]);
    let r = |name, body: &[NameId], history: &[Step]| {
        resolve_in(&reference(name, Want::One), &g, body, &[], history)
    };
    let unknown = unresolved(UnresolvedReason::NameUnknown);
    // Not in the graph, even when the body holds it.
    assert_eq!(r(stranger, &[a], &[]), unknown);
    assert_eq!(r(stranger, &[stranger], &[]), unknown);
    // In the graph, but not in the body and no step since.
    assert_eq!(r(b, &[a], &[]), unknown);
    // Steps that never hold it.
    assert_eq!(r(b, &[a], &[recorded(2, vec![unchanged(a)], &[])]), unknown);
    // A piece deleted while the other later vanishes with no record: not deleted, unknown.
    let history = [
        recorded(
            2,
            vec![split(p, a, t, Side::Back), split(q, a, t, Side::Front)],
            &[],
        ),
        recorded(3, vec![unchanged(q), unchanged(b)], &[p]),
        recorded(4, vec![unchanged(b)], &[]),
    ];
    assert_eq!(r(a, &[b], &history), unknown);
    // Led to a name the graph does not hold.
    let history = [recorded(2, vec![modified(stranger, &[a])], &[])];
    assert_eq!(r(a, &[stranger], &history), unknown);
}

/// The identifiers of the code of `src`, comments left out, in order.
fn code_words(src: &str) -> Vec<&str> {
    src.lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .flat_map(|code| code.split(|c: char| !(c.is_alphanumeric() || c == '_')))
        .filter(|word| !word.is_empty())
        .collect()
}

/// 20.4 step 5: the resolver's code names a reference's signature, kind and element only where
/// `Reference` declares them, so no expression or pattern reads them.
#[test]
fn the_resolver_never_reads_the_signature_kind_or_element() {
    let words = code_words(include_str!("../src/resolve.rs"));
    for field in ["signature", "kind", "element"] {
        assert_eq!(
            words.iter().filter(|&&w| w == field).count(),
            1,
            "`{field}` is named outside its declaration"
        );
    }
    // The check reads code, not comments: the module documentation names all three.
    assert!(include_str!("../src/resolve.rs").contains("signature, kind or element"));
}

fn config(cases: u32) -> Config {
    Config {
        cases,
        rng_seed: RngSeed::Fixed(0x0000_6789),
        failure_persistence: None,
        ..Config::default()
    }
}

fn inputs(count: u8) -> Vec<NameId> {
    (1..=count).map(|x| NameId::from_bytes([x; 16])).collect()
}

/// A generated chain's names, each interned as a face of its own (so the graph classifies every
/// name, and none is weak).
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

/// The modified images of `name` in `histories` composed (law 4), in name order.
fn images(histories: &[History], name: NameId) -> BTreeSet<NameId> {
    let composed = histories
        .iter()
        .map(History::relations)
        .reduce(|a, b| a.then(&b))
        .unwrap();
    composed
        .modified()
        .filter(|&(from, _)| from == name)
        .map(|(_, to)| to)
        .collect()
}

proptest! {
    #![proptest_config(config(256))]

    /// Every input of a generated chain resolves as the composed history says, for each want: in
    /// the body, `Exact`; with modified images, `FollowedHistory` to them (`Ambiguous` for several
    /// and one wanted); with none, `Deleted` by the first step after which it has none.
    #[test]
    fn every_input_resolves_as_the_composed_history_says(
        steps in (1_u8..7, 1_u8..4).prop_flat_map(|(c, len)| chain(inputs(c), len)),
    ) {
        let mut interned = Interned::default();
        let histories: Vec<History> = steps.iter().map(|(h, _)| interned.history(h)).collect();
        let first: Vec<NameId> = steps[0].1.names().iter().map(|&n| interned.name(n)).collect();
        let body = NameSet::new(histories.last().unwrap().records().iter().map(|r| r.output)).unwrap();
        let carriers: Vec<(NameId, CarrierIx)> = body
            .names()
            .iter()
            .filter(|n| n.to_bytes()[0] % 5 != 0)
            .map(|&n| (n, CarrierIx(u32::from(n.to_bytes()[0] % 3))))
            .collect();
        let history: Vec<Step> = (0_u8..)
            .zip(&histories)
            .map(|(k, h)| Step::Recorded { by: op(k), history: h.clone() })
            .collect();
        let graph = &interned.graph;
        let lookup = Lookup::Found {
            body: Body { names: &body, carriers: &carriers, history: &history, graph },
            version: None,
        };
        for name in first {
            let live = images(&histories, name);
            let get = |want| checked(&reference(name, want), &lookup);
            if live.is_empty() {
                let by = (1..=histories.len())
                    .find(|&k| images(&histories[..k], name).is_empty())
                    .unwrap();
                let deleted = Resolution::Deleted { by: op(u8::try_from(by - 1).unwrap()) };
                for want in [Want::One, Want::AllPieces, Want::Carrier] {
                    prop_assert_eq!(get(want), deleted.clone());
                }
                continue;
            }
            let names: Vec<NameId> = live.iter().copied().collect();
            prop_assert!(names.iter().all(|n| body.names().binary_search(n).is_ok()));
            let wrap = |target| if body.names().binary_search(&name).is_ok() {
                exact(target)
            } else {
                followed(target)
            };
            let candidates = Resolution::Ambiguous { candidates: set(&names) };
            prop_assert_eq!(get(Want::AllPieces), wrap(Target::Pieces(set(&names))));
            prop_assert_eq!(
                get(Want::One),
                if let [one] = names.as_slice() { wrap(Target::One(*one)) } else { candidates.clone() }
            );
            let on: Option<BTreeSet<CarrierIx>> = names
                .iter()
                .map(|n| carriers.iter().find(|(c, _)| c == n).map(|&(_, ix)| ix))
                .collect();
            let carrier = match on {
                None => unresolved(UnresolvedReason::NoCarrier),
                Some(on) if on.len() == 1 => wrap(Target::Carrier(*on.first().unwrap())),
                Some(_) => candidates,
            };
            prop_assert_eq!(get(Want::Carrier), carrier);
        }
    }
}
