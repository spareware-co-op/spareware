//! Frame rules: how a swept profile turns as it travels along its path, the stored data of each
//! rule and its evaluation (chapter 13.3; piece G254).
//!
//! What it does: a [`FrameRule`] says where a profile is at each parameter `t` of a path, as a
//! [`Placement`]: an origin, a right-handed orthonormal frame (x̂ and ŷ span the profile's plane,
//! ẑ is the direction of travel) and a scale. A profile point (p, q) lands at
//! origin + scale·(p·x̂ + q·ŷ). [`placement`] evaluates any rule, with the twist and scale
//! [`Laws`] on top of it. The rules:
//!
//! - [`FrameRule::Translate`]: the profile never turns; its frame is the world's.
//! - [`FrameRule::RotationMinimising`]: the least twist, by the double reflection method, from
//!   station frames stored bit for bit ([`Stations`]) and the C1 rule between them (below).
//! - [`FrameRule::Frenet`]: x̂ the principal normal, ŷ the binormal; undefined where the
//!   curvature is zero, which [`frenet_defined`] finds with certified bounds.
//! - [`FrameRule::FixedReference`]: IFC's fixed reference sweep, the profile's x axis towards a
//!   fixed direction ([`ProfileAxis::X`]), or an up-vector ([`ProfileAxis::Y`]) its y axis;
//!   undefined where the tangent turns along the direction, which [`reference_defined`] finds.
//! - [`FrameRule::Placements`]: IFC's placements along a directrix, as `geom-align` supplies
//!   them, each frame given at its parameter and the turn between two of them shared out evenly.
//! - [`FrameRule::Helical`]: a screw motion about an axis, its pitch and radius each a [`Law`] of
//!   the turned angle; a rigid screw motion when both are constant.
//!
//! The guide, two-rail and surface-normal rules, mitres and the fold tests are piece G256's.
//!
//! **Rotation-minimising frames.** One double-reflection step (Wang, Jüttler, Zheng and Liu) takes
//! a frame (t̂ᵢ, r̂ᵢ) at xᵢ to one at x: reflect in the plane bisecting xᵢ and x
//! (v₁ = x − xᵢ: r̂ᴸ = r̂ᵢ − (2 v₁·r̂ᵢ / v₁·v₁) v₁, t̂ᴸ likewise), then in the plane that takes t̂ᴸ
//! to the path's tangent t̂ there (v₂ = t̂ − t̂ᴸ: r̂ = r̂ᴸ − (2 v₂·r̂ᴸ / v₂·v₂) v₂). [`Stations`]
//! holds frames F₀ … Fₙ at s₀ < … < sₙ, each one step from the last, placed by a deterministic
//! rule until each step's estimated twist error times the profile's radius is within the asked
//! tolerance, and stored as the doubles the steps produced (decision 32.28). The estimate is the
//! method's own: a step's error is fifth order in its length, so two half steps err a sixteenth
//! as much as one, and the stored step's error is 16/15 of the difference between one step and
//! two half steps (Richardson's argument). Each step is an orthogonal map that takes one station's
//! plane across the tangent to the next one's, so the steps' errors add along the path. Between
//! stations the frame at s is the step from Fᵢ to s turned about t̂ by a cubic correction h, zero
//! at both stations, whose end slopes cancel the step's own spin about t̂ there: the step's spin is
//! zero at sᵢ (it agrees with the true frame to fifth order) and ωᵢ₊₁ at sᵢ₊₁, so
//! h(u) = −ωᵢ₊₁·Δ·(u³ − u²), u = (s − sᵢ)/Δ. The frame meets every station exactly and its spin
//! about the tangent is zero at every station from both sides, so it is C1 along the whole path.
//! To leading order the error it adds between two stations rises steadily from 0 to the step's
//! own error e: the step's error u⁵·e and the correction's 5(u² − u³)·e add to
//! (u⁵ − 5u³ + 5u²)·e, whose slope 5u(1 − u)²(u + 2) is never negative. This rule is ours, built on
//! Wang et al.'s step (13.3). On a closed path the closing angle is spread at a constant twist
//! rate, or refused when the caller asks for none ([`Closing`]); the stations are stored as the
//! steps produced them, before the spread.
//!
//! **Closed forms.** [`closed_form_frame`] gives the exact rotation-minimising frame on a line (a
//! constant frame), a circular arc (the start frame turned about the arc's axis by the angle the
//! tangent has turned) and a circular helix (its Frenet frame turned back about the tangent by
//! the torsion's integral, φ = φ₀ − h·θ/L for rise h per radian and L = √(r² + h²)). They hold the
//! stations to the mathematical frame in the laws (`tests/frame_rule.rs`).
//!
//! Guarantees:
//!
//! - **Bit-identical everywhere.** Every frame is a fixed sequence of IEEE additions,
//!   multiplications, divisions and correctly rounded square roots, and angles go through
//!   `geom-num`'s correctly rounded `sinpi`, `cospi` and `atan2`; nothing reads a clock, a thread
//!   or a platform maths library, so a stored rule gives the same frames on every target.
//! - **The stored definition is the shape.** A rotation-minimising rule's frames are the stored
//!   stations and the C1 rule above, whatever later improvements the station rule gets; the
//!   difference from the mathematical frame is measured and reported
//!   ([`Stations::measured_twist`], each step's estimated error, which the steps add), never
//!   claimed proven (32.3, 32.28).
//! - **Undefined is an error, never a guess.** A Frenet frame where the curvature may vanish, a
//!   fixed reference along the tangent, a zero tangent, a scale that is not positive and a law
//!   that is not finite are [`FrameError`]s, each naming where.
//! - **Certified where it says so.** [`frenet_defined`] and [`reference_defined`] decide from the
//!   path's certified derivative bounds (G68), in outward-rounded interval arithmetic: a span they
//!   pass has nonzero curvature (or a tangent off the reference) at every parameter, and a span
//!   they report holds every parameter where it may vanish, to a width of 2⁻²⁴ of the domain.
//! - **Twist and scale on top of any rule.** Laws are functions of the arc length from the path's
//!   start (of the turned angle under a helical rule), the length in closed form on lines, arcs
//!   and helices and by deterministic Gauss-Kronrod quadrature elsewhere; they need a
//!   tangent-continuous path, which the caller's sweep enforces (13.3).
//!
//! Method: the double reflection method of W. Wang, B. Jüttler, D. Zheng and Y. Liu, "Computation
//! of rotation minimizing frames", ACM Transactions on Graphics 27(1), 2008,
//! <https://doi.org/10.1145/1330511.1330513>, as tinyspline converts it (its
//! `ts_bspline_compute_rmf`, lines 1537 to 1687 of `src/tinyspline.c`); the C1 rule between stations is ours (13.3); the rotation-minimising
//! frame's closed forms on lines, circles and helices from its definition (zero spin about the
//! tangent, R. L. Bishop, "There is more than one way to frame a curve", American Mathematical
//! Monthly 82(3), 1975, <https://doi.org/10.1080/00029890.1975.11993807>); IFC's fixed reference
//! swept area solid and its placements along a directrix (ISO 16739-1, `IfcFixedReferenceSweptAreaSolid`
//! and `IfcLinearPlacement`), the turn between two placements by its quaternion from S. W.
//! Shepperd, "Quaternion from rotation matrix", Journal of Guidance and Control 1(3), 1978,
//! <https://doi.org/10.2514/3.55767b>; interval subdivision for the certified zero tests
//! (R. E. Moore, *Interval Analysis*, Prentice-Hall, 1966).
//! Source: PORT from tinyspline, MIT,
//!   <https://github.com/msteinbeck/tinyspline/blob/bb3b02cdfaa039aa236a5240f2ba8bce66bea527/src/tinyspline.c>
//!   Copyright (c) 2016 Marcel Steinbeck
//!   Changed for Spare Kit: the double reflection step converted into Rust over arrays, with the
//!   step's derivative in the path parameter for the C1 rule between stored stations, the
//!   station placement rule and the closing twist; tinyspline's own frames at given knots, its
//!   first normal by the smallest tangent component and its error codes are not taken.
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/7a593124c98e2bdfc7adc4fa94aeeb9b758cf450/crates/solidcore/src/surface/procedural_sweep/path.rs>,
//!   for the rotation-minimising frame's closed form on lines and circular arcs, each span
//!   starting from the frame the last one ended with. No code copied.
//! SPDX-License-Identifier: Apache-2.0 AND MIT
//!
//! Cost: a placement is a few dozen operations, after a binary search of the stations and, for a
//! law on a path with no closed-form length, one quadrature. [`Stations::build`] costs a few
//! steps per station it places, at most [`MAX_STATIONS`]; the certified tests at most
//! [`MAX_SPANS`] derivative bounds of the path.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the method's: x, t and r the point, tangent and frame normal, u and h the correction's"
)]

