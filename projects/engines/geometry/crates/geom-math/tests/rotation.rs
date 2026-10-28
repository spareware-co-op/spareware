//! Piece G46's rational rotations (`geom_math::rotation`, decision 32.14), against exact oracles.
//!
//! Every angle bound a search reports is checked against the true angle, computed exactly in
//! rationals from the doubles of the target and the integers of the result: the bound must hold
//! (the true angle is at most the reported one), it must be at most the bound asked for, and it
//! must be tight (within 2^−10, relatively, of the truth, plus the interval arithmetic's slack).
//! In 2D, a brute-force search over every smaller half-angle denominator checks that no simpler
//! rotation was missed. Images of grid points are checked against the exact image rounded half to
//! even, compositions against exact matrix products, and the typed transform's new variant against
//! the transforms it stands beside.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::needless_range_loop,
    clippy::single_match_else,
    clippy::unreachable,
    reason = "test cases and oracles, named as in the formulas they check; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]

mod common;

use common::{config, exact, int, is_nearest_double, pow2, round_half_even, to_i128};
use geom_math::rotation::{BOUND_MAX, NORM2_LIMIT};
use geom_math::{
    Axis, F2, GridTransform, MathError, Pythagorean, RationalRotation, RationalTransform, Rotation,
    Scale, Transform, TransformKind, TypedTransform,
};
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::{NumError, Real, ladder};
use num_rational::BigRational;
use num_traits::{Signed, Zero};
use proptest::prelude::*;

/// The angular bound 2^−34, the tightest bound a search accepts.
fn eps() -> f64 {
    ladder::ANGULAR.get()
}

fn real(x: f64) -> Real {
    Real::new(x).unwrap()
}

/// `SplitMix64`: the test's own generator, from fixed seeds.
struct Mix(u64);

