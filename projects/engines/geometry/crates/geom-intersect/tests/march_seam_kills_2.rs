//! The march at the exact ends of its comparisons (chapters 12.3, 12.8 and 12.10): settled pairs
//! built by hand so that one comparison stands exactly at its tie, or one start's enclosure reaches
//! a pole it is not held on, and the branches the march gives are the curve's own: how many, which
//! starts each joins, and whether the pair is left unproven.
//!
//! Every case's curve is known exactly: two planes meet in a line, the plane z = 80y
//! meets the unit cylinder about z in an ellipse, a plane through the apex of the cone
//! x² + y² = z² meets it in two of its rulings, and the plane x = z + 1024 through the south pole
//! of the sphere of radius 1024 meets it in a circle through that pole.
//!
//! And the uniqueness proof the march lends to known curves (12.11), where an entry of I − Y·J is
//! further below zero than above it: a sheet whose slope steepens across the line it shares with
//! a plane, so that every cell tried reads that entry past −1 and none is proven.
//!
//! And a pole read exactly a floor from the other surface (12.10): the tip of a flat triangle on a
//! plane whose parameters place the tip's foot no finer than their own steps, at the floor that
//! reading stands exactly at.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::Frame;
use geom_intersect::general::{Settled, Subdivided, subdivide};
use geom_intersect::known::{Known, KnownContact, subdivide_known};
use geom_intersect::march::{Traced, march};
use geom_intersect::project::{Projection, project_surface};
use geom_intersect::result::{Degeneracy, Unresolved};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, ParamSurface, Plane, Sphere, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn around(x: f64, half: f64) -> Interval {
    Interval::new(x - half, x + half).unwrap()
}

