//! Fuzz target: a torus's special positions say what is so.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a ring torus at the origin about
//! z, and a plane square to its axis or through it, or a sphere, cylinder, cone or second torus about
//! the same axis, on a coarse lattice so that touching is common. [`check`] asks where they meet in
//! closed form and holds the answer to the laws below. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: an answer
//!   that changes between two runs or with the order of the pair, a point of a curve off the torus by
//!   the torus's own equation, or a smaller budget that changes an answer instead of stopping it.
//!
//! Method: chapter 12.7 (`projects/engines/geometry/architecture/12-intersections.md`), with chapter
//! 28.5's oracle stack (`projects/engines/geometry/architecture/28-testing.md`): the torus's equation
//! is written here from the drawn numbers, sharing nothing with the reduction it judges.
//!
//! Cost: a curve-curve pair in the half-plane, up to [`WORK`]; at most [`MAX_INPUT_BYTES`] bytes
//! read.

use geom_curve::{Frame, ParamCurve};
use geom_intersect::quadrics::natural;
use geom_intersect::{IntersectError, Intersection, Item};
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind, Torus};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The work a case may take, and a stop there is an answer the laws accept.
pub const WORK: u64 = 1 << 20;

/// The most bytes [`decode`] reads from any input: 8.
///
/// Six for the pair, a seventh for the budget and an eighth kept for the rerun's kind: 6 + 2 = 8. The
/// unit test `the_input_bound_is_reached_and_never_passed` reads exactly that.
pub const MAX_INPUT_BYTES: usize = 8;

/// How far a point of a curve may stand from the torus by its equation: rings round, and a stretch
/// past a tangent point reads a little off.
const OFF: f64 = 1e-3;

/// A whole turn, in half-turns.
const TURN: (f64, f64) = (0.0, 2.0);

/// The half-width of a plane and the half-length of a cylinder or cone.
const SIDE: f64 = 2000.0;

/// One input of the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Case {
    /// Which partner, by its remainder.
    pub kind: u8,
    /// The torus's radii, as small numbers.
    pub radii: [u8; 2],
    /// The partner's size and height.
    pub at: [u8; 3],
    /// The work budget of the budgeted rerun, as a power of two.
    pub work: u8,
}

/// The case in `data`, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let kind = src.byte();
    let radii = [src.byte(), src.byte()];
    let at = [src.byte(), src.byte(), src.byte()];
    let work = src.byte();
    let _ = src.byte();
    let case = Case {
        kind,
        radii,
        at,
        work,
    };
    (case, data.len().saturating_sub(src.remaining()))
}

/// `made`, or the finding that a drawn value was not valid.
fn must<T, E: core::fmt::Debug>(made: Result<T, E>, law: &str) -> Result<T, Finding> {
    made.map_err(|e| Finding::new(law, || format!("{e:?}")))
}

/// The torus's major and minor radii: a ring.
fn radii(case: Case) -> (f64, f64) {
    let [a, b] = case.radii;
    let minor = f64::from(a % 8) + 1.0;
    (minor + f64::from(b % 32) + 1.0, minor)
}

/// The torus drawn.
fn torus(case: Case) -> Result<SurfaceKind, Finding> {
    let (major, minor) = radii(case);
    let frame = must(Frame::standard(), "the standard frame is valid")?;
    Ok(SurfaceKind::Torus(must(
        Torus::new([0.0; 3], major, minor, frame, (TURN, TURN)),
        "a drawn torus is valid",
    )?))
}

