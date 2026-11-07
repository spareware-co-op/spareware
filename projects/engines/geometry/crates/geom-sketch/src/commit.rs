//! The commit plan: from a solved sketch to stored geometry, with every declared incidence exact.
//!
//! What it does: [`commit`] turns the solver's floating-point positions into grid values and integer
//! forms (chapter 15.10). Points the sketch pins become grid points, each master rounded **once**,
//! in the sketch's own integer space; straight lines become integer carriers; a point that two
//! carriers must share, or that must lie on one, becomes their meeting, a depth-one point; an arc's
//! centre becomes a rational point on its chord's bisector; a circle keeps a grid centre and a whole
//! radius. Then every constraint the section 15.10 table calls exact is checked **exactly** against
//! what was stored, and marked [`Mark::Exact`], or [`Mark::Bounded`] and listed in
//! [`Committed::broken`] with the reason when the stored geometry cannot hold it.
//!
//! Guarantees:
//!
//! - **Rounded once.** A master (a coordinate group's value, a line's direction, a carrier's offset,
//!   a circle's radius) is rounded from the solved value exactly once, with ties to even; every
//!   other stored quantity is computed from masters in integers. Nothing is rounded twice.
//! - **Exact incidences.** A relation marked `Exact` was verified with `geom-num`'s exact predicates
//!   (`Carrier::side`, `Carrier::parallel`, `Carrier::same`, `Rational2::same`, `Arc2::new`) on the
//!   stored values, not assumed from how they were made. A T-junction, a corner of lines that cannot
//!   both pass through a rounded point, and a point sliding on a sloping line are depth-one points
//!   on their carriers, so a wall meeting a sloping wall leaves no gap and no sliver.
//! - **Cycles give way in a fixed order.** A relation whose quantities were all placed by others is
//!   only checked. If it fails it is `Bounded`, reported in [`Committed::broken`] with the reason,
//!   and it is always the later constraint in authoring order that gives way, so the same relation
//!   gives way on every machine.
//! - **Deterministic and total.** Integers except the rounding of a solved value; ties to even; every
//!   walk is in authoring order; every list is iterated in index order; every step is checked and
//!   charged to the meter; no recursion.
//!
//! Method: chapter 15.10 of the kernel architecture
//! (`projects/engines/geometry/architecture/15-sketches.md`, "Exact forms" and "The commit plan"):
//! carriers, depth-one points and rational-centre arcs from `geom_num::carrier` and `geom_num::arc`
//! (owner action G186, decision 32.26), masters rounded once as Shewchuk's and Hoffmann's
//! "rounded once" rule requires; the dependency order is a union-find over the exact relations (R.
//! E. Tarjan, "Efficiency of a good but not linear set union algorithm", J. ACM 22(2), 1975,
//! doi:10.1145/321879.321884), with offsets between members as in a weighted union-find.
//!
//! Cost: O((n + m) alpha(n)) for n entities and m constraints, plus one exact predicate per exact
//! relation; O(n + m) memory. No recursion. A 1,000-entity commit takes about 2 ms (`benches/commit.rs`).

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the carrier's a, b, c, a point's x, y, and the line's ends p, q, as in chapter 15.10"
)]

extern crate alloc;

use alloc::vec::Vec;

use geom_num::arc::{Arc2, Circle2, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::math::round_ties_even;
use geom_num::{NumError, Sign};
use geom_run::WorkMeter;

use crate::model::{
    Amount, CommitClass, Constraint, ConstraintKind, Entity, EntityKind, ItemId, Sketch,
    SketchError,
};

/// Where a point is stored.
#[derive(Clone, Copy, Debug)]
pub enum Stored {
    /// A grid point: a master rounded once, or a point sharing a master's coordinates.
    Grid(P2),
    /// A depth-one point: the meeting of two carriers, or of a carrier and a grid row or column.
    Meet(Rational2),
}

impl Stored {
    /// The point as a rational point.
    #[must_use]
    pub fn rational(&self) -> Rational2 {
        match *self {
            Stored::Grid(p) => Rational2::from(p),
            Stored::Meet(r) => r,
        }
    }
}

/// What the commit made of a constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Verified exactly on the stored geometry.
    Exact,
    /// Not exact: its class is bounded in section 15.10's table, or the stored geometry could not
    /// hold it (then it is also in [`Committed::broken`]).
    Bounded,
}

/// Why an exact relation could not be held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The stored geometry does not satisfy it: every quantity in it was placed by other relations
    /// first, and rounding left it unmet.
    Disagrees,
    /// It names a value the committed form cannot take (a figure past the budget, or a part of the
    /// sketch it needs that this commit has no form for).
    NoForm,
}

/// An exact-class relation the stored geometry does not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Broken {
    /// The constraint that gave way.
    pub constraint: ItemId,
    /// Why.
    pub why: Why,
}

/// The committed sketch.
#[derive(Clone, Debug, Default)]
pub struct Committed {
    /// Every point and reference point, in authoring order.
    pub points: Vec<(ItemId, Stored)>,
    /// Every line, as its carrier.
    pub lines: Vec<(ItemId, Carrier)>,
    /// Every full circle: a grid centre and a whole radius.
    pub circles: Vec<(ItemId, Circle2)>,
    /// Every full circle with a defining point and no whole radius to state: a grid centre and the
    /// grid point it passes through, so its radius is exact without being a whole number of steps.
    pub rounds: Vec<(ItemId, Round)>,
    /// Every arc: grid endpoints and a rational centre on their bisector.
    pub arcs: Vec<(ItemId, Arc2)>,
    /// Every constraint, in authoring order, with what the commit made of it.
    pub marks: Vec<(ItemId, Mark)>,
    /// The exact-class constraints that gave way, in authoring order.
    pub broken: Vec<Broken>,
}

impl Committed {
    /// The stored point of `id`.
    #[must_use]
    pub fn point(&self, id: ItemId) -> Option<&Stored> {
        find(&self.points, id)
    }

    /// The carrier of the line `id`.
    #[must_use]
    pub fn line(&self, id: ItemId) -> Option<&Carrier> {
        find(&self.lines, id)
    }

    /// The mark of the constraint `id`.
    #[must_use]
    pub fn mark(&self, id: ItemId) -> Option<Mark> {
        find(&self.marks, id).copied()
    }
}

/// The value of `id` in a list kept in id order, which every list of [`Committed`] is.
pub(crate) fn find<T>(list: &[(ItemId, T)], id: ItemId) -> Option<&T> {
    list.binary_search_by_key(&id, |&(k, _)| k)
        .ok()
        .and_then(|at| list.get(at))
        .map(|(_, v)| v)
}

