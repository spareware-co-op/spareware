//! Timings for `geom-run`: `cargo bench -p geom-run`.
//!
//! A plain timing harness (the median of repeated runs, measured with `std::time::Instant`) until
//! the kernel's benchmark crate, `geom-bench`, exists and its regression gate takes these over
//! (chapter 23.13). What it measures:
//!
//! - The helper's cost per item on the serial executor, against a plain loop doing the same work.
//! - Speed-up on a pool of every available thread, for cheap and for costly items. The pool here
//!   is a simple one written for the benchmark (it starts scoped threads for every wave), so the
//!   numbers are a floor for a real pool such as rayon's, which keeps its threads.
//! - `scan_exclusive` and `sort_stable` against plain serial code.
//! - Cancellation latency: the time from `CancelToken::cancel` to the helper's return, for an
//!   item that charges one work unit at a time (chapter 23.10's target is 1 ms on native targets).
//!
//! Timings depend on the machine; only the answers are deterministic. A helper call that does not
//! give the expected answer ends the run with an error, never a panic.

use std::error::Error;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use geom_run::{Budget, CancelToken, ChunkPlan, Exec, Phase, Pool, RunError, Stopped, WorkMeter};

const PHASE: Phase = Phase::new("bench");
const PLAN: ChunkPlan = ChunkPlan::fixed::<4096>();

/// Why a benchmark run stopped early.
type Failure = Box<dyn Error>;

/// `n` threads: the calling thread and `n − 1` scoped threads, each taking the next task.
struct Threads(usize);

impl Pool for Threads {
    fn run_all(&self, tasks: &mut [&mut (dyn FnMut() + Send)]) {
        let helpers = self.0.min(tasks.len()).saturating_sub(1);
        let queue = Mutex::new(tasks.iter_mut());
        let work = || {
            loop {
                // The lock is held only to take the next task, which cannot panic, so the queue
                // is never left half-updated; a poisoned lock still holds a sound queue.
                let next = queue.lock().unwrap_or_else(PoisonError::into_inner).next();
                match next {
                    Some(task) => task(),
                    None => return,
                }
            }
        };
        thread::scope(|scope| {
            for _ in 0..helpers {
                scope.spawn(work);
            }
            work();
        });
    }
}

/// The median time of `runs` runs of `f`, which must be at least one.
fn median(runs: usize, mut f: impl FnMut() -> Result<(), RunError>) -> Result<Duration, Failure> {
    let mut times: Vec<Duration> = Vec::with_capacity(runs);
    for _ in 0..runs {
        let start = Instant::now();
        f()?;
        times.push(start.elapsed());
    }
    times.sort_unstable();
    let middle = times.get(runs / 2).copied();
    middle.ok_or_else(|| "a median needs at least one run".into())
}

fn per_item(time: Duration, items: usize) -> f64 {
    time.as_secs_f64() * 1e9 / items as f64
}

fn spin(rounds: u64, seed: u64) -> u64 {
    let mut x = seed;
    for _ in 0..rounds {
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        x ^= x >> 29;
    }
    x
}

