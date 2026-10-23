//! Spare Kit geometry kernel: the parallel helper, budgets and cancellation.
//!
//! One job: the one way kernel code spreads work over threads, the budgets every operation runs
//! inside, and the caller's cancellation.
//!
//! Layer 1, the bottom of the kernel (architecture chapter 27): `no_std`, no dependencies, no
//! threads of its own. It holds the one way kernel code spreads work over threads ([`Exec`]), the
//! budgets every operation runs inside ([`Budget`], [`BatchBudget`], [`WorkMeter`]) and the
//! caller's cancellation ([`Cancel`]). Map: chapters 23.4 (deterministic parallelism: one helper)
//! and 23.10 (budgets and cancellation), with the determinism rules of chapter 21 and the error
//! kinds of chapter 22 (`projects/engines/geometry/architecture/`).
//!
//! - Threads come from the caller, as a [`Pool`] passed in with each call; with none, chunks run
//!   on the calling thread and the answer is the same. Kernel crates never start threads.
//! - A [`ChunkPlan`] cuts a problem into chunks of a fixed size, a constant at the call site,
//!   never derived from the thread count.
//! - Every helper call returns exactly what a serial loop over the items in index order returns
//!   (for a batch's total, in the order of the caller's item keys): values in item order, the
//!   lowest-numbered failure, and budget failures decided by chapter 23.10's phase and batch
//!   rules, the same on every pool, thread count and chunk size.
//! - A [`WorkMeter`] counts work units and logical bytes against a [`Budget`] and polls the
//!   caller's [`Cancel`] at fixed intervals. A cancelled or out-of-memory operation returns
//!   [`RunError::Cancelled`] or [`RunError::OutOfMemory`], never partial results, and they are
//!   never stored.
//!
//! ```
//! use geom_run::{Budget, ChunkPlan, Exec, Phase, RunError, WorkMeter};
//!
//! const PLAN: ChunkPlan = ChunkPlan::fixed::<64>();
//! const SQUARE: Phase = Phase::new("square");
//! let items: Vec<u64> = (0..1_000).collect();
//!
//! // One work unit per item: 1,000 items fit a budget of 1,000.
//! let mut meter = WorkMeter::new(Budget::of_work(1_000));
//! let squares = Exec::serial().map(&mut meter, SQUARE, PLAN, &items, |_, x, meter| {
//!     meter.charge(1)?;
//!     Ok(x * x)
//! })?;
//! assert_eq!(squares.get(999), Some(&998_001));
//! assert_eq!(meter.used().work, 1_000);
//!
//! // A budget one unit short fails at the end of the phase, having used 1,000 units.
//! let mut short = WorkMeter::new(Budget::of_work(999));
//! let Err(RunError::BudgetExceeded(exceeded)) =
//!     Exec::serial().map(&mut short, SQUARE, PLAN, &items, |_, x, meter| {
//!         meter.charge(1)?;
//!         Ok(x * x)
//!     })
//! else {
//!     panic!("expected a budget failure");
//! };
//! assert_eq!((exceeded.phase, exceeded.used.work), (SQUARE, 1_000));
//! # Ok::<(), RunError>(())
//! ```
//!
//! Method: the deterministic parallel helper of chapter 23.4, after rayon's indexed parallel
//! iterators, which keep input order and report the first match in input order
//! (<https://github.com/rayon-rs/rayon/blob/ee0a00bdb1ab039e178a215ad5712fb7fa58e58f/src/iter/mod.rs#L1675-L1680>),
//! without their unordered reductions; work in waves of independent tasks with a barrier, the
//! superstep of Valiant's bulk-synchronous parallel model (<https://doi.org/10.1145/79173.79181>);
//! and budgets as deterministic fuel, after Wasmtime's fuel metering
//! (<https://docs.rs/wasmtime/50.0.0/wasmtime/struct.Config.html#method.consume_fuel>). Each module
//! names the method it follows.
//!
//! Cost: a helper call runs the function passed in at most once per item (a pool that keeps its
//! contract runs each chunk once), plus a fixed amount of bookkeeping per item and per chunk. It
//! allocates its result and, on a pool, the bookkeeping of one wave of at most
//! [`pool::WAVE_MAX`] chunks; after a failure it runs at most one wave of chunks the answer does
//! not need. Budgets, meters, plans and tokens are a few machine words each and never allocate.
//! Each module states its own cost.
//!
//! Invariants this crate keeps:
//!
//! - Results, failures and budget failures are identical for every pool, thread count and chunk
//!   size. The tests check the serial executor against pools of 1 to 512 threads, pools that run
//!   tasks backwards, shuffled, twice or not at all, and chunk sizes 1, 7, 64 and 4,096, and a
//!   golden hash of a fixed corpus that must be the same on every target (`tests/golden.rs`).
//! - No thread is started here; no lock, atomic counter, hash map, clock or random source is used.
//! - No `unsafe`, no panics, no recursion; every allocation that grows with the input is fallible.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod batch;
pub mod budget;
pub mod cancel;
pub mod error;
pub mod exec;
pub mod meter;
mod phase;
pub mod plan;
pub mod pool;
mod scan;
mod sort;

pub use batch::ItemResult;
pub use budget::{BatchBudget, Budget, BudgetKind, LogicalSize, Phase, Usage};
pub use cancel::{Cancel, CancelToken};
pub use error::{Exceeded, Interrupted, ItemError, RunError};
pub use exec::Exec;
pub use meter::{POLL_EVERY, Stop, Stopped, WorkMeter};
pub use plan::{Chunk, ChunkPlan, Chunks};
pub use pool::Pool;
pub use scan::Place;
