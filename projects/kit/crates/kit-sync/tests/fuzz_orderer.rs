//! The orderer under random edits and random faults. Runs as a property test in `cargo test`, and
//! under libFuzzer with `cargo bolero test faults_in_any_order --package kit-sync`.

mod support;

use bolero::check;
use kit_store::memory::{Fault, Memory};
use kit_sync::Reply;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::*;

/// Whatever the order of edits, resends, restarts and failed commits, every edit that was
/// confirmed is in the log once, no number is skipped, and the log replays to what is stored.
#[test]
fn faults_in_any_order() {
    check!().with_type::<Vec<(u8, u8)>>().for_each(|steps| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .start_paused(true)
            .build()
            .unwrap();
        runtime.block_on(async {
            let store = Memory::new();
            let notes = Arc::new(Notes::default());
            let clock = Arc::new(TestClock::default());
            let mut current = server(&store, &notes, &clock);
            let mut next: BTreeMap<&str, u64> = BTreeMap::new();
            let mut applied = 0u64;

            for (what, who) in steps.iter().take(40) {
                let person = if who % 2 == 0 { "anna" } else { "ben" };
                let counter = *next.entry(person).or_insert(1);
                let note = if person == "anna" { "a" } else { "b" };
                match what % 6 {
                    0 => store.inject(Fault::Fails),
                    1 => store.inject(Fault::Conflict),
                    2 => store.inject(Fault::LandsAnswerLost),
                    3 => current = server(&store, &notes, &clock),
                    _ => {}
                }
                let edit = edit(person, "tab", counter, note, &counter.to_string());
                match run_until_answered(&current, &project(), edit.clone()).await {
                    Reply::Ack { .. } => applied += 1,
                    other => panic!("{other:?}"),
                }
                *next.entry(person).or_insert(1) += 1;
                if what % 5 == 4 {
                    let again = run_until_answered(&current, &project(), edit).await;
                    assert!(matches!(again, Reply::Ack { .. }), "a resend is confirmed");
                }
            }
            let check = verify(&store, &project()).await.unwrap();
            assert_eq!(check.entries, applied);
        });
    });
}
