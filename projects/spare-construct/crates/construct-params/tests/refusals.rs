//! What is refused, and that every refusal is a value naming what went wrong: the parameter, and
//! both kinds where two kinds met.

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
    Area, Choice, Definition, Format, Holder, Kind, Length, Op, ParamError, Unit, Units, Value,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn def(name: &str, kind: Kind) -> Definition {
    Definition::new(id("P1"), name, kind, Holder::Instance).unwrap()
}

fn finish() -> Definition {
    Definition::choice(
        id("P2"),
        "Finish",
        Holder::Type,
        vec![Choice::new("oak", "Oak"), Choice::new("ash", "Ash")],
    )
    .unwrap()
}

#[test]
fn a_value_of_the_wrong_kind_is_refused_naming_the_parameter_and_both_kinds() {
    let height = def("Height", Kind::Length);
    let error = height.check(&Value::Area(Area::from_mm2(5))).unwrap_err();
    assert_eq!(
        error,
        ParamError::WrongKind {
            parameter: "Height".to_owned(),
            expected: Kind::Length,
            found: Kind::Area,
        }
    );
    let text = error.to_string();
    assert!(text.contains("Height"), "{text}");
    assert!(text.contains("length"), "{text}");
    assert!(text.contains("area"), "{text}");
    // Writing goes through the same check.
    assert_eq!(height.write(&Value::Whole(3)).unwrap_err().to_string(), {
        ParamError::WrongKind {
            parameter: "Height".to_owned(),
            expected: Kind::Length,
            found: Kind::Whole,
        }
        .to_string()
    });
}

#[test]
fn a_unit_of_the_wrong_kind_is_refused_naming_the_parameter_and_both_kinds() {
    let units = Units::metric();
    let height = def("Height", Kind::Length);
    for (text, unit, found) in [
        ("2.4 m2", "m2", Kind::Area),
        ("2.4m³", "m³", Kind::Volume),
        ("45°", "°", Kind::Angle),
        ("2 %", "%", Kind::Slope),
        ("1:60", ":", Kind::Slope),
    ] {
        assert_eq!(
            height.parse(text, &units),
            Err(ParamError::WrongUnit {
                parameter: "Height".to_owned(),
                expected: Kind::Length,
                unit: unit.to_owned(),
                found,
            }),
            "{text}"
        );
    }
    let floor = def("Floor area", Kind::Area);
    let error = floor.parse("3 m", &units).unwrap_err();
    assert_eq!(
        error,
        ParamError::WrongUnit {
            parameter: "Floor area".to_owned(),
            expected: Kind::Area,
            unit: "m".to_owned(),
            found: Kind::Length,
        }
    );
    let text = error.to_string();
    assert!(text.contains("Floor area") && text.contains("area") && text.contains("length"));
    // A unit on a kind that has none.
    assert_eq!(
        def("Count", Kind::Whole).parse("3 m", &units),
        Err(ParamError::WrongUnit {
            parameter: "Count".to_owned(),
            expected: Kind::Whole,
            unit: "m".to_owned(),
            found: Kind::Length,
        })
    );
}

#[test]
fn feet_and_inches_are_not_read_yet() {
    let units = Units::metric();
    let height = def("Height", Kind::Length);
    for (text, unit) in [("8 ft", "ft"), ("8'", "'"), ("96 in", "in"), ("96\"", "\"")] {
        assert_eq!(
            height.parse(text, &units),
            Err(ParamError::UnknownUnit {
                parameter: "Height".to_owned(),
                unit: unit.to_owned(),
            }),
            "{text}"
        );
    }
}

#[test]
fn unit_symbols_are_case_sensitive() {
    // As the SI writes them: M is mega, not metre.
    let height = def("Height", Kind::Length);
    assert_eq!(
        height.parse("2400 MM", &Units::metric()),
        Err(ParamError::UnknownUnit {
            parameter: "Height".to_owned(),
            unit: "MM".to_owned(),
        })
    );
}

