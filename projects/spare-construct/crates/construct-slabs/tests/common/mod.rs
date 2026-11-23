//! What the tests share: lengths, layers, corners, and the floors the tests draw.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; each test file uses part of this fixture"
)]

use construct_materials::MaterialId;
use construct_params::Length;
use construct_slabs::{Boundary, Corner, FloorLayer, Function, Layers};
use construct_solver::Point;
use kit_core::Id;

/// Text as an ID.
pub fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// Whole millimetres as a length.
pub fn mm(mm: i64) -> Length {
    Length::checked_from_mm(mm).unwrap()
}

/// A layer of `material`, `thickness` whole millimetres thick.
pub fn layer(function: Function, thickness: i64, material: &str) -> FloorLayer {
    FloorLayer::new(
        function,
        mm(thickness),
        MaterialId::parse(material).unwrap(),
    )
}

/// A 50 mm screed over a 150 mm concrete slab.
pub fn slab() -> Layers {
    Layers::new(vec![
        layer(Function::Finish, 50, "screed-cement"),
        layer(Function::Structure, 150, "concrete-regular"),
    ])
    .unwrap()
}

/// A corner at whole millimetres, with its edge's ID.
pub fn at(edge: &str, x: i32, y: i32) -> Corner {
    Corner {
        id: Some(id(edge)),
        at: Point::mm(x, y),
    }
}

/// A corner at whole millimetres, its edge's ID left to the default.
pub fn bare(x: i32, y: i32) -> Corner {
    Corner {
        id: None,
        at: Point::mm(x, y),
    }
}

/// A rectangle's corners, anticlockwise from (x, y), its edges `<tag>0` to `<tag>3`.
pub fn rectangle(tag: &str, x: i32, y: i32, w: i32, d: i32) -> Vec<Corner> {
    vec![
        at(&format!("{tag}0"), x, y),
        at(&format!("{tag}1"), x + w, y),
        at(&format!("{tag}2"), x + w, y + d),
        at(&format!("{tag}3"), x, y + d),
    ]
}

/// The L-shaped floor the tests use: 8 m by 4 m along x and 4 m by 4 m more above its left
/// half, so 48 m2, edges `e0` to `e5`; and a 2 m by 2 m hole at (1 m, 1 m), edges `h0` to `h3`.
/// 44 m2 of floor in all.
pub fn l_corners() -> Vec<Corner> {
    vec![
        at("e0", 0, 0),
        at("e1", 8000, 0),
        at("e2", 8000, 4000),
        at("e3", 4000, 4000),
        at("e4", 4000, 8000),
        at("e5", 0, 8000),
    ]
}

/// The hole in the L-shaped floor.
pub fn l_hole() -> Vec<Corner> {
    rectangle("h", 1000, 1000, 2000, 2000)
}

/// The L-shaped floor with its hole.
pub fn l_with_hole() -> Boundary {
    Boundary::from_corners(&l_corners(), &[l_hole()]).unwrap()
}
