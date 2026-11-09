//! Piece G200's region finding (`geom_2d::region`) through its public interface: regions, holes,
//! dangles and cut edges against the polygonizer cases of JTS, points located in, out of and on
//! the linework, the island rules, exact doubled areas, and rooms from a wall plan.
//!
//! The cases of `polygonizer_cases` are JTS's `PolygonizerTest`
//! (<https://github.com/locationtech/jts/blob/5a0a73632216e3e9d277150336078ec423fd215f/modules/core/src/test/java/org/locationtech/jts/operation/polygonize/PolygonizerTest.java>),
//! their linework written as segments and their expected polygons as the shells, holes and
//! doubled areas they give. The cases are ported from JTS under EDL-1.0 (BSD-3-Clause), whose notice
//! they keep:
//! Copyright (c) 2016 Vivid Solutions; changed for Spare Kit: each WKT line string
//! written as its segments and each expected polygon as its area and hole count.
//! SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "test cases; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_2d::arrange::{Arrangement, arrange};
use geom_2d::region::{IslandRule, Located, Regions, polygonize};
use geom_num::carrier::Rational2;
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

/// The segments of line strings given as point lists.
fn lines(strings: &[&[(i64, i64)]]) -> Vec<(P2, P2)> {
    let mut out = Vec::new();
    for s in strings {
        for w in s.windows(2) {
            out.push((p(w[0].0, w[0].1), p(w[1].0, w[1].1)));
        }
    }
    out
}

fn built(strings: &[&[(i64, i64)]]) -> (Arrangement, Regions) {
    let arr = arrange(&lines(strings), &mut WorkMeter::new(Budget::UNLIMITED)).unwrap();
    let regions = polygonize(&arr).unwrap();
    (arr, regions)
}

/// Each region's doubled shell area and its holes' doubled areas, sorted.
fn shape(arr: &Arrangement, regions: &Regions) -> Vec<(i128, Vec<i128>)> {
    let mut out: Vec<(i128, Vec<i128>)> = regions
        .regions()
        .iter()
        .map(|r| {
            let shell = Regions::twice_area(arr, &r.shell).unwrap().unwrap();
            let mut holes: Vec<i128> = r
                .holes
                .iter()
                .map(|h| Regions::twice_area(arr, h).unwrap().unwrap())
                .collect();
            holes.sort_unstable();
            (shell, holes)
        })
        .collect();
    out.sort();
    out
}

fn located(arr: &Arrangement, regions: &Regions, x: i128, y: i128, w: i128) -> Located {
    regions.at(arr, &Rational2::new(x, y, w).unwrap()).unwrap()
}

#[test]
fn polygonizer_cases() {
    // testPolygonWithTouchingHole: a triangle with a triangular hole touching it at its apex.
    let (arr, r) = built(&[
        &[(100, 180), (20, 20), (160, 20), (100, 180)],
        &[(100, 180), (80, 60), (120, 60), (100, 180)],
    ]);
    // Twice the areas: the big triangle 22,400, the small 4,800, and the big less the small.
    assert_eq!(shape(&arr, &r), [(4_800, vec![]), (17_600, vec![])]);
    // testPolygonal1: a square with a square hole.
    let (arr, r) = built(&[
        &[(100, 100), (100, 300), (300, 300), (300, 100), (100, 100)],
        &[(150, 150), (150, 250), (250, 250), (250, 150), (150, 150)],
    ]);
    assert_eq!(shape(&arr, &r), [(20_000, vec![]), (80_000, vec![-20_000])]);
    assert!(r.dangles().is_empty() && r.cut_edges().is_empty());
    // testPolygonal2: a square holding a ring of four polygons around a small square.
    let (arr, r) = built(&[
        &[(100, 100), (100, 0), (0, 0), (0, 100), (100, 100)],
        &[(10, 10), (10, 30), (20, 30)],
        &[(20, 30), (30, 30), (30, 20)],
        &[(30, 20), (30, 10), (10, 10)],
        &[(40, 40), (40, 20), (30, 20)],
        &[(30, 20), (20, 20), (20, 30)],
        &[(20, 30), (20, 40), (40, 40)],
    ]);
    let s = shape(&arr, &r);
    assert_eq!(s.len(), 4);
    // The outer square holds the inner linework's outline as one hole: two 20 by 20 squares that
    // overlap by 10 by 10, twice 700.
    assert!(s.contains(&(20_000, vec![-1_400])));
    // testPolygonWithTouchingHoleAndNotch: three regions.
    let (arr, r) = built(&[
        &[(0, 0), (4, 0)],
        &[(4, 0), (5, 3)],
        &[(5, 3), (4, 6), (6, 6), (5, 3)],
        &[(5, 3), (6, 0)],
        &[(6, 0), (10, 0), (5, 10), (0, 0)],
        &[(4, 0), (6, 0)],
    ]);
    assert_eq!(shape(&arr, &r).len(), 3);
}

