//! Attacks on piece G52: the scalar residuals are written over, the forward-mode dual numbers, and
//! the Krawczyk and Hansen-Sengupta operators.
//!
//! What this file proves: π's two constants bracket and round π; every scalar operation on a
//! `Real` is the kernel's own (bit for bit) and on an `Interval` is the interval method, with the
//! errors each documents; every derivative rule agrees with central differences of the same code
//! at seeded points, alone and inside systems of one to four unknowns; a dual interval over a box
//! encloses the dual interval over any point of it, which encloses the dual real there (the
//! interval Jacobian contains the derivative); the NaN tripwire turns every NaN or infinity, in a
//! value or a derivative, into `NotFinite` from the operation that made it; derivatives over a box
//! where the function is not differentiable are refused; both operators prove known roots in one
//! to four unknowns, with the exact root inside the proven box; refuse singular systems, never
//! calling a double or triple root, or a box with two roots, unique; never lose a zero on seeded
//! random systems, and prove each one from a small box; and decide the edge cases of the
//! containment test, the midpoint and the pivoting the way their documentation says.
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
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_num::NumError;
use geom_num::dual::{Dual, jacobian};
use geom_num::interval::Interval;
use geom_num::math;
use geom_num::newton::{NewtonError, Residuals, Verdict, hansen_sengupta, krawczyk};
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_test::SplitMix64;
use geom_test::seed::{Seed, resolve};
use num_bigint::BigInt;
use num_rational::BigRational;

fn r(x: f64) -> Real {
    Real::new(x).unwrap()
}

fn iv(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

fn c<S: Scalar>(v: f64) -> S {
    S::constant(r(v))
}

fn exact(x: f64) -> BigRational {
    BigRational::from_float(x).unwrap()
}

/// A uniform double in [lo, hi).
fn uniform(rng: &mut SplitMix64, lo: f64, hi: f64) -> f64 {
    let u = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    lo + (hi - lo) * u
}

type Op<R, const N: usize> = fn(&R, [Interval; N]) -> Result<Verdict<N>, NewtonError>;

/// Runs `op` from `x` until it proves a unique zero or `steps` run out, panicking on `NoZero`.
fn prove<R: Residuals<N>, const N: usize>(
    op: Op<R, N>,
    f: &R,
    mut x: [Interval; N],
    steps: usize,
) -> Option<[Interval; N]> {
    for _ in 0..steps {
        match op(f, x).unwrap() {
            Verdict::Unique { enclosure, .. } => return Some(enclosure),
            Verdict::Narrowed { enclosure, .. } => x = enclosure,
            Verdict::NoZero => panic!("a box holding a zero was called empty: {x:?}"),
        }
    }
    None
}

/// Runs `op` on a proven box while it stays proven, up to `steps` more times: the narrowest box
/// reached. Each step's box holds the zero, so its proof is again a proof.
fn refine<R: Residuals<N>, const N: usize>(
    op: Op<R, N>,
    f: &R,
    mut x: [Interval; N],
    steps: usize,
) -> [Interval; N] {
    for _ in 0..steps {
        match op(f, x).unwrap() {
            Verdict::Unique { enclosure, .. } => {
                assert!(inside(&enclosure, &x));
                x = enclosure;
            }
            _ => break,
        }
    }
    x
}

fn width<const N: usize>(b: &[Interval; N]) -> f64 {
    b.iter()
        .map(|i| i.hi() - i.lo())
        .fold(0.0, |a, w| if w > a { w } else { a })
}

fn boxed<const N: usize>(centre: [f64; N], half: f64) -> [Interval; N] {
    centre.map(|v| iv(v - half, v + half))
}

fn contains_point<const N: usize>(b: &[Interval; N], p: [f64; N]) -> bool {
    b.iter().zip(p).all(|(i, v)| i.contains(v))
}

fn inside<const N: usize>(inner: &[Interval; N], outer: &[Interval; N]) -> bool {
    inner
        .iter()
        .zip(outer)
        .all(|(a, b)| a.lo() >= b.lo() && a.hi() <= b.hi())
}

// The systems.

/// x² − 2: the zeros ±√2.
struct Sqrt2;
impl Residuals<1> for Sqrt2 {
    fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
        Ok([x.square()?.sub(c(2.0))?])
    }
}

/// The circle x² + y² = 25 and the line x − y = 1: the zeros (4, 3) and (−3, −4).
struct CircleLine;
impl Residuals<2> for CircleLine {
    fn residuals<S: Scalar>(&self, [x, y]: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([
            x.square()?.add(y.square()?)?.sub(c(25.0))?,
            x.sub(y)?.sub(c(1.0))?,
        ])
    }
}

/// cospi(x) + y = 0 and x − 2y = 1/2: the zero (1/2, 0).
struct CosLine;
impl Residuals<2> for CosLine {
    fn residuals<S: Scalar>(&self, [x, y]: [S; 2]) -> Result<[S; 2], NumError> {
        Ok([x.cospi()?.add(y)?, x.sub(y.add(y)?)?.sub(c(0.5))?])
    }
}

/// x² + y² + z² = 6, x·y = 2 and sinpi(z/6) = 1/2: the zero (1, 2, 1).
struct Three;
impl Residuals<3> for Three {
    fn residuals<S: Scalar>(&self, [x, y, z]: [S; 3]) -> Result<[S; 3], NumError> {
        Ok([
            x.square()?
                .add(y.square()?)?
                .add(z.square()?)?
                .sub(c(6.0))?,
            x.mul(y)?.sub(c(2.0))?,
            z.div(c(6.0))?.sinpi()?.sub(c(0.5))?,
        ])
    }
}

/// exp(a) = b, ln(b) + a·c = 0, atan2(c, d) = π/4 and √(c + d + 2) = 2: the zero (0, 1, 1, 1).
struct Four;
impl Residuals<4> for Four {
    fn residuals<S: Scalar>(&self, [a, b, cc, d]: [S; 4]) -> Result<[S; 4], NumError> {
        Ok([
            a.exp()?.sub(b)?,
            a.mul_add(cc, b.ln()?)?,
            cc.atan2(d)?.sub(S::pi().div(c(4.0))?)?,
            cc.add(d)?.add(c(2.0))?.sqrt()?.sub(c(2.0))?,
        ])
    }
}

/// Fᵢ(x) = Σⱼ aᵢⱼ (xⱼ − rⱼ) + qᵢ (xᵢ − rᵢ)²: `r` is a zero, the only one near it.
#[derive(Clone, Copy, Debug)]
struct Random<const N: usize> {
    a: [[f64; N]; N],
    q: [f64; N],
    r: [f64; N],
}

