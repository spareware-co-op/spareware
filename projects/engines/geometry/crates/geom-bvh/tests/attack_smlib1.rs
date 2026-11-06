//! Attack tests from SMLib's confirmed defects, batch 1: `geom-bvh`.
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

use geom_bvh::{
    Aabb, BoxTree, BvhError, CellIndex, Entry, Inflation, LIMIT, MAX_DEPTH, Point, morton,
};
use geom_num::Real;
use geom_num::grid::{COORD_MAX, P3};
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// solidcore plan/upstream-issues/UB-35--mappointtovoxel-converts-negative-doubles-to-ulong.md
/// (b4de2149): a point below the box was converted to an unsigned voxel index, so it wrapped to
/// the top voxel (x86-64) or to the bottom (aarch64); NaN and infinity went to different cells on
/// different machines; a zero-length axis gave NaN or infinity for every point. Ours must put a
/// point below the box below it, on every machine, give a zero-length axis a box of the steps
/// around it, and refuse what is not a number.
#[test]
fn ub_35_points_below_the_range_stay_below_and_non_numbers_are_refused() {
    let exact = Inflation::Exact;
    // NaN and infinity in either corner, on any axis: refused as not finite.
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for axis in 0..3 {
            let mut lo = [0.0; 3];
            let mut hi = [1.0; 3];
            lo[axis] = bad;
            let r = Aabb::<3>::from_float(lo, hi, exact);
            assert!(
                matches!(
                    r,
                    Err(BvhError::BadBound | BvhError::Empty | BvhError::OutOfRange)
                ),
                "{bad} low on axis {axis}: {r:?}"
            );
            lo[axis] = 0.0;
            hi[axis] = bad;
            let r = Aabb::<3>::from_float(lo, hi, exact);
            assert!(
                matches!(
                    r,
                    Err(BvhError::BadBound | BvhError::Empty | BvhError::OutOfRange)
                ),
                "{bad} high on axis {axis}: {r:?}"
            );
        }
    }
    // Far below and far above the range: refused with the range error, not wrapped into it.
    let limit = LIMIT as f64;
    assert_eq!(
        Aabb::<3>::from_float([-2.0 * limit, 0.0, 0.0], [0.0; 3], exact),
        Err(BvhError::OutOfRange)
    );
    assert_eq!(
        Aabb::<3>::from_float([0.0; 3], [0.0, 2.0 * limit, 0.0], exact),
        Err(BvhError::OutOfRange)
    );
    // A box reversed on an axis is empty, not an inverted box.
    assert_eq!(
        Aabb::<2>::from_float([1.0, 0.0], [0.0, 1.0], exact),
        Err(BvhError::Empty)
    );
    // A zero-length axis: a point on the grid is a box of one point; one between steps is the
    // steps around it. Below zero rounds down at the bottom and up at the top.
    let b = Aabb::<3>::from_float([3.0, -4.0, 5.0], [3.0, -4.0, 5.0], exact).unwrap();
    assert_eq!((b.lo(), b.hi()), ([3, -4, 5], [3, -4, 5]));
    let b = Aabb::<3>::from_float([-0.25, -1.5, 0.5], [-0.25, -1.5, 0.5], exact).unwrap();
    assert_eq!((b.lo(), b.hi()), ([-1, -2, 0], [0, -1, 1]));
    // The range's own ends are accepted.
    let b = Aabb::<2>::from_float([-limit, -limit], [limit, limit], exact).unwrap();
    assert_eq!((b.lo(), b.hi()), ([-LIMIT, -LIMIT], [LIMIT, LIMIT]));
    // A padded box that leaves the range is refused too, not clamped.
    let pad = Inflation::Tolerant(Real::new(4.0).unwrap());
    assert_eq!(
        Aabb::<2>::from_float([-limit, 0.0], [0.0, 0.0], pad),
        Err(BvhError::OutOfRange)
    );
    // The cell codes: along each axis the code rises with the coordinate, negative to positive,
    // so a point below is below and never in the top cell.
    let xs = [
        -COORD_MAX,
        -COORD_MAX + 1,
        -1,
        0,
        1,
        COORD_MAX - 1,
        COORD_MAX,
    ];
    for axis in 0..3 {
        let codes: Vec<u128> = xs
            .iter()
            .map(|&c| {
                let mut v = [7, -9, 11];
                v[axis] = c;
                morton(p3(v[0], v[1], v[2])).unwrap()
            })
            .collect();
        assert!(
            codes.windows(2).all(|w| w[0] < w[1]),
            "axis {axis}: {codes:?}"
        );
    }
}

