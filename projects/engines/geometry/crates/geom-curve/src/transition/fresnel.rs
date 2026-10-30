//! The Fresnel integrals C(x) = ∫₀ˣ cos(πt²/2) dt and S(x) = ∫₀ˣ sin(πt²/2) dt, enclosed: the
//! clothoid's own functions, by series and asymptotic expansion with their remainder bounds, and
//! by the transition kind's certified quadrature between them; and a clothoid's point by them,
//! [`Transition::enclose_fresnel`] (piece G67).
//!
//! What it does: [`fresnel`] encloses C and S over an interval of arguments. Both are odd, so the
//! work is at |x|, at the double m nearest the interval's middle, in one of four ranges:
//!
//! | Range | Method | Remainder |
//! | --- | --- | --- |
//! | \|x\| < 2⁻³⁰⁰ | C(x) in [x − x³, x], S(x) in [0, x³]: the series' first terms | x² may underflow |
//! | \|x\| ≤ 1 | the power series of DLMF 7.6.4 and 7.6.6: C = Σ (−1)ⁿ·x·y²ⁿ/((2n)!·(4n + 1)) and S = Σ (−1)ⁿ·x·y²ⁿ⁺¹/((2n + 1)!·(4n + 3)), with y = πx²/2 | each is alternating, its terms falling once n + 2 > y, so its tail is at most its first neglected term |
//! | \|x\| ≥ 5 | C = ½ + f·sin(πx²/2) − g·cos(πx²/2) and S = ½ − f·cos(πx²/2) − g·sin(πx²/2) (DLMF 7.5.3 and 7.5.4), with the asymptotic expansions of the auxiliary functions f and g (DLMF 7.12.2 and 7.12.3) | for real x, each expansion's remainder is at most its first neglected term (DLMF 7.12(ii)) |
//! | between | C(1) and S(1) by the series, plus the integral from 1 by the transition kind's 8-point Gauss–Legendre rule on panels of width 2⁻ᵏ, each panel's remainder bounded by Trefethen's theorem (`super`'s `trefethen`, the least over five ellipses) at most 2⁻⁶²; at a node t = a + δ of a panel starting at a, πt²/2 = π·a²/2 + π·(a·δ + δ²/2), the first part a double taken exactly | the panels' bounds, summed |
//!
//! The interval's other arguments add at most its radius r times the integrands' bound, 1: C and
//! S over the interval lie within r of their values at m.
//!
//! Guarantees:
//!
//! - **Certified.** Each enclosure holds C(x) and S(x) for every x of the interval: every series
//!   and expansion summed in interval arithmetic with its remainder bound added, π between its
//!   neighbouring doubles, the angle πx²/2 taken from x² split exactly into two doubles
//!   (`geom-num`'s two-product) so `geom-num`'s proven `sinpi` and `cospi` see it exactly however
//!   large x is, and the quadrature's remainder bounded, never estimated.
//! - **Tight.** About 10⁻¹⁵ wide on the expansion's range, a few times that on the series', and a
//!   few times 10⁻¹⁴ between (the rounding of a few dozen panels).
//! - **Deterministic.** Fixed ranges, a fixed series stop, panels fixed by their position alone:
//!   the same bits on every target.
//!
//! Method: written from the published mathematical forms, DLMF (NIST Digital Library of
//! Mathematical Functions), §7.2 (definitions), §7.5 (7.5.3, 7.5.4), §7.6 (7.6.4, 7.6.6) and §7.12
//! (7.12.2, 7.12.3 and the remainder bounds of 7.12(ii)), <https://dlmf.nist.gov/7>, read
//! 2026-10-30, as chapter 17.8 asks; the alternating series' remainder by Leibniz's test; and the
//! quadrature of the transition kind (L. N. Trefethen, "Is Gauss quadrature better than
//! Clenshaw–Curtis?", SIAM Review 50 (2008), <https://doi.org/10.1137/060659831>, Theorem 4.5). No
//! code from the Clothoids library's `Fresnel.cc` (chapter 29's row for it), and none of its
//! rational approximations.
//!
//! Cost: the series about 20 terms (about 2 µs), the expansion at most 64 (about 2 µs); between,
//! up to about 35 panels of 8 evaluations each, with five of Trefethen's bounds per panel (a few
//! hundred µs: the slow range, which a clothoid near its inflection point never reaches).

use geom_num::eft::two_prod;
use geom_num::interval::Interval;

