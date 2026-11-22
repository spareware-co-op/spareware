//! Shared parameters and the shared parameter file: a UUID given by the caller, the same
//! parameter meaning the same thing in two projects, import that changes nothing the second time,
//! and a refusal for the same ID with another kind.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::{
    Choice, Definition, Holder, Kind, ProjectParams, SharedError, SharedFile, SharedParameter,
    is_uuid,
};
use kit_core::Id;

const FIRE: &str = "6f1d2c3a-4b5e-4f60-8a71-92b3c4d5e6f7";
const COST: &str = "0a1b2c3d-4e5f-4a6b-9c7d-8e9fa0b1c2d3";
const SIZE: &str = "ffeeddcc-bbaa-4998-8776-655443322110";

fn def(uuid: &str, name: &str, kind: Kind) -> Definition {
    Definition::new(Id::parse(uuid).unwrap(), name, kind, Holder::Instance).unwrap()
}

fn shared(uuid: &str, name: &str, kind: Kind, group: &str) -> SharedParameter {
    SharedParameter::new(def(uuid, name, kind), group).unwrap()
}

fn file() -> SharedFile {
    SharedFile::new()
        .add(shared(FIRE, "Fire rating", Kind::Text, "Fire"))
        .unwrap()
        .add(shared(COST, "Unit cost", Kind::Number, "Cost"))
        .unwrap()
}

#[test]
fn a_uuid_has_one_spelling() {
    assert!(is_uuid(FIRE));
    for bad in [
        "",
        "not-a-uuid",
        "00000000-0000-0000-0000-000000000000",
        "6F1D2C3A-4B5E-4F60-8A71-92B3C4D5E6F7",
        "6f1d2c3a4b5e4f608a7192b3c4d5e6f7",
        "{6f1d2c3a-4b5e-4f60-8a71-92b3c4d5e6f7}",
        "urn:uuid:6f1d2c3a-4b5e-4f60-8a71-92b3c4d5e6f7",
    ] {
        assert!(!is_uuid(bad), "{bad:?}");
    }
}

#[test]
fn a_shared_parameter_needs_a_uuid_and_a_group() {
    let plain = def("fire-rating", "Fire rating", Kind::Text);
    assert!(matches!(
        SharedParameter::new(plain, "Fire"),
        Err(SharedError::BadUuid(_))
    ));
    for group in ["", " Fire", "Fire "] {
        assert!(
            matches!(
                SharedParameter::new(def(FIRE, "Fire rating", Kind::Text), group),
                Err(SharedError::BadGroup(_))
            ),
            "{group:?}"
        );
    }
    let ok = shared(FIRE, "Fire rating", Kind::Text, "Fire");
    assert_eq!(ok.id().as_str(), FIRE);
    assert_eq!(ok.group(), "Fire");
    assert_eq!(ok.definition().name(), "Fire rating");
}

#[test]
fn importing_twice_changes_nothing() {
    let exported = file();
    let bytes = exported.to_bytes().unwrap();
    let once = SharedFile::new().import(&exported).unwrap();
    let twice = once.import(&exported).unwrap();
    assert_eq!(once, twice);
    assert_eq!(once.to_bytes().unwrap(), bytes);
    assert_eq!(twice.to_bytes().unwrap(), bytes);
    // Adding one already there is also a no-op.
    let again = once
        .add(shared(FIRE, "Fire rating", Kind::Text, "Fire"))
        .unwrap();
    assert_eq!(again, once);
}

#[test]
fn another_project_imports_what_one_exports() {
    let bytes = file().to_bytes().unwrap();
    let theirs = SharedFile::from_bytes(&bytes).unwrap();
    let mine = SharedFile::new()
        .add(shared(SIZE, "Size", Kind::Length, "Geometry"))
        .unwrap()
        .import(&theirs)
        .unwrap();
    assert_eq!(mine.len(), 3);
    let fire = mine.get(&Id::parse(FIRE).unwrap()).unwrap();
    assert_eq!(fire.definition().kind(), Kind::Text);
    assert_eq!(fire.group(), "Fire");
}

#[test]
fn the_same_id_with_a_different_kind_is_refused() {
    let base = file();
    let other = SharedFile::new()
        .add(shared(FIRE, "Fire rating", Kind::Number, "Fire"))
        .unwrap();
    match base.import(&other) {
        Err(SharedError::KindConflict {
            id,
            existing,
            incoming,
            ..
        }) => {
            assert_eq!(id.as_str(), FIRE);
            assert_eq!(existing, Kind::Text);
            assert_eq!(incoming, Kind::Number);
        }
        other => panic!("{other:?}"),
    }
    // The caller keeps the file it had.
    assert_eq!(base, file());
}

#[test]
fn the_same_id_with_another_name_group_holder_or_choices_is_refused() {
    let base = file();
    let field = |p: SharedParameter| match base.add(p) {
        Err(SharedError::Conflict { field, .. }) => field,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        field(shared(FIRE, "Fire grade", Kind::Text, "Fire")),
        "name"
    );
    assert_eq!(
        field(shared(FIRE, "Fire rating", Kind::Text, "Safety")),
        "group"
    );
    let as_type = Definition::new(
        Id::parse(FIRE).unwrap(),
        "Fire rating",
        Kind::Text,
        Holder::Type,
    )
    .unwrap();
    assert_eq!(
        field(SharedParameter::new(as_type, "Fire").unwrap()),
        "holder"
    );
    let colour = |keys: &[&str]| {
        Definition::choice(
            Id::parse(SIZE).unwrap(),
            "Colour",
            Holder::Instance,
            keys.iter().map(|k| Choice::new(k, k)).collect(),
        )
        .unwrap()
    };
    let with_red = base
        .add(SharedParameter::new(colour(&["red"]), "Looks").unwrap())
        .unwrap();
    let changed = SharedParameter::new(colour(&["red", "blue"]), "Looks").unwrap();
    assert!(matches!(
        with_red.add(changed),
        Err(SharedError::Conflict {
            field: "choices",
            ..
        })
    ));
}

