//! Where two quadrics in general position meet, and the curve they share (chapter 12.7).
//!
//! What it does: [`meet`] asks `geom_num::pencil` what a sphere, cylinder or cone shares with another,
//! exactly on whole-number data and in intervals otherwise, and where that is a quartic or a cubic
//! writes the curve as [`Parameterised`]: Dupont, Lazard, Lazard and Petitjean's parameterisation over
//! a ruled member of the pencil. It is this curve's definition, which chapter 12.12's procedural
//! kind holds; conics and lines are [`crate::quadrics::natural`]'s.
//!
//! Guarantees:
//!
//! - **Exact topology on exact data.** On whole-number data the number of branches and singular
//!   points comes from exact signs, within `geom_num::pencil`'s budget. Off the grid intervals decide
//!   a smooth quartic or nothing at all, and a pair they cannot tell is `None`.
//! - **Every point on both.** [`Parameterised::point`] encloses a point of both surfaces: the ruled
//!   quadric holds it by construction, and the sheet solves the other exactly, each step rounded
//!   outward, so no marching and no tolerance is involved.
//! - **The arcs agree with the topology.** [`Meeting::arcs`] are found by the sign of a quartic form
//!   on a walk around its line, and given only when they count what the exact answer says; otherwise
//!   there is no curve, never a wrong one.
//!
//! Method: Dupont, Lazard, Lazard and Petitjean, J. Symbolic Computation 43(3), 2008, part I
//! (<https://doi.org/10.1016/j.jsc.2007.10.006>), from the paper alone: a member of inertia (2, 2) is
//! ruled, so a point of it is (u : v) and (s : t) in X = s(uP₁ + vP₂) + t(uR₁ + vR₂). The other quadric
//! is then α(u, v)s² + β(u, v)st + γ(u, v)t² = 0, each of α, β and γ quadratic in (u, v), so
//! s/t = (−β ± √Δ)/2α where Δ = β² − 4αγ is a quartic form. Its real roots bound the branches. The
//! paper's care over which radicals are needed does not arise, since every value here is an enclosure.
//!
//! Cost: a few hundred interval operations to set up, and one quartic per step of the walk, charged
//! to the meter. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the paper's: u, v, s, t, α, β and γ, and P and R"
)]

use alloc::vec::Vec;

use geom_curve::tight;
use geom_num::grid::{P3, Steps};
use geom_num::interval::Interval;
use geom_num::pencil::{self, Entry, Form, Inertia, Quartic, Segre, Topology};
use geom_num::quadric;
use geom_num::quadric::Ring;
use geom_run::WorkMeter;
use geom_surface::{Cone, Cylinder, Sphere, SurfaceKind};

use crate::curve::{grid_point, whole};
use crate::error::IntersectError;
use crate::quadrics::{axis, funnel};

/// A homogeneous point, enclosed.
type Point4 = [Interval; 4];

/// The coarsest walk around a line.
const WALK: u32 = 64;

/// The finest: past it there is no curve.
const FINEST: u32 = 16_384;

/// How many halvings an end of an arc takes, to about the width of a double.
const HALVINGS: u32 = 48;

/// One surface, as what the two kinds of arithmetic need of it.
struct Surface {
    /// Its form in intervals, about the origin of the pair.
    form: Form<Interval>,
    /// Its form on whole numbers, about the same point in `P3`s, if its data are whole.
    exact: Option<Form<Entry>>,
}

/// The point the forms of a pair are written about: subtracting it first keeps their entries small
/// and the filter's cancellation low.
fn reference(s: &SurfaceKind) -> Result<Option<Vector3>, IntersectError> {
    Ok(match s {
        SurfaceKind::Sphere(x) => Some(lifted(x.center())?),
        SurfaceKind::Cylinder(x) => Some(lifted(x.center())?),
        SurfaceKind::Cone(x) => Some(funnel(x)?.apex),
        _ => None,
    })
}

