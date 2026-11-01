//! Seams, poles and knot lines: where a surface's parameterisation stops being regular, and how a
//! branch passes them (12.10).
//!
//! What it does: [`charts`] reads, from a kind's own stored data and exactly, which ends of its
//! domain are one curve (a whole turn of a round kind, a NURBS whose first and last rows of control
//! points are one row), which lines of its domain are each one point of space (a sphere's poles, a
//! cone's apex, a NURBS edge whose control points are one point), and the lines inside where it is
//! not proven tangent-continuous (a NURBS knot of full multiplicity). [`crate::general::subdivide`]
//! cuts each domain at its creases and inner poles before anything else ([`cuts`]), and tests a
//! patch edge on a pole line as the one point it is: the point against the other surface (12.4),
//! then the directions a branch leaves it by. [`crate::march`] and [`crate::level`] join a branch
//! across a closing seam, through a pole and across a knot line, recording the point once and
//! saying where in [`Pass`].
//!
//! Guarantees:
//!
//! - **Exact, or not claimed.** Ends close only when the data proves them one curve: a span of
//!   exactly two half-turns (an error-free sum), or control rows of identical doubles and weights
//!   on a clamped basis. A line is a pole only when the data makes it one point: v exactly ±1/2 on
//!   a sphere; r₀ + k·v exactly 0 on a cone (an error-free product); R + r·cos πv exactly 0 at a v
//!   whose cosine is exact (a multiple of a half-turn) on a torus; identical control points along a
//!   clamped NURBS edge (`geom_nurbs::Surface::collapsed_edges`). Anything else is an ordinary edge,
//!   and a branch that ends there ends at the boundary: a join is never made that is not proven.
//! - **One point, recorded once.** Two ends are joined across a seam only when one sits on each end
//!   of a closing parameter, exactly (a start on an edge holds the edge's value as a point), and
//!   their other parameters' enclosures meet: both are then roots of one edge system, the same
//!   standard [`crate::march`] joins pairs by. Ends held on one pole line are its one point
//!   whatever the line's free parameter. The joined branch keeps one of the enclosures.
//! - **A pole joins two, or none.** Where exactly two half-branches end at a pole, the curve goes
//!   through it as one branch (a sphere's meridians, a cone's two generators in a plane through its
//!   apex, a turn there allowed). Any other number makes it a singular point, where each ends
//!   ([`crate::result::CurveEnd::Singular`]), as 12.8's step 8 asks.
//! - **A pole is decided as a point.** It is proven apart from the other surface, or proven within
//!   the floor of it at a foot in the pair; otherwise the pair is undecided. A pole within the floor
//!   is treated as on it, the same resolution floor every other test stops at (12.3).
//!
//! Method: chapter 12.10 (`projects/engines/geometry/architecture/12-intersections.md`); each
//! kind's closed form in `geom_surface` (its table of S(u, v)); knot multiplicity and continuity
//! from L. Piegl and W. Tiller, *The NURBS Book*, 2nd ed., section 2.2: a knot of multiplicity k
//! leaves a degree-p basis C^(p−k) there, so k ≥ p is at most C⁰
//! (<https://doi.org/10.1007/978-3-642-59223-2>). The directions out of a pole are 12.10's
//! division of the vanishing factor: on a pole line the partial along the line is zero, so a step
//! off it moves along the partial across, and a branch leaves along those of its values that lie
//! in the other surface's tangent plane. This is our derivation; the engine of [`crate::solve`]
//! proves each such value once.
//!
//! What it leaves: the facts of a swept or extruded profile (closed profiles, a profile meeting
//! its axis) and the poles of an affine image or an offset are not read here, so their seams and
//! poles stay ordinary edges until a kind says them. A tangent contact at a pole (a plane touching
//! a sphere's pole) stays undecided: 12.9's analysis reads forms in the pole's own chart, where
//! they vanish, and needs a regular chart there. A spindle torus's axis points fall at no double,
//! so its cut is not made. Extraordinary vertices are G320's.
//!
//! Cost: constant for the analytic kinds; one pass over the control net and the knots for a NURBS.
//! A pole line in a pair costs one projection to an eighth of the floor and one run of the engine
//! along the line. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.10's: u and v one surface's parameters, s and r the other's, t along a line"
)]

use alloc::vec::Vec;

use geom_num::eft::{two_prod, two_sum};
use geom_num::interval::Interval;
use geom_num::math::{ceil, floor};
use geom_nurbs::{Basis, Edge};
use geom_run::WorkMeter;
use geom_surface::{ParamBox as Region, ParamSurface, SurfaceKind};

