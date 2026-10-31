//! Cache keys and the memo store (G105, 23.8): a key's bytes against an encoding written out by
//! hand, every field changing the key, the entries never stored, a hit only where a fresh run
//! gives the same answer, and the store's byte bound and forgetting order.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test"
)]

use geom_format::cache::{CacheEntry, CacheKey, MemoStore};
use geom_format::content::ContentId;
use geom_num::{Exactness, GridStep, Steps};
use geom_op::hash::sha256;
use geom_op::{
    Class, Family, OpError, OpErrorDetail, OpId, OpOutput, OpPath, OpSchema, Precision, RunDetail,
};
use geom_run::{Budget, BudgetKind, Exceeded, Phase, Usage};

const SCHEMA: OpSchema = OpSchema {
    name: "make.box",
    version: 1,
    family: Family::Make,
    doc: "A box.",
    inputs: &[],
    outputs: &[],
    roles: &[],
    errors: &[],
    diagnostics: &[],
    classes: &[Class::Exact],
    charges: &[],
    batch: false,
    name_free: true,
    example: "a box of 1 m",
};

fn path() -> OpPath {
    OpPath::new(OpId::from_bytes([7; 16]))
}

fn input(b: u8) -> ContentId {
    ContentId::of_payload(&[b])
}

fn key(
    edition: u64,
    schema: &OpSchema,
    path: &OpPath,
    p: Precision,
    ins: &[ContentId],
) -> CacheKey {
    CacheKey::new(edition, schema, path, p, ins).unwrap()
}

fn free_key(ins: &[ContentId]) -> CacheKey {
    CacheKey::name_free(0, &SCHEMA, Precision::Final, ins)
        .unwrap()
        .unwrap()
}

#[test]
fn a_schema_that_uses_names_gets_no_name_free_key() {
    let named = OpSchema {
        name_free: false,
        ..SCHEMA
    };
    assert_eq!(
        CacheKey::name_free(0, &named, Precision::Final, &[input(1)]),
        Ok(None)
    );
}

/// The key of a call written out byte by byte: `[0, "make.box", 1, [h'07..07'], 0, [[1, h'..']]]`.
#[test]
fn a_key_is_the_sha256_of_its_encoding_written_by_hand() {
    let digest = sha256(&[5]);
    let mut bytes = vec![0x86, 0x00, 0x68];
    bytes.extend_from_slice(b"make.box");
    bytes.extend_from_slice(&[0x01, 0x81, 0x50]);
    bytes.extend_from_slice(&[7; 16]);
    bytes.extend_from_slice(&[0x00, 0x81, 0x82, 0x01, 0x58, 0x20]);
    bytes.extend_from_slice(&digest);
    let k = key(0, &SCHEMA, &path(), Precision::Final, &[input(5)]);
    assert_eq!(k.to_bytes(), sha256(&bytes));
    // The name-free key leaves the path out, so its array has five items.
    let mut free = vec![0x85, 0x00, 0x68];
    free.extend_from_slice(b"make.box");
    free.extend_from_slice(&[0x01, 0x00, 0x81, 0x82, 0x01, 0x58, 0x20]);
    free.extend_from_slice(&digest);
    let f = free_key(&[input(5)]);
    assert_eq!(f.to_bytes(), sha256(&free));
    // A preview writes `[1, bound, step in quarter nanometres]`.
    let preview = Precision::Preview {
        bound: Steps(3),
        step: GridStep::Q0,
    };
    let mut p = vec![0x86, 0x00, 0x68];
    p.extend_from_slice(b"make.box");
    p.extend_from_slice(&[0x01, 0x81, 0x50]);
    p.extend_from_slice(&[7; 16]);
    p.extend_from_slice(&[0x83, 0x01, 0x03, 0x19, 0x02, 0x71, 0x80]);
    assert_eq!(
        key(0, &SCHEMA, &path(), preview, &[]).to_bytes(),
        sha256(&p)
    );
}

