//! What a recipe means, checked beyond its shape: every definition and value as construct-params
//! takes them; every formula read by construct-formulas, naming only number parameters, with
//! units that agree and no loop; every key a sketch, step, symbol or type names present, of the
//! right sort, and a step's inputs earlier than it; every profile one closed loop of lines; every
//! category, subcategory and material known to the project; every type value one its type holds,
//! and every type table row one such value per column; and no two things with one key (built-in
//! types and table rows share one set of keys and one of names). Each problem names its place as a JSON Pointer, and every
//! problem is reported, in document order.

use crate::model::{
    Base, Constraint, Hosting, Key, MaterialRef, Operation, Parameter, ParameterValue, Recipe,
    Sketch, View,
};
use construct_categories::{CategoryId, CategoryTable};
use construct_formulas::{Entry, Expr, FormulaError, evaluate, parse, value_of};
use construct_materials::{MaterialId, Materials};
use construct_params::{Definition, Holder, Kind, ParamError, Value};
use kit_core::Id;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// What a recipe is checked against: the project's categories (its own subcategories with the
/// built-in tree) and its materials.
#[derive(Clone, Copy)]
pub struct Library<'a> {
    /// The project's category table.
    pub categories: &'a CategoryTable,
    /// The project's materials.
    pub materials: &'a Materials,
}

/// The sort of thing a key names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    /// A parameter.
    Parameter,
    /// A plane.
    Plane,
    /// A sketch.
    Sketch,
    /// A sketch's point.
    Point,
    /// A sketch's straight line.
    Line,
    /// A sketch's arc or circle.
    Curve,
    /// A sketch's line, arc or circle.
    Edge,
    /// A step.
    Step,
}

impl fmt::Display for Sort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Parameter => "parameter",
            Self::Plane => "plane",
            Self::Sketch => "sketch",
            Self::Point => "point",
            Self::Line => "line",
            Self::Curve => "arc or circle",
            Self::Edge => "line, arc or circle",
            Self::Step => "step",
        })
    }
}

/// What is wrong at one place.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProblemKind {
    /// A name with nothing in it but spaces.
    EmptyName,
    /// Another thing in the same list (or the same sketch) already has this key.
    Duplicate {
        /// The key.
        key: String,
    },
    /// Another parameter already has this permanent id.
    DuplicateId {
        /// The id.
        id: Id,
    },
    /// Another built-in type already has this name (spaces at the ends and case do not count).
    DuplicateName {
        /// The name.
        name: String,
    },
    /// construct-params refuses the definition.
    BadDefinition(ParamError),
    /// construct-params refuses the value for its parameter, or it is not in its one spelling.
    BadValue(ParamError),
    /// A sketch's coordinate or radius is not a length in its one stored spelling.
    NotALength {
        /// The text.
        text: String,
    },
    /// Nothing of this sort has this key: a point a constraint names, a sketch or step a step
    /// uses, a parameter a formula names.
    Missing {
        /// What was looked for.
        sort: Sort,
        /// The key.
        key: String,
    },
    /// The key names something of another sort.
    WrongSort {
        /// The key.
        key: String,
        /// What was wanted.
        wanted: Sort,
        /// What it names.
        found: Sort,
    },
    /// A step uses a step that is not earlier than it: a later one, or itself.
    NotEarlier {
        /// The step it uses.
        key: String,
    },
    /// A formula names a parameter that is not a number (a length, an angle, a number or a whole
    /// number), or a parameter that is not a number has a formula.
    NotNumeric {
        /// The parameter.
        key: String,
    },
    /// construct-formulas refuses the formula: it cannot be read, its units do not agree, it
    /// divides by zero at the defaults, and so on (the error's `code` says which).
    Formula(FormulaError),
    /// These parameters' formulas lead back to themselves.
    Cycle {
        /// The parameters on the loop, in document order.
        keys: Vec<String>,
    },
    /// A profile is not one closed loop of three or more lines, each sharing a point with the
    /// next and the last with the first.
    NotALoop,
    /// The sketch is not on a plane of the base this needs (a plan symbol on xy, an elevation
    /// symbol or an opening on xz).
    WrongPlane {
        /// The base needed.
        wanted: Base,
    },
    /// The project has no category of this id.
    UnknownCategory {
        /// The id.
        id: CategoryId,
    },
    /// The category is neither the family's category nor one of its subcategories.
    NotOwnSubcategory {
        /// The id.
        id: CategoryId,
    },
    /// The project has no material of this id.
    UnknownMaterial {
        /// The id.
        id: Id,
    },
    /// A material is taken from a parameter that is not a material parameter.
    NotAMaterial {
        /// The parameter.
        key: String,
    },
    /// A symbol's condition is on a parameter that is neither yes or no nor a choice.
    NotACondition {
        /// The parameter.
        key: String,
    },
    /// A type sets a parameter each instance holds.
    HeldByInstance {
        /// The parameter.
        key: String,
    },
    /// A type sets a parameter that is worked out by its formula.
    SetsFormula {
        /// The parameter.
        key: String,
    },
    /// A type table's row has another number of values than the table has columns.
    ValueCount {
        /// The row's values.
        values: usize,
        /// The table's columns.
        columns: usize,
    },
}

