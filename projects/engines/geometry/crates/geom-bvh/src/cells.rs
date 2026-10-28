//! The Morton-sorted cell index (piece G55): grid points in a linear octree, for point searches
//! such as welding and envelopes (chapter 19.9).
//!
//! A point's code is its Morton code: each coordinate offset by 2^40, which makes a grid
//! coordinate's 41 bits non-negative, and the three interleaved into one 123-bit number (x in bits
//! 0, 3, 6, ..., y in bits 1, 4, ..., z in bits 2, 5, ...), so points near each other in space are
//! mostly near each other in the order. The cell of side 2^L whose corner's offset coordinates are
//! multiples of 2^L holds exactly the codes that share their top 123 − 3L bits: one contiguous run
//! of the sorted codes. So the index is a linear octree with nothing stored but the points sorted
//! by (code, key), and every cell is two binary searches away.
//!
//! What it does:
//!
//! - [`CellIndex::build`] sorts the points by key (skipped when they are already in strictly
//!   increasing key order) and refuses a repeated key, then codes and sorts runs of 32,768 points
//!   as one `geom-run` phase and merges the runs in pairs, a phase per round. What is sorted is
//!   each point's (code, position in key order), all distinct, so the order is (code, key) and
//!   unique: the same for any pool, any input order and any sorting algorithm. `core`'s unstable
//!   sort sorts each run, in place: `geom-run`'s general stable sort calls a key function per
//!   comparison and was five times slower on ten million points.
//! - [`CellIndex::within`] answers the keys of the points at most a radius r from a grid point,
//!   decided exactly on squared integer distances, ascending. Its search box [c − r, c + r] meets
//!   at most three cells of side 2^L on each axis for the smallest L with 2^L > r, and the cells
//!   the ball does not reach are skipped: at most 27 runs are scanned.
//! - [`CellIndex::pairs_within`] answers every pair of points at most r apart, as (smaller key,
//!   larger key), ascending, the points' searches run as one `geom-run` phase.
//!
//! Guarantees:
//!
//! - **The same answer everywhere.** The order (code, key) is unique for distinct keys, so the
//!   index, its bytes and every answer are the same for any input order, pool and target; every
//!   answer is sorted by key.
//! - **Exact.** Distances are compared as squared integer distances in `i128` (each coordinate
//!   difference below 2^41 + 2^42, so the sum of squares stays below 2^88), never in floating point.
//! - **Bounded and cancellable.** No recursion; the meter is charged one unit per point a search
//!   scans, so a cancelled or over-budget search returns an error and no keys.
//! - **Safe.** No `unsafe`, no panics, no unchecked indexing; allocations are fallible.
//!
//! Method: the Morton order (G. M. Morton, "A computer oriented geodetic data base and a new
//! technique in file sequencing", IBM Ltd., Ottawa (1966)) and the linear octree, the leaves of an
//! octree kept as a sorted list of location codes (I. Gargantini, "An effective way to represent
//! quadtrees", Communications of the ACM 25(12) (1982), 905–910,
//! <https://doi.org/10.1145/358728.358741>). The bits are spread by the mask-and-shift sequence for
//! 21-bit coordinates, applied to the low 21 and the high 20 bits of each.
//!
//! Cost: building is O(n log n): a sort by key when the input is not in key order, n codes, the
//! runs' sorts and ⌈log₂(n/32,768)⌉ rounds of merges, with a copy of the points in key order and
//! two buffers of n slots (24 bytes each) at the peak of a round. A search is two binary searches per
//! cell (at most 27 cells) and one exact test per point in those cells. The pairs are one search
//! per point.

#![allow(
    clippy::many_single_char_names,
    reason = "variable names follow the formulas of the method"
)]

extern crate alloc;

use alloc::vec::Vec;

use geom_num::grid::P3;
use geom_run::{ChunkPlan, Exec, Phase, RunError, WorkMeter};

use crate::error::BvhError;

/// The bits of one offset coordinate: 41, for grid coordinates within ±(2^40 − 1).
pub const AXIS_BITS: u32 = 41;

/// The bits of a code: 123.
pub const CODE_BITS: u32 = 123;

/// The offset that makes a grid coordinate non-negative: 2^40.
const OFFSET: i64 = 1 << 40;

/// A radius at which every grid point is within reach of every other: 2^42, above √3·2^41.
const RADIUS_ALL: u64 = 1 << 42;

