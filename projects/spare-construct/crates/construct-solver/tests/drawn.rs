//! A sketch checked as drawn, without solving: it passes when its stored points and radii already
//! meet every constraint within the model tolerance, and names the first constraint that does not
//! otherwise, by the measure a solve's rounded answer is checked with.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_solver::{
    Budget, Constraint, ConstraintId, MODEL_TOLERANCE_STEPS, Sketch, SolveError, check_drawn, mm,
    solve,
};

/// A 1,000 by 500 mm rectangle drawn exactly, held by a fixed corner, its sides' directions and
/// two lengths, the first `width_mm`.
fn rectangle(width_mm: i32) -> Sketch {
    let mut sketch = Sketch::new();
    let sw = sketch.add_point(mm(0), mm(0)).unwrap();
    let se = sketch.add_point(mm(1_000), mm(0)).unwrap();
    let ne = sketch.add_point(mm(1_000), mm(500)).unwrap();
    let nw = sketch.add_point(mm(0), mm(500)).unwrap();
    let bottom = sketch.add_line(sw, se).unwrap();
    let right = sketch.add_line(se, ne).unwrap();
    let top = sketch.add_line(ne, nw).unwrap();
    let left = sketch.add_line(nw, sw).unwrap();
    for constraint in [
        Constraint::Fixed(sw),
        Constraint::Horizontal(bottom),
        Constraint::Vertical(right),
        Constraint::Horizontal(top),
        Constraint::Vertical(left),
        Constraint::Distance {
            a: sw,
            b: se,
            steps: mm(width_mm),
        },
        Constraint::Distance {
            a: se,
            b: ne,
            steps: mm(500),
        },
    ] {
        sketch.add_constraint(constraint).unwrap();
    }
    sketch
}

#[test]
fn a_sketch_drawn_as_its_constraints_say_passes() {
    let s = rectangle(1_000);
    assert_eq!(check_drawn(&s), Ok(()));
    // And a solve of it agrees.
    assert!(solve(&s, Budget::DEFAULT).is_ok());
}

#[test]
fn a_length_it_does_not_meet_is_named_with_its_miss() {
    let s = rectangle(900);
    assert_eq!(
        check_drawn(&s),
        Err(SolveError::RoundingRefused {
            constraint: ConstraintId(5),
            residual_mm: 100.0,
        })
    );
}

#[test]
fn a_miss_within_the_model_tolerance_passes() {
    let mut s = Sketch::new();
    let a = s.add_point(0, 0).unwrap();
    let b = s.add_point(mm(1_000) + MODEL_TOLERANCE_STEPS, 0).unwrap();
    s.add_constraint(Constraint::Distance {
        a,
        b,
        steps: mm(1_000),
    })
    .unwrap();
    assert_eq!(check_drawn(&s), Ok(()));
    let c = s
        .add_point(mm(1_000) + MODEL_TOLERANCE_STEPS + 1, mm(10))
        .unwrap();
    s.add_constraint(Constraint::Distance {
        a,
        b: c,
        steps: mm(1_000),
    })
    .unwrap();
    assert!(matches!(
        check_drawn(&s),
        Err(SolveError::RoundingRefused {
            constraint: ConstraintId(1),
            ..
        })
    ));
}
