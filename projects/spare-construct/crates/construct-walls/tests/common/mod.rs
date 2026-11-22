//! What the tests share: lengths, layers, the wall parameters and the category table.

#![allow(
    dead_code,
    clippy::unwrap_used,
    reason = "test code; each test file uses part of this fixture"
)]

use construct_materials::MaterialId;
use construct_params::Length;
use construct_types::Parameters;
use construct_walls::{Assembly, Function, Layer, add_parameters};
use kit_core::Id;

/// Text as an ID.
pub fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// Whole millimetres as a length.
pub fn mm(mm: i64) -> Length {
    Length::from_mm(mm)
}

/// Whole millimetres as lengths.
pub fn mms(list: &[i64]) -> Vec<Length> {
    list.iter().map(|value| mm(*value)).collect()
}

/// A layer.
pub fn layer(function: Function, thickness: i64, material: &str, wraps: bool) -> Layer {
    Layer::new(
        function,
        mm(thickness),
        MaterialId::parse(material).unwrap(),
        wraps,
    )
}

/// A finish layer that does not wrap.
pub fn finish(thickness: i64) -> Layer {
    layer(Function::Finish, thickness, "finish", false)
}

/// A structure layer.
pub fn structure(thickness: i64) -> Layer {
    layer(Function::Structure, thickness, "structure", false)
}

/// A membrane that does not wrap.
pub fn membrane(thickness: i64) -> Layer {
    layer(Function::Membrane, thickness, "membrane", false)
}

/// 10 finish, 20 structure (the core), 30 finish: three layers, 60 thick.
pub fn three() -> Assembly {
    Assembly::new(vec![finish(10), structure(20), finish(30)], 1, 2).unwrap()
}

/// The wall parameters, alone.
pub fn wall_parameters() -> Parameters {
    let mut params = Parameters::new();
    add_parameters(&mut params).unwrap();
    params
}