use alloc::vec::Vec;
use core::fmt;

use geom_curve::{CurveError, CurveKind, HelixAxes, ParamCurve};
use geom_num::interval::Interval;
use geom_num::quadrature::{QuadError, integrate};
use geom_num::{NumError, Real, Sign, math};
use geom_nurbs::{Bounds, Limit};
use geom_run::{Stopped, WorkMeter};

use geom_curve::param::DerivativeBound;

/// The most stations [`Stations::build`] places.
pub const MAX_STATIONS: usize = 1 << 16;

/// The most spans the certified tests examine.
pub const MAX_SPANS: usize = 1 << 14;

/// The narrowest span the certified tests split, as a fraction of the domain: 2⁻²⁴.
const NARROWEST: f64 = 1.0 / 16_777_216.0;

/// A stations' first split of the domain, in quarters, before the rule refines it.
const QUARTERS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// A stored step's twist error over the difference between it and two half steps, to leading
/// order. The step's error is e·Δ⁵ + O(Δ⁶) for a step of length Δ (Wang et al.; on a circular
/// helix it is −κ²τ³Δ⁵/720 to leading order), so two half steps err 2·e·(Δ/2)⁵ = e·Δ⁵/16, the
/// difference is (15/16)·e·Δ⁵, and the step's own error is 16/15 of it (Richardson).
const RICHARDSON: f64 = 16.0 / 15.0;

/// What went wrong with a frame rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameError {
    /// The path refused.
    Curve(CurveError),
    /// The numbers layer refused.
    Num(NumError),
    /// The rule's frame does not exist on [from, to], or may not: zero curvature for a Frenet
    /// frame, a tangent along the reference for a fixed-reference frame (`FrameUndefined`, 13.3).
    FrameUndefined {
        /// The span's start.
        from: f64,
        /// The span's end.
        to: f64,
    },
    /// The path's tangent vanishes at `at`.
    ZeroTangent {
        /// The parameter.
        at: f64,
    },
    /// A rule or law that cannot be evaluated: what is wrong with it.
    InvalidRule(&'static str),
    /// A closed path's frame does not close, and the caller asked for no twist: the angle, in
    /// half-turns.
    ClosingTwist {
        /// The closing angle about the tangent.
        half_turns: f64,
    },
    /// A certified test ran out of its spans before deciding: the span it was on.
    Undecided {
        /// The span's start.
        from: f64,
        /// The span's end.
        to: f64,
    },
    /// The station rule needed more than [`MAX_STATIONS`] stations.
    TooManyStations,
    /// The arc-length quadrature did not converge.
    Quadrature,
    /// The work meter stopped.
    Stopped(Stopped),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::Curve(e) => write!(f, "the path refused: {e:?}"),
            FrameError::Num(e) => write!(f, "the numbers layer refused: {e:?}"),
            FrameError::FrameUndefined { from, to } => {
                write!(
                    f,
                    "the frame is undefined on the path between {from} and {to}"
                )
            }
            FrameError::ZeroTangent { at } => write!(f, "the path's tangent vanishes at {at}"),
            FrameError::InvalidRule(why) => write!(f, "the frame rule cannot be used: {why}"),
            FrameError::ClosingTwist { half_turns } => write!(
                f,
                "the closed path's frame does not close: it is off by {half_turns} half-turns"
            ),
            FrameError::Undecided { from, to } => write!(
                f,
                "the certified test could not decide the span from {from} to {to} within its budget"
            ),
            FrameError::TooManyStations => {
                f.write_str("the stations' rule needed too many stations")
            }
            FrameError::Quadrature => f.write_str("the arc length did not converge"),
            FrameError::Stopped(s) => write!(f, "stopped: {s:?}"),
        }
    }
}

impl From<CurveError> for FrameError {
    fn from(e: CurveError) -> Self {
        FrameError::Curve(e)
    }
}

impl From<NumError> for FrameError {
    fn from(e: NumError) -> Self {
        FrameError::Num(e)
    }
}

impl From<Stopped> for FrameError {
    fn from(e: Stopped) -> Self {
        FrameError::Stopped(e)
    }
}

