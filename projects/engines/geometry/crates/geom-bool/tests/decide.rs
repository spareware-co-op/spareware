//! The exact analytic decision table (G242, chapter 11.4): each decision on known points, then on
//! the corners of the coordinate range, against the same formulas in big integers. The corners
//! are where the bit budgets of `geom_bool::decide` are tight.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::decide::{
    Meeting, cylinders_coincide, plane_cylinder, plane_sphere, planes_coincide, point_cone,
    point_cylinder, point_sphere, point_torus, spheres_coincide,
};
use geom_num::grid::COORD_MAX;
use geom_num::plane::Plane;
use geom_num::quadric::{Cone, Cylinder, Slope, Sphere, Torus};
use geom_num::{NumError, P3, Sign, Steps};
use num_bigint::BigInt;

const M: i64 = COORD_MAX;

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn big(v: i64) -> BigInt {
    BigInt::from(v)
}

fn sign_of(v: &BigInt) -> Sign {
    match v.sign() {
        num_bigint::Sign::Plus => Sign::Positive,
        num_bigint::Sign::Minus => Sign::Negative,
        num_bigint::Sign::NoSign => Sign::Zero,
    }
}

type V = [BigInt; 3];

fn vec(a: P3, b: P3) -> V {
    [
        big(a.x.get()) - big(b.x.get()),
        big(a.y.get()) - big(b.y.get()),
        big(a.z.get()) - big(b.z.get()),
    ]
}

fn dot(a: &V, b: &V) -> BigInt {
    &a[0] * &b[0] + &a[1] * &b[1] + &a[2] * &b[2]
}

fn cross(a: &V, b: &V) -> V {
    [
        &a[1] * &b[2] - &a[2] * &b[1],
        &a[2] * &b[0] - &a[0] * &b[2],
        &a[0] * &b[1] - &a[1] * &b[0],
    ]
}

/// The coordinates the extreme-value tests use: both ends of the range, one step in from each, and
/// the middle.
const EDGES: [i64; 5] = [-M, -M + 1, 0, M - 1, M];

fn corners() -> Vec<P3> {
    let mut out = Vec::new();
    for x in EDGES {
        for y in EDGES {
            for z in EDGES {
                out.push(p(x, y, z));
            }
        }
    }
    out
}

fn radii() -> [Steps; 4] {
    [Steps(0), Steps(1), Steps((1 << 41) - 1), Steps(1 << 40)]
}

#[test]
fn a_point_against_a_sphere_on_its_three_sides() {
    let s = Sphere {
        centre: p(1, 2, 3),
        radius: Steps(5),
    };
    assert_eq!(point_sphere(p(1, 2, 8), &s).unwrap(), Sign::Zero);
    assert_eq!(point_sphere(p(1, 2, 9), &s).unwrap(), Sign::Positive);
    assert_eq!(point_sphere(p(1, 2, 7), &s).unwrap(), Sign::Negative);
    // A 3-4-5 point.
    assert_eq!(point_sphere(p(4, 6, 3), &s).unwrap(), Sign::Zero);
}

#[test]
fn a_point_against_a_cylinder_on_its_three_sides() {
    let c = Cylinder {
        base: p(0, 0, 0),
        along: p(0, 0, 7),
        radius: Steps(5),
    };
    assert_eq!(point_cylinder(p(3, 4, 100), &c).unwrap(), Sign::Zero);
    assert_eq!(point_cylinder(p(3, 5, -9), &c).unwrap(), Sign::Positive);
    assert_eq!(point_cylinder(p(3, 3, 1), &c).unwrap(), Sign::Negative);
    // The same cylinder given by another pair of axis points.
    let d = Cylinder {
        base: p(0, 0, 20),
        along: p(0, 0, 30),
        radius: Steps(5),
    };
    assert_eq!(point_cylinder(p(-4, 3, 7), &d).unwrap(), Sign::Zero);
}

#[test]
fn a_point_against_a_cone_on_its_three_sides() {
    // Slope 3/4: the radius at height z is 3z/4.
    let c = Cone {
        apex: p(0, 0, 0),
        along: p(0, 0, 1),
        slope: Slope { rise: 3, run: 4 },
    };
    assert_eq!(point_cone(p(3, 0, 4), &c).unwrap(), Sign::Zero);
    assert_eq!(point_cone(p(0, 3, -4), &c).unwrap(), Sign::Zero);
    assert_eq!(point_cone(p(4, 0, 4), &c).unwrap(), Sign::Positive);
    assert_eq!(point_cone(p(2, 0, 4), &c).unwrap(), Sign::Negative);
    assert_eq!(point_cone(p(0, 0, 0), &c).unwrap(), Sign::Zero);
}

