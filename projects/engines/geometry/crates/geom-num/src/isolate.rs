//! The certified root-isolation engine: every zero of a system of one to four equations in a box,
//! each proven to be the only one in a box of its own and refined to a requested size, or a flagged
//! box at the resolution floor where the tests could not decide. No zero is ever lost.
//!
//! What it does: [`isolate`] takes a [`System`] (residuals written once over [`Scalar`], and the
//! position in space that the unknowns stand for), a domain box, [`Options`] and the caller's
//! [`WorkMeter`]. Boxes wait on an explicit work list, a stack, never in recursion (principle 13),
//! and are taken in a fixed order: depth first, the lower half first, the first unknown before the
//! next (chapter 12.3). For each box taken:
//!
//! 1. One work unit is charged and the caller's cancellation is polled. A box inside the proof box
//!    of a zero already found is dropped: that zero is the only one there. A box whose interior
//!    meets a proof box's interior is replaced by the boxes that cover the rest of it (at most two
//!    per unknown: the parts below and above the proof box, each unknown in turn), which are put on
//!    the list as they are made, so the last made comes off first.
//! 2. The **evaluation error** is measured: the width of the enclosure of the position at the box's
//!    midpoint, which is all rounding. At a quarter step ([`EVALUATION_LIMIT`]) or more, the box
//!    and every box split from it are evaluated in double-double balls ([`crate::ball`]); still at
//!    a quarter step or more there, the engine stops with [`IsolateError::Imprecise`].
//! 3. **Exclusion:** if the enclosure of some residual over the box excludes zero, the box holds no
//!    zero. A box evaluated in double-double whose balls fail (a ball cannot record that a square
//!    is never negative) is enclosed in doubles for this test and for its image instead; the
//!    residuals failing in doubles stop the engine ([`IsolateError::Num`]), since the system must
//!    be defined over the whole domain.
//! 4. **One operator step** ([`crate::newton`], Krawczyk's or Hansen and Sengupta's, as
//!    [`Options::operator`] says), one more work unit. No zero: the box is done. A unique zero: it
//!    is settled (below). Narrowed: the engine goes on with the narrowed box, which holds every zero
//!    of the box; and when the step's contraction bound is below one, it takes one more step, one
//!    more work unit, on that box widened by half its width on each side and clipped to the domain
//!    (Rump's ε-inflation). A unique zero there is settled and ends the box. That step proves a zero
//!    lying on or next to the box's edge, such as a zero exactly on a line where boxes were split,
//!    which no box of the subdivision holds in its interior. Otherwise, with that bound below one,
//!    a narrowed box at most half as wide as the box in some unknown is put back on the list, to be
//!    taken again before anything is split, since the operator is still contracting it. (With the
//!    bound at one or above the box is split instead: narrowing it again could press one unknown
//!    onto a zero on the box's edge while another still holds two zeros, leaving a box too thin to
//!    prove anything in.) A singular midpoint Jacobian, or a Jacobian that cannot be enclosed (the
//!    residuals are not differentiable over the box, or an enclosure overflowed), leaves the box as
//!    it is.
//! 5. **The resolution floor:** a box whose image in space (the widest coordinate of the enclosure
//!    of its position) is below one grid step ([`RESOLUTION_FLOOR`]) is not split. Evaluated in
//!    doubles, it is evaluated again in double-double balls first; still undecided there, it is
//!    returned as a [`Flagged`] region with the [`Reason`] it could not be decided. Any other box
//!    is split at the midpoint of every unknown that can still be halved, into up to 2^N boxes put
//!    on the list so that the lowest comes off first. A box above the floor in which no unknown can
//!    be halved stops the engine with [`IsolateError::Unsplittable`].
//!
//! **Settling a zero.** A zero proven in a box (the proof box) with an enclosure inside it is
//! compared with every zero already found. An enclosure disjoint from another zero's is a different
//! zero. One inside the other zero's proof box, or holding the other's enclosure in its own proof
//! box, is the same zero, which is not returned twice. Otherwise, and while its image is above
//! [`Options::target`], the enclosure is refined by further operator steps on itself, one work unit
//! each. Each zero has its own cap on those steps, from chapter 12.3: with `r` the first
//! contraction bound below one, `⌈log(target / image) / log r⌉ + REFINE_MARGIN` steps; before
//! any bound below one is seen, [`REFINE_MARGIN`] steps. A step in doubles that leaves the
//! enclosure as it was is taken again in double-double balls; one that does so there, or reaching
//! the cap, stops the engine with [`IsolateError::CapReached`]. Nothing is returned as a
//! best-so-far answer. The zero's proof box is then grown: doubled about its centre and clipped to
//! the domain, one operator step and one work unit each time, for as long as the step still proves
//! a unique zero there, which is this one, its enclosure being inside. A larger proof box leaves
//! fewer boxes near the zero to cut and decide. A new zero then clears the regions flagged before
//! it: a region inside its proof box is dropped, and one whose interior meets the proof box's is
//! dropped and the rest of it put back on the list, as in step 1.
//!
//! Guarantees:
//!
//! - **No zero is lost.** Every zero of the system in the domain lies in the enclosure of a
//!   returned [`Zero`] or in a returned [`Flagged`] region. By induction over the boxes taken: each
//!   zero of the domain lies in a box on the list, a returned enclosure or a returned region. A box
//!   inside a proof box, or the part of a box or region inside one, holds at most that proof box's
//!   zero, already returned; the boxes put back for the rest cover the rest. Exclusion and the
//!   operators' `NoZero` drop only boxes with no zero; a narrowed box holds every zero of its box;
//!   an inflated box with a unique zero holds every zero of the narrowed box inside it; the halves
//!   of a box cover it; a zero proven in a proof box is returned or is the same as one returned;
//!   and refinement keeps every zero of the enclosure ([`crate::newton`]'s guarantees, in either
//!   precision).
//! - **Every returned zero is a zero, and they are distinct.** The proof box of each [`Zero`] lies
//!   in the domain and holds exactly one zero of the system, which lies in its enclosure, strictly
//!   inside the proof box ([`crate::newton`]: the operator's image strictly inside the box). A
//!   zero is returned only when its enclosure is disjoint from every enclosure returned before it,
//!   so no two returned zeros are the same. No returned region shares an interior point with a
//!   zero's proof box.
//! - **Every enclosure meets the target.** The enclosure's image is at most [`Options::target`]
//!   grid steps. Every flagged region's image is below one step.
//! - **Every cap is an error.** A refinement past its cap, a box that cannot be split, an
//!   evaluation error too large even in double-double, and the meter's work or logical-memory
//!   budget each stop the engine with an [`IsolateError`] saying which; so do the caller's
//!   cancellation and a refused allocation. No partial result is returned.
//! - **Deterministic.** The traversal, the splits, the complements, the inflation and the
//!   refinement are fixed, and all arithmetic is [`crate::newton`]'s, [`crate::interval`]'s and
//!   [`crate::ball`]'s, so every target returns the same zeros and regions in the same order, with
//!   the same bits, and charges the same work. Only cancellation depends on timing, and it returns
//!   no result.
//! - **What is charged** (chapter 23.10; changing it changes the engine's version): one work unit
//!   for each box taken from the list and one for each operator step, wherever it is taken; and
//!   logical memory for each box put on the list and each zero and region returned, at the fixed
//!   sizes of their [`LogicalSize`] implementations. The lists' real allocations are reserved
//!   fallibly, and a refusal stops the engine as [`IsolateError::OutOfMemory`].
//! - **Where it can stop short.** A zero exactly on the domain's boundary cannot be proven, since
//!   the proof box stays in the domain and must hold its zero strictly inside, so it lies in a
//!   flagged region; so do zeros closer together than the floor, multiple zeros, zeros where the
//!   residuals are not differentiable, and every zero of a domain with an unknown fixed at a point
//!   (no box is wider than a point there, so none can hold a zero strictly inside).
//!
//! Method: branch-and-prune subdivision with interval Newton operators, as R. B. Kearfott,
//! *Rigorous Global Search: Continuous Problems*, Kluwer, 1996, doi:10.1007/978-1-4757-2495-0,
//! chapter 4, and R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval
//! Analysis*, SIAM, 2009, chapter 8, doi:10.1137/1.9780898717716, describe it: a list of boxes,
//! exclusion by the range enclosure, contraction and proof by the operator, bisection of what is
//! left. Inflation of a box that does not prove its zero, after S. M. Rump, "A note on
//! epsilon-inflation", *Reliable Computing* 4(4) (1998), doi:10.1023/A:1024419816707. Proof boxes
//! as regions no other zero can share, every box and region cut against them, as the exclusion
//! regions of H. Schichl and A. Neumaier, "Exclusion regions for systems of equations", *SIAM J.
//! Numer. Anal.* 42(1) (2004), 383–408, doi:10.1137/S0036142902418898, and Kearfott's box
//! complementation (chapter 4 above). The fixed order, the floor, the evaluation limit and
//! double-double fallback, the per-zero caps and their errors are chapter 12.3's; the budgets and
//! cancellation are `geom-run`'s (chapter 23.10). Written for this kernel: it runs on the kernel's
//! own interval, ball and operator code, under its work meter.
//!
//! Cost: per box taken, a comparison with each zero found, one enclosure of the position at a point
//! and one of the residuals over the box, one operator step (`crate::newton`'s cost: a Jacobian on
//! dual intervals and O(N³) interval operations) and at most one more for inflation; double-double
//! boxes cost about ten times as much. The number of boxes depends on the system; the list holds at
//! most (2^N − 1) boxes per level of splitting, plus 2N per complement, and each zero's refinement
//! takes at most its cap of steps. Allocation: the work list and the two result lists, each
//! reserved fallibly. No recursion.

