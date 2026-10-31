//! Attacks on the plane-based fault catalogue and the second checker (chapter 28.4; piece G135).
//!
//! Every polyhedron of the corpus, with its planes where the grid puts them and moved a step or so,
//! passes both checkers; every injection that applies to it is caught with its kind; and the two
//! checkers, sharing no code, say the same thing of every body, valid or broken.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

use geom_check::FaultKind;
use geom_num::{Exactness, GridStep, Steps};
use geom_test::faults::{Applied, CATALOGUE, attack, attack_with, full_check, missed};
use geom_test::plane_bodies::{CORPUS, Spec, grid, meeting, scaled_jog, still};
use geom_test::second_check::{Finding, check};
use geom_topo::Body;

/// Every body of the corpus: grid corners, meetings at grid places, and meetings moved off them.
fn bodies() -> Vec<(String, Body)> {
    let mut out = Vec::new();
    for spec in CORPUS {
        out.push((format!("{} on the grid", spec.name), grid(spec).unwrap()));
        out.push((
            format!("{} of planes", spec.name),
            meeting(spec, &still).unwrap(),
        ));
        for seed in 0..6 {
            let jog = scaled_jog(0x6135 + seed);
            out.push((
                format!("{} tilted {seed}", spec.name),
                meeting(spec, &jog).unwrap(),
            ));
        }
    }
    out
}

/// The kinds `geom-check` reports of `body`.
fn kinds(body: &Body, exactness: Exactness) -> Vec<FaultKind> {
    let report = full_check(body, exactness).unwrap();
    report.faults().iter().map(|f| f.kind).collect()
}

#[test]
fn the_corpus_passes_both_checkers() {
    for (name, body) in bodies() {
        assert_eq!(kinds(&body, Exactness::Exact), [], "{name}");
        assert_eq!(check(&body, Exactness::Exact), [], "{name}");
    }
}

#[test]
fn every_fault_that_applies_is_caught_with_its_kind() {
    let mut applied = std::collections::BTreeSet::new();
    for (name, body) in bodies() {
        let outcomes = attack(&body, Exactness::Exact).unwrap();
        assert_eq!(
            missed(&outcomes),
            Vec::<&str>::new(),
            "{name}: {outcomes:?}"
        );
        applied.extend(
            outcomes
                .iter()
                .filter(|o| o.applied == Applied::Caught)
                .map(|o| o.name),
        );
    }
    for name in [
        "three planes that do not meet",
        "a vertex given another's planes",
        "a plane of the pool moved",
        "a shadow line far from its vertex",
        "a face's box off its corners",
        "a face turned the wrong way",
    ] {
        assert!(applied.contains(name), "{name} never applied");
    }
}

/// The injections about the geometry and the links both checkers read: the rest break lists only
/// `geom-check` keeps count of.
const SHARED: [&str; 11] = [
    "three planes that do not meet",
    "a vertex given another's planes",
    "a plane of the pool moved",
    "a shadow line far from its vertex",
    "a face's box off its corners",
    "a face turned the wrong way",
    "a flipped fin",
    "an open loop",
    "a vertex off its surface by just more than its bound",
    "a vertex off its curve by just more than its bound",
    "two crossing faces",
];

#[test]
fn the_two_checkers_agree_on_every_broken_body() {
    for (name, body) in bodies() {
        let mut disagreed = Vec::new();
        for injection in CATALOGUE.iter().filter(|i| SHARED.contains(&i.name)) {
            let Some(broken) = (injection.inject)(&body, Exactness::Exact) else {
                continue;
            };
            let first = kinds(&broken.body, broken.exactness);
            let second = check(&broken.body, broken.exactness);
            if first.is_empty() != second.is_empty() {
                disagreed.push((injection.name, first, second));
            }
        }
        assert_eq!(disagreed, [], "{name}");
    }
}

#[test]
fn the_harness_still_catches_a_checker_that_sees_nothing() {
    let body = meeting(CORPUS[0], &still).unwrap();
    let outcomes = attack_with(&body, Exactness::Exact, |_, _| Ok(Vec::new())).unwrap();
    assert!(!missed(&outcomes).is_empty());
}

