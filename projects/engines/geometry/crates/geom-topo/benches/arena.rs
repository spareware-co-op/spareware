//! The benchmarks of piece G80 (chapter 30): a million-face body's arenas filled, published,
//! edited in ten faces and republished, cloned, and one column scanned.
//!
//! Run with `cargo bench -p geom-topo`. It uses the standard library's clock only (wall-clock
//! time on this machine; the reference machine and instruction counts are geom-bench's, piece
//! G18). The inputs are fixed.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "a benchmark is development code, like a test; it measures wall-clock time by design"
)]

use std::hint::black_box;
use std::time::Instant;

use geom_math::Box3;
use geom_num::grid::P3;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::{Body, BodyKind, Builder, Face, Ix, Region, RegionKind, Sense, Shell};

fn time(name: &str, runs: u32, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("{name:<52} {us:>12.2} µs");
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn ix<K>(row: u32) -> Ix<K> {
    Ix::new(row).unwrap()
}

fn plane(z: f64) -> SurfaceKind {
    let domain = ((0.0, 1.0), (0.0, 1.0));
    SurfaceKind::Plane(Plane::new([0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain).unwrap())
}

const FACES: u32 = 1_000_000;

fn faces() -> Vec<Face> {
    (0..FACES)
        .map(|f| {
            let x = i64::from(f);
            Face {
                shell: ix(0),
                next: (f + 1 < FACES).then(|| ix(f + 1)),
                surface: ix(0),
                sense: Sense::Forward,
                first_loop: None,
                bound: Box3::new(P3::new(x, 0, 0).unwrap(), P3::new(x + 1, 1, 0).unwrap()).unwrap(),
            }
        })
        .collect()
}

fn filled(rows: &[Face]) -> Builder {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions
        .extend(&[
            Region {
                kind: RegionKind::Solid,
                first_shell: Some(ix(0)),
            },
            Region {
                kind: RegionKind::Void,
                first_shell: None,
            },
        ])
        .unwrap();
    a.shells
        .push(Shell {
            region: ix(0),
            next: None,
            first_face: Some(ix(0)),
        })
        .unwrap();
    a.surfaces.extend([plane(0.0), plane(1.0)]).unwrap();
    a.faces.extend(rows).unwrap();
    b
}

fn publish(b: Builder) -> Body {
    b.publish(BodyKind::Sheet, ix(1), &mut meter()).unwrap()
}

fn main() {
    let rows = faces();
    time("fill 1M faces (extend, exact chunks)", 5, || {
        black_box(filled(black_box(&rows)));
    });
    time("fill 1M faces (push, row by row)", 3, || {
        let mut b = Builder::new();
        for r in black_box(&rows) {
            b.arenas.faces.push(*r).unwrap();
        }
        black_box(b);
    });
    let body = publish(filled(&rows));
    time("publish 1M faces (every link checked)", 10, || {
        black_box(publish(Builder::edit(black_box(&body))));
    });
    time("edit 10 faces of 1M and publish", 10, || {
        let mut b = Builder::edit(black_box(&body));
        for k in 0..10 {
            b.arenas.faces.surface.set(ix(k * 100_003), ix(1)).unwrap();
        }
        black_box(publish(b));
    });
    time("clone a 1M-face body (one pointer per chunk)", 100, || {
        black_box(black_box(&body).clone());
    });
    time("scan 1M face boxes (one column)", 20, || {
        let mut lo = i64::MAX;
        for b in black_box(&body).arenas().faces.bound.iter() {
            lo = lo.min(b.min().x.get());
        }
        black_box(lo);
    });
    time("read 1M faces row by row (every column)", 5, || {
        let a = black_box(&body).arenas();
        for f in 0..FACES {
            black_box(a.faces.get(ix(f)));
        }
    });
}
