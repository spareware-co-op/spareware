//! Fuzz harnesses for every public operation of `geom-math`, driven here as property tests over
//! arbitrary bit patterns (chapter 28: a harness is a plain function; bolero or cargo-fuzz drive
//! the same functions from `geom-fuzz` once that crate exists).
//!
//! Every double is built from 64 arbitrary bits, so NaN, both infinities, both zeros, subnormals
//! and the largest values all occur; every integer is arbitrary within its type. What each
//! harness proves, for every input:
//!
//! - no panic (the debug build also checks every integer operation for overflow);
//! - every `Ok` floating-point result is finite and every double-double normalized;
//! - every error is one the operation documents, and none is `BudgetExceeded`, which only a defect
//!   can cause, never an input (`error.rs`), even where the harness discards the result;
//! - every rounded grid result is inside the frame's range (by its type) and every classification
//!   is consistent (a `Degenerate` transform is never a mirror and has no inverse).

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::unreadable_literal,
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

use common::config;
use geom_math::{
    Axis, Box3, CrsId, DD, F2, F3, FBox3, FrameRecord, GridTransform, MapPoint, MathError,
    RationalTransform, Rotation, Scale, Transform, TransformKind, TypedTransform, V3,
};
use geom_num::NumError;
use geom_num::grid::{COORD_MAX, GridStep, P3};
use proptest::prelude::*;

fn f(bits: u64) -> f64 {
    f64::from_bits(bits)
}

/// The value of a result, `None` for an error, and a failed test for `BudgetExceeded`: that error
/// reports a defect (a broken bit budget), so no input may produce it.
fn no_defect<T, E: Into<MathError>>(r: Result<T, E>) -> Option<T> {
    match r.map_err(Into::into) {
        Ok(v) => Some(v),
        Err(MathError::Num(NumError::BudgetExceeded)) => panic!("a defect was reported"),
        Err(_) => None,
    }
}

fn dd_ok(r: Result<DD, NumError>, allowed: &[NumError]) {
    match r {
        Ok(d) => {
            assert!(d.hi().is_finite() && d.lo().is_finite(), "{d:?}");
            assert_eq!(d.hi() + d.lo(), d.hi(), "not normalized: {d:?}");
        }
        Err(e) => assert!(allowed.contains(&e), "undocumented error {e:?}"),
    }
}

/// Double-doubles from arbitrary bits.
fn dd_harness(bits: [u64; 5], offset: i128) {
    let Ok(a) = DD::from_sum(f(bits[0]), f(bits[1])) else {
        return;
    };
    let b = DD::from_sum(f(bits[2]), f(bits[3])).unwrap_or(DD::ZERO);
    let y = f(bits[4]);
    let nf = [NumError::NotFinite];
    dd_ok(a.add(b), &nf);
    dd_ok(a.sub(b), &nf);
    dd_ok(a.add_f64(y), &nf);
    dd_ok(a.mul_f64(y), &nf);
    dd_ok(a.mul(b), &nf);
    dd_ok(a.div(b), &[NumError::NotFinite, NumError::Degenerate]);
    dd_ok(a.sqrt(), &[NumError::NegativeSqrt]);
    let _ = a.round_shifted(offset);
    let _ = DD::exact_product(f(bits[0]), y);
    let _ = DD::from_i128(offset);
}

fn finite2(v: F2) -> bool {
    v.x().is_finite() && v.y().is_finite()
}

/// Float vectors from arbitrary bits.
fn float_harness(bits: [u64; 7]) {
    let (Ok(a), Ok(b)) = (
        F2::new(f(bits[0]), f(bits[1])),
        F2::new(f(bits[2]), f(bits[3])),
    ) else {
        return;
    };
    for r in [a.add(b), a.sub(b), a.scale(f(bits[4])), a.normalized()]
        .into_iter()
        .filter_map(no_defect)
    {
        assert!(finite2(r));
    }
    for r in [a.dot(b), a.cross(b), a.norm2(), a.length()]
        .into_iter()
        .filter_map(no_defect)
    {
        assert!(r.is_finite());
    }
    if a.x() != 0.0 || a.y() != 0.0 {
        assert_ne!(a.normalized(), Err(NumError::Degenerate));
    }
    let Ok(c) = F3::new(f(bits[4]), f(bits[5]), f(bits[6])) else {
        return;
    };
    let d = F3::new(a.x(), a.y(), b.x()).unwrap();
    for r in [
        c.add(d),
        c.sub(d),
        c.cross(d),
        c.scale(a.x()),
        c.normalized(),
    ]
    .into_iter()
    .filter_map(no_defect)
    {
        assert!(r.to_array().iter().all(|v| v.is_finite()));
    }
    for r in [c.dot(d), c.norm2(), c.length()]
        .into_iter()
        .filter_map(no_defect)
    {
        assert!(r.is_finite());
    }
    if let Ok(bx) = FBox3::new(c, c) {
        no_defect(bx.to_grid_outward());
    }
}