/// Chunks of the code phase and the sorts.
const PLAN: ChunkPlan = ChunkPlan::fixed::<4096>();

/// Chunks of the pair phase: one search per item, so smaller chunks.
const PAIR_PLAN: ChunkPlan = ChunkPlan::fixed::<256>();

/// Points per run of the code sort: each run is coded and sorted on its own.
const RUN: usize = 1 << 15;

/// One run, or one pair of runs, per chunk.
const ONE: ChunkPlan = ChunkPlan::fixed::<1>();

const SORT: Phase = Phase::new("cell index: sort by key");
const RUNS: Phase = Phase::new("cell index: code and sort runs");
const MERGES: Phase = Phase::new("cell index: merge runs");
const GATHER: Phase = Phase::new("cell index: gather");
const PAIRS: Phase = Phase::new("cell index: pairs");

/// A point's place in the order: its code, as its high and low 64 bits, and its position in key
/// order. Positions are distinct, so slots are distinct, and their order is (code, key): whatever
/// sorts them, the sorted order is the same.
type Slot = (u64, u64, u32);

/// A point and its persistent key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    /// The point's key: every answer is in key order.
    pub key: u64,
    /// The grid point.
    pub at: P3,
}

/// Points sorted by Morton code, then key (module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellIndex {
    codes: Vec<u128>,
    points: Vec<Point>,
}

/// The low 21 bits of `v` moved to every third bit, bit i to bit 3i: the mask-and-shift sequence
/// of the method.
const fn spread21(v: u64) -> u64 {
    let mut x = v & 0x1f_ffff;
    x = (x | (x << 32)) & 0x001f_0000_0000_ffff;
    x = (x | (x << 16)) & 0x001f_0000_ff00_00ff;
    x = (x | (x << 8)) & 0x100f_00f0_0f00_f00f;
    x = (x | (x << 4)) & 0x10c3_0c30_c30c_30c3;
    (x | (x << 2)) & 0x1249_2492_4924_9249
}

/// The 41 bits of `v` moved to every third bit of a `u128`: the low 21 bits to bits 0 to 60, the
/// high 20 to bits 63 to 120.
fn spread(v: u64) -> u128 {
    u128::from(spread21(v)) | (u128::from(spread21(v >> 21)) << 63)
}

/// The offset coordinate c + 2^40, in [1, 2^41 − 1] for a grid coordinate.
fn offset(c: i64) -> Result<u64, BvhError> {
    u64::try_from(c.checked_add(OFFSET).ok_or(BvhError::Defect)?).map_err(|_| BvhError::Defect)
}

/// The code of the offset coordinates, each below 2^41.
fn interleave(x: u64, y: u64, z: u64) -> u128 {
    spread(x) | (spread(y) << 1) | (spread(z) << 2)
}

/// The Morton code of a grid point: its offset coordinates' bits interleaved, x lowest (module
/// documentation). Below 2^123.
///
/// # Errors
///
/// [`BvhError::Defect`] only if an offset failed, which the grid's range rules out.
pub fn morton(p: P3) -> Result<u128, BvhError> {
    Ok(interleave(
        offset(p.x.get())?,
        offset(p.y.get())?,
        offset(p.z.get())?,
    ))
}

