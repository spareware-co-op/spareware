//! Regions two edges' intersection could not decide (G243, chapter 11.4 steps 2 and 3): held
//! until the common blocks are known, and accepted only where common blocks pairing its two edges cover the region on
//! both its edges, or where it touches a vertex both edges pass through. Two circles of radius
//! 1105 whose centres lie 2210 steps apart, the second's radius a double's step past 1105, are
//! such a region: they overlap by a fraction of a step's billionth around (1105, 0), and their
//! closed form cannot hold the two crossings apart, so it makes them one tangent band there.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::PointGeom;
use geom_bool::distance::FIRST_FLOOR;
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, Entity, Interfered, Interference, VertexIn, interfere,
};
use geom_bool::merge::default_remaining;
use geom_bool::rule::REFINEMENTS;
use geom_curve::geom_nurbs::{Basis, Curve};
use geom_curve::{Circle, CurveKind, Frame, ParamCurve};
use geom_intersect::curve::{curves, curves_over};
use geom_intersect::project::{Projection, project_curve};
use geom_intersect::{Item, ParamBox, Seam, Unresolved};
use geom_num::P3;
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 30))
}

/// The first circle, about the origin, starting from `start` (a unit vector in z = 0).
fn first(start: [f64; 2]) -> Circle<3> {
    let frame = Frame::new([start[0], start[1], 0.0], [0.0, 0.0, 1.0]).unwrap();
    Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 2.0)).unwrap()
}

/// The second circle, about (2210, 0), a double's step wider.
fn second() -> Circle<3> {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    Circle::new([2210.0, 0.0, 0.0], 1105.0_f64.next_up(), frame, (0.0, 2.0)).unwrap()
}

/// Closed edges on `curves`, each of its own operand from 0, then grid vertices of the operand
/// after them.
fn arrangement(curves: Vec<CurveKind<3>>, vertices: &[[i64; 2]]) -> Arrangement {
    let mut arr = Arrangement::default();
    for (operand, curve) in (0..).zip(curves) {
        let curve = arr.carriers.add_curve(curve).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends: None,
            operand,
        });
    }
    let operand = u32::try_from(arr.edges.len()).unwrap();
    for &[x, y] in vertices {
        arr.vertices.push(VertexIn {
            point: PointGeom::Grid(P3::new(x, y, 0).unwrap()),
            operand,
        });
    }
    arr
}

fn run(arr: &Arrangement) -> Result<Interfered, BuildError> {
    interfere(arr, default_remaining(), &Exec::serial(), &mut meter())
}

/// The refused pair and why.
fn refused(got: Result<Interfered, BuildError>) -> (Entity, Entity, Unresolved) {
    match got {
        Err(BuildError::Unresolved {
            first, second, why, ..
        }) => (first, second, why),
        other => panic!("{other:?}"),
    }
}

/// The two circles alone: the band touches no vertex and no block covers it, so it is refused.
/// With a vertex at (1105, 0), which lies on both circles to within what an enclosure tells, the
/// band touches a vertex both pass through and stands.
#[test]
fn a_band_stands_only_at_a_vertex_both_circles_pass_through() {
    let curves = || {
        vec![
            CurveKind::Circle(first([1.0, 0.0])),
            CurveKind::Circle(second()),
        ]
    };
    let (a, b, why) = refused(run(&arrangement(curves(), &[])));
    assert_eq!((a, b), (Entity::Edge(0), Entity::Edge(1)));
    assert!(matches!(why, Unresolved::TangentBand { .. }), "{why:?}");
    let out = run(&arrangement(curves(), &[[1105, 0]])).unwrap();
    let v = out.merged[0];
    for edge in [0, 1] {
        assert!(
            out.interferences
                .contains(&Interference::VertexEdge { vertex: v, edge }),
            "{:?}",
            out.interferences
        );
    }
}

/// (1104, 47) lies on the first circle and two steps off the second, (1106, 47) on the second and
/// two steps off the first: each is a vertex of one circle only, near the band but no vertex both
/// pass through, so the band is still refused.
#[test]
fn a_band_near_a_vertex_of_one_circle_only_is_refused() {
    let curves = vec![
        CurveKind::Circle(first([1.0, 0.0])),
        CurveKind::Circle(second()),
    ];
    let arr = arrangement(curves, &[[1104, 47], [1106, 47]]);
    let (a, b, _) = refused(run(&arr));
    assert_eq!((a, b), (Entity::Edge(0), Entity::Edge(1)));
}

