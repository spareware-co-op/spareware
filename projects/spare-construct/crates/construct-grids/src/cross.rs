//! Where grid lines cross, and the point of a grid line nearest to a point.
//!
//! **Crossings.** [`crossings`] gives every point where two shapes meet, each one on both of them
//! (an end counts), in order of x then y: none if the two never meet (apart, or parallel, which
//! includes lying along each other, and concentric circles), one, or two (a line through a
//! circle, or two circles). Each [`Crossing`] says whether its point is exact, and its bound:
//! how far the true crossing can be from the point given, in steps, in each direction.
//!
//! - Two straight lines: whole-number arithmetic in 128 bits. The crossing is a rational point;
//!   when it falls on a step of the kernel's grid it is exact (`exact`, bound 0), and otherwise
//!   the nearest step with a bound of 1 step (the true point is within half a step in each
//!   direction).
//! - A line and an arc, or two arcs: the nearest step, worked out in `f64` from exact whole
//!   numbers (the cross and dot products are whole-number arithmetic; only the last square root
//!   and the division are floating), through `libm` so the bits are the same everywhere. The
//!   floating error is under 1/100 step before the geometry magnifies it: where the two curves
//!   meet at an angle `t` (the sine of it is worked out), the bound is `1 + ceil(0.01 / sin t)`
//!   steps. So for curves that meet at 1 degree or more it is at most 2 steps; for curves that
//!   graze, the bound says how little the point can be trusted. A grazing contact within about
//!   1/1000 step of touching may be taken as touching or as missing. A crossing within 1/100 step
//!   of an end of a segment or an arc counts as on it.
//!
//! **Nearest.** [`nearest_on`] gives the point of a shape nearest to a point and its distance:
//! the foot of the perpendicular if that lies on the shape, otherwise the nearer end. A line's foot
//! is the nearest step to the true foot (exact arithmetic, a half rounding up); an arc's is worked out in `f64`. The
//! distance is from the point to that foot, the nearest step, so what is compared is whole numbers.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are those of the formulas in the module's header: a, b, d, e, h, p, r, s, t, u, w"
)]

use crate::shape::{
    End, Point, Shape, cross, dot, float, length, nearest_step, radians, round_div, round_sqrt,
    sweep_units, wide,
};
use construct_params::{Angle, Length};

/// The tolerance at the ends of a segment or an arc, in steps.
const END_TOLERANCE: f64 = 0.01;

/// A point where two shapes meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crossing {
    /// The crossing, the nearest step to the true point.
    pub point: Point,
    /// Whether `point` is the true crossing, to the last step.
    pub exact: bool,
    /// How far the true crossing can be from `point` in each direction: zero when exact.
    pub bound: Length,
}

/// The bound, in steps, of a crossing at an angle whose sine is `sin`: `1 + ceil(0.01 / sin)`.
fn bound_for(sin: f64) -> Length {
    let steps = if sin > 0.0 {
        nearest_step((0.01 / sin).ceil() + 1.0).unwrap_or(i64::MAX)
    } else {
        i64::MAX
    };
    Length::from_steps(steps)
}

fn vector(from: Point, to: Point) -> (i128, i128) {
    (
        wide(to.x).saturating_sub(wide(from.x)),
        wide(to.y).saturating_sub(wide(from.y)),
    )
}

/// Where two straight segments meet: whole numbers all the way.
fn line_line(a: (Point, Point), b: (Point, Point)) -> Option<Crossing> {
    let d = vector(a.0, a.1);
    let e = vector(b.0, b.1);
    let mut denom = cross(d, e)?;
    if denom == 0 {
        return None;
    }
    let w = vector(a.0, b.0);
    // a.0 + t d = b.0 + u e, so t = (w x e) / (d x e) and u = (w x d) / (d x e).
    let mut t = cross(w, e)?;
    let mut u = cross(w, d)?;
    if denom < 0 {
        denom = denom.checked_neg()?;
        t = t.checked_neg()?;
        u = u.checked_neg()?;
    }
    if !(0..=denom).contains(&t) || !(0..=denom).contains(&u) {
        return None;
    }
    let along = |from: Length, step: i128| -> Option<(Length, bool)> {
        let moved = step.checked_mul(t)?;
        let whole = moved.checked_rem(denom)? == 0;
        let at = wide(from).checked_add(round_div(moved, denom)?)?;
        Some((length(at)?, whole))
    };
    let (x, exact_x) = along(a.0.x, d.0)?;
    let (y, exact_y) = along(a.0.y, d.1)?;
    let exact = exact_x && exact_y;
    Some(Crossing {
        point: Point::new(x, y),
        exact,
        bound: Length::from_steps(i64::from(!exact)),
    })
}

