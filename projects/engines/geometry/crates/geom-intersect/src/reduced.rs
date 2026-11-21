//! Closed forms by reduction (chapter 12.7): tori in special positions, surfaces of revolution and
//! extrusions.
//!
//! What it does: [`crate::quadrics::natural`] calls these for the rows of 12.7 past the natural
//! quadrics. Two surfaces about one axis meet in circles, one at each crossing of their meridians in
//! the half-plane of the axis, so [`rings`] is curve with curve in two dimensions (12.5). A plane
//! square to the axis is a line of that half-plane. A plane through a torus's axis cuts two tube
//! circles, and a plane through its centre at the bitangent angle two Villarceau circles. A plane
//! through a revolution's axis cuts two copies of the profile, the second mirrored. An extrusion cut
//! by a plane along its direction is lines at the profile's crossings with the plane's trace; by any
//! other plane it is an affine copy of the profile, of the kind the map keeps; and two sweeps of one
//! direction meet in lines at the crossings of their cross-sections. What is left is `None`.
//!
//! Guarantees:
//!
//! - **Exact where the data are.** A plane's being square to an axis is a sign on the stored
//!   doubles; through an axis, and Villarceau's angle, signs from `geom_num::quadric` on whole-number
//!   data. Off the grid an enclosure decides, and a pair it cannot tell is `None`.
//! - **A meridian only where it is one.** A revolution's profile is read in a half-plane of its
//!   axis only where its own data prove, by exact signs of sums of products of the stored doubles,
//!   that it lies in that plane and on one side of the axis: a line by its ends, a circle,
//!   ellipse or parabola by its plane and a ball about its centre, a NURBS curve by its control
//!   points (issue 921). What they cannot prove is `None`.
//! - **Curve with curve is 12.5.** Each ring comes from a crossing or a tangency that
//!   [`crate::curve::curves`] proved, and is tangent where that was.
//! - **The kind is kept.** Lines stay lines, a circle maps to a circle where the map is a turn and to
//!   an ellipse otherwise, conics stay conics and NURBS stay NURBS.
//! - **Whole turns.** A torus or revolution of a part of a turn is `None`, for the general paths.
//! - **Where the surface stands.** An extrusion or revolution keeps its shape as built and a
//!   placement c + L·x that a transform moves (`geom-surface`'s swept kinds), so each case reads it
//!   rebuilt in place first, the same surface on the same parameters: an extrusion's profile and
//!   direction moved by any L, a revolution's profile and axis by an L that is exactly a
//!   similarity, each moved datum exact (a transform stating no rounding). Any other placement is
//!   `None`, never read as if unmoved (issue 917: a wall moved by x ↦ (3x, 3y, −3z) met a cylinder
//!   moved with it in no lines).
//!
//! Method: the rows of chapter 12.7
//! (`projects/engines/geometry/architecture/12-intersections.md`). Villarceau's circles: a plane
//! through the centre of a ring torus at the angle θ to its equator with sin θ = r/R holds two
//! circles of radius R, centred ±r from the centre along the line where the plane meets the equator
//! plane's square. Symmetry reduces the rest to chapter 12.5.
//!
//! Cost: a curve-curve pair for each pair of meridian curves or cross-sections, and a curve-surface
//! pair for each profile cut.
//!
//! Recursion:
//!
//! - Cycle: `Affine::image`.
//!   Bound: 2.
//!   Why: `Affine::image` calls itself only for a trimmed curve, on the curve the trim is of, and
//!   that is a basis curve, which is never trimmed, so the second frame takes none of the
//!   recursive arm.
//!   Test: `the_image_of_a_trimmed_curve_runs_at_its_bound`.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the chapter's: n a plane's normal, e an axis, d a direction, r a radius"
)]

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_curve::{Axes, Basis, Circle, CurveKind, Ellipse, Frame, Hyperbola, Line, Parabola};
use geom_curve::{CurveError, ParamCurve, Trimmed, tight};
use geom_math::{Transform, TypedTransform};
use geom_num::eft::two_prod;
use geom_num::exact_sum::ExactSum;
use geom_num::grid::Steps;
use geom_num::interval::Interval;
use geom_num::{Sign, math, quadric};
use geom_nurbs::Curve;
use geom_run::WorkMeter;
use geom_surface::{Extrusion, ParamSurface, Plane, Revolution, SurfaceKind, Torus};

use crate::curve::{bound_on, curves, finite, seam_of, sign_of, turn_of, whole};
use crate::curve_surface::{cross, curve_surface, dot, lift, minus};
use crate::error::IntersectError;
use crate::quadrics::{
    Axis, Piece, Vector, axis, beside, circle, decided, funnel, middle, plus, point, same_line,
    scale, sheet, stretch, unit, wide,
};
use crate::result::{Item, PointContact};

/// How far a plane's line runs in the half-plane, in steps: past any surface of the frame.
const OUT: f64 = 4_398_046_511_104.0;

/// A surface's meridian curves, in (ρ, z) about an axis, each with what it is of.
type Meridians = Vec<(CurveKind<2>, Source)>;

/// What a meridian curve is of: its surface's own data as stored. The curve's data are rounded
/// images of these, so a meeting is placed against an arc's end from these (issue 1142).
#[derive(Clone, Copy)]
enum Source {
    /// A plane square to the axis, through `at`.
    Level { at: [f64; 3] },
    /// A cylinder about the axis, through `at` along `along`.
    Wall {
        at: [f64; 3],
        along: [f64; 3],
        radius: f64,
    },
    /// A line of a cone about the axis, from its apex at the slope `slope` and the rise `rise`
    /// (±1). The apex is not stored, so no end stands on it exactly.
    Slant {
        apex: Vector,
        slope: Interval,
        rise: f64,
    },
    /// A revolution's straight profile `origin + t·direction`, off the axis at `t = middle`.
    Ruled {
        origin: [f64; 3],
        direction: [f64; 3],
        middle: f64,
    },
    /// A sphere about the axis.
    Ball { centre: [f64; 3], radius: f64 },
    /// A revolution's circular profile.
    Round {
        centre: [f64; 3],
        radius: f64,
        frame: Frame<3>,
    },
    /// A whole circle, which has no end, or a curve no closed form here reads.
    Other,
}

/// A place in the half-plane, and whether two surfaces touch there.
type Touch = (Interval, Interval, bool);

/// How far along `on` the point `at` stands, exactly where the data are: an exact height is what
/// lets curve with curve see two surfaces touch.
fn level(at: Vector, on: &Axis) -> Result<Interval, IntersectError> {
    let ([a, b, c], [p, q, r], [x, y, z]) = (at, on.at, on.along);
    let first = tight::mul(tight::sub(a, p)?, x)?;
    let second = tight::mul(tight::sub(b, q)?, y)?;
    let third = tight::mul(tight::sub(c, r)?, z)?;
    Ok(tight::add(tight::add(first, second)?, third)?)
}

/// The ring about `on` at height `z`, its centre exact where the data are.
fn ring(on: &Axis, z: Interval, r: Interval, tangent: bool) -> Result<Piece, IntersectError> {
    let ([a, b, c], [x, y, w]) = (on.at, on.along);
    let centre = [
        tight::add(a, tight::mul(x, z)?)?,
        tight::add(b, tight::mul(y, z)?)?,
        tight::add(c, tight::mul(w, z)?)?,
    ];
    circle(centre, r, on.along, tangent)
}

/// Where a line meets a circle in the half-plane, as (ρ, z) and whether it touches: the quadratic in
/// the line's parameter, exact where the data are, kept where it lies on the line's span and on the
/// circle's arc ([`on_arc`], from the curves' `sources`, a line's and a circle's). `None` for a
/// discriminant no enclosure tells from zero, or a meeting [`on_arc`] places neither on the arc nor
/// off it (issue 1142: the arc was solved as its whole circle, so a profile's half circle met rings
/// off it).
fn line_and_circle(
    line: &Line<2>,
    round: &Circle<2>,
    sources: [Source; 2],
    about: (&Axis, [f64; 3]),
) -> Result<Option<Vec<Touch>>, IntersectError> {
    let (o, d, m) = (line.origin(), line.direction(), round.center());
    let (span, r) = (ParamCurve::domain(line), tight::point(round.radius())?);
    let [ox, oy] = o;
    let [dx, dy] = d;
    let [mx, my] = m;
    let (px, py) = (tight::point(ox)?, tight::point(oy)?);
    let (vx, vy) = (tight::point(dx)?, tight::point(dy)?);
    let wx = tight::sub(px, tight::point(mx)?)?;
    let wy = tight::sub(py, tight::point(my)?)?;
    let a = tight::add(tight::square(vx)?, tight::square(vy)?)?;
    let b = tight::add(tight::mul(vx, wx)?, tight::mul(vy, wy)?)?;
    let e = tight::sub(
        tight::add(tight::square(wx)?, tight::square(wy)?)?,
        tight::square(r)?,
    )?;
    let disc = tight::sub(tight::square(b)?, tight::mul(a, e)?)?;
    if disc.hi() < 0.0 {
        return Ok(Some(Vec::new()));
    }
    let touching = disc.lo() == 0.0 && disc.hi() == 0.0;
    if disc.contains(0.0) && !touching {
        return Ok(None);
    }
    let root = tight::sqrt(disc)?;
    let mut out = Vec::new();
    let signs: &[f64] = if touching { &[1.0] } else { &[1.0, -1.0] };
    let whole = crate::seam::turn(ParamCurve::domain(round));
    for sign in signs {
        let t = tight::div(tight::sub(tight::mul(root, tight::point(*sign)?)?, b)?, a)?;
        if t.hi() < span.0 || t.lo() > span.1 {
            continue;
        }
        if !whole {
            match on_arc(round, sources, about, (*sign, touching))? {
                Some(true) => {}
                Some(false) => continue,
                None => return Ok(None),
            }
        }
        let rho = tight::add(px, tight::mul(vx, t)?)?;
        let z = tight::add(py, tight::mul(vy, t)?)?;
        out.push((rho, z, touching));
    }
    Ok(Some(out))
}

/// `a · b` in the half-plane.
fn dot2(a: [Interval; 2], b: [Interval; 2]) -> Result<Interval, IntersectError> {
    let ([p, q], [x, y]) = (a, b);
    Ok(tight::add(tight::mul(p, x)?, tight::mul(q, y)?)?)
}

/// `|v|²` in the half-plane.
fn square2([x, y]: [Interval; 2]) -> Result<Interval, IntersectError> {
    Ok(tight::add(tight::square(x)?, tight::square(y)?)?)
}

/// `a·u + b·v` in the half-plane.
fn blend(
    (a, u): (Interval, [Interval; 2]),
    (b, v): (Interval, [Interval; 2]),
) -> Result<[Interval; 2], IntersectError> {
    let ([p, q], [x, y]) = (u, v);
    Ok([
        tight::add(tight::mul(a, p)?, tight::mul(b, x)?)?,
        tight::add(tight::mul(a, q)?, tight::mul(b, y)?)?,
    ])
}

