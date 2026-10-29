//! The golden hash of `geom-run`: a fixed corpus of helper calls, hashed with SHA-256, must give
//! one digest on every target, build mode, thread count and chunk size.
//!
//! Chapter 21 checks the kernel "with 1, 2, 3 and 8 threads" on every target, chapter 23.4 also
//! forces chunk sizes of 1, 7 and 4,096, and piece G33 is done when the replay vectors "give
//! identical hashes" across all of them. The operation-log replay corpus (pieces G97 and G98) does
//! not exist yet, so this file holds the helper to that promise directly. It makes a fixed corpus
//! of calls into every helper: `try_map` (two phases of one meter, and phases nested inside
//! items), `find_first`, `filter_ordered`, `map_each` with and without a total and with keys in
//! an order of their own, `scan_exclusive`, `count_then_fill` and `sort_stable`. It runs the corpus
//! on the serial executor and on pools of 1, 2, 3 and 8 threads, with chunk sizes 1, 7 and 4,096,
//! and hashes every answer, every error and every meter left behind.
//!
//! Every value is written in a canonical little-endian encoding, with a tag before every variant;
//! a `usize` is written as 8 bytes, so 32-bit and 64-bit targets write the same bytes. The bytes
//! are hashed with SHA-256 (FIPS 180-4), the hash chapters 21 and 28.13 specify for goldens, from
//! `geom_test::golden`, the one implementation every golden uses (piece G19), checked there against
//! the standard's test vectors. The corpus comes from `SplitMix64` with a fixed seed, so the inputs
//! are the same on every target by construction and only the helper's answers can differ.
//!
//! **The hash function changed once, and nothing else with it.** G04 first hashed these bytes with
//! 64-bit FNV-1a, to the golden `0x798d_bcf0_804b_82e3` (`FORMER_FNV_GOLDEN`); chapters 21 and
//! 28.13 ask for SHA-256, so the golden is now the SHA-256 of the same bytes. A test records the
//! corpus's bytes once and hashes that one record both ways, so it shows that both goldens are
//! reproduced from the same bytes: the switch changed the hash function, not the corpus, its
//! encoding or any answer.
//!
//! **The same digest must hold on every target**: Linux, Windows and macOS on x86-64 and arm64,
//! and wasm32-wasip1 (a 32-bit target) under wasmtime and Node, in debug and release, with overflow
//! checks on or off and with `-C target-cpu=native` (`kernel.yml` runs it in each, through `golden
//! run`). The digest lives in the golden store (`geom-test/golden/store.json`, id
//! `geom-run/helpers`), and `geom_test::golden::check` runs the serial corpus twice in one process
//! on two differently perturbed heaps before comparing it with the store's. A changed hash is one
//! of two things. Either it is a bug: some target, pool or chunk size gives other answers. Or the
//! helper's answers or this corpus were changed on purpose, in a pull request that records the
//! change in the store, says which answers changed and why, and is approved by a kernel owner.
//! Never update the store to make one target pass. The test also checks that the
//! corpus still reaches every outcome it is built to reach, so a corpus that silently stopped
//! testing the hard cases fails.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them to build their cases and oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test cases and oracles; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken corpus must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeSet;

use common::{
    Fault, Job, PHASE, Script, SplitMix, THREADS_1, THREADS_2, THREADS_3, THREADS_8, Threads, job,
    plan, scripted,
};
use geom_run::{
    BatchBudget, Budget, BudgetKind, Exceeded, Exec, Interrupted, ItemError, ItemResult, Phase,
    RunError, Stop, Stopped, Usage, WorkMeter,
};
use geom_test::golden::{self, Sha256, Store, THREAD_COUNTS, hex};

/// The corpus's golden in the golden store: its SHA-256 digest. See the module documentation
/// before changing it.
const GOLDEN: &str = "geom-run/helpers";

