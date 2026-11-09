// SPDX-License-Identifier: Apache-2.0
//! Attacks on piece G199a: arrangement updates and point location with circular arcs and one-root
//! points (`geom_2d::update`; chapter 14.4 of the kernel architecture). Every test tries to break a
//! claim of the piece:
//!
//! - **Incremental equals batch**: random small drawings of segments and arcs, inserted one by one
//!   in any order, give the pieces, on the same lines and circles and with the same sources, that
//!   the sweep gives; an overlay gives the sweep's arrangement of the union. The sweep
//!   (`geom_2d::arrange::arrange_curves`) is the independent oracle.
//! - **Shapes**: an arc across a segment (one-root vertices), two arcs across each other, an arc
//!   touching a line, a chord and its arc (two edges between one pair of vertices), two arcs of one
//!   circle overlapping, the two halves of one circle, and a small arc of a large circle beside a
//!   lattice (its box is the arc's, not the circle's).
//! - **Location is exact**: a probe point is a vertex, on an edge, or in the face that an
//!   independent point-in-polygon test over the sweep's cycles names, with each arc drawn as a
//!   polyline of many points; probes within a millionth of a curve are left to the exact tests.
//! - **Pieces**: every arc piece runs from its lower end to its higher on its half of its circle.
//!
//! Random inputs come from a fixed seed, printed as `seed: <hex>`; `SPAREKIT_SEED=<hex>` replays
//! another.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names
)]

use geom_2d::arrange::{Along, ArrangeError, Arrangement, At, Curve, arrange_curves};
use geom_2d::update::{Half, Live, Location, Track};
use geom_num::Sign;
use geom_num::arc::{Arc2, Round};
use geom_num::carrier::Rational2;
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};

const DEFAULT_SEED: u64 = 0x6199_a0a7_c5ee_d199;

fn seed() -> u64 {
    let s = std::env::var("SPAREKIT_SEED")
        .ok()
        .and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(DEFAULT_SEED);
    println!("seed: {s:x}");
    s
}

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

    fn below(&mut self, n: u64) -> i64 {
        (self.next_u64() % n) as i64
    }
}

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn seg(a: (i64, i64), b: (i64, i64)) -> Curve {
    Curve::Segment(p(a.0, a.1), p(b.0, b.1))
}

/// The arc counter-clockwise from `a` to `b` about the grid point `c`.
fn arc(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> Curve {
    Curve::Arc(Arc2::new(p(a.0, a.1), p(b.0, b.1), p(c.0, c.1).into()).unwrap())
}

fn unlimited() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn rat(x: i128, y: i128, w: i128) -> Rational2 {
    Rational2::new(x, y, w).unwrap()
}

/// The curves inserted one by one in `order`, each labelled by its place in `curves`.
fn built(curves: &[Curve], order: &[usize]) -> Live {
    let mut live = Live::new();
    for &i in order {
        live = live
            .insert_curve(&curves[i], i as u32, &mut unlimited())
            .unwrap();
    }
    live
}

fn batch(curves: &[Curve]) -> Live {
    Live::batch_curves(curves, &mut unlimited()).unwrap()
}

fn shuffled(rng: &mut SplitMix64, n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        order.swap(i, j);
    }
    order
}

/// A random arc between grid points of a small square, about a centre on its bisector a quarter
/// step apart: (2(P + Q) + s (Q - P) turned) / 4.
fn random_arc(rng: &mut SplitMix64, span: u64) -> Curve {
    loop {
        let (px, py, qx, qy) = (
            rng.below(span),
            rng.below(span),
            rng.below(span),
            rng.below(span),
        );
        if (px, py) == (qx, qy) {
            continue;
        }
        let s = rng.below(9) - 4;
        let x = 2 * (px + qx) - s * (qy - py);
        let y = 2 * (py + qy) + s * (qx - px);
        let centre = Rational2::new(i128::from(x), i128::from(y), 4).unwrap();
        return Curve::Arc(Arc2::new(p(px, py), p(qx, qy), centre).unwrap());
    }
}

