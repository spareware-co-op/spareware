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

#[test]
fn renaming_a_parameter_rewrites_only_its_names_in_the_one_spelling() {
    for (text, from, to, renamed) in [
        (
            "Width - 2 * Frame",
            "Width",
            "Opening",
            Some("Opening - 2 * Frame"),
        ),
        ("Width - 2 * Frame", "Frame", "m", Some("Width - 2 * m")),
        (
            "min(Width, 900 mm) + Width",
            "Width",
            "W",
            Some("min(W, 900 mm) + W"),
        ),
        (
            "Wide ? Width : 2 * Width",
            "Width",
            "Leaf",
            Some("Wide ? Leaf : 2 * Leaf"),
        ),
        ("-(a + b) / c", "b", "bb", Some("-(a + bb) / c")),
        // A function's name and a unit are not the parameter's.
        ("min(a, 2 mm)", "min", "least", None),
        ("2 mm", "mm", "x", None),
        ("Width - 2 * Frame", "Height", "Tall", None),
    ] {
        assert_eq!(
            construct_formulas::rename(text, from, to)
                .unwrap()
                .as_deref(),
            renamed,
            "{text}"
        );
        if let Some(renamed) = renamed {
            assert_eq!(canonical(renamed).unwrap(), renamed);
        }
    }
    // A name that can't stand in a formula is refused, only where the formula uses the parameter.
    let refused = construct_formulas::rename("Width / 2", "Width", "Door width").unwrap_err();
    assert_eq!(refused.code, "Name");
    assert_eq!(
        construct_formulas::rename("Frame / 2", "Width", "Door width").unwrap(),
        None
    );
}
