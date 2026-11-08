//! Tests of `geom_sketch::diagnose` (piece G224): degrees of freedom, free motions, redundant and
//! conflicting groups with the constraint to remove, near-singular pivots and the report text.
//!
//! Worked sketches are written in the `sketch v1` text form. The numerics are also driven on bare
//! columns, where the right answer is known by construction.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_run::{Budget, WorkMeter};
use geom_sketch::diagnose::{
    CARRY_ROWS, Class, Equation, MOTION_CUTOFF, Motion, NormBound, Outcome, RANGE_HALVINGS,
    RANGE_RESOLUTION, SketchReport, Thresholds, Verdict, analyse_columns, diagnose, free_motions,
    remedies, solve_checked, witness_point,
};
use geom_sketch::equation::Assembly;
use geom_sketch::model::{ItemId, Sketch, SketchError, Variable, power_of_two};
use geom_sketch::text::parse;
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

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

#[test]
fn a_conflict_comes_with_what_each_dimension_would_measure_without_the_other() {
    let s = sketch(
        "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.5 line 1.0 1.1 segment
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 horizontal 1.5
constraint 2.3 driving 0 distance 1.0 1.1 3000
constraint 2.4 driving 1 distance 1.0 1.1 3100
",
    );
    let Outcome::Conflict { report, remedies } = solve_checked(&s, 10_000.0, &mut meter()).unwrap()
    else {
        panic!("not a conflict");
    };
    assert_eq!(report.conflicting.len(), 1);
    // Without the newer 3100, the sketch settles at the older 3000; without the older, at 3100.
    let by = |c: ItemId| {
        remedies
            .iter()
            .find(|r| r.constraint == c)
            .copied()
            .unwrap()
    };
    let newer = by(id(2, 4));
    assert!((newer.asked - 3100.0).abs() < 1e-6);
    assert!((newer.measures - 3000.0).abs() < 0.1, "{newer:?}");
    let older = by(id(2, 3));
    assert!((older.measures - 3100.0).abs() < 0.1, "{older:?}");
}

#[test]
fn lengths_that_cannot_close_a_triangle_are_not_converged_and_the_remedy_search_finds_values() {
    // A and B are held 1000 apart; C is asked to be 1000 from A and 5000 from B. The equations are
    // independent, so there is no rank problem, only no solution from this start.
    let s = sketch(
        "entity 1.0 point 0 0
entity 1.1 point 1000 0
entity 1.2 point 500 800
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 fix 1.1 0 1000
constraint 2.3 driving 0 fix 1.1 1 0
constraint 2.4 driving 0 distance 1.0 1.2 1000
constraint 2.5 driving 0 distance 1.1 1.2 5000
",
    );
    let Outcome::NotConverged {
        report, remedies, ..
    } = solve_checked(&s, 10_000.0, &mut meter()).unwrap()
    else {
        panic!("converged or conflicting");
    };
    assert_eq!(report.conflicting.len(), 0);
    let third = remedies.iter().find(|r| r.constraint == id(2, 5)).unwrap();
    assert!((third.asked - 5000.0).abs() < 1e-6);
    // Held to the first side, C is at most 2000 from B.
    assert!(third.measures <= 2000.0 + 0.1, "{third:?}");
}

#[test]
fn a_dependence_that_only_this_configuration_has_is_special_not_redundant() {
    // Three points on one line under three distances: the lengths AB, BC and AC are dependent
    // here because AC = AB + BC, and independent as soon as the points leave the line.
    let s = sketch(
        "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.2 point 6000 0
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 fix 1.1 1 0
constraint 2.3 driving 0 distance 1.0 1.1 3000
constraint 2.4 driving 0 distance 1.1 1.2 3000
constraint 2.5 driving 0 distance 1.0 1.2 6000
",
    );
    let r = diagnose(&s, 10_000.0, &mut meter()).unwrap();
    assert_eq!(r.redundant.len(), 0, "{}", r.text());
    assert_eq!(r.special.len(), 1, "{}", r.text());
    assert!(r.text().contains("special configuration"));
}

#[test]
fn a_solved_sketch_comes_back_with_its_report() {
    let s = sketch(TRIANGLE);
    let Outcome::Solved { solved, report } = solve_checked(&s, 10_000.0, &mut meter()).unwrap()
    else {
        panic!("not solved");
    };
    assert_eq!(report.dof_least, 0);
    assert!(solved.report.largest <= 0.02);
    // The solver's own report carries the diagnosis too.
    assert_eq!(solved.report.diagnosis.as_deref(), Some(&report));
}

#[test]
fn a_plain_solve_does_not_diagnose() {
    let done = geom_sketch::solve::solve(&sketch(TRIANGLE), &mut meter()).unwrap();
    assert!(done.report.diagnosis.is_none());
}

