//! Angular, radial, diameter, arc length and ordinate dimensions, and making a string's segments
//! equal, worked by hand: 90 degrees between perpendicular walls, a 3:4:5 corner, a radius and a
//! length on a grid arc, ordinate values along a run of walls, a three-segment string made equal.
//! Written against this crate's own types only: the references' places come from a table
//! ([`Located`]), so the same tests judge whatever answers the locator.

#![allow(
    clippy::unwrap_used,
    clippy::assert_is_empty,
    clippy::too_many_lines,
    clippy::arithmetic_side_effects,
    clippy::panic,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_dimensions::{
    Curve, Dimension, DimensionError, Direction, Feature, Figure, Kind as DimensionKind, Layout,
    Located, Locus, Missing, Move, Placing, PlainText, REFERENCE_MISSING, REFERENCE_PARALLEL, Ray,
    Reference, Side, Sweep, equalize, labels, measure, placed,
};
use construct_grids::Point;
use construct_params::{Angle, Length, Units};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

const DEGREE: i64 = 9_000_000;
const STEPS_PER_MM: i64 = 6_400;

fn line(wall: &str) -> Reference {
    Reference::new(id(wall), Feature::WallLine)
}

fn grid(name: &str) -> Reference {
    Reference::new(id(name), Feature::GridLine)
}

/// A wall's line from `(x0, y0)` to `(x1, y1)` millimetres, the way it is drawn.
fn wall(x0: i32, y0: i32, x1: i32, y1: i32) -> Locus {
    Locus::line(Point::mm(x0, y0), mm(x1 - x0), mm(y1 - y0))
}

fn angular(first: Ray, second: Ray, radius: i32, references: Vec<Reference>) -> Dimension {
    Dimension::new(
        id("D-A"),
        id("V-1"),
        Layout::angular(first, second, mm(radius)).unwrap(),
        references,
    )
    .unwrap()
}

fn shown_angle(dimension: &Dimension, located: &Located) -> (Option<Angle>, bool) {
    let measured = measure(dimension, located);
    let [segment] = measured.segments.as_slice() else {
        panic!("one value")
    };
    assert_eq!(segment.value, None);
    (segment.angle, segment.exact)
}

#[test]
fn perpendicular_walls_make_ninety_degrees_exactly() {
    // W-1 runs east from the origin; W-2 runs north from (4000, 0).
    let located = Located::new()
        .with(line("W-1"), Ok(wall(0, 0, 4000, 0)))
        .with(line("W-2"), Ok(wall(4000, 0, 4000, 3000)));
    // The angle inside the corner: back along W-1 (west) and up W-2 (north).
    let dimension = angular(
        Ray::Backward,
        Ray::Forward,
        800,
        vec![line("W-1"), line("W-2")],
    );
    assert_eq!(
        shown_angle(&dimension, &located),
        (Some(Angle::from_units(90 * DEGREE)), true)
    );
    let measured = measure(&dimension, &located);
    assert!(measured.warnings.is_empty());
    assert_eq!(measured.kind, DimensionKind::Angular);
    // Drawn as an arc of 800 mm about the corner, from the west ray to the north ray: clockwise.
    let Figure::Arc {
        centre,
        radius,
        from,
        to,
        middle,
        anticlockwise,
        large,
    } = measured.figure
    else {
        panic!("an arc")
    };
    assert_eq!(centre, Point::mm(4000, 0));
    assert_eq!(radius, mm(800));
    assert_eq!(from, Point::mm(3200, 0));
    assert_eq!(to, Point::mm(4000, 800));
    assert!(!anticlockwise);
    assert!(!large);
    // Its middle is at 135 degrees: 800 / sqrt 2 = 565.685 mm each way, 3,620,386.72 steps, to
    // the step.
    assert_eq!(
        middle,
        Point::new(
            Length::from_steps(4000 * STEPS_PER_MM - 3_620_387),
            Length::from_steps(3_620_387)
        )
    );
    // Each witness line runs along its wall from its drawn point to the arc.
    assert_eq!(measured.witnesses[0].unwrap().to, Point::mm(3200, 0));
    assert_eq!(measured.witnesses[1].unwrap().from, Point::mm(4000, 0));
    // The text is the person's units: degrees to two places.
    let shown = labels(&measured, &Units::metric(), &PlainText::standard()).unwrap();
    assert_eq!(shown[0].text, "90.00°");
    assert_eq!(shown[0].fits, None);
}