/// A 3D vector.
type V = [f64; 3];

fn add(a: V, b: V) -> V {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    [a0 + b0, a1 + b1, a2 + b2]
}

fn sub(a: V, b: V) -> V {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    [a0 - b0, a1 - b1, a2 - b2]
}

fn times(k: f64, a: V) -> V {
    a.map(|x| k * x)
}

fn dot(a: V, b: V) -> f64 {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    a0 * b0 + a1 * b1 + a2 * b2
}

fn cross(a: V, b: V) -> V {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    [a1 * b2 - a2 * b1, a2 * b0 - a0 * b2, a0 * b1 - a1 * b0]
}

fn length(a: V) -> Result<f64, FrameError> {
    Ok(math::sqrt(dot(a, a))?.get())
}

/// `a` over its length; `None` for the zero vector.
fn unit(a: V) -> Result<Option<V>, FrameError> {
    let n = length(a)?;
    Ok((n > 0.0).then(|| times(1.0 / n, a)))
}

/// `a` turned about the unit `axis` by `half_turns` (Rodrigues' formula).
fn turned(a: V, axis: V, half_turns: f64) -> Result<V, FrameError> {
    let (c, s) = (
        math::cospi(half_turns)?.get(),
        math::sinpi(half_turns)?.get(),
    );
    Ok(add(
        add(times(c, a), times(s, cross(axis, a))),
        times(dot(axis, a) * (1.0 - c), axis),
    ))
}

/// Which axis of the profile a fixed reference steers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfileAxis {
    /// The profile's x axis points along the reference's part across the tangent: IFC's fixed
    /// reference.
    X,
    /// The profile's y axis does: an up-vector.
    Y,
}

/// A law: a function of arc length from the path's start (twist and scale), or of the turned
/// angle in half-turns (a helical rule's pitch and radius), stored exactly as given.
#[derive(Clone, Debug, PartialEq)]
pub enum Law {
    /// The same value everywhere.
    Constant(f64),
    /// `start + rate·s`.
    Linear {
        /// The value at 0.
        start: f64,
        /// Its change per unit.
        rate: f64,
    },
    /// Σ cₖ·sᵏ, the coefficients from the constant up.
    Polynomial(Vec<f64>),
}

impl Law {
    /// Refuses a law that is not finite, or a polynomial with no coefficients.
    fn check(&self) -> Result<(), FrameError> {
        let finite = match self {
            Law::Constant(c) => c.is_finite(),
            Law::Linear { start, rate } => start.is_finite() && rate.is_finite(),
            Law::Polynomial(c) => !c.is_empty() && c.iter().all(|x| x.is_finite()),
        };
        if finite {
            Ok(())
        } else {
            Err(FrameError::InvalidRule("a law that is not finite"))
        }
    }

    /// The value at `s`.
    #[must_use]
    pub fn value(&self, s: f64) -> f64 {
        match self {
            Law::Constant(c) => *c,
            Law::Linear { start, rate } => start + rate * s,
            Law::Polynomial(c) => c.iter().rev().fold(0.0, |acc, k| acc * s + k),
        }
    }

    /// The derivative at `s`.
    #[must_use]
    pub fn derivative(&self, s: f64) -> f64 {
        match self {
            Law::Constant(_) => 0.0,
            Law::Linear { rate, .. } => *rate,
            Law::Polynomial(c) => {
                let mut acc = 0.0;
                let mut k = c.len();
                for coefficient in c.iter().skip(1).rev() {
                    k = k.saturating_sub(1);
                    #[allow(
                        clippy::cast_precision_loss,
                        reason = "a power below the length of a stored list, far below 2^53"
                    )]
                    let power = k as f64;
                    acc = acc * s + power * coefficient;
                }
                acc
            }
        }
    }

    /// ∫₀ˢ of the law.
    #[must_use]
    pub fn integral(&self, s: f64) -> f64 {
        match self {
            Law::Constant(c) => c * s,
            Law::Linear { start, rate } => start * s + 0.5 * rate * s * s,
            Law::Polynomial(c) => {
                let mut acc = 0.0;
                let mut k = c.len();
                for coefficient in c.iter().rev() {
                    #[allow(
                        clippy::cast_precision_loss,
                        reason = "a power at most the length of a stored list, far below 2^53"
                    )]
                    let power = k as f64;
                    acc = acc * s + coefficient / power;
                    k = k.saturating_sub(1);
                }
                acc * s
            }
        }
    }
}

/// The twist and scale laws that act on top of any rule: functions of arc length from the path's
/// start, or under [`FrameRule::Helical`], which does not follow the path, of its turned angle in
/// half-turns. The twist turns the profile about its direction of travel by its value in
/// half-turns; the scale multiplies the profile's coordinates and must stay positive.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Laws {
    /// The twist, in half-turns about the tangent.
    pub twist: Option<Law>,
    /// The scale factor.
    pub scale: Option<Law>,
}

/// One stored station of a rotation-minimising frame: its parameter and its frame, bit for bit as
/// the double-reflection steps produced it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationFrame {
    /// The path parameter.
    pub at: f64,
    /// The path's unit tangent there.
    pub tangent: V,
    /// The frame's unit normal, across the tangent: the profile's x axis.
    pub normal: V,
}

impl StationFrame {
    /// The profile's y axis: tangent × normal.
    #[must_use]
    pub fn binormal(&self) -> V {
        cross(self.tangent, self.normal)
    }
}

/// What a closed path's rotation-minimising frame does with its closing angle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Closing {
    /// The path is open, or its closing angle is left as it is.
    Open,
    /// The closing angle is spread along the path at a constant twist rate, so the frame closes.
    Spread,
    /// The caller asked for no twist: a closing angle beyond the tolerance is
    /// [`FrameError::ClosingTwist`], and one within it, the method's rounding, is spread.
    Refuse,
}

/// The stations of a rotation-minimising frame (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Stations {
    frames: Vec<StationFrame>,
    /// The closing angle spread along the path, in half-turns (0 for an open path).
    closing: f64,
    /// The largest estimated twist error of one step the rule placed them by, in half-turns.
    measured: f64,
}

/// The side to read the path from at `t`: from the left at the domain's end, else the right.
fn side_at(path: &CurveKind<3>, t: f64) -> Limit {
    if t >= path.domain().1 {
        Limit::FromLeft
    } else {
        Limit::FromRight
    }
}

