//! Where viewports lie on the paper: outlines from extent and scale, alignments worked out and
//! surviving a change of extent, exact to the grid step.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::Length;
use construct_sheets::{
    Alignment, Anchor, Orientation, Outline, Paper, PaperSize, Sheet, SheetError, Sheets,
    ViewFacts, Viewport,
};
use construct_styles::Scale;
use kit_core::Id;
use std::collections::BTreeMap;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn facts(scale: u32, width: Length, height: Length) -> ViewFacts {
    ViewFacts {
        scale: Scale::new(scale).unwrap(),
        extent: Some((width, height)),
    }
}

fn outline(min_x: Length, min_y: Length, max_x: Length, max_y: Length) -> Outline {
    Outline {
        min_x,
        min_y,
        max_x,
        max_y,
    }
}

/// An A1 landscape sheet (841 by 594 mm) with two viewports.
fn two() -> Sheets {
    let paper = Paper::Standard {
        size: PaperSize::A1,
        orientation: Orientation::Landscape,
    };
    let mut sheets = Sheets::new();
    sheets
        .add_sheet(Sheet::new(id("s1"), "A101", "Plan", paper).unwrap())
        .unwrap();
    sheets
        .add_viewport(Viewport::new(
            id("p1"),
            id("s1"),
            id("v1"),
            mm(300),
            mm(200),
        ))
        .unwrap();
    sheets
        .add_viewport(Viewport::new(
            id("p2"),
            id("s1"),
            id("v2"),
            mm(600),
            mm(400),
        ))
        .unwrap();
    sheets
}

#[test]
fn an_outline_is_the_views_extent_over_its_scale_about_the_centre() {
    let sheets = two();
    let known: BTreeMap<Id, ViewFacts> = [
        // 20 m by 10 m at 1:100 is 200 mm by 100 mm on the paper.
        (id("v1"), facts(100, mm(20_000), mm(10_000))),
        // 12.5 m by 8 m at 1:50 is 250 mm by 160 mm.
        (id("v2"), facts(50, mm(12_500), mm(8_000))),
    ]
    .into();
    let placed = sheets
        .layout(&id("s1"), |view| known.get(view).copied())
        .unwrap();
    assert_eq!(placed.len(), 2);
    assert_eq!(placed[0].viewport, id("p1"));
    assert_eq!((placed[0].x, placed[0].y), (mm(300), mm(200)));
    assert_eq!(
        placed[0].outline,
        Some(outline(mm(200), mm(150), mm(400), mm(250)))
    );
    assert_eq!(
        placed[1].outline,
        Some(outline(mm(475), mm(320), mm(725), mm(480)))
    );
    assert_eq!(placed[1].scale, Scale::new(50).unwrap());
    assert!(placed[0].title && placed[0].on_paper && placed[1].on_paper);
}

#[test]
fn a_half_width_is_rounded_up_to_a_whole_grid_step_never_down() {
    let sheets = two();
    // One step of extent at 1:100: half a step over 100 is a fraction; the outline takes a step.
    let tiny = ViewFacts {
        scale: Scale::new(100).unwrap(),
        extent: Some((Length::from_steps(1), Length::from_steps(401))),
    };
    let placed = sheets.layout(&id("s1"), |_| Some(tiny)).unwrap();
    let box1 = placed[0].outline.unwrap();
    assert_eq!(box1.max_x.steps() - box1.min_x.steps(), 2);
    // 401 steps over 200 is 2.005, so 3 steps each way.
    assert_eq!(box1.max_y.steps() - box1.min_y.steps(), 6);
    // Exactly divisible extents are not widened: 200 steps at 1:100 is one step each way.
    let exact = ViewFacts {
        scale: Scale::new(100).unwrap(),
        extent: Some((Length::from_steps(200), Length::from_steps(0))),
    };
    let placed = sheets.layout(&id("s1"), |_| Some(exact)).unwrap();
    let outline = placed[0].outline.unwrap();
    assert_eq!(outline.max_x.steps() - outline.min_x.steps(), 2);
    assert_eq!(outline.max_y, outline.min_y);
}

#[test]
fn an_outline_off_the_paper_is_said_so_and_a_view_with_no_extent_is_a_point() {
    let sheets = two();
    let big = facts(10, mm(10_000), mm(1_000));
    let none = ViewFacts {
        scale: Scale::new(100).unwrap(),
        extent: None,
    };
    let placed = sheets
        .layout(&id("s1"), |view| {
            Some(if view == &id("v1") { big } else { none })
        })
        .unwrap();
    // 1000 mm wide about 300: from -200 to 800; the paper starts at 0.
    assert_eq!(placed[0].outline.unwrap().min_x, mm(-200));
    assert!(!placed[0].on_paper);
    assert_eq!(placed[1].outline, None);
    assert!(placed[1].on_paper);
}

