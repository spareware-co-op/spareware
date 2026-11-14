// SPDX-License-Identifier: Apache-2.0
//! The replay entry of piece G231: a fixed night of generated sketch cases, with the kernel's
//! diagnosis of each and the exact rank of its Jacobian, hashed with SHA-256, must give the same
//! digest on every target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus: 120 cases from the six families of `geom_test::oracle::families` (every sketch's
//! `sketch v1` text and its expected, kernel and exact-rank counts) so that a change to a family, to
//! the kernel's diagnosis or to the elimination moves the digest.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::fmt::Write;

use geom_test::Seed;
use geom_test::exact_rank::{bareiss_rank, jacobian};
use geom_test::golden::{self, hex, sha256};
use geom_test::oracle::families::{cases, replay_text};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-test/sketch-families";

fn corpus() -> Vec<u8> {
    let night = cases(Seed(0x0231_0231), 120);
    let mut text = replay_text(&night).unwrap();
    for case in &night {
        match jacobian(&case.job) {
            Ok(rows) => {
                let rank = bareiss_rank(&rows).unwrap();
                writeln!(
                    text,
                    "exact {} rows {} rank {} bits {} products {} pivots {:?}",
                    case.job.id,
                    rows.len(),
                    rank.rank,
                    rank.bits,
                    rank.products,
                    rank.pivots
                )
                .unwrap();
            }
            Err(_) => writeln!(text, "exact {} none", case.job.id).unwrap(),
        }
    }
    text.into_bytes()
}

#[test]
fn sketch_families_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    println!(
        "geom-test sketch-families corpus: {} bytes, golden hash {}",
        checked.value.len(),
        hex(sha256(&checked.value))
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target writes, orders or computes differently \
             (compare this log's digest with another target's), or a definition changed in place, \
             which chapter 26.2 forbids"
        )
    });
}
