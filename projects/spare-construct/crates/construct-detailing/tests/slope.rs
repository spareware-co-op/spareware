//! Spot slopes: the slope of a face at a point, never stored, worked out from the exact plane each
//! time, with the numbers worked by hand and every refusal by name.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::assert_is_empty,
    clippy::assigning_clones,
    clippy::assertions_on_constants,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_detailing::{DetailError, Faces, Plane, SlopeRefusal, SpotSlope, read};
use construct_grids::Point;
use construct_params::{Angle, Length, Ratio};
use kit_core::Id;
use std::collections::BTreeMap;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(x: i32, y: i32) -> Point {
    Point::new(Length::from_mm(x), Length::from_mm(y))
}

/// A point of a face in whole millimetres, as grid steps.
fn at(x: i64, y: i64, z: i64) -> [i64; 3] {
    [x * 6400, y * 6400, z * 6400]
}

/// A table of faces: for each element a plane, and the box in plan (low corner, high corner, in
/// steps) it covers.
struct Table(BTreeMap<Id, (Plane, [[i64; 2]; 2])>);

impl Faces for Table {
    fn plane_at(&self, face: &Id, point: Point) -> Result<Plane, SlopeRefusal> {
        let (plane, [lo, hi]) = self
            .0
            .get(face)
            .ok_or_else(|| SlopeRefusal::Missing(format!("There is no floor or roof {face}.")))?;
        let (x, y) = (point.x.steps(), point.y.steps());
        if x < lo[0] || x > hi[0] || y < lo[1] || y > hi[1] {
            return Err(SlopeRefusal::OffFace(format!("The point is off {face}.")));
        }
        Ok(*plane)
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "a test compares degrees to a hand-worked value"
)]
fn degrees_of(angle: Angle) -> f64 {
    angle.units() as f64 / Angle::UNITS_PER_DEGREE as f64
}

/// A face rising 3000 mm over a 4000 mm run up the y axis, 6 m wide: a slope of 3 in 4.
fn three_in_four() -> Plane {
    Plane::through_points(at(0, 0, 0), at(6000, 0, 0), at(0, 4000, 3000)).unwrap()
}

#[test]
fn a_face_rising_3_in_4_is_exactly_three_quarters() {
    let reading = read(&three_in_four()).unwrap();
    // 3 over 4, in lowest terms, exactly, and its square 9 over 16.
    assert_eq!(reading.rise_over_run, Some(Ratio::new(3, 4)));
    assert_eq!(reading.squared.0.to_string(), "9");
    assert_eq!(reading.squared.1.to_string(), "16");
    // atan(0.75) = 36.8698976458 degrees: 331,829,078.8 angle units, to the nearest unit.
    assert!(
        (reading.angle.units() - 331_829_079).abs() <= 1,
        "{}",
        reading.angle.units()
    );
    // The face rises towards +y, so water runs towards -y.
    assert_eq!(reading.fall, [0, -1]);
    assert_eq!(reading.text(), "36.87 degrees, rise over run 3/4");
}

#[test]
fn a_thirty_degree_roof_reads_thirty_degrees_to_the_grid() {
    // The roof rises 2886.7513 mm over a 5000 mm run (5000 x tan 30 degrees = 2886.7513459...), the
    // rise held to the nearest step: 2886.7513459 mm = 18,475,208.6 steps, so 18,475,209.
    let plane = Plane::through_points([0, 0, 0], [6000 * 6400, 0, 0], [0, 5000 * 6400, 18_475_209])
        .unwrap();
    let reading = read(&plane).unwrap();
    // Half a step in 32,000,000 is 1.6e-8 radian, 0.9e-6 degree: 8 angle units. Allow 10.
    assert!(
        (reading.angle.units() - 30 * Angle::UNITS_PER_DEGREE).abs() <= 10,
        "{}",
        reading.angle.units()
    );
    // The face as stored is on the grid, so its rise over run is the fraction of whole steps.
    assert_eq!(
        reading.rise_over_run,
        Some(Ratio::new(18_475_209, 32_000_000))
    );
    assert!((degrees_of(reading.angle) - 30.0).abs() < 1e-5);
    assert!((reading.rise_over_run_approx() - 0.577_350_269).abs() < 1e-7);
    assert_eq!(reading.text(), "30.00 degrees, rise over run 0.5774");
}

#[test]
fn a_slope_across_a_corner_is_the_steepest_line_not_either_axis() {
    // A face that rises 1 in 1 along x and 1 in 1 along y: its steepest line is 45 degrees across
    // the corner, the square root of 2 over 1: no exact ratio; the square is exactly 2.
    let plane = Plane::through_points(at(0, 0, 0), at(1000, 0, 1000), at(0, 1000, 1000)).unwrap();
    let reading = read(&plane).unwrap();
    assert_eq!(reading.squared.0.to_string(), "2");
    assert_eq!(reading.squared.1.to_string(), "1");
    assert_eq!(reading.rise_over_run, None);
    // atan(sqrt 2) = 54.7356103172 degrees.
    assert!((degrees_of(reading.angle) - 54.735_610_317).abs() < 1e-6);
    // Water runs the way the face falls: towards -x and -y.
    assert_eq!(reading.fall, [-1, -1]);
}