/// The first circle's half from (0, −1105) through (1105, 0) to (0, 1105) as an arc with its own
/// ends, and a third operand's NURBS copy of that arc between the same two points: one block by the
/// contact rule, which covers the band on the arc and on the copy. The second circle, started so
/// that the band lies halfway round it, is in no block, and a band needs a block on both its edges,
/// so it is refused.
#[test]
fn a_band_needs_a_block_on_both_its_edges() {
    let frame = Frame::new([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let half = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 1.0)).unwrap();
    let copy = CurveKind::Circle(half.clone())
        .to_nurbs(&mut meter())
        .unwrap()
        .value
        .curve;
    let frame = Frame::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let round = Circle::new([2210.0, 0.0, 0.0], 1105.0_f64.next_up(), frame, (0.0, 2.0)).unwrap();
    let mut arr = Arrangement::default();
    for (operand, curve) in [
        (0, CurveKind::Circle(half)),
        (1, CurveKind::Circle(round)),
        (2, CurveKind::Nurbs(copy)),
    ] {
        let ends = (operand != 1).then(|| {
            [-1105, 1105].map(|y| {
                arr.vertices.push(VertexIn {
                    point: PointGeom::Grid(P3::new(0, y, 0).unwrap()),
                    operand,
                });
                arr.vertices.len() - 1
            })
        });
        let curve = arr.carriers.add_curve(curve).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends,
            operand,
        });
    }
    let (a, b, _) = refused(run(&arr));
    assert!([a, b].contains(&Entity::Edge(1)), "{a:?} {b:?}");
    // Without the second circle the arc and its copy are one block and their region stands.
    arr.edges.remove(1);
    let out = run(&arr).unwrap();
    assert_eq!(out.common.len(), 1, "{:?}", out.common);
}

/// The two circles bound discs of two operands on the plane z = 0, and a third operand's
/// triangle inside the first disc has its corner at (1105, 0), where the band stands: the edge
/// stage makes the corner the circles' touch. The triangle's sides leave the corner into the disc
/// at a steep angle to the circles' tangent there, along the circle's way on one side and against
/// it on the other: no touch of theirs, so they are ordered round the corner by their tangents,
/// not by how they bend (which would put the straight side on the wrong side of the arc), and the
/// first disc splits into the triangle and the rest, whose loop runs round the circle and back
/// round the triangle through the corner.
#[test]
fn sides_leaving_a_touch_of_two_other_edges_go_by_their_tangents() {
    use geom_bool::carrier::CarrierIx;
    use geom_bool::interfere::{EdgeUse, FaceIn};
    use geom_bool::split::split;
    use geom_curve::Line;
    use geom_num::plane::Plane;

    let mut arr = arrangement(
        vec![
            CurveKind::Circle(first([1.0, 0.0])),
            CurveKind::Circle(second()),
        ],
        &[[1105, 0], [905, 600], [905, -600]],
    );
    // The triangle's corners, the vertices made above, in order counter-clockwise.
    let corners = [[1105.0, 0.0], [905.0, 600.0], [905.0, -600.0]];
    for k in 0..3 {
        let next = (k + 1) % 3;
        let ([ox, oy], [ex, ey]) = (corners[k], corners[next]);
        let line = Line::new([ox, oy, 0.0], [ex - ox, ey - oy, 0.0], (0.0, 1.0)).unwrap();
        let curve = arr.carriers.add_curve(CurveKind::Line(line)).unwrap();
        arr.edges.push(EdgeIn {
            curve,
            ends: Some([k, next]),
            operand: 2,
        });
    }
    let plane = arr
        .carriers
        .add_plane(&Plane::new([0, 0, 1], 0).unwrap())
        .unwrap();
    for (operand, loop_) in [(0, vec![0]), (1, vec![1]), (2, vec![2, 3, 4])] {
        arr.faces.push(FaceIn {
            surface: CarrierIx::Plane(plane),
            loops: vec![
                loop_
                    .into_iter()
                    .map(|edge| EdgeUse {
                        edge,
                        forward: true,
                    })
                    .collect(),
            ],
            operand,
        });
    }
    let out = run(&arr).unwrap();
    assert_eq!(out.touches.len(), 1, "{:?}", out.touches);
    let splitting = split(&arr, &out, &mut meter()).unwrap();
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let mut loops: Vec<usize> = splitting
        .faces
        .iter()
        .filter(|s| s.face == 0)
        .map(|s| s.outer.len())
        .collect();
    loops.sort_unstable();
    assert_eq!(loops, vec![3, 4], "{:?}", splitting.faces);
    for f in [1, 2] {
        assert_eq!(splitting.faces.iter().filter(|s| s.face == f).count(), 1);
    }
}

