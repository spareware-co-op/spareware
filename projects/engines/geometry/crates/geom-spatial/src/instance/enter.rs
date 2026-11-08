//! Entering a definition: a query region moved from the frame of the assembly that places an
//! instance into its definition's own frame, through the frames' exact shift and the placement
//! (19.12; piece G295).
//!
//! What it does:
//!
//! - [`Relation`] is how a definition's frame stands to the assembly's: the same or finer on a
//!   shared lattice (a point at `a` in the assembly's steps is at `refine·a + offset` in the
//!   definition's), or coarser (a point at `d` in the definition's steps is at
//!   `refine·d + offset` in the assembly's), from `geom-math`'s exact frame shifts (piece G48).
//!   [`Relation::outward`] gives a definition's box in the assembly's steps, and
//!   [`Relation::enter`] moves a region the other way.
//! - [`forward`] and [`backward`] carry a box through a general placement, one of the side
//!   table's rigid transforms: forward, a grid box holding the image of every point of the box;
//!   backward, a grid box holding every point of a definition's box that the transform sends
//!   within one step of a region, which is where every point lies whose image rounds into the
//!   region.
//!
//! Guarantees:
//!
//! - **Exact where the frames allow.** Moving a region into a finer frame multiplies by the
//!   refinement and adds the offset, exactly (chapter 6.1): no grid point is gained or lost. When
//!   that region does not fit the finer frame's range of ±(2^40 − 1) steps, the comparison falls
//!   back to the coarse frame (19.12): the region is grown by `refine − 1` fine steps, which is
//!   exactly what comparing each fine box rounded outward to coarse steps would find, and kept
//!   within the range, past which no geometry of the definition lies; the result is then marked
//!   not exact. Moving a region into a coarser frame divides by the refinement, which the
//!   region's denominator keeps exactly ([`Region`]): a part box between two coarse grid
//!   points still meets it exactly when its image does.
//! - **Never misses through a general placement.** [`backward`] holds every point `p` of the given
//!   box with `T·p` within one step of the region, for the exact transform `T` the side table
//!   holds: its rows are enclosed in intervals one unit in the last place wide either side of
//!   their doubles (a rational transform's doubles are its entries rounded to nearest, a certified
//!   one's are its entries), and the region is reached by the set form of Krawczyk's operator,
//!   `p ∈ M·(y − t) + (I − M·L)·p`, true for every matrix `M`, evaluated in outward-rounded
//!   interval arithmetic over the box, with `M` the doubles' inverse. A materialised vertex is
//!   `T·p` rounded once to the nearest step, so it lies in the region only if `T·p` lies within
//!   half a step of it. [`forward`] holds every point's image, and so every rounding of it.
//!
//! Method: frames and their exact shifts, chapter 6.1 and 17.1, and 19.12's frames
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`); the inclusion property of
//! interval arithmetic and Krawczyk's operator as a set enclosure, R. Krawczyk, "Newton-Algorithmen
//! zur Bestimmung von Nullstellen mit Fehlerschranken", *Computing* 4 (1969),
//! <https://doi.org/10.1007/BF02234767>, applied to a linear system as in A. Neumaier, *Interval
//! Methods for Systems of Equations*, Cambridge University Press (1990),
//! <https://doi.org/10.1017/CBO9780511526473>.
//!
//! Cost: a frame shift: a few `i128` operations per axis. A general placement: one 3×3 inverse in
//! doubles and about 90 interval operations. Nothing allocates; no recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "the names follow the formulas the header writes: a shift's refine r and offset o, a box b and its bounds l and h, and the operator's M, y, z and p"
)]

use geom_math::{Box3, FBox3, FrameRecord, FrameShift, MathError, TypedTransform};
use geom_num::grid::{COORD_MAX, Coord, P3};
use geom_num::interval::Interval;
use geom_num::{NumError, Real};

use super::error::InstanceError;
use super::region::{Region, wide};

/// How a definition's frame stands to the frame of the assembly that places it (module
/// documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Relation {
    /// The definition's frame is the assembly's, or finer: a point at `a` in the assembly's steps
    /// is at `refine·a + offset` in the definition's.
    Finer(FrameShift),
    /// The definition's frame is coarser: a point at `d` in the definition's steps is at
    /// `refine·d + offset` in the assembly's.
    Coarser(FrameShift),
}

/// The three coordinates of `p`.
fn coords(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// ⌊a / b⌋ for b > 0.
fn floor_div(a: i128, b: i128) -> Result<i128, InstanceError> {
    a.checked_div_euclid(b).ok_or(InstanceError::Defect)
}

/// ⌈a / b⌉ for b > 0.
fn ceil_div(a: i128, b: i128) -> Result<i128, InstanceError> {
    let neg = a.checked_neg().ok_or(InstanceError::Defect)?;
    floor_div(neg, b)?
        .checked_neg()
        .ok_or(InstanceError::Defect)
}

/// The grid's largest coordinate, 2^40 − 1, as an `i128` (a widening, exact).
const HIGH: i128 = COORD_MAX as i128;

/// The grid's smallest coordinate, −(2^40 − 1). Evaluated when the crate compiles.
const LOW: i128 = -HIGH;

/// `v` kept within the grid's range ±(2^40 − 1).
fn clamp(v: i128) -> i64 {
    let kept = v.clamp(LOW, HIGH);
    i64::try_from(kept).unwrap_or(0)
}

/// The box with corners `lo` and `hi`, each within the grid's range, or `None` if `lo` is above
/// `hi` on some axis.
fn grid(lo: [i128; 3], hi: [i128; 3]) -> Result<Option<Box3>, InstanceError> {
    if lo.iter().zip(&hi).any(|(l, h)| l > h) {
        return Ok(None);
    }
    let point = |v: [i128; 3]| -> Result<P3, InstanceError> {
        let [x, y, z] = v;
        Ok(P3 {
            x: Coord::from_i128(x)?,
            y: Coord::from_i128(y)?,
            z: Coord::from_i128(z)?,
        })
    };
    Ok(Some(Box3::new(point(lo)?, point(hi)?)?))
}

/// The shift's refinement and offset, in `i128`.
fn parts(s: &FrameShift) -> (i128, [i128; 3]) {
    (i128::from(s.refine()), s.offset().map(i128::from))
}

impl Relation {
    /// How `definition`'s frame stands to `assembly`'s.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Frame`] for frames with no exact shift between them in either direction:
    /// other coordinate reference systems, units, epochs, deformation models, rotations or
    /// scales, or off a shared lattice.
    pub fn between(
        assembly: &FrameRecord,
        definition: &FrameRecord,
    ) -> Result<Relation, InstanceError> {
        match assembly.shift_to(definition) {
            Ok(s) => Ok(Relation::Finer(s)),
            Err(MathError::Num(NumError::WrongDirection)) => definition
                .shift_to(assembly)
                .map(Relation::Coarser)
                .map_err(InstanceError::Frame),
            Err(e) => Err(InstanceError::Frame(e)),
        }
    }

    /// The definition's box `b`, given in its own steps, in the assembly's steps: rounded outward
    /// when the definition's frame is finer, exact when it is the same or coarser.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Num`] for a corner beyond the assembly's range.
    pub fn outward(&self, b: &Box3) -> Result<Box3, InstanceError> {
        let (lo, hi) = (coords(b.min()), coords(b.max()));
        match self {
            Relation::Finer(s) => {
                let (r, o) = parts(s);
                let mut l = [0i128; 3];
                let mut h = [0i128; 3];
                for (((ol, oh), (bl, bh)), off) in
                    l.iter_mut().zip(&mut h).zip(lo.iter().zip(&hi)).zip(o)
                {
                    let from = |v: i64| i128::from(v).checked_sub(off);
                    *ol = floor_div(from(*bl).ok_or(InstanceError::Defect)?, r)?;
                    *oh = ceil_div(from(*bh).ok_or(InstanceError::Defect)?, r)?;
                }
                grid(l, h)?.ok_or(InstanceError::Defect)
            }
            Relation::Coarser(s) => Ok(Box3::new(s.apply(b.min())?, s.apply(b.max())?)?),
        }
    }

    /// The region `r`, given in the assembly's steps, in the definition's steps, with whether the
    /// comparison there is exact; `None` when it lies wholly outside the definition frame's range,
    /// where no geometry of the definition lies (module documentation).
    ///
    /// # Errors
    ///
    /// As [`wide`], which the regions' bounds rule out.
    pub fn enter(&self, r: &Region) -> Result<Option<(Region, bool)>, InstanceError> {
        let (den, lo, hi) = (r.den(), r.lo(), r.hi());
        let mut l = [0i128; 3];
        let mut h = [0i128; 3];
        match self {
            Relation::Finer(s) => {
                let (k, o) = parts(s);
                // x / den in the assembly is (k·x + o·den) / den in the definition.
                let image = |v: i128, off: i128| {
                    wide(
                        k.checked_mul(v)
                            .zip(off.checked_mul(den))
                            .and_then(|(a, b)| a.checked_add(b)),
                    )
                };
                for (((ol, oh), (rl, rh)), off) in
                    l.iter_mut().zip(&mut h).zip(lo.iter().zip(&hi)).zip(o)
                {
                    *ol = image(*rl, off)?;
                    *oh = image(*rh, off)?;
                }
                let (low, high) = (wide(LOW.checked_mul(den))?, wide(HIGH.checked_mul(den))?);
                if l.iter().chain(&h).all(|v| (low..=high).contains(v)) {
                    return Ok(Region::new(l, h, den)?.map(|g| (g, true)));
                }
                // The coarse frame's comparison: a fine box rounded outward to coarse steps meets
                // the coarse region [x, y] exactly when the fine box meets
                // [k·⌈x⌉ + o − (k − 1), k·⌊y⌋ + o + (k − 1)] (module documentation).
                let grow = wide(k.checked_sub(1))?;
                for (((ol, oh), (rl, rh)), off) in
                    l.iter_mut().zip(&mut h).zip(lo.iter().zip(&hi)).zip(o)
                {
                    let from = |v: i128, sign: i128| {
                        wide(
                            k.checked_mul(v)
                                .and_then(|a| a.checked_add(off))
                                .zip(grow.checked_mul(sign))
                                .and_then(|(a, g)| a.checked_add(g)),
                        )
                    };
                    *ol = i128::from(clamp(from(ceil_div(*rl, den)?, -1)?));
                    *oh = i128::from(clamp(from(floor_div(*rh, den)?, 1)?));
                }
                Ok(Region::new(l, h, 1)?.map(|g| (g, grow == 0)))
            }
            Relation::Coarser(s) => {
                let (k, o) = parts(s);
                // x / den in the assembly is (x − o·den) / (k·den) in the definition.
                let below = wide(den.checked_mul(k))?;
                let (low, high) = (
                    wide(LOW.checked_mul(below))?,
                    wide(HIGH.checked_mul(below))?,
                );
                for (((ol, oh), (rl, rh)), off) in
                    l.iter_mut().zip(&mut h).zip(lo.iter().zip(&hi)).zip(o)
                {
                    let shifted = wide(off.checked_mul(den))?;
                    *ol = wide(rl.checked_sub(shifted))?.max(low);
                    *oh = wide(rh.checked_sub(shifted))?.min(high);
                }
                Ok(Region::new(l, h, below)?.map(|g| (g, true)))
            }
        }
    }
}

