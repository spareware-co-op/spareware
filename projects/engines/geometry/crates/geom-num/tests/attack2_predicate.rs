//! Second adversarial round against `geom_num::predicate`, aimed at its rewrite: a checked fast
//! path in `i128` and `I256`, and behind it an exact total path in `I384` that the module claims
//! holds every intermediate for any `i64` coordinates.
//!
//! The total path cannot be reached through the public API. `Coord` keeps its `i64` private and
//! every constructor checks the range (a test below checks every one), so every `P2` and `P3`
//! passes the fast path. To attack the total path anyway, this file compiles predicate.rs's own
//! code a second time, token for token, in the module `copy` below, and calls its private
//! functions directly. The first test proves the copy is still the module's code: it strips
//! comments and whitespace from both and compares them.
//!
//! Attacks:
//! - The total path against an independent oracle, the Leibniz permutation expansion of the
//!   homogeneous determinant on `num-bigint` (the module's unit tests use cofactor expansion and
//!   tests/`attack_predicate`.rs uses Bareiss elimination), at every combination of extreme `i64`
//!   coordinates and at random coordinates of every magnitude; and every intermediate against the
//!   bound its type declares in the source, symbolically and on those points.
//! - The two paths against each other: equal values on the grid, and off it the configurations
//!   that sit exactly on the fast path's overflow boundaries, where a wrapped value would have the
//!   wrong sign. One of them takes the in-sphere sum to 2^256, past what `I256` holds.
//! - Cocircular and cospherical points in general position at large scale, built from Gaussian
//!   integers and quaternions, with the query point on, one step inside and one step outside.
//! - The documented degenerate identities as exact values on the total path at `i64` scale.
//! - The symmetry laws: as exact values on the total path (permutation parity, reflection, axis
//!   rotation, translation, scaling), and as signs on the public path, where the forced total path
//!   must agree after every transformation.
//! - The stack each predicate needs in the current build profile, measured in child processes
//!   (principle 13: WebAssembly stacks have no guard page).

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "points and values are named a, b, c and so on, as in the formulas they check"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::needless_range_loop,
    clippy::too_many_arguments
)]

use std::fmt::Debug;
use std::sync::OnceLock;

use geom_num::grid::{COORD_MAX, Coord, GridStep, P2, P3, coarsen, refine};
use geom_num::predicate;
use geom_num::{I256, I384, Sign, Wide};
use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};
use proptest::array::uniform;
use proptest::prelude::*;
use proptest::test_runner::Config;

/// The largest grid coordinate magnitude, 2^40 − 1.
const M: i64 = COORD_MAX;
/// The smallest `i64`.
const LO: i64 = i64::MIN;
/// The largest `i64`.
const HI: i64 = i64::MAX;

type Pt2 = [i64; 2];
type Pt3 = [i64; 3];

/// Every property runs with this configuration. Failures are not persisted to a regressions file:
/// this file is the only one the attack may write.
fn cfg(cases: u32) -> Config {
    Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    }
}

// ---------------------------------------------------------------------------------------------
// predicate.rs's code, compiled a second time.
// ---------------------------------------------------------------------------------------------

// The copied code's `use crate::grid::…` lines resolve here, to the crate's own items.
mod grid {
    pub use geom_num::grid::{COORD_BITS, P2, P3};
}
mod sign {
    pub use geom_num::sign::Sign;
}
mod step {
    pub use geom_num::step::Step;
}
mod wide {
    pub use geom_num::wide::{I256, I384};
}
mod error {
    pub use geom_num::error::NumError;
}
mod generated {
    pub use geom_num::generated::{incircle, insphere, orient2d, orient3d};
}

/// predicate.rs's code outside its comments and its test module, compiled again so the attack can
/// call the private fast and total paths, followed by thin wrappers that expose them.
#[allow(clippy::many_single_char_names)]
mod copy {
    use crate::{Pt2, Pt3};

    /// Emits the code it is given, and the same code as the text `CODE`.
    macro_rules! verbatim {
        ($($code:tt)*) => {
            $($code)*

            /// The copied code as text, for the check that it is still predicate.rs's code.
            pub(crate) const CODE: &str = stringify!($($code)*);
        };
    }

