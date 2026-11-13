//! Tests of `geom_sketch`'s model, equations and text form (piece G217).
//!
//! The board's done-when: "Entities, scaled variables, constraint trait with singular sets and root
//! choices, priorities, driving and reference dimensions, sketch v1 text form; parser fuzzing;
//! 10,000 constraints assembled in 2 ms." The fuzz target is `geom-fuzz`'s `sketch_text`, and the
//! benchmark `benches/assemble.rs`.
//!
//! What this file proves: items order by their ids whatever order they were added in; references
//! must resolve and values be finite and positive; variables are laid out in authoring order with
//! exact power-of-two scales; every generated Jacobian entry equals a central difference of its
//! residual; a singular start names its constraint; a side comes from the exact predicate, and a
//! point on the line leaves it undetermined; a reference dimension is measured and never assembled;
//! and the text form round-trips every kind bit for bit and refuses every malformed line.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    reason = "tests, written as the formulas read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_num::{GridStep, P3};
use geom_sketch::equation::{Assembly, measure};
use geom_sketch::model::{
    Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Side, Sketch, SketchError,
    Standing, power_of_two,
};
use geom_sketch::plane::{Lattice, PlaneFrame, Rotated, SketchPlane};
use geom_sketch::text::{parse, write};

fn id(sequence: u64, key: u64) -> ItemId {
    ItemId::new(sequence, key)
}

fn plane() -> SketchPlane {
    let lattice = Lattice::new(P3::new(0, 0, 0).unwrap(), [1, 0, 0], [0, 1, 0]).unwrap();
    SketchPlane::new(PlaneFrame::Lattice(lattice), GridStep::Q0)
}

fn entity(id: ItemId, kind: EntityKind) -> Entity {
    Entity {
        id,
        kind,
        construction: false,
    }
}

fn point(sequence: u64, x: f64, y: f64) -> Entity {
    entity(id(sequence, 0), EntityKind::Point { x, y })
}

fn line(sequence: u64, a: u64, b: u64) -> Entity {
    entity(
        id(sequence, 0),
        EntityKind::Line {
            start: id(a, 0),
            end: id(b, 0),
            extent: Extent::Segment,
        },
    )
}

fn driving(sequence: u64, kind: ConstraintKind) -> Constraint {
    Constraint {
        id: id(sequence, 0),
        kind,
        standing: Standing::Driving,
        priority: 0,
    }
}

/// Three points, a line on the first two, and each kind of constraint on them.
fn sampler() -> Sketch {
    let mut s = Sketch::new(plane());
    for e in [
        point(1, 3.0, 4.0),
        point(2, 13.0, 9.0),
        point(3, -2.0, 7.0),
        line(4, 1, 2),
        entity(
            id(5, 0),
            EntityKind::Scalar {
                value: 40.0,
                size: 1024.0,
            },
        ),
    ] {
        s.add_entity(e).unwrap();
    }
    for c in [
        driving(
            10,
            ConstraintKind::Coincident {
                a: id(1, 0),
                b: id(3, 0),
            },
        ),
        driving(11, ConstraintKind::Horizontal { line: id(4, 0) }),
        driving(12, ConstraintKind::Vertical { line: id(4, 0) }),
        driving(
            13,
            ConstraintKind::Distance {
                a: id(1, 0),
                b: id(2, 0),
                distance: 20.0,
            },
        ),
        driving(
            14,
            ConstraintKind::PointLine {
                point: id(3, 0),
                line: id(4, 0),
                distance: 6.0,
                side: Side::Left,
            },
        ),
        driving(
            15,
            ConstraintKind::Fix {
                entity: id(5, 0),
                slot: 0,
                value: 32.0,
            },
        ),
    ] {
        s.add_constraint(c).unwrap();
    }
    s
}

