//! Tests of `geom_sketch::diagnose` (piece G224): degrees of freedom, free motions, redundant and
//! conflicting groups with the constraint to remove, near-singular pivots and the report text.
//!
//! Worked sketches are written in the `sketch v1` text form. The numerics are also driven on bare
//! columns, where the right answer is known by construction.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_run::{Budget, WorkMeter};
use geom_sketch::diagnose::{Class, Equation, Verdict, analyse_columns, diagnose, free_motions};
use geom_sketch::model::{ItemId, Sketch, SketchError};
use geom_sketch::text::parse;

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";

fn id(sequence: u64, key: u64) -> ItemId {
    ItemId::new(sequence, key)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn sketch(lines: &str) -> Sketch {
    parse(&format!("{HEAD}{lines}")).unwrap()
}

const TRIANGLE: &str = "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.2 point 3000 4000
constraint 3.0 driving 0 fix 1.0 0 0
constraint 3.1 driving 0 fix 1.0 1 0
constraint 3.2 driving 0 distance 1.0 1.1 3000
constraint 3.3 driving 0 distance 1.1 1.2 4000
constraint 3.4 driving 0 distance 1.0 1.2 5000
constraint 3.5 driving 0 fix 1.1 1 0
";

fn eq(sequence: u64, priority: u8, order: usize) -> Equation {
    Equation {
        constraint: id(sequence, 0),
        priority,
        order,
    }
}

#[test]
fn a_sketch_with_no_constraints_has_every_variable_free() {
    let s = sketch("entity 1.0 point 0 0\nentity 1.1 point 5 5\n");
    let r = diagnose(&s, 1000.0, &mut meter()).unwrap();
    assert_eq!((r.dof_least, r.dof_most), (4, 4));
    assert_eq!(r.redundant.len() + r.conflicting.len(), 0);
    assert_eq!(r.analysis.equations, 0);
}

#[test]
fn a_fully_constrained_triangle_has_no_freedom_and_nothing_repeats() {
    // Six equations, six variables, all independent. A seventh that restates one is redundant.
    let s = sketch(TRIANGLE);
    let r = diagnose(&s, 10_000.0, &mut meter()).unwrap();
    assert_eq!(r.dof_least, 0, "{}", r.text());
    assert_eq!(r.near_singular.len(), 0);
    assert!(!r.has_conflict());
    assert_eq!(r.redundant.len(), 0, "{}", r.text());
    let more = sketch(&format!(
        "{TRIANGLE}constraint 3.6 driving 1 fix 1.0 0 0
"
    ));
    let r = diagnose(&more, 10_000.0, &mut meter()).unwrap();
    assert_eq!(r.redundant.len(), 1, "{}", r.text());
    assert_eq!(r.redundant[0].class, Class::Redundant);
}

#[test]
fn a_repeated_distance_is_redundant_and_a_contradicting_one_is_a_conflict() {
    let base = "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.5 line 1.0 1.1 segment
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 horizontal 1.5
constraint 2.3 driving 0 distance 1.0 1.1 3000
";
    let same = sketch(&format!(
        "{base}constraint 2.4 driving 1 distance 1.0 1.1 3000\n"
    ));
    let r = diagnose(&same, 10_000.0, &mut meter()).unwrap();
    assert_eq!(r.redundant.len(), 1, "{}", r.text());
    assert_eq!(r.conflicting.len(), 0);

    let other = sketch(&format!(
        "{base}constraint 2.4 driving 1 distance 1.0 1.1 3100\n"
    ));
    let r = diagnose(&other, 10_000.0, &mut meter()).unwrap();
    assert_eq!(r.conflicting.len(), 1, "{}", r.text());
    let g = &r.conflicting[0];
    assert_eq!(g.class, Class::Conflicting);
    assert!(g.defect > 1.0);
    // The newer, lower-ranked distance is the one to remove.
    assert_eq!(g.suggested_removal, Some(id(2, 4)));
    assert!(r.text().contains("conflict: "));
    assert!(r.text().contains("suggest removing 2.4"));
}

#[test]
fn free_motions_are_in_echelon_form_over_the_variables() {
    // One coordinate of one point held: its y and both coordinates of the other point are free.
    let s = sketch(
        "entity 1.0 point 0 0
entity 1.1 point 5 5
constraint 2.0 driving 0 fix 1.0 0 0
",
    );
    let motions = free_motions(&s, &mut meter()).unwrap();
    assert_eq!(motions.len(), 3);
    for m in &motions {
        assert_ne!(m.moves.len(), 0);
        // A unit at the motion's own leading variable.
        assert!((m.moves[0].2.abs() - 1.0).abs() < 1e-9, "{m:?}");
    }
    assert_eq!(motions[0].owners(), [id(1, 0)]);
}

#[test]
fn the_pivot_prefers_the_lower_priority_number_among_near_equals() {
    // Two identical gradients: the one with priority 0 is chosen and the other is left out.
    let columns = vec![vec![1.0, 0.0], vec![1.0, 0.0]];
    let equations = [eq(1, 5, 0), eq(2, 0, 1)];
    let a = analyse_columns(&columns, &equations, &[0.0, 0.0], 2, 10.0, &mut meter()).unwrap();
    assert_eq!(a.independent, 1);
    assert_eq!(a.pivots[0].equation, 1);
    assert_eq!(a.groups.len(), 1);
    assert_eq!(a.groups[0].left_out, 0);
    assert_eq!(a.groups[0].class, Class::Redundant);
    // With equal priorities the older goes first.
    let equations = [eq(1, 3, 0), eq(2, 3, 1)];
    let a = analyse_columns(&columns, &equations, &[0.0, 0.0], 2, 10.0, &mut meter()).unwrap();
    assert_eq!(a.pivots[0].equation, 0);
}

#[test]
fn a_dependence_is_conflicting_when_the_residuals_do_not_agree() {
    // The third gradient is the sum of the first two; its residual should be the sum of theirs.
    let columns = vec![
        vec![1.0, 0.0, 0.0],
        vec![0.0, 1.0, 0.0],
        vec![1.0, 1.0, 0.0],
    ];
    let equations = [eq(1, 0, 0), eq(2, 0, 1), eq(3, 0, 2)];
    let ok = analyse_columns(
        &columns,
        &equations,
        &[0.5, 0.25, 0.75],
        3,
        10.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(ok.groups.len(), 1);
    assert_eq!(ok.groups[0].class, Class::Redundant);
    assert_eq!(ok.groups[0].equations, [0, 1, 2]);
    let bad = analyse_columns(
        &columns,
        &equations,
        &[0.5, 0.25, 4.0],
        3,
        10.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(bad.groups[0].class, Class::Conflicting);
    assert_eq!(bad.groups[0].suggested_removal, Some(id(3, 0)));
    assert_eq!((bad.dof_least(), bad.dof_most()), (1, 1));
}

#[test]
fn a_pivot_between_the_thresholds_is_near_singular_and_never_rounded() {
    // A huge block (a million grid steps) lowers the dependence threshold far below 2^-12, so a
    // second gradient 1e-5 off the first is between the two.
    let columns = vec![vec![1.0, 0.0], vec![1.0, 1e-5]];
    let equations = [eq(1, 0, 0), eq(2, 0, 1)];
    let a = analyse_columns(&columns, &equations, &[0.0, 0.0], 2, 1.0e6, &mut meter()).unwrap();
    assert_eq!(a.pivots[1].verdict, Verdict::NearSingular);
    assert_eq!(a.near_singular, 1);
    assert_eq!((a.dof_least(), a.dof_most()), (0, 1));
    assert_eq!(a.near.len(), 1);
    // A small block calls the same pair dependent.
    let a = analyse_columns(&columns, &equations, &[0.0, 0.0], 2, 1.0, &mut meter()).unwrap();
    assert_eq!(a.pivots[1].verdict, Verdict::Dependent);
    assert_eq!(a.groups.len(), 1);
}

#[test]
fn unrelated_items_do_not_change_the_verdict_or_the_text() {
    let a = sketch(&format!("{TRIANGLE}entity 9.0 point 7 7\n"));
    let ra = diagnose(&a, 10_000.0, &mut meter()).unwrap();
    // One more free point adds two degrees of freedom and nothing else.
    let rb = diagnose(&sketch(TRIANGLE), 10_000.0, &mut meter()).unwrap();
    assert_eq!(ra.dof_least, rb.dof_least + 2);
    assert_eq!(ra.redundant.len(), rb.redundant.len());
    assert_eq!(
        ra.text(),
        diagnose(&a, 10_000.0, &mut meter()).unwrap().text()
    );
}

#[test]
fn a_budget_too_small_stops_the_diagnosis() {
    let s = sketch(TRIANGLE);
    let mut tiny = WorkMeter::new(Budget::of_work(1));
    assert_eq!(diagnose(&s, 100.0, &mut tiny), Err(SketchError::Stopped));
}

#[test]
fn inconsistent_inputs_are_refused() {
    let columns = vec![vec![1.0, 0.0]];
    let r = analyse_columns(&columns, &[], &[0.0], 2, 1.0, &mut meter());
    assert_eq!(r, Err(SketchError::Internal));
}