impl Mix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A double in [−1, 1).
    fn signed(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A lower bound on sin z for z ≥ 0: z − z³/6.
fn sin_lower(z: &BigRational) -> BigRational {
    z - z * z * z / int(6)
}

/// An upper bound on sin z for z ≥ 0: z − z³/6 + z⁵/120.
fn sin_upper(z: &BigRational) -> BigRational {
    let z2 = z * z;
    z - z * &z2 / int(6) + z * &z2 * &z2 / int(120)
}

/// The exact sin² of the half-angle between the integer quaternion `q` and the target `t`, which
/// must have a positive dot product with it.
fn half_angle_sin2(q: [i64; 4], t: &[BigRational; 4]) -> BigRational {
    let q: Vec<BigRational> = q.iter().map(|&v| int(i128::from(v))).collect();
    let dot: BigRational = q.iter().zip(t).map(|(a, b)| a * b).sum();
    assert!(dot.is_positive(), "the result is not on the target's side");
    let mut wedge = BigRational::zero();
    for i in 0..4 {
        for j in i + 1..4 {
            let w = &q[i] * &t[j] - &q[j] * &t[i];
            wedge += &w * &w;
        }
    }
    let qn: BigRational = q.iter().map(|v| v * v).sum();
    let tn: BigRational = t.iter().map(|v| v * v).sum();
    wedge / (qn * tn)
}

/// The reported bound holds and is tight, for a true angle 2∠ with sin²∠ = `s2`: 2∠ ≤ reported,
/// and reported ≤ 2·sin∠·(1 + 2^−10) + 2^−40.
fn check_half_angle(s2: &BigRational, reported: f64) {
    let r = exact(reported);
    let y = sin_lower(&(&r / int(2)));
    assert!(
        y.is_positive() && s2 <= &(&y * &y),
        "the true angle exceeds the bound {reported:e}"
    );
    let z = (&r - pow2(-40)) / (int(2) * (int(1) + pow2(-10)));
    if z.is_positive() {
        assert!(&z * &z <= *s2, "the bound {reported:e} is not tight");
    }
}

/// The certificate of a 3D search or a re-approximation against the exact target.
fn check_quaternion(q: [i64; 4], t: &[BigRational; 4], reported: f64) {
    check_half_angle(&half_angle_sin2(q, t), reported);
}

/// The angle between `direction` and the Pythagorean direction `p`: certified and tight.
fn check_direction(d: [f64; 2], p: &Pythagorean, reported: f64) {
    let (a, b, c) = (int(p.a()), int(p.b()), int(p.c()));
    assert_eq!(&a * &a + &b * &b, &c * &c, "not a Pythagorean triple");
    let (dx, dy) = (exact(d[0]), exact(d[1]));
    let dot = &dx * &a + &dy * &b;
    assert!(dot.is_positive(), "the direction is on the wrong side");
    let cross = &dx * &b - &dy * &a;
    let s2 = &cross * &cross / ((&dx * &dx + &dy * &dy) * &c * &c);
    let r = exact(reported);
    let y = sin_lower(&r);
    assert!(
        s2 <= &y * &y,
        "the true angle exceeds the bound {reported:e}"
    );
    // The angle φ is at least sin φ, and at most sin φ·(1 + φ²): the bound is tight to 2^−10.
    let z = (&r - pow2(-40)) / ((int(1) + pow2(-10)) * (int(1) + &r * &r));
    if z.is_positive() {
        assert!(&z * &z <= s2, "the bound {reported:e} is not tight");
    }
}

/// The direction turned clockwise by quarter turns, as the 2D search does: (k, the turned one).
fn octant(d: [f64; 2]) -> (u32, [f64; 2]) {
    let [x, y] = d;
    if x > 0.0 && x >= y.abs() {
        (0, [x, y])
    } else if y > 0.0 && y >= x.abs() {
        (1, [y, -x])
    } else if x < 0.0 && -x >= y.abs() {
        (2, [-x, -y])
    } else {
        (3, [-y, x])
    }
}

/// No rotation (m', 0, 0, n') with m' below the result's half-angle denominator is within
/// (1 − 2^−9)·bound of the direction (checked exactly for denominators up to `cap`). Returns
/// whether the check ran.
fn check_simplest(d: [f64; 2], r: &RationalRotation, bound: f64, cap: i64) -> bool {
    let (k, e) = octant(d);
    let back = RationalRotation::new([1, 0, 0, -1]).unwrap();
    let mut reduced = *r;
    for _ in 0..k {
        reduced = reduced.compose_exact(&back).unwrap().unwrap();
    }
    let [m, x, y, n] = reduced.quaternion();
    assert_eq!((x, y), (0, 0));
    assert!(m > 0 && n.abs() <= m, "not in the octant: {m} {n}");
    if m > cap {
        return false;
    }
    let (ex, ey) = (exact(e[0]), exact(e[1]));
    let d2 = (&ex * &ex + &ey * &ey) * int(1);
    let near = exact(bound) * (int(1) - pow2(-9));
    let limit = sin_upper(&near);
    let limit2 = &limit * &limit;
    for mm in 1..m {
        for nn in -mm..=mm {
            let (mm, nn) = (int(i128::from(mm)), int(i128::from(nn)));
            let (a, b) = (&mm * &mm - &nn * &nn, int(2) * &mm * &nn);
            let c = &mm * &mm + &nn * &nn;
            let dot = &ex * &a + &ey * &b;
            if !dot.is_positive() {
                continue;
            }
            let cross = &ex * &b - &ey * &a;
            let s2 = &cross * &cross / (&d2 * &c * &c);
            assert!(
                s2 > limit2,
                "a simpler rotation ({mm}, {nn}) is within the bound"
            );
        }
    }
    true
}

/// The budget of every result: |q|² < 2^72.
fn check_budget(r: &RationalRotation) {
    assert!(r.norm2() < NORM2_LIMIT);
    for v in r.quaternion() {
        assert!(v.unsigned_abs() < 1 << 36);
    }
}

/// M(q) from the module's formula, in rationals.
fn matrix_of(q: [i64; 4]) -> [[BigRational; 3]; 3] {
    let [w, x, y, z] = q.map(|v| int(i128::from(v)));
    let two = int(2);
    [
        [
            &w * &w + &x * &x - &y * &y - &z * &z,
            &two * (&x * &y - &w * &z),
            &two * (&x * &z + &w * &y),
        ],
        [
            &two * (&x * &y + &w * &z),
            &w * &w - &x * &x + &y * &y - &z * &z,
            &two * (&y * &z - &w * &x),
        ],
        [
            &two * (&x * &z - &w * &y),
            &two * (&y * &z + &w * &x),
            &w * &w - &x * &x - &y * &y + &z * &z,
        ],
    ]
}

/// The rotation R = M(q)/|q|² in rationals.
fn rotation_of(q: [i64; 4]) -> [[BigRational; 3]; 3] {
    let n: BigRational = q
        .iter()
        .map(|&v| int(i128::from(v)) * int(i128::from(v)))
        .sum();
    matrix_of(q).map(|row| row.map(|v| v / &n))
}

fn mat_mul(a: &[[BigRational; 3]; 3], b: &[[BigRational; 3]; 3]) -> [[BigRational; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| &a[i][k] * &b[k][j]).sum()))
}

/// A random quaternion with every component below 2^bits in magnitude, not zero.
fn random_q(mix: &mut Mix, bits: u32) -> [i64; 4] {
    loop {
        let q: [i64; 4] =
            std::array::from_fn(|_| (mix.next() >> (64 - bits)) as i64 - (1i64 << (bits - 1)));
        if q != [0; 4] {
            return q;
        }
    }
}

