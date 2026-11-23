//! Cut regions merged by material, judged on drawings made here from plain loops: two walls of one
//! material at a corner, a tee and an overlap become one region with no line between them, each
//! outline edge keeping its source; different materials stay apart; a hole stays a hole unless the
//! same material fills it; regions touching only at a point stay apart; and a crossing off the grid
//! is exact.

// The crate denies native arithmetic, lossy casts and unchecked indexing outside tests
// (Cargo.toml); a test's helpers may use them, and a broken helper must stop the test loudly.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests; the rules are for the crate's own code"
)]

use construct_categories::{CategoryId, FLOORS, WALLS};
use construct_drawing::drawing::{
    Drawing, Entity, Line, LineKind, Region, Shape, Source, ViewPoint,
};
use construct_drawing::merge::{MergedRegion, merge};
use construct_drawing::svg::PER_MM;
use construct_drawing::view::View;
use construct_materials::MaterialId;
use construct_params::Length;
use kit_core::Id;

/// A point `x`, `y` millimetres.
fn p(x: i128, y: i128) -> ViewPoint {
    ViewPoint::grid(x * PER_MM, y * PER_MM)
}

/// The rectangle from `(x0, y0)` to `(x1, y1)` millimetres, counter-clockwise from its least
/// corner: an outline. Its edges are bottom, right, top and left.
fn rect(x0: i128, y0: i128, x1: i128, y1: i128) -> Vec<ViewPoint> {
    vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
}

/// The same rectangle clockwise from its least corner: a hole. Its edges are left, top, right and
/// bottom.
fn hole(x0: i128, y0: i128, x1: i128, y1: i128) -> Vec<ViewPoint> {
    vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0)]
}

/// One element to draw: its id, category, material and loops (each from its least point).
struct El {
    id: &'static str,
    category: CategoryId,
    material: Option<&'static str>,
    loops: Vec<Vec<ViewPoint>>,
}

fn wall(id: &'static str, material: &'static str, loops: Vec<Vec<ViewPoint>>) -> El {
    El {
        id,
        category: WALLS,
        material: Some(material),
        loops,
    }
}

/// The role of loop `l`'s edge `e`: `l0e2` for the outline's third edge.
fn role(l: usize, e: usize) -> String {
    format!("l{l}e{e}")
}

/// A plan of `elements`: each edge of each loop a cut line named by its loop and place, and each
/// element's region.
fn drawing(elements: &[El]) -> Drawing {
    let mut lines = Vec::new();
    let mut regions = Vec::new();
    for el in elements {
        let id = Id::parse(el.id).unwrap();
        for (l, ring) in el.loops.iter().enumerate() {
            for e in 0..ring.len() {
                lines.push(Line {
                    kind: LineKind::Cut,
                    source: Source {
                        element: id.clone(),
                        entity: Entity::Face,
                        name: format!("{}-{}", el.id, role(l, e)),
                        role: role(l, e),
                    },
                    category: el.category,
                    shape: Shape::Segment {
                        start: ring[e],
                        end: ring[(e + 1) % ring.len()],
                    },
                });
            }
        }
        let mut loops = el.loops.clone();
        loops.sort();
        regions.push(Region {
            element: id,
            category: el.category,
            material: el.material.map(|m| MaterialId::parse(m).unwrap()),
            loops,
        });
    }
    Drawing::new(View::plan(Length::from_mm(1200)), lines, regions).unwrap()
}

/// A merged outline as (element, role, start, end).
fn outline(region: &MergedRegion) -> Vec<(String, String, ViewPoint, ViewPoint)> {
    region
        .outline
        .iter()
        .map(|line| {
            let Shape::Segment { start, end } = line.shape else {
                panic!("an arc in a merged outline");
            };
            assert_eq!(line.kind, LineKind::Cut);
            (
                line.source.element.as_str().to_owned(),
                line.source.role.clone(),
                start,
                end,
            )
        })
        .collect()
}

fn edge(
    element: &str,
    role: &str,
    a: ViewPoint,
    b: ViewPoint,
) -> (String, String, ViewPoint, ViewPoint) {
    (element.to_owned(), role.to_owned(), a, b)
}

fn ids(region: &MergedRegion) -> Vec<&str> {
    region.elements.iter().map(Id::as_str).collect()
}

