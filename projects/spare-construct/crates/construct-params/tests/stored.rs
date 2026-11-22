//! The stored form: one spelling for each value, the same bytes for the same values, and the
//! element record's `props` map carrying them unchanged.

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
    Angle, Area, Choice, Definition, Holder, Kind, Length, Slope, Units, Value, Values, Volume,
};
use kit_core::Id;
use std::collections::BTreeMap;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn every_kind() -> Vec<(Value, &'static str)> {
    vec![
        (Value::Length(Length::from_mm(2400)), "length:q0:15360000"),
        (Value::Length(Length::from_mm(-150)), "length:q0:-960000"),
        (Value::Length(Length::from_steps(80_000)), "length:q0:80000"),
        (Value::Length(Length::from_steps(1)), "length:q0:1"),
        (Value::Length(Length::from_steps(0)), "length:q0:0"),
        (
            Value::Area(Area::from_mm2(5_760_000)),
            "area:q0:235929600000000",
        ),
        (Value::Area(Area::from_square_steps(1)), "area:q0:1"),
        (Value::Volume(Volume::from_mm3(1)), "volume:q0:262144000000"),
        (Value::Volume(Volume::from_cubic_steps(-7)), "volume:q0:-7"),
        (
            Value::Angle(Angle::from_units(405_000_000)),
            "angle:405000000",
        ),
        (Value::Slope(Slope::new(1, 40).unwrap()), "slope:1/40"),
        (Value::Slope(Slope::new(-3, 7).unwrap()), "slope:-3/7"),
        (Value::Slope(Slope::new(0, 5).unwrap()), "slope:0/1"),
        (Value::number("2.50".parse().unwrap()), "number:2.5"),
        (Value::number("-0.000".parse().unwrap()), "number:0"),
        (Value::number("1200".parse().unwrap()), "number:1200"),
        (Value::Whole(-3), "whole:-3"),
        (Value::Text("a: b".to_owned()), "text:a: b"),
        (Value::Text(String::new()), "text:"),
        (Value::YesNo(true), "yesno:yes"),
        (Value::YesNo(false), "yesno:no"),
        (Value::Choice("oak".to_owned()), "choice:oak"),
        (
            Value::Element(id("01JD0000000000000000000000")),
            "element:01JD0000000000000000000000",
        ),
        (Value::Material(id("concrete-30")), "material:concrete-30"),
        (
            Value::link("https://example.org").unwrap(),
            "link:https://example.org/",
        ),
    ]
}

#[test]
fn every_kind_has_one_stored_spelling() {
    for (value, text) in every_kind() {
        assert_eq!(value.stored(), text);
        assert_eq!(Value::from_stored(text), Some(value), "{text}");
    }
}

#[test]
fn other_spellings_are_not_stored_values() {
    for text in [
        "",
        "length",
        "length:",
        "length:+1",
        "length:01",
        "length:-0",
        "length:1.0",
        "length: 1",
        "Length:1",
        "length:q0:",
        "length:q0:+1",
        "length:q0:01",
        "length:q0:-0",
        "length:q0:1.0",
        "length:q0: 1",
        "length:q0:9223372036854775808",
        "length:q1:1",
        "length:Q0:1",
        "length:q0:q0:1",
        "area:q0:-0",
        "volume:q0:01",
        // Version 1 millimetres too large to be grid steps.
        "length:9223372036854775807",
        "metres:1",
        "slope:2/80",
        "slope:1/-40",
        "slope:-1/-40",
        "slope:1/0",
        "slope:0/5",
        "slope:1",
        "number:2.50",
        "number:+2.5",
        "number:-0",
        "number:.5",
        "number:1_000",
        "whole:1.0",
        "yesno:Yes",
        "yesno:true",
        "choice:",
        "element:",
        "element:not an id",
        "link:https://example.org",
        "link:ftp://example.org/",
        "link:",
    ] {
        assert_eq!(Value::from_stored(text), None, "{text}");
    }
}