#[test]
fn pythagorean_triples_and_their_rotations() {
    let p = Pythagorean::new(3, 4, 5).unwrap();
    assert_eq!((p.a(), p.b(), p.c()), (3, 4, 5));
    assert_eq!(Pythagorean::new(6, 8, 10).unwrap(), p);
    assert_eq!(Pythagorean::new(-6, -8, -10).unwrap(), p);
    let q = Pythagorean::new(3, 4, -5).unwrap();
    assert_eq!((q.a(), q.b(), q.c()), (-3, -4, 5));
    let half = Pythagorean::new(-5, 0, 5).unwrap();
    assert_eq!((half.a(), half.b(), half.c()), (-1, 0, 1));
    assert_eq!(Pythagorean::new(0, -7, 7).unwrap().b(), -1);
    for (a, b, c) in [(1, 1, 1), (0, 0, 0), (3, 4, 0), (3, 4, 6)] {
        assert_eq!(Pythagorean::new(a, b, c), Err(NumError::Degenerate));
    }
    assert_eq!(Pythagorean::X, Pythagorean::new(1, 0, 1).unwrap());

    // The rotation that turns the x axis to (3, 4)/5 is (2, 0, 0, 1): tan(θ/2) = 4/(5 + 3).
    let r = RationalRotation::about(Axis::Z, &p).unwrap();
    assert_eq!(r.quaternion(), [2, 0, 0, 1]);
    assert_eq!(r.direction(), Some(p));
    assert_eq!(
        RationalRotation::about(Axis::Z, &half)
            .unwrap()
            .quaternion(),
        [0, 0, 0, 1]
    );
    assert_eq!(
        RationalRotation::about(Axis::Z, &Pythagorean::X).unwrap(),
        RationalRotation::IDENTITY
    );
    assert_eq!(
        RationalRotation::about(Axis::X, &p).unwrap().quaternion(),
        [2, 1, 0, 0]
    );
    assert_eq!(
        RationalRotation::about(Axis::Y, &p).unwrap().quaternion(),
        [2, 0, 1, 0]
    );
    assert_eq!(
        RationalRotation::about(Axis::X, &p).unwrap().direction(),
        None
    );

    // About each axis, the same transform as the rational transforms' own Pythagorean rotation.
    for (c, s) in [
        (3, 4),
        (4, 3),
        (-3, 4),
        (5, -12),
        (-20, -21),
        (0, 1),
        (-1, 0),
    ] {
        let len = f64::from(c * c + s * s).sqrt() as i64;
        let p = Pythagorean::new(i64::from(c), i64::from(s), len).unwrap();
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            let ours = TypedTransform::from(RationalRotation::about(axis, &p).unwrap());
            let theirs =
                TypedTransform::rotation(axis, Rotation::direction(c, s).unwrap()).unwrap();
            assert_eq!(ours, theirs, "({c}, {s}) about {axis:?}");
        }
    }

    // A direction turned back from its rotation, for every canonical (w, z) up to 40.
    for w in -40..=40i64 {
        for z in -40..=40i64 {
            let Ok(r) = RationalRotation::new([w, 0, 0, z]) else {
                assert_eq!((w, z), (0, 0));
                continue;
            };
            let d = r.direction().unwrap();
            assert_eq!(RationalRotation::about(Axis::Z, &d).unwrap(), r);
        }
    }
}

#[test]
fn canonical_form_budget_and_matrix() {
    let quarter = RationalRotation::new([1, 0, 0, 1]).unwrap();
    assert_eq!(RationalRotation::new([2, 0, 0, 2]).unwrap(), quarter);
    assert_eq!(RationalRotation::new([-3, 0, 0, -3]).unwrap(), quarter);
    assert_eq!(
        RationalRotation::new([0, -3, 0, 0]).unwrap().quaternion(),
        [0, 1, 0, 0]
    );
    assert_eq!(
        RationalRotation::new([0, 0, -2, 4]).unwrap().quaternion(),
        [0, 0, 1, -2]
    );
    assert_eq!(RationalRotation::new([0; 4]), Err(NumError::Degenerate));
    assert_eq!(RationalRotation::default(), RationalRotation::IDENTITY);

    let big = (1i64 << 36) - 1;
    let r = RationalRotation::new([big, 1, 0, 0]).unwrap();
    assert_eq!(r.norm2(), (1u128 << 72) - (1 << 37) + 2);
    check_budget(&r);
    assert_eq!(
        RationalRotation::new([1 << 36, 1, 0, 0]),
        Err(NumError::OutOfRange { value: 1 << 36 })
    );
    assert_eq!(
        RationalRotation::new([-(1 << 36), 1, 0, 0]),
        Err(NumError::OutOfRange { value: 1 << 36 })
    );
    // Every component below 2^36, but |q|² past 2^72.
    let wide = RationalRotation::new([big, big, 0, 0]);
    assert!(matches!(wide, Err(NumError::OutOfRange { value }) if value >= 1 << 72));
    // The same multiple of a small rotation reduces into the budget.
    assert_eq!(
        RationalRotation::new([3 << 35, 4 << 35, 0, 0])
            .unwrap()
            .quaternion(),
        [3, 4, 0, 0]
    );
    assert!(matches!(
        RationalRotation::new([i64::MIN, 0, 0, 0]),
        Ok(r) if r == RationalRotation::IDENTITY
    ));

    let mut mix = Mix(0x6460_0001);
    for bits in [2, 8, 20, 35, 36] {
        for _ in 0..200 {
            let Ok(r) = RationalRotation::new(random_q(&mut mix, bits)) else {
                continue;
            };
            check_budget(&r);
            let q = r.quaternion();
            let m = r.matrix().unwrap();
            let want = matrix_of(q);
            for i in 0..3 {
                for j in 0..3 {
                    assert_eq!(int(m[i][j]), want[i][j]);
                }
            }
            // M(q)·M(q)ᵀ = |q|⁴·I and det M(q) = |q|⁶: a rotation.
            let rot = rotation_of(q);
            let mut rt = rot.clone();
            for i in 0..3 {
                for j in 0..3 {
                    rt[i][j] = rot[j][i].clone();
                }
            }
            let id = mat_mul(&rot, &rt);
            for i in 0..3 {
                for j in 0..3 {
                    assert_eq!(id[i][j], int(i128::from(i == j)));
                }
            }
            assert_eq!(r.classify().kind, TransformKind::Rigid);
            assert!(!r.classify().mirror);
            // The inverse is the conjugate, and composes to the identity exactly.
            let inv = r.inverse();
            check_budget(&inv);
            assert_eq!(inv.inverse(), r);
            let id = r.compose(&inv, real(eps())).unwrap();
            assert!(id.is_exact() && id.value() == RationalRotation::IDENTITY);
        }
    }
    assert_eq!(
        RationalRotation::IDENTITY.classify().kind,
        TransformKind::Identity
    );
    assert_eq!(
        RationalRotation::new([0, 0, 0, 5])
            .unwrap()
            .inverse()
            .quaternion(),
        [0, 0, 0, 1]
    );
}