/// A drawing of `n` curves, about half arcs, some of them on circles of earlier arcs: the other
/// way round, or with an end moved round the same circle.
fn random_drawing(rng: &mut SplitMix64, n: usize, span: u64) -> Vec<Curve> {
    let mut out: Vec<Curve> = Vec::new();
    while out.len() < n {
        let pick = rng.below(5);
        if pick == 0
            && let Some(Curve::Arc(a)) = out.iter().rev().find(|c| matches!(c, Curve::Arc(_)))
        {
            // The rest of an earlier arc's circle, or the arc itself again.
            let again = if rng.below(2) == 0 {
                Arc2::new(a.end(), a.start(), a.round().centre()).unwrap()
            } else {
                *a
            };
            out.push(Curve::Arc(again));
        } else if pick < 3 {
            out.push(random_arc(rng, span));
        } else {
            let (a, b) = (
                (rng.below(span), rng.below(span)),
                (rng.below(span), rng.below(span)),
            );
            out.push(seg(a, b));
        }
    }
    out
}

#[test]
fn arcs_inserted_in_any_order_give_the_sweeps_pieces() {
    let mut rng = SplitMix64(seed());
    for round in 0..150 {
        let n = 1 + rng.below(7) as usize;
        let curves = random_drawing(&mut rng, n, 6);
        let want = batch(&curves);
        for _ in 0..3 {
            let order = shuffled(&mut rng, n);
            let live = built(&curves, &order);
            assert!(
                live.same_as(&want).unwrap(),
                "round {round}, order {order:?}: {curves:?}"
            );
        }
    }
}

#[test]
fn an_overlay_of_arcs_is_the_sweep_of_the_union() {
    let mut rng = SplitMix64(seed() ^ 0x0E1A);
    for round in 0..80 {
        let n = 2 + rng.below(6) as usize;
        let curves = random_drawing(&mut rng, n, 6);
        let order = shuffled(&mut rng, n);
        let half = n / 2;
        let (left, right) = (
            built(&curves, &order[..half]),
            built(&curves, &order[half..]),
        );
        let both = left.overlay(&right, &mut unlimited()).unwrap();
        assert!(
            both.same_as(&batch(&curves)).unwrap(),
            "round {round}: {curves:?}"
        );
    }
}

/// The pieces of a drawing with their sources, as the sweep gives them.
fn pieces(live: &Live) -> Vec<(Track, Vec<u32>)> {
    live.pieces()
        .map(|e| (e.track().unwrap(), e.sources().to_vec()))
        .collect()
}

#[test]
fn shapes_with_arcs_are_the_sweeps() {
    let cases: [(&str, Vec<Curve>, usize); 7] = [
        (
            "an arc across a segment",
            vec![arc((5, 0), (-5, 0), (0, 0)), seg((-6, 1), (6, 1))],
            // The arc in three pieces at the two crossings, (±√24, 1), the segment in three.
            6,
        ),
        (
            "two arcs across each other",
            vec![arc((5, 0), (-5, 0), (0, 0)), arc((9, 0), (1, 0), (5, 0))],
            4,
        ),
        (
            "an arc touching a line",
            vec![arc((5, 0), (-5, 0), (0, 0)), seg((-6, 5), (6, 5))],
            4,
        ),
        (
            "a chord and its arc",
            vec![arc((5, 0), (0, 5), (0, 0)), seg((0, 5), (5, 0))],
            2,
        ),
        (
            "two arcs of one circle overlapping",
            vec![arc((5, 0), (-5, 0), (0, 0)), arc((0, 5), (0, -5), (0, 0))],
            3,
        ),
        (
            "the two halves of one circle",
            vec![arc((5, 0), (-5, 0), (0, 0)), arc((-5, 0), (5, 0), (0, 0))],
            2,
        ),
        (
            "an arc past its circle's top, leftmost point and bottom",
            vec![arc((3, 4), (3, -4), (0, 0))],
            2,
        ),
    ];
    for (what, curves, count) in cases {
        let want = batch(&curves);
        assert_eq!(want.len(), count, "{what}: {:?}", pieces(&want));
        for order in [vec![0, 1], vec![1, 0]] {
            let order: Vec<usize> = order.into_iter().filter(|&i| i < curves.len()).collect();
            let live = built(&curves, &order);
            assert!(live.same_as(&want).unwrap(), "{what}: {:?}", pieces(&live));
        }
    }
    // The overlap is one stretch with both arcs' labels: the quarter from (0, 5) to (-5, 0).
    let overlap = batch(&[arc((5, 0), (-5, 0), (0, 0)), arc((0, 5), (0, -5), (0, 0))]);
    let shared: Vec<_> = overlap.pieces().filter(|e| e.sources() == [0, 1]).collect();
    assert_eq!(shared.len(), 1);
    assert!(shared[0].lo().same(&At::Rational(rat(-5, 0, 1))).unwrap());
    assert!(shared[0].hi().same(&At::Rational(rat(0, 5, 1))).unwrap());
}

