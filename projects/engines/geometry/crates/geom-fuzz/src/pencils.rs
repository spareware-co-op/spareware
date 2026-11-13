//! Fuzz target: what the pencil of two curved quadrics says they share is so.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: two surfaces, each a sphere,
//! cylinder or cone, on a coarse lattice with axes from a short list, so that tangency, shared
//! rulings and special positions are common. [`check`] asks where they meet and holds the answer to
//! the laws below. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: an answer
//!   that changes between two runs, with the order of the pair, or when both surfaces move by a whole
//!   vector; nothing shared where a sphere has points on both sides of the other surface; a curve's
//!   point off either surface by the surface's own equation; or a smaller budget that changes an
//!   answer instead of stopping it.
//!
//! Method: chapter 12.7 (`projects/engines/geometry/architecture/12-intersections.md`), with chapter
//! 28.5's oracle stack (`projects/engines/geometry/architecture/28-testing.md`): each surface's
//! equation is written here from the drawn numbers, sharing nothing with the pencil it judges.
//!
//! Cost: a pencil's arithmetic, and a walk of up to [`WORK`] steps; at most [`MAX_INPUT_BYTES`] bytes
//! read.

#![allow(
    clippy::many_single_char_names,
    reason = "a case's drawn numbers are short names, as its fields are read off in order"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::quartic::{Arc, Meeting, Parameterised, meet};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Sphere, SurfaceKind};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The work a case may take, and a stop there is an answer the laws accept.
pub const WORK: u64 = 1 << 20;

/// The most bytes [`decode`] reads from any input: 16.
///
/// Two surfaces of 7 bytes and two for the budget and the rerun's kind: 7 + 7 + 2 = 16. The unit test
/// `the_input_bound_is_reached_and_never_passed` reads exactly that.
pub const MAX_INPUT_BYTES: usize = 16;

/// How far a point of a curve may stand from a surface by its equation, over the enclosure.
const OFF: f64 = 1e-5;

/// A whole turn, in half-turns.
const TURN: (f64, f64) = (0.0, 2.0);

/// The half-length of a cylinder or cone.
const SIDE: f64 = 4096.0;

/// The frames drawn from: a direction in the plane and the axis, all whole.
const FRAMES: [([f64; 3], [f64; 3]); 4] = [
    ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
    ([0.0, 0.0, 1.0], [3.0, 4.0, 0.0]),
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
    f64::from(i8::from_ne_bytes([x % 32])) - 16.0
}

/// The drawn point, moved by `by`.
fn centre(d: Drawn, by: f64) -> [f64; 3] {
    let [a, b, c, ..] = d.at;
    [lattice(a) + by, lattice(b) - by, lattice(c) + by]
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
    f64::from(e % 16) + 1.0
}

/// The drawn slope.
fn slope(d: Drawn) -> f64 {
    let [.., g] = d.at;
    SLOPES.get(usize::from(g % 4)).copied().unwrap_or(1.0)
}

/// The surface drawn, moved by `by`.
fn surface(d: Drawn, by: f64) -> Result<SurfaceKind, Finding> {
    let (x, n) = axes(d);
    let frame = must(Frame::new(x, n), "a drawn frame is valid")?;
    let side = (-SIDE, SIDE);
    Ok(match d.kind % 3 {
        0 => SurfaceKind::Sphere(must(
            Sphere::new(centre(d, by), radius(d), frame, (TURN, (-0.5, 0.5))),
            "a drawn sphere is valid",
        )?),
        1 => SurfaceKind::Cylinder(must(
            Cylinder::new(centre(d, by), radius(d), frame, (TURN, side)),
            "a drawn cylinder is valid",
        )?),
        _ => SurfaceKind::Cone(must(
            Cone::new(centre(d, by), 0.0, slope(d), frame, (TURN, side)),
            "a drawn cone is valid",
        )?),
    })
}

/// A vector in intervals.
fn enclosed(v: [f64; 3]) -> Result<[Interval; 3], Finding> {
    let [x, y, z] = v;
    Ok([
        must(Interval::point(x), "a number has an enclosure")?,
        must(Interval::point(y), "a number has an enclosure")?,
        must(Interval::point(z), "a number has an enclosure")?,
    ])
}

/// `a` · `b`, enclosed.
fn dot(a: [Interval; 3], b: [Interval; 3]) -> Option<Interval> {
    let [p, q, r] = a;
    let [x, y, z] = b;
    p.mul(x)
        .ok()?
        .add(q.mul(y).ok()?)
        .ok()?
        .add(r.mul(z).ok()?)
        .ok()
}

/// How the point `p` stands to the surface drawn, by the surface's own equation, moved by `by`:
/// positive outside, enclosed, and scaled to the size of a distance by the surface's radius.
fn off(d: Drawn, by: f64, p: [Interval; 3]) -> Option<Interval> {
    let [a, b, c] = centre(d, by);
    let [x, y, z] = p;
    let w = [
        x.sub(Interval::point(a).ok()?).ok()?,
        y.sub(Interval::point(b).ok()?).ok()?,
        z.sub(Interval::point(c).ok()?).ok()?,
    ];
    let (_, n) = axes(d);
    let [e, f, g] = n;
    // The unit axis is (e, f, g) over a whole length: 1 or 5.
    let length = if e == 3.0 && f == 4.0 && g == 0.0 {
        5.0
    } else {
        1.0
    };
    let along = dot(w, enclosed(n).ok()?)?
        .div(Interval::point(length).ok()?)
        .ok()?;
    let square = dot(w, w)?;
    let r = Interval::point(radius(d)).ok()?;
    let two = Interval::point(2.0).ok()?;
    Some(match d.kind % 3 {
        0 => square
            .sub(r.mul(r).ok()?)
            .ok()?
            .div(two.mul(r).ok()?)
            .ok()?,
        1 => square
            .sub(along.mul(along).ok()?)
            .ok()?
            .sub(r.mul(r).ok()?)
            .ok()?
            .div(two.mul(r).ok()?)
            .ok()?,
        _ => {
            let k = Interval::point(slope(d)).ok()?;
            let flat = square.sub(along.mul(along).ok()?).ok()?;
            flat.sub(k.mul(along).ok()?.mul(k.mul(along).ok()?).ok()?)
                .ok()?
        }
    })
}

