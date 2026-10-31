//! Certified bounds for group 2 on every carrier it measures against: how far a grid point, a line
//! segment or a circle can lie from a plane, cylinder, cone, sphere or ring torus, and how far a
//! grid point lies from a circle's end. Never sampled.
//!
//! What it does: [`carrier`] turns a face's surface into its closed form ([`Carrier`]), and each
//! check bounds a distance from above and compares it with the body's bound b. With w = p − c,
//! h = w · n̂ (height along the axis) and ρ = |w × n̂| (distance from it), the distance of a point
//! from each kind is:
//!
//! | Kind | Distance |
//! | --- | --- |
//! | plane through o, normal n | \|(p − o) · n\| / \|n\|, compared squared so it is exact on grid data |
//! | cylinder or cone, radius R(h) = r₀ + k·h at height h (k = 0 for a cylinder) | at most \|ρ − R(h)\| / √(1 + k²), when R(h) ≥ 0 |
//! | sphere | \| \|w\| − R \| |
//! | ring torus, radii R > r | \| √((ρ − R)² + h²) − r \| |
//!
//! The cone's line is the meridian's: with R(h) ≥ 0 and ρ ≥ 0, the foot of the perpendicular from
//! (ρ, h) to the line ρ = R(h) has radius (R(h) + k²·ρ) / (1 + k²) ≥ 0, so it is a point of the
//! cone and the line distance bounds the true one from above. Where R(h) may be negative (past the
//! apex) nothing is certified.
//!
//! A line segment p(t) = o + t·d on a plane lies within b when both ends do (distance to a plane is
//! affine along it). On a cylinder or cone, f(t) = ρ(t) − R(h(t)) is convex (ρ is the norm of an
//! affine vector, R affine), so its largest value is at an end; below, ρ(t) ≥ u(t) · û for the unit
//! û of either end's u = w × n̂, an affine bound equal to ρ at that end, so f is at least the smaller
//! of its two ends' values. A segment can lie on neither a sphere nor a torus: not certified.
//!
//! A circle of centre c, unit normal ẑ and radius r is compared with the ideal circle of the
//! surface's family, through the same Lipschitz argument (distance to a surface changes by at most
//! the distance a point moves): with α the angle between ẑ and the family's normal, a point of the
//! circle has |e · n̂| ≤ sin α and |e × n̂| ≥ cos α ≥ 1 − sin α for its unit radius vector e, so its
//! distance from the axis is within δ + r·sin α of r, and its height within r·sin α of the
//! centre's (δ the centre's distance from the axis):
//!
//! | Kind | Bound on every point's distance |
//! | --- | --- |
//! | plane | (\|(c − o) · n\| + r·\|ẑ × n\|) / \|n\| (exact: the largest of the affine distance) |
//! | cylinder or cone | \|r − R(H)\| / √(1 + k²) + δ + r·sin α, H the centre's height ± r·sin α, R(H) ≥ 0 |
//! | sphere | \| √(r² + h²) − R \| + δ, h and δ the sphere centre's offsets along and across ẑ |
//! | torus, a parallel | \| √((r − R)² + h²) − rₜ \| + δ + 2r·sin α |
//! | torus, a meridian | \|r − rₜ\| + \|c − core\| + 2r·sin β, β between ẑ and the meridian's normal |
//!
//! The meridian's term pairs each point with the ideal circle's by the rotation taking ẑ to the
//! meridian's normal, which moves a unit vector at most 2·sin(β/2) ≤ 2·sin β. A torus takes the
//! smaller of its two families' bounds, each valid for any circle.
//!
//! Guarantees:
//!
//! - **Upper bounds, certified.** Every distance is bounded from above in interval arithmetic that
//!   stays exact where the doubles are (`geom_curve::tight`), on the exact unit vectors' enclosures
//!   (`geom_curve::Frame::unit_vectors`) and certified sines and cosines of half-turns
//!   (`geom_num::Interval::{sinpi, cospi}`).
//! - **Exact on exact data.** A grid point on a plane given by doubles, a cylinder, a sphere or a
//!   torus whose data are small integers on an axis-aligned frame is measured exactly, so a vertex
//!   exactly at the bound passes and one a step beyond fails. A circle's points come from `sinpi`
//!   and `cospi`, which widen by a step even where exact: its checks hold to that step.
//! - **Fails closed.** A value that cannot be enclosed, a cone point whose radius may be negative,
//!   a torus that is not a ring, a surface kind without a closed form here: not certified, never
//!   passed.
//! - **Nothing allocates; nothing is sampled.**
//!
//! Method: the closed forms of the elementary surfaces of ISO 10303-42 as `geom_surface::analytic`
//! parametrises them, the Lipschitz bound of the distance function, and interval arithmetic
//! (R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM 2009,
//! <https://doi.org/10.1137/1.9780898717716>), for chapter 8.3's group 2
//! (`projects/engines/geometry/architecture/08-topology.md`).
//!
//! Cost: a few dozen interval operations per check; no loop. Recursion: none.

