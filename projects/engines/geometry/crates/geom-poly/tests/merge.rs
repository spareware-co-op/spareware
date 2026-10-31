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

use geom_num::plane::Plane;
use geom_num::{NumError, P3, Sign};
use geom_poly::convex::{Piece, convex_pieces};
use geom_poly::merge::{
    Merge, MergeError, Merged, Outward, Rules, as_kept, both_ways, merge, merge_with, section,
    select,
};
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange};
use geom_poly::trace::{Classified, Polygon, TraceError};
use geom_poly::winding::{Term, Winding, WindingTest};
use geom_run::{Budget, Exec, WorkMeter};
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
    let shell_of = |at: usize| m.faces[m.piece_faces[at]].shell;
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
    let shell_of = |at: usize| m.faces[m.piece_faces[at]].shell;
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

#[test]
fn a_section_runs_where_two_owners_meet() {
    let pieces = |blocks: &[Block], leaf: usize| -> Vec<(Piece, usize)> {
        let polygons = scene(blocks);
        let owner = |c: &Classified| polygons[c.polygon].entry.unwrap_or(0) as usize;
        let found = classified(&polygons, leaf);
        found.iter().map(|c| (c.piece.clone(), owner(c))).collect()
    };
    for leaf in [1, LEAF_POLYGONS] {
        let curves = section(&pieces(&[A, B], leaf), &mut meter()).unwrap();
        assert_eq!((curves.edges.len(), curves.vertices.len()), (6, 6));
        assert!(curves.edges.iter().all(|(low, high)| low < high));
    }
    let flush = block([4, 0, 0], [8, 4, 4]);
    let curves = section(&pieces(&[A, flush], LEAF_POLYGONS), &mut meter()).unwrap();
    assert_eq!(curves.edges.len(), 4, "around their contact, not across it");
    let apart = block([9, 9, 9], [12, 12, 12]);
    let curves = section(&pieces(&[A, apart], LEAF_POLYGONS), &mut meter()).unwrap();
    assert_eq!((curves.edges.len(), curves.vertices.len()), (0, 0));
    let mut small = WorkMeter::new(Budget::new(3, u64::MAX));
    assert!(matches!(
        section(&pieces(&[A, B], LEAF_POLYGONS), &mut small),
        Err(MergeError::Stopped(_))
    ));
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
        MergeError::from(stopped).to_string(),
        "the meter stopped the merge"
    );
    let num = MergeError::from(NumError::Degenerate);
    assert!(num.to_string().starts_with("pieces that cannot be merged"));
    let trace = TraceError::Num(NumError::Degenerate);
    assert_eq!(MergeError::from(trace).to_string(), trace.to_string());
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
    Some(m)
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
fn an_edge_breaks_where_a_corner_rests_on_it() {
    let mut polygons = scene(&[A]);
    let tip = [[2, 4, 4], [2, 8, 5], [3, 9, 9], [1, 5, 9]];
    tetrahedron(tip, 1, &mut polygons).unwrap();
    let pieces = kept(&polygons, &WindingTest::unite(), LEAF_POLYGONS);
    let m = merge(&pieces, Merge::RemoveRedundant, &mut meter()).unwrap();
    closed(&m, &pieces);
    assert_eq!(counts(&m), [2, 10, 10, 19, 12]);
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