#[test]
fn a_remedy_gives_the_range_of_values_that_fit_by_continuation() {
    // C is 1000 from A and the distance to B is asked to be 5000; B is 1000 from A. C can be
    // anywhere from B itself to 2000 away, and not nearer than 0 nor farther than 2000.
    let s = sketch(
        "entity 1.0 point 0 0
entity 1.1 point 1000 0
entity 1.2 point 500 800
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 fix 1.1 0 1000
constraint 2.3 driving 0 fix 1.1 1 0
constraint 2.4 driving 0 distance 1.0 1.2 1000
constraint 2.5 driving 0 distance 1.1 1.2 5000
",
    );
    let Outcome::NotConverged { remedies, .. } = solve_checked(&s, 10_000.0, &mut meter()).unwrap()
    else {
        panic!("converged or conflicting");
    };
    let third = remedies.iter().find(|r| r.constraint == id(2, 5)).unwrap();
    let range = third.range.unwrap();
    // C is 1000 from A and the distance from B runs from nothing to 2000.
    assert!(
        range.least.unwrap() >= 0.0 && range.least.unwrap() < 0.5,
        "{third:?}"
    );
    assert!((range.most.unwrap() - 2000.0).abs() < 0.3, "{third:?}");
    // And the distance from A runs from 4000 to 6000 with B 5000 away and A 1000 from B.
    let second = remedies.iter().find(|r| r.constraint == id(2, 4)).unwrap();
    let range = second.range.unwrap();
    assert!((range.least.unwrap() - 4000.0).abs() < 0.5, "{second:?}");
    assert!((range.most.unwrap() - 6000.0).abs() < 0.5, "{second:?}");
    // Five thousand is outside the first range: the words say so by giving the range.
    assert!(range.most.unwrap() < 6000.5 && third.asked > 2000.0);
    assert!(
        third.text().contains("any value from 0.2 to 1999.9"),
        "{}",
        third.text()
    );
}

#[test]
fn two_dimensions_that_pin_one_value_have_a_range_of_that_value_alone() {
    let s = sketch(
        "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.5 line 1.0 1.1 segment
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 horizontal 1.5
constraint 2.3 driving 0 distance 1.0 1.1 3000
constraint 2.4 driving 1 distance 1.0 1.1 3100
",
    );
    let Outcome::Conflict { remedies, .. } = solve_checked(&s, 10_000.0, &mut meter()).unwrap()
    else {
        panic!("not a conflict");
    };
    let newer = remedies.iter().find(|r| r.constraint == id(2, 4)).unwrap();
    let range = newer.range.unwrap();
    assert!(
        range.most.unwrap() - range.least.unwrap() < 0.5,
        "{newer:?}"
    );
}

#[test]
fn a_remedy_in_words_names_the_ends_it_has() {
    use geom_sketch::diagnose::{Remedy, ValueRange};
    let said = |least, most| {
        Remedy {
            constraint: id(2, 7),
            asked: 4200.0,
            measures: 4150.0,
            range: Some(ValueRange { least, most }),
        }
        .text()
    };
    assert_eq!(
        said(Some(3900.0), Some(4180.0)),
        "2.7 (4200) without its conflicts measures 4150, and any value from 3900.0 to 4180.0 fits"
    );
    assert!(said(None, Some(4180.0)).ends_with(", and any value up to 4180.0 fits"));
    assert!(said(Some(3900.0), None).ends_with(", and any value from 3900.0 up fits"));
    assert!(said(None, None).ends_with("measures 4150"));
    let bare = Remedy {
        constraint: id(2, 7),
        asked: 4200.0,
        measures: 4150.0,
        range: None,
    };
    assert!(bare.text().ends_with("measures 4150"));
}

#[test]
fn a_dimension_restates_its_value_and_nothing_else_does() {
    use geom_sketch::model::{Amount, ConstraintKind as K, Side};
    let (a, b, c) = (id(1, 0), id(1, 1), id(1, 2));
    let v = Amount::Value(1.0);
    let dimensions = [
        K::Turned { a, b, angle: v },
        K::Heading { line: a, angle: v },
        K::Sweep { arc: a, angle: v },
        K::Distance { a, b, distance: v },
        K::FromLine {
            point: a,
            line: b,
            distance: v,
            side: Side::Left,
        },
        K::FromCircle {
            point: a,
            circle: b,
            distance: v,
            outside: true,
        },
        K::LinesApart {
            a,
            b,
            distance: v,
            side: Side::Left,
        },
        K::Across { a, b, distance: v },
        K::Upward { a, b, distance: v },
        K::Length { line: a, length: v },
        K::ArcLength { arc: a, length: v },
        K::Radius {
            circle: a,
            radius: v,
        },
        K::Diameter {
            circle: a,
            diameter: v,
        },
        K::Distance {
            a,
            b,
            distance: Amount::Named(c),
        },
    ];
    for kind in dimensions {
        let again = kind.with_stated(7.5).unwrap();
        assert_eq!(again.stated(), Some(7.5), "{kind:?}");
        assert_eq!(again.with_stated(1.0).unwrap().stated(), Some(1.0));
    }
    assert!(K::Coincident { a, b }.with_stated(7.5).is_none());
}

// ---- the numerics, case by case ----

/// The independence threshold `σ_ind`, `2^-12`.
const IND: f64 = 0.000_244_140_625;