/// `a − b`, each entry [`tight`], so exact where the data are: the true meridians are read in
/// these, whose enclosures widen only where a bound rounds.
fn less_tight(a: Vector, b: Vector) -> Result<Vector, IntersectError> {
    let ([p, q, r], [x, y, z]) = (a, b);
    Ok([tight::sub(p, x)?, tight::sub(q, y)?, tight::sub(r, z)?])
}

/// `k · v`, each entry [`tight`].
fn scale_tight(v: Vector, k: Interval) -> Result<Vector, IntersectError> {
    let [x, y, z] = v;
    Ok([tight::mul(x, k)?, tight::mul(y, k)?, tight::mul(z, k)?])
}

/// `a · b`, each step [`tight`].
fn dot_tight(a: Vector, b: Vector) -> Result<Interval, IntersectError> {
    let ([p, q, r], [x, y, z]) = (a, b);
    let sum = tight::add(tight::mul(p, x)?, tight::mul(q, y)?)?;
    Ok(tight::add(sum, tight::mul(r, z)?)?)
}

/// `a × b`, each step [`tight`].
fn cross_tight(a: Vector, b: Vector) -> Result<Vector, IntersectError> {
    let ([p, q, r], [x, y, z]) = (a, b);
    let part =
        |a: Interval, b: Interval, c: Interval, d: Interval| -> Result<Interval, IntersectError> {
            Ok(tight::sub(tight::mul(a, b)?, tight::mul(c, d)?)?)
        };
    Ok([part(q, z, r, y)?, part(r, x, p, z)?, part(p, y, q, x)?])
}

/// The unit vector from the axis toward `p`, square to it, and `p`'s distance from the axis; `None`
/// where no enclosure tells `p` off the axis.
fn outward(on: &Axis, p: Vector) -> Result<Option<(Vector, Interval)>, IntersectError> {
    let w = less_tight(p, on.at)?;
    let lean = less_tight(w, scale_tight(on.along, dot_tight(w, on.along)?)?)?;
    if lean.iter().all(|c| c.contains(0.0)) {
        return Ok(None);
    }
    let size = tight::norm(&lean)?;
    let [x, y, z] = lean;
    let out = [
        tight::div(x, size)?,
        tight::div(y, size)?,
        tight::div(z, size)?,
    ];
    Ok(Some((out, size)))
}

/// The vector `v` in the half-plane of `on` toward `out`: its parts across the axis and along it.
fn flat(on: &Axis, out: Vector, v: Vector) -> Result<[Interval; 2], IntersectError> {
    Ok([dot_tight(v, out)?, dot_tight(v, on.along)?])
}

/// A straight meridian's true origin and direction in the half-plane, enclosed from its source, on
/// the curve's own parameter. `None` for no line's source, or a profile no enclosure tells off the
/// axis.
fn true_line(source: Source, on: &Axis) -> Result<Option<[[Interval; 2]; 2]>, IntersectError> {
    let (zero, one) = (tight::point(0.0)?, tight::point(1.0)?);
    Ok(Some(match source {
        Source::Level { at } => [[zero, level(lift(at)?, on)?], [one, zero]],
        Source::Wall { at, radius, .. } => {
            [[tight::point(radius)?, level(lift(at)?, on)?], [zero, one]]
        }
        Source::Slant { apex, slope, rise } => {
            [[zero, level(apex, on)?], [slope, tight::point(rise)?]]
        }
        Source::Ruled {
            origin,
            direction,
            middle,
        } => {
            let (o, d) = (lift(origin)?, lift(direction)?);
            let [p, q, r] = scale_tight(d, tight::point(middle)?)?;
            let [x, y, z] = o;
            let inner = [tight::add(x, p)?, tight::add(y, q)?, tight::add(z, r)?];
            let Some((out, _)) = outward(on, inner)? else {
                return Ok(None);
            };
            [flat(on, out, less_tight(o, on.at)?)?, flat(on, out, d)?]
        }
        _ => return Ok(None),
    }))
}

/// A circular meridian's true centre in the half-plane and its frame's x̂ and ŷ there, each times
/// one positive factor, enclosed, and its radius. A profile's are |p|·x̂ = q/|n| and |p|·ŷ = p, with
/// p = n × x and q = p × n on its frame's stored x and normal n, so that no length but |n|, which is
/// often one, rounds them: a turn or a sign read on them is the frame's.
type TrueRound = ([[Interval; 2]; 3], f64);

/// The [`TrueRound`] of a circular meridian, from its source. `None` for no arc's source, or a
/// centre no enclosure tells off the axis.
fn true_round(source: Source, on: &Axis) -> Result<Option<TrueRound>, IntersectError> {
    let (zero, one) = (tight::point(0.0)?, tight::point(1.0)?);
    Ok(Some(match source {
        Source::Ball { centre, radius } => {
            let middle = [zero, level(lift(centre)?, on)?];
            ([middle, [one, zero], [zero, one]], radius)
        }
        Source::Round {
            centre,
            radius,
            frame,
        } => {
            let Some((out, rho)) = outward(on, lift(centre)?)? else {
                return Ok(None);
            };
            let n = lift(frame.normal())?;
            let p = cross_tight(n, lift(frame.x())?)?;
            let size = tight::norm(&n)?;
            let [a, b, c] = cross_tight(p, n)?;
            let x = [
                tight::div(a, size)?,
                tight::div(b, size)?,
                tight::div(c, size)?,
            ];
            // The centre's ρ is its distance from the axis, exact where that is a double; its part
            // along `out` would round.
            let middle = [rho, dot_tight(less_tight(lift(centre)?, on.at)?, on.along)?];
            ([middle, flat(on, out, x)?, flat(on, out, p)?], radius)
        }
        _ => return Ok(None),
    }))
}

/// Whether the meeting `which` (its root's sign) of a line with `round` stands on the circle's arc,
/// decided on the true meridians, enclosed from the curves' `sources` (a line's and a circle's),
/// never on the curves, whose data are rounded: its turn on the true frame, moved by whole turns
/// ([`moved`]); where that reaches an end s, the sign of sin π(t − s), with cos πs and sin πs from
/// geom-num's proven `cospi` and `sinpi`, a proof wherever its enclosure is clear of zero
/// ([`from_end`]); and at an end of a whole number of half-turns, the end on the line exactly
/// ([`end_on_line`]). `None` where none of these tells, or where the true meridians' meetings are
/// not told apart as the curves' are (`touching`).
fn on_arc(
    round: &Circle<2>,
    sources: [Source; 2],
    (on, e): (&Axis, [f64; 3]),
    (which, touching): (f64, bool),
) -> Result<Option<bool>, IntersectError> {
    let [line, circle] = sources;
    let (Some([at, along]), Some(([m, x, y], r))) = (true_line(line, on)?, true_round(circle, on)?)
    else {
        return Ok(None);
    };
    // The foot of the centre on the line, from the centre, (k/|d|²)·d⊥ for d⊥ = (−d_z, d_ρ) and
    // k = d⊥·w, w the line's origin from the centre, and the half chord h² = r² − k²/|d|² there:
    // the meetings are foot ± (h/|d|)·d, read without the cancellation of the quadratic's terms,
    // and w is read once, so an exact touch stays exact.
    let ([ax, ay], [mx, my], [dx, dy]) = (at, m, along);
    let a = square2(along)?;
    let (wx, wy) = (tight::sub(ax, mx)?, tight::sub(ay, my)?);
    let k = tight::sub(tight::mul(dx, wy)?, tight::mul(dy, wx)?)?;
    let across = tight::div(k, a)?;
    let foot = [tight::mul(across, dy.neg())?, tight::mul(across, dx)?];
    let chord = tight::sub(
        tight::square(tight::point(r)?)?,
        tight::div(tight::square(k)?, a)?,
    )?;
    let told = if touching {
        chord.lo() == 0.0 && chord.hi() == 0.0
    } else {
        chord.lo() > 0.0
    };
    if !told {
        return Ok(None);
    }
    let half = tight::sqrt(tight::div(chord, a)?)?;
    // The meeting of sign `s`, from the circle's centre.
    let meeting = |s: f64| -> Result<[Interval; 2], IntersectError> {
        let t = tight::mul(half, tight::point(s)?)?;
        let [u, v] = foot;
        Ok([
            tight::add(u, tight::mul(dx, t)?)?,
            tight::add(v, tight::mul(dy, t)?)?,
        ])
    };
    let here = meeting(which)?;
    let (lo, hi) = ParamCurve::domain(round);
    let [start, end] = match moved(turn_of(dot2(here, x)?, dot2(here, y)?)?, (lo, hi))? {
        Arc::On => return Ok(Some(true)),
        Arc::Off => return Ok(Some(false)),
        Arc::Ends(ends) => ends,
    };
    let length = tight::sub(tight::point(hi)?, tight::point(lo)?)?;
    for (s, reached, last) in [(lo, start, false), (hi, end, true)] {
        if !reached {
            continue;
        }
        let turn = Interval::point(s)?;
        let (cos, sin) = (turn.cospi()?, turn.sinpi()?);
        // cos π(t − s) and sin π(t − s), each times r·|y|.
        let toward = dot2(here, blend((cos, x), (sin, y))?)?;
        let beside = dot2(here, blend((cos, y), (sin.neg(), x))?)?;
        if toward.lo() > 0.0
            && let Some(found) = from_end(turn_of(toward, beside)?, beside, last, length)?
        {
            return Ok(Some(found));
        }
    }
    // An end on the line is a meeting; it is this one where the other meeting's turn, read as this
    // one's, misses it: no move of it by whole turns meets the end, the arc (s, s).
    let clear = if touching {
        [true, true]
    } else {
        let other = meeting(-which)?;
        let turn = turn_of(dot2(other, x)?, dot2(other, y)?)?;
        [
            matches!(moved(turn, (lo, lo))?, Arc::Off),
            matches!(moved(turn, (hi, hi))?, Arc::Off),
        ]
    };
    for ((s, reached), apart) in [(lo, start), (hi, end)].into_iter().zip(clear) {
        let (Some(cos), Some(sin)) = (
            crate::seam::exact_cospi(s),
            crate::seam::exact_cospi(s - 0.5),
        ) else {
            continue;
        };
        if reached && end_on_line(sources, e, (cos, sin)) && apart {
            return Ok(Some(true));
        }
    }
    Ok(None)
}

/// Where a circle's point stands on the circle's arc, by the enclosure of its turn.
enum Arc {
    /// On the arc: the domain holds the enclosure moved by whole turns.
    On,
    /// Off it: no move of the enclosure by whole turns meets the domain.
    Off,
    /// The enclosure, moved by whole turns, reaches the domain's start, its end, or both.
    Ends([bool; 2]),
}

