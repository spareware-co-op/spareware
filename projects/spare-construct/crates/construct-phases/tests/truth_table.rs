//! The filter rule's truth table, written out by hand: every created and demolished phase over
//! three phases, seen from every view phase, then every filter against every state, then the
//! option rule.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_phases::{
    DesignOptions, ElementStaging, ElementState, OptionChoice, OptionRef, PhaseError, PhaseFilter,
    PhaseList, ViewSettings, Visibility, visibility,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn phases() -> PhaseList {
    PhaseList::new()
        .add(id("ph-a"), "Existing")
        .unwrap()
        .add(id("ph-b"), "Demolition")
        .unwrap()
        .add(id("ph-c"), "New works")
        .unwrap()
}

fn options() -> DesignOptions {
    DesignOptions::new()
        .add_set(id("set-1"), "Entrance", id("opt-ramp"), "Ramp")
        .unwrap()
        .add_option(&id("set-1"), id("opt-steps"), "Steps")
        .unwrap()
}

fn phase_id(index: usize) -> Id {
    id(["ph-a", "ph-b", "ph-c"][index])
}

fn view(phase: usize, filter: PhaseFilter) -> ViewSettings {
    ViewSettings {
        phase: phase_id(phase),
        filter,
        choices: Vec::new(),
    }
}

fn staging(created: usize, demolished: Option<usize>) -> ElementStaging {
    ElementStaging::new(
        &phases(),
        &options(),
        phase_id(created),
        demolished.map(phase_id),
        None,
    )
    .unwrap()
}

fn ask(view: &ViewSettings, element: &ElementStaging) -> Visibility {
    visibility(&phases(), &options(), view, element).unwrap()
}

fn letter(visible: Visibility) -> char {
    match visible {
        Visibility::Hidden => 'H',
        Visibility::Shown(ElementState::Existing) => 'E',
        Visibility::Shown(ElementState::New) => 'N',
        Visibility::Shown(ElementState::Demolished) => 'D',
        Visibility::Shown(ElementState::Temporary) => 'T',
    }
}

#[test]
fn states_over_three_phases_with_show_all() {
    // created, demolished, then the state seen from view phase 0, 1 and 2.
    let table: [(usize, Option<usize>, &str); 9] = [
        (0, None, "NEE"),
        (0, Some(0), "THH"),
        (0, Some(1), "NDH"),
        (0, Some(2), "NED"),
        (1, None, "HNE"),
        (1, Some(1), "HTH"),
        (1, Some(2), "HND"),
        (2, None, "HHN"),
        (2, Some(2), "HHT"),
    ];
    for (created, demolished, expected) in table {
        let element = staging(created, demolished);
        let seen: String = (0..3)
            .map(|at| letter(ask(&view(at, PhaseFilter::ShowAll), &element)))
            .collect();
        assert_eq!(
            seen, expected,
            "created {created}, demolished {demolished:?}"
        );
    }
}

#[test]
fn every_filter_against_every_state() {
    use ElementState::{Demolished, Existing, New, Temporary};
    // Each filter, then whether it shows existing, new, demolished, temporary.
    let table: [(PhaseFilter, [bool; 4]); 7] = [
        (PhaseFilter::ShowAll, [true, true, true, true]),
        (PhaseFilter::ShowNew, [false, true, false, true]),
        (PhaseFilter::ShowExisting, [true, false, false, false]),
        (PhaseFilter::ShowDemolished, [false, false, true, false]),
        (
            PhaseFilter::ShowExistingAndDemolished,
            [true, false, true, false],
        ),
        (PhaseFilter::ShowExistingAndNew, [true, true, false, false]),
        (PhaseFilter::ShowNewAndDemolished, [false, true, true, true]),
    ];
    for (filter, shown) in table {
        for (state, expected) in [Existing, New, Demolished, Temporary]
            .into_iter()
            .zip(shown)
        {
            assert_eq!(filter.shows(state), expected, "{filter:?} on {state:?}");
        }
    }
}

#[test]
fn filters_apply_through_the_rule_and_never_reveal_hidden_elements() {
    // Seen from the middle phase: existing, new, demolished and temporary examples.
    let existing = staging(0, None);
    let new = staging(1, None);
    let demolished = staging(0, Some(1));
    let temporary = staging(1, Some(1));
    let gone = staging(0, Some(0));
    let later = staging(2, None);
    let shown = |filter: PhaseFilter| -> String {
        [&existing, &new, &demolished, &temporary, &gone, &later]
            .into_iter()
            .map(|element| letter(ask(&view(1, filter), element)))
            .collect()
    };
    assert_eq!(shown(PhaseFilter::ShowAll), "ENDTHH");
    assert_eq!(shown(PhaseFilter::ShowNew), "HNHTHH");
    assert_eq!(shown(PhaseFilter::ShowExisting), "EHHHHH");
    assert_eq!(shown(PhaseFilter::ShowDemolished), "HHDHHH");
    assert_eq!(shown(PhaseFilter::ShowExistingAndDemolished), "EHDHHH");
    assert_eq!(shown(PhaseFilter::ShowExistingAndNew), "ENHHHH");
    assert_eq!(shown(PhaseFilter::ShowNewAndDemolished), "HNDTHH");
}

