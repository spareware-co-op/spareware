//! Attacks on G159: exact division, Sturm chains against polynomials with known roots, and every
//! generated predicate against the discriminant of its own polynomial, computed in big integers.
//!
//! The chains are built from factors, so the answer is known before the chain is: a quartic of four
//! whole roots has four, a factor x² + 1 removes two, a squared factor repeats. Scaling a polynomial
//! by a constant changes no root and reaches the budget's 2^256. The predicates are judged by an
//! oracle that shares nothing with them: `polynomial` over `Wide`, read back into `num-bigint`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

use geom_num::Sign;
use geom_num::grid::{COORD_MAX, P3, Steps};
use geom_num::interval::Interval;
use geom_num::quadric::{
    Cone, Cylinder, Line, Slope, Sphere, Surface, Torus, axis_gap, line_cone, line_cone_steep,
    line_cylinder, line_sphere, parallel, polar, polynomial, roots, roots_between,
    tangent_cone_cone, tangent_cylinder_cone,
};
use geom_num::sturm::{Big, Chain, MAX_COEFFICIENT_BITS, Point, Roots, div_exact, filtered};
use geom_num::wide::{I320, Wide};
use num_bigint::BigInt;

fn big(v: i128) -> Big {
    Big::from_i128(v)
}

/// The polynomial with these coefficients, lowest first.
fn poly(c: &[i128]) -> [Big; 5] {
    let mut out = [Big::ZERO; 5];
    for (slot, v) in out.iter_mut().zip(c) {
        *slot = big(*v);
    }
    out
}

/// `a · b`, for polynomials given lowest first.
fn times(a: &[i128], b: &[i128]) -> Vec<i128> {
    let mut out = vec![0; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            out[i + j] += x * y;
        }
    }
    out
}

fn count(c: &[i128]) -> Roots {
    Chain::of(&poly(c)).unwrap().roots()
}

fn roots_of(distinct: u8, repeated: bool) -> Roots {
    Roots { distinct, repeated }
}

#[test]
fn exact_division_gives_the_quotient_or_refuses() {
    let n = Wide::<8>::from_i128(1 << 100)
        .checked_mul(&Wide::from_i128(987_654_321))
        .unwrap();
    let d = Wide::<8>::from_i128(1 << 100);
    assert_eq!(div_exact(&n, &d).unwrap(), Wide::from_i128(987_654_321));
    let minus = d.checked_neg().unwrap();
    assert_eq!(
        div_exact(&n, &minus).unwrap(),
        Wide::from_i128(-987_654_321)
    );
    let top = n.checked_neg().unwrap();
    assert_eq!(
        div_exact(&top, &minus).unwrap(),
        Wide::from_i128(987_654_321)
    );
    assert_eq!(div_exact(&top, &d).unwrap(), Wide::from_i128(-987_654_321));
    // An odd divisor, several limbs of quotient, and a divisor whose zeros cross a limb.
    let odd = Wide::<8>::from_i128((1 << 90) + 1);
    let q = Wide::<8>::from_i128((1 << 120) + 12345);
    assert_eq!(div_exact(&q.checked_mul(&odd).unwrap(), &odd).unwrap(), q);
    let shifted = Wide::<8>::from_i128(3 << 70);
    assert_eq!(
        div_exact(&q.checked_mul(&shifted).unwrap(), &shifted).unwrap(),
        q
    );
    // Refused: no exact quotient, and a zero divisor.
    assert_eq!(
        div_exact(&Wide::<8>::from_i128(7), &Wide::from_i128(2)),
        None
    );
    assert_eq!(
        div_exact(&Wide::<8>::from_i128(7), &Wide::from_i128(3)),
        None
    );
    assert_eq!(div_exact(&Wide::<8>::from_i128(7), &Wide::ZERO), None);
    assert_eq!(
        div_exact(&Wide::<8>::ZERO, &Wide::from_i128(7)),
        Some(Wide::ZERO)
    );
}

