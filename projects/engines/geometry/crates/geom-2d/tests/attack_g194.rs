//! Attacks on piece G194: the drafting entity model (`geom_2d::entity`; chapter 14.1 and 14.10 of
//! the kernel architecture). Every test tries to break a claim of the piece's done-when:
//! every 14.1 entity as a closed enum with stable tags, attribute pass-through, regions as Sheet
//! bodies, and kind changes under transforms.
//!
//! - **Stable tags**: the numbers are fixed; each round-trips; every other number is refused.
//! - **Every entity**: one of each shape is made, its tag is the one the table says, and all
//!   eighteen tags are reached.
//! - **Attribute pass-through**: the key survives every shape under every class of transform.
//! - **Kind changes**: a circle becomes an ellipse and an arc an elliptical arc under a non-uniform
//!   scale and nowhere else; a bulge polyline becomes a run; a mirror is reported; a degenerate
//!   transform is refused. Classes come from real transforms of `geom-math`.
//! - **Valid by construction**: each constructor's refusals, at and one past its edge.
//! - **Regions** are Sheet bodies and nothing else.
//! - **Fractions** are reduced, signed on the numerator and bounded.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_2d::carrier::{Circle2, Line2};
use geom_2d::entity::{
    Array, Attach, Attr, Becomes, BlockDef, BlockIx, DimKind, Dimension, Ends, Entity, EntityError,
    EntityIx, Fill, FitIntent, Frac, Hatch, Insert, Island, Key, Leader, Param, Polyline, Ref,
    Region, Shape, Spline, Tag, Text, ViewIx, Viewport, FRAC_LIMIT,
};
use geom_2d::point::Pt2;
use geom_math::{Axis, Box2, Classification, GridTransform, Transform, TransformKind, TypedTransform};
use geom_num::arc::Circle2 as GridCircle;
use geom_num::conic::Ellipse2;
use geom_num::grid::{P2, Steps};
use geom_run::{Budget, WorkMeter};
use geom_topo::{BodyKind, Builder, Ix, RegionKind};

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn frac(n: i64, d: i64) -> Frac {
    Frac::new(n, d).unwrap()
}

fn sheet() -> geom_topo::Body {
    let mut b = Builder::new();
    b.arenas.regions.kind.push(RegionKind::Void).unwrap();
    b.arenas.regions.first_shell.push(None).unwrap();
    b.publish(
        BodyKind::Sheet,
        Ix::new(0).unwrap(),
        &mut WorkMeter::new(Budget::UNLIMITED),
    )
    .unwrap()
}

fn solid_outside_only() -> geom_topo::Body {
    let mut b = Builder::new();
    b.arenas.regions.kind.push(RegionKind::Void).unwrap();
    b.arenas.regions.first_shell.push(None).unwrap();
    b.publish(
        BodyKind::Acorn,
        Ix::new(0).unwrap(),
        &mut WorkMeter::new(Budget::UNLIMITED),
    )
    .unwrap()
}

fn line() -> Line2 {
    Line2::new(0, 1, 0).unwrap()
}

fn circle() -> Circle2 {
    Circle2::Grid(GridCircle::new(p2(0, 0), 5).unwrap())
}

fn ellipse() -> Ellipse2 {
    Ellipse2::new(p2(0, 0), [5, 0], 3, 5).unwrap()
}

fn polyline(bulge: i64) -> Polyline {
    Polyline::new(
        vec![p2(0, 0), p2(10, 0), p2(10, 10)],
        vec![frac(bulge, 1), frac(0, 1)],
        None,
        false,
    )
    .unwrap()
}

