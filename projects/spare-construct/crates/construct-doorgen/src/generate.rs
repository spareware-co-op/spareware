//! A family generated from specs: the template's recipe, with the family's own id and name, the
//! tag parameters added, and one type table row per spec (note
//! CON-A7-F4-P1-door-window-generator).
//!
//! What the generator writes, and all it writes: the family's id and name; five tag parameters
//! (type mark, clear width, clear height, leaf area, glass area: the last two only where the
//! template has a leaf or glass), each held by the type with a stored value; and a type table
//! whose columns are the type mark, the width and height, every part and material the template
//! has, and the tags, one row per spec. The template's own table of sizes is left out: every type
//! of a generated family is one a spec asked for. Sketches, steps, symbols and hosting are the
//! template's, untouched: the generator writes no geometry, and every solid, opening and symbol
//! comes from the template through construct-families' evaluator.
//!
//! The tags' values are measured from what the evaluator built (crate `checks`), then the whole
//! recipe is checked again from its written rows ([`verify`]), so a stored tag is always what its
//! type's solids measure.

use crate::checks::{Judged, Quantities, judge_built};
use crate::refusal::{Refusal, Rule};
use crate::rules::{Resolved, judge};
use crate::spec::{Hand, Spec};
use crate::template::Template;
use construct_families::{
    DefinitionRow, Key, Library, Parameter, ParameterValue, Recipe, SolvedOpening, TableRow,
    TypeTable, TypeValues,
};
use construct_params::{Holder, Kind, Length, Value};
use kit_core::Id;

/// The family a set of specs is generated into: its permanent id, its name and its template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Family {
    /// The family's permanent id (it never changes, so placed units keep their link).
    pub id: Id,
    /// The name people see.
    pub name: String,
    /// The template every spec fills.
    pub template: Template,
}

/// A generated family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    /// The recipe: the template with the family's id and name, the tags and one row per spec.
    pub recipe: Recipe,
    /// Each spec's type, in the order the specs were given.
    pub types: Vec<GeneratedType>,
}

/// One spec's type in a generated family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedType {
    /// The type's key in the recipe (from its mark).
    pub key: Key,
    /// Its type mark.
    pub mark: String,
    /// The values a placed unit of this type starts with: the row's, with the spec's hand (a
    /// door's `swing`) and sill height as the instance's.
    pub values: TypeValues,
    /// What its solids measure.
    pub quantities: Quantities,
    /// The opening its wall is cut by, in the family's frame.
    pub opening: SolvedOpening,
}

/// A tag parameter: its key, people's name, kind and default.
struct Tag {
    key: &'static str,
    name: &'static str,
    kind: Kind,
    default: &'static str,
}

const TYPE_MARK: Tag = Tag {
    key: "type_mark",
    name: "Type mark",
    kind: Kind::Text,
    default: "text:",
};
const CLEAR_WIDTH: Tag = Tag {
    key: "clear_width",
    name: "Clear width",
    kind: Kind::Length,
    default: "length:q0:0",
};
const CLEAR_HEIGHT: Tag = Tag {
    key: "clear_height",
    name: "Clear height",
    kind: Kind::Length,
    default: "length:q0:0",
};
const LEAF_AREA: Tag = Tag {
    key: "leaf_area",
    name: "Leaf area",
    kind: Kind::Area,
    default: "area:q0:0",
};
const GLASS_AREA: Tag = Tag {
    key: "glass_area",
    name: "Glass area",
    kind: Kind::Area,
    default: "area:q0:0",
};

/// The permanent id of a tag parameter: one for every generated door and window, so a schedule
/// reads one column across them.
const TAG_ID_PREFIX: &str = "library.doors-windows.";

/// The tags a template's generated types carry, in column order.
fn tags(template: Template) -> Vec<&'static Tag> {
    let mut all = vec![&TYPE_MARK, &CLEAR_WIDTH, &CLEAR_HEIGHT];
    if template.has_leaf() {
        all.push(&LEAF_AREA);
    }
    if template.glazed() {
        all.push(&GLASS_AREA);
    }
    all
}