/// `points` in key order, with a repeated key refused.
fn sorted_by_key(
    points: &[Point],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Point>, BvhError> {
    let mut sorted = Vec::new();
    sorted
        .try_reserve_exact(points.len())
        .map_err(|_| BvhError::OutOfMemory)?;
    if points
        .windows(2)
        .all(|w| matches!(w, [a, b] if a.key < b.key))
    {
        // Already in key order with no key repeated: the sort would change nothing.
        sorted.extend_from_slice(points);
        return Ok(sorted);
    }
    let order = exec
        .sort_stable(meter, SORT, PLAN, points, |p: &Point| p.key)
        .map_err(|e: RunError<BvhError>| BvhError::from(e))?;
    for i in order {
        sorted.push(*points.get(i).ok_or(BvhError::Defect)?);
    }
    for pair in sorted.windows(2) {
        if let [a, b] = pair
            && a.key == b.key
        {
            return Err(BvhError::DuplicateKey(a.key));
        }
    }
    Ok(sorted)
}

/// The ranges of runs of [`RUN`] items among `n`.
fn runs_of(n: usize) -> Result<Vec<(usize, usize)>, BvhError> {
    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(n.div_ceil(RUN))
        .map_err(|_| BvhError::OutOfMemory)?;
    for start in (0..n).step_by(RUN) {
        ranges.push((start, start.saturating_add(RUN).min(n)));
    }
    Ok(ranges)
}

/// The slots of `by_key` (at most `u32::MAX` points), sorted: runs of [`RUN`] points coded and
/// sorted as one `geom-run` phase, then merged in pairs, a phase per round, until one run is left.
/// A run is sorted by `core`'s unstable sort, which allocates nothing and, the slots being
/// distinct, gives the one sorted order; the merges keep it. The meter is charged one unit per
/// point coded.
fn sorted_slots(
    by_key: &[Point],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Slot>, BvhError> {
    let ranges = runs_of(by_key.len())?;
    let mut runs = exec
        .try_map(meter, RUNS, ONE, &ranges, |_, &(start, end), m| {
            let points = by_key.get(start..end).ok_or(BvhError::Defect)?;
            m.charge(u64::try_from(points.len()).map_err(|_| BvhError::Defect)?)?;
            let mut run: Vec<Slot> = Vec::new();
            run.try_reserve_exact(points.len())
                .map_err(|_| BvhError::OutOfMemory)?;
            for (i, p) in (start..end).zip(points) {
                let code = morton(p.at)?;
                // The code is below 2^123: its high and low 64 bits.
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "the low 64 bits, kept on purpose; the high 59 are the other half"
                )]
                let low = code as u64;
                let high = u64::try_from(code >> 64).map_err(|_| BvhError::Defect)?;
                run.push((high, low, u32::try_from(i).map_err(|_| BvhError::Defect)?));
            }
            run.sort_unstable();
            Ok(run)
        })
        .map_err(|e: RunError<BvhError>| BvhError::from(e))?;
    while runs.len() > 1 {
        let mut firsts = Vec::new();
        firsts
            .try_reserve_exact(runs.len().div_ceil(2))
            .map_err(|_| BvhError::OutOfMemory)?;
        firsts.extend((0..runs.len()).step_by(2));
        let merged = exec
            .try_map(meter, MERGES, ONE, &firsts, |_, &i, _| {
                let left = runs.get(i).ok_or(BvhError::Defect)?;
                let right = runs.get(i.saturating_add(1)).map_or(&[][..], Vec::as_slice);
                merge(left, right)
            })
            .map_err(|e: RunError<BvhError>| BvhError::from(e))?;
        runs = merged;
    }
    Ok(runs.pop().unwrap_or_default())
}

/// The sorted runs `left` and `right` merged into one, left first on equal slots (which distinct
/// slots never are).
fn merge(left: &[Slot], right: &[Slot]) -> Result<Vec<Slot>, BvhError> {
    let mut out = Vec::new();
    out.try_reserve_exact(left.len().saturating_add(right.len()))
        .map_err(|_| BvhError::OutOfMemory)?;
    let (mut l, mut r) = (left.iter().peekable(), right.iter().peekable());
    loop {
        let next = match (l.peek(), r.peek()) {
            (Some(a), Some(b)) => {
                if b < a {
                    r.next()
                } else {
                    l.next()
                }
            }
            (Some(_), None) => l.next(),
            (None, Some(_)) => r.next(),
            (None, None) => break,
        };
        if let Some(s) = next {
            out.push(*s);
        }
    }
    Ok(out)
}

