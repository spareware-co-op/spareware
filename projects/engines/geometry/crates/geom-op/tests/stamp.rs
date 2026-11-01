//! A `Stamp` writes what a run at the real path on the real names writes, for every kind of name,
//! and refuses a name it cannot fill in (G105, 23.8).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test"
)]

use geom_op::{
    BLANK_OP, EdgeOrVertex, NameError, NameId, NameList, NameSet, OpId, OpPath, Ordinal, PieceKey,
    Role, RoleArg, Side, Stamp, TopoName,
};

/// One name of every kind, each after the names it refers to, as a run at `path` on `inputs`
/// makes them.
fn run(path: &OpPath, inputs: &[NameId; 3]) -> Vec<TopoName> {
    let created = TopoName::Created {
        op: path.step("cut").unwrap(),
        role: Role::new("side", [RoleArg::Name(inputs[0]), RoleArg::Count(3)]).unwrap(),
        from: NameList::new([inputs[1], inputs[0]]),
    };
    let split = TopoName::Split {
        op: path.clone(),
        of: created.id().unwrap(),
        piece: PieceKey::ByTools {
            tools: NameSet::new([inputs[2], created.id().unwrap()]).unwrap(),
            side: Side::Front,
        },
    };
    let merged = TopoName::Merged {
        op: path.item([9; 16]),
        of: NameSet::new([created.id().unwrap(), split.id().unwrap()]).unwrap(),
    };
    let bounded = TopoName::Bounded {
        kind: EdgeOrVertex::Edge,
        by: NameSet::new([split.id().unwrap(), merged.id().unwrap()]).unwrap(),
        ends: NameSet::new([created.id().unwrap()]).unwrap(),
    };
    let weak = TopoName::Weak {
        base: bounded.id().unwrap(),
        ord: Ordinal::new(1, 2).unwrap(),
    };
    vec![created, split, merged, bounded, weak]
}

fn blanks() -> [NameId; 3] {
    [
        Stamp::blank_input(0).unwrap(),
        Stamp::blank_input(1).unwrap(),
        Stamp::blank_input(2).unwrap(),
    ]
}

fn real_inputs() -> [NameId; 3] {
    [
        NameId::from_bytes([1; 16]),
        NameId::from_bytes([2; 16]),
        NameId::from_bytes([3; 16]),
    ]
}

fn real_path() -> OpPath {
    OpPath::new(OpId::from_bytes([7; 16]))
        .step("outer")
        .unwrap()
}

fn stamp() -> Stamp {
    Stamp::new(&real_path(), &real_inputs()).unwrap()
}

#[test]
fn every_kind_of_name_stamps_to_what_the_run_would_make() {
    let blank = run(&OpPath::new(BLANK_OP), &blanks());
    let real = run(&real_path(), &real_inputs());
    let mut s = stamp();
    for (b, r) in blank.iter().zip(&real) {
        assert_eq!(s.name(b).unwrap(), *r);
        assert_eq!(s.id(b.id().unwrap()).unwrap(), r.id().unwrap());
    }
}

#[test]
fn an_input_stands_for_its_own_name() {
    let s = stamp();
    for (blank, real) in blanks().iter().zip(real_inputs()) {
        assert_eq!(s.id(*blank).unwrap(), real);
    }
}

#[test]
fn blanks_differ_and_are_no_names() {
    let [a, b, c] = blanks();
    assert_ne!(a, b);
    assert_ne!(b, c);
    let path = OpPath::new(BLANK_OP);
    assert!(run(&path, &blanks()).iter().all(|n| n.id().unwrap() != a));
    // `[5, 0]` under the name scheme's prefix, written by hand.
    assert_eq!(a, geom_op::name::name_id(&[0x82, 0x05, 0x00]));
    assert_eq!(b, geom_op::name::name_id(&[0x82, 0x05, 0x01]));
}

#[test]
fn a_path_that_is_not_blank_is_refused() {
    let names = run(&real_path(), &blanks());
    assert_eq!(stamp().name(&names[0]), Err(NameError::NotBlank));
    assert_eq!(stamp().name(&names[1]), Err(NameError::NotBlank));
    assert_eq!(stamp().name(&names[2]), Err(NameError::NotBlank));
}

#[test]
fn a_name_not_stamped_yet_is_refused() {
    let blank = run(&OpPath::new(BLANK_OP), &blanks());
    for later in &blank[1..] {
        assert_eq!(stamp().name(later), Err(NameError::NotBlank));
    }
    let foreign = NameId::from_bytes([9; 16]);
    assert_eq!(stamp().id(foreign), Err(NameError::NotBlank));
    let in_role = TopoName::Created {
        op: OpPath::new(BLANK_OP),
        role: Role::new("side", [RoleArg::Name(foreign)]).unwrap(),
        from: NameList::default(),
    };
    assert_eq!(stamp().name(&in_role), Err(NameError::NotBlank));
    let in_from = TopoName::Created {
        op: OpPath::new(BLANK_OP),
        role: Role::new("side", []).unwrap(),
        from: NameList::new([foreign]),
    };
    assert_eq!(stamp().name(&in_from), Err(NameError::NotBlank));
    let in_set = TopoName::Merged {
        op: OpPath::new(BLANK_OP),
        of: NameSet::new([foreign]).unwrap(),
    };
    assert_eq!(stamp().name(&in_set), Err(NameError::NotBlank));
}

/// Two blanks of a set that stand for one name would give a set no run can make.
#[test]
fn a_set_whose_blanks_stand_for_one_name_is_refused() {
    let [a, b, _] = blanks();
    let same = NameId::from_bytes([1; 16]);
    let mut s = Stamp::new(&real_path(), &[same, same, same]).unwrap();
    let merged = TopoName::Merged {
        op: OpPath::new(BLANK_OP),
        of: NameSet::new([a, b]).unwrap(),
    };
    assert_eq!(s.name(&merged), Err(NameError::Repeated));
}

#[test]
fn a_stamp_error_says_what_was_wrong() {
    assert_eq!(
        NameError::NotBlank.to_string(),
        "a name to stamp holds something not blank"
    );
}
