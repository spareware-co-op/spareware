//! Tests that run, through the crate's public interface, the lines of `grid` that no other
//! integration test reached (piece G21, the coverage gate of section 28.8).
//!
//! `P2::new` refuses a coordinate outside the frame's range, as `P3::new` does; the integration
//! tests made only points inside it, so its refusing arm never ran.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "expected values; the rules are for kernel code"
)]

use geom_num::grid::COORD_MAX;
use geom_num::{NumError, P2};

/// `P2::new` reports the first coordinate outside ±(2^40 − 1), x before y, with its value.
#[test]
fn a_2d_point_outside_the_frame_is_refused_naming_the_first_bad_coordinate() {
    let over = COORD_MAX + 1;
    let under = -COORD_MAX - 1;
    let out = |v: i64| {
        Err(NumError::OutOfRange {
            value: i128::from(v),
        })
    };
    assert_eq!(P2::new(over, 0), out(over));
    assert_eq!(P2::new(0, under), out(under));
    assert_eq!(P2::new(under, over), out(under));
    assert_eq!(P2::new(i64::MAX, i64::MIN), out(i64::MAX));
    let edge = P2::new(COORD_MAX, -COORD_MAX);
    assert_eq!(
        edge.map(|p| (p.x.get(), p.y.get())),
        Ok((COORD_MAX, -COORD_MAX))
    );
}
