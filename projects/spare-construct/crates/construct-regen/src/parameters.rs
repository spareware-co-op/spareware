//! How categories, parameters and types sit in a document's element records, and how a shape reads
//! parameter values. The server's commands (construct-api) and the regenerator both read through
//! this one module, so what a command checks and what a shape is built from cannot differ.
//!
//! What it promises:
//! - Every record a command writes carries its category under [`CATEGORY_KEY`]: a type record its
//!   own (construct-types writes it), any other element the category of its kind
//!   ([`category_of`], construct-categories' table). An instance names its type under
//!   [`TYPE_KEY`]. Both are construct-types' keys, so the same key means the same on every record.
//! - A document's parameters (construct-params' [`ProjectParams`]: definitions, bindings to
//!   categories, defaults) are one record of kind [`TABLE_KIND`] under the element ID
//!   [`TABLE_ID`], the table's own stored bytes under [`TABLE_KEY`]. Reading it is strict.
//! - A recipe argument written `{"param":"<id>"}` is filled, before the shape is built, with the
//!   value the element reads for that parameter: through construct-types' [`resolve`] (its own
//!   value, then its type's, then the default) when it has a type, else its own value, then the
//!   default. A length goes in as the whole millimetres the stand-in's recipes take.
//! - A recipe argument written `{"wall":"type"}` is filled in the same read with the layers of
//!   the element's wall type ([`crate::walls`]), so [`recipe_at`] is the one place a recipe that
//!   reads records is filled.
//! - Records of kind `type`, [`TABLE_KIND`], `level`, `view` and `schedule` have no shape ([`has_shape`]).
//! - A parameter whose definition carries a formula is worked out whenever it is read, never
//!   stored: [`Context::worked`] for an element, [`Context::type_worked`] for a type, with
//!   construct-formulas' `work_out` over the values of the holder's other parameters, in the order
//!   they depend on each other. [`Context::value_of`] and [`Context::view`] answer with it, so a
//!   shape, the properties panel and a command read the same value; a formula that can't be
//!   worked out is its reason, and a shape that reads it is the build failure `BadParameter`.
//!   A formula's inputs are the element's own record, its type's and the parameter table, which
//!   are what the regenerator's `reads` table already rebuilds an element on.
//!
//! Depends on construct-model (records), construct-categories, construct-params,
//! construct-formulas and construct-types. Interim: [`recipe_number`] turns a length into the stand-in's whole-millimetre
//! recipe number; it changes with the kernel's recipes (piece A39 makes lengths the kernel's grid
//! unit, job 1.1 the kernel seam), and nothing else here knows the stand-in's units.
//!
//! Not here yet, with their places: the document's own subcategories (the categories command, a
//! later job, feeds [`Context::categories`]); display settings per project (typed text is read
//! with construct-params' metric [`Units`] default, [`Context::units`]).

use construct_categories::{CategoryError, CategoryId, CategoryTable, category_of_kind};
use construct_formulas::{FormulaError, work_out};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Definition, Holder, ProjectParams, Units, Value, Values};
use construct_types::{ElementType, Instance, Parameters, Source, Types, resolve};
use kit_core::Id;
use kit_store::{Place, Reader, StoreError, read_row};
use serde::Serialize;
use std::collections::BTreeMap;

pub use construct_types::{CATEGORY_KEY, TYPE_KEY, TYPE_KIND};

/// The element ID of a document's parameter table record.
pub const TABLE_ID: &str = "parameters";
/// The kind of a document's parameter table record.
pub const TABLE_KIND: &str = "parameters";
/// The `props` key that holds the table's stored bytes (construct-params' FORMAT.md).
pub const TABLE_KEY: &str = "@parameters";
/// The name a parameter table record is given.
const TABLE_NAME: &str = "Parameters";
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";
/// The argument form that names a parameter: `{"param":"<id>"}`.
const PARAM_ARG: &str = "param";

/// The kind name construct-categories knows an element by: its recipe's operation, with the
/// owner in front when the owner is not Spare Construct itself (`workshop.part`).
#[must_use]
pub fn kind_name(recipe: &Recipe) -> String {
    if recipe.owner == OWN {
        recipe.op.clone()
    } else {
        format!("{}.{}", recipe.owner, recipe.op)
    }
}

