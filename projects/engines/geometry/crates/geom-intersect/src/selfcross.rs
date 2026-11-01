//! Surface self-intersection: where one surface crosses itself, for the checker (8.3, group 3) and
//! offsets (13), by Volino and Magnenat-Thalmann's test and a split fallback (12.13).
//!
//! What it does: [`self_intersect`] runs the general search of 12.8 over the surface and itself,
//! from every pair of its cells, and marches what that settles. Besides what the search proves
//! apart, it leaves out a pair of pieces that a clean region holds: the rectangle that bounds them
//! in parameters, or, where the domain closes (12.10), the two strips that reach them glued across
//! the seam. A region is clean when its Gauss map lies in an open hemisphere and its boundary,
//! projected along that hemisphere's axis, is a simple curve. A pair that touches, in parameters,
//! across a seam or at a pole, and is not held clean is split, never settled: its starts would be
//! the points it shares. The search cuts its two sides at different shares (12.8), so the pieces of
//! the two sheets of a crossing never share a cut line inside the domain, and a sweep's crossing,
//! on which the two sheets' parameters along the sweep agree, never runs through a corner both
//! patches share.
//!
//! Guarantees:
//!
//! - **A clean region cannot cross itself.** The proof: let D be a closed disk of parameters made
//!   of rectangles meeting edge to edge (the cells of 12.10 that the region spans), S continuous on
//!   D and C¹ on each rectangle, and π(x) = (b₁·x, b₂·x) a linear map onto the plane. The Jacobian
//!   determinant of π∘S is (b₁ × b₂)·(`S_u` × `S_v`). If it is positive on every rectangle, π∘S is a
//!   local homeomorphism inside each one, and across an edge between two, whose images lie on
//!   either side of the edge's image since both turn one way; where four meet, each image is a
//!   sector of less than a half-turn, so together they wind once. If π∘S is also one to one on
//!   ∂D, π∘S(∂D) is a Jordan curve J, and the number of preimages in D of a point off J is the
//!   degree of π∘S over it, J's winding number about it: 0 or 1. A point of J has its preimage on
//!   ∂D and none inside, or points beside it would have two. So π∘S is one to one on D, and so is
//!   S. A line of ∂D on which S is one point, a pole, is one point of J. This is Volino and
//!   Magnenat-Thalmann's criterion, with the degree argument for local homeomorphisms of a disk.
//! - **Each step of it is proven.** b₁ × b₂ is enclosed from the doubles b₁ and b₂ themselves, and
//!   its product with each rectangle's normal box ([`ParamSurface::normal_bound`], a positive
//!   multiple of `S_u` × `S_v`) proven positive. The boundary is cut into stretches, each proven to move
//!   strictly one way along some direction of the plane (so it does not meet itself); each two in a
//!   row proven to move along one shared direction (so they meet only where they join); and every
//!   two others proven apart, their boxes disjoint. A stretch that will not prove is halved; past
//!   [`STRETCHES`] stretches, or at the floor, the region is not clean.
//! - **Nothing is dropped.** A pair of pieces is proven apart, held clean, settled and marched,
//!   split, or flagged at the floor: a piece not proven clean there is flagged as a pair with a
//!   piece it touches, and where a crossing meets the domain's edge on both sheets at once, as a
//!   sweep's does, no start can be told from the patches' edges and a box a floor across is
//!   flagged, the branch traced up to it.
//! - **Each crossing from both sheets.** The set where S crosses itself, the pairs p ≠ q with
//!   S(p) = S(q), is symmetric: each branch, (u, v) of one sheet then (u, v) of the other, is
//!   reported with its mirror image, (q, p) along it. So a branch whose sheets trade pieces, or
//!   trade places altogether, is followed whole.
//! - **Exact for the analytic kinds.** A plane, cylinder, sphere, cone or torus, or an affine image
//!   of one, over at most a turn (as each kind's constructor requires) is one to one but on its
//!   poles and seams (12.10): it has no crossing to search. A cone's nappes meet only at its apex,
//!   and a spindle or horn torus's sheets only at its poles on the axis: the points
//!   ((R + r·cos v)·e(u), r·sin v) and ((R + r·cos v′)·e(u′), r·sin v′) are one only where
//!   R + r·cos v = 0.
//!
//! Method: chapter 12.13 (`projects/engines/geometry/architecture/12-intersections.md`); P. Volino
//! and N. Magnenat-Thalmann, "Efficient self-collision detection on smoothly discretized surface
//! animations using geometrical shape regularity", Computer Graphics Forum 13(3) (1994),
//! <https://doi.org/10.1111/1467-8659.1330155>; the degree of a map, as in E. Outerelo and J. M.
//! Ruiz, *Mapping Degree Theory*, AMS (2009), <https://doi.org/10.1090/gsm/108>.
//!
//! What this piece leaves: a pole of a surface searched against a piece of the same surface that
//! does not reach it reads as undecided (the pole lies on the surface along the whole line), so
//! such a pair is split to the floor and flagged; the rectangle about the two usually holds them
//! first.
//!
//! Cost: a region's test takes one normal box per cell it spans and a read of each boundary
//! stretch, at most [`STRETCHES`] of them, every two compared, for each pair the search does not
//! prove apart first; the search and march are 12.8's, over both orders of every pair. Recursion:
//! none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.13's: S the surface, D the disk, a the axis, u and v its parameters"
)]

