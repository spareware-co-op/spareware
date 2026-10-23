//! The 1M-point transform benchmark of piece G45 (chapter 30), with the frame conversions beside
//! it: wall-clock time per point for every transform variant and every kind of frame.
//!
//! Run with `cargo bench -p geom-math`. It uses the standard library's clock only, so it builds
//! with no extra dependency; when the benchmark harness of chapter 28 (`geom-bench`, piece G06)
//! merges, these cases move there and gain its regression gate and instruction counts. Points are
//! drawn from a fixed `SplitMix64` sequence, so every run measures the same work.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "a benchmark is development code, like a test; the rules are for kernel code"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_math::{
    Axis, CrsId, FrameRecord, GridTransform, MapPoint, RationalTransform, Rotation, Scale,
    Transform, TypedTransform,
};
use geom_num::grid::{GridStep, P3};

const POINTS: usize = 1_000_000;
const FRAME_POINTS: usize = 200_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A coordinate within ±2^38, so every transform below keeps it in range.
    fn coord(&mut self) -> i64 {
        (self.next() % (1 << 39)) as i64 - (1 << 38)
    }
}

#[allow(
    clippy::unwrap_used,
    reason = "a benchmark is development code, like a test: a failure should stop it loudly"
)]
fn points(n: usize) -> Vec<P3> {
    let mut rng = Rng(0x06E0_3BE7);
    (0..n)
        .map(|_| P3::new(rng.coord(), rng.coord(), rng.coord() >> 8).unwrap())
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
    }
}