/// One shape of every tag, in tag order.
fn every_shape() -> Vec<Shape> {
    let a = Pt2::Grid(p2(0, 0));
    let b = Pt2::Grid(p2(5, 0));
    let spline = Spline::new(
        2,
        vec![p2(0, 0), p2(5, 5), p2(10, 0)],
        None,
        vec![frac(0, 1), frac(0, 1), frac(0, 1), frac(1, 1), frac(1, 1), frac(1, 1)],
        Some(FitIntent {
            points: vec![p2(0, 0), p2(10, 0)],
            start_tangent: None,
            end_tangent: Some([1, 0]),
            tolerance: Steps(2),
            version: 1,
        }),
    )
    .unwrap();
    let text = Text::new(
        p2(1, 1),
        Attach::MiddleCentre,
        [1, 0],
        Steps(10),
        None,
        "A".into(),
    )
    .unwrap();
    let hatch = Hatch::new(vec![EntityIx(0)], Island::Normal, Fill::Solid).unwrap();
    let dim = Dimension::new(DimKind::Linear, vec![Ref::Point(a), Ref::Point(b)], Key(1)).unwrap();
    let leader = Leader::new(vec![p2(0, 0), p2(3, 3)], None, Key(2)).unwrap();
    let block = BlockDef {
        origin: p2(0, 0),
        entities: vec![Entity::new(Attr(1), Shape::Point(p2(1, 1)))],
    };
    let insert = Insert {
        block: BlockIx(0),
        at: TypedTransform::from(GridTransform::translation([1, 2, 0]).unwrap()),
        array: Some(Array::new(2, 3, [10, 0], [0, 10]).unwrap()),
        values: vec![(Key(3), "x".into())],
    };
    let window = Box2::new(p2(0, 0), p2(100, 100)).unwrap();
    let viewport = Viewport::new(window, ViewIx(0), frac(1, 50), frac(0, 1), None).unwrap();
    vec![
        Shape::Point(p2(1, 2)),
        Shape::Line {
            carrier: line(),
            ends: Ends::Segment(a, b),
        },
        Shape::Line {
            carrier: line(),
            ends: Ends::Ray(a),
        },
        Shape::Line {
            carrier: line(),
            ends: Ends::Infinite,
        },
        Shape::Circle(circle()),
        Shape::Arc {
            circle: circle(),
            start: a,
            end: b,
        },
        Shape::Ellipse(ellipse()),
        Shape::EllipticArc {
            ellipse: ellipse(),
            start: Param::At(frac(0, 1)),
            end: Param::Far,
        },
        Shape::Spline(spline),
        Shape::Polyline(polyline(1)),
        Shape::Region(Region::new(sheet()).unwrap()),
        Shape::Hatch(hatch),
        Shape::Text(text),
        Shape::Dimension(dim),
        Shape::Leader(leader),
        Shape::Block(block),
        Shape::Insert(insert),
        Shape::Viewport(viewport),
    ]
}

fn class(kind: TransformKind, mirror: bool) -> Classification {
    Classification { kind, mirror }
}

#[test]
fn the_tag_numbers_are_the_written_ones_and_round_trip() {
    let written: [(Tag, u8); 18] = [
        (Tag::Point, 1),
        (Tag::Segment, 2),
        (Tag::Ray, 3),
        (Tag::Infinite, 4),
        (Tag::Circle, 5),
        (Tag::Arc, 6),
        (Tag::Ellipse, 7),
        (Tag::EllipticArc, 8),
        (Tag::Spline, 9),
        (Tag::Polyline, 10),
        (Tag::Region, 11),
        (Tag::Hatch, 12),
        (Tag::Text, 13),
        (Tag::Dimension, 14),
        (Tag::Leader, 15),
        (Tag::Block, 16),
        (Tag::Insert, 17),
        (Tag::Viewport, 18),
    ];
    assert_eq!(Tag::ALL.len(), written.len());
    for (i, (tag, number)) in written.iter().enumerate() {
        assert_eq!(tag.as_u8(), *number);
        assert_eq!(Tag::ALL[i], *tag);
        assert_eq!(Tag::from_u8(*number), Some(*tag));
    }
}

#[test]
fn no_other_number_is_a_tag() {
    assert_eq!(Tag::from_u8(0), None);
    for n in 19..=u8::MAX {
        assert_eq!(Tag::from_u8(n), None, "{n}");
    }
}

#[test]
fn every_entity_of_the_table_is_made_and_tagged_as_it_should() {
    let shapes = every_shape();
    assert_eq!(shapes.len(), Tag::ALL.len());
    for (shape, tag) in shapes.iter().zip(Tag::ALL) {
        assert_eq!(shape.tag(), tag);
        assert_eq!(Entity::new(Attr(7), shape.clone()).tag(), tag);
    }
}

