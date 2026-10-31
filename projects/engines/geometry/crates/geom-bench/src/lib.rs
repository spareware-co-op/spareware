//! Spare Kit geometry kernel, development only: benchmarks, and the count gates that block a
//! regression over 5%.
//!
//! One job: the kernel's benchmarks, and the count gates that block a regression over 5%.
//!
//! Layer: Development, in chapter 27's crate table: a development crate, never shipped.
//!
//! Development only (`publish = false`): no kernel or program crate depends on this one. It
//! measures every operation of the numbers layer, `geom-num` ([`catalogue::OPS`]): the four exact
//! predicates on typical, degenerate, nearly degenerate and extreme inputs; fixed-width integer
//! arithmetic, comparison and conversion at every width the kernel uses; planes, the points where
//! three planes meet, and committing them to the grid; the error-free transformations; interval
//! arithmetic, with the certified enclosures of the proven maths functions; grid coordinates and
//! grid steps; total order on doubles; exactness classes; the deterministic maths functions, those
//! with proven error bounds and those without; the counter-based random numbers (the Philox4x32-10 block, a
//! stream's draws, and stream ranges); double-double arithmetic; the exact sum of doubles; the
//! interval Newton operators and the Jacobians of the dual numbers; whole root isolations and the
//! double-double balls they fall back to; and chapter 23.3's comparison of exact-path variants (piece G42): every generated exact path in
//! plain `i128` and in width-aware 32-bit limbs beside the shipped one, and the 32-bit-limb
//! integers `Wide32` at the bit widths `Wide` is measured at; with them, [`avnaim`], the reference
//! chapter 23.3 names (Avnaim et al.'s signs), which lives here because no kernel crate ships it.
//! Public functions of `geom-num` without a benchmark of their own
//! are listed with the reason in [`catalogue::UNMEASURED`], and a test keeps that list complete.
//! It also measures the hot path of every phase-2 code piece in the crates above the numbers
//! layer (decision 32.44): `geom-math`'s typed transforms, rational rotation, frames and decimal
//! reader, `geom-linalg`'s pivoted QR and SVD, `geom-bvh`'s box tree and cell index,
//! `geom-nurbs`'s evaluation, interpolation and Bernstein bound, and the curve and surface kinds'
//! evaluation, certified enclosures and conversion to NURBS in `geom-curve` and `geom-surface`,
//! with, from phase 3, `geom-make`'s primitives and transforms,
//! each named in the catalogue with its crate's path and checked to be a public function there.
//!
//! Map: chapter 23.13 (benchmarks and regression gates) and 28.14 (benchmarks in the test suite)
//! of the kernel architecture, `projects/engines/geometry/architecture/`; board entry G18, which
//! this piece (G06) covers.
//!
//! Invariants this crate keeps, each checked by its tests:
//!
//! - Every benchmark runs the operation it is named after (`tests/attack_g06.rs`), on inputs that
//!   are what their class claims (`tests/inputs.rs`).
//! - Inputs and results are pinned by golden digests that must match in every build profile and on
//!   wasm32 (`tests/golden.rs`, and the fuel gate).
//! - The gate, the catalogue and `geom-num`'s public functions agree (`tests/coverage.rs`).
//! - The library has no `unsafe` code, no panicking calls or indexing outside tests, no recursion,
//!   no hash maps of its own (Gungraun's summary model keeps counts in an insertion-ordered
//!   `IndexMap`, which [`callgrind`] only looks up by key, so no result depends on its hashing),
//!   no randomness (inputs come from fixed seeds) and no platform maths.
//!
//! Method: deterministic count gates in place of timing, as chapter 23.13 sets them out: Valgrind's
//! Callgrind and DHAT through Gungraun (<https://github.com/gungraun/gungraun>), Wasmtime's fuel
//! (<https://docs.wasmtime.dev/api/wasmtime/struct.Config.html#method.consume_fuel>), the doubling
//! test for growth (<https://algs4.cs.princeton.edu/14analysis/>), and SHA-256 golden digests of
//! canonical outputs (NIST FIPS 180-4, <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>).
//! Consulted: Gungraun, Apache-2.0 OR MIT,
//!   <https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md>,
//!   for its library-benchmark API and its Callgrind and DHAT settings. No code copied.
//!
//! Cost: a benchmark is one pass of its operation over [`inputs::BATCH`] inputs (two passes under
//! the instruction-count gate, whose first pass warms the caches; one and two runs under the fuel
//! gate, whose difference is one pass). Building a batch takes O([`inputs::BATCH`]) generator draws
//! and hashing it O([`inputs::BATCH`]) SHA-256 blocks, once per benchmark, outside every measured
//! region.
//!
//! # The gates
//!
//! Every pull request that could change a count runs `.github/workflows/kernel-bench.yml`, and so
//! does every such push to `main`, compared with the `main` it replaced. One job checks this crate:
//! formatting, Clippy with this crate's lints, and the tests in release with overflow checks and in
//! debug, warnings as errors. This is a development crate (chapter 27): its lints deny unwrap,
//! expect, `panic!` and unchecked indexing outside tests, as `geom-num`'s do (and `unreachable!`,
//! `todo!` and `unimplemented!`), but not the native arithmetic, lossy casts and disallowed
//! methods that `geom-num` also denies in kernel code. The others each build the merge base and the
//! change, with the toolchain each pins in `toolchain.txt` (the same one unless the change bumps
//! it, and the one `kernel.yml` pins; a test checks), and compare them (chapter 28.14):
//!
//! - **Instructions and cache** (Linux x86-64 and Linux aarch64, each a gate of its own). Gungraun,
//!   formerly iai-callgrind (Apache-2.0 OR MIT,
//!   <https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md>),
//!   runs every benchmark under Valgrind's Callgrind
//!   (<https://valgrind.org/docs/manual/cl-manual.html>), which executes the program on a
//!   simulated processor and counts what it does. The gate fails if any benchmark's instructions
//!   or estimated cycles rise by more than [`GATE_PERCENT`] on either processor, or one of its L1
//!   or last-level cache misses (instruction reads, data reads, data writes: [`GATED_METRICS`])
//!   does on a miss count large enough to show it: from 20 up, where one event is within 5%. A
//!   smaller miss count cannot show a 5% change smaller than one event, and code layout alone
//!   moves it by an event or two, so its changes are listed and judged through estimated cycles,
//!   which weight every miss ([`callgrind`] derives the rule and applies it to Gungraun's
//!   machine-readable summaries). The two processors run different instructions, so each compares
//!   only its own two runs, under the same rule. With the fuel gate below,
//!   every benchmark is measured on the three targets chapter 23.3 names, x86-64 and aarch64
//!   natively and wasm32, so each predicate's filtered and exact cost is known on all three
//!   (piece G28; [`inputs::predicate`] says which benchmarks give which).
//! - **Heap** (Linux x86-64 and Linux aarch64). The same benchmarks under Valgrind's DHAT
//!   (<https://valgrind.org/docs/manual/dh-manual.html>), counting only allocations made inside the
//!   measured function: bytes and blocks allocated, bytes live at the heap's peak, and the
//!   largest bytes live at once ([`GATED_HEAP_METRICS`]); a rise over 5% fails. The numbers layer
//!   allocates nothing, so every count is zero and any allocation at all fails the gate.
//! - **WebAssembly fuel** (wasm32). Every benchmark built for `wasm32-wasip1` and run in Wasmtime
//!   with fuel metering, which counts the WebAssembly operators executed and is deterministic
//!   ([`guest`] and the host in `fuel/`); a rise over 5% fails.
//! - **Growth.** Each scale benchmark ([`catalogue::GROWTH`]) runs at n, 2n and 4n, and its fuel must
//!   grow no faster than its stated cost ([`counts::growth`]). The part of a count that does not
//!   grow damps each doubling, so faster growth fails only once it outweighs that part ([`counts`]
//!   gives the threshold); for a benchmark the merge base has, the fuel gate fails the rise anyway.
//! - **Output hash.** Every benchmark's inputs and results are hashed with SHA-256 after every
//!   measured run and checked against `golden.txt` ([`golden`]): no benchmark can get faster by
//!   being wrong, and the wasm32 results must be the native bits. The tests check the same
//!   digests in every build profile.
//! - **Nothing hides.** Every change over [`LIST_PERCENT`] either way is listed in the job's summary
//!   and as an annotation on the pull request's checks, and a benchmark the merge base has but the
//!   pull request lacks fails both count gates unless `retired.txt` names it.
//!
//! The limits are passed by the workflow ([`CALLGRIND_LIMITS`], [`DHAT_LIMITS`] and the fuel
//! comparison's arguments), not read from the tree under test, so a pull request cannot loosen its
//! own gate by editing this crate; the workflow and this crate are reviewed by the kernel owners
//! (`.github/CODEOWNERS`).
//!
//! # How noise is handled
//!
//! Wall-clock time on shared CI machines varies by more than 5% between runs, so no gate times
//! anything. Each counts, and every source of variation in the counts is removed:
//!
//! 1. **Counts, not time.** Callgrind counts instructions and simulates a fixed cache: Gungraun pins a
//!    32 KiB 8-way L1 for instructions and for data and an 8 MiB 16-way last level, whatever the
//!    real processor has. Wasmtime's fuel counts WebAssembly operators. For a given binary every
//!    count is the same on every run, so one run per benchmark is enough.
//! 2. **Same job, same machine, same compiler.** Each gate builds the merge base and the pull
//!    request in one job, with the toolchain pinned in `toolchain.txt`, and compares only those two
//!    runs. Library code that picks an implementation by processor picks the same one for both.
//! 3. **A warm, steady-state measurement.** The measured function runs the kernel twice and
//!    Callgrind's costs are zeroed at the start of each pass, so the counts are those of the second
//!    pass, which runs code and touches data the first pass already brought into the simulated
//!    caches ([`harness`] has the details). No count then depends on where the linker placed code
//!    or the allocator placed data: every line the counted pass touches was touched by the first
//!    pass, so a benchmark whose work fits the L1 caches counts almost no misses. The few left, one
//!    to three on some benchmarks, still depend on which cache lines the code falls on, and move by
//!    an event or two when a change elsewhere shifts the layout; counts that small are judged
//!    through estimated cycles, not on their own ([`callgrind`]).
//!    The gate's build aligns every function to a 64-byte cache line
//!    (`-C llvm-args=-align-all-functions=6`), so the little code run only once, the returns at the
//!    end, sits on lines already fetched.
//!    It also builds every crate as one codegen unit (`CARGO_PROFILE_BENCH_CODEGEN_UNITS=1`, and
//!    `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1` for the fuel guest). In Cargo's default 16, how
//!    rustc splits this crate into units moves whenever a benchmark module is added, and which
//!    functions are inlined moves with it, in benchmarks the change never touched (chapter 23.13).
//! 4. **Fixed addresses and environment.** Gungraun runs Valgrind with address-space randomisation
//!    off and the environment cleared.
//! 5. **Only the kernel is counted.** Building the inputs, hashing the results and freeing memory
//!    happen outside the measured function; each benchmark's state is allocated before it and never
//!    freed inside it, and the kernels allocate nothing ([`kernels`]).
//! 6. **Single-threaded.** No benchmark starts a thread, so scheduling cannot change a count; there
//!    is no thread pool to switch off.
//!
//! What is left is real: a change in the kernel or in how the compiler laid it out.
//!
//! # The loop around each operation
//!
//! Each benchmark runs its operation on [`inputs::BATCH`] inputs in a loop that loads each input,
//! passes it through `black_box` and stores the result. Those instructions are the same before and
//! after a change, so they dilute a regression in the cheapest operations rather than hide it: a
//! change that adds x% to the operation alone adds x · op / (op + loop) percent to the count, and
//! the gate trips when the operation alone rises by 5% · (op + loop) / op.
//!
//! Measured in WebAssembly fuel, the loop costs about 27 to 33 per input, by the sizes of the input
//! and the result. With the call replaced by its argument, or by a constant result, 64 inputs cost
//! 1,719 for `eft/split` (one double in, two out), 1,783 for `math/abs` and `predicate/orient2d`
//! (a `Result` out) and 2,103 for `eft/two_sum`, `eft/fast_two_sum` and `interval/neg` (two
//! doubles in and out). So, with the counts of the numbers layer as it is now, the gate trips at a
//! rise in the operation itself of about 5.3% for `orient2d` (37,056 for 64 inputs), 9.9% for
//! `split` (3,479), 10% for `fast_two_sum` (4,224), 11% for `two_sum` (3,816) and 46% for interval
//! negation (2,359), whose operation is about 4 per input against a loop of 33; the absolute value
//! (3,456: the operation, its two finiteness tests and its `Result` cost about 26 per input) trips
//! it when it gains three per input. Every change over 1% of a count is listed for review, which
//! for interval negation is a change of about 9% in the operation. The barrier is kept on every
//! input anyway: without it the same loops cost about 17 to 24 per input (1,113 to 1,518 for 64,
//! measured the same way), but the compiler may vectorise cheap operations across inputs, which no
//! caller in the kernel does, and the count would no longer be the cost of a call.
//!
//! # Running the benchmarks locally
//!
//! The checks of inputs, results and the gates' wiring run everywhere:
//!
//! ```text
//! cargo test -p geom-bench --release
//! ```
//!
//! Wall clock, on any platform (time per batch and inputs per second; never a gate):
//!
//! ```text
//! cargo bench -p geom-bench --bench wall_clock
//! cargo bench -p geom-bench --bench wall_clock -- orient   # only names matching a pattern
//! ```
//!
//! WebAssembly fuel and growth, on any platform with the `wasm32-wasip1` target installed
//! (`rustup target add wasm32-wasip1`). Counts go to standard output, the golden digests are
//! checked on wasm32 for every benchmark, and the growth check reads the counts:
//!
//! ```text
//! CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 cargo build --release -p geom-bench --bin geom-bench-guest \
//!     --target wasm32-wasip1
//! cargo run --release --manifest-path projects/engines/geometry/crates/geom-bench/fuel/Cargo.toml -- target/wasm32-wasip1/release/geom-bench-guest.wasm > fuel.txt
//! cargo run --release -p geom-bench --bin geom-bench-counts -- growth fuel.txt
//! ```
//!
//! Instruction counts, cache misses and heap, on Linux, x86-64 or aarch64 (on Windows use WSL 2;
//! recent macOS has no Valgrind). Install Valgrind and the Gungraun runner, whose version must
//! equal the `gungraun` version in `Cargo.lock` ([`GUNGRAUN_VERSION`]), and build with the gate's
//! flags:
//!
//! ```text
//! sudo apt-get install valgrind
//! cargo install --locked gungraun-runner --version 0.20.0
//! RUSTFLAGS="-C llvm-args=-align-all-functions=6" CARGO_PROFILE_BENCH_CODEGEN_UNITS=1 \
//!     cargo bench -p geom-bench --bench instructions
//! ```
//!
//! To compare a change with `main` as CI does, save a baseline on `main` and compare the branch
//! with it, passing the gate's limits and saving Gungraun's summaries. A rise over 5% in
//! instructions, estimated cycles or heap ends the run with `Gungraun result: Regressed`, the list
//! of rises, and a failed bench (the harness exits with code 3); every Callgrind count is then
//! judged from the summaries, and `geom-bench-counts` exits with 1 on a rise over 5% in
//! instructions, in estimated cycles, or in a miss count of 20 or more:
//!
//! ```text
//! export RUSTFLAGS="-C llvm-args=-align-all-functions=6"
//! export CARGO_PROFILE_BENCH_CODEGEN_UNITS=1
//! git switch main
//! rm -rf target/gungraun
//! cargo bench --no-default-features -p geom-bench --bench instructions -- --save-baseline=main
//! git switch my-branch
//! cargo bench --no-default-features -p geom-bench --bench instructions -- --baseline=main \
//!     --save-summary=json --callgrind-limits='ir=5%,estimatedcycles=5%' \
//!     --dhat-limits='totalbytes=5%,totalblocks=5%,attgmaxbytes=5%,maximumbytes=5%'
//! cargo bench --no-default-features -p geom-bench --bench instructions -- --list \
//!     | sed -nE 's/^instructions::([a-z0-9_]+)::([a-z0-9_]+)::([a-z0-9_]+): benchmark$/\1\/\2\/\3/p' \
//!     > names.txt
//! cargo run --release -p geom-bench --bin geom-bench-counts -- callgrind \
//!     target/gungraun/geom-bench/instructions names.txt 5 "Instructions, cycles and misses"
//! ```
//!
//! The measured runs leave out the `judge` feature (`--no-default-features`), as CI's do, so the
//! benchmark binary is the same whether or not the judge is built (`Cargo.toml` says why).
//!
//! Pass `--bench instructions` or `--bench wall_clock`: the two harnesses take different options.
//! On other platforms `benches/instructions` only prints a note and exits.
//!
//! # Changing the benchmarks
//!
//! - **Adding an operation or class:** add its inputs to [`inputs`], its kernel to [`kernels`],
//!   its entry to [`catalogue::OPS`] and its `#[bench::…]` line to
//!   `benches/instructions/gate.rs`; `tests/coverage.rs` fails until the gate and the catalogue
//!   agree, and `tests/golden.rs` prints the new `golden.txt` to record. A new public function in
//!   `geom-num` fails `tests/coverage.rs` until it has a benchmark or a reasoned entry in
//!   [`catalogue::UNMEASURED`].
//! - **A golden digest changed:** the inputs or the results of a benchmark changed. If that is
//!   intended (a deliberate change to `geom-num` or to a batch), replace `src/golden.txt` with the
//!   file `tests/golden.rs` prints, and say why in the pull request. Otherwise it is a bug.
//! - **Renaming or removing a benchmark:** add its old name to `retired.txt`, or both count gates
//!   fail. Changing a benchmark's inputs changes its counts; say so in the pull request.
//!
//! # Toolchain and dependency updates: re-recording the baselines
//!
//! There are no stored baselines: every run measures the merge base afresh, so a baseline is
//! "re-recorded" by merging the change that moves it. The procedure (chapter 23.13, "Toolchain
//! bumps"):
//!
//! 1. Change only `toolchain.txt` (a toolchain bump) or only `Cargo.lock` (a dependency bump) in a
//!    pull request of its own. The workflow builds the merge base with its own pinned toolchain and
//!    the pull request with the new one, so the comparison lists every change the update causes;
//!    a toolchain bump that changes anything else in the kernel fails, so the effects never mix.
//! 2. The kernel owners review the listed changes. Rises over 5% still fail the gate: an update
//!    that makes the kernel slower needs a fix to the kernel first, or an owner decision recorded
//!    in the decision log (chapter 32) before the gate is changed.
//! 3. Once merged, the new toolchain or dependency is the merge base of every later pull request.
//!
//! # Not here yet
//!
//! Chapter 23.13's other gates measure what later pieces build: logical bytes and golden work
//! counts need the work meter (G23), reference mode needs caches and filters to switch off, and
//! scenario benchmarks need the operations above the numbers layer. The nightly wall-clock gate
//! on the reference runner is G107.

