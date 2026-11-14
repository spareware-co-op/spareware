//! Merged pieces published as a body, row by row against the merge they came from: regions and
//! shells, faces, loops and fins in their rings, senses, boxes, edges' lines and the planes a point
//! off the grid is kept on (G149, 11.3).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and short names, and a failure stops the test"
)]

use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, P3, Sign};
use geom_poly::convex::{Piece, convex_pieces};
use geom_poly::merge::{Curves, Merge, Merged, Outward, merge, select};
use geom_poly::publish::{Carrier, PublishError, publish, publish_with, wire};
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange};
use geom_poly::trace::Polygon;
use geom_poly::winding::{Winding, WindingTest};
use geom_run::{Budget, Exec, WorkMeter};
use geom_surface::Plane as Flat;
use geom_topo::order::{NameKey, Names};
use geom_topo::{
    Arena, Body, BodyKind, Box3, Column, CurveKind, Line, PointKind, RegionKind, Sense,
    SurfaceKind, TopoError,
};

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

fn opposite(p: &Plane) -> Plane {
    Plane::new(p.n().map(|v| -v), -p.d()).unwrap()
}

type Block = ([i64; 3], [i64; 3]);

fn side(b: &Block, axis: usize, at: i64, upward: bool) -> Piece {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let (ul, uh) = (b.0[u], b.1[u]);
    let (vl, vh) = (b.0[v], b.1[v]);
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
            for (at, upward) in [(b.0[axis], false), (b.1[axis], true)] {
                polygons.push(Polygon {
                    piece: side(b, axis, at, upward),
                    entry: Some(entry as u32),
                });
            }
        }
    }
    polygons
}

fn tetrahedron(corner: [[i64; 3]; 4], entry: u32, polygons: &mut Vec<Polygon>) {
    let at = |p: [i64; 3]| P3::new(p[0], p[1], p[2]).unwrap();
    let mut planes = Vec::new();
    for i in 0..4 {
        let [a, b, c] = [(i + 1) % 4, (i + 2) % 4, (i + 3) % 4].map(|k| corner[k]);
        let plane = Plane::through(at(a), at(b), at(c)).unwrap();
        planes.push(match plane.side(at(corner[i])).unwrap() {
            Sign::Negative => plane,
            _ => opposite(&plane),
        });
    }
    for i in 0..4 {
        let sides: Vec<Plane> = (1..4).map(|k| opposite(&planes[(i + k) % 4])).collect();
        let mut pieces = convex_pieces(&planes[i], &[sides], &mut meter()).unwrap();
        polygons.push(Polygon {
            piece: pieces.pop().unwrap(),
            entry: Some(entry),
        });
    }
}

fn kept(polygons: &[Polygon], test: &WindingTest) -> Vec<Outward> {
    let cell = around(polygons).unwrap();
    let classified = arrange(
        &Exec::serial(),
        cell,
        Winding::new(),
        polygons,
        LEAF_POLYGONS,
        &[],
        &mut meter(),
    )
    .unwrap();
    select(test, &classified).unwrap()
}

fn joined(polygons: &[Polygon], test: &WindingTest) -> Merged {
    merge(&kept(polygons, test), Merge::RemoveRedundant, &mut meter()).unwrap()
}

fn merged(blocks: &[Block], test: &WindingTest) -> Merged {
    joined(&scene(blocks), test)
}

fn leaning(test: &WindingTest) -> Merged {
    let mut polygons = Vec::new();
    tetrahedron(
        [[0, 0, 0], [9, 1, 0], [1, 8, 1], [2, 3, 9]],
        0,
        &mut polygons,
    );
    tetrahedron(
        [[1, 1, 1], [8, 2, 7], [2, 9, 6], [7, 8, -3]],
        1,
        &mut polygons,
    );
    joined(&polygons, test)
}

