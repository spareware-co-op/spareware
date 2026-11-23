//! A test for each refusal: every rule a spec or a family of specs can break is refused by its
//! name, about the field it names, with the spec's mark; and a stored recipe changed after it was
//! generated is refused by `verify` for what its solids, opening and tags now show. (The rules
//! `template` and `round-trip` guard this crate's own build: every template reads and every
//! generated recipe reads back, which the other tests hold. The rules `symbol` and `quantity`
//! are judged on what the evaluator draws and builds, which no spec inside the rules can make
//! wrong; the crate's own unit tests in `checks` refuse them on made-up solids and symbols.)

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{Fixture, family, make, mm, overall};
use construct_doorgen::{Hand, Refusal, Rule, Size, Spec, Template, generate, mark_key, verify};
use construct_families::Key;
use construct_params::Length;
use kit_core::Id;

fn door(mark: &str) -> Spec {
    Spec::new(mark, Template::SingleSwingDoor, overall(mm(900), mm(2100)))
}

fn fixed(mark: &str) -> Spec {
    Spec::new(mark, Template::FixedWindow, overall(mm(1200), mm(1200)))
}

fn sliding(mark: &str) -> Spec {
    Spec::new(mark, Template::SlidingWindow, overall(mm(1800), mm(1200)))
}

fn refused(specs: &[Spec]) -> Refusal {
    make(specs).unwrap_err()
}

fn is(refusal: &Refusal, rule: Rule, what: &str) {
    assert_eq!(
        (refusal.rule, refusal.what.as_str()),
        (rule, what),
        "{refusal}"
    );
    assert_ne!(refusal.reason, "");
    assert!(refusal.to_string().contains(rule.name()), "{refusal}");
}

#[test]
fn every_rule_has_its_own_name() {
    let mut names: Vec<&str> = Rule::ALL.iter().map(|r| r.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Rule::ALL.len());
}

#[test]
fn no_specs_and_a_spec_of_another_template_are_refused() {
    let fixture = Fixture::new();
    let r = generate(&family(Template::FixedWindow), &[], &fixture.library()).unwrap_err();
    is(&r, Rule::NoSpecs, "specs");
    let r = generate(
        &family(Template::FixedWindow),
        &[fixed("W1"), door("D1")],
        &fixture.library(),
    )
    .unwrap_err();
    is(&r, Rule::OtherTemplate, "template");
    assert_eq!(r.mark.as_deref(), Some("D1"));
}

#[test]
fn a_mark_that_makes_no_key_and_two_marks_of_one_key_are_refused() {
    for bad in ["", "1D", "D/1", "Dé", &"D".repeat(65)] {
        let r = refused(&[door(bad)]);
        is(&r, Rule::Mark, "mark");
        assert_eq!(r.mark.as_deref(), Some(bad));
    }
    assert_eq!(mark_key("W-12 a").unwrap(), Key::parse("w_12_a").unwrap());
    let r = refused(&[door("D-1"), door("d 1")]);
    is(&r, Rule::DuplicateMark, "mark");
    assert_eq!(r.mark.as_deref(), Some("d 1"));
}

#[test]
fn a_field_the_template_does_not_have_is_refused_by_its_name() {
    let mut glass_door = door("D1");
    glass_door.glass = Some(mm(6));
    is(&refused(&[glass_door]), Rule::NotInTemplate, "glass");
    let mut sill_door = door("D1");
    sill_door.sill_height = Some(mm(100));
    is(&refused(&[sill_door]), Rule::NotInTemplate, "sill_height");
    let mut leaf_window = fixed("W1");
    leaf_window.leaf = Some(mm(40));
    is(&refused(&[leaf_window]), Rule::NotInTemplate, "leaf");
    let mut sash_door = door("D1");
    sash_door.sash = Some(mm(40));
    is(&refused(&[sash_door]), Rule::NotInTemplate, "sash");
    let mut leaf_material = fixed("W1");
    leaf_material.leaf_material = Some(Id::parse("timber-white-oak").unwrap());
    is(
        &refused(&[leaf_material]),
        Rule::NotInTemplate,
        "leaf_material",
    );
    let mut handed = fixed("W1");
    handed.hand = Some(Hand::Left);
    is(&refused(&[handed]), Rule::NotInTemplate, "hand");
}

#[test]
fn a_hand_the_template_does_not_draw_is_refused() {
    let mut left = sliding("S1");
    left.hand = Some(Hand::Left);
    let r = refused(&[left]);
    is(&r, Rule::Hand, "hand");
    assert!(r.reason.contains("mirrored"), "{r}");
    let mut right = sliding("S1");
    right.hand = Some(Hand::Right);
    assert!(make(&[right]).is_ok());
}