#[test]
fn a_point_against_a_torus_on_its_three_sides() {
    // Major radius 5, minor 3 about the z axis.
    let t = Torus {
        centre: p(0, 0, 0),
        along: p(0, 0, 2),
        major: Steps(5),
        minor: Steps(3),
    };
    assert_eq!(point_torus(p(8, 0, 0), &t).unwrap(), Sign::Zero);
    assert_eq!(point_torus(p(2, 0, 0), &t).unwrap(), Sign::Zero);
    assert_eq!(point_torus(p(5, 0, 3), &t).unwrap(), Sign::Zero);
    assert_eq!(point_torus(p(9, 0, 0), &t).unwrap(), Sign::Positive);
    assert_eq!(point_torus(p(5, 0, 0), &t).unwrap(), Sign::Negative);
    assert_eq!(point_torus(p(0, 0, 0), &t).unwrap(), Sign::Positive);
}

#[test]
fn a_plane_against_a_sphere_apart_tangent_or_crossing() {
    let s = Sphere {
        centre: p(0, 0, 0),
        radius: Steps(5),
    };
    let plane = |z: i64| Plane::new([0, 0, 1], -i128::from(z)).unwrap();
    assert_eq!(plane_sphere(&plane(6), &s).unwrap(), Meeting::Apart);
    assert_eq!(plane_sphere(&plane(5), &s).unwrap(), Meeting::Tangent);
    assert_eq!(plane_sphere(&plane(-5), &s).unwrap(), Meeting::Tangent);
    assert_eq!(plane_sphere(&plane(4), &s).unwrap(), Meeting::Crossing);
    // A tilted plane: the normal (3, 4, 0) and distance 25 / 5 = 5 from the origin.
    let tilted = Plane::new([3, 4, 0], -25).unwrap();
    assert_eq!(plane_sphere(&tilted, &s).unwrap(), Meeting::Tangent);
}

#[test]
fn a_plane_against_a_cylinder() {
    let c = Cylinder {
        base: p(0, 0, 0),
        along: p(0, 0, 1),
        radius: Steps(5),
    };
    // Parallel to the axis.
    let wall = |x: i64| Plane::new([1, 0, 0], -i128::from(x)).unwrap();
    assert_eq!(plane_cylinder(&wall(6), &c).unwrap(), Meeting::Apart);
    assert_eq!(plane_cylinder(&wall(5), &c).unwrap(), Meeting::Tangent);
    assert_eq!(plane_cylinder(&wall(2), &c).unwrap(), Meeting::Crossing);
    // Not parallel to the axis: always an ellipse, however far.
    let floor = Plane::new([0, 0, 1], -1_000_000).unwrap();
    assert_eq!(plane_cylinder(&floor, &c).unwrap(), Meeting::Crossing);
    let slanted = Plane::new([1, 0, 1], -1_000_000).unwrap();
    assert_eq!(plane_cylinder(&slanted, &c).unwrap(), Meeting::Crossing);
}

#[test]
fn coincidence_of_planes_spheres_and_cylinders() {
    let a = Plane::new([1, 2, 3], -7).unwrap();
    let scaled = Plane::new([-2, -4, -6], 14).unwrap();
    let other = Plane::new([2, 4, 6], -15).unwrap();
    let turned = Plane::new([1, 2, 4], -7).unwrap();
    assert!(planes_coincide(&a, &a).unwrap());
    assert!(planes_coincide(&a, &scaled).unwrap());
    assert!(!planes_coincide(&a, &other).unwrap());
    assert!(!planes_coincide(&a, &turned).unwrap());
    let s = Sphere {
        centre: p(1, 1, 1),
        radius: Steps(4),
    };
    assert!(spheres_coincide(&s, &s));
    assert!(!spheres_coincide(
        &s,
        &Sphere {
            radius: Steps(5),
            ..s
        }
    ));
    assert!(!spheres_coincide(
        &s,
        &Sphere {
            centre: p(1, 1, 2),
            ..s
        }
    ));
    let c = Cylinder {
        base: p(0, 0, 0),
        along: p(0, 0, 1),
        radius: Steps(5),
    };
    let same = Cylinder {
        base: p(0, 0, -9),
        along: p(0, 0, 40),
        radius: Steps(5),
    };
    assert!(cylinders_coincide(&c, &same).unwrap());
    assert!(
        !cylinders_coincide(
            &c,
            &Cylinder {
                radius: Steps(6),
                ..same
            }
        )
        .unwrap()
    );
    assert!(
        !cylinders_coincide(
            &c,
            &Cylinder {
                base: p(1, 0, 0),
                along: p(1, 0, 1),
                ..same
            }
        )
        .unwrap()
    );
    assert!(
        !cylinders_coincide(
            &c,
            &Cylinder {
                along: p(1, 0, 1),
                base: p(0, 0, 0),
                ..same
            }
        )
        .unwrap()
    );
}

