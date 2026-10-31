//! Recorded samples of every entry format ever written. Each must keep reading, and the entries
//! of the newest format must keep writing to the same bytes, because content IDs hash bytes.
//! A failure here means the format changed. Never edit a sample to pass: add a new format version
//! and a new sample, and teach `Entry::decode` to read both (EXTENDING.md, section 3).

use kit_ops::{ENTRY_FORMAT, Entry, content_id};

const V1_MOVE: &[u8] = include_bytes!("samples/entry-v1-move.json");
const V1_CREATE: &[u8] = include_bytes!("samples/entry-v1-create.json");
const V1_TURNED_BACK: &[u8] = include_bytes!("samples/entry-v1-turned-back.json");

/// A stored sample, without the line ending a text editor might add.
fn sample(bytes: &[u8]) -> &[u8] {
    bytes.trim_ascii_end()
}

#[test]
fn version_1_samples_still_read_and_write_the_same_bytes() {
    assert_eq!(ENTRY_FORMAT, 1, "a new format needs its own samples, and this test extended");
    for bytes in [V1_MOVE, V1_CREATE, V1_TURNED_BACK] {
        let entry = Entry::decode(sample(bytes)).unwrap();
        assert_eq!(entry.encode(), sample(bytes));
        assert_eq!(entry.content_id, content_id(&entry.changes));
    }
}

#[test]
fn the_samples_say_what_they_recorded() {
    let moved = Entry::decode(sample(V1_MOVE)).unwrap();
    assert_eq!(moved.cmd, "element.move");
    assert_eq!(moved.args["x"], 2500);
    let turned_back = Entry::decode(sample(V1_TURNED_BACK)).unwrap();
    assert_eq!(turned_back.failed.as_deref(), Some("Anna is working on this."));
    assert!(turned_back.changes.is_empty());
}