/// The path's point, unit tangent and their derivatives in the parameter at `t`.
#[derive(Clone, Copy, Debug)]
struct Jet {
    x: V,
    dx: V,
    t: V,
    dt: V,
}

fn jet(path: &CurveKind<3>, at: f64, side: Limit) -> Result<Jet, FrameError> {
    let mut d = [[0.0; 3]; 3];
    path.derivatives(at, side, &mut d)?;
    let [x, dx, ddx] = d;
    let speed = length(dx)?;
    if speed <= 0.0 {
        return Err(FrameError::ZeroTangent { at });
    }
    let t = times(1.0 / speed, dx);
    // t' = (x'' − (t·x'') t) / |x'|.
    let dt = times(1.0 / speed, sub(ddx, times(dot(t, ddx), t)));
    Ok(Jet { x, dx, t, dt })
}

/// One double-reflection step from `from` (at `x0`) to the path's point and tangent `to`: the
/// frame's normal there, and its derivative in the path parameter.
fn step(x0: V, from: &StationFrame, to: &Jet) -> (V, V) {
    let v1 = sub(to.x, x0);
    let c1 = dot(v1, v1);
    if c1 <= 0.0 {
        return (from.normal, [0.0; 3]);
    }
    let dv1 = to.dx;
    let dc1 = 2.0 * dot(v1, dv1);
    // r̂ᴸ = r̂ − k v₁ with k = 2 v₁·r̂ / c₁.
    let a = dot(v1, from.normal);
    let da = dot(dv1, from.normal);
    let k = 2.0 * a / c1;
    let dk = 2.0 * (da * c1 - a * dc1) / (c1 * c1);
    let rl = sub(from.normal, times(k, v1));
    let drl = times(-1.0, add(times(dk, v1), times(k, dv1)));
    // t̂ᴸ = t̂ − m v₁ with m = 2 v₁·t̂ / c₁.
    let b = dot(v1, from.tangent);
    let db = dot(dv1, from.tangent);
    let m = 2.0 * b / c1;
    let dm = 2.0 * (db * c1 - b * dc1) / (c1 * c1);
    let tl = sub(from.tangent, times(m, v1));
    let dtl = times(-1.0, add(times(dm, v1), times(m, dv1)));
    let v2 = sub(to.t, tl);
    let dv2 = sub(to.dt, dtl);
    let c2 = dot(v2, v2);
    if c2 <= 0.0 {
        return (rl, drl);
    }
    let dc2 = 2.0 * dot(v2, dv2);
    let e = dot(v2, rl);
    let de = dot(dv2, rl) + dot(v2, drl);
    let n = 2.0 * e / c2;
    let dn = 2.0 * (de * c2 - e * dc2) / (c2 * c2);
    let r = sub(rl, times(n, v2));
    let dr = sub(drl, add(times(dn, v2), times(n, dv2)));
    (r, dr)
}

/// The spin about the tangent of a frame with normal `r` and derivative `dr`, per unit of the
/// path parameter, in radians: dr̂·(t̂ × r̂).
fn spin(t: V, r: V, dr: V) -> f64 {
    dot(dr, cross(t, r))
}

/// The angle from `a` to `b` about the unit `axis`, both across it, in half-turns.
fn angle_about(axis: V, a: V, b: V) -> Result<f64, FrameError> {
    let y = dot(cross(a, b), axis);
    let x = dot(a, b);
    Ok(math::atan2(y, x)?.get() / core::f64::consts::PI)
}