#[test]
fn the_thresholds_follow_the_block_size_and_the_rounding_bound() {
    let u = 2f64.powi(-53);
    // A block of at most one step wants 1/64, held to σ_ind.
    let t = Thresholds::for_block(0.5, 2, 2, 1.0);
    assert_eq!(
        (t.independent, t.dependent, t.limited_by_rounding),
        (IND, IND, false)
    );
    // A block of 1000 steps: 1/(64 · 1000).
    let t = Thresholds::for_block(1000.0, 2, 2, 1.0);
    assert_eq!(
        (t.dependent, t.limited_by_rounding),
        (1.0 / 64_000.0, false)
    );
    // A rounding bound m·n·u·‖A‖ above 1/(64 S) raises σ_dep to it.
    let t = Thresholds::for_block(1.0e6, 3, 4, 1.0e8);
    assert_eq!(
        (t.dependent, t.limited_by_rounding),
        (12.0 * u * 1.0e8, true)
    );
    // A bound exactly equal to what is wanted does not: 2^-53 · 2^47 = 2^-6 = 1/64.
    let t = Thresholds::for_block(1.0, 1, 1, 2f64.powi(47));
    assert_eq!((t.dependent, t.limited_by_rounding), (IND, false));
    // A block of one step whose bound passes 1/64 is limited, though σ_dep stays σ_ind.
    let t = Thresholds::for_block(0.5, 1, 1, 2f64.powi(50));
    assert_eq!((t.dependent, t.limited_by_rounding), (IND, true));
}

#[test]
fn columns_too_large_or_too_small_to_square_are_measured_by_scaling() {
    for scale in [1.0e200, 1.0e-200] {
        let a = analyse_columns(
            &[vec![scale, 3.0 * scale]],
            &[eq(1, 0, 0)],
            &[0.0],
            2,
            1000.0,
            &mut meter(),
        )
        .unwrap();
        let want = 10f64.sqrt() * scale;
        assert!(
            (a.pivots[0].size - want).abs() <= want * 1e-15,
            "{scale:e}: {} against {want:e}",
            a.pivots[0].size
        );
    }
    // Two columns at right angles, each too large or too small to square: the reflector is scaled
    // too, so the second keeps all of its length.
    for scale in [1.0e200, 1.0e-200] {
        let a = analyse_columns(
            &[vec![scale, 0.0], vec![0.0, 3.0 * scale]],
            &[eq(1, 0, 0), eq(2, 0, 1)],
            &[0.0, 0.0],
            2,
            1000.0,
            &mut meter(),
        )
        .unwrap();
        assert_eq!(a.pivots[0].equation, 1, "{scale:e}");
        for (k, want) in [(0, 3.0 * scale), (1, scale)] {
            let got = a.pivots[k].size;
            assert!((got - want).abs() <= want * 1e-15, "{scale:e}: {got:e}");
        }
    }
}

#[test]
fn a_column_is_preferred_within_a_factor_of_two_and_by_priority_then_order() {
    let pick = |columns: &[Vec<f64>], equations: &[Equation]| {
        let zeros = vec![0.0; columns.len()];
        analyse_columns(columns, equations, &zeros, 2, 1000.0, &mut meter())
            .unwrap()
            .pivots[0]
            .equation
    };
    // A tenth of the largest is not within a factor of 2, whatever its priority.
    let far = [vec![1.0, 0.0], vec![0.0, 0.1]];
    assert_eq!(pick(&far, &[eq(1, 5, 0), eq(2, 0, 1)]), 0);
    // Exactly half is within it, and the lower priority number wins.
    let half = [vec![1.0, 0.0], vec![0.0, 0.5]];
    assert_eq!(pick(&half, &[eq(1, 5, 0), eq(2, 0, 1)]), 1);
    // Equal priority and order: the first stays.
    let same = [vec![1.0, 0.0], vec![0.0, 1.0]];
    assert_eq!(pick(&same, &[eq(1, 0, 0), eq(1, 0, 0)]), 0);
}