    verbatim! {
        use crate::error::NumError;
        use crate::generated;
        use crate::grid::{COORD_BITS, P2, P3};
        use crate::sign::Sign;
        use crate::step::Step;
        use crate::wide::{I256, I384};
        use bounded::Bounded;
        const fn exponent_sum(a: u32, b: u32) -> u32 {
            match a.checked_add(b) {
                Some(sum) => sum,
                None => u32::MAX,
            }
        }
        const FAST_DIFF_BITS: u32 = exponent_sum(COORD_BITS, 1);
        const FAST_PRODUCT_BITS: u32 = exponent_sum(FAST_DIFF_BITS, FAST_DIFF_BITS);
        const FAST_DET2_BITS: u32 = exponent_sum(FAST_PRODUCT_BITS, 1);
        const FAST_LIFT2_BITS: u32 = exponent_sum(FAST_PRODUCT_BITS, 1);
        const FAST_LIFT3_BITS: u32 = exponent_sum(FAST_PRODUCT_BITS, 2);
        const FAST_DET3_BITS: u32 = exponent_sum(exponent_sum(FAST_DIFF_BITS, FAST_DET2_BITS), 2);
        const FAST_INCIRCLE_BITS: u32 = exponent_sum(exponent_sum(FAST_LIFT2_BITS, FAST_DET2_BITS), 2);
        const FAST_INSPHERE_BITS: u32 = exponent_sum(exponent_sum(FAST_LIFT3_BITS, FAST_DET3_BITS), 2);
        const I128_MAGNITUDE_BITS: u32 = match i128::BITS.checked_sub(1) {
            Some(bits) => bits,
            None => 0,
        };
        const I256_MAGNITUDE_BITS: u32 = match u64::BITS.checked_mul(4) {
            Some(bits) => match bits.checked_sub(1) {
                Some(magnitude) => magnitude,
                None => 0,
            },
            None => 0,
        };
        const _: () = {
            assert!(I256::ZERO.to_le_limbs().len() == 4, "I256 has four limbs");
            assert!(I128_MAGNITUDE_BITS == 127 && I256_MAGNITUDE_BITS == 255);
            assert!(
                FAST_DIFF_BITS == 41 && FAST_PRODUCT_BITS == 82,
                "the figures at 40 bits"
            );
            assert!(FAST_DET2_BITS == 83 && FAST_LIFT2_BITS == 83 && FAST_LIFT3_BITS == 84);
            assert!(FAST_DET3_BITS == 126 && FAST_INCIRCLE_BITS == 168 && FAST_INSPHERE_BITS == 212);
            assert!(FAST_DET2_BITS <= I128_MAGNITUDE_BITS, "orient2d fits i128");
            assert!(FAST_DET3_BITS <= I128_MAGNITUDE_BITS, "orient3d fits i128");
            assert!(FAST_LIFT2_BITS <= I128_MAGNITUDE_BITS && FAST_LIFT3_BITS <= I128_MAGNITUDE_BITS);
            assert!(
                FAST_INCIRCLE_BITS <= I256_MAGNITUDE_BITS,
                "incircle fits I256"
            );
            assert!(
                FAST_INSPHERE_BITS <= I256_MAGNITUDE_BITS,
                "insphere fits I256"
            );
        };
        fn diff(p: i64, q: i64) -> Option<i128> {
            i128::from(p).checked_sub(i128::from(q))
        }
        fn det2(a: i128, b: i128, c: i128, d: i128) -> Option<i128> {
            a.checked_mul(d)?.checked_sub(b.checked_mul(c)?)
        }
        #[derive(Clone, Copy)]
        struct D2 {
            x: i128,
            y: i128,
        }
        impl D2 {
            fn sub(p: [i64; 2], q: [i64; 2]) -> Option<D2> {
                let ([px, py], [qx, qy]) = (p, q);
                Some(D2 {
                    x: diff(px, qx)?,
                    y: diff(py, qy)?,
                })
            }
            fn cross(self, other: D2) -> Option<i128> {
                det2(self.x, self.y, other.x, other.y)
            }
            fn norm2(self) -> Option<i128> {
                let xx = self.x.checked_mul(self.x)?;
                xx.checked_add(self.y.checked_mul(self.y)?)
            }
        }
        #[derive(Clone, Copy)]
        struct D3 {
            x: i128,
            y: i128,
            z: i128,
        }
        impl D3 {
            fn sub(p: [i64; 3], q: [i64; 3]) -> Option<D3> {
                let ([px, py, pz], [qx, qy, qz]) = (p, q);
                Some(D3 {
                    x: diff(px, qx)?,
                    y: diff(py, qy)?,
                    z: diff(pz, qz)?,
                })
            }
            fn norm2(self) -> Option<i128> {
                let xx = self.x.checked_mul(self.x)?;
                let yy = self.y.checked_mul(self.y)?;
                let zz = self.z.checked_mul(self.z)?;
                xx.checked_add(yy)?.checked_add(zz)
            }
        }
        fn det3(p: D3, q: D3, r: D3) -> Option<i128> {
            let tx = p.x.checked_mul(det2(q.y, q.z, r.y, r.z)?)?;
            let ty = p.y.checked_mul(det2(q.z, q.x, r.z, r.x)?)?;
            let tz = p.z.checked_mul(det2(q.x, q.y, r.x, r.y)?)?;
            tx.checked_add(ty)?.checked_add(tz)
        }
        fn sum_of_products(terms: &[(i128, i128)]) -> Option<I256> {
            terms.iter().try_fold(I256::ZERO, |sum, &(x, y)| {
                let product = I256::from_i128(x).checked_mul(&I256::from_i128(y))?;
                sum.checked_add(&product)
            })
        }
        fn orient2d_fast(a: [i64; 2], b: [i64; 2], c: [i64; 2]) -> Option<i128> {
            D2::sub(b, a)?.cross(D2::sub(c, a)?)
        }
        fn orient3d_fast(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3]) -> Option<i128> {
            det3(D3::sub(b, a)?, D3::sub(c, a)?, D3::sub(d, a)?)
        }
        fn incircle_terms(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Option<[(i128, i128); 3]> {
            let a = D2::sub(a, d)?;
            let b = D2::sub(b, d)?;
            let c = D2::sub(c, d)?;
            Some([
                (a.norm2()?, b.cross(c)?),
                (b.norm2()?, c.cross(a)?),
                (c.norm2()?, a.cross(b)?),
            ])
        }
        fn incircle_fast(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Option<I256> {
            sum_of_products(&incircle_terms(a, b, c, d)?)
        }
        #[allow(
            clippy::many_single_char_names,
            reason = "a to e name the points of the documented formula"
        )]
        fn insphere_terms(
            a: [i64; 3],
            b: [i64; 3],
            c: [i64; 3],
            d: [i64; 3],
            e: [i64; 3],
        ) -> Option<[(i128, i128); 4]> {
            let a = D3::sub(a, e)?;
            let b = D3::sub(b, e)?;
            let c = D3::sub(c, e)?;
            let d = D3::sub(d, e)?;
            Some([
                (a.norm2()?, det3(b, c, d)?),
                (b.norm2()?, det3(a, c, d)?.checked_neg()?),
                (c.norm2()?, det3(a, b, d)?),
                (d.norm2()?, det3(a, b, c)?.checked_neg()?),
            ])
        }
        #[allow(
            clippy::many_single_char_names,
            reason = "a to e name the points of the documented formula"
        )]
        fn insphere_fast(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3], e: [i64; 3]) -> Option<I256> {
            sum_of_products(&insphere_terms(a, b, c, d, e)?)
        }
        type Total = I384;
        mod bounded {
            use super::Total;
            const ANY_I64_DIFFERENCE_BITS: usize = 64;
            const fn magnitude_bits(limbs: usize) -> usize {
                let Some(bits) = limbs.checked_mul(64) else {
                    return 0;
                };
                let Some(magnitude) = bits.checked_sub(1) else {
                    return 0;
                };
                magnitude
            }
            pub(super) const TOTAL_BITS: usize = magnitude_bits(Total::ZERO.to_le_limbs().len());
            pub(super) const fn product_bits(left: usize, right: usize) -> Option<usize> {
                left.checked_add(right)
            }
            pub(super) const fn sum_bits(left: usize, right: usize) -> Option<usize> {
                let larger = if left < right { right } else { left };
                larger.checked_add(1)
            }
            pub(super) const fn declarable(needed: Option<usize>, declared: usize) -> bool {
                matches!(needed, Some(bits) if bits <= declared) && declared <= TOTAL_BITS
            }
            const _: () = assert!(TOTAL_BITS == 383, "I384 holds magnitudes below 2^383");
            const _: () = assert!(declarable(
                Some(ANY_I64_DIFFERENCE_BITS),
                ANY_I64_DIFFERENCE_BITS
            ));
            struct Rule<const LEFT: usize, const RIGHT: usize, const OUT: usize>;
            impl<const LEFT: usize, const RIGHT: usize, const OUT: usize> Rule<LEFT, RIGHT, OUT> {
                const PRODUCT: () = assert!(
                    declarable(product_bits(LEFT, RIGHT), OUT),
                    "a product's declared bound does not follow from its factors' bounds"
                );
                const SUM: () = assert!(
                    declarable(sum_bits(LEFT, RIGHT), OUT),
                    "a sum's declared bound does not follow from its terms' bounds"
                );
            }
            #[derive(Clone, Copy)]
            pub(super) struct Bounded<const BITS: usize>(Total);
            impl Bounded<ANY_I64_DIFFERENCE_BITS> {
                pub(super) fn difference(p: i64, q: i64) -> Option<Self> {
                    let (p, q) = (Total::from_i64(p), Total::from_i64(q));
                    p.checked_sub(&q).map(Bounded)
                }
            }
            impl<const BITS: usize> Bounded<BITS> {
                pub(super) fn times<const OTHER: usize, const OUT: usize>(
                    &self,
                    other: &Bounded<OTHER>,
                ) -> Option<Bounded<OUT>> {
                    let () = Rule::<BITS, OTHER, OUT>::PRODUCT;
                    self.0.checked_mul(&other.0).map(Bounded)
                }
                pub(super) fn plus<const OTHER: usize, const OUT: usize>(
                    &self,
                    other: &Bounded<OTHER>,
                ) -> Option<Bounded<OUT>> {
                    let () = Rule::<BITS, OTHER, OUT>::SUM;
                    self.0.checked_add(&other.0).map(Bounded)
                }
                pub(super) fn minus<const OTHER: usize, const OUT: usize>(
                    &self,
                    other: &Bounded<OTHER>,
                ) -> Option<Bounded<OUT>> {
                    let () = Rule::<BITS, OTHER, OUT>::SUM;
                    self.0.checked_sub(&other.0).map(Bounded)
                }
                pub(super) fn value(self) -> Total {
                    self.0
                }
            }
        }
        type Diff = Bounded<64>;
        type Det2 = Bounded<129>;
        type Lift2 = Bounded<129>;
        type Lift3 = Bounded<130>;
        type Det3 = Bounded<195>;
        fn total_sub2(p: [i64; 2], q: [i64; 2]) -> Option<[Diff; 2]> {
            let ([px, py], [qx, qy]) = (p, q);
            Some([Diff::difference(px, qx)?, Diff::difference(py, qy)?])
        }
        fn total_sub3(p: [i64; 3], q: [i64; 3]) -> Option<[Diff; 3]> {
            let ([px, py, pz], [qx, qy, qz]) = (p, q);
            Some([
                Diff::difference(px, qx)?,
                Diff::difference(py, qy)?,
                Diff::difference(pz, qz)?,
            ])
        }
        fn total_det2(a: &Diff, b: &Diff, c: &Diff, d: &Diff) -> Option<Det2> {
            let ad: Bounded<128> = a.times(d)?;
            let bc: Bounded<128> = b.times(c)?;
            ad.minus(&bc)
        }
        fn total_norm2_2d([x, y]: &[Diff; 2]) -> Option<Lift2> {
            let xx: Bounded<128> = x.times(x)?;
            let yy: Bounded<128> = y.times(y)?;
            xx.plus(&yy)
        }
        fn total_norm2_3d([x, y, z]: &[Diff; 3]) -> Option<Lift3> {
            let xx: Bounded<128> = x.times(x)?;
            let yy: Bounded<128> = y.times(y)?;
            let zz: Bounded<128> = z.times(z)?;
            let xy: Bounded<129> = xx.plus(&yy)?;
            xy.plus(&zz)
        }
        fn total_det3(p: &[Diff; 3], q: &[Diff; 3], r: &[Diff; 3]) -> Option<Det3> {
            let ([px, py, pz], [qx, qy, qz], [rx, ry, rz]) = (p, q, r);
            let tx: Bounded<193> = px.times(&total_det2(qy, qz, ry, rz)?)?;
            let ty: Bounded<193> = py.times(&total_det2(qz, qx, rz, rx)?)?;
            let tz: Bounded<193> = pz.times(&total_det2(qx, qy, rx, ry)?)?;
            let txy: Bounded<194> = tx.plus(&ty)?;
            txy.plus(&tz)
        }
        fn orient2d_total(a: [i64; 2], b: [i64; 2], c: [i64; 2]) -> Option<Det2> {
            let [ux, uy] = total_sub2(b, a)?;
            let [vx, vy] = total_sub2(c, a)?;
            total_det2(&ux, &uy, &vx, &vy)
        }
        fn orient3d_total(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3]) -> Option<Det3> {
            let ab = total_sub3(b, a)?;
            let ac = total_sub3(c, a)?;
            let ad = total_sub3(d, a)?;
            total_det3(&ab, &ac, &ad)
        }
        fn incircle_total(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Option<Bounded<260>> {
            let a = total_sub2(a, d)?;
            let b = total_sub2(b, d)?;
            let c = total_sub2(c, d)?;
            let ([ax, ay], [bx, by], [cx, cy]) = (&a, &b, &c);
            let ta: Bounded<258> = total_norm2_2d(&a)?.times(&total_det2(bx, by, cx, cy)?)?;
            let tb: Bounded<258> = total_norm2_2d(&b)?.times(&total_det2(cx, cy, ax, ay)?)?;
            let tc: Bounded<258> = total_norm2_2d(&c)?.times(&total_det2(ax, ay, bx, by)?)?;
            let tab: Bounded<259> = ta.plus(&tb)?;
            tab.plus(&tc)
        }
        #[allow(
            clippy::many_single_char_names,
            reason = "a to e name the points of the documented formula"
        )]
        fn insphere_total(
            a: [i64; 3],
            b: [i64; 3],
            c: [i64; 3],
            d: [i64; 3],
            e: [i64; 3],
        ) -> Option<Bounded<327>> {
            let a = total_sub3(a, e)?;
            let b = total_sub3(b, e)?;
            let c = total_sub3(c, e)?;
            let d = total_sub3(d, e)?;
            let ta: Bounded<325> = total_norm2_3d(&a)?.times(&total_det3(&b, &c, &d)?)?;
            let tb: Bounded<325> = total_norm2_3d(&b)?.times(&total_det3(&a, &c, &d)?)?;
            let tc: Bounded<325> = total_norm2_3d(&c)?.times(&total_det3(&a, &b, &d)?)?;
            let td: Bounded<325> = total_norm2_3d(&d)?.times(&total_det3(&a, &b, &c)?)?;
            let first: Bounded<326> = ta.minus(&tb)?;
            let second: Bounded<326> = tc.minus(&td)?;
            first.plus(&second)
        }
        fn fast_else_total<const BITS: usize>(
            fast: Option<Sign>,
            total: impl FnOnce() -> Option<Bounded<BITS>>,
        ) -> Result<Sign, NumError> {
            match fast {
                Some(sign) => Ok(sign),
                None => total()
                    .map(|exact| exact.value().signum())
                    .ok_or(NumError::BudgetExceeded),
            }
        }
        fn xy(p: P2) -> [i64; 2] {
            [p.x.get(), p.y.get()]
        }
        fn xyz(p: P3) -> [i64; 3] {
            [p.x.get(), p.y.get(), p.z.get()]
        }
        const NARROW_2D_BITS: u32 = 31;
        const I64_MAGNITUDE_BITS: u32 = 63;
        const _: () = {
            let products = exponent_sum(NARROW_2D_BITS, NARROW_2D_BITS);
            assert!(
                exponent_sum(products, 1) <= I64_MAGNITUDE_BITS,
                "orient2d's narrow step fits i64"
            );
        };
        fn narrow(p: i64, q: i64, bits: u32) -> Option<i64> {
            let d = p.checked_sub(q)?;
            (d.unsigned_abs() < u64::pow(2, bits)).then_some(d)
        }
        #[must_use]
        #[allow(
            clippy::many_single_char_names,
            reason = "a, b and c are the points, and p, q, r and s the differences of the formula"
        )]
        pub fn orient2d_narrow(a: P2, b: P2, c: P2) -> Option<Sign> {
            let [ax, ay] = xy(a);
            let [bx, by] = xy(b);
            let [cx, cy] = xy(c);
            let (p, q) = (
                narrow(bx, ax, NARROW_2D_BITS)?,
                narrow(cy, ay, NARROW_2D_BITS)?,
            );
            let (r, s) = (
                narrow(by, ay, NARROW_2D_BITS)?,
                narrow(cx, ax, NARROW_2D_BITS)?,
            );
            Some(Sign::of_i64(
                p.checked_mul(q)?.checked_sub(r.checked_mul(s)?)?,
            ))
        }
        pub fn orient2d(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
            if ORIENT2D_FILTER_FIRST {
                match generated::orient2d::filter(a, b, c) {
                    Some(sign) => Ok(sign),
                    None => orient2d_unfiltered(a, b, c),
                }
            } else {
                orient2d_exact(a, b, c)
            }
        }
        pub fn orient2d_step(a: P2, b: P2, c: P2) -> Result<(Sign, Step), NumError> {
            let sign = orient2d(a, b, c)?;
            let filtered = generated::orient2d::filter(a, b, c).is_some();
            let narrowed = orient2d_narrow(a, b, c).is_some();
            Ok((sign, Step::route(ORIENT2D_FILTER_FIRST, filtered, narrowed)))
        }
        const ORIENT2D_FILTER_FIRST: bool = !cfg!(all(
            target_pointer_width = "64",
            not(target_family = "wasm")
        ));
        #[inline(never)]
        fn orient2d_unfiltered(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
            match orient2d_narrow(a, b, c) {
                Some(sign) => Ok(sign),
                None => orient2d_exact(a, b, c),
            }
        }
        #[inline]
        fn orient2d_exact(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
            let exact = generated::orient2d::exact(a, b, c);
            let [a, b, c] = [a, b, c].map(xy);
            let fast = exact.or_else(|| orient2d_fast(a, b, c).map(Sign::of_i128));
            fast_else_total(fast, || orient2d_total(a, b, c))
        }
        pub fn orient3d(a: P3, b: P3, c: P3, d: P3) -> Result<Sign, NumError> {
            match generated::orient3d::filter(a, b, c, d) {
                Some(sign) => Ok(sign),
                None => orient3d_unfiltered(a, b, c, d),
            }
        }
        pub fn orient3d_step(a: P3, b: P3, c: P3, d: P3) -> Result<(Sign, Step), NumError> {
            let filtered = generated::orient3d::filter(a, b, c, d).is_some();
            Ok((orient3d(a, b, c, d)?, Step::route(true, filtered, false)))
        }
        #[inline(never)]
        fn orient3d_unfiltered(a: P3, b: P3, c: P3, d: P3) -> Result<Sign, NumError> {
            let exact = generated::orient3d::exact(a, b, c, d);
            let [a, b, c, d] = [a, b, c, d].map(xyz);
            let fast = exact.or_else(|| orient3d_fast(a, b, c, d).map(Sign::of_i128));
            fast_else_total(fast, || orient3d_total(a, b, c, d))
        }
        pub fn incircle(a: P2, b: P2, c: P2, d: P2) -> Result<Sign, NumError> {
            match generated::incircle::filter(a, b, c, d) {
                Some(sign) => Ok(sign),
                None => incircle_unfiltered(a, b, c, d),
            }
        }
        pub fn incircle_step(a: P2, b: P2, c: P2, d: P2) -> Result<(Sign, Step), NumError> {
            let filtered = generated::incircle::filter(a, b, c, d).is_some();
            Ok((incircle(a, b, c, d)?, Step::route(true, filtered, false)))
        }
        #[inline(never)]
        fn incircle_unfiltered(a: P2, b: P2, c: P2, d: P2) -> Result<Sign, NumError> {
            let exact = generated::incircle::exact(a, b, c, d);
            let [a, b, c, d] = [a, b, c, d].map(xy);
            let fast = exact.or_else(|| incircle_fast(a, b, c, d).map(|sum| sum.signum()));
            fast_else_total(fast, || incircle_total(a, b, c, d))
        }
        #[allow(
            clippy::many_single_char_names,
            reason = "a to e name the points of the documented formula"
        )]
        pub fn insphere(a: P3, b: P3, c: P3, d: P3, e: P3) -> Result<Sign, NumError> {
            match generated::insphere::filter(a, b, c, d, e) {
                Some(sign) => Ok(sign),
                None => insphere_unfiltered(a, b, c, d, e),
            }
        }
        #[inline(never)]
        #[allow(
            clippy::many_single_char_names,
            reason = "a to e name the points of the documented formula"
        )]
        fn insphere_unfiltered(a: P3, b: P3, c: P3, d: P3, e: P3) -> Result<Sign, NumError> {
            let exact = generated::insphere::exact(a, b, c, d, e);
            let [a, b, c, d, e] = [a, b, c, d, e].map(xyz);
            let fast = exact.or_else(|| insphere_fast(a, b, c, d, e).map(|sum| sum.signum()));
            fast_else_total(fast, || insphere_total(a, b, c, d, e))
        }
    }

    // The wrappers. "routed" is a predicate's own body on raw `i64` coordinates, so it can be
    // called off the grid; "forced" skips the fast path and takes the total path's sign.

    /// orient2d's fast path: its exact value, or `None` where a checked step overflowed.
    pub(crate) fn o2_fast([a, b, c]: [Pt2; 3]) -> Option<i128> {
        orient2d_fast(a, b, c)
    }

    /// orient3d's fast path.
    pub(crate) fn o3_fast([a, b, c, d]: [Pt3; 4]) -> Option<i128> {
        orient3d_fast(a, b, c, d)
    }

    /// incircle's fast path.
    pub(crate) fn ic_fast([a, b, c, d]: [Pt2; 4]) -> Option<I256> {
        incircle_fast(a, b, c, d)
    }

    /// insphere's fast path.
    pub(crate) fn is_fast([a, b, c, d, e]: [Pt3; 5]) -> Option<I256> {
        insphere_fast(a, b, c, d, e)
    }

    /// incircle's (lift, 2 × 2 determinant) pairs in `i128`.
    pub(crate) fn ic_terms([a, b, c, d]: [Pt2; 4]) -> Option<[(i128, i128); 3]> {
        incircle_terms(a, b, c, d)
    }

    /// insphere's (lift, signed 3 × 3 cofactor) pairs in `i128`.
    pub(crate) fn is_terms([a, b, c, d, e]: [Pt3; 5]) -> Option<[(i128, i128); 4]> {
        insphere_terms(a, b, c, d, e)
    }

    /// orient2d's total path.
    pub(crate) fn o2_total([a, b, c]: [Pt2; 3]) -> Option<I384> {
        orient2d_total(a, b, c).map(Bounded::value)
    }

    /// orient3d's total path.
    pub(crate) fn o3_total([a, b, c, d]: [Pt3; 4]) -> Option<I384> {
        orient3d_total(a, b, c, d).map(Bounded::value)
    }

    /// incircle's total path.
    pub(crate) fn ic_total([a, b, c, d]: [Pt2; 4]) -> Option<I384> {
        incircle_total(a, b, c, d).map(Bounded::value)
    }

    /// insphere's total path.
    pub(crate) fn is_total([a, b, c, d, e]: [Pt3; 5]) -> Option<I384> {
        insphere_total(a, b, c, d, e).map(Bounded::value)
    }

    /// orient2d's body on raw coordinates.
    pub(crate) fn o2_routed([a, b, c]: [Pt2; 3]) -> Sign {
        let fast = orient2d_fast(a, b, c).map(Sign::of_i128);
        fast_else_total(fast, || orient2d_total(a, b, c))
            .expect("both paths failed, which only a defect in geom-num can cause")
    }

    /// orient3d's body on raw coordinates.
    pub(crate) fn o3_routed([a, b, c, d]: [Pt3; 4]) -> Sign {
        let fast = orient3d_fast(a, b, c, d).map(Sign::of_i128);
        fast_else_total(fast, || orient3d_total(a, b, c, d))
            .expect("both paths failed, which only a defect in geom-num can cause")
    }

    /// incircle's body on raw coordinates.
    pub(crate) fn ic_routed([a, b, c, d]: [Pt2; 4]) -> Sign {
        let fast = incircle_fast(a, b, c, d).map(|sum| sum.signum());
        fast_else_total(fast, || incircle_total(a, b, c, d))
            .expect("both paths failed, which only a defect in geom-num can cause")
    }

    /// insphere's body on raw coordinates.
    pub(crate) fn is_routed([a, b, c, d, e]: [Pt3; 5]) -> Sign {
        let fast = insphere_fast(a, b, c, d, e).map(|sum| sum.signum());
        fast_else_total(fast, || insphere_total(a, b, c, d, e))
            .expect("both paths failed, which only a defect in geom-num can cause")
    }

    /// orient2d through the total path only.
    pub(crate) fn o2_forced([a, b, c]: [Pt2; 3]) -> Sign {
        fast_else_total(None, || orient2d_total(a, b, c))
            .expect("both paths failed, which only a defect in geom-num can cause")
    }

    /// orient3d through the total path only.
    pub(crate) fn o3_forced([a, b, c, d]: [Pt3; 4]) -> Sign {
        fast_else_total(None, || orient3d_total(a, b, c, d))
            .expect("both paths failed, which only a defect in geom-num can cause")
    }

    /// incircle through the total path only.
    pub(crate) fn ic_forced([a, b, c, d]: [Pt2; 4]) -> Sign {
        fast_else_total(None, || incircle_total(a, b, c, d))
            .expect("both paths failed, which only a defect in geom-num can cause")
    }

    /// insphere through the total path only.
    pub(crate) fn is_forced([a, b, c, d, e]: [Pt3; 5]) -> Sign {
        fast_else_total(None, || insphere_total(a, b, c, d, e))
            .expect("both paths failed, which only a defect in geom-num can cause")
    }
}

