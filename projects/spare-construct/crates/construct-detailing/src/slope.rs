//! Spot slopes: the slope of a floor or roof face at a point, in a view.
//!
//! A spot slope is an element record of one view that names a face's element (a floor or a roof)
//! and a point in the view's plan. It stores nothing it can work out: the slope is read from the
//! face each time it is asked, so it follows the roof when the roof changes, as a dimension follows
//! its references. The one question it asks of the model is [`Faces::plane_at`]: the plane of the
//! element's face that lies over that point, which construct-api answers from the document's
//! records and the tests from a table.
//!
//! What is read ([`Reading`]): the exact squared rise over run of the steepest line on the plane,
//! the rise over run itself when it is a ratio of whole numbers (a face rising 3 in 4 along one axis
//! is exactly 3/4, and a 30 degree roof as stored on the grid is the fraction of its whole steps; one
//! rising 1 in 1 along both axes is the square root of 2, which is not), the angle from
//! the level in whole angle units, and the direction water runs. The angle is worked out from the
//! exact numbers by `atan2` of libm (a pure Rust port of musl's, the same on every machine) in
//! double precision and rounded to the nearest angle unit (1/9,000,000 degree): it is within one
//! unit of the true angle, which is the one float in this crate and is display only.
//!
//! Refused by name ([`SlopeRefusal`]): a face that is missing, a point that is over no face, a
//! face that is flat, and a face that stands upright.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "the only unchecked operators here are on num-bigint whole numbers of any size, which cannot overflow, with a divisor above zero; every fixed-width step is a checked method"
)]
use crate::error::{DetailError, SlopeRefusal};
use crate::num::point_in_grid;
use crate::record::{Props, Reader};
use construct_categories::SPOT_DIMENSIONS;
use construct_grids::Point;
use construct_model::element::Element;
use construct_params::{Angle, Ratio};
use kit_core::Id;
use num_bigint::BigInt;
use num_traits::{Signed, Zero};

/// The `kind` of every spot slope record, and its recipe's operation.
pub const SLOPE_KIND: &str = "detail.slope";
/// The name every spot slope record carries.
pub const SLOPE_NAME: &str = "Spot slope";
/// The floor or roof the slope is read from, an ID.
pub const FACE_KEY: &str = "@detail.slope.face";
/// The point in the view's plan the slope is read at.
pub const POINT_KEY: &str = "@detail.slope.point";

/// A plane in the model: a point on it, in grid steps, and a normal to it (not zero), as whole
/// numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plane {
    /// A point on the plane.
    pub through: [i64; 3],
    /// A vector square to the plane.
    pub normal: [i128; 3],
}

impl Plane {
    /// The plane through three points, exactly.
    ///
    /// # Errors
    /// [`SlopeRefusal::Missing`] for three points on one line, which make no plane.
    pub fn through_points(
        origin: [i64; 3],
        along: [i64; 3],
        across: [i64; 3],
    ) -> Result<Self, SlopeRefusal> {
        let gap = |to: [i64; 3], from: [i64; 3]| -> [i128; 3] {
            [
                i128::from(to[0]).saturating_sub(i128::from(from[0])),
                i128::from(to[1]).saturating_sub(i128::from(from[1])),
                i128::from(to[2]).saturating_sub(i128::from(from[2])),
            ]
        };
        let (first, second) = (gap(along, origin), gap(across, origin));
        // Each component of a difference is under 2^41 in size, so each product is under 2^82 and
        // each part of the cross product under 2^83: no overflow.
        let cross = |i: usize, j: usize| -> i128 {
            let at = |w: [i128; 3], k: usize| w.get(k).copied().unwrap_or(0);
            at(first, i)
                .saturating_mul(at(second, j))
                .saturating_sub(at(first, j).saturating_mul(at(second, i)))
        };
        let normal = [cross(1, 2), cross(2, 0), cross(0, 1)];
        if normal == [0, 0, 0] {
            return Err(SlopeRefusal::Missing(
                "Three points of the face are on one line: they make no plane.".to_owned(),
            ));
        }
        Ok(Self {
            through: origin,
            normal,
        })
    }
}

/// What a spot slope asks of the model: the plane of a face over a point.
pub trait Faces {
    /// The plane of the floor or roof `face`, the part of it over `at` in plan.
    ///
    /// # Errors
    /// [`SlopeRefusal::Missing`] when there is no such floor or roof, [`SlopeRefusal::OffFace`]
    /// when `at` is over no part of it.
    fn plane_at(&self, face: &Id, at: Point) -> Result<Plane, SlopeRefusal>;
}

