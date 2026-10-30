//! Evidence: the values an error or a warning points at, each named for what it is.
//!
//! What it does: an [`Evidence`] is one named [`Value`]: a point, a coordinate, a length, a
//! parameter, an exactness class and its bound, or a count, such as "largest radius: 4.2 mm" or
//! "station: 3.40 m" (chapter 22's `at: Vec<Evidence>`). Values are `geom-num` values only, so
//! this crate needs no geometry crate (chapter 27, "Who owns what").
//!
//! Guarantees:
//!
//! - **Numbers layer values only.** Every [`Value`] holds `geom-num` values or plain integers, in
//!   the kernel's own units: grid steps with their step, parameters as certified intervals.
//! - **A length always carries its step.** A count of steps means nothing alone (64 steps of `Q0`
//!   are 10 µm, of `Q1` 0.4 µm), so [`Value::Length`] holds both, as an exactness bound does.
//! - **Plain words.** `Display` writes the name and the value, a length with its step, so a
//!   message can be read without the code.
//!
//! Method: none; the values chapter 22's `Evidence` carries, written as data
//! (`projects/engines/geometry/architecture/22-errors-queries-ai.md`), in `geom-num`'s own types.
//!
//! Cost: a few machine words per value; nothing allocates.

use core::fmt;

use geom_num::interval::Interval;
use geom_num::{Exactness, GridStep, P2, P3, Steps};

/// A value an error or warning points at, in the numbers layer's own types.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// A point on the grid in 2D.
    Point2(P2),
    /// A point on the grid in 3D.
    Point3(P3),
    /// A coordinate in grid steps that need not fit a frame, such as one past ±(2^40 − 1) steps.
    Coordinate(i128),
    /// A distance or a bound: a count of grid steps and the step it counts.
    Length {
        /// The count.
        steps: Steps,
        /// The grid step it counts.
        step: GridStep,
    },
    /// A parameter value or range, enclosed: a point interval when it is exact.
    Parameter(Interval),
    /// An exactness class and its bound.
    Exactness(Exactness),
    /// A count: entities, work units or logical bytes.
    Count(u128),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Point2(p) => write!(f, "({}, {}) steps", p.x.get(), p.y.get()),
            Value::Point3(p) => write!(f, "({}, {}, {}) steps", p.x.get(), p.y.get(), p.z.get()),
            Value::Coordinate(c) => write!(f, "{c} steps"),
            Value::Length { steps, step } => write!(f, "{} steps of {step:?}", steps.0),
            Value::Parameter(range) => write!(f, "[{}, {}]", range.lo(), range.hi()),
            Value::Exactness(class) => write!(f, "{class:?}"),
            Value::Count(n) => write!(f, "{n}"),
        }
    }
}

/// One named value an error or a warning points at (chapter 22).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evidence {
    /// What the value is, in plain words: `largest radius`, `station`, `gap`.
    pub what: &'static str,
    /// The value.
    pub value: Value,
}

impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.what, self.value)
    }
}
