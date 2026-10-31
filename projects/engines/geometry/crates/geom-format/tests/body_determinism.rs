//! Body records give the same bytes on every target and at any number of threads (principle 5,
//! chapter 21; G94): the records of bodies as geom-topo builds them and as an edit leaves them, and
//! kinds whose reading fits again (cached offsets) or integrates (a transition), each read back and
//! written again, hashed into one golden digest.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    reason = "a test corpus, and chapter 21's thread-count test spawns threads"
)]

#[path = "../../geom-topo/tests/common/mod.rs"]
#[allow(
    dead_code,
    reason = "geom-topo's fixtures; the corpus uses some of them"
)]
mod topo;

use geom_curve::{
    Basis, Circle, CurvatureLaw, CurveKind, Frame, Offset, OffsetLaw, Transition, Trimmed,
};
use geom_format::body::{read_body, write_body};
use geom_format::cbor::Decoder;
use geom_format::kinds::{DEPTH, read_curve, read_surface, write_curve, write_surface};
use geom_op::cbor::Encoder;
use geom_surface::{Cylinder, SurfaceKind};
use geom_test::golden::{self, Sha256, hex};
use geom_topo::{Body, Builder};

use topo::{ix, many_faces, meter, wall};

const GOLDEN: &str = "geom-format/body-records";

struct Corpus {
    total: Sha256,
    report: Vec<(&'static str, [u8; 32])>,
}

fn put(total: &mut Sha256, section: &mut Sha256, bytes: &[u8]) {
    for h in [total, section] {
        h.write(&(bytes.len() as u64).to_le_bytes());
        h.write(bytes);
    }
}

/// A body's records, each encoded, after reading it back and writing it again.
fn records(body: &Body, total: &mut Sha256, section: &mut Sha256) {
    let stored = write_body(body, &mut meter()).unwrap();
    let again = write_body(&read_body(&stored, &mut meter()).unwrap(), &mut meter()).unwrap();
    assert_eq!(again, stored);
    for r in stored.chunks.iter().chain([&stored.index]) {
        put(total, section, &r.encode(&mut meter()).unwrap());
    }
}

fn corpus() -> Corpus {
    let mut total = Sha256::new();
    let mut report = Vec::new();

    let mut s = Sha256::new();
    records(&wall(), &mut total, &mut s);
    let body = many_faces(9_000);
    records(&body, &mut total, &mut s);
    let mut arenas = body.arenas().clone();
    for row in 4_200..4_290 {
        arenas.faces.surface.set(ix(row), ix(1)).unwrap();
    }
    let edited = Builder { arenas }
        .publish(body.kind(), body.outside(), &mut meter())
        .unwrap();
    records(&edited, &mut total, &mut s);
    report.push(("bodies", s.finish()));

    let mut s = Sha256::new();
    let frame = Frame::new([0.6, 0.8, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let circle = Circle::new([1.0, -2.0, 0.5], 2.5, frame, (0.25, 1.5)).unwrap();
    let law = OffsetLaw::Reference {
        distance: 0.5,
        direction: [0.0, 0.0, 1.0],
    };
    let offset = Offset::new(
        Trimmed::new(Basis::Circle(circle), (0.5, 1.0)).unwrap(),
        law,
    )
    .unwrap()
    .cached(&mut meter())
    .unwrap();
    let clothoid = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.01,
    };
    let transition =
        Transition::<2>::new([0.0, 0.0], [1.0, 0.0], 100.0, clothoid, &mut meter()).unwrap();
    for bytes in [
        curve_again(&CurveKind::Offset(offset)),
        curve_again(&CurveKind::Transition(
            transition.segment((10.0, 60.0), true).unwrap(),
        )),
    ] {
        put(&mut total, &mut s, &bytes);
    }
    let cylinder = Cylinder::new([0.0; 3], 2.0, frame, ((0.0, 1.0), (-1.0, 3.0))).unwrap();
    let surface = geom_surface::Offset::new(SurfaceKind::Cylinder(cylinder), 0.5)
        .unwrap()
        .cached(&mut meter())
        .unwrap();
    put(
        &mut total,
        &mut s,
        &surface_again(&SurfaceKind::Offset(surface)),
    );
    report.push(("kinds", s.finish()));

    Corpus { total, report }
}

/// A curve's bytes after reading it back.
fn curve_again<const D: usize>(c: &CurveKind<D>) -> Vec<u8> {
    let mut e = Encoder::new();
    write_curve(&mut e, c).unwrap();
    let bytes = e.finish().unwrap().into_bytes();
    let back = read_curve::<D>(&mut Decoder::new(&bytes, DEPTH), &mut meter()).unwrap();
    assert_eq!(&back, c);
    let mut e = Encoder::new();
    write_curve(&mut e, &back).unwrap();
    e.finish().unwrap().into_bytes()
}

/// A surface's bytes after reading it back.
fn surface_again(s: &SurfaceKind) -> Vec<u8> {
    let mut e = Encoder::new();
    write_surface(&mut e, s).unwrap();
    let bytes = e.finish().unwrap().into_bytes();
    let back = read_surface(&mut Decoder::new(&bytes, DEPTH), &mut meter()).unwrap();
    assert_eq!(&back, s);
    let mut e = Encoder::new();
    write_surface(&mut e, &back).unwrap();
    e.finish().unwrap().into_bytes()
}

#[test]
fn golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let c = corpus();
        (c.total.clone().finish(), c)
    });
    for (name, digest) in &checked.value.report {
        println!("  {name:<8} {}", hex(*digest));
    }
    println!(
        "geom-format body-records golden hash: {}",
        hex(checked.value.total.clone().finish())
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!("{e}. Compare the section digests above with another target's log to find the part that differs")
    });
}

/// Chapter 21's thread-count test: the same digest on 1, 2, 3 and 8 threads at once.
#[test]
#[cfg_attr(
    all(target_family = "wasm", not(target_feature = "atomics")),
    ignore = "WebAssembly without the atomics feature (wasm32-wasip1) has no threads: \
              std::thread::scope cannot spawn"
)]
fn golden_hash_is_the_same_on_every_thread_count() {
    let one = hex(corpus().total.finish());
    for threads in golden::THREAD_COUNTS {
        let digests: Vec<String> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| s.spawn(|| hex(corpus().total.finish())))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(
            digests.iter().all(|d| *d == one),
            "{threads} threads: {digests:?}"
        );
    }
}