/// `code` without its `//` comments, its whitespace and its trailing commas, so that only its
/// tokens count and never their layout (rustfmt adds a trailing comma when it breaks a list).
fn squash(code: &str) -> String {
    let uncommented = code.lines().map(|line| line.split("//").next().unwrap());
    let chars = uncommented.flat_map(str::chars);
    let mut dense: String = chars.filter(|c| !c.is_whitespace()).collect();
    for close in [")", "]", "}"] {
        dense = dense.replace(&format!(",{close}"), close);
    }
    dense
}

/// The layout of a list, a comment or a trailing comma never changes what `squash` sees.
#[test]
fn squash_sees_tokens_not_layout() {
    let broken = "f(\n    a, // the first\n    [b, c,],\n)";
    assert_eq!(squash(broken), squash("f(a, [b, c])"));
    assert_ne!(squash("f(a, b)"), squash("f(a, c)"));
}

/// Up to 80 characters of `s` around character `at`.
fn excerpt(s: &str, at: usize) -> String {
    s.chars().skip(at.saturating_sub(40)).take(80).collect()
}

/// The copy above is predicate.rs's code: the same tokens outside comments and the test module.
/// If this fails, predicate.rs changed and every other test in this file is attacking stale code.
#[test]
fn the_copy_is_still_predicate_rs_code_token_for_token() {
    let source = include_str!("../src/predicate.rs");
    let end = source.find("#[cfg(test)]").unwrap();
    let module = squash(&source[..end]);
    let copied = squash(copy::CODE);
    let pairs = module.chars().zip(copied.chars());
    let same = pairs.take_while(|(m, c)| m == c).count();
    let (ours, theirs) = (excerpt(&module, same), excerpt(&copied, same));
    assert!(
        module == copied,
        "predicate.rs and the copy differ after {same} characters:\n{ours}\n{theirs}"
    );
    // The comparison read real code, not two empty strings.
    let length = module.len();
    assert!(length > 5_000, "only {length} characters of code");
}

// ---------------------------------------------------------------------------------------------
// Oracles and helpers.
// ---------------------------------------------------------------------------------------------

/// Orderings of 0..n, each with whether it is odd.
type Orderings = Vec<(Vec<usize>, bool)>;

/// Every ordering of 0..n with whether it is odd, generated once per n.
fn permutations(n: usize) -> &'static [(Vec<usize>, bool)] {
    static CACHE: [OnceLock<Orderings>; 6] = [const { OnceLock::new() }; 6];
    CACHE[n].get_or_init(|| {
        let mut orders: Vec<Vec<usize>> = vec![Vec::new()];
        for _ in 0..n {
            let mut longer = Vec::new();
            for order in &orders {
                for v in (0..n).filter(|v| !order.contains(v)) {
                    let mut next = order.clone();
                    next.push(v);
                    longer.push(next);
                }
            }
            orders = longer;
        }
        let odd = |order: &[usize]| {
            let pairs = (0..n).flat_map(|i| (i + 1..n).map(move |j| (i, j)));
            pairs.filter(|&(i, j)| order[i] > order[j]).count() % 2 == 1
        };
        let parity = |order: Vec<usize>| {
            let is_odd = odd(&order);
            (order, is_odd)
        };
        orders.into_iter().map(parity).collect()
    })
}

/// `p` reordered: entry k of the result is `p[order[k]]`.
fn reorder<T: Copy, const N: usize>(p: [T; N], order: &[usize]) -> [T; N] {
    std::array::from_fn(|k| p[order[k]])
}

/// The determinant by the Leibniz formula: the sum over every permutation σ of
/// sign(σ) · m[0][σ(0)] · … · m[n−1][σ(n−1)]. It shares no step with cofactor expansion,
/// Bareiss elimination or the module's differences.
fn leibniz(m: &[Vec<BigInt>]) -> BigInt {
    let mut det = BigInt::zero();
    for (order, odd) in permutations(m.len()) {
        let entries = order.iter().enumerate().map(|(row, &col)| &m[row][col]);
        let term = entries.fold(BigInt::one(), |t, e| t * e);
        if *odd {
            det -= term;
        } else {
            det += term;
        }
    }
    det
}

/// The homogeneous row of a point: its coordinates, |p|² if `lift`, then 1.
fn row<const D: usize>(p: [i64; D], lift: bool) -> Vec<BigInt> {
    let mut out: Vec<BigInt> = p.iter().map(|&v| BigInt::from(v)).collect();
    if lift {
        out.push(p.iter().map(|&v| BigInt::from(v) * v).sum());
    }
    out.push(BigInt::one());
    out
}

/// orient2d's exact value: det[a 1; b 1; c 1], twice the signed area.
fn o2_exact(p: [Pt2; 3]) -> BigInt {
    leibniz(&p.map(|q| row(q, false)))
}

/// orient3d's exact value in this module's convention: −det[a 1; b 1; c 1; d 1].
fn o3_exact(p: [Pt3; 4]) -> BigInt {
    -leibniz(&p.map(|q| row(q, false)))
}

/// incircle's exact value: det[a |a|² 1; b |b|² 1; c |c|² 1; d |d|² 1].
fn ic_exact(p: [Pt2; 4]) -> BigInt {
    leibniz(&p.map(|q| row(q, true)))
}

/// insphere's exact value in this module's convention: −det[a |a|² 1; …; e |e|² 1].
fn is_exact(p: [Pt3; 5]) -> BigInt {
    -leibniz(&p.map(|q| row(q, true)))
}

/// The sign of an exact value.
fn sign_of(v: &BigInt) -> Sign {
    match v.sign() {
        num_bigint::Sign::Minus => Sign::Negative,
        num_bigint::Sign::NoSign => Sign::Zero,
        num_bigint::Sign::Plus => Sign::Positive,
    }
}

/// The value a `Wide` holds, read from its two's-complement limbs.
fn wide_big<const L: usize>(w: &Wide<L>) -> BigInt {
    let limbs = w.to_le_limbs();
    let bytes: Vec<u8> = limbs.iter().flat_map(|l| l.to_le_bytes()).collect();
    BigInt::from_signed_bytes_le(&bytes)
}

/// 2^n.
fn pow2(n: u32) -> BigInt {
    BigInt::one() << n
}

/// v reduced into the two's-complement range of `bits` bits: what a wrapping evaluation would
/// hold instead of v.
fn wrapped(v: &BigInt, bits: u32) -> BigInt {
    let modulus = pow2(bits);
    let r = ((v % &modulus) + &modulus) % &modulus;
    if r >= pow2(bits - 1) { r - modulus } else { r }
}

/// The value of a total path, which must exist for every `i64` input.
fn total_value<T: Debug>(what: &str, value: Option<I384>, p: T) -> BigInt {
    let Some(value) = value else {
        panic!("{what}'s total path returned None at {p:?}");
    };
    wide_big(&value)
}

fn o2_total(p: [Pt2; 3]) -> BigInt {
    total_value("orient2d", copy::o2_total(p), p)
}

fn o3_total(p: [Pt3; 4]) -> BigInt {
    total_value("orient3d", copy::o3_total(p), p)
}

fn ic_total(p: [Pt2; 4]) -> BigInt {
    total_value("incircle", copy::ic_total(p), p)
}

fn is_total(p: [Pt3; 5]) -> BigInt {
    total_value("insphere", copy::is_total(p), p)
}

fn g2([x, y]: Pt2) -> P2 {
    P2::new(x, y).unwrap()
}

fn g3([x, y, z]: Pt3) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// The crate's public predicates on plain arrays of grid coordinates. Each returns a `Result`
/// whose error only a defect could cause, so these unwrap it with a message.
fn o2(p: [Pt2; 3]) -> Sign {
    let [a, b, c] = p.map(g2);
    predicate::orient2d(a, b, c).expect("grid points always give a sign")
}

fn o3(p: [Pt3; 4]) -> Sign {
    let [a, b, c, d] = p.map(g3);
    predicate::orient3d(a, b, c, d).expect("grid points always give a sign")
}

fn ic(p: [Pt2; 4]) -> Sign {
    let [a, b, c, d] = p.map(g2);
    predicate::incircle(a, b, c, d).expect("grid points always give a sign")
}

