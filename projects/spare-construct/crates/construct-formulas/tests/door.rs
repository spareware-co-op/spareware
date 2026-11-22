//! A door family's parameters, as a family recipe writes them (note A07's door, in the same
//! style): sizes in mixed units, formulas of other parameters in any order, and a choice. The
//! answers are `construct-params` values, each the old answer converted exactly by
//! `construct-params`' own reading of typed text.

#![allow(clippy::unwrap_used, reason = "test code")]

use construct_formulas::{Entry, evaluate};
use construct_params::{Definition, Holder, Kind, Units, Value};
use kit_core::Id;

fn entry(name: &str, text: &str, kind: Kind) -> Entry {
    Entry {
        name: name.to_owned(),
        text: text.to_owned(),
        kind,
    }
}

/// An old answer, as typed text read by construct-params: `742.8 mm`, `90°`, `2`.
fn typed(kind: Kind, text: &str) -> Value {
    Definition::new(Id::parse("P1").unwrap(), "P", kind, Holder::Instance)
        .unwrap()
        .parse(text, &Units::metric())
        .unwrap()
}

#[test]
fn a_door_works_out_exactly() {
    let table = [
        entry(
            "LeafWidth",
            "Width - 2 * FrameWidth - 2 * Clearance",
            Kind::Length,
        ),
        entry("Width", "2 ft 8 in", Kind::Length),
        entry("FrameWidth", "32 mm", Kind::Length),
        entry("Clearance", "3", Kind::Length),
        entry("Height", "2040", Kind::Length),
        entry("Panels", "Height > 2100 mm ? 3 : 2", Kind::Number),
        entry(
            "PanelHeight",
            "(Height - FrameWidth) / Panels",
            Kind::Length,
        ),
        entry("Swing", "90 deg", Kind::Angle),
    ];
    let values: Vec<Value> = evaluate(&table).into_iter().map(Result::unwrap).collect();
    // The old answers: 742.8, 812.8, 32, 3, 2040 (mm), 2 (a count), 1004 (mm), 90 (degrees).
    let length = |mm| typed(Kind::Length, mm);
    assert_eq!(
        values,
        [
            length("742.8 mm"),
            length("812.8 mm"),
            length("32 mm"),
            length("3 mm"),
            length("2040 mm"),
            typed(Kind::Number, "2"),
            length("1004 mm"),
            typed(Kind::Angle, "90°"),
        ]
    );
    // In stored text: grid steps and angle units.
    assert_eq!(values[0].stored(), "length:q0:4753920");
    assert_eq!(values[7].stored(), "angle:810000000");
}

#[test]
fn a_door_that_doesnt_add_up_says_why() {
    let table = [
        entry("Width", "820 mm", Kind::Length),
        entry("Swing", "Width / 9", Kind::Angle),
        entry("Area", "Width + Width * Width", Kind::Area),
    ];
    let out = evaluate(&table);
    let why = out[1].as_ref().unwrap_err();
    assert_eq!(why.code, "Units");
    assert_eq!(
        why.message,
        "This works out to a length, and it should be an angle."
    );
    // A length plus an area, refused by the type check before anything is worked out.
    let why = out[2].as_ref().unwrap_err();
    assert_eq!(why.code, "Units");
    assert_eq!(why.message, "A length and an area don't add.");
}