#[test]
fn images_are_the_exact_image_rounded_once() {
    let mut mix = Mix(0x6460_0002);
    let edge = COORD_MAX;
    let mut ties = 0;
    let mut inexact = 0;
    for bits in [2, 3, 5, 11, 21, 31, 36] {
        for _ in 0..150 {
            let Ok(r) = RationalRotation::new(random_q(&mut mix, bits)) else {
                continue;
            };
            let rot = rotation_of(r.quaternion());
            for _ in 0..8 {
                let pick = |mix: &mut Mix| match mix.below(4) {
                    0 => edge,
                    1 => -edge,
                    2 => (mix.next() >> 44) as i64 - (1 << 19),
                    _ => (mix.next() >> 25) as i64 - (1 << 38),
                };
                let p = [pick(&mut mix), pick(&mut mix), pick(&mut mix)];
                let point = P3::new(p[0], p[1], p[2]).unwrap();
                let image: Vec<BigRational> = (0..3)
                    .map(|i| (0..3).map(|j| &rot[i][j] * int(i128::from(p[j]))).sum())
                    .collect();
                let rounded: Vec<i128> =
                    image.iter().map(|v| to_i128(&round_half_even(v))).collect();
                let exact_image = image.iter().all(BigRational::is_integer);
                ties += usize::from(
                    image
                        .iter()
                        .any(|v| (v - v.floor()) == BigRational::new(1.into(), 2.into())),
                );
                match r.apply(point) {
                    Ok(got) => {
                        let v = got.value();
                        assert_eq!(
                            [v.x.get(), v.y.get(), v.z.get()].map(i128::from),
                            [rounded[0], rounded[1], rounded[2]]
                        );
                        assert_eq!(got.is_exact(), exact_image);
                        inexact += usize::from(!exact_image);
                        // Beside the rational transform it is equal to, the same answer.
                        if let Some(t) = r.to_rational() {
                            assert_eq!(t.apply(point).unwrap(), got);
                        }
                        assert_eq!(TypedTransform::from(r).apply(point).unwrap(), got);
                    }
                    Err(NumError::OutOfRange { value }) => {
                        assert!(value.unsigned_abs() > edge as u128);
                        assert!(rounded.iter().any(|v| v.unsigned_abs() > edge as u128));
                    }
                    Err(e) => panic!("{e:?}"),
                }
            }
        }
    }
    assert!(ties > 0 && inexact > 100, "ties {ties}, inexact {inexact}");

    // In the plane, the 2D image is the 3D image at z = 0; a 3D rotation refuses it.
    let r = RationalRotation::new([123_456_789, 0, 0, 987_654_321]).unwrap();
    let p = P2::new(edge, -12345).unwrap();
    let got = r.apply_2d(p).unwrap();
    let full = r.apply(P3::new(edge, -12345, 0).unwrap()).unwrap();
    assert_eq!(
        (got.value().x, got.value().y),
        (full.value().x, full.value().y)
    );
    assert_eq!(full.value().z.get(), 0);
    assert_eq!(got.is_exact(), full.is_exact());
    let tilt = RationalRotation::new([3, 1, 0, 0]).unwrap();
    assert_eq!(tilt.apply_2d(p), Err(MathError::NotInPlane));
    assert!(!tilt.acts_in_plane() && r.acts_in_plane());
    // Out of range in the plane.
    let eighth = RationalRotation::new([5741, 0, 0, 2378]).unwrap();
    assert!(matches!(
        eighth.apply_2d(P2::new(edge, edge).unwrap()),
        Err(MathError::Num(NumError::OutOfRange { .. }))
    ));
}

