//! The mass function and the stored form.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_materials::{
    Class, Drawing, Graphics, MaterialError, MaterialId, Materials, Measure, NewMaterial, Physical,
    RefName, mass_grams,
};
use construct_params::{Length, Value, Volume};
use construct_styles::{Colour, patterns::SOLID};

fn physical(density: u64) -> Physical {
    Physical {
        density_g_per_m3: Measure {
            value: density,
            source: None,
        },
        conductivity_mw_per_m_k: None,
        specific_heat_j_per_kg_k: None,
    }
}

#[test]
fn a_cubic_metre_of_steel_is_its_density() {
    let library = Materials::default_metric();
    let steel = MaterialId::parse("steel-aisi-1025").unwrap();
    // 7861 kg per cubic metre is 7,861,000 g.
    assert_eq!(
        library
            .mass_grams(&steel, Volume::from_mm3(1_000_000_000))
            .unwrap(),
        7_861_000
    );
    // A 100 x 100 x 1000 mm post: 0.01 m3, 78,610 g.
    assert_eq!(
        library
            .mass_grams(&steel, Volume::from_mm3(10_000_000))
            .unwrap(),
        78_610
    );
    assert_eq!(library.mass_grams(&steel, Volume::from_mm3(0)).unwrap(), 0);
}

#[test]
fn rounding_is_to_the_nearest_gram_a_half_to_the_even_gram() {
    let dense = physical(100_000_000);
    // 5 mm3 * 1e8 / 1e9 = 0.5 g -> 0 (even); 15 -> 1.5 -> 2; 25 -> 2.5 -> 2; 35 -> 3.5 -> 4.
    let grams = |mm3| mass_grams(Volume::from_mm3(mm3), &dense).unwrap();
    assert_eq!(grams(5), 0);
    assert_eq!(grams(15), 2);
    assert_eq!(grams(25), 2);
    assert_eq!(grams(35), 4);
    // Just under and just over a half.
    assert_eq!(grams(4), 0);
    assert_eq!(grams(6), 1);
    // Concrete, 1 mm3: 0.0023 g -> 0.
    let concrete = Materials::default_metric()
        .get(&MaterialId::parse("concrete-regular").unwrap())
        .unwrap()
        .physical
        .clone();
    assert_eq!(mass_grams(Volume::from_mm3(1), &concrete).unwrap(), 0);
}

#[test]
fn the_largest_inputs_do_not_overflow() {
    let dense = physical(100_000_000);
    let most = mass_grams(Volume::from_mm3(i64::MAX), &dense).unwrap();
    assert_eq!(most, 922_337_203_685_477_581);
}

/// The rule as it was written for whole cubic millimetres: `mm3 * density / 10^9`, to the nearest
/// gram, a half to the even gram.
fn grams_from_mm3(mm3: u128, density: u128) -> u64 {
    let product = mm3 * density;
    let (whole, rest) = (product / 1_000_000_000, product % 1_000_000_000);
    let up = rest * 2 > 1_000_000_000 || (rest * 2 == 1_000_000_000 && whole % 2 == 1);
    (whole + u128::from(up)) as u64
}

#[test]
fn a_volume_gives_the_same_mass_as_in_whole_cubic_millimetres() {
    for density in [1, 999, 2_300_000, 7_861_000, 99_999_999, 100_000_000] {
        for mm3 in [
            0_i64,
            1,
            4,
            5,
            6,
            15,
            25,
            35,
            1_953,
            10_000_000,
            1_000_000_000,
            123_456_789_012,
            i64::MAX / 3,
            i64::MAX,
        ] {
            assert_eq!(
                mass_grams(Volume::from_mm3(mm3), &physical(density)).unwrap(),
                grams_from_mm3(mm3 as u128, u128::from(density)),
                "{mm3} mm3 at {density} g/m3"
            );
        }
    }
}

#[test]
fn a_volume_between_whole_cubic_millimetres_has_its_exact_mass() {
    // A 12.5 mm cube of steel: 1,953.125 mm3 x 7,861,000 g/m3 = 15.35 g, so 15 g.
    let side = Value::Length(Length::from_steps(80_000));
    let cube = Volume::from_cubic_steps(80_000_i128.pow(3));
    assert_eq!(
        side.multiply(&side).and_then(|area| area.multiply(&side)),
        Ok(Value::Volume(cube))
    );
    assert_eq!(mass_grams(cube, &physical(7_861_000)).unwrap(), 15);
    // Half a gram exactly, between whole cubic millimetres: 5 mm3 and one cubic step more is
    // just over a half, so 1 g; 5 mm3 is exactly a half, so 0 g.
    let half = Volume::from_mm3(5);
    assert_eq!(mass_grams(half, &physical(100_000_000)).unwrap(), 0);
    let over = Volume::from_cubic_steps(half.cubic_steps() + 1);
    assert_eq!(mass_grams(over, &physical(100_000_000)).unwrap(), 1);
}

#[test]
fn a_mass_beyond_a_u64_of_grams_is_refused() {
    assert_eq!(
        mass_grams(Volume::from_cubic_steps(i128::MAX), &physical(100_000_000)).unwrap_err(),
        MaterialError::MassTooLarge
    );
}

