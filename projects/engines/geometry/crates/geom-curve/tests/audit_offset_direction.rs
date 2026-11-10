//! Audit of the side a 3D offset curve lies on.
//!
//! `OffsetLaw::Reference` is documented as "the offset along the cross product of the tangent and
//! a reference direction V, as ISO 10303-42's `offset_curve_3d` does", O = C + d·(C′ × V)/|C′ × V|.
//! ISO 10303-42 (and IFC 4.3's `IfcOffsetCurve3D`, which quotes it) puts the offset "in the
//! direction V × T where V is the fixed reference direction and T is the unit tangent to the basis
//! curve": the other order, so the other side. With V the plane's normal, V × T is T turned a
//! quarter turn anticlockwise, the same side `offset_curve_2d` and this crate's `Planar` law give
//! a positive distance.
//!
//! Source of the reference: IFC 4.3.x documentation, `IfcOffsetCurve3D.md`, at the commit the
//! transition module already cites
//! (<https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometryResource/Entities/IfcOffsetCurve3D.md>).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

use geom_curve::{Basis, Line, Offset, OffsetLaw, ParamCurve, Trimmed};
use geom_nurbs::Limit;

/// A line along +x, offset by +1 with the reference direction +z: V × T = z × x = +y, so the
/// offset curve is the line y = 1.
#[test]
fn a_3d_offset_lies_along_the_reference_direction_cross_the_tangent() {
    let line = Line::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], (0.0, 4.0)).unwrap();
    let o = Offset::new(
        Trimmed::whole(Basis::Line(line)).unwrap(),
        OffsetLaw::Reference {
            distance: 1.0,
            direction: [0.0, 0.0, 1.0],
        },
    )
    .unwrap();
    let p = o.point(2.0, Limit::FromRight).unwrap();
    assert_eq!(
        p,
        [2.0, 1.0, 0.0],
        "ISO 10303-42's offset_curve_3d puts the point at C + d·(V × T) = (2, 1, 0)"
    );
}

/// The same curve in the plane, by the 2D law: a positive distance is to the left, y = 1. The
/// two laws must agree for a planar curve offset about its plane's normal.
#[test]
fn the_planar_and_the_3d_offsets_of_one_line_agree() {
    let flat = Line::new([0.0, 0.0], [1.0, 0.0], (0.0, 4.0)).unwrap();
    let planar = Offset::new(
        Trimmed::whole(Basis::Line(flat)).unwrap(),
        OffsetLaw::Planar { distance: 1.0 },
    )
    .unwrap();
    let line = Line::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], (0.0, 4.0)).unwrap();
    let spatial = Offset::new(
        Trimmed::whole(Basis::Line(line)).unwrap(),
        OffsetLaw::Reference {
            distance: 1.0,
            direction: [0.0, 0.0, 1.0],
        },
    )
    .unwrap();
    let p = planar.point(2.0, Limit::FromRight).unwrap();
    let q = spatial.point(2.0, Limit::FromRight).unwrap();
    assert_eq!(p, [2.0, 1.0]);
    assert_eq!([q[0], q[1]], p, "the 3D offset about +z against the 2D offset");
}
