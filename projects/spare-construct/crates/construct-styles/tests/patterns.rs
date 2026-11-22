//! Line patterns: the built-ins, project patterns, and the refusals.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_styles::patterns::{
    DASH_DOT, DASH_DOT_DOT, DASHED, DOTTED, FIRST_USER_PATTERN_ID, HIDDEN, SOLID,
};
use construct_styles::{Micrometres, PatternId, PatternTable, Segment, StyleError};

fn um(value: u32) -> Micrometres {
    Micrometres::new(value).unwrap()
}

#[test]
fn the_six_built_ins_are_there_in_order() {
    let table = PatternTable::built_in();
    let names: Vec<&str> = table.patterns().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Solid",
            "Dashed",
            "Dotted",
            "Dash-dot (centre)",
            "Dash-dot-dot",
            "Hidden"
        ]
    );
    let ids: Vec<PatternId> = table.patterns().iter().map(|p| p.id).collect();
    assert_eq!(ids, [SOLID, DASHED, DOTTED, DASH_DOT, DASH_DOT_DOT, HIDDEN]);
    assert_eq!(table.next_id(), PatternId(FIRST_USER_PATTERN_ID));
}

#[test]
fn built_in_cycles_are_exact() {
    let table = PatternTable::built_in();
    let cycle = |id| table.get(id).unwrap().cycle_micrometres();
    assert_eq!(cycle(SOLID), 0);
    assert_eq!(cycle(DASHED), 8000);
    assert_eq!(cycle(DOTTED), 1500);
    assert_eq!(cycle(DASH_DOT), 16_000);
    assert_eq!(cycle(DASH_DOT_DOT), 18_000);
    assert_eq!(cycle(HIDDEN), 4000);
    assert_eq!(table.get(SOLID).unwrap().segments.len(), 0);
    assert_eq!(
        table.get(DOTTED).unwrap().segments,
        [Segment::Dot, Segment::Gap(um(1500))]
    );
}

#[test]
fn a_project_adds_a_pattern_and_gets_the_next_id() {
    let table = PatternTable::built_in();
    let (one, id) = table
        .with_pattern(
            "Long dash",
            vec![Segment::Dash(um(20_000)), Segment::Gap(um(3000))],
        )
        .unwrap();
    assert_eq!(id, PatternId(100));
    assert_eq!(one.get(id).unwrap().cycle_micrometres(), 23_000);
    assert_eq!(one.next_id(), PatternId(101));
    // The old table is as it was.
    assert!(table.get(id).is_none());
    let (two, second) = one
        .with_pattern(
            "Dot dash",
            vec![
                Segment::Dot,
                Segment::Gap(um(1000)),
                Segment::Dash(um(2000)),
                Segment::Gap(um(1000)),
            ],
        )
        .unwrap();
    assert_eq!(second, PatternId(101));
    assert_eq!(two.patterns().len(), 8);
}

#[test]
fn a_pattern_of_a_dot_and_a_gap_is_allowed() {
    let table = PatternTable::built_in();
    assert!(
        table
            .with_pattern("Sparse", vec![Segment::Dot, Segment::Gap(um(5000))])
            .is_ok()
    );
}

#[test]
fn bad_patterns_are_refused_as_values() {
    let table = PatternTable::built_in();
    let add =
        |name: &str, segments: Vec<Segment>| table.with_pattern(name, segments).map(|(_, id)| id);
    assert_eq!(add("Empty", vec![]), Err(StyleError::NoMarks));
    assert_eq!(
        add("Gap only", vec![Segment::Gap(um(1000))]),
        Err(StyleError::NoMarks)
    );
    assert_eq!(add("Dots", vec![Segment::Dot]), Err(StyleError::ZeroCycle));
    assert_eq!(
        add("Dots", vec![Segment::Dot, Segment::Dot]),
        Err(StyleError::ZeroCycle)
    );
    assert_eq!(
        add("Many", vec![Segment::Dot; 65]),
        Err(StyleError::TooManySegments)
    );
    assert_eq!(
        add("Solid", vec![Segment::Dash(um(1000))]),
        Err(StyleError::DuplicateName)
    );
    assert_eq!(
        add("", vec![Segment::Dash(um(1000))]),
        Err(StyleError::BadName)
    );
}

#[test]
fn built_ins_cannot_be_removed() {
    let table = PatternTable::built_in();
    for id in [SOLID, DASHED, DOTTED, DASH_DOT, DASH_DOT_DOT, HIDDEN] {
        assert_eq!(
            table.without_pattern(id),
            Err(StyleError::BuiltIn { id: id.0 })
        );
    }
}

#[test]
fn a_project_pattern_can_be_removed_and_its_id_is_not_reused() {
    let (table, id) = PatternTable::built_in()
        .with_pattern("Mine", vec![Segment::Dash(um(1000))])
        .unwrap();
    let removed = table.without_pattern(id).unwrap();
    assert!(removed.get(id).is_none());
    assert_eq!(removed.next_id(), PatternId(101));
    assert_eq!(
        removed.without_pattern(id),
        Err(StyleError::NoSuchPattern { id: 100 })
    );
    let (again, new_id) = removed
        .with_pattern("Mine", vec![Segment::Dash(um(1000))])
        .unwrap();
    assert_eq!(new_id, PatternId(101));
    assert!(again.get(PatternId(100)).is_none());
}