/// A vector, enclosed.
type Vector3 = [Interval; 3];

/// `p` as points.
fn lifted(p: [f64; 3]) -> Result<Vector3, IntersectError> {
    let [x, y, z] = p;
    Ok([tight::point(x)?, tight::point(y)?, tight::point(z)?])
}

/// `a − b`, staying exact where it is.
fn apart(a: Vector3, b: Vector3) -> Result<Vector3, IntersectError> {
    let ([p, q, r], [x, y, z]) = (a, b);
    Ok([tight::sub(p, x)?, tight::sub(q, y)?, tight::sub(r, z)?])
}

/// The interval form `revolution` writes, or the budget's error.
fn written(
    c: Vector3,
    e: Vector3,
    alpha: Interval,
    beta: Interval,
    gamma: Interval,
) -> Result<Form<Interval>, IntersectError> {
    pencil::revolution(c, e, alpha, beta, gamma).ok_or(IntersectError::BudgetExceeded)
}

/// The grid point of a reference, if it has one.
fn grid_origin(origin: &Vector3) -> Option<P3> {
    let [x, y, z] = origin;
    if x.lo() != x.hi() || y.lo() != y.hi() || z.lo() != z.hi() {
        return None;
    }
    grid_point(&[x.lo(), y.lo(), z.lo()])
}

/// A sphere about `origin`.
fn sphere(s: &Sphere, origin: &Vector3) -> Result<Surface, IntersectError> {
    let one = tight::point(1.0)?;
    let radius = tight::point(s.radius())?;
    let form = written(
        apart(lifted(s.center())?, *origin)?,
        [tight::point(0.0)?; 3],
        one,
        tight::point(0.0)?,
        tight::square(radius)?,
    )?;
    let exact = (|| {
        let solid = quadric::Sphere {
            centre: grid_point(&s.center())?,
            radius: Steps(u64::try_from(whole(s.radius())?).ok()?),
        };
        pencil::sphere_form(&solid, grid_origin(origin)?)
    })();
    Ok(Surface { form, exact })
}

/// A cylinder about `origin`.
fn cylinder(c: &Cylinder, origin: &Vector3) -> Result<Surface, IntersectError> {
    let line = axis(c.center(), &c.frame(), 1.0)?;
    let one = tight::point(1.0)?;
    let form = written(
        apart(line.at, *origin)?,
        line.along,
        one,
        one,
        tight::square(tight::point(c.radius())?)?,
    )?;
    let exact = (|| {
        let (base, along) = line.exact?;
        let solid = quadric::Cylinder {
            base,
            along,
            radius: Steps(u64::try_from(whole(c.radius())?).ok()?),
        };
        pencil::cylinder_form(&solid, grid_origin(origin)?)
    })();
    Ok(Surface { form, exact })
}

/// A cone about `origin`.
fn cone(c: &Cone, origin: &Vector3) -> Result<Surface, IntersectError> {
    let f = funnel(c)?;
    let one = tight::point(1.0)?;
    let form = written(
        apart(f.apex, *origin)?,
        f.along,
        one,
        tight::add(one, tight::square(f.slope)?)?,
        tight::point(0.0)?,
    )?;
    let exact = f
        .exact
        .and_then(|solid| pencil::cone_form(&solid, grid_origin(origin)?));
    Ok(Surface { form, exact })
}

/// `s` as a [`Surface`], for the three kinds a pencil of curved quadrics holds.
fn surface(s: &SurfaceKind, origin: &Vector3) -> Result<Option<Surface>, IntersectError> {
    Ok(match s {
        SurfaceKind::Sphere(x) => Some(sphere(x, origin)?),
        SurfaceKind::Cylinder(x) => Some(cylinder(x, origin)?),
        SurfaceKind::Cone(x) => Some(cone(x, origin)?),
        _ => None,
    })
}

