//! Attacks on piece G29: the implicit-point predicates of `geom_num::plane` and
//! `geom_num::crossing`: the three-plane point classified against a fourth plane, the point where a
//! line meets a plane classified against a second plane, and the point where two lines cross in
//! the plane classified against a third line, each routed through its generated filter (chapter
//! 6.2 of the kernel architecture). Every test tries to break a claim of the piece's done-when or
//! of the modules' guarantees.
//!
//! What decides what is right is a big-integer oracle that shares no code and no formulation with
//! the modules: each implicit point is solved by Cramer's rule in parametric form (t along a line,
//! or the three-plane system), on `dashu` integers, and the classified expression is evaluated
//! from that solution. The modules instead form homogeneous coordinates from plane offsets and
//! orientations, and the generated predicates evaluate determinants.
//!
//! - **Random inputs** across the grid and at every scale, against the oracle.
//! - **The corners** (the done-when's maximal-term corners): every coordinate at ±M, M = 2^40 − 1,
//!   where the bit budgets are reached; no step may fail and every sign must match the oracle.
//! - **One step from degenerate**: points built to lie exactly on the classifying plane or line,
//!   then moved one grid step, so the filters must refuse and the exact paths decide.
//! - **Properties**: each implicit point satisfies its defining planes or lines exactly (sign
//!   `Zero`), a crossing built at a grid point commits to that point exactly, and swapping a
//!   line's two points never moves the point.
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Case counts are a twentieth in debug
//! builds.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "points are a to d, p, q and r as in the predicates"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::array;

use dashu_int::IBig;
use geom_num::crossing::Ssi;
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_num::{NumError, Sign};

/// The largest coordinate magnitude, M = 2^40 − 1.
const M: i64 = COORD_MAX;

type Pt2 = [i64; 2];
type Pt3 = [i64; 3];

/// The default seed of every randomised test here (G29, issue 195).
const DEFAULT_SEED: u64 = 0x6729_0000_5eed_0195;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), written out as the other predicate tests do:
/// `geom-num` cannot depend on its own generator for its tests' inputs without testing it with
/// itself.
struct Rng(u64);

impl Rng {
    /// The test's generator: the seed from `SPAREKIT_SEED` (hex) or the default, printed; each test
    /// its own stream, so adding a test never moves another's inputs.
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        let mut name_hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in name.bytes() {
            name_hash = (name_hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        Rng(seed ^ name_hash)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in lo..=hi.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (i128::from(hi) - i128::from(lo) + 1) as u128;
        (i128::from(lo) + (u128::from(self.next()) % span) as i128) as i64
    }

    /// A coordinate: anywhere on the grid, or within 2^k of zero for a random k, so every scale
    /// is reached.
    fn coord(&mut self) -> i64 {
        if self.next() & 1 == 0 {
            self.range(-M, M)
        } else {
            let reach = (1i64 << self.range(0, 40)) - 1;
            self.range(-reach, reach)
        }
    }

    fn p2(&mut self) -> Pt2 {
        array::from_fn(|_| self.coord())
    }

    fn p3(&mut self) -> Pt3 {
        array::from_fn(|_| self.coord())
    }

    /// A corner coordinate: −M, 0 or M.
    fn corner(&mut self) -> i64 {
        [-M, 0, M][self.range(0, 2) as usize]
    }
}

/// Release builds run every count in full; debug builds a twentieth.
fn cases(release: usize) -> usize {
    if cfg!(debug_assertions) {
        (release / 20).max(1)
    } else {
        release
    }
}

fn p2([x, y]: Pt2) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: Pt3) -> P3 {
    P3::new(x, y, z).unwrap()
}

// ---------------------------------------------------------------------------------------------
// The oracle: implicit points solved in parametric form on dashu integers.
// ---------------------------------------------------------------------------------------------

fn big(v: i64) -> IBig {
    IBig::from(v)
}

fn sign_of(v: &IBig) -> Sign {
    match v.signum().cmp(&IBig::ZERO) {
        std::cmp::Ordering::Less => Sign::Negative,
        std::cmp::Ordering::Equal => Sign::Zero,
        std::cmp::Ordering::Greater => Sign::Positive,
    }
}

