//! Attacks on piece G206: transform and length edits (`geom_2d::edit`; chapter 14.7 of the kernel
//! architecture). Every test tries to break a claim of the piece's done-when: stretch, lengthen,
//! arrays, mirror, scale, align, divide and measure with certified arc length, exact at quarter
//! turns and whole scales.
//!
//! - **Exact placement**: a quarter turn, a mirror, a whole scale and a shift take grid points to
//!   the grid points the definition gives, and the inverse of a unit map undoes it.
//! - **Arcs keep their sweep**: a bulge is unchanged by a turn, a scale or a shift and negated by a
//!   mirror.
//! - **Refusals**: a zero scale, a result past the range, an inexact polar array, an alignment no
//!   whole scale makes, a shape kind not placed.
//! - **Certified lengths**: the interval holds the exact length of a straight run and of a
//!   semicircle, and a placed polyline has the same length.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_2d::edit::{
    EditError, End, Similarity, Window, align, array_polar, array_rectangular, divide, lengthen,
    measure, place, place_polyline, polyline_length, stretch,
};
use geom_2d::entity::{Ends, EntityError, Frac, Polyline, Shape};
use geom_2d::point::Pt2;
use geom_num::grid::{COORD_MAX, P2};

fn p(x: i64, y: i64) -> P2 {
    P2::new(x, y).unwrap()
}

fn frac(n: i64, d: i64) -> Frac {
    Frac::new(n, d).unwrap()
}

fn xy(q: P2) -> (i64, i64) {
    (q.x.get(), q.y.get())
}

fn straight(points: &[(i64, i64)], closed: bool) -> Polyline {
    let v: Vec<P2> = points.iter().map(|&(x, y)| p(x, y)).collect();
    let n = if closed { v.len() } else { v.len() - 1 };
    Polyline::new(v, vec![Frac::new(0, 1).unwrap(); n], None, closed).unwrap()
}

#[test]
fn a_quarter_turn_takes_each_point_where_the_definition_says() {
    let about = p(10, 20);
    for (turns, expect) in [
        (0, (15, 20)),
        (1, (10, 25)),
        (2, (5, 20)),
        (3, (10, 15)),
        (4, (15, 20)),
    ] {
        let m = Similarity::quarter_turns(turns, about).unwrap();
        assert_eq!(xy(m.point(p(15, 20)).unwrap()), expect, "turns {turns}");
        assert_eq!(xy(m.point(about).unwrap()), (10, 20), "the centre is fixed");
    }
}

#[test]
fn a_whole_scale_fixes_its_centre_and_stretches_distances() {
    let m = Similarity::scaling(3, p(1, 1)).unwrap();
    assert_eq!(xy(m.point(p(1, 1)).unwrap()), (1, 1));
    assert_eq!(xy(m.point(p(2, 4)).unwrap()), (4, 10));
    let neg = Similarity::scaling(-2, p(0, 0)).unwrap();
    assert_eq!(xy(neg.point(p(3, -1)).unwrap()), (-6, 2));
    assert_eq!(
        Similarity::scaling(0, p(0, 0)).unwrap_err(),
        EditError::Degenerate
    );
}

#[test]
fn mirrors_fix_their_line_and_swap_the_sides() {
    let row = Similarity::mirror_in_row(5).unwrap();
    assert_eq!(xy(row.point(p(7, 9)).unwrap()), (7, 1));
    assert_eq!(xy(row.point(p(7, 5)).unwrap()), (7, 5));
    let col = Similarity::mirror_in_column(-3).unwrap();
    assert_eq!(xy(col.point(p(2, 9)).unwrap()), (-8, 9));
    assert_eq!(xy(col.point(p(-3, 0)).unwrap()), (-3, 0));
    assert!(row.mirrors() && col.mirrors());
}