use geom_run::WorkMeter;

use super::{CurvatureLaw, Heading, Parent, RULE, TERMS, Transition, embed, trefethen};
use crate::error::CurveError;
use crate::form;
use crate::tight::{self, point};

/// Below 2⁻³⁰⁰, C(x) lies in [x − x³, x] and S(x) in [0, x³], and x² may underflow.
const TINY: f64 = 4.909_093_465_297_727e-91;

/// The series' range: |x| ≤ 1.
const SERIES_END: f64 = 1.0;

/// The expansion's range: |x| ≥ 5, where its smallest term is below 10⁻¹⁷.
const EXPANSION_START: f64 = 5.0;

/// The most terms either sum takes.
const MAX_TERMS: usize = 64;

/// A term below 2⁻⁶⁴ ends a sum.
const NEGLIGIBLE: f64 = 1.0 / 18_446_744_073_709_551_616.0;

/// The halvings a panel's width may take from its first guess.
const HALVINGS: usize = 8;

/// The ellipses tried for a panel's remainder: ρ = 4, 8, 16, 32 and 64, where the best lies for
/// the panels' widths here.
const ELLIPSES: [f64; 5] = [4.0, 8.0, 16.0, 32.0, 64.0];

/// Each panel's remainder is at most 2⁻⁶².
const PANEL_TOLERANCE: f64 = 1.0 / 4_611_686_018_427_387_904.0;

/// C(x) and S(x) for every x in `x`, enclosed (module documentation).
///
/// # Errors
///
/// [`CurveError::NotFinite`] for a bound that is not finite; [`CurveError::Defect`] if a panel's
/// remainder stays above its tolerance at the narrowest width (no argument does).
pub fn fresnel(x: Interval) -> Result<[Interval; 2], CurveError> {
    let m = tight::middle(x);
    let [c, s] = at(m)?;
    if x.lo() == x.hi() {
        return Ok([c, s]);
    }
    let r = tight::larger(
        point(m)?.sub(point(x.lo())?)?.hi(),
        point(x.hi())?.sub(point(m)?)?.hi(),
    );
    let spread = Interval::new(-r, r)?;
    Ok([tight::add(c, spread)?, tight::add(s, spread)?])
}

/// C(m) and S(m) at a double, enclosed: odd, so from |m|.
fn at(m: f64) -> Result<[Interval; 2], CurveError> {
    if !m.is_finite() {
        return Err(CurveError::NotFinite);
    }
    let x = m.abs();
    let [c, s] = if x < TINY {
        tiny(x)?
    } else if x <= SERIES_END {
        series(x)?
    } else if x >= EXPANSION_START {
        expansion(x)?
    } else {
        between(x)?
    };
    Ok(if m < 0.0 { [c.neg(), s.neg()] } else { [c, s] })
}

/// πx²/2 for a double x ≥ 0, enclosed from x² = p + e exactly, with sin and cos of it: (y, sin,
/// cos). An x² past the doubles leaves the angle unknown: sin and cos are then [−1, 1].
fn angle(x: f64) -> Result<(Interval, Interval, Interval), CurveError> {
    let (p, e) = two_prod(x, x);
    let half_pi = tight::mul(form::pi()?, point(0.5)?)?;
    if !p.is_finite() {
        let any = Interval::new(-1.0, 1.0)?;
        return Ok((Interval::new(f64::MAX, f64::MAX)?, any, any));
    }
    // πx²/2 = π·(p/2) + π·(e/2): p/2 and e/2 are exact (halving a double), and sinpi and cospi
    // take each exactly.
    let (a, b) = (point(p * 0.5)?, point(e * 0.5)?);
    let (sa, ca, sb, cb) = (a.sinpi()?, a.cospi()?, b.sinpi()?, b.cospi()?);
    let sin = tight::add(tight::mul(sa, cb)?, tight::mul(ca, sb)?)?;
    let cos = tight::sub(tight::mul(ca, cb)?, tight::mul(sa, sb)?)?;
    let y = tight::mul(half_pi, tight::add(point(p)?, point(e)?)?)?;
    Ok((y, unit(sin)?, unit(cos)?))
}

/// An enclosure of a sine or cosine cut to [−1, 1], where they lie.
fn unit(x: Interval) -> Result<Interval, CurveError> {
    Ok(Interval::new(
        tight::larger(x.lo(), -1.0),
        tight::smaller(x.hi(), 1.0),
    )?)
}