/// Two concrete walls meeting at a corner are one region: one outline round both, the line where
/// they meet gone, the first wall's interior face shortened to where the second stands, and every
/// edge still named by its own wall's face.
#[test]
fn two_walls_of_one_material_at_a_corner_are_one_region() {
    let d = drawing(&[
        wall("wall-a", "concrete-regular", vec![rect(0, 0, 4000, 200)]),
        wall(
            "wall-b",
            "concrete-regular",
            vec![rect(3800, 200, 4000, 3200)],
        ),
    ]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 1);
    let m = &merged[0];
    assert_eq!(ids(m), ["wall-a", "wall-b"]);
    assert_eq!(m.category, WALLS);
    assert_eq!(
        m.material,
        Some(MaterialId::parse("concrete-regular").unwrap())
    );
    assert_eq!(
        m.loops,
        vec![vec![
            p(0, 0),
            p(4000, 0),
            p(4000, 200),
            p(4000, 3200),
            p(3800, 3200),
            p(3800, 200),
            p(0, 200),
        ]]
    );
    assert_eq!(
        outline(m),
        vec![
            edge("wall-a", "l0e0", p(0, 0), p(4000, 0)),
            edge("wall-a", "l0e1", p(4000, 0), p(4000, 200)),
            edge("wall-b", "l0e1", p(4000, 200), p(4000, 3200)),
            edge("wall-b", "l0e2", p(4000, 3200), p(3800, 3200)),
            edge("wall-b", "l0e3", p(3800, 3200), p(3800, 200)),
            edge("wall-a", "l0e2", p(3800, 200), p(0, 200)),
            edge("wall-a", "l0e3", p(0, 200), p(0, 0)),
        ]
    );
}

/// A wall standing against the middle of another of the same material: one region, the long
/// wall's interior face in two pieces either side of the tee.
#[test]
fn a_tee_of_one_material_is_one_region() {
    let d = drawing(&[
        wall("wall-a", "concrete-regular", vec![rect(0, 0, 4000, 200)]),
        wall(
            "wall-b",
            "concrete-regular",
            vec![rect(1900, 200, 2100, 3000)],
        ),
    ]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 1);
    assert_eq!(
        outline(&merged[0]),
        vec![
            edge("wall-a", "l0e0", p(0, 0), p(4000, 0)),
            edge("wall-a", "l0e1", p(4000, 0), p(4000, 200)),
            edge("wall-a", "l0e2", p(4000, 200), p(2100, 200)),
            edge("wall-b", "l0e1", p(2100, 200), p(2100, 3000)),
            edge("wall-b", "l0e2", p(2100, 3000), p(1900, 3000)),
            edge("wall-b", "l0e3", p(1900, 3000), p(1900, 200)),
            edge("wall-a", "l0e2", p(1900, 200), p(0, 200)),
            edge("wall-a", "l0e3", p(0, 200), p(0, 0)),
        ]
    );
}

/// Two walls of one material that run past each other: one region, its outline exactly where
/// their edges cross; where both walls' faces lie on one line, the first in the drawing's order
/// names it.
#[test]
fn overlapping_walls_of_one_material_merge_exactly() {
    let d = drawing(&[
        wall("wall-a", "concrete-regular", vec![rect(0, 0, 4000, 200)]),
        wall(
            "wall-b",
            "concrete-regular",
            vec![rect(3800, -100, 4000, 3200)],
        ),
    ]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 1);
    assert_eq!(
        outline(&merged[0]),
        vec![
            edge("wall-a", "l0e0", p(0, 0), p(3800, 0)),
            edge("wall-b", "l0e3", p(3800, 0), p(3800, -100)),
            edge("wall-b", "l0e0", p(3800, -100), p(4000, -100)),
            edge("wall-b", "l0e1", p(4000, -100), p(4000, 0)),
            edge("wall-a", "l0e1", p(4000, 0), p(4000, 200)),
            edge("wall-b", "l0e1", p(4000, 200), p(4000, 3200)),
            edge("wall-b", "l0e2", p(4000, 3200), p(3800, 3200)),
            edge("wall-b", "l0e3", p(3800, 3200), p(3800, 200)),
            edge("wall-a", "l0e2", p(3800, 200), p(0, 200)),
            edge("wall-a", "l0e3", p(0, 200), p(0, 0)),
        ]
    );
}

/// A wall of another material beside a concrete one stays its own region, and both keep the line
/// between them in their outlines (the picture draws it once).
#[test]
fn walls_of_different_materials_stay_apart_with_their_line() {
    let d = drawing(&[
        wall("wall-a", "concrete-regular", vec![rect(0, 0, 1000, 200)]),
        wall("wall-c", "plaster-gypsum", vec![rect(1000, 0, 2000, 200)]),
    ]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 2);
    let by = |m: &str| {
        merged
            .iter()
            .find(|r| r.material == Some(MaterialId::parse(m).unwrap()))
            .unwrap()
    };
    let (a, c) = (by("concrete-regular"), by("plaster-gypsum"));
    assert_eq!(ids(a), ["wall-a"]);
    assert_eq!(ids(c), ["wall-c"]);
    assert_eq!(a.loops, vec![rect(0, 0, 1000, 200)]);
    assert_eq!(c.loops, vec![rect(1000, 0, 2000, 200)]);
    assert!(outline(a).contains(&edge("wall-a", "l0e1", p(1000, 0), p(1000, 200))));
    assert!(outline(c).contains(&edge("wall-c", "l0e3", p(1000, 200), p(1000, 0))));
}

/// The same material in another category does not merge: a floor under a wall's cut stays apart.
#[test]
fn the_same_material_in_another_category_stays_apart() {
    let floor = El {
        id: "floor-a",
        category: FLOORS,
        material: Some("concrete-regular"),
        loops: vec![rect(1000, 0, 2000, 200)],
    };
    let d = drawing(&[
        wall("wall-a", "concrete-regular", vec![rect(0, 0, 1000, 200)]),
        floor,
    ]);
    assert_eq!(merge(&d).unwrap().len(), 2);
}