#[test]
fn a_three_four_five_corner_is_square_and_its_other_angles_are_the_nearest_unit() {
    // A triangle of walls 3000, 4000 and 5000 mm long, off the axes: A runs 3000 mm along (3, 4)
    // from the origin, B 4000 mm along (-4, 3) from its end, and C, 5000 mm, closes it.
    let located = Located::new()
        .with(line("A"), Ok(wall(0, 0, 1800, 2400)))
        .with(line("B"), Ok(wall(1800, 2400, -1400, 4800)))
        .with(line("C"), Ok(wall(-1400, 4800, 0, 0)));
    // At the corner of the legs: back along A and on along B.
    let corner = angular(Ray::Backward, Ray::Forward, 500, vec![line("A"), line("B")]);
    assert_eq!(
        shown_angle(&corner, &located),
        (Some(Angle::from_units(90 * DEGREE)), true)
    );
    // At the origin, between A and C (C arrives at the origin, so back along it): 4000 opposite,
    // 3000 beside, so the angle's tangent is 4/3: 53.1301023...°, the nearest unit.
    let at_origin = angular(Ray::Forward, Ray::Backward, 500, vec![line("A"), line("C")]);
    let (found, exact) = shown_angle(&at_origin, &located);
    assert_eq!(found, Some(Angle::from_units(478_170_921)));
    assert!(!exact);
    // The third, 36.8698976...°; the three make 180 degrees to the unit.
    let third = angular(Ray::Backward, Ray::Forward, 500, vec![line("B"), line("C")]);
    let (other, _) = shown_angle(&third, &located);
    assert_eq!(other, Some(Angle::from_units(331_829_079)));
    let sum = 90 * DEGREE + 331_829_079 + 478_170_921;
    assert_eq!(sum, 180 * DEGREE);
    let measured = measure(&at_origin, &located);
    let shown = labels(&measured, &Units::metric(), &PlainText::standard()).unwrap();
    assert_eq!(shown[0].text, "53.13°");
}

#[test]
fn an_angle_needs_two_crossing_straight_lines() {
    let located = Located::new()
        .with(line("W-1"), Ok(wall(0, 0, 4000, 0)))
        .with(line("W-2"), Ok(wall(0, 900, 4000, 900)))
        .with(
            grid("G-C"),
            Ok(Locus::Arc(Curve {
                centre: Point::mm(0, 0),
                radius: mm(6000),
                sweep: Sweep::Full,
            })),
        );
    let parallel = angular(
        Ray::Forward,
        Ray::Forward,
        500,
        vec![line("W-1"), line("W-2")],
    );
    let measured = measure(&parallel, &located);
    assert_eq!(measured.segments[0].angle, None);
    assert_eq!(measured.figure, Figure::Nothing);
    assert_eq!(measured.unusable, [1]);
    assert_eq!(measured.warnings[0].kind, REFERENCE_PARALLEL);
    assert!(
        measured.warnings[0]
            .words
            .contains("parallel to reference 1")
    );
    let curved = angular(
        Ray::Forward,
        Ray::Forward,
        500,
        vec![line("W-1"), grid("G-C")],
    );
    let measured = measure(&curved, &located);
    assert_eq!(measured.unusable, [1]);
    assert!(
        measured.warnings[0]
            .words
            .contains("a curved grid has no straight line to measure to")
    );
    let gone = angular(
        Ray::Forward,
        Ray::Forward,
        500,
        vec![line("W-1"), line("W-9")],
    );
    let measured = measure(&gone, &located);
    assert_eq!(measured.warnings[0].kind, REFERENCE_MISSING);
    assert_eq!(measured.unusable, [1]);
    // An angle has exactly two references.
    assert!(matches!(
        Dimension::new(
            id("D-A"),
            id("V-1"),
            Layout::angular(Ray::Forward, Ray::Forward, mm(1)).unwrap(),
            vec![line("W-1"), line("W-2"), grid("G-C")],
        ),
        Err(DimensionError::References(_))
    ));
    assert!(matches!(
        Layout::angular(Ray::Forward, Ray::Forward, mm(0)),
        Err(DimensionError::OutOfRange(_))
    ));
}