/// solidcore plan/upstream-issues/UB-36--findcoincidentpairs-reads-getat-0-of-an-empty-bucket-list.md
/// (b4de2149): the first of an empty bucket list was read. Ours is asked for every search of an
/// index with no points and of an index of one point: each must answer, and the answer for
/// nothing is nothing.
#[test]
fn ub_36_an_empty_cell_index_answers_every_search_with_nothing() {
    let exec = Exec::serial();
    let empty = CellIndex::build(&[], &exec, &mut meter()).unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    for radius in [0, 1, 1 << 40, u64::MAX] {
        assert_eq!(
            empty.within(p3(0, 0, 0), radius, &mut meter()).unwrap(),
            Vec::<u64>::new()
        );
        assert_eq!(
            empty
                .within(p3(COORD_MAX, -COORD_MAX, 0), radius, &mut meter())
                .unwrap(),
            Vec::<u64>::new()
        );
        assert_eq!(
            empty.pairs_within(radius, &exec, &mut meter()).unwrap(),
            Vec::<(u64, u64)>::new()
        );
    }
    assert!(empty.to_bytes().is_ok());
    // One point: found by every radius from zero, and has no pair.
    let one = CellIndex::build(
        &[Point {
            key: 9,
            at: p3(-5, 5, 0),
        }],
        &exec,
        &mut meter(),
    )
    .unwrap();
    for radius in [0, 1, u64::MAX] {
        assert_eq!(
            one.within(p3(-5, 5, 0), radius, &mut meter()).unwrap(),
            vec![9]
        );
        assert_eq!(
            one.pairs_within(radius, &exec, &mut meter()).unwrap(),
            Vec::<(u64, u64)>::new()
        );
    }
    assert_eq!(
        one.within(p3(-5, 6, 0), 0, &mut meter()).unwrap(),
        Vec::<u64>::new()
    );
    // A repeated key is refused, as a failure the caller can read.
    let twice = [
        Point {
            key: 1,
            at: p3(0, 0, 0),
        },
        Point {
            key: 1,
            at: p3(1, 1, 1),
        },
    ];
    assert_eq!(
        CellIndex::build(&twice, &exec, &mut meter()).map(|_| ()),
        Err(BvhError::DuplicateKey(1))
    );
}

/// solidcore plan/upstream-issues/UB-221--smtreenode-neighbour-and-side-searches-trust-the-node-links.md
/// (b4de2149): a tree whose node links disagreed (a missing child, a duplicated node, a cycle)
/// dereferenced null or recursed without end. Ours cannot be handed links: the tree is built from
/// entries, and the inputs that make links disagree are the empty tree, one entry, entries whose
/// keys repeat, a refit that names a key the tree does not hold, and every box identical (the
/// split that never separates). Each must give the brute-force answer or a defined refusal, and
/// the depth stays within `MAX_DEPTH`.
#[test]
fn ub_221_the_box_tree_never_has_links_that_disagree() {
    let exec = Exec::serial();
    let b = |lo: [i64; 3], hi: [i64; 3]| Aabb::<3>::new(lo, hi).unwrap();
    let query = b([-1, -1, -1], [1, 1, 1]);

    let none = BoxTree::<3>::build(&[], &exec, &mut meter()).unwrap();
    assert!(none.is_empty());
    assert_eq!(
        none.overlapping(&query, &mut meter()).unwrap(),
        Vec::<u64>::new()
    );
    assert_eq!(none.pairs(&mut meter()).unwrap(), Vec::<(u64, u64)>::new());
    assert_eq!(none.depth().unwrap(), 0);

    let one = [Entry {
        key: 4,
        bbox: b([0, 0, 0], [0, 0, 0]),
    }];
    let tree = BoxTree::build(&one, &exec, &mut meter()).unwrap();
    assert_eq!(tree.overlapping(&query, &mut meter()).unwrap(), vec![4]);
    assert_eq!(tree.pairs(&mut meter()).unwrap(), Vec::<(u64, u64)>::new());

    // Two entries with one key (a node listed twice): refused, naming the key.
    let twice = [
        Entry {
            key: 5,
            bbox: b([0, 0, 0], [1, 1, 1]),
        },
        Entry {
            key: 5,
            bbox: b([0, 0, 0], [1, 1, 1]),
        },
    ];
    assert_eq!(
        BoxTree::build(&twice, &exec, &mut meter()).map(|_| ()),
        Err(BvhError::DuplicateKey(5))
    );

    // A refit naming a key the tree does not hold, or one key twice: refused, tree unchanged.
    let mut tree = BoxTree::build(&one, &exec, &mut meter()).unwrap();
    let before = tree.clone();
    let ghost = [Entry {
        key: 77,
        bbox: b([0, 0, 0], [0, 0, 0]),
    }];
    assert_eq!(
        tree.refit(&ghost, &exec, &mut meter()).map(|_| ()),
        Err(BvhError::UnknownKey(77))
    );
    let dup = [one[0], one[0]];
    assert_eq!(
        tree.refit(&dup, &exec, &mut meter()).map(|_| ()),
        Err(BvhError::DuplicateKey(4))
    );
    assert_eq!(tree, before);

    // Many identical boxes: no split separates them, and the tree is still shallow, and every
    // pair is found (n choose 2), against the brute-force count.
    let same: Vec<Entry<3>> = (0..300u64)
        .map(|k| Entry {
            key: k,
            bbox: b([-2, -2, -2], [2, 2, 2]),
        })
        .collect();
    let tree = BoxTree::build(&same, &exec, &mut meter()).unwrap();
    assert!(tree.depth().unwrap() <= MAX_DEPTH);
    assert_eq!(tree.overlapping(&query, &mut meter()).unwrap().len(), 300);
    assert_eq!(tree.pairs(&mut meter()).unwrap().len(), 300 * 299 / 2);
}