#[test]
fn version_1_whole_millimetres_still_read_exactly_and_write_as_version_2() {
    // FORMAT.md, version 1: a length, area or volume with no `q0:` is whole millimetres.
    for (old, value, new) in [
        (
            "length:2400",
            Value::Length(Length::from_mm(2400)),
            "length:q0:15360000",
        ),
        (
            "length:-150",
            Value::Length(Length::from_mm(-150)),
            "length:q0:-960000",
        ),
        ("length:0", Value::Length(Length::from_mm(0)), "length:q0:0"),
        (
            "area:5760000",
            Value::Area(Area::from_mm2(5_760_000)),
            "area:q0:235929600000000",
        ),
        (
            "volume:1",
            Value::Volume(Volume::from_mm3(1)),
            "volume:q0:262144000000",
        ),
    ] {
        assert_eq!(Value::from_stored(old), Some(value.clone()), "{old}");
        assert_eq!(value.stored(), new, "{old}");
    }
    // A version 1 number is never read as grid steps, nor a version 2 one as millimetres.
    assert_ne!(
        Value::from_stored("length:2400"),
        Value::from_stored("length:q0:2400")
    );
    let height = Definition::new(id("H"), "Height", Kind::Length, Holder::Instance).unwrap();
    assert_eq!(
        height.read("length:2100").unwrap(),
        Value::Length(Length::from_mm(2100))
    );
}

#[test]
fn the_grid_step_is_the_kernels() {
    // One millimetre in quarter-nanometres is 4,000,000; the kernel's Q0 step is 625 of them.
    let quarter_nm_per_mm: u64 = 4_000_000;
    assert_eq!(
        engine_geometry::value::grid::GridStep::Q0.steps_per(quarter_nm_per_mm),
        Some(Length::STEPS_PER_MM as u64)
    );
    assert_eq!(
        Area::SQUARE_STEPS_PER_MM2,
        i128::from(Length::STEPS_PER_MM).pow(2)
    );
    assert_eq!(
        Volume::CUBIC_STEPS_PER_MM3,
        i128::from(Length::STEPS_PER_MM).pow(3)
    );
}

#[test]
fn a_length_prints_its_exact_millimetres() {
    const WALL: Length = Length::from_mm(90);
    assert_eq!(WALL.steps(), 576_000);
    assert_eq!(Length::from_mm(2400).to_string(), "2400 mm");
    assert_eq!(Length::from_steps(80_000).to_string(), "12.5 mm");
    assert_eq!(Length::from_steps(40_832).to_string(), "6.38 mm");
    assert_eq!(Length::from_steps(-3_520).to_string(), "-0.55 mm");
    assert_eq!(Length::from_steps(1).to_string(), "0.00015625 mm");
    assert_eq!(
        Length::from_steps(i64::MIN).to_string(),
        "-1441151880758558.72 mm"
    );
    assert_eq!(Length::from_mm(2400).whole_mm(), Some(2400));
    assert_eq!(Length::from_steps(80_000).whole_mm(), None);
    assert_eq!(Length::checked_from_mm(i64::MAX), None);
    assert_eq!(
        Length::checked_from_mm(-1_000_000),
        Some(Length::from_mm(-1_000_000))
    );
    assert_eq!(
        Length::from_mm(i32::MAX).steps(),
        i64::from(i32::MAX) * 6_400
    );
}

#[test]
fn a_definition_is_stored_as_json_in_one_order() {
    let height = Definition::new(id("P1"), "Height", Kind::Length, Holder::Instance).unwrap();
    let json = height.to_json().unwrap();
    assert_eq!(
        json,
        r#"{"id":"P1","name":"Height","kind":"length","holder":"instance","choices":[]}"#
    );
    assert_eq!(Definition::from_json(&json).unwrap(), height);
    let finish = Definition::choice(
        id("P2"),
        "Finish",
        Holder::Type,
        vec![Choice::new("oak", "Oak"), Choice::new("ash", "Ash")],
    )
    .unwrap();
    let json = finish.to_json().unwrap();
    assert_eq!(
        json,
        r#"{"id":"P2","name":"Finish","kind":"choice","holder":"type","choices":[{"key":"oak","label":"Oak"},{"key":"ash","label":"Ash"}]}"#
    );
    assert_eq!(Definition::from_json(&json).unwrap(), finish);
    assert_eq!(finish.id(), &id("P2"));
    assert_eq!(finish.name(), "Finish");
    assert_eq!(finish.kind(), Kind::Choice);
    assert_eq!(finish.holder(), Holder::Type);
    assert_eq!(finish.choices().len(), 2);
}

