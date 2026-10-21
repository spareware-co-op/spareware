//! Every benchmark's inputs and results, pinned: their SHA-256 digests must equal the values in
//! `src/golden.txt`.
//!
//! The benchmarked kernels are the functions checked here, so a change that makes a benchmark
//! faster by computing something else fails this test (chapter 23.13, "Output hash"), and so does
//! a change to a batch. The digests also pin determinism: they are recorded on one platform and
//! must match on every other, in every build profile, and on wasm32 (the fuel gate checks them
//! there).
//!
//! When digests change on purpose (a deliberate change to `geom-num`'s results, or new or changed
//! inputs), the failure message prints the whole new `golden.txt`, ready to replace the old one.
//!
//! Guarantees, when these tests pass: `golden.txt` has exactly one entry per catalogued benchmark,
//! in catalogue order, and every entry equals the digests this build computes; no two benchmarks
//! share a batch; and a mismatch is reported as a change of inputs or of results.
//!
//! Method: golden-master (characterization) testing
//! (<https://en.wikipedia.org/wiki/Characterization_test>) over SHA-256 digests of canonical
//! encodings (NIST FIPS 180-4, <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>).
//!
//! Cost: one pass and one pair of digests per benchmark.

use std::collections::BTreeSet;

use geom_bench::catalogue::{names, prepare_named};
use geom_bench::golden::{self, GOLDEN};

#[test]
fn every_digest_matches_its_recorded_value() {
    let actual: Vec<(String, geom_bench::harness::Digests)> = names()
        .into_iter()
        .map(|name| {
            let digests = prepare_named(&name)
                .expect("every catalogued name prepares")
                .run();
            (name, digests)
        })
        .collect();
    let recorded = golden::parse(GOLDEN).expect("golden.txt parses");
    let same = recorded.len() == actual.len()
        && recorded
            .iter()
            .zip(&actual)
            .all(|(r, (name, d))| r.name == name && r.digests == *d);
    if !same {
        let changed: Vec<&str> = actual
            .iter()
            .filter(|(name, d)| !recorded.iter().any(|r| r.name == name && r.digests == *d))
            .map(|(name, _)| name.as_str())
            .collect();
        let gone: Vec<&str> = recorded
            .iter()
            .filter(|r| !actual.iter().any(|(name, _)| name == r.name))
            .map(|r| r.name)
            .collect();
        let file = golden::render(actual.iter().map(|(name, d)| (name.as_str(), *d)));
        panic!(
            "benchmark inputs or results differ from golden.txt.\nChanged or new: {changed:?}\n\
             No longer catalogued: {gone:?}\nIf the change is intended, replace src/golden.txt \
             with:\n<<<GOLDEN\n{file}GOLDEN>>>"
        );
    }
}

#[test]
fn every_benchmark_has_its_own_inputs() {
    // The gate's teardown and the fuel guest find benchmarks by name, but two benchmarks with the
    // same batch would measure the same thing twice: every input digest is distinct.
    let recorded = golden::parse(GOLDEN).expect("golden.txt parses");
    let inputs: BTreeSet<_> = recorded.iter().map(|r| r.digests.inputs).collect();
    assert_eq!(inputs.len(), recorded.len());
}

#[test]
fn golden_check_reports_what_changed() {
    let name = "predicate/orient2d/typical";
    let mut digests = prepare_named(name).unwrap().run();
    assert_eq!(golden::check(name, &digests), Ok(()));
    let recorded = golden::lookup(name).unwrap();
    digests.outputs = recorded.inputs;
    assert!(
        golden::check(name, &digests)
            .unwrap_err()
            .contains("the results changed")
    );
    digests.inputs = recorded.outputs;
    assert!(
        golden::check(name, &digests)
            .unwrap_err()
            .contains("the inputs changed")
    );
    assert!(
        golden::check("no/such/benchmark", &digests)
            .unwrap_err()
            .contains("no golden")
    );
}
