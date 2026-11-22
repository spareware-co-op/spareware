//! A parameter's value given by a formula over other parameters of the same element or type
//! (CON-A2-F2-P2): definitions carry a formula's text, are worked out in the order they depend on
//! each other, a loop is refused naming its members, and a formula must give its parameter's
//! kind.

#![allow(clippy::unwrap_used, reason = "test code")]

use construct_formulas::{check, define, work_out};
use construct_params::{Decimal, Definition, Holder, Kind, Length, Value, Values};
use kit_core::Id;
use std::collections::BTreeMap;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn plain(key: &str, name: &str, kind: Kind, holder: Holder) -> Definition {
    Definition::new(id(key), name, kind, holder).unwrap()
}

fn with(key: &str, name: &str, kind: Kind, holder: Holder, formula: &str) -> Definition {
    define(plain(key, name, kind, holder), formula).unwrap()
}

fn mm(n: i32) -> Value {
    Value::Length(Length::from_mm(n))
}

/// A door type: its width and frame given, its leaf worked out from them in any order.
fn door() -> (Vec<Definition>, Values) {
    let defs = vec![
        with(
            "P3",
            "Leaf",
            Kind::Length,
            Holder::Type,
            "Opening - 2*Frame",
        ),
        plain("P1", "Width", Kind::Length, Holder::Type),
        plain("P2", "Frame", Kind::Length, Holder::Type),
        with("P4", "Opening", Kind::Length, Holder::Type, "Width-10mm"),
        with("P5", "Wide", Kind::YesNo, Holder::Instance, "Leaf > 800 mm"),
        with(
            "P6",
            "Panels",
            Kind::Whole,
            Holder::Instance,
            "Wide ? 3 : 2",
        ),
        plain("P7", "Mark", Kind::Text, Holder::Instance),
    ];
    let mut given = Values::new();
    given.set(&defs[1], mm(926)).unwrap();
    given.set(&defs[2], mm(32)).unwrap();
    given.set(&defs[6], Value::Text("D01".to_owned())).unwrap();
    (defs, given)
}

#[test]
fn formulas_are_worked_out_in_the_order_they_depend_on_each_other() {
    let (defs, given) = door();
    assert_eq!(check(&defs), BTreeMap::new());
    let out = work_out(&defs, &given);
    let expected: BTreeMap<Id, Result<Value, _>> = [
        (id("P3"), Ok(mm(852))),
        (id("P4"), Ok(mm(916))),
        (id("P5"), Ok(Value::YesNo(true))),
        (id("P6"), Ok(Value::Whole(3))),
    ]
    .into_iter()
    .collect();
    assert_eq!(out, expected);
}

#[test]
fn a_formula_is_stored_in_its_one_spelling() {
    let (defs, _) = door();
    assert_eq!(defs[0].formula(), Some("Opening - 2 * Frame"));
    assert_eq!(defs[3].formula(), Some("Width - 10 mm"));
    assert_eq!(
        defs[0].to_json().unwrap(),
        r#"{"id":"P3","name":"Leaf","kind":"length","holder":"type","choices":[],"formula":"Opening - 2 * Frame"}"#
    );
    // A row read back in another spelling is refused where it is worked out, not guessed at.
    let other = Definition::from_json(
        r#"{"id":"P3","name":"Leaf","kind":"length","holder":"type","choices":[],"formula":"Opening-2*Frame"}"#,
    )
    .unwrap();
    let mut defs = defs;
    defs[0] = other;
    let refused = check(&defs);
    assert_eq!(refused[&id("P3")].code, "Spelling");
    assert!(
        refused[&id("P3")]
            .message
            .contains("\"Opening - 2 * Frame\""),
        "{}",
        refused[&id("P3")].message
    );
}

#[test]
fn a_loop_is_refused_naming_its_members() {
    let defs = vec![
        with("A", "a", Kind::Length, Holder::Instance, "b + 1 mm"),
        with("B", "b", Kind::Length, Holder::Instance, "c"),
        with("C", "c", Kind::Length, Holder::Instance, "a * 2"),
        with("D", "d", Kind::Length, Holder::Instance, "a"),
    ];
    let refused = check(&defs);
    for key in ["A", "B", "C"] {
        let e = &refused[&id(key)];
        assert_eq!(e.code, "Circular");
        assert_eq!(e.message, "a, b, c are defined from each other.");
    }
    assert_eq!(refused[&id("D")].code, "Depends");
    let out = work_out(&defs, &Values::new());
    assert_eq!(out[&id("A")].as_ref().unwrap_err().code, "Circular");
}