/// A branch's stretch of the line (u : v), as a turn θ in half-turns, u = cos πθ and v = sin πθ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arc {
    /// All of the line: both sheets are closed curves, or one crossing itself.
    Whole,
    /// From one root of Δ to another, up the first sheet and back down the second, the ends enclosed.
    Between {
        /// Where it starts.
        from: Interval,
        /// Where it ends, past `from`, by less than a whole turn of the line.
        to: Interval,
    },
}

/// A curve on two quadrics, as (u : v) and the sheet of s/t over it.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameterised {
    /// The ruled quadric's generators: X = s(uP₁ + vP₂) + t(uR₁ + vR₂).
    p: [Point4; 2],
    /// As `p`.
    r: [Point4; 2],
    /// α, β and γ, each as its coefficients of u², uv and v².
    coefficients: [[Interval; 3]; 3],
    /// What the forms were written about.
    origin: Vector3,
}

impl Parameterised {
    /// (cos πθ, sin πθ).
    fn turn(theta: Interval) -> Result<(Interval, Interval), IntersectError> {
        Ok((theta.cospi()?, theta.sinpi()?))
    }

    /// A quadratic form in (u, v).
    fn form(k: &[Interval; 3], u: Interval, v: Interval) -> Result<Interval, IntersectError> {
        let [a, b, c] = k;
        let first = a.mul(u)?.add(b.mul(v)?)?.mul(u)?;
        Ok(first.add(c.mul(v)?.mul(v)?)?)
    }

    /// α, β and γ at (u, v).
    fn at(&self, u: Interval, v: Interval) -> Result<[Interval; 3], IntersectError> {
        let [a, b, c] = &self.coefficients;
        Ok([
            Self::form(a, u, v)?,
            Self::form(b, u, v)?,
            Self::form(c, u, v)?,
        ])
    }

    /// Δ = β² − 4αγ at the turn `theta`.
    ///
    /// # Errors
    ///
    /// An interval's error.
    pub fn discriminant(&self, theta: Interval) -> Result<Interval, IntersectError> {
        let (u, v) = Self::turn(theta)?;
        let [a, b, c] = self.at(u, v)?;
        let four = tight::point(4.0)?;
        Ok(b.square()?.sub(four.mul(a)?.mul(c)?)?)
    }

    /// The point of the sheet `upper` over the turn `theta`, enclosed, in the frame of the model.
    /// `None` where it stands at infinity, or at a root of α, β and γ together.
    ///
    /// # Errors
    ///
    /// An interval's error.
    pub fn point(&self, theta: Interval, upper: bool) -> Result<Option<Vector3>, IntersectError> {
        let (u, v) = Self::turn(theta)?;
        let [a, b, c] = self.at(u, v)?;
        let four = tight::point(4.0)?;
        let delta = b.square()?.sub(four.mul(a)?.mul(c)?)?;
        // Within an arc Δ is not negative, whatever its enclosure's lower end says.
        let root = if delta.hi() <= 0.0 {
            tight::point(0.0)?
        } else {
            Interval::new(tight::larger(delta.lo(), 0.0), delta.hi())?.sqrt()?
        };
        let signed = if upper { root } else { root.neg() };
        let two = tight::point(2.0)?;
        // The two forms of the root, the one that does not cancel: −β + σ√Δ over 2α, or 2γ over
        // −β − σ√Δ.
        let adds = (b.lo() > 0.0) != upper;
        let (s, t) = if adds {
            (b.neg().add(signed)?, two.mul(a)?)
        } else {
            (two.mul(c)?, b.neg().sub(signed)?)
        };
        let mut x = [tight::point(0.0)?; 4];
        let ([p1, p2], [r1, r2]) = (&self.p, &self.r);
        for (k, slot) in x.iter_mut().enumerate() {
            let cell = |w: &Point4| w.get(k).copied().ok_or(IntersectError::Internal);
            let along_p = u.mul(cell(p1)?)?.add(v.mul(cell(p2)?)?)?;
            let along_r = u.mul(cell(r1)?)?.add(v.mul(cell(r2)?)?)?;
            *slot = s.mul(along_p)?.add(t.mul(along_r)?)?;
        }
        let [x0, x1, x2, w] = x;
        if w.contains(0.0) {
            return Ok(None);
        }
        let [o0, o1, o2] = self.origin;
        Ok(Some([
            x0.div(w)?.add(o0)?,
            x1.div(w)?.add(o1)?,
            x2.div(w)?.add(o2)?,
        ]))
    }
}

