//! Hatches: a family of lines clipped exactly to a region, judged by their count and their end
//! points: across a square with a hole (the hole is not hatched, and a line along an edge draws
//! nothing), across an L-shape (a line through the notch is two pieces, one through its corner is
//! one), at three scales, at a spacing off the grid, over budget, and the default patterns.

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

use construct_drawing::DrawingError;
use construct_drawing::drawing::ViewPoint;
use construct_drawing::hatch::{
    Family, FillPattern, FillPatterns, HATCH_PEN, MOST_HATCH_LINES, hatch,
};
use construct_drawing::svg::PER_MM;
use construct_materials::Materials;
use construct_styles::{Micrometres, Scale};

fn p(x: i128, y: i128) -> ViewPoint {
    ViewPoint::grid(x * PER_MM, y * PER_MM)
}

fn rect(x0: i128, y0: i128, x1: i128, y1: i128) -> Vec<ViewPoint> {
    vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
}

fn hole(x0: i128, y0: i128, x1: i128, y1: i128) -> Vec<ViewPoint> {
    vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0)]
}

/// A pattern of one family: `direction`, `um` printed micrometres apart.
fn one(direction: [i32; 2], um: u32) -> FillPattern {
    FillPattern {
        name: "test".to_owned(),
        families: vec![Family {
            direction,
            spacing: Micrometres::new(um).unwrap(),
        }],
        pen: HATCH_PEN,
    }
}

fn scale(n: u32) -> Scale {
    Scale::new(n).unwrap()
}

/// Lines along x, 1 mm apart on paper at 1:100 (100 mm in the model), across a 1000 mm square
/// with a 200 mm hole in its middle: nine lines inside, the three that meet the hole in two pieces
/// each (the one along the hole's bottom edge too), twelve pieces; the lines along the square's
/// bottom and top edges draw nothing.
#[test]
fn lines_across_a_hole_leave_it_unhatched() {
    let region = vec![rect(0, 0, 1000, 1000), hole(400, 400, 600, 600)];
    let lines = hatch(&region, &one([1, 0], 1000), scale(100)).unwrap();
    let mut expected = Vec::new();
    for k in 1..=9 {
        let y = k * 100;
        if (4..=6).contains(&k) {
            expected.push([p(0, y), p(400, y)]);
            expected.push([p(600, y), p(1000, y)]);
        } else {
            expected.push([p(0, y), p(1000, y)]);
        }
    }
    assert_eq!(lines.len(), 12);
    assert_eq!(lines, expected);
}

/// Lines running down to the right (direction (1, -1)), 1 mm apart along x at 1:100, across an
/// L-shape of two 200 mm arms (4000 mm along x, 3000 mm up y): the lines x + y = 100k mm. Up to
/// k = 4 a line crosses the corner square in one piece (k = 4 passes through the inner corner and
/// stays one); from k = 5 to 31 it crosses both arms apart, two pieces; then the horizontal arm
/// alone to k = 41. 68 pieces in all, and the ends exactly where the edges are.
#[test]
fn lines_across_an_l_shape_are_clipped_exactly() {
    let l = vec![vec![
        p(0, 0),
        p(4000, 0),
        p(4000, 200),
        p(200, 200),
        p(200, 3000),
        p(0, 3000),
    ]];
    let lines = hatch(&l, &one([1, -1], 1000), scale(100)).unwrap();
    // The closed form, line by line: the vertical arm's piece, then the horizontal arm's.
    let mut expected = Vec::new();
    for k in 1..=41 {
        let c = k * 100;
        if c <= 400 {
            expected.push([p(0, c), p(c, 0)]);
            continue;
        }
        if c < 3200 {
            let top = if c > 3000 { p(c - 3000, 3000) } else { p(0, c) };
            expected.push([top, p(200, c - 200)]);
        }
        let end = if c > 4000 { p(4000, c - 4000) } else { p(c, 0) };
        expected.push([p(c - 200, 200), end]);
    }
    assert_eq!(expected.len(), 68);
    assert_eq!(lines, expected);
    // Two of them, written out.
    assert!(lines.contains(&[p(0, 500), p(200, 300)]));
    assert!(lines.contains(&[p(2900, 200), p(3100, 0)]));
}

/// The same pattern prints at the same paper spacing at every scale, so the model spacing follows
/// the scale: across a 1000 mm square, lines 1 mm apart on paper are 100 mm apart at 1:100 (nine
/// inside), 50 mm at 1:50 (19) and 20 mm at 1:20 (49).
#[test]
fn the_spacing_follows_the_scale() {
    let square = vec![rect(0, 0, 1000, 1000)];
    let count = |n| hatch(&square, &one([1, 0], 1000), scale(n)).unwrap().len();
    assert_eq!(count(100), 9);
    assert_eq!(count(50), 19);
    assert_eq!(count(20), 49);
}

/// A spacing that is no whole number of grid steps (1 µm at 1:1 is 6.4 steps) puts every line at
/// its exact fraction: the first, 32/5 steps up a 1 mm square.
#[test]
fn a_spacing_off_the_grid_is_exact() {
    let lines = hatch(&[rect(0, 0, 1, 1)], &one([1, 0], 1), scale(1)).unwrap();
    assert_eq!(lines.len(), 999);
    assert_eq!(
        lines[0],
        [
            ViewPoint::new(0, 32, 5).unwrap(),
            ViewPoint::new(32_000, 32, 5).unwrap()
        ]
    );
}

/// A hatch past its budget is refused whole, with its count, before any line is made.
#[test]
fn a_hatch_past_its_budget_is_refused() {
    let r = hatch(&[rect(0, 0, 100, 100)], &one([1, 0], 1), scale(1));
    assert_eq!(
        r,
        Err(DrawingError::Budget {
            lines: 100_001,
            most: MOST_HATCH_LINES
        })
    );
}

/// Every cut fill the default materials name has a default pattern, and each pattern has a family
/// with a direction.
#[test]
fn every_default_cut_fill_has_a_pattern() {
    let fills = FillPatterns::default_metric();
    let mut named = 0;
    for material in Materials::default_metric().materials() {
        if let Some(fill) = &material.drawing.cut.fill {
            assert!(fills.get(fill.as_str()).is_some(), "{}", fill.as_str());
            named += 1;
        }
    }
    assert!(named > 0);
    assert_eq!(fills.patterns().len(), 9);
    assert_eq!(
        FillPatterns::new(fills.patterns().to_vec()),
        Ok(fills.clone())
    );
    let concrete = fills.get("concrete").unwrap();
    assert_eq!(concrete.families.len(), 2);
    assert_eq!(concrete.families[0].direction, [1, 1]);
    assert_eq!(concrete.families[0].spacing.get(), 1500);
}

/// A table with two patterns of one name, a pattern with no family, or a family with no direction
/// is refused.
#[test]
fn a_bad_fill_table_is_refused() {
    let bad = |patterns: Vec<FillPattern>| {
        matches!(FillPatterns::new(patterns), Err(DrawingError::BadShape(_)))
    };
    assert!(bad(vec![one([1, 0], 1000), one([1, 1], 1000)]));
    let mut empty = one([1, 0], 1000);
    empty.families.clear();
    assert!(bad(vec![empty]));
    assert!(bad(vec![one([0, 0], 1000)]));
}
