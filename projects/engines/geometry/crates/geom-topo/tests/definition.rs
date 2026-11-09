//! Attribute definitions through the public interface (piece G90): what a definition refuses,
//! that a definition never changes, and the actions it gives each event.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_topo::attributes::{AttrId, Class, MergeAction, SplitAction};
use geom_topo::definition::{
    Behaviour, ChangeAction, CopyAction, Definition, DefinitionError, Definitions, EntityKind,
    OnTransform, TransformAction, TransformKind, ValueSchema,
};

fn attr(name: &str, version: u32) -> AttrId {
    AttrId {
        owner: "kit.geom".to_owned(),
        name: name.to_owned(),
        version,
    }
}

fn behaviour(transform: TransformAction) -> Behaviour {
    Behaviour {
        changed: ChangeAction::Keep,
        transformed: OnTransform::all(transform),
        split: SplitAction::Propagate,
        merged: MergeAction::Combine,
        copied: CopyAction::Copy,
    }
}

fn opaque() -> ValueSchema {
    ValueSchema::Opaque {
        id: "acme/1".to_owned(),
    }
}

#[test]
fn a_definition_holds_what_it_was_given() {
    let b = behaviour(TransformAction::Transform);
    let d = Definition::new(
        attr("datum", 1),
        [EntityKind::Vertex, EntityKind::Face, EntityKind::Face],
        ValueSchema::Point,
        b,
    )
    .unwrap();
    assert_eq!(d.attr(), &attr("datum", 1));
    assert_eq!(d.schema(), &ValueSchema::Point);
    assert_eq!(d.behaviour(), &b);
    assert_eq!(
        d.kinds().collect::<Vec<_>>(),
        [EntityKind::Face, EntityKind::Vertex]
    );
    for kind in [EntityKind::Body, EntityKind::Region, EntityKind::Edge] {
        assert!(!d.allows(kind));
    }
    assert!(d.allows(EntityKind::Face) && d.allows(EntityKind::Vertex));
    assert_eq!(
        b.class(),
        Class {
            split: SplitAction::Propagate,
            merge: MergeAction::Combine
        }
    );
}

#[test]
fn only_movable_schemas_move_and_something_must_carry_it() {
    for schema in [
        ValueSchema::Point,
        ValueSchema::Direction,
        ValueSchema::Frame,
        ValueSchema::Length,
        ValueSchema::Field,
    ] {
        assert!(schema.movable());
        assert!(
            Definition::new(
                attr("x", 1),
                [EntityKind::Body],
                schema,
                behaviour(TransformAction::Transform)
            )
            .is_ok()
        );
    }
    assert!(!opaque().movable());
    for kind in TransformKind::ALL {
        let mut b = behaviour(TransformAction::Keep);
        match kind {
            TransformKind::Move => b.transformed.moved = TransformAction::Transform,
            TransformKind::Rotate => b.transformed.rotated = TransformAction::Transform,
            TransformKind::Mirror => b.transformed.mirrored = TransformAction::Transform,
            TransformKind::Scale => b.transformed.scaled = TransformAction::Transform,
        }
        assert_eq!(b.transformed.get(kind), TransformAction::Transform);
        assert_eq!(
            Definition::new(attr("x", 1), [EntityKind::Face], opaque(), b),
            Err(DefinitionError::CannotTransform(kind))
        );
    }
    // Keep and delete are fine for opaque values.
    for action in [TransformAction::Keep, TransformAction::Delete] {
        assert!(
            Definition::new(
                attr("x", 1),
                [EntityKind::Face],
                opaque(),
                behaviour(action)
            )
            .is_ok()
        );
    }
    assert_eq!(
        Definition::new(
            attr("x", 1),
            [],
            ValueSchema::Point,
            behaviour(TransformAction::Keep)
        ),
        Err(DefinitionError::NoKinds)
    );
}

#[test]
fn a_definition_never_changes() {
    let one = Definition::new(
        attr("rating", 1),
        [EntityKind::Face],
        opaque(),
        behaviour(TransformAction::Keep),
    )
    .unwrap();
    let defs = Definitions::new().define(one.clone()).unwrap();
    assert_eq!(defs.get(&attr("rating", 1)), Some(&one));
    assert_eq!(defs.get(&attr("rating", 2)), None);
    // The same definition again changes nothing.
    assert_eq!(defs.define(one.clone()).unwrap(), defs);
    // Any difference under the same identity is refused.
    let other = Definition::new(
        attr("rating", 1),
        [EntityKind::Edge],
        opaque(),
        behaviour(TransformAction::Keep),
    )
    .unwrap();
    assert_eq!(
        defs.define(other),
        Err(DefinitionError::Redefined(attr("rating", 1)))
    );
    // A new version is a new definition, and they iterate in identity order.
    let two = Definition::new(
        attr("rating", 2),
        [EntityKind::Edge],
        opaque(),
        behaviour(TransformAction::Delete),
    )
    .unwrap();
    let a = Definition::new(
        attr("a", 9),
        [EntityKind::Body],
        opaque(),
        behaviour(TransformAction::Delete),
    )
    .unwrap();
    let defs = defs.define(two.clone()).unwrap().define(a.clone()).unwrap();
    let order: Vec<&Definition> = defs.iter().collect();
    assert_eq!(order, [&a, &one, &two]);
    assert_eq!(Definitions::default(), Definitions::new());
}

#[test]
fn errors_and_kinds_print_what_they_are() {
    assert_eq!(
        DefinitionError::NoKinds.to_string(),
        "no kind of entity may carry the attribute"
    );
    assert_eq!(
        DefinitionError::CannotTransform(TransformKind::Mirror).to_string(),
        "the attribute's values are opaque, so it cannot move them under a mirror"
    );
    assert_eq!(
        DefinitionError::Redefined(attr("rating", 3)).to_string(),
        "kit.geom/rating version 3 is already defined otherwise; a new behaviour is a new version"
    );
    let names: Vec<String> = TransformKind::ALL.iter().map(ToString::to_string).collect();
    assert_eq!(names, ["move", "rotate", "mirror", "scale"]);
}

#[test]
fn attributes_rebuild_with_their_classes_and_take_a_checked_class() {
    use geom_topo::attributes::{Attributes, Carried, Value};
    use geom_topo::order::NameKey;
    let a = attr("rating", 1);
    let b = behaviour(TransformAction::Keep);
    let held = Attributes::new().define(a.clone(), b.class()).unwrap();
    let other = attr("missing", 1);
    let rebuilt = held.rebuilt([
        (a.clone(), NameKey([1; 16]), Carried::One(Value(vec![1]))),
        (
            other.clone(),
            NameKey([2; 16]),
            Carried::One(Value(vec![2])),
        ),
    ]);
    // A value whose attribute has no class is left out; the classes stay.
    assert_eq!(rebuilt.class(&a), Some(b.class()));
    assert_eq!(rebuilt.class(&other), None);
    let all: Vec<_> = rebuilt.iter().map(|(x, n, _)| (x.clone(), *n)).collect();
    assert_eq!(all, [(a.clone(), NameKey([1; 16]))]);
    // A class set by `defined_as` replaces whatever was held.
    let mut c = b.class();
    c.split = SplitAction::Delete;
    let changed = held.defined_as(a.clone(), c);
    assert_eq!(changed.class(&a), Some(c));
    assert_eq!(held.class(&a), Some(b.class()));
    assert_eq!(
        Attributes::new().defined_as(a.clone(), c).class(&a),
        Some(c)
    );
}
