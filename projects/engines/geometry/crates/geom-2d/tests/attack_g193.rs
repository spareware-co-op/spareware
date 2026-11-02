//! Attacks on piece G193: tangent constructions (`geom_2d::tangent`; chapter 14.3 of the kernel
//! architecture). Every test tries to break a claim of the piece's done-when or of the module's
//! guarantees.
//!
//! - **Every join an exact shared point**: read back through [`Pt2::on`] against both its carriers,
//!   for every pairing of line, circle and ellipse.
//! - **Exact for rational normals and parallels**: exactly tangent, gap exactly zero.
//! - **Within `ε_ang`**: every Certified gap at most the ladder's angular bound, at every scale.
//! - **No extra contact**: the other crossing of each neighbour lies off the arc.
//! - **Wherever it touches an ellipse**, the ends of its axes included, where its two crossings
//!   fall in two quarters of the ellipse's t.
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
    clippy::many_single_char_names,
    reason = "c, r, p, q and the like are the geometry's"
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
    Corner, Fillet, TangentCircle, TangentLine, Touch, common_tangent, fillet, fillet_all,
    semicircle, tangent_circle, tangent_from,
};
use geom_num::arc::{Circle2 as GridCircle, Contact, Disc};
use geom_num::conic::{Ellipse2, Meeting};
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
    let turns = [
        p2(1_000, 500),
        p2(1_000, -500),
        p2(700, 400),
        p2(1_300, 400),
    ];
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
        let done = fillet(
            &mut carriers,
            &from,
            &into,
            40,
            GridStep::Q1,
            &Accept::Exact,
        )
        .unwrap();
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
        let to = p2(
            s + 1 + (rng.next() % 1_000) as i64,
            1 + (rng.next() % 1_000) as i64,
        );
        let bits = rng.next() % 16;
        let r = i128::from(1 + (rng.next() % (1 << bits)) as i64);
        let (mut carriers, [a, b], [first, second]) = left_corner(s, to);
        let Ok(done) = fillet(
            &mut carriers,
            &first,
            &second,
            r,
            GridStep::Q0,
            &certified(),
        ) else {
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
    let done = fillet(
        &mut carriers,
        &first,
        &second,
        900,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
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
    assert_eq!(
        semicircle(&mut carriers, a, b, p2(0, 0)).err(),
        Some(refused)
    );
    assert_eq!(
        semicircle(&mut carriers, a, c, p2(0, 0)).err(),
        Some(refused)
    );
    let (mut carriers, [d, e]) = table([line(p2(0, 0), p2(10, 0)), line(p2(0, 5), p2(10, 5))]);
    assert_eq!(
        semicircle(&mut carriers, d, e, p2(1, 1)).err(),
        Some(refused)
    );
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
    let done = fillet(
        &mut carriers,
        &first,
        &second,
        8_000,
        GridStep::Q0,
        &certified(),
    );
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
    let done = fillet(
        &mut carriers,
        &above,
        &inside,
        3_000,
        GridStep::Q0,
        &certified(),
    );
    let done = done.unwrap();
    sound(&carriers, &done, chord, wheel);
}

#[test]
fn a_fillet_of_two_circles() {
    let (mut carriers, [left, right]) =
        table([circle(p2(0, 0), 3_000), circle(p2(8_000, 1_000), 4_000)]);
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
    let done = fillet(
        &mut carriers,
        &over,
        &out,
        2_000,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
    assert!(matches!(done.end, Pt2::ConicRoot { .. }), "{:?}", done.end);
    sound(&carriers, &done, ground, oval);
}

/// Where `point`, a root of an ellipse and a circle, lies: its angle, its stretch narrowed past
/// what a double holds.
fn conic_point(carriers: &Carriers, point: &Pt2) -> [f64; 2] {
    let Pt2::ConicRoot { a, b, k } = *point else {
        panic!("{point:?}")
    };
    let Some(Curve2::Ellipse(ellipse)) = carriers.get(a).copied() else {
        panic!()
    };
    let conic = carriers.get(b).unwrap().conic().unwrap();
    let Meeting::At { quartic, roots } = ellipse.meeting(&conic).unwrap() else {
        panic!()
    };
    let roots = roots.narrowed(&quartic, 64).unwrap();
    let [c, s] = roots.iter().nth(usize::from(k)).unwrap().angle().unwrap();
    let (c, s) = (f64::midpoint(c.lo(), c.hi()), f64::midpoint(s.lo(), s.hi()));
    let ([mx, my], (p, q)) = (ellipse.major(), ellipse.ratio());
    let (mx, my, ratio) = (mx as f64, my as f64, p as f64 / q as f64);
    let o = ellipse.centre();
    [
        o.x.get() as f64 + c * mx - s * ratio * my,
        o.y.get() as f64 + c * my + s * ratio * mx,
    ]
}

#[test]
fn a_circle_touching_an_ellipse_at_each_end_of_its_axes() {
    // The ellipse of semi-axes 20,000 and 12,000 along x, and a wedge of two lines beyond each end
    // of its axes, opening onto it. The circle touching the ellipse at the end and both lines, from
    // outside it and from inside, crosses it either side of the end: in two quarters of its t, the
    // ends at θ = −π and π, 0, π/2 and −π/2, which isolation alone leaves a quarter turn wide each.
    // Each row: the end, the wedge's sides and a point near each touch, and the ellipse's
    // (leaving, arriving) roots from outside and from inside.
    type End = (P2, [P2; 4], [P2; 2], [(u8, u8); 2]);
    let ends: [End; 4] = [
        (
            p2(-20_000, 0),
            [
                p2(-40_000, 0),
                p2(0, 20_000),
                p2(0, -20_000),
                p2(-40_000, 0),
            ],
            [p2(-28_000, 6_000), p2(-28_000, -6_000)],
            [(1, 0), (0, 3)],
        ),
        (
            p2(20_000, 0),
            [p2(40_000, 0), p2(0, -20_000), p2(0, 20_000), p2(40_000, 0)],
            [p2(28_000, -6_000), p2(28_000, 6_000)],
            [(0, 1), (2, 1)],
        ),
        (
            p2(0, 12_000),
            [p2(0, 30_000), p2(40_000, 0), p2(-40_000, 0), p2(0, 30_000)],
            [p2(12_000, 21_000), p2(-12_000, 21_000)],
            [(0, 1), (1, 0)],
        ),
        (
            p2(0, -12_000),
            [
                p2(0, -30_000),
                p2(-40_000, 0),
                p2(40_000, 0),
                p2(0, -30_000),
            ],
            [p2(-12_000, -21_000), p2(12_000, -21_000)],
            [(0, 1), (1, 0)],
        ),
    ];
    for (end, [a, b, c, d], [one, two], roots) in ends {
        for (side, (leaving, arriving)) in [Sign::Positive, Sign::Negative].into_iter().zip(roots) {
            let (mut carriers, [oval, upper, lower]) = table([
                Curve2::Ellipse(Ellipse2::new(p2(0, 0), [20_000, 0], 3, 5).unwrap()),
                line(a, b),
                line(c, d),
            ]);
            let touch = |curve, side, near| Touch { curve, side, near };
            let touches = [
                touch(oval, side, end),
                touch(upper, Sign::Negative, one),
                touch(lower, Sign::Negative, two),
            ];
            let done = tangent_circle(&mut carriers, &touches, GridStep::Q0, &certified())
                .unwrap_or_else(|e| panic!("{end:?} {side:?}: {e:?}"));
            for (join, name) in done.joins.iter().zip([oval, upper, lower]) {
                joined(&carriers, &join.leaving, name, done.circle);
                joined(&carriers, &join.arriving, name, done.circle);
            }
            assert!(done.gap.get() <= ANGULAR.get());
            let join = done.joins[0];
            let k = |p: &Pt2| match p {
                Pt2::ConicRoot { k, .. } => *k,
                other => panic!("{other:?}"),
            };
            assert_eq!(
                (k(&join.leaving), k(&join.arriving)),
                (leaving, arriving),
                "{end:?} {side:?}"
            );
            // The cap past the ellipse runs counter-clockwise from arriving to leaving, and is a
            // hair of the circle.
            let disc = disc_of(&carriers, done.circle);
            let ([x, y], w) = disc.centre().coordinates();
            let (cx, cy) = (x as f64 / w as f64, y as f64 / w as f64);
            let way = |p: [f64; 2]| [p[0] - cx, p[1] - cy];
            let ([ax, ay], [lx, ly]) = (
                way(conic_point(&carriers, &join.arriving)),
                way(conic_point(&carriers, &join.leaving)),
            );
            let lengths = geom_num::math::sqrt((ax * ax + ay * ay) * (lx * lx + ly * ly))
                .unwrap()
                .get();
            let (cap, ahead) = ((ax * ly - ay * lx) / lengths, ax * lx + ay * ly);
            assert!(
                ahead > 0.0 && 0.0 < cap && cap < 1e-10,
                "{end:?} {side:?}: {cap:e}"
            );
        }
    }
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
            carriers
                .get(done.line)
                .unwrap()
                .side(&disc.centre())
                .unwrap(),
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
        let (centre, other) = (
            p2(rng.up_to(far), rng.up_to(far)),
            p2(rng.up_to(far), rng.up_to(far)),
        );
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
        let Ok(done) = common_tangent(&mut carriers, one, two, sides, GridStep::Q0, &certified())
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
    let refused = fillet(
        &mut carriers,
        &first,
        &second,
        900,
        GridStep::Q0,
        &Accept::Exact,
    );
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
    assert_eq!(
        at(1 << 41).err(),
        Some(PointError::Num(NumError::BudgetExceeded))
    );
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
        fillet(
            &mut more,
            &touch(names[0]),
            &touch(names[1]),
            5,
            step,
            &accept,
        )
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
    let done = fillet(
        &mut carriers,
        &touch(b),
        &touch(a),
        1 << 30,
        GridStep::Q0,
        &Accept::Exact,
    );
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
            _ => (
                p2(x - 4 * 1_000, y - 3 * 1_000),
                p2(x + 4 * 1_000, y + 3 * 1_000),
            ),
        };
        out.push(Corner { before, at, after });
    }
    out
}

