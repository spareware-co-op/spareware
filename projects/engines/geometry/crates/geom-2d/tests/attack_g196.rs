//! Attacks on piece G196a: the arrangement of line segments (`geom_2d::arrange`; chapter 14.4 of
//! the kernel architecture). Every test tries to break a claim of the piece: an exact sweep that
//! finds every crossing, merges overlaps, orders the edges at each vertex, places holes and the
//! unbounded face, names its output by rank so that the input order does not matter, and stops
//! when its meter does.
//!
//! - **Vertices are exact**: the vertex set of random small drawings equals an independent
//!   oracle's (end points and the reduced rational crossing points), and a crossing at (3/2, 1/2)
//!   is exactly that.
//! - **The graph is planar and complete**: Euler's formula holds for every random drawing, every
//!   half-edge is on exactly one cycle, every clockwise cycle is a hole of exactly one face.
//! - **Shapes**: crossings, a square, a square with diagonals, a T-junction, collinear overlaps,
//!   vertical segments, nested squares, and a dangling edge inside a face.
//! - **Permutation-invariant**: shuffling the input changes only the source labels.
//! - **Refusals**: a stopped meter stops the sweep; a zero-length segment is ignored.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names
)]

use std::collections::BTreeSet;

use geom_2d::arrange::{ArrangeError, Arrangement, arrange};
use geom_num::carrier::Rational2;
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};
use geom_test::{Seed, SplitMix64};

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn seg(a: (i64, i64), b: (i64, i64)) -> (P2, P2) {
    (p(a.0, a.1), p(b.0, b.1))
}

