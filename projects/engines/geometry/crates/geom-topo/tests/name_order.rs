//! Name order through the public interface: whatever order an operation produced an arena's rows
//! in, the rows published are in name order, and the same (8.2, 32.17's shuffle test).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

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

    #[test]
    fn the_permutation_and_its_inverse_agree(names in keys()) {
        let p = Permutation::by_name(&names).unwrap();
        for old in 0..p.len() {
            prop_assert_eq!(p.old_of(p.new_of(old).unwrap()), Some(old));
        }
    }
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
