//! Sketch planes: how 2D sketch coordinates map into the 3D frame, and how lengths are measured on a
//! plane whose grid is not square.
//!
//! What it does: a [`SketchPlane`] holds a [`PlaneFrame`]. A [`Lattice`] frame is exact both ways: a
//! sketch point (s, t) is origin + s·u + t·v, whole numbers throughout, and every grid point of the
//! plane is one (s, t). Axis-aligned planes have unit vectors for u and v. A plane with a small
//! whole-number normal, such as a 5-in-12 roof, takes the two shortest vectors of its grid
//! ([`Lattice::of_normal`]), and the sketch measures through their Gram matrix, so a rafter measures
//! its true length ([`Lattice::length_squared`]). A [`Rotated`] frame serves every other plane: its
//! axes are orthonormal doubles, and placing a point in 3D rounds it to the grid.
//!
//! Guarantees:
//!
//! - **The class is stated.** [`SketchPlane::place`] gives the point and its class: `Exact` on a
//!   lattice, `Certified` to one step on a rotated frame, where the rounding is at most √3/2 of a
//!   step and the arithmetic adds under 2^−11 of one.
//! - **Exact both ways on a lattice.** [`Lattice::locate`] returns the (s, t) of a grid point of the
//!   plane, the inverse of [`Lattice::place`], and refuses a point off the plane. u and v must be a
//!   basis of the plane's whole grid, which their cross product being primitive says, so no grid
//!   point of the plane falls between lattice points.
//! - **A cell of at most 16 steps.** A lattice whose cell's longer diagonal passes [`LATTICE_CELL`]
//!   is refused: the commit rung section 15.1 proposes. Above it, the plane takes a rotated frame.
//! - **Orthonormal or refused.** A rotated frame's axes must be unit and at right angles to within
//!   2^−40.
//! - **Checked.** Every integer step is checked; a point outside the grid's range is refused.
//!
//! Method: sections 15.1 and 6.1 (`projects/engines/geometry/architecture/15-sketches.md`). The grid
//! of a plane n·x = d from Bézout's identity on n (D. E. Knuth, *The Art of Computer Programming*
//! vol. 2, 3rd ed., section 4.5.2, algorithm X), and its two shortest vectors by Lagrange's reduction
//! (P. Q. Nguyen and D. Stehlé, "Low-dimensional lattice basis reduction revisited", ACM Trans.
//! Algorithms 5(4) (2009), doi:10.1145/1597036.1597050). The extended Euclidean step is written here
//! since a kernel crate takes no integer library (chapter 27).
//!
//! Cost: a lattice's construction takes O(log n) divisions and O(log n) reduction steps; every map is
//! constant time. No allocation; no recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the section's: s and t for sketch coordinates, u and v for the basis, n the normal, p a point"
)]

use geom_num::math::round_ties_even;
use geom_num::{Exactness, GridStep, P3, Steps};

/// The most a lattice cell may measure across, in steps: section 15.1's proposed lattice-commit rung.
/// It joins section 6.5's ladder if the owner adopts it; until then this is the one place it lives.
pub const LATTICE_CELL: Steps = Steps(16);

/// How far a rotated frame's axes may be from orthonormal: 2^−40, about one step in the grid's whole
/// range, so the frame cannot move a point of the range by more than a step.
const ORTHONORMAL: f64 = 1.0 / 1_099_511_627_776.0;

/// Why a sketch plane or a map could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaneError {
    /// The vectors do not span a plane, or the normal is zero.
    Degenerate,
    /// u and v do not reach every grid point of their plane: their cross product is not primitive.
    NotPrimitive,
    /// The cell's longer diagonal passes [`LATTICE_CELL`]; its square in steps is given.
    CellTooLarge {
        /// The square of the longer diagonal.
        diagonal_squared: u64,
    },
    /// The point is not on the plane.
    OffPlane,
    /// A rotated frame's axes are not orthonormal, or not finite.
    NotOrthonormal,
    /// A value leaves the grid's range or the arithmetic's.
    OutOfRange,
}

