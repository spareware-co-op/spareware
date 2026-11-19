//! Attacks on piece G196b's ellipses in the arrangement (`geom_2d::arrange::arrange_curves`;
//! chapter 14.4 of the kernel architecture). Every test tries to break a claim: a whole ellipse is
//! cut where it is leftmost and rightmost into two halves split by the diameter through those
//! points, a line meets it exactly at one-root points and touches it at one vertex, one ellipse
//! written two ways is listed once, an ellipse of ratio one is its circle, holes are placed inside
//! ellipses, the input order does not matter, and an ellipse that may meet a circle or another
//! ellipse is refused, never arranged wrongly.
//!
//! - **Shapes**: a rotated ellipse, an axis-aligned one, a line across, a line touching, a line
//!   through both extremes, a vertical line at an extreme, nested ellipses, an ellipse beside a
//!   circle, and the same ellipse twice.
//! - **Vertices are exact**: on random drawings of segments and ellipses, the vertices are exactly
//!   an independent oracle's (every segment end, both extremes of every ellipse, every crossing of
//!   two segments, and every crossing of a segment's line with an ellipse that lies on the
//!   segment), compared by the exact `same`.
//! - **The graph is planar and complete**: Euler's formula, every half-edge on one cycle, every
//!   cycle in one face, the vertices ascending.
//! - **Permutation-invariant**: shuffling the input changes only the source labels.
//! - **Extremes**: an ellipse as wide as the frame, crossed by a line at the frame's edge, under two
//!   circles about the heaviest centres whose crossings lie over it.
//! - **Refusals**: an ellipse crossing a circle, or another ellipse, is `ArrangeError::Conic`; an
//!   arc whose circle meets an ellipse only away from the arc is arranged.
//! - **Elliptic arcs**: cut only where an extreme lies inside the arc; their ends exactly the points
//!   of their parameters, and an end past the budget refused; an arc's halves and its whole ellipse
//!   one curve; random drawings of segments and elliptic arcs against an oracle that places a point
//!   on an arc by the angle of its parameter, and an end by exact comparison.
//!
//! Seeds: the random tests print `seed: <hex>` and run from a fixed default; `SPAREKIT_SEED=<hex>`
//! replays another (chapter 28.17).

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

use geom_2d::arrange::{Along, ArrangeError, Arrangement, At, BodyError, Curve, arrange_curves};
use geom_2d::entity::{Frac, Param};
use geom_num::arc::{Arc2, Contact, Disc};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::conic::{Conic, Ellipse2, Meeting};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::one_root::{Crossing2, Root};
use geom_num::{NumError, Sign};
use geom_run::{Budget, WorkMeter};

/// The default seed of the random tests here.
const DEFAULT_SEED: u64 = 0x6196_b0e1_1195_e196;

fn seed() -> u64 {
    let s = std::env::var("SPAREKIT_SEED")
        .ok()
        .and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(DEFAULT_SEED);
    println!("seed: {s:x}");
    s
}

/// `SplitMix64` (Steele, Lea and Flood 2014), for the drawings.
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

fn ellipse(c: (i64, i64), m: [i64; 2], ratio: (i64, i64)) -> Ellipse2 {
    Ellipse2::new(p(c.0, c.1), m, ratio.0, ratio.1).unwrap()
}

/// A whole circle about a grid centre of whole radius, as its two halves.
fn circle(cx: i64, cy: i64, r: i64) -> [Curve; 2] {
    let arc = |a: (i64, i64), b: (i64, i64)| {
        Curve::Arc(Arc2::new(p(a.0, a.1), p(b.0, b.1), p(cx, cy).into()).unwrap())
    };
    [
        arc((cx + r, cy), (cx - r, cy)),
        arc((cx - r, cy), (cx + r, cy)),
    ]
}

fn try_run(curves: &[Curve]) -> Result<Arrangement, ArrangeError> {
    arrange_curves(curves, &mut WorkMeter::new(Budget::UNLIMITED))
}

fn run(curves: &[Curve]) -> Arrangement {
    try_run(curves).unwrap()
}

fn grid(x: i64, y: i64) -> At {
    At::Rational(p(x, y).into())
}

/// The rank of the vertex at `at`.
fn rank_of(a: &Arrangement, at: &Crossing2) -> u32 {
    a.vertices()
        .iter()
        .position(|v| v.crossing().unwrap().same(at).unwrap())
        .unwrap_or_else(|| panic!("no vertex at {at:?}")) as u32
}

/// The root of `i` in a union-find forest.
fn find(up: &mut [usize], mut i: usize) -> usize {
    while up[i] != i {
        up[i] = up[up[i]];
        i = up[i];
    }
    i
}

/// The number of connected components of the edges.
fn components(a: &Arrangement) -> usize {
    let n = a.vertices().len();
    let mut up: Vec<usize> = (0..n).collect();
    for e in a.edges() {
        let (x, y) = (find(&mut up, e.from as usize), find(&mut up, e.to as usize));
        up[x] = y;
    }
    let mut roots: Vec<usize> = (0..n).map(|i| find(&mut up, i)).collect();
    roots.sort_unstable();
    roots.dedup();
    roots.len()
}

