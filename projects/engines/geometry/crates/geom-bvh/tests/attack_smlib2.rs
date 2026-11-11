//! Attack tests from SMLib's confirmed defects, batch 2: `geom-bvh`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the exact answer (every answer here is compared with brute
//! force), or a defined refusal, and never a panic, a hang or a wrong answer.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::manual_midpoint,
    clippy::type_complexity,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::range_minus_one,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), prose that names defect files, and a failure stops the test"
)]

use geom_bvh::{Aabb, BoxTree, CellIndex, Entry, MAX_DEPTH, Point};
use geom_num::grid::{COORD_MAX, P3};
use geom_run::{Budget, Exec, WorkMeter};

/// The work each build and query here may do: at least 64 times what the largest correct one
/// needs, and little enough that a walk which never ends, over a tree whose child links lead back
/// up, stops within a second with `OverBudget` instead of hanging this binary. Under an unlimited
/// meter such a walk ran until its stack could not grow, so whether the mutation gate saw those
/// mutants caught or timed out depended on the runner (decision 32.46).
const WORK: u64 = 1 << 24;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(WORK))
}

fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn xyz(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// A deterministic stream of small numbers (a linear congruential generator).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> i64 {
        (self.next() % n) as i64
    }
}

/// The squared distance of two grid points, exactly.
fn dist2(a: [i64; 3], b: [i64; 3]) -> u128 {
    (0..3)
        .map(|i| {
            let d = i128::from(a[i]) - i128::from(b[i]);
            (d * d) as u128
        })
        .sum()
}

/// The keys within `radius` of `center`, by brute force, ascending.
fn brute_within(points: &[Point], center: [i64; 3], radius: u64) -> Vec<u64> {
    let r2 = u128::from(radius) * u128::from(radius);
    let mut out: Vec<u64> = points
        .iter()
        .filter(|p| dist2(xyz(p.at), center) <= r2)
        .map(|p| p.key)
        .collect();
    out.sort_unstable();
    out
}

/// Every key pair within `radius`, by brute force, as (smaller, larger), ascending.
fn brute_pairs(points: &[Point], radius: u64) -> Vec<(u64, u64)> {
    let r2 = u128::from(radius) * u128::from(radius);
    let mut out = Vec::new();
    for (i, a) in points.iter().enumerate() {
        for b in &points[i + 1..] {
            if dist2(xyz(a.at), xyz(b.at)) <= r2 {
                out.push((a.key.min(b.key), a.key.max(b.key)));
            }
        }
    }
    out.sort_unstable();
    out
}

fn sorted_pairs(mut v: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    for p in &mut v {
        *p = (p.0.min(p.1), p.0.max(p.1));
    }
    v.sort_unstable();
    v
}

/// solidcore plan/upstream-issues/WR-12--smobjsinvoxels-findcoincidentpairs-misses-pairs-and-changes.md
/// (b4de2149): the search for coincident pairs missed pairs: with three coincident points in one
/// voxel the pair not involving the head of the list was never reported. The input is the
/// defect's: three coincident points, then 4, 10 and 40, each with other points in the same cell
/// and beside it. Ours must give every pair, equal to brute force, for radius 0 and others, and
/// the same pairs whichever order the points are given in.
#[test]
fn wr_12_coincident_points_give_every_pair() {
    let exec = Exec::serial();
    for count in [3usize, 4, 10, 40] {
        let mut points: Vec<Point> = (0..count)
            .map(|k| Point {
                key: 100 + k as u64,
                at: p3(7, -3, 2),
            })
            .collect();
        // A neighbour in the same cell and others at distance 1 and 5.
        points.push(Point {
            key: 1,
            at: p3(8, -3, 2),
        });
        points.push(Point {
            key: 2,
            at: p3(7, 2, 2),
        });
        points.push(Point {
            key: 3,
            at: p3(1000, 1000, 1000),
        });
        let index = CellIndex::build(&points, &exec, &mut meter()).unwrap();
        let mut reversed = points.clone();
        reversed.reverse();
        let other = CellIndex::build(&reversed, &exec, &mut meter()).unwrap();
        for radius in [0u64, 1, 5, 6, 2000, u64::MAX] {
            let want = brute_pairs(&points, radius);
            let got = index.pairs_within(radius, &exec, &mut meter()).unwrap();
            assert_eq!(
                sorted_pairs(got.clone()),
                want,
                "{count} coincident, radius {radius}"
            );
            assert_eq!(
                other.pairs_within(radius, &exec, &mut meter()).unwrap(),
                got,
                "order of the points changed the pairs, radius {radius}"
            );
            for c in [[7, -3, 2], [8, -3, 2], [0, 0, 0]] {
                assert_eq!(
                    index
                        .within(p3(c[0], c[1], c[2]), radius, &mut meter())
                        .unwrap(),
                    brute_within(&points, c, radius),
                    "{count} coincident, centre {c:?}, radius {radius}"
                );
            }
        }
        // Radius 0: exactly the coincident points pair among themselves.
        let zero = index.pairs_within(0, &exec, &mut meter()).unwrap();
        assert_eq!(zero.len(), count * (count - 1) / 2);
    }
}

