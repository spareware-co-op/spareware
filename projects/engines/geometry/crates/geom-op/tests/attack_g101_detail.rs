//! Attacks on the text and remedies of `MakeDetail` (piece G101): every kind of make failure
//! says what is wrong in words and names the change that fixes it, which a caller shows and acts
//! on. A detail that said nothing, or offered no remedy, would leave the caller guessing.
//!
//! - **Words**: every detail's `Display` is a sentence that names what it refers to (the input, the
//!   count, the radius, the piece).
//! - **Remedies**: a detail the caller can act on offers exactly the change that fixes it, with the
//!   nearest working value where it is known, and the kinds the caller cannot fix offer a report.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_op::{Bound, Exactness, MakeDetail, Remedy, Value};

/// Every kind of detail, once.
fn all() -> Vec<MakeDetail> {
    vec![
        MakeDetail::ZeroLength { input: "height" },
        MakeDetail::TooFewCorners { corners: 2 },
        MakeDetail::BaseNotLevel,
        MakeDetail::BaseNotConvex,
        MakeDetail::MinorNotBelowMajor { major: 7 },
        MakeDetail::NotAccepted {
            would: Exactness::Exact,
        },
        MakeDetail::KindNotCertified,
        MakeDetail::NotCertifiedYet { piece: "G180" },
        MakeDetail::BodyNotNamed,
        MakeDetail::EmptyPattern,
        MakeDetail::RepeatedKey,
        MakeDetail::SingularTransform,
        MakeDetail::ResultInvalid,
        MakeDetail::BuildFailed,
    ]
}

#[test]
fn every_detail_says_something_and_names_what_it_refers_to() {
    for d in all() {
        let text = d.to_string();
        assert!(text.len() > 10, "{d:?} says {text:?}");
    }
    assert_eq!(
        MakeDetail::ZeroLength { input: "height" }.to_string(),
        "the length height is zero"
    );
    assert_eq!(
        MakeDetail::TooFewCorners { corners: 2 }.to_string(),
        "the base has 2 corners, fewer than three"
    );
    assert_eq!(
        MakeDetail::MinorNotBelowMajor { major: 7 }.to_string(),
        "the minor radius is not below the major radius, 7"
    );
    assert_eq!(
        MakeDetail::NotCertifiedYet { piece: "G180" }.to_string(),
        "the checker cannot certify the transformed body yet; piece G180 widens it"
    );
    assert_eq!(MakeDetail::BodyNotNamed.to_string(), "the body carries no names");
    assert_eq!(
        MakeDetail::BuildFailed.to_string(),
        "the body could not be assembled"
    );
    // No two kinds read alike.
    let mut texts: Vec<String> = all().iter().map(ToString::to_string).collect();
    texts.sort();
    texts.dedup();
    assert_eq!(texts.len(), all().len());
}

#[test]
fn every_detail_offers_a_remedy_and_the_right_one() {
    for d in all() {
        assert!(!d.remedies().is_empty(), "{d:?} offers no remedy");
    }
    assert_eq!(
        MakeDetail::ZeroLength { input: "height" }.remedies(),
        vec![Remedy::ChangeInput {
            input: "height",
            to: Some((Bound::AtLeast, Value::Count(1))),
        }]
    );
    assert_eq!(
        MakeDetail::TooFewCorners { corners: 2 }.remedies(),
        vec![Remedy::ChangeInput {
            input: "base",
            to: Some((Bound::AtLeast, Value::Count(3))),
        }]
    );
    assert_eq!(
        MakeDetail::MinorNotBelowMajor { major: 7 }.remedies(),
        vec![Remedy::ChangeInput {
            input: "minor",
            to: Some((Bound::AtMost, Value::Count(6))),
        }]
    );
    assert_eq!(
        MakeDetail::NotAccepted {
            would: Exactness::Exact
        }
        .remedies(),
        vec![Remedy::Accept {
            class: Exactness::Exact
        }]
    );
    assert_eq!(
        MakeDetail::NotCertifiedYet { piece: "G180" }.remedies(),
        vec![Remedy::AwaitPiece { piece: "G180" }]
    );
    assert_eq!(MakeDetail::ResultInvalid.remedies(), vec![Remedy::Report]);
    assert_eq!(MakeDetail::BuildFailed.remedies(), vec![Remedy::Report]);
}
