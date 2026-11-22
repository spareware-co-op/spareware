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
//! - Records of kind `type` and [`TABLE_KIND`] have no shape ([`has_shape`]).
//!
//! Depends on construct-model (records), construct-categories, construct-params and
//! construct-types. Interim: [`recipe_number`] turns a length into the stand-in's whole-millimetre
//! recipe number; it changes with the kernel's recipes (piece A39 makes lengths the kernel's grid
//! unit, job 1.1 the kernel seam), and nothing else here knows the stand-in's units.
//!
//! Not here yet, with their places: the document's own subcategories (the categories command, a
//! later job, feeds [`Context::categories`]); display settings per project (typed text is read
//! with construct-params' metric [`Units`] default, [`Context::units`]); formulas (job 2.5) fill
//! a parameter from others before [`value_of`] answers.

use construct_categories::{CategoryError, CategoryId, CategoryTable, category_of_kind};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Holder, ProjectParams, Units, Value, Values};
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

/// Whether a record is an element with a shape: type records, parameter tables and levels are
/// not.
#[must_use]
pub fn has_shape(element: &Element) -> bool {
    element.kind != TYPE_KIND
        && element.kind != TABLE_KIND
        && element.kind != construct_levels::LEVEL_KIND
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

    /// The value an element reads for a parameter: through construct-types' [`resolve`] (its own,
    /// then its type's, then the default) when it has a type; else its own, then the default.
    /// `None` when nothing holds one.
    ///
    /// # Errors
    /// The refusal, in words: a parameter that is not defined, or one its type lacks.
    pub fn value_of(
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

    /// What a page shows of an element: its category, its type, and every parameter its category
    /// has with the value it reads, where that came from, and the value shown in units.
    ///
    /// # Errors
    /// The refusal of a stored value or of the element's type, in words.
    pub fn view(&self, element: &Element, ty: Option<&ElementType>) -> Result<ElementView, String> {
        let category = match stored_category(element) {
            Some(category) => Some(category),
            None if has_shape(element) => category_of(&element.recipe).ok(),
            None => None,
        };
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
            let (value, from) = match (ty, &instance) {
                (Some(ty), Some(instance)) => {
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
                _ => match (own.get(def.id()), parameter.default) {
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
            });
        }
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
    /// Where the value came from: `instance`, `type`, `default` or `unset`.
    pub from: &'static str,
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
