//! The benchmark of piece G67 (chapter 30): what the analytic and transcendental enclosures cost.
//!
//! Run with `cargo bench -p geom-curve --bench enclosure`. It uses the standard library's clock
//! only (wall-clock time on this machine; the reference machine and instruction counts are
//! geom-bench's, piece G18). It times, per call: a closed-form extent of each scalar in one
//! parameter, an analytic kind's bound and derivative bound over a span, a transition's bound and
//! derivative bound over a span for three laws, the clothoid's point by the Fresnel integrals
//! beside its point by quadrature, and the Fresnel integrals on each of their ranges.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_curve::extent::{Scalar, extent};
use geom_curve::transition::fresnel::fresnel;
use geom_curve::{CurvatureLaw, Frame, Helix, HelixAxes, Involute, ParamCurve, Transition};
use geom_num::interval::Interval;
use geom_nurbs::Limit;
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Runs `f` `n` times and prints the time per call.
fn time(name: &str, n: u32, mut f: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..n {
        f();
    }
    let per = start.elapsed().as_secs_f64() / f64::from(n);
    println!("{name:<52} {:>10.3} µs", per * 1e6);
}

fn point(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

fn main() {
    println!("Closed-form extents in one parameter, over [−0.7, 2.9]:");
    let scalars = [
        (
            "  wave (a circle's coordinate)",
            Scalar::Wave {
                p: point(1.0),
                q: point(0.0),
                a: point(0.6),
                b: point(-0.8),
            },
        ),
        (
            "  wave with a slope (a helix's)",
            Scalar::Wave {
                p: point(1.0),
                q: point(0.3),
                a: point(0.6),
                b: point(-0.8),
            },
        ),
        (
            "  quadratic (a parabola's)",
            Scalar::Quadratic {
                p: point(1.0),
                q: point(-2.0),
                r: point(0.7),
            },
        ),
        (
            "  hyperbolic (a hyperbola's)",
            Scalar::Hyperbolic {
                p: point(0.0),
                a: point(1.0),
                b: point(0.5),
            },
        ),
        (
            "  involute",
            Scalar::Involute {
                p: point(0.0),
                a: point(0.6),
                b: point(-0.8),
            },
        ),
        (
            "  modulated (monotone pieces)",
            Scalar::Modulated {
                p: point(0.0),
                a: point(0.6),
                b: point(-0.8),
                c: point(1.9),
                d: point(2.5),
            },
        ),
    ];
    for (name, g) in &scalars {
        time(name, 100_000, || {
            black_box(extent(g, (-0.7, 2.9), &mut meter()).unwrap());
        });
    }

    println!("Analytic kinds over a span:");
    let z = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let helix = Helix::new(
        [0.0; 3],
        HelixAxes::Frame {
            frame: z,
            radius: 3.0,
            pitch: 0.5,
        },
        (0.0, 2000.0),
    )
    .unwrap();
    time("  helix bound over a thousand turns", 10_000, || {
        black_box(helix.bound((0.0, 2000.0), &mut meter()).unwrap());
    });
    time("  helix second derivative bound", 10_000, || {
        black_box(
            helix
                .derivative_bound((0.0, 2000.0), 2, &mut meter())
                .unwrap(),
        );
    });
    let involute = Involute::new([0.0; 3], 1.0, z, (0.0, 3.0)).unwrap();
    time("  involute bound", 10_000, || {
        black_box(involute.bound((0.2, 2.7), &mut meter()).unwrap());
    });
    time("  involute first derivative bound (pieces)", 10_000, || {
        black_box(
            involute
                .derivative_bound((0.2, 2.7), 1, &mut meter())
                .unwrap(),
        );
    });

    println!("Transitions of 120 m over [10 m, 110 m]:");
    let laws = [
        (
            "clothoid",
            CurvatureLaw::Clothoid {
                start: 0.0,
                end: 1.0 / 400.0,
            },
        ),
        (
            "Bloss",
            CurvatureLaw::Bloss {
                start: 0.0,
                end: 1.0 / 400.0,
            },
        ),
        (
            "sine spiral",
            CurvatureLaw::SineSpiral {
                constant: Some(800.0),
                linear: Some(300.0),
                sine: 600.0,
                length: 150.0,
                from: 20.0,
            },
        ),
    ];
    for (name, law) in laws {
        let t = Transition::<2>::new([0.0, 0.0], [1.0, 0.0], 120.0, law, &mut meter()).unwrap();
        time(&format!("  {name}: bound"), 10_000, || {
            black_box(t.bound((10.0, 110.0), &mut meter()).unwrap());
        });
        time(
            &format!("  {name}: second derivative bound"),
            10_000,
            || {
                black_box(t.derivative_bound((10.0, 110.0), 2, &mut meter()).unwrap());
            },
        );
    }
    let clothoid = Transition::<2>::new(
        [0.0, 0.0],
        [1.0, 0.0],
        120.0,
        CurvatureLaw::Clothoid {
            start: 0.0,
            end: 1.0 / 400.0,
        },
        &mut meter(),
    )
    .unwrap();
    time("  clothoid point by quadrature (enclose)", 100_000, || {
        black_box(
            clothoid
                .enclose(77.0, Limit::FromRight, 0, &mut meter())
                .unwrap(),
        );
    });
    time("  clothoid point by the Fresnel integrals", 100_000, || {
        black_box(clothoid.enclose_fresnel(77.0, &mut meter()).unwrap());
    });

    println!("Fresnel integrals:");
    for (name, x) in [
        ("  series range, x = 0.7", 0.7),
        ("  between the ranges, x = 2.5", 2.5),
        ("  between the ranges, x = 4.9", 4.9),
        ("  expansion range, x = 7", 7.0),
        ("  expansion range, x = 10⁶", 1e6),
    ] {
        time(name, 10_000, || {
            black_box(fresnel(point(x)).unwrap());
        });
    }
}
