//! What must hold when things go wrong. Each test stages one failure and then checks the log
//! against the model with `verify_log`: nothing lost, nothing doubled, no gap.

mod support;

use kit_store::key::Scope;
use kit_store::memory::{Fault, Memory};
use kit_sync::Reply;
use kit_sync::live::Watcher;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use support::*;
use tokio::time::{sleep, timeout};

fn setup() -> (Memory, Arc<Notes>, Arc<TestClock>) {
    (
        Memory::new(),
        Arc::new(Notes::default()),
        Arc::new(TestClock::default()),
    )
}

#[tokio::test]
async fn edits_land_in_order_with_no_gaps() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    for n in 1..=20 {
        let edit = edit("anna", "tab", n, "a", &n.to_string());
        assert_eq!(
            server.run(&project(), edit).await,
            Ok(Reply::Ack { counter: n, seq: n })
        );
    }
    assert_eq!(verify(&store, &project()).await.unwrap().entries, 20);
    assert_eq!(stored_text(&store, "a").await.as_deref(), Some("20"));
}

#[tokio::test]
async fn a_commit_that_failed_is_tried_again_and_lands_once() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    store.inject(Fault::Fails);
    store.inject(Fault::Conflict);
    let reply = server
        .run(&project(), edit("anna", "tab", 1, "a", "x"))
        .await;
    assert_eq!(reply, Ok(Reply::Ack { counter: 1, seq: 1 }));
    assert_eq!(verify(&store, &project()).await.unwrap().entries, 1);
}

/// The worst case: the commit landed and its answer was lost. The retry must find it done.
#[tokio::test]
async fn a_crashed_commit_that_landed_is_not_applied_twice() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    store.inject(Fault::LandsAnswerLost);
    let reply = server
        .run(&project(), edit("anna", "tab", 1, "a", "x"))
        .await;
    assert_eq!(reply, Ok(Reply::Ack { counter: 1, seq: 1 }));
    assert_eq!(head(&store, &project()).await, 1);
    assert_eq!(verify(&store, &project()).await.unwrap().entries, 1);
}

#[tokio::test]
async fn a_resent_edit_is_confirmed_and_changes_nothing() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    let first = edit("anna", "tab", 1, "a", "x");
    server.run(&project(), first.clone()).await.unwrap();
    let again = server.run(&project(), first).await;
    assert_eq!(again, Ok(Reply::Ack { counter: 1, seq: 1 }));
    assert_eq!(head(&store, &project()).await, 1);
}

/// If edit 6 failed and edit 7 landed, a resend of 6 would look like a repeat and vanish.
#[tokio::test]
async fn an_edit_ahead_of_its_turn_waits_so_the_one_before_isnt_lost() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    let ahead = server
        .run(&project(), edit("anna", "tab", 2, "a", "second"))
        .await;
    assert_eq!(ahead, Ok(Reply::Retry { counter: 2 }));
    assert_eq!(head(&store, &project()).await, 0);
    server
        .run(&project(), edit("anna", "tab", 1, "a", "first"))
        .await
        .unwrap();
    let now = server
        .run(&project(), edit("anna", "tab", 2, "a", "second"))
        .await;
    assert_eq!(now, Ok(Reply::Ack { counter: 2, seq: 2 }));
    assert_eq!(stored_text(&store, "a").await.as_deref(), Some("second"));
}

/// Two servers, one project, every commit fighting for the same head.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_writers_on_one_number_never_share_it() {
    let (store, notes, clock) = setup();
    let first = Arc::new(server(&store, &notes, &clock));
    let second = Arc::new(server(&store, &notes, &clock));
    const EDITS: u64 = 200;
    let write = |server: Arc<Server>, person: &'static str, note: &'static str| async move {
        for n in 1..=EDITS {
            let edit = edit(person, "tab", n, note, &n.to_string());
            let reply = run_until_answered(&server, &project(), edit).await;
            assert!(matches!(reply, Reply::Ack { .. }), "{reply:?}");
        }
    };
    let (a, b) = tokio::join!(
        tokio::spawn(write(first, "anna", "a")),
        tokio::spawn(write(second, "ben", "b")),
    );
    a.unwrap();
    b.unwrap();
    let check = verify(&store, &project()).await.unwrap();
    assert_eq!(
        check.entries,
        EDITS * 2,
        "every edit took one number and no number was shared"
    );
    assert_eq!(stored_text(&store, "a").await, Some(EDITS.to_string()));
    assert_eq!(stored_text(&store, "b").await, Some(EDITS.to_string()));
}