/// Whether the angle `theta` (radians, any) is on the arc from `from` to `to`, to `slack` radians.
fn on_arc(theta: f64, from: Angle, to: Angle, slack: f64) -> bool {
    let turn = core::f64::consts::TAU;
    let sweep = radians(sweep_units(from, to));
    let relative = (theta - radians(from.units())).rem_euclid(turn);
    relative <= sweep + slack || relative >= turn - slack
}

/// The crossing at an `f64` point, if it is on the arc (`centre`, `radius`, `from`, `to`).
fn on_this_arc(at: (f64, f64), arc: (Point, Length, Angle, Angle), sin: f64) -> Option<Crossing> {
    let (centre, radius, from, to) = arc;
    let theta = libm::atan2(at.1 - float(wide(centre.y)), at.0 - float(wide(centre.x)));
    let slack = END_TOLERANCE / float(wide(radius));
    on_arc(theta, from, to, slack).then(|| Crossing {
        point: Point::new(
            Length::from_steps(nearest_step(at.0).unwrap_or(0)),
            Length::from_steps(nearest_step(at.1).unwrap_or(0)),
        ),
        exact: false,
        bound: bound_for(sin),
    })
}

/// Where a straight segment meets an arc.
fn line_arc(line: (Point, Point), arc: (Point, Length, Angle, Angle)) -> Vec<Crossing> {
    let (centre, radius, _, _) = arc;
    let d = vector(line.0, line.1);
    let p = vector(centre, line.0);
    let (Some(len2), Some(side), Some(along)) = (dot(d, d), cross(p, d), dot(p, d)) else {
        return Vec::new();
    };
    let long = float(len2).sqrt();
    if long == 0.0 {
        return Vec::new();
    }
    // The line's foot from the circle's centre: `h` away from the centre, `s` along the line from
    // its start (a negative `along` puts the foot ahead of the start).
    let h = float(side).abs() / long;
    let s = -float(along) / long;
    let r = float(wide(radius));
    let half2 = (r - h) * (r + h);
    if half2 < 0.0 {
        return Vec::new();
    }
    let half = half2.sqrt();
    let sin = half / r;
    let unit = (float(d.0) / long, float(d.1) / long);
    let mut found = Vec::new();
    let offsets: &[f64] = if half == 0.0 { &[0.0] } else { &[-1.0, 1.0] };
    for sign in offsets {
        let t = s + sign * half;
        if t < -END_TOLERANCE || t > long + END_TOLERANCE {
            continue;
        }
        let at = (
            float(wide(line.0.x)) + unit.0 * t,
            float(wide(line.0.y)) + unit.1 * t,
        );
        found.extend(on_this_arc(at, arc, sin));
    }
    found
}

/// Where two arcs meet.
fn arc_arc(a: (Point, Length, Angle, Angle), b: (Point, Length, Angle, Angle)) -> Vec<Crossing> {
    let ((c1, r1, ..), (c2, r2, ..)) = (a, b);
    let d = vector(c1, c2);
    let (Some(d2), Some(sum), Some(gap)) = (
        dot(d, d),
        wide(r1).checked_add(wide(r2)),
        wide(r1).checked_sub(wide(r2)),
    ) else {
        return Vec::new();
    };
    let (Some(sum2), Some(gap2)) = (sum.checked_mul(sum), gap.checked_mul(gap)) else {
        return Vec::new();
    };
    // Concentric, apart, or one inside the other: they never meet.
    if d2 == 0 || d2 > sum2 || d2 < gap2 {
        return Vec::new();
    }
    let touching = d2 == sum2 || d2 == gap2;
    let (Some(r1_2), Some(r2_2)) = (
        wide(r1).checked_mul(wide(r1)),
        wide(r2).checked_mul(wide(r2)),
    ) else {
        return Vec::new();
    };
    let Some(top) = r1_2.checked_sub(r2_2).and_then(|n| n.checked_add(d2)) else {
        return Vec::new();
    };
    let long = float(d2).sqrt();
    // The distance from the first centre to the line through both crossings, and the half chord.
    let from_first = float(top) / (2.0 * long);
    let r1f = float(wide(r1));
    let half = if touching {
        0.0
    } else {
        ((r1f - from_first) * (r1f + from_first)).max(0.0).sqrt()
    };
    let sin = long * half / (r1f * float(wide(r2)));
    let unit = (float(d.0) / long, float(d.1) / long);
    let base = (
        float(wide(c1.x)) + unit.0 * from_first,
        float(wide(c1.y)) + unit.1 * from_first,
    );
    let mut found = Vec::new();
    let offsets: &[f64] = if half == 0.0 { &[0.0] } else { &[-1.0, 1.0] };
    for sign in offsets {
        let at = (base.0 - unit.1 * half * sign, base.1 + unit.0 * half * sign);
        if let (Some(first), Some(_)) = (on_this_arc(at, a, sin), on_this_arc(at, b, sin)) {
            found.push(first);
        }
    }
    found
}