/// A meter for a case.
fn meter<'a>() -> WorkMeter<'a> {
    WorkMeter::new(Budget::new(WORK, 0))
}

/// What `result` says; a stop on the budget is no answer.
fn answer(
    result: Result<Option<Meeting>, IntersectError>,
    case: &Case,
) -> Result<Option<Meeting>, Finding> {
    match result {
        Ok(a) => Ok(a),
        Err(IntersectError::BudgetExceeded) => Ok(None),
        Err(e) => Err(Finding::new("a valid pair is answered", || {
            format!("{e:?}, {case:?}")
        })),
    }
}

/// Every sample of a curve lies on both surfaces.
fn curve_is_on_both(curve: &Parameterised, found: &Meeting, case: &Case) -> Result<(), Finding> {
    let held = |theta: f64| -> Result<(), Finding> {
        let t = must(Interval::point(theta), "a turn has an enclosure")?;
        let delta = must(curve.discriminant(t), "a turn has a discriminant")?;
        if delta.hi() < 0.0 {
            return Ok(());
        }
        for upper in [true, false] {
            let Some(p) = must(curve.point(t, upper), "a sheet has a point")? else {
                continue;
            };
            for d in [case.first, case.second] {
                let side = off(d, 0.0, p);
                let near = side.is_none_or(|s| s.lo() <= OFF && s.hi() >= -OFF);
                if !near {
                    return Err(Finding::new("a curve lies on both", || {
                        format!("{p:?}, {side:?}, {d:?}, {case:?}")
                    }));
                }
            }
        }
        Ok(())
    };
    for arc in &found.arcs {
        match arc {
            Arc::Whole => {
                for i in 0_u32..16 {
                    held(f64::from(i) / 16.0 + 1e-3)?;
                }
            }
            Arc::Between { from, to } => {
                let (lo, hi) = (from.hi(), to.lo());
                for i in 1_u32..8 {
                    held(lo + (hi - lo) * f64::from(i) / 8.0)?;
                }
            }
        }
    }
    Ok(())
}

/// A surface with points inside and outside a sphere crosses it, so they share something.
fn no_crossing_is_lost(
    a: &SurfaceKind,
    b: &SurfaceKind,
    found: &Meeting,
    case: &Case,
) -> Result<(), Finding> {
    let (walked, ball) = if case.second.kind % 3 == 0 {
        (a, case.second)
    } else if case.first.kind % 3 == 0 {
        (b, case.first)
    } else {
        return Ok(());
    };
    if found.topology != geom_num::pencil::Topology::default() {
        return Ok(());
    }
    let ((u0, u1), (v0, v1)) = walked.domain();
    let share = |lo: f64, hi: f64, i: u32| lo + (hi - lo) * f64::from(i) / 16.0;
    let (mut inside, mut outside) = (false, false);
    let mut work = meter();
    for i in 0..=16 {
        for j in 0..=16 {
            let (u, v) = (share(u0, u1, i), share(v0, v1, j));
            let at = must(
                walked.bound(((u, u), (v, v)), &mut work),
                "a sample is in the domain",
            )?;
            let Some(side) = off(ball, 0.0, enclosed(at.lo)?) else {
                continue;
            };
            inside |= side.hi() < -1e-3;
            outside |= side.lo() > 1e-3;
        }
    }
    if inside && outside {
        return Err(Finding::new("a crossing of a sphere is shared", || {
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
    let (a, b) = (surface(case.first, 0.0)?, surface(case.second, 0.0)?);
    let Some(found) = answer(meet(&a, &b, &mut meter()), case)? else {
        return Ok(());
    };
    let again = answer(meet(&a, &b, &mut meter()), case)?;
    if again.as_ref() != Some(&found) {
        return Err(Finding::new("an answer is the same twice", || {
            format!("{case:?}")
        }));
    }
    if let Some(swapped) = answer(meet(&b, &a, &mut meter()), case)?
        && swapped.topology != found.topology
    {
        return Err(Finding::new("an answer does not hang on the order", || {
            format!("{found:?}, {swapped:?}, {case:?}")
        }));
    }
    let (p, q) = (surface(case.first, 37.0)?, surface(case.second, 37.0)?);
    if let Some(moved) = answer(meet(&p, &q, &mut meter()), case)?
        && moved.topology != found.topology
    {
        return Err(Finding::new("an answer moves with its surfaces", || {
            format!("{found:?}, {moved:?}, {case:?}")
        }));
    }
    if let Some(curve) = &found.curve {
        curve_is_on_both(curve, &found, case)?;
    }
    no_crossing_is_lost(&a, &b, &found, case)?;

    let budget = 1_u64 << (case.work % 20);
    let mut small = WorkMeter::new(Budget::new(budget, 0));
    match meet(&a, &b, &mut small) {
        Ok(Some(rerun)) if rerun == found => Ok(()),
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
