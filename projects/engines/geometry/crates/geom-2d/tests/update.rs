// SPDX-License-Identifier: Apache-2.0
//! Attacks on piece G199: arrangement updates and point location (`geom_2d::update`; chapter 14.4
//! of the kernel architecture). Every test tries to break a claim of the piece:
//!
//! - **Incremental equals batch**: the segments of random small drawings, inserted one by one in
//!   any order, give the pieces and sources that the sweep gives, and an overlay gives the sweep's
//!   arrangement of the union. The sweep (`geom_2d::arrange`) is the independent oracle.
//! - **Shapes**: a crossing, a T, an overlap, end-to-end collinear segments, a vertical segment,
//!   a repeated segment and a repeated label.
//! - **Old versions stay**: an insert changes no earlier version, and shares every chunk it did
//!   not touch.
//! - **Location is exact**: a probe point is a vertex, on an edge, or in the face that an
//!   independent point-in-polygon test over the sweep's cycles names.
//! - **Refusals**: a stopped meter stops an insert and a location.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names
)]

use geom_2d::arrange::{ArrangeError, Arrangement, arrange};
use geom_2d::update::{Half, Live, Location};
use geom_num::carrier::Rational2;
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};

/// A small deterministic generator (`SplitMix64`, Steele, Lea and Flood 2014).
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn unlimited() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The drawing built by inserting `segs` in the order `order`, each labelled by its place in `segs`.
fn built(segs: &[(P2, P2)], order: &[usize]) -> Live {
    let mut live = Live::new();
    for &i in order {
        let (a, b) = segs[i];
        live = live.insert(a, b, i as u32, &mut unlimited()).unwrap();
    }
    live
}

fn random_segments(rng: &mut SplitMix64, n: usize, size: u64) -> Vec<(P2, P2)> {
    (0..n)
        .map(|_| {
            let c = |rng: &mut SplitMix64| rng.below(size) as i64;
            (p(c(rng), c(rng)), p(c(rng), c(rng)))
        })
        .collect()
}

fn shuffled(rng: &mut SplitMix64, n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        order.swap(i, rng.below(i as u64 + 1) as usize);
    }
    order
}

#[test]
fn a_crossing_makes_four_pieces_and_a_vertex() {
    let segs = [(p(0, 0), p(4, 4)), (p(0, 4), p(4, 0))];
    let live = built(&segs, &[0, 1]);
    assert_eq!(live.len(), 4);
    for piece in live.pieces() {
        assert_eq!(piece.sources().len(), 1);
    }
    assert!(
        live.same_as(&Live::batch(&segs, &mut unlimited()).unwrap())
            .unwrap()
    );
}

#[test]
fn a_t_a_vertical_and_an_overlap_are_the_sweeps() {
    let shapes: [Vec<(P2, P2)>; 5] = [
        vec![(p(0, 0), p(6, 0)), (p(3, 0), p(3, 5))],
        vec![(p(2, 0), p(2, 8)), (p(0, 3), p(5, 3)), (p(2, 5), p(2, 12))],
        vec![(p(0, 0), p(6, 0)), (p(2, 0), p(9, 0)), (p(4, 0), p(5, 0))],
        vec![(p(0, 0), p(3, 3)), (p(3, 3), p(6, 6)), (p(0, 0), p(6, 6))],
        vec![(p(0, 0), p(5, 5)), (p(5, 5), p(0, 0)), (p(0, 0), p(5, 5))],
    ];
    for segs in &shapes {
        let order: Vec<usize> = (0..segs.len()).collect();
        let live = built(segs, &order);
        let batch = Live::batch(segs, &mut unlimited()).unwrap();
        assert!(live.same_as(&batch).unwrap(), "{segs:?}");
        let reversed: Vec<usize> = order.iter().rev().copied().collect();
        assert!(built(segs, &reversed).same_as(&batch).unwrap(), "{segs:?}");
    }
}

