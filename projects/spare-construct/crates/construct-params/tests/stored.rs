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
        (Value::Length(Length::from_mm(2400)), "length:2400"),
        (Value::Length(Length::from_mm(-150)), "length:-150"),
        (Value::Area(Area::from_mm2(5_760_000)), "area:5760000"),
        (Value::Volume(Volume::from_mm3(1)), "volume:1"),
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
        r#"{"A":"yesno:yes","H":"length:2100","M":"text:D01"}"#
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
    props.insert("H".to_owned(), "length:900".to_owned());
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
