//! The stored form: one spelling, same bytes, and refusals as values.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_styles::{LineStyles, Micrometres, Segment, StyleError};

fn styles_with_changes() -> LineStyles {
    let base = LineStyles::default_metric();
    let widths: Vec<Micrometres> = [700, 700, 700, 500, 500, 350, 350, 250, 180, 130]
        .iter()
        .map(|w| Micrometres::new(*w).unwrap())
        .collect();
    let (patterns, _) = base
        .patterns
        .with_pattern(
            "Long dash",
            vec![
                Segment::Dash(Micrometres::new(20_000).unwrap()),
                Segment::Gap(Micrometres::new(3000).unwrap()),
            ],
        )
        .unwrap();
    LineStyles {
        model_pens: base.model_pens.with_pen_widths(6, &widths).unwrap(),
        annotation_pens: base
            .annotation_pens
            .with_width(1, Micrometres::new(200).unwrap())
            .unwrap(),
        patterns,
    }
}

fn default_text() -> String {
    String::from_utf8(LineStyles::default_metric().to_bytes().unwrap()).unwrap()
}

fn read(text: &str) -> Result<LineStyles, StyleError> {
    LineStyles::from_bytes(text.as_bytes())
}

#[test]
fn the_default_bytes_start_and_end_as_pinned() {
    let text = default_text();
    let start = r#"{"format":1,"model_pens":{"scales":[1,2,5,10,20,50,100,200,500,1000],"pens":[{"number":1,"name":"Pen 1","widths":[130,130,130,130,130,130,130,130,130,130]},"#;
    let end =
        r#"{"id":6,"name":"Hidden","segments":[{"dash":3000},{"gap":1000}]}],"next_id":100}}"#;
    assert!(text.starts_with(start), "{text}");
    assert!(text.ends_with(end), "{text}");
}

#[test]
fn the_same_styles_are_the_same_bytes() {
    let a = styles_with_changes().to_bytes().unwrap();
    let b = styles_with_changes().to_bytes().unwrap();
    assert_eq!(a, b);
    assert_eq!(
        LineStyles::default_metric().to_bytes().unwrap(),
        LineStyles::default_metric().to_bytes().unwrap()
    );
    assert_ne!(a, LineStyles::default_metric().to_bytes().unwrap());
}

#[test]
fn bytes_round_trip() {
    for styles in [LineStyles::default_metric(), styles_with_changes()] {
        let bytes = styles.to_bytes().unwrap();
        let back = LineStyles::from_bytes(&bytes).unwrap();
        assert_eq!(back, styles);
        assert_eq!(back.to_bytes().unwrap(), bytes);
    }
}

#[test]
fn unreadable_or_unknown_input_is_refused() {
    assert!(matches!(read(""), Err(StyleError::Format(_))));
    assert!(matches!(read("{}"), Err(StyleError::Format(_))));
    let extra = default_text().replacen(r#""format":1"#, r#""format":1,"extra":0"#, 1);
    assert!(matches!(read(&extra), Err(StyleError::Format(_))));
}

#[test]
fn another_version_is_refused() {
    let two = default_text().replacen(r#""format":1"#, r#""format":2"#, 1);
    assert_eq!(read(&two), Err(StyleError::Version { found: 2 }));
}

#[test]
fn another_spelling_is_refused() {
    let spaced = default_text().replacen(r#""format":1,"#, r#""format": 1,"#, 1);
    assert_eq!(read(&spaced), Err(StyleError::NotCanonical));
}

#[test]
fn a_table_that_breaks_a_rule_is_refused_on_reading() {
    // A pen that grows thicker toward a smaller scale.
    let grows = default_text().replacen(
        "[130,130,130,130,130,130,130,130,130,130]",
        "[130,130,130,130,130,130,130,130,130,180]",
        1,
    );
    assert!(matches!(read(&grows), Err(StyleError::Format(text)) if text.contains("thicker")));
    // A built-in pattern changed.
    let changed = default_text().replacen(
        r#"{"dash":3000},{"gap":1000}"#,
        r#"{"dash":3000},{"gap":1500}"#,
        1,
    );
    assert!(matches!(read(&changed), Err(StyleError::Format(text)) if text.contains("built-in")));
    // A zero-length dash.
    let zero = default_text().replacen(r#"{"dash":3000}"#, r#"{"dash":0}"#, 1);
    assert!(matches!(read(&zero), Err(StyleError::Format(_))));
}

#[test]
fn a_user_pattern_that_draws_nothing_is_refused_on_reading() {
    let text = String::from_utf8(styles_with_changes().to_bytes().unwrap()).unwrap();
    let bad = text.replacen(r#"{"dash":20000},"#, "", 1);
    assert!(matches!(read(&bad), Err(StyleError::Format(text)) if text.contains("dash or dot")));
}
