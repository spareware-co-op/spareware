//! The category tree and the project table: shape, rules and bytes.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_categories::{
    BUILT_IN, CategoryError, CategoryId, CategoryKind, CategoryTable, DOORS, FIRST_USER_ID,
    GENERIC_MODELS, WALLS, category_of_kind,
};
use std::collections::BTreeSet;

/// No two entries share an id, and every id is below the first project id.
#[test]
fn tree_has_no_duplicate_ids() {
    let mut seen = BTreeSet::new();
    for c in BUILT_IN {
        assert!(seen.insert(c.id), "duplicate id {}", c.id.0);
        assert!(c.id.0 < FIRST_USER_ID);
    }
}

/// Following parents always ends at a top-level category within one step, so there is no cycle,
/// and every parent named exists and comes before its children.
#[test]
fn tree_has_no_cycles_and_parents_exist() {
    let table = CategoryTable::new();
    for (index, c) in BUILT_IN.iter().enumerate() {
        let mut at = c.parent;
        let mut steps = 0;
        while let Some(p) = at {
            steps += 1;
            assert!(steps <= 1, "{} is deeper than a subcategory", c.name);
            let parent_index = BUILT_IN.iter().position(|x| x.id == p);
            assert!(
                parent_index.is_some_and(|i| i < index),
                "{} has a missing or later parent",
                c.name
            );
            at = table.get(p).and_then(|x| x.parent);
        }
    }
}

/// Names are unique within a parent, compared without regard to case.
#[test]
fn built_in_names_are_unique_within_a_parent() {
    let mut seen = BTreeSet::new();
    for c in BUILT_IN {
        assert!(
            seen.insert((c.parent, c.name.to_lowercase())),
            "{} repeats under its parent",
            c.name
        );
    }
}

/// The list covers what a building model holds, and the non-model kinds.
#[test]
fn covers_the_building_and_the_drawing() {
    let table = CategoryTable::new();
    let names: BTreeSet<String> = table
        .all()
        .into_iter()
        .map(|c| c.name.to_lowercase())
        .collect();
    for want in [
        "walls",
        "doors",
        "windows",
        "floors",
        "roofs",
        "ceilings",
        "stairs",
        "railings",
        "columns",
        "beams",
        "foundations",
        "rooms",
        "levels",
        "grids",
        "furniture",
        "plumbing fixtures",
        "generic models",
        "site",
        "topography",
        "views",
        "sheets",
        "dimensions",
        "tags",
        "text notes",
        "section markers",
        "elevation markers",
        "panel",
        "frame",
        "swing",
        "core cut line",
        "finish cut line",
    ] {
        assert!(names.contains(want), "no category named {want}");
    }
    let kind_of = |id| table.get(id).map(|c| c.kind);
    assert_eq!(kind_of(WALLS), Some(CategoryKind::Model));
    assert_eq!(
        kind_of(construct_categories::DIMENSIONS),
        Some(CategoryKind::Annotation)
    );
    assert_eq!(
        kind_of(construct_categories::VIEWS),
        Some(CategoryKind::View)
    );
    assert_eq!(
        kind_of(construct_categories::ANALYTICAL_MEMBERS),
        Some(CategoryKind::Analytical)
    );
}

/// A subcategory has the same kind as its parent.
#[test]
fn subcategories_share_their_parents_kind() {
    let table = CategoryTable::new();
    for c in table.all() {
        if let Some(p) = c.parent {
            assert_eq!(table.get(p).map(|x| x.kind), Some(c.kind), "{}", c.name);
        }
    }
}

/// A door's swing is not cut; its panel and frame are. Rooms and views are never cut.
#[test]
fn cut_flags_follow_the_drawing() {
    let table = CategoryTable::new();
    let cut = |name: &str, parent| {
        table
            .children(parent)
            .into_iter()
            .find(|c| c.name == name)
            .map(|c| c.cuttable)
    };
    assert_eq!(cut("Panel", DOORS), Some(true));
    assert_eq!(cut("Frame", DOORS), Some(true));
    assert_eq!(cut("Swing", DOORS), Some(false));
    assert_eq!(table.get(WALLS).map(|c| c.cuttable), Some(true));
    assert_eq!(
        table.get(construct_categories::ROOMS).map(|c| c.cuttable),
        Some(false)
    );
    assert_eq!(
        table.get(construct_categories::VIEWS).map(|c| c.cuttable),
        Some(false)
    );
}