use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;

use geom_run::{Exceeded, LogicalSize, Phase, Stop, Stopped, WorkMeter};

use crate::error::NumError;
use crate::interval::{Interval, max2, min2};
use crate::ladder::COMMIT_PER_AXIS;
use crate::math;
use crate::newton::{
    self, Arithmetic, InBalls, InDoubles, NewtonError, Operator, Precision, Residuals, Verdict,
    midpoint,
};
use crate::real::Real;
use crate::scalar::Scalar;
use crate::sign::Sign;

/// The resolution floor: one grid step. A box whose image in space is smaller is not split
/// (chapter 12.3). It is the true minimum edge length of chapter 6.5's table ("the true minimum is
/// one grid step"): nothing smaller than a step can be stored, so splitting further cannot change
/// what is committed. Proposed as the rung "Resolution floor, 1q" (6.5, pending an owner's
/// decision), so it is named here, not in [`crate::ladder`], which holds only accepted rungs.
pub const RESOLUTION_FLOOR: Real = Real::ONE;

/// The evaluation limit near the floor: a quarter step. Half of commit rounding's half step
/// ([`COMMIT_PER_AXIS`]), so commit rounding stays the largest error and the two together stay
/// below three quarters of a step, inside the one-step floor (chapter 12.3). Proposed as the rung
/// "Evaluation limit near the floor, q/4" (6.5, pending), so it is named here.
pub const EVALUATION_LIMIT: Real = match Real::new(0.25) {
    Ok(r) => r,
    // A finite constant: the compile-time check below fails if this arm were ever taken.
    Err(_) => Real::ZERO,
};