#[test]
fn a_chord_and_its_arc_are_two_edges_and_the_lens_between_them_is_one_face() {
    let curves = [arc((5, 0), (0, 5), (0, 0)), seg((0, 5), (5, 0))];
    let live = built(&curves, &[1, 0]);
    let mut m = unlimited();
    assert_eq!(live.len(), 2);
    // (3, 3) is between the chord (x + y = 5) and the arc (x² + y² = 25).
    let Location::Face(Some(h)) = live.locate(&rat(3, 3, 1), &mut m).unwrap() else {
        panic!("inside the lens");
    };
    let cycle = live.face_cycle(&h, &mut m).unwrap();
    assert_eq!(cycle.len(), 2, "{cycle:?}");
    let kinds: Vec<bool> = cycle
        .iter()
        .map(|h| matches!(h.on, Track::Arc { .. }))
        .collect();
    assert!(kinds.contains(&true) && kinds.contains(&false), "{cycle:?}");
    // Below the chord nothing is under the point: the unbounded face, with no edge below. Above the
    // arc the ray meets the arc at (4, 3), inside it, and the face is the unbounded one on the arc's
    // left as it runs, whose boundary is the lens's two edges, the arc first.
    assert!(matches!(
        live.locate(&rat(1, 1, 1), &mut m).unwrap(),
        Location::Face(None)
    ));
    let Location::Face(Some(h)) = live.locate(&rat(4, 4, 1), &mut m).unwrap() else {
        panic!("a face with the arc under it");
    };
    let ring = live.face_cycle(&h, &mut m).unwrap();
    assert_eq!(ring.len(), 2);
    assert!(matches!(ring[0].on, Track::Arc { .. }), "{ring:?}");
    assert!(matches!(ring[1].on, Track::Line(_)), "{ring:?}");
    // On the arc, at one-root precision: (3, 4) is a grid point of the circle; on the chord, (2, 3).
    assert!(matches!(
        live.locate(&rat(3, 4, 1), &mut m).unwrap(),
        Location::Edge(Half {
            on: Track::Arc { .. },
            ..
        })
    ));
    assert!(matches!(
        live.locate(&rat(2, 3, 1), &mut m).unwrap(),
        Location::Edge(Half {
            on: Track::Line(_),
            ..
        })
    ));
    assert!(matches!(
        live.locate(&rat(5, 0, 1), &mut m).unwrap(),
        Location::Vertex(_)
    ));
}

