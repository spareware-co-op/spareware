//! Attacks on G158: the result types keep their invariants; output order is 12.15's; operand order
//! is by kind, then by defining data; intersect(A, B) equals intersect(B, A) with the roles swapped
//! on 10,000 pairs; and a batch gives the same answers at 1 to 8 threads.
//!
//! The pairs are line segments in a plane, solved exactly in rationals by a stand-in for the
//! methods G163 and on bring: it takes its operands in the order it is given and would give another
//! answer the other way round, so only the canonical order can make the two calls agree.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly, and a test pool of scoped threads"
)]

use geom_curve::{CurveKind, Frame, Line};
use geom_intersect::{
    CanonicalKey, Coincidence, Coverage, CurveContact, CurveEnd, Diag, Geom, IntersectError,
    Intersection, Item, Operand, OverlapRegion, ParamBox, PointContact, StepsInterval, Unresolved,
    pair, pairs, share,
};
use geom_math::Box3;
use geom_num::interval::Interval;
use geom_num::{Exactness, P3, Steps};
use geom_run::{Budget, CancelToken, Exec, Pool, WorkMeter};
use geom_surface::{Plane, Sphere, SurfaceKind};
use geom_test::{Seed, SplitMix64};

/// A test pool: `threads` scoped threads, task i on thread i mod threads, each running its tasks
/// last first, so the tasks run in an order unlike the serial one.
struct Threads(usize);

impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let mut groups: Vec<Vec<&mut (dyn FnMut() + Send)>> =
            (0..self.0).map(|_| Vec::new()).collect();
        for (i, t) in tasks.iter_mut().enumerate() {
            groups[i % self.0].push(&mut **t);
        }
        let run = |group: Vec<&mut (dyn FnMut() + Send)>| {
            for t in group.into_iter().rev() {
                t();
            }
        };
        std::thread::scope(|s| {
            // wasm32-wasip1 has no threads: where none can start, every group runs here, in turn.
            if std::thread::Builder::new().spawn_scoped(s, || {}).is_err() {
                groups.into_iter().for_each(run);
                return;
            }
            for group in groups {
                s.spawn(move || run(group));
            }
        });
    }
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn at(spans: &[(f64, f64)]) -> ParamBox {
    let spans: Vec<Interval> = spans
        .iter()
        .map(|&(lo, hi)| Interval::new(lo, hi).unwrap())
        .collect();
    ParamBox::new(&spans).unwrap()
}

fn space() -> Box3 {
    Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(1, 1, 1).unwrap()).unwrap()
}

fn point(lo: f64) -> Item {
    Item::Point {
        at: at(&[(lo, lo + 0.5), (0.0, 1.0)]),
        xyz: space(),
        contact: PointContact::Crossing,
        class: Exactness::Exact,
    }
}

fn flagged(lo: f64) -> Item {
    Item::Flagged {
        at: at(&[(lo, lo + 0.5), (0.0, 1.0)]),
        xyz: space(),
        why: Unresolved::HigherOrderContact,
    }
}

fn line(a: [f64; 3], b: [f64; 3]) -> CurveKind<3> {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    CurveKind::Line(Line::new(a, d, (0.0, 1.0)).unwrap())
}

fn operand(curve: &CurveKind<3>) -> Operand<'_> {
    Operand {
        geom: Geom::Curve(curve),
        domain: None,
        bound: Steps(0),
        key: CanonicalKey::of_curve(curve, &mut meter()).unwrap(),
    }
}

fn answer(items: Vec<Item>) -> Result<Intersection, IntersectError> {
    Intersection::new(
        items,
        Coverage::CompleteOutsideFlagged,
        Vec::new(),
        &mut meter(),
    )
}

/// The ends of a segment in the plane z = 0, in the order the kind holds them.
fn ends(geom: &Geom<'_>) -> [[f64; 2]; 2] {
    let Geom::Curve(CurveKind::Line(l)) = geom else {
        panic!("a segment");
    };
    let ([ox, oy, _], [dx, dy, _]) = (l.origin(), l.direction());
    [[ox, oy], [ox + dx, oy + dy]]
}

