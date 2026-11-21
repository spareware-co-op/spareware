//! The replay hash of per-entity error bounds (piece G143): `Bounds::after` and `Bounds::join`
//! along seeded chains of operations, every bound and every refusal written out and hashed with
//! SHA-256. The digest must be the same on every target (chapters 13.2 and 28.13, decision 32.21;
//! the golden store of piece G19).
//!
//! The corpus, from a fixed `SplitMix64` stream, integers only, so the inputs are the same on
//! every target by construction: 12 chains of 24 operations, on `Q0` or `Q1`, under the ladder's
//! limits or a caller's larger ones. Each operation's history keeps, modifies, splits, merges,
//! generates from or deletes each live entity, and makes a few new ones from nothing; about half
//! its changed outputs spend a term of their own (zero among them, which is no term), reported in
//! a shuffled order. Some operations carry a fault: an own term past the step's limit, one on an
//! unchanged output, one on an entity that is not an output, or one output named twice. Every
//! eighth operation joins the chain's bounds with an operand's, made from nothing and sharing
//! some names, and now and then on the other grid step. A refused operation leaves the chain as
//! it was; sums past the ceiling come of the chains' own growth.
//!
//! Written per result: each entity's name, bound and terms (the step's path and its spend), oldest
//! first, the body's largest bound and class; or the refusal's kind, path, detail, entities,
//! message and remedies. A target that orders an entity or a term differently, sums or saturates
//! differently, or refuses a different entity changes the digest. `Bounds` runs on no executor,
//! so there is no thread-count test.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_num::{GridStep, Steps};
use geom_op::bounds::{Bounds, Limits};
use geom_op::detail::{BoundDetail, OpErrorDetail};
use geom_op::{
    History, NameId, NameSet, OpError, OpId, OpPath, Origin, PieceKey, Record, Role, Side,
};
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-op/bounds";

/// The chains, and the operations in each.
const CHAINS: u64 = 12;
const STEPS: u64 = 24;

/// The results the corpus holds and the refusals it meets.
const HELD: u32 = 235;
const REFUSED: u32 = 53;

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A new name from the stream.
fn fresh(rng: &mut SplitMix64) -> NameId {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&rng.next_u64().to_le_bytes());
    bytes[8..].copy_from_slice(&rng.next_u64().to_le_bytes());
    NameId::from_bytes(bytes)
}

fn set(names: &[NameId]) -> NameSet {
    NameSet::new(names.iter().copied()).unwrap()
}

fn record(output: NameId, origin: Origin) -> Record {
    Record { output, origin }
}

fn generated(output: NameId, from: &[NameId]) -> Record {
    record(
        output,
        Origin::Generated {
            from: set(from),
            role: Role::new("fillet", []).unwrap(),
        },
    )
}

/// A spend for one changed output: mostly a few steps, now and then up to the step's limit.
fn spend(rng: &mut SplitMix64, limits: Limits) -> Steps {
    if below(rng, 12) == 0 {
        Steps(below(rng, limits.own.0 + 1))
    } else {
        Steps(below(rng, 3))
    }
}