/// C and S at 0 ≤ x < 2⁻³⁰⁰: C(x) = x − π²x⁵/40 + … lies in [x − x³, x], and
/// S(x) = πx³/6 − … in [0, x³] (alternating series whose terms fall from the first).
fn tiny(x: f64) -> Result<[Interval; 2], CurveError> {
    let p = point(x)?;
    let cube = tight::mul(tight::mul(p, p)?, p)?;
    Ok([
        Interval::new(tight::sub(p, cube)?.lo(), x)?,
        Interval::new(0.0, tight::larger(cube.hi(), 0.0))?,
    ])
}

/// The power series at 0 ≤ x ≤ 1 (module documentation). There y = πx²/2 ≤ π/2 < 2, so the
/// terms pₙ = x·yⁿ/n! fall from n = 1 on, and each sum's tail after pₙ is at most pₙ/(2n + 1).
fn series(x: f64) -> Result<[Interval; 2], CurveError> {
    let (y, _, _) = angle(x)?;
    if y.hi() >= 2.0 {
        return Err(CurveError::Defect);
    }
    let zero = point(0.0)?;
    let (mut c, mut s) = (zero, zero);
    // pₙ = x·yⁿ/n!; C takes the even n, S the odd, each (−1)^⌊n/2⌋·pₙ/(2n + 1).
    let mut p = point(x)?;
    for n in 0..MAX_TERMS {
        let term = tight::div(
            p,
            point(form::count(n.saturating_mul(2).saturating_add(1))?)?,
        )?;
        let term = if (n >> 1) & 1 == 0 { term } else { term.neg() };
        if n & 1 == 0 {
            c = tight::add(c, term)?;
        } else {
            s = tight::add(s, term)?;
        }
        let next = n.checked_add(1).ok_or(CurveError::Defect)?;
        p = tight::mul(p, tight::div(y, point(form::count(next)?)?)?)?;
        let tail = tight::div(
            p,
            point(form::count(next.saturating_mul(2).saturating_add(1))?)?,
        )?
        .hi();
        // Stop after an S term, so both sums have their first term.
        if n & 1 == 1 && tail <= NEGLIGIBLE {
            let r = Interval::new(-tail, tail)?;
            return Ok([tight::add(c, r)?, tight::add(s, r)?]);
        }
    }
    Err(CurveError::Defect)
}

/// The asymptotic expansion at x ≥ 5 (module documentation).
fn expansion(x: f64) -> Result<[Interval; 2], CurveError> {
    let (y, sin, cos) = angle(x)?;
    let zero = point(0.0)?;
    let (mut f, mut g) = (zero, zero);
    // aₖ = (½)ₖ/yᵏ: f takes the even k with the sign (−1)^(k/2), g the odd with (−1)^((k−1)/2).
    let mut a = point(1.0)?;
    let mut k = 0usize;
    loop {
        let term = if (k >> 1) & 1 == 0 { a } else { a.neg() };
        if k & 1 == 0 {
            f = tight::add(f, term)?;
        } else {
            g = tight::add(g, term)?;
        }
        a = tight::div(tight::mul(a, point(form::count(k)? + 0.5)?)?, y)?;
        k = k.checked_add(1).ok_or(CurveError::Defect)?;
        // Stop at a negligible term, or at the smallest: past k + ½ ≥ y the terms grow, and the
        // first neglected term, which bounds the remainder whatever k is, is then least.
        if a.hi() <= NEGLIGIBLE || form::count(k)? + 0.5 >= y.lo() || k >= MAX_TERMS {
            break;
        }
    }
    // The first neglected terms: aₖ and aₖ₊₁, one of each parity, each a remainder's bound.
    let after = tight::div(tight::mul(a, point(form::count(k)? + 0.5)?)?, y)?;
    let (rf, rg) = if k & 1 == 0 { (a, after) } else { (after, a) };
    // 1/(πx), as (1/π)/x so that a huge x does not overflow.
    let scale = tight::div(tight::div(point(1.0)?, form::pi()?)?, point(x)?)?;
    let f = tight::mul(scale, tight::add(f, Interval::new(-rf.hi(), rf.hi())?)?)?;
    let g = tight::mul(scale, tight::add(g, Interval::new(-rg.hi(), rg.hi())?)?)?;
    let half = point(0.5)?;
    let c = tight::sub(tight::add(half, tight::mul(f, sin)?)?, tight::mul(g, cos)?)?;
    let s = tight::sub(tight::sub(half, tight::mul(f, cos)?)?, tight::mul(g, sin)?)?;
    Ok([c, s])
}