#[test]
fn a_segment_of_no_length_and_a_repeated_label_change_nothing() {
    let live = Live::new()
        .insert(p(0, 0), p(4, 0), 7, &mut unlimited())
        .unwrap();
    let same = live.insert(p(2, 2), p(2, 2), 8, &mut unlimited()).unwrap();
    assert!(live.same_as(&same).unwrap());
    let again = live.insert(p(0, 0), p(4, 0), 7, &mut unlimited()).unwrap();
    assert!(live.same_as(&again).unwrap());
    assert_eq!(again.pieces().next().unwrap().sources(), &[7]);
}

#[test]
fn incremental_equals_batch_for_any_order() {
    let mut rng = SplitMix64(0x6199);
    for round in 0..300 {
        let n = 2 + rng.below(8) as usize;
        let segs = random_segments(&mut rng, n, 7);
        let batch = Live::batch(&segs, &mut unlimited()).unwrap();
        for _ in 0..3 {
            let order = shuffled(&mut rng, n);
            let live = built(&segs, &order);
            assert!(
                live.same_as(&batch).unwrap(),
                "round {round}, order {order:?}, segments {segs:?}"
            );
        }
    }
}

#[test]
fn an_overlay_is_the_arrangement_of_the_union() {
    let mut rng = SplitMix64(0x0BE7);
    for round in 0..150 {
        let n = 1 + rng.below(6) as usize;
        let m = 1 + rng.below(6) as usize;
        let all = random_segments(&mut rng, n + m, 6);
        let order: Vec<usize> = (0..n).collect();
        let left = built(&all, &order);
        let mut right = Live::new();
        for (i, &(a, b)) in all.iter().enumerate().skip(n) {
            right = right.insert(a, b, i as u32, &mut unlimited()).unwrap();
        }
        let both = left.overlay(&right, &mut unlimited()).unwrap();
        let flipped = right.overlay(&left, &mut unlimited()).unwrap();
        let batch = Live::batch(&all, &mut unlimited()).unwrap();
        assert!(both.same_as(&batch).unwrap(), "round {round}: {all:?}");
        assert!(flipped.same_as(&batch).unwrap(), "round {round}: {all:?}");
    }
}

/// A lattice of unit segments, noded by construction: `side` cells each way.
fn lattice(side: i64) -> Vec<(P2, P2, u32)> {
    let mut out = Vec::new();
    let mut label = 0_u32;
    for y in 0..=side {
        for x in 0..=side {
            if x < side {
                out.push((p(x, y), p(x + 1, y), label));
                label += 1;
            }
            if y < side {
                out.push((p(x, y), p(x, y + 1), label));
                label += 1;
            }
        }
    }
    out
}

#[test]
fn an_insert_leaves_the_old_version_and_shares_its_chunks() {
    let segs = lattice(40);
    let live = Live::from_noded(&segs).unwrap();
    assert!(live.chunk_count() > 10);
    let plain: Vec<(P2, P2)> = segs.iter().map(|&(a, b, _)| (a, b)).collect();
    assert!(
        live.same_as(&Live::batch(&plain, &mut unlimited()).unwrap())
            .unwrap_or(false)
            || live.len() == plain.len()
    );
    let before: Vec<usize> = live.pieces().map(|piece| piece.sources().len()).collect();
    let (next, cost) = live
        .insert_costed(p(10, 10), p(12, 11), segs.len() as u32, &mut unlimited())
        .unwrap();
    // The old version is as it was.
    let after: Vec<usize> = live.pieces().map(|piece| piece.sources().len()).collect();
    assert_eq!(before, after);
    assert_eq!(live.len(), segs.len());
    assert!(next.len() > live.len());
    // Only the chunks the zone is in were copied, and the rest are the same chunks.
    assert!(cost.chunks_copied <= 6, "{cost:?}");
    assert!(next.shared_chunks(&live) + 6 >= live.chunk_count());
    assert!(cost.examined < 400, "{cost:?}");
    // And the result is the sweep's arrangement of the same segments.
    let mut all = plain.clone();
    all.push((p(10, 10), p(12, 11)));
    let batch = Live::batch(&all, &mut unlimited()).unwrap();
    assert!(next.same_as(&batch).unwrap());
}

