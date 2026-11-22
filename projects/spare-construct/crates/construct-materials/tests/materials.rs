//! Adding, duplicating, renaming and editing materials; deleting; refusals as values.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_materials::{
    Class, Drawing, Graphics, MaterialError, MaterialId, Materials, Measure, NewMaterial, Physical,
    RefName, SourceRef,
};
use construct_styles::{Colour, patterns::DASHED};

fn id(text: &str) -> MaterialId {
    MaterialId::parse(text).unwrap()
}

fn graphics(red: u8) -> Graphics {
    Graphics {
        colour: Colour(red, 1, 2),
        pattern: DASHED,
        fill: Some(RefName::new("hatch-a").unwrap()),
    }
}

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

fn new_material(name: &str) -> NewMaterial {
    NewMaterial {
        name: name.to_owned(),
        class: Class::Finish,
        drawing: Drawing {
            surface: graphics(10),
            cut: graphics(20),
        },
        physical: physical(1_500_000),
        look: Some(RefName::new("oak-satin").unwrap()),
    }
}

#[test]
fn add_returns_a_new_library_and_leaves_the_old_one() {
    let base = Materials::default_metric();
    let (next, made) = base
        .with_material(id("user-tile"), new_material("Floor tile"))
        .unwrap();
    assert_eq!(made, id("user-tile"));
    assert_eq!(next.materials().len(), base.materials().len() + 1);
    assert!(base.get(&made).is_none());
    let tile = next.get(&made).unwrap();
    assert!(!tile.built_in);
    assert_eq!(tile.class, Class::Finish);
    assert_eq!(tile.look.as_ref().unwrap().as_str(), "oak-satin");
}

#[test]
fn names_are_unique_without_regard_to_case_and_spaces() {
    let base = Materials::default_metric();
    let existing = base.by_name("concrete, REGULAR").unwrap().id.clone();
    for clash in ["CONCRETE, regular", "  concrete, regular  "] {
        assert_eq!(
            base.with_material(id("user-a"), new_material(clash))
                .unwrap_err(),
            MaterialError::DuplicateName {
                existing: existing.clone()
            }
        );
    }
    let (next, _) = base
        .with_material(id("user-a"), new_material("Plain"))
        .unwrap();
    assert_eq!(
        next.with_material(id("user-b"), new_material("pLAIN"))
            .unwrap_err(),
        MaterialError::DuplicateName {
            existing: id("user-a")
        }
    );
}

#[test]
fn bad_names_and_ids_are_refused() {
    let base = Materials::default_metric();
    assert_eq!(
        base.with_material(id("user-a"), new_material("   "))
            .unwrap_err(),
        MaterialError::EmptyName
    );
    assert_eq!(
        base.with_material(id("user-a"), new_material("tab\there"))
            .unwrap_err(),
        MaterialError::BadText { field: "name" }
    );
    assert_eq!(
        base.with_material(id("user-a"), new_material(&"x".repeat(101)))
            .unwrap_err(),
        MaterialError::BadText { field: "name" }
    );
    assert!(MaterialId::parse("").is_err());
    assert!(MaterialId::parse("has space").is_err());
}

#[test]
fn an_id_in_use_or_built_in_is_refused() {
    let base = Materials::default_metric();
    let built_in = base.materials().first().unwrap().id.clone();
    assert_eq!(
        base.with_material(built_in.clone(), new_material("Another"))
            .unwrap_err(),
        MaterialError::DuplicateId { id: built_in }
    );
    let (next, _) = base
        .with_material(id("user-a"), new_material("One"))
        .unwrap();
    assert_eq!(
        next.with_material(id("user-a"), new_material("Two"))
            .unwrap_err(),
        MaterialError::DuplicateId { id: id("user-a") }
    );
}

#[test]
fn duplicate_copies_everything_under_a_new_id_and_name() {
    let base = Materials::default_metric();
    let from = id("concrete-regular");
    let (next, copy) = base
        .duplicated(&from, id("user-concrete"), "Concrete, ours")
        .unwrap();
    let a = next.get(&from).unwrap();
    let b = next.get(&copy).unwrap();
    assert_ne!(a.id, b.id);
    assert_eq!(b.name, "Concrete, ours");
    assert!(a.built_in);
    assert!(!b.built_in);
    assert_eq!(a.class, b.class);
    assert_eq!(a.drawing, b.drawing);
    assert_eq!(a.physical, b.physical);
    assert_eq!(a.look, b.look);
    assert_eq!(
        base.duplicated(&id("nope"), id("user-x"), "X").unwrap_err(),
        MaterialError::NoSuchMaterial { id: id("nope") }
    );
    // The copy's name must still be free.
    assert!(matches!(
        base.duplicated(&from, id("user-y"), "concrete, regular"),
        Err(MaterialError::DuplicateName { .. })
    ));
}

#[test]
fn a_built_in_can_be_edited_and_its_id_never_changes() {
    let base = Materials::default_metric();
    let steel = id("steel-aisi-1025");
    let renamed = base.renamed(&steel, "Mild steel").unwrap();
    assert_eq!(renamed.get(&steel).unwrap().name, "Mild steel");
    assert!(renamed.get(&steel).unwrap().built_in);
    assert!(renamed.by_name("carbon steel, aisi 1025").is_none());
    assert_eq!(renamed.by_name("MILD STEEL").unwrap().id, steel);
    // Renaming to its own name in other case is allowed.
    base.renamed(&steel, "CARBON STEEL, AISI 1025").unwrap();
    // Renaming onto another material's name is not.
    assert!(matches!(
        base.renamed(&steel, "glass, plate"),
        Err(MaterialError::DuplicateName { .. })
    ));
    assert_eq!(
        base.renamed(&id("nope"), "X").unwrap_err(),
        MaterialError::NoSuchMaterial { id: id("nope") }
    );
}

