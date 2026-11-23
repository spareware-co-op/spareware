//! The drawing types: the exact view map for every grid-axis view, points in lowest terms, shapes
//! checked, one canonical order whatever order the lines come in, and the stored form pinned and
//! read back only in its one form.

// The crate denies native arithmetic, lossy casts and unchecked indexing outside tests
// (Cargo.toml); a test's helpers may use them, and a broken helper must stop the test loudly.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests; the rules are for the crate's own code"
)]

use construct_categories::{DOORS, WALLS};
use construct_drawing::DrawingError;
use construct_drawing::drawing::{
    Drawing, Entity, Line, LineKind, Region, Shape, Source, Turn, ViewPoint,
};
use construct_drawing::view::{Looking, View};
use construct_materials::MaterialId;
use construct_params::Length;
use engine_geometry::value::engine::{CutPoint, GridPlane};
use kit_core::Id;

fn point(x: i128, y: i128, z: i128) -> CutPoint {
    CutPoint {
        num: [x, y, z],
        den: 1,
    }
}

/// Every view's map is the table in `view`'s documentation: right, up and the cut plane, and the
/// view point of (1, 2, 3) steps moved by (10, 20, 30).
#[test]
fn each_grid_axis_view_maps_model_points_exactly() {
    let cut = Length::from_steps(500);
    let rows = [
        (Looking::Down, (11, 22), ([0, 0, 1], 500)),
        (Looking::North, (11, 33), ([0, -1, 0], -500)),
        (Looking::South, (-11, 33), ([0, 1, 0], 500)),
        (Looking::East, (-22, 33), ([-1, 0, 0], -500)),
        (Looking::West, (22, 33), ([1, 0, 0], 500)),
    ];
    for (looking, (x, y), (normal, offset)) in rows {
        let view = View { looking, cut };
        let v = looking.direction();
        let (r, u) = (looking.right(), looking.up());
        // right = v × up, so right × up = -v: the view is seen from the viewer's side.
        let cross = |a: [i128; 3], b: [i128; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        assert_eq!(cross(v, u), r, "{looking:?}");
        assert_eq!(cross(r, u), v.map(|c| -c), "{looking:?}");
        assert_eq!(looking.toward_viewer(), v.map(|c| -c), "{looking:?}");
        assert_eq!(
            view.project(&point(1, 2, 3), [10, 20, 30]).unwrap(),
            ViewPoint::grid(x, y),
            "{looking:?}"
        );
        assert_eq!(
            view.cut_plane().unwrap(),
            GridPlane { normal, offset },
            "{looking:?}"
        );
    }
}

/// A point is kept in lowest terms over a positive denominator, so equal points are equal values;
/// a fraction of a step stays exact.
#[test]
fn points_are_exact_and_in_lowest_terms() {
    assert_eq!(ViewPoint::new(6, -4, -2).unwrap(), ViewPoint::grid(-3, 2));
    assert_eq!(
        ViewPoint::new(3, 5, 6).unwrap(),
        ViewPoint { x: 3, y: 5, den: 6 }
    );
    assert_eq!(ViewPoint::new(1, 1, 0), Err(DrawingError::BadPoint));
    let half = CutPoint {
        num: [1, 3, 0],
        den: 2,
    };
    assert_eq!(
        View::plan(Length::from_steps(0))
            .project(&half, [1, 0, 0])
            .unwrap(),
        ViewPoint { x: 3, y: 3, den: 2 }
    );
}

fn source(element: &str, role: &str) -> Source {
    Source {
        element: Id::parse(element).unwrap(),
        entity: Entity::Face,
        name: format!("name-of-{role}"),
        role: role.to_owned(),
    }
}

fn segment(a: (i128, i128), b: (i128, i128)) -> Shape {
    Shape::Segment {
        start: ViewPoint::grid(a.0, a.1),
        end: ViewPoint::grid(b.0, b.1),
    }
}

/// A small drawing: a cut wall, a projected door edge and an arc, and the wall's region.
fn small() -> (View, Vec<Line>, Vec<Region>) {
    let lines = vec![
        Line {
            kind: LineKind::Projected,
            source: source("door-1", "top"),
            category: DOORS,
            shape: segment((10, 0), (20, 0)),
        },
        Line {
            kind: LineKind::Cut,
            source: source("wall-1", "interior"),
            category: WALLS,
            shape: segment((40, 2), (0, 2)),
        },
        Line {
            kind: LineKind::Cut,
            source: source("wall-1", "exterior"),
            category: WALLS,
            shape: segment((0, 0), (40, 0)),
        },
        Line {
            kind: LineKind::Projected,
            source: source("door-1", "swing"),
            category: DOORS,
            shape: Shape::Arc {
                centre: ViewPoint::grid(10, 0),
                start: ViewPoint::grid(20, 0),
                end: ViewPoint::grid(10, 10),
                turn: Turn::Counterclockwise,
            },
        },
    ];
    let regions = vec![Region {
        element: Id::parse("wall-1").unwrap(),
        category: WALLS,
        material: Some(MaterialId::parse("concrete-regular").unwrap()),
        loops: vec![vec![
            ViewPoint::grid(0, 0),
            ViewPoint::grid(40, 0),
            ViewPoint::grid(40, 2),
            ViewPoint::grid(0, 2),
        ]],
    }];
    (View::plan(Length::from_mm(1200)), lines, regions)
}

/// Lines given in any order make one drawing, cut lines first, then by source: two input orders
/// give equal drawings and equal bytes.
#[test]
fn two_input_orders_give_the_same_drawing_and_bytes() {
    let (view, lines, regions) = small();
    let a = Drawing::new(view, lines.clone(), regions.clone()).unwrap();
    let mut reversed = lines;
    reversed.reverse();
    let b = Drawing::new(view, reversed, regions).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.to_bytes().unwrap(), b.to_bytes().unwrap());
    let order: Vec<(LineKind, &str)> = a
        .lines()
        .iter()
        .map(|l| (l.kind, l.source.role.as_str()))
        .collect();
    assert_eq!(
        order,
        [
            (LineKind::Cut, "exterior"),
            (LineKind::Cut, "interior"),
            (LineKind::Projected, "swing"),
            (LineKind::Projected, "top"),
        ]
    );
}

