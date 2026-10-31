//! Every curve and surface kind a body's pools hold reads back as the value written, bit for bit
//! (G94): built through each kind's constructor, and as the operations leave them (reversed,
//! split, moved, mirrored, scaled, fitted), each written, read back, compared and written again.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::geom_nurbs::{self, Direction};
use geom_curve::{
    Axes, Basis, BulgePolyline, Circle, Composite, CurvatureLaw, CurveKind, Ellipse, Frame, Helix,
    HelixAxes, Hyperbola, Involute, Line, Offset, OffsetLaw, OnSurface, Parabola, ParamCurve,
    Polynomial, Station, SurfaceRef, Transition, Trimmed,
};
use geom_format::cbor::Decoder;
use geom_format::kinds::{DEPTH, KindError, read_curve, read_surface, write_curve, write_surface};
use geom_math::{Axis, GridTransform, Transform, TypedTransform};
use geom_op::cbor::Encoder;
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, ParamSurface, Plane, Revolution, Ruled, Sphere, SurfaceKind, Torus,
};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn curve_bytes<const D: usize>(c: &CurveKind<D>) -> Vec<u8> {
    let mut e = Encoder::new();
    write_curve(&mut e, c).unwrap();
    e.finish().unwrap().into_bytes()
}

fn surface_bytes(s: &SurfaceKind) -> Vec<u8> {
    let mut e = Encoder::new();
    write_surface(&mut e, s).unwrap();
    e.finish().unwrap().into_bytes()
}

/// Writes `c`, reads it back, and checks the value and the bytes of writing it again.
fn curve_trip<const D: usize>(what: &str, c: &CurveKind<D>) {
    let bytes = curve_bytes(c);
    let mut d = Decoder::new(&bytes, DEPTH);
    let back = read_curve::<D>(&mut d, &mut meter()).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!(d.next(&mut meter()).unwrap(), None, "{what}");
    assert_eq!(d.end().unwrap(), bytes.len(), "{what}");
    assert_eq!(&back, c, "{what}");
    assert_eq!(curve_bytes(&back), bytes, "{what}: written again");
}

/// As [`curve_trip`], for a surface.
fn surface_trip(what: &str, s: &SurfaceKind) {
    let bytes = surface_bytes(s);
    let mut d = Decoder::new(&bytes, DEPTH);
    let back = read_surface(&mut d, &mut meter()).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!(d.next(&mut meter()).unwrap(), None, "{what}");
    assert_eq!(&back, s, "{what}");
    assert_eq!(surface_bytes(&back), bytes, "{what}: written again");
}

