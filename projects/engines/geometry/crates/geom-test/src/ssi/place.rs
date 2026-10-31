//! Places a corpus case on the grid, and checks that rounding has not changed its truth (chapter
//! 12.17; piece G160).
//!
//! What it does: every coordinate the kernel stores is a whole number of grid steps (6.1), and the
//! corpus's are doubles in units of their own. [`place`] maps a case by 2^k steps to the unit, which
//! is exact in doubles, and rounds each point and length to a whole step. Directions, angles, knots
//! and weights are not grid data and stay as they are. It tries k upward from the first scale at
//! which the tolerance is at least ε_fit, and takes the smallest that passes [`kept`]'s checks, so
//! a case is never scaled further than it needs: thresholds in steps stay as small as they can.
//!
//! Guarantees: what [`kept`] holds, for the case's two surfaces together:
//!
//! - **ε_fit**: the case's tolerance is at least [`FIT_STEPS`] steps at this scale.
//! - **The range**: every coordinate is inside the grid's range.
//! - **The truth is still on the surfaces**: every truth sample is within the tolerance of each
//!   analytic surface as placed, by its closed form. A NURBS surface cannot move further than its
//!   furthest control point did, since a point on it is a convex combination of its net with the
//!   same weights; that bound is the surface's `moved`.
//! - **The answer did not move past the tolerance**: where a case states how far a residual moves
//!   the answer, `moved` × that factor is within the tolerance.
//! - **Small features survive**: `moved` is at most a quarter of a stated feature size.
//! - **A contact did not spread past the tolerance**: where surfaces touch, a change of δ moves the
//!   contact by about √(2δ/Δκ) (12.9). The harness has no Δκ for a NURBS pair, so it takes the
//!   flattest curvature the case's size allows, 1/L, and asks √(2δL) to be within the tolerance.
//!   Contacts of higher order are named in [`HIGHER_ORDER`], and the same bound is taken at their
//!   order. A case whose data is exact on the grid moves nothing and passes at once.
//!
//! Method: chapter 12.17's adapter
//! (`projects/engines/geometry/architecture/12-intersections.md`); the band formula of 12.9; the
//! convex hull property of NURBS (The NURBS Book, P4.2 and P4.11).
//!
//! Cost: linear in the case's samples and control points, for each scale tried; at most 40 scales.

use core::f64::consts::PI;

use geom_curve::frame::Frame;
use geom_nurbs::{Basis, Limit};
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind, Torus};

use super::case::{Case, Contact, Implicit, Surface, V3, along, cross, norm, sub, unit};
use super::{SsiError, cos_deg, larger, sin_deg};

/// ε_fit in steps: `geom_num::ladder::FIT`.
pub const FIT_STEPS: f64 = 64.0;
const _: () = assert!(geom_num::ladder::FIT.0 == 64);

/// The grid's range in steps: `geom_num::grid::COORD_MAX`.
pub const RANGE: f64 = 1_099_511_627_775.0;
const _: () = assert!(geom_num::grid::COORD_MAX == 1_099_511_627_775);

/// Contacts of higher order than two, with their order. `c2n_quartic_contact_parabola`'s surfaces
/// separate as w⁴ across the contact, and `y23_fig1f_high_order_contact`'s as t³; the corpus's
/// notes on each state it.
pub const HIGHER_ORDER: [(&str, u32); 2] = [
    ("c2n_quartic_contact_parabola", 4),
    ("y23_fig1f_high_order_contact", 3),
];

/// 2^`k`, exactly.
fn two(k: u32) -> f64 {
    f64::from_bits((1023 + u64::from(k)) << 52)
}

/// `x` steps, rounded to a whole step ties to even, and how far that moved it.
fn step(x: f64, scale: f64) -> (f64, f64) {
    let exact = x * scale;
    let whole = exact.round_ties_even();
    (whole, (whole - exact).abs())
}

