//! Tests of `geom_sketch::cluster` and `geom_sketch::place` (piece G223): rigid clusters by the
//! Hoffmann–Lomonosov–Sitharam method, generic in D.
//!
//! The board's done-when: "Hoffmann-Lomonosov-Sitharam's Dense and Minimal algorithms, Modified
//! Frontier recombination, the ground cluster and Kabsch placement, generic in D; Laman families,
//! the HLS examples and shuffle determinism tested; a 1,000-entity family in 10 ms." The benchmark
//! is `benches/cluster.rs`.
//!
//! What this file proves: on the classical examples (a triangle, two triangles on an edge, two
//! triangles on a vertex, a four-bar linkage, a redundant bar, the ground, the tetrahedron in
//! space), the plan is the one the definitions give; on random Laman graphs built by Henneberg's
//! two moves (Henneberg 1911; Laman 1970, doi:10.1007/BF01534980) the whole graph is one rigid
//! cluster, and with one bar removed it is not; a plan is the same for every order of the input;
//! a component past the size limit is reported and left alone; and a budget too small refuses.
//! The paper's own figures are not reproduced: they were not available to the build, so the
//! examples here are the standard ones the papers describe in words.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;

use geom_run::{Budget, WorkMeter};
use geom_sketch::cluster::{Child, Link, Node, Plan, Problem, SIZE_LIMIT, plan};
use geom_sketch::model::SketchError;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// `n` points in the plane, keys 100 upwards, two freedoms each.
fn points(n: usize) -> Vec<Node> {
    (0..n)
        .map(|i| Node {
            key: 100 + i as u64,
            weight: 2,
            fixed: false,
        })
        .collect()
}

/// A distance between two nodes, key `key`.
fn bar(key: u64, a: usize, b: usize) -> Link {
    Link {
        key,
        members: vec![a, b],
        weight: 1,
    }
}

fn planar(nodes: Vec<Node>, links: Vec<Link>) -> Problem {
    Problem {
        motions: 3,
        nodes,
        links,
    }
}

fn run(p: &Problem) -> Plan {
    plan(p, SIZE_LIMIT, &mut meter()).unwrap()
}

/// Every entity key under a child, by an explicit stack.
fn keys_under(plan: &Plan, child: Child) -> BTreeSet<u64> {
    let mut keys = BTreeSet::new();
    let mut stack = vec![child];
    while let Some(c) = stack.pop() {
        match c {
            Child::Node(k) => {
                keys.insert(k);
            }
            Child::Cluster(i) => stack.extend(plan.clusters[i].children.iter().copied()),
        }
    }
    keys
}

#[test]
fn a_triangle_of_bars_is_one_rigid_cluster() {
    let p = planar(points(3), vec![bar(1, 0, 1), bar(2, 1, 2), bar(3, 2, 0)]);
    let plan = run(&p);
    // The first bar is a rigid pair (a minimal dense set); the two bars left join it to the third
    // point: 3 + 2 - 2 = 3.
    assert_eq!(plan.clusters.len(), 2, "{plan:?}");
    assert_eq!(
        plan.clusters[0].children,
        [Child::Node(100), Child::Node(101)]
    );
    assert_eq!(plan.clusters[0].links, [1]);
    assert_eq!(
        plan.clusters[1].children,
        [Child::Cluster(0), Child::Node(102)]
    );
    assert_eq!(plan.clusters[1].links, [2, 3]);
    assert!(
        plan.clusters
            .iter()
            .all(|c| !c.grounded && c.redundant == 0)
    );
    assert_eq!(plan.top, [Child::Cluster(1)]);
    assert_eq!(
        keys_under(&plan, plan.top[0]),
        BTreeSet::from([100, 101, 102])
    );
}

#[test]
fn two_bars_and_a_point_are_not() {
    // A path of two bars is floppy: it can fold. No cluster, three loose points.
    let p = planar(points(3), vec![bar(1, 0, 1), bar(2, 1, 2)]);
    let plan = run(&p);
    // A bar alone joins two points: 4 - 1 = 3 freedoms, a rigid body. The first bar makes that
    // cluster, and the second then joins it to the third point: 3 + 2 - 1 = 4 freedoms, floppy.
    assert_eq!(plan.clusters.len(), 1, "{plan:?}");
    assert_eq!(plan.top.len(), 2);
}