/// solidcore plan/upstream-issues/UB-41--smpointset3d-gettightpbox-on-an-empty-planar-set-reads-m.md
/// (b4de2149): a planar point set was given extra work that read its first point, and the set was
/// empty. Ours is asked for the boxes of a planar and a linear set (every box flat on one or two
/// axes): the tree must hold them and answer every overlap query as brute force does, and a query
/// box that is itself flat finds what touches it, and nothing that is a step away.
#[test]
fn ub_41_flat_boxes_are_held_and_found_exactly() {
    let exec = Exec::serial();
    // A planar set: every box has z = 0 and a thickness of zero steps; a linear set: y = z = 0.
    let mut entries = Vec::new();
    for i in 0..40i64 {
        for j in 0..5i64 {
            let k = (i * 5 + j) as u64;
            entries.push(Entry {
                key: k,
                bbox: Aabb::<3>::new([i * 3, j * 3, 0], [i * 3 + 1, j * 3, 0]).unwrap(),
            });
        }
    }
    for i in 0..30i64 {
        entries.push(Entry {
            key: 1000 + i as u64,
            bbox: Aabb::<3>::new([i * 2, 0, 0], [i * 2, 0, 0]).unwrap(),
        });
    }
    let tree = BoxTree::build(&entries, &exec, &mut meter()).unwrap();
    assert_eq!(tree.len(), entries.len());
    let queries = [
        Aabb::<3>::new([0, 0, 0], [0, 0, 0]).unwrap(),
        Aabb::<3>::new([-5, -5, 0], [200, 5, 0]).unwrap(),
        Aabb::<3>::new([10, 0, 1], [20, 9, 1]).unwrap(),
        Aabb::<3>::new([10, 0, -1], [20, 9, 1]).unwrap(),
        Aabb::<3>::new([7, 3, 0], [7, 3, 0]).unwrap(),
        Aabb::<3>::new([5, 0, 0], [5, 0, 0]).unwrap(),
    ];
    for q in queries {
        let mut want: Vec<u64> = entries
            .iter()
            .filter(|e| e.bbox.overlaps(&q))
            .map(|e| e.key)
            .collect();
        want.sort_unstable();
        assert_eq!(tree.overlapping(&q, &mut meter()).unwrap(), want, "{q:?}");
    }
    // A box flat on every axis has an area of the points it touches, never a zero that loses it.
    let dot = Aabb::<3>::new([5, 5, 5], [5, 5, 5]).unwrap();
    assert!(dot.area() > 0);
    // The pairs equal the brute-force pairs.
    let mut want = Vec::new();
    for (i, a) in entries.iter().enumerate() {
        for c in &entries[i + 1..] {
            if a.bbox.overlaps(&c.bbox) {
                want.push((a.key.min(c.key), a.key.max(c.key)));
            }
        }
    }
    want.sort_unstable();
    assert_eq!(tree.pairs(&mut meter()).unwrap(), want);
}
