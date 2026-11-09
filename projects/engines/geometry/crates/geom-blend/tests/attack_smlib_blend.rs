//! Attack tests from SMLib's confirmed defects, batch 6: blend feasibility (`geom-blend`, piece
//! G262).
//!
//! Each test feeds the plan the input of one defect in solidcore's log of upstream SMLib defects
//! (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description as the facts G262's plan reads (chains, joins, limits,
//! shared faces), in our own API; no fixture is copied. SMLib finds out that a round cannot be
//! built while building it, in its corners or its merge, and sometimes returns an open body as a
//! success; G262 decides first, from certified facts, so a defect is taken where its cause is a
//! fact the plan reads: rounds that take more of a face than it has, a ball larger than the
//! round it rolls on, a ball as wide as the cylinder whose offset it collapses, an apex where
//! many blended edges meet, and a long tangent loop. The tests also attack the plan's own claims
//! at their edges: a radius that is zero, negative or not a number, a limit whose bound is zero,
//! and a face of negative width. Each test asserts what the plan owes: the feasible radius
//! certified, a refusal that names the size that fits, a cluster that fails as one, chains that
//! partition the edges, and never a size that cannot fit.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::doc_markdown,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::assertions_on_result_states,
    clippy::manual_is_multiple_of,
    reason = "tests: native arithmetic and indexing, prose that names defect files, and a failure stops the test"
)]

use geom_blend::chain::{EdgeId, EdgeRow, Join, Tangency, VertexId, chains};
use geom_blend::cluster::{Cluster, FaceId};
use geom_blend::convexity::{Convexity, corner_class};
use geom_blend::overflow::Policy;
use geom_blend::plan::{ChainFacts, SharedFace, plan};
use geom_blend::radius::{
    Cause, Limit, Verdict, collapse_limit, fold_limit, half_angle_tangent, judge, largest,
    reach_limit,
};
use geom_num::interval::Interval;

/// The tangent of half a right angle, from the normals of two faces of a box.
fn right_angle() -> Interval {
    half_angle_tangent([1, 0, 0], [0, 1, 0]).unwrap()
}

/// WR-460--two-fillet-rounds-on-opposite-edges-of-a-face-wider-than-the-face-build-an-open-body.md:
/// a 14 x 6 x 10 post, its four vertical edges rounded r = 3.03, 3.0625, 3.5 and 4, built as a
/// success a body that is not closed: the two rounds on each 6-wide face took more of it than it
/// has and their rails crossed. r = 2.9 is a closed solid. Here each vertical edge is a chain of
/// one convex right-angled edge, each face shared by the two chains on its edges; the plan finds
/// every cluster of the post infeasible past r = 3, naming each 6-wide face with the factor that
/// fits, and the 14-wide faces only once their rounds pass 7.
#[test]
fn rounds_that_take_more_of_a_face_than_it_has_fail_as_one() {
    let t = right_angle();
    let face = |id: u32, width: f64, a: usize, b: usize| SharedFace {
        face: FaceId(id),
        width: Interval::point(width).unwrap(),
        demands: vec![(a, t), (b, t)],
    };
    // Chains 0 to 3 run round the post; the faces between them are 6, 14, 6 and 14 wide.
    let shared = [
        face(0, 6.0, 0, 1),
        face(1, 14.0, 1, 2),
        face(2, 6.0, 2, 3),
        face(3, 14.0, 3, 0),
    ];
    let facts = vec![
        ChainFacts {
            limits: vec![reach_limit(Interval::point(6.0).unwrap(), t, false).unwrap()],
            sites: vec![],
        };
        4
    ];
    let one = [Cluster {
        chains: vec![0, 1, 2, 3],
    }];
    for r in [2.9, 2.99] {
        let p = plan(&facts, &[r; 4], &one, &shared, &Policy::default()).unwrap();
        assert!(p.clusters[0].feasible, "r {r}: {:?}", p.clusters[0]);
        assert_eq!(p.clusters[0].crowded.len(), 0);
        assert!(p.chains.iter().all(|c| c.verdict == Verdict::Feasible));
    }
    for r in [3.03, 3.0625, 3.5, 4.0] {
        let p = plan(&facts, &[r; 4], &one, &shared, &Policy::default()).unwrap();
        let c = &p.clusters[0];
        assert!(!c.feasible, "r {r}");
        let narrow: Vec<u32> = c.crowded.iter().map(|x| x.face.0).collect();
        assert_eq!(narrow, vec![0, 2], "r {r}");
        for x in &c.crowded {
            // The factor that fits, certified: at or below 6 / 2r, and within a rounding of it.
            let exact = 6.0 / (2.0 * r);
            assert!(
                x.scale <= exact && x.scale >= exact * (1.0 - 1e-12),
                "r {r}: {}",
                x.scale
            );
            assert!(x.scale < 1.0);
        }
        // Each chain alone fits its faces: the failure is the cluster's, as one.
        assert!(
            p.chains.iter().all(|c| c.verdict == Verdict::Feasible),
            "r {r}"
        );
    }
    // Past 7 the 14-wide faces are crowded too, and past 6 each chain is too large alone.
    let p = plan(&facts, &[7.5; 4], &one, &shared, &Policy::default()).unwrap();
    assert_eq!(p.clusters[0].crowded.len(), 4);
    assert!(
        p.chains.iter().all(
            |c| matches!(c.verdict, Verdict::RadiusTooLarge(l) if l.cause == Cause::FaceWidth)
        )
    );
    // Rounds of different sizes on the same face: what one leaves, the other may take.
    let p = plan(
        &facts,
        &[2.0, 3.9, 2.0, 3.9],
        &one,
        &shared,
        &Policy::default(),
    )
    .unwrap();
    assert!(p.clusters[0].feasible, "{:?}", p.clusters[0]);
    let p = plan(
        &facts,
        &[2.0, 4.1, 2.0, 3.9],
        &one,
        &shared,
        &Policy::default(),
    )
    .unwrap();
    assert_eq!(p.clusters[0].crowded.len(), 1);
    assert_eq!(p.clusters[0].crowded[0].face, FaceId(0));
}

