//! The triangulation through the public interface (piece G153), against what defines it: every
//! edge locally Delaunay by exact tests, every constraint a chain of edges, the triangles a true
//! partition of the hull, and one answer whatever order the input comes in.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "test checks and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use geom_mesh::cdt::{Cdt, CdtError, Quality, Site, triangulate};
use geom_num::Sign;
use geom_num::grid::P2;
use geom_num::predicate::{incircle, orient2d};
use geom_run::{Budget, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use proptest::prelude::*;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

/// Sites at `points`, keyed by their order.
fn sites(points: &[(i64, i64)]) -> Vec<Site<u32>> {
    points
        .iter()
        .zip(0_u32..)
        .map(|(&(x, y), key)| Site { at: p(x, y), key })
        .collect()
}

fn run(s: &[Site<u32>], c: &[[u32; 2]]) -> Cdt<u32> {
    triangulate(s, c, None, &mut meter()).unwrap()
}

/// Shuffles by `rng` with Fisher and Yates.
fn shuffle<T>(rng: &mut SplitMix64, items: &mut [T]) {
    for i in (1..items.len()).rev() {
        let j = rng.below(NonZeroU64::new(i as u64 + 1).unwrap()) as usize;
        items.swap(i, j);
    }
}

/// Twice the signed area of `t`.
fn area(c: &Cdt<u32>, t: [u32; 3]) -> i128 {
    let [a, b, d] = t.map(|v| c.vertices[v as usize].at);
    let (ax, ay) = (i128::from(a.x.get()), i128::from(a.y.get()));
    (i128::from(b.x.get()) - ax) * (i128::from(d.y.get()) - ay)
        - (i128::from(b.y.get()) - ay) * (i128::from(d.x.get()) - ax)
}

/// The hull's twice-area, by gift wrapping in integers.
fn hull_area(points: &[P2]) -> i128 {
    let mut sorted: Vec<P2> = points.to_vec();
    sorted.sort();
    sorted.dedup();
    let half = |items: &mut dyn Iterator<Item = P2>| {
        let mut h: Vec<P2> = Vec::new();
        for q in items {
            while h.len() >= 2
                && orient2d(h[h.len() - 2], h[h.len() - 1], q).unwrap() != Sign::Positive
            {
                h.pop();
            }
            h.push(q);
        }
        h.pop();
        h
    };
    let mut hull = half(&mut sorted.iter().copied());
    hull.extend(half(&mut sorted.iter().rev().copied()));
    let mut sum = 0_i128;
    for i in 0..hull.len() {
        let (a, b) = (hull[i], hull[(i + 1) % hull.len()]);
        sum += i128::from(a.x.get()) * i128::from(b.y.get())
            - i128::from(a.y.get()) * i128::from(b.x.get());
    }
    sum
}

/// Everything a result must be: a partition of the hull, each edge used once each way or on the
/// hull, every edge not constrained locally Delaunay with ties by rank, and each constraint's
/// two ends joined by constrained edges along its line.
fn assert_valid(c: &Cdt<u32>, wanted: &[[u32; 2]]) {
    let points: Vec<P2> = c.vertices.iter().map(|v| v.at).collect();
    let mut uses: BTreeMap<[u32; 2], u32> = BTreeMap::new();
    let mut twice = 0_i128;
    for &t in &c.triangles {
        assert!(area(c, t) > 0, "{t:?} is not counter-clockwise");
        twice += area(c, t);
        let [a, b, d] = t;
        assert!(a < b && a < d, "{t:?} does not start at its lowest vertex");
        for (x, y, z) in [(a, b, d), (b, d, a), (d, a, b)] {
            assert_eq!(uses.insert([x, y], z), None, "an edge used twice one way");
        }
    }
    assert_eq!(twice, hull_area(&points), "the triangles fill the hull");
    let mut sorted = c.triangles.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, c.triangles, "sorted");
    let fixed: BTreeSet<[u32; 2]> = c.constraints.iter().copied().collect();
    for (&[x, y], &z) in &uses {
        let Some(&w) = uses.get(&[y, x]) else {
            continue;
        };
        if x > y || fixed.contains(&[x, y]) {
            continue;
        }
        let (px, py, pz, pw) = (
            points[x as usize],
            points[y as usize],
            points[z as usize],
            points[w as usize],
        );
        let inside = match incircle(px, py, pz, pw).unwrap() {
            Sign::Positive => true,
            Sign::Negative => false,
            Sign::Zero => {
                // The lowest of the four is raised most: the cofactor of its row decides.
                let low = x.min(y).min(z).min(w);
                let sign = if low == w {
                    Sign::Negative
                } else if low == x {
                    orient2d(py, pz, pw).unwrap()
                } else if low == y {
                    orient2d(px, pw, pz).unwrap()
                } else {
                    orient2d(px, py, pw).unwrap()
                };
                sign == Sign::Positive
            }
        };
        assert!(!inside, "edge {x}-{y} is not locally Delaunay");
    }
    // Each constraint walks along its fixed edges from one end to the other.
    let mut next: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &[a, b] in &c.constraints {
        next.entry(a).or_default().push(b);
        next.entry(b).or_default().push(a);
        if !c.triangles.is_empty() {
            assert!(
                uses.contains_key(&[a, b]) || uses.contains_key(&[b, a]),
                "constraint {a}-{b} is no edge"
            );
        }
    }
    for &[a, b] in wanted {
        let mut seen = BTreeSet::from([a]);
        let mut todo = vec![a];
        while let Some(v) = todo.pop() {
            for &w in next.get(&v).map_or(&[][..], Vec::as_slice) {
                if seen.insert(w) {
                    todo.push(w);
                }
            }
        }
        assert!(seen.contains(&b), "{a} and {b} are not joined");
    }
}

