//! Attacks on piece G196a's arcs: the arrangement of segments and circular arcs
//! (`geom_2d::arrange::arrange_curves`; chapter 14.4 of the kernel architecture). Every test tries
//! to break a claim: arcs are cut where their circles are leftmost and rightmost, every crossing of
//! an arc with a line or another arc is found exactly, a tangency is one touching vertex, arcs of
//! one circle share the edges of their common stretch, edges round a vertex are ordered by tangent
//! and then curvature, holes are placed inside circles, and the input order does not matter.
//!
//! - **Shapes**: a circle, a circle about a rational centre (cut at two one-root points), a circle
//!   and a line, two crossing circles, circles touching outside and inside, a line touching a
//!   circle, concentric circles, overlapping arcs, a chord and its arc, a segment of no length.
//! - **Vertices are exact**: on random drawings of segments and arcs, the vertices are exactly an
//!   independent oracle's (every end, every cut strictly inside an arc, every pairwise crossing on
//!   both curves, found pair by pair with `holds_crossing` rather than by a sweep), compared by the
//!   exact `same`.
//! - **The graph is planar and complete**: Euler's formula, every half-edge on one cycle, every
//!   clockwise cycle a hole of exactly one face.
//! - **Permutation-invariant**: shuffling the input changes only the source labels.
//! - **Extremes**: arcs at the frame's edge about centres of the largest weights.
//!
//! Seeds: the random tests print `seed: <hex>` and run from a fixed default; `SPAREKIT_SEED=<hex>`
//! replays another (chapter 28.17).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names
)]

use geom_2d::arrange::{Along, Arrangement, At, BodyError, Curve, arrange_curves};
use geom_num::arc::{Arc2, Contact, Disc};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::one_root::{Crossing2, Root};
use geom_num::{NumError, Sign};
use geom_run::{Budget, WorkMeter};

/// The default seed of the random tests here.
const DEFAULT_SEED: u64 = 0x6196_a0a7_c5ee_d196;

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

/// The arc counter-clockwise from `a` to `b` about the grid point `c`.
fn arc(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> Curve {
    Curve::Arc(Arc2::new(p(a.0, a.1), p(b.0, b.1), p(c.0, c.1).into()).unwrap())
}

/// A whole circle about a grid centre of whole radius, as its two halves.
fn circle(cx: i64, cy: i64, r: i64) -> [Curve; 2] {
    [
        arc((cx + r, cy), (cx - r, cy), (cx, cy)),
        arc((cx - r, cy), (cx + r, cy), (cx, cy)),
    ]
}

fn run(curves: &[Curve]) -> Arrangement {
    arrange_curves(curves, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap()
}

fn grid(x: i64, y: i64) -> At {
    At::Rational(p(x, y).into())
}

/// The rank of the vertex at the grid point (x, y).
fn rank(a: &Arrangement, x: i64, y: i64) -> u32 {
    let at = grid(x, y);
    a.vertices()
        .iter()
        .position(|v| v.same(&at).unwrap())
        .unwrap_or_else(|| panic!("no vertex at ({x}, {y})")) as u32
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

#[test]
fn a_circle_is_two_edges_between_its_leftmost_and_rightmost_points() {
    let a = run(&circle(0, 0, 5));
    laws(&a, "circle");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (2, 2, 2)
    );
    assert_eq!(a.circles().len(), 1);
    assert_eq!((rank(&a, -5, 0), rank(&a, 5, 0)), (0, 1));
    // Between the same two vertices, in the order they leave the left one: down first.
    assert_eq!(
        a.along(0),
        Some(Along::Arc {
            circle: 0,
            upper: false
        })
    );
    assert_eq!(
        a.along(1),
        Some(Along::Arc {
            circle: 0,
            upper: true
        })
    );
    assert_eq!(a.edges()[0].sources, [1]);
    assert_eq!(a.edges()[1].sources, [0]);
    // Arcs are not stored in a body: the first arc edge is named.
    assert!(matches!(
        a.to_body(&mut WorkMeter::new(Budget::UNLIMITED)),
        Err(BodyError::Curved { edge: 0 })
    ));
}

#[test]
fn a_circle_about_a_rational_centre_is_cut_at_two_one_root_points() {
    // The circle about (1, 1/2) through (0, 0) and (2, 0): the short arc below the chord and the
    // long one above it, which holds the leftmost and rightmost points (1 -+ sqrt(5/4), 1/2).
    let centre = Rational2::new(2, 1, 2).unwrap();
    let below = Curve::Arc(Arc2::new(p(0, 0), p(2, 0), centre).unwrap());
    let above = Curve::Arc(Arc2::new(p(2, 0), p(0, 0), centre).unwrap());
    let a = run(&[below, above]);
    laws(&a, "rational circle");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (4, 4, 2)
    );
    let roots: Vec<usize> = (0..4)
        .filter(|&i| a.vertices()[i].rational().is_none())
        .collect();
    assert_eq!(
        roots,
        [0, 3],
        "the leftmost point comes first and the rightmost last"
    );
    let disc = Disc::of_round(&a.circles()[0]).unwrap();
    for (i, r) in [(0, Root::First), (3, Root::Second)] {
        let want = At::Root(Box::new(Crossing2::of_extreme(&disc, r).unwrap()));
        assert!(a.vertices()[i].same(&want).unwrap());
    }
    // Every edge names the arc it lies on; the long arc has three pieces.
    let mut from_above = 0;
    for e in a.edges() {
        if e.sources == [1] {
            from_above += 1;
        }
    }
    assert_eq!(from_above, 3);
}

#[test]
fn a_line_across_a_circle_splits_both_and_shares_its_two_crossings() {
    let mut curves = circle(0, 0, 5).to_vec();
    curves.push(seg((-10, 3), (10, 3)));
    let a = run(&curves);
    laws(&a, "circle and line");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (6, 7, 3)
    );
    // The crossings are grid points, found as crossings of the line and the circle.
    let (l, r) = (rank(&a, -4, 3), rank(&a, 4, 3));
    let between: Vec<usize> = (0..a.edges().len())
        .filter(|&i| a.edges()[i].from == l && a.edges()[i].to == r)
        .collect();
    assert_eq!(between.len(), 2, "the chord and the arc above it");
    // The chord leaves (-4, 3) eastward, the arc up and to the right: the chord first.
    assert!(matches!(a.along(between[0] as u32), Some(Along::Line(_))));
    assert_eq!(
        a.along(between[1] as u32),
        Some(Along::Arc {
            circle: 0,
            upper: true
        })
    );
}

