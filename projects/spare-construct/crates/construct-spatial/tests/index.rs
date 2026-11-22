//! The index's promises, judged against a plain scan of every box.
#![allow(
    clippy::unwrap_used,
    clippy::assert_is_empty,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

mod common;

use common::{Seq, id, owned, random_box, random_scope, scan_inside, scan_meeting, scan_nearest};
use construct_model::geometry::Bounds;
use construct_spatial::{Entry, MAX_COORDINATE, Scope, SpatialError, SpatialIndex};

fn b(min: [i64; 3], max: [i64; 3]) -> Bounds {
    Bounds { min, max }
}

#[test]
fn an_empty_index_answers_with_nothing() {
    let index = SpatialIndex::new();
    let everything = b([-1000; 3], [1000; 3]);
    assert!(index.is_empty());
    assert!(index.meeting(&everything, None).unwrap().is_empty());
    assert!(index.inside(&everything, None).unwrap().is_empty());
    assert!(index.nearest([0; 3], 5, None).unwrap().is_empty());
}

#[test]
fn insert_get_update_remove() {
    let mut index = SpatialIndex::new();
    index.insert(id(1), b([0; 3], [10; 3]), None).unwrap();
    index
        .insert(id(2), b([100; 3], [110; 3]), Some(Scope::Level(3)))
        .unwrap();
    assert_eq!(index.len(), 2);
    assert_eq!(index.get(&id(2)).unwrap().scope, Some(Scope::Level(3)));

    let near_one = b([5; 3], [6; 3]);
    assert_eq!(owned(index.meeting(&near_one, None).unwrap()), vec![id(1)]);
    index.update(&id(1), b([500; 3], [510; 3]), None).unwrap();
    assert!(index.meeting(&near_one, None).unwrap().is_empty());
    assert_eq!(index.get(&id(1)).unwrap().bounds, b([500; 3], [510; 3]));

    let removed = index.remove(&id(1)).unwrap();
    assert_eq!(removed.bounds, b([500; 3], [510; 3]));
    assert_eq!(index.len(), 1);
    assert!(
        index
            .meeting(&b([500; 3], [510; 3]), None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn bad_input_is_refused_and_changes_nothing() {
    let mut index = SpatialIndex::new();
    index.insert(id(1), b([0; 3], [10; 3]), None).unwrap();
    assert_eq!(
        index.insert(id(1), b([0; 3], [1; 3]), None),
        Err(SpatialError::Duplicate(id(1)))
    );
    assert_eq!(
        index.insert(id(2), b([5, 0, 0], [4, 1, 1]), None),
        Err(SpatialError::Inverted)
    );
    assert_eq!(
        index.insert(id(2), b([0; 3], [MAX_COORDINATE + 1, 1, 1]), None),
        Err(SpatialError::OutOfRange)
    );
    assert_eq!(
        index.update(&id(9), b([0; 3], [1; 3]), None),
        Err(SpatialError::Missing(id(9)))
    );
    assert_eq!(
        index.update(&id(1), b([3, 0, 0], [2, 1, 1]), None),
        Err(SpatialError::Inverted)
    );
    assert_eq!(index.remove(&id(9)), Err(SpatialError::Missing(id(9))));
    assert_eq!(
        index.meeting(&b([4, 0, 0], [3, 1, 1]), None),
        Err(SpatialError::Inverted)
    );
    assert_eq!(
        index.nearest([MAX_COORDINATE + 1, 0, 0], 1, None),
        Err(SpatialError::OutOfRange)
    );
    assert_eq!(index.len(), 1);
    assert_eq!(index.get(&id(1)).unwrap().bounds, b([0; 3], [10; 3]));
    // The limit itself is allowed.
    index
        .insert(id(3), b([-MAX_COORDINATE; 3], [MAX_COORDINATE; 3]), None)
        .unwrap();
}

#[test]
fn touching_counts_for_both_box_queries() {
    let mut index = SpatialIndex::new();
    index
        .insert(id(1), b([10, 0, 0], [20, 10, 10]), None)
        .unwrap();
    // Shares the face x = 10, and the corner (10, 10, 10).
    let face = b([0, 0, 0], [10, 10, 10]);
    assert_eq!(owned(index.meeting(&face, None).unwrap()), vec![id(1)]);
    let corner = b([0, 10, 10], [10, 20, 20]);
    assert_eq!(owned(index.meeting(&corner, None).unwrap()), vec![id(1)]);
    // One millimetre clear.
    assert!(
        index
            .meeting(&b([0; 3], [9, 10, 10]), None)
            .unwrap()
            .is_empty()
    );
    // Inside: exactly equal counts; one millimetre short does not.
    assert_eq!(
        owned(index.inside(&b([10, 0, 0], [20, 10, 10]), None).unwrap()),
        vec![id(1)]
    );
    assert!(
        index
            .inside(&b([10, 0, 0], [19, 10, 10]), None)
            .unwrap()
            .is_empty()
    );
    // A point box is a box.
    index.insert(id(2), b([5, 5, 5], [5, 5, 5]), None).unwrap();
    assert_eq!(
        owned(index.inside(&b([5; 3], [5; 3]), None).unwrap()),
        vec![id(2)]
    );
}

#[test]
fn nearest_ranks_by_distance_to_the_box_and_breaks_ties_by_id() {
    let mut index = SpatialIndex::new();
    // A big box holding the point is at distance zero, nearer than a tiny box 101 mm away.
    index.insert(id(5), b([-100; 3], [100; 3]), None).unwrap();
    index
        .insert(id(1), b([101, 0, 0], [102, 1, 1]), None)
        .unwrap();
    // Four boxes all 300 mm away along different axes: a tie at the last place.
    index
        .insert(id(4), b([300, 0, 0], [301, 1, 1]), None)
        .unwrap();
    index
        .insert(id(3), b([0, 300, 0], [1, 301, 1]), None)
        .unwrap();
    index
        .insert(id(2), b([-301, 0, 0], [-300, 1, 1]), None)
        .unwrap();
    index
        .insert(id(6), b([0, 0, 300], [1, 1, 301]), None)
        .unwrap();
    // Ranking from the origin: 5 (inside), 1, then the four at 300. Three asked: 5, 1 and the
    // smallest id of the tie, 2.
    assert_eq!(
        owned(index.nearest([0; 3], 2, None).unwrap()),
        vec![id(1), id(5)]
    );
    assert_eq!(
        owned(index.nearest([0; 3], 3, None).unwrap()),
        vec![id(1), id(2), id(5)]
    );
    assert_eq!(owned(index.nearest([0; 3], 0, None).unwrap()), Vec::new());
    assert_eq!(index.nearest([0; 3], 99, None).unwrap().len(), 6);
}

#[test]
fn a_scope_limits_every_query() {
    let mut index = SpatialIndex::new();
    index
        .insert(id(1), b([0; 3], [10; 3]), Some(Scope::Level(0)))
        .unwrap();
    index
        .insert(id(2), b([0; 3], [10; 3]), Some(Scope::Level(1)))
        .unwrap();
    index.insert(id(3), b([0; 3], [10; 3]), None).unwrap();
    let all = b([-5; 3], [50; 3]);
    let level_one = Scope::Level(1);
    assert_eq!(
        owned(index.meeting(&all, Some(&level_one)).unwrap()),
        vec![id(2)]
    );
    assert_eq!(
        owned(index.inside(&all, Some(&level_one)).unwrap()),
        vec![id(2)]
    );
    assert_eq!(
        owned(index.nearest([0; 3], 3, Some(&level_one)).unwrap()),
        vec![id(2)]
    );
    assert_eq!(index.meeting(&all, None).unwrap().len(), 3);
}

/// Every query, many shapes of box, compared with the scan.
fn compare_all(index: &SpatialIndex, all: &[Entry], seq: &mut Seq, queries: usize) {
    let extent = [30_000, 30_000, 10_000];
    for _ in 0..queries {
        let scope = random_scope(seq);
        let scope = scope.as_ref();
        let q = random_box(seq, extent, 8_000);
        assert_eq!(
            owned(index.meeting(&q, scope).unwrap()),
            scan_meeting(all, &q, scope)
        );
        assert_eq!(
            owned(index.inside(&q, scope).unwrap()),
            scan_inside(all, &q, scope)
        );
        let p = [
            seq.between(-extent[0], extent[0]),
            seq.between(-extent[1], extent[1]),
            seq.between(-extent[2], extent[2]),
        ];
        let count = usize::try_from(seq.between(1, 30)).unwrap();
        assert_eq!(
            owned(index.nearest(p, count, scope).unwrap()),
            scan_nearest(all, p, count, scope)
        );
    }
}

#[test]
fn thousands_of_moves_stay_correct() {
    let mut seq = Seq(20_261_122);
    let extent = [30_000, 30_000, 10_000];
    let mut all: Vec<Entry> = (0..4_000)
        .map(|n| Entry {
            id: id(n),
            bounds: random_box(&mut seq, extent, 3_000),
            scope: random_scope(&mut seq),
        })
        .collect();
    let mut index = SpatialIndex::from_entries(all.clone()).unwrap();
    compare_all(&index, &all, &mut seq, 150);

    let mut next_new = 4_000_u64;
    for round in 0..6_000 {
        match seq.next() % 10 {
            // Move or resize.
            0..=6 => {
                let at = usize::try_from(seq.next()).unwrap() % all.len();
                let bounds = random_box(&mut seq, extent, 3_000);
                let scope = random_scope(&mut seq);
                index.update(&all[at].id, bounds, scope.clone()).unwrap();
                all[at] = Entry {
                    id: all[at].id.clone(),
                    bounds,
                    scope,
                };
            }
            // Remove.
            7 if all.len() > 100 => {
                let at = usize::try_from(seq.next()).unwrap() % all.len();
                let gone = all.swap_remove(at);
                assert_eq!(index.remove(&gone.id).unwrap(), gone);
            }
            // Add.
            _ => {
                let entry = Entry {
                    id: id(next_new),
                    bounds: random_box(&mut seq, extent, 3_000),
                    scope: random_scope(&mut seq),
                };
                next_new += 1;
                index
                    .insert(entry.id.clone(), entry.bounds, entry.scope.clone())
                    .unwrap();
                all.push(entry);
            }
        }
        assert_eq!(index.len(), all.len());
        if round % 500 == 499 {
            compare_all(&index, &all, &mut seq, 80);
        }
    }
    compare_all(&index, &all, &mut seq, 300);

    // Rebuilt from the records alone, it answers as the updated one does.
    let rebuilt = SpatialIndex::from_entries(all).unwrap();
    for _ in 0..200 {
        let q = random_box(&mut seq, extent, 8_000);
        assert_eq!(
            index.meeting(&q, None).unwrap(),
            rebuilt.meeting(&q, None).unwrap()
        );
        assert_eq!(
            index.inside(&q, None).unwrap(),
            rebuilt.inside(&q, None).unwrap()
        );
    }
}

#[test]
fn many_equal_boxes_do_not_confuse_removal() {
    // Identical boxes under different ids: removing one takes exactly that one.
    let mut index = SpatialIndex::new();
    for n in 0..50 {
        index.insert(id(n), b([0; 3], [10; 3]), None).unwrap();
    }
    for n in (0..50).step_by(2) {
        index.remove(&id(n)).unwrap();
    }
    let left = owned(index.meeting(&b([0; 3], [1; 3]), None).unwrap());
    assert_eq!(
        left,
        (0..50).filter(|n| n % 2 == 1).map(id).collect::<Vec<_>>()
    );
}

#[test]
fn from_entries_refuses_a_repeated_id_and_a_bad_box() {
    let entry = |n, bounds| Entry {
        id: id(n),
        bounds,
        scope: None,
    };
    let twice =
        SpatialIndex::from_entries([entry(1, b([0; 3], [1; 3])), entry(1, b([5; 3], [6; 3]))]);
    assert_eq!(twice.unwrap_err(), SpatialError::Duplicate(id(1)));
    let bad = SpatialIndex::from_entries([entry(1, b([2, 0, 0], [1, 1, 1]))]);
    assert_eq!(bad.unwrap_err(), SpatialError::Inverted);
}

#[test]
fn answers_do_not_depend_on_the_order_boxes_went_in() {
    let mut seq = Seq(5);
    let entries: Vec<Entry> = (0..2_000)
        .map(|n| Entry {
            id: id(n),
            bounds: random_box(&mut seq, [20_000; 3], 2_000),
            scope: None,
        })
        .collect();
    let forward = SpatialIndex::from_entries(entries.clone()).unwrap();
    let mut backward = SpatialIndex::new();
    for e in entries.iter().rev() {
        backward.insert(e.id.clone(), e.bounds, None).unwrap();
    }
    for _ in 0..200 {
        let q = random_box(&mut seq, [20_000; 3], 6_000);
        assert_eq!(
            forward.meeting(&q, None).unwrap(),
            backward.meeting(&q, None).unwrap()
        );
        let p = [seq.between(-20_000, 20_000); 3];
        assert_eq!(
            forward.nearest(p, 10, None).unwrap(),
            backward.nearest(p, 10, None).unwrap()
        );
    }
}

#[test]
fn the_far_edge_of_the_range_is_still_exact() {
    // Boxes near the largest coordinate, where a squared distance no longer fits a float exactly.
    let mut seq = Seq(40);
    let far = [MAX_COORDINATE - 20_000; 3];
    let all: Vec<Entry> = (0..1_500)
        .map(|n| Entry {
            id: id(n),
            bounds: {
                let near = random_box(&mut seq, [10_000; 3], 500);
                let shift = |c: [i64; 3]| [0, 1, 2].map(|axis| c[axis] + far[axis] - 5_000);
                Bounds {
                    min: shift(near.min),
                    max: shift(near.max),
                }
            },
            scope: None,
        })
        .collect();
    let index = SpatialIndex::from_entries(all.clone()).unwrap();
    for _ in 0..200 {
        let p = [
            -MAX_COORDINATE + seq.between(0, 5),
            seq.between(-MAX_COORDINATE, MAX_COORDINATE),
            far[2] + seq.between(-9_000, 9_000),
        ];
        let count = usize::try_from(seq.between(1, 40)).unwrap();
        assert_eq!(
            owned(index.nearest(p, count, None).unwrap()),
            scan_nearest(&all, p, count, None)
        );
    }
}