#[test]
fn an_alignment_holds_when_the_views_extent_changes() {
    let mut sheets = two();
    // p2 follows p1's left edge across and its top edge up.
    let p2 = sheets
        .viewport(&id("p2"))
        .unwrap()
        .clone()
        .with_align_x(Some(Alignment {
            to: id("p1"),
            anchor: Anchor::Min,
        }))
        .with_align_y(Some(Alignment {
            to: id("p1"),
            anchor: Anchor::Max,
        }));
    sheets.replace_viewport(p2).unwrap();
    let layout = |sheets: &Sheets, first: ViewFacts| {
        let second = facts(50, mm(12_500), mm(8_000));
        sheets
            .layout(&id("s1"), |view| {
                Some(if view == &id("v1") { first } else { second })
            })
            .unwrap()
    };
    let before = layout(&sheets, facts(100, mm(20_000), mm(10_000)));
    let (a, b) = (&before[0], &before[1]);
    // The left edges are level, and so are the top edges, to the step.
    assert_eq!(b.outline.unwrap().min_x, a.outline.unwrap().min_x);
    assert_eq!(b.outline.unwrap().max_y, a.outline.unwrap().max_y);
    assert_eq!((b.x, b.y), (mm(325), mm(170)));
    // The first view grows to 40 m by 20 m: its outline grows about its centre, and the second
    // follows with nothing written.
    let after = layout(&sheets, facts(100, mm(40_000), mm(20_000)));
    let (a, b) = (&after[0], &after[1]);
    assert_eq!(a.outline, Some(outline(mm(100), mm(100), mm(500), mm(300))));
    assert_eq!(b.outline.unwrap().min_x, mm(100));
    assert_eq!(b.outline.unwrap().max_y, mm(300));
    assert_eq!((b.x, b.y), (mm(225), mm(220)));
    // The view's scale changing moves it the same way.
    let rescaled = layout(&sheets, facts(200, mm(40_000), mm(20_000)));
    assert_eq!(
        rescaled[0].outline,
        Some(outline(mm(200), mm(150), mm(400), mm(250)))
    );
    assert_eq!(rescaled[1].outline.unwrap().min_x, mm(200));
}

#[test]
fn centres_align_and_chains_follow_through() {
    let mut sheets = two();
    sheets
        .add_viewport(Viewport::new(id("p3"), id("s1"), id("v3"), mm(50), mm(50)))
        .unwrap();
    let base = sheets.clone();
    let chain = |one: &str, to: &str, anchor| {
        base.viewport(&id(one))
            .unwrap()
            .clone()
            .with_align_x(Some(Alignment { to: id(to), anchor }))
    };
    let p2 = chain("p2", "p1", Anchor::Centre);
    sheets.replace_viewport(p2).unwrap();
    let p3 = chain("p3", "p2", Anchor::Max);
    sheets.replace_viewport(p3).unwrap();
    let known = |view: &Id| {
        Some(match view.as_str() {
            "v1" => facts(100, mm(20_000), mm(10_000)),
            "v2" => facts(100, mm(40_000), mm(10_000)),
            _ => facts(100, mm(10_000), mm(10_000)),
        })
    };
    let placed = sheets.layout(&id("s1"), known).unwrap();
    // p2 is centred on p1 (300); p3's right edge is p2's right edge (300 + 200 = 500), so its
    // centre is 500 - 50 = 450.
    assert_eq!(placed[1].x, mm(300));
    assert_eq!(placed[2].x, mm(450));
    // Up the paper nothing is aligned: the centres are where they were put.
    assert_eq!(placed[2].y, mm(50));
}

#[test]
fn a_layout_names_what_it_cannot_work_out() {
    let sheets = two();
    assert_eq!(
        sheets.layout(&id("nope"), |_| None).unwrap_err(),
        SheetError::UnknownSheet(id("nope"))
    );
    assert_eq!(
        sheets.layout(&id("s1"), |_| None).unwrap_err(),
        SheetError::UnknownView(id("v1"))
    );
    let negative = facts(100, Length::from_steps(-1), mm(1));
    let error = sheets.layout(&id("s1"), |_| Some(negative)).unwrap_err();
    assert!(matches!(error, SheetError::BadExtent(_)), "{error}");
    let huge = facts(1, Length::from_steps(i64::MAX), mm(1));
    // Half of the largest length still fits, and is far off the paper.
    let placed = sheets.layout(&id("s1"), |_| Some(huge)).unwrap();
    assert!(!placed[0].on_paper);
}