/// Where the point of turn `turn` stands on the arc `(lo, hi)`: each move of it by whole turns
/// that can meet the domain, as `onto` moves it there. The domain is closed, so an enclosure
/// inside it, an end included, is on the arc.
fn moved(turn: Interval, (lo, hi): (f64, f64)) -> Result<Arc, IntersectError> {
    let mut turns = math::floor((lo - turn.hi()) / 2.0)?.get();
    let (mut start, mut end) = (false, false);
    for _ in 0..3 {
        let moved = tight::add(turn, tight::point(2.0 * turns)?)?;
        turns += 1.0;
        if moved.lo() >= lo && moved.hi() <= hi {
            return Ok(Arc::On);
        }
        start = start || moved.contains(lo);
        end = end || moved.contains(hi);
    }
    Ok(if start || end {
        Arc::Ends([start, end])
    } else {
        Arc::Off
    })
}

/// Whether a meeting stands on the arc, from its offset δ = t − s from the arc's start s, or its
/// end where `last`, enclosed within a half-turn, and `beside`, sin πδ enclosed, whose sign is a
/// proof of its side. A meeting past the start by d = δ, or before the end by d = −δ, is on the
/// arc where d ≤ the arc's `length`; one on the far side of the end, d < 0, only where the arc runs
/// round to it, d + 2 ≤ `length`. `None` where the enclosures do not tell.
fn from_end(
    delta: Interval,
    beside: Interval,
    last: bool,
    length: Interval,
) -> Result<Option<bool>, IntersectError> {
    let inward = match beside.certain_sign() {
        Some(Sign::Positive) => !last,
        Some(Sign::Negative) => last,
        _ => return Ok(None),
    };
    let d = if last { delta.neg() } else { delta };
    let room = if inward {
        length
    } else {
        tight::sub(length, tight::point(2.0)?)?
    };
    Ok(if d.hi() <= room.lo() {
        Some(true)
    } else if d.lo() > room.hi() {
        Some(false)
    } else {
        None
    })
}

/// Whether the circle's end at a whole number of half-turns, of cosine and sine `(cos, sin)`, lies
/// on the line, exactly: on the `sources`' stored doubles (a line's and a circle's), so no turn
/// and no root is rounded. The end is E = c + r·g/|g| for an exact g: ±e at a sphere's pole, e the
/// axis as stored; on a profile ±q or ±p, its frame's x̂ = q/|q| and ŷ = p/|p| with p = n × x and
/// q = p × n. It is on a plane square to the axis through a where (E − a)·e = 0, on a cylinder
/// through a along f of radius R where |(E − a) × f|² = R²·|f|², and on a profile line through o
/// along d where (E − o) × d = 0: each α + β·|g| = 0 once multiplied out ([`vanishes`]). A cone's
/// apex is not stored, so no end is on its line here.
fn end_on_line([line, circle]: [Source; 2], e: [f64; 3], (cos, sin): (f64, f64)) -> bool {
    let (centre, radius, g) = match circle {
        Source::Ball { centre, radius } => (centre, radius, scale3(&exact3(e), &Exact::of(sin))),
        Source::Round {
            centre,
            radius,
            frame,
        } => {
            let n = exact3(frame.normal());
            let p = cross3(&n, &exact3(frame.x()));
            let q = cross3(&p, &n);
            let ([a, b, c], [x, y, z]) = (scale3(&q, &Exact::of(cos)), scale3(&p, &Exact::of(sin)));
            (centre, radius, [a.plus(&x), b.plus(&y), c.plus(&z)])
        }
        _ => return false,
    };
    let Some(g) = flat3(&g) else {
        return false;
    };
    let size = dot3(&g, &g);
    let r = Exact::of(radius);
    let from = |p: [f64; 3]| less3(&exact3(centre), &exact3(p));
    match line {
        Source::Level { at } => {
            let axis = exact3(e);
            vanishes(&r.times(&dot3(&g, &axis)), &dot3(&from(at), &axis), &size)
        }
        Source::Wall {
            at,
            along,
            radius: reach,
        } => {
            let f = exact3(along);
            let (Some(u), Some(v)) = (flat3(&cross3(&from(at), &f)), flat3(&cross3(&g, &f))) else {
                return false;
            };
            let far = Exact::of(reach)
                .times(&Exact::of(reach))
                .times(&dot3(&f, &f));
            let alpha = size
                .times(&dot3(&u, &u).less(&far))
                .plus(&r.times(&r).times(&dot3(&v, &v)));
            vanishes(
                &alpha,
                &Exact::of(2.0).times(&r).times(&dot3(&u, &v)),
                &size,
            )
        }
        Source::Ruled {
            origin, direction, ..
        } => {
            let d = exact3(direction);
            let [u0, u1, u2] = cross3(&from(origin), &d);
            let [v0, v1, v2] = cross3(&g, &d);
            [(u0, v0), (u1, v1), (u2, v2)]
                .iter()
                .all(|(u, v)| vanishes(&r.times(v), u, &size))
        }
        _ => false,
    }
}

/// Whether α + β·√g is zero, exactly, for g > 0: α and β zero or of opposite signs, and α² = β²·g.
/// Each is first made a few doubles ([`Exact::flat`]), so the squares stay small.
fn vanishes(alpha: &Exact, beta: &Exact, g: &Exact) -> bool {
    let (Some(a), Some(b), Some(g)) = (alpha.flat(), beta.flat(), g.flat()) else {
        return false;
    };
    let opposite = matches!(
        (a.sign(), b.sign()),
        (Some(Sign::Zero), Some(Sign::Zero))
            | (Some(Sign::Positive), Some(Sign::Negative))
            | (Some(Sign::Negative), Some(Sign::Positive))
    );
    opposite && a.times(&a).less(&b.times(&b).times(&g)).is(Sign::Zero)
}

/// A straight meridian curve.
fn straight(
    at: [f64; 2],
    along: [f64; 2],
    span: (f64, f64),
) -> Result<CurveKind<2>, IntersectError> {
    Ok(CurveKind::Line(Line::new(at, along, span)?))
}

/// The sign of a · b, exactly.
fn sign_along(a: [f64; 3], b: [f64; 3]) -> Sign {
    let [x, y, z] = a;
    let [p, q, r] = b;
    sign_of(&[(x, p), (y, q), (z, r)])
}

/// Whether the plane is square to `e`, exactly.
fn square_to(plane: &Plane, e: [f64; 3]) -> bool {
    let (_, u, v) = plane.vectors();
    sign_along(u, e) == Sign::Zero && sign_along(v, e) == Sign::Zero
}

/// Whether the surface turns all the way round its own axis: its u span is exactly two half-turns,
/// decided by an error-free sum, as the seam charts decide it (12.10). A span whose difference only
/// rounds to 2 is a part of a turn.
fn whole_turn(s: &SurfaceKind) -> bool {
    let ((u0, u1), _) = ParamSurface::domain(s);
    crate::seam::turn((u0, u1))
}

/// A torus's whole tube.
fn torus_meridian(t: &Torus, on: &Axis) -> Result<Meridians, IntersectError> {
    let (major, minor) = t.radii();
    let z = tight::middle(level(lift(t.center())?, on)?);
    let frame = Frame::<2>::standard()?;
    let tube = Circle::new([major, z], minor, frame, (0.0, 2.0))?;
    Ok(alloc::vec![(CurveKind::Circle(tube), Source::Other)])
}

/// `kind` as a curve on its own, cut to `span`.
fn on_span(kind: CurveKind<3>, span: (f64, f64)) -> Result<Option<CurveKind<3>>, IntersectError> {
    if finite(&kind)? == span {
        return Ok(Some(kind));
    }
    stretch(&kind, span.0, span.1)
}

/// The curve inside a trimmed one.
fn untrimmed(profile: &Trimmed<3>) -> Result<Option<CurveKind<3>>, IntersectError> {
    on_span(CurveKind::from(profile.basis().clone()), profile.interval())
}

/// The map from space to the half-plane of `on` that holds `at`, or `None` if `at` is on the axis.
fn to_half_plane(on: &Axis, at: Vector) -> Result<Option<Affine<2>>, IntersectError> {
    let w = minus(at, on.at)?;
    let lean = minus(w, scale(on.along, dot(w, on.along)?)?)?;
    if lean.iter().all(|c| c.contains(0.0)) {
        return Ok(None);
    }
    let out = unit(lean)?;
    Ok(Some(Affine::onto(on.at, [out, on.along])?))
}

/// The map to the half-plane of `on` that holds the profile `kind`, if the curve lies in that
/// half-plane: decided from its own data, exactly, by [`in_half_plane`]. The axis is the profile's
/// as stored, through `c` along `e`. The side and the map are read at one point, the anchor: a
/// circle's centre, which that proof needs off the axis and puts in the plane of the axis, and any
/// other curve's middle. (An ellipse's or a parabola's middle is nearer its centre than the radius
/// of the ball [`in_half_plane`] proves clear, strictly, as its axes are not parallel; so where that
/// proof holds the middle is off the axis, on the centre's side, and the middle serves.) A
/// circle's map was read at its middle as rounded, which is not data the proof holds: where the
/// circle touches the axis there, as a horn torus's does, the middle rounded across the axis
/// mirrored the meridian and lost its rings, and rounded off the plane of the axis turned it out
/// of that plane (issue 921).
fn half_plane_of(
    on: &Axis,
    (c, e): ([f64; 3], [f64; 3]),
    kind: &CurveKind<3>,
) -> Result<Option<Affine<2>>, IntersectError> {
    let (lo, hi) = finite(kind)?;
    let middle_of = kind.point(lo * 0.5 + hi * 0.5, geom_nurbs::Limit::FromRight)?;
    let anchor = match kind {
        CurveKind::Circle(x) => x.center(),
        _ => middle_of,
    };
    let Some(map) = to_half_plane(on, lift(anchor)?)? else {
        return Ok(None);
    };
    Ok(in_half_plane(&Turn::new(c, e, anchor), kind, (lo, hi)).then_some(map))
}

/// A sum of products of doubles, kept as its terms so that its sign is exact: each product is
/// expanded into doubles by error-free products, and the parts summed exactly. A polynomial in
/// the stored data, so no rounding of a difference or a product ever decides a side.
#[derive(Clone)]
struct Exact(Vec<Vec<f64>>);

impl Exact {
    /// The double `x`.
    fn of(x: f64) -> Exact {
        Exact(alloc::vec![alloc::vec![x]])
    }

    /// `self + other`.
    fn plus(&self, other: &Exact) -> Exact {
        let mut terms = self.0.clone();
        terms.extend(other.0.iter().cloned());
        Exact(terms)
    }

    /// `self − other`.
    fn less(&self, other: &Exact) -> Exact {
        let mut terms = self.0.clone();
        for term in &other.0 {
            let mut negated = term.clone();
            if let Some(first) = negated.first_mut() {
                *first = -*first;
            }
            terms.push(negated);
        }
        Exact(terms)
    }