/// WR-462--fillet-corner-fails-generically-when-the-rolling-ball-does-not-fit.md: a ball larger
/// than the r3 round it rolls on (r3.1 and up) cannot exist, and SMLib found out only in the
/// corners, failing generically. The plan refuses it at once, naming the fold of the r3 round and
/// the largest radius below 3 that fits; a ball of exactly 3 folds too.
#[test]
fn a_ball_larger_than_the_round_it_rolls_on_is_refused_with_the_size_that_fits() {
    let limits = [fold_limit(Interval::point(3.0).unwrap())];
    let fit = largest(&limits).unwrap();
    assert_eq!(fit.cause, Cause::Fold);
    assert!(fit.open);
    assert_eq!(fit.radius, 3.0_f64.next_down());
    for r in [3.1, 3.5, 5.0, 3.0] {
        match judge(&limits, r) {
            Verdict::RadiusTooLarge(l) => assert_eq!(l, fit, "r {r}"),
            other => panic!("r {r}: {other:?}"),
        }
    }
    assert_eq!(judge(&limits, fit.radius), Verdict::Feasible);
    assert_eq!(judge(&limits, 2.9), Verdict::Feasible);
    // The second round's own r5 neighbour does not hide the binding one.
    let both = [
        fold_limit(Interval::point(5.0).unwrap()),
        fold_limit(Interval::point(3.0).unwrap()),
    ];
    assert_eq!(largest(&both).unwrap(), fit);
}

/// WR-346--inset-round-thinner-than-wall.md and
/// WR-420--no-corner-ball-patch-where-a-rounds-centre-line-collapses-to-a-point.md: a ball as wide
/// as the round it rolls inside collapses that round's offset onto its axis (an r1 or r0.5 round
/// shelled 2 inward; R2 rounds meeting a following R2 round down their seam), and SMLib failed with
/// 1001. The plan names the collapse, with the largest radius below the support's that fits.
#[test]
fn a_ball_as_wide_as_its_support_collapses_its_offset_and_is_refused() {
    for support in [0.5, 1.0, 2.0] {
        let limits = [collapse_limit(Interval::point(support).unwrap())];
        let fit = largest(&limits).unwrap();
        assert_eq!((fit.cause, fit.open), (Cause::CollapsedOffset, true));
        assert_eq!(fit.radius, support.next_down());
        for r in [support, 2.0 * support, 4.0] {
            assert!(
                matches!(judge(&limits, r), Verdict::RadiusTooLarge(l) if l == fit),
                "support {support}, r {r}"
            );
        }
        assert_eq!(judge(&limits, 0.5 * support), Verdict::Feasible);
    }
}

