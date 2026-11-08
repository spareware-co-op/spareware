//! Attacks on piece G295's claims: the instance tree at the grid's extremes, at the ends of a fine
//! frame's range on both sides, on touching boxes, repeated and reversed records, coarse parts in
//! fine frames below zero, and the enclosures of general placements, each against an oracle that
//! does not share the code under test.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::range_plus_one,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    reason = "generators and brute-force oracles; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::{
    Inst, add_parts, bx, coords, exact, found, frame, grid, int, key, meter, oracle, p3, pick,
};
use geom_bvh::{Aabb, BoxTree, Entry};
use geom_math::{Axis, GridTransform, Rotation, TypedTransform};
use geom_num::grid::{COORD_MAX, GridStep};
use geom_run::Exec;
use geom_spatial::instance::enter::{backward, forward};
use geom_spatial::instance::{
    Assembly, Definitions, InstanceError, InstanceTree, Region, Relation,
};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn tree(boxes: &[([i64; 3], [i64; 3])]) -> BoxTree<3> {
    let entries: Vec<Entry<3>> = boxes
        .iter()
        .enumerate()
        .map(|(i, (lo, hi))| Entry {
            key: 100 + i as u64,
            bbox: Aabb::new(*lo, *hi).unwrap(),
        })
        .collect();
    BoxTree::build(&entries, &Exec::serial(), &mut meter()).unwrap()
}

