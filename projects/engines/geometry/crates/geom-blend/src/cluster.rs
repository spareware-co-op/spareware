//! Interaction clusters: chains that must be blended together (13.10, "Feasibility first", step 4,
//! G262).
//!
//! What it does: each chain's **footprint** is the band between its contact curves, extended by its
//! setbacks, written as the [`Patch`]es of faces it covers and the vertices it touches
//! ([`Footprint`]). [`clusters`] joins the chains whose footprints overlap, or that share a vertex,
//! into [`Cluster`]s, even when they share no vertex: two fillets eating one narrow face, or the
//! opposite edges of a thin rib. A cluster succeeds or fails as one.
//!
//! Guarantees:
//!
//! - **Overlap is closed.** Two patches of one face overlap when their regions share a point,
//!   touching included, since a blend that only touches another's band still meets it at a setback.
//!   A patch of another face never overlaps.
//! - **Transitive.** Chains joined through a third are one cluster: the clusters are the connected
//!   parts of the overlap relation.
//! - **A partition, in a fixed order.** Every chain is in exactly one cluster; the chains of a
//!   cluster are in input order and the clusters are in the order of their first chains.
//! - **No false separation.** A region is an enclosure of the band, so a pair that could overlap is
//!   joined: separation is only declared where the enclosures are proven apart.
//!
//! Method: chapter 13.10 of the kernel architecture, "Feasibility first", step 4
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`), with the connected parts
//! of the relation found by the disjoint-set structure of Tarjan and van Leeuwen
//! (<https://doi.org/10.1145/62.2160>).
//!
//! Cost: quadratic in the patches of one face at worst, and linear elsewhere. No recursion.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use geom_num::interval::Interval;

use crate::chain::VertexId;

/// The identity of a face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FaceId(pub u32);

/// A region of a face's parameter space that a chain's band and setbacks cover.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patch {
    /// The face.
    pub face: FaceId,
    /// The region, as an enclosure of its first and second parameter.
    pub reach: [Interval; 2],
}

/// What one chain covers.
#[derive(Clone, Debug, PartialEq)]
pub struct Footprint {
    /// The vertices the chain touches, ends included.
    pub vertices: Vec<VertexId>,
    /// The patches of faces its band covers.
    pub patches: Vec<Patch>,
}

/// Chains that are blended together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cluster {
    /// The indices of the chains, in input order.
    pub chains: Vec<usize>,
}

/// Whether two enclosures share a point.
fn meets(a: Interval, b: Interval) -> bool {
    a.lo() <= b.hi() && b.lo() <= a.hi()
}

/// Whether two patches overlap.
fn overlap(a: &Patch, b: &Patch) -> bool {
    a.face == b.face && meets(a.reach[0], b.reach[0]) && meets(a.reach[1], b.reach[1])
}

/// The root of `i` in a disjoint-set forest, with path halving.
fn find(parent: &mut [usize], mut i: usize) -> usize {
    while let Some(&up) = parent.get(i) {
        if up == i {
            break;
        }
        let grand = parent.get(up).copied().unwrap_or(up);
        if let Some(slot) = parent.get_mut(i) {
            *slot = grand;
        }
        i = grand;
    }
    i
}

/// Joins `a` and `b`; the lower root stays the root.
fn join(parent: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    let (low, high) = if ra <= rb { (ra, rb) } else { (rb, ra) };
    if let Some(slot) = parent.get_mut(high) {
        *slot = low;
    }
}

/// Groups the chains into interaction clusters.
#[must_use]
pub fn clusters(feet: &[Footprint]) -> Vec<Cluster> {
    let mut parent: Vec<usize> = (0..feet.len()).collect();
    // Chains that share a vertex.
    let mut by_vertex: BTreeMap<VertexId, usize> = BTreeMap::new();
    for (i, foot) in feet.iter().enumerate() {
        for v in &foot.vertices {
            match by_vertex.get(v) {
                Some(&first) => join(&mut parent, first, i),
                None => {
                    by_vertex.insert(*v, i);
                }
            }
        }
    }
    // Chains whose patches overlap, face by face.
    let mut by_face: BTreeMap<FaceId, Vec<(usize, Patch)>> = BTreeMap::new();
    for (i, foot) in feet.iter().enumerate() {
        for patch in &foot.patches {
            by_face.entry(patch.face).or_default().push((i, *patch));
        }
    }
    for patches in by_face.values() {
        for (k, (i, a)) in patches.iter().enumerate() {
            for (j, b) in patches.iter().skip(k.saturating_add(1)) {
                if i != j && overlap(a, b) {
                    join(&mut parent, *i, *j);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..feet.len() {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }
    // The roots are the lowest members, so the map is already in the order of the first chains.
    groups
        .into_values()
        .map(|chains| Cluster { chains })
        .collect()
}
