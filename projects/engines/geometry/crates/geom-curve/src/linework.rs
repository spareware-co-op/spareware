//! Surveyed 3D linework (chapter 17.2): the polyline coded field points become, with lines, arcs in
//! plan on a steady grade and smooth figures, joined into one curve that passes within the fit
//! bound of every surveyed point, certified.
//!
//! What it does: [`linework`] takes one figure's points, in order, and the [`Segment`]s that join
//! them. [`Segment::Line`] runs straight to the next point. [`Segment::Arc`] is the circular arc in
//! plan through the next point to the one after, rising on a steady grade: a circular helix
//! ([`Helix`]), or a level arc ([`Circle`]) where its heights are equal, since a helix of no pitch
//! is a circle. An arc whose three heights are not on one grade becomes two pieces meeting at the
//! middle point, so all three heights are kept. [`Segment::Smooth`] is the NURBS curve
//! interpolating the next points (The NURBS Book, A9.1, through `geom-nurbs`), of degree three, or
//! two through three points. The pieces are joined end to end into one [`Composite`], and
//! [`Linework`] holds it with every point's parameter on it and a certified bound on how far the
//! curve at that parameter lies from the point.
//!
//! Guarantees:
//!
//! - **Through every point, within `ε_fit`, certified.** [`Linework::bound`] is an upper bound, from
//!   interval enclosures of the composite curve as returned, on the distance of each surveyed point
//!   from the curve at its parameter, from both sides where pieces join; it is at most the fit
//!   bound `ε_fit` ([`FIT`], 64 steps). A figure that cannot be shown within it fails with
//!   [`LineworkError::OverFit`], never returns a curve that misses a point.
//! - **Exact where it can be.** A line between grid points is exact, so linework of lines alone is
//!   [`Exactness::Exact`]; anything with an arc or a figure is Certified with the bound
//!   ([`Linework::exactness`]).
//! - **Decided in integers.** Whether an arc turns left or right, has its points collinear in plan,
//!   and lies on one grade is decided exactly on the grid coordinates. One grade means three equal
//!   heights (a level arc), or equal plan chords with equal rises (one helix): then the two halves
//!   turn through equal angles, so the grades are equal exactly. Any other arc is two pieces, which
//!   keeps every height whatever the grades are, so no tolerance decides it.
//! - **Deterministic.** The curves' doubles come from double-double arithmetic and the kernel's
//!   `atan2` and `sqrt` (`geom_num::math`), in a fixed order, so every target gets the same bits.
//! - **Refused, never bent.** A repeated point, an arc collinear in plan, a smooth figure through
//!   fewer than three points, and segments that do not use exactly the points given are typed
//!   errors that name the point or segment; nothing is moved to make a figure fit.
//!
//! Method: chapter 17.2's linework (`projects/engines/geometry/architecture/17-survey-terrain-civil.md`): the
//! arc's center is the circumcenter of its plan points, from the determinant formula in
//! double-double arithmetic; each piece starts at its first point, with its frame's reference
//! vector toward it and its normal up for an arc turning left (down for one turning right), so the
//! parameter runs from 0 over the piece's turn in half-turns, from the kernel's `atan2`; its pitch
//! is twice its rise over that turn. Smooth figures follow The NURBS Book's global interpolation
//! (L. Piegl and W. Tiller, 2nd ed., algorithm A9.1, <https://doi.org/10.1007/978-3-642-59223-2>)
//! as `geom-nurbs` ports it. Pieces join as chapter 14's composite curves, each joint keeping its
//! certified gap (`crate::composite`).
//!
//! Cost: linear in the points: an arc is a few dozen double-double operations and two `atan2`s; a
//! figure of n points is `geom-nurbs`'s O(n) banded solve and certification; every point is checked
//! with one or two enclosures of the composite curve, each a binary search over its pieces. One
//! work unit is charged per segment, beside what the interpolation and enclosures charge. Nothing
//! recurses.

