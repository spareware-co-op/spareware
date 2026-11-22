//! Text a person types reads to the stored value, and the stored value formats back to text that
//! reads to it again. Typed lengths, areas and volumes are exact in grid steps; typed angles and
//! shown values round by one rule: half to even, in whole numbers.

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
    Angle, Area, Choice, Definition, Format, Holder, Kind, Length, Ratio, Slope, Unit, Units,
    Value, Volume,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn def(kind: Kind) -> Definition {
    Definition::new(id("P1"), "P", kind, Holder::Instance).unwrap()
}

fn with(kind: Kind, unit: Unit, places: u8) -> Units {
    let mut units = Units::metric();
    units.set(kind, Format { unit, places }).unwrap();
    units
}

/// Reads `text`, checks it is `want`, formats it, and checks the formatted text reads back.
fn reads(kind: Kind, units: &Units, text: &str, want: &Value) {
    let def = def(kind);
    let value = def.parse(text, units).unwrap();
    assert_eq!(&value, want, "{text}");
    let shown = def.format(&value, units).unwrap();
    assert_eq!(
        &def.parse(&shown, units).unwrap(),
        want,
        "{text} shown as {shown}"
    );
}

#[test]
fn lengths_read_from_what_people_type() {
    let units = Units::metric();
    let want = Value::Length(Length::from_mm(2400));
    for text in [
        "2400",
        "2400mm",
        "2400 mm",
        " 2400 mm ",
        "2.4 m",
        "2.4m",
        "2.400 m",
        "+2.4 m",
    ] {
        reads(Kind::Length, &units, text, &want);
    }
    reads(
        Kind::Length,
        &units,
        "-150",
        &Value::Length(Length::from_mm(-150)),
    );
    // A bare number is in the display unit.
    let metres = with(Kind::Length, Unit::Metre, 3);
    reads(Kind::Length, &metres, "2.4", &want);
    reads(Kind::Length, &metres, "2400 mm", &want);
}

#[test]
fn typed_lengths_are_exact_in_grid_steps() {
    // Everyday sizes that are not whole millimetres are stored exactly: plasterboard, laminated
    // glass, steel sheet, a membrane. Nothing typed is rounded.
    let units = Units::metric();
    for (text, steps) in [
        ("12.5", 80_000),
        ("12.5 mm", 80_000),
        ("6.38mm", 40_832),
        ("0.55 mm", 3_520),
        ("0.25", 1_600),
        ("2.4005 m", 15_363_200),
        ("-2.4015 m", -15_369_600),
        ("0.5", 3_200),
        ("-0.4", -2_560),
        ("0.005 mm", 32),
        ("0.00015625", 1),
        ("0", 0),
    ] {
        assert_eq!(
            def(Kind::Length).parse(text, &units).unwrap(),
            Value::Length(Length::from_steps(steps)),
            "{text}"
        );
    }
}

#[test]
fn products_of_lengths_are_exact() {
    let board = Value::Length(Length::from_steps(80_000));
    let square = board.multiply(&board).unwrap();
    assert_eq!(square, Value::Area(Area::from_square_steps(6_400_000_000)));
    let mm2 = with(Kind::Area, Unit::SquareMillimetre, 2);
    assert_eq!(def(Kind::Area).format(&square, &mm2).unwrap(), "156.25 mm²");
    let cube = square.multiply(&board).unwrap();
    let mm3 = with(Kind::Volume, Unit::CubicMillimetre, 3);
    assert_eq!(
        def(Kind::Volume).format(&cube, &mm3).unwrap(),
        "1953.125 mm³"
    );
    // A cubic metre is beyond 64 bits of cubic steps, and is held.
    let metre = Value::Length(Length::from_mm(1_000));
    let cubic_metre = metre.multiply(&metre).unwrap().multiply(&metre).unwrap();
    assert_eq!(cubic_metre, Value::Volume(Volume::from_mm3(1_000_000_000)));
    assert_eq!(
        def(Kind::Volume)
            .format(&cubic_metre, &Units::metric())
            .unwrap(),
        "1.00 m³"
    );
}