#[test]
fn every_corner_of_a_hundred_thousand() {
    let mut rng = Rng::new();
    let count = if cfg!(debug_assertions) {
        5_000
    } else {
        100_000
    };
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

/// What a construction made, in full: its circle or line, its joins, its gap's bits and its class;
/// each join checked on the curve it joins and on what was made, and the gap within the ladder's
/// angle, and above 0 where the result is Certified, not Exact.
fn made(
    carriers: &Carriers,
    circle: CurveIx,
    joins: &[(&Pt2, CurveIx)],
    gap: f64,
    exactness: Exactness,
) -> String {
    for (point, curve) in joins {
        joined(carriers, point, *curve, circle);
    }
    assert!(gap <= ANGULAR.get(), "{gap}");
    match exactness {
        Exactness::Certified { .. } => assert!(gap > 0.0, "{exactness:?}"),
        _ => assert_eq!(gap, 0.0),
    }
    let joins: Vec<&Pt2> = joins.iter().map(|(p, _)| *p).collect();
    format!(
        "{:?} {joins:?} {:#018x} {exactness:?}",
        carriers.get(circle).unwrap(),
        gap.to_bits()
    )
}

/// One construction of each kind, each pairing of curves and each way round, by what it makes. The
/// pins hold the solver's aims, the rounding of the centre and every bound the gaps and the classes
/// come from. Each was read once and is checked against the exact questions by the tests above
/// (every join on both its carriers, every gap within the ladder's angle, every cap off the arc), so
/// a change to any of them is a change to what is made.
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one record per construction, in the order of the pins"
)]
fn every_construction_makes_what_it_made() {
    type Wedge = (P2, [P2; 4], [P2; 2]);
    let mut out: Vec<String> = Vec::new();
    let touch = |curve, side, near| Touch { curve, side, near };
    // Two lines whose normals have no whole length, at a sharp corner and an open one.
    for (s, to, r) in [
        (10_000, p2(10_301, 703), 900),
        (50_000, p2(40_000, 30_000), 7_000),
    ] {
        let (mut carriers, _, [first, second]) = left_corner(s, to);
        let f = fillet(
            &mut carriers,
            &first,
            &second,
            r,
            GridStep::Q0,
            &certified(),
        )
        .unwrap();
        out.push(made(
            &carriers,
            f.circle,
            &[(&f.start, first.curve), (&f.end, second.curve)],
            f.gap.get(),
            f.exactness,
        ));
    }
    // A line and a circle: outside it, either way round, and inside it, against a chord.
    let (mut carriers, [ground, wheel]) = table([
        line(p2(-100_000, 0), p2(100_000, 0)),
        circle(p2(0, 40_000), 30_000),
    ]);
    let (a, b) = (
        touch(wheel, Sign::Positive, p2(16_000, 16_000)),
        touch(ground, Sign::Positive, p2(20_000, 0)),
    );
    for (first, second) in [(&a, &b), (&b, &a)] {
        match fillet(
            &mut carriers,
            first,
            second,
            8_000,
            GridStep::Q0,
            &certified(),
        ) {
            Ok(f) => out.push(made(
                &carriers,
                f.circle,
                &[(&f.start, first.curve), (&f.end, second.curve)],
                f.gap.get(),
                f.exactness,
            )),
            Err(e) => out.push(format!("refused {e:?}")),
        }
    }
    let (mut carriers, [chord, wheel]) = table([
        line(p2(-100_000, 10_000), p2(100_000, 10_000)),
        circle(p2(0, 0), 30_000),
    ]);
    let (inside, above) = (
        touch(wheel, Sign::Negative, p2(26_000, 14_500)),
        touch(chord, Sign::Positive, p2(23_600, 10_000)),
    );
    let f = fillet(
        &mut carriers,
        &above,
        &inside,
        3_000,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
    out.push(made(
        &carriers,
        f.circle,
        &[(&f.start, above.curve), (&f.end, inside.curve)],
        f.gap.get(),
        f.exactness,
    ));
    // Two circles, the fillet outside both.
    let (mut carriers, [left, right]) =
        table([circle(p2(0, 0), 3_000), circle(p2(8_000, 1_000), 4_000)]);
    let (a, b) = (
        touch(left, Sign::Positive, p2(2_000, 2_200)),
        touch(right, Sign::Positive, p2(5_000, 3_600)),
    );
    let f = fillet(&mut carriers, &a, &b, 2_000, GridStep::Q0, &certified()).unwrap();
    out.push(made(
        &carriers,
        f.circle,
        &[(&f.start, a.curve), (&f.end, b.curve)],
        f.gap.get(),
        f.exactness,
    ));
    // A line and an ellipse.
    let (mut carriers, [ground, oval]) = table([
        line(p2(-100_000, -2_000), p2(100_000, -2_000)),
        Curve2::Ellipse(Ellipse2::new(p2(0, 4_000), [20_000, 0], 3, 5).unwrap()),
    ]);
    let (over, beside) = (
        touch(ground, Sign::Positive, p2(20_000, -2_000)),
        touch(oval, Sign::Positive, p2(16_000, -3_000)),
    );
    let f = fillet(
        &mut carriers,
        &over,
        &beside,
        2_000,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
    out.push(made(
        &carriers,
        f.circle,
        &[(&f.start, over.curve), (&f.end, beside.curve)],
        f.gap.get(),
        f.exactness,
    ));
    // A fine line, the tangent from far away to a small circle, and a grid line across it.
    let (mut carriers, [wheel, cross]) = table([
        circle(p2(1 << 39, 1 << 38), 1_000),
        line(p2((1 << 39) - 100_000, 0), p2((1 << 39) - 100_000, 1 << 39)),
    ]);
    let t = tangent_from(
        &mut carriers,
        p2(-(1 << 39), -(1 << 38)),
        wheel,
        Sign::Positive,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
    out.push(made(
        &carriers,
        t.line,
        &[(&t.start, t.line), (&t.end, wheel)],
        t.gap.get(),
        t.exactness,
    ));
    for sides in [
        (Sign::Positive, Sign::Positive),
        (Sign::Negative, Sign::Negative),
        (Sign::Positive, Sign::Negative),
        (Sign::Negative, Sign::Positive),
    ] {
        let mut copy = carriers.clone();
        let near = p2((1 << 39) - 100_000, (1 << 38) - 50_000);
        let (a, b) = (touch(t.line, sides.0, near), touch(cross, sides.1, near));
        match fillet(&mut copy, &a, &b, 5_000, GridStep::Q0, &certified()) {
            Ok(f) => out.push(made(
                &copy,
                f.circle,
                &[(&f.start, a.curve), (&f.end, b.curve)],
                f.gap.get(),
                f.exactness,
            )),
            Err(e) => out.push(format!("refused {e:?}")),
        }
    }
    // Three lines, and the tangent circles at two ends of an ellipse's axes.
    let (mut carriers, [base, up, down]) = table([
        line(p2(0, 0), p2(60_000, 0)),
        line(p2(30_000, 40_000), p2(0, 0)),
        line(p2(60_000, 0), p2(30_000, 40_000)),
    ]);
    let touches = [
        touch(base, Sign::Positive, p2(30_000, 0)),
        touch(up, Sign::Positive, p2(12_000, 16_000)),
        touch(down, Sign::Positive, p2(48_000, 16_000)),
    ];
    let c = tangent_circle(&mut carriers, &touches, GridStep::Q0, &certified()).unwrap();
    let joins: Vec<(&Pt2, CurveIx)> = c
        .joins
        .iter()
        .zip([base, up, down])
        .flat_map(|(j, k)| [(&j.leaving, k), (&j.arriving, k)])
        .collect();
    out.push(made(&carriers, c.circle, &joins, c.gap.get(), c.exactness));
    let wedges: [Wedge; 2] = [
        (
            p2(-20_000, 0),
            [
                p2(-40_000, 0),
                p2(0, 20_000),
                p2(0, -20_000),
                p2(-40_000, 0),
            ],
            [p2(-28_000, 6_000), p2(-28_000, -6_000)],
        ),
        (
            p2(0, 12_000),
            [p2(0, 30_000), p2(40_000, 0), p2(-40_000, 0), p2(0, 30_000)],
            [p2(12_000, 21_000), p2(-12_000, 21_000)],
        ),
    ];
    for (end, [a, b, cc, d], [one, two]) in wedges {
        for side in [Sign::Positive, Sign::Negative] {
            let (mut carriers, [oval, upper, lower]) = table([
                Curve2::Ellipse(Ellipse2::new(p2(0, 0), [20_000, 0], 3, 5).unwrap()),
                line(a, b),
                line(cc, d),
            ]);
            let touches = [
                touch(oval, side, end),
                touch(upper, Sign::Negative, one),
                touch(lower, Sign::Negative, two),
            ];
            let c = tangent_circle(&mut carriers, &touches, GridStep::Q0, &certified()).unwrap();
            let joins: Vec<(&Pt2, CurveIx)> = c
                .joins
                .iter()
                .zip([oval, upper, lower])
                .flat_map(|(j, k)| [(&j.leaving, k), (&j.arriving, k)])
                .collect();
            out.push(made(&carriers, c.circle, &joins, c.gap.get(), c.exactness));
        }
    }
    // A tangent from a point, each way, and the common tangents of two circles, each kind.
    let (mut carriers, [wheel]) = table([circle(p2(1_000, 2_000), 700)]);
    for side in [Sign::Positive, Sign::Negative] {
        let t = tangent_from(
            &mut carriers,
            p2(-3_000, 500),
            wheel,
            side,
            GridStep::Q0,
            &certified(),
        )
        .unwrap();
        out.push(made(
            &carriers,
            t.line,
            &[(&t.start, t.line), (&t.end, wheel)],
            t.gap.get(),
            t.exactness,
        ));
    }
    let (mut carriers, [one, two]) =
        table([circle(p2(0, 0), 900), circle(p2(7_000, 2_000), 1_300)]);
    for sides in [
        [Sign::Positive, Sign::Positive],
        [Sign::Negative, Sign::Negative],
        [Sign::Positive, Sign::Negative],
        [Sign::Negative, Sign::Positive],
    ] {
        let t = common_tangent(&mut carriers, one, two, sides, GridStep::Q0, &certified()).unwrap();
        out.push(made(
            &carriers,
            t.line,
            &[(&t.start, one), (&t.end, two)],
            t.gap.get(),
            t.exactness,
        ));
    }
    for (k, line) in out.iter().enumerate() {
        println!("{k}: {line}");
    }
    assert_eq!(
        out.len(),
        PINNED.len(),
        "the pins are the lines printed above"
    );
    for (k, (got, want)) in out.iter().zip(PINNED).enumerate() {
        assert_eq!(got, want, "construction {k}");
    }
}

/// What [`every_construction_makes_what_it_made`] read.
const PINNED: &[&str] = &[
    "Circle(Disc(Disc { centre: Rational2 { x: 347032018456251748475488, y: 33204139332677192908772, w: 36893488147419103232 }, rr: Wide<4>(0x0000000000000000000000000031704000000000000000000000000000000000) })) [Crossing { a: CurveIx(0), b: CurveIx(2), root: Second }, Crossing { a: CurveIx(1), b: CurveIx(2), root: First }] 0x3dc693bcc90bae75 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 185795692977543314095699, y: 32281802128991715327973, w: 4611686018427387904 }, rr: Wide<4>(0x000000000000000000000000002ebae400000000000000000000000000000000) })) [Crossing { a: CurveIx(0), b: CurveIx(2), root: Second }, Crossing { a: CurveIx(1), b: CurveIx(2), root: First }] 0x3dc67c192c8760c1 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 94511439157779386054008, y: 36893488147419103231969, w: 4611686018427387904 }, rr: Wide<4>(0x000000000000000000000000003d090000000000000000000000000000000000) })) [Crossing { a: CurveIx(2), b: CurveIx(1), root: First }, Crossing { a: CurveIx(0), b: CurveIx(2), root: First }] 0x3dc6896841a002df Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 94511439157779386054008, y: 36893488147419103231969, w: 4611686018427387904 }, rr: Wide<4>(0x000000000000000000000000003d090000000000000000000000000000000000) })) [Crossing { a: CurveIx(0), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(3), b: CurveIx(1), root: Second }] 0x3dc6896841a002df Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 218264819356971463544617, y: 119903836479112085503977, w: 9223372036854775808 }, rr: Wide<4>(0x0000000000000000000000000022551000000000000000000000000000000000) })) [Crossing { a: CurveIx(0), b: CurveIx(2), root: Second }, Crossing { a: CurveIx(2), b: CurveIx(1), root: First }] 0x3dc66a4e1af619ae Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 52805008463364199722017, y: 75622022283244295855894, w: 18446744073709551616 }, rr: Wide<4>(0x000000000000000000000000003d090000000000000000000000000000000000) })) [Crossing { a: CurveIx(2), b: CurveIx(0), root: First }, Crossing { a: CurveIx(2), b: CurveIx(1), root: Second }] 0x3dc6683b7952d71f Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 99685302156899410986465950, y: -7840, w: 4722366482869645213696 }, rr: Wide<4>(0x00000000000000000000003d0900000000000000000000000000000000000000) })) [Crossing { a: CurveIx(0), b: CurveIx(2), root: Second }, ConicRoot { a: CurveIx(1), b: CurveIx(2), k: 1 }] 0x3dc66667ccccccd5 Certified { bound: Steps(1), step: Q0 }",
    "Fine(FineLine { a: -1052665063990798, b: 2105330132263189, c: -1176915322226081792 }) [Grid(P2 { x: Coord(-549755813888), y: Coord(-274877906944) }), Crossing { a: CurveIx(2), b: CurveIx(0), root: Second }] 0x3dc5ea7c05f28ab2 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 2535300716229426868117676490771, y: 1267650378738800774232519619996, w: 4611686018427387904 }, rr: Wide<4>(0x0000000000000000000000000017d78400000000000000000000000000000000) })) [Crossing { a: CurveIx(2), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(1), b: CurveIx(3), root: First }] 0x3dc6810ccdc29b41 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 2535300762346287052391555530733, y: 1267650350237013700002299087128, w: 4611686018427387904 }, rr: Wide<4>(0x0000000000000000000000000017d78400000000000000000000000000000000) })) [Crossing { a: CurveIx(2), b: CurveIx(3), root: First }, Crossing { a: CurveIx(1), b: CurveIx(3), root: Second }] 0x3dc6514d2d4334d4 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 2535300762346287052391555530733, y: 1267650401797230819475714847554, w: 4611686018427387904 }, rr: Wide<4>(0x0000000000000000000000000017d78400000000000000000000000000000000) })) [Crossing { a: CurveIx(2), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(1), b: CurveIx(3), root: Second }] 0x3dc68dee3962c563 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 2535300716229426868117676490771, y: 1267650327178583654759103859570, w: 4611686018427387904 }, rr: Wide<4>(0x0000000000000000000000000017d78400000000000000000000000000000000) })) [Crossing { a: CurveIx(2), b: CurveIx(3), root: First }, Crossing { a: CurveIx(1), b: CurveIx(3), root: First }] 0x3dc6514d2d4334d4 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 69175290276410818560000, y: 34587645138205409280000, w: 2305843009213693952 }, rr: Wide<4>(0x0000000000000000000000000035a4e9000000000001a8ce0000000000000349) })) [Crossing { a: CurveIx(0), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(0), b: CurveIx(3), root: First }, Crossing { a: CurveIx(1), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(1), b: CurveIx(3), root: First }, Crossing { a: CurveIx(2), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(2), b: CurveIx(3), root: First }] 0x3dc683334380968f Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: -61816579797430146198986414, y: 0, w: 2361183241434822606848 }, rr: Wide<4>(0x000000000000000000000091b55e48002c1a968e362766a06753533f5c9f7c40) })) [ConicRoot { a: CurveIx(0), b: CurveIx(3), k: 1 }, ConicRoot { a: CurveIx(0), b: CurveIx(3), k: 0 }, Crossing { a: CurveIx(1), b: CurveIx(3), root: First }, Crossing { a: CurveIx(1), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(2), b: CurveIx(3), root: First }, Crossing { a: CurveIx(2), b: CurveIx(3), root: Second }] 0x3dc6667a93e6e584 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: -2254729361403633001601097, y: 0, w: 590295810358705651712 }, rr: Wide<4>(0x00000000000000000000003e6b361b7ffd3e569ac480ccd95ff2ea6568248584) })) [ConicRoot { a: CurveIx(0), b: CurveIx(3), k: 0 }, ConicRoot { a: CurveIx(0), b: CurveIx(3), k: 3 }, Crossing { a: CurveIx(1), b: CurveIx(3), root: First }, Crossing { a: CurveIx(1), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(2), b: CurveIx(3), root: First }, Crossing { a: CurveIx(2), b: CurveIx(3), root: Second }] 0x3dc6665f719c4c40 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 0, y: 23611832414348226068484342, w: 1180591620717411303424 }, rr: Wide<4>(0x00000000000000000000003d09000000000000010a8a6000000000000122fee1) })) [ConicRoot { a: CurveIx(0), b: CurveIx(3), k: 0 }, ConicRoot { a: CurveIx(0), b: CurveIx(3), k: 1 }, Crossing { a: CurveIx(1), b: CurveIx(3), root: First }, Crossing { a: CurveIx(1), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(2), b: CurveIx(3), root: First }, Crossing { a: CurveIx(2), b: CurveIx(3), root: Second }] 0x3dc6669ff9026b08 Certified { bound: Steps(1), step: Q0 }",
    "Circle(Disc(Disc { centre: Rational2 { x: 0, y: -8854437155380584775724249, w: 147573952589676412928 }, rr: Wide<4>(0x00000000000000000000004d3f6400000000000bdc471c0000000000748bd099) })) [ConicRoot { a: CurveIx(0), b: CurveIx(3), k: 1 }, ConicRoot { a: CurveIx(0), b: CurveIx(3), k: 0 }, Crossing { a: CurveIx(1), b: CurveIx(3), root: First }, Crossing { a: CurveIx(1), b: CurveIx(3), root: Second }, Crossing { a: CurveIx(2), b: CurveIx(3), root: First }, Crossing { a: CurveIx(2), b: CurveIx(3), root: Second }] 0x3dcaf3ecaa98ba3f Certified { bound: Steps(1), step: Q0 }",
    "Line(Carrier { a: -9652409441, b: 49084602782, c: 53499529714000 }) [Grid(P2 { x: Coord(-3000), y: Coord(500) }), Crossing { a: CurveIx(1), b: CurveIx(0), root: Second }] 0x3dc2e7a0bdb10117 Certified { bound: Steps(1), step: Q0 }",
    "Line(Carrier { a: -39226815521, b: 67978666024, c: 151669779575000 }) [Grid(P2 { x: Coord(-3000), y: Coord(500) }), Crossing { a: CurveIx(2), b: CurveIx(0), root: Second }] 0x3dc4d28a9fea362a Certified { bound: Steps(1), step: Q0 }",
    "Fine(FineLine { a: -3027466529831649280, b: 13330033756296183808, c: -12302554058484350975984 }) [Crossing { a: CurveIx(2), b: CurveIx(0), root: First }, Crossing { a: CurveIx(2), b: CurveIx(1), root: Second }] 0x3dc61427d2ca09b3 Certified { bound: Steps(1), step: Q0 }",
    "Fine(FineLine { a: -4779958608018276352, b: 13807551382837788672, c: 13150366853517803519990 }) [Crossing { a: CurveIx(3), b: CurveIx(0), root: First }, Crossing { a: CurveIx(3), b: CurveIx(1), root: Second }] 0x3dc5e18ee717eac2 Certified { bound: Steps(1), step: Q0 }",
    "Fine(FineLine { a: -5985699785787572224, b: 9031490230007365632, c: -9751466471112930397092 }) [Crossing { a: CurveIx(4), b: CurveIx(0), root: First }, Crossing { a: CurveIx(4), b: CurveIx(1), root: Second }] 0x3dc859fa20fe48ed Certified { bound: Steps(1), step: Q0 }",
    "Fine(FineLine { a: 357239489107591168, b: 12447001810340675584, c: 11206914563632291095270 }) [Crossing { a: CurveIx(5), b: CurveIx(0), root: First }, Crossing { a: CurveIx(5), b: CurveIx(1), root: Second }] 0x3dc662d5ad36ac2c Certified { bound: Steps(1), step: Q0 }",
];

/// The touches of `c`, each join on its curve and the circle.
fn three_joined(carriers: &Carriers, c: &TangentCircle, curves: [CurveIx; 3]) {
    for (join, name) in c.joins.iter().zip(curves) {
        joined(carriers, &join.leaving, name, c.circle);
        joined(carriers, &join.arriving, name, c.circle);
    }
    assert!(
        0.0 < c.gap.get() && c.gap.get() <= ANGULAR.get(),
        "{}",
        c.gap.get()
    );
}

#[test]
fn caps_on_opposite_sides_of_the_circle_are_clear() {
    // A circle on a floor, under a ceiling of each kind, against a wall: its caps on the floor and
    // the ceiling are opposite, the way from the centre to each pointing away from the other, so
    // they are as far apart as caps can be. The ceiling is a circle from outside, a circle from
    // inside, and an ellipse; each cap's way is the curve's own, which this proves the right way
    // round.
    let touch = |curve, side, near| Touch { curve, side, near };
    let floor = line(p2(-50_000, 0), p2(50_000, 0));
    let wall = line(p2(1_000, 50_000), p2(1_000, -50_000));
    // The circle of radius 1,000 about (0, 1,000): the round ceiling above it about (0, 3,000) of
    // radius 1,000; the round room about it, about (0, 500) of radius 1,500, whose top it touches
    // from inside; the elliptical ceiling of semi-axes 20,000 and 12,000 about (0, 14,000).
    let cases = [
        (
            circle(p2(0, 3_000), 1_000),
            Sign::Positive,
            p2(0, 2_000),
            wall,
            p2(1_000, 1_000),
        ),
        (
            circle(p2(0, 500), 1_500),
            Sign::Negative,
            p2(0, 2_000),
            wall,
            p2(1_000, 1_000),
        ),
        (
            Curve2::Ellipse(Ellipse2::new(p2(0, 14_000), [20_000, 0], 3, 5).unwrap()),
            Sign::Positive,
            p2(0, 2_000),
            wall,
            p2(1_000, 1_000),
        ),
    ];
    for (ceiling, side, top, wall, beside) in cases {
        let (mut carriers, [low, high, right]) = table([floor, ceiling, wall]);
        let touches = [
            touch(low, Sign::Positive, p2(0, 0)),
            touch(high, side, top),
            touch(right, Sign::Negative, beside),
        ];
        let c = tangent_circle(&mut carriers, &touches, GridStep::Q0, &certified())
            .unwrap_or_else(|e| panic!("{ceiling:?}: {e:?}"));
        three_joined(&carriers, &c, [low, high, right]);
    }
}

#[test]
fn caps_too_near_to_tell_apart_are_refused() {
    // Two lines that turn by 2^−38 of a radian: a fillet's two caps are that far apart about its
    // centre, under the sum of their gaps, so no exact question can say which comes first.
    let (mut carriers, _, [first, second]) = left_corner(1 << 38, p2(1 << 39, 1));
    let before = carriers.len();
    assert_eq!(
        fillet(
            &mut carriers,
            &first,
            &second,
            1_000,
            GridStep::Q0,
            &certified()
        )
        .err(),
        Some(PointError::Num(NumError::Degenerate))
    );
    assert_eq!(carriers.len(), before);
}

#[test]
fn a_circle_about_another_touches_it_at_its_far_side() {
    // The fillet of radius 15,000 of the circle of radius 10,000 about the origin, from inside it,
    // and the line y = 15,000: about (5,000, 0), it holds the smaller circle and touches it at
    // (−10,000, 0), the point of it farthest from its own centre, so the circle that crosses there
    // is the smaller one; each way round, its joins are the crossings either side of each touch.
    let touch = |curve, side, near| Touch { curve, side, near };
    let (mut carriers, [inner, high]) = table([
        circle(p2(0, 0), 10_000),
        line(p2(50_000, 15_000), p2(-50_000, 15_000)),
    ]);
    let (around, below) = (
        touch(inner, Sign::Negative, p2(-10_000, 0)),
        touch(high, Sign::Positive, p2(5_000, 15_000)),
    );
    let done = fillet(
        &mut carriers,
        &around,
        &below,
        15_000,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
    sound(&carriers, &done, inner, high);
    let disc = disc_of(&carriers, done.circle);
    let ([x, y], w) = disc.centre().coordinates();
    assert!(((x as f64 / w as f64) - 5_000.0).abs() < 1e-3, "{x} {w}");
    assert!((y as f64 / w as f64).abs() < 1e-3, "{y} {w}");
    let root = |p: &Pt2| match p {
        Pt2::Crossing { root, .. } => *root,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        (root(&done.start), root(&done.end)),
        (Root::First, Root::First)
    );
    let back = fillet(
        &mut carriers,
        &below,
        &around,
        15_000,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
    sound(&carriers, &back, high, inner);
    assert_eq!(
        (root(&back.start), root(&back.end)),
        (Root::Second, Root::Second)
    );
}

/// The greatest common divisor of whole numbers' magnitudes.
fn gcd(mut a: i128, mut b: i128) -> i128 {
    (a, b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[test]
fn a_huge_fillet_aims_by_its_chord() {
    // At a radius of 2^36 steps the chord's angle, 1 / (2r), is below the one aimed at, so the
    // depth comes from it, and the join lies just over half a step along each line from the ideal
    // touch: the class's bound of one step is the least whole one past that.
    let k = 1_i64 << 36;
    let (mut carriers, [a, b], [first, second]) =
        left_corner(1 << 38, p2((1 << 38) + 2 * k, 3 * k));
    let done = fillet(
        &mut carriers,
        &first,
        &second,
        1 << 36,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
    sound(&carriers, &done, a, b);
    assert_eq!(
        done.exactness,
        Exactness::Certified {
            bound: Steps(1),
            step: GridStep::Q0
        }
    );
    // The angle each line crosses at is about its chord's, 2^−37.
    let chord = 1.0 / (1_u64 << 37) as f64;
    assert!(
        chord < done.gap.get() && done.gap.get() < 1.1 * chord,
        "{}",
        done.gap.get()
    );
    assert_eq!(
        format!("{:?}", carriers.get(done.circle).unwrap()),
        HUGE_FILLET,
        "the circle as it was read"
    );
    // A tangent to a circle of that radius from far away aims the same way.
    let (mut carriers, [wheel]) = table([circle(p2(0, 0), 1 << 36)]);
    let t = tangent_from(
        &mut carriers,
        p2(-(1 << 39), 1 << 37),
        wheel,
        Sign::Positive,
        GridStep::Q0,
        &certified(),
    )
    .unwrap();
    joined(&carriers, &t.end, t.line, wheel);
    assert!(t.gap.get() <= ANGULAR.get());
    assert_eq!(
        format!(
            "{:?} {:?} {:#018x}",
            carriers.get(t.line).unwrap(),
            t.exactness,
            t.gap.get().to_bits()
        ),
        HUGE_TANGENT,
        "the line as it was read"
    );
}

/// The circle of [`a_huge_fillet_aims_by_its_chord`]'s fillet, as it was read.
const HUGE_FILLET: &str = "Circle(Disc(Disc { centre: Rational2 { x: 2094352907291988929982650, y: 604462909807314587353072, w: 8796093022208 }, rr: Wide<4>(0x0000000000000000000000004000000000000000000000000000000000000000) }))";

/// Its tangent line, class and gap, as they were read.
const HUGE_TANGENT: &str = "Line(Carrier { a: 267069911968, b: 695685715759, c: -51208920099628133646336 }) Certified { bound: Steps(1), step: Q0 } 0x3d9fc86bd3ce6bdd";

#[test]
fn fillet_all_starts_each_arc_where_it_turns_from() {
    // A left turn's arc starts on the first line, a right turn's on the second, as fillet runs.
    let left = Corner {
        before: p2(0, 0),
        at: p2(10_000, 0),
        after: p2(10_301, 703),
    };
    let right = Corner {
        before: p2(0, 0),
        at: p2(10_000, 0),
        after: p2(10_301, -703),
    };
    let mut carriers = Carriers::new();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let done = fillet_all(
        &mut carriers,
        &[left, right],
        900,
        GridStep::Q0,
        &certified(),
        &Exec::serial(),
        &mut meter,
    )
    .unwrap();
    // Each corner adds its two lines and its circle, in corner order.
    let name = |k: u32| {
        let mut table = Carriers::new();
        let mut last = None;
        for _ in 0..=k {
            last = Some(table.push(line(p2(0, 0), p2(1, 0))).unwrap());
        }
        last.unwrap()
    };
    let [Some(l), Some(r)] = [done[0], done[1]] else {
        panic!("{done:?}")
    };
    assert!(l.start.on(&carriers, name(0)).unwrap());
    assert!(l.end.on(&carriers, name(1)).unwrap());
    assert!(r.start.on(&carriers, name(4)).unwrap());
    assert!(r.end.on(&carriers, name(3)).unwrap());
}

#[test]
fn exact_centres_and_lines_are_in_lowest_terms() {
    // A semicircle's centre and an exact tangent's coefficients are reduced, as the grid and the
    // exact questions read them cheapest.
    for (b0, b1, at) in [
        (p2(0, 100), p2(1_000, 100), p2(500, 0)),
        (p2(0, 101), p2(1_000, 101), p2(500, 0)),
        (p2(0, 300), p2(1_000, 300), p2(600, 0)),
    ] {
        let (mut carriers, [a, b]) = table([line(p2(0, 0), p2(1_000, 0)), line(b0, b1)]);
        let done = semicircle(&mut carriers, a, b, at).unwrap();
        let ([x, y], w) = disc_of(&carriers, done.circle).centre().coordinates();
        assert_eq!(gcd(gcd(x, y), w), 1, "{x} {y} {w}");
    }
    // The tangent at (6, 8) of the circle of radius 10 about the origin: 3x + 4y = 50.
    let (mut carriers, [wheel]) = table([circle(p2(0, 0), 10)]);
    let done = tangent_from(
        &mut carriers,
        p2(6, 8),
        wheel,
        Sign::Positive,
        GridStep::Q0,
        &Accept::Exact,
    )
    .unwrap();
    let Some(Curve2::Line(l)) = carriers.get(done.line) else {
        panic!()
    };
    let ([a, b], c) = l.coefficients();
    assert_eq!(gcd(gcd(a, b), c), 1, "{a} {b} {c}");
    assert_eq!(a * a + b * b, 25, "{a} {b} {c}");
}

#[test]
fn a_common_tangent_says_how_far_its_wider_join_may_be() {
    // A circle of radius 2^36 and one of 1,000: a join on the large one may lie further from its
    // touch, for the same angle, so the class is the large circle's, three or four steps, which the
    // caller here accepts. Each kind each way round: the line's offset, which about the large
    // circle's centre is its radius times the normal's length, past what a double-double rounds,
    // is taken about the foot of the centre on the line.
    let accept = Accept::Certified {
        pos: Steps(16),
        step: GridStep::Q0,
        gap: ANGULAR,
    };
    let (mut carriers, [big, small]) = table([
        circle(p2(0, 0), 1 << 36),
        circle(p2(1 << 38, 1 << 37), 1_000),
    ]);
    let mut seen = Vec::new();
    for sides in [
        [Sign::Positive, Sign::Positive],
        [Sign::Negative, Sign::Negative],
        [Sign::Positive, Sign::Negative],
        [Sign::Negative, Sign::Positive],
    ] {
        for (one, two) in [(big, small), (small, big)] {
            match common_tangent(&mut carriers, one, two, sides, GridStep::Q0, &accept) {
                Ok(t) => {
                    joined(&carriers, &t.start, t.line, one);
                    joined(&carriers, &t.end, t.line, two);
                    seen.push(format!("{:?} {:#018x}", t.exactness, t.gap.get().to_bits()));
                }
                Err(e) => seen.push(format!("{sides:?} {one:?}: {e:?}")),
            }
        }
    }
    let seen: Vec<&str> = seen.iter().map(String::as_str).collect();
    assert_eq!(seen, WIDE_TANGENTS);
}

/// The classes and gaps of [`a_common_tangent_says_how_far_its_wider_join_may_be`], as read.
const WIDE_TANGENTS: &[&str] = &[
    "Certified { bound: Steps(3), step: Q0 } 0x3dc13bd69f8137ff",
    "Certified { bound: Steps(4), step: Q0 } 0x3dc6548ddc92244e",
    "Certified { bound: Steps(3), step: Q0 } 0x3dc13bd69f8137ff",
    "Certified { bound: Steps(4), step: Q0 } 0x3dc614aa893f22fb",
    "Certified { bound: Steps(4), step: Q0 } 0x3dc7748b703ff7b2",
    "Certified { bound: Steps(4), step: Q0 } 0x3dc6dcc7d5f99846",
    "Certified { bound: Steps(3), step: Q0 } 0x3dc2198386defd1e",
    "Certified { bound: Steps(4), step: Q0 } 0x3dc664231d3c4a22",
];
