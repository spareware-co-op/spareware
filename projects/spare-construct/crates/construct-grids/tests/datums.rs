//! A document's grids and reference planes together: the rules that need to see them all.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{degrees, id, mm, pt};
use construct_grids::{Datums, Grid, GridError, PlaneShape, ReferencePlane, Shape};

fn line(a: (i32, i32), b: (i32, i32)) -> Shape {
    Shape::line(pt(a.0, a.1), pt(b.0, b.1)).unwrap()
}

fn grid(key: &str, name: &str, a: (i32, i32), b: (i32, i32)) -> Grid {
    Grid::new(id(key), name, line(a, b)).unwrap()
}

fn vertical(key: &str, name: Option<&str>) -> ReferencePlane {
    let shape = PlaneShape::Vertical {
        from: pt(0, 0),
        to: pt(0, 1000),
    };
    ReferencePlane::new(id(key), name, shape).unwrap()
}

fn horizontal(key: &str, name: Option<&str>, elevation: i32) -> ReferencePlane {
    let shape = PlaneShape::Horizontal {
        elevation: mm(elevation),
    };
    ReferencePlane::new(id(key), name, shape).unwrap()
}

/// Three grids across (1, 2, 3) and two up (A, B), 6 m apart.
fn set() -> Datums {
    let mut set = Datums::new();
    for (n, x) in [("1", 0), ("2", 6000), ("3", 12_000)] {
        set.add_grid(grid(&format!("G-{n}"), n, (x, -1000), (x, 7000)))
            .unwrap();
    }
    for (n, y) in [("A", 0), ("B", 6000)] {
        set.add_grid(grid(&format!("G-{n}"), n, (-1000, y), (13_000, y)))
            .unwrap();
    }
    set
}

#[test]
fn a_grids_name_is_unique_without_regard_to_case() {
    let mut set = set();
    let clash = grid("G-new", "a", (0, 0), (1, 1));
    assert_eq!(
        set.add_grid(clash),
        Err(GridError::DuplicateGridName("a".to_owned()))
    );
    assert_eq!(set.grids().len(), 5, "nothing changed");
    assert!(set.add_grid(grid("G-new", "C", (0, 0), (1, 1))).is_ok());
}

#[test]
fn a_renamed_grid_keeps_its_own_name_free_and_takes_no_other() {
    let mut set = set();
    assert_eq!(
        set.rename_grid(&id("G-A"), "b"),
        Err(GridError::DuplicateGridName("b".to_owned()))
    );
    // Its own name in another case is its own.
    set.rename_grid(&id("G-A"), "a").unwrap();
    assert_eq!(set.grid(&id("G-A")).unwrap().name(), "a");
    assert_eq!(set.rename_grid(&id("G-A"), "  "), Err(GridError::EmptyName));
    assert_eq!(
        set.rename_grid(&id("G-none"), "Z"),
        Err(GridError::UnknownGrid(id("G-none")))
    );
}

#[test]
fn an_id_is_a_grids_or_a_planes_never_both() {
    let mut set = set();
    assert_eq!(
        set.add_grid(grid("G-A", "Q", (0, 0), (1, 1))),
        Err(GridError::DuplicateId(id("G-A")))
    );
    set.add_plane(vertical("P-1", None)).unwrap();
    assert_eq!(
        set.add_grid(grid("P-1", "Q", (0, 0), (1, 1))),
        Err(GridError::DuplicateId(id("P-1")))
    );
    assert_eq!(
        set.add_plane(vertical("G-A", None)),
        Err(GridError::DuplicateId(id("G-A")))
    );
}

#[test]
fn a_reference_plane_may_have_no_name_but_a_name_is_unique_when_given() {
    let mut set = Datums::new();
    set.add_plane(vertical("P-1", None)).unwrap();
    set.add_plane(vertical("P-2", None)).unwrap();
    set.add_plane(horizontal("P-3", Some("Datum"), -300))
        .unwrap();
    assert_eq!(
        set.add_plane(vertical("P-4", Some("DATUM"))),
        Err(GridError::DuplicatePlaneName("DATUM".to_owned()))
    );
    // A grid of the same name is no clash: the two are separate.
    set.add_grid(grid("G-1", "Datum", (0, 0), (1000, 0)))
        .unwrap();
    set.rename_plane(&id("P-1"), Some("Centre")).unwrap();
    assert_eq!(
        set.rename_plane(&id("P-2"), Some("centre")),
        Err(GridError::DuplicatePlaneName("centre".to_owned()))
    );
    set.rename_plane(&id("P-1"), None).unwrap();
    assert!(set.plane(&id("P-1")).unwrap().name().is_none());
}

#[test]
fn a_named_plane_is_found_by_name() {
    let mut set = Datums::new();
    set.add_plane(horizontal("P-3", Some("Datum"), -300))
        .unwrap();
    set.add_plane(vertical("P-1", None)).unwrap();
    let found = set.plane_named("datum").unwrap();
    assert_eq!(found.id(), &id("P-3"));
    assert_eq!(
        found.shape(),
        &PlaneShape::Horizontal {
            elevation: mm(-300)
        }
    );
    assert!(set.plane_named("Nothing").is_none());
    assert!(
        set.plane_named("").is_none(),
        "an unnamed plane is not found by no name"
    );
    assert_eq!(
        set.grid_named("Datum"),
        None,
        "grids are looked up as grids"
    );
}

