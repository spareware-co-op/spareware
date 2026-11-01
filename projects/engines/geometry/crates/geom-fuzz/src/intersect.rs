//! Fuzz target: `geom-intersect`'s result types keep their invariants and its operand order is one
//! order, on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_ITEMS`] items whose
//! enclosures sit on a coarse lattice, so ties, touches and overlaps are common, a coverage, and two
//! operands, each a line, a circle, a plane or a sphere. [`check`] builds the intersection and holds
//! it to the laws below, with an all-pairs oracle that shares nothing with the sweep it judges.
//! [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: a set of
//!   items accepted or refused against the oracle, an order that is not 12.15's, an answer that
//!   depends on the order of the items given, swaps that do not undo, a budget that changes an
//!   answer instead of stopping it, or two operand orders that make different calls.
//!
//! Method: chapter 12's result rules and canonical order
//! (`projects/engines/geometry/architecture/12-intersections.md`), with chapter 28.5's oracle stack
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: at most [`MAX_ITEMS`] items, compared pairwise by the oracle; at most [`MAX_INPUT_BYTES`]
//! bytes read.

use core::cell::RefCell;
use core::cmp::Ordering;

use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_intersect::{
    CanonicalKey, Coverage, CurveContact, CurveEnd, Geom, IntersectError, Intersection, Item,
    Operand, ParamBox, PointContact, Unresolved, pair,
};
use geom_math::Box3;
use geom_num::interval::Interval;
use geom_num::{Exactness, P3, Steps};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, Sphere, SurfaceKind};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most items a case holds. The oracle is quadratic and a fuzzer's input short, and a dozen
/// reach every order of the sweep: 12 is enough for three kinds of three items and a remainder.
pub const MAX_ITEMS: u8 = 12;

/// The most bytes [`decode`] reads from any input: 120.
///
/// Two operands of 4 bytes, one byte each for the unknowns, the coverage, the budget and the item
/// count, and 9 bytes for each of [`MAX_ITEMS`] items: 8 + 4 + 12 · 9 = 120. The unit test
/// `the_input_bound_is_reached_and_never_passed` reads exactly that.
pub const MAX_INPUT_BYTES: usize = 120;

/// One operand: which kind, and three numbers that place and size it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Line, circle, plane or sphere, by its remainder on division by four.
    pub kind: u8,
    /// Three small numbers.
    pub at: [u8; 3],
}

/// One item: its kind, and for each unknown where it starts on the lattice and how wide it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drawn {
    /// Point, curve, overlap or flagged, by its remainder on division by four.
    pub kind: u8,
    /// Each unknown's start and width.
    pub spans: [(u8, u8); 4],
}

/// One input of the target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The two operands.
    pub shapes: [Shape; 2],
    /// Unknowns: 2, 3 or 4.
    pub unknowns: u8,
    /// Whether the answer claims a certified topology.
    pub certified: bool,
    /// The work budget of the budgeted rerun.
    pub work: u8,
    /// The items.
    pub items: Vec<Drawn>,
}

/// The case in `data`, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let mut shape = || Shape {
        kind: src.byte(),
        at: [src.byte(), src.byte(), src.byte()],
    };
    let shapes = [shape(), shape()];
    let unknowns = (src.byte() % 3).saturating_add(2);
    let certified = src.byte() % 2 == 1;
    let work = src.byte();
    let count = src.byte() % (MAX_ITEMS + 1);
    let items = (0..count)
        .map(|_| {
            let kind = src.byte();
            let mut span = || (src.byte(), src.byte());
            Drawn {
                kind,
                spans: [span(), span(), span(), span()],
            }
        })
        .collect();
    let case = Case {
        shapes,
        unknowns,
        certified,
        work,
        items,
    };
    (case, data.len().saturating_sub(src.remaining()))
}

/// `made`, or the finding that a drawn value was not valid.
fn must<T, E: core::fmt::Debug>(made: Result<T, E>, law: &str) -> Result<T, Finding> {
    made.map_err(|e| Finding::new(law, || format!("{e:?}")))
}

/// A lattice position: small, so enclosures tie and touch often.
fn lattice(x: u8) -> f64 {
    f64::from(x % 8)
}

fn span((start, width): (u8, u8)) -> Result<Interval, Finding> {
    let lo = lattice(start);
    must(
        Interval::new(lo, lo + f64::from(width % 3)),
        "a lattice span is an interval",
    )
}

fn enclosure(drawn: &Drawn, unknowns: u8) -> Result<ParamBox, Finding> {
    let spans = drawn
        .spans
        .iter()
        .take(usize::from(unknowns))
        .map(|s| span(*s))
        .collect::<Result<Vec<_>, _>>()?;
    must(ParamBox::new(&spans), "a drawn box is a box")
}

fn space() -> Result<Box3, Finding> {
    let lo = must(P3::new(0, 0, 0), "the origin is on the grid")?;
    let hi = must(P3::new(1, 1, 1), "a unit corner is on the grid")?;
    must(Box3::new(lo, hi), "the unit box is a box")
}