#[test]
fn a_chain_counts_roots_it_was_built_from() {
    let linear = |r: i128| vec![-r, 1];
    let pair = |a: i128, b: i128| times(&linear(a), &linear(b));
    // Four simple roots, then each way of losing them.
    assert_eq!(count(&times(&pair(1, 2), &pair(3, 4))), roots_of(4, false));
    assert_eq!(count(&times(&pair(1, 2), &[1, 0, 1])), roots_of(2, false));
    assert_eq!(count(&times(&[1, 0, 1], &[2, 0, 1])), roots_of(0, false));
    // Repeats: one double, two doubles, a triple, a quadruple, a double complex pair.
    assert_eq!(count(&times(&pair(1, 1), &pair(3, 4))), roots_of(3, true));
    assert_eq!(count(&times(&pair(1, 1), &pair(3, 3))), roots_of(2, true));
    assert_eq!(count(&times(&pair(1, 1), &pair(1, 4))), roots_of(2, true));
    assert_eq!(count(&times(&pair(2, 2), &pair(2, 2))), roots_of(1, true));
    assert_eq!(count(&times(&[1, 0, 1], &[1, 0, 1])), roots_of(0, true));
    assert_eq!(count(&times(&pair(1, 1), &[1, 0, 1])), roots_of(1, true));
    // Lower degrees and a negative leading coefficient.
    assert_eq!(count(&[-2, 0, 1]), roots_of(2, false));
    assert_eq!(count(&[2, 0, 1]), roots_of(0, false));
    assert_eq!(count(&[1, 2, 1]), roots_of(1, true));
    assert_eq!(count(&[-1, 1]), roots_of(1, false));
    assert_eq!(count(&[0, 0, 0, 1]), roots_of(1, true));
    assert_eq!(count(&[6, -11, 6, -1]), roots_of(3, false));
    assert_eq!(count(&times(&pair(1, 2), &[-1, 0, -1])), roots_of(2, false));
    assert_eq!(count(&[-1, 0, 0, 0, -1]), roots_of(0, false));
    // Degree drops inside the chain: x⁴ + 1 has a gap of two, x⁴ − x a derivative with no
    // middle terms.
    assert_eq!(count(&[1, 0, 0, 0, 1]), roots_of(0, false));
    assert_eq!(count(&[0, -1, 0, 0, 1]), roots_of(2, false));
    assert_eq!(count(&[-1, 0, 0, 0, 1]), roots_of(2, false));
    assert_eq!(count(&[0, 1, 0, 1]), roots_of(1, false));
}

#[test]
fn a_constant_has_no_chain() {
    assert_eq!(
        Chain::of(&poly(&[3])).unwrap_err(),
        geom_num::NumError::Degenerate
    );
    assert_eq!(
        Chain::of(&poly(&[0])).unwrap_err(),
        geom_num::NumError::Degenerate
    );
}

/// Roots of every kind scaled to the top of the budget: the count does not move and nothing fails.
#[test]
fn the_budget_holds_at_its_top() {
    assert_eq!(MAX_COEFFICIENT_BITS, 256);
    let scale = |c: &[i128], k: u32| -> [Big; 5] {
        let m = Big::one()
            .checked_mul(&big(1 << 120))
            .unwrap()
            .checked_mul(&big(1 << 120))
            .unwrap();
        let m = m.checked_mul(&big(1 << k)).unwrap();
        let mut out = [Big::ZERO; 5];
        for (slot, v) in out.iter_mut().zip(c) {
            *slot = big(*v).checked_mul(&m).unwrap();
        }
        out
    };
    let cases: [(&[i128], Roots); 5] = [
        (&[24, -50, 35, -10, 1], roots_of(4, false)),
        (&[6, -5, 1, 0, 0], roots_of(2, false)),
        (&[2, 0, 3, 0, 1], roots_of(0, false)),
        (&[1, -2, 3, -2, 1], roots_of(0, true)),
        (&[1, -4, 6, -4, 1], roots_of(1, true)),
    ];
    for (c, want) in cases {
        // A coefficient of 50 at 2^(240 + 10) is near 2^256.
        assert_eq!(Chain::of(&scale(c, 10)).unwrap().roots(), want, "{c:?}");
    }
    // Coefficients with no common factor at the top of the budget, so nothing cancels.
    let huge = |sign: i128, low: i128| {
        let top = big(1 << 120).checked_mul(&big(1 << 120)).unwrap();
        top.checked_mul(&big(sign * ((1 << 15) + low)))
            .unwrap()
            .checked_add(&big(low))
            .unwrap()
    };
    let f = [
        huge(1, 1),
        huge(-1, 3),
        huge(1, 7),
        huge(-1, 11),
        huge(1, 13),
    ];
    let chain = Chain::of(&f).unwrap();
    assert!(chain.roots().distinct <= 4);
    assert!(
        chain
            .roots_between(Point::whole(-5), Point::whole(5))
            .is_ok()
    );
}

