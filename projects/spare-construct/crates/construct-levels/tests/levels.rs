//! A document's levels: names, elevations, storeys, their order, the levels an old record's
//! integer implies, how an element is tied to a level, and the one function that says how high
//! something tied to a level sits.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_levels::{
    Level, LevelError, Levels, Tie, base_of, height_of, implied, implied_id, implied_number,
    parse_length, show, span, tied_to, top_of, write_base, write_top,
};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Units};
use kit_core::Id;
use std::collections::BTreeMap;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn level(key: &str, name: &str, elevation: i32, storey: bool) -> Level {
    Level::new(id(key), name, mm(elevation), storey).unwrap()
}

/// A wall record on the old integer level `n`, with no tie.
fn old_wall(key: &str, n: i32) -> Element {
    Element {
        id: id(key),
        document: id("doc"),
        kind: "wall".to_owned(),
        name: key.to_owned(),
        level: n,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct("wall.straight", BTreeMap::new()),
        props: BTreeMap::new(),
        references: Vec::new(),
    }
}

fn tied_wall(key: &str, to: &str, offset: i32) -> Element {
    let mut wall = old_wall(key, 0);
    write_base(
        &mut wall.props,
        Some(&Tie {
            level: id(to),
            offset: mm(offset),
        }),
    );
    wall
}

fn two() -> Levels {
    let mut levels = Levels::new();
    levels.add(level("L-g", "Ground", 0, true)).unwrap();
    levels.add(level("L-1", "First", 3200, true)).unwrap();
    levels
}

#[test]
fn a_level_has_an_id_a_trimmed_name_an_elevation_and_a_storey_flag() {
    let basement = Level::new(id("L-b"), "  Basement ", mm(-2800), true).unwrap();
    assert_eq!(basement.id(), &id("L-b"));
    assert_eq!(basement.name(), "Basement");
    assert_eq!(basement.elevation(), mm(-2800));
    assert!(basement.is_storey());
    assert_eq!(
        Level::new(id("L-x"), "   ", mm(0), false),
        Err(LevelError::EmptyName)
    );
}

#[test]
fn names_are_unique_in_the_document_without_regard_to_case() {
    let mut levels = two();
    let clash = levels.add(level("L-2", "GROUND", 6400, true));
    assert_eq!(clash, Err(LevelError::DuplicateName("GROUND".to_owned())));
    assert_eq!(
        levels.rename(&id("L-1"), "ground").unwrap_err(),
        LevelError::DuplicateName("ground".to_owned())
    );
    // A level may change the case of its own name.
    assert_eq!(
        levels.rename(&id("L-g"), "GROUND").unwrap().name(),
        "GROUND"
    );
    assert_eq!(
        levels.add(level("L-g", "Other", 9000, true)),
        Err(LevelError::DuplicateLevel(id("L-g")))
    );
    assert_eq!(
        levels.rename(&id("L-9"), "Roof").unwrap_err(),
        LevelError::UnknownLevel(id("L-9"))
    );
}

#[test]
fn levels_are_ordered_by_elevation_then_name_then_id() {
    let mut levels = two();
    levels.add(level("L-r", "Roof", 6400, false)).unwrap();
    levels.add(level("L-s", "Site", -450, false)).unwrap();
    // Same elevation as Ground: allowed, as it is not a storey; ordered by name, case aside.
    levels.add(level("L-a", "a datum", 0, false)).unwrap();
    let names: Vec<&str> = levels.ordered().iter().map(|l| l.name()).collect();
    assert_eq!(names, ["Site", "a datum", "Ground", "First", "Roof"]);
}