#[test]
fn two_crossing_circles_make_a_lens_and_two_crescents() {
    let mut curves = circle(0, 0, 5).to_vec();
    curves.extend(circle(6, 0, 5));
    let a = run(&curves);
    laws(&a, "two circles");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (6, 8, 4)
    );
    rank(&a, 3, 4);
    rank(&a, 3, -4);
    assert_eq!(a.circles().len(), 2);
    for f in &a.faces()[1..] {
        assert_eq!(f.holes, Vec::<u32>::new());
    }
}

#[test]
fn circles_touching_outside_meet_at_one_vertex() {
    let mut curves = circle(0, 0, 5).to_vec();
    curves.extend(circle(10, 0, 5));
    let a = run(&curves);
    laws(&a, "touching outside");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (3, 4, 3)
    );
    assert_eq!(a.around(rank(&a, 5, 0)).len(), 4);
}

/// Two circles of radius near 2^38.5 about centres √2 apart cross a hair's breadth off the line of
/// their centres, far out along the diagonal. The first is about the origin through (a, a); the
/// second about (1, 1) through (a + 1, a − 1), so its radius squared is 2(a − 1)² + 2 and it meets
/// the first (2a − 1)/√2 along the diagonal and √((4a − 1)/2), about 2^19.5, off it. There the turn
/// of the crossing and the two centres is about 2^20, and the certified intervals of its two
/// products, each about 2^76, are some 2^25 wide: only the exact sign orders the arcs leaving each
/// crossing. The four faces and Euler's formula hold.
#[test]
fn circles_crossing_beside_the_line_of_their_centres_are_ordered_exactly() {
    let a = 1_i64 << 38;
    let mut curves = vec![arc((a, a), (-a, -a), (0, 0)), arc((-a, -a), (a, a), (0, 0))];
    curves.push(arc((a + 1, a - 1), (a - 1, a + 1), (1, 1)));
    curves.push(arc((a - 1, a + 1), (a + 1, a - 1), (1, 1)));
    let got = run(&curves);
    laws(&got, "crossing beside the centres' line");
    assert_eq!(
        (got.vertices().len(), got.edges().len(), got.faces().len()),
        (10, 12, 4)
    );
}

