//! The interval engine: every zero of a small system over a box, each proven alone in a box of its
//! own or flagged where it could not be decided (chapter 12.3).
//!
//! What it does: a [`Field`] gives a system's residuals at a point and over a box, its Jacobian over
//! a box, and how far a box reaches in space, from whatever a kind can bound. [`solve`] keeps boxes
//! on a work list, discards each one a caller's test rules out or whose residuals exclude zero,
//! takes a Krawczyk step, and splits what is left. `geom_num::isolate` does this for a system that
//! is written once over its scalars; kinds that are bounded through their own traits come here.
//!
//! Guarantees:
//!
//! - **A zero is proven.** A [`Zero`] has passed Krawczyk's test: K(X) ⊆ X with ‖I − Y·J(X)‖ < 1, so
//!   its `proof` box holds exactly one zero, and its `enclosure` holds that zero.
//! - **Nothing is lost.** A box is dropped only when its residuals exclude zero, when the operator
//!   proves it empty, when the caller's test rules it out, or when a proven zero's box covers it.
//!   Everything else ends as a [`Zero`] or a [`Flagged`] region.
//! - **Zeros on split lines are found once.** Each step runs on the box widened a little inside the
//!   domain, and a zero whose enclosure meets an earlier proof box is that zero.
//! - **Bounded.** The list is explicit, each box is charged, and a box that reaches less than the
//!   resolution floor, or cannot be split, is flagged rather than split again.
//! - **Enclosures apart.** A zero's enclosure reaches less than a step, and two zeros nearer than
//!   that are refined until the doubles part them.
//! - **Deterministic.** Boxes are taken lower first, the longest side split at its middle.
//!
//! Method: Krawczyk's operator ([Krawczyk 1969](https://doi.org/10.1007/BF02234767)) on a
//! subdivision work list, as `geom_num::isolate` runs it, and the small inverse is that module's
//! (Golub and Van Loan, algorithm 3.4.1). Chapter 12.3,
//! `projects/engines/geometry/architecture/12-intersections.md`, sets out the tests.
//!
//! Cost: each box is one residual range and at most one operator step, each a few evaluations of
//! the kind; O(N³) doubles for the inverse. One allocation for each list. Recursion: none.

use alloc::vec::Vec;

use geom_num::Real;
use geom_num::interval::Interval;
use geom_num::isolate::RESOLUTION_FLOOR;
use geom_num::newton::inverse;
use geom_run::WorkMeter;

use crate::error::IntersectError;

/// How much of a box's width a step widens it by on each side: a zero on a split line then stands
/// inside one box. Any small fraction would do; a power of two widens exactly.
const WIDEN: f64 = 1.0 / 64.0;

/// The least a box kept from a step reaches on each side, as a share of the domain's side: past
/// where the rounding of the kinds' bounds swamps their slopes. Any small power of two would do.
const GROW: f64 = 1.0 / 67_108_864.0;

/// The refinement steps a zero may take. Each step at least halves what is left, near a simple zero
/// far more, so this is past what the 1,074 binades of a double could need; a zero still wider is
/// kept with the enclosure it has, which is proven all the same.
const REFINE: usize = 64;

/// A system of N residuals in N unknowns, bounded however its kinds can be.
pub trait Field<const N: usize> {
    /// The residuals at the point `x`, enclosed.
    ///
    /// # Errors
    ///
    /// The kind's, or the meter's stop.
    fn at(&self, x: [f64; N], meter: &mut WorkMeter<'_>) -> Result<[Interval; N], IntersectError>;

    /// The residuals at every point of `x`, or `None` where they cannot be bounded.
    ///
    /// # Errors
    ///
    /// As [`Field::at`].
    fn over(
        &self,
        x: &[Interval; N],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; N]>, IntersectError>;

    /// The Jacobian at every point of `x`, row by row, or `None` where it cannot be bounded.
    ///
    /// # Errors
    ///
    /// As [`Field::at`].
    fn slope(
        &self,
        x: &[Interval; N],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; N]; N]>, IntersectError>;

