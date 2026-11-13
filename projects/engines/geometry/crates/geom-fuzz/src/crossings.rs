//! Fuzz target: `geom-intersect` says where any two curves meet, and what it says is so.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: two curves, each a segment, a
//! circle or arc, an ellipse or a cubic graph over the same span, on a coarse lattice so that shared
//! ends, tangencies and copies are common. [`check`] asks where they meet and holds the answer to the
//! laws below. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: an answer
//!   that changes between two runs, a point where the curves are apart, two cubic graphs that change
//!   sides between two samples with no item between them, a pair asked either way round that finds a
//!   different number of proven points, or a smaller budget that changes an answer instead of
//!   stopping it. Self-crossings are held to the first two.
//!
//! Method: chapter 12.5 (`projects/engines/geometry/architecture/12-intersections.md`), with chapter
//! 28.5's oracle stack (`projects/engines/geometry/architecture/28-testing.md`): the sign changes are
//! read from the curves' Bernstein sums, which share nothing with the solver they judge.
//!
//! Cost: a closed form, or the engine up to [`WORK`]; at most [`MAX_INPUT_BYTES`] bytes read.

#![allow(
    clippy::many_single_char_names,
    reason = "a case's drawn numbers are short names, as its fields are read off in order"
)]

use geom_curve::tight::larger;
use geom_curve::{Axes, Circle, CurveKind, Ellipse, Frame, Line, ParamCurve};
use geom_intersect::curve::{curves, self_crossings};
use geom_intersect::{IntersectError, Intersection, Item};
use geom_nurbs::{Basis, Bounds, Curve};
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The work a case may take: a near-coincident pair walks its whole length at the floor, and a stop
/// there is an answer the laws accept.
pub const WORK: u64 = 1 << 24;

/// The most bytes [`decode`] reads from any input: 15.
///
/// Two curves of 7 bytes and one byte for the budget of the rerun: 2 · 7 + 1 = 15. The unit test
/// `the_input_bound_is_reached_and_never_passed` reads exactly that.
pub const MAX_INPUT_BYTES: usize = 15;

/// How far apart a point's two boxes may stand: a step each way for the floor's enclosures.
const SLACK: f64 = 2.0;

/// Samples of a cubic graph for the sign-change law.
const SAMPLES: u32 = 64;

/// The span of a cubic graph.
const SPAN: f64 = 900.0;

/// One curve: which kind, and six numbers that place and size it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drawn {
    /// Segment, circle, ellipse or cubic graph, by its remainder on division by four.
    pub kind: u8,
    /// Six small numbers.
    pub at: [u8; 6],
}

/// One input of the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Case {
    /// The two curves.
    pub drawn: [Drawn; 2],
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
    let drawn = [drawn(), drawn()];
    let work = src.byte();
    let case = Case { drawn, work };
    (case, data.len().saturating_sub(src.remaining()))
}

/// `made`, or the finding that a drawn value was not valid.
fn must<T, E: core::fmt::Debug>(made: Result<T, E>, law: &str) -> Result<T, Finding> {
    made.map_err(|e| Finding::new(law, || format!("{e:?}")))
}

/// A lattice position: small and signed, so ends and tangents coincide often.
fn lattice(x: u8) -> f64 {
    f64::from(i8::from_ne_bytes([x])) * 4.0
}

/// A size of at least one.
fn size(x: u8) -> f64 {
    f64::from(x) + 1.0
}

/// A cubic graph's control heights.
fn heights(d: Drawn) -> [f64; 4] {
    let [a, b, c, e, ..] = d.at;
    [a, b, c, e].map(|x| lattice(x) * 2.0)
}

