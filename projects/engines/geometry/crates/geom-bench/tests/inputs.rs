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
use geom_num::wide32::Wide32;
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
    assert!(inputs::math::unproven::sin(OnGrid).is_empty());
    assert!(inputs::math::unproven::sin(WrongDirection).is_empty());
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

/// The `Wide32` batches (piece G42) are the `Wide` classes at the same bit widths: typical and
/// extreme operations succeed, overflow ones fail, and `product_i64`'s classes are what they claim.
fn check_wide32<const L: usize, const M: usize>() {
    use inputs::wide32 as w;
    for (x, y) in w::add::<L, M>(Typical)
        .iter()
        .chain(&w::add::<L, M>(Extreme))
    {
        assert!(x.checked_add(y).is_some(), "add, M = {M}");
    }
    assert!(
        w::add::<L, M>(Overflow)
            .iter()
            .all(|(x, y)| x.checked_add(y).is_none())
    );
    for (x, y) in w::sub::<L, M>(Typical)
        .iter()
        .chain(&w::sub::<L, M>(Extreme))
    {
        assert!(x.checked_sub(y).is_some(), "sub, M = {M}");
    }
    assert!(
        w::sub::<L, M>(Overflow)
            .iter()
            .all(|(x, y)| x.checked_sub(y).is_none())
    );
    let product = |(x, y): &(Wide32<M>, Wide32<M>)| Wide32::<M>::product(x, y);
    assert!(
        w::product::<L, M>(Typical)
            .iter()
            .all(|p| product(p).is_some())
    );
    assert!(
        w::product::<L, M>(Extreme)
            .iter()
            .all(|p| product(p).is_some())
    );
    assert!(
        w::product::<L, M>(Overflow)
            .iter()
            .all(|p| product(p).is_none())
    );
    assert!(!w::product::<L, M>(Extreme).is_empty());
    assert!(
        w::neg::<L, M>(Overflow)
            .iter()
            .all(|x| x.checked_neg().is_none())
    );
    let signs: BTreeSet<Sign> = w::signum::<L, M>(Typical)
        .iter()
        .map(Wide32::signum)
        .collect();
    assert_eq!(signs.len(), 3);
    assert!(
        w::to_i128::<L, M>(Typical)
            .iter()
            .all(|x| x.to_i128().is_some())
    );
    assert!(
        w::to_i128::<L, M>(Overflow)
            .iter()
            .all(|x| x.to_i128().is_none())
    );
    let values = w::from_i128::<M>(Typical);
    assert!(values.contains(&i128::MIN) && values.contains(&i128::MAX));
}

