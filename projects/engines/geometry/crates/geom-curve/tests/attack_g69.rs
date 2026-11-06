//! What piece G69's law catalogue found in `geom-curve`, kept as permanent tests (chapter 28.16).
//!
//! - **Grid transforms are exact.** A circle on a frame whose unit vectors are irrational, and a
//!   bulge polyline, stated a rounding of about 10⁻¹⁴ under a quarter turn, a mirror and a
//!   whole-step translation, though each moves exactly (chapter 7: they "stay exact"). Now a grid
//!   transform that moves every stored point exactly states deviation 0, and one that rounds a
//!   point, or a transform that is not of the grid, still states its rounding.
//! - **An offset's cache tries narrower pieces.** Where the offset's derivatives could not be
//!   bounded over a piece of the coarsest cut, the conversion failed as a defect; now it tries
//!   narrower pieces, as a transition's does. (Where the offset folds, the cache it makes states a
//!   large deviation: honest, and checked against the definition by the catalogue.)
//! - **An offset is refused only where it is undefined (issue #526).** A piece still too wide at
//!   the finest even cut made the conversion refuse as `Unbounded`, the error of a ray or an
//!   infinite line, though the domain is finite: an offset of a nearly flat helix along its axis,
//!   whose normal comes within 0.08 of zero but never reaches it, was refused so. Now the pieces
//!   that need it are halved until they are bounded, and the conversion refuses only where the
//!   normal vanishes even on the narrowest piece, as `Degenerate`: a cusp of the basis, or a
//!   tangent parallel to the reference direction. The halving stops exactly where the module
//!   header says (2^−52 of the domain, or two neighbouring doubles), and runs within a stated
//!   budget of work, so a halving that never stopped fails rather than hangs.
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
use geom_curve::tight;
use geom_curve::trimmed::Trimmed;
use geom_curve::{
    Axes, Basis, BulgePolyline, Circle, CurveError, CurveKind, DerivativeBound, Ellipse, Frame,
    Helix, HelixAxes, Line, ParamCurve, Polynomial,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, TypedTransform};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The work an offset's conversion is given in the tests of its halving (issue #526): about
