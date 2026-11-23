//! Laying a note out, with exact numbers, on test metrics: every character is 600 thousandths of
//! the size wide and a line is 1,200 thousandths high. At a size of 5 mm a character is 3 mm wide
//! and a line is 6 mm high.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::items_after_statements,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::Length;
use construct_text::{
    Align, Face, FixedWidth, FontMetrics, ListKind, TextError, TextNote, TextStyle, lay_out,
};
use kit_core::Id;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

const METRICS: FixedWidth = FixedWidth {
    advance: 600,
    line_height: 1200,
};

fn style() -> TextStyle {
    TextStyle::new(i("ST-1"), "Notes", "Test Sans", mm(5)).unwrap()
}

fn note(text: &str) -> TextNote {
    TextNote::new(i("N-1"), i("V-1"), i("ST-1"), (mm(100), mm(200)), text).unwrap()
}

/// Each laid line as its text and width in millimetres.
fn shown(note: &TextNote, style: &TextStyle) -> Vec<(String, i64)> {
    lay_out(note, style, &METRICS)
        .unwrap()
        .lines
        .iter()
        .map(|line| {
            (
                line.text.clone(),
                line.width.whole_mm().expect("whole millimetres here"),
            )
        })
        .collect()
}

#[test]
fn a_wrapped_note_breaks_at_the_width_with_exact_sizes() {
    let wrapped = note("hello wide world").with_width(Some(mm(30))).unwrap();
    let laid = lay_out(&wrapped, &style(), &METRICS).unwrap();
    // 30 mm holds ten characters: "hello wide" exactly, then "world".
    assert_eq!(
        shown(&wrapped, &style()),
        [("hello wide".to_owned(), 30), ("world".to_owned(), 15)]
    );
    assert_eq!(laid.line_height, mm(6));
    assert_eq!((laid.width, laid.height), (mm(30), mm(12)));
    assert_eq!(laid.lines[0].top, mm(0));
    assert_eq!(laid.lines[1].top, mm(6));
    // One character narrower and the second word moves down too.
    let narrower = note("hello wide world").with_width(Some(mm(29))).unwrap();
    assert_eq!(
        shown(&narrower, &style()),
        [
            ("hello".to_owned(), 15),
            ("wide".to_owned(), 12),
            ("world".to_owned(), 15)
        ]
    );
}

#[test]
fn a_note_that_does_not_wrap_is_one_line_per_paragraph_and_as_wide_as_its_widest() {
    let plain = note("ab\ncdef");
    let laid = lay_out(&plain, &style(), &METRICS).unwrap();
    assert_eq!(
        shown(&plain, &style()),
        [("ab".to_owned(), 6), ("cdef".to_owned(), 12)]
    );
    assert_eq!((laid.width, laid.height), (mm(12), mm(12)));
    // Trailing space is not part of a line.
    assert_eq!(shown(&note("ab  "), &style()), [("ab".to_owned(), 6)]);
}

#[test]
fn the_box_runs_right_and_down_from_the_anchor() {
    let wrapped = note("hello wide world").with_width(Some(mm(30))).unwrap();
    let laid = lay_out(&wrapped, &style(), &METRICS).unwrap();
    assert_eq!(
        laid.bounds(&wrapped).unwrap(),
        [mm(100), mm(188), mm(130), mm(200)]
    );
}

#[test]
fn alignment_places_each_line_in_the_box() {
    let at = |align| {
        let aligned = note("hi")
            .with_width(Some(mm(30)))
            .unwrap()
            .with_align(align);
        lay_out(&aligned, &style(), &METRICS).unwrap().lines[0].x
    };
    // The line is 6 mm wide in a box 30 mm wide.
    assert_eq!(at(Align::Left), mm(0));
    assert_eq!(at(Align::Centre), mm(12));
    assert_eq!(at(Align::Right), mm(24));
    // An odd room rounds down to a whole step, never up.
    let odd = note("hi")
        .with_width(Some(Length::from_steps(57_601)))
        .unwrap()
        .with_align(Align::Centre);
    let x = lay_out(&odd, &style(), &METRICS).unwrap().lines[0].x;
    assert_eq!(x.steps(), (57_601 - 38_400) / 2);
}

#[test]
fn a_word_wider_than_the_width_keeps_a_line_and_runs_past_the_box() {
    let long = note("abcdefghijklmnop").with_width(Some(mm(30))).unwrap();
    let laid = lay_out(&long, &style(), &METRICS).unwrap();
    assert_eq!(
        shown(&long, &style()),
        [("abcdefghijklmnop".to_owned(), 48)]
    );
    assert_eq!(laid.width, mm(30));
    assert_eq!(laid.lines[0].x, mm(0));
}

