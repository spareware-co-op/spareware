//! Kept pieces merged into faces, edges and shells, against closed forms and against what a closed
//! boundary must be, and the same numbers however the boxes fell (G129, 11.3).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short names, and a failure stops the test"
)]

use core::cmp::Ordering;
use geom_num::plane::{Plane, Tpi, normals_sign};
use geom_num::{NumError, P3, Sign};
use geom_poly::convex::{Piece, convex_pieces};
use geom_poly::merge::{
    Memo, Merge, MergeError, Merged, Outward, Rules, as_kept, both_ways, merge, merge_with,
    merge_with_reusing, section, select,
};
use geom_poly::plane::canonical;
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange};
use geom_poly::trace::{Classified, Polygon, TraceError};
use geom_poly::winding::{Term, Winding, WindingTest};
use geom_run::{Budget, Exec, WorkMeter};
use num_bigint::BigInt;
use proptest::prelude::*;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn up(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = 1;
    Plane::new(n, -i128::from(at)).unwrap()
}

fn down(axis: usize, at: i64) -> Plane {
    let mut n = [0i128; 3];
    n[axis] = -1;
    Plane::new(n, i128::from(at)).unwrap()
}

fn opposite(p: &Plane) -> Result<Plane, NumError> {
    Plane::new(p.n().map(|v| -v), -p.d())
}

#[derive(Clone, Copy, Debug)]
struct Block {
    low: [i64; 3],
    high: [i64; 3],
}

fn block(low: [i64; 3], high: [i64; 3]) -> Block {
    Block { low, high }
}

fn side(b: &Block, axis: usize, at: i64, upward: bool) -> Piece {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let (ul, uh) = (b.low[u], b.high[u]);
    let (vl, vh) = (b.low[v], b.high[v]);
    if upward {
        Piece {
            face: up(axis, at),
            bounds: vec![up(v, vl), down(u, uh), down(v, vh), up(u, ul)],
        }
    } else {
        Piece {
            face: down(axis, at),
            bounds: vec![up(u, ul), down(v, vh), down(u, uh), up(v, vl)],
        }
    }
}

fn scene(blocks: &[Block]) -> Vec<Polygon> {
    let mut polygons = Vec::new();
    for (entry, b) in blocks.iter().enumerate() {
        for axis in 0..3 {
            for (at, upward) in [(b.high[axis], true), (b.low[axis], false)] {
                polygons.push(Polygon {
                    piece: side(b, axis, at, upward),
                    entry: Some(entry as u32),
                });
            }
        }
    }
    polygons
}

fn classified(polygons: &[Polygon], leaf: usize) -> Vec<Classified> {
    let cell = around(polygons).unwrap();
    arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        polygons,
        leaf,
        &[],
        &mut meter(),
    )
    .unwrap()
}

fn kept(polygons: &[Polygon], test: &WindingTest, leaf: usize) -> Vec<Outward> {
    select(test, &classified(polygons, leaf)).unwrap()
}

fn merged(blocks: &[Block], test: &WindingTest, leaf: usize) -> Merged {
    let pieces = kept(&scene(blocks), test, leaf);
    let out = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&out, &pieces);
    out
}

/// Holds `m` to what the faces, edges and shells of a closed boundary of `pieces` must be.
fn closed(m: &Merged, pieces: &[Outward]) {
    for (at, fin) in m.fins.iter().enumerate() {
        let edge = &m.edges[fin.edge];
        assert!(edge.fins.contains(&at), "fin {at} missing from its edge");
        assert_eq!(fin.from.min(fin.to), edge.low);
        assert_eq!(fin.from.max(fin.to), edge.high);
        let plane = m.sheets[m.faces[fin.face].sheet].face;
        assert_eq!(pieces[fin.piece].piece.face, plane, "fin {at}: wrong piece");
        // Both ends lie on the fin's line: its face's plane and its bound.
        for end in [fin.from, fin.to] {
            let point = &m.vertices[end];
            for on in [plane, fin.bound] {
                assert_eq!(
                    on.side_of_tpi(point),
                    Ok(Sign::Zero),
                    "fin {at} leaves its line"
                );
            }
        }
    }
    for (at, edge) in m.edges.iter().enumerate() {
        assert!(edge.low < edge.high, "edge {at} runs down");
        assert_eq!(edge.fins.len() % 2, 0, "edge {at}");
        let turns = edge.fins.iter().cycle().skip(1);
        for (fin, next) in edge.fins.iter().zip(turns) {
            let rising = |f: &usize| m.fins[*f].from < m.fins[*f].to;
            assert_ne!(rising(fin), rising(next), "edge {at} does not alternate");
        }
    }
    let mut seen = vec![false; m.fins.len()];
    for (at, face) in m.faces.iter().enumerate() {
        assert!(m.shells[face.shell].faces.contains(&at));
        for ring in &face.rings {
            let turns = ring.iter().cycle().skip(1);
            for (fin, next) in ring.iter().zip(turns) {
                assert_eq!(m.fins[*fin].to, m.fins[*next].from, "face {at} ring breaks");
                assert_eq!(m.fins[*fin].face, at);
                assert!(!seen[*fin], "fin {fin} on two rings");
                seen[*fin] = true;
            }
        }
    }
    assert!(seen.iter().all(|s| *s), "a fin on no ring");
    for shell in &m.shells {
        assert_eq!(shell.outer, shell.holder.is_none());
        assert!(shell.holder.is_none_or(|h| m.shells[h].outer));
    }
}

/// Shells, faces, rings, edges and vertices.
fn counts(m: &Merged) -> [usize; 5] {
    let rings = m.faces.iter().map(|f| f.rings.len()).sum();
    [
        m.shells.len(),
        m.faces.len(),
        rings,
        m.edges.len(),
        m.vertices.len(),
    ]
}

const A: Block = Block {
    low: [0, 0, 0],
    high: [4, 4, 4],
};
const B: Block = Block {
    low: [2, 2, 2],
    high: [6, 6, 6],
};

