//! The count gate's benchmark of EMBER's pair 827 (Gate 4, G156): `mesh/ember/pair_827`, the
//! boolean EMBER benchmarks, on two of its own models, under Valgrind's Callgrind and DHAT through
//! Gungraun, held to the same 5% gate as every catalogued benchmark.
//!
//! It is the instruction gate's last part (`kernel-bench.yml`, the job `instructions-base`'s
//! comment): its models are two files of the fetched corpus `ember-cleared-pairs`, which only that
//! part fetches, each checked against its SHA-256 in the corpus register, so no other bench or test
//! job fetches anything. Every part lists this benchmark with the catalogue's, so the judge refuses
//! the run if no part measured it. `pair.rs` holds the batch, the kernel and the golden check.
//!
//! The measurement is the catalogue's: the measured function runs `Bench::measure`, two passes
//! with Callgrind's costs zeroed at the start of each (`geom_bench::harness`), and the teardown
//! checks the golden digests and ends the run with a failure if they differ. The tools and limits
//! are `gate.rs`'s ([`config`], which `tests/coverage.rs` keeps equal to it), with the folder of
//! the models passed through Gungraun's cleared environment.
//!
//! Valgrind runs on Linux here, so on other platforms this program only prints a note.
//!
//! Identifiers are `ember::mesh::ember pair_827`; the run's list names it `mesh/ember/pair_827`.

#[cfg(target_os = "linux")]
mod pair;

#[cfg(target_os = "linux")]
use gate::mesh::mesh;

/// The gate's tools and limits, exactly `gate.rs`'s `config` (`tests/coverage.rs` checks the two
/// agree), with [`pair::FOLDER`] passed through to the measured process. The workflow passes its
/// own limits on the command line, which replace these.
#[cfg(target_os = "linux")]
pub fn config() -> gungraun::LibraryBenchmarkConfig {
    use geom_bench::GATE_PERCENT;
    use geom_bench::harness::ZERO_BEFORE;
    use gungraun::{Callgrind, Dhat, DhatMetric, EventKind, LibraryBenchmarkConfig};

    let mut callgrind = Callgrind::default();
    callgrind.args([ZERO_BEFORE]);
    callgrind.soft_limits([
        (EventKind::Ir, GATE_PERCENT),
        (EventKind::EstimatedCycles, GATE_PERCENT),
        (EventKind::I1mr, GATE_PERCENT),
        (EventKind::D1mr, GATE_PERCENT),
        (EventKind::D1mw, GATE_PERCENT),
        (EventKind::ILmr, GATE_PERCENT),
        (EventKind::DLmr, GATE_PERCENT),
        (EventKind::DLmw, GATE_PERCENT),
    ]);
    let mut dhat = Dhat::default();
    dhat.soft_limits([
        (DhatMetric::TotalBytes, GATE_PERCENT),
        (DhatMetric::TotalBlocks, GATE_PERCENT),
        (DhatMetric::AtTGmaxBytes, GATE_PERCENT),
        (DhatMetric::MaximumBytes, GATE_PERCENT),
    ]);
    let mut config = LibraryBenchmarkConfig::default();
    config.tool(callgrind);
    config.tool(dhat);
    config.pass_through_env(pair::FOLDER);
    config
}

/// The benchmark, in its group's module as `gate.rs` holds each of the catalogue's.
#[cfg(target_os = "linux")]
mod gate {
    /// EMBER's pair, in the group `mesh`.
    pub mod mesh {
        use std::process::exit;

        use geom_bench::harness::Named;
        use gungraun::{library_benchmark, library_benchmark_group};

        use crate::pair;

        /// The benchmark named `name`, built and leaked, so the measured function frees nothing of
        /// it. A missing or refused model ends the run with a failure.
        fn leak(name: &'static str) -> &'static mut Named {
            match pair::prepare() {
                Ok(bench) => Box::leak(Box::new(Named { name, bench })),
                Err(message) => {
                    eprintln!("geom-bench ember: {message}");
                    exit(2)
                }
            }
        }

        /// After the measured function: the golden digests, or the run ends with a failure.
        fn verify(run: &'static mut Named) {
            if let Err(message) = pair::check(&run.bench.digests()) {
                eprintln!("geom-bench gate: golden digests differ: {message}");
                exit(1);
            }
        }

        #[library_benchmark(teardown = verify)]
        #[bench::pair_827(leak(pair::NAME))]
        fn ember(run: &'static mut Named) -> &'static mut Named {
            run.bench.measure();
            run
        }

        library_benchmark_group!(
            name = mesh;
            benchmarks = ember
        );
    }
}

#[cfg(target_os = "linux")]
gungraun::main!(
    config = config();
    library_benchmark_groups = mesh
);

/// Off Linux: explain where the gate runs, and succeed.
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!(
        "geom-bench: the instruction-count gate needs Valgrind, which runs on Linux (use WSL 2 on \
         Windows); EMBER's pair 827 is measured there too"
    );
}