use alloc::vec::Vec;
use core::f64::consts::PI;
use core::fmt;

use geom_num::ladder::FIT;
use geom_num::{Coord, DD, Exactness, GridStep, P3, Real, Steps, math};
use geom_nurbs::{Curve, FitVersion, Limit, Parametrization};
use geom_run::WorkMeter;

use crate::axes::HelixAxes;
use crate::basis::Basis;
use crate::composite::Composite;
use crate::conic::Circle;
use crate::error::CurveError;
use crate::frame::Frame;
use crate::helix::Helix;
use crate::line::Line;
use crate::param::ParamCurve;
use crate::tight;
use crate::trimmed::Trimmed;

/// The fit bound `ε_fit` as a double: 64 steps, exactly.
const FIT_STEPS: f64 = 64.0;

// The double above is the ladder's rung.
const _: () = assert!(FIT.0 == 64);

/// The highest degree of a smooth figure: cubic, as The NURBS Book's interpolation usually is.
pub const SMOOTH_DEGREE: usize = 3;

/// How one run of a figure joins its points (chapter 17.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Segment {
    /// A straight line to the next point.
    Line,
    /// A circular arc in plan through the next point to the one after, on a steady grade where the
    /// heights allow, otherwise two pieces meeting at the middle point (module documentation).
    Arc,
    /// A smooth figure through the next `points` points: the NURBS curve interpolating them and the
    /// point before, of degree [`SMOOTH_DEGREE`] or one less than their number.
    Smooth {
        /// How many points after the run's first it passes through: at least two.
        points: usize,
        /// How the interpolation's parameters are chosen.
        rule: Parametrization,
    },
}

impl Segment {
    /// How many points after its first the segment uses.
    #[must_use]
    pub const fn uses(self) -> usize {
        match self {
            Segment::Line => 1,
            Segment::Arc => 2,
            Segment::Smooth { points, .. } => points,
        }
    }
}

/// Why no linework was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineworkError {
    /// No segments, or segments that do not use exactly the points given: `needed` points for the
    /// segments (saturating), `points` given.
    Count {
        /// The points given.
        points: usize,
        /// The points the segments use.
        needed: usize,
    },
    /// The smooth figure of segment `segment` passes through fewer than three points; a line or an
    /// arc says what two or three points need.
    Short {
        /// The segment, counting from 0.
        segment: usize,
    },
    /// Point `index` is the point before it again.
    Repeated {
        /// The point, counting from 0.
        index: usize,
    },
    /// The arc of segment `segment` has its three points on one line in plan, or two of them at
    /// one plan position, so no circle in plan passes through them.
    Collinear {
        /// The segment, counting from 0.
        segment: usize,
    },
    /// The curve could not be shown within `ε_fit` of every point: the certified bound found, in
    /// steps.
    OverFit {
        /// The bound, in steps.
        bound: Real,
    },
    /// A curve kind refused its data, or the meter stopped.
    Curve(CurveError),
}

impl fmt::Display for LineworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineworkError::Count { points, needed } => write!(
                f,
                "the segments use {needed} points, and {points} are given"
            ),
            LineworkError::Short { segment } => write!(
                f,
                "segment {segment} is a smooth figure through fewer than three points"
            ),
            LineworkError::Repeated { index } => {
                write!(f, "point {index} repeats the point before it")
            }
            LineworkError::Collinear { segment } => write!(
                f,
                "the arc of segment {segment} has its points on one line in plan"
            ),
            LineworkError::OverFit { bound } => write!(
                f,
                "the linework is certified only within {} steps of its points, past the fit bound \
                 of 64",
                bound.get()
            ),
            LineworkError::Curve(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for LineworkError {}

impl From<CurveError> for LineworkError {
    fn from(e: CurveError) -> Self {
        LineworkError::Curve(e)
    }
}

impl From<geom_num::NumError> for LineworkError {
    fn from(e: geom_num::NumError) -> Self {
        LineworkError::Curve(CurveError::from(e))
    }
}

impl From<geom_nurbs::NurbsError> for LineworkError {
    fn from(e: geom_nurbs::NurbsError) -> Self {
        LineworkError::Curve(CurveError::from(e))
    }
}

impl From<geom_run::Stopped> for LineworkError {
    fn from(e: geom_run::Stopped) -> Self {
        LineworkError::Curve(CurveError::from(e))
    }
}

/// A figure's linework: one composite curve, every surveyed point's parameter on it, and the
/// certified bound on their distances from it (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Linework {
    curve: Composite<3>,
    parameters: Vec<f64>,
    bound: f64,
}

