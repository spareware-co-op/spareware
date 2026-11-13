//! The body records' fuzz target (piece G94), as tests: `bodies_fuzz` is bolero's test mode,
//! which cargo-bolero and `ClusterFuzzLite` select by name, and `bodies_seeded` the seeded leg
//! that runs on every target, WebAssembly included (the crate README sets out the layout). Each
//! hands the fuzzer's raw bytes to `geom_fuzz::bodies::harness`. The corpus starts from the bodies
//! `geom-format`'s body tests store: a cube, a sheet, a wall and a body of many faces, each framed
//! by `geom_fuzz::bodies::encode`.

#![allow(clippy::panic, reason = "a broken law must stop the test loudly")]

mod common;

use common::holds;

use geom_fuzz::bodies::harness;
use geom_fuzz::seeded;
use geom_test::Seed;

/// Random byte strings per `cargo test` run of the bolero leg.
const ITERATIONS: usize = 5_000;

/// Byte strings in the seeded leg.
const SEEDED_CASES: u32 = 5_000;

/// The longest byte string the seeded leg draws; the harness reads all of it.
const MAX_LEN: u16 = 512;

#[cfg_attr(
    target_family = "wasm",
    ignore = "bolero's test mode reads its corpus folders from the host and catches panics, \
              which WASI cannot; the seeded leg runs the same harness there"
)]
#[test]
fn bodies_fuzz() {
    println!(
        "bolero test mode: corpus and crash files, then {ITERATIONS} random inputs whose seeds \
         bolero draws from the operating system; a failing input's seed is printed as \
         BOLERO_RANDOM_SEED=<seed>, which replays it"
    );
    bolero::check!()
        .with_iterations(ITERATIONS)
        .with_max_len(usize::from(MAX_LEN))
        .for_each(|data: &[u8]| holds(harness(data)));
}

/// The first finding (crash-3b985045191d43eb2bb86ed918c8fe2817ff930d, and the scheduled campaign's
/// crash-ea1e9ccf374666057eff8d9be7e2fbe0da8949b5), cut to an index of one table that lists one
/// chunk by the ID `[alg, h'']`: an algorithm other than SHA-256 with an empty digest, which the
/// format allows (24.2). Relinking the chunk searched the index for that empty digest, and a search
/// for an empty window panics. The body is refused, and that is all.
#[test]
fn an_id_with_an_empty_digest_is_a_case() {
    for alg in [0x00, 0x02, 0x17] {
        // `[1, 2, [[8, 8, [[alg, h'']]]], ...]`: the five fields' first three, all `chunk_ids` reads.
        let index: [u8; 11] = [
            0x85, 0x01, 0x02, 0x81, 0x83, 0x08, 0x08, 0x81, 0x82, alg, 0x40,
        ];
        let mut data = vec![11, 0];
        data.extend(index);
        data.extend([0, 0]);
        holds(harness(&data));
    }
}

#[test]
fn bodies_seeded() -> Result<(), seeded::Failure> {
    seeded::run(
        "bodies fuzz target, seeded",
        Seed::DEFAULT,
        SEEDED_CASES,
        MAX_LEN,
        harness,
    )
}
