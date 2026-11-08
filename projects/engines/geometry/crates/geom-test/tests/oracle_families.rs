// SPDX-License-Identifier: Apache-2.0
//! The generated sketch families and the diagnosis oracle check (`geom_test::oracle::families`,
//! piece G231).
//!
//! Every family is built so that its diagnosis is known by construction, so the kernel's diagnosis of
//! each case must be the expected one, and an oracle's report of the same three counts differs when
//! they are not.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "tests; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_test::Seed;
use geom_test::oracle::families::{
    Case, Diagnosis, Family, cases, check, holds, judge, replay_text,
};
use geom_test::oracle::geomdl::Compared;
use geom_test::seed::resolve;

fn seed() -> Seed {
    resolve("oracle_families", Seed::DEFAULT).unwrap()
}

fn report(case: &Case) -> Diagnosis {
    Diagnosis {
        id: case.job.id,
        freedom: Some(case.expect.freedom),
        redundant: Some(case.expect.redundant),
        conflicting: Some(case.expect.conflicting),
    }
}

#[test]
fn a_night_of_cases_is_the_same_from_its_seed() {
    assert_eq!(cases(seed(), 30), cases(seed(), 30));
    assert_ne!(cases(seed(), 30), cases(Seed(seed().0 ^ 1), 30));
}

#[test]
fn the_families_turn_in_order_and_expect_what_they_are_built_to() {
    let night = cases(seed(), 120);
    assert_eq!(night.len(), 120);
    for (i, case) in night.iter().enumerate() {
        assert_eq!(case.family, Family::ALL[i % 6]);
        assert_eq!(case.job.id, i as u64);
        let e = case.expect;
        match case.family {
            Family::Chain | Family::Ladder | Family::Growth => {
                assert_eq!(
                    (e.freedom, e.redundant, e.conflicting, e.consistent),
                    (0, 0, 0, true)
                );
            }
            Family::Loose => {
                assert!(e.freedom >= 1 && e.redundant == 0 && e.conflicting == 0 && e.consistent);
            }
            Family::Redundant => assert_eq!((e.freedom, e.redundant, e.conflicting), (0, 1, 0)),
            Family::Conflicting => {
                assert_eq!(
                    (e.freedom, e.redundant, e.conflicting, e.consistent),
                    (0, 0, 1, false)
                );
            }
        }
    }
}

#[test]
fn the_kernel_diagnoses_every_case_as_built() {
    let night = cases(seed(), 600);
    for case in &night {
        check(case).unwrap_or_else(|e| panic!("{e}"));
    }
}

#[test]
fn the_kernel_meets_every_rule_of_a_consistent_case_it_solves() {
    for case in cases(seed(), 300).iter().filter(|c| c.expect.consistent) {
        assert!(
            holds(case).unwrap(),
            "case {} ({})",
            case.job.id,
            case.family.name()
        );
    }
}

#[test]
fn an_oracle_that_reports_the_expected_counts_agrees() {
    for case in cases(seed(), 60) {
        assert_eq!(
            judge("ezpz", &case, &report(&case)).unwrap(),
            Compared::Agree
        );
    }
}

#[test]
fn an_oracle_that_reports_other_counts_differs_and_the_family_says_which() {
    let night = cases(seed(), 60);
    let signature = |case: &Case, d: Diagnosis| match judge("planegcs", case, &d).unwrap() {
        Compared::Differ { signature, .. } => signature,
        Compared::Agree => panic!("case {} agreed", case.job.id),
    };
    for case in &night {
        let mut wrong = report(case);
        wrong.freedom = Some(case.expect.freedom + 1);
        let s = signature(case, wrong);
        assert_eq!(
            (s.operation.as_str(), s.oracle.as_str(), s.kind.as_str()),
            ("diagnose", "planegcs", "oracle")
        );
        assert_eq!(s.family, format!("{}:freedom", case.family.name()));
        let mut wrong = report(case);
        wrong.conflicting = Some(case.expect.conflicting + 1);
        assert_eq!(
            signature(case, wrong).family,
            format!("{}:conflicts", case.family.name())
        );
        let mut wrong = report(case);
        wrong.redundant = Some(case.expect.redundant + 1);
        assert_eq!(
            signature(case, wrong).family,
            format!("{}:redundancy", case.family.name())
        );
        let silent = Diagnosis {
            id: case.job.id,
            freedom: None,
            redundant: None,
            conflicting: None,
        };
        assert_eq!(
            signature(case, silent).family,
            format!("{}:none", case.family.name())
        );
    }
    // A report for another case is refused.
    let case = &night[0];
    let mut other = report(case);
    other.id += 1;
    assert!(judge("ezpz", case, &other).is_err());
}

#[test]
fn the_replay_text_is_the_same_on_a_second_run() {
    let night = cases(seed(), 24);
    assert_eq!(replay_text(&night).unwrap(), replay_text(&night).unwrap());
    assert!(replay_text(&night).unwrap().contains("case 5 conflicting"));
}