impl core::fmt::Display for PlaneError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            PlaneError::Degenerate => "the vectors do not span a plane",
            PlaneError::NotPrimitive => "the vectors do not reach every grid point of the plane",
            PlaneError::CellTooLarge { .. } => "the lattice cell is too large",
            PlaneError::OffPlane => "the point is not on the plane",
            PlaneError::NotOrthonormal => "the axes are not orthonormal",
            PlaneError::OutOfRange => "a value leaves the grid's range",
        })
    }
}

type Vector = [i128; 3];

fn sum(a: i128, b: i128) -> Result<i128, PlaneError> {
    a.checked_add(b).ok_or(PlaneError::OutOfRange)
}

fn product(a: i128, b: i128) -> Result<i128, PlaneError> {
    a.checked_mul(b).ok_or(PlaneError::OutOfRange)
}

/// a·b − c·d.
fn minor(a: i128, b: i128, c: i128, d: i128) -> Result<i128, PlaneError> {
    product(a, b)?
        .checked_sub(product(c, d)?)
        .ok_or(PlaneError::OutOfRange)
}

fn dot(a: &Vector, b: &Vector) -> Result<i128, PlaneError> {
    sum(
        sum(product(a[0], b[0])?, product(a[1], b[1])?)?,
        product(a[2], b[2])?,
    )
}

fn cross(a: &Vector, b: &Vector) -> Result<Vector, PlaneError> {
    Ok([
        minor(a[1], b[2], a[2], b[1])?,
        minor(a[2], b[0], a[0], b[2])?,
        minor(a[0], b[1], a[1], b[0])?,
    ])
}

/// `a` + k·`b`.
fn along(a: &Vector, k: i128, b: &Vector) -> Result<Vector, PlaneError> {
    Ok([
        sum(a[0], product(k, b[0])?)?,
        sum(a[1], product(k, b[1])?)?,
        sum(a[2], product(k, b[2])?)?,
    ])
}

fn negated(a: &Vector) -> Result<Vector, PlaneError> {
    along(&[0; 3], -1, a)
}

/// (g, x, y) with g = gcd(a, b) ≥ 0 and a·x + b·y = g.
fn bezout(a: i128, b: i128) -> Result<(i128, i128, i128), PlaneError> {
    let (mut r0, mut r1) = (a, b);
    let (mut x0, mut x1) = (1_i128, 0_i128);
    let (mut y0, mut y1) = (0_i128, 1_i128);
    while r1 != 0 {
        let q = r0.checked_div(r1).ok_or(PlaneError::OutOfRange)?;
        let (r, x, y) = (
            minor(r0, 1, q, r1)?,
            minor(x0, 1, q, x1)?,
            minor(y0, 1, q, y1)?,
        );
        (r0, x0, y0) = (r1, x1, y1);
        (r1, x1, y1) = (r, x, y);
    }
    if r0 < 0 {
        Ok((
            r0.checked_neg().ok_or(PlaneError::OutOfRange)?,
            x0.checked_neg().ok_or(PlaneError::OutOfRange)?,
            y0.checked_neg().ok_or(PlaneError::OutOfRange)?,
        ))
    } else {
        Ok((r0, x0, y0))
    }
}

/// A basis of the grid vectors at right angles to the primitive `n`: from a·p + b·q = g, the vectors
/// (b/g, −a/g, 0) and (−c·p, −c·q, g), whose cross product is n up to sign.
fn basis_of(n: &Vector) -> Result<(Vector, Vector), PlaneError> {
    let [a, b, c] = *n;
    let (g, p, q) = bezout(a, b)?;
    if g == 0 {
        return Ok(([1, 0, 0], [0, 1, 0]));
    }
    let over = |x: i128| x.checked_div(g).ok_or(PlaneError::OutOfRange);
    let first = [
        over(b)?,
        over(a)?.checked_neg().ok_or(PlaneError::OutOfRange)?,
        0,
    ];
    let minus_c = c.checked_neg().ok_or(PlaneError::OutOfRange)?;
    let second = [product(minus_c, p)?, product(minus_c, q)?, g];
    Ok((first, second))
}