/// solidcore plan/upstream-issues/UB-34--smobjsinvoxels-incmaxitemcount-re-points-buckets-into-the.md
/// and UB-39--smgrid-indexing-and-conversion-hazards.md (b4de2149): adding items beyond the
/// reserved size re-pointed the buckets into the old storage, and a grid with a zero extent on
/// one axis divided by it. The inputs are the defects': index sizes either side of every small
/// reserve (1 to 9, 15, 16, 17, 100), all points on one plane, on one line, in one cell, and at
/// the ends of the frame. Every answer equals brute force.
#[test]
fn ub_34_ub_39_any_size_and_any_flat_set_of_points_agrees_with_brute_force() {
    let exec = Exec::serial();
    let mut rng = Lcg(0x5eed);
    for n in [1usize, 2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 100] {
        let layouts: [(&str, Box<dyn Fn(&mut Lcg) -> [i64; 3]>); 6] = [
            (
                "cloud",
                Box::new(|r: &mut Lcg| [r.below(40) - 20, r.below(40) - 20, r.below(40) - 20]),
            ),
            (
                "plane z = 0",
                Box::new(|r: &mut Lcg| [r.below(40) - 20, r.below(40) - 20, 0]),
            ),
            (
                "line along x",
                Box::new(|r: &mut Lcg| [r.below(40) - 20, 5, 5]),
            ),
            ("one cell", Box::new(|_: &mut Lcg| [3, 3, 3])),
            (
                "frame ends",
                Box::new(|r: &mut Lcg| {
                    let e = |r: &mut Lcg| {
                        if r.below(2) == 0 {
                            COORD_MAX
                        } else {
                            -COORD_MAX
                        }
                    };
                    [e(r), e(r), e(r)]
                }),
            ),
            (
                "one cell column",
                Box::new(|r: &mut Lcg| [0, 0, r.below(1 << 20)]),
            ),
        ];
        for (name, make) in &layouts {
            let points: Vec<Point> = (0..n)
                .map(|k| {
                    let [x, y, z] = make(&mut rng);
                    Point {
                        key: 10 * k as u64 + 1,
                        at: p3(x, y, z),
                    }
                })
                .collect();
            let index = CellIndex::build(&points, &exec, &mut meter()).unwrap();
            assert_eq!(index.len(), n, "{name}, n = {n}");
            for radius in [0u64, 1, 3, 10, 1 << 20, u64::MAX] {
                assert_eq!(
                    sorted_pairs(index.pairs_within(radius, &exec, &mut meter()).unwrap()),
                    brute_pairs(&points, radius),
                    "{name}, n = {n}, radius {radius}"
                );
                for k in [0, n / 2, n - 1] {
                    let c = xyz(points[k].at);
                    assert_eq!(
                        index.within(points[k].at, radius, &mut meter()).unwrap(),
                        brute_within(&points, c, radius),
                        "{name}, n = {n}, radius {radius}, centre {c:?}"
                    );
                }
            }
        }
    }
}

