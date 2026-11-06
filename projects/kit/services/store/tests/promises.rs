//! TiKV keeps the promises `kit-store` asks of a store. These are the same tests as kit-store's
//! `tests/store.rs`, on the real thing, plus what only the real thing can show. They need a
//! running TiKV at STACK_PD, and each uses a project of its own, so they leave the rest alone.

use kit_core::Id;
use kit_store::key::list;
use kit_store::{Kv, Reader, Scope, StoreError, Txn};
use kit_store_tikv::Tikv;

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