fn ins(p: [Pt3; 5]) -> Sign {
    let [a, b, c, d, e] = p.map(g3);
    predicate::insphere(a, b, c, d, e).expect("grid points always give a sign")
}

/// A small deterministic generator (`SplitMix64`, Steele, Lea and Flood, OOPSLA 2014), so every
/// run draws the same points.
struct Rng(u64);

impl Rng {
    fn draw(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in 0..n.
    fn below(&mut self, n: usize) -> usize {
        let n = u64::try_from(n).unwrap();
        usize::try_from(self.draw() % n).unwrap()
    }

    /// A value in lo..=hi.
    fn within(&mut self, lo: i64, hi: i64) -> i64 {
        let span = u128::try_from(i128::from(hi) - i128::from(lo)).unwrap() + 1;
        let offset = i128::try_from(u128::from(self.draw()) % span).unwrap();
        i64::try_from(i128::from(lo) + offset).unwrap()
    }
}

/// Any `i64`, with the extremes, their neighbours, small values and every magnitude from 2^0 to
/// 2^63 drawn often, so the paths' intermediates sweep across every overflow boundary.
fn wild() -> BoxedStrategy<i64> {
    prop_oneof![
        Just(LO),
        Just(HI),
        Just(LO + 1),
        Just(HI - 1),
        Just(-1i64),
        Just(0i64),
        Just(1i64),
        (0u32..64, any::<i64>()).prop_map(|(k, v)| v >> k),
        any::<i64>(),
    ]
    .boxed()
}

/// Any grid coordinate, with the edges, their neighbours and zero drawn often.
fn on_grid() -> impl Strategy<Value = i64> {
    prop_oneof![
        Just(-M),
        Just(M),
        Just(1 - M),
        Just(M - 1),
        Just(0i64),
        -M..=M,
    ]
}

// ---------------------------------------------------------------------------------------------
// The oracle and the copy are what they claim to be; the total path is out of public reach.
// ---------------------------------------------------------------------------------------------

/// The oracle's signs follow the documented geometry, and so do the crate and the forced total
/// path: counter-clockwise turns, the side the right-hand normal points to, a point inside a
/// counter-clockwise circle, a point inside a positively oriented sphere. The values are the
/// documented identities: incircle is orient2d's determinant (16) times r² − |d − o|² (8 − 2), and
/// insphere is orient3d's determinant (64) times r² − |e − o|² (12 − 3).
#[test]
fn the_oracle_and_both_paths_follow_the_documented_geometry() {
    let turn: [Pt2; 3] = [[0, 0], [1, 0], [0, 1]];
    let tetra: [Pt3; 4] = [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]];
    let circle: [Pt2; 4] = [[0, 0], [4, 0], [0, 4], [1, 1]];
    let sphere: [Pt3; 5] = [[0, 0, 0], [4, 0, 0], [0, 4, 0], [0, 0, 4], [1, 1, 1]];
    let one = BigInt::one();
    assert_eq!(o2_exact(turn), one, "a counter-clockwise turn");
    assert_eq!(o3_exact(tetra), one, "d on the normal's side");
    let (in_circle, in_sphere) = (BigInt::from(16 * 6), BigInt::from(64 * 9));
    assert_eq!(ic_exact(circle), in_circle, "d inside the circle");
    assert_eq!(ic_total(circle), in_circle, "incircle's total path");
    assert_eq!(is_exact(sphere), in_sphere, "e inside the sphere");
    assert_eq!(is_total(sphere), in_sphere, "insphere's total path");
    for (got, what) in [
        (o2(turn), "public orient2d"),
        (copy::o2_forced(turn), "forced orient2d"),
        (o3(tetra), "public orient3d"),
        (copy::o3_forced(tetra), "forced orient3d"),
        (ic(circle), "public incircle"),
        (copy::ic_forced(circle), "forced incircle"),
        (ins(sphere), "public insphere"),
        (copy::is_forced(sphere), "forced insphere"),
    ] {
        assert_eq!(got, Sign::Positive, "{what}");
    }
}

