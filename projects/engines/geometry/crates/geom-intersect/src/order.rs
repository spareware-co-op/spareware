//! The canonical order of a pair: which operand goes first, and how a batch of pairs runs (12.15).
//!
//! What it does: a [`CanonicalKey`] orders operands by kind, then by a kernel-internal encoding of
//! their defining data: each kind's NURBS form, which is a function of that data alone, or, for a
//! line or plane that has no such form because it may be unbounded, the data itself. [`pair`] puts
//! two [`Operand`]s in that order, runs a method on them and gives its answer back in the caller's
//! roles. [`pairs`] does so for many pairs on the kernel's parallel helper, each under an equal
//! share of the budget, in canonical pair order.
//!
//! Guarantees:
//!
//! - **The same items with the roles swapped.** A method only ever sees the canonical order, so
//!   `pair(a, b)` and `pair(b, a)` make the same call, and differ only in which operand's unknowns
//!   come first in each item's enclosure.
//! - **No address and no file format.** The key reads the defining data's values and nothing else,
//!   so it never depends on memory addresses, nor on file-format bytes, whose version can change
//!   independently (12.15). It is versioned with the kernel edition (26.2).
//! - **The same answers on any thread count.** Pairs run in chunks fixed by [`PLAN`], each under its
//!   own budget, and come back in the caller's order. A pair's budget never depends on the others'
//!   costs, only on how many pairs there are, so the shares of 12.15 are the same in any order.
//!
//! Method: chapter 12.15's canonical operand order and per-pair budgets
//! (`projects/engines/geometry/architecture/12-intersections.md`), on `geom-run`'s batch rule.
//!
//! Cost: a key is one conversion to NURBS, charged to the meter; ordering a pair compares two keys,
//! linear in their lengths. Recursion: none.

use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_curve::{CurveKind, ParamCurve};
use geom_nurbs::Basis;
use geom_run::{BatchBudget, Budget, Cancel, ChunkPlan, Exec, ItemResult, Phase, WorkMeter};
use geom_surface::{ImplicitBound, ParamSurface, SurfaceKind};

use crate::error::IntersectError;
use crate::result::{Intersection, ParamBox};

/// The phase a batch of pairs charges in.
pub const PHASE: Phase = Phase::new("intersect");

/// Pairs per chunk: an intersection is long, so each goes on its own.
pub const PLAN: ChunkPlan = ChunkPlan::fixed::<1>();

/// What fixes an operand's place in a pair: its kind, then its defining data's encoding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalKey {
    kind: u16,
    data: Vec<u64>,
}

/// A double's bits, in an order that is total and the same on every target.
const fn bits(x: f64) -> u64 {
    x.to_bits()
}

/// `basis`, as words: its degree, then its knots.
fn basis_words(data: &mut Vec<u64>, basis: &Basis) {
    data.push(u64::try_from(basis.degree()).unwrap_or(u64::MAX));
    data.extend(basis.knots().iter().map(|&k| bits(k)));
}

/// Control points and weights, as words.
fn net_words<const D: usize>(data: &mut Vec<u64>, net: &[[f64; D]], weights: Option<&[f64]>) {
    data.extend(net.iter().flatten().map(|&x| bits(x)));
    if let Some(weights) = weights {
        data.extend(weights.iter().map(|&w| bits(w)));
    }
}

/// A curve kind's place among the kinds. The match is exhaustive, so a new kind must take a place.
const fn curve_rank(curve: &CurveKind<3>) -> u16 {
    match curve {
        CurveKind::Line(_) => 0,
        CurveKind::Circle(_) => 1,
        CurveKind::Ellipse(_) => 2,
        CurveKind::Parabola(_) => 3,
        CurveKind::Hyperbola(_) => 4,
        CurveKind::Polynomial(_) => 5,
        CurveKind::Nurbs(_) => 6,
        CurveKind::Helix(_) => 7,
        CurveKind::Involute(_) => 8,
        CurveKind::Bulge(_) => 9,
        CurveKind::Transition(_) => 10,
        CurveKind::Trimmed(_) => 11,
        CurveKind::Composite(_) => 12,
        CurveKind::Offset(_) => 13,
        CurveKind::OnSurface(_) => 14,
    }
}

/// A surface kind's place among the kinds, after every curve's.
const fn surface_rank(surface: &SurfaceKind) -> u16 {
    match surface {
        SurfaceKind::Plane(_) => 100,
        SurfaceKind::Cylinder(_) => 101,
        SurfaceKind::Cone(_) => 102,
        SurfaceKind::Sphere(_) => 103,
        SurfaceKind::Torus(_) => 104,
        SurfaceKind::Nurbs(_) => 105,
        SurfaceKind::Affine(_) => 106,
        SurfaceKind::Revolution(_) => 107,
        SurfaceKind::Extrusion(_) => 108,
        SurfaceKind::Ruled(_) => 109,
        SurfaceKind::Offset(_) => 110,
    }
}

/// The first rank an implicit operand can have, after every parametric kind's.
const IMPLICIT: u16 = 1000;

