//! The phase list, the option sets and the element fields: changes, refusals, and stored bytes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use construct_phases::{DesignOptions, ElementStaging, OptionRef, PhaseError, PhaseList};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn names(list: &PhaseList) -> Vec<&str> {
    list.phases().iter().map(|p| p.name.as_str()).collect()
}

fn base() -> PhaseList {
    PhaseList::new()
        .add(id("ph-a"), "Existing")
        .unwrap()
        .add(id("ph-b"), "New works")
        .unwrap()
}

#[test]
fn phases_are_added_at_the_end() {
    let list = base().add(id("ph-c"), "Later").unwrap();
    assert_eq!(names(&list), ["Existing", "New works", "Later"]);
    assert_eq!(names(&base()), ["Existing", "New works"]);
}

#[test]
fn renaming_keeps_the_id_and_the_place() {
    let list = base().rename(&id("ph-a"), "As found").unwrap();
    assert_eq!(names(&list), ["As found", "New works"]);
    assert_eq!(list.position(&id("ph-a")), Some(0));
    assert_eq!(base().rename(&id("ph-a"), "Existing").unwrap(), base());
}

#[test]
fn moving_keeps_every_id() {
    let three = base().add(id("ph-c"), "Later").unwrap();
    let first = three.move_to(&id("ph-c"), 0).unwrap();
    assert_eq!(names(&first), ["Later", "Existing", "New works"]);
    let last = three.move_to(&id("ph-a"), 2).unwrap();
    assert_eq!(names(&last), ["New works", "Later", "Existing"]);
    assert_eq!(three.move_to(&id("ph-b"), 1).unwrap(), three);
    assert_eq!(last.position(&id("ph-a")), Some(2));
}

#[test]
fn phase_changes_are_refused_as_values() {
    assert_eq!(base().add(id("ph-c"), "  "), Err(PhaseError::EmptyName));
    assert_eq!(
        base().add(id("ph-a"), "Another"),
        Err(PhaseError::DuplicateId(id("ph-a")))
    );
    assert_eq!(
        base().add(id("ph-c"), "Existing"),
        Err(PhaseError::DuplicateName("Existing".to_owned()))
    );
    assert_eq!(
        base().rename(&id("ph-b"), "Existing"),
        Err(PhaseError::DuplicateName("Existing".to_owned()))
    );
    assert_eq!(
        base().rename(&id("ph-x"), "Name"),
        Err(PhaseError::UnknownPhase(id("ph-x")))
    );
    assert_eq!(
        base().move_to(&id("ph-a"), 2),
        Err(PhaseError::PositionOutOfRange {
            position: 2,
            len: 2
        })
    );
    assert_eq!(
        base().move_to(&id("ph-x"), 0),
        Err(PhaseError::UnknownPhase(id("ph-x")))
    );
}

#[test]
fn a_demolished_phase_may_not_come_before_the_created_phase() {
    let options = DesignOptions::new();
    let ok = ElementStaging::new(&base(), &options, id("ph-a"), Some(id("ph-b")), None);
    assert!(ok.is_ok());
    let same = ElementStaging::new(&base(), &options, id("ph-b"), Some(id("ph-b")), None);
    assert!(same.is_ok());
    assert_eq!(
        ElementStaging::new(&base(), &options, id("ph-b"), Some(id("ph-a")), None),
        Err(PhaseError::DemolishedBeforeCreated {
            created: id("ph-b"),
            demolished: id("ph-a")
        })
    );
    assert_eq!(
        ElementStaging::new(&base(), &options, id("ph-x"), None, None),
        Err(PhaseError::UnknownPhase(id("ph-x")))
    );
    assert_eq!(
        ElementStaging::new(&base(), &options, id("ph-a"), Some(id("ph-x")), None),
        Err(PhaseError::UnknownPhase(id("ph-x")))
    );
}

#[test]
fn moving_phases_can_break_an_element_and_check_says_so() {
    let element = ElementStaging::new(
        &base(),
        &DesignOptions::new(),
        id("ph-a"),
        Some(id("ph-b")),
        None,
    )
    .unwrap();
    let swapped = base().move_to(&id("ph-b"), 0).unwrap();
    assert!(element.check(&swapped, &DesignOptions::new()).is_err());
}

fn sets() -> DesignOptions {
    DesignOptions::new()
        .add_set(id("set-1"), "Entrance", id("opt-ramp"), "Ramp")
        .unwrap()
}

#[test]
fn a_new_set_has_its_first_option_as_primary() {
    let set = sets().set(&id("set-1")).unwrap().clone();
    assert_eq!(set.primary, id("opt-ramp"));
    assert_eq!(set.options.len(), 1);
}

#[test]
fn adding_an_option_keeps_the_primary_and_making_primary_keeps_the_options() {
    let two = sets()
        .add_option(&id("set-1"), id("opt-steps"), "Steps")
        .unwrap();
    let set = two.set(&id("set-1")).unwrap();
    assert_eq!(set.primary, id("opt-ramp"));
    assert_eq!(set.options.len(), 2);
    let moved = two.make_primary(&id("set-1"), &id("opt-steps")).unwrap();
    let set = moved.set(&id("set-1")).unwrap();
    assert_eq!(set.primary, id("opt-steps"));
    assert_eq!(set.options.len(), 2);
}