fn frame3() -> Frame<3> {
    Frame::new([0.6, 0.8, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

fn frame2() -> Frame<2> {
    Frame::new([1.0, 0.5], [0.0, 0.0, 1.0]).unwrap()
}

fn circle3() -> Circle<3> {
    Circle::new([1.0, -2.0, 0.5], 2.5, frame3(), (0.25, 1.5)).unwrap()
}

fn segment3(a: [f64; 3], b: [f64; 3]) -> Trimmed<3> {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    Trimmed::new(
        Basis::Line(Line::new(a, d, (0.0, 1.0)).unwrap()),
        (0.0, 1.0),
    )
    .unwrap()
}

fn nurbs3() -> geom_nurbs::Curve<3> {
    let basis = geom_nurbs::Basis::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap();
    geom_nurbs::Curve::from_homogeneous(
        basis,
        vec![
            [0.0, 0.0, 0.0],
            [0.7, 0.7, -0.0],
            [2.0, 0.25, 1.0],
            [3.0, -1.0, 0.125],
        ],
        Some(vec![1.0, 0.7, 1.3, 1.0]),
    )
    .unwrap()
}

/// Every leaf curve kind in 3D, and those that live in 2D.
fn leaves3() -> Vec<(&'static str, CurveKind<3>)> {
    let axes = Axes::Frame {
        frame: frame3(),
        a: 3.0,
        b: 1.25,
    };
    let vectors = Axes::Vectors {
        u: [1.0, 0.5, 0.0],
        v: [-0.25, 2.0, 0.75],
    };
    vec![
        ("nurbs", CurveKind::Nurbs(nurbs3())),
        (
            "line",
            CurveKind::Line(Line::new([1.0, 2.0, 3.0], [0.5, -0.25, 1.0], (-1.5, 2.0)).unwrap()),
        ),
        (
            "ray",
            CurveKind::Line(Line::ray([1.0, 2.0, 3.0], [0.0, 0.0, -1.0]).unwrap()),
        ),
        (
            "infinite line",
            CurveKind::Line(Line::infinite([0.0, 0.0, 0.0], [1.0, 1.0, 0.0]).unwrap()),
        ),
        ("circle", CurveKind::Circle(circle3())),
        (
            "ellipse",
            CurveKind::Ellipse(Ellipse::new([0.0, 1.0, 2.0], axes, (0.0, 1.0)).unwrap()),
        ),
        (
            "ellipse on vectors",
            CurveKind::Ellipse(Ellipse::new([0.0; 3], vectors, (-0.5, 0.5)).unwrap()),
        ),
        (
            "hyperbola",
            CurveKind::Hyperbola(Hyperbola::new([1.0, 0.0, 0.0], axes, (-0.5, 0.75)).unwrap()),
        ),
        (
            "parabola",
            CurveKind::Parabola(Parabola::new([0.0, 0.0, 1.0], axes, (-2.0, 1.0)).unwrap()),
        ),
        (
            "helix",
            CurveKind::Helix(
                Helix::new(
                    [0.0, 0.0, -1.0],
                    HelixAxes::Frame {
                        frame: frame3(),
                        radius: 2.0,
                        pitch: -0.5,
                    },
                    (0.0, 4.0),
                )
                .unwrap(),
            ),
        ),
        (
            "helix on vectors",
            CurveKind::Helix(
                Helix::new(
                    [0.0; 3],
                    HelixAxes::Vectors {
                        u: [1.0, 0.0, 0.0],
                        v: [0.0, 2.0, 0.0],
                        w: [0.0, 0.0, 0.5],
                    },
                    (-1.0, 1.0),
                )
                .unwrap(),
            ),
        ),
        (
            "polynomial",
            CurveKind::Polynomial(
                Polynomial::new(
                    vec![[1.0, 0.0, 0.0], [0.5, 2.0, -1.0], [0.0, 0.25, 3.0]],
                    (0.0, 1.0),
                )
                .unwrap(),
            ),
        ),
        (
            "involute",
            CurveKind::Involute(Involute::new([0.0; 3], 1.5, frame3(), (0.0, 1.0)).unwrap()),
        ),
        (
            "involute on vectors",
            CurveKind::Involute(Involute::with_axes([0.0; 3], vectors, (0.0, 0.5)).unwrap()),
        ),
    ]
}

#[test]
fn every_leaf_curve_kind_reads_back() {
    for (what, c) in leaves3() {
        curve_trip(what, &c);
    }
}

#[test]
fn the_two_dimensional_kinds_read_back() {
    let bulge = BulgePolyline::<2>::new(
        vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        vec![0.5, -0.25],
        Some(vec![(0.1, 0.2), (0.2, 0.0)]),
        false,
    )
    .unwrap();
    curve_trip("bulge", &CurveKind::Bulge(bulge.clone()));
    let closed = BulgePolyline::<2>::new(
        vec![[0.0, 0.0], [2.0, 0.0], [1.0, 1.5]],
        vec![0.0, 1.0, -0.5],
        None,
        true,
    )
    .unwrap();
    curve_trip("closed bulge", &CurveKind::Bulge(closed));
    // Reversed, its parameter starts below zero.
    let reversed = CurveKind::Bulge(bulge)
        .reversed(&mut meter())
        .unwrap()
        .value;
    curve_trip("reversed bulge", &reversed);
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.01,
    };
    let t = Transition::<2>::new([0.0, 0.0], [1.0, 0.0], 100.0, law, &mut meter()).unwrap();
    curve_trip("transition", &CurveKind::Transition(t.clone()));
    curve_trip(
        "transition part",
        &CurveKind::Transition(t.segment((10.0, 60.0), true).unwrap()),
    );
    let laws = [
        CurvatureLaw::Bloss {
            start: 0.0,
            end: 0.002,
        },
        CurvatureLaw::CantCoupled {
            start: 0.0,
            end: 0.004,
            height: 1.8,
            cant: 0.1,
        },
        CurvatureLaw::Polynomial {
            terms: [None, Some(0.0), Some(1.0e-4), None, None, None, None, None],
            from: 0.0,
        },
        CurvatureLaw::SineSpiral {
            constant: None,
            linear: Some(0.0),
            sine: 400.0,
            length: 100.0,
            from: 0.0,
        },
    ];
    for law in laws {
        if let Ok(t) = Transition::<2>::new([1.0, 2.0], [0.0, 1.0], 100.0, law, &mut meter()) {
            curve_trip("transition law", &CurveKind::Transition(t));
        }
    }
    let line2 = Basis::Line(Line::new([0.0, 0.0], [1.0, 2.0], (0.0, 1.0)).unwrap());
    let offset = Offset::new(
        Trimmed::new(line2, (0.0, 1.0)).unwrap(),
        OffsetLaw::Planar { distance: 0.25 },
    )
    .unwrap();
    curve_trip("planar offset", &CurveKind::Offset(offset.clone()));
    curve_trip(
        "planar offset, cached",
        &CurveKind::Offset(offset.cached(&mut meter()).unwrap()),
    );
    let circle2 = Circle::new([0.0, 0.0], 1.0, frame2(), (0.0, 1.0)).unwrap();
    curve_trip("2D circle", &CurveKind::Circle(circle2));
}

#[test]
fn compound_curve_kinds_read_back() {
    let trimmed = Trimmed::new(Basis::Circle(circle3()), (0.5, 1.0)).unwrap();
    curve_trip("trimmed", &CurveKind::Trimmed(trimmed.clone()));
    let nurbs = Trimmed::new(Basis::Nurbs(nurbs3()), (0.25, 0.75)).unwrap();
    curve_trip("trimmed nurbs", &CurveKind::Trimmed(nurbs));
    let composite = Composite::new(vec![
        segment3([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        segment3([1.0, 0.0, 0.0], [1.0, 2.0, 0.0]),
        segment3([1.0, 2.0, 0.0], [0.0, 2.0, 1.0]),
    ])
    .unwrap();
    curve_trip("composite", &CurveKind::Composite(composite.clone()));
    // Moved and scaled, its joints' gaps are certified again.
    let scale = TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap());
    let moved = CurveKind::Composite(composite)
        .transformed(&scale, &mut meter())
        .unwrap()
        .value;
    curve_trip("scaled composite", &moved);
    let (left, right) = moved.split(1.5, &mut meter()).unwrap();
    curve_trip("split composite, left", &left.value);
    curve_trip("split composite, right", &right.value);
    let law = OffsetLaw::Reference {
        distance: 0.5,
        direction: [0.0, 0.0, 1.0],
    };
    let offset = Offset::new(trimmed.clone(), law).unwrap();
    curve_trip("offset", &CurveKind::Offset(offset.clone()));
    curve_trip(
        "offset, cached",
        &CurveKind::Offset(offset.cached(&mut meter()).unwrap()),
    );
    let stations = OffsetLaw::Distances(vec![
        Station {
            at: 0.5,
            lateral: 0.25,
            vertical: 0.0,
        },
        Station {
            at: 1.0,
            lateral: 0.5,
            vertical: -0.125,
        },
    ]);
    if let Ok(o) = Offset::new(trimmed, stations) {
        curve_trip("offset by stations", &CurveKind::Offset(o));
    }
    let pcurve = Trimmed::new(
        Basis::Line(Line::new([0.0, 0.0], [1.0, 1.0], (0.0, 1.0)).unwrap()),
        (0.0, 1.0),
    )
    .unwrap();
    let on = OnSurface::<3>::new(pcurve, SurfaceRef(7), nurbs3(), 1.0e-9).unwrap();
    curve_trip("on a surface", &CurveKind::OnSurface(on));
}

#[test]
fn the_operations_results_read_back() {
    let ops: Vec<TypedTransform> = vec![
        TypedTransform::Exact(GridTransform::mirror(Axis::Y)),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
    ];
    for (what, c) in leaves3() {
        if let Ok(r) = c.reversed(&mut meter()) {
            curve_trip(what, &r.value);
        }
        for t in &ops {
            if let Ok(m) = c.transformed(t, &mut meter()) {
                curve_trip(what, &m.value);
            }
        }
    }
}

fn surfaces() -> Vec<(&'static str, SurfaceKind)> {
    let f = frame3();
    let basis = geom_nurbs::Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let nurbs = geom_nurbs::Surface::from_homogeneous(
        basis.clone(),
        basis,
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.5],
            [0.0, 1.0, 0.0],
            [2.0, 2.0, 1.0],
        ],
        Some(vec![1.0, 0.5, 0.75, 1.0]),
    )
    .unwrap();
    let profile = segment3([1.0, 0.0, 0.0], [2.0, 0.0, 3.0]);
    let arc = Trimmed::new(Basis::Circle(circle3()), (0.25, 1.5)).unwrap();
    vec![
        ("nurbs", SurfaceKind::Nurbs(nurbs)),
        (
            "plane",
            SurfaceKind::Plane(
                Plane::new(
                    [0.0, 1.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 0.5, 1.0],
                    ((0.0, 2.0), (-1.0, 1.0)),
                )
                .unwrap(),
            ),
        ),
        (
            "cylinder",
            SurfaceKind::Cylinder(
                Cylinder::new([0.0; 3], 2.0, f, ((0.0, 1.0), (-1.0, 3.0))).unwrap(),
            ),
        ),
        (
            "cone",
            SurfaceKind::Cone(Cone::new([0.0; 3], 1.0, 0.5, f, ((0.0, 2.0), (0.0, 2.0))).unwrap()),
        ),
        (
            "sphere",
            SurfaceKind::Sphere(
                Sphere::new([1.0, 0.0, 0.0], 3.0, f, ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
            ),
        ),
        (
            "torus",
            SurfaceKind::Torus(
                Torus::new([0.0; 3], 3.0, 1.0, f, ((0.0, 2.0), (0.0, 2.0))).unwrap(),
            ),
        ),
        (
            "revolution",
            SurfaceKind::Revolution(
                Revolution::new(profile.clone(), [0.0, 0.0, 1.0], f, (0.0, 1.5)).unwrap(),
            ),
        ),
        (
            "extrusion",
            SurfaceKind::Extrusion(Extrusion::new(arc, [0.0, 0.5, 2.0], (0.0, 2.0)).unwrap()),
        ),
        (
            "ruled",
            SurfaceKind::Ruled(
                Ruled::new(
                    profile,
                    segment3([0.0, 2.0, 0.0], [0.0, 3.0, 2.0]),
                    (0.0, 1.0),
                )
                .unwrap(),
            ),
        ),
    ]
}

#[test]
fn every_surface_kind_reads_back() {
    for (what, s) in surfaces() {
        surface_trip(what, &s);
    }
}

#[test]
fn surfaces_as_the_operations_leave_them_read_back() {
    let ops: Vec<TypedTransform> = vec![
        TypedTransform::Exact(GridTransform::mirror(Axis::Y)),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
    ];
    let mut affine = 0;
    let mut placed = 0;
    let mut flipped = 0;
    for (what, s) in surfaces() {
        for dir in [Direction::U, Direction::V] {
            if let Ok(r) = s.reversed(dir, &mut meter()) {
                surface_trip(what, &r.value);
                flipped += usize::from(
                    matches!(&r.value, SurfaceKind::Cylinder(c) if c.axial_sign() < 0.0),
                );
                flipped += usize::from(
                    matches!(&r.value, SurfaceKind::Ruled(r) if r.shape().unwrap().2 < 0.0),
                );
            }
            let ((u0, u1), (v0, v1)) = s.domain();
            let t = match dir {
                Direction::U => 0.5 * (u0 + u1),
                Direction::V => 0.5 * (v0 + v1),
            };
            if let Ok((l, r)) = s.split(dir, t, &mut meter()) {
                surface_trip(what, &l.value);
                surface_trip(what, &r.value);
            }
        }
        for t in &ops {
            if let Ok(m) = s.transformed(t, &mut meter()) {
                surface_trip(what, &m.value);
                affine += usize::from(matches!(m.value, SurfaceKind::Affine(_)));
                placed += usize::from(
                    matches!(&m.value, SurfaceKind::Revolution(r) if r.placement().1[0][1] != 0.0),
                );
            }
        }
    }
    assert!(
        affine > 0 && placed > 0 && flipped > 0,
        "{affine} {placed} {flipped}"
    );
}

#[test]
fn offset_surfaces_read_back_cached_split_and_nested() {
    let plane = surfaces()
        .into_iter()
        .find(|(w, _)| *w == "plane")
        .unwrap()
        .1;
    let cylinder = surfaces()
        .into_iter()
        .find(|(w, _)| *w == "cylinder")
        .unwrap()
        .1;
    for base in [plane, cylinder] {
        let offset = geom_surface::Offset::new(base, 0.5).unwrap();
        surface_trip("offset", &SurfaceKind::Offset(offset.clone()));
        surface_trip(
            "offset, cached",
            &SurfaceKind::Offset(offset.cached(&mut meter()).unwrap()),
        );
        let whole = SurfaceKind::Offset(offset.clone());
        let ((u0, u1), _) = whole.domain();
        let (l, r) = whole
            .split(Direction::U, 0.5 * (u0 + u1), &mut meter())
            .unwrap();
        surface_trip("split offset", &l.value);
        surface_trip("split offset", &r.value);
        // Offsets of offsets, as deep as an offset nests.
        let mut s = SurfaceKind::Offset(offset);
        while let Ok(o) = geom_surface::Offset::new(s.clone(), 0.25) {
            s = SurfaceKind::Offset(o);
            surface_trip("nested offset", &s);
        }
    }
}

#[test]
fn a_kind_from_another_form_is_refused() {
    let line = CurveKind::Line(Line::new([1.0, 2.0, 3.0], [0.5, -0.25, 1.0], (-1.5, 2.0)).unwrap());
    let bytes = curve_bytes(&line);
    // A 2D reader refuses a 3D line's vectors.
    let mut d = Decoder::new(&bytes, DEPTH);
    assert!(matches!(
        read_curve::<2>(&mut d, &mut meter()),
        Err(KindError::Shape(_))
    ));
    // An unknown code, and a known one with another number of fields.
    for (code, len) in [(99_u64, 4_u64), (2, 5)] {
        let mut e = Encoder::new();
        e.array(len).unwrap();
        e.uint(code).unwrap();
        for _ in 1..len {
            e.uint(0).unwrap();
        }
        let b = e.finish().unwrap().into_bytes();
        assert!(matches!(
            read_curve::<3>(&mut Decoder::new(&b, DEPTH), &mut meter()),
            Err(KindError::Shape(_))
        ));
        assert!(matches!(
            read_surface(&mut Decoder::new(&b, DEPTH), &mut meter()),
            Err(KindError::Shape(_))
        ));
    }
    // A degenerate direction is the constructor's to refuse.
    let mut e = Encoder::new();
    e.array(4).unwrap();
    e.uint(2).unwrap();
    for v in [[1.0, 2.0, 3.0], [0.0, 0.0, 0.0]] {
        e.array(3).unwrap();
        for x in v {
            e.float(x).unwrap();
        }
    }
    e.array(2).unwrap();
    e.float(0.0).unwrap();
    e.float(1.0).unwrap();
    let b = e.finish().unwrap().into_bytes();
    assert!(matches!(
        read_curve::<3>(&mut Decoder::new(&b, DEPTH), &mut meter()),
        Err(KindError::Curve(_))
    ));
}