#[test]
fn a_four_bar_linkage_is_floppy() {
    // Four points, four bars in a ring: 8 - 4 = 4 freedoms, one more than a body.
    let p = planar(
        points(4),
        vec![bar(1, 0, 1), bar(2, 1, 2), bar(3, 2, 3), bar(4, 3, 0)],
    );
    let plan = run(&p);
    // Each bar alone is a rigid pair, so two pairs form, and the two bars left between them are
    // 3 + 3 - 2 = 4 freedoms: a four-bar linkage, with its one internal freedom, is not a body.
    assert_eq!(plan.top.len(), 2, "{plan:?}");
    assert!(
        plan.clusters.iter().all(|c| c.children.len() == 2),
        "{plan:?}"
    );
}

#[test]
fn two_triangles_on_an_edge_join_into_one_body() {
    // Points 0,1,2 and 1,2,3: five bars, 8 - 5 = 3 freedoms.
    let p = planar(
        points(4),
        vec![
            bar(1, 0, 1),
            bar(2, 1, 2),
            bar(3, 2, 0),
            bar(4, 1, 3),
            bar(5, 2, 3),
        ],
    );
    let plan = run(&p);
    assert_eq!(plan.top.len(), 1, "{plan:?}");
    assert_eq!(
        keys_under(&plan, plan.top[0]),
        BTreeSet::from([100, 101, 102, 103])
    );
    // Cluster 0 is the first pair by key order, cluster 1 the first triangle, cluster 2 the body.
    assert_eq!(plan.clusters.len(), 3, "{plan:?}");
    assert_eq!(
        plan.clusters[0].children,
        [Child::Node(100), Child::Node(101)]
    );
    assert_eq!(plan.clusters.last().unwrap().redundant, 0);
}

#[test]
fn two_triangles_on_a_vertex_are_a_cluster_and_two_loose_points() {
    // Triangles 0,1,2 and 2,3,4 share point 2: a hinge. The first body takes the shared point; the
    // other two points then make a pair, and the two bars left between the bodies are
    // 3 + 3 - 2 = 4 freedoms: a hinge with one turning freedom, not a body.
    let p = planar(
        points(5),
        vec![
            bar(1, 0, 1),
            bar(2, 1, 2),
            bar(3, 2, 0),
            bar(4, 2, 3),
            bar(5, 3, 4),
            bar(6, 4, 2),
        ],
    );
    let plan = run(&p);
    let mut sizes: Vec<usize> = plan
        .top
        .iter()
        .map(|c| keys_under(&plan, *c).len())
        .collect();
    sizes.sort_unstable();
    assert_eq!(sizes, [2, 3], "{plan:?}");
}

#[test]
fn a_redundant_bar_is_counted() {
    // A triangle with a fourth, doubled bar on one side: 6 - 4 = 2 freedoms, one equation too many.
    let p = planar(
        points(3),
        vec![bar(1, 0, 1), bar(2, 1, 2), bar(3, 2, 0), bar(4, 0, 1)],
    );
    let plan = run(&p);
    // The doubled side is a pair with two bars, 4 - 2 = 2 freedoms: one equation too many.
    assert_eq!(plan.clusters.len(), 2, "{plan:?}");
    assert_eq!(plan.clusters[0].redundant, 1);
    assert_eq!(plan.clusters[0].links, [1, 4]);
    assert_eq!(plan.clusters[1].redundant, 0);
}

