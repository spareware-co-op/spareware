//! Every input class is what it claims to be, so each benchmark measures the path it is named
//! after: degenerate inputs really are exact zeros, nearly degenerate inputs really are nonzero and
//! below every floating-point filter's error bound, overflow inputs really overflow, extreme inputs
//! really reach the largest magnitudes, and every function's batches take the paths they name.
//!
//! Guarantees, when these tests pass: every catalogued batch is full and the same on every run,
//! and every class claim the input builders' documentation makes holds for every input (the
//! generic fixed-width builders from two limbs up).
//!
//! Method: exact values from Laplace expansion over all permutations (Heap's algorithm,
//! <https://en.wikipedia.org/wiki/Heap%27s_algorithm>) in `Wide<8>`, compared with Shewchuk's
//! floating-point filter bounds (1997, section 4, <https://doi.org/10.1007/PL00009321>); the paths
//! of `geom_num::eft` worked out from the conditions its documentation states.
//!
//! Cost: one batch per class of every benchmark, and at most 4! = 24 products per determinant.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use geom_bench::catalogue::{cases, growth_cases, names, prepare_named};
use geom_bench::inputs::Class::{
    Degenerate, Extreme, NearlyDegenerate, OnGrid, Overflow, Typical, WrongDirection,
};
use geom_bench::inputs::{self, BATCH, Class, EXTREME_SLACK, TYPICAL_REACH};
use geom_num::grid::{self, COORD_MAX, GridStep, P2, P3, Steps};
use geom_num::interval::Interval;
use geom_num::plane::{Plane, Tpi};
use geom_num::wide::Wide;
use geom_num::{Exactness, NumError, Real, Sign, eft, math, predicate};

const M: i64 = COORD_MAX;

fn coords2(p: &P2) -> [i64; 2] {
    [p.x.get(), p.y.get()]
}