#[test]
fn items_sort_by_their_ids_whatever_order_they_came_in() {
    let mut s = Sketch::new(plane());
    s.add_entity(point(3, 0.0, 0.0)).unwrap();
    s.add_entity(point(1, 0.0, 0.0)).unwrap();
    s.add_entity(entity(id(2, 7), EntityKind::Reference))
        .unwrap();
    s.add_entity(entity(id(2, 3), EntityKind::Reference))
        .unwrap();
    let order: Vec<ItemId> = s.entities().iter().map(|e| e.id).collect();
    assert_eq!(order, [id(1, 0), id(2, 3), id(2, 7), id(3, 0)]);
    let mut other = Sketch::new(plane());
    for e in s.entities().iter().rev() {
        other.add_entity(e.clone()).unwrap();
    }
    assert_eq!(other, s);
    assert_eq!(
        s.add_entity(point(1, 5.0, 5.0)),
        Err(SketchError::Duplicate(id(1, 0)))
    );
    let later = driving(
        9,
        ConstraintKind::Fix {
            entity: id(1, 0),
            slot: 1,
            value: 0.0,
        },
    );
    s.add_constraint(later).unwrap();
    assert_eq!(
        s.add_constraint(later),
        Err(SketchError::Duplicate(id(9, 0)))
    );
    s.add_constraint(driving(
        8,
        ConstraintKind::Fix {
            entity: id(1, 0),
            slot: 0,
            value: 0.0,
        },
    ))
    .unwrap();
    assert_eq!(s.constraints()[0].id, id(8, 0));
}

