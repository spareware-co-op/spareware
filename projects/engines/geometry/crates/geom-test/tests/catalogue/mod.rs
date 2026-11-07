// SPDX-License-Identifier: Apache-2.0
//! What the law catalogue's files for booleans, mass properties, names, formats and numbers share
//! (chapter 28.3; piece G155): a seeded draw, a report of what a law found, and flat solids drawn
//! as the kernel's bodies and the flat reference's solids ([`flat`]).
//!
//! Every law prints `seed: <hex>` and replays with `SPAREKIT_SEED`; pull requests run the fixed
//! default seed given to [`seed`], and the nightly workflow a fresh one (`kernel-nightly.yml`).

#![allow(
    dead_code,
    clippy::unwrap_used,
    reason = "each law binary uses part of this module, and a failure stops the test"
)]

pub mod flat;

use core::num::NonZeroU64;

use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

/// The draws of one law, from the seed the run gives it.
pub struct Draw(pub SplitMix64);

impl Draw {
    /// The generator for the law `label`, from `SPAREKIT_SEED` or else `default`.
    pub fn new(label: &str, default: u64) -> Draw {
        Draw(SplitMix64::new(resolve(label, Seed(default)).unwrap()))
    }

    /// A number below `n`, which must be above zero.
    pub fn below(&mut self, n: u64) -> u64 {
        self.0.below(NonZeroU64::new(n).unwrap())
    }

    /// A number in `lo..=hi`.
    pub fn between(&mut self, lo: i64, hi: i64) -> i64 {
        let span = u64::try_from(hi - lo + 1).unwrap();
        lo + i64::try_from(self.below(span)).unwrap()
    }

    /// True about once in `n`.
    pub fn one_in(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }

    /// An index into a slice of `len` items.
    pub fn pick(&mut self, len: usize) -> usize {
        usize::try_from(self.below(u64::try_from(len).unwrap())).unwrap()
    }

    /// Bytes.
    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len)
            .map(|_| u8::try_from(self.below(256)).unwrap())
            .collect()
    }
}

/// What a law found: the first failure of each case, and how many there were.
#[derive(Default)]
pub struct Findings {
    first: Vec<(String, String, usize)>,
    /// How many cases the law ran.
    pub cases: usize,
}

impl Findings {
    /// Counts a case run.
    pub fn ran(&mut self) {
        self.cases += 1;
    }

    /// A failure of `case`, described by `detail` the first time.
    pub fn add(&mut self, case: &str, detail: impl FnOnce() -> String) {
        if let Some(f) = self.first.iter_mut().find(|f| f.0 == case) {
            f.2 += 1;
        } else {
            self.first.push((case.to_owned(), detail(), 1));
        }
    }

    /// Checks `holds`, adding a failure of `case` if not.
    pub fn check(&mut self, holds: bool, case: &str, detail: impl FnOnce() -> String) {
        if !holds {
            self.add(case, detail);
        }
    }

    /// Fails with every case found, if any, and if the law ran fewer than `least` cases.
    pub fn assert_none(&self, label: &str, least: usize) {
        let lines: Vec<String> = self
            .first
            .iter()
            .map(|(c, d, n)| format!("- {c} ({n} times), first: {d}"))
            .collect();
        assert!(
            self.first.is_empty(),
            "{label}: {} broken:\n{}",
            self.first.len(),
            lines.join("\n")
        );
        assert!(
            self.cases >= least,
            "{label}: only {} cases ran, {least} wanted",
            self.cases
        );
    }
}
