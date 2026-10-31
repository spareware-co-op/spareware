//! Piece G34: the predicate verification suite (chapter 28.6 of the kernel architecture; 28.16
//! for fixtures, 28.17 for when it runs).
//!
//! The done-when: "Every predicate compared with a big-integer oracle (num-bigint and
//! num-rational, or dashu, 28.6) on random, ±1-step-degenerate and filter-boundary inputs on every
//! pull request; corner sets as above; perturbation laws; failures minimised into fixtures;
//! nightly fresh seeds."
//!
//! Most of it already runs in the predicates' own attack tests (G28, G29, G30 and the plane tests
//! before them). This suite holds them together and adds what was missing:
//!
//! - **The matrix** (`every_predicate_is_verified_in_every_class`): for every public predicate of
//!   `geom-num` and every class chapter 28.6 names (random inputs, inputs one step from exact
//!   degeneracy, inputs on both sides of the filter's bound, the corner sets, the laws), the test
//!   that compares it with a big-integer oracle, by file and function. Each named test must exist
//!   and reach the predicate in its code, directly or through the functions and constants of its
//!   file it uses (`geom_test::rust_source::references`, so the matrix's own strings never count:
//!   gate 1, piece G43, found that a file-wide search let them); a public predicate the matrix does
//!   not list fails it, so a new predicate cannot merge unverified.
//! - **The perturbed predicates' corners and ties** (G43): all four at every sign corner of their
//!   unperturbed predicates, insphere's 32,768 included, and on ties anywhere on the grid against
//!   `symbolic_sign`, an oracle that multiplies the perturbed determinant out into monomials of ε,
//!   so insphere's ties, which `attack_g30.rs`'s concrete ε cannot afford, meet an oracle too.
//! - **The filter boundary of the implicit-point predicates**, the class G29's tests did not have:
//!   `side_of_tpi`, `side_of_lpi` and `Ssi::orient2d`, each on families of inputs whose exact value
//!   grows step by step from zero, searched for the step where the generated filter starts to
//!   decide; every input in a window around that step is compared with a rational oracle written
//!   here (the implicit point solved by Cramer's rule or along its line in `dashu` rationals, then
//!   classified), so the filter's answers just outside its bound and the exact path's just inside
//!   are both checked. The suite counts both kinds and fails if either is missing.
//! - **The laws of the line-plane point and the crossing**, which had none: swapping a line's two
//!   points keeps the answer, reversing the classifying plane or line flips it, and the crossing
//!   does not depend on which line comes first.
//! - **Fixtures** (`every_fixture_replays`): each line of `tests/verification/fixtures.txt` is a
//!   minimised input with its oracle's sign, replayed on every run against the predicate and the
//!   oracle. Every property here runs through `geom_test::prop`, so a failure is shrunk to a
//!   minimal input, and its report carries that input as a fixture line, ready to add once the bug
//!   is fixed (28.16). `a_failure_shrinks_into_a_fixture_that_catches_it` proves the route on a
//!   predicate broken on purpose: the shrunk fixture fails the broken predicate and passes the
//!   real one.
//! - **Nightly fresh seeds**: `.github/workflows/kernel-nightly.yml` runs this suite and the
//!   predicates' attack tests every night with `SPAREKIT_SEED` taken from the run, printed; on
//!   pull requests every seed is the fixed default (28.17).
//!
//! Seeds: each randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another. Case counts are a twentieth in debug builds.

// Tests may use native arithmetic, casts and std's reductions to build their fixtures and oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    reason = "fixtures and oracles; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken fixture must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::cell::RefCell;

use dashu_int::IBig;
use dashu_ratio::RBig;
use geom_num::crossing::Ssi;
use geom_num::generated::{lpi_side, ssi_side, tpi_side};
use geom_num::grid::{COORD_MAX, P2, P3};
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_num::{NumError, Sign, perturb, predicate};
use geom_test::prop;
use geom_test::rust_source::references;
use geom_test::seed::Seed;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// The fixtures: one minimised input per line, with its oracle's sign.
const FIXTURES: &str = include_str!("verification/fixtures.txt");

/// The cases a property runs: `release` in release builds, a twentieth in debug builds.
fn cases(release: u32) -> u32 {
    if cfg!(debug_assertions) {
        (release / 20).max(2)
    } else {
        release
    }
}

// ---------------------------------------------------------------------------------------------
// Points and the oracle.

type Pt2 = [i64; 2];
type Pt3 = [i64; 3];

fn p2([x, y]: Pt2) -> P2 {
    P2::new(x, y).unwrap()
}

fn p3([x, y, z]: Pt3) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn on_grid(v: &[i64]) -> bool {
    v.iter().all(|c| c.abs() <= COORD_MAX)
}

fn big(v: i64) -> IBig {
    IBig::from(v)
}

fn sign_of(v: &IBig) -> Sign {
    match v.cmp(&IBig::ZERO) {
        core::cmp::Ordering::Greater => Sign::Positive,
        core::cmp::Ordering::Less => Sign::Negative,
        core::cmp::Ordering::Equal => Sign::Zero,
    }
}

fn sign_of_ratio(v: &RBig) -> Sign {
    sign_of(&v.numerator().clone())
}

/// A determinant by cofactor expansion along the first row: the definition, n at most 5 here.
fn det(m: &[Vec<IBig>]) -> IBig {
    if m.len() == 1 {
        return m[0][0].clone();
    }
    let mut total = IBig::ZERO;
    for (j, entry) in m[0].iter().enumerate() {
        let minor: Vec<Vec<IBig>> = m[1..]
            .iter()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .filter(|&(k, _)| k != j)
                    .map(|(_, v)| v.clone())
                    .collect()
            })
            .collect();
        let term = entry * det(&minor);
        if j % 2 == 0 {
            total += term;
        } else {
            total -= term;
        }
    }
    total
}

fn diff<const D: usize>(p: [i64; D], q: [i64; D]) -> Vec<IBig> {
    p.iter()
        .zip(q.iter())
        .map(|(a, b)| big(*a) - big(*b))
        .collect()
}

fn lifted<const D: usize>(p: [i64; D], o: [i64; D]) -> Vec<IBig> {
    let d = diff(p, o);
    let norm = d.iter().fold(IBig::ZERO, |acc, v| acc + v * v);
    std::iter::once(norm).chain(d).collect()
}

fn oracle_orient2d(a: Pt2, b: Pt2, c: Pt2) -> Sign {
    sign_of(&det(&[diff(b, a), diff(c, a)]))
}

fn oracle_orient3d(a: Pt3, b: Pt3, c: Pt3, d: Pt3) -> Sign {
    sign_of(&det(&[diff(b, a), diff(c, a), diff(d, a)]))
}

fn oracle_incircle(a: Pt2, b: Pt2, c: Pt2, d: Pt2) -> Sign {
    sign_of(&det(&[lifted(a, d), lifted(b, d), lifted(c, d)]))
}

fn oracle_insphere(a: Pt3, b: Pt3, c: Pt3, d: Pt3, e: Pt3) -> Sign {
    sign_of(&det(&[
        lifted(a, e),
        lifted(b, e),
        lifted(c, e),
        lifted(d, e),
    ]))
}

/// The plane through a, b, c as rationals: n = (b − a) × (c − a), d = −n · a.
fn oracle_plane([a, b, c]: [Pt3; 3]) -> ([RBig; 3], RBig) {
    let u = diff(b, a);
    let v = diff(c, a);
    let n = [
        &u[1] * &v[2] - &u[2] * &v[1],
        &u[2] * &v[0] - &u[0] * &v[2],
        &u[0] * &v[1] - &u[1] * &v[0],
    ];
    let d = -(&n[0] * big(a[0]) + &n[1] * big(a[1]) + &n[2] * big(a[2]));
    (n.map(RBig::from), RBig::from(d))
}

fn value_at((n, d): &([RBig; 3], RBig), x: &[RBig; 3]) -> RBig {
    &n[0] * &x[0] + &n[1] * &x[1] + &n[2] * &x[2] + d
}

fn rational(p: Pt3) -> [RBig; 3] {
    p.map(|v| RBig::from(big(v)))
}

fn oracle_plane_side(plane: [Pt3; 3], p: Pt3) -> Sign {
    sign_of_ratio(&value_at(&oracle_plane(plane), &rational(p)))
}