fn item(drawn: &Drawn, unknowns: u8) -> Result<Item, Finding> {
    let at = enclosure(drawn, unknowns)?;
    let xyz = space()?;
    let class = Exactness::Exact;
    Ok(match drawn.kind % 4 {
        0 => Item::Point {
            at,
            xyz,
            contact: PointContact::Crossing,
            class,
        },
        1 => Item::Curve {
            at,
            curve: Box::new(curve(Shape {
                kind: 0,
                at: [0, 0, 0],
            })?),
            contact: CurveContact::Crossing,
            ends: [CurveEnd::Boundary; 2],
            class,
        },
        2 => Item::Overlap {
            at,
            region: geom_intersect::OverlapRegion {
                boundary: Vec::new(),
            },
            proof: geom_intersect::Coincidence::Within(Steps(1)),
            class,
        },
        _ => Item::Flagged {
            at,
            xyz,
            why: Unresolved::HigherOrderContact,
        },
    })
}

fn curve(shape: Shape) -> Result<CurveKind<3>, Finding> {
    let centre = shape.at.map(f64::from);
    if shape.kind % 2 == 0 {
        let line = Line::new(centre, [1.0, 0.0, 0.0], (0.0, 1.0));
        must(line, "a drawn line is valid").map(CurveKind::Line)
    } else {
        let frame = must(Frame::standard(), "the standard frame is valid")?;
        let circle = Circle::full(centre, 1.0, frame);
        must(circle, "a drawn circle is valid").map(CurveKind::Circle)
    }
}

fn surface(shape: Shape) -> Result<SurfaceKind, Finding> {
    let centre = shape.at.map(f64::from);
    if shape.kind % 2 == 0 {
        let domain = ((0.0, 1.0), (0.0, 1.0));
        let plane = Plane::new(centre, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain);
        must(plane, "a drawn plane is valid").map(SurfaceKind::Plane)
    } else {
        let frame = must(Frame::standard(), "the standard frame is valid")?;
        let sphere = Sphere::new(centre, 1.0, frame, ((0.0, 1.0), (-0.5, 0.5)));
        must(sphere, "a drawn sphere is valid").map(SurfaceKind::Sphere)
    }
}

/// A drawn operand's kinds: curve or surface.
enum Kind {
    Curve(Box<CurveKind<3>>),
    Surface(Box<SurfaceKind>),
}

fn kind(shape: Shape) -> Result<Kind, Finding> {
    if shape.kind % 4 < 2 {
        curve(shape).map(|c| Kind::Curve(Box::new(c)))
    } else {
        surface(shape).map(|s| Kind::Surface(Box::new(s)))
    }
}

fn operand(kind: &Kind) -> Result<Operand<'_>, Finding> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let law = "a valid operand has a key";
    let (geom, key) = match kind {
        Kind::Curve(c) => (
            Geom::Curve(c),
            must(CanonicalKey::of_curve(c, &mut meter), law)?,
        ),
        Kind::Surface(s) => (
            Geom::Surface(s),
            must(CanonicalKey::of_surface(s, &mut meter), law)?,
        ),
    };
    Ok(Operand {
        geom,
        domain: None,
        bound: Steps(0),
        key,
    })
}

/// The oracle: whether two items of one kind share a point, by comparing every unknown. Curves may:
/// a box holds all of a curve, and two curves of one pair may cross.
fn collide(a: &Item, b: &Item) -> bool {
    if matches!(a, Item::Curve { .. }) {
        return false;
    }
    core::mem::discriminant(a) == core::mem::discriminant(b)
        && a.at()
            .spans()
            .iter()
            .zip(b.at().spans())
            .all(|(x, y)| x.lo() <= y.hi() && y.lo() <= x.hi())
}

/// The oracle's verdict on a set of items: whether a valid intersection holds them.
fn valid(items: &[Item], coverage: Coverage) -> bool {
    let unproven = coverage == Coverage::TopologyCertified
        && items.iter().any(|i| matches!(i, Item::Flagged { .. }));
    let clash = items.iter().enumerate().any(|(i, a)| {
        items
            .iter()
            .skip(i.saturating_add(1))
            .any(|b| collide(a, b))
    });
    !(clash || unproven)
}

/// Where a kind of item goes in 12.15's order.
fn kind_place(item: &Item) -> u8 {
    match item {
        Item::Point { .. } => 0,
        Item::Curve { .. } => 1,
        Item::Overlap { .. } => 2,
        Item::Flagged { .. } => 3,
    }
}

/// Lower corner, then upper corner, coordinate by coordinate.
fn enclosure_place(a: &Item, b: &Item) -> Ordering {
    let corner = |pick: fn(&Interval) -> f64| {
        for (x, y) in a.at().spans().iter().zip(b.at().spans()) {
            let o = pick(x).total_cmp(&pick(y));
            if o != Ordering::Equal {
                return o;
            }
        }
        Ordering::Equal
    };
    corner(|i| i.lo()).then_with(|| corner(|i| i.hi()))
}

