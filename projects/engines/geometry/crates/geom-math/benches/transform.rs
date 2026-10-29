//! The 1M-point transform benchmark of piece G45 (chapter 30), with the frame conversions beside
//! it: wall-clock time per point for every transform variant, in 3D and in the plane (2D points,
//! through the same transforms at z = 0), and every kind of frame. Piece G46 adds its rational
//! rotation (the variant for rotations too large for a rational transform) and its searches: the
//! closest rational rotation within the angular bound in 2D and 3D, and re-approximated
//! compositions. Piece G48 adds frames whose map coordinates are in US survey feet (the exact
//! path) and survey coordinates given as decimal text: reading the text, and converting it.
//!
//! Run with `cargo bench -p geom-math`. It uses the standard library's clock only, so it builds
//! with no extra dependency; when the benchmark harness of chapter 28 (`geom-bench`, piece G06)
//! merges, these cases move there and gain its regression gate and instruction counts. Points are
//! drawn from the kernel's one generator, `geom_num::rng` (Philox4x32-10), with a fixed seed and
//! stream, so every run measures the same work.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "a benchmark is development code, like a test; the rules are for kernel code"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "a benchmark measures wall-clock time by design (Instant::now and elapsed); the time is               printed, never fed into anything the kernel computes"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_math::{
    Axis, CrsId, Decimal, DecimalPoint, F2, FrameRecord, GridTransform, LinearUnit, MapPoint,
    RationalRotation, RationalTransform, Rotation, Scale, Transform, TypedTransform,
};
use geom_num::grid::{GridStep, P2, P3};
use geom_num::{ladder, math};
use geom_num::rng::{Philox, Seed, Stream, StreamId};

const POINTS: usize = 1_000_000;
const FRAME_POINTS: usize = 200_000;

/// A coordinate within ±2^38, so every transform below keeps it in range: a whole number drawn
/// uniformly from [0, 2^39), less 2^38.
#[allow(
    clippy::unwrap_used,
    reason = "a benchmark is development code, like a test: a failure should stop it loudly"
)]
fn coord(stream: &mut Stream) -> i64 {
    stream.below(1 << 39).unwrap() as i64 - (1 << 38)
}

#[allow(
    clippy::unwrap_used,
    reason = "a benchmark is development code, like a test: a failure should stop it loudly"
)]
fn points(n: usize) -> Vec<P3> {
    let mut stream = Philox::new(Seed(0x06E0_3BE7), StreamId(0)).stream();
    (0..n)
        .map(|_| {
            let (x, y, z) = (coord(&mut stream), coord(&mut stream), coord(&mut stream));
            P3::new(x, y, z >> 8).unwrap()
        })
        .collect()
}

fn report(name: &str, n: usize, start: Instant, check: u64) {
    let ns = start.elapsed().as_nanos() as f64;
    let (ms, per) = (ns / 1e6, ns / n as f64);
    println!("{name:<48} {n:>9} points {ms:>9.1} ms {per:>8.1} ns/point (check {check:x})");
}

#[allow(
    clippy::unwrap_used,
    reason = "a benchmark is development code, like a test: a failure should stop it loudly"
)]
fn run(name: &str, pts: &[P3], t: &TypedTransform) {
    let start = Instant::now();
    let mut check = 0u64;
    for &p in pts {
        let r = t.apply(black_box(p)).unwrap();
        check = check.wrapping_add(r.value().x.get() as u64 ^ u64::from(r.is_exact()));
    }
    report(name, pts.len(), start, check);
}

#[allow(
    clippy::unwrap_used,
    reason = "a benchmark is development code, like a test: a failure should stop it loudly"
)]
fn run_2d(name: &str, pts: &[P2], t: &TypedTransform) {
    let start = Instant::now();
    let mut check = 0u64;
    for &p in pts {
        let r = t.apply_2d(black_box(p)).unwrap();
        check = check.wrapping_add(r.value().x.get() as u64 ^ u64::from(r.is_exact()));
    }
    report(name, pts.len(), start, check);
}