/// The crossing of two segments as its two parameters, with a diagnostic naming the first operand:
/// so an answer shows which way round it was computed.
fn solve_with(
    a: &Operand<'_>,
    b: &Operand<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Intersection, IntersectError> {
    let ([p, q], [r, s]) = (ends(&a.geom), ends(&b.geom));
    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
    let (ex, ey) = (s[0] - r[0], s[1] - r[1]);
    let det = dx * ey - dy * ex;
    let code = format!("first {:.0}", p[0]);
    let diagnostics = vec![Diag { code }];
    if det == 0.0 {
        return Intersection::new(Vec::new(), Coverage::TopologyCertified, diagnostics, meter);
    }
    let t = ((r[0] - p[0]) * ey - (r[1] - p[1]) * ex) / det;
    let u = ((r[0] - p[0]) * dy - (r[1] - p[1]) * dx) / det;
    let items = if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        vec![Item::Point {
            at: at(&[(t, t), (u, u)]),
            xyz: space(),
            contact: PointContact::Crossing,
            class: Exactness::Exact,
        }]
    } else {
        Vec::new()
    };
    Intersection::new(items, Coverage::TopologyCertified, diagnostics, meter)
}

/// [`solve_with`], on a meter of its own.
fn solve(a: &Operand<'_>, b: &Operand<'_>) -> Result<Intersection, IntersectError> {
    solve_with(a, b, &mut meter())
}

#[test]
fn a_param_box_has_two_to_four_unknowns() {
    let one = Interval::point(0.5).unwrap();
    for (n, ok) in [
        (0, false),
        (1, false),
        (2, true),
        (3, true),
        (4, true),
        (5, false),
    ] {
        let spans = vec![one; n];
        match ParamBox::new(&spans) {
            Ok(b) => {
                assert!(ok, "{n}");
                assert_eq!(b.spans().len(), n);
                assert_eq!(usize::from(b.unknowns()), n);
            }
            Err(e) => {
                assert!(!ok, "{n}");
                assert_eq!(e, IntersectError::InvalidInput);
            }
        }
    }
}

#[test]
fn a_range_of_steps_runs_upward() {
    let range = StepsInterval::new(Steps(2), Steps(5)).unwrap();
    assert_eq!((range.lo(), range.hi()), (Steps(2), Steps(5)));
    assert!(StepsInterval::new(Steps(3), Steps(3)).is_some());
    assert!(StepsInterval::new(Steps(4), Steps(3)).is_none());
}

#[test]
fn items_sort_by_kind_then_by_enclosure() {
    let c = line([0.0; 3], [1.0, 0.0, 0.0]);
    let curve = Item::Curve {
        at: at(&[(0.0, 0.25), (0.0, 1.0)]),
        curve: Box::new(c.clone()),
        contact: CurveContact::Crossing,
        ends: [CurveEnd::Boundary, CurveEnd::Closed],
        class: Exactness::Exact,
    };
    let overlap = Item::Overlap {
        at: at(&[(0.0, 0.25), (0.0, 1.0)]),
        region: OverlapRegion { boundary: vec![c] },
        proof: Coincidence::Identical,
        class: Exactness::Exact,
    };
    let done = answer(vec![
        flagged(4.0),
        overlap.clone(),
        point(3.0),
        curve.clone(),
        flagged(1.0),
        point(1.0),
    ])
    .unwrap();
    let want = vec![
        point(1.0),
        point(3.0),
        curve,
        overlap,
        flagged(1.0),
        flagged(4.0),
    ];
    assert_eq!(done.items(), want);
}

#[test]
fn the_order_compares_upper_corners_when_lower_ones_tie() {
    // Two boxes with one lower corner would meet, so a tie reaches the upper corner only across
    // kinds, which rank first: the order must still be total and independent of the input's order.
    let a = at(&[(0.0, 1.0), (2.0, 3.0)]);
    let b = at(&[(0.0, 1.0), (4.0, 5.0)]);
    let items = |x: ParamBox, y: ParamBox| {
        vec![
            Item::Point {
                at: x,
                xyz: space(),
                contact: PointContact::Crossing,
                class: Exactness::Exact,
            },
            Item::Point {
                at: y,
                xyz: space(),
                contact: PointContact::BoundaryTouch,
                class: Exactness::Exact,
            },
        ]
    };
    let one = answer(items(a, b)).unwrap();
    let two = answer(items(b, a)).unwrap();
    assert_eq!(one.items()[0].at(), &a);
    assert_eq!(two.items()[0].at(), &a);
}