/// The steps a zero's refinement may take beyond the count its contraction bound predicts, and the
/// steps it may take before any bound below one is seen. The prediction `⌈log(target / image) /
/// log r⌉` assumes the image shrinks by `r` each step, while `r` bounds the widths of the unknowns,
/// the image is an enclosure of the position over them, and the rounding of each step adds to the
/// width; eight more steps cover a factor of 2⁸ between those at `r ≤ 1/2`, and far more near a
/// simple zero, where the operators converge quadratically.
pub const REFINE_MARGIN: u64 = 8;

/// The phase the engine's budget failures name.
pub const PHASE: Phase = Phase::new("isolate zeros");

/// Checks, at compile time, the derivations of the constants: the floor is one step, and the
/// evaluation limit is half of commit rounding's half step.
const _: () = {
    assert!(RESOLUTION_FLOOR.get().to_bits() == 1.0_f64.to_bits());
    assert!(COMMIT_PER_AXIS.0 == 1);
    assert!(EVALUATION_LIMIT.get().to_bits() == (0.5_f64 * 0.5).to_bits());
};

/// A system the engine isolates the zeros of: its residuals ([`Residuals`]), and the point or
/// points of space its unknowns stand for, from which the engine measures a box's image (the
/// resolution floor) and its evaluation error.
pub trait System<const N: usize, const D: usize>: Residuals<N> {
    /// The `D` coordinates, in grid steps of the operation's frame, of the point or points of space
    /// the unknowns `x` stand for: for two curves meeting at parameters `x`, the point on each
    /// curve; for unknowns that are themselves coordinates, the unknowns.
    ///
    /// # Errors
    ///
    /// Whatever the operations return; the engine stops with it.
    fn position<S: Scalar>(&self, x: [S; N]) -> Result<[S; D], NumError>;
}

/// How the engine works on a system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// The operator of every step.
    pub operator: Operator,
    /// The size, in grid steps, each zero's enclosure is refined to: its image is at most this.
    /// Positive.
    pub target: Real,
}

impl Options {
    /// Krawczyk's operator, and enclosures refined to the resolution floor.
    pub const DEFAULT: Options = Options {
        operator: Operator::Krawczyk,
        target: RESOLUTION_FLOOR,
    };
}

/// A zero of the system, proven to be the only one in its proof box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zero<const N: usize> {
    /// A box holding the zero, whose image is at most the target.
    pub enclosure: [Interval; N],
    /// A box inside the domain in which this zero is the only one, holding the enclosure.
    pub proof: [Interval; N],
    /// The arithmetic the last step of the zero's refinement used.
    pub precision: Precision,
}

/// Why a flagged region could not be decided at the floor, even in double-double balls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reason {
    /// Neither exclusion nor the operator decided: the region may hold no zero, one, several, or a
    /// multiple one.
    Unproven,
    /// The midpoint of the region's Jacobian enclosure is singular, as at a multiple zero or a
    /// dependent system, so no operator step could be taken.
    Singular,
    /// The Jacobian could not be enclosed over the region: the residuals are not differentiable
    /// there, or an enclosure in the step overflowed.
    NotSmooth,
}

/// A box at the resolution floor that the engine could not decide: it may hold zeros.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flagged<const N: usize> {
    /// The box.
    pub region: [Interval; N],
    /// Why it could not be decided.
    pub reason: Reason,
}

/// What [`isolate`] found, in the order it found them.
#[derive(Clone, Debug, PartialEq)]
pub struct Isolation<const N: usize> {
    /// The zeros, each proven and refined.
    pub zeros: Vec<Zero<N>>,
    /// The regions at the floor that could not be decided.
    pub flagged: Vec<Flagged<N>>,
}

