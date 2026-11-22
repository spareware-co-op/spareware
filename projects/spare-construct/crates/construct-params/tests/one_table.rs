//! The one units table and what formulas need from this crate: centimetres, inches and feet held
//! with exact factors for formula literals but neither typed nor shown, a definition that carries a
//! formula's text with rows without one keeping their bytes, and areas and volumes shown exactly.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::{
    Area, Definition, Format, Holder, Kind, Length, ParamError, Unit, Units, Value, Volume,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

#[test]
fn centimetres_inches_and_feet_are_whole_grid_steps() {
    let steps = |unit: Unit| unit.factor().unwrap();
    assert_eq!(steps(Unit::Centimetre), (64_000, 1));
    // 25.4 mm and 304.8 mm, by the international inch of 1959.
    assert_eq!(steps(Unit::Inch), (162_560, 1));
    assert_eq!(steps(Unit::Foot), (1_950_720, 1));
    assert_eq!(
        i128::from(Length::from_mm(254).steps()),
        10 * steps(Unit::Inch).0
    );
    assert_eq!(steps(Unit::Foot).0, 12 * steps(Unit::Inch).0);
    for unit in [Unit::Centimetre, Unit::Inch, Unit::Foot] {
        assert_eq!(unit.kind(), Kind::Length);
        assert!(!unit.shown(), "{unit:?}");
    }
}

#[test]
fn every_unit_has_one_symbol_and_the_metric_ones_are_shown() {
    let symbols: Vec<&str> = Unit::ALL.iter().map(|u| u.symbol()).collect();
    assert_eq!(
        symbols,
        [
            "mm", "m", "mm2", "m2", "mm3", "m3", "deg", "%", "", "cm", "in", "ft"
        ]
    );
    let shown: Vec<Unit> = Unit::ALL.into_iter().filter(|u| u.shown()).collect();
    assert_eq!(&shown, &Unit::ALL[..9]);
}

#[test]
fn imperial_and_centimetres_are_not_display_units_yet() {
    let mut units = Units::metric();
    for unit in [Unit::Centimetre, Unit::Inch, Unit::Foot] {
        let format = Format { unit, places: 0 };
        assert_eq!(
            units.set(Kind::Length, format),
            Err(ParamError::BadFormat {
                kind: Kind::Length,
                unit,
                places: 0
            })
        );
    }
    assert_eq!(units, Units::metric());
    let row = r#"{"length":{"unit":"in","places":0},"area":{"unit":"m2","places":2},"volume":{"unit":"m3","places":2},"angle":{"unit":"degree","places":2},"slope":{"unit":"percent","places":2}}"#;
    assert!(serde_json::from_str::<Units>(row).is_err());
}

#[test]
fn a_definition_carries_a_formula_and_one_without_keeps_its_bytes() {
    let plain = Definition::new(id("P1"), "Width", Kind::Length, Holder::Type).unwrap();
    let plain_row = r#"{"id":"P1","name":"Width","kind":"length","holder":"type","choices":[]}"#;
    assert_eq!(plain.to_json().unwrap(), plain_row);
    assert_eq!(plain.formula(), None);
    assert_eq!(Definition::from_json(plain_row).unwrap(), plain);

    let leaf = Definition::new(id("P2"), "LeafWidth", Kind::Length, Holder::Type)
        .unwrap()
        .with_formula("Width - 2 * Frame")
        .unwrap();
    let leaf_row = r#"{"id":"P2","name":"LeafWidth","kind":"length","holder":"type","choices":[],"formula":"Width - 2 * Frame"}"#;
    assert_eq!(leaf.formula(), Some("Width - 2 * Frame"));
    assert_eq!(leaf.to_json().unwrap(), leaf_row);
    assert_eq!(Definition::from_json(leaf_row).unwrap(), leaf);
}

#[test]
fn only_numbers_and_yes_or_no_take_a_formula() {
    let define = |kind: Kind, formula: &str| {
        Definition::new(id("P1"), "X", kind, Holder::Instance)
            .unwrap()
            .with_formula(formula)
    };
    let choice = Definition::choice(
        id("P1"),
        "X",
        Holder::Instance,
        vec![construct_params::Choice::new("a", "A")],
    )
    .unwrap();
    assert!(matches!(
        choice.with_formula("1"),
        Err(ParamError::BadDefinition { .. })
    ));
    for kind in Kind::ALL.into_iter().filter(|k| *k != Kind::Choice) {
        let made = define(kind, "1");
        assert_eq!(made.is_ok(), kind.takes_formula(), "{kind:?}");
        if !kind.takes_formula() {
            assert!(matches!(made, Err(ParamError::BadDefinition { .. })));
        }
    }
    for text in ["", " 1", "1 "] {
        assert!(define(Kind::Length, text).is_err(), "{text:?}");
    }
    let row =
        r#"{"id":"P1","name":"Mark","kind":"text","holder":"instance","choices":[],"formula":"1"}"#;
    assert!(Definition::from_json(row).is_err());
}

#[test]
fn areas_and_volumes_show_exactly_in_square_and_cubic_millimetres() {
    assert_eq!(Area::from_mm2(5_760_000).to_string(), "5760000 mm²");
    assert_eq!(
        Area::from_square_steps(1).to_string(),
        "0.0000000244140625 mm²"
    );
    assert_eq!(Volume::from_mm3(-3).to_string(), "-3 mm³");
    assert_eq!(
        Value::Volume(Volume::from_cubic_steps(131_072_000_000)).stored(),
        "volume:q0:131072000000"
    );
    assert_eq!(
        Volume::from_cubic_steps(131_072_000_000).to_string(),
        "0.5 mm³"
    );
}