#[test]
fn lengths_display_in_mm_and_m() {
    let len = |mm| Value::Length(Length::from_mm(mm));
    let length = def(Kind::Length);
    assert_eq!(
        length.format(&len(2400), &Units::metric()).unwrap(),
        "2400 mm"
    );
    assert_eq!(
        length.format(&len(-35), &Units::metric()).unwrap(),
        "-35 mm"
    );
    let m3 = with(Kind::Length, Unit::Metre, 3);
    assert_eq!(length.format(&len(2400), &m3).unwrap(), "2.400 m");
    assert_eq!(length.format(&len(-5), &m3).unwrap(), "-0.005 m");
    let m2 = with(Kind::Length, Unit::Metre, 2);
    assert_eq!(length.format(&len(2405), &m2).unwrap(), "2.40 m");
    assert_eq!(length.format(&len(2415), &m2).unwrap(), "2.42 m");
    assert_eq!(length.format(&len(2406), &m2).unwrap(), "2.41 m");
    assert_eq!(length.format(&len(-4), &m2).unwrap(), "0.00 m");
    let m0 = with(Kind::Length, Unit::Metre, 0);
    assert_eq!(length.format(&len(2500), &m0).unwrap(), "2 m");
    assert_eq!(length.format(&len(3500), &m0).unwrap(), "4 m");
    let mm1 = with(Kind::Length, Unit::Millimetre, 1);
    assert_eq!(length.format(&len(2400), &mm1).unwrap(), "2400.0 mm");
    // A size between whole millimetres shows at its places, rounded half to even.
    let steps = |s| Value::Length(Length::from_steps(s));
    assert_eq!(length.format(&steps(80_000), &mm1).unwrap(), "12.5 mm");
    assert_eq!(
        length.format(&steps(80_000), &Units::metric()).unwrap(),
        "12 mm"
    );
    assert_eq!(
        length.format(&steps(86_400), &Units::metric()).unwrap(),
        "14 mm"
    );
    let mm2 = with(Kind::Length, Unit::Millimetre, 2);
    assert_eq!(length.format(&steps(40_832), &mm2).unwrap(), "6.38 mm");
    assert_eq!(length.format(&steps(1), &mm2).unwrap(), "0.00 mm");
    let m5 = with(Kind::Length, Unit::Metre, 5);
    assert_eq!(length.format(&steps(80_000), &m5).unwrap(), "0.01250 m");
    reads(Kind::Length, &mm2, "6.38", &steps(40_832));
    reads(Kind::Length, &m5, "0.0125", &steps(80_000));
}

#[test]
fn areas_and_volumes_read_and_display_in_m2_and_m3() {
    let units = Units::metric();
    let area = Value::Area(Area::from_mm2(12_500_000));
    for text in [
        "12.5",
        "12.5 m2",
        "12.5m²",
        "12.50 m²",
        "12500000 mm2",
        "12500000mm²",
    ] {
        reads(Kind::Area, &units, text, &area);
    }
    assert_eq!(def(Kind::Area).format(&area, &units).unwrap(), "12.50 m²");
    let volume = Value::Volume(Volume::from_mm3(1_500_000_000));
    for text in ["1.5", "1.5 m3", "1.5m³", "1500000000 mm3"] {
        reads(Kind::Volume, &units, text, &volume);
    }
    assert_eq!(
        def(Kind::Volume).format(&volume, &units).unwrap(),
        "1.50 m³"
    );
    let m6 = with(Kind::Area, Unit::SquareMetre, 6);
    assert_eq!(
        def(Kind::Area)
            .format(&Value::Area(Area::from_mm2(1)), &m6)
            .unwrap(),
        "0.000001 m²"
    );
}

#[test]
fn angles_read_and_display_in_degrees() {
    let units = Units::metric();
    let right = Value::Angle(Angle::from_units(90 * Angle::UNITS_PER_DEGREE));
    for text in ["90", "90°", "90 °", "90deg", "90 deg", "90.00°"] {
        reads(Kind::Angle, &units, text, &right);
    }
    assert_eq!(def(Kind::Angle).format(&right, &units).unwrap(), "90.00°");
    // Decimal degrees to six places are exact.
    assert_eq!(
        def(Kind::Angle).parse("0.000001°", &units).unwrap(),
        Value::Angle(Angle::from_units(9))
    );
    let six = with(Kind::Angle, Unit::Degree, 6);
    reads(
        Kind::Angle,
        &six,
        "12.345678",
        &Value::Angle(Angle::from_units(111_111_102)),
    );
    // One unit is a 9,000,000th of a degree: shown at two places it rounds.
    assert_eq!(
        def(Kind::Angle)
            .format(&Value::Angle(Angle::from_units(45_000)), &units)
            .unwrap(),
        "0.00°"
    );
    assert_eq!(
        def(Kind::Angle)
            .format(&Value::Angle(Angle::from_units(135_000)), &units)
            .unwrap(),
        "0.02°"
    );
}