#![forbid(unsafe_code)]

pub mod avnaim;
// Unix and Windows only, with the `judge` feature (on by default): Gungraun's summary model does not
// build for WebAssembly, and the gate's measured runs leave it out (Cargo.toml).
#[cfg(all(feature = "judge", any(unix, windows)))]
pub mod callgrind;
pub mod canonical;
pub mod catalogue;
pub mod counts;
pub mod golden;
pub mod guest;
pub mod harness;
pub mod inputs;
pub mod kernels;
pub mod rng;
pub mod sha256;

/// The gate: the largest rise, in percent, that any gated measure of any benchmark may show
/// against the merge base. Principle 12 of the kernel architecture sets it at 5.
pub const GATE_PERCENT: f64 = 5.0;

/// Changes larger than this, in percent, either way, are listed on the pull request (chapter
/// 23.13: "Every change over 1% in the deterministic gates is listed on the pull request").
pub const LIST_PERCENT: f64 = 1.0;

/// The Callgrind measures the gate judges, by their Gungraun names: instructions executed and
/// estimated cycles, gated at [`GATE_PERCENT`] on every benchmark, and the L1 and last-level misses
/// of instruction reads, data reads and data writes, gated at [`GATE_PERCENT`] where the merge
/// base's count resolves it and otherwise judged through estimated cycles (chapter 23.13,
/// "Instructions and cache"; [`callgrind::JUDGED`] holds the same names).
pub const GATED_METRICS: [&str; 8] = [
    "Ir",
    "EstimatedCycles",
    "I1mr",
    "D1mr",
    "D1mw",
    "ILmr",
    "DLmr",
    "DLmw",
];

