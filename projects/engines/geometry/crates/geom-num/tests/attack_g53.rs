//! Attacks on piece G53: the double-double balls and the certified root-isolation engine.
//!
//! What this file proves: on seeded polynomial systems in one to four unknowns (10,000 per
//! dimension in release, the board's count), whose zeros are known exactly as rationals, the
//! engine loses no zero (each lies in a returned enclosure or flagged region), every returned
//! enclosure holds exactly one exact zero, which is the only one in its proof box, no zero is
//! returned twice, every enclosure meets its target and every flagged region is below the floor
//! and clear of every proof box, and at least a stated share of the zeros is proven. The balls hold
//! every exact result of every operation on seeded operands (against exact rationals), commute,
//! have the double-double results as centres, hold and are held by their intervals, refuse what
//! they cannot hold, survive the cancellation that defeats intervals, and carry derivatives. The
//! engine proves zeros on split lines once each, by either operator, and a zero proven a second
//! time is returned once; cuts or drops the flagged regions a later proof box meets; splits a box
//! whose image is exactly one step; flags a zero on the domain's
//! boundary, zeros closer than the floor, multiple zeros, a dependent system and a zero without a
//! derivative, never proving them; moves to double-double for a large evaluation error, for
//! undecided boxes at the floor and for a stalled refinement, and falls back to doubles where balls
//! fail; refines a zero for as many steps as its contraction bound predicts, and no more than eight
//! beyond; and returns an error at every cap: the work and logical-memory budgets, cancellation, a
//! refused allocation, a target that is not positive, an unreachable target, a refinement slower
//! than its cap, an evaluation error too large even in balls, a box above the floor that cannot be
//! split, residuals that fail, and a proof that does not close because the residuals are not a
//! function.
//!
//! Seeded inputs come from `geom_test`'s `SplitMix64`, from the seed each test prints;
//! `SPAREKIT_SEED` replaces it.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::manual_range_contains,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::unreadable_literal,
    clippy::assert_is_empty,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::cell::Cell;
use std::collections::BTreeMap;

use geom_num::NumError;
use geom_num::ball::Ball;
use geom_num::dd::DD;
use geom_num::dual::{Dual, jacobian};
use geom_num::interval::Interval;
use geom_num::isolate::{IsolateError, Isolation, Options, PHASE, Reason, System, isolate};
use geom_num::newton::{Operator, Precision, Residuals};
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_run::{Budget, BudgetKind, CancelToken, Exceeded, Usage, WorkMeter};
use geom_test::SplitMix64;
use geom_test::seed::{Seed, resolve};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero as _};

fn r(x: f64) -> Real {
    Real::new(x).unwrap()
}

fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

fn c<S: Scalar>(v: f64) -> S {
    S::constant(r(v))
}

/// The integer dot product of `a` and `p`, in order.
fn dot<const N: usize>(a: &[i64; N], p: &[i64; N]) -> i64 {
    let mut s = 0;
    for (x, y) in a.iter().zip(p) {
        s += x * y;
    }
    s
}

/// 2^k exactly, for k in the normal range, from its bits (no platform maths).
fn pow2(k: i32) -> f64 {
    f64::from_bits(((1023 + k) as u64) << 52)
}

fn exact(x: f64) -> BigRational {
    BigRational::from_float(x).unwrap()
}

fn int(v: i64) -> BigRational {
    BigRational::from_integer(BigInt::from(v))
}

/// A uniform integer in [lo, hi].
fn between(rng: &mut SplitMix64, lo: i64, hi: i64) -> i64 {
    lo + (rng.next_u64() % (hi - lo + 1) as u64) as i64
}

/// An affine form `a·x − b` with integer coefficients.
#[derive(Clone, Copy, Debug)]
struct Form<const N: usize> {
    a: [i64; N],
    b: i64,
}

impl<const N: usize> Form<N> {
    fn eval<S: Scalar>(&self, x: [S; N]) -> Result<S, NumError> {
        let mut acc = c::<S>(-(self.b as f64));
        for (aj, xj) in self.a.iter().zip(x) {
            acc = c::<S>(*aj as f64).mul_add(xj, acc)?;
        }
        Ok(acc)
    }
}

/// A system whose equation `i` is the product of the affine forms `forms[i]`, evaluated as that
/// product or as the expanded polynomial (integer coefficients, exact in doubles).
#[derive(Clone, Debug)]
struct Products<const N: usize> {
    forms: Vec<Vec<Form<N>>>,
    /// Grid steps per unit of the unknowns: the position is the unknowns times this.
    scale: f64,
    /// Equation `i` expanded: (coefficient, exponent of each unknown).
    expanded: Option<Vec<Vec<(i64, [u32; N])>>>,
}

impl<const N: usize> Products<N> {
    fn expand(&mut self) {
        let mut all = Vec::new();
        for forms in &self.forms {
            let mut poly: BTreeMap<[u32; N], i64> = BTreeMap::new();
            poly.insert([0; N], 1);
            for f in forms {
                let mut next: BTreeMap<[u32; N], i64> = BTreeMap::new();
                for (e, coef) in &poly {
                    *next.entry(*e).or_insert(0) -= coef * f.b;
                    for j in 0..N {
                        let mut e2 = *e;
                        e2[j] += 1;
                        *next.entry(e2).or_insert(0) += coef * f.a[j];
                    }
                }
                poly = next;
            }
            all.push(
                poly.into_iter()
                    .filter(|(_, v)| *v != 0)
                    .map(|(e, v)| (v, e))
                    .collect(),
            );
        }
        self.expanded = Some(all);
    }

    /// Every zero of the system, exactly: one form of each equation chosen, the linear system
    /// solved. `None` when some choice is singular, two choices meet in one point, or a zero lies on
    /// two forms of one equation (a zero where that equation's gradient vanishes).
    fn zeros(&self) -> Option<Vec<[BigRational; N]>> {
        let mut out: Vec<[BigRational; N]> = Vec::new();
        let counts: Vec<usize> = self.forms.iter().map(Vec::len).collect();
        let mut total = 1;
        for count in &counts {
            total *= count;
        }
        for mut k in 0..total {
            let mut rows = Vec::new();
            for i in 0..N {
                rows.push(self.forms[i][k % counts[i]]);
                k /= counts[i];
            }
            let x = solve(&rows)?;
            if out.contains(&x) {
                return None;
            }
            for forms in &self.forms {
                let on = forms.iter().filter(|f| value(f, &x).is_zero()).count();
                if on != 1 {
                    return None;
                }
            }
            out.push(x);
        }
        Some(out)
    }
}

fn value<const N: usize>(f: &Form<N>, x: &[BigRational; N]) -> BigRational {
    let mut acc = -int(f.b);
    for j in 0..N {
        acc += int(f.a[j]) * &x[j];
    }
    acc
}

/// The solution of `a_i · x = b_i`, exactly, or `None` when the matrix is singular.
fn solve<const N: usize>(rows: &[Form<N>]) -> Option<[BigRational; N]> {
    let mut m: Vec<Vec<BigRational>> = rows
        .iter()
        .map(|f| f.a.iter().map(|&v| int(v)).chain([int(f.b)]).collect())
        .collect();
    for col in 0..N {
        let p = (col..N).find(|&r| !m[r][col].is_zero())?;
        m.swap(col, p);
        for r in 0..N {
            if r != col && !m[r][col].is_zero() {
                let f = &m[r][col] / &m[col][col];
                for k in col..=N {
                    let v = &m[col][k] * &f;
                    m[r][k] -= v;
                }
            }
        }
    }
    Some(core::array::from_fn(|i| &m[i][N] / &m[i][i]))
}

