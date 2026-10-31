//! Fuzz targets for the log entry. Each runs as a property test in `cargo test`, and under
//! libFuzzer with `cargo bolero test <name> --package kit-ops` (the `kit` workflow).

use bolero::check;
use kit_core::Id;
use kit_ops::{Change, Edit, Entry, content_id};
use serde_json::json;

/// Any bytes at all read as an entry or fail politely, and an entry that reads writes back to
/// something that reads the same.
#[test]
fn reading_any_bytes_is_safe() {
    check!().for_each(|bytes: &[u8]| {
        if let Ok(entry) = Entry::decode(bytes) {
            assert_eq!(Entry::decode(&entry.encode()).as_ref(), Ok(&entry));
        }
    });
}

/// Whatever goes into an entry comes back out, and its content ID stays right.
#[test]
fn entries_round_trip() {
    check!()
        .with_type::<(u64, u64, String, String, Vec<(u8, bool)>, bool)>()
        .for_each(|(seq, counter, cmd, note, pieces, turned_back)| {
            let id = |text: &str| Id::parse(text).unwrap();
            let edit = Edit {
                person: id("anna"),
                client: id("tab"),
                counter: *counter,
                base: seq / 2,
                target: id("document"),
                cmd: cmd.clone(),
                args: json!({ "note": note, "n": seq }),
                author_ms: 1,
            };
            let changes: Vec<Change> = pieces
                .iter()
                .map(|(n, gone)| Change {
                    piece: id(&format!("p{n}")),
                    before: Some(id("before")),
                    after: (!gone).then(|| id("after")),
                })
                .collect();
            let failed = turned_back.then(|| note.clone());
            let entry = Entry::new(*seq, &edit, 1, 2, changes, Vec::new(), failed);
            let back = Entry::decode(&entry.encode()).unwrap();
            assert_eq!(back, entry);
            assert_eq!(back.content_id, content_id(&back.changes));
        });
}