impl fmt::Display for ProblemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => f.write_str("the name is empty"),
            Self::Duplicate { key } => write!(f, "the key {key} is used twice"),
            Self::DuplicateId { id } => write!(f, "two parameters have the id {id}"),
            Self::DuplicateName { name } => write!(f, "two types are named {name}"),
            Self::BadDefinition(e) | Self::BadValue(e) => write!(f, "{e}"),
            Self::NotALength { text } => write!(f, "{text} is not a length in its stored spelling"),
            Self::Missing { sort, key } => write!(f, "there is no {sort} {key}"),
            Self::WrongSort { key, wanted, found } => {
                write!(f, "{key} is a {found}, and a {wanted} is wanted")
            }
            Self::NotEarlier { key } => write!(f, "the step {key} does not come earlier"),
            Self::NotNumeric { key } => write!(f, "{key} is not a number a formula can use"),
            Self::Formula(e) => write!(f, "{e}"),
            Self::Cycle { keys } => write!(f, "{} are worked out from each other", keys.join(", ")),
            Self::NotALoop => f.write_str("the lines are not one closed loop"),
            Self::WrongPlane { wanted } => {
                write!(
                    f,
                    "the sketch is not on a plane parallel to {}",
                    wanted.name()
                )
            }
            Self::UnknownCategory { id } => write!(f, "there is no category {}", id.0),
            Self::NotOwnSubcategory { id } => {
                write!(
                    f,
                    "category {} is not the family's or one of its subcategories",
                    id.0
                )
            }
            Self::UnknownMaterial { id } => write!(f, "there is no material {id}"),
            Self::NotAMaterial { key } => write!(f, "{key} is not a material parameter"),
            Self::NotACondition { key } => {
                write!(f, "{key} is neither a yes or no nor a choice parameter")
            }
            Self::HeldByInstance { key } => {
                write!(f, "{key} is held by each instance, so a type cannot set it")
            }
            Self::SetsFormula { key } => write!(f, "{key} is worked out by its formula"),
            Self::ValueCount { values, columns } => {
                write!(f, "the row has {values} values for {columns} columns")
            }
        }
    }
}

/// One problem and where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    at: String,
    kind: ProblemKind,
}

impl Problem {
    /// Where, as a JSON Pointer (RFC 6901) into the recipe's document.
    #[must_use]
    pub fn at(&self) -> &str {
        &self.at
    }

    /// What is wrong.
    #[must_use]
    pub fn kind(&self) -> &ProblemKind {
        &self.kind
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {}: {}", self.at, self.kind)
    }
}

/// The construct-params definition a row describes, made by construct-params' own reader of its
/// JSON, so every rule it keeps is kept here.
pub(crate) fn definition(parameter: &Parameter) -> Result<Definition, ParamError> {
    let row = &parameter.definition;
    let choices: Vec<_> = row
        .choices
        .iter()
        .map(|c| json!({"key": c.key, "label": c.label}))
        .collect();
    let text = json!({
        "id": row.id.as_str(),
        "name": row.name,
        "kind": row.kind.tag(),
        "holder": match row.holder {
            Holder::Instance => "instance",
            Holder::Type => "type",
        },
        "choices": choices,
    })
    .to_string();
    Definition::from_json(&text)
}

/// A stored value of a definition, in its one spelling.
pub(crate) fn stored_value(def: &Definition, text: &str) -> Result<Value, ParamError> {
    let value = def.read(text)?;
    if value.stored() != text {
        return Err(ParamError::BadStored {
            parameter: def.name().to_owned(),
            text: text.to_owned(),
        });
    }
    Ok(value)
}

/// The formula kind of a parameter kind, if formulas can use it.
pub(crate) fn formula_kind(kind: Kind) -> Option<Kind> {
    match kind {
        Kind::Length | Kind::Angle | Kind::Number | Kind::Whole => Some(kind),
        _ => None,
    }
}