/// What a face's slope is, worked out exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    /// The square of the rise over the run of the steepest line on the face, as a fraction in
    /// lowest terms: numerator and denominator.
    pub squared: (BigInt, BigInt),
    /// The rise over the run, when it is a ratio of whole numbers that fits 128 bits; `None` when
    /// it is not (a face rising equally along both axes).
    pub rise_over_run: Option<Ratio<i128>>,
    /// The angle of the steepest line above the level, to the nearest angle unit.
    pub angle: Angle,
    /// The direction water runs down the face, in plan, as a whole vector in lowest terms.
    pub fall: [i128; 2],
}

fn gcd(a: &BigInt, b: &BigInt) -> BigInt {
    let (mut a, mut b) = (a.abs(), b.abs());
    while !b.is_zero() {
        let r = &a % &b;
        a = std::mem::replace(&mut b, r);
    }
    a
}

fn gcd_i128(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a.checked_rem(b).unwrap_or(0));
    }
    i128::try_from(a).unwrap_or(1)
}

/// `n` as a float, for display and for the angle only.
#[allow(
    clippy::cast_precision_loss,
    reason = "the angle and its display are not stored: double precision is within an angle unit"
)]
fn float(n: i128) -> f64 {
    n as f64
}

/// The angle in whole angle units of the line that rises `h` over `v` (both above zero).
#[allow(
    clippy::cast_possible_truncation,
    reason = "the angle is between 0 and 90 degrees: 810 million units, far inside an i64"
)]
fn angle_of(h: f64, v: f64) -> Angle {
    let degrees = libm::atan2(h, v) * 180.0 / std::f64::consts::PI;
    let units = libm::round(degrees * float(i128::from(Angle::UNITS_PER_DEGREE)));
    Angle::from_units(units as i64)
}

/// The slope of a plane.
///
/// # Errors
/// [`SlopeRefusal::Flat`] for a level plane, [`SlopeRefusal::Vertical`] for an upright one.
pub fn read(plane: &Plane) -> Result<Reading, SlopeRefusal> {
    let [mut nx, mut ny, mut nz] = plane.normal;
    if nz < 0 {
        // Turn the normal up: the plane is the same, and water runs along (nx, ny) from then on.
        (nx, ny, nz) = (
            nx.saturating_neg(),
            ny.saturating_neg(),
            nz.saturating_neg(),
        );
    }
    if nz == 0 {
        return Err(SlopeRefusal::Vertical);
    }
    if nx == 0 && ny == 0 {
        return Err(SlopeRefusal::Flat);
    }
    let horizontal = BigInt::from(nx) * BigInt::from(nx) + BigInt::from(ny) * BigInt::from(ny);
    let vertical = BigInt::from(nz) * BigInt::from(nz);
    let g = gcd(&horizontal, &vertical);
    let squared = (&horizontal / &g, &vertical / &g);
    let (rise, run) = (squared.0.sqrt(), squared.1.sqrt());
    let exact = &rise * &rise == squared.0 && &run * &run == squared.1;
    let rise_over_run = if exact {
        match (i128::try_from(&rise), i128::try_from(&run)) {
            (Ok(r), Ok(n)) => Some(Ratio::new(r, n)),
            _ => None,
        }
    } else {
        None
    };
    // The horizontal length of the normal, as a float, over its height: the steepest rise over run.
    let h = libm::sqrt(float(nx) * float(nx) + float(ny) * float(ny));
    let angle = angle_of(h, float(nz));
    let f = gcd_i128(nx, ny).max(1);
    Ok(Reading {
        squared,
        rise_over_run,
        angle,
        fall: [
            nx.checked_div(f).unwrap_or(nx),
            ny.checked_div(f).unwrap_or(ny),
        ],
    })
}

impl Reading {
    /// The rise over the run as a float, for display only.
    #[must_use]
    pub fn rise_over_run_approx(&self) -> f64 {
        let n = float(i128::try_from(&self.squared.0).unwrap_or(i128::MAX));
        let d = float(i128::try_from(&self.squared.1).unwrap_or(i128::MAX));
        libm::sqrt(n / d)
    }

    /// The slope in the person's words: `36.87 degrees, rise over run 3/4` for a face whose rise
    /// over run is a fraction with a denominator up to 100, else the decimal to four places:
    /// `30.00 degrees, rise over run 0.5774`.
    #[must_use]
    pub fn text(&self) -> String {
        let degrees =
            float(i128::from(self.angle.units())) / float(i128::from(Angle::UNITS_PER_DEGREE));
        match &self.rise_over_run {
            Some(r) if *r.denom() <= 100 => format!(
                "{degrees:.2} degrees, rise over run {}/{}",
                r.numer(),
                r.denom()
            ),
            _ => format!(
                "{degrees:.2} degrees, rise over run {:.4}",
                self.rise_over_run_approx()
            ),
        }
    }
}

