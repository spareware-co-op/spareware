//! Attacks on G159's closed forms: every way a line can meet a quadric, each of Goldman and Miller's
//! positions, and the refusals, at small sizes where the geometry can be read off, and at the far
//! corners of the grid, where the budgets are.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

use geom_intersect::IntersectError;
use geom_intersect::closed::{
    Meeting, Natural, Position, between, collapsed, line_cone, line_cylinder, line_sphere,
    line_torus, position,
};
use geom_num::grid::{COORD_MAX, P3, Steps};
use geom_num::quadric::{Cone, Cylinder, Line, Slope, Sphere, Surface, Torus};
use geom_num::sturm::{Point, Roots};

const M: i64 = COORD_MAX;

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn line(a: P3, b: P3) -> Line {
    Line { a, b }
}

fn sphere(c: P3, r: u64) -> Sphere {
    Sphere {
        centre: c,
        radius: Steps(r),
    }
}

fn cylinder(base: P3, along: P3, r: u64) -> Cylinder {
    Cylinder {
        base,
        along,
        radius: Steps(r),
    }
}

fn cone(apex: P3, along: P3, rise: u64, run: u64) -> Cone {
    Cone {
        apex,
        along,
        slope: Slope { rise, run },
    }
}

#[test]
fn a_line_meets_a_sphere() {
    let s = sphere(p(0, 0, 0), 5);
    let at = |y: i64| line(p(0, y, 0), p(1, y, 0));
    assert_eq!(line_sphere(&at(4), &s), Ok(Meeting::Twice));
    assert_eq!(line_sphere(&at(5), &s), Ok(Meeting::Tangent));
    assert_eq!(line_sphere(&at(6), &s), Ok(Meeting::Nowhere));
    let invalid = Err(IntersectError::InvalidInput);
    assert_eq!(line_sphere(&line(p(1, 1, 1), p(1, 1, 1)), &s), invalid);
    assert_eq!(line_sphere(&at(4), &sphere(p(0, 0, 0), 0)), invalid);
}

#[test]
fn a_line_meets_a_cylinder() {
    let c = cylinder(p(0, 0, 0), p(0, 0, 2), 5);
    let at = |y: i64| line(p(0, y, 0), p(1, y, 0));
    assert_eq!(line_cylinder(&at(4), &c), Ok(Meeting::Twice));
    assert_eq!(line_cylinder(&at(5), &c), Ok(Meeting::Tangent));
    assert_eq!(line_cylinder(&at(6), &c), Ok(Meeting::Nowhere));
    // Parallel to the axis: inside, on the surface, outside.
    let up = |x: i64| line(p(x, 0, 0), p(x, 0, 7));
    assert_eq!(line_cylinder(&up(4), &c), Ok(Meeting::Nowhere));
    assert_eq!(line_cylinder(&up(5), &c), Ok(Meeting::Within));
    assert_eq!(line_cylinder(&up(6), &c), Ok(Meeting::Nowhere));
    let invalid = Err(IntersectError::InvalidInput);
    assert_eq!(
        line_cylinder(&at(4), &cylinder(p(1, 1, 1), p(1, 1, 1), 5)),
        invalid
    );
    assert_eq!(
        line_cylinder(&at(4), &cylinder(p(0, 0, 0), p(0, 0, 1), 0)),
        invalid
    );
    assert_eq!(line_cylinder(&line(p(1, 1, 1), p(1, 1, 1)), &c), invalid);
}

/// The double cone x² + y² = z².
#[test]
fn a_line_meets_a_cone() {
    let c = cone(p(0, 0, 0), p(0, 0, 1), 1, 1);
    // Across both sides at a height, touching it, and missing it.
    let high = |y: i64| line(p(0, y, 1), p(1, y, 1));
    assert_eq!(line_cone(&high(0), &c), Ok(Meeting::Twice));
    assert_eq!(line_cone(&high(1), &c), Ok(Meeting::Tangent));
    assert_eq!(line_cone(&high(2), &c), Ok(Meeting::Nowhere));
    // Parallel to a ruling, off the apex: once.
    assert_eq!(
        line_cone(&line(p(1, 0, 0), p(2, 0, 1)), &c),
        Ok(Meeting::Once)
    );
    // Parallel to a ruling, in the tangent plane along it: nowhere.
    assert_eq!(
        line_cone(&line(p(1, 2, 1), p(2, 2, 2)), &c),
        Ok(Meeting::Nowhere)
    );
    // A ruling itself.
    assert_eq!(
        line_cone(&line(p(1, 0, 1), p(2, 0, 2)), &c),
        Ok(Meeting::Within)
    );
    // Through the apex and nothing else, the axis included.
    assert_eq!(
        line_cone(&line(p(0, 0, -1), p(0, 0, 3)), &c),
        Ok(Meeting::Apex)
    );
    assert_eq!(
        line_cone(&line(p(-1, 0, -3), p(1, 0, 3)), &c),
        Ok(Meeting::Apex)
    );
    // Refused: no axis, or a slope with a zero part.
    let invalid = Err(IntersectError::InvalidInput);
    let l = high(0);
    assert_eq!(line_cone(&l, &cone(p(1, 1, 1), p(1, 1, 1), 1, 1)), invalid);
    assert_eq!(line_cone(&l, &cone(p(0, 0, 0), p(0, 0, 1), 0, 1)), invalid);
    assert_eq!(line_cone(&l, &cone(p(0, 0, 0), p(0, 0, 1), 1, 0)), invalid);
    assert_eq!(line_cone(&line(p(1, 1, 1), p(1, 1, 1)), &c), invalid);
}