#[test]
fn enclosures_of_one_kind_may_not_meet() {
    // Touching at a corner is meeting; a gap in one unknown is not.
    assert_eq!(
        answer(vec![point(1.0), point(1.5)]),
        Err(IntersectError::Internal)
    );
    assert!(answer(vec![point(1.0), point(1.75)]).is_ok());
    // Across kinds it is allowed: a flagged region holds what could not be split from a point.
    assert!(answer(vec![point(1.0), flagged(1.0)]).is_ok());
}

/// Two boxes that overlap with a third sorted between them, which meets neither: comparing
/// neighbours alone would pass this.
#[test]
fn an_overlap_is_found_across_an_item_between() {
    let item = |spans: &[(f64, f64)]| Item::Point {
        at: at(spans),
        xyz: space(),
        contact: PointContact::Crossing,
        class: Exactness::Exact,
    };
    let (a, b, c) = (
        item(&[(0.0, 5.0), (0.0, 0.5)]),
        item(&[(1.0, 1.5), (5.0, 6.0)]),
        item(&[(2.0, 3.0), (0.25, 0.75)]),
    );
    assert_eq!(answer(vec![a, b, c]), Err(IntersectError::Internal));
    // Move the last one off in the second unknown and all three are disjoint.
    let (a, b, c) = (
        item(&[(0.0, 5.0), (0.0, 0.5)]),
        item(&[(1.0, 1.5), (5.0, 6.0)]),
        item(&[(2.0, 3.0), (0.75, 1.0)]),
    );
    assert!(answer(vec![a, b, c]).is_ok());
    // A box ending exactly where a later one starts still meets it.
    let touching = vec![
        item(&[(0.0, 1.0), (0.0, 1.0)]),
        item(&[(1.0, 2.0), (1.0, 2.0)]),
    ];
    assert_eq!(answer(touching), Err(IntersectError::Internal));
}

#[test]
fn the_sweep_stops_where_the_first_unknown_ends() {
    // Nine disjoint items, each beyond the last: one charge per item, and no comparison at all.
    let items: Vec<Item> = (0..9).map(|i| point(f64::from(i))).collect();
    let mut m = meter();
    Intersection::new(items, Coverage::CompleteOutsideFlagged, Vec::new(), &mut m).unwrap();
    assert_eq!(m.used().work, 9);
    // Stacked in the second unknown: each meets the rest in the first, so all are compared.
    let stacked = |n: u32| -> Vec<Item> {
        (0..n)
            .map(|i| Item::Point {
                at: at(&[(0.0, 1.0), (f64::from(i), f64::from(i) + 0.5)]),
                xyz: space(),
                contact: PointContact::Crossing,
                class: Exactness::Exact,
            })
            .collect()
    };
    let mut m = meter();
    Intersection::new(
        stacked(5),
        Coverage::CompleteOutsideFlagged,
        Vec::new(),
        &mut m,
    )
    .unwrap();
    assert_eq!(m.used().work, 5 + 10);
    // The budget stops it, at the sort or at the sweep, with nothing returned.
    for work in [0, 5, 9] {
        let mut m = WorkMeter::new(Budget::new(work, 0));
        let stopped = Intersection::new(
            stacked(5),
            Coverage::CompleteOutsideFlagged,
            Vec::new(),
            &mut m,
        );
        assert_eq!(stopped, Err(IntersectError::BudgetExceeded), "{work}");
    }
}

#[test]
fn every_item_has_the_pairs_unknowns() {
    let three = Item::Point {
        at: at(&[(9.0, 9.5), (0.0, 1.0), (0.0, 1.0)]),
        xyz: space(),
        contact: PointContact::Crossing,
        class: Exactness::Exact,
    };
    assert_eq!(
        answer(vec![point(1.0), three]),
        Err(IntersectError::Internal)
    );
}