/// Lagrange's reduction: while the longer vector can be shortened by a whole multiple of the shorter,
/// shorten it. Each pass at least halves the product of their lengths' squares or stops.
fn reduced(first: Vector, second: Vector) -> Result<(Vector, Vector), PlaneError> {
    let (mut u, mut v) = (first, second);
    loop {
        if dot(&u, &u)? > dot(&v, &v)? {
            core::mem::swap(&mut u, &mut v);
        }
        let k = nearest(dot(&u, &v)?, dot(&u, &u)?)?;
        if k == 0 {
            return Ok((u, v));
        }
        v = along(&v, k.checked_neg().ok_or(PlaneError::OutOfRange)?, &u)?;
    }
}

/// The whole number nearest top/bottom, bottom > 0, a tie going toward zero so that the reduction
/// stops on it.
fn nearest(top: i128, bottom: i128) -> Result<i128, PlaneError> {
    let whole = top
        .checked_div_euclid(bottom)
        .ok_or(PlaneError::OutOfRange)?;
    let rest = top
        .checked_rem_euclid(bottom)
        .ok_or(PlaneError::OutOfRange)?;
    let twice = product(rest, 2)?;
    Ok(if twice > bottom || (twice == bottom && whole < 0) {
        sum(whole, 1)?
    } else {
        whole
    })
}

/// A plane frame on whole grid vectors: (s, t) is origin + s·u + t·v.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lattice {
    origin: [i64; 3],
    u: Vector,
    v: Vector,
    /// u × v, primitive.
    normal: Vector,
    /// u·u, u·v and v·v.
    gram: [i128; 3],
}

impl Lattice {
    /// The frame with these vectors.
    ///
    /// # Errors
    ///
    /// [`PlaneError::Degenerate`] if they are parallel; [`PlaneError::NotPrimitive`] if they miss
    /// grid points of their plane; [`PlaneError::CellTooLarge`] past [`LATTICE_CELL`];
    /// [`PlaneError::OutOfRange`].
    pub fn new(origin: P3, u: [i64; 3], v: [i64; 3]) -> Result<Lattice, PlaneError> {
        let (u, v) = (u.map(i128::from), v.map(i128::from));
        let normal = cross(&u, &v)?;
        if normal == [0; 3] {
            return Err(PlaneError::Degenerate);
        }
        let (g, ..) = bezout(normal[0], normal[1])?;
        let (g, ..) = bezout(g, normal[2])?;
        if g != 1 {
            return Err(PlaneError::NotPrimitive);
        }
        let gram = [dot(&u, &u)?, dot(&u, &v)?, dot(&v, &v)?];
        let across = sum(sum(gram[0], gram[2])?, product(gram[1], 2)?.abs())?;
        if across > i128::from(LATTICE_CELL.0).saturating_mul(i128::from(LATTICE_CELL.0)) {
            return Err(PlaneError::CellTooLarge {
                diagonal_squared: u64::try_from(across).unwrap_or(u64::MAX),
            });
        }
        Ok(Lattice {
            origin: [origin.x.get(), origin.y.get(), origin.z.get()],
            u,
            v,
            normal,
            gram,
        })
    }

    /// The frame through `origin` at right angles to `normal`, on the two shortest vectors of the
    /// plane's grid, the right way round for `normal`, and with u's first nonzero component positive
    /// so that one plane has one frame.
    ///
    /// # Errors
    ///
    /// [`PlaneError::Degenerate`] for a zero normal; as [`Lattice::new`].
    pub fn of_normal(origin: P3, normal: [i64; 3]) -> Result<Lattice, PlaneError> {
        let n = normal.map(i128::from);
        let (g, ..) = bezout(n[0], n[1])?;
        let (g, ..) = bezout(g, n[2])?;
        if g == 0 {
            return Err(PlaneError::Degenerate);
        }
        let primitive = n.map(|x| x.checked_div(g).unwrap_or(0));
        let (first, second) = basis_of(&primitive)?;
        let (mut u, mut v) = reduced(first, second)?;
        if u.iter().find(|x| **x != 0).is_some_and(|x| *x < 0) {
            u = negated(&u)?;
            v = negated(&v)?;
        }
        if cross(&u, &v)? != primitive {
            v = negated(&v)?;
        }
        let small = |w: &Vector| -> Result<[i64; 3], PlaneError> {
            let mut out = [0_i64; 3];
            for (slot, x) in out.iter_mut().zip(w) {
                *slot = i64::try_from(*x).map_err(|_| PlaneError::OutOfRange)?;
            }
            Ok(out)
        };
        Lattice::new(origin, small(&u)?, small(&v)?)
    }

