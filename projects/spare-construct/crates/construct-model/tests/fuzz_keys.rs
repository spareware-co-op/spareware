//! Fuzz targets for Spare Construct's tables. Each runs as a property test in `cargo test`, and
//! under libFuzzer with `cargo bolero test <name> --package construct-model` (the `kit` workflow).

use bolero::check;
use construct_model::keys::Place;
use kit_core::Id;
use kit_store::Key;

/// Any text at all parses to nothing or to the place whose key is exactly that text, so a key has
/// one spelling and reading never panics.
#[test]
fn construct_keys_have_one_spelling() {
    check!().for_each(|bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let key = Key::from_stored(text.clone());
        if let Some(place) = Place::parse(&key) {
            assert_eq!(place.key().as_str(), text);
        }
    });
}

/// IDs built from text round-trip through every kind of key that holds them.
#[test]
fn construct_keys_read_back() {
    check!()
        .with_type::<(String, String, i32, i64, i64)>()
        .for_each(|(a, b, level, x, y)| {
            let (Ok(a), Ok(b)) = (Id::parse(a), Id::parse(b)) else {
                return;
            };
            let places = [
                Place::Body {
                    project: a.clone(),
                    id: b.clone(),
                },
                Place::Built {
                    project: a.clone(),
                    document: b.clone(),
                    element: a.clone(),
                },
                Place::Regenerated { project: a.clone() },
                Place::Area {
                    project: a.clone(),
                    document: b.clone(),
                    level: *level,
                    x: *x,
                    y: *y,
                    element: b.clone(),
                },
                Place::Item {
                    project: a.clone(),
                    team: b.clone(),
                    folder: a.clone(),
                    item: b.clone(),
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
            ];
            for place in places {
                assert_eq!(Place::parse(&place.key()).as_ref(), Some(&place));
            }
        });
}