/// The golden G04 first recorded: the 64-bit FNV-1a hash of the same corpus bytes. Not a golden
/// any more; kept to show that the move to SHA-256 changed only the hash function (see
/// `the_sha256_golden_hashes_the_bytes_the_fnv_golden_hashed`).
const FORMER_FNV_GOLDEN: u64 = 0x798d_bcf0_804b_82e3;

/// The corpus's seed.
const SEED: u64 = 0x0004_6704_601D;

/// The second phase of a meter, and the phase nested inside items.
const SECOND: Phase = Phase::new("second phase");
const INNER: Phase = Phase::new("inner phase");

/// FNV-1a 64 offset basis and prime (Fowler, Noll and Vo,
/// <http://www.isthe.com/chongo/tech/comp/fnv/index.html>).
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// An FNV-1a 64 hash: the hash of G04's first golden, used now only to show that the SHA-256
/// golden hashes the same bytes (`FORMER_FNV_GOLDEN`).
struct Fnv(u64);

impl Fnv {
    const fn new() -> Fnv {
        Fnv(FNV_OFFSET)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }
}

/// Writes answers in the canonical encoding into SHA-256, and notes which outcomes the corpus
/// reached.
struct Recorder {
    hash: Sha256,
    /// Every byte written, in order, if this recorder keeps them ([`Recorder::keeping`]).
    kept: Option<Vec<u8>>,
    calls: u64,
    seen: BTreeSet<&'static str>,
}

impl Recorder {
    fn new() -> Recorder {
        Recorder {
            hash: Sha256::new(),
            kept: None,
            calls: 0,
            seen: BTreeSet::new(),
        }
    }

    /// A recorder that also keeps every byte it hashes, so that the same bytes can be hashed again.
    fn keeping() -> Recorder {
        Recorder {
            kept: Some(Vec::new()),
            ..Recorder::new()
        }
    }

    /// The canonical bytes of one value: into the hash, and into the record if one is kept.
    fn write(&mut self, bytes: &[u8]) {
        self.hash.write(bytes);
        if let Some(kept) = &mut self.kept {
            kept.extend_from_slice(bytes);
        }
    }

    /// The SHA-256 of everything written so far, in hexadecimal.
    fn digest(&self) -> String {
        hex(self.hash.clone().finish())
    }

    /// Start one helper call's record: its operation tag.
    fn call(&mut self, op: u8) {
        self.calls += 1;
        self.u8(op);
    }

    fn see(&mut self, outcome: &'static str) {
        self.seen.insert(outcome);
    }

    fn u8(&mut self, v: u8) {
        self.write(&[v]);
    }

    fn u64(&mut self, v: u64) {
        self.write(&v.to_le_bytes());
    }

    /// A `usize` as 8 bytes, the same on 32-bit and 64-bit targets.
    fn usize(&mut self, v: usize) {
        self.u64(u64::try_from(v).unwrap());
    }

    fn u128(&mut self, v: u128) {
        self.write(&v.to_le_bytes());
    }

    fn text(&mut self, s: &str) {
        self.usize(s.len());
        self.write(s.as_bytes());
    }

    fn indices(&mut self, v: &[usize]) {
        self.usize(v.len());
        for &i in v {
            self.usize(i);
        }
    }

    fn budget(&mut self, b: Budget) {
        self.u64(b.work);
        self.u64(b.logical_bytes);
    }

    fn usage(&mut self, u: Usage) {
        self.u128(u.work);
        self.u128(u.logical_bytes);
    }

    fn kind(&mut self, k: BudgetKind) {
        self.u8(match k {
            BudgetKind::Work => 0,
            BudgetKind::LogicalBytes => 1,
        });
    }

    fn exceeded(&mut self, e: &Exceeded) {
        self.budget(e.budget);
        self.kind(e.kind);
        self.text(e.phase.name());
        self.usage(e.used);
    }

    fn stop(&mut self, s: Stop) {
        match s {
            Stop::OverBudget(k) => {
                self.u8(0);
                self.kind(k);
            }
            Stop::Cancelled => self.u8(1),
            Stop::OutOfMemory => self.u8(2),
        }
    }

