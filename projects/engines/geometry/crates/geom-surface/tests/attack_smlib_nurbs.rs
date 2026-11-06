// SPDX-License-Identifier: Apache-2.0
//! Attack tests from SMLib's confirmed defects, batch 6: `geom-surface` on B-spline nets.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: a defined refusal or a finite, right answer, and never a
//! panic, a hang, a NaN or a wrong value.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    reason = "tests: native arithmetic and indexing, prose that names defect files, and a failure stops the test"
)]

use geom_nurbs::{Basis, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{ParamSurface, SurfaceKind, partial_index};

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 38))
}

/// The enclosure of the surface's point at `(u, v)`, as its midpoint, or the refusal's text.
fn point_at(k: &SurfaceKind, u: f64, v: f64) -> Result<[f64; 3], String> {
    let all = k
        .partials_over(((u, u), (v, v)), SIDES, 0, &mut meter())
        .map_err(|e| format!("{e:?}"))?;
    let e = all[partial_index(0, 0).unwrap()];
    Ok([
        e[0].lo() * 0.5 + e[0].hi() * 0.5,
        e[1].lo() * 0.5 + e[1].hi() * 0.5,
        e[2].lo() * 0.5 + e[2].hi() * 0.5,
    ])
}

/// A net of `nu` by `nv` points on the plane z = u + v scaled by `scale`, row by row.
fn net(nu: usize, nv: usize, scale: f64) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            out.push([i as f64 * scale, j as f64 * scale, (i + j) as f64 * scale]);
        }
    }
    out
}

/// solidcore plan/upstream-issues/UB-166--sm-decomposesrf-writes-past-the-surface-cache-s-patch-array.md
/// (b4de2149): a v knot vector whose interior knot sits within the C++ code's scaled zero of the
/// start knot (`[0, 0, 0, 1e-12, 1, 1, 1]`, and the pairs of knots 5e-9 and 1.2e-8 apart) was
/// snapped by the constructor to a start multiplicity above the degree plus one, and the
/// decomposition into Bezier patches then wrote past its patch array. Ours either refuses such a
/// vector or evaluates the surface it describes: the net is the plane z = x + y over a unit
/// square of control points, so every point of the surface lies on that plane, inside the net's
/// box, with no panic and no non-finite number.
#[test]
fn ub_166_knots_a_hair_from_the_start_knot_are_refused_or_evaluate_on_the_plane() {
    let u = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let vs: [Vec<f64>; 4] = [
        vec![0.0, 0.0, 0.0, 1e-12, 1.0, 1.0, 1.0],
        vec![0.0, 0.0, 0.0, 6.2e-31, 6.8e-31, 1.0, 1.0, 1.0],
        vec![
            0.0, 0.0, 0.0, 5e-9, 5e-9, 1.212e-8, 1.212e-8, 1.212e-8, 1.0, 1.0, 1.0,
        ],
        vec![0.0, 0.0, 0.0, 5e-9, 5e-9, 1.0, 1.0, 1.0],
    ];
    for v in vs {
        let bu = Basis::new(2, u.clone()).unwrap();
        let Ok(bv) = Basis::new(2, v.clone()) else {
            continue;
        };
        let (nu, nv) = (bu.count(), bv.count());
        let Ok(surface) = Surface::<3>::new(bu, bv, &net(nu, nv, 1.0)) else {
            continue;
        };
        let k = SurfaceKind::Nurbs(surface);
        let (u_hi, v_hi) = ((nu - 1) as f64, (nv - 1) as f64);
        for a in 0..=10 {
            for b in 0..=10 {
                let (pu, pv) = (f64::from(a) / 10.0, f64::from(b) / 10.0);
                match point_at(&k, pu, pv) {
                    Ok(p) => {
                        assert!(p.iter().all(|x| x.is_finite()), "knots {v:?}: {p:?}");
                        assert!(
                            p[0] >= -1e-9 && p[0] <= u_hi + 1e-9,
                            "knots {v:?}: x {p:?}"
                        );
                        assert!(
                            p[1] >= -1e-9 && p[1] <= v_hi + 1e-9,
                            "knots {v:?}: y {p:?}"
                        );
                        assert!(
                            (p[2] - (p[0] + p[1])).abs() <= 1e-6 * (1.0 + p[2].abs()),
                            "knots {v:?}: off the plane {p:?}"
                        );
                    }
                    Err(e) => assert!(!e.contains("Defect"), "knots {v:?}: {e}"),
                }
            }
        }
    }
}

/// solidcore plan/upstream-issues/UB-unnumbered--smsurface-surface-p8-segfaults-on-extreme-1e308-b-spline.md
/// (b4de2149): a B-spline surface whose net held coordinates near 1e308 beside points near 1e-6
/// crashed the C++ evaluator (signal 11). Ours builds the surface or refuses it, and every
/// evaluation is a finite point inside the net's box, or a typed refusal: never a panic, a hang
/// or a wrong finite value.
#[test]
fn ub_unnumbered_an_extreme_net_evaluates_inside_its_box_or_is_refused() {
    let k = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    for big in [1e300, 1e307, 1.7e308] {
        let bu = Basis::new(2, k.clone()).unwrap();
        let bv = Basis::new(2, k.clone()).unwrap();
        let mut points = net(3, 3, 1e-6);
        points[4] = [big, -big, big];
        points[8] = [-big, big, 1e-6];
        let Ok(surface) = Surface::<3>::new(bu, bv, &points) else {
            continue;
        };
        let kind = SurfaceKind::Nurbs(surface);
        for a in 0..=4 {
            for b in 0..=4 {
                let (pu, pv) = (f64::from(a) / 4.0, f64::from(b) / 4.0);
                match point_at(&kind, pu, pv) {
                    Ok(p) => {
                        assert!(p.iter().all(|x| x.is_finite()), "net {big}: {p:?}");
                        assert!(p.iter().all(|x| x.abs() <= big), "net {big}: outside {p:?}");
                    }
                    Err(e) => assert!(!e.contains("Defect"), "net {big}: {e}"),
                }
            }
        }
    }
}