impl<const N: usize> Residuals<N> for Products<N> {
    fn residuals<S: Scalar>(&self, x: [S; N]) -> Result<[S; N], NumError> {
        let mut out = [c::<S>(0.0); N];
        for (i, o) in out.iter_mut().enumerate() {
            if let Some(expanded) = &self.expanded {
                let mut acc = c::<S>(0.0);
                for (coef, e) in &expanded[i] {
                    let mut term = c::<S>(*coef as f64);
                    for (j, xj) in x.iter().enumerate() {
                        for _ in 0..e[j] {
                            term = term.mul(*xj)?;
                        }
                    }
                    acc = acc.add(term)?;
                }
                *o = acc;
            } else {
                let mut acc = c::<S>(1.0);
                for f in &self.forms[i] {
                    acc = acc.mul(f.eval(x)?)?;
                }
                *o = acc;
            }
        }
        Ok(out)
    }
}

impl<const N: usize> System<N, N> for Products<N> {
    fn position<S: Scalar>(&self, x: [S; N]) -> Result<[S; N], NumError> {
        let mut out = x;
        for o in &mut out {
            *o = c::<S>(self.scale).mul(*o)?;
        }
        Ok(out)
    }
}

/// How many forms each equation has, by dimension: 2^N zeros in one and two unknowns, fewer above.
fn degrees(n: usize) -> &'static [usize] {
    match n {
        1 => &[3],
        2 => &[2, 2],
        3 => &[2, 2, 1],
        _ => &[2, 1, 1, 1],
    }
}

/// A seeded system with simple zeros: each form passes through a random grid point with a random
/// small normal.
fn random_system<const N: usize>(rng: &mut SplitMix64) -> (Products<N>, Vec<[BigRational; N]>) {
    loop {
        let mut forms = Vec::new();
        for &d in degrees(N) {
            let mut eq = Vec::new();
            for _ in 0..d {
                let mut a = [0i64; N];
                while a.iter().all(|v| *v == 0) {
                    for v in &mut a {
                        *v = between(rng, -3, 3);
                    }
                }
                let p: [i64; N] = core::array::from_fn(|_| between(rng, -12, 12));
                let b = dot(&a, &p) + between(rng, -2, 2);
                eq.push(Form { a, b });
            }
            forms.push(eq);
        }
        let mut system = Products {
            forms,
            scale: SCALE,
            expanded: None,
        };
        if let Some(zeros) = system.zeros() {
            if rng.next_u64() % 2 == 0 {
                system.expand();
            }
            return (system, zeros);
        }
    }
}

fn inside_box<const N: usize>(x: &[BigRational; N], b: &[Interval; N]) -> bool {
    x.iter()
        .zip(b)
        .all(|(xi, bi)| exact(bi.lo()) <= *xi && *xi <= exact(bi.hi()))
}

/// A budget no isolation in these tests comes near (the largest, on the board in four unknowns,
/// charges about 150,000 work units and 6 MB of logical memory), so that an engine that loops
/// stops with `Exceeded` within seconds instead of running on.
const ROOMY: Budget = Budget::new(2_000_000, 64 << 20);

/// Grid steps per unit of the seeded systems' unknowns: their zeros are rationals a few units
/// apart, and the floor is 1/64 of a unit, as a parameter's floor is a small part of its range.
const SCALE: f64 = 64.0;

/// Checks an isolation against the exact zeros: completeness, soundness, distinctness, target
/// (`target` and the floor are in units of the unknowns).
fn check<const N: usize>(
    found: &Isolation<N>,
    zeros: &[[BigRational; N]],
    domain: &[Interval; N],
    target: f64,
    floor: f64,
) {
    for z in zeros.iter().filter(|z| inside_box(z, domain)) {
        let holders = found
            .zeros
            .iter()
            .filter(|f| inside_box(z, &f.enclosure))
            .count();
        let flagged = found.flagged.iter().any(|f| inside_box(z, &f.region));
        assert!(holders <= 1, "a zero returned twice");
        assert!(holders == 1 || flagged, "a zero lost: {z:?}");
    }
    for f in &found.zeros {
        let in_proof: Vec<_> = zeros.iter().filter(|z| inside_box(z, &f.proof)).collect();
        assert_eq!(
            in_proof.len(),
            1,
            "a proof box with {} zeros",
            in_proof.len()
        );
        assert!(
            inside_box(in_proof[0], &f.enclosure),
            "an enclosure without its zero"
        );
        assert!(inside_box_iv(&f.enclosure, &f.proof));
        assert!(inside_box_iv(&f.proof, domain));
        for e in &f.enclosure {
            assert!(
                exact(e.hi()) - exact(e.lo()) <= exact(target),
                "an enclosure above target"
            );
        }
    }
    for g in &found.flagged {
        for e in &g.region {
            assert!(
                exact(e.hi()) - exact(e.lo()) < exact(floor),
                "a flagged region above the floor"
            );
        }
        for f in &found.zeros {
            assert!(
                !overlaps(&g.region, &f.proof),
                "a flagged region inside a proof box"
            );
        }
    }
}

fn overlaps<const N: usize>(a: &[Interval; N], b: &[Interval; N]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| x.lo() < y.hi() && y.lo() < x.hi())
}

fn inside_box_iv<const N: usize>(a: &[Interval; N], b: &[Interval; N]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| x.lo() >= y.lo() && x.hi() <= y.hi())
}

/// The systems per dimension: 10,000 in release, as the board asks; fewer in debug, where the
/// same code runs about thirty times slower.
const SYSTEMS: usize = if cfg!(debug_assertions) { 300 } else { 10_000 };

#[derive(Default, Debug)]
struct Tally {
    systems: usize,
    zeros: usize,
    proven: usize,
    flagged_regions: usize,
    work: u128,
}

/// The board's test in `N` unknowns: no zero lost and every enclosure holding its zero, on
/// `SYSTEMS` seeded polynomial systems against exact rationals; and at least `min_proven` of the
/// zeros in the domain proven, a floor on the engine's strength measured on this corpus (the
/// others lie in flagged regions: zeros whose enclosures in doubles and double-double balls are
/// still too wide to decide at the resolution floor, as for expanded cubics with close zeros).
fn board<const N: usize>(name: &str, seed: u64, min_proven: f64) {
    let seed = resolve(name, Seed(seed)).unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut tally = Tally::default();
    for s in 0..SYSTEMS {
        let (system, zeros) = random_system::<N>(&mut rng);
        let (domain, found, meter) = isolate_case(s, &system, &zeros);
        tally.systems += 1;
        tally.zeros += zeros.iter().filter(|z| inside_box(z, &domain)).count();
        tally.proven += found.zeros.len();
        tally.flagged_regions += found.flagged.len();
        tally.work += meter.used().work;
    }
    println!("{N} unknowns: {tally:?}");
    let proven = tally.proven as f64 / tally.zeros as f64;
    assert!(proven >= min_proven, "{proven} of the zeros proven");
}

/// Isolates case `s` of a board's corpus, `system` with its exact `zeros`, and checks it; returns
/// the case's domain, what was found and the meter. Half the domains split on dyadic lines through
/// the grid points the forms pass through, so zeros land on split lines; the others are offset off
/// them. A third of the cases use Hansen-Sengupta.
fn isolate_case<const N: usize>(
    s: usize,
    system: &Products<N>,
    zeros: &[[BigRational; N]],
) -> ([Interval; N], Isolation<N>, WorkMeter<'static>) {
    let shift = if s % 2 == 0 { 0.0 } else { 0.3 };
    let domain: [Interval; N] = core::array::from_fn(|_| iv(-16.0 + shift, 16.0 + shift));
    let operator = if s % 3 == 0 {
        Operator::HansenSengupta
    } else {
        Operator::Krawczyk
    };
    let options = Options {
        operator,
        target: Options::DEFAULT.target,
    };
    let mut meter = WorkMeter::new(ROOMY);
    let found = isolate(system, domain, options, &mut meter)
        .unwrap_or_else(|e| panic!("system {s} in {N} unknowns: {e:?}\n{system:?}"));
    check(&found, zeros, &domain, 1.0 / SCALE, 1.0 / SCALE);
    (domain, found, meter)
}