impl Stations {
    /// The stations of `path`'s rotation-minimising frame from `start_normal` (any vector across
    /// the start tangent; its part across it is taken), placed until each step's estimated twist
    /// error times `radius` is within `tolerance`, and closed as `closing` says.
    ///
    /// The rule: the domain in four equal spans, then from the start, span by span,
    /// one step against two half steps; a span whose step's estimated error, 16/15 of the two
    /// answers' difference (module documentation), is more than the tolerance allows is halved,
    /// its first half taken next. So the stations depend on the path, the start, the tolerance
    /// and nothing else.
    ///
    /// # Errors
    ///
    /// [`FrameError::InvalidRule`] for a start along the tangent, a radius or tolerance that is not
    /// positive and finite, or [`Closing::Spread`] or [`Closing::Refuse`] on a path whose end is
    /// not its start, or whose tangent there is not its start tangent, within the tolerance;
    /// [`FrameError::ZeroTangent`]; [`FrameError::TooManyStations`]; [`FrameError::ClosingTwist`]
    /// under [`Closing::Refuse`]; the path's and the meter's errors.
    pub fn build(
        path: &CurveKind<3>,
        start_normal: V,
        radius: f64,
        tolerance: f64,
        closing: Closing,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Stations, FrameError> {
        if !(radius > 0.0 && radius.is_finite() && tolerance > 0.0 && tolerance.is_finite()) {
            return Err(FrameError::InvalidRule(
                "a radius or tolerance that is not positive",
            ));
        }
        let (a, b) = path.domain();
        if !(a.is_finite() && b.is_finite() && a < b) {
            return Err(FrameError::InvalidRule("a path with no finite domain"));
        }
        let first = jet(path, a, Limit::FromRight)?;
        let across = sub(start_normal, times(dot(start_normal, first.t), first.t));
        let normal =
            unit(across)?.ok_or(FrameError::InvalidRule("a start normal along the tangent"))?;
        let limit = tolerance / radius / core::f64::consts::PI;
        if closing != Closing::Open {
            // A closing angle means something only on a closed path: its end at its start, with
            // the same tangent there, within the tolerance (a closed sweep's path is G1, 13.3).
            let last = jet(path, b, Limit::FromLeft)?;
            let gap = length(sub(last.x, first.x))?;
            let turn = math::atan2(length(cross(first.t, last.t))?, dot(first.t, last.t))?.get()
                / core::f64::consts::PI;
            if !(gap <= tolerance && turn <= limit) {
                return Err(FrameError::InvalidRule(
                    "a closing rule on a path that is not closed",
                ));
            }
        }
        let mut frames = Vec::new();
        frames.push(StationFrame {
            at: a,
            tangent: first.t,
            normal,
        });
        let mut measured: f64 = 0.0;
        // The spans still to place, the next one last: the domain's quarters, its ends exact.
        let at = |q: f64| {
            if q <= 0.0 {
                a
            } else if q >= 1.0 {
                b
            } else {
                a + (b - a) * q
            }
        };
        let mut todo: Vec<(f64, f64)> = Vec::with_capacity(QUARTERS.len());
        for w in QUARTERS.windows(2).rev() {
            if let [lo, hi] = w {
                todo.push((at(*lo), at(*hi)));
            }
        }
        while let Some((from, to)) = todo.pop() {
            meter.charge(1)?;
            let last = *frames.last().ok_or(FrameError::InvalidRule("no station"))?;
            let x0 = path.point(last.at, side_at(path, last.at))?;
            let end = jet(path, to, Limit::FromLeft)?;
            let (one, _) = step(x0, &last, &end);
            let mid = 0.5 * from + 0.5 * to;
            let middle = jet(path, mid, Limit::FromRight)?;
            let (half, _) = step(x0, &last, &middle);
            let halfway = StationFrame {
                at: mid,
                tangent: middle.t,
                normal: unit(half)?.ok_or(FrameError::InvalidRule("a degenerate step"))?,
            };
            let (two, _) = step(middle.x, &halfway, &end);
            let estimate = RICHARDSON * angle_about(end.t, one, two)?.abs();
            let narrow = (to - from) <= (b - a) * NARROWEST;
            if estimate <= limit || narrow {
                measured = Real::new(measured)?.total_max(Real::new(estimate)?).get();
                let normal = unit(one)?.ok_or(FrameError::InvalidRule("a degenerate step"))?;
                if frames.len() >= MAX_STATIONS {
                    return Err(FrameError::TooManyStations);
                }
                frames.push(StationFrame {
                    at: to,
                    tangent: end.t,
                    normal,
                });
            } else {
                todo.push((mid, to));
                todo.push((from, mid));
            }
        }
        let mut stations = Stations {
            frames,
            closing: 0.0,
            measured,
        };
        if closing != Closing::Open {
            let (head, tail) = (stations.frames.first(), stations.frames.last());
            if let (Some(head), Some(tail)) = (head, tail) {
                let angle = angle_about(tail.tangent, head.normal, tail.normal)?;
                // Under Refuse a closing angle within the tolerance is the method's own rounding,
                // and is spread like any other so the frame closes exactly.
                if closing == Closing::Refuse && angle.abs() > limit {
                    return Err(FrameError::ClosingTwist { half_turns: angle });
                }
                stations.closing = angle;
            }
        }
        Ok(stations)
    }

    /// Stations from stored frames, as a file reads them back: each frame as it was produced,
    /// bit for bit, and the closing angle spread along the path. The frames are the definition,
    /// so they give the same frame everywhere as the stations they were stored from; the
    /// estimate [`Stations::measured_twist`] is not part of it, and reads 0.
    ///
    /// # Errors
    ///
    /// [`FrameError::InvalidRule`] for no frames, parameters that do not increase, or entries
    /// that are not finite.
    pub fn from_frames(frames: Vec<StationFrame>, closing: f64) -> Result<Stations, FrameError> {
        let finite = |v: V| v.iter().all(|x| x.is_finite());
        if frames.is_empty()
            || !closing.is_finite()
            || !frames
                .iter()
                .all(|f| f.at.is_finite() && finite(f.tangent) && finite(f.normal))
            || !frames
                .windows(2)
                .all(|w| matches!(w, [p, q] if p.at < q.at))
        {
            return Err(FrameError::InvalidRule(
                "station frames that are not stored stations",
            ));
        }
        Ok(Stations {
            frames,
            closing,
            measured: 0.0,
        })
    }

    /// The stored frames.
    #[must_use]
    pub fn frames(&self) -> &[StationFrame] {
        &self.frames
    }

    /// The closing angle spread along the path, in half-turns.
    #[must_use]
    pub const fn closing(&self) -> f64 {
        self.closing
    }

    /// The largest estimated twist error of one step the rule placed them by, in half-turns: 16/15
    /// of the difference between one step and two half steps, the method's own estimate of the
    /// stored step's error to leading order (module documentation). The steps' errors add, so
    /// with n stations the frame is within about (n − 1) times this of the mathematical frame
    /// everywhere. Measured, not proven (13.3).
    #[must_use]
    pub const fn measured_twist(&self) -> f64 {
        self.measured
    }

    /// The frame's normal at `t` (module documentation: the step from the station before `t`,
    /// turned by the C1 correction, then by the spread closing angle).
    fn normal_at(&self, path: &CurveKind<3>, t: f64, here: &Jet) -> Result<V, FrameError> {
        let at = self.frames.partition_point(|f| f.at <= t);
        let i = at.saturating_sub(1);
        let from = *self
            .frames
            .get(i)
            .ok_or(FrameError::InvalidRule("no station"))?;
        let x0 = path.point(from.at, side_at(path, from.at))?;
        let mut r = if t <= from.at {
            from.normal
        } else if let Some(next) = self.frames.get(at) {
            let (step_here, _) = step(x0, &from, here);
            // The step's spin at the next station, which the correction cancels.
            let end = jet(path, next.at, Limit::FromLeft)?;
            let (r_end, dr_end) = step(x0, &from, &end);
            let r_end = unit(r_end)?.unwrap_or(next.normal);
            let omega = spin(end.t, r_end, dr_end);
            let width = next.at - from.at;
            let u = (t - from.at) / width;
            let h = -omega * width * (u * u * u - u * u) / core::f64::consts::PI;
            let r = unit(step_here)?.ok_or(FrameError::InvalidRule("a degenerate step"))?;
            turned(r, here.t, h)?
        } else {
            // At or past the last station.
            let (r, _) = step(x0, &from, here);
            unit(r)?.ok_or(FrameError::InvalidRule("a degenerate step"))?
        };
        if self.closing != 0.0 {
            let (a, b) = path.domain();
            let share = -self.closing * ((t - a) / (b - a));
            r = turned(r, here.t, share)?;
        }
        Ok(r)
    }
}

/// An explicit frame at a path parameter: IFC's placement along a directrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationPlacement {
    /// The path parameter.
    pub at: f64,
    /// The profile's x axis there.
    pub x: V,
    /// The profile's normal there, the direction of travel.
    pub z: V,
}