#[test]
fn a_line_is_a_segment_a_ray_or_infinite_by_its_ends() {
    let a = Pt2::Grid(p2(0, 0));
    for (ends, tag) in [
        (Ends::Infinite, Tag::Infinite),
        (Ends::Ray(a), Tag::Ray),
        (Ends::Segment(a, a), Tag::Segment),
    ] {
        let shape = Shape::Line {
            carrier: line(),
            ends,
        };
        assert_eq!(shape.tag(), tag);
    }
}

#[test]
fn the_attribute_key_survives_every_shape_under_every_transform() {
    let classes = [
        class(TransformKind::Identity, false),
        class(TransformKind::Translation, false),
        class(TransformKind::Rigid, false),
        class(TransformKind::Rigid, true),
        class(TransformKind::UniformScale, false),
        class(TransformKind::UniformScale, true),
        class(TransformKind::NonUniform, false),
        class(TransformKind::NonUniform, true),
    ];
    for (i, shape) in every_shape().into_iter().enumerate() {
        for key in [0_u64, 1, u64::MAX, 0x0123_4567_89ab_cdef + i as u64] {
            let e = Entity::new(Attr(key), shape.clone());
            assert_eq!(e.attr(), Attr(key));
            for c in classes {
                let under = e.under(c).unwrap();
                assert_eq!(under.attr, Attr(key), "{:?} under {c:?}", e.tag());
                assert_eq!(under.change.mirror, c.mirror);
            }
        }
    }
}

#[test]
fn a_degenerate_transform_has_no_image_for_any_shape() {
    for shape in every_shape() {
        for mirror in [false, true] {
            let c = class(TransformKind::Degenerate, mirror);
            assert_eq!(shape.under(c).unwrap_err(), EntityError::Degenerate);
        }
    }
}

#[test]
fn only_a_non_uniform_transform_changes_a_kind() {
    let keeps = [
        TransformKind::Identity,
        TransformKind::Translation,
        TransformKind::Rigid,
        TransformKind::UniformScale,
    ];
    for shape in every_shape() {
        let tag = shape.tag();
        for kind in keeps {
            let got = shape.under(class(kind, false)).unwrap().becomes;
            assert_eq!(got, Becomes::One(tag), "{tag:?} under {kind:?}");
        }
        let got = shape
            .under(class(TransformKind::NonUniform, false))
            .unwrap()
            .becomes;
        let expected = match tag {
            Tag::Circle => Becomes::One(Tag::Ellipse),
            Tag::Arc => Becomes::One(Tag::EllipticArc),
            Tag::Polyline => Becomes::Run,
            other => Becomes::One(other),
        };
        assert_eq!(got, expected, "{tag:?}");
    }
}

#[test]
fn a_straight_polyline_stays_a_polyline_under_a_non_uniform_scale() {
    let straight = Shape::Polyline(polyline(0));
    let c = class(TransformKind::NonUniform, false);
    assert_eq!(straight.under(c).unwrap().becomes, Becomes::One(Tag::Polyline));
    let curved = Shape::Polyline(polyline(1));
    assert_eq!(curved.under(c).unwrap().becomes, Becomes::Run);
    // One bulge anywhere is enough, and a negative one counts.
    let last = Polyline::new(
        vec![p2(0, 0), p2(10, 0), p2(10, 10)],
        vec![frac(0, 1), frac(-1, 3)],
        None,
        false,
    )
    .unwrap();
    assert_eq!(Shape::Polyline(last).under(c).unwrap().becomes, Becomes::Run);
}

#[test]
fn real_transforms_classify_into_the_kind_changes() {
    let scale = Transform::scale(2.0, 1.0, 1.0).unwrap().classify().unwrap();
    let uniform = Transform::uniform_scale(3.0).unwrap().classify().unwrap();
    let mirror = Transform::mirror(Axis::X).classify().unwrap();
    let c = Shape::Circle(circle());
    assert_eq!(c.under(scale).unwrap().becomes, Becomes::One(Tag::Ellipse));
    assert!(!scale.mirror);
    assert_eq!(c.under(uniform).unwrap().becomes, Becomes::One(Tag::Circle));
    let m = c.under(mirror).unwrap();
    assert_eq!(m.becomes, Becomes::One(Tag::Circle));
    assert!(m.mirror);
    let flatten = Transform::scale(0.0, 1.0, 1.0).unwrap().classify().unwrap();
    assert_eq!(c.under(flatten).unwrap_err(), EntityError::Degenerate);
}