#[test]
fn dangles_and_cut_edges_bound_no_region() {
    // A square with a spike out of one corner, a spike into it, and an inner square tied to the
    // outer one by a segment.
    let (arr, r) = built(&[
        &[(0, 0), (100, 0), (100, 100), (0, 100), (0, 0)],
        &[(100, 100), (150, 150)],
        &[(0, 0), (20, 10)],
        &[(40, 40), (60, 40), (60, 60), (40, 60), (40, 40)],
        &[(60, 60), (100, 100)],
    ]);
    assert_eq!(r.dangles().len(), 2);
    assert_eq!(r.cut_edges().len(), 1);
    // The outer square less the inner, and the inner.
    assert_eq!(shape(&arr, &r), [(800, vec![]), (20_000, vec![-800])]);
    // Only dangles: no region at all.
    let (_, r) = built(&[&[(0, 0), (10, 0), (10, 10)], &[(10, 0), (20, 5)]]);
    assert_eq!(r.regions().len(), 0);
    assert_eq!(r.dangles().len(), 3);
}

#[test]
fn a_point_is_located_in_open_or_on_the_linework() {
    let (arr, r) = built(&[
        &[(0, 0), (100, 0), (100, 100), (0, 100), (0, 0)],
        &[(40, 40), (60, 40), (60, 60), (40, 60), (40, 40)],
    ]);
    let outer = r
        .regions()
        .iter()
        .position(|g| !g.holes.is_empty())
        .unwrap();
    let inner = 1 - outer;
    assert_eq!(located(&arr, &r, 10, 10, 1), Located::In(outer));
    assert_eq!(located(&arr, &r, 50, 50, 1), Located::In(inner));
    // A rational point just inside the hole's edge, and just outside it.
    assert_eq!(located(&arr, &r, 121, 150, 3), Located::In(inner));
    assert_eq!(located(&arr, &r, 119, 150, 3), Located::In(outer));
    assert_eq!(located(&arr, &r, 200, 50, 1), Located::Open);
    assert_eq!(located(&arr, &r, -1, 50, 1), Located::Open);
    assert_eq!(located(&arr, &r, 100, 50, 1), Located::On);
    assert_eq!(located(&arr, &r, 40, 50, 1), Located::On);
    assert_eq!(located(&arr, &r, 0, 0, 1), Located::On);
    // A point level with a vertex, so the ray passes through it.
    assert_eq!(located(&arr, &r, 20, 40, 1), Located::In(outer));
}

#[test]
fn island_rules_keep_or_skip_the_nested_regions() {
    // Three nested squares: the outer region, an island, and the island's island.
    let (arr, r) = built(&[
        &[(0, 0), (90, 0), (90, 90), (0, 90), (0, 0)],
        &[(20, 20), (70, 20), (70, 70), (20, 70), (20, 20)],
        &[(40, 40), (50, 40), (50, 50), (40, 50), (40, 40)],
    ]);
    let outer = match located(&arr, &r, 10, 10, 1) {
        Located::In(i) => i,
        other => panic!("{other:?}"),
    };
    let area = |rings: &[(Vec<u32>, Vec<Vec<u32>>)]| -> i128 {
        let mut total = 0;
        for (shell, holes) in rings {
            total += Regions::twice_area(&arr, shell).unwrap().unwrap();
            for h in holes {
                total += Regions::twice_area(&arr, h).unwrap().unwrap();
            }
        }
        total
    };
    // Ignore: the whole outer square, 2 x 8,100.
    assert_eq!(area(&r.filled(outer, IslandRule::Ignore).unwrap()), 16_200);
    // Outermost: the outer square less the island, 2 x (8,100 - 2,500).
    assert_eq!(
        area(&r.filled(outer, IslandRule::Outermost).unwrap()),
        11_200
    );
    // Parity: that, and the island's island again, 2 x 100.
    let parity = r.filled(outer, IslandRule::Parity).unwrap();
    assert_eq!(parity.len(), 2);
    assert_eq!(area(&parity), 11_400);
}

#[test]
fn rooms_come_from_a_wall_plan() {
    // A plan of three rooms: the walls' centre lines, one wall across the plan and one across its
    // right part, and a wall stub that ends short of the next wall.
    let (arr, r) = built(&[
        &[(0, 0), (1_000, 0), (1_000, 600), (0, 600), (0, 0)],
        &[(400, 0), (400, 600)],
        &[(400, 300), (1_000, 300)],
        // A wall stub that ends short of the outer wall: a dangle, not a room boundary.
        &[(200, 600), (200, 450)],
    ]);
    assert_eq!(r.regions().len(), 3);
    assert_eq!(r.dangles().len(), 1);
    assert_eq!(
        shape(&arr, &r),
        [(360_000, vec![]), (360_000, vec![]), (480_000, vec![])]
    );
    // A point in the left room, and one on the stub, which lies inside that room.
    assert!(matches!(located(&arr, &r, 100, 100, 1), Located::In(_)));
    assert!(matches!(located(&arr, &r, 200, 500, 1), Located::In(_)));
}