/// An edge of `operand` along `curve` between new grid vertices of its own at `ends`.
fn between(arr: &mut Arrangement, curve: CurveKind<3>, ends: [[i64; 2]; 2], operand: u32) -> usize {
    let ends = Some(ends.map(|[x, y]| {
        arr.vertices.push(VertexIn {
            point: PointGeom::Grid(P3::new(x, y, 0).unwrap()),
            operand,
        });
        arr.vertices.len() - 1
    }));
    let curve = arr.carriers.add_curve(curve).unwrap();
    arr.edges.push(EdgeIn {
        curve,
        ends,
        operand,
    });
    arr.edges.len() - 1
}

/// The first circle's arc from (0, −1105) turning `turns` half-turns counter-clockwise.
fn arc(turns: f64) -> Circle<3> {
    let frame = Frame::new([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, turns)).unwrap()
}

/// A NURBS copy of a circle or arc.
fn copy(c: &Circle<3>) -> CurveKind<3> {
    CurveKind::Nurbs(
        CurveKind::Circle(c.clone())
            .to_nurbs(&mut meter())
            .unwrap()
            .value
            .curve,
    )
}

/// The first circle's half from (0, −1105) to (0, 1105) and its NURBS copy, of two operands, with
/// a third operand's vertex at (1105, 0) halfway along: the vertex cuts both, and the two halves
/// of each are two common blocks, which between them cover the region where the arc and its copy
/// overlap, on both. A fourth operand's segment far off, whose parameter runs over the arc's, is
/// in no block and changes nothing.
#[test]
fn an_overlap_cut_into_two_blocks_by_a_vertex_stands() {
    let mut arr = Arrangement::default();
    between(
        &mut arr,
        CurveKind::Circle(arc(1.0)),
        [[0, -1105], [0, 1105]],
        0,
    );
    between(&mut arr, copy(&arc(1.0)), [[0, -1105], [0, 1105]], 2);
    arr.vertices.push(VertexIn {
        point: PointGeom::Grid(P3::new(1105, 0, 0).unwrap()),
        operand: 3,
    });
    let far = geom_curve::Line::new([5000.0, 0.0, 0.0], [1000.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    between(&mut arr, CurveKind::Line(far), [[5000, 0], [6000, 0]], 4);
    let out = run(&arr).unwrap();
    assert_eq!(out.common.len(), 2, "{:?}", out.common);
    assert!(
        out.common.iter().all(|c| c.members.len() == 2),
        "{:?}",
        out.common
    );
}

/// The first circle's half through (1105, 0) and its NURBS copy, and the second circle's half
/// through its own leftmost point and its copy, of four operands: each arc is one block with its
/// copy, but no block holds an edge of each circle, so the band where the circles touch is
/// covered on neither and is refused.
#[test]
fn a_band_between_two_blocks_of_other_edges_is_refused() {
    let mut arr = Arrangement::default();
    between(
        &mut arr,
        CurveKind::Circle(arc(1.0)),
        [[0, -1105], [0, 1105]],
        0,
    );
    between(&mut arr, copy(&arc(1.0)), [[0, -1105], [0, 1105]], 1);
    let frame = Frame::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let other = Circle::new([2210.0, 0.0, 0.0], 1105.0_f64.next_up(), frame, (0.0, 1.0)).unwrap();
    between(
        &mut arr,
        CurveKind::Circle(other.clone()),
        [[2210, 1105], [2210, -1105]],
        2,
    );
    between(&mut arr, copy(&other), [[2210, 1105], [2210, -1105]], 3);
    let (a, b, _) = refused(run(&arr));
    let first = [Entity::Edge(0), Entity::Edge(1)];
    let second = [Entity::Edge(2), Entity::Edge(3)];
    assert!(
        first.contains(&a) && second.contains(&b) || first.contains(&b) && second.contains(&a),
        "{a:?} {b:?}"
    );
}

/// The first circle's half from (0, −1105) to (0, 1105), and a NURBS copy of three quarters of it
/// from the same start on to (−1105, 0): the arc's end cuts the copy, whose first piece is one
/// block with the arc. The region where they overlap ends, on the copy, in a box that straddles
/// the pave at the arc's end. Past that pave's enclosure the two edges are proven apart, so the
/// region does not reach into the copy's second piece, which no block holds, and stands.
#[test]
fn a_region_straddling_the_pave_at_an_overlap_s_end_stands() {
    let mut arr = Arrangement::default();
    between(
        &mut arr,
        CurveKind::Circle(arc(1.0)),
        [[0, -1105], [0, 1105]],
        0,
    );
    between(&mut arr, copy(&arc(1.5)), [[0, -1105], [-1105, 0]], 2);
    let out = run(&arr).unwrap();
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![0, 1]], "{:?}", out.common);
}