impl<const N: usize> Residuals<N> for Random<N> {
    fn residuals<S: Scalar>(&self, x: [S; N]) -> Result<[S; N], NumError> {
        let mut out = [c::<S>(0.0); N];
        for i in 0..N {
            let di = x[i].sub(c(self.r[i]))?;
            let mut acc = c::<S>(self.q[i]).mul(di.square()?)?;
            for j in 0..N {
                let dj = x[j].sub(c(self.r[j]))?;
                acc = c::<S>(self.a[i][j]).mul_add(dj, acc)?;
            }
            out[i] = acc;
        }
        Ok(out)
    }
}

impl<const N: usize> Random<N> {
    /// A diagonally dominant linear part, so the Jacobian at `r` is far from singular, and a small
    /// quadratic part, so the system is not linear.
    fn new(rng: &mut SplitMix64) -> Random<N> {
        let mut a = [[0.0; N]; N];
        for (i, row) in a.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = uniform(rng, -1.0, 1.0);
                if i == j {
                    *v += if *v < 0.0 { -(N as f64) } else { N as f64 };
                }
            }
        }
        let q = [(); N].map(|()| uniform(rng, -0.25, 0.25));
        let r = [(); N].map(|()| uniform(rng, -10.0, 10.0));
        Random { a, q, r }
    }
}

// Constants and the scalar.

/// `Real::PI` is π rounded to nearest and `Interval::PI` brackets π, checked against π's first 50
/// digits: the double below π, and the double after it above.
#[test]
fn pi_is_rounded_and_enclosed() {
    let digits = "31415926535897932384626433832795028841971693993751";
    let pi = BigRational::new(digits.parse::<BigInt>().unwrap(), BigInt::from(10).pow(49));
    let tail = BigRational::new(BigInt::from(1), BigInt::from(10).pow(49));
    assert!(exact(Interval::PI.lo()) < pi.clone() - tail.clone());
    assert!(exact(Interval::PI.hi()) > pi.clone() + tail.clone());
    assert_eq!(Interval::PI.hi(), Interval::PI.lo().next_up());
    assert_eq!(Real::PI.get(), core::f64::consts::PI);
    assert_eq!(Real::PI.get(), Interval::PI.lo());
    // Nearest: π is closer to the double below it than to the one above.
    assert!(pi.clone() - exact(Interval::PI.lo()) < exact(Interval::PI.hi()) - pi);
    assert_eq!(Real::ONE.get(), 1.0);
    assert_eq!(<Real as Scalar>::pi(), Real::PI);
    assert_eq!(<Interval as Scalar>::pi(), Interval::PI);
}

#[test]
fn a_real_converts_to_its_point_interval_with_one_zero() {
    for v in [1.5, -2.25, f64::MAX, -f64::from_bits(1), 0.0] {
        assert_eq!(Interval::from(r(v)), Interval::point(v).unwrap());
    }
    let z = Interval::from(r(-0.0));
    assert_eq!((z.lo().to_bits(), z.hi().to_bits()), (0, 0));
}

