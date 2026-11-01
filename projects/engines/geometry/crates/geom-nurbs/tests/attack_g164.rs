//! Attacks on G164's claims for the Bernstein toolkit: a patch's clip against a slab, one step of
//! the Interval Projected Polyhedron solver, is exact where the patch is flat, never drops a point
//! the slab holds, and keeps a meeting of one point on a domain's end; a curve's clip keeps that
//! one point too.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod common;

use common::Lcg;
use geom_num::interval::Interval;
use geom_nurbs::{BernsteinCurve, BernsteinPatch, Slab};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The interval holding exactly `x`.
fn pt(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

/// The flat patch x = 10·(u − 2)/4, y = 10·(v + 1)/4, z = 0 over [2, 6] × [−1, 3], bilinear: its
/// corners (0, 0), (0, 10), (10, 0) and (10, 10), in rows of u.
fn square() -> BernsteinPatch<3> {
    let corner = |x: f64, y: f64| [pt(x), pt(y), pt(0.0)];
    let points = vec![
        corner(0.0, 0.0),
        corner(0.0, 10.0),
        corner(10.0, 0.0),
        corner(10.0, 10.0),
    ];
    BernsteinPatch::new(((2.0, 6.0), (-1.0, 3.0)), (1, 1), points, None).unwrap()
}

fn along(normal: [f64; 3], lo: f64, hi: f64) -> Slab<3> {
    Slab {
        origin: [0.0; 3],
        normal,
        lo,
        hi,
    }
}

/// `got` holds `want` and reaches past it by less than 1e-12.
fn tight((a, b): (f64, f64), (lo, hi): (f64, f64)) {
    assert!(
        a <= lo && hi <= b && lo - a < 1e-12 && b - hi < 1e-12,
        "({a}, {b}) against ({lo}, {hi})"
    );
}

/// Claim: on a flat patch the clip is the slab's own range, in the patch's own parameters, and
/// the other parameter keeps its whole domain.
#[test]
fn a_flat_patch_clips_to_the_slab() {
    let m = &mut meter();
    // 2.5 ≤ x ≤ 5 is 3 ≤ u ≤ 4, for every v.
    let (u, v) = square()
        .clip(&along([1.0, 0.0, 0.0], 2.5, 5.0), m)
        .unwrap()
        .unwrap();
    tight(u, (3.0, 4.0));
    assert_eq!(v, (-1.0, 3.0));
    // 5 ≤ y ≤ 7.5 is 1 ≤ v ≤ 2, for every u.
    let (u, v) = square()
        .clip(&along([0.0, 1.0, 0.0], 5.0, 7.5), m)
        .unwrap()
        .unwrap();
    assert_eq!(u, (2.0, 6.0));
    tight(v, (1.0, 2.0));
    // The diagonal x + y ≤ 5 from below: u and v each up to where the other is at its least.
    let (u, v) = square()
        .clip(&along([1.0, 1.0, 0.0], -100.0, 5.0), m)
        .unwrap()
        .unwrap();
    tight(u, (2.0, 4.0));
    tight(v, (-1.0, 1.0));
}

/// Claim: a slab the patch never reaches clips it away, in either parameter alone.
#[test]
fn a_slab_the_patch_misses_clips_it_away() {
    let m = &mut meter();
    for slab in [
        along([1.0, 0.0, 0.0], 20.0, 30.0),
        along([1.0, 0.0, 0.0], -30.0, -20.0),
        along([0.0, 1.0, 0.0], 11.0, 12.0),
        along([0.0, 0.0, 1.0], 1.0, 2.0),
    ] {
        assert_eq!(square().clip(&slab, m).unwrap(), None, "{slab:?}");
    }
}

/// Claim: a slab that meets the patch only along its last edge keeps that edge, a range of one
/// parameter value; so does a curve that meets a slab only at its end.
#[test]
fn a_meeting_on_an_end_is_kept() {
    let m = &mut meter();
    let (u, v) = square()
        .clip(&along([1.0, 0.0, 0.0], 10.0, 20.0), m)
        .unwrap()
        .unwrap();
    assert!(u.0 <= 6.0 && u.1 == 6.0, "{u:?}");
    assert_eq!(v, (-1.0, 3.0));
    let rising = BernsteinCurve::new(
        (0.0, 1.0),
        vec![[pt(0.0), pt(0.0)], [pt(1.0), pt(1.0)]],
        None,
    )
    .unwrap();
    let slab = Slab {
        origin: [0.0, 0.0],
        normal: [0.0, 1.0],
        lo: 1.0,
        hi: 2.0,
    };
    let (start, end) = rising.clip(&slab, m).unwrap().unwrap();
    assert!(start <= 1.0 && end == 1.0, "({start}, {end})");
}

/// Evaluates a nonrational patch at (s, t) in [0, 1]² by de Casteljau on its coefficients' middles.
fn at(patch: &BernsteinPatch<3>, s: f64, t: f64) -> [f64; 3] {
    let (p, q) = patch.degrees();
    let mid = |x: &Interval| x.lo() * 0.5 + x.hi() * 0.5;
    let reduce = |mut row: Vec<[f64; 3]>, x: f64| {
        while row.len() > 1 {
            row = row
                .windows(2)
                .map(|w| core::array::from_fn(|d| w[0][d] * (1.0 - x) + w[1][d] * x))
                .collect();
        }
        row[0]
    };
    let rows: Vec<[f64; 3]> = (0..=p)
        .map(|i| {
            let row = (0..=q)
                .map(|j| patch.points()[i * (q + 1) + j].each_ref().map(mid))
                .collect();
            reduce(row, t)
        })
        .collect();
    reduce(rows, s)
}

/// Claim: the clip never drops a point of the patch that the slab holds. Random patches of degree
/// up to 3 by 3, random slabs, and every point of a 33 × 33 grid inside a slab by more than its
/// evaluation's rounding must lie in the clipped ranges.
#[test]
fn a_clip_keeps_every_point_the_slab_holds() {
    let m = &mut meter();
    let mut rng = Lcg(164);
    let mut kept = 0;
    for trial in 0..400 {
        let (p, q) = (rng.below(4), rng.below(4));
        let points: Vec<[Interval; 3]> = (0..(p + 1) * (q + 1))
            .map(|_| core::array::from_fn(|_| pt(200.0 * rng.unit() - 100.0)))
            .collect();
        let (u0, v0) = (10.0 * rng.unit() - 5.0, 10.0 * rng.unit() - 5.0);
        let domain = ((u0, u0 + 1.0 + rng.unit()), (v0, v0 + 1.0 + rng.unit()));
        let patch = BernsteinPatch::new(domain, (p, q), points, None).unwrap();
        let normal: [f64; 3] = core::array::from_fn(|_| 2.0 * rng.unit() - 1.0);
        let centre = 120.0 * rng.unit() - 60.0;
        let half = 40.0 * rng.unit();
        let slab = along(normal, centre - half, centre + half);
        let clip = patch.clip(&slab, m).unwrap();
        for i in 0..=32 {
            for j in 0..=32 {
                let (s, t) = (f64::from(i) / 32.0, f64::from(j) / 32.0);
                let x = at(&patch, s, t);
                let d = normal[0] * x[0] + normal[1] * x[1] + normal[2] * x[2];
                if d < slab.lo + 1e-9 || d > slab.hi - 1e-9 {
                    continue;
                }
                kept += 1;
                let ((a, b), (c, e)) = domain;
                let (u, v) = (a + s * (b - a), c + t * (e - c));
                let Some((cu, cv)) = clip else {
                    panic!("trial {trial}: ({u}, {v}) at distance {d} dropped");
                };
                assert!(
                    cu.0 <= u + 1e-12
                        && u - 1e-12 <= cu.1
                        && cv.0 <= v + 1e-12
                        && v - 1e-12 <= cv.1,
                    "trial {trial}: ({u}, {v}) outside {cu:?} × {cv:?}"
                );
            }
        }
    }
    assert!(kept > 10_000, "only {kept} points inside a slab");
}
