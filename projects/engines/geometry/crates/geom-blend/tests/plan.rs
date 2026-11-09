//! The blend plan (G262, chapter 13.10, "Feasibility first" steps 3 and 5, and "Overflows"):
//! certified radius bounds on planar edges, the largest feasible radius per chain, overflows
//! resolved by policy, and clusters that succeed or fail as one.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]

use geom_blend::chain::EdgeId;
use geom_blend::cluster::{Cluster, FaceId};
use geom_blend::overflow::{
    Beyond, Found, OverflowError, OverflowKind, Policy, Site, detect, resolve,
};
use geom_blend::plan::{ChainFacts, PlanError, SharedFace, plan};
use geom_blend::radius::{
    Cause, Limit, Verdict, chain_limits, collapse_limit, fold_limit, half_angle_tangent, judge,
    largest, reach_limit, room_limit,
};
use geom_num::NumError;
use geom_num::interval::Interval;

fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

fn near(x: Interval, v: f64) -> bool {
    x.lo() <= v && v <= x.hi() && x.hi() - x.lo() < 1e-9 * v.abs().max(1.0)
}

#[test]
fn the_half_angle_tangent_of_a_right_angle_edge_is_one_and_of_a_flat_one_zero() {
    // A box edge: normals square to each other, tan(45 degrees) = 1.
    let t = half_angle_tangent([0, 0, 1], [0, 1, 0]).unwrap();
    assert!(near(t, 1.0), "{t:?}");
    // Normals 120 degrees apart, tan(60 degrees) = sqrt 3, with large integer scalings.
    let big = 1_i128 << 60;
    let t = half_angle_tangent(
        [big, 0, 0],
        [-big / 2, big * 866_025_403 / 1_000_000_000, 0],
    )
    .unwrap();
    assert!(
        (t.lo() - 3.0_f64.sqrt()).abs() < 1e-6 && t.hi() >= t.lo(),
        "{t:?}"
    );
    // A flat edge: parallel normals, tangent zero.
    let flat = half_angle_tangent([0, 0, 3], [0, 0, 7]).unwrap();
    assert!(flat.lo() <= 0.0 && flat.hi() <= 1e-9, "{flat:?}");
    // A knife edge has no half angle tangent.
    assert_eq!(
        half_angle_tangent([0, 0, 1], [0, 0, -1]),
        Err(NumError::Degenerate)
    );
    assert_eq!(
        half_angle_tangent([1_i128 << 83, 0, 0], [0, 1, 0]),
        Err(NumError::BudgetExceeded)
    );
}

#[test]
fn a_face_width_sets_the_largest_radius_and_a_refusal_names_the_size_that_fits() {
    // A right-angled edge with faces 10 steps wide: r tan 45 degrees = r is at most 10.
    let tan = half_angle_tangent([0, 0, 1], [0, 1, 0]).unwrap();
    let limit = reach_limit(iv(10.0, 10.0), tan, false).unwrap();
    assert!(limit.radius.lo() <= 10.0 && limit.radius.hi() >= 10.0);
    let fit = largest(&[limit]).unwrap();
    assert_eq!(fit.cause, Cause::FaceWidth);
    assert!(fit.radius <= 10.0 && fit.radius > 9.999, "{fit:?}");
    assert_eq!(judge(&[limit], 5.0), Verdict::Feasible);
    assert_eq!(judge(&[limit], fit.radius), Verdict::Feasible);
    match judge(&[limit], 12.0) {
        Verdict::RadiusTooLarge(l) => assert_eq!(l, fit),
        other => panic!("{other:?}"),
    }
    // A radius inside the enclosure of the limit is undecided, never accepted.
    assert_eq!(judge(&[limit], 10.0), Verdict::Undecided(Cause::FaceWidth));
}

#[test]
fn an_open_bound_is_not_feasible_and_the_largest_is_the_number_below_it() {
    let open = Limit {
        cause: Cause::CollapsedOffset,
        radius: iv(4.0, 4.0),
        open: true,
    };
    let fit = largest(&[open]).unwrap();
    assert!(
        fit.open && fit.radius < 4.0 && fit.radius > 3.999_999,
        "{fit:?}"
    );
    assert!(matches!(judge(&[open], 4.0), Verdict::RadiusTooLarge(_)));
    assert_eq!(judge(&[open], fit.radius), Verdict::Feasible);
}