/// UB-209--collecttangentedgesequence-calls-gettangentfilletsolver-on.md: SMLib's tangent walk
/// took an apex where five or more filleted edges meet for a corner it had not made, and called a
/// method of the wrong type there. The chains end at such an apex: every edge in exactly one
/// chain, none passing through it, even with every join there tangent, and its corner counted
/// with its mixed convexity.
#[test]
fn an_apex_of_many_blended_edges_ends_every_chain_there() {
    let apex = VertexId(0);
    for n in 3_u32..=8 {
        let edges: Vec<EdgeRow> = (0..n)
            .map(|i| EdgeRow {
                id: EdgeId(i),
                start: apex,
                end: VertexId(i + 1),
            })
            .collect();
        // Every pair at the apex said to be tangent: still a corner, as more than two meet there.
        let mut joins = Vec::new();
        for a in 0..n {
            for b in a + 1..n {
                joins.push(Join {
                    at: apex,
                    first: EdgeId(a),
                    second: EdgeId(b),
                    tangency: Tangency::Tangent,
                });
            }
        }
        let found = chains(&edges, &joins).unwrap();
        assert_eq!(found.len(), n as usize, "{n}");
        for (i, c) in found.iter().enumerate() {
            assert_eq!(c.links.len(), 1);
            assert_eq!(c.links[0].edge, EdgeId(i as u32));
            assert!(!c.closed);
        }
        let kinds: Vec<Convexity> = (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    Convexity::Convex
                } else {
                    Convexity::Concave
                }
            })
            .collect();
        let corner = corner_class(&kinds);
        assert_eq!(corner.edges, n as usize);
        assert_eq!(corner.convex + corner.concave, n as usize);
        assert!(corner.mixed());
    }
}

/// A fixed pseudo-random sequence, the same on every machine.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

/// HANG-36--retired-2026-10-20-was-collecttangentedgesequence-never.md and UB-209's tangent walk:
/// SMLib's walk along a sequence of tangent edges could run on without end. A loop of a thousand
/// tangent edges, given in a shuffled order, each walked either way round, is one closed chain
/// whose links meet end to end, starting at its lowest edge walked forward; one more edge at a
/// vertex of the loop makes that vertex a corner, and the loop opens there.
#[test]
fn a_long_tangent_loop_closes_and_a_corner_on_it_opens_it() {
    let n = 1000_u32;
    let mut rng = Lcg(0x6262_0001);
    let mut edges: Vec<EdgeRow> = (0..n)
        .map(|i| {
            let (a, b) = (VertexId(i), VertexId((i + 1) % n));
            let (start, end) = if rng.next() % 2 == 0 { (a, b) } else { (b, a) };
            EdgeRow {
                id: EdgeId(i),
                start,
                end,
            }
        })
        .collect();
    let mut joins: Vec<Join> = (0..n)
        .map(|i| Join {
            at: VertexId((i + 1) % n),
            first: EdgeId(i),
            second: EdgeId((i + 1) % n),
            tangency: Tangency::Tangent,
        })
        .collect();
    for i in (1..edges.len()).rev() {
        edges.swap(i, rng.next() as usize % (i + 1));
    }
    for i in (1..joins.len()).rev() {
        joins.swap(i, rng.next() as usize % (i + 1));
    }
    let row = |id: EdgeId, rows: &[EdgeRow]| *rows.iter().find(|r| r.id == id).unwrap();
    let found = chains(&edges, &joins).unwrap();
    assert_eq!(found.len(), 1);
    let c = &found[0];
    assert!(c.closed);
    assert_eq!(c.links.len(), n as usize);
    assert_eq!((c.links[0].edge, c.links[0].reversed), (EdgeId(0), false));
    // Each link ends where the next begins, the last where the first begins.
    let ends = |l: &geom_blend::chain::Link| {
        let r = row(l.edge, &edges);
        if l.reversed {
            (r.end, r.start)
        } else {
            (r.start, r.end)
        }
    };
    for (a, b) in c.links.iter().zip(c.links.iter().cycle().skip(1)) {
        assert_eq!(ends(a).1, ends(b).0);
    }
    // A spur at vertex 500: three edges meet there, so the loop opens at it.
    let mut with_spur = edges.clone();
    with_spur.push(EdgeRow {
        id: EdgeId(n),
        start: VertexId(500),
        end: VertexId(n),
    });
    let found = chains(&with_spur, &joins).unwrap();
    assert_eq!(found.len(), 2);
    let ring = found.iter().find(|c| c.links.len() == n as usize).unwrap();
    assert!(!ring.closed);
    let (first, last) = (ring.links[0], *ring.links.last().unwrap());
    assert_eq!(ends(&first).0, VertexId(500));
    assert_eq!(ends(&last).1, VertexId(500));
}

/// The plan's claims at their edges, beside the defects: a request whose radius is zero,
/// negative, or not a number is not a blend, so no plan judges it feasible, whatever its limits.
#[test]
#[ignore = "defect in G262: judge and plan call a radius that is not positive, or NaN, feasible"]
fn a_radius_that_is_not_positive_is_never_feasible() {
    let t = right_angle();
    let limits = [reach_limit(Interval::point(6.0).unwrap(), t, false).unwrap()];
    for r in [0.0, -0.0, -1.0, -f64::MAX, f64::NAN, f64::NEG_INFINITY] {
        assert_ne!(judge(&limits, r), Verdict::Feasible, "r {r}");
        assert_ne!(judge(&[], r), Verdict::Feasible, "no limits, r {r}");
        let facts = [ChainFacts {
            limits: limits.to_vec(),
            sites: vec![],
        }];
        let clusters = [Cluster { chains: vec![0] }];
        if let Ok(p) = plan(&facts, &[r], &clusters, &[], &Policy::default()) {
            assert!(!p.clusters[0].feasible, "plan, r {r}");
        }
    }
}

