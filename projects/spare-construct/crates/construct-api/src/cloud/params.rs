//! The commands that change a document's parameters and types, and the parameter values and type
//! of an element. They go through the same edit log as `element.*`, with the same access check
//! (`model.edit`) and the same claims: a piece another person is working on is refused, so two
//! people never write the same value at once, and one person's edits land in log order.
//!
//! Each command reads what it needs through [`construct_regen::parameters`] (the same reader the
//! regenerator builds shapes with), checks the change with the data crates' own functions
//! (construct-params for definitions, bindings and values, construct-types for types and
//! instances, construct-categories for categories), and writes the changed records. A refusal is
//! the command's result, in the data crate's own words, and changes nothing. Nothing here reads a
//! clock or a random number: the claim's time is the orderer's, recorded in the entry.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `parameter.define` | `definition`: a construct-params definition row | The parameter table |
//! | `parameter.bind` | `parameter`, `categories` (top-level category IDs), `default` (typed text or null) | The table, and each type of those categories, which then lists the parameter |
//! | `type.create` | `type`, `name`, `category` | A type record listing every parameter bound to its category |
//! | `type.rename` | `type`, `name` | The type |
//! | `type.duplicate` | `type`, `new`, `name` | A new type, a copy |
//! | `type.delete` | `type` | Its pointer goes; refused while an element is of it |
//! | `type.set` | `type`, `parameter`, `value` (typed text) | The type |
//! | `element.param` | `element`, `parameter`, `value` (typed text) | The element: that parameter's entry replaced |
//! | `element.type` | `element`, `type` | The element: its type, and the values its new type lacks dropped |
//!
//! Typed text is read with [`construct_params::Definition::parse`]: `2400`, `2.4 m`, `2400mm`.
//! A command acts on one document's parameters: its table is one record (see
//! [`construct_regen::parameters`]).
//!
//! Later, with their places: unbinding and removing a parameter (construct-params' `unbind`
//! counts the values that go); clearing a value back to its default (construct-types' `clear`);
//! schedules (job 7.9) read the values through
//! [`construct_regen::parameters::Context::value_of`]; formulas (job 2.5). Undo and redo of
//! these commands are [`crate::cloud::undo`].

use crate::cloud::elements::write_record;
use construct_categories::CategoryId;
use construct_model::element::Element;
use construct_params::{Definition, Holder, ProjectParams, Value};
use construct_regen::parameters::{self as records, CATEGORY_KEY, Context, TYPE_KEY, TYPE_KIND};
use construct_types::{ElementType, TypeError, Types};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 9] = [
    "parameter.define",
    "parameter.bind",
    "type.create",
    "type.rename",
    "type.duplicate",
    "type.delete",
    "type.set",
    "element.param",
    "element.type",
];

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    definition: Option<serde_json::Value>,
    #[serde(default, with = "kit_ops::id::option")]
    parameter: Option<Id>,
    categories: Option<Vec<CategoryId>>,
    default: Option<String>,
    #[serde(default, rename = "type", with = "kit_ops::id::option")]
    type_id: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    new: Option<Id>,
    name: Option<String>,
    category: Option<CategoryId>,
    value: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

/// What a command is run against: the store, where, who, and when the orderer says it is.
struct Run<'a, T: Txn> {
    txn: &'a mut T,
    project: &'a Id,
    document: &'a Id,
    edit: &'a Edit,
    now_ms: u64,
    claim_ms: u64,
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

/// Turns a data crate's refusal into the command's result.
fn why(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn need<T>(value: Option<T>, what: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("This command needs {what}."))
}