#[test]
fn wide32_classes() {
    check_wide32::<4, 8>();
    check_wide32::<5, 10>();
    check_wide32::<6, 12>();
    check_wide32::<8, 16>();
    use inputs::wide32 as w;
    assert!(
        w::resize::<6, 4, 12>(Overflow)
            .iter()
            .all(|x| x.resize::<8>().is_none())
    );
    assert!(
        w::resize::<4, 6, 8>(Typical)
            .iter()
            .all(|x| x.resize::<12>().is_some())
    );
    assert!(
        w::to_wide::<6, 12>(Typical)
            .iter()
            .all(|x| x.to_wide::<6>().is_some())
    );
    let product = |&(a, b): &(i64, i64)| Wide32::<3>::product_i64(a, b);
    assert!(w::product_i64(Typical).iter().all(|p| product(p).is_some()));
    assert!(w::product_i64(Extreme).iter().all(|p| product(p).is_some()));
    assert!(
        w::product_i64(Overflow)
            .iter()
            .all(|p| product(p).is_none())
    );
    // Typical operands are translations of the grid, of 41 bits.
    assert!(
        w::product_i64(Typical)
            .iter()
            .all(|&(a, b)| a.unsigned_abs() >> 40 == 1 && b.unsigned_abs() >> 40 == 1)
    );
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
    use geom_num::math::unproven as fu;
    use inputs::math as m;
    use inputs::math::unproven as u;
    let unary: [(&str, Doubles, Unary); 22] = [
        ("sqrt", m::sqrt, f::sqrt),
        ("exp", m::exp, f::exp),
        ("ln", m::ln, f::ln),
        ("abs", m::abs, f::abs),
        ("floor", m::floor, f::floor),
        ("ceil", m::ceil, f::ceil),
        ("round_ties_even", m::round_ties_even, f::round_ties_even),
        ("sinpi", m::sinpi, f::sinpi),
        ("cospi", m::cospi, f::cospi),
        ("unproven::sin", u::sin, fu::sin),
        ("unproven::cos", u::cos, fu::cos),
        ("unproven::tan", u::tan, fu::tan),
        ("unproven::asin", u::asin, fu::asin),
        ("unproven::acos", u::acos, fu::acos),
        ("unproven::atan", u::atan, fu::atan),
        ("unproven::exp", u::exp, fu::exp),
        ("unproven::ln", u::ln, fu::ln),
        ("unproven::log2", u::log2, fu::log2),
        ("unproven::log10", u::log10, fu::log10),
        ("unproven::cbrt", u::cbrt, fu::cbrt),
        ("unproven::sinpi", u::sinpi, fu::sinpi),
        ("unproven::cospi", u::cospi, fu::cospi),
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
    let binary: [(&str, DoublePairs, Binary); 4] = [
        ("atan2", m::atan2, f::atan2),
        ("unproven::atan2", u::atan2, fu::atan2),
        ("unproven::pow", u::pow, fu::pow),
        ("unproven::hypot", u::hypot, fu::hypot),
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
        for (x, n) in u::powi(class) {
            assert!(
                x.is_finite() && in_domain(fu::powi(x, n)),
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
        u::powi(Typical)
            .into_iter()
            .all(|(x, n)| fu::powi(x, n).is_ok())
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
        u::powi(Extreme)
            .into_iter()
            .map(|(x, n)| fu::powi(x, n))
            .collect()
    ));
    assert!(overflows(
        m::fma(Extreme)
            .into_iter()
            .map(|(a, b, c)| math::fma(a, b, c))
            .collect()
    ));
    // powi: both ends of the exponent's type, and both signs of base in the typical class.
    let powers = u::powi(Extreme);
    assert!(
        powers.iter().any(|&(_, n)| n == i32::MAX) && powers.iter().any(|&(_, n)| n == i32::MIN)
    );
    assert!(u::powi(Typical).iter().any(|&(x, _)| x < 0.0));
    // fma: the addend cancels the rounded product exactly, so the result is the product's
    // rounding error, which the single rounding keeps.
    assert!(
        m::fma(Extreme)
            .iter()
            .any(|&(a, b, c)| c == -(a * b) && math::fma(a, b, c).is_ok_and(|r| r.get() != 0.0))
    );
    // The extreme sines need the longest argument reduction.
    assert!(u::sin(Extreme).iter().any(|&x| x.abs() > pow2(1000)));
    // A quarter of the typical half-turn inputs take the exact branch.
    let exact = m::sinpi(Typical)
        .iter()
        .filter(|&&x| math::floor(x * 2.0).is_ok_and(|f| f.get() == x * 2.0))
        .count();
    assert!(exact >= BATCH / 4);
}

/// Whether Lemire's method rejects the first word a draw of `below(n)` from `stream` takes: the
/// low half of w · n is below 2^64 mod n.
fn first_word_rejected(stream: &geom_num::rng::Stream, n: u64) -> bool {
    let w = stream.clone().next_u64().unwrap();
    let low = (u128::from(w) * u128::from(n)) % (1 << 64);
    n != 0 && low < (1u128 << 64) % u128::from(n)
}

#[test]
fn rng_classes_take_every_path() {
    use geom_num::rng::{RngError, Seed, StreamId};
    use inputs::rng as r;
    const END: u128 = 1 << 66;
    // stream_from: every combination of 0 and 2^64 − 1 for the seed, the stream id and the block.
    let ends: BTreeSet<(u64, u64, u64)> = r::stream_from(Extreme)
        .iter()
        .map(|(p, b)| (p.seed().0, p.stream_id().0, *b))
        .collect();
    assert_eq!(ends.len(), 8);
    assert!(
        ends.iter()
            .all(|&(s, i, b)| [s, i, b].iter().all(|&v| v == 0 || v == u64::MAX))
    );
    // The draws: typical streams at all four places of a block, far from the end, so one draw in
    // four computes the next block; extreme streams in their last block with enough words left;
    // overflow streams with too few.
    let words_needed = [
        (r::next_u32 as fn(Class) -> Vec<_>, 1),
        (r::next_u64, 2),
        (r::next_f64, 2),
    ];
    for (build, words) in words_needed {
        let places: BTreeSet<u128> = build(Typical).iter().map(|s| s.position() % 4).collect();
        assert_eq!(places.len(), 4);
        assert!(build(Typical).iter().all(|s| s.remaining() > END / 4));
        let left: BTreeSet<u128> = build(Extreme).iter().map(|s| s.remaining()).collect();
        assert_eq!(left, (words..=4).collect());
        let left: BTreeSet<u128> = build(Overflow).iter().map(|s| s.remaining()).collect();
        assert_eq!(left, (0..words).collect());
    }
    // below: typical bounds of every size up to about 2^40, never rejected; the extreme bounds,
    // with words really rejected; overflow streams with too few words for a 64-bit draw.
    let typical = r::below(Typical);
    assert!(
        typical
            .iter()
            .all(|&(_, n)| (2..=(1 << 40) + 1).contains(&n))
    );
    assert!(typical.iter().any(|&(_, n)| n < 1 << 8) && typical.iter().any(|&(_, n)| n > 1 << 32));
    let extreme = r::below(Extreme);
    for wanted in [0, 1, u64::MAX, (1 << 63) + 1] {
        assert!(extreme.iter().any(|&(_, n)| n == wanted), "{wanted}");
    }
    assert!(
        extreme
            .iter()
            .any(|(s, n)| *n > 1 << 63 && first_word_rejected(s, *n))
    );
    assert!(r::below(Overflow).iter().all(|(s, _)| s.remaining() < 2));
    // range_inclusive: the same, and the whole of u64, one value, and an empty range.
    let typical = r::range_inclusive(Typical);
    assert!(
        typical
            .iter()
            .all(|&(_, lo, hi)| lo <= hi && hi - lo < 1 << 41)
    );
    let extreme = r::range_inclusive(Extreme);
    assert!(extreme.iter().any(|&(_, lo, hi)| (lo, hi) == (0, u64::MAX)));
    assert!(extreme.iter().any(|&(_, lo, hi)| lo == hi));
    assert!(extreme.iter().any(|&(_, lo, hi)| lo > hi));
    assert!(extreme.iter().any(|(s, lo, hi)| {
        hi.checked_sub(*lo) == Some(1 << 63) && first_word_rejected(s, (1 << 63) + 1)
    }));
    assert!(
        r::range_inclusive(Overflow)
            .iter()
            .all(|(s, _, _)| s.remaining() < 2)
    );
    // range_philox: inside the range, then past its last stream, including a first id plus an
    // index past 2^64 − 1.
    for (range, index) in r::range_philox(Typical) {
        assert!(range.philox(index).is_ok());
    }
    let past = r::range_philox(Overflow);
    assert!(
        past.iter().all(|(range, index)| matches!(
            range.philox(*index),
            Err(RngError::TooFewStreams { .. })
        ))
    );
    assert!(
        past.iter()
            .any(|(range, index)| range.first().0.checked_add(*index).is_none())
    );
    // chunk: inside the range; the last chunks of a whole range and a chunk of no streams; past
    // the range, including an index times a size past 2^64 − 1.
    for (range, index, size) in r::chunk(Typical) {
        assert!(range.chunk(index, size).is_ok());
    }
    let extreme = r::chunk(Extreme);
    let lasts: BTreeSet<(u64, u64)> = extreme
        .iter()
        .filter_map(|(range, index, size)| range.chunk(*index, *size).ok())
        .map(|c| (c.first().0, c.last().0))
        .collect();
    let expected = [(1 << 63, u64::MAX), (0, u64::MAX - 1), (u64::MAX, u64::MAX)];
    assert_eq!(lasts, expected.into_iter().collect());
    assert!(
        extreme
            .iter()
            .any(|(range, index, size)| range.chunk(*index, *size) == Err(RngError::EmptyChunk))
    );
    let past = r::chunk(Overflow);
    assert!(past.iter().all(|(range, index, size)| matches!(
        range.chunk(*index, *size),
        Err(RngError::TooFewStreams { .. })
    )));
    assert!(
        past.iter()
            .any(|(_, index, size)| index.checked_mul(*size).is_none())
    );
    // Every typical seed and stream id is its own: no two inputs share a stream.
    let streams: BTreeSet<(Seed, StreamId)> = r::next_u32(Typical)
        .iter()
        .map(|s| (s.philox().seed(), s.philox().stream_id()))
        .collect();
    assert_eq!(streams.len(), BATCH);
}

#[test]
fn dd_classes_take_every_path() {
    use geom_num::dd::{self as d, DD};
    use inputs::dd as i;
    let below_normal = |x: DD| x.hi() != 0.0 && x.hi().abs() < f64::MIN_POSITIVE;
    // 2^−968, from which dd::two_prod_fma uses the fused multiply-add.
    let threshold = pow2(-968);
    // The products: the fused multiply-add's path, the scaled path below 2^−968, a zero factor,
    // an overflow, an error below 2^−1074, and for exact_product a factor that is not finite.
    let products = [
        ("two_prod_fma", i::two_prod_fma(Extreme)),
        ("exact_product", i::exact_product(Extreme)),
    ];
    for (name, batch) in products {
        let paths: Vec<(f64, f64)> = batch.iter().map(|&(a, b)| d::two_prod_fma(a, b)).collect();
        let fma = |&(p, e): &(f64, f64)| p.is_finite() && p.abs() >= threshold && e.is_finite();
        assert!(paths.iter().any(fma), "{name}");
        let scaled = |&(p, e): &(f64, f64)| p != 0.0 && p.abs() < threshold && e.is_finite();
        assert!(paths.iter().any(scaled), "{name}");
        assert!(batch.iter().any(|&(a, b)| a == 0.0 || b == 0.0), "{name}");
        assert!(paths.iter().any(|&(p, _)| p.is_infinite()), "{name}");
        assert!(
            paths.iter().any(|&(p, e)| p.is_finite() && !e.is_finite()),
            "{name}"
        );
        let all_finite = batch.iter().all(|&(a, b)| a.is_finite() && b.is_finite());
        assert_eq!(all_finite, name == "two_prod_fma", "{name}");
    }
    assert!(
        i::two_prod_fma(Typical)
            .iter()
            .all(|&(a, b)| (a * b).abs() >= threshold)
    );
    // The exact sum of two doubles: near overflow, overflowing, subnormal, huge plus tiny, and not
    // finite.
    let sums = i::from_sum(Extreme);
    let made: Vec<_> = sums.iter().map(|&(a, b)| DD::from_sum(a, b)).collect();
    assert!(
        made.iter()
            .any(|x| x.is_ok_and(|x| x.hi().abs() > pow2(1000)))
    );
    let overflowed = |(&(a, b), x): (&(f64, f64), &Result<DD, NumError>)| {
        a.is_finite() && b.is_finite() && x.is_err()
    };
    assert!(sums.iter().zip(&made).any(overflowed));
    assert!(made.iter().any(|x| x.is_ok_and(below_normal)));
    assert!(
        made.iter()
            .any(|x| x.is_ok_and(|x| x.lo() != 0.0 && x.hi().abs() > pow2(800)))
    );
    assert!(sums.iter().any(|&(a, _)| !a.is_finite()));
    // The integers: the ends of i64; every size below 2^106, its ends included; and from 2^106 up.
    let small = i::from_i64(Typical);
    assert!([i64::MIN, i64::MAX, 0].iter().all(|v| small.contains(v)));
    let typical = i::from_i128(Typical);
    assert!(typical.iter().all(|v| v.unsigned_abs() < 1 << 106));
    assert!(typical.contains(&((1 << 106) - 1)));
    assert!(typical.iter().any(|v| v.unsigned_abs() < 1 << 40));
    assert!(
        i::from_i128(Overflow)
            .iter()
            .all(|v| v.unsigned_abs() >= 1 << 106)
    );
    // Sign, negation and magnitude: both signs and zero.
    for batch in [i::sign(Typical), i::negate(Typical), i::abs(Typical)] {
        let signs: BTreeSet<_> = batch.iter().map(|x| x.sign()).collect();
        assert_eq!(signs.len(), 3);
    }
    // The arithmetic: overflow refused, results below the normal range, zero operands, exact
    // cancellation, non-finite doubles refused, zero divisors refused.
    let add = i::add(Extreme);
    assert!(add.iter().any(|&(x, y)| x.add(y).is_err()));
    assert!(add.iter().any(|&(x, y)| x.add(y).is_ok_and(below_normal)));
    assert!(
        add.iter()
            .any(|&(x, y)| x.hi() != 0.0 && x.add(y) == Ok(DD::ZERO))
    );
    assert!(add.iter().any(|&(_, y)| y == DD::ZERO));
    let add_f64 = i::add_f64(Extreme);
    assert!(add_f64.iter().any(|&(_, y)| !y.is_finite()));
    assert!(
        add_f64
            .iter()
            .any(|&(x, y)| y.is_finite() && x.add_f64(y).is_err())
    );
    let mul_f64 = i::mul_f64(Extreme);
    assert!(
        mul_f64
            .iter()
            .any(|&(x, y)| y.is_finite() && x.mul_f64(y).is_err())
    );
    assert!(
        mul_f64
            .iter()
            .any(|&(x, y)| x.mul_f64(y).is_ok_and(below_normal))
    );
    assert!(mul_f64.iter().any(|&(_, y)| y == 0.0));
    assert!(mul_f64.iter().any(|&(_, y)| !y.is_finite()));
    let mul = i::mul(Extreme);
    assert!(mul.iter().any(|&(x, y)| x.mul(y).is_err()));
    assert!(
        mul.iter()
            .any(|&(x, y)| x.hi() != 0.0 && x.mul(y) == Ok(DD::ZERO))
    );
    let huge_by_tiny = |&(x, y): &(DD, DD)| {
        x.hi().abs() > pow2(1000) && y.hi().abs() < pow2(-1000) && x.mul(y).is_ok()
    };
    assert!(mul.iter().any(huge_by_tiny));
    let div_f64 = i::div_f64(Extreme);
    assert!(div_f64.iter().any(|&(_, y)| y == 0.0));
    assert!(div_f64.iter().any(|&(_, y)| !y.is_finite()));
    assert!(
        div_f64
            .iter()
            .any(|&(x, y)| y != 0.0 && y.is_finite() && x.div_f64(y).is_err())
    );
    assert!(
        div_f64
            .iter()
            .any(|&(x, y)| x.div_f64(y).is_ok_and(below_normal))
    );
    assert!(div_f64.iter().any(|&(x, _)| x == DD::ZERO));
    let div = i::div(Extreme);
    assert!(div.iter().any(|&(_, y)| y == DD::ZERO));
    assert!(div.iter().any(|&(x, _)| x == DD::ZERO));
    assert!(div.iter().any(|&(x, y)| y != DD::ZERO && x.div(y).is_err()));
    assert!(div.iter().any(|&(x, y)| x.div(y).is_ok_and(below_normal)));
    let roots = i::sqrt(Extreme);
    assert!(
        roots
            .iter()
            .any(|x| x.sqrt() == Err(NumError::OutsideDomain))
    );
    assert!(roots.contains(&DD::ZERO));
    assert!(roots.iter().any(|x| x.hi() > pow2(1022)));
    assert!(roots.iter().any(|x| below_normal(*x)));
    assert!(i::sqrt(Typical).iter().all(|x| x.hi() > 0.0));
    // Rounding to a whole number: halves (in the leading part, and as the trailing part), whole
    // numbers, 2^100, and the ends of i128, all answered; refused above 2^100 or past i128.
    let rounds = i::round_shifted(Extreme);
    let leading_half = |&(x, _): &(DD, i128)| x.lo() == 0.0 && (x.hi() * 2.0) % 2.0 != 0.0;
    assert!(rounds.iter().any(leading_half));
    assert!(rounds.iter().any(|&(x, _)| x.lo() == 0.5));
    assert!(
        rounds
            .iter()
            .any(|&(x, k)| x.round_shifted(k).is_some_and(|r| r.is_exact()))
    );
    assert!(rounds.iter().any(|&(x, _)| x.hi() == pow2(100)));
    assert!(rounds.iter().any(|&(_, k)| k == i128::MAX));
    assert!(rounds.iter().any(|&(_, k)| k == i128::MIN + 1));
    assert!(rounds.iter().all(|&(x, k)| x.round_shifted(k).is_some()));
    assert!(
        i::round_shifted(Overflow)
            .iter()
            .all(|&(x, k)| x.round_shifted(k).is_none())
    );
    // The order: equal pairs, the same leading part with different trailing parts, opposite signs.
    let pairs = i::cmp(Typical);
    assert!(pairs.iter().any(|(x, y)| x == y));
    assert!(
        pairs
            .iter()
            .any(|(x, y)| x.hi() == y.hi() && x.lo() != y.lo())
    );
    assert!(pairs.iter().any(|(x, y)| x.sign() != y.sign()));
}

#[test]
fn exact_sum_classes_take_every_path() {
    use geom_num::exact_sum::LARGE_SLICE;
    use inputs::exact_sum as i;
    // Typical terms are finite; extreme ones reach subnormals, the top of the range and the
    // special values.
    assert!(i::add(Typical).iter().flatten().all(|x| x.is_finite()));
    let extreme: Vec<f64> = i::add(Extreme).iter().flatten().copied().collect();
    assert!(extreme.iter().any(|x| x.is_nan()));
    assert!(extreme.contains(&f64::INFINITY) && extreme.contains(&f64::NEG_INFINITY));
    assert!(extreme.iter().any(|x| x.is_subnormal()));
    assert!(extreme.iter().any(|x| x.abs() > pow2(1015)));
    // The slices: short ones one term at a time, long ones through the large superaccumulator.
    assert!(i::add_slice(Typical).iter().all(|t| t.len() < LARGE_SLICE));
    assert!(i::add_slice(Extreme).iter().all(|t| t.len() >= LARGE_SLICE));
    let iters = i::add_iter(Extreme);
    assert!(iters.iter().all(|(t, _)| t.len() > LARGE_SLICE));
    assert!(iters.iter().any(|(_, known)| *known));
    assert!(iters.iter().any(|(_, known)| !known));
    // The roundings: an exact zero, a tie, a subnormal sum, an overflow of finite terms, and
    // special values.
    let roundings = [
        (i::round_terms(Extreme), i::round(Extreme)),
        (i::round_ieee_terms(Extreme), i::round_ieee(Extreme)),
    ];
    for (terms, sums) in roundings {
        let rounded: Vec<f64> = sums.iter().map(|s| s.round_ieee()).collect();
        let pairs: Vec<(f64, &Vec<f64>)> = rounded.iter().copied().zip(&terms).collect();
        assert!(
            pairs
                .iter()
                .any(|(r, t)| r.to_bits() == 0 && t.iter().any(|x| *x != 0.0))
        );
        assert!(terms.iter().any(|t| t == &[1.0, pow2(-53)]));
        assert!(rounded.iter().any(|r| r.is_subnormal()));
        assert!(
            pairs
                .iter()
                .any(|(r, t)| r.is_infinite() && t.iter().all(|x| x.is_finite()))
        );
        assert!(rounded.iter().any(|r| r.is_nan()));
    }
    // Merging: special values, and pairs that cancel exactly.
    let merges = i::merge_terms(Extreme);
    assert!(
        merges
            .iter()
            .any(|(a, b)| a.iter().chain(b).any(|x| !x.is_finite()))
    );
    let cancels =
        |(a, b): &(Vec<f64>, Vec<f64>)| a.iter().zip(b).all(|(x, y)| x.to_bits() == (-y).to_bits());
    assert!(merges.iter().any(cancels));
}

#[test]
fn enclosure_classes_take_every_path() {
    use inputs::interval as i;
    // The periodic two: typical intervals around a peak and between peaks; extreme ones wider than
    // a turn ([−1, 1]), from 2^53 up, with ends at multiples of ½, a point at a peak, subnormal ends
    // and one double short of a peak.
    type Intervals = fn(Class) -> Vec<Interval>;
    type Enclosure = fn(Interval) -> Result<Interval, NumError>;
    let periodic: [(Intervals, Enclosure); 2] =
        [(i::sinpi, Interval::sinpi), (i::cospi, Interval::cospi)];
    for (build, enclose) in periodic {
        let typical: Vec<Interval> = build(Typical)
            .into_iter()
            .map(|x| enclose(x).unwrap())
            .collect();
        assert!(typical.iter().any(|y| y.hi() == 1.0));
        assert!(typical.iter().any(|y| y.hi() < 1.0 && y.lo() > -1.0));
        let extreme = build(Extreme);
        let enclosed: Vec<Interval> = extreme.iter().map(|x| enclose(*x).unwrap()).collect();
        assert!(enclosed.iter().any(|y| (y.lo(), y.hi()) == (-1.0, 1.0)));
        assert!(extreme.iter().any(|x| x.lo() >= pow2(53)));
        assert!(extreme.iter().any(|x| x.lo() != x.hi()
            && (x.lo() * 2.0).fract() == 0.0
            && (x.hi() * 2.0).fract() == 0.0));
        assert!(extreme.iter().any(|x| x.lo() == x.hi()));
        assert!(
            extreme
                .iter()
                .any(|x| x.lo() != 0.0 && x.lo().is_subnormal())
        );
        assert!(
            extreme
                .iter()
                .any(|x| (x.hi() * 2.0).fract() != 0.0 && (x.hi().next_up() * 2.0).fract() == 0.0)
        );
    }
    // exp: within ±32 typically; past overflow refused, below it answered, the deep underflow, an
    // end at exactly 0.
    assert!(
        i::exp(Typical)
            .iter()
            .all(|x| x.lo() >= -32.0 && x.hi() <= 32.0)
    );
    let exp = i::exp(Extreme);
    assert!(exp.iter().any(|x| x.exp() == Err(NumError::NotFinite)));
    assert!(exp.iter().any(|x| x.hi() > 709.0 && x.exp().is_ok()));
    assert!(exp.iter().any(|x| x.exp().is_ok_and(|y| y.lo() == 0.0)));
    assert!(exp.iter().any(|x| x.lo() == 0.0));
    // ln: every refusal and the exact point.
    assert!(i::ln(Typical).iter().all(|x| x.lo() > 0.0));
    let ln = i::ln(Extreme);
    assert!(ln.iter().any(|x| x.ln() == Err(NumError::OutsideDomain)));
    assert!(ln.iter().any(|x| x.ln() == Err(NumError::NotFinite)));
    assert!(ln.iter().any(|x| x.lo() == 1.0));
    assert!(ln.iter().any(|x| x.lo().is_subnormal()));
    assert!(ln.iter().any(|x| x.hi() > pow2(1020)));
    // atan2: typical boxes never meet the branch cut and always have an angle; extreme ones meet
    // it from both sides, hold the origin at a corner (an exact zero bound), are the origin alone,
    // subnormal, huge, or have their lower edge on the negative x-axis (an upper bound past π).
    let pi_up = std::f64::consts::PI.next_up();
    let wraps = |(y, x): &(Interval, Interval)| x.lo() < 0.0 && y.lo() < 0.0 && y.hi() >= 0.0;
    let typical = i::atan2(Typical);
    assert!(typical.iter().all(|b| !wraps(b) && b.0.atan2(b.1).is_ok()));
    assert!(
        typical
            .iter()
            .any(|(y, x)| y.lo() < 0.0 && y.hi() > 0.0 && x.lo() > 0.0)
    );
    assert!(
        typical
            .iter()
            .any(|(y, x)| x.lo() < 0.0 && x.hi() > 0.0 && y.lo() > 0.0)
    );
    assert!(typical.iter().any(|(y, x)| x.hi() < 0.0 && y.hi() < 0.0));
    let atan2 = i::atan2(Extreme);
    assert!(atan2.iter().any(wraps));
    assert!(
        atan2
            .iter()
            .any(|(y, x)| y.atan2(*x) == Err(NumError::OutsideDomain))
    );
    assert!(
        atan2
            .iter()
            .any(|(y, x)| y.atan2(*x).is_ok_and(|r| r.lo() == 0.0))
    );
    assert!(
        atan2
            .iter()
            .any(|(y, _)| y.lo() != 0.0 && y.lo().is_subnormal())
    );
    assert!(atan2.iter().any(|(y, _)| y.lo() > pow2(1020)));
    assert!(
        atan2
            .iter()
            .any(|b| !wraps(b) && b.0.atan2(b.1).is_ok_and(|r| r.hi() == pi_up))
    );
}

/// The public predicates' batches take the routes their benchmarks are read as (piece G28): on
/// typical inputs the generated filter decides every call, so `predicate/<op>/typical` is the
/// filtered cost; on nearly degenerate inputs it decides none, so every call also runs the exact
/// path (whose signs, never zero, `predicate_classes` checks), and
/// `predicate/<op>/nearly_degenerate` is the cost of the filter followed by the exact path. The
/// exception is orient2d on 64-bit targets other than WebAssembly, whose route skips the filter
/// (`predicate.rs`, routing), so there every class of `predicate/orient2d` is the exact path's
/// cost; the WebAssembly fuel gate measures its filtered route.
#[test]
fn predicate_classes_take_the_filtered_and_the_exact_route() {
    use geom_num::generated as gp;
    use inputs::predicate as i;
    for (class, decides) in [(Typical, true), (NearlyDegenerate, false)] {
        assert!(
            i::orient2d(class)
                .iter()
                .all(|&[a, b, c]| gp::orient2d::filter(a, b, c).is_some() == decides),
            "orient2d, {class}"
        );
        assert!(
            i::orient3d(class)
                .iter()
                .all(|&[a, b, c, d]| gp::orient3d::filter(a, b, c, d).is_some() == decides),
            "orient3d, {class}"
        );
        assert!(
            i::incircle(class)
                .iter()
                .all(|&[a, b, c, d]| gp::incircle::filter(a, b, c, d).is_some() == decides),
            "incircle, {class}"
        );
        assert!(
            i::insphere(class)
                .iter()
                .all(|&[a, b, c, d, e]| gp::insphere::filter(a, b, c, d, e).is_some() == decides),
            "insphere, {class}"
        );
    }
}

#[test]
fn generated_classes_take_their_paths() {
    use geom_num::generated as gp;
    use inputs::generated as i;
    // Typical inputs: the filter decides. Nearly degenerate ones: it cannot, and the exact value is
    // not zero. Degenerate ones: the exact value is zero.
    let typical: Vec<bool> = i::orient2d_filter(Typical)
        .iter()
        .map(|&[a, b, c]| gp::orient2d::filter(a, b, c).is_some())
        .collect();
    assert!(typical.iter().all(|&d| d));
    assert!(
        i::orient2d_filter(NearlyDegenerate)
            .iter()
            .all(|&[a, b, c]| gp::orient2d::filter(a, b, c).is_none())
    );
    assert!(
        i::orient2d_value(Degenerate)
            .iter()
            .all(|&[a, b, c]| gp::orient2d::value(a, b, c) == Some(0))
    );
    assert!(
        i::orient3d_filter(Typical)
            .iter()
            .all(|&[a, b, c, d]| gp::orient3d::filter(a, b, c, d).is_some())
    );
    assert!(
        i::orient3d_filter(NearlyDegenerate)
            .iter()
            .all(|&[a, b, c, d]| gp::orient3d::filter(a, b, c, d).is_none())
    );
    assert!(
        i::orient3d_value(Degenerate)
            .iter()
            .all(|&[a, b, c, d]| gp::orient3d::value(a, b, c, d) == Some(0))
    );
    assert!(
        i::incircle_filter(Typical)
            .iter()
            .all(|&[a, b, c, d]| gp::incircle::filter(a, b, c, d).is_some())
    );
    assert!(
        i::incircle_filter(NearlyDegenerate)
            .iter()
            .all(|&[a, b, c, d]| gp::incircle::filter(a, b, c, d).is_none())
    );
    assert!(
        i::incircle_value(Degenerate)
            .iter()
            .all(|&[a, b, c, d]| gp::incircle::exact(a, b, c, d) == Some(Sign::Zero))
    );
    assert!(
        i::insphere_filter(Typical)
            .iter()
            .all(|&[a, b, c, d, e]| gp::insphere::filter(a, b, c, d, e).is_some())
    );
    assert!(
        i::insphere_filter(NearlyDegenerate)
            .iter()
            .all(|&[a, b, c, d, e]| gp::insphere::filter(a, b, c, d, e).is_none())
    );
    assert!(
        i::insphere_value(Degenerate)
            .iter()
            .all(|&[a, b, c, d, e]| gp::insphere::exact(a, b, c, d, e) == Some(Sign::Zero))
    );
    assert!(
        i::plane_side_filter(Typical)
            .iter()
            .all(|&([x, y, z], d, p)| gp::plane_side::filter(x, y, z, d, p).is_some())
    );
    assert!(
        i::plane_side_filter(NearlyDegenerate)
            .iter()
            .all(|&([x, y, z], d, p)| gp::plane_side::filter(x, y, z, d, p).is_none())
    );
    assert!(
        i::plane_side_value(Degenerate)
            .iter()
            .all(|&([x, y, z], d, p)| gp::plane_side::value(x, y, z, d, p) == Some(0))
    );
}

/// inputs/newton.rs: typical boxes are proven in one step with the zero inside; extreme ones end
/// every other way the documentation lists, never with a proof; typical Jacobians succeed and
/// extreme ones are refused.
#[test]
fn newton_and_dual_classes_are_what_they_claim() {
    use geom_bench::inputs::newton::{self as n, CircleLine, Four};
    use geom_num::dual::jacobian;
    use geom_num::newton::{NewtonError, Residuals, Verdict, hansen_sengupta, krawczyk};

    fn ending<const N: usize>(v: &Result<Verdict<N>, NewtonError>) -> &'static str {
        match v {
            Ok(Verdict::Unique { .. }) => "unique",
            Ok(Verdict::Narrowed { .. }) => "narrowed",
            Ok(Verdict::NoZero) => "no zero",
            Err(NewtonError::Singular) => "singular",
            Err(NewtonError::Num(NumError::NotFinite)) => "not finite",
            Err(NewtonError::Num(NumError::OutsideDomain)) => "outside domain",
            Err(NewtonError::Num(_)) => "other",
        }
    }
    let holds = |v: &Result<Verdict<2>, NewtonError>, zeros: &[[f64; 2]]| match v {
        Ok(Verdict::Unique { enclosure, .. }) => zeros
            .iter()
            .any(|z| enclosure[0].contains(z[0]) && enclosure[1].contains(z[1])),
        _ => false,
    };
    for (build, op) in [
        (
            n::krawczyk_2 as fn(Class) -> Vec<[Interval; 2]>,
            krawczyk::<CircleLine, 2> as fn(&CircleLine, [Interval; 2]) -> _,
        ),
        (n::hansen_sengupta_2, hansen_sengupta::<CircleLine, 2>),
    ] {
        let typical = build(Typical);
        assert!(
            typical
                .iter()
                .all(|b| holds(&op(&CircleLine, *b), &[[4.0, 3.0], [-3.0, -4.0]]))
        );
        let endings: BTreeSet<&str> = build(Extreme)
            .iter()
            .map(|b| ending(&op(&CircleLine, *b)))
            .collect();
        assert_eq!(
            endings,
            BTreeSet::from(["narrowed", "no zero", "not finite", "singular"]),
            "circle and line"
        );
    }
    for (build, op) in [
        (
            n::krawczyk_4 as fn(Class) -> Vec<[Interval; 4]>,
            krawczyk::<Four, 4> as fn(&Four, [Interval; 4]) -> _,
        ),
        (n::hansen_sengupta_4, hansen_sengupta::<Four, 4>),
    ] {
        for b in build(Typical) {
            let Ok(Verdict::Unique { enclosure, .. }) = op(&Four, b) else {
                panic!("{b:?} not proven")
            };
            assert!(
                enclosure
                    .iter()
                    .zip([0.0, 1.0, 1.0, 1.0])
                    .all(|(i, z)| i.contains(z))
            );
        }
        let endings: BTreeSet<&str> = build(Extreme)
            .iter()
            .map(|b| ending(&op(&Four, *b)))
            .collect();
        assert_eq!(
            endings,
            BTreeSet::from(["narrowed", "no zero", "not finite", "outside domain"]),
            "four"
        );
    }
    assert!(
        n::jacobian_real(Typical)
            .into_iter()
            .all(|x| jacobian(|v| Four.residuals(v), x).is_ok())
    );
    assert!(
        n::jacobian_real(Extreme)
            .into_iter()
            .all(|x| jacobian(|v| Four.residuals(v), x) == Err(NumError::NotFinite))
    );
    assert!(
        n::jacobian_interval(Typical)
            .into_iter()
            .all(|x| jacobian(|v| Four.residuals(v), x).is_ok())
    );
    let refused: BTreeSet<String> = n::jacobian_interval(Extreme)
        .into_iter()
        .map(|x| format!("{:?}", jacobian(|v| Four.residuals(v), x).unwrap_err()))
        .collect();
    assert_eq!(
        refused,
        BTreeSet::from(["NegativeSqrt", "NotFinite", "OutsideDomain"].map(String::from))
    );
}

/// Every typical isolation proves every zero in its domain and flags nothing; the extreme batches
/// end in exactly the outcomes their inputs' header lists; every typical ball succeeds, and the
/// extreme balls include an overflow.
#[test]
fn isolate_and_ball_classes_are_what_they_claim() {
    use geom_bench::inputs::isolate::{self as i, Cubic};
    use geom_bench::inputs::newton::CircleLine;
    use geom_num::ball::Ball;
    use geom_num::isolate::{IsolateError, Isolation, Options, System, isolate};
    use geom_num::newton::Residuals;
    use geom_num::real::Real;
    use geom_run::{Budget, WorkMeter};

    fn run<Sys: System<N, D>, const N: usize, const D: usize>(
        system: &Sys,
        domain: [Interval; N],
        target: Real,
    ) -> Result<Isolation<N>, IsolateError<N>> {
        let options = Options {
            target,
            ..Options::DEFAULT
        };
        isolate(
            system,
            domain,
            options,
            &mut WorkMeter::new(Budget::UNLIMITED),
        )
    }
    fn ending<const N: usize>(r: &Result<Isolation<N>, IsolateError<N>>) -> &'static str {
        match r {
            Ok(found) if found.zeros.is_empty() && found.flagged.is_empty() => "no zero",
            Ok(found) if found.flagged.is_empty() => "proven",
            Ok(_) => "flagged",
            Err(IsolateError::CapReached { .. }) => "cap reached",
            Err(_) => "other",
        }
    }
    let inside = |b: &[Interval], z: &[f64]| b.iter().zip(z).all(|(i, v)| i.contains(*v));
    for (domain, target) in i::cubic_1(Typical) {
        let found = run(&Cubic, domain, target).unwrap();
        let expected = [-1.5, 0.25, 2.0]
            .iter()
            .filter(|z| domain[0].contains(**z))
            .count();
        assert_eq!(found.zeros.len(), expected);
        assert!(found.flagged.is_empty());
    }
    let endings: BTreeSet<&str> = i::cubic_1(Extreme)
        .into_iter()
        .map(|(domain, target)| ending(&run(&Cubic, domain, target)))
        .collect();
    assert_eq!(
        endings,
        BTreeSet::from(["cap reached", "flagged", "no zero", "proven"])
    );
    for (domain, target) in i::circle_line_2(Typical) {
        let found = run(&CircleLine, domain, target).unwrap();
        assert_eq!(found.zeros.len(), 1);
        assert!(found.flagged.is_empty());
        let e = &found.zeros[0].enclosure;
        assert!(inside(e, &[4.0, 3.0]) || inside(e, &[-3.0, -4.0]));
    }
    let endings: BTreeSet<&str> = i::circle_line_2(Extreme)
        .into_iter()
        .map(|(domain, target)| ending(&run(&CircleLine, domain, target)))
        .collect();
    assert_eq!(
        endings,
        BTreeSet::from(["cap reached", "flagged", "no zero"])
    );
    let cubic = |[x]: [Interval; 1]| -> Result<Interval, NumError> {
        let [v] = Cubic.residuals([Ball::from_interval(x)?])?;
        v.to_interval()
    };
    assert!(i::ball_cubic(Typical).into_iter().all(|x| cubic(x).is_ok()));
    assert!(
        i::ball_cubic(Extreme)
            .into_iter()
            .any(|x| cubic(x) == Err(NumError::NotFinite))
    );
}

