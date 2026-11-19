//! Spare Kit geometry kernel: solidcore, taken whole at one commit, behind the kernel's own
//! interface.
//!
//! One job: Run solidcore's curved operations for the kernel, through an interface of the
//! kernel's own, so that no solidcore type reaches a caller.
//!
//! Layer: outside the kernel's layers and its no_std gate, in its own workspace (decision 32.70).
//!
//! Invariants this crate keeps:
//!
//! - No solidcore type appears in this crate's public interface: a [`Solid`] holds solidcore's
//!   body privately and gives the kernel's own, a `geom_topo::Body`; operations take grid points
//!   and lengths of the frame (`geom_num::P3`, `geom_num::Steps`) on the [`Modeller`]'s step,
//!   a profile's corners as exact fractions of its steps ([`Corner`]), a grid point among them,
//!   and a section plane by a grid point on it or by its distance from one along a whole-number
//!   normal.
//! - Operands are never consumed or changed: solidcore's operations take their operands by value
//!   and modify them, so each operation is handed copies made by solidcore's own `brep_copy`.
//! - Every body that comes back is converted into `geom-topo`'s rows and checked by `geom-check`
//!   at its full level before it is returned ([`convert`]); a body that fails the check, or that
//!   holds what has no kernel kind, is an [`Error`], never an answer. solidcore's own validity
//!   check (SMLib's `AssertValid`, at its first level) runs first, as before.
//! - Flat-faced booleans stay on the kernel's exact `geom-bool`; nothing calls this crate in their
//!   place (decision 32.70).
//!
//! Sections: 8, 11, 12 and 29 of the kernel architecture, and decision 32.70
//! (`projects/engines/geometry/architecture/`).
//!
//! What it does: a [`Modeller`] owns one solidcore kernel and one grid step. It makes the four
//! primitives the stored cases need ([`Modeller::cuboid`], [`Modeller::sphere`],
//! [`Modeller::cylinder`], [`Modeller::cone`]), the extrusion of a plane profile with holes by a
//! vector of whole steps ([`Modeller::extrude`], chapter 13.4's Extrude with distance ends and no
//! taper), the three booleans ([`Modeller::union`],
//! [`Modeller::difference`], [`Modeller::intersection`]), a solid cut by a plane
//! ([`Modeller::plane_section`], [`Modeller::plane_section_at_distance`]) and two solids met
//! ([`Modeller::meet`]), and reads a solid's volume and face count. Lengths reach solidcore in
//! metres, the unit of its tolerances: a length of whole steps becomes the f64 nearest its
//! decimal value ([`convert::to_metres`]), a profile's corner the f64 nearest its exact fraction,
//! a corner on no grid included (decision 32.97), and a plane given by its distance the point of
//! it nearest the grid point it is measured from, each coordinate the f64 nearest its exact
//! value, a plane that holds no point of any grid included (decision 32.99): nothing is rounded
//! before that one rounding at solidcore's door.
//!
//! Not yet the kernel's: the meeting curves and plane sections are solidcore's curves, read by
//! point in grid steps, not `geom-curve` kinds; the converted body's entities are not named by
//! role (chapter 20) yet.
//!
//! Method: solidcore at commit cb9bee4bb9f56880c9973a9f8e99c83fdec24ad3 of the project's fork of
//! tangency-labs/solidcore (Apache-2.0); its methods are SMLib's. Its record of a body is written
//! by its own `solidcore-convert` and read here.
//!
//! Cost: solidcore's, for each operation, plus one copy of each operand, one validity check, one
//! record, one conversion and one full check of each result. Recursion: solidcore's own, bounded
//! by its README's 8 MiB stack (the workspace's `.cargo/config.toml`).
//!
//! SPDX-License-Identifier: Apache-2.0

pub mod convert;

use std::fmt;

use geom_num::{GridStep, P3, Steps};
use geom_topo::Body;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use solidcore::boolean::api_brep::{boolean_difference, boolean_intersection, boolean_union};
use solidcore::curve::curve::Curve;
use solidcore::geom::axis2_placement::Axis2Placement;
use solidcore::intersect::api_intersectors::{
    BrepOperands, intersect_brep_with_plane, intersect_breps,
};
use solidcore::mass::integrator::properties::brep_volume_default;
use solidcore::nurbs::b_spline_curve::p1::{BSplineCurve, NurbCircleParam};
use solidcore::primitive::api_curves::create_line_segment;
use solidcore::primitive::api_primitives::{
    create_box, create_cone, create_cylinder, create_sphere,
};
use solidcore::sweep::api_brep::create_linear_sweep;
use solidcore::topology::brep::p3::brep_copy;
use solidcore::topology::brep::validation::{CheckLevel, check_brep};
use solidcore::{Brep, Kernel, KernelOptions, Point3, SolidError, Vec3};

use crate::convert::{Refusal, metres, to_metres};

/// The relative accuracy the volume integral is asked for: solidcore's integrator stops when its
/// estimate is within this of the true volume.
const VOLUME_ACCURACY: f64 = 1e-10;

