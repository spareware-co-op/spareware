//! The law catalogue's intersections row (chapter 28.3; piece G180): a plane with every natural
//! quadric in every position, through `geom_intersect::order::pair`'s seam.
//!
//! - **Symmetric in its arguments.** The pair in either order gives one answer, roles swapped.
//! - **Every returned point lies on both inputs within the bound.** Each point, and a dense sample
//!   of each curve, is refuted against both operands' definitions in 400-bit fixed point, each
//!   distance a closed form written here that shares nothing with `geom-intersect`.
//! - **A plane with a quadric gives the exact conic of its closed form.** Which conic it is comes
//!   from the plane and the quadric in exact rationals: the quadric's form restricted to the plane
//!   and its discriminants. The answer must be that kind itself, never a wrapped one, the whole
//!   conic: a closed curve one full turn, an open one running to the patch's edge or its apex.
//!
//! The fourth law of the row, completeness as section 12 promises it, is scored on the exact-truth
//! SSI corpus by `ssi gate` (`geom_test::ssi::gate`), nightly in `ssi-acceptance`.
//!
//! The method these laws are for is G165's closed form, `geom_intersect::quadrics::natural`, which
//! `the_laws_hold` runs on every case beside `stand_in`, each case's stated closed form, which holds
//! the table and the laws to each other. `each_law_catches_its_planted_fault` shows each law
//! refuting the wrong answer it exists for. A method joins by one more call in `the_laws_hold`:
//! everything the laws read is `pair`'s. G170's marching joins when it answers these pairs.
//!
//! Each law lists every broken case it finds. Seeds: each law prints `seed: <hex>` and replays with
//! `SPAREKIT_SEED`; pull requests run the fixed default seed, and the nightly workflow a fresh one.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::large_enum_variant,
    clippy::similar_names,
    clippy::unreadable_literal,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    reason = "tests: oracles and generators written as the formulas read, and a failure stops the test"
)]

// Piece G63's transitions and their truth, which the shared kinds module draws on.
#[allow(
    dead_code,
    unreachable_pub,
    reason = "the catalogue uses the transitions' truth only"
)]
#[path = "../../geom-curve/tests/laws/mod.rs"]
mod laws;
#[allow(
    dead_code,
    unreachable_pub,
    reason = "the catalogue uses the transitions' truth only"
)]
#[path = "../../geom-curve/tests/series/mod.rs"]
mod series;
mod truth;

use core::cell::Cell;
use core::cmp::Ordering;

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_curve::{Axes, Circle, CurveKind, Ellipse, Frame, Hyperbola, Line, Parabola, ParamCurve};
use geom_intersect::{
    CanonicalKey, Coverage, CurveContact, CurveEnd, Geom, IntersectError, Intersection, Item,
    Operand, ParamBox, PointContact, StepsInterval, pair, quadrics,
};
use geom_math::Box3;
use geom_num::interval::Interval;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_nurbs::Limit;
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind};
use geom_test::fixed;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use truth::{Findings, Rng, fx, meter, scale};

/// The bound the stand-in states, in steps.
const STATED: u64 = 1;

/// What a double's rounding may add to a sampled distance, in steps: far under one step, on
/// coordinates under 2^24.
const SLACK: f64 = 1.0 / (1u64 << 20) as f64;

/// A grid symmetry, a scale and a whole shift: what a case is drawn under.
struct Move {
    rows: [[f64; 3]; 3],
    shift: [f64; 3],
    scale: f64,
}

impl Move {
    fn draw(rng: &mut Rng) -> Move {
        let mut order = [0, 1, 2];
        for i in [2, 1] {
            order.swap(i, rng.below(i as u64 + 1) as usize);
        }
        let mut rows = [[0.0; 3]; 3];
        for i in 0..3 {
            rows[i][order[i]] = if rng.below(2) == 0 { 1.0 } else { -1.0 };
        }
        let shift = [0; 3].map(|_: i32| rng.below(1 << 20) as f64 - (1u64 << 19) as f64);
        let scale = (1u64 << (8 + rng.below(6))) as f64;
        Move { rows, shift, scale }
    }

    fn turn(&self, x: [f64; 3]) -> [f64; 3] {
        self.rows.map(|r| r[0] * x[0] + r[1] * x[1] + r[2] * x[2])
    }

    fn vector(&self, x: [f64; 3]) -> [f64; 3] {
        self.turn(x).map(|v| v * self.scale)
    }

    fn point(&self, x: [f64; 3]) -> [f64; 3] {
        let v = self.vector(x);
        [
            v[0] + self.shift[0],
            v[1] + self.shift[1],
            v[2] + self.shift[2],
        ]
    }
}

