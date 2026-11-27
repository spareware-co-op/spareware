//! A picture clipped to a crop outline, exactly: a rectangle keeps the pieces of lines inside it
//! and the hatch lines with them, nothing is drawn along the cut, a line along the outline stays,
//! an outline across a line off the grid gives a fraction, an outline with a notch gives a line
//! two pieces, an arc is kept, dropped or refused by name, either way round gives the same, and the
//! refusals of a bad outline.

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

use construct_categories::{CategoryTable, WALLS};
use construct_drawing::DrawingError;
use construct_drawing::crop::clip;
use construct_drawing::drawing::{
    Drawing, Entity, Line, LineKind, Region, Shape, Source, Turn, ViewPoint,
};
use construct_drawing::hatch::FillPatterns;
use construct_drawing::pens::Styles;
use construct_drawing::picture::{Picture, picture};
use construct_drawing::svg::PER_MM;
use construct_drawing::view::View;
use construct_materials::{MaterialId, Materials};
use construct_params::Length;
use construct_styles::{LineStyles, ObjectStyles, Scale};
use kit_core::Id;

/// A point `x`, `y` millimetres.
fn p(x: i128, y: i128) -> ViewPoint {
    ViewPoint::grid(x * PER_MM, y * PER_MM)
}

/// The corners `(x0, y0)` to `(x1, y1)` millimetres, counter-clockwise from the lower left.
fn rect(x0: i128, y0: i128, x1: i128, y1: i128) -> Vec<ViewPoint> {
    vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
}

/// One concrete wall, 4000 by 200 mm from the origin, printed at 1:100: its four outline edges
/// (bottom, right, top, left) and its hatch.
fn wall() -> Picture {
    let id = Id::parse("wall-a").unwrap();
    let ring = rect(0, 0, 4000, 200);
    let lines = (0..4)
        .map(|e| Line {
            kind: LineKind::Cut,
            source: Source {
                element: id.clone(),
                entity: Entity::Face,
                name: format!("wall-a-{e}"),
                role: format!("e{e}"),
            },
            category: WALLS,
            shape: Shape::Segment {
                start: ring[e],
                end: ring[(e + 1) % 4],
            },
        })
        .collect();
    let region = Region {
        element: id,
        category: WALLS,
        material: Some(MaterialId::parse("concrete-regular").unwrap()),
        loops: vec![ring],
    };
    let drawing = Drawing::new(View::plan(Length::from_mm(1200)), lines, vec![region]).unwrap();
    let (categories, lines, objects, materials, fills) = (
        CategoryTable::new(),
        LineStyles::default_metric(),
        ObjectStyles::default_metric(),
        Materials::default_metric(),
        FillPatterns::default_metric(),
    );
    let styles = Styles {
        looks: None,
        categories: &categories,
        lines: &lines,
        objects: &objects,
        materials: &materials,
        fills: &fills,
    };
    picture(&drawing, styles, Scale::new(100).unwrap()).unwrap()
}

/// The segments of the strokes, each with its ends in order.
fn segments(pic: &Picture) -> Vec<[ViewPoint; 2]> {
    let mut out: Vec<[ViewPoint; 2]> = pic
        .strokes
        .iter()
        .map(|s| {
            let Shape::Segment { start, end } = s.shape else {
                panic!("an arc");
            };
            let mut ends = [start, end];
            ends.sort();
            ends
        })
        .collect();
    out.sort();
    out
}

fn hatch_lines(pic: &Picture) -> Vec<[ViewPoint; 2]> {
    pic.hatches.iter().flat_map(|h| h.lines.clone()).collect()
}

/// A rectangle across the middle of the wall keeps the top and bottom edges between its sides, drops
/// the wall's two ends, draws nothing along the cut, and cuts every hatch line to the sides.
#[test]
fn a_rectangle_keeps_the_pieces_inside_it() {
    let pic = wall();
    let out = clip(&pic, &rect(1000, -100, 3000, 300)).unwrap();
    assert_eq!(
        segments(&out),
        [[p(1000, 0), p(3000, 0)], [p(1000, 200), p(3000, 200)]]
    );
    let (before, after) = (hatch_lines(&pic), hatch_lines(&out));
    assert!(!after.is_empty() && after.len() < before.len());
    for [a, b] in &after {
        for v in [a, b] {
            assert!(
                v.x >= 1000 * PER_MM * v.den && v.x <= 3000 * PER_MM * v.den,
                "{v:?}"
            );
        }
    }
    // The pen, colour and source of a line stay with its pieces.
    assert!(
        out.strokes
            .iter()
            .all(|s| s.width == pic.strokes[0].width && s.element.as_str() == "wall-a")
    );
    assert_eq!(out.hatches[0].width, pic.hatches[0].width);
    assert_eq!(out.scale, pic.scale);
}