use crate::IntersectError;
use crate::general::{ends, normals, partials, point, span};
use crate::project::{Projection, dot, image, project_surface};
use crate::solve::{Field, solve};

/// What a branch passes at one of its vertices (12.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Passing {
    /// A seam where the domain closes: on one side of the vertex the closing parameter is read
    /// near one end of the domain, on the other side near the other.
    Seam,
    /// A knot line where the surface is not proven tangent-continuous: the branch may turn there.
    KnotLine,
    /// A pole, where a whole line of the domain is one point: the branch leaves along another
    /// value of the line's free parameter than it came in by.
    Pole,
}

/// Where a branch passes a seam, a knot line or a pole. The vertex is the point passed, recorded
/// once, with its parameters as one side reads them; the other side reads a seam's parameter at
/// the domain's other end, and a pole's free parameter at another value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pass {
    /// The vertex's index in the branch.
    pub at: usize,
    /// What it passes.
    pub passing: Passing,
}

/// What a surface's own data proves about where its parameterisation is not regular (12.10).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Charts {
    /// Whether the domain's two ends in u, then in v, are one curve point by point:
    /// S(u₀, v) = S(u₁, v) for every v (then likewise in v).
    pub closes: [bool; 2],
    /// Lines of constant u, then of constant v, on each of which the surface is one point: the
    /// domain's own ends, or lines inside it.
    pub poles: (Vec<f64>, Vec<f64>),
    /// Lines inside the domain, in u then in v, where the surface is not proven
    /// tangent-continuous.
    pub creases: (Vec<f64>, Vec<f64>),
}

/// Whether `(a, b)` is exactly one turn: b − a = 2 half-turns, decided by an error-free sum.
fn turn((a, b): (f64, f64)) -> bool {
    let (s, e) = two_sum(b, -a);
    s == 2.0 && e == 0.0
}

/// Whether r₀ + k·v is exactly zero. It can only be if k·v is a double, so the product's error is
/// zero and its double is −r₀.
fn apex(r0: f64, k: f64, v: f64) -> bool {
    let (p, e) = two_prod(k, v);
    e == 0.0 && p == -r0
}

/// The cosine of π·v where it is exact, at a multiple of a half-turn: 1, 0, −1 or 0.
fn exact_cospi(v: f64) -> Option<f64> {
    // v·2 is exact; it is a whole number exactly when its floor is itself.
    let twice = v * 2.0;
    if floor(twice).ok()?.get() != twice {
        return None;
    }
    // Half-turns past the last whole turn below: 0, 1, 2 or 3, each exact (Sterbenz).
    let quarter = twice - floor(twice / 4.0).ok()?.get() * 4.0;
    Some(if quarter == 0.0 {
        1.0
    } else if quarter == 2.0 {
        -1.0
    } else {
        0.0
    })
}

/// The lines of v in `[v₀, v₁]` at a multiple of a half-turn where R + r·cos πv is exactly zero.
fn torus_poles(big: f64, small: f64, (v0, v1): (f64, f64)) -> Vec<f64> {
    let mut out = Vec::new();
    let (Ok(first), Ok(last)) = (ceil(v0 * 2.0), floor(v1 * 2.0)) else {
        return out;
    };
    let mut k = first.get();
    // A torus's v spans at most one turn, so at most five half-turns are visited.
    while k <= last.get() {
        let v = k / 2.0;
        // R + r·c with c in {1, 0, −1}: r·c is r, 0 or −r, so the test is exact.
        if exact_cospi(v).is_some_and(|c| big == -(small * c)) {
            out.push(v);
        }
        k += 1.0;
    }
    out
}

/// The interior knots of `basis` where it is at most C⁰: multiplicity at least the degree.
fn crease_knots(basis: &Basis) -> Vec<f64> {
    let (lo, hi) = basis.domain();
    let p = basis.degree();
    let mut out: Vec<f64> = basis
        .knots()
        .iter()
        .copied()
        .filter(|&k| lo < k && k < hi && basis.multiplicity(k) >= p)
        .collect();
    out.dedup();
    out
}

