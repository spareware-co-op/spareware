//! Committed planes are the same on every target and at any number of threads (principle 5, chapter
//! 21; G142): seeded commits of both kinds and their images, every coefficient and every deviation's
//! bits, hashed into one golden digest.

#![allow(
    clippy::disallowed_methods,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test corpus, and chapter 21's thread-count test spawns threads"
)]

use geom_math::{Axis, GridTransform};
use geom_num::P3;
use geom_poly::commit::{Committed, PlaneClass};
use geom_test::golden::{self, Sha256, hex};
use geom_test::rng::SplitMix64;
use geom_test::seed::Seed;

const GOLDEN: &str = "geom-poly/commits";

/// A component in (-1, 1), from the top bits of a word.
fn unit_component(rng: &mut SplitMix64) -> f64 {
    let x = i64::from_ne_bytes(rng.next_u64().to_ne_bytes()) >> 11;
    #[allow(clippy::cast_precision_loss, reason = "53 bits, exact in a double")]
    let x = x as f64;
    x / 4_503_599_627_370_496.0
}

/// A grid coordinate of a building's size.
fn coordinate(rng: &mut SplitMix64) -> i64 {
    i64::from_ne_bytes(rng.next_u64().to_ne_bytes()) >> 40
}

fn put(hash: &mut Sha256, c: &Committed) {
    for x in c.plane.n() {
        hash.write(&x.to_le_bytes());
    }
    hash.write(&c.plane.d().to_le_bytes());
    hash.write(&c.deviation.to_bits().to_le_bytes());
}

fn corpus() -> Sha256 {
    let mut rng = SplitMix64::new(Seed(0x434f_4d4d_4954_3031));
    let mut hash = Sha256::new();
    let turn = GridTransform::quarter_turns(Axis::Z, 1);
    let mirror = GridTransform::mirror(Axis::X);
    for _ in 0..2_000 {
        let direction = [0, 1, 2].map(|_| unit_component(&mut rng));
        let edge = [0, 1, 2].map(|_| i128::from(coordinate(&mut rng)));
        let [x, y, z] = [0, 1, 2].map(|_| coordinate(&mut rng));
        let point = P3::new(x, y, z).unwrap();
        let distance = coordinate(&mut rng);
        let free = PlaneClass::along(direction);
        let held = PlaneClass::holding(edge, direction);
        for class in [free, held].into_iter().flatten() {
            for class in [class, class.image(&turn), class.image(&mirror)] {
                if let Ok(c) = class.plane(point, distance, &[point]) {
                    put(&mut hash, &c);
                }
            }
        }
    }
    hash
}

#[test]
fn golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || (corpus().finish(), ()));
    println!("geom-poly commits golden hash: {}", hex(checked.digest.0));
    checked.verdict().unwrap_or_else(|e| panic!("{e}"));
}

/// Chapter 21's thread-count test: the same digest on 1, 2, 3 and 8 threads at once.
#[test]
#[cfg_attr(
    all(target_family = "wasm", not(target_feature = "atomics")),
    ignore = "WebAssembly without the atomics feature (wasm32-wasip1) has no threads: \
              std::thread::scope cannot spawn"
)]
fn golden_hash_is_the_same_on_every_thread_count() {
    let one = hex(corpus().finish());
    for threads in golden::THREAD_COUNTS {
        let digests: Vec<String> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| s.spawn(|| hex(corpus().finish())))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(
            digests.iter().all(|d| *d == one),
            "{threads} threads: {digests:?}"
        );
    }
}