#[test]
fn a_column_nearly_along_the_first_axis_is_reflected_without_cancelling() {
    // The second pivot is what is left of (1, 1) off (1, 1e-9): 1 - 1e-9 to rounding.
    let a = analyse_columns(
        &[vec![1.0, 1.0e-9], vec![1.0, 1.0]],
        &[eq(1, 0, 0), eq(2, 1, 1)],
        &[0.0, 0.0],
        2,
        1000.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(a.pivots[0].equation, 0);
    assert!(
        (a.pivots[1].size - (1.0 - 1.0e-9)).abs() < 1e-15,
        "{}",
        a.pivots[1].size
    );
}

#[test]
fn a_cancelled_downdate_is_recomputed() {
    // Taking (1, 0) out of (1, 1e-9) leaves 1e-9, which the downdate 1·√(1 - 1²) loses entirely.
    let a = analyse_columns(
        &[vec![1.0, 0.0], vec![1.0, 1.0e-9]],
        &[eq(1, 0, 0), eq(2, 1, 1)],
        &[0.0, 0.0],
        2,
        1000.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!((a.pivots[0].size, a.pivots[1].size), (1.0, 1.0e-9));
    assert_eq!(a.pivots[1].verdict, Verdict::Dependent);
}

#[test]
fn a_group_leans_by_back_substitution_through_the_diagonal() {
    // (4, 0) is twice (2, 0): with residuals 1 and 2 the group holds exactly.
    let a = analyse_columns(
        &[vec![2.0, 0.0], vec![4.0, 0.0]],
        &[eq(1, 0, 0), eq(2, 1, 1)],
        &[1.0, 2.0],
        2,
        1000.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(a.groups.len(), 1);
    let g = &a.groups[0];
    assert_eq!((g.left_out, g.equations.clone()), (1, vec![0, 1]));
    assert_eq!((g.class, g.defect), (Class::Redundant, 0.0));
}

#[test]
fn an_equation_with_no_gradient_is_a_group_of_its_own() {
    let a = analyse_columns(
        &[vec![1.0, 0.0], vec![0.0, 0.0]],
        &[eq(1, 0, 0), eq(2, 0, 1)],
        &[0.0, 0.0],
        2,
        1000.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(a.groups.len(), 1);
    assert_eq!(a.groups[0].equations, vec![1]);
    assert_eq!(a.groups[0].constraints, vec![id(2, 0)]);
}

#[test]
fn a_group_is_partly_one_constraint_only_when_another_of_its_equations_was_chosen() {
    // Constraint 1 has equations 0 and 2; 2 = 0 + 1 is left out while 0 was chosen: partly.
    let x = |order| Equation {
        constraint: id(1, 0),
        priority: 1,
        order,
    };
    let columns = [vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]];
    let a = analyse_columns(
        &columns,
        &[x(0), eq(2, 0, 1), x(0)],
        &[0.0; 3],
        2,
        1000.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(a.groups.len(), 1);
    assert_eq!(a.groups[0].left_out, 2);
    assert!(a.groups[0].partly);
    let report = SketchReport {
        dof_least: a.dof_least(),
        dof_most: a.dof_most(),
        redundant: a.groups.clone(),
        conflicting: Vec::new(),
        near_singular: Vec::new(),
        special: Vec::new(),
        analysis: a,
    };
    assert!(
        report
            .text()
            .contains("redundant: 1.0, 2.0 (only some of one constraint's equations)"),
        "{}",
        report.text()
    );
    // Both of constraint 1's equations have no gradient: neither was chosen, so neither group is
    // partly.
    let a = analyse_columns(
        &[vec![1.0, 0.0], vec![0.0, 0.0], vec![0.0, 0.0]],
        &[eq(2, 0, 0), x(1), x(1)],
        &[0.0; 3],
        2,
        1000.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(a.groups.len(), 2);
    assert!(a.groups.iter().all(|g| !g.partly));
}

#[test]
fn a_pivot_just_above_sigma_dep_is_confirmed_dependent_by_its_pair() {
    // σ_dep = 1/640000; the pivot δ = 1.2 σ_dep, but the pair's least singular value is δ/√2.
    let dep = 1.0 / 640_000.0;
    let delta = 1.2 * dep;
    let a = analyse_columns(
        &[vec![1.0, 0.0], vec![1.0, delta]],
        &[eq(1, 0, 0), eq(2, 1, 1)],
        &[0.0, 0.0],
        2,
        1.0e4,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(a.thresholds.dependent, dep);
    let p = a.pivots[1];
    assert!((p.size - delta).abs() < 1e-18, "{}", p.size);
    assert_eq!((p.verdict, p.confirmed), (Verdict::Dependent, true));
    assert_eq!(a.groups.len(), 1);
}

#[test]
fn a_pivot_called_dependent_whose_group_is_not_is_confirmed_near_singular() {
    // (1, b, p) leans on (1, 0, 0) and, by b below σ_ind, not on (0, 1, 0): its group is the first
    // and itself, whose least singular value is about b/√2, between the thresholds, while its pivot
    // p is below σ_dep.
    let (b, p) = (1.0e-4, 1.0e-6);
    let a = analyse_columns(
        &[vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0], vec![1.0, b, p]],
        &[eq(1, 0, 0), eq(2, 0, 1), eq(3, 1, 2)],
        &[0.0; 3],
        3,
        1.0e4,
        &mut meter(),
    )
    .unwrap();
    let last = a.pivots[2];
    assert_eq!(last.equation, 2);
    assert!((last.size - p).abs() < 1e-15, "{}", last.size);
    assert_eq!(
        (last.verdict, last.confirmed),
        (Verdict::NearSingular, true)
    );
    assert_eq!((a.near_singular, a.groups.len()), (1, 0));
}

#[test]
fn a_near_singular_pivot_after_a_dependent_one_is_confirmed_too() {
    // Two pivots just above σ_dep, each against (1, 0, 0) alone: both are confirmed dependent.
    let dep = 1.0 / 640_000.0;
    let d = 1.2 * dep;
    let a = analyse_columns(
        &[vec![1.0, 0.0, 0.0], vec![1.0, d, 0.0], vec![1.0, 0.0, d]],
        &[eq(1, 0, 0), eq(2, 1, 1), eq(3, 2, 2)],
        &[0.0; 3],
        3,
        1.0e4,
        &mut meter(),
    )
    .unwrap();
    for k in [1, 2] {
        let p = a.pivots[k];
        assert_eq!(p.equation, k);
        assert_eq!((p.verdict, p.confirmed), (Verdict::Dependent, true), "{k}");
    }
    assert_eq!(a.groups.len(), 2);
}

#[test]
fn lists_of_other_lengths_are_refused() {
    let one = [vec![1.0, 0.0]];
    let refused = |columns: &[Vec<f64>], equations: &[Equation], residuals: &[f64]| {
        matches!(
            analyse_columns(columns, equations, residuals, 2, 1000.0, &mut meter()),
            Err(SketchError::Internal)
        )
    };
    assert!(refused(&one, &[eq(1, 0, 0)], &[]));
    assert!(refused(&one, &[], &[0.0]));
    assert!(refused(&[vec![1.0]], &[eq(1, 0, 0)], &[0.0]));
    assert!(!refused(&one, &[eq(1, 0, 0)], &[0.0]));
}

#[test]
fn the_removal_suggested_is_in_the_most_conflicting_groups_among_equal_priorities() {
    // A, B and C each fix the one variable, at 0, 1 and 2: B and C each conflict with A.
    let a = analyse_columns(
        &[vec![1.0], vec![1.0], vec![1.0]],
        &[eq(1, 1, 0), eq(2, 1, 1), eq(3, 1, 2)],
        &[0.0, 1.0, 2.0],
        1,
        1000.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(a.groups.len(), 2);
    for g in &a.groups {
        assert_eq!(g.class, Class::Conflicting);
        assert_eq!(g.suggested_removal, Some(id(1, 0)), "{g:?}");
    }
}

#[test]
fn the_priorities_come_from_each_constraints_own_line() {
    // The newer distance ranks first (priority 0), so the older one, priority 1, is to be removed.
    let s = sketch(
        "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.5 line 1.0 1.1 segment
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 horizontal 1.5
constraint 2.3 driving 1 distance 1.0 1.1 3000
constraint 2.4 driving 0 distance 1.0 1.1 3100
",
    );
    let r = diagnose(&s, 10_000.0, &mut meter()).unwrap();
    assert_eq!(r.conflicting.len(), 1, "{}", r.text());
    assert_eq!(r.conflicting[0].suggested_removal, Some(id(2, 3)));
}

#[test]
fn remedies_skip_what_is_not_a_driving_dimension() {
    let s = sketch(
        "entity 1.0 point 0 0
entity 1.1 point 3000 0
entity 1.5 line 1.0 1.1 segment
constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
constraint 2.2 driving 0 horizontal 1.5
constraint 2.3 driving 0 distance 1.0 1.1 3000
constraint 2.4 reference 0 distance 1.0 1.1 3000
",
    );
    let found = remedies(&s, &[id(2, 2), id(2, 4), id(9, 9)], &mut meter()).unwrap();
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(remedies(&s, &[id(2, 3)], &mut meter()).unwrap().len(), 1);
}

#[test]
fn the_range_resolution_is_two_to_minus_the_halvings() {
    assert_eq!(
        RANGE_RESOLUTION.to_bits(),
        2f64.powi(-i32::try_from(RANGE_HALVINGS).unwrap()).to_bits()
    );
}

#[test]
fn a_column_at_right_angles_to_those_chosen_is_confirmed_on_its_own() {
    // (0, 0, p) leans on nothing chosen, so its group is itself alone: an independent pivot of
    // p = 3e-4, near σ_ind, stays independent, though the pair before it is near-singular.
    let (e, p) = (2.0e-4, 3.0e-4);
    let a = analyse_columns(
        &[vec![1.0, 0.0, 0.0], vec![1.0, e, 0.0], vec![0.0, 0.0, p]],
        &[eq(1, 0, 0), eq(2, 1, 1), eq(3, 2, 2)],
        &[0.0; 3],
        3,
        1.0e4,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(a.pivots[1].verdict, Verdict::NearSingular);
    let last = a.pivots[2];
    assert_eq!(last.equation, 2);
    assert_eq!(
        (last.verdict, last.confirmed),
        (Verdict::Independent, false)
    );
}

#[test]
fn the_witness_moves_each_variable_up_by_its_own_small_amount() {
    // 1 to 61 of 2^-7 of a step, from each variable's owner, slot and key, and nothing else.
    let s = sketch(
        "entity 1.0 point 0 0
entity 1.1 point 3 4
entity 2.7 point 5 1
",
    );
    let assembly = Assembly::new(&s).unwrap();
    let start = assembly.start().unwrap();
    let moved = witness_point(&assembly).unwrap();
    assert_eq!(moved.len(), start.len());
    let mut amounts = Vec::new();
    for (v, var) in assembly.variables().iter().enumerate() {
        let h = var.owner.sequence * 31 + var.owner.key * 17 + u64::from(var.slot) * 7;
        let amount = f64::from(u32::try_from(h % 61).unwrap() + 1) / 128.0;
        assert_eq!(moved[v] - start[v], amount, "variable {v}");
        amounts.push(amount);
    }
    amounts.dedup();
    assert!(amounts.len() > 1, "{amounts:?}");
}

#[test]
fn a_pivot_is_confirmed_within_a_factor_of_sixteen_of_either_threshold() {
    let t = Thresholds::for_block(1.0e4, 2, 2, 1.0);
    let (ind, dep) = (t.independent, t.dependent);
    for (size, want) in [
        (ind, true),
        (16.0 * ind, true),
        (ind / 16.0, true),
        (17.0 * ind, false),
        (dep, true),
        (16.0 * dep, true),
        (dep / 16.0, true),
        (dep / 17.0, false),
        (1.0, false),
        (0.0, false),
    ] {
        assert_eq!(t.confirms(size), want, "{size:e}");
    }
}

#[test]
fn a_subnormal_column_is_measured_exactly() {
    // Its entries are divided by the smallest normal power of two, which is exact.
    for x in [1.0e-310, 5.0e-324] {
        let a =
            analyse_columns(&[vec![x]], &[eq(1, 0, 0)], &[0.0], 1, 1000.0, &mut meter()).unwrap();
        assert_eq!(a.pivots[0].size, x);
    }
}

#[test]
fn among_constraints_ranked_alike_the_greater_id_is_suggested() {
    // Two constraints of one priority, given the same authoring order on bare data: neither is
    // newer, so the greater id is the one to remove.
    let a = analyse_columns(
        &[vec![1.0], vec![1.0]],
        &[eq(1, 1, 0), eq(2, 1, 0)],
        &[0.0, 1.0],
        1,
        1000.0,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(a.groups.len(), 1);
    assert_eq!(a.groups[0].class, Class::Conflicting);
    assert_eq!(a.groups[0].constraints, vec![id(1, 0), id(2, 0)]);
    assert_eq!(a.groups[0].suggested_removal, Some(id(2, 0)));
}

#[test]
fn a_motion_keeps_the_entries_above_the_cutoff_in_model_units() {
    let var = |slot, scale| Variable {
        owner: id(1, 0),
        slot,
        value: 0.0,
        scale,
    };
    let variables = [var(0, 2.0), var(1, 2.0), var(2, 4.0), var(3, 0.5)];
    let m = Motion::of_row(
        &[MOTION_CUTOFF, -MOTION_CUTOFF, 2.0 * MOTION_CUTOFF, -0.5],
        &variables,
    );
    assert_eq!(
        m.moves,
        vec![(id(1, 0), 2, 8.0 * MOTION_CUTOFF), (id(1, 0), 3, -0.25)]
    );
    assert_eq!(m.owners(), vec![id(1, 0)]);
}

/// The norm of `column` from row `from` down, as the factorisation computes it: the sum of squares
/// of the entries divided by the power of two at or below the largest.
fn fresh_norm(column: &[f64], from: usize) -> f64 {
    let tail = &column[from..];
    let big = tail
        .iter()
        .fold(0.0f64, |acc, x| if x.abs() > acc { x.abs() } else { acc });
    let step = power_of_two(big);
    let mut sum = 0.0;
    for &x in tail {
        let s = x / step;
        sum += s * s;
    }
    sum.sqrt() * step
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for (x, y) in a.iter().zip(b) {
        s += x * y;
    }
    s
}

/// Reflects columns `k..` of `a` from row `k` down by the Householder reflection that takes
/// column `k` there to a multiple of the first axis, as the factorisation does, and puts its pivot
/// on the diagonal.
fn reflect(a: &mut [Vec<f64>], k: usize, norm: f64) {
    let alpha = a[k][k];
    let beta = if alpha >= 0.0 { -norm } else { norm };
    let mut v = a[k][k..].to_vec();
    v[0] = alpha - beta;
    let step = power_of_two(
        v.iter()
            .fold(0.0f64, |acc, x| if x.abs() > acc { x.abs() } else { acc }),
    );
    for x in &mut v {
        *x /= step;
    }
    let vv = dot(&v, &v);
    if vv > 0.0 {
        for column in a.iter_mut().skip(k) {
            let tail = &mut column[k..];
            let scale = 2.0 * dot(&v, tail) / vv;
            for (c, x) in tail.iter_mut().zip(&v) {
                *c -= scale * x;
            }
        }
    }
    for x in a[k].iter_mut().skip(k + 1) {
        *x = 0.0;
    }
    a[k][k] = beta;
}

/// The factorisation with every column's norm computed afresh at every step: the order the columns
/// are taken in, the pivots, and the largest norm at each step. The choices it makes are the ones
/// `analyse_columns` must make bit for bit, whatever it carries between steps.
fn fresh_factor(
    columns: &[Vec<f64>],
    equations: &[Equation],
    rows: usize,
) -> (Vec<usize>, Vec<f64>, Vec<f64>) {
    let m = columns.len();
    let mut a = columns.to_vec();
    let mut order: Vec<usize> = (0..m).collect();
    let (mut sizes, mut maxima) = (Vec::new(), Vec::new());
    for k in 0..rows.min(m) {
        let norms: Vec<f64> = (k..m).map(|j| fresh_norm(&a[j], k)).collect();
        let largest = norms
            .iter()
            .fold(0.0f64, |acc, &n| if n > acc { n } else { acc });
        if largest == 0.0 {
            break;
        }
        let key =
            |order: &[usize], j: usize| (equations[order[j]].priority, equations[order[j]].order);
        let mut best: Option<usize> = None;
        for (offset, &n) in norms.iter().enumerate() {
            let j = k + offset;
            if n * 2.0 >= largest && best.is_none_or(|b| key(&order, j) < key(&order, b)) {
                best = Some(j);
            }
        }
        let p = best.unwrap();
        a.swap(k, p);
        order.swap(k, p);
        let size = fresh_norm(&a[k], k);
        sizes.push(size);
        maxima.push(largest);
        reflect(&mut a, k, size);
    }
    sizes.resize(m, 0.0);
    maxima.resize(m, 0.0);
    (order, sizes, maxima)
}

/// Checks that `analyse_columns` takes the columns in the order, with the pivots and the verdicts
/// before confirmation, that computing every norm afresh gives, for a block of 1024 grid steps
/// (`σ_dep` = 2^-16).
fn assert_as_fresh(columns: &[Vec<f64>], equations: &[Equation], rows: usize, case: &str) {
    let zeros = vec![0.0; columns.len()];
    let got = analyse_columns(columns, equations, &zeros, rows, 1024.0, &mut meter()).unwrap();
    let (order, sizes, maxima) = fresh_factor(columns, equations, rows);
    let t = got.thresholds;
    for (k, p) in got.pivots.iter().enumerate() {
        let factored = if sizes[k] >= t.independent {
            Verdict::Independent
        } else if maxima[k] <= t.dependent {
            Verdict::Dependent
        } else {
            Verdict::NearSingular
        };
        assert_eq!(p.equation, order[k], "{case}: pivot {k}");
        assert_eq!(p.size.to_bits(), sizes[k].to_bits(), "{case}: size {k}");
        // A confirmation changes the verdict the factorisation gave, and only a confirmation does.
        assert_eq!(p.verdict == factored, !p.confirmed, "{case}: verdict {k}");
    }
}

fn equations(keys: &[(u8, usize)]) -> Vec<Equation> {
    keys.iter()
        .enumerate()
        .map(|(j, &(priority, order))| eq(j as u64, priority, order))
        .collect()
}

#[test]
fn a_norm_at_the_factor_of_two_is_decided_on_the_norm_computed_afresh() {
    // (4, 4) goes first. What is left of (0, 1) is then exactly half of what is left of (-2, 0) in
    // exact arithmetic, so (0, 1), of the lower priority number, is within the factor of 2 and
    // goes next. Computed afresh, the two norms keep that ratio to the bit.
    let columns = [vec![0.0, 1.0], vec![4.0, 4.0], vec![-2.0, 0.0]];
    let keys = equations(&[(0, 0), (0, 1), (2, 2)]);
    let a = analyse_columns(&columns, &keys, &[0.0; 3], 2, 1000.0, &mut meter()).unwrap();
    let order: Vec<usize> = a.pivots.iter().map(|p| p.equation).collect();
    assert_eq!(order, vec![1, 0, 2]);
    let mut reflected = vec![columns[1].clone(), columns[0].clone(), columns[2].clone()];
    reflect(&mut reflected, 0, fresh_norm(&columns[1], 0));
    assert_eq!(a.pivots[1].size, fresh_norm(&reflected[1], 1));
    assert_eq!(
        fresh_norm(&reflected[1], 1) * 2.0,
        fresh_norm(&reflected[2], 1)
    );
    // Businger and Golub's downdate, n·√(1 − (y/n)²) from the norm n before the step and the entry
    // y the reflection leaves in row 0, rounds the other way: it puts (0, 1) outside the factor of
    // 2, and would take (-2, 0) next.
    let downdated = |n: f64, y: f64| n * (1.0 - (y / n) * (y / n)).max(0.0).sqrt();
    let (zero, two) = (
        downdated(1.0, reflected[1][0]),
        downdated(2.0, reflected[2][0]),
    );
    assert!(zero * 2.0 < two, "{zero} {two}");
    assert_as_fresh(&columns, &keys, 2, "factor of 2");
}

#[test]
fn choices_the_carried_bounds_cannot_settle_are_made_on_norms_computed_afresh() {
    // σ_dep is 2^-16 for 1024 steps. In each of the first two, a pivot below σ_ind leaves a
    // largest norm within the carried bounds of σ_dep, so the verdict rests on the norms computed
    // afresh: dependent in the first, near-singular in the second.
    let s = 2f64.powi(-16);
    assert_as_fresh(
        &[vec![0.0, -0.5 * s], vec![-s, 1.5 * s], vec![0.0, -s]],
        &equations(&[(2, 0), (2, 3), (1, 1)]),
        2,
        "dependent at sigma_dep",
    );
    assert_as_fresh(
        &[
            vec![-3.0 * s, 2.0 * s, 2.0 * s],
            vec![-2.0 * s, 0.0, 2.0 * s],
            vec![2.0 * s, 3.0 * s, -2.0 * s],
        ],
        &equations(&[(2, 0), (1, 0), (2, 3)]),
        3,
        "near-singular at sigma_dep",
    );
    // And at the factor of 2, inside it and outside it, on columns whose carried norms cannot tell.
    assert_as_fresh(
        &[vec![0.0, 3.0], vec![1.0, 3.0], vec![-1.0, 3.0]],
        &equations(&[(1, 2), (2, 2), (1, 0)]),
        2,
        "inside the factor of 2",
    );
    assert_as_fresh(
        &[vec![3.0, 0.0], vec![12.0, 6.0], vec![-12.0, -9.0]],
        &equations(&[(1, 2), (0, 0), (1, 1)]),
        2,
        "outside the factor of 2",
    );
}

#[test]
fn every_choice_is_the_one_norms_computed_afresh_make() {
    let seed = resolve(
        "diagnose::every_choice_is_the_one_norms_computed_afresh_make",
        Seed::DEFAULT,
    )
    .unwrap();
    let mut rng = SplitMix64::new(seed);
    let mut below = |n: u64| rng.next_u64() % n;
    let scales = [1.0, 3.0, 2f64.powi(-15), 2f64.powi(-16), 2f64.powi(-17)];
    let picks = [0.0, 1.0, -1.0, 2.0, 0.5, 3.0, 0.25];
    for case in 0..4000 {
        let rows = 1 + below(5) as usize;
        let m = 1 + below(6) as usize;
        let kind = below(3);
        let scale = scales[below(scales.len() as u64) as usize];
        let mut columns = Vec::new();
        for _ in 0..m {
            let column: Vec<f64> = (0..rows)
                .map(|_| {
                    let x = match kind {
                        0 => below(9) as f64 - 4.0,
                        1 => picks[below(picks.len() as u64) as usize],
                        _ => (below(1 << 20) as f64 - 524_288.0) / 524_288.0,
                    };
                    x * scale
                })
                .collect();
            columns.push(column);
        }
        let keys: Vec<(u8, usize)> = (0..m)
            .map(|_| (below(3) as u8, below(m as u64 + 1) as usize))
            .collect();
        assert_as_fresh(&columns, &equations(&keys), rows, &format!("case {case}"));
    }
}

#[test]
fn a_norm_bound_starts_from_the_square_and_grows_by_the_derived_terms() {
    let u = 2f64.powi(-53);
    // Computed from a tail of 4 rows: the square, with (3 · 4 + 6)u of it.
    let start = NormBound::computed(3.0, 4).unwrap();
    assert_eq!(
        start,
        NormBound {
            square: 9.0,
            error: 162.0 * u,
            fresh: 9.0
        }
    );
    // Downdated by 2 over the same 4 rows: 4 less, and (9 + error)(8 · 4 + 28)u + 2^-700 more.
    let next = start.downdate(2.0, 4).unwrap();
    assert_eq!(
        next,
        NormBound {
            square: 5.0,
            error: 7.793_765_632_868_61e-14,
            fresh: 9.0
        }
    );
    // The allowance for underflow shows at the smallest square carried: 6u of it to start, then
    // (square + error) · 28u and 2^-700 more.
    let least = NormBound::computed(2f64.powi(-300), 0).unwrap();
    assert_eq!(least.square, 2f64.powi(-600));
    assert_eq!(least.error, 1.605_329_113_043_264e-196);
    assert_eq!(
        least.downdate(0.0, 0).unwrap().error,
        9.096_864_973_911_835e-196
    );
}

#[test]
fn a_norm_is_carried_only_inside_its_window_and_above_the_drift_test() {
    assert!(NormBound::computed(3.0, CARRY_ROWS).is_some());
    assert_eq!(NormBound::computed(3.0, CARRY_ROWS + 1), None);
    assert!(NormBound::computed(2f64.powi(-300), 1).is_some());
    assert_eq!(NormBound::computed(2f64.powi(-300).next_down(), 1), None);
    assert!(NormBound::computed(2f64.powi(300), 1).is_some());
    assert_eq!(NormBound::computed(2f64.powi(300).next_up(), 1), None);
    assert_eq!(NormBound::computed(0.0, 1), None);
    // xGEQP3's drift test: a square at most 2^-26 of the one last computed is computed again.
    let worn = |fresh: f64| NormBound {
        square: 1.0,
        error: 0.0,
        fresh,
    };
    assert_eq!(worn(2f64.powi(26)).downdate(0.0, 1), None);
    assert_eq!(
        worn(2f64.powi(26).next_down()).downdate(0.0, 1),
        Some(NormBound {
            square: 1.0,
            error: 36.0 * 2f64.powi(-53),
            fresh: 2f64.powi(26).next_down()
        })
    );
    assert_eq!(worn(1.0).downdate(1.0, 1), None);
}

#[test]
fn a_norm_bound_widens_the_roots_of_its_least_and_greatest_squares() {
    let b = |square: f64, error: f64| NormBound {
        square,
        error,
        fresh: square,
    };
    assert_eq!(b(4.0, 1.0).squares(), (3.0, 5.0));
    assert_eq!(b(1.0, 2.0).squares(), (0.0, 3.0));
    assert_eq!(
        b(4.0, 0.0).range().unwrap(),
        (1.999_999_999_998_181, 2.000_000_000_001_819)
    );
    assert_eq!(
        b(4.0, 1.0).range().unwrap(),
        (1.732_050_807_567_302, 2.236_067_977_501_823_3)
    );
    assert_eq!(b(1.0, 2.0).range().unwrap().0, 0.0);
}
