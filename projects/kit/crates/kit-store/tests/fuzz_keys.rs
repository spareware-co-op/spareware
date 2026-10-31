//! Fuzz targets for the key layout. Each runs as a property test in `cargo test`, and under
//! libFuzzer with `cargo bolero test <name> --package kit-store` (the `kit` workflow).

use bolero::check;
use kit_core::Id;
use kit_store::key::list;
use kit_store::{Key, Place, Scope};

/// Any text at all parses to nothing or to the place whose key is exactly that text, so a key has
/// one spelling and reading never panics.
#[test]
fn key_parsing_has_one_spelling() {
    check!().for_each(|bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let key = Key::from_stored(text.clone());
        if let Some(place) = key.parse() {
            assert_eq!(place.key().as_str(), text);
        }
    });
}

/// IDs built from text round-trip through every kind of key that holds them.
#[test]
fn keys_built_from_ids_read_back() {
    check!()
        .with_type::<(String, String, i32, i64, u64)>()
        .for_each(|(a, b, level, x, seq)| {
            let (Ok(a), Ok(b)) = (Id::parse(a), Id::parse(b)) else {
                return;
            };
            let places = [
                Place::Head(Scope::Directory),
                Place::Head(Scope::Project(a.clone())),
                Place::Log(Scope::Project(a.clone()), *seq),
                Place::Client(Scope::Directory, a.clone(), b.clone()),
                Place::Record {
                    project: a.clone(),
                    hash: b.clone(),
                },
                Place::Element {
                    project: a.clone(),
                    model: b.clone(),
                    element: a.clone(),
                },
                Place::Area {
                    project: a.clone(),
                    model: b.clone(),
                    level: *level,
                    x: *x,
                    y: -*x / 2,
                    element: b.clone(),
                },
                Place::MemberOfTeam {
                    person: a.clone(),
                    project: b.clone(),
                    team: a.clone(),
                },
                Place::Bridge {
                    hub: a.clone(),
                    other: b.clone(),
                },
                Place::Session(a.clone()),
            ];
            for place in places {
                assert_eq!(place.key().parse().as_ref(), Some(&place));
            }
        });
}

/// Log keys sort as their numbers do.
#[test]
fn log_keys_keep_number_order() {
    check!().with_type::<(u64, u64)>().for_each(|(a, b)| {
        let scope = Scope::Directory;
        assert_eq!(scope.log(*a).cmp(&scope.log(*b)), a.cmp(b));
    });
}

/// A project's range holds every key of that project and no key of any other.
#[test]
fn one_projects_range_never_holds_anothers_keys() {
    check!()
        .with_type::<(String, String, u64)>()
        .for_each(|(a, b, seq)| {
            let (Ok(a), Ok(b)) = (Id::parse(a), Id::parse(b)) else {
                return;
            };
            let (start, end) = list::project(&a).bounds();
            let inside = |key: &Key| key.as_bytes() >= &start[..] && key.as_bytes() < &end[..];
            assert!(inside(&Scope::Project(a.clone()).log(*seq)));
            assert_eq!(inside(&Scope::Project(b.clone()).log(*seq)), a == b);
        });
}