/// Case `s` of the board's corpus in `N` unknowns from its default `seed` (never the override).
fn board_case<const N: usize>(seed: u64, s: usize) -> Isolation<N> {
    let mut rng = SplitMix64::new(Seed(seed));
    let mut drawn = random_system::<N>(&mut rng);
    for _ in 0..s {
        drawn = random_system::<N>(&mut rng);
    }
    let (system, zeros) = drawn;
    isolate_case(s, &system, &zeros).1
}

#[test]
fn no_zero_is_lost_in_one_unknown() {
    board::<1>(
        "attack_g53 systems in one unknown",
        0x6753_0000_0001_0001,
        0.995,
    );
}

#[test]
fn no_zero_is_lost_in_two_unknowns() {
    board::<2>(
        "attack_g53 systems in two unknowns",
        0x6753_0000_0001_0002,
        0.985,
    );
}

#[test]
fn no_zero_is_lost_in_three_unknowns() {
    board::<3>(
        "attack_g53 systems in three unknowns",
        0x6753_0000_0001_0003,
        0.93,
    );
}

#[test]
fn no_zero_is_lost_in_four_unknowns() {
    board::<4>(
        "attack_g53 systems in four unknowns",
        0x6753_0000_0001_0004,
        0.88,
    );
}

// The board's cases below take paths the first 300 systems of each dimension (the debug run) never
// take, found by counting, in an instrumented build, where the release run's 10,000 systems per
// dimension take each one (system 874 of two unknowns by running the release board with the
// duplicate check needing both containments). Each is replayed from the board's default seed and
// checked as the board checks it.

/// A zero proven a second time, from an inflated box reaching into the proof box of the zero found
/// first, is returned once (systems 336, 388 and 874 of two unknowns): each time the new enclosure
/// lies in the first proof box or the first enclosure in the new proof box, and in 874 only one of
/// the two holds.
#[test]
fn a_zero_proven_twice_is_returned_once() {
    for s in [336, 388, 874] {
        board_case::<2>(0x6753_0000_0001_0002, s);
    }
}

/// A flagged region that a proof box found later meets is cut against it, and what lies outside is
/// examined again (system 5784 of one unknown); one inside a later proof box leaves nothing (system
/// 2927 of two unknowns). No flagged region is left inside a proof box, and no zero is lost.
#[test]
fn flagged_regions_a_later_proof_box_meets_are_cut_or_dropped() {
    board_case::<1>(0x6753_0000_0001_0001, 5784);
    board_case::<2>(0x6753_0000_0001_0002, 2927);
}

// ------------------------------------------------------------------------------------------------
// The double-double balls.

/// The exact ends of a ball: centre − radius and centre + radius.
fn ends(b: Ball) -> (BigRational, BigRational) {
    let centre = exact(b.mid().hi()) + exact(b.mid().lo());
    let radius = exact(b.rad());
    (&centre - &radius, centre + radius)
}

fn holds(b: Ball, x: &BigRational) -> bool {
    let (lo, hi) = ends(b);
    lo <= *x && *x <= hi
}

/// The exact reals a test tries in a ball: its two ends and its centre.
fn samples(b: Ball) -> [BigRational; 3] {
    let (lo, hi) = ends(b);
    let centre = exact(b.mid().hi()) + exact(b.mid().lo());
    [lo, centre, hi]
}

/// A decimal written with a point, exactly.
fn decimal(s: &str) -> BigRational {
    let (whole, frac) = s.split_once('.').unwrap();
    let digits: BigInt = format!("{whole}{frac}").parse().unwrap();
    BigRational::new(digits, BigInt::from(10).pow(frac.len() as u32))
}

/// π to 50 decimal places, and the same plus one in the last place: π lies between them.
const PI_50: &str = "3.14159265358979323846264338327950288419716939937510";
const PI_50_UP: &str = "3.14159265358979323846264338327950288419716939937511";

#[test]
fn the_ball_of_pi_holds_pi() {
    let pi = Ball::pi();
    assert!(holds(pi, &decimal(PI_50)) && holds(pi, &decimal(PI_50_UP)));
    assert_eq!(pi.rad(), pow2(-52));
    assert_eq!(pi.mid().hi(), core::f64::consts::PI);
    // The radius is less than twice π's distance from its nearest double: not loose.
    assert!(exact(pi.rad()) < (decimal(PI_50) - exact(core::f64::consts::PI)) * int(2));
}

/// A seeded ball: a point, an interval, or a quotient whose centre has a trailing part, at
/// magnitudes from 2^−30 to 2^30.
fn random_ball(rng: &mut SplitMix64) -> Ball {
    let scale = pow2(between(rng, -30, 30) as i32);
    let centre = between(rng, -1000, 1000) as f64 / 7.0 * scale;
    let width = match rng.next_u64() % 3 {
        0 => 0.0,
        1 => scale * 1e-9,
        _ => scale * between(rng, 1, 400) as f64 / 100.0,
    };
    let base = Ball::from_interval(iv(centre, centre + width)).unwrap();
    if rng.next_u64() % 2 == 0 {
        base
    } else {
        base.div(Ball::constant(r(3.0))).unwrap()
    }
}

