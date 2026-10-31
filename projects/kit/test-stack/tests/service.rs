//! Every rule a person meets, in-process on the test store, so these run without TiKV. The seed
//! data goes in first through the real doors, then each test tries the doors it shouldn't be able
//! to open. `tests/checklist.rs` runs the same story on the real stack.

use kit_accounts::{Access, Accounts, DirectoryCommands, SignInSettings};
use kit_core::Id;
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{FileStore, Kv, Scope};
use kit_sync::{Clock, Orderers, SystemClock, verify_log};
use serde_json::json;
use std::sync::Arc;
use test_stack::backup;
use test_stack::commands::ProjectCommands;
use test_stack::seed::seed;
use test_stack::service::{Refusal, Service, View};
use test_stack::spaces::{LinkKind, folders};

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

struct Stack {
    store: Memory,
    files: MemoryFiles,
    service: Service<Memory, MemoryFiles>,
}

fn build(store: &Memory, files: &MemoryFiles) -> Service<Memory, MemoryFiles> {
    let access = Arc::new(Access::new(store.clone()).unwrap());
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let commands = Arc::new(DirectoryCommands::new(access.clone()));
    let directory = Orderers::new(store.clone(), commands, clock.clone(), 50);
    let settings = SignInSettings {
        issuer: "http://localhost:9".to_owned(),
        client_id: String::new(),
        client_secret: String::new(),
        public_url: "http://localhost:8".to_owned(),
    };
    let accounts = Accounts::new(store.clone(), clock.clone(), settings, directory);
    let commands = Arc::new(ProjectCommands::new(store.clone(), access.clone(), 30_000));
    let orderers = Orderers::new(store.clone(), commands, clock.clone(), 50);
    Service::new(
        store.clone(),
        access,
        accounts,
        orderers,
        clock,
        files.clone(),
    )
}

async fn seeded() -> Stack {
    let store = Memory::new();
    let files = MemoryFiles::default();
    let service = build(&store, &files);
    seed(&store, &service).await.unwrap();
    Stack {
        store,
        files,
        service,
    }
}

fn refused<T: std::fmt::Debug>(answer: Result<T, Refusal>) -> bool {
    matches!(answer, Err(Refusal::Refused(_)))
}

const VIEW: View = View {
    level: 1,
    x0: 0,
    y0: 0,
    x1: 9000,
    y1: 9000,
};

#[tokio::test(flavor = "multi_thread")]
async fn every_log_replays_after_the_seed() {
    let stack = seeded().await;
    let mut snapshot = stack.store.snapshot().await.unwrap();
    assert!(
        verify_log(&mut snapshot, &Scope::Directory)
            .await
            .unwrap()
            .entries
            > 20
    );
    let check = verify_log(&mut snapshot, &Scope::Project(i("example-1")))
        .await
        .unwrap();
    assert_eq!(check.elements, 31);
}

