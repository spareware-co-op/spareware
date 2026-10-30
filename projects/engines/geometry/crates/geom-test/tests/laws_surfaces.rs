//! The law catalogue's kinds row for surfaces (chapter 28.3; piece G69), for every surface kind:
//! plane, cylinder, cone, sphere, torus and their affine images, NURBS, revolution, extrusion,
//! ruled and offset surfaces. Every stated bound is refuted, if it is wrong, by a dense sample of
//! the kind's definition (`tests/truth`), in 400-bit fixed point or exactly.
//!
//! - **Conversion to NURBS within its stated error.** On a grid of parameters over the domain,
//!   with every break of the two parameter maps and either side of it, the NURBS form at the
//!   parameters its maps give (exactly) lies within the stated deviation of the definition.
//! - **Transforms change a kind only as chapter 7 says.** Under grid transforms, rotations, a
//!   uniform and a non-uniform scale, each kind becomes the kind its rule names (a cylinder an
//!   affine image under a non-uniform scale, and so on: `truth::surface_rule`), and the image's
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

// Piece G63's transitions and their truth, which the shared kinds module draws on.
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
use geom_curve::CurveError;
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_nurbs::Limit;
use geom_surface::{ParamSurface, SurfaceForm, SurfaceKind};
use geom_test::fixed::{self, within};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use truth::{
    Class, Entries, Findings, Rng, Surface, entries, fx, image, map_at, map_breaks, meter,
    patch_at, patch_of, samples, scale, slack, surface_name, surface_rule, surfaces, transforms,
};

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn rng(label: &str) -> Rng {
    Rng(SplitMix64::new(resolve(label, Seed::DEFAULT).unwrap()))
}

/// A grid of parameter pairs over the domain, with the breaks given in each direction.
fn grid(
    rng: &mut Rng,
    (us, vs): ((f64, f64), (f64, f64)),
    breaks: (&[f64], &[f64]),
    n: usize,
) -> Vec<(f64, f64)> {
    let (a, b) = (samples(rng, us, breaks.0, n), samples(rng, vs, breaks.1, n));
    a.iter()
        .flat_map(|&u| b.iter().map(move |&v| (u, v)))
        .collect()
}

#[test]
fn surfaces_convert_to_nurbs_within_their_stated_error() {
    let label = "G69 surface kinds' NURBS forms against their definitions";
    let rng = &mut rng(label);
    let m = &mut meter();
    let mut found = Findings::default();
    let mut checked = 0usize;
    for round in 0..scale(3) {
        for (kind, truth) in surfaces(rng, false) {
            let name = surface_name(&kind);
            let form = match kind.to_nurbs(m) {
                Ok(f) => f,
                Err(e) => {
                    found.add(format!("{name}: refused"), || format!("round {round}: {e}"));
                    continue;
                }
            };
            let SurfaceForm { surface, u, v } = &form.value;
            let patch = patch_of(surface);
            let dev = fx(form.deviation);
            let n = if truth.coarse() { scale(3) } else { scale(6) };
            let breaks = (map_breaks(u), map_breaks(v));
            for (s, t) in grid(rng, kind.domain(), (&breaks.0, &breaks.1), n) {
                let (sf, tf) = (fx(s), fx(t));
                let p = truth.point(&sf, &tf);
                let q = patch_at(&patch, &map_at(u, &sf), &map_at(v, &tf));
                let r = &dev + slack(truth.coarse(), &p);
                if !within(&p, &q, &r) {
                    found.add(format!("{name}: beyond its deviation"), || {
                        format!(
                            "round {round} at ({s}, {t}): {} away, stated {}",
                            fixed::distance(&p, &q),
                            form.deviation
                        )
                    });
                }
                checked += 1;
            }
        }
    }
    println!("{label}: {checked} parameter pairs checked");
    found.assert_none(label);
    assert!(checked > 100);
}

/// Checks `moved` against the exact image of `truth` on a grid over the domain: its enclosure,
/// widened by `dev`, must hold the exact image.
fn check_image(
    found: &mut Findings,
    case: &str,
    rng: &mut Rng,
    (kind, truth): (&SurfaceKind, &Surface),
    moved: &SurfaceKind,
    e: &Entries,
    dev: &IBig,
) -> usize {
    let n = if truth.coarse() { scale(2) } else { scale(4) };
    let mut checked = 0;
    for (u, v) in grid(rng, kind.domain(), (&[], &[]), n) {
        let exact = image(e, &truth.point(&fx(u), &fx(v)));
        let r = dev + slack(truth.coarse(), &exact);
        match moved.enclose(u, v, SIDES, &mut meter()) {
            Ok(got) => {
                let got = got[0];
                let holds = (0..3)
                    .all(|i| fx(got[i].lo()) - &r <= exact[i] && exact[i] <= fx(got[i].hi()) + &r);
                if !holds {
                    found.add(format!("{case}: misses the exact image"), || {
                        format!("at ({u}, {v})")
                    });
                }
            }
            Err(err) => found.add(format!("{case}: refused an evaluation"), || {
                format!("at ({u}, {v}): {err}")
            }),
        }
        checked += 1;
    }
    checked
}

#[test]
fn surfaces_change_kind_only_as_chapter_7_says() {
    let label = "G69 surface kinds under transforms";
    let rng = &mut rng(label);
    let m = &mut meter();
    let mut found = Findings::default();
    let mut checked = 0usize;
    for round in 0..scale(2) {
        let all = transforms::<3>(rng);
        for (kind, truth) in surfaces(rng, false) {
            let name = surface_name(&kind);
            for (tr, class, what) in &all {
                let case = format!("{name} under {what}");
                let moved = match kind.transformed(tr, m) {
                    Ok(x) => x,
                    Err(e) => {
                        found.add(format!("{case}: refused"), || format!("round {round}: {e}"));
                        continue;
                    }
                };
                let want = surface_rule(&kind, *class);
                let got = surface_name(&moved.value);
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
    println!("{label}: {checked} parameter pairs checked");
    found.assert_none(label);
    assert!(checked > 100);
}

/// `k` under `t`, `times` times.
fn repeated(k: &SurfaceKind, t: &TypedTransform, times: usize) -> Result<SurfaceKind, CurveError> {
    let m = &mut meter();
    let mut out = k.clone();
    for _ in 0..times {
        out = out.transformed(t, m)?.value;
    }
    Ok(out)
}

#[test]
fn surfaces_are_exact_under_grid_transforms() {
    let label = "G69 surface kinds under grid transforms";
    let rng = &mut rng(label);
    let m = &mut meter();
    let mut found = Findings::default();
    let mut checked = 0usize;
    for round in 0..scale(2) {
        let grid_transforms: Vec<_> = transforms::<3>(rng)
            .into_iter()
            .filter(|(_, c, _)| *c == Class::Grid)
            .collect();
        for (kind, truth) in surfaces(rng, true) {
            let name = surface_name(&kind);
            for (tr, _, what) in &grid_transforms {
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
            let turn = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Y, 1));
            let mirror = TypedTransform::Exact(GridTransform::mirror(Axis::X));
            let there = TypedTransform::Exact(GridTransform::translation([7, -3, 5]).unwrap());
            let back = TypedTransform::Exact(GridTransform::translation([-7, 3, -5]).unwrap());
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
    println!("{label}: {checked} parameter pairs checked");
    found.assert_none(label);
    assert!(checked > 100);
}