fn run(segs: &[(P2, P2)]) -> Arrangement {
    arrange(segs, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap()
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// A rational point as a reduced (x numerator, x denominator, y numerator, y denominator).
type Red = (i128, i128, i128, i128);

fn reduced(r: &Rational2) -> Red {
    let ([x, y], w) = r.coordinates();
    let gx = gcd(x, w).max(1);
    let gy = gcd(y, w).max(1);
    (x / gx, w / gx, y / gy, w / gy)
}

fn grid(x: i64, y: i64) -> Red {
    (i128::from(x), 1, i128::from(y), 1)
}

#[test]
fn two_crossing_segments_make_a_vertex_at_the_crossing() {
    let a = run(&[seg((0, 0), (4, 4)), seg((0, 4), (4, 0))]);
    assert_eq!(a.vertices().len(), 5);
    assert_eq!(a.edges().len(), 4);
    assert_eq!(a.faces().len(), 1, "only the unbounded face");
    let centre = a.vertices().iter().map(reduced).any(|r| r == grid(2, 2));
    assert!(centre);
    // Each segment is cut into two edges at the crossing.
    for e in a.edges() {
        assert_eq!(e.sources.len(), 1);
    }
}

#[test]
fn a_crossing_off_the_grid_is_exact() {
    let a = run(&[seg((0, 0), (3, 1)), seg((0, 1), (3, 0))]);
    let set: BTreeSet<Red> = a.vertices().iter().map(reduced).collect();
    assert!(set.contains(&(3, 2, 1, 2)), "{set:?}");
    assert_eq!(a.vertices().len(), 5);
}

#[test]
fn a_square_has_one_bounded_face() {
    let a = run(&[
        seg((0, 0), (10, 0)),
        seg((10, 0), (10, 10)),
        seg((10, 10), (0, 10)),
        seg((0, 10), (0, 0)),
    ]);
    assert_eq!((a.vertices().len(), a.edges().len()), (4, 4));
    assert_eq!(a.faces().len(), 2);
    assert!(a.faces()[0].outer.is_none());
    let outer = a.faces()[1].outer.unwrap();
    assert_eq!(a.cycles()[outer as usize].len(), 4);
    assert_eq!(a.cycles().len(), 2, "the inside and the outside walk");
}

#[test]
fn a_square_with_diagonals_has_four_triangles() {
    let a = run(&[
        seg((0, 0), (10, 0)),
        seg((10, 0), (10, 10)),
        seg((10, 10), (0, 10)),
        seg((0, 10), (0, 0)),
        seg((0, 0), (10, 10)),
        seg((0, 10), (10, 0)),
    ]);
    assert_eq!((a.vertices().len(), a.edges().len()), (5, 8));
    assert_eq!(a.faces().len(), 5);
    for f in &a.faces()[1..] {
        assert_eq!(a.cycles()[f.outer.unwrap() as usize].len(), 3);
    }
}

#[test]
fn a_t_junction_splits_the_segment_it_ends_on() {
    let a = run(&[seg((0, 0), (10, 0)), seg((5, 0), (5, 7))]);
    assert_eq!(a.vertices().len(), 4);
    assert_eq!(a.edges().len(), 3);
    let at5 = a
        .vertices()
        .iter()
        .position(|v| reduced(v) == grid(5, 0))
        .unwrap() as u32;
    assert_eq!(a.around(at5).len(), 3, "the stem and the two halves");
}

#[test]
fn collinear_overlaps_share_the_common_stretch() {
    let a = run(&[seg((0, 0), (10, 0)), seg((5, 0), (15, 0))]);
    let xs: Vec<Red> = a.vertices().iter().map(reduced).collect();
    assert_eq!(xs, [grid(0, 0), grid(5, 0), grid(10, 0), grid(15, 0)]);
    assert_eq!(a.edges().len(), 3);
    let sources: Vec<Vec<u32>> = a.edges().iter().map(|e| e.sources.clone()).collect();
    assert_eq!(sources, [vec![0], vec![0, 1], vec![1]]);
    // Identical segments in opposite directions are one edge with both sources.
    let b = run(&[seg((0, 0), (7, 3)), seg((7, 3), (0, 0))]);
    assert_eq!(b.edges().len(), 1);
    assert_eq!(b.edges()[0].sources, [0, 1]);
}

#[test]
fn vertical_segments_cross_and_touch() {
    let a = run(&[
        seg((5, 0), (5, 10)),
        seg((0, 4), (10, 4)),
        seg((5, 10), (9, 12)),
    ]);
    assert_eq!(a.vertices().len(), 6);
    assert_eq!(a.edges().len(), 5);
    let cross = a
        .vertices()
        .iter()
        .position(|v| reduced(v) == grid(5, 4))
        .unwrap() as u32;
    assert_eq!(a.around(cross).len(), 4);
}

#[test]
fn the_edges_around_a_vertex_run_counter_clockwise() {
    let a = run(&[
        seg((0, 0), (4, 0)),
        seg((0, 0), (4, 4)),
        seg((0, 0), (0, 4)),
        seg((0, 0), (-4, 4)),
        seg((0, 0), (-4, 0)),
        seg((0, 0), (0, -4)),
    ]);
    let o = a
        .vertices()
        .iter()
        .position(|v| reduced(v) == grid(0, 0))
        .unwrap() as u32;
    // From the negative x axis counter-clockwise: west is first, then south, east, northeast,
    // north, northwest... the list starts after the half-plane split, so check the cyclic order.
    let ends: Vec<Red> = a
        .around(o)
        .iter()
        .map(|&h| {
            let e = &a.edges()[(h / 2) as usize];
            let far = if h % 2 == 0 { e.to } else { e.from };
            reduced(&a.vertices()[far as usize])
        })
        .collect();
    let want = [
        grid(-4, 0),
        grid(0, -4),
        grid(4, 0),
        grid(4, 4),
        grid(0, 4),
        grid(-4, 4),
    ];
    let start = ends.iter().position(|e| *e == want[0]).unwrap();
    let rotated: Vec<Red> = (0..6).map(|i| ends[(start + i) % 6]).collect();
    assert_eq!(rotated, want);
}

#[test]
fn nested_squares_place_the_inner_component_as_a_hole() {
    let square = |lo: i64, hi: i64| {
        vec![
            seg((lo, lo), (hi, lo)),
            seg((hi, lo), (hi, hi)),
            seg((hi, hi), (lo, hi)),
            seg((lo, hi), (lo, lo)),
        ]
    };
    let mut segs = square(0, 20);
    segs.extend(square(5, 15));
    let a = run(&segs);
    assert_eq!(a.faces().len(), 3);
    // The ring face (the outer square's inside) holds the inner square as a hole.
    let ring = &a.faces()[1];
    assert_eq!(ring.holes.len(), 1);
    assert!(a.faces()[0].holes.len() == 1, "the outer square is a hole of the unbounded face");
    assert!(a.faces()[2].holes.is_empty());
}

#[test]
fn a_dangling_edge_inside_a_face_makes_no_face() {
    let a = run(&[
        seg((0, 0), (10, 0)),
        seg((10, 0), (10, 10)),
        seg((10, 10), (0, 10)),
        seg((0, 10), (0, 0)),
        seg((0, 0), (5, 5)),
    ]);
    assert_eq!(a.faces().len(), 2);
    let spike_cycle = a.cycles().iter().find(|c| c.len() == 6).unwrap();
    assert_eq!(spike_cycle.len(), 6, "the inside walk goes down the spike and back");
}

#[test]
fn a_zero_length_segment_is_ignored_and_a_stopped_meter_stops() {
    let a = run(&[seg((3, 3), (3, 3)), seg((0, 0), (1, 1))]);
    assert_eq!((a.vertices().len(), a.edges().len()), (2, 1));
    assert_eq!(a.edges()[0].sources, [1]);
    let mut tight = WorkMeter::new(Budget::new(1, 1));
    let r = arrange(&[seg((0, 0), (4, 4)), seg((0, 4), (4, 0))], &mut tight);
    assert!(matches!(r, Err(ArrangeError::Stopped(_))));
}

#[test]
fn the_arrangement_does_not_depend_on_the_input_order() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("arrange permute", Seed::DEFAULT));
    for round in 0..40 {
        let segs = random_segments(&mut rng, 9, 12);
        let a = run(&segs);
        let mut order: Vec<usize> = (0..segs.len()).collect();
        // A deterministic shuffle.
        for i in (1..order.len()).rev() {
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            order.swap(i, j);
        }
        let shuffled: Vec<(P2, P2)> = order.iter().map(|&i| segs[i]).collect();
        let b = run(&shuffled);
        let va: Vec<Red> = a.vertices().iter().map(reduced).collect();
        let vb: Vec<Red> = b.vertices().iter().map(reduced).collect();
        assert_eq!(va, vb, "round {round}");
        let ea: Vec<(u32, u32)> = a.edges().iter().map(|e| (e.from, e.to)).collect();
        let eb: Vec<(u32, u32)> = b.edges().iter().map(|e| (e.from, e.to)).collect();
        assert_eq!(ea, eb, "round {round}");
        // The sources agree once the labels are mapped back.
        for (x, y) in a.edges().iter().zip(b.edges()) {
            let mapped: BTreeSet<u32> = y.sources.iter().map(|&s| order[s as usize] as u32).collect();
            let want: BTreeSet<u32> = x.sources.iter().copied().collect();
            assert_eq!(mapped, want, "round {round}");
        }
        assert_eq!(a.faces().len(), b.faces().len(), "round {round}");
    }
}

