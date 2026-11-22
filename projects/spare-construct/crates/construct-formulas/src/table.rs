//! Tables of parameters worked out together: each parameter a value or a formula of others, in
//! any order. Two passes over one table: the kinds (the type check, which needs no values), then
//! the values. Formulas that lead back to themselves are refused, naming the loop.
//!
//! - [`evaluate`]: a table of texts, as Workshop's parts and family recipes write them (a number
//!   in the kind's own unit, a name or a formula, each with its kind).
//! - [`work_out`] and [`check`]: `construct-params` definitions that carry a formula
//!   (CON-A2-F2-P2), over the values of the other parameters of the same element or type.

use crate::FormulaError;
use crate::eval::eval_as;
use crate::parse::{Expr, canonical, parse};
use crate::types::check_as;
use construct_params::{Definition, Holder, Kind, Value, Values};
use kit_core::Id;
use std::collections::BTreeMap;

/// One parameter, as a table of texts holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Its name, as formulas use it.
    pub name: String,
    /// A number, a name or a formula, as written.
    pub text: String,
    /// The kind of value it gives.
    pub kind: Kind,
}

/// Whether a name can be used in a formula: letters, digits and `_`, starting with a letter or
/// `_`.
fn good_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Where a row's value comes from.
enum Source<'a> {
    /// A formula, read, or why it can't be.
    Formula(Result<Expr, FormulaError>),
    /// A value given, or none.
    Given(Option<&'a Value>),
}

struct Row<'a> {
    name: &'a str,
    kind: Kind,
    /// Who holds it, for definitions; a table of texts has none.
    holder: Option<Holder>,
    source: Source<'a>,
}

/// What a name in a formula finds.
enum Slot {
    One(usize),
    /// More than one parameter has this name.
    Many,
}

/// One pass over a table: what a formula gives (its kind, or its value), and what a given value
/// gives.
trait Pass {
    type Out: Clone;
    fn formula(
        &self,
        e: &Expr,
        kind: Kind,
        lookup: &mut dyn FnMut(&str) -> Result<Self::Out, FormulaError>,
    ) -> Result<Self::Out, FormulaError>;
    fn given(&self, row: &Row<'_>, value: Option<&Value>) -> Result<Self::Out, FormulaError>;
}

/// The type check: kinds only.
struct Kinds;

impl Pass for Kinds {
    type Out = Kind;
    fn formula(
        &self,
        e: &Expr,
        kind: Kind,
        lookup: &mut dyn FnMut(&str) -> Result<Kind, FormulaError>,
    ) -> Result<Kind, FormulaError> {
        check_as(e, kind, lookup)?;
        Ok(kind)
    }
    fn given(&self, row: &Row<'_>, _: Option<&Value>) -> Result<Kind, FormulaError> {
        Ok(row.kind)
    }
}

/// Working out: values.
struct Worked;

impl Pass for Worked {
    type Out = Value;
    fn formula(
        &self,
        e: &Expr,
        kind: Kind,
        lookup: &mut dyn FnMut(&str) -> Result<Value, FormulaError>,
    ) -> Result<Value, FormulaError> {
        eval_as(e, kind, lookup)
    }
    fn given(&self, row: &Row<'_>, value: Option<&Value>) -> Result<Value, FormulaError> {
        value
            .cloned()
            .ok_or_else(|| FormulaError::new("Missing", format!("{} has no value.", row.name)))
    }
}

enum State<T> {
    Waiting,
    /// Being worked out: meeting it again means a loop.
    Working,
    Done(Result<T, FormulaError>),
}

struct Table<'a, 'r, P: Pass> {
    pass: &'a P,
    rows: &'a [Row<'r>],
    index: &'a BTreeMap<&'r str, Slot>,
    states: Vec<State<P::Out>>,
    /// The rows being worked out, outermost first, to name a loop.
    path: Vec<usize>,
    /// The rows found on a loop.
    looped: Vec<usize>,
}