/// Why [`isolate`] returned no result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IsolateError<const N: usize> {
    /// The target is not positive.
    InvalidTarget,
    /// The residuals or the position failed over a box of the domain, or a box could not be
    /// formed: the system must be defined, and finite, over its whole domain.
    Num(NumError),
    /// The meter's work or logical-memory budget ran out. Deterministic.
    Exceeded(Exceeded),
    /// The caller's cancellation stopped the engine. Not deterministic; never stored.
    Cancelled,
    /// An allocation was refused. Not deterministic; never stored.
    OutOfMemory,
    /// A zero's refinement reached its cap, or stopped shrinking in double-double balls, before its
    /// image reached the target or it could be told apart from a zero found before it.
    CapReached {
        /// The zero's enclosure when it stopped.
        enclosure: [Interval; N],
    },
    /// The evaluation error at the midpoint of a box is a quarter step or more even in
    /// double-double balls.
    Imprecise {
        /// The box.
        region: [Interval; N],
    },
    /// A box above the resolution floor could not be split: no unknown has a double strictly
    /// between its bounds.
    Unsplittable {
        /// The box.
        region: [Interval; N],
    },
    /// A proof did not close: an operator step found no zero, or no preconditioner, in a box inside
    /// a zero's proof box. Only a system whose residuals are not a function of its unknowns (a
    /// defect in the caller) or a defect in geom-num can cause it; it is reported instead of a
    /// wrong answer.
    Internal {
        /// The box.
        region: [Interval; N],
    },
}

impl<const N: usize> From<NumError> for IsolateError<N> {
    fn from(e: NumError) -> Self {
        IsolateError::Num(e)
    }
}

impl<const N: usize> fmt::Display for IsolateError<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsolateError::InvalidTarget => f.write_str("the target size is not positive"),
            IsolateError::Num(e) => e.fmt(f),
            IsolateError::Exceeded(e) => e.fmt(f),
            IsolateError::Cancelled => f.write_str("cancelled by the caller"),
            IsolateError::OutOfMemory => f.write_str("an allocation was refused"),
            IsolateError::CapReached { .. } => {
                f.write_str("a zero's refinement reached its cap before the target size")
            }
            IsolateError::Imprecise { .. } => f.write_str(
                "the evaluation error is a quarter step or more even in double-double arithmetic",
            ),
            IsolateError::Unsplittable { .. } => {
                f.write_str("a box above the resolution floor cannot be split")
            }
            IsolateError::Internal { .. } => f.write_str(
                "a proof did not close inside a zero's proof box: the residuals are not a \
                 function of the unknowns, or geom-num has a defect",
            ),
        }
    }
}

/// A box on the work list and the arithmetic it is evaluated in.
#[derive(Clone, Copy, Debug)]
struct Entry<const N: usize> {
    region: [Interval; N],
    precision: Precision,
}

// Logical sizes (chapter 23.10): 16 bytes for each interval (two doubles) and 8 for a tag, the same
// on every target.
impl<const N: usize> LogicalSize for Entry<N> {
    const LOGICAL_BYTES: u64 = 16 * N as u64 + 8;
}

impl<const N: usize> LogicalSize for Zero<N> {
    const LOGICAL_BYTES: u64 = 32 * N as u64 + 8;
}

impl<const N: usize> LogicalSize for Flagged<N> {
    const LOGICAL_BYTES: u64 = 16 * N as u64 + 8;
}

/// Every zero of `system` in `domain`, each proven and refined to `options.target`, and the
/// regions at the resolution floor the engine could not decide (module documentation).
///
/// # Errors
///
/// [`IsolateError::InvalidTarget`] for a target that is not positive; the meter's stops
/// ([`IsolateError::Exceeded`], [`IsolateError::Cancelled`], [`IsolateError::OutOfMemory`]);
/// [`IsolateError::CapReached`], [`IsolateError::Imprecise`] and [`IsolateError::Unsplittable`]
/// for the engine's own caps; [`IsolateError::Num`] when the system fails on a box of the domain;
/// [`IsolateError::Internal`] for a proof that did not close.
pub fn isolate<Sys: System<N, D>, const N: usize, const D: usize>(
    system: &Sys,
    domain: [Interval; N],
    options: Options,
    meter: &mut WorkMeter<'_>,
) -> Result<Isolation<N>, IsolateError<N>> {
    const {
        assert!(1 <= N && N <= 4, "the engine takes 1 to 4 unknowns");
        assert!(D >= 1, "a position has at least one coordinate");
    };
    if options.target.get() <= 0.0 {
        return Err(IsolateError::InvalidTarget);
    }
    let mut engine = Engine {
        system,
        domain,
        options,
        meter,
        work: Vec::new(),
        found: Isolation {
            zeros: Vec::new(),
            flagged: Vec::new(),
        },
        position: PhantomData,
    };
    push(&mut engine.work, engine.meter, domain, Precision::Double)?;
    while let Some(entry) = engine.work.pop() {
        engine.take(entry)?;
    }
    Ok(engine.found)
}