impl CellIndex {
    /// The index over `points` (module documentation).
    ///
    /// # Errors
    ///
    /// [`BvhError::TooManyEntries`] for more than `u32::MAX` points; [`BvhError::DuplicateKey`]
    /// for the smallest repeated key; [`BvhError::Stopped`] or
    /// [`BvhError::BudgetExceeded`] if the meter stops the build; [`BvhError::OutOfMemory`].
    pub fn build(
        points: &[Point],
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<CellIndex, BvhError> {
        if u32::try_from(points.len()).is_err() {
            return Err(BvhError::TooManyEntries);
        }
        let by_key = sorted_by_key(points, exec, meter)?;
        let slots = sorted_slots(&by_key, exec, meter)?;
        // The codes and points in slot order, gathered in runs as one phase, then joined.
        let ranges = runs_of(slots.len())?;
        let parts = exec
            .try_map(meter, GATHER, ONE, &ranges, |_, &(start, end), _| {
                let part = slots.get(start..end).ok_or(BvhError::Defect)?;
                let mut codes = Vec::new();
                codes
                    .try_reserve_exact(part.len())
                    .map_err(|_| BvhError::OutOfMemory)?;
                let mut sorted = Vec::new();
                sorted
                    .try_reserve_exact(part.len())
                    .map_err(|_| BvhError::OutOfMemory)?;
                for &(hi, lo, at) in part {
                    codes.push((u128::from(hi) << 64) | u128::from(lo));
                    let i = usize::try_from(at).map_err(|_| BvhError::Defect)?;
                    sorted.push(*by_key.get(i).ok_or(BvhError::Defect)?);
                }
                Ok((codes, sorted))
            })
            .map_err(|e: RunError<BvhError>| BvhError::from(e))?;
        let mut codes = Vec::new();
        codes
            .try_reserve_exact(slots.len())
            .map_err(|_| BvhError::OutOfMemory)?;
        let mut sorted = Vec::new();
        sorted
            .try_reserve_exact(slots.len())
            .map_err(|_| BvhError::OutOfMemory)?;
        for (c, s) in parts {
            codes.extend_from_slice(&c);
            sorted.extend_from_slice(&s);
        }
        Ok(CellIndex {
            codes,
            points: sorted,
        })
    }

    /// The points, in (code, key) order.
    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// The codes, ascending, one per point.
    #[must_use]
    pub fn codes(&self) -> &[u128] {
        &self.codes
    }

    /// How many points the index holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the index holds no point.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The keys of the points at Euclidean distance at most `radius` grid steps from `center`,
    /// ascending (module documentation).
    ///
    /// # Errors
    ///
    /// [`BvhError::Stopped`] if the meter stops; [`BvhError::OutOfMemory`].
    pub fn within(
        &self,
        center: P3,
        radius: u64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<u64>, BvhError> {
        let mut out = Vec::new();
        self.search(center, radius, meter, |p| {
            out.try_reserve(1).map_err(|_| BvhError::OutOfMemory)?;
            out.push(p.key);
            Ok(())
        })?;
        out.sort_unstable();
        Ok(out)
    }

    /// Every pair of points at Euclidean distance at most `radius` grid steps, as (smaller key,
    /// larger key), ascending (module documentation).
    ///
    /// # Errors
    ///
    /// [`BvhError::Stopped`] or [`BvhError::BudgetExceeded`] if the meter stops;
    /// [`BvhError::OutOfMemory`].
    pub fn pairs_within(
        &self,
        radius: u64,
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<(u64, u64)>, BvhError> {
        let lists = exec
            .try_map(meter, PAIRS, PAIR_PLAN, &self.points, |_, p: &Point, m| {
                let mut near = Vec::new();
                self.search(p.at, radius, m, |q| {
                    if q.key > p.key {
                        near.try_reserve(1).map_err(|_| BvhError::OutOfMemory)?;
                        near.push((p.key, q.key));
                    }
                    Ok(())
                })?;
                Ok(near)
            })
            .map_err(|e: RunError<BvhError>| BvhError::from(e))?;
        let total = lists
            .iter()
            .try_fold(0usize, |n, l| n.checked_add(l.len()))
            .ok_or(BvhError::Defect)?;
        let mut out = Vec::new();
        out.try_reserve_exact(total)
            .map_err(|_| BvhError::OutOfMemory)?;
        for l in lists {
            out.extend_from_slice(&l);
        }
        out.sort_unstable();
        Ok(out)
    }

    /// Calls `found` with every point at most `radius` from `center`, in index order, charging the
    /// meter one unit per point scanned.
    fn search<F>(
        &self,
        center: P3,
        radius: u64,
        meter: &mut WorkMeter<'_>,
        mut found: F,
    ) -> Result<(), BvhError>
    where
        F: FnMut(&Point) -> Result<(), BvhError>,
    {
        let r = radius.min(RADIUS_ALL);
        let r2 = u128::from(r)
            .checked_mul(u128::from(r))
            .ok_or(BvhError::Defect)?;
        let c = [center.x.get(), center.y.get(), center.z.get()].map(i128::from);
        // The search box on each axis, in offset coordinates clamped to [0, 2^41 − 1].
        let top: i128 = 0x1ff_ffff_ffff;
        let mut lo = [0u64; 3];
        let mut hi = [0u64; 3];
        for ((l, h), &v) in lo.iter_mut().zip(hi.iter_mut()).zip(c.iter()) {
            let o = v.checked_add(i128::from(OFFSET)).ok_or(BvhError::Defect)?;
            let a = o
                .checked_sub(i128::from(r))
                .ok_or(BvhError::Defect)?
                .clamp(0, top);
            let b = o
                .checked_add(i128::from(r))
                .ok_or(BvhError::Defect)?
                .clamp(0, top);
            *l = u64::try_from(a).map_err(|_| BvhError::Defect)?;
            *h = u64::try_from(b).map_err(|_| BvhError::Defect)?;
        }
        // The smallest level L with 2^L above half of every side of the box: the box meets at
        // most three cells of side 2^L on each axis. Sides are at most 2^41 − 1, so L ≤ 41.
        let side = lo
            .iter()
            .zip(hi.iter())
            .map(|(l, h)| h.saturating_sub(*l))
            .max()
            .unwrap_or(0);
        let level = u64::BITS
            .saturating_sub((side >> 1).leading_zeros())
            .min(AXIS_BITS);
        let span = 1u128 << level.saturating_mul(3);
        let width = (1u64 << level).saturating_sub(1);
        // The centre in offset coordinates, and its squared distance from a cell along one axis.
        let centre = c.map(|v| v.saturating_add(i128::from(OFFSET)));
        let gap = |cell: u64, at: i128| -> u128 {
            let first = i128::from(cell << level);
            let last = first.saturating_add(i128::from(width));
            let g = if at < first {
                first.saturating_sub(at)
            } else {
                at.saturating_sub(last).max(0)
            };
            g.unsigned_abs().saturating_mul(g.unsigned_abs())
        };
        let ([lx, ly, lz], [hx, hy, hz]) = (lo, hi);
        let [ox, oy, oz] = centre;
        for cx in (lx >> level)..=(hx >> level) {
            for cy in (ly >> level)..=(hy >> level) {
                for cz in (lz >> level)..=(hz >> level) {
                    // A cell the ball does not reach holds no answer.
                    let near = gap(cx, ox)
                        .saturating_add(gap(cy, oy))
                        .saturating_add(gap(cz, oz));
                    if near > r2 {
                        continue;
                    }
                    let base = interleave(cx << level, cy << level, cz << level);
                    let end = base.checked_add(span).ok_or(BvhError::Defect)?;
                    let start = self.codes.partition_point(|&k| k < base);
                    let stop = self.codes.partition_point(|&k| k < end);
                    for p in self.points.get(start..stop).ok_or(BvhError::Defect)? {
                        meter.charge(1)?;
                        let at = [p.at.x.get(), p.at.y.get(), p.at.z.get()].map(i128::from);
                        let mut d2 = 0u128;
                        for (a, b) in at.iter().zip(c.iter()) {
                            let d = a.checked_sub(*b).ok_or(BvhError::Defect)?.unsigned_abs();
                            d2 = d
                                .checked_mul(d)
                                .and_then(|s| d2.checked_add(s))
                                .ok_or(BvhError::Defect)?;
                        }
                        if d2 <= r2 {
                            found(p)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// The canonical bytes: "SKCELL1", the count as a little-endian `u64`, then for each point in
    /// index order its code (`u128`), key (`u64`) and coordinates (`i64` each), little-endian.
    ///
    /// # Errors
    ///
    /// [`BvhError::OutOfMemory`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, BvhError> {
        let mut out = Vec::new();
        let size = self
            .points
            .len()
            .checked_mul(48)
            .and_then(|n| n.checked_add(15))
            .ok_or(BvhError::OutOfMemory)?;
        out.try_reserve_exact(size)
            .map_err(|_| BvhError::OutOfMemory)?;
        out.extend_from_slice(b"SKCELL1");
        out.extend_from_slice(
            &u64::try_from(self.points.len())
                .map_err(|_| BvhError::Defect)?
                .to_le_bytes(),
        );
        for (code, p) in self.codes.iter().zip(self.points.iter()) {
            out.extend_from_slice(&code.to_le_bytes());
            out.extend_from_slice(&p.key.to_le_bytes());
            for v in [p.at.x.get(), p.at.y.get(), p.at.z.get()] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        Ok(out)
    }
}