/// The DHAT measures the gate applies [`GATE_PERCENT`] to, by their Gungraun names: bytes and
/// blocks allocated, bytes live at the heap's peak, and the largest bytes live at once (chapter
/// 23.13, "Memory").
pub const GATED_HEAP_METRICS: [&str; 4] =
    ["TotalBytes", "TotalBlocks", "AtTGmaxBytes", "MaximumBytes"];

/// The measures of [`GATED_METRICS`] that Gungraun gates itself, instructions and estimated cycles,
/// at [`GATE_PERCENT`], as the workflow passes them to Gungraun (`--callgrind-limits`, which
/// replaces the limits `benches/instructions/gate.rs` sets); a test checks the two agree. The miss
/// counts are left out: a percentage limit cannot leave alone the counts too small to resolve it,
/// so [`callgrind::judge`] gates them from Gungraun's summaries.
pub const CALLGRIND_LIMITS: &str = "ir=5%,estimatedcycles=5%";

/// [`GATED_HEAP_METRICS`] at [`GATE_PERCENT`], as the workflow passes them to Gungraun
/// (`--dhat-limits`); a test checks the two agree.
pub const DHAT_LIMITS: &str = "totalbytes=5%,totalblocks=5%,attgmaxbytes=5%,maximumbytes=5%";

/// The version of the Gungraun library the gate is built with. The `gungraun-runner` binary must
/// have exactly this version; CI reads the version from `Cargo.lock` instead, so this constant
/// is the one to keep in step with `Cargo.toml` for local runs (a test checks they agree).
pub const GUNGRAUN_VERSION: &str = "0.20.0";

/// The version of Wasmtime the fuel host (`fuel/`) is built with; its fuel model defines the
/// counts, so it changes only in a pull request of its own (a test checks `fuel/Cargo.toml`).
pub const WASMTIME_VERSION: &str = "49.0.0";

/// The Rust toolchain both trees are built with, from `toolchain.txt`.
pub const TOOLCHAIN: &str = include_str!("../toolchain.txt").trim_ascii();
