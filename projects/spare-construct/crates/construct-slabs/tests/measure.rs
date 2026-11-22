//! What a floor reads back from its boundary and its type: area and perimeter exactly from the
//! corners, holes taken off; volume per layer; mass from construct-materials; and the read-only
//! parameters a schedule lists.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{bare, l_with_hole, layer, mm, slab};
use construct_materials::{MaterialId, Materials};
use construct_params::{Area, Decimal, Kind, Length, Value, Volume};
use construct_slabs::{
    AREA, Boundary, Corner, Function, Layers, MASS, PERIMETER, SlabError, THICKNESS, VOLUME, area,
    perimeter, quantities, read_only,
};
use construct_solver::Point;

/// The L-shaped floor is 48 m2 less its 4 m2 hole, and its edges add up to 40 m, the hole's
/// included; both exact, from the corners in grid steps.
#[test]
fn area_and_perimeter_of_the_l_with_its_hole() {
    let floor = l_with_hole();
    assert_eq!(area(&floor).unwrap(), Area::from_mm2(44_000_000));
    assert_eq!(perimeter(&floor).unwrap(), mm(40_000));
    let no_hole = Boundary::from_corners(&common::l_corners(), &[]).unwrap();
    assert_eq!(area(&no_hole).unwrap(), Area::from_mm2(48_000_000));
    assert_eq!(perimeter(&no_hole).unwrap(), mm(32_000));
}

/// A sloping edge's length is the nearest whole grid step to its true length, so a 3-4-5
/// triangle is exact and a step-sized right triangle's long side is one step; an area that is
/// not a whole number of square steps is refused, never rounded.
#[test]
fn sloping_edges_and_an_area_off_the_grid() {
    let three_four_five =
        Boundary::from_corners(&[bare(0, 0), bare(3, 0), bare(0, 4)], &[]).unwrap();
    assert_eq!(perimeter(&three_four_five).unwrap(), mm(12));
    assert_eq!(area(&three_four_five).unwrap(), Area::from_mm2(6));
    let step = |x: i64, y: i64| Corner {
        id: None,
        at: Point { x, y },
    };
    let tiny = Boundary::from_corners(&[step(0, 0), step(1, 0), step(0, 1)], &[]).unwrap();
    assert_eq!(perimeter(&tiny).unwrap(), Length::from_steps(3));
    assert_eq!(area(&tiny), Err(SlabError::AreaNotExact));
    let two = Boundary::from_corners(&[step(0, 0), step(2, 0), step(0, 1)], &[]).unwrap();
    assert_eq!(area(&two).unwrap(), Area::from_square_steps(1));
    // sqrt(5) is 2.24 steps: 2; sqrt(8) is 2.83: 3.
    assert_eq!(perimeter(&two).unwrap(), Length::from_steps(2 + 1 + 2));
    let wide = Boundary::from_corners(&[step(0, 0), step(2, 0), step(0, 2)], &[]).unwrap();
    assert_eq!(perimeter(&wide).unwrap(), Length::from_steps(2 + 3 + 2));
}

/// Volume per layer is the area times its thickness, and the floor's the sum; mass is each
/// layer's volume of its material, through construct-materials' one function, and the floor's
/// is known only when every layer's material is in the library.
#[test]
fn volume_and_mass_per_layer() {
    let library = Materials::default_metric();
    let floor = l_with_hole();
    let q = quantities(&floor, &slab(), &library).unwrap();
    assert_eq!(q.area, Area::from_mm2(44_000_000));
    assert_eq!(q.perimeter, mm(40_000));
    assert_eq!(q.thickness, mm(200));
    assert_eq!(q.volume, Volume::from_mm3(8_800_000_000));
    assert_eq!(q.layers.len(), 2);
    assert_eq!(q.layers[0].volume, Volume::from_mm3(2_200_000_000));
    assert_eq!(q.layers[1].volume, Volume::from_mm3(6_600_000_000));
    // No screed in the default library yet, so the floor's mass is not known.
    assert_eq!(q.layers[0].mass_grams, None);
    let concrete = MaterialId::parse("concrete-regular").unwrap();
    let expected = library
        .mass_grams(&concrete, Volume::from_mm3(6_600_000_000))
        .unwrap();
    assert_eq!(q.layers[1].mass_grams, Some(expected));
    assert_eq!(q.mass_grams, None);

    let concrete_only =
        Layers::new(vec![layer(Function::Structure, 200, "concrete-regular")]).unwrap();
    let q = quantities(&floor, &concrete_only, &library).unwrap();
    let whole = library
        .mass_grams(&concrete, Volume::from_mm3(8_800_000_000))
        .unwrap();
    assert_eq!(q.mass_grams, Some(whole));
}

/// The read-only parameters: area, perimeter, thickness and volume in their own kinds, mass as a
/// number of kilograms (construct-params has no mass kind yet), each with a fixed ID, held by the
/// instance and never stored.
#[test]
fn the_read_only_parameters() {
    let library = Materials::default_metric();
    let concrete_only =
        Layers::new(vec![layer(Function::Structure, 200, "concrete-regular")]).unwrap();
    let q = quantities(&l_with_hole(), &concrete_only, &library).unwrap();
    let rows = read_only(&q).unwrap();
    let ids: Vec<String> = rows.iter().map(|(d, _)| d.id().to_string()).collect();
    assert_eq!(ids, [AREA, PERIMETER, THICKNESS, VOLUME, MASS]);
    let kinds: Vec<Kind> = rows.iter().map(|(d, _)| d.kind()).collect();
    assert_eq!(
        kinds,
        [
            Kind::Area,
            Kind::Length,
            Kind::Length,
            Kind::Volume,
            Kind::Number
        ]
    );
    assert_eq!(rows[0].1, Some(Value::Area(Area::from_mm2(44_000_000))));
    assert_eq!(rows[2].1, Some(Value::Length(mm(200))));
    let grams = q.mass_grams.unwrap();
    assert_eq!(
        rows[4].1,
        Some(Value::number(Decimal::new(
            i64::try_from(grams).unwrap(),
            3
        )))
    );
    let names: Vec<&str> = rows.iter().map(|(d, _)| d.name()).collect();
    assert_eq!(
        names,
        ["Area", "Perimeter", "Thickness", "Volume", "Mass (kg)"]
    );
    // With a material the library lacks, the mass has no value.
    let rows = read_only(&quantities(&l_with_hole(), &slab(), &library).unwrap()).unwrap();
    assert_eq!(rows[4].1, None);
}