/// A value as formula text that reads back to it exactly: a length in exact millimetres (its own
/// text, whatever its step count), an angle as a ratio of degrees, numbers as they are.
pub(crate) fn formula_text(def: &Definition, value: &Value) -> Option<String> {
    match value {
        Value::Length(length) => {
            def.check(value).ok()?;
            Some(length.to_string())
        }
        Value::Angle(angle) => {
            let degrees = angle.degrees();
            Some(format!("{} deg / {}", degrees.numer(), degrees.denom()))
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Whole(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Every parameter name a formula reads.
pub(crate) fn names(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Name(name) => out.push(name.clone()),
        Expr::Neg(inner) | Expr::Not(inner) => names(inner, out),
        Expr::Binary(_, a, b) => {
            names(a, out);
            names(b, out);
        }
        Expr::If(test, yes, no) => {
            names(test, out);
            names(yes, out);
            names(no, out);
        }
        Expr::Call(_, args) => {
            for arg in args {
                names(arg, out);
            }
        }
        _ => {}
    }
}

/// The sort of a sketch item.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    Point,
    Line,
    Curve,
}

impl Item {
    fn sort(self) -> Sort {
        match self {
            Self::Point => Sort::Point,
            Self::Line => Sort::Line,
            Self::Curve => Sort::Curve,
        }
    }

    fn is(self, wanted: Sort) -> bool {
        matches!(
            (self, wanted),
            (Self::Point, Sort::Point)
                | (Self::Line, Sort::Line | Sort::Edge)
                | (Self::Curve, Sort::Curve | Sort::Edge)
        )
    }
}

/// A sketch's items by key (the first of each key) and its lines' ends.
struct Items<'r> {
    sorts: BTreeMap<&'r str, Item>,
    lines: BTreeMap<&'r str, (&'r str, &'r str)>,
}

impl<'r> Items<'r> {
    fn of(sketch: &'r Sketch) -> Self {
        let mut sorts = BTreeMap::new();
        let mut lines = BTreeMap::new();
        for p in &sketch.points {
            sorts.entry(p.key.as_str()).or_insert(Item::Point);
        }
        for l in &sketch.lines {
            if !sorts.contains_key(l.key.as_str()) {
                sorts.insert(l.key.as_str(), Item::Line);
                lines.insert(l.key.as_str(), (l.start.as_str(), l.end.as_str()));
            }
        }
        for a in &sketch.arcs {
            sorts.entry(a.key.as_str()).or_insert(Item::Curve);
        }
        for c in &sketch.circles {
            sorts.entry(c.key.as_str()).or_insert(Item::Curve);
        }
        Self { sorts, lines }
    }
}

/// What the checker knows of one parameter (the first with its key).
struct Param<'r> {
    row: &'r Parameter,
    definition: Option<Definition>,
}

/// A formula's verdict, worked out before the problems are written in document order.
#[derive(Clone)]
enum Worked {
    /// Its value at the defaults.
    Value(Value),
    /// It fails because something it reads fails, which is reported there.
    Depends,
    /// Its own problems.
    Problems(Vec<ProblemKind>),
}

struct Checker<'r, 'l> {
    recipe: &'r Recipe,
    library: Library<'l>,
    problems: Vec<Problem>,
    params: BTreeMap<&'r str, Param<'r>>,
    /// Each number parameter's value at the defaults, or why it has none.
    worked: BTreeMap<&'r str, Worked>,
    /// Whether the family's category is known, so its subcategories can be asked about.
    category_known: bool,
}

/// Checks a recipe against a project's library.
pub(crate) fn check(recipe: &Recipe, library: &Library<'_>) -> Result<(), Vec<Problem>> {
    let mut params = BTreeMap::new();
    for row in &recipe.parameters {
        params.entry(row.key.as_str()).or_insert_with(|| Param {
            row,
            definition: definition(row).ok(),
        });
    }
    let mut checker = Checker {
        recipe,
        library: *library,
        problems: Vec::new(),
        params,
        worked: BTreeMap::new(),
        category_known: library.categories.get(recipe.category).is_some(),
    };
    checker.work_parameters();
    checker.run();
    if checker.problems.is_empty() {
        Ok(())
    } else {
        Err(checker.problems)
    }
}

