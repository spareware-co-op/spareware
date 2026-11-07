//! Tests that try to break each claim of piece G262's plan: the chains partition the edges under any
//! order, the clusters partition the chains, a radius reported as feasible really is within the
//! planar bound, and the exact convexity test agrees with floating point wherever floating point
//! can tell.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_blend::chain::{EdgeId, EdgeRow, Join, Tangency, VertexId, chains};
use geom_blend::cluster::{FaceId, Footprint, Patch, clusters};
use geom_blend::convexity::{PlaneEdge, plane_convexity};
use geom_blend::radius::{Verdict, half_angle_tangent, judge, largest, reach_limit};
use geom_num::interval::Interval;

/// A fixed pseudo-random sequence, the same on every machine.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn signed(&mut self, n: i64) -> i128 {
        i128::from(self.below(2 * n as u64 + 1) as i64 - n)
    }
}

#[test]
fn chains_partition_the_edges_whatever_order_they_and_their_joins_are_given_in() {
    let mut rng = Lcg(0x6262);
    // A path of 40 edges 0 - 1 - ... - 40 with every third join sharp, plus a ring of 6.
    let mut edges: Vec<EdgeRow> = (0..40)
        .map(|k| EdgeRow {
            id: EdgeId(k),
            start: VertexId(k),
            end: VertexId(k + 1),
        })
        .collect();
    edges.extend((0..6).map(|k| EdgeRow {
        id: EdgeId(100 + k),
        start: VertexId(200 + k),
        end: VertexId(200 + (k + 1) % 6),
    }));
    let mut joins: Vec<Join> = (1..40)
        .map(|k| Join {
            at: VertexId(k),
            first: EdgeId(k - 1),
            second: EdgeId(k),
            tangency: if k % 3 == 0 {
                Tangency::Sharp
            } else {
                Tangency::Tangent
            },
        })
        .collect();
    joins.extend((0..6).map(|k| Join {
        at: VertexId(200 + (k + 1) % 6),
        first: EdgeId(100 + k),
        second: EdgeId(100 + (k + 1) % 6),
        tangency: Tangency::Tangent,
    }));
    let base = chains(&edges, &joins).unwrap();
    // 40 edges cut at every third vertex: 14 chains; one ring.
    assert_eq!(base.len(), 14 + 1);
    assert_eq!(base.iter().filter(|c| c.closed).count(), 1);
    let mut seen: Vec<u32> = base
        .iter()
        .flat_map(|c| c.links.iter().map(|l| l.edge.0))
        .collect();
    seen.sort_unstable();
    let mut want: Vec<u32> = edges.iter().map(|e| e.id.0).collect();
    want.sort_unstable();
    assert_eq!(seen, want);
    for _ in 0..25 {
        let (mut e, mut j) = (edges.clone(), joins.clone());
        for i in (1..e.len()).rev() {
            e.swap(i, rng.below(i as u64 + 1) as usize);
        }
        for i in (1..j.len()).rev() {
            j.swap(i, rng.below(i as u64 + 1) as usize);
        }
        assert_eq!(chains(&e, &j).unwrap(), base);
    }
}

#[test]
fn clusters_partition_the_chains_and_equal_the_connected_parts_of_the_overlaps() {
    let mut rng = Lcg(0x2621);
    for _ in 0..30 {
        let n = 12;
        let feet: Vec<Footprint> = (0..n)
            .map(|_| Footprint {
                vertices: vec![VertexId(rng.below(30) as u32)],
                patches: vec![Patch {
                    face: FaceId(rng.below(4) as u32),
                    reach: {
                        let u = rng.below(20) as f64;
                        let v = rng.below(20) as f64;
                        [
                            Interval::new(u, u + 2.0).unwrap(),
                            Interval::new(v, v + 2.0).unwrap(),
                        ]
                    },
                }],
            })
            .collect();
        let found = clusters(&feet);
        // A partition.
        let mut all: Vec<usize> = found.iter().flat_map(|c| c.chains.clone()).collect();
        all.sort_unstable();
        assert_eq!(all, (0..n).collect::<Vec<_>>());
        // Any two chains that share a vertex or overlap share a cluster; brute force.
        let cluster_of = |i: usize| found.iter().position(|c| c.chains.contains(&i)).unwrap();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (&feet[i], &feet[j]);
                let share = a.vertices.iter().any(|v| b.vertices.contains(v));
                let (pa, pb) = (a.patches[0], b.patches[0]);
                let meets = pa.face == pb.face
                    && pa.reach[0].lo() <= pb.reach[0].hi()
                    && pb.reach[0].lo() <= pa.reach[0].hi()
                    && pa.reach[1].lo() <= pb.reach[1].hi()
                    && pb.reach[1].lo() <= pa.reach[1].hi();
                if share || meets {
                    assert_eq!(cluster_of(i), cluster_of(j), "{i} {j}");
                }
            }
        }
        // And within a cluster the chains are in input order, the clusters by first chain.
        for c in &found {
            assert!(c.chains.windows(2).all(|w| w[0] < w[1]));
        }
        assert!(found.windows(2).all(|w| w[0].chains[0] < w[1].chains[0]));
    }
}