/// The heading ξ²/2 in half-turns: its cosine and sine are the Fresnel integrands.
fn integrand() -> Result<Heading, CurveError> {
    let zero = point(0.0)?;
    let mut poly = [zero; TERMS];
    let mut doubles = [0.0; TERMS];
    if let (Some(c), Some(d)) = (poly.get_mut(2), doubles.get_mut(2)) {
        *c = point(0.5)?;
        *d = 0.5;
    }
    Ok(Heading {
        poly,
        doubles,
        len: 3,
        wave: None,
    })
}

/// cos(π(A + D)) and sin(π(A + D)) for a double A taken exactly and a small enclosure D: by the
/// angle-sum formulas, so A's size costs nothing.
fn split_cos_sin(a: f64, d: Interval) -> Result<(Interval, Interval), CurveError> {
    let pa = point(a)?;
    let (ca, sa, cd, sd) = (pa.cospi()?, pa.sinpi()?, d.cospi()?, d.sinpi()?);
    Ok((
        unit(tight::sub(tight::mul(ca, cd)?, tight::mul(sa, sd)?)?)?,
        unit(tight::add(tight::mul(sa, cd)?, tight::mul(ca, sd)?)?)?,
    ))
}

/// ∫ cos(πt²/2) and ∫ sin(πt²/2) over [a, a + 2·half] by the rule, for a panel start `a` whose
/// square is a double: at each node t = a + δ, t²/2 = a²/2 + (a·δ + δ²/2), the first part exact.
fn panel(a: f64, half: Interval) -> Result<[Interval; 2], CurveError> {
    let (square, error) = two_prod(a, a);
    if error != 0.0 {
        return Err(CurveError::Defect);
    }
    let base = square * 0.5;
    let (pa, one) = (point(a)?, point(1.0)?);
    let zero = point(0.0)?;
    let (mut c, mut s) = (zero, zero);
    for (&node, &weight) in RULE.nodes.iter().zip(RULE.weights) {
        let node = Interval::new(node.next_down(), node.next_up())?;
        let weight = Interval::new(weight.next_down(), weight.next_up())?;
        let delta = tight::mul(half, tight::add(one, node)?)?;
        let small = tight::add(
            tight::mul(pa, delta)?,
            tight::mul(tight::square(delta)?, point(0.5)?)?,
        )?;
        let (cos, sin) = split_cos_sin(base, small)?;
        c = tight::add(c, tight::mul(weight, cos)?)?;
        s = tight::add(s, tight::mul(weight, sin)?)?;
    }
    Ok([tight::mul(c, half)?, tight::mul(s, half)?])
}

/// From the series at 1 by the quadrature's panels, for 1 < x < 5 (module documentation).
fn between(x: f64) -> Result<[Interval; 2], CurveError> {
    let h = integrand()?;
    let [mut c, mut s] = series(SERIES_END)?;
    let mut start = SERIES_END;
    while start < x {
        // The integrand turns about π·start per unit, so start from a width of 2⁻² over [1, 2),
        // 2⁻³ over [2, 4) and 2⁻⁴ from 4; halve until the remainder meets the tolerance.
        let mut width = if start < 2.0 {
            0.25
        } else if start < 4.0 {
            0.125
        } else {
            0.0625
        };
        let mut found = None;
        for _ in 0..HALVINGS {
            let half = width * 0.5;
            let mid = point(start)?.add(point(half)?)?;
            let mut r = f64::INFINITY;
            for rho in ELLIPSES {
                // An ellipse whose bound overflows is skipped: a smaller one may serve.
                if let Ok(b) = trefethen(&h, mid, half, rho, 1.0) {
                    r = tight::smaller(r, b);
                }
            }
            if r <= PANEL_TOLERANCE {
                found = Some(r);
                break;
            }
            width *= 0.5;
        }
        let Some(bound) = found else {
            return Err(CurveError::Defect);
        };
        // A last panel short of its width keeps the whole panel's bound: its ellipse lies inside.
        let end = start + width;
        let half = if end <= x {
            point(width * 0.5)?
        } else {
            tight::mul(tight::sub(point(x)?, point(start)?)?, point(0.5)?)?
        };
        let [dc, ds] = panel(start, half)?;
        let r = Interval::new(-bound, bound)?;
        c = tight::add(tight::add(c, dc)?, r)?;
        s = tight::add(tight::add(s, ds)?, r)?;
        start = end;
    }
    Ok([c, s])
}