impl<'r> Checker<'r, '_> {
    fn push(&mut self, at: String, kind: ProblemKind) {
        self.problems.push(Problem { at, kind });
    }

    /// The problems with the names a formula reads: each must be a number parameter. `None` if
    /// they are all good; the names' own failures count as `Depends`.
    fn name_problems(&self, expr: &Expr) -> Option<Worked> {
        let mut read = Vec::new();
        names(expr, &mut read);
        let mut problems = Vec::new();
        let mut depends = false;
        for name in read {
            match self.params.get(name.as_str()) {
                None => problems.push(ProblemKind::Missing {
                    sort: Sort::Parameter,
                    key: name,
                }),
                Some(Param {
                    definition: None, ..
                }) => depends = true,
                Some(Param {
                    definition: Some(def),
                    ..
                }) => {
                    if formula_kind(def.kind()).is_none() {
                        problems.push(ProblemKind::NotNumeric { key: name });
                    }
                }
            }
        }
        if !problems.is_empty() {
            Some(Worked::Problems(problems))
        } else if depends {
            Some(Worked::Depends)
        } else {
            None
        }
    }

    /// Works out every number parameter at the defaults: its default, or its formula, with the
    /// names each reads checked first and any loop found and named.
    fn work_parameters(&mut self) {
        // Each number parameter's entry for construct-formulas' table, and the formulas' reads.
        let mut entries: BTreeMap<&'r str, Entry> = BTreeMap::new();
        let mut reads: BTreeMap<&'r str, Vec<String>> = BTreeMap::new();
        for (&key, param) in &self.params {
            let Some(def) = &param.definition else {
                continue;
            };
            let Some(kind) = formula_kind(def.kind()) else {
                continue;
            };
            match &param.row.value {
                ParameterValue::Default(text) => {
                    let value = stored_value(def, text).ok();
                    if let Some(text) = value.and_then(|v| formula_text(def, &v)) {
                        entries.insert(
                            key,
                            Entry {
                                name: key.to_owned(),
                                text,
                                kind,
                            },
                        );
                    }
                }
                ParameterValue::Formula(text) => match parse(text) {
                    Err(e) => {
                        self.worked
                            .insert(key, Worked::Problems(vec![ProblemKind::Formula(e)]));
                    }
                    Ok(expr) => {
                        if let Some(verdict) = self.name_problems(&expr) {
                            self.worked.insert(key, verdict);
                            continue;
                        }
                        let mut read = Vec::new();
                        names(&expr, &mut read);
                        reads.insert(key, read);
                        entries.insert(
                            key,
                            Entry {
                                name: key.to_owned(),
                                text: text.clone(),
                                kind,
                            },
                        );
                    }
                },
            }
        }
        self.find_loops(&reads, &mut entries);
        // A formula that reads one left out fails because of it.
        loop {
            let left_out: Vec<&'r str> = reads
                .iter()
                .filter(|(key, read)| {
                    entries.contains_key(*key)
                        && read.iter().any(|name| !entries.contains_key(name.as_str()))
                })
                .map(|(key, _)| *key)
                .collect();
            if left_out.is_empty() {
                break;
            }
            for key in left_out {
                entries.remove(key);
                self.worked.entry(key).or_insert(Worked::Depends);
            }
        }
        let keys: Vec<&'r str> = entries.keys().copied().collect();
        let table: Vec<Entry> = entries.into_values().collect();
        for (key, result) in keys.into_iter().zip(evaluate(&table)) {
            let verdict = match result {
                Ok(q) => Worked::Value(q),
                Err(e) if e.code == "Depends" => Worked::Depends,
                Err(e) => Worked::Problems(vec![ProblemKind::Formula(e)]),
            };
            self.worked.insert(key, verdict);
        }
    }

    /// Finds every loop of formulas, records one problem per loop on its first parameter in
    /// document order, and leaves the parameters on it out of the table.
    fn find_loops(
        &mut self,
        reads: &BTreeMap<&'r str, Vec<String>>,
        entries: &mut BTreeMap<&'r str, Entry>,
    ) {
        let reach = |from: &str| -> BTreeSet<String> {
            let mut seen = BTreeSet::new();
            let mut next: Vec<String> = reads.get(from).cloned().unwrap_or_default();
            while let Some(name) = next.pop() {
                if seen.insert(name.clone())
                    && let Some(more) = reads.get(name.as_str())
                {
                    next.extend(more.iter().cloned());
                }
            }
            seen
        };
        let order: Vec<&'r str> = self
            .recipe
            .parameters
            .iter()
            .map(|p| p.key.as_str())
            .filter(|key| reads.contains_key(key))
            .collect();
        let mut on_loop: BTreeSet<&'r str> = BTreeSet::new();
        for &key in &order {
            if on_loop.contains(key) {
                continue;
            }
            let from = reach(key);
            if !from.contains(key) {
                continue;
            }
            let members: Vec<&'r str> = order
                .iter()
                .copied()
                .filter(|other| {
                    *other == key || (from.contains(*other) && reach(other).contains(key))
                })
                .collect();
            for &member in &members {
                on_loop.insert(member);
                entries.remove(member);
                self.worked.insert(member, Worked::Depends);
            }
            self.worked.insert(
                key,
                Worked::Problems(vec![ProblemKind::Cycle {
                    keys: members.iter().map(|m| (*m).to_owned()).collect(),
                }]),
            );
        }
    }

    /// Checks a length formula outside the parameter table, at `at`.
    fn length_formula(&mut self, text: &str, at: String) {
        let expr = match parse(text) {
            Ok(expr) => expr,
            Err(e) => return self.push(at, ProblemKind::Formula(e)),
        };
        match self.name_problems(&expr) {
            Some(Worked::Problems(problems)) => {
                for problem in problems {
                    self.push(at.clone(), problem);
                }
                return;
            }
            Some(_) => return,
            None => {}
        }
        let worked = &self.worked;
        let mut lookup = |name: &str| match worked.get(name) {
            Some(Worked::Value(q)) => Ok(q.clone()),
            _ => Err(FormulaError {
                code: "Depends",
                message: format!("{name} fails"),
            }),
        };
        match value_of(text, Kind::Length, &mut lookup) {
            Err(e) if e.code != "Depends" => self.push(at, ProblemKind::Formula(e)),
            _ => {}
        }
    }

    fn run(&mut self) {
        let recipe = self.recipe;
        if recipe.name.trim().is_empty() {
            self.push("/name".into(), ProblemKind::EmptyName);
        }
        if !self.category_known {
            self.push(
                "/category".into(),
                ProblemKind::UnknownCategory {
                    id: recipe.category,
                },
            );
        }
        self.parameters();
        let mut seen = BTreeSet::new();
        for (n, plane) in recipe.planes.iter().enumerate() {
            let at = format!("/planes/{n}");
            self.unique(&mut seen, &plane.key, format!("{at}/key"));
            self.length_formula(&plane.offset, format!("{at}/offset"));
        }
        let mut seen = BTreeSet::new();
        for (n, sketch) in recipe.sketches.iter().enumerate() {
            self.unique(&mut seen, &sketch.key, format!("/sketches/{n}/key"));
            self.sketch(sketch, &format!("/sketches/{n}"));
        }
        self.steps();
        self.symbols();
        if let Hosting::Wall { opening } = &recipe.hosting
            && let Some(sketch) =
                self.sketch_on(&opening.sketch, Some(Base::Xz), "/hosting/opening/sketch")
        {
            self.profile(sketch, &opening.profile, "/hosting/opening/profile");
        }
        self.types();
    }

    /// Records a duplicate if `key` is already in `seen`.
    fn unique(&mut self, seen: &mut BTreeSet<&'r str>, key: &'r Key, at: String) {
        if !seen.insert(key.as_str()) {
            self.push(
                at,
                ProblemKind::Duplicate {
                    key: key.as_str().to_owned(),
                },
            );
        }
    }

    fn parameters(&mut self) {
        let recipe = self.recipe;
        let mut keys = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for (n, row) in recipe.parameters.iter().enumerate() {
            let at = format!("/parameters/{n}");
            self.unique(&mut keys, &row.key, format!("{at}/key"));
            if !ids.insert(&row.definition.id) {
                self.push(
                    format!("{at}/definition/id"),
                    ProblemKind::DuplicateId {
                        id: row.definition.id.clone(),
                    },
                );
            }
            let def = match definition(row) {
                Ok(def) => def,
                Err(e) => {
                    self.push(format!("{at}/definition"), ProblemKind::BadDefinition(e));
                    continue;
                }
            };
            let first = self
                .params
                .get(row.key.as_str())
                .is_some_and(|p| std::ptr::eq(p.row, row));
            match &row.value {
                ParameterValue::Default(text) => {
                    self.value(&def, text, format!("{at}/default"));
                }
                ParameterValue::Formula(_) => {
                    let at = format!("{at}/formula");
                    if formula_kind(def.kind()).is_none() {
                        self.push(
                            at,
                            ProblemKind::NotNumeric {
                                key: row.key.as_str().to_owned(),
                            },
                        );
                    } else if first
                        && let Some(Worked::Problems(problems)) =
                            self.worked.get(row.key.as_str()).cloned()
                    {
                        for problem in problems {
                            self.push(at.clone(), problem);
                        }
                    }
                }
            }
        }
    }

    /// Checks a stored value of a definition, and a material value against the library.
    fn value(&mut self, def: &Definition, text: &str, at: String) {
        match stored_value(def, text) {
            Err(e) => self.push(at, ProblemKind::BadValue(e)),
            Ok(Value::Material(id)) => self.material_id(&id, at),
            Ok(_) => {}
        }
    }

    fn material_id(&mut self, id: &Id, at: String) {
        if self
            .library
            .materials
            .get(&MaterialId::from_id(id.clone()))
            .is_none()
        {
            self.push(at, ProblemKind::UnknownMaterial { id: id.clone() });
        }
    }

    /// Checks that a key names an item of the wanted sort in a sketch.
    fn item(&mut self, items: &Items<'_>, key: &Key, wanted: Sort, at: String) -> bool {
        match items.sorts.get(key.as_str()) {
            None => {
                self.push(
                    at,
                    ProblemKind::Missing {
                        sort: wanted,
                        key: key.as_str().to_owned(),
                    },
                );
                false
            }
            Some(item) if item.is(wanted) => true,
            Some(item) => {
                self.push(
                    at,
                    ProblemKind::WrongSort {
                        key: key.as_str().to_owned(),
                        wanted,
                        found: item.sort(),
                    },
                );
                false
            }
        }
    }

    /// Checks a sketch coordinate or radius: a length in its one stored spelling.
    fn length_text(&mut self, text: &str, at: String) {
        let good = Value::from_stored(text)
            .is_some_and(|v| matches!(v, Value::Length(_)) && v.stored() == text);
        if !good {
            self.push(
                at,
                ProblemKind::NotALength {
                    text: text.to_owned(),
                },
            );
        }
    }

    fn sketch(&mut self, sketch: &'r Sketch, at: &str) {
        if !self.recipe.planes.iter().any(|p| p.key == sketch.plane) {
            self.push(
                format!("{at}/plane"),
                ProblemKind::Missing {
                    sort: Sort::Plane,
                    key: sketch.plane.as_str().to_owned(),
                },
            );
        }
        let mut seen = BTreeSet::new();
        for (n, p) in sketch.points.iter().enumerate() {
            self.unique(&mut seen, &p.key, format!("{at}/points/{n}/key"));
            self.length_text(&p.x, format!("{at}/points/{n}/x"));
            self.length_text(&p.y, format!("{at}/points/{n}/y"));
        }
        let items = Items::of(sketch);
        for (n, l) in sketch.lines.iter().enumerate() {
            let here = format!("{at}/lines/{n}");
            self.unique(&mut seen, &l.key, format!("{here}/key"));
            self.item(&items, &l.start, Sort::Point, format!("{here}/start"));
            self.item(&items, &l.end, Sort::Point, format!("{here}/end"));
        }
        for (n, a) in sketch.arcs.iter().enumerate() {
            let here = format!("{at}/arcs/{n}");
            self.unique(&mut seen, &a.key, format!("{here}/key"));
            self.item(&items, &a.centre, Sort::Point, format!("{here}/centre"));
            self.item(&items, &a.start, Sort::Point, format!("{here}/start"));
            self.item(&items, &a.end, Sort::Point, format!("{here}/end"));
            self.length_text(&a.radius, format!("{here}/radius"));
        }
        for (n, c) in sketch.circles.iter().enumerate() {
            let here = format!("{at}/circles/{n}");
            self.unique(&mut seen, &c.key, format!("{here}/key"));
            self.item(&items, &c.centre, Sort::Point, format!("{here}/centre"));
            self.length_text(&c.radius, format!("{here}/radius"));
        }
        for (n, c) in sketch.constraints.iter().enumerate() {
            self.constraint(&items, c, &format!("{at}/constraints/{n}"));
        }
    }

    fn constraint(&mut self, items: &Items<'_>, c: &Constraint, at: &str) {
        let mut refs: Vec<(&str, &Key, Sort)> = Vec::new();
        let mut length: Option<&String> = None;
        match c {
            Constraint::Coincident { a, b } => {
                refs.extend([("a", a, Sort::Point), ("b", b, Sort::Point)]);
            }
            Constraint::Horizontal { line } | Constraint::Vertical { line } => {
                refs.push(("line", line, Sort::Line));
            }
            Constraint::Parallel { a, b }
            | Constraint::Perpendicular { a, b }
            | Constraint::EqualLength { a, b } => {
                refs.extend([("a", a, Sort::Line), ("b", b, Sort::Line)]);
            }
            Constraint::Distance { a, b, length: l } => {
                refs.extend([("a", a, Sort::Point), ("b", b, Sort::Point)]);
                length = Some(l);
            }
            Constraint::PointLineDistance {
                point,
                line,
                length: l,
                ..
            } => {
                refs.extend([("point", point, Sort::Point), ("line", line, Sort::Line)]);
                length = Some(l);
            }
            Constraint::Fixed { point } => refs.push(("point", point, Sort::Point)),
            Constraint::PointOnLine { point, line } => {
                refs.extend([("point", point, Sort::Point), ("line", line, Sort::Line)]);
            }
            Constraint::PointOnCurve { point, curve } => {
                refs.extend([("point", point, Sort::Point), ("curve", curve, Sort::Curve)]);
            }
            Constraint::Concentric { a, b }
            | Constraint::EqualRadius { a, b }
            | Constraint::TangentCurves { a, b, .. } => {
                refs.extend([("a", a, Sort::Curve), ("b", b, Sort::Curve)]);
            }
            Constraint::Radius { curve, length: l } | Constraint::Diameter { curve, length: l } => {
                refs.push(("curve", curve, Sort::Curve));
                length = Some(l);
            }
            Constraint::TangentLine { line, curve, .. } => {
                refs.extend([("line", line, Sort::Line), ("curve", curve, Sort::Curve)]);
            }
        }
        for (member, key, sort) in refs {
            self.item(items, key, sort, format!("{at}/{member}"));
        }
        if let Some(text) = length {
            self.length_formula(text, format!("{at}/length"));
        }
    }

    /// The sketch a key names, checked to lie on a plane of the base wanted (any base for
    /// `None`); `None` if it is missing or on the wrong plane (each recorded at `at`).
    fn sketch_on(&mut self, key: &Key, wanted: Option<Base>, at: &str) -> Option<&'r Sketch> {
        let recipe = self.recipe;
        let Some(sketch) = recipe.sketches.iter().find(|s| s.key == *key) else {
            self.push(
                at.to_owned(),
                ProblemKind::Missing {
                    sort: Sort::Sketch,
                    key: key.as_str().to_owned(),
                },
            );
            return None;
        };
        if let Some(wanted) = wanted {
            let base = recipe
                .planes
                .iter()
                .find(|p| p.key == sketch.plane)
                .map(|p| p.base);
            if base.is_some_and(|base| base != wanted) {
                self.push(at.to_owned(), ProblemKind::WrongPlane { wanted });
                return None;
            }
        }
        Some(sketch)
    }

    /// Checks that `edges` are lines of the sketch making one closed loop.
    fn profile(&mut self, sketch: &'r Sketch, edges: &[Key], at: &str) {
        let items = Items::of(sketch);
        let mut all_lines = true;
        for (n, key) in edges.iter().enumerate() {
            all_lines &= self.item(&items, key, Sort::Line, format!("{at}/{n}"));
        }
        if all_lines && !closed(&items, edges) {
            self.push(at.to_owned(), ProblemKind::NotALoop);
        }
    }

    fn subcategory(&mut self, id: CategoryId, at: String) {
        match self.library.categories.get(id) {
            None => self.push(at, ProblemKind::UnknownCategory { id }),
            Some(category) => {
                let family = self.recipe.category;
                if self.category_known && id != family && category.parent != Some(family) {
                    self.push(at, ProblemKind::NotOwnSubcategory { id });
                }
            }
        }
    }

    fn steps(&mut self) {
        let recipe = self.recipe;
        let all: BTreeSet<&str> = recipe.steps.iter().map(|s| s.key.as_str()).collect();
        let mut earlier = BTreeSet::new();
        for (n, step) in recipe.steps.iter().enumerate() {
            let at = format!("/steps/{n}");
            let before = earlier.clone();
            self.unique(&mut earlier, &step.key, format!("{at}/key"));
            match &step.operation {
                Operation::Extrude {
                    sketch,
                    profile,
                    holes,
                    start,
                    end,
                } => {
                    if let Some(found) = self.sketch_on(sketch, None, &format!("{at}/sketch")) {
                        self.profile(found, profile, &format!("{at}/profile"));
                        for (h, hole) in holes.iter().enumerate() {
                            self.profile(found, hole, &format!("{at}/holes/{h}"));
                        }
                    }
                    self.length_formula(start, format!("{at}/start"));
                    self.length_formula(end, format!("{at}/end"));
                }
                Operation::Subtract { target, tool } => {
                    self.input(&before, &all, target, format!("{at}/target"));
                    self.input(&before, &all, tool, format!("{at}/tool"));
                }
                Operation::Unite { a, b } => {
                    self.input(&before, &all, a, format!("{at}/a"));
                    self.input(&before, &all, b, format!("{at}/b"));
                }
            }
            self.subcategory(step.subcategory, format!("{at}/subcategory"));
            self.material(&step.material, &format!("{at}/material"));
        }
    }

    /// Checks a step's input: an earlier step.
    fn input(&mut self, before: &BTreeSet<&str>, all: &BTreeSet<&str>, key: &Key, at: String) {
        let key_text = key.as_str().to_owned();
        if before.contains(key.as_str()) {
            return;
        }
        if all.contains(key.as_str()) {
            self.push(at, ProblemKind::NotEarlier { key: key_text });
        } else {
            self.push(
                at,
                ProblemKind::Missing {
                    sort: Sort::Step,
                    key: key_text,
                },
            );
        }
    }

    fn material(&mut self, material: &MaterialRef, at: &str) {
        match material {
            MaterialRef::Fixed(id) => self.material_id(id, format!("{at}/fixed")),
            MaterialRef::Parameter(key) => {
                let at = format!("{at}/parameter");
                match self.params.get(key.as_str()) {
                    None => self.push(
                        at,
                        ProblemKind::Missing {
                            sort: Sort::Parameter,
                            key: key.as_str().to_owned(),
                        },
                    ),
                    Some(Param {
                        definition: Some(def),
                        ..
                    }) if def.kind() != Kind::Material => self.push(
                        at,
                        ProblemKind::NotAMaterial {
                            key: key.as_str().to_owned(),
                        },
                    ),
                    Some(_) => {}
                }
            }
        }
    }

    fn symbols(&mut self) {
        let recipe = self.recipe;
        let mut seen = BTreeSet::new();
        for (n, symbol) in recipe.symbols.iter().enumerate() {
            let at = format!("/symbols/{n}");
            self.unique(&mut seen, &symbol.key, format!("{at}/key"));
            let wanted = match symbol.view {
                View::Plan => Base::Xy,
                View::Elevation => Base::Xz,
            };
            if let Some(sketch) =
                self.sketch_on(&symbol.sketch, Some(wanted), &format!("{at}/sketch"))
            {
                let items = Items::of(sketch);
                for (e, edge) in symbol.edges.iter().enumerate() {
                    self.item(&items, edge, Sort::Edge, format!("{at}/edges/{e}"));
                }
            }
            self.subcategory(symbol.subcategory, format!("{at}/subcategory"));
            if let Some(when) = &symbol.when {
                let here = format!("{at}/when");
                match self.params.get(when.parameter.as_str()) {
                    None => self.push(
                        format!("{here}/parameter"),
                        ProblemKind::Missing {
                            sort: Sort::Parameter,
                            key: when.parameter.as_str().to_owned(),
                        },
                    ),
                    Some(Param {
                        definition: Some(def),
                        ..
                    }) => {
                        let def = def.clone();
                        if matches!(def.kind(), Kind::YesNo | Kind::Choice) {
                            self.value(&def, &when.is, format!("{here}/is"));
                        } else {
                            self.push(
                                format!("{here}/parameter"),
                                ProblemKind::NotACondition {
                                    key: when.parameter.as_str().to_owned(),
                                },
                            );
                        }
                    }
                    Some(_) => {}
                }
            }
        }
    }

    /// The built-in types, then the type table: one set of keys and one set of names for both.
    fn types(&mut self) {
        let recipe = self.recipe;
        let mut keys = BTreeSet::new();
        let mut names = BTreeSet::new();
        for (n, ty) in recipe.types.iter().enumerate() {
            let at = format!("/types/{n}");
            self.unique(&mut keys, &ty.key, format!("{at}/key"));
            self.type_name(&mut names, &ty.name, format!("{at}/name"));
            let mut set = BTreeSet::new();
            for (v, value) in ty.values.iter().enumerate() {
                let here = format!("{at}/values/{v}");
                if !set.insert(value.parameter.as_str()) {
                    self.push(
                        format!("{here}/parameter"),
                        ProblemKind::Duplicate {
                            key: value.parameter.as_str().to_owned(),
                        },
                    );
                    continue;
                }
                if let Some(def) = self.settable(&value.parameter, format!("{here}/parameter")) {
                    self.value(&def, &value.value, format!("{here}/value"));
                }
            }
        }
        let Some(table) = &recipe.type_table else {
            return;
        };
        let mut set = BTreeSet::new();
        let mut columns = Vec::with_capacity(table.columns.len());
        for (c, column) in table.columns.iter().enumerate() {
            let at = format!("/type_table/columns/{c}");
            if set.insert(column.as_str()) {
                columns.push(self.settable(column, at));
            } else {
                self.push(
                    at,
                    ProblemKind::Duplicate {
                        key: column.as_str().to_owned(),
                    },
                );
                columns.push(None);
            }
        }
        for (r, row) in table.rows.iter().enumerate() {
            let at = format!("/type_table/rows/{r}");
            self.unique(&mut keys, &row.key, format!("{at}/key"));
            self.type_name(&mut names, &row.name, format!("{at}/name"));
            if row.values.len() != columns.len() {
                self.push(
                    format!("{at}/values"),
                    ProblemKind::ValueCount {
                        values: row.values.len(),
                        columns: columns.len(),
                    },
                );
            }
            for (v, (value, def)) in row.values.iter().zip(&columns).enumerate() {
                if let Some(def) = def {
                    self.value(def, value, format!("{at}/values/{v}"));
                }
            }
        }
    }

    /// Records an empty name, or one another type already has (case and end spaces aside).
    fn type_name(&mut self, names: &mut BTreeSet<String>, name: &str, at: String) {
        let name = name.trim();
        if name.is_empty() {
            self.push(at, ProblemKind::EmptyName);
        } else if !names.insert(name.to_lowercase()) {
            self.push(
                at,
                ProblemKind::DuplicateName {
                    name: name.to_owned(),
                },
            );
        }
    }

    /// The definition of a parameter a type may set (it exists, the type holds it, it is not
    /// worked out), or `None` with the problem recorded at `at`.
    fn settable(&mut self, key: &Key, at: String) -> Option<Definition> {
        let key = key.as_str();
        let Some(param) = self.params.get(key) else {
            self.push(
                at,
                ProblemKind::Missing {
                    sort: Sort::Parameter,
                    key: key.to_owned(),
                },
            );
            return None;
        };
        let formula = matches!(param.row.value, ParameterValue::Formula(_));
        let def = param.definition.clone()?;
        if def.holder() == Holder::Instance {
            self.push(
                at,
                ProblemKind::HeldByInstance {
                    key: key.to_owned(),
                },
            );
            None
        } else if formula {
            self.push(
                at,
                ProblemKind::SetsFormula {
                    key: key.to_owned(),
                },
            );
            None
        } else {
            Some(def)
        }
    }
}