/// The category of an element of this recipe, by its kind name.
///
/// # Errors
/// construct-categories' [`CategoryError::UnknownKind`] for a kind no category holds.
pub fn category_of(recipe: &Recipe) -> Result<CategoryId, CategoryError> {
    category_of_kind(&kind_name(recipe))
}

/// Whether a record is an element with a shape: type records, parameter tables, levels, views,
/// sheets and viewports
/// are not.
#[must_use]
pub fn has_shape(element: &Element) -> bool {
    element.kind != TYPE_KIND
        && element.kind != TABLE_KIND
        && element.kind != construct_levels::LEVEL_KIND
        && element.kind != construct_views::VIEW_KIND
        && element.kind != construct_views::TEMPLATE_KIND
        && element.kind != construct_schedules::SCHEDULE_KIND
        && element.kind != construct_sheets::SHEET_KIND
        && element.kind != construct_sheets::VIEWPORT_KIND
}

/// A category ID in its one spelling (construct-types' FORMAT.md).
#[must_use]
pub fn write_category(id: CategoryId) -> String {
    id.0.to_string()
}

/// The category a record says it is in, if it says so in the one spelling.
#[must_use]
pub fn stored_category(element: &Element) -> Option<CategoryId> {
    let text = element.props.get(CATEGORY_KEY)?;
    let id = text.parse::<u32>().ok()?;
    (id.to_string() == *text).then_some(CategoryId(id))
}

/// The type an element names, if it names one.
#[must_use]
pub fn type_of(element: &Element) -> Option<Id> {
    element
        .props
        .get(TYPE_KEY)
        .and_then(|text| Id::parse(text).ok())
}

/// A document's parameter table as its record: the table's stored bytes under [`TABLE_KEY`].
///
/// # Errors
/// The table's own error text if it cannot be written, which a valid table never gives.
pub fn table_record(document: &Id, table: &ProjectParams) -> Result<Element, String> {
    let bytes = table.to_bytes().map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut props = BTreeMap::new();
    props.insert(TABLE_KEY.to_owned(), text);
    Ok(Element {
        id: table_id(),
        document: document.clone(),
        kind: TABLE_KIND.to_owned(),
        name: TABLE_NAME.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct(TABLE_KIND, BTreeMap::new()),
        props,
        references: Vec::new(),
    })
}

/// [`TABLE_ID`] as an ID.
#[must_use]
pub fn table_id() -> Id {
    // A constant made of letters only, always an ID.
    Id::parse(TABLE_ID).unwrap_or_else(|_| Id::from_time_and_random(0, [0; 10]))
}

/// Reads a document's parameter table from its record. Anything [`table_record`] would not write
/// is refused.
///
/// # Errors
/// Why, in words: construct-params' own refusal of the bytes, or a record that is not a table.
pub fn read_table(record: &Element, categories: &CategoryTable) -> Result<ProjectParams, String> {
    let Some(text) = record.props.get(TABLE_KEY) else {
        return Err("the parameter table's record holds no table".to_owned());
    };
    let table =
        ProjectParams::from_bytes(text.as_bytes(), categories).map_err(|e| e.to_string())?;
    if table_record(&record.document, &table)? != *record {
        return Err("the parameter table's record is not as it is written".to_owned());
    }
    Ok(table)
}

/// Every parameter bound to a category of `categories`, as construct-types takes them: each
/// definition, with its binding's default. A parameter defined and bound nowhere is on no
/// element, so it is not here.
///
/// # Errors
/// construct-types' refusal, which a table construct-params read never gives.
pub fn parameters(table: &ProjectParams, categories: &CategoryTable) -> Result<Parameters, String> {
    let mut bound = BTreeMap::new();
    for category in categories.all() {
        for applicable in table.parameters_of(categories, category.id) {
            bound
                .entry(applicable.definition.id().clone())
                .or_insert((applicable.definition, applicable.default));
        }
    }
    let mut params = Parameters::new();
    for (definition, default) in bound.into_values() {
        params
            .add(definition.clone(), default.cloned())
            .map_err(|e| e.to_string())?;
    }
    Ok(params)
}

/// What a document's records are read against: its categories, its parameters and how typed text
/// is read.
#[derive(Clone, Debug)]
pub struct Context {
    /// The category table. The built-in tree today; the document's own subcategories come with
    /// the categories command.
    pub categories: CategoryTable,
    /// The document's parameter table.
    pub table: ProjectParams,
    /// The same parameters as construct-types takes them.
    pub params: Parameters,
    /// How typed values are read: construct-params' metric default until a project stores its
    /// own display settings.
    pub units: Units,
}

