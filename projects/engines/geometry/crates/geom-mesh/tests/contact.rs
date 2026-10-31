//! Contact and closest features through the public interface (piece G151), against oracles written
//! apart from the code: brute force over a small lattice, by edges and by exact fractions.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "test oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_mesh::contact::{Closest, Contact, closest_feature, point_triangle, triangles};
use geom_num::distance::{self, MAX_CLEARANCE};
use geom_num::grid::P3;
use geom_num::predicate::orient3d;
use geom_num::{NumError, Sign};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn sign(v: i128) -> Sign {
    Sign::of_i128(v)
}

/// A point as integers.
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

/// Whether segment `s` meets triangle `t`, closed, by exact fractions: where the segment crosses
/// the plane as a parameter a/b, with barycentric tests on that point.
fn segment_meets(s: [[i128; 3]; 2], t: [[i128; 3]; 3]) -> bool {
    let [a, b, c] = t;
    let n = cross(sub(b, a), sub(c, a));
    let (d0, d1) = (dot(n, sub(s[0], a)), dot(n, sub(s[1], a)));
    if d0 == 0 && d1 == 0 {
        return false; // In the plane: the 2D walk below handles it.
    }
    if (d0 > 0 && d1 > 0) || (d0 < 0 && d1 < 0) {
        return false;
    }
    // x = s0 + (d0 / (d0 − d1)) (s1 − s0), scaled by w = d0 − d1.
    let w = d0 - d1;
    let dir = sub(s[1], s[0]);
    let x: Vec<i128> = (0..3).map(|i| s[0][i] * w + d0 * dir[i]).collect();
    let scaled = |q: [i128; 3]| [x[0] - q[0] * w, x[1] - q[1] * w, x[2] - q[2] * w];
    let side = |p: [i128; 3], q: [i128; 3]| dot(n, cross(sub(q, p), scaled(p))) * w.signum();
    side(a, b) >= 0 && side(b, c) >= 0 && side(c, a) >= 0
}

/// Whether point `q` is in closed triangle `t`, all coplanar.
fn point_in(q: [i128; 3], t: [[i128; 3]; 3]) -> bool {
    let [a, b, c] = t;
    let n = cross(sub(b, a), sub(c, a));
    let side = |p: [i128; 3], r: [i128; 3]| dot(n, cross(sub(r, p), sub(q, p)));
    side(a, b) >= 0 && side(b, c) >= 0 && side(c, a) >= 0
}

/// Whether two coplanar closed segments meet.
fn segments_meet(s: [[i128; 3]; 2], t: [[i128; 3]; 2], n: [i128; 3]) -> bool {
    let side = |a: [i128; 3], b: [i128; 3], q: [i128; 3]| dot(n, cross(sub(b, a), sub(q, a)));
    let on = |a: [i128; 3], b: [i128; 3], q: [i128; 3]| {
        side(a, b, q) == 0 && dot(sub(q, a), sub(q, b)) <= 0
    };
    let (a, b) = (side(s[0], s[1], t[0]), side(s[0], s[1], t[1]));
    let (c, d) = (side(t[0], t[1], s[0]), side(t[0], t[1], s[1]));
    ((a > 0 && b < 0) || (a < 0 && b > 0)) && ((c > 0 && d < 0) || (c < 0 && d > 0))
        || on(s[0], s[1], t[0])
        || on(s[0], s[1], t[1])
        || on(t[0], t[1], s[0])
        || on(t[0], t[1], s[1])
}

/// Whether two triangles share a point, by their edges.
fn oracle_meets(a: [P3; 3], b: [P3; 3]) -> bool {
    let (ta, tb) = (a.map(xyz), b.map(xyz));
    let n = cross(sub(tb[1], tb[0]), sub(tb[2], tb[0]));
    let planar = ta.iter().all(|&v| dot(n, sub(v, tb[0])) == 0);
    let edges = |t: [[i128; 3]; 3]| [[t[0], t[1]], [t[1], t[2]], [t[2], t[0]]];
    if planar {
        return edges(ta)
            .iter()
            .any(|&e| edges(tb).iter().any(|&f| segments_meet(e, f, n)))
            || point_in(ta[0], flip_to(tb, n))
            || point_in(tb[0], flip_to(ta, n));
    }
    edges(ta).iter().any(|&e| segment_meets(e, oriented(tb)))
        || edges(tb).iter().any(|&e| segment_meets(e, oriented(ta)))
}