/// Euler's formula, every half-edge on one cycle, every cycle the outside of one face or a hole of
/// exactly one, and the vertices strictly ascending.
fn laws(a: &Arrangement, what: &str) {
    let (v, e, f, c) = (
        a.vertices().len(),
        a.edges().len(),
        a.faces().len(),
        components(a),
    );
    assert_eq!(
        v + f,
        e + c + 1,
        "{what}: Euler's formula V - E + F = 1 + C"
    );
    let halves: usize = a
        .cycles()
        .iter()
        .map(Vec::len)
        .fold(0, usize::saturating_add);
    assert_eq!(halves, 2 * e, "{what}: every half-edge is on one cycle");
    let mut owners = vec![0_u32; a.cycles().len()];
    for face in a.faces() {
        for c in face.outer.iter().chain(face.holes.iter()) {
            owners[*c as usize] += 1;
        }
    }
    assert!(
        owners.iter().all(|&n| n == 1),
        "{what}: every cycle is in one face"
    );
    for w in a.vertices().windows(2) {
        let x = w[0].cmp_x(&w[1]).unwrap();
        assert!(
            x == Sign::Negative
                || (x == Sign::Zero && w[0].cmp_y(&w[1]).unwrap() == Sign::Negative),
            "{what}: the vertices ascend strictly"
        );
    }
    for (i, edge) in a.edges().iter().enumerate() {
        assert!(edge.from < edge.to, "{what}: edge {i} runs up");
        assert!(a.along(i as u32).is_some());
    }
}

/// The leftmost (`Root::First`) or rightmost point of `e`.
fn extreme(e: &Ellipse2, r: Root) -> Crossing2 {
    Crossing2::of_ellipse_extreme(e, r).unwrap()
}

#[test]
fn an_ellipse_is_two_edges_between_its_leftmost_and_rightmost_points() {
    // Rotated: its major axis (3, 4), its minor half as long.
    let e = ellipse((1, 2), [3, 4], (1, 2));
    let a = run(&[Curve::Ellipse(e)]);
    laws(&a, "ellipse");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (2, 2, 2)
    );
    assert_eq!(a.ellipses(), &[e]);
    assert!(a.circles().is_empty());
    assert_eq!(rank_of(&a, &extreme(&e, Root::First)), 0);
    assert_eq!(rank_of(&a, &extreme(&e, Root::Second)), 1);
    // Between the same two vertices, in the order they leave the left one: down first.
    assert_eq!(
        a.along(0),
        Some(Along::Ellipse {
            ellipse: 0,
            upper: false
        })
    );
    assert_eq!(
        a.along(1),
        Some(Along::Ellipse {
            ellipse: 0,
            upper: true
        })
    );
    assert_eq!(a.edges()[0].sources, [0]);
    // A curved edge is not stored in a body: the first is named.
    assert!(matches!(
        a.to_body(&mut WorkMeter::new(Budget::UNLIMITED)),
        Err(BodyError::Curved { edge: 0 })
    ));
}

#[test]
fn an_axis_aligned_ellipse_has_its_extremes_on_its_axis() {
    let e = ellipse((0, 0), [4, 0], (1, 2));
    let a = run(&[Curve::Ellipse(e)]);
    laws(&a, "axis-aligned");
    assert_eq!(a.vertices().len(), 2);
    assert!(a.vertices()[0].same(&grid(-4, 0)).unwrap());
    assert!(a.vertices()[1].same(&grid(4, 0)).unwrap());
}

#[test]
fn a_line_across_an_ellipse_splits_both_and_shares_its_two_crossings() {
    let e = ellipse((0, 0), [4, 0], (1, 2));
    let a = run(&[Curve::Ellipse(e), seg((-6, -3), (6, 3))]);
    laws(&a, "across");
    // Two extremes, two crossings and the segment's two ends; the line splits the inside in two.
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (6, 7, 3)
    );
    let l = Carrier::through(p(-6, -3), p(6, 3)).unwrap();
    for r in [Root::First, Root::Second] {
        let z = Crossing2::of_ellipse(&l, &e, r).unwrap();
        let v = rank_of(&a, &z) as usize;
        assert_eq!(
            a.around(v as u32).len(),
            4,
            "a crossing has four half-edges"
        );
    }
}

#[test]
fn a_line_touching_an_ellipse_is_one_touching_vertex() {
    // x²/16 + y²/4 = 1 and the line y = 2, which touches it at (0, 2).
    let e = ellipse((0, 0), [4, 0], (1, 2));
    let a = run(&[Curve::Ellipse(e), seg((-5, 2), (5, 2))]);
    laws(&a, "touching");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (5, 5, 2)
    );
    let touch = rank_of(&a, &Crossing2::of_point(&p(0, 2).into()).unwrap());
    // Counter-clockwise from the west: the line leaves west, the ellipse west and below it, then
    // east below the line, and the line east.
    let around = a.around(touch);
    assert_eq!(around.len(), 4);
    let lies = |h: u32| a.along(h / 2).unwrap();
    assert!(matches!(lies(around[0]), Along::Line(_)));
    assert!(matches!(
        lies(around[1]),
        Along::Ellipse { upper: true, .. }
    ));
    assert!(matches!(
        lies(around[2]),
        Along::Ellipse { upper: true, .. }
    ));
    assert!(matches!(lies(around[3]), Along::Line(_)));
}

#[test]
fn a_vertical_line_at_an_extreme_touches_it_there() {
    let e = ellipse((0, 0), [4, 0], (1, 2));
    let a = run(&[Curve::Ellipse(e), seg((-4, -3), (-4, 3))]);
    laws(&a, "vertical at the extreme");
    assert_eq!(a.vertices().len(), 4);
    let at = rank_of(&a, &extreme(&e, Root::First));
    assert_eq!(a.around(at).len(), 4);
}

#[test]
fn a_chord_through_both_extremes_halves_the_inside() {
    let e = ellipse((0, 0), [4, 0], (1, 2));
    let a = run(&[Curve::Ellipse(e), seg((-4, 0), (4, 0))]);
    laws(&a, "chord");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (2, 3, 3)
    );
    // The chord's edge sits between the ellipse's two halves round the left extreme.
    let around = a.around(0);
    assert_eq!(around.len(), 3);
    assert!(matches!(a.along(around[1] / 2), Some(Along::Line(_))));
}

