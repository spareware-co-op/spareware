//! Exact hulls through the public interface (piece G149), against what 19.5 says of them: every
//! point on or under every face, rings of true corners, the order of the input never showing, and
//! a closed mesh whose volume is exact.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test checks and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_mesh::exact::build::Source;
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{MeshError, MeshKind, Role};
use geom_mesh::hull::{Hull, HullError};
use geom_num::Sign;
use geom_num::grid::{COORD_MAX, P3};
use geom_num::predicate::orient3d;
use geom_num::wide::I256;
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 22))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn source() -> Source {
    Source {
        name: NameId::from_bytes([1; 16]),
        role: Role::Face,
        flags: 0,
    }
}

fn hull(points: &[P3]) -> Hull {
    Hull::of(points, &mut meter()).unwrap()
}

/// Every point is on or under every face, a ring's corners lie on its plane, and no three in a row
/// are on a line.
fn holds(points: &[P3], h: &Hull) {
    let at = |i: u32| h.points()[i as usize];
    for ring in h.faces() {
        assert!(ring.len() >= 3);
        let (a, b, c) = (at(ring[0]), at(ring[1]), at(ring[2]));
        for q in points {
            assert_ne!(orient3d(a, b, c, *q).unwrap(), Sign::Positive);
        }
        let solid = h.is_solid();
        for k in 0..ring.len() {
            let u = at(ring[k]);
            let v = at(ring[(k + 1) % ring.len()]);
            let w = at(ring[(k + 2) % ring.len()]);
            assert_eq!(orient3d(a, b, c, u).unwrap(), Sign::Zero);
            let turn = geom_num::plane::Plane::through(u, v, w);
            assert!(turn.is_ok(), "{u:?} {v:?} {w:?} on a line");
            if solid {
                // The turn runs the way of the whole face.
                let t = turn.unwrap();
                let f = geom_num::plane::Plane::through(a, b, c).unwrap();
                let way = t
                    .n()
                    .iter()
                    .zip(f.n())
                    .all(|(p, q)| p.signum() * q.signum() >= 0);
                assert!(way);
            }
        }
    }
    for corner in h.points() {
        assert!(points.contains(corner));
    }
}

fn cube(s: i64) -> Vec<P3> {
    let mut out = Vec::new();
    for x in [0, s] {
        for y in [0, s] {
            for z in [0, s] {
                out.push(p(x, y, z));
            }
        }
    }
    out
}

#[test]
fn a_cube_is_eight_corners_and_six_squares() {
    let mut points = cube(4);
    // Inside, on faces and on edges.
    points.extend([
        p(2, 2, 2),
        p(1, 3, 2),
        p(2, 2, 0),
        p(2, 2, 4),
        p(0, 0, 2),
        p(4, 2, 4),
        p(2, 4, 0),
    ]);
    let h = hull(&points);
    assert_eq!(h.points().len(), 8);
    assert_eq!(h.faces().len(), 6);
    assert!(h.faces().iter().all(|f| f.len() == 4));
    holds(&points, &h);
    let m = h.mesh(source(), &Exec::serial(), &mut meter()).unwrap();
    assert_eq!(m.kind(), MeshKind::Closed);
    let Volume::Exact(six) = m.volume(&mut meter()).unwrap() else {
        panic!("a mesh on the grid has an exact volume");
    };
    assert_eq!(six, I256::from_i128(6 * 64));
}

#[test]
fn lattices_full_of_ties_give_true_hulls() {
    let mut rng = SplitMix64::new(resolve("geom-mesh hull lattices", Seed(0x149)).unwrap());
    let side = NonZeroU64::new(6).unwrap();
    let count = NonZeroU64::new(50).unwrap();
    let mut solid = 0;
    for _ in 0..300 {
        let n = 4 + rng.below(count) as usize;
        let mut c = || rng.below(side) as i64;
        let points: Vec<P3> = (0..n).map(|_| p(c(), c(), c())).collect();
        let h = hull(&points);
        holds(&points, &h);
        if h.is_solid() {
            solid += 1;
            let m = h.mesh(source(), &Exec::serial(), &mut meter()).unwrap();
            assert_eq!(m.kind(), MeshKind::Closed);
        }
    }
    assert!(solid > 200);
}