impl CanonicalKey {
    /// The key of `curve`: a line by its origin, direction and domain, since it may be unbounded;
    /// any other kind by its NURBS form.
    ///
    /// # Errors
    ///
    /// The conversion's: the meter's stop, or [`IntersectError::InvalidInput`].
    pub fn of_curve(
        curve: &CurveKind<3>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<CanonicalKey, IntersectError> {
        meter.poll()?;
        let mut data = Vec::new();
        if let CurveKind::Line(line) = curve {
            let (lo, hi) = ParamCurve::domain(line);
            let [ox, oy, oz] = line.origin();
            let [dx, dy, dz] = line.direction();
            data.extend([ox, oy, oz, dx, dy, dz, lo, hi].map(bits));
        } else {
            let form = curve.to_nurbs(meter)?.value;
            basis_words(&mut data, form.curve.basis());
            net_words(&mut data, form.curve.homogeneous(), form.curve.weights());
        }
        Ok(CanonicalKey {
            kind: curve_rank(curve),
            data,
        })
    }

    /// The key of `surface`: a plane by its origin, spanning vectors and domain, since it may be
    /// unbounded; any other kind by its NURBS form.
    ///
    /// # Errors
    ///
    /// The conversion's: the meter's stop, or [`IntersectError::InvalidInput`].
    pub fn of_surface(
        surface: &SurfaceKind,
        meter: &mut WorkMeter<'_>,
    ) -> Result<CanonicalKey, IntersectError> {
        meter.poll()?;
        let mut data = Vec::new();
        if let SurfaceKind::Plane(plane) = surface {
            let (origin, a, b) = plane.vectors();
            let ((u0, u1), (v0, v1)) = ParamSurface::domain(plane);
            for v in [origin, a, b] {
                data.extend(v.map(bits));
            }
            data.extend([u0, u1, v0, v1].map(bits));
        } else {
            let form = surface.to_nurbs(meter)?.value;
            basis_words(&mut data, form.surface.basis_u());
            basis_words(&mut data, form.surface.basis_v());
            net_words(
                &mut data,
                form.surface.homogeneous(),
                form.surface.weights(),
            );
        }
        Ok(CanonicalKey {
            kind: surface_rank(surface),
            data,
        })
    }

    /// The key of an implicit operand of `kind`, whose defining data the caller has as `data`: the
    /// ones who make implicit operands are the sections that define them (12.1).
    #[must_use]
    pub fn of_implicit(kind: u16, data: Vec<u64>) -> CanonicalKey {
        CanonicalKey {
            kind: IMPLICIT.saturating_add(kind),
            data,
        }
    }
}

/// What an operand is.
#[derive(Clone, Copy)]
pub enum Geom<'a> {
    /// A curve.
    Curve(&'a CurveKind<3>),
    /// A surface.
    Surface(&'a SurfaceKind),
    /// A field: its level set.
    Implicit(&'a (dyn ImplicitBound + Sync)),
}

impl Geom<'_> {
    /// How many unknowns it brings to a pair: one for a curve, two for a surface, and none for a
    /// field, which is solved in the other operand's parameters.
    #[must_use]
    pub const fn unknowns(&self) -> u8 {
        match self {
            Geom::Curve(_) => 1,
            Geom::Surface(_) => 2,
            Geom::Implicit(_) => 0,
        }
    }
}

/// One side of a pair (12.2).
#[derive(Clone)]
pub struct Operand<'a> {
    /// What it is.
    pub geom: Geom<'a>,
    /// Where to search.
    pub domain: Option<ParamBox>,
    /// How far this data may be from the true shape; 0 for exact data.
    pub bound: geom_num::Steps,
    /// What fixes the operand order.
    pub key: CanonicalKey,
}

/// `a` and `b`'s order: whether `b` goes first.
fn swapped(a: &CanonicalKey, b: &CanonicalKey) -> bool {
    a.cmp(b) == Ordering::Greater
}

/// `solve` on `a` and `b` in canonical order, its answer in the caller's roles: `a`'s unknowns
/// first. Two operands with one key are one shape, so either order is the same call.
///
/// # Errors
///
/// `solve`'s.
pub fn pair<'o, 'a, S>(
    a: &'o Operand<'a>,
    b: &'o Operand<'a>,
    solve: S,
) -> Result<Intersection, IntersectError>
where
    S: FnOnce(&'o Operand<'a>, &'o Operand<'a>) -> Result<Intersection, IntersectError>,
{
    if swapped(&a.key, &b.key) {
        Ok(solve(b, a)?.swapped(b.geom.unknowns()))
    } else {
        solve(a, b)
    }
}

/// The budget of one pair of `count`: an equal share of `budget`.
#[must_use]
pub fn share(budget: Budget, count: usize) -> Budget {
    let count = u64::try_from(count).unwrap_or(u64::MAX).max(1);
    Budget::new(
        budget.work.checked_div(count).unwrap_or(0),
        budget.logical_bytes.checked_div(count).unwrap_or(0),
    )
}

/// `solve` on every pair of `items`, each in canonical order under an equal share of `budget`, the
/// answers in the caller's order. `solve` gets the pair's own meter.
///
/// # Errors
///
/// A cancellation, as [`IntersectError::Cancelled`], or a refused allocation, as
/// [`IntersectError::BudgetExceeded`], either of which stops the whole batch. A pair that fails has
/// its own error in its place.
pub fn pairs<'a, S>(
    exec: &Exec<'_>,
    budget: Budget,
    cancel: Option<&dyn Cancel>,
    items: &[(Operand<'a>, Operand<'a>)],
    solve: S,
) -> Result<Vec<ItemResult<Intersection, IntersectError>>, IntersectError>
where
    S: Fn(&Operand<'a>, &Operand<'a>, &mut WorkMeter<'_>) -> Result<Intersection, IntersectError>
        + Sync,
{
    exec.map_each(
        BatchBudget::new(share(budget, items.len())),
        cancel,
        PHASE,
        PLAN,
        items,
        |i, _| i,
        |_, (a, b), meter| pair(a, b, |x, y| solve(x, y, meter)),
    )
    .map_err(IntersectError::from)
}
