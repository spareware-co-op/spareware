//! A family turned about its own z axis by whole quarter turns, so that it can be built standing in
//! a host that runs along any of the plan's four axis ways (piece A64, job 6.4).
//!
//! The kernel's `Engine` interface has no way to move or turn a solid once it is made, so a family
//! that is to stand turned is built turned: every corner of every extrusion, and the vector it is
//! extruded along, is turned before the engine is asked. A quarter turn of a grid point is a grid
//! point (two coordinates swap, one changes sign), so nothing is rounded, and the turned family is
//! exactly the family, turned. Turns are anticlockwise seen from above (from +z looking down), as
//! the plan's angles run.

use super::error::Reason;
use engine_geometry::value::grid::P3;

/// How far a family is turned about its z axis, anticlockwise seen from above.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Turn {
    /// Not turned: its x along the plan's x.
    #[default]
    None,
    /// A quarter turn: its x along the plan's y.
    Quarter,
    /// A half turn: its x along the plan's -x.
    Half,
    /// Three quarters: its x along the plan's -y.
    ThreeQuarters,
}

impl Turn {
    /// Every turn, from none.
    pub const ALL: [Turn; 4] = [Turn::None, Turn::Quarter, Turn::Half, Turn::ThreeQuarters];

    /// How many quarter turns it is: 0 to 3.
    #[must_use]
    pub const fn quarters(self) -> u8 {
        match self {
            Turn::None => 0,
            Turn::Quarter => 1,
            Turn::Half => 2,
            Turn::ThreeQuarters => 3,
        }
    }

    /// The turn of `quarters` quarter turns, if it is 0 to 3.
    #[must_use]
    pub const fn of_quarters(quarters: u8) -> Option<Turn> {
        match quarters {
            0 => Some(Turn::None),
            1 => Some(Turn::Quarter),
            2 => Some(Turn::Half),
            3 => Some(Turn::ThreeQuarters),
            _ => None,
        }
    }

    /// The turn that takes the family's x axis to the plan way `[x, y]`, if that is a unit vector
    /// along an axis (`[1, 0]`, `[0, 1]`, `[-1, 0]` or `[0, -1]`).
    #[must_use]
    pub const fn taking_x_to(way: [i64; 2]) -> Option<Turn> {
        match way {
            [1, 0] => Some(Turn::None),
            [0, 1] => Some(Turn::Quarter),
            [-1, 0] => Some(Turn::Half),
            [0, -1] => Some(Turn::ThreeQuarters),
            _ => None,
        }
    }

    /// A plan point or vector `[x, y]` turned, exactly; `None` only where a coordinate is
    /// `i64::MIN`, which has no negative.
    #[must_use]
    pub fn plan(self, [x, y]: [i64; 2]) -> Option<[i64; 2]> {
        Some(match self {
            Turn::None => [x, y],
            Turn::Quarter => [y.checked_neg()?, x],
            Turn::Half => [x.checked_neg()?, y.checked_neg()?],
            Turn::ThreeQuarters => [y, x.checked_neg()?],
        })
    }

    /// A vector of whole steps turned about z.
    pub(crate) fn vector(self, [x, y, z]: [i64; 3]) -> Result<[i64; 3], Reason> {
        let [x, y] = self.plan([x, y]).ok_or(Reason::OutOfRange)?;
        Ok([x, y, z])
    }

    /// A grid point turned about the family's z axis.
    pub(crate) fn point(self, p: P3) -> Result<P3, Reason> {
        if self == Turn::None {
            return Ok(p);
        }
        let [x, y, z] = self.vector([p.x.get(), p.y.get(), p.z.get()])?;
        P3::new(x, y, z).map_err(|_| Reason::OutOfRange)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each turn takes x where its name says, four quarter turns are none, and a point keeps its
    /// height.
    #[test]
    fn a_quarter_turn_swaps_two_coordinates_and_changes_one_sign() {
        for turn in Turn::ALL {
            let way = turn.plan([1, 0]);
            assert_eq!(way.and_then(Turn::taking_x_to), Some(turn));
            assert_eq!(Turn::of_quarters(turn.quarters()), Some(turn));
        }
        assert_eq!(Turn::Quarter.plan([3, 5]), Some([-5, 3]));
        assert_eq!(Turn::Half.plan([3, 5]), Some([-3, -5]));
        assert_eq!(Turn::ThreeQuarters.plan([3, 5]), Some([5, -3]));
        let mut p = [7, -2];
        for _ in 0..4 {
            p = Turn::Quarter.plan(p).unwrap_or_default();
        }
        assert_eq!(p, [7, -2]);
        assert_eq!(Turn::of_quarters(4), None);
        assert_eq!(Turn::taking_x_to([1, 1]), None);
        assert_eq!(Turn::Half.plan([i64::MIN, 0]), None);
        assert_eq!(Turn::Quarter.vector([1, 2, 3]), Ok([-2, 1, 3]));
    }
}