/// One operation on `alive`: its history and its own terms, in a shuffled order.
fn operation(
    rng: &mut SplitMix64,
    alive: &[NameId],
    limits: Limits,
) -> (History, Vec<(NameId, Steps)>) {
    let mut records = Vec::new();
    let mut deleted = Vec::new();
    // A crowded body loses more than it gains.
    let choices = if alive.len() > 16 { 13 } else { 10 };
    for (i, &e) in alive.iter().enumerate() {
        let other = (alive.len() > 1).then(|| {
            let k = 1 + below(rng, alive.len() as u64 - 1) as usize;
            alive[(i + k) % alive.len()]
        });
        match (below(rng, choices), other) {
            (0..=3, _) => records.push(record(e, Origin::Unchanged)),
            (4, _) | (5 | 7, None) => {
                records.push(record(e, Origin::Modified { from: set(&[e]) }));
            }
            (5, Some(o)) => {
                records.push(record(fresh(rng), Origin::Modified { from: set(&[e, o]) }));
            }
            (6, _) => {
                let tools = set(&[fresh(rng)]);
                for side in [Side::Back, Side::Front] {
                    let piece = PieceKey::ByTools {
                        tools: tools.clone(),
                        side,
                    };
                    records.push(record(fresh(rng), Origin::Split { of: e, piece }));
                }
            }
            (7, Some(o)) => records.push(record(fresh(rng), Origin::Merged { of: set(&[e, o]) })),
            (8, _) => {
                records.push(record(e, Origin::Unchanged));
                records.push(generated(fresh(rng), &[e]));
            }
            _ => deleted.push(e),
        }
    }
    let new = if alive.is_empty() {
        2 + below(rng, 4)
    } else {
        below(rng, 3)
    };
    for _ in 0..new {
        records.push(generated(fresh(rng), &[]));
    }
    let mut own: Vec<(NameId, Steps)> = Vec::new();
    for r in &records {
        if r.origin != Origin::Unchanged && rng.coin() {
            own.push((r.output, spend(rng, limits)));
        }
    }
    let changed: Vec<NameId> = records
        .iter()
        .filter(|r| r.origin != Origin::Unchanged)
        .map(|r| r.output)
        .collect();
    let kept: Vec<NameId> = records
        .iter()
        .filter(|r| r.origin == Origin::Unchanged)
        .map(|r| r.output)
        .collect();
    match below(rng, 24) {
        // Past the step's limit.
        0 if !changed.is_empty() => {
            let e = changed[below(rng, changed.len() as u64) as usize];
            own.retain(|t| t.0 != e);
            own.push((e, Steps(limits.own.0 + 1 + below(rng, 3))));
        }
        // On an unchanged output.
        1 if !kept.is_empty() => {
            let e = kept[below(rng, kept.len() as u64) as usize];
            own.push((e, Steps(1 + below(rng, 4))));
        }
        // On an entity that is not an output.
        2 => own.push((fresh(rng), Steps(1 + below(rng, 4)))),
        // One output twice.
        3 if !changed.is_empty() => {
            let e = changed[below(rng, changed.len() as u64) as usize];
            own.push((e, Steps(1 + below(rng, 4))));
            own.push((e, Steps(1 + below(rng, 4))));
        }
        _ => {}
    }
    for i in (1..own.len()).rev() {
        own.swap(i, below(rng, i as u64 + 1) as usize);
    }
    (History::new(records, set(&deleted), []).unwrap(), own)
}

