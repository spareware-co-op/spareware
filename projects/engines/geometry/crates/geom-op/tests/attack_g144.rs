//! Piece G144's attack tests on geom-op's half: allow-sets of 13.14's change classes, and the
//! refusal of a change outside one (13.20).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

use geom_op::history::{ChangeClass, LoopChange, TopologyChange, Vanish};
use geom_op::id::NameId;
use geom_op::kind::ErrorKind;
use geom_op::local::{AllowSet, CLASSES, LocalDetail, allowed};
use geom_op::name::NameSet;
use geom_op::remedy::Remedy;

fn name(n: u8) -> NameId {
    NameId::from_bytes([n; 16])
}

#[test]
fn every_class_is_in_all_and_none_is_in_none() {
    for class in CLASSES {
        assert!(AllowSet::ALL.contains(class), "{class:?}");
        assert!(!AllowSet::NONE.contains(class), "{class:?}");
        assert_eq!(
            AllowSet::of(&[class]).classes().collect::<Vec<_>>(),
            [class]
        );
    }
    assert_eq!(AllowSet::ALL.classes().collect::<Vec<_>>(), CLASSES);
    assert_eq!(AllowSet::of(&CLASSES), AllowSet::ALL);
}

#[test]
fn a_caller_can_narrow_a_set_and_never_widen_it() {
    let declared = AllowSet::of(&[ChangeClass::FaceAdded, ChangeClass::FacesMerged]);
    let asked = AllowSet::of(&[ChangeClass::FaceAdded, ChangeClass::BodySplit]);
    let set = declared.narrowed(asked);
    assert_eq!(set, AllowSet::of(&[ChangeClass::FaceAdded]));
    for class in CLASSES {
        assert!(
            !set.contains(class) || declared.contains(class),
            "{class:?}"
        );
    }
    assert_eq!(AllowSet::ALL.narrowed(declared), declared);
    assert_eq!(
        declared.without(ChangeClass::FacesMerged),
        AllowSet::of(&[ChangeClass::FaceAdded])
    );
}

#[test]
fn the_first_change_outside_the_set_is_refused_with_its_class() {
    let changes = [
        TopologyChange::FaceAdded {
            face: name(1),
            role: geom_op::name::Role::new("cap", Vec::new()).unwrap(),
        },
        TopologyChange::FaceVanished {
            face: name(2),
            why: Vanish::Collapsed,
        },
        TopologyChange::LoopChanged {
            face: name(3),
            change: LoopChange::HoleClosed,
        },
        TopologyChange::ClashTrimmed {
            faces: NameSet::new([name(4), name(5)]).unwrap(),
        },
    ];
    let set = AllowSet::of(&[ChangeClass::FaceAdded, ChangeClass::ClashTrimmed]);
    let (detail, change) = allowed(&changes, set).unwrap_err();
    assert_eq!(
        detail,
        LocalDetail::TopologyChangeNotAllowed {
            class: ChangeClass::FaceVanished
        }
    );
    assert_eq!(change, &changes[1]);
    assert_eq!(detail.kind(), ErrorKind::NoSolution);
    assert_eq!(
        detail.remedies(),
        [Remedy::ChangeInput {
            input: "allow",
            to: None
        }]
    );
    assert!(detail.to_string().contains("FaceVanished"));
    assert!(allowed(&changes, AllowSet::ALL).is_ok());
    assert!(allowed(&[], AllowSet::NONE).is_ok());
}
