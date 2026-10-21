//! Helpers shared by the geom-run test files: thread pools of every kind the helper must give the
//! same answer on (including pools that break their contract), the executors built on them, a
//! seeded generator, busy work, counting cancellation sources, scripted items and independent
//! serial models of chapter 23.10's phase rule and batch rule.
//!
//! The pools are test code: the kernel itself starts no threads. Each test file uses only some of
//! these helpers. The counters are behind locks, not atomics: the crate's clippy.toml forbids
//! atomic counters (chapter 23.4), and its tests keep the same rule.
#![allow(
    dead_code,
    reason = "every test file includes this module and uses only some of its helpers"
)]

use std::any::Any;
use std::fmt::Debug;
use std::num::NonZeroUsize;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;
use std::thread;

use geom_run::{
    BatchBudget, Budget, BudgetKind, Cancel, ChunkPlan, Exceeded, Exec, ItemError, ItemResult,
    Phase, Pool, RunError, Stop, Stopped, Usage, WorkMeter,
};

/// A pool of `n` threads: the calling thread and up to `n − 1` scoped threads, never more than
/// tasks, each claiming the next task when it finishes one. Re-entrant: a task's nested call
/// starts threads of its own. A task's panic reaches the caller with its own payload, once every
/// thread has stopped. Where a thread cannot be started (wasm32-wasip1 has no threads), the
/// threads already started and the calling thread run every task.
pub struct Threads(pub usize);

impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let helpers = self.0.min(tasks.len()).saturating_sub(1);
        let queue = Mutex::new(tasks.iter_mut());
        let work = || {
            loop {
                let next = queue.lock().unwrap().next();
                match next {
                    Some(task) => task(),
                    None => return,
                }
            }
        };
        let panicked = thread::scope(|scope| {
            // `spawn_scoped` reports a thread it cannot start, where `Scope::spawn` would panic.
            let handles: Vec<_> = (0..helpers)
                .map_while(|_| thread::Builder::new().spawn_scoped(scope, work).ok())
                .collect();
            let mut first = panic::catch_unwind(AssertUnwindSafe(&work)).err();
            for handle in handles {
                if let Err(payload) = handle.join() {
                    first.get_or_insert(payload);
                }
            }
            first
        });
        if let Some(payload) = panicked {
            panic::resume_unwind(payload);
        }
    }
}

/// Runs every task on the calling thread, last first: the latest chunks finish first.
pub struct Backwards;

impl Pool for Backwards {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        for task in tasks.iter_mut().rev() {
            task();
        }
    }
}

/// Runs every task on the calling thread, in a seeded shuffled order.
pub struct Shuffled(pub u64);

impl Pool for Shuffled {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let mut order: Vec<usize> = (0..tasks.len()).collect();
        let mut rng = SplitMix(self.0 ^ tasks.len() as u64);
        for i in (1..order.len()).rev() {
            let j = rng.below(i as u64 + 1) as usize;
            order.swap(i, j);
        }
        for i in order {
            tasks[i]();
        }
    }
}

/// Breaks the contract: runs every task twice.
pub struct Twice;

impl Pool for Twice {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        for task in tasks.iter_mut() {
            task();
            task();
        }
    }
}

/// Breaks the contract: runs only every `n`th task, or none at all for `n == 0`.
pub struct Skipping(pub usize);

impl Pool for Skipping {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        if self.0 == 0 {
            return;
        }
        for task in tasks.iter_mut().step_by(self.0) {
            task();
        }
    }
}

/// Runs every task on the calling thread, in order.
pub struct Inline;

impl Pool for Inline {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        for task in tasks.iter_mut() {
            task();
        }
    }
}

pub static THREADS_1: Threads = Threads(1);
pub static THREADS_2: Threads = Threads(2);
pub static THREADS_3: Threads = Threads(3);
pub static THREADS_4: Threads = Threads(4);
pub static THREADS_8: Threads = Threads(8);
pub static THREADS_16: Threads = Threads(16);
pub static BACKWARDS: Backwards = Backwards;
pub static SHUFFLED: Shuffled = Shuffled(0x5EED);
pub static TWICE: Twice = Twice;
pub static SKIP_HALF: Skipping = Skipping(2);
pub static SKIP_ALL: Skipping = Skipping(0);
pub static INLINE: Inline = Inline;