    /// A smaller box holding every zero in `x`, or `None` if it holds none: a kind's own way of
    /// cutting a box down, such as Bézier clipping. By default, `x`.
    ///
    /// # Errors
    ///
    /// As [`Field::at`].
    fn narrow(
        &self,
        x: &[Interval; N],
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; N]>, IntersectError> {
        Ok(Some(*x))
    }

    /// How far `x` reaches in space, in grid steps: the widest side of a box around its image.
    ///
    /// # Errors
    ///
    /// As [`Field::at`].
    fn reach(&self, x: &[Interval; N], meter: &mut WorkMeter<'_>) -> Result<f64, IntersectError>;
}

/// A zero, proven the only one in its box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zero<const N: usize> {
    /// A box around the zero.
    pub enclosure: [Interval; N],
    /// A box around `enclosure` in which this zero is the only one.
    pub proof: [Interval; N],
}

/// Why a region was left undecided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reason {
    /// Neither exclusion nor the operator decided it.
    Unproven,
    /// The Jacobian is singular at the region's middle: a multiple zero, or a continuum of them.
    Singular,
    /// The Jacobian could not be bounded there.
    NotSmooth,
}

/// Two boxes.
type Pair<const N: usize> = ([Interval; N], [Interval; N]);

/// A box at the floor that may hold zeros.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flagged<const N: usize> {
    /// The box.
    pub region: [Interval; N],
    /// Why it stayed undecided.
    pub why: Reason,
}

/// What [`solve`] found.
#[derive(Clone, Debug, PartialEq)]
pub struct Solved<const N: usize> {
    /// The zeros, each proven.
    pub zeros: Vec<Zero<N>>,
    /// The regions it could not decide.
    pub flagged: Vec<Flagged<N>>,
}

/// The middle of `x`.
fn middle(x: Interval) -> f64 {
    x.lo() * 0.5 + x.hi() * 0.5
}

/// Whether `a` and `b` share a point.
fn meets(a: Interval, b: Interval) -> bool {
    a.lo() <= b.hi() && b.lo() <= a.hi()
}

/// Whether `a` lies in `b`.
fn inside(a: Interval, b: Interval) -> bool {
    b.lo() <= a.lo() && a.hi() <= b.hi()
}

/// The part `a` and `b` share, or `None`.
fn shared(a: Interval, b: Interval) -> Option<Interval> {
    let lo = Real::new(a.lo())
        .ok()?
        .total_max(Real::new(b.lo()).ok()?)
        .get();
    let hi = Real::new(a.hi())
        .ok()?
        .total_min(Real::new(b.hi()).ok()?)
        .get();
    Interval::new(lo, hi).ok()
}

/// What a step proved about a box.
enum Step<const N: usize> {
    /// No zero.
    Empty,
    /// One zero, the only one in the box, in this smaller box.
    Unique([Interval; N]),
    /// Every zero lies in this smaller box, and why the box was not settled.
    Narrowed([Interval; N], Reason),
    /// Nothing, and why.
    Stuck(Reason),
}

/// `value`, or `zero` when it is out of reach of a fixed array's index.
fn cell<const N: usize>(row: &[Interval; N], j: usize, zero: Interval) -> Interval {
    row.get(j).copied().unwrap_or(zero)
}

