//! A pitch is held as it was given, a slope or an angle, in construct-params' own spellings, and
//! an angle is worked as the slope nearest its tangent over a run of 10^9.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{degrees, id, slope};
use construct_params::{Angle, Slope, Value};
use construct_roofs::{ANGLE_RUN, Pitch};

/// 30 degrees is 577,350,269 in 1,000,000,000 (tan 30 is 0.5773502691...), 45 degrees is 1 in 1
/// in lowest terms, and the run is always 10^9 before the ratio is reduced.
#[test]
fn an_angle_is_worked_as_the_slope_nearest_its_tangent() {
    assert_eq!(ANGLE_RUN, 1_000_000_000);
    assert_eq!(
        degrees(30).slope(),
        Some(Slope::new(577_350_269, 1_000_000_000).unwrap())
    );
    assert_eq!(degrees(45).slope(), Some(Slope::new(1, 1).unwrap()));
    let tenth = Pitch::Angle(Angle::from_units(Angle::UNITS_PER_DEGREE / 10));
    // tan 0.1 degrees is 0.0017453310...
    assert_eq!(
        tenth.slope(),
        Some(Slope::new(1_745_331, 1_000_000_000).unwrap())
    );
    assert_eq!(degrees(0).slope(), None);
    assert_eq!(degrees(90).slope(), None);
    assert_eq!(degrees(-10).slope(), None);
    assert_eq!(slope(1, 4).slope(), Some(Slope::new(1, 4).unwrap()));
    assert_eq!(degrees(30).checked(&id("e")).unwrap().run(), 1_000_000_000);
}

/// The stored spelling is construct-params' own: `slope:1/4` and `angle:<units>`; anything else
/// is no pitch.
#[test]
fn a_pitch_is_stored_in_construct_params_spelling() {
    assert_eq!(slope(1, 4).stored(), "slope:1/4");
    assert_eq!(slope(2, 8).stored(), "slope:1/4");
    assert_eq!(degrees(30).stored(), "angle:270000000");
    assert_eq!(Pitch::from_stored("slope:1/4"), Some(slope(1, 4)));
    assert_eq!(Pitch::from_stored("angle:270000000"), Some(degrees(30)));
    assert_eq!(Pitch::from_stored("length:q0:6400"), None);
    assert_eq!(Pitch::from_stored("1:4"), None);
    assert_eq!(
        degrees(30).value(),
        Value::Angle(Angle::from_units(270_000_000))
    );
}
