//! Convexity along a chain, and where it flips (13.10, "Feasibility first", step 2, G262).
//!
//! What it does: [`classify`] reads an edge's [`Section`]s along its length (each convex, concave
//! or tangent, with certified enclosures of where it starts and ends) and cuts the edge into
//! [`Run`]s of one convexity, with a [`Pinch`] at each place the faces become tangent at a point and
//! the convexity flips: the two parts blend on opposite sides and the ball touches both faces at
//! that one point. Where the faces are tangent along a stretch with a proven extent the edge cannot
//! be blended as one: [`ConvexityError::MixedConvexity`]. [`plane_convexity`] decides the convexity
//! of an edge between two planar faces exactly, from integer normals and the edge's direction, and
//! [`corner_class`] counts the convex and concave edges that meet at a vertex.
//!
//! Guarantees:
//!
//! - **Exact for planes.** [`plane_convexity`] is the sign of (n₁ × n₂) · d on integers, in wide
//!   arithmetic with every step checked: convex when it is positive, concave when negative,
//!   [`PlaneEdge::Tangent`] when the normals are parallel and agree, [`PlaneEdge::Opposed`] when they
//!   are parallel and opposite (a knife edge, no ball fits).
//! - **A flip is a point, or it is refused.** A tangent section whose two ends are not proven apart
//!   is a pinch at the hull of its ends; one whose ends are proven apart by their enclosures is a
//!   stretch, and the edge is [`ConvexityError::MixedConvexity`] with the edge named. A tangent
//!   section between runs of the same convexity is no pinch: the edge stays one run.
//! - **Runs tile the edge.** The runs follow the sections in order, each starting where the one
//!   before ends, and together cover every convex or concave section; none overlaps another.
//! - **Counts are exact.** [`corner_class`] reports how many edges meet at a vertex and how many of
//!   them are convex and concave.
//!
//! Method: chapter 13.10 of the kernel architecture, "Feasibility first", step 2
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`), which rejects, as
//! `OpenCascade`'s offset does, an edge partly sharp and partly tangent along a stretch; the sign of
//! (n₁ × n₂) · d is the standard test of a dihedral edge's convexity, here on integers.
//!
//! Cost: linear in the number of sections. The exact test is a few wide multiplications. No
//! recursion.

use alloc::vec::Vec;
use core::fmt;

use geom_num::NumError;
use geom_num::interval::Interval;
use geom_num::sign::Sign;
use geom_num::wide::I256;

use crate::chain::EdgeId;

/// Which way an edge bends the material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Convexity {
    /// The faces fold away from each other like the outside of a box.
    Convex,
    /// The faces fold towards each other like the inside of a box.
    Concave,
}

/// What a section of an edge is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionKind {
    /// Convex or concave.
    Bent(Convexity),
    /// The faces are tangent there.
    Tangent,
}

/// A stretch of an edge, from where it starts to where it ends along the edge's parameter, each
/// certified by an enclosure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    /// What it is.
    pub kind: SectionKind,
    /// Where it starts.
    pub from: Interval,
    /// Where it ends.
    pub to: Interval,
}

/// A part of an edge of one convexity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    /// The edge.
    pub edge: EdgeId,
    /// Its convexity.
    pub convexity: Convexity,
    /// Where it starts.
    pub from: Interval,
    /// Where it ends.
    pub to: Interval,
}

/// A point where an edge's convexity flips and the blend pinches to zero width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pinch {
    /// The edge.
    pub edge: EdgeId,
    /// Where, as an enclosure.
    pub at: Interval,
}

/// An edge cut at its flips.
#[derive(Clone, Debug, PartialEq)]
pub struct Classified {
    /// The runs, in order along the edge.
    pub runs: Vec<Run>,
    /// The pinches between them, in order.
    pub pinches: Vec<Pinch>,
}

/// Why an edge could not be classified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvexityError {
    /// The faces are tangent along a stretch of the edge, not at a point.
    MixedConvexity(EdgeId),
    /// The edge has no convex or concave section to blend.
    NothingToBlend(EdgeId),
}

impl fmt::Display for ConvexityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvexityError::MixedConvexity(_) => {
                f.write_str("the faces are tangent along a stretch of the edge, not at a point")
            }
            ConvexityError::NothingToBlend(_) => {
                f.write_str("the edge has no convex or concave stretch to blend")
            }
        }
    }
}

impl core::error::Error for ConvexityError {}

/// The smallest interval holding both.
fn hull(a: Interval, b: Interval) -> Result<Interval, NumError> {
    let lo = if a.lo() < b.lo() { a.lo() } else { b.lo() };
    let hi = if a.hi() > b.hi() { a.hi() } else { b.hi() };
    Interval::new(lo, hi)
}

/// Cuts an edge at its flips.
///
/// # Errors
///
/// [`ConvexityError::MixedConvexity`] for a tangent section with a proven extent between two
/// bent sections, or at either end of an edge that is otherwise bent;
/// [`ConvexityError::NothingToBlend`] for an edge with no bent section.
pub fn classify(edge: EdgeId, sections: &[Section]) -> Result<Classified, ConvexityError> {
    let mut runs: Vec<Run> = Vec::new();
    let mut pinches: Vec<Pinch> = Vec::new();
    // A tangent section waiting for what follows it.
    let mut waiting: Option<Section> = None;
    for section in sections {
        match section.kind {
            SectionKind::Tangent => {
                // A stretch with a proven extent can never be blended as part of a run.
                if section.from.hi() < section.to.lo() {
                    return Err(ConvexityError::MixedConvexity(edge));
                }
                waiting = Some(*section);
            }
            SectionKind::Bent(convexity) => {
                match runs.last_mut() {
                    Some(last) if last.convexity == convexity => {
                        last.to = section.to;
                    }
                    Some(last) => {
                        let at = match waiting {
                            Some(t) => hull(t.from, t.to)
                                .map_err(|_| ConvexityError::NothingToBlend(edge))?,
                            None => hull(last.to, section.from)
                                .map_err(|_| ConvexityError::NothingToBlend(edge))?,
                        };
                        pinches.push(Pinch { edge, at });
                        runs.push(Run {
                            edge,
                            convexity,
                            from: section.from,
                            to: section.to,
                        });
                    }
                    None => runs.push(Run {
                        edge,
                        convexity,
                        from: section.from,
                        to: section.to,
                    }),
                }
                waiting = None;
            }
        }
    }
    if runs.is_empty() {
        return Err(ConvexityError::NothingToBlend(edge));
    }
    Ok(Classified { runs, pinches })
}

/// How two planar faces meet along an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaneEdge {
    /// A convex edge.
    Convex,
    /// A concave edge.
    Concave,
    /// The faces are tangent: parallel normals that agree.
    Tangent,
    /// The faces fold flat onto each other: parallel normals that are opposite.
    Opposed,
}

/// The cross product of two triples of integers below 2^84 in size, in wide integers.
fn cross(a: &[i128; 3], b: &[i128; 3]) -> Result<[I256; 3], NumError> {
    let w = |v: i128| I256::from_i128(v);
    let part = |p: i128, q: i128, r: i128, s: i128| -> Result<I256, NumError> {
        let left = w(p).checked_mul(&w(q)).ok_or(NumError::BudgetExceeded)?;
        let right = w(r).checked_mul(&w(s)).ok_or(NumError::BudgetExceeded)?;
        left.checked_sub(&right).ok_or(NumError::BudgetExceeded)
    };
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([
        part(a1, b2, a2, b1)?,
        part(a2, b0, a0, b2)?,
        part(a0, b1, a1, b0)?,
    ])
}

/// The sign of (n₁ × n₂) · d for outward face normals n₁ and n₂ (each below 2^83 in size, as a
/// plane's normal is) and the edge's direction d (below 2^41, a difference of grid points) as the
/// first face's boundary walks it, counter-clockwise seen from outside.
///
/// Bit budget: each component of n₁ × n₂ is below 2^167 and d is below 2^41, so a product is below
/// 2^208 and the sum of three below 2^210, in [`I256`].
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] only for inputs outside those sizes.
pub fn plane_convexity(
    first: [i128; 3],
    second: [i128; 3],
    direction: [i128; 3],
) -> Result<PlaneEdge, NumError> {
    let c = cross(&first, &second)?;
    if c.iter().all(I256::is_zero) {
        // Parallel normals: the dot product's sign says whether they agree.
        let dot = first
            .iter()
            .zip(second.iter())
            .try_fold(I256::from_i128(0), |sum, (p, q)| {
                let term = I256::from_i128(*p).checked_mul(&I256::from_i128(*q))?;
                sum.checked_add(&term)
            })
            .ok_or(NumError::BudgetExceeded)?;
        return Ok(if dot.signum() == Sign::Negative {
            PlaneEdge::Opposed
        } else {
            PlaneEdge::Tangent
        });
    }
    let total = c
        .iter()
        .zip(direction.iter())
        .try_fold(I256::from_i128(0), |sum, (p, q)| {
            let term = p.checked_mul(&I256::from_i128(*q))?;
            sum.checked_add(&term)
        })
        .ok_or(NumError::BudgetExceeded)?;
    Ok(match total.signum() {
        Sign::Positive => PlaneEdge::Convex,
        Sign::Negative => PlaneEdge::Concave,
        // A nonzero cross product with a direction square to it cannot be an edge of these faces.
        Sign::Zero => PlaneEdge::Tangent,
    })
}

/// What meets at a vertex of a blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corner {
    /// How many blended edges meet there.
    pub edges: usize,
    /// How many of them are convex.
    pub convex: usize,
    /// How many are concave.
    pub concave: usize,
}

impl Corner {
    /// Whether convex and concave edges meet.
    #[must_use]
    pub const fn mixed(&self) -> bool {
        self.convex > 0 && self.concave > 0
    }
}

/// The corner formed by edges of these convexities.
#[must_use]
pub fn corner_class(edges: &[Convexity]) -> Corner {
    let convex = edges.iter().filter(|c| **c == Convexity::Convex).count();
    Corner {
        edges: edges.len(),
        convex,
        concave: edges.len().saturating_sub(convex),
    }
}