#[test]
fn the_fixed_nodes_are_the_ground_cluster_and_it_absorbs_what_it_pins() {
    // Two fixed points; a point 2 joined to both by a bar is pinned (2 + 3 - 2 = 3 freedoms:
    // dense with the ground), a point 3 joined to one is not.
    let mut nodes = points(4);
    nodes[0].fixed = true;
    nodes[1].fixed = true;
    let p = planar(
        nodes,
        vec![bar(1, 0, 2), bar(2, 1, 2), bar(3, 2, 3), bar(4, 0, 1)],
    );
    let plan = run(&p);
    assert!(plan.clusters[0].grounded);
    assert_eq!(
        plan.clusters[0].children,
        [Child::Node(100), Child::Node(101)]
    );
    assert_eq!(plan.clusters[0].links, [4]);
    assert!(plan.clusters.iter().all(|c| c.grounded));
    let grounded_body = keys_under(&plan, Child::Cluster(plan.clusters.len() - 1));
    assert!(grounded_body.contains(&102));
    // Point 3 hangs by one bar: free to swing.
    assert!(plan.top.contains(&Child::Node(103)), "{plan:?}");
}

#[test]
fn a_cluster_without_the_ground_keeps_its_freedoms() {
    let mut nodes = points(4);
    nodes[0].fixed = true;
    let p = planar(
        nodes,
        vec![bar(1, 1, 2), bar(2, 2, 3), bar(3, 3, 1), bar(4, 0, 1)],
    );
    let plan = run(&p);
    // Points 1, 2, 3 form a body; the one bar from the fixed point leaves it 3 + 3 - 1 = 5.
    let body = plan.clusters.last().unwrap();
    assert!(!body.grounded, "{plan:?}");
    assert_eq!(
        keys_under(&plan, Child::Cluster(plan.clusters.len() - 1)).len(),
        3
    );
    assert!(plan.clusters[0].grounded);
}

/// Parts of an assembly: six freedoms each, joined by mates of the given number of equations
/// (D = 6, chapter 15.15).
fn parts(n: usize) -> Vec<Node> {
    (0..n)
        .map(|i| Node {
            key: 10 + i as u64,
            weight: 6,
            fixed: false,
        })
        .collect()
}

fn mate(key: u64, a: usize, b: usize, equations: u32) -> Link {
    Link {
        key,
        members: vec![a, b],
        weight: equations,
    }
}

fn spatial(nodes: Vec<Node>, links: Vec<Link>) -> Problem {
    Problem {
        motions: 6,
        nodes,
        links,
    }
}

#[test]
fn six_motions_count_assembly_mates() {
    // Two parts welded (a mate of six equations) are one body: 12 - 6 = 6. A chain of welds is
    // one body of all its parts.
    let chain = run(&spatial(
        parts(4),
        vec![mate(1, 0, 1, 6), mate(2, 1, 2, 6), mate(3, 2, 3, 6)],
    ));
    assert_eq!(chain.top.len(), 1, "{chain:?}");
    assert_eq!(keys_under(&chain, chain.top[0]).len(), 4);
    // A hinge (five equations) leaves one turning freedom: 12 - 5 = 7 > 6, not a body.
    let hinge = run(&spatial(parts(2), vec![mate(1, 0, 1, 5)]));
    assert!(hinge.clusters.is_empty(), "{hinge:?}");
    assert_eq!(hinge.top.len(), 2);
    // A loop of three parts with three-equation mates: 18 - 9 = 9, floppy.
    let ring = run(&spatial(
        parts(3),
        vec![mate(1, 0, 1, 3), mate(2, 1, 2, 3), mate(3, 2, 0, 3)],
    ));
    assert!(ring.clusters.is_empty(), "{ring:?}");
}

#[test]
fn the_ground_of_an_assembly_pins_a_part_with_two_hinges() {
    // Part 0 is fixed. One hinge to part 1 lets it swing (6 + 6 - 5 = 7); a second hinge on
    // another axis locks it (12 - 10 = 2, with four equations redundant by the count).
    let mut nodes = parts(2);
    nodes[0].fixed = true;
    let swinging = run(&spatial(nodes.clone(), vec![mate(1, 0, 1, 5)]));
    assert_eq!(swinging.clusters.len(), 1, "{swinging:?}");
    assert!(swinging.clusters[0].grounded);
    assert_eq!(swinging.top.len(), 2);
    let locked = run(&spatial(nodes, vec![mate(1, 0, 1, 5), mate(2, 0, 1, 5)]));
    assert_eq!(locked.clusters.len(), 2, "{locked:?}");
    assert!(locked.clusters[1].grounded);
    assert_eq!(locked.clusters[1].redundant, 4);
    assert_eq!(locked.top, [Child::Cluster(1)]);
}