/// The key of a tag parameter (every tag key is a key).
fn key(text: &str) -> Result<Key, Refusal> {
    Key::parse(text).ok_or_else(|| Refusal::new(Rule::Template, text, "not a key"))
}

/// A type mark's key: lowercase, with spaces and `-` as `_`.
///
/// # Errors
/// [`Rule::Mark`] if the mark is empty, longer than 64 characters, does not start with a letter,
/// or holds anything but ASCII letters, digits, spaces, `-` and `_`.
pub fn mark_key(mark: &str) -> Result<Key, Refusal> {
    let refuse = |why: &str| Err(Refusal::new(Rule::Mark, "mark", why.to_owned()).of(mark));
    if !mark.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return refuse("a type mark starts with a letter");
    }
    if !mark
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
    {
        return refuse("a type mark holds only letters, digits, spaces, - and _");
    }
    let text: String = mark
        .chars()
        .map(|c| match c {
            ' ' | '-' => '_',
            c => c.to_ascii_lowercase(),
        })
        .collect();
    match Key::parse(&text) {
        Some(key) => Ok(key),
        None => refuse("a type mark is at most 64 characters"),
    }
}

/// The template's recipe with the family's id and name and the tag parameters, no types.
fn base(family: &Family) -> Result<Recipe, Refusal> {
    let mut recipe = family.template.recipe()?;
    recipe.id = family.id.clone();
    recipe.name.clone_from(&family.name);
    recipe.types.clear();
    recipe.type_table = None;
    for tag in tags(family.template) {
        if recipe.parameters.iter().any(|p| p.key.as_str() == tag.key) {
            return Err(Refusal::new(
                Rule::Template,
                tag.key,
                "the template already has a parameter by this tag's key",
            ));
        }
        let id = Id::parse(&format!("{TAG_ID_PREFIX}{}", tag.key))
            .map_err(|_| Refusal::new(Rule::Template, tag.key, "not an id"))?;
        recipe.parameters.push(Parameter {
            key: key(tag.key)?,
            definition: DefinitionRow {
                id,
                name: tag.name.to_owned(),
                kind: tag.kind,
                holder: Holder::Type,
                choices: Vec::new(),
            },
            value: ParameterValue::Default(tag.default.to_owned()),
        });
    }
    Ok(recipe)
}

/// The type table's columns for a template: the type mark, width, height, every part, every
/// material parameter the type holds, then the measured tags.
fn columns(recipe: &Recipe, template: Template) -> Result<Vec<Key>, Refusal> {
    let mut columns = vec![key(TYPE_MARK.key)?, key("width")?, key("height")?];
    for part in template.parts() {
        columns.push(key(part.key())?);
    }
    for p in &recipe.parameters {
        if p.definition.kind == Kind::Material && p.definition.holder == Holder::Type {
            columns.push(p.key.clone());
        }
    }
    for tag in tags(template).into_iter().skip(1) {
        columns.push(key(tag.key)?);
    }
    Ok(columns)
}

/// A parameter's default text in `recipe`.
fn default_text(recipe: &Recipe, parameter: &Key) -> Option<String> {
    recipe
        .parameters
        .iter()
        .find(|p| p.key == *parameter)
        .and_then(|p| match &p.value {
            ParameterValue::Default(text) => Some(text.clone()),
            ParameterValue::Formula(_) => None,
        })
}

/// The values a type sets before it is measured: width, height, parts and materials.
fn type_values(r: &Resolved) -> Result<TypeValues, Refusal> {
    let mut values = TypeValues::new()
        .with(key("width")?, Value::Length(r.width).stored())
        .with(key("height")?, Value::Length(r.height).stored());
    for (part, length) in &r.parts {
        values.set(key(part.key())?, Value::Length(*length).stored());
    }
    for (parameter, id) in &r.materials {
        values.set(key(parameter)?, Value::Material(id.clone()).stored());
    }
    Ok(values)
}