/// How a profile turns along its path (module documentation; 13.3).
#[derive(Clone, Debug, PartialEq)]
pub enum FrameRule {
    /// The profile never turns.
    Translate,
    /// The least twist: double reflection from stored stations.
    RotationMinimising(Stations),
    /// The curve's own frame: principal normal and binormal.
    Frenet,
    /// IFC's fixed reference ([`ProfileAxis::X`]) or an up-vector ([`ProfileAxis::Y`]).
    FixedReference {
        /// The fixed direction.
        reference: V,
        /// Which profile axis it steers.
        axis: ProfileAxis,
    },
    /// Frames given at parameters, in increasing order.
    Placements(Vec<StationPlacement>),
    /// A screw motion about an axis: the profile turns about it and rises, the parameter the
    /// turned angle in half-turns.
    Helical {
        /// A point of the axis.
        point: V,
        /// The axis's direction.
        axis: V,
        /// The radial direction at angle 0, across the axis.
        reference: V,
        /// The rise per full turn, a law of the angle.
        pitch: Law,
        /// The profile's distance from the axis, a law of the angle.
        radius: Law,
    },
}

/// Where a profile is at one parameter: origin + scale·(p·x̂ + q·ŷ) for a profile point (p, q),
/// with ẑ = x̂ × ŷ the direction of travel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The origin.
    pub origin: V,
    /// The profile's x axis.
    pub x: V,
    /// The profile's y axis.
    pub y: V,
    /// The direction of travel.
    pub z: V,
    /// The scale.
    pub scale: f64,
}

impl Placement {
    /// Where the profile point (p, q) lands.
    #[must_use]
    pub fn place(&self, p: [f64; 2]) -> V {
        let [px, py] = p;
        add(
            self.origin,
            times(self.scale, add(times(px, self.x), times(py, self.y))),
        )
    }
}

/// Whether the path is a kind whose speed is constant, with that speed: a line, a circular arc or
/// a circular helix.
fn constant_speed(path: &CurveKind<3>) -> Option<f64> {
    match path {
        CurveKind::Line(l) => length(l.direction()).ok(),
        CurveKind::Circle(c) => Some(core::f64::consts::PI * c.radius()),
        CurveKind::Helix(h) => match h.axes() {
            HelixAxes::Frame { radius, pitch, .. } => {
                let rise = 0.5 * pitch;
                let round = core::f64::consts::PI * radius;
                math::sqrt(round * round + rise * rise).ok().map(Real::get)
            }
            HelixAxes::Vectors { .. } => None,
        },
        _ => None,
    }
}

/// The arc length from the path's start to `t`: exact in closed form on lines, arcs and helices,
/// and by deterministic Gauss-Kronrod quadrature of |C'| elsewhere.
///
/// # Errors
///
/// [`FrameError::Quadrature`] if the quadrature does not converge; the path's errors.
pub fn arc_length(path: &CurveKind<3>, t: f64) -> Result<f64, FrameError> {
    let (a, _) = path.domain();
    if t <= a {
        return Ok(0.0);
    }
    if let Some(speed) = constant_speed(path) {
        return Ok(speed * (t - a));
    }
    let speed = |u: Real| -> Result<Real, NumError> {
        let mut d = [[0.0; 3]; 2];
        path.derivatives(u.get(), Limit::FromRight, &mut d)
            .map_err(|_| NumError::OutsideDomain)?;
        let [_, dx] = d;
        math::sqrt(dot(dx, dx))
    };
    let tol = Real::new(1e-13 * (t - a))?;
    let found = integrate(
        speed,
        Real::new(a)?,
        Real::new(t)?,
        tol,
        Real::new(1e-14)?,
        2,
        64,
    );
    match found {
        Ok(i) => Ok(i.value.get()),
        Err(QuadError::Num(e)) => Err(FrameError::Num(e)),
        Err(_) => Err(FrameError::Quadrature),
    }
}

/// The interval box of a bound.
fn boxed(b: &Bounds<3>) -> Result<[Interval; 3], FrameError> {
    let ([l0, l1, l2], [h0, h1, h2]) = (b.lo, b.hi);
    Ok([
        Interval::new(l0, h0)?,
        Interval::new(l1, h1)?,
        Interval::new(l2, h2)?,
    ])
}

fn interval_cross(a: [Interval; 3], b: [Interval; 3]) -> Result<[Interval; 3], FrameError> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    Ok([
        a1.mul(b2)?.sub(a2.mul(b1)?)?,
        a2.mul(b0)?.sub(a0.mul(b2)?)?,
        a0.mul(b1)?.sub(a1.mul(b0)?)?,
    ])
}

/// Whether some component of `v` is certainly not zero.
fn certainly_nonzero(v: [Interval; 3]) -> bool {
    v.iter()
        .any(|c| matches!(c.certain_sign(), Some(Sign::Positive | Sign::Negative)))
}

/// Whether every component of `v` is exactly zero.
fn certainly_zero(v: [Interval; 3]) -> bool {
    v.iter().all(|c| c.certain_sign() == Some(Sign::Zero))
}

/// Splits the path's domain until `decided` passes every span, or a span narrower than 2⁻²⁴ of
/// the domain, or one where the test is certainly zero, is reported.
fn certify(
    path: &CurveKind<3>,
    meter: &mut WorkMeter<'_>,
    mut cross_of: impl FnMut(
        &CurveKind<3>,
        (f64, f64),
        &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 3]>, FrameError>,
) -> Result<(), FrameError> {
    let (a, b) = path.domain();
    let mut todo = alloc::vec![(a, b)];
    let mut spent = 0usize;
    while let Some((from, to)) = todo.pop() {
        meter.charge(1)?;
        spent = spent.saturating_add(1);
        if spent > MAX_SPANS {
            return Err(FrameError::Undecided { from, to });
        }
        match cross_of(path, (from, to), meter)? {
            Some(v) if certainly_nonzero(v) => {}
            Some(v) if certainly_zero(v) => return Err(FrameError::FrameUndefined { from, to }),
            _ if (to - from) <= (b - a) * NARROWEST => {
                return Err(FrameError::FrameUndefined { from, to });
            }
            _ => {
                let mid = 0.5 * from + 0.5 * to;
                todo.push((mid, to));
                todo.push((from, mid));
            }
        }
    }
    Ok(())
}