#[tokio::test]
async fn a_claim_turns_others_back_until_it_runs_out() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    server
        .run(&project(), edit("anna", "tab-a", 1, "a", "hers"))
        .await
        .unwrap();

    let refused = server
        .run(&project(), edit("ben", "tab-b", 1, "a", "his"))
        .await;
    let reason = "anna is working on this.".to_owned();
    assert_eq!(refused, Ok(Reply::Rejected { counter: 1, reason }));
    assert_eq!(stored_text(&store, "a").await.as_deref(), Some("hers"));

    clock.0.store(CLAIM_MS - 1, Ordering::SeqCst);
    let still = server
        .run(&project(), edit("ben", "tab-b", 2, "a", "his"))
        .await
        .unwrap();
    assert!(
        matches!(still, Reply::Rejected { .. }),
        "a millisecond before it runs out"
    );

    clock.0.store(CLAIM_MS, Ordering::SeqCst);
    let taken = server
        .run(&project(), edit("ben", "tab-b", 3, "a", "his"))
        .await
        .unwrap();
    assert!(
        matches!(taken, Reply::Ack { .. }),
        "the moment it has run out"
    );
    assert_eq!(stored_text(&store, "a").await.as_deref(), Some("his"));
}

#[tokio::test]
async fn someone_refused_access_is_logged_and_changes_nothing() {
    let (store, notes, clock) = setup();
    notes.refused.lock().unwrap().insert("dan".to_owned());
    let server = server(&store, &notes, &clock);
    let reply = server
        .run(&project(), edit("dan", "tab", 1, "a", "x"))
        .await
        .unwrap();
    assert!(matches!(reply, Reply::Rejected { .. }));
    assert_eq!(stored_text(&store, "a").await, None);
    let log = log(&store, &project()).await;
    assert_eq!(log.len(), 1, "the attempt is on the record");
    assert!(log[0].changes.is_empty() && log[0].failed.is_some());
}

/// A server dies with edits in flight. A new one takes over, the client sends what wasn't
/// confirmed, and the log holds each edit once.
#[tokio::test]
async fn a_restart_mid_edit_loses_nothing() {
    let (store, notes, clock) = setup();
    let before = server(&store, &notes, &clock);
    for n in 1..=3 {
        before
            .run(&project(), edit("anna", "tab", n, "a", &n.to_string()))
            .await
            .unwrap();
    }
    // Edit 3 landed but the server died before anyone heard, then 4 and 5 were still to send.
    drop(before);
    let after = server(&store, &notes, &clock);
    for n in 3..=5 {
        let reply = after
            .run(&project(), edit("anna", "tab", n, "a", &n.to_string()))
            .await;
        assert!(matches!(reply, Ok(Reply::Ack { counter, .. }) if counter == n));
    }
    assert_eq!(verify(&store, &project()).await.unwrap().entries, 5);
    assert_eq!(stored_text(&store, "a").await.as_deref(), Some("5"));
}

/// Writing "failed" for a store outage would lose the edit: it stays with its client.
#[tokio::test(start_paused = true)]
async fn a_store_outage_consumes_nothing() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    server
        .run(&project(), edit("anna", "tab", 1, "a", "x"))
        .await
        .unwrap();
    store.set_down(true);
    let during = server
        .run(&project(), edit("anna", "tab", 2, "a", "y"))
        .await;
    assert_eq!(during, Ok(Reply::Retry { counter: 2 }));
    store.set_down(false);
    assert_eq!(head(&store, &project()).await, 1);
    let after = server
        .run(&project(), edit("anna", "tab", 2, "a", "y"))
        .await;
    assert_eq!(after, Ok(Reply::Ack { counter: 2, seq: 2 }));
}