/// On a `Real`, each operation is the rounded basic operation or the kernel's maths function, bit
/// for bit, with its documented errors.
#[test]
fn real_scalar_operations_are_the_kernels() {
    let seed = resolve("attack_g52 real scalar", Seed(0x6752_0000_0000_0001)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..500 {
        let (a, b, cc) = (
            uniform(&mut rng, -50.0, 50.0),
            uniform(&mut rng, -50.0, 50.0),
            uniform(&mut rng, -50.0, 50.0),
        );
        let (ra, rb, rc) = (r(a), r(b), r(cc));
        assert_eq!(Scalar::add(ra, rb).unwrap().get(), a + b);
        assert_eq!(Scalar::sub(ra, rb).unwrap().get(), a - b);
        assert_eq!(Scalar::mul(ra, rb).unwrap().get(), a * b);
        assert_eq!(Scalar::div(ra, rb).unwrap().get(), a / b);
        assert_eq!(Scalar::neg(ra).get(), -a);
        assert_eq!(Scalar::square(ra).unwrap().get(), a * a);
        assert_eq!(
            Scalar::mul_add(ra, rb, rc).unwrap(),
            math::fma(a, b, cc).unwrap()
        );
        assert_eq!(
            Scalar::sqrt(r(a.abs())).unwrap(),
            math::sqrt(a.abs()).unwrap()
        );
        assert_eq!(Scalar::exp(ra).unwrap(), math::exp(a).unwrap());
        assert_eq!(
            Scalar::ln(r(a.abs() + 1.0)).unwrap(),
            math::ln(a.abs() + 1.0).unwrap()
        );
        assert_eq!(Scalar::sinpi(ra).unwrap(), math::sinpi(a).unwrap());
        assert_eq!(Scalar::cospi(ra).unwrap(), math::cospi(a).unwrap());
        assert_eq!(Scalar::atan2(ra, rb).unwrap(), math::atan2(a, b).unwrap());
        assert_eq!(<Real as Scalar>::constant(ra), ra);
    }
    let big = r(f64::MAX);
    assert_eq!(Scalar::add(big, big), Err(NumError::NotFinite));
    assert_eq!(Scalar::sub(big, big.neg()), Err(NumError::NotFinite));
    assert_eq!(Scalar::mul(big, r(2.0)), Err(NumError::NotFinite));
    assert_eq!(Scalar::div(r(1.0), r(0.0)), Err(NumError::NotFinite));
    assert_eq!(Scalar::div(r(0.0), r(0.0)), Err(NumError::NotFinite));
    assert_eq!(Scalar::square(big), Err(NumError::NotFinite));
    assert_eq!(Scalar::mul_add(big, big, r(0.0)), Err(NumError::NotFinite));
    assert_eq!(Scalar::sqrt(r(-1.0)), Err(NumError::OutsideDomain));
    assert_eq!(Scalar::ln(r(-1.0)), Err(NumError::OutsideDomain));
    assert_eq!(Scalar::ln(r(0.0)), Err(NumError::NotFinite));
    assert_eq!(Scalar::exp(r(710.0)), Err(NumError::NotFinite));
    assert_eq!(Scalar::neg(r(0.0)).get().to_bits(), (-0.0f64).to_bits());
}

/// On an `Interval`, each operation is the interval method of the same name.
#[test]
fn interval_scalar_operations_are_the_interval_methods() {
    let seed = resolve("attack_g52 interval scalar", Seed(0x6752_0000_0000_0002)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..300 {
        let mut pick = |lo: f64, hi: f64| {
            let (u, v) = (uniform(&mut rng, lo, hi), uniform(&mut rng, lo, hi));
            if u < v { iv(u, v) } else { iv(v, u) }
        };
        let (a, b, cc, p) = (
            pick(-20.0, 20.0),
            pick(-20.0, 20.0),
            pick(-20.0, 20.0),
            pick(0.5, 20.0),
        );
        assert_eq!(Scalar::add(a, b), a.add(b));
        assert_eq!(Scalar::sub(a, b), a.sub(b));
        assert_eq!(Scalar::mul(a, b), a.mul(b));
        assert_eq!(Scalar::div(a, p), a.div(p));
        assert_eq!(Scalar::div(a, b), a.div(b));
        assert_eq!(Scalar::neg(a), a.neg());
        assert_eq!(Scalar::mul_add(a, b, cc), a.mul_add(b, cc));
        assert_eq!(Scalar::square(a), a.square());
        assert_eq!(Scalar::sqrt(p), p.sqrt());
        assert_eq!(Scalar::sqrt(a), a.sqrt());
        assert_eq!(Scalar::exp(a), a.exp());
        assert_eq!(Scalar::ln(p), p.ln());
        assert_eq!(Scalar::ln(a), a.ln());
        assert_eq!(Scalar::sinpi(a), a.sinpi());
        assert_eq!(Scalar::cospi(a), a.cospi());
        assert_eq!(Scalar::atan2(a, b), a.atan2(b));
        assert_eq!(
            <Interval as Scalar>::constant(r(a.lo())),
            Interval::point(a.lo()).unwrap()
        );
    }
}

// Dual numbers.

#[test]
fn variables_are_the_unit_directions() {
    let x = [r(1.5), r(-2.0), r(0.25)];
    let vars = Dual::variables(x);
    for (i, v) in vars.iter().enumerate() {
        assert_eq!(v.value(), x[i]);
        for (j, p) in v.partials().iter().enumerate() {
            assert_eq!(p.get(), if i == j { 1.0 } else { 0.0 });
        }
    }
    let d = Dual::new(r(3.0), [r(1.0), r(2.0)]);
    assert_eq!((d.value(), d.partials()), (r(3.0), [r(1.0), r(2.0)]));
    let k: Dual<Real, 2> = Scalar::constant(r(7.0));
    assert_eq!((k.value(), k.partials()), (r(7.0), [r(0.0), r(0.0)]));
    let p: Dual<Interval, 1> = Scalar::pi();
    assert_eq!(
        (p.value(), p.partials()),
        (Interval::PI, [Interval::point(0.0).unwrap()])
    );
    let n = Scalar::neg(Dual::new(r(3.0), [r(1.0), r(-2.0)]));
    assert_eq!((n.value(), n.partials()), (r(-3.0), [r(-1.0), r(2.0)]));
}

/// Central differences of `f` at `x` along each direction, with steps of 2^−17 times the scale
/// (1 or |xⱼ|, the larger).
fn central<const N: usize, const M: usize>(
    f: &dyn Fn([Real; N]) -> Result<[Real; M], NumError>,
    x: [f64; N],
) -> [[f64; N]; M] {
    let mut out = [[0.0; N]; M];
    for j in 0..N {
        let scale = if x[j].abs() > 1.0 { x[j].abs() } else { 1.0 };
        let h = scale / 131_072.0;
        let (mut up, mut down) = (x, x);
        up[j] += h;
        down[j] -= h;
        let fu = f(up.map(r)).unwrap();
        let fd = f(down.map(r)).unwrap();
        for i in 0..M {
            out[i][j] = (fu[i].get() - fd[i].get()) / (2.0 * h);
        }
    }
    out
}

fn assert_close<const N: usize, const M: usize>(
    jac: [[Real; N]; M],
    fd: [[f64; N]; M],
    what: &str,
) {
    for i in 0..M {
        for j in 0..N {
            let (d, e) = (jac[i][j].get(), fd[i][j]);
            assert!(
                (d - e).abs() <= 1e-6 * (1.0 + d.abs()),
                "{what}: ∂{i}/∂{j} is {d:e}, central differences give {e:e}"
            );
        }
    }
}

/// Each elemental function's derivative rule, alone and composed, against central differences of
/// the same code.
#[test]
fn derivatives_match_central_differences() {
    let seed = resolve(
        "attack_g52 central differences",
        Seed(0x6752_0000_0000_0003),
    )
    .unwrap();
    let mut rng = SplitMix64::new(seed);
    type F2 = fn([Dual<Real, 2>; 2]) -> Result<[Dual<Real, 2>; 1], NumError>;
    let cases: [(&str, F2, fn([Real; 2]) -> Result<[Real; 1], NumError>); 13] = [
        ("add", |[a, b]| Ok([a.add(b)?]), |[a, b]| Ok([a.add(b)?])),
        ("sub", |[a, b]| Ok([a.sub(b)?]), |[a, b]| Ok([a.sub(b)?])),
        ("mul", |[a, b]| Ok([a.mul(b)?]), |[a, b]| Ok([a.mul(b)?])),
        ("div", |[a, b]| Ok([a.div(b)?]), |[a, b]| Ok([a.div(b)?])),
        (
            "mul_add",
            |[a, b]| Ok([a.mul_add(b, a.mul(a)?)?]),
            |[a, b]| Ok([a.mul_add(b, a.mul(a)?)?]),
        ),
        (
            "square",
            |[a, b]| Ok([a.mul(b)?.square()?]),
            |[a, b]| Ok([a.mul(b)?.square()?]),
        ),
        (
            "sqrt",
            |[a, b]| Ok([a.mul(b)?.sqrt()?]),
            |[a, b]| Ok([a.mul(b)?.sqrt()?]),
        ),
        (
            "exp",
            |[a, b]| Ok([a.div(b)?.exp()?]),
            |[a, b]| Ok([a.div(b)?.exp()?]),
        ),
        (
            "ln",
            |[a, b]| Ok([a.mul(b)?.ln()?]),
            |[a, b]| Ok([a.mul(b)?.ln()?]),
        ),
        (
            "sinpi",
            |[a, b]| Ok([a.mul(b)?.sinpi()?]),
            |[a, b]| Ok([a.mul(b)?.sinpi()?]),
        ),
        (
            "cospi",
            |[a, b]| Ok([a.mul(b)?.cospi()?]),
            |[a, b]| Ok([a.mul(b)?.cospi()?]),
        ),
        (
            "atan2",
            |[a, b]| Ok([a.atan2(b)?]),
            |[a, b]| Ok([a.atan2(b)?]),
        ),
        (
            "neg",
            |[a, b]| Ok([a.neg().mul(b)?]),
            |[a, b]| Ok([a.neg().mul(b)?]),
        ),
    ];
    for _ in 0..200 {
        // Positive operands keep the square root and the logarithm in their domains, and the
        // angle away from its cut.
        let x = [uniform(&mut rng, 0.25, 3.0), uniform(&mut rng, 0.25, 3.0)];
        for (name, dual, real) in &cases {
            let (values, jac) = jacobian(dual, x.map(r)).unwrap();
            assert_eq!(
                values,
                real(x.map(r)).unwrap(),
                "{name}: the value is the real one"
            );
            assert_close(jac, central(&|v| real(v), x), name);
        }
    }
}

/// The systems' Jacobians, from the residuals' one text, against central differences.
#[test]
fn system_jacobians_match_central_differences() {
    let seed = resolve("attack_g52 system jacobians", Seed(0x6752_0000_0000_0004)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..100 {
        let x = [uniform(&mut rng, 0.5, 2.0), uniform(&mut rng, 0.5, 2.0)];
        let (_, j) = jacobian(|v| CircleLine.residuals(v), x.map(r)).unwrap();
        assert_close(
            j,
            central(&|v| CircleLine.residuals(v), x),
            "circle and line",
        );
        let (_, j) = jacobian(|v| CosLine.residuals(v), x.map(r)).unwrap();
        assert_close(j, central(&|v| CosLine.residuals(v), x), "cosine and line");
        let x3 = [x[0], x[1], uniform(&mut rng, 0.5, 2.0)];
        let (_, j) = jacobian(|v| Three.residuals(v), x3.map(r)).unwrap();
        assert_close(j, central(&|v| Three.residuals(v), x3), "three");
        let x4 = [x[0] - 1.0, x[1], x3[2], uniform(&mut rng, 0.5, 2.0)];
        let (_, j) = jacobian(|v| Four.residuals(v), x4.map(r)).unwrap();
        assert_close(j, central(&|v| Four.residuals(v), x4), "four");
    }
}

/// A function of two unknowns with three outputs: the Jacobian is 3 × 2.
#[test]
fn a_rectangular_jacobian() {
    let (values, j) = jacobian(
        |[x, y]: [Dual<Real, 2>; 2]| Ok([x.mul(y)?, x.add(y)?, x.square()?]),
        [r(2.0), r(3.0)],
    )
    .unwrap();
    assert_eq!(values, [r(6.0), r(5.0), r(4.0)]);
    assert_eq!(j, [[r(3.0), r(2.0)], [r(1.0), r(1.0)], [r(4.0), r(0.0)]]);
}

fn within(inner: Interval, outer: Interval) -> bool {
    inner.lo() >= outer.lo() && inner.hi() <= outer.hi()
}

/// Over a box, a dual interval encloses the dual interval over any point of the box, and that
/// encloses the dual real there: the interval Jacobian holds the derivative everywhere in the box.
#[test]
fn interval_jacobians_enclose_point_jacobians() {
    let seed = resolve("attack_g52 interval jacobians", Seed(0x6752_0000_0000_0005)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..300 {
        let lo = [
            uniform(&mut rng, 0.5, 2.0),
            uniform(&mut rng, 0.5, 2.0),
            uniform(&mut rng, 0.5, 2.0),
        ];
        let width = uniform(&mut rng, 0.0, 0.3);
        let b = lo.map(|v| iv(v, v + width));
        let (bv, bj) = jacobian(|v| Three.residuals(v), b).unwrap();
        for _ in 0..5 {
            let p = [0, 1, 2].map(|k| uniform(&mut rng, b[k].lo(), b[k].hi()));
            let (pv, pj) = jacobian(
                |v| Three.residuals(v),
                p.map(|v| Interval::point(v).unwrap()),
            )
            .unwrap();
            let (rv, rj) = jacobian(|v| Three.residuals(v), p.map(r)).unwrap();
            for i in 0..3 {
                assert!(within(pv[i], bv[i]), "value {i} at {p:?}");
                assert!(pv[i].contains(rv[i].get()), "real value {i} at {p:?}");
                for k in 0..3 {
                    assert!(within(pj[i][k], bj[i][k]), "∂{i}/∂{k} at {p:?} in {b:?}");
                    assert!(pj[i][k].contains(rj[i][k].get()), "real ∂{i}/∂{k} at {p:?}");
                }
            }
        }
        // The four-unknown system exercises exp, ln, atan2 and the square root the same way.
        let b4 = [iv(-0.2, -0.2 + width), b[0], b[1], b[2]];
        let (_, bj4) = jacobian(|v| Four.residuals(v), b4).unwrap();
        let p4 = [0, 1, 2, 3].map(|k| uniform(&mut rng, b4[k].lo(), b4[k].hi()));
        let (_, rj4) = jacobian(|v| Four.residuals(v), p4.map(r)).unwrap();
        for i in 0..4 {
            for k in 0..4 {
                assert!(
                    bj4[i][k].contains(rj4[i][k].get()),
                    "four: ∂{i}/∂{k} at {p4:?}"
                );
            }
        }
    }
}

/// The tripwire: on dual reals, a NaN or an infinity in a value or a derivative is `NotFinite`
/// from the operation that made it; on dual intervals, a derivative over a box where the function
/// is not differentiable is refused.
#[test]
fn the_tripwire_and_the_refusals() {
    let x = |v: f64| Dual::<Real, 1>::variables([r(v)])[0];
    // The square root's slope at zero is infinite.
    assert_eq!(x(0.0).sqrt(), Err(NumError::NotFinite));
    // The logarithm's slope 1/x overflows at the smallest subnormal.
    assert_eq!(x(f64::from_bits(1)).ln(), Err(NumError::NotFinite));
    // The angle's slope at the origin is 0/0.
    let (y0, x0) = (
        Dual::<Real, 2>::variables([r(0.0), r(0.0)])[0],
        Dual::<Real, 2>::variables([r(0.0), r(0.0)])[1],
    );
    assert_eq!(y0.atan2(x0), Err(NumError::NotFinite));
    // A quotient by zero, an overflowing value, an overflowing derivative.
    assert_eq!(x(1.0).div(x(0.0)), Err(NumError::NotFinite));
    assert_eq!(x(800.0).exp(), Err(NumError::NotFinite));
    let steep = Dual::<Real, 1>::new(r(1.0), [r(f64::MAX)]);
    assert_eq!(
        steep.mul(Dual::new(r(4.0), [r(0.0)])),
        Err(NumError::NotFinite)
    );
    assert_eq!(steep.square(), Err(NumError::NotFinite));
    assert_eq!(steep.add(steep), Err(NumError::NotFinite));
    assert_eq!(steep.sub(steep.neg()), Err(NumError::NotFinite));
    assert_eq!(
        steep.mul_add(Dual::new(r(4.0), [r(0.0)]), steep),
        Err(NumError::NotFinite)
    );
    assert_eq!(
        steep.div(Dual::new(r(0.25), [r(0.0)])),
        Err(NumError::NotFinite)
    );
    assert_eq!(steep.exp(), Err(NumError::NotFinite));
    assert_eq!(steep.sinpi(), Err(NumError::NotFinite));
    assert_eq!(
        Dual::<Real, 1>::new(r(0.5), [r(f64::MAX)]).cospi(),
        Err(NumError::NotFinite)
    );
    assert_eq!(
        Dual::<Real, 1>::new(r(0.5), [r(f64::MAX)]).ln(),
        Err(NumError::NotFinite)
    );
    assert_eq!(
        Dual::<Real, 1>::new(r(0.0625), [r(f64::MAX)]).sqrt(),
        Err(NumError::NotFinite)
    );
    let tiny_angle = Dual::<Real, 1>::new(r(1e-300), [r(f64::MAX)]);
    assert_eq!(
        tiny_angle.atan2(Dual::new(r(1e-300), [r(0.0)])),
        Err(NumError::NotFinite)
    );
    // A value that fails before its derivative is formed.
    assert_eq!(x(-1.0).sqrt(), Err(NumError::OutsideDomain));
    assert_eq!(x(-1.0).ln(), Err(NumError::OutsideDomain));
    assert_eq!(
        steep.mul(Dual::new(r(f64::MAX), [r(0.0)])).map(|_| ()),
        Err(NumError::NotFinite)
    );

    // Dual intervals: the square root at zero and the angle at the origin divide by an interval
    // holding zero; the logarithm refuses a box reaching zero; the angle refuses its cut.
    let b = |lo: f64, hi: f64| Dual::<Interval, 2>::variables([iv(lo, hi), iv(lo, hi)])[0];
    assert_eq!(b(0.0, 1.0).sqrt(), Err(NumError::DivisorContainsZero));
    assert_eq!(b(0.0, 1.0).ln(), Err(NumError::NotFinite));
    let [y, xx] = Dual::<Interval, 2>::variables([iv(-1.0, 1.0), iv(-2.0, -1.0)]);
    assert_eq!(y.atan2(xx), Err(NumError::OutsideDomain), "across the cut");
    let [y, xx] = Dual::<Interval, 2>::variables([iv(-1.0, 0.0), iv(-2.0, -1.0)]);
    assert_eq!(
        y.atan2(xx),
        Err(NumError::OutsideDomain),
        "touching the cut from below"
    );
    let [y, xx] = Dual::<Interval, 2>::variables([iv(0.0, 1.0), iv(-2.0, -1.0)]);
    assert!(
        y.atan2(xx).is_ok(),
        "the closed upper half-plane is smooth up to the cut"
    );
    let [y, xx] = Dual::<Interval, 2>::variables([iv(-1.0, -0.5), iv(-2.0, -1.0)]);
    assert!(y.atan2(xx).is_ok(), "the open lower half-plane");
    let [y, xx] = Dual::<Interval, 2>::variables([iv(-1.0, 1.0), iv(0.0, 1.0)]);
    assert_eq!(
        y.atan2(xx),
        Err(NumError::DivisorContainsZero),
        "the origin on the box's edge"
    );
    let [y, xx] = Dual::<Interval, 2>::variables([iv(-1.0, 1.0), iv(0.5, 1.0)]);
    assert!(
        y.atan2(xx).is_ok(),
        "the right half-plane, crossing the x-axis"
    );
    let [y, xx] = Dual::<Interval, 2>::variables([iv(-1.0, 1.0), iv(-0.5, 1.0)]);
    assert_eq!(
        y.atan2(xx),
        Err(NumError::OutsideDomain),
        "x reaching below zero across the axis"
    );
    let [y, xx] = Dual::<Interval, 2>::variables([iv(-1.0, -0.5), iv(-0.5, 1.0)]);
    assert!(
        y.atan2(xx).is_ok(),
        "x reaching below zero, y below the axis"
    );
}

// The operators.

const OPS1: [(&str, Op<Sqrt2, 1>); 2] =
    [("krawczyk", krawczyk), ("hansen_sengupta", hansen_sengupta)];

/// x² = 2 from [1, 2] and [−2, −1]: a unique zero proven, holding √2 or −√2 exactly.
#[test]
fn the_root_of_two_is_proven() {
    let two = BigRational::from_integer(BigInt::from(2));
    for (name, op) in OPS1 {
        for sign in [1.0, -1.0] {
            let b = if sign > 0.0 {
                [iv(1.0, 2.0)]
            } else {
                [iv(-2.0, -1.0)]
            };
            let [e] = prove(op, &Sqrt2, b, 6).unwrap_or_else(|| panic!("{name}"));
            let (lo, hi) = (exact(e.lo()), exact(e.hi()));
            if sign > 0.0 {
                assert!(
                    lo.clone() * lo < two && hi.clone() * hi > two,
                    "{name}: {e:?}"
                );
            } else {
                assert!(
                    lo.clone() * lo > two && hi.clone() * hi < two,
                    "{name}: {e:?}"
                );
            }
            let [e] = refine(op, &Sqrt2, [e], 20);
            assert!(e.hi() - e.lo() < 1e-14, "{name} converges: {e:?}");
        }
    }
}

/// One Krawczyk step on [1, 2] is K = m − C·f(m) + (1 − C·[2, 4])·(X − m) with m = 3/2 and
/// C = 1/3: [5/4, 19/12] and a contraction of 1/3, to rounding.
#[test]
fn one_krawczyk_step_is_the_formula() {
    let Verdict::Unique {
        enclosure: [k],
        contraction,
    } = krawczyk(&Sqrt2, [iv(1.0, 2.0)]).unwrap()
    else {
        panic!("not proven")
    };
    assert!((k.lo() - 1.25).abs() < 1e-14, "{k:?}");
    assert!((k.hi() - 19.0 / 12.0).abs() < 1e-14, "{k:?}");
    let third = contraction.get();
    assert!(third >= 1.0 / 3.0 && third < 1.0 / 3.0 + 1e-14, "{third:e}");
    // Hansen and Sengupta on the same box: d = −C·f(m) / (C·[2, 4]), so m + d is
    // 3/2 − (1/12)/[2/3, 4/3] = [3/2 − 1/8, 3/2 − 1/16].
    let Verdict::Unique {
        enclosure: [h],
        contraction,
    } = hansen_sengupta(&Sqrt2, [iv(1.0, 2.0)]).unwrap()
    else {
        panic!("not proven")
    };
    assert!(
        (h.lo() - 1.375).abs() < 1e-14 && (h.hi() - 1.4375).abs() < 1e-14,
        "{h:?}"
    );
    assert_eq!(contraction.get(), third);
}

fn both<R: Residuals<N>, const N: usize>() -> [(&'static str, Op<R, N>); 2] {
    [("krawczyk", krawczyk), ("hansen_sengupta", hansen_sengupta)]
}

/// Known zeros in two, three and four unknowns, each proven from a box around it, with the exact
/// zero inside the proven box.
#[test]
fn known_zeros_are_proven_in_two_to_four_unknowns() {
    for (name, op) in both::<CircleLine, 2>() {
        for root in [[4.0, 3.0], [-3.0, -4.0]] {
            let e = prove(op, &CircleLine, boxed(root, 0.5), 8).unwrap_or_else(|| panic!("{name}"));
            assert!(contains_point(&e, root), "{name}: {e:?}");
        }
    }
    for (name, op) in both::<CosLine, 2>() {
        let e = prove(op, &CosLine, boxed([0.5, 0.0], 0.2), 8).unwrap_or_else(|| panic!("{name}"));
        assert!(contains_point(&e, [0.5, 0.0]), "{name}: {e:?}");
    }
    for (name, op) in both::<Three, 3>() {
        let e =
            prove(op, &Three, boxed([1.0, 2.0, 1.0], 0.1), 8).unwrap_or_else(|| panic!("{name}"));
        assert!(contains_point(&e, [1.0, 2.0, 1.0]), "{name}: {e:?}");
    }
    for (name, op) in both::<Four, 4>() {
        let e = prove(op, &Four, boxed([0.0, 1.0, 1.0, 1.0], 0.1), 8)
            .unwrap_or_else(|| panic!("{name}"));
        assert!(contains_point(&e, [0.0, 1.0, 1.0, 1.0]), "{name}: {e:?}");
        let e = refine(op, &Four, e, 20);
        assert!(contains_point(&e, [0.0, 1.0, 1.0, 1.0]), "{name}: {e:?}");
        assert!(width(&e) < 1e-13, "{name} converges: {e:?}");
    }
}

/// Boxes with no zero: excluded by the residuals' range, or by the operator's image.
#[test]
fn empty_boxes_are_proven_empty() {
    for (name, op) in both::<CircleLine, 2>() {
        // The line x − y = 1 misses this box entirely: F₂(X) excludes zero.
        assert_eq!(
            op(&CircleLine, [iv(0.0, 1.0), iv(2.0, 3.0)]).unwrap(),
            Verdict::NoZero,
            "{name}"
        );
        // Both residuals' ranges hold zero, the circle and the line each cross the box, but not
        // at a common point: the operator's image misses the box.
        let b = [iv(3.5, 3.9), iv(2.8, 3.2)];
        let (v, _) = jacobian(|v| CircleLine.residuals(v), b).unwrap();
        assert!(v.iter().all(|i| i.contains(0.0)), "{v:?}");
        assert_eq!(op(&CircleLine, b).unwrap(), Verdict::NoZero, "{name}");
    }
}

/// x² = 0 (a double root), x³ = 0 (a triple one), two equations that are one, and a box holding
/// both zeros of the circle and the line: refused as singular, or narrowed, never called unique.
#[test]
fn singular_systems_are_refused() {
    struct Double;
    impl Residuals<1> for Double {
        fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
            Ok([x.square()?])
        }
    }
    struct Triple;
    impl Residuals<1> for Triple {
        fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
            Ok([x.square()?.mul(x)?])
        }
    }
    struct Dependent;
    impl Residuals<2> for Dependent {
        fn residuals<S: Scalar>(&self, [x, y]: [S; 2]) -> Result<[S; 2], NumError> {
            let s = x.add(y)?.sub(c(1.0))?;
            Ok([s, s.add(s)?])
        }
    }
    for (name, op) in both::<Double, 1>() {
        assert_eq!(
            op(&Double, [iv(-1.0, 1.0)]),
            Err(NewtonError::Singular),
            "{name}"
        );
        for b in [iv(-0.5, 1.0), iv(-0.25, 0.125), iv(-1e-3, 3e-3)] {
            never_unique(op, &Double, [b], [0.0], name);
        }
    }
    for (name, op) in both::<Triple, 1>() {
        for b in [iv(-0.5, 1.0), iv(-1.0, 0.75)] {
            never_unique(op, &Triple, [b], [0.0], name);
        }
    }
    for (name, op) in both::<Dependent, 2>() {
        assert_eq!(
            op(&Dependent, boxed([0.5, 0.5], 0.25)),
            Err(NewtonError::Singular),
            "{name}"
        );
    }
    for (name, op) in both::<CircleLine, 2>() {
        never_unique(op, &CircleLine, boxed([0.5, -0.5], 5.0), [4.0, 3.0], name);
        never_unique(
            op,
            &CircleLine,
            [iv(-3.5, 4.5), iv(-4.25, 3.5)],
            [4.0, 3.0],
            name,
        );
    }
}

/// Twenty steps of `op` from `b`: never `Unique`, never `NoZero`, and `zero` stays inside, until
/// the operator refuses the box as singular.
fn never_unique<R: Residuals<N>, const N: usize>(
    op: Op<R, N>,
    f: &R,
    mut b: [Interval; N],
    zero: [f64; N],
    name: &str,
) {
    for _ in 0..20 {
        match op(f, b) {
            Ok(Verdict::Narrowed { enclosure, .. }) => {
                assert!(contains_point(&enclosure, zero), "{name}: lost the zero");
                b = enclosure;
            }
            Err(NewtonError::Singular) => return,
            other => panic!("{name}: {other:?} from {b:?}"),
        }
    }
}

/// Seeded random systems in one to four unknowns with a known zero: no step ever loses it, a
/// step's enclosure lies inside its box, and a small box around it is proven within a few steps.
#[test]
fn no_zero_is_lost_and_small_boxes_are_proven() {
    fn run<const N: usize>(rng: &mut SplitMix64) {
        for _ in 0..150 {
            let f = Random::<N>::new(rng);
            for (name, op) in both::<Random<N>, N>() {
                // Wide boxes, off centre: every enclosure keeps the zero.
                let mut b = f.r.map(|v| {
                    let (lo, hi) = (uniform(rng, 0.0, 0.5), uniform(rng, 0.0, 0.5));
                    iv(v - lo, v + hi)
                });
                for _ in 0..6 {
                    match op(&f, b) {
                        Ok(
                            Verdict::Unique {
                                enclosure,
                                contraction,
                            }
                            | Verdict::Narrowed {
                                enclosure,
                                contraction,
                            },
                        ) => {
                            assert!(
                                contains_point(&enclosure, f.r),
                                "{name} lost {:?} from {b:?}",
                                f.r
                            );
                            assert!(
                                inside(&enclosure, &b),
                                "{name}: {enclosure:?} outside {b:?}"
                            );
                            assert!(contraction.get() >= 0.0);
                            b = enclosure;
                        }
                        Ok(Verdict::NoZero) => panic!("{name}: {b:?} holds {:?}", f.r),
                        Err(e) => panic!("{name}: {e:?} on {b:?}"),
                    }
                }
                // A small box: proven.
                let half = uniform(rng, 1e-6, 1e-3);
                let e = prove(op, &f, boxed(f.r, half), 6)
                    .unwrap_or_else(|| panic!("{name}: {f:?} not proven from half-width {half:e}"));
                assert!(contains_point(&e, f.r), "{name}");
            }
        }
    }
    let seed = resolve("attack_g52 random systems", Seed(0x6752_0000_0000_0006)).unwrap();
    let mut rng = SplitMix64::new(seed);
    run::<1>(&mut rng);
    run::<2>(&mut rng);
    run::<3>(&mut rng);
    run::<4>(&mut rng);
}

/// A linear system: the Jacobian is exact, the preconditioner inverts it, and the contraction is
/// rounding alone. Its matrix needs a row exchange (a zero first pivot), and a tie between two
/// pivots of equal size keeps the first.
#[test]
fn linear_systems_and_pivoting() {
    struct Linear<const N: usize>([[f64; N]; N], [f64; N]);
    impl<const N: usize> Residuals<N> for Linear<N> {
        fn residuals<S: Scalar>(&self, x: [S; N]) -> Result<[S; N], NumError> {
            let mut out = [c::<S>(0.0); N];
            for i in 0..N {
                let mut acc = c::<S>(-self.1[i]);
                for j in 0..N {
                    acc = c::<S>(self.0[i][j]).mul_add(x[j], acc)?;
                }
                out[i] = acc;
            }
            Ok(out)
        }
    }
    // y = 2, x = 3: a zero first pivot, so the rows are exchanged.
    let swap = Linear([[0.0, 1.0], [1.0, 0.0]], [2.0, 3.0]);
    // x + 2y = 5, −x + 3y = 0: a tie of first pivots, |1| = |−1|.
    let tie = Linear([[1.0, 2.0], [-1.0, 3.0]], [5.0, 0.0]);
    // A 4 × 4 with its largest first-column entry last.
    let four = Linear(
        [
            [1.0, 2.0, 0.0, 1.0],
            [2.0, 4.0, 1.0, 0.0],
            [0.0, 1.0, 3.0, 1.0],
            [4.0, 1.0, 0.0, 2.0],
        ],
        [4.0, 7.0, 5.0, 7.0],
    );
    let zero4 = [1.0, 1.0, 1.0, 1.0];
    for (name, op) in both::<Linear<2>, 2>() {
        for (f, z) in [(&swap, [3.0, 2.0]), (&tie, [3.0, 1.0])] {
            let Verdict::Unique {
                enclosure,
                contraction,
            } = op(f, boxed(z, 1.0)).unwrap()
            else {
                panic!("{name}")
            };
            assert!(contains_point(&enclosure, z), "{name}: {enclosure:?}");
            assert!(
                enclosure.iter().all(|i| i.hi() - i.lo() < 1e-14),
                "{name}: {enclosure:?}"
            );
            assert!(contraction.get() < 1e-14, "{name}: {contraction:?}");
        }
    }
    for (name, op) in both::<Linear<4>, 4>() {
        let Verdict::Unique {
            enclosure,
            contraction,
        } = op(&four, boxed(zero4, 1.0)).unwrap()
        else {
            panic!("{name}")
        };
        assert!(contains_point(&enclosure, zero4), "{name}: {enclosure:?}");
        assert!(contraction.get() < 1e-14, "{name}");
    }
}

/// Krawczyk's step in one unknown, written out from its module documentation with the public
/// operations, in the order the operator applies them.
fn krawczyk_by_hand<R: Residuals<1>>(f: &R, x: Interval) -> Interval {
    let mid = |i: Interval| {
        let m = 0.5 * i.lo() + 0.5 * i.hi();
        if m < i.lo() {
            i.lo()
        } else if m > i.hi() {
            i.hi()
        } else {
            m
        }
    };
    let (_, [[j]]) = jacobian(|v| f.residuals(v), [x]).unwrap();
    let m = Interval::point(mid(x)).unwrap();
    let [fm] = f.residuals([m]).unwrap();
    let c = Interval::point(1.0 / mid(j)).unwrap();
    let zero = Interval::point(0.0).unwrap();
    let a = c.mul_add(j, zero).unwrap();
    let b = c.mul_add(fm, zero).unwrap();
    let rest = Interval::point(1.0).unwrap().sub(a).unwrap();
    rest.mul_add(x.sub(m).unwrap(), m.sub(b).unwrap()).unwrap()
}

/// One step in one unknown is the formula, bit for bit, on seeded boxes around √2 and around the
/// zero of a line.
#[test]
fn a_krawczyk_step_in_one_unknown_is_the_formula_bit_for_bit() {
    struct Line;
    impl Residuals<1> for Line {
        fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
            Ok([x.mul(c(3.0))?.sub(c(1.0))?])
        }
    }
    let seed = resolve("attack_g52 one unknown", Seed(0x6752_0000_0000_0007)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..500 {
        let (lo, hi) = (uniform(&mut rng, 0.0, 0.5), uniform(&mut rng, 0.0, 0.5));
        let b = iv(2f64.sqrt() - lo, 2f64.sqrt() + hi);
        let k = krawczyk_by_hand(&Sqrt2, b);
        match krawczyk(&Sqrt2, [b]).unwrap() {
            Verdict::Unique { enclosure: [e], .. } => assert_eq!(e, k, "{b:?}"),
            Verdict::Narrowed { enclosure: [e], .. } => {
                let lo = if k.lo() > b.lo() { k.lo() } else { b.lo() };
                let hi = if k.hi() < b.hi() { k.hi() } else { b.hi() };
                assert_eq!(e, iv(lo, hi), "{b:?}");
            }
            Verdict::NoZero => panic!("{b:?}"),
        }
        let b = iv(1.0 / 3.0 - lo, 1.0 / 3.0 + hi);
        let Verdict::Unique { enclosure: [e], .. } = krawczyk(&Line, [b]).unwrap() else {
            panic!("{b:?}")
        };
        assert_eq!(e, krawczyk_by_hand(&Line, b), "{b:?}");
    }
}

/// The containment test is strict: an image sharing a bound with the box, and otherwise inside
/// it, is narrowed, not proven. Such a box is found by moving the box's bound onto the step's own
/// image of it until the two agree exactly.
#[test]
fn an_image_touching_the_box_is_not_proof() {
    struct Half;
    impl Residuals<1> for Half {
        fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
            Ok([x.add(x)?.sub(c(1.0))?])
        }
    }
    for lower in [true, false] {
        let mut b = iv(0.0, 1.0);
        let mut found = None;
        for _ in 0..64 {
            let k = krawczyk_by_hand(&Half, b);
            let touching = if lower {
                k.lo() == b.lo() && k.hi() < b.hi()
            } else {
                k.hi() == b.hi() && k.lo() > b.lo()
            };
            if touching {
                found = Some(b);
                break;
            }
            b = if lower {
                iv(k.lo(), 1.0)
            } else {
                iv(0.0, k.hi())
            };
        }
        let b = found.unwrap_or_else(|| panic!("no touching box found (lower bound: {lower})"));
        let Verdict::Narrowed { enclosure: [e], .. } = krawczyk(&Half, [b]).unwrap() else {
            panic!("an image touching {b:?} was called a proof")
        };
        assert_eq!(e, krawczyk_by_hand(&Half, b));
    }
}

/// Degenerate boxes at the smallest subnormals, where halving the bounds rounds the midpoint out
/// of the box: the midpoint is moved back in, and the step narrows to the box.
#[test]
fn midpoints_of_subnormal_point_boxes() {
    struct Shift(f64);
    impl Residuals<1> for Shift {
        fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
            Ok([x.sub(c(self.0))?])
        }
    }
    for v in [
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::from_bits(3),
        -f64::from_bits(3),
    ] {
        let b = [iv(v, v)];
        for (name, op) in both::<Shift, 1>() {
            assert_eq!(
                op(&Shift(v), b).unwrap(),
                Verdict::Narrowed {
                    enclosure: b,
                    contraction: krawczyk_contraction(&Shift(v), b)
                },
                "{name} at {v:e}"
            );
        }
    }
}

fn krawczyk_contraction<R: Residuals<1>>(f: &R, b: [Interval; 1]) -> Real {
    match krawczyk(f, b).unwrap() {
        Verdict::Unique { contraction, .. } | Verdict::Narrowed { contraction, .. } => contraction,
        Verdict::NoZero => panic!(),
    }
}

/// Errors: a residual that fails is returned as it is, and every error reads as documented.
#[test]
fn errors_pass_through_and_read_as_documented() {
    struct Log;
    impl Residuals<1> for Log {
        fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
            Ok([x.ln()?])
        }
    }
    struct Huge;
    impl Residuals<2> for Huge {
        fn residuals<S: Scalar>(&self, [x, y]: [S; 2]) -> Result<[S; 2], NumError> {
            Ok([x.mul(c(1e300))?.sub(y)?, y.sub(c(1.0))?])
        }
    }
    for (name, op) in both::<Log, 1>() {
        assert_eq!(
            op(&Log, [iv(-1.0, 2.0)]),
            Err(NewtonError::Num(NumError::OutsideDomain)),
            "{name}"
        );
    }
    for (name, op) in both::<Huge, 2>() {
        assert_eq!(
            op(&Huge, [iv(-1e10, 1e10), iv(0.0, 2.0)]),
            Err(NewtonError::Num(NumError::NotFinite)),
            "{name}"
        );
    }
    assert_eq!(
        NewtonError::Singular.to_string(),
        "the Jacobian at the box's midpoint is singular: the system is refused"
    );
    assert_eq!(
        NewtonError::Num(NumError::NotFinite).to_string(),
        NumError::NotFinite.to_string()
    );
    assert_eq!(
        NewtonError::from(NumError::Degenerate),
        NewtonError::Num(NumError::Degenerate)
    );
}

