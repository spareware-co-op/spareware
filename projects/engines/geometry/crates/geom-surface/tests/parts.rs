//! What a stored surface is rebuilt from (piece G94): an affine image from its terms, a swept
//! surface from its shape and placement, a ruled surface's σ and an offset on a narrower domain,
//! each given back as built and refused when it is not one the kernel could have made.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "tests: a failure stops the test, and values are compared exactly"
)]

use geom_curve::{Basis, CurveError, Frame, Line, Trimmed};
use geom_math::{Axis, Transform, TypedTransform};
use geom_nurbs::Direction;
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    AffineSurface, Cone, Cylinder, Extrusion, Offset, ParamSurface, Plane, Revolution, Ruled,
    SurfaceKind, Torus,
};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn frame() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

fn segment(a: [f64; 3], b: [f64; 3]) -> Trimmed<3> {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    Trimmed::new(
        Basis::Line(Line::new(a, d, (0.0, 1.0)).unwrap()),
        (0.0, 1.0),
    )
    .unwrap()
}

fn scaled(s: &SurfaceKind) -> AffineSurface {
    let t = TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap());
    match s.transformed(&t, &mut meter()).unwrap().value {
        SurfaceKind::Affine(a) => a,
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_affine_image_is_rebuilt_from_its_terms() {
    let cylinder = Cylinder::new([1.0, 2.0, 3.0], 2.0, frame(), ((0.0, 1.0), (-1.0, 3.0))).unwrap();
    let cone = Cone::new([0.0; 3], 1.0, 0.5, frame(), ((0.0, 2.0), (0.0, 2.0))).unwrap();
    let torus = Torus::new([0.0; 3], 3.0, 1.0, frame(), ((0.0, 2.0), (0.0, 2.0))).unwrap();
    let cases = [
        (SurfaceKind::Cylinder(cylinder.clone()), (true, false)),
        (SurfaceKind::Cone(cone), (true, false)),
        (SurfaceKind::Torus(torus), (true, true)),
    ];
    for (s, angular) in cases {
        let a = scaled(&s);
        assert_eq!(a.angular(), angular);
        let rebuilt = AffineSurface::from_terms(a.center(), &a.terms(), a.domain(), a.angular());
        assert_eq!(rebuilt.unwrap(), a);
    }
    let a = scaled(&SurfaceKind::Cylinder(cylinder));
    assert_eq!(a.center(), [2.0, 1.0, 9.0]);
    let (terms, domain) = (a.terms(), a.domain());
    let refused = |c: [f64; 3], d, angular| AffineSurface::from_terms(c, &terms, d, angular);
    assert_eq!(
        AffineSurface::from_terms(a.center(), &[], domain, (true, false)),
        Err(CurveError::Degenerate)
    );
    // An angular side longer than a turn; a side not ordered or not finite.
    assert_eq!(
        refused(a.center(), ((0.0, 2.5), domain.1), (true, false)),
        Err(CurveError::SpanOutside)
    );
    assert!(refused(a.center(), ((0.0, 2.5), domain.1), (false, false)).is_ok());
    assert_eq!(
        refused(a.center(), (domain.0, (1.0, 1.0)), (true, false)),
        Err(CurveError::SpanOutside)
    );
    assert_eq!(
        refused(a.center(), (domain.0, (0.0, f64::INFINITY)), (true, false)),
        Err(CurveError::SpanOutside)
    );
    assert_eq!(
        refused(a.center(), (domain.0, (0.0, 3.0)), (true, true)),
        Err(CurveError::SpanOutside)
    );
    assert!(refused([f64::NAN, 0.0, 0.0], domain, (true, false)).is_err());
}

#[test]
fn a_swept_surface_gives_back_its_shape_and_placement() {
    let profile = segment([1.0, 0.0, 0.0], [2.0, 0.0, 3.0]);
    let r = Revolution::new(profile.clone(), [0.0, 0.0, 1.0], frame(), (0.0, 1.5)).unwrap();
    assert_eq!(r.shape().unwrap(), (&profile, [0.0, 0.0, 1.0], frame()));
    let identity = (
        [0.0; 3],
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    );
    assert_eq!(r.placement(), identity);
    let x = Extrusion::new(profile.clone(), [0.0, 0.5, 2.0], (0.0, 2.0)).unwrap();
    assert_eq!(x.shape().unwrap(), (&profile, [0.0, 0.5, 2.0]));
    let second = segment([0.0, 2.0, 0.0], [0.0, 3.0, 2.0]);
    let ruled = Ruled::new(profile.clone(), second.clone(), (0.0, 1.0)).unwrap();
    assert_eq!(ruled.shape().unwrap(), (&profile, &second, 1.0));
    // Moved, a surface is placed again from its placement alone.
    let t = TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap());
    let SurfaceKind::Revolution(moved) = SurfaceKind::Revolution(r.clone())
        .transformed(&t, &mut meter())
        .unwrap()
        .value
    else {
        panic!("not a revolution")
    };
    let (c, l) = moved.placement();
    assert_eq!(r.placed(c, l).unwrap(), moved);
    assert_eq!(
        r.placed([f64::NAN, 0.0, 0.0], l),
        Err(CurveError::NotFinite)
    );
    assert_eq!(
        r.placed(c, [[f64::INFINITY; 3]; 3]),
        Err(CurveError::NotFinite)
    );
}