/// `p` in steps, rounded, and how far that moved it.
fn point(p: V3, scale: f64) -> (V3, f64) {
    let exact = p.map(|x| x * scale);
    let whole = exact.map(f64::round_ties_even);
    (whole, norm(sub(whole, exact)))
}

/// `r` scaled, not rounded: a parameter range, whose ends are not grid data.
fn range(r: [f64; 2], scale: f64) -> [f64; 2] {
    r.map(|x| x * scale)
}

/// The largest absolute coordinate of a point.
fn reach(p: V3) -> f64 {
    p.iter().fold(0.0, |m, x| larger(m, x.abs()))
}

/// How far from the origin `surface` can reach, along any axis.
fn extent(surface: &Surface) -> f64 {
    match surface {
        Surface::Plane {
            origin,
            xaxis,
            yaxis,
            u,
            v,
        } => {
            let mut most = 0.0;
            for a in u {
                for b in v {
                    most = larger(most, reach(along(along(*origin, *a, *xaxis), *b, *yaxis)));
                }
            }
            most
        }
        Surface::Sphere { center, radius, .. } => reach(*center) + radius,
        Surface::Cylinder {
            origin,
            radius,
            height,
            ..
        } => reach(*origin) + radius + larger(height[0].abs(), height[1].abs()),
        Surface::Cone {
            apex,
            half_angle_deg,
            height,
            ..
        } => {
            let tall = larger(height[0].abs(), height[1].abs());
            reach(*apex) + tall * (1.0 + sin_deg(*half_angle_deg) / cos_deg(*half_angle_deg))
        }
        Surface::Torus {
            center, big, small, ..
        } => reach(*center) + big + small,
        Surface::Nurbs { points, .. } => points.iter().fold(0.0, |m, p| larger(m, reach(*p))),
    }
}

/// `surface` on the grid, and how far it moved at most, in steps.
fn rounded(surface: &Surface, scale: f64) -> (Surface, f64) {
    match surface {
        Surface::Plane {
            origin,
            xaxis,
            yaxis,
            u,
            v,
        } => {
            let (origin, moved) = point(*origin, scale);
            (
                Surface::Plane {
                    origin,
                    xaxis: *xaxis,
                    yaxis: *yaxis,
                    u: range(*u, scale),
                    v: range(*v, scale),
                },
                moved,
            )
        }
        Surface::Sphere {
            center,
            xaxis,
            yaxis,
            radius,
            lon,
            lat,
        } => {
            let (center, shift) = point(*center, scale);
            let (radius, grown) = step(*radius, scale);
            (
                Surface::Sphere {
                    center,
                    xaxis: *xaxis,
                    yaxis: *yaxis,
                    radius,
                    lon: *lon,
                    lat: *lat,
                },
                shift + grown,
            )
        }
        Surface::Cylinder {
            origin,
            xaxis,
            yaxis,
            radius,
            angle,
            height,
        } => {
            let (origin, shift) = point(*origin, scale);
            let (radius, grown) = step(*radius, scale);
            (
                Surface::Cylinder {
                    origin,
                    xaxis: *xaxis,
                    yaxis: *yaxis,
                    radius,
                    angle: *angle,
                    height: range(*height, scale),
                },
                shift + grown,
            )
        }
        Surface::Cone {
            apex,
            xaxis,
            yaxis,
            half_angle_deg,
            height,
        } => {
            let (apex, moved) = point(*apex, scale);
            (
                Surface::Cone {
                    apex,
                    xaxis: *xaxis,
                    yaxis: *yaxis,
                    half_angle_deg: *half_angle_deg,
                    height: range(*height, scale),
                },
                moved,
            )
        }
        Surface::Torus {
            center,
            xaxis,
            yaxis,
            big,
            small,
            major,
            minor,
        } => {
            let (center, shift) = point(*center, scale);
            let (big, wide) = step(*big, scale);
            let (small, thin) = step(*small, scale);
            (
                Surface::Torus {
                    center,
                    xaxis: *xaxis,
                    yaxis: *yaxis,
                    big,
                    small,
                    major: *major,
                    minor: *minor,
                },
                shift + wide + thin,
            )
        }
        Surface::Nurbs {
            label,
            degree,
            knots_u,
            knots_v,
            nu,
            nv,
            points,
            weights,
        } => {
            let mut moved = 0.0;
            let net = points
                .iter()
                .map(|p| {
                    let (whole, shift) = point(*p, scale);
                    moved = larger(moved, shift);
                    whole
                })
                .collect();
            (
                Surface::Nurbs {
                    label: label.clone(),
                    degree: *degree,
                    knots_u: knots_u.clone(),
                    knots_v: knots_v.clone(),
                    nu: *nu,
                    nv: *nv,
                    points: net,
                    weights: weights.clone(),
                },
                moved,
            )
        }
    }
}