/// Every point where two shapes meet, in order of x then y; see the module's header for what
/// counts, and for the bound of each.
#[must_use]
pub fn crossings(a: &Shape, b: &Shape) -> Vec<Crossing> {
    let mut found = match (*a, *b) {
        (Shape::Line { from: f1, to: t1 }, Shape::Line { from: f2, to: t2 }) => {
            line_line((f1, t1), (f2, t2)).into_iter().collect()
        }
        (
            Shape::Line { from, to },
            Shape::Arc {
                centre,
                radius,
                from: s,
                to: e,
            },
        )
        | (
            Shape::Arc {
                centre,
                radius,
                from: s,
                to: e,
            },
            Shape::Line { from, to },
        ) => line_arc((from, to), (centre, radius, s, e)),
        (
            Shape::Arc {
                centre: c1,
                radius: r1,
                from: s1,
                to: e1,
            },
            Shape::Arc {
                centre: c2,
                radius: r2,
                from: s2,
                to: e2,
            },
        ) => arc_arc((c1, r1, s1, e1), (c2, r2, s2, e2)),
    };
    found.sort_by_key(|crossing: &Crossing| crossing.point);
    found.dedup_by_key(|crossing| crossing.point);
    found
}

/// The point of a shape nearest to a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Foot {
    /// The nearest point on the shape, the nearest step.
    pub point: Point,
    /// The distance from the asked point to `point`, the nearest step.
    pub distance: Length,
    /// The distance squared, in whole steps, which is what two feet are compared by.
    pub squared: i128,
}

fn foot_at(from: Point, foot: Point) -> Option<Foot> {
    let d = vector(from, foot);
    let squared = dot(d, d)?;
    Some(Foot {
        point: foot,
        distance: length(round_sqrt(squared)?)?,
        squared,
    })
}

/// The point of `shape` nearest to `point`. `None` only for a shape beyond the limit.
#[must_use]
pub fn nearest_on(shape: &Shape, point: Point) -> Option<Foot> {
    match *shape {
        Shape::Line { from, to } => {
            let d = vector(from, to);
            let len2 = dot(d, d)?;
            if len2 == 0 {
                return None;
            }
            let along = dot(vector(from, point), d)?.clamp(0, len2);
            let at = |start: Length, step: i128| -> Option<Length> {
                length(wide(start).checked_add(round_div(step.checked_mul(along)?, len2)?)?)
            };
            foot_at(point, Point::new(at(from.x, d.0)?, at(from.y, d.1)?))
        }
        Shape::Arc {
            centre,
            radius,
            from,
            to,
        } => {
            let v = vector(centre, point);
            let long = float(dot(v, v)?).sqrt();
            let on = if long == 0.0 {
                false
            } else {
                let theta = libm::atan2(float(v.1), float(v.0));
                on_arc(theta, from, to, 0.0)
            };
            if on {
                let scale = float(wide(radius)) / long;
                let x = float(wide(centre.x)) + float(v.0) * scale;
                let y = float(wide(centre.y)) + float(v.1) * scale;
                let foot = Point::new(
                    Length::from_steps(nearest_step(x)?),
                    Length::from_steps(nearest_step(y)?),
                );
                return foot_at(point, foot);
            }
            // Beyond the arc's ends, or at its centre: the nearer end.
            let start = foot_at(point, shape.end_point(End::Start)?)?;
            let finish = foot_at(point, shape.end_point(End::Finish)?)?;
            Some(if finish.squared < start.squared {
                finish
            } else {
                start
            })
        }
    }
}