/// A deterministic generator (`SplitMix64`).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A random Laman graph on `n` vertices by Henneberg's moves: add a vertex joined to two others,
/// or add one joined to the ends of a bar it replaces and to a third vertex.
fn laman(n: usize, rng: &mut Rng) -> Vec<(usize, usize)> {
    let mut edges = vec![(0, 1)];
    for v in 2..n {
        if edges.len() > 1 && rng.below(3) == 0 {
            let at = rng.below(edges.len());
            let (a, b) = edges.swap_remove(at);
            let mut c = rng.below(v);
            while c == a || c == b {
                c = rng.below(v);
            }
            edges.extend([(a, v), (b, v), (c, v)]);
        } else {
            let a = rng.below(v);
            let mut b = rng.below(v);
            while b == a {
                b = rng.below(v);
            }
            edges.extend([(a, v), (b, v)]);
        }
    }
    edges
}

fn bars(edges: &[(usize, usize)]) -> Vec<Link> {
    edges
        .iter()
        .enumerate()
        .map(|(i, &(a, b))| bar(1 + i as u64, a, b))
        .collect()
}

#[test]
fn a_laman_graph_is_one_rigid_cluster_and_without_a_bar_it_is_not() {
    let mut rng = Rng(0x6223_0000_5eed_0223);
    for n in [3, 4, 5, 8, 13, 21, 34] {
        for _ in 0..4 {
            let edges = laman(n, &mut rng);
            assert_eq!(edges.len(), 2 * n - 3);
            let rigid = run(&planar(points(n), bars(&edges)));
            assert_eq!(rigid.top.len(), 1, "n = {n}: {edges:?}");
            assert_eq!(keys_under(&rigid, rigid.top[0]).len(), n);
            assert_eq!(rigid.clusters.last().unwrap().redundant, 0);
            // Every constraint is consumed by exactly one cluster.
            let consumed = rigid.clusters.iter().fold(0, |n, c| n + c.links.len());
            assert_eq!(consumed, edges.len());
            // Take any one bar away: 2n - 4 bars cannot hold n points rigid.
            if n >= 4 {
                let cut = rng.below(edges.len());
                let mut fewer = edges.clone();
                fewer.remove(cut);
                let loose = run(&planar(points(n), bars(&fewer)));
                assert!(
                    loose.top.iter().all(|c| keys_under(&loose, *c).len() < n),
                    "n = {n}: {fewer:?}"
                );
            }
        }
    }
}

#[test]
fn a_cluster_is_minimal_when_it_is_made() {
    // The rectangle with a diagonal: pairs are minimal, so the first cluster is the pair of the first
    // bar, then a triangle, then the rectangle: never the four points at once.
    let p = planar(
        points(4),
        vec![
            bar(1, 0, 1),
            bar(2, 1, 2),
            bar(3, 2, 3),
            bar(4, 3, 0),
            bar(5, 0, 2),
        ],
    );
    let plan = run(&p);
    assert_eq!(plan.clusters.len(), 3);
    assert_eq!(
        plan.clusters[0].children,
        [Child::Node(100), Child::Node(101)]
    );
    assert_eq!(
        plan.clusters[1].children,
        [Child::Cluster(0), Child::Node(102)]
    );
    assert_eq!(
        plan.clusters[2].children,
        [Child::Cluster(1), Child::Node(103)]
    );
    assert_eq!(plan.top, [Child::Cluster(2)]);
}