/// The rule for two levels at one elevation: allowed, but at most one of them is a building
/// storey. A storey is a slice of the building, so the storey above another must be one level.
#[test]
fn two_levels_may_share_an_elevation_but_only_one_of_them_a_storey() {
    let mut levels = two();
    levels
        .add(level("L-t", "Top of slab", 3200, false))
        .unwrap();
    let second = levels.add(level("L-m", "Mezzanine", 3200, true));
    assert_eq!(
        second,
        Err(LevelError::StoreyAtElevation {
            name: "Mezzanine".to_owned(),
            other: "First".to_owned(),
            elevation: mm(3200),
        })
    );
    // Making the datum a storey, or moving a storey onto another's height, is the same refusal.
    assert!(matches!(
        levels.set_storey(&id("L-t"), true),
        Err(LevelError::StoreyAtElevation { .. })
    ));
    assert!(matches!(
        levels.set_elevation(&id("L-g"), mm(3200)),
        Err(LevelError::StoreyAtElevation { .. })
    ));
    // A storey may move to the datum's height; the datum stays where it was.
    levels.set_elevation(&id("L-g"), mm(-150)).unwrap();
    levels.set_elevation(&id("L-t"), mm(-150)).unwrap();
    assert_eq!(levels.get(&id("L-t")).unwrap().elevation(), mm(-150));
    let message = LevelError::StoreyAtElevation {
        name: "Mezzanine".to_owned(),
        other: "First".to_owned(),
        elevation: mm(3200),
    }
    .to_string();
    assert_eq!(
        message,
        "Mezzanine can't be a storey at 3200 mm: First is already the storey at that elevation."
    );
}

/// How an old record's integer maps to a level element, pinned. 0 is no level; n is the level
/// whose ID is n's decimal text (`m` and the digits below zero), named `Level n`, a storey, at
/// (n - 1) x 3000 mm above zero, or n x 3000 mm below it.
#[test]
fn an_old_integer_names_its_implied_level_by_a_fixed_rule() {
    assert_eq!(implied_id(0), None);
    assert_eq!(implied_id(1), Some(id("1")));
    assert_eq!(implied_id(12), Some(id("12")));
    assert_eq!(implied_id(-1), Some(id("m1")));
    assert_eq!(implied_id(i32::MIN), Some(id("m2147483648")));
    for (n, elevation) in [(1, 0), (2, 3000), (3, 6000), (-1, -3000), (-2, -6000)] {
        let level = implied(n).unwrap();
        assert_eq!(level.id(), &implied_id(n).unwrap());
        assert_eq!(level.name(), format!("Level {n}"));
        assert_eq!(level.elevation(), mm(elevation), "{n}");
        assert!(level.is_storey());
        assert_eq!(implied_number(level.id()), Some(n));
    }
    assert_eq!(implied(0), None);
    // The extremes still fit a length.
    assert!(implied(i32::MAX).is_some() && implied(i32::MIN).is_some());
    // Only the one spelling is an implied ID.
    for not in ["0", "01", "m0", "m01", "-1", "L1", "1.5", "m", "2147483648"] {
        assert_eq!(implied_number(&id_or_skip(not)), None, "{not}");
    }
}

/// Any text, made an ID where it is one; `x` otherwise (no implied ID starts with `x`).
fn id_or_skip(text: &str) -> Id {
    Id::parse(text).unwrap_or_else(|_| id("x"))
}