#[test]
fn rational_transforms_both_ways() {
    let mut mix = Mix(0x6460_0003);
    let mut small = 0;
    for bits in [2, 4, 8, 11, 12, 21] {
        for _ in 0..300 {
            let Ok(r) = RationalRotation::new(random_q(&mut mix, bits)) else {
                continue;
            };
            match r.to_rational() {
                Some(t) => {
                    small += 1;
                    assert!(r.norm2() <= 1 << 40);
                    // The same map: R·e for each axis.
                    let rot = rotation_of(r.quaternion());
                    let m = t.matrix();
                    let d = int(i128::from(t.denominator()));
                    for i in 0..3 {
                        for j in 0..3 {
                            assert_eq!(int(i128::from(m[i][j])) / &d, rot[i][j]);
                        }
                    }
                    assert_eq!(t.translation_part(), [0; 3]);
                    assert_eq!(RationalRotation::from_rational(&t), Some(r));
                    assert!(matches!(
                        TypedTransform::from(r),
                        TypedTransform::Rational(_) | TypedTransform::Exact(_)
                    ));
                }
                None => {
                    assert!(r.norm2() > 1 << 40);
                    assert_eq!(TypedTransform::from(r), TypedTransform::Rotation(r));
                }
            }
        }
    }
    assert!(small > 500);

    // Every grid rotation (the 24 rotations of the cube) is a rotation, every mirror is not.
    let mut rotations = 0;
    let identity = GridTransform::IDENTITY;
    for a in [Axis::X, Axis::Y, Axis::Z] {
        for turns in 0..4 {
            for b in [Axis::X, Axis::Y, Axis::Z] {
                for more in 0..4 {
                    let g = GridTransform::quarter_turns(a, turns)
                        .compose(&GridTransform::quarter_turns(b, more))
                        .unwrap();
                    let t = RationalTransform::from_grid(&g);
                    let r = RationalRotation::from_rational(&t).unwrap();
                    assert_eq!(r.to_rational(), Some(t));
                    rotations += 1;
                    let mirrored = g.compose(&GridTransform::mirror(Axis::X)).unwrap();
                    assert_eq!(
                        RationalRotation::from_rational(&RationalTransform::from_grid(&mirrored)),
                        None
                    );
                }
            }
        }
    }
    assert_eq!(rotations, 144);
    assert_eq!(
        RationalRotation::from_rational(&RationalTransform::from_grid(&identity)),
        Some(RationalRotation::IDENTITY)
    );
    // Not rotations: a translation, a scale, a shear, a singular map.
    let moved = RationalTransform::translation([1, 0, 0], 3).unwrap();
    let scaled = RationalTransform::uniform_scale(Scale::new(3, 2).unwrap()).unwrap();
    let shear = RationalTransform::new([[1, 1, 0], [0, 1, 0], [0, 0, 1]], [0; 3], 1).unwrap();
    let flat = RationalTransform::new([[1, 0, 0], [0, 1, 0], [0, 0, 0]], [0; 3], 1).unwrap();
    let turned_and_moved =
        RationalTransform::new([[3, -4, 0], [4, 3, 0], [0, 0, 5]], [5, 0, 0], 5).unwrap();
    for t in [moved, scaled, shear, flat, turned_and_moved] {
        assert_eq!(RationalRotation::from_rational(&t), None);
    }
    // Rotations whose largest diagonal entry is each of the four.
    for q in [
        [5, 1, 2, 3],
        [1, 5, 2, 3],
        [1, 2, 5, 3],
        [1, 2, 3, 5],
        [0, 1, 1, 0],
        [0, 0, 1, 1],
    ] {
        let r = RationalRotation::new(q).unwrap();
        assert_eq!(
            RationalRotation::from_rational(&r.to_rational().unwrap()),
            Some(r)
        );
    }
}

#[test]
fn floating_point_form_rounds_each_entry() {
    let mut mix = Mix(0x6460_0004);
    for bits in [3, 17, 36] {
        for _ in 0..100 {
            let Ok(r) = RationalRotation::new(random_q(&mut mix, bits)) else {
                continue;
            };
            let t = r.to_transform().unwrap();
            let rot = rotation_of(r.quaternion());
            for (i, row) in t.rows().iter().enumerate() {
                for j in 0..3 {
                    assert!(is_nearest_double(row[j], &rot[i][j]));
                }
                assert_eq!(row[3], 0.0);
            }
            assert_eq!(TypedTransform::Rotation(r).to_transform().unwrap(), t);
        }
    }
}

/// Directions for the 2D search: random angles, integer directions, the octants' edges, axes,
/// tiny, subnormal and huge components.
fn directions(mix: &mut Mix) -> Vec<[f64; 2]> {
    let mut out = vec![
        [1.0, 0.0],
        [0.0, 1.0],
        [-1.0, 0.0],
        [0.0, -1.0],
        [1.0, 1.0],
        [-1.0, 1.0],
        [-1.0, -1.0],
        [1.0, -1.0],
        [3.0, 4.0],
        [1.0, 2.0],
        [-7.0, 3.0],
        [1.0, 1e-300],
        [1.0, -1e-300],
        [5e-324, 1.0],
        [1e308, 1e308],
        [-1e308, 1.5e308],
        [1e-310, 3e-310],
        [1.0, 1.0 + f64::EPSILON],
        [f64::MAX, -f64::MAX],
        [1.0, 2f64.powi(-35)],
        [1.0, 1.5 * 2f64.powi(-35)],
        [1.0, 2f64.powi(-33)],
    ];
    for _ in 0..300 {
        let (x, y) = (mix.signed(), mix.signed());
        let scale = 2f64.powi(mix.below(200) as i32 - 100);
        out.push([x * scale, y * scale]);
    }
    for _ in 0..100 {
        let (x, y) = (
            (mix.next() >> 33) as i64 - (1 << 30),
            (mix.next() >> 33) as i64,
        );
        out.push([x as f64, y as f64]);
    }
    out
}