/// Whether the path's Frenet frame exists everywhere: C' × C'' is certainly nonzero on every span
/// of the domain, from the path's certified derivative bounds.
///
/// # Errors
///
/// [`FrameError::FrameUndefined`] with a span holding every parameter where the curvature may
/// vanish (a line, an inflection), at most 2⁻²⁴ of the domain wide unless the curvature is
/// certainly zero there (the second derivative's bound exactly zero over the span, as on a line,
/// whose whole domain is reported); [`FrameError::Undecided`] past [`MAX_SPANS`]; the path's and
/// the meter's errors.
pub fn frenet_defined(path: &CurveKind<3>, meter: &mut WorkMeter<'_>) -> Result<(), FrameError> {
    certify(path, meter, |p, span, m| {
        let (DerivativeBound::Bounded(d1), DerivativeBound::Bounded(d2)) = (
            p.derivative_bound(span, 1, m)?,
            p.derivative_bound(span, 2, m)?,
        ) else {
            return Ok(None);
        };
        let d2 = boxed(&d2)?;
        if certainly_zero(d2) {
            // C'' is exactly zero over the span (a line), so C' × C'' is too: the curvature
            // vanishes on all of it. The interval product, widened by a step, could never show it.
            return Ok(Some(d2));
        }
        Ok(Some(interval_cross(boxed(&d1)?, d2)?))
    })
}

/// Whether a fixed reference is off the path's tangent everywhere: C' × reference is certainly
/// nonzero on every span.
///
/// # Errors
///
/// As [`frenet_defined`], with the span where the tangent may turn along the reference.
pub fn reference_defined(
    path: &CurveKind<3>,
    reference: V,
    meter: &mut WorkMeter<'_>,
) -> Result<(), FrameError> {
    let [r0, r1, r2] = reference;
    let r = [
        Interval::point(r0)?,
        Interval::point(r1)?,
        Interval::point(r2)?,
    ];
    certify(path, meter, |p, span, m| {
        let DerivativeBound::Bounded(d1) = p.derivative_bound(span, 1, m)? else {
            return Ok(None);
        };
        Ok(Some(interval_cross(boxed(&d1)?, r)?))
    })
}

/// The exact rotation-minimising frame's normal at `t` on a line, a circular arc or a circular
/// helix, from the normal `start` at the domain's start (its part across the start tangent);
/// `None` for any other kind.
///
/// # Errors
///
/// [`FrameError::InvalidRule`] for a start along the tangent; the path's errors.
pub fn closed_form_frame(path: &CurveKind<3>, start: V, t: f64) -> Result<Option<V>, FrameError> {
    let (a, _) = path.domain();
    let first = jet(path, a, Limit::FromRight)?;
    let across = sub(start, times(dot(start, first.t), first.t));
    let r0 = unit(across)?.ok_or(FrameError::InvalidRule("a start normal along the tangent"))?;
    match path {
        CurveKind::Line(_) => Ok(Some(r0)),
        CurveKind::Circle(c) => {
            let (axes, _) = c.frame().unit_vectors()?;
            let [_, _, n] = axes;
            Ok(Some(turned(r0, n, t - a)?))
        }
        CurveKind::Helix(h) => {
            let HelixAxes::Frame {
                frame,
                radius,
                pitch,
            } = h.axes()
            else {
                return Ok(None);
            };
            let ([x, y, n], _) = frame.unit_vectors()?;
            // θ = πt; per radian the helix rises h = pitch/(2π) and its speed is L = √(r² + h²).
            let rise = pitch / (2.0 * core::f64::consts::PI);
            let speed = math::sqrt(radius * radius + rise * rise)?.get();
            let frenet = |u: f64| -> Result<(V, V, V), FrameError> {
                let (c, s) = (math::cospi(u)?.get(), math::sinpi(u)?.get());
                let tangent = times(
                    1.0 / speed,
                    add(
                        add(times(-radius * s, x), times(radius * c, y)),
                        times(rise, n),
                    ),
                );
                let normal = times(-1.0, add(times(c, x), times(s, y)));
                let binormal = cross(tangent, normal);
                Ok((tangent, normal, binormal))
            };
            let (_, n0, b0) = frenet(a)?;
            let phi0 = math::atan2(dot(r0, b0), dot(r0, n0))?.get() / core::f64::consts::PI;
            // φ(t) = φ₀ − (h/L)·(t − a), in half-turns.
            let phi = phi0 - (rise / speed) * (t - a);
            let (_, nt, bt) = frenet(t)?;
            let (c, s) = (math::cospi(phi)?.get(), math::sinpi(phi)?.get());
            Ok(Some(add(times(c, nt), times(s, bt))))
        }
        _ => Ok(None),
    }
}

/// The relative turn from frame (x₀, z₀) to (x₁, z₁), shared out: frame (x₀, z₀) turned by the
/// fraction `share` of the turn about its axis.
fn share_turn(x0: V, z0: V, x1: V, z1: V, share: f64) -> Result<(V, V), FrameError> {
    let y0 = cross(z0, x0);
    let y1 = cross(z1, x1);
    // The rotation R = F₁·F₀ᵀ as a matrix with columns R·e_k.
    let r = |v: V| {
        add(
            add(times(dot(v, x0), x1), times(dot(v, y0), y1)),
            times(dot(v, z0), z1),
        )
    };
    let [c0, c1, c2] = [r([1.0, 0.0, 0.0]), r([0.0, 1.0, 0.0]), r([0.0, 0.0, 1.0])];
    let ([m00, m10, m20], [m01, m11, m21], [m02, m12, m22]) = (c0, c1, c2);
    let trace = m00 + m11 + m22;
    // R's quaternion (w, q) by Shepperd's method. Of 4w² = 1 + trace and 4qₖ² = 1 + 2mₖₖ − trace,
    // which sum to 4, the largest gives its component by a square root (at least 1/2), and the
    // other three come from R's symmetric and antisymmetric parts divided by it, so the axis
    // stays accurate up to a half turn.
    let quarter = |square: f64| -> Result<(f64, f64), FrameError> {
        let big = 0.5 * math::sqrt(square)?.get();
        Ok((big, 0.25 / big))
    };
    let (w, q) = if trace >= m00 && trace >= m11 && trace >= m22 {
        let (w, f) = quarter(1.0 + trace)?;
        (w, [(m21 - m12) * f, (m02 - m20) * f, (m10 - m01) * f])
    } else if m00 >= m11 && m00 >= m22 {
        let (x, f) = quarter(1.0 + m00 - m11 - m22)?;
        ((m21 - m12) * f, [x, (m01 + m10) * f, (m02 + m20) * f])
    } else if m11 >= m22 {
        let (y, f) = quarter(1.0 - m00 + m11 - m22)?;
        ((m02 - m20) * f, [(m01 + m10) * f, y, (m12 + m21) * f])
    } else {
        let (z, f) = quarter(1.0 - m00 - m11 + m22)?;
        ((m10 - m01) * f, [(m02 + m20) * f, (m12 + m21) * f, z])
    };
    // (w, q) and (−w, −q) are the same turn; with w > 0 it is the shorter way round, by
    // θ = 2·atan2(|q|, w) < π. With w = 0 it is a half turn, which has no shorter way.
    if w == 0.0 {
        return Err(FrameError::InvalidRule("placements a half turn apart"));
    }
    let (w, q) = if w < 0.0 {
        (-w, times(-1.0, q))
    } else {
        (w, q)
    };
    let Some(k) = unit(q)? else {
        return Ok((x0, z0));
    };
    let angle = 2.0 * math::atan2(length(q)?, w)?.get() / core::f64::consts::PI;
    let part = angle * share;
    let turn = |v: V| -> Result<V, FrameError> {
        // Rodrigues: v cos + (k × v) sin + k (k·v)(1 − cos).
        let (c, s) = (math::cospi(part)?.get(), math::sinpi(part)?.get());
        Ok(add(
            add(times(c, v), times(s, cross(k, v))),
            times(dot(k, v) * (1.0 - c), k),
        ))
    };
    Ok((turn(x0)?, turn(z0)?))
}