/// A grid arc of radius 6 m about (1000, 2000), from 0 to 90 degrees.
fn quarter() -> Locus {
    Locus::Arc(Curve {
        centre: Point::mm(1000, 2000),
        radius: mm(6000),
        sweep: Sweep::Between {
            from: Angle::from_units(0),
            to: Angle::from_units(90 * DEGREE),
        },
    })
}

#[test]
fn a_grid_arcs_radius_and_diameter_are_exact() {
    let located = Located::new().with(grid("G-R"), Ok(quarter()));
    let toward = Direction::new(mm(3), mm(4)).unwrap();
    let radial = Dimension::new(
        id("D-R"),
        id("V-1"),
        Layout::radial(toward),
        vec![grid("G-R")],
    )
    .unwrap();
    let measured = measure(&radial, &located);
    assert!(measured.warnings.is_empty());
    assert_eq!(measured.kind, DimensionKind::Radial);
    assert_eq!(measured.segments[0].value, Some(mm(6000)));
    assert!(measured.segments[0].exact);
    // The line runs from the centre toward (3, 4): it meets the arc at (1000 + 3600, 2000 + 4800).
    assert_eq!(
        measured.figure,
        Figure::Leader {
            centre: Point::mm(1000, 2000),
            from: Point::mm(1000, 2000),
            to: Point::mm(4600, 6800),
        }
    );
    assert_eq!(measured.segments[0].middle, Some(Point::mm(2800, 4400)));
    let diameter = Dimension::new(
        id("D-D"),
        id("V-1"),
        Layout::diameter(toward),
        vec![grid("G-R")],
    )
    .unwrap();
    let measured = measure(&diameter, &located);
    assert_eq!(measured.segments[0].value, Some(mm(12_000)));
    let Figure::Leader { from, to, .. } = measured.figure else {
        panic!("a line")
    };
    assert_eq!((from, to), (Point::mm(-2600, -2800), Point::mm(4600, 6800)));
    let shown = labels(&measured, &Units::metric(), &PlainText::standard()).unwrap();
    assert_eq!(shown[0].text, "12000 mm");
    // A straight grid has no radius.
    let straight = Located::new().with(grid("G-R"), Ok(wall(0, 0, 0, 5000)));
    let measured = measure(&radial, &straight);
    assert_eq!(measured.segments[0].value, None);
    assert!(measured.warnings[0].words.contains("is straight"));
    assert_eq!(measured.unusable, [0]);
}

#[test]
fn a_grid_arcs_length_is_the_nearest_step_beside_it() {
    let located = Located::new().with(grid("G-R"), Ok(quarter()));
    let dimension = Dimension::new(
        id("D-L"),
        id("V-1"),
        Layout::arc_length(mm(500)).unwrap(),
        vec![grid("G-R")],
    )
    .unwrap();
    let measured = measure(&dimension, &located);
    assert!(measured.warnings.is_empty());
    // A quarter of a 6 m circle: 3000 pi mm, 9424.778 mm, the nearest step 60,318,579.
    assert_eq!(
        measured.segments[0].value,
        Some(Length::from_steps(60_318_579))
    );
    assert!(!measured.segments[0].exact);
    let Figure::Arc {
        centre,
        radius,
        from,
        to,
        anticlockwise,
        large,
        ..
    } = measured.figure
    else {
        panic!("an arc")
    };
    assert_eq!(centre, Point::mm(1000, 2000));
    assert_eq!(radius, mm(6500));
    assert_eq!((from, to), (Point::mm(7500, 2000), Point::mm(1000, 8500)));
    assert!(anticlockwise && !large);
    // Its witness lines run out from the arc's ends to the dimension's arc.
    assert_eq!(measured.witnesses.len(), 2);
    assert_eq!(measured.witnesses[0].unwrap().from, Point::mm(7000, 2000));
    assert_eq!(measured.witnesses[1].unwrap().to, Point::mm(1000, 8500));
    let shown = labels(&measured, &Units::metric(), &PlainText::standard()).unwrap();
    assert_eq!(shown[0].text, "9425 mm");
    // A whole circle has no ends.
    let circle = Located::new().with(
        grid("G-R"),
        Ok(Locus::Arc(Curve {
            centre: Point::mm(0, 0),
            radius: mm(6000),
            sweep: Sweep::Full,
        })),
    );
    let measured = measure(&dimension, &circle);
    assert_eq!(measured.segments[0].value, None);
    assert!(measured.warnings[0].words.contains("whole circle"));
    // An arc inside it, past its centre, has no radius.
    let inside = Dimension::new(
        id("D-L"),
        id("V-1"),
        Layout::arc_length(mm(-6000)).unwrap(),
        vec![grid("G-R")],
    )
    .unwrap();
    assert_eq!(measure(&inside, &located).figure, Figure::Nothing);
}