#[test]
fn a_line_meets_a_torus() {
    let ring = Torus {
        centre: p(0, 0, 0),
        along: p(0, 0, 1),
        major: Steps(10),
        minor: Steps(3),
    };
    let roots = |l: Line| line_torus(&l, &ring);
    assert_eq!(
        roots(line(p(0, 0, 0), p(1, 0, 0))),
        Ok(Roots {
            distinct: 4,
            repeated: false
        })
    );
    assert_eq!(
        roots(line(p(0, 0, 3), p(1, 0, 3))),
        Ok(Roots {
            distinct: 2,
            repeated: true
        })
    );
    assert_eq!(
        roots(line(p(0, 0, 9), p(1, 0, 9))),
        Ok(Roots {
            distinct: 0,
            repeated: false
        })
    );
    let invalid = Err(IntersectError::InvalidInput);
    assert_eq!(roots(line(p(1, 1, 1), p(1, 1, 1))), invalid);
    let thin = Torus {
        minor: Steps(0),
        ..ring
    };
    assert_eq!(line_torus(&line(p(0, 0, 0), p(1, 0, 0)), &thin), invalid);
    let axisless = Torus {
        along: ring.centre,
        ..ring
    };
    assert_eq!(
        line_torus(&line(p(0, 0, 0), p(1, 0, 0)), &axisless),
        invalid
    );
}

#[test]
fn a_stretch_of_a_line_counts_what_it_holds() {
    let s = Surface::Sphere(sphere(p(0, 0, 0), 5));
    let l = line(p(0, 0, 0), p(1, 0, 0));
    let w = Point::whole;
    assert_eq!(between(&l, &s, w(-6), w(6)), Ok(2));
    assert_eq!(between(&l, &s, w(0), w(6)), Ok(1));
    assert_eq!(between(&l, &s, w(0), w(5)), Ok(0));
    assert_eq!(
        between(&l, &s, w(6), w(0)),
        Err(IntersectError::InvalidInput)
    );
}

fn nat_cyl(base: P3, along: P3, r: u64) -> Natural {
    Natural::Cylinder(cylinder(base, along, r))
}

fn nat_cone(apex: P3, along: P3, rise: u64, run: u64) -> Natural {
    Natural::Cone(cone(apex, along, rise, run))
}