/// The stored form of the small drawing, pinned byte for byte, and read back to the same drawing.
#[test]
fn the_stored_form_is_pinned_and_reads_back() {
    let (view, lines, regions) = small();
    let drawing = Drawing::new(view, lines, regions).unwrap();
    let bytes = drawing.to_bytes().unwrap();
    let expected = concat!(
        r#"{"format":1,"view":{"looking":"down","cut":"length:q0:7680000"},"lines":["#,
        r#"{"kind":"cut","source":{"element":"wall-1","entity":"face","name":"name-of-exterior","role":"exterior"},"category":100,"shape":{"segment":{"start":{"x":0,"y":0,"den":1},"end":{"x":40,"y":0,"den":1}}}},"#,
        r#"{"kind":"cut","source":{"element":"wall-1","entity":"face","name":"name-of-interior","role":"interior"},"category":100,"shape":{"segment":{"start":{"x":40,"y":2,"den":1},"end":{"x":0,"y":2,"den":1}}}},"#,
        r#"{"kind":"projected","source":{"element":"door-1","entity":"face","name":"name-of-swing","role":"swing"},"category":300,"shape":{"arc":{"centre":{"x":10,"y":0,"den":1},"start":{"x":20,"y":0,"den":1},"end":{"x":10,"y":10,"den":1},"turn":"counterclockwise"}}},"#,
        r#"{"kind":"projected","source":{"element":"door-1","entity":"face","name":"name-of-top","role":"top"},"category":300,"shape":{"segment":{"start":{"x":10,"y":0,"den":1},"end":{"x":20,"y":0,"den":1}}}}"#,
        r#"],"regions":[{"element":"wall-1","category":100,"material":"concrete-regular","loops":[[{"x":0,"y":0,"den":1},{"x":40,"y":0,"den":1},{"x":40,"y":2,"den":1},{"x":0,"y":2,"den":1}]]}]}"#,
    );
    assert_eq!(String::from_utf8(bytes.clone()).unwrap(), expected);
    assert_eq!(Drawing::from_bytes(&bytes).unwrap(), drawing);
}

