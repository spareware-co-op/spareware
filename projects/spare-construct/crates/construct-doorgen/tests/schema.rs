//! The generated families round-trip through family recipe format 1: each validates against the
//! published schema, reads back from its written form unchanged, keeps its content id, and every
//! type it names evaluates; the same specs give the same bytes on a second run.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{make, mm, overall, tenths};
use construct_doorgen::{Hand, Spec, Template};
use construct_families::{Evaluator, Recipe, SCHEMA};

/// A small catalogue of each template: three sizes, one off the whole millimetre, one with its
/// own parts.
fn catalogue(template: Template) -> Vec<Spec> {
    let (w, h) = (template.widths(), template.heights());
    let mut own = Spec::new("X3", template, overall(w.high, h.low));
    own.frame = Some(mm(60));
    if template == Template::SingleSwingDoor {
        own.hand = Some(Hand::Right);
    } else {
        own.glass = Some(mm(10));
        own.sill_height = Some(mm(600));
    }
    vec![
        Spec::new("X1", template, overall(w.low, h.high)),
        Spec::new("X2", template, overall(tenths(10005), tenths(20005))),
        own,
    ]
}

#[test]
fn every_generated_family_validates_and_reads_back_unchanged() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for template in Template::ALL {
        let generated = make(&catalogue(template)).unwrap();
        let text = generated.recipe.to_json();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        assert!(errors.is_empty(), "{template:?}: {errors:#?}");
        let read = Recipe::from_json(&text).unwrap();
        assert_eq!(read, generated.recipe, "{template:?}");
        assert_eq!(read.to_json(), text, "{template:?}");
        assert_eq!(read.content_id(), generated.recipe.content_id());
        assert_eq!(value["id"], format!("project.test.{}", template.stem()));
        // Every type it names evaluates, on an engine of the caller's choosing.
        let mut evaluator = Evaluator::new(construct_doorgen::measure::Measure);
        let built = evaluator.evaluate_types(&read);
        assert_eq!(built.len(), 3);
        assert!(built.iter().all(|(_, r)| r.is_ok()), "{template:?}");
    }
}

#[test]
fn the_same_specs_give_the_same_bytes_on_a_second_run() {
    for template in Template::ALL {
        let one = make(&catalogue(template)).unwrap();
        let two = make(&catalogue(template)).unwrap();
        assert_eq!(one.recipe.to_json(), two.recipe.to_json(), "{template:?}");
        assert_eq!(one, two, "{template:?}");
    }
}