/// One Krawczyk step on `x`: K(X) = m − Y·f(m) + (I − Y·J(X))·(X − m).
fn step<F: Field<N>, const N: usize>(
    field: &F,
    x: &[Interval; N],
    meter: &mut WorkMeter<'_>,
) -> Result<Step<N>, IntersectError> {
    let Some(slope) = field.slope(x, meter)? else {
        return Ok(Step::Stuck(Reason::NotSmooth));
    };
    let mut centre = [[0.0; N]; N];
    for (row, values) in centre.iter_mut().zip(&slope) {
        for (slot, v) in row.iter_mut().zip(values) {
            *slot = middle(*v);
        }
    }
    let Some(inverted) = inverse(centre) else {
        return Ok(Step::Stuck(Reason::Singular));
    };
    let zero = Interval::point(0.0)?;
    let one = Interval::point(1.0)?;
    let mid = x.map(middle);
    let value = field.at(mid, meter)?;
    let mut image = *x;
    let mut contraction: f64 = 0.0;
    for (i, (slot, y)) in image.iter_mut().zip(&inverted).enumerate() {
        // m − Y·f(m)
        let mut k = Interval::point(*mid.get(i).ok_or_internal()?)?;
        for (c, f) in y.iter().zip(&value) {
            k = k.sub(Interval::point(*c)?.mul(*f)?)?;
        }
        // (I − Y·J(X))·(X − m), and the row's sum for ‖I − Y·J(X)‖.
        let mut row = 0.0;
        for j in 0..N {
            let mut entry = if i == j { one } else { zero };
            for (c, r) in y.iter().zip(&slope) {
                entry = entry.sub(Interval::point(*c)?.mul(cell(r, j, zero))?)?;
            }
            let width = cell(x, j, zero).sub(Interval::point(*mid.get(j).ok_or_internal()?)?)?;
            k = k.add(entry.mul(width)?)?;
            row += entry.lo().abs().max_of(entry.hi().abs());
        }
        contraction = contraction.max_of(row);
        *slot = k;
    }
    if x.iter().zip(&image).any(|(a, b)| !meets(*a, *b)) {
        return Ok(Step::Empty);
    }
    if contraction < 1.0 && x.iter().zip(&image).all(|(a, b)| inside(*b, *a)) {
        return Ok(Step::Unique(image));
    }
    let mut narrowed = *x;
    for (slot, k) in narrowed.iter_mut().zip(&image) {
        match shared(*slot, *k) {
            Some(both) => *slot = both,
            None => return Ok(Step::Empty),
        }
    }
    // Where I − Y·J(X) does not contract, a singular Jacobian in X cannot be ruled out.
    let why = if contraction < 1.0 {
        Reason::Unproven
    } else {
        Reason::Singular
    };
    Ok(Step::Narrowed(narrowed, why))
}

/// Comparisons of finite doubles, by total order, so nothing depends on `f64::max`'s treatment of
/// the sign of zero.
trait Larger {
    fn max_of(self, other: f64) -> f64;
    fn neg_of(self) -> f64;
}

impl Larger for f64 {
    fn max_of(self, other: f64) -> f64 {
        if self.total_cmp(&other).is_lt() {
            other
        } else {
            self
        }
    }

    fn neg_of(self) -> f64 {
        -self
    }
}

/// An `Option`'s value, or the error of a broken invariant.
trait OrInternal<T> {
    fn ok_or_internal(self) -> Result<T, IntersectError>;
}

impl<T> OrInternal<T> for Option<T> {
    fn ok_or_internal(self) -> Result<T, IntersectError> {
        self.ok_or(IntersectError::Internal)
    }
}

/// `x` widened by [`WIDEN`] of its width on each side, inside `domain`.
fn widened<const N: usize>(
    x: &[Interval; N],
    domain: &[Interval; N],
) -> Result<[Interval; N], IntersectError> {
    let mut out = *x;
    for (slot, d) in out.iter_mut().zip(domain) {
        let pad = (slot.hi() - slot.lo()) * WIDEN;
        let wide = Interval::new(slot.lo() - pad, slot.hi() + pad)?;
        *slot = shared(wide, *d).ok_or_internal()?;
    }
    Ok(out)
}

/// `y` grown by an eighth of its width on each side, and at least [`GROW`] of the domain's, inside
/// `domain`.
pub(crate) fn grown<const N: usize>(
    y: &[Interval; N],
    domain: &[Interval; N],
) -> Result<[Interval; N], IntersectError> {
    let mut out = *y;
    for (slot, d) in out.iter_mut().zip(domain) {
        let pad = ((slot.hi() - slot.lo()) * 0.125).max_of((d.hi() - d.lo()) * GROW);
        let wide = Interval::new(slot.lo() - pad, slot.hi() + pad)?;
        *slot = shared(wide, *d).ok_or_internal()?;
    }
    Ok(out)
}

