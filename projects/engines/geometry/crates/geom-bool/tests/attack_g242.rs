//! Tests that try to break each claim of piece G242 (chapter 11.4): the exact decisions refuse what
//! is out of range, the contact rule touches only strictly inside the radius, apart wins, a merge
//! is proven against carriers and not against its links, and nothing depends on the order things
//! were found in.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::{Bound, CarrierIx, PointGeom, SortedVec};
use geom_bool::decide::{Meeting, plane_cylinder, plane_sphere, point_cone, point_sphere};
use geom_bool::distance::Carriers;
use geom_bool::merge::{default_remaining, merge_cluster, merge_stage};
use geom_bool::rule::{ContactError, EPS_FIT, REFINEMENTS, Verdict, decide, judge};
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_num::quadric::{Cone, Cylinder, Slope, Sphere};
use geom_num::{NumError, P3, Sign, Steps};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn wall(c: &mut Carriers, at: i64) -> CarrierIx {
    CarrierIx::Plane(
        c.add_plane(&Plane::new([1, 0, 0], -i128::from(at)).unwrap())
            .unwrap(),
    )
}

fn certified(at: P3, on: &[CarrierIx], within: u64) -> PointGeom {
    PointGeom::Certified {
        at,
        on: SortedVec::from_items(on.iter().copied()),
        within: Bound(Steps(within)),
    }
}

#[test]
fn the_exact_decisions_refuse_inputs_past_their_ranges_and_never_answer_wrongly() {
    let q = p(0, 0, 0);
    let ball = |r: u64| Sphere {
        centre: q,
        radius: Steps(r),
    };
    // The largest radius in range is decided; one more is refused.
    assert_eq!(
        point_sphere(q, &ball((1 << 41) - 1)).unwrap(),
        Sign::Negative
    );
    assert_eq!(
        point_sphere(q, &ball(1 << 41)),
        Err(NumError::BudgetExceeded)
    );
    let cone = |rise: u64, run: u64| Cone {
        apex: q,
        along: p(0, 0, 1),
        slope: Slope { rise, run },
    };
    assert_eq!(point_cone(p(1, 0, 1), &cone(1, 1)).unwrap(), Sign::Zero);
    assert_eq!(
        point_cone(q, &cone(1 << 40, 1)),
        Err(NumError::BudgetExceeded)
    );
    assert_eq!(point_cone(q, &cone(1, 0)), Err(NumError::Degenerate));
    // A plane through the middle of a sphere that fills the range crosses it; one just past its
    // edge is apart by one step, not tangent by rounding.
    let big = ball((1 << 41) - 1);
    let reach = i128::from((1_i64 << 41) - 1);
    let tangent = Plane::new([0, 0, 1], -reach).unwrap();
    let past = Plane::new([0, 0, 1], -(reach + 1)).unwrap();
    assert_eq!(plane_sphere(&tangent, &big).unwrap(), Meeting::Tangent);
    assert_eq!(plane_sphere(&past, &big).unwrap(), Meeting::Apart);
    // A plane that is nearly parallel to a cylinder axis but not exactly crosses it, however far.
    let axis = Cylinder {
        base: q,
        along: p((1 << 40) - 1, 0, 1),
        radius: Steps(1),
    };
    let nearly = Plane::new([1, 0, -((1 << 40) - 1)], -(1_i128 << 100)).unwrap();
    assert_eq!(plane_cylinder(&nearly, &axis).unwrap(), Meeting::Apart);
    let tilted = Plane::new([1, 0, -((1 << 40) - 2)], -(1_i128 << 100)).unwrap();
    assert_eq!(plane_cylinder(&tilted, &axis).unwrap(), Meeting::Crossing);
}

#[test]
fn touching_needs_the_whole_enclosure_strictly_inside_the_contact_radius() {
    let r = EPS_FIT.0 as f64;
    let iv = |lo: f64, hi: f64| Interval::new(lo, hi).unwrap();
    assert_eq!(judge(iv(0.0, r)), Verdict::Near);
    assert_eq!(judge(iv(0.0, r.next_down())), Verdict::Touch);
    assert_eq!(judge(iv(-r, 0.0)), Verdict::Near);
    assert_eq!(judge(iv(-r.next_down(), 0.0)), Verdict::Touch);
    // Strictly one-sided at the smallest positive number: decided, not touching.
    assert_eq!(
        judge(iv(f64::MIN_POSITIVE, r * 2.0)),
        Verdict::Decided(Sign::Positive)
    );
    // Two points each certified within 32 steps, at the same place, have a separation of [0, 64]:
    // near, never touching; at 31 and 32 they touch.
    let mut c = Carriers::new();
    let w = wall(&mut c, 0);
    let pair = |a: u64, b: u64| {
        merge_stage(
            &c,
            &[
                certified(p(0, 0, 0), &[w], a),
                certified(p(0, 0, 0), &[w], b),
            ],
            &[(0, 1)],
            Bound(Steps(64)),
            &mut meter(),
        )
    };
    assert!(matches!(pair(32, 32), Err(ContactError::NearContact(_))));
    assert!(pair(31, 32).is_ok());
}

