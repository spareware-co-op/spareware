//! Styles, notes and leaders and the rules that bind them: every refusal by name, a style change
//! reaching its notes, a style in use kept.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::items_after_statements,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::{Angle, Length};
use construct_text::{Arrow, Leader, ListKind, TURN, TextError, TextNote, TextStyle, Texts};
use kit_core::Id;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn style(id: &str, name: &str) -> TextStyle {
    TextStyle::new(i(id), name, "Test Sans", mm(3)).unwrap()
}

fn note(id: &str, style: &str) -> TextNote {
    TextNote::new(i(id), i("V-1"), i(style), (mm(10), mm(20)), "Hello").unwrap()
}

fn known(view: &Id) -> bool {
    view == &i("V-1")
}

fn with_style() -> Texts {
    let mut texts = Texts::new();
    texts.add_style(style("ST-1", "General")).unwrap();
    texts
}

#[test]
fn a_style_is_refused_by_name() {
    assert_eq!(
        TextStyle::new(i("S"), "   ", "Test Sans", mm(3)),
        Err(TextError::EmptyName)
    );
    assert!(matches!(
        TextStyle::new(i("S"), "bad\nname", "Test Sans", mm(3)),
        Err(TextError::BadName(_))
    ));
    assert!(matches!(
        TextStyle::new(i("S"), "Ok", "", mm(3)),
        Err(TextError::BadFont(_))
    ));
    for size in [mm(0), mm(-3), mm(501)] {
        assert!(
            matches!(
                TextStyle::new(i("S"), "Ok", "Test Sans", size),
                Err(TextError::BadSize(_))
            ),
            "{size}"
        );
    }
    assert!(matches!(
        style("S", "Ok").with_spacing(10),
        Err(TextError::BadSize(_))
    ));
    // The name is trimmed, and the one name cannot be taken twice, case aside.
    let mut texts = with_style();
    assert_eq!(texts.style(&i("ST-1")).unwrap().name(), "General");
    assert_eq!(
        texts.add_style(style("ST-2", " GENERAL ")),
        Err(TextError::DuplicateName("GENERAL".to_owned()))
    );
    assert_eq!(
        texts.add_style(style("ST-1", "Other")),
        Err(TextError::DuplicateStyle(i("ST-1")))
    );
    // A style may change the case of its own name.
    texts.replace_style(style("ST-1", "general")).unwrap();
    texts.add_style(style("ST-2", "Heading")).unwrap();
    assert_eq!(
        texts.replace_style(style("ST-2", "General")),
        Err(TextError::DuplicateName("General".to_owned()))
    );
    assert_eq!(
        texts.replace_style(style("ST-9", "Nine")),
        Err(TextError::UnknownStyle(i("ST-9")))
    );
}

#[test]
fn a_note_is_refused_by_name() {
    let mut texts = with_style();
    assert!(matches!(
        TextNote::new(i("N"), i("V-1"), i("ST-1"), (mm(0), mm(0)), "  \n "),
        Err(TextError::BadText(_))
    ));
    assert!(matches!(
        TextNote::new(i("N"), i("V-1"), i("ST-1"), (mm(0), mm(0)), "bell\u{7}"),
        Err(TextError::BadText(_))
    ));
    assert!(matches!(
        note("N-1", "ST-1").with_width(Some(mm(0))),
        Err(TextError::BadSize(_))
    ));
    assert!(matches!(
        note("N-1", "ST-1").with_rotation(Angle::from_units(TURN + 1)),
        Err(TextError::BadRotation(_))
    ));
    // In a view that is not there; reading a style that is not there.
    let elsewhere = TextNote::new(i("N-2"), i("V-9"), i("ST-1"), (mm(0), mm(0)), "x").unwrap();
    assert_eq!(
        texts.add_note(elsewhere, known),
        Err(TextError::UnknownView(i("V-9")))
    );
    assert_eq!(
        texts.add_note(note("N-3", "ST-9"), known),
        Err(TextError::UnknownStyle(i("ST-9")))
    );
    texts.add_note(note("N-1", "ST-1"), known).unwrap();
    assert_eq!(
        texts.add_note(note("N-1", "ST-1"), known),
        Err(TextError::DuplicateNote(i("N-1")))
    );
    assert_eq!(
        texts.replace_note(note("N-1", "ST-9")),
        Err(TextError::UnknownStyle(i("ST-9")))
    );
    assert_eq!(
        texts.replace_note(note("N-8", "ST-1")),
        Err(TextError::UnknownNote(i("N-8")))
    );
    let mut moved = note("N-1", "ST-1").to_record(&i("D"));
    moved
        .props
        .insert("@text.view".to_owned(), "V-2".to_owned());
    let moved = TextNote::from_record(&moved).unwrap();
    assert!(matches!(
        texts.replace_note(moved),
        Err(TextError::Malformed(_))
    ));
    assert_eq!(
        texts.remove_note(&i("N-8")),
        Err(TextError::UnknownNote(i("N-8")))
    );
}