/// The arc and its copy of three quarters above, with a fourth operand's vertex at (−663, 884) on
/// the copy past the arc's end: it cuts the copy's second piece in two, and the piece from it on
/// is in no block either. The region ends in the box that straddles the pave at the arc's end,
/// well short of the vertex, so it lies wholly before that piece, reaches no part of it, and
/// stands as without the cut.
#[test]
fn a_region_straddling_the_pave_at_an_overlap_s_end_stands_before_a_later_cut() {
    let mut arr = Arrangement::default();
    between(
        &mut arr,
        CurveKind::Circle(arc(1.0)),
        [[0, -1105], [0, 1105]],
        0,
    );
    between(&mut arr, copy(&arc(1.5)), [[0, -1105], [-1105, 0]], 2);
    arr.vertices.push(VertexIn {
        point: PointGeom::Grid(P3::new(-663, 884, 0).unwrap()),
        operand: 3,
    });
    let out = run(&arr).unwrap();
    assert_eq!(out.splits.len(), 4, "{:?}", out.splits);
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![0, 1]], "{:?}", out.common);
}

/// The first circle's upper half, from (1105, 0) to (−1105, 0), and a NURBS curve that follows it
/// from (1105, 0) to (0, 1105) and runs on straight along its tangent there to (−600, 1105), of two
/// operands, with a third operand's vertex at (663, 884) on both. The vertex cuts both, and their
/// pieces before it are one block. Past it the two still overlap, to (0, 1105) and in the tangent
/// band beyond, but their pieces there end at different vertices and are in no block. The region
/// straddles the vertex's pave on the circle, and on the part past it the two edges cannot be
/// told apart, so it reaches into the circle's second piece and is refused.
#[test]
fn a_region_overlapping_past_a_pave_into_a_piece_of_no_block_is_refused() {
    let mut arr = Arrangement::default();
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let upper = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 1.0)).unwrap();
    between(
        &mut arr,
        CurveKind::Circle(upper),
        [[1105, 0], [-1105, 0]],
        0,
    );
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0]).unwrap();
    let points = [
        [1105.0, 0.0, 0.0],
        [1105.0, 1105.0, 0.0],
        [0.0, 1105.0, 0.0],
        [-300.0, 1105.0, 0.0],
        [-600.0, 1105.0, 0.0],
    ];
    let weights = [1.0, core::f64::consts::FRAC_1_SQRT_2, 1.0, 1.0, 1.0];
    let onward = Curve::new_rational(basis, &points, &weights).unwrap();
    between(
        &mut arr,
        CurveKind::Nurbs(onward),
        [[1105, 0], [-600, 1105]],
        1,
    );
    arr.vertices.push(VertexIn {
        point: PointGeom::Grid(P3::new(663, 884, 0).unwrap()),
        operand: 2,
    });
    let (a, b, _) = refused(run(&arr));
    assert_eq!((a, b), (Entity::Edge(0), Entity::Edge(1)));
}

