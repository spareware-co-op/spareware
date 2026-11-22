//! A parameter table: each parameter a number, another's name or a formula of others, in any
//! order. Formulas that lead back to themselves are refused, naming the loop.

use crate::FormulaError;
use crate::eval::as_kind;
use crate::parse::{Expr, parse};
use crate::quantity::{Kind, Quantity};
use std::collections::BTreeMap;

/// One parameter, as a table holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// A number, a name or a formula, as written.
    pub text: String,
    pub kind: Kind,
}

/// Whether a name can be a parameter's: letters, digits and `_`, starting with a letter or `_`.
fn good_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

enum State {
    /// Read, not yet worked out.
    Waiting(Result<Expr, FormulaError>),
    /// Being worked out: meeting it again means a loop.
    Working,
    Done(Result<Quantity, FormulaError>),
}

struct Table<'a> {
    entries: &'a [Entry],
    index: BTreeMap<&'a str, usize>,
    states: Vec<State>,
    /// The names being worked out, outermost first, to name a loop.
    path: Vec<usize>,
    /// The parameters found on a loop.
    looped: Vec<usize>,
}

impl Table<'_> {
    fn work(&mut self, at: usize) -> Result<Quantity, FormulaError> {
        let state = std::mem::replace(&mut self.states[at], State::Working);
        let expr = match state {
            State::Done(result) => {
                self.states[at] = State::Done(result.clone());
                return result;
            }
            State::Working => {
                self.states[at] = State::Working;
                let from = self.path.iter().position(|&p| p == at).unwrap_or(0);
                self.looped.extend_from_slice(&self.path[from..]);
                let names: Vec<&str> = self.path[from..]
                    .iter()
                    .map(|&p| self.entries[p].name.as_str())
                    .collect();
                return Err(FormulaError::new(
                    "Circular",
                    if names.len() == 1 {
                        format!("{} is defined from itself.", names[0])
                    } else {
                        format!("{} are defined from each other.", names.join(", "))
                    },
                ));
            }
            State::Waiting(expr) => expr,
        };
        self.path.push(at);
        let result = expr.and_then(|expr| {
            let kind = self.entries[at].kind;
            let mut lookup = |name: &str| self.lookup(name);
            as_kind(&expr, kind, &mut lookup)
        });
        self.path.pop();
        // A loop belongs to the parameters on it; one that only uses them depends on it.
        let result = result.map_err(|e| {
            if e.code == "Circular" && !self.looped.contains(&at) {
                FormulaError::new(
                    "Depends",
                    format!("It uses a loop of formulas: {}", e.message),
                )
            } else {
                e
            }
        });
        self.states[at] = State::Done(result.clone());
        result
    }

    fn lookup(&mut self, name: &str) -> Result<Quantity, FormulaError> {
        let Some(&at) = self.index.get(name) else {
            return Err(FormulaError::new(
                "UnknownName",
                format!("There is no parameter called {name}."),
            ));
        };
        self.work(at).map_err(|e| {
            if matches!(e.code, "Circular" | "Depends") {
                e
            } else {
                FormulaError::new("Depends", format!("{name} fails: {}", e.message))
            }
        })
    }
}

/// Works out every parameter of a table, in any order. Each gets its value, or why it has none:
/// a name used twice, a formula that can't be read, units that don't agree, a loop of formulas, or
/// another parameter it uses that fails.
#[must_use]
pub fn evaluate(entries: &[Entry]) -> Vec<Result<Quantity, FormulaError>> {
    let mut index = BTreeMap::new();
    let mut states = Vec::with_capacity(entries.len());
    for (at, entry) in entries.iter().enumerate() {
        let read = if !good_name(&entry.name) {
            Err(FormulaError::new(
                "Name",
                "A name is letters, digits and _, starting with a letter.",
            ))
        } else if index.contains_key(entry.name.as_str()) {
            Err(FormulaError::new(
                "Name",
                "Another parameter has this name.",
            ))
        } else {
            index.insert(entry.name.as_str(), at);
            parse(&entry.text)
        };
        states.push(match read {
            Ok(expr) => State::Waiting(Ok(expr)),
            Err(e) => State::Done(Err(e)),
        });
    }
    let mut table = Table {
        entries,
        index,
        states,
        path: Vec::new(),
        looped: Vec::new(),
    };
    (0..entries.len()).map(|at| table.work(at)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, text: &str) -> Entry {
        Entry {
            name: name.to_owned(),
            text: text.to_owned(),
            kind: Kind::Length,
        }
    }

    fn values(entries: &[Entry]) -> Vec<Result<f64, &'static str>> {
        evaluate(entries)
            .into_iter()
            .map(|r| r.map(|q| q.to_f64()).map_err(|e| e.code))
            .collect()
    }

    #[test]
    fn formulas_read_others_in_any_order() {
        let table = [
            entry("inner", "outer - 2 * wall"),
            entry("outer", "120"),
            entry("wall", "4 mm"),
            entry("half", "inner / 2"),
        ];
        assert_eq!(values(&table), [Ok(112.0), Ok(120.0), Ok(4.0), Ok(56.0)]);
    }

    #[test]
    fn a_loop_is_refused_and_named() {
        let table = [
            entry("a", "b + 1 mm"),
            entry("b", "c"),
            entry("c", "a * 2"),
            entry("d", "a + 1 mm"),
            entry("e", "e"),
            entry("f", "10"),
        ];
        let out = evaluate(&table);
        for result in &out[..3] {
            let e = result.as_ref().unwrap_err();
            assert_eq!(e.code, "Circular");
            assert!(e.message.contains("a, b, c"), "{}", e.message);
        }
        assert_eq!(out[3].as_ref().unwrap_err().code, "Depends");
        assert_eq!(
            out[4].as_ref().unwrap_err().message,
            "e is defined from itself."
        );
        assert_eq!(out[5].as_ref().unwrap().to_f64(), 10.0);
    }

    #[test]
    fn names_and_failures_carry() {
        let table = [
            entry("a", "1 mm"),
            entry("a", "2 mm"),
            entry("2b", "1"),
            entry("c", "a + nothing"),
            entry("d", "c + 1 mm"),
            entry("e", "a +"),
            entry("n", "a / 1 mm"),
        ];
        let out = values(&table);
        assert_eq!(out[0], Ok(1.0));
        assert_eq!(out[1], Err("Name"));
        assert_eq!(out[2], Err("Name"));
        assert_eq!(out[3], Err("UnknownName"));
        assert_eq!(out[4], Err("Depends"));
        assert_eq!(out[5], Err("Syntax"));
        assert_eq!(out[6], Err("Units"), "a count where a length is wanted");
        let counts = [
            entry("a", "3 mm"),
            Entry {
                name: "n".into(),
                text: "a / 1 mm + 1".into(),
                kind: Kind::Count,
            },
        ];
        assert_eq!(values(&counts)[1], Ok(4.0));
    }
}