/// The engine's state: the work list and what has been found.
struct Engine<'s, 'm, 'c, Sys, const N: usize, const D: usize> {
    system: &'s Sys,
    domain: [Interval; N],
    options: Options,
    meter: &'m mut WorkMeter<'c>,
    work: Vec<Entry<N>>,
    found: Isolation<N>,
    /// The number of coordinates of the system's position.
    position: PhantomData<[(); D]>,
}

/// The error a meter's stop means for the engine.
fn stopped<const N: usize>(meter: &WorkMeter<'_>, stop: Stopped) -> IsolateError<N> {
    match stop.reason() {
        Stop::OverBudget(kind) => IsolateError::Exceeded(Exceeded {
            budget: meter.cap(),
            kind,
            phase: PHASE,
            used: meter.used(),
        }),
        Stop::Cancelled => IsolateError::Cancelled,
        Stop::OutOfMemory => IsolateError::OutOfMemory,
    }
}

/// Reserve the logical memory of one value of kind `K` and room for one more value in `list`.
fn room<K: LogicalSize, T, const N: usize>(
    meter: &mut WorkMeter<'_>,
    list: &mut Vec<T>,
) -> Result<(), IsolateError<N>> {
    meter
        .reserve_for::<K>(1)
        .map_err(|stop| stopped(meter, stop))?;
    list.try_reserve(1).map_err(|_| IsolateError::OutOfMemory)
}

/// Put a box on the work list.
fn push<const N: usize>(
    work: &mut Vec<Entry<N>>,
    meter: &mut WorkMeter<'_>,
    region: [Interval; N],
    precision: Precision,
) -> Result<(), IsolateError<N>> {
    room::<Entry<N>, _, N>(meter, work)?;
    work.push(Entry { region, precision });
    Ok(())
}

/// Put on the work list the boxes covering the part of `x` outside the interior of `proof`: for
/// each unknown in turn, the part of what is left of `x` below `proof` and the part above it, then
/// what is left narrowed to `proof` in that unknown.
fn complement<const N: usize>(
    work: &mut Vec<Entry<N>>,
    meter: &mut WorkMeter<'_>,
    x: [Interval; N],
    proof: [Interval; N],
    precision: Precision,
) -> Result<(), IsolateError<N>> {
    let mut rest = x;
    for (i, (xi, pi)) in x.iter().zip(proof).enumerate() {
        if xi.lo() < pi.lo() {
            let below = with(rest, i, Interval::new(xi.lo(), pi.lo())?);
            push(work, meter, below, precision)?;
        }
        if pi.hi() < xi.hi() {
            let above = with(rest, i, Interval::new(pi.hi(), xi.hi())?);
            push(work, meter, above, precision)?;
        }
        let met = Interval::new(max2(xi.lo(), pi.lo()), min2(xi.hi(), pi.hi()))?;
        rest = with(rest, i, met);
    }
    Ok(())
}