/// Every operation's ball holds the exact operation on every pair of reals tried in its operands,
/// against exact rationals.
#[test]
fn ball_operations_hold_every_exact_result() {
    let seed = resolve("attack_g53 ball operations", Seed(0x6753_0000_0000_0002)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..3000 {
        let (a, b, c3) = (
            random_ball(&mut rng),
            random_ball(&mut rng),
            random_ball(&mut rng),
        );
        let (sa, sb, sc) = (samples(a), samples(b), samples(c3));
        for xa in &sa {
            assert!(holds(a.neg(), &-xa.clone()));
            assert!(holds(a.square().unwrap(), &(xa * xa)));
            for xb in &sb {
                assert!(holds(a.add(b).unwrap(), &(xa + xb)));
                assert!(holds(a.sub(b).unwrap(), &(xa - xb)));
                assert!(holds(a.mul(b).unwrap(), &(xa * xb)));
                for xc in &sc {
                    assert!(holds(a.mul_add(b, c3).unwrap(), &(xa * xb + xc)));
                }
            }
        }
        match a.div(b) {
            Ok(q) => {
                for xa in &sa {
                    for xb in &sb {
                        assert!(holds(q, &(xa / xb)));
                    }
                }
            }
            Err(e) => {
                assert_eq!(e, NumError::DivisorContainsZero);
                // Refused only when the divisor's ball comes within a hair of zero.
                let (lo, hi) = ends(b);
                let centre = exact(b.mid().hi()).abs();
                assert!(
                    lo.abs().min(hi.abs()) <= centre * exact(pow2(-40))
                        || lo.is_negative() != hi.is_negative()
                );
            }
        }
        match a.sqrt() {
            Ok(s) => {
                let (lo, hi) = ends(s);
                for xa in sa.iter().filter(|x| !x.is_negative()) {
                    assert!(lo.is_negative() || &lo * &lo <= *xa, "sqrt's lower end");
                    assert!(!hi.is_negative() && &hi * &hi >= *xa, "sqrt's upper end");
                }
            }
            Err(e) => {
                assert_eq!(e, NumError::NegativeSqrt);
                let (lo, _) = ends(a);
                let centre = exact(a.mid().hi()).abs();
                assert!(lo <= centre * exact(pow2(-50)), "a root refused above zero");
            }
        }
    }
}

/// Each operation's radius is the bound its documentation states and no wider: the operands'
/// spread plus the double-double rounding term (`2B·|c| + 2A`), recomputed in exact rationals, with
/// a relative slack of 2^−40 for the rounding up of each step and 2^−1000 absolute; and a point's
/// interval is the two doubles beside it.
#[test]
fn ball_radii_are_their_formulas_and_no_wider() {
    use geom_num::dd::{
        ADD_BOUND, DIV_BOUND, MUL_BOUND, MUL_F64_BOUND, SQRT_BOUND, UNDERFLOW_BOUND,
    };
    let seed = resolve("attack_g53 ball radii", Seed(0x6753_0000_0000_0005)).unwrap();
    let mut rng = SplitMix64::new(seed);
    let slack = |x: BigRational| x * (int(1) + exact(pow2(-40))) + exact(pow2(-1000));
    let centre = |b: Ball| exact(b.mid().hi()) + exact(b.mid().lo());
    let term = |c: &BigRational, bound: f64, a: f64| exact(2.0 * bound) * c.abs() + exact(2.0 * a);
    for _ in 0..3000 {
        let (a, b) = (random_ball(&mut rng), random_ball(&mut rng));
        let (ca, cb, ra, rb) = (centre(a), centre(b), exact(a.rad()), exact(b.rad()));
        let sum = a.add(b).unwrap();
        let bound = &ra + &rb + term(&centre(sum), ADD_BOUND, 0.0);
        assert!(exact(sum.rad()) <= slack(bound) && exact(sum.rad()) >= &ra + &rb);
        let diff = a.sub(b).unwrap();
        let bound = &ra + &rb + term(&centre(diff), ADD_BOUND, 0.0);
        assert!(exact(diff.rad()) <= slack(bound) && exact(diff.rad()) >= &ra + &rb);
        let prod = a.mul(b).unwrap();
        let spread = ca.abs() * &rb + cb.abs() * &ra + &ra * &rb;
        let bound = &spread + term(&centre(prod), MUL_BOUND, UNDERFLOW_BOUND);
        assert!(exact(prod.rad()) <= slack(bound) && exact(prod.rad()) >= spread);
        let sq = a.square().unwrap();
        let spread = int(2) * ca.abs() * &ra + &ra * &ra;
        let bound = &spread + term(&centre(sq), MUL_BOUND, UNDERFLOW_BOUND);
        assert!(exact(sq.rad()) <= slack(bound) && exact(sq.rad()) >= spread);
        // A divisor well away from zero: near zero, the double below its size moves the gap by
        // far more than the slack.
        if let (Ok(q), true) = (a.div(b), &rb * int(2) < cb.abs()) {
            let size = cb.abs();
            let spread = (&ra * &size + ca.abs() * &rb) / (&size * (&size - &rb));
            let bound = &spread + term(&centre(q), DIV_BOUND, UNDERFLOW_BOUND);
            assert!(exact(q.rad()) <= slack(bound) && exact(q.rad()) >= spread);
        }
        if let Ok(root) = a.sqrt() {
            // The slope bound r/√c, checked squared: (rad − rounding)² · c ≤ r² (with slack).
            let rounding = term(&centre(root), SQRT_BOUND, 0.0);
            let spread = exact(root.rad()) - rounding;
            if ra > int(0) && ra < ca.clone() * exact(pow2(-10)) {
                assert!(
                    &spread * &spread * &ca <= slack(slack(&ra * &ra)),
                    "a root's radius"
                );
            }
        }
        let lo = between(&mut rng, -1000, 1000) as f64 / 3.0;
        let hi = lo + between(&mut rng, 1, 100) as f64 / 7.0;
        let made = Ball::from_interval(iv(lo, hi)).unwrap();
        let half = (exact(hi) - exact(lo)) / int(2);
        let bound = &half + term(&centre(made), MUL_F64_BOUND, UNDERFLOW_BOUND);
        assert!(exact(made.rad()) <= slack(bound) && exact(made.rad()) >= half);
        let x = between(&mut rng, -1 << 40, 1 << 40) as f64 / 5.0;
        assert_eq!(
            Ball::constant(r(x)).to_interval().unwrap(),
            iv(x.next_down(), x.next_up())
        );
        // A centre with a trailing part: each end one double past the leading part on that side.
        let third = Ball::constant(r(x)).div(Ball::constant(r(3.0))).unwrap();
        let i = third.to_interval().unwrap();
        let (h, t, rad) = (third.mid().hi(), third.mid().lo(), third.rad());
        let below = if t < 0.0 { h.next_down() } else { h };
        let above = if t > 0.0 { h.next_up() } else { h };
        assert_eq!(i, iv((below - rad).next_down(), (above + rad).next_up()));
    }
}

/// Sums and products have the same bits in either order, and on exact operands the centre is the
/// double-double operation's own result.
#[test]
fn balls_commute_and_their_centres_are_the_double_double_results() {
    let seed = resolve("attack_g53 ball order", Seed(0x6753_0000_0000_0003)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..2000 {
        let (a, b) = (random_ball(&mut rng), random_ball(&mut rng));
        assert_eq!(a.add(b), b.add(a));
        assert_eq!(a.mul(b), b.mul(a));
        let (x, y) = (
            between(&mut rng, 1, 1 << 40) as f64 / 3.0,
            between(&mut rng, 1, 1 << 40) as f64 / 7.0,
        );
        let (bx, by) = (Ball::constant(r(x)), Ball::constant(r(y)));
        let (dx, dy) = (DD::from_f64(x).unwrap(), DD::from_f64(y).unwrap());
        assert_eq!(bx.add(by).unwrap().mid(), dx.add(dy).unwrap());
        assert_eq!(bx.sub(by).unwrap().mid(), dx.sub(dy).unwrap());
        assert_eq!(bx.mul(by).unwrap().mid(), dx.mul(dy).unwrap());
        assert_eq!(bx.div(by).unwrap().mid(), dx.div(dy).unwrap());
        assert_eq!(bx.sqrt().unwrap().mid(), dx.sqrt().unwrap());
        assert_eq!(bx.square().unwrap().mid(), dx.mul(dx).unwrap());
    }
}

/// A ball and its interval hold each other, a point interval is an exact ball, and the maths
/// functions taken through intervals hold the interval functions' results.
#[test]
fn balls_and_intervals_hold_each_other() {
    let seed = resolve("attack_g53 ball intervals", Seed(0x6753_0000_0000_0004)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..2000 {
        let a = random_ball(&mut rng);
        let i = a.to_interval().unwrap();
        let (lo, hi) = ends(a);
        assert!(exact(i.lo()) <= lo && hi <= exact(i.hi()));
        let back = Ball::from_interval(i).unwrap();
        assert!(holds(back, &exact(i.lo())) && holds(back, &exact(i.hi())));
        let small = Ball::from_interval(iv(-0.3, 0.2))
            .unwrap()
            .mul(
                a.div(Ball::constant(r(1e9)))
                    .unwrap()
                    .add(Ball::constant(r(1.0)))
                    .unwrap(),
            )
            .unwrap();
        let fs: [(
            fn(Ball) -> Result<Ball, NumError>,
            fn(Interval) -> Result<Interval, NumError>,
        ); 4] = [
            (Ball::exp, Interval::exp),
            (Ball::ln, Interval::ln),
            (Ball::sinpi, Interval::sinpi),
            (Ball::cospi, Interval::cospi),
        ];
        for x in [a, small] {
            for (fb, fi) in fs {
                match (fb(x), fi(x.to_interval().unwrap())) {
                    (Ok(b), Ok(v)) => {
                        assert!(holds(b, &exact(v.lo())) && holds(b, &exact(v.hi())));
                    }
                    (Err(e), Err(f)) => assert_eq!(e, f),
                    (b, v) => panic!("ball {b:?} against interval {v:?}"),
                }
            }
            let y = Ball::constant(r(1.5));
            let b = y.atan2(x).unwrap();
            let v = Interval::from(r(1.5))
                .atan2(x.to_interval().unwrap())
                .unwrap();
            assert!(holds(b, &exact(v.lo())) && holds(b, &exact(v.hi())));
        }
    }
    let point = Ball::from_interval(iv(0.1, 0.1)).unwrap();
    assert_eq!(
        (point.mid().hi(), point.mid().lo(), point.rad()),
        (0.1, 0.0, 0.0)
    );
    assert_eq!(Ball::constant(r(-2.5)).neg(), Ball::constant(r(2.5)));
    assert_eq!(Ball::constant(r(0.0)).sqrt().unwrap().mid(), DD::ZERO);
}

/// Each refusal: an overflow is `NotFinite`, a divisor ball through zero
/// `DivisorContainsZero`, a root of a ball reaching below zero `NegativeSqrt`, and a ball cannot
/// take the root of a square that reaches zero, which an interval can.
#[test]
fn balls_refuse_what_they_cannot_hold() {
    let max = f64::MAX;
    assert_eq!(
        Ball::from_interval(iv(max.next_down(), max)),
        Err(NumError::NotFinite)
    );
    assert_eq!(Ball::from_interval(iv(-max, max)), Err(NumError::NotFinite));
    let near_max = Ball::constant(r(max)).sub(Ball::constant(r(1.0))).unwrap();
    assert_eq!(near_max.to_interval(), Err(NumError::NotFinite));
    let huge = Ball::from_interval(iv(-(pow2(1000)), pow2(1000))).unwrap();
    assert_eq!(huge.mul(huge), Err(NumError::NotFinite));
    assert_eq!(
        Ball::constant(r(max)).mul(Ball::constant(r(2.0))),
        Err(NumError::NotFinite)
    );
    let around_zero = Ball::from_interval(iv(-1.0, 1.0)).unwrap();
    assert_eq!(
        Ball::constant(r(1.0)).div(around_zero),
        Err(NumError::DivisorContainsZero)
    );
    assert_eq!(
        Ball::constant(r(1.0)).div(Ball::constant(r(0.0))),
        Err(NumError::DivisorContainsZero)
    );
    assert_eq!(
        Ball::constant(r(-1e-300)).sqrt(),
        Err(NumError::NegativeSqrt)
    );
    assert_eq!(
        around_zero.square().unwrap().sqrt(),
        Err(NumError::NegativeSqrt)
    );
    assert!(iv(-1.0, 1.0).square().unwrap().sqrt().is_ok());
    assert_eq!(Ball::constant(r(-1.0)).ln(), Err(NumError::OutsideDomain));
}

/// The reason for balls: a residual that is the small difference of large terms is wide in
/// doubles and narrow in balls.
#[test]
fn balls_survive_the_cancellation_intervals_do_not() {
    let big = pow2(60);
    let x = 1000.25;
    let ball = Ball::constant(r(x))
        .add(c(big))
        .unwrap()
        .sub(c(big))
        .unwrap();
    let interval = Interval::from(r(x))
        .add(c(big))
        .unwrap()
        .sub(c(big))
        .unwrap();
    assert!(holds(ball, &exact(x)));
    assert!(ball.rad() < 1e-12, "{ball:?}");
    assert!(interval.hi() - interval.lo() >= 256.0, "{interval:?}");
}

/// Dual numbers over balls give enclosures of derivatives, and keep the cut refusal of the angle.
#[test]
fn dual_balls_enclose_derivatives() {
    let at = [Ball::constant(r(1.5)), Ball::constant(r(-2.25))];
    let (values, jac) = jacobian(
        |v: [Dual<Ball, 2>; 2]| -> Result<[Dual<Ball, 2>; 2], NumError> {
            Ok([v[0].mul(v[1])?, v[0].square()?.add(v[1].exp()?)?])
        },
        at,
    )
    .unwrap();
    assert!(holds(values[0], &(exact(1.5) * exact(-2.25))));
    assert!(holds(jac[0][0], &exact(-2.25)) && holds(jac[0][1], &exact(1.5)));
    assert!(holds(jac[1][0], &exact(3.0)));
    let cut = jacobian(
        |v: [Dual<Ball, 2>; 2]| -> Result<[Dual<Ball, 2>; 1], NumError> { Ok([v[0].atan2(v[1])?]) },
        [
            Ball::from_interval(iv(-0.1, 0.1)).unwrap(),
            Ball::from_interval(iv(-2.0, -1.0)).unwrap(),
        ],
    );
    assert_eq!(cut.err(), Some(NumError::OutsideDomain));
}

// ------------------------------------------------------------------------------------------------
// The engine's cases.

/// One unknown: the product of `x − rᵢ`, with the position `x` times `scale` grid steps.
struct Roots {
    roots: Vec<f64>,
    scale: f64,
}

impl Residuals<1> for Roots {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let mut acc = c::<S>(1.0);
        for root in &self.roots {
            acc = acc.mul(x[0].sub(c(*root))?)?;
        }
        Ok([acc])
    }
}

impl System<1, 1> for Roots {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([c::<S>(self.scale).mul(x[0])?])
    }
}

fn run<Sys: System<N, D>, const N: usize, const D: usize>(
    system: &Sys,
    domain: [(f64, f64); N],
    options: Options,
) -> Result<Isolation<N>, IsolateError<N>> {
    let mut meter = WorkMeter::new(ROOMY);
    isolate(
        system,
        domain.map(|(lo, hi)| iv(lo, hi)),
        options,
        &mut meter,
    )
}

fn holds_point<const N: usize>(b: &[Interval; N], x: [f64; N]) -> bool {
    b.iter().zip(x).all(|(i, v)| i.lo() <= v && v <= i.hi())
}

/// Zeros exactly on the lines where boxes split, which no box of the subdivision holds in its
/// interior, are proven by inflation, each once, by either operator.
#[test]
fn zeros_on_split_lines_are_proven_once() {
    for operator in [Operator::Krawczyk, Operator::HansenSengupta] {
        let options = Options {
            operator,
            ..Options::DEFAULT
        };
        let roots = vec![-1.0, -0.5, 0.0, 0.5, 1.0];
        let found = run(
            &Roots {
                roots: roots.clone(),
                scale: 64.0,
            },
            [(-2.0, 2.0)],
            options,
        )
        .unwrap();
        assert_eq!(found.zeros.len(), 5, "{found:?}");
        assert!(found.flagged.is_empty(), "{found:?}");
        for (z, root) in found.zeros.iter().zip(&roots) {
            assert!(holds_point(&z.enclosure, [*root]));
        }
        let found = run(&Grid, [(-2.0, 2.0), (-2.0, 2.0)], options).unwrap();
        assert_eq!(found.zeros.len(), 4, "{found:?}");
        assert!(found.flagged.is_empty(), "{found:?}");
    }
}

/// `x(x − 1) = 0, y(y + 1) = 0`: four zeros on split lines.
struct Grid;

impl Residuals<2> for Grid {
    fn residuals<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([x[0].mul(x[0].sub(c(1.0))?)?, x[1].mul(x[1].add(c(1.0))?)?])
    }
}