/// A project adds a subcategory under a built-in category; the original table is not changed.
#[test]
fn a_project_adds_a_subcategory() {
    let before = CategoryTable::new();
    let (after, id) = before.add_subcategory(WALLS, "Fire rating line").unwrap();
    assert_eq!(id.0, FIRST_USER_ID);
    let got = after.get(id).unwrap();
    assert_eq!(got.name, "Fire rating line");
    assert_eq!(got.parent, Some(WALLS));
    assert_eq!(got.kind, CategoryKind::Model);
    assert!(got.cuttable);
    assert!(!got.built_in);
    assert!(before.get(id).is_none());
    assert!(after.children(WALLS).iter().any(|c| c.id == id));
    let (again, second) = after.add_subcategory(DOORS, "Kick plate").unwrap();
    assert_eq!(second.0, FIRST_USER_ID + 1);
    assert_eq!(again.all().len(), BUILT_IN.len() + 2);
}

/// Adding the same name twice under one parent is refused, whatever its case; the same name under
/// another parent is fine, and so is a built-in's name under a parent that has none like it.
#[test]
fn a_duplicate_name_is_refused() {
    let (one, _) = CategoryTable::new()
        .add_subcategory(WALLS, "Fire rating line")
        .unwrap();
    for name in [
        "Fire rating line",
        "FIRE RATING LINE",
        "  fire rating line ",
    ] {
        assert!(matches!(
            one.add_subcategory(WALLS, name),
            Err(CategoryError::DuplicateName { .. })
        ));
    }
    assert!(one.add_subcategory(DOORS, "Fire rating line").is_ok());
    assert!(matches!(
        CategoryTable::new().add_subcategory(DOORS, "panel"),
        Err(CategoryError::DuplicateName { .. })
    ));
}

/// Bad requests are error values.
#[test]
fn bad_subcategories_are_refused() {
    let t = CategoryTable::new();
    assert_eq!(
        t.add_subcategory(WALLS, "  "),
        Err(CategoryError::EmptyName)
    );
    assert_eq!(
        t.add_subcategory(CategoryId(7), "x"),
        Err(CategoryError::UnknownCategory(CategoryId(7)))
    );
    // Under a subcategory, and under a project subcategory, are not allowed.
    assert_eq!(
        t.add_subcategory(CategoryId(301), "x"),
        Err(CategoryError::ParentNotTopLevel(CategoryId(301)))
    );
    let (one, id) = t.add_subcategory(WALLS, "A").unwrap();
    assert_eq!(
        one.add_subcategory(id, "B"),
        Err(CategoryError::ParentNotTopLevel(id))
    );
}

/// Built-in categories are never changed by adding project ones.
#[test]
fn built_ins_survive_a_project_table() {
    let mut t = CategoryTable::new();
    for n in 0..20 {
        t = t.add_subcategory(WALLS, &format!("Extra {n}")).unwrap().0;
    }
    for b in BUILT_IN {
        let got = t.get(b.id).unwrap();
        assert_eq!(got.name, b.name);
        assert_eq!(got.parent, b.parent);
        assert!(got.built_in);
    }
}

