//! Containers and trust on open give the same bytes on every target and at any number of threads
//! (principle 5, chapter 21; G108): the archived stamped bodies taken into the reference container
//! and written back, each blob salvaged with one bit flipped at its first, middle and last byte,
//! and the document opened at both trusts with each blob so damaged, hashed into one golden digest.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    reason = "a test corpus, and chapter 21's thread-count test spawns threads"
)]

use geom_format::container::Container;
use geom_format::file::read;
use geom_format::registry::knows;
use geom_format::source::SliceSource;
use geom_format::trust::{Checked, Checker, Level, Trust, open};
use geom_run::{Budget, Stopped, WorkMeter};
use geom_test::golden::{self, Sha256, hex};
use geom_topo::Body;

const GOLDEN: &str = "geom-format/containers";

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A checker that passes every body: the corpus is about reading, not about checking.
struct Pass;

impl Checker for Pass {
    type Report = ();

    fn version(&self) -> u64 {
        1
    }

    fn groups(&self) -> Vec<u64> {
        vec![1, 4]
    }

    fn check(
        &mut self,
        _: &Body,
        _: Level,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Checked<()>, Stopped> {
        meter.charge(1)?;
        Ok(Checked {
            ran: vec![1, 4],
            passed: true,
            report: (),
        })
    }
}

fn put(total: &mut Sha256, bytes: &[u8]) {
    total.write(&(bytes.len() as u64).to_le_bytes());
    total.write(bytes);
}

/// The open of `c` at `trust`: each entry's state and stamp, and the work it charged.
fn opened(c: &Container, trust: Trust) -> Vec<u8> {
    let mut m = meter();
    let o = open(c, &c.document, &c.roots, trust, &mut Pass, &mut m).unwrap();
    let mut out = format!("{:?} {:?} {:?}", o.other, o.skipped, m.used()).into_bytes();
    for e in &o.entries {
        out.extend(format!("|{:?} {:?} {:?}", e.root, e.state, e.stamp).bytes());
    }
    out
}

fn corpus() -> Sha256 {
    let bytes = include_bytes!("archive/v1/stamped-bodies.skgf");
    let file = read(&mut SliceSource::new(bytes), knows, &mut meter()).unwrap();
    let c = Container::from_file(&file, &mut meter()).unwrap();
    let mut total = Sha256::new();
    put(&mut total, &c.to_file(&mut meter()).unwrap());
    for trust in [Trust::Known, Trust::Unknown] {
        put(&mut total, &opened(&c, trust));
    }
    for key in c.blobs.keys() {
        let len = c.blobs[key].len();
        for at in [0, len / 2, len - 1] {
            let mut d = c.clone();
            d.blobs.get_mut(key).unwrap()[at] ^= 1;
            let s = d.salvage(&mut meter()).unwrap();
            put(&mut total, &s.file);
            put(&mut total, format!("{:?}", s.damaged).as_bytes());
            for trust in [Trust::Known, Trust::Unknown] {
                put(&mut total, &opened(&d, trust));
            }
        }
    }
    total
}

#[test]
fn golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let c = corpus();
        (c.clone().finish(), c)
    });
    println!(
        "geom-format containers golden hash: {}",
        hex(checked.value.clone().finish())
    );
    checked.verdict().unwrap_or_else(|e| panic!("{e}"));
}

/// Chapter 21's thread-count test: the same digest on 1, 2, 3 and 8 threads at once.
#[test]
#[cfg_attr(
    all(target_family = "wasm", not(target_feature = "atomics")),
    ignore = "WebAssembly without the atomics feature (wasm32-wasip1) has no threads: \
              std::thread::scope cannot spawn"
)]
fn golden_hash_is_the_same_on_every_thread_count() {
    let one = hex(corpus().finish());
    for threads in golden::THREAD_COUNTS {
        let digests: Vec<String> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| s.spawn(|| hex(corpus().finish())))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(
            digests.iter().all(|d| *d == one),
            "{threads} threads: {digests:?}"
        );
    }
}
