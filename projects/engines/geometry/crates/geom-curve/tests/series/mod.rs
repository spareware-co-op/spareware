//! The fixed-point truth of piece G63's tests (`attack_g63.rs` and `clothoids_oracle.rs`): a curve
//! whose heading θ(t) is a polynomial in t, piece by piece, plus at most one sine or cosine wave,
//! integrated as ∫e^{iθ} by the Taylor series of e^{iθ} about the left end of short pieces, in
//! 256-bit fixed point. For the clothoid from zero curvature that series is the power series of the
//! Fresnel integrals (DLMF 7.6.4). No quadrature, no panels and no `f64` maths.

use dashu_int::IBig;
use dashu_ratio::RBig;

/// The fixed point: n stands for n / 2^F.
pub const F: usize = 256;

/// Taylor terms per piece of the truth's integral.
pub const TERMS: usize = 48;

/// Pieces per smooth part of [0, t] in the truth's integral.
pub const PIECES: i64 = 32;

pub fn one() -> IBig {
    IBig::ONE << F
}

/// A double, exactly but for a truncation below 2^−256.
pub fn fx(x: f64) -> IBig {
    let q = RBig::try_from(x).unwrap();
    (q.numerator().clone() << F) / IBig::from(q.denominator().clone())
}

/// n / d.
pub fn frac(n: i64, d: i64) -> IBig {
    (IBig::from(n) << F) / IBig::from(d)
}

pub fn mul(a: &IBig, b: &IBig) -> IBig {
    (a * b) >> F
}

pub fn div(a: &IBig, b: &IBig) -> IBig {
    (a << F) / b
}

pub fn abs(a: &IBig) -> IBig {
    if *a < IBig::ZERO {
        -a.clone()
    } else {
        a.clone()
    }
}

/// π to 100 decimal places.
pub fn pi() -> IBig {
    let digits: IBig = "31415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679"
        .parse()
        .unwrap();
    (digits << F) / IBig::from(10).pow(100)
}

/// cos x and sin x by their series, x first reduced by whole turns.
pub fn cos_sin(x: &IBig) -> (IBig, IBig) {
    let turn = pi() << 1;
    let k = div(&(x + pi()), &turn) >> F;
    let x = x - k * &turn;
    let mut term = one();
    let (mut c, mut s) = (IBig::ZERO, IBig::ZERO);
    for n in 0..400usize {
        match n % 4 {
            0 => c += &term,
            1 => s += &term,
            2 => c -= &term,
            _ => s -= &term,
        }
        term = mul(&term, &x) / IBig::from(n + 1);
        if term == IBig::ZERO {
            break;
        }
    }
    (c, s)
}

/// The Taylor coefficients of a polynomial about c: q(c + τ) = Σ dₖ τᵏ (Horner's shift).
pub fn shift(p: &[IBig], c: &IBig) -> Vec<IBig> {
    let mut q = p.to_vec();
    let n = q.len();
    for k in 0..n {
        for j in (k..n - 1).rev() {
            let v = mul(c, &q[j + 1]);
            q[j] += v;
        }
    }
    q
}

/// The integral of a polynomial (coefficients from τ⁰), with constant 0.
pub fn integral(p: &[IBig]) -> Vec<IBig> {
    let mut q = vec![IBig::ZERO];
    for (k, c) in p.iter().enumerate() {
        q.push(c / IBig::from(k + 1));
    }
    q
}

/// A polynomial in ξ = t/L as one in t.
pub fn in_t(p: &[IBig], l: &IBig) -> Vec<IBig> {
    let mut scale = one();
    let mut out = Vec::new();
    for c in p {
        out.push(div(c, &scale));
        scale = mul(&scale, l);
    }
    out
}

/// The value of a polynomial at x.
pub fn value(p: &[IBig], x: &IBig) -> IBig {
    p.iter().rev().fold(IBig::ZERO, |s, c| mul(&s, x) + c)
}

/// A wave of the heading: a·sin(α + βt), or a·cos when `cosine`.
#[derive(Clone, Debug)]
pub struct Wave {
    pub cosine: bool,
    pub a: IBig,
    pub alpha: IBig,
    pub beta: IBig,
}

/// The exact heading θ(t), in radians, from 0 at t = 0, and the placement.
#[derive(Clone, Debug)]
pub struct Truth {
    /// Each smooth part: where it starts in t, and its polynomial in t.
    pub parts: Vec<(IBig, Vec<IBig>)>,
    pub wave: Option<Wave>,
    pub start: [IBig; 2],
    pub unit: [IBig; 2],
}

