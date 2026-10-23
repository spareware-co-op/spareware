//! A value that may have been rounded, with whether it was: the report chapter 7 asks of every
//! transform and conversion that rounds to the grid ("other rotations round to the grid and the
//! result says so"), and chapter 6.1 of every conversion from fine to coarse.
//!
//! The numbers layer has its own [`geom_num::grid::Rounded`] for its own roundings, and only
//! `geom-num` can make one: its fields are private and its constructor is the crate's own, so its
//! `is_exact` is always the truth about the rounding that made it. This crate's roundings (map to
//! frame and back, transforms applied to grid points, exact rounding of double-doubles and wide
//! integers) are reported the same way, with a type of their own that only this crate makes.
//!
//! Guarantees:
//!
//! - **The report is the truth.** A `Rounded` is made only inside this crate, by the function that
//!   did the rounding, so `is_exact` says whether the value equals what was converted. A caller can
//!   read a `Rounded`, never build one that claims exactness for a rounded value.
//! - **Plain value.** `Copy`, compared with `==`, hashed; no allocation.
//!
//! Method: the rounding report of chapters 6.1 and 7 of the kernel architecture
//! (<https://github.com/spareware-co-op/spareware/blob/piece/G01/projects/engines/geometry/architecture/07-geometry-types.md>),
//! with the same shape as the numbers layer's.
//!
//! Cost: a value and one flag; reading either is a copy.

/// A value that may have been rounded, with whether it is exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rounded<T> {
    /// The value.
    value: T,
    /// Whether no rounding happened.
    exact: bool,
}

impl<T: Copy> Rounded<T> {
    /// A value and whether it is exact, for this crate's rounding functions only.
    pub(crate) const fn new(value: T, exact: bool) -> Rounded<T> {
        Rounded { value, exact }
    }

    /// The value.
    #[must_use]
    pub const fn value(&self) -> T {
        self.value
    }

    /// Whether no rounding happened: the value is exactly what was converted.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        self.exact
    }
}