impl<T: Txn> Run<'_, T> {
    /// The claim this person takes on a piece, or who else holds it.
    async fn claim(&mut self, piece: &Id) -> Result<Result<Claim, String>, StoreError> {
        let holder = claims::held_by_another(
            self.txn,
            self.project,
            self.document,
            piece,
            &self.edit.person,
            self.now_ms,
        )
        .await?;
        if let Some(claim) = holder {
            return Ok(Err(format!("{} is working on this.", claim.name)));
        }
        let person: Option<Named> =
            read_row(self.txn, &Place::Person(self.edit.person.clone()).key()).await?;
        let name = person.map_or_else(|| self.edit.person.to_string(), |person| person.name);
        Ok(Ok(Claim {
            person: self.edit.person.clone(),
            name,
            // milliseconds since 1970 plus a claim's length: far below u64::MAX
            until_ms: self.now_ms.saturating_add(self.claim_ms),
        }))
    }

    /// Writes a record and points its piece at it; the change, from `before`.
    async fn put(&mut self, record: &Element, before: Option<Id>) -> Result<Change, StoreError> {
        let hash = write_record(self.txn, self.project, record).await?;
        let pointer = Place::Piece {
            project: self.project.clone(),
            document: self.document.clone(),
            piece: record.id.clone(),
        };
        self.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        Ok(Change {
            piece: record.id.clone(),
            before,
            after: Some(hash),
        })
    }

    /// Writes one record whose piece this person claims, as one command's whole result.
    async fn put_claimed(&mut self, record: &Element, before: Option<Id>) -> Answer {
        let claim = match self.claim(&record.id).await? {
            Ok(claim) => claim,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let change = self.put(record, before).await?;
        let claim = claims::take(self.txn, self.project, self.document, &record.id, claim).await?;
        Ok(Ok(Applied {
            changes: vec![change],
            claims: vec![claim],
            ..Applied::default()
        }))
    }

    /// The document's current record of a piece, and its hash.
    async fn element(&mut self, id: &Id) -> Result<Option<(Id, Element)>, StoreError> {
        records::read_element(self.txn, self.project, self.document, id).await
    }

    /// Every type of the document, read against `context`, as construct-types keeps them.
    async fn types(&mut self, context: &Context) -> Result<Result<Types, String>, StoreError> {
        let all = records::read_document(self.txn, self.project, self.document).await?;
        let records: Vec<Element> = all
            .into_values()
            .map(|(_, record)| record)
            .filter(|record| record.kind == TYPE_KIND)
            .collect();
        Ok(Types::from_records(&records, &context.params, &context.categories).map_err(why))
    }
}

/// A parameter's definition in the context, or construct-types' refusal naming it.
fn definition<'a>(context: &'a Context, id: &Id) -> Result<&'a Definition, String> {
    context
        .params
        .definition(id)
        .ok_or_else(|| why(TypeError::UnknownParameter(id.clone())))
}

/// Typed text read as a value of the parameter, on the server, with construct-params' parser.
fn typed(context: &Context, id: &Id, text: &str) -> Result<(Definition, Value), String> {
    let def = definition(context, id)?;
    let value = def.parse(text, &context.units).map_err(why)?;
    Ok((def.clone(), value))
}