/// What `surface`'s own data proves about its seams, poles and knot lines (module documentation).
#[must_use]
pub fn charts(surface: &SurfaceKind) -> Charts {
    let ((u0, u1), (v0, v1)) = surface.domain();
    let mut out = Charts::default();
    match surface {
        SurfaceKind::Cylinder(_) | SurfaceKind::Revolution(_) => {
            out.closes = [turn((u0, u1)), false];
        }
        SurfaceKind::Cone(c) => {
            out.closes = [turn((u0, u1)), false];
            let (r0, k) = c.radius_and_slope();
            for v in [v0, v1] {
                if apex(r0, k, v) {
                    out.poles.1.push(v);
                }
            }
            // An apex inside the domain, when it sits on a double: −r₀/k rounded, then checked.
            let inner = -r0 / k;
            if v0 < inner && inner < v1 && apex(r0, k, inner) {
                out.poles.1.push(inner);
                out.poles.1.sort_by(f64::total_cmp);
            }
        }
        SurfaceKind::Sphere(_) => {
            out.closes = [turn((u0, u1)), false];
            for (v, at) in [(v0, -0.5), (v1, 0.5)] {
                if v == at {
                    out.poles.1.push(v);
                }
            }
        }
        SurfaceKind::Torus(t) => {
            out.closes = [turn((u0, u1)), turn((v0, v1))];
            let (big, small) = t.radii();
            out.poles.1 = torus_poles(big, small, (v0, v1));
        }
        SurfaceKind::Affine(a) => {
            let (au, av) = a.angular();
            out.closes = [au && turn((u0, u1)), av && turn((v0, v1))];
        }
        SurfaceKind::Offset(o) => {
            // An offset closes where its base does and the base's normal is the same on both
            // ends: true of the round kinds, which are smooth across their seams, but not proven
            // for a NURBS, whose closing rows may meet at a crease.
            let base = charts(o.base());
            if !matches!(o.base(), SurfaceKind::Nurbs(_) | SurfaceKind::Offset(_)) {
                out.closes = base.closes;
            }
            out.creases = base.creases;
        }
        SurfaceKind::Nurbs(s) => {
            let (bu, bv) = (s.basis_u(), s.basis_v());
            let (rows, cols) = (bu.count(), bv.count());
            let pts = s.homogeneous();
            let w = s.weights();
            let at = |i: usize, j: usize| -> Option<([f64; 3], f64)> {
                let k = i.checked_mul(cols)?.checked_add(j)?;
                Some((*pts.get(k)?, w.map_or(Some(1.0), |ws| ws.get(k).copied())?))
            };
            let last_row = rows.saturating_sub(1);
            let last_col = cols.saturating_sub(1);
            let rows_one = (0..cols).all(|j| {
                let (a, b) = (at(0, j), at(last_row, j));
                a.is_some() && a == b
            });
            let cols_one = (0..rows).all(|i| {
                let (a, b) = (at(i, 0), at(i, last_col));
                a.is_some() && a == b
            });
            out.closes = [
                bu.is_clamped() && rows > 1 && rows_one,
                bv.is_clamped() && cols > 1 && cols_one,
            ];
            for e in s.collapsed_edges() {
                match e {
                    Edge::U0 if bu.is_clamped() => out.poles.0.push(u0),
                    Edge::U1 if bu.is_clamped() => out.poles.0.push(u1),
                    Edge::V0 if bv.is_clamped() => out.poles.1.push(v0),
                    Edge::V1 if bv.is_clamped() => out.poles.1.push(v1),
                    _ => {}
                }
            }
            out.creases = (crease_knots(bu), crease_knots(bv));
        }
        SurfaceKind::Plane(_) | SurfaceKind::Extrusion(_) | SurfaceKind::Ruled(_) => {}
    }
    out
}

/// The lines a surface's domain is cut at before anything else: its creases and the poles inside
/// it, in u then in v, each increasing and without repeats.
#[must_use]
pub fn cuts(charts: &Charts, surface: &SurfaceKind) -> (Vec<f64>, Vec<f64>) {
    let ((u0, u1), (v0, v1)) = surface.domain();
    let inside = |lines: [&[f64]; 2], (lo, hi): (f64, f64)| -> Vec<f64> {
        let mut out: Vec<f64> = lines
            .into_iter()
            .flatten()
            .copied()
            .filter(|&x| lo < x && x < hi)
            .collect();
        out.sort_by(f64::total_cmp);
        out.dedup();
        out
    };
    (
        inside([&charts.creases.0, &charts.poles.0], (u0, u1)),
        inside([&charts.creases.1, &charts.poles.1], (v0, v1)),
    )
}

/// How two enclosures of points on a pair of surfaces meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Meet {
    /// They overlap in every parameter.
    Same,
    /// They overlap in every parameter but those held at the two ends of a closing one.
    Seam,
    /// They are held on one pole line of a surface, so they are its one point there, whatever
    /// the line's free parameter; every other parameter overlaps or meets across a seam.
    Pole,
}

