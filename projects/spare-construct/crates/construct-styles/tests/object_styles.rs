//! Object styles: the inheritance rule, the refusals, the lookup at several scales, the pure
//! changes and the stored form. Written against this crate's and the category crate's own types.
#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_categories::{
    CategoryId, CategoryTable, DOORS, FILLED_REGIONS, FURNITURE, TEXT_NOTES, WALLS,
};
use construct_styles::patterns::{DASHED, PatternId, SOLID};
use construct_styles::{
    Colour, Edge, LineStyles, ObjectStyle, ObjectStyleError, ObjectStyles, Scale,
};

const DOOR_SWING: CategoryId = CategoryId(304);
const DOOR_PANEL: CategoryId = CategoryId(301);
const WALL_FINISH: CategoryId = CategoryId(102);

fn scale(denominator: u32) -> Scale {
    Scale::new(denominator).unwrap()
}

fn lines() -> LineStyles {
    LineStyles::default_metric()
}

fn width(
    styles: &ObjectStyles,
    cats: &CategoryTable,
    id: CategoryId,
    edge: Edge,
    denominator: u32,
) -> u32 {
    styles
        .lookup(cats, &lines(), id, edge, scale(denominator))
        .unwrap()
        .width
        .get()
}

fn style(projection: u16, cut: Option<u16>) -> ObjectStyle {
    ObjectStyle {
        projection_pen: projection,
        cut_pen: cut,
        colour: Colour(10, 20, 30),
        pattern: DASHED,
    }
}

// ---- the defaults ----

#[test]
fn every_built_in_category_has_a_default_with_a_cut_pen_only_if_it_can_be_cut() {
    let cats = CategoryTable::new();
    let styles = ObjectStyles::default_metric();
    for built in construct_categories::BUILT_IN {
        let own = styles
            .own_style(built.id)
            .unwrap_or_else(|| panic!("no default for {}", built.name));
        assert_eq!(
            own.cut_pen.is_some(),
            built.cuttable,
            "cut pen of {} ({})",
            built.name,
            built.id.0
        );
        assert_eq!(styles.style_of(&cats, built.id).unwrap(), own);
    }
    assert_eq!(styles.entries().len(), construct_categories::BUILT_IN.len());
}

#[test]
fn the_defaults_meet_every_rule_against_the_default_lines() {
    let bytes = ObjectStyles::default_metric().to_bytes().unwrap();
    let read = ObjectStyles::from_bytes(&bytes, &CategoryTable::new(), &lines()).unwrap();
    assert_eq!(read, ObjectStyles::default_metric());
}

#[test]
fn pinned_default_rows() {
    let styles = ObjectStyles::default_metric();
    let black = Colour(0, 0, 0);
    let pinned: [(u32, u16, Option<u16>, Colour, PatternId); 12] = [
        (100, 4, Some(6), black, SOLID),
        (102, 2, Some(3), black, SOLID),
        (300, 3, Some(5), black, SOLID),
        (304, 1, None, black, SOLID),
        (405, 1, None, black, DASHED),
        (1100, 4, Some(6), black, SOLID),
        (1700, 2, Some(4), black, SOLID),
        (2200, 1, Some(2), black, SOLID),
        (3100, 3, None, Colour(170, 30, 30), PatternId(4)),
        (5200, 2, None, black, SOLID),
        (6000, 1, None, black, SOLID),
        (9100, 2, Some(3), Colour(110, 110, 110), SOLID),
    ];
    for (id, projection, cut, colour, pattern) in pinned {
        assert_eq!(
            styles.own_style(CategoryId(id)).unwrap(),
            ObjectStyle {
                projection_pen: projection,
                cut_pen: cut,
                colour,
                pattern
            },
            "category {id}"
        );
    }
}

