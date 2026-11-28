//! Ceilings (job A101, CON-A4-F4-P2): the type, the record, the grid pattern and its exact count,
//! with the numbers worked by hand in each test's comment.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{id, l_corners, l_with_hole, layer, mm, rectangle};
use construct_categories::{CEILINGS, CategoryTable, FLOORS};
use construct_levels::Tie;
use construct_params::Length;
use construct_slabs::{
    Boundary, CEILING_KIND, CEILING_OP, Function, GRID_ARG, Grid, LAYERS_ARG, LOOPS_ARG, Layers,
    SlabError, Tiles, area, ceiling_boundary, ceiling_layers_of, ceiling_read_only, ceiling_record,
    contains, default_ceiling_types, default_ceilings, filled, grid_of, interior_point, is_ceiling,
    layers_of, new_ceiling_type, quantities, shape_of, slab_layers_of, with_ceiling_boundary,
    with_grid,
};
use construct_solver::Point;

fn grid(width: i64, depth: i64, x: i64, y: i64, degrees: i64) -> Grid {
    let at = Point {
        x: mm(x).steps(),
        y: mm(y).steps(),
    };
    Grid::new(mm(width), mm(depth), at, degrees).unwrap()
}

/// The L-shaped outline without its hole: 8 m by 4 m, and 4 m by 4 m more above its left half.
fn l_plain() -> Boundary {
    Boundary::from_corners(&l_corners(), &[]).unwrap()
}

fn plaster() -> Layers {
    Layers::new(vec![layer(Function::Finish, 13, "plaster-gypsum")]).unwrap()
}

#[test]
fn a_ceiling_type_holds_its_layers_and_is_not_a_floor_type() {
    let ty = new_ceiling_type(id("ct-1"), "Plaster 13", &plaster(), &CategoryTable::new()).unwrap();
    assert_eq!(ty.category(), CEILINGS);
    assert_eq!(ceiling_layers_of(&ty).unwrap(), plaster());
    assert_eq!(slab_layers_of(&ty).unwrap(), plaster());
    assert!(matches!(
        layers_of(&ty),
        Err(SlabError::NotAFloorType { category }) if category == CEILINGS
    ));
    let floor =
        construct_slabs::new_floor_type(id("ft-1"), "Slab", &plaster(), &CategoryTable::new())
            .unwrap();
    assert_eq!(floor.category(), FLOORS);
    assert!(matches!(
        ceiling_layers_of(&floor),
        Err(SlabError::NotACeilingType { .. })
    ));
    assert_eq!(slab_layers_of(&floor).unwrap(), plaster());
}

#[test]
fn the_default_ceiling_types_are_three_and_read_back() {
    let all = default_ceilings().unwrap();
    let ids: Vec<&str> = all.iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        [
            "ceiling-plaster-13",
            "ceiling-plaster-13-timber-45",
            "ceiling-suspended-tile-15"
        ]
    );
    let totals: Vec<Length> = all.iter().map(|c| c.layers.total()).collect();
    assert_eq!(totals, [mm(13), mm(58), mm(15)]);
    let types = default_ceiling_types(&CategoryTable::new()).unwrap();
    for (ty, ceiling) in types.iter().zip(&all) {
        assert_eq!(ceiling_layers_of(ty).unwrap(), ceiling.layers);
    }
}

/// The L outline, 600 by 1200 tiles from its corner. Columns of 600 over x 0 to 8000 are 14
/// (13.33), rows of 1200 over y 0 to 8000 are 7 (6.67). Rows 0 to 2 (y to 3600) lie in the 8 m
/// wide part: 13 whole tiles and the one over x = 8000 cut, so 39 whole and 3 cut. Row 3
/// (3600 to 4800) is crossed by the step at y = 4000: the six tiles left of x = 3600 lie in the
/// tall part whole, and the eight from 3600 on are cut (one by the wall at x = 4000 too). Rows 4
/// and 5 (to 7200): six whole and the tile over x = 4000 cut. Row 6 (7200 to 8400) is cut by
/// the top edge at y = 8000: seven cut. 6 + 6 + 39 = 57 whole; 3 + 8 + 1 + 1 + 7 = 20 cut.
#[test]
fn tiles_of_an_l_shaped_ceiling_are_counted_exactly() {
    let tiles = grid(600, 1200, 0, 0, 0).tiles(&l_plain()).unwrap();
    assert_eq!(tiles, Tiles { whole: 57, cut: 20 });
    assert_eq!(tiles.total(), 77);
}