/// General transforms from arbitrary bits.
fn transform_harness(bits: [u64; 12], p: [i64; 3]) {
    let rows: [[f64; 4]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| f(bits[4 * i + j])));
    let Ok(t) = Transform::from_rows(rows) else {
        return;
    };
    let cl = t.classify().unwrap();
    if cl.kind == TransformKind::Degenerate {
        assert!(!cl.mirror);
        assert_eq!(t.inverse(), Err(MathError::Singular));
    } else {
        match t.inverse() {
            Ok(inv) => assert!(inv.rows().iter().flatten().all(|v| v.is_finite())),
            Err(e) => assert_eq!(e, MathError::Num(NumError::NotFinite)),
        }
    }
    let q = P3::new(p[0], p[1], p[2]).unwrap();
    match t.apply_grid(q) {
        Ok(_) | Err(NumError::OutOfRange { .. }) => {}
        Err(e) => panic!("undocumented error {e:?}"),
    }
    if let Ok(img) = t.apply_point(F3::from(q)) {
        assert!(img.to_array().iter().all(|v| v.is_finite()));
    }
    if let Ok(tt) = t.compose(&t) {
        assert!(tt.rows().iter().flatten().all(|v| v.is_finite()));
    }
    let bx = FBox3::from_box3(&Box3::from_point(q));
    if let Ok(b) = bx.transformed(&t) {
        assert!(
            b.lo()
                .to_array()
                .iter()
                .chain(b.hi().to_array().iter())
                .all(|v| v.is_finite())
        );
    }
    let typed = TypedTransform::from(t);
    no_defect(typed.apply(q));
    no_defect(typed.inverse());
    let _ = typed.classify().unwrap();
}

/// Rational transforms from arbitrary integers.
fn rational_harness(m: [[i64; 3]; 3], t: [i128; 3], d: i64, p: [i64; 3], v: [i64; 3]) {
    let Ok(r) = RationalTransform::new(m, t, d) else {
        return;
    };
    let q = P3::new(p[0], p[1], p[2]).unwrap();
    match r.apply(q) {
        Ok(_) | Err(NumError::OutOfRange { .. }) => {}
        Err(e) => panic!("undocumented error {e:?}"),
    }
    if let Ok(w) = V3::new(v[0], v[1], v[2]) {
        no_defect(r.apply_vector(w));
    }
    no_defect(r.compose(&r));
    match r.inverse() {
        Ok(inv) => {
            if let Ok(id) = inv.compose(&r) {
                assert_eq!(id, RationalTransform::IDENTITY);
            }
        }
        Err(MathError::Singular) => {
            assert_eq!(r.classify().unwrap().kind, TransformKind::Degenerate);
        }
        Err(MathError::Num(NumError::OutOfRange { .. })) => {}
        Err(e) => panic!("undocumented error {e:?}"),
    }
    let cl = r.classify().unwrap();
    assert!(!(cl.kind == TransformKind::Degenerate && cl.mirror));
    assert!(
        r.to_transform()
            .unwrap()
            .rows()
            .iter()
            .flatten()
            .all(|v| v.is_finite())
    );
    let _ = r.to_grid();
    let typed = TypedTransform::from(r);
    no_defect(
        typed.compose(&TypedTransform::Exact(GridTransform::quarter_turns(
            Axis::Z,
            1,
        ))),
    );
}

fn step(k: u8) -> GridStep {
    match k % 3 {
        0 => GridStep::Q0,
        1 => GridStep::Q1,
        _ => GridStep::Q2,
    }
}

/// Frames from arbitrary integers and map points from arbitrary bits.
#[allow(
    clippy::too_many_arguments,
    reason = "a harness takes every input separately"
)]
fn frame_harness(
    origin: [i64; 3],
    dir: (i32, i32),
    scale: (u32, u32),
    steps: (u8, u8),
    map: [u64; 3],
    p: [i64; 3],
    index: [i64; 2],
) {
    let (Ok(rot), Ok(k)) = (
        Rotation::direction(dir.0, dir.1),
        Scale::new(scale.0, scale.1),
    ) else {
        return;
    };
    let Ok(fr) = FrameRecord::new(CrsId::Epsg(2193), step(steps.0), origin, rot, k) else {
        return;
    };
    let m = MapPoint {
        easting: f(map[0]),
        northing: f(map[1]),
        height: f(map[2]),
    };
    match fr.to_frame(m) {
        Ok(_) | Err(NumError::OutOfRange { .. } | NumError::NotFinite) => {}
        Err(e) => panic!("undocumented error {e:?}"),
    }
    let q = P3::new(p[0], p[1], p[2]).unwrap();
    match fr.to_map(q) {
        Ok(r) => {
            let v = r.value();
            assert!(v.easting.is_finite() && v.northing.is_finite() && v.height.is_finite());
            // The map point converts back without a panic, and at q0 near the origin exactly.
            no_defect(fr.to_frame(v));
        }
        Err(e) => panic!("to_map failed: {e:?}"),
    }
    let other = FrameRecord::new(CrsId::Epsg(2193), step(steps.1), origin, rot, k);
    if let Some(shift) = other.ok().and_then(|other| no_defect(fr.shift_to(&other))) {
        no_defect(shift.apply(q));
    }
    if let Ok(cell) = FrameRecord::lattice(CrsId::Epsg(2193), step(steps.0), origin, index) {
        let bx = Box3::from_point(q);
        match cell.home_frame(&bx) {
            Ok((home, shift)) => {
                assert!(home.owns(shift.apply(q).unwrap()));
            }
            Err(MathError::Num(NumError::OutOfRange { .. })) => {}
            Err(e) => panic!("undocumented error {e:?}"),
        }
    }
}