/// The closed form of an analytic `surface`; a NURBS surface has none here.
fn implicit(surface: &Surface) -> Option<Implicit> {
    Some(match surface {
        Surface::Plane {
            origin,
            xaxis,
            yaxis,
            ..
        } => Implicit::Plane {
            point: *origin,
            normal: cross(*xaxis, *yaxis),
        },
        Surface::Sphere { center, radius, .. } => Implicit::Sphere {
            center: *center,
            radius: *radius,
        },
        Surface::Cylinder {
            origin,
            xaxis,
            yaxis,
            radius,
            ..
        } => Implicit::Cylinder {
            point: *origin,
            axis: cross(*xaxis, *yaxis),
            radius: *radius,
        },
        Surface::Cone {
            apex,
            xaxis,
            yaxis,
            half_angle_deg,
            ..
        } => Implicit::Cone {
            point: *apex,
            axis: cross(*xaxis, *yaxis),
            half_angle_deg: *half_angle_deg,
        },
        Surface::Torus {
            center,
            xaxis,
            yaxis,
            big,
            small,
            ..
        } => Implicit::Torus {
            point: *center,
            axis: cross(*xaxis, *yaxis),
            big: *big,
            small: *small,
        },
        Surface::Nurbs { .. } => return None,
    })
}

/// A case on the grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The scale: 2^`log2` steps to the case's unit.
    pub log2: u32,
    /// The two surfaces, in steps.
    pub surfaces: [Surface; 2],
    /// How far each surface moved at most, in steps.
    pub moved: [f64; 2],
    /// The case's reach, in steps: its L.
    pub extent: f64,
    /// The order of the case's highest contact.
    pub order: u32,
}

impl Placed {
    /// Steps to the case's unit.
    #[must_use]
    pub fn scale(&self) -> f64 {
        two(self.log2)
    }

    /// How far the two surfaces moved together, in steps.
    #[must_use]
    pub fn moved(&self) -> f64 {
        self.moved[0] + self.moved[1]
    }

    /// `p`, in steps.
    #[must_use]
    pub fn steps(&self, p: V3) -> V3 {
        p.map(|x| x * self.scale())
    }

    /// `p`, in steps, back in the case's units.
    #[must_use]
    pub fn units(&self, p: V3) -> V3 {
        p.map(|x| x / self.scale())
    }

    /// `p` at the nearest grid point, in the case's units.
    #[must_use]
    pub fn snapped(&self, p: V3) -> V3 {
        self.units(point(p, self.scale()).0)
    }

    /// The surfaces as the kernel's kinds, in steps.
    ///
    /// # Errors
    ///
    /// [`SsiError::Kernel`] for a surface the kernel refuses.
    pub fn kinds(&self, id: &str) -> Result<[SurfaceKind; 2], SsiError> {
        let [a, b] = &self.surfaces;
        Ok([kind(a, id)?, kind(b, id)?])
    }
}

/// `deg` degrees in radians.
fn radians(deg: f64) -> f64 {
    deg / 180.0 * PI
}

