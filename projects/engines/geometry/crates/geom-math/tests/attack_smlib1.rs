//! Attack tests from SMLib's confirmed defects, batch 1: `geom-math`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the exact or correctly rounded answer, or a defined
//! refusal, and never a panic, a NaN or a silently wrong value.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use geom_math::{Axis, Box2, Box3, F2, F3, FBox3, GridTransform, Transform};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, P2, P3};

fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn xyz(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// solidcore plan/upstream-issues/WR-159--smvector3d-unitize-turns-a-vector-with-an-overflowing.md
/// (b4de2149): a vector whose squared length overflows (a component of 1.34e154 or more) came out
/// as a silent zero vector. Ours must give the unit vector, or refuse; never a zero or a NaN.
#[test]
fn wr_159_overflowing_squared_length_still_normalises() {
    // The defect's own axis, then the edge of the squared-length overflow and far beyond it.
    let cases: [[f64; 3]; 6] = [
        [0.0, -1e300, 1.0],
        [1.34e154, 0.0, 0.0],
        [1e154, 1e154, 1e154],
        [1e160, -1e160, 0.0],
        [f64::MAX, f64::MAX, f64::MAX],
        [1e-300, 0.0, -1e-310],
    ];
    for [x, y, z] in cases {
        let v = F3::new(x, y, z).unwrap();
        let n = v.normalized().expect("a nonzero vector has a unit vector");
        for c in n.to_array() {
            assert!(c.is_finite(), "{x:e} {y:e} {z:e}: component {c}");
            assert!(c.abs() <= 1.0);
        }
        let len = n.length().unwrap();
        assert!((len - 1.0).abs() <= 4.0 * f64::EPSILON, "{x:e}: {len}");
        // Same direction: the signs are kept, and a zero component stays a zero.
        for (a, b) in v.to_array().into_iter().zip(n.to_array()) {
            assert!(a == 0.0 || a.signum() == b.signum());
            assert_eq!(a == 0.0, b == 0.0, "{x:e} {y:e} {z:e}");
        }
    }
    // The defect's own vector: (0, -1e300, 1) has the unit vector (0, -1, 1e-300).
    let n = F3::new(0.0, -1e300, 1.0).unwrap().normalized().unwrap();
    assert_eq!(n.x(), 0.0);
    assert_eq!(n.y(), -1.0);
    assert!(n.z() > 0.0 && n.z() < 1.0e-299);
    // The zero vector is refused, not turned into a default axis.
    assert_eq!(F3::ZERO.normalized(), Err(NumError::Degenerate));
    // The plane has the same overflow.
    let n = F2::new(1e200, -1e200).unwrap().normalized().unwrap();
    assert!((n.length().unwrap() - 1.0).abs() <= 4.0 * f64::EPSILON);
    assert!(n.x() > 0.0 && n.y() < 0.0);
    assert_eq!(F2::ZERO.normalized(), Err(NumError::Degenerate));
}

/// solidcore plan/upstream-issues/WR-5--smextent3d-circumscribe-reads-the-axis-map-backwards.md
/// (b4de2149): the box of a box under a signed axis permutation read the axis map the wrong way
/// round, right for swaps and flips and wrong for 3-cycles. The input is the defect's: the box
/// [1,2] x [3,5] x [-7,-6] under the cyclic permutation (y, z, x). Every permutation and every
/// sign flip is checked against the eight transformed corners.
#[test]
fn wr_5_box_under_every_signed_permutation_is_the_box_of_its_corners() {
    let b = Box3::new(p3(1, 3, -7), p3(2, 5, -6)).unwrap();
    let cycle = GridTransform::from_parts([Axis::Y, Axis::Z, Axis::X], [false; 3], [0; 3]).unwrap();
    // out = (y, z, x): the box becomes [3,5] x [-7,-6] x [1,2].
    let image = b.transformed(&cycle).unwrap();
    assert_eq!(xyz(image.min()), [3, -7, 1]);
    assert_eq!(xyz(image.max()), [5, -6, 2]);
    // The inverse cycle too, the one a backwards read of the map would give.
    let back = GridTransform::from_parts([Axis::Z, Axis::X, Axis::Y], [false; 3], [0; 3]).unwrap();
    let image = b.transformed(&back).unwrap();
    assert_eq!(xyz(image.min()), [-7, 1, 3]);
    assert_eq!(xyz(image.max()), [-6, 2, 5]);

    let corners: Vec<P3> = (0..8u8)
        .map(|c| {
            p3(
                if c & 1 == 0 { 1 } else { 2 },
                if c & 2 == 0 { 3 } else { 5 },
                if c & 4 == 0 { -7 } else { -6 },
            )
        })
        .collect();
    let perms = [
        [Axis::X, Axis::Y, Axis::Z],
        [Axis::X, Axis::Z, Axis::Y],
        [Axis::Y, Axis::X, Axis::Z],
        [Axis::Y, Axis::Z, Axis::X],
        [Axis::Z, Axis::X, Axis::Y],
        [Axis::Z, Axis::Y, Axis::X],
    ];
    let shift = [10, -20, 30];
    for perm in perms {
        for signs in 0..8u8 {
            let negate = [signs & 1 != 0, signs & 2 != 0, signs & 4 != 0];
            let t = GridTransform::from_parts(perm, negate, shift).unwrap();
            let image = b.transformed(&t).unwrap();
            // The oracle: the box of the eight corners, each moved on its own by the formula
            // out[i] = (+-)in[source[i]] + shift[i].
            let mut lo = [i64::MAX; 3];
            let mut hi = [i64::MIN; 3];
            for c in &corners {
                let v = xyz(*c);
                for i in 0..3 {
                    let s = v[usize::from(perm[i].index())];
                    let s = if negate[i] { -s } else { s } + shift[i];
                    lo[i] = lo[i].min(s);
                    hi[i] = hi[i].max(s);
                }
            }
            assert_eq!(xyz(image.min()), lo, "{perm:?} {negate:?}");
            assert_eq!(xyz(image.max()), hi, "{perm:?} {negate:?}");
            for c in &corners {
                assert!(image.contains(t.apply(*c).unwrap()));
            }
        }
    }
}

/// solidcore plan/upstream-issues/WR-5--smextent3d-circumscribe-reads-the-axis-map-backwards.md
/// (b4de2149), on the float box: the same 3-cycle as a general transform. The box of the image
/// holds the cycled intervals, not the backwards cycle's, and is no wider than they are.
#[test]
fn wr_5_float_box_under_a_three_cycle_holds_the_cycled_intervals() {
    let b = FBox3::new(
        F3::new(1.0, 3.0, -7.0).unwrap(),
        F3::new(2.0, 5.0, -6.0).unwrap(),
    )
    .unwrap();
    // Rows of out = (y, z, x).
    let cycle = Transform::from_rows([
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
    ])
    .unwrap();
    let image = b.transformed(&cycle).unwrap();
    let [x, y, z] = image.axes().unwrap();
    assert!(x.lo() <= 3.0 && x.hi() >= 5.0 && x.hi() - x.lo() < 2.0 + 1e-9);
    assert!(y.lo() <= -7.0 && y.hi() >= -6.0 && y.hi() - y.lo() < 1.0 + 1e-9);
    assert!(z.lo() <= 1.0 && z.hi() >= 2.0 && z.hi() - z.lo() < 1.0 + 1e-9);
}

/// solidcore plan/upstream-issues/UB-41--smpointset3d-gettightpbox-on-an-empty-planar-set-reads-m.md
/// (b4de2149): the box of an empty planar point set read the first point of nothing. Ours has no
/// box of no points, says so with `None`, and the box of one point or of a flat set is exact.
#[test]
fn ub_41_the_box_of_no_points_is_none_and_of_a_flat_set_is_flat() {
    assert_eq!(Box3::from_points(std::iter::empty()), None);
    assert_eq!(Box2::from_points(std::iter::empty()), None);
    // A planar set (all z equal): a box of zero height, holding every point and nothing above.
    let pts = [p3(0, 0, 7), p3(5, -3, 7), p3(-2, 9, 7)];
    let b = Box3::from_points(pts).unwrap();
    assert_eq!(xyz(b.min()), [-2, -3, 7]);
    assert_eq!(xyz(b.max()), [5, 9, 7]);
    assert_eq!(b.extent().unwrap().z(), 0);
    assert!(pts.iter().all(|&p| b.contains(p)));
    assert!(!b.contains(p3(0, 0, 8)));
    assert!(!b.contains(p3(0, 0, 6)));
    // One point is a box of one point.
    let one = Box3::from_points([p3(4, 4, 4)]).unwrap();
    assert_eq!(one.min(), one.max());
}

/// solidcore plan/upstream-issues/UB-35--mappointtovoxel-converts-negative-doubles-to-ulong.md
/// (b4de2149): a coordinate below the box was converted to an unsigned integer, so it wrapped (or
/// saturated, by machine), and a zero-length axis gave NaN or infinity for every point. Ours
/// turns a float box into grid steps by rounding outward, the same on every machine: below the
/// box is below, a zero-length axis is a box of zero or one step, and NaN or infinity is refused
/// when the point is made.
#[test]
fn ub_35_negative_and_degenerate_coordinates_round_outward_not_wrap() {
    let f = |a: [f64; 3], b: [f64; 3]| {
        FBox3::new(
            F3::new(a[0], a[1], a[2]).unwrap(),
            F3::new(b[0], b[1], b[2]).unwrap(),
        )
        .unwrap()
    };
    // Negative values round down at the low end and up at the high end, never to the top.
    let g = f([-0.25, -1.5, -3.0], [-0.25, -0.5, 0.0])
        .to_grid_outward()
        .unwrap();
    assert_eq!(xyz(g.min()), [-1, -2, -3]);
    assert_eq!(xyz(g.max()), [0, 0, 0]);
    // A point exactly on the grid is a box of one point (a zero-length axis stays zero long).
    let g = f([3.0, -4.0, 5.0], [3.0, -4.0, 5.0])
        .to_grid_outward()
        .unwrap();
    assert_eq!(g.min(), g.max());
    // A point strictly between two steps becomes the one step on each side.
    let g = FBox3::from_point(F3::new(0.5, -0.5, 2.5).unwrap())
        .to_grid_outward()
        .unwrap();
    assert_eq!(xyz(g.min()), [0, -1, 2]);
    assert_eq!(xyz(g.max()), [1, 0, 3]);
    // Below the frame is refused with the value, not wrapped into it; so is above.
    let low = f([-1e15, 0.0, 0.0], [0.0, 0.0, 0.0]).to_grid_outward();
    assert!(matches!(low, Err(NumError::OutOfRange { value }) if value == -1_000_000_000_000_000));
    let high = f([0.0, 0.0, 0.0], [1e40, 0.0, 0.0]).to_grid_outward();
    assert!(matches!(high, Err(NumError::OutOfRange { .. })));
    // The frame's own edge is accepted: -(2^40 - 1) is a coordinate, not an overflow.
    let edge = COORD_MAX as f64;
    let g = f([-edge, 0.0, 0.0], [edge, 0.0, 0.0])
        .to_grid_outward()
        .unwrap();
    assert_eq!(g.min().x.get(), -COORD_MAX);
    assert_eq!(g.max().x.get(), COORD_MAX);
    // NaN and infinity never make a point.
    assert_eq!(F3::new(f64::NAN, 0.0, 0.0), Err(NumError::NotFinite));
    assert_eq!(F3::new(0.0, f64::INFINITY, 0.0), Err(NumError::NotFinite));
    assert_eq!(
        F3::new(0.0, 0.0, f64::NEG_INFINITY),
        Err(NumError::NotFinite)
    );
    assert_eq!(F2::new(f64::NAN, 0.0), Err(NumError::NotFinite));
    // A point below a grid box is not inside it, and one above is not either.
    let b = Box2::new(P2::new(-5, -5).unwrap(), P2::new(5, 5).unwrap()).unwrap();
    assert!(!b.contains(P2::new(-6, 0).unwrap()));
    assert!(!b.contains(P2::new(0, 6).unwrap()));
    assert!(b.contains(P2::new(-5, 5).unwrap()));
}