/// The first circle whole, a closed edge starting from (1105, 0), and a NURBS copy of its right
/// half from (0, −1105) to (0, 1105), of another operand: the copy's ends cut the circle into its
/// left half and its right half, which runs round the seam. The right half is one block with the
/// copy, and the region where they overlap, which reaches into the right half on both sides of
/// the seam, stands.
#[test]
fn a_copy_of_the_half_across_a_circle_s_seam_is_one_block_with_it() {
    let mut arr = arrangement(vec![CurveKind::Circle(first([1.0, 0.0]))], &[]);
    between(&mut arr, copy(&arc(1.0)), [[0, -1105], [0, 1105]], 1);
    let out = run(&arr).unwrap();
    assert!(out.splits[1].block.wraps, "{:?}", out.splits[1]);
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![1, 2]], "{:?}", out.common);
}

/// The first circle whole, a closed edge starting from (1105, 0), and a NURBS copy of its first
/// quarter from there to (0, 1105), of another operand: the copy's ends cut the circle into that
/// quarter, one block with the copy, and the rest, which runs from (0, 1105) round to the seam and
/// is in no block. The region where the circle and the copy overlap starts at the seam, where the
/// rest ends, and stops short of the pave at (0, 1105). Its span on the circle is kept whole
/// across the join (decision 32.104): it starts in the window a little before the end and passes
/// the seam pave's enclosure, a box at each end of the domain. Before the box at the end the two
/// edges are proven apart, so the region stays out of the rest and stands.
#[test]
fn a_region_from_the_seam_whose_span_runs_back_past_the_join_stands() {
    let mut arr = arrangement(vec![CurveKind::Circle(first([1.0, 0.0]))], &[]);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let quarter = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 0.5)).unwrap();
    between(&mut arr, copy(&quarter), [[1105, 0], [0, 1105]], 1);
    let out = run(&arr).unwrap();
    assert!(out.splits[1].block.wraps, "{:?}", out.splits[1]);
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![0, 2]], "{:?}", out.common);
}

/// The region of the quarter's copy above, asked as the held check asks it: its span on the
/// circle runs across the join and passes both boxes the projection gives the seam vertex
/// (1105, 0), one at each end of the domain. Over the stretch to the domain's end less the box
/// there the circle and the copy are proven apart; over the whole stretch, which holds the vertex,
/// they meet.
#[test]
fn the_region_from_the_seam_passes_both_boxes_of_the_seam_pave_and_is_apart_before_them() {
    let circle = CurveKind::Circle(first([1.0, 0.0]));
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let quarter = copy(&Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 0.5)).unwrap());
    let mut m = meter();
    let met = curves(&circle, &quarter, &mut m).unwrap();
    let regions: Vec<ParamBox> = met
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Flagged { at, .. } => Some(*at),
            _ => None,
        })
        .collect();
    assert_eq!(regions.len(), 1, "{:?}", met.items());
    let (s, t) = (regions[0].spans()[0], regions[0].spans()[1]);
    let [Some(ending), Some(starting)] = Seam::of_curve(&circle).spans(s) else {
        panic!("{s:?}");
    };
    let [Some(other), None] = Seam::of_curve(&quarter).spans(t) else {
        panic!("{t:?}");
    };
    let floor = FIRST_FLOOR / f64::from(1_u32 << REFINEMENTS);
    let Projection::Points(found) =
        project_curve(&circle, &[1105.0, 0.0, 0.0], floor, &mut m).unwrap()
    else {
        panic!("not points");
    };
    let boxes: Vec<Interval> = found.iter().map(|n| n.at[0]).collect();
    assert_eq!(boxes.len(), 2, "{boxes:?}");
    let (near, far) = (boxes[0], boxes[1]);
    assert_eq!((near.lo(), far.hi()), (0.0, 2.0), "{boxes:?}");
    assert!(
        ending.0 < far.lo() && near.hi() < starting.1,
        "{s:?} {boxes:?}"
    );
    let before = curves_over(&circle, (ending.0, far.lo()), &quarter, other, &mut m).unwrap();
    assert!(before.items().is_empty(), "{:?}", before.items());
    let whole = curves_over(&circle, ending, &quarter, other, &mut m).unwrap();
    assert_ne!(whole.items(), []);
}