#[test]
fn references_must_resolve_and_values_be_sound() {
    let mut s = Sketch::new(plane());
    s.add_entity(point(1, 0.0, 0.0)).unwrap();
    s.add_entity(entity(id(2, 0), EntityKind::Reference))
        .unwrap();
    let missing = id(9, 9);
    let refused = |s: &mut Sketch, kind: EntityKind| s.add_entity(entity(id(50, 0), kind));
    let (p, r) = (id(1, 0), id(2, 0));
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Line {
                start: p,
                end: missing,
                extent: Extent::Ray
            }
        ),
        Err(SketchError::Unknown(missing))
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Line {
                start: r,
                end: p,
                extent: Extent::Infinite
            }
        ),
        Err(SketchError::WrongKind(r))
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Circle {
                centre: p,
                radius: 0.0
            }
        ),
        Err(SketchError::NotPositive)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Circle {
                centre: p,
                radius: f64::NAN
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Point {
                x: f64::INFINITY,
                y: 0.0
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Point {
                x: 0.0,
                y: f64::NAN
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Arc {
                centre: p,
                radius: 1.0,
                start: p,
                end: missing
            }
        ),
        Err(SketchError::Unknown(missing))
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Arc {
                centre: p,
                radius: -1.0,
                start: p,
                end: p
            }
        ),
        Err(SketchError::NotPositive)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Ellipse {
                centre: p,
                axis: [f64::NAN, 0.0],
                minor: 1.0
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Ellipse {
                centre: p,
                axis: [1.0, f64::NAN],
                minor: 1.0
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Ellipse {
                centre: p,
                axis: [1.0, 0.0],
                minor: 0.0
            }
        ),
        Err(SketchError::NotPositive)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Spline {
                degree: 1,
                points: vec![missing],
                knots: vec![],
                weights: vec![]
            }
        ),
        Err(SketchError::Unknown(missing))
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Spline {
                degree: 1,
                points: vec![p],
                knots: vec![f64::NAN],
                weights: vec![]
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Spline {
                degree: 1,
                points: vec![p],
                knots: vec![0.0],
                weights: vec![0.0]
            }
        ),
        Err(SketchError::NotPositive)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Parameter {
                curve: missing,
                t: 0.0,
                speed: 1.0
            }
        ),
        Err(SketchError::Unknown(missing))
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Parameter {
                curve: p,
                t: f64::NAN,
                speed: 1.0
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Parameter {
                curve: p,
                t: 0.0,
                speed: 0.0
            }
        ),
        Err(SketchError::NotPositive)
    );
    // A speed so small its inverse overflows has no scale.
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Parameter {
                curve: p,
                t: 0.0,
                speed: 1e-320
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Scalar {
                value: f64::NAN,
                size: 1.0
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Scalar {
                value: 0.0,
                size: 0.0
            }
        ),
        Err(SketchError::NotPositive)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Placement {
                offset: [f64::NAN, 0.0],
                turn: 0.0,
                size: 1.0
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Placement {
                offset: [0.0, f64::NAN],
                turn: 0.0,
                size: 1.0
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Placement {
                offset: [0.0, 0.0],
                turn: f64::NAN,
                size: 1.0
            }
        ),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        refused(
            &mut s,
            EntityKind::Placement {
                offset: [0.0, 0.0],
                turn: 0.0,
                size: -1.0
            }
        ),
        Err(SketchError::NotPositive)
    );
    assert_eq!(s.entities().len(), 2);
    assert_eq!(s.point(missing), Err(SketchError::Unknown(missing)));
    assert_eq!(s.point(r), Err(SketchError::WrongKind(r)));
    assert_eq!(s.line(p), Err(SketchError::WrongKind(p)));
    let c = |kind| driving(60, kind);
    assert_eq!(
        s.add_constraint(c(ConstraintKind::Coincident { a: p, b: r })),
        Err(SketchError::WrongKind(r))
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::Coincident { a: r, b: p })),
        Err(SketchError::WrongKind(r))
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::Horizontal { line: p })),
        Err(SketchError::WrongKind(p))
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::Vertical { line: p })),
        Err(SketchError::WrongKind(p))
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::Distance {
            a: p,
            b: p,
            distance: 0.0
        })),
        Err(SketchError::NotPositive)
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::Distance {
            a: p,
            b: r,
            distance: 1.0
        })),
        Err(SketchError::WrongKind(r))
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::PointLine {
            point: p,
            line: r,
            distance: 1.0,
            side: Side::Left
        })),
        Err(SketchError::WrongKind(r))
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::PointLine {
            point: p,
            line: r,
            distance: 0.0,
            side: Side::Left
        })),
        Err(SketchError::NotPositive)
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::PointLine {
            point: r,
            line: p,
            distance: 1.0,
            side: Side::Left
        })),
        Err(SketchError::WrongKind(r))
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::Fix {
            entity: p,
            slot: 2,
            value: 0.0
        })),
        Err(SketchError::WrongKind(p))
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::Fix {
            entity: p,
            slot: 0,
            value: f64::NAN
        })),
        Err(SketchError::NotFinite)
    );
    assert_eq!(
        s.add_constraint(c(ConstraintKind::Fix {
            entity: r,
            slot: 0,
            value: 0.0
        })),
        Err(SketchError::WrongKind(r))
    );
    let reference = Constraint {
        standing: Standing::Reference,
        ..c(ConstraintKind::Coincident { a: p, b: p })
    };
    assert_eq!(
        s.add_constraint(reference),
        Err(SketchError::NotADimension(id(60, 0)))
    );
}

#[test]
fn errors_say_what_went_wrong() {
    let x = id(1, 1);
    for (e, text) in [
        (SketchError::Duplicate(x), "two items have one id"),
        (SketchError::Unknown(x), "nothing has this id"),
        (SketchError::WrongKind(x), "the item is not the kind needed"),
        (SketchError::NotFinite, "a value is not finite"),
        (
            SketchError::NotPositive,
            "a value that must be positive is not",
        ),
        (
            SketchError::NotADimension(x),
            "only a dimension can be a reference",
        ),
        (
            SketchError::RootChoiceUndetermined(x),
            "the drawing does not decide the root",
        ),
        (SketchError::Degenerate(x), "the start is on a singular set"),
        (SketchError::Text { line: 3 }, "text that is not sketch v1"),
        (SketchError::OutOfMemory, "an allocation could not be made"),
        (SketchError::Stopped, "the work meter stopped"),
        (
            SketchError::Internal,
            "a step its own bounds rule out failed (a defect)",
        ),
        (
            geom_num::NumError::NotFinite.into(),
            "the arithmetic refused",
        ),
        (
            geom_linalg::LinalgError::Shape.into(),
            "the linear algebra refused",
        ),
        (
            geom_sketch::plane::PlaneError::OffPlane.into(),
            "the plane's frame is refused",
        ),
    ] {
        assert_eq!(e.to_string(), text);
    }
}