use geom_curve::{Circle, Frame, Line, ParamCurve, tight};
use geom_num::P3;
use geom_num::interval::Interval;
use geom_topo::SurfaceKind;

/// Three enclosed coordinates.
type V3 = [Interval; 3];

/// A face's surface in closed form, as group 2 measures against it.
pub(crate) enum Carrier {
    /// A plane: a point on it and its normal a × b (not of unit length).
    Flat {
        /// A point of the plane.
        origin: V3,
        /// The normal.
        normal: V3,
    },
    /// A cylinder or a cone: its radius at height h along the axis is `radius` + `slope` · h.
    Round(Cone),
    /// A sphere.
    Ball {
        /// The centre.
        center: V3,
        /// The radius.
        radius: f64,
    },
    /// A ring torus, its major radius above its minor one.
    Ring(Torus),
}

/// A cylinder's or cone's data.
pub(crate) struct Cone {
    /// The point at height 0 on the axis.
    center: V3,
    /// The unit axis n̂.
    axis: V3,
    /// The radius at height 0.
    radius: f64,
    /// The change of radius per unit of height: 0 for a cylinder.
    slope: f64,
}

/// A ring torus's data.
pub(crate) struct Torus {
    /// The centre.
    center: V3,
    /// The unit axis n̂.
    axis: V3,
    /// The radius of the core circle.
    major: f64,
    /// The radius of the tube.
    minor: f64,
}

/// The closed form of `surface`; `None` for a kind group 2 has none for (a NURBS or swept surface,
/// a torus that is not a ring).
pub(crate) fn carrier(surface: &SurfaceKind) -> Option<Carrier> {
    match surface {
        SurfaceKind::Plane(p) => {
            let (origin, a, b) = p.vectors();
            Some(Carrier::Flat {
                origin: lift(origin)?,
                normal: cross(lift(a)?, lift(b)?)?,
            })
        }
        SurfaceKind::Cylinder(s) => Some(Carrier::Round(Cone {
            center: lift(s.center())?,
            axis: axis(&s.frame())?,
            radius: s.radius(),
            slope: 0.0,
        })),
        SurfaceKind::Cone(s) => {
            // S(u, v) = c + (r₀ + k·v)·e(u) + s·v·n̂: at height h = s·v the radius is r₀ + k·s·h.
            let (radius, slope) = s.radius_and_slope();
            Some(Carrier::Round(Cone {
                center: lift(s.center())?,
                axis: axis(&s.frame())?,
                radius,
                slope: slope * s.axial_sign(),
            }))
        }
        SurfaceKind::Sphere(s) => Some(Carrier::Ball {
            center: lift(s.center())?,
            radius: s.radius(),
        }),
        SurfaceKind::Torus(s) => {
            let (major, minor) = s.radii();
            if major > minor {
                Some(Carrier::Ring(Torus {
                    center: lift(s.center())?,
                    axis: axis(&s.frame())?,
                    major,
                    minor,
                }))
            } else {
                None
            }
        }
        _ => None,
    }
}

impl Carrier {
    /// Whether grid point `p` lies within `b` steps of the surface.
    pub(crate) fn holds_point(&self, p: P3, b: f64) -> bool {
        grid(p).is_some_and(|q| self.holds(q, b))
    }

