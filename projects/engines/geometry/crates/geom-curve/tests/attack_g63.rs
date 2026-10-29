//! Attacks on piece G63's claims: every law's points, derivatives, turn and curvature against an
//! independent truth in 256-bit fixed point. The truth integrates each law's curvature as IFC 4.3
//! prints it (the heading as its integral, worked here from the curvature, never from the kernel's
//! coefficients), then the position as ∫e^{iθ} by the Taylor series of e^{iθ} about the left end
//! of short pieces: for the clothoid from zero curvature that is the power series of the Fresnel
//! integrals (DLMF 7.6.4), in arbitrary precision as chapter 28 asks. No quadrature, no panels and
//! no `f64` maths on the truth's side. A call may refuse an attack; it may never return an
//! enclosure that misses the truth.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod series;

use dashu_int::IBig;
use geom_curve::{CurvatureLaw, CurveError, Transition};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use series::{
    Truth, Wave, abs, cos_sin, div, frac, fx, in_t, integral, mul, one, pi, shift, value,
};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The truth's heading for a law, worked from IFC's curvature formulas.
fn truth(law: &CurvatureLaw, length: f64, start: [f64; 2], unit: [(i64, i64); 2]) -> Truth {
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
struct Case {
    name: &'static str,
    law: CurvatureLaw,
    start: [f64; 2],
    direction: [f64; 2],
    unit: [(i64, i64); 2],
    length: f64,
}

fn cases() -> Vec<Case> {
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

fn build(c: &Case) -> Transition {
    Transition::new(c.start, c.direction, c.length, c.law, &mut meter())
        .unwrap_or_else(|e| panic!("{}: {e}", c.name))
}

/// The truth slack: the truth's own truncation, far below a double's step.
fn slack() -> IBig {
    one() >> 200
}

fn holds(i: Interval, v: &IBig) -> bool {
    fx(i.lo()) - slack() <= *v && *v <= fx(i.hi()) + slack()
}

/// The stations each case is checked at: the ends, inside, and next to the panel ends.
fn stations(c: &Case, t: &Transition) -> Vec<f64> {
    let l = c.length;
    let mut out = vec![0.0, l / 7.0, l / 2.0, 0.8 * l, l * 0.999, l];
    let n = t.panels();
    for p in 1..n.min(4) {
        let b = l * (p as f64 / n as f64);
        out.extend([b.next_down(), b, b.next_up()]);
    }
    out.extend([(l / 2.0).next_down(), (l / 2.0).next_up()]);
    out
}

#[test]
fn every_law_encloses_the_truth() {
    for c in cases() {
        let t = build(&c);
        let truth = truth(&c.law, c.length, c.start, c.unit);
        let scale = 1.0 + c.start[0].abs() + c.start[1].abs() + c.length;
        for s in stations(&c, &t) {
            let jet = truth.jet(&fx(s));
            let e = t.enclose(s, 3, &mut meter()).unwrap();
            assert_eq!(e.len(), 4);
            for (k, (ek, tk)) in e.iter().zip(&jet).enumerate() {
                for d in 0..2 {
                    assert!(
                        holds(ek[d], &tk[d]),
                        "{} at {s}: derivative {k}, axis {d}: {:?} misses {}",
                        c.name,
                        ek[d],
                        tk[d].to_f64().value()
                    );
                    let width = ek[d].hi() - ek[d].lo();
                    let allowed = if k == 0 { scale * 2e-12 } else { 1e-12 };
                    assert!(
                        width <= allowed,
                        "{} at {s}: derivative {k} width {width}",
                        c.name
                    );
                }
            }
            // The fast path agrees with the truth to a few hundred steps.
            let p = t.point(s).unwrap();
            for d in 0..2 {
                let err = abs(&(fx(p[d]) - &jet[0][d])).to_f64().value() / one().to_f64().value();
                assert!(
                    err <= scale * 1e-13,
                    "{} at {s}: point axis {d} off by {err}",
                    c.name
                );
            }
            let mut out = [[0.0; 2]; 4];
            t.derivatives(s, &mut out).unwrap();
            assert_eq!(out[0], p);
            for k in 1..4 {
                for d in 0..2 {
                    let err = abs(&(fx(out[k][d]) - &jet[k][d])).to_f64().value()
                        / one().to_f64().value();
                    assert!(
                        err <= 1e-12,
                        "{} at {s}: derivative {k} axis {d} off by {err}",
                        c.name
                    );
                }
            }
            // The turn and the curvature, from the closed form.
            let d = truth.taylor(&fx(s));
            let turn = div(&d[0], &pi());
            assert!(holds(t.turn(s).unwrap(), &turn), "{} at {s}: turn", c.name);
            assert!(
                holds(t.curvature(s).unwrap(), &d[1]),
                "{} at {s}: curvature",
                c.name
            );
            let w = t.turn(s).unwrap();
            assert!(w.hi() - w.lo() <= 1e-12, "{} at {s}: turn width", c.name);
        }
    }
}

#[test]
fn the_remainders_meet_the_tolerance_on_powers_of_two() {
    for c in cases() {
        let t = build(&c);
        let n = t.panels();
        assert!(
            n.is_power_of_two() && n <= geom_curve::transition::MAX_PANELS,
            "{}",
            c.name
        );
        if matches!(c.law, CurvatureLaw::Helmert { .. }) {
            assert!(n >= 2, "{}: Helmert's halves need their own panels", c.name);
        }
        assert!(t.remainder() > 0.0, "{}", c.name);
        assert!(
            t.remainder() <= c.length / 72_057_594_037_927_936.0,
            "{}: {}",
            c.name,
            t.remainder()
        );
        assert_eq!(
            t,
            build(&c),
            "{}: the same doubles build the same transition",
            c.name
        );
        assert_eq!(t.domain(), (0.0, c.length));
        assert_eq!(t.length(), c.length);
        assert_eq!(t.start(), c.start);
        assert_eq!(t.direction(), c.direction);
        assert_eq!(t.law(), c.law);
    }
}

#[test]
fn a_gentle_road_spiral_needs_one_panel_and_a_winding_one_more() {
    let gentle = build(&cases()[0]);
    assert_eq!(gentle.panels(), 1);
    let winding = build(&cases()[2]);
    assert!(winding.panels() >= 8, "{}", winding.panels());
}

#[test]
fn a_straight_law_is_a_line() {
    // A zero law is a straight line: each enclosure holds the exact point, to a step of 2⁻¹⁰⁷⁴.
    let t = Transition::new(
        [1.0, 2.0],
        [3.0, 4.0],
        10.0,
        CurvatureLaw::Clothoid {
            start: 0.0,
            end: 0.0,
        },
        &mut meter(),
    )
    .unwrap();
    let e = t.enclose(10.0, 1, &mut meter()).unwrap();
    assert!(e[0][0].contains(7.0) && e[0][1].contains(10.0), "{e:?}");
    assert!(e[1][0].contains(0.6) && e[1][1].contains(0.8), "{e:?}");
    let p = t.point(10.0).unwrap();
    assert!(
        (p[0] - 7.0).abs() < 1e-13 && (p[1] - 10.0).abs() < 1e-13,
        "{p:?}"
    );
    let turn = t.turn(5.0).unwrap();
    assert!(
        turn.contains(0.0) && turn.hi() - turn.lo() < 1e-300,
        "{turn:?}"
    );
    assert_eq!(t.point(0.0).unwrap(), [1.0, 2.0]);
}

#[test]
fn the_derivatives_follow_the_order_asked() {
    let c = &cases()[3];
    let t = build(c);
    assert_eq!(t.enclose(10.0, 0, &mut meter()).unwrap().len(), 1);
    let e = t.enclose(10.0, 6, &mut meter()).unwrap();
    assert_eq!(e.len(), 7);
    let mut out = [[0.0; 2]; 7];
    t.derivatives(10.0, &mut out).unwrap();
    for (k, (ek, ok)) in e.iter().zip(&out).enumerate() {
        for d in 0..2 {
            let w = ek[d].hi() - ek[d].lo() + 1e-300;
            assert!(
                ek[d].lo() - 1e3 * w <= ok[d] && ok[d] <= ek[d].hi() + 1e3 * w,
                "derivative {k}: {:?} and {}",
                ek[d],
                ok[d]
            );
        }
    }
    // Nothing asked, nothing written.
    t.derivatives(10.0, &mut []).unwrap();
    // The tangent is a unit vector.
    let tangent = e[1];
    let norm = tangent[0]
        .square()
        .unwrap()
        .add(tangent[1].square().unwrap())
        .unwrap();
    assert!(norm.contains(1.0), "{norm:?}");
}

#[test]
fn bad_data_is_refused() {
    let m = &mut meter();
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.01,
    };
    let new = |start, direction, length, law, m: &mut WorkMeter<'static>| {
        Transition::new(start, direction, length, law, m).map(|_| ())
    };
    assert_eq!(
        new([0.0, 0.0], [0.0, 0.0], 1.0, law, m),
        Err(CurveError::Degenerate)
    );
    assert_eq!(
        new([0.0, 0.0], [1.0, 0.0], 0.0, law, m),
        Err(CurveError::Degenerate)
    );
    assert_eq!(
        new([0.0, 0.0], [1.0, 0.0], -1.0, law, m),
        Err(CurveError::Degenerate)
    );
    assert_eq!(
        new([f64::NAN, 0.0], [1.0, 0.0], 1.0, law, m),
        Err(CurveError::NotFinite)
    );
    assert_eq!(
        new([0.0, 0.0], [1.0, f64::INFINITY], 1.0, law, m),
        Err(CurveError::NotFinite)
    );
    assert_eq!(
        new([0.0, 0.0], [1.0, 0.0], f64::INFINITY, law, m),
        Err(CurveError::NotFinite)
    );
    let nan = CurvatureLaw::Bloss {
        start: f64::NAN,
        end: 0.0,
    };
    assert_eq!(
        new([0.0, 0.0], [1.0, 0.0], 1.0, nan, m),
        Err(CurveError::NotFinite)
    );
    let mut terms = [None; 8];
    terms[2] = Some(0.0);
    let zero = CurvatureLaw::Polynomial { terms, from: 0.0 };
    assert_eq!(
        new([0.0, 0.0], [1.0, 0.0], 1.0, zero, m),
        Err(CurveError::Degenerate)
    );
    terms[2] = Some(f64::INFINITY);
    let inf = CurvatureLaw::Polynomial { terms, from: 0.0 };
    assert_eq!(
        new([0.0, 0.0], [1.0, 0.0], 1.0, inf, m),
        Err(CurveError::NotFinite)
    );
    for length in [0.0, -1.0] {
        let cosine = CurvatureLaw::CosineSpiral {
            constant: None,
            cosine: 10.0,
            length,
            from: 0.0,
        };
        assert_eq!(
            new([0.0, 0.0], [1.0, 0.0], 1.0, cosine, m),
            Err(CurveError::Degenerate)
        );
    }
    let sine = CurvatureLaw::SineSpiral {
        constant: Some(0.0),
        linear: None,
        sine: 10.0,
        length: 5.0,
        from: 0.0,
    };
    assert_eq!(
        new([0.0, 0.0], [1.0, 0.0], 1.0, sine, m),
        Err(CurveError::Degenerate)
    );
    let sine = CurvatureLaw::SineSpiral {
        constant: None,
        linear: None,
        sine: 10.0,
        length: f64::NAN,
        from: 0.0,
    };
    assert_eq!(
        new([0.0, 0.0], [1.0, 0.0], 1.0, sine, m),
        Err(CurveError::NotFinite)
    );
    // Curvature turning thousands of half-turns per panel even at the cap: refused.
    let wild = CurvatureLaw::Clothoid {
        start: 1e9,
        end: 1e9,
    };
    assert_eq!(
        new([0.0, 0.0], [1.0, 0.0], 1.0, wild, m),
        Err(CurveError::TooManyPanels)
    );
}