#[test]
fn the_next_grid_name_follows_the_rule_and_skips_what_is_taken() {
    let set = set();
    assert_eq!(set.suggest_grid_name(Some("3")), "4");
    assert_eq!(set.suggest_grid_name(Some("B")), "C");
    assert_eq!(set.suggest_grid_name(Some("1")), "4", "2 and 3 are taken");
    assert_eq!(
        set.suggest_grid_name(Some("a")),
        "c",
        "B is taken without regard to case; the case of a is kept"
    );
    assert_eq!(set.suggest_grid_name(None), "4");
    assert_eq!(Datums::new().suggest_grid_name(None), "1");
}

#[test]
fn lists_come_in_one_order_whatever_order_things_were_added() {
    let one = set();
    let mut two = Datums::new();
    for grid in one.grids().into_iter().rev() {
        two.add_grid(grid.clone()).unwrap();
    }
    assert_eq!(one, two);
    let names: Vec<_> = one.grids().iter().map(|g| g.name().to_owned()).collect();
    assert_eq!(names, ["1", "2", "3", "A", "B"]);
    let document = id("DOC-1");
    assert_eq!(one.to_records(&document), two.to_records(&document));
}

#[test]
fn where_two_grids_cross_and_the_grid_nearest_a_point() {
    let set = set();
    let crossing = set.crossings_of(&id("G-2"), &id("G-B")).unwrap();
    assert_eq!(crossing.len(), 1);
    assert_eq!(crossing[0].point, pt(6000, 6000));
    assert!(crossing[0].exact);
    assert!(
        set.crossings_of(&id("G-1"), &id("G-3")).unwrap().is_empty(),
        "parallel"
    );
    assert_eq!(
        set.crossings_of(&id("G-1"), &id("G-none")),
        Err(GridError::UnknownGrid(id("G-none")))
    );

    let near = set.nearest_grid(pt(5800, 3000)).unwrap();
    assert_eq!(near.name, "2");
    assert_eq!(near.foot, pt(6000, 3000));
    assert_eq!(near.distance, mm(200));
    let low = set.nearest_grid(pt(3000, 100)).unwrap();
    assert_eq!(low.name, "A");
    assert!(Datums::new().nearest_grid(pt(0, 0)).is_none());
}

#[test]
fn a_tie_goes_to_the_first_name() {
    let mut set = Datums::new();
    set.add_grid(grid("G-b", "b", (1000, 0), (1000, 5000)))
        .unwrap();
    set.add_grid(grid("G-a", "A", (-1000, 0), (-1000, 5000)))
        .unwrap();
    assert_eq!(set.nearest_grid(pt(0, 2000)).unwrap().name, "A");
}

#[test]
fn a_grid_is_reshaped_and_a_bad_shape_changes_nothing() {
    let mut set = set();
    let arc = Shape::arc(pt(0, 0), mm(500), degrees(0), degrees(90)).unwrap();
    set.reshape_grid(&id("G-A"), arc).unwrap();
    assert_eq!(set.grid(&id("G-A")).unwrap().shape(), &arc);
    let before = set.clone();
    let bad = Shape::Arc {
        centre: pt(0, 0),
        radius: mm(0),
        from: degrees(0),
        to: degrees(90),
    };
    assert_eq!(
        set.reshape_grid(&id("G-A"), bad),
        Err(GridError::ZeroRadius)
    );
    assert_eq!(set, before);
    assert_eq!(set.remove_grid(&id("G-A")).unwrap().name(), "A");
    assert_eq!(
        set.remove_grid(&id("G-A")),
        Err(GridError::UnknownGrid(id("G-A")))
    );
}

#[test]
fn a_plane_is_moved_in_the_directions_it_has() {
    let vertical = vertical("P-1", None);
    let moved = vertical.shape().moved(mm(100), mm(-50), mm(0)).unwrap();
    assert_eq!(
        moved,
        PlaneShape::Vertical {
            from: pt(100, -50),
            to: pt(100, 950)
        }
    );
    assert!(matches!(
        vertical.shape().moved(mm(0), mm(0), mm(10)),
        Err(GridError::WrongShape(_))
    ));
    let level = horizontal("P-2", None, 0);
    assert_eq!(
        level.shape().moved(mm(0), mm(0), mm(2700)).unwrap(),
        PlaneShape::Horizontal {
            elevation: mm(2700)
        }
    );
    assert!(matches!(
        level.shape().moved(mm(10), mm(0), mm(0)),
        Err(GridError::WrongShape(_))
    ));
}

#[test]
fn a_plane_of_no_length_is_refused() {
    let shape = PlaneShape::Vertical {
        from: pt(5, 5),
        to: pt(5, 5),
    };
    assert_eq!(
        ReferencePlane::new(id("P-1"), None, shape),
        Err(GridError::ZeroLength)
    );
}

#[test]
fn a_document_that_somehow_holds_two_of_a_name_still_reads() {
    let document = id("DOC-1");
    let one = grid("G-1", "A", (0, 0), (1, 1)).to_record(&document);
    let two = grid("G-2", "a", (0, 0), (2, 2)).to_record(&document);
    let read = Datums::from_records([&one, &two]).unwrap();
    assert_eq!(read.grids().len(), 2);
    assert_eq!(
        read.check_names(),
        Err(GridError::DuplicateGridName("A".to_owned()))
    );
}