    /// Whether every point of `line`'s segment lies within `b` steps of the surface.
    pub(crate) fn holds_line(&self, line: &Line<3>, b: f64) -> bool {
        let (lo, hi) = line.domain();
        let Some((start, end)) = on_line(line, lo).zip(on_line(line, hi)) else {
            return false;
        };
        match self {
            Carrier::Flat { .. } => self.holds(start, b) && self.holds(end, b),
            Carrier::Round(cone) => cone.segment(start, end).is_some_and(|f| fits(f, b)),
            Carrier::Ball { .. } | Carrier::Ring(_) => false,
        }
    }

    /// Whether every point of `circle` lies within `b` steps of the surface.
    pub(crate) fn holds_circle(&self, circle: &Circle<3>, b: f64) -> bool {
        self.circle_bound(circle).is_some_and(|d| d <= b)
    }

    /// Whether the enclosed point `q` lies within `b` steps of the surface.
    fn holds(&self, q: V3, b: f64) -> bool {
        match self {
            Carrier::Flat { origin, normal } => flat(*origin, *normal, q, b),
            Carrier::Round(cone) => polar(cone.center, cone.axis, q)
                .and_then(|(h, rho)| cone.gap(h, rho))
                .is_some_and(|f| fits(f, b)),
            Carrier::Ball { center, radius } => sub3(q, *center)
                .and_then(length)
                .and_then(|d| minus(d, at(*radius)?))
                .is_some_and(|f| fits(f, b)),
            Carrier::Ring(torus) => polar(torus.center, torus.axis, q)
                .and_then(|(h, rho)| torus.gap(rho, h))
                .is_some_and(|f| fits(f, b)),
        }
    }

    /// An upper bound on the distance of every point of `circle` from the surface.
    fn circle_bound(&self, circle: &Circle<3>) -> Option<f64> {
        let (_, [_, _, own]) = circle.frame().unit_vectors().ok()?;
        let c = lift(circle.center())?;
        let r = at(circle.radius())?;
        match self {
            Carrier::Flat { origin, normal } => {
                let off = dot(sub3(c, *origin)?, *normal)?;
                let tilt = length(cross(own, *normal)?)?;
                let reach = plus(at(tight::magnitude(off))?, times(r, tilt)?)?;
                Some(tight::magnitude(divide(reach, length(*normal)?)?))
            }
            Carrier::Round(cone) => {
                let (h, delta) = polar(cone.center, cone.axis, c)?;
                let reach = times(r, length(cross(own, cone.axis)?)?)?;
                let heights = plus(h, spread(reach)?)?;
                let ideal = cone.gap(heights, r)?;
                upper(&[at(tight::magnitude(ideal))?, delta, reach])
            }
            Carrier::Ball { center, radius } => {
                let w = sub3(c, *center)?;
                let h = dot(w, own)?;
                let delta = length(cross(w, own)?)?;
                let ideal = minus(length([r, h, at(0.0)?])?, at(*radius)?)?;
                upper(&[at(tight::magnitude(ideal))?, delta])
            }
            Carrier::Ring(torus) => match (torus.parallel(c, own, r), torus.meridian(c, own, r)) {
                (Some(a), Some(b)) => Some(tight::smaller(a, b)),
                (a, b) => a.or(b),
            },
        }
    }
}

/// Whether grid point `p` lies within `b` steps of `circle`'s point at parameter `t`.
pub(crate) fn at_circle_end(circle: &Circle<3>, t: f64, p: P3, b: f64) -> bool {
    circle_point(circle, t)
        .zip(grid(p))
        .and_then(|(q, v)| length(sub3(q, v)?))
        .is_some_and(|d| fits(d, b))
}

/// One end of a segment against a cone: u = w × n̂, ρ = |u| and the radius at its height.
struct End {
    across: V3,
    rho: Interval,
    radius: Interval,
}

impl Cone {
    /// The radius at height `h`; `None` where it may be negative.
    fn radius_at(&self, h: Interval) -> Option<Interval> {
        let r = plus(at(self.radius)?, times(at(self.slope)?, h)?)?;
        if r.lo() < 0.0 {
            return None;
        }
        Some(r)
    }

