//! The benchmark of piece G254 (chapter 30): the frame rules' main operations, one timing each:
//! placing rotation-minimising stations on a helix and on a twisted cubic, a placement under each
//! rule, a placement with twist and scale laws, and the certified Frenet and fixed-reference tests.
//!
//! Run with `cargo bench -p geom-surface --bench frames`. It uses the standard library's clock only
//! (wall-clock time on this machine; the reference machine and instruction counts are geom-bench's,
//! piece G18).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    clippy::too_many_lines,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_curve::{CurveKind, Frame, Helix, HelixAxes, Polynomial};
use geom_run::{Budget, WorkMeter};
use geom_surface::frame_rule::{
    Closing, FrameRule, Law, Laws, ProfileAxis, Stations, frenet_defined, placement,
    reference_defined,
};

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("{name:<52} {us:>12.3} µs");
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn main() {
    let helix = CurveKind::Helix(
        Helix::new(
            [0.0; 3],
            HelixAxes::Frame {
                frame: Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
                radius: 2.0,
                pitch: 1.5,
            },
            (0.0, 6.0),
        )
        .unwrap(),
    );
    let cubic = CurveKind::Polynomial(
        Polynomial::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.5]],
            (0.1, 2.0),
        )
        .unwrap(),
    );
    let start = [0.0, 0.0, 1.0];
    for (name, path) in [("helix", &helix), ("twisted cubic", &cubic)] {
        time(
            &format!("stations on a {name}, 1e-9 over radius 1"),
            20,
            || {
                black_box(
                    Stations::build(path, start, 1.0, 1e-9, Closing::Open, &mut meter()).unwrap(),
                );
            },
        );
    }
    let stations = Stations::build(&cubic, start, 1.0, 1e-9, Closing::Open, &mut meter()).unwrap();
    let rules = [
        ("translate", FrameRule::Translate),
        (
            "rotation-minimising",
            FrameRule::RotationMinimising(stations),
        ),
        ("Frenet", FrameRule::Frenet),
        (
            "fixed reference",
            FrameRule::FixedReference {
                reference: [0.0, 0.0, 1.0],
                axis: ProfileAxis::X,
            },
        ),
    ];
    for (name, rule) in &rules {
        time(&format!("placement, {name}, on the cubic"), 20_000, || {
            black_box(placement(rule, &Laws::default(), &cubic, black_box(1.234)).unwrap());
        });
    }
    let helical = FrameRule::Helical {
        point: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        reference: [1.0, 0.0, 0.0],
        pitch: Law::Constant(1.5),
        radius: Law::Constant(2.0),
    };
    time("placement, helical", 20_000, || {
        black_box(placement(&helical, &Laws::default(), &cubic, black_box(1.234)).unwrap());
    });
    let laws = Laws {
        twist: Some(Law::Linear {
            start: 0.0,
            rate: 0.1,
        }),
        scale: Some(Law::Constant(1.5)),
    };
    let (_, rmf) = &rules[1];
    time(
        "placement with laws, on the helix (closed-form length)",
        20_000,
        || {
            black_box(placement(&FrameRule::Frenet, &laws, &helix, black_box(1.234)).unwrap());
        },
    );
    time(
        "placement with laws, on the cubic (quadrature)",
        2_000,
        || {
            black_box(placement(rmf, &laws, &cubic, black_box(1.234)).unwrap());
        },
    );
    time("certified Frenet test, helix", 200, || {
        frenet_defined(&helix, &mut meter()).unwrap();
    });
    time("certified fixed-reference test, helix", 200, || {
        reference_defined(&helix, [0.0, 0.0, 1.0], &mut meter()).unwrap();
    });
}
