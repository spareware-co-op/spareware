//! Convexity along a chain (G262, chapter 13.10, "Feasibility first", step 2): the exact test on
//! planar edges, the cut at a flip, the refusal of a tangent stretch, and the corner counts.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_blend::chain::EdgeId;
use geom_blend::convexity::{
    Convexity, ConvexityError, PlaneEdge, Run, Section, SectionKind, classify, corner_class,
    plane_convexity,
};
use geom_num::interval::Interval;

fn at(x: f64) -> Interval {
    Interval::point(x).unwrap()
}

fn between(lo: f64, hi: f64) -> Interval {
    Interval::new(lo, hi).unwrap()
}

fn bent(c: Convexity, from: f64, to: f64) -> Section {
    Section {
        kind: SectionKind::Bent(c),
        from: at(from),
        to: at(to),
    }
}

fn tangent(from: Interval, to: Interval) -> Section {
    Section {
        kind: SectionKind::Tangent,
        from,
        to,
    }
}

#[test]
fn the_edges_of_a_box_are_convex_and_the_edges_of_a_corner_notch_are_concave() {
    // The top back edge of the unit-scaled box: top face normal +z, back face normal +y, the edge
    // walked as the top face's boundary walks it, in -x.
    let top = [0, 0, 1];
    let back = [0, 1, 0];
    assert_eq!(
        plane_convexity(top, back, [-4, 0, 0]).unwrap(),
        PlaneEdge::Convex
    );
    // The same edge walked the other way is the other face's: swapping the faces and flipping the
    // direction gives the same answer.
    assert_eq!(
        plane_convexity(back, top, [4, 0, 0]).unwrap(),
        PlaneEdge::Convex
    );
    // The inside corner of an L: the floor's normal +z and the wall's normal +x meet concavely.
    let floor = [0, 0, 1];
    let wall = [1, 0, 0];
    assert_eq!(
        plane_convexity(floor, wall, [0, -7, 0]).unwrap(),
        PlaneEdge::Concave
    );
}

#[test]
fn parallel_normals_are_tangent_or_opposed() {
    assert_eq!(
        plane_convexity([0, 0, 5], [0, 0, 2], [1, 0, 0]).unwrap(),
        PlaneEdge::Tangent
    );
    assert_eq!(
        plane_convexity([0, 0, 5], [0, 0, -2], [1, 0, 0]).unwrap(),
        PlaneEdge::Opposed
    );
}

#[test]
fn the_test_is_exact_at_the_largest_normals_and_refuses_what_does_not_fit() {
    let big = (1_i128 << 83) - 1;
    // Two nearly parallel huge normals: the cross product is tiny but not zero, and its sign
    // decides.
    let first = [big, big - 1, 1];
    let second = [big - 1, big - 2, 1];
    let result = plane_convexity(first, second, [1 << 40, 0, 1]).unwrap();
    assert!(matches!(result, PlaneEdge::Convex | PlaneEdge::Concave));
    let flipped = plane_convexity(second, first, [1 << 40, 0, 1]).unwrap();
    assert_ne!(result, flipped);
    // Past the budget: refused.
    assert!(
        plane_convexity(
            [i128::MAX, 0, 0],
            [0, i128::MAX, 0],
            [i128::MAX, i128::MAX, i128::MAX]
        )
        .is_err()
    );
}

#[test]
fn an_edge_of_one_convexity_is_one_run_and_a_tangent_point_inside_it_is_no_split() {
    let edge = EdgeId(3);
    let sections = [
        bent(Convexity::Convex, 0.0, 4.0),
        tangent(at(4.0), at(4.0)),
        bent(Convexity::Convex, 4.0, 9.0),
    ];
    let found = classify(edge, &sections).unwrap();
    assert_eq!(found.pinches, []);
    assert_eq!(
        found.runs,
        [Run {
            edge,
            convexity: Convexity::Convex,
            from: at(0.0),
            to: at(9.0)
        }]
    );
}

#[test]
fn a_flip_at_a_tangent_point_cuts_the_edge_and_pinches_the_blend_there() {
    let edge = EdgeId(8);
    let sections = [
        bent(Convexity::Convex, 0.0, 3.9),
        tangent(between(3.9, 4.0), between(4.0, 4.1)),
        bent(Convexity::Concave, 4.1, 10.0),
    ];
    let found = classify(edge, &sections).unwrap();
    assert_eq!(found.runs.len(), 2);
    assert_eq!(found.runs[0].convexity, Convexity::Convex);
    assert_eq!(found.runs[1].convexity, Convexity::Concave);
    assert_eq!(found.runs[0].to, at(3.9));
    assert_eq!(found.runs[1].from, at(4.1));
    assert_eq!(found.pinches.len(), 1);
    let pinch = found.pinches[0];
    assert_eq!(pinch.edge, edge);
    assert!(
        pinch.at.lo() <= 3.9 && pinch.at.hi() >= 4.1,
        "{:?}",
        pinch.at
    );
}

#[test]
fn a_flip_with_no_tangent_section_between_cuts_at_the_boundary_between_the_runs() {
    let sections = [
        bent(Convexity::Concave, 0.0, 5.0),
        bent(Convexity::Convex, 5.0, 8.0),
    ];
    let found = classify(EdgeId(1), &sections).unwrap();
    assert_eq!(found.runs.len(), 2);
    assert_eq!(found.pinches.len(), 1);
    assert!(found.pinches[0].at.contains(5.0));
}

#[test]
fn faces_tangent_along_a_stretch_make_the_edge_mixed_and_it_is_refused() {
    let edge = EdgeId(2);
    // The tangent section's two ends are proven apart: a stretch, not a point.
    let sections = [
        bent(Convexity::Convex, 0.0, 3.0),
        tangent(between(3.0, 3.1), between(5.0, 5.1)),
        bent(Convexity::Convex, 5.1, 9.0),
    ];
    assert_eq!(
        classify(edge, &sections),
        Err(ConvexityError::MixedConvexity(edge))
    );
    // A stretch at the end of the edge is refused too.
    let at_end = [bent(Convexity::Convex, 0.0, 3.0), tangent(at(3.0), at(9.0))];
    assert_eq!(
        classify(edge, &at_end),
        Err(ConvexityError::MixedConvexity(edge))
    );
}

#[test]
fn an_edge_with_nothing_bent_has_nothing_to_blend() {
    let edge = EdgeId(5);
    assert_eq!(
        classify(edge, &[]),
        Err(ConvexityError::NothingToBlend(edge))
    );
    assert_eq!(
        classify(edge, &[tangent(at(1.0), at(1.0))]),
        Err(ConvexityError::NothingToBlend(edge))
    );
}

#[test]
fn a_corner_counts_its_convex_and_concave_edges() {
    use Convexity::{Concave, Convex};
    let c = corner_class(&[Convex, Convex, Convex]);
    assert_eq!((c.edges, c.convex, c.concave, c.mixed()), (3, 3, 0, false));
    let m = corner_class(&[Convex, Concave, Concave]);
    assert_eq!((m.edges, m.convex, m.concave, m.mixed()), (3, 1, 2, true));
    assert_eq!(corner_class(&[]).edges, 0);
}
