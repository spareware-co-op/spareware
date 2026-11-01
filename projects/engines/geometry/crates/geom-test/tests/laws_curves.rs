//! The law catalogue's kinds row for curves (chapter 28.3; piece G69), in 2D and 3D, for every
//! curve kind: the analytic kinds, NURBS, trimmed and composite curves, offsets, bulge
//! polylines, curves on surfaces and transitions. Every stated bound is refuted, if it is wrong,
//! by a dense sample of the kind's definition (`tests/truth`), in 400-bit fixed point or exactly.
//!
//! - **Conversion to NURBS within its stated error.** At the ends, at every break of the
//!   parameter map and either side of it, and at evenly spaced and random parameters, the NURBS
//!   form at the parameter its map gives (both exact) lies within the stated deviation of the
//!   kind's definition.
//! - **Transforms change a kind only as chapter 7 says.** Under grid transforms, rotations, a
//!   uniform and a non-uniform scale, each kind becomes the kind its rule names (a circle an
//!   ellipse under a non-uniform scale, and so on: `truth::curve_rule`), and the image's
//!   enclosures, widened by the stated deviation, hold the exact image of the definition.
//! - **Grid translations, quarter turns and mirrors are exact.** On data on a grid of 2^−12, each
//!   gives deviation 0 and an image whose enclosures hold the exact image unwidened; four quarter
//!   turns, two mirrors, and a translation and its opposite give the kind back unchanged.
//!
//! Each law lists every broken case it finds (the first failure of each, and how many), not just
//! the first. Seeds: each law prints `seed: <hex>` and replays with `SPAREKIT_SEED`; pull requests
//! run the fixed default seed, and the nightly workflow a fresh one (`kernel-nightly.yml`).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    reason = "tests: oracles and generators written as the formulas read, and a failure stops the test"
)]

// Piece G63's transitions and their truth, shared with `geom-curve`'s own tests.
#[allow(
    dead_code,
    unreachable_pub,
    reason = "the catalogue uses the transitions' truth only"
)]
#[path = "../../geom-curve/tests/laws/mod.rs"]
mod laws;
#[allow(
    dead_code,
    unreachable_pub,
    reason = "the catalogue uses the transitions' truth only"
)]
#[path = "../../geom-curve/tests/series/mod.rs"]
mod series;
mod truth;

use dashu_int::IBig;
use geom_curve::map::NurbsForm;
use geom_curve::{CurveError, CurveKind, ParamCurve};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_nurbs::Limit;
use geom_test::fixed::{self, within};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use truth::{
    Class, Entries, Findings, Rng, Truth, curve_name, curve_rule, curves, entries, fx, image,
    map_at, map_breaks, meter, samples, scale, slack, spline_at, spline_of, transforms,
};

fn rng(label: &str) -> Rng {
    Rng(SplitMix64::new(resolve(label, Seed::DEFAULT).unwrap()))
}

fn conversion<const D: usize>(label: &str) {
    let rng = &mut rng(label);
    let m = &mut meter();
    let mut found = Findings::default();
    let mut checked = 0usize;
    for round in 0..scale(4) {
        for (kind, truth) in curves::<D>(rng, false) {
            let name = curve_name(&kind);
            let form = match kind.to_nurbs(m) {
                Ok(f) => f,
                Err(e) => {
                    found.add(format!("{name}: refused"), || {
                        format!("round {round}: {e}: {kind:?}")
                    });
                    continue;
                }
            };
            let NurbsForm { curve, map } = &form.value;
            let spline = spline_of(curve);
            let dev = fx(form.deviation);
            let n = if truth.coarse() { scale(6) } else { scale(32) };
            for t in samples(rng, ParamCurve::domain(&kind), &map_breaks(map), n) {
                let tf = fx(t);
                let p = truth.point(&tf);
                let q: [_; D] = spline_at(&spline, &map_at(map, &tf));
                let r = &dev + slack(truth.coarse(), &p);
                if !within(&p, &q, &r) {
                    found.add(format!("{name}: beyond its deviation"), || {
                        format!(
                            "round {round} at {t}: {} away, stated {}",
                            fixed::distance(&p, &q),
                            form.deviation
                        )
                    });
                }
                checked += 1;
            }
        }
    }
    println!("{label}: {checked} parameters checked");
    found.assert_none(label);
    assert!(checked > 100);
}

#[test]
fn plane_curves_convert_to_nurbs_within_their_stated_error() {
    conversion::<2>("G69 plane curve kinds' NURBS forms against their definitions");
}

#[test]
fn space_curves_convert_to_nurbs_within_their_stated_error() {
    conversion::<3>("G69 space curve kinds' NURBS forms against their definitions");
}

/// Checks `moved` against the exact image of `truth` at samples of the domain: its enclosure,
/// widened by `dev`, must hold the exact image.
fn check_image<const D: usize>(
    found: &mut Findings,
    case: &str,
    rng: &mut Rng,
    (kind, truth): (&CurveKind<D>, &Truth<D>),
    moved: &CurveKind<D>,
    e: &Entries,
    dev: &IBig,
) -> usize {
    let n = if truth.coarse() { scale(4) } else { scale(16) };
    let mut checked = 0;
    for t in samples(rng, ParamCurve::domain(kind), &[], n) {
        let exact = image(e, &truth.point(&fx(t)));
        let r = dev + slack(truth.coarse(), &exact);
        match moved.enclose(t, Limit::FromRight, 0, &mut meter()) {
            Ok(got) => {
                let got = got[0];
                let holds = (0..D)
                    .all(|i| fx(got[i].lo()) - &r <= exact[i] && exact[i] <= fx(got[i].hi()) + &r);
                if !holds {
                    found.add(format!("{case}: misses the exact image"), || {
                        format!("at {t}")
                    });
                }
            }
            Err(err) => found.add(format!("{case}: refused an evaluation"), || {
                format!("at {t}: {err}")
            }),
        }
        checked += 1;
    }
    checked
}

