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
//! - **Refusals**: a stopped meter stops the sweep; a zero-length segment is ignored; each refusal
//!   says what it is.
//! - **Bodies**: every link of a body's fins, loops, faces and shell, against the arrangement it
//!   came from; a crossing whole in x only is off the grid.
//! - **Holes**: a component above another's top is a hole of the face around both, not of the
//!   face below that top.

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

use geom_2d::arrange::{ArrangeError, Arrangement, At, arrange};
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};

/// A small deterministic generator (`SplitMix64`, Steele, Lea and Flood 2014), for the drawings.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> SplitMix64 {
        SplitMix64(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

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

fn reduced(v: &At) -> Red {
    let ([x, y], w) = v.rational().expect("a rational vertex").coordinates();
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
    // From the negative x direction counter-clockwise, which the list starts at: west first, then
    // south, east, northeast, north and northwest. The start is part of the published order, so
    // it is checked, not only the cyclic order.
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
    assert_eq!(ends, want);
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
    assert_eq!(
        a.faces()[0].holes.len(),
        1,
        "the outer square is a hole of the unbounded face"
    );
    assert_eq!(a.faces()[2].holes.len(), 0);
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
    assert_eq!(
        spike_cycle.len(),
        6,
        "the inside walk goes down the spike and back"
    );
}

#[test]
fn a_line_edge_lies_on_the_smallest_carrier_of_its_line_whatever_the_segments() {
    use geom_2d::arrange::Along;
    use geom_num::carrier::Carrier;
    // Two overlapping segments on y = x / 2, one twice the other's length, given either way round.
    for segs in [
        [seg((0, 0), (4, 2)), seg((6, 3), (2, 1))],
        [seg((2, 1), (6, 3)), seg((4, 2), (0, 0))],
    ] {
        let a = run(&segs);
        assert_eq!(a.edges().len(), 3);
        assert!(a.circles().is_empty());
        for e in 0..3_u32 {
            // -x + 2y = 0, running (2, 1): from the lower vertex to the higher.
            assert_eq!(
                a.along(e),
                Some(Along::Line(Carrier::new(-1, 2, 0).unwrap()))
            );
        }
        assert_eq!(a.along(3), None);
        // Vertices compare exactly, whichever way they are stored.
        let v = a.vertices();
        assert_eq!(v[0].cmp_x(&v[1]).unwrap(), geom_num::Sign::Negative);
        assert_eq!(v[3].cmp_y(&v[2]).unwrap(), geom_num::Sign::Positive);
        assert!(v[1].same(&v[1]).unwrap() && !v[1].same(&v[2]).unwrap());
        let root = At::Root(Box::new(v[2].crossing().unwrap()));
        assert!(root.same(&v[2]).unwrap() && v[2].same(&root).unwrap());
        assert!(root.rational().is_none());
        assert_eq!(root.cmp_x(&v[3]).unwrap(), geom_num::Sign::Negative);
    }
}

#[test]
fn each_refusal_says_what_it_is() {
    let stopped = WorkMeter::new(Budget::UNLIMITED).out_of_memory();
    let said = [
        ArrangeError::Range,
        ArrangeError::TooMany,
        ArrangeError::Stopped(stopped),
    ]
    .map(|e| e.to_string());
    assert_eq!(
        said,
        [
            "a step of the exact arithmetic did not fit",
            "more items than a 32-bit index names",
            "stopped: Stopped { reason: OutOfMemory }",
        ]
    );
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
    let mut rng = SplitMix64::new(0x5EED_0001);
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
            let mapped: BTreeSet<u32> = y
                .sources
                .iter()
                .map(|&s| order[s as usize] as u32)
                .collect();
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
            let (tn, un, den) = if den < 0 {
                (-tn, -un, -den)
            } else {
                (tn, un, den)
            };
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
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let n = a.vertices().len();
    let mut parent: Vec<usize> = (0..n).collect();
    for e in a.edges() {
        let (u, v) = (
            find(&mut parent, e.from as usize),
            find(&mut parent, e.to as usize),
        );
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
    let mut rng = SplitMix64::new(0x5EED_0002);
    for round in 0..300 {
        let count = 3 + (rng.next_u64() % 9) as usize;
        let span = 4 + rng.next_u64() % 12;
        let segs = random_segments(&mut rng, count, span);
        let a = run(&segs);
        let got: BTreeSet<Red> = a.vertices().iter().map(reduced).collect();
        assert_eq!(got, oracle_vertices(&segs), "round {round}: {segs:?}");
        assert_eq!(
            got.len(),
            a.vertices().len(),
            "round {round}: vertices are distinct"
        );
        // Vertices ascend lexicographically.
        let pts: Vec<(i128, i128)> = a
            .vertices()
            .iter()
            .map(|v| {
                let ([x, y], w) = v.rational().unwrap().coordinates();
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
        let total: usize = a.cycles().iter().fold(0, |n, c| n + c.len());
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
        assert!(
            count_of.iter().all(|&n| n == 1),
            "round {round}: {count_of:?}"
        );
    }
}

/// The rows of each arena of a body, in the order region, shell, face, loop, fin, edge, vertex.
fn rows(body: &geom_topo::Body) -> [usize; 7] {
    use geom_topo::Arena;
    let a = body.arenas();
    [
        Arena::Region,
        Arena::Shell,
        Arena::Face,
        Arena::Loop,
        Arena::Fin,
        Arena::Edge,
        Arena::Vertex,
    ]
    .map(|k| usize::try_from(a.rows(k)).unwrap())
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

#[test]
fn a_square_is_a_general_body_with_one_face_and_one_loop() {
    let a = run(&[
        seg((0, 0), (10, 0)),
        seg((10, 0), (10, 10)),
        seg((10, 10), (0, 10)),
        seg((0, 10), (0, 0)),
    ]);
    let body = a.to_body(&mut meter()).unwrap();
    assert_eq!(body.kind(), geom_topo::BodyKind::General);
    // 1 outside region, 1 shell, 1 face, 1 loop, 4 fins (the outside has none), 4 edges, 4 vertices.
    assert_eq!(rows(&body), [1, 1, 1, 1, 4, 4, 4]);
}

#[test]
fn a_square_with_diagonals_has_four_faces_and_a_fin_for_each_side_with_a_face() {
    let a = run(&[
        seg((0, 0), (10, 0)),
        seg((10, 0), (10, 10)),
        seg((10, 10), (0, 10)),
        seg((0, 10), (0, 0)),
        seg((0, 0), (10, 10)),
        seg((0, 10), (10, 0)),
    ]);
    // The diagonals cross at (5, 5), a grid point.
    let body = a.to_body(&mut meter()).unwrap();
    // 4 triangles of 3 fins each; 8 edges; 5 vertices.
    assert_eq!(rows(&body), [1, 1, 4, 4, 12, 8, 5]);
}

#[test]
fn nested_squares_give_the_ring_a_second_loop() {
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
    let body = run(&segs).to_body(&mut meter()).unwrap();
    // Two faces; the ring has its outer loop and the hole (3 loops), 4 fins on each of the three
    // cycles; 8 edges; 8 vertices.
    assert_eq!(rows(&body), [1, 1, 2, 3, 12, 8, 8]);
}

/// The grid point of the vertex of rank `v`.
fn grid_point(a: &Arrangement, v: u32) -> (i64, i64) {
    let ([x, y], w) = a.vertices()[v as usize].rational().unwrap().coordinates();
    assert!(x % w == 0 && y % w == 0, "vertex {v} is a grid point");
    ((x / w) as i64, (y / w) as i64)
}

/// The index of row `n`.
fn ix<K: geom_topo::Kind>(n: usize) -> geom_topo::Ix<K> {
    geom_topo::Ix::new(u32::try_from(n).unwrap()).unwrap()
}

/// Where a body's vertex lies.
fn point_of(body: &geom_topo::Body, v: geom_topo::VertexIx) -> (i64, i64) {
    let ar = body.arenas();
    let q = ar.points.get(ar.vertices.get(v).unwrap().point).unwrap();
    (q.x.get(), q.y.get())
}

/// Where a body's fin begins and ends, by its edge's ends and its sense.
fn fin_ends(body: &geom_topo::Body, f: geom_topo::FinIx) -> ((i64, i64), (i64, i64)) {
    let ar = body.arenas();
    let x = ar.fins.get(f).unwrap();
    let e = ar.edges.get(x.edge).unwrap();
    let (s, t) = (
        point_of(body, e.start.unwrap()),
        point_of(body, e.end.unwrap()),
    );
    if x.sense == geom_topo::Sense::Forward {
        (s, t)
    } else {
        (t, s)
    }
}

/// The edges and fins `to_body` sets, against the arrangement it came from: each edge between its
/// edge's vertices; each fin ending where its next begins, its next's previous, and on its next's
/// loop; a fin and its radial on one edge, opposite ways, each the other's radial, `paired` fins in
/// all.
fn edge_and_fin_laws(a: &Arrangement, body: &geom_topo::Body, paired: usize, what: &str) {
    let ar = body.arenas();
    for (k, e) in a.edges().iter().enumerate() {
        let t = ar.edges.get(ix(k)).unwrap();
        let ends = (
            point_of(body, t.start.unwrap()),
            point_of(body, t.end.unwrap()),
        );
        assert_eq!(
            ends,
            (grid_point(a, e.from), grid_point(a, e.to)),
            "{what}: edge {k}"
        );
    }
    let fin = |f: geom_topo::FinIx| ar.fins.get(f).unwrap();
    let mut radial = 0;
    for k in 0..ar.rows(geom_topo::Arena::Fin) as usize {
        let (f, x) = (ix(k), fin(ix(k)));
        let joined = fin_ends(body, f).1 == fin_ends(body, x.next).0;
        assert!(joined, "{what}: fin {k} ends where its next begins");
        assert_eq!(
            fin(x.next).prev,
            f,
            "{what}: fin {k} is its next's previous"
        );
        assert_eq!(
            fin(x.next).loop_,
            x.loop_,
            "{what}: fin {k}'s next is on its loop"
        );
        if x.radial != f {
            let r = fin(x.radial);
            assert_eq!((r.edge, r.radial), (x.edge, f), "{what}: fin {k}'s radial");
            assert_ne!(
                r.sense, x.sense,
                "{what}: fin {k} and its radial run opposite ways"
            );
            radial += 1;
        }
    }
    assert_eq!(radial, paired, "{what}: fins with a radial");
}

/// A loop's corners, from its first fin round to it again, each fin on the loop, and twice the area
/// they enclose.
fn loop_walk(body: &geom_topo::Body, l: geom_topo::LoopIx, what: &str) -> (Vec<(i64, i64)>, i64) {
    let ar = body.arenas();
    let first = ar.loops.get(l).unwrap().first_fin.unwrap();
    let (mut f, mut corners) = (first, Vec::new());
    loop {
        let x = ar.fins.get(f).unwrap();
        assert_eq!(x.loop_, l, "{what}: a loop's fins are on it");
        corners.push(fin_ends(body, f).0);
        f = x.next;
        if f == first {
            break;
        }
    }
    let twice = (0..corners.len()).fold(0_i64, |sum, i| {
        let ((x0, y0), (x1, y1)) = (corners[i], corners[(i + 1) % corners.len()]);
        sum + x0 * y1 - x1 * y0
    });
    (corners, twice)
}

/// The loops, faces and shell `to_body` sets: each face's loops its own and chained, the outer
/// counter-clockwise and the others clockwise, every fin on one; the faces chained in order, the
/// last to none; each face's box its outer loop's corners; and the shell's first face the first,
/// when there is one.
fn loop_and_face_laws(body: &geom_topo::Body, what: &str) {
    let ar = body.arenas();
    let faces = ar.rows(geom_topo::Arena::Face) as usize;
    let mut fins_on_loops = 0;
    for k in 0..faces {
        let face = ar.faces.get(ix(k)).unwrap();
        assert_eq!(
            face.next,
            (k + 1 < faces).then(|| ix(k + 1)),
            "{what}: face {k}'s next"
        );
        let mut l = face.first_loop;
        let mut outer = true;
        while let Some(at) = l {
            let one = ar.loops.get(at).unwrap();
            assert_eq!(one.face, ix(k), "{what}: face {k}'s loops are its own");
            let (corners, twice) = loop_walk(body, at, what);
            fins_on_loops += corners.len();
            if outer {
                assert!(
                    twice > 0,
                    "{what}: face {k}'s outer loop runs counter-clockwise"
                );
                let (xs, ys) = (corners.iter().map(|c| c.0), corners.iter().map(|c| c.1));
                let low = (xs.clone().min().unwrap(), ys.clone().min().unwrap());
                let high = (xs.max().unwrap(), ys.max().unwrap());
                let (lo, hi) = (face.bound.min(), face.bound.max());
                let bound = ((lo.x.get(), lo.y.get()), (hi.x.get(), hi.y.get()));
                assert_eq!(bound, (low, high), "{what}: face {k}'s box");
            } else {
                assert!(twice < 0, "{what}: face {k}'s holes run clockwise");
            }
            outer = false;
            l = one.next;
        }
    }
    let fins = ar.rows(geom_topo::Arena::Fin) as usize;
    assert_eq!(fins_on_loops, fins, "{what}: every fin is on a face's loop");
    let shell = ar.shells.get(ix(0)).unwrap();
    assert_eq!(
        shell.first_face,
        (faces > 0).then(|| ix(0)),
        "{what}: the shell's first face"
    );
}

#[test]
fn a_body_s_fins_loops_faces_and_shell_keep_every_link() {
    let square = |lo: i64, hi: i64| {
        vec![
            seg((lo, lo), (hi, lo)),
            seg((hi, lo), (hi, hi)),
            seg((hi, hi), (lo, hi)),
            seg((lo, hi), (lo, lo)),
        ]
    };
    let mut crossed = square(0, 10);
    crossed.extend([seg((0, 0), (10, 10)), seg((0, 10), (10, 0))]);
    let mut ring = square(0, 20);
    ring.extend(square(5, 15));
    let mut spiked = square(0, 10);
    spiked.push(seg((0, 0), (5, 5)));
    let mut apart = square(0, 10);
    apart.extend(square(30, 40));
    let open = vec![seg((0, 0), (5, 0)), seg((5, 0), (5, 5))];
    // The fins with a radial: those of edges with a bounded face on each side.
    for (segs, paired, what) in [
        (square(0, 10), 0, "a square"),
        (crossed, 8, "a square with its diagonals"),
        (ring, 8, "a square in a square"),
        (spiked, 2, "a square with a spike"),
        (apart, 0, "two squares apart"),
        (open, 0, "two segments"),
    ] {
        let a = run(&segs);
        let body = a.to_body(&mut meter()).unwrap();
        edge_and_fin_laws(&a, &body, paired, what);
        loop_and_face_laws(&body, what);
    }
}

#[test]
fn a_crossing_whole_in_x_only_is_off_the_grid() {
    // (0, 0)-(2, 1) and (0, 1)-(2, 0) cross at (1, 1/2).
    let a = run(&[seg((0, 0), (2, 1)), seg((0, 1), (2, 0))]);
    let err = a.to_body(&mut meter()).unwrap_err();
    assert!(
        matches!(err, geom_2d::arrange::BodyError::OffGrid { vertex: 2, .. }),
        "{err:?}"
    );
}

#[test]
fn a_component_above_another_s_top_is_a_hole_of_the_face_round_both() {
    // A square, a bar inside it, and a small square above it: when the bar's left end is placed,
    // the square's top passes above it, and stays in the status for the small square, whose
    // nearest edge below is that top, so it is a hole of the unbounded face, beside the square.
    let square = |x0: i64, y0: i64, x1: i64, y1: i64| {
        vec![
            seg((x0, y0), (x1, y0)),
            seg((x1, y0), (x1, y1)),
            seg((x1, y1), (x0, y1)),
            seg((x0, y1), (x0, y0)),
        ]
    };
    let mut segs = square(0, 0, 100, 100);
    segs.push(seg((5, 60), (95, 60)));
    segs.extend(square(50, 150, 55, 155));
    let a = run(&segs);
    let (v, e) = (a.vertices().len(), a.edges().len());
    assert_eq!(
        v + a.faces().len(),
        e + 1 + components(&a),
        "Euler's formula"
    );
    // The unbounded face, the big square's and the small square's.
    assert_eq!(a.faces().len(), 3);
    assert_eq!(
        a.faces()[0].holes.len(),
        2,
        "both squares lie in the unbounded face"
    );
    let big = a.faces().iter().position(|f| f.holes.len() == 1).unwrap();
    assert_ne!(big, 0);
}

#[test]
fn a_crossing_off_the_grid_is_refused_naming_the_first_such_vertex() {
    let a = run(&[seg((0, 0), (3, 1)), seg((0, 1), (3, 0))]);
    let err = a.to_body(&mut meter()).unwrap_err();
    // The vertices ascend: (0,0), (0,1), (3/2,1/2), ...: the crossing is the third.
    match err {
        geom_2d::arrange::BodyError::OffGrid { vertex, at } => {
            assert_eq!(vertex, 2);
            assert_eq!(reduced(&at), (3, 2, 1, 2));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn drawings_without_faces_are_bodies_without_fins() {
    let a = run(&[seg((0, 0), (5, 0)), seg((5, 0), (5, 5))]);
    let body = a.to_body(&mut meter()).unwrap();
    assert_eq!(rows(&body), [1, 1, 0, 0, 0, 2, 3]);
}