    fn fault(&mut self, f: &Fault) {
        match f {
            Fault::Own(i) => {
                self.u8(0);
                self.usize(*i);
            }
            Fault::Meter(stopped) => {
                self.u8(1);
                self.stop(stopped.reason());
            }
        }
    }

    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "passed to `answer`, which hands the function's error over by reference"
    )]
    fn stopped(&mut self, s: &Stopped) {
        self.stop(s.reason());
    }

    /// A helper's error, with `inner` writing the function's own error.
    fn run_error<E>(&mut self, e: &RunError<E>, inner: impl Fn(&mut Recorder, &E)) {
        match e {
            RunError::Failed(error) => {
                self.see("failed");
                self.u8(0);
                inner(self, error);
            }
            RunError::BudgetExceeded(exceeded) => {
                self.see(match exceeded.kind {
                    BudgetKind::Work => "over work",
                    BudgetKind::LogicalBytes => "over logical bytes",
                });
                self.u8(1);
                self.exceeded(exceeded);
            }
            RunError::Cancelled => self.u8(2),
            RunError::OutOfMemory => self.u8(3),
            RunError::Internal(what) => {
                self.u8(4);
                self.text(what);
            }
        }
    }

    /// A helper's answer: `ok` writes the value, `inner` the function's own error.
    fn answer<T, E>(
        &mut self,
        got: &Result<T, RunError<E>>,
        ok: impl Fn(&mut Recorder, &T),
        inner: impl Fn(&mut Recorder, &E),
    ) {
        match got {
            Ok(value) => {
                self.see("ok");
                self.u8(0);
                ok(self, value);
            }
            Err(error) => {
                self.u8(1);
                self.run_error(error, inner);
            }
        }
    }

    /// What a meter has used, and whether and why it stopped.
    fn meter(&mut self, m: &WorkMeter<'_>) {
        self.usage(m.used());
        match m.stopped() {
            None => self.u8(0),
            Some(stop) => {
                self.u8(1);
                self.stop(stop);
            }
        }
    }
}

/// A cap that is often small enough to fail and sometimes unlimited.
fn cap(rng: &mut SplitMix) -> u64 {
    match rng.below(6) {
        0 => 0,
        1 => rng.below(40),
        2 => rng.below(400),
        3 => rng.below(2000),
        4 => 100_000,
        _ => u64::MAX,
    }
}

/// `try_map` on scripted items, then a second phase on the same meter, so R falls by the first
/// phase's total and a stopped meter fails the second at once.
fn phases(exec: Exec<'_>, size: usize, rng: &mut SplitMix, r: &mut Recorder) {
    for _ in 0..40 {
        let len = rng.below(300) as usize;
        let items: Vec<Script> = (0..len)
            .map(|_| Script {
                work: rng.below(8),
                bytes: rng.below(4),
                fails: rng.below(80) == 0,
            })
            .collect();
        let mut meter = WorkMeter::new(Budget::new(cap(rng), cap(rng)));
        for phase in [PHASE, SECOND] {
            let stopped_before = meter.stopped().is_some();
            let got = exec.try_map(&mut meter, phase, plan(size), &items, scripted);
            if stopped_before && got.is_err() {
                r.see("stopped meter");
            }
            r.call(1);
            r.answer(&got, |r, v| r.indices(v), Recorder::fault);
            r.meter(&meter);
        }
    }
}