fn main() -> Result<(), Failure> {
    let threads = thread::available_parallelism().map_or(4, NonZeroUsize::get);
    let pool = Threads(threads);
    let pooled = Exec::with_pool(&pool);
    let serial = Exec::serial();
    let items: Vec<u64> = (0..1_000_000).collect();
    println!("geom-run timings ({threads} threads available), median of repeated runs");

    // Cheap items: the helper's own cost per item.
    let plain = median(15, || {
        let out: Vec<u64> = items.iter().map(|x| x.wrapping_mul(3)).collect();
        black_box(out);
        Ok(())
    })?;
    let cheap = |_: usize, x: &u64, meter: &mut WorkMeter<'_>| {
        meter.charge(1)?;
        Ok::<_, Stopped>(x.wrapping_mul(3))
    };
    let helper = median(15, || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        black_box(serial.map(&mut meter, PHASE, PLAN, &items, cheap)?);
        Ok(())
    })?;
    let threaded = median(15, || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        black_box(pooled.map(&mut meter, PHASE, PLAN, &items, cheap)?);
        Ok(())
    })?;
    println!(
        "cheap item, plain loop:      {:8.2} ns/item",
        per_item(plain, items.len())
    );
    println!(
        "cheap item, helper serial:   {:8.2} ns/item",
        per_item(helper, items.len())
    );
    println!(
        "cheap item, helper on pool:  {:8.2} ns/item",
        per_item(threaded, items.len())
    );

    // Costly items: speed-up on the pool.
    let costly = |i: usize, x: &u64, meter: &mut WorkMeter<'_>| {
        meter.charge(1)?;
        Ok::<_, Stopped>(spin(200, x ^ i as u64))
    };
    let some = items
        .get(..200_000)
        .ok_or("fewer than 200,000 items for the costly runs")?;
    let alone = median(5, || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        black_box(serial.map(&mut meter, PHASE, PLAN, some, costly)?);
        Ok(())
    })?;
    let together = median(5, || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        black_box(pooled.map(&mut meter, PHASE, PLAN, some, costly)?);
        Ok(())
    })?;
    println!(
        "costly item, helper serial:  {:8.2} ns/item",
        per_item(alone, some.len())
    );
    println!(
        "costly item, helper on pool: {:8.2} ns/item",
        per_item(together, some.len())
    );
    let speedup = alone.as_secs_f64() / together.as_secs_f64();
    println!("costly item, speed-up:       {speedup:8.2}x");

    // Scan and sort.
    let counts: Vec<usize> = items.iter().map(|x| (x % 7) as usize).collect();
    let plain_scan = median(15, || {
        let mut total = 0_usize;
        let offsets: Vec<usize> = counts
            .iter()
            .map(|c| {
                let offset = total;
                total += c;
                offset
            })
            .collect();
        black_box((offsets, total));
        Ok(())
    })?;
    let scan = median(15, || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        black_box(pooled.scan_exclusive::<Stopped>(&mut meter, PHASE, PLAN, &counts)?);
        Ok(())
    })?;
    println!(
        "scan, plain loop:            {:8.2} ns/item",
        per_item(plain_scan, counts.len())
    );
    println!(
        "scan, helper on pool:        {:8.2} ns/item",
        per_item(scan, counts.len())
    );
    let keys: Vec<u64> = items.iter().map(|x| spin(3, *x) % 1000).collect();
    let plain_sort = median(5, || {
        let mut order: Vec<usize> = (0..keys.len()).collect();
        // Every index is below `keys.len()`, so every key is `Some` and they sort as the keys do.
        order.sort_by_key(|&i| keys.get(i).copied());
        black_box(order);
        Ok(())
    })?;
    let sort_serial = median(5, || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        black_box(serial.sort_stable::<_, _, Stopped, _>(&mut meter, PHASE, PLAN, &keys, |k| *k)?);
        Ok(())
    })?;
    let sort_pool = median(5, || {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        black_box(pooled.sort_stable::<_, _, Stopped, _>(&mut meter, PHASE, PLAN, &keys, |k| *k)?);
        Ok(())
    })?;
    println!(
        "sort, std stable sort:       {:8.2} ns/item",
        per_item(plain_sort, keys.len())
    );
    println!(
        "sort, helper serial:         {:8.2} ns/item",
        per_item(sort_serial, keys.len())
    );
    println!(
        "sort, helper on pool:        {:8.2} ns/item",
        per_item(sort_pool, keys.len())
    );

    // Cancellation latency.
    let mut latencies: Vec<Duration> = Vec::with_capacity(21);
    for _ in 0..21 {
        latencies.push(cancel_latency(serial)?);
    }
    latencies.sort_unstable();
    let middle = latencies.get(latencies.len() / 2);
    let (Some(middle), Some(worst)) = (middle, latencies.last()) else {
        return Err("no cancellation latencies were measured".into());
    };
    println!("cancellation latency:        median {middle:?}, worst {worst:?} (target 1 ms)");
    Ok(())
}

/// The time from a cancellation to the helper's return, for an item charging one unit at a time.
fn cancel_latency(exec: Exec<'_>) -> Result<Duration, Failure> {
    let token = CancelToken::new();
    let started = AtomicBool::new(false);
    thread::scope(|scope| {
        let canceller = scope.spawn(|| {
            while !started.load(Ordering::SeqCst) {
                thread::yield_now();
            }
            thread::sleep(Duration::from_millis(5));
            token.cancel();
            Instant::now()
        });
        let mut meter = WorkMeter::new(Budget::UNLIMITED).with_cancel(&token);
        let got: Result<Vec<()>, RunError> =
            exec.map(&mut meter, PHASE, PLAN, &[()], |_, _, meter| {
                started.store(true, Ordering::SeqCst);
                loop {
                    meter.charge(1)?;
                    black_box(spin(20, 1));
                }
            });
        let returned = Instant::now();
        let cancelled_at = canceller
            .join()
            .map_err(|_| "the cancelling thread panicked")?;
        if got != Err(RunError::Cancelled) {
            return Err(format!("expected a cancelled run, got {got:?}").into());
        }
        Ok(returned.saturating_duration_since(cancelled_at))
    })
}