/// The same with the 2 m square opening at (1 m, 1 m). Its vertical edges are x = 1000 (inside
/// column 1, 600 to 1200) and x = 3000 (on a tile line, so it cuts nothing); its horizontal edges
/// y = 1000 (row 0) and y = 3000 (row 2) cross columns 1 to 4, which lie over its x range. Row 0:
/// columns 1 to 4 turn from whole to cut. Row 1: columns 2 to 4 lie inside the opening and are
/// not counted, column 1 is cut by x = 1000. Row 2: columns 1 to 4 turn cut. So whole falls by
/// 4 + 4 + 4 = 12 to 45 and cut rises by 4 + 1 + 4 = 9 to 29.
#[test]
fn an_opening_takes_tiles_out_and_cuts_those_it_crosses() {
    let tiles = grid(600, 1200, 0, 0, 0).tiles(&l_with_hole()).unwrap();
    assert_eq!(tiles, Tiles { whole: 45, cut: 29 });
}

/// A 3 m by 2.4 m rectangle. 600 by 1200 at 0 degrees: 5 columns by 2 rows, all whole, 10. At 90
/// degrees the tile is 1200 along x and 600 along y: 3000 / 1200 gives two whole columns and a
/// third cut, and 2400 / 600 gives four rows: 8 whole and 4 cut. With the origin moved to
/// x = 300 at 0 degrees the columns run -300 to 300 (cut), then four whole to 2700, then 2700
/// to 3300 (cut): 2 rows of four whole = 8, and cut 2 x 2 = 4.
#[test]
fn the_angle_and_the_origin_move_the_tiles() {
    let room = Boundary::from_corners(&rectangle("r", 0, 0, 3000, 2400), &[]).unwrap();
    assert_eq!(
        grid(600, 1200, 0, 0, 0).tiles(&room).unwrap(),
        Tiles { whole: 10, cut: 0 }
    );
    assert_eq!(
        grid(600, 1200, 0, 0, 90).tiles(&room).unwrap(),
        Tiles { whole: 8, cut: 4 }
    );
    assert_eq!(
        grid(600, 1200, 300, 0, 0).tiles(&room).unwrap(),
        Tiles { whole: 8, cut: 4 }
    );
}

#[test]
fn a_turned_grid_is_kept_and_refused_a_count_by_name() {
    let room = Boundary::from_corners(&rectangle("r", 0, 0, 3000, 2400), &[]).unwrap();
    let turned = grid(600, 1200, 0, 0, 45);
    assert_eq!(turned.angle(), 45);
    let refusal = turned.tiles(&room).unwrap_err();
    assert!(matches!(refusal, SlabError::BadGrid(_)), "{refusal}");
    assert!(refusal.to_string().contains("only 0 and 90"), "{refusal}");
}

#[test]
fn a_grid_refuses_what_it_cannot_hold() {
    let at = Point { x: 0, y: 0 };
    assert!(Grid::new(mm(0), mm(600), at, 0).is_err());
    assert!(Grid::new(mm(600), Length::from_steps(-1), at, 0).is_err());
    assert!(Grid::new(mm(600), mm(600), at, 180).is_err());
    assert!(Grid::new(mm(600), mm(600), at, -1).is_err());
    assert!(Grid::new(mm(600), mm(600), Point { x: i64::MAX, y: 0 }, 0).is_err());
    assert!(Grid::new(mm(600), mm(600), at, 179).is_ok());
}

#[test]
fn a_grid_is_stored_in_one_spelling_and_read_strictly() {
    let g = grid(600, 1200, 100, -200, 30);
    let stored = g.to_value();
    assert_eq!(
        stored,
        serde_json::json!({
            "width": mm(600).steps(),
            "depth": mm(1200).steps(),
            "origin": [mm(100).steps(), mm(-200).steps()],
            "angle": 30,
        })
    );
    assert_eq!(Grid::from_value(&stored).unwrap(), g);
    let mut extra = stored.clone();
    extra["colour"] = serde_json::json!("red");
    assert!(Grid::from_value(&extra).is_err());
    let mut text = stored;
    text["width"] = serde_json::json!("600 mm");
    assert!(Grid::from_value(&text).is_err());
}

