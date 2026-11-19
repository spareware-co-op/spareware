//! Certified bounds for group 2 on every carrier it measures against: how far a grid point, a line
//! segment or a circle can lie from a plane, cylinder, cone, sphere or ring torus, how far an
//! ellipse can lie from a plane or cylinder, and how far a grid point lies from a circle's or an
//! ellipse's end. Never sampled.
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
//! An ellipse p(t) = c + cos πt·u + sin πt·v (its semi-axes on a frame, u = a·x̂ and v = b·ŷ, or
//! any conjugate pair) is bounded in closed form, with no ideal curve between:
//!
//! | Kind | Bound on every point's distance |
//! | --- | --- |
//! | plane | (\|(c − o) · n\| + √((u · n)² + (v · n)²)) / \|n\|, the largest of the affine distance (a circle's is the case u = r·x̂, v = r·ŷ) |
//! | cylinder of radius R | δ + max(\|A − R\|, \|B − R\|), δ the centre's distance from the axis |
//!
//! On the cylinder, (p − C) × n̂ = (c − C) × n̂ + cos πt·X + sin πt·Y with X = u × n̂ and
//! Y = v × n̂, so a point's distance from the axis is within δ of |cos πt·X + sin πt·Y|, the
//! radius of the ellipse's shadow across the axis, which runs over [B, A] for its semi-axes
//! A = √(m + s) and B = |X × Y| / A, where m = (|X|² + |Y|²) / 2 and
//! s = √(((|X|² − |Y|²) / 2)² + (X · Y)²) (the extremes of |X|² cos² + |Y|² sin² + 2 X · Y cos sin,
//! whose product is |X × Y|²). The distance from the cylinder is |ρ − R|, so every point lies
//! within δ + max(|A − R|, |B − R|) of it: 0 exactly when the centre is on the axis and the
//! shadow is the circle of radius R, as for every plane's cut of the cylinder. A cone (whose
//! radius changes along the height the ellipse spans), a sphere and a torus have no such bound
//! here: an ellipse on one is not certified, and group 2 says so, never that it is off.
//!
//! Guarantees:
//!
//! - **Upper bounds, certified.** Every distance is bounded from above in interval arithmetic that
//!   stays exact where the doubles are (`geom_curve::tight`), on the exact unit vectors' enclosures
//!   (`geom_curve::Frame::unit_vectors`) and certified sines and cosines of half-turns
//!   (`geom_num::Interval::{sinpi, cospi}`).
//! - **Exact on exact data.** A grid point on a plane given by doubles, a cylinder, a sphere or a
//!   torus whose data are small integers on an axis-aligned frame is measured exactly, so a vertex
//!   exactly at the bound passes and one a step beyond fails; so is an ellipse whose centre and
//!   vectors are exact doubles on such a plane or cylinder. A circle's and an ellipse's points
//!   come from `sinpi` and `cospi`, which widen by a step even where exact: their ends' checks
//!   hold to that step.
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
//! Cost: a few dozen interval operations per check (an ellipse about 40 on a plane, about 95 with
//! three more square roots on a cylinder, about 30 with a certified sine and cosine at an end);
//! no loop. Recursion: none.

use geom_curve::{Axes, Circle, Ellipse, Frame, Line, ParamCurve, tight};
use geom_num::interval::Interval;
use geom_num::plane::Plane as GridPlane;
use geom_num::{P3, Sign};
use geom_topo::SurfaceKind;