#[test]
fn plane_rotations_are_certified_and_the_simplest() {
    let mut mix = Mix(0x6460_0005);
    let dirs = directions(&mut mix);
    let bounds = [
        eps(),
        2f64.powi(-30),
        2f64.powi(-20),
        2f64.powi(-12),
        0.01,
        0.1,
        0.3,
        BOUND_MAX,
    ];
    let mut simplest = 0;
    let mut largest = 0;
    for d in &dirs {
        for &bound in &bounds {
            let got =
                RationalRotation::plane_within(F2::new(d[0], d[1]).unwrap(), real(bound)).unwrap();
            let r = got.value();
            check_budget(&r);
            assert!(r.acts_in_plane());
            assert!(got.angle().get() <= bound);
            let p = r.direction().unwrap();
            check_direction(*d, &p, got.angle().get());
            let pw = Pythagorean::within(F2::new(d[0], d[1]).unwrap(), real(bound)).unwrap();
            assert_eq!((pw.value(), pw.angle()), (p, got.angle()));
            if bound >= 2f64.powi(-12) {
                simplest += usize::from(check_simplest(*d, &r, bound, 40));
            }
            if bound == eps() {
                largest = largest.max(r.norm2());
            }
        }
    }
    assert!(simplest > 800, "{simplest} brute-force checks");
    // At ε_ang the numbers stay below the budget, with room.
    assert!(largest < 1 << 72 && largest > 1 << 30, "{largest}");

    // Directions that are Pythagorean already come back exactly (the angle certified tiny).
    for (a, b, c) in [
        (3, 4, 5),
        (-5, 12, 13),
        (8, -15, 17),
        (-20, -21, 29),
        (0, -9, 9),
    ] {
        let got = Pythagorean::within(F2::new(a as f64, b as f64).unwrap(), real(eps())).unwrap();
        assert_eq!(got.value(), Pythagorean::new(a, b, c).unwrap());
        assert!(got.angle().get() < 2f64.powi(-40));
    }
    // At a coarse bound, a small direction: within 0.3 rad of (1, 0.2), (1, 0) itself.
    let got = Pythagorean::within(F2::new(1.0, 0.2).unwrap(), real(0.3)).unwrap();
    assert_eq!(got.value(), Pythagorean::X);
    // The smallest denominator near a tiny angle: 1.5·2^−34 is not within 2^−34 of 0, so the
    // search must go as far as about 2^34.
    let got =
        RationalRotation::plane_within(F2::new(1.0, 1.5 * eps()).unwrap(), real(eps())).unwrap();
    let [w, _, _, z] = got.value().quaternion();
    assert!(z == 1 && w > 1 << 33, "{w} {z}");
}

/// Quaternion targets for the 3D search.
fn targets(mix: &mut Mix) -> Vec<[f64; 4]> {
    let mut out = vec![
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, -1.0],
        [1.0, 1.0, 1.0, 1.0],
        [3.0, 1.0, 1.0, 1.0],
        [1.0, 1e-300, -1e-300, 0.0],
        [1e300, 1e300, -3e299, 5e-324],
        [5e-324, 0.0, 0.0, 1e-320],
        [f64::MAX, f64::MAX, f64::MAX, -f64::MAX],
        [-1.0, 2.0, -3.0, 4.0],
        [0.6, 0.0, 0.8, 0.0],
    ];
    for _ in 0..300 {
        let scale = 2f64.powi(mix.below(400) as i32 - 200);
        let mut t = [mix.signed(), mix.signed(), mix.signed(), mix.signed()];
        if mix.below(5) == 0 {
            t[mix.below(4) as usize] = 0.0;
        }
        if mix.below(7) == 0 {
            t[mix.below(4) as usize] *= 2f64.powi(-60);
        }
        out.push(t.map(|v| v * scale));
    }
    out
}

#[test]
fn space_rotations_are_certified() {
    let mut mix = Mix(0x6460_0006);
    let bounds = [eps(), 2f64.powi(-25), 2f64.powi(-10), 0.1, BOUND_MAX];
    let mut largest = 0;
    for t in targets(&mut mix) {
        if t.iter().all(|&v| v == 0.0) {
            continue;
        }
        let exact_t = t.map(exact);
        for &bound in &bounds {
            let got = RationalRotation::within(t, real(bound)).unwrap();
            let r = got.value();
            check_budget(&r);
            assert!(got.angle().get() <= bound);
            check_quaternion(r.quaternion(), &exact_t, got.angle().get());
            if bound == eps() {
                largest = largest.max(r.norm2());
            }
        }
    }
    assert!(largest < 9 << 68, "{largest}");
    // The identity and a half turn are found at once.
    let id = RationalRotation::within([2.0, 0.0, 0.0, 0.0], real(eps())).unwrap();
    assert_eq!(id.value(), RationalRotation::IDENTITY);
    let half = RationalRotation::within([0.0, -0.5, 0.0, 0.0], real(eps())).unwrap();
    assert_eq!(half.value().quaternion(), [0, 1, 0, 0]);
}