#[test]
fn the_lowest_limit_wins_with_its_cause_and_no_limit_gives_no_answer() {
    let wide = Limit {
        cause: Cause::FaceWidth,
        radius: iv(20.0, 20.5),
        open: false,
    };
    let fold = Limit {
        cause: Cause::Fold,
        radius: iv(6.0, 6.25),
        open: false,
    };
    let room = Limit {
        cause: Cause::NoRoom,
        radius: iv(9.0, 9.0),
        open: false,
    };
    let limits = chain_limits(&[vec![wide, room], vec![fold]]);
    let fit = largest(&limits).unwrap();
    assert_eq!((fit.radius, fit.cause), (6.0, Cause::Fold));
    assert_eq!(judge(&limits, 7.0), Verdict::RadiusTooLarge(fit));
    assert_eq!(largest(&[]), None);
    assert_eq!(judge(&[], 1.0e9), Verdict::Feasible);
}

fn site(reach: Interval, beyond: Beyond) -> Site {
    Site {
        edge: EdgeId(1),
        face: FaceId(4),
        boundary: EdgeId(9),
        reach,
        tan_half: iv(1.0, 1.0),
        beyond,
        internal: false,
    }
}

#[test]
fn overflows_are_found_certain_or_possible_and_never_missed() {
    let sites = [
        site(iv(10.0, 10.0), Beyond::Smooth),
        site(iv(3.0, 3.0), Beyond::Smooth),
        site(iv(7.9, 8.1), Beyond::Smooth),
    ];
    let found = detect(&sites, 8.0).unwrap();
    // Certain at the second, possible at the third, none at the first.
    assert_eq!(found.len(), 2);
    assert!(found[0].certain && found[0].site.reach.hi() == 3.0);
    assert!(!found[1].certain);
}

#[test]
fn the_policy_chooses_the_type_in_its_order_and_a_refusal_names_one_that_would_work() {
    let smooth = Found {
        site: site(iv(1.0, 1.0), Beyond::Smooth),
        certain: true,
    };
    let cliff = Found {
        site: site(iv(1.0, 1.0), Beyond::Sharp { cliff_fits: true }),
        certain: true,
    };
    let cliff_end = Found {
        site: site(iv(1.0, 1.0), Beyond::Sharp { cliff_fits: false }),
        certain: true,
    };
    let corner = Found {
        site: site(iv(1.0, 1.0), Beyond::Corner),
        certain: true,
    };
    let kinds = |found: &[Found], policy: &Policy| -> Vec<OverflowKind> {
        resolve(found, policy)
            .unwrap()
            .iter()
            .map(|r| r.kind)
            .collect()
    };
    let default = Policy::default();
    assert_eq!(
        kinds(&[smooth, cliff, cliff_end, corner], &default),
        [
            OverflowKind::Smooth,
            OverflowKind::Cliff,
            OverflowKind::CliffEnd,
            OverflowKind::Notch
        ]
    );
    // A policy that allows only a notch takes it where a cliff would also do.
    let notch_only = Policy {
        order: vec![OverflowKind::Notch],
    };
    assert_eq!(kinds(&[cliff], &notch_only), [OverflowKind::Notch]);
    // Where nothing allowed works, the refusal names the type that would.
    assert_eq!(
        resolve(&[smooth], &notch_only),
        Err(OverflowError::OverflowNotAllowed {
            boundary: EdgeId(9),
            would_work: OverflowKind::Smooth
        })
    );
    let none = Policy { order: vec![] };
    assert_eq!(
        resolve(&[corner], &none),
        Err(OverflowError::OverflowNotAllowed {
            boundary: EdgeId(9),
            would_work: OverflowKind::Notch
        })
    );
}

fn facts(width: f64) -> ChainFacts {
    let tan = half_angle_tangent([0, 0, 1], [0, 1, 0]).unwrap();
    ChainFacts {
        limits: vec![reach_limit(iv(width, width), tan, false).unwrap()],
        sites: vec![],
    }
}

#[test]
fn a_cluster_succeeds_or_fails_as_one_and_each_chain_reports_what_fits() {
    let chains = [facts(10.0), facts(4.0), facts(10.0)];
    let clusters = [Cluster { chains: vec![0] }, Cluster { chains: vec![1, 2] }];
    // Radii 5, 5, 5: chain 1 is too narrow (4), so its cluster fails and chain 0's succeeds.
    let result = plan(
        &chains,
        &[5.0, 5.0, 5.0],
        &clusters,
        &[],
        &Policy::default(),
    )
    .unwrap();
    assert_eq!(result.chains[0].verdict, Verdict::Feasible);
    assert!(matches!(
        result.chains[1].verdict,
        Verdict::RadiusTooLarge(l) if l.radius <= 4.0 && l.radius > 3.999
    ));
    assert_eq!(result.chains[2].verdict, Verdict::Feasible);
    assert!(result.clusters[0].feasible);
    assert!(!result.clusters[1].feasible);
    // The largest per chain is the preview.
    assert!(result.chains[0].largest.unwrap().radius <= 10.0);
    assert!(result.chains[1].largest.unwrap().radius <= 4.0);
}