#[test]
fn every_kind_name_is_its_stored_tag() {
    for kind in Kind::ALL {
        assert_eq!(Kind::from_tag(kind.tag()), Some(kind));
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{}\"", kind.tag()));
    }
    assert_eq!(Kind::ALL.len(), 13);
}

#[test]
fn unit_settings_are_stored_as_json_in_one_order() {
    let json = serde_json::to_string(&Units::metric()).unwrap();
    assert_eq!(
        json,
        r#"{"length":{"unit":"mm","places":0},"area":{"unit":"m2","places":2},"volume":{"unit":"m3","places":2},"angle":{"unit":"degree","places":2},"slope":{"unit":"percent","places":2}}"#
    );
    assert_eq!(
        serde_json::from_str::<Units>(&json).unwrap(),
        Units::metric()
    );
    // Reading runs the same checks as setting.
    let bad = json.replace(r#""unit":"mm""#, r#""unit":"m2""#);
    assert!(serde_json::from_str::<Units>(&bad).is_err());
}

fn definitions() -> Vec<Definition> {
    vec![
        Definition::new(id("H"), "Height", Kind::Length, Holder::Instance).unwrap(),
        Definition::new(id("A"), "Fire rated", Kind::YesNo, Holder::Type).unwrap(),
        Definition::new(id("M"), "Mark", Kind::Text, Holder::Instance).unwrap(),
    ]
}

#[test]
fn values_go_into_props_by_permanent_id_and_back() {
    let defs = definitions();
    let mut first = Values::new();
    first
        .set(&defs[0], Value::Length(Length::from_mm(2100)))
        .unwrap();
    first.set(&defs[1], Value::YesNo(true)).unwrap();
    first.set(&defs[2], Value::Text("D01".to_owned())).unwrap();
    // The same values set in another order give the same bytes.
    let mut second = Values::new();
    second.set(&defs[2], Value::Text("D01".to_owned())).unwrap();
    second.set(&defs[1], Value::YesNo(true)).unwrap();
    second
        .set(&defs[0], Value::Length(Length::from_mm(2100)))
        .unwrap();
    let props = first.to_props();
    assert_eq!(props, second.to_props());
    assert_eq!(
        serde_json::to_string(&props).unwrap(),
        r#"{"A":"yesno:yes","H":"length:q0:13440000","M":"text:D01"}"#
    );
    assert_eq!(Values::from_props(&defs, &props).unwrap(), first);
    assert_eq!(
        first.get(&id("H")),
        Some(&Value::Length(Length::from_mm(2100)))
    );
    assert_eq!(first.len(), 3);
}

#[test]
fn props_that_are_not_defined_parameters_are_left_alone() {
    let defs = definitions();
    let mut props = BTreeMap::new();
    props.insert("H".to_owned(), "length:q0:5760000".to_owned());
    props.insert(
        "comment".to_owned(),
        "free text from before parameters".to_owned(),
    );
    let values = Values::from_props(&defs, &props).unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values.get(&id("H")),
        Some(&Value::Length(Length::from_mm(900)))
    );
}

#[test]
fn a_prop_of_the_wrong_kind_is_refused_naming_the_parameter() {
    let defs = definitions();
    let mut props = BTreeMap::new();
    props.insert("H".to_owned(), "yesno:yes".to_owned());
    let error = Values::from_props(&defs, &props).unwrap_err();
    assert!(error.to_string().contains("Height"), "{error}");
    let mut values = Values::new();
    assert!(values.set(&defs[0], Value::YesNo(true)).is_err());
    assert!(values.is_empty());
}
