//! A built row's level: written as the base level's ID, and read from a row written when levels
//! were integers, which must keep reading (FORMAT.md: we never stop reading).

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_model::rows::BuiltElement;
use kit_core::Id;

fn row(level: &str) -> String {
    format!(
        concat!(
            r#"{{"input":"abc","edition":{{"engine":"construct.stand-in","version":1,"name_scheme":0}},"#,
            r#""level":{},"cells":[[0,0]],"result":{{"failed":{{"code":"X","message":"y"}}}}}}"#
        ),
        level
    )
}

fn level_of(text: &str) -> Option<Id> {
    serde_json::from_str::<BuiltElement>(&row(text))
        .unwrap()
        .level
}

#[test]
fn a_row_of_the_integer_days_reads_its_level_as_the_integers_text() {
    assert_eq!(level_of("2"), Some(Id::parse("2").unwrap()));
    assert_eq!(level_of("0"), Some(Id::parse("0").unwrap()));
    // A negative integer is no ID: its cells name no level anyone can ask for.
    assert_eq!(level_of("-1"), None);
    assert_eq!(level_of("\"L-1\""), Some(Id::parse("L-1").unwrap()));
    assert_eq!(level_of("null"), None);
    assert!(serde_json::from_str::<BuiltElement>(&row("\"-bad\"")).is_err());
}

#[test]
fn a_row_is_written_with_its_levels_id_and_its_elevation() {
    let mut built: BuiltElement = serde_json::from_str(&row("2")).unwrap();
    assert_eq!(built.elevation, None);
    assert_eq!(serde_json::to_string(&built).unwrap(), row("\"2\""));
    built.level = Some(Id::parse("L-1").unwrap());
    built.elevation = Some("length:q0:19200000".to_owned());
    let text = serde_json::to_string(&built).unwrap();
    assert!(
        text.contains(r#""level":"L-1","elevation":"length:q0:19200000","cells""#),
        "{text}"
    );
    built.level = None;
    built.elevation = None;
    assert_eq!(serde_json::to_string(&built).unwrap(), row("null"));
}