fn get<T: Copy>(list: &[T], i: usize) -> Result<T, SketchError> {
    list.get(i).copied().ok_or(SketchError::Internal)
}

fn slot<T>(list: &mut [T], i: usize) -> Result<&mut T, SketchError> {
    list.get_mut(i).ok_or(SketchError::Internal)
}

fn push<T>(list: &mut Vec<T>, value: T) -> Result<(), SketchError> {
    list.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
    list.push(value);
    Ok(())
}

/// The position of the entity `id` in a list sorted by id.
fn lookup(index: &[(ItemId, usize)], id: ItemId) -> Result<usize, SketchError> {
    index
        .binary_search_by_key(&id, |&(k, _)| k)
        .ok()
        .and_then(|at| index.get(at))
        .map(|&(_, i)| i)
        .ok_or(SketchError::Unknown(id))
}

fn cost(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The coordinate bound as a float, for range checks.
const RANGE: f64 = 1_099_511_627_775.0;

/// The nearest whole number, ties to even, if it is a grid coordinate.
///
/// # Errors
///
/// [`SketchError::NotFinite`] for NaN or infinity; [`SketchError::Numeric`] past the grid.
fn nearest(v: f64) -> Result<i64, SketchError> {
    if !v.is_finite() {
        return Err(SketchError::NotFinite);
    }
    let r = round_ties_even(v)?.get();
    if r.abs() > RANGE {
        return Err(SketchError::Numeric(NumError::OutOfRange {
            value: i128::MAX,
        }));
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "checked against the grid's range above, and a whole number"
    )]
    Ok(r as i64)
}

/// Euclid's greatest common divisor of two magnitudes. Written here because it is four lines and the
/// kernel crates admit no new dependency without the owner's decision (`deny-no-std.toml`).
fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let r = a.checked_rem(b).unwrap_or(0);
        a = b;
        b = r;
    }
    a
}

/// A weighted union-find over coordinate slots: `value(v) = value(parent(v)) + offset(v)`.
struct Offsets {
    parent: Vec<usize>,
    offset: Vec<i64>,
    fixed: Vec<Option<i64>>,
}

impl Offsets {
    fn new(n: usize) -> Offsets {
        Offsets {
            parent: (0..n).collect(),
            offset: alloc::vec![0; n],
            fixed: alloc::vec![None; n],
        }
    }

    /// The root of `s` and `value(s) - value(root)`; compresses the path.
    fn find(&mut self, s: usize) -> Result<(usize, i64), SketchError> {
        let up = get(&self.parent, s)?;
        if up == s {
            return Ok((s, 0));
        }
        if get(&self.parent, up)? == up {
            return Ok((up, get(&self.offset, s)?));
        }
        let mut path = Vec::new();
        let mut at = s;
        let mut total = 0_i64;
        while get(&self.parent, at)? != at {
            push(&mut path, at)?;
            total = total
                .checked_add(get(&self.offset, at)?)
                .ok_or(SketchError::Internal)?;
            at = get(&self.parent, at)?;
        }
        let mut rest = total;
        for v in path {
            let old = get(&self.offset, v)?;
            *slot(&mut self.parent, v)? = at;
            *slot(&mut self.offset, v)? = rest;
            rest = rest.checked_sub(old).ok_or(SketchError::Internal)?;
        }
        Ok((at, total))
    }

    /// Says `value(b) - value(a) = d`. `false` if that contradicts what is already said.
    fn union(&mut self, a: usize, b: usize, d: i64) -> Result<bool, SketchError> {
        let ((ra, oa), (rb, ob)) = (self.find(a)?, self.find(b)?);
        let sum = |x: i64, y: i64| x.checked_add(y).ok_or(SketchError::Internal);
        let diff = |x: i64, y: i64| x.checked_sub(y).ok_or(SketchError::Internal);
        if ra == rb {
            return Ok(diff(ob, oa)? == d);
        }
        // value(rb) = value(ra) + shift
        let shift = diff(sum(oa, d)?, ob)?;
        if let (Some(fa), Some(fb)) = (get(&self.fixed, ra)?, get(&self.fixed, rb)?)
            && sum(fa, shift)? != fb
        {
            return Ok(false);
        }
        if get(&self.fixed, ra)?.is_none()
            && let Some(fb) = get(&self.fixed, rb)?
        {
            *slot(&mut self.fixed, ra)? = Some(diff(fb, shift)?);
        }
        *slot(&mut self.parent, rb)? = ra;
        *slot(&mut self.offset, rb)? = shift;
        Ok(true)
    }

    /// Says `value(s) = v`. `false` if that contradicts what is already said.
    fn fix(&mut self, s: usize, v: i64) -> Result<bool, SketchError> {
        let (r, o) = self.find(s)?;
        let target = v.checked_sub(o).ok_or(SketchError::Internal)?;
        if let Some(x) = get(&self.fixed, r)? {
            Ok(x == target)
        } else {
            *slot(&mut self.fixed, r)? = Some(target);
            Ok(true)
        }
    }
}

/// A plain union-find with a parity: `parity(v)` is 0 or 1, the quarter turns of `v` from its
/// parent. Used for the points a coincidence merges (parity unused) and for line directions.
struct Classes {
    parent: Vec<usize>,
    parity: Vec<u8>,
}

impl Classes {
    fn new(n: usize) -> Classes {
        Classes {
            parent: (0..n).collect(),
            parity: alloc::vec![0; n],
        }
    }

    fn find(&mut self, s: usize) -> Result<(usize, u8), SketchError> {
        let up = get(&self.parent, s)?;
        if up == s {
            return Ok((s, 0));
        }
        if get(&self.parent, up)? == up {
            return Ok((up, get(&self.parity, s)?));
        }
        let mut path = Vec::new();
        let mut at = s;
        let mut total = 0_u8;
        while get(&self.parent, at)? != at {
            push(&mut path, at)?;
            total ^= get(&self.parity, at)?;
            at = get(&self.parent, at)?;
        }
        let mut rest = total;
        for v in path {
            let old = get(&self.parity, v)?;
            *slot(&mut self.parent, v)? = at;
            *slot(&mut self.parity, v)? = rest;
            rest ^= old;
        }
        Ok((at, total))
    }

    /// Says `b` is `turns` quarter turns from `a`; the lower index becomes the root.
    fn union(&mut self, a: usize, b: usize, turns: u8) -> Result<(), SketchError> {
        let ((ra, pa), (rb, pb)) = (self.find(a)?, self.find(b)?);
        if ra == rb {
            return Ok(());
        }
        let link = pa ^ pb ^ (turns & 1);
        if ra < rb {
            *slot(&mut self.parent, rb)? = ra;
            *slot(&mut self.parity, rb)? = link;
        } else {
            *slot(&mut self.parent, ra)? = rb;
            *slot(&mut self.parity, ra)? = link;
        }
        Ok(())
    }
}