/// Why an operation gave no answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// solidcore refused the operation; its own message is kept as text.
    Refused {
        /// The operation, by this crate's name for it.
        operation: &'static str,
        /// solidcore's message.
        reason: String,
    },
    /// solidcore returned a body that fails a check this crate runs on every result: its own
    /// validity check, or the kernel's checker on the converted body.
    CheckFailed {
        /// The operation, by this crate's name for it.
        operation: &'static str,
        /// What the check reported.
        report: String,
    },
    /// solidcore returned a body that holds what has no row or kind in the kernel.
    Unconverted {
        /// The operation, by this crate's name for it.
        operation: &'static str,
        /// What has no row or kind, by name.
        what: String,
    },
    /// The input is not one the operation takes, decided by this crate before solidcore is
    /// reached.
    Invalid {
        /// The operation, by this crate's name for it.
        operation: &'static str,
        /// What is wrong with the input.
        what: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Refused { operation, reason } => {
                write!(f, "{operation}: solidcore refused it: {reason}")
            }
            Error::CheckFailed { operation, report } => {
                write!(f, "{operation}: the result failed its check: {report}")
            }
            Error::Unconverted { operation, what } => {
                write!(
                    f,
                    "{operation}: the result holds {what}, which the kernel has no kind for"
                )
            }
            Error::Invalid { operation, what } => {
                write!(f, "{operation}: not an input it takes: {what}")
            }
        }
    }
}

impl std::error::Error for Error {}

/// A solid made by solidcore and checked as the kernel's body.
#[derive(Debug)]
pub struct Solid {
    brep: Brep,
    body: Body,
    /// The centre and radius the modeller made the solid with, when it is a ball.
    ball: Option<(P3, Steps)>,
}

impl Solid {
    /// The kernel's body of the solid, which `geom-check` passed.
    #[must_use]
    pub fn body(&self) -> &Body {
        &self.body
    }
}

/// Whether the balls of centre `a` and `b` and radius `ra` and `rb` (grid points and steps) have
/// spheres that meet nowhere, decided exactly in whole numbers: the centres' squared distance is
/// past the squared sum of the radii (apart), or short of the squared difference (one inside the
/// other, concentric included). Touching, crossing and one sphere twice are not apart. Every
/// coordinate is under 2^40 steps and a radius under 2^41, so each square and sum fits an i128;
/// past that the answer is `false`, never a guess.
fn spheres_apart((a, ra): (P3, Steps), (b, rb): (P3, Steps)) -> bool {
    let apart = || -> Option<bool> {
        let d = |p: geom_num::Coord, q: geom_num::Coord| {
            let t = i128::from(p.get()).checked_sub(i128::from(q.get()))?;
            t.checked_mul(t)
        };
        let dd = d(a.x, b.x)?
            .checked_add(d(a.y, b.y)?)?
            .checked_add(d(a.z, b.z)?)?;
        let (ra, rb) = (i128::from(ra.0), i128::from(rb.0));
        let sum = ra.checked_add(rb)?;
        let diff = ra.checked_sub(rb)?;
        Some(dd > sum.checked_mul(sum)? || dd < diff.checked_mul(diff)?)
    };
    apart().unwrap_or(false)
}

/// A corner of a [`Profile`]: a point given exactly as a fraction of grid steps of the modeller's
/// step, `num[i] / den` steps along each axis, within the grid's range. A grid point is the
/// corner with denominator 1 ([`From<P3>`]). A corner the caller derives, such as where two of a
/// profile's sides meet, is given as the fraction it is, never rounded to the grid or to an f64
/// first (decision 32.97).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corner {
    num: [i128; 3],
    den: i128,
}

impl Corner {
    /// The corner `num / den` steps, or `None` for a denominator that is not positive or a
    /// coordinate past the grid's range (more than 2^40 - 1 steps from zero).
    #[must_use]
    pub fn new(num: [i128; 3], den: i128) -> Option<Corner> {
        let max = i128::from(geom_num::grid::COORD_MAX).checked_mul(den)?;
        (den > 0 && num.iter().all(|c| c.unsigned_abs() <= max.unsigned_abs()))
            .then_some(Corner { num, den })
    }

    /// The corner as exact fractions of steps.
    fn exact(self) -> [BigRational; 3] {
        self.num
            .map(|c| BigRational::new(BigInt::from(c), BigInt::from(self.den)))
    }
}

impl From<P3> for Corner {
    fn from(p: P3) -> Corner {
        let c = |x: geom_num::Coord| i128::from(x.get());
        Corner {
            num: [c(p.x), c(p.y), c(p.z)],
            den: 1,
        }
    }
}

/// One hole of a [`Profile`], in the profile's plane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hole {
    /// A polygon, in order either way round.
    Polygon(Vec<Corner>),
    /// The circle of `radius` about the grid point `centre`.
    Circle {
        /// The circle's centre.
        centre: P3,
        /// The circle's radius.
        radius: Steps,
    },
}

/// A plane region: an outer polygon, in order either way round, and the holes inside it, every
/// one in the outer polygon's plane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// The outer polygon's corners.
    pub outer: Vec<Corner>,
    /// The holes.
    pub holes: Vec<Hole>,
}

/// `a - b`, exactly.
fn diff(a: &[BigRational; 3], b: &[BigRational; 3]) -> [BigRational; 3] {
    [&a[0] - &b[0], &a[1] - &b[1], &a[2] - &b[2]]
}