#[test]
fn the_three_groups_change_apart() {
    let base = Materials::default_metric();
    let steel = id("steel-aisi-1025");
    let before = base.get(&steel).unwrap().clone();

    let drawn = base
        .with_drawing(
            &steel,
            Drawing {
                surface: graphics(99),
                cut: graphics(98),
            },
        )
        .unwrap();
    let after = drawn.get(&steel).unwrap();
    assert_eq!(after.drawing.surface.colour, Colour(99, 1, 2));
    assert_eq!(after.drawing.cut.colour, Colour(98, 1, 2));
    assert_eq!(after.physical, before.physical);
    assert_eq!(after.look, before.look);

    let phys = base.with_physical(&steel, physical(7_000_000)).unwrap();
    let after = phys.get(&steel).unwrap();
    assert_eq!(after.physical.density_g_per_m3.value, 7_000_000);
    assert_eq!(after.drawing, before.drawing);
    assert_eq!(after.look, before.look);

    let looked = base
        .with_look(&steel, Some(RefName::new("brushed-steel").unwrap()))
        .unwrap();
    let after = looked.get(&steel).unwrap();
    assert_eq!(after.look.as_ref().unwrap().as_str(), "brushed-steel");
    assert_eq!(after.drawing, before.drawing);
    assert_eq!(after.physical, before.physical);
    assert!(
        looked
            .with_look(&steel, None)
            .unwrap()
            .get(&steel)
            .unwrap()
            .look
            .is_none()
    );

    let classed = base.with_class(&steel, Class::Metal).unwrap();
    assert_eq!(classed.get(&steel).unwrap().class, Class::Metal);
}

#[test]
fn physical_values_are_range_checked_and_sources_must_exist() {
    let base = Materials::default_metric();
    let steel = id("steel-aisi-1025");
    assert_eq!(
        base.with_physical(&steel, physical(0)).unwrap_err(),
        MaterialError::BadValue {
            property: "density",
            value: 0
        }
    );
    assert!(base.with_physical(&steel, physical(100_000_001)).is_err());
    let mut wide = physical(1000);
    wide.conductivity_mw_per_m_k = Some(Measure {
        value: 1_000_001,
        source: None,
    });
    assert_eq!(
        base.with_physical(&steel, wide).unwrap_err(),
        MaterialError::BadValue {
            property: "conductivity",
            value: 1_000_001
        }
    );
    let mut cited = physical(1000);
    cited.specific_heat_j_per_kg_k = Some(Measure {
        value: 500,
        source: Some(SourceRef {
            source: "no-such-source".to_owned(),
            at: "page 1".to_owned(),
        }),
    });
    assert_eq!(
        base.with_physical(&steel, cited.clone()).unwrap_err(),
        MaterialError::UnknownSource {
            key: "no-such-source".to_owned()
        }
    );
    cited.specific_heat_j_per_kg_k.as_mut().unwrap().source = Some(SourceRef {
        source: "mil-hdbk-5j".to_owned(),
        at: "page 1".to_owned(),
    });
    let ok = base.with_physical(&steel, cited).unwrap();
    assert_eq!(
        ok.get(&steel)
            .unwrap()
            .physical
            .specific_heat_j_per_kg_k
            .as_ref()
            .unwrap()
            .value,
        500
    );
}

#[test]
fn delete_is_refused_when_in_use_and_for_a_built_in() {
    let base = Materials::default_metric();
    let (next, own) = base
        .with_material(id("user-a"), new_material("One"))
        .unwrap();
    let users = vec!["wall type W1".to_owned(), "floor type F2".to_owned()];
    assert_eq!(
        next.without(&own, &users).unwrap_err(),
        MaterialError::InUse {
            id: own.clone(),
            users
        }
    );
    assert!(next.get(&own).is_some());
    let gone = next.without(&own, &[]).unwrap();
    assert!(gone.get(&own).is_none());
    assert_eq!(gone, base);
    assert_eq!(
        base.without(&id("steel-aisi-1025"), &[]).unwrap_err(),
        MaterialError::BuiltInNotDeletable {
            id: id("steel-aisi-1025")
        }
    );
    assert_eq!(
        base.without(&id("nope"), &[]).unwrap_err(),
        MaterialError::NoSuchMaterial { id: id("nope") }
    );
}

#[test]
fn a_source_can_be_added_once() {
    let base = Materials::default_metric();
    let source = construct_materials::Source {
        key: "my-handbook".to_owned(),
        title: "A handbook".to_owned(),
        by: "A publisher".to_owned(),
        date: "2020".to_owned(),
        status: "Open".to_owned(),
        url: "https://example.org/h".to_owned(),
    };
    let next = base.with_source(source.clone()).unwrap();
    assert!(next.source("my-handbook").is_some());
    assert_eq!(
        next.with_source(source.clone()).unwrap_err(),
        MaterialError::DuplicateSource {
            key: "my-handbook".to_owned()
        }
    );
    let mut bad = source;
    bad.key = "Bad Key".to_owned();
    assert!(matches!(
        base.with_source(bad),
        Err(MaterialError::BadSource { .. })
    ));
}
