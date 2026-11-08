//! Fuzz target: `geom-intersect` says where any curve meets any surface, and what it says is so.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a curve, a segment or cubic, and a
//! surface, a plane, sphere, cylinder, cone, torus or biquadratic graph, on a coarse lattice so that
//! tangencies and lines lying in surfaces are common. [`check`] asks where they meet and holds the
//! answer to the laws below. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: an answer
//!   that changes between two runs, a point where the curve and surface are apart, a segment across
//!   a graph that changes sides between its ends with no item on it, or a smaller budget that changes
//!   an answer instead of stopping it.
//!
//! Method: chapter 12.6 (`projects/engines/geometry/architecture/12-intersections.md`), with chapter
//! 28.5's oracle stack (`projects/engines/geometry/architecture/28-testing.md`): the sides are read
//! from the graph's Bernstein sum, which shares nothing with the solver it judges.
//!
//! Cost: a closed form, or the engine up to [`WORK`]; at most [`MAX_INPUT_BYTES`] bytes read.

#![allow(
    clippy::many_single_char_names,
    reason = "a case's drawn numbers are short names, as its fields are read off in order"
)]

use geom_curve::tight::{larger, smaller};
use geom_curve::{CurveKind, Frame, Line, ParamCurve};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::{IntersectError, Intersection, Item};
use geom_nurbs::{Basis, Bounds, Curve, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind, Torus};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The work a case may take: a curve lying near a surface walks its length at the floor, and a stop
/// there is an answer the laws accept.
pub const WORK: u64 = 1 << 24;

/// The most bytes [`decode`] reads from any input: 16.
///
/// A curve of 7 bytes, a surface of 7 and two for the budget and the rerun's kind: 7 + 7 + 2 = 16.
/// The unit test `the_input_bound_is_reached_and_never_passed` reads exactly that.
pub const MAX_INPUT_BYTES: usize = 16;

/// How far apart a point's two boxes may stand: a step each way for the floor's enclosures.
const SLACK: f64 = 2.0;

/// The span of a graph.
const SPAN: f64 = 900.0;

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
    /// The curve.
    pub curve: Drawn,
    /// The surface.
    pub surface: Drawn,
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
    let (curve, surface) = (drawn(), drawn());
    let work = src.byte();
    let _ = src.byte();
    let case = Case {
        curve,
        surface,
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

/// A size of at least one.
fn size(x: u8) -> f64 {
    f64::from(x) + 1.0
}

/// The curve drawn: a segment, or a cubic across the same span.
fn curve(d: Drawn) -> Result<CurveKind<3>, Finding> {
    let [a, b, c, e, f, g] = d.at;
    if d.kind % 2 == 0 {
        let first = [lattice(a), lattice(b), lattice(c)];
        let mut last = [lattice(e), lattice(f), lattice(g)];
        if first == last {
            last = [lattice(e) + 1.0, lattice(f), lattice(g)];
        }
        let line = must(Line::segment(first, last), "a drawn segment is valid")?;
        return Ok(CurveKind::Line(line.value));
    }
    let points = [
        [0.0, lattice(a), lattice(e)],
        [SPAN / 3.0, lattice(b), lattice(f)],
        [SPAN * 2.0 / 3.0, lattice(c), lattice(g)],
        [SPAN, lattice(a), lattice(g)],
    ];
    let basis = must(
        Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]),
        "a cubic's basis is valid",
    )?;
    Ok(CurveKind::Nurbs(must(
        Curve::new(basis, &points),
        "a drawn cubic is valid",
    )?))
}

/// The heights of a graph's net, row by row.
fn net(d: Drawn) -> [f64; 9] {
    let [a, b, c, e, f, g] = d.at;
    let h = |x: u8| lattice(x) * 2.0;
    [h(a), h(b), h(c), h(e), h(f), h(g), h(a), h(c), h(e)]
}

/// The surface drawn.
fn surface(d: Drawn) -> Result<SurfaceKind, Finding> {
    let [a, b, c, e, f, g] = d.at;
    let centre = [lattice(a), lattice(b), lattice(c)];
    let frame = must(Frame::standard(), "the standard frame is valid")?;
    let turn = (0.0, 2.0);
    let side = (-300.0, 300.0);
    Ok(match d.kind % 6 {
        0 => {
            let u = [size(e), 0.0, lattice(f) / 8.0];
            let v = [0.0, size(g), 0.0];
            SurfaceKind::Plane(must(
                Plane::new(centre, u, v, (side, side)),
                "a drawn plane is valid",
            )?)
        }
        1 => SurfaceKind::Sphere(must(
            Sphere::new(centre, size(e), frame, (turn, (-0.5, 0.5))),
            "a drawn sphere is valid",
        )?),
        2 => SurfaceKind::Cylinder(must(
            Cylinder::new(centre, size(e), frame, (turn, side)),
            "a drawn cylinder is valid",
        )?),
        3 => SurfaceKind::Cone(must(
            Cone::new(
                centre,
                size(e),
                lattice(f) / 64.0 + 0.5,
                frame,
                (turn, side),
            ),
            "a drawn cone is valid",
        )?),
        4 => SurfaceKind::Torus(must(
            Torus::new(
                centre,
                larger(size(e), size(f)) + 1.0,
                smaller(size(e), size(f)) * 0.5,
                frame,
                (turn, turn),
            ),
            "a drawn torus is valid",
        )?),
        _ => {
            let h = net(d);
            let mut points = Vec::with_capacity(9);
            for (k, z) in (0_u32..).zip(h) {
                let (i, j) = (f64::from(k / 3), f64::from(k % 3));
                points.push([SPAN * 0.5 * i, SPAN * 0.5 * j, z]);
            }
            let basis = || Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
            let (u, v) = (
                must(basis(), "a graph's basis is valid")?,
                must(basis(), "a graph's basis is valid")?,
            );
            SurfaceKind::Nurbs(must(Surface::new(u, v, &points), "a drawn graph is valid")?)
        }
    })
}

