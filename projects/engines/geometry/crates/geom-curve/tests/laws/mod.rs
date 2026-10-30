//! The test transitions of pieces G63's and G64's attacks: one or more cases of every law, each
//! with its truth's heading worked from IFC 4.3's curvature formulas (the heading as their
//! integral, worked here from the curvature, never from the kernel's coefficients), for
//! `tests/series`'s fixed-point integral.

use dashu_int::IBig;
use geom_curve::CurvatureLaw;

use crate::series::{
    Truth, Wave, abs, cos_sin, div, frac, fx, in_t, integral, mul, one, pi, shift, value,
};

/// The truth's heading for a law, worked from IFC's curvature formulas.
pub fn truth(law: &CurvatureLaw, length: f64, start: [f64; 2], unit: [(i64, i64); 2]) -> Truth {
    let l = fx(length);
    let zero = IBig::ZERO;
    let by_xi = |kappa_xi: Vec<IBig>| -> Vec<IBig> {
        // κ as a polynomial in ξ, so θ(t) = ∫κ dt = L·∫κ dξ, as a polynomial in t.
        let theta_xi: Vec<IBig> = integral(&kappa_xi).iter().map(|c| mul(c, &l)).collect();
        in_t(&theta_xi, &l)
    };
    let (parts, wave) = match *law {
        CurvatureLaw::Clothoid { start, end } => {
            let (k1, k2) = (fx(start), fx(end));
            (vec![(zero, by_xi(vec![k1.clone(), k2 - k1]))], None)
        }
        CurvatureLaw::Bloss { start, end } => {
            let (k1, k2) = (fx(start), fx(end));
            let dk = k2 - &k1;
            // κ₁ + (3 − 2ξ)ξ²Δκ.
            let k = vec![k1, IBig::ZERO, &dk * IBig::from(3), &dk * IBig::from(-2)];
            (vec![(zero, by_xi(k))], None)
        }
        CurvatureLaw::Helmert { start, end } => {
            let (k1, k2) = (fx(start), fx(end));
            let dk = k2 - &k1;
            // First half κ₁ + 2ξ²Δκ; second κ₁ + (1 − 2(1 − ξ)²)Δκ = κ₁ − Δκ + 4ξΔκ − 2ξ²Δκ.
            let first = by_xi(vec![k1.clone(), IBig::ZERO, &dk * IBig::from(2)]);
            let mut second = by_xi(vec![&k1 - &dk, &dk * IBig::from(4), &dk * IBig::from(-2)]);
            let mid = &l >> 1;
            second[0] = value(&first, &mid) - value(&second, &mid);
            (vec![(zero, first), (mid, second)], None)
        }
        CurvatureLaw::Cosine { start, end } => {
            let (k1, k2) = (fx(start), fx(end));
            let dk = k2 - &k1;
            // κ₁ + ½(1 − cos πξ)Δκ: θ = (κ₁ + Δκ/2)t − (Δκ/2)(L/π)sin(πt/L).
            let half = &dk >> 1;
            let wave = Wave {
                cosine: false,
                a: -div(&mul(&half, &l), &pi()),
                alpha: IBig::ZERO,
                beta: div(&pi(), &l),
            };
            (vec![(zero, vec![IBig::ZERO, &k1 + &half])], Some(wave))
        }
        CurvatureLaw::Sine { start, end } => {
            let (k1, k2) = (fx(start), fx(end));
            let dk = k2 - &k1;
            // κ₁ + (ξ − sin(2πξ)/(2π))Δκ: θ = κ₁t + Δκ·t²/(2L) + Δκ·L/(4π²)·(cos(2πt/L) − 1).
            let a = div(&mul(&dk, &l), &(mul(&pi(), &pi()) * IBig::from(4)));
            let wave = Wave {
                cosine: true,
                a: a.clone(),
                alpha: IBig::ZERO,
                beta: div(&(pi() * IBig::from(2)), &l),
            };
            let p = vec![-a, k1, div(&dk, &(&l * IBig::from(2)))];
            (vec![(zero, p)], Some(wave))
        }
        CurvatureLaw::CantCoupled {
            start,
            end,
            height,
            cant,
        } => {
            let (k1, k2) = (fx(start), fx(end));
            let dk = k2 - &k1;
            let g = div(
                &(mul(&fx(height), &fx(cant)) * IBig::from(420)),
                &mul(&l, &l),
            );
            // κ₁ + Δκξ⁴(35 − 84ξ + 70ξ² − 20ξ³) − 420(hΔψ/L²)ξ²(1 − 4ξ + 5ξ² − 2ξ³).
            let mut k = vec![IBig::ZERO; 8];
            k[0] = k1;
            for (n, c) in [(4, 35), (5, -84), (6, 70), (7, -20)] {
                k[n] += &dk * IBig::from(c);
            }
            for (n, c) in [(2, 1), (3, -4), (4, 5), (5, -2)] {
                k[n] -= &g * IBig::from(c);
            }
            (vec![(zero, by_xi(k))], None)
        }
        CurvatureLaw::Polynomial { terms, from } => {
            // κ(s) = Σ sign(Aₖ)sᵏ/|Aₖ|ᵏ⁺¹; θ(t) = Θ(f + t) − Θ(f).
            let mut k = vec![IBig::ZERO; 8];
            for (n, a) in terms.iter().enumerate() {
                if let Some(a) = a {
                    let mut den = one();
                    for _ in 0..=n {
                        den = mul(&den, &abs(&fx(*a)));
                    }
                    k[n] = div(&one(), &den) * IBig::from(if *a < 0.0 { -1 } else { 1 });
                }
            }
            let mut p = shift(&integral(&k), &fx(from));
            p[0] = IBig::ZERO;
            (vec![(zero, p)], None)
        }
        CurvatureLaw::CosineSpiral {
            constant,
            cosine,
            length: lc,
            from,
        } => {
            // θ(s) = s/A₀ + (Lc/(πA₁))sin(πs/Lc), from s = f.
            let lc = fx(lc);
            let a = div(&lc, &mul(&pi(), &fx(cosine)));
            let alpha = div(&mul(&pi(), &fx(from)), &lc);
            let (_, s0) = cos_sin(&alpha);
            let p = vec![
                -mul(&a, &s0),
                constant.map_or(IBig::ZERO, |a0| div(&one(), &fx(a0))),
            ];
            let wave = Wave {
                cosine: false,
                a,
                alpha,
                beta: div(&pi(), &lc),
            };
            (vec![(zero, p)], Some(wave))
        }
        CurvatureLaw::SineSpiral {
            constant,
            linear,
            sine,
            length: ls,
            from,
        } => {
            // θ(s) = s/A₀ + ½sign(A₁)(s/A₁)² − (Ls/(2πA₂))(cos(2πs/Ls) − 1), from s = f.
            let ls = fx(ls);
            let f = fx(from);
            let a = -div(&ls, &(mul(&pi(), &fx(sine)) * IBig::from(2)));
            let alpha = div(&(mul(&pi(), &f) * IBig::from(2)), &ls);
            let (c0, _) = cos_sin(&alpha);
            let mut p = vec![
                -mul(&a, &c0),
                constant.map_or(IBig::ZERO, |a0| div(&one(), &fx(a0))),
                IBig::ZERO,
            ];
            if let Some(a1) = linear {
                let sq = mul(&fx(a1), &fx(a1));
                let sign = IBig::from(if a1 < 0.0 { -1 } else { 1 });
                p[1] += div(&f, &sq) * &sign;
                p[2] += div(&one(), &(sq * IBig::from(2))) * sign;
            }
            let wave = Wave {
                cosine: true,
                a,
                alpha,
                beta: div(&(pi() * IBig::from(2)), &ls),
            };
            (vec![(zero, p)], Some(wave))
        }
    };
    Truth {
        parts,
        wave,
        start: start.map(fx),
        unit: unit.map(|(n, d)| frac(n, d)),
    }
}