#[test]
fn a_certified_topology_has_nothing_flagged() {
    let certified =
        |items| Intersection::new(items, Coverage::TopologyCertified, Vec::new(), &mut meter());
    assert_eq!(certified(vec![flagged(1.0)]), Err(IntersectError::Internal));
    let done = certified(vec![point(1.0)]).unwrap();
    assert_eq!(done.coverage(), Coverage::TopologyCertified);
    assert!(answer(vec![flagged(1.0)]).is_ok());
}

#[test]
fn an_answer_gives_back_what_it_was_given() {
    let diagnostics = vec![Diag {
        code: "tie".to_owned(),
    }];
    let mut m = meter();
    m.charge(7).unwrap();
    m.reserve(9).unwrap();
    let done = Intersection::new(
        Vec::new(),
        Coverage::CompleteOutsideFlagged,
        diagnostics.clone(),
        &mut m,
    )
    .unwrap();
    assert_eq!(done.diagnostics(), diagnostics);
    assert_eq!(done.work(), m.used());
    assert_eq!((done.work().work, done.work().logical_bytes), (7, 9));
    assert_eq!(done.coverage(), Coverage::CompleteOutsideFlagged);
    assert_eq!(done.items().len(), 0);
}

#[test]
fn swapping_moves_the_first_operands_unknowns_behind() {
    // A curve with a surface: t, then u and v.
    let item = |spans: &[(f64, f64)]| Item::Point {
        at: at(spans),
        xyz: space(),
        contact: PointContact::Crossing,
        class: Exactness::Exact,
    };
    let done = answer(vec![item(&[(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)])]).unwrap();
    let back = done.clone().swapped(1);
    assert_eq!(back.items(), [item(&[(2.0, 2.0), (3.0, 3.0), (1.0, 1.0)])]);
    // Back again: the other operand brought two.
    assert_eq!(back.swapped(2), done);
    // Every kind of item moves.
    let c = line([0.0; 3], [1.0, 0.0, 0.0]);
    let spans = [(1.0, 1.0), (2.0, 2.0)];
    let turned = [(2.0, 2.0), (1.0, 1.0)];
    let items = |s: &[(f64, f64)]| {
        vec![
            Item::Curve {
                at: at(s),
                curve: Box::new(c.clone()),
                contact: CurveContact::Crossing,
                ends: [CurveEnd::Boundary; 2],
                class: Exactness::Exact,
            },
            Item::Overlap {
                at: at(s),
                region: OverlapRegion {
                    boundary: Vec::new(),
                },
                proof: Coincidence::Within(Steps(1)),
                class: Exactness::Exact,
            },
            Item::Flagged {
                at: at(s),
                xyz: space(),
                why: Unresolved::KnownCurveUnproven,
            },
        ]
    };
    let swapped = answer(items(&spans)).unwrap().swapped(1);
    assert_eq!(swapped.items(), items(&turned));
}

fn plane(z: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, z],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
}

fn ball(radius: f64) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            radius,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    )
}