#[test]
fn one_block_is_a_block_again_with_its_vertices_in_order() {
    let m = merged(&[A], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(counts(&m), [1, 6, 6, 12, 8]);
    assert_eq!(m.fins.len(), 24);
    let grid: Vec<[i64; 3]> = m
        .vertices
        .iter()
        .map(|v| {
            let p = v.to_grid().unwrap().value();
            [p.x.get(), p.y.get(), p.z.get()]
        })
        .collect();
    let mut sorted = grid.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(grid, sorted);
    assert_eq!(m.shells[0].faces, [0, 1, 2, 3, 4, 5]);
}

#[test]
fn overlapping_blocks_unite_into_one_shell() {
    let m = merged(&[A, B], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(counts(&m)[0], 1);
    assert_eq!(counts(&m)[1], 12);
}

#[test]
fn their_intersection_is_a_cube_and_their_difference_a_notched_one() {
    let both = merged(&[A, B], &WindingTest::intersect(2), LEAF_POLYGONS);
    assert_eq!(counts(&both), [1, 6, 6, 12, 8]);
    let less = merged(&[A, B], &WindingTest::subtract(0), LEAF_POLYGONS);
    assert_eq!(counts(&less), [1, 9, 9, 21, 14]);
}

#[test]
fn a_hole_right_through_makes_faces_with_two_rings() {
    let tool = block([1, 1, -1], [3, 3, 5]);
    let m = merged(&[A, tool], &WindingTest::subtract(0), LEAF_POLYGONS);
    assert_eq!(counts(&m), [1, 10, 12, 24, 16]);
}

#[test]
fn a_block_inside_a_block_leaves_a_cavity_held_by_it() {
    let tool = block([1, 1, 1], [3, 3, 3]);
    let m = merged(&[A, tool], &WindingTest::subtract(0), LEAF_POLYGONS);
    assert_eq!(counts(&m), [2, 12, 12, 24, 16]);
    assert!(m.shells[0].outer);
    assert_eq!(m.shells[1].holder, Some(0));
}

#[test]
fn a_cavity_is_held_by_the_innermost_shell_around_it() {
    let shells = [
        block([0, 0, 0], [20, 20, 20]),
        block([2, 2, 2], [18, 18, 18]),
        block([4, 4, 4], [16, 16, 16]),
        block([6, 6, 6], [14, 14, 14]),
    ];
    let test = WindingTest::new(vec![
        Term::Inside(0),
        Term::Inside(1),
        Term::Not,
        Term::And,
        Term::Inside(2),
        Term::Inside(3),
        Term::Not,
        Term::And,
        Term::Or,
    ])
    .unwrap();
    let m = merged(&shells, &test, LEAF_POLYGONS);
    assert_eq!(m.shells.len(), 4);
    // Lexically each shell's corners come four at a time, the widest shell's first.
    let depth = |s: usize| {
        let low = m.shells[s]
            .faces
            .iter()
            .flat_map(|f| m.faces[*f].rings.iter());
        let low = low.flatten().map(|fin| m.fins[*fin].from).min().unwrap();
        low / 4
    };
    let at_depth = |d: usize| (0..4).find(|s| depth(*s) == d).unwrap();
    assert!(m.shells[at_depth(0)].outer && m.shells[at_depth(2)].outer);
    assert_eq!(m.shells[at_depth(1)].holder, Some(at_depth(0)));
    assert_eq!(m.shells[at_depth(3)].holder, Some(at_depth(2)));
}

#[test]
fn a_cavity_is_held_by_the_innermost_shell_whatever_their_numbers() {
    // A tilted solid's first face sorts before a block's, so the inner solid is numbered first.
    let mut polygons = scene(&[
        block([0, 0, 0], [100, 100, 100]),
        block([10, 10, 10], [90, 90, 90]),
    ]);
    let solid = [[20, 20, 20], [80, 20, 21], [20, 80, 22], [21, 22, 80]];
    let cavity = [[30, 30, 30], [50, 30, 31], [30, 50, 32], [31, 32, 50]];
    for (entry, corners) in [(2, solid), (3, cavity)] {
        let turned = [corners[1], corners[0], corners[2], corners[3]];
        tetrahedron(corners, entry, &mut polygons)
            .or_else(|| tetrahedron(turned, entry, &mut polygons))
            .unwrap();
    }
    let test = WindingTest::new(vec![
        Term::Inside(0),
        Term::Inside(1),
        Term::Not,
        Term::And,
        Term::Inside(2),
        Term::Inside(3),
        Term::Not,
        Term::And,
        Term::Or,
    ])
    .unwrap();
    let pieces = kept(&polygons, &test, LEAF_POLYGONS);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&m, &pieces);
    assert_eq!(m.shells.len(), 4);
    let find = |outer: bool, sides: usize| {
        (0..4)
            .find(|s| m.shells[*s].outer == outer && m.shells[*s].faces.len() == sides)
            .unwrap()
    };
    assert!(find(true, 4) < find(true, 6));
    assert_eq!(m.shells[find(false, 4)].holder, Some(find(true, 4)));
    assert_eq!(m.shells[find(false, 6)].holder, Some(find(true, 6)));
}

#[test]
fn blocks_apart_make_two_outer_shells() {
    let far = block([9, 0, 0], [12, 4, 4]);
    let m = merged(&[A, far], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(m.shells.len(), 2);
    assert!(m.shells.iter().all(|s| s.outer));
}

#[test]
fn a_shared_face_disappears_and_flush_faces_merge() {
    let right = block([4, 0, 0], [8, 4, 4]);
    let m = merged(&[A, right], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(counts(&m), [1, 6, 6, 12, 8]);
}

#[test]
fn seams_stay_when_asked() {
    let right = block([4, 0, 0], [8, 4, 4]);
    let pieces = kept(&scene(&[A, right]), &WindingTest::unite(), LEAF_POLYGONS);
    let m = merge(&pieces, Merge::KeepSeams, &mut meter()).unwrap();
    closed(&m, &pieces);
    assert_eq!(counts(&m), [1, 10, 10, 20, 12]);
}

#[test]
fn blocks_that_share_only_an_edge_stay_two_shells_with_four_fins_on_it() {
    let other = block([4, 4, 0], [8, 8, 4]);
    let m = merged(&[A, other], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(m.shells.len(), 2);
    let widest = m.edges.iter().map(|e| e.fins.len()).max();
    assert_eq!(widest, Some(4));
}

#[test]
fn an_edge_ends_where_another_block_starts_along_it() {
    let other = block([4, 4, 1], [8, 8, 3]);
    let m = merged(&[A, other], &WindingTest::unite(), LEAF_POLYGONS);
    assert_eq!(m.shells.len(), 2);
    // Along the shared line: two fins, four where the blocks share it, two.
    let mut widths: Vec<usize> = m.edges.iter().map(|e| e.fins.len()).collect();
    widths.sort_unstable();
    assert_eq!(m.edges.len(), 12 + 12 + 1);
    assert_eq!(widths.last(), Some(&4));
    assert_eq!(widths.iter().filter(|w| **w == 4).count(), 1);
}

#[test]
fn a_ring_stays_in_its_corner_of_the_face_where_two_holes_touch() {
    let plate = block([0, 0, 0], [6, 6, 2]);
    let first = block([1, 1, -1], [3, 3, 3]);
    let second = block([3, 3, -1], [5, 5, 3]);
    let across = block([1, 3, -1], [3, 5, 3]);
    let over = block([3, 1, -1], [5, 3, 3]);
    let test = WindingTest::subtract(0);
    for tools in [[first, second], [across, over]] {
        let m = merged(&[plate, tools[0], tools[1]], &test, LEAF_POLYGONS);
        assert_eq!(m.shells.len(), 1);
        // The outer ring, and one ring past both holes that meets the corner twice.
        let top = m.faces.iter().find(|f| f.rings.len() == 2).unwrap();
        let inner = top.rings.iter().find(|r| r.len() == 8).unwrap();
        let mut corners: Vec<usize> = inner.iter().map(|f| m.fins[*f].from).collect();
        corners.sort_unstable();
        corners.dedup();
        assert_eq!(corners.len(), 7);
    }
}

/// A's boundary and a square polygon on the plane x = 2 over `low` to `high` in y and z, which
/// cuts and counts for nothing, as classified pieces: A's first, the polygon's after.
fn with_wall(low: [i64; 2], high: [i64; 2]) -> (Vec<Classified>, usize) {
    let wall = block([0, low[0], low[1]], [4, high[0], high[1]]);
    let mut polygons = scene(&[A]);
    let solid = polygons.len();
    polygons.push(Polygon {
        piece: side(&wall, 0, 2, true),
        entry: None,
    });
    (classified(&polygons, LEAF_POLYGONS), solid)
}

/// A's pieces of `found`, kept, and the wall's.
fn parts(found: &[Classified], solid: usize) -> (Vec<Outward>, Vec<Classified>) {
    let test = WindingTest::unite();
    let kept = select(&test, found).unwrap();
    let inside = |c: &&Classified| test.holds(&c.front) && test.holds(&c.back);
    let wall = found.iter().filter(|c| c.polygon >= solid).filter(inside);
    (kept, wall.cloned().collect())
}

#[test]
fn a_cut_holds_a_seam_where_it_lies_along_a_face() {
    let (found, solid) = with_wall([-1, -1], [5, 5]);
    let (kept, wall) = parts(&found, solid);
    let plain = merge(&kept, Merge::RemoveRedundant, &mut meter()).unwrap();
    assert_eq!(counts(&plain), [1, 6, 6, 12, 8]);
    let cuts: Vec<Piece> = wall.iter().map(|c| c.piece.clone()).collect();
    let rules = Rules {
        cuts: &cuts,
        open: false,
    };
    let seamed = merge_with(&kept, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
    closed(&seamed, &kept);
    assert_eq!(counts(&seamed), [1, 10, 10, 20, 12]);
}

#[test]
fn a_cut_that_stops_inside_a_face_leaves_it_whole() {
    let (found, solid) = with_wall([-1, -1], [2, 5]);
    let (kept, wall) = parts(&found, solid);
    let cuts: Vec<Piece> = wall.iter().map(|c| c.piece.clone()).collect();
    let rules = Rules {
        cuts: &cuts,
        open: false,
    };
    let m = merge_with(&kept, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
    closed(&m, &kept);
    // Only the face at y = 0 is cut right through; the two it stops in stay whole.
    assert_eq!(counts(&m), [1, 7, 7, 15, 10]);
}

#[test]
fn a_cut_nowhere_near_changes_nothing() {
    let kept_pieces = kept(&scene(&[A]), &WindingTest::unite(), LEAF_POLYGONS);
    let far = block([20, 20, 20], [24, 24, 24]);
    let cuts = [side(&far, 0, 24, true)];
    let rules = Rules {
        cuts: &cuts,
        open: false,
    };
    let m = merge_with(&kept_pieces, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
    assert_eq!(counts(&m), [1, 6, 6, 12, 8]);
}

#[test]
fn a_cut_on_a_face_holds_its_outline_and_not_where_the_boxes_parted_it() {
    // Gate 4's corpus (EMBER's figures 24 and 25, imprint and general fuse) found seams on the
    // boxes' planes: a cut lying in a face, parted by a box, held a seam along the part, so the
    // result had vertices on odd half steps and changed with the leaf size. A imprinted by a
    // square in its side at x = 4, and by a block standing on its side at y = 4, is A with that
    // side cut round the square, however small the boxes. The block's outline is held by its
    // face on y = 4 and its sides, whose sides along it run opposite ways: cuts of different
    // planes never cancel. Neither side is the first of A's planes in the sheets' order, so a
    // cut must find its own plane among them.
    let square = Block {
        low: [4, 1, 1],
        high: [4, 3, 3],
    };
    let standing = block([1, 4, 1], [3, 6, 3]);
    let mut with_square = scene(&[A]);
    with_square.push(Polygon {
        piece: side(&square, 0, 4, true),
        entry: None,
    });
    for polygons in [with_square, scene(&[A, standing])] {
        for leaf in [1, 2, LEAF_POLYGONS] {
            let found = classified(&polygons, leaf);
            // A's pieces, as an imprint keeps its target's; the others' are cuts.
            let test = WindingTest::new(vec![Term::Inside(0)]).unwrap();
            let kept = select(&test, &found).unwrap();
            let cuts: Vec<Piece> = found
                .iter()
                .filter(|c| c.polygon >= 6)
                .map(|c| c.piece.clone())
                .collect();
            let rules = Rules {
                cuts: &cuts,
                open: false,
            };
            let m = merge_with(&kept, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
            closed(&m, &kept);
            // The side is a ring round the square and the square: one more face and ring, the
            // square's four edges and corners.
            assert_eq!(counts(&m), [1, 7, 8, 16, 12], "leaf {leaf}");
            for v in &m.vertices {
                assert!(v.to_grid().unwrap().is_exact(), "leaf {leaf}: {v:?}");
            }
        }
    }
}

#[test]
fn a_wall_back_to_back_makes_two_cells_whichever_fin_comes_first() {
    let (found, solid) = with_wall([-1, -1], [5, 5]);
    let (kept, wall) = parts(&found, solid);
    let mut pieces = kept.clone();
    let cuts: Vec<Piece> = wall.iter().map(|c| c.piece.clone()).collect();
    for c in &wall {
        pieces.extend(both_ways(c).unwrap());
    }
    let rules = Rules {
        cuts: &cuts,
        open: false,
    };
    let m = merge_with(&pieces, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
    assert_eq!(m.shells.len(), 2);
    assert!(m.shells.iter().all(|s| s.outer && s.faces.len() == 6));
    closed(&m, &pieces);
    let four = m.edges.iter().filter(|e| e.fins.len() == 4).count();
    assert_eq!(four, 4);
    // The two sides of the wall lie in different shells.
    let back = kept.len();
    let shell_of = |at: usize| m.faces[m.piece_face[at]].shell;
    assert_ne!(shell_of(back), shell_of(back + 1));
    // Which fin an edge starts from follows the sheets, so turn them all over and go again.
    let turned: Vec<Outward> = pieces
        .iter()
        .map(|p| Outward {
            piece: Piece {
                face: mirrored(&p.piece.face),
                bounds: p.piece.bounds.iter().rev().map(mirrored).collect(),
            },
            ..p.clone()
        })
        .collect();
    let cuts: Vec<Piece> = cuts
        .iter()
        .map(|p| Piece {
            face: mirrored(&p.face),
            bounds: p.bounds.iter().rev().map(mirrored).collect(),
        })
        .collect();
    let rules = Rules {
        cuts: &cuts,
        open: false,
    };
    let m = merge_with(&turned, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
    assert_eq!(m.shells.len(), 2);
    closed(&m, &turned);
}

/// `p` mirrored through the plane x = 0.
fn mirrored(p: &Plane) -> Plane {
    let [x, y, z] = p.n();
    Plane::new([-x, y, z], p.d()).unwrap()
}

#[test]
fn a_wall_that_stops_short_has_one_cell_on_both_sides() {
    let (found, solid) = with_wall([-1, -1], [2, 5]);
    let (kept, wall) = parts(&found, solid);
    let mut pieces = kept.clone();
    let cuts: Vec<Piece> = wall.iter().map(|c| c.piece.clone()).collect();
    for c in &wall {
        pieces.extend(both_ways(c).unwrap());
    }
    let rules = Rules {
        cuts: &cuts,
        open: false,
    };
    let m = merge_with(&pieces, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
    let back = kept.len();
    let shell_of = |at: usize| m.faces[m.piece_face[at]].shell;
    assert_eq!(shell_of(back), shell_of(back + 1));
}

#[test]
fn an_open_merge_gives_a_sheet() {
    let square = |low: [i64; 3], high: [i64; 3]| {
        let polygons = vec![Polygon {
            piece: side(&block(low, high), 2, low[2], true),
            entry: None,
        }];
        as_kept(&classified(&polygons, LEAF_POLYGONS)[0])
    };
    let rules = Rules {
        cuts: &[],
        open: true,
    };
    let one = [square([0, 0, 0], [4, 4, 1])];
    let m = merge_with(&one, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
    assert_eq!(counts(&m), [1, 1, 1, 4, 4]);
    assert_eq!(m.shells[0].holder, None);
    assert!(m.shells[0].outer);
    assert_eq!(m.edges[0].fins.len(), 1);
    let two = [square([0, 0, 0], [4, 4, 1]), square([4, 0, 0], [8, 4, 1])];
    let m = merge_with(&two, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
    assert_eq!(counts(&m), [1, 1, 1, 4, 4]);
    let closed_rules = merge(&one, Merge::RemoveRedundant, &mut meter());
    assert_eq!(closed_rules, Err(MergeError::Open));
}

#[test]
fn faces_of_an_open_merge_that_share_an_edge_are_one_shell() {
    let pieces = kept(&scene(&[A]), &WindingTest::unite(), LEAF_POLYGONS);
    let rules = Rules {
        cuts: &[],
        open: true,
    };
    let opened: Vec<Outward> = pieces.iter().skip(1).cloned().collect();
    let m = merge_with(&opened, Merge::RemoveRedundant, rules, &mut meter()).unwrap();
    assert_eq!(counts(&m), [1, 5, 5, 12, 8]);
    assert_eq!(m.edges.iter().filter(|e| e.fins.len() == 1).count(), 4);
}

#[test]
fn a_doubled_piece_beside_its_neighbour_is_refused() {
    let right = block([4, 0, 0], [8, 4, 4]);
    let polygons = scene(&[A, right]);
    let mut pieces = kept(&polygons, &WindingTest::unite(), LEAF_POLYGONS);
    let top = pieces.iter().find(|p| p.piece.face == up(2, 4)).unwrap();
    let copy = top.clone();
    pieces.push(copy);
    assert_eq!(
        merge(&pieces, Merge::RemoveRedundant, &mut meter()),
        Err(MergeError::Overlap)
    );
}

#[test]
fn seams_follow_the_source_a_caller_names() {
    let right = block([4, 0, 0], [8, 4, 4]);
    let mut pieces = kept(&scene(&[A, right]), &WindingTest::unite(), LEAF_POLYGONS);
    for piece in &mut pieces {
        piece.source = 0;
        piece.front = Winding::new();
        piece.back = Winding::new();
    }
    let m = merge(&pieces, Merge::KeepSeams, &mut meter()).unwrap();
    assert_eq!(counts(&m), [1, 6, 6, 12, 8]);
}

/// A section runs where two owners meet: one test a scene (decision 32.78: the six sections
/// together were too long for one Miri gate part), each with all its checks.
mod a_section_runs_where_two_owners_meet {
    use super::*;

    /// The pieces of `blocks` classified at leaf size `leaf`, each with its owner.
    fn pieces(blocks: &[Block], leaf: usize) -> Vec<(Piece, usize)> {
        let polygons = scene(blocks);
        let owner = |c: &Classified| polygons[c.polygon].entry.unwrap_or(0) as usize;
        let found = classified(&polygons, leaf);
        found.iter().map(|c| (c.piece.clone(), owner(c))).collect()
    }

    fn two_blocks_at(leaf: usize) {
        let curves = section(&pieces(&[A, B], leaf), &mut meter()).unwrap();
        assert_eq!((curves.edges.len(), curves.vertices.len()), (6, 6));
        assert!(curves.edges.iter().all(|(low, high)| low < high));
    }

    #[test]
    fn two_blocks_at_a_leaf_of_one_polygon() {
        two_blocks_at(1);
    }

    #[test]
    fn two_blocks_at_the_usual_leaf() {
        two_blocks_at(LEAF_POLYGONS);
    }

    /// Two bars over one block: two stretches of one line, with a gap between them.
    #[test]
    fn two_bars_over_one_block() {
        let long = block([0, 0, 0], [10, 4, 4]);
        let bars = [
            long,
            block([1, -1, 2], [3, 5, 6]),
            block([6, -1, 2], [8, 5, 6]),
        ];
        let curves = section(&pieces(&bars, LEAF_POLYGONS), &mut meter()).unwrap();
        assert_eq!((curves.edges.len(), curves.vertices.len()), (16, 16));
    }

    #[test]
    fn flush_blocks_meet_around_their_contact() {
        let flush = block([4, 0, 0], [8, 4, 4]);
        let curves = section(&pieces(&[A, flush], LEAF_POLYGONS), &mut meter()).unwrap();
        assert_eq!(curves.edges.len(), 4, "around their contact, not across it");
    }

    #[test]
    fn blocks_apart_meet_nowhere() {
        let apart = block([9, 9, 9], [12, 12, 12]);
        let curves = section(&pieces(&[A, apart], LEAF_POLYGONS), &mut meter()).unwrap();
        assert_eq!((curves.edges.len(), curves.vertices.len()), (0, 0));
    }

    #[test]
    fn a_small_budget_stops_it() {
        let mut small = WorkMeter::new(Budget::new(3, u64::MAX));
        assert!(matches!(
            section(&pieces(&[A, B], LEAF_POLYGONS), &mut small),
            Err(MergeError::Stopped(_))
        ));
    }
}

#[test]
fn nothing_kept_is_nothing() {
    let far = block([9, 0, 0], [12, 4, 4]);
    let m = merged(&[A, far], &WindingTest::intersect(2), LEAF_POLYGONS);
    assert_eq!(counts(&m), [0, 0, 0, 0, 0]);
    assert!(m.sheets.is_empty() && m.fins.is_empty());
}

#[test]
fn the_same_numbers_come_out_however_the_boxes_fall() {
    let blocks = [A, B, block([1, -2, 1], [3, 9, 3])];
    let test = WindingTest::unite();
    let pieces = |leaf| {
        let polygons = scene(&blocks);
        let out = merge(
            &kept(&polygons, &test, leaf),
            Merge::RemoveRedundant,
            &mut meter(),
        );
        let mut out = out.unwrap();
        out.fins.iter_mut().for_each(|f| f.piece = 0);
        out
    };
    let whole = pieces(1000);
    for leaf in [1, 2, 4, 8, LEAF_POLYGONS] {
        assert_eq!(pieces(leaf).faces, whole.faces, "leaf {leaf}");
        assert_eq!(pieces(leaf).edges, whole.edges, "leaf {leaf}");
        assert_eq!(pieces(leaf).vertices, whole.vertices, "leaf {leaf}");
        assert_eq!(pieces(leaf).shells, whole.shells, "leaf {leaf}");
    }
}

#[test]
fn the_same_numbers_come_out_whatever_order_the_pieces_came_in() {
    let mut pieces = kept(&scene(&[A, B]), &WindingTest::unite(), 4);
    let once = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    pieces.reverse();
    let again = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&again, &pieces);
    assert_eq!(once.faces, again.faces);
    assert_eq!(once.edges, again.edges);
    assert_eq!(once.vertices, again.vertices);
}

#[test]
fn each_fin_names_the_piece_it_came_from() {
    let pieces = kept(&scene(&[A]), &WindingTest::unite(), LEAF_POLYGONS);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    let polygons = scene(&[A]);
    for fin in &m.fins {
        let source = pieces[fin.piece].polygon;
        assert_eq!(polygons[source].piece.face, pieces[fin.piece].piece.face);
    }
}

#[test]
fn select_keeps_each_overlap_once_and_turns_what_faces_in() {
    let polygons = scene(&[A]);
    let found = classified(&polygons, LEAF_POLYGONS);
    let inside = WindingTest::new(vec![Term::Inside(0), Term::Not]).unwrap();
    let turned = select(&inside, &found).unwrap();
    assert_eq!(turned.len(), found.len());
    for (out, from) in turned.iter().zip(&found) {
        assert!(out.turned);
        assert_eq!(out.piece.face, opposite(&from.piece.face).unwrap());
        assert_eq!(out.front, from.back);
        assert_eq!(out.back, from.front);
    }
    let same = select(&WindingTest::unite(), &found).unwrap();
    assert!(same.iter().all(|o| !o.turned));
    let right = block([4, 0, 0], [8, 4, 4]);
    let both = kept(&scene(&[A, right]), &WindingTest::unite(), LEAF_POLYGONS);
    assert!(
        both.iter().all(|o| o.piece.face != up(0, 4)),
        "inner wall kept"
    );
    let none = select(&WindingTest::intersect(2), &found).unwrap();
    assert_eq!(none.len(), 0);
}

#[test]
fn pieces_that_do_not_close_are_refused() {
    let mut pieces = kept(&scene(&[A]), &WindingTest::unite(), LEAF_POLYGONS);
    pieces.pop();
    assert_eq!(
        merge(&pieces, Merge::RemoveRedundant, &mut meter()),
        Err(MergeError::Open)
    );
}

#[test]
fn a_piece_turned_against_its_neighbours_is_refused() {
    let mut pieces = kept(&scene(&[A]), &WindingTest::unite(), LEAF_POLYGONS);
    let piece = &mut pieces[0].piece;
    piece.face = opposite(&piece.face).unwrap();
    piece.bounds.reverse();
    assert_eq!(
        merge(&pieces, Merge::RemoveRedundant, &mut meter()),
        Err(MergeError::Open)
    );
}

#[test]
fn a_piece_with_its_bounds_in_the_wrong_turn_is_refused() {
    let mut pieces = kept(&scene(&[A]), &WindingTest::unite(), LEAF_POLYGONS);
    pieces[0].piece.bounds.reverse();
    assert_eq!(
        merge(&pieces, Merge::RemoveRedundant, &mut meter()),
        Err(MergeError::Open)
    );
}

#[test]
fn pieces_that_overlap_are_refused() {
    let mut pieces = kept(&scene(&[A]), &WindingTest::unite(), LEAF_POLYGONS);
    let copy = pieces[0].clone();
    pieces.push(copy);
    assert_eq!(
        merge(&pieces, Merge::RemoveRedundant, &mut meter()),
        Err(MergeError::Overlap)
    );
}

#[test]
fn a_shell_turned_inside_out_is_refused() {
    let test = WindingTest::new(vec![Term::Inside(0), Term::Not]).unwrap();
    let pieces = kept(&scene(&[A]), &test, LEAF_POLYGONS);
    assert_eq!(
        merge(&pieces, Merge::RemoveRedundant, &mut meter()),
        Err(MergeError::Inside)
    );
}

#[test]
fn errors_say_what_was_wrong() {
    let stopped = WorkMeter::new(Budget::new(0, u64::MAX))
        .charge(1)
        .unwrap_err();
    assert_eq!(
        MergeError::Overlap.to_string(),
        "two pieces of one face overlap"
    );
    assert_eq!(MergeError::Open.to_string(), "kept pieces do not close");
    assert_eq!(
        MergeError::Inside.to_string(),
        "a shell faces inward with no shell around it"
    );
    assert_eq!(
        MergeError::Defect.to_string(),
        "the merge's parallel helper broke a rule"
    );
    assert_eq!(
        MergeError::from(stopped).to_string(),
        "the meter stopped the merge"
    );
    let num = MergeError::from(NumError::Degenerate);
    assert!(num.to_string().starts_with("pieces that cannot be merged"));
    let trace = TraceError::Num(NumError::Degenerate);
    assert_eq!(MergeError::from(trace).to_string(), trace.to_string());
}

#[test]
fn a_corner_that_is_no_point_is_reported_from_its_phase() {
    // A side parallel to the piece's face meets it nowhere: the corner there is no point, and the
    // merge says so with the arithmetic's own error, read from the phase that made the corners.
    let z = |at: i128| Plane::new([0, 0, 1], -at).unwrap();
    let piece = Piece {
        face: z(0),
        bounds: vec![
            Plane::new([1, 0, 0], 0).unwrap(),
            z(1),
            Plane::new([0, 1, 0], 0).unwrap(),
        ],
    };
    let outward = Outward {
        piece,
        polygon: 0,
        turned: false,
        source: 0,
        front: Winding::new(),
        back: Winding::new(),
    };
    assert!(matches!(
        merge(&[outward], Merge::RemoveRedundant, &mut meter()),
        Err(MergeError::Num(_))
    ));
}

#[test]
fn every_budget_stops_a_nested_merge_in_some_step_or_gives_the_same_answer() {
    // A box with a box-shaped cavity: two shells, so the merge also nests them. Every budget up
    // to the whole merge's, one unit apart, so each parallel step is stopped in turn.
    let tool = block([1, 1, 1], [3, 3, 3]);
    let pieces = kept(&scene(&[A, tool]), &WindingTest::subtract(0), LEAF_POLYGONS);
    let mut counted = meter();
    let whole = merge(&pieces, Merge::RemoveRedundant, &mut counted).unwrap();
    assert_eq!(whole.shells.len(), 2);
    let used = u64::try_from(counted.used().work).unwrap();
    for budget in 0..=used {
        let mut small = WorkMeter::new(Budget::new(budget, u64::MAX));
        match merge(&pieces, Merge::RemoveRedundant, &mut small) {
            Ok(m) => assert_eq!(m, whole, "budget {budget}"),
            Err(e) => assert!(
                matches!(
                    e,
                    MergeError::Stopped(_) | MergeError::Trace(TraceError::Stopped(_))
                ),
                "budget {budget}: {e:?}"
            ),
        }
    }
}

#[test]
fn any_budget_either_stops_the_merge_or_gives_the_same_answer() {
    let tool = block([1, 1, 1], [3, 3, 3]);
    let pieces = kept(&scene(&[A, tool]), &WindingTest::subtract(0), LEAF_POLYGONS);
    let whole = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    let mut stops = 0;
    for budget in (0..4000).step_by(7) {
        let mut small = WorkMeter::new(Budget::new(budget, u64::MAX));
        match merge(&pieces, Merge::RemoveRedundant, &mut small) {
            Ok(m) => assert_eq!(m, whole),
            Err(e) => {
                assert!(matches!(e, MergeError::Stopped(_) | MergeError::Trace(_)));
                stops += 1;
            }
        }
    }
    assert!(stops > 0);
}

/// A tetrahedron's faces, each the plane through three corners, outward.
fn tetrahedron(corner: [[i64; 3]; 4], entry: u32, polygons: &mut Vec<Polygon>) -> Option<()> {
    let at = |p: [i64; 3]| P3::new(p[0], p[1], p[2]).unwrap();
    let mut planes = Vec::new();
    for i in 0..4 {
        let [a, b, c] = [(i + 1) % 4, (i + 2) % 4, (i + 3) % 4].map(|k| corner[k]);
        let plane = Plane::through(at(a), at(b), at(c)).ok()?;
        planes.push(match plane.side(at(corner[i])).unwrap() {
            Sign::Zero => return None,
            Sign::Negative => plane,
            Sign::Positive => opposite(&plane).unwrap(),
        });
    }
    for i in 0..4 {
        let sides: Vec<Plane> = (1..4)
            .map(|k| opposite(&planes[(i + k) % 4]).unwrap())
            .collect();
        let mut pieces = convex_pieces(&planes[i], &[sides], &mut meter()).ok()?;
        polygons.push(Polygon {
            piece: pieces.pop()?,
            entry: Some(entry),
        });
    }
    Some(())
}

fn tilted(solids: &[[[i64; 3]; 4]], test: &WindingTest, leaf: usize) -> Option<Merged> {
    let mut polygons = Vec::new();
    for (entry, corners) in solids.iter().enumerate() {
        tetrahedron(*corners, entry as u32, &mut polygons)?;
    }
    let pieces = kept(&polygons, test, leaf);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&m, &pieces);
    named_by_its_faces(&m);
    Some(m)
}

#[test]
fn tilted_solids_near_the_edge_of_the_grid_merge_as_they_do_near_the_origin() {
    // Scaled by 2^32, the planes' normals reach about 2^77 and the points' homogeneous coordinates
    // pass what an i128 holds, so the merge compares them on its wide path, not in machine words.
    let a = [[0, 0, 0], [90, 10, 0], [10, 80, 10], [20, 10, 70]];
    let b = [[10, 10, 10], [80, 30, 20], [30, 90, 40], [40, 20, 90]];
    let scaled = |solid: [[i64; 3]; 4]| solid.map(|p| p.map(|v| v << 32));
    let test = WindingTest::unite();
    let near = tilted(&[a, b], &test, 1000).unwrap();
    for leaf in [1, 1000] {
        let far = tilted(&[scaled(a), scaled(b)], &test, leaf).unwrap();
        assert_eq!(counts(&far), counts(&near), "leaf {leaf}");
    }
}

#[test]
fn tilted_solids_meet_where_three_planes_do() {
    let a = [[0, 0, 0], [90, 10, 0], [10, 80, 10], [20, 10, 70]];
    let b = [[10, 10, 10], [80, 30, 20], [30, 90, 40], [40, 20, 90]];
    let test = WindingTest::unite();
    let whole = tilted(&[a, b], &test, 1000).unwrap();
    assert!(
        whole
            .vertices
            .iter()
            .any(|v| !v.to_grid().unwrap().is_exact())
    );
    for leaf in [1, 2, 4, 16] {
        let m = tilted(&[a, b], &test, leaf).unwrap();
        assert_eq!(m.vertices, whole.vertices, "leaf {leaf}");
        assert_eq!(m.edges, whole.edges, "leaf {leaf}");
    }
    for test in [WindingTest::intersect(2), WindingTest::subtract(0)] {
        tilted(&[a, b], &test, 4).unwrap();
    }
}

#[test]
fn a_point_on_a_split_is_not_named_by_it() {
    // The edge from (0, 0, -1) to (1, 2, 1) pierces the wide solid's top at (1/2, 1, 0), and the
    // first split, half way along x, runs through that point.
    let turned_out = |t: [[i64; 3]; 4]| {
        let fits = tetrahedron(t, 0, &mut Vec::new()).is_some();
        if fits { t } else { [t[1], t[0], t[2], t[3]] }
    };
    let wide = turned_out([[-20, -15, 0], [21, -15, 0], [0, 15, 0], [0, 0, -30]]);
    let spike = turned_out([[0, 0, -1], [1, 2, 1], [3, 0, 1], [0, 4, 1]]);
    let test = WindingTest::unite();
    let whole = tilted(&[wide, spike], &test, 1000).unwrap();
    for leaf in [1, 2, 4, 8] {
        let m = tilted(&[wide, spike], &test, leaf).unwrap();
        assert_eq!(m.vertices, whole.vertices, "leaf {leaf}");
    }
}

#[test]
fn a_point_where_four_planes_meet_is_named_the_same_whichever_way_they_reach_it() {
    // The cases `every_tilted_union_closes_and_the_boxes_do_not_show` met on wasm32, shrunk. In
    // the first, an edge of one solid crosses an edge of the other at (3, 37/2, 31/2), where four
    // planes meet, and the splits at y = 37/2 and z = 31/2 run through it. In one box the second
    // solid's face on one of the planes has no corner there, and the plane reaches the point only
    // as a bound of the face beside it, facing the other way; in the smallest boxes the face has
    // corners there too. The second meets four planes at (-1/2, 27, -6), on the split x = -1/2.
    let found = [
        vec![
            [[0, -24, 0], [24, 37, 9], [8, 8, 31], [-18, 0, 22]],
            [[27, 23, 29], [0, 0, -34], [-5, 17, 11], [-17, 39, 0]],
        ],
        vec![
            [[-9, 25, 3], [25, 33, -33], [0, 0, 0], [19, 35, -30]],
            [[0, 0, 0], [0, 0, -1], [0, 1, 0], [-20, 0, 0]],
            [[0, 0, 0], [-15, 25, -6], [0, -4, 0], [14, 29, -6]],
        ],
    ];
    let test = WindingTest::unite();
    for (at, solids) in found.iter().enumerate() {
        let whole = tilted(solids, &test, 1000).unwrap();
        for leaf in 1..10 {
            let m = tilted(solids, &test, leaf).unwrap();
            assert_eq!(m.vertices, whole.vertices, "case {at}, leaf {leaf}");
            assert_eq!(m.edges, whole.edges, "case {at}, leaf {leaf}");
        }
    }
}

#[test]
fn a_point_is_named_the_same_whatever_scale_its_planes_reach_it_at() {
    // A plane reaches a point at more than one scale where the faces beside a face write its
    // plane longer or shorter than the face does, as an operand's sides stood on its edges, or
    // another operand's face on the same plane, write it. Here the first case above, with the
    // second solid's bounds written three times longer than its faces: the plane that reaches
    // (3, 37/2, 31/2) as a face in the smallest boxes and only as a bound in one box reaches it
    // at one scale there and at another here. Facing up alone would name the point by the plane
    // three times longer in one box and not in another; read in canonical form, it is named the
    // same in every box, and the same as when every plane is written once.
    let solids = [
        [[0, -24, 0], [24, 37, 9], [8, 8, 31], [-18, 0, 22]],
        [[27, 23, 29], [0, 0, -34], [-5, 17, 11], [-17, 39, 0]],
    ];
    let mut polygons = Vec::new();
    for (entry, corners) in solids.iter().enumerate() {
        tetrahedron(*corners, entry as u32, &mut polygons).unwrap();
    }
    let longer = |p: &Plane| Plane::new(p.n().map(|v| v * 3), p.d() * 3).unwrap();
    let mixed: Vec<Polygon> = polygons
        .iter()
        .map(|polygon| Polygon {
            piece: Piece {
                face: polygon.piece.face,
                bounds: if polygon.entry == Some(1) {
                    polygon.piece.bounds.iter().map(longer).collect()
                } else {
                    polygon.piece.bounds.clone()
                },
            },
            entry: polygon.entry,
        })
        .collect();
    let test = WindingTest::unite();
    let merged = |polygons: &[Polygon], leaf: usize| {
        let pieces = kept(polygons, &test, leaf);
        let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
        closed(&m, &pieces);
        m
    };
    let whole = merged(&polygons, 1000);
    for leaf in (1..10).chain([1000]) {
        let m = merged(&mixed, leaf);
        assert_eq!(m.vertices, whole.vertices, "leaf {leaf}");
        assert_eq!(m.edges, whole.edges, "leaf {leaf}");
    }
}

/// `polygons` with every plane's coefficients multiplied by `k`: the same planes, written longer.
fn scaled(polygons: &[Polygon], k: i128) -> Vec<Polygon> {
    let times = |p: &Plane| Plane::new(p.n().map(|v| v * k), p.d() * k).unwrap();
    polygons
        .iter()
        .map(|polygon| Polygon {
            piece: Piece {
                face: times(&polygon.piece.face),
                bounds: polygon.piece.bounds.iter().map(times).collect(),
            },
            entry: polygon.entry,
        })
        .collect()
}

/// `plane` in canonical form.
fn reduced(plane: &Plane) -> Plane {
    canonical(plane).unwrap().plane
}

/// `planes` in plane order, each once.
fn in_plane_order(mut planes: Vec<Plane>) -> Vec<Plane> {
    planes.sort_unstable_by_key(|plane| (plane.n(), plane.d()));
    planes.dedup();
    planes
}

/// The first three of `planes`, in their order, that meet in one point.
fn first_meeting(planes: &[Plane]) -> Option<Tpi> {
    for (i, a) in planes.iter().enumerate() {
        for (j, b) in planes.iter().enumerate().skip(i + 1) {
            for c in planes.iter().skip(j + 1) {
                if normals_sign(a, b, c).unwrap() != Sign::Zero {
                    return Some(Tpi::meeting(*a, *b, *c).unwrap());
                }
            }
        }
    }
    None
}

/// The fins of `m` with an end at vertex `at`.
fn fins_at(m: &Merged, at: usize) -> impl Iterator<Item = &geom_poly::merge::Fin> {
    m.fins
        .iter()
        .filter(move |fin| fin.from == at || fin.to == at)
}

/// The planes of the faces of `m` that meet at vertex `at`, those with an edge ending there, in
/// canonical form and plane order.
fn faces_at(m: &Merged, at: usize) -> Vec<Plane> {
    let face = |fin: &geom_poly::merge::Fin| reduced(&m.sheets[m.faces[fin.face].sheet].face);
    in_plane_order(fins_at(m, at).map(face).collect())
}

/// The bounds the fins of `m` that end at vertex `at` stand on, in canonical form and plane order,
/// leaving out the planes of the faces there.
fn bounds_at(m: &Merged, at: usize) -> Vec<Plane> {
    let faces = faces_at(m, at);
    let bounds = fins_at(m, at).map(|fin| reduced(&fin.bound));
    in_plane_order(bounds.filter(|bound| !faces.contains(bound)).collect())
}

/// Whether `point` is on the grid.
fn on_grid(point: &Tpi) -> bool {
    point.to_grid().unwrap().is_exact()
}

/// Holds every vertex of `m`, a closed body, to its name: a grid point's three axis planes, and
/// for a point off the grid the lowest three planes of the faces that meet there, in plane order,
/// that meet in one point.
fn named_by_its_faces(m: &Merged) {
    for (at, point) in m.vertices.iter().enumerate() {
        let name = if on_grid(point) {
            let p = point.to_grid().unwrap().value();
            Tpi::meeting(up(0, p.x.get()), up(1, p.y.get()), up(2, p.z.get())).unwrap()
        } else {
            let faces = faces_at(m, at);
            first_meeting(&faces).unwrap_or_else(|| panic!("vertex {at}: no three of {faces:?}"))
        };
        assert_eq!(*point, name, "vertex {at}");
    }
}

#[test]
fn a_point_is_named_the_same_however_its_planes_are_written() {
    // A block's faces on odd whole steps, written with normals one long, and the same faces written
    // with normals two long and even offsets. Neither is a split of the boxes, 2x = odd, though the
    // one shares its odd offset with them and the other its normal. A grid point is named by its
    // axis planes. A spike's three edges up from (4, 4, 4) cross the block's top, z = 9, off the
    // grid at thirds and seventeenths, and the lowest three planes of the faces at each such point
    // name it, read in canonical form.
    let mut polygons = scene(&[block([1, 1, 1], [9, 9, 9])]);
    let spike = [[4, 4, 4], [6, 5, 19], [5, 7, 19], [3, 6, 21]];
    tetrahedron(spike, 1, &mut polygons).unwrap();
    for k in [1, 2] {
        let polygons = scaled(&polygons, k);
        for leaf in [1, 2, 4, 1000] {
            let pieces = kept(&polygons, &WindingTest::unite(), leaf);
            let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
            closed(&m, &pieces);
            let off = m.vertices.iter().filter(|v| !on_grid(v));
            assert_eq!(
                off.count(),
                3,
                "k {k}, leaf {leaf}: the spike's edges through the top"
            );
            named_by_its_faces(&m);
        }
    }
}

/// `polygons` with each bound leant over: the face's plane added to it `k` times, which is another
/// plane through the same line of the face, positive on the same side of it.
fn leant(polygons: &[Polygon], k: i128) -> Vec<Polygon> {
    polygons
        .iter()
        .map(|polygon| {
            let face = polygon.piece.face;
            let lean = |b: &Plane| {
                let n: Vec<i128> = b.n().iter().zip(face.n()).map(|(v, f)| v + k * f).collect();
                Plane::new([n[0], n[1], n[2]], b.d() + k * face.d()).unwrap()
            };
            Polygon {
                piece: Piece {
                    face,
                    bounds: polygon.piece.bounds.iter().map(lean).collect(),
                },
                entry: polygon.entry,
            }
        })
        .collect()
}

#[test]
fn a_closed_body_s_points_are_named_by_its_faces_however_its_bounds_are_written() {
    // Two tilted solids, each face's bounds the planes of the faces beside it, and the same solids
    // with every bound leant over about its edge: planes no face lies on, as a caller's sides
    // stood on a face's edges are. Some of them come before the faces in plane order, and they
    // pass through the corners of the pieces at every point of an edge. The points where the two
    // solids cross are named by their faces all the same, so both ways of writing the bounds give
    // one name, at every leaf size.
    let a = [[0, 0, 0], [90, 10, 0], [10, 80, 10], [20, 10, 70]];
    let b = [[10, 10, 10], [80, 30, 20], [30, 90, 40], [40, 20, 90]];
    let test = WindingTest::unite();
    let whole = tilted(&[a, b], &test, 1000).unwrap();
    assert!(whole.vertices.iter().any(|v| !on_grid(v)));
    let mut polygons = Vec::new();
    tetrahedron(a, 0, &mut polygons).unwrap();
    tetrahedron(b, 1, &mut polygons).unwrap();
    for k in [-3, 1, 5] {
        let leant = leant(&polygons, k);
        for leaf in [1, 2, 4, 1000] {
            let pieces = kept(&leant, &test, leaf);
            let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
            closed(&m, &pieces);
            assert_eq!(m.vertices, whole.vertices, "k {k}, leaf {leaf}");
            assert_eq!(m.edges, whole.edges, "k {k}, leaf {leaf}");
        }
    }
}

#[test]
fn a_point_is_not_named_by_a_plane_whose_face_stops_short_of_it() {
    // The case a seeded sweep of `every_tilted_union_closes_and_the_boxes_do_not_show` met (#908).
    // The first solid's edge from (-26, 0, 0) to (-26, 1, 9) lies in the plane x = -26 of the
    // third solid's face, and a face of the second solid crosses that edge off the grid, outside
    // the third solid's triangle. In one box the third solid's face is beside the second's and its
    // plane cuts the second's piece right across, through the point, so x = -26 is among the
    // planes of the corners there; in smaller boxes the two are apart and it is not. The point's
    // faces are the first solid's two on the edge and the second's that crosses it, in every box.
    let solids = [
        [[0, 0, 0], [0, 0, 1], [-26, 0, 0], [-26, 1, 9]],
        [[-25, 23, 0], [-38, -4, 8], [40, 0, 0], [0, 0, 0]],
        [[0, 0, 0], [-26, 0, 5], [-26, 19, 0], [-26, 5, 4]],
    ];
    let test = WindingTest::unite();
    let whole = tilted(&solids, &test, 1000).unwrap();
    // The edge is where x = -26 and z = 9y, the second a face of the first solid, and the face
    // of the second solid that crosses it outside the triangle is 92x + 100y + 487z = 0.
    let stray = Plane::new([1, 0, 0], 26).unwrap();
    let beside = Plane::new([0, 9, -1], 0).unwrap();
    let across = Plane::new([92, 100, 487], 0).unwrap();
    let on = |plane: &Plane, v: &Tpi| plane.side_of_tpi(v) == Ok(Sign::Zero);
    let on_the_edge = |v: &&Tpi| on(&stray, v) && on(&beside, v) && on(&across, v);
    let crossed: Vec<&Tpi> = whole.vertices.iter().filter(on_the_edge).collect();
    assert_eq!(crossed.len(), 1, "where the second solid crosses the edge");
    assert!(!on_grid(crossed[0]) && !crossed[0].planes().contains(&stray));
    assert!(crossed[0].planes().contains(&beside) && crossed[0].planes().contains(&across));
    for leaf in 1..10 {
        let m = tilted(&solids, &test, leaf).unwrap();
        assert_eq!(m.vertices, whole.vertices, "leaf {leaf}");
        assert_eq!(m.edges, whole.edges, "leaf {leaf}");
    }
}

#[test]
fn a_sheet_s_rim_is_named_by_its_face_and_then_the_bounds_of_its_fins() {
    // What a union keeps of one face of a tilted solid, merged as a sheet: its rim runs along the
    // face's own edges and along the other solid's faces, and turns off the grid where the other
    // solid's edges pierce the face and where its faces cross the face's edges. One face lies at
    // such a point, so no three planes of its faces meet there: the name is the face's plane and
    // then the lowest two bounds of the fins that end there, the same in every box.
    let a = [[0, 0, 0], [90, 10, 0], [10, 80, 10], [20, 10, 70]];
    let b = [[10, 10, 10], [80, 30, 20], [30, 90, 40], [40, 20, 90]];
    let mut polygons = Vec::new();
    tetrahedron(a, 0, &mut polygons).unwrap();
    tetrahedron(b, 1, &mut polygons).unwrap();
    let rules = Rules {
        cuts: &[],
        open: true,
    };
    let sheet = |polygon: usize, leaf: usize| {
        let mut pieces = kept(&polygons, &WindingTest::unite(), leaf);
        pieces.retain(|piece| piece.polygon == polygon);
        merge_with(&pieces, Merge::RemoveRedundant, rules, &mut meter()).unwrap()
    };
    let mut turns = 0;
    for (polygon, own) in polygons.iter().enumerate() {
        let whole = sheet(polygon, 1000);
        let face = reduced(&own.piece.face);
        for (at, point) in whole.vertices.iter().enumerate() {
            if on_grid(point) {
                continue;
            }
            turns += 1;
            assert_eq!(
                faces_at(&whole, at),
                [face],
                "polygon {polygon}, vertex {at}"
            );
            let mut planes = vec![face];
            planes.extend(bounds_at(&whole, at));
            let name = first_meeting(&planes).unwrap();
            assert_eq!(*point, name, "polygon {polygon}, vertex {at}");
        }
        for leaf in [1, 2, 4] {
            let m = sheet(polygon, leaf);
            assert_eq!(m.vertices, whole.vertices, "polygon {polygon}, leaf {leaf}");
            assert_eq!(m.edges, whole.edges, "polygon {polygon}, leaf {leaf}");
        }
    }
    assert!(turns > 0, "no rim turns off the grid");
}

#[test]
fn the_end_of_a_section_s_curve_is_named_by_its_two_faces_and_then_a_plane_of_its_corners() {
    // Each face of two tilted solids against the whole of the other: the curve where they meet stops
    // where the face does, off the grid on the face's edge. One edge of the curve ends there, on
    // two faces, and each fin's bound is the other's face, so neither the faces nor the bounds
    // name the point: the third plane is the lowest other plane through its corners, the face's
    // own bound along the edge. Where the curve turns, at an edge of the other solid, three faces
    // meet and name the point.
    let a = [[0, 0, 0], [90, 10, 0], [10, 80, 10], [20, 10, 70]];
    let b = [[10, 10, 10], [80, 30, 20], [30, 90, 40], [40, 20, 90]];
    let mut polygons = Vec::new();
    tetrahedron(a, 0, &mut polygons).unwrap();
    tetrahedron(b, 1, &mut polygons).unwrap();
    let cut = |face: usize, leaf: usize| {
        let found = classified(&polygons, leaf);
        // The face, and every face of the solid it is no face of.
        let other = |c: &Classified| (c.polygon >= 4) != (face >= 4);
        let pieces: Vec<(Piece, usize)> = found
            .iter()
            .filter(|c| c.polygon == face || other(c))
            .map(|c| (c.piece.clone(), usize::from(other(c))))
            .collect();
        (section(&pieces, &mut meter()).unwrap(), pieces)
    };
    let (mut ends, mut turns) = (0, 0);
    for (face, own) in polygons.iter().enumerate() {
        let (whole, pieces) = cut(face, 1000);
        for (at, point) in whole.vertices.iter().enumerate() {
            if on_grid(point) {
                continue;
            }
            let here = |(low, high): &&(usize, usize)| *low == at || *high == at;
            let edges: Vec<usize> = (0..whole.edges.len())
                .filter(|e| here(&&whole.edges[*e]))
                .collect();
            let faces = in_plane_order(
                edges
                    .iter()
                    .flat_map(|e| &whole.meets[*e])
                    .map(|piece| reduced(&pieces[*piece].0.face))
                    .collect(),
            );
            let [p, q, r] = *point.planes();
            if edges.len() == 1 {
                ends += 1;
                assert_eq!(faces, [p, q], "face {face}, vertex {at}");
                assert!(!faces.contains(&r) && r == reduced(&r));
                assert_eq!(r.side_of_tpi(point), Ok(Sign::Zero));
                let sides: Vec<Plane> = own.piece.bounds.iter().map(reduced).collect();
                assert!(sides.contains(&r), "face {face}, vertex {at}: {r:?}");
            } else {
                turns += 1;
                assert_eq!(
                    first_meeting(&faces),
                    Some(*point),
                    "face {face}, vertex {at}"
                );
            }
        }
        for leaf in [1, 2, 4] {
            let (curves, _) = cut(face, leaf);
            assert_eq!(curves.vertices, whole.vertices, "face {face}, leaf {leaf}");
            assert_eq!(curves.edges, whole.edges, "face {face}, leaf {leaf}");
        }
    }
    assert!(ends > 0 && turns > 0, "{ends} ends, {turns} turns");
}

#[test]
fn planes_through_one_line_are_passed_over_for_the_next_that_crosses_it() {
    // Two solids that touch along the z axis and nowhere else, one between x = 0 and y = 0, the
    // other between x = 10y and 10x = y, and a third whose face 4x + 4y + 27z = 144 crosses the
    // axis at z = 16/3. Below that point all four faces on the axis are in the union, and three
    // of them come before the crossing face in plane order: y = 0, x = 10y and x = 0 hold one line
    // and meet in no point, so the name is the first two and the crossing face.
    let solids = [
        [[0, 0, 0], [0, 0, 10], [10, 0, 5], [0, 10, 5]],
        [[0, 0, 2], [0, 0, 12], [-10, -1, 7], [-1, -10, 7]],
        [[9, 0, 4], [0, 9, 4], [-9, -9, 8], [0, 0, 20]],
    ];
    let plane = |n: [i128; 3], d: i128| Plane::new(n, d).unwrap();
    let on_the_axis = [
        plane([0, 1, 0], 0),
        plane([1, -10, 0], 0),
        plane([1, 0, 0], 0),
        plane([10, -1, 0], 0),
    ];
    let across = plane([4, 4, 27], -144);
    let test = WindingTest::unite();
    let whole = tilted(&solids, &test, 1000).unwrap();
    let on = |plane: &Plane, v: &Tpi| plane.side_of_tpi(v) == Ok(Sign::Zero);
    let crossed = |v: &Tpi| on(&across, v) && on_the_axis.iter().all(|p| on(p, v));
    let at = whole.vertices.iter().position(crossed).unwrap();
    let mut faces = on_the_axis.to_vec();
    faces.insert(3, across);
    assert_eq!(faces_at(&whole, at), faces);
    let name = [on_the_axis[0], on_the_axis[1], across];
    assert_eq!(*whole.vertices[at].planes(), name);
    for leaf in 1..10 {
        let m = tilted(&solids, &test, leaf).unwrap();
        assert_eq!(m.vertices, whole.vertices, "leaf {leaf}");
        assert_eq!(m.edges, whole.edges, "leaf {leaf}");
    }
}

#[test]
fn a_rim_on_a_block_s_face_takes_the_block_s_bounds_however_they_are_written() {
    // What a union keeps of each face of a block on odd whole steps, merged as a sheet, with a
    // tilted solid through the block: the sheet's rim turns off the grid where the solid's faces
    // cross the block's edges, and there the fins' bounds are the block's own planes, each square
    // to an axis. Written one long, or two long with even offsets, none is a split of the boxes
    // (2x = odd), so each is among the planes that name the point after the face's.
    let turned_out = |t: [[i64; 3]; 4]| {
        let fits = tetrahedron(t, 0, &mut Vec::new()).is_some();
        if fits { t } else { [t[1], t[0], t[2], t[3]] }
    };
    let wedge = turned_out([[-4, 3, 4], [14, 5, 6], [5, 15, 3], [4, 4, 15]]);
    let rules = Rules {
        cuts: &[],
        open: true,
    };
    let square = |plane: &Plane| plane.n().iter().filter(|v| **v != 0).count() == 1;
    for k in [1, 2] {
        let mut polygons = scaled(&scene(&[block([1, 1, 1], [9, 9, 9])]), k);
        tetrahedron(wedge, 1, &mut polygons).unwrap();
        let sheet = |polygon: usize, leaf: usize| {
            let mut pieces = kept(&polygons, &WindingTest::unite(), leaf);
            pieces.retain(|piece| piece.polygon == polygon);
            merge_with(&pieces, Merge::RemoveRedundant, rules, &mut meter()).unwrap()
        };
        let mut on_an_edge = 0;
        for (polygon, own) in polygons.iter().enumerate().take(6) {
            let whole = sheet(polygon, 1000);
            let face = reduced(&own.piece.face);
            for (at, point) in whole.vertices.iter().enumerate() {
                if on_grid(point) {
                    continue;
                }
                let bounds = bounds_at(&whole, at);
                on_an_edge += usize::from(bounds.iter().any(square));
                let mut planes = vec![face];
                planes.extend(bounds);
                let name = first_meeting(&planes).unwrap();
                assert_eq!(*point, name, "k {k}, polygon {polygon}, vertex {at}");
            }
            for leaf in [1, 2, 4] {
                let m = sheet(polygon, leaf);
                assert_eq!(
                    m.vertices, whole.vertices,
                    "k {k}, polygon {polygon}, leaf {leaf}"
                );
                assert_eq!(
                    m.edges, whole.edges,
                    "k {k}, polygon {polygon}, leaf {leaf}"
                );
            }
        }
        assert!(
            on_an_edge > 0,
            "k {k}: no rim turns on an edge of the block"
        );
    }
}

#[test]
fn a_split_through_the_end_of_a_section_s_curve_is_not_among_its_planes() {
    // The solids of `a_point_on_a_split_is_not_named_by_it`: the spike's edge from (0, 0, -1) to
    // (1, 2, 1) pierces the wide solid's top at (1/2, 1, 0), and the first split, 2x = 1, runs
    // through that point. Each face of the spike on that edge, against the whole wide solid,
    // meets it in a curve that ends there, so the point's third plane comes from its corners,
    // and in the smallest boxes the split is one of their planes. It is left out, and the point
    // is named as it is in one box, where no split is.
    let turned_out = |t: [[i64; 3]; 4]| {
        let fits = tetrahedron(t, 0, &mut Vec::new()).is_some();
        if fits { t } else { [t[1], t[0], t[2], t[3]] }
    };
    let wide = turned_out([[-20, -15, 0], [21, -15, 0], [0, 15, 0], [0, 0, -30]]);
    let spike = turned_out([[0, 0, -1], [1, 2, 1], [3, 0, 1], [0, 4, 1]]);
    let mut polygons = Vec::new();
    tetrahedron(wide, 0, &mut polygons).unwrap();
    tetrahedron(spike, 1, &mut polygons).unwrap();
    let cut = |face: usize, leaf: usize| {
        let found = classified(&polygons, leaf);
        let pieces: Vec<(Piece, usize)> = found
            .iter()
            .filter(|c| c.polygon == face || c.polygon < 4)
            .map(|c| (c.piece.clone(), usize::from(c.polygon < 4)))
            .collect();
        section(&pieces, &mut meter()).unwrap()
    };
    let split = Plane::new([2, 0, 0], -1).unwrap();
    let top = Plane::new([0, 0, 1], 0).unwrap();
    let mut ends = 0;
    for face in 4..8 {
        let whole = cut(face, 1000);
        for (at, point) in whole.vertices.iter().enumerate() {
            let pierced = split.side_of_tpi(point) == Ok(Sign::Zero) && !on_grid(point);
            let edges = whole
                .edges
                .iter()
                .filter(|(low, high)| *low == at || *high == at);
            if pierced && edges.count() == 1 {
                ends += 1;
                assert!(point.planes().contains(&top), "face {face}: {point:?}");
                assert!(!point.planes().contains(&split), "face {face}: {point:?}");
            }
        }
        for leaf in [1, 2, 4, 8] {
            let curves = cut(face, leaf);
            assert_eq!(curves.vertices, whole.vertices, "face {face}, leaf {leaf}");
            assert_eq!(curves.edges, whole.edges, "face {face}, leaf {leaf}");
        }
    }
    assert_eq!(
        ends, 2,
        "the two faces of the spike on the edge that pierces the top"
    );
}

#[test]
fn two_faces_that_meet_at_their_lowest_corner_keep_their_numbers() {
    let mut polygons = Vec::new();
    let up = [[0, 0, 0], [40, 10, 0], [40, 30, 0], [30, 20, 30]];
    let down = [[0, 0, 0], [40, -10, 0], [40, -30, 0], [30, -20, 30]];
    tetrahedron(up, 0, &mut polygons).unwrap();
    tetrahedron(down, 1, &mut polygons).unwrap();
    let mut pieces = kept(&polygons, &WindingTest::unite(), LEAF_POLYGONS);
    let once = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&once, &pieces);
    pieces.reverse();
    let again = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    assert_eq!(once.faces, again.faces);
    assert_eq!(once.edges, again.edges);
    assert_eq!(once.shells, again.shells);
}

/// Whether a point's X or w leaves `i128`: the merge then has no cells for it and compares it in
/// wide integers.
fn wide(point: &Tpi) -> bool {
    let (x, w) = point.homogeneous().unwrap();
    w.to_i128().is_none() || x.iter().any(|c| c.to_i128().is_none())
}

#[test]
fn far_tilted_solids_merge_in_wide_integers_and_the_boxes_do_not_show() {
    // The corners of two tilted solids at 2^32 times their size, each moved a few steps off that
    // grid: the planes through them then have coprime coefficients near 2^70, and the points where
    // three of them meet have coordinates past `i128`, so every comparison of them is wide.
    let s = 1i64 << 32;
    let far = |c: [[i64; 3]; 4], nudge: i64| {
        let mut k = 0;
        c.map(|p| {
            p.map(|v| {
                k += 1;
                v * s + nudge * k
            })
        })
    };
    let a = far([[0, 0, 0], [90, 10, 0], [10, 80, 10], [20, 10, 70]], 3);
    let b = far([[10, 10, 10], [80, 30, 20], [30, 90, 40], [40, 20, 90]], -5);
    // And the same mirrored in x, which turns the sign of every three planes' determinant.
    let mirror = |c: [[i64; 3]; 4]| c.map(|[x, y, z]| [-x, y, z]);
    // A union keeps every piece as it faces; a difference turns the second solid's round.
    for test in [WindingTest::unite(), WindingTest::subtract(0)] {
        for (a, b) in [(a, b), (mirror(a), mirror(b))] {
            let whole = tilted(&[a, b], &test, 1000).unwrap();
            assert!(whole.vertices.iter().any(wide));
            // Where faces cross, the points are off the grid, and named as such.
            assert!(
                whole
                    .vertices
                    .iter()
                    .any(|v| !v.to_grid().unwrap().is_exact())
            );
            for leaf in [1, 4, 16] {
                let m = tilted(&[a, b], &test, leaf).unwrap();
                assert_eq!(m.vertices, whole.vertices, "leaf {leaf}");
                assert_eq!(m.edges, whole.edges, "leaf {leaf}");
            }
            sorted_on_the_grid(&whole);
        }
    }
}

/// The points come in lexicographic order, as the wide comparison decides it.
fn sorted_on_the_grid(whole: &Merged) {
    for pair in whole.vertices.windows(2) {
        let (p, q) = (
            pair[0].to_grid().unwrap().value(),
            pair[1].to_grid().unwrap().value(),
        );
        assert!((p.x.get(), p.y.get(), p.z.get()) <= (q.x.get(), q.y.get(), q.z.get()));
    }
}

#[test]
fn two_tilted_solids_moved_to_the_frame_s_far_corner_merge_the_same() {
    // Small tilted solids, their edges a few thousand steps, so their normals are near 2^24; moved
    // by a grid vector to within 10^4 steps of the frame's corner, their planes' offsets pass
    // `i64` while their normals and meeting points still fit `i128`. Moving every input by a grid
    // vector moves every vertex by it and changes nothing else.
    let a = [
        [0, 0, 0],
        [4999, 701, 3],
        [607, 4801, 903],
        [1301, 599, 4703],
    ];
    let b = [
        [701, 803, 599],
        [5303, 2101, 1409],
        [2003, 5897, 2609],
        [2707, 1301, 5801],
    ];
    let t = (1i64 << 40) - 7001;
    let moved = |c: [[i64; 3]; 4]| c.map(|p| p.map(|v| v + t));
    let test = WindingTest::unite();
    let near = tilted(&[a, b], &test, LEAF_POLYGONS).unwrap();
    let far = tilted(&[moved(a), moved(b)], &test, LEAF_POLYGONS).unwrap();
    assert!(
        far.sheets
            .iter()
            .any(|s| i64::try_from(s.face.d()).is_err())
    );
    assert_eq!(near.vertices.len(), far.vertices.len());
    for (p, q) in near.vertices.iter().zip(&far.vertices) {
        let (p, q) = (p.to_grid().unwrap(), q.to_grid().unwrap());
        assert_eq!(p.is_exact(), q.is_exact());
        let (p, q) = (p.value(), q.value());
        assert_eq!(
            [p.x.get() + t, p.y.get() + t, p.z.get() + t],
            [q.x.get(), q.y.get(), q.z.get()]
        );
    }
    let ends = |m: &Merged| m.edges.iter().map(|e| (e.low, e.high)).collect::<Vec<_>>();
    assert_eq!(ends(&near), ends(&far));
    assert_eq!(near.faces.len(), far.faces.len());
}

#[test]
fn a_point_off_the_grid_is_named_by_the_lowest_three_planes_of_its_faces() {
    // A box on odd steps crossed by a tilted solid: points off the grid on the box's faces and
    // the solid's. Each is named by three planes of the faces that meet there, in plane order,
    // never a corner's own three.
    let mut polygons = scene(&[block([1, 1, 1], [5, 5, 5])]);
    tetrahedron(
        [[-2, 0, 2], [7, 1, 3], [2, 8, 2], [3, 2, 9]],
        1,
        &mut polygons,
    )
    .unwrap();
    let key = |p: &Plane| (p.n(), p.d());
    for leaf in [1, 4, LEAF_POLYGONS] {
        let pieces = kept(&polygons, &WindingTest::unite(), leaf);
        let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
        closed(&m, &pieces);
        let mut seen = false;
        for v in m
            .vertices
            .iter()
            .filter(|v| !v.to_grid().unwrap().is_exact())
        {
            seen = true;
            let [p, q, r] = v.planes();
            assert!(key(p) < key(q) && key(q) < key(r), "leaf {leaf}: {v:?}");
        }
        assert!(seen, "leaf {leaf}: no point off the grid");
        named_by_its_faces(&m);
    }
}

#[test]
fn a_line_read_in_words_and_in_wide_integers_is_one_line() {
    // A box at the frame's far corner and a wedge whose face holds one of the box's edges. The
    // box's own faces fit machine words, so that edge's line is read in words from them; the
    // wedge's face has an offset past `i64`, so the same line is read in wide integers from it.
    // Both readings must be one line, or the edge's fins would not meet.
    let t = (1i64 << 40) - (1 << 27);
    let at = |p: [i64; 3]| p.map(|v| v + t);
    let mut polygons = scene(&[block(at([0, 0, 0]), at([4, 4, 4]))]);
    let wedge = [
        at([0, 0, 0]),
        at([0, 0, 4]),
        at([-33_554_393, 16_777_259, 1]),
        at([16_777_213, -33_554_467, 3]),
    ];
    tetrahedron(wedge, 1, &mut polygons).unwrap();
    for leaf in [1, LEAF_POLYGONS] {
        let pieces = kept(&polygons, &WindingTest::unite(), leaf);
        assert!(
            pieces
                .iter()
                .any(|o| i64::try_from(o.piece.face.d()).is_err())
        );
        let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
        closed(&m, &pieces);
    }
}

#[test]
fn two_faces_in_a_line_that_touch_at_a_vertex_keep_their_own_edges() {
    let mut polygons = Vec::new();
    let one = [[0, 0, 0], [10, 0, 0], [2, 8, 0], [4, 3, 9]];
    let two = [[10, 0, 0], [20, 0, 0], [18, 8, 0], [15, 3, 9]];
    tetrahedron(one, 0, &mut polygons).unwrap();
    tetrahedron(two, 1, &mut polygons).unwrap();
    let pieces = kept(&polygons, &WindingTest::unite(), LEAF_POLYGONS);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&m, &pieces);
    assert_eq!(counts(&m), [2, 8, 8, 12, 7]);
}

#[test]
fn an_edge_breaks_at_each_corner_that_rests_on_it() {
    let mut polygons = scene(&[A]);
    let one = [[1, 4, 4], [1, 8, 5], [2, 9, 9], [0, 5, 9]];
    let two = [[3, 4, 4], [3, 8, 5], [4, 9, 9], [2, 5, 9]];
    tetrahedron(one, 1, &mut polygons).unwrap();
    tetrahedron(two, 2, &mut polygons).unwrap();
    let pieces = kept(&polygons, &WindingTest::unite(), LEAF_POLYGONS);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&m, &pieces);
    assert_eq!(counts(&m), [3, 14, 14, 26, 16]);
}

#[test]
fn an_edge_breaks_where_a_corner_rests_on_it() {
    let mut polygons = scene(&[A]);
    let tip = [[2, 4, 4], [2, 8, 5], [3, 9, 9], [1, 5, 9]];
    tetrahedron(tip, 1, &mut polygons).unwrap();
    let pieces = kept(&polygons, &WindingTest::unite(), LEAF_POLYGONS);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&m, &pieces);
    assert_eq!(counts(&m), [2, 10, 10, 19, 12]);
}

/// The merge of `pieces` from what the merge of `before` kept, under `budget`, checked against
/// the merge without it: the same body or error, the same memo, the same work charged (G139). How
/// many pieces it took from the memo, with its result.
fn merged_again(
    before: &[Outward],
    pieces: &[Outward],
    budget: Option<u64>,
) -> (Result<Merged, MergeError>, usize) {
    merged_again_with((before, &[]), (pieces, &[]), budget)
}

/// [`merged_again`], each merge under its cuts.
fn merged_again_with(
    (before, before_cuts): (&[Outward], &[Piece]),
    (pieces, cuts): (&[Outward], &[Piece]),
    budget: Option<u64>,
) -> (Result<Merged, MergeError>, usize) {
    let cap = budget.map_or(Budget::UNLIMITED, |work| Budget::new(work, u64::MAX));
    let mode = Merge::RemoveRedundant;
    let rules = |cuts| Rules { cuts, open: false };
    let memo = merge_with_reusing(before, mode, rules(before_cuts), None, &mut meter());
    let memo = memo.map(|m| m.1);
    let mut fresh_meter = WorkMeter::new(cap);
    let fresh = merge_with_reusing(pieces, mode, rules(cuts), None, &mut fresh_meter);
    let mut again_meter = WorkMeter::new(cap);
    let reuse = memo.as_ref().ok();
    let again = merge_with_reusing(pieces, mode, rules(cuts), reuse, &mut again_meter);
    assert_eq!(again, fresh);
    assert_eq!(again_meter.used(), fresh_meter.used());
    let fresh_alone = merge_with(pieces, mode, rules(cuts), &mut WorkMeter::new(cap));
    assert_eq!(fresh.as_ref().map(|m| &m.0).ok(), fresh_alone.as_ref().ok());
    let shared = match (&again, reuse) {
        (Ok((_, now)), Some(memo)) => now.shared_with(memo),
        _ => 0,
    };
    (again.map(|m| m.0), shared)
}

#[test]
fn a_merge_from_another_merges_memo_is_the_merge_without_it() {
    // A wall with three openings, the last moved along it: most pieces stay as they were.
    let wall = block([0, 0, 0], [40, 2, 10]);
    let opening = |at: i64| block([at, -1, 2], [at + 4, 3, 8]);
    let scene_with = |last: i64| scene(&[wall, opening(4), opening(14), opening(last)]);
    let test = WindingTest::subtract(0);
    let before = kept(&scene_with(26), &test, LEAF_POLYGONS);
    let after = kept(&scene_with(30), &test, LEAF_POLYGONS);
    let (m, shared) = merged_again(&before, &after, None);
    closed(&m.unwrap(), &after);
    assert!(shared > after.len() / 2, "took {shared} of {}", after.len());
    // Everything again, nothing again, and a memo of nothing.
    let (_, all) = merged_again(&after, &after, None);
    assert_eq!(all, after.len());
    let (_, none) = merged_again(&kept(&scene(&[A]), &test, 1), &after, None);
    assert_eq!(none, 0);
    let (m, _) = merged_again(&[], &after, None);
    assert!(m.is_ok());
    assert!(Memo::default().is_empty());
    // A doubled piece takes nothing, and is refused as without.
    let mut doubled = after.clone();
    doubled.push(after[0].clone());
    let (m, shared) = merged_again(&after, &doubled, None);
    assert!(m.is_err());
    assert_eq!(shared, 0);
    let (m, _) = merged_again(&after, &after, None);
    let memo = merge_with_reusing(
        &after,
        Merge::RemoveRedundant,
        Rules::default(),
        None,
        &mut meter(),
    );
    let (again, memo) = memo.unwrap();
    assert_eq!(again, m.unwrap());
    assert_eq!(memo.len(), after.len());
    assert!(!memo.is_empty());
}

#[test]
fn a_piece_held_twice_starts_the_sorts_from_nothing() {
    // The same cut twice is held twice, and the memo's orders hold one place for both: the merge
    // takes the parts but not the orders, and is the merge without the memo. A memo that holds a
    // piece twice gives its parts once.
    let (found, solid) = with_wall([-1, -1], [5, 5]);
    let (kept, wall) = parts(&found, solid);
    let cuts: Vec<Piece> = wall.iter().map(|c| c.piece.clone()).collect();
    let twice: Vec<Piece> = cuts.iter().chain(&cuts).cloned().collect();
    let (m, shared) = merged_again_with((&kept, &cuts), (&kept, &twice), None);
    assert!(m.is_ok());
    assert_eq!(shared, kept.len() + twice.len());
    let (m, shared) = merged_again_with((&kept, &twice), (&kept, &cuts), None);
    assert!(m.is_ok());
    assert_eq!(shared, kept.len() + cuts.len());
}

#[test]
fn a_memo_starts_the_sorts_but_the_budget_stops_them_where_it_would() {
    let tool = block([1, 1, 1], [3, 3, 3]);
    let moved = block([1, 1, 2], [3, 3, 4]);
    let before = kept(&scene(&[A, tool]), &WindingTest::subtract(0), LEAF_POLYGONS);
    let after = kept(
        &scene(&[A, moved]),
        &WindingTest::subtract(0),
        LEAF_POLYGONS,
    );
    let mut stops = 0;
    for budget in (0..4000).step_by(7) {
        let (m, _) = merged_again(&before, &after, Some(budget));
        stops += usize::from(m.is_err());
    }
    assert!(stops > 0);
}

#[test]
fn faces_that_look_like_splits_name_the_points_on_them() {
    // A block whose faces lie on odd whole steps, its planes written with normals one long, then
    // two long with even offsets, and a tilted solid whose edges cross those faces off the grid.
    // The boxes split on half steps, 2x = odd; neither way of writing a face on x = 1 is one, and
    // a face is among the planes a point on it is named by however it is written.
    let turned_out = |t: [[i64; 3]; 4]| {
        let fits = tetrahedron(t, 0, &mut Vec::new()).is_some();
        if fits { t } else { [t[1], t[0], t[2], t[3]] }
    };
    let wedge = turned_out([[-4, 3, 4], [14, 5, 6], [5, 15, 3], [4, 4, 15]]);
    for k in [1, 2] {
        let mut polygons = scaled(&scene(&[block([1, 1, 1], [9, 9, 9])]), k);
        tetrahedron(wedge, 1, &mut polygons).unwrap();
        for leaf in [1, 4, 1000] {
            let pieces = kept(&polygons, &WindingTest::unite(), leaf);
            let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
            closed(&m, &pieces);
            let block_s = |plane: &Plane| plane.n().iter().filter(|v| **v != 0).count() == 1;
            let on_the_block = m
                .vertices
                .iter()
                .filter(|v| !on_grid(v) && v.planes().iter().any(block_s));
            assert!(on_the_block.count() > 0, "k {k}, leaf {leaf}");
            named_by_its_faces(&m);
        }
    }
}

/// The determinant of three rows.
fn det(a: &[BigInt; 3], b: &[BigInt; 3], c: &[BigInt; 3]) -> BigInt {
    &a[0] * (&b[1] * &c[2] - &b[2] * &c[1]) - &a[1] * (&b[0] * &c[2] - &b[2] * &c[0])
        + &a[2] * (&b[0] * &c[1] - &b[1] * &c[0])
}

/// Where a point's three planes meet, exactly, by Cramer's rule in big integers: X and w with w
/// positive, the point being X / w. An oracle of its own, sharing no arithmetic with the merge.
fn exact_point(t: &Tpi) -> ([BigInt; 3], BigInt) {
    let rows = t.planes().map(|p| p.n().map(BigInt::from));
    let minus_d = t.planes().map(|p| -BigInt::from(p.d()));
    let with = |i: usize| {
        let mut r = rows.clone();
        for (row, v) in r.iter_mut().zip(&minus_d) {
            row[i].clone_from(v);
        }
        det(&r[0], &r[1], &r[2])
    };
    let (x, w) = (
        [with(0), with(1), with(2)],
        det(&rows[0], &rows[1], &rows[2]),
    );
    assert!(w != BigInt::from(0), "{t:?} is no point");
    if w < BigInt::from(0) {
        (x.map(|v| -v), -w)
    } else {
        (x, w)
    }
}

/// a / b against c / d, b and d positive.
fn ratio_cmp(a: &BigInt, b: &BigInt, c: &BigInt, d: &BigInt) -> Ordering {
    (a * d).cmp(&(c * b))
}

/// Two points lexicographically, exactly.
fn point_cmp(p: &([BigInt; 3], BigInt), q: &([BigInt; 3], BigInt)) -> Ordering {
    (0..3)
        .map(|i| ratio_cmp(&p.0[i], &p.1, &q.0[i], &q.1))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// Holds `m`'s vertices to their promise, lexicographic order, checked exactly: each strictly
/// after the one before, so no point is listed twice.
fn in_point_order(m: &Merged) {
    let points: Vec<_> = m.vertices.iter().map(exact_point).collect();
    for (at, pair) in points.windows(2).enumerate() {
        assert_eq!(
            point_cmp(&pair[0], &pair[1]),
            Ordering::Less,
            "vertices {at} and {}",
            at + 1
        );
    }
}

/// The line where two planes meet, as the merge orders lines: which component of its direction
/// `n_p` × `n_q` is the first not zero, that component made positive, then its direction and
/// moment `d_p` `n_q` − `d_q` `n_p` over it.
fn exact_line(p: &Plane, q: &Plane) -> (usize, [BigInt; 6], BigInt) {
    let (a, b) = (p.n().map(BigInt::from), q.n().map(BigInt::from));
    let (da, db) = (BigInt::from(p.d()), BigInt::from(q.d()));
    let mut c = [
        &a[1] * &b[2] - &a[2] * &b[1],
        &a[2] * &b[0] - &a[0] * &b[2],
        &a[0] * &b[1] - &a[1] * &b[0],
        &da * &b[0] - &db * &a[0],
        &da * &b[1] - &db * &a[1],
        &da * &b[2] - &db * &a[2],
    ];
    let first = (0..3).find(|i| c[*i] != BigInt::from(0)).unwrap();
    if c[first] < BigInt::from(0) {
        c = c.map(|v| -v);
    }
    let over = c[first].clone();
    (first, c, over)
}

/// The line of each edge of `m`, from its first fin's face and bound.
fn edge_lines(m: &Merged) -> Vec<(usize, [BigInt; 6], BigInt)> {
    m.edges
        .iter()
        .map(|e| {
            let fin = &m.fins[e.fins[0]];
            let face = m.sheets[m.faces[fin.face].sheet].face;
            exact_line(&face, &fin.bound)
        })
        .collect()
}

/// Holds `m`'s edges to coming line by line in the order of their lines, checked exactly, and
/// each edge's fins to lie on its line.
fn in_line_order(m: &Merged) {
    let lines = edge_lines(m);
    let cmp = |p: &(usize, [BigInt; 6], BigInt), q: &(usize, [BigInt; 6], BigInt)| {
        p.0.cmp(&q.0).then_with(|| {
            (0..6)
                .map(|i| ratio_cmp(&p.1[i], &p.2, &q.1[i], &q.2))
                .find(|o| o.is_ne())
                .unwrap_or(Ordering::Equal)
        })
    };
    for (at, pair) in lines.windows(2).enumerate() {
        assert_ne!(
            cmp(&pair[0], &pair[1]),
            Ordering::Greater,
            "edges {at} and {}",
            at + 1
        );
    }
    for (e, edge) in m.edges.iter().enumerate() {
        for f in &edge.fins {
            let fin = &m.fins[*f];
            let face = m.sheets[m.faces[fin.face].sheet].face;
            assert_eq!(
                cmp(&exact_line(&face, &fin.bound), &lines[e]),
                Ordering::Equal
            );
        }
    }
}

/// `t` turned to face the other way along x, its corners' order swapped where that keeps them
/// a tetrahedron the helper accepts.
fn mirrored_x(t: [[i64; 3]; 4]) -> [[i64; 3]; 4] {
    t.map(|[x, y, z]| [-x, y, z])
}

#[test]
fn points_in_wide_cells_keep_their_order_and_meet_points_in_words() {
    // Tilted solids with edges of a few thousand steps: three of their faces meet where w, near
    // 2^73, is past `i64` and X still fits `i128`, so their corners' cells come from 128-bit
    // division. A vertex is named by its planes in canonical form, so the corners are chosen to
    // leave each face's plane a common factor of 2 at most: w stays that wide once they are
    // reduced. A box touches one solid at its corner (0, 0, 0), which the box's faces name with
    // w = 1, in machine words: both must be one vertex. The same mirrored in x puts the points at
    // negative x, where a division rounds the other way.
    let a = [
        [0, 0, 0],
        [5000, 701, 3],
        [607, 4802, 903],
        [1301, 599, 4705],
    ];
    let b = [
        [702, 803, 599],
        [5303, 2101, 1409],
        [2003, 5897, 2610],
        [2708, 1301, 5801],
    ];
    for mirror in [false, true] {
        let way = |t: [[i64; 3]; 4]| if mirror { mirrored_x(t) } else { t };
        let mut polygons = scene(&[if mirror {
            block([0, -4, -4], [4, 0, 0])
        } else {
            block([-4, -4, -4], [0, 0, 0])
        }]);
        for (entry, t) in [a, b].into_iter().enumerate() {
            let t = way(t);
            if tetrahedron(t, entry as u32 + 1, &mut polygons).is_none() {
                tetrahedron([t[1], t[0], t[2], t[3]], entry as u32 + 1, &mut polygons).unwrap();
            }
        }
        for leaf in [1, LEAF_POLYGONS] {
            let pieces = kept(&polygons, &WindingTest::unite(), leaf);
            let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
            closed(&m, &pieces);
            in_point_order(&m);
            in_line_order(&m);
            let origin = m
                .vertices
                .iter()
                .filter(|v| {
                    let (x, _) = exact_point(v);
                    x.iter().all(|c| *c == BigInt::from(0))
                })
                .count();
            assert_eq!(origin, 1, "mirror {mirror}, leaf {leaf}");
            assert!(m.vertices.iter().any(|v| {
                let (_, w) = v.homogeneous().unwrap();
                w.to_i128().is_some_and(|w| w.unsigned_abs() > 1 << 64)
            }));
            assert!(m.vertices.iter().any(|v| !v.to_grid().unwrap().is_exact()));
        }
    }
}

#[test]
fn points_whose_first_coordinates_agree_to_32_binary_places_are_ordered_by_them() {
    // A box's four edges along x pierce a face nearly square to x, its normal (2^20 + 3) 2^20
    // along x and about 2^20 across: where they cross it, x is 4 and a little, and the crossings on
    // the edges at (y, z) = (8, 0) and (0, 8) lie 8 / ((2^20 + 3) 2^20), about 2^-37, apart.
    // Their x share their floor and first 32 binary places, and neither stops there, so only the
    // whole fraction orders them: the one at y = 0 has the greater x and comes after.
    let (a, c) = ((1i64 << 20) + 3, 1i64 << 20);
    let p0 = [4, -1, -1];
    let u = [p0[0] + 1, p0[1] + a, p0[2]];
    let v = [p0[0], p0[1] - c - 1, p0[2] + c];
    let mut polygons = scene(&[block([0, 0, 0], [8, 8, 8])]);
    let wedge = [p0, u, v, [-100, 3, 3]];
    if tetrahedron(wedge, 1, &mut polygons).is_none() {
        tetrahedron([u, p0, v, [-100, 3, 3]], 1, &mut polygons).unwrap();
    }
    let at = |m: &Merged, y: i64, z: i64| {
        let found = m.vertices.iter().enumerate().find(|(_, v)| {
            let (x, w) = exact_point(v);
            x[1] == BigInt::from(y) * &w
                && x[2] == BigInt::from(z) * &w
                && x[0] > BigInt::from(4) * &w
                && x[0] < BigInt::from(5) * &w
        });
        let (k, v) = found.unwrap();
        (k, exact_point(v))
    };
    for leaf in [1, 4, LEAF_POLYGONS] {
        let pieces = kept(&polygons, &WindingTest::unite(), leaf);
        let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
        closed(&m, &pieces);
        in_point_order(&m);
        in_line_order(&m);
        let ((low, p), (high, q)) = (at(&m, 8, 0), at(&m, 0, 8));
        assert!(low < high, "leaf {leaf}");
        // The floor and first 32 binary places of x agree, and neither fraction stops there.
        let lead = |(x, w): &([BigInt; 3], BigInt)| {
            let scaled = &x[0] << 32;
            (&scaled / w, &scaled % w != BigInt::from(0))
        };
        assert_eq!(lead(&p), lead(&q));
        assert!(lead(&p).1);
    }
}

#[test]
fn points_whose_fractions_need_more_than_96_bits_keep_their_order() {
    // Two tilted solids a thousand times larger: their faces' normals near 2^33, and where three
    // faces of the two meet, w is past 2^96 and X still fits `i128`, so a corner's fraction is
    // read a binary place at a time. A vertex is named by its planes in canonical form, so each
    // corner is 1024 times a small solid's, moved a few steps off that grid: a solid scaled whole
    // would have planes that reduce to the small solid's, with a narrow w, while these keep a
    // common factor of 3 at most.
    let a = [
        [0, 5, 3],
        [92163, 10241, 6],
        [10246, 81924, 10242],
        [20482, 10240, 71685],
    ];
    let b = [
        [10240, 10243, 10246],
        [81931, 30721, 20484],
        [30729, 92172, 40962],
        [40967, 20490, 92160],
    ];
    for (a, b) in [(a, b), (mirrored_x(a), mirrored_x(b))] {
        let mut polygons = Vec::new();
        for (entry, t) in [a, b].into_iter().enumerate() {
            if tetrahedron(t, entry as u32, &mut polygons).is_none() {
                tetrahedron([t[1], t[0], t[2], t[3]], entry as u32, &mut polygons).unwrap();
            }
        }
        for leaf in [1, LEAF_POLYGONS] {
            let pieces = kept(&polygons, &WindingTest::unite(), leaf);
            let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
            closed(&m, &pieces);
            in_point_order(&m);
            in_line_order(&m);
            assert!(m.vertices.iter().any(|v| {
                let (x, w) = v.homogeneous().unwrap();
                let fits = x.iter().all(|c| c.to_i128().is_some());
                fits && w.to_i128().is_some_and(|w| w.unsigned_abs() >> 96 != 0)
            }));
        }
    }
}

/// A prism along `axis` from `low` to `high` over a convex polygon, each face one piece of entry
/// `entry`: the polygon's sides in turn, counterclockwise seen from the high end, each a line
/// `n[0] u + n[1] v + d = 0` facing out, u and v the next two axes in turn. Each face's bounds run
/// in turn about its outward normal, as [`side`] makes a block's.
fn prism(
    axis: usize,
    (low, high): (i64, i64),
    sides: &[([i128; 2], i128)],
    entry: u32,
    polygons: &mut Vec<Polygon>,
) {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let walls: Vec<Plane> = sides
        .iter()
        .map(|(n, d)| {
            let mut m = [0i128; 3];
            m[u] = n[0];
            m[v] = n[1];
            Plane::new(m, *d).unwrap()
        })
        .collect();
    let (bottom, top) = (down(axis, low), up(axis, high));
    let inside = |p: &Plane| opposite(p).unwrap();
    let k = walls.len();
    let mut push = |face: Plane, bounds: Vec<Plane>| {
        polygons.push(Polygon {
            piece: Piece { face, bounds },
            entry: Some(entry),
        });
    };
    push(top, walls.iter().map(inside).collect());
    push(bottom, walls.iter().rev().map(inside).collect());
    for (i, wall) in walls.iter().enumerate() {
        let bounds = vec![
            inside(&bottom),
            inside(&walls[(i + 1) % k]),
            inside(&top),
            inside(&walls[(i + k - 1) % k]),
        ];
        push(*wall, bounds);
    }
}

/// A quadrilateral with corners (0, 0) and (0, 12) on its left and a right side nearly square to
/// the first axis, x = 13 / 3 + (20 − y) / `n` (`n` a multiple of 3), whose ends meet a bottom
/// x = `n1` y and a top y = 12 + x / `n2`: the two corners on the right both lie a little past
/// x = 13 / 3, the lower further, by 12 / `n` and less, with w = `n` `n1` + 1 and `n` `n2` + 1.
fn sliver(n: i128, n1: i128, n2: i128) -> Vec<([i128; 2], i128)> {
    vec![
        ([1, -n1], 0),
        ([n, 1], -(13 * (n / 3) + 20)),
        ([-1, n2], -12 * n2),
        ([-1, 0], 0),
    ]
}

/// Its floor and first 32 binary places, and whether it runs on past them, of X / w.
fn lead_of(x: &BigInt, w: &BigInt) -> (BigInt, bool) {
    let scaled = x << 32;
    let floor = num_integer_floor(&scaled, w);
    let rest = &scaled - &floor * w;
    (floor, rest != BigInt::from(0))
}

/// The floor of a / b, b positive.
fn num_integer_floor(a: &BigInt, b: &BigInt) -> BigInt {
    let q = a / b;
    if &q * b > *a { q - 1 } else { q }
}

#[test]
fn points_whose_first_coordinates_agree_past_32_binary_places_keep_their_order_at_any_width() {
    // A prism over a sliver: its right side's two lower corners lie a little past x = 13 / 3, at
    // y near 0 and near 12, their x apart by about 12 / n. With n near 2^37 the corners' w is
    // below 2^63 and their remainders below 2^96, so a fraction's first 32 places come from one
    // division; with n near 2^50 w passes 2^96 and so does the remainder, and they come a place
    // at a time. Either way the two x share their floor and first 32 places and both run on, so
    // only the whole fractions order them: the corner near y = 12, with the smaller x, first.
    for (n, n1, n2, wide) in [
        (3i128 << 36, 1i128 << 24, (1i128 << 24) + 1, false),
        (3 << 48, (1 << 50) + 1, (1 << 50) + 3, true),
    ] {
        let mut polygons = Vec::new();
        prism(2, (0, 8), &sliver(n, n1, n2), 0, &mut polygons);
        for leaf in [1, LEAF_POLYGONS] {
            let pieces = kept(&polygons, &WindingTest::unite(), leaf);
            let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
            closed(&m, &pieces);
            in_point_order(&m);
            in_line_order(&m);
            let corner = |near_top: bool| {
                let found = m.vertices.iter().enumerate().find(|(_, v)| {
                    let (x, w) = exact_point(v);
                    let y_high = x[1] > BigInt::from(6) * &w;
                    x[2] == BigInt::from(0) && x[0] > BigInt::from(4) * &w && y_high == near_top
                });
                let (k, v) = found.unwrap();
                (k, exact_point(v))
            };
            let ((upper, p), (lower, q)) = (corner(true), corner(false));
            assert!(upper < lower, "n {n}, leaf {leaf}");
            assert_eq!(lead_of(&p.0[0], &p.1), lead_of(&q.0[0], &q.1));
            assert!(lead_of(&p.0[0], &p.1).1);
            let past_96 = q.1.bits() > 97;
            assert_eq!(past_96, wide, "n {n}");
        }
    }
}

#[test]
fn a_fraction_that_stops_is_ordered_by_its_places_when_w_is_wide() {
    // A prism whose bottom right corner is (9 / 2, 0), where its bottom and right side meet with
    // w near 2^100, so the half is read a binary place at a time and stops after one; a block's
    // corner at (4, 5, 20) has the same floor in x and a fraction of none. The corner at x = 4
    // comes first, though its y is the greater.
    let (n, n1) = (1i128 << 49, (1i128 << 49) + 1);
    let sides = vec![
        ([2, -2 * n1], -9),
        ([2 * n, 2], -9 * n),
        ([-1, n1], -12 * n1),
        ([-1, 0], 0),
    ];
    let mut polygons = scene(&[block([0, 5, 20], [4, 9, 24])]);
    prism(2, (0, 8), &sides, 1, &mut polygons);
    let pieces = kept(&polygons, &WindingTest::unite(), LEAF_POLYGONS);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&m, &pieces);
    in_point_order(&m);
    let at = |want: [i64; 3], half: bool| {
        m.vertices
            .iter()
            .position(|v| {
                let (x, w) = exact_point(v);
                let twice = |i: usize| BigInt::from(2) * &x[i];
                let goal = |i: usize| BigInt::from(want[i] * 2 + i64::from(half && i == 0)) * &w;
                (0..3).all(|i| twice(i) == goal(i))
            })
            .unwrap()
    };
    let (half, whole) = (at([4, 0, 0], true), at([4, 5, 20], false));
    assert!(whole < half);
    let (_, w) = exact_point(&m.vertices[half]);
    assert!(w.bits() > 97);
}

/// Lines whose marked cells pass the reach keep their order: at a leaf of one polygon and at the
/// usual leaf, one test each (decision 32.78: the two together were too long for one Miri gate
/// part).
mod lines_whose_marked_cells_pass_the_reach_keep_their_order {
    #[test]
    fn at_a_leaf_of_one_polygon() {
        super::lines_past_the_reach_at(1);
    }

    #[test]
    fn at_the_usual_leaf() {
        super::lines_past_the_reach_at(super::LEAF_POLYGONS);
    }
}

/// The check of `lines_whose_marked_cells_pass_the_reach_keep_their_order` at leaf size `leaf`.
fn lines_past_the_reach_at(leaf: usize) {
    // A prism along x over a sliver in (y, z): its right side is two faces nearly square to y,
    // normals (0, K, 2) with K just past 2^63 and differing by two, meeting at (y, z) = (-1, 4),
    // and its left side a third, normal (0, -K', 2). Where an end of the prism meets them, a line
    // runs along y whose z over y is -K / 2 or K' / 2, past the 2^62 a mark holds, and whose
    // moment's x over y is within it, for the two right faces in the other order from their z.
    // Lines of the prism's ends along y with z over y 0 lie between those past the reach.
    let k1 = (1i128 << 63) + 1;
    let k3 = (1i128 << 63) + 5;
    let sides = vec![
        ([0, -1], 0),
        ([k1 + 2, 2], k1 - 6),
        ([k1, 2], k1 - 8),
        ([0, 1], -8),
        ([-k3, 2], -5 * k3),
    ];
    let mut polygons = Vec::new();
    prism(0, (0, 8), &sides, 0, &mut polygons);
    let pieces = kept(&polygons, &WindingTest::unite(), leaf);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&m, &pieces);
    in_point_order(&m);
    in_line_order(&m);
    let beyond = edge_lines(&m)
        .iter()
        .filter(|(first, c, over)| *first == 1 && num_integer_floor(&c[2], over).bits() > 62)
        .count();
    assert!(beyond >= 6, "leaf {leaf}: {beyond}");
}

#[test]
fn an_edge_s_turn_is_taken_from_a_memo_only_for_fins_that_run_as_they_did() {
    // Wedges round the z axis, each a prism over a triangle with a corner on the axis. One, from
    // 0 to about 27 degrees, runs the whole height. Below z = 4 two more stand beside it, from 63
    // to 101 degrees and from 146 to 170; above z = 6 two others, from 63 to 170 and from 281 to
    // 326, on the same four planes facing the same ways. The axis is then two edges of six fins
    // on the same planes, the first fin the same, which turn about it in different orders, as
    // their fins run up the axis in different patterns.
    let wedge = |a: [i128; 2], b: [i128; 2]| {
        let n = [b[1] - a[1], a[0] - b[0]];
        vec![
            ([a[1], -a[0]], 0),
            (n, -10 * (n[0] * a[0] + n[1] * a[1])),
            ([-b[1], b[0]], 0),
        ]
    };
    let (start, end) = ([1, 0], [2, 1]);
    let (a, b, c, d) = ([1, 2], [-1, 5], [-3, 2], [-6, 1]);
    let (b_back, c_back) = ([1, -5], [3, -2]);
    let mut polygons = Vec::new();
    prism(2, (0, 10), &wedge(start, end), 0, &mut polygons);
    prism(2, (0, 4), &wedge(a, b), 1, &mut polygons);
    prism(2, (0, 4), &wedge(c, d), 2, &mut polygons);
    prism(2, (6, 10), &wedge(a, d), 3, &mut polygons);
    prism(2, (6, 10), &wedge(b_back, c_back), 4, &mut polygons);
    let pieces = kept(&polygons, &WindingTest::unite(), LEAF_POLYGONS);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&m, &pieces);
    // The two edges of six fins on the axis read the same planes in sheet order, from the same
    // first fin, and turn their fins in different orders.
    let on_axis = |v: usize| {
        let (x, _) = exact_point(&m.vertices[v]);
        x[0] == BigInt::from(0) && x[1] == BigInt::from(0)
    };
    let six: Vec<&geom_poly::merge::Edge> = m
        .edges
        .iter()
        .filter(|e| e.fins.len() == 6 && on_axis(e.low) && on_axis(e.high))
        .collect();
    assert_eq!(six.len(), 2);
    let read = |e: &geom_poly::merge::Edge| {
        let mut fins: Vec<usize> = e.fins.clone();
        fins.sort_by_key(|f| m.faces[m.fins[*f].face].sheet);
        let planes: Vec<Plane> = fins
            .iter()
            .map(|f| m.sheets[m.faces[m.fins[*f].face].sheet].face)
            .collect();
        let turn: Vec<usize> = e
            .fins
            .iter()
            .map(|f| fins.iter().position(|g| g == f).unwrap())
            .collect();
        (m.fins[fins[0]].bound, planes, turn)
    };
    let (one, other) = (read(six[0]), read(six[1]));
    assert_eq!((one.0, &one.1), (other.0, &other.1));
    assert_ne!(one.2, other.2);
    // A merge from its own memo is the merge without it.
    let (again, shared) = merged_again(&pieces, &pieces, None);
    assert_eq!(again.unwrap(), m);
    assert_eq!(shared, pieces.len());
}

fn any_block() -> impl Strategy<Value = Block> {
    ([-6i64..6, -6i64..6, -6i64..6], [1i64..6, 1i64..6, 1i64..6]).prop_map(|(low, size)| Block {
        low,
        high: [low[0] + size[0], low[1] + size[1], low[2] + size[2]],
    })
}

fn any_corners() -> impl Strategy<Value = [[i64; 3]; 4]> {
    let corner = || [-40i64..=40, -40i64..=40, -40i64..=40];
    [corner(), corner(), corner(), corner()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn every_union_closes_and_the_boxes_do_not_show(
        blocks in prop::collection::vec(any_block(), 1..4),
        leaf in 1usize..10,
    ) {
        let test = WindingTest::unite();
        let once = merged(&blocks, &test, leaf);
        let whole = merged(&blocks, &test, 1000);
        prop_assert_eq!(&once.edges, &whole.edges);
        prop_assert_eq!(&once.shells, &whole.shells);
    }

    #[test]
    fn a_merge_from_a_memo_is_the_merge_without_it(
        blocks in prop::collection::vec(any_block(), 1..4),
        moved in any_block(),
        which in 0usize..4,
        leaf in 1usize..10,
        budget in prop::option::of(0u64..3000),
    ) {
        let test = WindingTest::unite();
        let before = kept(&scene(&blocks), &test, leaf);
        let mut after_blocks = blocks.clone();
        let at = which % after_blocks.len();
        after_blocks[at] = moved;
        let after = kept(&scene(&after_blocks), &test, leaf);
        let _ = merged_again(&before, &after, budget);
    }

    #[test]
    fn a_tilted_merge_from_a_memo_is_the_merge_without_it(
        solids in prop::collection::vec(any_corners(), 2..4),
        moved in any_corners(),
        leaf in 1usize..10,
    ) {
        let solid_pieces = |solids: &[[[i64; 3]; 4]]| {
            let mut polygons = Vec::new();
            for (entry, corners) in solids.iter().enumerate() {
                tetrahedron(*corners, entry as u32, &mut polygons)?;
            }
            Some(kept(&polygons, &WindingTest::unite(), leaf))
        };
        let mut after = solids.clone();
        after[0] = moved;
        let (before, after) = (solid_pieces(&solids), solid_pieces(&after));
        prop_assume!(before.is_some() && after.is_some());
        let _ = merged_again(&before.unwrap(), &after.unwrap(), None);
    }

    #[test]
    fn every_tilted_union_closes_and_the_boxes_do_not_show(
        solids in prop::collection::vec(any_corners(), 1..4),
        leaf in 1usize..10,
    ) {
        let test = WindingTest::unite();
        let once = tilted(&solids, &test, leaf);
        prop_assume!(once.is_some());
        let once = once.unwrap();
        let whole = tilted(&solids, &test, 1000).unwrap();
        prop_assert_eq!(&once.vertices, &whole.vertices);
        prop_assert_eq!(&once.edges, &whole.edges);
    }
}