#[test]
fn composition_is_doing_one_then_the_other() {
    let a = Similarity::quarter_turns(1, p(2, 3)).unwrap();
    let b = Similarity::mirror_in_column(4).unwrap();
    let c = Similarity::scaling(2, p(-1, 0)).unwrap();
    let both = c.after(&b.after(&a).unwrap()).unwrap();
    for q in [p(0, 0), p(5, -7), p(-100, 33), p(1, 1)] {
        let step = c.point(b.point(a.point(q).unwrap()).unwrap()).unwrap();
        assert_eq!(both.point(q).unwrap(), step);
    }
}

#[test]
fn a_unit_map_is_undone_by_its_inverse_and_a_scale_is_not() {
    let a = Similarity::quarter_turns(3, p(2, 3)).unwrap();
    let b = Similarity::mirror_in_row(-9).unwrap().after(&a).unwrap();
    let c = Similarity::scaling(-1, p(4, 4)).unwrap().after(&b).unwrap();
    for m in [a, b, c] {
        let back = m.inverse().unwrap();
        for q in [p(0, 0), p(17, -4), p(-3, 8)] {
            assert_eq!(back.point(m.point(q).unwrap()).unwrap(), q);
            assert_eq!(m.point(back.point(q).unwrap()).unwrap(), q);
        }
    }
    assert_eq!(
        Similarity::scaling(2, p(0, 0))
            .unwrap()
            .inverse()
            .unwrap_err(),
        EditError::NotExact
    );
}

#[test]
fn a_result_past_the_range_is_refused_not_wrapped() {
    let big = Similarity::scaling(4, p(0, 0)).unwrap();
    assert_eq!(
        big.point(p(COORD_MAX / 2, 0)).unwrap_err(),
        EditError::Range
    );
    let shift = Similarity::translation(COORD_MAX, 0);
    assert_eq!(shift.point(p(1, 0)).unwrap_err(), EditError::Range);
}

#[test]
fn arcs_keep_their_sweep_under_turns_scales_and_shifts_and_flip_under_a_mirror() {
    let poly = Polyline::new(
        vec![p(0, 0), p(10, 0), p(10, 10)],
        vec![frac(1, 2), frac(-3, 5)],
        None,
        false,
    )
    .unwrap();
    for m in [
        Similarity::quarter_turns(1, p(3, 3)).unwrap(),
        Similarity::scaling(5, p(0, 0)).unwrap(),
        Similarity::translation(7, -2),
    ] {
        let placed = place_polyline(&poly, &m).unwrap();
        assert_eq!(placed.bulges(), poly.bulges());
    }
    let mirrored = place_polyline(&poly, &Similarity::mirror_in_row(0).unwrap()).unwrap();
    assert_eq!(mirrored.bulges(), &[frac(-1, 2), frac(3, 5)]);
}

#[test]
fn a_placed_polyline_has_the_vertices_placed_and_scaled_widths() {
    let poly = Polyline::new(
        vec![p(0, 0), p(4, 0)],
        vec![frac(0, 1)],
        Some(vec![(geom_num::grid::Steps(2), geom_num::grid::Steps(3))]),
        false,
    )
    .unwrap();
    let m = Similarity::scaling(3, p(0, 0)).unwrap();
    let placed = place_polyline(&poly, &m).unwrap();
    assert_eq!(xy(placed.vertices()[1]), (12, 0));
    assert_eq!(
        placed.widths().unwrap(),
        &[(geom_num::grid::Steps(6), geom_num::grid::Steps(9))]
    );
}

#[test]
fn a_segment_is_placed_with_its_carrier() {
    let seg = Shape::Line {
        carrier: geom_2d::carrier::Line2::new(0, 1, 0).unwrap(),
        ends: Ends::Segment(Pt2::Grid(p(0, 0)), Pt2::Grid(p(6, 0))),
    };
    let m = Similarity::quarter_turns(1, p(0, 0)).unwrap();
    let Shape::Line {
        carrier,
        ends: Ends::Segment(Pt2::Grid(a), Pt2::Grid(b)),
    } = place(&seg, &m).unwrap()
    else {
        panic!("a segment stays a segment");
    };
    assert_eq!((xy(a), xy(b)), ((0, 0), (0, 6)));
    // Both ends are on the carrier a x + b y = c.
    let ([ca, cb], cc) = carrier.coefficients();
    for q in [a, b] {
        assert_eq!(ca * i128::from(q.x.get()) + cb * i128::from(q.y.get()), cc);
    }
}

