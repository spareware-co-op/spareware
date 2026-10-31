//! Attacks on the Bernstein signs of a quadric over a grid Bézier curve (G159): the coefficients
//! against the equation itself at the ends and the middle, the signs against curves whose position
//! is known, and the budget at degree 28 on the grid's corners.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_num::grid::{COORD_MAX, P3, Steps};
use geom_num::quadric::{Cone, Cylinder, Slope, Sphere, Surface, Torus, polar};
use geom_num::wide::I320;
use geom_nurbs::NurbsError;
use geom_nurbs::exact::{quadric_coefficients, signs_of};
use geom_run::{Budget, WorkMeter};

const M: i64 = COORD_MAX;

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn coefficients(points: &[P3], surface: &Surface) -> Vec<I320> {
    quadric_coefficients(points, surface, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap()
}

fn surfaces() -> [Surface; 3] {
    [
        Surface::Sphere(Sphere { centre: p(1, 2, 3), radius: Steps(9) }),
        Surface::Cylinder(Cylinder { base: p(0, 0, 0), along: p(1, 1, 7), radius: Steps(4) }),
        Surface::Cone(Cone {
            apex: p(4, 0, 0),
            along: p(4, 0, 5),
            slope: Slope { rise: 3, run: 4 },
        }),
    ]
}

/// The scaled basis at t = 0 and t = 1 leaves one coefficient, and at t = 1/2 the sum of them over
/// 2^(2p) is the equation at the curve's middle.
#[test]
fn the_coefficients_are_the_equation() {
    // Multiples of four, so the middle (P₀ + 2P₁ + P₂)/4 is a grid point.
    let points = [p(4, 8, -12), p(20, 0, 16), p(-8, 28, 4)];
    let middle = p(9, 9, 6);
    for s in surfaces() {
        let c = coefficients(&points, &s);
        assert_eq!(c.len(), 5);
        assert_eq!(c[0], polar(&s, points[0], points[0]).unwrap());
        assert_eq!(c[4], polar(&s, points[2], points[2]).unwrap());
        let mut sum = I320::ZERO;
        for v in &c {
            sum = sum.checked_add(v).unwrap();
        }
        let scaled = polar(&s, middle, middle)
            .unwrap()
            .checked_mul(&I320::from_i64(16))
            .unwrap();
        assert_eq!(sum, scaled, "{s:?}");
    }
}

#[test]
fn the_signs_say_where_a_curve_can_be() {
    let ball = Surface::Sphere(Sphere { centre: p(0, 0, 0), radius: Steps(5) });
    // All outside, and control points all outside: no crossing.
    let out = signs_of(&coefficients(&[p(9, 0, 0), p(9, 9, 0)], &ball));
    assert!(out.excludes_zero());
    assert_eq!((out.negative, out.zero), (0, 0));
    // All inside.
    let inside = signs_of(&coefficients(&[p(1, 0, 0), p(0, 1, 0)], &ball));
    assert!(inside.excludes_zero());
    assert_eq!((inside.positive, inside.zero), (0, 0));
    // From inside to outside: both signs.
    let across = signs_of(&coefficients(&[p(1, 0, 0), p(9, 0, 0)], &ball));
    assert!(!across.excludes_zero());
    assert!(across.positive > 0 && across.negative > 0);
    // Starting on the surface: a zero coefficient, exactly.
    let touching = signs_of(&coefficients(&[p(5, 0, 0), p(9, 0, 0)], &ball));
    assert_eq!(touching.zero, 1);
    assert!(!touching.excludes_zero());
}

#[test]
fn what_it_cannot_do_is_refused() {
    let ball = surfaces()[0];
    let run = |points: &[P3], s: &Surface| {
        quadric_coefficients(points, s, &mut WorkMeter::new(Budget::UNLIMITED))
    };
    assert_eq!(run(&[], &ball), Err(NurbsError::Defect));
    let torus = Surface::Torus(Torus {
        centre: p(0, 0, 0),
        along: p(0, 0, 1),
        major: Steps(10),
        minor: Steps(3),
    });
    assert_eq!(run(&[p(0, 0, 0), p(1, 0, 0)], &torus), Err(NurbsError::Defect));
    assert_eq!(run(&[p(0, 0, 0); 30], &ball), Err(NurbsError::Defect));
    // The charge comes before the work.
    let mut meter = WorkMeter::new(Budget::new(3, u64::MAX));
    let stopped = quadric_coefficients(&[p(0, 0, 0), p(1, 0, 0)], &ball, &mut meter);
    assert!(matches!(stopped, Err(NurbsError::Stopped(_))));
}

/// Degree 28, the most a curve has, with every control point at a far corner and the largest slope.
#[test]
fn the_budget_holds_at_degree_28() {
    let corners: Vec<P3> = (0..29)
        .map(|i| {
            if i % 2 == 0 { p(M, M, M) } else { p(-M, -M, -M) }
        })
        .collect();
    let big = (1 << 40) - 1;
    let cone = Surface::Cone(Cone {
        apex: p(-M, M, -M),
        along: p(M, -M, M),
        slope: Slope { rise: big, run: big },
    });
    let cylinder = Surface::Cylinder(Cylinder {
        base: p(-M, M, -M),
        along: p(M, -M, M),
        radius: Steps((1 << 41) - 1),
    });
    let sphere = Surface::Sphere(Sphere { centre: p(-M, M, -M), radius: Steps((1 << 41) - 1) });
    for s in [cone, cylinder, sphere] {
        assert_eq!(coefficients(&corners, &s).len(), 57);
    }
}