/// A spot slope: its ID, its view, the face it reads and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpotSlope {
    id: Id,
    view: Id,
    face: Id,
    at: Point,
}

impl SpotSlope {
    /// A spot slope of the floor or roof `face`, read at `at` in plan.
    ///
    /// # Errors
    /// [`DetailError::OutOfRange`] for a point past the kernel's grid.
    pub fn new(id: Id, view: Id, face: Id, at: Point) -> Result<Self, DetailError> {
        point_in_grid(at)?;
        Ok(Self { id, view, face, at })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The view it is in.
    #[must_use]
    pub fn view(&self) -> &Id {
        &self.view
    }

    /// The element whose face it reads.
    #[must_use]
    pub fn face(&self) -> &Id {
        &self.face
    }

    /// Where it reads.
    #[must_use]
    pub fn at(&self) -> Point {
        self.at
    }

    /// The slope moved to another point.
    ///
    /// # Errors
    /// [`DetailError::OutOfRange`].
    pub fn with_point(mut self, at: Point) -> Result<Self, DetailError> {
        point_in_grid(at)?;
        self.at = at;
        Ok(self)
    }

    /// The slope of the face, now.
    ///
    /// # Errors
    /// [`DetailError::Slope`] naming why there is none.
    pub fn read(&self, faces: &dyn Faces) -> Result<Reading, DetailError> {
        let plane = faces
            .plane_at(&self.face, self.at)
            .map_err(DetailError::Slope)?;
        read(&plane).map_err(DetailError::Slope)
    }

    /// The spot slope as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = Props::new(SPOT_DIMENSIONS.0, &self.view);
        props.text(FACE_KEY, self.face.as_str());
        props.point(POINT_KEY, self.at);
        props.finish(&self.id, document, SLOPE_KIND, SLOPE_NAME, Vec::new())
    }

    /// Reads a spot slope from its record; anything [`SpotSlope::to_record`] would not write is
    /// refused.
    ///
    /// # Errors
    /// [`DetailError::Malformed`], naming the record.
    pub fn from_record(record: &Element) -> Result<Self, DetailError> {
        let read = Reader::new(record, SLOPE_KIND, "spot slope")?;
        let face = Id::parse(read.text(FACE_KEY)?).map_err(|_| read.bad("holds no face ID"))?;
        let slope = Self::new(
            record.id.clone(),
            read.view()?,
            face,
            read.point(POINT_KEY)?,
        )
        .map_err(|e| read.bad(&e.to_string()))?;
        read.same_as(&slope.to_record(&record.document))?;
        Ok(slope)
    }
}

/// Whether `at` is over the polygon `corners` in plan (grid steps), a point on its boundary
/// counted. Exact: whole numbers only. A polygon may be any simple outline, concave or not.
#[must_use]
pub fn polygon_covers(corners: &[[i64; 2]], at: [i64; 2]) -> bool {
    covers(corners, at, true)
}

/// Whether `at` is strictly inside the polygon `corners`: a point on its boundary is not.
#[must_use]
pub fn polygon_covers_inside(corners: &[[i64; 2]], at: [i64; 2]) -> bool {
    covers(corners, at, false)
}

fn covers(corners: &[[i64; 2]], at: [i64; 2], boundary: bool) -> bool {
    let wide = |v: i64| i128::from(v);
    let (px, py) = (wide(at[0]), wide(at[1]));
    let mut inside = false;
    for (i, a) in corners.iter().enumerate() {
        let Some(b) = corners.get(i.saturating_add(1)).or_else(|| corners.first()) else {
            continue;
        };
        let (ax, ay, bx, by) = (wide(a[0]), wide(a[1]), wide(b[0]), wide(b[1]));
        // Coordinates are within 2^41 of each other, so every product is under 2^83.
        let cross = bx
            .saturating_sub(ax)
            .saturating_mul(py.saturating_sub(ay))
            .saturating_sub(by.saturating_sub(ay).saturating_mul(px.saturating_sub(ax)));
        if cross == 0
            && px >= ax.min(bx)
            && px <= ax.max(bx)
            && py >= ay.min(by)
            && py <= ay.max(by)
        {
            return boundary;
        }
        if (ay > py) != (by > py) {
            // The edge crosses the horizontal line through the point: is the crossing to its right?
            let rise = by.saturating_sub(ay);
            let left = px.saturating_sub(ax).saturating_mul(rise);
            let right = bx.saturating_sub(ax).saturating_mul(py.saturating_sub(ay));
            if (rise > 0 && left < right) || (rise < 0 && left > right) {
                inside = !inside;
            }
        }
    }
    inside
}