#[test]
fn chains_that_crowd_one_narrow_face_fail_together_with_the_factor_that_fits() {
    let chains = [facts(100.0), facts(100.0)];
    let clusters = [Cluster { chains: vec![0, 1] }];
    let one = iv(1.0, 1.0);
    let shared = [SharedFace {
        face: FaceId(7),
        width: iv(8.0, 8.0),
        demands: vec![(0, one), (1, one)],
    }];
    // Each asks 5 of a face 8 wide.
    let result = plan(&chains, &[5.0, 5.0], &clusters, &shared, &Policy::default()).unwrap();
    assert!(!result.clusters[0].feasible);
    let crowded = result.clusters[0].crowded[0];
    assert_eq!(crowded.face, FaceId(7));
    assert!((crowded.scale - 0.8).abs() < 1e-9, "{crowded:?}");
    // At 3.9 each, 7.8 of 8 fits. Exactly 8 of 8 is not certified: the sum's enclosure reaches past
    // the width by a rounding, and a touching fit is not a proven one.
    let fits = plan(&chains, &[3.9, 3.9], &clusters, &shared, &Policy::default()).unwrap();
    assert!(fits.clusters[0].feasible);
    let touching = plan(&chains, &[4.0, 4.0], &clusters, &shared, &Policy::default()).unwrap();
    assert!(!touching.clusters[0].feasible);
}

#[test]
fn an_overflow_with_no_allowed_type_is_a_plan_error_and_unknown_chains_are_refused() {
    let mut f = facts(100.0);
    f.sites.push(site(iv(1.0, 1.0), Beyond::Smooth));
    let notch_only = Policy {
        order: vec![OverflowKind::Notch],
    };
    let result = plan(
        &[f.clone()],
        &[5.0],
        &[Cluster { chains: vec![0] }],
        &[],
        &notch_only,
    );
    assert!(matches!(
        result,
        Err(PlanError::Overflow(
            OverflowError::OverflowNotAllowed { .. }
        ))
    ));
    // With the default policy the overflow is resolved and the plan is made.
    let ok = plan(
        &[f],
        &[5.0],
        &[Cluster { chains: vec![0] }],
        &[],
        &Policy::default(),
    )
    .unwrap();
    assert_eq!(ok.chains[0].overflows.len(), 1);
    assert_eq!(ok.chains[0].overflows[0].kind, OverflowKind::Smooth);
    let unknown = plan(
        &[facts(1.0)],
        &[1.0],
        &[Cluster { chains: vec![3] }],
        &[],
        &Policy::default(),
    );
    assert_eq!(unknown, Err(PlanError::UnknownChain(3)));
    assert_eq!(
        plan(&[facts(1.0)], &[], &[], &[], &Policy::default()),
        Err(PlanError::UnknownChain(0))
    );
}

#[test]
fn curvature_room_and_collapse_set_their_own_limits() {
    // A concave fillet cannot be larger than the supports' smallest radius of curvature, and a
    // radius at it folds the face: open.
    let fold = fold_limit(iv(5.0, 5.0));
    assert_eq!(fold.cause, Cause::Fold);
    assert!(fold.open);
    // A slot 9 wide takes a ball up to 4.5.
    let room = room_limit(iv(9.0, 9.0)).unwrap();
    assert_eq!(room.cause, Cause::NoRoom);
    assert!(room.radius.lo() <= 4.5 && room.radius.hi() >= 4.5);
    assert_eq!(room_limit(iv(0.0, 1.0)), Err(NumError::Degenerate));
    // A cylinder of 7 collapses a ball of 7 rolling inside it.
    let collapse = collapse_limit(iv(7.0, 7.0));
    assert!(collapse.open);
    // Together: the lowest wins, and the open ones report the number below their bound.
    let limits = [fold, room, collapse];
    let fit = largest(&limits).unwrap();
    assert_eq!(fit.cause, Cause::NoRoom);
    assert!(fit.radius <= 4.5 && !fit.open);
    assert_eq!(judge(&limits, 4.0), Verdict::Feasible);
    assert!(matches!(judge(&limits, 6.0), Verdict::RadiusTooLarge(_)));
}
