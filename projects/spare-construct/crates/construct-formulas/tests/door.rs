//! A door family's parameters, as a family recipe writes them (note A07's door, in the same
//! style): sizes in mixed units, formulas of other parameters in any order, and a choice.

use construct_formulas::{Entry, Kind, evaluate};

fn entry(name: &str, text: &str, kind: Kind) -> Entry {
    Entry {
        name: name.to_owned(),
        text: text.to_owned(),
        kind,
    }
}

#[test]
fn a_door_works_out_exactly() {
    let table = [
        entry(
            "LeafWidth",
            "Width - 2 * FrameWidth - 2 * Clearance",
            Kind::Length,
        ),
        entry("Width", "2 ft 8 in", Kind::Length),
        entry("FrameWidth", "32 mm", Kind::Length),
        entry("Clearance", "3", Kind::Length),
        entry("Height", "2040", Kind::Length),
        entry("Panels", "Height > 2100 mm ? 3 : 2", Kind::Count),
        entry(
            "PanelHeight",
            "(Height - FrameWidth) / Panels",
            Kind::Length,
        ),
        entry("Swing", "90 deg", Kind::Angle),
    ];
    let values: Vec<f64> = evaluate(&table)
        .into_iter()
        .map(|r| r.unwrap().to_f64())
        .collect();
    assert_eq!(values, [742.8, 812.8, 32.0, 3.0, 2040.0, 2.0, 1004.0, 90.0]);
}

#[test]
fn a_door_that_doesnt_add_up_says_why() {
    let table = [
        entry("Width", "820 mm", Kind::Length),
        entry("Swing", "Width / 9", Kind::Angle),
    ];
    let out = evaluate(&table);
    let why = out[1].as_ref().unwrap_err();
    assert_eq!(why.code, "Units");
    assert_eq!(
        why.message,
        "This works out to a length, and it should be an angle."
    );
}
