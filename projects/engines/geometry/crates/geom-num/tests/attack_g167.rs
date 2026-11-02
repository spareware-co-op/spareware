//! Attacks on G167's predicate: Villarceau's angle of a torus, read exactly on whole-number data.
//!
//! A plane through a torus's centre stands at Villarceau's angle when sin θ = r/R from the
//! equator. For R = 5 and r = 3 the plane through the z axis's foot on (4, 0, 3) and (0, 1, 0) has
//! normal (−3, 0, 4), so |n|²|e|²(R² − r²) = 25·16 = 400 = (n·e)²R²: exactly at the angle; one
//! leaning less or more gives each sign, and a radius past the budget is refused.

#![allow(
    clippy::unwrap_used,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_num::grid::{P3, Steps};
use geom_num::quadric::{Plane, villarceau};
use geom_num::{NumError, Sign};

#[test]
fn villarceau_s_angle_is_read_exactly() {
    let p = |x: i64, y: i64, z: i64| P3::new(x, y, z).unwrap();
    let (o, up) = (p(0, 0, 0), p(0, 0, 1));
    let plane = |c: P3| Plane {
        a: o,
        b: c,
        c: p(0, 1, 0),
    };
    assert_eq!(
        villarceau(&plane(p(4, 0, 3)), o, up, Steps(5), Steps(3)),
        Ok(Sign::Zero)
    );
    // Normal (−1, 0, 4): 17·16 − 16·25 < 0.
    assert_eq!(
        villarceau(&plane(p(4, 0, 1)), o, up, Steps(5), Steps(3)),
        Ok(Sign::Negative)
    );
    // Normal (−3, 0, 1): 10·16 − 1·25 > 0.
    assert_eq!(
        villarceau(&plane(p(1, 0, 3)), o, up, Steps(5), Steps(3)),
        Ok(Sign::Positive)
    );
    assert_eq!(
        villarceau(&plane(p(4, 0, 3)), o, up, Steps(1 << 63), Steps(3)),
        Err(NumError::BudgetExceeded)
    );
}
