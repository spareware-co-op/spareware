# G33: the written audit of rayon 1.12.0

*Piece G33, Spare Kit geometry kernel. The audit chapter 29.20 (rule 3) requires before a crate ships, here rayon and the crates it brings, which only `geom-pool` may name (29.4; `deny-ships.toml`). Written 2026-10-26 by Claude Opus 5.5 for the owners.*

## The answer in plain words

rayon gives `geom-pool` its threads on native targets. `geom-pool` uses three of its calls: build a pool of a fixed number of threads, run two closures on it with `join`, and ask the pool its size. Behind those sit rayon's own pool, `rayon-core`, and three crossbeam crates that carry its work-stealing queues.

They are fit to ship in `geom-pool`, with these things to know:

- **`unsafe` is everywhere in them**, by design: a thread pool and lock-free queues cannot be written without it. `geom-pool` itself has none (`forbid(unsafe_code)`). rayon's soundness rests on its scope waiting for every job before it returns, which its documentation promises and `geom-pool`'s tests attack.
- **Scheduling is not deterministic, and answers are.** Which thread runs which job differs from run to run (rayon picks steal victims with a random generator). `geom-run`'s helper never lets that reach an answer: each task writes only its own slot, and the helper reads the slots in index order. `tests/attack_g33.rs` checks every helper's answer against the serial one at 1, 2, 3 and 8 threads.
- **A panic in a job is passed on**, never lost, when the job was spawned into a scope, which is all `geom-pool` does. rayon aborts the process only if its own internal code panics, a rayon bug.
- **They allocate**, a small box per spawned job, and they keep some global state: rayon's global pool, which only rayon's global entry points use and `clippy.toml` bans in kernel crates; thread-locals; and crossbeam's global memory reclaimer. None of it can change an answer.
- **Licences:** all five are MIT OR Apache-2.0, recorded as MIT in the gate. No file carries a third-party notice.
- **Advisories:** cargo-deny 0.20.2 reports none for `geom-pool`'s graph (run 2026-10-26).

## 1. What was audited