/// The 2D cases: the same kinds of transform, acting in the plane, on the points' x and y.
#[allow(
    clippy::unwrap_used,
    reason = "a benchmark is development code, like a test: a failure should stop it loudly"
)]
fn plane(pts: &[P3]) {
    let pts: Vec<P2> = pts.iter().map(|p| P2 { x: p.x, y: p.y }).collect();
    let grid = GridTransform::mirror_diagonal(false)
        .compose(&GridTransform::translation([12_345, -678, 0]).unwrap())
        .unwrap();
    run_2d(
        "2D exact: mirror about y = x and translation",
        &pts,
        &TypedTransform::Exact(grid),
    );
    let r34 = RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
    let r34 = r34
        .compose(&RationalTransform::translation([1, 2, 0], 2).unwrap())
        .unwrap();
    run_2d(
        "2D rational: Pythagorean rotation, half steps",
        &pts,
        &TypedTransform::Rational(r34),
    );
    let turn = Transform::rotation(Axis::Z, 1.0 / 3.0).unwrap();
    let shift = Transform::translation(geom_math::F3::new(0.25, -1.0, 0.0).unwrap());
    run_2d(
        "2D certified: rotation by 60 degrees, rounded once",
        &pts,
        &TypedTransform::Certified(shift.compose(&turn).unwrap()),
    );
    // Piece G46: the rational rotation within the angular bound of 60 degrees.
    let sixty = F2::new(0.5, 0.866_025_403_784_438_6).unwrap();
    let r60 = RationalRotation::plane_within(sixty, ladder::ANGULAR).unwrap();
    run_2d(
        "2D rotation: 60 degrees within 2^-34 rad (G46)",
        &pts,
        &TypedTransform::from(r60.value()),
    );
}

/// Piece G46's searches: the closest rational rotation within the angular bound, for 10,000
/// directions in 2D and 10,000 quaternions in 3D, and the exact or re-approximated compositions.
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test: a failure should stop it loudly"
)]
fn searches() {
    const N: usize = 10_000;
    let mut stream = Philox::new(Seed(0x0646_0646), StreamId(1)).stream();
    let mut unit = || {
        let v = stream.below(1 << 53).unwrap() as f64;
        v / (1u64 << 52) as f64 - 1.0
    };
    let dirs: Vec<F2> = (0..N).map(|_| F2::new(unit(), unit()).unwrap()).collect();
    let quats: Vec<[f64; 4]> = (0..N).map(|_| [unit(), unit(), unit(), unit()]).collect();
    let start = Instant::now();
    let mut check = 0u64;
    for &d in &dirs {
        let r = RationalRotation::plane_within(black_box(d), ladder::ANGULAR).unwrap();
        check = check.wrapping_add(r.value().quaternion()[0] as u64);
    }
    report("2D search within 2^-34 rad (G46)", N, start, check);
    let start = Instant::now();
    let mut found = Vec::with_capacity(N);
    for &q in &quats {
        let r = RationalRotation::within(black_box(q), ladder::ANGULAR).unwrap();
        check = check.wrapping_add(r.value().quaternion()[0] as u64);
        found.push(r.value());
    }
    report("3D search within 2^-34 rad (G46)", N, start, check);
    let start = Instant::now();
    for pair in found.chunks(2) {
        let r = pair[0].compose(&pair[1], ladder::ANGULAR).unwrap();
        check = check.wrapping_add(r.value().quaternion()[0] as u64);
    }
    report("3D composition, re-approximated (G46)", N / 2, start, check);
}