/// The two slots of the point at entity `i`: x then y.
fn xs(i: usize) -> Result<usize, SketchError> {
    i.checked_mul(2).ok_or(SketchError::Internal)
}

fn ys(i: usize) -> Result<usize, SketchError> {
    xs(i)?.checked_add(1).ok_or(SketchError::Internal)
}

/// A carrier's coefficients as a pair, quarter-turned: (a, b) becomes (-b, a).
fn quarter(a: i128, b: i128) -> Result<(i128, i128), SketchError> {
    Ok((b.checked_neg().ok_or(SketchError::Internal)?, a))
}

/// The constraint's whole stated value, if it has one.
fn whole(amount: Amount) -> Option<i64> {
    match amount {
        Amount::Value(x) if x.is_finite() && x.abs() < 1.0e15 => {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_precision_loss,
                reason = "tested to convert back to the same number"
            )]
            let n = x as i64;
            #[allow(clippy::cast_precision_loss, reason = "the round trip is the test")]
            (n as f64 == x).then_some(n)
        }
        _ => None,
    }
}

/// An angle, in half-turns, as a whole number of quarter turns, if it is one.
fn quarters(amount: Amount) -> Option<i64> {
    match amount {
        Amount::Value(x) => whole(Amount::Value(x * 2.0)),
        Amount::Named(_) => None,
    }
}

/// What a line is constrained to be, besides its endpoints.
#[derive(Clone, Copy, Default)]
struct Axis {
    /// 0: none, 1: horizontal, 2: vertical.
    kind: u8,
}

struct Plan<'a> {
    ents: &'a [Entity],
    cons: &'a [Constraint],
    index: Vec<(ItemId, usize)>,
    /// The endpoints of each line, as entity indices, found once.
    line_ends: Vec<Option<[usize; 2]>>,
}

impl Plan<'_> {
    fn at(&self, id: ItemId) -> Result<usize, SketchError> {
        lookup(&self.index, id)
    }

    fn entity(&self, i: usize) -> Result<&Entity, SketchError> {
        self.ents.get(i).ok_or(SketchError::Internal)
    }

    /// Whether entity `i` is a point of the sketch (free or reference).
    fn is_point(&self, i: usize) -> bool {
        matches!(
            self.ents.get(i).map(|e| &e.kind),
            Some(EntityKind::Point { .. } | EntityKind::Reference { .. })
        )
    }

    /// The endpoints of the line at `i`, as entity indices.
    fn ends(&self, i: usize) -> Result<Option<[usize; 2]>, SketchError> {
        get(&self.line_ends, i)
    }

    /// The solved position of the point at `i`, in steps.
    fn solved(&self, i: usize) -> Result<[f64; 2], SketchError> {
        match &self.entity(i)?.kind {
            EntityKind::Point { x, y } | EntityKind::Reference { x, y } => Ok([*x, *y]),
            _ => Err(SketchError::WrongKind(self.entity(i)?.id)),
        }
    }
}

/// The rounded normal of a direction (dx, dy): the longest component scaled to 2^40, each rounded
/// once, divided by their greatest common divisor. It turns the line by at most about 2^-40 rad.
fn rounded_normal(dx: f64, dy: f64, id: ItemId) -> Result<(i128, i128), SketchError> {
    if !(dx.is_finite() && dy.is_finite()) {
        return Err(SketchError::NotFinite);
    }
    let longest = if dx.abs() >= dy.abs() {
        dx.abs()
    } else {
        dy.abs()
    };
    if longest == 0.0 {
        return Err(SketchError::Degenerate(id));
    }
    let scale = 1_099_511_627_776.0;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "at most 2^40 in magnitude, a whole number after rounding"
    )]
    let round = |v: f64| -> Result<i128, SketchError> {
        Ok(round_ties_even(v / longest * scale)?.get() as i128)
    };
    let (a, b) = (round(-dy)?, round(dx)?);
    let g = gcd(a, b);
    if g == 0 {
        return Err(SketchError::Degenerate(id));
    }
    Ok((
        a.checked_div(g).ok_or(SketchError::Internal)?,
        b.checked_div(g).ok_or(SketchError::Internal)?,
    ))
}

/// A carrier's coefficients quarter-turned the other way: (a, b) becomes (b, -a).
fn unquarter(a: i128, b: i128) -> Result<(i128, i128), SketchError> {
    Ok((b, a.checked_neg().ok_or(SketchError::Internal)?))
}

fn neg(a: i128) -> Result<i128, SketchError> {
    a.checked_neg().ok_or(SketchError::Internal)
}

fn div(a: i128, g: i128) -> Result<i128, SketchError> {
    a.checked_div(g).ok_or(SketchError::Internal)
}

/// c = a x + b y for a grid point.
fn offset_of(a: i128, b: i128, p: P2) -> Result<i128, SketchError> {
    let (x, y) = (i128::from(p.x.get()), i128::from(p.y.get()));
    a.checked_mul(x)
        .and_then(|ax| b.checked_mul(y).and_then(|by| ax.checked_add(by)))
        .ok_or(SketchError::Numeric(NumError::BudgetExceeded))
}

