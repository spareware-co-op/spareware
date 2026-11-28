//! A wall's own edits, each against the 6 m by 3 m wall of 12 panels (vertical lines at 1.5, 3
//! and 4.5 m, horizontal at 1 and 2 m): grid lines added and removed one by one, segments left out
//! and put back, panels swapped, and edits the grid no longer has a place for.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{all, every, mm, mullion, places, six_by_three, solid, spec};
use construct_curtain::{
    Crossing, CurtainError, Direction, DoorPanel, Mullions, Panel, Segment, quantities,
};
use construct_params::Area;

/// A segment of a line, in whole millimetres.
fn segment(direction: Direction, at: i64, from: i64) -> Segment {
    Segment {
        direction,
        at: mm(at),
        from: mm(from),
    }
}

/// A grid line added at 750 mm makes a fifth column; one at a line it has, within the tolerance
/// of one, or not inside the borders is refused, naming the line.
#[test]
fn a_grid_line_is_added_one_by_one() {
    let wall = six_by_three(Mullions::default());
    let more = wall.with_line(Direction::Vertical, mm(750)).unwrap();
    let grid = more.grid().unwrap();
    assert_eq!(places(&grid.vertical), [0, 750, 1500, 3000, 4500, 6000]);
    assert!(grid.vertical[1].added && !grid.vertical[2].added);
    assert_eq!(grid.panels.len(), 15);
    assert_eq!(
        wall.with_line(Direction::Vertical, mm(1500)).unwrap_err(),
        CurtainError::LineTaken {
            direction: Direction::Vertical,
            at: mm(1500),
            near: mm(1500),
        }
    );
    let close = construct_params::Length::from_steps(mm(1500).steps() + 64);
    assert!(matches!(
        wall.with_line(Direction::Vertical, close),
        Err(CurtainError::LineTaken { .. })
    ));
    for outside in [0, 6000, 7000] {
        assert_eq!(
            wall.with_line(Direction::Vertical, mm(outside))
                .unwrap_err(),
            CurtainError::LineOutside {
                direction: Direction::Vertical,
                at: mm(outside),
            }
        );
    }
    let higher = wall.with_line(Direction::Horizontal, mm(2500)).unwrap();
    assert_eq!(
        places(&higher.grid().unwrap().horizontal),
        [0, 1000, 2000, 2500, 3000]
    );
}

/// A line of its type is removed (its columns merge into one panel 3 m wide), an added line is
/// taken away, a border is no grid line, and adding a removed line of its type back brings it
/// back as its type's.
#[test]
fn a_grid_line_is_removed_one_by_one() {
    let wall = six_by_three(Mullions::default());
    let less = wall.without_line(Direction::Vertical, mm(3000)).unwrap();
    let grid = less.grid().unwrap();
    assert_eq!(places(&grid.vertical), [0, 1500, 4500, 6000]);
    assert_eq!(grid.panels.len(), 9);
    assert_eq!(grid.panels[3].width(), mm(3000));
    let back = less.with_line(Direction::Vertical, mm(3000)).unwrap();
    assert_eq!(back, wall);
    let added = wall.with_line(Direction::Vertical, mm(750)).unwrap();
    assert_eq!(
        added.without_line(Direction::Vertical, mm(750)).unwrap(),
        wall
    );
    for none in [0, 6000, 750] {
        assert_eq!(
            wall.without_line(Direction::Vertical, mm(none))
                .unwrap_err(),
            CurtainError::NoSuchLine {
                direction: Direction::Vertical,
                at: mm(none),
            }
        );
    }
}

/// Leaving out the segment of the 1.5 m line from the base to 1 m makes the two panels either side
/// one, 3 m by 1 m, and takes its mullion away: 11 panels, 30 mullions. Putting it back gives the
/// wall it was.
#[test]
fn leaving_a_segment_out_merges_its_panels() {
    let wall = six_by_three(all(&mullion(50, 150)));
    let cut = segment(Direction::Vertical, 1500, 0);
    let merged = wall.with_segment_omitted(cut).unwrap();
    let grid = merged.grid().unwrap();
    assert_eq!(grid.panels.len(), 11);
    assert_eq!(grid.panels[0].grid, [[mm(0), mm(3000)], [mm(0), mm(1000)]]);
    assert_eq!(grid.panels[0].width(), mm(2925));
    let q = quantities(&grid).unwrap();
    assert_eq!(q.mullions, 30);
    assert!(
        grid.vertical[1].segments[0].omitted && !grid.vertical[1].segments[0].mullion,
        "the left-out segment carries no mullion"
    );
    // With no vertical mullion at the 1.5 m line's foot, the sill runs through there: its two
    // mullions meet on the line. The vertical mullion above stops on the 1 m line's band.
    let sill: Vec<_> = grid
        .mullions
        .iter()
        .filter(|m| m.direction == Direction::Horizontal && m.at == mm(0))
        .map(|m| m.run)
        .collect();
    assert_eq!(sill[..2], [[mm(50), mm(1500)], [mm(1500), mm(2975)]]);
    let above = grid
        .mullions
        .iter()
        .find(|m| m.direction == Direction::Vertical && m.at == mm(1500))
        .unwrap();
    assert_eq!((above.from, above.run), (mm(1000), [mm(975), mm(2000)]));
    assert_eq!(
        merged.with_segment_omitted(cut).unwrap_err(),
        CurtainError::AlreadyOmitted {
            direction: Direction::Vertical,
            at: mm(1500),
            from: mm(0),
        }
    );
    assert_eq!(merged.with_segment_restored(cut).unwrap(), wall);
    assert_eq!(
        wall.with_segment_restored(cut).unwrap_err(),
        CurtainError::NotOmitted {
            direction: Direction::Vertical,
            at: mm(1500),
            from: mm(0),
        }
    );
    assert_eq!(
        wall.with_segment_omitted(segment(Direction::Vertical, 1500, 500))
            .unwrap_err(),
        CurtainError::NoSuchSegment {
            direction: Direction::Vertical,
            at: mm(1500),
            from: mm(500),
        }
    );
    assert_eq!(
        wall.with_segment_omitted(segment(Direction::Vertical, 1000, 0))
            .unwrap_err(),
        CurtainError::NoSuchLine {
            direction: Direction::Vertical,
            at: mm(1000),
        }
    );
}