/// The instance values a placed unit starts with: a door's swing, a window's sill height.
fn with_instance(
    mut values: TypeValues,
    template: Template,
    r: &Resolved,
) -> Result<TypeValues, Refusal> {
    if template == Template::SingleSwingDoor
        && let Some(hand) = r.hand
    {
        let choice = match hand {
            Hand::Left => "choice:left",
            Hand::Right => "choice:right",
        };
        values.set_instance(key("swing")?, choice);
    }
    if let Some(sill) = r.sill {
        values.set_instance(key("sill_height")?, Value::Length(sill).stored());
    }
    Ok(values)
}

/// The text of a tag's measured value.
fn measured(q: &Quantities, tag: &str) -> Option<String> {
    match tag {
        "clear_width" => Some(Value::Length(q.clear_width).stored()),
        "clear_height" => Some(Value::Length(q.clear_height).stored()),
        "leaf_area" => q.leaf_area.map(|a| Value::Area(a).stored()),
        "glass_area" => q.glass_area.map(|a| Value::Area(a).stored()),
        _ => None,
    }
}

/// A row's name: the mark and the overall size (`D01: 900 mm x 2100 mm`).
fn row_name(mark: &str, width: Length, height: Length) -> String {
    format!("{mark}: {width} x {height}")
}

/// One spec's type: judged, built, measured, and its row written.
fn one_type(
    spec: &Spec,
    row_key: Key,
    base: &Recipe,
    columns: &[Key],
) -> Result<(TableRow, GeneratedType), Refusal> {
    let template = spec.template;
    let resolved = judge(spec, base).map_err(|r| r.of(&spec.mark))?;
    let values = with_instance(type_values(&resolved)?, template, &resolved)?;
    let judged = judge_built(template, base, &values).map_err(|r| r.of(&spec.mark))?;
    if let Some((w, h)) = resolved.clear
        && (judged.quantities.clear_width != w || judged.quantities.clear_height != h)
    {
        return Err(Refusal::new(
            Rule::Quantity,
            "clear",
            format!(
                "the clear opening asked for is {w} x {h}; the solids measure {} x {}",
                judged.quantities.clear_width, judged.quantities.clear_height
            ),
        )
        .of(&spec.mark));
    }
    let mut row = Vec::with_capacity(columns.len());
    for column in columns {
        let text = match column.as_str() {
            "type_mark" => Value::Text(spec.mark.clone()).stored(),
            other => match values.get(other) {
                Some(text) => text.to_owned(),
                None => measured(&judged.quantities, other)
                    .or_else(|| default_text(base, column))
                    .ok_or_else(|| {
                        Refusal::new(Rule::Template, other, "no value for this column")
                    })?,
            },
        };
        row.push(text);
    }
    let row = TableRow {
        key: row_key.clone(),
        name: row_name(&spec.mark, resolved.width, resolved.height),
        values: row,
    };
    let ty = GeneratedType {
        key: row_key,
        mark: spec.mark.clone(),
        values,
        quantities: judged.quantities,
        opening: judged.opening,
    };
    Ok((row, ty))
}