/// Commits the sketch's solved positions.
///
/// # Errors
///
/// [`SketchError::Unknown`] for a constraint that names no entity; [`SketchError::NotFinite`] for a
/// position that is NaN or infinite; [`SketchError::Numeric`] for a position past the grid or a
/// form past its budget; [`SketchError::Degenerate`] for a line whose two ends are one point, or a
/// circle of no radius; [`SketchError::OutOfMemory`]; [`SketchError::Stopped`] when the meter stops
/// it.
#[allow(
    clippy::too_many_lines,
    reason = "one straight sequence of the commit plan's passes, in the order chapter 15.10 gives them"
)]
pub fn commit(sketch: &Sketch, meter: &mut WorkMeter<'_>) -> Result<Committed, SketchError> {
    let ents = sketch.entities();
    let cons = sketch.constraints();
    meter.charge(cost(ents.len()).saturating_add(cost(cons.len()).saturating_mul(4)))?;
    let mut index: Vec<(ItemId, usize)> = Vec::new();
    index
        .try_reserve_exact(ents.len())
        .map_err(|_| SketchError::OutOfMemory)?;
    for (i, e) in ents.iter().enumerate() {
        index.push((e.id, i));
    }
    index.sort_unstable();
    let mut previous: Option<ItemId> = None;
    for &(id, _) in &index {
        if previous == Some(id) {
            return Err(SketchError::Internal);
        }
        previous = Some(id);
    }
    let mut line_ends = Vec::new();
    for e in ents {
        let both = match &e.kind {
            EntityKind::Line { start, end, .. } => {
                Some([lookup(&index, *start)?, lookup(&index, *end)?])
            }
            _ => None,
        };
        push(&mut line_ends, both)?;
    }
    let plan = Plan {
        ents,
        cons,
        index,
        line_ends,
    };
    let n = ents.len();

    // Pass 1: the exact relations between coordinates, as a weighted union-find over slots, and
    // which points coincide.
    let mut co = Offsets::new(n.checked_mul(2).ok_or(SketchError::Internal)?);
    let mut same_point = Classes::new(n);
    let mut axis = alloc::vec![Axis::default(); n];
    let mut direction = Classes::new(n);
    let mut collinear: Vec<Option<usize>> = alloc::vec![None; n];
    for c in plan.cons {
        match c.kind {
            ConstraintKind::Coincident { a, b } => {
                let (a, b) = (plan.at(a)?, plan.at(b)?);
                if plan.is_point(a) && plan.is_point(b) {
                    co.union(xs(a)?, xs(b)?, 0)?;
                    co.union(ys(a)?, ys(b)?, 0)?;
                    same_point.union(a, b, 0)?;
                }
            }
            ConstraintKind::Concentric { a, b } => {
                let (ca, cb) = (
                    centre_of(&plan, plan.at(a)?)?,
                    centre_of(&plan, plan.at(b)?)?,
                );
                if let (Some(ca), Some(cb)) = (ca, cb) {
                    co.union(xs(ca)?, xs(cb)?, 0)?;
                    co.union(ys(ca)?, ys(cb)?, 0)?;
                    same_point.union(ca, cb, 0)?;
                }
            }
            ConstraintKind::Horizontal { line } => {
                let l = plan.at(line)?;
                if let Some([p, q]) = plan.ends(l)? {
                    co.union(ys(p)?, ys(q)?, 0)?;
                    slot(&mut axis, l)?.kind = 1;
                }
            }
            ConstraintKind::Vertical { line } => {
                let l = plan.at(line)?;
                if let Some([p, q]) = plan.ends(l)? {
                    co.union(xs(p)?, xs(q)?, 0)?;
                    slot(&mut axis, l)?.kind = 2;
                }
            }
            ConstraintKind::Across { a, b, distance }
            | ConstraintKind::Upward { a, b, distance } => {
                let (a, b) = (plan.at(a)?, plan.at(b)?);
                if let (true, true, Some(d)) = (plan.is_point(a), plan.is_point(b), whole(distance))
                {
                    if matches!(c.kind, ConstraintKind::Across { .. }) {
                        co.union(xs(a)?, xs(b)?, d)?;
                    } else {
                        co.union(ys(a)?, ys(b)?, d)?;
                    }
                }
            }
            ConstraintKind::Fix {
                entity,
                slot: which,
                value,
            } => {
                let e = plan.at(entity)?;
                if plan.is_point(e) && which < 2 {
                    let s = if which == 0 { xs(e)? } else { ys(e)? };
                    co.fix(s, nearest(value)?)?;
                }
            }
            ConstraintKind::Parallel { a, b } => {
                direction.union(plan.at(a)?, plan.at(b)?, 0)?;
            }
            ConstraintKind::Perpendicular { a, b } => {
                direction.union(plan.at(a)?, plan.at(b)?, 1)?;
            }
            ConstraintKind::Collinear { a, b } => {
                let (a, b) = (plan.at(a)?, plan.at(b)?);
                direction.union(a, b, 0)?;
                let (first, later) = (a.min(b), a.max(b));
                if get(&collinear, later)?.is_none() {
                    *slot(&mut collinear, later)? = Some(first);
                }
            }
            ConstraintKind::Turned { a, b, angle } => {
                if let Some(k) = quarters(angle) {
                    direction.union(plan.at(a)?, plan.at(b)?, u8::from(k.rem_euclid(2) == 1))?;
                }
            }
            ConstraintKind::Heading { line, angle } => {
                if let Some(k) = quarters(angle) {
                    let l = plan.at(line)?;
                    if let Some([p, q]) = plan.ends(l)? {
                        if k.rem_euclid(2) == 0 {
                            co.union(ys(p)?, ys(q)?, 0)?;
                            slot(&mut axis, l)?.kind = 1;
                        } else {
                            co.union(xs(p)?, xs(q)?, 0)?;
                            slot(&mut axis, l)?.kind = 2;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Symmetry about a level or plumb line: the coordinate along the line is shared, and the other
    // is mirrored exactly (computed from masters in pass 2).
    let mut mirrors: Vec<(usize, usize, usize, u8)> = Vec::new();
    for c in plan.cons {
        if let ConstraintKind::Symmetric { a, b, line } = c.kind {
            let (a, b, l) = (plan.at(a)?, plan.at(b)?, plan.at(line)?);
            let kind = get(&axis, l)?.kind;
            if plan.is_point(a) && plan.is_point(b) && kind != 0 && plan.ends(l)?.is_some() {
                if kind == 1 {
                    co.union(xs(a)?, xs(b)?, 0)?;
                } else {
                    co.union(ys(a)?, ys(b)?, 0)?;
                }
                push(&mut mirrors, (a, b, l, kind))?;
            }
        }
    }
    // Reference points are fixed where they stand.
    for (i, e) in ents.iter().enumerate() {
        if let EntityKind::Reference { x, y } = e.kind {
            co.fix(xs(i)?, nearest(x)?)?;
            co.fix(ys(i)?, nearest(y)?)?;
        }
    }
    // The pinned points: ends and centres of arcs, centres of circles, and anything fixed.
    let mut pinned = alloc::vec![false; n];
    for (i, e) in ents.iter().enumerate() {
        match &e.kind {
            EntityKind::Circle { centre, .. } => *slot(&mut pinned, plan.at(*centre)?)? = true,
            EntityKind::Arc {
                centre, start, end, ..
            } => {
                for p in [centre, start, end] {
                    *slot(&mut pinned, plan.at(*p)?)? = true;
                }
            }
            _ => {}
        }
        if plan.is_point(i) {
            let (rx, _) = co.find(xs(i)?)?;
            let (ry, _) = co.find(ys(i)?)?;
            if get(&co.fixed, rx)?.is_some() || get(&co.fixed, ry)?.is_some() {
                *slot(&mut pinned, i)? = true;
            }
        }
    }

    // Pass 2: each coordinate group's master, rounded once from its first member's solved value.
    let mut master: Vec<Option<i64>> = alloc::vec![None; n.saturating_mul(2)];
    for i in 0..n {
        if !plan.is_point(i) {
            continue;
        }
        let at = plan.solved(i)?;
        for (s, v) in [(xs(i)?, at[0]), (ys(i)?, at[1])] {
            let (r, o) = co.find(s)?;
            if get(&master, r)?.is_none() {
                let m = match get(&co.fixed, r)? {
                    Some(f) => f,
                    None => nearest(v)?.checked_sub(o).ok_or(SketchError::Internal)?,
                };
                *slot(&mut master, r)? = Some(m);
            }
        }
    }
    // A mirrored coordinate that nothing else pins is computed from its partner's and the axis's:
    // y_b = 2 y_axis - y_a (or x), exactly, not rounded again.
    let mut size = alloc::vec![0_usize; n.saturating_mul(2)];
    if !mirrors.is_empty() {
        for i in 0..n {
            if plan.is_point(i) {
                for s in [xs(i)?, ys(i)?] {
                    let r = co.find(s)?.0;
                    let count = slot(&mut size, r)?;
                    *count = count.checked_add(1).ok_or(SketchError::Internal)?;
                }
            }
        }
    }
    for &(a, b, l, kind) in &mirrors {
        let Some([lp, _]) = plan.ends(l)? else {
            continue;
        };
        let across = |i: usize| if kind == 1 { ys(i) } else { xs(i) };
        let mut value = |s: usize| -> Result<i64, SketchError> {
            let (r, o) = co.find(s)?;
            get(&master, r)?
                .and_then(|m| m.checked_add(o))
                .ok_or(SketchError::Internal)
        };
        let (va, vl) = (value(across(a)?)?, value(across(lp)?)?);
        let (rb, ob) = co.find(across(b)?)?;
        if get(&size, rb)? == 1 && get(&co.fixed, rb)?.is_none() {
            let want = vl
                .checked_mul(2)
                .and_then(|t| t.checked_sub(va))
                .and_then(|t| t.checked_sub(ob))
                .ok_or(SketchError::Internal)?;
            *slot(&mut master, rb)? = Some(want);
            *slot(&mut size, rb)? = 2;
        }
    }
    let mut grid: Vec<Option<P2>> = alloc::vec![None; n];
    for i in 0..n {
        if !plan.is_point(i) {
            continue;
        }
        let mut coordinate = |s: usize| -> Result<i64, SketchError> {
            let (r, o) = co.find(s)?;
            get(&master, r)?
                .and_then(|m| m.checked_add(o))
                .filter(|v| v.abs() <= COORD_MAX)
                .ok_or(SketchError::Numeric(NumError::OutOfRange {
                    value: i128::MAX,
                }))
        };
        let p = P2::new(coordinate(xs(i)?)?, coordinate(ys(i)?)?)?;
        *slot(&mut grid, i)? = Some(p);
    }

    // Pass 3: carriers from masters, in authoring order. A class of lines made parallel,
    // perpendicular or collinear shares one normal, rounded once.
    let mut roots: Vec<usize> = Vec::new();
    for l in 0..n {
        push(&mut roots, direction.find(l)?.0)?;
    }
    let mut members = alloc::vec![0_usize; n];
    for &r in &roots {
        let count = slot(&mut members, r)?;
        *count = count.checked_add(1).ok_or(SketchError::Internal)?;
    }
    // Incidence of lines on point classes: a point class is a coincidence class.
    // Per point class: how many line ends are in it. Per direction class: its first line and its
    // normal if an axis fixes one. One pass each, so no line scans the others.
    let mut end_count = alloc::vec![0_usize; n];
    for l in 0..n {
        if let Some([p, q]) = plan.ends(l)? {
            for e in [p, q] {
                let r = same_point.find(e)?.0;
                let count = slot(&mut end_count, r)?;
                *count = count.checked_add(1).ok_or(SketchError::Internal)?;
            }
        }
    }
    let mut class_first: Vec<Option<usize>> = alloc::vec![None; n];
    let mut class_axis: Vec<Option<(i128, i128)>> = alloc::vec![None; n];
    for m in 0..n {
        if plan.ends(m)?.is_none() {
            continue;
        }
        let (r, turned) = direction.find(m)?;
        if get(&class_first, r)?.is_none() {
            *slot(&mut class_first, r)? = Some(m);
        }
        if get(&class_axis, r)?.is_none() {
            let own = match get(&axis, m)?.kind {
                1 => Some((0_i128, 1_i128)),
                2 => Some((1, 0)),
                _ => None,
            };
            if let Some((a, b)) = own {
                *slot(&mut class_axis, r)? = Some(if turned == 0 {
                    (a, b)
                } else {
                    unquarter(a, b)?
                });
            }
        }
    }
    let mut carriers: Vec<Option<Carrier>> = alloc::vec![None; n];
    let mut class_normal: Vec<Option<(i128, i128)>> = alloc::vec![None; n];
    for l in 0..n {
        let Some([p, q]) = plan.ends(l)? else {
            continue;
        };
        let (gp, gq) = (
            get(&grid, p)?.ok_or(SketchError::Internal)?,
            get(&grid, q)?.ok_or(SketchError::Internal)?,
        );
        let id = plan.entity(l)?.id;
        let (root, turns) = direction.find(l)?;
        let in_class = get(&members, root)? > 1;
        // Where the carrier must pass: a pinned end first, else a shared end, else the first end.
        let shared = |e: usize, same_point: &mut Classes| -> Result<bool, SketchError> {
            let r = same_point.find(e)?.0;
            Ok(get(&end_count, r)? > 1)
        };
        let anchor_end = if get(&pinned, p)? {
            0
        } else if get(&pinned, q)? {
            1
        } else if shared(p, &mut same_point)? {
            0
        } else {
            usize::from(shared(q, &mut same_point)?)
        };
        let anchor = if anchor_end == 0 { gp } else { gq };
        let axis_normal = match get(&axis, l)?.kind {
            1 => Some((0_i128, 1_i128)),
            2 => Some((1, 0)),
            _ => None,
        };
        let carrier = if let Some(first) = get(&collinear, l)?
            && let Some(c) = get(&carriers, first)?
        {
            c
        } else if !in_class {
            if let Some((a, b)) = axis_normal {
                Carrier::new(a, b, offset_of(a, b, anchor)?)?
            } else {
                if gp == gq {
                    return Err(SketchError::Degenerate(id));
                }
                let through = Carrier::through(gp, gq)?;
                let ([a, b], c) = through.coefficients();
                let g = gcd(a, b);
                if g > 1 && c.checked_rem(g) == Some(0) {
                    Carrier::new(div(a, g)?, div(b, g)?, div(c, g)?)?
                } else {
                    through
                }
            }
        } else {
            // The class's normal, in its root's frame: an axis fixes it; else the root's own line.
            if get(&class_normal, root)?.is_none() {
                let mut found: Option<(i128, i128)> = get(&class_axis, root)?;
                if found.is_none() {
                    // The first line of the class, in authoring order.
                    let first = get(&class_first, root)?.ok_or(SketchError::Internal)?;
                    let [fp, fq] = plan.ends(first)?.ok_or(SketchError::Internal)?;
                    let (gfp, gfq) = (
                        get(&grid, fp)?.ok_or(SketchError::Internal)?,
                        get(&grid, fq)?.ok_or(SketchError::Internal)?,
                    );
                    let (a, b) = if gfp == gfq {
                        let (sp, sq) = (plan.solved(fp)?, plan.solved(fq)?);
                        rounded_normal(sq[0] - sp[0], sq[1] - sp[1], plan.entity(first)?.id)?
                    } else {
                        let ([a, b], _) = Carrier::through(gfp, gfq)?.coefficients();
                        let g = gcd(a, b);
                        (div(a, g)?, div(b, g)?)
                    };
                    let turned = direction.find(first)?.1;
                    found = Some(if turned == 0 {
                        (a, b)
                    } else {
                        unquarter(a, b)?
                    });
                }
                *slot(&mut class_normal, root)? = found;
            }
            let (ra, rb) = get(&class_normal, root)?.ok_or(SketchError::Internal)?;
            let (a, b) = if turns == 0 {
                (ra, rb)
            } else {
                quarter(ra, rb)?
            };
            Carrier::new(a, b, offset_of(a, b, anchor)?)?
        };
        *slot(&mut carriers, l)? = Some(carrier);
    }

    // Pass 4: where a carrier misses a point it must pass through, the point is the meeting of the
    // carriers. A point class is every point a coincidence joins; the carriers through it are those
    // of the lines ending in it and of the lines an OnLine constraint puts it on.
    let mut through: Vec<Vec<usize>> = Vec::new();
    for _ in 0..n {
        push(&mut through, Vec::new())?;
    }
    for l in 0..n {
        if let Some([p, q]) = plan.ends(l)? {
            for e in [p, q] {
                let r = same_point.find(e)?.0;
                push(slot(&mut through, r)?, l)?;
            }
        }
    }
    for c in plan.cons {
        if let ConstraintKind::OnLine { point, line } = c.kind {
            let (p, l) = (plan.at(point)?, plan.at(line)?);
            if plan.is_point(p) && plan.ends(l)?.is_some() {
                let r = same_point.find(p)?.0;
                let list = slot(&mut through, r)?;
                if !list.contains(&l) {
                    push(list, l)?;
                }
            }
        }
    }
    let mut stored: Vec<Option<Stored>> = alloc::vec![None; n];
    for i in 0..n {
        if !plan.is_point(i) {
            continue;
        }
        let g = get(&grid, i)?.ok_or(SketchError::Internal)?;
        let r = same_point.find(i)?.0;
        let lines = through.get(r).ok_or(SketchError::Internal)?;
        meter.charge(cost(lines.len()))?;
        let mut missing = false;
        for &l in lines {
            let c = get(&carriers, l)?.ok_or(SketchError::Internal)?;
            if c.side(&Rational2::from(g))? != Sign::Zero {
                missing = true;
            }
        }
        let value = if missing && !get(&pinned, i)? {
            // Two non-parallel carriers meet; one alone gives a point sliding on it, found as its
            // meeting with the grid row or column nearest the solved position.
            let mut first: Option<Carrier> = None;
            let mut meet: Option<Rational2> = None;
            for &l in lines {
                let c = get(&carriers, l)?.ok_or(SketchError::Internal)?;
                match first {
                    None => first = Some(c),
                    Some(f) => {
                        if !f.parallel(&c)? {
                            meet = Some(f.meet(&c)?);
                            break;
                        }
                    }
                }
            }
            match (meet, first) {
                (Some(m), _) => Stored::Meet(m),
                (None, Some(f)) => {
                    let at = plan.solved(i)?;
                    let ([a, b], _) = f.coefficients();
                    let row = if a.abs() >= b.abs() {
                        Carrier::new(0, 1, i128::from(nearest(at[1])?))?
                    } else {
                        Carrier::new(1, 0, i128::from(nearest(at[0])?))?
                    };
                    Stored::Meet(f.meet(&row)?)
                }
                (None, None) => Stored::Grid(g),
            }
        } else {
            Stored::Grid(g)
        };
        *slot(&mut stored, i)? = Some(value);
    }
    // Coincident points must be one stored point: a class's members take its first member's form.
    for i in 0..n {
        if plan.is_point(i) {
            let r = same_point.find(i)?.0;
            if r != i
                && let Some(s) = get(&stored, r)?
            {
                *slot(&mut stored, i)? = Some(s);
            }
        }
    }

    // Pass 5: circles and arcs.
    let mut circles = Vec::new();
    let mut rounds = Vec::new();
    let mut arcs = Vec::new();
    for (i, e) in ents.iter().enumerate() {
        match &e.kind {
            EntityKind::Circle { centre, radius } => {
                let c = plan.at(*centre)?;
                let g = get(&grid, c)?.ok_or(SketchError::Internal)?;
                // A circle with no whole radius stated, through a grid point it is declared to
                // pass through, is stored by that defining point: its radius is exact.
                let whole_radius = plan.cons.iter().any(|k| match k.kind {
                    ConstraintKind::Radius { circle, radius } => {
                        circle == e.id && whole(radius).is_some()
                    }
                    ConstraintKind::Diameter { circle, diameter } => {
                        circle == e.id && whole(diameter).is_some()
                    }
                    _ => false,
                });
                let defining = plan.cons.iter().find_map(|k| match k.kind {
                    ConstraintKind::OnCircle { point, circle } if circle == e.id => Some(point),
                    _ => None,
                });
                let defining = match defining {
                    Some(pt) => match get(&stored, plan.at(pt)?)? {
                        Some(Stored::Grid(p)) => Some(p),
                        _ => None,
                    },
                    None => None,
                };
                if !whole_radius
                    && let Some(p) = defining
                    && p != g
                {
                    push(&mut rounds, (e.id, Round::new(p, Rational2::from(g))))?;
                } else {
                    let r = nearest(*radius)?;
                    push(&mut circles, (e.id, Circle2::new(g, i128::from(r))?))?;
                }
            }
            EntityKind::Arc {
                centre, start, end, ..
            } => {
                let (pc, ps, pe) = (plan.at(*centre)?, plan.at(*start)?, plan.at(*end)?);
                let (p, q) = (
                    get(&grid, ps)?.ok_or(SketchError::Internal)?,
                    get(&grid, pe)?.ok_or(SketchError::Internal)?,
                );
                let bisector = Arc2::bisector(p, q)?;
                // The centre: where the bisector meets the normal of a line tangent at an end,
                // else the grid row or column nearest the solved centre.
                let mut centre_at: Option<Rational2> = None;
                for c in plan.cons {
                    if let ConstraintKind::TangentAtEnd { line, arc, point } = c.kind
                        && plan.at(arc)? == i
                    {
                        let l = plan.at(line)?;
                        let end_point =
                            get(&grid, plan.at(point)?)?.ok_or(SketchError::Internal)?;
                        if let Some(lc) = get(&carriers, l)? {
                            let ([a, b], _) = lc.coefficients();
                            let normal = Carrier::new(
                                b.checked_neg().ok_or(SketchError::Internal)?,
                                a,
                                offset_of(neg(b)?, a, end_point)?,
                            )?;
                            if !normal.parallel(&bisector)? {
                                centre_at = Some(bisector.meet(&normal)?);
                                break;
                            }
                        }
                    }
                }
                let centre_at = if let Some(c) = centre_at {
                    c
                } else {
                    let at = plan.solved(pc)?;
                    let ([a, b], _) = bisector.coefficients();
                    let row = if a.abs() >= b.abs() {
                        Carrier::new(0, 1, i128::from(nearest(at[1])?))?
                    } else {
                        Carrier::new(1, 0, i128::from(nearest(at[0])?))?
                    };
                    bisector.meet(&row)?
                };
                push(&mut arcs, (e.id, Arc2::new(p, q, centre_at)?))?;
            }
            _ => {}
        }
    }

    // Pass 6: every exact relation, checked on what was stored.
    let mut out = Committed::default();
    for (i, e) in ents.iter().enumerate() {
        if let Some(s) = get(&stored, i)? {
            push(&mut out.points, (e.id, s))?;
        }
        if let Some(c) = get(&carriers, i)? {
            push(&mut out.lines, (e.id, c))?;
        }
    }
    out.circles = circles;
    out.rounds = rounds;
    out.arcs = arcs;
    for c in plan.cons {
        meter.charge(1)?;
        let mark = if c.kind.commit() == CommitClass::Exact {
            match holds(&plan, &out, &stored, &carriers, c)? {
                Some(true) => Mark::Exact,
                Some(false) => {
                    push(
                        &mut out.broken,
                        Broken {
                            constraint: c.id,
                            why: Why::Disagrees,
                        },
                    )?;
                    Mark::Bounded
                }
                None => {
                    push(
                        &mut out.broken,
                        Broken {
                            constraint: c.id,
                            why: Why::NoForm,
                        },
                    )?;
                    Mark::Bounded
                }
            }
        } else if let Some(verdict) = extra_holds(&plan, &out, &stored, &carriers, c)? {
            if verdict {
                Mark::Exact
            } else {
                push(
                    &mut out.broken,
                    Broken {
                        constraint: c.id,
                        why: Why::Disagrees,
                    },
                )?;
                Mark::Bounded
            }
        } else if radius_holds(&out, c) == Some(true) {
            Mark::Exact
        } else {
            Mark::Bounded
        };
        push(&mut out.marks, (c.id, mark))?;
    }
    Ok(out)
}

/// The rows of 15.10's table that the constraint's own class cannot decide because they need the
/// sketch: symmetry about a level or plumb line is exact when the mirrored points are grid points
/// and the line is level or plumb (`Some(false)` if the stored points are not mirrored), and a point
/// on a circle is exact when the circle's form passes through it (a defining point, or a whole radius
/// the point meets); a point that misses stays bounded, as the table says of any other point.
fn extra_holds(
    plan: &Plan<'_>,
    out: &Committed,
    stored: &[Option<Stored>],
    carriers: &[Option<Carrier>],
    c: &Constraint,
) -> Result<Option<bool>, SketchError> {
    let grid_of = |i: usize| -> Result<Option<P2>, SketchError> {
        Ok(match get(stored, i)? {
            Some(Stored::Grid(p)) => Some(p),
            _ => None,
        })
    };
    Ok(match c.kind {
        ConstraintKind::Symmetric { a, b, line } => {
            let l = plan.at(line)?;
            let (Some(k), Some([lp, _])) = (get(carriers, l)?, plan.ends(l)?) else {
                return Ok(None);
            };
            let ([ca, cb], _) = k.coefficients();
            if ca != 0 && cb != 0 {
                return Ok(None);
            }
            let (Some(pa), Some(pb), Some(pl)) =
                (grid_of(plan.at(a)?)?, grid_of(plan.at(b)?)?, grid_of(lp)?)
            else {
                return Ok(Some(false));
            };
            let mirrored =
                |from: i64, to: i64, axis: i64| from.checked_add(to) == axis.checked_mul(2);
            Some(if ca == 0 {
                pa.x == pb.x && mirrored(pa.y.get(), pb.y.get(), pl.y.get())
            } else {
                pa.y == pb.y && mirrored(pa.x.get(), pb.x.get(), pl.x.get())
            })
        }
        ConstraintKind::OnCircle { point, circle } => {
            let Some(at) = get(stored, plan.at(point)?)?.map(|s| s.rational()) else {
                return Ok(None);
            };
            let on = if let Some((_, r)) = out.rounds.iter().find(|(i, _)| *i == circle) {
                r.side(&at)? == Sign::Zero
            } else if let Some((_, k)) = out.circles.iter().find(|(i, _)| *i == circle) {
                k.side(&at)? == Sign::Zero
            } else if let Some((_, a)) = out.arcs.iter().find(|(i, _)| *i == circle) {
                a.round().side(&at)? == Sign::Zero
            } else {
                return Ok(None);
            };
            // A miss is only bounded: such a point is not a defining point.
            on.then_some(true)
        }
        _ => None,
    })
}

/// The centre point of a circle or arc entity, as an entity index.
fn centre_of(plan: &Plan<'_>, i: usize) -> Result<Option<usize>, SketchError> {
    match &plan.entity(i)?.kind {
        EntityKind::Circle { centre, .. } | EntityKind::Arc { centre, .. } => {
            Ok(Some(plan.at(*centre)?))
        }
        _ => Ok(None),
    }
}

/// A driving radius or diameter that is a whole number of steps on a full circle holds exactly when
/// the stored radius is that number.
fn radius_holds(out: &Committed, c: &Constraint) -> Option<bool> {
    let (circle, want) = match c.kind {
        ConstraintKind::Radius { circle, radius } => (circle, whole(radius)),
        ConstraintKind::Diameter { circle, diameter } => (
            circle,
            whole(diameter)
                .filter(|d| d.checked_rem(2) == Some(0))
                .and_then(|d| d.checked_div(2)),
        ),
        _ => return None,
    };
    let want = want?;
    out.circles
        .iter()
        .find(|(id, _)| *id == circle)
        .map(|(_, k)| k.radius() == i128::from(want))
}

/// Whether the exact-class constraint `c` holds on the stored geometry: `Some(true)` verified,
/// `Some(false)` it does not, `None` the committed form has nothing to check it with.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per constraint kind of section 15.10's exact rows"
)]
fn holds(
    plan: &Plan<'_>,
    out: &Committed,
    stored: &[Option<Stored>],
    carriers: &[Option<Carrier>],
    c: &Constraint,
) -> Result<Option<bool>, SketchError> {
    let point = |id: ItemId| -> Result<Option<Rational2>, SketchError> {
        Ok(get(stored, plan.at(id)?)?.map(|s| s.rational()))
    };
    let carrier =
        |id: ItemId| -> Result<Option<Carrier>, SketchError> { get(carriers, plan.at(id)?) };
    let grid_of = |id: ItemId| -> Result<Option<P2>, SketchError> {
        Ok(match get(stored, plan.at(id)?)? {
            Some(Stored::Grid(p)) => Some(p),
            _ => None,
        })
    };
    let dot = |a: &Carrier, b: &Carrier| -> Result<i128, SketchError> {
        let (([a1, b1], _), ([a2, b2], _)) = (a.coefficients(), b.coefficients());
        a1.checked_mul(a2)
            .and_then(|x| b1.checked_mul(b2).and_then(|y| x.checked_add(y)))
            .ok_or(SketchError::Numeric(NumError::BudgetExceeded))
    };
    // A line holds only if both its stored ends lie on its carrier: a carrier that misses its own
    // end would leave a gap or a sliver.
    let ends_on = |id: ItemId| -> Result<bool, SketchError> {
        let l = plan.at(id)?;
        let (Some([p, q]), Some(k)) = (plan.ends(l)?, get(carriers, l)?) else {
            return Ok(false);
        };
        for e in [p, q] {
            match get(stored, e)? {
                Some(s) if k.side(&s.rational())? == Sign::Zero => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    };
    Ok(match c.kind {
        ConstraintKind::Coincident { a, b } => match (point(a)?, point(b)?) {
            (Some(p), Some(q)) => Some(p.same(&q)?),
            _ => None,
        },
        ConstraintKind::Concentric { a, b } => {
            let centre = |id: ItemId| -> Result<Option<Rational2>, SketchError> {
                match centre_of(plan, plan.at(id)?)? {
                    Some(k) => Ok(get(stored, k)?.map(|s| s.rational())),
                    None => Ok(None),
                }
            };
            match (centre(a)?, centre(b)?) {
                (Some(p), Some(q)) => Some(p.same(&q)?),
                _ => None,
            }
        }
        ConstraintKind::OnLine { point: p, line } => match (point(p)?, carrier(line)?) {
            (Some(p), Some(l)) => Some(l.side(&p)? == Sign::Zero),
            _ => None,
        },
        ConstraintKind::Collinear { a, b } => match (carrier(a)?, carrier(b)?) {
            (Some(p), Some(q)) => Some(p.same(&q)? && ends_on(a)? && ends_on(b)?),
            _ => None,
        },
        ConstraintKind::Horizontal { line } => match carrier(line)? {
            Some(l) => Some(l.coefficients().0[0] == 0 && ends_on(line)?),
            None => None,
        },
        ConstraintKind::Vertical { line } => match carrier(line)? {
            Some(l) => Some(l.coefficients().0[1] == 0 && ends_on(line)?),
            None => None,
        },
        ConstraintKind::Parallel { a, b } => match (carrier(a)?, carrier(b)?) {
            (Some(p), Some(q)) => Some(p.parallel(&q)? && ends_on(a)? && ends_on(b)?),
            _ => None,
        },
        ConstraintKind::Perpendicular { a, b } => match (carrier(a)?, carrier(b)?) {
            (Some(p), Some(q)) => Some(dot(&p, &q)? == 0 && ends_on(a)? && ends_on(b)?),
            _ => None,
        },
        ConstraintKind::Turned { a, b, angle } => match (quarters(angle), carrier(a)?, carrier(b)?)
        {
            (Some(k), Some(p), Some(q)) => Some(
                (if k.rem_euclid(2) == 0 {
                    p.parallel(&q)?
                } else {
                    dot(&p, &q)? == 0
                }) && ends_on(a)?
                    && ends_on(b)?,
            ),
            _ => None,
        },
        ConstraintKind::Heading { line, angle } => match (quarters(angle), carrier(line)?) {
            (Some(k), Some(l)) => {
                let ([a, b], _) = l.coefficients();
                Some(if k.rem_euclid(2) == 0 { a == 0 } else { b == 0 })
            }
            _ => None,
        },
        ConstraintKind::Across { a, b, distance } | ConstraintKind::Upward { a, b, distance } => {
            match (whole(distance), grid_of(a)?, grid_of(b)?) {
                (Some(d), Some(p), Some(q)) => {
                    let (from, to) = if matches!(c.kind, ConstraintKind::Across { .. }) {
                        (p.x.get(), q.x.get())
                    } else {
                        (p.y.get(), q.y.get())
                    };
                    Some(to.checked_sub(from) == Some(d))
                }
                _ => Some(false),
            }
        }
        ConstraintKind::Fix {
            entity,
            slot: which,
            value,
        } => match grid_of(entity)? {
            Some(p) if which < 2 => {
                let v = if which == 0 { p.x.get() } else { p.y.get() };
                Some(v == nearest(value)?)
            }
            _ => Some(false),
        },
        ConstraintKind::EqualRadius { a, b } => {
            let radius = |id: ItemId| {
                out.circles
                    .iter()
                    .find(|(i, _)| *i == id)
                    .map(|(_, k)| k.radius())
            };
            match (radius(a), radius(b)) {
                (Some(p), Some(q)) => Some(p == q),
                _ => None,
            }
        }
        ConstraintKind::TangentAtEnd {
            line,
            arc,
            point: p,
        } => {
            let arc2 = out.arcs.iter().find(|(i, _)| *i == arc).map(|(_, a)| a);
            match (carrier(line)?, arc2, grid_of(p)?) {
                (Some(l), Some(a), Some(end)) => {
                    let ([la, lb], _) = l.coefficients();
                    let normal = Carrier::new(
                        lb.checked_neg().ok_or(SketchError::Internal)?,
                        la,
                        offset_of(neg(lb)?, la, end)?,
                    )?;
                    let on_arc = end == a.start() || end == a.end();
                    Some(on_arc && normal.side(&a.round().centre())? == Sign::Zero)
                }
                _ => None,
            }
        }
        _ => None,
    })
}
