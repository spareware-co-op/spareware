//! Name order through the public interface: whatever order an operation produced an arena's rows
//! in, the rows published are in name order, and the same (8.2, 32.17's shuffle test).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

#[macro_use]
mod slices;

use geom_topo::order::{NameKey, OrderError, Permutation};
use proptest::prelude::*;

fn keys() -> impl Strategy<Value = Vec<NameKey>> {
    proptest::collection::btree_set(any::<[u8; 16]>(), 0..64)
        .prop_map(|s| s.into_iter().map(NameKey).collect())
}

proptest! {
    #[test]
    fn any_order_of_the_same_names_publishes_the_same_rows(names in keys(), seed in any::<u64>()) {
        let mut shuffled = names.clone();
        // A Fisher-Yates shuffle from the seed, so the case replays.
        let mut s = seed | 1;
        for i in (1..shuffled.len()).rev() {
            s ^= s << 13; s ^= s >> 7; s ^= s << 17;
            shuffled.swap(i, (s % (i as u64 + 1)) as usize);
        }
        let a = Permutation::by_name(&names).unwrap().apply(&names).unwrap();
        let b = Permutation::by_name(&shuffled).unwrap().apply(&shuffled).unwrap();
        prop_assert_eq!(&a, &b);
        prop_assert!(a.windows(2).all(|w| w[0] < w[1]));
    }
}

// The permutation and its inverse agree: in quarters of its one stream of cases (decision 32.78,
// `slices`).
sliced!(
    the_permutation_and_its_inverse_agree,
    ProptestConfig::default(),
    (keys(),),
    |(names,)| {
        let p = Permutation::by_name(&names).unwrap();
        for old in 0..p.len() {
            prop_assert_eq!(p.old_of(p.new_of(old).unwrap()), Some(old));
        }
        Ok(())
    }
);

/// The slices' shares add up exactly to the whole's count, each once. A property of a random
/// seed draws new cases on every run, natively and under Miri alike, so its cases are not a fixed
/// set: each slice draws its own share of fresh cases from the same strategy and `Config`, never
/// fewer (the main session's ruling of 19 Nov 2026). A property of a fixed seed, and a list of
/// cases, is one stream cut into these ranges, nothing missing or repeated.
#[test]
fn the_slices_join_to_every_case_once() {
    slices::assert_joins(ProptestConfig::default().cases, "a property's cases");
}

#[test]
fn a_repeated_name_is_refused_not_ordered() {
    let a = NameKey([7; 16]);
    let b = NameKey([1; 16]);
    assert_eq!(
        Permutation::by_name(&[a, b, a]),
        Err(OrderError::DuplicateName {
            first: 0,
            second: 2
        })
    );
}

#[test]
fn a_permutation_of_no_rows_is_empty() {
    assert!(Permutation::by_name(&[]).unwrap().is_empty());
    let one = Permutation::by_name(&[NameKey([3; 16])]).unwrap();
    assert!(!one.is_empty());
    assert_eq!(one.len(), 1);
}