/// Whether `x` is exactly the point `at`.
fn held_at(x: Interval, at: f64) -> bool {
    x.lo() == at && x.hi() == at
}

/// Whether `x` and `y` are both held exactly on one of `lines`.
fn on_one(x: Interval, y: Interval, lines: &[f64]) -> bool {
    lines.iter().any(|&p| held_at(x, p) && held_at(y, p))
}

/// How `a` and `b`, enclosures of (u₁, v₁, u₂, v₂), meet (module documentation), on two surfaces
/// with `domains` and `charts`.
pub(crate) fn meet(
    a: &[Interval; 4],
    b: &[Interval; 4],
    domains: [Region; 2],
    charts: [&Charts; 2],
) -> Option<Meet> {
    let [((u1, u2), (v1, v2)), ((s1, s2), (r1, r2))] = domains;
    let [one, two] = charts;
    let ([cu, cv], [cs, cr]) = (one.closes, two.closes);
    let [au, av, as_, ar] = *a;
    let [bu, bv, bs, br] = *b;
    // A parameter may differ freely where the other of its surface holds both on a pole line.
    let free = [
        on_one(av, bv, &one.poles.1),
        on_one(au, bu, &one.poles.0),
        on_one(ar, br, &two.poles.1),
        on_one(as_, bs, &two.poles.0),
    ];
    let lines = [
        ((u1, u2), cu),
        ((v1, v2), cv),
        ((s1, s2), cs),
        ((r1, r2), cr),
    ];
    let mut how = Meet::Same;
    for (((x, y), ((lo, hi), closes)), free) in a.iter().zip(b).zip(lines).zip(free) {
        if x.lo() <= y.hi() && y.lo() <= x.hi() {
            continue;
        }
        if free {
            how = Meet::Pole;
        } else if closes
            && ((held_at(*x, lo) && held_at(*y, hi)) || (held_at(*x, hi) && held_at(*y, lo)))
        {
            if how == Meet::Same {
                how = Meet::Seam;
            }
        } else {
            return None;
        }
    }
    Some(how)
}

/// Whether `x` lies on a pole line of either surface: held exactly on one.
pub(crate) fn at_pole(x: &[Interval; 4], charts: [&Charts; 2]) -> bool {
    let [u, v, s, r] = *x;
    let [one, two] = charts;
    let held = |x: Interval, lines: &[f64]| lines.iter().any(|&p| held_at(x, p));
    held(u, &one.poles.0) || held(v, &one.poles.1) || held(s, &two.poles.0) || held(r, &two.poles.1)
}

/// The directions a branch leaves a pole by (12.10): on a pole line of `own`, where one parameter
/// is held at `at`, the values t of the other at which `n · S_off(t)` is zero. `S_off` is the
/// partial off the line, the way a step from the pole moves: the factor that vanishes along the
/// line, so that `S_t` is zero there, is divided out by reading it. `n` is the other surface's
/// normal where the pole meets it, so a zero is a direction in both tangent planes: a branch's.
struct Toward<'a> {
    own: &'a SurfaceKind,
    at: f64,
    fixed_u: bool,
    normal: [Interval; 3],
    floor: f64,
}

/// `S_off` over a span of the line, and its rate along it.
type OffAndRate = ([Interval; 3], [Interval; 3]);

impl Toward<'_> {
    /// The line's region over `t`.
    fn region(&self, t: Interval) -> Region {
        if self.fixed_u {
            ((self.at, self.at), ends(t))
        } else {
            (ends(t), (self.at, self.at))
        }
    }

    /// `S_off` over `t` and its rate along the line, `S_uv` whichever parameter is held.
    fn read(
        &self,
        t: Interval,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<OffAndRate>, IntersectError> {
        let Some(found) = partials(self.own, self.region(t), 2, meter)? else {
            return Ok(None);
        };
        let ([_, su, sv, _, suv, ..], fixed_u) = (found.as_slice(), self.fixed_u) else {
            return Err(IntersectError::Internal);
        };
        Ok(Some((if fixed_u { *su } else { *sv }, *suv)))
    }
}