#[test]
fn variables_are_laid_out_in_order_on_exact_scales() {
    assert_eq!(power_of_two(1.0), 1.0);
    assert_eq!(power_of_two(1.5), 1.0);
    assert_eq!(power_of_two(1024.0), 1024.0);
    assert_eq!(power_of_two(1025.0), 1024.0);
    assert_eq!(power_of_two(0.75), 0.5);
    assert_eq!(power_of_two(1e-320), f64::MIN_POSITIVE);
    let mut s = Sketch::new(plane());
    s.add_entity(point(1, 1.0, 2.0)).unwrap();
    s.add_entity(entity(
        id(2, 0),
        EntityKind::Circle {
            centre: id(1, 0),
            radius: 3.0,
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(3, 0),
        EntityKind::Arc {
            centre: id(1, 0),
            radius: 4.0,
            start: id(1, 0),
            end: id(1, 0),
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(4, 0),
        EntityKind::Ellipse {
            centre: id(1, 0),
            axis: [5.0, 6.0],
            minor: 7.0,
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(5, 0),
        EntityKind::Parameter {
            curve: id(4, 0),
            t: 0.5,
            speed: 3.0,
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(6, 0),
        EntityKind::Scalar {
            value: 8.0,
            size: 100.0,
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(7, 0),
        EntityKind::Placement {
            offset: [9.0, 10.0],
            turn: 0.25,
            size: 1000.0,
        },
    ))
    .unwrap();
    s.add_entity(entity(id(8, 0), EntityKind::Reference))
        .unwrap();
    s.add_entity(line(9, 1, 1)).unwrap();
    s.add_entity(entity(
        id(10, 0),
        EntityKind::Spline {
            degree: 1,
            points: vec![id(1, 0)],
            knots: vec![0.0, 1.0],
            weights: vec![1.0],
        },
    ))
    .unwrap();
    let got: Vec<(u64, u8, f64, f64)> = s
        .variables()
        .unwrap()
        .iter()
        .map(|v| (v.owner.sequence, v.slot, v.value, v.scale))
        .collect();
    assert_eq!(
        got,
        [
            (1, 0, 1.0, 1.0),
            (1, 1, 2.0, 1.0),
            (2, 0, 3.0, 1.0),
            (3, 0, 4.0, 1.0),
            (4, 0, 5.0, 1.0),
            (4, 1, 6.0, 1.0),
            (4, 2, 7.0, 1.0),
            (5, 0, 0.5, 0.25),
            (6, 0, 8.0, 0.0078125),
            (7, 0, 9.0, 1.0),
            (7, 1, 10.0, 1.0),
            (7, 2, 0.25, 0.0009765625),
        ]
    );
}

/// The residuals of `s` with variable `which` moved by `by`.
fn residuals_moved(s: &Sketch, which: usize, by: f64) -> Vec<f64> {
    let variable = s.variables().unwrap()[which];
    let mut moved = Sketch::new(*s.plane());
    for e in s.entities() {
        let mut e = e.clone();
        if e.id == variable.owner {
            match &mut e.kind {
                EntityKind::Point { x, y } => {
                    if variable.slot == 0 {
                        *x += by;
                    } else {
                        *y += by;
                    }
                }
                EntityKind::Scalar { value, .. } => *value += by,
                _ => panic!("the sampler has no other variables"),
            }
        }
        moved.add_entity(e).unwrap();
    }
    for c in s.constraints() {
        moved.add_constraint(*c).unwrap();
    }
    let mut a = Assembly::new(&moved).unwrap();
    a.evaluate(&moved).unwrap();
    a.residuals().to_vec()
}

#[test]
fn every_jacobian_entry_is_the_derivative_of_its_residual() {
    let s = sampler();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    assert_eq!(a.variables().len(), 7);
    let jacobian = a.jacobian().to_dense().unwrap();
    assert_eq!(jacobian.rows(), a.residuals().len());
    assert_eq!(a.residuals().len(), 7);
    // Residuals at hand: the coincidence's two gaps, 5 and −3; the line's dy and dx, 5... as values.
    assert_eq!(a.residuals()[0], 5.0);
    assert_eq!(a.residuals()[1], -3.0);
    assert_eq!(a.residuals()[2], 4.0 - 9.0);
    assert_eq!(a.residuals()[3], 3.0 - 13.0);
    assert_eq!(a.residuals()[4], (125.0 - 400.0) / 40.0);
    assert_eq!(a.residuals()[6], 8.0);
    for j in 0..a.variables().len() {
        let h = 1e-5;
        let (up, down) = (residuals_moved(&s, j, h), residuals_moved(&s, j, -h));
        let scale = a.variables()[j].scale;
        for i in 0..up.len() {
            let numeric = (up[i] - down[i]) / (2.0 * h) * scale;
            let exact = jacobian.get(i, j).unwrap();
            assert!(
                (numeric - exact).abs() < 1e-6,
                "{i} {j}: {numeric} and {exact}"
            );
        }
    }
    // The fixed scalar's column is its scale exactly: the solver's variable acts as a length.
    assert_eq!(
        jacobian.get(6, 6).unwrap(),
        1024.0_f64.recip() * 1024.0 / 1024.0
    );
}

#[test]
fn a_distance_residual_has_a_gradient_of_length_one_at_its_solution() {
    let mut s = Sketch::new(plane());
    s.add_entity(point(1, 0.0, 0.0)).unwrap();
    s.add_entity(point(2, 3.0, 4.0)).unwrap();
    s.add_constraint(driving(
        3,
        ConstraintKind::Distance {
            a: id(1, 0),
            b: id(2, 0),
            distance: 5.0,
        },
    ))
    .unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    assert_eq!(a.residuals(), [0.0]);
    let row: Vec<f64> = (0..4)
        .map(|j| a.jacobian().to_dense().unwrap().get(0, j).unwrap())
        .collect();
    assert_eq!(row, [-0.6, -0.8, 0.6, 0.8]);
}

#[test]
fn a_singular_start_names_its_constraint() {
    let mut s = Sketch::new(plane());
    s.add_entity(point(1, 2.0, 2.0)).unwrap();
    s.add_entity(point(2, 2.0, 2.0)).unwrap();
    s.add_entity(point(3, 5.0, 5.0)).unwrap();
    s.add_entity(line(4, 1, 2)).unwrap();
    let c = driving(
        9,
        ConstraintKind::PointLine {
            point: id(3, 0),
            line: id(4, 0),
            distance: 1.0,
            side: Side::Right,
        },
    );
    s.add_constraint(c).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    assert_eq!(a.evaluate(&s), Err(SketchError::Degenerate(id(9, 0))));
    assert_eq!(measure(&s, &c), Err(SketchError::Degenerate(id(9, 0))));
    // Off by one coordinate alone, on either axis, it is not singular.
    for (x, y) in [(3.0, 2.0), (2.0, 3.0)] {
        let mut t = Sketch::new(plane());
        t.add_entity(point(1, 2.0, 2.0)).unwrap();
        t.add_entity(point(2, x, y)).unwrap();
        t.add_entity(point(3, 5.0, 5.0)).unwrap();
        t.add_entity(line(4, 1, 2)).unwrap();
        t.add_constraint(c).unwrap();
        let mut a = Assembly::new(&t).unwrap();
        assert_eq!(a.evaluate(&t), Ok(()));
    }
}

#[test]
fn the_side_comes_from_the_exact_predicate() {
    let mut s = Sketch::new(plane());
    s.add_entity(point(1, 0.0, 0.0)).unwrap();
    s.add_entity(point(2, 10.0, 0.0)).unwrap();
    s.add_entity(point(3, 4.0, 3.0)).unwrap();
    s.add_entity(point(4, 4.0, -3.0)).unwrap();
    s.add_entity(point(5, 4.0, 0.0)).unwrap();
    s.add_entity(point(6, 0.5, 3.0)).unwrap();
    s.add_entity(point(7, 3.0, 0.5)).unwrap();
    s.add_entity(point(8, 1e15, 0.0)).unwrap();
    s.add_entity(line(9, 1, 2)).unwrap();
    let side = |p: u64| s.drawn_side(id(p, 0), id(9, 0));
    assert_eq!(side(3), Ok(Side::Left));
    assert_eq!(side(4), Ok(Side::Right));
    assert_eq!(side(5), Err(SketchError::RootChoiceUndetermined(id(5, 0))));
    assert_eq!(side(6), Err(SketchError::RootChoiceUndetermined(id(6, 0))));
    assert_eq!(side(7), Err(SketchError::RootChoiceUndetermined(id(7, 0))));
    assert_eq!(side(8), Err(SketchError::RootChoiceUndetermined(id(8, 0))));
    assert_eq!(Side::Left.sign(), 1.0);
    assert_eq!(Side::Right.sign(), -1.0);
    assert_eq!(
        s.drawn_side(id(3, 0), id(1, 0)),
        Err(SketchError::WrongKind(id(1, 0)))
    );
    assert_eq!(
        s.drawn_side(id(77, 0), id(9, 0)),
        Err(SketchError::Unknown(id(77, 0)))
    );
}

#[test]
fn a_reference_dimension_is_measured_and_never_assembled() {
    let mut s = Sketch::new(plane());
    s.add_entity(point(1, 0.0, 0.0)).unwrap();
    s.add_entity(point(2, 3.0, 4.0)).unwrap();
    s.add_entity(point(3, 1.0, 7.0)).unwrap();
    s.add_entity(line(4, 1, 2)).unwrap();
    let length = Constraint {
        standing: Standing::Reference,
        ..driving(
            5,
            ConstraintKind::Distance {
                a: id(1, 0),
                b: id(2, 0),
                distance: 99.0,
            },
        )
    };
    let gap = Constraint {
        standing: Standing::Reference,
        ..driving(
            6,
            ConstraintKind::PointLine {
                point: id(3, 0),
                line: id(4, 0),
                distance: 99.0,
                side: Side::Left,
            },
        )
    };
    s.add_constraint(length).unwrap();
    s.add_constraint(gap).unwrap();
    let mut a = Assembly::new(&s).unwrap();
    a.evaluate(&s).unwrap();
    assert_eq!(a.residuals().len(), 0);
    assert_eq!(a.jacobian().nonzeros(), 0);
    assert_eq!(a.jacobian().cols(), 6);
    assert_eq!(measure(&s, &length), Ok(5.0));
    // (3, 4) × (1, 7) = 17, over 5.
    assert_eq!(measure(&s, &gap), Ok(3.4));
    let right = Constraint {
        id: id(7, 0),
        kind: ConstraintKind::PointLine {
            point: id(3, 0),
            line: id(4, 0),
            distance: 99.0,
            side: Side::Right,
        },
        ..gap
    };
    assert_eq!(measure(&s, &right), Ok(-3.4));
    let other = driving(8, ConstraintKind::Horizontal { line: id(4, 0) });
    assert_eq!(
        measure(&s, &other),
        Err(SketchError::NotADimension(id(8, 0)))
    );
}

#[test]
fn priorities_change_nothing_in_the_equations() {
    let (a, mut b) = (sampler(), Sketch::new(*sampler().plane()));
    for e in a.entities() {
        b.add_entity(e.clone()).unwrap();
    }
    for c in a.constraints() {
        b.add_constraint(Constraint {
            priority: 200,
            ..*c
        })
        .unwrap();
    }
    let (mut x, mut y) = (Assembly::new(&a).unwrap(), Assembly::new(&b).unwrap());
    x.evaluate(&a).unwrap();
    y.evaluate(&b).unwrap();
    assert_eq!(x, y);
}

/// A sketch with every kind of item, and a rotated plane when `rotated`.
fn everything(rotated: bool) -> Sketch {
    let frame = if rotated {
        PlaneFrame::Rotated(
            Rotated::new(
                P3::new(5, -6, 7).unwrap(),
                [0.6, 0.8, 0.0],
                [-0.8, 0.6, 0.0],
            )
            .unwrap(),
        )
    } else {
        PlaneFrame::Lattice(Lattice::of_normal(P3::new(1, 2, 3).unwrap(), [0, -5, 12]).unwrap())
    };
    let mut s = Sketch::new(SketchPlane::new(frame, GridStep::Q1));
    let (a, b, c) = (id(1, 0), id(1, 1), id(1, 2));
    s.add_entity(entity(a, EntityKind::Point { x: 0.1, y: -1e300 }))
        .unwrap();
    s.add_entity(entity(
        b,
        EntityKind::Point {
            x: 300.0,
            y: 1.0 / 3.0,
        },
    ))
    .unwrap();
    s.add_entity(Entity {
        id: c,
        kind: EntityKind::Point { x: -0.0, y: 5e-324 },
        construction: true,
    })
    .unwrap();
    for (key, extent) in [
        (0, Extent::Segment),
        (1, Extent::Ray),
        (2, Extent::Infinite),
    ] {
        s.add_entity(entity(
            id(2, key),
            EntityKind::Line {
                start: a,
                end: b,
                extent,
            },
        ))
        .unwrap();
    }
    s.add_entity(entity(
        id(3, 0),
        EntityKind::Circle {
            centre: a,
            radius: 12.5,
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(3, 1),
        EntityKind::Arc {
            centre: a,
            radius: 2.0,
            start: b,
            end: c,
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(3, 2),
        EntityKind::Ellipse {
            centre: a,
            axis: [3.0, -4.0],
            minor: 1.5,
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(3, 3),
        EntityKind::Spline {
            degree: 2,
            points: vec![a, b, c],
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            weights: vec![1.0, 0.5, 1.0],
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(4, 0),
        EntityKind::Parameter {
            curve: id(3, 2),
            t: 0.25,
            speed: 7.0,
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(4, 1),
        EntityKind::Scalar {
            value: -2.5,
            size: 100.0,
        },
    ))
    .unwrap();
    s.add_entity(entity(
        id(4, 2),
        EntityKind::Placement {
            offset: [1.0, 2.0],
            turn: 0.125,
            size: 64.0,
        },
    ))
    .unwrap();
    s.add_entity(entity(id(4, 3), EntityKind::Reference))
        .unwrap();
    let line = id(2, 0);
    for (key, kind) in [
        (0, ConstraintKind::Coincident { a, b }),
        (1, ConstraintKind::Horizontal { line }),
        (2, ConstraintKind::Vertical { line }),
        (
            3,
            ConstraintKind::Distance {
                a,
                b,
                distance: 1234.5,
            },
        ),
        (
            4,
            ConstraintKind::PointLine {
                point: c,
                line,
                distance: 2.0,
                side: Side::Left,
            },
        ),
        (
            5,
            ConstraintKind::PointLine {
                point: c,
                line,
                distance: 3.0,
                side: Side::Right,
            },
        ),
        (
            6,
            ConstraintKind::Fix {
                entity: id(4, 1),
                slot: 0,
                value: 0.1,
            },
        ),
    ] {
        s.add_constraint(Constraint {
            id: id(5, key),
            kind,
            standing: Standing::Driving,
            priority: 7,
        })
        .unwrap();
    }
    s.add_constraint(Constraint {
        id: id(6, 0),
        kind: ConstraintKind::Distance {
            a,
            b,
            distance: 3.0,
        },
        standing: Standing::Reference,
        priority: 255,
    })
    .unwrap();
    s
}

#[test]
fn the_text_form_round_trips_every_kind_bit_for_bit() {
    for rotated in [false, true] {
        let s = everything(rotated);
        let text = write(&s).unwrap();
        let back = parse(&text).unwrap();
        assert_eq!(write(&back).unwrap(), text);
        // `==` calls −0 and 0 equal; the text does not.
        assert!(text.contains("point -0 0.000"), "{text}");
        assert_eq!(back, s);
    }
    let small = sampler();
    let expected = "sketch v1\n\
        plane lattice 0 0 0 1 0 0 0 1 0 q0\n\
        entity 1.0 point 3 4\n\
        entity 2.0 point 13 9\n\
        entity 3.0 point -2 7\n\
        entity 4.0 line 1.0 2.0 segment\n\
        entity 5.0 scalar 40 1024\n\
        constraint 10.0 driving 0 coincident 1.0 3.0\n\
        constraint 11.0 driving 0 horizontal 4.0\n\
        constraint 12.0 driving 0 vertical 4.0\n\
        constraint 13.0 driving 0 distance 1.0 2.0 20\n\
        constraint 14.0 driving 0 point-line 3.0 4.0 6 left\n\
        constraint 15.0 driving 0 fix 5.0 0 32\n";
    assert_eq!(write(&small).unwrap(), expected);
    assert_eq!(parse(expected).unwrap(), small);
    // Blank lines and spacing are not part of the form, and every step reads back.
    let spaced = "sketch v1  \n  plane lattice 0 0 0 1 0 0 0 1 0 q2\n\n  entity 1.0   point 1 2\n";
    assert_eq!(
        write(&parse(spaced).unwrap()).unwrap(),
        "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q2\nentity 1.0 point 1 2\n"
    );
    let q0 = parse("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n").unwrap();
    assert_eq!(q0.plane().step(), GridStep::Q0);
}

#[test]
fn malformed_text_is_refused_with_its_line() {
    let head = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";
    let bad = |text: &str, line: usize| {
        assert_eq!(parse(text), Err(SketchError::Text { line }), "{text}");
    };
    bad("", 1);
    bad("sketch v2\n", 1);
    bad("sketch v1", 2);
    bad("sketch v1\nnot a plane\n", 2);
    bad("sketch v1\nplane square\n", 2);
    bad("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q9\n", 2);
    bad("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0 extra\n", 2);
    bad("sketch v1\nplane lattice 0 0 x 1 0 0 0 1 0 q0\n", 2);
    bad(
        "sketch v1\nplane lattice 1099511627777 0 0 1 0 0 0 1 0 q0\n",
        2,
    );
    bad(
        "sketch v1\nplane rotated 1099511627777 0 0 1 0 0 0 1 0 q0\n",
        2,
    );
    for line in [
        "nonsense",
        "entity",
        "entity 1 point 0 0",
        "entity x.0 point 0 0",
        "entity 1.x point 0 0",
        "entity 1.0 shape 0 0",
        "entity 1.0 point 0",
        "entity 1.0 point 0 nan",
        "entity 1.0 point 0 inf",
        "entity 1.0 point 0 x",
        "entity 1.0 point 0 0 0",
        "entity 1.0 construction",
        "entity 1.0 line 1.0 1.0 forever",
        "entity 1.0 spline 1 2 2.0",
        "entity 1.0 spline x",
        "constraint 1.0",
        "constraint 1.0 maybe 0 horizontal 1.0",
        "constraint 1.0 driving 256 horizontal 1.0",
        "constraint 1.0 driving 0 tangent 1.0",
        "constraint 1.0 driving 0 point-line 1.0 2.0 3 up",
        "constraint 1.0 driving 0 horizontal 1.0 more",
    ] {
        bad(&format!("{head}{line}\n"), 3);
    }
    // What the model refuses, the text does too, as the model's own error.
    assert_eq!(
        parse(&format!("{head}entity 1.0 line 7.7 7.7 ray\n")),
        Err(SketchError::Unknown(id(7, 7)))
    );
    assert_eq!(
        parse(&format!(
            "{head}entity 1.0 point 0 0\nentity 1.0 point 1 1\n"
        )),
        Err(SketchError::Duplicate(id(1, 0)))
    );
    assert_eq!(
        parse("sketch v1\nplane lattice 0 0 0 2 0 0 0 2 0 q0\n"),
        Err(SketchError::Plane(
            geom_sketch::plane::PlaneError::NotPrimitive
        ))
    );
}
