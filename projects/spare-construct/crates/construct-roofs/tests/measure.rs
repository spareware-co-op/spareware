//! What a roof reads back: its areas, thickness, pitch, a gable's ridge height, volume and mass,
//! each layer's volume and mass, and the same as read-only parameters.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{degrees, gable, mono, rafters, slope};
use construct_materials::{MaterialId, Materials};
use construct_params::{Angle, Area, Kind, Length, Slope, Value, Volume};
use construct_roofs::{
    AREA, MASS, PITCH, RIDGE_HEIGHT, Roof, SLOPE_AREA, THICKNESS, VOLUME, default_roofs,
    layer_read_only, quantities, read_only, shape,
};

/// The 1 in 4 gable of 150 mm rafters reads back its shape's figures: 33 m2 in plan, 34.0 m2 as
/// laid, 150 mm thick, its pitch as given, its ridge 654.6 mm up, and the rafters' volume and
/// mass as solid Douglas-fir; a 30 degree pitch reads back as the angle it was given.
#[test]
fn a_gable_reads_back_its_shape() {
    let library = Materials::default_metric();
    let roof = gable(slope(1, 4));
    let q = quantities(&roof, &rafters(), &library).unwrap();
    let built = shape(&roof, &rafters()).unwrap();
    assert_eq!(q.area, Area::from_mm2(33_000_000));
    assert_eq!(q.slope_area, built.slope_area);
    assert_eq!(q.thickness, Length::from_mm(150));
    assert_eq!(q.pitch, Some(Value::Slope(Slope::new(1, 4).unwrap())));
    assert_eq!(q.ridge_height, Some(Length::from_steps(4_189_545)));
    assert_eq!(q.volume, built.solids[0].volume);
    let timber = MaterialId::parse("timber-douglas-fir").unwrap();
    let grams = library.mass_grams(&timber, q.volume).unwrap();
    assert_eq!(q.mass_grams, Some(grams));
    assert_eq!(q.layers[0].mass_grams, Some(grams));
    let at_thirty = quantities(&gable(degrees(30)), &rafters(), &library).unwrap();
    assert_eq!(
        at_thirty.pitch,
        Some(Value::Angle(Angle::from_units(270_000_000)))
    );
    let lean_to = quantities(&mono(), &rafters(), &library).unwrap();
    assert_eq!(lean_to.ridge_height, None);
    assert!(lean_to.pitch.is_some());
    let flat = Roof::new(common::rectangle(), &common::edges(&[])).unwrap();
    let flat = quantities(&flat, &rafters(), &library).unwrap();
    assert_eq!(flat.pitch, None);
    assert_eq!(flat.area, flat.slope_area);
    assert_eq!(flat.volume, Volume::from_mm3(6000 * 4000 * 150));
}

/// The read-only parameters in order, a sloped roof's pitch and a gable's ridge after the five;
/// each layer's volume and mass after, named with its function and material; a membrane's volume
/// is none, and a layer of a material the library lacks has no mass, nor then has the roof.
#[test]
fn the_read_only_parameters() {
    let library = Materials::default_metric();
    let steel = default_roofs().unwrap().remove(0).layers;
    let q = quantities(&gable(slope(1, 4)), &steel, &library).unwrap();
    let rows = read_only(&q).unwrap();
    let ids: Vec<String> = rows.iter().map(|(d, _)| d.id().to_string()).collect();
    assert_eq!(
        ids,
        [
            AREA,
            SLOPE_AREA,
            THICKNESS,
            VOLUME,
            MASS,
            PITCH,
            RIDGE_HEIGHT
        ]
    );
    assert_eq!(rows[5].0.kind(), Kind::Slope);
    assert!(rows[4].1.is_some(), "every material is in the library");
    let layers = layer_read_only(&steel, &q).unwrap();
    let names: Vec<&str> = layers.iter().map(|(d, _)| d.name()).collect();
    assert_eq!(
        names,
        [
            "Layer 1 (finish, steel-aisi-1025) volume",
            "Layer 1 (finish, steel-aisi-1025) mass (kg)",
            "Layer 2 (membrane, membrane-polyethylene) volume",
            "Layer 2 (membrane, membrane-polyethylene) mass (kg)",
            "Layer 3 (structure, timber-douglas-fir) volume",
            "Layer 3 (structure, timber-douglas-fir) mass (kg)",
        ]
    );
    assert_eq!(
        layers[2].1,
        Some(Value::Volume(Volume::from_cubic_steps(0)))
    );
    assert_eq!(layers[0].0.id().as_str(), "roof.layer-1.volume");
    let flat = Roof::new(common::rectangle(), &common::edges(&[])).unwrap();
    let odd = construct_slabs::Layers::new(vec![common::layer(
        construct_slabs::Function::Structure,
        150,
        "no-such-material",
    )])
    .unwrap();
    let q = quantities(&flat, &odd, &library).unwrap();
    assert_eq!(q.mass_grams, None);
    let rows = read_only(&q).unwrap();
    assert_eq!(rows.len(), 5, "a flat roof has no pitch and no ridge");
    assert_eq!(rows[4].1, None);
    assert!(
        layer_read_only(&steel, &q).is_err(),
        "other layers' quantities"
    );
}
