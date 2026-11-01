//! Attacks on piece G192: carriers, implicit points and commits (`geom_2d`; chapters 14.2, 14.3 and
//! 6.3 of the kernel architecture). Every test tries to break a claim of the piece's done-when or of
//! the modules' guarantees.
//!
//! - **Every committed point exactly on its carriers**, over random lines at every scale and every
//!   class of crossing: a [`Commit::Grid`] is checked again by the predicates of `geom-num`.
//! - **The rounding** is held to whole-number arithmetic: nearest step, ties to even, at and past the
//!   frame's edge.
//! - **Keeping implicit**: off a carrier, flipping a decision, and a root of a quartic.
//! - **Accepting**: each class against each acceptance, across grid steps, and what an operation
//!   reports when it is refused.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Case counts are a twentieth in debug
//! builds.

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
    clippy::indexing_slicing
)]

use geom_2d::accept::Accept;
use geom_2d::carrier::{Carriers, Circle2, Curve2, CurveIx, Line2};
use geom_2d::commit::{Commit, Decision, Kept, commit, explicit};
use geom_2d::point::{Place, PointError, Pt2};
use geom_num::arc::{Circle2 as GridCircle, Round};
use geom_num::carrier::Rational2;
use geom_num::conic::Ellipse2;
use geom_num::grid::{COORD_MAX, GridStep, P2, Steps};
use geom_num::ladder::{ANGULAR, FIT};
use geom_num::one_root::Root;
use geom_num::real::Real;
use geom_num::{Exactness, NumError, Sign};
use geom_op::{ErrorKind, OpErrorDetail};

const M: i64 = COORD_MAX;

/// The default seed of every randomised test here (G192).
const DEFAULT_SEED: u64 = 0x6719_2000_5eed_0192;

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

    /// A signed integer of at most `bits` bits, its size drawn too, so every scale is reached.
    fn signed(&mut self, bits: u32) -> i64 {
        let size = 1 + (self.next() % u64::from(bits)) as u32;
        let magnitude = (self.next() >> (64 - size)) as i64;
        if self.next() & 1 == 0 {
            magnitude
        } else {
            -magnitude
        }
    }

    fn grid(&mut self) -> P2 {
        let mut c = || self.signed(40).clamp(-M, M);
        P2::new(c(), c()).unwrap()
    }
}