    /// (ρ − R(h)) / √(1 + k²) at height `h` and distance `rho` from the axis.
    fn gap(&self, h: Interval, rho: Interval) -> Option<Interval> {
        divide(minus(rho, self.radius_at(h)?)?, slant(self.slope)?)
    }

    /// The end at `q`.
    fn end(&self, q: V3) -> Option<End> {
        let w = sub3(q, self.center)?;
        let across = cross(w, self.axis)?;
        Some(End {
            across,
            rho: length(across)?,
            radius: self.radius_at(dot(w, self.axis)?)?,
        })
    }

    /// An enclosure of f(t) / √(1 + k²) over the segment from `start` to `end`, f = ρ − R(h)
    /// (module documentation): at most each end's value, at least the smaller of them and the
    /// affine bound through either end.
    fn segment(&self, start: V3, end: V3) -> Option<Interval> {
        let (s, e) = (self.end(start)?, self.end(end)?);
        let (fs, fe) = (minus(s.rho, s.radius)?, minus(e.rho, e.radius)?);
        let along = dot(s.across, e.across)?;
        let affine = divide(along, s.rho)
            .and_then(|x| minus(x, e.radius))
            .or_else(|| minus(divide(along, e.rho)?, s.radius))?;
        let lower = tight::smaller(tight::smaller(fs.lo(), fe.lo()), affine.lo());
        let higher = tight::larger(fs.hi(), fe.hi());
        divide(Interval::new(lower, higher).ok()?, slant(self.slope)?)
    }
}

impl Torus {
    /// √((ρ − R)² + h²) − r at distance `rho` from the axis and height `h`.
    fn gap(&self, rho: Interval, h: Interval) -> Option<Interval> {
        minus(
            length([minus(rho, at(self.major)?)?, h, at(0.0)?])?,
            at(self.minor)?,
        )
    }

    /// The bound for a circle near a parallel: centred on the axis, normal along it.
    fn parallel(&self, c: V3, own: V3, r: Interval) -> Option<f64> {
        let (h, delta) = polar(self.center, self.axis, c)?;
        let reach = times(r, length(cross(own, self.axis)?)?)?;
        let ideal = self.gap(r, h)?;
        upper(&[at(tight::magnitude(ideal))?, delta, reach, reach])
    }

    /// The bound for a circle near a meridian: centred on the core circle, in the plane of the
    /// axis and its centre.
    fn meridian(&self, c: V3, own: V3, r: Interval) -> Option<f64> {
        let w = sub3(c, self.center)?;
        let h = dot(w, self.axis)?;
        let rho = length(cross(w, self.axis)?)?;
        let shift = length([minus(rho, at(self.major)?)?, h, at(0.0)?])?;
        let radial = sub3(w, scale(h, self.axis)?)?;
        let outward = divide(dot(own, radial)?, rho)?;
        let tilt = length([dot(own, self.axis)?, outward, at(0.0)?])?;
        let reach = times(r, tilt)?;
        let size = minus(r, at(self.minor)?)?;
        upper(&[at(tight::magnitude(size))?, shift, reach, reach])
    }
}

/// Whether `gap` is at most `b` in size.
fn fits(gap: Interval, b: f64) -> bool {
    tight::magnitude(gap) <= b
}

/// Whether `q` lies within `b` of the plane through `origin` with `normal`:
/// ((q − o) · n)² ≤ b² · |n|², exact on exact data.
fn flat(origin: V3, normal: V3, q: V3, b: f64) -> bool {
    let room = || times(square(at(b)?)?, dot(normal, normal)?);
    sub3(q, origin)
        .and_then(|w| square(dot(w, normal)?))
        .zip(room())
        .is_some_and(|(have, room)| have.hi() <= room.lo())
}

/// A point's height h along the axis and its distance ρ from it.
fn polar(center: V3, axis: V3, q: V3) -> Option<(Interval, Interval)> {
    let w = sub3(q, center)?;
    Some((dot(w, axis)?, length(cross(w, axis)?)?))
}

/// √(1 + k²).
fn slant(slope: f64) -> Option<Interval> {
    tight::sqrt(plus(at(1.0)?, square(at(slope)?)?)?).ok()
}

