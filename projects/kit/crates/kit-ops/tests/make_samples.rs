//! Writes the recorded samples once: `cargo test -p kit-ops --test make_samples -- --ignored`.
//! Run this only when adding a new format version's samples, and never over an old one.

use kit_core::Id;
use kit_ops::{Change, Claim, ClaimChange, Edit, Entry};
use serde_json::json;
use std::path::Path;

#[test]
#[ignore = "only to record new samples"]
fn record() {
    let id = |text: &str| Id::parse(text).unwrap();
    let edit = |cmd: &str, args| Edit {
        person: id("anna"),
        client: id("01JB8ZK3QH6Y7M4T9VXCDE2FGH"),
        counter: 12,
        base: 40,
        target: id("doc-1"),
        cmd: cmd.to_owned(),
        args,
        author_ms: 1_793_413_539_019,
    };
    let claim = ClaimChange {
        piece: id("P-104"),
        claim: Some(Claim {
            person: id("anna"),
            name: "Anna".to_owned(),
            until_ms: 1_793_413_569_100,
        }),
    };
    let hash = |digit: char| id(&digit.to_string().repeat(64));
    let moved = Entry::new(
        41,
        &edit(
            "piece.move",
            json!({ "piece": "P-104", "x": 2500, "y": 300 }),
        ),
        1,
        1_793_413_539_100,
        vec![Change {
            piece: id("P-104"),
            before: Some(hash('a')),
            after: Some(hash('b')),
        }],
        vec![claim],
        None,
    );
    let created = Entry::new(
        42,
        &edit(
            "piece.create",
            json!({ "piece": "P-105", "kind": "note", "level": 2, "x": 100, "y": 0,
                    "w": 900, "d": 300, "h": 2100, "props": { "fire rating": "FD30" } }),
        ),
        1,
        1_793_413_539_200,
        vec![Change {
            piece: id("P-105"),
            before: None,
            after: Some(hash('c')),
        }],
        Vec::new(),
        None,
    );
    let turned_back = Entry::new(
        43,
        &edit(
            "piece.move",
            json!({ "piece": "W-101", "x": 1, "y": 1 }),
        ),
        1,
        1_793_413_539_300,
        Vec::new(),
        Vec::new(),
        Some("Anna is working on this.".to_owned()),
    );
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/samples");
    std::fs::create_dir_all(&folder).unwrap();
    for (name, entry) in [
        ("entry-v1-move.json", moved),
        ("entry-v1-create.json", created),
        ("entry-v1-turned-back.json", turned_back),
    ] {
        let path = folder.join(name);
        assert!(
            !path.exists(),
            "{name} is recorded already: samples are never rewritten"
        );
        std::fs::write(path, entry.encode()).unwrap();
    }
}
