//! `geom-bench-steps`: prints chapter 23.2's step mix, one row per corpus.
//!
//! Guarantees: the table is a pure function of the build target ([`geom_bench::steps`]).
//!
//! Method: the tally of `geom_bench::steps`, over the `_step` twins of `geom-num`'s predicates
//! (<https://doi.org/10.1145/2660193.2660195> for the generator that draws its corpora).
//!
//! Cost: a few thousand predicate calls per corpus.

use geom_bench::steps::mix;
use geom_num::Step;

fn main() {
    let names: Vec<&str> = Step::ALL.iter().map(|s| s.name()).collect();
    println!("| Corpus | Calls | {} |", names.join(" | "));
    println!("| --- | --- |{}", " --- |".repeat(names.len()));
    for row in mix() {
        let counts: Vec<String> = row.counts.iter().map(u64::to_string).collect();
        println!(
            "| {} | {} | {} |",
            row.corpus,
            row.calls(),
            counts.join(" | ")
        );
    }
}
