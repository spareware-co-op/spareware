//! The merge stage: touching entities become one, checked against their carriers (11.4, G242).
//!
//! What it does: [`merge_stage`] takes the points of an arrangement and the pairs a broad phase
//! says may touch, takes the contact rule's decision on each pair ([`crate::rule::decide`]), joins
//! the pairs that touch into clusters, and gives each cluster one representative. Every point
//! comes out as the representative of its cluster, so a later decision uses the merged entity and
//! never one of its members: merges are decided before any later predicate uses them. The
//! representative is the lowest exact grid point of the cluster if it has one, otherwise the
//! member's committed grid point that is proven nearest all the carriers of the cluster, ties to
//! the lowest point. It is accepted only if it is proven within the remaining bound of every
//! carrier of every member, measured from the carriers' definitions ([`Carriers::distance`]);
//! otherwise the merge fails with [`CoincidenceChain`].
//!
//! Guarantees:
//!
//! - **Measured against carriers, never against an earlier representative.** The proof of a
//!   representative uses the carriers of all members, which never move, so repeated merges cannot
//!   walk a vertex away: its bound is the worst distance from a carrier, not a sum of steps.
//! - **Transitive coincidence is checked.** A chain of touches a ~ b ~ c in which a and c are not
//!   within the bound of each other's carriers fails with `CoincidenceChain`; it is never merged
//!   on the strength of its links alone.
//! - **Entities proven apart stay apart.** A pair whose distance enclosure excludes zero at any
//!   refinement is never merged, however close.
//! - **Deterministic.** Pairs are taken in the order given, clusters are joined by the lowest
//!   member, and representatives are chosen by distance and then by point order, never by hash
//!   order or memory address.
//!
//! Method: the merge rule of chapter 11.4 (`projects/engines/geometry/architecture/11-booleans.md`),
//! "Merges are checked against carriers". The idea that coincidence is held as explicit relations
//! between entities, and merged in one stage, is that of `SMLib`'s `SmMerge` mates
//! (<https://github.com/NVIDIA-Omniverse/usd-brep>); its tolerance growth on relating is not taken.
//!
//! Cost: one decision per pair, one projection per member and carrier. All of it is charged to the
//! meter. No recursion.

use alloc::vec::Vec;

use geom_num::P3;
use geom_run::WorkMeter;

use crate::carrier::{Bound, CarrierIx, PointGeom, SortedVec};
use crate::distance::{Carriers, separation};
use crate::rule::{CoincidenceChain, Contact, ContactError, EPS_FIT, REFINEMENTS, decide};

/// The disjoint sets of point indices that touch, joined in the order of the pairs.
struct Clusters {
    parent: Vec<usize>,
}

impl Clusters {
    fn new(n: usize) -> Self {
        Clusters {
            parent: (0..n).collect(),
        }
    }

    /// The lowest member of the set holding `i`.
    fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while let Some(&up) = self.parent.get(root) {
            if up == root {
                break;
            }
            root = up;
        }
        let mut at = i;
        while let Some(&up) = self.parent.get(at) {
            if up == root {
                break;
            }
            if let Some(slot) = self.parent.get_mut(at) {
                *slot = root;
            }
            at = up;
        }
        root
    }

    /// Joins the sets of `a` and `b`; the lower root stays the root.
    fn join(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        let (low, high) = (ra.min(rb), ra.max(rb));
        if let Some(slot) = self.parent.get_mut(high) {
            *slot = low;
        }
    }
}

/// The worst distance, as an upper bound in whole steps, of a grid point from a set of carriers, at
/// the finest refinement; and the carrier that is that far, with its distance enclosure.
fn worst(
    carriers: &Carriers,
    p: P3,
    on: &SortedVec<CarrierIx>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Bound, Option<CarrierIx>, geom_num::interval::Interval), ContactError> {
    let mut far = (Bound::ZERO, None, geom_num::interval::Interval::point(0.0)?);
    for &c in on.as_slice() {
        let (bound, d) = carriers.upper_bound(p, c, REFINEMENTS, meter)?;
        if bound >= far.0 {
            far = (bound, Some(c), d);
        }
    }
    Ok(far)
}