impl Context {
    /// The context of a document with this table.
    ///
    /// # Errors
    /// As [`parameters`].
    pub fn new(table: ProjectParams) -> Result<Self, String> {
        let categories = CategoryTable::new();
        let params = parameters(&table, &categories)?;
        Ok(Self {
            categories,
            table,
            params,
            units: Units::default(),
        })
    }

    /// An element's own parameter values. For an element with a type, construct-types reads them
    /// and checks them against its type; for one without, construct-params reads every defined
    /// parameter's entry. Other keys are not parameters and are left alone.
    ///
    /// # Errors
    /// The refusal of a stored value, in words.
    pub fn own_values(
        &self,
        element: &Element,
        ty: Option<&ElementType>,
    ) -> Result<Values, String> {
        match ty {
            Some(ty) => Ok(self.instance(element, ty)?.values().clone()),
            None => Values::from_props(self.params.definitions(), &element.props)
                .map_err(|e| e.to_string()),
        }
    }

    /// An element of type `ty`, as construct-types reads it from the record's `props`.
    ///
    /// # Errors
    /// construct-types' refusal, in words.
    pub fn instance(&self, element: &Element, ty: &ElementType) -> Result<Instance, String> {
        let mut types = Types::new();
        types.add(ty.clone()).map_err(|e| e.to_string())?;
        let props = instance_props(&element.props);
        Instance::from_props(&props, &self.params, &types).map_err(|e| e.to_string())
    }

    /// The category an element is in: the one it records, else its kind's.
    #[must_use]
    pub fn category_of_element(element: &Element) -> Option<CategoryId> {
        match stored_category(element) {
            Some(category) => Some(category),
            None if has_shape(element) => category_of(&element.recipe).ok(),
            None => None,
        }
    }

    /// The value an element reads for a parameter as it is stored, before any formula: its own,
    /// then its type's, then the default.
    fn stored_value_of(
        &self,
        element: &Element,
        ty: Option<&ElementType>,
        id: &Id,
    ) -> Result<Option<Value>, String> {
        if let Some(ty) = ty {
            let instance = self.instance(element, ty)?;
            let resolved = resolve(&self.params, ty, &instance, id).map_err(|e| e.to_string())?;
            return Ok(resolved.value.cloned());
        }
        if self.params.definition(id).is_none() {
            return Err(format!("no parameter has id {id}"));
        }
        let own = self.own_values(element, None)?;
        Ok(own
            .get(id)
            .cloned()
            .or_else(|| self.params.default_of(id).cloned()))
    }

    /// Every parameter with a formula that an element of its category has, worked out with
    /// construct-formulas' `work_out` over the values it reads for the others (its own, its
    /// type's, the defaults), in the order they depend on each other: each one's value, or why
    /// its formula gives none (one that uses a parameter whose formula fails says so, by name).
    ///
    /// # Errors
    /// The refusal of a stored value or of the element's type, in words.
    pub fn worked(
        &self,
        element: &Element,
        ty: Option<&ElementType>,
    ) -> Result<BTreeMap<Id, Result<Value, FormulaError>>, String> {
        let Some(category) = Self::category_of_element(element) else {
            return Ok(BTreeMap::new());
        };
        let applicable = self.table.parameters_of(&self.categories, category);
        if applicable.iter().all(|a| a.definition.formula().is_none()) {
            return Ok(BTreeMap::new());
        }
        let mut given = Values::new();
        let mut defs: Vec<Definition> = Vec::with_capacity(applicable.len());
        for parameter in applicable {
            let def = parameter.definition;
            if def.formula().is_none()
                && let Some(value) = self.stored_value_of(element, ty, def.id())?
            {
                given.set(def, value).map_err(|e| e.to_string())?;
            }
            defs.push(def.clone());
        }
        Ok(work_out(&defs, &given))
    }