#[test]
fn stations_outside_are_refused() {
    let t = build(&cases()[0]);
    for s in [-1e-300, 120.0f64.next_up(), f64::NAN, f64::INFINITY] {
        assert_eq!(t.point(s), Err(CurveError::ParameterOutside));
        assert_eq!(
            t.enclose(s, 1, &mut meter()).map(|_| ()),
            Err(CurveError::ParameterOutside)
        );
        assert_eq!(t.turn(s).map(|_| ()), Err(CurveError::ParameterOutside));
        assert_eq!(
            t.curvature(s).map(|_| ()),
            Err(CurveError::ParameterOutside)
        );
        assert_eq!(
            t.derivatives(s, &mut [[0.0; 2]; 2]),
            Err(CurveError::ParameterOutside)
        );
    }
}

#[test]
fn the_meter_stops_the_work() {
    let budget = Budget::of_work(0);
    let mut m = WorkMeter::new(budget);
    let c = &cases()[0];
    assert!(matches!(
        Transition::new(c.start, c.direction, c.length, c.law, &mut m),
        Err(CurveError::Stopped(_))
    ));
    let t = build(c);
    let mut m = WorkMeter::new(budget);
    assert!(matches!(
        t.enclose(1.0, 0, &mut m),
        Err(CurveError::Stopped(_))
    ));
}