/// Where the profile is at `t` under `rule` along `path`, with `laws` on top of it.
///
/// # Errors
///
/// [`FrameError::FrameUndefined`] where the rule's frame does not exist at `t`;
/// [`FrameError::ZeroTangent`]; [`FrameError::InvalidRule`] for a rule or law that cannot be
/// evaluated or a scale that is not positive; the path's errors.
pub fn placement(
    rule: &FrameRule,
    laws: &Laws,
    path: &CurveKind<3>,
    t: f64,
) -> Result<Placement, FrameError> {
    let side = side_at(path, t);
    let (origin, x, z) = match rule {
        FrameRule::Helical {
            point,
            axis,
            reference,
            pitch,
            radius,
        } => {
            pitch.check()?;
            radius.check()?;
            let n = unit(*axis)?.ok_or(FrameError::InvalidRule("a zero helical axis"))?;
            let u = unit(sub(*reference, times(dot(*reference, n), n)))?.ok_or(
                FrameError::InvalidRule("a helical reference along its axis"),
            )?;
            let v = cross(n, u);
            let (c, s) = (math::cospi(t)?.get(), math::sinpi(t)?.get());
            let radial = add(times(c, u), times(s, v));
            let around = add(times(-s, u), times(c, v));
            // The rise is ∫ pitch/2 over the half-turns turned.
            let rise = 0.5 * pitch.integral(t);
            let origin = add(add(*point, times(rise, n)), times(radius.value(t), radial));
            // x̂ along the axis and ŷ radial span the profile's plane through the axis; then
            // x̂ × ŷ = n̂ × radial is the direction the angle turns.
            return finish(origin, n, radial, around, laws, path, t, false);
        }
        FrameRule::Translate => {
            let here = jet(path, t, side)?;
            (here.x, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
        }
        FrameRule::RotationMinimising(stations) => {
            let here = jet(path, t, side)?;
            (here.x, stations.normal_at(path, t, &here)?, here.t)
        }
        FrameRule::Frenet => {
            let mut d = [[0.0; 3]; 3];
            path.derivatives(t, side, &mut d)?;
            let [p, d1, d2] = d;
            let tangent = unit(d1)?.ok_or(FrameError::ZeroTangent { at: t })?;
            let b = unit(cross(d1, d2))?.ok_or(FrameError::FrameUndefined { from: t, to: t })?;
            (p, cross(b, tangent), tangent)
        }
        FrameRule::FixedReference { reference, axis } => {
            let here = jet(path, t, side)?;
            let across = sub(*reference, times(dot(*reference, here.t), here.t));
            let steered = unit(across)?.ok_or(FrameError::FrameUndefined { from: t, to: t })?;
            let x = match axis {
                ProfileAxis::X => steered,
                // ŷ = steered, so x̂ = ŷ × ẑ.
                ProfileAxis::Y => cross(steered, here.t),
            };
            (here.x, x, here.t)
        }
        FrameRule::Placements(list) => {
            let here = path.point(t, side)?;
            let at = list.partition_point(|p| p.at <= t);
            let first = list.get(at.saturating_sub(1));
            let (x, z) = match (first, list.get(at)) {
                (Some(p), Some(q)) if t > p.at => {
                    let share = (t - p.at) / (q.at - p.at);
                    let (px, pz) = orthonormal(p.x, p.z)?;
                    let (qx, qz) = orthonormal(q.x, q.z)?;
                    share_turn(px, pz, qx, qz, share)?
                }
                (Some(p), _) => orthonormal(p.x, p.z)?,
                (None, Some(q)) => orthonormal(q.x, q.z)?,
                (None, None) => return Err(FrameError::InvalidRule("no placements")),
            };
            (here, x, z)
        }
    };
    let y = cross(z, x);
    finish(origin, x, y, z, laws, path, t, true)
}

/// A placement's x and z made orthonormal: z over its length, x its part across z.
fn orthonormal(x: V, z: V) -> Result<(V, V), FrameError> {
    let z = unit(z)?.ok_or(FrameError::InvalidRule("a placement with a zero axis"))?;
    let x = unit(sub(x, times(dot(x, z), z)))?.ok_or(FrameError::InvalidRule(
        "a placement whose x is along its axis",
    ))?;
    Ok((x, z))
}

/// The placement with the laws applied: the twist turns x̂ and ŷ about ẑ, the scale multiplies.
#[allow(
    clippy::too_many_arguments,
    reason = "the placement's three axes, its origin and what the laws need"
)]
fn finish(
    origin: V,
    x: V,
    y: V,
    z: V,
    laws: &Laws,
    path: &CurveKind<3>,
    t: f64,
    along_path: bool,
) -> Result<Placement, FrameError> {
    let s = if laws.twist.is_some() || laws.scale.is_some() {
        if along_path { arc_length(path, t)? } else { t }
    } else {
        0.0
    };
    let (x, y) = match &laws.twist {
        Some(law) => {
            law.check()?;
            let angle = law.value(s);
            (turned(x, z, angle)?, turned(y, z, angle)?)
        }
        None => (x, y),
    };
    let scale = match &laws.scale {
        Some(law) => {
            law.check()?;
            law.value(s)
        }
        None => 1.0,
    };
    if !(scale > 0.0 && scale.is_finite()) {
        return Err(FrameError::InvalidRule("a scale that is not positive"));
    }
    Ok(Placement {
        origin,
        x,
        y,
        z,
        scale,
    })
}
