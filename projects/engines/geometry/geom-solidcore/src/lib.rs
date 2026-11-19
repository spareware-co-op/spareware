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
//!   and lengths of the frame (`geom_num::P3`, `geom_num::Steps`) on the [`Modeller`]'s step.
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
//! [`Modeller::cylinder`], [`Modeller::cone`]), the three booleans ([`Modeller::union`],
//! [`Modeller::difference`], [`Modeller::intersection`]), a solid cut by a plane
//! ([`Modeller::plane_section`]) and two solids met ([`Modeller::meet`]), and reads a solid's
//! volume and face count. Lengths reach solidcore in metres, the unit of its tolerances: a length
//! of whole steps becomes the f64 nearest its decimal value ([`convert::to_metres`]).
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
use solidcore::boolean::api_brep::{boolean_difference, boolean_intersection, boolean_union};
use solidcore::curve::curve::Curve;
use solidcore::intersect::api_intersectors::{
    BrepOperands, intersect_brep_with_plane, intersect_breps,
};
use solidcore::mass::integrator::properties::brep_volume_default;
use solidcore::primitive::api_primitives::{
    create_box, create_cone, create_cylinder, create_sphere,
};
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
        let mut c = self.copy("plane_section", s)?;
        let n = Vec3::new(normal[0], normal[1], normal[2]);
        let curves = intersect_brep_with_plane(&self.kernel, &mut c, self.point(at), n)
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