/// A plane through three grid points, as (normal, offset) in big integers: n = (b − a) × (c − a),
/// d = −n · a.
fn plane_of(a: Pt3, b: Pt3, c: Pt3) -> ([IBig; 3], IBig) {
    let u: [IBig; 3] = array::from_fn(|i| big(b[i]) - big(a[i]));
    let v: [IBig; 3] = array::from_fn(|i| big(c[i]) - big(a[i]));
    let n = [
        &u[1] * &v[2] - &u[2] * &v[1],
        &u[2] * &v[0] - &u[0] * &v[2],
        &u[0] * &v[1] - &u[1] * &v[0],
    ];
    let d = -(&n[0] * big(a[0]) + &n[1] * big(a[1]) + &n[2] * big(a[2]));
    (n, d)
}

fn dot(n: &[IBig; 3], p: &[IBig; 3]) -> IBig {
    &n[0] * &p[0] + &n[1] * &p[1] + &n[2] * &p[2]
}

/// The side of plane 2 for the point where the line from p to q meets plane 1: the point is
/// p + t (q − p) with t = num / den, num = −(n₁ · p + d₁), den = n₁ · (q − p); so n₂ · x + d₂ =
/// ((n₂ · p + d₂) den + num n₂ · (q − p)) / den.
fn lpi_side_oracle(plane1: &([IBig; 3], IBig), plane2: &([IBig; 3], IBig), p: Pt3, q: Pt3) -> Sign {
    let pb: [IBig; 3] = p.map(big);
    let qp: [IBig; 3] = array::from_fn(|i| big(q[i]) - big(p[i]));
    let num = -(dot(&plane1.0, &pb) + &plane1.1);
    let den = dot(&plane1.0, &qp);
    assert_ne!(den, IBig::ZERO, "the oracle's line meets its plane");
    let value = (dot(&plane2.0, &pb) + &plane2.1) * &den + num * dot(&plane2.0, &qp);
    sign_of(&value).mul(sign_of(&den))
}

/// orient2d(p, q, x) for the point x where the line from a to b crosses the line from c to d:
/// x = a + t (b − a) with t = ((c − a) × (d − c)) / ((b − a) × (d − c)), so (q − p) × (x − p) =
/// ((q − p) × ((a − p) den + num (b − a))) / den.
fn ssi_orient_oracle(a: Pt2, b: Pt2, c: Pt2, d: Pt2, p: Pt2, q: Pt2) -> Sign {
    let v = |s: Pt2, t: Pt2| [big(t[0]) - big(s[0]), big(t[1]) - big(s[1])];
    let cross = |u: &[IBig; 2], w: &[IBig; 2]| &u[0] * &w[1] - &u[1] * &w[0];
    let (ba, dc, ca) = (v(a, b), v(c, d), v(a, c));
    let den = cross(&ba, &dc);
    assert_ne!(den, IBig::ZERO, "the oracle's lines cross");
    let num = cross(&ca, &dc);
    let ap = v(p, a);
    let rel = [&ap[0] * &den + &num * &ba[0], &ap[1] * &den + &num * &ba[1]];
    sign_of(&cross(&v(p, q), &rel)).mul(sign_of(&den))
}

/// The side of plane 4 for the point where planes 1 to 3 meet, solved by Cramer's rule:
/// N x = −D with rows nₖ, so `x_i` = det(Nᵢ) / det(N) where Nᵢ has column i replaced by −D.
fn tpi_side_oracle(planes: &[([IBig; 3], IBig); 3], s: &([IBig; 3], IBig)) -> Sign {
    let det3 = |m: &[[IBig; 3]; 3]| {
        &m[0][0] * (&m[1][1] * &m[2][2] - &m[1][2] * &m[2][1])
            - &m[0][1] * (&m[1][0] * &m[2][2] - &m[1][2] * &m[2][0])
            + &m[0][2] * (&m[1][0] * &m[2][1] - &m[1][1] * &m[2][0])
    };
    let n: [[IBig; 3]; 3] = array::from_fn(|k| planes[k].0.clone());
    let w = det3(&n);
    assert_ne!(w, IBig::ZERO, "the oracle's planes meet in a point");
    let x: [IBig; 3] = array::from_fn(|i| {
        let mut m = n.clone();
        for k in 0..3 {
            m[k][i] = -planes[k].1.clone();
        }
        det3(&m)
    });
    sign_of(&(dot(&s.0, &x) + &s.1 * &w)).mul(sign_of(&w))
}

// ---------------------------------------------------------------------------------------------
// Building inputs.
// ---------------------------------------------------------------------------------------------

