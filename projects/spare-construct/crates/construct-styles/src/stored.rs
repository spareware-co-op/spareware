//! The stored form of a project's line styles: one JSON document with one spelling.
//!
//! Field order is fixed, pens are in number order, patterns in ID order, and there is no spare
//! space, so the same styles are always the same bytes. A reader refuses any other spelling of
//! the same data ([`StyleError::NotCanonical`]) so that what is read is what would be written.

use crate::default::{default_annotation_pens, default_model_pens};
use crate::error::StyleError;
use crate::patterns::PatternTable;
use crate::pens::{AnnotationPens, PenTable};
use serde::{Deserialize, Serialize};

/// The version of the stored form this crate writes. Only this version exists so far; a reader never stops reading an old one.
pub const FORMAT_VERSION: u32 = 1;

/// A project's pens and line patterns. Object styles per category (the next job) refer to pens
/// by number and to patterns by ID, and sit beside this, not inside it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LineStyles {
    /// Pens for model lines: a width for each view scale.
    pub model_pens: PenTable,
    /// Pens for annotation and perspective lines: one width.
    pub annotation_pens: AnnotationPens,
    /// The built-in and the project's line patterns.
    pub patterns: PatternTable,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    format: u32,
    model_pens: PenTable,
    annotation_pens: AnnotationPens,
    patterns: PatternTable,
}

impl LineStyles {
    /// The default metric set: see [`crate::default`].
    #[must_use]
    pub fn default_metric() -> Self {
        Self {
            model_pens: default_model_pens(),
            annotation_pens: default_annotation_pens(),
            patterns: PatternTable::built_in(),
        }
    }

    /// The stored bytes: the same styles always give the same bytes.
    ///
    /// # Errors
    /// [`StyleError::Format`] if the writer fails, which it does not for these types.
    pub fn to_bytes(&self) -> Result<Vec<u8>, StyleError> {
        serde_json::to_vec(&Stored {
            format: FORMAT_VERSION,
            model_pens: self.model_pens.clone(),
            annotation_pens: self.annotation_pens.clone(),
            patterns: self.patterns.clone(),
        })
        .map_err(|why| StyleError::Format(why.to_string()))
    }

    /// Reads stored bytes, checking every rule on the way in.
    ///
    /// # Errors
    /// [`StyleError::Format`] for anything that is not valid JSON of this shape (this includes a
    /// table that breaks a rule: its refusal is the message), [`StyleError::Version`] for a
    /// version other than 1, and [`StyleError::NotCanonical`] for valid data spelt another way.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StyleError> {
        let stored: Stored =
            serde_json::from_slice(bytes).map_err(|why| StyleError::Format(why.to_string()))?;
        if stored.format != FORMAT_VERSION {
            return Err(StyleError::Version {
                found: stored.format,
            });
        }
        let styles = Self {
            model_pens: stored.model_pens,
            annotation_pens: stored.annotation_pens,
            patterns: stored.patterns,
        };
        if styles.to_bytes()?.as_slice() != bytes {
            return Err(StyleError::NotCanonical);
        }
        Ok(styles)
    }
}