/// The point where three planes meet, by Cramer's rule over the rationals, classified against a
/// fourth plane.
fn oracle_side_of_tpi(planes: [[Pt3; 3]; 3], s: [Pt3; 3]) -> Sign {
    let ps = planes.map(oracle_plane);
    let m: Vec<Vec<RBig>> = ps.iter().map(|(n, _)| n.to_vec()).collect();
    let rhs: Vec<RBig> = ps.iter().map(|(_, d)| -d.clone()).collect();
    let det3 = |m: &[Vec<RBig>]| -> RBig {
        &m[0][0] * (&m[1][1] * &m[2][2] - &m[1][2] * &m[2][1])
            - &m[0][1] * (&m[1][0] * &m[2][2] - &m[1][2] * &m[2][0])
            + &m[0][2] * (&m[1][0] * &m[2][1] - &m[1][1] * &m[2][0])
    };
    let w = det3(&m);
    let x: [RBig; 3] = std::array::from_fn(|j| {
        let mj: Vec<Vec<RBig>> = m
            .iter()
            .zip(&rhs)
            .map(|(row, r)| {
                let mut row = row.clone();
                row[j].clone_from(r);
                row
            })
            .collect();
        det3(&mj) / &w
    });
    sign_of_ratio(&value_at(&oracle_plane(s), &x))
}

/// The point where the line from p to q meets a plane, along the line over the rationals,
/// classified against a second plane.
fn oracle_side_of_lpi(p: Pt3, q: Pt3, plane: [Pt3; 3], s: [Pt3; 3]) -> Sign {
    let first = oracle_plane(plane);
    let (pr, qr) = (rational(p), rational(q));
    let dir: [RBig; 3] = std::array::from_fn(|i| &qr[i] - &pr[i]);
    let along = &first.0[0] * &dir[0] + &first.0[1] * &dir[1] + &first.0[2] * &dir[2];
    let t = -value_at(&first, &pr) / along;
    let x: [RBig; 3] = std::array::from_fn(|i| &pr[i] + &t * &dir[i]);
    sign_of_ratio(&value_at(&oracle_plane(s), &x))
}

fn cross2(u: &[RBig; 2], v: &[RBig; 2]) -> RBig {
    &u[0] * &v[1] - &u[1] * &v[0]
}

fn rational2(p: Pt2) -> [RBig; 2] {
    p.map(|v| RBig::from(big(v)))
}

fn minus2(p: &[RBig; 2], q: &[RBig; 2]) -> [RBig; 2] {
    [&p[0] - &q[0], &p[1] - &q[1]]
}

/// The point where the lines a–b and c–d cross, over the rationals, classified against the line
/// from p to q: the sign of orient2d(p, q, x).
fn oracle_ssi(a: Pt2, b: Pt2, c: Pt2, d: Pt2, p: Pt2, q: Pt2) -> Sign {
    let [a, b, c, d, p, q] = [a, b, c, d, p, q].map(rational2);
    let s = cross2(&minus2(&c, &a), &minus2(&d, &c)) / cross2(&minus2(&b, &a), &minus2(&d, &c));
    let ba = minus2(&b, &a);
    let x = [&a[0] + &s * &ba[0], &a[1] + &s * &ba[1]];
    sign_of_ratio(&cross2(&minus2(&q, &p), &minus2(&x, &p)))
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

/// A predicate a fixture can name, and how many points (of which dimension) it takes.
const FIXTURE_PREDICATES: [(&str, usize, usize); 8] = [
    ("orient2d", 2, 3),
    ("orient3d", 3, 4),
    ("incircle", 2, 4),
    ("insphere", 3, 5),
    ("plane_side", 3, 4),
    ("side_of_tpi", 3, 12),
    ("side_of_lpi", 3, 8),
    ("ssi_orient2d", 2, 6),
];

/// One fixture: a predicate, its points (every coordinate in order) and the expected sign.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fixture {
    predicate: String,
    points: Vec<Vec<i64>>,
    sign: Sign,
}

fn sign_word(s: Sign) -> &'static str {
    match s {
        Sign::Negative => "Negative",
        Sign::Zero => "Zero",
        Sign::Positive => "Positive",
    }
}

/// The fixture as one line of `fixtures.txt`: `<predicate> | <points> | <sign>`, points
/// separated by `;` and coordinates by `,`.
fn fixture_line(f: &Fixture) -> String {
    let points: Vec<String> = f
        .points
        .iter()
        .map(|p| p.iter().map(i64::to_string).collect::<Vec<_>>().join(","))
        .collect();
    format!(
        "{} | {} | {}",
        f.predicate,
        points.join(";"),
        sign_word(f.sign)
    )
}

/// A fixture line read back, refusing any line that is not one.
fn parse_fixture(line: &str) -> Result<Fixture, String> {
    let parts: Vec<&str> = line.split('|').map(str::trim).collect();
    let [predicate, points, sign] = parts.as_slice() else {
        return Err(format!("not three fields: {line}"));
    };
    let Some(&(_, dim, count)) = FIXTURE_PREDICATES.iter().find(|(n, ..)| n == predicate) else {
        return Err(format!("no predicate `{predicate}`"));
    };
    let points: Vec<Vec<i64>> = points
        .split(';')
        .map(|p| p.split(',').map(|c| c.trim().parse::<i64>()).collect())
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{e}: {line}"))?;
    if points.len() != count || points.iter().any(|p| p.len() != dim || !on_grid(p)) {
        return Err(format!(
            "`{predicate}` takes {count} grid points of {dim} coordinates: {line}"
        ));
    }
    let sign = match *sign {
        "Negative" => Sign::Negative,
        "Zero" => Sign::Zero,
        "Positive" => Sign::Positive,
        other => return Err(format!("no sign `{other}`")),
    };
    Ok(Fixture {
        predicate: (*predicate).to_owned(),
        points,
        sign,
    })
}

fn pt2_of(v: &[i64]) -> Pt2 {
    [v[0], v[1]]
}

fn pt3_of(v: &[i64]) -> Pt3 {
    [v[0], v[1], v[2]]
}

fn plane_of(points: &[Vec<i64>]) -> [Pt3; 3] {
    [pt3_of(&points[0]), pt3_of(&points[1]), pt3_of(&points[2])]
}

/// What the predicate says, and what the oracle says, on a fixture's points.
fn evaluate(f: &Fixture) -> (Result<Sign, NumError>, Sign) {
    let pts = &f.points;
    let q2 = |i: usize| pt2_of(&pts[i]);
    let q3 = |i: usize| pt3_of(&pts[i]);
    let built = |planes: &[[Pt3; 3]]| -> Result<Vec<Plane>, NumError> {
        planes
            .iter()
            .map(|[a, b, c]| Plane::through(p3(*a), p3(*b), p3(*c)))
            .collect()
    };
    match f.predicate.as_str() {
        "orient2d" => (
            predicate::orient2d(p2(q2(0)), p2(q2(1)), p2(q2(2))),
            oracle_orient2d(q2(0), q2(1), q2(2)),
        ),
        "orient3d" => (
            predicate::orient3d(p3(q3(0)), p3(q3(1)), p3(q3(2)), p3(q3(3))),
            oracle_orient3d(q3(0), q3(1), q3(2), q3(3)),
        ),
        "incircle" => (
            predicate::incircle(p2(q2(0)), p2(q2(1)), p2(q2(2)), p2(q2(3))),
            oracle_incircle(q2(0), q2(1), q2(2), q2(3)),
        ),
        "insphere" => (
            predicate::insphere(p3(q3(0)), p3(q3(1)), p3(q3(2)), p3(q3(3)), p3(q3(4))),
            oracle_insphere(q3(0), q3(1), q3(2), q3(3), q3(4)),
        ),
        "plane_side" => {
            let plane = plane_of(&pts[0..3]);
            (
                built(&[plane]).and_then(|pl| pl[0].side(p3(q3(3)))),
                oracle_plane_side(plane, q3(3)),
            )
        }
        "side_of_tpi" => {
            let planes = [
                plane_of(&pts[0..3]),
                plane_of(&pts[3..6]),
                plane_of(&pts[6..9]),
            ];
            let s = plane_of(&pts[9..12]);
            let answer = built(&[planes[0], planes[1], planes[2], s])
                .and_then(|pl| Tpi::new(pl[0], pl[1], pl[2]).and_then(|t| pl[3].side_of_tpi(&t)));
            (answer, oracle_side_of_tpi(planes, s))
        }
        "side_of_lpi" => {
            let (plane, s) = (plane_of(&pts[2..5]), plane_of(&pts[5..8]));
            let answer = built(&[plane, s]).and_then(|pl| {
                Lpi::new(p3(q3(0)), p3(q3(1)), pl[0]).and_then(|l| pl[1].side_of_lpi(&l))
            });
            (answer, oracle_side_of_lpi(q3(0), q3(1), plane, s))
        }
        "ssi_orient2d" => {
            let answer = Ssi::new(p2(q2(0)), p2(q2(1)), p2(q2(2)), p2(q2(3)))
                .and_then(|x| x.orient2d(p2(q2(4)), p2(q2(5))));
            (answer, oracle_ssi(q2(0), q2(1), q2(2), q2(3), q2(4), q2(5)))
        }
        other => panic!("no predicate `{other}`"),
    }
}

/// The fixtures in `fixtures.txt`: every line that is not blank or a `#` comment.
fn fixtures() -> Vec<(usize, Result<Fixture, String>)> {
    FIXTURES
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|(i, l)| (i + 1, parse_fixture(l)))
        .collect()
}