impl System<2, 2> for Grid {
    fn position<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([c::<S>(64.0).mul(x[0])?, c::<S>(64.0).mul(x[1])?])
    }
}

/// A zero on the domain's boundary cannot be proven inside it, so it is flagged, never lost.
#[test]
fn a_zero_on_the_domain_boundary_is_flagged() {
    let system = Roots {
        roots: vec![0.0],
        scale: 64.0,
    };
    let found = run(&system, [(0.0, 1.0)], Options::DEFAULT).unwrap();
    assert!(found.zeros.is_empty());
    assert!(found.flagged.iter().any(|f| holds_point(&f.region, [0.0])));
    let found = run(&system, [(-1.0, 1.0)], Options::DEFAULT).unwrap();
    assert_eq!(found.zeros.len(), 1);
}

/// Zeros closer together than the floor are flagged together; a finer frame separates them, and a
/// target below the floor refines each enclosure below it.
#[test]
fn the_floor_decides_what_is_separated() {
    let near = Roots {
        roots: vec![0.25, 0.5],
        scale: 1.0,
    };
    let found = run(&near, [(-3.0, 3.0)], Options::DEFAULT).unwrap();
    assert!(found.zeros.is_empty(), "{found:?}");
    for root in [0.25, 0.5] {
        assert!(found.flagged.iter().any(|f| holds_point(&f.region, [root])));
    }
    let finer = Roots {
        roots: vec![0.25, 0.5],
        scale: 64.0,
    };
    assert_eq!(
        run(&finer, [(-3.0, 3.0)], Options::DEFAULT)
            .unwrap()
            .zeros
            .len(),
        2
    );
    let options = Options {
        target: r(1e-9),
        ..Options::DEFAULT
    };
    let found = run(
        &Roots {
            roots: vec![1.5, 3.25],
            scale: 1.0,
        },
        [(-8.0, 8.0)],
        options,
    )
    .unwrap();
    assert_eq!(found.zeros.len(), 2);
    for z in &found.zeros {
        assert!(z.enclosure[0].hi() - z.enclosure[0].lo() <= 1e-9);
    }
}

