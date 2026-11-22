//! The default library: ids pinned, every physical value sourced, the classes it covers.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_materials::{Class, Materials};

/// The permanent IDs, in ascending order. Changing one is a break for every project: this test
/// is the pin.
const PINNED_IDS: [&str; 16] = [
    "air-dry",
    "aluminium-6061",
    "concrete-regular",
    "earth-average",
    "glass-plate",
    "insulation-foam-glass",
    "membrane-polyethylene",
    "plaster-gypsum",
    "steel-aisi-1025",
    "steel-stainless-aisi-301",
    "timber-douglas-fir",
    "timber-eastern-white-pine",
    "timber-loblolly-pine",
    "timber-sitka-spruce",
    "timber-western-redcedar",
    "timber-white-oak",
];

#[test]
fn ids_are_pinned() {
    let library = Materials::default_metric();
    let ids: Vec<&str> = library.materials().iter().map(|m| m.id.as_str()).collect();
    let pinned: Vec<&str> = PINNED_IDS.to_vec();
    assert_eq!(ids, pinned);
}

#[test]
fn every_default_physical_value_has_a_source_that_exists() {
    let library = Materials::default_metric();
    for material in library.materials() {
        assert!(
            material.physical.all_sourced(),
            "{} has a physical value with no source",
            material.name
        );
        let reference = material.physical.density_g_per_m3.source.as_ref().unwrap();
        assert!(!reference.at.is_empty(), "{} has no place", material.name);
        assert!(
            library.source(&reference.source).is_some(),
            "{} cites a missing source",
            material.name
        );
        assert!(material.built_in);
        assert!(material.look.is_none());
    }
}

#[test]
fn every_default_source_is_named_and_used() {
    let library = Materials::default_metric();
    for source in library.sources() {
        assert!(source.url.starts_with("https://"));
        assert!(
            library.materials().iter().any(|m| m
                .physical
                .density_g_per_m3
                .source
                .as_ref()
                .is_some_and(|r| r.source == source.key)),
            "{} is not cited",
            source.key
        );
    }
}

#[test]
fn workshop_values_are_kept_as_recorded() {
    let library = Materials::default_metric();
    let oak = library.by_name("White oak").unwrap();
    let reference = oak.physical.density_g_per_m3.source.as_ref().unwrap();
    assert_eq!(reference.source, "fpl-gtr-282");
    assert_eq!(reference.at, "Table 5-3a, page 5-6");
    assert_eq!(oak.physical.density_g_per_m3.value, 762_000);
    let steel = library.by_name("Carbon steel, AISI 1025").unwrap();
    let reference = steel.physical.density_g_per_m3.source.as_ref().unwrap();
    assert_eq!(reference.source, "mil-hdbk-5j");
    assert_eq!(reference.at, "Table 2.2.1.0(b), page 2-8");
    assert_eq!(steel.physical.density_g_per_m3.value, 7_861_000);
}

#[test]
fn the_default_set_covers_the_classes_it_has_sources_for() {
    let library = Materials::default_metric();
    for class in [
        Class::Concrete,
        Class::Timber,
        Class::Steel,
        Class::Metal,
        Class::Glass,
        Class::Insulation,
        Class::Plaster,
        Class::Membrane,
        Class::Earth,
        Class::AirGap,
    ] {
        assert!(
            library.materials().iter().any(|m| m.class == class),
            "no default {class:?}"
        );
    }
}

#[test]
fn default_names_are_unique_without_regard_to_case() {
    Materials::default_metric().check().unwrap();
}