/// "A refusal names the size that fits": the largest radius reported is one that fits, so it is
/// never negative. A limit whose bound is zero (a face of no width, a support of no radius), open
/// or closed, leaves no radius that fits, and a face of negative width is not a face; neither may
/// name a negative size.
#[test]
#[ignore = "defect in G262: an open bound at zero, or a face of negative width, names a negative radius"]
fn the_largest_radius_named_is_never_negative() {
    let t = right_angle();
    let zero = Interval::point(0.0).unwrap();
    for limit in [
        collapse_limit(zero),
        fold_limit(zero),
        reach_limit(zero, t, true).unwrap(),
    ] {
        if let Some(fit) = largest(&[limit]) {
            assert!(fit.radius >= 0.0, "{limit:?}: {fit:?}");
        }
    }
    let negative = Interval::new(-2.0, -1.0).unwrap();
    if let Ok(limit) = reach_limit(negative, t, false) {
        let fit = largest(&[limit]).unwrap();
        assert!(fit.radius >= 0.0, "{limit:?}: {fit:?}");
    }
}

/// The half-angle tangent at its claimed edges: the same either way round, the same for normals
/// scaled by any positive factor (as an enclosure that holds the other's), a right angle's holding
/// one, a straight edge's holding zero, opposite normals refused, and normals at the budget's edge
/// accepted while one step past it is refused.
#[test]
fn the_half_angle_tangent_holds_its_value_at_its_edges() {
    let holds = |i: Interval, x: f64| i.lo() <= x && x <= i.hi();
    let t = right_angle();
    assert!(holds(t, 1.0), "{t:?}");
    let big = (1_i128 << 83) - 1;
    for (a, b) in [
        ([1, 0, 0], [0, 1, 0]),
        ([3, 4, 0], [4, -3, 0]),
        ([big, 0, 0], [0, big, 0]),
        ([big, big, 0], [0, big, big]),
        ([1, 2, 3], [-3, 2, 5]),
    ] {
        let (ab, ba) = (
            half_angle_tangent(a, b).unwrap(),
            half_angle_tangent(b, a).unwrap(),
        );
        assert_eq!(ab, ba, "{a:?} {b:?}");
        assert!(ab.lo() >= 0.0);
        // Scaled by 7 where the budget allows, the enclosures still share the true value.
        if a.iter().all(|v| v.abs() < 1 << 80) {
            let scaled = half_angle_tangent(a.map(|v| v * 7), b).unwrap();
            assert!(
                scaled.lo() <= ab.hi() && ab.lo() <= scaled.hi(),
                "{a:?} {b:?}"
            );
        }
    }
    // A right angle at the budget's edge still holds one.
    assert!(holds(
        half_angle_tangent([big, 0, 0], [0, 0, big]).unwrap(),
        1.0
    ));
    // A straight edge: zero, and no reach limit from it.
    let flat = half_angle_tangent([0, 0, 5], [0, 0, 9]).unwrap();
    assert!(holds(flat, 0.0), "{flat:?}");
    assert!(reach_limit(Interval::point(1.0).unwrap(), flat, false).is_err());
    // Opposite normals: a knife edge, refused.
    assert!(half_angle_tangent([0, 0, 5], [0, 0, -9]).is_err());
    // One past the budget.
    assert!(half_angle_tangent([big + 1, 0, 0], [0, 1, 0]).is_err());
    assert!(half_angle_tangent([0, 1, 0], [0, 0, -(big + 1)]).is_err());
}

/// A limit's own causes survive the plan: a chain whose limits are of several causes is refused
/// with the binding one, and a request just below every bound passes.
#[test]
fn the_binding_cause_is_the_one_named() {
    let t = right_angle();
    let limits: Vec<Limit> = vec![
        reach_limit(Interval::point(6.0).unwrap(), t, false).unwrap(),
        fold_limit(Interval::point(4.0).unwrap()),
        collapse_limit(Interval::point(5.0).unwrap()),
    ];
    let fit = largest(&limits).unwrap();
    assert_eq!((fit.cause, fit.radius), (Cause::Fold, 4.0_f64.next_down()));
    assert!(matches!(judge(&limits, 4.5), Verdict::RadiusTooLarge(l) if l == fit));
    assert_eq!(judge(&limits, 3.9), Verdict::Feasible);
}