#[test]
fn roots_between_excludes_its_ends() {
    let f = times(&times(&[-1, 1], &[-2, 1]), &times(&[-3, 1], &[-4, 1]));
    let chain = Chain::of(&poly(&f)).unwrap();
    let whole = Point::whole;
    assert_eq!(chain.roots_between(whole(0), whole(5)).unwrap(), 4);
    assert_eq!(chain.roots_between(whole(1), whole(4)).unwrap(), 2);
    assert_eq!(chain.roots_between(whole(1), whole(2)).unwrap(), 0);
    let half = Point { num: 3, den: 2 };
    assert_eq!(chain.roots_between(half, whole(3)).unwrap(), 1);
    assert_eq!(chain.roots_between(whole(-9), half).unwrap(), 1);
    // A repeated root at an end, and one inside, counted once.
    let g = times(&times(&[-1, 1], &[-1, 1]), &times(&[-3, 1], &[-3, 1]));
    let chain = Chain::of(&poly(&g)).unwrap();
    assert_eq!(chain.roots_between(whole(1), whole(5)).unwrap(), 1);
    assert_eq!(chain.roots_between(whole(1), whole(3)).unwrap(), 0);
    assert_eq!(chain.roots_between(whole(0), whole(5)).unwrap(), 2);
    // Refused: an empty range, a bad denominator.
    let degenerate = Err(geom_num::NumError::Degenerate);
    assert_eq!(chain.roots_between(whole(2), whole(2)), degenerate);
    assert_eq!(chain.roots_between(whole(3), whole(2)), degenerate);
    let bad = Point { num: 1, den: 0 };
    assert_eq!(chain.roots_between(bad, whole(2)), degenerate);
    assert_eq!(chain.roots_between(whole(0), bad), degenerate);
}