impl Linework {
    /// The curve: its pieces in order, lines, helices, level arcs and NURBS figures, each trimmed
    /// to its run.
    #[must_use]
    pub fn curve(&self) -> &Composite<3> {
        &self.curve
    }

    /// The parameter on [`Linework::curve`] of each surveyed point, in the points' order.
    #[must_use]
    pub fn parameters(&self) -> &[f64] {
        &self.parameters
    }

    /// The certified bound, in steps: no surveyed point lies further than this from the curve at
    /// its parameter. At most `ε_fit`; 0 when every piece is a line.
    #[must_use]
    pub fn bound(&self) -> f64 {
        self.bound
    }

    /// The linework's exactness on a frame of grid step `step`: Exact when its bound is 0,
    /// otherwise Certified with its bound rounded up to whole steps, at most [`FIT`].
    #[must_use]
    pub fn exactness(&self, step: GridStep) -> Exactness {
        if self.bound == 0.0 {
            return Exactness::Exact;
        }
        // The bound is at most 64 steps (checked when made), so the least whole number of steps at
        // or above it is found among 1 to 64.
        let whole = (1_u32..=64)
            .find(|&n| self.bound <= f64::from(n))
            .map_or(FIT, |n| Steps(u64::from(n)));
        Exactness::Certified { bound: whole, step }
    }
}

/// A grid coordinate as a double: exact, since |x| ≤ 2^40 − 1 < 2^53 (`DD::from_i64` rounds to
/// nearest, which changes no integer below 2^53).
fn double(x: Coord) -> f64 {
    DD::from_i64(x.get()).to_f64()
}

/// A grid point as doubles, exactly.
fn doubles(p: P3) -> [f64; 3] {
    [double(p.x), double(p.y), double(p.z)]
}

/// A difference of two grid coordinates as a double: exact, since |a − b| < 2^41 < 2^53.
fn difference(a: Coord, b: Coord) -> Result<f64, LineworkError> {
    let d = a
        .get()
        .checked_sub(b.get())
        .ok_or(LineworkError::Curve(CurveError::Defect))?;
    Ok(DD::from_i64(d).to_f64())
}

/// The sign of the plan turn a → m → b, exactly: the cross product (m − a) × (b − a) in `i128`
/// (each factor below 2^41, so each product below 2^82).
fn turn(a: P3, m: P3, b: P3) -> Option<i128> {
    let d = |p: Coord, q: Coord| i128::from(p.get()).checked_sub(i128::from(q.get()));
    let (mx, my) = (d(m.x, a.x)?, d(m.y, a.y)?);
    let (bx, by) = (d(b.x, a.x)?, d(b.y, a.y)?);
    mx.checked_mul(by)?.checked_sub(my.checked_mul(bx)?)
}

/// The squared plan distance from `a` to `b`, exactly, in `i128` (below 2^83).
fn chord2(a: P3, b: P3) -> Option<i128> {
    let dx = i128::from(b.x.get()).checked_sub(i128::from(a.x.get()))?;
    let dy = i128::from(b.y.get()).checked_sub(i128::from(a.y.get()))?;
    dx.checked_mul(dx)?.checked_add(dy.checked_mul(dy)?)
}