impl<P: Pass> Table<'_, '_, P> {
    fn work(&mut self, at: usize) -> Result<P::Out, FormulaError> {
        let Some(state) = self.states.get_mut(at) else {
            return Err(FormulaError::new("Name", "No such parameter."));
        };
        match std::mem::replace(state, State::Working) {
            State::Done(result) => {
                *state = State::Done(result.clone());
                return result;
            }
            State::Working => {
                *state = State::Working;
                let from = self.path.iter().position(|&p| p == at).unwrap_or(0);
                let on_loop = self.path.get(from..).unwrap_or_default().to_vec();
                self.looped.extend_from_slice(&on_loop);
                let names: Vec<&str> = on_loop
                    .iter()
                    .filter_map(|&p| self.rows.get(p).map(|r| r.name))
                    .collect();
                return Err(FormulaError::new(
                    "Circular",
                    match names.as_slice() {
                        [one] => format!("{one} is defined from itself."),
                        _ => format!("{} are defined from each other.", names.join(", ")),
                    },
                ));
            }
            State::Waiting => {}
        }
        let rows = self.rows;
        let Some(row) = rows.get(at) else {
            return Err(FormulaError::new("Name", "No such parameter."));
        };
        self.path.push(at);
        let pass = self.pass;
        let result = match &row.source {
            Source::Given(value) => pass.given(row, *value),
            Source::Formula(Err(e)) => Err(e.clone()),
            Source::Formula(Ok(e)) => {
                let mut lookup = |name: &str| self.lookup(at, name);
                pass.formula(e, row.kind, &mut lookup)
            }
        };
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
        if let Some(state) = self.states.get_mut(at) {
            *state = State::Done(result.clone());
        }
        result
    }

    fn lookup(&mut self, from: usize, name: &str) -> Result<P::Out, FormulaError> {
        let at = match self.index.get(name) {
            Some(Slot::One(at)) => *at,
            Some(Slot::Many) => {
                return Err(FormulaError::new(
                    "Name",
                    format!("More than one parameter is called {name}."),
                ));
            }
            None => {
                return Err(FormulaError::new(
                    "UnknownName",
                    format!("There is no parameter called {name}."),
                ));
            }
        };
        let holders = (
            self.rows.get(from).and_then(|r| r.holder),
            self.rows.get(at).and_then(|r| r.holder),
        );
        if holders == (Some(Holder::Type), Some(Holder::Instance)) {
            return Err(FormulaError::new(
                "Holder",
                format!(
                    "{name} is on each instance, and this is on the type, so its formula can use \
                     only the type's parameters."
                ),
            ));
        }
        self.work(at).map_err(|e| {
            if matches!(e.code, "Circular" | "Depends") {
                e
            } else {
                FormulaError::new("Depends", format!("{name} fails: {}", e.message))
            }
        })
    }
}

