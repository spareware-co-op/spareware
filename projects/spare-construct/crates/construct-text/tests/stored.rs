//! The stored form: styles and notes written as element records, read back, in one spelling; a
//! record that is not as written refused, never repaired; old documents (none of these records)
//! reading as having none.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::items_after_statements,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_model::element::Element;
use construct_params::{Angle, Length};
use construct_text::{
    Align, Arrow, Leader, ListKind, NOTE_KIND, STYLE_KIND, TextError, TextNote, TextStyle, Texts,
};
use kit_core::Id;
use serde_json::json;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn style() -> TextStyle {
    TextStyle::new(
        i("ST-1"),
        "General",
        "Test Sans",
        Length::from_steps(16_000),
    )
    .unwrap()
    .with_bold(true)
    .with_underline(true)
    .with_colour([200, 16, 0])
    .with_spacing(120)
    .unwrap()
}

fn note() -> TextNote {
    let leader = Leader::new(
        vec![(mm(10), mm(20)), (Length::from_steps(1), mm(-5))],
        Arrow::Filled,
    )
    .unwrap();
    TextNote::new(
        i("N-1"),
        i("V-1"),
        i("ST-1"),
        (Length::from_steps(1_904_000), mm(-250)),
        "Line one\nLine two",
    )
    .unwrap()
    .with_width(Some(mm(80)))
    .unwrap()
    .with_align(Align::Centre)
    .with_list(ListKind::Bullet)
    .with_rotation(Angle::from_units(45 * Angle::UNITS_PER_DEGREE))
    .unwrap()
    .with_leader(leader)
    .unwrap()
    .0
}

#[test]
fn a_style_is_stored_in_one_spelling_and_read_back() {
    let record = style().to_record(&i("DOC"));
    assert_eq!(record.kind, STYLE_KIND);
    assert_eq!(record.name, "General");
    assert_eq!(
        serde_json::to_value(&record).unwrap(),
        json!({
            "id": "ST-1", "document": "DOC", "kind": "text-style", "name": "General",
            "level": 0, "placement": { "x": 0, "y": 0, "z": 0 },
            "recipe": { "owner": "construct", "schema": 1, "op": "text-style", "op_version": 1,
                        "args": {} },
            "props": {
                "@category": "5200",
                "@text.style.bold": "yesno:yes",
                "@text.style.colour": "#c81000",
                "@text.style.font": "Test Sans",
                "@text.style.italic": "yesno:no",
                "@text.style.size": "length:q0:16000",
                "@text.style.spacing": "120",
                "@text.style.underline": "yesno:yes"
            },
            "references": []
        })
    );
    assert_eq!(TextStyle::from_record(&record).unwrap(), style());
}

#[test]
fn a_note_is_stored_in_one_spelling_and_read_back() {
    let record = note().to_record(&i("DOC"));
    assert_eq!(record.kind, NOTE_KIND);
    assert_eq!(
        serde_json::to_value(&record.props).unwrap(),
        json!({
            "@category": "5200",
            "@text.align": "choice:centre",
            "@text.leader.1": "filled;length:q0:64000,length:q0:128000;length:q0:1,length:q0:-32000",
            "@text.lines": "Line one\nLine two",
            "@text.list": "choice:bullet",
            "@text.rotation": "angle:405000000",
            "@text.style": "ST-1",
            "@text.view": "V-1",
            "@text.width": "length:q0:512000",
            "@text.x": "length:q0:1904000",
            "@text.y": "length:q0:-1600000"
        })
    );
    assert_eq!(TextNote::from_record(&record).unwrap(), note());
    // A note that does not wrap has no width key at all.
    let plain = TextNote::new(i("N-2"), i("V-1"), i("ST-1"), (mm(0), mm(0)), "x").unwrap();
    assert!(!plain.to_record(&i("DOC")).props.contains_key("@text.width"));
}

#[test]
fn the_same_data_is_the_same_bytes_in_any_order() {
    let a = style().to_record(&i("DOC"));
    let b = note().to_record(&i("DOC"));
    let forward = Texts::from_records([&a, &b]).unwrap();
    let backward = Texts::from_records([&b, &a]).unwrap();
    assert_eq!(forward, backward);
    assert_eq!(
        serde_json::to_string(&forward.to_records(&i("DOC"))).unwrap(),
        serde_json::to_string(&backward.to_records(&i("DOC"))).unwrap()
    );
    assert_eq!(forward.to_records(&i("DOC")), vec![a, b]);
}

#[test]
fn an_old_document_with_no_text_records_reads_as_having_none() {
    let wall = Element {
        kind: "wall.straight".to_owned(),
        ..style().to_record(&i("DOC"))
    };
    let texts = Texts::from_records([&wall]).unwrap();
    assert!(texts.styles().is_empty() && texts.notes().is_empty());
    assert_eq!(Texts::from_records([]).unwrap(), Texts::new());
}

fn refused(record: &Element) -> bool {
    let read = if record.kind == STYLE_KIND {
        TextStyle::from_record(record).map(|_| ())
    } else {
        TextNote::from_record(record).map(|_| ())
    };
    matches!(read, Err(TextError::Malformed(_)))
}

#[test]
fn a_record_that_is_not_as_written_is_refused_never_repaired() {
    let good_note = note().to_record(&i("DOC"));
    let good_style = style().to_record(&i("DOC"));
    let tweak = |record: &Element, key: &str, value: &str| {
        let mut changed = record.clone();
        changed.props.insert(key.to_owned(), value.to_owned());
        changed
    };
    for (key, value) in [
        ("@text.x", "length:q0:+5"),
        ("@text.x", "10"),
        ("@text.align", "choice:middle"),
        ("@text.list", "choice:roman"),
        ("@text.rotation", "angle:99999999999999"),
        ("@text.width", "length:q0:0"),
        ("@text.view", "not an id"),
        ("@text.lines", "   "),
        ("@text.leader.1", "filled;length:q0:1,length:q0:1"),
        (
            "@text.leader.1",
            "arrow;length:q0:1,length:q0:1;length:q0:2,length:q0:2",
        ),
        (
            "@text.leader.01",
            "none;length:q0:1,length:q0:1;length:q0:2,length:q0:2",
        ),
        ("@text.unknown", "x"),
    ] {
        assert!(refused(&tweak(&good_note, key, value)), "{key} {value}");
    }
    for (key, value) in [
        ("@text.style.size", "length:q0:0"),
        ("@text.style.colour", "#FF0000"),
        ("@text.style.colour", "red"),
        ("@text.style.spacing", "0120"),
        ("@text.style.bold", "true"),
        ("@text.style.extra", "x"),
    ] {
        assert!(refused(&tweak(&good_style, key, value)), "{key} {value}");
    }
    let mut named = good_style.clone();
    named.name = "  ".to_owned();
    assert!(refused(&named));
    let mut placed = good_note.clone();
    placed.level = 1;
    assert!(refused(&placed));
    let mut wrong_kind = good_note;
    wrong_kind.kind = "text-style".to_owned();
    assert!(refused(&wrong_kind));
}