fn point(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn line(a: [i64; 3], b: [i64; 3]) -> Line {
    Line {
        a: point(a[0], a[1], a[2]),
        b: point(b[0], b[1], b[2]),
    }
}

fn to_big(v: &I320) -> BigInt {
    let mut out = BigInt::from(0);
    for (i, limb) in v.to_le_limbs().iter().enumerate() {
        out += BigInt::from(*limb) << (64 * i);
    }
    if v.is_negative() {
        out -= BigInt::from(1) << 320;
    }
    out
}

/// The sign of the discriminant of a quadratic's polynomial, in big integers.
fn discriminant(line: &Line, surface: &Surface) -> i32 {
    let (f, degree) = polynomial::<I320>(line, surface).unwrap();
    assert_eq!(degree, 2);
    let (c, b, a) = (to_big(&f[0]), to_big(&f[1]), to_big(&f[2]));
    match (&b * &b - BigInt::from(4) * a * c).sign() {
        num_bigint::Sign::Plus => 1,
        num_bigint::Sign::NoSign => 0,
        num_bigint::Sign::Minus => -1,
    }
}

fn as_int(s: Sign) -> i32 {
    match s {
        Sign::Positive => 1,
        Sign::Zero => 0,
        Sign::Negative => -1,
    }
}

const M: i64 = COORD_MAX;

/// Lines of every extreme sign, the far corners of the grid included.
fn extreme_lines() -> Vec<Line> {
    let mut out = Vec::new();
    for mask in 0..64 {
        let at = |bit: u32| if mask >> bit & 1 == 1 { M } else { -M };
        let (a, b) = ([at(0), at(1), at(2)], [at(3), at(4), at(5)]);
        if a != b {
            out.push(line(a, b));
        }
    }
    out
}

#[test]
fn each_discriminant_is_its_polynomials() {
    let far = point(M, -M, M);
    let near = point(5, 7, -3);
    let radius = |r: u64| Steps(r);
    let spheres = [
        Sphere {
            centre: near,
            radius: radius(10),
        },
        Sphere {
            centre: far,
            radius: radius((1 << 41) - 1),
        },
    ];
    let cylinders = [
        Cylinder {
            base: near,
            along: point(5, 7, 9),
            radius: radius(10),
        },
        Cylinder {
            base: far,
            along: point(-M, M, -M),
            radius: radius((1 << 41) - 1),
        },
    ];
    let cones = [
        Cone {
            apex: near,
            along: point(5, 7, 9),
            slope: Slope { rise: 3, run: 4 },
        },
        Cone {
            apex: far,
            along: point(-M, M, -M),
            slope: Slope {
                rise: (1 << 40) - 1,
                run: (1 << 40) - 1,
            },
        },
    ];
    let mut lines = extreme_lines();
    lines.extend([
        line([0, 0, 0], [1, 0, 0]),
        line([5, 17, -3], [6, 17, -3]),
        line([5, 18, -3], [6, 18, -3]),
        line([-9, 2, 4], [8, 8, -8]),
    ]);
    for l in &lines {
        for s in &spheres {
            let got = line_sphere(l, s).unwrap();
            assert_eq!(
                as_int(got),
                discriminant(l, &Surface::Sphere(*s)),
                "{l:?} {s:?}"
            );
        }
        for c in &cylinders {
            // The sign of the polynomial's discriminant when it has one, and zero for a line parallel
            // to the axis, whose polynomial is the constant the discriminant's factor |e|² leaves.
            let got = line_cylinder(l, c).unwrap();
            assert_eq!(
                as_int(got),
                discriminant(l, &Surface::Cylinder(*c)),
                "{l:?} {c:?}"
            );
        }
        for c in &cones {
            let got = line_cone(l, c).unwrap();
            assert_eq!(
                as_int(got),
                discriminant(l, &Surface::Cone(*c)),
                "{l:?} {c:?}"
            );
            let lead = line_cone_steep(l, c).unwrap();
            let (f, _) = polynomial::<I320>(l, &Surface::Cone(*c)).unwrap();
            assert_eq!(lead, f[2].signum(), "{l:?} {c:?}");
        }
    }
}

#[test]
fn tangents_and_one_step_off() {
    let sphere = Sphere {
        centre: point(0, 0, 0),
        radius: Steps(5),
    };
    let at = |y: i64| line([0, y, 0], [1, y, 0]);
    assert_eq!(line_sphere(&at(5), &sphere), Ok(Sign::Zero));
    assert_eq!(line_sphere(&at(4), &sphere), Ok(Sign::Positive));
    assert_eq!(line_sphere(&at(6), &sphere), Ok(Sign::Negative));
    let cylinder = Cylinder {
        base: point(0, 0, 0),
        along: point(0, 0, 9),
        radius: Steps(5),
    };
    assert_eq!(line_cylinder(&at(5), &cylinder), Ok(Sign::Zero));
    assert_eq!(line_cylinder(&at(4), &cylinder), Ok(Sign::Positive));
    assert_eq!(line_cylinder(&at(6), &cylinder), Ok(Sign::Negative));
    // The cone x² + y² = z²: a line in the plane z = 1 touches it at y = 1.
    let cone = Cone {
        apex: point(0, 0, 0),
        along: point(0, 0, 1),
        slope: Slope { rise: 1, run: 1 },
    };
    let high = |y: i64| line([0, y, 1], [1, y, 1]);
    assert_eq!(line_cone(&high(1), &cone), Ok(Sign::Zero));
    assert_eq!(line_cone(&high(0), &cone), Ok(Sign::Positive));
    assert_eq!(line_cone(&high(2), &cone), Ok(Sign::Negative));
    // Steeper than the cone, along a ruling, and flatter.
    assert_eq!(
        line_cone_steep(&line([0, 0, 0], [1, 0, 1]), &cone),
        Ok(Sign::Zero)
    );
    assert_eq!(
        line_cone_steep(&line([0, 0, 0], [1, 0, 2]), &cone),
        Ok(Sign::Negative)
    );
    assert_eq!(
        line_cone_steep(&line([0, 0, 0], [2, 0, 1]), &cone),
        Ok(Sign::Positive)
    );
}

#[test]
fn the_position_tests_decide_what_they_say() {
    let (o, x, y) = (point(0, 0, 0), point(1, 0, 0), point(0, 1, 0));
    assert_eq!(parallel(o, x, y, point(9, 1, 0)), Ok(true));
    assert_eq!(parallel(o, x, y, point(9, 2, 0)), Ok(false));
    assert_eq!(parallel(o, o, y, x), Ok(true));
    assert_eq!(
        parallel(
            point(-M, -M, -M),
            point(M, M, M),
            point(M, -M, M),
            point(-M, M, -M)
        ),
        Ok(false)
    );
    let z = point(0, 0, 1);
    assert_eq!(axis_gap(o, z, point(3, 4, 7), Steps(5)), Ok(Sign::Zero));
    assert_eq!(axis_gap(o, z, point(3, 4, 7), Steps(4)), Ok(Sign::Positive));
    assert_eq!(axis_gap(o, z, point(3, 4, 7), Steps(6)), Ok(Sign::Negative));
    assert_eq!(
        axis_gap(
            point(-M, -M, -M),
            point(M, M, M),
            point(M, -M, M),
            Steps((1 << 42) - 1)
        ),
        Ok(Sign::Negative)
    );
    assert_eq!(
        axis_gap(o, z, x, Steps(1 << 42)),
        Err(geom_num::NumError::BudgetExceeded)
    );
}

/// A cone with half-angle of tangent 3/4 has sine 3/5, so from 10 steps along its axis the sphere that
/// touches it has radius 6.
#[test]
fn a_tangent_sphere_is_found_exactly() {
    let cone = Cone {
        apex: point(0, 0, 10),
        along: point(0, 0, 0),
        slope: Slope { rise: 3, run: 4 },
    };
    let cylinder = |r: u64| Cylinder {
        base: point(-7, 0, 0),
        along: point(9, 0, 0),
        radius: Steps(r),
    };
    assert_eq!(tangent_cylinder_cone(&cylinder(6), &cone), Ok(Sign::Zero));
    assert_eq!(
        tangent_cylinder_cone(&cylinder(7), &cone),
        Ok(Sign::Positive)
    );
    assert_eq!(
        tangent_cylinder_cone(&cylinder(5), &cone),
        Ok(Sign::Negative)
    );
    // A slope of 4/3 has sine 4/5: 15 steps along gives radius 12, as 20 along at 3/4 does.
    let first = Cone {
        apex: point(0, 0, 20),
        along: point(0, 0, 0),
        slope: Slope { rise: 3, run: 4 },
    };
    let second = |at: i64| Cone {
        apex: point(at, 0, 0),
        along: point(0, 0, 0),
        slope: Slope { rise: 4, run: 3 },
    };
    assert_eq!(tangent_cone_cone(&first, &second(15)), Ok(Sign::Zero));
    assert_eq!(tangent_cone_cone(&first, &second(14)), Ok(Sign::Positive));
    assert_eq!(tangent_cone_cone(&first, &second(16)), Ok(Sign::Negative));
    assert_eq!(tangent_cone_cone(&second(15), &first), Ok(Sign::Zero));
    // The budgets at the far corners of the grid.
    let huge = Slope {
        rise: (1 << 40) - 1,
        run: 1,
    };
    let cone = Cone {
        apex: point(-M, -M, M),
        along: point(M, M, M),
        slope: huge,
    };
    let other = Cone {
        apex: point(M, -M, -M),
        along: point(-M, M, M),
        slope: Slope {
            rise: 1,
            run: (1 << 40) - 1,
        },
    };
    let wide = Cylinder {
        base: point(M, -M, M),
        along: point(-M, M, -M),
        radius: Steps((1 << 41) - 1),
    };
    assert!(tangent_cylinder_cone(&wide, &cone).is_ok());
    assert!(tangent_cone_cone(&cone, &other).is_ok());
}

fn torus(major: u64, minor: u64) -> Surface {
    Surface::Torus(Torus {
        centre: point(0, 0, 0),
        along: point(0, 0, 1),
        major: Steps(major),
        minor: Steps(minor),
    })
}

#[test]
fn a_line_meets_a_torus_as_its_shape_says() {
    let ring = torus(10, 3);
    // Through the middle in the plane of the ring: in and out of the tube twice.
    assert_eq!(
        roots(&line([0, 0, 0], [1, 0, 0]), &ring),
        Ok(roots_of(4, false))
    );
    // Along the axis: through the hole. The quartic is a square, so its complex roots repeat.
    assert_eq!(
        roots(&line([0, 0, 0], [0, 0, 1]), &ring),
        Ok(roots_of(0, true))
    );
    // Along the top of the tube: touching it on both sides, two double roots.
    assert_eq!(
        roots(&line([0, 0, 3], [1, 0, 3]), &ring),
        Ok(roots_of(2, true))
    );
    // A step below the top: four roots, and a step above: none.
    assert_eq!(
        roots(&line([0, 0, 2], [1, 0, 2]), &ring),
        Ok(roots_of(4, false))
    );
    assert_eq!(
        roots(&line([0, 0, 4], [1, 0, 4]), &ring),
        Ok(roots_of(0, false))
    );
    // Through the tube once, off to the side.
    assert_eq!(
        roots(&line([11, 0, 0], [11, 1, 0]), &ring),
        Ok(roots_of(2, false))
    );
    // Along the outside: y = 13 touches the ring at one point, a double root.
    assert_eq!(
        roots(&line([0, 13, 0], [1, 13, 0]), &ring),
        Ok(roots_of(1, true))
    );
    // On a stretch of the line: the two crossings of the near side only.
    let across = line([0, 0, 0], [1, 0, 0]);
    let lo = Point::whole(0);
    assert_eq!(roots_between(&across, &ring, lo, Point::whole(20)), Ok(2));
    assert_eq!(roots_between(&across, &ring, lo, Point::whole(10)), Ok(1));
    assert_eq!(roots_between(&across, &ring, lo, Point::whole(5)), Ok(0));
    assert_eq!(roots_between(&across, &ring, Point::whole(-20), lo), Ok(2));
}

/// The chain and the filter on the coefficients of real lines at the grid's far corners: the budget's
/// B = 256 holds, each coefficient fits it, and the filter never disagrees with the chain.
#[test]
fn the_torus_budget_holds_at_the_far_corners() {
    let ring = Surface::Torus(Torus {
        centre: point(M, M, -M),
        along: point(-M, M, M),
        major: Steps((1 << 41) - 1),
        minor: Steps((1 << 41) - 1),
    });
    let limit = BigInt::from(1) << MAX_COEFFICIENT_BITS;
    for l in extreme_lines() {
        let (f, degree) = polynomial::<I320>(&l, &ring).unwrap();
        assert_eq!(degree, 4);
        for c in &f {
            let v = to_big(c);
            assert!(v < limit && v > -limit.clone(), "{l:?}");
        }
        let exact = {
            let mut wide = [Big::ZERO; 5];
            for (slot, c) in wide.iter_mut().zip(&f) {
                *slot = c.resize().unwrap();
            }
            Chain::of(&wide).unwrap().roots()
        };
        let (g, degree) = polynomial::<Interval>(&l, &ring).unwrap();
        if let Some(fast) = filtered(&g, degree) {
            assert_eq!(fast, exact, "{l:?}");
        }
        assert_eq!(roots(&l, &ring), Ok(exact), "{l:?}");
    }
}

#[test]
fn the_interval_polynomial_encloses_the_exact_one() {
    let cases = [
        (line([1, 2, 3], [4, 7, -2]), torus(10, 3)),
        (
            line([1, 2, 3], [4, 7, -2]),
            Surface::Sphere(Sphere {
                centre: point(1, 1, 1),
                radius: Steps(4),
            }),
        ),
        (
            line([1, 2, 3], [4, 7, -2]),
            Surface::Cylinder(Cylinder {
                base: point(0, 0, 0),
                along: point(1, 2, 3),
                radius: Steps(4),
            }),
        ),
        (
            line([1, 2, 3], [4, 7, -2]),
            Surface::Cone(Cone {
                apex: point(0, 0, 0),
                along: point(1, 2, 3),
                slope: Slope { rise: 2, run: 3 },
            }),
        ),
    ];
    for (l, s) in cases {
        let (exact, degree) = polynomial::<I320>(&l, &s).unwrap();
        let (around, same) = polynomial::<Interval>(&l, &s).unwrap();
        assert_eq!(degree, same);
        for (e, i) in exact.iter().zip(&around) {
            let v = e.to_i128().unwrap() as f64;
            assert!(i.contains(v), "{v} {i:?}");
        }
    }
}

#[test]
fn a_size_past_the_budget_is_refused() {
    let past = Steps(1 << 41);
    let l = line([0, 0, 0], [1, 0, 0]);
    let refused = Err(geom_num::NumError::BudgetExceeded);
    assert_eq!(
        line_sphere(
            &l,
            &Sphere {
                centre: point(0, 0, 0),
                radius: past
            }
        ),
        refused
    );
    let axis = (point(0, 0, 0), point(0, 0, 1));
    assert_eq!(
        line_cylinder(
            &l,
            &Cylinder {
                base: axis.0,
                along: axis.1,
                radius: past
            }
        ),
        refused
    );
    let steep = Slope {
        rise: 1 << 40,
        run: 1,
    };
    let cone = Cone {
        apex: axis.0,
        along: axis.1,
        slope: steep,
    };
    assert_eq!(line_cone(&l, &cone), refused);
    assert_eq!(line_cone_steep(&l, &cone), refused);
}

fn at(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

#[test]
fn the_polar_form_is_each_quadric_on_the_diagonal() {
    let q = |v: i128| Some(I320::from_i128(v));
    let sphere = Surface::Sphere(Sphere {
        centre: at(1, 0, 0),
        radius: Steps(5),
    });
    assert_eq!(polar(&sphere, at(4, 4, 0), at(4, 4, 0)), q(0));
    assert_eq!(polar(&sphere, at(1, 0, 0), at(2, 3, 0)), q(-25));
    assert_eq!(polar(&sphere, at(2, 0, 0), at(3, 1, 1)), q(-23));
    assert_eq!(polar(&sphere, at(3, 1, 1), at(2, 0, 0)), q(-23));
    let cylinder = Surface::Cylinder(Cylinder {
        base: at(0, 0, 0),
        along: at(0, 0, 2),
        radius: Steps(3),
    });
    assert_eq!(polar(&cylinder, at(3, 0, 9), at(3, 0, 9)), q(0));
    assert_eq!(polar(&cylinder, at(1, 0, 0), at(0, 1, 0)), q(-36));
    let cone = Surface::Cone(Cone {
        apex: at(0, 0, 0),
        along: at(0, 0, 1),
        slope: Slope { rise: 1, run: 1 },
    });
    assert_eq!(polar(&cone, at(1, 0, 1), at(1, 0, 1)), q(0));
    assert_eq!(polar(&cone, at(2, 0, 1), at(2, 0, 1)), q(3));
    let torus = Surface::Torus(Torus {
        centre: at(0, 0, 0),
        along: at(0, 0, 1),
        major: Steps(5),
        minor: Steps(1),
    });
    assert_eq!(polar(&torus, at(1, 0, 0), at(1, 0, 0)), None);
}

#[test]
fn a_bound_no_double_holds_goes_to_the_exact_chain() {
    let sphere = Surface::Sphere(Sphere {
        centre: at(0, 0, 0),
        radius: Steps(5),
    });
    let line = Line {
        a: at(-9, 0, 0),
        b: at(9, 0, 0),
    };
    let far = (1_i64 << 53) + 1;
    let found = roots_between(&line, &sphere, Point::whole(-far), Point::whole(far));
    assert_eq!(found, Ok(2));
    let tiny = Point { num: -1, den: far };
    assert_eq!(roots_between(&line, &sphere, tiny, Point::whole(1)), Ok(2));
    // Within the doubles the filter answers, and a bound with no positive denominator is refused
    // whichever path it takes.
    let near = roots_between(&line, &sphere, Point::whole(-1), Point::whole(1));
    assert_eq!(near, Ok(2));
    let wrong = Point { num: 1, den: -1 };
    let refused = roots_between(&line, &sphere, wrong, Point::whole(1));
    assert_eq!(refused, Err(geom_num::NumError::Degenerate));
}