/// Runs `kernel` on `batch` and requires a full batch of `Ok` results, naming the first error.
fn all_ok<T, R, E: core::fmt::Debug>(
    name: &str,
    kernel: fn(&[T], &mut Vec<Result<R, E>>),
    batch: &[T],
) {
    let mut out = Vec::with_capacity(batch.len());
    kernel(batch, &mut out);
    assert_eq!(
        out.len(),
        BATCH,
        "{name}: a kernel that could not build its transform or frame"
    );
    for r in &out {
        assert!(r.is_ok(), "{name}: {:?}", r.as_ref().err());
    }
}

/// Every typical batch of `geom-math`'s benchmarks (decision 32.44) is what its module says: full,
/// and every transform image, rotation, map point and decimal in it is `Ok`, so each benchmark
/// measures the path that succeeds.
#[test]
fn math_types_typical_batches_all_succeed() {
    use geom_bench::inputs::math_types as m;
    use geom_bench::kernels::math_types as k;
    all_ok("apply_exact", k::apply_exact, &m::apply_exact(Typical));
    all_ok(
        "apply_rational",
        k::apply_rational,
        &m::apply_rational(Typical),
    );
    all_ok(
        "apply_certified",
        k::apply_certified,
        &m::apply_certified(Typical),
    );
    all_ok(
        "rotation_apply",
        k::rotation_apply,
        &m::rotation_apply(Typical),
    );
    all_ok("plane_within", k::plane_within, &m::plane_within(Typical));
    all_ok("to_map", k::to_map, &m::to_map(Typical));
    all_ok(
        "decimal_parse",
        k::decimal_parse,
        &m::decimal_parse(Typical),
    );
}