#[test]
fn proving_apart_at_a_finer_refinement_beats_touching_at_a_coarser_one() {
    let mut finest = None;
    let verdict = decide(|level| {
        // Coarse enclosures hold zero well inside the radius; the last one does not.
        let e = if level < REFINEMENTS {
            Interval::new(-5.0, 5.0).unwrap()
        } else {
            Interval::new(1.0e-9, 3.0).unwrap()
        };
        finest = Some(level);
        Ok(e)
    })
    .unwrap();
    assert_eq!(verdict, geom_bool::rule::Contact::Apart(Sign::Positive));
    assert_eq!(finest, Some(REFINEMENTS));
}

#[test]
fn a_merge_is_proven_against_the_carriers_and_not_the_links_between_members() {
    let mut c = Carriers::new();
    let walls: Vec<CarrierIx> = (0..5).map(|k| wall(&mut c, 3 * k)).collect();
    // Five points in a row, each touching the next (separation 3, bounds 2 and 2), but the row
    // is 12 steps long: no point is within 3 of all five walls, so the chain fails at bound 3 and
    // is accepted at 6 (the middle point, 6 from the end walls).
    let points: Vec<PointGeom> = (0..5_i64)
        .map(|k| certified(p(3 * k, 0, 0), &[walls[usize::try_from(k).unwrap()]], 2))
        .collect();
    let pairs = [(0, 1), (1, 2), (2, 3), (3, 4)];
    let tight = merge_stage(&c, &points, &pairs, Bound(Steps(3)), &mut meter());
    assert!(
        matches!(tight, Err(ContactError::CoincidenceChain(_))),
        "{tight:?}"
    );
    let loose = merge_stage(&c, &points, &pairs, Bound(Steps(6)), &mut meter()).unwrap();
    assert!(loose.iter().all(|v| *v == loose[0]));
    let PointGeom::Certified { at, within, on } = &loose[0] else {
        panic!("{:?}", loose[0]);
    };
    assert_eq!((*at, within.steps(), on.len()), (p(6, 0, 0), 6, 5));
}

#[test]
fn a_merge_never_depends_on_the_order_of_the_points_or_of_the_pairs() {
    let mut c = Carriers::new();
    let ws: Vec<CarrierIx> = (0..4).map(|k| wall(&mut c, 100 + k)).collect();
    let points: Vec<PointGeom> = (0..4_i64)
        .map(|k| certified(p(100 + k, 0, 0), &[ws[usize::try_from(k).unwrap()]], 1))
        .collect();
    let remaining = Bound(Steps(3));
    let all = [(0, 1), (1, 2), (2, 3), (0, 2), (1, 3), (0, 3)];
    let base = merge_stage(&c, &points, &all, remaining, &mut meter()).unwrap();
    // The pairs reversed.
    let mut reversed = all;
    reversed.reverse();
    assert_eq!(
        merge_stage(&c, &points, &reversed, remaining, &mut meter()).unwrap(),
        base
    );
    // The points reversed: the same representative for the same cluster.
    let mut flipped = points.clone();
    flipped.reverse();
    let flipped_pairs: Vec<(usize, usize)> = all.iter().map(|&(a, b)| (3 - a, 3 - b)).collect();
    let out = merge_stage(&c, &flipped, &flipped_pairs, remaining, &mut meter()).unwrap();
    assert_eq!(out[0], base[0]);
}

#[test]
fn a_cluster_of_one_is_unchanged_and_a_lone_merge_is_refused_without_members() {
    let mut c = Carriers::new();
    let w = wall(&mut c, 0);
    let one = certified(p(5, 5, 5), &[w], 3);
    let kept = merge_cluster(
        &c,
        std::slice::from_ref(&one),
        default_remaining(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(kept, one);
    // An out-of-range pair index is refused, never read past.
    let bad = merge_stage(&c, &[one], &[(0, 1)], default_remaining(), &mut meter());
    assert_eq!(bad, Err(ContactError::Num(NumError::Degenerate)));
}
