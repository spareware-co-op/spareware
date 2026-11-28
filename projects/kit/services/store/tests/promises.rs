//! TiKV keeps the promises `kit-store` asks of a store. These are the same tests as kit-store's
//! `tests/store.rs`, on the real thing, plus what only the real thing can show. They need a
//! running TiKV at STACK_PD, and each uses a project of its own, so they leave the rest alone.

use kit_core::Id;
use kit_store::key::list;
use kit_store::{Kv, Reader, Scope, StoreError, Txn};
use kit_store_tikv::Tikv;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;

async fn tikv() -> Tikv {
    let pd = std::env::var("STACK_PD").unwrap_or_else(|_| "127.0.0.1:2379".to_owned());
    let endpoints: Vec<String> = pd.split(',').map(str::to_owned).collect();
    Tikv::connect(&endpoints)
        .await
        .expect("TiKV should be running")
}

/// A scope no earlier run has used.
fn fresh(name: &str) -> Scope {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    Scope::Project(Id::parse(&format!("promise-{name}-{}", now.as_nanos())).unwrap())
}

#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn a_snapshot_stays_at_its_moment() {
    let store = tikv().await;
    let key = fresh("snapshot").head();
    let mut txn = store.begin().await.unwrap();
    txn.put(key.clone(), b"1".to_vec()).await.unwrap();
    txn.commit().await.unwrap();

    let mut snapshot = store.snapshot().await.unwrap();
    let mut txn = store.begin().await.unwrap();
    txn.put(key.clone(), b"2".to_vec()).await.unwrap();
    txn.commit().await.unwrap();
    assert_eq!(snapshot.get(&key).await.unwrap(), Some(b"1".to_vec()));
}

/// The promise the whole log rests on, and that the conflict is recognised as one.
#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn two_writers_of_one_key_cant_both_win() {
    let store = tikv().await;
    let key = fresh("conflict").head();
    let mut first = store.begin().await.unwrap();
    let mut second = store.begin().await.unwrap();
    first.put(key.clone(), b"1".to_vec()).await.unwrap();
    second.put(key.clone(), b"2".to_vec()).await.unwrap();
    first.commit().await.unwrap();
    assert_eq!(second.commit().await, Err(StoreError::Conflict));
    let mut snapshot = store.snapshot().await.unwrap();
    assert_eq!(snapshot.get(&key).await.unwrap(), Some(b"1".to_vec()));
}

#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn a_transaction_sees_its_own_writes() {
    let store = tikv().await;
    let scope = fresh("own");
    let mut txn = store.begin().await.unwrap();
    txn.put(scope.log(1), b"a".to_vec()).await.unwrap();
    txn.put(scope.log(2), b"b".to_vec()).await.unwrap();
    txn.delete(scope.log(1)).await.unwrap();
    assert_eq!(txn.get(&scope.log(1)).await.unwrap(), None);
    let seen = txn.scan(&list::log(&scope)).await.unwrap();
    assert_eq!(seen, vec![(scope.log(2), b"b".to_vec())]);
    txn.rollback().await.unwrap();
    assert_eq!(
        store
            .snapshot()
            .await
            .unwrap()
            .get(&scope.log(2))
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn a_scan_reads_past_a_full_page() {
    let store = tikv().await;
    let scope = fresh("pages");
    const ROWS: u64 = 5000;
    for chunk in (1..=ROWS).collect::<Vec<_>>().chunks(1000) {
        let mut txn = store.begin().await.unwrap();
        for n in chunk {
            txn.put(scope.log(*n), n.to_string().into_bytes())
                .await
                .unwrap();
        }
        txn.commit().await.unwrap();
    }
    let mut snapshot = store.snapshot().await.unwrap();
    let rows = snapshot.scan(&list::log(&scope)).await.unwrap();
    assert_eq!(rows.len() as u64, ROWS);
    assert!(
        rows.windows(2).all(|pair| pair[0].0 < pair[1].0),
        "in order"
    );
    let some = snapshot
        .scan_between(&scope.log(10), &scope.log(20))
        .await
        .unwrap();
    assert_eq!(some.len(), 10, "up to, and not including, the end");
}

#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn a_dropped_transaction_leaves_nothing_and_does_no_harm() {
    let store = tikv().await;
    let key = fresh("dropped").head();
    {
        let mut txn = store.begin().await.unwrap();
        txn.put(key.clone(), b"1".to_vec()).await.unwrap();
    }
    assert_eq!(
        store.snapshot().await.unwrap().get(&key).await.unwrap(),
        None
    );
}