/// `a . b`, exactly.
fn dot(a: &[BigRational; 3], b: &[BigRational; 3]) -> BigRational {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Twice the vector area of the polygon `ps` of at least one corner (its Newell normal), in
/// squared steps, exactly, taken about its first corner.
fn newell(ps: &[Corner]) -> [BigRational; 3] {
    let o = ps[0].exact();
    let mut n = [
        BigRational::zero(),
        BigRational::zero(),
        BigRational::zero(),
    ];
    for w in ps.windows(2).skip(1) {
        let (a, b) = (diff(&w[0].exact(), &o), diff(&w[1].exact(), &o));
        n[0] += &a[1] * &b[2] - &a[2] * &b[1];
        n[1] += &a[2] * &b[0] - &a[0] * &b[2];
        n[2] += &a[0] * &b[1] - &a[1] * &b[0];
    }
    n
}

/// One piece of a meeting curve, held privately, read by its parameter.
#[derive(Debug)]
pub struct Piece {
    curve: Curve,
}

impl Piece {
    /// The piece's parameter interval, `(start, end)`.
    #[must_use]
    pub fn span(&self) -> (f64, f64) {
        let iv = self.curve.natural_interval();
        (iv.min(), iv.max())
    }

    /// The point at parameter `t`, in grid steps of the modeller's step.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore cannot evaluate the curve there.
    pub fn point_at(&self, modeller: &Modeller, t: f64) -> Result<[f64; 3], Error> {
        let q = self
            .curve
            .evaluate_point(&modeller.kernel, t)
            .map_err(|e| refused("point_at", &e))?;
        Ok(modeller.in_steps(q))
    }
}

/// Where two solids meet: the curves, and the points where they only touch.
#[derive(Debug, Default)]
pub struct Meeting {
    /// The meeting curves, each in one or more pieces.
    pub pieces: Vec<Piece>,
    /// The points where the solids touch without crossing, in grid steps.
    pub points: Vec<[f64; 3]>,
}

/// One solidcore kernel on one grid step, and every operation this crate gives.
pub struct Modeller {
    kernel: Kernel,
    step: GridStep,
}

impl fmt::Debug for Modeller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Modeller on {:?}", self.step)
    }
}

fn refused(operation: &'static str, e: &SolidError) -> Error {
    Error::Refused {
        operation,
        reason: e.to_string(),
    }
}

impl Modeller {
    /// A modeller with solidcore's default options, on the grid step `step`.
    #[must_use]
    pub fn new(step: GridStep) -> Modeller {
        Modeller {
            kernel: Kernel::new(&KernelOptions::default()),
            step,
        }
    }

    /// The grid step every point and length is in.
    #[must_use]
    pub const fn step(&self) -> GridStep {
        self.step
    }

    fn point(&self, p: P3) -> Point3 {
        let m = |c: geom_num::Coord| to_metres(c.get(), self.step);
        Point3::new(m(p.x), m(p.y), m(p.z))
    }

    /// The corner `c` in metres, each coordinate the f64 nearest its exact value (num-rational's
    /// conversion rounds once, correctly), as a grid point's is: a corner of denominator 1 gives
    /// what [`Modeller::point`] gives, bit for bit.
    fn corner(&self, c: Corner) -> Point3 {
        let per = BigInt::from(c.den) * 4_000_000_000_u64;
        let m = |x: i128| {
            // `None` is num-rational's answer for 0 / 0 alone, which a positive denominator
            // never is.
            BigRational::new(BigInt::from(x) * self.step.quarter_nm(), per.clone())
                .to_f64()
                .unwrap_or(f64::NAN)
        };
        Point3::new(m(c.num[0]), m(c.num[1]), m(c.num[2]))
    }

