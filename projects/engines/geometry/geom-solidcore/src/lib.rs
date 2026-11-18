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
//!   body privately, and every answer is given in plain coordinates and numbers.
//! - Operands are never consumed or changed: solidcore's operations take their operands by value
//!   and modify them, so each operation is handed copies made by solidcore's own `brep_copy`.
//! - Every body that comes back is checked before it is returned, and a body that fails the check
//!   is an [`Error::CheckFailed`], never a silent answer.
//! - Flat-faced booleans stay on the kernel's exact `geom-bool`; nothing calls this crate in their
//!   place (decision 32.70).
//!
//! Sections: 11, 12 and 29 of the kernel architecture, and decision 32.70
//! (`projects/engines/geometry/architecture/`).
//!
//! What it does: a [`Modeller`] owns one solidcore kernel. It makes the four primitives the stored
//! cases need ([`Modeller::cuboid`], [`Modeller::sphere`], [`Modeller::cylinder`],
//! [`Modeller::cone`]), the three booleans ([`Modeller::union`], [`Modeller::difference`],
//! [`Modeller::intersection`]), a solid cut by a plane ([`Modeller::plane_section`]) and two
//! solids met ([`Modeller::meet`]), and reads a solid's volume and face count.
//!
//! The check today is solidcore's own validity check (SMLib's `AssertValid`, at its first level):
//! the conversion of solidcore's bodies into `geom-topo`'s, so that `geom-check` can check them, is
//! not built yet (decision 32.70, what is not yet shown).
//!
//! Method: solidcore at commit cb9bee4bb9f56880c9973a9f8e99c83fdec24ad3 of the project's fork of
//! tangency-labs/solidcore (Apache-2.0); its methods are SMLib's.
//!
//! Cost: solidcore's, for each operation, plus one copy of each operand and one validity check of
//! each result. Recursion: solidcore's own, bounded by its README's 8 MiB stack (the workspace's
//! `.cargo/config.toml`).
//!
//! SPDX-License-Identifier: Apache-2.0

use std::fmt;

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
    /// solidcore returned a body that fails the check this crate runs on every result.
    CheckFailed {
        /// The operation, by this crate's name for it.
        operation: &'static str,
        /// What the check reported.
        report: String,
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
        }
    }
}

impl std::error::Error for Error {}

/// A solid made by solidcore, held privately.
#[derive(Debug)]
pub struct Solid {
    brep: Brep,
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

    /// The point at parameter `t`.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore cannot evaluate the curve there.
    pub fn point_at(&self, modeller: &Modeller, t: f64) -> Result<[f64; 3], Error> {
        let q = self
            .curve
            .evaluate_point(&modeller.kernel, t)
            .map_err(|e| refused("point_at", &e))?;
        Ok([q.x, q.y, q.z])
    }
}

/// Where two solids meet: the curves, and the points where they only touch.
#[derive(Debug, Default)]
pub struct Meeting {
    /// The meeting curves, each in one or more pieces.
    pub pieces: Vec<Piece>,
    /// The points where the solids touch without crossing.
    pub points: Vec<[f64; 3]>,
}

/// One solidcore kernel, and every operation this crate gives.
pub struct Modeller {
    kernel: Kernel,
}

impl fmt::Debug for Modeller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Modeller")
    }
}

impl Default for Modeller {
    fn default() -> Self {
        Modeller::new()
    }
}

fn refused(operation: &'static str, e: &SolidError) -> Error {
    Error::Refused {
        operation,
        reason: e.to_string(),
    }
}

fn point(p: [f64; 3]) -> Point3 {
    Point3::new(p[0], p[1], p[2])
}

impl Modeller {
    /// A modeller with solidcore's default options.
    #[must_use]
    pub fn new() -> Modeller {
        Modeller {
            kernel: Kernel::new(&KernelOptions::default()),
        }
    }

    /// A copy of `s`, for an operation that consumes its operand.
    fn copy(&mut self, operation: &'static str, s: &Solid) -> Result<Brep, Error> {
        brep_copy(&mut self.kernel, &s.brep).map_err(|e| refused(operation, &e))
    }

