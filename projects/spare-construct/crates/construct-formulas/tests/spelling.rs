//! A formula's one spelling (FORMAT.md beside this crate): pinned for each form, the same for
//! every way of writing one formula, and reading back to the same formula.

#![allow(clippy::unwrap_used, reason = "test code")]

use construct_formulas::{canonical, parse};

#[test]
fn each_form_has_one_spelling() {
    for (written, spelled) in [
        ("width/2-4mm", "width / 2 - 4 mm"),
        ("  Width - 2 * FrameWidth  ", "Width - 2 * FrameWidth"),
        ("2ft 8in", "2 ft 8 in"),
        ("007.50 mm", "7.5 mm"),
        (".5", "0.5"),
        ("3.0", "3"),
        ("+a", "a"),
        ("((a))", "a"),
        ("(a + b) + c", "a + b + c"),
        ("a + (b + c)", "a + (b + c)"),
        ("a - (b - c)", "a - (b - c)"),
        ("(a * b) / c", "a * b / c"),
        ("a * (b + c)", "a * (b + c)"),
        ("-(a + b)", "-(a + b)"),
        ("a - -b", "a - -b"),
        ("!(a<b)", "!(a < b)"),
        ("a<b&&b<=c||d", "a < b && b <= c || d"),
        ("(a || b) && c", "(a || b) && c"),
        ("count>3?12mm:8mm", "count > 3 ? 12 mm : 8 mm"),
        ("(a ? b : c) ? d : e", "(a ? b : c) ? d : e"),
        ("a ? b : c ? d : e", "a ? b : c ? d : e"),
        ("(a ? b : c) + d", "(a ? b : c) + d"),
        ("max( a ,3mm,b )", "max(a, 3 mm, b)"),
        ("round(x,0.010 mm)", "round(x, 0.01 mm)"),
        ("2 m2 * 1.5m", "2 m2 * 1.5 m"),
        ("90deg", "90 deg"),
    ] {
        assert_eq!(canonical(written).unwrap(), spelled, "{written}");
        // The spelling is its own spelling, and reads as the same formula as the text written.
        assert_eq!(canonical(spelled).unwrap(), spelled, "{spelled}");
        assert_eq!(
            canonical(&format!("({written})")).unwrap(),
            spelled,
            "{written}"
        );
    }
}

#[test]
fn the_spelling_reads_back_to_the_same_tree() {
    for text in [
        "Width - 2 * FrameWidth - 2 * Clearance",
        "Height > 2100 mm ? 3 : 2",
        "(Height - FrameWidth) / Panels",
        "a - (b - c) * -(d / e)",
        "!a == (b < c)",
    ] {
        let spelled = canonical(text).unwrap();
        assert_eq!(parse(&spelled).unwrap(), parse(text).unwrap(), "{text}");
    }
}

#[test]
fn what_doesnt_read_has_no_spelling() {
    for bad in ["", "a +", "2 width", "1 %", "1 mm2x"] {
        assert!(canonical(bad).is_err(), "{bad}");
    }
}