#[test]
fn every_field_changes_the_key() {
    let base = key(0, &SCHEMA, &path(), Precision::Final, &[input(1), input(2)]);
    let v2 = OpSchema {
        version: 2,
        ..SCHEMA
    };
    let renamed = OpSchema {
        name: "make.cube",
        ..SCHEMA
    };
    let preview = Precision::Preview {
        bound: Steps(1),
        step: GridStep::Q0,
    };
    let others = [
        key(1, &SCHEMA, &path(), Precision::Final, &[input(1), input(2)]),
        key(0, &v2, &path(), Precision::Final, &[input(1), input(2)]),
        key(
            0,
            &renamed,
            &path(),
            Precision::Final,
            &[input(1), input(2)],
        ),
        key(
            0,
            &SCHEMA,
            &path().step("cut").unwrap(),
            Precision::Final,
            &[input(1), input(2)],
        ),
        key(
            0,
            &SCHEMA,
            &path().item([1; 16]),
            Precision::Final,
            &[input(1), input(2)],
        ),
        key(0, &SCHEMA, &path(), preview, &[input(1), input(2)]),
        key(0, &SCHEMA, &path(), Precision::Final, &[input(2), input(1)]),
        key(0, &SCHEMA, &path(), Precision::Final, &[input(1)]),
        key(0, &SCHEMA, &path(), Precision::Final, &[input(1), input(3)]),
        free_key(&[input(1), input(2)]),
    ];
    for (i, k) in others.iter().enumerate() {
        assert_ne!(*k, base, "field {i}");
        for (j, other) in others.iter().enumerate().skip(i + 1) {
            assert_ne!(k, other, "fields {i} and {j}");
        }
    }
    // The name-free key ignores the path; the full key does not.
    let elsewhere = OpPath::new(OpId::from_bytes([8; 16]));
    assert_eq!(free_key(&[input(1)]), free_key(&[input(1)]));
    assert_ne!(
        key(0, &SCHEMA, &path(), Precision::Final, &[]),
        key(0, &SCHEMA, &elsewhere, Precision::Final, &[])
    );
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "an answer as a store holds it, which is a Result"
)]
fn ok(v: u32) -> Result<OpOutput<u32>, OpError> {
    Ok(OpOutput::new(v, Exactness::Exact))
}

fn err(detail: RunDetail) -> Result<OpOutput<u32>, OpError> {
    Err(OpError::new(path(), OpErrorDetail::Run(detail)))
}

fn exceeded(budget: Budget) -> Exceeded {
    Exceeded {
        budget,
        kind: BudgetKind::Work,
        phase: Phase::new("faces"),
        used: Usage::new(u128::from(budget.work) + 1, 0),
    }
}

#[test]
fn cancellation_memory_and_batch_totals_are_never_stored() {
    let b = Budget::new(10, 10);
    let u = Usage::new(1, 1);
    assert!(CacheEntry::new(err(RunDetail::Cancelled), b, u).is_none());
    assert!(CacheEntry::new(err(RunDetail::OutOfMemory), b, u).is_none());
    assert!(CacheEntry::new(err(RunDetail::BatchTotal(exceeded(b))), b, u).is_none());
    assert!(CacheEntry::new(err(RunDetail::BudgetExceeded(exceeded(b))), b, u).is_some());
    assert!(CacheEntry::new(err(RunDetail::Internal { what: "x" }), b, u).is_some());
    assert!(CacheEntry::new(ok(1), b, u).is_some());
}

#[test]
fn a_hit_needs_a_budget_that_covers_what_the_run_used() {
    let entry = CacheEntry::new(ok(4), Budget::new(100, 100), Usage::new(30, 20)).unwrap();
    assert_eq!(entry.used(), Usage::new(30, 20));
    // Exactly what was used, or more, of each kind: the same answer.
    for b in [
        Budget::new(30, 20),
        Budget::new(31, 20),
        Budget::new(30, 21),
        Budget::UNLIMITED,
    ] {
        assert_eq!(entry.answer(b), Some(&ok(4)), "{b:?}");
    }
    // One less of either kind: a fresh run would stop, so the caller runs it.
    assert_eq!(entry.answer(Budget::new(29, 20)), None);
    assert_eq!(entry.answer(Budget::new(30, 19)), None);
    // A deterministic error is answered the same way.
    let failed = CacheEntry::new(
        err(RunDetail::Internal { what: "x" }),
        Budget::new(9, 9),
        Usage::new(5, 5),
    )
    .unwrap();
    assert!(failed.answer(Budget::new(5, 5)).is_some());
    assert!(failed.answer(Budget::new(4, 5)).is_none());
}

