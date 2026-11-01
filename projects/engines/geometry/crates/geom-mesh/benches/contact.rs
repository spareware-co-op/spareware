//! How fast triangle pairs are tested, by the standard library's clock (piece G151): the
//! done-when's 5M pairs a second, on pairs of small triangles scattered in a room, most of them
//! apart, as a broad phase hands them on. Prints one line per run; the gate is the count benchmark
//! in `geom-bench`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::print_stdout,
    reason = "a benchmark program; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::hint::black_box;
use std::num::NonZeroU64;
use std::time::Instant;

use geom_mesh::contact::{Contact, triangles};
use geom_num::grid::P3;
use geom_test::{Seed, SplitMix64};

fn main() {
    let mut rng = SplitMix64::new(Seed(0x151));
    let room = NonZeroU64::new(1 << 12).unwrap();
    let small = NonZeroU64::new(1 << 11).unwrap();
    let mut pairs = Vec::with_capacity(1 << 16);
    for _ in 0..1 << 16 {
        let mut triangle = || {
            let at = [rng.below(room), rng.below(room), rng.below(room)];
            let mut corner = || {
                let off = |v: u64, r: &mut SplitMix64| (v + r.below(small)) as i64;
                P3::new(
                    off(at[0], &mut rng),
                    off(at[1], &mut rng),
                    off(at[2], &mut rng),
                )
                .unwrap()
            };
            [corner(), corner(), corner()]
        };
        let (a, b) = (triangle(), triangle());
        pairs.push((a, b));
    }
    let rounds = 64;
    let start = Instant::now();
    let mut met = 0_u64;
    for _ in 0..rounds {
        for (a, b) in &pairs {
            if let Ok(Contact::Touch | Contact::Cross) = triangles(black_box(*a), black_box(*b)) {
                met += 1;
            }
        }
    }
    let time = start.elapsed();
    let tests = rounds * pairs.len();
    println!(
        "{tests} triangle pairs in {time:?}: {:.1}M a second, {met} met",
        tests as f64 / time.as_secs_f64() / 1e6
    );
}