/// The upper bound of a sum of enclosures.
fn upper(terms: &[Interval]) -> Option<f64> {
    let mut sum = at(0.0)?;
    for &t in terms {
        sum = plus(sum, t)?;
    }
    Some(sum.hi())
}

/// [−|x|, |x|] for every x in `a`.
fn spread(a: Interval) -> Option<Interval> {
    let m = tight::magnitude(a);
    Interval::new(-m, m).ok()
}

/// The enclosure of `circle`'s point at parameter `t`, in half-turns:
/// c + r·cos πt·x̂ + r·sin πt·ŷ.
fn circle_point(circle: &Circle<3>, t: f64) -> Option<V3> {
    let (_, [x, y, _]) = circle.frame().unit_vectors().ok()?;
    let r = at(circle.radius())?;
    let turn = at(t)?;
    let along = scale(times(r, turn.cospi().ok()?)?, x)?;
    let across = scale(times(r, turn.sinpi().ok()?)?, y)?;
    add3(lift(circle.center())?, add3(along, across)?)
}

/// The enclosure of `line`'s point at parameter `t`: origin + t · direction.
fn on_line(line: &Line<3>, t: f64) -> Option<V3> {
    add3(
        lift(line.origin())?,
        scale(at(t)?, lift(line.direction())?)?,
    )
}

/// The unit axis n̂ of `frame`, enclosed.
fn axis(frame: &Frame<3>) -> Option<V3> {
    let (_, [_, _, n]) = frame.unit_vectors().ok()?;
    Some(n)
}

/// A grid point, enclosed exactly.
fn grid(p: P3) -> Option<V3> {
    let whole = |c: geom_num::Coord| {
        #[allow(
            clippy::cast_precision_loss,
            reason = "a grid coordinate is below 2^40, so the double holds it exactly"
        )]
        let x = c.get() as f64;
        at(x)
    };
    Some([whole(p.x)?, whole(p.y)?, whole(p.z)?])
}

/// The point interval at `x`.
fn at(x: f64) -> Option<Interval> {
    tight::point(x).ok()
}

/// `a + b`.
fn plus(a: Interval, b: Interval) -> Option<Interval> {
    tight::add(a, b).ok()
}

/// `a − b`.
fn minus(a: Interval, b: Interval) -> Option<Interval> {
    tight::sub(a, b).ok()
}

/// `a · b`.
fn times(a: Interval, b: Interval) -> Option<Interval> {
    tight::mul(a, b).ok()
}

/// `a / b`, for a `b` certainly not zero.
fn divide(a: Interval, b: Interval) -> Option<Interval> {
    tight::div(a, b).ok()
}

/// `a²`.
fn square(a: Interval) -> Option<Interval> {
    tight::square(a).ok()
}

/// |v|.
fn length(v: V3) -> Option<Interval> {
    tight::norm(&v).ok()
}

/// Doubles, enclosed.
fn lift([x, y, z]: [f64; 3]) -> Option<V3> {
    Some([at(x)?, at(y)?, at(z)?])
}

/// `a + b`, by coordinates.
fn add3([ax, ay, az]: V3, [bx, by, bz]: V3) -> Option<V3> {
    Some([plus(ax, bx)?, plus(ay, by)?, plus(az, bz)?])
}

/// `a − b`, by coordinates.
fn sub3([ax, ay, az]: V3, [bx, by, bz]: V3) -> Option<V3> {
    Some([minus(ax, bx)?, minus(ay, by)?, minus(az, bz)?])
}

/// `s · a`, by coordinates.
fn scale(s: Interval, [ax, ay, az]: V3) -> Option<V3> {
    Some([times(s, ax)?, times(s, ay)?, times(s, az)?])
}

/// `a · b`.
fn dot([ax, ay, az]: V3, [bx, by, bz]: V3) -> Option<Interval> {
    plus(plus(times(ax, bx)?, times(ay, by)?)?, times(az, bz)?)
}

/// `a × b`.
fn cross([ax, ay, az]: V3, [bx, by, bz]: V3) -> Option<V3> {
    Some([
        minus(times(ay, bz)?, times(az, by)?)?,
        minus(times(az, bx)?, times(ax, bz)?)?,
        minus(times(ax, by)?, times(ay, bx)?)?,
    ])
}