#[tokio::test(flavor = "multi_thread")]
async fn seeding_twice_changes_nothing() {
    let stack = seeded().await;
    let before = stack.store.keys().len();
    seed(&stack.store, &stack.service).await.unwrap();
    assert_eq!(stack.store.keys().len(), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_teams_wip_is_shut_to_everyone_else() {
    let stack = seeded().await;
    let s = &stack.service;
    let (dan, project, model) = (i("dan"), i("example-1"), i("arch-model"));
    let hash = {
        let anna = i("anna");
        let area = s.area(&anna, &project, &model, &VIEW).await.unwrap();
        i(area.pieces.values().next().unwrap())
    };
    assert!(refused(s.area(&dan, &project, &model, &VIEW).await));
    assert!(refused(s.claims(&dan, &project, &model).await));
    assert!(refused(s.record(&dan, &project, &model, &hash).await));
    assert!(refused(s.shape(&dan, &project, &model, &hash).await));
    assert!(refused(s.links(&dan, &project, &model).await));
    assert!(!s.may_watch(&dan, &project, &model).await.unwrap());
    let edit = json!({ "element": "W-101", "x": 1, "y": 1 });
    assert!(refused(
        s.edit(&dan, &project, &model, "element.move", edit).await
    ));
    let listing = s.project(&dan, &project).await.unwrap();
    let arch = &listing["teams"][0];
    let kinds: Vec<_> = arch["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].clone())
        .collect();
    assert_eq!(kinds, [json!("shared"), json!("consumed")]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_record_cant_be_reached_through_another_model() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, project) = (i("anna"), i("example-1"));
    let arch = s
        .area(&anna, &project, &i("arch-model"), &VIEW)
        .await
        .unwrap();
    let hash = i(arch.pieces.values().next().unwrap());
    let elsewhere = s
        .record(&anna, &project, &i("interiors-model"), &hash)
        .await;
    assert!(matches!(elsewhere, Err(Refusal::NotFound)));
}

#[tokio::test(flavor = "multi_thread")]
async fn people_off_a_project_cant_open_it_or_its_hub() {
    let stack = seeded().await;
    let s = &stack.service;
    let (cara, anna, project) = (i("cara"), i("anna"), i("example-1"));
    assert!(refused(s.project(&cara, &project).await));
    assert!(refused(s.bridge_targets(&cara, &project).await));
    assert!(refused(s.may_open_project(&cara, &project).await));
    assert!(refused(s.hub(&anna, &i("company-2")).await));
    assert!(s.hub(&i("dan"), &i("company-1")).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn nobody_outside_a_team_can_publish_or_take_for_it() {
    let stack = seeded().await;
    let s = &stack.service;
    let (dan, ben, project, arch) = (i("dan"), i("ben"), i("example-1"), i("arch"));
    assert!(refused(
        s.publish(&dan, &project, &arch, &i("arch-model")).await
    ));
    assert!(refused(
        s.publish(&ben, &project, &i("civil"), &i("arch-model"))
            .await
    ));
    assert!(refused(
        s.consume(&dan, &project, &arch, &i("nothing")).await
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bridge_needs_both_hubs_and_the_owning_team() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, arch) = (i("anna"), i("dan"), i("example-1"), i("arch"));
    let item = s
        .publish(&anna, &project, &arch, &i("arch-model"))
        .await
        .unwrap();
    assert_eq!(item.name, "Arch package v12");

    let one_sided = s
        .bridge(
            &anna,
            &project,
            &arch,
            &item.id,
            &i("riverside-bridged"),
            &i("landscape"),
        )
        .await;
    assert!(refused(one_sided), "Company 3 hasn't permitted Company 1");
    let not_theirs = s
        .bridge(
            &dan,
            &project,
            &arch,
            &item.id,
            &i("example-1-bridged"),
            &i("structure"),
        )
        .await;
    assert!(refused(not_theirs));
    s.bridge(
        &anna,
        &project,
        &arch,
        &item.id,
        &i("example-1-bridged"),
        &i("structure"),
    )
    .await
    .unwrap();

    // The arrival is in the receiving project's own log, written in the same transaction.
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let bridged = Scope::Project(i("example-1-bridged"));
    let head = verify_log(&mut snapshot, &bridged).await.unwrap().entries;
    let log = kit_sync::read_log(&mut snapshot, &bridged, head - 1, head)
        .await
        .unwrap();
    assert_eq!(log[0].cmd, "package.arrived");

    let incoming = i("company-1.arch-package-v12");
    let taken = s
        .consume(&dan, &i("example-1-bridged"), &i("structure"), &incoming)
        .await
        .unwrap();
    assert_eq!(taken.from.as_ref().unwrap().hub_name, "Company 1");
    assert_eq!(taken.sha256, item.sha256);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_package_whose_file_changed_is_refused() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project, arch) = (i("anna"), i("dan"), i("example-1"), i("arch"));
    let item = s
        .publish(&anna, &project, &arch, &i("arch-model"))
        .await
        .unwrap();
    s.bridge(
        &anna,
        &project,
        &arch,
        &item.id,
        &i("example-1-bridged"),
        &i("structure"),
    )
    .await
    .unwrap();
    stack
        .files
        .put(&item.file, b"something else".to_vec())
        .await
        .unwrap();
    let incoming = i("company-1.arch-package-v12");
    let taken = s
        .consume(&dan, &i("example-1-bridged"), &i("structure"), &incoming)
        .await;
    assert!(matches!(taken, Err(Refusal::Bad(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn links_follow_their_rules() {
    let stack = seeded().await;
    let s = &stack.service;
    let (anna, dan, project) = (i("anna"), i("dan"), i("example-1"));
    let site = i("site-setout");
    let across = s
        .add_link(
            &anna,
            &project,
            &site,
            LinkKind::Live,
            None,
            &i("arch-model"),
        )
        .await;
    assert!(refused(across), "she isn't on the Civil team");
    let across = s
        .add_link(
            &i("ben"),
            &project,
            &site,
            LinkKind::Live,
            None,
            &i("arch-model"),
        )
        .await;
    assert!(refused(across), "a live link stays inside one team's WIP");
    let civil = i("civil");
    let outside = s
        .add_link(
            &dan,
            &project,
            &i("arch-model"),
            LinkKind::Shared,
            Some(&civil),
            &i("site-setout-v3"),
        )
        .await;
    assert!(refused(outside));
    let kinds: Vec<_> = s
        .links(&anna, &project, &i("arch-model"))
        .await
        .unwrap()
        .iter()
        .map(|l| l.kind)
        .collect();
    assert_eq!(kinds.len(), 3);
    assert!(
        kinds.contains(&LinkKind::Live)
            && kinds.contains(&LinkKind::Shared)
            && kinds.contains(&LinkKind::Consumed)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_package_is_read_from_its_own_folder() {
    let stack = seeded().await;
    let s = &stack.service;
    let (dan, project) = (i("dan"), i("example-1"));
    let shared = folders::shared();
    let file = s
        .package_file(&dan, &project, &i("arch"), &shared, &i("arch-package-v11"))
        .await;
    assert!(
        !file.unwrap().is_empty(),
        "Shared opens for the whole project"
    );
    let wip = folders::wip();
    let closed = s
        .package_file(&dan, &project, &i("arch"), &wip, &i("arch-package-v11"))
        .await;
    assert!(refused(closed));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_backup_restores_every_key_and_file_into_an_empty_store() {
    let stack = seeded().await;
    let target = MemoryFiles::default();
    let before = backup::digest(&stack.store, &stack.files).await.unwrap();
    backup::backup(&stack.store, &stack.files, &target, "b")
        .await
        .unwrap();

    let (fresh, fresh_files) = (Memory::new(), MemoryFiles::default());
    backup::restore(&fresh, &fresh_files, &target, "b")
        .await
        .unwrap();
    assert_eq!(backup::digest(&fresh, &fresh_files).await.unwrap(), before);
    assert!(
        backup::restore(&fresh, &fresh_files, &target, "b")
            .await
            .is_err(),
        "never over live work"
    );

    // The restored store is a working one.
    let service = build(&fresh, &fresh_files);
    let edit = json!({ "element": "W-101", "x": 5, "y": 5 });
    service
        .edit(
            &i("anna"),
            &i("example-1"),
            &i("arch-model"),
            "element.move",
            edit,
        )
        .await
        .unwrap();
}
