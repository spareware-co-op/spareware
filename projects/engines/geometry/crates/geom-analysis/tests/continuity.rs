//! Piece G346's acceptance: certified G0 to G3 maxima per edge with a witness parameter, on
//! constructed joins whose maxima are known in closed form, and on a body's edges.
//!
//! - **Constructed G1, G2 and G3 joins measured.** A flat strip z = 0 (x ≤ 0) meets, along
//!   x = 0, exact Bezier graphs (x ≥ 0) whose jump varies along the edge y = t: z = x²·h(y) is a
//!   G1 join with a curvature jump of 2·h(t), z = x³·h(y) a G2 join with κ′ jumping by 6·h(t),
//!   z = x⁴ a G3 join, z = x·y a G0 join whose normals part by atan(t), and z = h(y) − h(0)
//!   lifted off the strip by a gap that peaks mid-edge; h(y) = 1 + y − y², largest (5/4) at
//!   y = 1/2. Each maximum's two ends hold the closed form, are within the tightness asked, and
//!   its witness is where the closed form peaks.
//! - **Per edge.** On a box whose fins carry pcurves, every edge is a crease of a quarter turn
//!   with no gap and no curvature jump; an edge with a fin that has no pcurve, or with one fin,
//!   is refused.
//! - **The maximum holds every value.** Over random graph joins (a printed seed; `SPAREKIT_SEED`
//!   replays one), the value at random parameters is never above the upper end, and the
//!   witness's lower end never above it either.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests: oracles and generators, and a failure stops the test"
)]

mod common;

use core::f64::consts::PI;
use core::num::NonZeroU64;

use common::{along, graph, meter, strip, unit_box};
use geom_analysis::continuity::{self, Ask, ContinuityError, Maximum, Upper};
use geom_surface::class_a::{self, Limit, Measure, Side};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use geom_topo::Ix;

/// The join of `a` and `b` along u = 0, v = t.
fn measure(
    a: &geom_surface::SurfaceKind,
    b: &geom_surface::SurfaceKind,
    m: Measure,
    tight: f64,
) -> Maximum {
    let p = along(0.0);
    let s = [
        Side {
            surface: a,
            pcurve: &p,
            turned: false,
        },
        Side {
            surface: b,
            pcurve: &p,
            turned: false,
        },
    ];
    continuity::join(s, Ask::new(m, tight).unwrap(), &mut meter()).unwrap()
}

fn upper(m: &Maximum) -> f64 {
    match m.upper {
        Upper::Bounded(u) => u,
        Upper::Unresolved { from, to } => panic!("unresolved over [{from}, {to}]"),
    }
}

/// The maximum's ends hold `want`, are within `tight`, and the witness is within `near` of `at`.
fn check(m: &Maximum, want: f64, tight: f64, at: f64, near: f64) {
    let u = upper(m);
    assert!(m.lower() <= want && want <= u, "{m:?}, want {want}");
    assert!(m.as_asked && u - m.lower() <= tight, "{m:?}");
    let w = m.witness.unwrap();
    assert!(
        (w.at - at).abs() <= near,
        "{m:?}: witness at {}, want {at}",
        w.at
    );
}

/// h(y) = 1 + y − y² in Bernstein form of degree 2: 1, 3/2, 1.
const H: [f64; 3] = [1.0, 1.5, 1.0];

#[test]
fn a_g1_join_s_curvature_jump_peaks_mid_edge() {
    // z = x²·h(y): Bezier coefficients x² → (0, 0, 1) in degree 2.
    let b = graph(&[0.0, 0.0, 1.0], &H);
    let m = measure(&strip(0.0), &b, Measure::Curvature, 1e-6);
    check(&m, 2.5, 1e-6, 0.5, 1e-2);
    for g in [Measure::Gap, Measure::NormalAngle] {
        let m = measure(&strip(0.0), &b, g, 1e-9);
        assert!(
            m.lower() == 0.0 && upper(&m) <= 1e-9 && m.as_asked,
            "{g:?}: {m:?}"
        );
    }
}

#[test]
fn a_g2_join_s_curvature_change_peaks_mid_edge() {
    let b = graph(&[0.0, 0.0, 0.0, 1.0], &H);
    check(
        &measure(&strip(0.0), &b, Measure::CurvatureChange, 1e-6),
        7.5,
        1e-6,
        0.5,
        1e-2,
    );
    let m = measure(&strip(0.0), &b, Measure::Curvature, 1e-8);
    assert!(upper(&m) <= 1e-8 && m.as_asked, "{m:?}");
}