    /// Every parameter with a formula that a type holds, worked out over the values the type
    /// holds for the others, or their defaults. A type parameter's formula uses only type
    /// parameters (construct-formulas' rule), so this is the value every element of the type
    /// reads.
    ///
    /// # Errors
    /// The refusal of a stored value, in words, which a type construct-types read never gives.
    pub fn type_worked(
        &self,
        ty: &ElementType,
    ) -> Result<BTreeMap<Id, Result<Value, FormulaError>>, String> {
        let held: Vec<Definition> = self
            .table
            .parameters_of(&self.categories, ty.category())
            .into_iter()
            .filter(|a| a.holder == Holder::Type)
            .map(|a| a.definition.clone())
            .collect();
        if held.iter().all(|def| def.formula().is_none()) {
            return Ok(BTreeMap::new());
        }
        let mut given = Values::new();
        for def in held.iter().filter(|def| def.formula().is_none()) {
            let value = ty
                .value(def.id())
                .or_else(|| self.params.default_of(def.id()));
            if let Some(value) = value {
                given.set(def, value.clone()).map_err(|e| e.to_string())?;
            }
        }
        Ok(work_out(&held, &given))
    }

    /// The value an element reads for a parameter: worked out from its formula when it has one
    /// ([`Context::worked`]); else through construct-types' [`resolve`] (its own, then its
    /// type's, then the default) when it has a type; else its own, then the default. `None` when
    /// nothing holds one.
    ///
    /// # Errors
    /// The refusal, in words: a parameter that is not defined, one its type lacks, or why its
    /// formula gives no value.
    pub fn value_of(
        &self,
        element: &Element,
        ty: Option<&ElementType>,
        id: &Id,
    ) -> Result<Option<Value>, String> {
        if self
            .params
            .definition(id)
            .is_some_and(|def| def.formula().is_some())
        {
            return match self.worked(element, ty)?.remove(id) {
                Some(Ok(value)) => Ok(Some(value)),
                Some(Err(why)) => Err(why.message),
                None => Err(format!("parameter {id} is not one of this element's")),
            };
        }
        self.stored_value_of(element, ty, id)
    }

    /// What a page shows of an element: its category, its type, and every parameter its category
    /// has with the value it reads, where that came from, and the value shown in units; then, for
    /// a floor or a roof, its read-only quantities ([`crate::slabs::read_only_view`],
    /// [`crate::roofs::read_only_view`]).
    ///
    /// # Errors
    /// The refusal of a stored value or of the element's type, in words.
    pub fn view(&self, element: &Element, ty: Option<&ElementType>) -> Result<ElementView, String> {
        self.view_cut(element, ty, &[])
    }

    /// [`Context::view`] of a floor cut by `cuts`: the outlines of the openings and shafts that
    /// pass through it (grid steps, its own frame), so its area, volume and mass are those after
    /// the cut ([`crate::slabs::read_only_view_cut`]).
    ///
    /// # Errors
    /// As [`Context::view`].
    pub fn view_cut(
        &self,
        element: &Element,
        ty: Option<&ElementType>,
        cuts: &[Vec<[i64; 2]>],
    ) -> Result<ElementView, String> {
        let category = Self::category_of_element(element);
        let mut worked = self.worked(element, ty)?;
        let mut parameters = Vec::new();
        let applicable = category
            .map(|category| self.table.parameters_of(&self.categories, category))
            .unwrap_or_default();
        let instance = match ty {
            Some(ty) => Some(self.instance(element, ty)?),
            None => None,
        };
        let own = self.own_values(element, None)?;
        for parameter in applicable {
            let def = parameter.definition;
            let mut reason = None;
            let (value, from) = match (worked.remove(def.id()), ty, &instance) {
                (Some(Ok(value)), _, _) => (Some(value), "formula"),
                (Some(Err(why)), _, _) => {
                    reason = Some(why.message);
                    (None, "formula")
                }
                (None, Some(ty), Some(instance)) => {
                    let resolved =
                        resolve(&self.params, ty, instance, def.id()).map_err(|e| e.to_string())?;
                    let from = match resolved.from {
                        Source::Instance => "instance",
                        Source::Type => "type",
                        Source::Default => "default",
                        Source::Unset => "unset",
                    };
                    (resolved.value.cloned(), from)
                }
                (None, ..) => match (own.get(def.id()), parameter.default) {
                    (Some(value), _) => (Some(value.clone()), "instance"),
                    (None, Some(value)) => (Some(value.clone()), "default"),
                    (None, None) => (None, "unset"),
                },
            };
            let shown = match &value {
                Some(value) => Some(def.format(value, &self.units).map_err(|e| e.to_string())?),
                None => None,
            };
            parameters.push(ParameterView {
                id: def.id().to_string(),
                name: def.name().to_owned(),
                kind: def.kind().to_string(),
                holder: match parameter.holder {
                    Holder::Instance => "instance",
                    Holder::Type => "type",
                },
                value: value.map(|value| value.stored()),
                shown,
                from,
                formula: def.formula().map(str::to_owned),
                reason,
            });
        }
        // A floor's area, perimeter, volume and mass, worked out and never stored.
        parameters.extend(crate::slabs::read_only_view_cut(
            element,
            ty,
            &self.units,
            cuts,
        ));
        // A roof's areas, pitch, ridge, volume and mass, the same way.
        parameters.extend(crate::roofs::read_only_view(element, ty, &self.units));
        Ok(ElementView {
            element: element.id.to_string(),
            category: category.and_then(|id| {
                self.categories.get(id).map(|found| CategoryView {
                    id: id.0,
                    name: found.name,
                })
            }),
            element_type: ty.map(|ty| TypeView {
                id: ty.id().to_string(),
                name: ty.name().to_owned(),
            }),
            parameters,
        })
    }
}

