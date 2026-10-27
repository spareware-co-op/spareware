//! Shared by the editing, import, turning-count and land-use truth tests: a checker that counts comparisons with the
//! prototype's recorded answers, and small helpers for reading them.
#![allow(dead_code)] // each test file uses its own part of this module

use serde_json::{Value, json};
use std::fmt::Write as _;

#[allow(unused_imports)]
pub use crate::support::{
    PROJECTS, assert_close, diff_json, load_expected, load_input, load_json, load_project,
    read_truth, truth_dir,
};

/// Counts checks against the JavaScript's answers and reports every difference at the end.
#[derive(Default)]
pub struct Checker {
    /// Checks run.
    pub run: usize,
    /// Checks that matched.
    pub passed: usize,
    report: String,
}

impl Checker {
    /// Compares one value with the JavaScript's; a mismatch is recorded with its first differences.
    pub fn check(&mut self, path: &str, actual: &Value, expected: &Value) {
        self.run += 1;
        let (diffs, total) = diff_json(actual, expected, path);
        if total == 0 {
            self.passed += 1;
            return;
        }
        let _ = write!(self.report, "\n{total} difference(s) under {path}:");
        for d in diffs.iter().take(8) {
            let _ = write!(
                self.report,
                "\n  {}\n      expected {}\n      actual   {}",
                d.path, d.expected, d.actual
            );
        }
    }
    /// Counts one exact comparison of two values.
    pub fn same<T: PartialEq + std::fmt::Debug>(&mut self, path: &str, actual: &T, expected: &T) {
        self.run += 1;
        if actual == expected {
            self.passed += 1;
        } else {
            let _ = write!(
                self.report,
                "\n{path}\n      expected {expected:?}\n      actual   {actual:?}"
            );
        }
    }
    /// Prints the counts and panics when any check failed.
    pub fn finish(self, what: &str) {
        println!(
            "{what}: {} of {} checks match the JavaScript",
            self.passed, self.run
        );
        if self.passed != self.run {
            let mut r = self.report;
            if r.len() > 60000 {
                r.truncate(60000);
                r.push_str("\n...");
            }
            panic!(
                "{what}: {} of {} checks differ from the JavaScript:{r}",
                self.run - self.passed,
                self.run
            );
        }
    }
}

/// A number as the harness writes it: finite as is, others as "NaN", "Infinity" or "-Infinity".
pub fn n(x: f64) -> Value {
    if x.is_finite() {
        json!(x)
    } else if x.is_nan() {
        json!("NaN")
    } else if x > 0.0 {
        json!("Infinity")
    } else {
        json!("-Infinity")
    }
}

/// The text of a recorded file under tools/transport-truth.
pub fn text(rel: &str) -> String {
    read_truth(rel)
}