impl<Sys: System<N, D>, const N: usize, const D: usize> Engine<'_, '_, '_, Sys, N, D> {
    /// Charge `units` work units.
    fn charge(&mut self, units: u64) -> Result<(), IsolateError<N>> {
        self.meter
            .charge(units)
            .map_err(|stop| stopped(self.meter, stop))
    }

    /// Steps 1 to 5 of the module documentation for one box.
    fn take(&mut self, entry: Entry<N>) -> Result<(), IsolateError<N>> {
        self.charge(1)?;
        self.meter
            .poll()
            .map_err(|stop| stopped(self.meter, stop))?;
        let x = entry.region;
        if self.found.zeros.iter().any(|z| inside(x, z.proof)) {
            return Ok(());
        }
        if let Some(z) = self.found.zeros.iter().find(|z| overlap(x, z.proof)) {
            return complement(&mut self.work, self.meter, x, z.proof, entry.precision);
        }
        let precision = self.precision_for(x, entry.precision)?;
        if self.excluded(x, precision)? {
            return Ok(());
        }
        let (rest, reason) = match self.operate(x, precision)? {
            Ok(Verdict::NoZero) => return Ok(()),
            Ok(Verdict::Unique {
                enclosure,
                contraction,
            }) => return self.settle(enclosure, x, contraction, precision),
            Ok(Verdict::Narrowed {
                enclosure,
                contraction,
            }) => {
                if under(contraction.get(), Real::ONE) {
                    if self.inflated(enclosure, precision)? {
                        return Ok(());
                    }
                    if halved(enclosure, x) {
                        return push(&mut self.work, self.meter, enclosure, precision);
                    }
                }
                (enclosure, Reason::Unproven)
            }
            Err(NewtonError::Singular) => (x, Reason::Singular),
            Err(NewtonError::Num(_)) => (x, Reason::NotSmooth),
        };
        if under(self.image(rest, precision)?, RESOLUTION_FLOOR) {
            return match precision {
                Precision::Double => {
                    push(&mut self.work, self.meter, rest, Precision::DoubleDouble)
                }
                Precision::DoubleDouble => {
                    room::<Flagged<N>, _, N>(self.meter, &mut self.found.flagged)?;
                    self.found.flagged.push(Flagged {
                        region: rest,
                        reason,
                    });
                    Ok(())
                }
            };
        }
        self.split(rest, precision)
    }

    /// The arithmetic `x` is evaluated in: `precision`, or double-double balls when the evaluation
    /// error in doubles is at the limit or above (step 2).
    fn precision_for(
        &self,
        x: [Interval; N],
        precision: Precision,
    ) -> Result<Precision, IsolateError<N>> {
        if under(self.evaluation_error(x, precision)?, EVALUATION_LIMIT) {
            return Ok(precision);
        }
        if precision == Precision::Double
            && under(
                self.evaluation_error(x, Precision::DoubleDouble)?,
                EVALUATION_LIMIT,
            )
        {
            return Ok(Precision::DoubleDouble);
        }
        Err(IsolateError::Imprecise { region: x })
    }

    /// The width of the enclosure of the position at the midpoint of `x`, in grid steps.
    fn evaluation_error(&self, x: [Interval; N], precision: Precision) -> Result<f64, NumError> {
        let mut m = x;
        for (mi, xi) in m.iter_mut().zip(x) {
            *mi = Interval::point(midpoint(xi))?;
        }
        self.image(m, precision)
    }

    /// The widest coordinate of the enclosure of the position over `x`, in grid steps, rounded up;
    /// in double-double, the enclosure in doubles where the balls fail ([`Engine::values`]).
    fn image(&self, x: [Interval; N], precision: Precision) -> Result<f64, NumError> {
        let position = match precision {
            Precision::Double => self.position::<InDoubles>(x)?,
            Precision::DoubleDouble => self
                .position::<InBalls>(x)
                .or_else(|_| self.position::<InDoubles>(x))?,
        };
        Ok(position
            .iter()
            .fold(0.0, |w, c| max2(w, (c.hi() - c.lo()).next_up())))
    }

    /// The enclosure of the position over `x` in the arithmetic `A`.
    fn position<A: Arithmetic>(&self, x: [Interval; N]) -> Result<[Interval; D], NumError> {
        A::lower_all(self.system.position(A::lift(x)?)?)
    }

    /// Whether the enclosure of some residual over `x` excludes zero (step 3).
    fn excluded(&self, x: [Interval; N], precision: Precision) -> Result<bool, NumError> {
        Ok(self
            .values(x, precision)?
            .iter()
            .any(|v| matches!(v.certain_sign(), Some(Sign::Positive | Sign::Negative))))
    }

    /// The enclosure of the residuals over `x` in `precision`. In double-double it is the one in
    /// doubles where the balls fail: a ball cannot record that a square is never negative, so the
    /// root of a sum of squares whose ball reaches zero is refused in balls and taken in doubles
    /// ([`crate::ball`]). Both enclose the residuals, so either serves every test.
    fn values(&self, x: [Interval; N], precision: Precision) -> Result<[Interval; N], NumError> {
        match precision {
            Precision::Double => self.residuals::<InDoubles>(x),
            Precision::DoubleDouble => self
                .residuals::<InBalls>(x)
                .or_else(|_| self.residuals::<InDoubles>(x)),
        }
    }

    /// The enclosure of the residuals over `x` in the arithmetic `A`.
    fn residuals<A: Arithmetic>(&self, x: [Interval; N]) -> Result<[Interval; N], NumError> {
        A::lower_all(self.system.residuals(A::lift(x)?)?)
    }

    /// One operator step on `x`, charged.
    fn operate(
        &mut self,
        x: [Interval; N],
        precision: Precision,
    ) -> Result<Result<Verdict<N>, NewtonError>, IsolateError<N>> {
        self.charge(1)?;
        Ok(newton::step(
            self.options.operator,
            precision,
            self.system,
            x,
        ))
    }

    /// `x` widened by half its width on each side and clipped to the domain.
    fn widened(&self, x: [Interval; N]) -> Result<[Interval; N], NumError> {
        let mut y = x;
        for ((yi, xi), di) in y.iter_mut().zip(x).zip(self.domain) {
            // Rounded to nearest, `lo − half` is at most `lo` and `hi + half` at least `hi`, so the
            // widened box holds `x`; the domain holds `x`, so clipping keeps it.
            let half = 0.5 * xi.hi() - 0.5 * xi.lo();
            *yi = Interval::new(max2(di.lo(), xi.lo() - half), min2(di.hi(), xi.hi() + half))?;
        }
        Ok(y)
    }

    /// The inflation step of step 4: whether it proved the one zero of the narrowed box `x`
    /// [`widened`](Engine::widened), which is then settled.
    fn inflated(
        &mut self,
        x: [Interval; N],
        precision: Precision,
    ) -> Result<bool, IsolateError<N>> {
        let y = self.widened(x)?;
        if let Ok(Verdict::Unique {
            enclosure,
            contraction,
        }) = self.operate(y, precision)?
        {
            self.settle(enclosure, y, contraction, precision)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Split `x` at the midpoint of every unknown that can be halved and put the boxes on the
    /// list, the lowest last so that it comes off first (step 5).
    fn split(&mut self, x: [Interval; N], precision: Precision) -> Result<(), IsolateError<N>> {
        let (boxes, count) = halves(x)?;
        if count == 1 {
            return Err(IsolateError::Unsplittable { region: x });
        }
        for b in boxes.iter().take(count).rev() {
            push(&mut self.work, self.meter, *b, precision)?;
        }
        Ok(())
    }

    /// Settle the zero proven in `proof` with enclosure `enclosure` (module documentation,
    /// "Settling a zero").
    fn settle(
        &mut self,
        enclosure: [Interval; N],
        proof: [Interval; N],
        contraction: Real,
        precision: Precision,
    ) -> Result<(), IsolateError<N>> {
        let target = self.options.target.get();
        let (mut e, mut p, mut r) = (enclosure, precision, contraction.get());
        let mut steps: u64 = 0;
        let mut cap: Option<u64> = None;
        loop {
            let size = self.image(e, p)?;
            let mut ambiguous = false;
            for z in &self.found.zeros {
                if disjoint(e, z.enclosure) {
                    continue;
                }
                if inside(e, z.proof) || inside(z.enclosure, proof) {
                    return Ok(());
                }
                ambiguous = true;
            }
            if !ambiguous && size <= target {
                break;
            }
            if cap.is_none() && under(r, Real::ONE) {
                cap = Some(
                    steps
                        .saturating_add(steps_to(size, target, r)?)
                        .saturating_add(REFINE_MARGIN),
                );
            }
            if steps >= cap.unwrap_or(REFINE_MARGIN) {
                return Err(IsolateError::CapReached { enclosure: e });
            }
            steps = steps.saturating_add(1);
            let next = match self.operate(e, p)? {
                Ok(
                    Verdict::Unique {
                        enclosure,
                        contraction,
                    }
                    | Verdict::Narrowed {
                        enclosure,
                        contraction,
                    },
                ) => {
                    r = contraction.get();
                    enclosure
                }
                Ok(Verdict::NoZero) | Err(NewtonError::Singular) => {
                    return Err(IsolateError::Internal { region: e });
                }
                Err(NewtonError::Num(err)) => return Err(IsolateError::Num(err)),
            };
            if next == e {
                if p == Precision::DoubleDouble {
                    return Err(IsolateError::CapReached { enclosure: e });
                }
                p = Precision::DoubleDouble;
            }
            e = next;
        }
        let mut proof = proof;
        loop {
            let wider = self.widened(proof)?;
            if wider == proof {
                break;
            }
            match self.operate(wider, p)? {
                Ok(Verdict::Unique { .. }) => proof = wider,
                _ => break,
            }
        }
        room::<Zero<N>, _, N>(self.meter, &mut self.found.zeros)?;
        self.found.zeros.push(Zero {
            enclosure: e,
            proof,
            precision: p,
        });
        for f in &self.found.flagged {
            // A region inside the proof box leaves nothing to examine again.
            if overlap(f.region, proof) {
                complement(
                    &mut self.work,
                    self.meter,
                    f.region,
                    proof,
                    Precision::DoubleDouble,
                )?;
            }
        }
        self.found
            .flagged
            .retain(|f| !overlap(f.region, proof) && !inside(f.region, proof));
        Ok(())
    }
}

/// The steps a contraction factor `r` in (0, 1) predicts for an image of `size` to shrink to
/// `target`: `⌈(ln target − ln size) / ln r⌉`, or 0 when it is already there.
///
/// The cast is exact: the logarithms of the sizes, positive finite doubles, lie between
/// ln 2^−1074 ≈ −744.5 and ln(2^1024) ≈ 709.8, so their difference is at most 1455 in magnitude;
/// `r` is at most 1 − 2^−53, so `ln r` is at most −2^−53 (the logarithm is correctly rounded); so
/// the quotient is at most 1455 · 2^53 < 2^64, positive, and its ceiling is a whole number.
fn steps_to(size: f64, target: f64, r: f64) -> Result<u64, NumError> {
    if size <= target {
        return Ok(0);
    }
    let ratio = (math::ln(target)?.get() - math::ln(size)?.get()) / math::ln(r)?.get();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole number in [1, 2^64), derived above"
    )]
    let steps = math::ceil(ratio)?.get() as u64;
    Ok(steps)
}

