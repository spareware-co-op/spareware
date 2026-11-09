//! Attacks on every claim piece G88 makes about history records and their laws.
//!
//! What this file proves, with `geom-test`'s history kit and fixed seeds:
//!
//! - **Valid histories keep the laws**: every generated step passes laws 1 to 3, and every
//!   generated chain of two to four steps passes law 2 at each step and once composed, law 4's
//!   associativity (composing from the left and from the right agree), and laws 1 and 3 for the
//!   chain's first inputs.
//! - **Breaks are found**: a deleted input that also passes through, and a record naming something
//!   that is not an input, are each reported, on generated histories, by the kit as by the
//!   checker.
//! - **Order never matters**: the same records given in any order make the same history.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test cases; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use geom_op::{History, LawBreak, NameId, NameSet, Origin, Record};
use geom_test::history::{ChainBreak, chain, check_chain, check_laws, step};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

fn config(cases: u32) -> Config {
    Config {
        cases,
        rng_seed: RngSeed::Fixed(0x0000_6788),
        failure_persistence: None,
        ..Config::default()
    }
}

fn inputs(count: u8) -> Vec<NameId> {
    (1..=count).map(|x| NameId::from_bytes([x; 16])).collect()
}

proptest! {
    #![proptest_config(config(256))]

    #[test]
    fn every_generated_step_keeps_the_laws(
        (count, (history, _)) in (1_u8..24).prop_flat_map(|c| (Just(c), step(0, inputs(c)))),
    ) {
        let ins = NameSet::new(inputs(count)).unwrap();
        prop_assert_eq!(check_laws(&history, &ins), Ok(()));
        prop_assert_eq!(check_chain(&[(history, ins)]), Ok(()));
    }

    #[test]
    fn every_generated_chain_keeps_the_laws_and_composes_either_way(
        steps in (1_u8..16, 2_u8..5).prop_flat_map(|(c, len)| chain(inputs(c), len)),
    ) {
        prop_assert_eq!(check_chain(&steps), Ok(()));
    }

    #[test]
    fn a_deleted_input_that_passes_through_is_found(
        (count, (history, _)) in (1_u8..24).prop_flat_map(|c| (Just(c), step(0, inputs(c)))),
    ) {
        let ins = NameSet::new(inputs(count)).unwrap();
        // Delete an input that passes through unchanged, if one does.
        if let Some(kept) = history
            .records()
            .iter()
            .find(|r| r.origin == Origin::Unchanged)
            .map(|r| r.output)
        {
            let mut deleted: Vec<NameId> = history.deleted().names().to_vec();
            deleted.push(kept);
            let broken = History::new(
                history.records().to_vec(),
                NameSet::new(deleted).unwrap(),
                [],
            )
            .unwrap();
            prop_assert_eq!(
                check_laws(&broken, &ins),
                Err(LawBreak::DeletedButModified { input: kept, output: kept })
            );
            let at_first_step = matches!(
                check_chain(&[(broken, ins)]),
                Err(ChainBreak::Step { at: 0, .. })
            );
            prop_assert!(at_first_step);
        }
    }

    #[test]
    fn a_record_naming_a_non_input_is_found(
        (count, (history, _)) in (1_u8..24).prop_flat_map(|c| (Just(c), step(0, inputs(c)))),
    ) {
        let ins = NameSet::new(inputs(count)).unwrap();
        let stranger = NameId::from_bytes([0xEE; 16]);
        let mut records = history.records().to_vec();
        records.push(Record {
            output: NameId::from_bytes([0xEF; 16]),
            origin: Origin::Modified { from: NameSet::new([stranger]).unwrap() },
        });
        let broken = History::new(records, history.deleted().clone(), []).unwrap();
        prop_assert_eq!(check_laws(&broken, &ins), Err(LawBreak::NotAnInput { name: stranger }));
    }

    #[test]
    fn the_order_records_are_given_in_never_matters(
        (history, _) in step(0, inputs(12)),
        seed in any::<u64>(),
    ) {
        let mut records = history.records().to_vec();
        // A seeded rotation and reversal of the records.
        let shift = (seed % (records.len().max(1) as u64)) as usize;
        records.rotate_left(shift);
        if seed % 2 == 1 {
            records.reverse();
        }
        let again = History::new(records, history.deleted().clone(), []).unwrap();
        prop_assert_eq!(again, history);
    }
}

#[test]
fn a_chain_break_says_where() {
    let id = |x: u8| NameId::from_bytes([x; 16]);
    let law = LawBreak::NotAnInput { name: id(3) };
    assert_eq!(
        ChainBreak::Step {
            at: 2,
            broken: law.clone()
        }
        .to_string(),
        format!("step 2: {law}")
    );
    assert_eq!(
        ChainBreak::Composed(law.clone()).to_string(),
        format!("the composed chain: {law}")
    );
    assert_eq!(
        ChainBreak::Groupings.to_string(),
        "law 4: composing from the left and from the right differ"
    );
    assert_eq!(
        ChainBreak::GeneratedAndModified {
            at: Some(1),
            input: id(1),
            output: id(2)
        }
        .to_string(),
        format!(
            "step 1: law 2: {} is both generated and modified from {}",
            id(2),
            id(1)
        )
    );
    assert_eq!(
        ChainBreak::GeneratedAndModified {
            at: None,
            input: id(1),
            output: id(2)
        }
        .to_string(),
        format!(
            "the composed chain: law 2: {} is both generated and modified from {}",
            id(2),
            id(1)
        )
    );
    // No steps: nothing to break.
    assert_eq!(check_chain(&[]), Ok(()));
}
