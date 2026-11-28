//! A curtain wall laid out, against examples worked by hand: where its lines, panels and mullions
//! stand, how the crossings settle which mullion runs through, and what a wall reads back.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{all, every, mm, mullion, places, six_by_three, spec, wall};
use construct_curtain::{Crossing, Direction, Grid, Layout, Mullions, PanelKind, Part, quantities};
use construct_params::{Area, Length};

/// Square millimetres as an area.
fn mm2(mm2: i64) -> Area {
    Area::from_mm2(mm2)
}

/// Each panel's own width and height, in whole millimetres, by its corner.
fn sizes(grid: &Grid) -> Vec<([i64; 2], [i64; 2])> {
    grid.panels
        .iter()
        .map(|p| {
            (
                [p.at[0].whole_mm().unwrap(), p.at[1].whole_mm().unwrap()],
                [
                    p.width().whole_mm().unwrap(),
                    p.height().whole_mm().unwrap(),
                ],
            )
        })
        .collect()
}

/// Worked by hand: a 6 m by 3 m wall with vertical lines every 1.5 m and horizontal lines every
/// 1 m, no mullions. Vertical lines at 1.5, 3 and 4.5 m make 4 columns; horizontal lines at 1 and
/// 2 m make 3 rows: 12 panels of 1.5 m by 1 m, 1.5 m2 each, 18 m2 in all, all glazed.
#[test]
fn six_by_three_at_one_and_a_half_by_one_is_twelve_panels() {
    let grid = six_by_three(Mullions::default()).grid().unwrap();
    assert_eq!(places(&grid.vertical), [0, 1500, 3000, 4500, 6000]);
    assert_eq!(places(&grid.horizontal), [0, 1000, 2000, 3000]);
    assert_eq!(grid.panels.len(), 12);
    for (_, size) in sizes(&grid) {
        assert_eq!(size, [1500, 1000]);
    }
    assert_eq!(grid.mullions.len(), 0);
    let q = quantities(&grid).unwrap();
    assert_eq!(q.panels, 12);
    assert_eq!(q.panel_area, mm2(18_000_000));
    assert_eq!(q.glazed_area, mm2(18_000_000));
    assert_eq!(q.area, mm2(18_000_000));
    assert_eq!(q.mullions, 0);
    assert_eq!(q.mullion_length, mm(0));
}

/// The brief's count of 8 panels is the same wall at 1.5 m by 1.5 m: 4 columns by 2 rows, each
/// 1.5 m by 1.5 m, 2.25 m2.
#[test]
fn six_by_three_at_one_and_a_half_square_is_eight_panels() {
    let grid = wall(
        6000,
        3000,
        spec(
            every(1500),
            every(1500),
            Mullions::default(),
            Crossing::Vertical,
        ),
    )
    .grid()
    .unwrap();
    assert_eq!(places(&grid.horizontal), [0, 1500, 3000]);
    assert_eq!(grid.panels.len(), 8);
    for (_, size) in sizes(&grid) {
        assert_eq!(size, [1500, 1500]);
    }
    let q = quantities(&grid).unwrap();
    assert_eq!(q.panel_area, mm2(18_000_000));
}