#[test]
fn a_kind_not_placed_is_refused_by_its_tag() {
    let c = Shape::Circle(geom_2d::carrier::Circle2::Grid(
        geom_num::arc::Circle2::new(p(0, 0), 5).unwrap(),
    ));
    assert!(matches!(
        place(&c, &Similarity::IDENTITY),
        Err(EditError::Unsupported(_))
    ));
}

#[test]
fn align_finds_the_quarter_turn_and_whole_scale_or_refuses() {
    let m = align(p(0, 0), p(3, 0), p(10, 10), p(10, 16)).unwrap();
    assert_eq!(xy(m.point(p(0, 0)).unwrap()), (10, 10));
    assert_eq!(xy(m.point(p(3, 0)).unwrap()), (10, 16));
    assert_eq!(xy(m.point(p(1, 0)).unwrap()), (10, 12));
    // Not a quarter turn, not a whole multiple, and degenerate.
    assert_eq!(
        align(p(0, 0), p(1, 0), p(0, 0), p(1, 1)).unwrap_err(),
        EditError::NotExact
    );
    assert_eq!(
        align(p(0, 0), p(2, 0), p(0, 0), p(3, 0)).unwrap_err(),
        EditError::NotExact
    );
    assert_eq!(
        align(p(1, 1), p(1, 1), p(0, 0), p(1, 0)).unwrap_err(),
        EditError::Degenerate
    );
}

#[test]
fn stretch_moves_only_the_vertices_in_the_window_and_keeps_the_bulges() {
    let poly = Polyline::new(
        vec![p(0, 0), p(10, 0), p(10, 10), p(0, 10)],
        vec![frac(0, 1), frac(1, 3), frac(0, 1)],
        None,
        false,
    )
    .unwrap();
    let window = Window {
        low: p(5, -1),
        high: p(11, 11),
    };
    let out = stretch(&poly, &window, 4, 1).unwrap();
    let got: Vec<(i64, i64)> = out.vertices().iter().map(|q| xy(*q)).collect();
    assert_eq!(got, [(0, 0), (14, 1), (14, 11), (0, 10)]);
    assert_eq!(out.bulges(), poly.bulges());
    // A window that holds nothing changes nothing.
    let none = Window {
        low: p(100, 100),
        high: p(101, 101),
    };
    assert_eq!(stretch(&poly, &none, 4, 1).unwrap(), poly);
}

#[test]
fn a_rectangular_array_is_row_major_with_the_original_first() {
    let pt = Shape::Point(p(1, 1));
    let out = array_rectangular(&pt, 2, 3, [10, 0], [0, 5]).unwrap();
    let got: Vec<(i64, i64)> = out
        .iter()
        .map(|s| {
            let Shape::Point(q) = s else { panic!() };
            xy(*q)
        })
        .collect();
    assert_eq!(got, [(1, 1), (11, 1), (21, 1), (1, 6), (11, 6), (21, 6)]);
    assert_eq!(
        array_rectangular(&pt, 0, 3, [1, 0], [0, 1]).unwrap_err(),
        EditError::Degenerate
    );
}

#[test]
fn a_polar_array_is_exact_for_one_two_and_four_copies_only() {
    let pt = Shape::Point(p(3, 0));
    let four = array_polar(&pt, p(0, 0), 4).unwrap();
    let got: Vec<(i64, i64)> = four
        .iter()
        .map(|s| {
            let Shape::Point(q) = s else { panic!() };
            xy(*q)
        })
        .collect();
    assert_eq!(got, [(3, 0), (0, 3), (-3, 0), (0, -3)]);
    assert_eq!(array_polar(&pt, p(0, 0), 2).unwrap().len(), 2);
    assert_eq!(array_polar(&pt, p(0, 0), 1).unwrap().len(), 1);
    assert_eq!(
        array_polar(&pt, p(0, 0), 3).unwrap_err(),
        EditError::NotExact
    );
    assert_eq!(
        array_polar(&pt, p(0, 0), 0).unwrap_err(),
        EditError::Degenerate
    );
}

