//! `replay-hashes`: one line per seeded operation log, for reference mode's comparison (23.13;
//! piece G106).
//!
//! ```text
//! cargo run --release -p geom-fuzz --bin replay-hashes > normal.txt
//! cargo run --release -p geom-fuzz --features reference --bin replay-hashes > reference.txt
//! diff normal.txt reference.txt
//! ```
//!
//! What it does: draws [`LOGS`] byte strings from a fixed seed, decodes each as a `sequence` case,
//! runs it, replays its log and prints the log's hash and the result's measures. The hash covers
//! every operation's bodies, names, history, exactness and diagnostics (28.13), so two builds that
//! print the same lines gave the same answers. Golden work counts join with G98.
//!
//! Guarantees: the same lines on every run of a build: the seed is a constant, and nothing is read
//! from the world. Exit code 0 unless a line cannot be written.
//!
//! Method: chapter 23.13's reference mode, with chapter 28.5's sequence target as its corpus
//! (`projects/engines/geometry/architecture/23-performance.md`).
//!
//! Cost: one run of each log. Recursion: none.

use geom_fuzz::sequence::{decode, replay_line};
use geom_test::{Seed, SplitMix64};

/// How many logs it prints.
const LOGS: usize = 2_000;

/// The longest byte string it draws.
const MAX_LEN: u64 = 600;

fn main() {
    let mut rng = SplitMix64::new(geom_test::seed::fixed("replay hashes", Seed::DEFAULT));
    let mut printed = 0usize;
    for _ in 0..LOGS {
        let Some(len) = core::num::NonZeroU64::new(MAX_LEN) else {
            return;
        };
        let mut data = vec![0u8; usize::try_from(rng.below(len)).unwrap_or(0)];
        rng.fill_bytes(&mut data);
        let (case, _) = decode(&data);
        if let Some(line) = replay_line(&case) {
            println!("{line}");
            printed = printed.saturating_add(1);
        }
    }
    eprintln!("{printed} logs");
}