/// What a page shows of an element ([`Context::view`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ElementView {
    /// The element's ID.
    pub element: String,
    /// Its category, if it has one.
    pub category: Option<CategoryView>,
    /// Its type, if it has one.
    #[serde(rename = "type")]
    pub element_type: Option<TypeView>,
    /// Every parameter its category has, by name.
    pub parameters: Vec<ParameterView>,
}

/// A category, as a page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CategoryView {
    /// Its permanent ID.
    pub id: u32,
    /// Its name.
    pub name: String,
}

/// A type, as a page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TypeView {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
}

/// One parameter of an element, as a page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ParameterView {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its kind's tag, such as `length`.
    pub kind: String,
    /// `instance` or `type`: who holds its value.
    pub holder: &'static str,
    /// The value it reads, in its stored text; `None` when nothing holds one.
    pub value: Option<String>,
    /// The same value shown in the document's units, such as `820 mm`.
    pub shown: Option<String>,
    /// Where the value came from: `instance`, `type`, `default` or `unset`; `formula` when its
    /// formula gives it, which a person can't set; `read-only` for what a floor reads back from
    /// its boundary and its type ([`crate::slabs::read_only_view`]), which nobody sets.
    pub from: &'static str,
    /// Its formula, in its one spelling, when it has one.
    pub formula: Option<String>,
    /// Why its formula gives no value, in construct-formulas' words: one it uses that fails is
    /// named ("Width fails: ..."), so a parameter waiting on another says which.
    pub reason: Option<String>,
}

/// An instance's `props` as construct-types reads them: without [`CATEGORY_KEY`] and its level
/// ties (construct-levels), which are the element record's own and not the type format's.
#[must_use]
pub fn instance_props(props: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    props
        .iter()
        .filter(|(key, _)| key.as_str() != CATEGORY_KEY && !construct_levels::is_tie_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// The parameter an argument names, if it is written `{"param":...}`: its ID, or the text that
/// is not one.
fn named_parameter(arg: &serde_json::Value) -> Option<Result<Id, String>> {
    let object = arg.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let id = object.get(PARAM_ARG)?;
    Some(
        id.as_str()
            .and_then(|text| Id::parse(text).ok())
            .ok_or_else(|| id.to_string()),
    )
}

/// Whether any argument of a recipe names a parameter, so its shape reads parameter values.
#[must_use]
pub fn reads_parameters(recipe: &Recipe) -> bool {
    recipe
        .args
        .values()
        .any(|arg| named_parameter(arg).is_some())
}

/// A value as the number a stand-in recipe takes: a length in whole millimetres, a whole number
/// as it is. Interim (see the module header): the one place that knows the recipe's unit. A
/// length that is not a whole number of millimetres is a build failure naming the parameter.
fn recipe_number(value: &Value, name: &str, id: &Id) -> Result<i64, GeometryError> {
    match value {
        Value::Length(length) => length.whole_mm().ok_or_else(|| {
            failure(
                "BadParameter",
                format!(
                    "{name} reads parameter {id}, which is {length}: the stand-in builder takes whole millimetres only"
                ),
            )
        }),
        Value::Whole(n) => Ok(*n),
        _ => Err(failure(
            "BadParameter",
            format!("{name} reads parameter {id}, which is not a length or a whole number"),
        )),
    }
}

fn failure(code: &str, message: String) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message,
    }
}

