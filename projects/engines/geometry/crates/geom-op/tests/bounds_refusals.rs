//! Per-entity error bounds (G143, 13.2, 32.21): the refusals the module's words promise, worked by
//! hand. A sum past the largest word is past any ceiling, the largest word's too (#1170), and a
//! refusal names the first entity at fault in name order, whatever kinds of fault the operation
//! has (#1171).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    reason = "test cases; the rules are for kernel code"
)]

use geom_num::ladder::PER_OPERATION;
use geom_num::{GridStep, Steps};
use geom_op::bounds::{Bounds, Limits};
use geom_op::detail::{BoundDetail, OpErrorDetail};
use geom_op::{History, NameId, NameSet, OpId, OpPath, Origin, Record, Role};

fn name(x: u8) -> NameId {
    NameId::from_bytes([x; 16])
}

fn set(xs: &[u8]) -> NameSet {
    NameSet::new(xs.iter().map(|x| name(*x))).unwrap()
}

fn step(x: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([x; 16]))
}

fn history(records: Vec<Record>) -> History {
    History::new(records, set(&[]), []).unwrap()
}

fn generated(output: u8) -> Record {
    Record {
        output: name(output),
        origin: Origin::Generated {
            from: set(&[]),
            role: Role::new("fillet", []).unwrap(),
        },
    }
}

fn merged(output: u8, of: &[u8]) -> Record {
    Record {
        output: name(output),
        origin: Origin::Merged { of: set(of) },
    }
}

/// The largest word, as own limit and as ceiling.
const WIDEST: Limits = Limits {
    own: Steps(u64::MAX),
    total: Steps(u64::MAX),
};

/// Faces 1 and 2 generated with bounds of 16 and `top`, under [`WIDEST`].
fn two_faces(top: u64) -> Bounds {
    Bounds::new(GridStep::Q0)
        .after(
            &step(10),
            &history(vec![generated(1), generated(2)]),
            &[(name(1), Steps(16)), (name(2), Steps(top))],
            WIDEST,
        )
        .unwrap()
}

#[test]
fn a_sum_past_the_largest_word_is_past_a_ceiling_of_the_largest_word() {
    // 16 + (2^64 - 4) = 2^64 + 12: past the largest word, so past any ceiling.
    let faces = two_faces(u64::MAX - 3);
    let error = faces
        .after(&step(11), &history(vec![merged(3, &[1, 2])]), &[], WIDEST)
        .unwrap_err();
    assert_eq!(error.entities(), [name(3)]);
    assert_eq!(
        *error.detail(),
        OpErrorDetail::Bound(BoundDetail::OverCeiling {
            needed: Steps(u64::MAX),
            ceiling: Steps(u64::MAX),
            step: GridStep::Q0,
        })
    );
    // An own term that carries the sum past the word is refused the same way.
    let error = faces
        .after(
            &step(11),
            &history(vec![Record {
                output: name(3),
                origin: Origin::Modified { from: set(&[2]) },
            }]),
            &[(name(3), Steps(4))],
            WIDEST,
        )
        .unwrap_err();
    assert_eq!(error.entities(), [name(3)]);
    assert!(matches!(
        error.detail(),
        OpErrorDetail::Bound(BoundDetail::OverCeiling { .. })
    ));
}

#[test]
fn a_sum_of_exactly_the_largest_word_is_within_a_ceiling_of_it() {
    // 16 + (2^64 - 17) = 2^64 - 1: the largest word, not past it.
    let faces = two_faces(u64::MAX - 16);
    let got = faces
        .after(&step(11), &history(vec![merged(3, &[1, 2])]), &[], WIDEST)
        .unwrap();
    assert_eq!(got.of(name(3)), Steps(u64::MAX));
    assert_eq!(got.terms(name(3)).len(), 2);
}

#[test]
fn a_term_on_no_output_before_an_output_at_fault_is_the_one_named() {
    // Entity 1 is no output yet has a term; output 2's term is past the step's limit.
    let error = Bounds::new(GridStep::Q0)
        .after(
            &step(10),
            &history(vec![generated(2)]),
            &[(name(2), Steps(PER_OPERATION.0 + 1)), (name(1), Steps(1))],
            Limits::DEFAULT,
        )
        .unwrap_err();
    assert_eq!(error.entities(), [name(1)]);
    assert_eq!(
        *error.detail(),
        OpErrorDetail::Bound(BoundDetail::OwnTermAstray)
    );
}

#[test]
fn an_output_at_fault_before_an_entity_named_twice_is_the_one_named() {
    // Output 1's term is past the step's limit; entity 2 is named twice.
    let error = Bounds::new(GridStep::Q0)
        .after(
            &step(10),
            &history(vec![generated(1), generated(2)]),
            &[
                (name(2), Steps(1)),
                (name(1), Steps(PER_OPERATION.0 + 1)),
                (name(2), Steps(2)),
            ],
            Limits::DEFAULT,
        )
        .unwrap_err();
    assert_eq!(error.entities(), [name(1)]);
    assert_eq!(
        *error.detail(),
        OpErrorDetail::Bound(BoundDetail::OverStep {
            own: Steps(PER_OPERATION.0 + 1),
            limit: PER_OPERATION,
            step: GridStep::Q0,
        })
    );
}

#[test]
fn a_term_on_no_output_before_an_entity_named_twice_is_the_one_named() {
    // Entity 1 is no output yet has a term; output 2 is named twice.
    let error = Bounds::new(GridStep::Q0)
        .after(
            &step(10),
            &history(vec![generated(2)]),
            &[
                (name(2), Steps(1)),
                (name(2), Steps(2)),
                (name(1), Steps(1)),
            ],
            Limits::DEFAULT,
        )
        .unwrap_err();
    assert_eq!(error.entities(), [name(1)]);
    assert_eq!(
        *error.detail(),
        OpErrorDetail::Bound(BoundDetail::OwnTermAstray)
    );
}