#[test]
fn text_that_is_not_a_number_is_refused() {
    let units = Units::metric();
    let height = def("Height", Kind::Length);
    for text in ["", "   ", "m", "abc", "2..4 m", ".", "-", "+-2", "2,400"] {
        let error = height.parse(text, &units).unwrap_err();
        assert!(
            matches!(
                error,
                ParamError::NotANumber { ref parameter, .. }
                    | ParamError::UnknownUnit { ref parameter, .. }
                    if parameter == "Height"
            ),
            "{text}: {error:?}"
        );
    }
    assert_eq!(
        height.parse("abc", &units),
        Err(ParamError::NotANumber {
            parameter: "Height".to_owned(),
            text: "abc".to_owned(),
        })
    );
}

#[test]
fn a_whole_number_refuses_a_fraction() {
    assert_eq!(
        def("Count", Kind::Whole).parse("2.5", &Units::metric()),
        Err(ParamError::NotWhole {
            parameter: "Count".to_owned(),
            text: "2.5".to_owned(),
        })
    );
}

#[test]
fn a_value_too_large_to_store_is_refused() {
    let units = Units::metric();
    let too_big = "99999999999999999999";
    assert_eq!(
        def("Height", Kind::Length).parse(too_big, &units),
        Err(ParamError::OutOfRange {
            parameter: "Height".to_owned(),
            text: too_big.to_owned(),
        })
    );
    assert_eq!(
        def("Count", Kind::Whole).parse(too_big, &units),
        Err(ParamError::OutOfRange {
            parameter: "Count".to_owned(),
            text: too_big.to_owned(),
        })
    );
    // A vertical slope has no ratio.
    assert_eq!(
        def("Fall", Kind::Slope).parse("1:0", &units),
        Err(ParamError::OutOfRange {
            parameter: "Fall".to_owned(),
            text: "1:0".to_owned(),
        })
    );
}

#[test]
fn a_choice_not_in_the_list_is_refused() {
    let units = Units::metric();
    assert_eq!(
        finish().parse("Pine", &units),
        Err(ParamError::NotAChoice {
            parameter: "Finish".to_owned(),
            text: "Pine".to_owned(),
        })
    );
    assert_eq!(
        finish().check(&Value::Choice("pine".to_owned())),
        Err(ParamError::NotAChoice {
            parameter: "Finish".to_owned(),
            text: "pine".to_owned(),
        })
    );
}

#[test]
fn links_ids_and_yes_or_no_are_checked() {
    let units = Units::metric();
    for text in [
        "not a link",
        "ftp://example.org/a",
        "mailto:someone@example.org",
        "",
    ] {
        assert_eq!(
            def("Product page", Kind::Link).parse(text, &units),
            Err(ParamError::BadLink {
                parameter: "Product page".to_owned(),
                text: text.to_owned(),
            }),
            "{text}"
        );
    }
    for kind in [Kind::Element, Kind::Material] {
        assert_eq!(
            def("Host", kind).parse("not an id", &units),
            Err(ParamError::BadId {
                parameter: "Host".to_owned(),
                text: "not an id".to_owned(),
            })
        );
    }
    assert_eq!(
        def("Fire rated", Kind::YesNo).parse("maybe", &units),
        Err(ParamError::NotYesOrNo {
            parameter: "Fire rated".to_owned(),
            text: "maybe".to_owned(),
        })
    );
}

#[test]
fn stored_text_of_another_kind_or_spelling_is_refused() {
    let height = def("Height", Kind::Length);
    assert_eq!(
        height.read("area:5"),
        Err(ParamError::WrongKind {
            parameter: "Height".to_owned(),
            expected: Kind::Length,
            found: Kind::Area,
        })
    );
    for text in [
        "length:02400",
        "length:+1",
        "length:-0",
        "length: 1",
        "length",
        "2400",
        "",
    ] {
        assert_eq!(
            height.read(text),
            Err(ParamError::BadStored {
                parameter: "Height".to_owned(),
                text: text.to_owned(),
            }),
            "{text}"
        );
    }
    assert_eq!(
        finish().read("choice:pine"),
        Err(ParamError::NotAChoice {
            parameter: "Finish".to_owned(),
            text: "pine".to_owned(),
        })
    );
}