| Crate | Version | Checksum (`Cargo.lock`) | Source commit (`.cargo_vcs_info.json`) | Size |
| --- | --- | --- | --- | --- |
| rayon | 1.12.0 | `fb39b166781f92d482534ef4b4b1b2568f42613b53e5b6c160e24cfbfa30926d` | [rayon-rs/rayon `c9ced185`](https://github.com/rayon-rs/rayon/tree/c9ced185ae3508246a9eb70c8407a1199bb1b77f) | 100 files, 27,141 lines |
| rayon-core | 1.13.0 | `22e18b0f0062d30d4230b2e85ff77fdfe4326feb054b9783a3460d8435c8ab91` | [rayon-rs/rayon `7af20d76`](https://github.com/rayon-rs/rayon/tree/7af20d7692d5decbcb4adcaa079cd607e3e50814/rayon-core) | 26 files, 7,069 lines |
| crossbeam-deque | 0.8.8 | `622f3fc73690be383c7214310406f28a90e6edeadc3cea882f9d71e495b9711a` | [crossbeam-rs/crossbeam `2920c984`](https://github.com/crossbeam-rs/crossbeam/tree/2920c984290229ab4e0ca0452ef09e48a82063f3) | 2 files, 2,367 lines |
| crossbeam-epoch | 0.9.21 | `dc74980687109a3b14c72fd458107bf0baa1da1a1a805e178d15501ba9b86d9d` | the same commit | 12 files, 4,945 lines |
| crossbeam-utils | 0.8.23 | `a31eee39dddec8330830986fcd7625edb5a24ec90ea038215273bbc3adb08ac6` | the same commit | 14 files, 4,168 lines |

rayon 1.12.0 is the version `clippy.toml`'s bans were first checked against (G32). Each crate was read in the published `.crate` Cargo unpacked, whose checksum is the one above. rayon's last dependency, `either`, is already in the workspace through bolero, and its licence is recorded.

## 2. What `geom-pool` calls

`geom_pool::native::Rayon` (G33) calls exactly:

- `rayon::ThreadPoolBuilder::new().num_threads(n).build()`, with n ≥ 1;
- `rayon::ThreadPool::join`, on the pool's own threads, recursively: `run_all` halves the task slice and joins the halves (this replaced `ThreadPool::in_place_scope` and `Scope::spawn`, see the Miri point on scopes below);
- `rayon::ThreadPool::current_num_threads`.

Dropping the pool stops its threads. Everything else rayon offers is unused. Its global-pool entry points (`join`, `scope`, `spawn`, `broadcast`, the parallel iterators, the same on a `ThreadPool`) are banned in every kernel crate by `clippy.toml`, which `geom-pool` is also built with.

So the code reached is `rayon-core`'s thread pool, registry, jobs, latches, scopes, sleep and unwinding modules, and the crossbeam queues and reclaimer beneath them. The `rayon` crate itself contributes only the re-exports.

## 3. Notices

All five crates are MIT OR Apache-2.0, with both licence texts in each package. The MIT texts carry:

- rayon and rayon-core: "Copyright (c) 2010 The Rust Project Developers";
- the crossbeam crates: "Copyright (c) 2019 The Crossbeam Project Developers".

No package has an Apache NOTICE file, and no source file carries another party's copyright line or SPDX identifier. The gate records the choice as MIT for each (`licence::LICENCE_CHOICES`). The generated third-party notices file (piece G481) must carry the MIT texts with those lines.

## 4. `unsafe`

Counted outside comments: rayon 69 uses, rayon-core 88, crossbeam-deque 42, crossbeam-epoch 185, crossbeam-utils 80. None of the five forbids it.

What the code `geom-pool` reaches does with it:

- **Join jobs live on a stack and outlive their borrows on paper, not in fact.** `ThreadPool::join` runs on a worker (`Registry::in_worker`, which injects the call into the pool and blocks the outside caller on a thread-local `LockLatch` when it is not on one). `join_context` (`join/mod.rs`) puts the second closure in a `StackJob` on that worker's own stack, hands a raw `JobRef` to the deque and does not return until the job's latch is set: it pops the job back and runs it itself if no thief took it, and otherwise runs other jobs until the thief finishes. That is the documented contract `geom-pool` relies on (both closures have run when `join` returns), and its tests attack it (every task runs exactly once, before `run_all` returns). `Latch::set` takes a raw `*const Self`, so no reference into the job is live when the waiting thread is released and returns.
- **Jobs are raw pointers on the queues** (`job.rs`, `JobRef`), executed once and freed by the job type that made them.
- **Each worker thread knows itself through a thread-local raw pointer** (`registry.rs`, `WORKER_THREAD_STATE`), set when the thread starts and cleared when it stops.
- **The queues are lock-free.** crossbeam-deque's Chase–Lev work-stealing deques, and crossbeam-epoch's epoch-based reclamation of the buffers they grow out of, are where most of the `unsafe` lives. They are the crossbeam project's core, used across the Rust ecosystem; this audit did not re-prove them.
- **Miri finds one aliasing violation in crossbeam-epoch 0.9.21** (added after G33 merged, from the weekly Miri run's first pass over `tests/attack_g33.rs`). `IsElement::element_of` (`internal.rs`) turns a `&Entry`, a reference to the first field of a thread's `Local` record, into a `&Local` covering the whole record. Stacked Borrows, Miri's default model, refuses it: the `&Entry` grants access to its own 8 bytes only. Tree Borrows, the newer model, allows it. crossbeam fixed it upstream by passing raw pointers instead (crossbeam-rs/crossbeam#871, commit `02fb08a6c`, "epoch: Fix stacked borrows violations"), which changes a public trait, so it ships only in a 0.10 release; 0.9.21, released after it, does not carry it. Until the lock moves, Miri runs `geom-pool`'s tests under Tree Borrows and every other crate's under Stacked Borrows (`.github/workflows/kernel-quality.yml`); geom-test's `kernel_quality_workflow` test fails once the lock leaves 0.9.21, so the exception is looked at again then. That step also turns Miri's leak check off, as rayon's own CI does (`-Zmiri-tree-borrows -Zmiri-ignore-leaks`): dropping a `ThreadPool` signals its threads to stop and does not join them, so each worker's crossbeam-epoch registration is still alive when a test ends.
- **Miri finds one aliasing violation in rayon-core 1.13.0's scopes, which `geom-pool` therefore does not use** (found by the first full kernel-quality run under Tree Borrows; `geom-pool` used `in_place_scope` and `Scope::spawn` until then). A spawned job sets the scope's latch from inside `ScopeBase::execute_job_closure`, while that function's closure argument still holds a `&Scope`. Miri protects that reference until the call returns, but the thread that opened the scope can see the latch first, return and free the scope. Every scope has the race, whether it was opened on a worker or not, and which Miri schedule loses it depends on the seed (run 42288828480, `attack_g33`'s sweep test; 4 of 64 seeds on a two-thread pool with one empty job). Reported as rayon-rs/rayon#1328 and fixed upstream by rayon-rs/rayon#1330 (merged 2026-10-27: the job captures a raw scope pointer and wraps its body in `MaybeUninit`), which no release carries yet. `geom-pool` now runs its tasks with `ThreadPool::join`, which does not have the race (`Latch::set` takes a raw pointer, see the first point above), so nothing is ignored under Miri. A later rayon-core release with #1330 makes scopes usable again; there is no need to go back.

`geom-pool` adds no `unsafe` of its own. Its three calls are safe functions.

## 5. Panics

- **A closure that panics inside `join`** has its panic caught (`unwind::halt_unwinding`) and held while the other closure finishes: if the first one panics, `join_recover_from_panic` waits for the second (running it itself if no thief took it) before resuming the panic; if the second panics, its result is stored in the `StackJob` and resumed once the first returns. With nested joins, every task has therefore run when the panic reaches `run_all`, which is the "pass on a task's panic" the `Pool` trait allows. `tests/attack_g33.rs` checks it, and that the pool still works after.
- **A job spawned outside a scope** (`rayon::spawn`) that panics goes to the pool's panic handler, and with none set rayon aborts the process (`registry.rs`, `catch_unwind`). `geom-pool` never spawns outside a scope, and kernel crates cannot (`clippy.toml`).
- **rayon's own code** runs under `AbortIfPanic` guards in a few places, a job's execution bookkeeping and a worker thread's main loop: a panic there aborts the process. Only a rayon bug reaches them.
- **Building a pool can fail**, if the operating system refuses a thread. `ThreadPoolBuilder::build` reports it, and `geom-pool` returns it as `PoolError::CannotStartThreads`.

## 6. Allocation, global state and the environment

- **Allocation:**
  - one box per spawned job;
  - the queues' buffers, which grow;
  - the registry, behind an `Arc`;
  - each thread's stack.

  None is fallible. Chapter 23.10 already says small allocations cannot fail softly in Rust. `geom-run`'s helper keeps its own large buffers fallible, and spawns one job per chunk, not per item.
- **Global state:**
  - rayon-core's global pool (`static mut THE_REGISTRY`, set once behind a `Once`), made only by the global entry points, which `geom-pool` does not call;
  - the thread-locals `WORKER_THREAD_STATE` and, for a thread outside every pool, `LOCK_LATCH`;
  - a global counter that seeds each worker's steal-victim generator (`XorShift64Star`);
  - crossbeam-epoch's global collector, a `OnceLock`, and its per-thread handle.

  Principle 7 bans such state from the kernel's own crates. Here it changes which thread runs what, never an answer (section 7).
- **Environment:** `RAYON_NUM_THREADS` and the older `RAYON_RS_NUM_CPUS` are read only when no thread count was given (`lib.rs`, `get_num_threads`). `geom-pool` always gives one, so neither variable changes a run.
- **Build scripts:**
  - rayon-core's prints only a rerun line; it exists for `links = "rayon-core"`, which stops two copies of the crate linking into one program;
  - crossbeam-utils' reads the target name to decide whether the target has atomics, and emits `cfg` flags;
  - crossbeam-deque's and crossbeam-epoch's read the target and the sanitizer setting, to turn on a `cfg` for the thread sanitizer.

  None reads anything else.

## 7. Determinism

rayon's schedule is not deterministic. Work stealing picks victims at random, and threads race, so which job runs where and in what order differs from run to run. The kernel's answers do not depend on it, for two reasons:

- `geom-run`'s helper cuts the work into chunks fixed by the chunk plan alone, never by the thread count (23.4).
- Each chunk writes only its own slot, and the helper reads the slots in index order, settling failures by lowest number and budgets by chapter 23.10's rules.

So the pool decides only how fast the answer comes. `geom-pool`'s `tests/attack_g33.rs` checks every helper on seeded inputs: `map`, `try_map` with budgets, `find_first`, `filter_ordered`, `map_each` with a batch total, `scan_exclusive`, `count_then_fill` and `sort_stable`. On rayon pools of 1, 2, 3 and 8 threads, at chunk sizes 1, 7, 64 and 4,096, each gives the serial executor's answer: values, failures, `Exceeded` records and the meter left behind. `geom-run`'s golden test holds those answers to one SHA-256 digest with pools that start threads directly.

## 8. Findings and what follows

- **Fit to ship in `geom-pool`**, under the conditions above: only `join` on the pool; an explicit thread count; no `unsafe` of our own. The kernel crates reach none of it (`deny-ships.toml`; `clippy.toml`).
- **The notices** go into the generated third-party notices file with piece G481.
- **On WebAssembly** rayon is not built (`geom-pool`'s manifest gates it to native targets). The browser's workers, through wasm-bindgen-rayon, are piece G456's, with its own audit.
- **A rayon or crossbeam version change** is a change to `Cargo.lock` and needs this audit redone. The merge gate already holds the versions to the lock, and cargo-deny checks advisories on every pull request.

## Method

The audit read each published crate at the checksum above and traced the calls of section 2 into `rayon-core` and crossbeam. For each crate it recorded:

- the licence texts and any notice;
- every `unsafe`, counted, and in the code reached, what it relies on;
- every way to panic or abort;
- allocation and global state;
- environment reads and the build scripts.

rayon's documented scope contract, the thing `geom-pool` relies on, is in its API documentation for [`ThreadPool::in_place_scope`](https://docs.rs/rayon/1.12.0/rayon/struct.ThreadPool.html#method.in_place_scope) and [`Scope::spawn`](https://docs.rs/rayon/1.12.0/rayon/struct.Scope.html#method.spawn). The rules audited against are chapter 29.20's rule 3 and the kernel's principles 5, 7 and 13.
