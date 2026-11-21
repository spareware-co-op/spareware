//! A kind over another domain is the same surface there: the points both domains hold agree to the
//! bit, the new domain is the one asked for (it may reach past the old), and a domain that is not
//! ordered is refused. `geom-intersect` reads an edge's zeros in the room past a plane's, a
//! cylinder's or a cone's end, and a contact on a seam in a chart whose domain is moved half a turn.

#![allow(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "tests: exact comparison of the same formulas, and a failure stops the test"
)]

use geom_curve::{CurveError, Frame};
use geom_nurbs::Limit;
use geom_surface::{Cone, Cylinder, ParamBox, ParamSurface, Plane, Sphere, SurfaceKind, Torus};

#[test]
fn a_kind_over_another_domain_is_the_same_surface() {
    let sides = (Limit::FromRight, Limit::FromRight);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let wider: ParamBox = ((0.0, 2.0), (-4.0, 8.0));
    let narrow: ParamBox = ((0.0, 2.0), (0.0, 4.0));
    let kinds: Vec<(SurfaceKind, SurfaceKind)> = vec![
        {
            let k = Plane::new([1.0, 2.0, 3.0], [1.0, 0.0, 0.0], [0.0, 1.0, 1.0], narrow).unwrap();
            (
                SurfaceKind::Plane(k.with_domain(wider).unwrap()),
                SurfaceKind::Plane(k),
            )
        },
        {
            let k = Cylinder::new([1.0, 2.0, 3.0], 2.0, frame, narrow).unwrap();
            (
                SurfaceKind::Cylinder(k.with_domain(wider).unwrap()),
                SurfaceKind::Cylinder(k),
            )
        },
        {
            let k = Cone::new([1.0, 2.0, 3.0], 1.0, 0.5, frame, narrow).unwrap();
            (
                SurfaceKind::Cone(k.with_domain(wider).unwrap()),
                SurfaceKind::Cone(k),
            )
        },
    ];
    for (wide, old) in kinds {
        assert_eq!(wide.domain(), wider);
        for (u, v) in [(0.25, 0.5), (1.5, 3.5), (0.0, 0.0)] {
            assert_eq!(
                wide.point(u, v, sides).unwrap(),
                old.point(u, v, sides).unwrap()
            );
        }
        // The old domain refuses what the wider one holds.
        assert!(old.point(1.0, -3.0, sides).is_err());
        assert!(wide.point(1.0, -3.0, sides).is_ok());
    }
    let plane = Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], narrow).unwrap();
    assert_eq!(
        plane.with_domain(((1.0, 1.0), (0.0, 1.0))).err(),
        Some(CurveError::SpanOutside)
    );
    let sphere = Sphere::new([0.0; 3], 1.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    assert_eq!(
        sphere.with_domain(((0.0, 2.0), (0.5, 0.25))).err(),
        Some(CurveError::SpanOutside)
    );
    let torus = Torus::new([0.0; 3], 3.0, 1.0, frame, ((0.0, 2.0), (0.0, 2.0))).unwrap();
    assert_eq!(
        torus
            .with_domain(((0.0, 1.0), (0.0, 1.0)))
            .unwrap()
            .domain(),
        ((0.0, 1.0), (0.0, 1.0))
    );
}