/// Whether `a` may stand before `b`.
fn in_order(a: &Item, b: &Item) -> bool {
    kind_place(a)
        .cmp(&kind_place(b))
        .then_with(|| enclosure_place(a, b))
        .is_le()
}

/// `case` held to every law.
///
/// # Errors
///
/// The first law it breaks.
pub fn check(case: &Case) -> Result<(), Finding> {
    let items = case
        .items
        .iter()
        .map(|d| item(d, case.unknowns))
        .collect::<Result<Vec<_>, _>>()?;
    let coverage = if case.certified {
        Coverage::TopologyCertified
    } else {
        Coverage::CompleteOutsideFlagged
    };
    let detail = || format!("{case:?}");
    let build = |list: Vec<Item>, meter: &mut WorkMeter<'_>| {
        Intersection::new(list, coverage, Vec::new(), meter)
    };
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let answer = build(items.clone(), &mut meter);
    if answer.is_ok() != valid(&items, coverage) {
        return Err(Finding::new(
            "a result is refused exactly when invalid",
            detail,
        ));
    }
    let reversed: Vec<Item> = items.iter().rev().cloned().collect();
    let again = build(reversed, &mut WorkMeter::new(Budget::UNLIMITED));
    if answer != again {
        return Err(Finding::new(
            "the items given in any order give one answer",
            detail,
        ));
    }
    if let Err(e) = &answer {
        if *e != IntersectError::Internal {
            return Err(Finding::new(
                "an unlimited build only fails as a bug",
                detail,
            ));
        }
    }
    if let Ok(done) = &answer {
        let sorted = done
            .items()
            .windows(2)
            .all(|w| matches!(w, [a, b] if in_order(a, b)));
        if !sorted {
            return Err(Finding::new("items run in 12.15's order", detail));
        }
        let round = done
            .clone()
            .swapped(1)
            .swapped(case.unknowns.saturating_sub(1));
        if round != *done {
            return Err(Finding::new("swapping twice is nothing", detail));
        }
    }
    let cap = Budget::new(u64::from(case.work), 0);
    let mut small = WorkMeter::new(cap);
    match build(items, &mut small) {
        Ok(done) if answer.as_ref() == Ok(&done) => {}
        Err(IntersectError::BudgetExceeded) => {}
        Err(IntersectError::Internal) if answer.is_err() => {}
        _ => {
            return Err(Finding::new(
                "a budget stops an answer or leaves it alone",
                detail,
            ));
        }
    }
    let [first, second] = case.shapes;
    let (a, b) = (kind(first)?, kind(second)?);
    let (a, b) = (operand(&a)?, operand(&b)?);
    orders(&a, &b, detail)
}

/// A stand-in for a method: one point, its first operand's unknowns at 1 and its second's at 2, so
/// which way round it ran shows in the answer.
fn stand_in(x: &Operand<'_>, y: &Operand<'_>, xyz: Box3) -> Result<Intersection, IntersectError> {
    let at = |value: f64, count: u8| {
        Interval::point(value)
            .map(|one| vec![one; usize::from(count)])
            .map_err(|_| IntersectError::InvalidInput)
    };
    let mut spans = at(1.0, x.geom.unknowns())?;
    spans.extend(at(2.0, y.geom.unknowns())?);
    let item = Item::Point {
        at: ParamBox::new(&spans)?,
        xyz,
        contact: PointContact::Crossing,
        class: Exactness::Exact,
    };
    Intersection::new(
        vec![item],
        Coverage::CompleteOutsideFlagged,
        Vec::new(),
        &mut WorkMeter::new(Budget::UNLIMITED),
    )
}

/// One pair asked both ways round makes one call, and the answers are the same with the roles
/// changed.
fn orders(
    a: &Operand<'_>,
    b: &Operand<'_>,
    detail: impl Fn() -> String + Copy,
) -> Result<(), Finding> {
    let xyz = space()?;
    let seen = RefCell::new(Vec::new());
    let solve = |x: &Operand<'_>, y: &Operand<'_>| {
        seen.borrow_mut().push((x.key.clone(), y.key.clone()));
        stand_in(x, y, xyz)
    };
    let one = must(pair(a, b, solve), "the stand-in answers")?;
    let two = must(pair(b, a, solve), "the stand-in answers")?;
    // Operands with one key are one shape: either order is the same call, and it shows nothing.
    if a.key != b.key && one.clone().swapped(a.geom.unknowns()) != two {
        return Err(Finding::new(
            "one pair gives one answer with the roles changed",
            detail,
        ));
    }
    let calls = seen.into_inner();
    let same = matches!(calls.as_slice(), [p, q] if p == q);
    let first = matches!(calls.as_slice(), [p, _] if p.0 <= p.1);
    if !same || !first {
        return Err(Finding::new(
            "a method only sees the canonical order",
            detail,
        ));
    }
    Ok(())
}

/// `decode` then `check`: the plain function every engine drives.
///
/// # Errors
///
/// The first law the case breaks.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