/// A writer that dies between the two phases of its commit leaves its locks behind until their
/// time to live runs out: three seconds in tikv-client for a small transaction. A read that meets
/// one waits for that and reads past it. Issue #762: a server killed mid-commit left its locks on
/// a project's `head`, and the next page read gave up after tikv-client's default 1.5 s.
#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn a_read_waits_out_the_locks_of_a_writer_that_died() {
    let store = tikv().await;
    let scope = fresh("dead-writer");
    let scenario = fail::FailScenario::setup();
    // Stops each commit after its locks are written, as killing the writer there would, and
    // stops the rollback a failed commit is given, which a dead writer never sends.
    fail::cfg("after-prewrite", "return").unwrap();
    fail::cfg("before-rollback", "return").unwrap();
    let mut txn = store.begin().await.unwrap();
    txn.put(scope.head(), b"1".to_vec()).await.unwrap();
    txn.put(scope.log(1), b"a".to_vec()).await.unwrap();
    let died = txn.commit().await;
    fail::remove("after-prewrite");
    fail::remove("before-rollback");
    assert!(died.is_err(), "the commit should stop after its prewrite");

    let mut snapshot = store.snapshot().await.unwrap();
    assert_eq!(snapshot.get(&scope.head()).await, Ok(None));
    assert_eq!(snapshot.scan(&list::log(&scope)).await, Ok(vec![]));
    scenario.teardown();
}

/// A commit that loses a write conflict on its primary key, after another of its prewrite batches
/// has locked a key no one else wrote, leaves no lock there. Two servers regenerating one project
/// met exactly this: a transaction's first key (its primary) lost to the other server, a second
/// key in a batch of its own kept its lock, and every read of that key failed with `TxnNotFound`
/// until the lock ran out, `verify` among them.
///
/// The overlap is forced, not waited for: the loser writes its primary first and big enough
/// (over `tikv-client`'s 16 KiB batch) to go alone, the winner commits that key in between, and
/// the read comes straight after, well inside the lock's three seconds.
#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn a_commit_that_loses_a_conflict_leaves_no_locks() {
    let store = tikv().await;
    let scope = fresh("lost-conflict");
    let mut loser = store.begin().await.unwrap();
    loser
        .put(scope.head(), vec![b'x'; 20 * 1024])
        .await
        .unwrap();
    loser.put(scope.log(1), b"a".to_vec()).await.unwrap();
    let mut winner = store.begin().await.unwrap();
    winner.put(scope.head(), b"1".to_vec()).await.unwrap();
    winner.commit().await.unwrap();
    assert_eq!(loser.commit().await, Err(StoreError::Conflict));

    let mut snapshot = store.snapshot().await.unwrap();
    assert_eq!(snapshot.get(&scope.log(1)).await, Ok(None));
    assert_eq!(snapshot.get(&scope.head()).await, Ok(Some(b"1".to_vec())));
}

/// Leaves `log(1)` locked by a writer whose primary, `head`, holds no lock of that writer: the
/// state two writers of the same keys put each other in when their prewrite batches cross (each
/// holds a lock the other's other batch needs, and one holds the primary both name), and the
/// state a writer killed between its batches leaves. It is made here as the conflict test above
/// makes it, with the loser stopped before the rollback its failed commit is given, so the lock
/// stays until its time to live, three seconds, runs out.
async fn strand_a_lock_with_no_primary(store: &Tikv, scope: &Scope) {
    let mut loser = store.begin().await.unwrap();
    loser
        .put(scope.head(), vec![b'x'; 20 * 1024])
        .await
        .unwrap();
    loser.put(scope.log(1), b"a".to_vec()).await.unwrap();
    let mut winner = store.begin().await.unwrap();
    winner.put(scope.head(), b"1".to_vec()).await.unwrap();
    winner.commit().await.unwrap();
    fail::cfg("before-rollback", "return").unwrap();
    let lost = loser.commit().await;
    fail::remove("before-rollback");
    assert_eq!(lost, Err(StoreError::Conflict));
}

/// A read that meets a lock whose primary holds no lock of its writer waits until the lock can
/// be cleared, and reads past it. `tikv-client` asks after such a primary for 1.5 s and then
/// gives the read up with `TxnNotFound`, half the lock's three seconds, so a page's read that met
/// one failed: "Something went wrong."
#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn a_read_waits_out_a_lock_whose_primary_holds_none() {
    let store = tikv().await;
    let scope = fresh("no-primary-read");
    let scenario = fail::FailScenario::setup();
    strand_a_lock_with_no_primary(&store, &scope).await;

    let mut snapshot = store.snapshot().await.unwrap();
    assert_eq!(snapshot.get(&scope.log(1)).await, Ok(None));
    assert_eq!(snapshot.scan(&list::log(&scope)).await, Ok(vec![]));
    let mut txn = store.begin().await.unwrap();
    assert_eq!(txn.get(&scope.log(1)).await, Ok(None));
    txn.rollback().await.unwrap();
    scenario.teardown();
}

