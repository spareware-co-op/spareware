//! Every failure of the Combine family has its kind, its words and its remedies (G130, 11.11).

#![allow(clippy::panic, reason = "tests: a failure stops the test")]

use geom_num::{Exactness, GridStep, Steps};
use geom_op::{CombineDetail, ErrorKind, OpErrorDetail, Remedy};

#[test]
fn every_combine_detail_says_what_went_wrong() {
    let would = Exactness::Certified {
        bound: Steps(4),
        step: GridStep::Q0,
    };
    for (detail, kind, text) in [
        (
            CombineDetail::DuplicateOperand,
            ErrorKind::InvalidInput,
            "two operands share a key",
        ),
        (
            CombineDetail::UnknownOperand,
            ErrorKind::InvalidInput,
            "a selection names no solid operand",
        ),
        (
            CombineDetail::BadSelection,
            ErrorKind::InvalidInput,
            "a test on winding numbers leaves no answer",
        ),
        (
            CombineDetail::InvalidOperand,
            ErrorKind::InvalidInput,
            "an operand cannot enter the arrangement",
        ),
        (
            CombineDetail::NotPlaneBased,
            ErrorKind::Unsupported,
            "an operand has a face the flat path cannot take",
        ),
        (
            CombineDetail::UnboundedResult,
            ErrorKind::InvalidInput,
            "the selection would include the outside",
        ),
        (
            CombineDetail::SeedOnBoundary,
            ErrorKind::InvalidInput,
            "a seed lies between two pieces",
        ),
        (
            CombineDetail::SeedOutside,
            ErrorKind::InvalidInput,
            "a seed lies on no piece",
        ),
        (
            CombineDetail::ResultInvalid,
            ErrorKind::Internal,
            "a rebuilt body failed the checker",
        ),
    ] {
        let whole = OpErrorDetail::from(detail);
        assert_eq!(detail.kind(), kind);
        assert_eq!(whole.kind(), kind);
        assert_eq!(whole.to_string(), text);
    }
    let refused = OpErrorDetail::from(CombineDetail::NotAccepted { would });
    assert_eq!(refused.kind(), ErrorKind::ExactnessNotAccepted);
    assert!(refused.to_string().contains("does not accept"));
}

#[test]
fn every_combine_detail_offers_what_it_can() {
    let change = |input| Remedy::ChangeInput { input, to: None };
    for detail in [
        CombineDetail::DuplicateOperand,
        CombineDetail::UnknownOperand,
        CombineDetail::InvalidOperand,
    ] {
        assert_eq!(detail.remedies(), [change("operands")]);
    }
    for detail in [
        CombineDetail::BadSelection,
        CombineDetail::UnboundedResult,
        CombineDetail::SeedOnBoundary,
        CombineDetail::SeedOutside,
    ] {
        assert_eq!(detail.remedies(), [change("select")]);
    }
    assert_eq!(
        CombineDetail::NotPlaneBased.remedies(),
        [Remedy::AwaitPiece { piece: "G242" }]
    );
    assert_eq!(CombineDetail::ResultInvalid.remedies(), []);
    let would = Exactness::Exact;
    assert_eq!(
        CombineDetail::NotAccepted { would }.remedies(),
        [Remedy::Accept { class: would }]
    );
}