#[test]
fn the_default_bytes_start_as_pinned() {
    let text = String::from_utf8(ObjectStyles::default_metric().to_bytes().unwrap()).unwrap();
    let start = r#"{"format":1,"entries":[{"category":100,"style":{"projection_pen":4,"cut_pen":6,"colour":[0,0,0],"pattern":1}},{"category":101,"#;
    assert!(text.starts_with(start), "{text}");
    assert!(text.contains(r#"{"category":304,"style":{"projection_pen":1,"cut_pen":null,"#));
}

#[test]
fn the_defaults_follow_the_weighting_conventions() {
    let styles = ObjectStyles::default_metric();
    let cats = CategoryTable::new();
    for built in construct_categories::BUILT_IN {
        let own = styles.own_style(built.id).unwrap();
        if let Some(cut) = own.cut_pen {
            assert!(
                cut >= own.projection_pen,
                "{} is cut lighter than it is seen",
                built.name
            );
        }
    }
    // Heavier where a view cuts through structure, lighter for fittings, lightest for hatching.
    let cut = |id| styles.style_of(&cats, id).unwrap().cut_pen.unwrap();
    assert!(cut(WALLS) > cut(DOORS));
    assert!(cut(DOORS) > cut(FURNITURE));
    assert!(cut(WALLS) > cut(WALL_FINISH));
    let hatching = styles.style_of(&cats, FILLED_REGIONS).unwrap();
    assert_eq!(hatching.projection_pen, 1);
    for built in construct_categories::BUILT_IN {
        let own = styles.own_style(built.id).unwrap();
        assert!(own.projection_pen >= hatching.projection_pen);
    }
}

// ---- the inheritance rule ----

#[test]
fn a_user_subcategory_with_no_entry_takes_its_parents_values() {
    let cats = CategoryTable::new();
    let (cats, mine) = cats.add_subcategory(DOORS, "Hardware").unwrap();
    let styles = ObjectStyles::default_metric();
    assert_eq!(styles.own_style(mine), None);
    assert_eq!(
        styles.style_of(&cats, mine).unwrap(),
        styles.style_of(&cats, DOORS).unwrap()
    );
    for denominator in [20, 100] {
        for edge in [Edge::Projected, Edge::Cut] {
            let own = styles
                .lookup(&cats, &lines(), mine, edge, scale(denominator))
                .unwrap();
            let parent = styles
                .lookup(&cats, &lines(), DOORS, edge, scale(denominator))
                .unwrap();
            assert_eq!(own, parent);
        }
    }
}

#[test]
fn a_subcategory_that_cannot_be_cut_inherits_no_cut_pen() {
    let cats = CategoryTable::new();
    let styles = ObjectStyles::default_metric()
        .without_style(&cats, DOOR_SWING)
        .unwrap();
    assert_eq!(styles.own_style(DOOR_SWING), None);
    let door = styles.style_of(&cats, DOORS).unwrap();
    let swing = styles.style_of(&cats, DOOR_SWING).unwrap();
    assert_eq!(swing.cut_pen, None);
    assert_eq!(swing.projection_pen, door.projection_pen);
    assert_eq!(swing.colour, door.colour);
    assert_eq!(swing.pattern, door.pattern);
    assert_eq!(
        styles.lookup(&cats, &lines(), DOOR_SWING, Edge::Cut, scale(50)),
        Err(ObjectStyleError::NotCuttable(DOOR_SWING))
    );
}

#[test]
fn removing_a_built_in_subcategorys_style_makes_it_follow_the_parent() {
    let cats = CategoryTable::new();
    let styles = ObjectStyles::default_metric()
        .without_style(&cats, DOOR_PANEL)
        .unwrap();
    assert_eq!(
        styles.style_of(&cats, DOOR_PANEL).unwrap(),
        styles.style_of(&cats, DOORS).unwrap()
    );
    // Taking away what is not there changes nothing.
    assert_eq!(styles.without_style(&cats, DOOR_PANEL).unwrap(), styles);
}

#[test]
fn a_user_subcategory_can_be_given_its_own_style() {
    let cats = CategoryTable::new();
    let (cats, mine) = cats.add_subcategory(DOORS, "Hardware").unwrap();
    let base = ObjectStyles::default_metric();
    let given = base
        .with_style(&cats, &lines(), mine, style(2, Some(4)))
        .unwrap();
    assert_eq!(given.own_style(mine), Some(style(2, Some(4))));
    assert_eq!(given.style_of(&cats, mine).unwrap(), style(2, Some(4)));
    // The parent and the old table are untouched.
    assert_eq!(
        given.style_of(&cats, DOORS).unwrap(),
        base.style_of(&cats, DOORS).unwrap()
    );
    assert_eq!(base.own_style(mine), None);
    // And it can be taken away again.
    assert_eq!(given.without_style(&cats, mine).unwrap(), base);
    // Giving it again replaces, not adds.
    let again = given
        .with_style(&cats, &lines(), mine, style(1, Some(3)))
        .unwrap();
    assert_eq!(again.entries().len(), given.entries().len());
}

// ---- the refusals ----

#[test]
fn a_cut_pen_on_a_category_that_cannot_be_cut_is_refused() {
    let cats = CategoryTable::new();
    let base = ObjectStyles::default_metric();
    assert_eq!(
        base.with_style(&cats, &lines(), DOOR_SWING, style(1, Some(3))),
        Err(ObjectStyleError::CutPenOnUncuttable(DOOR_SWING))
    );
    assert_eq!(
        base.with_style(&cats, &lines(), TEXT_NOTES, style(2, Some(3))),
        Err(ObjectStyleError::CutPenOnUncuttable(TEXT_NOTES))
    );
}

#[test]
fn a_category_that_can_be_cut_needs_a_cut_pen() {
    let cats = CategoryTable::new();
    assert_eq!(
        ObjectStyles::default_metric().with_style(&cats, &lines(), WALLS, style(4, None)),
        Err(ObjectStyleError::CutPenRequired(WALLS))
    );
}

#[test]
fn a_user_subcategory_can_be_cut_exactly_when_its_parent_can() {
    let (cats, hardware) = CategoryTable::new()
        .add_subcategory(DOORS, "Hardware")
        .unwrap();
    let (cats, trim) = cats.add_subcategory(TEXT_NOTES, "Trim").unwrap();
    let base = ObjectStyles::default_metric();
    assert!(
        base.with_style(&cats, &lines(), hardware, style(2, Some(4)))
            .is_ok()
    );
    assert_eq!(
        base.with_style(&cats, &lines(), trim, style(2, Some(4))),
        Err(ObjectStyleError::CutPenOnUncuttable(trim))
    );
    assert!(
        base.with_style(&cats, &lines(), trim, style(2, None))
            .is_ok()
    );
}

#[test]
fn an_unknown_pen_pattern_or_category_is_refused() {
    let cats = CategoryTable::new();
    let base = ObjectStyles::default_metric();
    assert_eq!(
        base.with_style(&cats, &lines(), WALLS, style(99, Some(6))),
        Err(ObjectStyleError::NoSuchPen { pen: 99 })
    );
    assert_eq!(
        base.with_style(&cats, &lines(), WALLS, style(4, Some(0))),
        Err(ObjectStyleError::NoSuchPen { pen: 0 })
    );
    let mut odd = style(4, Some(6));
    odd.pattern = PatternId(77);
    assert_eq!(
        base.with_style(&cats, &lines(), WALLS, odd),
        Err(ObjectStyleError::NoSuchPattern { id: 77 })
    );
    let nowhere = CategoryId(424_242);
    assert_eq!(
        base.with_style(&cats, &lines(), nowhere, style(1, None)),
        Err(ObjectStyleError::UnknownCategory(nowhere))
    );
    assert_eq!(
        base.style_of(&cats, nowhere),
        Err(ObjectStyleError::UnknownCategory(nowhere))
    );
    assert_eq!(
        base.lookup(&cats, &lines(), nowhere, Edge::Projected, scale(50)),
        Err(ObjectStyleError::UnknownCategory(nowhere))
    );
    assert_eq!(
        base.without_style(&cats, nowhere),
        Err(ObjectStyleError::UnknownCategory(nowhere))
    );
}

#[test]
fn a_top_level_category_keeps_its_style() {
    assert_eq!(
        ObjectStyles::default_metric().without_style(&CategoryTable::new(), WALLS),
        Err(ObjectStyleError::TopLevelNeedsStyle(WALLS))
    );
}

#[test]
fn a_refused_change_leaves_the_table_as_it_was() {
    let cats = CategoryTable::new();
    let base = ObjectStyles::default_metric();
    let before = base.to_bytes().unwrap();
    let _ = base.with_style(&cats, &lines(), WALLS, style(99, Some(6)));
    assert_eq!(base.to_bytes().unwrap(), before);
}

// ---- the lookup ----

#[test]
fn the_cut_wall_prints_thinner_on_a_smaller_scale_drawing() {
    let cats = CategoryTable::new();
    let styles = ObjectStyles::default_metric();
    let cut = |d| width(&styles, &cats, WALLS, Edge::Cut, d);
    assert_eq!(cut(1), 700);
    assert_eq!(cut(20), 700);
    assert_eq!(cut(50), 500);
    // 1:75 is between listed scales and takes the coarser row, 1:100.
    assert_eq!(cut(75), 500);
    assert_eq!(cut(100), 500);
    assert_eq!(cut(200), 350);
    assert_eq!(cut(500), 250);
    assert_eq!(cut(1000), 180);
    // Past the last listed scale, the last row.
    assert_eq!(cut(5000), 180);
}

#[test]
fn the_projected_wall_is_lighter_than_the_cut_wall() {
    let cats = CategoryTable::new();
    let styles = ObjectStyles::default_metric();
    for denominator in [20, 50, 100, 200, 500] {
        let projected = width(&styles, &cats, WALLS, Edge::Projected, denominator);
        let cut = width(&styles, &cats, WALLS, Edge::Cut, denominator);
        assert!(projected < cut, "1:{denominator}");
    }
    assert_eq!(width(&styles, &cats, WALLS, Edge::Projected, 100), 250);
    assert_eq!(width(&styles, &cats, WALLS, Edge::Projected, 500), 130);
}

#[test]
fn the_lookup_returns_the_colour_and_pattern() {
    let cats = CategoryTable::new();
    let got = ObjectStyles::default_metric()
        .lookup(
            &cats,
            &lines(),
            CategoryId(405),
            Edge::Projected,
            scale(100),
        )
        .unwrap();
    assert_eq!(got.colour, Colour(0, 0, 0));
    assert_eq!(got.pattern, DASHED);
    assert_eq!(got.width.get(), 130);
}

#[test]
fn annotation_prints_at_one_width_at_every_scale() {
    let cats = CategoryTable::new();
    let styles = ObjectStyles::default_metric();
    for denominator in [1, 20, 100, 500, 1000] {
        assert_eq!(
            width(&styles, &cats, TEXT_NOTES, Edge::Projected, denominator),
            180
        );
    }
    assert_eq!(
        styles.lookup(&cats, &lines(), TEXT_NOTES, Edge::Cut, scale(100)),
        Err(ObjectStyleError::NotCuttable(TEXT_NOTES))
    );
}

#[test]
fn a_style_can_use_the_heaviest_pen() {
    let cats = CategoryTable::new();
    let heavy = ObjectStyles::default_metric()
        .with_style(&cats, &lines(), WALLS, style(9, Some(9)))
        .unwrap();
    assert_eq!(width(&heavy, &cats, WALLS, Edge::Cut, 1), 2000);
    assert_eq!(width(&heavy, &cats, WALLS, Edge::Projected, 1), 2000);
}

// ---- the stored form ----

fn changed() -> (CategoryTable, ObjectStyles) {
    let (cats, mine) = CategoryTable::new()
        .add_subcategory(DOORS, "Hardware")
        .unwrap();
    let styles = ObjectStyles::default_metric()
        .with_style(&cats, &lines(), mine, style(2, Some(4)))
        .unwrap()
        .with_style(&cats, &lines(), WALLS, style(3, Some(7)))
        .unwrap();
    (cats, styles)
}

#[test]
fn the_same_table_is_the_same_bytes_and_reads_back() {
    let (cats, styles) = changed();
    let a = styles.to_bytes().unwrap();
    assert_eq!(a, changed().1.to_bytes().unwrap());
    let read = ObjectStyles::from_bytes(&a, &cats, &lines()).unwrap();
    assert_eq!(read, styles);
    assert_eq!(read.to_bytes().unwrap(), a);
}

#[test]
fn a_colour_part_is_stored_exactly_and_above_255_is_refused() {
    let (cats, styles) = changed();
    let text = String::from_utf8(styles.to_bytes().unwrap()).unwrap();
    assert!(text.contains(r#""colour":[10,20,30]"#));
    let bad = text.replacen(r#""colour":[10,20,30]"#, r#""colour":[10,256,30]"#, 1);
    assert!(matches!(
        ObjectStyles::from_bytes(bad.as_bytes(), &cats, &lines()),
        Err(ObjectStyleError::Format(_))
    ));
    let negative = text.replacen(r#""colour":[10,20,30]"#, r#""colour":[10,-1,30]"#, 1);
    assert!(matches!(
        ObjectStyles::from_bytes(negative.as_bytes(), &cats, &lines()),
        Err(ObjectStyleError::Format(_))
    ));
}

#[test]
fn the_reader_refuses_what_the_writer_would_not_write() {
    let cats = CategoryTable::new();
    let text = String::from_utf8(ObjectStyles::default_metric().to_bytes().unwrap()).unwrap();
    let read = |t: &str| ObjectStyles::from_bytes(t.as_bytes(), &cats, &lines());
    assert!(read(&text).is_ok());
    // Another spelling of the same data.
    assert_eq!(
        read(&text.replacen("\"format\":1,", "\"format\":1, ", 1)),
        Err(ObjectStyleError::NotCanonical)
    );
    // Another version.
    assert_eq!(
        read(&text.replacen("\"format\":1", "\"format\":2", 1)),
        Err(ObjectStyleError::Version { found: 2 })
    );
    // A cut pen on a category that cannot be cut.
    let swing = text.replacen(
        r#"{"category":304,"style":{"projection_pen":1,"cut_pen":null"#,
        r#"{"category":304,"style":{"projection_pen":1,"cut_pen":3"#,
        1,
    );
    assert_eq!(
        read(&swing),
        Err(ObjectStyleError::CutPenOnUncuttable(DOOR_SWING))
    );
    // A pen that does not exist.
    let pen = text.replacen(
        r#"{"category":100,"style":{"projection_pen":4"#,
        r#"{"category":100,"style":{"projection_pen":40"#,
        1,
    );
    assert_eq!(read(&pen), Err(ObjectStyleError::NoSuchPen { pen: 40 }));
    // A category the project does not have.
    let unknown = text.replacen(r#""category":100,"#, r#""category":1000001,"#, 1);
    assert!(matches!(
        read(&unknown),
        Err(ObjectStyleError::UnknownCategory(_) | ObjectStyleError::EntriesNotAscending)
    ));
    // A built-in top-level category with no entry.
    let walls_at = text.find("{\"category\":100").unwrap();
    let next_at = text.find("{\"category\":101").unwrap();
    let without_walls = format!("{}{}", &text[..walls_at], &text[next_at..]);
    assert_eq!(
        read(&without_walls),
        Err(ObjectStyleError::MissingTopLevel(WALLS))
    );
    // Garbage and unknown fields.
    assert!(matches!(read("{}"), Err(ObjectStyleError::Format(_))));
    assert!(matches!(
        read(&text.replacen("\"entries\"", "\"extra\":1,\"entries\"", 1)),
        Err(ObjectStyleError::Format(_))
    ));
}

#[test]
fn entries_out_of_order_are_refused() {
    let cats = CategoryTable::new();
    let text = r#"{"format":1,"entries":[{"category":200,"style":{"projection_pen":3,"cut_pen":5,"colour":[0,0,0],"pattern":1}},{"category":100,"style":{"projection_pen":4,"cut_pen":6,"colour":[0,0,0],"pattern":1}}]}"#;
    assert_eq!(
        ObjectStyles::from_bytes(text.as_bytes(), &cats, &lines()),
        Err(ObjectStyleError::EntriesNotAscending)
    );
}

#[test]
fn a_stored_entry_for_a_user_subcategory_needs_that_subcategory() {
    let (cats, styles) = changed();
    let bytes = styles.to_bytes().unwrap();
    assert!(matches!(
        ObjectStyles::from_bytes(&bytes, &CategoryTable::new(), &lines()),
        Err(ObjectStyleError::UnknownCategory(_))
    ));
    assert!(ObjectStyles::from_bytes(&bytes, &cats, &lines()).is_ok());
}
