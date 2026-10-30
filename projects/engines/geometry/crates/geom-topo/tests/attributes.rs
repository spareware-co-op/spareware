//! Attributes through the public interface: values keyed by name, and what each class does when
//! its entity is split or merged (8.4).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::many_single_char_names
)]

use std::collections::BTreeMap;

use geom_topo::attributes::{
    AttrId, Attributes, Carried, Class, Dropped, MergeAction, SplitAction, Value,
};
use geom_topo::order::NameKey;

fn n(b: u8) -> NameKey {
    NameKey([b; 16])
}

fn id(name: &str) -> AttrId {
    AttrId {
        owner: "kit.test".into(),
        name: name.into(),
        version: 1,
    }
}

fn v(b: u8) -> Value {
    Value(vec![b])
}

fn with(class: Class) -> Attributes {
    Attributes::new().define(id("rating"), class).unwrap()
}

#[test]
fn a_value_needs_a_definition_and_a_definition_never_changes() {
    let a = Attributes::new();
    assert!(a.set(&id("rating"), n(1), v(9)).is_none());
    let class = Class {
        split: SplitAction::Propagate,
        merge: MergeAction::Combine,
    };
    let a = a.define(id("rating"), class).unwrap();
    assert!(a.define(id("rating"), class).is_some());
    assert!(
        a.define(
            id("rating"),
            Class {
                split: SplitAction::Delete,
                ..class
            }
        )
        .is_none()
    );
    let a = a.set(&id("rating"), n(1), v(9)).unwrap();
    assert_eq!(a.get(&id("rating"), n(1)), Some(&Carried::One(v(9))));
    assert_eq!(a.remove(&id("rating"), n(1)).get(&id("rating"), n(1)), None);
}

#[test]
fn a_split_propagates_or_deletes() {
    let keep = with(Class {
        split: SplitAction::Propagate,
        merge: MergeAction::Delete,
    });
    let keep = keep
        .set(&id("rating"), n(1), v(9))
        .unwrap()
        .split(n(1), &[n(2), n(3)]);
    assert_eq!(keep.get(&id("rating"), n(1)), None);
    assert_eq!(keep.get(&id("rating"), n(2)), Some(&Carried::One(v(9))));
    assert_eq!(keep.get(&id("rating"), n(3)), Some(&Carried::One(v(9))));
    let drop = with(Class {
        split: SplitAction::Delete,
        merge: MergeAction::Delete,
    });
    let drop = drop
        .set(&id("rating"), n(1), v(9))
        .unwrap()
        .split(n(1), &[n(2)]);
    assert_eq!(drop.iter().count(), 0);
}

#[test]
fn a_merge_keeps_equal_values_combines_or_deletes() {
    let class = Class {
        split: SplitAction::Delete,
        merge: MergeAction::KeepIfEqual,
    };
    let a = with(class)
        .set(&id("rating"), n(1), v(9))
        .unwrap()
        .set(&id("rating"), n(2), v(9))
        .unwrap();
    let (m, dropped) = a.merge(&[n(1), n(2)], n(5));
    assert_eq!(dropped, []);
    assert_eq!(m.get(&id("rating"), n(5)), Some(&Carried::One(v(9))));
    // Different values, or an input without one: dropped, and reported.
    let b = a.set(&id("rating"), n(2), v(8)).unwrap();
    let (m, dropped) = b.merge(&[n(1), n(2)], n(5));
    assert_eq!(m.get(&id("rating"), n(5)), None);
    assert_eq!(
        dropped,
        [Dropped {
            attr: id("rating"),
            into: n(5)
        }]
    );
    let (_, dropped) = a.merge(&[n(1), n(2), n(3)], n(5));
    assert_eq!(dropped.len(), 1);

    let class = Class {
        split: SplitAction::Delete,
        merge: MergeAction::Combine,
    };
    let c = with(class)
        .set(&id("rating"), n(1), v(9))
        .unwrap()
        .set(&id("rating"), n(2), v(8))
        .unwrap();
    let (m, _) = c.merge(&[n(2), n(1)], n(5));
    let each: BTreeMap<NameKey, Value> = [(n(1), v(9)), (n(2), v(8))].into_iter().collect();
    assert_eq!(m.get(&id("rating"), n(5)), Some(&Carried::Combined(each)));
    assert_eq!(m.get(&id("rating"), n(1)), None);

    let class = Class {
        split: SplitAction::Delete,
        merge: MergeAction::Delete,
    };
    let d = with(class).set(&id("rating"), n(1), v(9)).unwrap();
    assert_eq!(d.merge(&[n(1)], n(5)).0.iter().count(), 0);
}

#[test]
fn iteration_is_in_attribute_then_name_order() {
    let class = Class {
        split: SplitAction::Delete,
        merge: MergeAction::Delete,
    };
    let a = Attributes::new()
        .define(id("b"), class)
        .unwrap()
        .define(id("a"), class)
        .unwrap();
    let a = a
        .set(&id("b"), n(1), v(1))
        .unwrap()
        .set(&id("a"), n(3), v(3))
        .unwrap()
        .set(&id("a"), n(2), v(2))
        .unwrap();
    let order: Vec<(String, u8)> = a
        .iter()
        .map(|(at, k, _)| (at.name.clone(), k.0[0]))
        .collect();
    assert_eq!(order, [("a".into(), 2), ("a".into(), 3), ("b".into(), 1)]);
}