    #[allow(
        clippy::cast_possible_wrap,
        reason = "a length of the frame is at most 2^41 steps, far below i64's range"
    )]
    fn length(&self, l: Steps) -> f64 {
        to_metres(l.0 as i64, self.step)
    }

    fn in_steps(&self, q: Point3) -> [f64; 3] {
        let s = metres(self.step);
        [q.x / s, q.y / s, q.z / s]
    }

    /// A copy of `s`, for an operation that consumes its operand.
    fn copy(&mut self, operation: &'static str, s: &Solid) -> Result<Brep, Error> {
        brep_copy(&mut self.kernel, &s.brep).map_err(|e| refused(operation, &e))
    }

    /// Checks a body solidcore returned with its own check, converts it into the kernel's body,
    /// checks that with the kernel's checker, and wraps both.
    fn checked(&mut self, operation: &'static str, mut brep: Brep) -> Result<Solid, Error> {
        let report = check_brep(&mut self.kernel, &mut brep, CheckLevel::Level1)
            .map_err(|e| refused(operation, &e))?;
        if report.assert_valid != Some(true) {
            return Err(Error::CheckFailed {
                operation,
                report: format!(
                    "solidcore's own check: pointers {} assert_valid {:?}",
                    report.pointers_status, report.assert_valid
                ),
            });
        }
        let unconverted = |u: convert::Unconverted| Error::Unconverted {
            operation,
            what: u.0,
        };
        let body = if solidcore::topology::brep::p1::num_faces(&brep) == 0 {
            convert::empty().map_err(unconverted)?
        } else {
            // The record is written from a copy: solidcore adds the pcurves it lacks to the body
            // it writes, and the body returned stays as solidcore made it.
            let mut c = brep_copy(&mut self.kernel, &brep).map_err(|e| refused(operation, &e))?;
            let (record, order) = solidcore_convert::brep::to_record(&mut self.kernel, &mut c)
                .map_err(|e| Error::Unconverted {
                    operation,
                    what: format!("what solidcore's record refuses ({e:?})"),
                })?;
            let (body, exactness) =
                convert::body_of(&record, &order, self.step).map_err(unconverted)?;
            convert::checked(body, exactness).map_err(|r| match r {
                Refusal::Unconverted(u) => unconverted(u),
                Refusal::Failed { first, faults } => Error::CheckFailed {
                    operation,
                    report: format!("geom-check found {faults} faults, the first: {first}"),
                },
            })?
        };
        Ok(Solid {
            brep,
            body,
            ball: None,
        })
    }

    /// The box with its least corner at `corner` and sides `size` along x, y and z.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for a side that is not positive; [`Error::CheckFailed`] and
    /// [`Error::Unconverted`] as every result.
    pub fn cuboid(&mut self, corner: P3, size: [Steps; 3]) -> Result<Solid, Error> {
        let [x, y, z] = size.map(|l| self.length(l));
        let c = self.point(corner);
        let b = create_box(&mut self.kernel, c, x, y, z).map_err(|e| refused("cuboid", &e))?;
        self.checked("cuboid", b)
    }

    /// The ball of `radius` about `centre`.
    ///
    /// # Errors
    ///
    /// As [`Modeller::cuboid`].
    pub fn sphere(&mut self, centre: P3, radius: Steps) -> Result<Solid, Error> {
        let (c, r) = (self.point(centre), self.length(radius));
        let b = create_sphere(&mut self.kernel, c, r).map_err(|e| refused("sphere", &e))?;
        let ball = Some((centre, radius));
        self.checked("sphere", b).map(|s| Solid { ball, ..s })
    }

    /// The solid cylinder of `radius` whose base circle is centred at `base`, rising `height`
    /// along z.
    ///
    /// # Errors
    ///
    /// As [`Modeller::cuboid`].
    pub fn cylinder(&mut self, base: P3, radius: Steps, height: Steps) -> Result<Solid, Error> {
        let (o, r, h) = (self.point(base), self.length(radius), self.length(height));
        let b = create_cylinder(&mut self.kernel, o, r, h).map_err(|e| refused("cylinder", &e))?;
        self.checked("cylinder", b)
    }

    /// The solid cone or frustum whose base circle of `base_radius` is centred at `base`, rising
    /// `height` along z to a circle of `top_radius`.
    ///
    /// # Errors
    ///
    /// As [`Modeller::cuboid`].
    pub fn cone(
        &mut self,
        base: P3,
        base_radius: Steps,
        top_radius: Steps,
        height: Steps,
    ) -> Result<Solid, Error> {
        let (o, rb, rt, h) = (
            self.point(base),
            self.length(base_radius),
            self.length(top_radius),
            self.length(height),
        );
        let b = create_cone(&mut self.kernel, o, rb, rt, h).map_err(|e| refused("cone", &e))?;
        self.checked("cone", b)
    }

    /// What solidcore is handed for the round holes `circles` (each a grid point centre and a
    /// radius) of a profile whose outer polygon solidcore is handed as `outer`, turning
    /// positively about the extrusion's vector: for each circle, in metres, the origin and the x
    /// and y axes of the frame it is written on, and its radius. The origin is the centre and the
    /// x axis runs along the outer polygon's first edge; the y axis is the x axis crossed with the
    /// outer polygon's Newell normal, so that both lie in the profile's plane and the circle
    /// turns negatively about the vector.
    fn handed_circles(
        &self,
        outer: &[Corner],
        circles: &[(P3, Steps)],
    ) -> Vec<(Point3, Vec3, Vec3, f64)> {
        let unit = |v: [f64; 3]| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            Vec3::new(v[0] / l, v[1] / l, v[2] / l)
        };
        let (p0, p1) = (self.corner(outer[0]), self.corner(outer[1]));
        let xa = unit([p1.x - p0.x, p1.y - p0.y, p1.z - p0.z]);
        // The normal on the vector's side is the Newell normal of the outer polygon as solidcore
        // is handed it, turning positively about the vector. Only its direction is used, which
        // the f64 keeps to rounding; a component is at most about 2^83 squared steps, so it is
        // finite.
        let nu = unit(newell(outer).map(|c| c.to_f64().unwrap_or(f64::NAN)));
        let ya = Vec3::new(
            xa.y * nu.z - xa.z * nu.y,
            xa.z * nu.x - xa.x * nu.z,
            xa.x * nu.y - xa.y * nu.x,
        );
        circles
            .iter()
            .map(|&(centre, radius)| (self.point(centre), xa, ya, self.length(radius)))
            .collect()
    }

    /// The solid swept by the region `profile` along the vector `by` (whole steps along x, y and
    /// z), capped at both ends: chapter 13.4's Extrude with distance ends and no taper, made by
    /// solidcore's linear sweep. The loops' order of corners does not matter: the outer polygon is
    /// handed to solidcore counter-clockwise about `by` and each hole clockwise, decided exactly
    /// from the corners' exact fractions of steps. solidcore is handed each corner as the f64
    /// nearest it in metres, as it is every grid point (decision 32.97).
    ///
    /// # Errors
    ///
    /// [`Error::Invalid`] for an outer polygon or hole of fewer than three corners, a loop with
    /// no area, a corner or circle centre off the outer polygon's plane, or a vector in that
    /// plane; [`Error::Refused`] if solidcore refuses; [`Error::CheckFailed`] and
    /// [`Error::Unconverted`] as every result.
    pub fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Solid, Error> {
        const OP: &str = "extrude";
        let invalid = |what: &str| Error::Invalid {
            operation: OP,
            what: what.to_owned(),
        };
        let outer = &profile.outer;
        if outer.len() < 3 {
            return Err(invalid("an outer polygon of fewer than three corners"));
        }
        let by_q = by.map(|c| BigRational::from_integer(BigInt::from(c)));
        let n = newell(outer);
        if n.iter().all(Zero::is_zero) {
            return Err(invalid("an outer polygon with no area"));
        }
        if dot(&n, &by_q).is_zero() {
            return Err(invalid("a vector in the profile's plane"));
        }
        let o = outer[0].exact();
        let off = |p: Corner| !dot(&n, &diff(&p.exact(), &o)).is_zero();
        if outer.iter().any(|&p| off(p)) {
            return Err(invalid("an outer corner off the polygon's plane"));
        }
        // The loops as solidcore takes them: the outer one turning positively about `by`, each
        // hole negatively.
        let ccw = |ps: &[Corner], positive: bool| -> Result<Vec<Corner>, Error> {
            let few = || invalid("a hole of fewer than three corners or with no area");
            if ps.len() < 3 {
                return Err(few());
            }
            let turn = dot(&newell(ps), &by_q);
            if turn.is_zero() {
                return Err(few());
            }
            let mut q = ps.to_vec();
            if turn.is_positive() != positive {
                q.reverse();
            }
            Ok(q)
        };
        let mut curves: Vec<Curve> = Vec::new();
        let mut polygon = |m: &Modeller, ps: &[Corner]| -> Result<(), Error> {
            for (i, &a) in ps.iter().enumerate() {
                let b = ps[(i + 1) % ps.len()];
                let line =
                    create_line_segment(m.corner(a), m.corner(b)).map_err(|e| refused(OP, &e))?;
                curves.push(Curve::from(line));
            }
            Ok(())
        };
        let outer = ccw(outer, true)?;
        polygon(self, &outer)?;
        let mut circles = Vec::new();
        for hole in &profile.holes {
            match hole {
                Hole::Polygon(ps) => {
                    if ps.iter().any(|&p| off(p)) {
                        return Err(invalid("a hole's corner off the profile's plane"));
                    }
                    let q = ccw(ps, false)?;
                    polygon(self, &q)?;
                }
                Hole::Circle { centre, radius } => {
                    if off(Corner::from(*centre)) {
                        return Err(invalid("a circle's centre off the profile's plane"));
                    }
                    circles.push((*centre, *radius));
                }
            }
        }
        for (origin, x_axis, y_axis, radius) in self.handed_circles(&outer, &circles) {
            let frame = Axis2Placement::new(origin, x_axis, y_axis);
            let arc = BSplineCurve::create_circle_segment(
                3,
                &frame,
                radius,
                0.0,
                360.0,
                NurbCircleParam::Quadratic,
            )
            .map_err(|e| refused(OP, &e))?;
            curves.push(Curve::from(arc));
        }
        let v = by.map(|c| to_metres(c, self.step));
        let dir = Vec3::new(v[0], v[1], v[2]);
        let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let refs: Vec<Option<&Curve>> = curves.iter().map(Some).collect();
        let b = create_linear_sweep(&mut self.kernel, &refs, dir, dist, true)
            .map_err(|e| refused(OP, &e))?;
        self.checked(OP, b)
    }

    /// `a` united with `b`; neither is changed.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore refuses; [`Error::CheckFailed`] and [`Error::Unconverted`]
    /// as every result.
    pub fn union(&mut self, a: &Solid, b: &Solid) -> Result<Solid, Error> {
        let (x, y) = (self.copy("union", a)?, self.copy("union", b)?);
        let r = boolean_union(&mut self.kernel, x, y).map_err(|e| refused("union", &e))?;
        self.checked("union", r)
    }

    /// `a` less `b`; neither is changed.
    ///
    /// # Errors
    ///
    /// As [`Modeller::union`].
    pub fn difference(&mut self, a: &Solid, b: &Solid) -> Result<Solid, Error> {
        let (x, y) = (self.copy("difference", a)?, self.copy("difference", b)?);
        let r =
            boolean_difference(&mut self.kernel, x, y).map_err(|e| refused("difference", &e))?;
        self.checked("difference", r)
    }

    /// What `a` and `b` share; neither is changed.
    ///
    /// # Errors
    ///
    /// As [`Modeller::union`].
    pub fn intersection(&mut self, a: &Solid, b: &Solid) -> Result<Solid, Error> {
        let (x, y) = (self.copy("intersection", a)?, self.copy("intersection", b)?);
        let r = boolean_intersection(&mut self.kernel, x, y)
            .map_err(|e| refused("intersection", &e))?;
        self.checked("intersection", r)
    }

    /// The solid's volume, in cubic grid steps; the solid is not changed (solidcore's integrator
    /// keeps caches in the body it reads, so it reads a copy).
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore cannot integrate it.
    pub fn volume(&mut self, s: &Solid) -> Result<f64, Error> {
        let mut c = self.copy("volume", s)?;
        let m3 = brep_volume_default(&self.kernel, &mut c, VOLUME_ACCURACY)
            .map_err(|e| refused("volume", &e))?;
        let step = metres(self.step);
        Ok(m3 / step / step / step)
    }

    /// How many faces the solid's body has.
    #[must_use]
    pub fn face_count(&self, s: &Solid) -> u32 {
        s.body.arenas().faces.len()
    }

    /// The curves where the plane through `at` with normal `normal` cuts `s`; `s` is not changed.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore refuses.
    pub fn plane_section(
        &mut self,
        s: &Solid,
        at: P3,
        normal: [f64; 3],
    ) -> Result<Vec<Piece>, Error> {
        let n = Vec3::new(normal[0], normal[1], normal[2]);
        self.section(s, self.point(at), n)
    }

    /// The curves where the plane `distance` steps from the grid point `from` along the
    /// whole-number normal `normal` cuts `s`; `s` is not changed. The plane is exactly the one
    /// meant, n . (x - from) = distance |n|, though it may hold no point of any grid (decision
    /// 32.99): solidcore is handed its point nearest `from`, each coordinate the f64 nearest its
    /// exact value in metres ([`Modeller::foot`]), as a grid point and a profile's corner are, and
    /// the normal exactly. Nothing is rounded before that one rounding.
    ///
    /// # Errors
    ///
    /// [`Error::Invalid`] for the normal (0, 0, 0) or a distance past the frame's largest
    /// clearance ([`geom_num::distance::MAX_CLEARANCE`], 2^42 steps); [`Error::Refused`] if
    /// solidcore refuses.
    pub fn plane_section_at_distance(
        &mut self,
        s: &Solid,
        from: P3,
        normal: [i32; 3],
        distance: Steps,
    ) -> Result<Vec<Piece>, Error> {
        let invalid = |what: &str| Error::Invalid {
            operation: "plane_section",
            what: what.to_owned(),
        };
        if normal == [0; 3] {
            return Err(invalid("the normal (0, 0, 0)"));
        }
        if distance.0 > geom_num::distance::MAX_CLEARANCE {
            return Err(invalid("a distance past the frame's largest clearance"));
        }
        let n = Vec3::new(
            f64::from(normal[0]),
            f64::from(normal[1]),
            f64::from(normal[2]),
        );
        self.section(s, self.foot(from, normal, distance), n)
    }

    /// The point `distance` steps from the grid point `from` along the normal `normal`, not
    /// (0, 0, 0), and at most 2^42 steps: in metres, each coordinate the f64 nearest its exact
    /// value, `from_i s + sign(n_i) sqrt(C_i)` with `s` the step in metres, `N = n . n` and
    /// `C_i = (distance s n_i)^2 / N`.
    ///
    /// No library rounds such a sum of a fraction and a square root correctly, so it is done here
    /// (exact maths the kernel's principles need): `l_i = from_i s + sign(n_i) floor(sqrt(C_i)
    /// 2^K) / 2^K` is within 2^-K of `x_i`, and num-rational rounds the fraction `l_i` once,
    /// correctly, to the f64 `x_i` rounds to, for no point where the rounding changes (a midpoint
    /// `b = m / 2^e` of two f64s, `e <= 1075`) is as near `x_i` as 2^-K = 2^-2560 nor is `x_i`:
    /// - `x_i` rational (`N` a square or `n_i = 0`): `x_i = M / (v 2^11 5^k)` for whole `M`, `v`
    ///   the odd denominator of `n_i / sqrt(N)` and `2^11 5^k` the step's metre (32.71). A
    ///   midpoint's odd numerator is at least 2^53 + 1 or its value below 2^-1022, but a dyadic
    ///   `x_i` has a numerator below `(2^40 + 2^42) / 5^5 < 2^43` and is 0 or at least 2^-11; and
    ///   `x_i != b` gives `|x_i - b| >= 1 / (v 2^32 2^e) >= 2^-1140`.
    /// - `x_i` irrational: with `D = b - from_i s`, `|x_i - b|` is at least `sqrt(C_i)` (over
    ///   2^-64) when `D` is 0 or of the other sign, and otherwise `|C_i - D^2| / (sqrt(C_i) + |D|)`,
    ///   where `C_i - D^2 != 0` has a denominator at most 2^128 2^(2e + 64) and `sqrt(C_i) + |D|`
    ///   is below 2^21 when `|x_i - b| < 1`: so `|x_i - b| >= 2^-(213 + 2e) >= 2^-2363`.
    fn foot(&self, from: P3, normal: [i32; 3], distance: Steps) -> Point3 {
        /// The bits kept after the binary point of each square root.
        const K: usize = 2560;
        let s = BigRational::new(
            BigInt::from(self.step.quarter_nm()),
            BigInt::from(4_000_000_000_u64),
        );
        let nn = BigRational::from_integer(normal.iter().map(|&c| BigInt::from(c) * c).sum());
        let along = &s * BigInt::from(distance.0);
        let m = |p: geom_num::Coord, c: i32| {
            let r = &along * BigInt::from(c);
            let sq = &r * &r / &nn;
            let root = ((sq.numer() << (2 * K)) / sq.denom()).sqrt();
            let x = &s * BigInt::from(p.get())
                + BigRational::new(root * c.signum(), BigInt::one() << K);
            // `None` is num-rational's answer for 0 / 0 alone, which a denominator 2^K never is.
            x.to_f64().unwrap_or(f64::NAN)
        };
        Point3::new(
            m(from.x, normal[0]),
            m(from.y, normal[1]),
            m(from.z, normal[2]),
        )
    }

    /// The curves where the plane through `at` (in metres) with normal `n` cuts `s`; `s` is not
    /// changed.
    fn section(&mut self, s: &Solid, at: Point3, n: Vec3) -> Result<Vec<Piece>, Error> {
        let mut c = self.copy("plane_section", s)?;
        let curves = intersect_brep_with_plane(&self.kernel, &mut c, at, n)
            .map_err(|e| refused("plane_section", &e))?;
        Ok(curves.into_iter().map(|curve| Piece { curve }).collect())
    }

    /// Where the boundaries of `a` and `b` meet; neither is changed. `with_points` asks for the
    /// points where they only touch as well.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore reports a failure, whatever it found before it, but for two
    /// balls whose spheres the kernel's own exact decision finds apart (decision 32.71): solidcore
    /// reports two solids that do not meet as a failure (SC-2), and for those the answer is that
    /// they meet nowhere.
    pub fn meet(&mut self, a: &Solid, b: &Solid, with_points: bool) -> Result<Meeting, Error> {
        let (mut x, mut y) = (self.copy("meet", a)?, self.copy("meet", b)?);
        let (found, status) =
            intersect_breps(&self.kernel, BrepOperands::Two(&mut x, &mut y), with_points);
        if let Err(e) = status {
            return match (a.ball, b.ball) {
                (Some(p), Some(q)) if spheres_apart(p, q) => Ok(Meeting {
                    pieces: Vec::new(),
                    points: Vec::new(),
                }),
                _ => Err(refused("meet", &e)),
            };
        }
        Ok(Meeting {
            pieces: found
                .curves
                .into_iter()
                .map(|curve| Piece { curve })
                .collect(),
            points: found
                .points
                .unwrap_or_default()
                .into_iter()
                .map(|p| self.in_steps(p))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grid step of the stored cases, 6.25 nm.
    const STEP: GridStep = GridStep::Q1;

    /// `h` half-metres in steps of [`STEP`].
    fn halves(h: i32) -> i64 {
        let per = STEP
            .steps_per(2_000_000_000)
            .expect("a whole number of steps");
        i64::from(h) * i64::try_from(per).expect("a small number")
    }

    /// The grid point `h` half-metres along x, y and z.
    fn at(h: [i32; 3]) -> P3 {
        let [x, y, z] = h.map(halves);
        P3::new(x, y, z).expect("a grid point")
    }

    /// The radius of `h` half-metres.
    fn radius(h: i32) -> Steps {
        Steps(halves(h).unsigned_abs())
    }

    fn xyz(v: Vec3) -> [f64; 3] {
        [v.x, v.y, v.z]
    }

    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    /// What solidcore is handed for a round hole is the circle the profile means, with nothing
    /// refused before solidcore: bldg-47's square of side 2 m with a round hole of radius 0.5 m at
    /// its centre, at z = 0 and 1 m, swept 3 m up, is neither refused as an input nor refused by
    /// solidcore. And for each outer polygon below, given as solidcore is handed it (turning
    /// positively about the vector), each circle's frame has its origin exactly at the circle's
    /// centre and the radius exactly its radius, in metres; its x axis is exactly the unit vector
    /// along the polygon's first edge, and its y axis exactly the one that makes x cross y the
    /// opposite of the polygon's unit Newell normal, so the circle lies in the profile's plane and
    /// turns negatively about the vector. The polygons are bldg-47's square at both heights and
    /// squares of side 3 m off the origin, on a plane square to each axis in turn, with their first
    /// edge along each axis in turn and once against one.
    #[test]
    fn a_round_hole_is_handed_to_solidcore_on_its_frame() {
        let mut m = Modeller::new(STEP);
        for z in [0, 2] {
            let p = |x, y| Corner::from(at([x, y, z]));
            let profile = Profile {
                outer: vec![p(-2, -2), p(2, -2), p(2, 2), p(-2, 2)],
                holes: vec![Hole::Circle {
                    centre: at([0, 0, z]),
                    radius: radius(1),
                }],
            };
            let found = m.extrude(&profile, [0, 0, halves(6)]);
            assert!(
                !matches!(found, Err(Error::Invalid { .. } | Error::Refused { .. })),
                "at z {z}: {found:?}"
            );
        }
        // Each row: the outer polygon's corners, its circles (centre and radius), the expected x
        // and y axes and the polygon's unit Newell normal, all in half-metres but the axes.
        type Row = (
            [[i32; 3]; 4],
            Vec<([i32; 3], i32)>,
            [f64; 3],
            [f64; 3],
            [f64; 3],
        );
        let rows: [Row; 8] = [
            // bldg-47's square at z = 0 and at 1 m.
            (
                [[-2, -2, 0], [2, -2, 0], [2, 2, 0], [-2, 2, 0]],
                vec![([0, 0, 0], 1)],
                [1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            (
                [[-2, -2, 2], [2, -2, 2], [2, 2, 2], [-2, 2, 2]],
                vec![([0, 0, 2], 1)],
                [1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            // On z = 0, turning positively about z: first edge along x, then along y.
            (
                [[-2, -2, 0], [4, -2, 0], [4, 4, 0], [-2, 4, 0]],
                vec![([1, 1, 0], 2), ([2, 2, 0], 1)],
                [1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            (
                [[4, -2, 0], [4, 4, 0], [-2, 4, 0], [-2, -2, 0]],
                vec![([1, 1, 0], 2)],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            // On z = 0, turning positively about -z (swept down): first edge along x.
            (
                [[-2, 4, 0], [4, 4, 0], [4, -2, 0], [-2, -2, 0]],
                vec![([1, 1, 0], 2)],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, -1.0],
            ),
            // On y = 1 m, turning positively about -y: first edge along z.
            (
                [[0, 2, 0], [0, 2, 6], [-6, 2, 6], [-6, 2, 0]],
                vec![([-3, 2, 3], 2)],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
            ),
            // On x = 1 m, turning positively about x: first edge along y.
            (
                [[2, 0, 0], [2, 6, 0], [2, 6, 6], [2, 0, 6]],
                vec![([2, 3, 3], 2)],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, -1.0],
                [1.0, 0.0, 0.0],
            ),
            // On x = 1 m, turning positively about -x: first edge against z.
            (
                [[2, 6, 6], [2, 6, 0], [2, 0, 0], [2, 0, 6]],
                vec![([2, 3, 3], 2)],
                [0.0, 0.0, -1.0],
                [0.0, 1.0, 0.0],
                [-1.0, 0.0, 0.0],
            ),
        ];
        for (corners, circles, x, y, normal) in rows {
            let outer = corners.map(|h| Corner::from(at(h)));
            let given: Vec<(P3, Steps)> =
                circles.iter().map(|&(c, r)| (at(c), radius(r))).collect();
            let handed = m.handed_circles(&outer, &given);
            assert_eq!(handed.len(), circles.len(), "{corners:?}");
            for ((origin, xa, ya, r), (centre, half)) in handed.into_iter().zip(&circles) {
                assert_eq!(
                    xyz(origin),
                    centre.map(|h| f64::from(h) / 2.0),
                    "{corners:?}"
                );
                assert_eq!(r, f64::from(*half) / 2.0, "{corners:?}");
                assert_eq!((xyz(xa), xyz(ya)), (x, y), "{corners:?}");
                assert_eq!(cross(xyz(xa), xyz(ya)), normal.map(|c| -c), "{corners:?}");
            }
        }
    }

    /// Whether `y` is the f64 nearest `x = a + sign(n) sqrt(c)`, by exact comparison alone: `x`
    /// lies between the midpoints of `y` and its two neighbours.
    fn is_nearest(y: f64, a: &BigRational, n: i32, c: &BigRational) -> bool {
        // Whether x < t.
        let below = |t: &BigRational| {
            let u = t - a;
            match n.signum() {
                0 => u.is_positive(),
                1 => u.is_positive() && *c < &u * &u,
                _ => !u.is_negative() || *c > &u * &u,
            }
        };
        let q = |v: f64| BigRational::from_float(v).expect("a finite f64");
        let two = BigRational::from_integer(BigInt::from(2));
        let lo = (q(y.next_down()) + q(y)) / &two;
        let hi = (q(y) + q(y.next_up())) / &two;
        !below(&lo) && below(&hi)
    }

    /// The point solidcore is handed for a plane given by its distance (decision 32.99) is, in
    /// each coordinate, the f64 nearest its exact value `from_i s + sign(n_i) sqrt(C_i)`, by exact
    /// comparison: for isect-15 to isect-20's planes, `d / sqrt(3)` in each coordinate, one unit in
    /// the last place below the decimal the table wrote for it before; and on each grid, for
    /// planes from grid points off the origin (one at the edge of the frame) at distances up to
    /// the largest clearance, along normals with components of each sign, of zero and past one, of
    /// whole length and not.
    #[test]
    fn a_plane_s_point_is_the_f64_nearest_it() {
        let origin = P3::new(0, 0, 0).expect("a grid point");
        let m = Modeller::new(GridStep::Q1);
        // Each distance in steps of Q1 (1.6e8 to the metre) and the table's decimal of before.
        let six: [(u64, f64); 6] = [
            (48_000_000, 0.173_205_080_756_887_76),
            (96_000_000, 0.346_410_161_513_775_5),
            (144_000_000, 0.519_615_242_270_663_2),
            (159_840_000, 0.576_772_918_920_436_3),
            (160_000_000, 0.577_350_269_189_625_8),
            (192_000_000, 0.692_820_323_027_551),
        ];
        let zero = BigRational::zero();
        for (k, wrote) in six {
            let d = BigRational::new(BigInt::from(k), BigInt::from(160_000_000));
            let c = &d * &d / BigRational::from_integer(BigInt::from(3));
            let p = m.foot(origin, [1, 1, 1], Steps(k));
            for y in [p.x, p.y, p.z] {
                assert!(is_nearest(y, &zero, 1, &c), "{k}: {y}");
                assert_eq!(y, wrote.next_down(), "{k}");
            }
        }
        let edge = geom_num::grid::COORD_MAX;
        let froms = [
            P3::new(3, -5, 7).expect("a grid point"),
            P3::new(edge, -edge, 12_345).expect("a grid point"),
        ];
        let normals = [
            [1, 1, 1],
            [-1, 1, -1],
            [1, -2, 2],
            [0, 0, -1],
            [2, 0, -1],
            [-3, -1, 2],
            [i32::MAX, i32::MIN, 1],
        ];
        let far = geom_num::distance::MAX_CLEARANCE;
        for step in [GridStep::Q0, GridStep::Q1, GridStep::Q2] {
            let m = Modeller::new(step);
            let s = BigRational::new(
                BigInt::from(step.quarter_nm()),
                BigInt::from(4_000_000_000_u64),
            );
            for from in froms {
                for normal in normals {
                    let nn: BigInt = normal.iter().map(|&c| BigInt::from(c) * c).sum();
                    for k in [1, 48_000_000, far - 1, far] {
                        let p = m.foot(from, normal, Steps(k));
                        let given = [from.x, from.y, from.z];
                        for (i, y) in [p.x, p.y, p.z].into_iter().enumerate() {
                            let a = &s * BigInt::from(given[i].get());
                            let r = &s * BigInt::from(k) * BigInt::from(normal[i]);
                            let c = &r * &r / BigRational::from_integer(nn.clone());
                            assert!(
                                is_nearest(y, &a, normal[i], &c),
                                "{step:?} {from:?} {normal:?} {k}: {i} {y}"
                            );
                        }
                    }
                }
            }
        }
    }
}
