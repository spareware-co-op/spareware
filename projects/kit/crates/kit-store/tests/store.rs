//! The promises of the store trait, on the in-memory store: every backend must keep these. The
//! same promises are checked on TiKV by the backend's own tests.

use kit_core::Id;
use kit_store::key::list;
use kit_store::memory::{Fault, Memory};
use kit_store::{Kv, Reader, Scope, StoreError, Txn};

fn scope() -> Scope {
    Scope::Project(Id::parse("x").unwrap())
}

#[tokio::test]
async fn a_snapshot_stays_at_its_moment() {
    let store = Memory::new();
    let key = scope().head();
    let mut txn = store.begin().await.unwrap();
    txn.put(key.clone(), b"1".to_vec()).await.unwrap();
    txn.commit().await.unwrap();

    let mut snapshot = store.snapshot().await.unwrap();
    let mut txn = store.begin().await.unwrap();
    txn.put(key.clone(), b"2".to_vec()).await.unwrap();
    txn.commit().await.unwrap();
    assert_eq!(snapshot.get(&key).await.unwrap(), Some(b"1".to_vec()));
}

#[tokio::test]
async fn two_writers_of_one_key_cant_both_win() {
    let store = Memory::new();
    let key = scope().head();
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
async fn a_transaction_sees_its_own_writes() {
    let store = Memory::new();
    let scope = scope();
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
async fn a_lost_answer_still_landed() {
    let store = Memory::new();
    let key = scope().head();
    store.inject(Fault::LandsAnswerLost);
    let mut txn = store.begin().await.unwrap();
    txn.put(key.clone(), b"1".to_vec()).await.unwrap();
    assert!(matches!(
        txn.commit().await,
        Err(StoreError::Undetermined(_))
    ));
    assert_eq!(
        store.snapshot().await.unwrap().get(&key).await.unwrap(),
        Some(b"1".to_vec())
    );
}

#[tokio::test]
async fn a_failed_commit_leaves_nothing() {
    let store = Memory::new();
    let key = scope().head();
    store.inject(Fault::Fails);
    let mut txn = store.begin().await.unwrap();
    txn.put(key.clone(), b"1".to_vec()).await.unwrap();
    assert!(txn.commit().await.is_err());
    assert_eq!(
        store.snapshot().await.unwrap().get(&key).await.unwrap(),
        None
    );
}
