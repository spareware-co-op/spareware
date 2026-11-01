//! The ideal a construction aims at: feet on curves, and the circle at a given distance from them.
//!
//! What it does: a tangent circle's centre needs square roots that stay closed under nothing, so it
//! is never stored. Chapter 14.3 keeps a rational centre near it instead, and this module says where
//! near is. Every curve of [`crate::carrier::Curve2`] is a conic, so one solver serves them all: the
//! foot of a point on a curve is where the normal through the point meets it, and the distance from a
//! curve moves with the point by the unit normal at the foot, which is Newton's step for the centre.
//!
//! Guarantees:
//!
//! - **Nothing here is a decision.** These are aims: a wrong aim makes the exact questions of
//!   [`crate::tangent`] refuse the circle, or report a larger gap. Doubles of doubles reach these
//!   numbers because each curve's equation is written about a grid point beside the centre by exact
//!   integers ([`geom_num::conic::Conic::about`]), so the size of the coordinates is not in them.
//! - **Deterministic.** Double-doubles from `geom_num::dd`, a fixed limit of steps, and no platform
//!   maths; no allocation.
//!
//! Method: the distance function's gradient is the unit normal at the foot, which makes this
//! Newton's method on the distances; the foot is Newton's method on the curve's equation and the
//! condition that the normal passes through the point. The depths are chapter 14.3's η
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`); the numbers that matter are the
//! exact ones of [`crate::tangent`].
//!
//! Cost: a few hundred double-double operations for lines, some thousands for ellipses.

use geom_num::conic::Conic;
use geom_num::grid::P2;
use geom_num::ladder::ANGULAR;
use geom_num::quartic::Big;
use geom_num::{DD, NumError, Sign};

/// A point or vector, about the local origin.
pub(crate) type Pair = [DD; 2];

/// Steps past which Newton's method has not met its aim.
const STEPS: u32 = 48;

/// A step this small, relative to the size of the point, has met it: 2^−96.
const SMALL: f64 = 1.262_177_448_353_618_9e-29;

/// The angle aimed at where a circle crosses a curve, of [`ANGULAR`]: leaves room for the centre's
/// rounding, as a gap goes by the square root of the depth, and for the bound's looseness.
const AIM: f64 = 0.7;

/// A bound on a crossing's angle by an ellipse's curvature is looser than one by exact integers: so
/// much tighter an aim. The bound's looseness is 4 for the root's stretch, and 2 for the worst
/// curvature.
const TIGHTER: f64 = 1.0 / 16.0;

/// The larger of two doubles, ordered as doubles are.
fn larger(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// A double-double from an integer: its limbs, the highest first.
///
/// # Errors
///
/// [`NumError::NotFinite`] past a double's range, which no coefficient reaches.
pub(crate) fn dd_of<const L: usize>(v: &geom_num::Wide<L>) -> Result<DD, NumError> {
    let magnitude = if v.is_negative() {
        v.checked_neg().ok_or(NumError::BudgetExceeded)?
    } else {
        *v
    };
    let base = 18_446_744_073_709_551_616.0;
    let mut sum = DD::from_i64(0);
    for limb in magnitude.to_le_limbs().iter().rev() {
        let part = DD::from_i128(i128::from(*limb)).ok_or(NumError::BudgetExceeded)?;
        sum = sum.mul_f64(base)?.add(part)?;
    }
    Ok(if v.is_negative() { sum.negate() } else { sum })
}

/// a − b.
pub(crate) fn minus(a: Pair, b: Pair) -> Result<Pair, NumError> {
    let ([ax, ay], [bx, by]) = (a, b);
    Ok([ax.sub(bx)?, ay.sub(by)?])
}

/// a · b.
pub(crate) fn dot(a: Pair, b: Pair) -> Result<DD, NumError> {
    let ([ax, ay], [bx, by]) = (a, b);
    ax.mul(bx)?.add(ay.mul(by)?)
}

/// a × b.
pub(crate) fn cross(a: Pair, b: Pair) -> Result<DD, NumError> {
    let ([ax, ay], [bx, by]) = (a, b);
    ax.mul(by)?.sub(ay.mul(bx)?)
}

/// ‖a‖.
pub(crate) fn norm(a: Pair) -> Result<DD, NumError> {
    dot(a, a)?.sqrt()
}

/// `a` over `by`.
pub(crate) fn over(a: Pair, by: DD) -> Result<Pair, NumError> {
    let [x, y] = a;
    Ok([x.div(by)?, y.div(by)?])
}

/// `a` times `by`.
pub(crate) fn scaled(a: Pair, by: DD) -> Result<Pair, NumError> {
    let [x, y] = a;
    Ok([x.mul(by)?, y.mul(by)?])
}

/// The size of a point, for a step to be small against.
fn size(a: Pair) -> f64 {
    let [x, y] = a;
    larger(x.abs().hi(), y.abs().hi())
}

/// A curve's equation in double-doubles, about a grid point.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Equation {
    xx: DD,
    xy: DD,
    yy: DD,
    x: DD,
    y: DD,
    constant: DD,
}

impl Equation {
    /// `conic`, about `at`.
    pub(crate) fn about(conic: &Conic, at: P2) -> Result<Equation, NumError> {
        let of = |v: &Big| dd_of(v);
        let [xx, xy, yy, x, y, constant] = conic.about(at)?.coefficients();
        Ok(Equation {
            xx: of(&xx)?,
            xy: of(&xy)?,
            yy: of(&yy)?,
            x: of(&x)?,
            y: of(&y)?,
            constant: of(&constant)?,
        })
    }

    /// Its value at `p`.
    fn value(&self, p: Pair) -> Result<DD, NumError> {
        let [x, y] = p;
        let along = self.xx.mul(x)?.add(self.xy.mul(y)?)?.add(self.x)?.mul(x)?;
        let across = self.yy.mul(y)?.add(self.y)?.mul(y)?;
        along.add(across)?.add(self.constant)
    }

    /// Its second derivatives, xx, xy and yy.
    fn second(&self) -> Result<[DD; 3], NumError> {
        Ok([self.xx.add(self.xx)?, self.xy, self.yy.add(self.yy)?])
    }

    /// Its gradient at `p`.
    fn gradient(&self, p: Pair) -> Result<Pair, NumError> {
        let [x, y] = p;
        let [hxx, hxy, hyy] = self.second()?;
        Ok([
            hxx.mul(x)?.add(hxy.mul(y)?)?.add(self.x)?,
            hxy.mul(x)?.add(hyy.mul(y)?)?.add(self.y)?,
        ])
    }

    /// The foot of `c` on the curve near `seed`: Newton's method on the equation and the condition
    /// that the normal there passes through `c`.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] if it does not settle.
    pub(crate) fn foot(&self, c: Pair, seed: Pair) -> Result<Foot, NumError> {
        let [hxx, hxy, hyy] = self.second()?;
        let mut t = seed;
        for _ in 0..STEPS {
            let g = self.gradient(t)?;
            let [gx, gy] = g;
            let [dx, dy] = minus(c, t)?;
            let on = self.value(t)?;
            let aimed = cross([dx, dy], g)?;
            let first = dx.mul(hxy)?.sub(dy.mul(hxx)?)?.sub(gy)?;
            let second = dx.mul(hyy)?.sub(dy.mul(hxy)?)?.add(gx)?;
            let det = gx.mul(second)?.sub(gy.mul(first)?)?;
            if det.sign() == Sign::Zero {
                return Err(NumError::Degenerate);
            }
            let step = [
                aimed.mul(gy)?.sub(on.mul(second)?)?.div(det)?,
                on.mul(first)?.sub(gx.mul(aimed)?)?.div(det)?,
            ];
            let [sx, sy] = step;
            let [x, y] = t;
            t = [x.add(sx)?, y.add(sy)?];
            if size(step) <= SMALL * larger(1.0, size(t)) {
                return self.finished(t);
            }
        }
        Err(NumError::Degenerate)
    }

    /// What the curve is at the point `t` of it: its unit normal and its curvature about that normal.
    fn finished(&self, t: Pair) -> Result<Foot, NumError> {
        let g = self.gradient(t)?;
        let length = norm(g)?;
        let [gx, gy] = over(g, length)?;
        let [hxx, hxy, hyy] = self.second()?;
        let twisted = gy.mul(gx)?.mul(hxy)?;
        let bent = hxx.mul(gy)?.mul(gy)?.sub(twisted)?.sub(twisted)?;
        let curvature = bent.add(hyy.mul(gx)?.mul(gx)?)?.div(length)?;
        Ok(Foot {
            at: t,
            normal: [gx, gy],
            curvature,
        })
    }
}

/// A point of a curve, with what the construction needs of it there.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Foot {
    /// Where it is.
    pub(crate) at: Pair,
    /// The unit normal.
    pub(crate) normal: Pair,
    /// The curvature, positive where the curve bends away from the normal.
    pub(crate) curvature: DD,
}

/// A curve the circle touches, as the solver takes it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Aim {
    /// The curve's equation, about the local origin.
    pub(crate) equation: Equation,
    /// Which side of it the centre lies on: 1 or −1.
    pub(crate) side: f64,
    /// Whether its crossing's angle is bounded by curvature, not by exact integers.
    pub(crate) loose: bool,
    /// Where it was last touched.
    pub(crate) foot: Pair,
}

/// How far past a curve a circle is to reach, and which way.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Depth {
    /// 1 if the foot is the nearest point of the curve, so a larger circle crosses it; −1 if the
    /// farthest, so a smaller one does.
    pub(crate) reach: f64,
    /// The depth: chosen so that the crossing's angle is the aim, and its half chord under half a
    /// step.
    pub(crate) depth: DD,
}

/// The depth for the curve at `foot`, seen from `c` by a circle of radius `r`. Two curves of relative
/// curvature κ, d apart, cross at an angle of √(2dκ) along a half chord of √(2d / κ).
fn depth(foot: &Foot, aim: &Aim, c: Pair, r: DD) -> Result<Depth, NumError> {
    let d = dot(minus(c, foot.at)?, foot.normal)?;
    let bend = foot.curvature.mul(d)?.add(DD::from_i64(1))?;
    let reach = if bend.sign() == Sign::Negative { -1.0 } else { 1.0 };
    let bend = bend.abs();
    let angle = if aim.loose {
        AIM * ANGULAR.get() * TIGHTER
    } else {
        AIM * ANGULAR.get()
    };
    let chord = bend.div(r.add(r)?)?;
    let angle = if chord.hi() < angle {
        chord
    } else {
        DD::from_f64(angle)?
    };
    let depth = angle.mul(angle)?.mul(r)?.div(bend.add(bend)?)?;
    Ok(Depth { reach, depth })
}

/// Whether each curve is as far from the circle as it is aimed to be: within a 256th of its depth,
/// or within `rough` steps on a pass that only finds where to work. By the distances, not the steps
/// of the centre: two lines nearly parallel leave the centre ill-determined along them, and their
/// distances not.
fn settled<const N: usize>(miss: &[DD; N], depths: &[Depth; N], rough: Option<f64>) -> bool {
    miss.iter().zip(depths).all(|(off, deep)| {
        off.abs().hi() <= rough.map_or(deep.depth.hi() / 256.0, |steps| steps)
    })
}

/// The depth of a circle of radius `r` past a line, as [`Depth`] says.
pub(crate) fn plain_depth(r: DD) -> Result<DD, NumError> {
    let angle = AIM * ANGULAR.get();
    let chord = DD::from_i64(1).div(r.add(r)?)?;
    let angle = if chord.hi() < angle {
        chord
    } else {
        DD::from_f64(angle)?
    };
    angle.mul(angle)?.mul(r)?.mul_f64(0.5)
}

/// The solution of the two equations with these rows.
fn solved(rows: [Pair; 2], right: [DD; 2]) -> Result<Pair, NumError> {
    let [first, second] = rows;
    let [p, q] = right;
    let det = cross(first, second)?;
    if det.sign() == Sign::Zero {
        return Err(NumError::Degenerate);
    }
    let [ax, ay] = first;
    let [bx, by] = second;
    Ok([
        p.mul(by)?.sub(q.mul(ay)?)?.div(det)?,
        ax.mul(q)?.sub(bx.mul(p)?)?.div(det)?,
    ])
}

/// What a solve leaves.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Solved<const N: usize> {
    /// The centre.
    pub(crate) centre: Pair,
    /// The radius.
    pub(crate) radius: DD,
    /// Each curve's foot.
    pub(crate) feet: [Foot; N],
    /// Each curve's depth.
    pub(crate) depths: [Depth; N],
}

/// How far the circle about `c` of radius `r` misses each curve's depth, with the feet and depths.
fn residual<const N: usize>(
    aims: &mut [Aim; N],
    c: Pair,
    r: DD,
) -> Result<([Foot; N], [Depth; N], [DD; N]), NumError> {
    let mut feet = [Foot {
        at: c,
        normal: c,
        curvature: r,
    }; N];
    let mut depths = [Depth {
        reach: 1.0,
        depth: r,
    }; N];
    let mut off = [r; N];
    for (aim, (foot, (deep, miss))) in aims
        .iter_mut()
        .zip(feet.iter_mut().zip(depths.iter_mut().zip(off.iter_mut())))
    {
        *foot = aim.equation.foot(c, aim.foot)?;
        aim.foot = foot.at;
        *deep = depth(foot, aim, c, r)?;
        let d = dot(minus(c, foot.at)?, foot.normal)?;
        *miss = d
            .mul_f64(aim.side)?
            .sub(r.sub(deep.depth.mul_f64(deep.reach)?)?)?;
    }
    Ok((feet, depths, off))
}

/// The centre at the distance `r` from both curves, less their depths, starting from `c`.
///
/// # Errors
///
/// [`NumError::Degenerate`] if it does not settle.
pub(crate) fn with_radius(
    aims: &mut [Aim; 2],
    start: Pair,
    r: DD,
    rough: Option<f64>,
) -> Result<Solved<2>, NumError> {
    let mut c = start;
    for _ in 0..STEPS {
        let (feet, depths, miss) = residual(aims, c, r)?;
        if settled(&miss, &depths, rough) {
            return Ok(Solved {
                centre: c,
                radius: r,
                feet,
                depths,
            });
        }
        let [p, q] = miss;
        let [first, second] = &*aims;
        let [one, other] = &feet;
        let rows = [
            scaled(one.normal, DD::from_f64(first.side)?)?,
            scaled(other.normal, DD::from_f64(second.side)?)?,
        ];
        let step = solved(rows, [p.negate(), q.negate()])?;
        let [cx, cy] = c;
        let [sx, sy] = step;
        c = [cx.add(sx)?, cy.add(sy)?];
    }
    Err(NumError::Degenerate)
}

/// The centre and radius at which three curves are touched, starting from `start` and `radius`.
///
/// # Errors
///
/// [`NumError::Degenerate`] if it does not settle.
pub(crate) fn of_three(
    aims: &mut [Aim; 3],
    start: Pair,
    radius: DD,
    rough: Option<f64>,
) -> Result<Solved<3>, NumError> {
    let (mut c, mut r) = (start, radius);
    for _ in 0..STEPS {
        let (feet, depths, miss) = residual(aims, c, r)?;
        if settled(&miss, &depths, rough) {
            return Ok(Solved {
                centre: c,
                radius: r,
                feet,
                depths,
            });
        }
        let [p, q, s] = miss;
        let [one, two, three] = &*aims;
        let [first, second, third] = &feet;
        let row = |foot: &Foot, side: f64| scaled(foot.normal, DD::from_f64(side)?);
        let lead = row(first, one.side)?;
        let rows = [
            minus(row(second, two.side)?, lead)?,
            minus(row(third, three.side)?, lead)?,
        ];
        let step = solved(rows, [p.sub(q)?, p.sub(s)?])?;
        let grown = dot(lead, step)?.add(p)?;
        let [cx, cy] = c;
        let [sx, sy] = step;
        c = [cx.add(sx)?, cy.add(sy)?];
        r = r.add(grown)?;
    }
    Err(NumError::Degenerate)
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "a failed test is its own report"
)]
mod tests {
    use geom_num::arc::Circle2;
    use geom_num::carrier::Carrier;

    use super::*;

    fn zero() -> Pair {
        [DD::from_i64(0), DD::from_i64(0)]
    }

    fn aim(conic: &Conic, side: f64) -> Aim {
        Aim {
            equation: Equation::about(conic, P2::new(0, 0).unwrap()).unwrap(),
            side,
            loose: false,
            foot: zero(),
        }
    }

    #[test]
    fn a_foot_on_a_line_is_one_step_and_the_normal_is_unit() {
        let l = Carrier::new(3, 4, 25).unwrap();
        let conic = Conic::of_carrier(&l).unwrap();
        let foot = aim(&conic, 1.0).equation.foot(zero(), zero()).unwrap();
        assert!((foot.at[0].hi() - 3.0).abs() < 1e-25);
        assert!((foot.at[1].hi() - 4.0).abs() < 1e-25);
        assert!((foot.normal[0].hi() - 0.6).abs() < 1e-25);
        assert_eq!(foot.curvature.hi(), 0.0);
    }

    #[test]
    fn a_foot_on_a_circle_has_its_curvature() {
        let c = Circle2::new(P2::new(10, 0).unwrap(), 5).unwrap();
        let conic = Conic::of_circle(&c).unwrap();
        let seed = [DD::from_i64(4), DD::from_i64(1)];
        let foot = aim(&conic, 1.0).equation.foot(zero(), seed).unwrap();
        assert!((foot.at[0].hi() - 5.0).abs() < 1e-25);
        assert!((foot.normal[0].hi() + 1.0).abs() < 1e-25);
        assert!((foot.curvature.hi() - 0.2).abs() < 1e-25);
    }

    #[test]
    fn two_lines_settle_with_the_circle_past_them_by_the_depth() {
        let (x, y) = (Carrier::new(0, 1, 0).unwrap(), Carrier::new(1, 0, 0).unwrap());
        let mut aims = [
            aim(&Conic::of_carrier(&x).unwrap(), 1.0),
            aim(&Conic::of_carrier(&y).unwrap(), 1.0),
        ];
        let r = DD::from_i64(1_000);
        let done = with_radius(&mut aims, zero(), r, None).unwrap();
        let [first, _] = done.depths;
        assert!(first.depth.hi() > 0.0);
        let to_axis = done.centre[1].hi();
        assert!((to_axis + first.depth.hi() - 1_000.0).abs() < 1e-9);
    }

    #[test]
    fn three_lines_settle_on_their_incircle() {
        let lines = [
            Carrier::new(0, 1, 0).unwrap(),
            Carrier::new(-4, 3, 0).unwrap(),
            Carrier::new(4, 3, 240).unwrap(),
        ];
        let mut aims = [
            aim(&Conic::of_carrier(&lines[0]).unwrap(), 1.0),
            aim(&Conic::of_carrier(&lines[1]).unwrap(), -1.0),
            aim(&Conic::of_carrier(&lines[2]).unwrap(), -1.0),
        ];
        let start = [DD::from_i64(30), DD::from_i64(10)];
        let done = of_three(&mut aims, start, DD::from_i64(5), None).unwrap();
        assert!((done.radius.hi() - 15.0).abs() < 1e-9);
        assert!((done.centre[0].hi() - 30.0).abs() < 1e-9);
    }
}
