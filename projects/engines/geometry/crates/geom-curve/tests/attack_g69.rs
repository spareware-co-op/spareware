//! What piece G69's law catalogue found in `geom-curve`, kept as permanent tests (chapter 28.16).
//!
//! - **Grid transforms are exact.** A circle on a frame whose unit vectors are irrational, and a
//!   bulge polyline, stated a rounding of about 10⁻¹⁴ under a quarter turn, a mirror and a
//!   whole-step translation, though each moves exactly (chapter 7: they "stay exact"). Now a grid
//!   transform that moves every stored point exactly states deviation 0, and one that rounds a
//!   point, or a transform that is not of the grid, still states its rounding.
//! - **An offset's cache tries narrower pieces.** Where the offset's derivatives could not be
//!   bounded over a piece of the coarsest cut, the conversion failed as a defect; now it tries
//!   narrower pieces, as a transition's does, and a piece too wide even at the finest cut is
//!   `Unbounded`: an offset through a cusp of its basis, whose normal vanishes, refuses so. (Where
//!   the offset folds, the cache it makes states a large deviation: honest, and checked against
//!   the definition by the catalogue.)
//! - **The contract kit steps past huge ends.** Testing that a kind refuses outside its domain, the
//!   kit stepped one unit past each end, which beyond 2^53 in magnitude rounds back to the end
//!   itself, inside: `geom-fuzz`'s `curves` target found it reporting kinds that rightly accepted
//!   it. Now each step is at least one double.
#![allow(
    clippy::unwrap_used,
    clippy::float_cmp,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "tests: a failure stops the test, exactness is equality, and the names are the formulas'"
)]

use geom_curve::offset::{Offset, OffsetLaw};
use geom_curve::trimmed::Trimmed;
use geom_curve::{
    Axes, Basis, BulgePolyline, Circle, CurveError, CurveKind, DerivativeBound, Ellipse, Frame,
    Line, ParamCurve, Polynomial,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, TypedTransform};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn grid() -> [TypedTransform; 3] {
    [
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::X)),
        TypedTransform::Exact(GridTransform::translation([7, -3, 0]).unwrap()),
    ]
}

/// A circle on a frame whose unit vectors are irrational.
fn circle<const D: usize>(center: [f64; D]) -> CurveKind<D> {
    let mut x = [0.0; D];
    x[0] = 1.0;
    x[1] = 0.3;
    let n = if D == 2 {
        [0.0, 0.0, 1.0]
    } else {
        [0.1, -0.4, 1.0]
    };
    let f = Frame::new(x, n).unwrap();
    CurveKind::Circle(Circle::new(center, 2.5, f, (-0.75, 1.0)).unwrap())
}

fn polyline(vertices: Vec<[f64; 2]>) -> CurveKind<2> {
    CurveKind::Bulge(BulgePolyline::new(vertices, vec![0.0, 0.5, -1.25], None, false).unwrap())
}

#[test]
fn grid_transforms_of_circles_and_polylines_are_exact() {
    let m = &mut meter();
    let kinds2 = [
        circle([1.5, -2.25]),
        polyline(vec![[0.0, 0.0], [3.0, 1.0], [5.5, -2.0], [8.0, 0.5]]),
    ];
    for k in &kinds2 {
        for t in &grid() {
            let r = k.transformed(t, m).unwrap();
            assert_eq!(r.deviation, 0.0, "{k:?} under {t:?}");
            assert_eq!(
                core::mem::discriminant(&r.value),
                core::mem::discriminant(k)
            );
        }
    }
    let k3 = circle([1.5, -2.25, 3.0]);
    for t in &grid() {
        assert_eq!(k3.transformed(t, m).unwrap().deviation, 0.0, "{t:?}");
    }
}

