//! Committing planes: a flat construction with irrational coefficients, taken once to integers
//! within 6.2's budget (chapter 13.2).
//!
//! What it does: a 1.5° draft, a 30° roof or a face moved along a sloping normal needs a plane whose
//! ideal coefficients are irrational. [`through_edge`] commits a plane that must hold an exact
//! edge: its normal is the edge crossed with the integer vector nearest the ideal tilt, and its
//! offset comes from a point of the edge, so the edge lies on it exactly. [`moved_along`] commits a
//! free plane moved a whole number of steps along its normal. Each proves how far the committed
//! plane strays from the ideal one over the face's extent. [`PlaneClass`] is the symmetric mode: one
//! commit for a class of planes, whose images under the grid's symmetries are exact. [`vertex_slide`]
//! bounds how far a vertex moves when its three planes move, and [`check_flip`] compares a decision
//! on the committed planes with the same decision on the ideal ones.
//!
//! Guarantees:
//!
//! - **Within budget, or refused.** A committed plane is a [`Plane`], so its normal is below 2^83
//!   and its offset below 2^125. A tilt along the edge, or a direction of no length, is
//!   `Degenerate`; a direction with a non-finite component, or too large to normalise, is
//!   `NotFinite`.
//! - **The edge lies on the plane.** [`through_edge`]'s plane holds both ends of its edge exactly
//!   (tested): the normal is a cross product with the edge, and the offset is exact.
//! - **A proven deviation.** [`Committed::deviation`] bounds in grid steps the distance between the
//!   two planes over the convex hull of the extent. The signed distance between two planes is
//!   affine, so its largest magnitude over a hull lies at a corner of it, and each is evaluated in
//!   outward-rounded intervals. Nothing rounds again after a commit (6.3).
//! - **The same plane on every target.** Scaling by a power of two and rounding ties to even are
//!   exact in IEEE 754, and the rest is integer arithmetic.
//! - **A commit that changes the shape fails.** [`check_flip`] is [`CommitError::Flip`] when the
//!   ideal sign is certain and the committed sign differs, and that is `ExactnessNotAccepted`
//!   (`CommitFlip`) as an operation's error. If the ideal sign cannot be certified, a designed tie,
//!   the committed sign stands (13.2).
//! - **Symmetric roofs stay symmetric.** Every plane of a [`PlaneClass`] has one normal, and an
//!   image's normal is that normal under a signed permutation, so no rounding happens twice and
//!   designed ties survive (13.15).
//! - **Certified, always.** [`Committed::class`] is Certified with the proven deviation, however
//!   small it is: a commit rounds, so it is never Exact (32.21).
//!
//! Method: chapter 13.2's "Committed planes" and decision 32.21
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`). The bound on a vertex is
//! Cramer's rule on the three plane errors; the integer square root is the binary method of
//! Knuth, *The Art of Computer Programming*, volume 2, section 4.3.1, exercise 4.3.3; intervals are
//! Moore's, 1966, as in `geom_num::interval`, <https://doi.org/10.1137/1.9780898717716>.
//!
//! Cost: a constant number of wide products per plane, 125 of them for the square root in
//! [`moved_along`], and a dozen interval operations per corner of an extent.

use core::fmt;

use geom_math::GridTransform;
use geom_num::interval::Interval;
use geom_num::math::{ceil, round_ties_even};
use geom_num::plane::Plane;
use geom_num::{Exactness, GridStep, I256, NumError, P3, Real, Sign, Steps};
use geom_op::{BoundDetail, OpErrorDetail};

/// Why a plane could not be committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitError {
    /// The numbers layer refused: a direction of no length, a tilt along the edge, or an extent
    /// with no corners.
    Num(NumError),
    /// The commit changed a decision the ideal shape makes for certain (`CommitFlip`, section 22).
    Flip,
}

impl fmt::Display for CommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitError::Num(e) => write!(f, "a plane that cannot be committed: {e:?}"),
            CommitError::Flip => f.write_str("a commit changed a certain decision"),
        }
    }
}

impl core::error::Error for CommitError {}

impl From<NumError> for CommitError {
    fn from(e: NumError) -> Self {
        CommitError::Num(e)
    }
}

impl From<CommitError> for OpErrorDetail {
    fn from(e: CommitError) -> Self {
        match e {
            CommitError::Num(e) => OpErrorDetail::Num(e.into()),
            CommitError::Flip => OpErrorDetail::Bound(BoundDetail::CommitFlip),
        }
    }
}

/// A committed plane and what it costs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Committed {
    /// The plane.
    pub plane: Plane,
    /// How far it strays from the ideal plane over the extent, at most, in grid steps.
    pub deviation: f64,
}

impl Committed {
    /// The deviation in whole steps, rounded up: the term this commit adds to its face's bound.
    #[must_use]
    pub fn term(&self) -> Steps {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole, non-negative double, far under 2^64 anywhere in the grid's range"
        )]
        let whole = ceil(self.deviation).map_or(u64::MAX, |x| x.get() as u64);
        Steps(whole)
    }

    /// The class of what this commit makes: Certified with the proven deviation, as every commit
    /// is (32.21).
    #[must_use]
    pub fn class(&self, step: GridStep) -> Exactness {
        Exactness::Certified {
            bound: self.term(),
            step,
        }
    }
}

/// An ideal plane: its unit normal and offset, enclosed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ideal {
    /// The unit normal.
    pub normal: [Interval; 3],
    /// The offset, in grid steps.
    pub offset: Interval,
}

/// `v`, enclosed: the double nearest it, one step out on each side.
fn enclosed(v: i128) -> Result<Interval, NumError> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the cast rounds to nearest, and the interval widens by a step on each side"
    )]
    let x = v as f64;
    Interval::new(x.next_down(), x.next_up())
}

/// A grid point's coordinates, which fit a double exactly.
fn enclosed_point(p: P3) -> Result<[Interval; 3], NumError> {
    let one = |v: i64| enclosed(i128::from(v));
    Ok([one(p.x.get())?, one(p.y.get())?, one(p.z.get())?])
}

/// a · b for vectors of intervals.
fn dot(a: &[Interval; 3], b: &[Interval; 3]) -> Result<Interval, NumError> {
    let [ax, ay, az] = *a;
    let [bx, by, bz] = *b;
    ax.mul(bx)?.add(ay.mul(by)?)?.add(az.mul(bz)?)
}

/// a × b for vectors of intervals.
fn cross(a: &[Interval; 3], b: &[Interval; 3]) -> Result<[Interval; 3], NumError> {
    let [ax, ay, az] = *a;
    let [bx, by, bz] = *b;
    Ok([
        ay.mul(bz)?.sub(az.mul(by)?)?,
        az.mul(bx)?.sub(ax.mul(bz)?)?,
        ax.mul(by)?.sub(ay.mul(bx)?)?,
    ])
}

/// `v` scaled to length one.
fn unit(v: &[Interval; 3]) -> Result<[Interval; 3], NumError> {
    let length = Interval::norm(v)?;
    let [x, y, z] = *v;
    Ok([x.div(length)?, y.div(length)?, z.div(length)?])
}

/// The largest magnitude in `v`.
fn magnitude(v: Interval) -> Result<Real, NumError> {
    Ok(Real::new(v.lo().abs())?.total_max(Real::new(v.hi().abs())?))
}

impl Ideal {
    /// The plane through `point` with normal along `direction`, moved `distance` steps along it.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for a direction of no length, [`NumError::NotFinite`] for a
    /// non-finite one.
    pub fn moved(direction: [f64; 3], point: P3, distance: i64) -> Result<Ideal, NumError> {
        let [x, y, z] = direction.map(Interval::point);
        let normal = unit(&[x?, y?, z?])?;
        let along = dot(&normal, &enclosed_point(point)?)?;
        let offset = along.add(enclosed(i128::from(distance))?)?.neg();
        Ok(Ideal { normal, offset })
    }

    /// The plane through the edge from `a` to `b` that holds the direction `tilt`.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for a tilt along the edge, [`NumError::NotFinite`] for a non-finite
    /// one.
    #[allow(
        clippy::many_single_char_names,
        reason = "a and b are the edge's ends, and x, y and z the tilt's components"
    )]
    pub fn through_edge(a: P3, b: P3, tilt: [f64; 3]) -> Result<Ideal, NumError> {
        let ([ax, ay, az], [bx, by, bz]) = (enclosed_point(a)?, enclosed_point(b)?);
        let edge = [bx.sub(ax)?, by.sub(ay)?, bz.sub(az)?];
        let [x, y, z] = tilt.map(Interval::point);
        let normal = unit(&cross(&edge, &[x?, y?, z?])?)?;
        let offset = dot(&normal, &[ax, ay, az])?.neg();
        Ok(Ideal { normal, offset })
    }

    /// What the plane reads at `p`: its signed distance in grid steps.
    ///
    /// # Errors
    ///
    /// What the interval arithmetic gives, which finite coefficients cannot reach.
    pub fn reading(&self, p: P3) -> Result<Interval, NumError> {
        dot(&self.normal, &enclosed_point(p)?)?.add(self.offset)
    }
}

/// `direction` scaled by a power of two so that its largest component fills `bits` bits, each
/// component rounded to a whole number. A double's product by a power of two is exact.
fn scaled(direction: [f64; 3], bits: u32) -> Result<[i128; 3], NumError> {
    let mut largest = Real::new(0.0)?;
    for v in direction {
        largest = largest.total_max(Real::new(v.abs())?);
    }
    let largest = largest.get();
    if largest == 0.0 {
        return Err(NumError::Degenerate);
    }
    // The exponent field of a double, biased by 1023; the scale's field is that of 2^(bits - 1 - e).
    let field = i64::try_from(largest.to_bits() >> 52).map_err(|_| NumError::Degenerate)?;
    let wanted = i64::from(bits).saturating_add(2045).saturating_sub(field);
    let scale_field = u64::try_from(wanted).map_err(|_| NumError::Degenerate)?;
    if scale_field >= 2047 {
        return Err(NumError::Degenerate);
    }
    let scale = f64::from_bits(scale_field << 52);
    let whole = |v: f64| -> Result<i128, NumError> {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a whole double below 2^bits, and every caller's bits are under 127"
        )]
        Ok(round_ties_even(v * scale)?.get() as i128)
    };
    let [x, y, z] = direction;
    Ok([whole(x)?, whole(y)?, whole(z)?])
}

/// The offset that puts `p` on the plane with normal `n`, less `extra`.
fn offset_through(n: &[i128; 3], p: P3, extra: i128) -> Result<i128, NumError> {
    let [nx, ny, nz] = *n;
    let term = |c: i128, v: i64| c.checked_mul(i128::from(v));
    let tx = term(nx, p.x.get());
    let ty = term(ny, p.y.get());
    let tz = term(nz, p.z.get());
    tx.zip(ty)
        .zip(tz)
        .and_then(|((x, y), z)| {
            x.checked_add(y)?
                .checked_add(z)?
                .checked_add(extra)?
                .checked_neg()
        })
        .ok_or(NumError::BudgetExceeded)
}

/// The largest r with r² ≤ `square`, for a square below 2^250. The kernel carries no big-integer
/// crate, and an interval's square root brackets only to a unit in the last place, where 13.2 asks
/// for the bracket of an exact integer root.
fn integer_root(square: &I256) -> Option<i128> {
    let mut root = 0i128;
    for bit in (0..=124u32).rev() {
        let trial = root.checked_add(1i128.checked_shl(bit)?)?;
        let wide = I256::from_i128(trial);
        if !square.checked_sub(&wide.checked_mul(&wide)?)?.is_negative() {
            root = trial;
        }
    }
    Some(root)
}

/// `distance` · |`n`|, to the nearest whole number: 13.2's offset for a free plane, |N| bracketed
/// exactly by taking the root of N²·δ² in one, and the nearest by testing (2r + 1)² against 4·N²·δ².
fn moved_by(normal: &[i128; 3], distance: i64) -> Option<i128> {
    let [x, y, z] = normal.map(I256::from_i128);
    let steps = I256::from_i128(i128::from(distance));
    let square = |v: &I256| v.checked_mul(v);
    let product = square(&x)?
        .checked_add(&square(&y)?)?
        .checked_add(&square(&z)?)?
        .checked_mul(&square(&steps)?)?;
    let root = integer_root(&product)?;
    let odd = I256::from_i128(root.checked_mul(2)?.checked_add(1)?);
    let four = product.checked_mul(&I256::from_i128(4))?;
    let up = !four.checked_sub(&square(&odd)?)?.is_negative();
    let nearest = root.checked_add(i128::from(up))?;
    if distance < 0 {
        nearest.checked_neg()
    } else {
        Some(nearest)
    }
}

/// The most `plane` strays from `ideal` over the convex hull of `extent`, in grid steps. On
/// `plane`, its own reading is zero, so the distance to `ideal` is the difference of the two
/// readings, each normalised, and that is affine: its largest magnitude is at a corner.
fn deviation(plane: &Plane, ideal: &Ideal, extent: &[P3]) -> Result<f64, NumError> {
    let [x, y, z] = plane.n();
    let n = [enclosed(x)?, enclosed(y)?, enclosed(z)?];
    let (offset, length) = (enclosed(plane.d())?, Interval::norm(&n)?);
    if extent.is_empty() {
        return Err(NumError::Degenerate);
    }
    let mut worst = Real::new(0.0)?;
    for p in extent {
        let own = dot(&n, &enclosed_point(*p)?)?.add(offset)?.div(length)?;
        worst = worst.total_max(magnitude(own.sub(ideal.reading(*p)?)?)?);
    }
    Ok(worst.get())
}

/// One commit shared by every plane of a class (13.15): one integer normal, rounded once. Its planes
/// are exactly parallel, and a grid symmetry maps it exactly, so a footprint's symmetric planes come
/// out exact images of each other and the ties their design makes survive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneClass {
    normal: [i128; 3],
    ideal: [Interval; 3],
}

impl PlaneClass {
    /// The planes that hold lines along `edge` and the direction `tilt`: an eave direction and a
    /// slope.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] in a [`CommitError`] for a tilt along the edge.
    pub fn holding(edge: [i128; 3], tilt: [f64; 3]) -> Result<PlaneClass, CommitError> {
        let largest = edge.iter().map(|v| v.unsigned_abs()).fold(0u128, u128::max);
        // A component of the normal is a difference of two products of a tilt and an edge component.
        let bits = 82u32.saturating_sub(128u32.saturating_sub(largest.leading_zeros()));
        let t = scaled(tilt, bits)?;
        let ([ex, ey, ez], [tx, ty, tz]) = (edge, t);
        let minor =
            |p: i128, q: i128, r: i128, s: i128| p.checked_mul(q)?.checked_sub(r.checked_mul(s)?);
        let normal = [
            minor(ey, tz, ez, ty).ok_or(NumError::BudgetExceeded)?,
            minor(ez, tx, ex, tz).ok_or(NumError::BudgetExceeded)?,
            minor(ex, ty, ey, tx).ok_or(NumError::BudgetExceeded)?,
        ];
        if normal == [0; 3] {
            return Err(NumError::Degenerate.into());
        }
        let [x, y, z] = tilt.map(Interval::point);
        let along = [enclosed(ex)?, enclosed(ey)?, enclosed(ez)?];
        let ideal = unit(&cross(&along, &[x?, y?, z?])?)?;
        Ok(PlaneClass { normal, ideal })
    }

    /// The planes with their normal along `direction`.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] in a [`CommitError`] for a direction of no length.
    pub fn along(direction: [f64; 3]) -> Result<PlaneClass, CommitError> {
        let normal = scaled(direction, 82)?;
        let [x, y, z] = direction.map(Interval::point);
        let ideal = unit(&[x?, y?, z?])?;
        Ok(PlaneClass { normal, ideal })
    }

    /// The class `symmetry` carries this one to. Only its signed permutation counts: a plane says
    /// where it goes by [`PlaneClass::plane`]'s point.
    #[must_use]
    pub fn image(&self, symmetry: &GridTransform) -> PlaneClass {
        PlaneClass {
            normal: symmetry.apply_signed(self.normal, i128::saturating_neg),
            ideal: symmetry.apply_signed(self.ideal, Interval::neg),
        }
    }

    /// The integer normal.
    #[must_use]
    pub const fn normal(&self) -> [i128; 3] {
        self.normal
    }

    /// The plane of this class through `point`, moved `distance` whole steps along its normal,
    /// committed over `extent`. Its offset is the nearest whole number to the exact one, so the plane
    /// sits at most 1/(2|N|) of a step from the exact move.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] in a [`CommitError`] for no extent;
    /// [`NumError::BudgetExceeded`] for a move past the plane budget.
    pub fn plane(&self, point: P3, distance: i64, extent: &[P3]) -> Result<Committed, CommitError> {
        let along = moved_by(&self.normal, distance).ok_or(NumError::BudgetExceeded)?;
        let plane = Plane::new(self.normal, offset_through(&self.normal, point, along)?)?;
        let reading = dot(&self.ideal, &enclosed_point(point)?)?;
        let ideal = Ideal {
            normal: self.ideal,
            offset: reading.add(enclosed(i128::from(distance))?)?.neg(),
        };
        Ok(Committed {
            plane,
            deviation: deviation(&plane, &ideal, extent)?,
        })
    }
}

/// A plane through the edge from `a` to `b` that holds the direction `tilt`, as near as the budget
/// allows, committed over `extent`: the corners of the face, or of anything that holds it.
///
/// # Errors
///
/// [`NumError::Degenerate`] in a [`CommitError`] for a tilt along the edge, or no extent.
pub fn through_edge(a: P3, b: P3, tilt: [f64; 3], extent: &[P3]) -> Result<Committed, CommitError> {
    let edge = [
        i128::from(b.x.get()).saturating_sub(i128::from(a.x.get())),
        i128::from(b.y.get()).saturating_sub(i128::from(a.y.get())),
        i128::from(b.z.get()).saturating_sub(i128::from(a.z.get())),
    ];
    PlaneClass::holding(edge, tilt)?.plane(a, 0, extent)
}

/// The plane through `point` with normal along `direction`, moved `distance` whole steps along it,
/// as near as the budget allows, committed over `extent`.
///
/// # Errors
///
/// [`NumError::Degenerate`] in a [`CommitError`] for a direction of no length, or no extent.
pub fn moved_along(
    direction: [f64; 3],
    point: P3,
    distance: i64,
    extent: &[P3],
) -> Result<Committed, CommitError> {
    PlaneClass::along(direction)?.plane(point, distance, extent)
}

/// How far the point where three planes meet moves, at most, in grid steps, when each plane moves
/// by its `errors`: by Cramer's rule on unit normals, the sum of each error times the length of the
/// other two normals' cross product, over the determinant of the three. A sharp wedge has a small
/// determinant, and that is the slide 13.2 counts.
///
/// # Errors
///
/// [`NumError::Degenerate`] in a [`CommitError`] if the determinant of the normals may be zero.
#[allow(
    clippy::many_single_char_names,
    reason = "p, q and r are the planes, and a, b and c their unit normals"
)]
pub fn vertex_slide(planes: [&Plane; 3], errors: [f64; 3]) -> Result<f64, CommitError> {
    let unit_normal = |plane: &Plane| -> Result<[Interval; 3], NumError> {
        let [x, y, z] = plane.n();
        unit(&[enclosed(x)?, enclosed(y)?, enclosed(z)?])
    };
    let [p, q, r] = planes;
    let (a, b, c) = (unit_normal(p)?, unit_normal(q)?, unit_normal(r)?);
    let (bc, ca, ab) = (cross(&b, &c)?, cross(&c, &a)?, cross(&a, &b)?);
    let weight = dot(&a, &bc)?;
    if weight.certain_sign().is_none() {
        return Err(NumError::Degenerate.into());
    }
    let [e0, e1, e2] = errors.map(Interval::point);
    let sum = Interval::norm(&bc)?
        .mul(e0?)?
        .add(Interval::norm(&ca)?.mul(e1?)?)?
        .add(Interval::norm(&ab)?.mul(e2?)?)?;
    Ok(magnitude(sum.div(weight)?)?.get())
}

/// Holds a decision made on committed planes to the same decision on the ideal ones (13.2).
///
/// # Errors
///
/// [`CommitError::Flip`] if the ideal sign is certain and `committed` differs.
pub fn check_flip(committed: Sign, ideal: Interval) -> Result<(), CommitError> {
    match ideal.certain_sign() {
        Some(sign) if sign != committed => Err(CommitError::Flip),
        _ => Ok(()),
    }
}
