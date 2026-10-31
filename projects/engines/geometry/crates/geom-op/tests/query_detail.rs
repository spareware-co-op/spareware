//! A query's typed failures (chapter 22; piece G102): each detail's base kind, its remedy and what
//! it says, in `OpError`'s terms, as a caller reads them. The Make family's details have their full
//! tests with their operations (`geom-make`); this crate holds that each says something.

#![allow(
    clippy::unwrap_used,
    reason = "test code: a failure should stop loudly"
)]

use geom_op::detail::QueryDetail;
use geom_op::{ErrorKind, MakeDetail, OpError, OpErrorDetail, OpId, OpPath, Remedy};

fn error(detail: QueryDetail) -> OpError {
    OpError::new(OpPath::new(OpId::from_bytes([1; 16])), detail)
}

#[test]
fn each_detail_has_its_kind() {
    for (detail, kind) in [
        (QueryDetail::Malformed, ErrorKind::InvalidInput),
        (QueryDetail::NotSolid, ErrorKind::InvalidInput),
        (QueryDetail::CurvedEdge { edge: 1 }, ErrorKind::Unsupported),
        (QueryDetail::CurvedFace { face: 2 }, ErrorKind::Unsupported),
        (QueryDetail::NoLoops { face: 3 }, ErrorKind::Unsupported),
        (QueryDetail::Overflow, ErrorKind::Internal),
    ] {
        assert_eq!(detail.kind(), kind, "{detail:?}");
        assert_eq!(error(detail).kind(), kind, "{detail:?}");
        assert_eq!(*error(detail).detail(), OpErrorDetail::Query(detail));
    }
}

#[test]
fn each_detail_says_what_can_be_done() {
    let change = Remedy::ChangeInput {
        input: "body",
        to: None,
    };
    let wait = Remedy::AwaitPiece { piece: "G177" };
    assert_eq!(
        QueryDetail::Malformed.remedies(),
        std::slice::from_ref(&change)
    );
    assert_eq!(QueryDetail::NotSolid.remedies(), [change]);
    for curved in [
        QueryDetail::CurvedEdge { edge: 1 },
        QueryDetail::CurvedFace { face: 2 },
        QueryDetail::NoLoops { face: 3 },
    ] {
        assert_eq!(curved.remedies(), std::slice::from_ref(&wait));
    }
    assert_eq!(QueryDetail::Overflow.remedies(), [Remedy::Report]);
    assert_eq!(
        OpErrorDetail::Query(QueryDetail::Overflow).remedies(),
        [Remedy::Report]
    );
}

#[test]
fn a_make_detail_says_what_went_wrong_and_what_to_do() {
    let detail = MakeDetail::ZeroLength { input: "radius" };
    assert_eq!(detail.to_string(), "the length radius is zero");
    assert_ne!(detail.remedies(), []);
}

#[test]
fn each_detail_says_what_went_wrong() {
    for (detail, text) in [
        (
            QueryDetail::Malformed,
            "the body's links do not make a body",
        ),
        (
            QueryDetail::CurvedEdge { edge: 7 },
            "edge 7 is not a straight line between two vertices",
        ),
        (
            QueryDetail::CurvedFace { face: 8 },
            "face 8 is not on a plane",
        ),
        (QueryDetail::NoLoops { face: 9 }, "face 9 has no loop"),
        (QueryDetail::NotSolid, "the body is not a solid"),
        (QueryDetail::Overflow, "a sum went past its integer's width"),
    ] {
        assert_eq!(detail.to_string(), text);
        assert_eq!(OpErrorDetail::Query(detail).to_string(), text);
    }
}
