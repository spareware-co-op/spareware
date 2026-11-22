//! The default metric set, pinned value by value.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_styles::default::{
    DEFAULT_SCALE_DENOMINATORS, ISO_128_WIDTHS_MICROMETRES, default_annotation_pens,
    default_model_pens, steps_down,
};
use construct_styles::{AnnotationPens, LineStyles, PenTable, Scale};

fn scale(denominator: u32) -> Scale {
    Scale::new(denominator).unwrap()
}

#[test]
fn the_ladder_is_the_iso_128_series() {
    assert_eq!(
        ISO_128_WIDTHS_MICROMETRES,
        [130, 180, 250, 350, 500, 700, 1000, 1400, 2000]
    );
}

#[test]
fn steps_down_by_the_rule() {
    // None to 1:20, then one for each factor of 2.5 in the denominator.
    let steps: Vec<u32> = [
        1, 2, 5, 10, 20, 49, 50, 124, 125, 312, 313, 781, 782, 1_000_000,
    ]
    .iter()
    .map(|d| steps_down(scale(*d)))
    .collect();
    assert_eq!(steps, [0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 8]);
}

/// Pen 1 to 9, at 1:1, 1:2, 1:5, 1:10, 1:20, 1:50, 1:100, 1:200, 1:500, 1:1000, in micrometres.
const PINNED: [[u32; 10]; 9] = [
    [130, 130, 130, 130, 130, 130, 130, 130, 130, 130],
    [180, 180, 180, 180, 180, 130, 130, 130, 130, 130],
    [250, 250, 250, 250, 250, 180, 180, 130, 130, 130],
    [350, 350, 350, 350, 350, 250, 250, 180, 130, 130],
    [500, 500, 500, 500, 500, 350, 350, 250, 180, 130],
    [700, 700, 700, 700, 700, 500, 500, 350, 250, 180],
    [1000, 1000, 1000, 1000, 1000, 700, 700, 500, 350, 250],
    [1400, 1400, 1400, 1400, 1400, 1000, 1000, 700, 500, 350],
    [2000, 2000, 2000, 2000, 2000, 1400, 1400, 1000, 700, 500],
];

#[test]
fn the_default_model_pens_are_pinned() {
    let table = default_model_pens();
    let scales: Vec<u32> = table.scales().iter().map(|s| s.denominator()).collect();
    assert_eq!(scales, DEFAULT_SCALE_DENOMINATORS);
    assert_eq!(table.pens().len(), 9);
    for (index, row) in PINNED.iter().enumerate() {
        let number = u16::try_from(index + 1).unwrap();
        let pen = table.pen(number).unwrap();
        assert_eq!(pen.name, format!("Pen {number}"));
        let widths: Vec<u32> = pen.widths.iter().map(|w| w.get()).collect();
        assert_eq!(&widths, row, "pen {number}");
    }
}

#[test]
fn the_default_model_pens_pass_the_checks() {
    let table = default_model_pens();
    let again = PenTable::new(table.scales().to_vec(), table.pens().to_vec()).unwrap();
    assert_eq!(again, table);
}

#[test]
fn the_same_pen_draws_thinner_on_a_small_scale_drawing() {
    let table = default_model_pens();
    for number in 1..=9 {
        let large = table.width(number, scale(20)).unwrap();
        let small = table.width(number, scale(500)).unwrap();
        assert!(small <= large, "pen {number}");
    }
    assert!(table.width(9, scale(500)).unwrap() < table.width(9, scale(20)).unwrap());
}

#[test]
fn the_default_lookup_between_scales_takes_the_coarser_row() {
    let table = default_model_pens();
    // 1:150 lies between 1:100 and 1:200: the 1:200 row (pen 8: 1000 at 1:100, 700 at 1:200).
    assert_eq!(table.width(8, scale(150)).unwrap().get(), 700);
    assert_eq!(table.width(8, scale(100)).unwrap().get(), 1000);
    assert_eq!(table.width(6, scale(75)).unwrap().get(), 500);
}

#[test]
fn the_default_annotation_pens_are_the_ladder() {
    let pens = default_annotation_pens();
    assert_eq!(pens.pens().len(), 9);
    for (index, width) in ISO_128_WIDTHS_MICROMETRES.iter().enumerate() {
        let number = u16::try_from(index + 1).unwrap();
        assert_eq!(pens.width(number).unwrap().get(), *width);
    }
    let again = AnnotationPens::new(pens.pens().to_vec()).unwrap();
    assert_eq!(again, pens);
}

#[test]
fn the_default_set_is_one_value() {
    let styles = LineStyles::default_metric();
    assert_eq!(styles.model_pens, default_model_pens());
    assert_eq!(styles.patterns.patterns().len(), 6);
}