/// Every typical batch of `geom-linalg`'s benchmarks (decision 32.44) factorises every matrix.
#[test]
fn linalg_typical_batches_all_succeed() {
    use geom_bench::inputs::linalg as m;
    use geom_bench::kernels::linalg as k;
    all_ok("pivoted_qr", k::pivoted_qr, &m::pivoted_qr(Typical));
    all_ok("svd_jacobi", k::svd_jacobi, &m::svd_jacobi(Typical));
}

/// Every typical batch of `geom-bvh`'s benchmarks (decision 32.44) builds every scene and answers
/// every query.
#[test]
fn bvh_typical_batches_all_succeed() {
    use geom_bench::inputs::bvh as m;
    use geom_bench::kernels::bvh as k;
    all_ok(
        "tree_build_query",
        k::tree_build_query,
        &m::tree_build_query(Typical),
    );
    all_ok("cells_within", k::cells_within, &m::cells_within(Typical));
}

/// Every typical batch of `geom-nurbs`'s benchmarks (decision 32.44) evaluates, interpolates and
/// bounds every input.
#[test]
fn nurbs_typical_batches_all_succeed() {
    use geom_bench::inputs::nurbs as m;
    use geom_bench::kernels::nurbs as k;
    all_ok("curve_point", k::curve_point, &m::curve_point(Typical));
    all_ok("interpolate", k::interpolate, &m::interpolate(Typical));
    all_ok(
        "bernstein_bound",
        k::bernstein_bound,
        &m::bernstein_bound(Typical),
    );
}

