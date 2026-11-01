//! Fuzz target: `geom-intersect` says where two natural quadrics meet, and what it says is so.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: two surfaces, each a plane, sphere,
//! cylinder or cone, on a coarse lattice with axes from a short list, so that coaxial, parallel,
//! tangent and same-apex pairs are common. [`check`] asks where they meet in closed form and holds
//! the answer to the laws below. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: an answer
//!   that changes between two runs or with the order of the pair, a point of a curve off either
//!   surface or past either's domain, a point where the surfaces are apart, a surface that crosses a
//!   sphere with no item, or a smaller budget that changes an answer instead of stopping it.
//!
//! Method: chapter 12.7 (`projects/engines/geometry/architecture/12-intersections.md`), with chapter
//! 28.5's oracle stack (`projects/engines/geometry/architecture/28-testing.md`): each surface's
//! equation is written here from the drawn numbers, sharing nothing with the closed forms it judges.
//!
//! Cost: a few predicates and square roots, and a curve-surface pair for each edge, up to [`WORK`]; at
//! most [`MAX_INPUT_BYTES`] bytes read.

#![allow(
    clippy::many_single_char_names,
    reason = "a case's drawn numbers are short names, as its fields are read off in order"
)]

use geom_curve::tight::larger;
use geom_curve::{Frame, ParamCurve};
use geom_intersect::quadrics::natural;
use geom_intersect::{IntersectError, Intersection, Item};
use geom_nurbs::{Bounds, Limit};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The work a case may take: an edge of a surface against the other, and a stop there is an answer
/// the laws accept.
pub const WORK: u64 = 1 << 22;

/// The most bytes [`decode`] reads from any input: 16.
///
/// Two surfaces of 7 bytes and two for the budget and the rerun's kind: 7 + 7 + 2 = 16. The unit test
/// `the_input_bound_is_reached_and_never_passed` reads exactly that.
pub const MAX_INPUT_BYTES: usize = 16;

/// How far apart a point's two boxes may stand: a step each way for the floor's enclosures.
const SLACK: f64 = 2.0;

/// How far a point of a curve may stand from a surface: the conics' data round, and a cone's
/// equation reads a little past a true distance by its slope.
const OFF: f64 = 1e-6;

/// The half-width of a plane, and of a cylinder or cone along its axis.
const SIDE: f64 = 300.0;

/// A whole turn, in half-turns.
const TURN: (f64, f64) = (0.0, 2.0);

/// The frames drawn from: a direction in the plane and the axis. The last is off the grid.
const FRAMES: [([f64; 3], [f64; 3]); 4] = [
    ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
    ([0.0, 0.0, 1.0], [0.6, 0.8, 0.0]),
];

/// The slopes drawn from.
const SLOPES: [f64; 4] = [0.5, 0.75, 1.0, 2.0];

/// One shape: which kind, and six numbers that place and size it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drawn {
    /// Which kind, by its remainder.
    pub kind: u8,
    /// Six small numbers.
    pub at: [u8; 6],
}

/// One input of the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Case {
    /// The first surface.
    pub first: Drawn,
    /// The second.
    pub second: Drawn,
    /// The work budget of the budgeted rerun, as a power of two.
    pub work: u8,
}

/// The case in `data`, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let mut drawn = || Drawn {
        kind: src.byte(),
        at: [
            src.byte(),
            src.byte(),
            src.byte(),
            src.byte(),
            src.byte(),
            src.byte(),
        ],
    };
    let (first, second) = (drawn(), drawn());
    let work = src.byte();
    let _ = src.byte();
    let case = Case {
        first,
        second,
        work,
    };
    (case, data.len().saturating_sub(src.remaining()))
}

/// `made`, or the finding that a drawn value was not valid.
fn must<T, E: core::fmt::Debug>(made: Result<T, E>, law: &str) -> Result<T, Finding> {
    made.map_err(|e| Finding::new(law, || format!("{e:?}")))
}

/// A lattice position: small and signed, so that shapes touch often.
fn lattice(x: u8) -> f64 {
    f64::from(i8::from_ne_bytes([x])) * 4.0
}

