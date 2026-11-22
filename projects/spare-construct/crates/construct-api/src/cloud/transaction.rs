//! Every command fully applies or fully rolls back (CON-A2-F5-P2; ARCHITECTURE.md, "Transactions").
//!
//! The orderer runs a whole batch of edits in one store transaction, and a command that is turned
//! back is still an entry of that batch. So a command never writes into the batch's transaction
//! directly: it runs on a [`Staged`] transaction over it, which holds every write until the
//! command has answered. [`ProjectCommands`](crate::cloud::ProjectCommands) hands the writes on
//! when the answer is that it applied, and drops them when it was turned back, so a refusal found
//! half way through a command (a type that cannot take a parameter after the table was written,
//! an undo whose model check fails after its records were written) leaves nothing behind.
//!
//! Reads see the transaction underneath plus the writes held here, in key order, as a store
//! transaction sees its own writes. Depends on kit-store's traits only; nothing here reads a clock.

use kit_store::{Key, Pair, Prefix, Reader, StoreError, Txn};
use std::collections::BTreeMap;
use std::future::ready;

/// A command's writes, held over the batch's transaction until the command answers.
pub struct Staged<'a, T: Txn> {
    txn: &'a mut T,
    writes: BTreeMap<String, Option<Vec<u8>>>,
}

impl<'a, T: Txn> Staged<'a, T> {
    /// Nothing written yet, over `txn`.
    pub fn new(txn: &'a mut T) -> Self {
        Self {
            txn,
            writes: BTreeMap::new(),
        }
    }

    /// Hands every write held here to the transaction underneath, in key order.
    ///
    /// # Errors
    /// The transaction underneath failed to take a write.
    pub async fn keep(self) -> Result<(), StoreError> {
        let Self { txn, writes } = self;
        for (key, value) in writes {
            let key = Key::from_stored(key);
            match value {
                Some(value) => txn.put(key, value).await?,
                None => txn.delete(key).await?,
            }
        }
        Ok(())
    }

    /// Pairs read underneath, with the writes held here between `from` and `to` laid over them.
    fn over(&self, pairs: Vec<Pair>, from: &[u8], to: &[u8]) -> Vec<Pair> {
        let mut found: BTreeMap<String, Vec<u8>> = pairs
            .into_iter()
            .map(|(key, value)| (key.as_str().to_owned(), value))
            .collect();
        for (key, value) in &self.writes {
            let bytes = key.as_bytes();
            if bytes < from || bytes >= to {
                continue;
            }
            match value {
                Some(value) => found.insert(key.clone(), value.clone()),
                None => found.remove(key),
            };
        }
        found
            .into_iter()
            .map(|(key, value)| (Key::from_stored(key), value))
            .collect()
    }
}

impl<T: Txn> Reader for Staged<'_, T> {
    async fn get(&mut self, key: &Key) -> Result<Option<Vec<u8>>, StoreError> {
        match self.writes.get(key.as_str()) {
            Some(value) => Ok(value.clone()),
            None => self.txn.get(key).await,
        }
    }

    async fn scan(&mut self, prefix: &Prefix) -> Result<Vec<Pair>, StoreError> {
        let pairs = self.txn.scan(prefix).await?;
        let (from, to) = prefix.bounds();
        Ok(self.over(pairs, &from, &to))
    }

    async fn scan_between(&mut self, from: &Key, to: &Key) -> Result<Vec<Pair>, StoreError> {
        let pairs = self.txn.scan_between(from, to).await?;
        Ok(self.over(pairs, from.as_bytes(), to.as_bytes()))
    }
}

impl<T: Txn> Txn for Staged<'_, T> {
    fn put(
        &mut self,
        key: Key,
        value: Vec<u8>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        self.writes.insert(key.as_str().to_owned(), Some(value));
        ready(Ok(()))
    }

    fn delete(&mut self, key: Key) -> impl Future<Output = Result<(), StoreError>> + Send {
        self.writes.insert(key.as_str().to_owned(), None);
        ready(Ok(()))
    }

    /// The same as [`Staged::keep`].
    async fn commit(self) -> Result<(), StoreError> {
        self.keep().await
    }

    /// Drops every write held here.
    fn rollback(self) -> impl Future<Output = Result<(), StoreError>> + Send {
        ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;
    use kit_store::memory::Memory;
    use kit_store::{Kv, table, table_prefix};

    fn key(name: &str) -> Key {
        table("p:x", "t", &[name])
    }

    fn all() -> Prefix {
        table_prefix("p:x", "t", &[])
    }

    async fn seeded() -> Memory {
        let store = Memory::new();
        let mut txn = store.begin().await.unwrap();
        txn.put(key("a"), b"1".to_vec()).await.unwrap();
        txn.put(key("b"), b"2".to_vec()).await.unwrap();
        txn.commit().await.unwrap();
        store
    }

    async fn rows(store: &Memory) -> Vec<(String, Vec<u8>)> {
        let mut snapshot = store.snapshot().await.unwrap();
        snapshot
            .scan(&all())
            .await
            .unwrap()
            .into_iter()
            .map(|(key, value)| (key.as_str().to_owned(), value))
            .collect()
    }

    #[tokio::test]
    async fn writes_are_seen_while_held_and_gone_when_dropped() {
        let store = seeded().await;
        let before = rows(&store).await;
        let mut txn = store.begin().await.unwrap();
        {
            let mut staged = Staged::new(&mut txn);
            staged.put(key("c"), b"3".to_vec()).await.unwrap();
            staged.delete(key("a")).await.unwrap();
            assert_eq!(staged.get(&key("c")).await.unwrap(), Some(b"3".to_vec()));
            assert_eq!(staged.get(&key("a")).await.unwrap(), None);
            let seen: Vec<_> = staged
                .scan(&all())
                .await
                .unwrap()
                .into_iter()
                .map(|(key, _)| key)
                .collect();
            assert_eq!(seen, vec![key("b"), key("c")]);
            let between = staged.scan_between(&key("b"), &key("d")).await.unwrap();
            assert_eq!(between.len(), 2);
            staged.rollback().await.unwrap();
        }
        assert_eq!(txn.get(&key("c")).await.unwrap(), None);
        txn.commit().await.unwrap();
        assert_eq!(rows(&store).await, before, "nothing of it landed");
    }

    #[tokio::test]
    async fn kept_writes_land_with_the_transaction_underneath() {
        let store = seeded().await;
        let mut txn = store.begin().await.unwrap();
        let mut staged = Staged::new(&mut txn);
        staged.put(key("c"), b"3".to_vec()).await.unwrap();
        staged.delete(key("a")).await.unwrap();
        staged.keep().await.unwrap();
        txn.commit().await.unwrap();
        let keys: Vec<String> = rows(&store).await.into_iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec![key("b").as_str(), key("c").as_str()]);
    }
}