#[test]
fn every_fixture_replays() {
    for (line, fixture) in fixtures() {
        let f = fixture.unwrap_or_else(|e| panic!("fixtures.txt line {line}: {e}"));
        let (answer, oracle) = evaluate(&f);
        assert_eq!(
            oracle, f.sign,
            "line {line}: the oracle disagrees with the recorded sign"
        );
        assert_eq!(answer, Ok(f.sign), "line {line}: {}", fixture_line(&f));
    }
}

/// A property's verdict on one input: `Ok`, or a failure whose reason carries the input as a
/// fixture line between `fixture[` and `]`, so the shrunk failure's report holds the fixture.
fn verdict(f: &Fixture, answer: Result<Sign, NumError>) -> Result<(), TestCaseError> {
    if answer == Ok(f.sign) {
        Ok(())
    } else {
        Err(TestCaseError::fail(format!(
            "answered {answer:?} where the oracle says {:?}: fixture[{}]",
            f.sign,
            fixture_line(f)
        )))
    }
}

/// The fixture line a failed run's report carries.
fn fixture_in(report: &str) -> Option<&str> {
    let start = report.rfind("fixture[")? + "fixture[".len();
    let end = report[start..].find(']')? + start;
    Some(&report[start..end])
}

fn orient2d_fixture(a: Pt2, b: Pt2, c: Pt2) -> Fixture {
    Fixture {
        predicate: "orient2d".to_owned(),
        points: vec![a.to_vec(), b.to_vec(), c.to_vec()],
        sign: oracle_orient2d(a, b, c),
    }
}

fn grid_pt2() -> impl Strategy<Value = Pt2> {
    prop::p2().prop_map(|p| [p.x.get(), p.y.get()])
}

/// The route of 28.16 on a predicate broken on purpose (orient2d with its sign flipped whenever
/// a's x is at least 1000): the property fails, proptest shrinks the input, the report carries
/// the minimal input as a fixture line, and that fixture fails the broken predicate and passes
/// the real one.
#[test]
fn a_failure_shrinks_into_a_fixture_that_catches_it() {
    let broken = |a: P2, b: P2, c: P2| -> Result<Sign, NumError> {
        let s = predicate::orient2d(a, b, c)?;
        Ok(if a.x.get() >= 1000 { s.negate() } else { s })
    };
    let strategy = (grid_pt2(), grid_pt2(), grid_pt2());
    let failure = prop::run(
        "a_failure_shrinks_into_a_fixture_that_catches_it",
        Seed(0x6734_0000_0000_0001),
        256,
        &strategy,
        |(a, b, c)| {
            let f = orient2d_fixture(a, b, c);
            verdict(&f, broken(p2(a), p2(b), p2(c)))
        },
    )
    .unwrap_err();
    let prop::Failure::Property { report, .. } = failure else {
        panic!("not a property failure: {failure}");
    };
    let line = fixture_in(&report).unwrap_or_else(|| panic!("no fixture in: {report}"));
    let f = parse_fixture(line).unwrap();
    // Shrunk: a's x at the threshold, the smallest input that still fails.
    assert_eq!(f.points[0][0], 1000, "{line}");
    let [a, b, c] = [0, 1, 2].map(|i| p2(pt2_of(&f.points[i])));
    assert_ne!(
        broken(a, b, c),
        Ok(f.sign),
        "the fixture must catch the bug"
    );
    assert_eq!(
        evaluate(&f),
        (Ok(f.sign), f.sign),
        "the fixture must pass the real predicate"
    );
}

#[test]
fn fixture_lines_round_trip_and_refuse_what_is_not_one() {
    let f = Fixture {
        predicate: "ssi_orient2d".to_owned(),
        points: vec![
            vec![1, -2],
            vec![3, 4],
            vec![-5, 6],
            vec![7, 8],
            vec![0, 0],
            vec![9, -9],
        ],
        sign: Sign::Negative,
    };
    assert_eq!(parse_fixture(&fixture_line(&f)), Ok(f));
    for bad in [
        "orient2d | 0,0;1,0 | Zero",
        "orient2d | 0,0;1,0;0,1 | Sideways",
        "orient9d | 0,0;1,0;0,1 | Zero",
        "orient2d | 0,0;1,0;0,1,2 | Zero",
        "orient2d | 0,0;1,0;0,1099511627776 | Zero",
        "orient2d | 0,0;1,0;0,1",
    ] {
        assert!(parse_fixture(bad).is_err(), "{bad}");
    }
}

// ---------------------------------------------------------------------------------------------
// The filter boundary of the implicit-point predicates.

/// One member of a family: what the generated filter decides (already multiplied by the sign of
/// the implicit point's weight, so it is a claim about the answer), what the predicate answers,
/// and the input as a fixture with the oracle's sign.
struct Member {
    filter: Option<Sign>,
    answer: Result<Sign, NumError>,
    fixture: Fixture,
}

/// How often each kind of input was met: the filter declining where the exact value is not zero
/// (the exact path decides, inside the bound), and the filter deciding (outside it).
#[derive(Default)]
struct Kinds {
    inside: u32,
    outside: u32,
}

/// The window searched on each side of the step where the filter starts to decide.
const WINDOW: i64 = 6;

/// Checks one family: member 0 is exactly degenerate; the step where the filter starts to decide
/// is found by doubling, then bisection; every member in a window around it must be answered with
/// the oracle's sign, and the filter, when it decides, must agree.
fn crossing(
    kinds: &RefCell<Kinds>,
    family: impl Fn(i64) -> Option<Member>,
) -> Result<(), TestCaseError> {
    let Some(zero) = family(0) else {
        return Ok(());
    };
    if zero.fixture.sign != Sign::Zero {
        return Err(TestCaseError::fail(format!(
            "member 0 is not degenerate: fixture[{}]",
            fixture_line(&zero.fixture)
        )));
    }
    // Doubling to the first deciding member, then bisection back to the first step that decides.
    let mut hi = 1i64;
    let mut lo = 0i64;
    loop {
        match family(hi) {
            None => break,
            Some(m) if m.filter.is_some() => break,
            Some(_) => {
                lo = hi;
                hi = hi.saturating_mul(2);
            }
        }
        if hi > 1 << 40 {
            break;
        }
    }
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        match family(mid) {
            Some(m) if m.filter.is_none() => lo = mid,
            _ => hi = mid,
        }
    }
    for k in (lo - WINDOW).max(0)..=hi + WINDOW {
        let Some(m) = family(k) else {
            continue;
        };
        verdict(&m.fixture, m.answer)?;
        match m.filter {
            Some(s) if s != m.fixture.sign => {
                return Err(TestCaseError::fail(format!(
                    "the filter claimed {s:?}: fixture[{}]",
                    fixture_line(&m.fixture)
                )));
            }
            Some(_) => kinds.borrow_mut().outside += 1,
            None if m.fixture.sign != Sign::Zero => kinds.borrow_mut().inside += 1,
            None => {}
        }
    }
    Ok(())
}

/// F(n), the n-th Fibonacci number; F(58) is the last below 2^40.
fn fibonacci(n: usize) -> i64 {
    let (mut a, mut b) = (0i64, 1i64);
    for _ in 0..n {
        (a, b) = (b, a + b);
    }
    a
}

/// The orders of three axes.
const ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

/// `v` with its axes in `order` and their signs flipped where `flip` says, so a family runs along
/// every axis and both ways. The determinants below keep their magnitude.
fn arranged<const D: usize>(v: [i64; D], order: [usize; D], flip: [bool; D]) -> [i64; D] {
    std::array::from_fn(|i| if flip[i] { -v[order[i]] } else { v[order[i]] })
}

/// `v` mapped into [−m, m].
fn span(v: i64, m: i64) -> i64 {
    v.rem_euclid(2 * m + 1) - m
}

/// A grid point near zero at a drawn scale, and two more draws for the family.
fn draws() -> impl Strategy<Value = (u32, i64, i64, i64)> {
    (4u32..=39, any::<i64>(), any::<i64>(), any::<i64>())
}

/// The three-dimensional family's shape: a point x0, a step e, and offsets B and C with
/// det(e, B, C) = ±1. With e the first axis, B = (b, F(n+1), F(n)) and C = (c, F(n), F(n−1)),
/// det(e, B, C) = F(n+1) F(n−1) − F(n)² = ±1 (Cassini's identity). So the plane through x0 + k·e,
/// x0 + B and x0 + C has the value ±k at x0, while B and C, and so the plane's coefficients, are
/// as large as n makes them: the cancellation that keeps a filter from deciding for small k, and
/// lets it decide once k is large enough.
fn cassini3() -> impl Strategy<Value = (Pt3, Pt3, Pt3, Pt3)> {
    (3usize..=56, draws(), 0u8..6, any::<[bool; 3]>()).prop_map(|(n, (bits, r, s, t), o, flip)| {
        let room = (COORD_MAX - fibonacci(n + 1)) / 2;
        let xm = (1i64 << bits).min(room);
        let x0 = [span(r, xm), span(s, xm), span(t, xm)];
        let b = [
            span(r.rotate_left(17), room),
            fibonacci(n + 1),
            fibonacci(n),
        ];
        let c = [
            span(s.rotate_left(29), room),
            fibonacci(n),
            fibonacci(n - 1),
        ];
        let order = ORDERS[usize::from(o)];
        (
            x0,
            arranged([1, 0, 0], order, flip),
            arranged(b, order, flip),
            arranged(c, order, flip),
        )
    })
}