/// `surface`, as the kernel's kind.
fn kind(surface: &Surface, id: &str) -> Result<SurfaceKind, SsiError> {
    let refused = |e: &dyn core::fmt::Debug| SsiError::Kernel {
        id: id.to_owned(),
        why: format!("{e:?}"),
    };
    let frame = |x: V3, y: V3| Frame::new(x, cross(x, y)).map_err(|e| refused(&e));
    let turn = |r: [f64; 2]| (radians(r[0]), radians(r[1]));
    Ok(match surface {
        Surface::Plane {
            origin,
            xaxis,
            yaxis,
            u,
            v,
        } => SurfaceKind::Plane(
            Plane::new(*origin, *xaxis, *yaxis, ((u[0], u[1]), (v[0], v[1])))
                .map_err(|e| refused(&e))?,
        ),
        Surface::Sphere {
            center,
            xaxis,
            yaxis,
            radius,
            lon,
            lat,
        } => SurfaceKind::Sphere(
            Sphere::new(*center, *radius, frame(*xaxis, *yaxis)?, (turn(*lon), turn(*lat)))
                .map_err(|e| refused(&e))?,
        ),
        Surface::Cylinder {
            origin,
            xaxis,
            yaxis,
            radius,
            angle,
            height,
        } => SurfaceKind::Cylinder(
            Cylinder::new(
                *origin,
                *radius,
                frame(*xaxis, *yaxis)?,
                (turn(*angle), (height[0], height[1])),
            )
            .map_err(|e| refused(&e))?,
        ),
        Surface::Cone {
            apex,
            xaxis,
            yaxis,
            half_angle_deg,
            height,
        } => SurfaceKind::Cone(
            Cone::new(
                *apex,
                0.0,
                sin_deg(*half_angle_deg) / cos_deg(*half_angle_deg),
                frame(*xaxis, *yaxis)?,
                (turn([0.0, 360.0]), (height[0], height[1])),
            )
            .map_err(|e| refused(&e))?,
        ),
        Surface::Torus {
            center,
            xaxis,
            yaxis,
            big,
            small,
            major,
            minor,
        } => SurfaceKind::Torus(
            Torus::new(
                *center,
                *big,
                *small,
                frame(*xaxis, *yaxis)?,
                (turn(*major), turn(*minor)),
            )
            .map_err(|e| refused(&e))?,
        ),
        Surface::Nurbs {
            degree,
            knots_u,
            knots_v,
            points,
            weights,
            ..
        } => {
            let u = Basis::new(degree[0], knots_u.clone()).map_err(|e| refused(&e))?;
            let v = Basis::new(degree[1], knots_v.clone()).map_err(|e| refused(&e))?;
            SurfaceKind::Nurbs(
                geom_nurbs::Surface::new_rational(u, v, points, weights)
                    .map_err(|e| refused(&e))?,
            )
        }
    })
}

/// The case's order of contact, 2 unless [`HIGHER_ORDER`] names it.
fn order_of(id: &str) -> u32 {
    HIGHER_ORDER
        .iter()
        .find(|(name, _)| *name == id)
        .map_or(2, |&(_, order)| order)
}

/// Whether a band of half-width `width` holds a contact of order `order`, moved by `moved`, at
/// curvature scale `extent`: whether `width`ⁿ ≥ n!·`moved`·`extent`ⁿ⁻¹, the bound of 12.9 at n = 2.
#[must_use]
pub fn holds_contact(width: f64, moved: f64, extent: f64, order: u32) -> bool {
    let mut left = 1.0;
    let mut right = moved;
    for n in 1..=order {
        left *= width;
        right *= f64::from(n);
        if n < order {
            right *= extent;
        }
    }
    left >= right
}