#[test]
fn circles_touching_inside_are_ordered_round_the_touch_by_curvature() {
    let mut curves = circle(0, 0, 5).to_vec();
    curves.extend(circle(2, 0, 3));
    let a = run(&curves);
    laws(&a, "touching inside");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (3, 4, 3)
    );
    // Leaving (5, 0): both lower halves head down and bend west, the smaller sooner; both upper
    // halves head up and bend west, the smaller sooner. Counter-clockwise from west: the small
    // lower, the large lower, the large upper, the small upper.
    let touch = rank(&a, 5, 0);
    let got: Vec<Option<Along>> = a.around(touch).iter().map(|&h| a.along(h / 2)).collect();
    let (large, small) = (0, 1);
    assert_eq!(
        got,
        [
            Some(Along::Arc {
                circle: small,
                upper: false
            }),
            Some(Along::Arc {
                circle: large,
                upper: false
            }),
            Some(Along::Arc {
                circle: large,
                upper: true
            }),
            Some(Along::Arc {
                circle: small,
                upper: true
            }),
        ]
    );
    // The crescent is one cycle pinched at the touch; no bounded face has a hole.
    assert!(a.faces()[1..].iter().all(|f| f.holes.is_empty()));
}

#[test]
fn a_line_touching_a_circle_is_one_touching_vertex() {
    let mut curves = circle(0, 0, 5).to_vec();
    curves.push(seg((-10, 5), (10, 5)));
    let a = run(&curves);
    laws(&a, "tangent line");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (5, 5, 2)
    );
    let top = rank(&a, 0, 5);
    assert_eq!(a.around(top).len(), 4);
}

#[test]
fn concentric_circles_make_a_ring_with_a_hole() {
    let mut curves = circle(0, 0, 10).to_vec();
    curves.extend(circle(0, 0, 3));
    // And a square inside the small circle, and a circle inside a square beside them.
    curves.extend([
        seg((-1, -1), (1, -1)),
        seg((1, -1), (1, 1)),
        seg((1, 1), (-1, 1)),
        seg((-1, 1), (-1, -1)),
    ]);
    curves.extend([
        seg((20, -5), (30, -5)),
        seg((30, -5), (30, 5)),
        seg((30, 5), (20, 5)),
        seg((20, 5), (20, -5)),
    ]);
    curves.extend(circle(25, 0, 2));
    let a = run(&curves);
    laws(&a, "nested");
    // Unbounded, big ring, small ring, square inside the small one, square ring, circle in it.
    assert_eq!(a.faces().len(), 6);
    let holes: usize = a
        .faces()
        .iter()
        .map(|f| f.holes.len())
        .fold(0, usize::saturating_add);
    // The unbounded face holds the two outside boundaries; three faces hold one hole each.
    assert_eq!(holes, 5);
    assert_eq!(a.faces()[0].holes.len(), 2);
    let with_one = a.faces()[1..].iter().filter(|f| f.holes.len() == 1).count();
    assert_eq!(with_one, 3);
}

#[test]
fn arcs_of_one_circle_share_their_common_stretch() {
    // Two arcs of the circle of radius 5: (5, 0) to (0, 5), and (3, 4) to (-5, 0).
    let a = run(&[arc((5, 0), (0, 5), (0, 0)), arc((3, 4), (-5, 0), (0, 0))]);
    laws(&a, "overlap");
    assert_eq!((a.vertices().len(), a.edges().len()), (4, 3));
    let (top, side) = (rank(&a, 0, 5), rank(&a, 3, 4));
    let shared = a
        .edges()
        .iter()
        .find(|e| e.from == top && e.to == side)
        .unwrap();
    assert_eq!(shared.sources, [0, 1]);
    assert_eq!(a.circles().len(), 1);
}