#[test]
fn a_ceiling_record_holds_its_boundary_and_grid_and_builds_like_a_floor() {
    let tie = Tie {
        level: id("level-1"),
        offset: mm(2700),
    };
    let g = grid(600, 1200, 0, 0, 0);
    let record = ceiling_record(
        &id("c1"),
        &id("doc"),
        "Hall ceiling",
        &id("ct-1"),
        &tie,
        &l_with_hole(),
        Some(&g),
    );
    assert!(is_ceiling(&record));
    assert_eq!(record.kind, CEILING_KIND);
    assert_eq!(record.recipe.op, CEILING_OP);
    assert_eq!(record.recipe.op_version, 1);
    assert!(record.recipe.args.contains_key(GRID_ARG));
    assert_eq!(ceiling_boundary(&record.recipe).unwrap(), l_with_hole());
    assert_eq!(grid_of(&record.recipe).unwrap(), Some(g));
    // The builders read it as a floor's recipe: the same loops and layers.
    assert_eq!(shape_of(&record.recipe).unwrap().boundary(), &l_with_hole());
    let built = filled(&record.recipe, &plaster()).unwrap();
    assert!(built.args.contains_key(LOOPS_ARG));
    assert!(built.args.contains_key(LAYERS_ARG));
    assert!(!built.args.contains_key(GRID_ARG));
    assert_eq!(built.op, CEILING_OP);
    // Its boundary changes and its grid stays; its grid changes and its boundary stays.
    let moved = Boundary::from_corners(&rectangle("r", 0, 0, 3000, 2400), &[]).unwrap();
    let changed = with_ceiling_boundary(record.clone(), &moved).unwrap();
    assert_eq!(ceiling_boundary(&changed.recipe).unwrap(), moved);
    assert_eq!(grid_of(&changed.recipe).unwrap(), Some(g));
    let bare_one = with_grid(record.clone(), None).unwrap();
    assert_eq!(grid_of(&bare_one.recipe).unwrap(), None);
    assert_eq!(ceiling_boundary(&bare_one.recipe).unwrap(), l_with_hole());
    // A floor's record is no ceiling's.
    let floor = construct_slabs::floor_record(
        &id("f1"),
        &id("doc"),
        "Slab",
        &id("ft-1"),
        &tie,
        &l_with_hole(),
    );
    assert!(with_grid(floor, None).is_err());
    assert!(ceiling_boundary(&construct_slabs::floor_recipe(&l_with_hole())).is_err());
}

#[test]
fn a_ceiling_recipe_of_version_two_is_refused() {
    let mut recipe = construct_slabs::ceiling_recipe(&l_with_hole(), None);
    recipe.op_version = construct_slabs::SHAPED_VERSION;
    assert!(shape_of(&recipe).is_err());
}

/// The L outline with its opening is 48 m2 less 4 m2 = 44 m2; 13 mm of plaster over it is
/// 44 m2 x 0.013 m = 0.572 m3. The tiles come with a grid: 45 whole and 29 cut, 74 in all.
#[test]
fn a_ceilings_read_only_rows_are_its_area_volume_and_tiles() {
    let b = l_with_hole();
    assert_eq!(
        area(&b).unwrap(),
        construct_params::Area::from_square_steps(44_000_000 * 6400 * 6400)
    );
    let q = quantities(
        &b,
        &plaster(),
        &construct_materials::Materials::default_metric(),
    )
    .unwrap();
    let tiles = grid(600, 1200, 0, 0, 0).tiles(&b).unwrap();
    let rows = ceiling_read_only(&q, Some(tiles)).unwrap();
    let ids: Vec<&str> = rows.iter().map(|(d, _)| d.id().as_str()).collect();
    assert_eq!(
        ids,
        [
            "ceiling.area",
            "ceiling.perimeter",
            "ceiling.thickness",
            "ceiling.volume",
            "ceiling.mass",
            "ceiling.tiles",
            "ceiling.tiles-cut"
        ]
    );
    assert_eq!(ceiling_read_only(&q, None).unwrap().len(), 5);
    let volume = rows[3].1.clone().unwrap();
    let construct_params::Value::Volume(volume) = volume else {
        panic!("a volume");
    };
    assert_eq!(volume, q.volume);
}

#[test]
fn a_point_is_in_the_region_not_on_its_edge_and_not_in_an_opening() {
    let b = l_with_hole();
    let at = |x: i64, y: i64| [mm(x).steps(), mm(y).steps()];
    assert!(contains(&b, at(6000, 2000)));
    assert!(contains(&b, at(2000, 6000)));
    assert!(!contains(&b, at(2000, 2000)), "in the opening");
    assert!(!contains(&b, at(6000, 6000)), "outside the L");
    assert!(!contains(&b, at(0, 0)), "on a corner");
    assert!(!contains(&b, at(4000, 6000)), "on the edge");
    assert!(!contains(&b, at(1000, 2000)), "on the opening's edge");
}

/// The first ear of the L, its corners (0, 8000), (0, 0), (8000, 0) mm, has its centre in the
/// opening ((2667, 2667) mm), so the next ear (0, 0), (8000, 0), (8000, 4000) answers: its centre
/// is (16000 / 3, 4000 / 3) mm, to a whole step below.
#[test]
fn an_interior_point_is_the_same_each_time_and_inside() {
    let b = l_with_hole();
    let p = interior_point(&b).unwrap();
    assert_eq!(p, [102_400_000 / 3, 25_600_000 / 3]);
    assert!(contains(&b, p));
    assert_eq!(interior_point(&b), Some(p));
    let plain = interior_point(&l_plain()).unwrap();
    assert_eq!(plain, [51_200_000 / 3, 51_200_000 / 3]);
}
