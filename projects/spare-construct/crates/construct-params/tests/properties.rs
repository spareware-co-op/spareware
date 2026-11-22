//! Properties over many values, from fixed seeds so every run checks the same cases.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::{
    Angle, Area, Definition, Format, Holder, Kind, Length, Slope, Unit, Units, Value, Volume,
};
use kit_core::Id;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

/// A thousand kilometres, either way: far past any building, well inside the stored range.
const FAR: i64 = 1_000_000_000_000;

fn config(seed: u64) -> Config {
    Config {
        cases: 512,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: None,
        ..Config::default()
    }
}

fn def(kind: Kind) -> Definition {
    Definition::new(Id::parse("P").unwrap(), "P", kind, Holder::Instance).unwrap()
}

fn units(kind: Kind, unit: Unit, places: u8) -> Units {
    let mut units = Units::metric();
    units.set(kind, Format { unit, places }).unwrap();
    units
}

/// Formats `value` and reads the text back.
fn shown_and_read(kind: Kind, units: &Units, value: &Value) -> Value {
    let def = def(kind);
    let text = def.format(value, units).unwrap();
    def.parse(&text, units).unwrap()
}

fn any_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<i64>().prop_map(|mm| Value::Length(Length::from_mm(mm))),
        any::<i64>().prop_map(|v| Value::Area(Area::from_mm2(v))),
        any::<i64>().prop_map(|v| Value::Volume(Volume::from_mm3(v))),
        any::<i64>().prop_map(|v| Value::Angle(Angle::from_units(v))),
        (-FAR..FAR, 1..FAR).prop_map(|(rise, run)| Value::Slope(Slope::new(rise, run).unwrap())),
        (any::<i64>(), 0u32..20)
            .prop_map(|(m, scale)| { Value::number(construct_params::Decimal::new(m, scale)) }),
        any::<i64>().prop_map(Value::Whole),
        ".*".prop_map(Value::Text),
        any::<bool>().prop_map(Value::YesNo),
        "[a-z][a-z0-9-]{0,20}".prop_map(Value::Choice),
        "[0-9A-Z][0-9A-Za-z._-]{0,40}".prop_map(|t| Value::Element(Id::parse(&t).unwrap())),
        "[0-9A-Z][0-9A-Za-z._-]{0,40}".prop_map(|t| Value::Material(Id::parse(&t).unwrap())),
        "[a-z]{1,12}(/[a-z0-9]{0,8}){0,3}"
            .prop_map(|path| Value::link(&format!("https://example.org/{path}")).unwrap()),
    ]
}

proptest! {
    #![proptest_config(config(0x5EED_0001))]

    #[test]
    fn lengths_round_trip_through_mm_and_m(mm in -FAR..FAR) {
        let value = Value::Length(Length::from_mm(mm));
        prop_assert_eq!(shown_and_read(Kind::Length, &Units::metric(), &value), value.clone());
        let metres = units(Kind::Length, Unit::Metre, 3);
        prop_assert_eq!(shown_and_read(Kind::Length, &metres, &value), value);
    }

    #[test]
    fn areas_and_volumes_round_trip_at_full_places(v in -FAR..FAR) {
        let area = Value::Area(Area::from_mm2(v));
        let m2 = units(Kind::Area, Unit::SquareMetre, 6);
        prop_assert_eq!(shown_and_read(Kind::Area, &m2, &area), area);
        let volume = Value::Volume(Volume::from_mm3(v));
        let m3 = units(Kind::Volume, Unit::CubicMetre, 9);
        prop_assert_eq!(shown_and_read(Kind::Volume, &m3, &volume), volume);
    }

    #[test]
    fn angles_in_millionths_of_a_degree_round_trip(millionths in -FAR..FAR) {
        let value = Value::Angle(Angle::from_units(millionths * 9));
        let degrees = units(Kind::Angle, Unit::Degree, 6);
        prop_assert_eq!(shown_and_read(Kind::Angle, &degrees, &value), value);
    }

    #[test]
    fn angles_convert_to_exact_degrees_and_back(units in any::<i64>()) {
        let angle = Angle::from_units(units);
        prop_assert_eq!(Angle::from_degrees(angle.degrees()), Some(angle));
    }

    #[test]
    fn shown_values_read_back_to_what_was_shown(mm in -FAR..FAR, places in 0u8..=9) {
        // Fewer places lose part of a value, but what is shown reads back to itself.
        let length = def(Kind::Length);
        let metres = units(Kind::Length, Unit::Metre, places);
        let shown = length.format(&Value::Length(Length::from_mm(mm)), &metres).unwrap();
        let again = length.parse(&shown, &metres).unwrap();
        prop_assert_eq!(length.format(&again, &metres).unwrap(), shown);
    }

    #[test]
    fn slopes_shown_in_both_forms_read_back_to_what_was_shown(
        rise in -FAR..FAR,
        run in 1..FAR,
        places in 0u8..=6,
    ) {
        let slope = def(Kind::Slope);
        let value = Value::Slope(Slope::new(rise, run).unwrap());
        for unit in [Unit::Percent, Unit::Ratio] {
            let form = units(Kind::Slope, unit, places);
            let shown = slope.format(&value, &form).unwrap();
            let again = slope.parse(&shown, &form).unwrap();
            prop_assert_eq!(slope.format(&again, &form).unwrap(), shown);
        }
    }

    #[test]
    fn every_value_reads_back_from_its_stored_text(value in any_value()) {
        let text = value.stored();
        prop_assert_eq!(Value::from_stored(&text), Some(value.clone()));
        // The same value gives the same bytes.
        prop_assert_eq!(value.clone().stored(), text);
    }

    #[test]
    fn sums_of_like_values_are_exact(a in -FAR..FAR, b in -FAR..FAR, c in -FAR..FAR) {
        let len = |mm| Value::Length(Length::from_mm(mm));
        prop_assert_eq!(len(a).add(&len(b)), Ok(len(a + b)));
        prop_assert_eq!(len(a).add(&len(b)), len(b).add(&len(a)));
        prop_assert_eq!(
            len(a).add(&len(b)).and_then(|ab| ab.add(&len(c))),
            len(b).add(&len(c)).and_then(|bc| len(a).add(&bc))
        );
        prop_assert_eq!(len(a).subtract(&len(b)), Ok(len(a - b)));
        let area = |v| Value::Area(Area::from_mm2(v));
        prop_assert_eq!(area(a).add(&area(b)), Ok(area(a + b)));
        let vol = |v| Value::Volume(Volume::from_mm3(v));
        prop_assert_eq!(vol(a).add(&vol(b)), Ok(vol(a + b)));
        let ang = |v| Value::Angle(Angle::from_units(v));
        prop_assert_eq!(ang(a).add(&ang(b)), Ok(ang(a + b)));
        prop_assert_eq!(Value::Whole(a).add(&Value::Whole(b)), Ok(Value::Whole(a + b)));
    }

    #[test]
    fn length_times_length_is_an_area_and_area_times_length_a_volume(
        a in -1_000_000i64..1_000_000,
        b in -1_000_000i64..1_000_000,
        c in -1_000_000i64..1_000_000,
    ) {
        let len = |mm| Value::Length(Length::from_mm(mm));
        let area = len(a).multiply(&len(b)).unwrap();
        prop_assert_eq!(&area, &Value::Area(Area::from_mm2(a * b)));
        prop_assert_eq!(area.multiply(&len(c)), Ok(Value::Volume(Volume::from_mm3(a * b * c))));
        prop_assert_eq!(len(c).multiply(&area), Ok(Value::Volume(Volume::from_mm3(a * b * c))));
    }
}