#[test]
fn degenerate_and_oversize_inputs_are_refused() {
    let q = p(0, 0, 0);
    let dull = Cylinder {
        base: q,
        along: q,
        radius: Steps(1),
    };
    assert_eq!(point_cylinder(q, &dull), Err(NumError::Degenerate));
    assert_eq!(
        point_cone(
            q,
            &Cone {
                apex: q,
                along: p(0, 0, 1),
                slope: Slope { rise: 1, run: 0 }
            }
        ),
        Err(NumError::Degenerate)
    );
    assert_eq!(
        point_sphere(
            q,
            &Sphere {
                centre: q,
                radius: Steps(1 << 41)
            }
        ),
        Err(NumError::BudgetExceeded)
    );
    assert_eq!(
        point_cone(
            q,
            &Cone {
                apex: q,
                along: p(0, 0, 1),
                slope: Slope {
                    rise: 1 << 40,
                    run: 1
                }
            }
        ),
        Err(NumError::BudgetExceeded)
    );
}

#[test]
fn the_point_decisions_agree_with_big_integers_at_the_corners_of_the_range() {
    let points = corners();
    let probes = [p(M, M, M), p(-M, M, -M), p(0, 0, 0), p(1, -1, 2)];
    for centre in &points {
        for &radius in &radii() {
            let sphere = Sphere {
                centre: *centre,
                radius,
            };
            for q in &probes {
                let x = vec(*q, *centre);
                let want =
                    sign_of(&(dot(&x, &x) - BigInt::from(radius.0) * BigInt::from(radius.0)));
                assert_eq!(
                    point_sphere(*q, &sphere).unwrap(),
                    want,
                    "{centre:?} {radius:?} {q:?}"
                );
            }
        }
    }
    // Axes through every pair of distinct corners of a coarse set, for the other three quadrics.
    let axes = [
        (p(-M, -M, -M), p(M, M, M)),
        (p(-M, M, 0), p(M, -M, 1)),
        (p(0, 0, 0), p(1, 0, 0)),
        (p(M, M, M), p(-M, M, M)),
        (p(-M, -M, M), p(-M + 1, M, -M)),
    ];
    for (a, b) in axes {
        let d = vec(b, a);
        for &radius in &radii() {
            let cyl = Cylinder {
                base: a,
                along: b,
                radius,
            };
            let r = BigInt::from(radius.0);
            for q in &probes {
                let x = vec(*q, a);
                let lateral = cross(&x, &d);
                let want = sign_of(&(dot(&lateral, &lateral) - &r * &r * dot(&d, &d)));
                assert_eq!(point_cylinder(*q, &cyl).unwrap(), want, "{a:?} {b:?} {q:?}");
            }
        }
        for (rise, run) in [
            (1u64, 1u64),
            ((1 << 40) - 1, 1),
            (1, (1 << 40) - 1),
            ((1 << 40) - 1, (1 << 40) - 1),
        ] {
            let cone = Cone {
                apex: a,
                along: b,
                slope: Slope { rise, run },
            };
            for q in &probes {
                let x = vec(*q, a);
                let lateral = cross(&x, &d);
                let along = dot(&x, &d);
                let want = sign_of(
                    &(BigInt::from(run).pow(2) * dot(&lateral, &lateral)
                        - BigInt::from(rise).pow(2) * &along * &along),
                );
                assert_eq!(
                    point_cone(*q, &cone).unwrap(),
                    want,
                    "{a:?} {b:?} {q:?} {rise}/{run}"
                );
            }
        }
        for (major, minor) in [
            (M as u64, M as u64),
            ((1 << 41) - 1, 1),
            (1, (1 << 41) - 1),
            (0, 0),
            ((1 << 41) - 1, (1 << 41) - 1),
        ] {
            let torus = Torus {
                centre: a,
                along: b,
                major: Steps(major),
                minor: Steps(minor),
            };
            let (mj, mn) = (BigInt::from(major), BigInt::from(minor));
            for q in &probes {
                let x = vec(*q, a);
                let big_e = dot(&d, &d);
                let tube = dot(&x, &x) + &mj * &mj - &mn * &mn;
                let along = dot(&x, &d);
                let want = sign_of(
                    &(&big_e * &tube * &tube
                        - BigInt::from(4) * &mj * &mj * (&big_e * dot(&x, &x) - &along * &along)),
                );
                assert_eq!(
                    point_torus(*q, &torus).unwrap(),
                    want,
                    "{a:?} {b:?} {q:?} {major} {minor}"
                );
            }
        }
    }
}