/// The drawn point.
fn centre(d: Drawn) -> [f64; 3] {
    let [a, b, c, ..] = d.at;
    [lattice(a), lattice(b), lattice(c)]
}

/// The drawn frame, as its direction and axis.
fn axes(d: Drawn) -> ([f64; 3], [f64; 3]) {
    let [.., f, _] = d.at;
    FRAMES
        .get(usize::from(f % 4))
        .copied()
        .unwrap_or(([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]))
}

/// The drawn radius: at least one.
fn radius(d: Drawn) -> f64 {
    let [.., e, _, _] = d.at;
    f64::from(e) + 1.0
}

/// The drawn cone: its radius where its parameter is zero, none on every other draw, and its slope.
fn flare(d: Drawn) -> (f64, f64) {
    let [.., g] = d.at;
    let start = if g % 2 == 0 { 0.0 } else { radius(d) };
    (
        start,
        SLOPES.get(usize::from(g / 2 % 4)).copied().unwrap_or(1.0),
    )
}

/// `a` · `b`.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    let [x, y, z] = a;
    let [p, q, r] = b;
    x * p + y * q + z * r
}

/// `a` − `b`.
fn minus(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = a;
    let [p, q, r] = b;
    [x - p, y - q, z - r]
}

/// The surface drawn.
fn surface(d: Drawn) -> Result<SurfaceKind, Finding> {
    let (x, n) = axes(d);
    let [a, b, c] = x;
    let [p, q, r] = n;
    let y = [q * c - r * b, r * a - p * c, p * b - q * a];
    let frame = must(Frame::new(x, n), "a drawn frame is valid")?;
    let side = (-SIDE, SIDE);
    Ok(match d.kind % 4 {
        0 => SurfaceKind::Plane(must(
            Plane::new(centre(d), x, y, (side, side)),
            "a drawn plane is valid",
        )?),
        1 => SurfaceKind::Sphere(must(
            Sphere::new(centre(d), radius(d), frame, (TURN, (-0.5, 0.5))),
            "a drawn sphere is valid",
        )?),
        2 => SurfaceKind::Cylinder(must(
            Cylinder::new(centre(d), radius(d), frame, (TURN, side)),
            "a drawn cylinder is valid",
        )?),
        _ => {
            let (start, slope) = flare(d);
            SurfaceKind::Cone(must(
                Cone::new(centre(d), start, slope, frame, (TURN, side)),
                "a drawn cone is valid",
            )?)
        }
    })
}

/// Which side of the surface drawn `p` lies on, in steps near it, by the surface's own equation:
/// positive outside.
fn off(d: Drawn, p: [f64; 3]) -> f64 {
    let (_, n) = axes(d);
    let w = minus(p, centre(d));
    let along = dot(w, n);
    let square = dot(w, w);
    match d.kind % 4 {
        0 => along,
        1 => {
            let r = radius(d);
            (square - r * r) / (2.0 * r)
        }
        2 => {
            let r = radius(d);
            (square - along * along - r * r) / (2.0 * r)
        }
        _ => {
            let (start, slope) = flare(d);
            let r = start + slope * along;
            (square - along * along - r * r) / (2.0 * r.abs() + 2.0)
        }
    }
}

/// Whether `p` lies in the domain drawn, by the frame.
fn in_domain(d: Drawn, p: [f64; 3]) -> bool {
    let (x, n) = axes(d);
    let w = minus(p, centre(d));
    let reach = SIDE + SLACK;
    match d.kind % 4 {
        0 => dot(w, x).abs() <= reach,
        1 => true,
        _ => dot(w, n).abs() <= reach,
    }
}

/// A meter for a case.
fn meter<'a>() -> WorkMeter<'a> {
    WorkMeter::new(Budget::new(WORK, 0))
}

/// What `result` says: no closed form or a stop on the budget are no answer.
fn answer(
    result: Result<Option<Intersection>, IntersectError>,
    case: &Case,
) -> Result<Option<Intersection>, Finding> {
    match result {
        Ok(a) => Ok(a),
        Err(IntersectError::BudgetExceeded) => Ok(None),
        Err(e) => Err(Finding::new("a valid pair is answered", || {
            format!("{e:?}, {case:?}")
        })),
    }
}