/// Every typical batch of `geom-curve`'s benchmarks (decision 32.44) evaluates, encloses and
/// converts every input.
#[test]
fn curve_typical_batches_all_succeed() {
    use geom_bench::inputs::curve as m;
    use geom_bench::kernels::curve as k;
    all_ok("helix_point", k::helix_point, &m::helix_point(Typical));
    all_ok(
        "helix_enclose",
        k::helix_enclose,
        &m::helix_enclose(Typical),
    );
    all_ok(
        "bulge_to_nurbs",
        k::bulge_to_nurbs,
        &m::bulge_to_nurbs(Typical),
    );
    all_ok(
        "transition_point",
        k::transition_point,
        &m::transition_point(Typical),
    );
    all_ok(
        "transition_to_nurbs",
        k::transition_to_nurbs,
        &m::transition_to_nurbs(Typical),
    );
}

/// Every typical batch of `geom-surface`'s benchmarks (decision 32.44) evaluates and encloses
/// every pair.
#[test]
fn surface_typical_batches_all_succeed() {
    use geom_bench::inputs::surface as m;
    use geom_bench::kernels::surface as k;
    all_ok("torus_point", k::torus_point, &m::torus_point(Typical));
    all_ok(
        "torus_enclose",
        k::torus_enclose,
        &m::torus_enclose(Typical),
    );
    all_ok("offset_point", k::offset_point, &m::offset_point(Typical));
    all_ok(
        "torus_second_bound",
        k::torus_second_bound,
        &m::torus_second_bound(Typical),
    );
    all_ok(
        "torus_normal_bound",
        k::torus_normal_bound,
        &m::torus_normal_bound(Typical),
    );
    // The torus has no pole, so every box bounds both partials and the normal's.
    let mut seconds = Vec::new();
    k::torus_second_bound(&m::torus_second_bound(Typical), &mut seconds);
    assert!(
        seconds
            .iter()
            .all(|r| matches!(r, Ok(geom_surface::SecondDerivativeBound::Bounded(_))))
    );
    let mut normals = Vec::new();
    k::torus_normal_bound(&m::torus_normal_bound(Typical), &mut normals);
    assert!(normals.iter().all(|r| matches!(
        r,
        Ok([
            geom_curve::DerivativeBound::Bounded(_),
            geom_curve::DerivativeBound::Bounded(_)
        ])
    )));
}