/// A plane through three grid points, from the module and the oracle, or `None` when they are
/// collinear.
fn planes(a: Pt3, b: Pt3, c: Pt3) -> Option<(Plane, ([IBig; 3], IBig))> {
    match Plane::through(p3(a), p3(b), p3(c)) {
        Ok(plane) => Some((plane, plane_of(a, b, c))),
        Err(NumError::Degenerate) => None,
        Err(e) => panic!("Plane::through({a:?}, {b:?}, {c:?}): {e:?}"),
    }
}

/// Checks the line-plane predicate on one input against the oracle; `false` when the input is
/// degenerate (the line does not meet plane 1 in a point, or a plane is not one).
fn check_lpi(tri1: [Pt3; 3], tri2: [Pt3; 3], p: Pt3, q: Pt3) -> bool {
    let (Some((plane1, o1)), Some((plane2, o2))) = (
        planes(tri1[0], tri1[1], tri1[2]),
        planes(tri2[0], tri2[1], tri2[2]),
    ) else {
        return false;
    };
    let l = match Lpi::new(p3(p), p3(q), plane1) {
        Ok(l) => l,
        Err(NumError::Degenerate) => return false,
        Err(e) => panic!("Lpi::new: {e:?} for {tri1:?} {p:?} {q:?}"),
    };
    let got = plane2.side_of_lpi(&l).unwrap();
    let want = lpi_side_oracle(&o1, &o2, p, q);
    assert_eq!(got, want, "side_of_lpi: {tri1:?} {tri2:?} line {p:?} {q:?}");
    assert_eq!(
        plane1.side_of_lpi(&l).unwrap(),
        Sign::Zero,
        "the point lies on its own plane"
    );
    let swapped = Lpi::new(p3(q), p3(p), plane1).unwrap();
    assert_eq!(
        plane2.side_of_lpi(&swapped).unwrap(),
        got,
        "swapping p and q moves nothing"
    );
    true
}

/// Checks the crossing predicate on one input against the oracle; `false` when the lines are
/// parallel or a classifying line is not one.
fn check_ssi(a: Pt2, b: Pt2, c: Pt2, d: Pt2, p: Pt2, q: Pt2) -> bool {
    let s = match Ssi::new(p2(a), p2(b), p2(c), p2(d)) {
        Ok(s) => s,
        Err(NumError::Degenerate) => return false,
        Err(e) => panic!("Ssi::new: {e:?}"),
    };
    let got = s.orient2d(p2(p), p2(q)).unwrap();
    if p == q {
        assert_eq!(
            got,
            Sign::Zero,
            "a line of one point classifies everything as Zero"
        );
    } else {
        assert_eq!(
            got,
            ssi_orient_oracle(a, b, c, d, p, q),
            "orient2d: {a:?} {b:?} {c:?} {d:?} against {p:?} {q:?}"
        );
        let flipped = s.orient2d(p2(q), p2(p)).unwrap();
        assert_eq!(flipped, got.negate(), "swapping p and q flips the sign");
    }
    assert_eq!(
        s.orient2d(p2(a), p2(b)).unwrap(),
        Sign::Zero,
        "the point lies on the first line"
    );
    assert_eq!(
        s.orient2d(p2(c), p2(d)).unwrap(),
        Sign::Zero,
        "the point lies on the second line"
    );
    let swapped = Ssi::new(p2(b), p2(a), p2(d), p2(c)).unwrap();
    assert_eq!(
        swapped.orient2d(p2(p), p2(q)).unwrap(),
        got,
        "swapping a line's points moves nothing"
    );
    true
}

/// Checks the three-plane predicate on one input against the oracle; `false` when degenerate.
fn check_tpi(tris: [[Pt3; 3]; 3], fourth: [Pt3; 3]) -> bool {
    let mut ours = Vec::new();
    let mut theirs = Vec::new();
    for t in tris {
        let Some((plane, o)) = planes(t[0], t[1], t[2]) else {
            return false;
        };
        ours.push(plane);
        theirs.push(o);
    }
    let Some((s, os)) = planes(fourth[0], fourth[1], fourth[2]) else {
        return false;
    };
    let t = match Tpi::new(ours[0], ours[1], ours[2]) {
        Ok(t) => t,
        Err(NumError::Degenerate) => return false,
        Err(e) => panic!("Tpi::new: {e:?}"),
    };
    let theirs: [([IBig; 3], IBig); 3] = theirs.try_into().unwrap();
    assert_eq!(
        s.side_of_tpi(&t).unwrap(),
        tpi_side_oracle(&theirs, &os),
        "side_of_tpi: {tris:?} against {fourth:?}"
    );
    for plane in &ours {
        assert_eq!(
            plane.side_of_tpi(&t).unwrap(),
            Sign::Zero,
            "the point lies on each of its planes"
        );
    }
    true
}