#[test]
fn the_grids_extremes_are_answered_exactly() {
    let m = COORD_MAX;
    let coarse = frame(GridStep::Q0, [0; 3]);
    let fine = frame(GridStep::Q1, [0; 3]);
    // Parts at every corner of the grid, and one through its middle.
    let mut corners = Vec::new();
    for x in [-m, m] {
        for y in [-m, m] {
            for z in [-m, m] {
                corners.push(([x, y, z], [x, y, z]));
            }
        }
    }
    corners.push(([-m, 0, 0], [m, 0, 0]));
    let mut defs = Definitions::new();
    add_parts(&mut defs, key(1, 1), coarse, tree(&corners));
    add_parts(&mut defs, key(2, 1), fine, tree(&corners));
    // Every one of the 48 permutations sends the whole grid onto itself.
    let records: Vec<Inst> = (0..96)
        .map(|i| exact(&defs, (i / 48) as u32, &coarse, &grid(i % 48, [0; 3])))
        .collect();
    let root = Assembly::build(coarse, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    let corner = |s: [i64; 3]| bx(s.map(|v| v * m), s.map(|v| v * m));
    let mut regions = vec![
        bx([-m; 3], [m; 3]),
        bx([0; 3], [0; 3]),
        bx([m, 0, 0], [m, 0, 0]),
    ];
    for x in [-1, 1] {
        for y in [-1, 1] {
            for z in [-1, 1] {
                regions.push(corner([x, y, z]));
            }
        }
    }
    for region in regions {
        let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
        assert_eq!(
            found(&hits),
            oracle(&root, &defs, &region),
            "region {region:?}"
        );
        assert!(hits.iter().all(|h| h.exact));
    }
}

#[test]
fn both_ends_of_a_fine_frame_fall_back_and_never_miss() {
    let m = COORD_MAX;
    let coarse = frame(GridStep::Q0, [0; 3]);
    // Origins one step off on two axes: neither end of the fine range is a coarse step.
    let fine = frame(GridStep::Q1, [1, -1, 0]);
    let parts: Vec<([i64; 3], [i64; 3])> = (0..40)
        .flat_map(|i| {
            let d = 30 * i;
            [
                ([m - d - 2, -m + d, 0], [m - d, -m + d + 2, 3]),
                ([-m + d, m - d - 2, 0], [-m + d + 2, m - d, 3]),
            ]
        })
        .collect();
    let mut defs = Definitions::new();
    add_parts(&mut defs, key(1, 1), fine, tree(&parts));
    let records: Vec<Inst> = [0usize, 1, 2, 3, 6, 7, 40, 47]
        .into_iter()
        .map(|p| exact(&defs, 0, &coarse, &grid(p, [0; 3])))
        .collect();
    let root = Assembly::build(coarse, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    let mut fell_back = 0;
    for (i, rec) in root.records().iter().enumerate() {
        let b = *rec.0.bound();
        let (lo, hi) = (coords(b.min()), coords(b.max()));
        for x in [lo[0], lo[0] + 1, lo[0] + 2, hi[0] - 2, hi[0] - 1, hi[0]] {
            for y in [lo[1], lo[1] + 1, hi[1] - 1, hi[1]] {
                let region = bx([x, y, lo[2]], [x, y, hi[2]]);
                let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
                let got = found(&hits);
                let want = oracle(&root, &defs, &region);
                assert!(got.is_superset(&want), "record {i}: missed at {region:?}");
                if hits.iter().all(|h| h.exact) {
                    assert_eq!(
                        got, want,
                        "record {i}: an exact answer that is not brute force's"
                    );
                } else {
                    fell_back += 1;
                }
            }
        }
    }
    assert!(fell_back > 0, "no query fell back to the coarse frame");
}

#[test]
fn touching_boxes_meet_and_one_step_apart_do_not() {
    let seed = resolve("instance tree touching", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let fs = [
        frame(GridStep::Q0, [0; 3]),
        frame(GridStep::Q1, [7, 0, -3]),
        frame(GridStep::Q0, [2, 2, 2]),
    ];
    for _ in 0..20 {
        let mut defs = Definitions::new();
        let boxes: Vec<([i64; 3], [i64; 3])> = (0..8)
            .map(|_| {
                let lo: [i64; 3] = std::array::from_fn(|_| int(&mut rng, 5000));
                (lo, lo.map(|v| v + pick(&mut rng, 300) as i64))
            })
            .collect();
        let d = fs[pick(&mut rng, 3)];
        add_parts(&mut defs, key(1, 1), d, tree(&boxes));
        let a = fs[pick(&mut rng, 3)];
        let records: Vec<Inst> = (0..6)
            .map(|_| {
                exact(
                    &defs,
                    0,
                    &a,
                    &grid(
                        pick(&mut rng, 48),
                        [
                            int(&mut rng, 9000),
                            int(&mut rng, 9000),
                            int(&mut rng, 9000),
                        ],
                    ),
                )
            })
            .collect();
        let root = Assembly::build(a, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
        // Regions touching each record's box at a face, an edge and a corner, and one step off.
        for rec in root.records() {
            let b = *rec.0.bound();
            let (lo, hi) = (coords(b.min()), coords(b.max()));
            for (off, size) in [(0i64, 0i64), (1, 0), (0, 5), (1, 5)] {
                for region in [
                    bx(
                        [hi[0] + off, lo[1], lo[2]],
                        [hi[0] + off + size, hi[1], hi[2]],
                    ),
                    bx(
                        [lo[0] - off - size, lo[1] - off - size, lo[2]],
                        [lo[0] - off, lo[1] - off, hi[2]],
                    ),
                    bx(
                        [hi[0] + off, hi[1] + off, lo[2]],
                        [hi[0] + off + size, hi[1] + off + size, hi[2]],
                    ),
                    bx(hi.map(|v| v + off), hi.map(|v| v + off + size)),
                ] {
                    let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
                    assert_eq!(
                        found(&hits),
                        oracle(&root, &defs, &region),
                        "region {region:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn repeated_and_reversed_records_are_each_answered() {
    let seed = resolve("instance tree repeated", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let f = frame(GridStep::Q0, [0; 3]);
    let mut defs = Definitions::new();
    let boxes: Vec<([i64; 3], [i64; 3])> = (0..10)
        .map(|_| {
            let lo: [i64; 3] = std::array::from_fn(|_| int(&mut rng, 1000));
            (lo, lo.map(|v| v + 50))
        })
        .collect();
    add_parts(&mut defs, key(1, 1), f, tree(&boxes));
    let one = exact(&defs, 0, &f, &grid(13, [5, 6, 7]));
    let records = vec![one; 9];
    let root = Assembly::build(f, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    let region = root.bound().unwrap().unwrap();
    assert_eq!(
        root.overlapping(&region, &mut meter()).unwrap(),
        (0..9).collect::<Vec<u32>>()
    );
    let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
    assert_eq!(hits.len(), 90);
    assert_eq!(found(&hits), oracle(&root, &defs, &region));

    let forward_records: Vec<Inst> = (0..200)
        .map(|_| {
            exact(
                &defs,
                0,
                &f,
                &grid(
                    pick(&mut rng, 48),
                    [int(&mut rng, 1 << 16), int(&mut rng, 1 << 16), 0],
                ),
            )
        })
        .collect();
    let mut reversed = forward_records.clone();
    reversed.reverse();
    let a = Assembly::build(
        f,
        forward_records,
        &defs,
        &[],
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let b = Assembly::build(f, reversed, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    for _ in 0..50 {
        let c: [i64; 3] = std::array::from_fn(|_| int(&mut rng, 1 << 16));
        let region = bx(c, c.map(|v| v + 4000));
        let mut flipped: Vec<u32> = b
            .overlapping(&region, &mut meter())
            .unwrap()
            .into_iter()
            .map(|i| 199 - i)
            .collect();
        flipped.sort_unstable();
        assert_eq!(a.overlapping(&region, &mut meter()).unwrap(), flipped);
    }
    // A record slice that is not the tree's is refused.
    let t = InstanceTree::build(a.records(), &Exec::serial(), &mut meter()).unwrap();
    assert_eq!(t, *a.tree());
    assert!(matches!(
        t.overlapping(&a.records()[..199], &region, &mut meter()),
        Err(InstanceError::Stale(_))
    ));
}

#[test]
fn coarse_parts_in_a_fine_frame_below_zero_round_inward() {
    // A fine assembly whose origin is not on a coarse step, so the shift's offset is negative and
    // odd, and every region crosses zero.
    let fine = frame(GridStep::Q1, [13, -4, 0]);
    let coarse = frame(GridStep::Q0, [0; 3]);
    let boxes: Vec<([i64; 3], [i64; 3])> = (-6..6).map(|i| ([i, 0, 0], [i, 0, 0])).collect();
    let mut defs = Definitions::new();
    add_parts(&mut defs, key(1, 1), coarse, tree(&boxes));
    let relation = Relation::between(&fine, &coarse).unwrap();
    assert!(matches!(relation, Relation::Coarser(s) if s.offset() == [-13, 4, 0]));
    let records = vec![
        exact(&defs, 0, &fine, &GridTransform::IDENTITY),
        exact(&defs, 0, &fine, &grid(1, [-31, 0, 0])),
    ];
    let root = Assembly::build(fine, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    for x in -200..200 {
        for w in [0, 1, 24, 25, 26] {
            let region = bx([x, 4, 0], [x + w, 4, 0]);
            let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
            assert_eq!(
                found(&hits),
                oracle(&root, &defs, &region),
                "x = {x}, width {w}"
            );
        }
    }
}

#[test]
fn outward_boxes_are_the_tightest_that_hold() {
    let seed = resolve("instance tree outward", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let coarse = frame(GridStep::Q0, [0; 3]);
    for origin in [[0, 0, 0], [1, -1, 24], [-25, 26, 50]] {
        let fine = frame(GridStep::Q1, origin);
        let relation = Relation::between(&coarse, &fine).unwrap();
        let Relation::Finer(s) = relation else {
            panic!("a fine frame is finer")
        };
        let (r, o) = (s.refine(), s.offset());
        for _ in 0..500 {
            let lo: [i64; 3] = std::array::from_fn(|_| int(&mut rng, 1 << 30));
            let hi: [i64; 3] = std::array::from_fn(|a| lo[a] + pick(&mut rng, 100) as i64);
            let b = bx(lo, hi);
            let out = relation.outward(&b).unwrap();
            let (ol, oh) = (coords(out.min()), coords(out.max()));
            for a in 0..3 {
                // The coarse box's fine image holds the fine box, and is the smallest that does.
                assert!(r * ol[a] + o[a] <= lo[a] && lo[a] < r * (ol[a] + 1) + o[a]);
                assert!(r * oh[a] + o[a] >= hi[a] && hi[a] > r * (oh[a] - 1) + o[a]);
            }
            // Entering the outward box gives back a box holding the fine one.
            let (back, exact) = relation.enter(&Region::from_box(&out)).unwrap().unwrap();
            assert!(exact);
            assert_eq!(back.den(), 1);
            for a in 0..3 {
                assert!(back.lo()[a] <= i128::from(lo[a]) && i128::from(hi[a]) <= back.hi()[a]);
            }
        }
    }
}

#[test]
fn general_enclosures_hold_every_point_they_owe() {
    let seed = resolve("instance tree enclosures", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let directions = [
        (3, 4),
        (1, 2),
        (5, 12),
        (7, -3),
        (-8, 15),
        (1, 1000),
        (999, 1),
    ];
    let axes = [Axis::X, Axis::Y, Axis::Z];
    let mut checked = 0;
    for (k, (c, s)) in directions.into_iter().enumerate() {
        for axis in axes {
            let shift = [
                int(&mut rng, 1 << 30),
                int(&mut rng, 1 << 30),
                int(&mut rng, 1 << 30),
            ];
            let t = TypedTransform::from(GridTransform::translation(shift).unwrap())
                .compose(
                    &TypedTransform::rotation(axis, Rotation::direction(c, s).unwrap()).unwrap(),
                )
                .unwrap();
            let within = bx([-(1 << 20); 3], [1 << 20; 3]);
            let image = forward(&t, &within).unwrap();
            for _ in 0..300 {
                let p: [i64; 3] = std::array::from_fn(|_| int(&mut rng, 1 << 20));
                let q = t.apply(p3(p)).unwrap().value();
                assert!(
                    image.contains(q),
                    "forward missed {p:?} under direction {k}"
                );
                // A region around the rounded image, from a point to a few thousand steps.
                let size = 1i64 << pick(&mut rng, 13);
                let qc = coords(q);
                let lo: [i64; 3] =
                    std::array::from_fn(|a| qc[a] - pick(&mut rng, size as usize + 1) as i64);
                let region = bx(lo, lo.map(|v| v + size));
                let back = backward(&t, &region, &within)
                    .unwrap()
                    .expect("a point is owed");
                assert!(
                    back.contains(p3(p)),
                    "backward missed {p:?} under direction {k}"
                );
                checked += 1;
            }
            // A region no point of `within` comes near gives nothing.
            let far = bx([COORD_MAX - 10; 3], [COORD_MAX; 3]);
            assert_eq!(backward(&t, &far, &within).unwrap(), None);
        }
    }
    assert_eq!(checked, 7 * 3 * 300);
    // A transform with no inverse cannot be entered.
    let flat = TypedTransform::from(
        geom_math::Transform::from_rows([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
        ])
        .unwrap(),
    );
    assert!(matches!(
        backward(&flat, &bx([0; 3], [1; 3]), &bx([0; 3], [9; 3])),
        Err(InstanceError::Transform(_))
    ));
}

#[test]
fn regions_stay_exact_and_small_through_coarse_and_fine_frames() {
    let fine = frame(GridStep::Q1, [7, 0, -3]);
    let coarse = frame(GridStep::Q0, [2, 2, 2]);
    // Fine to coarse: (a − o) / 25 with o = 25·[2, 2, 2] − [7, 0, −3] = [43, 50, 53].
    let down = Relation::between(&fine, &coarse).unwrap();
    let up = Relation::between(&coarse, &fine).unwrap();
    let r = Region::from_box(&bx([95_277, 50, 103], [95_277, 75, 128]));
    let (c, exact) = down.enter(&r).unwrap().unwrap();
    assert!(exact);
    // x: (95277 − 43) / 25 is not whole, so the denominator stays 25; y and z are whole.
    assert_eq!(
        (c.lo(), c.hi(), c.den()),
        ([95_234, 0, 50], [95_234, 25, 75], 25)
    );
    // x is 3809.36 in coarse steps: a box from 3809 to 3810 meets it, neither end alone does.
    assert!(c.meets([3_809, 0, 2], [3_810, 0, 2]));
    assert!(!c.meets([3_809, 0, 2], [3_809, 1, 3]));
    assert!(!c.meets([3_810, 0, 2], [3_810, 0, 2]));
    let o = c.outward().unwrap();
    assert_eq!(
        (coords(o.min()), coords(o.max())),
        ([3_809, 0, 2], [3_810, 1, 3])
    );
    // Back up to the fine frame: the denominator reduces to 1, and the region is r again.
    let (back, exact) = up.enter(&c).unwrap().unwrap();
    assert!(exact);
    assert_eq!(back, r);
    // A region whose every corner is a coarse step reduces to whole steps at once.
    let whole = Region::from_box(&bx([43, 50, 53], [93, 100, 103]));
    let (w, _) = down.enter(&whole).unwrap().unwrap();
    assert_eq!((w.lo(), w.hi(), w.den()), ([0, 0, 0], [2, 2, 2], 1));
    assert_eq!(coords(w.outward().unwrap().max()), [2, 2, 2]);
    // Clipping keeps the denominator and the exact bounds, and refuses a box it misses.
    let clipped = c.clip(&bx([3_800, 0, 0], [3_812, 0, 9])).unwrap().unwrap();
    assert_eq!(
        (clipped.lo(), clipped.hi(), clipped.den()),
        ([95_234, 0, 50], [95_234, 0, 75], 25)
    );
    assert_eq!(c.clip(&bx([3_810, 0, 0], [3_811, 9, 9])).unwrap(), None);
    assert!(clipped.meets_box(&bx([3_808, 0, 2], [3_810, 0, 2])));
    assert!(!clipped.meets_box(&bx([3_808, 0, 3], [3_809, 0, 9])));
}

#[test]
fn errors_say_what_they_are_and_convert_faithfully() {
    use geom_bvh::BvhError;
    use geom_num::NumError;
    use geom_run::{BudgetKind, Stop};
    let all = [
        InstanceError::Stopped(Stop::Cancelled),
        InstanceError::Tree(BvhError::OutOfRange),
        InstanceError::NoDefinition(3),
        InstanceError::NoTransform(4),
        InstanceError::Placement,
        InstanceError::Unbounded(5),
        InstanceError::Empty,
        InstanceError::TooDeep,
        InstanceError::Frame(geom_math::MathError::Singular),
        InstanceError::Transform(geom_math::MathError::Singular),
        InstanceError::Num(NumError::Degenerate),
        InstanceError::TooManyRecords,
        InstanceError::Stale(6),
        InstanceError::OutOfMemory,
        InstanceError::Defect,
    ];
    let texts: Vec<String> = all.iter().map(ToString::to_string).collect();
    for (e, t) in all.iter().zip(&texts) {
        assert!(t.len() > 8, "{e:?} says {t:?}");
    }
    let distinct: std::collections::BTreeSet<&String> = texts.iter().collect();
    assert_eq!(distinct.len(), all.len(), "two kinds say the same");
    assert!(
        texts[2].contains('3')
            && texts[3].contains('4')
            && texts[5].contains('5')
            && texts[12].contains('6')
    );
    assert!(texts[7].contains("16"));
    assert_eq!(
        InstanceError::from(BvhError::Stopped(Stop::OverBudget(BudgetKind::Work))),
        InstanceError::Stopped(Stop::OverBudget(BudgetKind::Work))
    );
    assert_eq!(
        InstanceError::from(BvhError::OutOfMemory),
        InstanceError::OutOfMemory
    );
    assert_eq!(
        InstanceError::from(BvhError::Defect),
        InstanceError::Tree(BvhError::Defect)
    );
    assert_eq!(
        InstanceError::from(NumError::Degenerate),
        InstanceError::Num(NumError::Degenerate)
    );
    let e: &dyn std::error::Error = &all[0];
    assert!(e.source().is_none());
}

#[test]
fn paths_hash_as_they_compare_and_trees_count_their_bytes() {
    use geom_spatial::instance::Path;
    use std::hash::DefaultHasher;
    use std::hash::{BuildHasher, BuildHasherDefault};
    let hash = |p: &Path| BuildHasherDefault::<DefaultHasher>::default().hash_one(p);
    let a = Path::EMPTY.then(4).unwrap().then(9).unwrap();
    let b = Path::EMPTY.then(4).unwrap().then(9).unwrap();
    let c = Path::EMPTY.then(9).unwrap().then(4).unwrap();
    assert_eq!(hash(&a), hash(&b));
    assert_ne!(hash(&a), hash(&c));
    assert_ne!(hash(&a), hash(&Path::EMPTY));
    assert_eq!(a.records(), &[4, 9]);
    assert!(Path::EMPTY.records().is_empty());

    let f = frame(GridStep::Q0, [0; 3]);
    let mut defs = Definitions::new();
    assert!(defs.is_empty());
    add_parts(&mut defs, key(1, 1), f, tree(&[([0; 3], [9; 3])]));
    assert!(!defs.is_empty());
    let records: Vec<Inst> = (0..1000)
        .map(|i| exact(&defs, 0, &f, &grid(i % 48, [i as i64 * 20, 0, 0])))
        .collect();
    let a = Assembly::build(f, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    let t = a.tree();
    assert!(!t.is_empty());
    assert!(
        InstanceTree::build::<Inst>(&[], &Exec::serial(), &mut meter())
            .unwrap()
            .is_empty()
    );
    let least = size_of_val(t.nodes()) + size_of_val(t.leaves());
    assert!(t.heap_bytes() >= least && t.heap_bytes() < 2 * least);
    let records = size_of_val(a.records());
    assert!(a.heap_bytes() >= records + least && a.heap_bytes() < 2 * (records + least));
    assert_eq!(a.placed().collect::<Vec<_>>(), vec![(0, key(1, 1))]);
}