fn pt(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// One settled pair and nothing flagged.
fn one_pair(
    first: ((f64, f64), (f64, f64)),
    second: ((f64, f64), (f64, f64)),
    starts: Vec<[Interval; 4]>,
) -> Subdivided {
    Subdivided {
        settled: vec![Settled {
            first,
            second,
            starts,
        }],
        flagged: Vec::new(),
    }
}

/// The point of `surface` at the middle of the parameters `(u, v)`.
fn at(surface: &SurfaceKind, u: Interval, v: Interval) -> [f64; 3] {
    surface
        .point(mid(u), mid(v), (Limit::FromRight, Limit::FromRight))
        .unwrap()
}

/// Every vertex of every branch is one point of both surfaces, to within `room`, and lies where
/// `on` says the exact curve lies.
fn on_the_curve(
    (a, b): (&SurfaceKind, &SurfaceKind),
    traced: &Traced,
    room: f64,
    on: impl Fn([f64; 3]) -> f64,
) {
    for branch in &traced.branches {
        for x in &branch.vertices {
            let (p, q) = (at(a, x[0], x[1]), at(b, x[2], x[3]));
            for k in 0..3 {
                assert!((p[k] - q[k]).abs() <= room, "{x:?}: {p:?} against {q:?}");
            }
            assert!(on(p).abs() <= room, "{x:?} at {p:?} is off the curve");
        }
    }
}

/// The start each branch begins and ends at, by index into `starts`, read from its end vertices.
fn joins(traced: &Traced, starts: &[[Interval; 4]]) -> Vec<[usize; 2]> {
    let which = |x: &[Interval; 4]| {
        starts
            .iter()
            .position(|s| {
                s.iter()
                    .zip(x)
                    .all(|(a, b)| a.lo() == b.lo() && a.hi() == b.hi())
            })
            .unwrap_or_else(|| panic!("{x:?} is no start"))
    };
    let mut out: Vec<[usize; 2]> = traced
        .branches
        .iter()
        .map(|b| {
            let mut ends = [which(&b.vertices[0]), which(b.vertices.last().unwrap())];
            ends.sort_unstable();
            ends
        })
        .collect();
    out.sort_unstable();
    out
}

/// The planes z = 0, as (x, y), and x = `side`, as (y, z), which meet in the line x = `side`,
/// z = 0, marched between starts on it at y = 0 and y = 2⁻³³ at a floor of `k` · 2⁻⁴²: what was
/// traced, and the starts.
fn two_planes(side: f64, k: f64) -> (Traced, Vec<[Interval; 4]>) {
    let one = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((side - 1.0, side + 1.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    let two = SurfaceKind::Plane(
        Plane::new(
            [side, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            ((-1.0, 1.0), (-1.0, 1.0)),
        )
        .unwrap(),
    );
    let far = 1.0 / 8_589_934_592.0;
    let starts = vec![
        [pt(side), pt(0.0), pt(0.0), pt(0.0)],
        [pt(side), pt(far), pt(far), pt(0.0)],
    ];
    let found = one_pair(
        ((side - 0.5, side + 0.5), (-0.5, 0.5)),
        ((-0.5, 0.5), (-0.5, 0.5)),
        starts.clone(),
    );
    let floor = k / 4_398_046_511_104.0;
    let traced = march(&one, &two, &found, floor, 1.0, &mut meter()).unwrap();
    (traced, starts)
}

#[test]
fn a_step_whose_image_ends_on_its_boxs_own_end_is_not_proven() {
    // A step is proven only if the image of its box lies strictly inside the box (12.3). Along the
    // line x = ±1, z = 0 the parameter x never moves, so its side of the box is the step's pad
    // alone, a few of the doubles next to 1 and wider with the floor, while the image the proof
    // computes is wide by its own rounding alone. At a floor of 8 · 2⁻⁴² the image's end away from
    // zero is the box's own end, to the bit: the upper ends on x = 1, the lower ends on x = −1. An
    // end reached is not strictly inside, so the pair is left unproven; one more double of floor
    // and the step is proven, and the one branch joins the two starts.
    for side in [1.0, -1.0] {
        let (traced, _) = two_planes(side, 8.0);
        assert!(traced.branches.is_empty(), "x = {side}: {traced:?}");
        assert_eq!(traced.unproven.len(), 1, "x = {side}: {traced:?}");
        let (traced, starts) = two_planes(side, 9.0);
        assert!(traced.unproven.is_empty(), "x = {side}: {traced:?}");
        assert_eq!(joins(&traced, &starts), [[0, 1]], "x = {side}: {traced:?}");
    }
}

#[test]
fn a_march_exactly_twice_its_step_past_the_last_start_goes_on_and_reaches_it() {
    // z = 0 and the plane through y = x and the z axis meet in the line y = x, z = 0. One pair
    // holds its piece from (3, 3) to (3.5, 3.5): the first start exact, the second given wide, the
    // one point it holds at the middle of its x, but the middle of its y 21/16 to one side of the
    // line. The floor is 9/16, more than the 1/2 between the starts, and the first step 1/16: the
    // first vertex stands at x = 3 + 9/16 + 1/16, past the second start by twice the step exactly,
    // and over twice the floor from that start's middle in y; the next vertex stands within it.
    // Past every start by more than twice its step a march has nothing left to reach, but at
    // exactly twice it goes on, and here ends at the second start.
    let one = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 16.0), (0.0, 16.0)),
        )
        .unwrap(),
    );
    // As (x, z).
    let two = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            ((0.0, 16.0), (-4.0, 4.0)),
        )
        .unwrap(),
    );
    let starts = vec![
        [pt(3.0), pt(3.0), pt(3.0), pt(0.0)],
        [
            Interval::new(3.25, 3.75).unwrap(),
            Interval::new(3.3125, 6.3125).unwrap(),
            pt(3.5),
            pt(0.0),
        ],
    ];
    let found = one_pair(
        ((2.0, 8.0), (2.0, 8.0)),
        ((2.0, 8.0), (-1.0, 1.0)),
        starts.clone(),
    );
    let traced = march(&one, &two, &found, 0.5625, 1.0, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(joins(&traced, &starts), [[0, 1]], "{traced:?}");
    // The one vertex between the starts is the line's point at x = 3.625.
    let branch = &traced.branches[0];
    assert_eq!(branch.vertices.len(), 3, "{branch:?}");
    let [x, y, s, z] = branch.vertices[1].map(mid);
    for (got, want) in [(x, 3.625), (y, 3.625), (s, 3.625), (z, 0.0)] {
        assert!((got - want).abs() <= 1e-9, "{branch:?}");
    }
}