fn kinds_under_transforms<const D: usize>(label: &str) {
    let rng = &mut rng(label);
    let m = &mut meter();
    let mut found = Findings::default();
    let mut checked = 0usize;
    for round in 0..scale(3) {
        let all = transforms::<D>(rng);
        for (kind, truth) in curves::<D>(rng, false) {
            let name = curve_name(&kind);
            for (tr, class, what) in &all {
                let case = format!("{name} under {what}");
                let moved = match kind.transformed(tr, m) {
                    Ok(x) => x,
                    Err(e) => {
                        found.add(format!("{case}: refused"), || format!("round {round}: {e}"));
                        continue;
                    }
                };
                let want = curve_rule(&kind, *class, moved.deviation == 0.0);
                let got = curve_name(&moved.value);
                if got != want {
                    found.add(format!("{case}: became {got}"), || {
                        format!(
                            "round {round}: its rule makes it {want}; deviation {}",
                            moved.deviation
                        )
                    });
                }
                let dev = fx(moved.deviation);
                checked += check_image(
                    &mut found,
                    &case,
                    rng,
                    (&kind, &truth),
                    &moved.value,
                    &entries(tr),
                    &dev,
                );
            }
        }
    }
    println!("{label}: {checked} parameters checked");
    found.assert_none(label);
    assert!(checked > 100);
}

#[test]
fn plane_curves_change_kind_only_as_chapter_7_says() {
    kinds_under_transforms::<2>("G69 plane curve kinds under transforms");
}

#[test]
fn space_curves_change_kind_only_as_chapter_7_says() {
    kinds_under_transforms::<3>("G69 space curve kinds under transforms");
}

/// `k` under `t`, `times` times.
fn repeated<const D: usize>(
    k: &CurveKind<D>,
    t: &TypedTransform,
    times: usize,
) -> Result<CurveKind<D>, CurveError> {
    let m = &mut meter();
    let mut out = k.clone();
    for _ in 0..times {
        out = out.transformed(t, m)?.value;
    }
    Ok(out)
}

fn grid_transforms_are_exact<const D: usize>(label: &str) {
    let rng = &mut rng(label);
    let m = &mut meter();
    let mut found = Findings::default();
    let mut checked = 0usize;
    for round in 0..scale(3) {
        let grid: Vec<_> = transforms::<D>(rng)
            .into_iter()
            .filter(|(_, c, _)| *c == Class::Grid)
            .collect();
        for (kind, truth) in curves::<D>(rng, true) {
            let name = curve_name(&kind);
            for (tr, _, what) in &grid {
                let case = format!("{name} under {what}");
                let moved = match kind.transformed(tr, m) {
                    Ok(x) => x,
                    Err(e) => {
                        found.add(format!("{case}: refused"), || format!("round {round}: {e}"));
                        continue;
                    }
                };
                if moved.deviation != 0.0 {
                    found.add(format!("{case}: deviation not 0"), || {
                        format!("round {round}: {}", moved.deviation)
                    });
                }
                checked += check_image(
                    &mut found,
                    &case,
                    rng,
                    (&kind, &truth),
                    &moved.value,
                    &entries(tr),
                    &IBig::ZERO,
                );
            }
            // A curve on a surface scales its error bound by the transform's stretch bound (√3
            // for a signed permutation, by its stated rule), so a round trip keeps its curve
            // exactly but not that bound; every other kind comes back unchanged.
            if matches!(kind, CurveKind::OnSurface(_)) {
                continue;
            }
            let (turn, mirror) = if D == 2 {
                (Axis::Z, Axis::X)
            } else {
                (Axis::Y, Axis::X)
            };
            let turn = TypedTransform::Exact(GridTransform::quarter_turns(turn, 1));
            let mirror = TypedTransform::Exact(GridTransform::mirror(mirror));
            let there = TypedTransform::Exact(GridTransform::translation([7, -3, 0]).unwrap());
            let back = TypedTransform::Exact(GridTransform::translation([-7, 3, 0]).unwrap());
            let trips = [
                ("four quarter turns", repeated(&kind, &turn, 4)),
                ("two mirrors", repeated(&kind, &mirror, 2)),
                (
                    "a translation and back",
                    repeated(&kind, &there, 1).and_then(|k| repeated(&k, &back, 1)),
                ),
            ];
            for (what, trip) in trips {
                if trip.as_ref().ok() != Some(&kind) {
                    found.add(format!("{name} after {what}: changed"), || {
                        format!("round {round}")
                    });
                }
            }
        }
    }
    println!("{label}: {checked} parameters checked");
    found.assert_none(label);
    assert!(checked > 100);
}

#[test]
fn plane_curves_are_exact_under_grid_transforms() {
    grid_transforms_are_exact::<2>("G69 plane curve kinds under grid transforms");
}

#[test]
fn space_curves_are_exact_under_grid_transforms() {
    grid_transforms_are_exact::<3>("G69 space curve kinds under grid transforms");
}