#[test]
fn the_special_positions_are_each_found() {
    let (o, z) = (p(0, 0, 0), p(0, 0, 5));
    let ball = |c: P3| Natural::Sphere(sphere(c, 3));
    // Spheres: always one axis. A sphere on an axis, and off it.
    assert_eq!(position(&ball(o), &ball(p(1, 2, 3))), Ok(Position::Coaxial));
    assert_eq!(
        position(&ball(p(0, 0, 9)), &nat_cyl(o, z, 2)),
        Ok(Position::Coaxial)
    );
    assert_eq!(
        position(&nat_cyl(o, z, 2), &ball(p(1, 0, 9))),
        Ok(Position::General)
    );
    assert_eq!(
        position(&ball(p(0, 0, 9)), &nat_cone(o, z, 1, 1)),
        Ok(Position::Coaxial)
    );
    // Cylinders: one axis, parallel, the pipe tee, unequal, skew.
    assert_eq!(
        position(&nat_cyl(o, z, 2), &nat_cyl(p(0, 0, 9), p(0, 0, 3), 7)),
        Ok(Position::Coaxial)
    );
    assert_eq!(
        position(&nat_cyl(o, z, 2), &nat_cyl(p(1, 0, 0), p(1, 0, 3), 2)),
        Ok(Position::ParallelAxes)
    );
    let across = |r: u64| nat_cyl(p(-5, 0, 2), p(5, 0, 2), r);
    assert_eq!(
        position(&nat_cyl(o, z, 2), &across(2)),
        Ok(Position::TangentSphere)
    );
    assert_eq!(
        position(&nat_cyl(o, z, 2), &across(3)),
        Ok(Position::AxesMeet)
    );
    assert_eq!(
        position(&nat_cyl(o, z, 2), &nat_cyl(p(-5, 1, 2), p(5, 1, 2), 2)),
        Ok(Position::General)
    );
    // A cone and a cylinder: slope 3/4 has sine 3/5, so 10 along it the tangent sphere has radius 6.
    let shape = nat_cone(p(0, 0, 10), o, 3, 4);
    let tee = |r: u64| nat_cyl(p(-7, 0, 0), p(9, 0, 0), r);
    assert_eq!(position(&shape, &tee(6)), Ok(Position::TangentSphere));
    assert_eq!(position(&tee(6), &shape), Ok(Position::TangentSphere));
    assert_eq!(position(&shape, &tee(5)), Ok(Position::AxesMeet));
    // Two cones: tangent, not, parallel, one axis, one apex.
    let left = nat_cone(p(0, 0, 20), o, 3, 4);
    let right = |at: i64| nat_cone(p(at, 0, 0), o, 4, 3);
    assert_eq!(position(&left, &right(15)), Ok(Position::TangentSphere));
    assert_eq!(position(&left, &right(14)), Ok(Position::AxesMeet));
    assert_eq!(
        position(&left, &nat_cone(p(1, 0, 20), p(1, 0, 0), 3, 4)),
        Ok(Position::ParallelAxes)
    );
    assert_eq!(
        position(&left, &nat_cone(o, p(0, 0, 3), 1, 1)),
        Ok(Position::Coaxial)
    );
    assert_eq!(
        position(&nat_cone(o, z, 1, 1), &nat_cone(o, p(1, 0, 0), 1, 2)),
        Ok(Position::SharedApex)
    );
    assert_eq!(
        position(
            &nat_cone(o, z, 1, 1),
            &nat_cone(p(9, 1, 0), p(9, 1, 3), 1, 2)
        ),
        Ok(Position::ParallelAxes)
    );
    assert_eq!(
        position(
            &nat_cone(o, z, 1, 1),
            &nat_cone(p(5, 1, 2), p(6, 1, 2), 1, 2)
        ),
        Ok(Position::General)
    );
    // An axis of one point is refused from either side.
    let invalid = Err(IntersectError::InvalidInput);
    assert_eq!(position(&nat_cyl(o, o, 2), &tee(1)), invalid);
    assert_eq!(position(&tee(1), &nat_cone(o, o, 1, 1)), invalid);
    assert_eq!(position(&ball(o), &nat_cyl(o, o, 2)), invalid);
}

#[test]
fn every_decision_holds_at_the_far_corners() {
    let big: u64 = (1 << 41) - 1;
    let far = line(p(-M, -M, -M), p(M, M, M));
    assert!(line_sphere(&far, &sphere(p(M, -M, M), big)).is_ok());
    assert!(line_cylinder(&far, &cylinder(p(M, -M, -M), p(-M, M, M), big)).is_ok());
    assert!(
        line_cone(
            &far,
            &cone(p(M, -M, -M), p(-M, M, M), (1 << 40) - 1, (1 << 40) - 1)
        )
        .is_ok()
    );
    let ring = Torus {
        centre: p(M, M, -M),
        along: p(-M, M, M),
        major: Steps(big),
        minor: Steps(big),
    };
    assert!(line_torus(&far, &ring).is_ok());
    let a = nat_cone(p(-M, -M, M), p(M, M, M), (1 << 40) - 1, 1);
    let b = nat_cone(p(M, -M, -M), p(-M, M, M), 1, (1 << 40) - 1);
    let c = nat_cyl(p(M, -M, M), p(-M, M, M), big);
    assert!(position(&a, &b).is_ok());
    assert!(position(&a, &c).is_ok());
    assert!(position(&b, &c).is_ok());
}

#[test]
fn an_edge_collapses_only_to_one_point() {
    let (a, b) = (p(M, -M, M), p(M, -M, M - 1));
    assert!(collapsed(&[]));
    assert!(collapsed(&[a]));
    assert!(collapsed(&[a, a, a]));
    assert!(!collapsed(&[a, a, b]));
    assert!(!collapsed(&[b, a, a]));
}