/// Generates `family` from `specs`, judged against the template's rules, built, measured, and
/// checked again from the written recipe, against the project's categories and materials.
///
/// # Errors
/// The first [`Refusal`]: [`Rule::NoSpecs`] or [`Rule::OtherTemplate`] for the set, then each
/// spec in order ([`Rule::Mark`], [`Rule::DuplicateMark`], the template's rules, then what its
/// built solids, opening and symbols show), then the recipe's own checks ([`Rule::Checker`],
/// [`Rule::RoundTrip`], and [`verify`]'s).
pub fn generate(
    family: &Family,
    specs: &[Spec],
    library: &Library<'_>,
) -> Result<Generated, Refusal> {
    if specs.is_empty() {
        return Err(Refusal::new(
            Rule::NoSpecs,
            "specs",
            "a family is generated from one spec or more",
        ));
    }
    let template = family.template;
    let base = base(family)?;
    let columns = columns(&base, template)?;
    let mut rows: Vec<TableRow> = Vec::with_capacity(specs.len());
    let mut types: Vec<GeneratedType> = Vec::with_capacity(specs.len());
    for spec in specs {
        if spec.template != template {
            return Err(Refusal::new(
                Rule::OtherTemplate,
                "template",
                format!(
                    "the spec names the {} template and the family is made from the {}",
                    spec.template.stem(),
                    template.stem()
                ),
            )
            .of(&spec.mark));
        }
        let row_key = mark_key(&spec.mark)?;
        if let Some(other) = types.iter().find(|t| t.key == row_key) {
            return Err(Refusal::new(
                Rule::DuplicateMark,
                "mark",
                format!(
                    "the marks {} and {} make one type key",
                    other.mark, spec.mark
                ),
            )
            .of(&spec.mark));
        }
        let (row, ty) = one_type(spec, row_key, &base, &columns)?;
        rows.push(row);
        types.push(ty);
    }
    let mut recipe = base;
    recipe.type_table = Some(TypeTable { columns, rows });
    recipe.check(library).map_err(|problems| {
        let said: Vec<String> = problems
            .iter()
            .map(|p| format!("{} at {}", p.kind(), p.at()))
            .collect();
        Refusal::new(Rule::Checker, "recipe", said.join("; "))
    })?;
    match Recipe::from_json(&recipe.to_json()) {
        Ok(read) if read == recipe => {}
        _ => {
            return Err(Refusal::new(
                Rule::RoundTrip,
                "recipe",
                "the recipe does not read back from its written form unchanged",
            ));
        }
    }
    let checked = verify(&recipe, template)?;
    // Each type's values as its row stores them, with the instance's.
    for (ty, from_row) in types.iter_mut().zip(checked) {
        let mut values = from_row.0;
        for (parameter, text) in ty.values.instance_values() {
            values.set_instance(parameter.clone(), text);
        }
        ty.values = values;
    }
    Ok(Generated { recipe, types })
}

/// Checks every type a generated recipe names, from its stored rows: each builds, cuts a clean
/// opening, keeps its symbols inside it, and holds tags equal to what its solids measure, its
/// width and height the opening's (crate `checks`). Gives each type's values and quantities, in
/// row order. A later edit to a type is checked again with this.
///
/// # Errors
/// The first [`Refusal`], naming the type's mark: a type that does not build or whose opening or
/// symbols are not clean as [`generate()`] refuses it, and [`Rule::Tag`] for a tag that is not what
/// its solids measure.
pub fn verify(
    recipe: &Recipe,
    template: Template,
) -> Result<Vec<(TypeValues, Quantities)>, Refusal> {
    let mut out = Vec::new();
    for ty in recipe.named_types() {
        let values = TypeValues::of(recipe, ty.key())
            .map_err(|e| Refusal::new(Rule::Evaluation, ty.key().as_str(), e.to_string()))?;
        let mark = match values.get(TYPE_MARK.key).and_then(Value::from_stored) {
            Some(Value::Text(mark)) if !mark.is_empty() => mark,
            _ => {
                return Err(Refusal::new(
                    Rule::Tag,
                    TYPE_MARK.key,
                    format!("the type {} has no type mark", ty.key()),
                ));
            }
        };
        let Judged { quantities, .. } =
            judge_built(template, recipe, &values).map_err(|r| r.of(&mark))?;
        let mut want: Vec<(&str, Option<String>)> = vec![
            ("width", Some(Value::Length(quantities.width).stored())),
            ("height", Some(Value::Length(quantities.height).stored())),
        ];
        for tag in tags(template).into_iter().skip(1) {
            want.push((tag.key, measured(&quantities, tag.key)));
        }
        for (tag, measured) in want {
            let stored = values.get(tag);
            if stored.map(str::to_owned) != measured {
                return Err(Refusal::new(
                    Rule::Tag,
                    tag,
                    format!(
                        "the type holds {} and its solids measure {}",
                        stored.unwrap_or("nothing"),
                        measured.as_deref().unwrap_or("nothing")
                    ),
                )
                .of(&mark));
            }
        }
        out.push((values, quantities));
    }
    Ok(out)
}