#[test]
fn a_meter_that_stops_stops_an_insert_and_a_location() {
    let live = Live::new()
        .insert(p(0, 0), p(4, 4), 0, &mut unlimited())
        .unwrap();
    let mut tight = WorkMeter::new(Budget::new(1, 1));
    let stopped = live.insert(p(0, 4), p(4, 0), 1, &mut tight);
    assert!(matches!(stopped, Err(ArrangeError::Stopped(_))));
    let mut tight = WorkMeter::new(Budget::new(1, 1));
    let at = Rational2::new(1, 3, 2).unwrap();
    assert!(matches!(
        live.locate(&at, &mut tight),
        Err(ArrangeError::Stopped(_))
    ));
}

// ---- location ----

fn rat(x: i128, y: i128, w: i128) -> Rational2 {
    Rational2::new(x, y, w).unwrap()
}

fn as_f64(r: &Rational2) -> (f64, f64) {
    let ([x, y], w) = r.coordinates();
    (x as f64 / w as f64, y as f64 / w as f64)
}

fn polygon(a: &Arrangement, cycle: &[u32]) -> Vec<(f64, f64)> {
    cycle
        .iter()
        .map(|&h| {
            let e = &a.edges()[(h / 2) as usize];
            let v = if h % 2 == 0 { e.from } else { e.to };
            as_f64(&a.vertices()[v as usize])
        })
        .collect()
}

fn inside(poly: &[(f64, f64)], at: (f64, f64)) -> bool {
    let mut crossings = 0;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.1 > at.1) != (b.1 > at.1) {
            let x = a.0 + (at.1 - a.1) / (b.1 - a.1) * (b.0 - a.0);
            if x > at.0 {
                crossings += 1;
            }
        }
    }
    crossings % 2 == 1
}

fn area(poly: &[(f64, f64)]) -> f64 {
    let mut s = 0.0;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        s += a.0 * b.1 - b.0 * a.1;
    }
    (s / 2.0).abs()
}

/// The face of the sweep's arrangement that contains `at`, by testing every outer cycle, the
/// innermost winning; `None` for the unbounded face.
fn oracle_face(a: &Arrangement, at: (f64, f64)) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for (f, face) in a.faces().iter().enumerate() {
        let Some(outer) = face.outer else { continue };
        let poly = polygon(a, &a.cycles()[outer as usize]);
        if inside(&poly, at) {
            let size = area(&poly);
            if best.is_none_or(|(s, _)| size < s) {
                best = Some((size, f));
            }
        }
    }
    best.map(|(_, f)| f)
}

/// The face of the sweep's arrangement that has the half-edge `h` on its left.
fn face_of_half(a: &Arrangement, h: &Half) -> usize {
    let same = |r: &Rational2, v: u32| r.same(&a.vertices()[v as usize]).unwrap();
    let e = a
        .edges()
        .iter()
        .position(|e| {
            (same(&h.from, e.from) && same(&h.to, e.to))
                || (same(&h.from, e.to) && same(&h.to, e.from))
        })
        .expect("a half-edge of the drawing");
    let id = if same(&h.from, a.edges()[e].from) {
        2 * e as u32
    } else {
        2 * e as u32 + 1
    };
    let cycle = a
        .cycles()
        .iter()
        .position(|c| c.contains(&id))
        .expect("on a cycle");
    a.faces()
        .iter()
        .position(|f| f.outer == Some(cycle as u32) || f.holes.contains(&(cycle as u32)))
        .expect("a face")
}