/// `x` and the doubles either side of it: an interval holding the real `x` was rounded to nearest
/// from.
fn widened(x: f64) -> Result<Interval, InstanceError> {
    Ok(Interval::new(x.next_down(), x.next_up())?)
}

/// The rows of `t`'s exact transform, each entry an interval holding it.
fn enclosed(t: &TypedTransform) -> Result<[[Interval; 4]; 3], InstanceError> {
    let mut rows = [[Interval::from(Real::ZERO); 4]; 3];
    for (out, row) in rows.iter_mut().zip(t.to_transform()?.rows()) {
        for (o, v) in out.iter_mut().zip(row) {
            *o = widened(v)?;
        }
    }
    Ok(rows)
}

/// The doubles of the linear part of the inverse of `t`'s doubles: any matrix serves the
/// enclosure, and this one makes `I − M·L` small.
fn approximate_inverse(t: &TypedTransform) -> Result<[[f64; 3]; 3], InstanceError> {
    let inverse = t
        .to_transform()?
        .inverse()
        .map_err(InstanceError::Transform)?;
    let mut m = [[0.0; 3]; 3];
    for (out, row) in m.iter_mut().zip(inverse.rows()) {
        for (o, v) in out.iter_mut().zip(row) {
            *o = v;
        }
    }
    Ok(m)
}