#[test]
fn a_formula_must_give_its_parameters_kind() {
    let defs = vec![
        plain("W", "Width", Kind::Length, Holder::Instance),
        with("A", "Area", Kind::Area, Holder::Instance, "Width * Width"),
        with("B", "Bad", Kind::Length, Holder::Instance, "Width * Width"),
        with("S", "Sum", Kind::Area, Holder::Instance, "Width + Area"),
        with("M", "UsesMark", Kind::Length, Holder::Instance, "Mark"),
        plain("K", "Mark", Kind::Text, Holder::Instance),
        with("R", "Ratio", Kind::Number, Holder::Instance, "Width / 3 mm"),
    ];
    let refused = check(&defs);
    assert_eq!(
        refused[&id("B")].message,
        "This works out to an area, and it should be a length."
    );
    assert_eq!(refused[&id("S")].message, "A length and an area don't add.");
    assert_eq!(refused[&id("M")].code, "Type");
    assert_eq!(refused.len(), 3, "{refused:?}");

    let mut given = Values::new();
    given.set(&defs[0], mm(30)).unwrap();
    let out = work_out(&defs, &given);
    assert_eq!(
        out[&id("A")],
        Ok(Value::Area(construct_params::Area::from_mm2(900)))
    );
    assert_eq!(out[&id("R")], Ok(Value::number(Decimal::from(10))));
    // The type check's refusal stands when values are worked out.
    assert_eq!(out[&id("S")].as_ref().unwrap_err().code, "Units");
}

#[test]
fn a_type_parameter_uses_only_the_types_parameters() {
    let defs = vec![
        plain("H", "Height", Kind::Length, Holder::Instance),
        with("T", "Head", Kind::Length, Holder::Type, "Height + 50 mm"),
        plain("W", "Width", Kind::Length, Holder::Type),
        with("I", "Sill", Kind::Length, Holder::Instance, "Width / 2"),
    ];
    let refused = check(&defs);
    assert_eq!(refused[&id("T")].code, "Holder");
    assert!(!refused.contains_key(&id("I")));
}

#[test]
fn missing_values_and_doubled_names_are_named() {
    let defs = vec![
        plain("W", "Width", Kind::Length, Holder::Instance),
        with("L", "Leaf", Kind::Length, Holder::Instance, "Width - 1 mm"),
        plain("X1", "Depth", Kind::Length, Holder::Instance),
        plain("X2", "Depth", Kind::Length, Holder::Instance),
        with("D", "Half", Kind::Length, Holder::Instance, "Depth / 2"),
        with("U", "Other", Kind::Length, Holder::Instance, "Nothing"),
    ];
    let out = work_out(&defs, &Values::new());
    let leaf = out[&id("L")].as_ref().unwrap_err();
    assert_eq!(leaf.code, "Depends");
    assert_eq!(leaf.message, "Width fails: Width has no value.");
    assert_eq!(out[&id("D")].as_ref().unwrap_err().code, "Name");
    assert_eq!(out[&id("U")].as_ref().unwrap_err().code, "UnknownName");
}

#[test]
fn a_length_off_the_grid_is_refused_naming_the_nearest() {
    let defs = vec![
        plain("W", "Width", Kind::Length, Holder::Instance),
        with("T", "Third", Kind::Length, Holder::Instance, "Width / 3"),
    ];
    let mut given = Values::new();
    given.set(&defs[0], mm(1000)).unwrap();
    let e = work_out(&defs, &given)[&id("T")].clone().unwrap_err();
    assert_eq!(e.code, "OffGrid");
    assert_eq!(
        e.nearest,
        Some((
            Value::Length(Length::from_steps(2_133_333)),
            Value::Length(Length::from_steps(2_133_334)),
        ))
    );
}

#[test]
fn a_kind_no_formula_gives_cannot_carry_one() {
    let e = define(plain("M", "Mark", Kind::Text, Holder::Instance), "1").unwrap_err();
    assert_eq!(e.code, "Definition");
    let e = define(plain("W", "Width", Kind::Length, Holder::Instance), "1 +").unwrap_err();
    assert_eq!(e.code, "Syntax");
}