fn oriented(t: [[i128; 3]; 3]) -> [[i128; 3]; 3] {
    t
}

/// `t`, turned to face along `n`.
fn flip_to(t: [[i128; 3]; 3], n: [i128; 3]) -> [[i128; 3]; 3] {
    let m = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    if dot(m, n) < 0 { [t[0], t[2], t[1]] } else { t }
}

fn no_area(t: [P3; 3]) -> bool {
    let [a, b, c] = t.map(xyz);
    cross(sub(b, a), sub(c, a)) == [0; 3]
}

fn triangle(rng: &mut SplitMix64, reach: u64) -> [P3; 3] {
    let mut c = || {
        let v = rng.below(NonZeroU64::new(reach).unwrap()) as i64;
        v
    };
    [p(c(), c(), c()), p(c(), c(), c()), p(c(), c(), c())]
}

#[test]
fn contact_agrees_with_the_edges_on_small_lattices() {
    let seed = resolve("geom-mesh contact lattices", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let (mut apart, mut touch, mut cross_count) = (0, 0, 0);
    for case in 0..60_000 {
        let reach = [3, 4, 6, 9][case % 4];
        let (a, b) = (triangle(&mut rng, reach), triangle(&mut rng, reach));
        if no_area(a) || no_area(b) {
            assert_eq!(triangles(a, b), Err(NumError::Degenerate));
            continue;
        }
        let got = triangles(a, b).unwrap();
        assert_eq!(
            got != Contact::Apart,
            oracle_meets(a, b),
            "{a:?} {b:?}: {got:?}"
        );
        match got {
            Contact::Apart => apart += 1,
            Contact::Touch => touch += 1,
            Contact::Cross => cross_count += 1,
        }
        assert_eq!(triangles(b, a), Ok(got), "swapped");
        let [x, y, z] = a;
        assert_eq!(triangles([y, z, x], b), Ok(got), "turned");
        assert_eq!(triangles([x, z, y], b), Ok(got), "reversed");
        let [u, v, w] = b;
        assert_eq!(triangles(a, [w, u, v]), Ok(got), "turned");
    }
    println!("{apart} apart, {touch} touch, {cross_count} cross");
    assert!(apart > 1000 && touch > 1000 && cross_count > 1000);
}

#[test]
fn a_clean_crossing_is_cross_and_a_shared_edge_touches() {
    let a = [p(0, 0, -5), p(10, 0, -5), p(5, 0, 5)];
    let b = [p(5, -5, 0), p(5, 5, 0), p(-20, 1, 0)];
    assert_eq!(triangles(a, b), Ok(Contact::Cross));
    let on_the_plane = [p(5, -5, 0), p(5, 5, 0), p(-20, 0, 0)];
    assert_eq!(triangles(a, on_the_plane), Ok(Contact::Touch));
    let c = [p(0, 0, 0), p(4, 0, 0), p(0, 4, 0)];
    let d = [p(0, 0, 0), p(4, 0, 0), p(0, 0, 4)];
    assert_eq!(triangles(c, d), Ok(Contact::Touch));
    let e = [p(0, 0, 9), p(4, 0, 9), p(0, 4, 9)];
    assert_eq!(triangles(c, e), Ok(Contact::Apart));
    let f = [p(1, 1, 0), p(2, 1, 0), p(1, 2, 0)];
    assert_eq!(triangles(c, f), Ok(Contact::Touch), "one inside the other");
    let g = [p(9, 9, 0), p(12, 9, 0), p(9, 12, 0)];
    assert_eq!(triangles(c, g), Ok(Contact::Apart), "coplanar and apart");
}

#[test]
fn contact_holds_at_the_edge_of_the_frame() {
    let k = 1_i64 << 35;
    let scaled = |x: i64, y: i64, z: i64| p(x * k, y * k, z * k);
    let a = [scaled(0, 0, -5), scaled(10, 0, -5), scaled(5, 0, 5)];
    let b = [scaled(5, -5, 0), scaled(5, 5, 0), scaled(-20, 1, 0)];
    assert_eq!(triangles(a, b), Ok(Contact::Cross));
    assert_eq!(triangles(b, a), Ok(Contact::Cross));
    let e = (1_i64 << 40) - 1;
    let far = [p(-e, -e, e), p(e, -e, e), p(-e, e, e)];
    let near = [p(-e, -e, e - 1), p(e, -e, e - 1), p(-e, e, e - 1)];
    assert_eq!(triangles(far, near), Ok(Contact::Apart));
    let across = [p(-e, -e, -e), p(e, -e, e), p(-e, e, e)];
    assert_eq!(triangles(far, across), Ok(Contact::Touch));
}

/// Squared distance from `q` to segment `ab`, as a fraction top/bottom.
fn to_segment(q: [i128; 3], a: [i128; 3], b: [i128; 3]) -> (i128, i128) {
    let (ab, aq) = (sub(b, a), sub(q, a));
    let (t, l) = (dot(aq, ab), dot(ab, ab));
    if t <= 0 {
        (dot(aq, aq), 1)
    } else if t >= l {
        let bq = sub(q, b);
        (dot(bq, bq), 1)
    } else {
        (dot(aq, aq) * l - t * t, l)
    }
}

/// Whether the foot of `q` on the plane of `t` is inside `t`, with the squared distance to it.
fn to_face(q: [i128; 3], t: [[i128; 3]; 3]) -> Option<(i128, i128)> {
    let [a, b, c] = t;
    let n = cross(sub(b, a), sub(c, a));
    let height = dot(n, sub(q, a));
    let length = dot(n, n);
    // The foot is q − (height / length) n: scaled by length, test each edge.
    let foot = [
        q[0] * length - height * n[0],
        q[1] * length - height * n[1],
        q[2] * length - height * n[2],
    ];
    let side = |p: [i128; 3], r: [i128; 3]| {
        let scaled = [
            foot[0] - p[0] * length,
            foot[1] - p[1] * length,
            foot[2] - p[2] * length,
        ];
        dot(n, cross(sub(r, p), scaled))
    };
    (side(a, b) >= 0 && side(b, c) >= 0 && side(c, a) >= 0).then_some((height * height, length))
}

/// The smaller of two fractions.
fn less(a: (i128, i128), b: (i128, i128)) -> bool {
    a.0 * b.1 < b.0 * a.1
}

#[test]
fn the_closest_feature_is_as_close_as_any_part_of_the_triangle() {
    let seed = resolve("geom-mesh closest features", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let below = |rng: &mut SplitMix64, n: u64| rng.below(NonZeroU64::new(n).unwrap());
    let mut seen = [0_u32; 3];
    for _ in 0..40_000 {
        let t = triangle(&mut rng, 16);
        if no_area(t) {
            continue;
        }
        let q = p(
            below(&mut rng, 30) as i64 - 7,
            below(&mut rng, 30) as i64 - 7,
            below(&mut rng, 30) as i64 - 7,
        );
        let c = t.map(xyz);
        let qi = xyz(q);
        let edges = [
            to_segment(qi, c[0], c[1]),
            to_segment(qi, c[1], c[2]),
            to_segment(qi, c[2], c[0]),
        ];
        let mut best = edges[0];
        for e in edges {
            if less(e, best) {
                best = e;
            }
        }
        if let Some(f) = to_face(qi, c)
            && less(f, best)
        {
            best = f;
        }
        let got = match closest_feature(q, t).unwrap() {
            Closest::Vertex(i) => {
                seen[0] += 1;
                let d = sub(qi, c[i]);
                (dot(d, d), 1)
            }
            Closest::Edge(i) => {
                seen[1] += 1;
                to_segment(qi, c[i], c[(i + 1) % 3])
            }
            Closest::Face => {
                seen[2] += 1;
                to_face(qi, c).unwrap()
            }
        };
        assert!(
            !less(best, got) && !less(got, best),
            "{q:?} {t:?}: {got:?} against {best:?}"
        );
        // The comparison with a clearance agrees with the fraction on each side of it.
        let clearance = below(&mut rng, 24);
        let c2 = i128::from(clearance) * i128::from(clearance);
        assert_eq!(
            point_triangle(q, t, clearance),
            Ok(sign(best.0 - c2 * best.1)),
            "{q:?} {t:?} {clearance}"
        );
    }
    assert!(seen.iter().all(|&n| n > 100), "{seen:?}");
}

#[test]
fn distances_compare_exactly_at_the_edge_of_the_frame() {
    let e = (1_i64 << 40) - 1;
    let (a, b) = (p(-e, -e, -e), p(e, e, e));
    // The frame's diagonal squared is 3 · (2^41 − 2)².
    let diagonal = 3 * (i128::from(e) * 2).pow(2);
    // The first whole number at or above its square root.
    let (mut low, mut root) = (0_u64, MAX_CLEARANCE);
    while root - low > 1 {
        let middle = low + (root - low) / 2;
        if i128::from(middle) * i128::from(middle) >= diagonal {
            root = middle;
        } else {
            low = middle;
        }
    }
    assert_eq!(distance::point_point(a, b, root), Ok(Sign::Negative));
    assert_eq!(distance::point_point(a, b, root - 1), Ok(Sign::Positive));
    assert_eq!(distance::point_point(a, a, 0), Ok(Sign::Zero));
    assert!(matches!(
        distance::point_point(a, b, MAX_CLEARANCE + 1),
        Err(NumError::OutOfRange { .. })
    ));
    // A point 2e from a line, exactly.
    let on = p(-e, -e, 0);
    assert_eq!(
        distance::point_line(p(-e, -e, -e), p(-e, -e, e), p(e, -e, 0), 2 * e as u64),
        Ok(Sign::Zero)
    );
    assert_eq!(
        distance::point_line(on, on, p(0, 0, 0), 1),
        Err(NumError::Degenerate)
    );
    let flat = [p(-e, -e, -e), p(e, -e, -e), p(-e, e, -e)];
    assert_eq!(
        distance::point_plane(flat[0], flat[1], flat[2], p(e, e, e), 2 * e as u64),
        Ok(Sign::Zero)
    );
    assert_eq!(
        distance::point_plane(flat[0], flat[0], flat[2], p(e, e, e), 1),
        Err(NumError::Degenerate)
    );
    assert_eq!(
        distance::line_line(
            p(-e, -e, -e),
            p(e, -e, -e),
            p(-e, -e, e),
            p(-e, e, e),
            2 * e as u64
        ),
        Ok(Sign::Zero)
    );
    assert_eq!(
        distance::line_line(flat[0], flat[1], flat[0], flat[1], 1),
        Err(NumError::Degenerate)
    );
}

#[test]
fn line_comparisons_agree_with_fractions() {
    let seed = resolve("geom-num distance comparisons", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let below = |rng: &mut SplitMix64, n: u64| rng.below(NonZeroU64::new(n).unwrap());
    for _ in 0..20_000 {
        let mut point = || {
            let mut c = || below(&mut rng, 21) as i64 - 10;
            p(c(), c(), c())
        };
        let (a, b, d, q) = (point(), point(), point(), point());
        let clearance = below(&mut rng, 16);
        let c2 = i128::from(clearance) * i128::from(clearance);
        let (ai, bi, di, qi) = (xyz(a), xyz(b), xyz(d), xyz(q));
        let (ab, aq) = (sub(bi, ai), sub(qi, ai));
        let n = cross(ab, sub(di, ai));
        let length = dot(ab, ab);
        if length != 0 {
            let t = dot(aq, ab);
            let want = sign(dot(aq, aq) * length - t * t - c2 * length);
            assert_eq!(distance::point_line(a, b, q, clearance), Ok(want));
        }
        if n != [0; 3] {
            let h = dot(n, aq);
            let want = sign(h * h - c2 * dot(n, n));
            assert_eq!(distance::point_plane(a, b, d, q, clearance), Ok(want));
            // The line through a and d, against the line through b and q.
            let (u, v) = (sub(di, ai), sub(qi, bi));
            let m = cross(u, v);
            if m != [0; 3] {
                let h = dot(m, sub(bi, ai));
                let want = sign(h * h - c2 * dot(m, m));
                assert_eq!(distance::line_line(a, d, b, q, clearance), Ok(want));
                assert_eq!(distance::line_line(b, q, a, d, clearance), Ok(want));
            }
        }
    }
}

#[test]
fn orientation_is_what_the_contact_asks() {
    // A guard on the sign the test relies on: a, b, c counter-clockwise seen from d is positive.
    assert_eq!(
        orient3d(p(0, 0, 0), p(1, 0, 0), p(0, 1, 0), p(0, 0, 1)),
        Ok(Sign::Positive)
    );
}