/// A commit that meets such a lock, on a key it writes, while it holds the primary that lock
/// names, is told another writer got there first, and lands when it is run again, as every
/// caller runs a conflict again. This is what two passes of one project's regeneration met when
/// their batches crossed: both write the same rows and name the same first row as their primary,
/// and the one that held it failed with `TxnNotFound`, which reached a person as a failed read.
#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn a_commit_that_meets_a_lock_naming_its_own_primary_is_a_conflict() {
    let store = tikv().await;
    let scope = fresh("no-primary-commit");
    let scenario = fail::FailScenario::setup();
    strand_a_lock_with_no_primary(&store, &scope).await;

    let mut outcomes = Vec::new();
    for _ in 0..10 {
        let mut txn = store.begin().await.unwrap();
        txn.put(scope.head(), vec![b'y'; 20 * 1024]).await.unwrap();
        txn.put(scope.log(1), b"b".to_vec()).await.unwrap();
        let outcome = txn.commit().await;
        let landed = outcome.is_ok();
        outcomes.push(outcome);
        if landed {
            break;
        }
    }
    let (landed, earlier) = outcomes.split_last().unwrap();
    assert_eq!(landed, &Ok(()), "every try: {outcomes:?}");
    assert!(
        earlier.iter().all(|e| e == &Err(StoreError::Conflict)),
        "every try: {outcomes:?}"
    );
    let mut snapshot = store.snapshot().await.unwrap();
    assert_eq!(snapshot.get(&scope.log(1)).await, Ok(Some(b"b".to_vec())));
    scenario.teardown();
}

/// Two writers of the same rows at once, each naming the same first row as its primary and each
/// row in a prewrite batch of its own, as two passes of one project's regeneration are: however
/// their batches cross, each commit lands or is told another writer got there first, at most one
/// lands, and a read made meanwhile reads. On a development machine the batches crossed in about
/// half of such pairs; before, the writer holding the primary failed with `TxnNotFound`, and so
/// did a read that met the other's lock, in 16 commits and 6 reads of 30 pairs.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs a running TiKV"]
async fn two_writers_of_the_same_rows_at_once_end_in_one_win_or_conflicts() {
    const PAIRS: usize = 20;
    const ROWS: u64 = 6;
    let store = tikv().await;
    for _ in 0..PAIRS {
        let scope = fresh("crossed");
        let start = Arc::new(Barrier::new(3));
        let mut writers = Vec::new();
        for writer in *b"ab" {
            let (store, scope, start) = (store.clone(), scope.clone(), start.clone());
            writers.push(tokio::spawn(async move {
                let mut txn = store.begin().await.unwrap();
                txn.put(scope.head(), vec![writer; 20 * 1024])
                    .await
                    .unwrap();
                for n in 1..=ROWS {
                    txn.put(scope.log(n), vec![writer; 20 * 1024])
                        .await
                        .unwrap();
                }
                start.wait().await;
                (writer, txn.commit().await)
            }));
        }
        let reader = {
            let (store, scope, start) = (store.clone(), scope.clone(), start.clone());
            tokio::spawn(async move {
                start.wait().await;
                tokio::time::sleep(Duration::from_millis(20)).await;
                let mut snapshot = store.snapshot().await.unwrap();
                snapshot.get(&scope.log(ROWS)).await
            })
        };
        let mut won = Vec::new();
        for writer in writers {
            match writer.await.unwrap() {
                (writer, Ok(())) => won.push(writer),
                (_, Err(StoreError::Conflict)) => {}
                (writer, Err(other)) => panic!("writer {}: {other}", char::from(writer)),
            }
        }
        assert!(won.len() <= 1, "both writers landed");
        let read = reader.await.unwrap();
        assert!(read.is_ok(), "the read failed: {read:?}");
        let rows = store
            .snapshot()
            .await
            .unwrap()
            .scan(&list::log(&scope))
            .await
            .unwrap();
        let wanted: Vec<_> = won
            .iter()
            .flat_map(|writer| (1..=ROWS).map(|n| (scope.log(n), vec![*writer; 20 * 1024])))
            .collect();
        assert_eq!(rows, wanted, "the rows are the winner's, or nobody's");
    }
}

#[tokio::test]
#[ignore = "needs a running TiKV"]
async fn garbage_collection_leaves_the_live_values() {
    let store = tikv().await;
    let key = fresh("gc").head();
    for value in [b"1", b"2", b"3"] {
        let mut txn = store.begin().await.unwrap();
        txn.put(key.clone(), value.to_vec()).await.unwrap();
        txn.commit().await.unwrap();
    }
    store.collect_garbage(0).await.unwrap();
    assert_eq!(
        store.snapshot().await.unwrap().get(&key).await.unwrap(),
        Some(b"3".to_vec())
    );
}
