//! What a dimension reports when a reference cannot be used: a warning as data, in the shape
//! construct-warnings gives every warning (CON-A2-F7-P1): a stable kind name, how serious it is,
//! the elements involved by permanent ID in a stated order, and a sentence for people.
//!
//! Interim: construct-warnings (piece A77, in train 10) is not on main when this crate is made, and
//! a crate of this layer may not wait on it, so the shape is repeated here field for field.
//! [`DimensionWarning`] converts one to one into construct-warnings' `Warning` when both are on
//! main: its `kind` is the same `&'static str`, its `severity` has the same three words, its
//! `elements` the same IDs in the same order and its `words` the same sentence. The warnings
//! lane's `RULES` table then gets one entry that calls [`crate::measure`] for each dimension.

use kit_core::Id;

/// How serious a warning is (the same three as construct-warnings').
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Worth knowing; nothing is wrong yet.
    Notice,
    /// A problem a person should look at; the model still works.
    Warning,
    /// The model cannot be trusted until it is fixed.
    Error,
}

impl Severity {
    /// The word it is read by: `notice`, `warning` or `error`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Notice => "notice",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// The kind of a warning about a reference that is not there or has no place in the view.
pub const REFERENCE_MISSING: &str = "dimension.reference_missing";
/// The kind of a warning about a line reference that runs along the dimension's offset line, so
/// that it has no place along it.
pub const REFERENCE_PARALLEL: &str = "dimension.reference_parallel";
/// The kind of a warning about a value or a point that does not fit the kernel's grid.
pub const OUT_OF_RANGE: &str = "dimension.out_of_range";

/// One problem a dimension found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DimensionWarning {
    /// Its kind's stable name.
    pub kind: &'static str,
    /// How serious it is.
    pub severity: Severity,
    /// The elements involved, by permanent ID: the dimension, then the element referenced.
    pub elements: Vec<Id>,
    /// A plain sentence for people.
    pub words: String,
}

impl DimensionWarning {
    /// Its identity: its kind and its elements, as `kind|id|id` (construct-warnings' rule), so the
    /// same problem has the same identity for as long as it lasts.
    #[must_use]
    pub fn identity(&self) -> String {
        let mut text = self.kind.to_owned();
        for element in &self.elements {
            text.push('|');
            text.push_str(element.as_str());
        }
        text
    }
}