#[test]
fn angles_convert_to_degrees_exactly() {
    // The form a formula holds an angle in: degrees as an exact ratio.
    let third = Angle::from_degrees(Ratio::new(1, 3)).unwrap();
    assert_eq!(third, Angle::from_units(3_000_000));
    assert_eq!(third.degrees(), Ratio::new(1, 3));
    assert_eq!(Angle::from_degrees(Ratio::new(1, 7)), None);
    assert_eq!(Angle::from_degrees(Ratio::new(i128::MAX, 1)), None);
    assert_eq!(
        Angle::from_units(-405_000_000).degrees(),
        Ratio::new(-45, 1)
    );
}

#[test]
fn slopes_read_as_percent_or_ratio() {
    let units = Units::metric();
    let fall = Value::Slope(Slope::new(1, 40).unwrap());
    for text in ["2.5%", "2.5 %", "2.50 %", "2.5", "1:40", "0.5:20", "1 : 40"] {
        reads(Kind::Slope, &units, text, &fall);
    }
    assert_eq!(def(Kind::Slope).format(&fall, &units).unwrap(), "2.50 %");
    let ratio = with(Kind::Slope, Unit::Ratio, 0);
    assert_eq!(def(Kind::Slope).format(&fall, &ratio).unwrap(), "1:40");
    // A bare number in ratio form is the run for a rise of one.
    reads(Kind::Slope, &ratio, "40", &fall);
    reads(
        Kind::Slope,
        &ratio,
        "-1:60",
        &Value::Slope(Slope::new(-1, 60).unwrap()),
    );
    reads(
        Kind::Slope,
        &units,
        "0%",
        &Value::Slope(Slope::new(0, 1).unwrap()),
    );
    let ratio2 = with(Kind::Slope, Unit::Ratio, 2);
    assert_eq!(
        def(Kind::Slope)
            .format(&Value::Slope(Slope::new(3, 7).unwrap()), &ratio2)
            .unwrap(),
        "1:2.33"
    );
    assert_eq!(
        def(Kind::Slope)
            .format(&Value::Slope(Slope::new(0, 1).unwrap()), &ratio2)
            .unwrap(),
        "0:1"
    );
    assert_eq!(Slope::new(2, -80), Slope::new(-1, 40));
    assert_eq!(Slope::new(1, 0), None);
}

#[test]
fn the_other_kinds_read_and_display() {
    let units = Units::metric();
    reads(
        Kind::Number,
        &units,
        "2.50",
        &Value::number("2.5".parse().unwrap()),
    );
    assert_eq!(
        def(Kind::Number)
            .format(&Value::number("2.50".parse().unwrap()), &units)
            .unwrap(),
        "2.5"
    );
    reads(Kind::Whole, &units, "-3", &Value::Whole(-3));
    reads(Kind::Whole, &units, "3.0", &Value::Whole(3));
    reads(
        Kind::Text,
        &units,
        " Door: A ",
        &Value::Text(" Door: A ".to_owned()),
    );
    for (text, yes) in [("yes", true), ("Yes", true), ("NO", false), ("no", false)] {
        reads(Kind::YesNo, &units, text, &Value::YesNo(yes));
    }
    assert_eq!(
        def(Kind::YesNo)
            .format(&Value::YesNo(true), &units)
            .unwrap(),
        "Yes"
    );
    let finish = Definition::choice(
        id("P2"),
        "Finish",
        Holder::Type,
        vec![Choice::new("oak", "Oak"), Choice::new("ash", "Ash")],
    )
    .unwrap();
    let oak = Value::Choice("oak".to_owned());
    assert_eq!(finish.parse("Oak", &units).unwrap(), oak);
    assert_eq!(finish.parse("oak", &units).unwrap(), oak);
    assert_eq!(finish.format(&oak, &units).unwrap(), "Oak");
    let element = Value::Element(id("01JD0000000000000000000000"));
    reads(
        Kind::Element,
        &units,
        "01JD0000000000000000000000",
        &element,
    );
    reads(
        Kind::Material,
        &units,
        "concrete-30",
        &Value::Material(id("concrete-30")),
    );
    let link = def(Kind::Link)
        .parse("https://example.org", &units)
        .unwrap();
    assert_eq!(link, Value::Link("https://example.org/".to_owned()));
    reads(
        Kind::Link,
        &units,
        "HTTP://Example.org/a b",
        &Value::Link("http://example.org/a%20b".to_owned()),
    );
}