    /// `self · other`.
    fn times(&self, other: &Exact) -> Exact {
        let mut terms = Vec::new();
        for a in &self.0 {
            for b in &other.0 {
                let mut term = a.clone();
                term.extend_from_slice(b);
                terms.push(term);
            }
        }
        Exact(terms)
    }

    /// The sign, exactly; `None` where [`Exact::total`] is.
    fn sign(&self) -> Option<Sign> {
        let sum = self.total()?;
        // The exact sum of finite doubles, rounded once to nearest: a nonzero one is a whole
        // multiple of 2^−1074, so it rounds to a nonzero double or an infinity of its own sign,
        // and zero is +0. Not a number only where the sum was lost, which has no order.
        sum.round_ieee().partial_cmp(&0.0).map(|order| match order {
            Ordering::Less => Sign::Negative,
            Ordering::Equal => Sign::Zero,
            Ordering::Greater => Sign::Positive,
        })
    }

    /// The same sum as a few doubles, so that its products stay few: each the rest rounded once
    /// to nearest, until the rest is zero. Each takes all of the rest but half an ulp of itself,
    /// and the rest is a whole multiple of 2^−1074, so a double's range of 2098 bits leaves at
    /// most 40 of them. `None` where [`Exact::total`] is, or the rest is lost.
    fn flat(&self) -> Option<Exact> {
        let mut rest = self.total()?;
        let mut parts = Vec::new();
        for _ in 0..48 {
            let part = rest.round_ieee();
            if part == 0.0 {
                return Some(Exact(parts));
            }
            if !part.is_finite() {
                return None;
            }
            parts.push(alloc::vec![part]);
            rest.add(-part);
        }
        None
    }

    /// The exact sum; `None` where a product of two parts is not exactly a pair of doubles: it
    /// overflows, or its error is finer than the smallest subnormal. [`two_prod`] says which by
    /// its error part, which is finite exactly when the pair is exact, so that one check is the
    /// whole test, at every magnitude.
    fn total(&self) -> Option<ExactSum> {
        let mut sum = ExactSum::new();
        for term in &self.0 {
            let Some((&first, rest)) = term.split_first() else {
                continue;
            };
            if term.contains(&0.0) {
                continue;
            }
            let mut parts = alloc::vec![first];
            for &factor in rest {
                let mut next = Vec::with_capacity(parts.len().saturating_mul(2));
                for part in parts {
                    let (p, e) = two_prod(part, factor);
                    if !e.is_finite() {
                        return None;
                    }
                    next.push(p);
                    if e != 0.0 {
                        next.push(e);
                    }
                }
                parts = next;
            }
            sum.add_slice(&parts);
        }
        Some(sum)
    }

    /// Whether the sign is exactly `s`.
    fn is(&self, s: Sign) -> bool {
        self.sign() == Some(s)
    }

    /// Whether the sum is proven at least zero.
    fn at_least_zero(&self) -> bool {
        matches!(self.sign(), Some(Sign::Positive | Sign::Zero))
    }
}

/// A vector of [`Exact`] entries.
type Exact3 = [Exact; 3];

/// `v` as exact entries.
fn exact3(v: [f64; 3]) -> Exact3 {
    v.map(Exact::of)
}

/// `v` with each entry [`Exact::flat`].
fn flat3(v: &Exact3) -> Option<Exact3> {
    let [x, y, z] = v;
    Some([x.flat()?, y.flat()?, z.flat()?])
}

/// `a − b`.
fn less3(a: &Exact3, b: &Exact3) -> Exact3 {
    let [p, q, r] = a;
    let [x, y, z] = b;
    [p.less(x), q.less(y), r.less(z)]
}

/// `k · v`.
fn scale3(v: &Exact3, k: &Exact) -> Exact3 {
    let [x, y, z] = v;
    [x.times(k), y.times(k), z.times(k)]
}

/// `a · b`.
fn dot3(a: &Exact3, b: &Exact3) -> Exact {
    let [p, q, r] = a;
    let [x, y, z] = b;
    p.times(x).plus(&q.times(y)).plus(&r.times(z))
}

/// `a × b`.
fn cross3(a: &Exact3, b: &Exact3) -> Exact3 {
    let [p, q, r] = a;
    let [x, y, z] = b;
    [
        q.times(z).less(&r.times(y)),
        r.times(x).less(&p.times(z)),
        p.times(y).less(&q.times(x)),
    ]
}

/// A profile's axis, through `c` along `e`, and the side of it the map reads: the side of the
/// profile's middle point `m`, as (m − c) × e.
struct Turn {
    c: Exact3,
    e: Exact3,
    toward: Exact3,
}

impl Turn {
    fn new(c: [f64; 3], e: [f64; 3], m: [f64; 3]) -> Turn {
        let (c, e) = (exact3(c), exact3(e));
        let toward = cross3(&less3(&exact3(m), &c), &e);
        Turn { c, e, toward }
    }

    /// `x` − c.
    fn offset(&self, x: [f64; 3]) -> Exact3 {
        less3(&exact3(x), &self.c)
    }

    /// Whether the direction `d` is square to the normal `n`, exactly.
    fn square(n: &Exact3, d: &Exact3) -> bool {
        dot3(n, d).is(Sign::Zero)
    }

    /// Whether the plane through c of normal `n` holds the axis, exactly.
    fn holds_axis(&self, n: &Exact3) -> bool {
        Turn::square(n, &self.e)
    }

    /// Where the point c + `w` stands about the axis: the sign of (w × e) · `toward`, positive on
    /// the side the map reads and zero on the axis, for a point of the plane of the axis and the
    /// middle point. Any positive multiple of w gives the same sign.
    fn side(&self, w: &Exact3) -> Option<Sign> {
        dot3(&cross3(w, &self.e), &self.toward).sign()
    }

    /// Whether every point c + wᵢ is on the side the map reads or on the axis, and one of them off
    /// it: a curve in the convex hull of such points lies in that half-plane.
    fn beside(&self, hull: &[Exact3]) -> bool {
        let mut off = false;
        for w in hull {
            match self.side(w) {
                Some(Sign::Positive) => off = true,
                Some(Sign::Zero) => {}
                _ => return false,
            }
        }
        off
    }

    /// Whether the ball about c + `w` of squared radius `reach` stays on the side the map reads:
    /// its centre is on that side and |w × e|² ≥ reach·|e|², so no point of it is across the
    /// axis.
    fn clear(&self, w: &Exact3, reach: &Exact) -> bool {
        let across = cross3(w, &self.e);
        self.side(w) == Some(Sign::Positive)
            && dot3(&across, &across)
                .less(&reach.times(&dot3(&self.e, &self.e)))
                .at_least_zero()
    }
}

/// Whether the conic of `axes` about `centre` lies in a plane of the axis: by its plane's normal,
/// the frame's or u × v, square to the axis and to the centre's offset from it.
fn conic_in_plane(turn: &Turn, centre: [f64; 3], axes: &Axes<3>) -> bool {
    let normal = match axes {
        Axes::Frame { frame, .. } => exact3(frame.normal()),
        Axes::Vectors { u, v } => {
            let n = cross3(&exact3(*u), &exact3(*v));
            if n.iter().all(|x| x.is(Sign::Zero)) {
                return false;
            }
            n
        }
    };
    turn.holds_axis(&normal) && Turn::square(&normal, &turn.offset(centre))
}

/// |u|² + |v|² for a conic's axes, a² + b² on a frame. A point α·u + β·v from the centre is
/// within (α² + β²)(|u|² + |v|²) of it, squared, by Cauchy and Schwarz.
fn axes_reach(axes: &Axes<3>) -> Exact {
    match axes {
        Axes::Frame { a, b, .. } => {
            let (a, b) = (Exact::of(*a), Exact::of(*b));
            a.times(&a).plus(&b.times(&b))
        }
        Axes::Vectors { u, v } => {
            let (u, v) = (exact3(*u), exact3(*v));
            dot3(&u, &u).plus(&dot3(&v, &v))
        }
    }
}

/// Whether the profile `kind` on `(lo, hi)` lies in the half-plane of the turn's axis that holds
/// its middle point, decided from its own data, exactly (issue 921; it read the ends and the
/// middle, which a circle or a NURBS curve can leave the plane between, and not which side of the
/// axis they were on, so a profile across the axis lost the rings of its far part). A line by its
/// ends; a circle by its plane, through the axis, and its centre at least its radius off the
/// axis; an ellipse or a parabola by its plane and a ball about its centre that holds it; a NURBS
/// curve by every control point lying in the plane and on the side (its weights are positive, so
/// it is in their convex hull); a curve on part of its span by the curve it is part of. What none
/// of these proves is `false`, for the general paths: a hyperbola, an arc whose whole circle
/// reaches across the axis, another kind.
fn in_half_plane(turn: &Turn, kind: &CurveKind<3>, (lo, hi): (f64, f64)) -> bool {
    // A curve on part of its span is read as the curve it is part of, on the same span, as a
    // profile on part of its span is read: where the whole curve is a meridian, so is any part of
    // it. What a trim holds is a leaf (`Basis`), never itself trimmed, so one unwrapping reaches
    // the leaf and this function never calls itself.
    let leaf;
    let kind = match kind {
        CurveKind::Trimmed(t) => {
            leaf = CurveKind::from(t.basis().clone());
            &leaf
        }
        other => other,
    };
    match kind {
        CurveKind::Line(l) => {
            let (o, d) = (turn.offset(l.origin()), exact3(l.direction()));
            let end = |t: f64| {
                let [p, q, r] = &o;
                let [x, y, z] = &d;
                let t = Exact::of(t);
                [
                    p.plus(&x.times(&t)),
                    q.plus(&y.times(&t)),
                    r.plus(&z.times(&t)),
                ]
            };
            Turn::square(&cross3(&turn.e, &d), &o) && turn.beside(&[end(lo), end(hi)])
        }
        CurveKind::Circle(x) => {
            let r = Exact::of(x.radius());
            let centre = turn.offset(x.center());
            turn.holds_axis(&exact3(x.frame().normal()))
                && Turn::square(&exact3(x.frame().normal()), &centre)
                && turn.clear(&centre, &r.times(&r))
        }
        CurveKind::Ellipse(x) => {
            // c + cos πt·u + sin πt·v: cos² + sin² = 1.
            let axes = x.axes();
            conic_in_plane(turn, x.center(), &axes)
                && turn.clear(&turn.offset(x.center()), &axes_reach(&axes))
        }
        CurveKind::Parabola(x) => {
            // c + t²·u + t·v with |t| ≤ T: t⁴ + t² ≤ T⁴ + T².
            let axes = x.axes();
            let far = Exact::of(tight::larger(lo.abs(), hi.abs()));
            let square = far.times(&far);
            let sum = square.times(&square).plus(&square);
            conic_in_plane(turn, x.vertex(), &axes)
                && turn.clear(&turn.offset(x.vertex()), &sum.times(&axes_reach(&axes)))
        }
        CurveKind::Nurbs(n) => {
            let weights = n.weights();
            let hull: Vec<Exact3> = n
                .homogeneous()
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let w = weights.and_then(|all| all.get(i)).copied().unwrap_or(1.0);
                    less3(&exact3(*h), &scale3(&turn.c, &Exact::of(w)))
                })
                .collect();
            // The plane of the axis and the first control point off it holds every other.
            let Some(normal) = hull
                .iter()
                .map(|w| cross3(&turn.e, w))
                .find(|n| n.iter().any(|x| !x.is(Sign::Zero)))
            else {
                return false;
            };
            hull.iter().all(|w| Turn::square(&normal, w)) && turn.beside(&hull)
        }
        _ => false,
    }
}