#[test]
fn nested_ellipses_make_a_ring_with_a_hole() {
    let inner = ellipse((0, 0), [4, 0], (1, 2));
    let outer = ellipse((0, 0), [8, 0], (1, 2));
    let a = run(&[Curve::Ellipse(outer), Curve::Ellipse(inner)]);
    laws(&a, "nested");
    assert_eq!(a.ellipses().len(), 2);
    assert_eq!(a.faces().len(), 3);
    let with_hole = a.faces().iter().filter(|f| !f.holes.is_empty()).count();
    assert_eq!(
        with_hole, 2,
        "the unbounded face and the ring each hold one hole"
    );
}

#[test]
fn one_ellipse_written_two_ways_is_listed_once_and_shares_its_edges() {
    let e = ellipse((2, 1), [3, 4], (1, 3));
    let turned = ellipse((2, 1), [-3, -4], (1, 3));
    let a = run(&[Curve::Ellipse(turned), Curve::Ellipse(e)]);
    laws(&a, "twice");
    assert_eq!(a.ellipses().len(), 1);
    // The stored form is the one with the least integers.
    assert_eq!(a.ellipses()[0], turned);
    assert_eq!(a.edges().len(), 2);
    for edge in a.edges() {
        assert_eq!(edge.sources, [0, 1]);
    }
}

#[test]
fn an_ellipse_of_ratio_one_is_its_circle() {
    let e = ellipse((0, 0), [5, 0], (1, 1));
    let [upper, _] = circle(0, 0, 5);
    let a = run(&[Curve::Ellipse(e), upper]);
    laws(&a, "ratio one");
    assert_eq!(a.ellipses(), &[] as &[Ellipse2]);
    assert_eq!(a.circles().len(), 1);
    assert_eq!(a.edges().len(), 2);
    // The upper arc covers the upper edge with the ellipse; the lower is the ellipse's alone.
    let covers: Vec<Vec<u32>> = a.edges().iter().map(|e| e.sources.clone()).collect();
    assert_eq!(covers, [vec![0], vec![0, 1]]);
    // A tilted one too: its centre plus its major-axis vector is a point of the circle.
    let tilted = ellipse((1, 1), [3, 4], (1, 1));
    let b = run(&[Curve::Ellipse(tilted)]);
    laws(&b, "tilted ratio one");
    assert_eq!((b.circles().len(), b.ellipses().len()), (1, 0));
    // A whole circle, like a whole ellipse, has no vertex but its leftmost and rightmost points:
    // not the ends (4, 5) and (-2, -3) of the axis that wrote it.
    assert_eq!(b.vertices().len(), 2);
    assert!(b.vertices()[0].same(&grid(-4, 1)).unwrap());
    assert!(b.vertices()[1].same(&grid(6, 1)).unwrap());
    let halves: Vec<Option<Along>> = (0..2).map(|e| b.along(e)).collect();
    assert_eq!(
        halves,
        [
            Some(Along::Arc {
                circle: 0,
                upper: false
            }),
            Some(Along::Arc {
                circle: 0,
                upper: true
            })
        ]
    );
    // Beside the arc of its circle from (6, 1) to (4, 5), the arc's ends split it there, and the
    // arc covers the edge between them with it.
    let arc = Curve::Arc(Arc2::new(p(6, 1), p(4, 5), p(1, 1).into()).unwrap());
    let c = run(&[Curve::Ellipse(tilted), arc]);
    laws(&c, "ratio one beside an arc");
    assert_eq!((c.vertices().len(), c.edges().len()), (3, 3));
    let covers: Vec<Vec<u32>> = c.edges().iter().map(|e| e.sources.clone()).collect();
    assert_eq!(covers.iter().filter(|s| s.as_slice() == [0, 1]).count(), 1);
    // An arc of another circle after it is cut on its own circle: the circle written as an ellipse
    // takes both of the listed arcs it was listed by, and no more.
    let beyond = Curve::Arc(Arc2::new(p(22, 0), p(18, 0), p(20, 0).into()).unwrap());
    let d = run(&[Curve::Ellipse(tilted), beyond]);
    laws(&d, "ratio one, then an arc of another circle");
    assert_eq!(d.circles().len(), 2);
    assert_eq!((d.vertices().len(), d.edges().len()), (4, 3));
    let lies: Vec<(Vec<u32>, Option<Along>)> = (0..3)
        .map(|e| (d.edges()[e as usize].sources.clone(), d.along(e)))
        .collect();
    assert!(lies.contains(&(
        vec![1],
        Some(Along::Arc {
            circle: 1,
            upper: true
        })
    )));
}

#[test]
fn an_ellipse_crossing_a_circle_or_another_ellipse_is_refused() {
    let e = ellipse((0, 0), [6, 0], (1, 2));
    let mut curves = vec![Curve::Ellipse(e)];
    curves.extend(circle(0, 0, 4));
    assert!(matches!(try_run(&curves), Err(ArrangeError::Conic)));
    let other = ellipse((0, 0), [0, 6], (1, 2));
    assert!(matches!(
        try_run(&[Curve::Ellipse(e), Curve::Ellipse(other)]),
        Err(ArrangeError::Conic)
    ));
    // Touching from inside, at both ends of the major axis: refused too.
    let mut touching = vec![Curve::Ellipse(e)];
    touching.extend(circle(0, 0, 6));
    assert!(matches!(try_run(&touching), Err(ArrangeError::Conic)));
    // Two ellipses touching at both ends of their common major axis: their lower halves leave the
    // leftmost point together, straight down, and are refused there, before any meeting is sought.
    let wider = ellipse((0, 0), [6, 0], (2, 3));
    assert!(matches!(
        try_run(&[Curve::Ellipse(e), Curve::Ellipse(wider)]),
        Err(ArrangeError::Conic)
    ));
    assert_eq!(
        ArrangeError::Conic.to_string(),
        "an ellipse may meet a circle or another ellipse, at a root of a quartic the sweep does not place"
    );
}