#[test]
fn a_blank_line_takes_a_line_and_the_spacing_scales_every_line() {
    let blank = note("a\n\nb");
    let laid = lay_out(&blank, &style(), &METRICS).unwrap();
    assert_eq!(laid.lines.len(), 3);
    assert_eq!(laid.lines[1].text, "");
    assert_eq!(laid.height, mm(18));
    let loose = style().with_spacing(150).unwrap();
    let laid = lay_out(&blank, &loose, &METRICS).unwrap();
    assert_eq!(laid.line_height, mm(9));
    assert_eq!(laid.lines[2].top, mm(18));
}

#[test]
fn lists_set_a_marker_and_hang_the_continuation_lines() {
    let bullets = note("one two three\nfour")
        .with_width(Some(mm(30)))
        .unwrap()
        .with_list(ListKind::Bullet);
    let laid = lay_out(&bullets, &style(), &METRICS).unwrap();
    // The marker and a space take 6 mm, leaving 24 mm, eight characters, for the text.
    let seen: Vec<(&str, &str, i64)> = laid
        .lines
        .iter()
        .map(|l| {
            (
                l.marker.as_str(),
                l.text.as_str(),
                l.indent.whole_mm().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("\u{2022}", "one two", 6),
            ("", "three", 6),
            ("\u{2022}", "four", 6)
        ]
    );
    // The block is the indent and the text: 6 + 21 mm for the first line.
    assert_eq!(laid.lines[0].width, mm(27));

    let numbered = note("a\n\nb").with_list(ListKind::Number);
    let laid = lay_out(&numbered, &style(), &METRICS).unwrap();
    let markers: Vec<&str> = laid.lines.iter().map(|l| l.marker.as_str()).collect();
    // A blank line is not an item and takes no number.
    assert_eq!(markers, ["1.", "", "2."]);
}

#[test]
fn a_measured_width_is_rounded_up_to_a_whole_step() {
    // One thousandth of a one-step size is a thousandth of a step: it counts as a whole step.
    let tiny = TextStyle::new(i("ST-2"), "Tiny", "Test Sans", Length::from_steps(1)).unwrap();
    let thin = FixedWidth {
        advance: 1,
        line_height: 1,
    };
    let laid = lay_out(&note("ab"), &tiny, &thin).unwrap();
    assert_eq!(laid.width.steps(), 1);
    assert_eq!(laid.line_height.steps(), 1);
}

#[test]
fn the_metrics_get_the_style_s_face_and_their_refusal_comes_back() {
    struct Spy;
    impl FontMetrics for Spy {
        fn advance(&self, face: &Face<'_>, _: &str) -> Result<u64, TextError> {
            assert_eq!(
                face,
                &Face {
                    font: "Test Sans",
                    bold: true,
                    italic: true
                }
            );
            Ok(500)
        }
        fn line_height(&self, _: &Face<'_>) -> Result<u64, TextError> {
            Ok(1000)
        }
    }
    let styled = style().with_bold(true).with_italic(true);
    let laid = lay_out(&note("ab"), &styled, &Spy).unwrap();
    assert_eq!(laid.width, Length::from_steps(16_000));

    struct Unknown;
    impl FontMetrics for Unknown {
        fn advance(&self, face: &Face<'_>, _: &str) -> Result<u64, TextError> {
            Err(TextError::Metrics(format!("no font {}", face.font)))
        }
        fn line_height(&self, face: &Face<'_>) -> Result<u64, TextError> {
            Err(TextError::Metrics(format!("no font {}", face.font)))
        }
    }
    assert_eq!(
        lay_out(&note("ab"), &style(), &Unknown),
        Err(TextError::Metrics("no font Test Sans".to_owned()))
    );
}

#[test]
fn a_style_change_changes_the_layout_and_nothing_is_kept() {
    let wrapped = note("hello wide world").with_width(Some(mm(30))).unwrap();
    let before = lay_out(&wrapped, &style(), &METRICS).unwrap();
    // Twice the size: the same note, the same record, a different box.
    let bigger = style().with_size(mm(10)).unwrap();
    let after = lay_out(&wrapped, &bigger, &METRICS).unwrap();
    assert_ne!(before, after);
    assert_eq!(after.line_height, mm(12));
    assert_eq!(
        shown(&wrapped, &bigger),
        [
            ("hello".to_owned(), 30),
            ("wide".to_owned(), 24),
            ("world".to_owned(), 30)
        ]
    );
}