/// The circumcenter in plan of `a`, `m` and `b`, not collinear: with B = m − a and C = b − a,
/// `a + (Cy·|B|² − By·|C|², Bx·|C|² − Cx·|B|²) / (2·(Bx·Cy − By·Cx))`, each part in
/// double-double arithmetic and rounded once at the end.
fn circumcenter(a: P3, m: P3, b: P3) -> Result<[f64; 2], LineworkError> {
    let defect = LineworkError::Curve(CurveError::Defect);
    let (bx, by) = (difference(m.x, a.x)?, difference(m.y, a.y)?);
    let (cx, cy) = (difference(b.x, a.x)?, difference(b.y, a.y)?);
    let product = |p: f64, q: f64| DD::exact_product(p, q).ok_or(defect);
    let b2 = product(bx, bx)?.add(product(by, by)?)?;
    let c2 = product(cx, cx)?.add(product(cy, cy)?)?;
    let det = product(bx, cy)?.sub(product(by, cx)?)?.mul_f64(2.0)?;
    let ux = b2.mul_f64(cy)?.sub(c2.mul_f64(by)?)?.div(det)?;
    let uy = c2.mul_f64(bx)?.sub(b2.mul_f64(cx)?)?.div(det)?;
    Ok([
        ux.add_f64(double(a.x))?.to_f64(),
        uy.add_f64(double(a.y))?.to_f64(),
    ])
}

/// The turn about `center` from `from` to `to`, in half-turns in (0, 2): anticlockwise when `left`,
/// otherwise clockwise, by the kernel's `atan2` of the plan cross and dot products.
fn sweep(center: [f64; 2], from: P3, to: P3, left: bool) -> Result<f64, LineworkError> {
    let [cx, cy] = center;
    let (ux, uy) = (double(from.x) - cx, double(from.y) - cy);
    let (vx, vy) = (double(to.x) - cx, double(to.y) - cy);
    let cross = ux * vy - uy * vx;
    let dot = ux * vx + uy * vy;
    let signed = if left { cross } else { -cross };
    let half_turns = math::atan2(signed, dot)?.get() / PI;
    let turned = if half_turns > 0.0 {
        half_turns
    } else {
        half_turns + 2.0
    };
    if turned > 0.0 && turned < 2.0 {
        Ok(turned)
    } else {
        Err(LineworkError::Curve(CurveError::Degenerate))
    }
}

/// One piece of an arc: from `start` about `center`, turning through `turn` half-turns (left when
/// `left`) while rising `rise` steps: a level arc when the rise is 0, otherwise a circular helix.
fn arc_piece(
    center: [f64; 2],
    start: P3,
    rise: f64,
    turn: f64,
    left: bool,
) -> Result<Trimmed<3>, LineworkError> {
    let [cx, cy] = center;
    let x = [double(start.x) - cx, double(start.y) - cy, 0.0];
    let [x0, x1, _] = x;
    let radius = math::sqrt(x0 * x0 + x1 * x1)?.get();
    let up = if left { 1.0 } else { -1.0 };
    let frame = Frame::new(x, [0.0, 0.0, up])?;
    let base = [cx, cy, double(start.z)];
    let basis = if rise == 0.0 {
        Basis::Circle(Circle::new(base, radius, frame, (0.0, turn))?)
    } else {
        // Height = base + t·(pitch/2)·n̂ with n̂ = ±z, so a rise over `turn` half-turns needs
        // pitch = 2·rise/turn along n̂.
        let pitch = up * 2.0 * rise / turn;
        let axes = HelixAxes::Frame {
            frame,
            radius,
            pitch,
        };
        Basis::Helix(Helix::new(base, axes, (0.0, turn))?)
    };
    Ok(Trimmed::whole(basis)?)
}

/// A piece of the curve, and the local parameters of the surveyed points strictly inside it.
struct Piece {
    curve: Trimmed<3>,
    inner: Vec<f64>,
}