#[test]
fn two_starts_level_on_a_patchs_edge_each_end_their_own_arc() {
    // z = 80y meets the unit cylinder about z in an ellipse over the unit circle. The plane's
    // patch ends at y = cos 0.05, which cuts the top of the ellipse off: two arcs, each from a
    // start low on its side up to a start on that edge. The two starts on the edge are level, with
    // one y and one height between them, so a vertex of the left arc that comes beside them both
    // is as far, in the march's measure, from the one as from the other; the arc ends at its own.
    let m = 80.0;
    let pi = core::f64::consts::PI;
    let ramp = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, m],
            ((-2.0, 2.0), (0.0, 2.0)),
        )
        .unwrap(),
    );
    let post = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            1.0,
            Frame::standard().unwrap(),
            ((0.05, 0.95), (0.0, 100.0)),
        )
        .unwrap(),
    );
    let e = 1e-12;
    let cut = 0.05_f64;
    // The point of the ellipse at the cylinder's angle `phi`, in half turns.
    let low = |phi: f64| {
        let (x, y) = ((pi * phi).cos(), (pi * phi).sin());
        [
            around(x, e),
            around(y, e),
            around(phi, e),
            around(m * y, 64.0 * e),
        ]
    };
    let (y, z) = (pt(cut.cos()), around(m * cut.cos(), 64.0 * e));
    let starts = vec![
        low(1.0 - 0.6 / pi),
        [around(-cut.sin(), e), y, around(0.5 + cut / pi, e), z],
        [around(cut.sin(), e), y, around(0.5 - cut / pi, e), z],
        low(0.6 / pi),
    ];
    let found = one_pair(
        ((-1.0, 1.0), (0.5, cut.cos())),
        ((0.1, 0.9), (40.0, 80.0)),
        starts.clone(),
    );
    let traced = march(&ramp, &post, &found, 0.125, 0.003, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(joins(&traced, &starts), [[0, 1], [2, 3]], "{traced:?}");
    on_the_curve((&ramp, &post), &traced, 0.125, |[x, y, _]| {
        x * x + y * y - 1.0
    });
}

#[test]
fn a_start_level_with_an_edges_own_start_is_not_the_next_along_that_edge() {
    // The plane x = −z cos π/64 passes through the apex of the cone x² + y² = z² and holds its
    // rulings at the angles π(1 ∓ 1/64). The cone's domain begins at the first, so that ruling is
    // an edge of the cone lying in the plane, and the second runs inside. One pair holds both from
    // z = 2 to z = 3: two starts on each, level in twos. The inner ruling's lower start is given
    // wide, its u reaching the edge, where it stands level with the edge's own lower start. A
    // piece along an edge runs to the nearest start further along it, never to one level with its
    // own, so the edge's two starts are joined along it and the inner ruling's two by the march.
    let g = 1.0 / 64.0;
    let pi = core::f64::consts::PI;
    let (c, s) = ((pi * g).cos(), (pi * g).sin());
    let cone = SurfaceKind::Cone(
        Cone::new(
            [0.0, 0.0, 1.0],
            1.0,
            1.0,
            Frame::standard().unwrap(),
            ((1.0 - g, 1.25), (0.0, 4.0)),
        )
        .unwrap(),
    );
    // As (y, z).
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [0.0, 1.0, 0.0],
            [-c, 0.0, 1.0],
            ((-1.0, 1.0), (0.0, 5.0)),
        )
        .unwrap(),
    );
    let e = 1e-9;
    let floor = 1.0 / 64.0;
    let starts = vec![
        [pt(1.0 - g), pt(1.0), around(2.0 * s, e), around(2.0, e)],
        [
            Interval::new(1.0 - g, 1.0 + 3.0 * g).unwrap(),
            pt(1.0),
            around(-2.0 * s, e),
            around(2.0, e),
        ],
        [pt(1.0 - g), pt(2.0), around(3.0 * s, e), around(3.0, e)],
        [
            around(1.0 + g, e),
            pt(2.0),
            around(-3.0 * s, e),
            around(3.0, e),
        ],
    ];
    let found = one_pair(
        ((1.0 - g, 1.0 + 4.0 * g), (1.0, 2.0)),
        ((-0.5, 0.5), (1.5, 3.5)),
        starts.clone(),
    );
    let traced = march(&cone, &plane, &found, floor, floor, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(joins(&traced, &starts), [[0, 2], [1, 3]], "{traced:?}");
    on_the_curve((&cone, &plane), &traced, floor, |[x, y, z]| {
        (x + c * z).abs() + (y.abs() - s * z).abs()
    });
}