#[test]
fn the_plan_is_the_same_for_every_order_of_the_input() {
    let mut rng = Rng(0x6223_0000_0000_beef);
    for n in [6, 12, 25] {
        let edges = laman(n, &mut rng);
        let mut fewer = edges.clone();
        fewer.remove(0);
        for graph in [edges, fewer] {
            let base = planar(points(n), bars(&graph));
            let want = run(&base);
            for _ in 0..8 {
                // Shuffle the nodes (and so every member index), the links and each link's members.
                let mut order: Vec<usize> = (0..n).collect();
                for i in (1..n).rev() {
                    order.swap(i, rng.below(i + 1));
                }
                let mut at = vec![0; n];
                for (new, &old) in order.iter().enumerate() {
                    at[old] = new;
                }
                let nodes: Vec<Node> = order.iter().map(|&old| base.nodes[old]).collect();
                let mut links: Vec<Link> = base
                    .links
                    .iter()
                    .map(|l| Link {
                        key: l.key,
                        members: l.members.iter().rev().map(|&m| at[m]).collect(),
                        weight: l.weight,
                    })
                    .collect();
                for i in (1..links.len()).rev() {
                    links.swap(i, rng.below(i + 1));
                }
                let shuffled = Problem {
                    motions: 3,
                    nodes,
                    links,
                };
                assert_eq!(run(&shuffled), want, "n = {n}");
            }
        }
    }
}

#[test]
fn a_component_past_the_size_limit_is_named_and_left_alone() {
    // A triangle apart (3 nodes) and a rigid ring of 5 (past a limit of 4).
    let mut nodes = points(8);
    for (i, n) in nodes.iter_mut().enumerate() {
        n.key = 200 - i as u64;
    }
    let mut links = bars(&[(0, 1), (1, 2), (2, 0)]);
    let ring = [(3, 4), (4, 5), (5, 6), (6, 7), (7, 3), (3, 5), (4, 6)];
    links.extend(
        ring.iter()
            .enumerate()
            .map(|(i, &(a, b))| bar(50 + i as u64, a, b)),
    );
    let p = planar(nodes, links);
    let plan = plan(&p, 4, &mut meter()).unwrap();
    assert_eq!(plan.clusters.len(), 2, "{plan:?}");
    assert_eq!(plan.clusters[1].children.len(), 2);
    // The lowest key of the skipped component: its nodes are 3..8, keys 197 down to 193.
    assert_eq!(plan.too_large, [193]);
    assert_eq!(plan.top.len(), 1 + 5);
}

#[test]
fn a_budget_too_small_refuses_and_bad_input_is_an_error() {
    let p = planar(points(3), vec![bar(1, 0, 1), bar(2, 1, 2), bar(3, 2, 0)]);
    let mut none = WorkMeter::new(Budget::of_work(1));
    assert_eq!(plan(&p, SIZE_LIMIT, &mut none), Err(SketchError::Stopped));
    let past = planar(points(2), vec![bar(1, 0, 5)]);
    assert_eq!(
        plan(&past, SIZE_LIMIT, &mut meter()),
        Err(SketchError::Internal)
    );
    let twice = planar(
        vec![
            Node {
                key: 1,
                weight: 2,
                fixed: false,
            };
            2
        ],
        vec![],
    );
    assert_eq!(
        plan(&twice, SIZE_LIMIT, &mut meter()),
        Err(SketchError::Internal)
    );
    let empty = planar(
        points(2),
        vec![Link {
            key: 1,
            members: vec![],
            weight: 1,
        }],
    );
    assert_eq!(
        plan(&empty, SIZE_LIMIT, &mut meter()),
        Err(SketchError::Internal)
    );
}

#[test]
fn a_family_of_rigid_bodies_gives_one_body_each_and_a_floppy_chain_joins_none() {
    // 250 rectangles of four points and five bars (four sides and a diagonal): the 1,000-entity
    // family of the board. Each is a pair, a triangle and the body: three clusters.
    let family = |bodies: usize, chain: bool| {
        let mut links = Vec::new();
        for b in 0..bodies {
            let o = 4 * b;
            for (i, (x, y)) in [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)]
                .into_iter()
                .enumerate()
            {
                links.push(bar(10 * b as u64 + i as u64, o + x, o + y));
            }
            if chain && b > 0 {
                // One bar between bodies: 3 + 3 - 1 = 5 freedoms, floppy.
                links.push(bar(100_000 + b as u64, o - 1, o));
            }
        }
        planar(points(4 * bodies), links)
    };
    let plan = run(&family(250, false));
    assert_eq!(plan.clusters.len(), 3 * 250);
    assert_eq!(plan.top.len(), 250);
    let chained = run(&family(12, true));
    assert_eq!(chained.clusters.len(), 3 * 12);
    assert_eq!(chained.top.len(), 12);
}