#[test]
fn the_panel_counts_are_pinned() {
    // The count depends only on the stored doubles (17.8): any change to the bound or the search
    // shows here.
    let got: Vec<(&str, usize, u64)> = cases()
        .iter()
        .map(|c| {
            let t = build(c);
            (c.name, t.panels(), t.remainder().to_bits())
        })
        .collect();
    let pinned: [(usize, u64); 18] = [
        (1, 0x3c84_f571_8a12_f792),
        (1, 0x3cb1_07ff_3eec_0976),
        (32, 0x3c11_4b26_c612_0723),
        (4, 0x3c25_6c4d_a7cf_f659),
        (16, 0x3c79_d0c3_aaa7_6e56),
        (4, 0x3c75_20cd_0c87_7edf),
        (2, 0x3c7f_9597_7470_2b81),
        (8, 0x3c79_dff5_a22d_2e05),
        (4, 0x3c6d_9409_3ea7_193c),
        (16, 0x3bd8_77a7_7444_db8f),
        (16, 0x3c48_c11f_cdf0_0790),
        (64, 0x3ca5_a3d7_624c_976f),
        (1, 0x3b5d_f814_fea6_0473),
        (1, 0x3c73_0b8f_4cdc_a33d),
        (16, 0x3c66_de3f_4f5c_532f),
        (4, 0x3bf9_7bda_35aa_440a),
        (8, 0x3c77_e300_a11c_55ab),
        (8, 0x3bd9_6649_8568_fa94),
    ];
    for ((name, panels, remainder), (p, r)) in got.iter().zip(pinned) {
        assert_eq!((*panels, *remainder), (p, r), "{name}");
    }
    assert_eq!(got.len(), pinned.len());
}
