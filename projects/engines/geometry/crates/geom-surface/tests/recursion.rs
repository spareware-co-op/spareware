//! The recursion bounds of `geom-surface`'s module headers, run at their bound (chapter 28.8; the
//! recursion check of piece G13): an offset nested `MAX_NESTING` deep, the deepest
//! `Offset::new` allows, is cloned, compared, printed and dropped, evaluated, bounded, reversed
//! and moved, and one more level is refused.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    reason = "tests: a failure stops the test, and the names and loops are the formulas'"
)]

use geom_curve::CurveError;
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_nurbs::{Basis, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::offset::MAX_NESTING;
use geom_surface::{Offset, ParamSurface, SurfaceKind};

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A gently curved bicubic patch with a knot in u, offset `MAX_NESTING` times by small
/// distances that never fold it.
fn deepest() -> SurfaceKind {
    let bu = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut pts = Vec::new();
    for i in 0..5_u8 {
        for j in 0..4_u8 {
            let (x, y) = (f64::from(i), f64::from(j));
            pts.push([x, y, 0.05 * x * y]);
        }
    }
    let mut k = SurfaceKind::Nurbs(Surface::new(bu, bv, &pts).unwrap());
    for _ in 0..MAX_NESTING {
        k = SurfaceKind::Offset(Offset::new(k, 0.05).unwrap());
    }
    k
}

#[test]
fn offsets_clone_compare_and_print_at_their_bound() {
    let k = deepest();
    let copy = k.clone();
    assert_eq!(copy, k);
    assert!(format!("{copy:?}").contains("Offset"));
    drop(copy);
    assert_eq!(
        Offset::new(k, 0.05),
        Err(CurveError::TooDeep {
            depth: MAX_NESTING + 1
        })
    );
}

#[test]
fn offsets_evaluate_at_their_bound() {
    let k = deepest();
    let m = &mut meter();
    let p = k
        .partials_over(((0.3, 0.3), (0.6, 0.6)), SIDES, 1, m)
        .unwrap();
    assert_eq!(p.len(), 3);
    let e = k.enclose(0.3, 0.6, SIDES, m).unwrap();
    assert_eq!(e[0], p[0]);
    assert_eq!(k.breaks(), (vec![0.5], vec![]));
}

#[test]
fn offsets_bound_at_their_bound() {
    let k = deepest();
    let m = &mut meter();
    let region = ((0.25, 0.375), (0.5, 0.625));
    let b = k.bound(region, m).unwrap();
    let p = k.point(0.3, 0.6, SIDES).unwrap();
    for d in 0..3 {
        assert!(b.lo[d] <= p[d] && p[d] <= b.hi[d]);
    }
    let n = k.normal_bound(region, m).unwrap();
    assert!(n.lo[2] > 0.0 || n.hi[2] < 0.0);
}

#[test]
fn offsets_reverse_and_move_at_their_bound() {
    let k = deepest();
    let m = &mut meter();
    let r = k.reversed(Direction::V, m).unwrap();
    assert_eq!(r.deviation, 0.0);
    let a = k.point(0.3, 0.6, SIDES).unwrap();
    let b = r.value.point(0.3, -0.6, SIDES).unwrap();
    for d in 0..3 {
        assert!((a[d] - b[d]).abs() < 1e-12);
    }
    let t = k
        .transformed(&TypedTransform::Exact(GridTransform::mirror(Axis::X)), m)
        .unwrap();
    assert_eq!(t.deviation, 0.0);
    assert!(matches!(t.value, SurfaceKind::Offset(_)));
}