fn cases(full: u64) -> u64 {
    if cfg!(debug_assertions) {
        full / 20
    } else {
        full
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

fn table(curves: &[Curve2]) -> (Carriers, Vec<CurveIx>) {
    let mut carriers = Carriers::new();
    let names = curves.iter().map(|c| carriers.push(*c).unwrap()).collect();
    (carriers, names)
}

/// The grid point a commit gave, if it gave one.
fn at(commit: Result<Commit, PointError>) -> Option<P2> {
    match commit {
        Ok(Commit::Grid(p)) => Some(p),
        _ => None,
    }
}

/// Why a commit kept its point, if it did.
fn why(commit: Result<Commit, PointError>) -> Option<Kept> {
    match commit {
        Ok(Commit::Kept { why, .. }) => Some(why),
        _ => None,
    }
}

/// What a commit of the crossing of `a` and `b` does.
fn committed(curves: &[Curve2; 2], root: Root) -> Result<Commit, PointError> {
    let (carriers, names) = table(curves);
    let point = Pt2::Crossing {
        a: names[0],
        b: names[1],
        root,
    };
    commit(&point, &carriers, &[])
}

#[test]
fn the_table_names_what_it_holds() {
    let mut carriers = Carriers::new();
    assert!(carriers.is_empty());
    let l = line(p2(0, 0), p2(1, 0));
    let a = carriers.push(l).unwrap();
    assert!(matches!(carriers.get(a), Some(Curve2::Line(_))));
    assert_eq!(carriers.len(), 1);
    assert_eq!(a.get(), 0);
    let other = Carriers::new();
    assert!(other.get(a).is_none());
    let point = Pt2::Crossing {
        a,
        b: a,
        root: Root::First,
    };
    assert_eq!(point.place(&other).err(), Some(PointError::NoCarrier(a)));
}

#[test]
fn a_crossing_of_every_kind_of_pair_lies_on_both_its_carriers() {
    let centre = p2(0, 0);
    let round = Curve2::Circle(Circle2::Round(Round::new(p2(5, 0), centre.into())));
    let big = circle(centre, 5);
    let beside = circle(p2(6, 0), 5);
    let beside_round = Curve2::Circle(Circle2::Round(Round::new(p2(11, 0), p2(6, 0).into())));
    let across = line(p2(-9, 1), p2(9, 2));
    let down = line(p2(1, -9), p2(2, 9));
    let flat = Curve2::Ellipse(Ellipse2::new(centre, [5, 0], 3, 5).unwrap());
    for (a, b) in [
        (across, down),
        (across, big),
        (big, across),
        (across, round),
        (round, across),
        (across, flat),
        (flat, across),
        (big, beside),
        (round, beside_round),
        (round, beside),
        (beside, round),
    ] {
        for root in [Root::First, Root::Second] {
            let (carriers, names) = table(&[a, b]);
            let point = Pt2::Crossing {
                a: names[0],
                b: names[1],
                root,
            };
            assert!(point.on(&carriers, names[0]).unwrap(), "{a:?} {b:?}");
            assert!(point.on(&carriers, names[1]).unwrap(), "{a:?} {b:?}");
        }
    }
    // Crossing the circles the two ways round names the same point by the opposite root.
    let (carriers, names) = table(&[round, beside]);
    let first = Pt2::Crossing {
        a: names[0],
        b: names[1],
        root: Root::First,
    };
    let (Place::OneRoot(there), Place::OneRoot(back)) = (
        first.place(&carriers).unwrap(),
        Pt2::Crossing {
            a: names[1],
            b: names[0],
            root: Root::Second,
        }
        .place(&carriers)
        .unwrap(),
    ) else {
        panic!("one-root points");
    };
    assert!(there.same(&back).unwrap());
}

#[test]
fn a_point_is_not_on_a_carrier_it_misses() {
    let (carriers, names) = table(&[
        line(p2(0, 0), p2(10, 0)),
        line(p2(5, -5), p2(5, 5)),
        line(p2(6, -5), p2(6, 5)),
        circle(p2(0, 0), 5),
        circle(p2(0, 0), 6),
    ]);
    let meeting = Pt2::Crossing {
        a: names[0],
        b: names[1],
        root: Root::First,
    };
    assert!(!meeting.on(&carriers, names[2]).unwrap());
    assert!(meeting.on(&carriers, names[3]).unwrap());
    assert!(!meeting.on(&carriers, names[4]).unwrap());
    let grid = Pt2::Grid(p2(5, 0));
    assert!(grid.on(&carriers, names[1]).unwrap());
    let rational = Pt2::Rational(Rational2::new(11, 0, 2).unwrap());
    assert!(!rational.on(&carriers, names[1]).unwrap());
    assert!(rational.on(&carriers, names[0]).unwrap());
}

#[test]
fn pairs_that_make_another_class_or_nothing_are_refused() {
    let e = Curve2::Ellipse(Ellipse2::new(p2(0, 0), [5, 0], 3, 5).unwrap());
    let c = circle(p2(0, 0), 5);
    let (carriers, names) = table(&[e, c, line(p2(9, 0), p2(9, 1))]);
    let crossing = |a: usize, b: usize| Pt2::Crossing {
        a: names[a],
        b: names[b],
        root: Root::First,
    };
    assert_eq!(
        crossing(0, 1).place(&carriers).err(),
        Some(PointError::WrongClass)
    );
    assert_eq!(
        crossing(1, 0).place(&carriers).err(),
        Some(PointError::WrongClass)
    );
    assert_eq!(
        crossing(0, 0).place(&carriers).err(),
        Some(PointError::WrongClass)
    );
    assert_eq!(
        crossing(2, 1).place(&carriers).err(),
        Some(PointError::Num(NumError::Degenerate))
    );
    let conic = |a: usize, b: usize, k: u8| Pt2::ConicRoot {
        a: names[a],
        b: names[b],
        k,
    };
    assert_eq!(
        conic(1, 0, 0).place(&carriers).err(),
        Some(PointError::WrongClass)
    );
    assert_eq!(
        conic(0, 1, 2).place(&carriers).err(),
        Some(PointError::NoSuchRoot(2))
    );
    assert_eq!(
        conic(0, 0, 0).place(&carriers).err(),
        Some(PointError::Num(NumError::Degenerate))
    );
}

#[test]
fn a_root_of_a_quartic_is_on_what_passes_through_it() {
    // The ellipse x²/625 + y²/225 = 1 and the circle about (15, 0) of radius 12 meet at (15, ±12).
    let e = Curve2::Ellipse(Ellipse2::new(p2(0, 0), [25, 0], 3, 5).unwrap());
    let (carriers, names) = table(&[
        e,
        circle(p2(15, 0), 12),
        line(p2(0, 12), p2(1, 12)),
        line(p2(0, 13), p2(1, 13)),
        line(p2(15, 0), p2(15, 1)),
    ]);
    let (below, above) = (
        Pt2::ConicRoot {
            a: names[0],
            b: names[1],
            k: 0,
        },
        Pt2::ConicRoot {
            a: names[0],
            b: names[1],
            k: 1,
        },
    );
    assert!(above.on(&carriers, names[0]).unwrap());
    assert!(above.on(&carriers, names[1]).unwrap());
    assert!(above.on(&carriers, names[2]).unwrap());
    assert!(!below.on(&carriers, names[2]).unwrap());
    assert!(!above.on(&carriers, names[3]).unwrap());
    assert!(above.on(&carriers, names[4]).unwrap());
    assert!(below.on(&carriers, names[4]).unwrap());
    assert_eq!(why(commit(&above, &carriers, &[])), Some(Kept::ConicRoot));
}

#[test]
fn a_point_on_the_grid_commits_and_one_off_it_stays_implicit() {
    // Two lines meeting at a grid point, and at a half step.
    let cross = [line(p2(0, 0), p2(10, 10)), line(p2(0, 10), p2(10, 0))];
    assert_eq!(at(committed(&cross, Root::First)), Some(p2(5, 5)));
    let (carriers, names) = table(&[line(p2(0, 0), p2(10, 11)), line(p2(0, 11), p2(10, 0))]);
    let point = Pt2::Crossing {
        a: names[0],
        b: names[1],
        root: Root::First,
    };
    assert_eq!(
        why(commit(&point, &carriers, &[])),
        Some(Kept::OffCarrier(names[0]))
    );
    // One carrier holds the rounded point and the other does not.
    let (carriers, names) = table(&[line(p2(0, 0), p2(10, 0)), line(p2(5, -1), p2(6, 2))]);
    let point = Pt2::Crossing {
        a: names[0],
        b: names[1],
        root: Root::First,
    };
    assert_eq!(
        why(commit(&point, &carriers, &[])),
        Some(Kept::OffCarrier(names[1]))
    );
    // A line through (3, 4) on the circle of radius 5, and the crossing with no grid form.
    let on_circle = [line(p2(0, 4), p2(1, 4)), circle(p2(0, 0), 5)];
    assert_eq!(at(committed(&on_circle, Root::Second)), Some(p2(3, 4)));
    assert_eq!(at(committed(&on_circle, Root::First)), Some(p2(-3, 4)));
    let off = [line(p2(0, 1), p2(1, 1)), circle(p2(0, 0), 5)];
    assert!(matches!(
        committed(&off, Root::First),
        Ok(Commit::Kept { .. })
    ));
    // Two circles at (3, ±4), and an ellipse's crossing with a line.
    let two = [circle(p2(0, 0), 5), circle(p2(6, 0), 5)];
    assert_eq!(at(committed(&two, Root::First)), Some(p2(3, 4)));
    assert_eq!(at(committed(&two, Root::Second)), Some(p2(3, -4)));
    let e = Curve2::Ellipse(Ellipse2::new(p2(0, 0), [25, 0], 3, 5).unwrap());
    assert_eq!(
        at(committed(&[line(p2(0, 12), p2(1, 12)), e], Root::Second)),
        Some(p2(15, 12))
    );
}

#[test]
fn a_commit_that_would_flip_a_decision_keeps_the_point() {
    // (1/2 + a hair, 0) rounds to (1, 0) or (0, 0): against x = 1 it is to the one side only.
    let (carriers, names) = table(&[line(p2(1, 0), p2(1, 1))]);
    let z = Rational2::new(7, 1, 8).unwrap();
    let point = Pt2::Rational(z);
    let side = carriers.get(names[0]).unwrap().side(&z).unwrap();
    let decision = Decision {
        curve: names[0],
        sign: side,
    };
    assert_eq!(
        why(commit(&point, &carriers, &[decision])),
        Some(Kept::Flips(decision))
    );
    // Asked nothing, it rounds; asked what stays true, it rounds too.
    assert_eq!(at(commit(&point, &carriers, &[])), Some(p2(1, 0)));
    let (far, names) = table(&[line(p2(5, 0), p2(5, 1))]);
    let held = Decision {
        curve: names[0],
        sign: far.get(names[0]).unwrap().side(&z).unwrap(),
    };
    assert_eq!(at(commit(&point, &far, &[held])), Some(p2(1, 0)));
    let nothing = names[0];
    let asked = commit(
        &point,
        &Carriers::new(),
        &[Decision {
            curve: nothing,
            sign: Sign::Zero,
        }],
    );
    assert_eq!(asked.err(), Some(PointError::NoCarrier(nothing)));
}

#[test]
fn every_committed_point_is_exactly_on_its_carriers() {
    let mut rng = Rng::new();
    let (mut grid, mut kept) = (0, 0);
    for _ in 0..cases(4_000) {
        // Lines through a grid point half the time, so that both answers come up.
        let on = rng.grid();
        let (a, b) = (rng.grid(), rng.grid());
        let through = rng.next() & 1 == 0;
        let (p, q) = if through {
            (on, on)
        } else {
            (rng.grid(), rng.grid())
        };
        let (Ok(l), Ok(m)) = (Line2::through(a, p), Line2::through(b, q)) else {
            continue;
        };
        let (carriers, names) = table(&[Curve2::Line(l), Curve2::Line(m)]);
        let point = Pt2::Crossing {
            a: names[0],
            b: names[1],
            root: Root::First,
        };
        match commit(&point, &carriers, &[]) {
            Ok(Commit::Grid(at)) => {
                grid += 1;
                let z: Rational2 = at.into();
                assert_eq!(l.side(&z), Ok(Sign::Zero));
                assert_eq!(m.side(&z), Ok(Sign::Zero));
            }
            Ok(Commit::Kept { point: same, .. }) => {
                kept += 1;
                assert!(matches!(same, Pt2::Crossing { .. }));
            }
            Err(PointError::Num(NumError::Degenerate | NumError::OutOfRange { .. })) => {}
            Err(e) => panic!("{e}"),
        }
    }
    assert!(grid > 0 && kept > 0, "{grid} {kept}");
}

/// The nearest whole number to n / d, d > 0, ties to even.
fn nearest(n: i128, d: i128) -> i128 {
    let (q, r) = (n.div_euclid(d), n.rem_euclid(d));
    match (2 * r).cmp(&d) {
        std::cmp::Ordering::Less => q,
        std::cmp::Ordering::Greater => q + 1,
        std::cmp::Ordering::Equal if q.rem_euclid(2) == 0 => q,
        std::cmp::Ordering::Equal => q + 1,
    }
}

fn rounded(x: i128, y: i128, w: i128) -> Result<(P2, Exactness), PointError> {
    let accept = Accept::Approximate {
        pos: Steps(u64::MAX >> 8),
        step: GridStep::Q0,
    };
    explicit(
        &Pt2::Rational(Rational2::new(x, y, w).unwrap()),
        &Carriers::new(),
        GridStep::Q0,
        &accept,
    )
}

#[test]
fn rounding_is_to_the_nearest_step_and_ties_go_even() {
    for (x, y, w, at) in [
        (1, 3, 2, [0, 2]),
        (-1, -3, 2, [0, -2]),
        (5, 7, 2, [2, 4]),
        (-5, -7, 2, [-2, -4]),
        (1, 2, 3, [0, 1]),
        (-1, -2, 3, [0, -1]),
    ] {
        let (p, _) = rounded(x, y, w).unwrap();
        assert_eq!([p.x.get(), p.y.get()], at, "{x} {y} {w}");
    }
    let mut rng = Rng::new();
    for _ in 0..cases(8_000) {
        let w = 1 + i128::from(rng.signed(30).abs());
        let top = i128::from(M) * w;
        let (x, y) = (
            (i128::from(rng.signed(62))).clamp(-top, top),
            (i128::from(rng.signed(62))).clamp(-top, top),
        );
        let (p, class) = rounded(x, y, w).unwrap();
        assert_eq!(i128::from(p.x.get()), nearest(x, w), "{x} {w}");
        assert_eq!(i128::from(p.y.get()), nearest(y, w), "{y} {w}");
        let exact = x.rem_euclid(w) == 0 && y.rem_euclid(w) == 0;
        assert_eq!(class == Exactness::Exact, exact);
    }
}

#[test]
fn a_coordinate_that_rounds_outside_the_frame_is_refused() {
    let m = i128::from(M);
    // Exactly half a step past the last step rounds to 2^40, the even one, outside; a hair less
    // stays in.
    for (x, y, w) in [
        (2 * m + 1, 0, 2),
        (-(2 * m + 1), 0, 2),
        (0, 2 * m + 1, 2),
        (0, -(2 * m + 1), 2),
        (m + 1, 0, 1),
        (-(m + 1), 0, 1),
    ] {
        let point = Pt2::Rational(Rational2::new(x, y, w).unwrap());
        assert!(
            matches!(
                commit(&point, &Carriers::new(), &[]),
                Err(PointError::Num(NumError::OutOfRange { .. }))
            ),
            "{x} {y} {w}"
        );
    }
    for (x, y, w, at) in [
        (4 * m + 1, 0, 4, [M, 0]),
        (-(4 * m + 1), 0, 4, [-M, 0]),
        (0, 4 * m + 1, 4, [0, M]),
    ] {
        let point = Pt2::Rational(Rational2::new(x, y, w).unwrap());
        assert_eq!(
            self::at(commit(&point, &Carriers::new(), &[])),
            Some(p2(at[0], at[1]))
        );
    }
}

#[test]
fn a_grid_point_commits_as_itself_and_keeps_its_exactness() {
    let (carriers, names) = table(&[line(p2(0, 0), p2(1, 1))]);
    let point = Pt2::Grid(p2(4, 4));
    assert_eq!(at(commit(&point, &carriers, &[])), Some(p2(4, 4)));
    assert_eq!(
        explicit(&point, &carriers, GridStep::Q0, &Accept::Exact),
        Ok((p2(4, 4), Exactness::Exact))
    );
    assert!(point.on(&carriers, names[0]).unwrap());
}

#[test]
fn explicit_output_is_refused_below_what_was_accepted() {
    let (carriers, names) = table(&[line(p2(0, 0), p2(10, 11)), line(p2(0, 11), p2(10, 0))]);
    let point = Pt2::Crossing {
        a: names[0],
        b: names[1],
        root: Root::First,
    };
    let reached = Exactness::Certified {
        bound: Steps(1),
        step: GridStep::Q0,
    };
    assert_eq!(
        explicit(&point, &carriers, GridStep::Q0, &Accept::Exact),
        Err(PointError::ExactnessNotAccepted {
            would_reach: reached,
            accepted: Accept::Exact
        })
    );
    let accept = Accept::Certified {
        pos: FIT,
        step: GridStep::Q0,
        gap: ANGULAR,
    };
    assert_eq!(
        explicit(&point, &carriers, GridStep::Q0, &accept),
        Ok((p2(5, 6), reached))
    );
    // A root of a quartic has no exact form to round.
    let e = Curve2::Ellipse(Ellipse2::new(p2(0, 0), [25, 0], 3, 5).unwrap());
    let (carriers, names) = table(&[e, circle(p2(15, 0), 12)]);
    let root = Pt2::ConicRoot {
        a: names[0],
        b: names[1],
        k: 0,
    };
    assert_eq!(
        explicit(&root, &carriers, GridStep::Q0, &accept),
        Err(PointError::WrongClass)
    );
}

#[test]
fn an_acceptance_admits_its_class_and_what_is_better() {
    let gap = |v: f64| Real::new(v).unwrap();
    let certified = |bound: u64, step: GridStep| Exactness::Certified {
        bound: Steps(bound),
        step,
    };
    let measured = |deviation: u64, step: GridStep| Exactness::Approximate {
        deviation: Steps(deviation),
        step,
    };
    let (q0, q1) = (GridStep::Q0, GridStep::Q1);
    let tight = Accept::Certified {
        pos: Steps(4),
        step: q0,
        gap: gap(1e-3),
    };
    let loose = Accept::Approximate {
        pos: Steps(4),
        step: q0,
    };
    for accept in [Accept::Exact, tight, loose] {
        assert_eq!(
            accept.admit(Exactness::Exact, Real::ZERO),
            Ok(Exactness::Exact)
        );
    }
    let cases = [
        (Accept::Exact, certified(1, q0), 0.0, false),
        (Accept::Exact, measured(0, q0), 0.0, false),
        (tight, certified(4, q0), 1e-3, true),
        (tight, certified(5, q0), 0.0, false),
        (tight, certified(4, q0), 2e-3, false),
        (tight, measured(0, q0), 0.0, false),
        // Each step is 25 times finer: 100 steps of Q1 are 4 of Q0, and 101 are past them.
        (tight, certified(100, q1), 0.0, true),
        (tight, certified(101, q1), 0.0, false),
        (loose, certified(4, q0), 9.0, true),
        (loose, certified(5, q0), 0.0, false),
        (loose, measured(4, q0), 0.0, true),
        (loose, measured(5, q0), 0.0, false),
        (loose, measured(100, q1), 0.0, true),
        (loose, measured(101, q1), 0.0, false),
    ];
    for (accept, reached, angle, admitted) in cases {
        let answer = accept.admit(reached, gap(angle));
        if admitted {
            assert_eq!(answer, Ok(reached), "{accept:?} {reached:?}");
        } else {
            assert_eq!(
                answer,
                Err(PointError::ExactnessNotAccepted {
                    would_reach: reached,
                    accepted: accept
                }),
                "{accept:?} {reached:?}"
            );
        }
    }
    // The most steps of the coarser grid are past the most of the finer one: counts alone would
    // call them equal.
    let (top, wide) = (
        certified(u64::MAX, q0),
        Accept::Certified {
            pos: Steps(u64::MAX),
            step: q1,
            gap: gap(1.0),
        },
    );
    assert!(wide.admit(top, Real::ZERO).is_err());
}

#[test]
fn an_operation_reports_each_failure_by_its_meaning() {
    let kind = |e: PointError| OpErrorDetail::from(e).kind();
    let refused = PointError::ExactnessNotAccepted {
        would_reach: Exactness::Exact,
        accepted: Accept::Exact,
    };
    assert_eq!(kind(refused), ErrorKind::ExactnessNotAccepted);
    let name = table(&[line(p2(0, 0), p2(1, 0))]).1[0];
    for e in [
        PointError::NoCarrier(name),
        PointError::WrongClass,
        PointError::NoSuchRoot(1),
        PointError::Num(NumError::Degenerate),
    ] {
        assert_eq!(kind(e), ErrorKind::Degenerate);
    }
    let outside = PointError::Num(NumError::OutOfRange { value: 1 << 41 });
    assert_eq!(kind(outside), ErrorKind::FrameRangeExceeded);
}

#[test]
fn errors_say_what_they_are() {
    let name = table(&[line(p2(0, 0), p2(1, 0))]).1[0];
    for (error, words) in [
        (PointError::Num(NumError::Degenerate), "degenerate"),
        (PointError::NoCarrier(name), "no carrier at 0"),
        (PointError::WrongClass, "another class"),
        (PointError::NoSuchRoot(3), "no real root at place 3"),
    ] {
        let text = error.to_string().to_lowercase();
        assert!(text.contains(words), "{text}");
    }
}