/// The recipe a shape is built from: each argument that names a parameter filled with the value
/// `value_of` gives for it. A recipe that names none is returned as it is.
///
/// # Errors
/// A build failure, which is model state like any other: `BadParameter` for an argument that
/// names no parameter's ID or a value that is not a length or a whole number, `UnsetParameter`
/// when nothing holds a value, or the reason `value_of` gave.
pub fn filled(
    recipe: &Recipe,
    mut value_of: impl FnMut(&Id) -> Result<Option<Value>, String>,
) -> Result<Recipe, GeometryError> {
    let mut out = recipe.clone();
    for (name, arg) in &mut out.args {
        let Some(id) = named_parameter(arg) else {
            continue;
        };
        let id = id.map_err(|text| {
            failure(
                "BadParameter",
                format!("{name} names {text}, which is not a parameter's ID"),
            )
        })?;
        let value = value_of(&id).map_err(|why| failure("BadParameter", why))?;
        let Some(value) = value else {
            return Err(failure(
                "UnsetParameter",
                format!("{name} reads parameter {id}, which has no value"),
            ));
        };
        let number = recipe_number(&value, name, &id)?;
        *arg = serde_json::Value::from(number);
    }
    Ok(out)
}

/// The current record of one element of a document, and its hash.
///
/// # Errors
/// A [`StoreError`] when the store fails or a pointer names a missing record.
pub async fn read_element<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Id,
) -> Result<Option<(Id, Element)>, StoreError> {
    let pointer = Place::Piece {
        project: project.clone(),
        document: document.clone(),
        piece: element.clone(),
    };
    let hash: Option<String> = read_row(reader, &pointer.key()).await?;
    let Some(hash) = hash else {
        return Ok(None);
    };
    let hash = Id::parse(&hash).map_err(|e| StoreError::Damaged(e.to_string()))?;
    let record = Place::Record {
        project: project.clone(),
        hash: hash.clone(),
    };
    let element: Option<Element> = read_row(reader, &record.key()).await?;
    let element = element.ok_or_else(|| StoreError::Damaged("record missing".into()))?;
    Ok(Some((hash, element)))
}

/// A document's context: its parameter table, or an empty one when it has none yet. The inner
/// error is a stored table that cannot be read, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_context<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<Context, String>, StoreError> {
    read_context_at(reader, project, document, At::Now).await
}

/// Which state of a document a read sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    /// Its current records, by their pointers: what a command checks against.
    Now,
    /// The records as of how far regeneration has got through the log, by the kind index it
    /// keeps in log order: what a shape is built from, so a shape is the same however the log
    /// was reached.
    Regenerated,
}

/// A record of a document, of the kind it is listed under, as seen `at` that state.
pub(crate) async fn read_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    kind: &str,
    id: &Id,
    at: At,
) -> Result<Option<Element>, StoreError> {
    if at == At::Now {
        return Ok(read_element(reader, project, document, id)
            .await?
            .map(|(_, record)| record));
    }
    let Ok(kind) = Id::parse(kind) else {
        return Ok(None);
    };
    let listed = construct_model::keys::Place::Kind {
        project: project.clone(),
        document: document.clone(),
        kind,
        element: id.clone(),
    };
    let hash: Option<String> = read_row(reader, &listed.key()).await?;
    let Some(hash) = hash else {
        return Ok(None);
    };
    let hash = Id::parse(&hash).map_err(|e| StoreError::Damaged(e.to_string()))?;
    let record = Place::Record {
        project: project.clone(),
        hash,
    };
    let element: Option<Element> = read_row(reader, &record.key()).await?;
    element
        .ok_or_else(|| StoreError::Damaged("record missing".into()))
        .map(Some)
}

/// [`read_context`], as the document is `at` that state.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_context_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    at: At,
) -> Result<Result<Context, String>, StoreError> {
    let categories = CategoryTable::new();
    let table = match read_at(reader, project, document, TABLE_KIND, &table_id(), at).await? {
        None => ProjectParams::new(),
        Some(record) => match read_table(&record, &categories) {
            Ok(table) => table,
            Err(why) => return Ok(Err(why)),
        },
    };
    Ok(Context::new(table))
}