/// An operand to join: entities made from nothing, some sharing `alive`'s names.
fn operand(
    rng: &mut SplitMix64,
    path: &OpPath,
    alive: &[NameId],
    step: GridStep,
    limits: Limits,
) -> Bounds {
    let mut names = Vec::new();
    for _ in 0..=below(rng, 4) {
        let name = if !alive.is_empty() && rng.coin() {
            alive[below(rng, alive.len() as u64) as usize]
        } else {
            fresh(rng)
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let made = History::new(names.iter().map(|n| generated(*n, &[])), set(&[]), []).unwrap();
    let own: Vec<(NameId, Steps)> = names
        .iter()
        .map(|n| (*n, Steps(1 + below(rng, limits.own.0))))
        .collect();
    Bounds::new(step).after(path, &made, &own, limits).unwrap()
}

fn text(out: &mut Vec<u8>, t: &str) {
    out.extend((t.len() as u64).to_le_bytes());
    out.extend(t.as_bytes());
}

/// Every entity's name, bound and terms, the largest bound and the class.
fn write(out: &mut Vec<u8>, b: &Bounds) {
    text(out, &format!("{:?}", b.step()));
    let entities: Vec<NameId> = b.entities().collect();
    out.extend((entities.len() as u64).to_le_bytes());
    for e in entities {
        out.extend(e.to_bytes());
        out.extend(b.of(e).0.to_le_bytes());
        let terms = b.terms(e);
        out.extend((terms.len() as u64).to_le_bytes());
        for t in terms {
            text(out, &t.step.to_string());
            out.extend(t.bound.0.to_le_bytes());
        }
    }
    out.extend(b.largest().0.to_le_bytes());
    text(out, &format!("{:?}", b.class()));
}

/// The refusal's kind, path, detail, entities, message and remedies.
fn refusal(out: &mut Vec<u8>, e: &OpError) {
    text(out, &format!("{:?}", e.kind()));
    text(out, &e.op().to_string());
    text(out, &format!("{:?}", e.detail()));
    out.extend((e.entities().len() as u64).to_le_bytes());
    for n in e.entities() {
        out.extend(n.to_bytes());
    }
    text(out, e.message());
    text(out, &format!("{:?}", e.remedies()));
}

/// Which of the four refusals `e` is.
fn which(e: &OpError) -> usize {
    match e.detail() {
        OpErrorDetail::Bound(BoundDetail::OverStep { .. }) => 0,
        OpErrorDetail::Bound(BoundDetail::OverCeiling { .. }) => 1,
        OpErrorDetail::Bound(BoundDetail::OwnTermAstray) => 2,
        OpErrorDetail::Bound(BoundDetail::MixedSteps) => 3,
        other => panic!("not a refusal of the bounds: {other:?}"),
    }
}

/// The corpus's canonical bytes, how many results it held, and how many of each refusal.
fn corpus() -> (Vec<u8>, u32, [u32; 4]) {
    let mut rng = SplitMix64::new(Seed(0x0143));
    let mut out = Vec::new();
    let mut held = 0;
    let mut refused = [0; 4];
    let base = OpPath::new(OpId::from_bytes([0x43; 16]))
        .step("chain")
        .unwrap();
    for chain in 0..CHAINS {
        let step = if chain % 3 == 2 {
            GridStep::Q1
        } else {
            GridStep::Q0
        };
        let other_step = if step == GridStep::Q0 {
            GridStep::Q1
        } else {
            GridStep::Q0
        };
        let limits = if chain % 4 == 3 {
            Limits {
                own: Steps(32),
                total: Steps(160 + below(&mut rng, 100)),
            }
        } else {
            Limits::DEFAULT
        };
        let mut bounds = Bounds::new(step);
        let mut alive: Vec<NameId> = Vec::new();
        for i in 0..STEPS {
            let mut key = [0u8; 16];
            key[..8].copy_from_slice(&chain.to_be_bytes());
            key[8..].copy_from_slice(&i.to_be_bytes());
            let path = base.item(key);
            let result = if i % 8 == 7 {
                let on = if below(&mut rng, 6) == 0 {
                    other_step
                } else {
                    step
                };
                let other = operand(&mut rng, &path, &alive, on, limits);
                bounds.join(&path, &other).inspect(|_| {
                    alive.extend(other.entities());
                    alive.sort_unstable();
                    alive.dedup();
                })
            } else {
                let (history, own) = operation(&mut rng, &alive, limits);
                bounds.after(&path, &history, &own, limits).inspect(|_| {
                    alive = history.records().iter().map(|r| r.output).collect();
                })
            };
            match result {
                Ok(after) => {
                    out.push(0);
                    write(&mut out, &after);
                    bounds = after;
                    held += 1;
                }
                Err(e) => {
                    out.push(1);
                    refusal(&mut out, &e);
                    refused[which(&e)] += 1;
                }
            }
        }
    }
    (out, held, refused)
}

#[test]
fn bounds_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.0), rec)
    });
    let (bytes, held, refused) = &checked.value;
    println!(
        "geom-op bounds corpus: {held} held, refused {refused:?} (past a step, past the ceiling, \
         astray, mixed steps), {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert!(refused.iter().all(|n| *n > 0), "every refusal met");
    assert_eq!(
        (*held, refused[0] + refused[1] + refused[2] + refused[3]),
        (HELD, REFUSED),
        "every operation ran"
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target orders an entity or a term differently, \
             sums or saturates differently, or refuses a different entity (compare this log's \
             digest with another target's), or a definition changed in place, which chapter \
             26.2 forbids"
        )
    });
}