/// The first circle whole and a NURBS copy of its arc from 2^-14 half-turns past the seam to
/// (0, 1105), of another operand, between grid vertices at (1105, 0) and (0, 1105). The region's
/// span on the circle runs back across the join only as far as the seam pave's box at the
/// domain's end, [2 - 2^-18, 2]: that stretch is inside the seam pave's enclosure and reaches no
/// piece, and the stretch from the start reaches only the piece the copy shares, so it stands.
#[test]
fn a_region_whose_span_runs_back_across_the_join_only_into_the_seam_box_stands() {
    let mut arr = arrangement(vec![CurveKind::Circle(first([1.0, 0.0]))], &[]);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let arc = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (2f64.powi(-14), 0.5)).unwrap();
    between(&mut arr, copy(&arc), [[1105, 0], [0, 1105]], 1);
    let out = run(&arr).unwrap();
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![0, 2]], "{:?}", out.common);
}

/// The first circle whole and a NURBS copy of its last quarter, from (0, -1105) to (1105, 0), of
/// another operand, between grid vertices there. The region's span on the circle runs across the
/// join: the stretch to the domain's end is in the piece the copy shares, and the stretch from the
/// start passes the seam pave's enclosure into the piece from the seam, where the edges are proven
/// apart past the pave's box at the start, so it stands.
#[test]
fn a_region_whose_span_runs_from_a_shared_piece_across_the_join_past_the_seam_pave_stands() {
    let mut arr = arrangement(vec![CurveKind::Circle(first([1.0, 0.0]))], &[]);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let arc = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (1.5, 2.0)).unwrap();
    between(&mut arr, copy(&arc), [[0, -1105], [1105, 0]], 1);
    let out = run(&arr).unwrap();
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![1, 2]], "{:?}", out.common);
}

/// The first circle whole and a NURBS copy of its last quarter that ends `short` half-turns before
/// the join, from (0, −1105) to 1105π·`short` steps short of (1105, 0), of another operand, between
/// grid vertices at (0, −1105) and (1105, 0). As with the exact quarter's copy above, the copy's
/// ends cut the circle into the piece from the seam to (0, −1105) and the piece from there round
/// to the join, where the seam pave stands at the domain's start. Inside its paves that piece is
/// the one stretch to the join, which the copy follows to well within the contact radius, so the
/// two are one block, which covers the region where they overlap, and it stands.
fn the_last_quarter_short_of_the_join(short: f64) {
    let mut arr = arrangement(vec![CurveKind::Circle(first([1.0, 0.0]))], &[]);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let arc = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (1.5, 2.0 - short)).unwrap();
    between(&mut arr, copy(&arc), [[0, -1105], [1105, 0]], 1);
    let out = run(&arr).unwrap();
    assert!(out.splits[1].block.wraps, "{:?}", out.splits[1]);
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![1, 2]], "{:?}", out.common);
}

/// The last quarter's copy ending at 2 − 2^-52, the double just before the join.
#[test]
fn the_last_quarter_ending_2_to_the_minus_52_short_of_the_join_stands() {
    the_last_quarter_short_of_the_join(2f64.powi(-52));
}

/// The last quarter's copy ending 2^-40 half-turns before the join.
#[test]
fn the_last_quarter_ending_2_to_the_minus_40_short_of_the_join_stands() {
    the_last_quarter_short_of_the_join(2f64.powi(-40));
}

/// The last quarter's copy ending 2^-20 half-turns before the join.
#[test]
fn the_last_quarter_ending_2_to_the_minus_20_short_of_the_join_stands() {
    the_last_quarter_short_of_the_join(2f64.powi(-20));
}

/// The last quarter's copy ending 2^-12 half-turns before the join, about 0.85 steps short of the
/// vertex.
#[test]
fn the_last_quarter_ending_2_to_the_minus_12_short_of_the_join_stands() {
    the_last_quarter_short_of_the_join(2f64.powi(-12));
}