#[test]
fn fractions_are_reduced_and_signed_on_the_numerator() {
    let f = frac(-6, -4);
    assert_eq!((f.numerator(), f.denominator()), (3, 2));
    let g = frac(6, -4);
    assert_eq!((g.numerator(), g.denominator()), (-3, 2));
    assert!(g.is_negative());
    assert!(frac(0, -9).is_zero());
    assert_eq!(frac(0, -9).denominator(), 1);
    assert_eq!(g.negated(), frac(3, 2));
    assert_eq!(Frac::whole(7).unwrap(), frac(14, 2));
}

#[test]
fn fractions_have_a_bound_and_no_zero_denominator() {
    assert_eq!(Frac::new(1, 0).unwrap_err(), EntityError::Fraction);
    assert!(Frac::new(FRAC_LIMIT - 1, 1).is_ok());
    assert!(Frac::new(1, FRAC_LIMIT - 1).is_ok());
    assert_eq!(Frac::new(FRAC_LIMIT, 1).unwrap_err(), EntityError::Fraction);
    assert_eq!(Frac::new(1, FRAC_LIMIT).unwrap_err(), EntityError::Fraction);
    assert_eq!(Frac::new(-FRAC_LIMIT, 1).unwrap_err(), EntityError::Fraction);
    // Past the bound only until it reduces.
    assert_eq!(Frac::new(FRAC_LIMIT * 3, FRAC_LIMIT).unwrap(), frac(3, 1));
    assert_eq!(Frac::new(i64::MIN, 1).unwrap_err(), EntityError::Fraction);
}

#[test]
fn fractions_order_by_exact_products() {
    assert!(frac(1, 3).at_most(frac(1, 2)));
    assert!(!frac(1, 2).at_most(frac(1, 3)));
    assert!(frac(2, 4).at_most(frac(1, 2)));
    assert!(frac(-1, 2).at_most(frac(1, 3)));
    let big = frac(FRAC_LIMIT - 1, FRAC_LIMIT - 2);
    let bigger = frac(FRAC_LIMIT - 2, FRAC_LIMIT - 3);
    assert!(big.at_most(bigger));
    assert!(!bigger.at_most(big));
}

#[test]
fn a_polyline_has_one_bulge_for_each_segment() {
    let v = vec![p2(0, 0), p2(1, 0), p2(1, 1)];
    let z = frac(0, 1);
    // Open: 3 vertices, 2 segments.
    assert!(Polyline::new(v.clone(), vec![z, z], None, false).is_ok());
    assert_eq!(
        Polyline::new(v.clone(), vec![z], None, false).unwrap_err(),
        EntityError::Polyline
    );
    assert_eq!(
        Polyline::new(v.clone(), vec![z, z, z], None, false).unwrap_err(),
        EntityError::Polyline
    );
    // Closed: 3 vertices, 3 segments.
    assert!(Polyline::new(v.clone(), vec![z, z, z], None, true).is_ok());
    assert_eq!(
        Polyline::new(v.clone(), vec![z, z], None, true).unwrap_err(),
        EntityError::Polyline
    );
    // Too few vertices.
    assert_eq!(
        Polyline::new(vec![p2(0, 0)], vec![], None, false).unwrap_err(),
        EntityError::Polyline
    );
    assert_eq!(
        Polyline::new(vec![p2(0, 0), p2(1, 1)], vec![z, z], None, true).unwrap_err(),
        EntityError::Polyline
    );
    // Widths, when given, are one for each segment too.
    let w = (Steps(1), Steps(2));
    let ok = Polyline::new(v.clone(), vec![z, z], Some(vec![w, w]), false).unwrap();
    assert_eq!(ok.widths().unwrap().len(), 2);
    assert_eq!(ok.vertices().len(), 3);
    assert_eq!(ok.bulges().len(), 2);
    assert!(!ok.is_closed());
    assert_eq!(
        Polyline::new(v, vec![z, z], Some(vec![w]), false).unwrap_err(),
        EntityError::Polyline
    );
}

