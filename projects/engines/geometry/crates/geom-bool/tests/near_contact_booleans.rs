//! Fifty successive near-contact booleans, then a certified check of every vertex against all its
//! carriers (G242, chapter 11.4), and the broad phase against brute force.
//!
//! Each round a tool brings three planes within a step or two of planes already in the model, so
//! the vertex they make touches the one already there. The merge stage joins them, and the
//! representative is proven against the carriers of every round so far, measured from the carriers
//! own definitions and never from the previous representative: after fifty rounds it has not walked
//! and its bound has not grown.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::broad::{candidate_pairs, point_entries, point_pairs};
use geom_bool::carrier::{Bound, CarrierIx, PointGeom, SortedVec};
use geom_bool::distance::{Carriers, separation};
use geom_bool::merge::{default_remaining, merge_stage};
use geom_num::plane::Plane;
use geom_num::{P3, Steps};
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 28))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// The three planes x = ax, y = ay, z = az, added to the carriers, as a vertex of their meeting
/// computed to the grid: certified within the one step that committing it to the grid costs. A
/// vertex that came from curved carriers is certified so; two exact points one step apart would be
/// proven apart and never merge.
fn vertex(c: &mut Carriers, (ax, ay, az): (i64, i64, i64)) -> PointGeom {
    let mut add = |n: [i128; 3], d: i64| {
        CarrierIx::Plane(
            c.add_plane(&Plane::new(n, -i128::from(d)).unwrap())
                .unwrap(),
        )
    };
    let on = [add([1, 0, 0], ax), add([0, 1, 0], ay), add([0, 0, 1], az)];
    PointGeom::Certified {
        at: p(ax, ay, az),
        on: SortedVec::from_items(on),
        within: Bound(Steps(1)),
    }
}

/// Every carrier of a point, checked from the carriers own definitions: each is within the point
/// bound, and the bound is within the contact radius.
fn certify(c: &Carriers, point: &PointGeom) -> usize {
    let PointGeom::Certified { at, on, within } = point else {
        panic!("not certified: {point:?}");
    };
    assert!(within.steps() < 64, "{within:?}");
    let mut m = meter();
    for &carrier in on.as_slice() {
        let (bound, d) = c.upper_bound(*at, carrier, 8, &mut m).unwrap();
        assert!(
            bound <= *within,
            "{carrier:?}: {bound:?} against {within:?}, {d:?}"
        );
        assert!(d.hi() <= within.steps() as f64 + 1e-6, "{carrier:?} {d:?}");
    }
    on.len()
}

fn bounds_of(model: &[PointGeom]) -> Vec<Bound> {
    model
        .iter()
        .map(|v| match v {
            PointGeom::Certified { within, .. } => *within,
            PointGeom::Grid(_) | PointGeom::Planes(_) => Bound::ZERO,
        })
        .collect()
}

#[test]
fn fifty_successive_near_contact_booleans_leave_every_vertex_proven_against_all_its_carriers() {
    let mut c = Carriers::new();
    let exec = Exec::serial();
    // Two sites far apart: their vertices never merge with each other.
    let sites = [(1000_i64, 1000_i64, 1000_i64), (5000, -4000, 2500)];
    let mut model: Vec<PointGeom> = sites
        .iter()
        .map(|&(x, y, z)| vertex(&mut c, (x, y, z)))
        .collect();
    let mut at_round_three: Option<Vec<Bound>> = None;
    for round in 1..=50_i64 {
        // The tool planes sit one step either side of the site planes.
        for &(x, y, z) in &sites {
            let wobble = |k: i64| (round + k) % 3 - 1;
            let v = vertex(&mut c, (x + wobble(0), y + wobble(1), z + wobble(2)));
            model.push(v);
        }
        let pairs = point_pairs(&c, &model, &exec, &mut meter()).unwrap();
        let merged = merge_stage(&c, &model, &pairs, default_remaining(), &mut meter()).unwrap();
        // One vertex per site, whatever the round: the others are the same entity now.
        let mut distinct: Vec<PointGeom> = Vec::new();
        for v in merged {
            if !distinct.contains(&v) {
                distinct.push(v);
            }
        }
        assert_eq!(distinct.len(), sites.len(), "round {round}: {distinct:?}");
        model = distinct;
        if round == 3 {
            at_round_three = Some(bounds_of(&model));
        }
    }
    // After fifty rounds each site has one vertex holding every plane any round brought, proven
    // within the contact radius of all of them, and it has not walked from where the first was.
    let mut total = 0;
    for (v, &(x, y, z)) in model.iter().zip(&sites) {
        total += certify(&c, v);
        let PointGeom::Certified { at, .. } = v else {
            panic!("{v:?}");
        };
        let from_site =
            separation(&c, &PointGeom::Grid(*at), &PointGeom::Grid(p(x, y, z))).unwrap();
        assert!(from_site.hi() <= 2.0, "walked to {at:?}: {from_site:?}");
    }
    // The plane table keeps one entry for a plane given twice, so the carriers are few, and more
    // than the three each site started with.
    assert!(total > 6 && total <= 2 * 3 * 51, "{total}");
    // The bound has not grown with the rounds: bounds add along each entity own history only.
    assert_eq!(Some(bounds_of(&model)), at_round_three);
}