/// Every executor the determinism tests compare, with a name for messages: serial first, then
/// 1, 2, 3, 4, 8 and 16 threads, then pools that run tasks backwards, shuffled, twice, half or
/// not at all.
pub fn execs() -> Vec<(&'static str, Exec<'static>)> {
    vec![
        ("serial", Exec::serial()),
        ("threads 1", Exec::with_pool(&THREADS_1)),
        ("threads 2", Exec::with_pool(&THREADS_2)),
        ("threads 3", Exec::with_pool(&THREADS_3)),
        ("threads 4", Exec::with_pool(&THREADS_4)),
        ("threads 8", Exec::with_pool(&THREADS_8)),
        ("threads 16", Exec::with_pool(&THREADS_16)),
        ("backwards", Exec::with_pool(&BACKWARDS)),
        ("shuffled", Exec::with_pool(&SHUFFLED)),
        ("twice", Exec::with_pool(&TWICE)),
        ("skips half", Exec::with_pool(&SKIP_HALF)),
        ("skips all", Exec::with_pool(&SKIP_ALL)),
    ]
}

/// A smaller set for slower cases: serial, 2, 3 and 8 threads, and the backwards pool.
pub fn few_execs() -> Vec<(&'static str, Exec<'static>)> {
    vec![
        ("serial", Exec::serial()),
        ("threads 2", Exec::with_pool(&THREADS_2)),
        ("threads 3", Exec::with_pool(&THREADS_3)),
        ("threads 8", Exec::with_pool(&THREADS_8)),
        ("backwards", Exec::with_pool(&BACKWARDS)),
    ]
}

/// The chunk sizes chapter 23.4 forces, 1, 7 and 4,096, and 64 between them.
pub const SIZES: [usize; 4] = [1, 7, 64, 4096];

/// A plan with chunk size `size` (which must be at least 1).
pub fn plan(size: usize) -> ChunkPlan {
    ChunkPlan::new(NonZeroUsize::new(size).expect("chunk size of at least 1"))
}

/// The phase every test runs as unless it says otherwise.
pub const PHASE: Phase = Phase::new("test phase");