#[test]
fn lengthening_moves_one_end_along_the_carrier_by_primitive_steps() {
    // The direction (4, 2) has the primitive step (2, 1) and the segment holds two of them.
    let (a, b) = (p(0, 0), p(4, 2));
    let (a2, b2) = lengthen(a, b, End::Finish, 3).unwrap();
    assert_eq!((xy(a2), xy(b2)), ((0, 0), (10, 5)));
    let (a3, b3) = lengthen(a, b, End::Start, 1).unwrap();
    assert_eq!((xy(a3), xy(b3)), ((-2, -1), (4, 2)));
    let (_, b4) = lengthen(a, b, End::Finish, -1).unwrap();
    assert_eq!(xy(b4), (2, 1));
    // Shortening through the other end is refused.
    assert_eq!(
        lengthen(a, b, End::Finish, -2).unwrap_err(),
        EditError::Degenerate
    );
    assert_eq!(
        lengthen(a, a, End::Finish, 1).unwrap_err(),
        EditError::Degenerate
    );
}

#[test]
fn dividing_gives_exact_equal_parts() {
    let pts = divide(p(0, 0), p(10, 4), 4).unwrap();
    assert_eq!(pts.len(), 3);
    for (i, r) in pts.iter().enumerate() {
        let ([x, y], w) = r.coordinates();
        let k = i as i128 + 1;
        // (10 k / 4, 4 k / 4) as x/w, y/w
        assert_eq!(x * 4, 10 * k * w);
        assert_eq!(y * 4, 4 * k * w);
    }
    assert_eq!(
        divide(p(0, 0), p(1, 1), 1).unwrap_err(),
        EditError::Degenerate
    );
    assert_eq!(
        divide(p(2, 2), p(2, 2), 3).unwrap_err(),
        EditError::Degenerate
    );
}

#[test]
fn measuring_steps_off_primitive_lengths_not_past_the_end() {
    let pts = measure(p(0, 0), p(12, 8), 1).unwrap();
    // g = 4: points at 1, 2, 3 and 4 primitive steps of (3, 2).
    let got: Vec<(i64, i64)> = pts.iter().map(|q| xy(*q)).collect();
    assert_eq!(got, [(3, 2), (6, 4), (9, 6), (12, 8)]);
    let by3 = measure(p(0, 0), p(12, 8), 3).unwrap();
    assert_eq!(by3.len(), 1);
    assert_eq!(xy(by3[0]), (9, 6));
    assert_eq!(
        measure(p(0, 0), p(0, 0), 1).unwrap_err(),
        EditError::Degenerate
    );
    assert_eq!(
        measure(p(0, 0), p(3, 4), 0).unwrap_err(),
        EditError::Degenerate
    );
}

#[test]
fn the_length_interval_holds_the_exact_length_of_straight_runs() {
    // A 3-4-5 triangle, closed: 3 + 4 + 5 = 12.
    let tri = straight(&[(0, 0), (3, 0), (3, 4)], true);
    let len = polyline_length(&tri).unwrap();
    assert!(len.contains(12.0), "{len:?}");
    assert!(len.hi() - len.lo() < 1e-12);
    // An open run of two unit steps and a diagonal.
    let open = straight(&[(0, 0), (1, 0), (1, 1), (2, 2)], false);
    let len = polyline_length(&open).unwrap();
    assert!(len.contains(2.0 + 2.0_f64.sqrt()) || (len.lo() < 3.4143 && len.hi() > 3.4142));
}