/// `x` split across its longest side relative to `domain`: the lower half and the upper; `None` if
/// that side has no double between its ends.
fn halves<const N: usize>(
    x: &[Interval; N],
    domain: &[Interval; N],
) -> Result<Option<Pair<N>>, IntersectError> {
    let mut longest = (0, f64::NEG_INFINITY);
    for (i, (a, d)) in x.iter().zip(domain).enumerate() {
        let whole = d.hi() - d.lo();
        let share = if whole > 0.0 {
            (a.hi() - a.lo()) / whole
        } else {
            0.0
        };
        if share > longest.1 {
            longest = (i, share);
        }
    }
    let side = *x.get(longest.0).ok_or_internal()?;
    let cut = middle(side);
    if cut <= side.lo() || cut >= side.hi() {
        return Ok(None);
    }
    let (mut lower, mut upper) = (*x, *x);
    *lower.get_mut(longest.0).ok_or_internal()? = Interval::new(side.lo(), cut)?;
    *upper.get_mut(longest.0).ok_or_internal()? = Interval::new(cut, side.hi())?;
    Ok(Some((lower, upper)))
}

/// The enclosure of the zero in `start`, by the operator, until it reaches less than `floor` or stops
/// shrinking.
fn refined<F: Field<N>, const N: usize>(
    field: &F,
    start: [Interval; N],
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; N], IntersectError> {
    let mut x = start;
    for _ in 0..REFINE {
        if field.reach(&x, meter)? < floor {
            break;
        }
        meter.charge(1)?;
        let next = match step(field, &x, meter)? {
            Step::Unique(next) | Step::Narrowed(next, _) => next,
            Step::Empty | Step::Stuck(_) => break,
        };
        if next == x {
            break;
        }
        x = next;
    }
    Ok(x)
}

/// Keeps the zero in `enclosure`, proven alone in `proof`, unless it is one already found.
fn found<F: Field<N>, const N: usize>(
    field: &F,
    zeros: &mut Vec<Zero<N>>,
    enclosure: [Interval; N],
    proof: [Interval; N],
    meter: &mut WorkMeter<'_>,
) -> Result<(), IntersectError> {
    let enclosure = refined(field, enclosure, RESOLUTION_FLOOR.get(), meter)?;
    let met = |z: &Zero<N>| enclosure.iter().zip(&z.proof).all(|(a, b)| meets(*a, *b));
    if !zeros.iter().any(met) {
        zeros.push(Zero { enclosure, proof });
    }
    Ok(())
}

/// Flagged boxes joined into runs, each a hull and the reason of its first box: a caller reports a
/// run as one item, since items must be disjoint, and its width is the band's.
pub(crate) fn runs<const N: usize>(
    mut regions: Vec<Flagged<N>>,
) -> Result<Vec<([Interval; N], Reason)>, IntersectError> {
    regions.sort_unstable_by(|a, b| {
        let lower = |f: &Flagged<N>| f.region.first().map_or(0.0, |x| x.lo());
        lower(a).total_cmp(&lower(b))
    });
    let touching =
        |a: &[Interval; N], b: &[Interval; N]| a.iter().zip(b).all(|(p, q)| meets(*p, *q));
    let mut out: Vec<([Interval; N], Reason)> = Vec::new();
    for f in regions {
        let mut hull = f.region;
        // Each hull it touches joins it, and a hull grown so may touch another.
        while let Some(k) = out.iter().position(|(h, _)| touching(h, &hull)) {
            let (other, _) = out.swap_remove(k);
            for (slot, r) in hull.iter_mut().zip(&other) {
                *slot = Interval::new(
                    (-slot.lo()).max_of(-r.lo()).neg_of(),
                    slot.hi().max_of(r.hi()),
                )?;
            }
        }
        out.push((hull, f.why));
    }
    Ok(out)
}