/// Whether `value` is strictly under `bound`: the one comparison behind the resolution floor (an
/// image under one step), the evaluation limit (an error under a quarter step) and a contracting
/// step (a contraction bound under one), so that each is decided the same way at its bound.
fn under(value: f64, bound: Real) -> bool {
    value < bound.get()
}

/// Whether `a`, narrowed from `x`, is at most half as wide as `x` in some unknown (and narrower: a
/// point stays a point).
fn halved<const N: usize>(a: [Interval; N], x: [Interval; N]) -> bool {
    a.iter().zip(x).any(|(ai, xi)| {
        let (wa, wx) = (ai.hi() - ai.lo(), xi.hi() - xi.lo());
        wa <= 0.5 * wx && wa < wx
    })
}

/// Whether every coordinate of `a` lies within the same coordinate of `b`: `b` holds both of its
/// ends.
fn inside<const N: usize>(a: [Interval; N], b: [Interval; N]) -> bool {
    a.iter()
        .zip(b)
        .all(|(ai, bi)| bi.contains(ai.lo()) && bi.contains(ai.hi()))
}

/// Whether the interiors of `a` and `b` meet: in every coordinate each begins before the other
/// ends.
fn overlap<const N: usize>(a: [Interval; N], b: [Interval; N]) -> bool {
    a.iter()
        .zip(b)
        .all(|(ai, bi)| ai.lo() < bi.hi() && bi.lo() < ai.hi())
}