#[test]
fn two_parameters_in_a_group_cannot_share_a_name() {
    let err = file()
        .add(shared(SIZE, "FIRE RATING", Kind::Text, "Fire"))
        .unwrap_err();
    assert!(matches!(err, SharedError::DuplicateName { .. }));
    // The same name in another group is allowed.
    assert!(
        file()
            .add(shared(SIZE, "Fire rating", Kind::Text, "Other"))
            .is_ok()
    );
}

#[test]
fn the_file_is_ordered_and_the_same_bytes_every_time() {
    let a = file()
        .add(shared(SIZE, "Size", Kind::Length, "Cost"))
        .unwrap();
    let b = SharedFile::new()
        .add(shared(SIZE, "Size", Kind::Length, "Cost"))
        .unwrap()
        .add(shared(COST, "Unit cost", Kind::Number, "Cost"))
        .unwrap()
        .add(shared(FIRE, "Fire rating", Kind::Text, "Fire"))
        .unwrap();
    assert_eq!(a.to_bytes().unwrap(), b.to_bytes().unwrap());
    let order: Vec<&str> = a
        .parameters()
        .iter()
        .map(|p| p.definition().name())
        .collect();
    assert_eq!(order, ["Size", "Unit cost", "Fire rating"]);
}

#[test]
fn golden_bytes() {
    let one = SharedFile::new()
        .add(shared(FIRE, "Fire rating", Kind::Text, "Fire"))
        .unwrap();
    assert_eq!(
        String::from_utf8(one.to_bytes().unwrap()).unwrap(),
        concat!(
            r#"{"format":1,"parameters":[{"group":"Fire","definition":"#,
            r#"{"id":"6f1d2c3a-4b5e-4f60-8a71-92b3c4d5e6f7","name":"Fire rating","#,
            r#""kind":"text","holder":"instance","choices":[]}}]}"#
        )
    );
}

#[test]
fn reading_round_trips_and_refuses_what_breaks_a_rule() {
    let f = file();
    let bytes = f.to_bytes().unwrap();
    assert_eq!(SharedFile::from_bytes(&bytes).unwrap(), f);

    let text = String::from_utf8(bytes.clone()).unwrap();
    // A later format is not read.
    let later = text.replace(r#""format":1"#, r#""format":2"#);
    assert_eq!(
        SharedFile::from_bytes(later.as_bytes()),
        Err(SharedError::UnsupportedFormat(2))
    );
    // Not a UUID.
    let not_uuid = text.replace(FIRE, "fire-rating");
    assert!(matches!(
        SharedFile::from_bytes(not_uuid.as_bytes()),
        Err(SharedError::BadUuid(_))
    ));
    // Another spelling of the same data is not the one spelling.
    let spaced = text.replace(r#""format":1,"#, r#""format": 1,"#);
    assert!(matches!(
        SharedFile::from_bytes(spaced.as_bytes()),
        Err(SharedError::Malformed(_))
    ));
    // Out of order.
    let mut rows: Vec<&str> = vec![];
    let cost = text.find(r#"{"group":"Cost""#).unwrap();
    let fire = text.find(r#"{"group":"Fire""#).unwrap();
    rows.push(&text[fire..text.len() - 2]);
    rows.push(&text[cost..fire - 1]);
    let swapped = format!(r#"{{"format":1,"parameters":[{}]}}"#, rows.join(","));
    assert!(matches!(
        SharedFile::from_bytes(swapped.as_bytes()),
        Err(SharedError::Malformed(_))
    ));
    // Repeated.
    let one = &text[cost..fire - 1];
    let twice = format!(r#"{{"format":1,"parameters":[{one},{one}]}}"#);
    assert!(matches!(
        SharedFile::from_bytes(twice.as_bytes()),
        Err(SharedError::Malformed(_))
    ));
    // Unknown field, garbage, empty.
    for bad in [
        &br#"{"format":1,"parameters":[],"extra":1}"#[..],
        b"garbage",
        b"",
    ] {
        assert!(matches!(
            SharedFile::from_bytes(bad),
            Err(SharedError::Malformed(_))
        ));
    }
}

#[test]
fn a_project_takes_a_shared_parameter_in_and_the_meaning_follows() {
    let p = shared(FIRE, "Fire rating", Kind::Text, "Fire");
    let a = ProjectParams::new().add_shared_parameter(&p).unwrap();
    let b = ProjectParams::new().add_shared_parameter(&p).unwrap();
    let id = Id::parse(FIRE).unwrap();
    assert_eq!(a.definition(&id), b.definition(&id));
    assert_eq!(a.add_shared_parameter(&p).unwrap(), a);
    // The same UUID as another kind in a project that has it is refused.
    let clash = shared(FIRE, "Fire rating", Kind::Number, "Fire");
    assert!(matches!(
        a.add_shared_parameter(&clash),
        Err(construct_params::ProjectError::KindConflict { .. })
    ));
}