/// Dual numbers nest: a dual over dual reals carries second derivatives, and a nested dual over
/// intervals refuses `atan2` across its cut, as the inner type does.
#[test]
fn nested_duals_give_second_derivatives() {
    let seed = resolve("attack_g52 nested duals", Seed(0x6752_0000_0000_0008)).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..200 {
        let x = uniform(&mut rng, -3.0, 3.0);
        let inner = Dual::<Real, 1>::variables([r(x)])[0];
        let [outer] = Dual::<Dual<Real, 1>, 1>::variables([inner]);
        // atan(x) = atan2(x, 1): f′ = 1/(1 + x²), f″ = −2x/(1 + x²)².
        let y = outer.atan2(Scalar::constant(r(1.0))).unwrap();
        let [d] = y.partials();
        let s = 1.0 + x * x;
        assert_eq!(y.value().value(), math::atan2(x, 1.0).unwrap(), "{x}");
        assert!((d.value().get() - 1.0 / s).abs() <= 1e-15, "f′ at {x}");
        assert!(
            (d.partials()[0].get() + 2.0 * x / (s * s)).abs() <= 1e-15,
            "f″ at {x}"
        );
        // exp: every derivative is the function.
        let e = outer.exp().unwrap();
        let ex = math::exp(x).unwrap();
        assert_eq!(e.value().value(), ex);
        assert_eq!(e.partials()[0].value(), ex);
        assert_eq!(e.partials()[0].partials()[0], ex);
    }
    let [yi, xi] = Dual::<Interval, 2>::variables([iv(-1.0, 1.0), iv(-2.0, -1.0)]);
    let [y, xx] = Dual::<Dual<Interval, 2>, 2>::variables([yi, xi]);
    assert_eq!(y.atan2(xx), Err(NumError::OutsideDomain));
    let [yi, xi] = Dual::<Interval, 2>::variables([iv(0.5, 1.0), iv(-2.0, -1.0)]);
    let [y, xx] = Dual::<Dual<Interval, 2>, 2>::variables([yi, xi]);
    assert!(y.atan2(xx).is_ok());
}