#[test]
fn ordinate_values_along_a_run_of_walls_are_read_from_the_first() {
    // Four walls across a run, their lines at x = 1000, 3400, 5000 and 9250 mm.
    let mut located = Located::new();
    let walls = ["W-1", "W-2", "W-3", "W-4"];
    for (name, x) in walls.iter().zip([1000, 3400, 5000, 9250]) {
        located.set(line(name), Ok(wall(x, 0, x, 3000)));
    }
    let dimension = Dimension::ordinate(
        id("D-O"),
        id("V-1"),
        Layout::horizontal(mm(-800)).unwrap(),
        walls.iter().map(|w| line(w)).collect(),
    )
    .unwrap();
    assert_eq!(dimension.kind(), DimensionKind::Ordinate);
    let measured = measure(&dimension, &located);
    let values: Vec<_> = measured.segments.iter().map(|s| s.value).collect();
    assert_eq!(values, [Some(mm(2400)), Some(mm(4000)), Some(mm(8250))]);
    assert!(measured.segments.iter().all(|s| s.exact && s.from == 0));
    assert_eq!(measured.segments[2].middle, Some(Point::mm(9250, -800)));
    assert_eq!(measured.overall, None);
    let shown = labels(&measured, &Units::metric(), &PlainText::standard()).unwrap();
    let numbered: Vec<_> = shown.iter().map(|l| (l.segment, l.text.as_str())).collect();
    assert_eq!(numbered, [(1, "2400 mm"), (2, "4000 mm"), (3, "8250 mm")]);
    // A wall before the origin reads below zero.
    located.set(line("W-0"), Ok(wall(400, 0, 400, 3000)));
    let behind = dimension
        .clone()
        .with_reference_added(4, line("W-0"))
        .unwrap();
    assert_eq!(measure(&behind, &located).segments[3].value, Some(mm(-600)));
    // An ordinate dimension holds no equalities, and lies along a line.
    assert!(matches!(
        dimension.with_equalities(vec![vec![0, 1]]),
        Err(DimensionError::BadEquality(_))
    ));
    assert!(matches!(
        Dimension::ordinate(
            id("D-O"),
            id("V-1"),
            Layout::arc_length(mm(1)).unwrap(),
            vec![line("W-1")]
        ),
        Err(DimensionError::References(_))
    ));
}

#[test]
fn a_place_means_the_angle_it_is_in() {
    // Two walls crossing at (2000, 1000): one running east, one north-east.
    let loci = [wall(0, 1000, 4000, 1000), wall(1000, 0, 3000, 2000)];
    let at = |x, y| placed(Placing::Angular, Point::mm(x, y), &loci).unwrap();
    // Between east and north-east, 500 mm from the crossing: forward on both.
    assert_eq!(
        at(2400, 1300),
        Layout::angular(Ray::Forward, Ray::Forward, mm(500)).unwrap()
    );
    // West of the north-east wall, above the east one: back along the first, forward the second.
    assert_eq!(at(1500, 1500).key(), "angular");
    let Layout::Angular { first, second, .. } = at(1500, 1500) else {
        panic!("an angle")
    };
    assert_eq!((first, second), (Ray::Backward, Ray::Forward));
    let Layout::Angular { first, second, .. } = at(2000, 500) else {
        panic!("an angle")
    };
    assert_eq!((first, second), (Ray::Forward, Ray::Backward));
    // On the crossing itself, nowhere to draw it.
    assert!(matches!(
        placed(Placing::Angular, Point::mm(2000, 1000), &loci),
        Err(DimensionError::CannotPlace(_))
    ));
    // A radius points at the place; an arc length is as far outside the arc as the place.
    let arc = [quarter()];
    assert_eq!(
        placed(Placing::Radial, Point::mm(4000, 6000), &arc).unwrap(),
        Layout::radial(Direction::new(mm(3000), mm(4000)).unwrap())
    );
    assert_eq!(
        placed(Placing::ArcLength, Point::mm(4600, 6800), &arc).unwrap(),
        Layout::arc_length(mm(0)).unwrap()
    );
    assert_eq!(
        placed(Placing::ArcLength, Point::mm(1000, 9000), &arc).unwrap(),
        Layout::arc_length(mm(1000)).unwrap()
    );
    assert!(matches!(
        placed(Placing::Radial, Point::mm(1000, 2000), &arc),
        Err(DimensionError::CannotPlace(_))
    ));
    assert!(matches!(
        placed(Placing::Radial, Point::mm(0, 0), &loci),
        Err(DimensionError::CannotPlace(_))
    ));
}