fn surface(polygon: usize) -> SurfaceKind {
    let wide = (f64::NEG_INFINITY, f64::INFINITY);
    SurfaceKind::Plane(
        Flat::new(
            [polygon as f64, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (wide, wide),
        )
        .unwrap(),
    )
}

/// A carrier for each polygon of `m`'s sheets, each with `sense`.
fn carriers(m: &Merged, sense: Sense) -> Vec<Carrier> {
    let polygons = m
        .sheets
        .iter()
        .map(|s| s.polygon)
        .max()
        .map_or(0, |p| p + 1);
    (0..polygons)
        .map(|polygon| Carrier {
            surface: surface(polygon),
            sense,
        })
        .collect()
}

fn body(m: &Merged, sense: Sense) -> Body {
    publish(m, &carriers(m, sense), &mut meter()).unwrap()
}

fn list<K, V: Copy>(column: &Column<K, V>) -> Vec<V> {
    column.iter().copied().collect()
}

/// Each row of `column`, as a row number.
fn rows<K, I: Copy + Into<RowOf>>(column: &Column<K, I>) -> Vec<u32> {
    column.iter().map(|i| (*i).into().0).collect()
}

/// A reference's row, or `u32::MAX` for none.
struct RowOf(u32);

impl<K> From<geom_topo::Ix<K>> for RowOf {
    fn from(i: geom_topo::Ix<K>) -> Self {
        RowOf(i.row())
    }
}

impl<K> From<Option<geom_topo::Ix<K>>> for RowOf {
    fn from(i: Option<geom_topo::Ix<K>>) -> Self {
        RowOf(i.map_or(u32::MAX, geom_topo::Ix::row))
    }
}

const NONE: u32 = u32::MAX;

/// What each vertex of `m` rounds to, and whether it is exact.
fn rounded(m: &Merged) -> Vec<(P3, bool)> {
    m.vertices
        .iter()
        .map(|v| {
            let near = v.to_grid().unwrap();
            (near.value(), near.is_exact())
        })
        .collect()
}

#[test]
fn errors_say_what_was_wrong() {
    assert_eq!(PublishError::Broken.to_string(), "a publication lost a row");
    let num = PublishError::from(NumError::Degenerate);
    assert_eq!(num, PublishError::Num(NumError::Degenerate));
    assert!(num.to_string().contains("Degenerate"), "{num}");
    let far = TopoError::NoSuchRow {
        arena: Arena::Plane,
        row: 3,
    };
    let topo = PublishError::from(far);
    assert_eq!(topo, PublishError::Topo(far));
    assert!(topo.to_string().contains("NoSuchRow"), "{topo}");
}

#[test]
fn each_outer_shell_is_a_solid_of_its_own() {
    let m = merged(
        &[([0, 0, 0], [2, 2, 2]), ([5, 0, 0], [7, 2, 2])],
        &WindingTest::unite(),
    );
    let b = body(&m, Sense::Forward);
    let a = b.arenas();
    assert_eq!(
        list(&a.regions.kind),
        [RegionKind::Solid, RegionKind::Solid, RegionKind::Void]
    );
    assert_eq!(rows(&a.regions.first_shell), [0, 1, NONE]);
    assert_eq!(rows(&a.shells.region), [0, 1]);
    assert_eq!(rows(&a.shells.next), [NONE, NONE]);
    assert_eq!(b.outside().row(), 2);
    assert_eq!(b.kind(), BodyKind::Solid);
}

#[test]
fn an_open_merge_keeps_its_shells_in_the_outside() {
    let m = merged(&[([0, 0, 0], [2, 2, 2])], &WindingTest::unite());
    let open = publish_with(
        &m,
        &carriers(&m, Sense::Forward),
        true,
        (None, None),
        &mut meter(),
    )
    .unwrap();
    let a = open.arenas();
    assert_eq!(list(&a.regions.kind), [RegionKind::Void]);
    assert_eq!(rows(&a.regions.first_shell), [0]);
    assert_eq!(rows(&a.shells.region), vec![0; m.shells.len()]);
    assert_eq!(open.outside().row(), 0);
    assert_eq!(open.kind(), BodyKind::Sheet);
    // Nothing kept: the outside has no shell.
    let none = Merged {
        sheets: Vec::new(),
        vertices: Vec::new(),
        edges: Vec::new(),
        fins: Vec::new(),
        faces: Vec::new(),
        shells: Vec::new(),
        piece_face: Vec::new(),
    };
    let empty = publish_with(&none, &[], true, (None, None), &mut meter()).unwrap();
    assert_eq!(rows(&empty.arenas().regions.first_shell), [NONE]);
    assert_eq!(empty.kind(), BodyKind::Sheet);
}

#[test]
fn a_cavity_is_a_second_shell_of_its_solid() {
    let m = merged(
        &[([0, 0, 0], [9, 9, 9]), ([3, 3, 3], [6, 6, 6])],
        &WindingTest::subtract(0),
    );
    let b = body(&m, Sense::Forward);
    let a = b.arenas();
    assert_eq!(list(&a.regions.kind), [RegionKind::Solid, RegionKind::Void]);
    assert_eq!(rows(&a.shells.region), [0, 0]);
    assert_eq!(rows(&a.shells.next), [1, NONE]);
    assert_eq!(rows(&a.regions.first_shell), [0, NONE]);
}

#[test]
fn a_shells_faces_chain_in_its_order() {
    let m = merged(
        &[([0, 0, 0], [9, 9, 9]), ([3, 3, 3], [6, 6, 6])],
        &WindingTest::subtract(0),
    );
    let b = body(&m, Sense::Forward);
    let a = b.arenas();
    let (first, next, shell) = (
        rows(&a.shells.first_face),
        rows(&a.faces.next),
        rows(&a.faces.shell),
    );
    for (s, held) in m.shells.iter().enumerate() {
        let mut walked = vec![first[s]];
        while next[*walked.last().unwrap() as usize] != NONE {
            walked.push(next[*walked.last().unwrap() as usize]);
        }
        let want: Vec<u32> = held.faces.iter().map(|f| *f as u32).collect();
        assert_eq!(walked, want);
        assert!(walked.iter().all(|f| shell[*f as usize] == s as u32));
    }
}

#[test]
fn a_faces_sense_is_its_sheets_turn_against_its_surfaces() {
    let m = merged(
        &[([0, 0, 0], [9, 9, 9]), ([3, 3, 3], [6, 6, 6])],
        &WindingTest::subtract(0),
    );
    assert!(m.sheets.iter().any(|s| s.turned));
    assert!(m.sheets.iter().any(|s| !s.turned));
    for sense in [Sense::Forward, Sense::Reversed] {
        let b = body(&m, sense);
        let senses = list(&b.arenas().faces.sense);
        for (face, got) in m.faces.iter().zip(senses) {
            let sheet = &m.sheets[face.sheet];
            let want = if sheet.turned == (sense == Sense::Reversed) {
                Sense::Forward
            } else {
                Sense::Reversed
            };
            assert_eq!(got, want);
        }
    }
}

#[test]
fn a_polygon_keeps_one_surface() {
    let m = merged(
        &[([0, 0, 0], [9, 3, 3]), ([4, -1, -1], [5, 4, 4])],
        &WindingTest::subtract(0),
    );
    let b = body(&m, Sense::Forward);
    let a = b.arenas();
    let surfaces = rows(&a.faces.surface);
    for (i, f) in m.faces.iter().enumerate() {
        for (j, g) in m.faces.iter().enumerate() {
            assert_eq!(
                surfaces[i] == surfaces[j],
                m.sheets[f.sheet].polygon == m.sheets[g.sheet].polygon
            );
        }
        let row = surfaces[i] as usize;
        let held: Vec<&SurfaceKind> = a.surfaces.iter().collect();
        assert_eq!(*held[row], surface(m.sheets[f.sheet].polygon));
    }
}

#[test]
fn a_face_with_a_hole_has_a_loop_for_each_ring() {
    let m = merged(
        &[([0, 0, 0], [9, 9, 3]), ([3, 3, -1], [6, 6, 4])],
        &WindingTest::subtract(0),
    );
    assert!(m.faces.iter().any(|f| f.rings.len() == 2));
    let b = body(&m, Sense::Forward);
    let a = b.arenas();
    let (first, next, face) = (
        rows(&a.faces.first_loop),
        rows(&a.loops.next),
        rows(&a.loops.face),
    );
    let mut at = 0;
    for (f, held) in m.faces.iter().enumerate() {
        assert_eq!(first[f], at);
        for ring in 0..held.rings.len() {
            let want = if ring + 1 < held.rings.len() {
                at + 1
            } else {
                NONE
            };
            assert_eq!(next[at as usize], want);
            assert_eq!(face[at as usize], f as u32);
            at += 1;
        }
    }
}

#[test]
fn fins_run_their_rings() {
    let m = leaning(&WindingTest::unite());
    let b = body(&m, Sense::Forward);
    let a = b.arenas();
    let (next, prev, edge, sense, radial, first_fin, owner) = (
        rows(&a.fins.next),
        rows(&a.fins.prev),
        rows(&a.fins.edge),
        list(&a.fins.sense),
        rows(&a.fins.radial),
        rows(&a.loops.first_fin),
        rows(&a.fins.loop_),
    );
    let order: Vec<usize> = m
        .faces
        .iter()
        .flat_map(|f| f.rings.iter().flatten().copied())
        .collect();
    let mut row_of = vec![0u32; m.fins.len()];
    for (row, fin) in order.iter().enumerate() {
        row_of[*fin] = row as u32;
    }
    let mut at = 0;
    for (looped, ring) in m.faces.iter().flat_map(|f| &f.rings).enumerate() {
        assert_eq!(first_fin[looped], at as u32);
        for (k, fin) in ring.iter().enumerate() {
            let here = at + k;
            let (after, before) = (
                ring[(k + 1) % ring.len()],
                ring[(k + ring.len() - 1) % ring.len()],
            );
            assert_eq!(next[here], row_of[after]);
            assert_eq!(prev[here], row_of[before]);
            assert_eq!(edge[here], m.fins[*fin].edge as u32);
            assert_eq!(owner[here], looped as u32);
            let forward = m.fins[*fin].from < m.fins[*fin].to;
            assert_eq!(sense[here] == Sense::Forward, forward);
        }
        at += ring.len();
    }
    for held in &m.edges {
        for (k, fin) in held.fins.iter().enumerate() {
            let after = held.fins[(k + 1) % held.fins.len()];
            assert_eq!(radial[row_of[*fin] as usize], row_of[after]);
        }
    }
}

#[test]
fn an_edge_runs_low_to_high_on_a_line_between_its_ends() {
    let m = leaning(&WindingTest::unite());
    let near = rounded(&m);
    assert!(near.iter().any(|(_, exact)| !exact));
    let b = body(&m, Sense::Forward);
    let a = b.arenas();
    let (start, end, first_fin) = (
        rows(&a.edges.start),
        rows(&a.edges.end),
        rows(&a.edges.first_fin),
    );
    let order: Vec<usize> = m
        .faces
        .iter()
        .flat_map(|f| f.rings.iter().flatten().copied())
        .collect();
    let curves: Vec<&CurveKind<3>> = a.curves.iter().collect();
    for (e, held) in m.edges.iter().enumerate() {
        assert_eq!(start[e], held.low as u32);
        assert_eq!(end[e], held.high as u32);
        assert_eq!(order[first_fin[e] as usize], held.fins[0]);
        let (p, q) = (near[held.low].0, near[held.high].0);
        let at = |p: P3| [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64];
        let (o, t) = (at(p), at(q));
        if o != t {
            let direction = [t[0] - o[0], t[1] - o[1], t[2] - o[2]];
            let line = Line::new(o, direction, (0.0, 1.0)).unwrap();
            assert_eq!(*curves[e], CurveKind::Line(line));
        }
    }
}

/// A box's corner at the origin, cut by the face 3x + y + z + 1 = 0 of a leaning tetrahedron a
/// third of a step along its edge in −x: that edge's ends round to one point, and its lower end is
/// the meeting, off the grid. A sub-grid edge stands a step long from whichever end is on the grid.
#[test]
fn an_edge_shorter_than_the_grid_stands_a_step_from_its_grid_end() {
    let mut polygons = scene(&[([-6, 0, 0], [0, 6, 6])]);
    tetrahedron(
        [[0, -1, 0], [0, 0, -1], [-1, 1, 1], [5, 5, 5]],
        1,
        &mut polygons,
    );
    let m = joined(&polygons, &WindingTest::intersect(2));
    let near = rounded(&m);
    let b = body(&m, Sense::Forward);
    let curves: Vec<&CurveKind<3>> = b.arenas().curves.iter().collect();
    let mut from_the_meeting = 0;
    for (e, held) in m.edges.iter().enumerate() {
        let ((p, low_exact), (q, _)) = (near[held.low], near[held.high]);
        if p != q {
            continue;
        }
        let o = [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64];
        let span = if low_exact {
            (0.0, 1.0)
        } else {
            from_the_meeting += 1;
            (-1.0, 0.0)
        };
        let line = Line::new(o, [1.0, 0.0, 0.0], span).unwrap();
        assert_eq!(*curves[e], CurveKind::Line(line));
    }
    assert!(from_the_meeting > 0);
}

#[test]
fn a_faces_box_holds_its_corners_a_step_wider_off_the_grid() {
    let m = leaning(&WindingTest::unite());
    let near = rounded(&m);
    let b = body(&m, Sense::Forward);
    let bounds = list(&b.arenas().faces.bound);
    let mut widened = false;
    for (face, got) in m.faces.iter().zip(bounds) {
        let mut low = [i64::MAX; 3];
        let mut high = [i64::MIN; 3];
        for fin in face.rings.iter().flatten() {
            let (p, exact) = near[m.fins[*fin].from];
            let margin = i64::from(!exact);
            widened |= !exact;
            for (axis, v) in [p.x.get(), p.y.get(), p.z.get()].into_iter().enumerate() {
                low[axis] = low[axis].min(v - margin);
                high[axis] = high[axis].max(v + margin);
            }
        }
        let want = Box3::new(
            P3::new(low[0], low[1], low[2]).unwrap(),
            P3::new(high[0], high[1], high[2]).unwrap(),
        )
        .unwrap();
        assert_eq!(got, want);
    }
    assert!(widened);
}

#[test]
fn a_point_off_the_grid_stays_on_the_lowest_planes_around_it() {
    let m = leaning(&WindingTest::unite());
    let b = body(&m, Sense::Forward);
    let a = b.arenas();
    let pool = list(&a.planes);
    let mut sorted = pool.clone();
    sorted.sort_by_key(|p| {
        let [x, y, z] = p.n();
        [x, y, z, p.d()]
    });
    sorted.dedup();
    assert_eq!(pool, sorted);
    let points = list(&a.points);
    let vertices = rows(&a.vertices.point);
    let planes_of: Vec<Plane> = m.faces.iter().map(|f| m.sheets[f.sheet].face).collect();
    let mut around_it = vec![Vec::new(); m.vertices.len()];
    for fin in &m.fins {
        around_it[fin.from].push(fin.face);
        around_it[fin.to].push(fin.face);
    }
    let mut off = 0;
    for (v, list) in around_it.iter_mut().enumerate() {
        list.sort_unstable();
        list.dedup();
        let near = m.vertices[v].to_grid().unwrap();
        match points[vertices[v] as usize] {
            PointKind::Grid(p) => {
                assert!(near.is_exact());
                assert_eq!(p, near.value());
            }
            PointKind::Meet(planes) => {
                assert!(!near.is_exact());
                off += 1;
                let [p, q, r] = planes.map(|i| pool[i.row() as usize]);
                let lowest = lowest_of(list, &planes_of);
                assert_eq!([p, q, r], lowest);
                assert_eq!(Tpi::new(p, q, r).unwrap().to_grid().unwrap(), near);
            }
        }
    }
    assert!(off > 0);
}

#[test]
fn a_wires_points_off_the_grid_keep_their_own_planes_each_stored_once() {
    let third = Plane::new([3, 0, 0], -1).unwrap();
    let (flat, low, high) = (up(1, 0), up(2, 0), up(2, 5));
    let curves = Curves {
        vertices: vec![
            Tpi::new(third, flat, low).unwrap(),
            Tpi::new(third, flat, high).unwrap(),
        ],
        edges: vec![(0, 1)],
        meets: vec![vec![0, 1]],
    };
    let b = wire(&curves, None, &mut meter()).unwrap();
    let a = b.arenas();
    assert_eq!(b.kind(), BodyKind::Wire);
    assert_eq!(list(&a.planes), [third, flat, low, high]);
    let points = list(&a.points);
    let met: Vec<[u32; 3]> = rows(&a.vertices.point)
        .iter()
        .map(|v| match points[*v as usize] {
            PointKind::Meet(planes) => planes.map(geom_topo::Ix::row),
            PointKind::Grid(p) => panic!("{p:?} is a grid point"),
        })
        .collect();
    assert_eq!(met, [[0, 1, 2], [0, 1, 3]]);
    assert_eq!(rows(&a.edges.start), [0]);
    assert_eq!(rows(&a.edges.end), [1]);
    assert_eq!(rows(&a.edges.first_fin), [NONE]);
}

#[test]
fn a_closed_merge_that_keeps_nothing_is_a_general_body() {
    // A solid body holds a solid region, so a selection that keeps nothing is a general one.
    let none = Merged {
        sheets: Vec::new(),
        vertices: Vec::new(),
        edges: Vec::new(),
        fins: Vec::new(),
        faces: Vec::new(),
        shells: Vec::new(),
        piece_face: Vec::new(),
    };
    let empty = publish_with(&none, &[], false, (None, None), &mut meter()).unwrap();
    assert_eq!(empty.kind(), BodyKind::General);
    assert_eq!(list(&empty.arenas().regions.kind), [RegionKind::Void]);
    assert_eq!(rows(&empty.arenas().regions.first_shell), [NONE]);
}

/// `count` names, the first row's the highest, so publishing in name order turns each arena round.
fn backwards(count: usize) -> Vec<NameKey> {
    (0..count)
        .map(|i| NameKey([(count - 1 - i) as u8; 16]))
        .collect()
}

#[test]
fn names_put_a_published_body_in_their_order() {
    let m = merged(&[([0, 0, 0], [2, 2, 2])], &WindingTest::unite());
    let carriers = carriers(&m, Sense::Forward);
    let plain = publish_with(&m, &carriers, false, (None, None), &mut meter()).unwrap();
    let keys = Names {
        regions: backwards(plain.arenas().regions.kind.iter().count()),
        faces: backwards(m.faces.len()),
        edges: backwards(m.edges.len()),
        vertices: backwards(m.vertices.len()),
    };
    let named = publish_with(&m, &carriers, false, (Some(&keys), None), &mut meter()).unwrap();
    assert_eq!(named.kind(), BodyKind::Solid);
    let mut bounds = list(&plain.arenas().faces.bound);
    bounds.reverse();
    assert_eq!(list(&named.arenas().faces.bound), bounds);
    let held = named.names().unwrap();
    assert!(held.faces.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(held.vertices.len(), m.vertices.len());
}

#[test]
fn a_rounding_given_is_the_one_published() {
    let m = leaning(&WindingTest::unite());
    let carriers = carriers(&m, Sense::Forward);
    let own = publish_with(&m, &carriers, false, (None, None), &mut meter()).unwrap();
    let given: Vec<_> = m.vertices.iter().map(Tpi::to_grid).collect();
    let same = publish_with(&m, &carriers, false, (None, Some(&given)), &mut meter()).unwrap();
    assert_eq!(list(&same.arenas().points), list(&own.arenas().points));
    assert_eq!(list(&same.arenas().planes), list(&own.arenas().planes));
    // A list shorter than the vertices leaves the rest to round themselves.
    let short = publish_with(
        &m,
        &carriers,
        false,
        (None, Some(&given[..1])),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(list(&short.arenas().points), list(&own.arenas().points));
    // A rounding that was refused is the publication's refusal.
    let mut refused = given.clone();
    refused[0] = Err(NumError::Degenerate);
    let err = publish_with(&m, &carriers, false, (None, Some(&refused)), &mut meter()).unwrap_err();
    assert_eq!(err, PublishError::Num(NumError::Degenerate));
}

#[test]
fn a_wire_edge_shorter_than_the_grid_stands_a_step_from_its_grid_end_and_names_order_it() {
    // The origin, on the grid, and the point a quarter step along x, which rounds to it: the edge
    // between them stands a step long from the origin.
    let (x, y, z) = (up(0, 0), up(1, 0), up(2, 0));
    let quarter = Plane::new([4, 0, 0], -1).unwrap();
    let curves = Curves {
        vertices: vec![Tpi::new(x, y, z).unwrap(), Tpi::new(quarter, y, z).unwrap()],
        edges: vec![(0, 1)],
        meets: vec![vec![0, 1]],
    };
    let b = wire(&curves, None, &mut meter()).unwrap();
    let a = b.arenas();
    let origin = P3::new(0, 0, 0).unwrap();
    let points = list(&a.points);
    assert_eq!(points[0], PointKind::Grid(origin));
    assert!(matches!(points[1], PointKind::Meet(_)));
    assert_eq!(list(&a.planes), [quarter, y, z]);
    let line = Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    let curves_held: Vec<&CurveKind<3>> = a.curves.iter().collect();
    assert_eq!(*curves_held[0], CurveKind::Line(line));
    // Named the other way round, the two vertices change rows and the edge runs between them.
    let keys = Names {
        regions: backwards(1),
        faces: Vec::new(),
        edges: backwards(1),
        vertices: backwards(2),
    };
    let named = wire(&curves, Some(&keys), &mut meter()).unwrap();
    assert_eq!(named.kind(), BodyKind::Wire);
    let a = named.arenas();
    assert_eq!(rows(&a.edges.start), [1]);
    assert_eq!(rows(&a.edges.end), [0]);
    let held = named.names().unwrap();
    assert!(held.vertices.windows(2).all(|pair| pair[0] < pair[1]));
}

/// The first three of `list`, by number, that meet in a point.
fn lowest_of(list: &[usize], planes: &[Plane]) -> [Plane; 3] {
    for (i, a) in list.iter().enumerate() {
        for (j, b) in list.iter().enumerate().skip(i + 1) {
            for c in list.iter().skip(j + 1) {
                if Tpi::new(planes[*a], planes[*b], planes[*c]).is_ok() {
                    return [planes[*a], planes[*b], planes[*c]];
                }
            }
        }
    }
    panic!("no three planes meet");
}

#[test]
fn solids_that_share_only_an_edge_make_a_general_body() {
    let m = merged(
        &[([0, 0, 0], [2, 2, 2]), ([2, 2, 0], [4, 4, 2])],
        &WindingTest::unite(),
    );
    assert!(m.edges.iter().any(|e| e.fins.len() == 4));
    assert_eq!(body(&m, Sense::Forward).kind(), BodyKind::General);
}

#[test]
fn a_union_whose_shell_pinches_at_a_corner_is_a_general_body() {
    // Each pair of boxes shares part of a face, and around (-1, 4, 0) the empty space is two
    // opposite octants: every edge has two fins, but the one shell goes round that vertex in two
    // fans, so it is non-manifold there (11.6) and the body is a general one.
    let m = merged(
        &[
            ([-1, 1, -1], [0, 4, 1]),
            ([-2, 4, -1], [0, 5, 0]),
            ([-2, 0, 0], [-1, 5, 1]),
        ],
        &WindingTest::unite(),
    );
    assert_eq!(m.shells.len(), 1);
    assert!(m.edges.iter().all(|e| e.fins.len() == 2));
    assert_eq!(body(&m, Sense::Forward).kind(), BodyKind::General);
}

#[test]
fn a_carrier_missing_is_reported() {
    let m = merged(&[([0, 0, 0], [2, 2, 2])], &WindingTest::unite());
    assert_eq!(
        publish(&m, &[], &mut meter()).unwrap_err(),
        PublishError::Broken
    );
}