#[test]
fn a_document_lists_its_stored_levels_and_the_implied_ones_in_use() {
    let ground = level("1", "Ground", 0, true).to_record(&id("doc"));
    let records = [
        ground,
        old_wall("W-1", 1),
        old_wall("W-2", 2),
        old_wall("W-3", 0),
        tied_wall("W-4", "m1", 0),
    ];
    let levels = Levels::from_records(&records).unwrap();
    let listed: Vec<(&str, &str, Length, bool)> = levels
        .ordered()
        .iter()
        .map(|l| {
            (
                l.id().as_str(),
                l.name(),
                l.elevation(),
                levels.is_implied(l.id()),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            ("m1", "Level -1", mm(-3000), true),
            // The record of ID 1 is the level the old integer 1 names, as it is now.
            ("1", "Ground", mm(0), false),
            ("2", "Level 2", mm(3000), true),
        ]
    );
    // The same records in any order give the same levels.
    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(Levels::from_records(&reversed).unwrap(), levels);
}

#[test]
fn an_element_is_tied_by_its_tie_or_else_by_its_old_integer() {
    let old = old_wall("W-1", 2);
    assert_eq!(
        base_of(&old).unwrap(),
        Some(Tie {
            level: id("2"),
            offset: mm(0)
        })
    );
    assert_eq!(base_of(&old_wall("W-0", 0)).unwrap(), None);
    // A tie wins over the integer.
    let mut tied = tied_wall("W-2", "L-1", 150);
    tied.level = 7;
    assert_eq!(
        base_of(&tied).unwrap(),
        Some(Tie {
            level: id("L-1"),
            offset: mm(150)
        })
    );
    assert_eq!(
        tied.props.get("@level.offset").map(String::as_str),
        Some("length:q0:960000")
    );
    // A tie's two keys go together: half a tie is refused.
    let mut half = tied_wall("W-3", "L-1", 0);
    half.props.remove("@level.offset");
    assert!(matches!(base_of(&half), Err(LevelError::Malformed(_))));
    // Untying removes both keys.
    write_base(&mut tied.props, None);
    assert!(tied.props.is_empty());
    // The top is the same, under its own keys, and never read from the integer.
    assert_eq!(top_of(&old).unwrap(), None);
    let top = Tie {
        level: id("L-r"),
        offset: mm(-300),
    };
    write_top(&mut tied.props, Some(&top));
    assert_eq!(top_of(&tied).unwrap(), Some(top));
    assert_eq!(
        tied.props.keys().collect::<Vec<_>>(),
        ["@top.level", "@top.offset"]
    );
}

#[test]
fn how_high_a_tied_thing_sits_is_one_function() {
    let first = level("L-1", "First", 3200, true);
    assert_eq!(height_of(&first, mm(0)).unwrap(), mm(3200));
    assert_eq!(height_of(&first, mm(-150)).unwrap(), mm(3050));
    let deep = Level::new(id("L-d"), "Deep", Length::from_steps(i64::MIN), false).unwrap();
    assert_eq!(
        height_of(&deep, mm(-1)),
        Err(LevelError::OutOfRange("Deep".to_owned()))
    );
    // A wall from Ground + 0 to First - 300 is 2900 mm high, and follows First when it moves.
    let ground = level("L-g", "Ground", 0, true);
    assert_eq!(
        span((&ground, mm(0)), (&first, mm(-300))).unwrap(),
        mm(2900)
    );
    let moved = Level::new(id("L-1"), "First", mm(3600), true).unwrap();
    assert_eq!(
        span((&ground, mm(0)), (&moved, mm(-300))).unwrap(),
        mm(3300)
    );
}

#[test]
fn a_level_is_not_deleted_while_anything_is_tied_to_it() {
    let records = [
        old_wall("W-1", 1),
        tied_wall("W-2", "1", 0),
        tied_wall("W-3", "L-r", 0),
        old_wall("W-4", 2),
    ];
    assert_eq!(tied_to(&id("1"), &records).unwrap(), 2);
    let mut topped = old_wall("W-5", 1);
    write_top(
        &mut topped.props,
        Some(&Tie {
            level: id("1"),
            offset: mm(0),
        }),
    );
    // Base and top on one level count once.
    assert_eq!(tied_to(&id("1"), [&topped]).unwrap(), 1);
    let mut levels = Levels::from_records(&records).unwrap();
    let refused = levels.remove(&id("1"), 2).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "Level 1 can't be deleted: 2 elements are tied to it."
    );
    assert_eq!(
        levels.remove(&id("2"), 1).unwrap_err().to_string(),
        "Level 2 can't be deleted: 1 element is tied to it."
    );
    let mut alone = two();
    assert_eq!(alone.remove(&id("L-1"), 0).unwrap().name(), "First");
    assert!(alone.get(&id("L-1")).is_none());
}

#[test]
fn an_implied_level_changed_is_a_stored_one() {
    let records = [old_wall("W-1", 2)];
    let mut levels = Levels::from_records(&records).unwrap();
    assert!(levels.is_implied(&id("2")));
    let renamed = levels.rename(&id("2"), "First floor").unwrap().clone();
    assert_eq!(renamed.elevation(), mm(3000));
    assert!(!levels.is_implied(&id("2")));
    // Any implied ID resolves, in use or not; an ID that is neither stored nor implied does not.
    assert_eq!(levels.resolve(&id("5")).unwrap().name(), "Level 5");
    assert_eq!(levels.resolve(&id("L-x")), None);
}

#[test]
fn elevations_are_typed_and_shown_in_the_project_units() {
    let units = Units::default();
    assert_eq!(parse_length("3.2 m", &units).unwrap(), mm(3200));
    assert_eq!(parse_length("-450", &units).unwrap(), mm(-450));
    assert_eq!(
        parse_length("12.5 mm", &units).unwrap(),
        Length::from_steps(80_000)
    );
    assert!(matches!(
        parse_length("3 m2", &units),
        Err(LevelError::Typed(_))
    ));
    assert_eq!(show(mm(3200), &units), "3200 mm");
    assert_eq!(show(mm(-450), &units), "-450 mm");
}
