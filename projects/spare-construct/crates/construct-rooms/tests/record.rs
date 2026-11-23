//! A room's record and recipe as FORMAT.md writes them: its kind, category, number and ties, its
//! point and upper limit in their one spelling, read back as written; a sloped limit refused by
//! name; and the recipe the builders take once its loop is found, pinned byte for byte.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_levels::{Tie, base_of, top_of};
use construct_params::Length;
use construct_rooms::{
    Bound, Limit, NUMBER_KEY, Outline, RoomError, at_of, filled, find, is_room, limit_of,
    number_of, reads_walls, room_recipe, room_record,
};
use kit_core::Id;
use serde_json::json;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn tie(level: &str, mm: i32) -> Tie {
    Tie {
        level: i(level),
        offset: Length::from_mm(mm),
    }
}

fn at() -> [Length; 2] {
    [Length::from_mm(2000), Length::from_steps(80_000)]
}

#[test]
fn a_rooms_record_holds_its_number_point_and_limits() {
    let up = Limit::Level(tie("L-1", -100));
    let record = room_record(
        &i("R-1"),
        &i("doc"),
        "Kitchen",
        "101",
        at(),
        &tie("L-0", 0),
        &up,
    )
    .unwrap();
    assert!(is_room(&record));
    assert_eq!(record.kind, "room");
    assert_eq!(record.name, "Kitchen");
    assert_eq!(number_of(&record), Some("101"));
    assert_eq!(record.props[NUMBER_KEY], "101");
    assert_eq!(record.props["@category"], "3300");
    assert_eq!(base_of(&record).unwrap(), Some(tie("L-0", 0)));
    assert_eq!(top_of(&record).unwrap(), Some(tie("L-1", -100)));
    assert_eq!(limit_of(&record).unwrap(), up);
    assert_eq!(at_of(&record.recipe).unwrap(), [12_800_000, 80_000]);
    assert!(reads_walls(&record.recipe));
    assert_eq!(
        serde_json::to_value(&record.recipe).unwrap(),
        json!({
            "owner": "construct",
            "schema": 1,
            "op": "room",
            "op_version": 1,
            "args": {
                "at": ["length:q0:12800000", "length:q0:80000"],
                "height": { "tie": "top" },
                "walls": { "room": "walls" },
            },
        })
    );

    let given = Limit::Height(Length::from_mm(2400));
    let record = room_record(
        &i("R-2"),
        &i("doc"),
        "Store",
        "",
        at(),
        &tie("L-0", 0),
        &given,
    )
    .unwrap();
    assert_eq!(record.recipe.args["height"], "length:q0:15360000");
    assert_eq!(limit_of(&record).unwrap(), given);
    assert_eq!(top_of(&record).unwrap(), None);
}

/// A sloped limit and a height at or below the base are refused by name; a recipe of another
/// operation is not a room's.
#[test]
fn a_sloped_limit_is_refused_by_name() {
    assert_eq!(
        room_recipe(at(), &Limit::Sloped).unwrap_err(),
        RoomError::SlopedLimit
    );
    assert_eq!(
        room_record(
            &i("R-1"),
            &i("doc"),
            "Attic",
            "301",
            at(),
            &tie("L-3", 0),
            &Limit::Sloped
        )
        .unwrap_err()
        .code(),
        "SlopedLimit"
    );
    let mut record = room_record(
        &i("R-1"),
        &i("doc"),
        "Attic",
        "301",
        at(),
        &tie("L-3", 0),
        &Limit::Height(Length::from_mm(2400)),
    )
    .unwrap();
    record
        .recipe
        .args
        .insert("height".to_owned(), json!({ "slope": "roof R-9" }));
    assert_eq!(limit_of(&record).unwrap_err(), RoomError::SlopedLimit);
    assert_eq!(
        room_record(
            &i("R-1"),
            &i("doc"),
            "Void",
            "",
            at(),
            &tie("L-0", 0),
            &Limit::Height(Length::from_mm(0))
        )
        .unwrap_err(),
        RoomError::NoHeight
    );
    let wall = construct_model::geometry::Recipe::construct(
        "wall.layered",
        std::collections::BTreeMap::default(),
    );
    assert!(matches!(at_of(&wall), Err(RoomError::NotARoom(_))));
}

/// The recipe the builders take: the height as a stored length and the loops in grid steps, each
/// corner naming the wall whose face runs from it, outer loop first.
#[test]
fn the_filled_recipe_holds_the_loops_and_the_height() {
    let mm = |v: i64| v * 6400;
    let rect = |id: &str, x0: i64, y0: i64, x1: i64, y1: i64| Bound {
        wall: i(id),
        outline: Outline::Polygon(vec![
            [mm(x0), mm(y0)],
            [mm(x1), mm(y0)],
            [mm(x1), mm(y1)],
            [mm(x0), mm(y1)],
        ]),
    };
    let walls = vec![
        rect("w-s", 0, -200, 3000, 0),
        rect("w-e", 3000, -200, 3200, 2000),
        rect("w-n", 0, 2000, 3200, 2200),
        rect("w-w", -200, -200, 0, 2200),
        rect("c-1", 1000, 1000, 1100, 1100),
    ];
    let found = find(&walls, [mm(500), mm(500)]).unwrap().unwrap();
    let mut recipe = room_recipe(at(), &Limit::Height(Length::from_mm(2400))).unwrap();
    let built = filled(&recipe, &found).unwrap();
    assert!(!reads_walls(&built));
    assert_eq!(
        serde_json::to_string(&built.args).unwrap(),
        concat!(
            r#"{"height":"length:q0:15360000","loops":[["#,
            r#"{"at":[0,0],"wall":"w-s"},{"at":[19200000,0],"wall":"w-e"},"#,
            r#"{"at":[19200000,12800000],"wall":"w-n"},{"at":[0,12800000],"wall":"w-w"}],["#,
            r#"{"at":[6400000,6400000],"wall":"c-1"},{"at":[6400000,7040000],"wall":"c-1"},"#,
            r#"{"at":[7040000,7040000],"wall":"c-1"},{"at":[7040000,6400000],"wall":"c-1"}]]}"#
        )
    );
    // A height not yet read from the levels cannot be built.
    recipe
        .args
        .insert("height".to_owned(), json!({ "tie": "top" }));
    assert!(matches!(
        filled(&recipe, &found),
        Err(RoomError::Malformed(_))
    ));
}
