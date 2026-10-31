//! The directory: its log is the only way in, and Cedar answers for every change.

use kit_accounts::{Access, DirectoryCommands, Target};
use kit_core::Id;
use kit_ops::Edit;
use kit_store::memory::Memory;
use kit_store::{Kv, Place, Reader, Scope};
use kit_sync::{Orderers, Reply, SystemClock, verify_log};
use serde_json::{Value, json};
use std::sync::Arc;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

struct Directory {
    store: Memory,
    access: Arc<Access<Memory>>,
    orderers: Orderers<Memory, DirectoryCommands<Memory>>,
    counter: std::sync::atomic::AtomicU64,
}

impl Directory {
    fn new() -> Self {
        let store = Memory::new();
        let access = Arc::new(Access::new(store.clone()).unwrap());
        let commands = Arc::new(DirectoryCommands::new(access.clone()));
        let orderers = Orderers::new(store.clone(), commands, Arc::new(SystemClock), 50);
        Self {
            store,
            access,
            orderers,
            counter: 0.into(),
        }
    }

    async fn edit(&self, person: &str, cmd: &str, args: Value) -> Reply {
        let n = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

    /// Starts a directory with one hub, Anna at home in it, and Dan outside it.
    async fn started() -> Self {
        let directory = Self::new();
        assert!(matches!(
            directory.edit("system", "directory.start", json!({})).await,
            Reply::Ack { .. }
        ));
        for (cmd, args) in [
            ("hub.add", json!({ "id": "one", "name": "One" })),
            ("hub.add", json!({ "id": "two", "name": "Two" })),
            (
                "person.add",
                json!({ "id": "anna", "name": "Anna", "home_hub": "one", "email_hash": "aa" }),
            ),
            (
                "person.add",
                json!({ "id": "dan", "name": "Dan", "home_hub": "two", "email_hash": "dd" }),
            ),
            (
                "project.add",
                json!({ "id": "p", "name": "P", "hub": "one" }),
            ),
            (
                "team.add",
                json!({ "project": "p", "id": "arch", "name": "Arch" }),
            ),
        ] {
            assert!(
                matches!(directory.edit("system", cmd, args).await, Reply::Ack { .. }),
                "{cmd}"
            );
        }
        directory
    }
}

fn turned_back(reply: &Reply, words: &str) -> bool {
    matches!(reply, Reply::Rejected { reason, .. } if reason.contains(words))
}

#[tokio::test]
async fn nobody_can_change_an_empty_directory_but_by_starting_it() {
    let directory = Directory::new();
    let reply = directory
        .edit("mallory", "hub.add", json!({ "id": "x", "name": "X" }))
        .await;
    assert!(turned_back(&reply, "can't change the directory"));
    assert!(
        directory
            .store
            .keys()
            .iter()
            .all(|key| !key.starts_with("dir/hub"))
    );
}

#[tokio::test]
async fn the_directory_can_be_started_only_once() {
    let directory = Directory::started().await;
    let reply = directory
        .edit("mallory", "directory.start", json!({}))
        .await;
    assert!(turned_back(&reply, "already been started"));
    let mallory = Place::Admin(i("mallory")).key();
    assert_eq!(
        directory
            .store
            .snapshot()
            .await
            .unwrap()
            .get(&mallory)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn starting_records_the_store_format() {
    let directory = Directory::started().await;
    let bytes = directory
        .store
        .snapshot()
        .await
        .unwrap()
        .get(&Place::Format.key())
        .await
        .unwrap();
    assert_eq!(
        kit_store::from_row::<u32>(&bytes.unwrap()),
        Ok(kit_store::FORMAT)
    );
}

#[tokio::test]
async fn only_an_administrator_changes_the_directory() {
    let directory = Directory::started().await;
    let reply = directory
        .edit("anna", "hub.add", json!({ "id": "three", "name": "Three" }))
        .await;
    assert!(turned_back(&reply, "can't change the directory"));
    let log_head = directory
        .store
        .snapshot()
        .await
        .unwrap()
        .get(&Scope::Directory.head())
        .await;
    assert!(log_head.unwrap().is_some(), "the refusal is on the record");
}

#[tokio::test]
async fn people_join_a_project_only_from_its_hub() {
    let directory = Directory::started().await;
    let args = json!({ "project": "p", "person": "dan" });
    let reply = directory
        .edit("system", "project.person", args.clone())
        .await;
    assert!(turned_back(&reply, "isn't in the project's hub"));
    directory
        .edit(
            "system",
            "hub.external",
            json!({ "hub": "one", "person": "dan" }),
        )
        .await;
    assert!(matches!(
        directory.edit("system", "project.person", args).await,
        Reply::Ack { .. }
    ));
}

#[tokio::test]
async fn access_follows_the_rows_at_once() {
    let directory = Directory::started().await;
    let anna = i("anna");
    let project = i("p");
    let target = Target::Project(&project);
    assert!(
        !directory
            .access
            .allowed(&anna, "project.open", target, None)
            .await
            .unwrap()
    );
    let args = json!({ "project": "p", "team": "arch", "person": "anna" });
    directory.edit("system", "team.person", args).await;
    assert!(
        directory
            .access
            .allowed(&anna, "project.open", target, None)
            .await
            .unwrap()
    );
    let team = i("arch");
    let wip = i("wip");
    let folder = Target::Folder(&project, &team, &wip);
    assert!(
        directory
            .access
            .allowed(&anna, "folder.open", folder, None)
            .await
            .unwrap()
    );
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
        json!({ "id": "a/b", "name": "x" }),
        json!("text"),
        Value::Null,
    ] {
        let reply = directory.edit("system", "hub.add", args).await;
        assert!(turned_back(&reply, "isn't valid"), "{reply:?}");
    }
}

#[tokio::test]
async fn the_directory_log_replays() {
    let directory = Directory::started().await;
    let mut snapshot = directory.store.snapshot().await.unwrap();
    let check = verify_log(&mut snapshot, &Scope::Directory).await.unwrap();
    assert_eq!(check.entries, 7);
}