/// Whether the lines make one closed loop: three or more, each sharing a point with the next,
/// the last with the first.
fn closed(items: &Items<'_>, edges: &[Key]) -> bool {
    let ends: Option<Vec<(&str, &str)>> = edges
        .iter()
        .map(|key| items.lines.get(key.as_str()).copied())
        .collect();
    let Some(ends) = ends else {
        return false;
    };
    let Some(((first_start, first_end), rest)) = ends.split_first() else {
        return false;
    };
    if rest.len() < 2 {
        return false;
    }
    let mut at = *first_end;
    for (start, end) in rest {
        at = if *start == at {
            end
        } else if *end == at {
            start
        } else {
            return false;
        };
    }
    at == *first_start
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> Key {
        Key::parse(text).unwrap()
    }

    #[test]
    fn values_become_formula_text_that_reads_back() {
        let def = Definition::new(
            Id::parse("P-1").unwrap(),
            "Width",
            Kind::Length,
            Holder::Type,
        )
        .unwrap();
        let value = Value::from_stored("length:q0:5248000").unwrap();
        let text = formula_text(&def, &value).unwrap();
        let back = value_of(&text, Kind::Length, &mut |_| {
            Err(FormulaError {
                code: "UnknownName",
                message: String::new(),
            })
        })
        .unwrap();
        assert_eq!(back, value);
        let angle = Definition::new(
            Id::parse("P-2").unwrap(),
            "Swing",
            Kind::Angle,
            Holder::Type,
        )
        .unwrap();
        let value = Value::from_stored("angle:405000000").unwrap();
        let text = formula_text(&angle, &value).unwrap();
        let back = value_of(&text, Kind::Angle, &mut |_| {
            Err(FormulaError {
                code: "UnknownName",
                message: String::new(),
            })
        })
        .unwrap();
        assert_eq!(back, value);
    }

    #[test]
    fn a_loop_needs_three_lines_end_to_end() {
        let sketch = Sketch {
            key: key("s"),
            plane: key("p"),
            points: Vec::new(),
            lines: [
                ("a", "p1", "p2"),
                ("b", "p3", "p2"),
                ("c", "p3", "p1"),
                ("d", "p1", "p2"),
            ]
            .iter()
            .map(|(k, s, e)| crate::model::SketchLine {
                key: key(k),
                start: key(s),
                end: key(e),
            })
            .collect(),
            arcs: Vec::new(),
            circles: Vec::new(),
            constraints: Vec::new(),
        };
        let items = Items::of(&sketch);
        assert!(closed(&items, &[key("a"), key("b"), key("c")]));
        assert!(!closed(&items, &[key("a"), key("d")]));
        assert!(!closed(&items, &[key("a"), key("c"), key("b")]));
        assert!(!closed(&items, &[key("a"), key("b")]));
        assert!(!closed(&items, &[]));
    }
}