    /// u·u, u·v and v·v: the Gram matrix sketch lengths are measured through.
    #[must_use]
    pub const fn gram(&self) -> [i128; 3] {
        self.gram
    }

    /// u × v.
    #[must_use]
    pub const fn normal(&self) -> [i128; 3] {
        self.normal
    }

    /// The square of the length, in steps, of the sketch vector (ds, dt): exact.
    ///
    /// # Errors
    ///
    /// [`PlaneError::OutOfRange`].
    pub fn length_squared(&self, ds: i64, dt: i64) -> Result<i128, PlaneError> {
        self.inner((ds, dt), (ds, dt))
    }

    /// The 3D dot product, in steps squared, of two sketch vectors: exact.
    ///
    /// # Errors
    ///
    /// [`PlaneError::OutOfRange`].
    pub fn inner(&self, a: (i64, i64), b: (i64, i64)) -> Result<i128, PlaneError> {
        let (a0, a1, b0, b1) = (
            i128::from(a.0),
            i128::from(a.1),
            i128::from(b.0),
            i128::from(b.1),
        );
        let [g0, g1, g2] = self.gram;
        sum(
            sum(
                product(product(a0, b0)?, g0)?,
                product(sum(product(a0, b1)?, product(a1, b0)?)?, g1)?,
            )?,
            product(product(a1, b1)?, g2)?,
        )
    }

    /// The grid point at (s, t): exact.
    ///
    /// # Errors
    ///
    /// [`PlaneError::OutOfRange`] outside the grid's range.
    pub fn place(&self, s: i64, t: i64) -> Result<P3, PlaneError> {
        let at = along(
            &along(&self.origin.map(i128::from), i128::from(s), &self.u)?,
            i128::from(t),
            &self.v,
        )?;
        let grid = |x: i128| i64::try_from(x).map_err(|_| PlaneError::OutOfRange);
        P3::new(grid(at[0])?, grid(at[1])?, grid(at[2])?).map_err(|_| PlaneError::OutOfRange)
    }

    /// (d·u, d·v) for `p`, d = p − origin.
    fn offsets(&self, p: P3) -> Result<(Vector, i128, i128), PlaneError> {
        let d = [
            minor(i128::from(p.x.get()), 1, i128::from(self.origin[0]), 1)?,
            minor(i128::from(p.y.get()), 1, i128::from(self.origin[1]), 1)?,
            minor(i128::from(p.z.get()), 1, i128::from(self.origin[2]), 1)?,
        ];
        Ok((d, dot(&d, &self.u)?, dot(&d, &self.v)?))
    }

    /// The (s, t) of a grid point of the plane: the inverse of [`Lattice::place`].
    ///
    /// # Errors
    ///
    /// [`PlaneError::OffPlane`] for a point off the plane; [`PlaneError::OutOfRange`].
    pub fn locate(&self, p: P3) -> Result<(i64, i64), PlaneError> {
        let (d, along_u, along_v) = self.offsets(p)?;
        if dot(&d, &self.normal)? != 0 {
            return Err(PlaneError::OffPlane);
        }
        let [g0, g1, g2] = self.gram;
        let det = minor(g0, g2, g1, g1)?;
        let s = minor(along_u, g2, along_v, g1)?;
        let t = minor(along_v, g0, along_u, g1)?;
        let whole = |x: i128| {
            x.checked_div(det)
                .and_then(|q| i64::try_from(q).ok())
                .ok_or(PlaneError::OutOfRange)
        };
        Ok((whole(s)?, whole(t)?))
    }
}

/// A plane frame on orthonormal doubles: (s, t) is origin + s·x + t·y, rounded to the grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotated {
    origin: [f64; 3],
    x_axis: [f64; 3],
    y_axis: [f64; 3],
}

fn real_dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A step's worth of the grid's coordinates in doubles: every coordinate is under 2^41, so exact.
#[allow(
    clippy::cast_precision_loss,
    reason = "a grid coordinate is within ±(2^40 − 1), which a double holds exactly"
)]
fn real(p: P3) -> [f64; 3] {
    [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64]
}