#[test]
fn the_length_interval_holds_a_semicircle() {
    // Bulge 1 is a half circle (tan of a quarter of pi); the chord from (0,0) to (2,0) is its
    // diameter, so the arc is pi.
    let half = Polyline::new(vec![p(0, 0), p(2, 0)], vec![frac(1, 1)], None, false).unwrap();
    let len = polyline_length(&half).unwrap();
    assert!(len.contains(core::f64::consts::PI), "{len:?}");
    assert!(len.hi() - len.lo() < 1e-9);
    // A quarter circle: bulge tan(pi / 8) is irrational, so use the semicircle's mirror and a
    // closed pair of half circles: a full circle of diameter 2.
    let full = Polyline::new(
        vec![p(0, 0), p(2, 0)],
        vec![frac(1, 1), frac(1, 1)],
        None,
        true,
    );
    // Two vertices are too few for a closed polyline: the model refuses it.
    assert!(full.is_err());
}

#[test]
fn a_placed_polyline_has_a_length_that_meets_the_original() {
    let poly = Polyline::new(
        vec![p(0, 0), p(6, 0), p(6, 8)],
        vec![frac(1, 2), frac(0, 1)],
        None,
        false,
    )
    .unwrap();
    let before = polyline_length(&poly).unwrap();
    for m in [
        Similarity::quarter_turns(1, p(5, 5)).unwrap(),
        Similarity::mirror_in_row(3).unwrap(),
        Similarity::translation(100, -40),
    ] {
        let after = polyline_length(&place_polyline(&poly, &m).unwrap()).unwrap();
        assert!(
            after.lo() <= before.hi() && before.lo() <= after.hi(),
            "{before:?} {after:?}"
        );
    }
    // A whole scale multiplies the length by its factor.
    let tripled =
        polyline_length(&place_polyline(&poly, &Similarity::scaling(3, p(0, 0)).unwrap()).unwrap())
            .unwrap();
    assert!(tripled.lo() <= before.hi() * 3.0 && before.lo() * 3.0 <= tripled.hi());
}

#[test]
fn placing_is_deterministic() {
    let poly = straight(&[(0, 0), (9, 1), (4, 7)], true);
    let m = Similarity::quarter_turns(2, p(1, 1)).unwrap();
    assert_eq!(
        place_polyline(&poly, &m).unwrap(),
        place_polyline(&poly, &m).unwrap()
    );
}

// The mutation gate's survivors in edit.rs (#726's judge), each killed through the public API.
// Two more were the same map by a redundant reduction modulo four, which edit.rs no longer makes.

#[test]
fn every_edit_error_says_what_went_wrong() {
    for (e, text) in [
        (
            EditError::Range,
            "the result is off the grid or past its range",
        ),
        (
            EditError::Degenerate,
            "a zero scale or count, or two equal points",
        ),
        (EditError::NotExact, "no exact map gives this"),
        (EditError::Unsupported(7), "shape kind 7 is not placed here"),
        (
            EditError::Entity(EntityError::Fraction),
            "the entity model refused the result: Fraction",
        ),
    ] {
        assert_eq!(e.to_string(), text);
    }
}

#[test]
fn a_quarter_turn_count_is_taken_modulo_four() {
    for about in [p(0, 0), p(3, -2)] {
        for t in 0..4_u8 {
            let once = Similarity::quarter_turns(t, about).unwrap();
            for more in [4, 8, 248] {
                assert_eq!(Similarity::quarter_turns(t + more, about).unwrap(), once);
            }
        }
    }
    assert_eq!(
        Similarity::quarter_turns(4, p(0, 0)).unwrap(),
        Similarity::IDENTITY
    );
}