/// A test transition: its name, law, start, direction, the direction's unit vector as exact
/// fractions, and length.
pub struct Case {
    pub name: &'static str,
    pub law: CurvatureLaw,
    pub start: [f64; 2],
    pub direction: [f64; 2],
    pub unit: [(i64, i64); 2],
    pub length: f64,
}

pub fn cases() -> Vec<Case> {
    let none = [None; 8];
    let mut seventh = none;
    seventh[0] = Some(-900.0);
    seventh[1] = Some(140.0);
    seventh[3] = Some(-75.0);
    seventh[7] = Some(52.0);
    let mut second = none;
    second[0] = Some(800.0);
    second[1] = Some(300.0);
    second[2] = Some(-120.0);
    let mut clothoid = none;
    clothoid[1] = Some(250.0);
    vec![
        Case {
            name: "clothoid from a tangent",
            law: CurvatureLaw::Clothoid {
                start: 0.0,
                end: 1.0 / 300.0,
            },
            start: [1000.5, -2000.25],
            direction: [3.0, 4.0],
            unit: [(3, 5), (4, 5)],
            length: 120.0,
        },
        Case {
            name: "clothoid through an inflection",
            law: CurvatureLaw::Clothoid {
                start: 1.0 / 500.0,
                end: -1.0 / 250.0,
            },
            start: [-12.0, 7.5],
            direction: [-5.0, 12.0],
            unit: [(-5, 13), (12, 13)],
            length: 90.0,
        },
        Case {
            name: "clothoid of many turns",
            law: CurvatureLaw::Clothoid {
                start: 0.0,
                end: 2.0,
            },
            start: [0.0, 0.0],
            direction: [1.0, 0.0],
            unit: [(1, 1), (0, 1)],
            length: 30.0,
        },
        Case {
            name: "Helmert",
            law: CurvatureLaw::Helmert {
                start: 0.0,
                end: 1.0 / 400.0,
            },
            start: [250.0, 125.0],
            direction: [8.0, -15.0],
            unit: [(8, 17), (-15, 17)],
            length: 150.0,
        },
        Case {
            name: "Helmert, sharp",
            law: CurvatureLaw::Helmert {
                start: -0.05,
                end: 0.4,
            },
            start: [0.0, 0.0],
            direction: [0.0, -2.0],
            unit: [(0, 1), (-1, 1)],
            length: 25.0,
        },
        Case {
            name: "Bloss",
            law: CurvatureLaw::Bloss {
                start: 1.0 / 1000.0,
                end: 1.0 / 350.0,
            },
            start: [3.0, -4.0],
            direction: [3.0, 4.0],
            unit: [(3, 5), (4, 5)],
            length: 200.0,
        },
        Case {
            name: "cosine",
            law: CurvatureLaw::Cosine {
                start: 0.0,
                end: 1.0 / 600.0,
            },
            start: [10.0, 20.0],
            direction: [1.0, 0.0],
            unit: [(1, 1), (0, 1)],
            length: 100.0,
        },
        Case {
            name: "cosine, sharp",
            law: CurvatureLaw::Cosine {
                start: 0.1,
                end: -0.3,
            },
            start: [0.0, 0.0],
            direction: [-5.0, 12.0],
            unit: [(-5, 13), (12, 13)],
            length: 40.0,
        },
        Case {
            name: "sine",
            law: CurvatureLaw::Sine {
                start: -1.0 / 800.0,
                end: 1.0 / 300.0,
            },
            start: [-500.0, 900.0],
            direction: [8.0, -15.0],
            unit: [(8, 17), (-15, 17)],
            length: 160.0,
        },
        Case {
            name: "sine, sharp",
            law: CurvatureLaw::Sine {
                start: 0.0,
                end: 0.5,
            },
            start: [0.0, 0.0],
            direction: [1.0, 0.0],
            unit: [(1, 1), (0, 1)],
            length: 30.0,
        },
        Case {
            name: "cant-coupled",
            law: CurvatureLaw::CantCoupled {
                start: 0.0,
                end: 1.0 / 1200.0,
                height: 1.8,
                cant: 0.1,
            },
            start: [7.0, 7.0],
            direction: [3.0, 4.0],
            unit: [(3, 5), (4, 5)],
            length: 210.0,
        },
        Case {
            name: "cant-coupled, sharp",
            law: CurvatureLaw::CantCoupled {
                start: 0.02,
                end: 0.3,
                height: 2.5,
                cant: -0.4,
            },
            start: [0.0, 0.0],
            direction: [1.0, 0.0],
            unit: [(1, 1), (0, 1)],
            length: 20.0,
        },
        Case {
            name: "IfcClothoid read from a station",
            law: CurvatureLaw::Polynomial {
                terms: clothoid,
                from: 30.0,
            },
            start: [100.0, 0.0],
            direction: [0.0, -2.0],
            unit: [(0, 1), (-1, 1)],
            length: 80.0,
        },
        Case {
            name: "second-order polynomial spiral",
            law: CurvatureLaw::Polynomial {
                terms: second,
                from: 0.0,
            },
            start: [0.0, 0.0],
            direction: [3.0, 4.0],
            unit: [(3, 5), (4, 5)],
            length: 50.0,
        },
        Case {
            name: "seventh-order polynomial spiral",
            law: CurvatureLaw::Polynomial {
                terms: seventh,
                from: 10.0,
            },
            start: [-3.0, 2.0],
            direction: [-5.0, 12.0],
            unit: [(-5, 13), (12, 13)],
            length: 60.0,
        },
        Case {
            name: "IfcCosineSpiral",
            law: CurvatureLaw::CosineSpiral {
                constant: Some(1500.0),
                cosine: 90.0,
                length: 120.0,
                from: 15.0,
            },
            start: [40.0, -40.0],
            direction: [8.0, -15.0],
            unit: [(8, 17), (-15, 17)],
            length: 80.0,
        },
        Case {
            name: "IfcSineSpiral",
            law: CurvatureLaw::SineSpiral {
                constant: Some(-2000.0),
                linear: Some(35.0),
                sine: 70.0,
                length: 150.0,
                from: 20.0,
            },
            start: [0.5, 0.25],
            direction: [1.0, 0.0],
            unit: [(1, 1), (0, 1)],
            length: 100.0,
        },
        Case {
            name: "IfcSineSpiral, no optional terms",
            law: CurvatureLaw::SineSpiral {
                constant: None,
                linear: None,
                sine: -40.0,
                length: 60.0,
                from: -5.0,
            },
            start: [0.0, 0.0],
            direction: [0.0, -2.0],
            unit: [(0, 1), (-1, 1)],
            length: 45.0,
        },
    ]
}