#[test]
fn rounding_moves_still_state_their_rounding() {
    let m = &mut meter();
    // A whole-step translation that rounds the centre (0.1 + 1000 is not a double).
    let far = TypedTransform::Exact(GridTransform::translation([1000, 0, 0]).unwrap());
    assert!(circle([0.1, 0.2]).transformed(&far, m).unwrap().deviation > 0.0);
    let p = polyline(vec![[0.1, 0.0], [3.0, 1.0], [5.5, -2.0], [8.0, 0.5]]);
    assert!(p.transformed(&far, m).unwrap().deviation > 0.0);
    // An exact rotation that is not of the grid, whose image of the frame rounds, at a centre it
    // moves exactly (the origin).
    let r = TypedTransform::from(
        RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
    );
    assert!(circle([0.0, 0.0]).transformed(&r, m).unwrap().deviation > 0.0);
    let origin = polyline(vec![[0.0, 0.0], [3.0, 1.0], [5.5, -2.0], [8.0, 0.5]]);
    assert!(origin.transformed(&r, m).unwrap().deviation > 0.0);
}

/// A flat ellipse offset beyond its least radius of curvature: the offset folds, and over one
/// eighth of the domain its derivatives cannot be bounded.
fn folding_offset() -> Offset<2> {
    let e = Ellipse::new(
        [-21.160_140_449_706_6, -9.621_490_488_624_58],
        Axes::Vectors {
            u: [-7.379_298_424_563_187, -8.526_764_795_006_15],
            v: [5.467_652_701_224_047, 5.829_108_881_255_316],
        },
        (-1.0, 1.0),
    )
    .unwrap();
    let basis = Trimmed::new(Basis::Ellipse(e), (-0.5, 0.5)).unwrap();
    Offset::new(basis, OffsetLaw::Planar { distance: 1.25 }).unwrap()
}

#[test]
fn a_folding_offset_converts_on_narrower_pieces() {
    let m = &mut meter();
    let o = folding_offset();
    let (a, b) = ParamCurve::domain(&o);
    let unbounded = (0..8)
        .filter(|&k| {
            let lo = a + (b - a) * f64::from(k) / 8.0;
            let hi = a + (b - a) * f64::from(k + 1) / 8.0;
            matches!(
                o.derivative_bound((lo, hi), 10, m),
                Ok(DerivativeBound::Unbounded)
            )
        })
        .count();
    assert!(
        unbounded > 0,
        "the coarsest cut must fail somewhere for this to test anything"
    );
    // The cache folds with the offset, so its stated deviation is large, but stated: the law
    // catalogue checks it against the definition (geom-test's tests/laws_curves.rs).
    let n = o.to_nurbs(m).unwrap();
    assert!(
        n.deviation.is_finite() && n.deviation > 0.0,
        "{}",
        n.deviation
    );
}

#[test]
fn an_offset_through_a_cusp_is_unbounded_even_on_the_finest_pieces() {
    // C(t) = (t², t³) stops at t = 0: its normal vanishes there, so every piece holding 0 is
    // unbounded at every width, and the conversion refuses as unbounded, not as a defect.
    let cusp = Polynomial::new(
        vec![[0.0, 0.0], [0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        (-1.0, 1.0),
    )
    .unwrap();
    let basis = Trimmed::new(Basis::Polynomial(cusp), (-1.0, 1.0)).unwrap();
    let o = Offset::new(basis, OffsetLaw::Planar { distance: 0.25 }).unwrap();
    assert!(matches!(
        o.to_nurbs(&mut meter()),
        Err(CurveError::Unbounded)
    ));
}

#[test]
fn the_contract_kit_steps_past_huge_domain_ends() {
    let m = &mut meter();
    let huge = 1_152_921_504_606_846_976.0; // 2^60
    let t = grid()[0];
    for domain in [(-huge, 0.0), (0.0, huge), (-huge, huge)] {
        let line = Line::new([0.0, 0.0], [1.0, 2.0], domain).unwrap();
        let (a, b) = domain;
        let params = [a, a / 2.0, b / 2.0, b];
        assert_eq!(
            geom_curve::contract::check(&line, &params, &t, m),
            Ok(()),
            "{domain:?}"
        );
    }
}
