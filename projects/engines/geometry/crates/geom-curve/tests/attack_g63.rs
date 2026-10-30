//! Attacks on piece G63's claims: every law's points, derivatives, turn and curvature against an
//! independent truth in 256-bit fixed point. The truth integrates each law's curvature as IFC 4.3
//! prints it (the heading as its integral, worked in `tests/laws` from the curvature, never from
//! the kernel's coefficients), then the position as ∫e^{iθ} by the Taylor series of e^{iθ} about the left end
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

mod laws;
mod series;

use dashu_int::IBig;
use geom_curve::{CurvatureLaw, CurveError, ParamCurve, Transition};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};
use laws::{Case, cases, truth};
use series::{abs, div, fx, one, pi};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn build(c: &Case) -> Transition<2> {
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
fn stations(c: &Case, t: &Transition<2>) -> Vec<f64> {
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
            let e = t.enclose(s, Limit::FromRight, 3, &mut meter()).unwrap();
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
            let p = t.point(s, Limit::FromRight).unwrap();
            for d in 0..2 {
                let err = abs(&(fx(p[d]) - &jet[0][d])).to_f64().value() / one().to_f64().value();
                assert!(
                    err <= scale * 1e-13,
                    "{} at {s}: point axis {d} off by {err}",
                    c.name
                );
            }
            let mut out = [[0.0; 2]; 4];
            t.derivatives(s, Limit::FromRight, &mut out).unwrap();
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
    let e = t.enclose(10.0, Limit::FromRight, 1, &mut meter()).unwrap();
    assert!(e[0][0].contains(7.0) && e[0][1].contains(10.0), "{e:?}");
    assert!(e[1][0].contains(0.6) && e[1][1].contains(0.8), "{e:?}");
    let p = t.point(10.0, Limit::FromRight).unwrap();
    assert!(
        (p[0] - 7.0).abs() < 1e-13 && (p[1] - 10.0).abs() < 1e-13,
        "{p:?}"
    );
    let turn = t.turn(5.0).unwrap();
    assert!(
        turn.contains(0.0) && turn.hi() - turn.lo() < 1e-300,
        "{turn:?}"
    );
    assert_eq!(t.point(0.0, Limit::FromRight).unwrap(), [1.0, 2.0]);
}

#[test]
fn the_derivatives_follow_the_order_asked() {
    let c = &cases()[3];
    let t = build(c);
    assert_eq!(
        t.enclose(10.0, Limit::FromRight, 0, &mut meter())
            .unwrap()
            .len(),
        1
    );
    let e = t.enclose(10.0, Limit::FromRight, 6, &mut meter()).unwrap();
    assert_eq!(e.len(), 7);
    let mut out = [[0.0; 2]; 7];
    t.derivatives(10.0, Limit::FromRight, &mut out).unwrap();
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
    t.derivatives(10.0, Limit::FromRight, &mut []).unwrap();
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
    assert_eq!(
        CurveError::TooManyPanels.to_string(),
        "the transition's heading turns too far for its certified quadrature's panels"
    );
}

#[test]
fn a_point_past_the_doubles_is_refused() {
    // Built, but its end lies past the largest double.
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.0,
    };
    let t = Transition::new([f64::MAX, 0.0], [1.0, 0.0], 1e300, law, &mut meter()).unwrap();
    assert_eq!(t.point(1e300, Limit::FromRight), Err(CurveError::NotFinite));
    assert_eq!(
        t.derivatives(1e300, Limit::FromRight, &mut [[0.0; 2]; 2]),
        Err(CurveError::NotFinite)
    );
    assert_eq!(
        t.enclose(1e300, Limit::FromRight, 0, &mut meter())
            .map(|_| ()),
        Err(CurveError::NotFinite)
    );
    assert_eq!(t.point(0.0, Limit::FromRight), Ok([f64::MAX, 0.0]));
}

#[test]
fn a_station_just_before_a_panel_start_stays_in_the_panel_before() {
    // L = 1 + 2⁻⁵², so L·¾ = ¾ + 1.5·2⁻⁵³ is not a double. The station ¾ + 2⁻⁵³ lies before it and
    // ¾ + 2⁻⁵² past it, yet both quotients by L round to ¾, a panel start: only an exact
    // comparison tells them apart.
    let length = 1.0 + f64::EPSILON;
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 60.0,
    };
    let t = Transition::new([0.0, 0.0], [1.0, 0.0], length, law, &mut meter()).unwrap();
    let n = t.panels();
    assert!(n >= 4, "{n}");
    let (before, after) = (0.75 + f64::EPSILON / 2.0, 0.75 + f64::EPSILON);
    assert_eq!((before / length, after / length), (0.75, 0.75));
    assert_eq!(t.panel_at(before), Ok(3 * n / 4 - 1));
    assert_eq!(t.panel_at(after), Ok(3 * n / 4));
    assert_eq!(t.panel_at(0.0), Ok(0));
    assert_eq!(t.panel_at(length), Ok(n - 1));
    assert_eq!(t.panel_at(-1.0), Err(CurveError::ParameterOutside));
    let truth = truth(&law, length, [0.0, 0.0], [(1, 1), (0, 1)]);
    for s in [before, after] {
        let e = t.enclose(s, Limit::FromRight, 1, &mut meter()).unwrap();
        let jet = truth.jet(&fx(s));
        for k in 0..2 {
            for d in 0..2 {
                assert!(
                    holds(e[k][d], &jet[k][d]),
                    "at {s}: derivative {k}, axis {d}"
                );
            }
        }
    }
}

#[test]
fn the_cap_is_the_last_count_built() {
    // Curvature 10⁴ over a unit length needs exactly the cap, 2¹² panels; twice that needs more.
    let constant = |k: f64| {
        Transition::new(
            [0.0, 0.0],
            [1.0, 0.0],
            1.0,
            CurvatureLaw::Clothoid { start: k, end: k },
            &mut meter(),
        )
    };
    assert_eq!(
        constant(1e4).unwrap().panels(),
        geom_curve::transition::MAX_PANELS
    );
    assert_eq!(constant(2e4).map(|_| ()), Err(CurveError::TooManyPanels));
}

#[test]
fn stations_outside_are_refused() {
    let t = build(&cases()[0]);
    for s in [-1e-300, 120.0f64.next_up(), f64::NAN, f64::INFINITY] {
        assert_eq!(
            t.point(s, Limit::FromRight),
            Err(CurveError::ParameterOutside)
        );
        assert_eq!(
            t.enclose(s, Limit::FromRight, 1, &mut meter()).map(|_| ()),
            Err(CurveError::ParameterOutside)
        );
        assert_eq!(t.turn(s).map(|_| ()), Err(CurveError::ParameterOutside));
        assert_eq!(
            t.curvature(s).map(|_| ()),
            Err(CurveError::ParameterOutside)
        );
        assert_eq!(
            t.derivatives(s, Limit::FromRight, &mut [[0.0; 2]; 2]),
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
        t.enclose(1.0, Limit::FromRight, 0, &mut m),
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