/// The element's `props` with its parameter entries replaced by `own` (an instance's or an
/// element's own values, as `props` entries), its type named or not, and its category kept.
/// Every key that is not a defined parameter or a reserved key stays as it was.
fn with_values(
    context: &Context,
    element: &Element,
    own: BTreeMap<String, String>,
    category: CategoryId,
) -> BTreeMap<String, String> {
    let mut props: BTreeMap<String, String> = element
        .props
        .iter()
        .filter(|(key, _)| {
            key.as_str() != TYPE_KEY
                && !Id::parse(key).is_ok_and(|id| context.params.definition(&id).is_some())
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    props.extend(own);
    props.insert(CATEGORY_KEY.to_owned(), records::write_category(category));
    props
}

/// An element's category: the one it records, else its kind's.
fn category_of_element(element: &Element) -> Result<CategoryId, String> {
    match records::stored_category(element) {
        Some(category) => Ok(category),
        None => records::category_of(&element.recipe).map_err(why),
    }
}

/// Runs one of these commands.
///
/// # Errors
///
/// A store error; a command that cannot be made is the outcome's error instead.
pub async fn apply<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Ordered<'_>,
    edit: &Edit,
    claim_ms: u64,
) -> Result<Outcome, StoreError> {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    let mut run = Run {
        txn,
        project,
        document: &edit.target,
        edit,
        now_ms: context.now_ms,
        claim_ms,
    };
    let document = match records::read_context(run.txn, project, &edit.target).await? {
        Ok(document) => document,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match edit.cmd.as_str() {
        "parameter.define" => define(&mut run, &document, args).await,
        "parameter.bind" => bind(&mut run, &document, args).await,
        "type.create" => create_type(&mut run, &document, args).await,
        "type.rename" | "type.duplicate" | "type.set" => {
            change_type(&mut run, &document, args).await
        }
        "type.delete" => delete_type(&mut run, &document, args).await,
        "element.param" => set_param(&mut run, &document, args).await,
        "element.type" => retype(&mut run, &document, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// The table record's write, from the table as it was.
async fn write_table<T: Txn>(
    run: &mut Run<'_, T>,
    table: &ProjectParams,
) -> Result<Result<(Element, Option<Id>), String>, StoreError> {
    let before = run
        .element(&records::table_id())
        .await?
        .map(|(hash, _)| hash);
    Ok(records::table_record(run.document, table).map(|record| (record, before)))
}

async fn define<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let row = match need(args.definition, "a definition") {
        Ok(row) => row.to_string(),
        Err(refusal) => return Ok(Err(refusal)),
    };
    let table = Definition::from_json(&row)
        .map_err(why)
        .and_then(|def| context.table.add_project_parameter(def).map_err(why));
    let table = match table {
        Ok(table) => table,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (record, before) = match write_table(run, &table).await? {
        Ok(write) => write,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.put_claimed(&record, before).await
}

async fn bind<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let checked = (|| {
        let id = need(args.parameter, "a parameter")?;
        let targets = need(args.categories, "categories")?;
        let def = context
            .table
            .definition(&id)
            .ok_or_else(|| why(construct_params::ProjectError::UnknownParameter(id.clone())))?;
        let default = match &args.default {
            Some(text) => Some(def.parse(text, &context.units).map_err(why)?),
            None => None,
        };
        let table = context
            .table
            .bind(&context.categories, &id, &targets, def.holder(), default)
            .map_err(why)?;
        Ok::<_, String>((id, targets, table))
    })();
    let (id, targets, table) = match checked {
        Ok(checked) => checked,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (record, before) = match write_table(run, &table).await? {
        Ok(write) => write,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let types = match run.types(context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let after = match Context::new(table) {
        Ok(after) => after,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let claim = match run.claim(&record.id).await? {
        Ok(claim) => claim,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut changes = vec![run.put(&record, before).await?];
    // Every type of a bound category lists the parameter from now on.
    for ty in types.iter() {
        if !targets.contains(&ty.category()) || ty.has(&id) {
            continue;
        }
        let mut grown = ty.clone();
        if let Err(e) = grown.add_parameter(&after.params, &id) {
            return Ok(Err(why(e)));
        }
        let old = run.element(ty.id()).await?.map(|(hash, _)| hash);
        changes.push(run.put(&grown.to_record(run.document), old).await?);
    }
    let claim = claims::take(run.txn, run.project, run.document, &record.id, claim).await?;
    Ok(Ok(Applied {
        changes,
        claims: vec![claim],
        ..Applied::default()
    }))
}

async fn create_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.type_id, "a type")?;
        let name = need(args.name, "a name")?;
        let category = need(args.category, "a category")?;
        let mut ty = ElementType::new(id, &name, category, &context.categories).map_err(why)?;
        for applicable in context.table.parameters_of(&context.categories, category) {
            ty.add_parameter(&context.params, applicable.definition.id())
                .map_err(why)?;
        }
        Ok::<_, String>(ty)
    })();
    let ty = match made {
        Ok(ty) => ty,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.element(ty.id()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut types = match run.types(context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let record = ty.to_record(run.document);
    if let Err(e) = types.add(ty) {
        return Ok(Err(why(e)));
    }
    run.put_claimed(&record, None).await
}

/// `type.rename`, `type.duplicate` and `type.set`: each changes one type in the set of types,
/// and writes the one record that changed.
async fn change_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let id = match need(args.type_id, "a type") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut types = match run.types(context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if types.get(&id).is_none() {
        return Ok(Err(why(TypeError::UnknownType(id))));
    }
    let changed = match run.edit.cmd.as_str() {
        "type.rename" => need(args.name, "a name")
            .and_then(|name| types.rename(&id, &name).map_err(why))
            .map(|()| id.clone()),
        "type.duplicate" => match (need(args.new, "a new type's id"), need(args.name, "a name")) {
            (Ok(new), Ok(name)) => {
                if run.element(&new).await?.is_some() {
                    return Ok(Err("That element already exists.".to_owned()));
                }
                types
                    .duplicate(&id, new.clone(), &name)
                    .map_err(why)
                    .map(|()| new)
            }
            (Err(refusal), _) | (_, Err(refusal)) => Err(refusal),
        },
        _ => {
            let set = (|| {
                let parameter = need(args.parameter, "a parameter")?;
                let text = need(args.value, "a value")?;
                let (_, value) = typed(context, &parameter, &text)?;
                let ty = types
                    .get_mut(&id)
                    .ok_or_else(|| why(TypeError::UnknownType(id.clone())))?;
                ty.set(&context.params, &parameter, value).map_err(why)
            })();
            set.map(|()| id.clone())
        }
    };
    let changed = match changed {
        Ok(changed) => changed,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some(ty) = types.get(&changed) else {
        return Ok(Err(why(TypeError::UnknownType(changed))));
    };
    let record = ty.to_record(run.document);
    let before = run.element(&changed).await?.map(|(hash, _)| hash);
    run.put_claimed(&record, before).await
}

async fn delete_type<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let id = match need(args.type_id, "a type") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut types = match run.types(context).await? {
        Ok(types) => types,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some(ty) = types.get(&id).cloned() else {
        return Ok(Err(why(TypeError::UnknownType(id))));
    };
    // construct-types keeps no index of instances: every element of the document that names
    // this type is read as an instance of it and handed over.
    let all = records::read_document(run.txn, run.project, run.document).await?;
    let mut instances = Vec::new();
    for (_, record) in all.values() {
        if records::type_of(record).as_ref() == Some(&id) {
            match context.instance(record, &ty) {
                Ok(instance) => instances.push(instance),
                Err(refusal) => return Ok(Err(refusal)),
            }
        }
    }
    if let Err(e) = types.delete(&id, instances.iter()) {
        return Ok(Err(why(e)));
    }
    if let Err(refusal) = run.claim(&id).await? {
        return Ok(Err(refusal));
    }
    let Some((hash, _)) = run.element(&id).await? else {
        return Ok(Err(why(TypeError::UnknownType(id))));
    };
    let pointer = Place::Piece {
        project: run.project.clone(),
        document: run.document.clone(),
        piece: id.clone(),
    };
    run.txn.delete(pointer.key()).await?;
    let released = claims::release(run.txn, run.project, run.document, &id).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: id,
            before: Some(hash),
            after: None,
        }],
        claims: vec![released],
        ..Applied::default()
    }))
}

/// The element a command acts on, with its type if it has one. Types and the parameter table
/// are not elements here.
async fn target<T: Txn>(
    run: &mut Run<'_, T>,
    context: &Context,
    element: Option<Id>,
) -> Result<Result<(Id, Element, Option<ElementType>), String>, StoreError> {
    let id = match need(element, "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((hash, record)) = run.element(&id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    if !records::has_shape(&record) {
        return Ok(Err(
            "Types, levels and the parameter table are changed by their own commands, not as elements."
                .to_owned(),
        ));
    }
    let ty = match records::type_of(&record) {
        None => None,
        Some(type_id) => {
            let read =
                records::read_type(run.txn, run.project, run.document, &type_id, context).await?;
            match read {
                Ok(ty) => Some(ty),
                Err(refusal) => return Ok(Err(refusal)),
            }
        }
    };
    Ok(Ok((hash, record, ty)))
}

async fn set_param<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let (hash, record, ty) = match target(run, context, args.element).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let props = (|| {
        let parameter = need(args.parameter, "a parameter")?;
        let text = need(args.value, "a value")?;
        let (def, value) = typed(context, &parameter, &text)?;
        let category = category_of_element(&record)?;
        let own = if let Some(ty) = &ty {
            let mut instance = context.instance(&record, ty)?;
            instance
                .set(&context.params, ty, &parameter, value)
                .map_err(why)?;
            instance.to_props()
        } else {
            // With no type, the element has what its category is bound to.
            let bound = context
                .table
                .parameters_of(&context.categories, category)
                .into_iter()
                .find(|applicable| applicable.definition.id() == &parameter);
            match bound.map(|applicable| applicable.holder) {
                None => {
                    return Err(format!(
                        "{:?} is not a parameter of this element's category",
                        def.name()
                    ));
                }
                Some(Holder::Type) => {
                    return Err(why(TypeError::HeldByType {
                        parameter: def.name().to_owned(),
                    }));
                }
                Some(Holder::Instance) => {}
            }
            let mut values = context.own_values(&record, None)?;
            values.set(&def, value).map_err(why)?;
            values.to_props()
        };
        Ok::<_, String>(with_values(context, &record, own, category))
    })();
    let props = match props {
        Ok(props) => props,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let after = Element { props, ..record };
    run.put_claimed(&after, Some(hash)).await
}

async fn retype<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let (hash, record, from) = match target(run, context, args.element).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let to = match need(args.type_id, "a type") {
        Ok(id) => records::read_type(run.txn, run.project, run.document, &id, context).await?,
        Err(refusal) => Err(refusal),
    };
    let props = (|| {
        let to = to?;
        let category = records::category_of(&record.recipe).map_err(why)?;
        let own = if let Some(from) = &from {
            let mut instance = context.instance(&record, from)?;
            instance.change_type(from, &to).map_err(why)?;
            instance.to_props()
        } else {
            if to.category() != category {
                return Err(why(TypeError::OtherCategory {
                    from: category,
                    to: to.category(),
                }));
            }
            // Its own values stay; construct-types checks each against the new type.
            let mut props = record.props.clone();
            props.insert(TYPE_KEY.to_owned(), to.id().to_string());
            let named = Element {
                props,
                ..record.clone()
            };
            context.instance(&named, &to)?.to_props()
        };
        Ok::<_, String>(with_values(context, &record, own, category))
    })();
    let props = match props {
        Ok(props) => props,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let after = Element { props, ..record };
    run.put_claimed(&after, Some(hash)).await
}
