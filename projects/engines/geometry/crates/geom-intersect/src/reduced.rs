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
use geom_num::{Sign, quadric};
use geom_nurbs::Curve;
use geom_run::WorkMeter;
use geom_surface::{Extrusion, ParamSurface, Plane, Revolution, SurfaceKind, Torus};

use crate::curve::{bound_on, curves, finite, seam_of, sign_of, whole};
use crate::curve_surface::{cross, curve_surface, dot, lift, minus};
use crate::error::IntersectError;
use crate::quadrics::{
    Axis, Piece, Vector, axis, beside, circle, decided, funnel, middle, plus, point, same_line,
    scale, sheet, stretch, unit, wide,
};
use crate::result::{Item, PointContact};

/// How far a plane's line runs in the half-plane, in steps: past any surface of the frame.
const OUT: f64 = 4_398_046_511_104.0;

/// A surface's meridian curves, in (ρ, z) about an axis.
type Meridians = Vec<CurveKind<2>>;

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
/// the line's parameter, exact where the data are. `None` for a discriminant no enclosure tells
/// from zero.
fn line_and_circle(
    line: &Line<2>,
    round: &Circle<2>,
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
    for sign in signs {
        let t = tight::div(tight::sub(tight::mul(root, tight::point(*sign)?)?, b)?, a)?;
        if t.hi() < span.0 || t.lo() > span.1 {
            continue;
        }
        let rho = tight::add(px, tight::mul(vx, t)?)?;
        let z = tight::add(py, tight::mul(vy, t)?)?;
        out.push((rho, z, touching));
    }
    Ok(Some(out))
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
    Ok(alloc::vec![CurveKind::Circle(tube)])
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
/// as stored, through `c` along `e`. The side is read at a circle's centre, which that proof needs
/// off the axis, and at any other curve's middle. The map is read at the middle, as it always was,
/// so no meridian moves; where the middle is on the axis, as a horn torus's circle has it, at the
/// centre. (An ellipse's or a parabola's middle is nearer its centre than the radius of the ball
/// [`in_half_plane`] proves clear, strictly, as its axes are not parallel; so where that proof
/// holds the middle is off the axis, on the centre's side, and the middle serves.)
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
    let map = match to_half_plane(on, lift(middle_of)?)? {
        Some(map) => map,
        None => match to_half_plane(on, lift(anchor)?)? {
            Some(map) => map,
            None => return Ok(None),
        },
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

    /// The sign, exactly; `None` where a product of two parts is not exactly a pair of doubles: it
    /// overflows, or its error is finer than the smallest subnormal. [`two_prod`] says which by
    /// its error part, which is finite exactly when the pair is exact, so that one check is the
    /// whole test, at every magnitude.
    fn sign(&self) -> Option<Sign> {
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
        // The exact sum of finite doubles, rounded once to nearest: a nonzero one is a whole
        // multiple of 2^−1074, so it rounds to a nonzero double or an infinity of its own sign,
        // and zero is +0. Not a number only where the sum was lost, which has no order.
        sum.round_ieee().partial_cmp(&0.0).map(|order| match order {
            Ordering::Less => Sign::Negative,
            Ordering::Equal => Sign::Zero,
            Ordering::Greater => Sign::Positive,
        })
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
    Ok(map.image(&kind)?.map(|(curve, _)| alloc::vec![curve]))
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
            alloc::vec![straight([0.0, z], [1.0, 0.0], (0.0, OUT))?]
        }
        SurfaceKind::Sphere(x) if about(x.center(), e) => {
            let z = tight::middle(level(lift(x.center())?, on)?);
            let frame = Frame::<2>::standard()?;
            alloc::vec![CurveKind::Circle(Circle::new(
                [0.0, z],
                x.radius(),
                frame,
                (-0.5, 0.5)
            )?)]
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
            alloc::vec![straight([x.radius(), z], [0.0, 1.0], (lo, hi))?]
        }
        SurfaceKind::Cone(x) if about(x.center(), x.frame().normal()) && whole_turn(s) => {
            let f = funnel(x)?;
            let z = tight::middle(level(f.apex, on)?);
            let k = tight::middle(f.slope);
            alloc::vec![
                straight([0.0, z], [k, 1.0], (0.0, OUT))?,
                straight([0.0, z], [k, -1.0], (0.0, OUT))?,
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

/// The rings at the crossings of two sets of meridian curves. `None` where curve with curve could
/// not decide.
fn rings(
    on: &Axis,
    first: &[CurveKind<2>],
    second: &[CurveKind<2>],
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
    for x in first {
        for y in second {
            let closed = match (x, y) {
                (CurveKind::Line(l), CurveKind::Circle(c))
                | (CurveKind::Circle(c), CurveKind::Line(l)) => Some(line_and_circle(l, c)?),
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
    rings(on, &first, &second, meter)
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