fn random_segments(rng: &mut SplitMix64, count: usize, span: u64) -> Vec<(P2, P2)> {
    let mut out = Vec::new();
    while out.len() < count {
        let mut c = || (rng.next_u64() % span) as i64;
        let (a, b) = ((c(), c()), (c(), c()));
        if a != b {
            out.push(seg(a, b));
        }
    }
    out
}

/// The oracle's vertices: every end point, and every proper crossing, as reduced fractions.
fn oracle_vertices(segs: &[(P2, P2)]) -> BTreeSet<Red> {
    let mut out = BTreeSet::new();
    let g = |q: P2| (i128::from(q.x.get()), i128::from(q.y.get()));
    for (a, b) in segs {
        for q in [a, b] {
            let (x, y) = g(*q);
            out.insert((x, 1, y, 1));
        }
    }
    for (i, (a, b)) in segs.iter().enumerate() {
        for (c, d) in &segs[i + 1..] {
            let ((ax, ay), (bx, by)) = (g(*a), g(*b));
            let ((cx, cy), (dx, dy)) = (g(*c), g(*d));
            let (rx, ry) = (bx - ax, by - ay);
            let (sx, sy) = (dx - cx, dy - cy);
            let den = rx * sy - ry * sx;
            if den == 0 {
                continue;
            }
            // a + t r = c + u s, t = ((c - a) x s) / den, u = ((c - a) x r) / den
            let (qx, qy) = (cx - ax, cy - ay);
            let tn = qx * sy - qy * sx;
            let un = qx * ry - qy * rx;
            let (tn, un, den) = if den < 0 { (-tn, -un, -den) } else { (tn, un, den) };
            if tn < 0 || tn > den || un < 0 || un > den {
                continue;
            }
            let (xn, yn) = (ax * den + tn * rx, ay * den + tn * ry);
            let gx = gcd(xn, den).max(1);
            let gy = gcd(yn, den).max(1);
            out.insert((xn / gx, den / gx, yn / gy, den / gy));
        }
    }
    out
}