/// The midpoint is inside the box even where halving the bounds rounds it out, as the operators'
/// proofs need: x/x − 1 has no value at 0, so a midpoint left at 0 (the rounded centre of
/// [2^−1074, 2^−1074]) or at −0 (that of [−2^−1074, −2^−1074]) would fail with
/// `DivisorContainsZero` where the step at the box's own point goes on.
#[test]
fn the_midpoint_stays_in_the_box() {
    struct Ratio;
    impl Residuals<1> for Ratio {
        fn residuals<S: Scalar>(&self, [x]: [S; 1]) -> Result<[S; 1], NumError> {
            Ok([x.div(x)?.sub(c(1.0))?])
        }
    }
    let tiny = f64::from_bits(1);
    for v in [tiny, -tiny] {
        assert_eq!(
            0.5 * v + 0.5 * v,
            0.0,
            "the centre of [{v:e}, {v:e}] rounds to zero"
        );
        let at_point = Ratio.residuals([Interval::point(v).unwrap()]);
        assert!(at_point.is_ok(), "{at_point:?}");
        assert_eq!(
            Ratio.residuals([Interval::point(0.0).unwrap()]),
            Err(NumError::DivisorContainsZero)
        );
        for (name, op) in both::<Ratio, 1>() {
            let step = op(&Ratio, [iv(v, v)]);
            assert_ne!(
                step,
                Err(NewtonError::Num(NumError::DivisorContainsZero)),
                "{name} at {v:e}"
            );
            assert!(
                matches!(step, Err(NewtonError::Singular) | Ok(_)),
                "{name} at {v:e}: {step:?}"
            );
        }
    }
}