/// The two-dimensional family's shape: a point x0, a step e and an offset D with
/// cross(e, D) = ±1: e the first axis and D = (m, 1). So orient2d(x0 − D, x0 + D + k·e, x0) = ±2k
/// while |D| is as large as m makes it.
fn cassini2() -> impl Strategy<Value = (Pt2, Pt2, Pt2)> {
    (draws(), any::<bool>(), any::<[bool; 2]>()).prop_map(|((bits, r, s, t), swap, flip)| {
        let m = span(t, (1i64 << bits).min(COORD_MAX / 4)).abs().max(1);
        let room = COORD_MAX - m - 1;
        let xm = (1i64 << bits).min(room);
        let x0 = [span(r, xm), span(s, xm)];
        let order = if swap { [1, 0] } else { [0, 1] };
        (
            x0,
            arranged([1, 0], order, flip),
            arranged([m, 1], order, flip),
        )
    })
}

/// Points spread over the grid at every scale: a magnitude bound drawn from 2^4 to 2^39.
fn scaled3() -> impl Strategy<Value = (Pt3, [Pt3; 8])> {
    (4u32..=39).prop_flat_map(|bits| {
        let m = 1i64 << bits;
        let pt = move || [-m..=m, -m..=m, -m..=m];
        (pt(), proptest::array::uniform8(pt()))
    })
}

fn scaled2() -> impl Strategy<Value = (Pt2, [Pt2; 6])> {
    (4u32..=39).prop_flat_map(|bits| {
        let m = 1i64 << bits;
        let pt = move || [-m..=m, -m..=m];
        (pt(), proptest::array::uniform6(pt()))
    })
}

fn moved<const D: usize>(p: [i64; D], e: [i64; D], k: i64) -> Option<[i64; D]> {
    let mut out = p;
    for (o, d) in out.iter_mut().zip(e) {
        *o = o.checked_add(d.checked_mul(k)?)?;
    }
    on_grid(&out).then_some(out)
}

fn coefficients(pl: &Plane) -> ([i128; 3], i128) {
    (pl.n(), pl.d())
}

/// Three planes through x0, from the drawn points: the three-plane point's planes, or a line's
/// plane and two more.
fn planes_through(x0: Pt3, pts: &[Pt3; 8]) -> [[Pt3; 3]; 3] {
    [
        [x0, pts[0], pts[1]],
        [x0, pts[2], pts[3]],
        [x0, pts[4], pts[5]],
    ]
}

fn made(planes: &[[Pt3; 3]]) -> Option<Vec<Plane>> {
    planes
        .iter()
        .map(|[a, b, c]| Plane::through(p3(*a), p3(*b), p3(*c)).ok())
        .collect()
}

/// `side_of_tpi` near its filter's bound: three planes through a grid point x0, and a fourth
/// through x0 + k·e, x0 + B and x0 + C ([`cassini3`]), whose value at x0 is ±k.
#[test]
fn side_of_tpi_filter_boundary_crossed_both_ways() {
    let kinds = RefCell::new(Kinds::default());
    let strategy = (cassini3(), scaled3());
    prop::run(
        "side_of_tpi_filter_boundary_crossed_both_ways",
        Seed(0x6734_0000_0000_0002),
        cases(128),
        &strategy,
        |((x0, e, bb, cc), (_, pts))| {
            let planes = planes_through(x0, &pts);
            let Some(made) = made(&planes) else {
                return Ok(());
            };
            let Ok(t) = Tpi::new(made[0], made[1], made[2]) else {
                return Ok(());
            };
            let w = {
                let n: Vec<Vec<IBig>> = made
                    .iter()
                    .map(|p| p.n().map(IBig::from).to_vec())
                    .collect();
                sign_of(&det(&n))
            };
            let (Some(b), Some(c)) = (moved(x0, bb, 1), moved(x0, cc, 1)) else {
                return Ok(());
            };
            let family = |k: i64| -> Option<Member> {
                let s = [moved(x0, e, k)?, b, c];
                let plane = Plane::through(p3(s[0]), p3(s[1]), p3(s[2])).ok()?;
                let ([ax, ay, az], ad) = coefficients(&made[0]);
                let ([bx, by, bz], bd) = coefficients(&made[1]);
                let ([cx, cy, cz], cd) = coefficients(&made[2]);
                let ([sx, sy, sz], sd) = coefficients(&plane);
                let filter = tpi_side::filter(
                    ax, ay, az, ad, bx, by, bz, bd, cx, cy, cz, cd, sx, sy, sz, sd,
                )
                .map(|v| v.mul(w));
                let points = planes
                    .iter()
                    .chain(std::iter::once(&s))
                    .flatten()
                    .map(|p| p.to_vec())
                    .collect();
                Some(Member {
                    filter,
                    answer: plane.side_of_tpi(&t),
                    fixture: Fixture {
                        predicate: "side_of_tpi".to_owned(),
                        points,
                        sign: oracle_side_of_tpi(planes, s),
                    },
                })
            };
            crossing(&kinds, family)
        },
    )
    .unwrap_or_else(|f| panic!("{f}"));
    let kinds = kinds.into_inner();
    println!(
        "{} inputs inside the bound, decided by the exact path; {} outside, by the filter",
        kinds.inside, kinds.outside
    );
    assert!(
        kinds.inside > 0 && kinds.outside > 0,
        "inside {} outside {}",
        kinds.inside,
        kinds.outside
    );
}

/// `side_of_lpi` near its filter's bound: a line through a grid point x0 on a plane, and a second
/// plane through x0 + k·e, x0 + B and x0 + C ([`cassini3`]), whose value at x0 is ±k.
#[test]
fn side_of_lpi_filter_boundary_crossed_both_ways() {
    let kinds = RefCell::new(Kinds::default());
    let strategy = (cassini3(), scaled3(), 1i64..=5);
    prop::run(
        "side_of_lpi_filter_boundary_crossed_both_ways",
        Seed(0x6734_0000_0000_0003),
        cases(128),
        &strategy,
        |((x0, e, bb, cc), (_, pts), j)| {
            // The line from p to q passes through x0: p = x0 − j·u, q = x0 + u.
            let u = pts[6].map(|v| v / 4);
            let (Some(p), Some(q)) = (moved(x0, u, -j), moved(x0, u, 1)) else {
                return Ok(());
            };
            let plane = [x0, pts[0], pts[1]];
            let Ok(first) = Plane::through(p3(plane[0]), p3(plane[1]), p3(plane[2])) else {
                return Ok(());
            };
            let Ok(l) = Lpi::new(p3(p), p3(q), first) else {
                return Ok(());
            };
            let w1 = {
                let (n1, _) = oracle_plane(plane);
                let dir = diff(q, p);
                let v = &n1[0] * RBig::from(dir[0].clone())
                    + &n1[1] * RBig::from(dir[1].clone())
                    + &n1[2] * RBig::from(dir[2].clone());
                sign_of_ratio(&v)
            };
            let (Some(b), Some(c)) = (moved(x0, bb, 1), moved(x0, cc, 1)) else {
                return Ok(());
            };
            let family = |k: i64| -> Option<Member> {
                let s = [moved(x0, e, k)?, b, c];
                let second = Plane::through(p3(s[0]), p3(s[1]), p3(s[2])).ok()?;
                let ([ax, ay, az], ad) = coefficients(&first);
                let ([bx, by, bz], bd) = coefficients(&second);
                let filter = lpi_side::filter(ax, ay, az, ad, bx, by, bz, bd, p3(p), p3(q))
                    .map(|v| v.mul(w1));
                let points = [p, q]
                    .iter()
                    .chain(plane.iter())
                    .chain(s.iter())
                    .map(|p| p.to_vec())
                    .collect();
                Some(Member {
                    filter,
                    answer: second.side_of_lpi(&l),
                    fixture: Fixture {
                        predicate: "side_of_lpi".to_owned(),
                        points,
                        sign: oracle_side_of_lpi(p, q, plane, s),
                    },
                })
            };
            crossing(&kinds, family)
        },
    )
    .unwrap_or_else(|f| panic!("{f}"));
    let kinds = kinds.into_inner();
    println!(
        "{} inputs inside the bound, decided by the exact path; {} outside, by the filter",
        kinds.inside, kinds.outside
    );
    assert!(
        kinds.inside > 0 && kinds.outside > 0,
        "inside {} outside {}",
        kinds.inside,
        kinds.outside
    );
}

