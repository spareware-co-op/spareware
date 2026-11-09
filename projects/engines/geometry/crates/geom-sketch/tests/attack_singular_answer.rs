//! An attack on the block solver's answers (piece G222, from G231's fuzz target `sketch_corpus`):
//! a solve must refuse, not return, an answer that leaves a rule on its singular set.
//!
//! The fixture, `data/singular_answer.sketch`, is G231's family case "growth" 3 with its first
//! length scaled by 3.359375, the input `00 68 92 06 c7 1c 71 c7 1c 71 c7` that libFuzzer found.
//! Point 1.2 must be as far from 1.1 as 1.1 is from 1.0, on their level line. From its start, past
//! 1.1, the least-change step folds it back onto 1.0, where the line 2.6 from 1.0 to 1.2 has no
//! length and the rule making it perpendicular to 2.7 has a zero residual that means nothing.
//! Before the answer was checked against the singular sets, the solve returned that answer, and the
//! line 2.7 stood 1.86 units off square to the line through 1.0 and 1.2, as the fuzz target
//! measured it. The same sketch at its built length solves, and a path to the scaled length keeps
//! the branch and solves too.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::float_arithmetic,
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "tests, written as the definitions read"
)]

use geom_run::{Budget, WorkMeter};
use geom_sketch::model::{ItemId, SketchError};
use geom_sketch::solve::{TOLERANCE, continue_to, solve};
use geom_sketch::text::parse;

const FOLDED: &str = include_str!("data/singular_answer.sketch");

/// The length 4.2 as G231's case was built, before the fuzz input scaled it.
const BUILT: &str = "16168.626745188612";

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn built() -> String {
    FOLDED.replace("54316.480472118", BUILT)
}

#[test]
fn an_answer_folded_onto_a_singular_set_is_refused() {
    let sketch = parse(FOLDED).unwrap();
    assert_eq!(
        solve(&sketch, &mut meter()).err(),
        Some(SketchError::Degenerate(ItemId::new(4, 7)))
    );
}

#[test]
fn the_same_sketch_at_its_built_length_solves_with_every_line_long() {
    let sketch = parse(&built()).unwrap();
    let solved = solve(&sketch, &mut meter()).unwrap();
    let at = |key| solved.sketch.point(ItemId::new(1, key)).unwrap();
    let [a, c] = [at(0), at(2)];
    assert!((c[0] - a[0]).hypot(c[1] - a[1]) > 1000.0 * TOLERANCE);
}

#[test]
fn a_path_to_the_scaled_length_keeps_the_branch() {
    let from = solve(&parse(&built()).unwrap(), &mut meter())
        .unwrap()
        .sketch;
    let to = parse(FOLDED).unwrap();
    let solved = continue_to(&from, &to, &mut meter()).unwrap();
    let at = |key| solved.sketch.point(ItemId::new(1, key)).unwrap();
    // 1.2 stays past 1.1, a length beyond it.
    let [a, b, c] = [at(0), at(1), at(2)];
    assert!(c[0] > b[0] && b[0] > a[0], "{a:?} {b:?} {c:?}");
}