/// What two curved quadrics share.
#[derive(Clone, Debug, PartialEq)]
pub struct Meeting {
    /// The pencil's symbol.
    pub segre: Segre,
    /// What they share over the reals, in projective space.
    pub topology: Topology,
    /// The quartic or cubic, if that is what they share and its arcs could be proven.
    pub curve: Option<Parameterised>,
    /// Its branches.
    pub arcs: Vec<Arc>,
}

/// `m` by elimination of Lagrange: a matrix T and a diagonal D with `TᵀmT` = D, or `None` if a pivot
/// cannot be told from zero.
fn diagonal(m: &Form<Interval>) -> Result<Option<(Form<Interval>, Point4)>, IntersectError> {
    let zero = tight::point(0.0)?;
    let one = tight::point(1.0)?;
    let mut t = [[zero; 4]; 4];
    for (i, row) in t.iter_mut().enumerate() {
        *row.get_mut(i).ok_or(IntersectError::Internal)? = one;
    }
    let mut a = *m;
    let cell = |a: &Form<Interval>, r: usize, c: usize| -> Result<Interval, IntersectError> {
        a.get(r)
            .and_then(|row| row.get(c))
            .copied()
            .ok_or(IntersectError::Internal)
    };
    let size = |x: Interval| tight::larger(x.lo().abs(), x.hi().abs());
    for k in 0..4_usize {
        // The largest pivot left; where the diagonal is too small, a sum of two rows makes one.
        let mut best = (k, 0.0_f64);
        for i in k..4 {
            let here = size(cell(&a, i, i)?);
            if here > best.1 {
                best = (i, here);
            }
        }
        let mut pair = (k, k, 0.0_f64);
        for i in k..4 {
            for j in i.saturating_add(1)..4 {
                let here = size(cell(&a, i, j)?);
                if here > pair.2 {
                    pair = (i, j, here);
                }
            }
        }
        if pair.2 > best.1 {
            let (i, j, _) = pair;
            for r in 0..4 {
                let add = cell(&a, j, r)?;
                let slot = a.get_mut(i).and_then(|row| row.get_mut(r));
                let slot = slot.ok_or(IntersectError::Internal)?;
                *slot = slot.add(add)?;
            }
            for r in 0..4 {
                let add = cell(&a, r, j)?;
                let slot = a.get_mut(r).and_then(|row| row.get_mut(i));
                let slot = slot.ok_or(IntersectError::Internal)?;
                *slot = slot.add(add)?;
                let add = cell(&t, r, j)?;
                let slot = t.get_mut(r).and_then(|row| row.get_mut(i));
                let slot = slot.ok_or(IntersectError::Internal)?;
                *slot = slot.add(add)?;
            }
            best = (i, size(cell(&a, i, i)?));
        }
        let i = best.0;
        a.swap(k, i);
        for row in &mut a {
            row.swap(k, i);
        }
        for row in &mut t {
            row.swap(k, i);
        }
        let pivot = cell(&a, k, k)?;
        if pivot.contains(0.0) {
            return Ok(None);
        }
        for r in k.saturating_add(1)..4 {
            let factor = cell(&a, r, k)?.div(pivot)?;
            for c in 0..4 {
                let less = factor.mul(cell(&a, k, c)?)?;
                let slot = a.get_mut(r).and_then(|row| row.get_mut(c));
                let slot = slot.ok_or(IntersectError::Internal)?;
                *slot = slot.sub(less)?;
            }
            for c in 0..4 {
                let less = factor.mul(cell(&a, c, k)?)?;
                let slot = a.get_mut(c).and_then(|row| row.get_mut(r));
                let slot = slot.ok_or(IntersectError::Internal)?;
                *slot = slot.sub(less)?;
                let less = factor.mul(cell(&t, c, k)?)?;
                let slot = t.get_mut(c).and_then(|row| row.get_mut(r));
                let slot = slot.ok_or(IntersectError::Internal)?;
                *slot = slot.sub(less)?;
            }
            // What elimination leaves here is zero, whatever the enclosure says.
            *a.get_mut(r)
                .and_then(|row| row.get_mut(k))
                .ok_or(IntersectError::Internal)? = zero;
            *a.get_mut(k)
                .and_then(|row| row.get_mut(r))
                .ok_or(IntersectError::Internal)? = zero;
        }
    }
    let mut d = [zero; 4];
    for (i, slot) in d.iter_mut().enumerate() {
        *slot = cell(&a, i, i)?;
    }
    Ok(Some((t, d)))
}

