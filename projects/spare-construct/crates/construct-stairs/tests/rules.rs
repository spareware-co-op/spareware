//! The limits as data: New Zealand's rule set (clause D1 through D1/AS1, second edition, amendment
//! 6) holds the code's figures with their clauses, every pitch is worked as the slope a roof's
//! pitch of that angle is, and a rule set's text that is not exactly as written is refused.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::mm;
use construct_params::Slope;
use construct_stairs::{NZ_D1_AS1, RuleSet, StairError};

/// The text of the built-in rule set, for the tests that change one field of it.
const NZ: &str = include_str!("../rules/nz-d1-as1.json");

/// One category's row: its ID, most pitch, most riser, least going, most rise between landings
/// and least width, in degrees and whole millimetres.
type Row = (&'static str, u32, i64, i64, i64, Option<i64>);

/// Table 6 (risers, goings and pitches), Table 7 (rise between landings) and paragraph 4.2.1
/// (widths) of D1/AS1, category by category, as the rule set holds them.
#[test]
fn new_zealands_rule_set_holds_d1_as1s_figures() {
    let rules = RuleSet::builtin(NZ_D1_AS1).unwrap();
    assert_eq!(rules.id, "nz-d1-as1");
    assert_eq!(
        rules.edition,
        "D1/AS1 second edition, amendment 6, effective 1 January 2017"
    );
    // (id, most pitch, most riser, least going, most rise between landings, least width)
    let table: [Row; 6] = [
        ("accessible", 32, 180, 310, 2500, Some(900)),
        ("common", 37, 190, 280, 2500, Some(900)),
        ("main-private", 37, 190, 280, 4000, None),
        ("secondary-private", 41, 200, 250, 4000, None),
        ("minor-private", 47, 220, 220, 4000, None),
        ("service", 47, 220, 220, 4000, None),
    ];
    assert_eq!(rules.categories.len(), table.len());
    for (id, pitch, riser, going, between, width) in table {
        let c = rules.category(id).unwrap();
        assert_eq!(c.pitch_most.degrees, pitch, "{id}");
        assert_eq!(c.pitch_most.clause, "D1/AS1 4.1.1 and Table 6");
        assert_eq!(c.riser_most.length, mm(riser), "{id}");
        assert_eq!(c.riser_most.clause, "D1/AS1 4.1.1 and Table 6");
        assert_eq!(c.going_least.length, mm(going), "{id}");
        assert_eq!(c.going_least.clause, "D1/AS1 4.1.4 a) and Table 6");
        assert_eq!(c.rise_between_landings_most.length, mm(between), "{id}");
        assert_eq!(
            c.rise_between_landings_most.clause,
            "D1/AS1 4.3.2 and Table 7"
        );
        assert_eq!(
            c.width_least.as_ref().map(|w| w.length),
            width.map(mm),
            "{id}"
        );
    }
    assert_eq!(rules.category("common").unwrap().name, "Common stairway");
    // The limits for every stair.
    assert_eq!(rules.pitch_least.degrees, 23);
    assert_eq!(
        rules.pitch_least.clause,
        "D1/AS1 1.2.1 and Figure 2, and 4.1.1 Comment 2"
    );
    assert_eq!(rules.riser_least.length, mm(150));
    assert_eq!(rules.going_most.length, mm(400));
    assert_eq!(rules.uniform_within.length, mm(5));
    assert_eq!(rules.uniform_within.clause, "D1/AS1 4.1.3");
    assert_eq!(rules.landing_least.length, mm(900));
    assert_eq!(rules.landing_least.clause, "D1/AS1 4.3.4");
    assert_eq!(rules.headroom_least.length, mm(2000));
    assert_eq!(
        rules.headroom_least.clause,
        "D1/AS1 1.4.1, Table 1 and Figure 3"
    );
}

/// A pitch in whole degrees is compared as the slope of rise `round(10^9 tan a)` over 10^9, the
/// rule a roof's pitch in degrees is worked by: tan 23 = 0.42447481620..., tan 32 =
/// 0.62486935190..., tan 37 = 0.75355405010..., tan 41 = 0.86928673781..., tan 47 =
/// 1.07236871002....
#[test]
fn every_pitch_is_the_slope_of_its_angle() {
    let rules = RuleSet::builtin(NZ_D1_AS1).unwrap();
    let slope = |rise: i64| Slope::new(rise, 1_000_000_000).unwrap();
    assert_eq!(rules.pitch_least.slope, slope(424_474_816));
    for (id, rise) in [
        ("accessible", 624_869_352),
        ("common", 753_554_050),
        ("secondary-private", 869_286_738),
        ("service", 1_072_368_710),
    ] {
        assert_eq!(
            rules.category(id).unwrap().pitch_most.slope,
            slope(rise),
            "{id}"
        );
    }
}

/// Table 6 is drawn so that its pitch limit never binds at its own least going: a riser at the
/// category's most pitch on its least going is higher than its most riser, for every category.
#[test]
fn table_6s_pitch_never_binds_at_its_least_going() {
    let rules = RuleSet::builtin(NZ_D1_AS1).unwrap();
    for c in &rules.categories {
        let (p, q) = (c.pitch_most.slope.rise(), c.pitch_most.slope.run());
        let at_pitch = i128::from(c.going_least.length.steps()) * i128::from(p) / i128::from(q);
        assert!(
            at_pitch > i128::from(c.riser_most.length.steps()),
            "{}: {at_pitch} steps",
            c.id
        );
    }
}

/// A rule set this program does not hold, and a category a rule set does not have, are refused
/// by name; the refusal lists the categories there are.
#[test]
fn an_unknown_rule_set_or_category_is_refused_by_name() {
    assert_eq!(
        RuleSet::builtin("au-ncc").unwrap_err(),
        StairError::UnknownRules("au-ncc".to_owned())
    );
    let rules = RuleSet::builtin(NZ_D1_AS1).unwrap();
    let err = rules.category("grand").unwrap_err();
    assert_eq!(
        err.to_string(),
        "The rule set nz-d1-as1 has no stairway category \"grand\"; it has accessible, common, main-private, secondary-private, minor-private, service."
    );
}

/// Another country's limits are another text of the same form: a rule set read from JSON is
/// used as the built-in one is.
#[test]
fn a_rule_set_is_read_from_its_text() {
    let text = NZ.replace("\"nz-d1-as1\"", "\"test-set\"");
    let rules = RuleSet::from_json(&text).unwrap();
    assert_eq!(rules.id, "test-set");
    assert_eq!(rules.categories.len(), 6);
}

/// A rule set's text that is not exactly as written is refused, saying what does not read.
#[test]
fn a_rule_set_that_is_not_as_written_is_refused() {
    let refused = |text: &str, says: &str| {
        let err = RuleSet::from_json(text).unwrap_err();
        let StairError::Malformed(why) = &err else {
            panic!("{err:?}");
        };
        assert!(why.contains(says), "{why}");
    };
    refused(
        &NZ.replace("\"version\": 1", "\"version\": 2"),
        "its version is 2",
    );
    refused(
        &NZ.replace("\"source\"", "\"extra\": 1, \"source\""),
        "unknown field",
    );
    refused(
        &NZ.replace("\"degrees\": 23", "\"degrees\": 90"),
        "the least pitch is 90 degrees",
    );
    refused(
        &NZ.replace("\"150 mm\"", "\"150.0001 mm\""),
        "the least riser",
    );
    refused(&NZ.replace("\"5 mm\"", "\"-5 mm\""), "below nothing");
    refused(
        &NZ.replace("\"clause\": \"D1/AS1 4.3.4\"", "\"clause\": \" \""),
        "the least landing cites no clause",
    );
    refused(
        &NZ.replace("\"id\": \"service\"", "\"id\": \"common\""),
        "category \"common\" is blank or given twice",
    );
    refused(
        &NZ.replace("\"200 mm\"", "\"two hundred\""),
        "secondary-private's most riser",
    );
}