/// A spec with one corner set moved by `by`, from corner `from` on.
fn moved(spec: Spec<'static>, from: usize, by: [i64; 3]) -> Vec<[i64; 3]> {
    spec.points
        .iter()
        .enumerate()
        .map(|(k, p)| {
            if k < from {
                *p
            } else {
                [p[0] + by[0], p[1] + by[1], p[2] + by[2]]
            }
        })
        .collect()
}

#[test]
fn both_checkers_find_a_lump_through_another() {
    let two = CORPUS[3];
    let points = moved(two, 8, [-250, 25, 25]);
    let spec = Spec {
        points: &points,
        ..two
    };
    let body = meeting(spec, &scaled_jog(1)).unwrap();
    assert!(kinds(&body, Exactness::Exact).contains(&FaultKind::FacesCross));
    assert!(
        check(&body, Exactness::Exact)
            .iter()
            .any(|f| matches!(f, Finding::FacesCross(..)))
    );
}

#[test]
fn both_checkers_find_a_lump_where_it_may_not_be() {
    let two = CORPUS[3];
    let mut points = moved(two, 0, [0; 3]);
    for p in &mut points[..8] {
        *p = [p[0] * 9, p[1] * 9, p[2] * 9];
    }
    for p in &mut points[8..] {
        *p = [p[0] - 200, p[1] + 300, p[2] + 300];
    }
    let spec = Spec {
        points: &points,
        ..two
    };
    let body = meeting(spec, &scaled_jog(2)).unwrap();
    assert_eq!(kinds(&body, Exactness::Exact), [FaultKind::ShellNesting]);
    assert!(
        check(&body, Exactness::Exact)
            .iter()
            .any(|f| matches!(f, Finding::Nesting(_)))
    );
}

#[test]
fn both_checkers_find_a_cavity_outside_its_shell() {
    let cavity = CORPUS[4];
    let points = moved(cavity, 8, [2000, 0, 0]);
    let spec = Spec {
        points: &points,
        ..cavity
    };
    let body = meeting(spec, &scaled_jog(3)).unwrap();
    assert_eq!(kinds(&body, Exactness::Exact), [FaultKind::ShellNesting]);
    assert!(
        check(&body, Exactness::Exact)
            .iter()
            .any(|f| matches!(f, Finding::Nesting(_)))
    );
}

#[test]
fn a_certified_record_is_read_by_both() {
    // A corner two steps off its plane is on it at a bound of a hundred and not at none. Its faces'
    // boxes, which hold a corner exactly in every class, still do not hold it, for both.
    let body = grid(CORPUS[0]).unwrap();
    let exact = (CATALOGUE
        .iter()
        .find(|i| i.name.starts_with("a vertex off its surface")))
    .and_then(|i| (i.inject)(&body, Exactness::Exact))
    .unwrap();
    let off_plane = |found: &[Finding]| found.iter().any(|f| matches!(f, Finding::OffPlane(_)));
    assert!(kinds(&exact.body, Exactness::Exact).contains(&FaultKind::VertexOffSurface));
    assert!(off_plane(&check(&exact.body, Exactness::Exact)));
    let room = Exactness::Certified {
        bound: Steps(100),
        step: GridStep::Q0,
    };
    let (first, second) = (kinds(&exact.body, room), check(&exact.body, room));
    assert!(!first.contains(&FaultKind::VertexOffSurface));
    assert!(first.contains(&FaultKind::FaceBound));
    assert!(!off_plane(&second));
    assert!(second.iter().any(|f| matches!(f, Finding::OutOfBox(_))));
}

#[test]
fn the_second_checker_shares_no_code_with_the_first() {
    let source = include_str!("../src/second_check.rs");
    for name in [
        "use geom_check",
        "geom_check::",
        "side_of_tpi",
        "orient3d",
        "orient2d",
    ] {
        assert!(!source.contains(name), "{name}");
    }
}