/// Regions with no material never merge, even touching.
#[test]
fn regions_with_no_material_never_merge() {
    let none = |id: &'static str, ring| El {
        id,
        category: WALLS,
        material: None,
        loops: vec![ring],
    };
    let d = drawing(&[
        none("wall-a", rect(0, 0, 1000, 200)),
        none("wall-b", rect(1000, 0, 2000, 200)),
    ]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 2);
    assert!(merged.iter().all(|m| m.material.is_none()));
}

/// A region with a hole keeps it.
#[test]
fn a_region_with_a_hole_keeps_it() {
    let d = drawing(&[wall(
        "pier",
        "concrete-regular",
        vec![rect(0, 0, 1000, 1000), hole(400, 400, 600, 600)],
    )]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 1);
    assert_eq!(
        merged[0].loops,
        vec![rect(0, 0, 1000, 1000), hole(400, 400, 600, 600)]
    );
    assert_eq!(merged[0].outline.len(), 8);
    assert_eq!(
        outline(&merged[0])[4..],
        [
            edge("pier", "l1e0", p(400, 400), p(400, 600)),
            edge("pier", "l1e1", p(400, 600), p(600, 600)),
            edge("pier", "l1e2", p(600, 600), p(600, 400)),
            edge("pier", "l1e3", p(600, 400), p(400, 400)),
        ]
    );
}

/// The same material filling the hole closes it: one region, no hole, both elements in it.
#[test]
fn the_same_material_filling_a_hole_closes_it() {
    let d = drawing(&[
        wall(
            "pier",
            "concrete-regular",
            vec![rect(0, 0, 1000, 1000), hole(400, 400, 600, 600)],
        ),
        wall("core", "concrete-regular", vec![rect(400, 400, 600, 600)]),
    ]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].loops, vec![rect(0, 0, 1000, 1000)]);
    assert_eq!(ids(&merged[0]), ["core", "pier"]);
}

/// Another material in the hole: the hole stays, and the core is its own region.
#[test]
fn another_material_in_a_hole_keeps_it() {
    let d = drawing(&[
        wall(
            "pier",
            "concrete-regular",
            vec![rect(0, 0, 1000, 1000), hole(400, 400, 600, 600)],
        ),
        wall("core", "plaster-gypsum", vec![rect(400, 400, 600, 600)]),
    ]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 2);
    let pier = merged.iter().find(|m| ids(m) == ["pier"]).unwrap();
    assert_eq!(pier.loops.len(), 2);
}

/// Two regions of one material that touch only at a corner stay two regions.
#[test]
fn regions_touching_only_at_a_point_stay_apart() {
    let d = drawing(&[
        wall("wall-a", "concrete-regular", vec![rect(0, 0, 200, 200)]),
        wall("wall-b", "concrete-regular", vec![rect(200, 200, 400, 400)]),
    ]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].loops, vec![rect(0, 0, 200, 200)]);
    assert_eq!(merged[1].loops, vec![rect(200, 200, 400, 400)]);
    assert_eq!(ids(&merged[0]), ["wall-a"]);
    assert_eq!(ids(&merged[1]), ["wall-b"]);
}

/// A sloped edge crossing a square's side off the grid: the crossing is the exact fraction.
#[test]
fn a_crossing_off_the_grid_is_exact() {
    let g = ViewPoint::grid;
    let square = vec![g(0, 0), g(2, 0), g(2, 2), g(0, 2)];
    let triangle = vec![g(1, 1), g(4, 2), g(1, 3)];
    let d = drawing(&[
        wall("wall-a", "concrete-regular", vec![square]),
        wall("wall-b", "concrete-regular", vec![triangle]),
    ]);
    let merged = merge(&d).unwrap();
    assert_eq!(merged.len(), 1);
    assert_eq!(
        merged[0].loops,
        vec![vec![
            g(0, 0),
            g(2, 0),
            ViewPoint::new(6, 4, 3).unwrap(),
            g(4, 2),
            g(1, 3),
            g(1, 2),
            g(0, 2),
        ]]
    );
}

/// The same drawing in any order merges to the same regions, and a second run gives them again.
#[test]
fn merging_is_deterministic() {
    let els = || {
        vec![
            wall("wall-a", "concrete-regular", vec![rect(0, 0, 4000, 200)]),
            wall(
                "wall-b",
                "concrete-regular",
                vec![rect(3800, 200, 4000, 3200)],
            ),
            wall("wall-c", "plaster-gypsum", vec![rect(-1000, 0, 0, 200)]),
        ]
    };
    let mut reversed = els();
    reversed.reverse();
    let a = merge(&drawing(&els())).unwrap();
    assert_eq!(a, merge(&drawing(&reversed)).unwrap());
    assert_eq!(a, merge(&drawing(&els())).unwrap());
}