/// Only the one canonical form reads: lines out of order, a point not in lowest terms, another
/// version, and an unknown field are refused.
#[test]
fn only_the_canonical_form_reads() {
    let (view, lines, regions) = small();
    let text = String::from_utf8(
        Drawing::new(view, lines, regions)
            .unwrap()
            .to_bytes()
            .unwrap(),
    )
    .unwrap();
    let swapped = text.replacen("\"name-of-exterior\"", "\"name-of-zzz\"", 1);
    assert!(matches!(
        Drawing::from_bytes(swapped.as_bytes()),
        Err(DrawingError::Format(_))
    ));
    let unreduced = text.replacen(r#"{"x":0,"y":0,"den":1}"#, r#"{"x":0,"y":0,"den":2}"#, 1);
    assert_eq!(
        Drawing::from_bytes(unreduced.as_bytes()),
        Err(DrawingError::BadPoint)
    );
    let other = text.replacen("\"format\":1", "\"format\":2", 1);
    assert!(matches!(
        Drawing::from_bytes(other.as_bytes()),
        Err(DrawingError::Format(_))
    ));
    let extra = text.replacen("\"format\":1", "\"format\":1,\"x\":0", 1);
    assert!(matches!(
        Drawing::from_bytes(extra.as_bytes()),
        Err(DrawingError::Format(_))
    ));
}

/// A segment of no length, an arc whose ends are not on one circle about its centre, a loop of
/// two points and a loop that does not start at its least point are not drawings.
#[test]
fn shapes_that_are_not_lines_are_refused() {
    let (view, lines, regions) = small();
    let bad_line = |shape: Shape| {
        let mut l = lines.clone();
        l[0].shape = shape;
        Drawing::new(view, l, regions.clone())
    };
    assert!(matches!(
        bad_line(segment((1, 1), (1, 1))),
        Err(DrawingError::BadShape(_))
    ));
    assert!(matches!(
        bad_line(Shape::Arc {
            centre: ViewPoint::grid(0, 0),
            start: ViewPoint::grid(3, 0),
            end: ViewPoint::grid(0, 4),
            turn: Turn::Clockwise,
        }),
        Err(DrawingError::BadShape(_))
    ));
    // A quarter circle of radius 5 steps about a centre half a step off the grid.
    assert!(
        bad_line(Shape::Arc {
            centre: ViewPoint { x: 1, y: 1, den: 2 },
            start: ViewPoint {
                x: 11,
                y: 1,
                den: 2
            },
            end: ViewPoint {
                x: 1,
                y: 11,
                den: 2
            },
            turn: Turn::Counterclockwise,
        })
        .is_ok()
    );
    let bad_region = |ring: Vec<ViewPoint>| {
        let mut r = regions.clone();
        r[0].loops = vec![ring];
        Drawing::new(view, lines.clone(), r)
    };
    assert!(matches!(
        bad_region(vec![ViewPoint::grid(0, 0), ViewPoint::grid(1, 0)]),
        Err(DrawingError::BadShape(_))
    ));
    assert!(matches!(
        bad_region(vec![
            ViewPoint::grid(1, 0),
            ViewPoint::grid(1, 1),
            ViewPoint::grid(0, 0)
        ]),
        Err(DrawingError::BadShape(_))
    ));
}
