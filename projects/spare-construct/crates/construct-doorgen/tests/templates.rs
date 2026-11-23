//! Each template filled from a spec, with exact numbers: the quantities its solids measure, the
//! opening its wall is cut by, its tags as a schedule reads them from the construct-types type,
//! a clear size made into the overall size whose clear opening it is, and the generator writing
//! no geometry of its own.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{Fixture, make, mm, mm2, overall};
use construct_doorgen::{Hand, Quantities, Size, Spec, Template};
use construct_families::{Key, family_type, parameters};
use construct_params::{Length, Value};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn corners(outline: &[[Length; 2]]) -> Vec<(i64, i64)> {
    outline
        .iter()
        .map(|p| (p[0].whole_mm().unwrap(), p[1].whole_mm().unwrap()))
        .collect()
}

#[test]
fn a_900_by_2100_door_measures_exactly() {
    let generated = make(&[Spec::new(
        "D01",
        Template::SingleSwingDoor,
        overall(mm(900), mm(2100)),
    )])
    .unwrap();
    let ty = &generated.types[0];
    assert_eq!(ty.key.as_str(), "d01");
    assert_eq!(
        ty.quantities,
        Quantities {
            width: mm(900),
            height: mm(2100),
            // 900 less two 40 mm jambs less the 40 mm leaf standing open in the opening.
            clear_width: mm(780),
            clear_height: mm(2060),
            leaf_area: Some(mm2(820 * 2060)),
            glass_area: None,
            depth: mm(100),
        }
    );
    assert_eq!(
        corners(&ty.opening.outline),
        vec![(-450, 0), (450, 0), (450, 2100), (-450, 2100)]
    );
    // The default hand is the template's: hinged on the left.
    assert_eq!(ty.values.get("swing"), Some("choice:left"));
}

#[test]
fn a_1200_square_fixed_light_measures_exactly() {
    let generated = make(&[Spec::new(
        "W01",
        Template::FixedWindow,
        overall(mm(1200), mm(1200)),
    )])
    .unwrap();
    let q = generated.types[0].quantities;
    assert_eq!(q.clear_width, mm(1100));
    assert_eq!(q.clear_height, mm(1100));
    assert_eq!(q.glass_area, Some(mm2(1100 * 1100)));
    assert_eq!(q.leaf_area, None);
    assert_eq!(q.depth, mm(70));
    // The opening stands on the default 900 mm sill.
    assert_eq!(
        corners(&generated.types[0].opening.outline),
        vec![(-600, 900), (600, 900), (600, 2100), (-600, 2100)]
    );
}

#[test]
fn an_1800_by_1200_sliding_window_measures_exactly() {
    let mut spec = Spec::new("S01", Template::SlidingWindow, overall(mm(1800), mm(1200)));
    spec.sill_height = Some(mm(1000));
    let generated = make(&[spec]).unwrap();
    let ty = &generated.types[0];
    let q = ty.quantities;
    // Each sash spans (1800 - 100 + 50) / 2 = 875; slid across, the sliding one leaves 825 open.
    assert_eq!(q.clear_width, mm(825));
    assert_eq!(q.clear_height, mm(1100));
    assert_eq!(q.leaf_area, Some(mm2(2 * 875 * 1100)));
    // Each glass is 875 - 100 wide and 1200 - 100 - 100 high.
    assert_eq!(q.glass_area, Some(mm2(2 * 775 * 1000)));
    assert_eq!(q.depth, mm(100));
    assert_eq!(ty.values.get("sill_height"), Some("length:q0:6400000"));
    assert_eq!(
        corners(&ty.opening.outline),
        vec![(-900, 1000), (900, 1000), (900, 2200), (-900, 2200)]
    );
}

#[test]
fn a_clear_size_gives_the_overall_size_whose_clear_opening_it_is() {
    let cases = [
        // 800 clear + two 40 frames + the 40 leaf; 2000 + the 40 head.
        (Template::SingleSwingDoor, (800, 2000), (920, 2040)),
        (Template::FixedWindow, (1000, 500), (1100, 600)),
        // Twice 700, two 50 frames and one 50 sash.
        (Template::SlidingWindow, (700, 1000), (1550, 1100)),
    ];
    for (template, (cw, ch), (w, h)) in cases {
        let spec = Spec::new(
            "C1",
            template,
            Size::Clear {
                width: mm(cw),
                height: mm(ch),
            },
        );
        let q = make(&[spec]).unwrap().types[0].quantities;
        assert_eq!((q.width, q.height), (mm(w), mm(h)), "{template:?}");
        assert_eq!(
            (q.clear_width, q.clear_height),
            (mm(cw), mm(ch)),
            "{template:?}"
        );
    }
}

