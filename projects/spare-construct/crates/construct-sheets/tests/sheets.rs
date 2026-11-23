//! The rules a document of sheets keeps: numbers, paper, a view on one sheet, a centre on the
//! paper, alignments, the standard parameters.

#![allow(
    clippy::unwrap_used,
    clippy::unnecessary_wraps,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{CategoryTable, SHEETS};
use construct_params::{Length, ProjectParams};
use construct_sheets::{
    Alignment, Anchor, MAX_CUSTOM, MIN_CUSTOM, Orientation, Paper, PaperSize, STANDARD, Sheet,
    SheetError, Sheets, Viewport, with_standard,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn a1() -> Paper {
    Paper::Standard {
        size: PaperSize::A1,
        orientation: Orientation::Landscape,
    }
}

fn sheet(sheet: &str, number: &str) -> Sheet {
    Sheet::new(id(sheet), number, "Ground floor", a1()).unwrap()
}

fn viewport(vp: &str, on: &str, view: &str, x: i32, y: i32) -> Viewport {
    Viewport::new(id(vp), id(on), id(view), mm(x), mm(y))
}

fn one_sheet() -> Sheets {
    let mut sheets = Sheets::new();
    sheets.add_sheet(sheet("s1", "A101")).unwrap();
    sheets
}

#[test]
fn standard_paper_is_iso_216_in_either_orientation() {
    let all = [
        (PaperSize::A0, 841, 1189),
        (PaperSize::A1, 594, 841),
        (PaperSize::A2, 420, 594),
        (PaperSize::A3, 297, 420),
        (PaperSize::A4, 210, 297),
        (PaperSize::A5, 148, 210),
    ];
    for (size, short, long) in all {
        let landscape = Paper::Standard {
            size,
            orientation: Orientation::Landscape,
        };
        let portrait = Paper::Standard {
            size,
            orientation: Orientation::Portrait,
        };
        assert_eq!(
            (landscape.width(), landscape.height()),
            (mm(long), mm(short))
        );
        assert_eq!((portrait.width(), portrait.height()), (mm(short), mm(long)));
        assert_eq!(PaperSize::from_key(size.key()), Some(size));
    }
    assert_eq!(a1().describe(), "A1 landscape (841 mm by 594 mm)");
}

#[test]
fn custom_paper_is_exact_and_held_to_its_limits() {
    let odd = Paper::Custom {
        width: Length::from_steps(1_900_800),
        height: mm(297),
    };
    assert!(odd.check().is_ok());
    assert_eq!(odd.width().steps(), 1_900_800);
    assert_eq!(odd.describe(), "297 mm by 297 mm");
    let half = Paper::Custom {
        width: Length::from_steps(1_904_000),
        height: mm(200),
    };
    assert_eq!(half.describe(), "297.5 mm by 200 mm");
    for (width, height) in [
        (Length::from_steps(MIN_CUSTOM.steps() - 1), mm(200)),
        (mm(200), Length::from_steps(MAX_CUSTOM.steps() + 1)),
    ] {
        let error = Sheet::new(id("s"), "1", "Name", Paper::Custom { width, height }).unwrap_err();
        assert!(matches!(error, SheetError::BadPaper(_)), "{error}");
    }
    let edge = Paper::Custom {
        width: MIN_CUSTOM,
        height: MAX_CUSTOM,
    };
    assert!(Sheet::new(id("s"), "1", "Name", edge).is_ok());
}

#[test]
fn a_sheet_needs_a_name_and_a_sound_number() {
    let new = |number: &str, name: &str| Sheet::new(id("s"), number, name, a1());
    assert_eq!(new("A1", "  ").unwrap_err(), SheetError::EmptyName);
    assert_eq!(new("   ", "Plan").unwrap_err(), SheetError::EmptyNumber);
    assert!(matches!(
        new(&"9".repeat(33), "Plan"),
        Err(SheetError::BadNumber(_))
    ));
    assert!(matches!(new("A\n1", "Plan"), Err(SheetError::BadNumber(_))));
    assert_eq!(new("  A-101 ", " Plan ").unwrap().number(), "A-101");
    assert_eq!(new("A-101", " Plan ").unwrap().name(), "Plan");
    assert!(new(&"9".repeat(32), "Plan").is_ok());
}

#[test]
fn a_number_is_unique_in_the_document_whatever_its_case() {
    let mut sheets = one_sheet();
    let error = sheets.add_sheet(sheet("s2", "a101")).unwrap_err();
    assert_eq!(error, SheetError::DuplicateNumber("a101".to_owned()));
    assert_eq!(
        error.to_string(),
        "There is already a sheet numbered \"a101\" in this model."
    );
    assert_eq!(
        sheets.add_sheet(sheet("s1", "B1")).unwrap_err(),
        SheetError::DuplicateSheet(id("s1"))
    );
    sheets.add_sheet(sheet("s2", "A102")).unwrap();
    // A sheet may change the case of its own number, never take another's.
    let own = sheets
        .sheet(&id("s1"))
        .unwrap()
        .clone()
        .with_number("a101")
        .unwrap();
    sheets.replace_sheet(own).unwrap();
    let taken = sheets
        .sheet(&id("s2"))
        .unwrap()
        .clone()
        .with_number("A101")
        .unwrap();
    assert_eq!(
        sheets.replace_sheet(taken).unwrap_err(),
        SheetError::DuplicateNumber("A101".to_owned())
    );
    let order: Vec<&str> = sheets.ordered().iter().map(|s| s.number()).collect();
    assert_eq!(order, ["a101", "A102"]);
    assert_eq!(
        sheets.replace_sheet(sheet("nope", "Z")).unwrap_err(),
        SheetError::UnknownSheet(id("nope"))
    );
}

#[test]
fn a_view_is_on_one_sheet_only() {
    let mut sheets = one_sheet();
    sheets.add_sheet(sheet("s2", "A102")).unwrap();
    sheets
        .add_viewport(viewport("p1", "s1", "v1", 100, 100))
        .unwrap();
    let again = sheets
        .add_viewport(viewport("p2", "s2", "v1", 100, 100))
        .unwrap_err();
    assert_eq!(
        again,
        SheetError::ViewPlaced {
            view: id("v1"),
            sheet_number: "A101".to_owned()
        }
    );
    assert_eq!(
        again.to_string(),
        "The view v1 is already placed, on sheet \"A101\"; a view is on one sheet only."
    );
    // Not twice on one sheet either.
    assert!(matches!(
        sheets.add_viewport(viewport("p3", "s1", "v1", 200, 200)),
        Err(SheetError::ViewPlaced { .. })
    ));
    assert_eq!(sheets.placement_of(&id("v1")).unwrap().0.number(), "A101");
    assert!(sheets.placement_of(&id("v2")).is_none());
    // Taking the viewport away frees the view.
    sheets.remove_viewport(&id("p1")).unwrap();
    sheets
        .add_viewport(viewport("p2", "s2", "v1", 100, 100))
        .unwrap();
    assert_eq!(sheets.placement_of(&id("v1")).unwrap().0.number(), "A102");
    assert_eq!(
        sheets
            .add_viewport(viewport("p2", "s2", "v9", 1, 1))
            .unwrap_err(),
        SheetError::DuplicateViewport(id("p2"))
    );
    assert_eq!(
        sheets
            .add_viewport(viewport("p9", "nope", "v9", 1, 1))
            .unwrap_err(),
        SheetError::UnknownSheet(id("nope"))
    );
}

#[test]
fn a_centre_is_on_the_paper_edges_included() {
    let mut sheets = one_sheet();
    // A1 landscape is 841 by 594 mm.
    sheets
        .add_viewport(viewport("p1", "s1", "v1", 0, 0))
        .unwrap();
    sheets
        .add_viewport(viewport("p2", "s1", "v2", 841, 594))
        .unwrap();
    let off = sheets
        .add_viewport(viewport("p3", "s1", "v3", 842, 10))
        .unwrap_err();
    assert_eq!(
        off.to_string(),
        "A viewport centred at 842 mm, 10 mm is off sheet \"A101\", which is A1 landscape (841 mm by 594 mm)."
    );
    let just = Viewport::new(
        id("p4"),
        id("s1"),
        id("v4"),
        mm(10),
        Length::from_steps(594 * 6400 + 1),
    );
    assert!(matches!(
        sheets.add_viewport(just),
        Err(SheetError::OffPaper { .. })
    ));
    let below = Viewport::new(id("p5"), id("s1"), id("v5"), Length::from_steps(-1), mm(10));
    assert!(matches!(
        sheets.add_viewport(below),
        Err(SheetError::OffPaper { .. })
    ));
    // Smaller paper that would leave a centre off it is refused.
    let a4 = Paper::Standard {
        size: PaperSize::A4,
        orientation: Orientation::Landscape,
    };
    let smaller = sheets
        .sheet(&id("s1"))
        .unwrap()
        .clone()
        .with_paper(a4)
        .unwrap();
    assert!(matches!(
        sheets.replace_sheet(smaller),
        Err(SheetError::OffPaper { .. })
    ));
}

#[test]
fn moving_a_viewport_is_exact_and_keeps_its_sheet_and_view() {
    let mut sheets = one_sheet();
    sheets.add_sheet(sheet("s2", "A102")).unwrap();
    sheets
        .add_viewport(viewport("p1", "s1", "v1", 100, 100))
        .unwrap();
    let moved = sheets
        .viewport(&id("p1"))
        .unwrap()
        .clone()
        .at(Length::from_steps(640_001), Length::from_steps(1));
    sheets.replace_viewport(moved).unwrap();
    let now = sheets.viewport(&id("p1")).unwrap().clone();
    assert_eq!((now.x().steps(), now.y().steps()), (640_001, 1));
    let elsewhere = Viewport::new(id("p1"), id("s2"), id("v1"), mm(1), mm(1));
    assert!(matches!(
        sheets.replace_viewport(elsewhere),
        Err(SheetError::BadAlignment(_))
    ));
    let other_view = Viewport::new(id("p1"), id("s1"), id("v2"), mm(1), mm(1));
    assert!(matches!(
        sheets.replace_viewport(other_view),
        Err(SheetError::BadAlignment(_))
    ));
    assert_eq!(
        sheets
            .replace_viewport(viewport("p9", "s1", "v9", 1, 1))
            .unwrap_err(),
        SheetError::UnknownViewport(id("p9"))
    );
    let off = now.clone().at(mm(900), mm(1));
    assert!(matches!(
        sheets.replace_viewport(off),
        Err(SheetError::OffPaper { .. })
    ));
    assert!(
        !sheets
            .viewport(&id("p1"))
            .unwrap()
            .clone()
            .with_title(false)
            .title()
    );
    assert!(sheets.viewport(&id("p1")).unwrap().title());
}

fn align(to: &str, anchor: Anchor) -> Option<Alignment> {
    Some(Alignment { to: id(to), anchor })
}

#[test]
fn alignments_name_viewports_of_the_sheet_and_never_loop() {
    let mut sheets = one_sheet();
    sheets.add_sheet(sheet("s2", "A102")).unwrap();
    sheets
        .add_viewport(viewport("p1", "s1", "v1", 100, 100))
        .unwrap();
    sheets
        .add_viewport(viewport("p2", "s1", "v2", 300, 300))
        .unwrap();
    sheets
        .add_viewport(viewport("p3", "s2", "v3", 300, 300))
        .unwrap();
    let base = sheets.clone();
    let with = |one: &str, x: Option<Alignment>, y: Option<Alignment>| {
        base.viewport(&id(one))
            .unwrap()
            .clone()
            .with_align_x(x)
            .with_align_y(y)
    };
    // Itself, an unknown one, one on another sheet.
    assert!(matches!(
        sheets
            .clone()
            .replace_viewport(with("p1", align("p1", Anchor::Min), None)),
        Err(SheetError::BadAlignment(_))
    ));
    assert_eq!(
        sheets
            .clone()
            .replace_viewport(with("p1", align("nope", Anchor::Min), None))
            .unwrap_err(),
        SheetError::UnknownViewport(id("nope"))
    );
    let other_sheet = sheets
        .clone()
        .replace_viewport(with("p1", None, align("p3", Anchor::Max)));
    assert!(matches!(other_sheet, Err(SheetError::BadAlignment(_))));
    // A loop of two, on one axis; the other axis may point back.
    sheets
        .replace_viewport(with("p1", align("p2", Anchor::Centre), None))
        .unwrap();
    let loops = sheets
        .clone()
        .replace_viewport(with("p2", align("p1", Anchor::Centre), None));
    assert!(matches!(loops, Err(SheetError::BadAlignment(_))));
    sheets
        .replace_viewport(with("p2", None, align("p1", Anchor::Min)))
        .unwrap();
    // A refused change leaves the document as it was.
    assert_eq!(sheets.viewport(&id("p2")).unwrap().align_x(), None);
}

#[test]
fn an_aligned_axis_is_not_placed_by_hand_and_a_followed_viewport_is_kept() {
    let mut sheets = one_sheet();
    sheets
        .add_viewport(viewport("p1", "s1", "v1", 100, 100))
        .unwrap();
    sheets
        .add_viewport(viewport("p2", "s1", "v2", 300, 300))
        .unwrap();
    let aligned = sheets
        .viewport(&id("p2"))
        .unwrap()
        .clone()
        .with_align_x(align("p1", Anchor::Max));
    sheets.replace_viewport(aligned).unwrap();
    let p2 = sheets.viewport(&id("p2")).unwrap().clone();
    let by_hand = p2.clone().at(mm(310), mm(300));
    let error = sheets.replace_viewport(by_hand).unwrap_err();
    assert_eq!(
        error,
        SheetError::Aligned("its horizontal position".to_owned())
    );
    // The other axis is free; so is the title.
    sheets
        .replace_viewport(p2.clone().at(mm(300), mm(320)).with_title(false))
        .unwrap();
    // Removing the alignment frees the axis (and the stored position stays as it was).
    let free = sheets
        .viewport(&id("p2"))
        .unwrap()
        .clone()
        .with_align_x(None);
    sheets.replace_viewport(free).unwrap();
    assert_eq!(sheets.viewport(&id("p2")).unwrap().x(), mm(300));
    // A viewport others follow is not taken away.
    let follow = sheets
        .viewport(&id("p2"))
        .unwrap()
        .clone()
        .with_align_y(align("p1", Anchor::Min));
    sheets.replace_viewport(follow).unwrap();
    let kept = sheets.remove_viewport(&id("p1")).unwrap_err();
    assert_eq!(
        kept,
        SheetError::Followed {
            viewport: id("p1"),
            follower: id("p2")
        }
    );
    assert_eq!(
        kept.to_string(),
        "The viewport p1 is followed by p2, which is aligned to it."
    );
    assert_eq!(
        sheets.remove_viewport(&id("nope")).unwrap_err(),
        SheetError::UnknownViewport(id("nope"))
    );
    // Removing a sheet takes its viewports, followers and all.
    let (_, gone) = sheets.remove_sheet(&id("s1")).unwrap();
    assert_eq!(gone.len(), 2);
    assert!(sheets.viewport(&id("p1")).is_none());
    assert_eq!(
        sheets.remove_sheet(&id("s1")).unwrap_err(),
        SheetError::UnknownSheet(id("s1"))
    );
}

#[test]
fn the_standard_sheet_parameters_are_defined_and_bound_to_sheets_once() {
    let categories = CategoryTable::new();
    let table = with_standard(&ProjectParams::new(), &categories).unwrap();
    assert_eq!(table.len(), STANDARD.len());
    for (key, name) in STANDARD {
        let definition = table.definition(&id(key)).unwrap();
        assert_eq!(definition.name(), name);
        let (_, bound) = table.binding(&id(key)).unwrap();
        assert_eq!(bound, [SHEETS]);
    }
    let names: Vec<&str> = STANDARD.iter().map(|(_, name)| *name).collect();
    for needed in [
        "Drawn by",
        "Checked by",
        "Sheet issue date",
        "Current revision",
    ] {
        assert!(names.contains(&needed), "{needed}");
    }
    // Again: nothing changes, so a command writes nothing.
    assert_eq!(with_standard(&table, &categories).unwrap(), table);
    // Another project parameter alongside is left alone.
    let own = construct_params::Definition::new(
        id("mine"),
        "Mine",
        construct_params::Kind::Text,
        construct_params::Holder::Instance,
    )
    .unwrap();
    let bigger = table.add_project_parameter(own).unwrap();
    assert_eq!(with_standard(&bigger, &categories).unwrap(), bigger);
}