#[test]
fn a_spline_must_fit_its_knots_to_its_control_points() {
    let c = vec![p2(0, 0), p2(1, 1), p2(2, 0), p2(3, 1)];
    let knots = |v: &[i64]| v.iter().map(|k| frac(*k, 1)).collect::<Vec<_>>();
    // Degree 2 with 4 points: 4 + 2 + 1 = 7 knots.
    let ok = Spline::new(2, c.clone(), None, knots(&[0, 0, 0, 1, 2, 2, 2]), None).unwrap();
    assert_eq!(ok.degree(), 2);
    assert_eq!(ok.control().len(), 4);
    assert_eq!(ok.knots().len(), 7);
    assert!(ok.weights().is_none() && ok.intent().is_none());
    for bad in [knots(&[0, 0, 0, 1, 2, 2]), knots(&[0, 0, 0, 1, 2, 2, 2, 2])] {
        assert_eq!(
            Spline::new(2, c.clone(), None, bad, None).unwrap_err(),
            EntityError::Spline
        );
    }
    // Knots in order, equal allowed, a step back refused.
    assert_eq!(
        Spline::new(2, c.clone(), None, knots(&[0, 0, 0, 2, 1, 2, 2]), None).unwrap_err(),
        EntityError::Spline
    );
    // Degree zero, and not more control points than the degree.
    assert_eq!(
        Spline::new(0, c.clone(), None, knots(&[0, 0, 0, 0, 0]), None).unwrap_err(),
        EntityError::Spline
    );
    assert_eq!(
        Spline::new(4, c.clone(), None, knots(&[0; 9]), None).unwrap_err(),
        EntityError::Spline
    );
}

#[test]
fn spline_weights_are_one_positive_fraction_for_each_control_point() {
    let c = vec![p2(0, 0), p2(1, 1), p2(2, 0)];
    let knots = vec![frac(0, 1), frac(0, 1), frac(0, 1), frac(1, 1), frac(1, 1), frac(1, 1)];
    let w = |v: &[(i64, i64)]| v.iter().map(|(n, d)| frac(*n, *d)).collect::<Vec<_>>();
    let ok = Spline::new(2, c.clone(), Some(w(&[(1, 1), (1, 2), (3, 1)])), knots.clone(), None);
    assert_eq!(ok.unwrap().weights().unwrap().len(), 3);
    for bad in [w(&[(1, 1), (1, 1)]), w(&[(1, 1), (0, 1), (1, 1)]), w(&[(1, 1), (-1, 2), (1, 1)])] {
        assert_eq!(
            Spline::new(2, c.clone(), Some(bad), knots.clone(), None).unwrap_err(),
            EntityError::Spline
        );
    }
}

#[test]
fn a_spline_keeps_its_creation_intent() {
    let spline = match every_shape().remove(8) {
        Shape::Spline(s) => s,
        other => panic!("{other:?}"),
    };
    let intent = spline.intent().unwrap();
    assert_eq!(intent.points, vec![p2(0, 0), p2(10, 0)]);
    assert_eq!(intent.end_tangent, Some([1, 0]));
    assert_eq!(intent.start_tangent, None);
    assert_eq!(intent.tolerance, Steps(2));
    assert_eq!(intent.version, 1);
}

#[test]
fn a_region_is_a_sheet_and_nothing_else() {
    let region = Region::new(sheet()).unwrap();
    assert_eq!(region.body().kind(), BodyKind::Sheet);
    assert_eq!(
        Region::new(solid_outside_only()).unwrap_err(),
        EntityError::NotASheet
    );
}