/// Multiple zeros, zeros of a domain with an unknown fixed at a point, and dependent systems are
/// never proven: they are flagged, singular where the midpoint Jacobian is.
#[test]
fn multiple_zeros_and_dependent_systems_are_flagged() {
    for roots in [vec![0.5, 0.5], vec![1.0, 1.0, 1.0]] {
        let root = roots[0];
        let found = run(
            &Roots { roots, scale: 64.0 },
            [(-2.0, 2.0)],
            Options::DEFAULT,
        )
        .unwrap();
        assert!(found.zeros.is_empty(), "{found:?}");
        assert!(found.flagged.iter().any(|f| holds_point(&f.region, [root])));
    }
    // An unknown fixed at a point: no box can hold a zero strictly inside it there.
    let found = run(&Grid, [(0.0, 0.0), (-2.0, 2.0)], Options::DEFAULT).unwrap();
    assert!(found.zeros.is_empty(), "{found:?}");
    for zero in [[0.0, 0.0], [0.0, -1.0]] {
        assert!(found.flagged.iter().any(|f| holds_point(&f.region, zero)));
    }
    let found = run(&Dependent, [(-1.0, 1.0), (-1.0, 1.0)], Options::DEFAULT).unwrap();
    assert!(found.zeros.is_empty());
    assert!(found.flagged.iter().any(|f| f.reason == Reason::Singular));
    assert!(
        found
            .flagged
            .iter()
            .any(|f| holds_point(&f.region, [0.25, -0.25]))
    );
}

/// `x + y = 0, 2x + 2y = 0`: a line of zeros, its Jacobian singular everywhere.
struct Dependent;

impl Residuals<2> for Dependent {
    fn residuals<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        let s = x[0].add(x[1])?;
        Ok([s, c::<S>(2.0).mul(s)?])
    }
}

impl System<2, 2> for Dependent {
    fn position<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([c::<S>(4.0).mul(x[0])?, c::<S>(4.0).mul(x[1])?])
    }
}

/// `√(x²) = 0`: a zero where the residual is not differentiable is flagged as not smooth.
struct Kink;

impl Residuals<1> for Kink {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].square()?.sqrt()?])
    }
}

impl System<1, 1> for Kink {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([c::<S>(64.0).mul(x[0])?])
    }
}

#[test]
fn a_zero_where_the_residual_has_no_derivative_is_flagged_not_smooth() {
    let found = run(&Kink, [(-1.3, 1.1)], Options::DEFAULT).unwrap();
    assert!(found.zeros.is_empty());
    assert!(
        found
            .flagged
            .iter()
            .any(|f| f.reason == Reason::NotSmooth && holds_point(&f.region, [0.0]))
    );
}

/// A system whose arithmetic cancels: `((x + shift) − shift) − at`, with the position
/// `(x + pshift) − pshift`. A large `shift` makes the residuals wide in doubles; a large `pshift`
/// makes the position's own evaluation error large.
struct Cancelling {
    shift: f64,
    pshift: f64,
    at: f64,
}

impl Residuals<1> for Cancelling {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0]
            .add(c(self.shift))?
            .sub(c(self.shift))?
            .sub(c(self.at))?])
    }
}

impl System<1, 1> for Cancelling {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].add(c(self.pshift))?.sub(c(self.pshift))?])
    }
}

/// An evaluation error of a quarter step or more moves the box into double-double balls, and the
/// zero is proven there; one that stays that large there stops the engine.
#[test]
fn a_large_evaluation_error_moves_to_double_double() {
    let big = pow2(60);
    for operator in [Operator::Krawczyk, Operator::HansenSengupta] {
        let options = Options {
            operator,
            ..Options::DEFAULT
        };
        let system = Cancelling {
            shift: big,
            pshift: big,
            at: 1000.25,
        };
        let found = run(&system, [(0.0, 2048.0)], options).unwrap();
        assert_eq!(found.zeros.len(), 1, "{found:?}");
        assert_eq!(found.zeros[0].precision, Precision::DoubleDouble);
        assert!(holds_point(&found.zeros[0].enclosure, [1000.25]));
    }
    let hopeless = Cancelling {
        shift: 0.0,
        pshift: pow2(200),
        at: 1.5,
    };
    assert!(matches!(
        run(&hopeless, [(0.0, 4.0)], Options::DEFAULT),
        Err(IsolateError::Imprecise { .. })
    ));
}

/// Residuals too wide in doubles to decide anything near their zero: the boxes at the floor are
/// evaluated again in balls, which decide them.
#[test]
fn undecided_boxes_at_the_floor_are_retried_in_double_double() {
    let system = Cancelling {
        shift: pow2(60),
        pshift: 0.0,
        at: 1024.0,
    };
    let found = run(&system, [(0.0, 2048.0)], Options::DEFAULT).unwrap();
    assert_eq!(found.zeros.len(), 1, "{found:?}");
    assert!(found.flagged.is_empty(), "{} flagged", found.flagged.len());
    assert_eq!(found.zeros[0].precision, Precision::DoubleDouble);
}

/// A refinement that stops shrinking in doubles continues in balls.
#[test]
fn a_refinement_that_stalls_in_doubles_continues_in_double_double() {
    let system = Cancelling {
        shift: pow2(30),
        pshift: 0.0,
        at: 0.125,
    };
    let options = Options {
        target: r(1e-9),
        ..Options::DEFAULT
    };
    let found = run(&system, [(-4.0, 4.0)], options).unwrap();
    assert_eq!(found.zeros.len(), 1, "{found:?}");
    let z = found.zeros[0];
    assert_eq!(z.precision, Precision::DoubleDouble);
    assert!(z.enclosure[0].hi() - z.enclosure[0].lo() <= 1e-9);
    assert!(holds_point(&z.enclosure, [0.125]));
}

/// `√(x² + 1) − 2`, evaluated in balls because its position's first coordinate cancels: over wide
/// boxes the ball of `x² + 1` reaches below zero and its root is refused, so the engine encloses
/// those boxes in doubles (their residuals, and their image through the position's second
/// coordinate, the same root), and still proves both zeros.
struct Hypot;