/// The partner drawn.
fn partner(case: Case) -> Result<SurfaceKind, Finding> {
    let [a, b, c] = case.at;
    let size = f64::from(a % 48) + 1.0;
    let height = f64::from(i8::from_ne_bytes([b])) / 8.0;
    let frame = must(Frame::standard(), "the standard frame is valid")?;
    let reach = (-SIDE, SIDE);
    Ok(match case.kind % 6 {
        0 => SurfaceKind::Plane(must(
            Plane::new(
                [0.0, 0.0, height],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                (reach, reach),
            ),
            "a drawn plane is valid",
        )?),
        1 => SurfaceKind::Plane(must(
            Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (reach, reach)),
            "a drawn plane is valid",
        )?),
        2 => SurfaceKind::Sphere(must(
            Sphere::new([0.0, 0.0, height], size, frame, (TURN, (-0.5, 0.5))),
            "a drawn sphere is valid",
        )?),
        3 => SurfaceKind::Cylinder(must(
            Cylinder::new([0.0; 3], size, frame, (TURN, reach)),
            "a drawn cylinder is valid",
        )?),
        4 => {
            let slope = f64::from(c % 4) * 0.5 + 0.5;
            SurfaceKind::Cone(must(
                Cone::new([0.0, 0.0, height], 0.0, slope, frame, (TURN, reach)),
                "a drawn cone is valid",
            )?)
        }
        _ => SurfaceKind::Torus(must(
            Torus::new(
                [0.0, 0.0, height],
                size + f64::from(c % 8) + 2.0,
                f64::from(c % 8) + 1.0,
                frame,
                (TURN, TURN),
            ),
            "a drawn torus is valid",
        )?),
    })
}

/// A meter for a case.
fn meter<'a>() -> WorkMeter<'a> {
    WorkMeter::new(Budget::new(WORK, 0))
}

/// What `result` says; a pair with no closed form, or a stop on the budget, is no answer.
fn answer(
    result: Result<Option<Intersection>, IntersectError>,
    case: Case,
) -> Result<Option<Intersection>, Finding> {
    match result {
        Ok(a) => Ok(a),
        Err(IntersectError::BudgetExceeded) => Ok(None),
        Err(e) => Err(Finding::new("a valid pair is answered", || {
            format!("{e:?}, {case:?}")
        })),
    }
}

/// How far `p` stands from the torus, by (ρ² + z² + R² − r²)² − 4R²ρ², over the size of that form.
fn off_torus(p: [f64; 3], major: f64, minor: f64) -> f64 {
    let [x, y, z] = p;
    let rho2 = x * x + y * y;
    let sum = rho2 + z * z + major * major - minor * minor;
    (sum * sum - 4.0 * major * major * rho2) / (8.0 * major * major * minor)
}

/// Every curve lies on the torus.
fn curves_are_on_it(found: &Intersection, case: Case) -> Result<(), Finding> {
    let (major, minor) = radii(case);
    for item in found.items() {
        let Item::Curve { curve, .. } = item else {
            continue;
        };
        let (lo, hi) = curve.domain();
        for share in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let t = lo * (1.0 - share) + hi * share;
            let p = must(
                curve.point(t, Limit::FromRight),
                "a curve is read in its domain",
            )?;
            if off_torus(p, major, minor).abs() > OFF {
                return Err(Finding::new("a curve lies on the torus", || {
                    format!("{p:?}, {case:?}")
                }));
            }
        }
    }
    Ok(())
}

/// The laws, on one case.
///
/// # Errors
///
/// A [`Finding`] naming the law broken and the case.
pub fn check(case: Case) -> Result<(), Finding> {
    let (a, b) = (partner(case)?, torus(case)?);
    let Some(found) = answer(natural(&a, &b, &mut meter()), case)? else {
        return Ok(());
    };
    let again = answer(natural(&a, &b, &mut meter()), case)?;
    if again.as_ref().is_none_or(|x| x.items() != found.items()) {
        return Err(Finding::new("an answer is the same twice", || {
            format!("{case:?}")
        }));
    }
    if let Some(swapped) = answer(natural(&b, &a, &mut meter()), case)?
        && swapped.items().len() != found.items().len()
    {
        return Err(Finding::new("an answer does not hang on the order", || {
            format!("{case:?}")
        }));
    }
    curves_are_on_it(&found, case)?;

    let budget = 1_u64 << (case.work % 20);
    let mut small = WorkMeter::new(Budget::new(budget, 0));
    match natural(&a, &b, &mut small) {
        Ok(Some(rerun)) if rerun.items() == found.items() => Ok(()),
        Err(IntersectError::BudgetExceeded) => Ok(()),
        other => Err(Finding::new(
            "a budget stops an answer or leaves it",
            || format!("{other:?}, {budget}, {case:?}"),
        )),
    }
}

/// [`decode`] then [`check`].
///
/// # Errors
///
/// As [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(decode(data).0)
}