// ---------------------------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------------------------

#[test]
fn random_inputs_at_every_scale_against_the_oracle() {
    let mut rng = Rng::for_test("random_inputs_at_every_scale_against_the_oracle");
    let (mut lpi, mut ssi, mut tpi) = (0, 0, 0);
    for _ in 0..cases(4000) {
        let tri = |rng: &mut Rng| [rng.p3(), rng.p3(), rng.p3()];
        lpi += usize::from(check_lpi(tri(&mut rng), tri(&mut rng), rng.p3(), rng.p3()));
        ssi += usize::from(check_ssi(
            rng.p2(),
            rng.p2(),
            rng.p2(),
            rng.p2(),
            rng.p2(),
            rng.p2(),
        ));
        tpi += usize::from(check_tpi(
            [tri(&mut rng), tri(&mut rng), tri(&mut rng)],
            tri(&mut rng),
        ));
    }
    let floor = cases(4000) * 9 / 10;
    assert!(
        lpi >= floor && ssi >= floor && tpi >= floor,
        "too few cases ran: {lpi} {ssi} {tpi}"
    );
}

#[test]
fn maximal_term_corners_never_fail_and_match_the_oracle() {
    let mut rng = Rng::for_test("maximal_term_corners_never_fail_and_match_the_oracle");
    let mut ran = 0;
    for _ in 0..cases(4000) {
        let c3 = |rng: &mut Rng| -> Pt3 { array::from_fn(|_| rng.corner()) };
        let c2 = |rng: &mut Rng| -> Pt2 { array::from_fn(|_| rng.corner()) };
        let tri = |rng: &mut Rng| [c3(rng), c3(rng), c3(rng)];
        ran += usize::from(check_lpi(
            tri(&mut rng),
            tri(&mut rng),
            c3(&mut rng),
            c3(&mut rng),
        ));
        ran += usize::from(check_ssi(
            c2(&mut rng),
            c2(&mut rng),
            c2(&mut rng),
            c2(&mut rng),
            c2(&mut rng),
            c2(&mut rng),
        ));
        ran += usize::from(check_tpi(
            [tri(&mut rng), tri(&mut rng), tri(&mut rng)],
            tri(&mut rng),
        ));
    }
    assert!(ran > cases(4000), "too few corner cases ran: {ran}");
}

/// The largest coordinates the frame allows on every axis at once: a line from one corner of the
/// cube to the opposite one, meeting a plane through three other corners, classified against a
/// plane through three more; and two diagonals of the square crossing, classified against an edge.
#[test]
fn the_cube_and_square_diagonals_reach_the_budgets_without_failing() {
    let corners: Vec<Pt3> = (0..8)
        .map(|i| {
            [
                if i & 1 == 0 { -M } else { M },
                if i & 2 == 0 { -M } else { M },
                if i & 4 == 0 { -M } else { M },
            ]
        })
        .collect();
    let mut ran = 0;
    for i in 0..8 {
        for j in 0..8 {
            for k in 0..8 {
                if i < j && j < k {
                    let tri = [corners[i], corners[j], corners[k]];
                    let other = [corners[7 - i], corners[7 - j], corners[7 - k]];
                    ran += usize::from(check_lpi(tri, other, corners[0], corners[7]));
                    ran += usize::from(check_lpi(other, tri, corners[3], corners[4]));
                }
            }
        }
    }
    assert!(ran > 0);
    assert!(check_ssi(
        [-M, -M],
        [M, M],
        [-M, M],
        [M, -M],
        [-M, -M],
        [M, -M]
    ));
    assert!(check_ssi(
        [-M, -M],
        [M, M - 1],
        [-M, M],
        [M, -M + 1],
        [M, M],
        [-M, M]
    ));
}