/// The largest gap between two boxes in any coordinate.
fn apart(a: &Bounds<3>, b: &Bounds<3>) -> f64 {
    let gap = |(lo, hi): (&f64, &f64), (other_lo, other_hi): (&f64, &f64)| {
        larger(larger(lo - other_hi, other_lo - hi), 0.0)
    };
    a.lo.iter()
        .zip(&a.hi)
        .zip(b.lo.iter().zip(&b.hi))
        .map(|(x, y)| gap(x, y))
        .fold(0.0, larger)
}

/// Every proven point lies on both.
fn points_are_meetings(
    a: &SurfaceKind,
    b: &SurfaceKind,
    item: &Item,
    case: &Case,
) -> Result<(), Finding> {
    let [u, v, s, t] = item.at().spans() else {
        return Err(Finding::new("a pair of surfaces' box has four", || {
            format!("{item:?}")
        }));
    };
    let mut work = meter();
    let x = must(
        a.bound(((u.lo(), u.hi()), (v.lo(), v.hi())), &mut work),
        "a point's box is in the domain",
    )?;
    let y = must(
        b.bound(((s.lo(), s.hi()), (t.lo(), t.hi())), &mut work),
        "a point's box is in the domain",
    )?;
    let gap = apart(&x, &y);
    if gap > SLACK {
        return Err(Finding::new("a point lies on both", || {
            format!("{gap} apart at {item:?}, {case:?}")
        }));
    }
    Ok(())
}

/// Every curve lies on both, within both domains.
fn curves_are_meetings(item: &Item, case: &Case) -> Result<(), Finding> {
    let Item::Curve { curve, .. } = item else {
        return Ok(());
    };
    let (lo, hi) = curve.domain();
    for share in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let t = lo * (1.0 - share) + hi * share;
        let p = must(
            curve.point(t, Limit::FromRight),
            "a curve is read in its domain",
        )?;
        for d in [case.first, case.second] {
            if off(d, p).abs() > OFF || !in_domain(d, p) {
                return Err(Finding::new(
                    "a curve lies on both, in both domains",
                    || format!("{p:?}, {d:?}, off by {}, {case:?}", off(d, p)),
                ));
            }
        }
    }
    Ok(())
}

/// A surface with points inside and outside a sphere crosses it, so there is an item.
fn no_crossing_is_lost(
    a: &SurfaceKind,
    b: &SurfaceKind,
    found: &Intersection,
    case: &Case,
) -> Result<(), Finding> {
    let (walked, ball) = if case.second.kind % 4 == 1 {
        (a, case.second)
    } else if case.first.kind % 4 == 1 {
        (b, case.first)
    } else {
        return Ok(());
    };
    if !found.items().is_empty() {
        return Ok(());
    }
    let ((u0, u1), (v0, v1)) = walked.domain();
    let share = |lo: f64, hi: f64, i: u32| lo + (hi - lo) * f64::from(i) / 8.0;
    let (mut inside, mut outside) = (false, false);
    let mut work = meter();
    for i in 0..=8 {
        for j in 0..=8 {
            let (u, v) = (share(u0, u1, i), share(v0, v1, j));
            let at = must(
                walked.bound(((u, u), (v, v)), &mut work),
                "a sample is in the domain",
            )?;
            let side = off(ball, at.lo);
            inside |= side < -1e-3;
            outside |= side > 1e-3;
        }
    }
    if inside && outside {
        return Err(Finding::new("a crossing of a sphere has an item", || {
            format!("{case:?}")
        }));
    }
    Ok(())
}

/// The laws, on one case.
///
/// # Errors
///
/// A [`Finding`] naming the law broken and the case.
pub fn check(case: &Case) -> Result<(), Finding> {
    let (a, b) = (surface(case.first)?, surface(case.second)?);
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
    for item in found.items() {
        if matches!(item, Item::Point { .. }) {
            points_are_meetings(&a, &b, item, case)?;
        }
        curves_are_meetings(item, case)?;
    }
    no_crossing_is_lost(&a, &b, &found, case)?;

    let budget = 1_u64 << (case.work % 24);
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
    check(&decode(data).0)
}