/// What a planted fault changes in the table's answers.
#[derive(Clone, Copy)]
struct Plant {
    /// Moves every answer off the true one, by this many steps.
    drift: f64,
    /// How much of a closed curve's turn is returned.
    turn: f64,
    /// Answers a circle as an ellipse.
    wrapped: bool,
    /// Leaves out the last part of each answer.
    dropped: bool,
}

const TRUE: Plant = Plant {
    drift: 0.0,
    turn: 2.0,
    wrapped: false,
    dropped: false,
};

/// One part of a stated answer.
#[derive(Clone)]
enum Part {
    Point([f64; 3], PointContact),
    Curve(CurveKind<3>, CurveContact, [CurveEnd; 2]),
}

/// A plane, a quadric and the answer stated for them.
struct Case {
    name: &'static str,
    plane: SurfaceKind,
    quadric: SurfaceKind,
    answer: Vec<Part>,
}

fn tangent() -> CurveContact {
    CurveContact::Tangent {
        band: Steps(1),
        gap: StepsInterval::new(Steps(0), Steps(0)).unwrap(),
    }
}

/// The table: a plane with a sphere, a cylinder and a cone in each position there is. Every answer
/// is the elementary closed form, and `plane_and_quadric_classify_as_stated` holds each to its exact
/// classification. The cone is one nappe from its apex at the centre, 3 out for every 4 up.
fn cases(mv: &Move, plant: Plant) -> Vec<Case> {
    let s = mv.scale;
    let off = mv.turn([0.2672612419124244, 0.5345224838248488, 0.8017837257372732]);
    let at = |x: [f64; 3]| {
        let p = mv.point(x);
        [
            p[0] + plant.drift * off[0],
            p[1] + plant.drift * off[1],
            p[2] + plant.drift * off[2],
        ]
    };
    let frame = |x: [f64; 3], n: [f64; 3]| Frame::new(mv.turn(x), mv.turn(n)).unwrap();
    let axial = || frame([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let plane = |o: [f64; 3], a: [f64; 3], b: [f64; 3]| {
        let far = 4096.0 * s;
        SurfaceKind::Plane(
            Plane::new(
                mv.point(o),
                mv.turn(a),
                mv.turn(b),
                ((-far, far), (-far, far)),
            )
            .unwrap(),
        )
    };
    let center = mv.point([0.0; 3]);
    let sphere = || {
        SurfaceKind::Sphere(
            Sphere::new(center, 5.0 * s, axial(), ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
        )
    };
    let cylinder = || {
        SurfaceKind::Cylinder(
            Cylinder::new(
                center,
                5.0 * s,
                axial(),
                ((0.0, 2.0), (-40.0 * s, 40.0 * s)),
            )
            .unwrap(),
        )
    };
    let cone = || {
        SurfaceKind::Cone(
            Cone::new(center, 0.0, 0.75, axial(), ((0.0, 2.0), (0.0, 16.0 * s))).unwrap(),
        )
    };
    let closed = if plant.turn == 2.0 {
        [CurveEnd::Closed; 2]
    } else {
        [CurveEnd::Boundary; 2]
    };
    let turn = (0.0, plant.turn);
    let circle = |c: [f64; 3], r: f64, x: [f64; 3], n: [f64; 3]| {
        let curve = if plant.wrapped {
            let axes = Axes::Frame {
                frame: frame(x, n),
                a: r * s,
                b: r * s,
            };
            CurveKind::Ellipse(Ellipse::new(at(c), axes, turn).unwrap())
        } else {
            CurveKind::Circle(Circle::new(at(c), r * s, frame(x, n), turn).unwrap())
        };
        Part::Curve(curve, CurveContact::Crossing, closed)
    };
    let ellipse = |c: [f64; 3], u: [f64; 3], v: [f64; 3]| {
        let axes = Axes::Vectors {
            u: mv.vector(u),
            v: mv.vector(v),
        };
        let curve = CurveKind::Ellipse(Ellipse::new(at(c), axes, turn).unwrap());
        Part::Curve(curve, CurveContact::Crossing, closed)
    };
    let line = |o: [f64; 3], d: [f64; 3], domain: (f64, f64), contact, ends| {
        let curve = CurveKind::Line(Line::new(at(o), mv.vector(d), domain).unwrap());
        Part::Curve(curve, contact, ends)
    };
    let through = [CurveEnd::Boundary; 2];
    let from_apex = [CurveEnd::Singular, CurveEnd::Boundary];
    let (x, y, z) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    // In the plane z = 8 + y/2, the conic is x² + (55/64)(y − y₀)² = R.
    let y0 = 288.0 / 110.0;
    let r2 = 36.0 + 55.0 / 64.0 * y0 * y0;
    let (a_x, a_y) = (f64::sqrt(r2), f64::sqrt(r2 * 64.0 / 55.0));
    let reach = f64::acosh(4.0);
    let wide = f64::sqrt(63.0);
    let mut table = vec![
        Case {
            name: "sphere, a parallel plane through it",
            plane: plane([0.0, 0.0, 3.0], x, y),
            quadric: sphere(),
            answer: vec![circle([0.0, 0.0, 3.0], 4.0, x, z)],
        },
        Case {
            name: "sphere, a great circle on a tilted plane",
            plane: plane([0.0; 3], x, [0.0, 4.0, -3.0]),
            quadric: sphere(),
            answer: vec![circle([0.0; 3], 5.0, x, [0.0, 3.0, 4.0])],
        },
        Case {
            name: "sphere, a tangent plane",
            plane: plane([0.0, 0.0, 5.0], x, y),
            quadric: sphere(),
            answer: vec![Part::Point(
                at([0.0, 0.0, 5.0]),
                PointContact::IsolatedTangent,
            )],
        },
        Case {
            name: "sphere, a plane that misses",
            plane: plane([0.0, 0.0, 6.0], x, y),
            quadric: sphere(),
            answer: vec![],
        },
        Case {
            name: "cylinder, a plane across its axis",
            plane: plane([0.0, 0.0, 2.0], x, y),
            quadric: cylinder(),
            answer: vec![circle([0.0, 0.0, 2.0], 5.0, x, z)],
        },
        Case {
            name: "cylinder, a tilted plane",
            plane: plane([0.0; 3], x, [0.0, 4.0, -3.0]),
            quadric: cylinder(),
            answer: vec![ellipse([0.0; 3], [5.0, 0.0, 0.0], [0.0, 5.0, -3.75])],
        },
        Case {
            name: "cylinder, a plane along its axis through it",
            plane: plane([0.0, 3.0, 0.0], x, z),
            quadric: cylinder(),
            answer: vec![
                line(
                    [4.0, 3.0, 0.0],
                    z,
                    (-40.0, 40.0),
                    CurveContact::Crossing,
                    through,
                ),
                line(
                    [-4.0, 3.0, 0.0],
                    z,
                    (-40.0, 40.0),
                    CurveContact::Crossing,
                    through,
                ),
            ],
        },
        Case {
            name: "cylinder, a tangent plane",
            plane: plane([0.0, 5.0, 0.0], x, z),
            quadric: cylinder(),
            answer: vec![line([0.0, 5.0, 0.0], z, (-40.0, 40.0), tangent(), through)],
        },
        Case {
            name: "cylinder, a plane along its axis that misses",
            plane: plane([0.0, 6.0, 0.0], x, z),
            quadric: cylinder(),
            answer: vec![],
        },
        Case {
            name: "cone, a plane across its axis",
            plane: plane([0.0, 0.0, 8.0], x, y),
            quadric: cone(),
            answer: vec![circle([0.0, 0.0, 8.0], 6.0, x, z)],
        },
        Case {
            name: "cone, a shallow tilted plane",
            plane: plane([0.0, 0.0, 8.0], x, [0.0, 2.0, 1.0]),
            quadric: cone(),
            answer: vec![ellipse(
                [0.0, y0, 8.0 + y0 / 2.0],
                [a_x, 0.0, 0.0],
                [0.0, a_y, a_y / 2.0],
            )],
        },
        Case {
            name: "cone, a plane along a generator",
            plane: plane([0.0, -3.0, 0.0], x, [0.0, 3.0, 4.0]),
            quadric: cone(),
            answer: vec![Part::Curve(
                CurveKind::Parabola(
                    Parabola::new(
                        at([0.0, -1.5, 2.0]),
                        Axes::Vectors {
                            u: mv.vector([0.0, 1.0 / 6.0, 2.0 / 9.0]),
                            v: mv.vector(x),
                        },
                        (-wide, wide),
                    )
                    .unwrap(),
                ),
                CurveContact::Crossing,
                through,
            )],
        },
        Case {
            name: "cone, a plane along its axis off the apex",
            plane: plane([0.0, 3.0, 0.0], x, z),
            quadric: cone(),
            answer: vec![Part::Curve(
                CurveKind::Hyperbola(
                    Hyperbola::new(
                        at([0.0, 3.0, 0.0]),
                        Axes::Vectors {
                            u: mv.vector([0.0, 0.0, 4.0]),
                            v: mv.vector([3.0, 0.0, 0.0]),
                        },
                        (-reach, reach),
                    )
                    .unwrap(),
                ),
                CurveContact::Crossing,
                through,
            )],
        },
        Case {
            name: "cone, a plane through its axis",
            plane: plane([0.0; 3], x, z),
            quadric: cone(),
            answer: vec![
                line(
                    [0.0; 3],
                    [3.0, 0.0, 4.0],
                    (0.0, 4.0),
                    CurveContact::Crossing,
                    from_apex,
                ),
                line(
                    [0.0; 3],
                    [-3.0, 0.0, 4.0],
                    (0.0, 4.0),
                    CurveContact::Crossing,
                    from_apex,
                ),
            ],
        },
        Case {
            name: "cone, a tangent plane along a generator",
            plane: plane([0.0; 3], x, [0.0, 3.0, 4.0]),
            quadric: cone(),
            answer: vec![line(
                [0.0; 3],
                [0.0, 3.0, 4.0],
                (0.0, 4.0),
                tangent(),
                from_apex,
            )],
        },
        Case {
            name: "cone, a plane through the apex alone",
            plane: plane([0.0; 3], x, y),
            quadric: cone(),
            answer: vec![Part::Point(at([0.0; 3]), PointContact::IsolatedTangent)],
        },
    ];
    if plant.dropped {
        for case in &mut table {
            case.answer.pop();
        }
    }
    table
}

/// The two operands of `case`, as a method is handed them.
fn operands(case: &Case) -> (Operand<'_>, Operand<'_>) {
    let side = |surface| Operand {
        geom: Geom::Surface(surface),
        domain: None,
        bound: Steps(0),
        key: CanonicalKey::of_surface(surface, &mut meter()).unwrap(),
    };
    (side(&case.plane), side(&case.quadric))
}

fn span(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

/// The stated answer, in the roles a method answers in: the plane's unknowns first.
fn stand_in(case: &Case) -> Result<Intersection, IntersectError> {
    let class = Exactness::Certified {
        bound: Steps(STATED),
        step: GridStep::Q0,
    };
    let items = case
        .answer
        .iter()
        .enumerate()
        .map(|(i, part)| {
            let i = i as f64;
            let at = ParamBox::new(&[
                span(i, i + 0.5),
                span(0.0, 1.0),
                span(0.0, 1.0),
                span(0.0, 1.0),
            ])?;
            Ok(match part.clone() {
                Part::Point(p, contact) => {
                    let corner = P3::new(p[0] as i64, p[1] as i64, p[2] as i64).unwrap();
                    Item::Point {
                        at,
                        xyz: Box3::new(corner, corner).unwrap(),
                        contact,
                        class,
                    }
                }
                Part::Curve(curve, contact, ends) => Item::Curve {
                    at,
                    curve: Box::new(curve),
                    contact,
                    ends,
                    class,
                },
            })
        })
        .collect::<Result<Vec<Item>, IntersectError>>()?;
    Intersection::new(items, Coverage::TopologyCertified, vec![], &mut meter())
}

/// A method the laws judge: a case's answer, in the roles the laws read.
type Method = dyn Fn(&Case) -> Result<Intersection, IntersectError>;

/// G165's answer in the roles the laws read, the plane's unknowns first, as `natural` gives them for
/// its arguments in that order. A pair it calls not natural is no answer, which the laws report.
fn closed_form(case: &Case) -> Result<Intersection, IntersectError> {
    quadrics::natural(&case.plane, &case.quadric, &mut meter())?.ok_or(IntersectError::Internal)
}

fn sub(a: &[IBig; 3], b: &[IBig; 3]) -> [IBig; 3] {
    [&a[0] - &b[0], &a[1] - &b[1], &a[2] - &b[2]]
}

fn dot(a: &[IBig; 3], b: &[IBig; 3]) -> IBig {
    fixed::mul(&a[0], &b[0]) + fixed::mul(&a[1], &b[1]) + fixed::mul(&a[2], &b[2])
}

fn double(x: &IBig) -> f64 {
    fixed::distance(core::slice::from_ref(x), &[IBig::ZERO])
}

/// How far `x` is from the patch `surface`: distance to the definition, in steps, or how far past
/// its domain in v when that is further. Plane domains are far beyond every answer here.
fn gap(surface: &SurfaceKind, x: [f64; 3]) -> f64 {
    let p = x.map(fx);
    let height = |c: [f64; 3], n: [f64; 3], domain: (f64, f64)| {
        let d = sub(&p, &c.map(fx));
        let n = n.map(fx);
        let h = dot(&d, &n);
        let radial = fixed::sqrt(&(dot(&d, &d) - fixed::mul(&h, &h))).unwrap();
        let past = f64::max(
            domain.0 - double(&h) * h_sign(&h),
            double(&h) * h_sign(&h) - domain.1,
        );
        (h, radial, past)
    };
    match surface {
        SurfaceKind::Plane(plane) => {
            let (o, a, b) = plane.vectors();
            let (a, b) = (a.map(fx), b.map(fx));
            let n = [
                fixed::mul(&a[1], &b[2]) - fixed::mul(&a[2], &b[1]),
                fixed::mul(&a[2], &b[0]) - fixed::mul(&a[0], &b[2]),
                fixed::mul(&a[0], &b[1]) - fixed::mul(&a[1], &b[0]),
            ];
            let size = fixed::sqrt(&dot(&n, &n)).unwrap();
            double(&fixed::div(&dot(&sub(&p, &o.map(fx)), &n), &size).unwrap())
        }
        SurfaceKind::Sphere(sphere) => {
            let d = sub(&p, &sphere.center().map(fx));
            double(&(fixed::sqrt(&dot(&d, &d)).unwrap() - fx(sphere.radius())))
        }
        SurfaceKind::Cylinder(c) => {
            let domain = ParamSurface::domain(c).1;
            let (_, radial, past) = height(c.center(), c.frame().normal(), domain);
            f64::max(double(&(radial - fx(c.radius()))), past)
        }
        SurfaceKind::Cone(c) => {
            let domain = ParamSurface::domain(c).1;
            let (h, radial, past) = height(c.center(), c.frame().normal(), domain);
            let (r, k) = c.radius_and_slope();
            let off = radial - fx(r) - fixed::mul(&fx(k), &h);
            let tilt = fixed::sqrt(&(fixed::one() + fx(k * k))).unwrap();
            f64::max(double(&fixed::div(&off, &tilt).unwrap()), past)
        }
        other => panic!("no distance yet for {}", truth::surface_name(other)),
    }
}

fn h_sign(h: &IBig) -> f64 {
    if *h < IBig::ZERO { -1.0 } else { 1.0 }
}

fn corners(xyz: &Box3) -> [f64; 3] {
    let (a, b) = (xyz.min(), xyz.max());
    let mid = |lo: i64, hi: i64| f64::midpoint(lo as f64, hi as f64);
    [
        mid(a.x.get(), b.x.get()),
        mid(a.y.get(), b.y.get()),
        mid(a.z.get(), b.z.get()),
    ]
}

fn stated(class: Exactness) -> f64 {
    match class {
        Exactness::Exact => 0.0,
        Exactness::Certified { bound, .. } => bound.0 as f64,
        Exactness::Approximate { deviation, .. } => deviation.0 as f64,
    }
}

fn at_param(curve: &CurveKind<3>, t: f64, side: Limit) -> [f64; 3] {
    ParamCurve::point(curve, t, side).unwrap()
}

/// `count` parameters across `curve`: evenly with a seeded jitter, and both ends.
fn sample(curve: &CurveKind<3>, count: usize, rng: &mut Rng) -> Vec<[f64; 3]> {
    let (lo, hi) = ParamCurve::domain(curve);
    let width = (hi - lo) / count as f64;
    let mut out = vec![
        at_param(curve, lo, Limit::FromRight),
        at_param(curve, hi, Limit::FromLeft),
    ];
    for i in 0..count {
        let t = lo + (i as f64 + rng.unit()) * width;
        out.push(at_param(curve, t.min(hi), Limit::FromRight));
    }
    out
}

/// Symmetric in its arguments: the pair either way round gives one answer, roles swapped.
fn symmetric(
    case: &Case,
    solve: &dyn Fn(&Case) -> Result<Intersection, IntersectError>,
) -> Result<(), String> {
    let (a, b) = operands(case);
    let run = |x: &Operand<'_>, y: &Operand<'_>| pair(x, y, |_, _| solve(case));
    let one = run(&a, &b).map_err(|e| format!("{e:?}"))?;
    let other = run(&b, &a)
        .map_err(|e| format!("{e:?}"))?
        .swapped(b.geom.unknowns());
    if one.items() == other.items() {
        Ok(())
    } else {
        Err("the pair's two orders give different answers".to_owned())
    }
}

/// Every returned point lies on both inputs within its item's bound.
fn on_both(case: &Case, answer: &Intersection, rng: &mut Rng) -> Result<(), String> {
    let hold = |x: [f64; 3], class: Exactness| -> Result<(), String> {
        for (which, surface) in [("plane", &case.plane), ("quadric", &case.quadric)] {
            let far = gap(surface, x);
            if far > stated(class) + SLACK {
                return Err(format!("{x:?} is {far} steps off the {which}"));
            }
        }
        Ok(())
    };
    for item in answer.items() {
        match item {
            Item::Point { xyz, class, .. } => hold(corners(xyz), *class)?,
            Item::Curve { curve, class, .. } => {
                for x in sample(curve, scale(256), rng) {
                    hold(x, *class)?;
                }
            }
            Item::Overlap { region, class, .. } => {
                for curve in &region.boundary {
                    for x in sample(curve, scale(256), rng) {
                        hold(x, *class)?;
                    }
                }
            }
            Item::Flagged { .. } => {}
        }
    }
    Ok(())
}

/// Which conic a plane cuts from a quadric, or what it degenerates to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Conic {
    Empty,
    Point,
    Circle,
    Ellipse,
    Parabola,
    Hyperbola,
    CrossingLines,
    ParallelLines,
    TangentLine,
}

fn exact(x: [f64; 3]) -> [RBig; 3] {
    x.map(|v| RBig::try_from(v).unwrap())
}

fn rdot(a: &[RBig; 3], b: &[RBig; 3]) -> RBig {
    &a[0] * &b[0] + &a[1] * &b[1] + &a[2] * &b[2]
}

fn rsub(a: &[RBig; 3], b: &[RBig; 3]) -> [RBig; 3] {
    [&a[0] - &b[0], &a[1] - &b[1], &a[2] - &b[2]]
}

/// A natural quadric as y·y − m(n·y)² + 2w(n·y) + e = 0, on y = x − c, with n̂ a unit vector here.
struct Quadric {
    center: [RBig; 3],
    axis: [RBig; 3],
    /// m: 0 for a sphere, 1 for a cylinder, 1 + k² for a cone of slope k.
    pull: RBig,
    /// w: −r·k for a cone with radius r at its centre.
    linear: RBig,
    /// e: −r².
    constant: RBig,
}

fn quadric_of(surface: &SurfaceKind) -> Quadric {
    let r = |x: f64| RBig::try_from(x).unwrap();
    match surface {
        SurfaceKind::Sphere(s) => Quadric {
            center: exact(s.center()),
            axis: exact([0.0; 3]),
            pull: RBig::ZERO,
            linear: RBig::ZERO,
            constant: -(r(s.radius()) * r(s.radius())),
        },
        SurfaceKind::Cylinder(c) => Quadric {
            center: exact(c.center()),
            axis: exact(c.frame().normal()),
            pull: RBig::ONE,
            linear: RBig::ZERO,
            constant: -(r(c.radius()) * r(c.radius())),
        },
        SurfaceKind::Cone(c) => {
            let (radius, k) = c.radius_and_slope();
            Quadric {
                center: exact(c.center()),
                axis: exact(c.frame().normal()),
                pull: RBig::ONE + r(k) * r(k),
                linear: -(r(radius) * r(k)),
                constant: -(r(radius) * r(radius)),
            }
        }
        other => panic!("not a natural quadric: {}", truth::surface_name(other)),
    }
}

/// The conic `case`'s plane cuts from its quadric, in exact rationals: the form's restriction to the
/// plane, Au² + Buv + Cv² + Du + Ev + F, and its discriminants.
fn classify(case: &Case) -> Conic {
    let SurfaceKind::Plane(plane) = &case.plane else {
        panic!("the first operand is a plane")
    };
    let (origin, a, b) = plane.vectors();
    let (a, b) = (exact(a), exact(b));
    let q = quadric_of(&case.quadric);
    let y0 = rsub(&exact(origin), &q.center);
    let form =
        |x: &[RBig; 3], y: &[RBig; 3]| rdot(x, y) - &q.pull * rdot(x, &q.axis) * rdot(y, &q.axis);
    let two = RBig::ONE + RBig::ONE;
    let four = &two * &two;
    let big_a = form(&a, &a);
    let big_b = &two * form(&a, &b);
    let big_c = form(&b, &b);
    let big_d = &two * (form(&a, &y0) + &q.linear * rdot(&a, &q.axis));
    let big_e = &two * (form(&b, &y0) + &q.linear * rdot(&b, &q.axis));
    let big_f = form(&y0, &y0) + &two * &q.linear * rdot(&y0, &q.axis) + &q.constant;
    let half = |x: &RBig| x / &two;
    let (hb, hd, he) = (half(&big_b), half(&big_d), half(&big_e));
    let minor = &big_a * &big_c - &hb * &hb;
    let det = &big_a * (&big_c * &big_f - &he * &he) - &hb * (&hb * &big_f - &he * &hd)
        + &hd * (&hb * &he - &big_c * &hd);
    let zero = RBig::ZERO;
    if minor > zero {
        if det == zero {
            return Conic::Point;
        }
        if (&big_a + &big_c) * &det > zero {
            return Conic::Empty;
        }
        let (g11, g12, g22) = (rdot(&a, &a), rdot(&a, &b), rdot(&b, &b));
        let round = &big_a * &g22 == &big_c * &g11 && &hb * &g11 == &big_a * &g12;
        return if round { Conic::Circle } else { Conic::Ellipse };
    }
    if minor < zero {
        return if det == zero {
            Conic::CrossingLines
        } else {
            Conic::Hyperbola
        };
    }
    if det != zero {
        return Conic::Parabola;
    }
    // Two parallel lines, one line twice, or none, by the two lines' discriminant.
    let spread = &big_d * &big_d + &big_e * &big_e - four * &big_f * (&big_a + &big_c);
    match spread.cmp(&zero) {
        Ordering::Greater => Conic::ParallelLines,
        Ordering::Equal => Conic::TangentLine,
        Ordering::Less => Conic::Empty,
    }
}

fn is_kind(curve: &CurveKind<3>, conic: Conic) -> bool {
    match conic {
        Conic::Circle => matches!(curve, CurveKind::Circle(_)),
        Conic::Ellipse => matches!(curve, CurveKind::Ellipse(_)),
        Conic::Parabola => matches!(curve, CurveKind::Parabola(_)),
        Conic::Hyperbola => matches!(curve, CurveKind::Hyperbola(_)),
        Conic::CrossingLines | Conic::ParallelLines | Conic::TangentLine => {
            matches!(curve, CurveKind::Line(_))
        }
        Conic::Empty | Conic::Point => false,
    }
}

/// How far `x` is from an edge of `quadric`'s patch in v, in steps.
fn from_edge(quadric: &SurfaceKind, x: [f64; 3]) -> f64 {
    let (center, normal, domain) = match quadric {
        SurfaceKind::Cylinder(c) => (c.center(), c.frame().normal(), ParamSurface::domain(c).1),
        SurfaceKind::Cone(c) => (c.center(), c.frame().normal(), ParamSurface::domain(c).1),
        // A sphere's patch closes at its poles, so no conic on one ends at an edge.
        _ => return f64::INFINITY,
    };
    let h = (0..3).map(|i| (x[i] - center[i]) * normal[i]).sum::<f64>();
    f64::min((h - domain.0).abs(), (h - domain.1).abs())
}

/// The whole of a curve: closed, one full turn; open, running to the patch's edge or its apex.
fn whole(
    quadric: &SurfaceKind,
    curve: &CurveKind<3>,
    ends: [CurveEnd; 2],
    class: Exactness,
) -> Result<(), String> {
    let (lo, hi) = ParamCurve::domain(curve);
    if ends == [CurveEnd::Closed; 2] {
        return if hi - lo == 2.0 {
            Ok(())
        } else {
            Err(format!("a closed curve on ({lo}, {hi})"))
        };
    }
    let reach = [
        (at_param(curve, lo, Limit::FromRight), ends[0]),
        (at_param(curve, hi, Limit::FromLeft), ends[1]),
    ];
    for (x, end) in reach {
        let far = match end {
            CurveEnd::Boundary | CurveEnd::Singular => from_edge(quadric, x),
            other => return Err(format!("a conic ends by {other:?}")),
        };
        if far > stated(class) + SLACK {
            return Err(format!(
                "its end {x:?} is {far} steps short of the patch's edge"
            ));
        }
    }
    Ok(())
}

/// The answer is the exact conic of the closed form: its own kind, and all of it.
fn is_the_conic(case: &Case, answer: &Intersection) -> Result<(), String> {
    let conic = classify(case);
    if answer.items().len() != case.answer_size() {
        return Err(format!(
            "{} items for {} parts of a {conic:?}",
            answer.items().len(),
            case.answer_size()
        ));
    }
    for item in answer.items() {
        match item {
            Item::Point { .. } if conic == Conic::Point => {}
            Item::Curve {
                curve,
                ends,
                class,
                contact,
                ..
            } => {
                if !is_kind(curve, conic) {
                    return Err(format!(
                        "{} for a {conic:?}",
                        truth::curve_name(curve.as_ref())
                    ));
                }
                let touching = conic == Conic::TangentLine;
                if touching != matches!(contact, CurveContact::Tangent { .. }) {
                    return Err(format!("{contact:?} on a {conic:?}"));
                }
                whole(&case.quadric, curve, *ends, *class)?;
            }
            other => return Err(format!("{other:?} for a {conic:?}")),
        }
    }
    Ok(())
}

impl Case {
    /// How many items the whole answer has: a hyperbola's second branch is outside one nappe.
    fn answer_size(&self) -> usize {
        match classify(self) {
            Conic::Empty => 0,
            Conic::CrossingLines | Conic::ParallelLines => 2,
            _ => 1,
        }
    }
}

fn rng(label: &str) -> Rng {
    Rng(SplitMix64::new(resolve(label, Seed::DEFAULT).unwrap()))
}

/// Every law on `case` with `solve`, what broke named.
fn judge(
    case: &Case,
    solve: &dyn Fn(&Case) -> Result<Intersection, IntersectError>,
    rng: &mut Rng,
) -> Vec<(&'static str, String)> {
    let mut broken = Vec::new();
    if let Err(why) = symmetric(case, solve) {
        broken.push(("symmetric", why));
    }
    match solve(case) {
        Err(e) => broken.push(("answers", format!("{e:?}"))),
        Ok(answer) => {
            if let Err(why) = on_both(case, &answer, rng) {
                broken.push(("on both", why));
            }
            if let Err(why) = is_the_conic(case, &answer) {
                broken.push(("the conic", why));
            }
        }
    }
    broken
}

#[test]
fn plane_and_quadric_classify_as_stated() {
    let mut rng = rng("laws_intersections classify");
    let want = [
        Conic::Circle,
        Conic::Circle,
        Conic::Point,
        Conic::Empty,
        Conic::Circle,
        Conic::Ellipse,
        Conic::ParallelLines,
        Conic::TangentLine,
        Conic::Empty,
        Conic::Circle,
        Conic::Ellipse,
        Conic::Parabola,
        Conic::Hyperbola,
        Conic::CrossingLines,
        Conic::TangentLine,
        Conic::Point,
    ];
    let mut findings = Findings::default();
    for _ in 0..scale(16) {
        let mv = Move::draw(&mut rng);
        for (case, want) in cases(&mv, TRUE).iter().zip(want) {
            let got = classify(case);
            if got != want {
                findings.add(case.name.to_owned(), || format!("{got:?}, stated {want:?}"));
            }
        }
    }
    findings.assert_none("a plane with a quadric classifies as stated");
}

/// Every method that answers these pairs runs here: G165's closed form, and the stated one, which
/// holds the laws and the table to each other.
#[test]
fn the_laws_hold() {
    let mut rng = rng("laws_intersections laws");
    let mut findings = Findings::default();
    let methods: [(&str, &Method); 2] = [("natural", &closed_form), ("stated", &stand_in)];
    for _ in 0..scale(8) {
        let mv = Move::draw(&mut rng);
        for case in &cases(&mv, TRUE) {
            for (method, solve) in methods {
                for (law, why) in judge(case, solve, &mut rng) {
                    findings.add(format!("{method}, {}: {law}", case.name), || why);
                }
            }
        }
    }
    findings.assert_none("the intersection laws");
}

#[test]
fn each_law_catches_its_planted_fault() {
    let mut rng = rng("laws_intersections faults");
    let mv = Move::draw(&mut rng);
    let planted = [
        (
            "an answer off both inputs",
            Plant {
                drift: 64.0,
                ..TRUE
            },
            "on both",
            None,
        ),
        (
            "half a closed curve",
            Plant { turn: 1.0, ..TRUE },
            "the conic",
            Some("sphere, a parallel plane through it"),
        ),
        (
            "a circle answered as an ellipse",
            Plant {
                wrapped: true,
                ..TRUE
            },
            "the conic",
            Some("sphere, a parallel plane through it"),
        ),
        (
            "a part left out",
            Plant {
                dropped: true,
                ..TRUE
            },
            "the conic",
            Some("cylinder, a plane along its axis through it"),
        ),
    ];
    for (what, plant, law, only) in planted {
        for case in &cases(&mv, plant) {
            let empty = case.answer.is_empty() && !plant.dropped;
            if empty || only.is_some_and(|name| name != case.name) {
                continue;
            }
            let broken = judge(case, &stand_in, &mut rng);
            assert!(
                broken.iter().any(|(name, _)| *name == law),
                "{what} on {}: {broken:?}",
                case.name
            );
        }
    }
    // A method that changes its answer between calls.
    let calls = Cell::new(0);
    let restless = |case: &Case| {
        calls.set(calls.get() + 1);
        let mut moved = cases(
            &mv,
            Plant {
                drift: f64::from(calls.get()),
                ..TRUE
            },
        );
        let index = cases(&mv, TRUE)
            .iter()
            .position(|c| c.name == case.name)
            .unwrap();
        stand_in(&moved.swap_remove(index))
    };
    let table = cases(&mv, TRUE);
    let broken = judge(&table[0], &restless, &mut rng);
    assert!(
        broken.iter().any(|(name, _)| *name == "symmetric"),
        "{broken:?}"
    );
}