#[test]
fn a_stored_budget_failure_answers_only_its_own_budget() {
    let b = Budget::new(10, 50);
    let stopped = err(RunDetail::BudgetExceeded(exceeded(b)));
    let entry = CacheEntry::new(stopped.clone(), b, Usage::new(11, 3)).unwrap();
    assert_eq!(entry.answer(b), Some(&stopped));
    for other in [
        Budget::new(9, 50),
        Budget::new(10, 49),
        Budget::new(11, 50),
        Budget::UNLIMITED,
    ] {
        assert_eq!(entry.answer(other), None, "{other:?}");
    }
}

fn k(i: u8) -> CacheKey {
    key(0, &SCHEMA, &path(), Precision::Final, &[input(i)])
}

fn entry(v: u32) -> CacheEntry<u32> {
    CacheEntry::new(ok(v), Budget::UNLIMITED, Usage::new(1, 1)).unwrap()
}

#[test]
fn the_store_keeps_to_its_bound_and_forgets_the_oldest_use_first() {
    let mut store = MemoStore::new(10);
    assert!(store.is_empty());
    assert_eq!(store.bound(), 10);
    assert!(store.insert(k(1), entry(1), 4));
    let empty = store.is_empty();
    assert!(!empty);
    assert!(store.insert(k(2), entry(2), 4));
    assert_eq!((store.len(), store.held()), (2, 8));
    // A hit on 1 makes 2 the oldest use, so 2 goes when 3 needs room.
    assert_eq!(store.get(&k(1), Budget::UNLIMITED), Some(&ok(1)));
    assert!(store.insert(k(3), entry(3), 4));
    assert_eq!(store.keys().copied().collect::<Vec<_>>(), [k(1), k(3)]);
    assert_eq!(store.held(), 8);
    assert_eq!(store.get(&k(2), Budget::UNLIMITED), None);
    // A miss for want of budget refreshes nothing: 1 is now the oldest and goes first.
    let tight = CacheEntry::new(ok(4), Budget::UNLIMITED, Usage::new(50, 1)).unwrap();
    assert!(store.insert(k(4), tight, 2));
    assert_eq!(store.get(&k(4), Budget::new(10, 10)), None);
    assert_eq!(
        store.keys().copied().collect::<Vec<_>>(),
        [k(1), k(3), k(4)]
    );
    // 5 needs four bytes: 1, the oldest use, goes, and that is enough.
    assert!(store.insert(k(5), entry(5), 4));
    assert_eq!(
        store.keys().copied().collect::<Vec<_>>(),
        [k(3), k(4), k(5)]
    );
    assert_eq!(store.held(), 10);
    // An entry larger than the bound is refused and changes nothing.
    assert!(!store.insert(k(6), entry(6), 11));
    assert_eq!((store.len(), store.held()), (3, 10));
    // Storing a key again replaces its entry and its size.
    assert!(store.insert(k(5), entry(50), 1));
    assert_eq!((store.len(), store.held()), (3, 7));
    assert_eq!(store.get(&k(5), Budget::UNLIMITED), Some(&ok(50)));
    // An entry of exactly the bound empties the rest.
    assert!(store.insert(k(7), entry(7), 10));
    assert_eq!(store.keys().copied().collect::<Vec<_>>(), [k(7)]);
    assert_eq!(store.held(), 10);
    store.forget(&k(7));
    store.forget(&k(7));
    assert!(store.is_empty());
    assert_eq!(store.held(), 0);
}

/// The same calls in the same order leave two stores the same, entry for entry.
#[test]
fn two_stores_given_the_same_calls_hold_the_same_entries() {
    let run = || {
        let mut store = MemoStore::new(20);
        for i in 0..40u8 {
            store.insert(k(i % 9), entry(u32::from(i)), u64::from(i % 5) + 1);
            store.get(&k(i % 4), Budget::UNLIMITED);
        }
        let held: Vec<_> = store
            .keys()
            .map(|key| (*key, store.clone().get(key, Budget::UNLIMITED).cloned()))
            .collect();
        (held, store.held())
    };
    let (a, held) = run();
    assert_eq!((a.clone(), held), run());
    assert!(held <= 20);
    assert_ne!(a.len(), 0);
}