#[test]
fn nothing_is_zero_or_less() {
    let zero = Length::from_steps(0);
    let mut frame = door("D1");
    frame.frame = Some(zero);
    is(&refused(&[frame]), Rule::Positive, "frame");
    let mut gap = sliding("S1");
    gap.track_gap = Some(Length::from_steps(-2));
    is(&refused(&[gap]), Rule::Positive, "track_gap");
    let mut sill = fixed("W1");
    sill.sill_height = Some(Length::from_steps(-1));
    is(&refused(&[sill]), Rule::Positive, "sill_height");
    let mut on_floor = fixed("W1");
    on_floor.sill_height = Some(zero);
    assert!(make(&[on_floor]).is_ok());
    let mut wall = fixed("W1");
    wall.wall = Some(zero);
    is(&refused(&[wall]), Rule::Positive, "wall");
    let flat = Spec::new(
        "W1",
        Template::FixedWindow,
        Size::Clear {
            width: mm(500),
            height: zero,
        },
    );
    is(&refused(&[flat]), Rule::Positive, "height");
}

#[test]
fn a_size_outside_the_range_is_refused_with_the_range() {
    let r = refused(&[Spec::new(
        "D1",
        Template::SingleSwingDoor,
        overall(mm(1300), mm(2100)),
    )]);
    is(&r, Rule::Width, "width");
    assert!(r.reason.contains("600 mm to 1200 mm"), "{r}");
    // A clear size is judged by the overall size it makes.
    let r = refused(&[Spec::new(
        "D1",
        Template::SingleSwingDoor,
        Size::Clear {
            width: mm(1100),
            height: mm(2000),
        },
    )]);
    is(&r, Rule::Width, "width");
    assert!(r.reason.contains("clear size"), "{r}");
    let r = refused(&[Spec::new(
        "S1",
        Template::SlidingWindow,
        overall(mm(1800), mm(2500)),
    )]);
    is(&r, Rule::Height, "height");
}

#[test]
fn parts_that_do_not_fit_leave_no_opening_or_are_off_the_grid_are_refused() {
    let mut leaf = door("D1");
    leaf.leaf = Some(mm(120));
    is(&refused(&[leaf]), Rule::Fits, "leaf");
    let mut frame = fixed("W1");
    frame.frame = Some(mm(600));
    is(&refused(&[frame]), Rule::Clear, "clear_width");
    let mut odd = door("D1");
    odd.frame_depth = Some(Length::from_steps(640_001));
    is(&refused(&[odd]), Rule::Even, "frame_depth");
    let mut thin_wall = sliding("S1");
    thin_wall.wall = Some(mm(90));
    let r = refused(&[thin_wall]);
    is(&r, Rule::Wall, "wall");
    assert!(r.reason.contains("100 mm"), "{r}");
}

#[test]
fn a_material_the_project_does_not_have_is_refused_by_the_checker() {
    let mut spec = door("D1");
    spec.frame_material = Some(Id::parse("unobtainium").unwrap());
    let r = refused(&[spec]);
    is(&r, Rule::Checker, "recipe");
    assert!(r.reason.contains("unobtainium"), "{r}");
}

/// A generated fixed window's recipe, as JSON to change.
fn stored() -> serde_json::Value {
    let generated = make(&[fixed("W1")]).unwrap();
    serde_json::from_str(&generated.recipe.to_json()).unwrap()
}

fn verify_json(value: &serde_json::Value) -> Refusal {
    let recipe = construct_families::Recipe::from_value(value).unwrap();
    verify(&recipe, Template::FixedWindow).unwrap_err()
}

#[test]
fn a_stored_tag_that_is_not_what_the_solids_measure_is_refused() {
    let mut value = stored();
    let columns = value["type_table"]["columns"].clone();
    let at = |name: &str| {
        columns
            .as_array()
            .unwrap()
            .iter()
            .position(|c| c == name)
            .unwrap()
    };
    let mut wrong = value.clone();
    wrong["type_table"]["rows"][0]["values"][at("clear_width")] = "length:q0:6400".into();
    let r = verify_json(&wrong);
    is(&r, Rule::Tag, "clear_width");
    assert_eq!(r.mark.as_deref(), Some("W1"));
    // A width changed without its tags: the glass no longer measures what the row says.
    wrong = value.clone();
    wrong["type_table"]["rows"][0]["values"][at("width")] = "length:q0:6400000".into();
    is(&verify_json(&wrong), Rule::Tag, "clear_width");
    value["type_table"]["rows"][0]["values"][at("type_mark")] = "text:".into();
    is(&verify_json(&value), Rule::Tag, "type_mark");
}

#[test]
fn a_stored_recipe_whose_opening_or_solids_changed_is_refused() {
    // The opening cut at the frame's inside: smaller than the frame, so not a clean cut.
    let mut value = stored();
    value["hosting"]["opening"]["profile"] =
        serde_json::json!(["inner_bottom", "inner_right", "inner_top", "inner_left"]);
    is(&verify_json(&value), Rule::Opening, "opening");
    // The glass step renamed: the solid the glass is measured from is not built.
    let mut value = stored();
    value["steps"][1]["key"] = "pane".into();
    is(&verify_json(&value), Rule::Evaluation, "glass");
}