/// Worked by hand, the 12-panel wall with 50 mm mullions on every line and border, the vertical
/// ones running through. The borders' mullions stand inside the wall (0 to 50 and 5950 to 6000
/// along; 0 to 50 and 2950 to 3000 up); the interior ones are centred on their lines (1475 to
/// 1525, ...). So the columns are 1425, 1450, 1450 and 1425 mm wide and the rows 925, 950 and
/// 925 mm high: the panels' area is 5750 by 2800 mm, 16.1 m2. The 5 vertical lines carry 3
/// mullions each, 1 m long, meeting at the crossings: 15 mullions, 15 m. The 4 horizontal lines
/// carry 4 each, stopped at the vertical mullions' faces, as long as the columns are wide: 16
/// mullions, 23 m. 31 mullions, 38 m.
#[test]
fn mullions_on_every_line_with_the_verticals_running_through() {
    let grid = six_by_three(all(&mullion(50, 150))).grid().unwrap();
    let widths: Vec<i64> = sizes(&grid)
        .iter()
        .filter(|(at, _)| at[1] == 0)
        .map(|(_, size)| size[0])
        .collect();
    assert_eq!(widths, [1425, 1450, 1450, 1425]);
    let heights: Vec<i64> = sizes(&grid)
        .iter()
        .filter(|(at, _)| at[0] == 0)
        .map(|(_, size)| size[1])
        .collect();
    assert_eq!(heights, [925, 950, 925]);
    let vertical: Vec<i64> = grid
        .mullions
        .iter()
        .filter(|m| m.direction == Direction::Vertical)
        .map(|m| m.length().whole_mm().unwrap())
        .collect();
    assert_eq!(vertical, [1000; 15]);
    let horizontal: Vec<i64> = grid
        .mullions
        .iter()
        .filter(|m| m.direction == Direction::Horizontal && m.at == mm(1000))
        .map(|m| m.length().whole_mm().unwrap())
        .collect();
    assert_eq!(horizontal, [1425, 1450, 1450, 1425]);
    let q = quantities(&grid).unwrap();
    assert_eq!(q.panels, 12);
    assert_eq!(q.panel_area, mm2(16_100_000));
    assert_eq!(q.mullions, 31);
    assert_eq!(q.mullion_length, mm(38_000));
    // The first vertical border mullion runs the full height, the base's first mullion from the
    // border mullion's face to the first interior one's.
    let first = &grid.mullions[0];
    assert_eq!(
        (first.band, first.run),
        ([mm(0), mm(50)], [mm(0), mm(1000)])
    );
    let sill = grid
        .mullions
        .iter()
        .find(|m| m.direction == Direction::Horizontal && m.at == mm(0))
        .unwrap();
    assert_eq!((sill.run, sill.band), ([mm(50), mm(1475)], [mm(0), mm(50)]));
}

/// The same wall with the horizontal mullions running through: each horizontal mullion runs from
/// crossing to crossing (1.5 m, 24 m in all), each vertical one stops at their faces (925, 950 and
/// 925 mm, 14 m in all): still 31 mullions and 38 m, the panels unchanged.
#[test]
fn horizontals_running_through_swap_the_lengths() {
    let grid = wall(
        6000,
        3000,
        spec(
            every(1500),
            every(1000),
            all(&mullion(50, 150)),
            Crossing::Horizontal,
        ),
    )
    .grid()
    .unwrap();
    let total = |d: Direction| -> i64 {
        grid.mullions
            .iter()
            .filter(|m| m.direction == d)
            .map(|m| m.length().whole_mm().unwrap())
            .sum()
    };
    assert_eq!(total(Direction::Horizontal), 24_000);
    assert_eq!(total(Direction::Vertical), 14_000);
    let q = quantities(&grid).unwrap();
    assert_eq!((q.mullions, q.mullion_length), (31, mm(38_000)));
    assert_eq!(q.panel_area, mm2(16_100_000));
    let corner = grid
        .mullions
        .iter()
        .find(|m| m.direction == Direction::Horizontal && m.at == mm(0) && m.from == mm(0))
        .unwrap();
    assert_eq!(corner.run, [mm(0), mm(1500)]);
}

/// No two solids of a wall overlap: every pair of boxes is apart along at least one axis, or
/// touches face to face.
#[test]
fn no_two_solids_overlap() {
    for crossing in [Crossing::Vertical, Crossing::Horizontal] {
        let grid = wall(
            6000,
            3000,
            spec(every(1500), every(1000), all(&mullion(50, 150)), crossing),
        )
        .grid()
        .unwrap();
        let solids = grid.solids().unwrap();
        assert_eq!(solids.len(), 12 + 31);
        for (n, a) in solids.iter().enumerate() {
            for b in &solids[n + 1..] {
                let apart = |x: [Length; 2], y: [Length; 2]| x[1] <= y[0] || y[1] <= x[0];
                assert!(
                    apart(a.frame.along, b.frame.along)
                        || apart(a.frame.across, b.frame.across)
                        || apart(a.frame.up, b.frame.up),
                    "{} and {} overlap",
                    a.key,
                    b.key
                );
            }
        }
        let panels = solids
            .iter()
            .filter(|s| s.part == Part::Panel(PanelKind::Glazed))
            .count();
        assert_eq!(panels, 12);
        assert_eq!(solids[0].frame.across, [mm(-12), mm(12)]);
    }
}