/// A panel is a rectangle of cells: leaving out a vertical and a horizontal segment round one
/// corner would make an L, and is refused. Leaving out the three segments round a 2 by 2 block
/// makes it one panel, and the fourth segment inside it carries no mullion.
#[test]
fn every_panel_is_a_rectangle_of_cells() {
    let wall = six_by_three(all(&mullion(50, 150)));
    let first = wall
        .with_segment_omitted(segment(Direction::Vertical, 1500, 0))
        .unwrap();
    assert_eq!(
        first
            .with_segment_omitted(segment(Direction::Horizontal, 1000, 0))
            .unwrap_err(),
        CurtainError::PanelNotRectangle { at: [mm(0), mm(0)] }
    );
    let block = first
        .with_segment_omitted(segment(Direction::Vertical, 1500, 1000))
        .unwrap()
        .with_segment_omitted(segment(Direction::Horizontal, 1000, 0))
        .unwrap();
    let grid = block.grid().unwrap();
    assert_eq!(grid.panels.len(), 9);
    assert_eq!(grid.panels[0].grid, [[mm(0), mm(3000)], [mm(0), mm(2000)]]);
    let inside = grid.horizontal[1].segments[1];
    assert!(!inside.omitted && !inside.mullion);
    assert_eq!(quantities(&grid).unwrap().mullions, 31 - 4);
}

/// A panel is swapped cell by cell, by the corner its cell starts at: a solid panel in the cell at
/// 1.5 m along and 1 m up takes 1.5 m2 off the glazed area; a door panel is placed at its panel's
/// middle and foot and sized to it; `None` puts the type's panel back; a corner no panel starts at
/// is refused.
#[test]
fn a_panel_is_swapped_cell_by_cell() {
    let wall = six_by_three(Mullions::default());
    let at = [mm(1500), mm(1000)];
    let swapped = wall.with_panel(at, Some(solid(50))).unwrap();
    let grid = swapped.grid().unwrap();
    let panel = grid.panels.iter().find(|p| p.at == at).unwrap();
    assert!(panel.swapped);
    let q = quantities(&grid).unwrap();
    assert_eq!(q.glazed_area, Area::from_mm2(16_500_000));
    assert_eq!(q.panel_area, Area::from_mm2(18_000_000));
    assert_eq!(swapped.with_panel(at, None).unwrap(), wall);
    assert_eq!(
        wall.with_panel([mm(750), mm(0)], Some(Panel::Empty))
            .unwrap_err(),
        CurtainError::NoSuchPanel {
            at: [mm(750), mm(0)]
        }
    );

    let family = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let door = DoorPanel::new(family, "d900x2100").unwrap();
    let with_door = wall
        .with_panel([mm(1500), mm(0)], Some(Panel::Door(door.clone())))
        .unwrap();
    let grid = with_door.grid().unwrap();
    let doors = grid.doors().unwrap();
    assert_eq!(doors.len(), 1);
    assert_eq!(
        (
            doors[0].width,
            doors[0].height,
            doors[0].middle,
            doors[0].foot
        ),
        (mm(1500), mm(1000), mm(2250), mm(0))
    );
    assert_eq!(doors[0].door, door);
    // A door panel has no solid of its own here: its family is its builder's.
    assert_eq!(grid.solids().unwrap().len(), 11);
    assert!(matches!(
        DoorPanel::new("door", "d900x2100"),
        Err(CurtainError::BadDoor(_))
    ));
}

/// An edit the grid no longer has a place for is kept and does nothing, and is listed as unused:
/// a removed line its new type does not lay out, a left-out segment of that line, and a swap of a
/// panel whose corner is gone. Changing the type back brings each back.
#[test]
fn an_edit_with_no_place_is_unused() {
    let wall = six_by_three(Mullions::default())
        .without_line(Direction::Vertical, mm(3000))
        .unwrap()
        .with_segment_omitted(segment(Direction::Vertical, 1500, 0))
        .unwrap()
        .with_panel([mm(4500), mm(2000)], Some(solid(50)))
        .unwrap();
    let wider = wall.with_spec(spec(
        every(2000),
        every(1000),
        Mullions::default(),
        Crossing::Vertical,
    ));
    let grid = wider.grid().unwrap();
    assert_eq!(places(&grid.vertical), [0, 2000, 4000, 6000]);
    assert_eq!(grid.unused.len(), 3, "{:?}", grid.unused);
    assert!(grid.unused[0].contains("removed vertical grid line at 3000 mm"));
    assert!(grid.unused[1].contains("left-out segment of the vertical grid line at 1500 mm"));
    assert!(grid.unused[2].contains("4500 mm along and 2000 mm up"));
    assert_eq!(grid.panels.len(), 9);
    let back = wider.with_spec(wall.spec().clone());
    assert_eq!(back, wall);
    assert_eq!(back.grid().unwrap().unused, Vec::<String>::new());
}