#[test]
fn a_ray_down_through_a_one_root_vertex_finds_the_face_above_it() {
    // Two circles about (0, 0) and (2, 0), through (4, 2) and (5, 1): they cross at (7/2, ±√7.75),
    // one-root points with a rational x, so a ray straight down at x = 7/2 meets each exactly. Above
    // the upper one is the unbounded face; above the lower one, the lens inside both circles.
    let curves = [
        arc((4, 2), (-4, -2), (0, 0)),
        arc((-4, -2), (4, 2), (0, 0)),
        arc((5, 1), (-1, -1), (2, 0)),
        arc((-1, -1), (5, 1), (2, 0)),
    ];
    let live = built(&curves, &[3, 0, 2, 1]);
    let a = arrange_curves(&curves, &mut unlimited()).unwrap();
    let mut m = unlimited();
    let lens = |face: usize| {
        let outer = a.faces()[face].outer.expect("a bounded face");
        let poly = polygon(&a, &a.cycles()[outer as usize]);
        inside(&poly, (3.0, 0.0)) && !inside(&poly, (-3.0, 0.0)) && !inside(&poly, (4.5, 0.0))
    };
    let Location::Face(Some(h)) = live.locate(&rat(7, 20, 2), &mut m).unwrap() else {
        panic!("a face over the upper crossing");
    };
    assert!(matches!(h.from, At::Root(_)), "{h:?}");
    assert!(a.faces()[face_of_half(&a, &h)].outer.is_none(), "{h:?}");
    let Location::Face(Some(h)) = live.locate(&rat(7, 0, 2), &mut m).unwrap() else {
        panic!("a face over the lower crossing");
    };
    assert!(matches!(h.from, At::Root(_)), "{h:?}");
    assert!(lens(face_of_half(&a, &h)), "{h:?}");
    assert!(matches!(
        live.locate(&rat(7, -20, 2), &mut m).unwrap(),
        Location::Face(None)
    ));
}

