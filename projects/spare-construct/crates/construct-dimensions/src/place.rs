//! Where a person puts a dimension: the layout a click means, from where its references are now.
//!
//! A person makes an angular, radial, diameter or arc length dimension by picking what it
//! measures and then clicking where it goes, and moves one the same way. [`placed`] turns that
//! place into the layout that is stored, so that the stored data is what the person meant and
//! stays so when the references move: for an angle, which of the four angles between the two
//! lines holds the place (the rays) and its distance from where they meet (the arc's radius); for
//! a radius or a diameter, the direction from the curve's centre to the place; for an arc length,
//! how far outside the arc the place is. Lengths are the nearest whole step to the true distance
//! (a half up), worked in whole numbers.
//!
//! A linear or ordinate dimension's line is given as it is (its y, x, or the point it goes
//! through), so it needs nothing here.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "big integers and fractions cannot overflow; every conversion back is checked"
)]

use crate::error::DimensionError;
use crate::exact::{Frac, round_root};
use crate::layout::{Direction, Layout, Ray};
use crate::locate::Locus;
use crate::measure::steps;
use construct_grids::Point;
use construct_params::Length;
use num_bigint::BigInt;

/// A layout worked out from a place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Placing {
    /// An angle between the first two references' lines.
    Angular,
    /// The first reference's radius.
    Radial,
    /// The first reference's diameter.
    Diameter,
    /// The first reference's arc length.
    ArcLength,
}

impl Placing {
    /// The layout key it makes.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Angular => "angular",
            Self::Radial => "radial",
            Self::Diameter => "diameter",
            Self::ArcLength => "arc_length",
        }
    }

    /// The placing a layout key names, for the layouts placed this way.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        [Self::Angular, Self::Radial, Self::Diameter, Self::ArcLength]
            .into_iter()
            .find(|placing| placing.key() == key)
    }

    /// The placing of a layout that is placed this way.
    #[must_use]
    pub fn of(layout: &Layout) -> Option<Self> {
        match layout {
            Layout::Angular { .. } => Some(Self::Angular),
            Layout::Radial { .. } => Some(Self::Radial),
            Layout::Diameter { .. } => Some(Self::Diameter),
            Layout::ArcLength { .. } => Some(Self::ArcLength),
            _ => None,
        }
    }
}

fn big(value: i64) -> BigInt {
    BigInt::from(value)
}

fn frac(value: i64) -> Frac {
    Frac::from_integer(big(value))
}

fn cannot(why: &str) -> DimensionError {
    DimensionError::CannotPlace(why.to_owned())
}

/// The nearest whole step to the length of `(dx, dy)`, given as exact fractions.
fn length_of(dx: &Frac, dy: &Frac) -> Result<i64, DimensionError> {
    let square = dx * dx + dy * dy;
    round_root(&square)
        .map(|(steps, _)| steps)
        .ok_or_else(|| cannot("the place is beyond the kernel's grid"))
}

/// The layout `placing` makes when a person puts the dimension at `place`, its references being
/// at `loci` (in the dimension's order).
///
/// # Errors
/// [`DimensionError::CannotPlace`], in words: an angle between lines that are not two straight,
/// crossing lines, a radius or an arc length of something that is not a curve, an arc length of
/// a whole circle, or a place on the very point the layout is measured from (where an angle's
/// lines meet, a curve's centre).
pub fn placed(placing: Placing, place: Point, loci: &[Locus]) -> Result<Layout, DimensionError> {
    let [px, py] = steps(place).map(frac);
    match placing {
        Placing::Angular => {
            let (
                Some(Locus::Line {
                    through: a1,
                    direction: e1,
                }),
                Some(Locus::Line {
                    through: a2,
                    direction: e2,
                }),
            ) = (loci.first(), loci.get(1))
            else {
                return Err(cannot("an angle is put between two straight lines"));
            };
            let [a1x, a1y] = steps(*a1).map(frac);
            let [a2x, a2y] = steps(*a2).map(frac);
            let [e1x, e1y] = e1.map(frac);
            let [e2x, e2y] = e2.map(frac);
            let across = &e1x * &e2y - &e1y * &e2x;
            if across == frac(0) {
                return Err(cannot("the two lines are parallel, so they make no angle"));
            }
            // Where they meet: a1 + t e1, t = ((a2 - a1) x e2) / (e1 x e2).
            let t = ((&a2x - &a1x) * &e2y - (&a2y - &a1y) * &e2x) / &across;
            let vx = &a1x + &t * &e1x;
            let vy = &a1y + &t * &e1y;
            let (qx, qy) = (&px - &vx, &py - &vy);
            // The place as alpha e1 + beta e2.
            let alpha = (&qx * &e2y - &qy * &e2x) / &across;
            let beta = (&e1x * &qy - &e1y * &qx) / &across;
            let ray = |weight: &Frac| {
                if *weight < frac(0) {
                    Ray::Backward
                } else {
                    Ray::Forward
                }
            };
            let radius = length_of(&qx, &qy)?;
            if radius <= 0 {
                return Err(cannot(
                    "put an angle's arc away from the point where its lines meet",
                ));
            }
            Layout::angular(ray(&alpha), ray(&beta), Length::from_steps(radius))
        }
        Placing::Radial | Placing::Diameter | Placing::ArcLength => {
            let Some(Locus::Arc(curve)) = loci.first() else {
                return Err(cannot(
                    "a radius, a diameter or an arc length is put beside an arc or a circle",
                ));
            };
            let [cx, cy] = steps(curve.centre);
            let [x, y] = steps(place);
            let away = x.checked_sub(cx).zip(y.checked_sub(cy));
            let Some((dx, dy)) = away else {
                return Err(cannot("the place is beyond the kernel's grid"));
            };
            if dx == 0 && dy == 0 {
                return Err(cannot("put the dimension away from the curve's centre"));
            }
            match placing {
                Placing::Radial => Ok(Layout::radial(Direction::new(
                    Length::from_steps(dx),
                    Length::from_steps(dy),
                )?)),
                Placing::Diameter => Ok(Layout::diameter(Direction::new(
                    Length::from_steps(dx),
                    Length::from_steps(dy),
                )?)),
                _ => {
                    if curve.sweep == crate::Sweep::Full {
                        return Err(cannot(
                            "a whole circle has no ends to measure an arc length between",
                        ));
                    }
                    let distance = length_of(&frac(dx), &frac(dy))?;
                    let offset = distance
                        .checked_sub(curve.radius.steps())
                        .ok_or_else(|| cannot("the place is beyond the kernel's grid"))?;
                    Layout::arc_length(Length::from_steps(offset))
                }
            }
        }
    }
}
