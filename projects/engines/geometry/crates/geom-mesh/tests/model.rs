//! Decimation and remeshing through the public interface (piece G300), and the envelope's
//! whole-triangle test they rest on, against an exact oracle written apart from the code: squared
//! distances from rational points to triangles as exact fractions.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    reason = "test oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use geom_mesh::envelope::{EnvelopeError, TriangleEnvelope, covers, half_side};
use geom_mesh::model::{Decimate, ModelError, Modelled, Remesh, decimate, remesh};
use geom_num::grid::{GridStep, P3, Steps};
use geom_num::{Exactness, NumError};
use std::num::NonZeroU64;

use geom_run::{Budget, BudgetKind, Exec, Stop, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 40))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn xyz(c: P3) -> [i128; 3] {
    [c.x.get(), c.y.get(), c.z.get()].map(i128::from)
}

fn sub(a: [i128; 3], b: [i128; 3]) -> [i128; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [i128; 3], b: [i128; 3]) -> i128 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [i128; 3], b: [i128; 3]) -> [i128; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Squared distance from `q` to segment `ab`, as a fraction.
fn to_segment(q: [i128; 3], a: [i128; 3], b: [i128; 3]) -> (i128, i128) {
    let (ab, aq) = (sub(b, a), sub(q, a));
    let (t, l) = (dot(aq, ab), dot(ab, ab));
    if l == 0 || t <= 0 {
        (dot(aq, aq), 1)
    } else if t >= l {
        let bq = sub(q, b);
        (dot(bq, bq), 1)
    } else {
        (dot(aq, aq) * l - t * t, l)
    }
}

/// Whether squared distance `d` (a fraction) is at most `bound`.
fn within((num, den): (i128, i128), bound: i128) -> bool {
    num <= bound * den
}

/// Whether `q` is within squared distance `bound` of triangle `t`: through the face if its foot
/// is inside, else through the nearest edge.
fn near_triangle(q: [i128; 3], t: [[i128; 3]; 3], bound: i128) -> bool {
    let [a, b, c] = t;
    let n = cross(sub(b, a), sub(c, a));
    let length = dot(n, n);
    if length != 0 {
        let height = dot(n, sub(q, a));
        // The foot is inside when q − height·n/length is on the inner side of each edge.
        let inside = [(a, b), (b, c), (c, a)].iter().all(|&(u, v)| {
            let edge = cross(n, sub(v, u));
            // Side of the foot: dot(edge, q − u) − height·dot(edge, n)/length; dot(edge, n) = 0.
            dot(edge, sub(q, u)) >= 0
        });
        if inside {
            return within((height * height, length), bound);
        }
    }
    [(a, b), (b, c), (c, a)]
        .iter()
        .any(|&(u, v)| within(to_segment(q, u, v), bound))
}

/// Every sample of each of `from`'s triangles (corners, edge points and inner points on a lattice
/// of step 1/`d`) is within `epsilon` of some triangle of `to`: exact, with everything scaled by d.
fn samples_within(from: &[[P3; 3]], to: &[[P3; 3]], epsilon: u64, d: i128) -> bool {
    let to: Vec<[[i128; 3]; 3]> = to
        .iter()
        .map(|t| t.map(|c| xyz(c).map(|v| v * d)))
        .collect();
    let bound = i128::from(epsilon) * i128::from(epsilon) * d * d;
    for t in from {
        let [a, b, c] = t.map(xyz);
        for i in 0..=d {
            for j in 0..=d - i {
                let k = d - i - j;
                let q = [0, 1, 2].map(|m| a[m] * i + b[m] * j + c[m] * k);
                if !to.iter().any(|u| near_triangle(q, *u, bound)) {
                    return false;
                }
            }
        }
    }
    true
}

/// A triangle list's corner positions.
fn soup(points: &[P3], triangles: &[[u32; 3]]) -> Vec<[P3; 3]> {
    triangles
        .iter()
        .map(|t| t.map(|i| points[i as usize]))
        .collect()
}

/// An n × n grid of cells of side s, heights from `z`, counter-clockwise seen from above.
fn sheet(n: i64, s: i64, z: impl Fn(i64, i64) -> i64) -> (Vec<P3>, Vec<[u32; 3]>) {
    let mut points = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            points.push(p(i * s, j * s, z(i, j)));
        }
    }
    let at = |i: i64, j: i64| (j * (n + 1) + i) as u32;
    let mut triangles = Vec::new();
    for j in 0..n {
        for i in 0..n {
            triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    (points, triangles)
}

/// The surface of the cube [0, n·s]³ in n × n cells a face, outward, its points moved by `warp`.
fn cube(n: i64, s: i64, warp: impl Fn([i64; 3]) -> [i64; 3]) -> (Vec<P3>, Vec<[u32; 3]>) {
    let mut index: BTreeMap<[i64; 3], u32> = BTreeMap::new();
    let mut points = Vec::new();
    let mut id = |c: [i64; 3], points: &mut Vec<P3>| -> u32 {
        *index.entry(c).or_insert_with(|| {
            let [x, y, z] = warp(c.map(|v| v * s));
            points.push(p(x, y, z));
            (points.len() - 1) as u32
        })
    };
    let mut triangles = Vec::new();
    for axis in 0..3 {
        for side in [0, n] {
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            for j in 0..n {
                for i in 0..n {
                    let corner = |di: i64, dj: i64| {
                        let mut c = [0; 3];
                        c[axis] = side;
                        c[a] = i + di;
                        c[b] = j + dj;
                        c
                    };
                    let q = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
                    let v = q.map(|c| id(c, &mut points));
                    // (a, b, axis) is right-handed, so a, b turn counter-clockwise seen from +axis.
                    let (t1, t2) = if side == n {
                        ([v[0], v[1], v[2]], [v[0], v[2], v[3]])
                    } else {
                        ([v[0], v[2], v[1]], [v[0], v[3], v[2]])
                    };
                    triangles.push(t1);
                    triangles.push(t2);
                }
            }
        }
    }
    (points, triangles)
}

/// The cube's lattice pushed out onto a sphere of radius r about its centre, on the grid.
fn ball(n: i64, r: f64) -> (Vec<P3>, Vec<[u32; 3]>) {
    let half = n as f64 / 2.0;
    cube(n, 1, |c| {
        let d = c.map(|v| v as f64 - half);
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        d.map(|v| (v / l * r).round() as i64)
    })
}

/// Each directed edge's count, for closedness: an edge run both ways once each.
fn closed(triangles: &[[u32; 3]]) -> bool {
    let mut edges: BTreeMap<(u32, u32), i32> = BTreeMap::new();
    for &[a, b, c] in triangles {
        for (u, v) in [(a, b), (b, c), (c, a)] {
            *edges.entry((u, v)).or_default() += 1;
        }
    }
    edges
        .iter()
        .all(|(&(u, v), &k)| k == 1 && edges.get(&(v, u)) == Some(&1))
}

/// V − E + F.
fn euler(points: usize, triangles: &[[u32; 3]]) -> i64 {
    let mut edges = std::collections::BTreeSet::new();
    for &[a, b, c] in triangles {
        for (u, v) in [(a, b), (b, c), (c, a)] {
            edges.insert((u.min(v), u.max(v)));
        }
    }
    points as i64 - edges.len() as i64 + triangles.len() as i64
}

/// The result is within ε of the input both ways, by the sampling oracle.
fn two_way(input: &(Vec<P3>, Vec<[u32; 3]>), out: &Modelled, epsilon: u64) {
    let a = soup(&input.0, &input.1);
    let b = soup(out.points(), out.triangles());
    assert!(samples_within(&b, &a, epsilon, 4), "result strays from the input");
    assert!(samples_within(&a, &b, epsilon, 4), "input strays from the result");
}

#[test]
fn a_flat_sheet_decimates_to_few_triangles_on_the_same_plane() {
    let input = sheet(4, 100, |_, _| 0);
    let epsilon = 8;
    let out = decimate(
        &input.0,
        &input.1,
        Decimate { epsilon, target: 2 },
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    assert!(out.triangles().len() < input.1.len() / 4, "{}", out.triangles().len());
    assert!(out.points().iter().all(|q| q.z.get() == 0));
    assert_eq!(euler(out.points().len(), out.triangles()), 1);
    two_way(&input, &out, epsilon);
    assert_eq!(
        out.exactness(GridStep::Q0),
        Exactness::Certified {
            bound: Steps(epsilon),
            step: GridStep::Q0
        }
    );
}

#[test]
fn a_closed_cube_stays_closed_and_within_epsilon() {
    let input = cube(2, 64, |c| c);
    let epsilon = 6;
    let out = decimate(
        &input.0,
        &input.1,
        Decimate { epsilon, target: 12 },
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    assert!(closed(out.triangles()));
    assert_eq!(euler(out.points().len(), out.triangles()), 2);
    assert!(out.triangles().len() < input.1.len() / 2, "{}", out.triangles().len());
    two_way(&input, &out, epsilon);
}

#[test]
fn a_ball_decimates_within_epsilon_and_a_tighter_bound_keeps_more() {
    let input = ball(3, 300.0);
    let loose = decimate(
        &input.0,
        &input.1,
        Decimate { epsilon: 40, target: 0 },
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    let tight = decimate(
        &input.0,
        &input.1,
        Decimate { epsilon: 12, target: 0 },
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    assert!(closed(loose.triangles()) && closed(tight.triangles()));
    assert!(loose.triangles().len() < tight.triangles().len());
    assert!(tight.triangles().len() < input.1.len());
    two_way(&input, &loose, 40);
    two_way(&input, &tight, 12);
}

#[test]
fn decimation_is_deterministic() {
    let input = ball(2, 300.0);
    let run = || {
        decimate(
            &input.0,
            &input.1,
            Decimate { epsilon: 20, target: 0 },
            &Exec::serial(),
            &mut meter(),
        )
        .unwrap()
    };
    assert_eq!(run(), run());
}

#[test]
fn a_coarse_sheet_remeshes_toward_the_target_length() {
    // A gently bent sheet of 2 × 2 cells of 400 steps, remeshed toward edges of 150.
    let input = sheet(1, 300, |_, _| 0);
    let epsilon = 6;
    let out = remesh(
        &input.0,
        &input.1,
        Remesh {
            epsilon,
            length: 150,
            iterations: 2,
        },
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    assert!(out.triangles().len() > 4 * input.1.len(), "{}", out.triangles().len());
    let longest = out
        .triangles()
        .iter()
        .flat_map(|&[a, b, c]| [(a, b), (b, c), (c, a)])
        .map(|(u, v)| {
            let d = sub(xyz(out.points()[u as usize]), xyz(out.points()[v as usize]));
            dot(d, d)
        })
        .max()
        .unwrap();
    // Every edge at most 4/3 of the target, rounding aside.
    assert!(9 * longest <= 16 * 150 * 150 + 9 * 3 * 4, "{longest}");
    assert_eq!(euler(out.points().len(), out.triangles()), 1);
    two_way(&input, &out, epsilon);
}

#[test]
fn a_fine_ball_remeshes_coarser_within_epsilon() {
    let input = ball(3, 300.0);
    let epsilon = 30;
    let out = remesh(
        &input.0,
        &input.1,
        Remesh {
            epsilon,
            length: 280,
            iterations: 2,
        },
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    assert!(closed(out.triangles()));
    assert_eq!(euler(out.points().len(), out.triangles()), 2);
    assert!(out.triangles().len() < input.1.len());
    let (kept, refused) = out.operations();
    assert!(kept > 0, "{kept} {refused}");
    two_way(&input, &out, epsilon);
}

#[test]
fn inputs_that_are_not_surfaces_are_refused() {
    let (points, mut triangles) = sheet(1, 10, |_, _| 0);
    let run = |points: &[P3], triangles: &[[u32; 3]]| {
        decimate(
            points,
            triangles,
            Decimate { epsilon: 2, target: 0 },
            &Exec::serial(),
            &mut meter(),
        )
    };
    assert_eq!(
        run(&points, &[[0, 1, 9]]),
        Err(ModelError::Index { face: 0 })
    );
    assert_eq!(run(&points, &[[0, 1, 1]]), Err(ModelError::Flat { face: 0 }));
    let line = [p(0, 0, 0), p(1, 1, 1), p(2, 2, 2)];
    assert_eq!(run(&line, &[[0, 1, 2]]), Err(ModelError::Flat { face: 0 }));
    assert!(run(&points, &triangles).is_ok());
    // A triangle running the first's diagonal the same way.
    triangles.push([0, 3, 2]);
    assert_eq!(
        run(&points, &triangles),
        Err(ModelError::NotSurface { face: 2 })
    );
    // Two triangles meeting only at a vertex: a bow tie.
    let bow = [p(0, 0, 0), p(10, 0, 0), p(10, 10, 0), p(-10, 0, 0), p(-10, -10, 0)];
    assert!(matches!(
        run(&bow, &[[0, 1, 2], [0, 3, 4]]),
        Err(ModelError::NotSurface { .. })
    ));
}

#[test]
fn a_budget_ends_the_run_with_an_error() {
    let input = ball(4, 300.0);
    let mut small = WorkMeter::new(Budget::of_work(20_000));
    let r = decimate(
        &input.0,
        &input.1,
        Decimate { epsilon: 20, target: 0 },
        &Exec::serial(),
        &mut small,
    );
    match r {
        Err(ModelError::Envelope(EnvelopeError::Stopped(s))) => {
            assert_eq!(s.reason(), Stop::OverBudget(BudgetKind::Work));
        }
        Err(ModelError::Envelope(EnvelopeError::Bvh(_))) => {}
        other => panic!("{other:?}"),
    }
}

/// Bodies of width `epsilon` round each of `triangles`.
fn bodies(triangles: &[[P3; 3]], epsilon: u64) -> Vec<TriangleEnvelope> {
    triangles
        .iter()
        .map(|t| TriangleEnvelope::new(*t, half_side(epsilon)).unwrap())
        .collect()
}

#[test]
fn a_triangle_across_two_coplanar_bodies_is_covered_and_one_past_them_is_not() {
    let square = [
        [p(0, 0, 0), p(100, 0, 0), p(100, 100, 0)],
        [p(0, 0, 0), p(100, 100, 0), p(0, 100, 0)],
    ];
    let b = bodies(&square, 4);
    let refs: Vec<&TriangleEnvelope> = b.iter().collect();
    let mut m = meter();
    // Inside the square, crossing its diagonal, lifted a step.
    let across = [p(10, 5, 1), p(90, 20, 1), p(30, 95, 1)];
    assert_eq!(covers(&refs, across, &mut m), Ok(true));
    // One corner 10 steps past the square's edge.
    let past = [p(10, 5, 0), p(110, 20, 0), p(30, 95, 0)];
    assert_eq!(covers(&refs, past, &mut m), Ok(false));
    // Lifted past ε.
    let high = [p(10, 5, 5), p(90, 20, 5), p(30, 95, 5)];
    assert_eq!(covers(&refs, high, &mut m), Ok(false));
    // A corner in, the rest out through a gap: two bodies apart.
    let apart = bodies(
        &[
            [p(0, 0, 0), p(40, 0, 0), p(0, 40, 0)],
            [p(60, 0, 0), p(100, 0, 0), p(60, 40, 0)],
        ],
        4,
    );
    let refs: Vec<&TriangleEnvelope> = apart.iter().collect();
    let bridge = [p(5, 5, 0), p(95, 5, 0), p(50, 10, 0)];
    assert_eq!(covers(&refs, bridge, &mut m), Ok(false));
    assert_eq!(covers(&[], bridge, &mut m), Ok(false));
    let flat = [p(5, 5, 0), p(95, 5, 0), p(185, 5, 0)];
    assert_eq!(
        covers(&refs, flat, &mut m),
        Err(EnvelopeError::Num(NumError::Degenerate))
    );
}

/// Random surfaces of triangles and random triangles near them: every triangle `covers` accepts
/// is within ε of the surface at every sample (soundness), and each surface triangle's own
/// sub-triangles are accepted (no needless refusal where one body holds the whole).
#[test]
fn covered_triangles_are_within_epsilon_at_every_sample() {
    let seed = resolve("geom-mesh model covers oracle", Seed::DEFAULT).unwrap();
    let mut g = SplitMix64::new(seed);
    let below = |g: &mut SplitMix64, n: u64| g.below(NonZeroU64::new(n).unwrap());
    let small = |g: &mut SplitMix64, r: u64| below(g, 2 * r + 1) as i64 - r as i64;
    let mut accepted = 0;
    for _ in 0..60 {
        let epsilon = 3 + below(&mut g, 8);
        let (points, triangles) = sheet(3, 40, |i, j| ((i * 7 + j * 3) % 5) * 2);
        let surface = soup(&points, &triangles);
        let b = bodies(&surface, epsilon);
        let refs: Vec<&TriangleEnvelope> = b.iter().collect();
        for _ in 0..20 {
            let t = [0, 1, 2].map(|_| {
                let (x, y) = (small(&mut g, 60) + 60, small(&mut g, 60) + 60);
                p(x, y, small(&mut g, 6) + 4)
            });
            let mut m = meter();
            match covers(&refs, t, &mut m) {
                Ok(true) => {
                    accepted += 1;
                    assert!(
                        samples_within(&[t], &surface, epsilon, 6),
                        "{seed:?}: {t:?} accepted at ε = {epsilon}"
                    );
                }
                Ok(false) | Err(EnvelopeError::Num(NumError::Degenerate)) => {}
                Err(e) => panic!("{seed:?}: {e:?}"),
            }
        }
        for s in &surface {
            let [a, b2, c] = s.map(xyz);
            let mid = |u: [i128; 3], v: [i128; 3]| {
                let m = [0, 1, 2].map(|k| i128::midpoint(u[k], v[k]));
                p(m[0] as i64, m[1] as i64, m[2] as i64)
            };
            let inner = [s[0], mid(a, b2), mid(a, c)];
            assert_eq!(covers(&refs, inner, &mut meter()), Ok(true), "{seed:?}");
        }
    }
    assert!(accepted > 0, "{seed:?}: nothing accepted");
}