#[test]
fn a_g3_join_measures_nothing_up_to_g3() {
    let b = graph(&[0.0, 0.0, 0.0, 0.0, 1.0], &[1.0, 1.0]);
    for m in [
        Measure::Gap,
        Measure::NormalAngle,
        Measure::Curvature,
        Measure::CurvatureChange,
    ] {
        let x = measure(&strip(0.0), &b, m, 1e-7);
        assert!(upper(&x) <= 1e-7 && x.as_asked, "{m:?}: {x:?}");
    }
}

#[test]
fn a_crease_opening_along_the_edge_peaks_at_its_end() {
    // z = x·y: the slope across the edge is t, so the normals part by atan(t), largest at t = 1.
    let b = graph(&[0.0, 1.0], &[0.0, 1.0]);
    check(
        &measure(&strip(0.0), &b, Measure::NormalAngle, 1e-9),
        PI / 4.0,
        1e-9,
        1.0,
        0.0,
    );
}

#[test]
fn a_gap_that_bulges_mid_edge_peaks_there() {
    // z = h(y) − 1 = y − y², a quarter at y = 1/2: Bernstein (0, 1/2, 0), the same for every x.
    let b = graph(&[1.0], &[0.0, 0.5, 0.0]);
    check(
        &measure(&strip(0.0), &b, Measure::Gap, 1e-9),
        0.25,
        1e-9,
        0.5,
        1e-3,
    );
}

#[test]
fn every_edge_of_a_box_is_a_quarter_turn_crease() {
    let body = unit_box(true);
    for e in 0..12u32 {
        for (m, want) in [
            (Measure::Gap, 0.0),
            (Measure::NormalAngle, PI / 2.0),
            (Measure::Curvature, 0.0),
            (Measure::CurvatureChange, 0.0),
        ] {
            let x = continuity::edge(
                &body,
                Ix::new(e).unwrap(),
                Ask::new(m, 1e-9).unwrap(),
                &mut meter(),
            )
            .unwrap();
            assert!(
                x.lower() <= want && want <= upper(&x) && x.as_asked,
                "edge {e} {m:?}: {x:?}"
            );
        }
    }
}

#[test]
fn an_edge_without_pcurves_or_without_a_second_fin_is_refused() {
    let body = unit_box(false);
    let ask = Ask::new(Measure::Gap, 1e-9).unwrap();
    assert!(matches!(
        continuity::edge(&body, Ix::new(0).unwrap(), ask, &mut meter()),
        Err(ContinuityError::NoPcurve(_))
    ));
    assert_eq!(
        continuity::edge(&body, Ix::new(99).unwrap(), ask, &mut meter()),
        Err(ContinuityError::NoSuchRow)
    );
    let sheet = common::lone_fin();
    assert_eq!(
        continuity::edge(&sheet, Ix::new(0).unwrap(), ask, &mut meter()),
        Err(ContinuityError::NotAJoin(Ix::new(0).unwrap()))
    );
}

fn unit(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

#[test]
fn the_maximum_holds_every_value_along_the_join() {
    let seed = resolve("G346 maxima hold every value", Seed(0x0346_A1A5)).unwrap();
    let mut rng = SplitMix64::new(seed);
    let rounds = if cfg!(debug_assertions) { 4 } else { 40 };
    let all = [
        Measure::Gap,
        Measure::NormalAngle,
        Measure::Curvature,
        Measure::CurvatureChange,
    ];
    for _ in 0..rounds {
        let du = 1 + rng.below(NonZeroU64::new(4).unwrap()) as usize;
        let dv = 1 + rng.below(NonZeroU64::new(3).unwrap()) as usize;
        let cu: Vec<f64> = (0..=du).map(|_| unit(&mut rng) * 2.0 - 1.0).collect();
        let cv: Vec<f64> = (0..=dv).map(|_| unit(&mut rng) + 0.5).collect();
        let (a, b) = (strip(unit(&mut rng) * 0.2), graph(&cu, &cv));
        let p = along(0.0);
        let s = [
            Side {
                surface: &a,
                pcurve: &p,
                turned: false,
            },
            Side {
                surface: &b,
                pcurve: &p,
                turned: false,
            },
        ];
        for m in all {
            let x = continuity::join(s, Ask::new(m, 1e-6).unwrap(), &mut meter()).unwrap();
            let u = upper(&x);
            assert!(x.lower() <= u, "{m:?}: {x:?}");
            for _ in 0..20 {
                let t = unit(&mut rng);
                let v = class_a::at(m, s, t, Limit::FromRight, &mut meter())
                    .unwrap()
                    .value()
                    .unwrap();
                assert!(v.lo() <= u, "{m:?} {cu:?} {cv:?} at {t}: {v:?} above {x:?}");
            }
        }
    }
}