/// `Ssi::orient2d` near its filter's bound: two lines crossing at a grid point x0, and the line
/// from x0 − D to x0 + D + k·e ([`cassini2`]), on which x0 lies at k = 0.
#[test]
fn ssi_orient2d_filter_boundary_crossed_both_ways() {
    let kinds = RefCell::new(Kinds::default());
    let strategy = (cassini2(), scaled2(), 1i64..=5);
    prop::run(
        "ssi_orient2d_filter_boundary_crossed_both_ways",
        Seed(0x6734_0000_0000_0004),
        cases(128),
        &strategy,
        |((x0, e, dd), (_, pts), j)| {
            let quarter = |v: Pt2| [v[0] / 4, v[1] / 4];
            let (u, v) = (quarter(pts[0]), quarter(pts[1]));
            let (Some(a), Some(b), Some(c), Some(d), Some(p), Some(q0)) = (
                moved(x0, u, -j),
                moved(x0, u, 1),
                moved(x0, v, -1),
                moved(x0, v, j),
                moved(x0, dd, -1),
                moved(x0, dd, 1),
            ) else {
                return Ok(());
            };
            let Ok(x) = Ssi::new(p2(a), p2(b), p2(c), p2(d)) else {
                return Ok(());
            };
            // The crossing's weight w = oa − ob, oa = orient2d(c, d, a) and ob = orient2d(c, d, b).
            let o = |z: Pt2| det(&[diff(d, c), diff(z, c)]);
            let w = sign_of(&(o(a) - o(b)));
            let family = |k: i64| -> Option<Member> {
                let q = moved(q0, e, k)?;
                let filter =
                    ssi_side::filter(p2(a), p2(b), p2(c), p2(d), p2(p), p2(q)).map(|s| s.mul(w));
                Some(Member {
                    filter,
                    answer: x.orient2d(p2(p), p2(q)),
                    fixture: Fixture {
                        predicate: "ssi_orient2d".to_owned(),
                        points: [a, b, c, d, p, q].iter().map(|p| p.to_vec()).collect(),
                        sign: oracle_ssi(a, b, c, d, p, q),
                    },
                })
            };
            crossing(&kinds, family)
        },
    )
    .unwrap_or_else(|f| panic!("{f}"));
    let kinds = kinds.into_inner();
    println!(
        "{} inputs inside the bound, decided by the exact path; {} outside, by the filter",
        kinds.inside, kinds.outside
    );
    assert!(
        kinds.inside > 0 && kinds.outside > 0,
        "inside {} outside {}",
        kinds.inside,
        kinds.outside
    );
}

// ---------------------------------------------------------------------------------------------
// The laws of the line-plane point and the crossing.

/// Swapping the line's two points keeps the line-plane point and its side; reversing the
/// classifying plane (its points in the other turn) flips the side; and every answer is the
/// oracle's.
#[test]
fn line_plane_point_laws() {
    let strategy = scaled3();
    prop::run(
        "line_plane_point_laws",
        Seed(0x6734_0000_0000_0005),
        cases(256),
        &strategy,
        |(_, pts)| {
            let [p, q, a, b, c, s0, s1, s2] = pts;
            let (Ok(first), Ok(second), Ok(reversed)) = (
                Plane::through(p3(a), p3(b), p3(c)),
                Plane::through(p3(s0), p3(s1), p3(s2)),
                Plane::through(p3(s0), p3(s2), p3(s1)),
            ) else {
                return Ok(());
            };
            let (Ok(l), Ok(swapped)) =
                (Lpi::new(p3(p), p3(q), first), Lpi::new(p3(q), p3(p), first))
            else {
                return Ok(());
            };
            let f = Fixture {
                predicate: "side_of_lpi".to_owned(),
                points: [p, q, a, b, c, s0, s1, s2]
                    .iter()
                    .map(|v| v.to_vec())
                    .collect(),
                sign: oracle_side_of_lpi(p, q, [a, b, c], [s0, s1, s2]),
            };
            verdict(&f, second.side_of_lpi(&l))?;
            prop_assert_eq!(second.side_of_lpi(&swapped), Ok(f.sign));
            prop_assert_eq!(reversed.side_of_lpi(&l), Ok(f.sign.negate()));
            Ok(())
        },
    )
    .unwrap_or_else(|f| panic!("{f}"));
}

/// Swapping the classifying line's points flips the side; swapping a crossing line's points, or
/// the two crossing lines, keeps it; and every answer is the oracle's.
#[test]
fn crossing_laws() {
    let strategy = scaled2();
    prop::run(
        "crossing_laws",
        Seed(0x6734_0000_0000_0006),
        cases(256),
        &strategy,
        |(_, [a, b, c, d, p, q])| {
            let Ok(x) = Ssi::new(p2(a), p2(b), p2(c), p2(d)) else {
                return Ok(());
            };
            let f = Fixture {
                predicate: "ssi_orient2d".to_owned(),
                points: [a, b, c, d, p, q].iter().map(|v| v.to_vec()).collect(),
                sign: oracle_ssi(a, b, c, d, p, q),
            };
            verdict(&f, x.orient2d(p2(p), p2(q)))?;
            prop_assert_eq!(x.orient2d(p2(q), p2(p)), Ok(f.sign.negate()));
            for other in [
                Ssi::new(p2(b), p2(a), p2(c), p2(d)),
                Ssi::new(p2(a), p2(b), p2(d), p2(c)),
                Ssi::new(p2(c), p2(d), p2(a), p2(b)),
            ] {
                prop_assert_eq!(other.and_then(|o| o.orient2d(p2(p), p2(q))), Ok(f.sign));
            }
            Ok(())
        },
    )
    .unwrap_or_else(|f| panic!("{f}"));
}