/// The connected components of the edge graph, by union-find over vertices that have an edge.
fn components(a: &Arrangement) -> usize {
    let n = a.vertices().len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for e in a.edges() {
        let (u, v) = (find(&mut parent, e.from as usize), find(&mut parent, e.to as usize));
        parent[u] = v;
    }
    let mut roots = BTreeSet::new();
    for v in 0..n {
        roots.insert(find(&mut parent, v));
    }
    roots.len()
}

#[test]
fn random_drawings_match_the_vertex_oracle_and_euler_s_formula() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("arrange random", Seed::DEFAULT));
    for round in 0..300 {
        let count = 3 + (rng.next_u64() % 9) as usize;
        let span = 4 + rng.next_u64() % 12;
        let segs = random_segments(&mut rng, count, span);
        let a = run(&segs);
        let got: BTreeSet<Red> = a.vertices().iter().map(reduced).collect();
        assert_eq!(got, oracle_vertices(&segs), "round {round}: {segs:?}");
        assert_eq!(got.len(), a.vertices().len(), "round {round}: vertices are distinct");
        // Vertices ascend lexicographically.
        let pts: Vec<(i128, i128)> = a
            .vertices()
            .iter()
            .map(|v| {
                let ([x, y], w) = v.coordinates();
                // Compare by cross-multiplied order below; here use floats only for monotone check.
                (x * 1_000_000 / w, y * 1_000_000 / w)
            })
            .collect();
        assert!(pts.windows(2).all(|w| w[0] <= w[1]), "round {round}");
        // Euler: V - E + F = 1 + C, F counting the unbounded face.
        let v = a.vertices().len() as i64;
        let e = a.edges().len() as i64;
        let f = a.faces().len() as i64;
        let c = components(&a) as i64;
        assert_eq!(v - e + f, 1 + c, "round {round}: {segs:?}");
        // Every half-edge is on exactly one cycle.
        let total: usize = a.cycles().iter().map(Vec::len).sum();
        assert_eq!(total, 2 * a.edges().len(), "round {round}");
        let mut seen = BTreeSet::new();
        for cy in a.cycles() {
            for h in cy {
                assert!(seen.insert(*h), "round {round}: half-edge twice");
            }
        }
        // Every cycle is the outer boundary of one face or a hole of exactly one.
        let mut count_of = vec![0_u32; a.cycles().len()];
        for f in a.faces() {
            if let Some(o) = f.outer {
                count_of[o as usize] += 1;
            }
            for h in &f.holes {
                count_of[*h as usize] += 1;
            }
        }
        assert!(count_of.iter().all(|&n| n == 1), "round {round}: {count_of:?}");
    }
}