use alloc::vec::Vec;

use geom_curve::tight::{larger, smaller};
use geom_num::interval::Interval;
use geom_num::math;
use geom_run::WorkMeter;
use geom_surface::{ParamBox as Region, ParamSurface, SurfaceKind};

use crate::IntersectError;
use crate::general::{FlaggedPair, Leave, Settled, cross, normals, partials, subdivide_from};
use crate::march::{Branch, march};
use crate::project::{dot, exact, image, positive};
use crate::result::{Test, Unresolved};
use crate::seam::{Charts, cells, charts, cuts};

/// The most stretches a boundary is cut into before it counts as not proven simple.
pub const STRETCHES: usize = 64;

/// Where a surface crosses itself.
#[derive(Clone, Debug, PartialEq)]
pub struct SelfCrossing {
    /// The curves where it does, each vertex (u, v) of one sheet and (u, v) of the other, in the
    /// order [`march`] leaves them.
    pub branches: Vec<Branch>,
    /// Settled pairs of pieces a march step could not be proven in.
    pub unproven: Vec<Settled>,
    /// What could not be decided, among it pairs of pieces that touch and are not proven clean at
    /// the floor.
    pub flagged: Vec<FlaggedPair>,
}

/// One stretch of a region's boundary: the parameter `fixed` (0 for u) held at `at`, the other
/// running over `span`, forward along the boundary or back.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Stretch {
    fixed: usize,
    at: f64,
    span: (f64, f64),
    forward: bool,
}

impl Stretch {
    fn region(self) -> Region {
        if self.fixed == 0 {
            ((self.at, self.at), self.span)
        } else {
            (self.span, (self.at, self.at))
        }
    }

    /// Its two halves, in the boundary's order.
    fn halves(self) -> Option<(Stretch, Stretch)> {
        let (lo, hi) = self.span;
        let m = lo / 2.0 + hi / 2.0;
        if !(lo < m && m < hi) {
            return None;
        }
        let low = Stretch {
            span: (lo, m),
            ..self
        };
        let high = Stretch {
            span: (m, hi),
            ..self
        };
        Some(if self.forward {
            (low, high)
        } else {
            (high, low)
        })
    }
}

/// A stretch along u (`fixed` 1) or v (`fixed` 0).
const fn stretch(fixed: usize, at: f64, span: (f64, f64), forward: bool) -> Stretch {
    Stretch {
        fixed,
        at,
        span,
        forward,
    }
}