fn coords3(p: &P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// Every coordinate is 0 or within the extreme slack of ±M, and ±M itself occurs.
fn assert_extreme(coords: impl IntoIterator<Item = i64>) {
    let coords: Vec<i64> = coords.into_iter().collect();
    assert!(
        coords
            .iter()
            .all(|&c| c == 0 || c.abs() >= M - EXTREME_SLACK),
        "{coords:?}"
    );
    assert!(coords.iter().any(|&c| c.abs() == M));
}

/// Every coordinate is within the typical reach.
fn assert_typical(coords: impl IntoIterator<Item = i64>) {
    assert!(coords.into_iter().all(|c| c.abs() <= TYPICAL_REACH));
}

/// 2^e exactly, for a normal exponent e.
fn pow2(e: i64) -> f64 {
    f64::from_bits(((e + 1023) as u64) << 52)
}

/// The binary exponent of a nonzero normal double.
fn exponent(x: f64) -> i64 {
    ((x.to_bits() >> 52) & 0x7FF) as i64 - 1023
}

#[test]
fn every_benchmark_has_a_full_batch() {
    for name in names() {
        assert_eq!(prepare_named(&name).unwrap().len(), BATCH, "{name}");
    }
    assert_eq!(names().len(), cases().count() + growth_cases().count());
}

#[test]
fn classes_an_operation_lacks_are_empty() {
    assert!(inputs::predicate::orient2d(Overflow).is_empty());
    assert!(inputs::wide::add::<4>(Degenerate).is_empty());
    assert!(inputs::wide::resize::<4, 6>(Overflow).is_empty());
    assert!(inputs::plane::to_grid(Degenerate).is_empty());
    assert!(inputs::plane::through(NearlyDegenerate).is_empty());
    assert!(inputs::interval::neg(Extreme).is_empty());
    assert!(inputs::grid::refine(Extreme).is_empty());
    assert!(inputs::grid::coarsen(Overflow).is_empty());
    assert!(inputs::math::sin(OnGrid).is_empty());
    assert!(inputs::math::sin(WrongDirection).is_empty());
}

#[test]
fn inputs_are_the_same_every_time() {
    for name in names() {
        let first = prepare_named(&name).unwrap().run();
        assert_eq!(first, prepare_named(&name).unwrap().run(), "{name}");
    }
}

// ------------------------------------------------------------------------------------------------
// Nearly degenerate predicates: exact values and Shewchuk's filter bounds.
// ------------------------------------------------------------------------------------------------

type W = Wide<8>;

fn w(v: i128) -> W {
    W::from_i128(v)
}

/// The exact determinant of a square matrix of i128 entries, by Laplace expansion, and the largest
/// magnitude of the n! products it sums (as f64): a lower bound on the permanent that Shewchuk's
/// error bounds multiply.
fn det_and_largest_term(m: &[Vec<i128>]) -> (W, f64) {
    let n = m.len();
    let mut det = W::ZERO;
    let mut largest = 0.0f64;
    let mut perm: Vec<usize> = (0..n).collect();
    // Heap's algorithm visits every permutation; the sign flips with every swap.
    let mut c = vec![0usize; n];
    let mut sign = 1i32;
    let mut visit = |perm: &[usize], sign: i32| {
        let mut product = w(1);
        let mut magnitude = 1.0f64;
        for (row, &col) in perm.iter().enumerate() {
            product = product.checked_mul(&w(m[row][col])).unwrap();
            magnitude *= (m[row][col] as f64).abs();
        }
        det = if sign > 0 {
            det.checked_add(&product)
        } else {
            det.checked_sub(&product)
        }
        .unwrap();
        largest = largest.max(magnitude);
    };
    visit(&perm, sign);
    let mut i = 0;
    while i < n {
        if c[i] < i {
            if i % 2 == 0 {
                perm.swap(0, i)
            } else {
                perm.swap(c[i], i)
            }
            sign = -sign;
            visit(&perm, sign);
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
    (det, largest)
}

/// |x| as a double (rounded; enough for comparing with an error bound with a factor 2 to spare).
fn magnitude(x: &W) -> f64 {
    let x = if x.is_negative() {
        x.checked_neg().unwrap()
    } else {
        *x
    };
    x.to_le_limbs()
        .iter()
        .rev()
        .fold(0.0, |acc, &limb| acc * pow2(64) + limb as f64)
}

/// Shewchuk's ε, 2^−53.
const EPS: f64 = 1.0 / 9_007_199_254_740_992.0;

/// Asserts the determinant is nonzero and smaller than half of `k` · ε times its largest term, so
/// Shewchuk's stage-A filter, whose bound is (k + O(ε)) · ε times a permanent at least that large,
/// can never certify its sign.
fn assert_below_filter(what: &str, m: &[Vec<i128>], k: f64) {
    let (det, largest) = det_and_largest_term(m);
    assert!(!det.is_zero(), "{what}: exactly degenerate");
    assert!(
        magnitude(&det) < 0.5 * k * EPS * largest,
        "{what}: a filter could certify it"
    );
}

fn d(a: i64, b: i64) -> i128 {
    i128::from(a) - i128::from(b)
}

#[test]
fn nearly_degenerate_predicates_defeat_every_filter() {
    use inputs::predicate as q;
    for t @ [a, b, c] in q::orient2d(NearlyDegenerate) {
        let rows = [a, b].map(|p| vec![d(p.x.get(), c.x.get()), d(p.y.get(), c.y.get())]);
        // Twice the area is exactly ±1.
        let (det, _) = det_and_largest_term(&rows);
        assert!(det == w(1) || det == w(-1), "{t:?}");
        assert_below_filter("orient2d", &rows, 3.0);
    }
    for t @ [a, b, c, e] in q::orient3d(NearlyDegenerate) {
        let rows = [a, b, c].map(|p| {
            vec![
                d(p.x.get(), e.x.get()),
                d(p.y.get(), e.y.get()),
                d(p.z.get(), e.z.get()),
            ]
        });
        let (det, _) = det_and_largest_term(&rows);
        assert!(det == w(1) || det == w(-1), "{t:?}");
        assert_below_filter("orient3d", &rows, 7.0);
    }
    for [a, b, c, e] in q::incircle(NearlyDegenerate) {
        let rows = [a, b, c].map(|p| {
            let (x, y) = (d(p.x.get(), e.x.get()), d(p.y.get(), e.y.get()));
            vec![x, y, x * x + y * y]
        });
        assert_below_filter("incircle", &rows, 10.0);
    }
    for [a, b, c, e, f] in q::insphere(NearlyDegenerate) {
        let rows = [a, b, c, e].map(|p| {
            let x = d(p.x.get(), f.x.get());
            let y = d(p.y.get(), f.y.get());
            let z = d(p.z.get(), f.z.get());
            vec![x, y, z, x * x + y * y + z * z]
        });
        assert_below_filter("insphere", &rows, 16.0);
    }
}

#[test]
fn predicate_classes() {
    use inputs::predicate as q;
    // Every answer on grid points is a sign: the predicates' `BudgetExceeded` cannot occur.
    let o2 = |c| {
        q::orient2d(c)
            .into_iter()
            .map(|[a, b, c]| predicate::orient2d(a, b, c).unwrap())
    };
    let o3 = |c| {
        q::orient3d(c)
            .into_iter()
            .map(|[a, b, c, d]| predicate::orient3d(a, b, c, d).unwrap())
    };
    let ic = |c| {
        q::incircle(c)
            .into_iter()
            .map(|[a, b, c, d]| predicate::incircle(a, b, c, d).unwrap())
    };
    let is = |c| {
        let batch = q::insphere(c).into_iter();
        batch.map(|[a, b, c, d, e]| predicate::insphere(a, b, c, d, e).unwrap())
    };
    // Typical and nearly degenerate: never zero, both signs. Degenerate: always exactly zero.
    for class in [Typical, NearlyDegenerate] {
        let all: [Vec<Sign>; 4] = [
            o2(class).collect(),
            o3(class).collect(),
            ic(class).collect(),
            is(class).collect(),
        ];
        for signs in all {
            assert!(signs.iter().all(|s| !s.is_zero()), "{class}");
            assert!(
                signs.contains(&Sign::Positive) && signs.contains(&Sign::Negative),
                "{class}"
            );
        }
    }
    assert!(o2(Degenerate).all(Sign::is_zero));
    assert!(o3(Degenerate).all(Sign::is_zero));
    assert!(ic(Degenerate).all(Sign::is_zero));
    assert!(is(Degenerate).all(Sign::is_zero));
    // The cospherical inputs have a unique sphere through their first four points.
    for class in [Degenerate, NearlyDegenerate] {
        for [a, b, c, d, _] in q::insphere(class) {
            assert!(!predicate::orient3d(a, b, c, d).unwrap().is_zero());
        }
    }
    // Typical coordinates stay within the typical reach.
    assert_typical(q::orient2d(Typical).iter().flatten().flat_map(coords2));
    assert_typical(q::insphere(Typical).iter().flatten().flat_map(coords3));
    // Extreme coordinates are at the edge of the frame, starting with the worst cases.
    assert_extreme(q::orient2d(Extreme).iter().flatten().flat_map(coords2));
    assert_extreme(q::orient3d(Extreme).iter().flatten().flat_map(coords3));
    assert_extreme(q::incircle(Extreme).iter().flatten().flat_map(coords2));
    assert_extreme(q::insphere(Extreme).iter().flatten().flat_map(coords3));
    let first = q::orient2d(Extreme)[0];
    assert_eq!(first.map(|p| coords2(&p)), [[-M, -M], [M, -M], [M, M]]);
}

type Pairs<const L: usize> = fn(Class) -> Vec<(Wide<L>, Wide<L>)>;
type Checked<const L: usize> = fn(&Wide<L>, &Wide<L>) -> Option<Wide<L>>;

/// Whether the top limb of `w` holds more than sign extension.
fn uses_top_limb<const L: usize>(w: &Wide<L>) -> bool {
    let fill = if w.is_negative() { u64::MAX } else { 0 };
    w.to_le_limbs().last().is_some_and(|&top| top != fill)
}

fn check_wide<const L: usize>() {
    use inputs::wide as w;
    let ops: [(Pairs<L>, Checked<L>); 3] = [
        (w::add::<L>, Wide::checked_add),
        (w::sub::<L>, Wide::checked_sub),
        (w::mul::<L>, Wide::checked_mul),
    ];
    for (build, op) in ops {
        assert!(
            build(Typical).iter().all(|(x, y)| op(x, y).is_some()),
            "typical, L = {L}"
        );
        assert!(
            build(Extreme).iter().all(|(x, y)| op(x, y).is_some()),
            "extreme, L = {L}"
        );
        assert!(
            build(Overflow).iter().all(|(x, y)| op(x, y).is_none()),
            "overflow, L = {L}"
        );
        // Typical operands are i128 values.
        for (x, y) in build(Typical) {
            assert!(x.to_i128().is_some() && y.to_i128().is_some());
        }
    }
    // Extreme sums use every limb; extreme products land in the top limb.
    for (x, y) in w::add::<L>(Extreme) {
        assert!(uses_top_limb(&x) && uses_top_limb(&y));
    }
    for (x, y) in w::mul::<L>(Extreme) {
        assert!(x.checked_mul(&y).is_some_and(|p| uses_top_limb(&p)));
    }
    // Negation: typical and extreme negate, the overflow class is the minimum value only.
    let mut negatable = w::neg::<L>(Typical).into_iter().chain(w::neg::<L>(Extreme));
    assert!(negatable.all(|x| x.checked_neg().is_some()));
    assert!(w::neg::<L>(Extreme).iter().all(uses_top_limb));
    let minimum = Wide::min_value();
    assert!(
        w::neg::<L>(Overflow)
            .iter()
            .all(|x| *x == minimum && x.checked_neg().is_none())
    );
    // Extreme comparisons agree in every limb but the lowest: the longest comparison.
    for (x, y) in w::cmp::<L>(Extreme) {
        let (a, b) = (x.to_le_limbs(), y.to_le_limbs());
        assert_eq!(a[1..], b[1..]);
    }
    assert!(w::cmp::<L>(Extreme).iter().any(|(x, y)| x == y));
    // Signum sees every sign, and positive values with only the top limb set.
    let signs: BTreeSet<Sign> = w::signum::<L>(Typical).iter().map(Wide::signum).collect();
    assert_eq!(signs.len(), 3);
    assert!(w::signum::<L>(Typical).iter().any(|x| {
        let limbs = x.to_le_limbs();
        limbs[..L - 1].iter().all(|&l| l == 0) && limbs[L - 1] != 0
    }));
    // to_i128: typical values fit (the ends of i128 included), overflow values do not.
    assert!(
        w::to_i128::<L>(Typical)
            .iter()
            .all(|x| x.to_i128().is_some())
    );
    assert!(
        w::to_i128::<L>(Typical)
            .iter()
            .any(|x| x.to_i128() == Some(i128::MIN))
    );
    assert!(
        w::to_i128::<L>(Overflow)
            .iter()
            .all(|x| x.to_i128().is_none())
    );
    // from_i128 sees both ends of the type.
    let values = w::from_i128::<L>(Typical);
    assert!(values.contains(&i128::MIN) && values.contains(&i128::MAX));
}

#[test]
fn wide_classes() {
    check_wide::<4>();
    check_wide::<5>();
    check_wide::<6>();
    check_wide::<8>();
    // The builders are generic: their classes hold at the narrowest widths `Wide` allows too, and a
    // class that cannot exist there is empty (typical products need four limbs; at two limbs every
    // value fits an i128).
    check_wide::<2>();
    check_wide::<3>();
    use inputs::wide as w;
    assert!(w::mul::<2>(Typical).is_empty() && w::mul::<3>(Typical).is_empty());
    assert_eq!(w::mul::<4>(Typical).len(), BATCH);
    assert!(w::to_i128::<2>(Overflow).is_empty());
    assert_eq!(w::to_i128::<3>(Overflow).len(), BATCH);
    assert!(
        w::resize::<3, 2>(Typical)
            .iter()
            .all(|x| x.resize::<2>().is_some())
    );
    assert!(
        w::resize::<3, 2>(Overflow)
            .iter()
            .all(|x| x.resize::<2>().is_none())
    );
    assert_eq!(w::resize::<3, 2>(Overflow).len(), BATCH);
    // Widening always fits and narrowing fits only the typical class.
    let widened = |x: &Wide<4>| x.resize::<6>().is_some() && uses_top_limb(x);
    assert!(w::resize::<4, 6>(Typical).iter().all(widened));
    let widened = |x: &Wide<5>| x.resize::<6>().is_some() && uses_top_limb(x);
    assert!(w::resize::<5, 6>(Typical).iter().all(widened));
    assert!(
        w::resize::<6, 4>(Typical)
            .iter()
            .all(|x| x.resize::<4>().is_some())
    );
    assert!(
        w::resize::<6, 4>(Overflow)
            .iter()
            .all(|x| x.resize::<4>().is_none())
    );
}

/// The index of the highest limb of `w` that holds more than sign extension, if any.
fn highest_used_limb<const L: usize>(w: &Wide<L>) -> Option<usize> {
    let fill = if w.is_negative() { u64::MAX } else { 0 };
    w.to_le_limbs().iter().rposition(|&limb| limb != fill)
}

#[test]
fn growth_inputs_use_every_limb_at_every_size() {
    use inputs::growth as g;
    fn check<const L: usize>() {
        let both = |(x, y): &(Wide<L>, Wide<L>)| uses_top_limb(x) && uses_top_limb(y);
        assert!(
            g::add::<L>()
                .iter()
                .all(|p @ (x, y)| x.checked_add(y).is_some() && both(p))
        );
        assert!(
            g::sub::<L>()
                .iter()
                .all(|p @ (x, y)| x.checked_sub(y).is_some() && both(p))
        );
        assert!(
            g::neg::<L>()
                .iter()
                .all(|x| x.checked_neg().is_some() && uses_top_limb(x))
        );
        // Products: operands in the lower half of the limbs (32L − 1 bits, so their highest limb
        // is L/2 − 1), products that reach the top limb.
        let full =
            |(x, y): &(Wide<L>, Wide<L>)| x.checked_mul(y).is_some_and(|p| uses_top_limb(&p));
        assert!(g::mul::<L>().iter().all(full));
        let half = |(x, y): &(Wide<L>, Wide<L>)| {
            highest_used_limb(x) == Some(L / 2 - 1) && highest_used_limb(y) == Some(L / 2 - 1)
        };
        assert!(g::mul::<L>().iter().all(half));
        assert!(
            g::cmp::<L>()
                .iter()
                .all(|p @ (x, y)| both(p) && x.to_le_limbs()[1..] == y.to_le_limbs()[1..])
        );
        // Distinct from the per-width benchmarks of the same width.
        assert_ne!(g::add::<L>(), inputs::wide::add::<L>(Extreme));
    }
    check::<2>();
    check::<4>();
    check::<8>();
}

/// Whether a plane has a normal component near its bound: at least 2^80, against 2^83.
fn spans(p: &Plane) -> bool {
    p.n().iter().any(|n| n.unsigned_abs() >= 1 << 80)
}

/// n · p + d, exactly (|n_i| < 2^83 and |p_i| < 2^40, so it fits i128).
fn plane_value(plane: &Plane, p: P3) -> i128 {
    let [nx, ny, nz] = plane.n();
    nx * i128::from(p.x.get()) + ny * i128::from(p.y.get()) + nz * i128::from(p.z.get()) + plane.d()
}

/// Whether two planes have parallel normals.
fn parallel(a: &Plane, b: &Plane) -> bool {
    let (n, m) = (a.n().map(w), b.n().map(w));
    let cross = |i: usize, j: usize| {
        let a = n[i].checked_mul(&m[j]).unwrap();
        let b = n[j].checked_mul(&m[i]).unwrap();
        a.checked_sub(&b).unwrap()
    };
    cross(1, 2).is_zero() && cross(2, 0).is_zero() && cross(0, 1).is_zero()
}

#[test]
fn plane_classes() {
    use inputs::plane as p;
    let through = |[a, b, c]: [P3; 3]| Plane::through(a, b, c);
    assert!(p::through(Typical).into_iter().all(|t| through(t).is_ok()));
    assert!(
        p::through(Extreme)
            .into_iter()
            .all(|t| through(t).is_ok_and(|q| spans(&q)))
    );
    assert!(
        p::through(Degenerate)
            .into_iter()
            .all(|t| through(t) == Err(NumError::Degenerate))
    );
    assert_extreme(p::through(Extreme).iter().flatten().flat_map(coords3));
    // The first extreme plane's normal components are all 4M², the frame's largest.
    let corner = through(p::through(Extreme)[0]).unwrap();
    let m = i128::from(M);
    assert!(
        corner
            .n()
            .iter()
            .all(|n| n.unsigned_abs() == (4 * m * m).unsigned_abs())
    );

    assert!(
        p::side(Typical)
            .iter()
            .all(|(q, r)| !q.side(*r).unwrap().is_zero())
    );
    assert!(
        p::side(Degenerate)
            .iter()
            .all(|(q, r)| q.side(*r).unwrap().is_zero())
    );
    assert!(
        p::side(NearlyDegenerate)
            .iter()
            .all(|(q, r)| plane_value(q, *r).abs() == 1)
    );
    assert!(p::side(Extreme).iter().all(|(q, _)| spans(q)));

    let meet = |[a, b, c]: [Plane; 3]| Tpi::new(a, b, c);
    assert!(p::tpi_new(Typical).into_iter().all(|t| meet(t).is_ok()));
    assert!(p::tpi_new(Extreme).into_iter().all(|t| meet(t).is_ok()));
    let degenerate = p::tpi_new(Degenerate);
    assert!(
        degenerate
            .iter()
            .all(|&t| meet(t) == Err(NumError::Degenerate))
    );
    // Every documented way to fail: parallel with equal normals, parallel with unequal normals,
    // three planes through one line (no two parallel), a plane given twice.
    let pairs = |[a, b, c]: &[Plane; 3]| [(*a, *b), (*b, *c), (*a, *c)];
    let any_pair = |test: &dyn Fn(&Plane, &Plane) -> bool| {
        degenerate
            .iter()
            .any(|t| pairs(t).iter().any(|(x, y)| test(x, y)))
    };
    assert!(any_pair(&|x, y| parallel(x, y) && x.n() == y.n() && x != y));
    assert!(any_pair(&|x, y| parallel(x, y) && x.n() != y.n()));
    assert!(
        degenerate
            .iter()
            .any(|t| pairs(t).iter().all(|(x, y)| !parallel(x, y)))
    );
    assert!(
        degenerate
            .iter()
            .any(|t| pairs(t).iter().any(|(x, y)| x == y))
    );

    assert!(
        p::side_of_tpi(Typical)
            .iter()
            .all(|(q, t)| !q.side_of_tpi(t).unwrap().is_zero())
    );
    assert!(
        p::side_of_tpi(Degenerate)
            .iter()
            .all(|(q, t)| q.side_of_tpi(t).unwrap().is_zero())
    );
    for (q, t) in p::side_of_tpi(NearlyDegenerate) {
        // The meeting point is a grid point one unit of n · p + d off the plane.
        let at = t.to_grid().unwrap();
        assert!(at.is_exact());
        assert_eq!(plane_value(&q, at.value()).abs(), 1);
        assert!(!q.side_of_tpi(&t).unwrap().is_zero());
    }
    for (q, t) in p::side_of_tpi(Extreme) {
        assert!(spans(&q) && t.planes().iter().all(spans));
    }

    assert!(p::to_grid(Typical).iter().all(|t| t.to_grid().is_ok()));
    assert!(p::to_grid(Extreme).iter().all(|t| t.to_grid().is_ok()));
    assert!(
        p::to_grid(OnGrid)
            .iter()
            .all(|t| t.to_grid().is_ok_and(|r| r.is_exact()))
    );
    for t in p::to_grid(Overflow) {
        // Beyond the frame, but short of the saturated values: the 127-bit division path.
        let Err(NumError::OutOfRange { value }) = t.to_grid() else {
            panic!("an overflow input landed in the frame");
        };
        assert!(value.unsigned_abs() > M as u128 && value.unsigned_abs() < 1 << 127);
    }
    // Committing typical points rounds most of them.
    let rounded = p::to_grid(Typical)
        .iter()
        .filter(|t| t.to_grid().is_ok_and(|r| !r.is_exact()))
        .count();
    assert!(rounded > BATCH / 2);
}

/// The paths of the error-free transformations, as `geom_num::eft` documents them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Path {
    /// The published algorithm, unchanged.
    Direct,
    /// `two_sum`: an intermediate overflowed, and `fast_two_sum` gave the pair.
    Fallback,
    /// `fast_two_sum`: the operand of larger magnitude came second and was moved first.
    Swapped,
    /// The rounded sum or product is not finite: `(s, −s)` or `(p, −p)`.
    Overflow,
    /// `two_prod`: a zero factor, `(p, 0)`.
    ZeroFactor,
    /// `split` and `two_prod`: the input was scaled by a power of two first, and the result is exact.
    Scaled,
    /// `split` and `two_prod`: no double holds the exact result, so it is reported as an infinity.
    Reported,
}

/// The path `two_sum(a, b)` takes: Knuth's six operations, unless one of them overflows.
fn two_sum_path(a: f64, b: f64) -> Path {
    let s = a + b;
    let b_virtual = s - a;
    let a_virtual = s - b_virtual;
    let e = (a - a_virtual) + (b - b_virtual);
    if !s.is_finite() {
        Path::Overflow
    } else if !e.is_finite() {
        Path::Fallback
    } else {
        Path::Direct
    }
}

/// The path `fast_two_sum(a, b)` takes.
fn fast_two_sum_path(a: f64, b: f64) -> Path {
    if !(a + b).is_finite() {
        Path::Overflow
    } else if b.abs() > a.abs() {
        Path::Swapped
    } else {
        Path::Direct
    }
}

/// The path `split(a)` takes: Veltkamp's method below 2^996, scaled by 2^−64 from there.
fn split_path(a: f64) -> Path {
    if a.abs() < pow2(996) {
        Path::Direct
    } else if eft::split(a).0.is_finite() {
        Path::Scaled
    } else {
        Path::Reported
    }
}

/// The path `two_prod(a, b)` takes: Dekker's method for factors below 2^996 whose product lies in
/// [2^−968, 2^1022), and the scaled path for every other finite product.
fn two_prod_path(a: f64, b: f64) -> Path {
    let p = a * b;
    let size = p.abs();
    let small = a.abs() < pow2(996) && b.abs() < pow2(996);
    if !p.is_finite() {
        Path::Overflow
    } else if small && size >= pow2(-968) && size < pow2(1022) {
        Path::Direct
    } else if a == 0.0 || b == 0.0 {
        Path::ZeroFactor
    } else if eft::two_prod(a, b).1.is_finite() {
        Path::Scaled
    } else {
        Path::Reported
    }
}

#[test]
fn eft_classes_take_every_path() {
    use Path::{Direct, Fallback, Overflow, Reported, Scaled, Swapped, ZeroFactor};
    use inputs::eft as e;
    let paths = |found: Vec<Path>| found.into_iter().collect::<BTreeSet<Path>>();
    // Typical inputs take the direct path, in either order for fast_two_sum.
    let typical = |found: Vec<Path>| paths(found) == BTreeSet::from([Direct]);
    assert!(typical(
        e::two_sum(Typical)
            .into_iter()
            .map(|(a, b)| two_sum_path(a, b))
            .collect()
    ));
    assert!(typical(
        e::split(Typical).into_iter().map(split_path).collect()
    ));
    assert!(typical(
        e::two_prod(Typical)
            .into_iter()
            .map(|(a, b)| two_prod_path(a, b))
            .collect()
    ));
    let fast = paths(
        e::fast_two_sum(Typical)
            .into_iter()
            .map(|(a, b)| fast_two_sum_path(a, b))
            .collect(),
    );
    assert_eq!(fast, BTreeSet::from([Direct, Swapped]));
    // Extreme inputs take every path each function has.
    let two_sum: Vec<Path> = e::two_sum(Extreme)
        .into_iter()
        .map(|(a, b)| two_sum_path(a, b))
        .collect();
    assert_eq!(paths(two_sum), BTreeSet::from([Direct, Fallback, Overflow]));
    let fast: Vec<Path> = e::fast_two_sum(Extreme)
        .into_iter()
        .map(|(a, b)| fast_two_sum_path(a, b))
        .collect();
    assert_eq!(paths(fast), BTreeSet::from([Direct, Swapped, Overflow]));
    let split: Vec<Path> = e::split(Extreme).into_iter().map(split_path).collect();
    assert_eq!(paths(split), BTreeSet::from([Direct, Scaled, Reported]));
    let prod: Vec<Path> = e::two_prod(Extreme)
        .into_iter()
        .map(|(a, b)| two_prod_path(a, b))
        .collect();
    assert_eq!(
        paths(prod),
        BTreeSet::from([Direct, Overflow, ZeroFactor, Scaled, Reported])
    );
    // Both edges of the direct paths: subnormals, ±0, the smallest normal exponent, the documented
    // fallback input of two_sum, and products just past each end of Dekker's range.
    assert!(e::two_sum(Extreme).iter().any(|(a, _)| a.is_subnormal()));
    assert!(
        e::two_sum(Extreme)
            .iter()
            .any(|&(a, b)| a == -3.0 * pow2(970) && b == f64::MAX)
    );
    let splits = e::split(Extreme);
    assert!(splits.iter().any(|a| a.is_subnormal()));
    assert!(
        splits.iter().any(|&a| a.to_bits() == 0)
            && splits.iter().any(|&a| a.to_bits() == (-0.0f64).to_bits())
    );
    assert!(splits.iter().any(|&a| exponent(a) == -1022));
    let products = e::two_prod(Extreme);
    let scaled_below =
        |&(a, b): &(f64, f64)| two_prod_path(a, b) == Scaled && (a * b).abs() < pow2(-968);
    let scaled_above =
        |&(a, b): &(f64, f64)| two_prod_path(a, b) == Scaled && (a * b).abs() >= pow2(1022);
    assert!(products.iter().any(scaled_below) && products.iter().any(scaled_above));
}

/// The kind of a typical interval: 0 positive, 1 negative, 2 around zero.
fn kind_of(x: &Interval) -> u8 {
    if x.lo() > 0.0 {
        0
    } else if x.hi() < 0.0 {
        1
    } else {
        2
    }
}

/// A builder of a batch of interval pairs.
type IntervalPairs = fn(Class) -> Vec<(Interval, Interval)>;

/// A binary interval operation.
type IntervalOp = fn(Interval, Interval) -> Result<Interval, NumError>;

#[test]
fn interval_classes() {
    use inputs::interval as i;
    let ops: [(&str, IntervalPairs, IntervalOp); 4] = [
        ("add", i::add, Interval::add),
        ("sub", i::sub, Interval::sub),
        ("mul", i::mul, Interval::mul),
        ("div", i::div, Interval::div),
    ];
    for (name, build, op) in ops {
        assert!(
            build(Typical).into_iter().all(|(x, y)| op(x, y).is_ok()),
            "{name}"
        );
        let extreme: Vec<_> = build(Extreme).into_iter().map(|(x, y)| op(x, y)).collect();
        assert!(extreme.iter().any(Result::is_ok), "{name}");
        assert!(extreme.iter().any(Result::is_err), "{name}");
    }
    let quotients: Vec<_> = i::div(Extreme).into_iter().map(|(x, y)| x.div(y)).collect();
    assert!(quotients.contains(&Err(NumError::DivisorContainsZero)));
    let sums: Vec<_> = i::add(Extreme).into_iter().map(|(x, y)| x.add(y)).collect();
    assert!(sums.contains(&Err(NumError::NotFinite)));
    assert!(i::sqrt(Typical).into_iter().all(|x| x.sqrt().is_ok()));
    let roots: Vec<_> = i::sqrt(Extreme).into_iter().map(Interval::sqrt).collect();
    assert!(roots.contains(&Err(NumError::NegativeSqrt)) && roots.iter().any(Result::is_ok));
    // square: the typical rotation takes each branch (above, below and around zero); the extreme
    // class overflows, holds an underflowing lower square at exactly 0, and has zero bounds.
    let branch = |x: &Interval| {
        if x.lo() >= 0.0 {
            0u8
        } else if x.hi() <= 0.0 {
            1
        } else {
            2
        }
    };
    let typical = i::square(Typical);
    assert!(typical.iter().all(|x| x.square().is_ok()));
    let branches: BTreeSet<u8> = typical.iter().map(branch).collect();
    assert_eq!(branches.len(), 3);
    let extreme = i::square(Extreme);
    assert!(
        extreme
            .iter()
            .any(|x| x.square() == Err(NumError::NotFinite))
    );
    let zero_lower = |x: &Interval| x.square().is_ok_and(|r| r.lo().to_bits() == 0);
    let near = |x: &Interval| if x.lo() > 0.0 { x.lo() } else { -x.hi() };
    assert!(
        extreme
            .iter()
            .any(|x| branch(x) != 2 && near(x) != 0.0 && near(x) * near(x) == 0.0 && zero_lower(x))
    );
    assert!(
        extreme
            .iter()
            .any(|x| (x.lo() == 0.0 || x.hi() == 0.0) && zero_lower(x))
    );
    assert!(
        extreme
            .iter()
            .any(|x| x.square().is_ok_and(|r| r.hi() > pow2(1000)))
    );
    // mul_add: typical triples all succeed; the extreme class overflows (NotFinite), succeeds, and
    // has the cancelling case: point factors a and b, and an addend whose lower bound is −(a · b)
    // rounded.
    let typical: Vec<_> = i::mul_add(Typical)
        .into_iter()
        .map(|(a, b, c)| a.mul_add(b, c))
        .collect();
    assert!(typical.iter().all(Result::is_ok));
    let extreme = i::mul_add(Extreme);
    let made: Vec<_> = extreme.iter().map(|&(a, b, c)| a.mul_add(b, c)).collect();
    assert!(made.contains(&Err(NumError::NotFinite)) && made.iter().any(Result::is_ok));
    assert!(
        extreme.iter().any(|(a, b, c)| {
            a.lo() == a.hi() && b.lo() == b.hi() && c.lo() == -(a.lo() * b.lo())
        })
    );
    // norm: typical vectors all have a length; the extreme class has a square that overflows, a
    // sum of finite squares that overflows, a sum of squares that widening takes below zero whose
    // length starts at exactly 0 (subnormal components, and components around zero), and large
    // finite lengths.
    assert!(i::norm(Typical).iter().all(|v| Interval::norm(v).is_ok()));
    let extreme = i::norm(Extreme);
    let squares_ok = |v: &[Interval; 3]| v.iter().all(|c| c.square().is_ok());
    let sum_lo = |v: &[Interval; 3]| {
        let squares: Vec<Interval> = v.iter().map(|c| c.square().unwrap()).collect();
        let sum = squares[0].add(squares[1]).unwrap().add(squares[2]).unwrap();
        sum.lo()
    };
    assert!(
        extreme
            .iter()
            .any(|v| !squares_ok(v) && Interval::norm(v) == Err(NumError::NotFinite))
    );
    assert!(
        extreme
            .iter()
            .any(|v| squares_ok(v) && Interval::norm(v) == Err(NumError::NotFinite))
    );
    let cut = |v: &[Interval; 3]| {
        Interval::norm(v).is_ok_and(|r| r.lo().to_bits() == 0) && sum_lo(v) < 0.0
    };
    assert!(
        extreme
            .iter()
            .any(|v| cut(v) && v.iter().all(|c| c.lo() > 0.0 && c.hi() < f64::MIN_POSITIVE))
    );
    assert!(
        extreme
            .iter()
            .any(|v| cut(v) && v.iter().all(|c| c.lo() < 0.0 && c.hi() > 0.0))
    );
    assert!(
        extreme
            .iter()
            .any(|v| Interval::norm(v).is_ok_and(|r| r.hi() > pow2(500)))
    );
    // Typical triples come in rotation: every triple of kinds occurs, for mul_add and norm alike.
    let triple = |[a, b, c]: [&Interval; 3]| (kind_of(a), kind_of(b), kind_of(c));
    let triples: BTreeSet<_> = i::mul_add(Typical)
        .iter()
        .map(|(a, b, c)| triple([a, b, c]))
        .collect();
    assert_eq!(triples.len(), 27);
    let triples: BTreeSet<_> = i::norm(Typical)
        .iter()
        .map(|[a, b, c]| triple([a, b, c]))
        .collect();
    assert_eq!(triples.len(), 27);
    // Typical operands come in rotation: positive, negative, around zero.
    let kinds: Vec<u8> = i::add(Typical).iter().map(|(x, _)| kind_of(x)).collect();
    assert!(
        kinds
            .iter()
            .enumerate()
            .all(|(n, &k)| usize::from(k) == n % 3),
        "{kinds:?}"
    );
    // new: valid pairs succeed; the extreme class has each error and successes.
    assert!(
        i::new(Typical)
            .into_iter()
            .all(|(lo, hi)| Interval::new(lo, hi).is_ok())
    );
    let made: Vec<_> = i::new(Extreme)
        .into_iter()
        .map(|(lo, hi)| Interval::new(lo, hi))
        .collect();
    assert!(made.contains(&Err(NumError::NotFinite)));
    assert!(made.contains(&Err(NumError::EmptyInterval)));
    assert!(made.iter().any(Result::is_ok));
    // contains: values inside and outside, and on both bounds.
    let found: Vec<bool> = i::contains(Typical)
        .iter()
        .map(|&(x, v)| x.contains(v))
        .collect();
    assert!(found.contains(&true) && found.contains(&false));
    assert!(i::contains(Typical).iter().any(|&(x, v)| v == x.lo()));
    assert!(i::contains(Typical).iter().any(|&(x, v)| v == x.hi()));
    // certain_sign takes every branch.
    let signs: Vec<_> = i::certain_sign(Typical)
        .into_iter()
        .map(Interval::certain_sign)
        .collect();
    for expected in [
        Some(Sign::Positive),
        Some(Sign::Negative),
        Some(Sign::Zero),
        None,
    ] {
        assert!(signs.contains(&expected), "{expected:?}");
    }
}

#[test]
fn grid_classes() {
    use inputs::grid as g;
    for (c, from, to) in g::refine(Typical) {
        assert!(grid::refine(c, from, to).is_ok());
    }
    for (c, from, to) in g::refine(Overflow) {
        assert!(matches!(
            grid::refine(c, from, to),
            Err(NumError::OutOfRange { .. })
        ));
    }
    for (c, from, to) in g::refine(WrongDirection) {
        assert_eq!(grid::refine(c, from, to), Err(NumError::WrongDirection));
    }
    for class in [Typical, Extreme] {
        for (c, from, to) in g::coarsen(class) {
            assert!(grid::coarsen(c, from, to).is_ok());
        }
    }
    for (c, from, to) in g::coarsen(WrongDirection) {
        assert_eq!(grid::coarsen(c, from, to), Err(NumError::WrongDirection));
    }
    assert_extreme(g::coarsen(Extreme).iter().map(|(c, _, _)| c.get()));
    // Typical batches cover every pair of steps.
    let pairs: BTreeSet<_> = g::refine(Typical)
        .iter()
        .map(|(_, f, t)| (f.rung(), t.rung()))
        .collect();
    assert_eq!(pairs.len(), 6);
    // Steps::refine: typical counts fit every finer step, the same step included; overflow counts
    // do not fit the finer step; the wrong direction is refused.
    assert!(
        g::steps_refine(Typical)
            .iter()
            .all(|&(s, from, to)| s.refine(from, to).is_some())
    );
    let pairs: BTreeSet<_> = g::steps_refine(Typical)
        .iter()
        .map(|(_, f, t)| (f.rung(), t.rung()))
        .collect();
    assert_eq!(pairs.len(), 6);
    assert!(
        g::steps_refine(Overflow)
            .iter()
            .all(|&(s, from, to)| from.rung() < to.rung() && s.refine(from, to).is_none())
    );
    assert!(
        g::steps_refine(WrongDirection)
            .iter()
            .all(|&(s, from, to)| from.rung() > to.rung() && s.refine(from, to).is_none())
    );
    // GridStep::steps_per: every step against every named unit, and units that are a whole number
    // of steps and units that are not.
    let per = g::steps_per(Typical);
    let units = [
        grid::QUARTER_NM_PER_MM,
        grid::QUARTER_NM_PER_INCH,
        grid::QUARTER_NM_PER_MICRON,
        grid::QUARTER_NM_PER_MIL,
        grid::QUARTER_NM_PER_NM,
    ];
    for step in [GridStep::Q0, GridStep::Q1, GridStep::Q2] {
        for unit in units {
            assert!(per.contains(&(step, unit)), "{step:?} {unit}");
        }
    }
    assert!(per.iter().any(|&(s, u)| s.steps_per(u).is_some()));
    assert!(per.iter().any(|&(s, u)| s.steps_per(u).is_none()));
    // Coordinates: inside the frame (±M included) succeed; outside fail, ±2^40 included.
    assert!(
        g::coord_new(Typical)
            .iter()
            .all(|&v| grid::Coord::new(v).is_ok())
    );
    assert!(g::coord_new(Typical).contains(&M) && g::coord_new(Typical).contains(&-M));
    assert!(
        g::coord_new(Overflow)
            .iter()
            .all(|&v| grid::Coord::new(v).is_err())
    );
    assert!(g::coord_new(Overflow).iter().any(|&v| v.abs() == M + 1));
    assert!(
        g::coord_from_i128(Typical)
            .iter()
            .all(|&v| grid::Coord::from_i128(v).is_ok())
    );
    assert!(
        g::coord_from_i128(Overflow)
            .iter()
            .all(|&v| grid::Coord::from_i128(v).is_err())
    );
    assert!(
        g::p2_new(Typical)
            .iter()
            .all(|&(x, y)| P2::new(x, y).is_ok())
    );
    assert!(
        g::p2_new(Overflow)
            .iter()
            .all(|&(x, y)| P2::new(x, y).is_err())
    );
    assert!(
        g::p3_new(Typical)
            .iter()
            .all(|&(x, y, z)| P3::new(x, y, z).is_ok())
    );
    assert!(
        g::p3_new(Overflow)
            .iter()
            .all(|&(x, y, z)| P3::new(x, y, z).is_err())
    );
    // Each position of a point is the bad one in turn.
    for position in 0..3 {
        assert!(
            g::p3_new(Overflow)
                .iter()
                .any(|&(x, y, z)| [x, y, z][position].abs() > M)
        );
    }
}

#[test]
fn scalar_classes() {
    use inputs::{exactness as e, real as r, sign as s};
    let signs: BTreeSet<Sign> = s::of_i128(Typical).into_iter().map(Sign::of_i128).collect();
    assert_eq!(signs.len(), 3);
    assert!(s::of_i128(Typical).contains(&i128::MIN) && s::of_i128(Typical).contains(&i128::MAX));
    // Total order: both zeros in both orders, equal values, and both orderings.
    let pairs = r::cmp(Typical);
    let bits = |x: geom_num::real::Real| x.get().to_bits();
    assert!(
        pairs
            .iter()
            .any(|&(x, y)| bits(x) == 0 && bits(y) == (-0.0f64).to_bits())
    );
    assert!(
        pairs
            .iter()
            .any(|&(x, y)| bits(y) == 0 && bits(x) == (-0.0f64).to_bits())
    );
    let orders: BTreeSet<_> = pairs.iter().map(|(x, y)| x.cmp(y)).collect();
    assert_eq!(orders.len(), 3);
    // Exactness::combine: all nine pairings; bounded pairs on one step and on different steps;
    // sums that fit and sums that do not (None).
    let pairs = e::combine(Typical);
    let kinds: BTreeSet<(u8, u8)> = pairs.iter().map(|(x, y)| (x.rank(), y.rank())).collect();
    assert_eq!(kinds.len(), 9);
    let steps = |x: &Exactness, y: &Exactness| x.bound().zip(y.bound()).map(|(a, b)| (a.1, b.1));
    assert!(
        pairs
            .iter()
            .any(|(x, y)| steps(x, y).is_some_and(|(a, b)| a == b))
    );
    assert!(
        pairs
            .iter()
            .any(|(x, y)| steps(x, y).is_some_and(|(a, b)| a != b))
    );
    let sums: Vec<Option<Exactness>> = pairs.iter().map(|(x, y)| x.combine(*y)).collect();
    assert!(sums.contains(&None));
    assert!(sums.iter().any(|s| s.and_then(Exactness::bound).is_some()));
    // Exactness::within: for certified and approximate classes, the bound below the ceiling, at
    // it and above it, compared exactly in the finer step, and a bound too large to convert.
    let mut seen = BTreeSet::new();
    for &(x, ceiling, step) in &e::within(Typical) {
        let Some((bound, bound_step)) = x.bound() else {
            assert_eq!(x.within(ceiling, step), Ok(x));
            seen.insert((0u8, Ordering3::Exact));
            continue;
        };
        let place = place(bound, bound_step, ceiling, step);
        seen.insert((x.rank(), place));
    }
    for rank in [1u8, 2] {
        for place in [
            Ordering3::Below,
            Ordering3::At,
            Ordering3::Above,
            Ordering3::DoesNotFit,
        ] {
            assert!(
                seen.contains(&(rank, place)),
                "within: rank {rank}, {place:?}"
            );
        }
    }
    assert!(seen.contains(&(0, Ordering3::Exact)));
    // Exactness::combine_within: every pairing; a combined bound below, at and above the ceiling;
    // and pairs whose combined bound does not fit.
    let mut seen = BTreeSet::new();
    let quads = e::combine_within(Typical);
    for &(x, y, ceiling, step) in &quads {
        let place = match x.combine(y) {
            None => Ordering3::DoesNotFit,
            Some(z) => match z.bound() {
                None => Ordering3::Exact,
                Some((bound, bound_step)) => place(bound, bound_step, ceiling, step),
            },
        };
        seen.insert(place);
    }
    assert_eq!(seen.len(), 5, "{seen:?}");
    let kinds: BTreeSet<(u8, u8)> = quads
        .iter()
        .map(|(x, y, _, _)| (x.rank(), y.rank()))
        .collect();
    assert_eq!(kinds.len(), 9);
}

/// Where a bound lies against a ceiling, compared exactly in the finer of their steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Ordering3 {
    /// No bound: the class is exact.
    Exact,
    Below,
    At,
    Above,
    /// The bound does not fit a `u64` count of the finer step.
    DoesNotFit,
}

/// `count` steps of `from` in steps of `to`, a step at least as fine: 25 per rung, exactly.
fn in_step(count: u64, from: GridStep, to: GridStep) -> u128 {
    u128::from(count) * 25u128.pow(to.rung() - from.rung())
}

/// Where `bound` steps of `bound_step` lie against `ceiling` steps of `step`.
fn place(bound: Steps, bound_step: GridStep, ceiling: Steps, step: GridStep) -> Ordering3 {
    let finer = if bound_step.rung() >= step.rung() {
        bound_step
    } else {
        step
    };
    let b = in_step(bound.get(), bound_step, finer);
    let c = in_step(ceiling.get(), step, finer);
    if b > u128::from(u64::MAX) {
        Ordering3::DoesNotFit
    } else if b < c {
        Ordering3::Below
    } else if b == c {
        Ordering3::At
    } else {
        Ordering3::Above
    }
}

/// A builder of a batch of doubles.
type Doubles = fn(Class) -> Vec<f64>;

/// A builder of a batch of pairs of doubles.
type DoublePairs = fn(Class) -> Vec<(f64, f64)>;

/// A maths function of one argument.
type Unary = fn(f64) -> Result<Real, NumError>;

/// A maths function of two arguments.
type Binary = fn(f64, f64) -> Result<Real, NumError>;

/// A maths result from an argument in the function's domain: a `Real`, or `NotFinite` because the
/// result overflows, never `OutsideDomain`.
fn in_domain(r: Result<Real, NumError>) -> bool {
    matches!(r, Ok(_) | Err(NumError::NotFinite))
}

#[test]
fn maths_inputs_stay_in_each_domain() {
    use geom_num::math as f;
    use inputs::math as m;
    let unary: [(&str, Doubles, Unary); 18] = [
        ("sqrt", m::sqrt, f::sqrt),
        ("sin", m::sin, f::sin),
        ("cos", m::cos, f::cos),
        ("tan", m::tan, f::tan),
        ("asin", m::asin, f::asin),
        ("acos", m::acos, f::acos),
        ("atan", m::atan, f::atan),
        ("exp", m::exp, f::exp),
        ("ln", m::ln, f::ln),
        ("log2", m::log2, f::log2),
        ("log10", m::log10, f::log10),
        ("cbrt", m::cbrt, f::cbrt),
        ("abs", m::abs, f::abs),
        ("floor", m::floor, f::floor),
        ("ceil", m::ceil, f::ceil),
        ("round_ties_even", m::round_ties_even, f::round_ties_even),
        ("sinpi", m::sinpi, f::sinpi),
        ("cospi", m::cospi, f::cospi),
    ];
    for (name, build, func) in unary {
        for class in [Typical, Extreme] {
            for x in build(class) {
                assert!(
                    x.is_finite() && in_domain(func(x)),
                    "{name}({x:e}), {class}"
                );
            }
        }
        // Ordinary arguments never overflow.
        assert!(
            build(Typical).into_iter().all(|x| func(x).is_ok()),
            "{name}"
        );
    }
    let binary: [(&str, DoublePairs, Binary); 3] = [
        ("atan2", m::atan2, f::atan2),
        ("pow", m::pow, f::pow),
        ("hypot", m::hypot, f::hypot),
    ];
    for (name, build, func) in binary {
        for class in [Typical, Extreme] {
            for (x, y) in build(class) {
                assert!(x.is_finite() && y.is_finite(), "{name}({x:e}, {y:e})");
                assert!(in_domain(func(x, y)), "{name}({x:e}, {y:e})");
            }
        }
        assert!(
            build(Typical).into_iter().all(|(x, y)| func(x, y).is_ok()),
            "{name}"
        );
    }
    for class in [Typical, Extreme] {
        for (x, n) in m::powi(class) {
            assert!(
                x.is_finite() && in_domain(math::powi(x, n)),
                "powi({x:e}, {n})"
            );
        }
        for (a, b, c) in m::fma(class) {
            let finite = a.is_finite() && b.is_finite() && c.is_finite();
            assert!(
                finite && in_domain(math::fma(a, b, c)),
                "fma({a:e}, {b:e}, {c:e})"
            );
        }
    }
    assert!(
        m::powi(Typical)
            .into_iter()
            .all(|(x, n)| math::powi(x, n).is_ok())
    );
    assert!(
        m::fma(Typical)
            .into_iter()
            .all(|(a, b, c)| math::fma(a, b, c).is_ok())
    );
    // Past overflow, where the documentation says so: NotFinite, beside finite results.
    let overflows = |results: Vec<Result<Real, NumError>>| {
        results.contains(&Err(NumError::NotFinite)) && results.iter().any(Result::is_ok)
    };
    assert!(overflows(
        m::exp(Extreme).into_iter().map(math::exp).collect()
    ));
    assert!(overflows(
        m::powi(Extreme)
            .into_iter()
            .map(|(x, n)| math::powi(x, n))
            .collect()
    ));
    assert!(overflows(
        m::fma(Extreme)
            .into_iter()
            .map(|(a, b, c)| math::fma(a, b, c))
            .collect()
    ));
    // powi: both ends of the exponent's type, and both signs of base in the typical class.
    let powers = m::powi(Extreme);
    assert!(
        powers.iter().any(|&(_, n)| n == i32::MAX) && powers.iter().any(|&(_, n)| n == i32::MIN)
    );
    assert!(m::powi(Typical).iter().any(|&(x, _)| x < 0.0));
    // fma: the addend cancels the rounded product exactly, so the result is the product's
    // rounding error, which the single rounding keeps.
    assert!(
        m::fma(Extreme)
            .iter()
            .any(|&(a, b, c)| c == -(a * b) && math::fma(a, b, c).is_ok_and(|r| r.get() != 0.0))
    );
    // The extreme sines need the longest argument reduction.
    assert!(m::sin(Extreme).iter().any(|&x| x.abs() > pow2(1000)));
    // A quarter of the typical half-turn inputs take the exact branch.
    let exact = m::sinpi(Typical)
        .iter()
        .filter(|&&x| math::floor(x * 2.0).is_ok_and(|f| f.get() == x * 2.0))
        .count();
    assert!(exact >= BATCH / 4);
}
