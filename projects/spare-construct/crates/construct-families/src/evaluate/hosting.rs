//! A hosted family's opening at one type's values (CON-A7-F3-P2, piece A64, job 6.4): what the
//! recipe's `hosting` declares, solved as a step's sketch is solved, for a builder to cut from the
//! host.
//!
//! A wall-hosted recipe names its opening as a closed loop of lines in a sketch on an `xz` plane
//! (FORMAT.md, "Hosting"). Here that sketch is solved at the type's values exactly as `sketch.rs`
//! solves a step's sketch (from where it is stored, the least change, on the grid, the same on
//! every run, within the same [`Limits`]), and the loop is read back as its corners in loop order,
//! in the family's own frame: x along the wall, z up, both `construct_params::Length` grid steps.
//! The depth is the format's one rule for a wall's opening, [`Depth::ThroughHost`]: through the
//! host's whole thickness, whatever the plane's offset. No engine is asked.
//!
//! Refused, each as an [`EvalError`] at [`Place::Hosting`] unless the sketch names its own place:
//! a recipe that declares no host ([`Reason::NotHosted`]); a host with no opening to cut, a floor
//! or any face ([`Reason::NoOpening`]); an opening on a sketch that is not on an `xz` plane
//! ([`Reason::NotOnWallPlane`]); an outline that is not one closed loop ([`Reason::NotALoop`]);
//! and whatever refuses the sketch itself (a missing sketch, a value or formula refused, a sketch
//! the solver cannot solve at these values, a limit reached).

use super::error::{EvalError, Place, Reason};
use super::limits::{Limits, Meter, Used};
use super::sketch::resolve;
use super::symbols::Point2;
use super::values::{TypeValues, work_out};
use crate::model::{Base, Hosting, Recipe};

/// What a family is hosted on, among the hosts that have an opening cut in them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    /// A wall.
    Wall,
}

/// How deep an opening is cut in its host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    /// Through the host's whole thickness: a wall's every layer, from one side to the other.
    ThroughHost,
}

/// A hosted family's opening at one type's values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolvedOpening {
    /// What it is cut in.
    pub host: Host,
    /// Its outline's corners in loop order, from the start of the loop's first line, in the
    /// family's frame: along x, up z.
    pub outline: Vec<Point2>,
    /// How deep it is cut.
    pub depth: Depth,
    /// What solving it used, counted as evaluation's limits count.
    pub used: Used,
}

/// The opening `recipe` declares, solved for the type whose values are `values`, within the
/// default limits ([`Limits::DEFAULT`]).
///
/// # Errors
///
/// As this module's header: an [`EvalError`] naming the place and the [`Reason`].
pub fn opening(recipe: &Recipe, values: &TypeValues) -> Result<SolvedOpening, EvalError> {
    opening_within(recipe, values, Limits::DEFAULT)
}

/// As [`opening()`], within `limits`.
///
/// # Errors
///
/// As [`opening()`].
pub fn opening_within(
    recipe: &Recipe,
    values: &TypeValues,
    limits: Limits,
) -> Result<SolvedOpening, EvalError> {
    let declared = match &recipe.hosting {
        Hosting::Wall { opening } => opening,
        Hosting::Unhosted => {
            return Err(EvalError::new(Place::Hosting, Reason::NotHosted));
        }
        Hosting::Floor => {
            return Err(EvalError::new(
                Place::Hosting,
                Reason::NoOpening { host: "floor" },
            ));
        }
        Hosting::Face => {
            return Err(EvalError::new(
                Place::Hosting,
                Reason::NoOpening { host: "face" },
            ));
        }
    };
    let mut meter = Meter::new(limits);
    let worked = work_out(recipe, values, &mut meter)?;
    let drawn = resolve(recipe, &declared.sketch, &worked, &mut meter)?;
    if drawn.base() != Base::Xz {
        return Err(EvalError::new(Place::Hosting, Reason::NotOnWallPlane));
    }
    let outline = drawn
        .outline(&declared.profile)
        .map_err(|reason| EvalError::new(Place::Hosting, reason))?;
    Ok(SolvedOpening {
        host: Host::Wall,
        outline,
        depth: Depth::ThroughHost,
        used: meter.used(),
    })
}
