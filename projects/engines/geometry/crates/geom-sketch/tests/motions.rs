// SPDX-License-Identifier: Apache-2.0
//! Tests of `geom_sketch::motions` (piece G224): the sparse, per-entity free motions equal the dense
//! canonical null space's, motion for motion, on sketches whose freedom is known by construction.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    reason = "tests, written as the definitions read"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fmt::Write;

use geom_run::{Budget, WorkMeter};
use geom_sketch::diagnose::{Motion, free_motions};
use geom_sketch::model::{ItemId, Sketch};
use geom_sketch::motions::MotionFinder;
use geom_sketch::text::parse;

const HEAD: &str = "sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n";

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn sketch(lines: &str) -> Sketch {
    parse(&format!("{HEAD}{lines}")).unwrap()
}

fn same(a: &[Motion], b: &[Motion]) {
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b) {
        assert_eq!(x.moves.len(), y.moves.len(), "{x:?} against {y:?}");
        for (p, q) in x.moves.iter().zip(&y.moves) {
            assert_eq!((p.0, p.1), (q.0, q.1));
            assert!((p.2 - q.2).abs() <= 1e-6 * (1.0 + p.2.abs()), "{x:?} against {y:?}");
        }
    }
}

/// A chain of points, each linked to the last by a distance, the first fixed.
fn chain(points: usize) -> Sketch {
    let mut text = String::new();
    for i in 0..points {
        writeln!(text, "entity 1.{i} point {} {}", i as f64 * 100.0, i as f64 * 50.0).unwrap();
    }
    writeln!(text, "constraint 2.0 driving 0 fix 1.0 0 0").unwrap();
    writeln!(text, "constraint 2.1 driving 0 fix 1.0 1 0").unwrap();
    for i in 1..points {
        writeln!(text, "constraint 3.{i} driving 0 across 1.{} 1.{i} 100", i - 1).unwrap();
    }
    sketch(&text)
}

#[test]
fn it_equals_the_dense_form_with_no_constraints() {
    let s = sketch("entity 1.0 point 1 2\nentity 1.1 point 5 7\n");
    let finder = MotionFinder::new(&s, &mut meter()).unwrap();
    assert_eq!(finder.dof(), 4);
    same(&finder.all(&mut meter()).unwrap(), &free_motions(&s, &mut meter()).unwrap());
}

#[test]
fn it_equals_the_dense_form_on_a_distance_chain() {
    let s = sketch(
        "entity 1.0 point 0 0\nentity 1.1 point 3000 0\nentity 1.2 point 3000 4000\n\
         constraint 3.0 driving 0 fix 1.0 0 0\nconstraint 3.1 driving 0 distance 1.0 1.1 3000\n\
         constraint 3.2 driving 0 distance 1.1 1.2 4000\n",
    );
    let finder = MotionFinder::new(&s, &mut meter()).unwrap();
    let dense = free_motions(&s, &mut meter()).unwrap();
    assert_eq!(finder.dof(), dense.len());
    same(&finder.all(&mut meter()).unwrap(), &dense);
}

#[test]
fn it_equals_the_dense_form_when_only_y_is_free_along_a_chain() {
    let s = chain(40);
    let finder = MotionFinder::new(&s, &mut meter()).unwrap();
    let dense = free_motions(&s, &mut meter()).unwrap();
    assert_eq!(finder.dof(), 39);
    same(&finder.all(&mut meter()).unwrap(), &dense);
}

#[test]
fn one_entity_gets_only_its_own_motions() {
    let s = chain(10);
    let finder = MotionFinder::new(&s, &mut meter()).unwrap();
    let all = finder.all(&mut meter()).unwrap();
    let id = ItemId::new(1, 4);
    let mine = finder.of_entity(id, &mut meter()).unwrap();
    assert_eq!(mine.len(), 1);
    assert!(all.contains(&mine[0]));
    assert!(finder.of_entity(ItemId::new(9, 9), &mut meter()).unwrap().is_empty());
}

#[test]
fn a_fully_constrained_sketch_has_no_motion() {
    let mut text = String::new();
    for i in 0..5 {
        writeln!(text, "entity 1.{i} point {} {}", i as f64 * 100.0, i as f64 * 50.0).unwrap();
    }
    text.push_str("constraint 2.0 driving 0 fix 1.0 0 0
constraint 2.1 driving 0 fix 1.0 1 0
");
    for i in 1..5 {
        writeln!(text, "constraint 3.{i} driving 0 across 1.{} 1.{i} 100", i - 1).unwrap();
        writeln!(text, "constraint 4.{i} driving 0 upward 1.{} 1.{i} 50", i - 1).unwrap();
    }
    let finder = MotionFinder::new(&sketch(&text), &mut meter()).unwrap();
    assert_eq!(finder.dof(), 0);
    assert!(finder.all(&mut meter()).unwrap().is_empty());
}

#[test]
fn a_budget_too_small_stops_it() {
    let s = chain(10);
    let mut small = WorkMeter::new(Budget::of_work(1));
    assert!(MotionFinder::new(&s, &mut small).is_err());
}
