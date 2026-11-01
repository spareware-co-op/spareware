//! Attacks on piece G193: tangent constructions (`geom_2d::tangent`; chapter 14.3 of the kernel
//! architecture). Every test tries to break a claim of the piece's done-when or of the module's
//! guarantees.
//!
//! - **Every join an exact shared point**: read back through [`Pt2::on`] against both its carriers,
//!   for every pairing of line, circle and ellipse.
//! - **Exact for rational normals and parallels**: exactly tangent, gap exactly zero.
//! - **Within ε_ang**: every Certified gap at most the ladder's angular bound, at every scale.
//! - **No extra contact**: the other crossing of each neighbour lies off the arc.
//! - **Accepted, atomic**: a refusal says what it would reach, and adds nothing to the table.
//! - **Every corner**: 100,000 corners of every turn, the same on any number of threads.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Case counts are a twentieth in debug
//! builds.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_2d::accept::Accept;
use geom_2d::carrier::{Carriers, Circle2, Curve2, CurveIx, Line2};
use geom_2d::point::{Place, PointError, Pt2};
use geom_2d::tangent::{
    Corner, Fillet, TangentLine, Touch, common_tangent, fillet, fillet_all, semicircle,
    tangent_circle, tangent_from,
};
use geom_num::arc::{Circle2 as GridCircle, Contact, Disc};
use geom_num::conic::Ellipse2;
use geom_num::grid::{COORD_MAX, GridStep, P2, Steps};
use geom_num::ladder::ANGULAR;
use geom_num::one_root::{Crossing2, Root};
use geom_num::{Exactness, NumError, Sign};
use geom_run::{Budget, Exec, WorkMeter};

const M: i64 = COORD_MAX;

/// The default seed of every randomised test here (G193).
const DEFAULT_SEED: u64 = 0x6719_3000_5eed_0193;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), written out as the other tests do.
struct Rng(u64);

impl Rng {
    fn new() -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("seed: {seed:x}");
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A whole number of magnitude up to a random power of two below 2^`bits`.
    fn sized(&mut self, bits: u64) -> i64 {
        let k = self.next() % bits;
        self.up_to(1 << k)
    }

    /// A whole number of magnitude up to `limit`.
    fn up_to(&mut self, limit: i64) -> i64 {
        (self.next() % (2 * limit as u64 + 1)) as i64 - limit
    }
}