use crate::flat::around;

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
    /// The change of radius per unit of height, enclosed: 0 for a cylinder.
    slope: Interval,
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
        // A cylinder's height is read from the point, h = w · n̂, never from its parameter v, so
        // its axial factor s (which a similarity scales) does not enter.
        SurfaceKind::Cylinder(s) => Some(Carrier::Round(Cone {
            center: lift(s.center())?,
            axis: axis(&s.frame())?,
            radius: s.radius(),
            slope: at(0.0)?,
        })),
        SurfaceKind::Cone(s) => {
            // S(u, v) = c + (r₀ + k·v)·e(u) + s·v·n̂: the point at v is at height h = s·v, so its
            // radius r₀ + k·v is r₀ + (k/s)·h. The factor s is ±1 as built, where k/s is k·s, but a
            // similarity multiplies it by its scale (and k with it), and then only k/s is the
            // slope. Enclosed: exact wherever the quotient is a double, as it is for s = ±1.
            let (radius, slope) = s.radius_and_slope();
            Some(Carrier::Round(Cone {
                center: lift(s.center())?,
                axis: axis(&s.frame())?,
                radius,
                slope: divide(at(slope)?, at(s.axial_sign())?)?,
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

    /// Whether every point of `ellipse` lies within `b` steps of the surface: `None` on a surface
    /// that has no certified test of an ellipse (a cone, a sphere, a torus).
    pub(crate) fn holds_ellipse(&self, ellipse: &Ellipse<3>, b: f64) -> Option<bool> {
        let bound = match self {
            Carrier::Flat { origin, normal } => {
                Oval::of(ellipse).and_then(|e| e.flat(*origin, *normal))
            }
            Carrier::Round(cylinder) if tight::magnitude(cylinder.slope) == 0.0 => {
                Oval::of(ellipse).and_then(|e| e.round(cylinder))
            }
            Carrier::Round(_) | Carrier::Ball { .. } | Carrier::Ring(_) => return None,
        };
        Some(bound.is_some_and(|d| d <= b))
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

/// An ellipse as group 2 measures it: its centre and the two vectors of its parameter,
/// c + cos πt·u + sin πt·v, enclosed (module documentation).
struct Oval {
    center: V3,
    u: V3,
    v: V3,
}

impl Oval {
    /// `ellipse`'s vectors: a·x̂ and b·ŷ on its frame's exact unit vectors, or the vectors stored.
    fn of(ellipse: &Ellipse<3>) -> Option<Oval> {
        let (first, second) = match ellipse.axes() {
            Axes::Frame {
                frame,
                a: along,
                b: across,
            } => {
                let (_, [x_hat, y_hat, _]) = frame.unit_vectors().ok()?;
                (scale(at(along)?, x_hat)?, scale(at(across)?, y_hat)?)
            }
            Axes::Vectors {
                u: first,
                v: second,
            } => (lift(first)?, lift(second)?),
        };
        Some(Oval {
            center: lift(ellipse.center())?,
            u: first,
            v: second,
        })
    }

    /// The enclosure of the point at parameter `t`, in half-turns.
    fn point(&self, t: f64) -> Option<V3> {
        let turn = at(t)?;
        let along = scale(turn.cospi().ok()?, self.u)?;
        let across = scale(turn.sinpi().ok()?, self.v)?;
        add3(self.center, add3(along, across)?)
    }

    /// An upper bound on every point's distance from the plane through `origin` with `normal`:
    /// (|(c − o) · n| + √((u · n)² + (v · n)²)) / |n|, the largest of the affine distance.
    fn flat(&self, origin: V3, normal: V3) -> Option<f64> {
        let off = dot(sub3(self.center, origin)?, normal)?;
        let tilt = length([dot(self.u, normal)?, dot(self.v, normal)?, at(0.0)?])?;
        let reach = plus(at(tight::magnitude(off))?, tilt)?;
        Some(tight::magnitude(divide(reach, length(normal)?)?))
    }

    /// An upper bound on every point's distance from `cylinder`: δ + the larger of |A − R| and
    /// |B − R|, A ≥ B the semi-axes of the ellipse's shadow across the axis (module
    /// documentation).
    fn round(&self, cylinder: &Cone) -> Option<f64> {
        let (_, delta) = polar(cylinder.center, cylinder.axis, self.center)?;
        let (x, y) = (cross(self.u, cylinder.axis)?, cross(self.v, cylinder.axis)?);
        let (xx, yy, xy) = (dot(x, x)?, dot(y, y)?, dot(x, y)?);
        let two = at(2.0)?;
        let mean = divide(plus(xx, yy)?, two)?;
        let half = divide(minus(xx, yy)?, two)?;
        let spread = length([half, xy, at(0.0)?])?;
        let long = tight::sqrt(plus(mean, spread)?).ok()?;
        let short = divide(length(cross(x, y)?)?, long)?;
        let r = at(cylinder.radius)?;
        let off = tight::larger(
            tight::magnitude(minus(long, r)?),
            tight::magnitude(minus(short, r)?),
        );
        upper(&[delta, at(off)?])
    }
}

/// Whether grid point `p` lies within `b` steps of `ellipse`'s point at parameter `t`.
pub(crate) fn at_ellipse_end(ellipse: &Ellipse<3>, t: f64, p: P3, b: f64) -> bool {
    Oval::of(ellipse)
        .and_then(|e| e.point(t))
        .zip(grid(p))
        .and_then(|(q, v)| length(sub3(q, v)?))
        .is_some_and(|d| fits(d, b))
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
        let r = plus(at(self.radius)?, times(self.slope, h)?)?;
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

    /// Whether the plane through `origin` with normal `normal` runs along this surface's axis and
    /// is certainly farther from it than the radius and `stray` together, on a cylinder: then no
    /// point within `stray` of the plane is within the radius of the axis, and two faces that
    /// between them stray no more cannot meet.
    ///
    /// A point p of the cylinder is within the radius R of its foot on the axis, and every point
    /// of the axis is at one distance D from a plane along it, so p is at least D − R from the
    /// plane. "Along" is exact: the normal's dot product with the axis must be 0, and the slope
    /// 0, with no width, since a plane that leans by any amount, or a cone, meets the surface
    /// somewhere. D > R + stray is compared squared, ((C − o) · n)² > (R + stray)² · |n|², as two
    /// parallel planes are ([`clear`]), on R + stray's certified upper end, so it is exact on exact
    /// data and a plane at exactly the radius is not apart.
    fn beside(&self, origin: V3, normal: V3, stray: f64) -> bool {
        let far = || {
            let along = dot(normal, self.axis)?;
            let offset = dot(sub3(self.center, origin)?, normal)?;
            let reach = plus(at(self.radius)?, at(stray)?)?.hi();
            Some(zero([along, self.slope, at(0.0)?]) && clear(offset, normal, reach))
        };
        far().unwrap_or(false)
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

/// How two faces' carriers lie, as far as group 3 can certify.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Pair {
    /// Two parallel planes, certainly farther apart than their faces can stray from them, or a
    /// cylinder and a plane along its axis certainly farther from it than the radius and that
    /// stray: the faces cannot meet.
    Apart,
    /// Two planes that certainly cross along a line.
    Crossing,
    /// A plane and a sphere, or a plane certainly perpendicular to a cylinder's or cone's axis:
    /// they meet in one circle at most.
    Circle,
    /// Nothing certified.
    Unknown,
}

/// How the carriers `first` and `second` lie, for two faces that between them may lie `stray`
/// steps off their own carriers: the sum of how far each may.
///
/// Two parallel planes keep their faces apart only when they are more than `stray` apart. A face
/// group 2 has held within b of its plane lies within b of it, every point of it a mix of corners
/// that do, so two such faces can meet wherever their planes are 2b apart or nearer, however the
/// planes are written. For faces that lie on their planes exactly, `stray` is nothing, and any
/// two parallel planes that are not one keep them apart. A cylinder and a plane exactly along its
/// axis keep their faces apart in the same way, when the plane is more than the radius and `stray`
/// from the axis ([`Cone::beside`]).
///
/// Two planes that cross are left unknown, as `Pair::Crossing` is not given here: the one caller,
/// group 3's test of two faces, settles only on apart or a shared circle, and treats crossing and
/// unknown alike, so a separate answer for crossing planes was one no input could tell apart (the
/// mutation gate found it). `geometric::planes_lie` gives it for integer planes, where the planar
/// tests use it.
pub(crate) fn pair(first: &Carrier, second: &Carrier, stray: f64) -> Pair {
    match (first, second) {
        (
            Carrier::Flat { origin, normal },
            Carrier::Flat {
                origin: other,
                normal: across,
            },
        ) => {
            let planes = || {
                let turn = cross(*normal, *across)?;
                let offset = dot(sub3(*other, *origin)?, *normal)?;
                Some((turn, offset))
            };
            if planes().is_some_and(|(turn, offset)| zero(turn) && clear(offset, *normal, stray)) {
                Pair::Apart
            } else {
                Pair::Unknown
            }
        }
        (Carrier::Flat { origin, normal }, Carrier::Round(cone))
        | (Carrier::Round(cone), Carrier::Flat { origin, normal }) => {
            if cross(*normal, cone.axis).is_some_and(zero) {
                Pair::Circle
            } else if cone.beside(*origin, *normal, stray) {
                Pair::Apart
            } else {
                Pair::Unknown
            }
        }
        (Carrier::Flat { .. }, Carrier::Ball { .. })
        | (Carrier::Ball { .. }, Carrier::Flat { .. }) => Pair::Circle,
        _ => Pair::Unknown,
    }
}

/// Whether the segment from grid point `from` to `to` certainly meets `circle` nowhere, or, when
/// `shared` (`from` is a vertex of both), nowhere but at `from`: both ends strictly on one side of
/// the circle's plane; or leaving that plane from the shared vertex; or, lying in it, on a line the
/// circle certainly misses; or crossing it once, at a point certainly off the circle.
pub(crate) fn segment_apart(from: P3, to: P3, shared: bool, circle: &Circle<3>) -> bool {
    let apart = || {
        let (_, [_, _, normal]) = circle.frame().unit_vectors().ok()?;
        let center = lift(circle.center())?;
        let r = at(circle.radius())?;
        let (start, end) = (grid(from)?, grid(to)?);
        let (above, below) = (
            dot(sub3(start, center)?, normal)?,
            dot(sub3(end, center)?, normal)?,
        );
        if same_side(above, below) {
            return Some(true);
        }
        if shared {
            return Some(away(below));
        }
        let along = sub3(end, start)?;
        if zero([above, below, at(0.0)?]) {
            // In the plane: the distance from the centre to the line, against the radius.
            let reach = divide(length(cross(sub3(center, start)?, along)?)?, length(along)?)?;
            return Some(minus(reach, r)?.lo() > 0.0);
        }
        // Through the plane once, at start + t · along with t = above / (above − below).
        let t = divide(above, minus(above, below)?)?;
        let crossing = add3(start, scale(t, along)?)?;
        Some(away(minus(length(sub3(crossing, center)?)?, r)?))
    };
    apart().unwrap_or(false)
}

/// Whether two circles certainly meet nowhere, or, when `shared` (a vertex of both), nowhere but
/// there: in parallel planes, apart or, in one plane, one certainly outside or inside the other;
/// in crossing planes, through their shared vertex and with their centres' feet on the planes'
/// line certainly apart (each circle meets that line at the vertex and at its reflection in the
/// foot of its centre, so the second points differ).
pub(crate) fn circles_apart(first: &Circle<3>, second: &Circle<3>, shared: bool) -> bool {
    let apart = || {
        let (_, [_, _, one]) = first.frame().unit_vectors().ok()?;
        let (_, [_, _, other]) = second.frame().unit_vectors().ok()?;
        let gap = sub3(lift(second.center())?, lift(first.center())?)?;
        let line = cross(one, other)?;
        if zero(line) {
            if away(dot(gap, one)?) {
                return Some(true);
            }
            let (r, s) = (at(first.radius())?, at(second.radius())?);
            let d = length(gap)?;
            let outside = minus(d, plus(r, s)?)?.lo();
            let inside = minus(at(tight::magnitude(minus(r, s)?))?, d)?.lo();
            return Some(tight::larger(outside, inside) > 0.0);
        }
        Some(shared && away(dot(gap, line)?))
    };
    apart().unwrap_or(false)
}

/// The side of the integer plane `plane` that every point of `circle` lies on, if certain: the
/// sign of n · p + d over the whole circle, whose values run over n · c + d ± r·|ẑ × n| (the
/// largest of the affine distance, as the circle's bound on a plane is). It takes the plane its
/// caller's corners are signed against, so the circle's side and theirs are of one plane, its
/// normal the same way round. The coefficients, integers up to 2^125, are enclosed by the doubles
/// either side of their nearest (`crate::flat::around`), so the sign is certified, not exact;
/// `None` where the circle may touch or cross the plane, or a number cannot be enclosed.
///
/// The sign is the enclosure's own ([`Interval::certain_sign`]). Where an end of it is exactly 0,
/// strict or not makes no unsound answer: `around(d)` lies strictly either side of d, so the
/// lower end of n · c + d lies strictly below its value, and the upper end of the reach at or
/// above it. A lower end of 0, at most that lower end less that reach, has every value strictly
/// above it, and an upper end of 0 every value strictly below. Nor is the enclosure ever the
/// point 0, which alone reads as `Sign::Zero`: `around(d)` is never a point, and a sum is at
/// least as wide as each term. (The caller refuses `Sign::Zero` all the same.)
pub(crate) fn circle_side(circle: &Circle<3>, plane: &GridPlane) -> Option<Sign> {
    let (_, [_, _, own]) = circle.frame().unit_vectors().ok()?;
    let [nx, ny, nz] = plane.n();
    let normal = [around(nx)?, around(ny)?, around(nz)?];
    let off = plus(dot(lift(circle.center())?, normal)?, around(plane.d())?)?;
    let reach = times(at(circle.radius())?, length(cross(own, normal)?)?)?;
    plus(off, spread(reach)?)?.certain_sign()
}

/// Whether every number in `a` is certainly not zero.
fn away(a: Interval) -> bool {
    a.lo() * a.hi() > 0.0
}

/// Whether `a` and `b` are certainly not zero and of one sign.
fn same_side(a: Interval, b: Interval) -> bool {
    [a.lo() * a.hi(), b.lo() * b.hi(), a.lo() * b.lo()]
        .into_iter()
        .fold(f64::INFINITY, tight::smaller)
        > 0.0
}

/// Whether two parallel planes are certainly more than `stray` apart, `offset` being (o' − o) · n
/// for a point of each and the normal `normal` of one: ((o' − o) · n)² > stray² · |n|², exact on
/// exact data, as [`flat`] compares a point's distance. A distance that cannot be formed certifies
/// nothing.
fn clear(offset: Interval, normal: V3, stray: f64) -> bool {
    let room = || times(square(at(stray)?)?, dot(normal, normal)?);
    square(offset)
        .zip(room())
        .is_some_and(|(have, room)| have.lo() > room.hi())
}

/// Whether every coordinate of `v` is exactly zero.
fn zero(v: V3) -> bool {
    v.into_iter().map(tight::magnitude).fold(0.0, tight::larger) == 0.0
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
fn slant(slope: Interval) -> Option<Interval> {
    tight::sqrt(plus(at(1.0)?, square(slope)?)?).ok()
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "points of finite doubles are intervals; the test builds them from literals"
)]
mod tests {
    use super::*;

    /// An exact point or vector.
    fn v(x: f64, y: f64, z: f64) -> V3 {
        [x, y, z].map(|c| Interval::point(c).unwrap())
    }

    fn flat_through(origin: V3, normal: V3) -> Carrier {
        Carrier::Flat { origin, normal }
    }

    /// Two parallel planes that do not coincide cannot meet: the faces on them are apart.
    #[test]
    fn parallel_planes_apart_are_apart_and_coinciding_ones_are_not() {
        let floor = flat_through(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0));
        let ceiling = flat_through(v(0.0, 0.0, 5.0), v(0.0, 0.0, 1.0));
        assert!(matches!(pair(&floor, &ceiling, 0.0), Pair::Apart));
        assert!(matches!(pair(&ceiling, &floor, 0.0), Pair::Apart));
        // The same plane again: nothing is certified, the faces may meet anywhere.
        let again = flat_through(v(3.0, 4.0, 0.0), v(0.0, 0.0, 2.0));
        assert!(matches!(pair(&floor, &again, 0.0), Pair::Unknown));
    }

    /// Faces that may stray from two parallel planes can meet unless the planes are farther apart
    /// than the two may stray together.
    #[test]
    fn parallel_planes_are_apart_only_beyond_what_their_faces_may_stray() {
        let floor = flat_through(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0));
        // Five apart, by a normal that is not of unit length, written from either side.
        let ceiling = flat_through(v(7.0, 1.0, 5.0), v(0.0, 0.0, -3.0));
        for (stray, apart) in [
            (0.0, true),
            (4.0, true),
            (4.5, true),
            (5.0, false),
            (6.0, false),
        ] {
            for (first, second) in [(&floor, &ceiling), (&ceiling, &floor)] {
                let found = pair(first, second, stray);
                assert_eq!(matches!(found, Pair::Apart), apart, "{stray}");
                assert_eq!(matches!(found, Pair::Unknown), !apart, "{stray}");
            }
        }
        // A distance that is no number certifies nothing.
        assert!(matches!(pair(&floor, &ceiling, f64::NAN), Pair::Unknown));
    }

    /// Two planes whose normals differ cross along a line, which certifies nothing that keeps
    /// their faces apart: they are left unknown, never apart.
    #[test]
    fn planes_with_different_normals_are_not_apart() {
        let floor = flat_through(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0));
        let wall = flat_through(v(2.0, 0.0, 0.0), v(1.0, 0.0, 0.0));
        assert!(matches!(pair(&floor, &wall, 0.0), Pair::Unknown));
        assert!(matches!(pair(&wall, &floor, 1.0), Pair::Unknown));
        let slant = flat_through(v(0.0, 0.0, 0.0), v(0.0, 1.0, 1.0));
        assert!(matches!(pair(&floor, &slant, 0.0), Pair::Unknown));
    }

    /// A plane's normal that is not certain (an interval across zero) certifies nothing.
    #[test]
    fn planes_with_uncertain_normals_certify_nothing() {
        let spread = Interval::new(-1.0, 1.0).unwrap();
        let zero_i = Interval::point(0.0).unwrap();
        let floor = flat_through(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0));
        let vague = flat_through(
            v(0.0, 0.0, 1.0),
            [spread, zero_i, Interval::point(1.0).unwrap()],
        );
        assert!(matches!(pair(&floor, &vague, 0.0), Pair::Unknown));
    }
}