#[test]
fn keys_order_by_kind_then_by_defining_data() {
    let mut m = meter();
    let of_c = |c: &CurveKind<3>, m: &mut WorkMeter<'_>| CanonicalKey::of_curve(c, m).unwrap();
    let of_s = |s: &SurfaceKind, m: &mut WorkMeter<'_>| CanonicalKey::of_surface(s, m).unwrap();
    let (l1, l2) = (
        line([0.0; 3], [1.0, 0.0, 0.0]),
        line([0.0; 3], [2.0, 0.0, 0.0]),
    );
    // One curve, one key; another curve, another.
    assert_eq!(of_c(&l1, &mut m), of_c(&l1.clone(), &mut m));
    assert_ne!(of_c(&l1, &mut m), of_c(&l2, &mut m));
    // Every curve before every surface; a plane before a sphere; a field after every surface.
    assert!(of_c(&l1, &mut m) < of_s(&plane(0.0), &mut m));
    assert!(of_s(&plane(9.0), &mut m) < of_s(&ball(1.0), &mut m));
    assert!(of_s(&ball(9.0), &mut m) < CanonicalKey::of_implicit(0, Vec::new()));
    assert!(CanonicalKey::of_implicit(0, vec![1]) < CanonicalKey::of_implicit(0, vec![2]));
    // Within a kind, by the data.
    assert_ne!(of_s(&plane(1.0), &mut m), of_s(&plane(2.0), &mut m));
    assert_ne!(of_s(&ball(1.0), &mut m), of_s(&ball(2.0), &mut m));
    // Curves other than lines go through their NURBS form, charged to the meter.
    let circle = CurveKind::Circle(
        geom_curve::Circle::full([0.0; 3], 2.0, Frame::standard().unwrap()).unwrap(),
    );
    let before = m.used().work;
    let key = of_c(&circle, &mut m);
    assert!(m.used().work >= before);
    assert!(key > of_c(&l1, &mut m));
    assert_ne!(key, of_c(&line([1.0; 3], [2.0; 3]), &mut m));
}

#[test]
fn a_stopped_meter_stops_a_key() {
    let circle = CurveKind::Circle(
        geom_curve::Circle::full([0.0; 3], 2.0, Frame::standard().unwrap()).unwrap(),
    );
    let mut m = WorkMeter::new(Budget::new(0, 0));
    let stopped = m.charge(1).unwrap_err();
    assert_eq!(
        IntersectError::from(stopped),
        IntersectError::BudgetExceeded
    );
    let e = CanonicalKey::of_curve(&circle, &mut m);
    assert!(matches!(e, Err(IntersectError::BudgetExceeded)), "{e:?}");
    let token = CancelToken::new();
    token.cancel();
    let mut m = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
    assert_eq!(
        CanonicalKey::of_surface(&ball(1.0), &mut m).unwrap_err(),
        IntersectError::Cancelled
    );
}

#[test]
fn an_operand_counts_its_unknowns() {
    let c = line([0.0; 3], [1.0, 0.0, 0.0]);
    let s = plane(0.0);
    let flat = geom_num::plane::Plane::through(
        P3::new(0, 0, 0).unwrap(),
        P3::new(1, 0, 0).unwrap(),
        P3::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    assert_eq!(Geom::Curve(&c).unknowns(), 1);
    assert_eq!(Geom::Surface(&s).unknowns(), 2);
    assert_eq!(Geom::Implicit(&flat).unknowns(), 0);
}

/// Random segments in the plane, as whole numbers, so their crossings are exact in doubles'
/// comparison of signs.
fn segments(rng: &mut SplitMix64) -> CurveKind<3> {
    let mut c = || rng.range_i64(-1000, 1000).unwrap() as f64;
    let (a, b, x, y) = (c(), c(), c(), c());
    if (a, b) == (x, y) {
        line([a, b, 0.0], [x + 1.0, y, 0.0])
    } else {
        line([a, b, 0.0], [x, y, 0.0])
    }
}

#[test]
fn intersect_a_b_is_intersect_b_a_with_the_roles_swapped_on_10000_pairs() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("g158 pairs", Seed::DEFAULT));
    let mut crossed = 0;
    for _ in 0..10_000 {
        let (a, b) = (segments(&mut rng), segments(&mut rng));
        let (x, y) = (operand(&a), operand(&b));
        let one = pair(&x, &y, solve).unwrap();
        let two = pair(&y, &x, solve).unwrap();
        // The stand-in saw one order both times: the diagnostics say so.
        assert_eq!(one.diagnostics(), two.diagnostics());
        let unknowns = if x.key <= y.key {
            y.geom.unknowns()
        } else {
            x.geom.unknowns()
        };
        assert_eq!(one.clone().swapped(unknowns), two, "{a:?} {b:?}");
        crossed += one.items().len();
    }
    assert!(crossed > 1000, "{crossed}");
}