/// The pieces of the arc `segment` from `a` through `m` to `b`.
fn arc(a: P3, m: P3, b: P3, segment: usize) -> Result<Vec<Piece>, LineworkError> {
    let defect = LineworkError::Curve(CurveError::Defect);
    let orientation = turn(a, m, b).ok_or(defect)?;
    if orientation == 0 {
        return Err(LineworkError::Collinear { segment });
    }
    let left = orientation > 0;
    let center = circumcenter(a, m, b)?;
    let first = sweep(center, a, m, left)?;
    let second = sweep(center, m, b, left)?;
    let rise = |p: P3, q: P3| difference(q.z, p.z);
    let level = a.z == m.z && m.z == b.z;
    let even =
        chord2(a, m).ok_or(defect)? == chord2(m, b).ok_or(defect)? && rise(a, m)? == rise(m, b)?;
    if level || even {
        let whole = arc_piece(center, a, rise(a, b)?, first + second, left)?;
        return Ok(Vec::from([Piece {
            curve: whole,
            inner: Vec::from([first]),
        }]));
    }
    Ok(Vec::from([
        Piece {
            curve: arc_piece(center, a, rise(a, m)?, first, left)?,
            inner: Vec::new(),
        },
        Piece {
            curve: arc_piece(center, m, rise(m, b)?, second, left)?,
            inner: Vec::new(),
        },
    ]))
}