#[test]
fn a_chord_and_its_arc_bound_a_face() {
    let a = run(&[arc((5, 0), (0, 5), (0, 0)), seg((0, 5), (5, 0))]);
    laws(&a, "segment of a circle");
    assert_eq!(
        (a.vertices().len(), a.edges().len(), a.faces().len()),
        (2, 2, 2)
    );
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

fn random_drawing(rng: &mut SplitMix64, n: usize, span: u64) -> Vec<Curve> {
    (0..n)
        .map(|_| {
            if rng.below(2) == 0 {
                random_arc(rng, span)
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

/// The oracle's vertices, pair by pair: every end, every leftmost or rightmost point strictly
/// inside an arc, and every crossing of two curves that lies on both. A segment of no length is
/// ignored, as `Curve::Segment` says, so its point is a vertex only if another curve makes it one.
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
            Curve::Ellipse(_) | Curve::EllipticArc { .. } => {
                panic!("these drawings hold no ellipse: attack_g196b_ellipses")
            }
        }
    }
    for (i, c) in curves.iter().enumerate() {
        for d in &curves[i + 1..] {
            let found: Vec<Crossing2> = match (c, d) {
                (Curve::Segment(a, b), Curve::Segment(e, f)) => {
                    if a == b || e == f {
                        continue;
                    }
                    let (l, m) = (
                        Carrier::through(*a, *b).unwrap(),
                        Carrier::through(*e, *f).unwrap(),
                    );
                    if l.parallel(&m).unwrap() {
                        continue;
                    }
                    let z = Crossing2::of_point(&l.meet(&m).unwrap()).unwrap();
                    if within(&z, *a, *b) && within(&z, *e, *f) {
                        vec![z]
                    } else {
                        vec![]
                    }
                }
                (Curve::Segment(a, b), Curve::Arc(arc))
                | (Curve::Arc(arc), Curve::Segment(a, b)) => {
                    if a == b {
                        continue;
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
                (Curve::Ellipse(_) | Curve::EllipticArc { .. }, _)
                | (_, Curve::Ellipse(_) | Curve::EllipticArc { .. }) => {
                    panic!("these drawings hold no ellipse: attack_g196b_ellipses")
                }
            };
            for z in found {
                add(z, &mut out);
            }
        }
    }
    out
}

/// Round 62 of the random drawings in a release build: a segment of no length at a point no other
/// curve passes is no vertex, and leaves the arrangement as it is without it.
#[test]
fn a_segment_of_no_length_is_ignored() {
    let centre = Rational2::new(9, 20, 4).unwrap();
    let with = [
        seg((7, 4), (0, 6)),
        seg((1, 0), (1, 0)),
        Curve::Arc(Arc2::new(p(0, 0), p(6, 1), centre).unwrap()),
        seg((3, 6), (2, 4)),
        seg((5, 3), (5, 2)),
    ];
    let without: Vec<Curve> = [0, 2, 3, 4].iter().map(|&i| with[i]).collect();
    let (a, b) = (run(&with), run(&without));
    laws(&a, "with the point");
    assert_eq!(a.vertices().len(), oracle(&with).len());
    assert!(
        a.vertices().iter().all(|v| !v.same(&grid(1, 0)).unwrap()),
        "the point is no vertex"
    );
    assert_eq!(a.vertices().len(), b.vertices().len());
    for (u, v) in a.vertices().iter().zip(b.vertices()) {
        assert!(u.same(v).unwrap());
    }
    let ends =
        |x: &Arrangement| -> Vec<(u32, u32)> { x.edges().iter().map(|e| (e.from, e.to)).collect() };
    assert_eq!(ends(&a), ends(&b));
    assert_eq!(a.faces(), b.faces());
}

#[test]
fn random_drawings_match_the_pairwise_oracle_and_keep_every_law() {
    let mut rng = SplitMix64(seed());
    let rounds = if cfg!(debug_assertions) { 60 } else { 400 };
    for round in 0..rounds {
        let n = 2 + rng.below(9) as usize;
        let curves = random_drawing(&mut rng, n, 9);
        let a = run(&curves);
        let what = format!("round {round}: {curves:?}");
        laws(&a, &what);
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
}

#[test]
fn the_arrangement_of_arcs_does_not_depend_on_the_input_order() {
    let mut rng = SplitMix64(seed() ^ 1);
    let rounds = if cfg!(debug_assertions) { 30 } else { 200 };
    for round in 0..rounds {
        let n = 3 + rng.below(7) as usize;
        let curves = random_drawing(&mut rng, n, 8);
        let a = run(&curves);
        let mut order: Vec<usize> = (0..curves.len()).collect();
        for i in (1..order.len()).rev() {
            let j = rng.below(i as u64 + 1) as usize;
            order.swap(i, j);
        }
        let shuffled: Vec<Curve> = order.iter().map(|&i| curves[i]).collect();
        let b = run(&shuffled);
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
fn arcs_at_the_frame_s_edge_about_the_heaviest_centres_stay_exact() {
    let m = COORD_MAX;
    // Centres on the bisector x = 0 of (-m, 0) and (m, 0), with weights near 2^82, either side.
    let w: i128 = (1 << 82) - 1;
    let mut curves = Vec::new();
    for (y, flip) in [((1_i128 << 121) + 1, false), (-(1_i128 << 121) - 3, true)] {
        let centre = Rational2::new(0, y, w).unwrap();
        let (s, e) = if flip {
            (p(m, 0), p(-m, 0))
        } else {
            (p(-m, 0), p(m, 0))
        };
        curves.push(Curve::Arc(Arc2::new(s, e, centre).unwrap()));
        curves.push(Curve::Arc(Arc2::new(e, s, centre).unwrap()));
    }
    curves.push(seg((-m, -m), (m, m)));
    curves.push(seg((-m, m), (m, -m)));
    curves.push(seg((-m, 1), (m, 1)));
    let a = run(&curves);
    laws(&a, "extremes");
    assert_eq!(a.circles().len(), 2);
    let want = oracle(&curves);
    assert_eq!(a.vertices().len(), want.len());
}