fn coord() -> BoxedStrategy<i64> {
    prop_oneof![
        -COORD_MAX..=COORD_MAX,
        Just(COORD_MAX),
        Just(-COORD_MAX),
        -3i64..=3
    ]
    .boxed()
}

fn wide_i64() -> BoxedStrategy<i64> {
    prop_oneof![any::<i64>(), -10i64..=10, Just(i64::MIN), Just(i64::MAX)].boxed()
}

proptest! {
    #![proptest_config(config(4096))]

    #[test]
    fn fuzz_double_double(bits in any::<[u64; 5]>(), offset in any::<i128>()) {
        dd_harness(bits, offset);
    }

    #[test]
    fn fuzz_float_vectors(bits in any::<[u64; 7]>()) {
        float_harness(bits);
    }

    #[test]
    fn fuzz_transforms(bits in any::<[u64; 12]>(), p in prop::array::uniform3(coord())) {
        transform_harness(bits, p);
    }

    #[test]
    fn fuzz_transforms_of_tame_entries(
        e in prop::array::uniform12(prop_oneof![
            Just(0u64),
            Just(1.0f64.to_bits()),
            Just((-1.0f64).to_bits()),
            Just(0.5f64.to_bits()),
            any::<u64>()
        ]),
        p in prop::array::uniform3(coord()),
    ) {
        transform_harness(e, p);
    }

    #[test]
    fn fuzz_rational_transforms(
        m in prop::array::uniform3(prop::array::uniform3(wide_i64())),
        t in prop::array::uniform3(prop_oneof![any::<i128>(), -10i128..=10]),
        d in wide_i64(),
        p in prop::array::uniform3(coord()),
        v in prop::array::uniform3(wide_i64()),
    ) {
        rational_harness(m, t, d, p, v);
    }

    #[test]
    fn fuzz_small_rational_transforms(
        m in prop::array::uniform3(prop::array::uniform3(-3i64..=3)),
        t in prop::array::uniform3(-5i128..=5),
        d in 1i64..=5,
        p in prop::array::uniform3(coord()),
    ) {
        rational_harness(m, t, d, p, [1, -1, 2]);
    }

    #[test]
    fn fuzz_frames(
        origin in prop::array::uniform3(prop_oneof![wide_i64(), -(1i64 << 60)..=(1i64 << 60)]),
        dir in (any::<i32>(), any::<i32>()),
        scale in (any::<u32>(), any::<u32>()),
        steps in (any::<u8>(), any::<u8>()),
        map in prop::array::uniform3(prop_oneof![
            any::<u64>(),
            (-10_000_000.0f64..10_000_000.0).prop_map(f64::to_bits)
        ]),
        p in prop::array::uniform3(coord()),
        index in prop::array::uniform2(prop_oneof![wide_i64(), -3i64..=3]),
    ) {
        frame_harness(origin, dir, scale, steps, map, p, index);
    }

    #[test]
    fn fuzz_typed_rotations(
        dir in (any::<i32>(), any::<i32>()),
        axis in 0usize..3,
        p in prop::array::uniform3(coord()),
    ) {
        let Ok(rot) = Rotation::direction(dir.0, dir.1) else { return Ok(()) };
        let t = TypedTransform::rotation(Axis::ALL[axis], rot).unwrap();
        let q = P3::new(p[0], p[1], p[2]).unwrap();
        no_defect(t.apply(q));
        match t.compose(&t) {
            Ok(tt) => prop_assert!(tt.classify().unwrap().kind != TransformKind::Degenerate),
            // A rational rotation by a large Pythagorean triple squares beyond the budget.
            Err(MathError::Num(NumError::OutOfRange { .. })) => {
                prop_assert!(matches!(t, TypedTransform::Rational(_)));
            }
            Err(e) => prop_assert!(false, "undocumented error {e:?}"),
        }
        let inv = t.inverse().unwrap();
        no_defect(inv.compose(&t).unwrap().apply(q));
    }
}