/// Column `k` of `t`, over the root of `size`.
fn column(t: &Form<Interval>, k: usize, size: Interval) -> Result<Point4, IntersectError> {
    let scale = size.sqrt()?;
    let mut out = [tight::point(0.0)?; 4];
    for (slot, row) in out.iter_mut().zip(t) {
        *slot = row.get(k).ok_or(IntersectError::Internal)?.div(scale)?;
    }
    Ok(out)
}

/// `(a ± b) / 2`, entry by entry.
fn half(a: &Point4, b: &Point4, plus: bool) -> Result<Point4, IntersectError> {
    let two = tight::point(2.0)?;
    let mut out = *a;
    for (slot, other) in out.iter_mut().zip(b) {
        let sum = if plus {
            slot.add(*other)?
        } else {
            slot.sub(*other)?
        };
        *slot = sum.div(two)?;
    }
    Ok(out)
}

/// xᵀ B y.
fn bilinear(b: &Form<Interval>, x: &Point4, y: &Point4) -> Result<Interval, IntersectError> {
    let mut sum = tight::point(0.0)?;
    for (row, p) in b.iter().zip(x) {
        let mut line = tight::point(0.0)?;
        for (cell, q) in row.iter().zip(y) {
            line = line.add(cell.mul(*q)?)?;
        }
        sum = sum.add(line.mul(*p)?)?;
    }
    Ok(sum)
}

/// `x` scaled so that its largest entry stands near one.
fn normalised(x: [[Interval; 3]; 3]) -> Result<[[Interval; 3]; 3], IntersectError> {
    let mut top = 0.0_f64;
    for k in x.iter().flatten() {
        top = tight::larger(top, tight::larger(k.lo().abs(), k.hi().abs()));
    }
    if top == 0.0 {
        return Ok(x);
    }
    let scale = tight::point(top)?;
    let mut out = x;
    for k in out.iter_mut().flatten() {
        *k = k.div(scale)?;
    }
    Ok(out)
}

/// The member at (p : q) of `a` and `b`.
fn member(
    a: &Form<Interval>,
    b: &Form<Interval>,
    p: i64,
    q: i64,
) -> Result<Form<Interval>, IntersectError> {
    let (x, y) = (
        <Interval as Ring>::of(p).ok_or(IntersectError::Internal)?,
        <Interval as Ring>::of(q).ok_or(IntersectError::Internal)?,
    );
    let mut out = *a;
    for (row, (ar, br)) in out.iter_mut().zip(a.iter().zip(b)) {
        for (slot, (u, v)) in row.iter_mut().zip(ar.iter().zip(br)) {
            *slot = x.mul(*u)?.add(y.mul(*v)?)?;
        }
    }
    Ok(out)
}