/// One representative for the points `members`, proven within `remaining` of every carrier of every
/// member.
///
/// # Errors
///
/// [`ContactError::CoincidenceChain`] when no candidate is proven within `remaining` of every
/// carrier; whatever a distance stops with.
pub fn merge_cluster(
    carriers: &Carriers,
    members: &[PointGeom],
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<PointGeom, ContactError> {
    let [only] = members else {
        return merge_many(carriers, members, remaining, meter);
    };
    Ok(only.clone())
}

fn merge_many(
    carriers: &Carriers,
    members: &[PointGeom],
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<PointGeom, ContactError> {
    let mut on = SortedVec::<CarrierIx>::default();
    for m in members {
        on = on.union(&m.carriers());
    }
    // The most exact member: the lowest grid point among the exact ones. Otherwise the committed
    // grid points of every member, to choose among by distance.
    let exact: Option<P3> = members
        .iter()
        .filter_map(|m| match m {
            PointGeom::Grid(p) => Some(*p),
            _ => None,
        })
        .min();
    let candidates: Vec<P3> = if let Some(p) = exact {
        alloc::vec![p]
    } else {
        let mut all = Vec::new();
        for m in members {
            all.push(carriers.commit(m)?.0);
        }
        all.sort_unstable();
        all.dedup();
        all
    };
    let mut best: Option<(Bound, P3, Option<CarrierIx>, geom_num::interval::Interval)> = None;
    for p in candidates {
        let (upper, carrier, distance) = worst(carriers, p, &on, meter)?;
        if best.as_ref().is_none_or(|(b, ..)| upper < *b) {
            best = Some((upper, p, carrier, distance));
        }
    }
    let (upper, at, carrier, distance) =
        best.ok_or(ContactError::Num(geom_num::NumError::Degenerate))?;
    let within = upper;
    if within > remaining {
        return Err(ContactError::CoincidenceChain(CoincidenceChain {
            carrier: carrier.ok_or(ContactError::Num(geom_num::NumError::Degenerate))?,
            distance,
            remaining,
        }));
    }
    if on.is_empty() {
        return Ok(PointGeom::Grid(at));
    }
    Ok(PointGeom::Certified { at, on, within })
}

/// Merges the points that touch: every pair in `pairs` is decided by the contact rule on the
/// separation of its points; those that touch are joined into clusters, and each cluster becomes
/// one representative. Returns, for each input point, its cluster's representative.
///
/// # Errors
///
/// [`ContactError::NearContact`] for a pair that is neither decided nor touching;
/// [`ContactError::CoincidenceChain`] for a cluster without a representative within the bound.
pub fn merge_stage(
    carriers: &Carriers,
    points: &[PointGeom],
    pairs: &[(usize, usize)],
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<PointGeom>, ContactError> {
    let (reps, of) = merge_clusters(carriers, points, pairs, remaining, meter)?;
    let mut out = Vec::with_capacity(points.len());
    for &cluster in &of {
        out.push(
            reps.get(cluster)
                .cloned()
                .ok_or(ContactError::Num(geom_num::NumError::Degenerate))?,
        );
    }
    Ok(out)
}

/// As [`merge_stage`], giving the clusters themselves: one representative per cluster, in the
/// order of each cluster's lowest member, and for each input point the index of its cluster.
///
/// # Errors
///
/// As [`merge_stage`].
pub fn merge_clusters(
    carriers: &Carriers,
    points: &[PointGeom],
    pairs: &[(usize, usize)],
    remaining: Bound,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<PointGeom>, Vec<usize>), ContactError> {
    let mut clusters = Clusters::new(points.len());
    for &(i, j) in pairs {
        let (Some(a), Some(b)) = (points.get(i), points.get(j)) else {
            return Err(ContactError::Num(geom_num::NumError::Degenerate));
        };
        // The separation of two points does not refine; the measure is the same at every level.
        let apart = separation(carriers, a, b)?;
        match decide(|_| Ok(apart))? {
            Contact::Touch(_) => clusters.join(i, j),
            Contact::Apart(_) => {}
        }
    }
    let mut roots = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        roots.push(clusters.find(i));
    }
    // Clusters are numbered by their lowest member, which is their root.
    let mut numbered: Vec<usize> = Vec::new();
    let mut reps: Vec<PointGeom> = Vec::new();
    let mut of = Vec::with_capacity(points.len());
    for &root in &roots {
        if let Some(k) = numbered.iter().position(|r| *r == root) {
            of.push(k);
            continue;
        }
        let members: Vec<PointGeom> = roots
            .iter()
            .zip(points)
            .filter(|(r, _)| **r == root)
            .map(|(_, p)| p.clone())
            .collect();
        reps.push(merge_cluster(carriers, &members, remaining, meter)?);
        of.push(numbered.len());
        numbered.push(root);
    }
    Ok((reps, of))
}

/// The remaining bound a merge is held to when the caller asked for none: `ε_fit`.
#[must_use]
pub const fn default_remaining() -> Bound {
    Bound(EPS_FIT)
}