/// The perturbed predicates at every sign corner of their unperturbed predicates (coordinates
/// ±M: 256 inputs for orient2d and incircle, 4,096 for orient3d and 32,768 for insphere), where
/// points repeat and ties abound: never `Zero`, the exact sign wherever it is not `Zero`, and
/// swapping two points with their keys flips the answer. Debug builds take every twentieth corner
/// of orient3d and insphere, as the suite's case counts do.
#[test]
fn perturbed_predicates_at_the_sign_corners() {
    let m = COORD_MAX;
    let bit = |code: u32, b: u32| if code >> b & 1 == 1 { m } else { -m };
    let corner = |code: u32, i: u32| -> Pt2 { [bit(code, 2 * i), bit(code, 2 * i + 1)] };
    let corner3 = |code: u32, i: u32| -> Pt3 {
        [bit(code, 3 * i), bit(code, 3 * i + 1), bit(code, 3 * i + 2)]
    };
    let step = if cfg!(debug_assertions) { 20 } else { 1 };
    for code in (0..1u32 << 12).step_by(step) {
        let pts: [Pt3; 4] = [0, 1, 2, 3].map(|i| corner3(code, i));
        let keyed = |i: usize| (p3(pts[i]), i as u64);
        let exact = oracle_orient3d(pts[0], pts[1], pts[2], pts[3]);
        let s = perturb::orient3d(keyed(0), keyed(1), keyed(2), keyed(3)).unwrap();
        assert_ne!(s, Sign::Zero);
        if exact != Sign::Zero {
            assert_eq!(s, exact);
        }
        assert_eq!(
            perturb::orient3d(keyed(1), keyed(0), keyed(2), keyed(3)).unwrap(),
            s.negate()
        );
    }
    for code in (0..1u32 << 15).step_by(step) {
        let pts: [Pt3; 5] = [0, 1, 2, 3, 4].map(|i| corner3(code, i));
        let keyed = |i: usize| (p3(pts[i]), i as u64);
        let exact = oracle_insphere(pts[0], pts[1], pts[2], pts[3], pts[4]);
        let s = perturb::insphere(keyed(0), keyed(1), keyed(2), keyed(3), keyed(4)).unwrap();
        assert_ne!(s, Sign::Zero);
        if exact != Sign::Zero {
            assert_eq!(s, exact);
        }
        assert_eq!(
            perturb::insphere(keyed(0), keyed(1), keyed(2), keyed(4), keyed(3)).unwrap(),
            s.negate()
        );
    }
    for code in 0..1u32 << 8 {
        let pts: [Pt2; 4] = [0, 1, 2, 3].map(|i| corner(code, i));
        let keyed = |i: usize| (p2(pts[i]), i as u64);
        let exact3 = oracle_orient2d(pts[0], pts[1], pts[2]);
        let s3 = perturb::orient2d(keyed(0), keyed(1), keyed(2)).unwrap();
        assert_ne!(s3, Sign::Zero);
        if exact3 != Sign::Zero {
            assert_eq!(s3, exact3);
        }
        assert_eq!(
            perturb::orient2d(keyed(1), keyed(0), keyed(2)).unwrap(),
            s3.negate()
        );
        let exact4 = oracle_incircle(pts[0], pts[1], pts[2], pts[3]);
        let s4 = perturb::incircle(keyed(0), keyed(1), keyed(2), keyed(3)).unwrap();
        assert_ne!(s4, Sign::Zero);
        if exact4 != Sign::Zero {
            assert_eq!(s4, exact4);
        }
        assert_eq!(
            perturb::incircle(keyed(0), keyed(2), keyed(1), keyed(3)).unwrap(),
            s4.negate()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The perturbed predicates' ties, against a symbolic oracle.

/// Every permutation of 0..n with whether it is odd, in lexicographic order.
fn permutations(n: usize) -> Vec<(Vec<usize>, bool)> {
    let mut out = Vec::new();
    let mut perm: Vec<usize> = (0..n).collect();
    loop {
        let inversions = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .filter(|&(i, j)| perm[i] > perm[j])
            .count();
        out.push((perm.clone(), inversions % 2 == 1));
        let Some(i) = (0..n.saturating_sub(1))
            .rev()
            .find(|&i| perm[i] < perm[i + 1])
        else {
            return out;
        };
        let j = (i + 1..n).rev().find(|&j| perm[j] > perm[i]).unwrap();
        perm.swap(i, j);
        perm[i + 1..].reverse();
    }
}

/// The sign symbolic perturbation gives the absolute matrix whose rows are `rows`: each row is a
/// point's perturbed entries (its coordinates, then its lifted coordinate for incircle and
/// insphere) with the point's key, and the constant 1 closes it. Entry c of the point whose key
/// has rank r gains ε^(2^(r·P + c)), P entries per row, and the answer is the sign of
/// det(M + εE) as ε → 0⁺: the ordering `geom_num::perturb` states and `attack_g30.rs`'s oracle
/// evaluates with a concrete ε.
///
/// Computed by neither method: the Leibniz expansion, every term multiplied out into monomials of
/// ε in exact integers. A perturbed entry is v + ε^(2^e), so a monomial's exponent is a sum of
/// distinct powers of two, one for each perturbed entry it took; the monomials are gathered by
/// exponent, and the lowest exponent whose coefficient is not zero dominates. The numbers stay
/// the size of the products of the entries, so insphere's twenty perturbed entries, which the
/// concrete ε cannot afford, cost 120 terms of at most 16 monomials.
fn symbolic_sign(rows: &[(Vec<i128>, u64)]) -> Sign {
    let n = rows.len();
    let p = n - 1;
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| rows[i].1);
    let mut rank = vec![0; n];
    for (r, &i) in order.iter().enumerate() {
        rank[i] = r;
    }
    let mut coefficients: std::collections::BTreeMap<u64, IBig> = std::collections::BTreeMap::new();
    for (perm, odd) in permutations(n) {
        let mut terms: Vec<(u64, IBig)> = vec![(0, IBig::ONE)];
        for (row, &col) in perm.iter().enumerate() {
            if col == p {
                continue;
            }
            let value = IBig::from(rows[row].0[col]);
            let e = 1u64 << (rank[row] * p + col);
            terms = terms
                .into_iter()
                .flat_map(|(x, c)| [(x, &c * &value), (x | e, c)])
                .collect();
        }
        for (x, c) in terms {
            let slot = coefficients.entry(x).or_insert(IBig::ZERO);
            if odd {
                *slot -= c;
            } else {
                *slot += c;
            }
        }
    }
    coefficients
        .values()
        .find(|c| **c != IBig::ZERO)
        .map_or(Sign::Zero, sign_of)
}

/// The rows of `symbolic_sign` for keyed points, lifted (incircle, insphere) or not: in `i128`,
/// since a lifted coordinate reaches 3 · 2^80.
fn keyed_rows<const D: usize>(points: &[([i64; D], u64)], lifted: bool) -> Vec<(Vec<i128>, u64)> {
    points
        .iter()
        .map(|&(p, key)| {
            let mut v: Vec<i128> = p.iter().map(|&c| i128::from(c)).collect();
            if lifted {
                v.push(v.iter().map(|c| c * c).sum());
            }
            (v, key)
        })
        .collect()
}

/// N distinct keys in a random order.
fn keys<const N: usize>() -> impl Strategy<Value = [u64; N]> {
    proptest::sample::subsequence((0..64u64).collect::<Vec<_>>(), N)
        .prop_shuffle()
        .prop_map(|v| <[u64; N]>::try_from(v).unwrap())
}

/// Integer points on x² + y² = 25.
const CIRCLE: [Pt2; 12] = [
    [5, 0],
    [4, 3],
    [3, 4],
    [0, 5],
    [-3, 4],
    [-4, 3],
    [-5, 0],
    [-4, -3],
    [-3, -4],
    [0, -5],
    [3, -4],
    [4, -3],
];

/// Integer points on x² + y² + z² = 9.
const SPHERE: [Pt3; 10] = [
    [3, 0, 0],
    [-3, 0, 0],
    [0, 3, 0],
    [0, -3, 0],
    [0, 0, 3],
    [0, 0, -3],
    [1, 2, 2],
    [2, -1, 2],
    [-2, 2, 1],
    [2, 2, -1],
];

/// Points on a line through a with direction u, a few steps apart (repeats included), anywhere on
/// the grid.
fn collinear() -> impl Strategy<Value = [Pt2; 3]> {
    let reach = COORD_MAX - 8 * 1024;
    (
        proptest::array::uniform2(-reach..=reach),
        proptest::array::uniform2(-1024i64..=1024),
        proptest::array::uniform3(-8i64..=8),
    )
        .prop_map(|(a, u, s)| s.map(|s| [a[0] + s * u[0], a[1] + s * u[1]]))
}

/// Points on a plane through a spanned by u and v (repeats and collinear sets included).
fn coplanar() -> impl Strategy<Value = [Pt3; 4]> {
    let reach = COORD_MAX - 16 * 1024;
    (
        proptest::array::uniform3(-reach..=reach),
        proptest::array::uniform3(-1024i64..=1024),
        proptest::array::uniform3(-1024i64..=1024),
        proptest::array::uniform4(proptest::array::uniform2(-8i64..=8)),
    )
        .prop_map(|(a, u, v, st)| st.map(|[s, t]| [0, 1, 2].map(|i| a[i] + s * u[i] + t * v[i])))
}

/// Points on a circle of radius 5s centred anywhere on the grid (repeats included).
fn cocircular() -> impl Strategy<Value = [Pt2; 4]> {
    (1i64..=1 << 20, proptest::array::uniform4(0..12usize))
        .prop_flat_map(|(s, pick)| {
            let reach = COORD_MAX - 5 * s;
            (
                Just(s),
                Just(pick),
                proptest::array::uniform2(-reach..=reach),
            )
        })
        .prop_map(|(s, pick, o)| pick.map(|i| [0, 1].map(|c| CIRCLE[i][c] * s + o[c])))
}

/// Points on a sphere of radius 3s centred anywhere on the grid (repeats, and sets on one circle,
/// included).
fn cospherical() -> impl Strategy<Value = [Pt3; 5]> {
    (1i64..=1 << 20, proptest::array::uniform5(0..10usize))
        .prop_flat_map(|(s, pick)| {
            let reach = COORD_MAX - 3 * s;
            (
                Just(s),
                Just(pick),
                proptest::array::uniform3(-reach..=reach),
            )
        })
        .prop_map(|(s, pick, o)| pick.map(|i| [0, 1, 2].map(|c| SPHERE[i][c] * s + o[c])))
}

/// One keyed tie: the oracle's sign (negated where the predicate's sign is the absolute
/// determinant's opposite) is never `Zero`, the predicate gives it, and the same points with the
/// first one moved a step off the tie get the exact sign from both, which holds the convention.
fn tie<const D: usize, const N: usize>(
    points: [[i64; D]; N],
    keys: [u64; N],
    lifted: bool,
    negated: bool,
    exact: impl Fn(&[[i64; D]; N]) -> Sign,
    perturbed: impl Fn(&[([i64; D], u64); N]) -> Result<Sign, NumError>,
) -> Result<(), TestCaseError> {
    let oracle = |pts: &[([i64; D], u64); N]| {
        let s = symbolic_sign(&keyed_rows(pts, lifted));
        if negated { s.negate() } else { s }
    };
    let keyed: [([i64; D], u64); N] = core::array::from_fn(|i| (points[i], keys[i]));
    let want = oracle(&keyed);
    prop_assert_ne!(want, Sign::Zero, "{:?}", keyed);
    prop_assert_eq!(perturbed(&keyed), Ok(want), "{:?}", keyed);
    let mut moved = points;
    moved[0][0] += if moved[0][0] < COORD_MAX { 1 } else { -1 };
    let exact = exact(&moved);
    if exact != Sign::Zero {
        let keyed: [([i64; D], u64); N] = core::array::from_fn(|i| (moved[i], keys[i]));
        prop_assert_eq!(oracle(&keyed), exact, "{:?}", keyed);
        prop_assert_eq!(perturbed(&keyed), Ok(exact), "{:?}", keyed);
    }
    Ok(())
}

/// The four perturbed predicates on ties (collinear, cocircular, coplanar and cospherical points
/// anywhere on the grid, repeated points with distinct keys among them), against
/// `symbolic_sign`, an oracle that shares no method with the module or with `attack_g30.rs`'s
/// concrete ε; insphere, which that oracle cannot afford, is held to an oracle here for the first
/// time (piece G43).
#[test]
fn perturbed_ties_match_a_symbolic_oracle() {
    let k2 = |p: &[(Pt2, u64)], i: usize| (p2(p[i].0), p[i].1);
    let k3 = |p: &[(Pt3, u64)], i: usize| (p3(p[i].0), p[i].1);
    prop::run(
        "perturbed_ties_match_a_symbolic_oracle (orient2d)",
        Seed(0x6743_0000_0000_0001),
        cases(512),
        &(collinear(), keys::<3>()),
        |(pts, keys)| {
            tie(
                pts,
                keys,
                false,
                false,
                |p| oracle_orient2d(p[0], p[1], p[2]),
                |p| perturb::orient2d(k2(p, 0), k2(p, 1), k2(p, 2)),
            )
        },
    )
    .unwrap_or_else(|f| panic!("{f}"));
    prop::run(
        "perturbed_ties_match_a_symbolic_oracle (incircle)",
        Seed(0x6743_0000_0000_0002),
        cases(512),
        &(cocircular(), keys::<4>()),
        |(pts, keys)| {
            tie(
                pts,
                keys,
                true,
                false,
                |p| oracle_incircle(p[0], p[1], p[2], p[3]),
                |p| perturb::incircle(k2(p, 0), k2(p, 1), k2(p, 2), k2(p, 3)),
            )
        },
    )
    .unwrap_or_else(|f| panic!("{f}"));
    prop::run(
        "perturbed_ties_match_a_symbolic_oracle (orient3d)",
        Seed(0x6743_0000_0000_0003),
        cases(512),
        &(coplanar(), keys::<4>()),
        |(pts, keys)| {
            tie(
                pts,
                keys,
                false,
                true,
                |p| oracle_orient3d(p[0], p[1], p[2], p[3]),
                |p| perturb::orient3d(k3(p, 0), k3(p, 1), k3(p, 2), k3(p, 3)),
            )
        },
    )
    .unwrap_or_else(|f| panic!("{f}"));
    prop::run(
        "perturbed_ties_match_a_symbolic_oracle (insphere)",
        Seed(0x6743_0000_0000_0004),
        cases(512),
        &(cospherical(), keys::<5>()),
        |(pts, keys)| {
            tie(
                pts,
                keys,
                true,
                true,
                |p| oracle_insphere(p[0], p[1], p[2], p[3], p[4]),
                |p| perturb::insphere(k3(p, 0), k3(p, 1), k3(p, 2), k3(p, 3), k3(p, 4)),
            )
        },
    )
    .unwrap_or_else(|f| panic!("{f}"));
}

// ---------------------------------------------------------------------------------------------
// The matrix.

/// The classes of chapter 28.6.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// Random inputs across the grid, at every scale.
    Random,
    /// Exactly degenerate inputs, then moved one step either way.
    OneStep,
    /// Inputs just inside and just outside the filter's bound.
    Boundary,
    /// The corner sets: every sign combination of ±(2^40 − 1), or the configurations that maximise
    /// each term of the bit budget.
    Corners,
    /// The algebraic laws, and for the perturbed predicates the perturbation laws.
    Laws,
}

/// Where a class is checked: a test file of this crate and a test function in it, or the reason
/// the class does not apply.
enum Where {
    Test(&'static str, &'static str),
    NotApplicable(&'static str),
}

/// A public predicate: its name, how a test names it in code (a path such as
/// `predicate::orient2d`, matched by its last segments, or a method such as `.side`), and where
/// each class is checked.
struct Row {
    predicate: &'static str,
    call: &'static str,
    classes: [(Class, Where); 5],
}

/// Every test file the matrix names, read when the test is built.
const FILES: [(&str, &str); 7] = [
    ("attack_g187.rs", include_str!("attack_g187.rs")),
    ("attack_g28.rs", include_str!("attack_g28.rs")),
    ("attack_g29.rs", include_str!("attack_g29.rs")),
    ("attack_g30.rs", include_str!("attack_g30.rs")),
    ("plane.rs", include_str!("plane.rs")),
    ("attack_plane.rs", include_str!("attack_plane.rs")),
    ("verification.rs", include_str!("verification.rs")),
];

/// The sources whose public predicates the matrix must list.
const SOURCES: [(&str, &str); 5] = [
    ("carrier", include_str!("../src/carrier.rs")),
    ("predicate", include_str!("../src/predicate.rs")),
    ("plane", include_str!("../src/plane.rs")),
    ("crossing", include_str!("../src/crossing.rs")),
    ("perturb", include_str!("../src/perturb.rs")),
];

fn explicit(predicate: &'static str, call: &'static str) -> Row {
    use Class::*;
    use Where::Test;
    Row {
        predicate,
        call,
        classes: [
            (
                Random,
                Test(
                    "attack_g28.rs",
                    "random_inputs_at_every_scale_against_the_oracle_and_robust",
                ),
            ),
            (
                OneStep,
                Test(
                    "attack_g28.rs",
                    "one_step_from_every_degeneracy_against_the_oracle_and_robust",
                ),
            ),
            (
                Boundary,
                Test(
                    "attack_g28.rs",
                    "every_filter_boundary_crossed_both_ways_against_the_oracle_and_robust",
                ),
            ),
            (
                Corners,
                Test(
                    "attack_g28.rs",
                    "every_sign_corner_against_the_oracle_robust_and_the_written_budget",
                ),
            ),
            (
                Laws,
                Test(
                    "attack_g28.rs",
                    "antisymmetry_permutation_and_translation_laws_on_every_input_class",
                ),
            ),
        ],
    }
}

fn implicit(
    predicate: &'static str,
    call: &'static str,
    boundary: &'static str,
    laws: (&'static str, &'static str),
) -> Row {
    use Class::*;
    use Where::Test;
    Row {
        predicate,
        call,
        classes: [
            (
                Random,
                Test(
                    "attack_g29.rs",
                    "random_inputs_at_every_scale_against_the_oracle",
                ),
            ),
            (
                OneStep,
                Test(
                    "attack_g29.rs",
                    "one_step_from_degenerate_is_decided_exactly",
                ),
            ),
            (Boundary, Test("verification.rs", boundary)),
            (
                Corners,
                Test(
                    "attack_g29.rs",
                    "maximal_term_corners_never_fail_and_match_the_oracle",
                ),
            ),
            (Laws, Test(laws.0, laws.1)),
        ],
    }
}

/// A predicate of `carrier`: every class in `attack_g187.rs`, by one test each but the boundary.
fn on_carrier(
    predicate: &'static str,
    call: &'static str,
    one_step: &'static str,
    boundary: &'static str,
) -> Row {
    use Class::*;
    use Where::Test;
    Row {
        predicate,
        call,
        classes: [
            (
                Random,
                Test(
                    "attack_g187.rs",
                    "random_inputs_at_every_scale_match_the_oracle",
                ),
            ),
            (OneStep, Test("attack_g187.rs", one_step)),
            (Boundary, Test("attack_g187.rs", boundary)),
            (
                Corners,
                Test(
                    "attack_g187.rs",
                    "the_extreme_values_reach_every_width_and_every_step_fits",
                ),
            ),
            (
                Laws,
                Test(
                    "attack_g187.rs",
                    "order_and_turn_change_sign_as_they_should",
                ),
            ),
        ],
    }
}

fn perturbed(
    predicate: &'static str,
    call: &'static str,
    ties: (&'static str, &'static str),
    laws: (&'static str, &'static str),
) -> Row {
    use Class::*;
    use Where::{NotApplicable, Test};
    Row {
        predicate,
        call,
        classes: [
            (
                Random,
                Test(
                    "attack_g30.rs",
                    "the_exact_sign_wins_wherever_it_is_not_zero",
                ),
            ),
            (OneStep, Test(ties.0, ties.1)),
            (
                Boundary,
                NotApplicable(
                    "it calls the unperturbed predicate first, whose filter boundary attack_g28 crosses, and breaks only exact ties, which no filter decides",
                ),
            ),
            (
                Corners,
                Test(
                    "verification.rs",
                    "perturbed_predicates_at_the_sign_corners",
                ),
            ),
            (Laws, Test(laws.0, laws.1)),
        ],
    }
}

fn matrix() -> Vec<Row> {
    use Class::*;
    use Where::{NotApplicable, Test};
    vec![
        explicit("predicate::orient2d", "predicate::orient2d"),
        explicit("predicate::orient3d", "predicate::orient3d"),
        explicit("predicate::incircle", "predicate::incircle"),
        explicit("predicate::insphere", "predicate::insphere"),
        Row {
            predicate: "plane::Plane::side",
            call: ".side",
            classes: [
                (Random, Test("plane.rs", "plane_matches_oracle")),
                (OneStep, Test("plane.rs", "points_almost_on_a_plane")),
                (
                    Boundary,
                    NotApplicable(
                        "it has no filter: a `const fn` computing n · p + d exactly in i128",
                    ),
                ),
                (
                    Corners,
                    Test(
                        "attack_plane.rs",
                        "plane_exhaustive_frame_corners_and_centres",
                    ),
                ),
                (Laws, Test("plane.rs", "plane_laws")),
            ],
        },
        implicit(
            "plane::Plane::side_of_tpi",
            ".side_of_tpi",
            "side_of_tpi_filter_boundary_crossed_both_ways",
            ("plane.rs", "tpi_permutation_laws"),
        ),
        implicit(
            "plane::Plane::side_of_lpi",
            ".side_of_lpi",
            "side_of_lpi_filter_boundary_crossed_both_ways",
            ("verification.rs", "line_plane_point_laws"),
        ),
        implicit(
            "crossing::Ssi::orient2d",
            ".orient2d",
            "ssi_orient2d_filter_boundary_crossed_both_ways",
            ("verification.rs", "crossing_laws"),
        ),
        on_carrier(
            "carrier::Carrier::side",
            ".side",
            "one_step_from_a_carrier_is_decided_by_the_exact_path",
            "side_filter_boundary_crossed_both_ways",
        ),
        on_carrier(
            "carrier::Carrier::order",
            ".order",
            "one_step_along_a_carrier_is_decided_by_the_exact_path",
            "order_filter_boundary_crossed_both_ways",
        ),
        on_carrier(
            "carrier::Rational2::cmp_x",
            ".cmp_x",
            "one_step_along_a_carrier_is_decided_by_the_exact_path",
            "cmp_filter_boundary_crossed_both_ways",
        ),
        on_carrier(
            "carrier::Rational2::cmp_y",
            ".cmp_y",
            "one_step_along_a_carrier_is_decided_by_the_exact_path",
            "cmp_filter_boundary_crossed_both_ways",
        ),
        on_carrier(
            "carrier::Rational2::orient",
            "Rational2::orient",
            "one_step_along_a_carrier_is_decided_by_the_exact_path",
            "orient_filter_boundary_crossed_both_ways",
        ),
        perturbed(
            "perturb::orient2d",
            "perturb::orient2d",
            (
                "attack_g30.rs",
                "orient2d_ties_match_the_oracle_and_obey_the_laws",
            ),
            (
                "attack_g30.rs",
                "orient2d_ties_match_the_oracle_and_obey_the_laws",
            ),
        ),
        perturbed(
            "perturb::orient3d",
            "perturb::orient3d",
            (
                "attack_g30.rs",
                "incircle_and_orient3d_ties_match_the_oracle",
            ),
            (
                "attack_g30.rs",
                "every_tie_obeys_the_laws_and_is_never_zero",
            ),
        ),
        perturbed(
            "perturb::incircle",
            "perturb::incircle",
            (
                "attack_g30.rs",
                "incircle_and_orient3d_ties_match_the_oracle",
            ),
            (
                "attack_g30.rs",
                "every_tie_obeys_the_laws_and_is_never_zero",
            ),
        ),
        perturbed(
            "perturb::insphere",
            "perturb::insphere",
            ("verification.rs", "perturbed_ties_match_a_symbolic_oracle"),
            (
                "attack_g30.rs",
                "every_tie_obeys_the_laws_and_is_never_zero",
            ),
        ),
    ]
}

/// Whether what a test reaches names `call`: a method by its name, a path by its last segments
/// (`geom_num::predicate::orient2d` names `predicate::orient2d`).
fn names(reach: &std::collections::BTreeSet<String>, call: &str) -> bool {
    if call.starts_with('.') {
        return reach.contains(call);
    }
    reach
        .iter()
        .any(|p| p == call || p.ends_with(&format!("::{call}")))
}

/// The public predicates of a source: every `pub fn` (or `pub const fn`) returning
/// `Result<Sign, NumError>`, with its `impl` type when it has one.
fn public_predicates(module: &str, source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut ty: Option<String> = None;
    let mut pending: Option<String> = None;
    for line in source.lines() {
        if line.starts_with("#[cfg(test)]") {
            break;
        }
        if let Some(rest) = line.strip_prefix("impl ") {
            ty = rest
                .split_whitespace()
                .next()
                .map(|t| t.trim_end_matches('{').to_owned());
        } else if line == "}" {
            ty = None;
        }
        let t = line.trim_start();
        let name = t
            .strip_prefix("pub fn ")
            .or_else(|| t.strip_prefix("pub const fn "))
            .and_then(|r| r.split(['(', '<']).next());
        if let Some(n) = name {
            pending = Some(match &ty {
                Some(ty) => format!("{module}::{ty}::{n}"),
                None => format!("{module}::{n}"),
            });
        }
        if let Some(p) = &pending {
            if t.contains("-> Result<Sign, NumError>") {
                found.push(p.clone());
                pending = None;
            } else if t.ends_with('{') || t.ends_with(';') {
                pending = None;
            }
        }
    }
    found
}

#[test]
fn every_predicate_is_verified_in_every_class() {
    let rows = matrix();
    let listed: Vec<&str> = rows.iter().map(|r| r.predicate).collect();
    let mut public: Vec<String> = SOURCES
        .iter()
        .flat_map(|(m, s)| public_predicates(m, s))
        .collect();
    public.sort();
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    assert_eq!(
        public, sorted,
        "the matrix must list exactly geom-num's public predicates"
    );
    for row in &rows {
        let classes: Vec<Class> = row.classes.iter().map(|(c, _)| *c).collect();
        assert_eq!(
            classes,
            [
                Class::Random,
                Class::OneStep,
                Class::Boundary,
                Class::Corners,
                Class::Laws
            ],
            "{}",
            row.predicate
        );
        for (class, place) in &row.classes {
            match place {
                Where::Test(file, function) => {
                    let text = FILES
                        .iter()
                        .find(|(f, _)| f == file)
                        .map_or_else(|| panic!("{}: no file {file}", row.predicate), |(_, t)| *t);
                    let reach = references(text, function)
                        .unwrap_or_else(|e| panic!("{file} is not Rust: {e}"))
                        .unwrap_or_else(|| {
                            panic!(
                                "{} {class:?}: {file} has no test `{function}`",
                                row.predicate
                            )
                        });
                    assert!(
                        names(&reach, row.call),
                        "{} {class:?}: `{function}` in {file} never reaches `{}`",
                        row.predicate,
                        row.call
                    );
                }
                Where::NotApplicable(why) => assert!(!why.is_empty(), "{}", row.predicate),
            }
        }
    }
    // Only a predicate with no filter, or one that defers to another predicate's filter, may skip
    // the boundary; nothing skips any other class.
    for row in &rows {
        for (class, place) in &row.classes {
            if matches!(place, Where::NotApplicable(_)) {
                assert_eq!(*class, Class::Boundary, "{}", row.predicate);
            }
        }
    }
}

/// The matrix's reach check refuses a cited test that never calls its predicate, even in a file
/// that spells the predicate's name elsewhere, as this file's matrix does (the gap gate 1 found:
/// a file-wide search counted the matrix's own strings).
#[test]
fn the_matrix_refuses_a_test_that_does_not_reach_its_predicate() {
    let this = FILES
        .iter()
        .find(|(f, _)| *f == "verification.rs")
        .unwrap()
        .1;
    assert!(this.contains("\"perturb::insphere\""));
    let fixtures = references(this, "every_fixture_replays").unwrap().unwrap();
    assert!(!names(&fixtures, "perturb::insphere"));
    let corners = references(this, "perturbed_predicates_at_the_sign_corners")
        .unwrap()
        .unwrap();
    assert!(names(&corners, "perturb::insphere"));
    assert!(!names(&corners, ".side"));
}

#[test]
fn the_matrix_reader_finds_methods_and_free_functions() {
    let source = "\
impl Plane {
    pub const fn side(&self, p: P3) -> Result<Sign, NumError> {
    }
    pub fn n(&self) -> [i128; 3] {
    }
}
pub fn orient2d(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
}
pub fn insphere(
    a: P3,
) -> Result<Sign, NumError> {
}
fn hidden(a: P2) -> Result<Sign, NumError> {
}
";
    assert_eq!(
        public_predicates("m", source),
        ["m::Plane::side", "m::orient2d", "m::insphere"]
    );
}