impl Field<1> for Toward<'_> {
    fn at(&self, x: [f64; 1], meter: &mut WorkMeter<'_>) -> Result<[Interval; 1], IntersectError> {
        let [t] = x;
        self.over(&[point(t)?], meter)?
            .ok_or(IntersectError::Internal)
    }

    fn over(
        &self,
        x: &[Interval; 1],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 1]>, IntersectError> {
        let [t] = *x;
        match self.read(t, meter)? {
            Some((off, _)) => Ok(Some([dot(&self.normal, &off)?])),
            None => Ok(None),
        }
    }

    fn slope(
        &self,
        x: &[Interval; 1],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; 1]; 1]>, IntersectError> {
        let [t] = *x;
        match self.read(t, meter)? {
            Some((_, rate)) => Ok(Some([[dot(&self.normal, &rate)?]])),
            None => Ok(None),
        }
    }

    /// How far apart, in floors, a unit step off the line lands across `x`: the spread of `S_off`.
    fn reach(&self, x: &[Interval; 1], meter: &mut WorkMeter<'_>) -> Result<f64, IntersectError> {
        let [t] = *x;
        match self.read(t, meter)? {
            Some((off, _)) => Ok(image(&off) / self.floor),
            None => Ok(f64::INFINITY),
        }
    }
}

/// Whether two spans share a point.
fn overlaps(x: Interval, (lo, hi): (f64, f64)) -> bool {
    x.lo() <= hi && lo <= x.hi()
}

/// The start points a pair takes from a pole of `own` (12.10), as (t, s, r): t along the pole
/// line of `here` where one parameter is held at `at`, (s, r) on `other`. The line is one point P,
/// so it is not searched as a curve, whose roots would be a continuum. P is tested against `other`
/// as a point (12.4): proven apart, it gives no start; within the floor of it at a foot inside
/// `there`, each direction a branch leaves P by inside `here` is one start; anything else is
/// undecided, and gives `None`.
pub(crate) fn pole_starts(
    own: &SurfaceKind,
    other: &SurfaceKind,
    (here, there): (Region, Region),
    (at, fixed_u): (f64, bool),
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<[Interval; 3]>>, IntersectError> {
    let ((a, b), (c, d)) = here;
    let (corner, along) = if fixed_u {
        (((at, at), (c, c)), (c, d))
    } else {
        (((a, a), (at, at)), (a, b))
    };
    // The whole line is one point, exactly (`charts`), so one parameter on it reads all of it.
    let Some(read) = partials(own, corner, 0, meter)? else {
        return Ok(None);
    };
    let [pole, ..] = read.as_slice() else {
        return Err(IntersectError::Internal);
    };
    let middle = pole.map(|x| x.lo() / 2.0 + x.hi() / 2.0);
    // Every point of the box is within its widest side of its middle.
    let spread = image(pole);
    // Projected to an eighth of the floor, so that a pole on the other surface is proven within
    // the floor of it: the projection's own enclosures are as wide as the floor it is given.
    let feet = match project_surface(other, &middle, floor / 8.0, meter)? {
        Projection::Points(found) => found,
        Projection::Set { distance, .. } if distance.lo() > spread => return Ok(Some(Vec::new())),
        Projection::Set { .. } | Projection::NonIsolated { .. } => return Ok(None),
    };
    let mut out = Vec::new();
    for foot in feet {
        if foot.distance.lo() > spread {
            continue;
        }
        if foot.distance.hi() + spread > floor {
            return Ok(None);
        }
        let [s, r] = foot.at;
        if !(overlaps(s, there.0) && overlaps(r, there.1)) {
            continue;
        }
        let toward = Toward {
            own,
            at,
            fixed_u,
            normal: normals(other, (ends(s), ends(r)), meter)?,
            floor,
        };
        let found = solve(&toward, [span(along)?], |_, _| Ok(true), meter)?;
        if !found.flagged.is_empty() {
            return Ok(None);
        }
        for zero in found.zeros {
            let [t] = zero.enclosure;
            out.push([t, s, r]);
        }
    }
    Ok(Some(out))
}

/// `surface`'s domain cut at the lines of [`cuts`], in order of u and then of v.
pub(crate) fn cells(surface: &SurfaceKind, charts: &Charts) -> Vec<Region> {
    let ((u0, u1), (v0, v1)) = surface.domain();
    let (us, vs) = cuts(charts, surface);
    let spans = |lo: f64, hi: f64, inner: &[f64]| -> Vec<(f64, f64)> {
        let mut out = Vec::with_capacity(inner.len().saturating_add(1));
        let mut from = lo;
        for &x in inner {
            out.push((from, x));
            from = x;
        }
        out.push((from, hi));
        out
    };
    let mut out = Vec::new();
    for u in spans(u0, u1, &us) {
        for &v in &spans(v0, v1, &vs) {
            out.push((u, v));
        }
    }
    out
}