/// The first circle whole and a NURBS copy of its upper half, from (1105, 0) to (−1105, 0): the
/// copy's ends cut the circle into the upper half, one block with the copy, and the lower half,
/// which runs from (−1105, 0) round to the seam and is in no block. The region where the circle
/// and the copy overlap ends, on the circle, in a box just past the pave at (−1105, 0). Past that
/// pave's enclosure the two edges are proven apart, so the region stays out of the lower half and
/// stands.
#[test]
fn a_region_ending_in_a_box_past_the_pave_of_a_piece_that_runs_to_the_seam_stands() {
    let mut arr = arrangement(vec![CurveKind::Circle(first([1.0, 0.0]))], &[]);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let upper = Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, 1.0)).unwrap();
    between(&mut arr, copy(&upper), [[1105, 0], [-1105, 0]], 1);
    let out = run(&arr).unwrap();
    assert!(out.splits[1].block.wraps, "{:?}", out.splits[1]);
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![0, 2]], "{:?}", out.common);
}

/// The first circle whole; a whole circle about (1105, −1105) whose radius is 14 doubles' steps
/// short of 1105, which crosses the first 3.2·10^-12 below its join, the crossing's pave there
/// ending exactly on the domain's end, 2; and an arc from (1100, −105) to (1100, 105) of a circle
/// 10^-10 smaller, its centre 10^-10 off towards 0.05 radians, of a third operand. The arc lies
/// within 10^-12 of the first circle, so the crossing cuts it at the same vertex, and the pieces
/// of the two either side of the vertex are two common blocks, the circle's piece after it running
/// round the join. Near 0.05 radians the two cross twice too close together to hold apart, one
/// tangent band past the join and short of (1100, 105), which reaches only that piece. Read round
/// the join from the pave that ends on its end, the piece holds the band, and it stands.
#[test]
fn a_band_past_the_join_stands_in_the_piece_from_a_pave_ending_on_the_join() {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let cutter = Circle::new(
        [1105.0, -1105.0, 0.0],
        1_104.999_999_999_996_8,
        frame,
        (0.0, 2.0),
    )
    .unwrap();
    let mut arr = arrangement(
        vec![
            CurveKind::Circle(first([1.0, 0.0])),
            CurveKind::Circle(cutter),
        ],
        &[],
    );
    let along = Frame::new(
        [0.995_475_113_122_170_7, -0.095_022_624_434_402_17, 0.0],
        [0.0, 0.0, 1.0],
    )
    .unwrap();
    let inner = Circle::new(
        [9.991_938_772_459_098e-11, 5.000_136_861_576_372e-12, 0.0],
        1_104.999_999_999_9,
        along,
        (0.0, 0.060_584_688_753_478_03),
    )
    .unwrap();
    let met = curves(
        &CurveKind::Circle(first([1.0, 0.0])),
        &CurveKind::Circle(inner.clone()),
        &mut meter(),
    )
    .unwrap();
    match met.items() {
        [
            Item::Flagged {
                at,
                why: Unresolved::TangentBand { .. },
                ..
            },
        ] => {
            let s = at.spans()[0];
            assert!(0.0 < s.lo() && s.hi() < 0.03, "{s:?}");
        }
        other => panic!("{other:?}"),
    }
    let b = between(
        &mut arr,
        CurveKind::Circle(inner),
        [[1100, -105], [1100, 105]],
        2,
    );
    let out = run(&arr).unwrap();
    let join = *out.paves[0].iter().find(|p| p.t.hi() == 2.0).unwrap();
    assert!(join.t.lo() < 2.0, "{join:?}");
    assert!(
        out.paves[b].iter().any(|p| p.vertex == join.vertex),
        "{:?}",
        out.paves[b]
    );
    let from = |e: usize| {
        (0..out.splits.len())
            .find(|&k| {
                out.splits[k].source == e
                    && out.splits[k]
                        .block
                        .from
                        .is_some_and(|p| p.vertex == join.vertex)
            })
            .unwrap()
    };
    let (round, after) = (from(0), from(b));
    assert!(out.splits[round].block.wraps, "{:?}", out.splits[round]);
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert!(members.contains(&vec![round, after]), "{:?}", out.common);
}

/// The first circle's arc from `start`, a point of it, turning `turns` half-turns, counter-clockwise
/// if `ccw`.
fn turning(start: [f64; 2], turns: f64, ccw: bool) -> Circle<3> {
    let normal = if ccw { 1.0 } else { -1.0 };
    let frame = Frame::new([start[0], start[1], 0.0], [0.0, 0.0, normal]).unwrap();
    Circle::new([0.0, 0.0, 0.0], 1105.0, frame, (0.0, turns)).unwrap()
}