/// Every composition of two of the eight unit maps about the origin, shifted, against doing one
/// map and then the other, point by point: a mirrored first map with a turned second among them.
#[test]
fn every_composition_of_unit_maps_is_one_then_the_other() {
    let mut maps = Vec::new();
    for t in 0..4_u8 {
        let turn = Similarity::quarter_turns(t, p(0, 0)).unwrap();
        maps.push(turn);
        maps.push(turn.after(&Similarity::mirror_in_row(0).unwrap()).unwrap());
    }
    let maps: Vec<Similarity> = maps
        .iter()
        .enumerate()
        .map(|(i, m)| {
            Similarity::translation(i as i64 - 3, 2 - i as i64)
                .after(m)
                .unwrap()
        })
        .collect();
    let probes = [p(0, 0), p(1, 0), p(0, 1), p(3, -7), p(-5, 2)];
    for first in &maps {
        for then in &maps {
            let both = then.after(first).unwrap();
            assert_eq!(both.mirrors(), first.mirrors() != then.mirrors());
            for q in probes {
                let step = then.point(first.point(q).unwrap()).unwrap();
                assert_eq!(
                    xy(both.point(q).unwrap()),
                    xy(step),
                    "{first:?} then {then:?}"
                );
            }
        }
    }
}

#[test]
fn turns_and_mirrors_undo_each_other_in_composition() {
    let o = p(0, 0);
    let row = Similarity::mirror_in_row(0).unwrap();
    assert_eq!(row.after(&row).unwrap(), Similarity::IDENTITY);
    for t in 0..4_u8 {
        let a = Similarity::quarter_turns(t, o).unwrap();
        let b = Similarity::quarter_turns(4 - t, o).unwrap();
        assert_eq!(a.after(&b).unwrap(), Similarity::IDENTITY, "{t}");
    }
    assert!(
        !Similarity::IDENTITY
            .after(&Similarity::IDENTITY)
            .unwrap()
            .mirrors()
    );
    assert!(row.after(&Similarity::IDENTITY).unwrap().mirrors());
}

#[test]
fn a_half_turn_by_a_negative_scale_is_its_own_inverse() {
    for about in [p(0, 0), p(4, -1)] {
        let half = Similarity::scaling(-1, about).unwrap();
        assert_eq!(half.inverse().unwrap(), half);
        assert_eq!(
            half.after(&half.inverse().unwrap()).unwrap(),
            Similarity::IDENTITY
        );
    }
}

#[test]
fn a_level_segment_is_placed_with_its_carrier() {
    let seg = Shape::Line {
        carrier: geom_2d::carrier::Line2::new(0, 1, 0).unwrap(),
        ends: Ends::Segment(Pt2::Grid(p(0, 0)), Pt2::Grid(p(6, 0))),
    };
    for m in [
        Similarity::IDENTITY,
        Similarity::translation(2, 5),
        Similarity::quarter_turns(2, p(1, 1)).unwrap(),
    ] {
        let Shape::Line {
            carrier,
            ends: Ends::Segment(Pt2::Grid(a), Pt2::Grid(b)),
        } = place(&seg, &m).unwrap()
        else {
            panic!("a segment stays a segment");
        };
        assert_eq!(a.y.get(), b.y.get());
        let ([ca, cb], cc) = carrier.coefficients();
        for q in [a, b] {
            assert_eq!(ca * i128::from(q.x.get()) + cb * i128::from(q.y.get()), cc);
        }
    }
}

#[test]
fn align_takes_the_turn_with_a_positive_scale() {
    assert_eq!(
        align(p(0, 0), p(1, 0), p(0, 0), p(2, 0)).unwrap(),
        Similarity::scaling(2, p(0, 0)).unwrap()
    );
    assert_eq!(
        align(p(0, 0), p(1, 0), p(0, 0), p(0, 3)).unwrap(),
        Similarity::scaling(3, p(0, 0))
            .unwrap()
            .after(&Similarity::quarter_turns(1, p(0, 0)).unwrap())
            .unwrap()
    );
}

#[test]
fn halving_a_segment_gives_its_midpoint() {
    let pts = divide(p(0, 0), p(10, 4), 2).unwrap();
    assert_eq!(pts.len(), 1);
    let ([x, y], w) = pts[0].coordinates();
    assert_eq!((x, y), (5 * w, 2 * w));
}