#[test]
fn a_batch_gives_the_same_answers_at_1_to_8_threads() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("g158 batch", Seed::DEFAULT));
    let curves: Vec<(CurveKind<3>, CurveKind<3>)> = (0..400)
        .map(|_| (segments(&mut rng), segments(&mut rng)))
        .collect();
    let list: Vec<(Operand<'_>, Operand<'_>)> = curves
        .iter()
        .map(|(a, b)| (operand(a), operand(b)))
        .collect();
    let run = |exec: &Exec<'_>| pairs(exec, Budget::UNLIMITED, None, &list, solve_with).unwrap();
    let serial = run(&Exec::serial());
    for threads in 1..=8 {
        let pool = Threads(threads);
        assert_eq!(run(&Exec::with_pool(&pool)), serial, "{threads}");
    }
    // In the caller's order: each answer is the one its pair gives alone.
    for (result, (a, b)) in serial.iter().zip(&list) {
        assert_eq!(result.as_ref().unwrap(), &pair(a, b, solve).unwrap());
    }
}

#[test]
fn every_pair_gets_an_equal_share_and_one_failure_stays_its_own() {
    assert_eq!(share(Budget::new(100, 40), 4), Budget::new(25, 10));
    assert_eq!(share(Budget::new(100, 40), 0), Budget::new(100, 40));
    let (a, b) = (
        line([0.0; 3], [1.0, 0.0, 0.0]),
        line([0.0; 3], [0.0, 1.0, 0.0]),
    );
    let list = vec![(operand(&a), operand(&b)), (operand(&b), operand(&a))];
    let spend = |_: &Operand<'_>, _: &Operand<'_>, m: &mut WorkMeter<'_>| {
        m.charge(30)?;
        Intersection::new(Vec::new(), Coverage::CompleteOutsideFlagged, Vec::new(), m)
    };
    // 60 in all is enough only if each pair may spend its 30.
    let done = pairs(&Exec::serial(), Budget::new(60, 60), None, &list, spend).unwrap();
    assert!(done.iter().all(Result::is_ok));
    let short = pairs(&Exec::serial(), Budget::new(58, 58), None, &list, spend).unwrap();
    assert!(short.iter().all(Result::is_err));
    // A cancelled batch answers nothing.
    let token = CancelToken::new();
    token.cancel();
    let stopped = pairs(
        &Exec::serial(),
        Budget::UNLIMITED,
        Some(&token),
        &list,
        spend,
    );
    assert_eq!(stopped.unwrap_err(), IntersectError::Cancelled);
}

#[test]
fn errors_keep_their_causes() {
    use geom_curve::CurveError;
    use geom_num::NumError;
    use geom_run::Interrupted;
    assert_eq!(
        IntersectError::from(Interrupted::Cancelled),
        IntersectError::Cancelled
    );
    assert_eq!(
        IntersectError::from(Interrupted::OutOfMemory),
        IntersectError::BudgetExceeded
    );
    assert_eq!(
        IntersectError::from(Interrupted::Internal("x")),
        IntersectError::Internal
    );
    assert_eq!(
        IntersectError::from(NumError::UnprovenMaths),
        IntersectError::UnprovenMaths
    );
    assert_eq!(
        IntersectError::from(NumError::OutOfRange { value: 1 }),
        IntersectError::FrameRange
    );
    assert_eq!(
        IntersectError::from(NumError::Degenerate),
        IntersectError::InvalidInput
    );
    assert_eq!(
        IntersectError::from(CurveError::Defect),
        IntersectError::Internal
    );
    assert_eq!(
        IntersectError::from(CurveError::ParameterOutside),
        IntersectError::InvalidInput
    );
    let mut m = WorkMeter::new(Budget::new(0, 0));
    let stopped = m.charge(1).unwrap_err();
    assert_eq!(
        IntersectError::from(CurveError::Stopped(stopped)),
        IntersectError::BudgetExceeded
    );
    for (e, words) in [
        (IntersectError::Cancelled, "cancelled"),
        (IntersectError::BudgetExceeded, "budget"),
        (IntersectError::FrameRange, "range"),
        (IntersectError::UnprovenMaths, "maths"),
        (IntersectError::InvalidInput, "precondition"),
        (IntersectError::Internal, "invariant"),
    ] {
        assert!(e.to_string().contains(words), "{e}");
    }
}