/// `find_first` and `filter_ordered`, charging one unit per item, sometimes with a failure.
fn searches(exec: Exec<'_>, size: usize, rng: &mut SplitMix, r: &mut Recorder) {
    for _ in 0..30 {
        let len = rng.below(400) as usize;
        let values: Vec<u64> = (0..len).map(|_| rng.below(1000)).collect();
        let modulus = 1 + rng.below(300);
        let fails = if rng.below(3) == 0 {
            Some(rng.below(len as u64 + 1) as usize)
        } else {
            None
        };
        let test = |i: usize, v: &u64, meter: &mut WorkMeter<'_>| -> Result<bool, Fault> {
            meter.charge(1)?;
            if Some(i) == fails {
                Err(Fault::Own(i))
            } else {
                Ok(v.is_multiple_of(modulus))
            }
        };
        let budget = Budget::of_work(cap(rng));

        let mut meter = WorkMeter::new(budget);
        let found = exec.find_first(&mut meter, PHASE, plan(size), &values, test);
        match found {
            Ok(Some(_)) => r.see("found"),
            Ok(None) => r.see("not found"),
            Err(_) => {}
        }
        r.call(2);
        r.answer(
            &found,
            |r, v| match v {
                Some(i) => {
                    r.u8(1);
                    r.usize(*i);
                }
                None => r.u8(0),
            },
            Recorder::fault,
        );
        r.meter(&meter);

        let mut meter = WorkMeter::new(budget);
        let kept = exec.filter_ordered(&mut meter, PHASE, plan(size), &values, test);
        r.call(3);
        r.answer(&kept, |r, v| r.indices(v), Recorder::fault);
        r.meter(&meter);
    }
}

/// `map_each` with keys of their own (many equal), with and without a total.
fn batches(exec: Exec<'_>, size: usize, rng: &mut SplitMix, r: &mut Recorder) {
    for _ in 0..30 {
        let len = rng.below(300) as usize;
        let items: Vec<Job> = (0..len)
            .map(|_| Job {
                work: rng.below(25),
                fails: rng.below(30) == 0,
            })
            .collect();
        let keys: Vec<u64> = (0..len).map(|_| rng.below(len as u64 / 2 + 1)).collect();
        let sum: u64 = items
            .iter()
            .map(|item| item.work)
            .fold(0, u64::saturating_add);
        let per_item = Budget::of_work(rng.below(25));
        let budget = if rng.below(3) == 0 {
            BatchBudget::new(per_item)
        } else {
            BatchBudget::new(per_item).with_total(Budget::of_work(rng.below(sum + 1)))
        };
        let got = exec.map_each(budget, None, PHASE, plan(size), &items, |i, _| keys[i], job);
        r.call(4);
        match &got {
            Ok(results) => {
                r.u8(0);
                r.usize(results.len());
                for result in results {
                    item(r, result);
                }
            }
            Err(interrupted) => {
                r.u8(1);
                r.u8(match interrupted {
                    Interrupted::Cancelled => 0,
                    Interrupted::OutOfMemory => 1,
                    Interrupted::Internal(_) => 2,
                });
            }
        }
    }
}

/// One batch item's result.
fn item(r: &mut Recorder, result: &ItemResult<usize, &'static str>) {
    match result {
        Ok(v) => {
            r.see("batch item ok");
            r.u8(0);
            r.usize(*v);
        }
        Err(ItemError::Failed(what)) => {
            r.see("batch item failed");
            r.u8(1);
            r.text(what);
        }
        Err(ItemError::BudgetExceeded(exceeded)) => {
            r.see("batch item over its budget");
            r.u8(2);
            r.exceeded(exceeded);
        }
        Err(ItemError::BatchTotal(exceeded)) => {
            r.see("batch total");
            r.u8(3);
            r.exceeded(exceeded);
        }
    }
}