impl Residuals<1> for Hypot {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].square()?.add(c(1.0))?.sqrt()?.sub(c(2.0))?])
    }
}

impl System<1, 2> for Hypot {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 2], NumError> {
        let big = pow2(60);
        Ok([
            x[0].add(c(big))?.sub(c(big))?,
            x[0].square()?.add(c(1.0))?.sqrt()?,
        ])
    }
}

#[test]
fn balls_that_fail_fall_back_to_doubles() {
    let found = run(&Hypot, [(-4.0, 4.0)], Options::DEFAULT).unwrap();
    assert_eq!(found.zeros.len(), 2, "{found:?}");
    let three = 3.0f64.sqrt();
    assert!(
        holds_point(&found.zeros[0].enclosure, [-three])
            || holds_point(&found.zeros[1].enclosure, [-three])
    );
}

// ------------------------------------------------------------------------------------------------
// Every cap is an error, and so is every failure.

#[test]
fn every_budget_and_stop_is_an_error() {
    let system = Roots {
        roots: vec![-0.75, 0.3, 1.2],
        scale: 64.0,
    };
    let domain = [iv(-2.0, 2.0)];
    let mut meter = WorkMeter::new(Budget::of_work(5));
    match isolate(&system, domain, Options::DEFAULT, &mut meter) {
        Err(IsolateError::Exceeded(e)) => {
            assert_eq!((e.kind, e.phase), (BudgetKind::Work, PHASE));
            assert!(e.used.work > 5);
        }
        other => panic!("{other:?}"),
    }
    let mut meter = WorkMeter::new(Budget::new(u64::MAX, 100));
    match isolate(&system, domain, Options::DEFAULT, &mut meter) {
        Err(IsolateError::Exceeded(e)) => assert_eq!(e.kind, BudgetKind::LogicalBytes),
        other => panic!("{other:?}"),
    }
    let token = CancelToken::new();
    token.cancel();
    let mut meter = WorkMeter::new(ROOMY).with_cancel(&token);
    assert_eq!(
        isolate(&system, domain, Options::DEFAULT, &mut meter),
        Err(IsolateError::Cancelled)
    );
    let mut meter = WorkMeter::new(ROOMY);
    let _ = meter.out_of_memory();
    assert_eq!(
        isolate(&system, domain, Options::DEFAULT, &mut meter),
        Err(IsolateError::OutOfMemory)
    );
    for target in [0.0, -1.0] {
        let options = Options {
            target: r(target),
            ..Options::DEFAULT
        };
        assert_eq!(
            run(&system, [(-2.0, 2.0)], options),
            Err(IsolateError::InvalidTarget)
        );
    }
    // The work the whole isolation charges: one unit for each box and for each operator step, the
    // same on every run.
    let mut a = WorkMeter::new(ROOMY);
    let mut b = WorkMeter::new(ROOMY);
    let first = isolate(&system, domain, Options::DEFAULT, &mut a).unwrap();
    assert_eq!(
        isolate(&system, domain, Options::DEFAULT, &mut b).unwrap(),
        first
    );
    assert_eq!(a.used(), b.used());
    assert_eq!(first.zeros.len(), 3);
}

/// `(x − 1.5)·(1 + k₁(π − π)) + k₂(π − π)`: exactly `x − 1.5` at every point, but enclosed loosely:
/// over any box, even a point, `π − π` is an interval about ±4.4 · 10⁻¹⁶ wide in doubles. So its
/// Jacobian's enclosure is ±0.4 wide however small the box (contraction bound 0.4), and its value's
/// ±`k₂ · 4.4 · 10⁻¹⁶`: a refinement shrinks by less each step than the bound predicts as it
/// nears that width, and a target just above it takes longer than the cap allows.
struct Loose {
    k2: f64,
}

impl Residuals<1> for Loose {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let slack = S::pi().sub(S::pi())?;
        let k1 = 0.8 * pow2(49);
        let factor = c::<S>(1.0).add(c::<S>(k1).mul(slack)?)?;
        Ok([x[0]
            .sub(c(1.5))?
            .mul(factor)?
            .add(c::<S>(self.k2).mul(slack)?)?])
    }
}

impl System<1, 1> for Loose {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok(x)
    }
}

/// `x · (1 + 2⁵⁰(π − π))`, a zero at 0 whose Jacobian's enclosure is about `[½, 3/2]` however small
/// the box: each refinement step halves the enclosure (the contraction bound is ½, and the value
/// at the midpoint, 0, is exact), as the bound predicts. Its position is the unknown at 2³⁰ steps
/// per unit plus `blur · (π − π)`: with a blur the image stops shrinking near `2^−50 · blur`
/// steps, while the unknown keeps halving.
struct Halving {
    blur: f64,
}

impl Residuals<1> for Halving {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let slack = S::pi().sub(S::pi())?;
        let factor = c::<S>(1.0).add(c::<S>(pow2(50)).mul(slack)?)?;
        Ok([x[0].mul(factor)?])
    }
}

impl System<1, 1> for Halving {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let slack = S::pi().sub(S::pi())?;
        Ok([c::<S>(pow2(30))
            .mul(x[0])?
            .add(c::<S>(self.blur).mul(slack)?)?])
    }
}

/// The per-zero cap of 12.3, `⌈log(target / image) / log r⌉ + 8` steps, counted exactly. The first
/// box, `[−3, 5]`, proves the zero with enclosure `[−2.5, 2.5]` (contraction bound ½): an image of
/// 5 · 2³⁰ steps. To a target of 2⁻³⁰ steps that predicts ⌈60 + log₂ 5⌉ = 63 steps, and halving
/// takes exactly those, inside the cap of 71. Blurred, the image never reaches a target of 2⁻⁴⁰
/// steps; the cap, ⌈70 + log₂ 5⌉ + 8 = 81 steps, stops it: one unit for the box, one for its step
/// and one for each refinement step, 83 in all.
#[test]
fn a_refinement_takes_the_steps_its_bound_predicts_and_stops_at_its_cap() {
    let mut meter = WorkMeter::new(ROOMY);
    let options = Options {
        target: r(pow2(-30)),
        ..Options::DEFAULT
    };
    let found = isolate(&Halving { blur: 0.0 }, [iv(-3.0, 5.0)], options, &mut meter).unwrap();
    assert_eq!(found.zeros.len(), 1);
    let e = found.zeros[0].enclosure[0];
    assert!(e.lo() <= 0.0 && 0.0 <= e.hi());
    assert!((e.hi() - e.lo()) * pow2(30) <= pow2(-30));
    assert_eq!(found.zeros[0].precision, Precision::Double);
    assert_eq!(meter.used().work, 2 + 63);
    let mut meter = WorkMeter::new(ROOMY);
    let options = Options {
        target: r(pow2(-40)),
        ..Options::DEFAULT
    };
    match isolate(
        &Halving { blur: pow2(20) },
        [iv(-3.0, 5.0)],
        options,
        &mut meter,
    ) {
        Err(IsolateError::CapReached { enclosure }) => {
            assert!(enclosure[0].lo() <= 0.0 && 0.0 <= enclosure[0].hi());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(meter.used().work, 2 + 81);
}

/// `x²`, its unknown in grid steps: a double zero, which no operator proves.
struct Square;

impl Residuals<1> for Square {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].square()?])
    }
}

impl System<1, 1> for Square {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok(x)
    }
}