/// A rectangle's boundary, counter-clockwise in parameters.
fn rectangle(((a, b), (c, d)): Region) -> Vec<Stretch> {
    alloc::vec![
        stretch(1, c, (a, b), true),
        stretch(0, b, (c, d), true),
        stretch(1, d, (a, b), false),
        stretch(0, a, (c, d), false),
    ]
}

/// The boundary of `low`, which starts at the start of the closing parameter `along`, glued across
/// the seam to `high`, which ends at its end, both over the same span across: one rectangle's loop
/// in a chart where `high` lies just before `low`, the seam left out.
fn glued(low: Region, high: Region, along: usize) -> Vec<Stretch> {
    let (((la, lb), (lc, ld)), ((ha, hb), (hc, hd))) = (low, high);
    if along == 0 {
        alloc::vec![
            stretch(1, lc, (ha, hb), true),
            stretch(1, lc, (la, lb), true),
            stretch(0, lb, (lc, ld), true),
            stretch(1, ld, (la, lb), false),
            stretch(1, ld, (ha, hb), false),
            stretch(0, ha, (lc, ld), false),
        ]
    } else {
        alloc::vec![
            stretch(1, hc, (la, lb), true),
            stretch(0, lb, (hc, hd), true),
            stretch(0, lb, (lc, ld), true),
            stretch(1, ld, (la, lb), false),
            stretch(0, la, (lc, ld), false),
            stretch(0, la, (hc, hd), false),
        ]
    }
}

/// What the surface's own data says of its lines (12.10): the charts, and the cut lines inside the
/// domain in u and in v.
struct Lines {
    charts: Charts,
    cut: (Vec<f64>, Vec<f64>),
    domain: Region,
}

/// `span` cut at the lines of `inner` strictly inside it.
fn cut_span((lo, hi): (f64, f64), inner: &[f64]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut from = lo;
    for &x in inner {
        if lo < x && x < hi {
            out.push((from, x));
            from = x;
        }
    }
    out.push((from, hi));
    out
}

impl Lines {
    /// `region` cut into the parts within one cell each.
    fn parts(&self, ((a, b), (c, d)): Region) -> Vec<Region> {
        let mut out = Vec::new();
        for u in cut_span((a, b), &self.cut.0) {
            for v in cut_span((c, d), &self.cut.1) {
                out.push((u, v));
            }
        }
        out
    }

    /// `s` cut at the lines across it, in the boundary's order; none if it lies on a pole line,
    /// where it is one point.
    fn stretches(&self, s: Stretch) -> Vec<Stretch> {
        let (poles, across) = if s.fixed == 0 {
            (&self.charts.poles.0, &self.cut.1)
        } else {
            (&self.charts.poles.1, &self.cut.0)
        };
        if poles.contains(&s.at) {
            return Vec::new();
        }
        let mut out: Vec<Stretch> = cut_span(s.span, across)
            .into_iter()
            .map(|span| Stretch { span, ..s })
            .collect();
        if !s.forward {
            out.reverse();
        }
        out
    }

    /// Whether a pole line lies strictly inside `region`, where S folds a line to a point.
    fn pole_inside(&self, ((a, b), (c, d)): Region) -> bool {
        let inside = |lines: &[f64], (lo, hi): (f64, f64)| lines.iter().any(|&p| lo < p && p < hi);
        inside(&self.charts.poles.0, (a, b)) || inside(&self.charts.poles.1, (c, d))
    }