#[test]
fn composition_is_exact_while_it_fits() {
    let q = RationalRotation::new([2, 0, 0, 1]).unwrap();
    let qq = q.compose(&q, real(eps())).unwrap();
    assert!(qq.is_exact());
    assert_eq!(qq.value().quaternion(), [3, 0, 0, 4]);
    assert_eq!(
        qq.value().direction(),
        Some(Pythagorean::new(-7, 24, 25).unwrap())
    );

    let mut mix = Mix(0x6460_0007);
    for bits in [3, 9, 17] {
        for _ in 0..200 {
            let (Ok(a), Ok(b)) = (
                RationalRotation::new(random_q(&mut mix, bits)),
                RationalRotation::new(random_q(&mut mix, bits)),
            ) else {
                continue;
            };
            let ab = a.compose(&b, real(eps())).unwrap();
            assert!(ab.is_exact());
            assert_eq!(a.compose_exact(&b).unwrap(), Some(ab.value()));
            let want = mat_mul(&rotation_of(a.quaternion()), &rotation_of(b.quaternion()));
            assert_eq!(rotation_of(ab.value().quaternion()), want);
            // As typed transforms, the same.
            let typed = TypedTransform::from(a)
                .compose(&TypedTransform::from(b))
                .unwrap();
            assert_eq!(typed, TypedTransform::from(ab.value()));
        }
    }
}

#[test]
fn composition_outgrowing_the_budget_is_reapproximated_once() {
    let mut mix = Mix(0x6460_0008);
    let ts = targets(&mut mix);
    let mut reapproximated = 0;
    for pair in ts.chunks(2).take(120) {
        let (Ok(a), Ok(b)) = (
            RationalRotation::within(pair[0], real(eps())),
            RationalRotation::within(pair[1], real(eps())),
        ) else {
            continue;
        };
        let (a, b) = (a.value(), b.value());
        for &bound in &[eps(), 2f64.powi(-20), 0.25] {
            let got = a.compose(&b, real(bound)).unwrap();
            let r = got.value();
            check_budget(&r);
            assert!(got.angle().get() <= bound);
            match a.compose_exact(&b).unwrap() {
                Some(exact_r) => {
                    assert!(got.is_exact());
                    assert_eq!(exact_r, r);
                }
                None => {
                    reapproximated += 1;
                    // The exact product, in rationals, is the target.
                    let (pa, pb) = (a.quaternion(), b.quaternion());
                    let [a1, b1, c1, d1] = pa.map(|v| int(i128::from(v)));
                    let [a2, b2, c2, d2] = pb.map(|v| int(i128::from(v)));
                    let p = [
                        &a1 * &a2 - &b1 * &b2 - &c1 * &c2 - &d1 * &d2,
                        &a1 * &b2 + &b1 * &a2 + &c1 * &d2 - &d1 * &c2,
                        &a1 * &c2 - &b1 * &d2 + &c1 * &a2 + &d1 * &b2,
                        &a1 * &d2 + &b1 * &c2 - &c1 * &b2 + &d1 * &a2,
                    ];
                    check_quaternion(r.quaternion(), &p, got.angle().get());
                    // As typed transforms, a product outgrowing the budget becomes certified.
                    let typed = TypedTransform::Rotation(a)
                        .compose(&TypedTransform::Rotation(b))
                        .unwrap();
                    assert!(matches!(typed, TypedTransform::Certified(_)));
                }
            }
        }
    }
    assert!(reapproximated > 100, "{reapproximated}");
}