/// twice what the most costly of them takes (254 282, the offset turning within 2^−45), so a
/// halving that never stops is stopped by the meter and fails its test, rather than hanging.
const WITHIN: Budget = Budget::of_work(500_000);

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
fn an_offset_through_a_cusp_is_degenerate_even_on_the_finest_pieces() {
    // C(t) = (t², t³) stops at t = 0: its normal vanishes there, so every piece holding 0 is
    // unbounded at every width, and the conversion refuses as degenerate: the offset is undefined
    // at 0. Not as a defect, and not as unbounded, the error of a ray or an infinite line
    // (issue #526).
    let cusp = Polynomial::new(
        vec![[0.0, 0.0], [0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        (-1.0, 1.0),
    )
    .unwrap();
    let basis = Trimmed::new(Basis::Polynomial(cusp), (-1.0, 1.0)).unwrap();
    let o = Offset::new(basis, OffsetLaw::Planar { distance: 0.25 }).unwrap();
    let m = &mut WorkMeter::new(WITHIN);
    assert_eq!(o.to_nurbs(m).err(), Some(CurveError::Degenerate));
}

/// The helix the law catalogue built at its default seed once one more kind joined its corpus
/// (issue #526), every value as reported: u, v and w nearly coplanar, |det[u v w]| / (|u|·|v|·|w|)
/// about 0.0019, trimmed to (−1, 1).
fn nearly_flat_helix() -> (Helix<3>, [f64; 3]) {
    let w = [
        -1.048_737_586_795_064_6,
        -1.954_900_089_637_301,
        0.999_701_552_135_342_7,
    ];
    let h = Helix::new(
        [
            -2.813_873_460_013_553_5,
            16.629_285_119_325_345,
            38.949_651_919_611_03,
        ],
        HelixAxes::Vectors {
            u: [
                -4.003_738_123_194_152,
                -2.492_461_280_537_769_5,
                -3.205_897_544_708_239_6,
            ],
            v: [
                6.270_044_551_006_718,
                2.331_366_262_026_993,
                7.292_276_771_508_675,
            ],
            w,
        },
        (-3.0, 5.0),
    )
    .unwrap();
    (h, w)
}

#[test]
fn a_nearly_flat_helix_offset_along_its_axis_converts() {
    // C′(t) = π(−u·sin πt + v·cos πt) + w, so C′ × w = π(−u·sin πt + v·cos πt) × w: zero only if
    // w lies in the plane of u and v, which it does not (det[u v w] ≈ −0.265). The offset is
    // defined on the whole arc, but its normal comes within 0.0755 of zero (its largest length is
    // 84.5) near t ≈ −0.345 and t ≈ 0.655, and turns fast there.
    let m = &mut meter();
    let (helix, w) = nearly_flat_helix();
    let basis = Trimmed::new(Basis::Helix(helix), (-1.0, 1.0)).unwrap();
    let o = Offset::new(
        basis,
        OffsetLaw::Reference {
            distance: 1.346_043_035_094_070_2,
            direction: w,
        },
    )
    .unwrap();
    // The finest even cut, 128 pieces, cannot bound the offset's tenth derivative over some piece:
    // the case this test is for.
    let unbounded = (0..128)
        .filter(|&k| {
            let lo = -1.0 + 2.0 * f64::from(k) / 128.0;
            let hi = -1.0 + 2.0 * f64::from(k + 1) / 128.0;
            matches!(
                o.derivative_bound((lo, hi), 10, m),
                Ok(DerivativeBound::Unbounded)
            )
        })
        .count();
    assert!(unbounded > 0, "the finest even cut must fail somewhere");
    // Converted, within the conversion's aim of 10⁻¹⁰ of the offset's size, and within the
    // halving tests' budget (it takes 122 389): halving that ran away, cutting pieces already
    // within the aim, would need billions, and stops there rather than hanging.
    let n = o.to_nurbs(&mut WorkMeter::new(WITHIN)).unwrap();
    let b = o.bound((-1.0, 1.0), m).unwrap();
    let scale =
        b.lo.iter()
            .chain(&b.hi)
            .fold(1.0, |s, x| tight::larger(s, x.abs()));
    assert!(n.deviation <= scale * 1e-10, "deviation {}", n.deviation);
    // Against the offset's own certified enclosures (the contract's laws), densely, and at the
    // two places the normal comes nearest zero.
    let mut params: Vec<f64> = (0..=64).map(|k| -1.0 + 2.0 * f64::from(k) / 64.0).collect();
    params.extend([-0.345_115, 0.654_885]);
    assert_eq!(
        geom_curve::contract::check(&o, &params, &grid()[0], m),
        Ok(())
    );
}

#[test]
fn an_offset_whose_tangent_meets_its_reference_direction_is_degenerate() {
    // u = 2x̂, v = 2ŷ and w = ŷ: w lies in the plane of u and v, and C′ × w = (0, 0, −2π·sin πt)
    // vanishes at t = 0, where the tangent is parallel to w. The offset along w is undefined
    // there. On (−0.3, 0.55), 0 is no cut of any even cut, so only the halving finds it.
    let helix = Helix::new(
        [0.0; 3],
        HelixAxes::Vectors {
            u: [2.0, 0.0, 0.0],
            v: [0.0, 2.0, 0.0],
            w: [0.0, 1.0, 0.0],
        },
        (-1.0, 1.0),
    )
    .unwrap();
    let basis = Trimmed::new(Basis::Helix(helix), (-0.3, 0.55)).unwrap();
    let o = Offset::new(
        basis,
        OffsetLaw::Reference {
            distance: 0.5,
            direction: [0.0, 1.0, 0.0],
        },
    )
    .unwrap();
    let m = &mut WorkMeter::new(WITHIN);
    assert_eq!(o.to_nurbs(m).err(), Some(CurveError::Degenerate));
}

/// C(t) = (t²/2, t²/2 − δ·t) on (−1, 1), offset by 0.25: C′(t) = (t, t − δ) never vanishes, but
/// turns through a right angle between t = 0 and t = δ, where its length is about δ/√2.
fn turning_at(delta: f64) -> Offset<2> {
    let p = Polynomial::new(vec![[0.0, 0.0], [0.0, -delta], [0.5, 0.5]], (-1.0, 1.0)).unwrap();
    let basis = Trimmed::new(Basis::Polynomial(p), (-1.0, 1.0)).unwrap();
    Offset::new(basis, OffsetLaw::Planar { distance: 0.25 }).unwrap()
}

#[test]
fn an_offset_is_halved_down_to_the_narrowest_piece_and_no_further() {
    // The narrowest piece on (−1, 1) is 2·2^−52 = 2^−51 wide, [0, 2^−51] beside t = 0, and its
    // normal is bounded over each quarter of it, the first [0, 2^−53]. The normal there is certain
    // apart from zero only if that quarter ends before δ (the tangent's y is t − δ < 0 on it).
    // At δ = 2^−53 it does not: the normal cannot be told from zero, so the offset is degenerate.
    // Halving one step further, or below the narrowest width, would separate it.
    let m = &mut WorkMeter::new(WITHIN);
    assert_eq!(
        turning_at(f64::EPSILON / 2.0).to_nurbs(m).err(),
        Some(CurveError::Degenerate)
    );
    // At δ = 2^−50 the narrowest piece separates the normal from zero, so the offset is defined,
    // but it turns through a right angle within 2^−50, and the Taylor remainder over the
    // narrowest piece overflows the doubles: not finite, not degenerate.
    let m = &mut WorkMeter::new(WITHIN);
    assert_eq!(
        turning_at(f64::EPSILON * 4.0).to_nurbs(m).err(),
        Some(CurveError::NotFinite)
    );
    // At δ = 2^−45 it converts, within the conversion's aim, but only because a remainder that
    // overflows is halved too: the pieces that first separate the normal from zero there bound
    // it so loosely that the remainder leaves the doubles, and their halves bound it.
    let m = &mut WorkMeter::new(WITHIN);
    let n = turning_at(f64::EPSILON * 128.0).to_nurbs(m).unwrap();
    assert!(n.deviation <= 1e-9, "deviation {}", n.deviation);
}

/// A helix with u = 2x̂, v = 2ŷ and w = (a, b, 0) in their plane, about t = 1000, offset along w:
/// C′ × w = (0, 0, −2π·(a·cos πt + b·sin πt)) vanishes where tan πt = −a/b, which is no double.
fn flat_near_1000(a: f64, b: f64) -> Offset<3> {
    let helix = Helix::new(
        [0.0; 3],
        HelixAxes::Vectors {
            u: [2.0, 0.0, 0.0],
            v: [0.0, 2.0, 0.0],
            w: [a, b, 0.0],
        },
        (999.0, 1001.0),
    )
    .unwrap();
    let basis = Trimmed::new(Basis::Helix(helix), (999.7, 1000.55)).unwrap();
    Offset::new(
        basis,
        OffsetLaw::Reference {
            distance: 0.5,
            direction: [a, b, 0.0],
        },
    )
    .unwrap()
}

#[test]
fn an_offset_is_halved_down_to_neighbouring_doubles_and_no_further() {
    // Near t = 1000 the doubles are 2^−43 apart, far wider than the narrowest piece (0.85·2^−52),
    // so halving the piece that holds the vanishing normal stops at two neighbouring doubles,
    // whose middle rounds to one of them, the even one: the left end for some of these zeros and
    // the right end for others. Either way the offset is degenerate there, and the halving ends.
    for (a, b) in [(0.001, 1.0), (-0.001, 1.0), (0.003, 1.0), (-0.2, 1.0)] {
        let m = &mut WorkMeter::new(WITHIN);
        assert_eq!(
            flat_near_1000(a, b).to_nurbs(m).err(),
            Some(CurveError::Degenerate),
            "w = ({a}, {b}, 0)"
        );
    }
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
