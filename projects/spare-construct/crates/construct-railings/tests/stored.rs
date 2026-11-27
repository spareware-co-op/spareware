//! A railing's stored form: the rule set read strictly with its clauses, the type and the railing
//! written and read back in their one spelling, the record and its recipe, the filled recipe the
//! builders take, its read-only parameters, and the same data giving the same bytes.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{at, laid, mm, on_stair, sketched, spaced};
use construct_levels::Tie;
use construct_params::{Units, Value};
use construct_railings::{
    BALUSTERS, NZ_F4_AS1, OpeningLimits, PIECES_ARG, RAIL_LENGTH, Railing, RailingError,
    RailingType, RuleSet, STANDARD_MATERIAL, Side, filled, railing_of, railing_record, read_only,
    stair_of, with_railing,
};
use kit_core::Id;
use serde_json::json;

/// The built-in rule set holds F4/AS1's figures, each with its clause.
#[test]
fn the_new_zealand_rule_set_holds_f4_as1() {
    let set = RuleSet::builtin(NZ_F4_AS1).unwrap();
    assert_eq!(
        set.edition,
        "F4/AS1 third edition, amendment 2, effective 1 January 2017; handrail heights from D1/AS1 second edition, amendment 6"
    );
    let heights: Vec<(&str, Option<i64>, i64)> = set
        .categories
        .iter()
        .map(|c| {
            (
                c.id.as_str(),
                c.flight_least.as_ref().and_then(|l| l.length.whole_mm()),
                c.level_least.length.whole_mm().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        heights,
        vec![
            ("household-stair", Some(900), 900),
            ("household-edge", None, 1000),
            ("stair", Some(900), 1100),
            ("seating", None, 800),
            ("other", None, 1100),
        ]
    );
    let stair = set.category("stair").unwrap();
    assert_eq!(
        stair.flight().clause,
        "F4/AS1 1.1.1, Table 1 and its Note 1"
    );
    assert_eq!(stair.openings, vec!["children", "no-children"]);
    match &set.openings_for(stair, "children").unwrap().limits {
        OpeningLimits::Sphere {
            sphere,
            stair_sphere,
        } => {
            assert_eq!(
                (sphere.length, sphere.clause.as_str()),
                (mm(100), "F4/AS1 1.2.1 b)")
            );
            assert_eq!(stair_sphere.length, mm(150));
        }
        other @ OpeningLimits::Most { .. } => panic!("{other:?}"),
    }
    assert_eq!(
        (set.handrail_least.length, set.handrail_most.length),
        (mm(900), mm(1000))
    );
    assert_eq!(
        RuleSet::builtin("nz-g1").unwrap_err(),
        RailingError::UnknownRules("nz-g1".to_owned())
    );
}

/// A rule set is read strictly: a field added, a clause left out or a length off the grid is
/// refused in words.
#[test]
fn a_rule_set_is_read_strictly() {
    let text = include_str!("../rules/nz-f4-as1.json");
    let refusals = [
        (
            text.replacen("\"version\": 1,", "\"version\": 1, \"extra\": 1,", 1),
            "unknown field",
        ),
        (
            text.replacen("\"clause\": \"D1/AS1 6.0.6\"", "\"clause\": \" \"", 1),
            "cites no clause",
        ),
        (
            text.replacen("\"900 mm\"", "\"900.0001 mm\"", 1),
            "household-stair's least height on a flight",
        ),
        (
            text.replacen(
                "\"openings\": [\"children\"]",
                "\"openings\": [\"toddlers\"]",
                1,
            ),
            "names no openings rule",
        ),
    ];
    for (bad, words) in refusals {
        let refused = RuleSet::from_json(&bad).unwrap_err().to_string();
        assert!(refused.contains(words), "{refused}");
    }
}

/// A railing written and read back is the same railing, with the same arguments; any other
/// spelling is refused.
#[test]
fn a_railing_reads_back_in_its_one_spelling() {
    let sketch = sketched(spaced(100), vec![at(0, 0), at(3000, 0), at(3000, 1500)]);
    let stair = on_stair(RailingType::standard(), Side::Both, 50);
    for railing in [&sketch, &stair] {
        let args = railing.to_args();
        assert_eq!(Railing::from_args(&args).unwrap(), *railing);
    }
    let args = sketch.to_args();
    assert_eq!(
        args["path"],
        json!({ "points": [
            ["length:q0:0", "length:q0:0"],
            ["length:q0:19200000", "length:q0:0"],
            ["length:q0:19200000", "length:q0:9600000"]
        ] })
    );
    assert_eq!(
        stair.to_args()["path"],
        json!({ "inset": "length:q0:320000", "side": "both", "stair": "S-1" })
    );
    assert_eq!(args["type"]["material"], json!(STANDARD_MATERIAL));
    assert_eq!(args["type"]["balusters"]["justify"], json!("centre"));
    let mut wrong = args.clone();
    wrong.insert("extra".to_owned(), json!(1));
    assert!(Railing::from_args(&wrong).is_err());
    let mut wrong = args.clone();
    wrong.insert("flipped".to_owned(), json!("no"));
    assert!(Railing::from_args(&wrong).is_err());
    let mut wrong = args;
    wrong.insert(
        "path".to_owned(),
        json!({ "points": [["0 mm", "0 mm"], ["1 m", "0 mm"]] }),
    );
    assert!(Railing::from_args(&wrong).is_err());
    assert_eq!(
        stair_of(&construct_railings::railing_recipe(&stair)),
        Some(Id::parse("S-1").unwrap())
    );
}

/// A type is patched from a person's typed text: a field given replaces its own, a handrail is
/// added and taken away, and anything that does not read or fit is refused.
#[test]
fn a_type_is_patched_from_typed_text() {
    let units = Units::default();
    let standard = RailingType::standard();
    let patched = standard
        .patched(
            &json!({ "top_rail": { "height": "1.1 m" },
                     "handrail": { "profile": { "round": "40 mm" }, "height": "900 mm", "offset": "60 mm" },
                     "balusters": { "justify": "start" } }),
            &units,
        )
        .unwrap();
    assert_eq!(patched.top_rail.height, mm(1100));
    assert_eq!(patched.top_rail.profile, standard.top_rail.profile);
    assert_eq!(patched.handrail.unwrap().offset, mm(60));
    assert_eq!(
        patched.balusters.justify,
        construct_railings::Justify::Start
    );
    let without = patched
        .patched(&json!({ "handrail": null }), &units)
        .unwrap();
    assert_eq!(without.handrail, None);
    let refusals = [
        (
            json!({ "balusters": { "spacing": "15 mm" } }),
            "Balusters 20 mm deep along the path do not fit at a spacing of 15 mm.",
        ),
        (
            json!({ "material": "cheese" }),
            "There is no material \"cheese\" in the default library.",
        ),
        (
            json!({ "top_rail": { "height": "0 mm" } }),
            "A railing's top rail's height must be more than nothing.",
        ),
        (
            json!({ "posts": { "round": "600 mm" } }),
            "A railing's posts' profile, 600 mm is beyond what it holds.",
        ),
        (
            json!({ "colour": "red" }),
            "A railing's type: the type has no field \"colour\".",
        ),
        (
            json!({ "handrail": { "height": "900 mm" } }),
            "A railing's type: the handrail's profile is missing.",
        ),
    ];
    for (patch, words) in refusals {
        assert_eq!(
            standard.patched(&patch, &units).unwrap_err().to_string(),
            words
        );
    }
}

/// The record holds the railing in its recipe and the railings category and base tie in its
/// props; the filled recipe holds the material and every piece, and nothing else.
#[test]
fn the_record_and_the_filled_recipe() {
    let railing = sketched(spaced(100), vec![at(0, 0), at(3000, 0)]);
    let base = Tie {
        level: Id::parse("L-g").unwrap(),
        offset: mm(0),
    };
    let record = railing_record(
        &Id::parse("R-1").unwrap(),
        &Id::parse("doc").unwrap(),
        "Balcony rail",
        &base,
        &railing,
    );
    assert_eq!(record.kind, "railing");
    assert_eq!(record.props["@category"], "1000");
    assert_eq!(railing_of(&record.recipe).unwrap(), railing);
    let built = filled(&record.recipe, None).unwrap();
    assert_eq!(
        built.args.keys().cloned().collect::<Vec<_>>(),
        vec!["material".to_owned(), PIECES_ARG.to_owned()]
    );
    let pieces = built.args[PIECES_ARG].as_array().unwrap();
    // Two posts, one piece of rail, 29 balusters.
    assert_eq!(pieces.len(), 32);
    assert_eq!(pieces[0]["id"], json!("post.1.1"));
    assert_eq!(pieces[2]["id"], json!("rail.1.1"));
    assert_eq!(pieces[3]["id"], json!("baluster.1.1.1"));
    assert_eq!(pieces[3]["part"], json!("baluster"));
    // The same railing gives the same bytes.
    let again = filled(&record.recipe, None).unwrap();
    assert_eq!(
        serde_json::to_vec(&again.args).unwrap(),
        serde_json::to_vec(&built.args).unwrap()
    );
    let moved = with_railing(record.clone(), &railing.flipped().unwrap()).unwrap();
    assert_eq!(moved.props, record.props);
    assert_ne!(moved.recipe, record.recipe);
}

/// What a railing reads back for a schedule: its rail length and baluster count among them.
#[test]
fn a_railing_reads_back_its_quantities() {
    let railing = sketched(spaced(100), vec![at(0, 0), at(3000, 0)]);
    let rows = read_only(&railing, &laid(&railing, None)).unwrap();
    let value = |id: &str| {
        rows.iter()
            .find(|(d, _)| d.id().as_str() == id)
            .and_then(|(_, v)| v.clone())
    };
    assert_eq!(value(RAIL_LENGTH), Some(Value::Length(mm(3000))));
    assert_eq!(value(BALUSTERS), Some(Value::Whole(29)));
    assert_eq!(value("railing.posts"), Some(Value::Whole(2)));
    assert_eq!(value("railing.handrail-length"), None);
    assert_eq!(value("railing.height"), Some(Value::Length(mm(1000))));
}