/// A fixed spacing justified at the centre puts a line on the middle and every spacing either
/// side; at the end, every spacing back from the far border. On 5 m at 1.5 m: centre 1, 2.5 and
/// 4 m (end panels of 1 m); start 1.5, 3 and 4.5 m; end 0.5, 2 and 3.5 m.
#[test]
fn a_spacing_is_justified_at_the_start_the_centre_or_the_end() {
    use construct_curtain::Justify;
    let at = |justify| {
        Layout::Spacing {
            spacing: mm(1500),
            justify,
        }
        .lines(Direction::Vertical, mm(5000))
        .unwrap()
    };
    assert_eq!(at(Justify::Start), common::mms(&[1500, 3000, 4500]));
    assert_eq!(at(Justify::Centre), common::mms(&[1000, 2500, 4000]));
    assert_eq!(at(Justify::End), common::mms(&[500, 2000, 3500]));
    // An extent of an odd number of steps has its middle half a step off the grid: the pattern is
    // put half a step up, its one rounding.
    let odd = Length::from_steps(mm(5000).steps() + 1);
    let lines = Layout::Spacing {
        spacing: mm(1500),
        justify: Justify::Centre,
    }
    .lines(Direction::Vertical, odd)
    .unwrap();
    assert_eq!(
        lines,
        [mm(1000), mm(2500), mm(4000)]
            .map(|l| Length::from_steps(l.steps() + 1))
            .to_vec()
    );
    // A line on a border, or within the tolerance of one, is the border.
    assert_eq!(
        every(1500).lines(Direction::Vertical, mm(6000)).unwrap(),
        common::mms(&[1500, 3000, 4500])
    );
    let near = Length::from_steps(mm(6000).steps() + 64);
    assert_eq!(
        every(1500).lines(Direction::Vertical, near).unwrap(),
        common::mms(&[1500, 3000, 4500])
    );
}

/// A fixed count divides equally, each line the nearest step (a half up): 7 panels on 6 m, whose
/// sevenths are not whole steps. A most spacing is the fewest equal panels none wider: 5 on 5 m
/// at 1.2 m (each 1 m), and exactly 5 on 6 m at 1.2 m.
#[test]
fn a_count_and_a_most_spacing_divide_equally() {
    let seven = Layout::Count { count: 7 }
        .lines(Direction::Vertical, mm(6000))
        .unwrap();
    let steps: Vec<i64> = seven.iter().map(|l| l.steps()).collect();
    assert_eq!(
        steps,
        [
            5_485_714, 10_971_429, 16_457_143, 21_942_857, 27_428_571, 32_914_286
        ]
    );
    let most = Layout::Most { spacing: mm(1200) };
    assert_eq!(
        most.lines(Direction::Vertical, mm(5000)).unwrap(),
        common::mms(&[1000, 2000, 3000, 4000])
    );
    assert_eq!(
        most.lines(Direction::Vertical, mm(6000)).unwrap(),
        common::mms(&[1200, 2400, 3600, 4800])
    );
    assert_eq!(
        Layout::Count { count: 1 }
            .lines(Direction::Horizontal, mm(3000))
            .unwrap(),
        common::mms(&[])
    );
    assert_eq!(
        Layout::None.lines(Direction::Horizontal, mm(3000)).unwrap(),
        common::mms(&[])
    );
}

/// A wall too short for its mullions: 50 mm mullions on lines 50 mm apart leave the panel between
/// them no room, and the layout says which panel.
#[test]
fn a_panel_with_no_room_is_refused() {
    let tight = wall(
        1000,
        1000,
        spec(
            every(50),
            Layout::None,
            all(&mullion(50, 100)),
            Crossing::Vertical,
        ),
    );
    assert_eq!(
        tight.grid().unwrap_err(),
        construct_curtain::CurtainError::NoRoomForPanel { at: [mm(0), mm(0)] }
    );
}

/// The quantities read back as the read-only parameters a schedule lists, in order.
#[test]
fn the_quantities_are_read_only_parameters() {
    let grid = six_by_three(all(&mullion(50, 150))).grid().unwrap();
    let rows = construct_curtain::read_only(&quantities(&grid).unwrap()).unwrap();
    let ids: Vec<String> = rows.iter().map(|(d, _)| d.id().to_string()).collect();
    assert_eq!(
        ids,
        [
            "curtain.length",
            "curtain.height",
            "curtain.area",
            "curtain.panels",
            "curtain.panel-area",
            "curtain.glazed-area",
            "curtain.mullions",
            "curtain.mullion-length",
        ]
    );
    let stored: Vec<String> = rows
        .iter()
        .map(|(_, v)| v.clone().unwrap().stored())
        .collect();
    assert_eq!(stored[3], "number:12");
    assert_eq!(stored[6], "number:31");
    assert_eq!(
        stored[7],
        construct_params::Value::Length(mm(38_000)).stored()
    );
}