/// Every typical batch of the root-finding and quadrature benchmarks (decision 32.44) finds every
/// zero and integrates every interval.
#[test]
fn roots_typical_batches_all_succeed() {
    use geom_bench::inputs::roots as m;
    use geom_bench::kernels::roots as k;
    all_ok("zero", k::zero, &m::zero(Typical));
    all_ok(
        "gauss_legendre",
        k::gauss_legendre,
        &m::gauss_legendre(Typical),
    );
}

/// The carrier and rational-point batches are what their classes claim, for the module and for the
/// generated functions it calls (piece G187).
#[test]
fn the_carrier_classes_are_what_they_say() {
    use geom_bench::inputs::{carrier as c, generated as i};
    use geom_num::carrier::Rational2;
    use geom_num::generated as gp;
    for class in [Typical, Degenerate, NearlyDegenerate, Extreme] {
        assert_eq!(c::side(class).len(), BATCH, "{class}");
        assert_eq!(c::order(class).len(), BATCH, "{class}");
        assert_eq!(c::cmp_x(class).len(), BATCH, "{class}");
        assert_eq!(c::orient(class).len(), BATCH, "{class}");
    }
    for class in [Typical, Degenerate, Extreme] {
        assert_eq!(c::meet(class).len(), BATCH, "{class}");
    }
    assert!(
        c::side(Degenerate)
            .iter()
            .all(|(l, p)| l.side(p) == Ok(Sign::Zero))
    );
    assert!(
        c::side(Typical)
            .iter()
            .all(|(l, p)| l.side(p) != Ok(Sign::Zero))
    );
    assert!(
        c::side(NearlyDegenerate)
            .iter()
            .all(|(l, p)| l.side(p) != Ok(Sign::Zero))
    );
    assert!(
        c::order(Degenerate)
            .iter()
            .all(|(l, p, q)| l.order(p, q) == Ok(Sign::Zero))
    );
    assert!(
        c::order(NearlyDegenerate)
            .iter()
            .all(|(l, p, q)| l.order(p, q) != Ok(Sign::Zero))
    );
    assert!(
        c::cmp_x(Degenerate)
            .iter()
            .all(|(p, q)| p.cmp_x(q) == Ok(Sign::Zero))
    );
    assert!(
        c::cmp_x(NearlyDegenerate)
            .iter()
            .all(|(p, q)| p.cmp_x(q) != Ok(Sign::Zero))
    );
    let turn = |[p, q, r]: &[Rational2; 3]| Rational2::orient(p, q, r);
    assert!(
        c::orient(Degenerate)
            .iter()
            .all(|t| turn(t) == Ok(Sign::Zero))
    );
    assert!(
        c::orient(NearlyDegenerate)
            .iter()
            .all(|t| turn(t) != Ok(Sign::Zero))
    );
    assert!(
        c::meet(Degenerate)
            .iter()
            .all(|(l, m)| l.meet(m).err() == Some(NumError::Degenerate))
    );
    assert!(c::meet(Typical).iter().all(|(l, m)| l.meet(m).is_ok()));

    assert!(
        i::carrier_side_filter(Typical)
            .iter()
            .all(|&([a, b, l], [x, y, w])| gp::carrier_side::filter(a, b, l, x, y, w).is_some())
    );
    assert!(
        i::carrier_side_filter(NearlyDegenerate)
            .iter()
            .all(|&([a, b, l], [x, y, w])| gp::carrier_side::filter(a, b, l, x, y, w).is_none())
    );
    assert!(
        i::carrier_side_value(Degenerate)
            .iter()
            .all(
                |&([a, b, l], [x, y, w])| gp::carrier_side::exact(a, b, l, x, y, w)
                    == Some(Sign::Zero)
            )
    );
    assert!(i::carrier_order_filter(Typical).iter().all(
        |&([a, b], [xp, yp, wp], [xq, yq, wq])| {
            gp::carrier_order::filter(a, b, xp, yp, xq, yq, wp, wq).is_some()
        }
    ));
    assert!(i::carrier_order_filter(NearlyDegenerate).iter().all(
        |&([a, b], [xp, yp, wp], [xq, yq, wq])| {
            gp::carrier_order::filter(a, b, xp, yp, xq, yq, wp, wq).is_none()
        }
    ));
    assert!(i::carrier_order_value(Degenerate).iter().all(
        |&([a, b], [xp, yp, wp], [xq, yq, wq])| {
            gp::carrier_order::exact(a, b, xp, yp, xq, yq, wp, wq) == Some(Sign::Zero)
        }
    ));
    assert!(
        i::rational_cmp_filter(Typical)
            .iter()
            .all(|&[p, q, wp, wq]| gp::rational_cmp::filter(p, q, wp, wq).is_some())
    );
    assert!(
        i::rational_cmp_filter(NearlyDegenerate)
            .iter()
            .all(|&[p, q, wp, wq]| gp::rational_cmp::filter(p, q, wp, wq).is_none())
    );
    assert!(
        i::rational_cmp_value(Degenerate)
            .iter()
            .all(|&[p, q, wp, wq]| gp::rational_cmp::exact(p, q, wp, wq) == Some(Sign::Zero))
    );
    assert!(i::rational_orient_filter(Typical).iter().all(
        |&[[xa, ya, wa], [xb, yb, wb], [xc, yc, wc]]| {
            gp::rational_orient::filter(xa, ya, xb, yb, xc, yc, wa, wb, wc).is_some()
        }
    ));
    assert!(i::rational_orient_filter(NearlyDegenerate).iter().all(
        |&[[xa, ya, wa], [xb, yb, wb], [xc, yc, wc]]| {
            gp::rational_orient::filter(xa, ya, xb, yb, xc, yc, wa, wb, wc).is_none()
        }
    ));
    assert!(i::rational_orient_value(Degenerate).iter().all(
        |&[[xa, ya, wa], [xb, yb, wb], [xc, yc, wc]]| {
            gp::rational_orient::exact(xa, ya, xb, yb, xc, yc, wa, wb, wc) == Some(Sign::Zero)
        }
    ));
}