/// Whether `a` and `b` share no point: in some coordinate neither holds the other's lower end (two
/// closed intervals meet exactly when one holds the other's lower end, the larger of the two).
fn disjoint<const N: usize>(a: [Interval; N], b: [Interval; N]) -> bool {
    a.iter()
        .zip(b)
        .any(|(ai, bi)| !ai.contains(bi.lo()) && !bi.contains(ai.lo()))
}

/// The boxes `x` splits into, in the engine's order (the first unknown most significant, the lower
/// half first), and how many there are: each unknown with a double strictly between its bounds
/// is halved at [`midpoint`], the others are kept whole. At most 2⁴ = 16 boxes.
fn halves<const N: usize>(x: [Interval; N]) -> Result<([[Interval; N]; 16], usize), NumError> {
    let mut boxes = [x; 16];
    let mut count: usize = 1;
    for (i, xi) in x.iter().enumerate() {
        let m = midpoint(*xi);
        if m <= xi.lo() || m >= xi.hi() {
            continue;
        }
        let (lower, upper) = (Interval::new(xi.lo(), m)?, Interval::new(m, xi.hi())?);
        let before = boxes;
        let doubled = before
            .iter()
            .take(count)
            .flat_map(|b| [with(*b, i, lower), with(*b, i, upper)]);
        for (slot, child) in boxes.iter_mut().zip(doubled) {
            *slot = child;
        }
        // At most 2^N ≤ 16.
        count = count.saturating_mul(2);
    }
    Ok((boxes, count))
}

/// `b` with coordinate `i` replaced by `v`.
fn with<const N: usize>(b: [Interval; N], i: usize, v: Interval) -> [Interval; N] {
    let mut out = b;
    for (j, c) in out.iter_mut().enumerate() {
        if j == i {
            *c = v;
        }
    }
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may unwrap and index"
)]
mod tests {
    //! What the public tests cannot reach on purpose: a zero whose enclosure meets another zero's
    //! without either proof box holding the other's enclosure.

    use alloc::vec;

    use geom_run::Budget;

    use super::*;

    /// `(x − 1)(x − 3)`, its unknown in grid steps.
    struct Two;

    impl Residuals<1> for Two {
        fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
            let one = S::constant(Real::ONE);
            let three = S::constant(Real::new(3.0)?);
            Ok([x[0].sub(one)?.mul(x[0].sub(three)?)?])
        }
    }

    impl System<1, 1> for Two {
        fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
            Ok(x)
        }
    }

    fn iv(lo: f64, hi: f64) -> Interval {
        Interval::new(lo, hi).unwrap()
    }

    /// The zero at 3 was found with a wide enclosure, `[1.05, 3.2]`, proven in `[1.04, 3.5]`. A
    /// zero at 1 proven in `[0.8, 1.2]` with enclosure `[0.9, 1.1]` meets that enclosure, and
    /// neither proof box holds the other's enclosure: it is refined until the two enclosures part,
    /// then returned as a second zero.
    #[test]
    fn an_enclosure_meeting_another_zero_is_refined_until_they_part() {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let mut engine = Engine {
            system: &Two,
            domain: [iv(0.0, 4.0)],
            options: Options::DEFAULT,
            meter: &mut meter,
            work: Vec::new(),
            found: Isolation {
                zeros: vec![Zero {
                    enclosure: [iv(1.05, 3.2)],
                    proof: [iv(1.04, 3.5)],
                    precision: Precision::Double,
                }],
                flagged: Vec::new(),
            },
            position: PhantomData,
        };
        let r = Real::new(0.5).unwrap();
        engine
            .settle([iv(0.9, 1.1)], [iv(0.8, 1.2)], r, Precision::Double)
            .unwrap();
        assert_eq!(engine.found.zeros.len(), 2);
        let e = engine.found.zeros[1].enclosure[0];
        assert!(e.lo() <= 1.0 && 1.0 <= e.hi() && e.hi() < 1.05);
        assert!(meter.used().work > 0);
    }
}