    /// Checks a body solidcore returned, and wraps it.
    fn checked(&mut self, operation: &'static str, mut brep: Brep) -> Result<Solid, Error> {
        let report = check_brep(&mut self.kernel, &mut brep, CheckLevel::Level1)
            .map_err(|e| refused(operation, &e))?;
        if report.assert_valid == Some(true) {
            Ok(Solid { brep })
        } else {
            Err(Error::CheckFailed {
                operation,
                report: format!(
                    "pointers {} assert_valid {:?}",
                    report.pointers_status, report.assert_valid
                ),
            })
        }
    }

    /// The box with one corner at `corner` and sides `size` along x, y and z.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] for a side that is not positive; [`Error::CheckFailed`] as every result.
    pub fn cuboid(&mut self, corner: [f64; 3], size: [f64; 3]) -> Result<Solid, Error> {
        let b = create_box(&mut self.kernel, point(corner), size[0], size[1], size[2])
            .map_err(|e| refused("cuboid", &e))?;
        self.checked("cuboid", b)
    }

    /// The ball of `radius` about `centre`.
    ///
    /// # Errors
    ///
    /// As [`Modeller::cuboid`].
    pub fn sphere(&mut self, centre: [f64; 3], radius: f64) -> Result<Solid, Error> {
        let b = create_sphere(&mut self.kernel, point(centre), radius)
            .map_err(|e| refused("sphere", &e))?;
        self.checked("sphere", b)
    }

    /// The solid cylinder of `radius` whose base circle is centred at `base`, rising `height`
    /// along z.
    ///
    /// # Errors
    ///
    /// As [`Modeller::cuboid`].
    pub fn cylinder(&mut self, base: [f64; 3], radius: f64, height: f64) -> Result<Solid, Error> {
        let b = create_cylinder(&mut self.kernel, point(base), radius, height)
            .map_err(|e| refused("cylinder", &e))?;
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
        base: [f64; 3],
        base_radius: f64,
        top_radius: f64,
        height: f64,
    ) -> Result<Solid, Error> {
        let b = create_cone(
            &mut self.kernel,
            point(base),
            base_radius,
            top_radius,
            height,
        )
        .map_err(|e| refused("cone", &e))?;
        self.checked("cone", b)
    }

    /// `a` united with `b`; neither is changed.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore refuses; [`Error::CheckFailed`] if its result fails the check.
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

    /// The solid's volume; the solid is not changed (solidcore's integrator keeps caches in the
    /// body it reads, so it reads a copy).
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore cannot integrate it.
    pub fn volume(&mut self, s: &Solid) -> Result<f64, Error> {
        let mut c = self.copy("volume", s)?;
        brep_volume_default(&self.kernel, &mut c, VOLUME_ACCURACY)
            .map_err(|e| refused("volume", &e))
    }

    /// How many faces the solid has.
    #[must_use]
    pub fn face_count(&self, s: &Solid) -> usize {
        solidcore::topology::brep::p1::num_faces(&s.brep)
    }

    /// The curves where the plane through `at` with normal `normal` cuts `s`; `s` is not changed.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore refuses.
    pub fn plane_section(
        &mut self,
        s: &Solid,
        at: [f64; 3],
        normal: [f64; 3],
    ) -> Result<Vec<Piece>, Error> {
        let mut c = self.copy("plane_section", s)?;
        let n = Vec3::new(normal[0], normal[1], normal[2]);
        let curves = intersect_brep_with_plane(&self.kernel, &mut c, point(at), n)
            .map_err(|e| refused("plane_section", &e))?;
        Ok(curves.into_iter().map(|curve| Piece { curve }).collect())
    }

    /// Where the boundaries of `a` and `b` meet; neither is changed. `with_points` asks for the
    /// points where they only touch as well.
    ///
    /// # Errors
    ///
    /// [`Error::Refused`] if solidcore reports a failure, whatever it found before it.
    pub fn meet(&mut self, a: &Solid, b: &Solid, with_points: bool) -> Result<Meeting, Error> {
        let (mut x, mut y) = (self.copy("meet", a)?, self.copy("meet", b)?);
        let (found, status) =
            intersect_breps(&self.kernel, BrepOperands::Two(&mut x, &mut y), with_points);
        status.map_err(|e| refused("meet", &e))?;
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
                .map(|p| [p.x, p.y, p.z])
                .collect(),
        })
    }
}