#[test]
fn a_leader_needs_two_points_and_is_found_by_its_number() {
    let at = |x: i32, y: i32| (mm(x), mm(y));
    assert!(matches!(
        Leader::new(vec![at(0, 0)], Arrow::Filled),
        Err(TextError::BadLeader(_))
    ));
    assert!(matches!(
        Leader::new(vec![], Arrow::None),
        Err(TextError::BadLeader(_))
    ));
    let too_many: Vec<_> = (0..33).map(|n| at(n, n)).collect();
    assert!(matches!(
        Leader::new(too_many, Arrow::Dot),
        Err(TextError::BadLeader(_))
    ));
    let first = Leader::new(vec![at(0, 0), at(5, 5)], Arrow::Filled).unwrap();
    let second = Leader::new(vec![at(1, 1), at(9, 9), at(9, 20)], Arrow::Open).unwrap();
    let (with_one, number) = note("N-1", "ST-1").with_leader(first.clone()).unwrap();
    assert_eq!(number, 1);
    let (two, number) = with_one.with_leader(second.clone()).unwrap();
    assert_eq!(number, 2);
    let moved = two.clone().with_leader_replaced(1, second.clone()).unwrap();
    assert_eq!(moved.leaders()[&1], second);
    let gone = moved.without_leader(1).unwrap();
    assert_eq!(gone.leaders().len(), 1);
    // The next number is one past the largest, so a number a leader had is never another's.
    let (again, number) = gone.with_leader(first).unwrap();
    assert_eq!((number, again.leaders().len()), (3, 2));
    assert_eq!(
        again.clone().without_leader(7),
        Err(TextError::UnknownLeader {
            note: i("N-1"),
            leader: 7
        })
    );
    assert_eq!(
        again.with_leader_replaced(7, second),
        Err(TextError::UnknownLeader {
            note: i("N-1"),
            leader: 7
        })
    );
}

#[test]
fn a_style_change_reaches_every_note_that_reads_it() {
    let mut texts = with_style();
    texts.add_note(note("N-1", "ST-1"), known).unwrap();
    texts.add_note(note("N-2", "ST-1"), known).unwrap();
    let bigger = texts
        .style(&i("ST-1"))
        .unwrap()
        .clone()
        .with_size(mm(7))
        .unwrap()
        .with_bold(true);
    texts.replace_style(bigger).unwrap();
    for id in ["N-1", "N-2"] {
        let note = texts.note(&i(id)).unwrap();
        // The note holds only the ID; reading it through the style gives the change.
        let read = texts.style(note.style()).unwrap();
        assert_eq!((read.size(), read.bold()), (mm(7), true));
    }
    // And the notes' own records did not change.
    assert_eq!(
        texts.note(&i("N-1")).unwrap().to_record(&i("D")),
        note("N-1", "ST-1").to_record(&i("D"))
    );
}

#[test]
fn a_style_in_use_is_not_deleted_and_a_view_with_notes_is_not_freed() {
    let mut texts = with_style();
    texts.add_style(style("ST-2", "Spare")).unwrap();
    texts.add_note(note("N-1", "ST-1"), known).unwrap();
    texts.add_note(note("N-2", "ST-1"), known).unwrap();
    let refused = texts.remove_style(&i("ST-1")).unwrap_err();
    assert_eq!(
        refused,
        TextError::StyleInUse {
            style: i("ST-1"),
            notes: 2
        }
    );
    assert!(refused.to_string().contains("used by 2 text notes"));
    assert_eq!(
        texts.remove_style(&i("ST-9")),
        Err(TextError::UnknownStyle(i("ST-9")))
    );
    // Give the notes another style and it goes.
    for id in ["N-1", "N-2"] {
        let changed = texts.note(&i(id)).unwrap().clone().with_style(i("ST-2"));
        texts.replace_note(changed).unwrap();
    }
    texts.remove_style(&i("ST-1")).unwrap();
    assert_eq!(texts.uses_of(&i("ST-2")), 2);

    assert_eq!(
        texts.view_free(&i("V-1")),
        Err(TextError::ViewHasNotes {
            view: i("V-1"),
            notes: 2
        })
    );
    texts.remove_note(&i("N-1")).unwrap();
    texts.remove_note(&i("N-2")).unwrap();
    texts.view_free(&i("V-1")).unwrap();
    assert!(texts.view_free(&i("V-2")).is_ok());
}

#[test]
fn text_is_cleaned_and_lists_and_rotation_are_kept() {
    let made = TextNote::new(
        i("N-1"),
        i("V-1"),
        i("ST-1"),
        (mm(0), mm(0)),
        "one\r\ntwo\tthree\n\n",
    )
    .unwrap();
    // Line endings are line feeds, trailing blank lines go, tabs stay.
    assert_eq!(made.lines(), ["one", "two\tthree"]);
    let turned = made
        .with_list(ListKind::Number)
        .with_rotation(Angle::from_units(-TURN))
        .unwrap();
    assert_eq!(turned.list(), ListKind::Number);
    assert_eq!(turned.rotation().units(), -TURN);
}
