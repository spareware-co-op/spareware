//! Names: what is allowed, how two compare, and the rule that suggests the next.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_grids::{GridError, MAX_NAME_CHARS, clean_name, next_name, same_name, suggest};

#[test]
fn the_rule_goes_one_on() {
    for (name, next) in [
        ("1", "2"),
        ("9", "10"),
        ("09", "10"),
        ("009", "010"),
        ("99", "100"),
        ("A", "B"),
        ("Y", "Z"),
        ("Z", "AA"),
        ("AZ", "BA"),
        ("ZZ", "AAA"),
        ("a", "b"),
        ("z", "aa"),
        ("A-3", "A-4"),
        ("A9", "A10"),
        ("G.09", "G.10"),
        ("1Z", "1AA"),
        ("Ab", "Ac"),
        ("A-", "A-1"),
        ("", "1"),
    ] {
        assert_eq!(next_name(name), next, "after {name:?}");
    }
}

#[test]
fn no_letter_is_skipped() {
    let mut name = "G".to_owned();
    for _ in 0..4 {
        name = next_name(&name);
    }
    assert_eq!(name, "K", "H, I, J, K: I and O are names like any other");
    assert_eq!(next_name("H"), "I");
    assert_eq!(next_name("N"), "O");
}

#[test]
fn a_run_of_names_is_every_name_once() {
    let mut name = "A".to_owned();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..800 {
        assert!(seen.insert(name.clone()), "{name} came twice");
        name = next_name(&name);
    }
    assert!(
        seen.contains("Z") && seen.contains("AA") && seen.contains("ZZ") && seen.contains("AAA")
    );
}

#[test]
fn suggesting_skips_what_is_taken_without_regard_to_case() {
    assert_eq!(suggest(None, |_| false), "1");
    assert_eq!(suggest(None, |name| name == "1"), "2");
    assert_eq!(suggest(Some("1"), |name| name == "2" || name == "3"), "4");
    assert_eq!(suggest(Some("A"), |name| same_name(name, "b")), "C");
    assert_eq!(suggest(Some("Y"), |name| name == "Z"), "AA");
    // A taker that says yes to everything still ends.
    assert_ne!(suggest(Some("A"), |_| true), "");
}

#[test]
fn names_compare_without_regard_to_case() {
    assert!(same_name("A", "a"));
    assert!(same_name("North Wall", "NORTH WALL"));
    assert!(!same_name("A", "B"));
    assert!(!same_name("A", "A "), "compare cleaned names");
}

#[test]
fn a_name_is_trimmed_and_limited() {
    assert_eq!(clean_name("  A  ").unwrap(), "A");
    assert_eq!(clean_name(""), Err(GridError::EmptyName));
    assert_eq!(clean_name("   "), Err(GridError::EmptyName));
    assert_eq!(clean_name("a\nb"), Err(GridError::BadName));
    assert!(clean_name(&"x".repeat(MAX_NAME_CHARS)).is_ok());
    assert_eq!(
        clean_name(&"x".repeat(MAX_NAME_CHARS + 1)),
        Err(GridError::BadName)
    );
}