/// The circle batches are what their classes claim, for the module and for the generated functions
/// it calls (piece G188).
#[test]
fn the_circle_classes_are_what_they_say() {
    use geom_bench::inputs::{arc as a, generated as i};
    use geom_num::arc::Contact;
    use geom_num::generated as gp;
    for class in [Typical, Degenerate, NearlyDegenerate, Extreme] {
        assert_eq!(a::round_grid(class).len(), BATCH, "{class}");
        assert_eq!(a::round_side(class).len(), BATCH, "{class}");
        assert_eq!(a::round_meets(class).len(), BATCH, "{class}");
        assert_eq!(a::circle_side(class).len(), BATCH, "{class}");
        assert_eq!(a::circle_meets(class).len(), BATCH, "{class}");
    }
    assert!(
        a::round_grid(Degenerate)
            .iter()
            .all(|(r, z)| r.side_of_grid(*z) == Ok(Sign::Zero))
    );
    assert!(
        a::round_side(Degenerate)
            .iter()
            .all(|(r, z)| r.side(z) == Ok(Sign::Zero))
    );
    assert!(
        a::round_meets(Degenerate)
            .iter()
            .all(|(r, l)| r.meets(l) == Ok(Contact::Tangent))
    );
    assert!(
        a::circle_side(Degenerate)
            .iter()
            .all(|(c, z)| c.side(z) == Ok(Sign::Zero))
    );
    assert!(
        a::circle_meets(Degenerate)
            .iter()
            .all(|(c, l)| c.meets(l) == Ok(Contact::Tangent))
    );

    assert!(
        i::round_grid_filter(Typical)
            .iter()
            .all(|&(z, p, [cx, cy, w])| gp::round_grid::filter(z, p, cx, cy, w).is_some())
    );
    assert!(
        i::round_grid_filter(NearlyDegenerate)
            .iter()
            .all(|&(z, p, [cx, cy, w])| gp::round_grid::filter(z, p, cx, cy, w).is_none())
    );
    assert!(
        i::circle_gap_filter(NearlyDegenerate)
            .iter()
            .all(|&(a, b, t)| gp::circle_gap::filter(a, b, t).is_none())
    );
    assert!(
        i::circle_gap_value(Degenerate)
            .iter()
            .all(|&(a, b, t)| gp::circle_gap::exact(a, b, t) == Some(Sign::Zero))
    );
}