impl<const D: usize> Transition<D> {
    /// The parent's point at station `s` by the Fresnel integrals instead of quadrature, for a
    /// parent whose heading is exactly a quadratic αξ² + βξ + γ in ξ = s/L (in half-turns) with α
    /// certainly not zero: the clothoid law, and IFC's `IfcClothoid` and polynomial spirals with
    /// no term past the linear one. `None` for any other law.
    ///
    /// Completing the square gives, for α > 0, u = √(2α)·(ξ + β/(2α)) and πh = πu²/2 − πφ with
    /// φ = β²/(4α) − γ, so the point is the start plus L/√(2α) times
    /// (cos πφ·ΔC + sin πφ·ΔS, cos πφ·ΔS − sin πφ·ΔC), ΔC and ΔS the change of C and S from u(0)
    /// to u(ξ), turned by the start direction; for α < 0 the curve is the mirror image of the one
    /// with −α, −β and −γ. A second route with no quadrature, against which the quadrature is
    /// tested (chapter 17.8); the station is the parent's, as [`Transition::turn`]'s.
    ///
    /// # Errors
    ///
    /// [`CurveError::ParameterOutside`] unless 0 ≤ s ≤ L; [`CurveError::NotFinite`];
    /// [`CurveError::Stopped`] by the meter.
    pub fn enclose_fresnel(
        &self,
        s: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; D]>, CurveError> {
        let parent = &self.parent;
        form::check_parameter((0.0, parent.length), s)?;
        meter.charge(1)?;
        let zero = point(0.0)?;
        Ok(parent.fresnel_point(s)?.map(|p| embed(p, zero)))
    }
}

impl Parent {
    /// The point at station s by the Fresnel integrals ([`Transition::enclose_fresnel`]).
    fn fresnel_point(&self, s: f64) -> Result<Option<[Interval; 2]>, CurveError> {
        // The laws whose heading is exactly a quadratic: decided from the law, since its higher
        // coefficients' enclosures may hold zero without being zero.
        let quadratic = match self.law {
            CurvatureLaw::Clothoid { .. } => true,
            CurvatureLaw::Polynomial { terms, .. } => terms.iter().skip(2).all(Option::is_none),
            _ => false,
        };
        // Such a law has no wave and no second half (`headings`).
        if !quadratic {
            return Ok(None);
        }
        let h = &self.first;
        let mut coefficients = h.poly.iter().copied();
        let zero = point(0.0)?;
        let gamma = coefficients.next().unwrap_or(zero);
        let beta = coefficients.next().unwrap_or(zero);
        let alpha = coefficients.next().unwrap_or(zero);
        let mirror = if alpha.lo() > 0.0 {
            false
        } else if alpha.hi() < 0.0 {
            true
        } else {
            return Ok(None);
        };
        let flip = |x: Interval| if mirror { x.neg() } else { x };
        let (alpha, beta, gamma) = (flip(alpha), flip(beta), flip(gamma));
        // u = √(2α)·(ξ + β/(2α)) and πh = πu²/2 − πφ, φ = β²/(4α) − γ.
        let root = tight::sqrt(tight::mul(point(2.0)?, alpha)?)?;
        let u0 = tight::div(beta, root)?;
        let l = point(self.length)?;
        let xi = tight::div(point(s)?, l)?;
        let u1 = tight::add(tight::mul(root, xi)?, u0)?;
        let phi = tight::sub(
            tight::div(tight::square(beta)?, tight::mul(point(4.0)?, alpha)?)?,
            gamma,
        )?;
        let [c0, s0] = fresnel(u0)?;
        let [c1, s1] = fresnel(u1)?;
        let (dc, ds) = (tight::sub(c1, c0)?, tight::sub(s1, s0)?);
        let (cp, sp) = (phi.cospi()?, phi.sinpi()?);
        let scale = tight::div(l, root)?;
        let x = tight::mul(scale, tight::add(tight::mul(cp, dc)?, tight::mul(sp, ds)?)?)?;
        let y = tight::mul(scale, tight::sub(tight::mul(cp, ds)?, tight::mul(sp, dc)?)?)?;
        Ok(Some(self.placed_enclosed([x, flip(y)])?))
    }
}