#[tokio::test]
async fn an_entry_for_another_log_lands_with_the_edit_or_not_at_all() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    let other = Scope::Project(i("other"));
    let mut copying = edit("anna", "tab", 1, "a", "x");
    copying.args["copy_to"] = "other".into();

    store.inject(Fault::Fails);
    server.run(&project(), copying).await.unwrap();
    assert_eq!(head(&store, &project()).await, 1);
    assert_eq!(head(&store, &other).await, 1);
    assert_eq!(log(&store, &other).await[0].cmd, "note.arrived");
    assert_eq!(verify(&store, &other).await.unwrap().entries, 1);
}

#[tokio::test]
async fn edits_ordered_by_another_server_reach_this_ones_watchers() {
    let (store, notes, clock) = setup();
    let here = server(&store, &notes, &clock);
    let there = server(&store, &notes, &clock);
    let mut feed = here.get(&project()).await.subscribe();
    there
        .run(&project(), edit("ben", "tab", 1, "a", "x"))
        .await
        .unwrap();
    let entry = timeout(Duration::from_secs(5), feed.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entry.author.as_str(), "ben");
}

#[tokio::test]
async fn a_watcher_sees_only_its_models_and_each_entry_once() {
    let (store, notes, clock) = setup();
    notes.refused.lock().unwrap().insert("dan".to_owned());
    let server = server(&store, &notes, &clock);
    server
        .run(&project(), edit("anna", "tab-a", 1, "a", "x"))
        .await
        .unwrap();
    server
        .run(&project(), edit("dan", "tab-d", 1, "a", "x"))
        .await
        .unwrap();
    let log = log(&store, &project()).await;

    let mut outsider = Watcher::new();
    assert!(outsider.next(&log[0]).is_none(), "not watching this model");

    let mut watcher = Watcher::new();
    watcher.set_client(i("tab-b"));
    watcher.watch(i("model"));
    assert!(watcher.next(&log[0]).is_some());
    assert!(watcher.next(&log[0]).is_none(), "already passed on");
    assert!(
        watcher.next(&log[1]).is_none(),
        "someone else's turned-back edit is theirs alone"
    );

    let mut author = Watcher::new();
    author.set_client(i("tab-d"));
    author.watch(i("model"));
    assert!(
        author.next(&log[1]).is_some(),
        "the tab that sent it hears why"
    );
}

#[tokio::test]
async fn verify_finds_a_missing_entry() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    for n in 1..=3 {
        server
            .run(&project(), edit("anna", "tab", n, "a", "x"))
            .await
            .unwrap();
    }
    store.tamper(&project().log(2), None);
    assert!(
        verify(&store, &project())
            .await
            .unwrap_err()
            .contains("head says 3")
    );
}

#[tokio::test]
async fn verify_finds_a_changed_entry() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    server
        .run(&project(), edit("anna", "tab", 1, "a", "x"))
        .await
        .unwrap();
    let mut entry = log(&store, &project()).await.remove(0);
    entry.changes.clear();
    store.tamper(&project().log(1), Some(entry.encode()));
    assert!(
        verify(&store, &project())
            .await
            .unwrap_err()
            .contains("content ID")
    );
}

#[tokio::test]
async fn verify_finds_a_damaged_record() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    server
        .run(&project(), edit("anna", "tab", 1, "a", "x"))
        .await
        .unwrap();
    let hash = log(&store, &project()).await[0].changes[0]
        .after
        .clone()
        .unwrap();
    let place = kit_store::Place::Record {
        project: i("project"),
        hash,
    };
    store.tamper(&place.key(), Some(b"something else".to_vec()));
    assert!(
        verify(&store, &project())
            .await
            .unwrap_err()
            .contains("doesn't match its hash")
    );
}

#[tokio::test]
async fn verify_finds_a_piece_the_log_doesnt_explain() {
    let (store, notes, clock) = setup();
    let server = server(&store, &notes, &clock);
    server
        .run(&project(), edit("anna", "tab", 1, "a", "x"))
        .await
        .unwrap();
    let extra = kit_store::Place::Element {
        project: i("project"),
        model: i("model"),
        element: i("b"),
    };
    store.tamper(&extra.key(), Some(kit_store::to_row(&"cc").unwrap()));
    assert!(
        verify(&store, &project())
            .await
            .unwrap_err()
            .contains("differ")
    );
    sleep(Duration::from_millis(1)).await;
}