/// A meter for a case.
fn meter<'a>() -> WorkMeter<'a> {
    WorkMeter::new(Budget::new(WORK, 0))
}

/// `result`, with a stop on the budget read as no answer.
fn answer(
    result: Result<Intersection, IntersectError>,
    case: &Case,
) -> Result<Option<Intersection>, Finding> {
    match result {
        Ok(a) => Ok(Some(a)),
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

/// The three spans of `item`'s box.
fn spans(item: &Item) -> Result<[(f64, f64); 3], Finding> {
    match item.at().spans() {
        [t, u, v, ..] => Ok([(t.lo(), t.hi()), (u.lo(), u.hi()), (v.lo(), v.hi())]),
        _ => Err(Finding::new("a curve and surface's box has three", || {
            format!("{item:?}")
        })),
    }
}

/// Every proven point lies on both.
fn points_are_meetings(
    a: &CurveKind<3>,
    b: &SurfaceKind,
    found: &Intersection,
    case: &Case,
) -> Result<(), Finding> {
    for item in found.items() {
        if !matches!(item, Item::Point { .. }) {
            continue;
        }
        let [t, u, v] = spans(item)?;
        let mut work = meter();
        let x = must(a.bound(t, &mut work), "a point's box is in the domain")?;
        let y = must(b.bound((u, v), &mut work), "a point's box is in the domain")?;
        let gap = apart(&x, &y);
        if gap > SLACK {
            return Err(Finding::new("a point lies on both", || {
                format!("{gap} apart at {item:?}, {case:?}")
            }));
        }
    }
    Ok(())
}

/// A graph's height at (u, v), by its Bernstein sum.
fn height(h: &[f64; 9], u: f64, v: f64) -> f64 {
    let b = |t: f64| [(1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t];
    let mut out = 0.0;
    for (k, z) in (0_usize..).zip(h) {
        let (i, j) = (k / 3, k % 3);
        out += b(u).get(i).copied().unwrap_or(0.0) * b(v).get(j).copied().unwrap_or(0.0) * z;
    }
    out
}

/// A segment with both ends over a graph, on opposite sides of it, has an item.
fn no_side_change_is_lost(
    a: &CurveKind<3>,
    case: &Case,
    found: &Intersection,
) -> Result<(), Finding> {
    if case.surface.kind % 6 != 5 || !found.items().is_empty() {
        return Ok(());
    }
    let CurveKind::Line(line) = a else {
        return Ok(());
    };
    let (lo, hi) = ParamCurve::domain(line);
    let (o, d) = (line.origin(), line.direction());
    let h = net(case.surface);
    let side = |t: f64| {
        let [x, y, z] = o;
        let [p, q, r] = d;
        let (u, v) = ((x + t * p) / SPAN, (y + t * q) / SPAN);
        ((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v)).then(|| z + t * r - height(&h, u, v))
    };
    if let (Some(first), Some(last)) = (side(lo), side(hi))
        && first * last < 0.0
        && first.abs() > 1e-3
        && last.abs() > 1e-3
    {
        // The segment stays over the unit square, which is convex.
        return Err(Finding::new("a change of sides has an item", || {
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
    let (a, b) = (curve(case.curve)?, surface(case.surface)?);
    let Some(found) = answer(curve_surface(&a, &b, &mut meter()), case)? else {
        return Ok(());
    };
    let again = answer(curve_surface(&a, &b, &mut meter()), case)?;
    if again.as_ref().is_none_or(|x| x.items() != found.items()) {
        return Err(Finding::new("an answer is the same twice", || {
            format!("{case:?}")
        }));
    }
    points_are_meetings(&a, &b, &found, case)?;
    no_side_change_is_lost(&a, case, &found)?;

    let budget = 1_u64 << (case.work % 24);
    let mut small = WorkMeter::new(Budget::new(budget, 0));
    match curve_surface(&a, &b, &mut small) {
        Ok(rerun) if rerun.items() != found.items() => Err(Finding::new(
            "a budget stops an answer or leaves it",
            || format!("{budget}, {case:?}"),
        )),
        Ok(_) | Err(IntersectError::BudgetExceeded) => Ok(()),
        Err(e) => Err(Finding::new(
            "a budget stops an answer or leaves it",
            || format!("{e:?}, {case:?}"),
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