/// Four grid lines across x at `xs` millimetres, a horizontal string between them.
fn string(xs: [i32; 4]) -> (Dimension, Located) {
    let names = ["G-1", "G-2", "G-3", "G-4"];
    let mut located = Located::new();
    for (name, x) in names.iter().zip(xs) {
        located.set(grid(name), Ok(wall(x, -1000, x, 7000)));
    }
    let dimension = Dimension::new(
        id("D-E"),
        id("V-1"),
        Layout::horizontal(mm(8000)).unwrap(),
        names.iter().map(|n| grid(n)).collect(),
    )
    .unwrap();
    (dimension, located)
}

fn moved(moves: &[Move]) -> Vec<(String, [i64; 2])> {
    moves
        .iter()
        .map(|m| (m.element.to_string(), m.by))
        .collect()
}

#[test]
fn a_three_segment_string_is_made_equal_by_moving_its_inner_references() {
    // 0, 2000, 5000, 9000: segments of 2000, 3000 and 4000. Equal, each is 3000, the ends kept.
    let (dimension, located) = string([0, 2000, 5000, 9000]);
    let moves = equalize(&dimension, &[vec![0, 1, 2]], &located).unwrap();
    assert_eq!(
        moved(&moves),
        [
            ("G-2".to_owned(), [1000 * STEPS_PER_MM, 0]),
            ("G-3".to_owned(), [1000 * STEPS_PER_MM, 0])
        ]
    );
    // Moved so, the dimension reads three equal segments, and its equality holds exactly.
    let mut after = Located::new();
    for (name, x) in ["G-1", "G-2", "G-3", "G-4"]
        .iter()
        .zip([0, 3000, 6000, 9000])
    {
        after.set(grid(name), Ok(wall(x, -1000, x, 7000)));
    }
    let held = dimension
        .clone()
        .with_equalities(vec![vec![0, 1, 2]])
        .unwrap();
    let measured = measure(&held, &after);
    let values: Vec<_> = measured.segments.iter().map(|s| s.value).collect();
    assert_eq!(values, [Some(mm(3000)); 3]);
    assert_eq!(measured.equalities[0].equal, Some(true));
    assert_eq!(measured.equalities[0].holds, Some(true));
    // Before, it did not hold.
    let before = measure(&held, &located);
    assert_eq!(before.equalities[0].equal, Some(false));
    assert_eq!(before.equalities[0].holds, Some(false));
    // Already equal: nothing moves.
    assert!(
        equalize(&dimension, &[vec![0, 1, 2]], &after)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_length_that_does_not_divide_shares_out_to_the_step() {
    // 1000 mm in three: 333.333... mm each, so each inner line goes to the nearest step.
    let (dimension, located) = string([0, 100, 200, 1000]);
    let moves = equalize(&dimension, &[vec![0, 1, 2]], &located).unwrap();
    let third = 1000 * STEPS_PER_MM / 3; // 2,133,333 steps, a third of a step short
    assert_eq!(
        moved(&moves),
        [
            ("G-2".to_owned(), [third - 100 * STEPS_PER_MM, 0]),
            ("G-3".to_owned(), [2 * third + 1 - 200 * STEPS_PER_MM, 0])
        ]
    );
    let mut after = Located::new();
    for (name, x) in
        ["G-1", "G-2", "G-3", "G-4"]
            .iter()
            .zip([0, third, 2 * third + 1, 1000 * STEPS_PER_MM])
    {
        after.set(
            grid(name),
            Ok(Locus::line(
                Point::new(Length::from_steps(x), mm(0)),
                mm(0),
                mm(1),
            )),
        );
    }
    let held = dimension.with_equalities(vec![vec![0, 1, 2]]).unwrap();
    let state = &measure(&held, &after).equalities[0];
    assert_eq!(state.equal, Some(false));
    assert_eq!(state.holds, Some(true));
}

#[test]
fn a_group_keeps_the_segments_it_does_not_hold() {
    // 0, 1000, 4000, 4500, 9000: hold segments 1 and 4 (of 1000 and 4500) equal across the run,
    // keeping 3000 and 500 between them: each becomes (9000 - 3500) / 2 = 2750.
    let names = ["G-1", "G-2", "G-3", "G-4", "G-5"];
    let mut located = Located::new();
    for (name, x) in names.iter().zip([0, 1000, 4000, 4500, 9000]) {
        located.set(grid(name), Ok(wall(x, 0, x, 1)));
    }
    let dimension = Dimension::new(
        id("D-E"),
        id("V-1"),
        Layout::horizontal(mm(0)).unwrap(),
        names.iter().map(|n| grid(n)).collect(),
    )
    .unwrap();
    let moves = equalize(&dimension, &[vec![0, 3]], &located).unwrap();
    let by = |mm: i64| [mm * STEPS_PER_MM, 0];
    assert_eq!(
        moved(&moves),
        [
            ("G-2".to_owned(), by(1750)),
            ("G-3".to_owned(), by(1750)),
            ("G-4".to_owned(), by(1750))
        ]
    );
}

#[test]
fn what_cannot_be_made_equal_is_refused_by_name() {
    // One wall at two references that would have to move apart.
    let names = [
        Reference::new(id("W-1"), Feature::WallFace(Side::Exterior)),
        Reference::new(id("W-1"), Feature::WallFace(Side::Interior)),
        grid("G-2"),
    ];
    let located = Located::new()
        .with(names[0].clone(), Ok(wall(0, 0, 0, 1)))
        .with(names[1].clone(), Ok(wall(200, 0, 200, 1)))
        .with(names[2].clone(), Ok(wall(3000, 0, 3000, 1)));
    let dimension = Dimension::new(
        id("D-E"),
        id("V-1"),
        Layout::horizontal(mm(0)).unwrap(),
        names.to_vec(),
    )
    .unwrap();
    let refused = equalize(&dimension, &[vec![0, 1]], &located).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "Cannot make the segments equal: references 1 and 2 are both of W-1, which would have to \
         move by two amounts."
    );
    // A reference that cannot be found is named.
    let gone = Located::new()
        .with(names[0].clone(), Ok(wall(0, 0, 0, 1)))
        .with(names[1].clone(), Err(Missing::deleted()));
    let refused = equalize(&dimension, &[vec![0, 1]], &gone).unwrap_err();
    assert!(
        refused
            .to_string()
            .contains("reference 2 (the interior face of W-1) cannot be found")
    );
    // A run whose ends are at one place.
    let (folded, _) = string([0, 1000, 2000, 0]);
    let mut back = Located::new();
    for (name, x) in ["G-1", "G-2", "G-3", "G-4"].iter().zip([0, 1000, 2000, 0]) {
        back.set(grid(name), Ok(wall(x, 0, x, 1)));
    }
    assert!(
        equalize(&folded, &[vec![0, 1, 2]], &back)
            .unwrap_err()
            .to_string()
            .contains("are at one place")
    );
    // Only a linear string is made equal; a bad group is refused as when it is stored.
    let (string_of_four, located) = string([0, 1, 2, 3]);
    assert!(matches!(
        equalize(&string_of_four, &[vec![0, 7]], &located),
        Err(DimensionError::BadEquality(_))
    ));
    let angle = angular(Ray::Forward, Ray::Forward, 1, vec![line("A"), line("B")]);
    assert!(matches!(
        equalize(&angle, &[vec![0, 1]], &Located::new()),
        Err(DimensionError::CannotEqualize(_))
    ));
}