/// A crop holding the whole picture changes nothing; one away from it leaves nothing.
#[test]
fn a_crop_around_everything_changes_nothing_and_one_away_leaves_nothing() {
    let pic = wall();
    assert_eq!(clip(&pic, &rect(-500, -500, 5000, 700)).unwrap(), pic);
    let none = clip(&pic, &rect(6000, 0, 7000, 100)).unwrap();
    assert!(none.strokes.is_empty() && none.hatches.is_empty());
    assert_eq!(none.scale, pic.scale);
}

/// An edge of the crop along the wall's bottom edge keeps that edge (the closed region counts) and
/// drops the top.
#[test]
fn a_line_along_the_outline_stays() {
    let out = clip(&wall(), &rect(0, 0, 4000, 100)).unwrap();
    assert_eq!(
        segments(&out),
        [
            [p(0, 0), p(0, 100)],
            [p(0, 0), p(4000, 0)],
            [p(4000, 0), p(4000, 100)]
        ]
    );
}

/// An outline across a line off the axes cuts it at a fraction of the grid, exactly: the triangle
/// (0,0), (1000,0), (0,3000) has the edge 3x + y = 3000, which crosses the wall's top (y = 200) at
/// x = 2800/3 mm, and its bottom at x = 1000 mm.
#[test]
fn an_outline_off_the_axes_cuts_at_an_exact_fraction() {
    let triangle = [p(0, 0), p(1000, 0), p(0, 3000)];
    let out = clip(&wall(), &triangle).unwrap();
    let at = ViewPoint::new(2800 * PER_MM, 200 * 3 * PER_MM, 3).unwrap();
    assert_eq!(at.den, 3);
    let mut want = vec![[p(0, 0), p(0, 200)], [p(0, 0), p(1000, 0)], [p(0, 200), at]];
    want.sort();
    assert_eq!(segments(&out), want);
    // Every hatch end lies in the triangle: 3x + y <= 3000 mm, exactly.
    for [a, b] in hatch_lines(&out) {
        for v in [a, b] {
            assert!(3 * v.x + v.y <= 3000 * PER_MM * v.den, "{v:?}");
        }
    }
}

/// An outline with a notch over the wall's top edge gives that line two pieces; the line below the
/// notch stays whole.
#[test]
fn a_notched_outline_cuts_a_line_into_two_pieces() {
    let u = [
        p(0, -100),
        p(4000, -100),
        p(4000, 500),
        p(3000, 500),
        p(3000, 100),
        p(1000, 100),
        p(1000, 500),
        p(0, 500),
    ];
    let out = clip(&wall(), &u).unwrap();
    let mut want = vec![
        [p(0, 0), p(0, 200)],
        [p(0, 0), p(4000, 0)],
        [p(0, 200), p(1000, 200)],
        [p(3000, 200), p(4000, 200)],
        [p(4000, 0), p(4000, 200)],
    ];
    want.sort();
    assert_eq!(segments(&out), want);
}

/// The same outline round the other way gives the same picture, whichever corner it starts at.
#[test]
fn the_way_round_and_the_first_corner_do_not_matter() {
    let pic = wall();
    let one = rect(1000, -100, 3000, 300);
    let mut back = one.clone();
    back.reverse();
    let mut shifted = one.clone();
    shifted.rotate_left(2);
    let want = clip(&pic, &one).unwrap();
    assert_eq!(clip(&pic, &back).unwrap(), want);
    assert_eq!(clip(&pic, &shifted).unwrap(), want);
    assert_eq!(clip(&pic, &one).unwrap(), want);
}

/// An arc is kept whole inside, dropped outside, and refused where its circle meets the outline.
#[test]
fn an_arc_is_kept_dropped_or_refused_by_name() {
    let mut pic = wall();
    pic.strokes.clear();
    let mut arc = wall().strokes[0].clone();
    arc.shape = Shape::Arc {
        centre: p(500, 500),
        start: p(600, 500),
        end: p(500, 600),
        turn: Turn::Counterclockwise,
    };
    pic.strokes.push(arc);
    let inside = clip(&pic, &rect(0, 0, 1000, 1000)).unwrap();
    assert_eq!(inside.strokes, pic.strokes);
    let outside = clip(&pic, &rect(2000, 2000, 3000, 3000)).unwrap();
    assert!(outside.strokes.is_empty());
    // An edge through the circle: no exact answer, so none is given.
    let meets = clip(&pic, &rect(550, 0, 1000, 1000));
    assert!(matches!(meets, Err(DrawingError::Crop(_))), "{meets:?}");
    // A circle that touches an edge from inside meets it too.
    let touches = clip(&pic, &rect(400, 400, 1000, 1000));
    assert!(matches!(touches, Err(DrawingError::Crop(_))), "{touches:?}");
}

/// An outline that is not one is refused in words.
#[test]
fn a_bad_outline_is_refused() {
    let pic = wall();
    for bad in [
        vec![p(0, 0), p(10, 10)],
        vec![p(0, 0), p(0, 0), p(10, 0), p(0, 10)],
        vec![p(0, 0), p(10, 0), p(20, 0)],
    ] {
        let found = clip(&pic, &bad);
        assert!(matches!(found, Err(DrawingError::BadShape(_))), "{found:?}");
    }
}