/// A fixed pseudo-random sequence, so the test is the same on every machine.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> i64 {
        (self.next() % n) as i64
    }
}

#[test]
fn the_broad_phase_finds_every_pair_the_contact_rule_could_join_and_agrees_with_brute_force() {
    let c = Carriers::new();
    let exec = Exec::serial();
    let mut rng = Lcg(0x5eed_0242);
    // Points in a box 400 steps wide: many within the contact radius of another, most not.
    let points: Vec<PointGeom> = (0..300)
        .map(|_| {
            let at = p(
                rng.below(400) - 200,
                rng.below(400) - 200,
                rng.below(400) - 200,
            );
            PointGeom::Grid(at)
        })
        .collect();
    let entries = point_entries(&c, &points).unwrap();
    let found = candidate_pairs(&entries, &exec, &mut meter()).unwrap();
    // Sorted, no repeats, smaller key first.
    assert!(found.windows(2).all(|w| w[0] < w[1]));
    assert!(found.iter().all(|(a, b)| a < b));
    // Every pair closer than the contact radius is there.
    let mut brute = 0;
    for i in 0..points.len() {
        for j in (i + 1)..points.len() {
            let d = separation(&c, &points[i], &points[j]).unwrap();
            if d.hi() < 64.0 {
                brute += 1;
                assert!(
                    found.contains(&(i as u64, j as u64)),
                    "missed {i} {j} at {d:?}"
                );
            }
        }
    }
    assert!(brute > 20, "{brute}");
    // The same set whatever order the entries come in.
    let mut reversed = entries.clone();
    reversed.reverse();
    assert_eq!(
        candidate_pairs(&reversed, &exec, &mut meter()).unwrap(),
        found
    );
}

#[test]
fn points_far_apart_have_no_candidate_pairs() {
    let c = Carriers::new();
    let points: Vec<PointGeom> = (0..200)
        .map(|k| PointGeom::Grid(p(k * 1000, -k * 700, k * 300)))
        .collect();
    let pairs = point_pairs(&c, &points, &Exec::serial(), &mut meter()).unwrap();
    assert!(pairs.is_empty(), "{pairs:?}");
    // And merging nothing returns every point as it was.
    let out = merge_stage(&c, &points, &pairs, Bound(Steps(64)), &mut meter()).unwrap();
    assert_eq!(out, points);
}

#[test]
fn a_plane_meeting_commits_to_its_grid_point_and_exact_points_one_step_apart_stay_apart() {
    let mut c = Carriers::new();
    let mut add = |n: [i128; 3], d: i64| {
        c.add_plane(&Plane::new(n, -i128::from(d)).unwrap())
            .unwrap()
    };
    let meeting = PointGeom::Planes([add([1, 0, 0], 3), add([0, 1, 0], 4), add([0, 0, 1], 5)]);
    assert!(meeting.is_exact());
    assert_eq!(meeting.carriers().len(), 3);
    assert_eq!(c.commit(&meeting).unwrap(), (p(3, 4, 5), Bound::ZERO));
    let next = PointGeom::Grid(p(4, 4, 5));
    let out = merge_stage(
        &c,
        &[meeting.clone(), next.clone()],
        &[(0, 1)],
        default_remaining(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(out, [meeting, next]);
}