/// `scan_exclusive`, `count_then_fill` and `sort_stable`.
fn layouts(exec: Exec<'_>, size: usize, rng: &mut SplitMix, r: &mut Recorder) {
    for _ in 0..20 {
        let len = rng.below(600) as usize;
        let counts: Vec<usize> = (0..len).map(|_| rng.below(50) as usize).collect();
        let got = exec.scan_exclusive::<Stopped>(
            &mut WorkMeter::new(Budget::UNLIMITED),
            PHASE,
            plan(size),
            &counts,
        );
        r.call(5);
        r.answer(
            &got,
            |r, (offsets, total)| {
                r.indices(offsets);
                r.usize(*total);
            },
            Recorder::stopped,
        );

        let values: Vec<u64> = (0..len).map(|_| rng.below(1000)).collect();
        let fails = if rng.below(4) == 0 {
            Some(rng.below(len as u64 + 1) as usize)
        } else {
            None
        };
        let count = |_: usize, v: &u64, meter: &mut WorkMeter<'_>| -> Result<usize, Fault> {
            meter.charge(1)?;
            Ok((v % 5) as usize)
        };
        let fill = |i: usize,
                    v: &u64,
                    out: &mut [u64],
                    place: geom_run::Place<'_>,
                    meter: &mut WorkMeter<'_>|
         -> Result<(), Fault> {
            meter.reserve(8 * out.len() as u64)?;
            if Some(i) == fails {
                return Err(Fault::Own(i));
            }
            for (j, slot) in out.iter_mut().enumerate() {
                *slot = v * 8 + j as u64 + place.first as u64 * 1_000_000;
            }
            Ok(())
        };
        let mut meter = WorkMeter::new(Budget::new(cap(rng), cap(rng)));
        let got = exec.count_then_fill(&mut meter, PHASE, plan(size), &values, count, fill);
        r.call(6);
        r.answer(
            &got,
            |r, (outputs, offsets)| {
                r.usize(outputs.len());
                for &v in outputs {
                    r.u64(v);
                }
                r.indices(offsets);
            },
            Recorder::fault,
        );
        r.meter(&meter);

        let got = exec.sort_stable::<_, _, Stopped, _>(
            &mut WorkMeter::new(Budget::UNLIMITED),
            PHASE,
            plan(size),
            &values,
            |v| v % 50,
        );
        if got.is_ok() {
            r.see("sorted");
        }
        r.call(7);
        r.answer(&got, |r, v| r.indices(v), Recorder::stopped);
    }
}

/// Phases nested inside items: each item runs its group of scripted items as a phase of its own
/// meter, on the same executor.
fn nested(exec: Exec<'_>, size: usize, rng: &mut SplitMix, r: &mut Recorder) {
    for _ in 0..12 {
        let groups: Vec<Vec<Script>> = (0..rng.below(40))
            .map(|_| {
                (0..rng.below(40))
                    .map(|_| Script {
                        work: rng.below(6),
                        bytes: rng.below(3),
                        fails: rng.below(150) == 0,
                    })
                    .collect()
            })
            .collect();
        let outer = |_: usize, group: &Vec<Script>, meter: &mut WorkMeter<'_>| {
            meter
                .charge(1)
                .map_err(|stopped| RunError::Failed(Fault::Meter(stopped)))?;
            exec.try_map(meter, INNER, plan(size), group, scripted)
        };
        let mut meter = WorkMeter::new(Budget::new(cap(rng), cap(rng)));
        let got = exec.try_map(&mut meter, PHASE, plan(size), &groups, outer);
        match got {
            Ok(_) => r.see("nested ok"),
            // An inner phase over budget stops the item's meter, so the outer phase reports it.
            Err(RunError::BudgetExceeded(_)) => r.see("nested over budget"),
            Err(_) => {}
        }
        r.call(8);
        r.answer(
            &got,
            |r, v| {
                r.usize(v.len());
                for group in v {
                    r.indices(group);
                }
            },
            |r, e| r.run_error(e, Recorder::fault),
        );
        r.meter(&meter);
    }
}

/// The whole corpus on `exec` with chunk size `size`, written into `r`.
fn corpus(exec: Exec<'_>, size: usize, mut r: Recorder) -> Recorder {
    let mut rng = SplitMix(SEED);
    phases(exec, size, &mut rng, &mut r);
    searches(exec, size, &mut rng, &mut r);
    batches(exec, size, &mut rng, &mut r);
    layouts(exec, size, &mut rng, &mut r);
    nested(exec, size, &mut rng, &mut r);
    r
}

/// The outcomes the corpus is built to reach.
const REQUIRED: [&str; 14] = [
    "ok",
    "failed",
    "over work",
    "over logical bytes",
    "stopped meter",
    "found",
    "not found",
    "batch item ok",
    "batch item failed",
    "batch item over its budget",
    "batch total",
    "sorted",
    "nested ok",
    "nested over budget",
];

