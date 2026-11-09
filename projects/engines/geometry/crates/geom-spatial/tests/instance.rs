//! Tests of the instance tree (piece G295) through its public interface, on chapter 19.12's one
//! instance record, `geom_topo::placed::Placed` (piece G145), wrapped as the facade wraps it.
//!
//! What this file proves: definitions are cached by content hash and edition and made once; one
//! level's answer is brute force's, ascending, and the tree is the same for any pool; a query
//! through nested assemblies, all 48 signed permutations, finer and coarser frames with offsets,
//! and neighbouring lattice frames gives exactly the parts brute force finds by mapping every part
//! of every instance up to the queried frame in exact rational arithmetic, every hit exact; a
//! region that does not fit the fine frame falls back to exactly the coarse frame's comparison,
//! marked not exact; general placements (rational and certified rotations, alone and around a
//! nested assembly) never miss a part their materialisation puts in the region; every refusal
//! names its cause and builds nothing; nesting stops at `MAX_NESTING`; a cancelled or short
//! budget stops a build or a query with no answer; and an instance costs at most 160 bytes with
//! its record.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::range_plus_one,
    clippy::redundant_closure_for_method_calls,
    clippy::assert_is_empty,
    reason = "generators, brute-force oracles and a test pool of scoped threads; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeSet;

use common::{
    Inst, Threads, add_parts, bx, coords, exact, found, frame, grid, int, key, meter, oracle, p3,
    parts, pick, sorted,
};
use geom_bvh::{Aabb, BoxTree, Entry};
use geom_math::{Axis, CrsId, FrameRecord, GridTransform, Rotation, TypedTransform};
use geom_num::grid::{COORD_MAX, GridStep, P3};
use geom_run::{Budget, CancelToken, Exec, Stop, WorkMeter};
use geom_spatial::instance::{
    Assembly, Content, DefinitionKey, Definitions, InstanceError, MAX_NESTING, Path, Record,
    Relation,
};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use geom_topo::placed::{Definition, Placed};

/// The frames a random scene draws from: two coarse, two fine, all on one lattice.
fn frames() -> [FrameRecord; 4] {
    [
        frame(GridStep::Q0, [0, 0, 0]),
        frame(GridStep::Q0, [500, -300, 7]),
        frame(GridStep::Q1, [3, 7, -11]),
        frame(GridStep::Q1, [0, 0, 0]),
    ]
}