    /// Whether two pieces share a point: in parameters, across a closing seam, or on one pole line.
    fn touch(&self, a: Region, b: Region) -> bool {
        let meets = |x: (f64, f64), y: (f64, f64)| x.0 <= y.1 && y.0 <= x.1;
        let ((a0, a1), (a2, a3)) = a;
        let ((b0, b1), (b2, b3)) = b;
        let ((u0, u1), (v0, v1)) = self.domain;
        let [cu, cv] = self.charts.closes;
        let (u, v) = (meets((a0, a1), (b0, b1)), meets((a2, a3), (b2, b3)));
        let across_u = cu && ((a0 == u0 && b1 == u1) || (b0 == u0 && a1 == u1));
        let across_v = cv && ((a2 == v0 && b3 == v1) || (b2 == v0 && a3 == v1));
        let on = |lines: &[f64], x: (f64, f64), y: (f64, f64)| {
            lines
                .iter()
                .any(|&p| x.0 <= p && p <= x.1 && y.0 <= p && p <= y.1)
        };
        ((u || across_u) && (v || across_v))
            || on(&self.charts.poles.0, (a0, a1), (b0, b1))
            || on(&self.charts.poles.1, (a2, a3), (b2, b3))
    }

    /// The regions holding both `a` and `b` to test as one, each with its boundary: the rectangle
    /// bounding them, and, where the domain closes and they reach its two ends, the two strips
    /// glued across the seam.
    fn holding(&self, a: Region, b: Region) -> Vec<(Vec<Region>, Vec<Stretch>)> {
        let ((a0, a1), (a2, a3)) = a;
        let ((b0, b1), (b2, b3)) = b;
        let ((u0, u1), (v0, v1)) = self.domain;
        let [cu, cv] = self.charts.closes;
        let (u, v) = (
            (smaller(a0, b0), larger(a1, b1)),
            (smaller(a2, b2), larger(a3, b3)),
        );
        let mut out = alloc::vec![(alloc::vec![(u, v)], rectangle((u, v)))];
        let mut glue = |low: Region, high: Region, along: usize| {
            out.push((alloc::vec![low, high], glued(low, high, along)));
        };
        if cu {
            if a0 == u0 && b1 == u1 {
                glue(((u0, a1), v), ((b0, u1), v), 0);
            } else if b0 == u0 && a1 == u1 {
                glue(((u0, b1), v), ((a0, u1), v), 0);
            }
        }
        if cv {
            if a2 == v0 && b3 == v1 {
                glue((u, (v0, a3)), (u, (b2, v1)), 1);
            } else if b2 == v0 && a3 == v1 {
                glue((u, (v0, b3)), (u, (a2, v1)), 1);
            }
        }
        out
    }
}

/// A stretch read in the projection: its points' box, and its tangent's along the boundary.
type Seen = (Stretch, [Interval; 2], [Interval; 2]);

/// Two vectors spanning the plane across `a`: a × e for the axis e along which `a` is least, whose
/// coordinates are `a`'s own, and a × that.
fn across(a: [f64; 3]) -> [[f64; 3]; 2] {
    let [x, y, z] = a;
    let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
    let first = if ax <= ay && ax <= az {
        [0.0, z, -y]
    } else if ay <= az {
        [-z, 0.0, x]
    } else {
        [y, -x, 0.0]
    };
    let [p, q, r] = first;
    [first, [y * r - z * q, z * p - x * r, x * q - y * p]]
}

/// `v` mapped by π, `basis` its rows.
fn project(v: &[Interval; 3], basis: &[[Interval; 3]; 2]) -> Result<[Interval; 2], IntersectError> {
    let [e, f] = basis;
    Ok([dot(e, v)?, dot(f, v)?])
}