#[test]
fn the_typed_transform_rotation_variant() {
    let big = RationalRotation::new([123_456_789, 2_345_678, -34_567, 4_567_890_123]).unwrap();
    let typed = TypedTransform::from(big);
    assert_eq!(typed, TypedTransform::Rotation(big));
    assert!(!typed.is_exact());
    assert!(!typed.acts_in_plane());
    let c = typed.classify().unwrap();
    assert_eq!((c.kind, c.mirror), (TransformKind::Rigid, false));
    assert_eq!(
        TypedTransform::Rotation(RationalRotation::IDENTITY)
            .classify()
            .unwrap()
            .kind,
        TransformKind::Identity
    );
    assert_eq!(
        typed.inverse().unwrap(),
        TypedTransform::Rotation(big.inverse())
    );
    assert_eq!(
        TypedTransform::Rotation(RationalRotation::new([1, 0, 0, 1]).unwrap()).simplified(),
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1))
    );
    let plane = RationalRotation::new([123_456_789, 0, 0, 987_654_321]).unwrap();
    let tp = TypedTransform::from(plane);
    assert!(tp.acts_in_plane());
    let p = P2::new(1_000_000, -7).unwrap();
    assert_eq!(tp.apply_2d(p), plane.apply_2d(p));
    assert_eq!(typed.apply_2d(p), Err(MathError::NotInPlane));
    assert_eq!(tp.to_transform().unwrap(), plane.to_transform().unwrap());

    // Composition with the other variants: exact rotations stay rotations, exactly.
    let quarter = TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1));
    let pyth = TypedTransform::rotation(Axis::Y, Rotation::direction(3, 4).unwrap()).unwrap();
    assert!(matches!(pyth, TypedTransform::Rational(_)));
    for other in [quarter, pyth] {
        let other_r = match other {
            TypedTransform::Exact(g) => {
                RationalRotation::from_rational(&RationalTransform::from_grid(&g)).unwrap()
            }
            TypedTransform::Rational(t) => RationalRotation::from_rational(&t).unwrap(),
            _ => unreachable!(),
        };
        let want = TypedTransform::from(big.compose_exact(&other_r).unwrap().unwrap());
        assert_eq!(typed.compose(&other).unwrap(), want);
        let want = TypedTransform::from(other_r.compose_exact(&big).unwrap().unwrap());
        assert_eq!(other.compose(&typed).unwrap(), want);
    }
    // With a translation, a scale or a certified transform: certified.
    let moved = TypedTransform::from(RationalTransform::translation([1, 2, 3], 1).unwrap());
    let scaled =
        TypedTransform::from(RationalTransform::uniform_scale(Scale::new(2, 1).unwrap()).unwrap());
    let general = TypedTransform::from(Transform::uniform_scale(1.5).unwrap());
    for other in [moved, scaled, general] {
        let got = typed.compose(&other).unwrap();
        assert!(matches!(got, TypedTransform::Certified(_)), "{other:?}");
        let want = typed
            .to_transform()
            .unwrap()
            .compose(&other.to_transform().unwrap())
            .unwrap();
        assert_eq!(got, TypedTransform::from(want));
        assert!(matches!(
            other.compose(&typed).unwrap(),
            TypedTransform::Certified(_)
        ));
    }
    // Two rational rotations stay on the rational path, as before.
    let both = pyth.compose(&pyth).unwrap();
    assert!(matches!(both, TypedTransform::Rational(_)));
    // A rotation composed with its inverse is the identity, exactly.
    assert_eq!(
        typed.compose(&typed.inverse().unwrap()).unwrap(),
        TypedTransform::IDENTITY
    );
}

#[test]
fn refusals() {
    let d = F2::new(1.0, 2.0).unwrap();
    let t = [1.0, 2.0, 3.0, 4.0];
    let r = RationalRotation::new([1, 2, 3, 4]).unwrap();
    let too_tight = real(eps().next_down());
    let too_loose = real(BOUND_MAX.next_up());
    for b in [too_tight, too_loose, real(0.0), real(1.0)] {
        let refused = MathError::Num(NumError::ToleranceExceeded);
        assert_eq!(RationalRotation::plane_within(d, b), Err(refused));
        assert_eq!(Pythagorean::within(d, b), Err(refused));
        assert_eq!(RationalRotation::within(t, b), Err(refused));
        assert_eq!(r.compose(&r, b), Err(refused));
    }
    for b in [real(eps()), real(BOUND_MAX)] {
        assert!(RationalRotation::plane_within(d, b).is_ok());
        assert!(RationalRotation::within(t, b).is_ok());
        assert!(r.compose(&r, b).is_ok());
    }
    let zero = F2::new(0.0, -0.0).unwrap();
    assert_eq!(
        RationalRotation::plane_within(zero, real(eps())),
        Err(MathError::Num(NumError::Degenerate))
    );
    assert_eq!(
        RationalRotation::within([0.0, -0.0, 0.0, 0.0], real(eps())),
        Err(MathError::Num(NumError::Degenerate))
    );
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            RationalRotation::within([1.0, bad, 0.0, 0.0], real(eps())),
            Err(MathError::Num(NumError::NotFinite))
        );
    }
}

proptest! {
    #![proptest_config(config(512))]

    /// Any finite target and any allowed bound: a result within the bound, certified, and never
    /// a defect.
    #[test]
    fn any_target_is_certified(
        bits in proptest::array::uniform4(any::<u64>()),
        expo in -1074i32..1024,
        bound_bits in 0u64..(1 << 52),
        log2 in -34i32..0,
    ) {
        let t = bits.map(|b| {
            let v = f64::from_bits(b & !(0x7ff << 52) | (0x3ff << 52)) - 1.5;
            let k = expo.clamp(-1022, 1023);
            v * 2f64.powi(k)
        });
        let bound = (f64::from_bits((0x3ff << 52) | bound_bits) * 2f64.powi(log2)).clamp(eps(), BOUND_MAX);
        match RationalRotation::within(t, real(bound)) {
            Ok(got) => {
                check_budget(&got.value());
                prop_assert!(got.angle().get() <= bound);
                check_quaternion(got.value().quaternion(), &t.map(exact), got.angle().get());
            }
            Err(e) => prop_assert_eq!(e, MathError::Num(NumError::Degenerate)),
        }
        let d = F2::new(t[0], t[1]).unwrap();
        match Pythagorean::within(d, real(bound)) {
            Ok(got) => {
                prop_assert!(got.angle().get() <= bound);
                check_direction([t[0], t[1]], &got.value(), got.angle().get());
            }
            Err(e) => prop_assert_eq!(e, MathError::Num(NumError::Degenerate)),
        }
    }
}