#[test]
fn bad_inputs_are_refused() {
    assert_eq!(
        mass_grams(Volume::from_mm3(-1), &physical(1000)).unwrap_err(),
        MaterialError::NegativeVolume
    );
    assert!(mass_grams(Volume::from_mm3(1), &physical(0)).is_err());
    let library = Materials::default_metric();
    let nope = MaterialId::parse("nope").unwrap();
    assert_eq!(
        library.mass_grams(&nope, Volume::from_mm3(1)).unwrap_err(),
        MaterialError::NoSuchMaterial { id: nope }
    );
}

fn empty() -> Materials {
    Materials::from_bytes(br#"{"format":1,"sources":[],"materials":[]}"#).unwrap()
}

#[test]
fn the_stored_form_is_pinned() {
    let graphics = |fill: Option<&str>| Graphics {
        colour: Colour(1, 2, 3),
        pattern: SOLID,
        fill: fill.map(|name| RefName::new(name).unwrap()),
    };
    let (library, _) = empty()
        .with_material(
            MaterialId::parse("m1").unwrap(),
            NewMaterial {
                name: "Test".to_owned(),
                class: Class::AirGap,
                drawing: Drawing {
                    surface: graphics(None),
                    cut: graphics(Some("hatch")),
                },
                physical: physical(1205),
                look: Some(RefName::new("clear").unwrap()),
            },
        )
        .unwrap();
    let text = String::from_utf8(library.to_bytes().unwrap()).unwrap();
    assert_eq!(
        text,
        concat!(
            r#"{"format":1,"sources":[],"materials":[{"id":"m1","name":"Test","class":"air_gap","#,
            r#""built_in":false,"drawing":{"surface":{"colour":[1,2,3],"pattern":1,"fill":null},"#,
            r#""cut":{"colour":[1,2,3],"pattern":1,"fill":"hatch"}},"#,
            r#""physical":{"density_g_per_m3":{"value":1205,"source":null},"#,
            r#""conductivity_mw_per_m_k":null,"specific_heat_j_per_kg_k":null},"look":"clear"}]}"#
        )
    );
}

#[test]
fn the_default_library_round_trips_with_the_same_bytes() {
    let library = Materials::default_metric();
    let bytes = library.to_bytes().unwrap();
    assert_eq!(bytes, Materials::default_metric().to_bytes().unwrap());
    let read = Materials::from_bytes(&bytes).unwrap();
    assert_eq!(read, library);
    assert_eq!(read.to_bytes().unwrap(), bytes);
}

#[test]
fn order_of_adding_does_not_change_the_bytes() {
    let make = |name: &str| NewMaterial {
        name: name.to_owned(),
        class: Class::Other,
        drawing: Drawing {
            surface: Graphics {
                colour: Colour(0, 0, 0),
                pattern: SOLID,
                fill: None,
            },
            cut: Graphics {
                colour: Colour(0, 0, 0),
                pattern: SOLID,
                fill: None,
            },
        },
        physical: physical(10),
        look: None,
    };
    let base = Materials::default_metric();
    let one = base
        .with_material(MaterialId::parse("u-b").unwrap(), make("B"))
        .unwrap()
        .0
        .with_material(MaterialId::parse("u-a").unwrap(), make("A"))
        .unwrap()
        .0;
    let two = base
        .with_material(MaterialId::parse("u-a").unwrap(), make("A"))
        .unwrap()
        .0
        .with_material(MaterialId::parse("u-b").unwrap(), make("B"))
        .unwrap()
        .0;
    assert_eq!(one.to_bytes().unwrap(), two.to_bytes().unwrap());
}

#[test]
fn readers_refuse_other_spellings_and_broken_rules() {
    let good = String::from_utf8(Materials::default_metric().to_bytes().unwrap()).unwrap();
    // Extra space: valid data, another spelling.
    let spaced = good.replacen("\"format\":1", "\"format\": 1", 1);
    assert_eq!(
        Materials::from_bytes(spaced.as_bytes()).unwrap_err(),
        MaterialError::NotCanonical
    );
    // Another version.
    let other = good.replacen("\"format\":1", "\"format\":2", 1);
    assert_eq!(
        Materials::from_bytes(other.as_bytes()).unwrap_err(),
        MaterialError::Version { found: 2 }
    );
    // Not JSON, wrong shape, unknown field.
    assert!(matches!(
        Materials::from_bytes(b"nope"),
        Err(MaterialError::Format(_))
    ));
    let extra = good.replacen("\"format\":1", "\"format\":1,\"extra\":1", 1);
    assert!(matches!(
        Materials::from_bytes(extra.as_bytes()),
        Err(MaterialError::Format(_))
    ));
    // A duplicate name.
    let dup = good.replacen("\"Glass, plate\"", "\"concrete, REGULAR\"", 1);
    assert!(matches!(
        Materials::from_bytes(dup.as_bytes()),
        Err(MaterialError::DuplicateName { .. })
    ));
    // A source that is not held.
    let lost = good.replacen("\"source\":\"fpl-gtr-282\"", "\"source\":\"gone\"", 1);
    assert!(matches!(
        Materials::from_bytes(lost.as_bytes()),
        Err(MaterialError::UnknownSource { .. })
    ));
    // A density of zero.
    let zero = good.replacen("\"value\":2300000", "\"value\":0", 1);
    assert!(matches!(
        Materials::from_bytes(zero.as_bytes()),
        Err(MaterialError::BadValue { .. })
    ));
    // A bad id.
    let bad_id = good.replacen("\"id\":\"air-dry\"", "\"id\":\"air dry\"", 1);
    assert!(matches!(
        Materials::from_bytes(bad_id.as_bytes()),
        Err(MaterialError::Format(_))
    ));
}
