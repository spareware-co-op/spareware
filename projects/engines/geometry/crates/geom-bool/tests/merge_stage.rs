//! Certified distances and the merge stage (G242, chapter 11.4): carriers measure points from their
//! definitions, touching entities become one representative proven within the bound of every
//! carrier of every member, and a chain that cannot be proven fails with `CoincidenceChain`.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::{Bound, CarrierIx, PointGeom, SortedVec};
use geom_bool::distance::{Carriers, separation};
use geom_bool::merge::{default_remaining, merge_stage};
use geom_bool::rule::ContactError;
use geom_curve::Frame;
use geom_num::plane::Plane;
use geom_num::{P3, Steps};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Sphere, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// The plane x = `at`.
fn wall_x(carriers: &mut Carriers, at: i64) -> CarrierIx {
    CarrierIx::Plane(
        carriers
            .add_plane(&Plane::new([1, 0, 0], -i128::from(at)).unwrap())
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
fn a_plane_measures_a_point_exactly_and_a_sphere_by_the_certified_search() {
    let mut c = Carriers::new();
    let wall = wall_x(&mut c, 1000);
    let mut m = meter();
    let d = c.distance(p(1007, 5, -9), wall, 0, &mut m).unwrap();
    assert!(
        d.lo() <= 7.0 && 7.0 <= d.hi() && d.hi() - d.lo() < 1e-9,
        "{d:?}"
    );
    // A plane scaled and turned over is the same table entry.
    let again = c.add_plane(&Plane::new([-3, 0, 0], 3000).unwrap()).unwrap();
    assert_eq!(CarrierIx::Plane(again), wall);
    // On the plane: zero, exactly.
    let on = c.distance(p(1000, 1, 1), wall, 0, &mut m).unwrap();
    assert_eq!((on.lo(), on.hi()), (0.0, 0.0));
    // A sphere of 100 steps about the origin, and a point 30 steps outside it.
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let ball = SurfaceKind::Sphere(
        Sphere::new([0.0, 0.0, 0.0], 100.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
    );
    let ix = c.add_surface(ball).unwrap();
    let far = c
        .distance(p(0, 0, 130), CarrierIx::Surface(ix), 6, &mut m)
        .unwrap();
    assert!(far.lo() <= 30.0 && 30.0 <= far.hi(), "{far:?}");
    assert!(far.hi() - far.lo() < 1.0, "{far:?}");
}

#[test]
fn the_separation_of_points_adds_their_bounds() {
    let mut c = Carriers::new();
    let w = wall_x(&mut c, 0);
    let a = PointGeom::Grid(p(0, 0, 0));
    let b = PointGeom::Grid(p(3, 4, 0));
    let d = separation(&c, &a, &b).unwrap();
    assert!(d.lo() <= 5.0 && 5.0 <= d.hi() && d.hi() - d.lo() < 1e-9);
    let ca = certified(p(0, 0, 0), &[w], 2);
    let cb = certified(p(3, 4, 0), &[w], 1);
    let d = separation(&c, &ca, &cb).unwrap();
    assert!(
        d.lo() <= 2.0 && d.lo() >= 1.99 && d.hi() >= 7.99 && d.hi() <= 8.01,
        "{d:?}"
    );
    let same = separation(&c, &ca, &certified(p(1, 0, 0), &[w], 1)).unwrap();
    assert_eq!(same.lo(), 0.0);
}

#[test]
fn points_proven_apart_stay_apart_however_close() {
    let c = Carriers::new();
    let points = [
        PointGeom::Grid(p(10, 10, 10)),
        PointGeom::Grid(p(11, 10, 10)),
    ];
    let out = merge_stage(&c, &points, &[(0, 1)], default_remaining(), &mut meter()).unwrap();
    assert_eq!(out, points);
}

#[test]
fn touching_certified_points_become_one_proven_against_every_carrier() {
    let mut c = Carriers::new();
    let (x0, x1) = (wall_x(&mut c, 1000), wall_x(&mut c, 1001));
    let y = CarrierIx::Plane(c.add_plane(&Plane::new([0, 1, 0], -1000).unwrap()).unwrap());
    let z = CarrierIx::Plane(c.add_plane(&Plane::new([0, 0, 1], -1000).unwrap()).unwrap());
    let a = certified(p(1000, 1000, 1000), &[x0, y, z], 1);
    let b = certified(p(1001, 1000, 1000), &[x1, y, z], 1);
    let out = merge_stage(&c, &[a, b], &[(0, 1)], default_remaining(), &mut meter()).unwrap();
    assert_eq!(out[0], out[1]);
    let PointGeom::Certified { at, on, within } = &out[0] else {
        panic!("{:?}", out[0]);
    };
    // The lowest of the two candidate grid points that is proven nearest all four carriers: they
    // are 1 step from the farther wall either way, so the lowest point wins.
    assert_eq!(*at, p(1000, 1000, 1000));
    assert_eq!(on.len(), 4);
    assert_eq!(*within, Bound(Steps(1)));
}

#[test]
fn an_exact_member_is_the_representative() {
    let mut c = Carriers::new();
    let x0 = wall_x(&mut c, 1000);
    let x1 = wall_x(&mut c, 1002);
    let exact = PointGeom::Grid(p(1001, 5, 5));
    let near = certified(p(1002, 5, 5), &[x1, x0], 2);
    let out = merge_stage(
        &c,
        &[near, exact],
        &[(0, 1)],
        default_remaining(),
        &mut meter(),
    )
    .unwrap();
    let PointGeom::Certified { at, within, .. } = &out[0] else {
        panic!("{:?}", out[0]);
    };
    assert_eq!(*at, p(1001, 5, 5));
    assert_eq!(*within, Bound(Steps(1)));
    assert_eq!(out[0], out[1]);
}

#[test]
fn a_chain_that_cannot_be_proven_within_the_bound_fails_with_the_coincidence_chain() {
    let mut c = Carriers::new();
    let (x0, x3, x6) = (
        wall_x(&mut c, 1000),
        wall_x(&mut c, 1003),
        wall_x(&mut c, 1006),
    );
    let a = certified(p(1000, 0, 0), &[x0], 2);
    let b = certified(p(1003, 0, 0), &[x3], 2);
    let d = certified(p(1006, 0, 0), &[x6], 2);
    let pairs = [(0, 1), (1, 2)];
    // Each link touches, but no point is within 2 of all three walls.
    let tight = merge_stage(
        &c,
        &[a.clone(), b.clone(), d.clone()],
        &pairs,
        Bound(Steps(2)),
        &mut meter(),
    );
    let Err(ContactError::CoincidenceChain(chain)) = tight else {
        panic!("{tight:?}");
    };
    assert_eq!(chain.remaining, Bound(Steps(2)));
    assert!(chain.distance.hi() >= 3.0);
    // Given the width it needed, the merge is accepted.
    let wide = merge_stage(&c, &[a, b, d], &pairs, Bound(Steps(3)), &mut meter()).unwrap();
    assert_eq!(wide[0], wide[2]);
    let PointGeom::Certified { at, within, .. } = &wide[0] else {
        panic!("{:?}", wide[0]);
    };
    assert_eq!(*at, p(1003, 0, 0));
    assert_eq!(*within, Bound(Steps(3)));
}

#[test]
fn a_pair_neither_decided_nor_touching_is_a_near_contact() {
    let mut c = Carriers::new();
    let w = wall_x(&mut c, 0);
    let a = certified(p(0, 0, 0), &[w], 40);
    let b = certified(p(60, 0, 0), &[w], 40);
    let result = merge_stage(&c, &[a, b], &[(0, 1)], default_remaining(), &mut meter());
    let Err(ContactError::NearContact(near)) = result else {
        panic!("{result:?}");
    };
    assert_eq!(near.enclosure.lo(), 0.0);
    assert!(near.enclosure.hi() >= 140.0);
}

#[test]
fn the_order_of_the_pairs_does_not_change_the_representatives() {
    let mut c = Carriers::new();
    let ws: Vec<CarrierIx> = (0..4).map(|k| wall_x(&mut c, 2000 + k)).collect();
    let points: Vec<PointGeom> = (0..4)
        .map(|k| certified(p(2000 + k, 0, 0), &[ws[usize::try_from(k).unwrap()]], 1))
        .collect();
    let forward = [(0, 1), (1, 2), (2, 3)];
    let backward = [(2, 3), (1, 2), (0, 1)];
    let r = Bound(Steps(2));
    let a = merge_stage(&c, &points, &forward, r, &mut meter()).unwrap();
    let b = merge_stage(&c, &points, &backward, r, &mut meter()).unwrap();
    assert_eq!(a, b);
}
