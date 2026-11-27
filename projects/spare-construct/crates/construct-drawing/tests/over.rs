//! What a view draws over its model: solid fills, pattern lines and lines of its own detail items,
//! laid on a picture and written by the SVG after everything else, in that order, with the box
//! round the drawing taking them in. Pinned on a drawing with no model, so the bytes are the over
//! layer's alone, worked by hand at 1:100.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    reason = "tests; the rules are for the crate's own code"
)]

use construct_categories::CategoryTable;
use construct_drawing::drawing::{Drawing, LineKind, Shape, Turn, ViewPoint};
use construct_drawing::hatch::FillPatterns;
use construct_drawing::over::{Over, OverFill, OverHatch};
use construct_drawing::pens::Styles;
use construct_drawing::picture::{Output, Stroke, picture};
use construct_drawing::svg::{Svg, to_svg};
use construct_drawing::view::View;
use construct_materials::Materials;
use construct_params::Length;
use construct_styles::patterns::SOLID;
use construct_styles::{Colour, LineStyles, Micrometres, ObjectStyles, Scale};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// A point `x` by `y` millimetres from the origin.
fn mm(x: i64, y: i64) -> ViewPoint {
    ViewPoint::grid(i128::from(x * 6400), i128::from(y * 6400))
}

struct Defaults {
    categories: CategoryTable,
    lines: LineStyles,
    objects: ObjectStyles,
    materials: Materials,
    fills: FillPatterns,
}

impl Defaults {
    fn new() -> Self {
        Self {
            categories: CategoryTable::new(),
            lines: LineStyles::default_metric(),
            objects: ObjectStyles::default_metric(),
            materials: Materials::default_metric(),
            fills: FillPatterns::default_metric(),
        }
    }

    fn styles(&self) -> Styles<'_> {
        Styles {
            looks: None,
            categories: &self.categories,
            lines: &self.lines,
            objects: &self.objects,
            materials: &self.materials,
            fills: &self.fills,
        }
    }
}

fn nothing() -> Drawing {
    Drawing::new(View::plan(Length::from_mm(1200)), Vec::new(), Vec::new()).unwrap()
}

fn stroke(shape: Shape, width: u32, element: &str, role: &str) -> Stroke {
    Stroke {
        shape,
        kind: LineKind::Projected,
        width: Micrometres::new(width).unwrap(),
        colour: Colour(0, 0, 0),
        pattern: SOLID,
        opacity: 100,
        element: id(element),
        role: role.to_owned(),
    }
}

fn square_over() -> Over {
    Over {
        fills: vec![OverFill {
            outline: vec![mm(0, 0), mm(1000, 0), mm(1000, 1000), mm(0, 1000)],
            colour: Colour(255, 255, 255),
            element: id("MR-1"),
        }],
        hatches: vec![OverHatch {
            lines: vec![[mm(0, 0), mm(1000, 1000)]],
            width: Micrometres::new(250).unwrap(),
            colour: Colour(0, 0, 255),
            element: id("FR-1"),
        }],
        strokes: vec![stroke(
            Shape::Segment {
                start: mm(0, 0),
                end: mm(1000, 0),
            },
            500,
            "DL-1",
            "detail line",
        )],
    }
}

#[test]
fn a_picture_with_nothing_over_it_is_the_bytes_it_was() {
    let defaults = Defaults::new();
    let scale = Scale::new(100).unwrap();
    let plain = picture(&nothing(), defaults.styles(), scale).unwrap();
    assert!(plain.over.is_empty());
    assert_eq!(
        Svg.document(&plain).unwrap(),
        to_svg(&nothing(), defaults.styles(), scale).unwrap()
    );
}

#[test]
fn fills_then_pattern_lines_then_lines_are_written_after_the_model() {
    let defaults = Defaults::new();
    let mut printed = picture(&nothing(), defaults.styles(), Scale::new(100).unwrap()).unwrap();
    printed.over = square_over();
    // 1:100: the box is the drawing, 1000 mm square, and 5 mm of paper (500 mm) all round, so 2000
    // mm square on the model and 20 mm on the sheet; a pen of 250 micrometres prints 25 mm wide on
    // the model, 500 micrometres 50 mm; y runs up, so it is written negated.
    let expected = concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20mm\" height=\"20mm\" viewBox=\"-500 -1500 2000 2000\">\n",
        "<path d=\"M 0 0 L 1000 0 L 1000 -1000 L 0 -1000 Z\" fill=\"#ffffff\" stroke=\"none\" data-element=\"MR-1\" data-role=\"fill\"/>\n",
        "<path d=\"M 0 0 L 1000 -1000\" fill=\"none\" stroke=\"#0000ff\" stroke-width=\"25\" stroke-linecap=\"butt\" data-element=\"FR-1\" data-role=\"pattern\"/>\n",
        "<line x1=\"0\" y1=\"0\" x2=\"1000\" y2=\"0\" stroke=\"#000000\" stroke-width=\"50\" stroke-linecap=\"round\" data-element=\"DL-1\" data-role=\"detail line\"/>\n",
        "</svg>\n"
    );
    assert_eq!(Svg.document(&printed).unwrap(), expected);
    // The same picture writes the same bytes through the Output trait.
    assert_eq!(Svg.write(&printed).unwrap(), expected.as_bytes());
}

#[test]
fn an_arc_over_the_model_is_written_as_a_path_and_takes_the_box_with_it() {
    let defaults = Defaults::new();
    let mut printed = picture(&nothing(), defaults.styles(), Scale::new(100).unwrap()).unwrap();
    printed.over = Over {
        strokes: vec![stroke(
            Shape::Arc {
                centre: mm(0, 0),
                start: mm(1000, 0),
                end: mm(0, 1000),
                turn: Turn::Counterclockwise,
            },
            350,
            "DL-2",
            "detail line",
        )],
        ..Over::none()
    };
    let svg = Svg.document(&printed).unwrap();
    assert!(svg.contains("<path d=\"M 1000 0 A 1000.000000000 1000.000000000 0 0 1 0 -1000\""));
    assert!(svg.contains("data-element=\"DL-2\""));
    // The arc's ends are 1000 mm from the origin: the box spans them and the margin.
    assert!(svg.contains("viewBox=\"-500 -1500 2000 2000\""), "{svg}");
}