/// A curve on a ruled member of the pencil of `a` and `b`, walked for one of inertia (2, 2).
fn ruled(
    a: &Form<Interval>,
    b: &Form<Interval>,
    origin: Vector3,
) -> Result<Option<Parameterised>, IntersectError> {
    const STEPS: i64 = 32;
    for i in 0..STEPS.saturating_mul(2) {
        let (p, q) = if i <= STEPS {
            (i, STEPS.saturating_sub(i))
        } else {
            (
                i.saturating_sub(STEPS.saturating_mul(2)),
                i.saturating_sub(STEPS),
            )
        };
        let m = member(a, b, p, q)?;
        if pencil::inertia_certain(&m)
            != Some(Inertia {
                positive: 2,
                negative: 2,
            })
        {
            continue;
        }
        let Some((t, d)) = diagonal(&m)? else {
            continue;
        };
        let mut up = Vec::new();
        let mut down = Vec::new();
        for (k, size) in d.iter().enumerate() {
            if size.lo() > 0.0 {
                up.push(column(&t, k, *size)?);
            } else if size.hi() < 0.0 {
                down.push(column(&t, k, size.neg())?);
            }
        }
        let ([c1, c2], [c3, c4]) = (up.as_slice(), down.as_slice()) else {
            continue;
        };
        // (α₁ + α₃)(α₁ − α₃) = (α₄ + α₂)(α₄ − α₂), with α₁ + α₃ = us, α₁ − α₃ = vt, α₄ + α₂ = ut and
        // α₄ − α₂ = vs.
        let p = [half(c1, c3, true)?, half(c4, c2, false)?];
        let r = [half(c2, c4, true)?, half(c1, c3, false)?];
        // Any member other than this one: the second of the pair, or the first.
        let other = if q == 0 { b } else { a };
        let [p1, p2] = &p;
        let [r1, r2] = &r;
        let double = tight::point(2.0)?;
        let coefficients = normalised([
            [
                bilinear(other, p1, p1)?,
                double.mul(bilinear(other, p1, p2)?)?,
                bilinear(other, p2, p2)?,
            ],
            [
                double.mul(bilinear(other, p1, r1)?)?,
                double.mul(bilinear(other, p1, r2)?.add(bilinear(other, p2, r1)?)?)?,
                double.mul(bilinear(other, p2, r2)?)?,
            ],
            [
                bilinear(other, r1, r1)?,
                double.mul(bilinear(other, r1, r2)?)?,
                bilinear(other, r2, r2)?,
            ],
        ])?;
        return Ok(Some(Parameterised {
            p,
            r,
            coefficients,
            origin,
        }));
    }
    Ok(None)
}