proptest! {
    #![proptest_config(cfg(256))]

    /// The copy gives the crate's answers, so attacking the copy attacks the crate.
    #[test]
    fn the_copy_answers_as_the_crate_does(
        v in uniform::<_, 5>(uniform::<_, 3>(on_grid())),
    ) {
        let p2: [Pt2; 5] = v.map(|[x, y, _]| [x, y]);
        let [a, b, c, d, e] = v.map(g3);
        let [f, g, h, k, _] = p2.map(g2);
        prop_assert_eq!(copy::orient2d(f, g, h), predicate::orient2d(f, g, h));
        prop_assert_eq!(copy::orient3d(a, b, c, d), predicate::orient3d(a, b, c, d));
        prop_assert_eq!(copy::incircle(f, g, h, k), predicate::incircle(f, g, h, k));
        prop_assert_eq!(copy::insphere(a, b, c, d, e), predicate::insphere(a, b, c, d, e));
    }

    /// No public route makes a coordinate off the grid, so no public call can reach the total
    /// path: `Coord::new` and `Coord::from_i128` accept exactly the magnitudes up to M, and
    /// refining or coarsening a grid coordinate between any two steps either fails or stays
    /// within M. (`Coord` keeps its value private, so these are the only ways to make one.)
    #[test]
    fn no_public_route_makes_a_coordinate_off_the_grid(
        v in prop_oneof![any::<i64>(), Just(M + 1), Just(-M - 1), Just(LO)],
        w in prop_oneof![any::<i128>(), Just(i128::from(M) + 1), Just(i128::MIN)],
        c in on_grid(),
    ) {
        let within = |x: Coord| x.get().unsigned_abs() <= M.unsigned_abs();
        let fits = v.unsigned_abs() <= M.unsigned_abs();
        prop_assert_eq!(Coord::new(v).is_ok(), fits, "Coord::new({})", v);
        prop_assert!(Coord::new(v).ok().is_none_or(within), "Coord::new({v})");
        let fits = w.unsigned_abs() <= M.unsigned_abs().into();
        prop_assert_eq!(Coord::from_i128(w).is_ok(), fits, "Coord::from_i128({})", w);
        prop_assert!(Coord::from_i128(w).ok().is_none_or(within), "Coord::from_i128({w})");
        let coord = Coord::new(c).unwrap();
        let steps = [GridStep::Q0, GridStep::Q1, GridStep::Q2];
        for from in steps {
            for to in steps {
                let finer = refine(coord, from, to).ok();
                prop_assert!(finer.is_none_or(within), "refine({c}, {from:?}, {to:?})");
                let coarser = coarsen(coord, from, to).ok().map(|r| r.value());
                prop_assert!(coarser.is_none_or(within), "coarsen({c}, {from:?}, {to:?})");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The total path: exact for every `i64` input, every intermediate within its declared bound.
// ---------------------------------------------------------------------------------------------

/// The largest magnitude seen so far of each intermediate of the total path, with the bound (a
/// power of two) that its type declares in predicate.rs.
#[derive(Default)]
struct Peaks(Vec<(&'static str, u32, BigInt)>);

impl Peaks {
    /// Records |v| for `name`, which predicate.rs bounds by 2^bits, and fails if |v| reaches it.
    fn note(&mut self, name: &'static str, bits: u32, v: &BigInt, at: &str) {
        let size = v.abs();
        assert!(
            size < pow2(bits),
            "{at}: the total path's {name} is {v}, not below 2^{bits}"
        );
        if let Some(entry) = self.0.iter_mut().find(|entry| entry.0 == name) {
            if size > entry.2 {
                entry.2 = size;
            }
        } else {
            self.0.push((name, bits, size));
        }
    }

    /// The largest |v| seen for `name`.
    fn largest(&self, name: &str) -> BigInt {
        let entry = self.0.iter().find(|entry| entry.0 == name);
        entry.map(|entry| entry.2.clone()).unwrap_or_default()
    }

    /// One line per intermediate: its declared bound and the bits of the largest value seen.
    fn report(&self) -> String {
        let line = |(name, bits, size): &(&str, u32, BigInt)| {
            let seen = size.bits();
            format!("  {name}: below 2^{bits}; largest seen has {seen} bits\n")
        };
        self.0.iter().map(line).collect()
    }
}

/// predicate.rs's total path transcribed onto `BigInt` step for step, in its order, noting every
/// intermediate in `peaks` against the bound its type declares in the source (each a
/// `Bounded<BITS>`, |v| < 2^BITS): differences below 2^64; the products in a 2 × 2 determinant,
/// and squares, below 2^128; 2 × 2 determinants below 2^129; two squares together below 2^129,
/// and in 3D all three below 2^130; each term of a 3 × 3 determinant below 2^193, the first two
/// together below 2^194 and the determinant below 2^195; in-circle terms below 2^258, the first
/// two together below 2^259 and the sum below 2^260; in-sphere terms below 2^325, the halves
/// t₀ − t₁ and t₂ − t₃ below 2^326 and the sum below 2^327.
struct Audit<'a> {
    peaks: &'a mut Peaks,
    at: &'a str,
}

impl Audit<'_> {
    fn note(&mut self, name: &'static str, bits: u32, v: &BigInt) {
        self.peaks.note(name, bits, v, self.at);
    }

    fn diff(&mut self, p: i64, q: i64) -> BigInt {
        let v = BigInt::from(p) - q;
        self.note("difference", 64, &v);
        v
    }

    fn sub<const D: usize>(&mut self, p: [i64; D], q: [i64; D]) -> [BigInt; D] {
        std::array::from_fn(|k| self.diff(p[k], q[k]))
    }

    fn det2(&mut self, a: &BigInt, b: &BigInt, c: &BigInt, d: &BigInt) -> BigInt {
        let (ad, bc) = (a * d, b * c);
        self.note("2 × 2 product", 128, &ad);
        self.note("2 × 2 product", 128, &bc);
        let v = ad - bc;
        self.note("2 × 2 det", 129, &v);
        v
    }

    /// x² + y², or x² + y² + z², in the source's order: (x² + y²) + z².
    fn norm2(&mut self, p: &[BigInt]) -> BigInt {
        let squares: Vec<BigInt> = p.iter().map(|x| x * x).collect();
        for square in &squares {
            self.note("square", 128, square);
        }
        let two = &squares[0] + &squares[1];
        self.note("two squares", 129, &two);
        let Some(third) = squares.get(2) else {
            return two;
        };
        let three = two + third;
        self.note("three squares", 130, &three);
        three
    }

    fn det3(&mut self, u: &[BigInt; 3], v: &[BigInt; 3], w: &[BigInt; 3]) -> BigInt {
        let ([ux, uy, uz], [vx, vy, vz], [wx, wy, wz]) = (u, v, w);
        let terms = [
            ux * self.det2(vy, vz, wy, wz),
            uy * self.det2(vz, vx, wz, wx),
            uz * self.det2(vx, vy, wx, wy),
        ];
        for term in &terms {
            self.note("3 × 3 term", 193, term);
        }
        let [tx, ty, tz] = terms;
        let two = tx + ty;
        self.note("3 × 3 det, two terms", 194, &two);
        let det = two + tz;
        self.note("3 × 3 det", 195, &det);
        det
    }

    fn orient2d(&mut self, [a, b, c]: [Pt2; 3]) -> BigInt {
        let [ux, uy] = self.sub(b, a);
        let [vx, vy] = self.sub(c, a);
        self.det2(&ux, &uy, &vx, &vy)
    }

    fn orient3d(&mut self, [a, b, c, d]: [Pt3; 4]) -> BigInt {
        let (u, v, w) = (self.sub(b, a), self.sub(c, a), self.sub(d, a));
        self.det3(&u, &v, &w)
    }

    fn incircle(&mut self, [a, b, c, d]: [Pt2; 4]) -> BigInt {
        let [a, b, c] = [a, b, c].map(|p| self.sub(p, d));
        let ([ax, ay], [bx, by], [cx, cy]) = (&a, &b, &c);
        let dets = [
            self.det2(bx, by, cx, cy),
            self.det2(cx, cy, ax, ay),
            self.det2(ax, ay, bx, by),
        ];
        let lifts = [&a, &b, &c].map(|p| self.norm2(p));
        let [ta, tb, tc] = {
            let mut products = lifts.into_iter().zip(dets).map(|(l, d)| l * d);
            std::array::from_fn(|_| products.next().unwrap())
        };
        for term in [&ta, &tb, &tc] {
            self.note("in-circle term", 258, term);
        }
        let two = ta + tb;
        self.note("in-circle sum, two terms", 259, &two);
        let sum = two + tc;
        self.note("in-circle sum", 260, &sum);
        sum
    }

    /// The source forms the sum as (t₀ − t₁) + (t₂ − t₃).
    fn insphere(&mut self, [a, b, c, d, e]: [Pt3; 5]) -> BigInt {
        let [a, b, c, d] = [a, b, c, d].map(|p| self.sub(p, e));
        let dets = [
            self.det3(&b, &c, &d),
            self.det3(&a, &c, &d),
            self.det3(&a, &b, &d),
            self.det3(&a, &b, &c),
        ];
        let lifts = [&a, &b, &c, &d].map(|p| self.norm2(p));
        let [ta, tb, tc, td] = {
            let mut products = lifts.into_iter().zip(dets).map(|(l, d)| l * d);
            std::array::from_fn(|_| products.next().unwrap())
        };
        for term in [&ta, &tb, &tc, &td] {
            self.note("in-sphere term", 325, term);
        }
        let first = ta - tb;
        self.note("in-sphere half", 326, &first);
        let second = tc - td;
        self.note("in-sphere half", 326, &second);
        let sum = first + second;
        self.note("in-sphere sum", 327, &sum);
        sum
    }
}

/// The verdict at one configuration: the audit's transcription agrees with the oracle, the total
/// path returns the oracle's value, the predicate's own body returns its sign, and the fast path,
/// wherever it answers, answers exactly.
fn judge(
    at: &str,
    exact: &BigInt,
    audited: &BigInt,
    total: Option<I384>,
    routed: Sign,
    fast: Option<BigInt>,
) {
    assert_eq!(audited, exact, "{at}: the audit disagrees");
    let Some(total) = total else {
        panic!("{at}: the total path returned None");
    };
    let total = wide_big(&total);
    assert_eq!(&total, exact, "{at}: the total path gave {total}");
    assert_eq!(routed, sign_of(exact), "{at}: exact value {exact}");
    if let Some(fast) = fast {
        assert_eq!(&fast, exact, "{at}: the fast path");
    }
}

fn exact_o2(peaks: &mut Peaks, p: [Pt2; 3]) {
    let at = format!("orient2d {p:?}");
    let audited = Audit { peaks, at: &at }.orient2d(p);
    let fast = copy::o2_fast(p).map(BigInt::from);
    let (total, routed) = (copy::o2_total(p), copy::o2_routed(p));
    judge(&at, &o2_exact(p), &audited, total, routed, fast);
}

fn exact_o3(peaks: &mut Peaks, p: [Pt3; 4]) {
    let at = format!("orient3d {p:?}");
    let audited = Audit { peaks, at: &at }.orient3d(p);
    let fast = copy::o3_fast(p).map(BigInt::from);
    let (total, routed) = (copy::o3_total(p), copy::o3_routed(p));
    judge(&at, &o3_exact(p), &audited, total, routed, fast);
}

fn exact_ic(peaks: &mut Peaks, p: [Pt2; 4]) {
    let at = format!("incircle {p:?}");
    let audited = Audit { peaks, at: &at }.incircle(p);
    let fast = copy::ic_fast(p).map(|v| wide_big(&v));
    let (total, routed) = (copy::ic_total(p), copy::ic_routed(p));
    judge(&at, &ic_exact(p), &audited, total, routed, fast);
}

fn exact_is(peaks: &mut Peaks, p: [Pt3; 5]) {
    let at = format!("insphere {p:?}");
    let audited = Audit { peaks, at: &at }.insphere(p);
    let fast = copy::is_fast(p).map(|v| wide_big(&v));
    let (total, routed) = (copy::is_total(p), copy::is_routed(p));
    judge(&at, &is_exact(p), &audited, total, routed, fast);
}

/// Coordinates at the limits of `i64`, one step inside them, and zero.
const EDGES: [i64; 5] = [LO, LO + 1, 0, HI - 1, HI];

/// Every point in D dimensions with all coordinates in `levels`.
fn lattice<const D: usize>(levels: &[i64]) -> Vec<[i64; D]> {
    let n = levels.len();
    let count = n.pow(u32::try_from(D).unwrap());
    let point = |code: usize| {
        let digit = |k: usize| code / n.pow(u32::try_from(k).unwrap()) % n;
        std::array::from_fn(|k| levels[digit(k)])
    };
    (0..count).map(point).collect()
}

/// orient2d on every ordered triple of points with coordinates in `EDGES`, and incircle with every
/// d and every unordered a, b, c of them: the total path is exact, every intermediate is within its
/// declared bound, and the largest difference, 2^64 − 1, is reached.
#[test]
fn total_path_is_exact_at_every_extreme_configuration_in_2d() {
    let mut peaks = Peaks::default();
    let square = lattice::<2>(&EDGES);
    for &a in &square {
        for &b in &square {
            for &c in &square {
                exact_o2(&mut peaks, [a, b, c]);
            }
        }
    }
    for &d in &square {
        for (i, &a) in square.iter().enumerate() {
            for (j, &b) in square.iter().enumerate().skip(i) {
                for &c in square.iter().skip(j) {
                    exact_ic(&mut peaks, [a, b, c, d]);
                }
            }
        }
    }
    let report = peaks.report();
    eprintln!("total path at the extremes in 2D:\n{report}");
    assert_eq!(peaks.largest("difference"), BigInt::from(u64::MAX));
}

/// orient3d on every ordered quadruple of the corners of the `i64` cube, insphere with every e and
/// every unordered a, b, c, d of them, and both on random points with coordinates in `EDGES`.
#[test]
fn total_path_is_exact_at_extreme_configurations_in_3d() {
    let mut peaks = Peaks::default();
    let corners = lattice::<3>(&[LO, HI]);
    for &a in &corners {
        for &b in &corners {
            for &c in &corners {
                for &d in &corners {
                    exact_o3(&mut peaks, [a, b, c, d]);
                }
            }
        }
    }
    for &e in &corners {
        for (i, &a) in corners.iter().enumerate() {
            for (j, &b) in corners.iter().enumerate().skip(i) {
                for (k, &c) in corners.iter().enumerate().skip(j) {
                    for &d in corners.iter().skip(k) {
                        exact_is(&mut peaks, [a, b, c, d, e]);
                    }
                }
            }
        }
    }
    let cube = lattice::<3>(&EDGES);
    let mut rng = Rng(3);
    for _ in 0..3_000 {
        let p: [Pt3; 5] = std::array::from_fn(|_| cube[rng.below(cube.len())]);
        exact_o3(&mut peaks, [p[0], p[1], p[2], p[3]]);
        exact_is(&mut peaks, p);
    }
    let report = peaks.report();
    eprintln!("total path at the extremes in 3D:\n{report}");
    assert_eq!(peaks.largest("difference"), BigInt::from(u64::MAX));
}

proptest! {
    #![proptest_config(cfg(512))]

    /// Random coordinates of every magnitude: the total path is exact and within its bounds, the
    /// predicate's body has the exact sign, and the fast path is exact wherever it answers.
    #[test]
    fn total_path_is_exact_for_any_i64_coordinates(
        v in uniform::<_, 5>(uniform::<_, 3>(wild())),
    ) {
        let mut peaks = Peaks::default();
        let flat: [Pt2; 5] = v.map(|[x, y, _]| [x, y]);
        exact_o2(&mut peaks, [flat[0], flat[1], flat[2]]);
        exact_ic(&mut peaks, [flat[0], flat[1], flat[2], flat[3]]);
        exact_o3(&mut peaks, [v[0], v[1], v[2], v[3]]);
        exact_is(&mut peaks, v);
    }
}

/// The bounds predicate.rs declares or writes, recomputed from the largest difference of two
/// coordinates: 2^64 − 1 for any `i64` coordinates (the total path, whose bounds are its types)
/// and δ = 2(2^40 − 1) on the grid (the fast path, whose bounds are written). Each bound, built up
/// as the source builds it, is below the power of two the source claims, and each claimed power
/// fits the type the value lives in. The compiler checks the total path's bounds by its own rules;
/// this checks them against the largest values instead.
#[test]
fn every_written_bound_follows_from_the_largest_difference() {
    let below = |v: &BigInt, bits: u32, what: &str| {
        assert!(v < &pow2(bits), "{what}: {v} is not below 2^{bits}");
    };
    // The total path, in I384 (values up to 2^383 − 1).
    let t = BigInt::from(u64::MAX);
    let tt = &t * &t;
    below(&t, 64, "total: difference");
    below(&tt, 128, "total: product in a 2 × 2 det");
    let det2 = BigInt::from(2) * &tt;
    below(&det2, 129, "total: 2 × 2 det");
    let lift2 = BigInt::from(2) * &tt;
    below(&lift2, 129, "total: lift in 2D");
    let lift3 = BigInt::from(3) * &tt;
    below(&lift3, 130, "total: lift in 3D");
    let term = &t * &det2;
    below(&term, 193, "total: term of a 3 × 3 det");
    below(&(BigInt::from(2) * &term), 194, "total: two 3 × 3 terms");
    let det3 = BigInt::from(3) * &term;
    below(&det3, 195, "total: 3 × 3 det");
    let circle_term = &lift2 * &det2;
    below(&circle_term, 258, "total: in-circle term");
    below(
        &(BigInt::from(2) * &circle_term),
        259,
        "total: two in-circle",
    );
    below(
        &(BigInt::from(3) * &circle_term),
        260,
        "total: in-circle sum",
    );
    let sphere_term = &lift3 * &det3;
    below(&sphere_term, 325, "total: in-sphere term");
    below(
        &(BigInt::from(2) * &sphere_term),
        326,
        "total: in-sphere half",
    );
    below(
        &(BigInt::from(4) * &sphere_term),
        327,
        "total: in-sphere sum",
    );
    below(&(pow2(327) << 56u32), 384, "total: headroom");
    // The fast path: i128 (up to 2^127 − 1), then I256 (up to 2^255 − 1).
    let delta = BigInt::from(2 * M);
    let dd = &delta * &delta;
    below(&delta, 41, "fast: δ");
    below(&dd, 82, "fast: δ²");
    below(&(BigInt::from(2) * &dd), 83, "fast: 2δ²");
    below(&(BigInt::from(3) * &dd), 84, "fast: 3δ²");
    let ddd = &dd * &delta;
    below(&(BigInt::from(2) * &ddd), 124, "fast: 2δ³");
    below(&(BigInt::from(6) * &ddd), 126, "fast: 6δ³");
    let dddd = &ddd * &delta;
    below(&(BigInt::from(4) * &dddd), 166, "fast: 4δ⁴");
    below(&(BigInt::from(12) * &dddd), 168, "fast: 12δ⁴");
    let ddddd = &dddd * &delta;
    below(&(BigInt::from(18) * &ddddd), 210, "fast: 18δ⁵");
    below(&(BigInt::from(72) * &ddddd), 212, "fast: 72δ⁵");
    // The compile-time assertion's reason: at 41 bits, orient3d's bound 6δ³ exceeds 2^127.
    let delta41 = BigInt::from(2 * ((1i64 << 41) - 1));
    let bound41 = BigInt::from(6) * &delta41 * &delta41 * &delta41;
    assert!(bound41 >= pow2(127), "6δ³ = {bound41} < 2^127");
}

// ---------------------------------------------------------------------------------------------
// The two paths against each other.
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(cfg(512))]

    /// On the grid the fast path always answers, with exactly the total path's value, and the
    /// crate's sign is the forced total path's sign.
    #[test]
    fn fast_and_total_paths_agree_exactly_on_the_grid(
        v in uniform::<_, 5>(uniform::<_, 3>(on_grid())),
    ) {
        let flat: [Pt2; 5] = v.map(|[x, y, _]| [x, y]);
        let p2 = [flat[0], flat[1], flat[2]];
        let fast = copy::o2_fast(p2).map(BigInt::from);
        prop_assert_eq!(fast, Some(o2_total(p2)), "orient2d {:?}", p2);
        prop_assert_eq!(o2(p2), copy::o2_forced(p2), "orient2d {:?}", p2);
        let q2 = [flat[0], flat[1], flat[2], flat[3]];
        let fast = copy::ic_fast(q2).map(|w| wide_big(&w));
        prop_assert_eq!(fast, Some(ic_total(q2)), "incircle {:?}", q2);
        prop_assert_eq!(ic(q2), copy::ic_forced(q2), "incircle {:?}", q2);
        let p3 = [v[0], v[1], v[2], v[3]];
        let fast = copy::o3_fast(p3).map(BigInt::from);
        prop_assert_eq!(fast, Some(o3_total(p3)), "orient3d {:?}", p3);
        prop_assert_eq!(o3(p3), copy::o3_forced(p3), "orient3d {:?}", p3);
        let fast = copy::is_fast(v).map(|w| wide_big(&w));
        prop_assert_eq!(fast, Some(is_total(v)), "insphere {:?}", v);
        prop_assert_eq!(ins(v), copy::is_forced(v), "insphere {:?}", v);
    }
}

/// p with the axes whose bit is set in `mask` reflected about −1/2 (v ↦ −1 − v, bitwise not),
/// which maps every `i64` to an `i64`.
fn flip<const D: usize>(p: [i64; D], mask: usize) -> [i64; D] {
    std::array::from_fn(|k| if (mask >> k) & 1 == 1 { !p[k] } else { p[k] })
}

/// Every ordering and every reflection about −1/2 of `base`.
fn variants<const D: usize, const N: usize>(base: [[i64; D]; N]) -> Vec<[[i64; D]; N]> {
    let mut out = Vec::new();
    for (order, _) in permutations(N) {
        for mask in 0..1 << D {
            out.push(reorder(base, order).map(|p| flip(p, mask)));
        }
    }
    out
}

/// Off the grid, at a configuration whose exact value does not fit the fast path's type (`bits`
/// wide): the fast path must give up, the predicate's body must take the total path to the exact
/// sign, and a wrapping evaluation would have had the opposite sign, so a silent wrap anywhere
/// would show here.
fn overflow_must_be_caught(
    at: &str,
    exact: &BigInt,
    fast: Option<&BigInt>,
    routed: Sign,
    bits: u32,
) {
    let size = exact.abs();
    assert!(size >= pow2(bits - 1), "{at}: it fits {bits} bits");
    assert_eq!(fast, None, "{at}: the fast path answered");
    assert_eq!(routed, sign_of(exact), "{at}: the predicate's body");
    let wrap = sign_of(&wrapped(exact, bits));
    assert_eq!(wrap, sign_of(exact).negate(), "{at}: wrap keeps sign");
}

/// orient2d where both products, 2^63 · (2^64 − 1) and (2^64 − 1) · (1 − 2^63), fit `i128` and
/// only their difference, 2^128 − 2^65 + 1, does not.
const O2_OVER: [Pt2; 3] = [[-1, LO], [HI, HI], [LO, HI]];

/// orient3d with a, b, c as in `O2_OVER` in the plane z = 0 and d one step above a, so the three
/// terms of the triple product are 2^63 · (2^64 − 1), (2^64 − 1) · (2^63 − 1) and 0: the first
/// two fit `i128` and their sum does not.
const O3_OVER: [Pt3; 4] = [[-1, LO, 0], [HI, HI, 0], [LO, HI, 0], [-1, LO, 1]];

/// incircle with d = (−1, −1) and c = (`i64::MAX`, `i64::MAX`), so c − d = (2^63, 2^63) and
/// |c − d|² = 2^127, one more than `i128` holds, after the first two lifts and determinants fit.
const IC_OVER: [Pt2; 4] = [[LO, HI], [LO, LO + 1], [HI, HI], [-1, -1]];

/// insphere with four corners of the square (±(2^63 − 1), ±(2^63 − 1)) lifted by small z, a flat
/// tetrahedron around e at the origin: every factor fits `i128`, and the sum is about −2^256.
const IS_OVER: [Pt3; 5] = [
    [HI, HI, 1],
    [-HI, HI, 0],
    [-HI, -HI, 0],
    [HI, -HI, -1],
    [0, 0, 0],
];

/// orient2d at `O2_OVER`: its exact value is 2^128 − 2^65 + 1, which wrapped to 128 bits would be
/// negative. Every ordering and reflection.
#[test]
fn orient2d_where_only_the_last_subtraction_overflows_i128() {
    let products = [
        BigInt::from(1u64 << 63) * u64::MAX,
        BigInt::from(u64::MAX) * (1 - (BigInt::one() << 63u32)),
    ];
    for product in &products {
        assert!(product.abs() < pow2(127), "{product} must fit i128");
    }
    assert_eq!(o2_exact(O2_OVER), pow2(128) - pow2(65) + 1);
    for p in variants(O2_OVER) {
        let exact = o2_exact(p);
        let fast = copy::o2_fast(p).map(BigInt::from);
        let at = format!("orient2d {p:?}");
        overflow_must_be_caught(&at, &exact, fast.as_ref(), copy::o2_routed(p), 128);
        assert_eq!(o2_total(p), exact, "{at}: the total path");
    }
}

/// orient3d at `O3_OVER`: the same value as orient2d at `O2_OVER`. Every ordering and reflection.
#[test]
fn orient3d_where_only_the_last_addition_overflows_i128() {
    assert_eq!(o3_exact(O3_OVER), pow2(128) - pow2(65) + 1);
    for p in variants(O3_OVER) {
        let exact = o3_exact(p);
        let fast = copy::o3_fast(p).map(BigInt::from);
        let at = format!("orient3d {p:?}");
        overflow_must_be_caught(&at, &exact, fast.as_ref(), copy::o3_routed(p), 128);
        assert_eq!(o3_total(p), exact, "{at}: the total path");
    }
}

/// incircle at the edge of the fast path's types, off the grid. At `IC_OVER` a lift is 2^127 and
/// does not fit `i128`, so the fast path must give up. With d at the origin and a, b, c at three
/// corners of the square (±(2^63 − 1), ±(2^63 − 1)), every factor fits and the sum is
/// 8(2^63 − 1)⁴, within 2^195 of `I256`'s limit 2^255, so the fast path must answer, exactly.
/// Every ordering and reflection of both, and of the extreme configuration that brings a partial
/// sum nearest to −2^255.
#[test]
fn incircle_at_the_edges_of_i128_and_i256() {
    assert_eq!(copy::ic_terms(IC_OVER), None, "a lift of 2^127 fits");
    let h = HI;
    let near: [Pt2; 4] = [[h, h], [-h, h], [-h, -h], [0, 0]];
    let eight_h4 = BigInt::from(8) * BigInt::from(h).pow(4u32);
    assert_eq!(ic_exact(near), eight_h4);
    let gap = pow2(255) - &eight_h4;
    assert!(gap.is_positive(), "8h⁴ = {eight_h4} does not fit I256");
    assert!(gap < pow2(195), "8h⁴ = {eight_h4} is not near 2^255");
    let deep: [Pt2; 4] = [[LO, HI], [HI, LO], [LO, LO + 1], [0, 0]];
    let mut answered = 0;
    for base in [IC_OVER, near, deep] {
        for p in variants(base) {
            let exact = ic_exact(p);
            let at = format!("incircle {p:?}");
            if let Some(fast) = copy::ic_fast(p) {
                answered += 1;
                assert_eq!(wide_big(&fast), exact, "{at}: the fast path");
            }
            assert_eq!(copy::ic_routed(p), sign_of(&exact), "{at}: routed sign");
            assert_eq!(ic_total(p), exact, "{at}: the total path");
        }
    }
    assert!(answered > 0, "the fast path never answered");
}

/// insphere off the grid where the fast path's `I256` sum cannot hold the answer: the flat
/// tetrahedron of `IS_OVER` over every choice of the four small z in {−3, …, 3}. Every lift is
/// about 2^127 and every cofactor about 2^127 with the signs lined up, so the exact value reaches
/// about ±2^256 while every factor still fits `i128`; wrapped to 256 bits such a value has the
/// opposite sign. The fast path is exact wherever it answers and gives up wherever the value
/// cannot fit, and the predicate's body always has the exact sign.
#[test]
fn insphere_where_the_i256_sum_overflows_off_the_grid() {
    let levels: [i64; 7] = [-3, -2, -1, 0, 1, 2, 3];
    let mut beyond = 0;
    for z in lattice::<4>(&levels) {
        let mut p = IS_OVER;
        for k in 0..4 {
            p[k][2] = z[k];
        }
        let exact = is_exact(p);
        let at = format!("insphere {p:?}");
        let fast = copy::is_fast(p).map(|v| wide_big(&v));
        let all_factors_fit = copy::is_terms(p).is_some();
        if all_factors_fit && exact.abs() >= pow2(255) {
            beyond += 1;
            overflow_must_be_caught(&at, &exact, fast.as_ref(), copy::is_routed(p), 256);
        } else if let Some(fast) = fast {
            assert_eq!(fast, exact, "{at}: the fast path");
        }
        assert_eq!(copy::is_routed(p), sign_of(&exact), "{at}: routed sign");
        assert_eq!(is_total(p), exact, "{at}: the total path");
    }
    assert!(beyond > 0, "no sum went past 2^255");
    eprintln!("{beyond} in-sphere configurations with a sum past I256");
}

proptest! {
    #![proptest_config(cfg(1024))]

    /// Random points with coordinates of every magnitude: wherever the fast path answers it is
    /// exact, and the predicate's body always has the exact sign.
    #[test]
    fn off_the_grid_the_fast_path_is_exact_or_silent(
        v in uniform::<_, 5>(uniform::<_, 3>(wild())),
    ) {
        let flat: [Pt2; 5] = v.map(|[x, y, _]| [x, y]);
        let p2 = [flat[0], flat[1], flat[2]];
        let exact = o2_exact(p2);
        if let Some(fast) = copy::o2_fast(p2) {
            prop_assert_eq!(BigInt::from(fast), exact.clone(), "orient2d {:?}", p2);
        }
        prop_assert_eq!(copy::o2_routed(p2), sign_of(&exact), "orient2d {:?}", p2);
        let q2 = [flat[0], flat[1], flat[2], flat[3]];
        let exact = ic_exact(q2);
        if let Some(fast) = copy::ic_fast(q2) {
            prop_assert_eq!(wide_big(&fast), exact.clone(), "incircle {:?}", q2);
        }
        prop_assert_eq!(copy::ic_routed(q2), sign_of(&exact), "incircle {:?}", q2);
        let p3 = [v[0], v[1], v[2], v[3]];
        let exact = o3_exact(p3);
        if let Some(fast) = copy::o3_fast(p3) {
            prop_assert_eq!(BigInt::from(fast), exact.clone(), "orient3d {:?}", p3);
        }
        prop_assert_eq!(copy::o3_routed(p3), sign_of(&exact), "orient3d {:?}", p3);
        let exact = is_exact(v);
        if let Some(fast) = copy::is_fast(v) {
            prop_assert_eq!(wide_big(&fast), exact.clone(), "insphere {:?}", v);
        }
        prop_assert_eq!(copy::is_routed(v), sign_of(&exact), "insphere {:?}", v);
    }
}

// ---------------------------------------------------------------------------------------------
// Cocircular and cospherical points in general position, at large scale.
// ---------------------------------------------------------------------------------------------

/// The Gaussian primes a + bi for the 14 primes p ≡ 1 (mod 4) below 114, each p = a² + b².
const GAUSSIAN_PRIMES: [(i128, i128); 14] = [
    (2, 1),
    (3, 2),
    (4, 1),
    (5, 2),
    (6, 1),
    (5, 4),
    (7, 2),
    (6, 5),
    (8, 3),
    (8, 5),
    (9, 4),
    (10, 1),
    (10, 3),
    (8, 7),
];

/// A point (x, y) with x² + y² = N, N being the product of those 14 primes (about 1.2 · 10^23, so
/// the radius is about 2^38.35): the product of each Gaussian prime or, where `mask` has its bit
/// set, of its conjugate. The norm is multiplicative, so every mask lands on the same circle, and
/// by unique factorisation in the Gaussian integers different masks land on different points.
fn on_big_circle(mask: usize) -> Pt2 {
    let (mut x, mut y) = (1i128, 0i128);
    for (k, &(a, b)) in GAUSSIAN_PRIMES.iter().enumerate() {
        let b = if (mask >> k) & 1 == 1 { -b } else { b };
        (x, y) = (x * a - y * b, x * b + y * a);
    }
    [i64::try_from(x).unwrap(), i64::try_from(y).unwrap()]
}

/// p under one of the eight symmetries of the square: bit 2 of `k` swaps the axes, bits 0 and 1
/// negate x and y.
fn square_symmetry(p: Pt2, k: usize) -> Pt2 {
    let [x, y] = if k & 4 == 0 { p } else { [p[1], p[0]] };
    let sign = |bit: usize| if k & bit == 0 { 1 } else { -1 };
    [x * sign(1), y * sign(2)]
}

/// q moved one grid step along the axis on which it is farthest from `centre`, toward the centre or
/// away from it. Its squared distance to the centre changes by 1 − 2|`q_k` − `centre_k`| < 0 toward
/// it (|`q_k` − `centre_k`| ≥ 1 on that axis unless q is the centre) and by
/// 1 + 2|`q_k` − `centre_k`| > 0 away from it.
fn step<const D: usize>(q: [i64; D], centre: [i64; D], toward: bool) -> [i64; D] {
    let k = (0..D).max_by_key(|&k| (q[k] - centre[k]).abs()).unwrap();
    let away = (q[k] - centre[k]).signum();
    let mut out = q;
    out[k] += if toward { -away } else { away };
    out
}

/// 64 points on one circle of radius about 2^38.35 centred off the origin, coordinates up to
/// about 0.82 M, from the Gaussian integers above. For 2,000 random choices of four of them,
/// incircle is `Zero` by both paths, the fast path's sum cancels to exactly zero, and d one step
/// inside gives orient2d(a, b, c), one step outside its opposite, by both paths.
#[test]
fn cocircular_points_in_general_position_at_large_scale() {
    let centre: Pt2 = [M / 2 - 1_234_567, 7_654_321 - M / 2];
    let norms = GAUSSIAN_PRIMES.map(|(a, b)| a * a + b * b);
    let radius2: i128 = norms.iter().product();
    let mut rng = Rng(5);
    let mut points = Vec::new();
    for _ in 0..64 {
        let q = on_big_circle(rng.below(1 << 14));
        let [x, y] = square_symmetry(q, rng.below(8));
        let r2 = i128::from(x).pow(2) + i128::from(y).pow(2);
        assert_eq!(r2, radius2, "({x}, {y}) is not on the circle");
        points.push([centre[0] + x, centre[1] + y]);
    }
    for _ in 0..2_000 {
        let p: [Pt2; 4] = std::array::from_fn(|_| points[rng.below(points.len())]);
        let at = format!("incircle {p:?}");
        assert_eq!(ic(p), Sign::Zero, "{at}: four points on one circle");
        assert_eq!(copy::ic_forced(p), Sign::Zero, "{at}: the total path");
        assert_eq!(copy::ic_fast(p), Some(I256::ZERO), "{at}: the fast path");
        let turn = o2([p[0], p[1], p[2]]);
        for (toward, expected) in [(true, turn), (false, turn.negate())] {
            let mut q = p;
            q[3] = step(p[3], centre, toward);
            let at = format!("incircle {q:?}, d moved toward the centre: {toward}");
            assert_eq!(ic(q), expected, "{at}");
            assert_eq!(copy::ic_forced(q), expected, "{at}: the total path");
        }
    }
}

/// Every quaternion (m, n, p, r) with m² + n² + p² + r² = 25: by Jacobi's four-square theorem
/// there are 8σ(25) = 248.
fn norm_25_quaternions() -> Vec<[i64; 4]> {
    let mut out = Vec::new();
    for m in -5i64..=5 {
        for n in -5i64..=5 {
            for p in -5i64..=5 {
                for r in -5i64..=5 {
                    if m * m + n * n + p * p + r * r == 25 {
                        out.push([m, n, p, r]);
                    }
                }
            }
        }
    }
    assert_eq!(out.len(), 248, "Jacobi's four-square theorem");
    out
}

/// The Hamilton product of two quaternions written (w, x, y, z).
fn hamilton(a: [i64; 4], b: [i64; 4]) -> [i64; 4] {
    let ([aw, ax, ay, az], [bw, bx, by, bz]) = (a, b);
    [
        aw * bw - ax * bx - ay * by - az * bz,
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
    ]
}

/// q i q̄ for q = m + n i + p j + r k: the vector (m² + n² − p² − r², 2(np + mr), 2(nr − mp)),
/// whose length is m² + n² + p² + r² (the rotation q stands for, applied to i and scaled by |q|²;
/// Euler's four-square identity).
fn rotated_i([m, n, p, r]: [i64; 4]) -> Pt3 {
    [
        m * m + n * n - p * p - r * r,
        2 * (n * p + m * r),
        2 * (n * r - m * p),
    ]
}

/// 48 points on one sphere of radius 25^8 ≈ 2^37.15 centred off the origin: each is i rotated by a
/// product of eight random quaternions of norm 25, so they lie in general position, not on the
/// planes of symmetry of a box. For 1,500 random choices of five, insphere is `Zero` by both paths
/// and the fast path's sum is exactly zero, and e one step inside gives orient3d(a, b, c, d), one
/// step outside its opposite, by both paths.
#[test]
fn cospherical_points_in_general_position_at_large_scale() {
    let units = norm_25_quaternions();
    let radius = 25i64.pow(8);
    let centre: Pt3 = [M / 2 - 3, 5 - M / 3, M / 5 + 7];
    let mut rng = Rng(6);
    let mut points = Vec::new();
    for _ in 0..48 {
        let mut q = [1, 0, 0, 0];
        for _ in 0..8 {
            q = hamilton(q, units[rng.below(units.len())]);
        }
        let v = rotated_i(q);
        let r2: i128 = v.iter().map(|&x| i128::from(x).pow(2)).sum();
        assert_eq!(r2, i128::from(radius).pow(2), "{v:?} is off the sphere");
        let p: Pt3 = std::array::from_fn(|k| centre[k] + v[k]);
        points.push(p);
    }
    for _ in 0..1_500 {
        let p: [Pt3; 5] = std::array::from_fn(|_| points[rng.below(points.len())]);
        let at = format!("insphere {p:?}");
        assert_eq!(ins(p), Sign::Zero, "{at}: five points on one sphere");
        assert_eq!(copy::is_forced(p), Sign::Zero, "{at}: the total path");
        assert_eq!(copy::is_fast(p), Some(I256::ZERO), "{at}: the fast path");
        let orientation = o3([p[0], p[1], p[2], p[3]]);
        for (toward, expected) in [(true, orientation), (false, orientation.negate())] {
            let mut q = p;
            q[4] = step(p[4], centre, toward);
            let at = format!("insphere {q:?}, e moved toward the centre: {toward}");
            assert_eq!(ins(q), expected, "{at}");
            assert_eq!(copy::is_forced(q), expected, "{at}: the total path");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The documented degenerate identities, as exact values on the total path at `i64` scale.
// ---------------------------------------------------------------------------------------------

/// The range of k for which base + k·w stays within `i64` on every axis. It always holds 0.
fn line_range<const D: usize>(base: [i64; D], w: [i64; D]) -> (i64, i64) {
    let (mut lo, mut hi) = (i128::from(LO), i128::from(HI));
    for (&b, &s) in base.iter().zip(&w) {
        let (b, s) = (i128::from(b), i128::from(s));
        // LO ≤ b + k·s ≤ HI, written as from ≤ k·|s| ≤ to.
        let (from, to) = (i128::from(LO) - b, i128::from(HI) - b);
        let (from, to) = if s < 0 { (-to, -from) } else { (from, to) };
        let size = s.abs();
        if size > 0 {
            lo = lo.max(-((-from).div_euclid(size)));
            hi = hi.min(to.div_euclid(size));
        }
    }
    (i64::try_from(lo).unwrap(), i64::try_from(hi).unwrap())
}

/// base + k·w, which `line_range` keeps within `i64`.
fn along<const D: usize>(base: [i64; D], w: [i64; D], k: i64) -> [i64; D] {
    let coordinate = |i: usize| i128::from(base[i]) + i128::from(k) * i128::from(w[i]);
    std::array::from_fn(|i| i64::try_from(coordinate(i)).unwrap())
}

/// Three points on the line through `base` with direction `w`, as far out as `i64` allows half the
/// time: base itself and two more at random multiples of w.
fn collinear<const D: usize>(base: [i64; D], w: [i64; D], seed: u64) -> [[i64; D]; 3] {
    let (lo, hi) = line_range(base, w);
    let mut rng = Rng(seed);
    let mut pick = || match rng.below(4) {
        0 => lo,
        1 => hi,
        _ => rng.within(lo, hi),
    };
    let (s, t) = (pick(), pick());
    [base, along(base, w, s), along(base, w, t)]
}

/// (c − a) · (c − b): the power of c with respect to every circle or sphere through a and b, when
/// c is on their line.
fn power_on_line<const D: usize>(a: [i64; D], b: [i64; D], c: [i64; D]) -> BigInt {
    let term = |k: usize| (BigInt::from(c[k]) - a[k]) * (BigInt::from(c[k]) - b[k]);
    (0..D).map(term).sum()
}

/// p − q.
fn vector(p: Pt3, q: Pt3) -> [BigInt; 3] {
    std::array::from_fn(|k| BigInt::from(p[k]) - q[k])
}

/// u × v.
fn cross(u: &[BigInt; 3], v: &[BigInt; 3]) -> [BigInt; 3] {
    std::array::from_fn(|k| {
        let (i, j) = ((k + 1) % 3, (k + 2) % 3);
        &u[i] * &v[j] - &u[j] * &v[i]
    })
}

/// u · v.
fn dot(u: &[BigInt; 3], v: &[BigInt; 3]) -> BigInt {
    u.iter().zip(v).map(|(x, y)| x * y).sum()
}

/// A direction for a line: small enough that the line holds many `i64` points.
fn direction<const D: usize>() -> impl Strategy<Value = [i64; D]> {
    uniform::<_, D>(-(1i64 << 20)..=1 << 20)
}

proptest! {
    #![proptest_config(cfg(256))]

    /// incircle with a, b, c on one line is exactly orient2d's determinant of (a, b, d) times
    /// (c − a) · (c − b), as the documentation derives, at `i64` scale on the total path.
    #[test]
    fn incircle_with_collinear_a_b_c_keeps_the_documented_identity(
        base in uniform::<_, 2>(wild()),
        w in direction::<2>(),
        d in uniform::<_, 2>(wild()),
        seed in any::<u64>(),
    ) {
        let [a, b, c] = collinear(base, w, seed);
        let p = [a, b, c, d];
        let expected = o2_exact([a, b, d]) * power_on_line(a, b, c);
        prop_assert_eq!(ic_total(p), expected.clone(), "incircle {:?}", p);
        prop_assert_eq!(copy::ic_routed(p), sign_of(&expected), "incircle {:?}", p);
    }

    /// insphere with a, b, c on one line is exactly minus orient3d's determinant of (a, b, d, e)
    /// times (c − a) · (c − b), as the documentation derives, at `i64` scale on the total path.
    #[test]
    fn insphere_with_collinear_a_b_c_keeps_the_documented_identity(
        base in uniform::<_, 3>(wild()),
        w in direction::<3>(),
        d in uniform::<_, 3>(wild()),
        e in uniform::<_, 3>(wild()),
        seed in any::<u64>(),
    ) {
        let [a, b, c] = collinear(base, w, seed);
        let p = [a, b, c, d, e];
        let expected = -o3_exact([a, b, d, e]) * power_on_line(a, b, c);
        prop_assert_eq!(is_total(p), expected.clone(), "insphere {:?}", p);
        prop_assert_eq!(copy::is_routed(p), sign_of(&expected), "insphere {:?}", p);
    }

    /// insphere with a, b, c, d in one plane and a, b, c not collinear is exactly orient3d's
    /// determinant of (a, b, c, e) times |d − o|² − r², o and r being the centre and radius of the
    /// circle through a, b, c, as the documentation derives, at `i64` scale on the total path.
    #[test]
    fn insphere_with_coplanar_a_b_c_d_keeps_the_documented_identity(
        a in uniform::<_, 3>(wild()),
        u in direction::<3>(),
        v in direction::<3>(),
        e in uniform::<_, 3>(wild()),
        seed in any::<u64>(),
    ) {
        let [_, b, _] = collinear(a, u, seed);
        let [_, c, _] = collinear(a, v, seed ^ 1);
        let [_, on_u, _] = collinear(a, u, seed ^ 2);
        let [_, d, _] = collinear(on_u, v, seed ^ 3);
        let (bu, cv) = (vector(b, a), vector(c, a));
        let normal = cross(&bu, &cv);
        prop_assume!(normal.iter().any(|x| !x.is_zero()), "a, b, c are collinear");
        // With m = o − a = (|u|² v × n + |v|² n × u) / (2|n|²) for u = b − a, v = c − a and
        // n = u × v, d's power is |w|² − 2 w · m for w = d − a: (|w|² den − 2 w · num) / den.
        let num: [BigInt; 3] = {
            let (left, right) = (cross(&cv, &normal), cross(&normal, &bu));
            let (uu, vv) = (dot(&bu, &bu), dot(&cv, &cv));
            std::array::from_fn(|k| &uu * &left[k] + &vv * &right[k])
        };
        let den = BigInt::from(2) * dot(&normal, &normal);
        let w = vector(d, a);
        let power_times_den = dot(&w, &w) * &den - BigInt::from(2) * dot(&w, &num);
        let p = [a, b, c, d, e];
        let expected_times_den = o3_exact([a, b, c, e]) * power_times_den;
        prop_assert_eq!(is_total(p) * &den, expected_times_den.clone(), "insphere {:?}", p);
        prop_assert_eq!(copy::is_routed(p), sign_of(&expected_times_den), "insphere {:?}", p);
    }

    /// Any two equal points make every predicate exactly zero, anywhere in `i64`.
    #[test]
    fn two_equal_points_give_zero_anywhere_in_i64(
        v in uniform::<_, 5>(uniform::<_, 3>(wild())),
        pair in (0usize..5, 0usize..4),
    ) {
        let (i, j) = (pair.0, (pair.0 + 1 + pair.1) % 5);
        let mut q = v;
        q[j] = q[i];
        prop_assert_eq!(is_total(q), BigInt::zero(), "insphere {:?}", q);
        prop_assert_eq!(copy::is_routed(q), Sign::Zero, "insphere {:?}", q);
        if i < 4 && j < 4 {
            let p3 = [q[0], q[1], q[2], q[3]];
            prop_assert_eq!(o3_total(p3), BigInt::zero(), "orient3d {:?}", p3);
            let p2: [Pt2; 4] = p3.map(|[x, y, _]| [x, y]);
            prop_assert_eq!(ic_total(p2), BigInt::zero(), "incircle {:?}", p2);
            if i < 3 && j < 3 {
                let t2 = [p2[0], p2[1], p2[2]];
                prop_assert_eq!(o2_total(t2), BigInt::zero(), "orient2d {:?}", t2);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Symmetry laws.
// ---------------------------------------------------------------------------------------------

/// p with every coordinate times `s`, if every product is still an `i64`.
fn scaled<const D: usize, const N: usize>(p: [[i64; D]; N], s: i64) -> Option<[[i64; D]; N]> {
    let mut out = p;
    for point in &mut out {
        for x in point.iter_mut() {
            *x = x.checked_mul(s)?;
        }
    }
    Some(out)
}

/// A translation, drawn from `seed`, that keeps every coordinate of p within lo..=hi.
fn translation<const D: usize, const N: usize>(
    p: [[i64; D]; N],
    lo: i64,
    hi: i64,
    seed: u64,
) -> [i64; D] {
    let mut rng = Rng(seed);
    std::array::from_fn(|k| {
        let least = p.iter().map(|q| q[k]).min().unwrap();
        let most = p.iter().map(|q| q[k]).max().unwrap();
        let from = (i128::from(lo) - i128::from(least)).max(i128::from(LO));
        let to = (i128::from(hi) - i128::from(most)).min(i128::from(HI));
        let from = i64::try_from(from).unwrap();
        let to = i64::try_from(to).unwrap();
        rng.within(from, to)
    })
}

/// p translated by t.
fn moved<const D: usize, const N: usize>(p: [[i64; D]; N], t: [i64; D]) -> [[i64; D]; N] {
    p.map(|q| std::array::from_fn(|k| q[k] + t[k]))
}

/// p with its axes rotated one place: (x, y) ↦ (y, x) in 2D, (x, y, z) ↦ (y, z, x) in 3D, a
/// reflection in 2D and a rotation in 3D.
fn axes_rotated<const D: usize, const N: usize>(p: [[i64; D]; N]) -> [[i64; D]; N] {
    p.map(|q| std::array::from_fn(|k| q[(k + 1) % D]))
}

/// p with the axes whose bit is set in `mask` negated, which keeps grid points on the grid.
fn negated<const D: usize, const N: usize>(p: [[i64; D]; N], mask: usize) -> [[i64; D]; N] {
    let one = |q: [i64; D]| -> [i64; D] {
        std::array::from_fn(|k| if (mask >> k) & 1 == 1 { -q[k] } else { q[k] })
    };
    p.map(one)
}

/// The exact-value laws of a determinant predicate of degree `degree` on its total path, anywhere
/// in `i64`: every reordering multiplies the value by the ordering's sign; reflecting k axes
/// about −1/2 multiplies it by (−1)^k; rotating the axes one place by (−1)^(D−1); a translation
/// that keeps every coordinate an `i64` leaves it unchanged; scaling by s, where the coordinates
/// still fit, multiplies it by s^degree.
fn value_laws<const D: usize, const N: usize>(
    what: &str,
    total: fn([[i64; D]; N]) -> BigInt,
    degree: u32,
    p: [[i64; D]; N],
    seed: u64,
    s: i64,
) {
    let v = total(p);
    let check = |q: [[i64; D]; N], expected: &BigInt, how: &str| {
        let got = total(q);
        assert_eq!(&got, expected, "{what} {q:?}, {how} {p:?}");
    };
    let minus = -&v;
    for (order, odd) in permutations(N) {
        let expected = if *odd { &minus } else { &v };
        check(reorder(p, order), expected, "reordered from");
    }
    for mask in 1..1usize << D {
        let odd = mask.count_ones() % 2 == 1;
        let expected = if odd { &minus } else { &v };
        check(p.map(|q| flip(q, mask)), expected, "reflected from");
    }
    let expected = if D % 2 == 0 { &minus } else { &v };
    check(axes_rotated(p), expected, "axes rotated from");
    let shift = translation(p, LO, HI, seed);
    check(moved(p, shift), &v, "translated from");
    if let Some(q) = scaled(p, s) {
        let expected = &v * BigInt::from(s).pow(degree);
        check(q, &expected, "scaled from");
    }
}

/// A scale factor: small, sometimes negative.
fn scale() -> impl Strategy<Value = i64> {
    prop_oneof![Just(-1i64), Just(2i64), Just(-2i64), Just(3i64)]
}

proptest! {
    #![proptest_config(cfg(256))]

    /// The exact-value symmetry laws hold on the total path for coordinates of every magnitude.
    #[test]
    fn total_path_values_obey_every_symmetry_anywhere_in_i64(
        v in uniform::<_, 5>(uniform::<_, 3>(wild())),
        seed in any::<u64>(),
        s in scale(),
    ) {
        let flat: [Pt2; 5] = v.map(|[x, y, _]| [x, y]);
        value_laws("orient2d", o2_total, 2, [flat[0], flat[1], flat[2]], seed, s);
        value_laws("incircle", ic_total, 4, [flat[0], flat[1], flat[2], flat[3]], seed, s);
        value_laws("orient3d", o3_total, 3, [v[0], v[1], v[2], v[3]], seed, s);
        value_laws("insphere", is_total, 5, v, seed, s);
    }
}

/// The sign laws of a predicate on the grid, through the crate and through the forced total path,
/// which must agree everywhere: every reordering multiplies the sign by the ordering's sign,
/// negating k axes by (−1)^k, rotating the axes one place by (−1)^(D−1), and a translation that
/// keeps every point on the grid leaves it unchanged.
fn sign_laws<const D: usize, const N: usize>(
    what: &str,
    public: fn([[i64; D]; N]) -> Sign,
    forced: fn([[i64; D]; N]) -> Sign,
    p: [[i64; D]; N],
    seed: u64,
) {
    let s = public(p);
    let check = |q: [[i64; D]; N], expected: Sign, how: &str| {
        let at = format!("{what} {q:?}, {how} {p:?}");
        assert_eq!(public(q), expected, "{at}");
        assert_eq!(forced(q), expected, "{at}: the total path");
    };
    check(p, s, "the same as");
    for (order, odd) in permutations(N) {
        let expected = if *odd { s.negate() } else { s };
        check(reorder(p, order), expected, "reordered from");
    }
    for mask in 1..1usize << D {
        let odd = mask.count_ones() % 2 == 1;
        let expected = if odd { s.negate() } else { s };
        check(negated(p, mask), expected, "axes negated from");
    }
    let expected = if D % 2 == 0 { s.negate() } else { s };
    check(axes_rotated(p), expected, "axes rotated from");
    let shift = translation(p, -M, M, seed);
    check(moved(p, shift), s, "translated from");
}

proptest! {
    #![proptest_config(cfg(256))]

    /// The sign laws hold on the grid, with the crate and the forced total path in agreement after
    /// every transformation.
    #[test]
    fn grid_signs_obey_every_symmetry_by_both_paths(
        v in uniform::<_, 5>(uniform::<_, 3>(on_grid())),
        seed in any::<u64>(),
    ) {
        let flat: [Pt2; 5] = v.map(|[x, y, _]| [x, y]);
        sign_laws("orient2d", o2, copy::o2_forced, [flat[0], flat[1], flat[2]], seed);
        sign_laws("incircle", ic, copy::ic_forced, [flat[0], flat[1], flat[2], flat[3]], seed);
        sign_laws("orient3d", o3, copy::o3_forced, [v[0], v[1], v[2], v[3]], seed);
        sign_laws("insphere", ins, copy::is_forced, v, seed);
    }
}

// ---------------------------------------------------------------------------------------------
// Stack: every predicate runs in a small, fixed stack.
// ---------------------------------------------------------------------------------------------

/// How much stack each predicate needs in the current build profile (principle 13: WebAssembly
/// stacks have no guard page, so an overflow there corrupts memory instead of stopping the
/// program). A stack overflow aborts the whole process, so each measurement runs in a child
/// process: this test binary again, running only `stack::probe`. The probe's thread has a fixed
/// stack; recursion holds a chosen number of padding frames on it and runs the predicate below
/// them. The deepest padding at which the predicate still finishes, against the deepest at which
/// doing nothing finishes, gives the predicate's stack use to within one padding frame. Not built
/// for WebAssembly, which has no processes to spawn.
#[cfg(not(target_family = "wasm"))]
mod stack {
    use std::fmt::Write;
    use std::hint::black_box;
    use std::process::Command;
    use std::ptr;
    use std::thread;

    use crate::{IC_OVER, IS_OVER, M, O2_OVER, O3_OVER, Pt2, Pt3, copy, ic, ins, o2, o3};

    /// The environment variable that turns `probe` on, holding "<work> <frames>".
    const PROBE: &str = "GEOM_NUM_ATTACK2_STACK_PROBE";
    /// What a probe prints when its work has finished.
    const FINISHED: &str = "attack2 stack probe finished";
    /// The probe thread's stack.
    const STACK: usize = 256 * 1024;
    /// The bytes of padding each frame holds.
    const PAD: usize = 256;
    /// The most stack one predicate call may use: 1/32 of the 1 MiB stack Rust gives a WebAssembly
    /// program by default, leaving the rest to the algorithms that call predicates.
    const BUDGET: usize = 32 * 1024;

    /// What a probe can run, by number: nothing, the four public predicates at the corners of the
    /// grid, and the four predicates' own bodies off the grid where the fast path runs to a late
    /// step, gives up and the total path follows: the deepest route through each.
    const WORKS: [&str; 9] = [
        "nothing",
        "orient2d",
        "orient3d",
        "incircle",
        "insphere",
        "orient2d, fast path giving up, then total path",
        "orient3d, fast path giving up, then total path",
        "incircle, fast path giving up, then total path",
        "insphere, fast path giving up, then total path",
    ];

    fn run(work: usize) {
        let n = -M;
        let square: [Pt2; 4] = [[M, M], [n, M], [n, n], [M, n]];
        let cube: [Pt3; 5] = [[M, M, M], [M, M, n], [M, n, M], [n, M, M], [n, n, n]];
        let sign = match work {
            1 => o2([square[0], square[1], square[2]]),
            2 => o3([cube[0], cube[1], cube[2], cube[3]]),
            3 => ic(square),
            4 => ins(cube),
            5 => copy::o2_routed(O2_OVER),
            6 => copy::o3_routed(O3_OVER),
            7 => copy::ic_routed(IC_OVER),
            8 => copy::is_routed(IS_OVER),
            _ => return,
        };
        black_box(sign);
    }

    /// Holds `frames` frames of at least `PAD` bytes each, runs `work` below them, and returns the
    /// addresses of the outermost and the innermost padding.
    fn burn(frames: usize, work: &dyn Fn()) -> [usize; 2] {
        let pad = black_box([0u8; PAD]);
        let here = ptr::from_ref(&pad).addr();
        let innermost = if frames == 0 {
            work();
            here
        } else {
            burn(frames - 1, work)[1]
        };
        black_box(&pad);
        [here, innermost]
    }

    /// The child's side. It does nothing unless `PROBE` is set, which only the test below does.
    #[test]
    fn probe() {
        let Ok(spec) = std::env::var(PROBE) else {
            return;
        };
        let numbers: Vec<usize> = spec.split(' ').map(|n| n.parse().unwrap()).collect();
        let (work, frames) = (numbers[0], numbers[1]);
        let builder = thread::Builder::new().stack_size(STACK);
        let child = builder.spawn(move || burn(frames, &|| run(work))).unwrap();
        child.join().unwrap();
        println!("{FINISHED}");
    }

    /// Whether `work` finishes below `frames` padding frames, in a child process.
    fn fits(work: usize, frames: usize) -> bool {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["stack::probe", "--exact", "--nocapture"])
            .env(PROBE, format!("{work} {frames}"))
            .output()
            .unwrap();
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains(FINISHED)
    }

    /// The most padding frames in lo..=hi below which `work` still finishes, given that it
    /// finishes at lo.
    fn deepest(work: usize, lo: usize, hi: usize) -> usize {
        let (mut fit, mut over) = (lo, hi + 1);
        while over - fit > 1 {
            let mid = fit.midpoint(over);
            if fits(work, mid) {
                fit = mid;
            } else {
                over = mid;
            }
        }
        fit
    }

    /// Every predicate, by every route, needs at most `BUDGET` bytes of stack in this build
    /// profile, the total path included.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "starts child processes and measures native stack frames; Miri runs neither"
    )]
    fn every_predicate_runs_in_a_small_fixed_stack() {
        let [outer, inner] = burn(16, &|| {});
        let frame = outer.abs_diff(inner) / 16;
        assert!(frame >= PAD, "a padding frame holds only {frame} bytes");
        assert!(fits(0, 0), "the probe did not run");
        let base = deepest(0, 0, STACK / frame);
        let allowance = BUDGET.div_ceil(frame);
        let mut report = format!("stack per predicate call (padding frame {frame} bytes):\n");
        let mut over = Vec::new();
        for (work, name) in WORKS.iter().enumerate().skip(1) {
            let lo = base.saturating_sub(allowance);
            if fits(work, lo) {
                let used = (base - deepest(work, lo, base)) * frame;
                writeln!(report, "  {name}: {used} bytes, to within {frame}").unwrap();
            } else {
                over.push(*name);
                writeln!(report, "  {name}: more than {BUDGET} bytes").unwrap();
            }
        }
        eprintln!("{report}");
        assert!(over.is_empty(), "over {BUDGET} bytes: {over:?}\n{report}");
    }
}