/// Points built to lie exactly on the classifying plane or line, then one grid step off it: the
/// exact sign is zero, then the sign of the step, which the filters cannot decide by margin.
#[test]
fn one_step_from_degenerate_is_decided_exactly() {
    let mut rng = Rng::for_test("one_step_from_degenerate_is_decided_exactly");
    for _ in 0..cases(2000) {
        // A crossing at a grid point x: two lines through x, classified against a line through x
        // and a point one step off it.
        let x: Pt2 = array::from_fn(|_| rng.range(-M / 4, M / 4));
        let step = |rng: &mut Rng| -> Pt2 { array::from_fn(|_| rng.range(-(1 << 20), 1 << 20)) };
        let (u, v) = (step(&mut rng), step(&mut rng));
        let (a, b) = ([x[0] - u[0], x[1] - u[1]], [x[0] + u[0], x[1] + u[1]]);
        let (c, d) = ([x[0] - v[0], x[1] - v[1]], [x[0] + v[0], x[1] + v[1]]);
        let w = step(&mut rng);
        let q = [x[0] + w[0], x[1] + w[1]];
        if let Ok(s) = Ssi::new(p2(a), p2(b), p2(c), p2(d)) {
            assert_eq!(
                s.orient2d(p2(x), p2(q)).unwrap(),
                Sign::Zero,
                "x lies on the line from x"
            );
            for off in [[1, 0], [0, 1], [-1, 0], [0, -1]] {
                let p = [x[0] + off[0], x[1] + off[1]];
                if p != q {
                    check_ssi(a, b, c, d, p, q);
                }
            }
            let grid = s.to_grid().unwrap();
            assert!(
                grid.is_exact(),
                "a crossing at a grid point commits exactly"
            );
            assert_eq!(grid.value(), p2(x), "and commits to that point");
        }
        // A line through a grid point y meeting a plane through y, classified against planes
        // through y and through its neighbours.
        let y: Pt3 = array::from_fn(|_| rng.range(-M / 4, M / 4));
        let s3 = |rng: &mut Rng| -> Pt3 { array::from_fn(|_| rng.range(-(1 << 13), 1 << 13)) };
        let (e, f, g) = (s3(&mut rng), s3(&mut rng), s3(&mut rng));
        let add = |p: Pt3, d: Pt3| -> Pt3 { array::from_fn(|i| p[i] + d[i]) };
        let sub = |p: Pt3, d: Pt3| -> Pt3 { array::from_fn(|i| p[i] - d[i]) };
        let tri1 = [y, add(y, e), add(y, f)];
        let (p, q) = (sub(y, g), add(y, g));
        for off in [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1], [-1, 0, 0]] {
            let tri2 = [add(y, off), add(y, f), sub(y, e)];
            check_lpi(tri1, tri2, p, q);
        }
        if let Some((plane1, _)) = planes(tri1[0], tri1[1], tri1[2]) {
            if let Ok(l) = Lpi::new(p3(p), p3(q), plane1) {
                let grid = l.to_grid().unwrap();
                assert!(
                    grid.is_exact(),
                    "a line-plane point at a grid point commits exactly"
                );
                assert_eq!(grid.value(), p3(y), "and commits to that point");
            }
        }
    }
}

#[test]
fn degenerate_inputs_are_refused_never_answered() {
    let plane = Plane::through(p3([0, 0, 0]), p3([1, 0, 0]), p3([0, 1, 0])).unwrap();
    // A line parallel to the plane, a line inside it and a line of one point.
    for (p, q) in [
        ([0, 0, 5], [7, 3, 5]),
        ([1, 1, 0], [9, -4, 0]),
        ([2, 3, 4], [2, 3, 4]),
    ] {
        assert_eq!(
            Lpi::new(p3(p), p3(q), plane),
            Err(NumError::Degenerate),
            "{p:?} {q:?}"
        );
    }
    // Parallel lines, the same line twice and a line of one point.
    assert_eq!(
        Ssi::new(p2([0, 0]), p2([2, 1]), p2([0, 5]), p2([4, 7])),
        Err(NumError::Degenerate)
    );
    assert_eq!(
        Ssi::new(p2([0, 0]), p2([2, 1]), p2([4, 2]), p2([6, 3])),
        Err(NumError::Degenerate)
    );
    assert_eq!(
        Ssi::new(p2([3, 3]), p2([3, 3]), p2([0, 5]), p2([4, 7])),
        Err(NumError::Degenerate)
    );
}

/// Two lines that cross far outside the frame: the point is exact and classified exactly, and
/// committing it reports `OutOfRange` instead of a wrong point.
#[test]
fn a_crossing_outside_the_frame_is_classified_and_refuses_to_commit() {
    let (a, b, c, d) = ([-M, 0], [M, 1], [-M, 2], [M, 4]);
    let s = Ssi::new(p2(a), p2(b), p2(c), p2(d)).unwrap();
    assert!(check_ssi(a, b, c, d, [0, 0], [1, 0]));
    assert!(
        matches!(s.to_grid(), Err(NumError::OutOfRange { .. })),
        "{:?}",
        s.to_grid()
    );
}