/// The drawn curve.
fn curve(d: Drawn) -> Result<CurveKind<2>, Finding> {
    let [a, b, c, e, f, g] = d.at;
    let centre = [lattice(a), lattice(b)];
    Ok(match d.kind % 4 {
        0 => {
            let mut end = [lattice(c), lattice(e)];
            if end == centre {
                end = [lattice(c) + 1.0, lattice(e)];
            }
            CurveKind::Line(must(Line::segment(centre, end), "a drawn segment is valid")?.value)
        }
        1 => {
            let frame = must(Frame::standard(), "the standard frame is valid")?;
            let radius = size(c);
            let circle = if e % 2 == 0 {
                Circle::full(centre, radius, frame)
            } else {
                let start = f64::from(f) / 64.0;
                Circle::new(
                    centre,
                    radius,
                    frame,
                    (start, start + 1.0 + f64::from(g) / 256.0),
                )
            };
            CurveKind::Circle(must(circle, "a drawn circle is valid")?)
        }
        2 => {
            let axes = Axes::Vectors {
                u: [size(c), 0.0],
                v: [lattice(f) / 8.0, size(e)],
            };
            let ellipse = Ellipse::new(centre, axes, (0.0, 2.0));
            CurveKind::Ellipse(must(ellipse, "a drawn ellipse is valid")?)
        }
        _ => {
            let [p, q, r, s] = heights(d);
            let points = [[0.0, p], [SPAN / 3.0, q], [SPAN * 2.0 / 3.0, r], [SPAN, s]];
            let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
            let basis = must(Basis::new(3, knots), "a cubic's basis is valid")?;
            CurveKind::Nurbs(must(Curve::new(basis, &points), "a drawn cubic is valid")?)
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
        Err(e) => Err(Finding::new("a pair of valid curves is answered", || {
            format!("{e:?}, {case:?}")
        })),
    }
}

/// The largest gap between two boxes in any coordinate.
fn apart(a: &Bounds<2>, b: &Bounds<2>) -> f64 {
    let gap = |(lo, hi): (&f64, &f64), (other_lo, other_hi): (&f64, &f64)| {
        larger(larger(lo - other_hi, other_lo - hi), 0.0)
    };
    a.lo.iter()
        .zip(&a.hi)
        .zip(b.lo.iter().zip(&b.hi))
        .map(|(x, y)| gap(x, y))
        .fold(0.0, larger)
}

/// The first two spans of `item`'s box.
fn spans(item: &Item) -> Result<(f64, f64, f64, f64), Finding> {
    match item.at().spans() {
        [s, t, ..] => Ok((s.lo(), s.hi(), t.lo(), t.hi())),
        _ => Err(Finding::new("a curve pair's box has two unknowns", || {
            format!("{item:?}")
        })),
    }
}

/// Every proven point lies on both curves.
fn points_are_meetings(
    a: &CurveKind<2>,
    b: &CurveKind<2>,
    found: &Intersection,
    case: &Case,
) -> Result<(), Finding> {
    for item in found.items() {
        if !matches!(item, Item::Point { .. }) {
            continue;
        }
        let (s0, s1, t0, t1) = spans(item)?;
        let mut work = meter();
        let x = must(
            a.bound((s0, s1), &mut work),
            "a point's box is in the domain",
        )?;
        let y = must(
            b.bound((t0, t1), &mut work),
            "a point's box is in the domain",
        )?;
        let gap = apart(&x, &y);
        if gap > SLACK {
            return Err(Finding::new("a point lies on both curves", || {
                format!("{gap} apart at {item:?}, {case:?}")
            }));
        }
    }
    Ok(())
}

/// A cubic graph's height at parameter `t`, by its Bernstein sum.
fn height([p, q, r, s]: [f64; 4], t: f64) -> f64 {
    let u = 1.0 - t;
    u * u * u * p + 3.0 * u * u * t * q + 3.0 * u * t * t * r + t * t * t * s
}

/// Two cubic graphs that change sides between two samples have an item between them.
fn no_side_change_is_lost(case: &Case, found: &Intersection) -> Result<(), Finding> {
    let [first, second] = case.drawn;
    if first.kind % 4 != 3 || second.kind % 4 != 3 {
        return Ok(());
    }
    let (p, q) = (heights(first), heights(second));
    let at = |k: u32| f64::from(k) / f64::from(SAMPLES);
    let gap = |t: f64| height(p, t) - height(q, t);
    for k in 0..SAMPLES {
        let (t0, t1) = (at(k), at(k.saturating_add(1)));
        // Clear of rounding: the heights are at most 2^10, so a thousandth is far above it.
        if gap(t0) * gap(t1) >= 0.0 || gap(t0).abs() < 1e-3 || gap(t1).abs() < 1e-3 {
            continue;
        }
        let mut seen = false;
        for item in found.items() {
            let (s0, s1, ..) = spans(item)?;
            seen = seen || (s0 <= t1 && t0 <= s1);
        }
        if !seen {
            return Err(Finding::new("a change of sides has an item", || {
                format!("between {t0} and {t1}, {case:?}: {:?}", found.items())
            }));
        }
    }
    Ok(())
}

/// The number of items, proven points among them, and whether any is flagged.
fn tally(found: &Intersection) -> (usize, bool) {
    let points = found
        .items()
        .iter()
        .filter(|i| matches!(i, Item::Point { .. }))
        .count();
    let flagged = found
        .items()
        .iter()
        .any(|i| matches!(i, Item::Flagged { .. }));
    (points, flagged)
}

/// Whether two curves' domains are the same object, as an arc's end needs for the swap law.
fn on_a_seam(a: &CurveKind<2>, b: &CurveKind<2>) -> bool {
    let closed = |c: &CurveKind<2>| matches!(c, CurveKind::Circle(_) | CurveKind::Ellipse(_));
    closed(a) || closed(b)
}

/// The laws, on one case.
///
/// # Errors
///
/// A [`Finding`] naming the law broken and the case.
pub fn check(case: &Case) -> Result<(), Finding> {
    let [first, second] = case.drawn;
    let (a, b) = (curve(first)?, curve(second)?);
    let Some(found) = answer(curves(&a, &b, &mut meter()), case)? else {
        return Ok(());
    };
    let again = answer(curves(&a, &b, &mut meter()), case)?;
    if again.as_ref().is_none_or(|x| x.items() != found.items()) {
        return Err(Finding::new("an answer is the same twice", || {
            format!("{case:?}")
        }));
    }
    points_are_meetings(&a, &b, &found, case)?;
    no_side_change_is_lost(case, &found)?;

    if let Some(swapped) = answer(curves(&b, &a, &mut meter()), case)? {
        let (x, y) = (tally(&found), tally(&swapped));
        // A flagged region stands for what could not be told, and a point on a closed curve's join
        // may be found once from each side of it, so only clean answers compare.
        let clean = !x.1 && !y.1 && !on_a_seam(&a, &b);
        if clean && x.0 != y.0 {
            return Err(Finding::new(
                "a pair either way round finds the same points",
                || format!("{x:?} and {y:?}, {case:?}"),
            ));
        }
    }

    let budget = 1_u64 << (case.work % 24);
    let mut small = WorkMeter::new(Budget::new(budget, 0));
    match curves(&a, &b, &mut small) {
        Ok(rerun) if rerun.items() != found.items() => {
            return Err(Finding::new(
                "a budget stops an answer or leaves it",
                || format!("{budget}, {case:?}"),
            ));
        }
        Ok(_) | Err(IntersectError::BudgetExceeded) => {}
        Err(e) => {
            return Err(Finding::new(
                "a budget stops an answer or leaves it",
                || format!("{e:?}, {case:?}"),
            ));
        }
    }

    if a.domain().1.is_finite() {
        let once = answer(self_crossings(&a, &mut meter()), case)?;
        let twice = answer(self_crossings(&a, &mut meter()), case)?;
        let same = match (&once, &twice) {
            (Some(x), Some(y)) => x.items() == y.items(),
            _ => true,
        };
        if !same {
            return Err(Finding::new("an answer is the same twice", || {
                format!("self-crossings, {case:?}")
            }));
        }
    }
    Ok(())
}

/// [`decode`] then [`check`].
///
/// # Errors
///
/// As [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