#[test]
fn an_ellipse_beside_a_circle_it_does_not_meet_is_arranged() {
    let e = ellipse((0, 0), [6, 0], (1, 2));
    let mut curves = vec![Curve::Ellipse(e)];
    curves.extend(circle(20, 1, 4));
    let a = run(&curves);
    laws(&a, "beside");
    assert_eq!((a.circles().len(), a.ellipses().len()), (1, 1));
    assert_eq!(a.faces().len(), 3);
    // A circle inside it: a hole of the ellipse's face.
    let mut inside = vec![Curve::Ellipse(e)];
    inside.extend(circle(1, 0, 2));
    let b = run(&inside);
    laws(&b, "inside");
    assert_eq!(b.faces().len(), 3);
}

#[test]
fn an_arc_whose_circle_meets_the_ellipse_only_elsewhere_is_arranged() {
    // The circle of radius 4 about (0, 0) crosses the ellipse x²/36 + y²/9 = 1 at
    // (±√(28/3), ±√(20/3)), near (±3.06, ±2.58): its quarter from (0, 4) to (−4, 0) passes one, so
    // it is refused.
    let e = ellipse((0, 0), [6, 0], (1, 2));
    let quarter = Curve::Arc(Arc2::new(p(0, 4), p(-4, 0), p(0, 0).into()).unwrap());
    assert!(matches!(
        try_run(&[Curve::Ellipse(e), quarter]),
        Err(ArrangeError::Conic)
    ));
    // The circle of radius 5 crosses it near (±4.62, ±1.91); its arc from (3, 4) over the top to
    // (−3, 4) reaches none, and lies outside the ellipse, so it is arranged.
    let over = Curve::Arc(Arc2::new(p(3, 4), p(-3, 4), p(0, 0).into()).unwrap());
    let b = run(&[Curve::Ellipse(e), over]);
    laws(&b, "an arc away from the crossings");
    assert_eq!(b.faces().len(), 2);
    // The circle of radius 2 about the ellipse's top (0, 3) crosses it twice, at y = (−3 + √132)/3
    // and x = ±√(4 − (y − 3)²), near (±1.99, 2.83), on the ellipse's upper half and its own lower:
    // its quarter from its lowest point (0, 1) to (2, 3) holds the right crossing on both pieces,
    // and the left one lies wholly left of the quarter, so it is refused for the right one alone.
    let short = Curve::Arc(Arc2::new(p(0, 1), p(2, 3), p(0, 3).into()).unwrap());
    assert!(matches!(
        try_run(&[Curve::Ellipse(e), short]),
        Err(ArrangeError::Conic)
    ));
}