/// Every zero of `field` in `domain`, past what `keep` rules out: a box for which `keep` says no is
/// dropped with whatever zeros it holds. A caller's test is what lets a search stop early.
///
/// # Errors
///
/// The field's, `keep`'s, or the meter's stop: [`IntersectError::Cancelled`] or
/// [`IntersectError::BudgetExceeded`].
pub fn solve<F, K, const N: usize>(
    field: &F,
    domain: [Interval; N],
    mut keep: K,
    meter: &mut WorkMeter<'_>,
) -> Result<Solved<N>, IntersectError>
where
    F: Field<N>,
    K: FnMut(&[Interval; N], &mut WorkMeter<'_>) -> Result<bool, IntersectError>,
{
    let (mut zeros, mut flagged) = (Vec::<Zero<N>>::new(), Vec::new());
    let mut work = Vec::new();
    work.push(domain);
    while let Some(x) = work.pop() {
        meter.charge(1)?;
        meter.poll()?;
        let known = |z: &Zero<N>| x.iter().zip(&z.proof).all(|(a, b)| inside(*a, *b));
        if zeros.iter().any(known) || !keep(&x, meter)? {
            continue;
        }
        let range = field.over(&x, meter)?;
        if range.is_some_and(|r| r.iter().any(|v| v.lo() > 0.0 || v.hi() < 0.0)) {
            continue;
        }
        // The operator first: a clip can take a box below what the rounding of the kinds lets the
        // operator prove, and the operator settles a simple zero from far wider boxes.
        let wide = widened(&x, &domain)?;
        let why = match step(field, &wide, meter)? {
            Step::Empty => continue,
            Step::Unique(enclosure) => {
                found(field, &mut zeros, enclosure, wide, meter)?;
                continue;
            }
            Step::Narrowed(y, why) => {
                // Worth keeping only if it cut a side to half or less, or the box would split
                // anyway and lose the work. What is kept is grown: a zero on a split line then
                // stays inside it, and no side falls to where rounding swamps the kinds' slopes.
                let kept = grown(&y, &domain)?;
                let halved = kept
                    .iter()
                    .zip(&wide)
                    .any(|(a, b)| (a.hi() - a.lo()) * 2.0 <= b.hi() - b.lo() && b.hi() > b.lo());
                if halved {
                    work.push(kept);
                    continue;
                }
                why
            }
            Step::Stuck(why) => why,
        };
        // A clip keeps every zero, so a good one replaces the box. One that removes too little is
        // left for bisection, as 12.5 says.
        let Some(clipped) = field.narrow(&x, meter)? else {
            continue;
        };
        let cut = clipped
            .iter()
            .zip(&x)
            .any(|(a, b)| (a.hi() - a.lo()) * 2.0 <= b.hi() - b.lo() && b.hi() > b.lo());
        if cut {
            work.push(clipped);
            continue;
        }
        if field.reach(&x, meter)? <= RESOLUTION_FLOOR.get() {
            flagged.push(Flagged { region: x, why });
            continue;
        }
        match halves(&x, &domain)? {
            Some((lower, upper)) => {
                work.push(upper);
                work.push(lower);
            }
            None => flagged.push(Flagged { region: x, why }),
        }
    }
    // Two zeros nearer than a step share the floor's enclosures, and tighter ones tell them apart.
    let boxes: Vec<[Interval; N]> = zeros.iter().map(|z| z.enclosure).collect();
    for (i, z) in zeros.iter_mut().enumerate() {
        let near = boxes
            .iter()
            .enumerate()
            .any(|(j, b)| j != i && b.iter().zip(&z.enclosure).all(|(p, q)| meets(*p, *q)));
        if near {
            z.enclosure = refined(field, z.enclosure, 0.0, meter)?;
        }
    }
    Ok(Solved { zeros, flagged })
}