#[test]
fn a_start_whose_box_reaches_a_pole_it_is_not_held_on_is_not_the_pole() {
    // x = z + 1024 passes through the south pole of the sphere of radius 1024 about the origin and
    // meets it in a circle through that pole. One pair holds the arc from the pole to the point a
    // thirty-second of a half turn up from it. That point's start is given wide: its v reaches
    // down to the pole line and its place on the plane reaches the pole's, though its u does not.
    // Only a start held exactly on the pole line is the pole, whatever its u, so these are two
    // points and the one branch joins them.
    let r = 1024.0;
    let pi = core::f64::consts::PI;
    let t = 1.0 / 32.0;
    let a = pi * t;
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            r,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.0)),
        )
        .unwrap(),
    );
    // As (y, x).
    let plane = SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, -r],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 1.0],
            ((-512.0, 512.0), (-512.0, 512.0)),
        )
        .unwrap(),
    );
    let e = 1e-9;
    let u = (a / 2.0).tan().acos() / pi;
    let y = r * a.sin() * (pi * u).sin();
    let x = 2.0 * r * (a / 2.0).sin() * (a / 2.0).sin();
    let pole = [pt(0.5), pt(-0.5), pt(0.0), pt(0.0)];
    let up = [
        around(u, e),
        Interval::new(-0.5, -0.5 + 2.0 * t).unwrap(),
        Interval::new(-e, 2.0 * y + e).unwrap(),
        Interval::new(-e, 2.0 * x + e).unwrap(),
    ];
    let starts = vec![pole, up];
    let found = one_pair(
        ((0.4, 0.6), (-0.5, -0.5 + 3.0 * t)),
        ((-8.0, 256.0), (-8.0, 64.0)),
        starts.clone(),
    );
    let traced = march(&ball, &plane, &found, 8.0, 10.24, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{traced:?}");
    assert_eq!(joins(&traced, &starts), [[0, 1]], "{traced:?}");
    on_the_curve((&ball, &plane), &traced, 8.0, |[x, _, z]| x - z - r);
}

