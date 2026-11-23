//! What the tests share: lengths, slopes, footprints, edges and the roofs the tests draw.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; each test file uses part of this fixture"
)]

use construct_materials::MaterialId;
use construct_params::{Angle, Length, Slope};
use construct_roofs::{EdgeSpec, Pitch, Roof};
use construct_slabs::{Boundary, Corner, FloorLayer, Function, Layers};
use construct_solver::Point;
use kit_core::Id;
use std::collections::BTreeMap;

/// Grid steps in a millimetre.
pub const MM: i64 = 6400;

/// Text as an ID.
pub fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// Whole millimetres as a length.
pub fn mm(mm: i64) -> Length {
    Length::checked_from_mm(mm).unwrap()
}

/// A slope of `rise` in `run`.
pub fn slope(rise: i64, run: i64) -> Pitch {
    Pitch::Slope(Slope::new(rise, run).unwrap())
}

/// An angle of `degrees` whole degrees.
pub fn degrees(degrees: i64) -> Pitch {
    Pitch::Angle(Angle::from_units(degrees * Angle::UNITS_PER_DEGREE))
}

/// A corner at whole millimetres, with no edge ID of its own.
pub fn at(x: i64, y: i64) -> Corner {
    Corner {
        id: None,
        at: Point {
            x: x * MM,
            y: y * MM,
        },
    }
}

/// The 6 m by 4 m rectangle anticlockwise from the origin: `o0` along x at y = 0, `o1` up the
/// right side, `o2` back along y = 4 m, `o3` down the left side.
pub fn rectangle() -> Boundary {
    Boundary::from_corners(&[at(0, 0), at(6000, 0), at(6000, 4000), at(0, 4000)], &[]).unwrap()
}

/// The L-shaped footprint of construct-slabs' tests: 8 m along x by 4 m, and 4 m by 4 m more
/// above its left half, its edges `o0` to `o5`.
pub fn l_shape() -> Boundary {
    Boundary::from_corners(
        &[
            at(0, 0),
            at(8000, 0),
            at(8000, 4000),
            at(4000, 4000),
            at(4000, 8000),
            at(0, 8000),
        ],
        &[],
    )
    .unwrap()
}

/// What each edge does, by ID: `(edge, overhang in mm, pitch)`.
pub fn edges(list: &[(&str, i64, Option<Pitch>)]) -> BTreeMap<Id, EdgeSpec> {
    list.iter()
        .map(|&(edge, overhang, pitch)| {
            (
                id(edge),
                EdgeSpec {
                    overhang: mm(overhang),
                    pitch,
                },
            )
        })
        .collect()
}

/// The 6 m by 4 m gable at `pitch` on `o0` and `o2`: 500 mm eaves overhangs, 300 mm verges.
pub fn gable(pitch: Pitch) -> Roof {
    Roof::new(
        rectangle(),
        &edges(&[
            ("o0", 500, Some(pitch)),
            ("o1", 300, None),
            ("o2", 500, Some(pitch)),
            ("o3", 300, None),
        ]),
    )
    .unwrap()
}

/// The 6 m by 4 m mono-pitch roof rising at 1 in 4 from `o0`: 500 mm over its eaves, 300 mm over
/// its high side and its verges.
pub fn mono() -> Roof {
    Roof::new(
        rectangle(),
        &edges(&[
            ("o0", 500, Some(slope(1, 4))),
            ("o1", 300, None),
            ("o2", 300, None),
            ("o3", 300, None),
        ]),
    )
    .unwrap()
}

/// A layer of `material`, `thickness` whole millimetres thick.
pub fn layer(function: Function, thickness: i64, material: &str) -> FloorLayer {
    FloorLayer::new(
        function,
        mm(thickness),
        MaterialId::parse(material).unwrap(),
    )
}

/// One 150 mm layer of rafters.
pub fn rafters() -> Layers {
    Layers::new(vec![layer(Function::Structure, 150, "timber-douglas-fir")]).unwrap()
}
