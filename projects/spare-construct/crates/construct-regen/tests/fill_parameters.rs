//! The recipe's parameter arguments are filled with whole numbers: a length is read in whole
//! millimetres, and one that is not a whole millimetre is a build failure naming the parameter.

#![cfg(feature = "server")]

use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use construct_regen::parameters::filled;
use kit_core::Id;
use serde_json::json;
use std::collections::BTreeMap;

fn recipe() -> Recipe {
    let mut args = BTreeMap::new();
    args.insert("length".to_owned(), json!({"param": "wall-length"}));
    Recipe {
        owner: "spare".to_owned(),
        schema: 1,
        op: "wall.straight".to_owned(),
        op_version: 1,
        args,
    }
}

#[test]
fn a_whole_millimetre_length_fills_as_its_millimetres() {
    let value = Value::Length(Length::from_mm(2400));
    let out = filled(&recipe(), |_: &Id| Ok(Some(value.clone()))).unwrap();
    assert_eq!(out.args.get("length"), Some(&json!(2400)));
}

#[test]
fn a_length_between_millimetres_is_refused_naming_the_parameter() {
    let value = Value::Length(Length::from_steps(80_000));
    let err = filled(&recipe(), |_: &Id| Ok(Some(value.clone()))).unwrap_err();
    assert_eq!(err.code, "BadParameter");
    assert!(err.message.contains("wall-length"), "{}", err.message);
    assert!(err.message.contains("12.5 mm"), "{}", err.message);
    assert!(
        err.message.contains("takes whole millimetres only"),
        "{}",
        err.message
    );
}