#[test]
fn a_shuffle_gives_the_same_hull() {
    let mut rng = SplitMix64::new(resolve("geom-mesh hull shuffle", Seed(0x1490)).unwrap());
    let side = NonZeroU64::new(9).unwrap();
    let mut c = || rng.below(side) as i64;
    let mut points: Vec<P3> = (0..60).map(|_| p(c(), c(), c())).collect();
    let first = hull(&points);
    points.reverse();
    assert_eq!(hull(&points), first);
    let mut doubled = points.clone();
    doubled.extend(points.iter().copied());
    assert_eq!(hull(&doubled), first);
    points.rotate_left(17);
    assert_eq!(hull(&points), first);
}

#[test]
fn a_hull_is_the_same_at_every_scale() {
    let tetrahedron = [p(0, 0, 0), p(3, 0, 0), p(0, 5, 0), p(0, 0, 7), p(1, 1, 1)];
    let scaled: Vec<P3> = tetrahedron
        .iter()
        .map(|q| p(q.x.get() << 36, q.y.get() << 36, q.z.get() << 36))
        .collect();
    let (small, large) = (hull(&tetrahedron), hull(&scaled));
    assert_eq!(small.faces().len(), 4);
    assert_eq!(small.points().len(), 4);
    assert_eq!(large.faces().len(), 4);
    assert_eq!(large.points().len(), 4);
}

#[test]
fn the_edge_of_the_frame_is_nothing_special() {
    let m = COORD_MAX;
    let mut points = Vec::new();
    for x in [-m, m] {
        for y in [-m, m] {
            for z in [-m, m] {
                points.push(p(x, y, z));
            }
        }
    }
    points.push(p(0, 0, 0));
    points.push(p(m, 0, 0));
    let h = hull(&points);
    assert_eq!(h.points().len(), 8);
    assert_eq!(h.faces().len(), 6);
    holds(&points, &h);
}

#[test]
fn flat_sets_are_answers_too() {
    assert!(hull(&[]).points().is_empty());
    let one = hull(&[p(1, 2, 3), p(1, 2, 3)]);
    assert_eq!(one.points(), &[p(1, 2, 3)]);
    assert!(one.faces().is_empty());
    let two = hull(&[p(5, 0, 0), p(0, 0, 0)]);
    assert_eq!(two.points(), &[p(0, 0, 0), p(5, 0, 0)]);
    let line = hull(&[p(2, 2, 2), p(0, 0, 0), p(1, 1, 1), p(3, 3, 3), p(9, 9, 9)]);
    assert_eq!(line.points(), &[p(0, 0, 0), p(9, 9, 9)]);
    assert!(line.faces().is_empty());
    let three = hull(&[p(1, 0, 0), p(0, 0, 0), p(0, 1, 0)]);
    assert_eq!(three.points().len(), 3);
    assert_eq!(three.faces().len(), 1);
    let mut square = Vec::new();
    for x in 0..5 {
        for y in 0..5 {
            square.push(p(x, y, 7));
        }
    }
    let flat = hull(&square);
    assert_eq!(flat.points().len(), 4);
    // A flat hull's ring runs the way of the plane's first non-zero component, up.
    let ring: Vec<P3> = flat.faces()[0]
        .iter()
        .map(|c| flat.points()[*c as usize])
        .collect();
    let (a, b, c) = (ring[0], ring[1], ring[2]);
    assert_eq!(orient3d(a, b, c, p(0, 0, 8)).unwrap(), Sign::Positive);
    assert_eq!(flat.faces().len(), 1);
    assert_eq!(flat.faces()[0].len(), 4);
    assert!(!flat.is_solid());
    assert_eq!(
        flat.mesh(source(), &Exec::serial(), &mut meter())
            .unwrap_err(),
        MeshError::NotClosed
    );
}

#[test]
fn the_meter_stops_a_hull() {
    let mut rng = SplitMix64::new(resolve("geom-mesh hull meter", Seed(0x1491)).unwrap());
    let side = NonZeroU64::new(1 << 20).unwrap();
    let mut c = || rng.below(side) as i64;
    let points: Vec<P3> = (0..500).map(|_| p(c(), c(), c())).collect();
    let mut short = WorkMeter::new(Budget::of_work(20));
    assert!(matches!(
        Hull::of(&points, &mut short),
        Err(HullError::Stopped(_))
    ));
}

#[test]
fn errors_say_what_went_wrong() {
    let texts = [
        HullError::Full.to_string(),
        HullError::Defect.to_string(),
        HullError::Num(geom_num::NumError::Degenerate).to_string(),
    ];
    assert!(texts.iter().all(|t| !t.is_empty()));
}