/// A vertex of `operand` where the planes x = −47, 3y = 3310 and z = 0 meet, at (−47, 1103⅓): a
/// meeting of three planes off the grid, as a body's vertex can be, so it is committed to
/// (−47, 1103) within a step. That is within a step of the first circle, so the vertex touches an
/// edge along it, and its pave there lies 0.04 steps on from (−47, 1104) counter-clockwise: the
/// piece between them is far shorter than a box at the intersection's floor.
fn off_the_grid(arr: &mut Arrangement, operand: u32) {
    let mut add =
        |n: [i128; 3], d: i128| arr.carriers.add_plane(&Plane::new(n, d).unwrap()).unwrap();
    let planes = [add([1, 0, 0], 47), add([0, 3, 0], -3310), add([0, 0, 1], 0)];
    arr.vertices.push(VertexIn {
        point: PointGeom::Planes(planes),
        operand,
    });
}

/// The first circle's half from (47, −1104) to (−47, 1104), and a NURBS copy of three quarters of
/// it from the same start on to (−1104, −47): the arc's end cuts the copy, whose first piece is one
/// block with the arc, and the region where they overlap ends, on the copy, in a box that
/// straddles the pave at the arc's end. Past that pave's enclosure the two edges are proven apart,
/// so alone the region stands. A vertex of the arc's operand off the grid ([`off_the_grid`]) cuts
/// the copy again just past that pave, within the box: the region runs over the whole of the piece
/// between them, which no block holds, so it reaches into that piece and is refused.
#[test]
fn a_region_over_the_whole_of_a_short_piece_past_an_overlap_s_end_is_refused() {
    let build = |cut: bool| {
        let mut arr = Arrangement::default();
        let half = turning([47.0, -1104.0], 1.0, true);
        between(
            &mut arr,
            CurveKind::Circle(half),
            [[47, -1104], [-47, 1104]],
            0,
        );
        let on = turning([47.0, -1104.0], 1.5, true);
        between(&mut arr, copy(&on), [[47, -1104], [-1104, -47]], 2);
        if cut {
            off_the_grid(&mut arr, 0);
        }
        arr
    };
    let out = run(&build(false)).unwrap();
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![0, 1]], "{:?}", out.common);
    let (a, b, _) = refused(run(&build(true)));
    assert_eq!((a, b), (Entity::Edge(0), Entity::Edge(1)));
}

/// The first circle's half from (47, −1104) to (−47, 1104), and a NURBS copy of three quarters of
/// it running the other way, from (−1104, −47) through (−47, 1104) back to (47, −1104): the arc's
/// end cuts the copy, whose second piece is one block with the arc, and the region where they
/// overlap starts, on the copy, in a box that straddles the pave at the arc's end. Before that
/// pave's enclosure the two edges are proven apart, so alone the region stands. A vertex of the
/// arc's operand off the grid ([`off_the_grid`]) cuts the copy again just before that pave, within
/// the box: the region runs over the whole of the piece between them, which no block holds, so it
/// reaches into that piece and is refused.
#[test]
fn a_region_over_the_whole_of_a_short_piece_before_an_overlap_s_start_is_refused() {
    let build = |cut: bool| {
        let mut arr = Arrangement::default();
        let half = turning([47.0, -1104.0], 1.0, true);
        between(
            &mut arr,
            CurveKind::Circle(half),
            [[47, -1104], [-47, 1104]],
            0,
        );
        let back = turning([-1104.0, -47.0], 1.5, false);
        between(&mut arr, copy(&back), [[-1104, -47], [47, -1104]], 2);
        if cut {
            off_the_grid(&mut arr, 0);
        }
        arr
    };
    let out = run(&build(false)).unwrap();
    let members: Vec<Vec<usize>> = out.common.iter().map(|c| c.members.clone()).collect();
    assert_eq!(members, vec![vec![0, 2]], "{:?}", out.common);
    let (a, b, _) = refused(run(&build(true)));
    assert_eq!((a, b), (Entity::Edge(0), Entity::Edge(1)));
}