#[test]
fn a_square_breaks_its_tie_by_key() {
    // Four cocircular corners: the lowest key is raised most, so the diagonal misses it.
    let square = [(0, 0), (4, 0), (4, 4), (0, 4)];
    let c = run(&sites(&square), &[]);
    assert_eq!(c.triangles, [[0, 1, 3], [1, 2, 3]]);
    assert_valid(&c, &[]);
    // Key the second corner lowest and the diagonal turns.
    let rekeyed: Vec<Site<u32>> = square
        .iter()
        .zip([1, 0, 3, 2])
        .map(|(&(x, y), key)| Site { at: p(x, y), key })
        .collect();
    let d = run(&rekeyed, &[]);
    assert_eq!(d.vertices[0].at, p(4, 0));
    assert_eq!(d.triangles, [[0, 3, 1], [1, 3, 2]]);
    assert_valid(&d, &[]);
}

#[test]
fn a_constraint_overrides_the_tie() {
    let c = run(&sites(&[(0, 0), (4, 0), (4, 4), (0, 4)]), &[[0, 2]]);
    assert_eq!(c.constraints, [[0, 2]]);
    assert_eq!(c.triangles, [[0, 1, 2], [0, 2, 3]]);
    assert_valid(&c, &[[0, 2]]);
}

#[test]
fn points_on_one_line_make_no_triangle_and_split_their_constraint() {
    let c = run(&sites(&[(0, 0), (2, 2), (5, 5), (9, 9)]), &[[0, 3]]);
    assert_eq!(c.triangles.len(), 0);
    assert_eq!(c.constraints, [[0, 1], [1, 2], [2, 3]]);
    assert_eq!(run(&sites(&[]), &[]).triangles.len(), 0);
    assert_eq!(run(&sites(&[(1, 1)]), &[]).triangles.len(), 0);
    assert_eq!(run(&sites(&[(1, 1), (2, 2)]), &[]).triangles.len(), 0);
}

#[test]
fn a_point_on_a_constraint_or_a_hull_edge_splits_it() {
    let c = run(
        &sites(&[(0, 0), (8, 0), (4, 8), (4, 0), (4, 3), (2, 0)]),
        &[[0, 1]],
    );
    assert_eq!(c.constraints, [[0, 5], [1, 3], [3, 5]]);
    assert_valid(&c, &[[0, 1]]);
    let vertical = run(
        &sites(&[(0, 0), (8, 0), (4, -6), (4, 6), (4, 1), (4, -1)]),
        &[[2, 3]],
    );
    assert_eq!(vertical.constraints.len(), 3);
    assert_valid(&vertical, &[[2, 3]]);
}

#[test]
fn collinear_constraints_that_overlap_share_their_pieces() {
    let c = run(
        &sites(&[(0, 0), (4, 0), (8, 0), (12, 0), (6, 5)]),
        &[[0, 2], [1, 3]],
    );
    assert_eq!(c.constraints, [[0, 1], [1, 2], [2, 3]]);
    assert_valid(&c, &[[0, 2], [1, 3]]);
}

#[test]
fn crossing_constraints_are_cut_at_a_committed_point() {
    // An exact crossing, on the grid: no rounding.
    let c = run(
        &sites(&[(0, 0), (8, 8), (0, 8), (8, 0), (20, 4)]),
        &[[0, 1], [2, 3]],
    );
    assert_eq!(c.vertices.len(), 6);
    assert_eq!(c.vertices[5].at, p(4, 4));
    assert_eq!(c.vertices[5].key, None);
    assert_eq!(c.constraints.len(), 4);
    assert!(c.constraints.iter().all(|e| e[1] == 5));
    assert_valid(&c, &[[0, 1], [2, 3]]);
    // A crossing off the grid is rounded once, and the constraints bend through it.
    let d = run(
        &sites(&[(0, 0), (7, 2), (0, 3), (7, 0), (20, 1)]),
        &[[0, 1], [2, 3]],
    );
    assert_eq!(d.vertices.len(), 6);
    assert_eq!(d.vertices[5].at, p(4, 1));
    assert_valid(&d, &[[0, 1], [2, 3]]);
}

