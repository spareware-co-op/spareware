//! The replay hash of piece G109: `PruneHistory` run over a fixed corpus, its kept steps hashed
//! with SHA-256, must give the same digest on every target, so every replica prunes alike
//! (chapters 21, 24.4 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed `SplitMix64` stream: 200 element histories of one to six steps, each
//! step keeping, modifying, splitting, merging, deleting, or deleting while generating every name
//! of the body before it, with outputs generated from nothing; a held set of up to eight names of
//! the history, some never in it; and every kept step's path, history (its canonical encoding),
//! operation and held set, written as bytes. Any target that keeps a record, a deletion or a step
//! differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use geom_name::prune;
use geom_name::resolve::Step;
use geom_op::name::encode_op_path;
use geom_op::{History, NameId, NameSet, OpId, OpPath, Origin, PieceKey, Record, Role, Side};
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-name/prune";

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

fn n(i: u64) -> NameId {
    let mut b = [0u8; 16];
    b[8..].copy_from_slice(&i.to_be_bytes());
    NameId::from_bytes(b)
}

fn op(i: u64) -> OpPath {
    let mut b = [0x10u8; 16];
    b[8..].copy_from_slice(&i.to_be_bytes());
    OpPath::new(OpId::from_bytes(b))
}

fn one(name: NameId) -> NameSet {
    NameSet::new([name]).unwrap()
}

/// One step over `body`, its outputs named from `fresh` on: the step's history and the body after.
fn step(rng: &mut SplitMix64, body: &[NameId], fresh: &mut u64) -> (History, Vec<NameId>) {
    let mut new = |fresh: &mut u64| {
        *fresh += 1;
        n(*fresh)
    };
    let cap = Role::new("cap", []).unwrap();
    let mut records = Vec::new();
    let mut deleted = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let x = body[i];
        match below(rng, 7) {
            0 | 1 => records.push(Record {
                output: x,
                origin: Origin::Unchanged,
            }),
            2 => records.push(Record {
                output: new(fresh),
                origin: Origin::Modified { from: one(x) },
            }),
            3 => {
                for side in [Side::Back, Side::Front] {
                    records.push(Record {
                        output: new(fresh),
                        origin: Origin::Split {
                            of: x,
                            piece: PieceKey::ByTools {
                                tools: one(n(0)),
                                side,
                            },
                        },
                    });
                }
            }
            4 if i + 1 < body.len() => {
                records.push(Record {
                    output: new(fresh),
                    origin: Origin::Merged {
                        of: NameSet::new([x, body[i + 1]]).unwrap(),
                    },
                });
                i += 1;
            }
            5 => {
                deleted.push(x);
                records.push(Record {
                    output: new(fresh),
                    origin: Origin::Generated {
                        from: one(x),
                        role: cap.clone(),
                    },
                });
            }
            _ => deleted.push(x),
        }
        i += 1;
    }
    if below(rng, 3) == 0 {
        records.push(Record {
            output: new(fresh),
            origin: Origin::Generated {
                from: NameSet::default(),
                role: cap,
            },
        });
    }
    let history = History::new(records, NameSet::new(deleted).unwrap(), []).unwrap();
    let after = history.records().iter().map(|r| r.output).collect();
    (history, after)
}

fn corpus() -> (Vec<u8>, usize) {
    let mut rng = SplitMix64::new(geom_test::seed::fixed(
        "geom-name prune golden",
        Seed::DEFAULT,
    ));
    let mut bytes = Vec::new();
    let mut kept_records = 0;
    for round in 0..200u64 {
        let mut fresh = 1_000;
        let mut body: Vec<NameId> = (1..=1 + below(&mut rng, 8)).map(n).collect();
        let mut every: BTreeSet<NameId> = body.iter().copied().collect();
        let mut history = Vec::new();
        for k in 0..=below(&mut rng, 6) {
            let (h, after) = step(&mut rng, &body, &mut fresh);
            every.extend(after.iter().copied());
            history.push(Step::Recorded {
                by: op(round * 8 + k),
                history: h,
            });
            body = after;
        }
        let every: Vec<NameId> = every.into_iter().collect();
        let held = NameSet::new(
            (0..below(&mut rng, 9))
                .map(|_| {
                    let i = below(&mut rng, every.len() as u64 + 2) as usize;
                    every.get(i).copied().unwrap_or(n(9_999))
                })
                .collect::<BTreeSet<NameId>>(),
        )
        .unwrap();
        for step in prune(&history, &held, &op(1 << 32 | round)) {
            let Step::Kept {
                by,
                history,
                pruned_by,
                held,
            } = step
            else {
                panic!("a recorded step was not kept");
            };
            kept_records += history.records().len();
            bytes.extend_from_slice(history.encode(&by).unwrap().as_bytes());
            bytes.extend_from_slice(encode_op_path(&pruned_by).unwrap().as_bytes());
            for name in held.names() {
                bytes.extend_from_slice(&name.to_bytes());
            }
            bytes.push(0xff);
        }
    }
    (bytes, kept_records)
}

#[test]
fn prune_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let (bytes, kept) = corpus();
        (sha256(&bytes), (bytes, kept))
    });
    let (bytes, kept) = &checked.value;
    println!(
        "geom-name prune corpus: {kept} kept records, {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert!(*kept > 300, "only {kept} kept records");
    checked.verdict().unwrap_or_else(|e| panic!("{e}"));
}