fn p2(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn line(a: P2, b: P2) -> Curve2 {
    Curve2::Line(Line2::through(a, b).unwrap())
}

fn circle(c: P2, r: i128) -> Curve2 {
    Curve2::Circle(Circle2::Grid(GridCircle::new(c, r).unwrap()))
}

/// What an interactive caller accepts: the ladder's angle and a step.
fn certified() -> Accept {
    Accept::Certified {
        pos: Steps(1),
        step: GridStep::Q0,
        gap: ANGULAR,
    }
}

/// `curves` in a table, and their names.
fn table<const N: usize>(curves: [Curve2; N]) -> (Carriers, [CurveIx; N]) {
    let mut carriers = Carriers::new();
    let names = curves.map(|c| carriers.push(c).unwrap());
    (carriers, names)
}

/// The fillet's circle as a `Disc`.
fn disc_of(carriers: &Carriers, at: CurveIx) -> Disc {
    match carriers.get(at) {
        Some(Curve2::Circle(c)) => c.disc().unwrap(),
        other => panic!("{other:?}"),
    }
}

/// `point` is exactly on both.
fn joined(carriers: &Carriers, point: &Pt2, a: CurveIx, b: CurveIx) {
    assert!(point.on(carriers, a).unwrap(), "{point:?}");
    assert!(point.on(carriers, b).unwrap(), "{point:?}");
}

/// `done` is what it says.
fn sound(carriers: &Carriers, done: &Fillet, first: CurveIx, second: CurveIx) {
    joined(carriers, &done.start, first, done.circle);
    joined(carriers, &done.end, second, done.circle);
    assert!(done.gap.get() <= ANGULAR.get(), "{}", done.gap.get());
}

/// The left turn from (0, 0) to (s, 0) on to `to`: line one, line two, and the touches of a fillet.
fn left_corner(s: i64, to: P2) -> (Carriers, [CurveIx; 2], [Touch; 2]) {
    let corner = p2(s, 0);
    let (carriers, names) = table([line(p2(0, 0), corner), line(corner, to)]);
    let touch = |curve| Touch {
        curve,
        side: Sign::Positive,
        near: corner,
    };
    (carriers, names, [touch(names[0]), touch(names[1])])
}

#[test]
fn a_fillet_of_rational_normals_is_exactly_tangent() {
    // Normals (0, 1) and (4, 3), of whole lengths 1 and 5.
    let (mut carriers, [a, b], [first, second]) = left_corner(1_000, p2(1_300, 400));
    let accept = Accept::Exact;
    let done = fillet(&mut carriers, &first, &second, 50, GridStep::Q0, &accept).unwrap();
    assert_eq!(done.exactness, Exactness::Exact);
    assert_eq!(done.gap.get(), 0.0);
    sound(&carriers, &done, a, b);
    let disc = disc_of(&carriers, done.circle);
    for name in [a, b] {
        let Some(Curve2::Line(l)) = carriers.get(name) else {
            panic!()
        };
        assert_eq!(disc.meets(l).unwrap(), Contact::Tangent);
        assert_eq!(l.side(&disc.centre()).unwrap(), Sign::Positive);
    }
}

#[test]
fn every_rational_corner_is_exact_in_every_turn() {
    let turns = [p2(1_000, 500), p2(1_000, -500), p2(700, 400), p2(1_300, 400)];
    for to in turns {
        let corner = p2(1_000, 0);
        let (mut carriers, [a, b]) = table([line(p2(0, 0), corner), line(corner, to)]);
        let Some(Curve2::Line(l)) = carriers.get(a) else {
            panic!()
        };
        let side = l.side(&to.into()).unwrap();
        let touch = |curve| Touch {
            curve,
            side,
            near: corner,
        };
        let (from, into) = if side == Sign::Positive {
            (touch(a), touch(b))
        } else {
            (touch(b), touch(a))
        };
        let done = fillet(&mut carriers, &from, &into, 40, GridStep::Q1, &Accept::Exact).unwrap();
        assert_eq!(done.exactness, Exactness::Exact);
        sound(&carriers, &done, from.curve, into.curve);
    }
}

#[test]
fn a_fillet_of_other_lines_goes_a_hair_past_them_and_keeps_its_radius() {
    // Normals (1, 0) and (2, 3): the second has no whole length.
    let mut rng = Rng::new();
    let cases = if cfg!(debug_assertions) { 40 } else { 800 };
    for _ in 0..cases {
        let s = 1 + (rng.next() % (1 << 38)) as i64;
        let to = p2(s + 1 + (rng.next() % 1_000) as i64, 1 + (rng.next() % 1_000) as i64);
        let bits = rng.next() % 16;
        let r = i128::from(1 + (rng.next() % (1 << bits)) as i64);
        let (mut carriers, [a, b], [first, second]) = left_corner(s, to);
        let Ok(done) = fillet(&mut carriers, &first, &second, r, GridStep::Q0, &certified()) else {
            panic!("{s} {to:?} {r}");
        };
        sound(&carriers, &done, a, b);
        let disc = disc_of(&carriers, done.circle);
        let (_, w) = disc.centre().coordinates();
        let whole = geom_num::I256::from_i128(r * w);
        assert_eq!(disc.squared_radius(), whole.checked_mul(&whole).unwrap());
        for name in [a, b] {
            let Some(Curve2::Line(l)) = carriers.get(name) else {
                panic!()
            };
            if done.gap.get() > 0.0 {
                assert_eq!(disc.meets(l).unwrap(), Contact::Crossing);
            }
        }
    }
}

#[test]
fn neither_cap_is_on_the_arc() {
    let (mut carriers, [a, b], [first, second]) = left_corner(10_000, p2(10_301, 703));
    let done = fillet(&mut carriers, &first, &second, 900, GridStep::Q0, &certified()).unwrap();
    assert!(matches!(done.exactness, Exactness::Certified { .. }));
    let disc = disc_of(&carriers, done.circle);
    let (Some(Curve2::Line(l)), Some(Curve2::Line(m))) = (carriers.get(a), carriers.get(b)) else {
        panic!()
    };
    let at = |p: &Pt2| {
        let Place::OneRoot(z) = p.place(&carriers).unwrap() else {
            panic!()
        };
        z
    };
    // The centre is on the left of both, so the arc leaves the first line at its later crossing and
    // reaches the second at its earlier: the others are behind and ahead of them along the lines.
    let (start, end) = (at(&done.start), at(&done.end));
    let behind = Crossing2::of_disc(l, &disc, Root::First).unwrap();
    assert_eq!(start.order(l, &behind).unwrap(), Sign::Negative);
    let ahead = Crossing2::of_disc(m, &disc, Root::Second).unwrap();
    assert_eq!(end.order(m, &ahead).unwrap(), Sign::Positive);
    // The arc runs from the first line round the side of the corner: the corner is outside it, and
    // the way from start to end turns left about the centre.
    let centre = Crossing2::of_point(&disc.centre()).unwrap();
    assert_eq!(start.side(m).unwrap(), Sign::Positive);
    assert_eq!(end.side(l).unwrap(), Sign::Positive);
    assert_eq!(centre.side(l).unwrap(), Sign::Positive);
}

#[test]
fn a_semicircle_joins_parallels_exactly_every_way() {
    for (b0, b1, at) in [
        (p2(0, 100), p2(1_000, 100), p2(500, 0)),
        (p2(0, 101), p2(1_000, 101), p2(500, 0)),
        (p2(1_000, 100), p2(0, 100), p2(1, 0)),
    ] {
        let (mut carriers, [a, b]) = table([line(p2(0, 0), p2(1_000, 0)), line(b0, b1)]);
        let done = semicircle(&mut carriers, a, b, at).unwrap();
        assert_eq!(done.exactness, Exactness::Exact);
        assert_eq!(done.gap.get(), 0.0);
        joined(&carriers, &done.start, a, done.circle);
        joined(&carriers, &done.end, b, done.circle);
        let disc = disc_of(&carriers, done.circle);
        for name in [a, b] {
            let Some(Curve2::Line(l)) = carriers.get(name) else {
                panic!()
            };
            assert_eq!(disc.meets(l).unwrap(), Contact::Tangent);
        }
    }
    // A slanted pair of lines, of different multiples.
    let (mut carriers, [a, b]) = table([
        Curve2::Line(Line2::new(1, 2, 0).unwrap()),
        Curve2::Line(Line2::new(3, 6, 95).unwrap()),
    ]);
    let done = semicircle(&mut carriers, a, b, p2(-20, 10)).unwrap();
    joined(&carriers, &done.end, b, done.circle);
}

#[test]
fn a_semicircle_refuses_what_is_not_one() {
    let (mut carriers, [a, b, c]) = table([
        line(p2(0, 0), p2(10, 0)),
        line(p2(0, 1), p2(10, 2)),
        line(p2(3, 0), p2(7, 0)),
    ]);
    let refused = PointError::Num(NumError::Degenerate);
    assert_eq!(semicircle(&mut carriers, a, b, p2(0, 0)).err(), Some(refused));
    assert_eq!(semicircle(&mut carriers, a, c, p2(0, 0)).err(), Some(refused));
    let (mut carriers, [d, e]) = table([line(p2(0, 0), p2(10, 0)), line(p2(0, 5), p2(10, 5))]);
    assert_eq!(semicircle(&mut carriers, d, e, p2(1, 1)).err(), Some(refused));
    assert_eq!(carriers.len(), 2);
}

#[test]
fn a_fillet_of_a_line_and_a_circle_each_way_round() {
    let (mut carriers, [ground, wheel]) = table([
        line(p2(-100_000, 0), p2(100_000, 0)),
        circle(p2(0, 40_000), 30_000),
    ]);
    // Outside the wheel, above the ground, touching it to the right of the wheel's foot.
    let first = Touch {
        curve: wheel,
        side: Sign::Positive,
        near: p2(16_000, 16_000),
    };
    let second = Touch {
        curve: ground,
        side: Sign::Positive,
        near: p2(20_000, 0),
    };
    let done = fillet(&mut carriers, &first, &second, 8_000, GridStep::Q0, &certified());
    let done = done.unwrap();
    assert!(matches!(done.exactness, Exactness::Certified { .. }));
    sound(&carriers, &done, wheel, ground);
    // And inside it, against a chord of it.
    let (mut carriers, [chord, wheel]) = table([
        line(p2(-100_000, 10_000), p2(100_000, 10_000)),
        circle(p2(0, 0), 30_000),
    ]);
    let inside = Touch {
        curve: wheel,
        side: Sign::Negative,
        near: p2(26_000, 14_500),
    };
    let above = Touch {
        curve: chord,
        side: Sign::Positive,
        near: p2(23_600, 10_000),
    };
    let done = fillet(&mut carriers, &above, &inside, 3_000, GridStep::Q0, &certified());
    let done = done.unwrap();
    sound(&carriers, &done, chord, wheel);
}

#[test]
fn a_fillet_of_two_circles() {
    let (mut carriers, [left, right]) = table([
        circle(p2(0, 0), 3_000),
        circle(p2(8_000, 1_000), 4_000),
    ]);
    let touch = |curve, near| Touch {
        curve,
        side: Sign::Positive,
        near,
    };
    let (a, b) = (
        touch(left, p2(2_000, 2_200)),
        touch(right, p2(5_000, 3_600)),
    );
    let done = fillet(&mut carriers, &a, &b, 2_000, GridStep::Q0, &certified()).unwrap();
    sound(&carriers, &done, left, right);
}

#[test]
fn a_fillet_of_a_line_and_an_ellipse_joins_at_a_root() {
    let (mut carriers, [ground, oval]) = table([
        line(p2(-100_000, -2_000), p2(100_000, -2_000)),
        Curve2::Ellipse(Ellipse2::new(p2(0, 4_000), [20_000, 0], 3, 5).unwrap()),
    ]);
    let (over, out) = (
        Touch {
            curve: ground,
            side: Sign::Positive,
            near: p2(20_000, -2_000),
        },
        Touch {
            curve: oval,
            side: Sign::Positive,
            near: p2(16_000, -3_000),
        },
    );
    let done = fillet(&mut carriers, &over, &out, 2_000, GridStep::Q0, &certified()).unwrap();
    assert!(matches!(done.end, Pt2::ConicRoot { .. }), "{:?}", done.end);
    sound(&carriers, &done, ground, oval);
}

#[test]
fn a_circle_touching_three_curves() {
    let (mut carriers, [base, up, down]) = table([
        line(p2(0, 0), p2(60_000, 0)),
        line(p2(30_000, 40_000), p2(0, 0)),
        line(p2(60_000, 0), p2(30_000, 40_000)),
    ]);
    let touch = |curve, near| Touch {
        curve,
        side: Sign::Positive,
        near,
    };
    let touches = [
        touch(base, p2(30_000, 0)),
        touch(up, p2(12_000, 16_000)),
        touch(down, p2(48_000, 16_000)),
    ];
    let done = tangent_circle(&mut carriers, &touches, GridStep::Q0, &certified()).unwrap();
    for (join, name) in done.joins.iter().zip([base, up, down]) {
        joined(&carriers, &join.leaving, name, done.circle);
        joined(&carriers, &join.arriving, name, done.circle);
    }
    assert!(done.gap.get() <= ANGULAR.get());
    let disc = disc_of(&carriers, done.circle);
    let ([x, y], w) = disc.centre().coordinates();
    assert!(((x as f64 / w as f64) - 30_000.0).abs() < 1e-3);
    assert!(((y as f64 / w as f64) - 15_000.0).abs() < 1e-3);
}

#[test]
fn a_line_to_a_circle_ends_on_it_past_the_touch() {
    let mut rng = Rng::new();
    let cases = if cfg!(debug_assertions) { 40 } else { 800 };
    for _ in 0..cases {
        let far = 1 << (10 + rng.next() % 28);
        let from = p2(rng.up_to(far), rng.up_to(far));
        let centre = p2(rng.up_to(far), rng.up_to(far));
        let r = 1 + i128::from(rng.sized(20).abs());
        let (mut carriers, [wheel]) = table([circle(centre, r)]);
        let side = if rng.next() & 1 == 0 {
            Sign::Positive
        } else {
            Sign::Negative
        };
        let result = tangent_from(&mut carriers, from, wheel, side, GridStep::Q0, &certified());
        let disc = disc_of(&carriers, wheel);
        match disc.side(&from.into()).unwrap() {
            Sign::Negative => {
                assert_eq!(result.err(), Some(PointError::Num(NumError::Degenerate)));
                continue;
            }
            Sign::Zero | Sign::Positive => {}
        }
        let done: TangentLine = result.unwrap();
        joined(&carriers, &done.end, done.line, wheel);
        joined(&carriers, &done.start, done.line, done.line);
        assert!(done.gap.get() <= ANGULAR.get(), "{}", done.gap.get());
        assert_eq!(
            carriers.get(done.line).unwrap().side(&disc.centre()).unwrap(),
            side
        );
    }
}

#[test]
fn a_point_on_a_circle_has_the_exact_tangent() {
    let (mut carriers, [wheel]) = table([circle(p2(0, 0), 5)]);
    let done = tangent_from(
        &mut carriers,
        p2(3, 4),
        wheel,
        Sign::Positive,
        GridStep::Q0,
        &Accept::Exact,
    )
    .unwrap();
    assert_eq!(done.exactness, Exactness::Exact);
    let disc = disc_of(&carriers, wheel);
    let Some(Curve2::Line(l)) = carriers.get(done.line) else {
        panic!()
    };
    assert_eq!(disc.meets(l).unwrap(), Contact::Tangent);
    assert_eq!(l.side(&disc.centre()).unwrap(), Sign::Positive);
}

#[test]
fn a_small_circle_far_away_takes_a_fine_line() {
    let (mut carriers, [wheel]) = table([circle(p2(1 << 39, 1 << 38), 1_000)]);
    let done = tangent_from(
        &mut carriers,
        p2(-(1 << 39), -(1 << 38)),
        wheel,
        Sign::Positive,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
    assert!(matches!(carriers.get(done.line), Some(Curve2::Fine(_))));
    joined(&carriers, &done.end, done.line, wheel);
    assert!(done.gap.get() <= ANGULAR.get(), "{}", done.gap.get());
}

#[test]
fn common_tangents_of_every_kind_at_every_scale() {
    let mut rng = Rng::new();
    let cases = if cfg!(debug_assertions) { 40 } else { 800 };
    let mut fine = 0;
    for _ in 0..cases {
        let far = 1_i64 << (16 + rng.next() % 22);
        let (centre, other) = (p2(rng.up_to(far), rng.up_to(far)), p2(rng.up_to(far), rng.up_to(far)));
        let r = 8 + i128::from(rng.sized(15).abs());
        let s = 8 + i128::from(rng.sized(15).abs());
        let (mut carriers, [one, two]) = table([circle(centre, r), circle(other, s)]);
        let sides = match rng.next() % 4 {
            0 => [Sign::Positive, Sign::Positive],
            1 => [Sign::Negative, Sign::Negative],
            2 => [Sign::Positive, Sign::Negative],
            _ => [Sign::Negative, Sign::Positive],
        };
        // Two circles that overlap, or one inside the other, have fewer tangents.
        let apart = |p: P2, q: P2| {
            let (dx, dy) = (
                i128::from(p.x.get() - q.x.get()),
                i128::from(p.y.get() - q.y.get()),
            );
            dx * dx + dy * dy
        };
        if apart(centre, other) <= (r + s) * (r + s) {
            continue;
        }
        let Ok(done) =
            common_tangent(&mut carriers, one, two, sides, GridStep::Q0, &certified())
        else {
            let why = common_tangent(&mut carriers, one, two, sides, GridStep::Q0, &certified());
            panic!("{centre:?} {other:?} {r} {s} {sides:?}: {:?}", why.err());
        };
        joined(&carriers, &done.start, done.line, one);
        joined(&carriers, &done.end, done.line, two);
        assert!(done.gap.get() <= ANGULAR.get(), "{}", done.gap.get());
        let way = carriers.get(done.line).unwrap();
        let disc = |at| disc_of(&carriers, at);
        assert_eq!(way.side(&disc(one).centre()).unwrap(), sides[0]);
        assert_eq!(way.side(&disc(two).centre()).unwrap(), sides[1]);
        fine += i32::from(matches!(way, Curve2::Fine(_)));
    }
    assert!(fine > 0);
}

#[test]
fn one_circle_inside_another_has_no_common_tangent() {
    let (mut carriers, [big, small]) = table([circle(p2(0, 0), 1_000), circle(p2(10, 0), 5)]);
    let sides = [Sign::Positive, Sign::Positive];
    let result = common_tangent(&mut carriers, big, small, sides, GridStep::Q0, &certified());
    assert_eq!(result.err(), Some(PointError::Num(NumError::Degenerate)));
    assert_eq!(carriers.len(), 2);
}

#[test]
fn a_refused_result_says_what_it_would_reach_and_adds_nothing() {
    let (mut carriers, _, [first, second]) = left_corner(10_000, p2(10_301, 703));
    let refused = fillet(&mut carriers, &first, &second, 900, GridStep::Q0, &Accept::Exact);
    assert!(matches!(
        refused.err(),
        Some(PointError::ExactnessNotAccepted {
            would_reach: Exactness::Certified { .. },
            accepted: Accept::Exact,
        })
    ));
    assert_eq!(carriers.len(), 2);
}

#[test]
fn a_fillet_refuses_what_is_no_fillet() {
    let (mut carriers, [a, b], [first, second]) = left_corner(1_000, p2(1_300, 400));
    let degenerate = PointError::Num(NumError::Degenerate);
    let accept = certified();
    let step = GridStep::Q0;
    let at = |r| fillet(&mut carriers.clone(), &first, &second, r, step, &accept);
    assert_eq!(at(0).err(), Some(degenerate));
    assert_eq!(at(-1).err(), Some(degenerate));
    assert_eq!(at(1 << 41).err(), Some(PointError::Num(NumError::BudgetExceeded)));
    let none = Touch {
        side: Sign::Zero,
        ..first
    };
    assert!(fillet(&mut carriers, &none, &second, 5, step, &accept).is_err());
    let parallel = {
        let (mut more, names) = table([line(p2(0, 0), p2(10, 0)), line(p2(0, 5), p2(10, 5))]);
        let touch = |curve| Touch {
            curve,
            side: Sign::Positive,
            near: p2(0, 0),
        };
        fillet(&mut more, &touch(names[0]), &touch(names[1]), 5, step, &accept)
    };
    assert_eq!(parallel.err(), Some(degenerate));
    assert_eq!(carriers.len(), 2);
    let _ = (a, b);
}

#[test]
fn a_centre_at_the_frames_edge() {
    let corner = p2(M, M);
    let (mut carriers, [a, b]) = table([line(p2(-M, M), corner), line(corner, p2(M, -M))]);
    let touch = |curve| Touch {
        curve,
        side: Sign::Negative,
        near: corner,
    };
    let done = fillet(&mut carriers, &touch(b), &touch(a), 1 << 30, GridStep::Q0, &Accept::Exact);
    let done = done.unwrap();
    sound(&carriers, &done, b, a);
}

/// `count` corners along a zigzag with every turn, each corner's segments long for its radius.
fn zigzag(count: usize, rng: &mut Rng) -> Vec<Corner> {
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let x = (k as i64 % 1_000) * 100_000 - 50_000_000;
        let y = (k as i64 / 1_000) * 100_000 - 50_000_000;
        let at = p2(x, y);
        let reach = |rng: &mut Rng| 40_000 + (rng.next() % 40_000) as i64;
        let (a, b) = (reach(rng), reach(rng));
        let (c, d) = (reach(rng), reach(rng));
        // A corner in every class: quarter turns and 3:4:5, exact; the rest certified; some none.
        let (before, after) = match rng.next() % 5 {
            0 => (p2(x - a, y), p2(x, y + b)),
            1 => (p2(x, y + a), p2(x + 30_000, y + 40_000)),
            2 => (p2(x - a, y - c), p2(x + b, y + d)),
            3 => (p2(x + a, y + c), p2(x - b, y + d)),
            _ => (p2(x - 4 * 1_000, y - 3 * 1_000), p2(x + 4 * 1_000, y + 3 * 1_000)),
        };
        out.push(Corner {
            before,
            at,
            after,
        });
    }
    out
}

#[test]
fn every_corner_of_a_hundred_thousand() {
    let mut rng = Rng::new();
    let count = if cfg!(debug_assertions) { 5_000 } else { 100_000 };
    let corners = zigzag(count, &mut rng);
    let mut carriers = Carriers::new();
    let accept = certified();
    for corner in &corners {
        let run = fillet_all(
            &mut Carriers::new(),
            core::slice::from_ref(corner),
            500,
            GridStep::Q0,
            &accept,
            &Exec::serial(),
            &mut WorkMeter::new(Budget::UNLIMITED),
        );
        assert!(run.is_ok(), "{corner:?}: {run:?}");
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let done = fillet_all(
        &mut carriers,
        &corners,
        500,
        GridStep::Q0,
        &accept,
        &Exec::serial(),
        &mut meter,
    )
    .unwrap();
    assert_eq!(done.len(), count);
    let (mut exact, mut near, mut none) = (0, 0, 0);
    for (k, fillet) in done.iter().enumerate() {
        let Some(fillet) = fillet else {
            none += 1;
            continue;
        };
        match fillet.exactness {
            Exactness::Exact => exact += 1,
            _ => near += 1,
        }
        assert!(fillet.gap.get() <= ANGULAR.get());
        // Reading every join back is what takes the time: a thousand of them, and the first.
        if k % (count / 1_000) == 0 {
            assert!(fillet.start.on(&carriers, fillet.circle).unwrap());
            assert!(fillet.end.on(&carriers, fillet.circle).unwrap());
        }
    }
    println!("exact {exact}, certified {near}, no turn {none}");
    assert!(exact > 0 && near > 0 && none > 0);
    assert_eq!(meter.used().work, count as u128);
}

#[test]
fn fillet_all_adds_nothing_when_a_corner_fails() {
    let good = Corner {
        before: p2(0, 0),
        at: p2(1_000, 0),
        after: p2(1_301, 703),
    };
    let bad = Corner {
        before: p2(0, 0),
        at: p2(0, 0),
        after: p2(1, 1),
    };
    let mut carriers = Carriers::new();
    let accept = certified();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let run = fillet_all(
        &mut carriers,
        &[good, bad],
        50,
        GridStep::Q0,
        &accept,
        &Exec::serial(),
        &mut meter,
    );
    assert!(run.is_err());
    assert_eq!(carriers.len(), 0);
}

#[test]
fn fillet_all_stops_at_its_budget() {
    let good = Corner {
        before: p2(0, 0),
        at: p2(1_000, 0),
        after: p2(1_300, 400),
    };
    let mut carriers = Carriers::new();
    let accept = certified();
    let mut meter = WorkMeter::new(Budget::new(1, 1 << 30));
    let run = fillet_all(
        &mut carriers,
        &[good; 8],
        50,
        GridStep::Q0,
        &accept,
        &Exec::serial(),
        &mut meter,
    );
    assert!(run.is_err());
    assert_eq!(carriers.len(), 0);
}