/// The one-root batches are what their classes claim (piece G189).
#[test]
fn the_one_root_classes_are_what_they_say() {
    use geom_bench::inputs::one_root as r;
    for class in [Typical, Degenerate, NearlyDegenerate, Extreme] {
        assert_eq!(r::cmp_x(class).len(), BATCH, "{class}");
        assert_eq!(r::side(class).len(), BATCH, "{class}");
    }
    for class in [Typical, Degenerate, Extreme] {
        assert_eq!(r::of_circle(class).len(), BATCH, "{class}");
        assert_eq!(r::against_circle(class).len(), BATCH, "{class}");
    }
    assert_eq!(r::of_round(Typical).len(), BATCH);
    assert_eq!(r::of_round(Degenerate).len(), BATCH);
    assert_eq!(r::order_crossings(Typical).len(), BATCH);
    assert!(
        r::cmp_x(Typical)
            .iter()
            .all(|(p, q)| p.cmp_x_filter(q).is_some())
    );
    assert!(
        r::cmp_x(NearlyDegenerate)
            .iter()
            .all(|(p, q)| p.cmp_x_filter(q).is_none())
    );
    assert!(
        r::cmp_x(Degenerate)
            .iter()
            .all(|(p, q)| p.same(q) == Ok(true))
    );
    assert!(
        r::side(Degenerate)
            .iter()
            .all(|(p, l)| p.side(l) == Ok(Sign::Zero))
    );
    assert!(
        r::side(NearlyDegenerate)
            .iter()
            .all(|(p, l)| p.side(l) != Ok(Sign::Zero))
    );
    assert!(
        r::against_circle(Degenerate)
            .iter()
            .all(|(p, c)| p.against_circle(c) == Ok(Sign::Zero))
    );
    assert!(r::of_circle(Degenerate).iter().all(|(l, c)| {
        geom_num::one_root::Crossing2::of_circle(l, c, geom_num::one_root::Root::First)
            .is_ok_and(|p| p.parts().2.is_zero())
    }));
}

/// The conic batches are what their classes claim, for the module and for the generated functions
/// it calls (piece G190).
#[test]
fn the_conic_classes_are_what_they_say() {
    use geom_bench::inputs::{conic as c, generated as i};
    use geom_num::arc::Contact;
    use geom_num::conic::{Conic, Meeting};
    use geom_num::generated as gp;
    use geom_num::one_root::{Crossing2, Root};
    for class in [Typical, Degenerate, NearlyDegenerate, Extreme] {
        assert_eq!(c::side(class).len(), BATCH, "{class}");
        assert_eq!(c::meets(class).len(), BATCH, "{class}");
        assert_eq!(c::meeting(class).len(), BATCH, "{class}");
    }
    for class in [Typical, Degenerate, Extreme] {
        assert_eq!(c::of_ellipse(class).len(), BATCH, "{class}");
        assert_eq!(c::against_ellipse(class).len(), BATCH, "{class}");
        assert_eq!(c::compare(class).len(), BATCH, "{class}");
    }
    assert!(
        c::side(Degenerate)
            .iter()
            .all(|(e, z)| e.side(z) == Ok(Sign::Zero))
    );
    assert!(
        c::side(NearlyDegenerate)
            .iter()
            .all(|(e, z)| e.side(z) == Ok(Sign::Positive))
    );
    assert!(
        c::meets(Degenerate)
            .iter()
            .all(|(e, l)| e.meets(l) == Ok(Contact::Tangent))
    );
    assert!(
        c::meets(NearlyDegenerate)
            .iter()
            .all(|(e, l)| e.meets(l) == Ok(Contact::Crossing))
    );
    assert!(c::of_ellipse(Degenerate).iter().all(|(l, e)| {
        Crossing2::of_ellipse(l, e, Root::First).is_ok_and(|p| p.parts().2.is_zero())
    }));
    assert!(
        c::against_ellipse(Degenerate)
            .iter()
            .all(|(p, e)| p.against_ellipse(e) == Ok(Sign::Zero))
    );
    assert!(c::meeting(Typical).iter().all(|(e, o)| {
        let meeting = Conic::of_ellipse(o).and_then(|c| e.meeting(&c));
        matches!(meeting, Ok(Meeting::At { roots, .. }) if roots.count() == 4)
    }));
    assert!(c::meeting(Degenerate).iter().all(|(e, o)| {
        let meeting = Conic::of_ellipse(o).and_then(|c| e.meeting(&c));
        matches!(meeting, Ok(Meeting::At { roots, .. })
            if roots.count() == 2 && roots.iter().all(|r| r.multiplicity() == 2))
    }));
    assert!(
        c::compare(Degenerate)
            .iter()
            .all(|(p, a, q, b)| geom_num::quartic::compare(p, a, q, b) == Ok(Sign::Zero))
    );

    assert!(
        i::ellipse_side_filter(Typical)
            .iter()
            .all(|&([x, y, w], k, [mx, my, p, q])| {
                gp::ellipse_side::filter(x, y, w, k, mx, my, p, q).is_some()
            })
    );
    assert!(i::ellipse_side_filter(NearlyDegenerate).iter().all(
        |&([x, y, w], k, [mx, my, p, q])| {
            gp::ellipse_side::filter(x, y, w, k, mx, my, p, q).is_none()
        }
    ));
    assert!(i::ellipse_line_filter(NearlyDegenerate).iter().all(
        |&([a, b, d], k, [mx, my, p, q])| {
            gp::ellipse_line::filter(a, b, d, k, mx, my, p, q).is_none()
        }
    ));
    assert!(
        i::ellipse_line_value(Degenerate)
            .iter()
            .all(|&([a, b, d], k, [mx, my, p, q])| {
                gp::ellipse_line::exact(a, b, d, k, mx, my, p, q) == Some(Sign::Zero)
            })
    );
}