#[test]
fn a_ruled_surface_takes_sigma_of_one_or_minus_one() {
    let first = segment([1.0, 0.0, 0.0], [2.0, 0.0, 3.0]);
    let ruled = Ruled::new(first, segment([0.0, 2.0, 0.0], [0.0, 3.0, 2.0]), (0.0, 1.0)).unwrap();
    assert_eq!(ruled.with_sigma(1.0).unwrap(), ruled);
    let minus = ruled.with_sigma(-1.0).unwrap();
    assert_eq!(minus.shape().unwrap().2, -1.0);
    // As reversing in v leaves it, on the reversed domain.
    let SurfaceKind::Ruled(reversed) = SurfaceKind::Ruled(ruled.clone())
        .reversed(Direction::V, &mut meter())
        .unwrap()
        .value
    else {
        panic!("not ruled")
    };
    assert_eq!(reversed.shape().unwrap().2, -1.0);
    for sigma in [0.0, 0.5, 2.0, -2.0, f64::NAN] {
        assert_eq!(
            ruled.with_sigma(sigma),
            Err(CurveError::Degenerate),
            "{sigma}"
        );
    }
}

#[test]
fn an_offset_is_restricted_only_within_its_domain() {
    let plane = Plane::new(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        ((0.0, 4.0), (0.0, 2.0)),
    );
    let o = Offset::new(SurfaceKind::Plane(plane.unwrap()), 0.5).unwrap();
    assert_eq!(o.restricted(((0.0, 4.0), (0.0, 2.0))).unwrap(), o);
    let narrow = o.restricted(((1.0, 3.0), (0.5, 1.5))).unwrap();
    assert_eq!(narrow.domain(), ((1.0, 3.0), (0.5, 1.5)));
    assert_eq!(narrow.base(), o.base());
    assert_eq!(narrow.distance(), 0.5);
    // Each side fails alone: not finite, below, empty, above.
    let nan = f64::NAN;
    for bad in [
        ((nan, 3.0), (0.5, 1.5)),
        ((1.0, nan), (0.5, 1.5)),
        ((1.0, 3.0), (nan, 1.5)),
        ((1.0, 3.0), (0.5, nan)),
        ((-1.0, 3.0), (0.5, 1.5)),
        ((2.0, 2.0), (0.5, 1.5)),
        ((1.0, 5.0), (0.5, 1.5)),
        ((1.0, 3.0), (-0.5, 1.5)),
        ((1.0, 3.0), (1.0, 1.0)),
        ((1.0, 3.0), (0.5, 2.5)),
    ] {
        assert_eq!(o.restricted(bad), Err(CurveError::SpanOutside), "{bad:?}");
    }
}