#[test]
fn a_flat_or_upright_face_has_no_slope_to_read() {
    let level = Plane::through_points(at(0, 0, 100), at(1000, 0, 100), at(0, 1000, 100)).unwrap();
    assert_eq!(read(&level), Err(SlopeRefusal::Flat));
    let wall = Plane::through_points(at(0, 0, 0), at(1000, 0, 0), at(0, 0, 1000)).unwrap();
    assert_eq!(read(&wall), Err(SlopeRefusal::Vertical));
    // Three points on a line make no plane at all.
    assert!(matches!(
        Plane::through_points(at(0, 0, 0), at(1, 1, 1), at(2, 2, 2)),
        Err(SlopeRefusal::Missing(_))
    ));
    assert_eq!(SlopeRefusal::Flat.kind(), "flat");
    assert_eq!(SlopeRefusal::Vertical.kind(), "vertical");
}

#[test]
fn a_face_that_stands_the_other_way_up_reads_the_same() {
    // The same plane with its three points taken the other way round.
    let a = Plane::through_points(at(0, 0, 0), at(6000, 0, 0), at(0, 4000, 3000)).unwrap();
    let b = Plane::through_points(at(0, 0, 0), at(0, 4000, 3000), at(6000, 0, 0)).unwrap();
    assert_eq!(read(&a).unwrap(), read(&b).unwrap());
}

#[test]
fn a_spot_slope_is_worked_out_from_the_face_every_time_it_is_asked() {
    let slope = SpotSlope::new(id("s1"), id("v1"), id("roof-1"), mm(3000, 2000)).unwrap();
    let mut faces = Table(BTreeMap::new());
    // The roof is not there.
    let missing = slope.read(&faces).unwrap_err();
    assert!(matches!(
        missing,
        DetailError::Slope(SlopeRefusal::Missing(_))
    ));
    // The roof comes: 3 in 4.
    faces.0.insert(
        id("roof-1"),
        (three_in_four(), [[0, 0], [6000 * 6400, 4000 * 6400]]),
    );
    assert_eq!(
        slope.read(&faces).unwrap().rise_over_run,
        Some(Ratio::new(3, 4))
    );
    // The roof is changed to 1 in 1: the same spot slope now reads 1/1, with nothing stored.
    let steeper = Plane::through_points(at(0, 0, 0), at(6000, 0, 0), at(0, 4000, 4000)).unwrap();
    faces.0.insert(
        id("roof-1"),
        (steeper, [[0, 0], [6000 * 6400, 4000 * 6400]]),
    );
    assert_eq!(
        slope.read(&faces).unwrap().rise_over_run,
        Some(Ratio::new(1, 1))
    );
    // Moved off the roof, it names that.
    let off = slope.clone().with_point(mm(9000, 2000)).unwrap();
    assert!(matches!(
        off.read(&faces),
        Err(DetailError::Slope(SlopeRefusal::OffFace(_)))
    ));
    // On a flat face it names that.
    let flat = Plane::through_points(at(0, 0, 0), at(6000, 0, 0), at(0, 4000, 0)).unwrap();
    faces
        .0
        .insert(id("roof-1"), (flat, [[0, 0], [6000 * 6400, 4000 * 6400]]));
    assert_eq!(
        slope.read(&faces),
        Err(DetailError::Slope(SlopeRefusal::Flat))
    );
}

#[test]
fn a_spot_slope_record_holds_the_face_and_the_point_and_no_value() {
    let slope = SpotSlope::new(id("s1"), id("v1"), id("roof-1"), mm(3000, 2000)).unwrap();
    let record = slope.to_record(&id("doc"));
    assert_eq!(record.kind, "detail.slope");
    // Nothing worked out is stored: the view, the face and the point only.
    let keys: Vec<&str> = record.props.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "@category",
            "@detail.slope.face",
            "@detail.slope.point.x",
            "@detail.slope.point.y",
            "@detail.view"
        ]
    );
    assert_eq!(SpotSlope::from_record(&record).unwrap(), slope);
    let mut bad = record.clone();
    bad.props
        .insert("@detail.slope.angle".to_owned(), "angle:1".to_owned());
    assert!(SpotSlope::from_record(&bad).is_err());
    // A point past the grid.
    let far = Point::new(Length::from_steps(1 << 41), Length::from_steps(0));
    assert!(SpotSlope::new(id("s1"), id("v1"), id("roof-1"), far).is_err());
}