#[test]
fn location_agrees_with_point_in_polygon_over_the_sweeps_cycles() {
    let mut rng = SplitMix64(0x10C8);
    let mut faces = 0;
    let mut edges = 0;
    let mut vertices = 0;
    for round in 0..120 {
        let n = 2 + rng.below(9) as usize;
        let segs = random_segments(&mut rng, n, 7);
        let a = arrange(&segs, &mut unlimited()).unwrap();
        let order = shuffled(&mut rng, n);
        let live = built(&segs, &order);
        for _ in 0..40 {
            let w = [1_i128, 2, 4][rng.below(3) as usize];
            let at = rat(
                i128::from(rng.below(8 * w as u64 + 1)) - w / 2,
                i128::from(rng.below(8 * w as u64 + 1)) - w / 2,
                w,
            );
            let got = live.locate(&at, &mut unlimited()).unwrap();
            let is_vertex = a.vertices().iter().any(|v| v.same(&at).unwrap());
            match got {
                Location::Vertex(v) => {
                    vertices += 1;
                    assert!(is_vertex && v.same(&at).unwrap(), "round {round}");
                }
                Location::Edge(lo, hi) => {
                    edges += 1;
                    assert!(!is_vertex, "round {round}");
                    let found = a.edges().iter().any(|e| {
                        lo.same(&a.vertices()[e.from as usize]).unwrap()
                            && hi.same(&a.vertices()[e.to as usize]).unwrap()
                    });
                    assert!(found, "round {round}: an edge of the sweep");
                }
                Location::Face(half) => {
                    faces += 1;
                    assert!(!is_vertex, "round {round}: {at:?} is a vertex");
                    let want = oracle_face(&a, as_f64(&at));
                    match half {
                        None => assert!(
                            want.is_none(),
                            "round {round}: {at:?} said outside, oracle {want:?}; {segs:?}"
                        ),
                        Some(h) => {
                            let face = face_of_half(&a, &h);
                            let want_face = want.unwrap_or_else(|| {
                                a.faces().iter().position(|f| f.outer.is_none()).unwrap()
                            });
                            assert_eq!(
                                face, want_face,
                                "round {round}: {at:?} in {face}, oracle {want_face}; {segs:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(
        faces > 500 && edges > 20 && vertices > 20,
        "{faces} {edges} {vertices}"
    );
}

#[test]
fn a_probe_on_an_edge_or_a_vertex_is_named_so() {
    let live = built(
        &[(p(0, 0), p(4, 0)), (p(4, 0), p(4, 4)), (p(4, 4), p(0, 0))],
        &[0, 1, 2],
    );
    let mut m = unlimited();
    assert!(matches!(
        live.locate(&rat(4, 0, 1), &mut m).unwrap(),
        Location::Vertex(_)
    ));
    assert!(matches!(
        live.locate(&rat(2, 0, 1), &mut m).unwrap(),
        Location::Edge(..)
    ));
    assert!(matches!(
        live.locate(&rat(5, 5, 2), &mut m).unwrap(),
        Location::Edge(..)
    ));
    assert!(matches!(
        live.locate(&rat(7, 2, 2), &mut m).unwrap(),
        Location::Face(Some(_))
    ));
    assert!(matches!(
        live.locate(&rat(1, -1, 1), &mut m).unwrap(),
        Location::Face(None)
    ));
}

#[test]
fn a_face_cycle_runs_round_the_boundary() {
    // A square with a diagonal: the point in the lower triangle has a 3-cycle, upper another.
    let segs = [
        (p(0, 0), p(4, 0)),
        (p(4, 0), p(4, 4)),
        (p(4, 4), p(0, 4)),
        (p(0, 4), p(0, 0)),
        (p(0, 0), p(4, 4)),
    ];
    let live = built(&segs, &[0, 1, 2, 3, 4]);
    let mut m = unlimited();
    let Location::Face(Some(h)) = live.locate(&rat(3, 1, 1), &mut m).unwrap() else {
        panic!("a face");
    };
    let cycle = live.face_cycle(&h, &mut m).unwrap();
    assert_eq!(cycle.len(), 3);
    let Location::Face(Some(h)) = live.locate(&rat(1, 3, 1), &mut m).unwrap() else {
        panic!("a face");
    };
    assert_eq!(live.face_cycle(&h, &mut m).unwrap().len(), 3);
}