/// A type of a document, read with construct-types against the context. The inner error is
/// why it cannot be: no such type, or a record that is not one, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_type<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    id: &Id,
    context: &Context,
) -> Result<Result<ElementType, String>, StoreError> {
    read_type_at(reader, project, document, id, context, At::Now).await
}

/// [`read_type`], as the document is `at` that state.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_type_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    id: &Id,
    context: &Context,
    at: At,
) -> Result<Result<ElementType, String>, StoreError> {
    let Some(record) = read_at(reader, project, document, TYPE_KIND, id, at).await? else {
        return Ok(Err(format!("no type has id {id}")));
    };
    Ok(
        ElementType::from_record(&record, &context.params, &context.categories)
            .map_err(|e| e.to_string()),
    )
}

/// Every current record of a document, by element ID, with its hash.
///
/// # Errors
/// A [`StoreError`] when the store fails or a pointer names a missing record.
pub async fn read_document<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<BTreeMap<Id, (Id, Element)>, StoreError> {
    let pairs = reader
        .scan(&kit_store::key::list::pieces(project, document))
        .await?;
    let mut found = BTreeMap::new();
    for (key, row) in pairs {
        let Some(Place::Piece { piece, .. }) = key.parse() else {
            continue;
        };
        let hash: String = kit_store::from_row(&row)?;
        let hash = Id::parse(&hash).map_err(|e| StoreError::Damaged(e.to_string()))?;
        let record = Place::Record {
            project: project.clone(),
            hash: hash.clone(),
        };
        let element: Option<Element> = read_row(reader, &record.key()).await?;
        let element = element.ok_or_else(|| StoreError::Damaged("record missing".into()))?;
        found.insert(piece, (hash, element));
    }
    Ok(found)
}

/// The recipe `element` is built from, as the document is `at` that state: its own, with every
/// argument that names a parameter filled ([`filled`]), every argument that reads its wall type's
/// layers ([`crate::walls::filled`]), a floor's layers and loops from its floor type
/// ([`crate::slabs::filled`]), and a roof's layers and shape from its roof type
/// ([`crate::roofs::filled`]). A recipe that reads none of them needs no read. The regenerator
/// builds with it [`At::Regenerated`]; a command checks with it [`At::Now`].
///
/// # Errors
/// A [`StoreError`] when the store fails. The inner error is the build failure: `BadParameter`
/// when the document's parameters or the element's type cannot be read, or [`filled`]'s own, or
/// [`crate::walls::filled`]'s, [`crate::slabs::filled`]'s or [`crate::roofs::filled`]'s.
pub async fn recipe_at<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
    at: At,
) -> Result<Result<Recipe, GeometryError>, StoreError> {
    if !reads_records(&element.recipe) {
        return Ok(Ok(element.recipe.clone()));
    }
    let bad = |why: String| failure("BadParameter", why);
    let context: Context = match read_context_at(reader, project, document, at).await? {
        Ok(context) => context,
        Err(why) => return Ok(Err(bad(why))),
    };
    let ty = match type_of(element) {
        None => None,
        Some(type_id) => {
            match read_type_at(reader, project, document, &type_id, &context, at).await? {
                Ok(ty) => Some(ty),
                Err(why) => return Ok(Err(bad(why))),
            }
        }
    };
    Ok(filled(&element.recipe, |id| {
        context.value_of(element, ty.as_ref(), id)
    })
    .and_then(|recipe| crate::walls::filled(&recipe, &context, element, ty.as_ref()))
    .and_then(|recipe| crate::slabs::filled(&recipe, ty.as_ref()))
    .and_then(|recipe| crate::roofs::filled(&recipe, ty.as_ref())))
}

/// Whether a recipe reads the document's parameter table and the element's type: it names a
/// parameter ([`reads_parameters`]), reads its wall type's layers ([`crate::walls::reads_type`]),
/// its floor type's ([`crate::slabs::reads_type`]) or its roof type's
/// ([`crate::roofs::reads_type`]). Its `reads` row lists both, so a change to either builds it
/// again.
#[must_use]
pub fn reads_records(recipe: &Recipe) -> bool {
    reads_parameters(recipe)
        || crate::walls::reads_type(recipe)
        || crate::slabs::reads_type(recipe)
        || crate::roofs::reads_type(recipe)
}