#[test]
fn parts_materials_and_hand_from_the_spec_are_the_types() {
    let mut spec = Spec::new(
        "D-7",
        Template::SingleSwingDoor,
        overall(mm(1000), mm(2400)),
    );
    spec.frame = Some(mm(50));
    spec.frame_depth = Some(mm(120));
    spec.leaf = Some(mm(45));
    spec.hand = Some(Hand::Right);
    spec.frame_material = Some(id("aluminium-6061"));
    spec.leaf_material = Some(id("timber-douglas-fir"));
    let generated = make(&[spec]).unwrap();
    let ty = &generated.types[0];
    assert_eq!(ty.key.as_str(), "d_7");
    assert_eq!(ty.values.get("frame"), Some("length:q0:320000"));
    assert_eq!(
        ty.values.get("leaf_material"),
        Some("material:timber-douglas-fir")
    );
    assert_eq!(ty.values.get("swing"), Some("choice:right"));
    let q = ty.quantities;
    assert_eq!(q.clear_width, mm(1000 - 100 - 45));
    assert_eq!(q.clear_height, mm(2350));
    assert_eq!(q.leaf_area, Some(mm2(900 * 2350)));
    assert_eq!(q.depth, mm(120));
}

#[test]
fn the_tags_a_schedule_reads_from_the_type_are_the_measured_values() {
    let fixture = Fixture::new();
    let specs = [
        Spec::new("W01", Template::SlidingWindow, overall(mm(1800), mm(1200))),
        Spec::new("W02", Template::SlidingWindow, overall(mm(2400), mm(900))),
    ];
    let generated = make(&specs).unwrap();
    let recipe = &generated.recipe;
    let params = parameters(recipe).unwrap();
    for (n, ty) in generated.types.iter().enumerate() {
        let made = family_type(
            recipe,
            &fixture.library(),
            &ty.key,
            id(&format!("T-{n}")),
            &params,
        )
        .unwrap();
        let read = |param: &str| made.value(&id(param)).cloned();
        assert_eq!(
            read("library.doors-windows.type_mark"),
            Some(Value::Text(ty.mark.clone()))
        );
        let q = ty.quantities;
        assert_eq!(
            read("library.doors-windows.clear_width"),
            Some(Value::Length(q.clear_width))
        );
        assert_eq!(
            read("library.doors-windows.clear_height"),
            Some(Value::Length(q.clear_height))
        );
        assert_eq!(
            read("library.doors-windows.leaf_area"),
            Some(Value::Area(q.leaf_area.unwrap()))
        );
        assert_eq!(
            read("library.doors-windows.glass_area"),
            Some(Value::Area(q.glass_area.unwrap()))
        );
        assert_eq!(
            read("library.doors-windows.sliding-window.width"),
            Some(Value::Length(q.width))
        );
        assert_eq!(
            made.name(),
            format!("{}: {} x {}", ty.mark, q.width, q.height)
        );
    }
    // The second: 2400 by 900, sashes spanning 1175.
    let q = generated.types[1].quantities;
    assert_eq!(q.clear_width, mm(1125));
    assert_eq!(q.glass_area, Some(mm2(2 * 1075 * 700)));
}

#[test]
fn the_generator_writes_no_geometry_of_its_own() {
    for template in Template::ALL {
        let original = template.recipe().unwrap();
        let generated = make(&[Spec::new("G1", template, overall(mm(1200), mm(2100)))]).unwrap();
        let recipe = &generated.recipe;
        assert_eq!(recipe.sketches, original.sketches, "{template:?}");
        assert_eq!(recipe.steps, original.steps, "{template:?}");
        assert_eq!(recipe.symbols, original.symbols, "{template:?}");
        assert_eq!(recipe.hosting, original.hosting, "{template:?}");
        assert_eq!(recipe.planes, original.planes, "{template:?}");
        assert_eq!(recipe.category, original.category, "{template:?}");
        // The template's parameters, unchanged, then the tags.
        assert_eq!(
            recipe.parameters[..original.parameters.len()],
            original.parameters[..],
            "{template:?}"
        );
        let tags: Vec<&str> = recipe.parameters[original.parameters.len()..]
            .iter()
            .map(|p| p.key.as_str())
            .collect();
        let want: &[&str] = match template {
            Template::SingleSwingDoor => &["type_mark", "clear_width", "clear_height", "leaf_area"],
            Template::FixedWindow => &["type_mark", "clear_width", "clear_height", "glass_area"],
            Template::SlidingWindow => &[
                "type_mark",
                "clear_width",
                "clear_height",
                "leaf_area",
                "glass_area",
            ],
        };
        assert_eq!(tags, want, "{template:?}");
        assert_eq!(recipe.types, vec![]);
        let table = recipe.type_table.as_ref().unwrap();
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].key, Key::parse("g1").unwrap());
    }
}