/// A random scene: parts definitions in all four frames, nested assemblies in random frames
/// placing them (and earlier assemblies), and a root assembly in the first frame.
fn scene(rng: &mut SplitMix64) -> (Definitions<Inst>, Assembly<Inst>) {
    let fs = frames();
    let mut defs = Definitions::new();
    let mut n = 0u32;
    for _ in 0..2 + pick(rng, 4) {
        n += 1;
        let f = fs[pick(rng, 4)];
        let count = 1 + pick(rng, 25);
        let centre = int(rng, 3000);
        let tree = parts(
            rng,
            count,
            [centre; 3],
            1 << 12,
            1 << 10,
            u64::from(n) << 20,
        );
        add_parts(&mut defs, key(n, 1), f, tree);
    }
    for _ in 0..1 + pick(rng, 3) {
        n += 1;
        let f = fs[pick(rng, 4)];
        let placeable = defs.len() as u32;
        let records: Vec<Inst> = (0..1 + pick(rng, 12))
            .map(|_| {
                let h = pick(rng, placeable as usize) as u32;
                exact(
                    &defs,
                    h,
                    &f,
                    &grid(
                        pick(rng, 48),
                        [int(rng, 1 << 16), int(rng, 1 << 16), int(rng, 1 << 16)],
                    ),
                )
            })
            .collect();
        let a = Assembly::build(f, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
        defs.get_or_insert_with(key(n, 1), |_| Ok(Content::Assembly(a)))
            .unwrap();
    }
    let root_frame = fs[0];
    let records: Vec<Inst> = (0..20 + pick(rng, 200))
        .map(|_| {
            let h = pick(rng, defs.len()) as u32;
            exact(
                &defs,
                h,
                &root_frame,
                &grid(
                    pick(rng, 48),
                    [int(rng, 1 << 22), int(rng, 1 << 22), int(rng, 1 << 22)],
                ),
            )
        })
        .collect();
    let root = Assembly::build(
        root_frame,
        records,
        &defs,
        &[],
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    (defs, root)
}

#[test]
fn definitions_are_cached_by_content_and_edition() {
    let seed = resolve("instance tree cache", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let f = frame(GridStep::Q0, [0; 3]);
    let mut defs: Definitions<Inst> = Definitions::new();
    let mut made = 0;
    let door = parts(&mut rng, 10, [0; 3], 100, 50, 0);
    let mut make = |defs: &mut Definitions<Inst>, k: DefinitionKey| {
        defs.get_or_insert_with(k, |_| {
            made += 1;
            Ok(Content::Parts {
                frame: f,
                tree: door.clone(),
            })
        })
        .unwrap()
    };
    let a = make(&mut defs, key(1, 1));
    let b = make(&mut defs, key(1, 1));
    assert_eq!(
        (a, b),
        (0, 0),
        "the same content and edition is one definition"
    );
    let c = make(&mut defs, key(1, 2));
    let d = make(&mut defs, key(2, 1));
    assert_eq!(
        (c, d),
        (1, 2),
        "another edition or other content is another definition"
    );
    assert_eq!(made, 3, "made once per key, never on a hit");
    assert_eq!(defs.len(), 3);
    assert_eq!(defs.find(&key(1, 2)), Some(1));
    assert_eq!(defs.find(&key(9, 1)), None);
    let def = defs.get(0).unwrap();
    assert_eq!(def.key(), key(1, 1));
    assert_eq!(def.depth(), 0);
    assert_eq!(*def.frame(), f);
    let root = door.nodes()[0].bbox();
    assert_eq!(*def.bound(), bx(root.lo(), root.hi()));
    // A refusal leaves the table as it was.
    let before = defs.clone();
    let empty = BoxTree::build(&[], &Exec::serial(), &mut meter()).unwrap();
    assert_eq!(
        defs.get_or_insert_with(key(3, 1), |_| Ok(Content::Parts {
            frame: f,
            tree: empty
        })),
        Err(InstanceError::Empty)
    );
    assert_eq!(
        defs.get_or_insert_with(key(3, 1), |_| Err(InstanceError::Placement)),
        Err(InstanceError::Placement)
    );
    let beyond = BoxTree::build(
        &[Entry {
            key: 0,
            bbox: Aabb::new([0; 3], [COORD_MAX + 1, 0, 0]).unwrap(),
        }],
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        defs.get_or_insert_with(key(3, 1), |_| Ok(Content::Parts {
            frame: f,
            tree: beyond
        })),
        Err(InstanceError::Num(_))
    ));
    let nothing =
        Assembly::build(f, Vec::new(), &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    assert!(nothing.is_empty());
    assert_eq!(nothing.bound(), Ok(None));
    assert_eq!(
        defs.get_or_insert_with(key(3, 1), |_| Ok(Content::Assembly(nothing))),
        Err(InstanceError::Empty)
    );
    assert_eq!(defs, before);
    assert_eq!(defs.get(3).map(|d| d.key()), None);
}

#[test]
fn one_level_answers_are_brute_force_and_the_tree_is_the_same_for_any_pool() {
    let seed = resolve("instance tree one level", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let f = frame(GridStep::Q0, [0; 3]);
    let mut defs = Definitions::new();
    for n in 0..5 {
        let tree = parts(&mut rng, 3, [0; 3], 1 << 10, 1 << 12, 0);
        add_parts(&mut defs, key(n, 1), f, tree);
    }
    for count in [0usize, 1, 4, 5, 17, 300, 9000] {
        let records: Vec<Inst> = (0..count)
            .map(|_| {
                let h = pick(&mut rng, 5) as u32;
                exact(
                    &defs,
                    h,
                    &f,
                    &grid(
                        pick(&mut rng, 48),
                        [
                            int(&mut rng, 1 << 24),
                            int(&mut rng, 1 << 24),
                            int(&mut rng, 1 << 20),
                        ],
                    ),
                )
            })
            .collect();
        let a = Assembly::build(
            f,
            records.clone(),
            &defs,
            &[],
            &Exec::serial(),
            &mut meter(),
        )
        .unwrap();
        assert_eq!(a.len(), count);
        assert_eq!(a.tree().len(), count);
        for threads in [1, 2, 3, 8] {
            let pool = Threads(threads);
            let b = Assembly::build(
                f,
                records.clone(),
                &defs,
                &[],
                &Exec::with_pool(&pool),
                &mut meter(),
            )
            .unwrap();
            assert_eq!(a.tree(), b.tree(), "{threads} threads built another tree");
        }
        for q in 0..40 {
            let lo = [
                int(&mut rng, 1 << 24),
                int(&mut rng, 1 << 24),
                int(&mut rng, 1 << 20),
            ];
            let s = if q == 0 { 1 << 30 } else { 1 << (8 + q % 16) };
            let region = bx(lo, [lo[0] + s, lo[1] + s, lo[2] + s]);
            let got = a.overlapping(&region, &mut meter()).unwrap();
            let want: Vec<u32> = (0..count as u32)
                .filter(|&i| records[i as usize].bound().overlaps(&region))
                .collect();
            assert_eq!(got, want, "{count} records, region {region:?}");
        }
        if let Some(root) = a.bound().unwrap() {
            let all = a.overlapping(&root, &mut meter()).unwrap();
            assert_eq!(all, (0..count as u32).collect::<Vec<_>>());
        }
    }
}

#[test]
fn nested_queries_through_every_frame_are_brute_force() {
    let seed = resolve("instance tree nested", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut checked = 0usize;
    for _ in 0..30 {
        let (defs, root) = scene(&mut rng);
        let whole = root.bound().unwrap().unwrap();
        let w = coords(whole.min());
        let e = coords(whole.max());
        for q in 0..30 {
            let region = if q == 0 {
                whole
            } else {
                let c: [i64; 3] = std::array::from_fn(|a| {
                    w[a] + pick(&mut rng, (e[a] - w[a] + 1) as usize) as i64
                });
                let s = 1i64 << (pick(&mut rng, 20));
                bx(c, [c[0] + s, c[1] + s, c[2] + s])
            };
            let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
            assert!(sorted(&hits), "hits out of order");
            assert!(
                hits.iter().all(|h| h.exact),
                "an exact path reported inexact"
            );
            let want = oracle(&root, &defs, &region);
            assert_eq!(found(&hits), want, "region {region:?}");
            assert_eq!(hits.len(), want.len(), "a hit reported twice");
            for h in &hits {
                assert!(h.path.records().len() <= MAX_NESTING);
                let mut a = &root;
                let mut def = None;
                for &r in h.path.records() {
                    let d = defs.get(a.records()[r as usize].definition()).unwrap();
                    def = Some(a.records()[r as usize].definition());
                    if let Content::Assembly(inner) = d.content() {
                        a = inner;
                    }
                }
                assert_eq!(def, Some(h.definition), "the hit names another definition");
            }
            checked += want.len();
        }
    }
    assert!(checked > 1000, "only {checked} hits checked");
}

#[test]
fn every_permutation_enters_exactly() {
    let seed = resolve("instance tree permutations", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    for (asm, def) in [(0, 0), (0, 2), (2, 0), (1, 3)] {
        let fs = frames();
        let mut defs = Definitions::new();
        let tree = parts(&mut rng, 40, [1000, -2000, 500], 1 << 11, 1 << 9, 5);
        add_parts(&mut defs, key(1, 1), fs[def], tree);
        let records: Vec<Inst> = (0..48)
            .map(|p| {
                exact(
                    &defs,
                    0,
                    &fs[asm],
                    &grid(p, [(p as i64) << 20, -((p as i64) << 19), 77]),
                )
            })
            .collect();
        let root =
            Assembly::build(fs[asm], records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
        for p in 0..48u32 {
            let b = *root.records()[p as usize].bound();
            let (lo, hi) = (coords(b.min()), coords(b.max()));
            for _ in 0..20 {
                let c: [i64; 3] = std::array::from_fn(|a| {
                    lo[a] + pick(&mut rng, (hi[a] - lo[a] + 1) as usize) as i64
                });
                let s = 1i64 << pick(&mut rng, 10);
                let region = bx(c, [c[0] + s, c[1] + s, c[2] + s]);
                let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
                assert_eq!(
                    found(&hits),
                    oracle(&root, &defs, &region),
                    "permutation {p}"
                );
                assert!(hits.iter().all(|h| h.exact));
            }
        }
    }
}

#[test]
fn a_region_too_wide_for_the_fine_frame_is_compared_in_the_coarse_one() {
    // A fine definition reaching the end of its frame's range, placed by the identity in a coarse
    // assembly whose frame is one fine step off: the instance's coarse box, moved back into the
    // fine frame, passes that range, so the comparison falls back to the coarse frame (19.12).
    let coarse = frame(GridStep::Q0, [0; 3]);
    let fine = frame(GridStep::Q1, [1, 0, 0]);
    let mut defs = Definitions::new();
    let edge = COORD_MAX;
    let entries: Vec<Entry<3>> = (0..60)
        .map(|i| {
            let x = edge - 40 * i;
            Entry {
                key: i as u64,
                bbox: Aabb::new([x - 3, 0, 0], [x, 5, 5]).unwrap(),
            }
        })
        .collect();
    let tree = BoxTree::build(&entries, &Exec::serial(), &mut meter()).unwrap();
    add_parts(&mut defs, key(1, 1), fine, tree);
    let records = vec![exact(&defs, 0, &coarse, &GridTransform::IDENTITY)];
    let root = Assembly::build(coarse, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    assert!(matches!(
        Relation::between(&coarse, &fine).unwrap(),
        Relation::Finer(s) if s.refine() == 25 && s.offset() == [-1, 0, 0]
    ));
    let whole = *root.records()[0].bound();
    let lo = coords(whole.min());
    let hi = coords(whole.max());
    let mut fallbacks = 0;
    for x in lo[0]..=hi[0] {
        let region = bx([x, 0, 0], [x, 0, 0]);
        let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
        // The coarse comparison: each fine part rounded outward to coarse steps.
        let coarse_want: BTreeSet<(Vec<u32>, u64)> = entries
            .iter()
            .filter(|e| {
                let a = (e.bbox.lo()[0] + 1).div_euclid(25);
                let b = -(-(e.bbox.hi()[0] + 1)).div_euclid(25);
                a <= x && x <= b
            })
            .map(|e| (vec![0], e.key))
            .collect();
        let exact_want = oracle(&root, &defs, &region);
        let got = found(&hits);
        assert!(got.is_superset(&exact_want), "a miss at x = {x}");
        if hits.iter().any(|h| !h.exact) {
            fallbacks += 1;
            assert_eq!(
                got, coarse_want,
                "the fallback at x = {x} is not the coarse comparison"
            );
            assert!(hits.iter().all(|h| !h.exact));
        } else {
            assert_eq!(got, exact_want, "x = {x}");
        }
    }
    assert!(fallbacks > 0, "no region fell back to the coarse frame");
}

#[test]
fn a_coarse_definition_in_a_fine_assembly_and_lattice_neighbours_are_exact() {
    // Coarse parts in a fine assembly: a region between two coarse grid points finds nothing.
    let fine = frame(GridStep::Q1, [0; 3]);
    let coarse = frame(GridStep::Q0, [0; 3]);
    let mut defs = Definitions::new();
    let tree = BoxTree::build(
        &[
            Entry {
                key: 9,
                bbox: Aabb::new([0; 3], [0; 3]).unwrap(),
            },
            Entry {
                key: 10,
                bbox: Aabb::new([1, 0, 0], [1, 0, 0]).unwrap(),
            },
        ],
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    add_parts(&mut defs, key(1, 1), coarse, tree);
    assert!(
        matches!(Relation::between(&fine, &coarse).unwrap(), Relation::Coarser(s) if s.refine() == 25)
    );
    assert_eq!(defs.bound_in(0, &fine).unwrap(), bx([0; 3], [25, 0, 0]));
    let records = vec![
        exact(&defs, 0, &fine, &GridTransform::IDENTITY),
        exact(&defs, 0, &fine, &grid(0, [100, 0, 0])),
    ];
    let root = Assembly::build(fine, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    let ask = |lo: [i64; 3], hi: [i64; 3]| {
        found(&root.query(&defs, &[], &bx(lo, hi), &mut meter()).unwrap())
    };
    assert_eq!(ask([0; 3], [0; 3]), BTreeSet::from([(vec![0], 9)]));
    assert_eq!(
        ask([90, -5, -5], [100, 5, 5]),
        BTreeSet::from([(vec![1], 9)])
    );
    // Between the two coarse points, 25 fine steps apart: nothing.
    assert!(ask([1, -5, -5], [24, 5, 5]).is_empty());
    assert!(ask([101, -5, -5], [124, 5, 5]).is_empty());
    assert_eq!(
        ask([101, -5, -5], [125, 5, 5]),
        BTreeSet::from([(vec![1], 10)])
    );
    for x in -30..160 {
        let region = bx([x, 0, 0], [x + 3, 0, 0]);
        let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
        assert_eq!(found(&hits), oracle(&root, &defs, &region), "x = {x}");
        assert!(hits.iter().all(|h| h.exact));
    }

    // Neighbouring cells of a regional lattice: an exact shift of 2^39 steps.
    let base = [0, 0, 0];
    let here = FrameRecord::lattice(CrsId::Epsg(2193), GridStep::Q0, base, [0, 0]).unwrap();
    let east = FrameRecord::lattice(CrsId::Epsg(2193), GridStep::Q0, base, [1, 0]).unwrap();
    let mut defs = Definitions::new();
    let seed = resolve("instance tree lattice", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let tree = parts(&mut rng, 30, [-(1 << 20), 0, 0], 1 << 12, 1 << 10, 0);
    add_parts(&mut defs, key(1, 1), east, tree);
    let image = defs.bound_in(0, &here).unwrap();
    assert_eq!(
        coords(image.min())[0],
        coords(defs.get(0).unwrap().bound().min())[0] + (1 << 39)
    );
    let records = vec![exact(&defs, 0, &here, &GridTransform::IDENTITY)];
    let root = Assembly::build(here, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    let lo = coords(image.min());
    for region in [
        image,
        bx([lo[0], -COORD_MAX, -COORD_MAX], [COORD_MAX; 3]),
        bx([-COORD_MAX; 3], [COORD_MAX; 3]),
    ] {
        let hits = root.query(&defs, &[], &region, &mut meter()).unwrap();
        assert_eq!(found(&hits), oracle(&root, &defs, &region));
        assert!(hits.iter().all(|h| h.exact));
        assert_eq!(hits.len(), 30);
    }
}

/// Where materialisation puts a vertex of a definition placed by `t`: moved, rounded once to the
/// nearest step.
fn materialised(t: &TypedTransform, v: [i64; 3]) -> P3 {
    t.apply(p3(v)).unwrap().value()
}

#[test]
fn general_placements_never_miss() {
    // The parts are vertices (boxes of one point each), so a part's materialisation is one
    // rounded point, and a hit is owed exactly when that point lies in the region.
    let seed = resolve("instance tree general", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let f = frame(GridStep::Q0, [0; 3]);
    let mut defs = Definitions::new();
    let tree = parts(&mut rng, 400, [0; 3], 1 << 12, 0, 1);
    let entries = tree.entries().to_vec();
    add_parts(&mut defs, key(1, 1), f, tree);
    let turn = |axis: Axis, c: i32, s: i32, shift: [i64; 3]| {
        TypedTransform::from(GridTransform::translation(shift).unwrap())
            .compose(&TypedTransform::rotation(axis, Rotation::direction(c, s).unwrap()).unwrap())
            .unwrap()
    };
    let side = vec![
        turn(Axis::Z, 3, 4, [1 << 20, 0, 0]),
        turn(Axis::Z, 1, 2, [0, 1 << 20, 0]),
        turn(Axis::X, 5, 12, [-(1 << 20), 0, 5]),
        turn(Axis::Y, 7, -3, [0, -(1 << 20), 9]),
    ];
    assert!(matches!(side[0], TypedTransform::Rational(_)));
    assert!(matches!(side[1], TypedTransform::Certified(_)));
    let shared = defs.bound_in(0, &f).unwrap();
    let records: Vec<Inst> = (0..4u32)
        .map(|i| Inst(Placed::general(Definition(0), i, &side[i as usize], &shared).unwrap()))
        .collect();
    let root = Assembly::build(f, records, &defs, &side, &Exec::serial(), &mut meter()).unwrap();
    let mut seen = 0;
    for (i, t) in side.iter().enumerate() {
        let b = *root.records()[i].bound();
        let (lo, hi) = (coords(b.min()), coords(b.max()));
        for q in 0..60 {
            let c: [i64; 3] = std::array::from_fn(|a| {
                lo[a] + pick(&mut rng, (hi[a] - lo[a] + 1) as usize) as i64
            });
            let s = 1i64 << (4 + q % 10);
            let region = bx(c, [c[0] + s, c[1] + s, c[2] + s]);
            let hits = root.query(&defs, &side, &region, &mut meter()).unwrap();
            assert!(
                hits.iter().all(|h| !h.exact),
                "a hit through a general placement called exact"
            );
            let got = found(&hits);
            for e in &entries {
                if region.contains(materialised(t, e.bbox.lo())) {
                    assert!(
                        got.contains(&(vec![i as u32], e.key)),
                        "missed part {} of record {i}",
                        e.key
                    );
                    seen += 1;
                }
            }
        }
        // Far from every instance: nothing.
        let far = bx([1 << 30; 3], [(1 << 30) + 10; 3]);
        assert!(
            root.query(&defs, &side, &far, &mut meter())
                .unwrap()
                .is_empty()
        );
    }
    assert!(seen > 100, "only {seen} materialised parts checked");

    // A nested assembly (a wing of exact placements) turned by a certified rotation.
    let wing_records: Vec<Inst> = (0..10)
        .map(|k| exact(&defs, 0, &f, &grid(k * 5 % 48, [k as i64 * 20_000, 0, 0])))
        .collect();
    let wing = Assembly::build(
        f,
        wing_records.clone(),
        &defs,
        &side,
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let wing_h = defs
        .get_or_insert_with(key(2, 1), |_| Ok(Content::Assembly(wing)))
        .unwrap();
    let shared = defs.bound_in(wing_h, &f).unwrap();
    let records = vec![Inst(
        Placed::general(Definition(wing_h), 1, &side[1], &shared).unwrap(),
    )];
    let root = Assembly::build(f, records, &defs, &side, &Exec::serial(), &mut meter()).unwrap();
    let b = *root.records()[0].bound();
    let (lo, hi) = (coords(b.min()), coords(b.max()));
    let mut seen = 0;
    for q in 0..200 {
        let c: [i64; 3] =
            std::array::from_fn(|a| lo[a] + pick(&mut rng, (hi[a] - lo[a] + 1) as usize) as i64);
        let s = 1i64 << (6 + q % 10);
        let region = bx(c, [c[0] + s, c[1] + s, c[2] + s]);
        let got = found(&root.query(&defs, &side, &region, &mut meter()).unwrap());
        for (k, rec) in wing_records.iter().enumerate() {
            let g = rec.0.placement().grid().unwrap();
            for e in &entries {
                // The wing's exact placement first, then the turn, rounded once.
                let inner = coords(g.apply(p3(e.bbox.lo())).unwrap());
                if region.contains(materialised(&side[1], inner)) {
                    assert!(
                        got.contains(&(vec![0, k as u32], e.key)),
                        "missed a part of wing record {k}"
                    );
                    seen += 1;
                }
            }
        }
    }
    assert!(seen > 20, "only {seen} nested materialised parts checked");
}

#[test]
fn refusals_name_their_cause_and_build_nothing() {
    let seed = resolve("instance tree refusals", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let f = frame(GridStep::Q0, [0; 3]);
    let mut defs = Definitions::new();
    add_parts(
        &mut defs,
        key(1, 1),
        f,
        parts(&mut rng, 5, [0; 3], 100, 50, 0),
    );
    let good = exact(&defs, 0, &f, &grid(3, [10, 20, 30]));
    let build = |records: Vec<Inst>, defs: &Definitions<Inst>, side: &[TypedTransform]| {
        Assembly::build(f, records, defs, side, &Exec::serial(), &mut meter())
    };
    // A definition that is not there.
    let stranger =
        Inst(Placed::exact(Definition(7), &GridTransform::IDENTITY, good.bound()).unwrap());
    assert_eq!(
        build(vec![good, stranger], &defs, &[]).unwrap_err(),
        InstanceError::NoDefinition(7)
    );
    // A side-table entry that is not there.
    let rot = TypedTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
    let general =
        Inst(Placed::general(Definition(0), 5, &rot, &defs.bound_in(0, &f).unwrap()).unwrap());
    assert_eq!(
        build(vec![general], &defs, &[rot]).unwrap_err(),
        InstanceError::NoTransform(5)
    );
    // A box that does not hold the definition placed.
    let small =
        Inst(Placed::exact(Definition(0), &GridTransform::IDENTITY, &bx([0; 3], [1; 3])).unwrap());
    assert_eq!(
        build(vec![good, good, small], &defs, &[]).unwrap_err(),
        InstanceError::Unbounded(2)
    );
    // A general record whose box is the exact image of a box too small.
    let tight = Inst(Placed::general(Definition(0), 0, &rot, &bx([0; 3], [1; 3])).unwrap());
    assert_eq!(
        build(vec![tight], &defs, &[rot]).unwrap_err(),
        InstanceError::Unbounded(0)
    );
    // Frames with no exact shift: another coordinate reference system.
    let other = FrameRecord::map(CrsId::Local(2), GridStep::Q0, [0; 3]).unwrap();
    add_parts(
        &mut defs,
        key(2, 1),
        other,
        parts(&mut rng, 5, [0; 3], 100, 50, 0),
    );
    let shared = *defs.get(1).unwrap().bound();
    let foreign = Inst(Placed::exact(Definition(1), &GridTransform::IDENTITY, &shared).unwrap());
    assert!(matches!(
        build(vec![foreign], &defs, &[]).unwrap_err(),
        InstanceError::Frame(_)
    ));
    assert!(matches!(defs.bound_in(1, &f), Err(InstanceError::Frame(_))));
    assert_eq!(defs.bound_in(9, &f), Err(InstanceError::NoDefinition(9)));
    // A query given another table: the handle now names another definition.
    let root = build(vec![good], &defs, &[]).unwrap();
    let mut elsewhere = Definitions::new();
    add_parts(
        &mut elsewhere,
        key(5, 1),
        f,
        parts(&mut rng, 5, [0; 3], 100, 50, 0),
    );
    let region = root.bound().unwrap().unwrap();
    assert_eq!(
        root.query(&elsewhere, &[], &region, &mut meter()),
        Err(InstanceError::Stale(0))
    );
    // An assembly made against another table, cached here.
    let alien = Assembly::build(
        f,
        vec![exact(&elsewhere, 0, &f, &GridTransform::IDENTITY)],
        &elsewhere,
        &[],
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let before = defs.clone();
    assert_eq!(
        defs.get_or_insert_with(key(6, 1), |_| Ok(Content::Assembly(alien))),
        Err(InstanceError::Stale(0))
    );
    let mut longer = elsewhere.clone();
    add_parts(
        &mut longer,
        key(7, 1),
        f,
        parts(&mut rng, 5, [0; 3], 100, 50, 0),
    );
    let far = Assembly::build(
        f,
        vec![exact(&longer, 1, &f, &GridTransform::IDENTITY)],
        &longer,
        &[],
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let mut short = Definitions::new();
    add_parts(
        &mut short,
        key(7, 1),
        f,
        parts(&mut rng, 5, [0; 3], 100, 50, 0),
    );
    assert_eq!(
        short.get_or_insert_with(key(8, 1), |_| Ok(Content::Assembly(far))),
        Err(InstanceError::NoDefinition(1))
    );
    assert_eq!(defs, before);
}

#[test]
fn nesting_stops_at_its_bound() {
    let f = frame(GridStep::Q0, [0; 3]);
    let mut defs = Definitions::new();
    let tree = BoxTree::build(
        &[Entry {
            key: 4,
            bbox: Aabb::new([0; 3], [1; 3]).unwrap(),
        }],
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let mut h = add_parts(&mut defs, key(0, 1), f, tree);
    for level in 1..=MAX_NESTING {
        let records = vec![exact(&defs, h, &f, &grid(level % 48, [1, 0, 0]))];
        let a = Assembly::build(f, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
        h = defs
            .get_or_insert_with(key(level as u32, 1), |_| Ok(Content::Assembly(a)))
            .unwrap();
        assert_eq!(usize::from(defs.get(h).unwrap().depth()), level);
    }
    // The deepest assembly allowed answers through every level, on an explicit stack.
    let Content::Assembly(deepest) = defs.get(h).unwrap().content() else {
        panic!()
    };
    let region = deepest.bound().unwrap().unwrap();
    let hits = deepest.query(&defs, &[], &region, &mut meter()).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path.records(), &[0; MAX_NESTING][..]);
    assert_eq!(hits[0].part, 4);
    assert_eq!(found(&hits), oracle(deepest, &defs, &region));
    // One more level is refused, built or cached.
    let records = vec![exact(&defs, h, &f, &GridTransform::IDENTITY)];
    assert_eq!(
        Assembly::build(f, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap_err(),
        InstanceError::TooDeep
    );
    // A path holds at most MAX_NESTING records, ordered record by record.
    let mut p = Path::EMPTY;
    for i in 0..MAX_NESTING as u32 {
        p = p.then(i).unwrap();
    }
    assert_eq!(p.then(0), Err(InstanceError::TooDeep));
    let a = Path::EMPTY.then(1).unwrap();
    let b = a.then(0).unwrap();
    let c = Path::EMPTY.then(2).unwrap();
    assert!(Path::EMPTY < a && a < b && b < c);
    assert_eq!(a, Path::EMPTY.then(1).unwrap());
}

#[test]
fn cancelled_or_short_budgets_stop_with_no_answer() {
    let seed = resolve("instance tree stops", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let (defs, root) = scene(&mut rng);
    let region = root.bound().unwrap().unwrap();
    let token = CancelToken::new();
    token.cancel();
    let mut cancelled = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        root.query(&defs, &[], &region, &mut cancelled),
        Err(InstanceError::Stopped(Stop::Cancelled))
    );
    let records = root.records().to_vec();
    let mut cancelled = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        Assembly::build(
            *root.frame(),
            records.clone(),
            &defs,
            &[],
            &Exec::serial(),
            &mut cancelled
        ),
        Err(InstanceError::Stopped(Stop::Cancelled))
    );
    let mut full = meter();
    root.query(&defs, &[], &region, &mut full).unwrap();
    let used = full.used().work;
    let mut short = WorkMeter::new(Budget::new(u64::try_from(used - 1).unwrap(), u64::MAX));
    assert!(matches!(
        root.query(&defs, &[], &region, &mut short),
        Err(InstanceError::Stopped(Stop::OverBudget(_)))
    ));
    let mut tiny = WorkMeter::new(Budget::new(u64::MAX, 8));
    assert!(matches!(
        Assembly::build(
            *root.frame(),
            records,
            &defs,
            &[],
            &Exec::serial(),
            &mut tiny
        ),
        Err(InstanceError::Stopped(Stop::OverBudget(_)))
    ));
}

#[test]
fn an_instance_costs_at_most_160_bytes_with_its_record() {
    let seed = resolve("instance tree bytes", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    assert!(
        size_of::<Placed>() <= 88,
        "chapter 19.12's record is at most 88 bytes"
    );
    assert_eq!(
        size_of::<Inst>(),
        size_of::<Placed>(),
        "the wrapper adds nothing"
    );
    let f = frame(GridStep::Q0, [0; 3]);
    let mut defs = Definitions::new();
    for n in 0..20 {
        add_parts(
            &mut defs,
            key(n, 1),
            f,
            parts(&mut rng, 4, [0; 3], 1 << 10, 1 << 9, 0),
        );
    }
    let count = 50_000;
    let records: Vec<Inst> = (0..count)
        .map(|_| {
            let h = pick(&mut rng, 20) as u32;
            exact(
                &defs,
                h,
                &f,
                &grid(
                    pick(&mut rng, 48),
                    [
                        int(&mut rng, 1 << 30),
                        int(&mut rng, 1 << 30),
                        int(&mut rng, 1 << 26),
                    ],
                ),
            )
        })
        .collect();
    let a = Assembly::build(f, records, &defs, &[], &Exec::serial(), &mut meter()).unwrap();
    let per = a.heap_bytes() as f64 / count as f64;
    let tree = a.tree().heap_bytes() as f64 / count as f64;
    println!(
        "instance tree: {per:.1} bytes an instance with its {}-byte record ({tree:.1} the tree's: {} nodes for {count} records)",
        size_of::<Inst>(),
        a.tree().nodes().len()
    );
    assert!(per <= 160.0, "{per:.1} bytes an instance");
    assert_eq!(a.tree().leaves().len(), count);
}

#[test]
fn an_assembly_with_records_is_not_empty_and_paths_that_differ_are_unequal() {
    let seed = resolve("instance tree emptiness", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let (_, root) = scene(&mut rng);
    assert!(root.len() >= 20);
    assert!(!root.is_empty());
    let a = Path::EMPTY.then(1).unwrap();
    assert_ne!(a, Path::EMPTY);
    assert_ne!(a, Path::EMPTY.then(2).unwrap());
    assert_ne!(a, a.then(1).unwrap());
    assert_eq!(a, Path::EMPTY.then(1).unwrap());
}
