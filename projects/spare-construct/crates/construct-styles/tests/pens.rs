//! The pen tables: the lookup rule, the checks and the pure changes.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_styles::{
    AnnotationPen, AnnotationPens, Micrometres, Pen, PenTable, Scale, StyleError,
};

fn um(value: u32) -> Micrometres {
    Micrometres::new(value).unwrap()
}

fn scale(denominator: u32) -> Scale {
    Scale::new(denominator).unwrap()
}

/// Scales 1:1, 1:10, 1:100, and one pen whose three widths differ.
fn small_table() -> PenTable {
    PenTable::new(
        vec![scale(1), scale(10), scale(100)],
        vec![Pen {
            number: 1,
            name: "Fine".to_owned(),
            widths: vec![um(500), um(350), um(180)],
        }],
    )
    .unwrap()
}

#[test]
fn a_listed_scale_takes_its_own_row() {
    let table = small_table();
    assert_eq!(table.width(1, scale(1)).unwrap(), um(500));
    assert_eq!(table.width(1, scale(10)).unwrap(), um(350));
    assert_eq!(table.width(1, scale(100)).unwrap(), um(180));
}

#[test]
fn a_scale_between_two_listed_takes_the_nearer_coarser_row() {
    let table = small_table();
    // 1:2 and 1:9 lie between 1:1 and 1:10: the coarser is 1:10, even when 1:2 is nearer 1:1.
    assert_eq!(table.width(1, scale(2)).unwrap(), um(350));
    assert_eq!(table.width(1, scale(9)).unwrap(), um(350));
    // One past a listed scale takes the next coarser row.
    assert_eq!(table.width(1, scale(11)).unwrap(), um(180));
    assert_eq!(table.width(1, scale(75)).unwrap(), um(180));
}

#[test]
fn a_scale_beyond_the_ends_takes_the_end_row() {
    let wide = PenTable::new(
        vec![scale(5), scale(50)],
        vec![Pen {
            number: 3,
            name: "P".to_owned(),
            widths: vec![um(700), um(250)],
        }],
    )
    .unwrap();
    // Larger than the first listed scale (1:1 is larger than 1:5): the first row.
    assert_eq!(wide.width(3, scale(1)).unwrap(), um(700));
    // Smaller than the last listed (1:5000): the last row.
    assert_eq!(wide.width(3, scale(5000)).unwrap(), um(250));
    assert_eq!(wide.row_for(scale(5000)), 1);
    assert_eq!(wide.row_for(scale(1)), 0);
}

#[test]
fn an_unknown_pen_is_an_error_value() {
    let table = small_table();
    assert_eq!(
        table.width(2, scale(1)),
        Err(StyleError::NoSuchPen { pen: 2 })
    );
}

#[test]
fn changing_a_pen_returns_a_new_table_and_keeps_the_old() {
    let table = small_table();
    let changed = table
        .with_pen_widths(1, &[um(700), um(500), um(250)])
        .unwrap();
    assert_eq!(changed.width(1, scale(10)).unwrap(), um(500));
    assert_eq!(table.width(1, scale(10)).unwrap(), um(350));
}

#[test]
fn changing_a_pen_refuses_bad_widths() {
    let table = small_table();
    assert_eq!(
        table.with_pen_widths(1, &[um(500), um(350)]),
        Err(StyleError::WidthCount {
            pen: 1,
            expected: 3,
            found: 2
        })
    );
    assert_eq!(
        table.with_pen_widths(1, &[um(500), um(350), um(400)]),
        Err(StyleError::WidthGrows {
            pen: 1,
            scale_index: 2
        })
    );
    assert_eq!(
        table.with_pen_widths(1, &[um(10_001), um(350), um(180)]),
        Err(StyleError::BadLength {
            micrometres: 10_001
        })
    );
    assert_eq!(
        table.with_pen_widths(9, &[um(500), um(350), um(180)]),
        Err(StyleError::NoSuchPen { pen: 9 })
    );
}

#[test]
fn equal_widths_at_every_scale_are_allowed() {
    let table = small_table();
    assert!(
        table
            .with_pen_widths(1, &[um(350), um(350), um(350)])
            .is_ok()
    );
}