#[test]
fn the_plane_decisions_agree_with_big_integers_at_the_largest_planes() {
    let m = i128::from(M);
    // Planes through grid points at the corners reach 4M^2 and 4M^3: the planes' own extremes.
    let planes = [
        Plane::through(p(-M, -M, -M), p(M, -M, -M), p(-M, M, M)).unwrap(),
        Plane::through(p(M, M, -M), p(-M, M, M), p(M, -M, M)).unwrap(),
        Plane::through(p(-M, 0, M), p(M, 1, -M), p(0, M, 0)).unwrap(),
        Plane::new([0, 0, 1], -m).unwrap(),
        Plane::new([(1 << 82) - 1, -((1 << 82) - 1), 1], (1 << 124) - 1).unwrap(),
    ];
    let spheres = corners();
    for plane in &planes {
        let n = plane.n();
        let nv: V = [BigInt::from(n[0]), BigInt::from(n[1]), BigInt::from(n[2])];
        let d = BigInt::from(plane.d());
        for c in spheres.iter().step_by(7) {
            for &radius in &radii() {
                let value = dot(&nv, &[big(c.x.get()), big(c.y.get()), big(c.z.get())]) + &d;
                let r = BigInt::from(radius.0);
                let want = (&value * &value).cmp(&(&r * &r * dot(&nv, &nv)));
                let got = plane_sphere(plane, &Sphere { centre: *c, radius }).unwrap();
                let want = match want {
                    std::cmp::Ordering::Greater => Meeting::Apart,
                    std::cmp::Ordering::Equal => Meeting::Tangent,
                    std::cmp::Ordering::Less => Meeting::Crossing,
                };
                assert_eq!(got, want, "{plane:?} {c:?} {radius:?}");
            }
        }
        // Coincidence with itself, its negative, and a neighbour that differs in the offset.
        let negative = Plane::new([-n[0], -n[1], -n[2]], -plane.d()).unwrap();
        assert!(planes_coincide(plane, plane).unwrap());
        assert!(planes_coincide(plane, &negative).unwrap());
        if plane.d() > -(1 << 124) {
            let shifted = Plane::new(n, plane.d() - 1).unwrap();
            assert!(!planes_coincide(plane, &shifted).unwrap());
        }
    }
    // A plane square to the axis of a cylinder, and one parallel to it, at the extremes.
    let axis_cylinder = Cylinder {
        base: p(-M, -M, -M),
        along: p(M, -M, -M),
        radius: Steps((1 << 41) - 1),
    };
    let across = Plane::new([1, 0, 0], 0).unwrap();
    assert_eq!(
        plane_cylinder(&across, &axis_cylinder).unwrap(),
        Meeting::Crossing
    );
    let beside = Plane::new([0, 1, 0], -m).unwrap();
    assert_eq!(
        plane_cylinder(&beside, &axis_cylinder).unwrap(),
        Meeting::Crossing
    );
    let narrow = Cylinder {
        radius: Steps(M as u64),
        ..axis_cylinder
    };
    assert_eq!(plane_cylinder(&beside, &narrow).unwrap(), Meeting::Apart);
    let touching = Cylinder {
        radius: Steps(2 * M as u64),
        ..axis_cylinder
    };
    assert_eq!(
        plane_cylinder(&beside, &touching).unwrap(),
        Meeting::Tangent
    );
}