/// The intervals of a grid box's axes.
fn axes(b: &Box3) -> Result<[Interval; 3], InstanceError> {
    Ok(FBox3::from_box3(b).axes()?)
}

/// A grid box holding the image under `t` of every point of `b`: so every rounding of an image to
/// the nearest step too (module documentation).
///
/// # Errors
///
/// [`InstanceError::Num`] for an image beyond the grid's range; [`InstanceError::Transform`] for a
/// transform with no inverse.
pub fn forward(t: &TypedTransform, b: &Box3) -> Result<Box3, InstanceError> {
    let rows = enclosed(t)?;
    let p = axes(b)?;
    let mut image = [Interval::from(Real::ZERO); 3];
    for (out, row) in image.iter_mut().zip(rows) {
        let [l0, l1, l2, tr] = row;
        let [x, y, z] = p;
        *out = tr.add(l0.mul(x)?)?.add(l1.mul(y)?)?.add(l2.mul(z)?)?;
    }
    Ok(FBox3::from_intervals(image).to_grid_outward()?)
}

/// A grid box holding every point `p` of `within` with `t·p` within one step of `region`, or
/// `None` when there is none (module documentation). `within` holds the definition, in the steps
/// `t` reads.
///
/// # Errors
///
/// [`InstanceError::Transform`] for a transform with no inverse; [`InstanceError::Num`] if an
/// enclosure overflows.
pub fn backward(
    t: &TypedTransform,
    region: &Box3,
    within: &Box3,
) -> Result<Option<Box3>, InstanceError> {
    let rows = enclosed(t)?;
    let m = approximate_inverse(t)?;
    let one = Interval::new(-1.0, 1.0)?;
    let y = axes(region)?;
    let p0 = axes(within)?;
    // z = y ± 1 − t, the region grown by a step, less the translation.
    let mut z = [Interval::from(Real::ZERO); 3];
    for ((out, yi), row) in z.iter_mut().zip(y).zip(&rows) {
        let [_, _, _, tr] = *row;
        *out = yi.add(one)?.sub(tr)?;
    }
    let mut out = [Interval::from(Real::ZERO); 3];
    for (i, (o, mi)) in out.iter_mut().zip(&m).enumerate() {
        // Row i of M·z.
        let mut acc = Interval::from(Real::ZERO);
        for (mij, zj) in mi.iter().zip(z) {
            acc = acc.add(Interval::point(*mij)?.mul(zj)?)?;
        }
        // Row i of (I − M·L)·p0.
        for (j, pj) in p0.iter().enumerate() {
            let mut a = Interval::from(if i == j { Real::ONE } else { Real::ZERO });
            for (mik, row) in mi.iter().zip(&rows) {
                let lkj = *row.get(j).ok_or(InstanceError::Defect)?;
                a = a.sub(Interval::point(*mik)?.mul(lkj)?)?;
            }
            acc = acc.add(a.mul(*pj)?)?;
        }
        *o = acc;
    }
    // Only points of `within` are asked for.
    let mut kept = [Interval::from(Real::ZERO); 3];
    for ((k, o), w) in kept.iter_mut().zip(out).zip(p0) {
        let lo = if o.lo() > w.lo() { o.lo() } else { w.lo() };
        let hi = if o.hi() < w.hi() { o.hi() } else { w.hi() };
        if lo > hi {
            return Ok(None);
        }
        *k = Interval::new(lo, hi)?;
    }
    Ok(Some(FBox3::from_intervals(kept).to_grid_outward()?))
}