/// The sheet (u, 3v, (v + c)³ − c³), c = 1/64, over u and v in [−1, 1]: it crosses the plane
/// z = 0 along v = 0 and nowhere else, and its slope across that line is 3·(v + c)².
fn steepening_sheet() -> SurfaceKind {
    // The Bézier heights of (v + c)³ − c³ over [−1, 1], each a whole number of 2⁻¹⁸.
    let heights = [-250_048.0, 257_984.0, -266_176.0, 274_624.0].map(|h: f64| h / 262_144.0);
    let across = Basis::new(3, vec![-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let along = Basis::new(1, vec![-1.0, -1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    for x in [-1.0, 1.0] {
        for (y, z) in [-3.0, -1.0, 1.0, 3.0].into_iter().zip(heights) {
            net.push([x, y, z]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(along, across, &net).unwrap())
}

#[test]
fn a_known_curve_whose_cells_read_a_slope_past_twice_their_middles_is_searched() {
    // A cell reaching w either side of the line v = 0 reads a slope between (1 − w/c)² and
    // (1 + w/c)² of its middle's, so the one entry of I − Y·J that is not zero lies between
    // 1 − (1 + w/c)² and 1 − (1 − w/c)²: under 1 for every w < c, and down to −1 once
    // w = (√2 − 1)·c. At a floor of 1/32 the narrowest cell tried has w = 0.46·c and reads that
    // entry down to −1.14, so the matrix is not proven regular and no cell is taken out: the line
    // is searched as it is with no known curve, every pair along it settled and none flagged.
    let sheet = steepening_sheet();
    let wall = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            ((-8.0, 8.0), (-8.0, 8.0)),
        )
        .unwrap(),
    );
    let line: Vec<[Interval; 4]> = (-2..=2)
        .map(|k| {
            let x = f64::from(k) / 4.0;
            [pt(x), pt(0.0), pt(x), pt(0.0)]
        })
        .collect();
    let known = Known {
        vertices: line,
        bound: 0.0,
        contact: KnownContact::Crossing,
    };
    let floor = 1.0 / 32.0;
    let found = subdivide_known(&sheet, &wall, &[known], floor, &mut meter()).unwrap();
    let plain = subdivide_known(&sheet, &wall, &[], floor, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:?}", found.flagged);
    assert_eq!(found, plain);
    // The settled pairs hold the whole line, from one edge of the sheet to the other.
    let ends = found
        .settled
        .iter()
        .fold((1.0_f64, -1.0_f64), |(lo, hi), s| {
            let ((a, b), (c, d)) = s.first;
            assert!(c < 0.0 && 0.0 < d, "{s:?}");
            (lo.min(a), hi.max(b))
        });
    assert_eq!(ends, (-1.0, 1.0));
}

#[test]
fn a_pole_read_exactly_a_floor_from_the_other_surface_is_read_as_on_it() {
    // A flat triangle with its tip, a collapsed first row of its net, at (1, 1, 0): z = u − 1,
    // so the tip is all of it that the plane z = 0 holds. The plane's parameters run a million
    // past the tip, so they place its foot no finer than their own steps there.
    let basis = || Basis::new(1, vec![1.0, 1.0, 2.0, 2.0]).unwrap();
    let tip = [1.0, 1.0, 0.0];
    let net = [tip, tip, [2.0, 0.0, 1.0], [2.0, 2.0, 1.0]];
    let tent = SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap());
    let (far, w) = (1_048_576.0, 1.0 / 1024.0);
    let c = far + 1.0;
    let plane = SurfaceKind::Plane(
        Plane::new(
            [-far, -far, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((c - w, c + w), (c - w, c + w)),
        )
        .unwrap(),
    );
    // The pole's reading (12.10): its enclosure's widest side, and how far its middle is from
    // its foot on the plane, the foot found to an eighth of the floor. The floor is the one that
    // reading stands exactly at, the far end of the distance and the spread together.
    let sides = (Limit::FromRight, Limit::FromRight);
    let read = tent
        .partials_over(((1.0, 1.0), (1.0, 1.0)), sides, 0, &mut meter())
        .unwrap();
    let pole = read[0];
    let spread = pole.iter().map(|x| x.hi() - x.lo()).fold(0.0, f64::max);
    let reading = |floor: f64| {
        let found = project_surface(&plane, &pole.map(mid), floor / 8.0, &mut meter()).unwrap();
        let Projection::Points(feet) = found else {
            panic!("{found:?}");
        };
        assert_eq!(feet.len(), 1, "{feet:?}");
        assert!(feet[0].distance.lo() <= spread, "{feet:?}");
        feet[0].distance.hi() + spread
    };
    let floor = reading(reading(2.0 * spread));
    assert_eq!(reading(floor), floor);
    // On the plane to within the floor, with no way out of the tip along the plane: every pair
    // left holds the tip on both surfaces and says a pole stands there alone.
    let found = subdivide(&tent, &plane, floor, &mut meter()).unwrap();
    assert!(found.settled.is_empty(), "{found:?}");
    assert!(!found.flagged.is_empty(), "{found:?}");
    for pair in &found.flagged {
        let (((u, _), _), ((s0, s1), (r0, r1))) = (pair.first, pair.second);
        assert_eq!(
            pair.why,
            Unresolved::NonSmooth {
                at: Degeneracy::Pole
            },
            "{pair:?}"
        );
        assert!(
            u == 1.0 && s0 <= c && c <= s1 && r0 <= c && c <= r1,
            "{pair:?}"
        );
    }
}
