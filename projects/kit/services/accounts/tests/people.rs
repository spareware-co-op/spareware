//! The commands for people, in a directory log. A program answers `permit`; this test's says yes to
//! everyone, because the commands' own checks are what is under test. Spare Construct's server
//! tests the same commands behind its Cedar rules.

use kit_accounts::people;
use kit_core::Id;
use kit_ops::Edit;
use kit_store::memory::Memory;
use kit_store::{Kv, Place, Reader, Scope, StoreError};
use kit_sync::{Commands, Context, Orderers, Outcome, Reply, SystemClock, verify_log};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

struct Everyone;

impl Commands<Memory> for Everyone {
    fn version(&self) -> u32 {
        1
    }

    async fn permit(&self, _: &Scope, _: &Edit) -> Result<Result<(), String>, StoreError> {
        Ok(Ok(()))
    }

    async fn apply(
        &self,
        txn: &mut <Memory as Kv>::Txn,
        context: &Context<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        people::apply(txn, context, edit).await
    }
}

struct Directory {
    store: Memory,
    orderers: Orderers<Memory, Everyone>,
    next: AtomicU64,
}

impl Directory {
    fn new() -> Self {
        let store = Memory::new();
        let orderers = Orderers::new(store.clone(), Arc::new(Everyone), Arc::new(SystemClock), 50);
        Self {
            store,
            orderers,
            next: AtomicU64::new(0),
        }
    }

    async fn edit(&self, person: &str, cmd: &str, args: Value) -> Reply {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        let edit = Edit {
            person: i(person),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(person),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        };
        self.orderers.run(&Scope::Directory, edit).await.unwrap()
    }

    /// A started directory with Anna and Dan.
    async fn started() -> Self {
        let directory = Self::new();
        for (person, cmd, args) in [
            ("system", "directory.start", json!({})),
            (
                "system",
                "person.add",
                json!({ "id": "anna", "name": "Anna", "email_hash": "aa" }),
            ),
            (
                "system",
                "person.add",
                json!({ "id": "dan", "name": "Dan", "email_hash": "dd" }),
            ),
        ] {
            let reply = directory.edit(person, cmd, args).await;
            assert!(matches!(reply, Reply::Ack { .. }), "{cmd}: {reply:?}");
        }
        directory
    }

    async fn has(&self, place: Place) -> bool {
        let mut snapshot = self.store.snapshot().await.unwrap();
        snapshot.get(&place.key()).await.unwrap().is_some()
    }
}

fn turned_back(reply: &Reply, words: &str) -> bool {
    matches!(reply, Reply::Rejected { reason, .. } if reason.contains(words))
}

#[tokio::test]
async fn the_directory_can_be_started_only_once() {
    let directory = Directory::started().await;
    let reply = directory
        .edit("mallory", "directory.start", json!({}))
        .await;
    assert!(turned_back(&reply, "already been started"));
    assert!(!directory.has(Place::Admin(i("mallory"))).await);
    assert!(directory.has(Place::Admin(i("system"))).await);
}

#[tokio::test]
async fn starting_records_the_store_format() {
    let directory = Directory::started().await;
    let mut snapshot = directory.store.snapshot().await.unwrap();
    let bytes = snapshot.get(&Place::Format.key()).await.unwrap();
    assert_eq!(
        kit_store::from_row::<u32>(&bytes.unwrap()),
        Ok(kit_store::FORMAT)
    );
}

#[tokio::test]
async fn a_person_holds_no_address() {
    let directory = Directory::started().await;
    let mut snapshot = directory.store.snapshot().await.unwrap();
    let row = snapshot
        .get(&Place::Person(i("anna")).key())
        .await
        .unwrap()
        .unwrap();
    let text = String::from_utf8(row).unwrap();
    assert!(text.contains("Anna") && !text.contains("aa\""));
    assert!(directory.has(Place::Email(i("aa"))).await);
}

#[tokio::test]
async fn an_address_goes_only_to_someone_who_exists() {
    let directory = Directory::started().await;
    let args = json!({ "person": "nobody", "email_hash": "ee" });
    let reply = directory.edit("system", "person.email", args).await;
    assert!(turned_back(&reply, "no such person"));
}

#[tokio::test]
async fn a_login_binds_only_to_the_address_of_the_person_binding_it() {
    let directory = Directory::started().await;
    let stolen = json!({ "login": "l1", "email_hash": "aa" });
    let reply = directory.edit("dan", "login.bind", stolen).await;
    assert!(turned_back(&reply, "belongs to someone else"));
    let own = json!({ "login": "l2", "email_hash": "dd" });
    assert!(matches!(
        directory.edit("dan", "login.bind", own).await,
        Reply::Ack { .. }
    ));
}

#[tokio::test]
async fn nonsense_arguments_are_turned_back_not_crashed_on() {
    let directory = Directory::started().await;
    for args in [
        json!({}),
        json!({ "id": "a/b", "name": "x", "email_hash": "x" }),
        json!("text"),
        Value::Null,
    ] {
        let reply = directory.edit("system", "person.add", args).await;
        assert!(turned_back(&reply, "isn't valid"), "{reply:?}");
    }
}

#[tokio::test]
async fn only_its_own_commands_are_its_own() {
    assert!(people::handles("person.add"));
    assert!(!people::handles("anything.else"));
    let directory = Directory::started().await;
    let reply = directory.edit("system", "anything.else", json!({})).await;
    assert!(turned_back(&reply, "Unknown command"));
}

#[tokio::test]
async fn the_directory_log_replays() {
    let directory = Directory::started().await;
    let mut snapshot = directory.store.snapshot().await.unwrap();
    let check = verify_log(&mut snapshot, &Scope::Directory).await.unwrap();
    assert_eq!(check.entries, 3);
}