#[test]
fn option_changes_are_refused_as_values() {
    assert_eq!(
        sets().add_set(id("set-2"), "", id("opt-b"), "B"),
        Err(PhaseError::EmptyName)
    );
    assert_eq!(
        sets().add_set(id("set-1"), "Other", id("opt-b"), "B"),
        Err(PhaseError::DuplicateId(id("set-1")))
    );
    assert_eq!(
        sets().add_set(id("set-2"), "Other", id("opt-ramp"), "B"),
        Err(PhaseError::DuplicateId(id("opt-ramp")))
    );
    assert_eq!(
        sets().add_set(id("set-2"), "Other", id("set-2"), "B"),
        Err(PhaseError::DuplicateId(id("set-2")))
    );
    assert_eq!(
        sets().add_set(id("set-2"), "Entrance", id("opt-b"), "B"),
        Err(PhaseError::DuplicateName("Entrance".to_owned()))
    );
    assert_eq!(
        sets().add_option(&id("set-9"), id("opt-b"), "B"),
        Err(PhaseError::UnknownSet(id("set-9")))
    );
    assert_eq!(
        sets().add_option(&id("set-1"), id("opt-ramp"), "B"),
        Err(PhaseError::DuplicateId(id("opt-ramp")))
    );
    assert_eq!(
        sets().add_option(&id("set-1"), id("opt-b"), "Ramp"),
        Err(PhaseError::DuplicateName("Ramp".to_owned()))
    );
    assert_eq!(
        sets().make_primary(&id("set-1"), &id("opt-b")),
        Err(PhaseError::UnknownOption {
            set: id("set-1"),
            option: id("opt-b")
        })
    );
    assert_eq!(
        sets().make_primary(&id("set-9"), &id("opt-ramp")),
        Err(PhaseError::UnknownSet(id("set-9")))
    );
}

#[test]
fn an_option_may_share_a_name_across_sets() {
    let two = sets()
        .add_set(id("set-2"), "Roof", id("opt-r1"), "Ramp")
        .unwrap();
    assert_eq!(two.sets().len(), 2);
}

#[test]
fn an_element_names_one_option_that_exists() {
    let reference = |set: &str, option: &str| OptionRef {
        set: id(set),
        option: id(option),
    };
    let good = ElementStaging::new(
        &base(),
        &sets(),
        id("ph-a"),
        None,
        Some(reference("set-1", "opt-ramp")),
    );
    assert!(good.is_ok());
    assert_eq!(
        ElementStaging::new(
            &base(),
            &sets(),
            id("ph-a"),
            None,
            Some(reference("set-9", "opt-ramp"))
        ),
        Err(PhaseError::UnknownSet(id("set-9")))
    );
    assert_eq!(
        ElementStaging::new(
            &base(),
            &sets(),
            id("ph-a"),
            None,
            Some(reference("set-1", "opt-x"))
        ),
        Err(PhaseError::UnknownOption {
            set: id("set-1"),
            option: id("opt-x")
        })
    );
}

#[test]
fn the_stored_bytes_are_pinned() {
    let list = serde_json::to_string(&base()).unwrap();
    assert_eq!(
        list,
        r#"[{"id":"ph-a","name":"Existing"},{"id":"ph-b","name":"New works"}]"#
    );
    let options = sets()
        .add_option(&id("set-1"), id("opt-steps"), "Steps")
        .unwrap();
    assert_eq!(
        serde_json::to_string(&options).unwrap(),
        concat!(
            r#"[{"id":"set-1","name":"Entrance","options":["#,
            r#"{"id":"opt-ramp","name":"Ramp"},{"id":"opt-steps","name":"Steps"}],"#,
            r#""primary":"opt-ramp"}]"#
        )
    );
    let element = ElementStaging::new(
        &base(),
        &options,
        id("ph-a"),
        Some(id("ph-b")),
        Some(OptionRef {
            set: id("set-1"),
            option: id("opt-steps"),
        }),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&element).unwrap(),
        concat!(
            r#"{"created":"ph-a","demolished":"ph-b","design_option":"#,
            r#"{"set":"set-1","option":"opt-steps"}}"#
        )
    );
    let plain = ElementStaging::new(&base(), &options, id("ph-b"), None, None).unwrap();
    assert_eq!(
        serde_json::to_string(&plain).unwrap(),
        r#"{"created":"ph-b","demolished":null,"design_option":null}"#
    );
}

#[test]
fn the_same_data_gives_the_same_bytes_and_reads_back() {
    let once = serde_json::to_string(&base()).unwrap();
    let twice = serde_json::to_string(&base()).unwrap();
    assert_eq!(once, twice);
    let back: PhaseList = serde_json::from_str(&once).unwrap();
    assert_eq!(back, base());
    let options = sets();
    let text = serde_json::to_string(&options).unwrap();
    let back: DesignOptions = serde_json::from_str(&text).unwrap();
    assert_eq!(back, options);
}

#[test]
fn reading_stored_values_checks_them() {
    let repeated = r#"[{"id":"ph-a","name":"One"},{"id":"ph-a","name":"Two"}]"#;
    assert!(serde_json::from_str::<PhaseList>(repeated).is_err());
    let same_name = r#"[{"id":"ph-a","name":"One"},{"id":"ph-b","name":"One"}]"#;
    assert!(serde_json::from_str::<PhaseList>(same_name).is_err());
    let bad_id = r#"[{"id":"no good","name":"One"}]"#;
    assert!(serde_json::from_str::<PhaseList>(bad_id).is_err());

    let empty_set = r#"[{"id":"s","name":"S","options":[],"primary":"o"}]"#;
    assert!(serde_json::from_str::<DesignOptions>(empty_set).is_err());
    let lost_primary = r#"[{"id":"s","name":"S","options":[{"id":"o","name":"O"}],"primary":"p"}]"#;
    assert!(serde_json::from_str::<DesignOptions>(lost_primary).is_err());
    let good = r#"[{"id":"s","name":"S","options":[{"id":"o","name":"O"}],"primary":"o"}]"#;
    assert!(serde_json::from_str::<DesignOptions>(good).is_ok());
}