impl Truth {
    /// θ's Taylor coefficients d₀ … `d_TERMS` about c, from the part holding [c, c + something).
    pub fn taylor(&self, c: &IBig) -> Vec<IBig> {
        let part = self.parts.iter().rev().find(|(from, _)| from <= c).unwrap();
        let mut d = shift(&part.1, c);
        d.resize(TERMS + 1, IBig::ZERO);
        if let Some(w) = &self.wave {
            let phi = &w.alpha + mul(&w.beta, c);
            let (cp, sp) = cos_sin(&phi);
            // dᵏ/dτᵏ sin(φ + βτ) = βᵏ·sin(φ + βτ + kπ/2); cos is sin a quarter on.
            let cycle = [sp.clone(), cp.clone(), -sp, -cp];
            let mut scale = w.a.clone();
            for k in 0..=TERMS {
                d[k] += mul(&scale, &cycle[(k + usize::from(w.cosine)) % 4]);
                scale = mul(&scale, &w.beta) / IBig::from(k + 1);
            }
        }
        d
    }

    /// ∫₀ᵗ e^{iθ}, in the transition's own axes.
    pub fn local(&self, t: &IBig) -> [IBig; 2] {
        let mut ends: Vec<IBig> = vec![IBig::ZERO];
        for (from, _) in &self.parts[1..] {
            if from < t {
                ends.push(from.clone());
            }
        }
        ends.push(t.clone());
        let (mut x, mut y) = (IBig::ZERO, IBig::ZERO);
        for pair in ends.windows(2) {
            let w = (&pair[1] - &pair[0]) / IBig::from(PIECES);
            for i in 0..PIECES {
                let c = &pair[0] + &w * IBig::from(i);
                let w = if i == PIECES - 1 {
                    &pair[1] - &c
                } else {
                    w.clone()
                };
                let d = self.taylor(&c);
                // e^{i(θ(c + τ) − θ(c))} = Σ eₖτᵏ: e₀ = 1, eₖ = (i/k)·Σⱼ j·dⱼ·eₖ₋ⱼ.
                let mut e: Vec<(IBig, IBig)> = vec![(one(), IBig::ZERO)];
                for k in 1..=TERMS {
                    let (mut re, mut im) = (IBig::ZERO, IBig::ZERO);
                    for j in 1..=k {
                        let f = &d[j] * IBig::from(j);
                        re += mul(&f, &e[k - j].0);
                        im += mul(&f, &e[k - j].1);
                    }
                    e.push((-im / IBig::from(k), re / IBig::from(k)));
                }
                let (mut sr, mut si) = (IBig::ZERO, IBig::ZERO);
                let mut wp = w.clone();
                for (k, (er, ei)) in e.iter().enumerate() {
                    sr += mul(er, &wp) / IBig::from(k + 1);
                    si += mul(ei, &wp) / IBig::from(k + 1);
                    wp = mul(&wp, &w);
                }
                let (c0, s0) = cos_sin(&d[0]);
                x += mul(&sr, &c0) - mul(&si, &s0);
                y += mul(&sr, &s0) + mul(&si, &c0);
            }
        }
        [x, y]
    }

    pub fn placed(&self, [x, y]: [IBig; 2]) -> [IBig; 2] {
        let [u, v] = &self.unit;
        [
            &self.start[0] + mul(u, &x) - mul(v, &y),
            &self.start[1] + mul(v, &x) + mul(u, &y),
        ]
    }

    pub fn turned(&self, [x, y]: [IBig; 2]) -> [IBig; 2] {
        let [u, v] = &self.unit;
        [mul(u, &x) - mul(v, &y), mul(v, &x) + mul(u, &y)]
    }

    /// The point and its first three derivatives at t.
    pub fn jet(&self, t: &IBig) -> [[IBig; 2]; 4] {
        let d = self.taylor(t);
        let (th1, th2) = (d[1].clone(), &d[2] * IBig::from(2));
        let (c, s) = cos_sin(&d[0]);
        let normal = [-s.clone(), c.clone()];
        [
            self.placed(self.local(t)),
            self.turned([c.clone(), s.clone()]),
            self.turned([mul(&th1, &normal[0]), mul(&th1, &normal[1])]),
            self.turned([
                mul(&th2, &normal[0]) - mul(&mul(&th1, &th1), &c),
                mul(&th2, &normal[1]) - mul(&mul(&th1, &th1), &s),
            ]),
        ]
    }
}