/// A meter with no limit worth the name.
pub fn unlimited() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Run `f` on every executor, check every answer equals the serial one, and return it.
pub fn same_everywhere<T: PartialEq + Debug>(f: impl Fn(Exec<'static>) -> T) -> T {
    let mut answers = execs().into_iter().map(|(name, exec)| (name, f(exec)));
    let (_, reference) = answers.next().expect("at least the serial executor");
    for (name, answer) in answers {
        assert_eq!(answer, reference, "{name} differs from serial");
    }
    reference
}

/// SplitMix64 (Steele, Lea and Flood, "Fast splittable pseudorandom number generators", OOPSLA
/// 2014): a fixed-seed generator, so every test case is the same on every run and machine.
pub struct SplitMix(pub u64);

impl SplitMix {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`, for `n > 0`.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// A finite double with a random sign, a random 52-bit fraction and a binary exponent in
    /// `-spread..spread`, built from its bits (no maths functions).
    pub fn wide_double(&mut self, spread: u64) -> f64 {
        let exponent = 1023 - spread + self.below(2 * spread);
        let sign = self.next_u64() & (1 << 63);
        f64::from_bits(sign | (exponent << 52) | (self.next_u64() >> 12))
    }
}

/// Busy work costing about `rounds` multiply-adds, with a result that depends on every round so the
/// optimiser cannot remove it.
pub fn spin(rounds: u64, seed: u64) -> u64 {
    let mut x = seed;
    for _ in 0..rounds {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        x ^= x >> 29;
    }
    std::hint::black_box(x)
}

/// A count that any thread may add to, behind a lock.
pub struct Tally(Mutex<usize>);

impl Tally {
    pub const fn new() -> Self {
        Self(Mutex::new(0))
    }

    /// Add one, and return the count before it.
    pub fn add(&self) -> usize {
        let mut count = self.0.lock().unwrap();
        let before = *count;
        *count += 1;
        before
    }

    /// The count so far.
    pub fn get(&self) -> usize {
        *self.0.lock().unwrap()
    }
}

/// A cancellation source that answers "not yet" to its first `allowed` questions and "stop" ever
/// after, and counts the questions.
pub struct Countdown {
    allowed: usize,
    asked: Tally,
}

impl Countdown {
    pub const fn new(allowed: usize) -> Self {
        Self {
            allowed,
            asked: Tally::new(),
        }
    }

    /// How many times it has been asked.
    pub fn asked(&self) -> usize {
        self.asked.get()
    }
}

impl Cancel for Countdown {
    fn requested(&self) -> bool {
        self.asked.add() >= self.allowed
    }
}

/// Run `f`, turning a panic into `Err(its message)`.
pub fn no_panic<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|payload| message(&*payload))
}

/// A panic payload's message.
pub fn message(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// A test function's own error, or a stop passed on from its meter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The item at this index failed on its own.
    Own(usize),
    /// The meter stopped (the helper reports the meter's stop instead).
    Meter(Stopped),
}

impl From<Stopped> for Fault {
    fn from(stopped: Stopped) -> Self {
        Fault::Meter(stopped)
    }
}

/// A scripted item: charges `work` units, then reserves `bytes` logical bytes, then fails if
/// `fails`, otherwise returns its index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Script {
    pub work: u64,
    pub bytes: u64,
    pub fails: bool,
}

impl Script {
    pub const fn ok(work: u64) -> Self {
        Self {
            work,
            bytes: 0,
            fails: false,
        }
    }

    pub const fn bytes(work: u64, bytes: u64) -> Self {
        Self {
            work,
            bytes,
            fails: false,
        }
    }

    pub const fn fail(work: u64) -> Self {
        Self {
            work,
            bytes: 0,
            fails: true,
        }
    }
}

/// The function every scripted test runs.
pub fn scripted(index: usize, item: &Script, meter: &mut WorkMeter<'_>) -> Result<usize, Fault> {
    meter.charge(item.work)?;
    meter.reserve(item.bytes)?;
    if item.fails {
        Err(Fault::Own(index))
    } else {
        Ok(index)
    }
}

/// What a phase of a fresh meter must return and leave behind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expected {
    pub answer: Result<Vec<usize>, RunError<Fault>>,
    /// The meter's use afterwards.
    pub used: Usage,
    /// The limit the meter is stopped on afterwards, if it is.
    pub stopped: Option<BudgetKind>,
}

/// The first kind, work before logical bytes, in which `used` is over `cap`.
fn first_over(used: Usage, cap: Budget) -> Option<BudgetKind> {
    if used.work > u128::from(cap.work) {
        Some(BudgetKind::Work)
    } else if used.logical_bytes > u128::from(cap.logical_bytes) {
        Some(BudgetKind::LogicalBytes)
    } else {
        None
    }
}

/// Chapter 23.10's phase rule, written as the plain serial loop the chapter describes and
/// independently of the crate, for a fresh meter with cap R = `cap`: each item capped at R, the
/// lowest-numbered failure of any kind wins with what the loop had used, and otherwise a total
/// over R fails at the end of the phase.
pub fn phase_model(items: &[Script], cap: Budget, phase: Phase) -> Expected {
    let exceeded = |kind, used| {
        RunError::BudgetExceeded(Exceeded {
            budget: cap,
            kind,
            phase,
            used,
        })
    };
    let mut total = Usage::ZERO;
    let mut values = Vec::new();
    for (index, item) in items.iter().enumerate() {
        // The item's own meter: the charge, then the reservation, each checked against R.
        total.work += u128::from(item.work);
        if item.work > cap.work {
            return Expected {
                answer: Err(exceeded(BudgetKind::Work, total)),
                used: total,
                stopped: Some(BudgetKind::Work),
            };
        }
        total.logical_bytes += u128::from(item.bytes);
        if item.bytes > cap.logical_bytes {
            return Expected {
                answer: Err(exceeded(BudgetKind::LogicalBytes, total)),
                used: total,
                stopped: Some(BudgetKind::LogicalBytes),
            };
        }
        if item.fails {
            return Expected {
                answer: Err(RunError::Failed(Fault::Own(index))),
                used: total,
                stopped: first_over(total, cap),
            };
        }
        values.push(index);
    }
    match first_over(total, cap) {
        Some(kind) => Expected {
            answer: Err(exceeded(kind, total)),
            used: total,
            stopped: Some(kind),
        },
        None => Expected {
            answer: Ok(values),
            used: total,
            stopped: None,
        },
    }
}

/// Run the scripted phase on `exec` with a fresh meter of cap `cap` and chunk size `size`, and
/// report it in the shape of [`phase_model`].
pub fn run_scripted(exec: Exec<'_>, items: &[Script], cap: Budget, size: usize) -> Expected {
    let mut meter = WorkMeter::new(cap);
    let answer = exec.try_map(&mut meter, PHASE, plan(size), items, scripted);
    let stopped = match meter.stopped() {
        Some(Stop::OverBudget(kind)) => Some(kind),
        Some(other) => panic!("unexpected stop {other:?}"),
        None => None,
    };
    Expected {
        answer,
        used: meter.used(),
        stopped,
    }
}

/// A batch item: charges `work`, then fails if `fails`, otherwise returns its index.
#[derive(Clone, Copy, Debug)]
pub struct Job {
    pub work: u64,
    pub fails: bool,
}

/// The function every batch test runs.
pub fn job(index: usize, item: &Job, meter: &mut WorkMeter<'_>) -> Result<usize, &'static str> {
    meter.charge(item.work).map_err(|_| "stopped")?;
    if item.fails {
        Err("own failure")
    } else {
        Ok(index)
    }
}

/// A batch key that is the item's index: for batches whose items are already in key order.
pub fn by_index<T>(index: usize, _: &T) -> usize {
    index
}

/// The batch rule of chapter 23.10 for items whose keys are their indices.
pub fn batch_model(items: &[Job], budget: BatchBudget) -> Vec<ItemResult<usize, &'static str>> {
    let keys: Vec<u64> = (0..items.len() as u64).collect();
    batch_model_keyed(items, &keys, budget)
}

/// The batch rule of chapter 23.10, written as a plain loop without the crate: item `i` has key
/// `keys[i]`, the total is summed in key order (equal keys in item order), and the results come
/// back in item order.
pub fn batch_model_keyed(
    items: &[Job],
    keys: &[u64],
    budget: BatchBudget,
) -> Vec<ItemResult<usize, &'static str>> {
    assert_eq!(items.len(), keys.len(), "one key per item");
    let mut order: Vec<usize> = (0..items.len()).collect();
    // A stable sort, so equal keys stay in item order.
    order.sort_by_key(|&index| keys[index]);
    let mut results = vec![None; items.len()];
    let mut running = Usage::ZERO;
    let mut crossing: Option<Exceeded> = None;
    for index in order {
        let item = &items[index];
        let used = Usage::new(u128::from(item.work), 0);
        let own = if item.work > budget.per_item.work {
            Err(ItemError::BudgetExceeded(Exceeded {
                budget: budget.per_item,
                kind: BudgetKind::Work,
                phase: PHASE,
                used,
            }))
        } else if item.fails {
            Err(ItemError::Failed("own failure"))
        } else {
            Ok(index)
        };
        if crossing.is_none() {
            running.work += used.work;
            crossing = budget
                .total
                .filter(|total| running.work > u128::from(total.work))
                .map(|total| Exceeded {
                    budget: total,
                    kind: BudgetKind::Work,
                    phase: PHASE,
                    used: running,
                });
        }
        results[index] = Some(match crossing {
            Some(crossing) => Err(ItemError::BatchTotal(crossing)),
            None => own,
        });
    }
    results
        .into_iter()
        .map(|result| result.expect("every item has a result"))
        .collect()
}