/// The smooth figure through `run`, by `rule`.
fn smooth(
    run: &[P3],
    rule: Parametrization,
    segment: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Piece, LineworkError> {
    if run.len() < 3 {
        return Err(LineworkError::Short { segment });
    }
    let points: Vec<[f64; 3]> = run.iter().map(|&p| doubles(p)).collect();
    let degree = if run.len() > SMOOTH_DEGREE {
        SMOOTH_DEGREE
    } else {
        run.len().saturating_sub(1)
    };
    let fit = Curve::interpolate(&points, degree, rule, FitVersion::CURRENT, meter)?;
    let last = fit.parameters.len().saturating_sub(1);
    let inner = fit.parameters.get(1..last).unwrap_or_default().to_vec();
    Ok(Piece {
        curve: Trimmed::whole(Basis::Nurbs(fit.curve))?,
        inner,
    })
}

/// The pieces of every segment, in order.
fn pieces(
    points: &[P3],
    segments: &[Segment],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Piece>, LineworkError> {
    let defect = LineworkError::Curve(CurveError::Defect);
    let mut out = Vec::with_capacity(segments.len());
    let mut first = 0_usize;
    for (k, &segment) in segments.iter().enumerate() {
        meter.charge(1)?;
        let last = first.checked_add(segment.uses()).ok_or(defect)?;
        let run = points.get(first..=last).ok_or(defect)?;
        match (segment, run) {
            (Segment::Line, &[a, b]) => {
                let line = Line::segment(doubles(a), doubles(b))?;
                out.push(Piece {
                    curve: Trimmed::whole(Basis::Line(line.value))?,
                    inner: Vec::new(),
                });
            }
            (Segment::Arc, &[a, m, b]) => out.extend(arc(a, m, b, k)?),
            (Segment::Smooth { rule, .. }, _) => out.push(smooth(run, rule, k, meter)?),
            _ => return Err(defect),
        }
        first = last;
    }
    Ok(out)
}

/// The surveyed points' parameters on `curve`, made of `pieces`, with the side each is checked
/// from: a piece's start from the right, a point inside a piece from the right, and the last end
/// from the left; a joint is also checked from the left, as the end of the piece before.
fn placed(curve: &Composite<3>, pieces: &[Piece]) -> Result<Vec<(f64, bool)>, LineworkError> {
    let defect = LineworkError::Curve(CurveError::Defect);
    let starts = curve.starts();
    let mut out = Vec::new();
    for (k, piece) in pieces.iter().enumerate() {
        let s0 = *starts.get(k).ok_or(defect)?;
        let s1 = *starts.get(k.checked_add(1).ok_or(defect)?).ok_or(defect)?;
        let (a, b) = piece.curve.interval();
        // A joint after the first piece is checked from both sides.
        out.push((s0, k > 0));
        let ratio = (s1 - s0) / (b - a);
        for &t in &piece.inner {
            out.push((s0 + (t - a) * ratio, false));
        }
    }
    let end = *starts.last().ok_or(defect)?;
    out.push((end, true));
    Ok(out)
}

/// An upper bound on the distance of `p` from `curve` at `s`, from `side`.
fn distance(
    curve: &Composite<3>,
    s: f64,
    side: Limit,
    p: &[f64; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<f64, LineworkError> {
    let e = curve.enclose(s, side, 0, meter)?;
    let at = e.first().ok_or(LineworkError::Curve(CurveError::Defect))?;
    Ok(tight::distance_to(p, at)?)
}

/// The linework of one figure: `points` in order, joined by `segments` (module documentation).
///
/// # Errors
///
/// [`LineworkError::Count`] unless there are segments and they use exactly the points given;
/// [`LineworkError::Repeated`] for a point equal to the one before it;
/// [`LineworkError::Collinear`] for an arc collinear in plan; [`LineworkError::Short`] for a
/// smooth figure through fewer than three points; [`LineworkError::OverFit`] if the curve cannot
/// be certified within `ε_fit` of every point; [`LineworkError::Curve`] if a curve kind refuses its
/// data (an arc so nearly a full turn that its turn rounds to two half-turns) or the meter stops.
pub fn linework(
    points: &[P3],
    segments: &[Segment],
    meter: &mut WorkMeter<'_>,
) -> Result<Linework, LineworkError> {
    let needed = segments
        .iter()
        .try_fold(1_usize, |n, s| n.checked_add(s.uses()));
    if segments.is_empty() || needed != Some(points.len()) {
        return Err(LineworkError::Count {
            points: points.len(),
            needed: needed.unwrap_or(usize::MAX),
        });
    }
    if let Some(index) = (1_usize..)
        .zip(points.windows(2))
        .find_map(|(i, w)| matches!(w, [p, q] if p == q).then_some(i))
    {
        return Err(LineworkError::Repeated { index });
    }
    let pieces = pieces(points, segments, meter)?;
    let joined: Vec<Trimmed<3>> = pieces.iter().map(|p| p.curve.clone()).collect();
    // Each joint is two pieces' ends at one surveyed point, each within `ε_fit` of it when the
    // linework passes the checks below, so within 2·`ε_fit` of each other; the composite records
    // each joint's certified gap, and the checks below bound every end's distance from its point.
    let allow = Real::new(2.0 * FIT_STEPS)?;
    let curve = Composite::starting_within(joined, 0.0, allow.get()).map_err(|e| match e {
        CurveError::Disjoint => LineworkError::OverFit { bound: allow },
        other => LineworkError::Curve(other),
    })?;
    let placed = placed(&curve, &pieces)?;
    let mut bound = 0.0_f64;
    let mut parameters = Vec::with_capacity(points.len());
    for (&p, &(s, both)) in points.iter().zip(&placed) {
        let xyz = doubles(p);
        let last = parameters.len().checked_add(1) == Some(points.len());
        let side = if last {
            Limit::FromLeft
        } else {
            Limit::FromRight
        };
        let mut d = distance(&curve, s, side, &xyz, meter)?;
        if both && !last {
            let left = distance(&curve, s, Limit::FromLeft, &xyz, meter)?;
            if left > d {
                d = left;
            }
        }
        if d > bound {
            bound = d;
        }
        parameters.push(s);
    }
    if parameters.len() != points.len() {
        return Err(LineworkError::Curve(CurveError::Defect));
    }
    if bound > FIT_STEPS {
        return Err(LineworkError::OverFit {
            bound: Real::new(bound)?,
        });
    }
    Ok(Linework {
        curve,
        parameters,
        bound,
    })
}