/// Why rounding at `scale` changes `case`'s truth, or `None` if it does not.
fn kept(case: &Case, placed: &Placed, tolerance: f64) -> Option<String> {
    let scale = placed.scale();
    let allowed = case.cov_tol * scale;
    if tolerance * scale < FIT_STEPS {
        return Some(format!("{tolerance:e} is under ε_fit"));
    }
    let moved = placed.moved();
    let closed: Vec<Implicit> = placed.surfaces.iter().filter_map(implicit).collect();
    for feature in case.features() {
        for p in feature.points {
            let p = placed.steps(*p);
            for surface in &closed {
                let off = surface.distance(p).unwrap_or(0.0);
                if off > allowed {
                    return Some(format!("a truth sample lies {off} steps off a surface"));
                }
            }
        }
    }
    if let Some(ill) = case.ill_cond {
        if moved * ill > allowed {
            return Some(format!("moving {moved} steps moves the answer past {allowed}"));
        }
    }
    if let Some(size) = case.feature_size {
        if moved * 4.0 > size * scale {
            return Some(format!("moving {moved} steps is too large for a feature of {size}"));
        }
    }
    let touching = case.features().iter().any(|f| {
        matches!(
            f.contact,
            Contact::Tangent | Contact::NearTangent | Contact::Isolated | Contact::Singular
        )
    });
    if touching && !holds_contact(allowed, moved, placed.extent, placed.order) {
        return Some(format!("moving {moved} steps spreads a contact past {allowed}"));
    }
    None
}

/// `case` on the grid, at the smallest scale that keeps its truth. `accuracy` is how close to the
/// surfaces the run under test must stay, if it is stricter than the case's tolerance.
///
/// # Errors
///
/// [`SsiError::Unplaced`] if no scale in the grid's range keeps it.
pub fn place(case: &Case, accuracy: Option<f64>) -> Result<Placed, SsiError> {
    let tolerance = accuracy.map_or(case.cov_tol, |a| super::smaller(a, case.cov_tol));
    let [a, b] = &case.surfaces;
    let far = larger(extent(a), extent(b));
    let unplaced = |why: String| SsiError::Unplaced {
        id: case.id.clone(),
        why,
    };
    let mut k = 0;
    while tolerance * two(k) < FIT_STEPS {
        k += 1;
    }
    let mut last = format!("a reach of {far} leaves no scale for a tolerance of {tolerance:e}");
    while far * two(k) <= RANGE {
        let scale = two(k);
        let (first, moved_a) = rounded(a, scale);
        let (second, moved_b) = rounded(b, scale);
        let placed = Placed {
            log2: k,
            surfaces: [first, second],
            moved: [moved_a, moved_b],
            extent: far * scale,
            order: order_of(&case.id),
        };
        match kept(case, &placed, tolerance) {
            None => return Ok(placed),
            Some(why) => last = why,
        }
        k += 1;
    }
    Err(unplaced(last))
}

/// How far `case`'s kinds, as the kernel evaluates them on a grid of parameters, lie from the
/// case's closed forms, in the case's units; a surface with no closed form here counts as zero.
///
/// # Errors
///
/// [`SsiError::Kernel`] for a surface the kernel refuses or cannot evaluate.
pub fn disagreement(case: &Case, placed: &Placed) -> Result<f64, SsiError> {
    const SIDE: u32 = 9;
    let kinds = placed.kinds(&case.id)?;
    let mut most: f64 = 0.0;
    for (kind, truth) in kinds.iter().zip(&case.truth) {
        let ((u0, u1), (v0, v1)) = kind.domain();
        for i in 0..SIDE {
            for j in 0..SIDE {
                let along_u = f64::from(i) / f64::from(SIDE - 1);
                let along_v = f64::from(j) / f64::from(SIDE - 1);
                let u = u0 + (u1 - u0) * along_u;
                let v = v0 + (v1 - v0) * along_v;
                let sides = (Limit::FromRight, Limit::FromRight);
                let p = kind.point(u, v, sides).map_err(|e| SsiError::Kernel {
                    id: case.id.clone(),
                    why: format!("{e:?}"),
                })?;
                most = larger(most, truth.distance(placed.units(p)).unwrap_or(0.0));
            }
        }
    }
    Ok(most)
}

/// The unit normal the case's frame gives `x` and `y`, for a catalogue's truth.
#[must_use]
pub fn axis_of(x: V3, y: V3) -> V3 {
    unit(cross(x, y))
}