#[test]
fn many_crossings_settle() {
    let mut points = Vec::new();
    let mut constraints = Vec::new();
    for i in 0..8_i64 {
        points.push((i * 7, 0));
        points.push((50 - i * 6, 60));
        points.push((-5, i * 9 + 1));
        points.push((70, 58 - i * 8));
        let v = (i * 4) as u32;
        constraints.push([v, v + 1]);
        constraints.push([v + 2, v + 3]);
    }
    let c = run(&sites(&points), &constraints);
    assert_valid(&c, &constraints);
    // Whatever the order they came in, the same bytes.
    let mut rng = SplitMix64::new(Seed(7));
    let s = sites(&points);
    for _ in 0..8 {
        let mut order: Vec<u32> = (0..s.len() as u32).collect();
        shuffle(&mut rng, &mut order);
        let mut place = vec![0_u32; s.len()];
        for (new, &old) in (0_u32..).zip(&order) {
            place[old as usize] = new;
        }
        let shuffled: Vec<Site<u32>> = order.iter().map(|&i| s[i as usize]).collect();
        let mut moved: Vec<[u32; 2]> = constraints
            .iter()
            .map(|e| e.map(|v| place[v as usize]))
            .collect();
        shuffle(&mut rng, &mut moved);
        assert_eq!(run(&shuffled, &moved), c);
    }
}

#[test]
fn a_cocircular_grid_is_one_answer_in_any_order() {
    let mut points = Vec::new();
    for y in 0..24 {
        for x in 0..24 {
            points.push((x * 16, y * 16));
        }
    }
    let s = sites(&points);
    let c = run(&s, &[]);
    assert_eq!(c.triangles.len(), 2 * 23 * 23);
    assert_valid(&c, &[]);
    let mut rng = SplitMix64::new(Seed(11));
    for _ in 0..4 {
        let mut shuffled = s.clone();
        shuffle(&mut rng, &mut shuffled);
        assert_eq!(run(&shuffled, &[]), c);
    }
}

#[test]
fn refusals_name_their_input() {
    let bad = |e: &[[u32; 2]]| triangulate(&sites(&[(0, 0), (1, 0)]), e, None, &mut meter());
    assert_eq!(bad(&[[0, 2]]), Err(CdtError::BadConstraint { index: 0 }));
    assert_eq!(bad(&[[2, 0]]), Err(CdtError::BadConstraint { index: 0 }));
    assert_eq!(
        bad(&[[0, 1], [1, 1]]),
        Err(CdtError::BadConstraint { index: 1 })
    );
    let twice = [
        Site {
            at: p(0, 0),
            key: 1_u32,
        },
        Site {
            at: p(1, 0),
            key: 1,
        },
    ];
    assert_eq!(
        triangulate(&twice, &[], None, &mut meter()),
        Err(CdtError::DuplicateKey {
            first: 0,
            second: 1
        })
    );
    let same = sites(&[(3, 3), (0, 0), (3, 3)]);
    assert_eq!(
        triangulate(&same, &[], None, &mut meter()),
        Err(CdtError::DuplicatePoint {
            first: 0,
            second: 2
        })
    );
}