#[test]
fn a_radius_judged_feasible_is_within_the_planar_bound_found_independently() {
    let mut rng = Lcg(0x1626);
    let mut feasible = 0;
    for _ in 0..400 {
        let n1 = [rng.signed(1000), rng.signed(1000), rng.signed(1000)];
        let n2 = [rng.signed(1000), rng.signed(1000), rng.signed(1000)];
        let Ok(tan) = half_angle_tangent(n1, n2) else {
            continue;
        };
        if tan.lo() <= 1e-9 {
            continue;
        }
        let width = (rng.below(1000) + 1) as f64;
        let limit = reach_limit(Interval::point(width).unwrap(), tan, false).unwrap();
        // The bound by floating point: tan(phi / 2) from the angle between the normals.
        let (a, b) = (n1.map(|c| c as f64), n2.map(|c| c as f64));
        let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let na = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        let nb = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
        let phi = (dot / (na * nb)).clamp(-1.0, 1.0).acos();
        let want = width / (phi / 2.0).tan();
        assert!(
            limit.radius.lo() <= want * (1.0 + 1e-9) && want * (1.0 - 1e-9) <= limit.radius.hi(),
            "{n1:?} {n2:?} {width}: {:?} against {want}",
            limit.radius
        );
        // A radius a tenth below the bound is feasible, one a tenth above is refused.
        assert_eq!(judge(&[limit], want * 0.9), Verdict::Feasible);
        assert!(matches!(
            judge(&[limit], want * 1.1),
            Verdict::RadiusTooLarge(_)
        ));
        let fit = largest(&[limit]).unwrap();
        assert!(fit.radius <= want * (1.0 + 1e-9));
        feasible += 1;
    }
    assert!(feasible > 300, "{feasible}");
}

#[test]
fn the_exact_convexity_test_agrees_with_floating_point_wherever_that_can_tell() {
    let mut rng = Lcg(0x6261);
    let mut agreed = 0;
    for _ in 0..2000 {
        let n1 = [rng.signed(5000), rng.signed(5000), rng.signed(5000)];
        let n2 = [rng.signed(5000), rng.signed(5000), rng.signed(5000)];
        let d = [rng.signed(5000), rng.signed(5000), rng.signed(5000)];
        let got = plane_convexity(n1, n2, d).unwrap();
        let f = |v: [i128; 3]| v.map(|c| c as f64);
        let (a, b, e) = (f(n1), f(n2), f(d));
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let total = cross[0] * e[0] + cross[1] * e[1] + cross[2] * e[2];
        // Only where the float answer is far from zero is it trusted.
        if total.abs() > 1.0 {
            let want = if total > 0.0 {
                PlaneEdge::Convex
            } else {
                PlaneEdge::Concave
            };
            assert_eq!(got, want, "{n1:?} {n2:?} {d:?}");
            agreed += 1;
        }
        // Swapping the faces and reversing the edge gives the same answer, and swapping the faces
        // alone gives the opposite.
        let neg = d.map(|c| -c);
        let swapped = plane_convexity(n2, n1, neg).unwrap();
        assert_eq!(got, swapped, "{n1:?} {n2:?} {d:?}");
        if matches!(got, PlaneEdge::Convex | PlaneEdge::Concave) {
            let opposite = plane_convexity(n2, n1, d).unwrap();
            assert_ne!(got, opposite);
        }
    }
    assert!(agreed > 1500, "{agreed}");
}
