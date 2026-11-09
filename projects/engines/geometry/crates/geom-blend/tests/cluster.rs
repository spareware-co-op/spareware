//! Interaction clusters (G262, chapter 13.10, "Feasibility first", step 4): chains that share a
//! vertex, or whose footprints overlap on a face, are blended together.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_blend::chain::VertexId;
use geom_blend::cluster::{Cluster, FaceId, Footprint, Patch, clusters};
use geom_num::interval::Interval;

fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

fn patch(face: u32, u: (f64, f64), v: (f64, f64)) -> Patch {
    Patch {
        face: FaceId(face),
        reach: [iv(u.0, u.1), iv(v.0, v.1)],
    }
}

fn foot(vertices: &[u32], patches: &[Patch]) -> Footprint {
    Footprint {
        vertices: vertices.iter().map(|v| VertexId(*v)).collect(),
        patches: patches.to_vec(),
    }
}

fn groups(found: &[Cluster]) -> Vec<Vec<usize>> {
    found.iter().map(|c| c.chains.clone()).collect()
}

#[test]
fn chains_that_share_a_vertex_are_one_cluster_and_the_rest_stand_alone() {
    let feet = [
        foot(&[1, 2], &[]),
        foot(&[2, 3], &[]),
        foot(&[10, 11], &[]),
        foot(&[3, 4], &[]),
    ];
    assert_eq!(groups(&clusters(&feet)), [vec![0, 1, 3], vec![2]]);
}

#[test]
fn two_fillets_eating_one_narrow_face_are_one_cluster_with_no_shared_vertex() {
    // The same face, bands that overlap in the middle of it.
    let a = foot(&[1, 2], &[patch(7, (0.0, 5.0), (0.0, 10.0))]);
    let b = foot(&[3, 4], &[patch(7, (4.0, 9.0), (0.0, 10.0))]);
    let far = foot(&[5, 6], &[patch(8, (0.0, 9.0), (0.0, 10.0))]);
    assert_eq!(groups(&clusters(&[a, b, far])), [vec![0, 1], vec![2]]);
}

#[test]
fn patches_that_only_touch_overlap_and_patches_proven_apart_do_not() {
    let a = foot(&[1], &[patch(7, (0.0, 5.0), (0.0, 1.0))]);
    let touching = foot(&[2], &[patch(7, (5.0, 6.0), (0.0, 1.0))]);
    let apart = foot(&[3], &[patch(7, (6.000_001, 7.0), (0.0, 1.0))]);
    let apart_in_v = foot(&[4], &[patch(7, (0.0, 5.0), (1.5, 2.0))]);
    let found = clusters(&[a, touching, apart, apart_in_v]);
    assert_eq!(groups(&found), [vec![0, 1], vec![2], vec![3]]);
}

#[test]
fn the_opposite_edges_of_a_thin_rib_are_one_cluster_through_a_third_chain() {
    // Chains 0 and 2 never meet each other, but both meet chain 1: one cluster.
    let feet = [
        foot(&[1], &[patch(1, (0.0, 2.0), (0.0, 9.0))]),
        foot(
            &[2],
            &[
                patch(1, (1.0, 4.0), (0.0, 9.0)),
                patch(2, (0.0, 1.0), (0.0, 1.0)),
            ],
        ),
        foot(&[3], &[patch(2, (0.5, 3.0), (0.5, 3.0))]),
    ];
    assert_eq!(groups(&clusters(&feet)), [vec![0, 1, 2]]);
}

#[test]
fn clusters_partition_the_chains_in_a_fixed_order_whatever_joins_them() {
    let feet = [
        foot(&[9], &[]),
        foot(&[1, 2], &[]),
        foot(&[5], &[]),
        foot(&[2, 5], &[]),
        foot(&[1], &[]),
    ];
    let found = clusters(&feet);
    assert_eq!(groups(&found), [vec![0], vec![1, 2, 3, 4]]);
    let mut all: Vec<usize> = found.iter().flat_map(|c| c.chains.clone()).collect();
    all.sort_unstable();
    assert_eq!(all, [0, 1, 2, 3, 4]);
    assert_eq!(clusters(&[]).len(), 0);
}
