//! Tests of `geom_2d::snap` (piece G198): snaps and tracking resolving to exact points.
//!
//! What this file proves: every `DIMASSOC` kind (end, mid, centre, node, quadrant, intersection,
//! insertion, perpendicular, tangent, nearest, apparent intersection, parallel, start) and the
//! tracking kinds give the exact point the construction names, checked with exact signs on the
//! one-root number, never with a tolerance; a crossing off an entity's extent is refused; a
//! reference that does not resolve is refused with its resolution; the codes of the thirteen kinds
//! round-trip.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_2d::carrier::{Carriers, Circle2, Curve2};
use geom_2d::entity::{Attr, Ends, Entity, Frac, Param, Polyline, Shape, Text};
use geom_2d::point::{Place, Pt2};
use geom_2d::snap::{Drawing, ExactParam, SnapError, SnapKind, SnapRef, Track};
use geom_name::NameGraph;
use geom_name::resolve::{
    Body, ElementId, EntityKind, Lookup, Reference, Resolution, Signature, Want,
};
use geom_num::Sign;
use geom_num::arc::{Circle2 as GridCircle, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::conic::Ellipse2;
use geom_num::grid::{P2, Steps};
use geom_num::one_root::Crossing2;
use geom_op::name::{NameList, NameSet, Role, TopoName};
use geom_op::{NameId, OpId, OpPath};

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn segment(a: (i64, i64), b: (i64, i64)) -> Shape {
    Shape::Line {
        carrier: Carrier::through(p(a.0, a.1), p(b.0, b.1)).unwrap(),
        ends: Ends::Segment(Pt2::Grid(p(a.0, a.1)), Pt2::Grid(p(b.0, b.1))),
    }
}

fn circle(cx: i64, cy: i64, r: i128) -> Circle2 {
    Circle2::Grid(GridCircle::new(p(cx, cy), r).unwrap())
}

/// A drawing of `shapes`, each named, with the lookup that resolves them.
struct Scene {
    graph: NameGraph,
    names: Vec<NameId>,
    drawing: Drawing,
    set: NameSet,
}

impl Scene {
    fn new(shapes: Vec<Shape>) -> Scene {
        let mut graph = NameGraph::new();
        let mut names = Vec::new();
        for n in 0..shapes.len() {
            let n = u8::try_from(n).unwrap();
            let id = graph
                .intern(TopoName::Created {
                    op: OpPath::new(OpId::from_bytes([n; 16])),
                    role: Role::new("edge", []).unwrap(),
                    from: NameList::new([]),
                })
                .unwrap();
            names.push(id);
        }
        let items = names
            .iter()
            .copied()
            .zip(shapes)
            .map(|(name, shape)| (name, Entity::new(Attr(0), shape)))
            .collect();
        let set = NameSet::new(names.iter().copied()).unwrap();
        Scene {
            graph,
            names,
            drawing: Drawing::new(items, Vec::new()).unwrap(),
            set,
        }
    }

    fn reference(&self, i: usize) -> Reference {
        Reference {
            doc: None,
            element: ElementId([1; 16]),
            name: self.names[i],
            kind: EntityKind::Edge,
            want: Want::One,
            signature: Signature::default(),
        }
    }

    fn resolve(&self, snap: &SnapRef, carriers: &mut Carriers) -> Result<Pt2, SnapError> {
        let lookup = Lookup::Found {
            body: Body {
                names: &self.set,
                carriers: &[],
                history: &[],
                graph: &self.graph,
            },
            version: None,
        };
        snap.resolve(&self.drawing, &lookup, carriers)
    }

    fn snap(&self, kind: SnapKind, on: usize) -> SnapRef {
        let mut s = SnapRef::new(kind, Pt2::Grid(p(0, 0)));
        s.on = Some(self.reference(on));
        s
    }
}

/// The one-root number a point is.
fn at(point: &Pt2, carriers: &Carriers) -> Crossing2 {
    match point.place(carriers).unwrap() {
        Place::OneRoot(z) => z,
        Place::Conic { .. } => panic!("a conic root"),
    }
}

/// Whether `point` is exactly the rational point (x, y) / w.
fn is(point: &Pt2, carriers: &Carriers, x: i128, y: i128, w: i128) -> bool {
    let z = at(point, carriers);
    z.same(&Crossing2::of_point(&Rational2::new(x, y, w).unwrap()).unwrap())
        .unwrap()
}

#[test]
fn the_codes_of_the_thirteen_kinds_round_trip() {
    for code in 1..=13 {
        let kind = SnapKind::from_dimassoc(code).unwrap();
        assert_eq!(kind.dimassoc(), Some(code));
    }
    assert_eq!(SnapKind::from_dimassoc(0), None);
    assert_eq!(SnapKind::from_dimassoc(14), None);
    assert_eq!(SnapKind::Extension.dimassoc(), None);
}

#[test]
fn ends_mid_and_node_of_a_segment_and_a_point() {
    let s = Scene::new(vec![segment((1, 2), (6, 9)), Shape::Point(p(4, 5))]);
    let mut c = Carriers::new();
    let start = s.resolve(&s.snap(SnapKind::Start, 0), &mut c).unwrap();
    assert!(is(&start, &c, 1, 2, 1));
    let end = s.resolve(&s.snap(SnapKind::End, 0), &mut c).unwrap();
    assert!(is(&end, &c, 6, 9, 1));
    // (1 + 6) / 2, (2 + 9) / 2: a rational point, not a grid one.
    let mid = s.resolve(&s.snap(SnapKind::Mid, 0), &mut c).unwrap();
    assert!(is(&mid, &c, 7, 11, 2));
    assert!(matches!(mid, Pt2::Rational(_)));
    let node = s.resolve(&s.snap(SnapKind::Node, 1), &mut c).unwrap();
    assert!(is(&node, &c, 4, 5, 1));
    // A node snap on a segment is refused, not guessed.
    assert!(matches!(
        s.resolve(&s.snap(SnapKind::Node, 0), &mut c),
        Err(SnapError::Unsupported(SnapKind::Node, _))
    ));
}

#[test]
fn the_centre_and_quadrants_of_a_circle_and_of_an_ellipse() {
    let ellipse = Ellipse2::new(p(10, 20), [8, 6], 1, 2).unwrap();
    let s = Scene::new(vec![
        Shape::Circle(circle(3, 4, 10)),
        Shape::Ellipse(ellipse),
    ]);
    let mut c = Carriers::new();
    let centre = s.resolve(&s.snap(SnapKind::Center, 0), &mut c).unwrap();
    assert!(is(&centre, &c, 3, 4, 1));
    for (pick, (x, y)) in [(13, 4), (3, 14), (-7, 4), (3, -6)].into_iter().enumerate() {
        let mut q = s.snap(SnapKind::Quadrant, 0);
        q.pick = u8::try_from(pick).unwrap();
        let point = s.resolve(&q, &mut c).unwrap();
        assert!(is(&point, &c, x, y, 1), "quadrant {pick}");
    }
    assert!(matches!(
        {
            let mut q = s.snap(SnapKind::Quadrant, 0);
            q.pick = 4;
            s.resolve(&q, &mut c)
        },
        Err(SnapError::Param)
    ));
    // The ellipse's axes end at C ± (8, 6) and C ± (p / q) (-6, 8) = C ± (-3, 4).
    let want = [(18, 26), (7, 24), (2, 14), (13, 16)];
    for (pick, (x, y)) in want.into_iter().enumerate() {
        let mut q = s.snap(SnapKind::Quadrant, 1);
        q.pick = u8::try_from(pick).unwrap();
        let point = s.resolve(&q, &mut c).unwrap();
        assert!(is(&point, &c, x, y, 1), "ellipse quadrant {pick}");
        assert_eq!(
            at(&point, &c).against_ellipse(&ellipse).unwrap(),
            Sign::Zero
        );
    }
    let centre = s.resolve(&s.snap(SnapKind::Center, 1), &mut c).unwrap();
    assert!(is(&centre, &c, 10, 20, 1));
}

#[test]
fn a_circle_whose_radius_is_not_whole_has_quadrants_that_are_one_root_points() {
    // About (1, 1) / 2 through (3, 0): radius squared (5/2)² + (1/2)² = 13/2.
    let round = Round::new(p(3, 0), Rational2::new(1, 1, 2).unwrap());
    let s = Scene::new(vec![Shape::Circle(Circle2::Round(round))]);
    let mut c = Carriers::new();
    let centre = s.resolve(&s.snap(SnapKind::Center, 0), &mut c).unwrap();
    assert!(is(&centre, &c, 1, 1, 2));
    for pick in 0..4_u8 {
        let mut q = s.snap(SnapKind::Quadrant, 0);
        q.pick = pick;
        let point = s.resolve(&q, &mut c).unwrap();
        assert!(matches!(point, Pt2::Crossing { .. }));
        let z = at(&point, &c);
        assert_eq!(z.against_round(&round).unwrap(), Sign::Zero);
        // On the line through the centre along its axis.
        let axis = if pick % 2 == 0 {
            Carrier::new(0, 2, 1).unwrap()
        } else {
            Carrier::new(2, 0, 1).unwrap()
        };
        assert_eq!(z.side(&axis).unwrap(), Sign::Zero);
    }
}

#[test]
fn the_middle_of_an_arc_is_a_one_root_point_on_the_bisector() {
    let arc = |s: (i64, i64), e: (i64, i64)| Shape::Arc {
        circle: circle(0, 0, 10),
        start: Pt2::Grid(p(s.0, s.1)),
        end: Pt2::Grid(p(e.0, e.1)),
    };
    // A quarter turn from east to north, a three-quarter turn from east to south, and a half turn.
    let s = Scene::new(vec![
        arc((10, 0), (0, 10)),
        arc((10, 0), (0, -10)),
        arc((10, 0), (-10, 0)),
    ]);
    let mut c = Carriers::new();
    let round = GridCircle::new(p(0, 0), 10).unwrap();
    for (i, (diagonal, x_sign, y_sign)) in [
        (
            Carrier::new(1, -1, 0).unwrap(),
            Sign::Positive,
            Sign::Positive,
        ),
        (
            Carrier::new(1, 1, 0).unwrap(),
            Sign::Negative,
            Sign::Positive,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mid = s.resolve(&s.snap(SnapKind::Mid, i), &mut c).unwrap();
        let z = at(&mid, &c);
        assert_eq!(z.against_circle(&round).unwrap(), Sign::Zero);
        assert_eq!(z.side(&diagonal).unwrap(), Sign::Zero, "arc {i}");
        let x = z.side(&Carrier::new(1, 0, 0).unwrap()).unwrap();
        let y = z.side(&Carrier::new(0, 1, 0).unwrap()).unwrap();
        assert_eq!((x, y), (x_sign, y_sign), "arc {i}");
    }
    // The half turn's middle is straight north.
    let mid = s.resolve(&s.snap(SnapKind::Mid, 2), &mut c).unwrap();
    assert!(
        is(&mid, &c, 0, 10, 1)
            || at(&mid, &c).side(&Carrier::new(1, 0, 0).unwrap()).unwrap() == Sign::Zero
    );
}

#[test]
fn intersections_are_exact_and_off_extent_ones_are_refused() {
    let s = Scene::new(vec![
        segment((0, 0), (10, 10)),
        segment((0, 10), (10, 0)),
        segment((0, 10), (4, 6)),
        Shape::Circle(circle(0, 0, 5)),
        segment((-10, 0), (10, 0)),
    ]);
    let mut c = Carriers::new();
    // Two segments crossing at (5, 5).
    let mut x = s.snap(SnapKind::Intersection, 0);
    x.with = Some(s.reference(1));
    let point = s.resolve(&x, &mut c).unwrap();
    assert!(is(&point, &c, 5, 5, 1));
    // The same lines, with the second cut short of the crossing: refused.
    let mut x = s.snap(SnapKind::Intersection, 0);
    x.with = Some(s.reference(2));
    assert_eq!(s.resolve(&x, &mut c).unwrap_err(), SnapError::OffExtent);
    // An apparent intersection is the same crossing of the projections.
    let mut x = s.snap(SnapKind::ApparentIntersection, 0);
    x.with = Some(s.reference(1));
    assert!(is(&s.resolve(&x, &mut c).unwrap(), &c, 5, 5, 1));
    // A segment through a circle: both crossings, on the circle and the line.
    let round = GridCircle::new(p(0, 0), 5).unwrap();
    let mut seen = Vec::new();
    for pick in 0..2 {
        let mut x = s.snap(SnapKind::Intersection, 4);
        x.with = Some(s.reference(3));
        x.pick = pick;
        let point = s.resolve(&x, &mut c).unwrap();
        let z = at(&point, &c);
        assert_eq!(z.against_circle(&round).unwrap(), Sign::Zero);
        assert_eq!(z.side(&Carrier::new(0, 1, 0).unwrap()).unwrap(), Sign::Zero);
        seen.push(is(&point, &c, 5, 0, 1) || is(&point, &c, -5, 0, 1));
    }
    assert_eq!(seen, [true, true]);
}

#[test]
fn an_arcs_extent_is_decided_exactly() {
    // The arc from east to north on the circle of radius 5; the line y = 3 meets the circle at
    // x = ±4, and only (4, 3) is on the arc.
    let s = Scene::new(vec![
        Shape::Arc {
            circle: circle(0, 0, 5),
            start: Pt2::Grid(p(5, 0)),
            end: Pt2::Grid(p(0, 5)),
        },
        segment((-9, 3), (9, 3)),
    ]);
    let mut c = Carriers::new();
    let mut found = Vec::new();
    for pick in 0..2 {
        let mut x = s.snap(SnapKind::Intersection, 1);
        x.with = Some(s.reference(0));
        x.pick = pick;
        found.push(s.resolve(&x, &mut c));
    }
    assert_eq!(found.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        found
            .iter()
            .filter(|r| matches!(r, Err(SnapError::OffExtent)))
            .count(),
        1
    );
    let ok = found.into_iter().find_map(Result::ok).unwrap();
    assert!(is(&ok, &c, 4, 3, 1));
}

#[test]
fn the_foot_of_a_perpendicular_is_rational_on_a_line_and_one_root_on_a_circle() {
    let s = Scene::new(vec![
        segment((0, 0), (10, 10)),
        Shape::Circle(circle(0, 0, 5)),
    ]);
    let mut c = Carriers::new();
    let mut foot = s.snap(SnapKind::Perpendicular, 0);
    foot.from = Some(Pt2::Grid(p(0, 10)));
    let point = s.resolve(&foot, &mut c).unwrap();
    assert!(is(&point, &c, 5, 5, 1));
    let round = GridCircle::new(p(0, 0), 5).unwrap();
    for (pick, toward) in [(0, Sign::Positive), (1, Sign::Negative)] {
        let mut foot = s.snap(SnapKind::Perpendicular, 1);
        foot.from = Some(Pt2::Grid(p(3, 4)));
        foot.pick = pick;
        let point = s.resolve(&foot, &mut c).unwrap();
        let z = at(&point, &c);
        assert_eq!(z.against_circle(&round).unwrap(), Sign::Zero);
        // On the line through the centre and the point, on the point's side or the other.
        assert_eq!(
            z.side(&Carrier::new(4, -3, 0).unwrap()).unwrap(),
            Sign::Zero
        );
        assert_eq!(
            z.side(&Carrier::new(3, 4, 0).unwrap()).unwrap(),
            toward,
            "pick {pick}"
        );
    }
}

#[test]
fn tangent_points_lie_on_the_polar_line_of_the_chosen_point() {
    let ellipse = Ellipse2::new(p(0, 0), [10, 0], 1, 2).unwrap();
    let s = Scene::new(vec![
        Shape::Circle(circle(0, 0, 10)),
        Shape::Ellipse(ellipse),
    ]);
    let mut c = Carriers::new();
    let round = GridCircle::new(p(0, 0), 10).unwrap();
    for pick in 0..2 {
        // From (20, 0): the polar of a circle of radius 10 is x = 100 / 20 = 5.
        let mut t = s.snap(SnapKind::Tangent, 0);
        t.from = Some(Pt2::Grid(p(20, 0)));
        t.pick = pick;
        let point = s.resolve(&t, &mut c).unwrap();
        let z = at(&point, &c);
        assert_eq!(z.against_circle(&round).unwrap(), Sign::Zero);
        assert_eq!(z.side(&Carrier::new(1, 0, 5).unwrap()).unwrap(), Sign::Zero);
        // The same for the ellipse of semi-axes 10 and 5: x = a² / 20 = 5.
        let mut t = s.snap(SnapKind::Tangent, 1);
        t.from = Some(Pt2::Grid(p(20, 0)));
        t.pick = pick;
        let point = s.resolve(&t, &mut c).unwrap();
        let z = at(&point, &c);
        assert_eq!(z.against_ellipse(&ellipse).unwrap(), Sign::Zero);
        assert_eq!(z.side(&Carrier::new(1, 0, 5).unwrap()).unwrap(), Sign::Zero);
    }
}

#[test]
fn the_nearest_point_is_at_the_stored_parameter_or_radial() {
    let s = Scene::new(vec![
        segment((0, 0), (8, 4)),
        Shape::Circle(circle(0, 0, 5)),
    ]);
    let mut c = Carriers::new();
    let mut n = s.snap(SnapKind::Nearest, 0);
    n.at = Some(ExactParam::new(3, 3).unwrap());
    // 3 / 8 of the way: (3, 3 / 2).
    assert!(is(&s.resolve(&n, &mut c).unwrap(), &c, 6, 3, 2));
    n.at = Some(ExactParam::new(9, 3).unwrap());
    assert_eq!(s.resolve(&n, &mut c).unwrap_err(), SnapError::Param);
    assert_eq!(ExactParam::new(1, 41), Err(SnapError::Param));
    let mut n = s.snap(SnapKind::Nearest, 1);
    n.from = Some(Pt2::Grid(p(30, 40)));
    let z = at(&s.resolve(&n, &mut c).unwrap(), &c);
    // Radially towards (30, 40): the point (3, 4).
    assert!(
        z.same(&Crossing2::of_point(&Rational2::new(3, 4, 1).unwrap()).unwrap())
            .unwrap()
    );
}

#[test]
fn tracking_lines_cross_entities_each_other_and_stand_at_parameters() {
    let s = Scene::new(vec![segment((4, -20), (4, 20)), segment((0, 0), (6, 3))]);
    let mut c = Carriers::new();
    // Through the origin in direction (1, 1) to the line x = 4.
    let mut d = SnapRef::new(SnapKind::Direction, Pt2::Grid(p(0, 0)));
    d.track = Some(Track {
        through: Pt2::Grid(p(0, 0)),
        direction: [1, 1],
    });
    d.with = Some(s.reference(0));
    assert!(is(&s.resolve(&d, &mut c).unwrap(), &c, 4, 4, 1));
    // Where it crosses another tracking line, x + y = 8 along (1, -1) through (8, 0).
    let mut d = SnapRef::new(SnapKind::Direction, Pt2::Grid(p(0, 0)));
    d.track = Some(Track {
        through: Pt2::Grid(p(0, 0)),
        direction: [1, 1],
    });
    d.track2 = Some(Track {
        through: Pt2::Grid(p(8, 0)),
        direction: [-1, 1],
    });
    assert!(is(&s.resolve(&d, &mut c).unwrap(), &c, 4, 4, 1));
    // At a dyadic parameter: (0, 0) + (3, 2) * 5 / 4.
    let mut d = SnapRef::new(SnapKind::Direction, Pt2::Grid(p(0, 0)));
    d.track = Some(Track {
        through: Pt2::Grid(p(0, 0)),
        direction: [3, 2],
    });
    d.at = Some(ExactParam::new(5, 2).unwrap());
    assert!(is(&s.resolve(&d, &mut c).unwrap(), &c, 15, 10, 4));
    // A parallel to the second segment (direction (6, 3) ~ (2, 1)) through (0, 5), to x = 4.
    let mut par = s.snap(SnapKind::Parallel, 1);
    par.from = Some(Pt2::Grid(p(0, 5)));
    par.with = Some(s.reference(0));
    assert!(is(&s.resolve(&par, &mut c).unwrap(), &c, 4, 7, 1));
    // No direction at all is refused.
    let mut d = SnapRef::new(SnapKind::Direction, Pt2::Grid(p(0, 0)));
    d.track = Some(Track {
        through: Pt2::Grid(p(0, 0)),
        direction: [0, 0],
    });
    d.at = Some(ExactParam::new(1, 0).unwrap());
    assert_eq!(s.resolve(&d, &mut c).unwrap_err(), SnapError::NoDirection);
}

#[test]
fn an_extension_beyond_a_segments_end_is_exact() {
    let s = Scene::new(vec![segment((0, 0), (4, 2))]);
    let mut c = Carriers::new();
    let mut e = s.snap(SnapKind::Extension, 0);
    // Past the end by a half: t = 3 / 2.
    e.at = Some(ExactParam::new(3, 1).unwrap());
    assert!(is(&s.resolve(&e, &mut c).unwrap(), &c, 6, 3, 1));
    // Before the start: t = -1 / 4.
    e.at = Some(ExactParam::new(-1, 2).unwrap());
    assert!(is(&s.resolve(&e, &mut c).unwrap(), &c, -2, -1, 2));
}

#[test]
fn the_centroid_of_a_closed_loop_is_rational() {
    // A right triangle (0,0), (6,0), (0,3): centroid (2, 1).
    let triangle = Polyline::new(
        vec![p(0, 0), p(6, 0), p(0, 3)],
        vec![Frac::whole(0).unwrap(); 3],
        None,
        true,
    )
    .unwrap();
    // An L-shape's centroid is not on a grid point: (0,0) (3,0) (3,1) (1,1) (1,3) (0,3).
    let l_shape = Polyline::new(
        vec![p(0, 0), p(3, 0), p(3, 1), p(1, 1), p(1, 3), p(0, 3)],
        vec![Frac::whole(0).unwrap(); 6],
        None,
        true,
    )
    .unwrap();
    let s = Scene::new(vec![Shape::Polyline(triangle), Shape::Polyline(l_shape)]);
    let mut c = Carriers::new();
    assert!(is(
        &s.resolve(&s.snap(SnapKind::Centroid, 0), &mut c).unwrap(),
        &c,
        2,
        1,
        1
    ));
    // A 3 by 1 bar and a 1 by 2 bar: area 5, moments 3 * 1.5 + 2 * 0.5 = 5.5 and 3 * 0.5 + 2 * 2 = 5.5.
    assert!(is(
        &s.resolve(&s.snap(SnapKind::Centroid, 1), &mut c).unwrap(),
        &c,
        11,
        11,
        10
    ));
}

#[test]
fn polylines_text_and_elliptic_arcs_give_their_own_points() {
    let line = Polyline::new(
        vec![p(0, 0), p(4, 0), p(4, 6)],
        vec![Frac::whole(0).unwrap(); 2],
        None,
        false,
    )
    .unwrap();
    let text = Text::new(
        p(7, 8),
        geom_2d::entity::Attach::BottomLeft,
        [1, 0],
        Steps(10),
        None,
        String::from("x"),
    )
    .unwrap();
    let ellipse = Ellipse2::new(p(10, 20), [8, 0], 1, 2).unwrap();
    let s = Scene::new(vec![
        Shape::Polyline(line),
        Shape::Text(text),
        Shape::EllipticArc {
            ellipse,
            start: Param::At(Frac::whole(0).unwrap()),
            end: Param::Far,
        },
    ]);
    let mut c = Carriers::new();
    assert!(is(
        &s.resolve(&s.snap(SnapKind::Start, 0), &mut c).unwrap(),
        &c,
        0,
        0,
        1
    ));
    assert!(is(
        &s.resolve(&s.snap(SnapKind::End, 0), &mut c).unwrap(),
        &c,
        4,
        6,
        1
    ));
    let mut vertex = s.snap(SnapKind::End, 0);
    vertex.sub = Some(1);
    assert!(is(&s.resolve(&vertex, &mut c).unwrap(), &c, 4, 0, 1));
    let mut mid = s.snap(SnapKind::Mid, 0);
    mid.sub = Some(1);
    assert!(is(&s.resolve(&mid, &mut c).unwrap(), &c, 4, 3, 1));
    assert!(is(
        &s.resolve(&s.snap(SnapKind::Insertion, 1), &mut c).unwrap(),
        &c,
        7,
        8,
        1
    ));
    // s = 0 is the major axis's near end, C + (8, 0); Far is its far end, C - (8, 0).
    assert!(is(
        &s.resolve(&s.snap(SnapKind::Start, 2), &mut c).unwrap(),
        &c,
        18,
        20,
        1
    ));
    assert!(is(
        &s.resolve(&s.snap(SnapKind::End, 2), &mut c).unwrap(),
        &c,
        2,
        20,
        1
    ));
}

#[test]
fn a_reference_that_does_not_resolve_is_refused_with_its_resolution() {
    let s = Scene::new(vec![segment((0, 0), (1, 1))]);
    let mut c = Carriers::new();
    // A name the body does not hold.
    let mut other = NameGraph::new();
    let stray = other
        .intern(TopoName::Created {
            op: OpPath::new(OpId::from_bytes([200; 16])),
            role: Role::new("edge", []).unwrap(),
            from: NameList::new([]),
        })
        .unwrap();
    let mut snap = s.snap(SnapKind::Start, 0);
    if let Some(r) = snap.on.as_mut() {
        r.name = stray;
    }
    match s.resolve(&snap, &mut c) {
        Err(SnapError::Unresolved(Resolution::Unresolved { .. })) => {}
        other => panic!("{other:?}"),
    }
    // A snap with no entity.
    let snap = SnapRef::new(SnapKind::Start, Pt2::Grid(p(0, 0)));
    assert!(matches!(
        s.resolve(&snap, &mut c),
        Err(SnapError::Missing(_))
    ));
}

#[test]
fn the_cached_point_is_never_read() {
    let s = Scene::new(vec![segment((0, 0), (6, 0))]);
    let mut c = Carriers::new();
    let mut snap = s.snap(SnapKind::End, 0);
    snap.cached = Pt2::Grid(p(999, 999));
    assert!(is(&s.resolve(&snap, &mut c).unwrap(), &c, 6, 0, 1));
    let _ = Curve2::Line(Carrier::new(0, 1, 0).unwrap());
}