#[test]
fn a_stopped_meter_stops_it() {
    let s = sites(&[(0, 0), (9, 0), (9, 9), (0, 9), (4, 5), (5, 4), (2, 7)]);
    let mut small = WorkMeter::new(Budget::of_work(2));
    assert!(matches!(
        triangulate(&s, &[], None, &mut small),
        Err(CdtError::Stopped(_))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn random_points_and_constraints_are_valid_and_one_answer(
        raw in proptest::collection::btree_set((0_i64..40, 0_i64..40), 3..40),
        pairs in proptest::collection::vec((0_usize..64, 0_usize..64), 0..8),
        seed in any::<u64>(),
    ) {
        let points: Vec<(i64, i64)> = raw.into_iter().collect();
        let n = points.len();
        let constraints: Vec<[u32; 2]> = pairs
            .into_iter()
            .map(|(a, b)| [(a % n) as u32, (b % n) as u32])
            .filter(|e| e[0] != e[1])
            .collect();
        let s = sites(&points);
        let c = run(&s, &constraints);
        assert_valid(&c, &constraints);
        let mut rng = SplitMix64::new(Seed(seed));
        let mut order: Vec<u32> = (0..n as u32).collect();
        shuffle(&mut rng, &mut order);
        let mut place = vec![0_u32; n];
        for (new, &old) in (0_u32..).zip(&order) {
            place[old as usize] = new;
        }
        let shuffled: Vec<Site<u32>> = order.iter().map(|&i| s[i as usize]).collect();
        let moved: Vec<[u32; 2]> = constraints
            .iter()
            .map(|e| e.map(|v| place[v as usize]))
            .collect();
        prop_assert_eq!(run(&shuffled, &moved), c);
    }
}

#[test]
fn seeded_large_scatter_is_valid() {
    let seed = resolve("geom-mesh cdt scatter", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut found = BTreeSet::new();
    while found.len() < 4000 {
        let x = rng.below(NonZeroU64::new(1 << 30).unwrap()) as i64;
        let y = rng.below(NonZeroU64::new(1 << 30).unwrap()) as i64;
        found.insert((x, y));
    }
    let points: Vec<(i64, i64)> = found.into_iter().collect();
    let constraints: Vec<[u32; 2]> = (0..40)
        .map(|i| [i * 7 % 4000, (i * 7 + 2000) % 4000])
        .collect();
    let c = run(&sites(&points), &constraints);
    assert_valid(&c, &constraints);
}

/// The ratio of circumradius to shortest edge, squared, and the shortest and longest edges, squared.
fn shape(c: &Cdt<u32>, t: [u32; 3]) -> (f64, f64, f64) {
    let [a, b, d] = t.map(|v| c.vertices[v as usize].at);
    let len = |p: P2, q: P2| {
        let (dx, dy) = (
            (q.x.get() - p.x.get()) as f64,
            (q.y.get() - p.y.get()) as f64,
        );
        dx * dx + dy * dy
    };
    let mut l = [len(a, b), len(b, d), len(d, a)];
    l.sort_by(f64::total_cmp);
    let x = area(c, t) as f64;
    (l[1] * l[2] / (4.0 * x * x), l[0], l[2])
}

const FINE: Quality = Quality {
    ratio_squared: 2.0,
    max_edge_squared: None,
    min_edge_squared: 16,
};

#[test]
fn refinement_meets_its_bound_and_keeps_every_rule() {
    // An L with a constraint across it, and a thin triangle at one corner.
    let points = [
        (0, 0),
        (4000, 0),
        (4000, 1500),
        (1500, 1500),
        (1500, 4000),
        (0, 4000),
        (3900, 40),
    ];
    let outline: Vec<[u32; 2]> = (0..6).map(|i| [i, (i + 1) % 6]).collect();
    let s = sites(&points);
    let c = triangulate(&s, &outline, Some(&FINE), &mut meter()).unwrap();
    assert!(c.vertices.len() > points.len(), "it added points");
    assert!(c.vertices[points.len()..].iter().all(|v| v.key.is_none()));
    assert_valid(&c, &outline);
    for &t in &c.triangles {
        let (ratio, low, _) = shape(&c, t);
        assert!(ratio <= 2.0 || low <= 16.0, "{t:?}: {ratio} {low}");
    }
    // One answer, whatever order the input came in.
    let mut rng = SplitMix64::new(Seed(5));
    let mut order: Vec<u32> = (0..s.len() as u32).collect();
    shuffle(&mut rng, &mut order);
    let mut place = vec![0_u32; s.len()];
    for (new, &old) in (0_u32..).zip(&order) {
        place[old as usize] = new;
    }
    let shuffled: Vec<Site<u32>> = order.iter().map(|&i| s[i as usize]).collect();
    let moved: Vec<[u32; 2]> = outline
        .iter()
        .map(|e| e.map(|v| place[v as usize]))
        .collect();
    let again = triangulate(&shuffled, &moved, Some(&FINE), &mut meter()).unwrap();
    assert_eq!(again, c);
}

#[test]
fn a_longest_edge_bound_is_met() {
    let q = Quality {
        ratio_squared: 4.0,
        max_edge_squared: Some(250_000),
        min_edge_squared: 1,
    };
    let s = sites(&[(0, 0), (3000, 0), (3000, 3000), (0, 3000)]);
    let c = triangulate(&s, &[], Some(&q), &mut meter()).unwrap();
    assert_valid(&c, &[]);
    for &t in &c.triangles {
        let (_, low, high) = shape(&c, t);
        assert!(high <= 250_000.0 || low <= 1.0, "{t:?}: {high}");
    }
}

#[test]
fn a_small_input_angle_ends() {
    // Two constraints from one point at a tiny angle: refinement must stop at its floor.
    let s = sites(&[(0, 0), (6000, 0), (6000, 30), (0, 3000), (6000, 3000)]);
    let c = triangulate(&s, &[[0, 1], [0, 2]], Some(&FINE), &mut meter()).unwrap();
    assert_valid(&c, &[[0, 1], [0, 2]]);
}