#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a benchmark is development code, like a test: a failure should stop it loudly"
)]
fn main() {
    let pts = points(POINTS);
    let grid = GridTransform::quarter_turns(Axis::Z, 1)
        .compose(&GridTransform::translation([12_345, -678, 9]).unwrap())
        .unwrap();
    run(
        "exact: quarter turn and translation",
        &pts,
        &TypedTransform::Exact(grid),
    );
    let r34 = RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap();
    let r34 = r34
        .compose(&RationalTransform::translation([1, 2, 3], 2).unwrap())
        .unwrap();
    run(
        "rational: Pythagorean rotation, half steps",
        &pts,
        &TypedTransform::Rational(r34),
    );
    let k = Scale::new(99_987_654, 100_000_000).unwrap();
    let scale = RationalTransform::uniform_scale(k).unwrap();
    run(
        "rational: scale 0.99987654",
        &pts,
        &TypedTransform::Rational(scale),
    );
    let turn = Transform::rotation(Axis::Z, 1.0 / 3.0).unwrap();
    let shift = Transform::translation(geom_math::F3::new(0.25, -1.0, 0.375).unwrap());
    let rot = shift.compose(&turn).unwrap();
    run(
        "certified: rotation by 60 degrees, rounded once",
        &pts,
        &TypedTransform::Certified(rot),
    );
    // The worst case: every z image is a midpoint, so every point takes the exact path.
    let ties = Transform::translation(geom_math::F3::new(0.25, -1.0, 0.5).unwrap());
    let ties = TypedTransform::Certified(ties.compose(&turn).unwrap());
    run("certified: the same, every z image a tie", &pts, &ties);
    let r3 = RationalRotation::within([0.9, 0.1, -0.3, 0.2], ladder::ANGULAR).unwrap();
    run(
        "rotation: 3D within 2^-34 rad, |q|^2 above 2^40 (G46)",
        &pts,
        &TypedTransform::from(r3.value()),
    );
    // The plain floating-point image, for comparison: no rounding to the grid.
    let start = Instant::now();
    let mut check = 0u64;
    for &p in &pts {
        let q = rot.apply_point(geom_math::F3::from(black_box(p))).unwrap();
        check = check.wrapping_add(q.x().to_bits());
    }
    report(
        "f64 apply_point (no grid rounding)",
        pts.len(),
        start,
        check,
    );
    plane(&pts);
    searches();

    // Frames: a site near Auckland, map to frame and back.
    let s = 6_400_000i64;
    let origin = [1_757_000 * s, 5_921_000 * s, 0];
    let nztm = CrsId::Epsg(2193);
    let ground = |c: i32, s: i32| {
        let r = Rotation::direction(c, s).unwrap();
        FrameRecord::new(nztm, GridStep::Q0, origin, r, k).unwrap()
    };
    let frames = [
        (
            "map frame",
            FrameRecord::map(nztm, GridStep::Q0, origin).unwrap(),
        ),
        ("ground frame, Pythagorean, scale", ground(3, 4)),
        (
            "ground frame, irrational direction, scale",
            ground(1_000, 7),
        ),
        (
            "map frame in US survey feet",
            FrameRecord::map(nztm, GridStep::Q0, origin)
                .unwrap()
                .with_linear_unit(LinearUnit::US_SURVEY_FOOT),
        ),
        (
            "ground frame in US survey feet, irrational direction, scale",
            ground(1_000, 7).with_linear_unit(LinearUnit::US_SURVEY_FOOT),
        ),
    ];
    let small = &pts[..FRAME_POINTS];
    for (name, fr) in frames {
        let maps: Vec<MapPoint> = small
            .iter()
            .map(|&p| fr.to_map(p).unwrap().value())
            .collect();
        let start = Instant::now();
        let mut check = 0u64;
        for &p in small {
            let easting = fr.to_map(black_box(p)).unwrap().value().easting;
            check = check.wrapping_add(easting.to_bits());
        }
        report(&format!("to_map, {name}"), small.len(), start, check);
        let start = Instant::now();
        let mut check = 0u64;
        for &m in &maps {
            // Nudged off the exact image, so most values are not whole steps.
            let m = MapPoint {
                easting: m.easting.next_up(),
                ..m
            };
            let x = fr.to_frame(black_box(m)).unwrap().value().x.get();
            check = check.wrapping_add(x as u64);
        }
        report(&format!("to_frame, {name}"), maps.len(), start, check);
        // The same points as decimal text to the micrometre, as a survey file gives them.
        let text: Vec<[String; 3]> = maps
            .iter()
            .map(|m| {
                [m.easting, m.northing, m.height].map(|v| {
                    Decimal::new(math::round_ties_even(v * 1e6).unwrap().get() as i128, -6)
                        .unwrap()
                        .to_string()
                })
            })
            .collect();
        let start = Instant::now();
        let mut check = 0u64;
        let mut decimals = Vec::with_capacity(text.len());
        for t in &text {
            let [e, n, h] = black_box(t).each_ref().map(|t| Decimal::parse(t).unwrap());
            check = check.wrapping_add(e.significand() as u64);
            decimals.push(DecimalPoint {
                easting: e,
                northing: n,
                height: h,
            });
        }
        report(&format!("parse decimal text, {name}"), text.len(), start, check);
        let start = Instant::now();
        let mut check = 0u64;
        for &d in &decimals {
            let x = fr.to_frame_decimal(black_box(d)).unwrap().value().x.get();
            check = check.wrapping_add(x as u64);
        }
        report(
            &format!("to_frame_decimal, {name}"),
            decimals.len(),
            start,
            check,
        );
    }
}
