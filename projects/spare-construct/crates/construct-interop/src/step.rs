//! The STEP text form (ISO 10303-21, the clear-text encoding of the exchange structure), written.
//!
//! A [`Step`] collects entity instances (`#12=IFCWALL(...);`), numbers them in the order they are
//! added, and at the end writes the header and data sections. Nothing here knows IFC: an entity is
//! a name and a list of [`Val`] attributes.
//!
//! Why this is ours: the maintained permissive STEP crate (step-p21, Apache-2.0) writes a real from
//! an f64, and an IFC length here is an exact decimal made from grid steps (src/decimal.rs), which
//! an f64 cannot be trusted to print (an exponent, a rounded last place). The crate is used in this
//! crate's tests to read our output back, so the syntax is judged by another implementation.
//!
//! Promises: the same entities in the same order give the same bytes; strings are escaped as the
//! standard says (`''` for an apostrophe, `\\` for a backslash, `\X2\...\X0\` for anything outside
//! printable ASCII); lines end with LF.

use crate::error::IfcError;
use std::fmt::Write as _;

/// One attribute of an entity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Val {
    /// `$`: not given.
    Null,
    /// `*`: derived, so left to the schema.
    Derived,
    /// `#n`: another instance.
    Ref(u64),
    /// A whole number.
    Int(i64),
    /// A real, as its finished text (`250.`, `12.5`).
    Real(String),
    /// A string.
    Text(String),
    /// An enumeration value, written between points (`.STANDARD.`).
    Enum(&'static str),
    /// A boolean (`.T.` or `.F.`).
    Bool(bool),
    /// A list.
    List(Vec<Val>),
}

impl Val {
    /// A string value.
    #[must_use]
    pub fn text(s: &str) -> Val {
        Val::Text(s.to_owned())
    }

    /// A list of references.
    #[must_use]
    pub fn refs(ids: &[u64]) -> Val {
        Val::List(ids.iter().map(|&id| Val::Ref(id)).collect())
    }

    /// A list of reals, from their finished text.
    #[must_use]
    pub fn reals(values: &[String]) -> Val {
        Val::List(values.iter().map(|v| Val::Real(v.clone())).collect())
    }
}

/// Writes a run of characters outside printable ASCII as one `\X2\...\X0\` group of UTF-16 units.
fn flush(out: &mut String, wide: &mut Vec<u16>) {
    if !wide.is_empty() {
        out.push_str("\\X2\\");
        for unit in wide.iter() {
            // Writing to a String cannot fail.
            let _ = write!(out, "{unit:04X}");
        }
        out.push_str("\\X0\\");
        wide.clear();
    }
}

/// A string as the standard writes it, between apostrophes.
fn quote(out: &mut String, text: &str) {
    out.push('\'');
    let mut wide: Vec<u16> = Vec::new();
    for c in text.chars() {
        if (' '..='~').contains(&c) {
            flush(out, &mut wide);
            match c {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                _ => out.push(c),
            }
        } else {
            let mut units = [0_u16; 2];
            wide.extend_from_slice(c.encode_utf16(&mut units));
        }
    }
    flush(out, &mut wide);
    out.push('\'');
}

fn put(out: &mut String, value: &Val) {
    match value {
        Val::Null => out.push('$'),
        Val::Derived => out.push('*'),
        Val::Ref(id) => {
            let _ = write!(out, "#{id}");
        }
        Val::Int(n) => {
            let _ = write!(out, "{n}");
        }
        Val::Real(text) => out.push_str(text),
        Val::Text(text) => quote(out, text),
        Val::Enum(name) => {
            out.push('.');
            out.push_str(name);
            out.push('.');
        }
        Val::Bool(true) => out.push_str(".T."),
        Val::Bool(false) => out.push_str(".F."),
        Val::List(items) => {
            out.push('(');
            for (at, item) in items.iter().enumerate() {
                if at > 0 {
                    out.push(',');
                }
                put(out, item);
            }
            out.push(')');
        }
    }
}

/// The instances of a file, numbered from 1 in the order added.
#[derive(Debug, Default)]
pub struct Step {
    lines: Vec<String>,
}

impl Step {
    /// An empty file.
    #[must_use]
    pub fn new() -> Step {
        Step::default()
    }

    /// Adds `NAME(attributes)` and returns its number.
    ///
    /// # Errors
    ///
    /// [`IfcError::TooBig`] past what a count holds.
    pub fn add(&mut self, entity: &str, attributes: &[Val]) -> Result<u64, IfcError> {
        let id = u64::try_from(self.lines.len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or(IfcError::TooBig)?;
        let mut line = format!("#{id}={entity}(");
        for (at, value) in attributes.iter().enumerate() {
            if at > 0 {
                line.push(',');
            }
            put(&mut line, value);
        }
        line.push_str(");");
        self.lines.push(line);
        Ok(id)
    }

    /// How many instances there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The whole file: the header (`description`, the file's `name`, a `stamp` in ISO 8601, the
    /// originating `system`, and the `schema`) and the data.
    #[must_use]
    pub fn finish(
        &self,
        description: &str,
        name: &str,
        stamp: &str,
        system: &str,
        schema: &str,
    ) -> String {
        let mut out = String::from("ISO-10303-21;\nHEADER;\n");
        let mut header = |entity: &str, values: Vec<Val>| {
            out.push_str(entity);
            put(&mut out, &Val::List(values));
            out.push_str(";\n");
        };
        header(
            "FILE_DESCRIPTION",
            vec![Val::List(vec![Val::text(description)]), Val::text("2;1")],
        );
        header(
            "FILE_NAME",
            vec![
                Val::text(name),
                Val::text(stamp),
                Val::List(vec![Val::text("")]),
                Val::List(vec![Val::text("")]),
                Val::text(system),
                Val::text(system),
                Val::text(""),
            ],
        );
        header("FILE_SCHEMA", vec![Val::List(vec![Val::text(schema)])]);
        out.push_str("ENDSEC;\nDATA;\n");
        for line in &self.lines {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{Step, Val};

    /// Strings are escaped as the standard says.
    #[test]
    fn strings_are_escaped() {
        let mut step = Step::new();
        step.add(
            "E",
            &[Val::text("it's a \\ b"), Val::text("caf\u{e9} \u{1F600}")],
        )
        .unwrap();
        let text = step.finish("d", "n", "2000-01-01T00:00:00", "s", "IFC4X3_ADD2");
        assert!(
            text.contains("#1=E('it''s a \\\\ b','caf\\X2\\00E9\\X0\\ \\X2\\D83DDE00\\X0\\');"),
            "{text}"
        );
    }

    /// Instances are numbered from 1 and every kind of value is written.
    #[test]
    fn every_value_is_written() {
        let mut step = Step::new();
        let a = step.add("A", &[]).unwrap();
        let b = step
            .add(
                "B",
                &[
                    Val::Null,
                    Val::Derived,
                    Val::Ref(a),
                    Val::Int(-3),
                    Val::Real("1.5".into()),
                    Val::Enum("X"),
                    Val::Bool(true),
                    Val::Bool(false),
                    Val::List(vec![Val::Int(1), Val::Int(2)]),
                ],
            )
            .unwrap();
        assert_eq!((a, b), (1, 2));
        let text = step.finish("d", "n", "t", "s", "S");
        assert!(
            text.contains("#2=B($,*,#1,-3,1.5,.X.,.T.,.F.,(1,2));\n"),
            "{text}"
        );
        assert!(text.starts_with("ISO-10303-21;\nHEADER;\n"));
        assert!(text.ends_with("ENDSEC;\nEND-ISO-10303-21;\n"));
    }
}