#[test]
fn fnv1a_64_matches_its_published_test_vectors() {
    let hash = |s: &str| {
        let mut h = Fnv::new();
        h.write(s.as_bytes());
        h.0
    };
    assert_eq!(hash(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(hash("a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(hash("foobar"), 0x8594_4171_f739_67e8);
}

/// The golden hash: the same on the serial executor, on 1, 2, 3 and 8 threads, with chunk sizes
/// 1, 7 and 4,096, and on every target.
#[test]
fn golden_hash_is_the_same_on_every_target_thread_count_and_chunk_size() {
    let pool = |n: usize| -> &'static Threads {
        match n {
            1 => &THREADS_1,
            2 => &THREADS_2,
            3 => &THREADS_3,
            8 => &THREADS_8,
            _ => panic!("no pool of {n} threads"),
        }
    };
    let executors: Vec<(String, Exec<'static>)> =
        std::iter::once(("serial".to_owned(), Exec::serial()))
            .chain(
                THREAD_COUNTS
                    .into_iter()
                    .map(|n| (format!("threads {n}"), Exec::with_pool(pool(n)))),
            )
            .collect();
    let checked = golden::check(GOLDEN, || {
        let r = corpus(Exec::serial(), 1, Recorder::new());
        (r.hash.clone().finish(), r)
    });
    let reference = &checked.value;
    let missing: Vec<_> = REQUIRED
        .iter()
        .filter(|outcome| !reference.seen.contains(*outcome))
        .collect();
    assert!(missing.is_empty(), "unreached outcomes: {missing:?}");
    println!(
        "geom-run determinism corpus: {} helper calls",
        reference.calls
    );
    let hash = reference.digest();
    for size in [1, 7, 4096] {
        for &(ref name, exec) in &executors {
            let got = corpus(exec, size, Recorder::new()).digest();
            println!("  {name:<10} chunk size {size:>4}: {got}");
            assert_eq!(
                got, hash,
                "{name} with chunk size {size} differs from serial with chunk size 1"
            );
        }
    }
    println!("geom-run golden hash: {hash}");
    checked.verdict().unwrap_or_else(|e| {
        panic!("{e}. For this corpus: this target gives other answers (a bug), or the helper's answers or the corpus changed on purpose")
    });
}

/// The move from FNV-1a 64 to SHA-256 changed the hash function and nothing else. The corpus's
/// bytes, recorded once, still hash with FNV-1a 64 to G04's first golden, and the same recorded
/// bytes hash with SHA-256 to `GOLDEN`, as the recorder's own streaming SHA-256 does.
#[test]
fn the_sha256_golden_hashes_the_bytes_the_fnv_golden_hashed() {
    let recorded = corpus(Exec::serial(), 1, Recorder::keeping());
    let bytes = recorded
        .kept
        .as_deref()
        .expect("a keeping recorder keeps its bytes");
    println!(
        "geom-run determinism corpus: {} canonical bytes",
        bytes.len()
    );

    let mut fnv = Fnv::new();
    fnv.write(bytes);
    println!("FNV-1a 64 of the recorded bytes: {:#018x}", fnv.0);
    assert_eq!(
        fnv.0, FORMER_FNV_GOLDEN,
        "the corpus's bytes are not the ones G04's first golden hashed"
    );

    let mut sha = Sha256::new();
    sha.write(bytes);
    let digest = hex(sha.finish());
    println!("SHA-256 of the recorded bytes:   {digest}");
    let golden = Store::embedded()
        .unwrap()
        .expected(GOLDEN)
        .unwrap()
        .to_string();
    assert_eq!(
        digest, golden,
        "the same bytes must give the SHA-256 golden"
    );
    assert_eq!(
        recorded.digest(),
        digest,
        "the streaming digest differs from the digest of the recorded bytes"
    );
}