#[test]
fn every_arc_piece_runs_from_its_lower_end_to_its_higher_on_its_half() {
    let mut rng = SplitMix64(seed() ^ 0x0A1F);
    for round in 0..60 {
        let n = 1 + rng.below(6) as usize;
        let curves = random_drawing(&mut rng, n, 6);
        let order = shuffled(&mut rng, n);
        for (what, live) in [
            ("batch", batch(&curves)),
            ("insert", built(&curves, &order)),
        ] {
            for e in live.pieces() {
                let (lo, hi) = (e.lo().crossing().unwrap(), e.hi().crossing().unwrap());
                let x = lo.cmp_x(&hi).unwrap();
                assert!(
                    x == Sign::Negative
                        || (x == Sign::Zero && lo.cmp_y(&hi).unwrap() == Sign::Negative),
                    "{what}, round {round}: {e:?}"
                );
                if let Track::Arc { circle, upper } = e.track().unwrap() {
                    let centre = geom_num::one_root::Crossing2::of_point(&circle.centre()).unwrap();
                    for end in [&lo, &hi] {
                        assert_eq!(
                            end.against_round(&circle).unwrap(),
                            Sign::Zero,
                            "{what}: {e:?}"
                        );
                        let level = end.cmp_y(&centre).unwrap();
                        let wrong = if upper {
                            Sign::Negative
                        } else {
                            Sign::Positive
                        };
                        assert_ne!(level, wrong, "{what}, round {round}: {e:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_small_arc_of_a_large_circle_opens_only_what_it_meets() {
    // A lattice of unit segments from 0 to 40, and the short arc from (100, 100) to (101, 101)
    // about a centre far up and to the left on their bisector: its circle passes far round the
    // lattice, but the arc's own box is a step across, so no chunk is opened.
    let mut lattice = Vec::new();
    for y in 0..=40_i64 {
        for x in 0..40_i64 {
            lattice.push((p(x, y), p(x + 1, y), lattice.len() as u32));
            lattice.push((p(y, x), p(y, x + 1), lattice.len() as u32));
        }
    }
    let live = Live::from_noded(&lattice).unwrap();
    // The bisector of (100, 100) and (101, 101) is x + y = 201; (-1000, 1201) is on it.
    let small = Curve::Arc(Arc2::new(p(100, 100), p(101, 101), p(-1000, 1201).into()).unwrap());
    let (after, cost) = live
        .insert_curve_costed(&small, 9999, &mut unlimited())
        .unwrap();
    assert_eq!(cost.chunks_opened, 0, "{cost:?}");
    assert_eq!(after.len(), live.len() + 1);
    // The rest of the circle, the long way round, passes by the lattice, and its boxes meet it.
    let big = Curve::Arc(Arc2::new(p(101, 101), p(100, 100), p(-1000, 1201).into()).unwrap());
    let (_, cost) = live
        .insert_curve_costed(&big, 9999, &mut unlimited())
        .unwrap();
    assert!(cost.chunks_opened > 0, "{cost:?}");
}

#[test]
fn an_arc_away_from_its_circles_lowest_point_opens_only_what_its_ends_span() {
    // A lattice of unit segments over [600, 640] x [0, 40], and the arc of the circle about
    // (0, 1000) through the origin from (600, 200) to (800, 400): on its lower half, but not past
    // its lowest point (0, 0), so its box is that of its ends, above the lattice.
    let mut lattice = Vec::new();
    for y in 0..=40_i64 {
        for x in 0..40_i64 {
            lattice.push((p(600 + x, y), p(601 + x, y), lattice.len() as u32));
            lattice.push((p(600 + y, x), p(600 + y, x + 1), lattice.len() as u32));
        }
    }
    let live = Live::from_noded(&lattice).unwrap();
    let side = arc((600, 200), (800, 400), (0, 1000));
    let (after, cost) = live
        .insert_curve_costed(&side, 9999, &mut unlimited())
        .unwrap();
    assert_eq!(cost.chunks_opened, 0, "{cost:?}");
    assert_eq!(after.len(), live.len() + 1);
    // Over the lowest point, from (-600, 200) to (600, 200), the box reaches down to the circle's
    // bottom, through the lattice, which the arc does not meet.
    let under = arc((-600, 200), (600, 200), (0, 1000));
    let (after, cost) = live
        .insert_curve_costed(&under, 9999, &mut unlimited())
        .unwrap();
    assert!(cost.chunks_opened > 0, "{cost:?}");
    assert_eq!(after.len(), live.len() + 1);
}

#[test]
fn the_two_halves_of_a_circle_between_the_same_points_are_not_the_same_drawing() {
    // Both run between (-5, 0) and (5, 0) on one circle with one label: only the half differs.
    let upper = built(&[arc((5, 0), (-5, 0), (0, 0))], &[0]);
    let lower = built(&[arc((-5, 0), (5, 0), (0, 0))], &[0]);
    assert_eq!((upper.len(), lower.len()), (1, 1));
    assert!(!upper.same_as(&lower).unwrap());
    assert!(!lower.same_as(&upper).unwrap());
    assert!(upper.same_as(&upper.clone()).unwrap());
    // The same half of another circle through both points is not the same either.
    let other = built(&[arc((5, 0), (-5, 0), (0, -1))], &[0]);
    assert!(!upper.same_as(&other).unwrap());
}

#[test]
fn a_stopped_meter_stops_an_arc_insert() {
    let live = built(&[seg((-6, 3), (6, 3))], &[0]);
    let mut tight = WorkMeter::new(Budget::new(1, 1));
    assert!(matches!(
        live.insert_curve(&arc((5, 0), (-5, 0), (0, 0)), 1, &mut tight),
        Err(ArrangeError::Stopped(_))
    ));
}

// ---- location against an independent point-in-polygon test ----

/// A vertex in doubles: the middle of its certified enclosure.
fn f64_of(v: &At) -> (f64, f64) {
    let [x, y] = v.crossing().unwrap().enclosure();
    (f64::midpoint(x.lo(), x.hi()), f64::midpoint(y.lo(), y.hi()))
}

/// A circle in doubles: its centre and radius.
fn circle_f64(r: &Round) -> ((f64, f64), f64) {
    let ([x, y], w) = r.centre().coordinates();
    let (cx, cy) = (x as f64 / w as f64, y as f64 / w as f64);
    let on = r.on();
    let (ox, oy) = (on.x.get() as f64, on.y.get() as f64);
    ((cx, cy), ((ox - cx).powi(2) + (oy - cy).powi(2)).sqrt())
}

/// The angle of `q` about `c`, on the upper half in [0, π] and on the lower in [-π, 0].
fn angle(c: (f64, f64), q: (f64, f64), upper: bool) -> f64 {
    let a = (q.1 - c.1).atan2(q.0 - c.0);
    if upper && a < 0.0 {
        a.abs()
    } else if !upper && a > 0.0 {
        -a
    } else {
        a
    }
}

/// The boundary `cycle` of the sweep's arrangement as a polygon, each arc drawn by many points.
fn polygon(a: &Arrangement, cycle: &[u32]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for &h in cycle {
        let e = &a.edges()[(h / 2) as usize];
        let (u, v) = if h % 2 == 0 {
            (e.from, e.to)
        } else {
            (e.to, e.from)
        };
        let (pu, pv) = (
            f64_of(&a.vertices()[u as usize]),
            f64_of(&a.vertices()[v as usize]),
        );
        out.push(pu);
        if let Some(Along::Arc { circle, upper }) = a.along(h / 2) {
            let (c, r) = circle_f64(&a.circles()[circle as usize]);
            let (t0, t1) = (angle(c, pu, upper), angle(c, pv, upper));
            for k in 1..64 {
                let t = t0 + (t1 - t0) * f64::from(k) / 64.0;
                out.push((c.0 + r * t.cos(), c.1 + r * t.sin()));
            }
        }
    }
    out
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

/// The face of the sweep's arrangement that holds `at`, the innermost outer cycle winning; `None`
/// for the unbounded face.
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

/// Whether the sweep's edge `e` runs along what `on` names.
fn along_is(a: &Arrangement, e: u32, on: &Track) -> bool {
    match (a.along(e).unwrap(), on) {
        (Along::Line(l), Track::Line(m)) => l.same(m).unwrap(),
        (
            Along::Arc { circle, upper },
            Track::Arc {
                circle: r,
                upper: u,
            },
        ) => {
            let mine = &a.circles()[circle as usize];
            upper == *u
                && mine.centre().same(&r.centre()).unwrap()
                && mine.side_of_grid(r.on()).unwrap() == Sign::Zero
        }
        _ => false,
    }
}

/// The face of the sweep's arrangement that has the half-edge `h` on its left.
fn face_of_half(a: &Arrangement, h: &Half) -> usize {
    let same = |r: &At, v: u32| r.same(&a.vertices()[v as usize]).unwrap();
    let e = a
        .edges()
        .iter()
        .enumerate()
        .position(|(i, e)| {
            ((same(&h.from, e.from) && same(&h.to, e.to))
                || (same(&h.from, e.to) && same(&h.to, e.from)))
                && along_is(a, i as u32, &h.on)
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

/// How near `at` is to a curve, in doubles.
fn near_a_curve(curves: &[Curve], at: (f64, f64)) -> bool {
    curves.iter().any(|c| match c {
        Curve::Segment(a, b) => {
            let (ax, ay, bx, by) = (
                a.x.get() as f64,
                a.y.get() as f64,
                b.x.get() as f64,
                b.y.get() as f64,
            );
            let (dx, dy) = (bx - ax, by - ay);
            let len = (dx * dx + dy * dy).max(1e-300);
            let t = (((at.0 - ax) * dx + (at.1 - ay) * dy) / len).clamp(0.0, 1.0);
            ((ax + t * dx - at.0).powi(2) + (ay + t * dy - at.1).powi(2)).sqrt() < 1e-6
        }
        Curve::Arc(arc) => {
            let (c, r) = circle_f64(&arc.round());
            (((at.0 - c.0).powi(2) + (at.1 - c.1).powi(2)).sqrt() - r).abs() < 1e-6
        }
    })
}

#[test]
fn location_with_arcs_agrees_with_point_in_polygon_over_the_sweeps_cycles() {
    let mut rng = SplitMix64(seed() ^ 0x10CA);
    let (mut faces, mut edges, mut vertices) = (0, 0, 0);
    for round in 0..100 {
        let n = 1 + rng.below(6) as usize;
        let curves = random_drawing(&mut rng, n, 6);
        let a = arrange_curves(&curves, &mut unlimited()).unwrap();
        let order = shuffled(&mut rng, n);
        let live = built(&curves, &order);
        for _ in 0..40 {
            let w = [1_i128, 2, 4, 8][rng.below(4) as usize];
            let at = rat(
                i128::from(rng.below(7 * w as u64 + 1)) - w / 2,
                i128::from(rng.below(7 * w as u64 + 1)) - w / 2,
                w,
            );
            let here = At::Rational(at);
            let got = live.locate(&at, &mut unlimited()).unwrap();
            let is_vertex = a.vertices().iter().any(|v| v.same(&here).unwrap());
            match got {
                Location::Vertex(v) => {
                    vertices += 1;
                    assert!(is_vertex && v.same(&here).unwrap(), "round {round}");
                }
                Location::Edge(h) => {
                    edges += 1;
                    assert!(!is_vertex, "round {round}");
                    let found = a.edges().iter().enumerate().any(|(i, e)| {
                        h.from.same(&a.vertices()[e.from as usize]).unwrap()
                            && h.to.same(&a.vertices()[e.to as usize]).unwrap()
                            && along_is(&a, i as u32, &h.on)
                    });
                    assert!(found, "round {round}: an edge of the sweep");
                }
                Location::Face(half) => {
                    assert!(!is_vertex, "round {round}: {at:?} is a vertex");
                    let (x, y) = f64_of(&here);
                    if near_a_curve(&curves, (x, y)) {
                        continue;
                    }
                    faces += 1;
                    let want = oracle_face(&a, (x, y));
                    match half {
                        None => assert!(
                            want.is_none(),
                            "round {round}: {at:?} said outside, oracle {want:?}; {curves:?}"
                        ),
                        Some(h) => {
                            let face = face_of_half(&a, &h);
                            let want_face = want.unwrap_or_else(|| {
                                a.faces().iter().position(|f| f.outer.is_none()).unwrap()
                            });
                            assert_eq!(
                                face, want_face,
                                "round {round}: {at:?} in {face}, oracle {want_face}; {curves:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(
        faces > 1000 && edges > 20 && vertices > 20,
        "{faces} {edges} {vertices}"
    );
}

#[test]
fn every_face_walk_comes_back_round_its_cycle() {
    // Each half-edge of the sweep starts a walk that comes back to it, as long as the sweep's
    // cycle through it, with the same faces on the left.
    let mut rng = SplitMix64(seed() ^ 0xCAFE);
    for round in 0..60 {
        let n = 1 + rng.below(6) as usize;
        let curves = random_drawing(&mut rng, n, 6);
        let a = arrange_curves(&curves, &mut unlimited()).unwrap();
        let live = built(&curves, &shuffled(&mut rng, n));
        for cycle in a.cycles() {
            let h = cycle[0];
            let e = &a.edges()[(h / 2) as usize];
            let (u, v) = if h % 2 == 0 {
                (e.from, e.to)
            } else {
                (e.to, e.from)
            };
            let on = match a.along(h / 2).unwrap() {
                Along::Line(l) => Track::Line(l),
                Along::Arc { circle, upper } => Track::Arc {
                    circle: a.circles()[circle as usize],
                    upper,
                },
            };
            let half = Half {
                from: a.vertices()[u as usize].clone(),
                to: a.vertices()[v as usize].clone(),
                on,
            };
            let walk = live.face_cycle(&half, &mut unlimited()).unwrap();
            assert_eq!(walk.len(), cycle.len(), "round {round}: {curves:?}");
            for step in &walk {
                assert_eq!(
                    face_of_half(&a, step),
                    face_of_half(&a, &half),
                    "round {round}"
                );
            }
        }
    }
}