impl Rotated {
    /// The frame with these axes.
    ///
    /// # Errors
    ///
    /// [`PlaneError::NotOrthonormal`] unless each axis is finite and of unit length, and they are at
    /// right angles, to within 2^−40.
    pub fn new(origin: P3, x_axis: [f64; 3], y_axis: [f64; 3]) -> Result<Rotated, PlaneError> {
        let off = |value: f64, want: f64| !value.is_finite() || (value - want).abs() > ORTHONORMAL;
        if off(real_dot(&x_axis, &x_axis), 1.0)
            || off(real_dot(&y_axis, &y_axis), 1.0)
            || off(real_dot(&x_axis, &y_axis), 0.0)
        {
            return Err(PlaneError::NotOrthonormal);
        }
        Ok(Rotated {
            origin: real(origin),
            x_axis,
            y_axis,
        })
    }

    /// The grid point nearest (s, t).
    ///
    /// # Errors
    ///
    /// [`PlaneError::OutOfRange`] outside the grid's range.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "s and t are within the grid's range, which a double holds exactly, and a rounded \
                  coordinate is checked against that range below"
    )]
    pub fn place(&self, s: i64, t: i64) -> Result<P3, PlaneError> {
        let (s, t) = (s as f64, t as f64);
        let at = |i: usize| -> Result<i64, PlaneError> {
            let (o, x, y) = (
                self.origin.get(i).ok_or(PlaneError::OutOfRange)?,
                self.x_axis.get(i).ok_or(PlaneError::OutOfRange)?,
                self.y_axis.get(i).ok_or(PlaneError::OutOfRange)?,
            );
            let value = round_ties_even(o + s * x + t * y)
                .map_err(|_| PlaneError::OutOfRange)?
                .get();
            if value.abs() < 2_199_023_255_552.0 {
                Ok(value as i64)
            } else {
                Err(PlaneError::OutOfRange)
            }
        };
        P3::new(at(0)?, at(1)?, at(2)?).map_err(|_| PlaneError::OutOfRange)
    }

    /// `p`'s sketch coordinates along the axes, and how far it is from the plane, in steps. The
    /// coordinates are not rounded: the sketch's own commit does that.
    #[must_use]
    pub fn project(&self, p: P3) -> (f64, f64, f64) {
        let [x, y, z] = real(p);
        let d = [x - self.origin[0], y - self.origin[1], z - self.origin[2]];
        let (s, t) = (real_dot(&d, &self.x_axis), real_dot(&d, &self.y_axis));
        let (a, b) = (&self.x_axis, &self.y_axis);
        let normal = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        (s, t, real_dot(&d, &normal).abs())
    }
}

/// How a sketch's coordinates map into the 3D frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaneFrame {
    /// Exact both ways.
    Lattice(Lattice),
    /// Placing a point rounds it.
    Rotated(Rotated),
}

/// A point placed in 3D, and how true it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// The point.
    pub point: P3,
    /// Its class against the sketch point it came from.
    pub class: Exactness,
}

/// The plane a sketch lives on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchPlane {
    frame: PlaneFrame,
    step: GridStep,
}

impl SketchPlane {
    /// The plane with `frame`, on a grid of `step`.
    #[must_use]
    pub const fn new(frame: PlaneFrame, step: GridStep) -> SketchPlane {
        SketchPlane { frame, step }
    }

    /// Its frame.
    #[must_use]
    pub const fn frame(&self) -> &PlaneFrame {
        &self.frame
    }

    /// The point at (s, t), with its class.
    ///
    /// # Errors
    ///
    /// [`PlaneError::OutOfRange`] outside the grid's range.
    pub fn place(&self, s: i64, t: i64) -> Result<Placed, PlaneError> {
        Ok(match &self.frame {
            PlaneFrame::Lattice(lattice) => Placed {
                point: lattice.place(s, t)?,
                class: Exactness::Exact,
            },
            PlaneFrame::Rotated(rotated) => Placed {
                point: rotated.place(s, t)?,
                class: Exactness::Certified {
                    bound: Steps(1),
                    step: self.step,
                },
            },
        })
    }
}