/// Runs one pass over every row; `first` holds the earlier pass's refusals, which stand.
fn run<P: Pass>(
    pass: &P,
    rows: &[Row<'_>],
    index: &BTreeMap<&str, Slot>,
    first: &[Option<FormulaError>],
) -> Vec<Result<P::Out, FormulaError>> {
    let states = (0..rows.len())
        .map(|at| match first.get(at) {
            Some(Some(e)) => State::Done(Err(e.clone())),
            _ => State::Waiting,
        })
        .collect();
    let mut table = Table {
        pass,
        rows,
        index,
        states,
        path: Vec::new(),
        looped: Vec::new(),
    };
    (0..rows.len()).map(|at| table.work(at)).collect()
}

/// Both passes: the type check, then the values of every row that passed it.
fn both(
    rows: &[Row<'_>],
    index: &BTreeMap<&str, Slot>,
    read: &[Option<FormulaError>],
) -> Vec<Result<Value, FormulaError>> {
    let kinds: Vec<Option<FormulaError>> = run(&Kinds, rows, index, read)
        .into_iter()
        .map(Result::err)
        .collect();
    run(&Worked, rows, index, &kinds)
}

/// Works out every parameter of a table of texts, in any order. Each gets its value, or why it
/// has none: a name used twice or not usable in a formula, a formula that can't be read, kinds
/// that don't agree (found before any value is worked out), a loop of formulas, a length off the
/// grid, or another parameter it uses that fails.
#[must_use]
pub fn evaluate(entries: &[Entry]) -> Vec<Result<Value, FormulaError>> {
    let mut index = BTreeMap::new();
    let mut read = Vec::with_capacity(entries.len());
    let mut rows = Vec::with_capacity(entries.len());
    for (at, entry) in entries.iter().enumerate() {
        let name = entry.name.as_str();
        let refused = if !good_name(name) {
            Some(FormulaError::new(
                "Name",
                "A name is letters, digits and _, starting with a letter.",
            ))
        } else if index.contains_key(name) {
            Some(FormulaError::new(
                "Name",
                "Another parameter has this name.",
            ))
        } else {
            index.insert(name, Slot::One(at));
            None
        };
        read.push(refused);
        rows.push(Row {
            name,
            kind: entry.kind,
            holder: None,
            source: Source::Formula(parse(&entry.text)),
        });
    }
    both(&rows, &index, &read)
}

/// The rows of a set of definitions, the names formulas find them by, and each formula's refusal
/// when it can't be read or is not in its one spelling.
fn definition_rows<'a>(
    defs: &'a [Definition],
    given: Option<&'a Values>,
) -> (
    Vec<Row<'a>>,
    BTreeMap<&'a str, Slot>,
    Vec<Option<FormulaError>>,
) {
    let mut index: BTreeMap<&str, Slot> = BTreeMap::new();
    let mut rows = Vec::with_capacity(defs.len());
    let mut read = Vec::with_capacity(defs.len());
    for (at, def) in defs.iter().enumerate() {
        if good_name(def.name()) {
            index
                .entry(def.name())
                .and_modify(|slot| *slot = Slot::Many)
                .or_insert(Slot::One(at));
        }
        let (source, refused) = match def.formula() {
            Some(text) => match canonical(text) {
                Ok(spelled) if spelled == text => (Source::Formula(parse(text)), None),
                Ok(spelled) => (
                    Source::Formula(parse(text)),
                    Some(FormulaError::new(
                        "Spelling",
                        format!("A stored formula is in its one spelling, here \"{spelled}\"."),
                    )),
                ),
                Err(e) => (Source::Formula(Err(e.clone())), Some(e)),
            },
            None => (Source::Given(given.and_then(|v| v.get(def.id()))), None),
        };
        rows.push(Row {
            name: def.name(),
            kind: def.kind(),
            holder: Some(def.holder()),
            source,
        });
        read.push(refused);
    }
    (rows, index, read)
}

/// The type check of every definition that carries a formula, without values: each one's formula
/// reads, is in its one spelling, uses parameters that exist and can be used, gives the
/// parameter's kind, is in no loop, and on a type parameter uses only type parameters. The
/// refusals, by parameter ID; none means every formula is sound.
#[must_use]
pub fn check(defs: &[Definition]) -> BTreeMap<Id, FormulaError> {
    let (rows, index, read) = definition_rows(defs, None);
    run(&Kinds, &rows, &index, &read)
        .into_iter()
        .zip(defs)
        .filter(|(_, def)| def.formula().is_some())
        .filter_map(|(result, def)| result.err().map(|e| (def.id().clone(), e)))
        .collect()
}

/// Works out the value of every definition that carries a formula, from `given`: the values of
/// the other parameters of the same element or type (an element's own values and its type's,
/// together). Definitions are worked out in the order they depend on each other; a loop is
/// refused naming its members; each result is of its parameter's kind and is checked against its
/// definition. A value `given` for a parameter with a formula is not read: the formula decides.
#[must_use]
pub fn work_out(defs: &[Definition], given: &Values) -> BTreeMap<Id, Result<Value, FormulaError>> {
    let (rows, index, read) = definition_rows(defs, Some(given));
    both(&rows, &index, &read)
        .into_iter()
        .zip(defs)
        .filter(|(_, def)| def.formula().is_some())
        .map(|(result, def)| {
            let checked = result.and_then(|value| {
                def.check(&value)
                    .map(|()| value)
                    .map_err(|e| FormulaError::new("Type", e.to_string()))
            });
            (def.id().clone(), checked)
        })
        .collect()
}

/// `def` with its value given by `formula`, written in the formula's one spelling.
///
/// # Errors
///
/// As [`canonical`]; `Definition` if the parameter's kind takes no formula.
pub fn define(def: Definition, formula: &str) -> Result<Definition, FormulaError> {
    let spelled = canonical(formula)?;
    def.with_formula(&spelled)
        .map_err(|e| FormulaError::new("Definition", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use construct_params::Length;

    fn entry(name: &str, text: &str) -> Entry {
        Entry {
            name: name.to_owned(),
            text: text.to_owned(),
            kind: Kind::Length,
        }
    }

    fn mm(n: i32) -> Value {
        Value::Length(Length::from_mm(n))
    }

    fn values(entries: &[Entry]) -> Vec<Result<Value, &'static str>> {
        evaluate(entries)
            .into_iter()
            .map(|r| r.map_err(|e| e.code))
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
        assert_eq!(
            values(&table),
            [Ok(mm(112)), Ok(mm(120)), Ok(mm(4)), Ok(mm(56))]
        );
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
        assert_eq!(out[5], Ok(mm(10)));
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
        assert_eq!(out[0], Ok(mm(1)));
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
                kind: Kind::Number,
            },
        ];
        assert_eq!(
            values(&counts)[1],
            Ok(Value::number(construct_params::Decimal::from(4)))
        );
    }
}