/// `s` read in the projection, or `None` where the surface cannot be bounded there.
fn see(
    surface: &SurfaceKind,
    s: Stretch,
    basis: &[[Interval; 3]; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Seen>, IntersectError> {
    let Some(found) = partials(surface, s.region(), 1, meter)? else {
        return Ok(None);
    };
    let [at, su, sv, ..] = found.as_slice() else {
        return Err(IntersectError::Internal);
    };
    let along = if s.fixed == 0 { sv } else { su };
    let way = if s.forward {
        *along
    } else {
        along.map(Interval::neg)
    };
    Ok(Some((s, project(at, basis)?, project(&way, basis)?)))
}

/// Whether `t` points strictly one way along `w`.
fn ahead(t: &[Interval; 2], w: [f64; 2]) -> Result<bool, IntersectError> {
    let [x, y] = *t;
    let [p, q] = w;
    let d = x
        .mul(Interval::point(p)?)?
        .add(y.mul(Interval::point(q)?)?)?;
    Ok(positive(d))
}

/// A vector's length, in doubles: it only chooses a direction, which the tests then prove.
fn length(v: &[f64]) -> Result<f64, IntersectError> {
    let mut square = 0.0;
    for x in v {
        square += x * x;
    }
    Ok(math::sqrt(square)?.get())
}

/// The middle of a box of the plane.
fn middle(t: &[Interval; 2]) -> [f64; 2] {
    t.map(|x| x.lo() / 2.0 + x.hi() / 2.0)
}

/// Whether two boxes of the plane are apart.
fn apart(a: &[Interval; 2], b: &[Interval; 2]) -> bool {
    a.iter()
        .zip(b)
        .any(|(x, y)| x.hi() < y.lo() || y.hi() < x.lo())
}

/// The first stretch a boundary's check fails on, to halve: one not moving one way, the wider
/// tangent of two in a row with no shared direction, or the wider box of two others not apart.
fn fault(seen: &[Seen]) -> Result<Option<usize>, IntersectError> {
    let n = seen.len();
    for (i, (_, at, way)) in seen.iter().enumerate() {
        if !ahead(way, middle(way))? {
            return Ok(Some(i));
        }
        let next = if i.saturating_add(1) == n {
            0
        } else {
            i.saturating_add(1)
        };
        let (_, _, then) = seen.get(next).ok_or(IntersectError::Internal)?;
        // Along the bisector of the two middles: their sum, each scaled to the other's length.
        let ([p, q], [r, s]) = (middle(way), middle(then));
        let (l1, l2) = (length(&[p, q])?, length(&[r, s])?);
        let shared = [p * l2 + r * l1, q * l2 + s * l1];
        if !(ahead(way, shared)? && ahead(then, shared)?) {
            return Ok(Some(if image(then) > image(way) { next } else { i }));
        }
        for (k, (_, other, _)) in seen.iter().enumerate().skip(i.saturating_add(2)) {
            // The last and the first are in a row too.
            if i == 0 && k.saturating_add(1) == n {
                continue;
            }
            if !apart(at, other) {
                return Ok(Some(if image(other) > image(at) { k } else { i }));
            }
        }
    }
    Ok(None)
}

/// Whether the region made of `pieces`, bounded by `boundary`, is proven clean (module
/// documentation), to a floor of `floor`.
fn clean(
    surface: &SurfaceKind,
    lines: &Lines,
    (pieces, boundary): (&[Region], &[Stretch]),
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, IntersectError> {
    if pieces.iter().any(|p| lines.pole_inside(*p)) {
        return Ok(false);
    }
    let mut boxes = Vec::new();
    for piece in pieces {
        for part in lines.parts(*piece) {
            boxes.push(normals(surface, part, meter)?);
        }
    }
    // The axis: the middle of the box holding every normal box.
    let mut hull: Option<[(f64, f64); 3]> = None;
    for b in &boxes {
        let mut next = b.map(|x| (x.lo(), x.hi()));
        if let Some(h) = hull {
            for (o, (lo, hi)) in next.iter_mut().zip(h) {
                *o = (smaller(o.0, lo), larger(o.1, hi));
            }
        }
        hull = Some(next);
    }
    let Some(hull) = hull else {
        return Ok(false);
    };
    // Scaled to unit length, so that π is near a rotation and keeps the boundary's corners as wide
    // as they are; the doubles only choose π, whose own b₁ × b₂ is what the test proves against.
    let middle = hull.map(|(lo, hi)| lo / 2.0 + hi / 2.0);
    let size = length(&middle)?;
    if size.is_nan() || size <= 0.0 {
        return Ok(false);
    }
    let [e, f] = across(middle.map(|x| x / size));
    let basis = [exact(&e)?, exact(&f)?];
    let [eb, fb] = &basis;
    let w = cross(eb, fb)?;
    for b in &boxes {
        if !positive(dot(b, &w)?) {
            return Ok(false);
        }
    }
    let mut seen: Vec<Seen> = Vec::new();
    for s in boundary.iter().flat_map(|s| lines.stretches(*s)) {
        let Some(read) = see(surface, s, &basis, meter)? else {
            return Ok(false);
        };
        seen.push(read);
    }
    loop {
        meter.charge(1)?;
        let Some(i) = fault(&seen)? else {
            return Ok(true);
        };
        let Some((s, at, _)) = seen.get(i).copied() else {
            return Err(IntersectError::Internal);
        };
        if seen.len() >= STRETCHES || image(&at) <= floor {
            return Ok(false);
        }
        let Some((p, q)) = s.halves() else {
            return Ok(false);
        };
        let (Some(p), Some(q)) = (
            see(surface, p, &basis, meter)?,
            see(surface, q, &basis, meter)?,
        ) else {
            return Ok(false);
        };
        seen.splice(i..=i, [p, q]);
    }
}

/// Whether the kind is one to one but on its poles and seams (module documentation).
const fn embedded(surface: &SurfaceKind) -> bool {
    matches!(
        surface,
        SurfaceKind::Plane(_)
            | SurfaceKind::Cylinder(_)
            | SurfaceKind::Cone(_)
            | SurfaceKind::Sphere(_)
            | SurfaceKind::Torus(_)
            | SurfaceKind::Affine(_)
    )
}

/// What the search over a surface and itself leaves out: a pair of pieces a clean region holds,
/// and, never settled, a pair that touches.
struct Clean<'a> {
    surface: &'a SurfaceKind,
    lines: Lines,
    floor: f64,
}

impl Leave for Clean<'_> {
    fn holds(
        &self,
        _: &SurfaceKind,
        (a, b): (Region, Region),
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, IntersectError> {
        for (pieces, boundary) in self.lines.holding(a, b) {
            if clean(
                self.surface,
                &self.lines,
                (&pieces, &boundary),
                self.floor,
                meter,
            )? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn splits_undecided(&self) -> bool {
        true
    }

    fn near(&self, (a, b): (Region, Region)) -> Result<Option<(bool, Unresolved)>, IntersectError> {
        Ok(self.lines.touch(a, b).then_some((
            true,
            Unresolved::UndecidedAtFloor {
                test: Test::Exclusion,
            },
        )))
    }
}

/// Where `surface` crosses itself (module documentation), to a floor of `floor` in space, every
/// branch within `ceiling` of its chords.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a floor or ceiling that is not positive;
/// [`IntersectError::BudgetExceeded`] and [`IntersectError::Cancelled`];
/// [`IntersectError::UnprovenMaths`] where a kind's bound needs one.
pub fn self_intersect(
    surface: &SurfaceKind,
    floor: f64,
    ceiling: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<SelfCrossing, IntersectError> {
    if floor.is_nan() || floor <= 0.0 || ceiling.is_nan() || ceiling <= 0.0 {
        return Err(IntersectError::InvalidInput);
    }
    if embedded(surface) {
        return Ok(SelfCrossing {
            branches: Vec::new(),
            unproven: Vec::new(),
            flagged: Vec::new(),
        });
    }
    let seams = charts(surface);
    let lines = Lines {
        cut: cuts(&seams, surface),
        domain: surface.domain(),
        charts: seams,
    };
    let whole = cells(surface, &lines.charts);
    let mut start = Vec::new();
    for a in &whole {
        for b in &whole {
            start.push((*a, *b));
        }
    }
    let leave = Clean {
        surface,
        lines,
        floor,
    };
    let found = subdivide_from(surface, surface, (start, floor), &leave, meter)?;
    let traced = march(surface, surface, &found, floor, ceiling, meter)?;
    Ok(SelfCrossing {
        branches: traced.branches,
        unproven: traced.unproven,
        flagged: found.flagged,
    })
}