/// The floor is strict: a box whose image is exactly one step is split, not flagged. The domain
/// `[−½, ½ − 2⁻⁵³]` is `1 − 2⁻⁵³` wide, rounded up once to an image of exactly 1. Its lower half
/// excludes the zero and its upper half, half a step, is flagged.
#[test]
fn a_box_whose_image_is_one_step_is_split() {
    let found = run(&Square, [(-0.5, 0.5 - pow2(-53))], Options::DEFAULT).unwrap();
    assert!(found.zeros.is_empty());
    assert_eq!(found.flagged.len(), 1);
    let f = found.flagged[0].region[0];
    assert!(f.lo() <= 0.0 && 0.0 <= f.hi());
    assert!((f.hi() - f.lo()).next_up() < 1.0, "{f:?}");
}

#[test]
fn every_refinement_cap_is_an_error() {
    // A target no arithmetic reaches: the refinement stops shrinking in doubles, then in balls.
    let options = Options {
        target: r(1e-300),
        ..Options::DEFAULT
    };
    match run(
        &Roots {
            roots: vec![1.5],
            scale: 1.0,
        },
        [(-4.0, 4.0)],
        options,
    ) {
        Err(IsolateError::CapReached { enclosure }) => {
            assert!(holds_point(&enclosure, [1.5]));
        }
        other => panic!("{other:?}"),
    }
    // A refinement that keeps shrinking, but more slowly than its contraction bound predicts.
    let loose = Loose { k2: 1e9 };
    let reached = run(
        &loose,
        [(0.0, 4.0)],
        Options {
            target: r(1e-3),
            ..Options::DEFAULT
        },
    )
    .unwrap();
    assert_eq!(reached.zeros.len(), 1);
    let floor_width = 2.0 * 1e9 * 4.44e-16;
    let options = Options {
        target: r(floor_width * 1.0001),
        ..Options::DEFAULT
    };
    match run(&loose, [(0.0, 4.0)], options) {
        Err(IsolateError::CapReached { enclosure }) => {
            assert!(holds_point(&enclosure, [1.5]));
            assert!(enclosure[0].hi() - enclosure[0].lo() > floor_width);
        }
        other => panic!("{other:?}"),
    }
}

/// `(x − at)² − 10⁻⁴`, with the position `(x + x·2⁹⁰) − x·2⁹⁰`: the position's error in balls
/// grows with `x`, so the domain's first box is evaluated in balls and a box further out is too
/// imprecise even there.
struct Spread {
    at: f64,
}

impl Residuals<1> for Spread {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].sub(c(self.at))?.square()?.sub(c(1e-4))?])
    }
}

impl System<1, 1> for Spread {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let far = x[0].mul(c(pow2(90)))?;
        Ok([x[0].add(far)?.sub(far)?])
    }
}

/// `(x − c)²`, a double zero just above 1, with the position `(x − 1) · 2⁶⁰`: within a frame's
/// range near the zero, yet a box one double wide there spans 256 grid steps, far above the floor.
struct Stretched;

impl Residuals<1> for Stretched {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let middle = 1.0 + 2.0 * f64::EPSILON;
        Ok([x[0].sub(c(middle))?.square()?])
    }
}

impl System<1, 1> for Stretched {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].sub(c(1.0))?.mul(c(pow2(60)))?])
    }
}

/// `ln x`, which has no value below zero.
struct Log;

impl Residuals<1> for Log {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x[0].ln()?])
    }
}

impl System<1, 1> for Log {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok(x)
    }
}

#[test]
fn every_engine_cap_is_an_error() {
    // An evaluation error too large even in balls, found on a box already in balls.
    assert!(matches!(
        run(&Spread { at: 450.0 }, [(0.0, 500.0)], Options::DEFAULT),
        Err(IsolateError::Imprecise { .. })
    ));
    // A box above the floor that cannot be halved.
    let ulp = f64::EPSILON;
    match run(&Stretched, [(1.0, 1.0 + 4.0 * ulp)], Options::DEFAULT) {
        Err(IsolateError::Unsplittable { region }) => {
            assert!(region[0].hi() - region[0].lo() <= ulp);
        }
        other => panic!("{other:?}"),
    }
    // Residuals with no value over part of the domain.
    assert!(matches!(
        run(&Log, [(-1.0, 1.0)], Options::DEFAULT),
        Err(IsolateError::Num(_))
    ));
}

/// What a system's residuals become after they have been evaluated `after` times.
#[derive(Clone, Copy)]
enum Then {
    /// `x² − 100`: the zero moves away.
    Moved,
    /// `0 · x`: zero everywhere, its Jacobian zero.
    Flat,
    /// An overflow.
    Fails,
}

/// `x² − 2.25` for the first `after` evaluations of its residuals, then `then`: residuals that are
/// not a function of the unknowns, which no real system has. The engine must report the proof that
/// no longer closes, never a wrong answer.
struct Fickle {
    calls: Cell<u64>,
    after: u64,
    then: Then,
}

impl Residuals<1> for Fickle {
    fn residuals<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        let n = self.calls.get() + 1;
        self.calls.set(n);
        if n <= self.after {
            return Ok([x[0].square()?.sub(c(2.25))?]);
        }
        match self.then {
            Then::Moved => Ok([x[0].square()?.sub(c(100.0))?]),
            Then::Flat => Ok([x[0].mul(c(0.0))?]),
            Then::Fails => Err(NumError::NotFinite),
        }
    }
}

impl System<1, 1> for Fickle {
    fn position<S: Scalar>(&self, x: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([c::<S>(64.0).mul(x[0])?])
    }
}

#[test]
fn a_proof_that_does_not_close_is_reported() {
    let options = Options {
        target: r(1e-9),
        ..Options::DEFAULT
    };
    let steady = Fickle {
        calls: Cell::new(0),
        after: u64::MAX,
        then: Then::Moved,
    };
    let found = run(&steady, [(1.0, 2.0)], options).unwrap();
    assert_eq!(found.zeros.len(), 1);
    let total = steady.calls.get();
    for then in [Then::Moved, Then::Flat, Then::Fails] {
        let mut internal = 0;
        for after in 0..total {
            let fickle = Fickle {
                calls: Cell::new(0),
                after,
                then,
            };
            match run(&fickle, [(1.0, 2.0)], options) {
                Err(IsolateError::Internal { region }) => {
                    internal += 1;
                    assert!(holds_point(&region, [1.5]));
                }
                Err(IsolateError::Num(e)) => assert_eq!(e, NumError::NotFinite),
                Ok(_) | Err(IsolateError::CapReached { .. }) => {}
                Err(other) => panic!("{other:?}"),
            }
        }
        match then {
            Then::Fails => assert_eq!(internal, 0),
            Then::Moved | Then::Flat => assert!(internal > 0),
        }
    }
}

#[test]
fn every_error_says_what_happened() {
    let b = [iv(0.0, 1.0)];
    let exceeded = Exceeded {
        budget: Budget::of_work(1),
        kind: BudgetKind::Work,
        phase: PHASE,
        used: Usage::new(2, 0),
    };
    let cases: [(IsolateError<1>, &str); 9] = [
        (IsolateError::InvalidTarget, "not positive"),
        (IsolateError::Num(NumError::NotFinite), "NaN or infinite"),
        (IsolateError::Exceeded(exceeded), "isolate zeros"),
        (IsolateError::Cancelled, "cancelled"),
        (IsolateError::OutOfMemory, "refused"),
        (IsolateError::CapReached { enclosure: b }, "cap"),
        (IsolateError::Imprecise { region: b }, "quarter step"),
        (IsolateError::Unsplittable { region: b }, "cannot be split"),
        (IsolateError::Internal { region: b }, "did not close"),
    ];
    for (e, words) in cases {
        let text = e.to_string();
        assert!(text.contains(words), "{text}");
        assert_eq!(
            IsolateError::<1>::from(NumError::NotFinite),
            IsolateError::Num(NumError::NotFinite)
        );
    }
}