#[test]
fn text_needs_a_height_and_a_direction() {
    let make = |dir: [i64; 2], h: u64, w: Option<Steps>| {
        Text::new(p2(0, 0), Attach::TopLeft, dir, Steps(h), w, "abc".into())
    };
    let t = make([0, -1], 5, Some(Steps(40))).unwrap();
    assert_eq!(t.anchor(), p2(0, 0));
    assert_eq!(t.attach(), Attach::TopLeft);
    assert_eq!(t.direction(), [0, -1]);
    assert_eq!(t.height(), Steps(5));
    assert_eq!(t.width(), Some(Steps(40)));
    assert_eq!(t.content(), "abc");
    assert_eq!(make([0, 0], 5, None).unwrap_err(), EntityError::Text);
    assert_eq!(make([1, 0], 0, None).unwrap_err(), EntityError::Text);
}

#[test]
fn hatches_dimensions_leaders_arrays_and_viewports_refuse_the_empty() {
    assert_eq!(
        Hatch::new(vec![], Island::Outer, Fill::Pattern(Key(1))).unwrap_err(),
        EntityError::Hatch
    );
    let h = Hatch::new(vec![EntityIx(2), EntityIx(3)], Island::Ignore, Fill::Gradient(Key(4))).unwrap();
    assert_eq!(h.boundary(), &[EntityIx(2), EntityIx(3)]);
    assert_eq!(h.island(), Island::Ignore);
    assert_eq!(h.fill(), Fill::Gradient(Key(4)));

    assert_eq!(
        Dimension::new(DimKind::Radial, vec![], Key(0)).unwrap_err(),
        EntityError::Annotation
    );
    let d = Dimension::new(DimKind::Diameter, vec![Ref::Entity(EntityIx(0))], Key(9)).unwrap();
    assert_eq!((d.kind(), d.refs().len(), d.style()), (DimKind::Diameter, 1, Key(9)));

    assert_eq!(
        Leader::new(vec![p2(0, 0)], None, Key(0)).unwrap_err(),
        EntityError::Annotation
    );
    let l = Leader::new(vec![p2(0, 0), p2(1, 1)], Some(Ref::Entity(EntityIx(1))), Key(5)).unwrap();
    assert_eq!((l.vertices().len(), l.style()), (2, Key(5)));
    assert!(l.target().is_some());

    assert_eq!(Array::new(0, 1, [1, 0], [0, 1]).unwrap_err(), EntityError::Array);
    assert_eq!(Array::new(1, 0, [1, 0], [0, 1]).unwrap_err(), EntityError::Array);
    let a = Array::new(4, 2, [10, 0], [0, -10]).unwrap();
    assert_eq!((a.columns(), a.rows(), a.column_step(), a.row_step()), (4, 2, [10, 0], [0, -10]));

    let w = Box2::new(p2(0, 0), p2(9, 9)).unwrap();
    assert_eq!(
        Viewport::new(w, ViewIx(0), frac(0, 1), frac(0, 1), None).unwrap_err(),
        EntityError::Viewport
    );
    assert_eq!(
        Viewport::new(w, ViewIx(0), frac(-1, 2), frac(0, 1), None).unwrap_err(),
        EntityError::Viewport
    );
    let v = Viewport::new(w, ViewIx(3), frac(1, 100), frac(1, 2), Some(Region::new(sheet()).unwrap())).unwrap();
    assert_eq!((v.view(), v.scale(), v.twist()), (ViewIx(3), frac(1, 100), frac(1, 2)));
    assert_eq!(v.window(), w);
    assert!(v.clip().is_some());
}

#[test]
fn a_hatch_does_not_store_its_lines_and_a_block_keeps_its_own_entities() {
    let shapes = every_shape();
    match &shapes[15] {
        Shape::Block(b) => {
            assert_eq!(b.entities.len(), 1);
            assert_eq!(b.entities[0].attr(), Attr(1));
        }
        other => panic!("{other:?}"),
    }
    match &shapes[16] {
        Shape::Insert(i) => {
            assert_eq!(i.block, BlockIx(0));
            assert_eq!(i.array.unwrap().rows(), 3);
            assert_eq!(i.values.len(), 1);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_elliptic_arc_ends_are_parameters_not_angles() {
    let far = Param::Far;
    let at = Param::At(frac(-1, 2));
    assert_ne!(far, at);
    assert_eq!(at, Param::At(frac(-2, 4)));
}