/// A random ellipse about a grid point of a small square: its major axis a small vector, its ratio
/// p / q below one.
fn random_ellipse(rng: &mut SplitMix64, span: u64) -> Ellipse2 {
    loop {
        let c = p(rng.below(span), rng.below(span));
        let m = [rng.below(9) - 4, rng.below(9) - 4];
        let q = 2 + rng.below(4);
        let pp = 1 + rng.below(q as u64 - 1);
        if m == [0, 0] || gcd(pp, q) != 1 {
            continue;
        }
        return Ellipse2::new(c, m, pp, q).unwrap();
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn random_drawing(rng: &mut SplitMix64, n: usize, span: u64) -> Vec<Curve> {
    (0..n)
        .map(|_| {
            if rng.below(3) == 0 {
                Curve::Ellipse(random_ellipse(rng, span))
            } else {
                let (a, b) = (
                    (rng.below(span), rng.below(span)),
                    (rng.below(span), rng.below(span)),
                );
                seg(a, b)
            }
        })
        .collect()
}

/// Whether the crossing `z` of a segment's line lies between the segment's ends.
fn within(z: &Crossing2, a: P2, b: P2) -> bool {
    let (za, zb) = (
        Crossing2::of_point(&a.into()).unwrap(),
        Crossing2::of_point(&b.into()).unwrap(),
    );
    let along = |u: &Crossing2, v: &Crossing2| match u.cmp_x(v).unwrap() {
        Sign::Zero => u.cmp_y(v).unwrap(),
        s => s,
    };
    let (sa, sb) = (along(z, &za), along(z, &zb));
    sa != sb || sa == Sign::Zero
}

/// Whether two ellipses meet: their quartic has a real root, or they are one curve.
fn ellipses_meet(e: &Ellipse2, f: &Ellipse2) -> bool {
    match e.meeting(&Conic::of_ellipse(f).unwrap()).unwrap() {
        Meeting::Same => true,
        Meeting::At { roots, .. } => roots.count() > 0,
    }
}

/// The oracle's vertices, pair by pair: every segment end, both extremes of every ellipse, and every
/// crossing of two curves that lies on both. Ellipses here never meet each other.
fn oracle(curves: &[Curve]) -> Vec<Crossing2> {
    let mut out: Vec<Crossing2> = Vec::new();
    let add = |z: Crossing2, out: &mut Vec<Crossing2>| {
        if !out.iter().any(|o| o.same(&z).unwrap()) {
            out.push(z);
        }
    };
    let point = |q: P2| Crossing2::of_point(&q.into()).unwrap();
    for c in curves {
        match c {
            Curve::Segment(a, b) => {
                if a != b {
                    add(point(*a), &mut out);
                    add(point(*b), &mut out);
                }
            }
            Curve::Ellipse(e) => {
                add(extreme(e, Root::First), &mut out);
                add(extreme(e, Root::Second), &mut out);
            }
            Curve::Arc(arc) => {
                add(point(arc.start()), &mut out);
                add(point(arc.end()), &mut out);
                let disc = Disc::of_round(&arc.round()).unwrap();
                for r in [Root::First, Root::Second] {
                    let z = Crossing2::of_extreme(&disc, r).unwrap();
                    if arc.holds_crossing(&z).unwrap() {
                        add(z, &mut out);
                    }
                }
            }
            Curve::EllipticArc { .. } => panic!("these drawings hold no elliptic arc"),
        }
    }
    for (i, c) in curves.iter().enumerate() {
        for d in &curves[i + 1..] {
            for z in crossings(c, d) {
                add(z, &mut out);
            }
        }
    }
    out
}

/// Where two curves of the drawing cross, on both: ellipses here never meet a circle or each other.
fn crossings(c: &Curve, d: &Curve) -> Vec<Crossing2> {
    match (c, d) {
        (Curve::Segment(a, b), Curve::Segment(e, f)) => {
            if a == b || e == f {
                return vec![];
            }
            let (l, m) = (
                Carrier::through(*a, *b).unwrap(),
                Carrier::through(*e, *f).unwrap(),
            );
            if l.parallel(&m).unwrap() {
                return vec![];
            }
            let z = Crossing2::of_point(&l.meet(&m).unwrap()).unwrap();
            if within(&z, *a, *b) && within(&z, *e, *f) {
                vec![z]
            } else {
                vec![]
            }
        }
        (Curve::Segment(a, b), Curve::Ellipse(e)) | (Curve::Ellipse(e), Curve::Segment(a, b)) => {
            if a == b {
                return vec![];
            }
            let l = Carrier::through(*a, *b).unwrap();
            let roots = match e.meets(&l).unwrap() {
                Contact::Apart => vec![],
                Contact::Tangent => vec![Root::First],
                Contact::Crossing => vec![Root::First, Root::Second],
            };
            roots
                .into_iter()
                .map(|r| Crossing2::of_ellipse(&l, e, r).unwrap())
                .filter(|z| within(z, *a, *b))
                .collect()
        }
        (Curve::Segment(a, b), Curve::Arc(arc)) | (Curve::Arc(arc), Curve::Segment(a, b)) => {
            if a == b {
                return vec![];
            }
            let l = Carrier::through(*a, *b).unwrap();
            let roots = match arc.round().meets(&l).unwrap() {
                Contact::Apart => vec![],
                Contact::Tangent => vec![Root::First],
                Contact::Crossing => vec![Root::First, Root::Second],
            };
            roots
                .into_iter()
                .map(|r| Crossing2::of_round(&l, &arc.round(), r).unwrap())
                .filter(|z| within(z, *a, *b) && arc.holds_crossing(z).unwrap())
                .collect()
        }
        (Curve::Arc(x), Curve::Arc(y)) => {
            let mut found = vec![];
            for r in [Root::First, Root::Second] {
                match Crossing2::of_rounds(&x.round(), &y.round(), r) {
                    Ok(z) => {
                        if x.holds_crossing(&z).unwrap() && y.holds_crossing(&z).unwrap() {
                            found.push(z);
                        }
                    }
                    Err(NumError::Degenerate) => {}
                    Err(e) => panic!("{e:?}"),
                }
            }
            found
        }
        (Curve::Ellipse(_), _) | (_, Curve::Ellipse(_)) => vec![],
        (Curve::EllipticArc { .. }, _) | (_, Curve::EllipticArc { .. }) => {
            panic!("these drawings hold no elliptic arc")
        }
    }
}

#[test]
fn random_drawings_of_segments_and_ellipses_match_the_pairwise_oracle_and_keep_every_law() {
    let mut rng = SplitMix64(seed());
    let rounds = if cfg!(debug_assertions) { 60 } else { 400 };
    let (mut arranged, mut refused) = (0, 0);
    for round in 0..rounds {
        let n = 2 + rng.below(8) as usize;
        let curves = random_drawing(&mut rng, n, 12);
        let ellipses: Vec<Ellipse2> = curves
            .iter()
            .filter_map(|c| match c {
                Curve::Ellipse(e) | Curve::EllipticArc { ellipse: e, .. } => Some(*e),
                _ => None,
            })
            .collect();
        let meet = ellipses
            .iter()
            .enumerate()
            .any(|(i, e)| ellipses[i + 1..].iter().any(|f| ellipses_meet(e, f)));
        let what = format!("round {round}: {curves:?}");
        let a = match try_run(&curves) {
            Ok(a) => a,
            Err(ArrangeError::Conic) => {
                assert!(meet, "{what}: refused, but no two ellipses meet");
                refused += 1;
                continue;
            }
            Err(e) => panic!("{what}: {e:?}"),
        };
        laws(&a, &what);
        if meet {
            // Meeting ellipses reached only through a third curve's vertex: still a valid graph.
            continue;
        }
        arranged += 1;
        let want = oracle(&curves);
        assert_eq!(a.vertices().len(), want.len(), "{what}: the vertex count");
        for v in a.vertices() {
            let z = v.crossing().unwrap();
            assert!(
                want.iter().any(|w| w.same(&z).unwrap()),
                "{what}: {v:?} is the oracle's"
            );
        }
    }
    assert!(
        arranged > rounds / 3,
        "{arranged} arranged, {refused} refused"
    );
}

#[test]
fn the_arrangement_of_ellipses_does_not_depend_on_the_input_order() {
    let mut rng = SplitMix64(seed() ^ 1);
    let rounds = if cfg!(debug_assertions) { 30 } else { 200 };
    for round in 0..rounds {
        let n = 3 + rng.below(6) as usize;
        let curves = random_drawing(&mut rng, n, 12);
        let Ok(a) = try_run(&curves) else {
            continue;
        };
        let mut order: Vec<usize> = (0..curves.len()).collect();
        for i in (1..order.len()).rev() {
            let j = rng.below(i as u64 + 1) as usize;
            order.swap(i, j);
        }
        let shuffled: Vec<Curve> = order.iter().map(|&i| curves[i]).collect();
        let b = try_run(&shuffled).unwrap();
        let what = format!("round {round}: {curves:?}");
        assert_eq!(a.vertices().len(), b.vertices().len(), "{what}");
        for (u, v) in a.vertices().iter().zip(b.vertices()) {
            assert!(u.same(v).unwrap(), "{what}: the vertices agree");
        }
        let ea: Vec<(u32, u32)> = a.edges().iter().map(|e| (e.from, e.to)).collect();
        let eb: Vec<(u32, u32)> = b.edges().iter().map(|e| (e.from, e.to)).collect();
        assert_eq!(ea, eb, "{what}: the edges agree");
        let la: Vec<Option<Along>> = (0..ea.len() as u32).map(|e| a.along(e)).collect();
        let lb: Vec<Option<Along>> = (0..eb.len() as u32).map(|e| b.along(e)).collect();
        assert_eq!(la, lb, "{what}: what each edge lies on agrees");
        assert_eq!(a.ellipses(), b.ellipses(), "{what}: the ellipses agree");
        for (ea, eb) in a.edges().iter().zip(b.edges()) {
            let mut mapped: Vec<u32> = eb
                .sources
                .iter()
                .map(|&s| order[s as usize] as u32)
                .collect();
            mapped.sort_unstable();
            assert_eq!(
                ea.sources, mapped,
                "{what}: the sources agree once relabelled"
            );
        }
        assert_eq!(a.faces(), b.faces(), "{what}: the faces agree");
        for v in 0..a.vertices().len() as u32 {
            assert_eq!(
                a.around(v),
                b.around(v),
                "{what}: the order round {v} agrees"
            );
        }
    }
}

#[test]
fn an_ellipse_as_wide_as_the_frame_under_crossings_of_the_heaviest_circles_stays_exact() {
    let m = COORD_MAX;
    // The ellipse: centre (0, -2^38), major axis (2^40, 0), ratio 1 / 1024, so it spans every x of
    // the frame and lies far below the circles.
    let e = Ellipse2::new(p(0, -(1 << 38)), [1 << 40, 0], 1, 1024).unwrap();
    // Two circles about centres on the y axis with weights near 2^82, near y = 2^39, of radius near
    // 2^30, a quarter of 2^30 apart: they cross at two one-root points above the ellipse.
    let w: i128 = (1 << 82) - 1;
    let mut curves = vec![Curve::Ellipse(e)];
    for (h, reach, lift) in [
        (1_i64 << 39, 1_i64 << 30, 12_345_i128),
        ((1 << 39) + (1 << 28), (1 << 30) + 1_000, 678_901),
    ] {
        let centre = Rational2::new(0, i128::from(h) * w + lift, w).unwrap();
        let (s, t) = (p(-reach, h), p(reach, h));
        curves.push(Curve::Arc(Arc2::new(s, t, centre).unwrap()));
        curves.push(Curve::Arc(Arc2::new(t, s, centre).unwrap()));
    }
    curves.push(seg(
        (-m, -(1 << 38) - (1 << 31)),
        (m, -(1 << 38) + (1 << 31)),
    ));
    for c in &curves[1..5] {
        let Curve::Arc(arc) = c else { panic!("an arc") };
        assert!(!matches!(
            e.meeting(&Conic::of_round(&arc.round()).unwrap()).unwrap(),
            Meeting::At { roots, .. } if roots.count() > 0
        ));
    }
    let a = run(&curves);
    laws(&a, "the frame");
    assert_eq!((a.circles().len(), a.ellipses().len()), (2, 1));
    let want = oracle(&curves);
    assert_eq!(a.vertices().len(), want.len());
    for v in a.vertices() {
        let z = v.crossing().unwrap();
        assert!(want.iter().any(|w| w.same(&z).unwrap()), "{v:?}");
    }
    // The two circles cross: two of the vertices are where.
    let [Curve::Arc(x), _, Curve::Arc(y), _] = [curves[1], curves[2], curves[3], curves[4]] else {
        panic!("arcs")
    };
    for r in [Root::First, Root::Second] {
        let z = Crossing2::of_rounds(&x.round(), &y.round(), r).unwrap();
        rank_of(&a, &z);
    }
}

// ------------------------------------------------------------------------------------------------
// Elliptic arcs.
// ------------------------------------------------------------------------------------------------

fn at(n: i64, d: i64) -> Param {
    Param::At(Frac::new(n, d).unwrap())
}

fn elliptic(e: Ellipse2, start: Param, end: Param) -> Curve {
    Curve::EllipticArc {
        ellipse: e,
        start,
        end,
    }
}

/// The point of `e` at `s`, worked here from x = a(1 − s²) / (1 + s²), y = b · 2s / (1 + s²) along
/// its axes, in whole numbers over q(1 + s²)'s.
fn point_at(e: &Ellipse2, s: Param) -> Rational2 {
    let k = e.centre();
    let [mx, my] = e.major().map(i128::from);
    let (kx, ky) = (i128::from(k.x.get()), i128::from(k.y.get()));
    let Param::At(f) = s else {
        return Rational2::new(kx - mx, ky - my, 1).unwrap();
    };
    let (n, d) = (i128::from(f.numerator()), i128::from(f.denominator()));
    let (p, q) = e.ratio();
    let (p, q) = (i128::from(p), i128::from(q));
    let w = q * (d * d + n * n);
    let cos = q * (d * d - n * n);
    let sin = p * 2 * n * d;
    Rational2::new(
        kx * w + mx * cos - my * sin,
        ky * w + my * cos + mx * sin,
        w,
    )
    .unwrap()
}

/// The parameter angle θ of a point of `e` from where it lies, cos θ = u · M / L and
/// sin θ = q (u · M⊥) / (p L), L = ‖M‖², u the point less the centre.
fn angle_of(e: &Ellipse2, z: &Crossing2) -> f64 {
    let [ix, iy] = z.enclosure();
    let (x, y) = (
        f64::midpoint(ix.lo(), ix.hi()),
        f64::midpoint(iy.lo(), iy.hi()),
    );
    let k = e.centre();
    let (ux, uy) = (x - k.x.get() as f64, y - k.y.get() as f64);
    let [mx, my] = e.major().map(|v| v as f64);
    let (p, q) = e.ratio();
    let l = mx * mx + my * my;
    let cos = (ux * mx + uy * my) / l;
    let sin = (q as f64) * (ux * -my + uy * mx) / ((p as f64) * l);
    geom_num::math::atan2(sin, cos).unwrap().get()
}

fn angle_of_param(s: Param) -> f64 {
    match s {
        // The denominator is positive, so atan(n / d) is atan2(n, d).
        Param::At(f) => {
            2.0 * geom_num::math::atan2(f.numerator() as f64, f.denominator() as f64)
                .unwrap()
                .get()
        }
        Param::Far => std::f64::consts::PI,
    }
}

/// Whether the point at angle `t` lies on the arc counter-clockwise from `from` to `to`: `None`
/// when it is too near an end for doubles to say.
fn on_arc(t: f64, from: f64, to: f64) -> Option<bool> {
    let turn = std::f64::consts::TAU;
    let span = (to - from).rem_euclid(turn);
    let span = if span == 0.0 { turn } else { span };
    let along = (t - from).rem_euclid(turn);
    if along < 1e-9 || (along - span).abs() < 1e-9 || (turn - along) < 1e-9 {
        return None;
    }
    Some(along < span)
}

#[test]
fn an_elliptic_arc_is_cut_only_where_its_extremes_lie_inside_it() {
    // x²/16 + y²/4 = 1: s = 0 is (4, 0), s = 1 is (0, 2), s = −1 is (0, −2), far is (−4, 0).
    let e = ellipse((0, 0), [4, 0], (1, 2));
    let lies = |a: &Arrangement| -> Vec<Along> {
        (0..a.edges().len() as u32)
            .map(|i| a.along(i).unwrap())
            .collect()
    };
    // A quarter from the rightmost point up to the top: one piece on the upper half.
    let a = run(&[elliptic(e, at(0, 1), at(1, 1))]);
    laws(&a, "quarter");
    assert_eq!((a.vertices().len(), a.edges().len()), (2, 1));
    assert!(a.vertices()[1].same(&grid(4, 0)).unwrap());
    assert_eq!(
        lies(&a),
        [Along::Ellipse {
            ellipse: 0,
            upper: true
        }]
    );
    // From the top over the leftmost point to the bottom: cut there.
    let b = run(&[elliptic(e, at(1, 1), at(-1, 1))]);
    laws(&b, "left half");
    assert_eq!((b.vertices().len(), b.edges().len()), (3, 2));
    assert!(b.vertices()[0].same(&grid(-4, 0)).unwrap());
    // From the bottom over the rightmost point to the top: cut there.
    let c = run(&[elliptic(e, at(-1, 1), at(1, 1))]);
    laws(&c, "right half");
    assert_eq!((c.vertices().len(), c.edges().len()), (3, 2));
    assert!(c.vertices()[2].same(&grid(4, 0)).unwrap());
    // From the leftmost point under to the rightmost: the lower half, between two extremes.
    let d = run(&[elliptic(e, Param::Far, at(0, 1))]);
    laws(&d, "lower half");
    assert_eq!(
        lies(&d),
        [Along::Ellipse {
            ellipse: 0,
            upper: false
        }]
    );
    // Equal ends: the whole ellipse.
    let whole = run(&[elliptic(e, at(1, 2), at(1, 2))]);
    laws(&whole, "whole");
    assert_eq!(
        (
            whole.vertices().len(),
            whole.edges().len(),
            whole.faces().len()
        ),
        (2, 2, 2)
    );
    // Its two halves as arcs, and the whole ellipse: one curve, sharing both edges.
    let both = run(&[
        elliptic(e, at(1, 1), at(-1, 1)),
        elliptic(e, at(-1, 1), at(1, 1)),
        Curve::Ellipse(e),
    ]);
    laws(&both, "halves and whole");
    assert_eq!(both.ellipses().len(), 1);
    assert!(both.edges().iter().all(|edge| edge.sources.contains(&2)));
    assert_eq!(both.faces().len(), 2);
}

#[test]
fn an_elliptic_arc_s_ends_are_the_points_of_its_parameters() {
    let e = ellipse((3, -2), [5, 2], (2, 3));
    for (n, d) in [(0, 1), (1, 3), (-2, 7), (5, 4), (-1, 1)] {
        let s = at(n, d);
        let a = run(&[elliptic(e, s, Param::Far)]);
        laws(&a, "ends");
        let want = Crossing2::of_point(&point_at(&e, s)).unwrap();
        // The point is exactly on the ellipse, and is a vertex.
        assert_eq!(e.side(&point_at(&e, s)).unwrap(), Sign::Zero);
        assert!(
            a.vertices()
                .iter()
                .any(|v| v.crossing().unwrap().same(&want).unwrap())
        );
    }
    // An end past a rational point's budget is refused.
    let wide = ellipse((COORD_MAX, 0), [1 << 40, 0], (1, 1 << 20));
    let far = Param::At(Frac::new(4_294_967_295, 4_294_967_291).unwrap());
    assert!(matches!(
        try_run(&[elliptic(wide, far, at(0, 1))]),
        Err(ArrangeError::Range)
    ));
}

/// A random arc of a random ellipse, its ends at small parameters or the far end.
fn random_elliptic(rng: &mut SplitMix64, span: u64) -> Curve {
    let e = random_ellipse(rng, span);
    let param = |rng: &mut SplitMix64| {
        if rng.below(6) == 0 {
            Param::Far
        } else {
            at(rng.below(9) - 4, 1 + rng.below(4))
        }
    };
    let (start, end) = (param(rng), param(rng));
    elliptic(e, start, end)
}

/// The oracle's vertices for segments and elliptic arcs: every end, every extreme of an arc's
/// ellipse strictly inside it, and every crossing of two curves on both, each arc tested by the
/// angle of its parameter; `None` if some point is too near an arc's end for doubles to say.
fn arcs_oracle(curves: &[Curve]) -> Option<Vec<Crossing2>> {
    let mut out: Vec<Crossing2> = Vec::new();
    let add = |z: Crossing2, out: &mut Vec<Crossing2>| {
        if !out.iter().any(|o| o.same(&z).unwrap()) {
            out.push(z);
        }
    };
    let point = |q: P2| Crossing2::of_point(&q.into()).unwrap();
    // Whether `z`, a point of the arc's ellipse, is on the arc: an end exactly, or strictly inside.
    let holds = |c: &Curve, z: &Crossing2| -> Option<bool> {
        let Curve::EllipticArc {
            ellipse,
            start,
            end,
        } = c
        else {
            panic!("an elliptic arc")
        };
        for s in [*start, *end] {
            if Crossing2::of_point(&point_at(ellipse, s))
                .unwrap()
                .same(z)
                .unwrap()
            {
                return Some(true);
            }
        }
        if angle_of_param(*start) == angle_of_param(*end) {
            return Some(true);
        }
        on_arc(
            angle_of(ellipse, z),
            angle_of_param(*start),
            angle_of_param(*end),
        )
    };
    for c in curves {
        match c {
            Curve::Segment(a, b) => {
                if a != b {
                    add(point(*a), &mut out);
                    add(point(*b), &mut out);
                }
            }
            Curve::EllipticArc {
                ellipse,
                start,
                end,
            } => {
                let whole = start == end;
                if !whole {
                    add(
                        Crossing2::of_point(&point_at(ellipse, *start)).unwrap(),
                        &mut out,
                    );
                    add(
                        Crossing2::of_point(&point_at(ellipse, *end)).unwrap(),
                        &mut out,
                    );
                }
                for r in [Root::First, Root::Second] {
                    let z = extreme(ellipse, r);
                    if whole || holds(c, &z)? {
                        add(z, &mut out);
                    }
                }
            }
            _ => panic!("segments and elliptic arcs only"),
        }
    }
    for (i, c) in curves.iter().enumerate() {
        for d in &curves[i + 1..] {
            let found: Vec<Crossing2> = match (c, d) {
                (Curve::Segment(..), Curve::Segment(..)) => crossings(c, d),
                (Curve::Segment(a, b), Curve::EllipticArc { ellipse, .. })
                | (Curve::EllipticArc { ellipse, .. }, Curve::Segment(a, b)) => {
                    let arc = if matches!(c, Curve::EllipticArc { .. }) {
                        c
                    } else {
                        d
                    };
                    let mut on = vec![];
                    for z in crossings(&Curve::Segment(*a, *b), &Curve::Ellipse(*ellipse)) {
                        if holds(arc, &z)? {
                            on.push(z);
                        }
                    }
                    on
                }
                _ => vec![],
            };
            for z in found {
                add(z, &mut out);
            }
        }
    }
    Some(out)
}

#[test]
fn random_drawings_of_segments_and_elliptic_arcs_match_the_oracle_and_keep_every_law() {
    let mut rng = SplitMix64(seed() ^ 2);
    let rounds = if cfg!(debug_assertions) { 60 } else { 400 };
    let (mut checked, mut refused) = (0, 0);
    for round in 0..rounds {
        let n = 2 + rng.below(7) as usize;
        let curves: Vec<Curve> = (0..n)
            .map(|_| {
                if rng.below(3) == 0 {
                    random_elliptic(&mut rng, 12)
                } else {
                    seg(
                        (rng.below(12), rng.below(12)),
                        (rng.below(12), rng.below(12)),
                    )
                }
            })
            .collect();
        let ellipses: Vec<Ellipse2> = curves
            .iter()
            .filter_map(|c| match c {
                Curve::EllipticArc { ellipse, .. } => Some(*ellipse),
                _ => None,
            })
            .collect();
        let meet = ellipses.iter().enumerate().any(|(i, e)| {
            ellipses[i + 1..]
                .iter()
                .any(|f| e != f && ellipses_meet(e, f))
        });
        let what = format!("round {round}: {curves:?}");
        let a = match try_run(&curves) {
            Ok(a) => a,
            Err(ArrangeError::Conic) => {
                assert!(meet, "{what}: refused, but no two ellipses meet");
                refused += 1;
                continue;
            }
            Err(e) => panic!("{what}: {e:?}"),
        };
        laws(&a, &what);
        if meet {
            continue;
        }
        let Some(want) = arcs_oracle(&curves) else {
            continue;
        };
        checked += 1;
        assert_eq!(a.vertices().len(), want.len(), "{what}: the vertex count");
        for v in a.vertices() {
            let z = v.crossing().unwrap();
            assert!(
                want.iter().any(|w| w.same(&z).unwrap()),
                "{what}: {v:?} is the oracle's"
            );
        }
    }
    assert!(checked > rounds / 3, "{checked} checked, {refused} refused");
}