/// The linear part of a swept surface's placement as built.
const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// L·v + t, each entry exactly, or `None` where one is not a double.
fn exact_image(
    l: [[f64; 3]; 3],
    v: [f64; 3],
    t: [f64; 3],
) -> Result<Option<[f64; 3]>, IntersectError> {
    let [x, y, z] = v;
    let mut out = [0.0; 3];
    for ((slot, [a, b, c]), shift) in out.iter_mut().zip(l).zip(t) {
        let first = tight::mul(point(a)?, point(x)?)?;
        let second = tight::mul(point(b)?, point(y)?)?;
        let third = tight::mul(point(c)?, point(z)?)?;
        let sum = tight::add(
            tight::add(tight::add(first, second)?, third)?,
            point(shift)?,
        )?;
        if sum.lo() != sum.hi() {
            return Ok(None);
        }
        *slot = sum.lo();
    }
    Ok(Some(out))
}

/// `profile` under x ↦ L·x + t, exactly: `None` where its transform states a rounding, or cannot
/// take L.
fn profile_moved(
    profile: &Trimmed<3>,
    l: [[f64; 3]; 3],
    t: [f64; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Trimmed<3>>, IntersectError> {
    let [[a, b, c], [d, e, f], [g, h, i]] = l;
    let [x, y, z] = t;
    let rows = [[a, b, c, x], [d, e, f, y], [g, h, i, z]];
    let transform = TypedTransform::from(Transform::from_rows(rows)?);
    let moved = match ParamCurve::transformed(profile, &transform, meter) {
        Ok(moved) => moved,
        Err(CurveError::Degenerate | CurveError::Unbounded) => return Ok(None),
        Err(other) => return Err(other.into()),
    };
    if moved.deviation != 0.0 {
        return Ok(None);
    }
    // A trimmed curve's image is a trimmed curve, on the same interval.
    let CurveKind::Trimmed(image) = moved.value else {
        return Err(IntersectError::Internal);
    };
    Ok(Some(image))
}

/// An extrusion where it stands. Its placement c + L·x moves P(u) + v·d to (c + L·P(u)) + v·L·d,
/// the extrusion of the moved profile along L·d on the same parameters, for any L; `None` where
/// those are not doubles exactly. As built (c = 0, L the identity) it is itself.
fn extrusion_in_place(
    x: &Extrusion,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Extrusion>, IntersectError> {
    let (c, l) = x.placement();
    if c == [0.0; 3] && l == IDENTITY {
        return Ok(Some(x.clone()));
    }
    let (profile, d) = x.shape()?;
    let (Some(profile), Some(d)) = (
        profile_moved(profile, l, c, meter)?,
        exact_image(l, d, [0.0; 3])?,
    ) else {
        return Ok(None);
    };
    let (_, along) = ParamSurface::domain(x);
    Ok(Some(Extrusion::new(profile, d, along)?))
}

/// Whether L is exactly a similarity, λ times an orthogonal matrix: its rows square to one another
/// and of one length, not zero.
fn similarity(l: [[f64; 3]; 3]) -> bool {
    let [p, q, r] = l;
    let as_long = |a: [f64; 3], b: [f64; 3]| {
        let ([x, y, z], [e, f, g]) = (a, b);
        sign_of(&[(x, x), (y, y), (z, z), (-e, e), (-f, f), (-g, g)]) == Sign::Zero
    };
    sign_along(p, p) == Sign::Positive
        && sign_along(p, q) == Sign::Zero
        && sign_along(p, r) == Sign::Zero
        && sign_along(q, r) == Sign::Zero
        && as_long(p, q)
        && as_long(p, r)
}

/// Whether the similarity L mirrors. Its rows ρ₀, ρ₁, ρ₂ are λ times those of an orthogonal Q, so
/// ρ₁ × ρ₂ = λ·det Q·ρ₀: det Q has the sign of an entry of ρ₁ × ρ₂ over that of ρ₀ where that is
/// not zero, each entry of the cross product the exact sign of a difference of two products.
fn mirrors(l: [[f64; 3]; 3]) -> bool {
    let [p, [a, b, c], [e, f, g]] = l;
    let cross = [
        sign_of(&[(b, g), (-c, f)]),
        sign_of(&[(c, e), (-a, g)]),
        sign_of(&[(a, f), (-b, e)]),
    ];
    let Some((sign, x)) = cross.into_iter().zip(p).find(|(_, x)| *x != 0.0) else {
        return false;
    };
    // Neither sign is zero here: x is not, and that entry of ρ₁ × ρ₂ is λ·det Q·x.
    sign != sign_of(&[(x, 1.0)])
}

/// A surface of revolution where it stands. Its placement c + L·x is a surface of revolution only
/// for L a similarity, about the axis through c along L·n̂: with w = P − a turned about n̂, L
/// turns L·w about m̂ = ±L·n̂/|L·n̂|, + where L keeps the sense and − where it mirrors
/// (L·(n̂ × w) = m̂ × L·w), so it is the revolution of the profile c + L·(P − a) about (c, m̂) on
/// the same parameters. `None` for any other L, or where those data are not doubles exactly. As
/// built (c = a, L the identity) it is itself.
fn revolution_in_place(
    r: &Revolution,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Revolution>, IntersectError> {
    let (c, l) = r.placement();
    let (profile, a, frame) = r.shape()?;
    if c == a && l == IDENTITY {
        return Ok(Some(r.clone()));
    }
    if !similarity(l) {
        return Ok(None);
    }
    let (Some(shift), Some(normal), Some(x)) = (
        exact_image(l, a.map(|v| -v), c)?,
        exact_image(l, frame.normal(), [0.0; 3])?,
        exact_image(l, frame.x(), [0.0; 3])?,
    ) else {
        return Ok(None);
    };
    let Some(profile) = profile_moved(profile, l, shift, meter)? else {
        return Ok(None);
    };
    let normal = if mirrors(l) {
        normal.map(|v| -v)
    } else {
        normal
    };
    let (angles, _) = ParamSurface::domain(r);
    let placed = Revolution::new(profile, c, Frame::new(x, normal)?, angles)?;
    Ok(Some(placed))
}

/// `s` where it stands: an extrusion or surface of revolution with its placement carried into its
/// shape, any other kind as it is; `None` where that cannot be done exactly.
fn in_place(
    s: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<SurfaceKind>, IntersectError> {
    Ok(Some(match s {
        SurfaceKind::Extrusion(x) => {
            let Some(placed) = extrusion_in_place(x, meter)? else {
                return Ok(None);
            };
            SurfaceKind::Extrusion(placed)
        }
        SurfaceKind::Revolution(x) => {
            let Some(placed) = revolution_in_place(x, meter)? else {
                return Ok(None);
            };
            SurfaceKind::Revolution(placed)
        }
        _ => s.clone(),
    }))
}

/// A surface of revolution's axis, of one [`revolution_in_place`] gave: the shape is as built.
fn revolution_axis(r: &Revolution) -> Result<Axis, IntersectError> {
    let (_, a, frame) = r.shape()?;
    axis(a, &frame, 1.0)
}

/// The profile of `r` in the half-plane of its axis, if it lies in one: of a revolution
/// [`revolution_in_place`] gave.
fn profile_meridian(r: &Revolution) -> Result<Option<Meridians>, IntersectError> {
    let (profile, at, frame) = r.shape()?;
    let on = revolution_axis(r)?;
    let Some(kind) = untrimmed(profile)? else {
        return Ok(None);
    };
    let Some(map) = half_plane_of(&on, (at, frame.normal()), &kind)? else {
        return Ok(None);
    };
    let source = match &kind {
        CurveKind::Line(l) => {
            let (lo, hi) = finite(&kind)?;
            Source::Ruled {
                origin: l.origin(),
                direction: l.direction(),
                middle: lo * 0.5 + hi * 0.5,
            }
        }
        CurveKind::Circle(c) => Source::Round {
            centre: c.center(),
            radius: c.radius(),
            frame: c.frame(),
        },
        _ => Source::Other,
    };
    Ok(map
        .image(&kind)?
        .map(|(curve, _)| alloc::vec![(curve, source)]))
}

/// `s`'s meridian curves about `on`, if it stands square to the axis or about it: of a surface
/// [`in_place`] gave.
fn meridians(
    s: &SurfaceKind,
    on: &Axis,
    e: [f64; 3],
    c: [f64; 3],
) -> Result<Option<Meridians>, IntersectError> {
    let about = |center: [f64; 3], along: [f64; 3]| same_line(center, along, c, e);
    Ok(Some(match s {
        SurfaceKind::Plane(p) if square_to(p, e) => {
            let (at, ..) = p.vectors();
            let z = tight::middle(level(lift(at)?, on)?);
            let line = straight([0.0, z], [1.0, 0.0], (0.0, OUT))?;
            alloc::vec![(line, Source::Level { at })]
        }
        SurfaceKind::Sphere(x) if about(x.center(), e) => {
            let z = tight::middle(level(lift(x.center())?, on)?);
            let frame = Frame::<2>::standard()?;
            let half = Circle::new([0.0, z], x.radius(), frame, (-0.5, 0.5))?;
            let ball = Source::Ball {
                centre: x.center(),
                radius: x.radius(),
            };
            alloc::vec![(CurveKind::Circle(half), ball)]
        }
        SurfaceKind::Cylinder(x) if about(x.center(), x.frame().normal()) && whole_turn(s) => {
            let z = tight::middle(level(lift(x.center())?, on)?);
            let (_, (v0, v1)) = ParamSurface::domain(x);
            // The point at v stands s·v along n̂, so s·v times ±1 (n̂ along `on` or against it)
            // past z: the axial factor s is ±1 as built, but a similarity scales it.
            let sense = tight::middle(dot(lift(x.frame().normal())?, on.along)?);
            let s = x.axial_sign();
            let rate = if sense > 0.0 { s } else { -s };
            let (a, b) = (v0 * rate, v1 * rate);
            let (lo, hi) = (tight::smaller(a, b), tight::larger(a, b));
            let wall = Source::Wall {
                at: x.center(),
                along: x.frame().normal(),
                radius: x.radius(),
            };
            alloc::vec![(straight([x.radius(), z], [0.0, 1.0], (lo, hi))?, wall)]
        }
        SurfaceKind::Cone(x) if about(x.center(), x.frame().normal()) && whole_turn(s) => {
            let f = funnel(x)?;
            let z = tight::middle(level(f.apex, on)?);
            let k = tight::middle(f.slope);
            let slant = |rise: f64| Source::Slant {
                apex: f.apex,
                slope: f.slope,
                rise,
            };
            alloc::vec![
                (straight([0.0, z], [k, 1.0], (0.0, OUT))?, slant(1.0)),
                (straight([0.0, z], [k, -1.0], (0.0, OUT))?, slant(-1.0)),
            ]
        }
        SurfaceKind::Torus(x) if about(x.center(), x.frame().normal()) && whole_turn(s) => {
            torus_meridian(x, on)?
        }
        SurfaceKind::Revolution(x) if whole_turn(s) => {
            let (_, a, frame) = x.shape()?;
            if !about(a, frame.normal()) {
                return Ok(None);
            }
            let Some(curves) = profile_meridian(x)? else {
                return Ok(None);
            };
            curves
        }
        _ => return Ok(None),
    }))
}

/// Whether two rings or points stand at one place of the axis.
fn same_place(a: (Interval, Interval), b: (Interval, Interval)) -> bool {
    let meet = |x: Interval, y: Interval| x.lo() <= y.hi() && y.lo() <= x.hi();
    meet(a.0, b.0) && meet(a.1, b.1)
}

/// The rings at the crossings of two sets of meridian curves, about the axis `on`, stored along `e`.
/// `None` where curve with curve could not decide.
fn rings(
    (on, e): (&Axis, [f64; 3]),
    first: &[(CurveKind<2>, Source)],
    second: &[(CurveKind<2>, Source)],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let mut out = Vec::new();
    let mut seen: Vec<(Interval, Interval)> = Vec::new();
    let mut place = |r: Interval, z: Interval, tangent: bool| -> Result<(), IntersectError> {
        if r.hi() < 0.0 || seen.iter().any(|at| same_place(*at, (r, z))) {
            return Ok(());
        }
        seen.push((r, z));
        out.push(if r.contains(0.0) {
            Piece::Point {
                at: plus(on.at, scale(on.along, z)?)?,
                tangent: true,
            }
        } else {
            ring(on, z, r, tangent)?
        });
        Ok(())
    };
    for (x, from) in first {
        for (y, to) in second {
            let closed = match (x, y) {
                (CurveKind::Line(l), CurveKind::Circle(c)) => {
                    Some(line_and_circle(l, c, [*from, *to], (on, e))?)
                }
                (CurveKind::Circle(c), CurveKind::Line(l)) => {
                    Some(line_and_circle(l, c, [*to, *from], (on, e))?)
                }
                _ => None,
            };
            if let Some(found) = closed {
                let Some(found) = found else {
                    return Ok(None);
                };
                for (r, z, tangent) in found {
                    place(r, z, tangent)?;
                }
                continue;
            }
            let found = curves(x, y, meter)?;
            for item in found.items() {
                match item {
                    Item::Point { at, contact, .. } => {
                        let [t, ..] = at.spans() else {
                            return Err(IntersectError::Internal);
                        };
                        // On a whole meridian circle a span across its join is read modulo the
                        // turn (decision 32.104).
                        let b = bound_on(x, *t, meter)?;
                        let ([r0, z0], [r1, z1]) = (b.lo, b.hi);
                        let (r, z) = (Interval::new(r0, r1)?, Interval::new(z0, z1)?);
                        place(r, z, !matches!(contact, PointContact::Crossing))?;
                    }
                    Item::Overlap { .. } => return Ok(Some(alloc::vec![Piece::Same])),
                    Item::Flagged { .. } => return Ok(None),
                    Item::Curve { .. } => {}
                }
            }
        }
    }
    Ok(Some(out))
}

/// A surface with a surface about one axis, or a plane square to it, the second giving the axis.
fn about_one_axis(
    a: &SurfaceKind,
    b: &SurfaceKind,
    on: &Axis,
    e: [f64; 3],
    c: [f64; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (Some(first), Some(second)) = (meridians(a, on, e, c)?, meridians(b, on, e, c)?) else {
        return Ok(None);
    };
    rings((on, e), &first, &second, meter)
}

/// Whether the torus's angle to the plane is Villarceau's, and its circles if it is.
fn villarceau(
    p: &crate::quadrics::Sheet,
    on: &Axis,
    t: &Torus,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (major, minor) = t.radii();
    let exact = (|| {
        let plane = p.exact?;
        let (g, h) = on.exact?;
        Some(quadric::villarceau(
            &plane,
            g,
            h,
            Steps(u64::try_from(whole(major)?).ok()?),
            Steps(u64::try_from(whole(minor)?).ok()?),
        ))
    })();
    let Some(Ok(Sign::Zero)) = exact else {
        return Ok(None);
    };
    if major <= minor {
        return Ok(None);
    }
    let along = unit(cross(on.along, p.normal)?)?;
    let shift = scale(along, point(minor)?)?;
    let radius = point(major)?;
    Ok(Some(alloc::vec![
        circle(plus(on.at, shift)?, radius, p.normal, false)?,
        circle(minus(on.at, shift)?, radius, p.normal, false)?,
    ]))
}

/// A plane with a torus: square to its axis, through it, or at Villarceau's angle through its centre.
pub(crate) fn plane_torus(
    a: &Plane,
    b: &Torus,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let surface = SurfaceKind::Torus(b.clone());
    if !whole_turn(&surface) {
        return Ok(None);
    }
    let on = axis(b.center(), &b.frame(), 1.0)?;
    let e = b.frame().normal();
    if square_to(a, e) {
        return about_one_axis(
            &SurfaceKind::Plane(a.clone()),
            &surface,
            &on,
            e,
            b.center(),
            meter,
        );
    }
    let p = sheet(a)?;
    let tilt = dot(p.normal, on.along)?;
    let side = dot(p.normal, minus(on.at, p.at)?)?;
    let (exact_tilt, exact_side) = match (p.exact, on.exact) {
        (Some(s), Some((g, h))) => (
            Some(quadric::plane_along(&s, g, h)),
            Some(quadric::plane_along(&s, s.a, g)),
        ),
        _ => (None, None),
    };
    let (Some(lean), Some(through)) = (decided(exact_tilt, tilt), decided(exact_side, side)) else {
        return Ok(None);
    };
    if through != Sign::Zero {
        return Ok(None);
    }
    if lean == Sign::Zero {
        let out = unit(cross(p.normal, on.along)?)?;
        let (major, minor) = t_radii(b)?;
        let shift = scale(out, major)?;
        return Ok(Some(alloc::vec![
            circle(plus(on.at, shift)?, minor, p.normal, false)?,
            circle(minus(on.at, shift)?, minor, p.normal, false)?,
        ]));
    }
    villarceau(&p, &on, b)
}

/// A torus's radii, enclosed.
fn t_radii(t: &Torus) -> Result<(Interval, Interval), IntersectError> {
    let (major, minor) = t.radii();
    Ok((point(major)?, point(minor)?))
}

/// A sphere, cylinder, cone or torus with a torus about one axis.
pub(crate) fn about_torus(
    a: &SurfaceKind,
    b: &Torus,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let surface = SurfaceKind::Torus(b.clone());
    if !whole_turn(&surface) {
        return Ok(None);
    }
    let on = axis(b.center(), &b.frame(), 1.0)?;
    about_one_axis(a, &surface, &on, b.frame().normal(), b.center(), meter)
}

/// A plane with a surface of revolution: square to its axis, or through it.
pub(crate) fn plane_revolution(
    a: &Plane,
    b: &Revolution,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let Some(b) = revolution_in_place(b, meter)? else {
        return Ok(None);
    };
    let surface = SurfaceKind::Revolution(b.clone());
    if !whole_turn(&surface) {
        return Ok(None);
    }
    let on = revolution_axis(&b)?;
    let (profile, at, frame) = b.shape()?;
    let e = frame.normal();
    if square_to(a, e) {
        return about_one_axis(&SurfaceKind::Plane(a.clone()), &surface, &on, e, at, meter);
    }
    let p = sheet(a)?;
    let tilt = dot(p.normal, on.along)?;
    let side = dot(p.normal, minus(on.at, p.at)?)?;
    let (exact_tilt, exact_side) = match (p.exact, on.exact) {
        (Some(s), Some((g, h))) => (
            Some(quadric::plane_along(&s, g, h)),
            Some(quadric::plane_along(&s, s.a, g)),
        ),
        _ => (None, None),
    };
    let (Some(Sign::Zero), Some(Sign::Zero)) =
        (decided(exact_tilt, tilt), decided(exact_side, side))
    else {
        return Ok(None);
    };
    let Some(kind) = untrimmed(profile)? else {
        return Ok(None);
    };
    // A profile off a half-plane of the axis sweeps no copies of itself.
    let Some(flat) = half_plane_of(&on, (at, e), &kind)? else {
        return Ok(None);
    };
    // Turn the half-plane of the profile onto each half of the plane.
    let out = unit(cross(p.normal, on.along)?)?;
    let mut pieces = Vec::new();
    for sense in [1.0, -1.0] {
        let toward = scale(out, point(sense)?)?;
        let map = flat.then_into(on.at, toward, on.along)?;
        let Some((curve, reach)) = map.image(&kind)? else {
            return Ok(None);
        };
        pieces.push(Piece::Curve {
            curve: Box::new(curve),
            tangent: false,
            reach,
        });
    }
    Ok(Some(pieces))
}

/// A quadric or a torus with a surface of revolution about one axis.
pub(crate) fn about_revolution(
    a: &SurfaceKind,
    b: &Revolution,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (Some(a), Some(b)) = (in_place(a, meter)?, revolution_in_place(b, meter)?) else {
        return Ok(None);
    };
    let surface = SurfaceKind::Revolution(b.clone());
    if !whole_turn(&surface) {
        return Ok(None);
    }
    let on = revolution_axis(&b)?;
    let (_, at, frame) = b.shape()?;
    about_one_axis(&a, &surface, &on, frame.normal(), at, meter)
}

/// The plane through `at` of `p`'s directions, far past any surface.
fn endless(p: &Plane, at: [f64; 3]) -> Result<SurfaceKind, IntersectError> {
    let (_, u, v) = p.vectors();
    let span = (-OUT, OUT);
    Ok(SurfaceKind::Plane(Plane::new(at, u, v, (span, span))?))
}

/// `from + d·s` for s over `span`, as a line, its data reaching as far as `from`'s enclosure or
/// `spread`, the width of what set its span.
fn along(
    from: Vector,
    d: [f64; 3],
    span: (f64, f64),
    tangent: bool,
    spread: f64,
) -> Result<Piece, IntersectError> {
    Ok(Piece::Curve {
        curve: Box::new(CurveKind::Line(Line::new(middle(from), d, span)?)),
        tangent,
        reach: tight::larger(wide(from), spread),
    })
}

/// A plane along an extrusion's direction: a line at each crossing of the profile.
fn plane_along(
    a: &Plane,
    kind: &CurveKind<3>,
    d: [f64; 3],
    span: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (at, ..) = a.vectors();
    let found = curve_surface(kind, &endless(a, at)?, meter)?;
    let mut out = Vec::new();
    for item in found.items() {
        match item {
            Item::Point {
                at: place, contact, ..
            } => {
                let [t, ..] = place.spans() else {
                    return Err(IntersectError::Internal);
                };
                // On a whole profile circle a span across its join is read modulo the turn
                // (decision 32.104).
                let b = bound_on(kind, *t, meter)?;
                let [x0, y0, z0] = b.lo;
                let [x1, y1, z1] = b.hi;
                let from = [
                    Interval::new(x0, x1)?,
                    Interval::new(y0, y1)?,
                    Interval::new(z0, z1)?,
                ];
                let tangent = !matches!(contact, PointContact::Crossing);
                out.push(along(from, d, span, tangent, 0.0)?);
            }
            Item::Overlap { .. } => return Ok(Some(alloc::vec![Piece::Same])),
            Item::Flagged { .. } => return Ok(None),
            Item::Curve { .. } => {}
        }
    }
    Ok(Some(out))
}

/// How far along `d` from the plane through `c` of normal `n` the point `x` stands.
fn height_on(x: Vector, c: Vector, n: Vector, d: Vector) -> Result<Interval, IntersectError> {
    Ok(dot(minus(c, x)?, n)?.div(dot(d, n)?)?)
}

/// A plane across an extrusion: the profile moved along `d` into the plane, on the stretches of it
/// the extrusion reaches.
fn plane_across(
    a: &Plane,
    kind: &CurveKind<3>,
    d: [f64; 3],
    span: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let p = sheet(a)?;
    let direction = lift(d)?;
    let Some((image, reach)) = Affine::along(p.at, p.normal, direction)?.image(kind)? else {
        return Ok(None);
    };
    let (at, ..) = a.vectors();
    let [x, y, z] = at;
    let [e, f, g] = d;
    // Where the profile passes the planes the extrusion's ends reach, whose images are its ends.
    let (lo, hi) = finite(kind)?;
    let seam = seam_of(kind, (lo, hi));
    let mut cuts = Vec::new();
    for v in [span.0, span.1] {
        let moved = endless(a, [x - v * e, y - v * f, z - v * g])?;
        for item in curve_surface(kind, &moved, meter)?.items() {
            let [t, ..] = item.at().spans() else {
                return Err(IntersectError::Internal);
            };
            // On a whole profile circle a span across its join is read modulo the turn
            // (decision 32.104): its middle, on the window.
            let t = seam.wrapped(tight::middle(*t));
            if t > lo && t < hi {
                cuts.push(t);
            }
        }
    }
    cuts.sort_unstable_by(f64::total_cmp);
    let mut out = Vec::new();
    let mut start = lo;
    cuts.push(hi);
    for end in cuts {
        let middle_of = start * 0.5 + end * 0.5;
        let there = lift(kind.point(middle_of, geom_nurbs::Limit::FromRight)?)?;
        let v = height_on(there, p.at, p.normal, direction)?;
        if end > start && v.hi() >= span.0 && v.lo() <= span.1 {
            let Some(part) = stretch(&image, start, end)? else {
                return Ok(None);
            };
            out.push(Piece::Curve {
                curve: Box::new(part),
                tangent: false,
                reach,
            });
        }
        start = end;
    }
    Ok(Some(out))
}

/// A plane with an extrusion.
pub(crate) fn plane_extrusion(
    a: &Plane,
    b: &Extrusion,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let Some(b) = extrusion_in_place(b, meter)? else {
        return Ok(None);
    };
    let (profile, d) = b.shape()?;
    let (_, span) = ParamSurface::domain(&b);
    let Some(kind) = untrimmed(profile)? else {
        return Ok(None);
    };
    let p = sheet(a)?;
    let tilt = dot(p.normal, lift(d)?)?;
    let (_, u, v) = a.vectors();
    let flat = sign_of_triple(u, v, d);
    let Some(lean) = decided(flat, tilt) else {
        return Ok(None);
    };
    if lean == Sign::Zero {
        plane_along(a, &kind, d, span, meter)
    } else {
        plane_across(a, &kind, d, span, meter)
    }
}

/// The sign of (u × v) · d where all are whole: the triple product of three grid directions.
fn sign_of_triple(
    u: [f64; 3],
    v: [f64; 3],
    d: [f64; 3],
) -> Option<Result<Sign, geom_num::NumError>> {
    let o = crate::curve::grid_point(&[0.0; 3])?;
    let plane = quadric::Plane {
        a: o,
        b: crate::curve::grid_point(&u)?,
        c: crate::curve::grid_point(&v)?,
    };
    Some(quadric::plane_along(
        &plane,
        o,
        crate::curve::grid_point(&d)?,
    ))
}

/// A sweep along one direction: its cross-section, where along the unit direction each point of it
/// starts, and how far past that it runs.
struct Sweep {
    section: CurveKind<2>,
    /// The swept curve, on the section's parameters: where it stands along the direction is where
    /// the sweep starts there. `None` for a cylinder, whose every point starts at `from`.
    profile: Option<CurveKind<3>>,
    from: f64,
    run: (f64, f64),
}

impl Sweep {
    /// How far along the unit direction `d` the sweep's points over `t` start, enclosed: on a
    /// whole profile circle `t` may run across its join (decision 32.104).
    fn height(
        &self,
        t: Interval,
        d: Vector,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Interval, IntersectError> {
        let Some(profile) = &self.profile else {
            return point(self.from);
        };
        let b = bound_on(profile, t, meter)?;
        let ([x0, y0, z0], [x1, y1, z1]) = (b.lo, b.hi);
        let at = [
            Interval::new(x0, x1)?,
            Interval::new(y0, y1)?,
            Interval::new(z0, z1)?,
        ];
        dot(at, d)
    }

    /// How far along `d` the sweep reaches at its points over `t`: from the middle of their start,
    /// and how wide that start is.
    fn reach(
        &self,
        t: Interval,
        d: Vector,
        meter: &mut WorkMeter<'_>,
    ) -> Result<((f64, f64), f64), IntersectError> {
        let h = self.height(t, d, meter)?;
        let at = tight::middle(h);
        Ok(((at + self.run.0, at + self.run.1), h.hi() - h.lo()))
    }

    /// Every height the sweep reaches, enclosed.
    fn whole(&self, d: Vector, meter: &mut WorkMeter<'_>) -> Result<(f64, f64), IntersectError> {
        let (lo, hi) = match &self.profile {
            Some(profile) => finite(profile)?,
            None => (0.0, 0.0),
        };
        let h = self.height(Interval::new(lo, hi)?, d, meter)?;
        Ok((h.lo() + self.run.0, h.hi() + self.run.1))
    }
}

/// The cross-section of `s` on `map`, if it sweeps along `direction`: of a surface [`in_place`]
/// gave.
fn sweep(
    s: &SurfaceKind,
    direction: [f64; 3],
    map: &Affine<2>,
    unit_d: Vector,
) -> Result<Option<Sweep>, IntersectError> {
    match s {
        SurfaceKind::Extrusion(x) => {
            let (profile, d) = x.shape()?;
            if !crate::quadrics::parallel(d, direction) {
                return Ok(None);
            }
            let Some(kind) = untrimmed(profile)? else {
                return Ok(None);
            };
            let Some((section, _)) = map.image(&kind)? else {
                return Ok(None);
            };
            let (_, (v0, v1)) = ParamSurface::domain(x);
            let scale_d = tight::middle(dot(lift(d)?, unit_d)?);
            let (a, b) = (v0 * scale_d, v1 * scale_d);
            Ok(Some(Sweep {
                section,
                profile: Some(kind),
                from: 0.0,
                run: (tight::smaller(a, b), tight::larger(a, b)),
            }))
        }
        SurfaceKind::Cylinder(x) => {
            let e = x.frame().normal();
            if !crate::quadrics::parallel(e, direction) || !whole_turn(s) {
                return Ok(None);
            }
            let c = lift(x.center())?;
            let [cx, cy] = map.at_point(c)?;
            let frame = Frame::<2>::standard()?;
            let circle = Circle::new(
                [tight::middle(cx), tight::middle(cy)],
                x.radius(),
                frame,
                (0.0, 2.0),
            )?;
            let (_, (v0, v1)) = ParamSurface::domain(x);
            let from = tight::middle(dot(c, unit_d)?);
            // The point at v stands s·v along the unit axis n̂: s·v·(n̂ · d̂) along d̂. The axial
            // factor s is ±1 as built, but a similarity scales it, and the stored normal need not
            // be of unit length.
            let (_, [_, _, n]) = x.frame().unit_vectors()?;
            let rate = tight::middle(tight::mul(dot(n, unit_d)?, tight::point(x.axial_sign())?)?);
            let (a, b) = (v0 * rate, v1 * rate);
            Ok(Some(Sweep {
                section: CurveKind::Circle(circle),
                profile: None,
                from,
                run: (tight::smaller(a, b), tight::larger(a, b)),
            }))
        }
        _ => Ok(None),
    }
}

/// Two sweeps of one direction: lines along it at the crossings of their cross-sections.
pub(crate) fn sweeps(
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Piece>>, IntersectError> {
    let (Some(a), Some(b)) = (in_place(a, meter)?, in_place(b, meter)?) else {
        return Ok(None);
    };
    let (a, b) = (&a, &b);
    let direction = match (a, b) {
        (_, SurfaceKind::Extrusion(x)) | (SurfaceKind::Extrusion(x), _) => x.shape()?.1,
        _ => return Ok(None),
    };
    let d = unit(lift(direction)?)?;
    let side = unit(cross(d, lift(beside(direction))?)?)?;
    let other = cross(d, side)?;
    let origin = [point(0.0)?; 3];
    let map = Affine::onto(origin, [side, other])?;
    let (Some(first), Some(second)) =
        (sweep(a, direction, &map, d)?, sweep(b, direction, &map, d)?)
    else {
        return Ok(None);
    };
    let ((a0, a1), (b0, b1)) = (first.whole(d, meter)?, second.whole(d, meter)?);
    if tight::larger(a0, b0) >= tight::smaller(a1, b1) {
        return Ok(Some(Vec::new()));
    }
    let mut out = Vec::new();
    for item in curves(&first.section, &second.section, meter)?.items() {
        match item {
            Item::Point { at, contact, .. } => {
                let [t, s, ..] = at.spans() else {
                    return Err(IntersectError::Internal);
                };
                // Each sweep runs from where its profile stands at the crossing. On a whole
                // cross-section circle a span across its join is read modulo the turn (decision
                // 32.104).
                let ((a0, a1), wide_a) = first.reach(*t, d, meter)?;
                let ((b0, b1), wide_b) = second.reach(*s, d, meter)?;
                let (lo, hi) = (tight::larger(a0, b0), tight::smaller(a1, b1));
                if lo >= hi {
                    continue;
                }
                let box2 = bound_on(&first.section, *t, meter)?;
                let ([x0, y0], [x1, y1]) = (box2.lo, box2.hi);
                let from = plus(
                    scale(side, Interval::new(x0, x1)?)?,
                    scale(other, Interval::new(y0, y1)?)?,
                )?;
                let tangent = !matches!(contact, PointContact::Crossing);
                let spread = tight::larger(wide_a, wide_b);
                out.push(along(from, middle(d), (lo, hi), tangent, spread)?);
            }
            Item::Overlap { .. } => return Ok(Some(alloc::vec![Piece::Same])),
            Item::Flagged { .. } => return Ok(None),
            Item::Curve { .. } => {}
        }
    }
    Ok(Some(out))
}

/// x ↦ `at` + Σ xᵢ·`columns`[i], from space to `D` dimensions, enclosed.
#[derive(Clone, Copy)]
struct Affine<const D: usize> {
    at: [Interval; D],
    columns: [[Interval; D]; 3],
}

/// The vector with the entries of `v` on the rows `k` of each column.
impl Affine<2> {
    /// The coordinates of x − `origin` along the two axes.
    fn onto(origin: Vector, [p, q]: [Vector; 2]) -> Result<Affine<2>, IntersectError> {
        let [a, b, c] = p;
        let [e, f, g] = q;
        let at = [dot(origin, p)?.neg(), dot(origin, q)?.neg()];
        Ok(Affine {
            at,
            columns: [[a, e], [b, f], [c, g]],
        })
    }

    /// The point `x` maps to.
    fn at_point(&self, x: Vector) -> Result<[Interval; 2], IntersectError> {
        self.point(x)
    }

    /// This map, then (ρ, z) ↦ `to` + ρ·`out` + z·`up`: a turn about the axis.
    fn then_into(&self, to: Vector, out: Vector, up: Vector) -> Result<Affine<3>, IntersectError> {
        let [r, z] = self.at;
        let [[a, e], [b, f], [c, g]] = self.columns;
        let image = |p: Interval, q: Interval| -> Result<Vector, IntersectError> {
            plus(scale(out, p)?, scale(up, q)?)
        };
        Ok(Affine {
            at: plus(to, image(r, z)?)?,
            columns: [image(a, e)?, image(b, f)?, image(c, g)?],
        })
    }
}

impl Affine<3> {
    /// The projection along `d` onto the plane through `c` of normal `n`.
    fn along(c: Vector, n: Vector, d: Vector) -> Result<Affine<3>, IntersectError> {
        let tilt = dot(d, n)?;
        let k = dot(c, n)?.div(tilt)?;
        let at = scale(d, k)?;
        let one = point(1.0)?;
        let zero = point(0.0)?;
        let mut columns = [[zero; 3]; 3];
        for (i, column) in columns.iter_mut().enumerate() {
            let ni = n
                .get(i)
                .copied()
                .ok_or(IntersectError::Internal)?
                .div(tilt)?;
            for (j, slot) in column.iter_mut().enumerate() {
                let dj = d.get(j).copied().ok_or(IntersectError::Internal)?;
                let identity = if i == j { one } else { zero };
                *slot = identity.sub(dj.mul(ni)?)?;
            }
        }
        Ok(Affine { at, columns })
    }
}

impl<const D: usize> Affine<D> {
    /// The direction `x` maps to.
    fn vector(&self, x: Vector) -> Result<[Interval; D], IntersectError> {
        let mut out = [point(0.0)?; D];
        let [a, b, c] = x;
        let [p, q, r] = &self.columns;
        for (k, slot) in out.iter_mut().enumerate() {
            let cell = |w: &[Interval; D]| w.get(k).copied().ok_or(IntersectError::Internal);
            *slot = a
                .mul(cell(p)?)?
                .add(b.mul(cell(q)?)?)?
                .add(c.mul(cell(r)?)?)?;
        }
        Ok(out)
    }

    /// The point `x` maps to.
    fn point(&self, x: Vector) -> Result<[Interval; D], IntersectError> {
        let mut out = self.vector(x)?;
        for (slot, at) in out.iter_mut().zip(&self.at) {
            *slot = slot.add(*at)?;
        }
        Ok(out)
    }

    /// The conic's axes.
    fn axes(&self, axes: &Axes<3>) -> Result<Option<(Axes<D>, f64)>, IntersectError> {
        let (u, v) = match axes {
            Axes::Vectors { u, v } => (lift(*u)?, lift(*v)?),
            Axes::Frame { frame, a, b } => {
                let (_, [x, y, _]) = frame.unit_vectors()?;
                (scale(x, point(*a)?)?, scale(y, point(*b)?)?)
            }
        };
        let (p, q) = (self.vector(u)?, self.vector(v)?);
        let reach = tight::larger(wide_of(&p), wide_of(&q));
        Ok(Some((
            Axes::Vectors {
                u: middle_of(&p),
                v: middle_of(&q),
            },
            reach,
        )))
    }

    /// The curve `curve` maps to, of the kind the map keeps, and how far its data reach. `None` for
    /// a kind no map here takes.
    fn image(&self, curve: &CurveKind<3>) -> Result<Option<(CurveKind<D>, f64)>, IntersectError> {
        Ok(Some(match curve {
            CurveKind::Line(l) => {
                let at = self.point(lift(l.origin())?)?;
                let d = self.vector(lift(l.direction())?)?;
                let line = Line::new(middle_of(&at), middle_of(&d), finite(curve)?)?;
                (
                    CurveKind::Line(line),
                    tight::larger(wide_of(&at), wide_of(&d)),
                )
            }
            CurveKind::Circle(c) => {
                let (_, [x, y, _]) = c.frame().unit_vectors()?;
                let r = point(c.radius())?;
                let axes = Axes::Vectors {
                    u: middle(scale(x, r)?),
                    v: middle(scale(y, r)?),
                };
                let center = self.point(lift(c.center())?)?;
                let Some((mapped, reach)) = self.axes(&axes)? else {
                    return Ok(None);
                };
                let reach = tight::larger(wide_of(&center), reach);
                if let Axes::Vectors { u, v } = &mapped
                    && let Some(frame) = turned(u, v, c.radius())?
                {
                    let image = Circle::new(middle_of(&center), c.radius(), frame, c.domain())?;
                    return Ok(Some((CurveKind::Circle(image), reach)));
                }
                (
                    CurveKind::Ellipse(Ellipse::new(middle_of(&center), mapped, c.domain())?),
                    reach,
                )
            }
            CurveKind::Ellipse(e) => {
                let center = self.point(lift(e.center())?)?;
                let Some((mapped, reach)) = self.axes(&e.axes())? else {
                    return Ok(None);
                };
                let image = Ellipse::new(middle_of(&center), mapped, e.domain())?;
                (
                    CurveKind::Ellipse(image),
                    tight::larger(wide_of(&center), reach),
                )
            }
            CurveKind::Hyperbola(h) => {
                let center = self.point(lift(h.center())?)?;
                let Some((mapped, reach)) = self.axes(&h.axes())? else {
                    return Ok(None);
                };
                let image = Hyperbola::new(middle_of(&center), mapped, h.domain())?;
                (
                    CurveKind::Hyperbola(image),
                    tight::larger(wide_of(&center), reach),
                )
            }
            CurveKind::Parabola(p) => {
                let vertex = self.point(lift(p.vertex())?)?;
                let Some((mapped, reach)) = self.axes(&p.axes())? else {
                    return Ok(None);
                };
                let image = Parabola::new(middle_of(&vertex), mapped, p.domain())?;
                (
                    CurveKind::Parabola(image),
                    tight::larger(wide_of(&vertex), reach),
                )
            }
            CurveKind::Nurbs(n) => {
                let weights = n.weights();
                let mut points = Vec::with_capacity(n.homogeneous().len());
                let mut reach = 0.0_f64;
                for (i, h) in n.homogeneous().iter().enumerate() {
                    let w = weights.and_then(|all| all.get(i)).copied().unwrap_or(1.0);
                    let mut image = self.vector(lift(*h)?)?;
                    for (slot, at) in image.iter_mut().zip(&self.at) {
                        *slot = slot.add(at.mul(point(w)?)?)?;
                    }
                    reach = tight::larger(reach, wide_of(&image));
                    points.push(middle_of(&image));
                }
                let kept = weights.map(<[f64]>::to_vec);
                let Ok(image) = Curve::from_homogeneous(n.basis().clone(), points, kept) else {
                    return Ok(None);
                };
                (CurveKind::Nurbs(image), reach)
            }
            CurveKind::Trimmed(t) => {
                let kind = CurveKind::from(t.basis().clone());
                let Some((whole_image, reach)) = self.image(&kind)? else {
                    return Ok(None);
                };
                let span = t.interval();
                let Ok(basis) = Basis::try_from(whole_image) else {
                    return Ok(None);
                };
                (CurveKind::Trimmed(Trimmed::new(basis, span)?), reach)
            }
            _ => return Ok(None),
        }))
    }
}

/// The frame of two image axes that are square and of length `r`, as a turn leaves them.
fn turned<const D: usize>(
    u: &[f64; D],
    v: &[f64; D],
    r: f64,
) -> Result<Option<Frame<D>>, IntersectError> {
    let (mut uu, mut vv, mut uv) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (a, b) in u.iter().zip(v) {
        uu += a * a;
        vv += b * b;
        uv += a * b;
    }
    let size = r * r;
    let close = |x: f64, y: f64| (x - y).abs() <= size * 1e-12;
    if !close(uu, size) || !close(vv, size) || !close(uv, 0.0) {
        return Ok(None);
    }
    let at = |w: &[f64; D], k: usize| w.get(k).copied().unwrap_or(0.0);
    let (a, b) = (
        [at(u, 0), at(u, 1), at(u, 2)],
        [at(v, 0), at(v, 1), at(v, 2)],
    );
    let [p, q, s] = a;
    let [x, y, z] = b;
    let normal = [q * z - s * y, s * x - p * z, p * y - q * x];
    Ok(Some(Frame::new(*u, normal)?))
}

/// The midpoints of `v`.
fn middle_of<const D: usize>(v: &[Interval; D]) -> [f64; D] {
    v.map(tight::middle)
}

/// The widest entry of `v`.
fn wide_of<const D: usize>(v: &[Interval; D]) -> f64 {
    let mut out = 0.0_f64;
    for x in v {
        out = tight::larger(out, x.hi() - x.lo());
    }
    out
}