#[test]
fn zero_and_huge_values_cannot_be_made() {
    assert_eq!(
        Micrometres::new(0),
        Err(StyleError::BadLength { micrometres: 0 })
    );
    assert_eq!(
        Micrometres::new(1_000_001),
        Err(StyleError::BadLength {
            micrometres: 1_000_001
        })
    );
    assert_eq!(Scale::new(0), Err(StyleError::BadScale { denominator: 0 }));
    assert_eq!(
        Scale::new(1_000_001),
        Err(StyleError::BadScale {
            denominator: 1_000_001
        })
    );
}

#[test]
fn a_table_that_breaks_a_rule_is_refused() {
    let pen = |number: u16, name: &str| Pen {
        number,
        name: name.to_owned(),
        widths: vec![um(250), um(180)],
    };
    let scales = || vec![scale(1), scale(10)];
    assert_eq!(
        PenTable::new(vec![], vec![pen(1, "A")]),
        Err(StyleError::NoScales)
    );
    assert_eq!(PenTable::new(scales(), vec![]), Err(StyleError::NoPens));
    assert_eq!(
        PenTable::new(vec![scale(10), scale(1)], vec![pen(1, "A")]),
        Err(StyleError::ScalesNotAscending)
    );
    assert_eq!(
        PenTable::new(vec![scale(10), scale(10)], vec![pen(1, "A")]),
        Err(StyleError::ScalesNotAscending)
    );
    assert_eq!(
        PenTable::new(scales(), vec![pen(0, "A")]),
        Err(StyleError::PensNotAscending)
    );
    assert_eq!(
        PenTable::new(scales(), vec![pen(2, "A"), pen(1, "B")]),
        Err(StyleError::PensNotAscending)
    );
    assert_eq!(
        PenTable::new(scales(), vec![pen(1, "A"), pen(1, "B")]),
        Err(StyleError::PensNotAscending)
    );
    assert_eq!(
        PenTable::new(scales(), vec![pen(1, "A"), pen(2, "A")]),
        Err(StyleError::DuplicateName)
    );
    assert_eq!(
        PenTable::new(scales(), vec![pen(1, "")]),
        Err(StyleError::BadName)
    );
    assert_eq!(
        PenTable::new(scales(), vec![pen(1, " A")]),
        Err(StyleError::BadName)
    );
    assert_eq!(
        PenTable::new(scales(), vec![pen(1, "A\nB")]),
        Err(StyleError::BadName)
    );
}

fn annotation() -> AnnotationPens {
    AnnotationPens::new(vec![
        AnnotationPen {
            number: 1,
            name: "Text".to_owned(),
            width: um(180),
        },
        AnnotationPen {
            number: 2,
            name: "Dimension".to_owned(),
            width: um(250),
        },
    ])
    .unwrap()
}

#[test]
fn an_annotation_pen_has_one_width() {
    let pens = annotation();
    assert_eq!(pens.width(2).unwrap(), um(250));
    assert_eq!(pens.width(7), Err(StyleError::NoSuchPen { pen: 7 }));
}

#[test]
fn changing_an_annotation_pen_returns_a_new_table() {
    let pens = annotation();
    let changed = pens.with_width(2, um(350)).unwrap();
    assert_eq!(changed.width(2).unwrap(), um(350));
    assert_eq!(pens.width(2).unwrap(), um(250));
    assert_eq!(
        pens.with_width(3, um(350)),
        Err(StyleError::NoSuchPen { pen: 3 })
    );
    assert_eq!(
        pens.with_width(1, um(20_000)),
        Err(StyleError::BadLength {
            micrometres: 20_000
        })
    );
}

#[test]
fn an_annotation_table_that_breaks_a_rule_is_refused() {
    assert_eq!(AnnotationPens::new(vec![]), Err(StyleError::NoPens));
    let pen = |number: u16, name: &str| AnnotationPen {
        number,
        name: name.to_owned(),
        width: um(250),
    };
    assert_eq!(
        AnnotationPens::new(vec![pen(2, "A"), pen(1, "B")]),
        Err(StyleError::PensNotAscending)
    );
    assert_eq!(
        AnnotationPens::new(vec![pen(1, "A"), pen(2, "A")]),
        Err(StyleError::DuplicateName)
    );
}