#[test]
fn arithmetic_on_unlike_kinds_is_refused_naming_both_kinds() {
    let length = Value::Length(Length::from_mm(2400));
    let area = Value::Area(Area::from_mm2(10));
    let error = length.add(&area).unwrap_err();
    assert_eq!(
        error,
        ParamError::Mismatch {
            op: Op::Add,
            left: Kind::Length,
            right: Kind::Area,
        }
    );
    assert!(error.to_string().contains("length") && error.to_string().contains("area"));
    assert_eq!(
        area.subtract(&length),
        Err(ParamError::Mismatch {
            op: Op::Subtract,
            left: Kind::Area,
            right: Kind::Length,
        })
    );
    assert_eq!(
        area.multiply(&area),
        Err(ParamError::Mismatch {
            op: Op::Multiply,
            left: Kind::Area,
            right: Kind::Area,
        })
    );
    let text = Value::Text("a".to_owned());
    assert_eq!(
        text.add(&text),
        Err(ParamError::Mismatch {
            op: Op::Add,
            left: Kind::Text,
            right: Kind::Text,
        })
    );
}

#[test]
fn arithmetic_that_overflows_is_refused() {
    let big = Value::Length(Length::from_mm(i64::MAX));
    let one = Value::Length(Length::from_mm(1));
    assert_eq!(
        big.add(&one),
        Err(ParamError::Overflow {
            op: Op::Add,
            kind: Kind::Length,
        })
    );
    assert_eq!(
        big.multiply(&big),
        Err(ParamError::Overflow {
            op: Op::Multiply,
            kind: Kind::Area,
        })
    );
    assert_eq!(
        Value::Whole(i64::MIN).subtract(&Value::Whole(1)),
        Err(ParamError::Overflow {
            op: Op::Subtract,
            kind: Kind::Whole,
        })
    );
}

#[test]
fn a_bad_definition_is_refused() {
    assert!(matches!(
        Definition::new(id("P1"), "", Kind::Length, Holder::Instance),
        Err(ParamError::BadDefinition { .. })
    ));
    // A choice needs its list.
    assert!(matches!(
        Definition::new(id("P1"), "Finish", Kind::Choice, Holder::Type),
        Err(ParamError::BadDefinition { .. })
    ));
    assert!(matches!(
        Definition::choice(id("P1"), "Finish", Holder::Type, vec![]),
        Err(ParamError::BadDefinition { .. })
    ));
    assert!(matches!(
        Definition::choice(
            id("P1"),
            "Finish",
            Holder::Type,
            vec![Choice::new("oak", "Oak"), Choice::new("oak", "Light oak")]
        ),
        Err(ParamError::BadDefinition { .. })
    ));
    assert!(matches!(
        Definition::choice(
            id("P1"),
            "Finish",
            Holder::Type,
            vec![Choice::new("oak", "Oak"), Choice::new("ash", "Oak")]
        ),
        Err(ParamError::BadDefinition { .. })
    ));
    assert!(matches!(
        Definition::choice(
            id("P1"),
            "Finish",
            Holder::Type,
            vec![Choice::new("", "Oak")]
        ),
        Err(ParamError::BadDefinition { .. })
    ));
    // The same checks run when a definition is read back.
    let row = r#"{"id":"P1","name":"Height","kind":"length","holder":"instance","choices":[{"key":"oak","label":"Oak"}]}"#;
    assert!(matches!(
        Definition::from_json(row),
        Err(ParamError::BadDefinition { .. })
    ));
    assert!(matches!(
        Definition::from_json(
            r#"{"id":"not an id","name":"H","kind":"length","holder":"instance","choices":[]}"#
        ),
        Err(ParamError::BadDefinition { .. })
    ));
}

#[test]
fn a_display_format_of_the_wrong_kind_is_refused() {
    let mut units = Units::metric();
    assert_eq!(
        units.set(
            Kind::Length,
            Format {
                unit: Unit::SquareMetre,
                places: 2,
            }
        ),
        Err(ParamError::BadFormat {
            kind: Kind::Length,
            unit: Unit::SquareMetre,
            places: 2,
        })
    );
    assert_eq!(
        units.set(
            Kind::Length,
            Format {
                unit: Unit::Metre,
                places: 10,
            }
        ),
        Err(ParamError::BadFormat {
            kind: Kind::Length,
            unit: Unit::Metre,
            places: 10,
        })
    );
    assert_eq!(
        units.set(
            Kind::Text,
            Format {
                unit: Unit::Metre,
                places: 0,
            }
        ),
        Err(ParamError::BadFormat {
            kind: Kind::Text,
            unit: Unit::Metre,
            places: 0,
        })
    );
    assert_eq!(units, Units::metric());
}