/// A turn enclosed between its two ends of a sign change of Δ.
fn refined(
    curve: &Parameterised,
    mut lo: f64,
    mut hi: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Interval>, IntersectError> {
    let sign_at = |x: f64| -> Result<Option<bool>, IntersectError> {
        let d = curve.discriminant(tight::point(x)?)?;
        Ok(if d.lo() > 0.0 {
            Some(true)
        } else if d.hi() < 0.0 {
            Some(false)
        } else {
            None
        })
    };
    let Some(left) = sign_at(lo)? else {
        return Ok(None);
    };
    for _ in 0..HALVINGS {
        meter.charge(1)?;
        let middle = lo * 0.5 + hi * 0.5;
        match sign_at(middle)? {
            Some(side) if side == left => lo = middle,
            Some(_) => hi = middle,
            None => break,
        }
    }
    Ok(Some(Interval::new(lo, hi)?))
}

/// The branches of `curve`: the sign of Δ on a walk of `steps` around the line, one arc for each
/// stretch not negative. `None` if a sign could not be told.
fn walked_arcs(
    curve: &Parameterised,
    steps: u32,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<Arc>>, IntersectError> {
    let share = 1.0 / f64::from(steps);
    let mut signs = Vec::new();
    for i in 0..steps {
        meter.charge(1)?;
        let theta = f64::from(i) * share;
        let d = curve.discriminant(tight::point(theta)?)?;
        if d.lo() > 0.0 {
            signs.push((theta, true));
        } else if d.hi() < 0.0 {
            signs.push((theta, false));
        }
    }
    let Some(&(first_at, first)) = signs.first() else {
        return Ok(None);
    };
    let mut changes = Vec::new();
    let mut last = (first_at, first);
    for &(at, sign) in &signs {
        if sign != last.1 {
            changes.push((last.0, at, sign));
        }
        last = (at, sign);
    }
    if last.1 != first {
        changes.push((last.0, first_at + 1.0, first));
    }
    if changes.is_empty() {
        return Ok(first.then(|| alloc::vec![Arc::Whole]));
    }
    // Rotate to start at a rise, and pair each rise with the fall after it.
    let rise = changes
        .iter()
        .position(|c| c.2)
        .ok_or(IntersectError::Internal)?;
    changes.rotate_left(rise);
    let mut out = Vec::new();
    for pair in changes.chunks(2) {
        let [(a, b, _), (c, d, _)] = pair else {
            return Ok(None);
        };
        let (Some(from), Some(to)) = (
            refined(curve, *a, *b, meter)?,
            refined(curve, *c, *d, meter)?,
        ) else {
            return Ok(None);
        };
        // The fall comes before the rise round the loop when its bracket ends where the rise's
        // begins or earlier: one negative sample between them is enough.
        let to = if d <= a {
            to.add(tight::point(1.0)?)?
        } else {
            to
        };
        out.push(Arc::Between { from, to });
    }
    Ok(Some(out))
}

/// Whether `arcs` count what `topology` says.
fn counts(arcs: &[Arc], topology: &Topology) -> bool {
    let whole = matches!(arcs, [Arc::Whole]);
    match topology.quartic {
        Some(Quartic::Smooth { ovals: 2 }) => arcs.len() == 2 || whole,
        Some(Quartic::Smooth { .. } | Quartic::Isolated | Quartic::Cusp) => {
            arcs.len() == 1 && !whole
        }
        _ => arcs.len() == 1,
    }
}

/// What two curved natural quadrics share, and the curve where that is a quartic or a cubic. `None`
/// for any other kind, or for a pair neither the filter nor exact arithmetic can read.
///
/// # Errors
///
/// A kind's and an interval's errors, and the meter's stop.
pub fn meet(
    a: &SurfaceKind,
    b: &SurfaceKind,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Meeting>, IntersectError> {
    meter.poll()?;
    let Some(origin) = reference(a)? else {
        return Ok(None);
    };
    let (Some(first), Some(second)) = (surface(a, &origin)?, surface(b, &origin)?) else {
        return Ok(None);
    };
    let found = match pencil::filtered(&first.form, &second.form) {
        Some(topology) => Some((Segre::Simple, topology)),
        None => match (&first.exact, &second.exact) {
            (Some(x), Some(y)) => {
                let answer = pencil::classify(x, y)?;
                Some((answer.segre, answer.topology))
            }
            _ => None,
        },
    };
    let Some((segre, topology)) = found else {
        return Ok(None);
    };
    let mut curve = None;
    let mut arcs = Vec::new();
    if (topology.quartic.is_some() || topology.cubics > 0)
        && let Some(on) = ruled(&first.form, &second.form, origin)?
    {
        let mut steps = WALK;
        while steps <= FINEST {
            if let Some(found) = walked_arcs(&on, steps, meter)?
                && counts(&found, &topology)
            {
                arcs = found;
                curve = Some(on);
                break;
            }
            steps = steps.saturating_mul(4);
        }
    }
    Ok(Some(Meeting {
        segre,
        topology,
        curve,
        arcs,
    }))
}