fn in_option(option: &str) -> ElementStaging {
    ElementStaging::new(
        &phases(),
        &options(),
        phase_id(0),
        None,
        Some(OptionRef {
            set: id("set-1"),
            option: id(option),
        }),
    )
    .unwrap()
}

#[test]
fn the_view_settings_are_stored_as_pinned_text() {
    let mut settings = view(1, PhaseFilter::ShowExistingAndNew);
    settings.choices.push(OptionChoice {
        set: id("set-1"),
        option: id("opt-steps"),
    });
    assert_eq!(
        serde_json::to_string(&settings).unwrap(),
        concat!(
            r#"{"phase":"ph-b","filter":"show_existing_and_new","#,
            r#""choices":[{"set":"set-1","option":"opt-steps"}]}"#
        )
    );
    let back: ViewSettings =
        serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
    assert_eq!(back, settings);
}

#[test]
fn main_model_elements_ignore_options() {
    let mut chosen = view(0, PhaseFilter::ShowAll);
    chosen.choices.push(OptionChoice {
        set: id("set-1"),
        option: id("opt-steps"),
    });
    assert_eq!(
        ask(&chosen, &staging(0, None)),
        Visibility::Shown(ElementState::New)
    );
}

#[test]
fn an_unchosen_set_shows_its_primary_only() {
    let plain = view(0, PhaseFilter::ShowAll);
    assert_eq!(
        ask(&plain, &in_option("opt-ramp")),
        Visibility::Shown(ElementState::New)
    );
    assert_eq!(ask(&plain, &in_option("opt-steps")), Visibility::Hidden);
}

#[test]
fn a_chosen_option_shows_and_the_others_do_not() {
    let mut chosen = view(0, PhaseFilter::ShowAll);
    chosen.choices.push(OptionChoice {
        set: id("set-1"),
        option: id("opt-steps"),
    });
    assert_eq!(
        ask(&chosen, &in_option("opt-steps")),
        Visibility::Shown(ElementState::New)
    );
    assert_eq!(ask(&chosen, &in_option("opt-ramp")), Visibility::Hidden);
}

#[test]
fn a_changed_primary_changes_what_an_unchosen_view_shows() {
    let swapped = options()
        .make_primary(&id("set-1"), &id("opt-steps"))
        .unwrap();
    let plain = view(0, PhaseFilter::ShowAll);
    let shown = visibility(&phases(), &swapped, &plain, &in_option("opt-steps")).unwrap();
    assert_eq!(shown, Visibility::Shown(ElementState::New));
    let hidden = visibility(&phases(), &swapped, &plain, &in_option("opt-ramp")).unwrap();
    assert_eq!(hidden, Visibility::Hidden);
}

#[test]
fn the_filter_and_the_option_both_have_to_agree() {
    let mut chosen = view(1, PhaseFilter::ShowDemolished);
    chosen.choices.push(OptionChoice {
        set: id("set-1"),
        option: id("opt-ramp"),
    });
    // Existing, in the chosen option, but the filter keeps only demolished work.
    assert_eq!(ask(&chosen, &in_option("opt-ramp")), Visibility::Hidden);
}

#[test]
fn questions_the_project_cannot_answer_are_refused() {
    let element = staging(0, None);
    let unknown_view = ViewSettings {
        phase: id("ph-zzz"),
        filter: PhaseFilter::ShowAll,
        choices: Vec::new(),
    };
    assert_eq!(
        visibility(&phases(), &options(), &unknown_view, &element),
        Err(PhaseError::UnknownPhase(id("ph-zzz")))
    );

    let mut twice = view(0, PhaseFilter::ShowAll);
    for option in ["opt-ramp", "opt-steps"] {
        twice.choices.push(OptionChoice {
            set: id("set-1"),
            option: id(option),
        });
    }
    assert_eq!(
        visibility(&phases(), &options(), &twice, &element),
        Err(PhaseError::DuplicateChoice(id("set-1")))
    );

    let mut bad_option = view(0, PhaseFilter::ShowAll);
    bad_option.choices.push(OptionChoice {
        set: id("set-1"),
        option: id("opt-nope"),
    });
    assert_eq!(
        visibility(&phases(), &options(), &bad_option, &element),
        Err(PhaseError::UnknownOption {
            set: id("set-1"),
            option: id("opt-nope")
        })
    );

    // Stored fields that no longer fit the list (for example after phases were moved).
    let reversed = phases().move_to(&id("ph-a"), 2).unwrap();
    let late_demolished = ElementStaging {
        created: id("ph-a"),
        demolished: Some(id("ph-b")),
        design_option: None,
    };
    assert_eq!(
        visibility(
            &reversed,
            &options(),
            &view(0, PhaseFilter::ShowAll),
            &late_demolished
        ),
        Err(PhaseError::DemolishedBeforeCreated {
            created: id("ph-a"),
            demolished: id("ph-b")
        })
    );
}