/// solidcore plan/upstream-issues/UB-38--point-trees-divide-by-zero-in-addtospatialtree.md
/// (b4de2149): a tree of points divided by zero when a point was added, because the points had
/// no size. The input is the defect's: a tree built over boxes of zero size, a single location,
/// repeated, on one axis, and at the ends of the frame. Ours must give the brute-force overlaps
/// and pairs, and never be deeper than `MAX_DEPTH`.
#[test]
fn ub_38_a_tree_over_boxes_of_zero_size_agrees_with_brute_force() {
    let exec = Exec::serial();
    let b = |lo: [i64; 3], hi: [i64; 3]| Aabb::<3>::new(lo, hi).unwrap();
    let mut rng = Lcg(77);
    for n in [1usize, 2, 5, 33, 200] {
        let layouts: [(&str, Box<dyn Fn(&mut Lcg) -> Aabb<3>>); 4] = [
            (
                "one location",
                Box::new(|_: &mut Lcg| Aabb::<3>::new([4, 4, 4], [4, 4, 4]).unwrap()),
            ),
            (
                "points on a line",
                Box::new(|r: &mut Lcg| {
                    let x = r.below(30);
                    Aabb::<3>::new([x, 0, 0], [x, 0, 0]).unwrap()
                }),
            ),
            (
                "flat boxes",
                Box::new(|r: &mut Lcg| {
                    let (x, y) = (r.below(30), r.below(30));
                    Aabb::<3>::new([x, y, 7], [x + r.below(5), y + r.below(5), 7]).unwrap()
                }),
            ),
            (
                "frame corners",
                Box::new(|r: &mut Lcg| {
                    let c = if r.below(2) == 0 {
                        COORD_MAX
                    } else {
                        -COORD_MAX
                    };
                    Aabb::<3>::new([c, c, c], [c, c, c]).unwrap()
                }),
            ),
        ];
        for (name, make) in &layouts {
            let entries: Vec<Entry<3>> = (0..n)
                .map(|k| Entry {
                    key: k as u64 + 1,
                    bbox: make(&mut rng),
                })
                .collect();
            let tree = BoxTree::build(&entries, &exec, &mut meter()).unwrap();
            assert_eq!(tree.len(), n, "{name}");
            // The pairs of overlapping boxes.
            let mut want = Vec::new();
            for (i, a) in entries.iter().enumerate() {
                for c in &entries[i + 1..] {
                    if a.bbox.overlaps(&c.bbox) {
                        want.push((a.key.min(c.key), a.key.max(c.key)));
                    }
                }
            }
            want.sort_unstable();
            assert_eq!(
                sorted_pairs(tree.pairs(&mut meter()).unwrap()),
                want,
                "{name}, n = {n}"
            );
            // After a metered walk of every pair: `depth` follows the child links with no meter,
            // so a tree whose links loop would hang it, and that walk stops such a tree first.
            assert!(tree.depth().unwrap() <= MAX_DEPTH, "{name}, n = {n}");
            // Queries: a box at each shape's place, one around everything, one holding nothing.
            let queries = [
                b([4, 4, 4], [4, 4, 4]),
                b([0, 0, 0], [29, 29, 7]),
                b(
                    [-COORD_MAX, -COORD_MAX, -COORD_MAX],
                    [COORD_MAX, COORD_MAX, COORD_MAX],
                ),
                b([100, 100, 100], [101, 101, 101]),
                b(
                    [COORD_MAX, COORD_MAX, COORD_MAX],
                    [COORD_MAX, COORD_MAX, COORD_MAX],
                ),
            ];
            for q in &queries {
                let mut keys: Vec<u64> = entries
                    .iter()
                    .filter(|e| e.bbox.overlaps(q))
                    .map(|e| e.key)
                    .collect();
                keys.sort_unstable();
                let mut got = tree.overlapping(q, &mut meter()).unwrap();
                got.sort_unstable();
                assert_eq!(got, keys, "{name}, n = {n}, query {:?}", (q.lo(), q.hi()));
            }
        }
    }
}