/// Every element kind that exists today names a category; an unknown kind is an error value.
#[test]
fn every_kind_asks_its_category() {
    assert_eq!(category_of_kind("wall.straight"), Ok(WALLS));
    assert_eq!(category_of_kind("wall.layered"), Ok(WALLS));
    assert_eq!(category_of_kind("door.single"), Ok(DOORS));
    assert_eq!(category_of_kind("family.instance"), Ok(GENERIC_MODELS));
    assert_eq!(category_of_kind("block"), Ok(GENERIC_MODELS));
    assert_eq!(category_of_kind("floor"), Ok(construct_categories::FLOORS));
    assert_eq!(category_of_kind("roof"), Ok(construct_categories::ROOFS));
    assert_eq!(
        category_of_kind("column"),
        Ok(construct_categories::COLUMNS)
    );
    assert_eq!(category_of_kind("workshop.part"), Ok(GENERIC_MODELS));
    assert_eq!(category_of_kind("workshop.assembly"), Ok(GENERIC_MODELS));
    assert_eq!(category_of_kind("view"), Ok(construct_categories::VIEWS));
    assert_eq!(category_of_kind("sheet"), Ok(construct_categories::SHEETS));
    assert_eq!(
        category_of_kind("viewport"),
        Ok(construct_categories::VIEWPORTS)
    );
    assert!(category_of_kind("part").is_err());
    assert_eq!(
        category_of_kind("teapot"),
        Err(CategoryError::UnknownKind("teapot".to_string()))
    );
    assert!(category_of_kind("").is_err());
    assert!(category_of_kind("Wall.Straight").is_err());
}

/// The empty table's bytes are pinned, and the same table gives the same bytes every time.
#[test]
fn bytes_are_stable() {
    let empty = CategoryTable::new();
    assert_eq!(
        empty.to_bytes().unwrap(),
        br#"{"format":1,"subcategories":[]}"#
    );
    let (t, _) = empty.add_subcategory(WALLS, "Fire rating line").unwrap();
    let (t, _) = t.add_subcategory(DOORS, "Kick plate").unwrap();
    assert_eq!(
        String::from_utf8(t.to_bytes().unwrap()).unwrap(),
        concat!(
            r#"{"format":1,"subcategories":["#,
            r#"{"id":1000000,"parent":100,"name":"Fire rating line"},"#,
            r#"{"id":1000001,"parent":300,"name":"Kick plate"}]}"#
        )
    );
    assert_eq!(t.to_bytes().unwrap(), t.to_bytes().unwrap());
}

/// A table read back is the table written, and writing it again gives the same bytes.
#[test]
fn bytes_round_trip() {
    let (t, _) = CategoryTable::new()
        .add_subcategory(WALLS, "Fire rating line")
        .unwrap();
    let (t, _) = t.add_subcategory(WALLS, "Acoustic line").unwrap();
    let bytes = t.to_bytes().unwrap();
    let back = CategoryTable::from_bytes(&bytes).unwrap();
    assert_eq!(back, t);
    assert_eq!(back.to_bytes().unwrap(), bytes);
}

/// Stored bytes that break a rule are refused, not repaired.
#[test]
fn bad_bytes_are_refused() {
    let read = |s: &str| CategoryTable::from_bytes(s.as_bytes());
    assert!(matches!(read("not json"), Err(CategoryError::Malformed(_))));
    assert_eq!(
        read(r#"{"format":2,"subcategories":[]}"#),
        Err(CategoryError::UnsupportedFormat(2))
    );
    // An id in the built-in range, out of order, or repeated.
    assert!(read(r#"{"format":1,"subcategories":[{"id":5,"parent":100,"name":"x"}]}"#).is_err());
    assert!(
        read(concat!(
            r#"{"format":1,"subcategories":["#,
            r#"{"id":1000001,"parent":100,"name":"a"},"#,
            r#"{"id":1000000,"parent":100,"name":"b"}]}"#
        ))
        .is_err()
    );
    // A name that repeats under a parent, or repeats a built-in's.
    assert!(
        read(concat!(
            r#"{"format":1,"subcategories":["#,
            r#"{"id":1000000,"parent":100,"name":"a"},"#,
            r#"{"id":1000001,"parent":100,"name":"A"}]}"#
        ))
        .is_err()
    );
    assert!(
        read(r#"{"format":1,"subcategories":[{"id":1000000,"parent":300,"name":"frame"}]}"#)
            .is_err()
    );
    // A missing parent, and a parent that is a subcategory.
    assert!(
        read(r#"{"format":1,"subcategories":[{"id":1000000,"parent":7,"name":"x"}]}"#).is_err()
    );
    assert!(
        read(r#"{"format":1,"subcategories":[{"id":1000000,"parent":301,"name":"x"}]}"#).is_err()
    );
}
