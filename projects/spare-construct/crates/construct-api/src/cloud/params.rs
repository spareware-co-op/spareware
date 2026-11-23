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
//! | `parameter.define` | `definition`: a construct-params definition row, which may carry a `formula` | The parameter table |
//! | `parameter.change` | `parameter`, and `name` (a new name), `formula` (text, or `null` to take it away) or both | The table: the parameter, and every formula that names it when it is renamed; when a formula is taken away, every holder of the parameter |
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
//! Formulas (construct-formulas, CON-A2-F2-P2). A definition may carry a formula over other
//! parameters by name; it is stored in its one spelling, and its value is worked out whenever it
//! is read ([`construct_regen::parameters::Context::worked`]), never stored, as every derived
//! value is (construct-model's FORMAT.md). The commands keep these rules:
//! - Every formula passes construct-formulas' `check` over every parameter of the table, and over
//!   the parameters of each category it is bound to, which are what it can see on an element: a
//!   name it uses that is not there, kinds that do not fit, a loop (named) and a type
//!   parameter's formula that uses an instance's are refused, in construct-formulas' words.
//! - A parameter with a formula takes no value from a person (`element.param`, `type.set`) and no
//!   default (`parameter.bind`). Taking its formula away (`parameter.change`) leaves each holder
//!   with the value the formula last gave it, as its own value, or none where it gave none.
//! - Renaming a parameter rewrites every formula that names it in the same command, in the one
//!   spelling (construct-formulas' `rename`); a new name a formula can't use is refused, naming
//!   the formulas that use the parameter.
//! - A command that would make a formula fail where it worked (or waited on a value) before, such
//!   as a value that divides by zero, is refused with the reason (`element.param`,
//!   `element.type`, `type.set`, `parameter.bind`, `parameter.change`). A failure other commands
//!   leave (a new element or type, an undo) is shown where the value is read, not refused.
//!
//! Later, with their places: unbinding and removing a parameter (construct-params' `unbind`
//! counts the values that go); clearing a value back to its default (construct-types' `clear`);
//! schedules (job 7.9) read the values through
//! [`construct_regen::parameters::Context::value_of`]. Undo and redo of these commands are
//! [`crate::cloud::undo`].

use crate::cloud::elements::write_record;
use construct_categories::{CategoryId, CategoryTable};
use construct_formulas::FormulaError;
use construct_model::element::Element;
use construct_params::{Definition, Holder, Kind, ProjectError, ProjectParams, Value};
use construct_regen::parameters::{self as records, CATEGORY_KEY, Context, TYPE_KEY, TYPE_KIND};
use construct_types::{ElementType, TypeError, Types};
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet};

/// Every command here.
pub const COMMANDS: [&str; 10] = [
    "parameter.define",
    "parameter.change",
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
    /// `parameter.change`'s formula: absent keeps it, `null` takes it away.
    #[serde(default, deserialize_with = "present")]
    formula: Option<NewFormula>,
}

/// The formula `parameter.change` is given.
enum NewFormula {
    /// `null`: the formula is taken away.
    Away,
    /// A formula, as typed.
    Text(String),
}

/// A formula field that is there, even as `null`.
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<NewFormula>, D::Error> {
    Option::<String>::deserialize(d).map(|f| Some(f.map_or(NewFormula::Away, NewFormula::Text)))
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

    /// Every element of the document with a shape, with its hash and its type, and every type,
    /// read against `context`: what holds parameter values.
    async fn holders(&mut self, context: &Context) -> Result<Result<Holders, String>, StoreError> {
        let all = records::read_document(self.txn, self.project, self.document).await?;
        let type_records: Vec<Element> = all
            .values()
            .filter(|(_, record)| record.kind == TYPE_KIND)
            .map(|(_, record)| record.clone())
            .collect();
        let types = match Types::from_records(&type_records, &context.params, &context.categories) {
            Ok(types) => types,
            Err(e) => return Ok(Err(why(e))),
        };
        let elements = all
            .into_values()
            .filter(|(_, record)| records::has_shape(record))
            .map(|(hash, record)| {
                let ty = records::type_of(&record).and_then(|id| types.get(&id).cloned());
                (hash, record, ty)
            })
            .collect();
        Ok(Ok(Holders { elements, types }))
    }

    /// Writes records whose pieces this person claims, as one command's whole result: another
    /// person's claim on any of them refuses it all.
    async fn put_all(&mut self, writes: Vec<(Element, Option<Id>)>) -> Answer {
        let mut held = Vec::with_capacity(writes.len());
        for (record, _) in &writes {
            match self.claim(&record.id).await? {
                Ok(claim) => held.push(claim),
                Err(refusal) => return Ok(Err(refusal)),
            }
        }
        let mut changes = Vec::with_capacity(writes.len());
        for (record, before) in &writes {
            changes.push(self.put(record, before.clone()).await?);
        }
        let mut taken = Vec::with_capacity(held.len());
        for ((record, _), claim) in writes.iter().zip(held) {
            taken.push(
                claims::take(self.txn, self.project, self.document, &record.id, claim).await?,
            );
        }
        Ok(Ok(Applied {
            changes,
            claims: taken,
            ..Applied::default()
        }))
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

/// What holds parameter values in a document: each element with a shape (its hash, its record
/// and its type) in ID order, and each type.
struct Holders {
    elements: Vec<(Id, Element, Option<ElementType>)>,
    types: Types,
}

/// One holder's formula results, by parameter ID, and its name for a refusal.
type Worked = (String, BTreeMap<Id, Result<Value, FormulaError>>);

/// An element's formula results. One whose values can't be read has none here: reading it
/// refuses elsewhere.
fn element_worked(context: &Context, element: &Element, ty: Option<&ElementType>) -> Worked {
    (
        element.name.clone(),
        context.worked(element, ty).unwrap_or_default(),
    )
}

/// A type's formula results.
fn type_worked(context: &Context, ty: &ElementType) -> Worked {
    (
        ty.name().to_owned(),
        context.type_worked(ty).unwrap_or_default(),
    )
}

/// Whether a formula's result is one it can't work out, rather than one waiting on a value (or on
/// another formula that can't, which is named where it fails).
fn fails(result: Option<&Result<Value, FormulaError>>) -> bool {
    matches!(result, Some(Err(e)) if e.code != "Depends")
}

/// Refuses a command that leaves a formula failing that did not fail before it: `before` and
/// `after` are the same holders, in the same order, either side of it. The reason is
/// construct-formulas', with the parameter and the holder named.
fn no_new_failure(context: &Context, before: &[Worked], after: &[Worked]) -> Result<(), String> {
    for ((_, was), (holder, now)) in before.iter().zip(after) {
        for (id, result) in now {
            if let Err(e) = result
                && fails(Some(result))
                && !fails(was.get(id))
            {
                let name = context
                    .table
                    .definition(id)
                    .map_or_else(|| id.to_string(), |def| def.name().to_owned());
                return Err(format!(
                    "{name} can't be worked out for {holder}: {}",
                    e.message
                ));
            }
        }
    }
    Ok(())
}

/// Refuses a value for a parameter that has a formula: the formula gives it.
fn settable(context: &Context, id: &Id) -> Result<(), String> {
    match context.table.definition(id) {
        Some(def) => match def.formula() {
            Some(formula) => Err(format!(
                "{} is given by its formula, {formula}; take the formula away to set it.",
                def.name()
            )),
            None => Ok(()),
        },
        None => Ok(()),
    }
}

/// construct-formulas' type check of every formula in `table`: over every parameter, and over
/// the parameters of each category, which are what a formula on an element of it can see. The
/// first refusal, `named`'s own first, in construct-formulas' words with the parameter (when it
/// is another) and the category (when only there) named.
fn formulas_checked(
    table: &ProjectParams,
    categories: &CategoryTable,
    named: &Id,
) -> Result<(), String> {
    let name = |id: &Id| {
        table
            .definition(id)
            .map_or_else(|| id.to_string(), |def| def.name().to_owned())
    };
    let first = |refusals: BTreeMap<Id, FormulaError>| {
        let mut ordered: Vec<(Id, FormulaError)> = refusals.into_iter().collect();
        ordered.sort_by_key(|(id, _)| id != named);
        ordered.into_iter().next()
    };
    let all: Vec<Definition> = table.definitions().into_iter().cloned().collect();
    if let Some((id, e)) = first(construct_formulas::check(&all)) {
        return Err(if &id == named {
            e.message
        } else {
            format!("{}: {}", name(&id), e.message)
        });
    }
    for category in categories.all() {
        if category.parent.is_some() {
            continue;
        }
        let defs: Vec<Definition> = table
            .parameters_of(categories, category.id)
            .into_iter()
            .map(|a| a.definition.clone())
            .collect();
        if defs.iter().all(|def| def.formula().is_none()) {
            continue;
        }
        if let Some((id, e)) = first(construct_formulas::check(&defs)) {
            return Err(if &id == named {
                format!("On {}: {}", category.name, e.message)
            } else {
                format!("On {}, {}: {}", category.name, name(&id), e.message)
            });
        }
    }
    Ok(())
}

/// `def` as written when it carries a formula: in its one spelling.
fn spelled(def: Definition) -> Result<Definition, String> {
    match def.formula().map(str::to_owned) {
        None => Ok(def),
        Some(text) => construct_formulas::define(def, &text).map_err(why),
    }
}

/// The parameter `old` under `name`, with no formula: everything else as it was.
fn plain(old: &Definition, name: &str) -> Result<Definition, String> {
    let made = if old.kind() == Kind::Choice {
        Definition::choice(old.id().clone(), name, old.holder(), old.choices().to_vec())
    } else {
        Definition::new(old.id().clone(), name, old.kind(), old.holder())
    };
    made.map_err(why)
}

/// `table` with every formula that names parameter `from` naming `to` instead. Refused, naming
/// the formulas, when `to` can't stand in one of them.
fn renamed_in_formulas(
    table: &ProjectParams,
    changed: &Id,
    from: &str,
    to: &str,
) -> Result<ProjectParams, String> {
    let mut next = table.clone();
    let mut refused: Vec<String> = Vec::new();
    let mut reason = None;
    for def in table.definitions() {
        let Some(formula) = def.formula() else {
            continue;
        };
        if def.id() == changed {
            continue;
        }
        match construct_formulas::rename(formula, from, to) {
            Ok(None) => {}
            Ok(Some(rewritten)) => {
                let def = plain(def, def.name())?
                    .with_formula(&rewritten)
                    .map_err(why)?;
                next = next.redefine(def).map_err(why)?;
            }
            Err(e) => {
                refused.push(def.name().to_owned());
                reason.get_or_insert(e.message);
            }
        }
    }
    match reason {
        None => Ok(next),
        Some(reason) => Err(format!(
            "{reason} The formulas of {} use {from}.",
            refused.join(", ")
        )),
    }
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
        "parameter.change" => change(&mut run, &document, args).await,
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
        .and_then(spelled)
        .and_then(|def| {
            let table = context
                .table
                .add_project_parameter(def.clone())
                .map_err(why)?;
            // The same parameter again changes nothing; another formula is parameter.change's.
            if let Some(existing) = context.table.definition(def.id())
                && existing.formula() != def.formula()
            {
                return Err(why(ProjectError::Conflict {
                    id: def.id().clone(),
                    name: existing.name().to_owned(),
                    field: "formula",
                }));
            }
            formulas_checked(&table, &context.categories, def.id())?;
            Ok(table)
        });
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
            Some(text) => {
                settable(context, &id).map_err(|_| {
                    format!(
                        "{} is given by its formula; it takes no default.",
                        def.name()
                    )
                })?;
                Some(def.parse(text, &context.units).map_err(why)?)
            }
            None => None,
        };
        let table = context
            .table
            .bind(&context.categories, &id, &targets, def.holder(), default)
            .map_err(why)?;
        formulas_checked(&table, &context.categories, &id)?;
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
    let holders = match run.holders(context).await? {
        Ok(holders) => holders,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let after = match Context::new(table) {
        Ok(after) => after,
        Err(refusal) => return Ok(Err(refusal)),
    };
    // Every type of a bound category lists the parameter from now on.
    let mut grown_types: BTreeMap<Id, ElementType> = BTreeMap::new();
    for ty in holders.types.iter() {
        if !targets.contains(&ty.category()) || ty.has(&id) {
            continue;
        }
        let mut grown = ty.clone();
        if let Err(e) = grown.add_parameter(&after.params, &id) {
            return Ok(Err(why(e)));
        }
        grown_types.insert(ty.id().clone(), grown);
    }
    // No formula of the categories may fail for the binding, or for its default.
    let (mut was, mut now) = (Vec::new(), Vec::new());
    for ty in holders
        .types
        .iter()
        .filter(|ty| targets.contains(&ty.category()))
    {
        was.push(type_worked(context, ty));
        now.push(type_worked(&after, grown_types.get(ty.id()).unwrap_or(ty)));
    }
    for (_, record, ty) in &holders.elements {
        if !Context::category_of_element(record).is_some_and(|c| targets.contains(&c)) {
            continue;
        }
        let grown = ty.as_ref().map(|ty| grown_types.get(ty.id()).unwrap_or(ty));
        was.push(element_worked(context, record, ty.as_ref()));
        now.push(element_worked(&after, record, grown));
    }
    if let Err(refusal) = no_new_failure(&after, &was, &now) {
        return Ok(Err(refusal));
    }
    let claim = match run.claim(&record.id).await? {
        Ok(claim) => claim,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut changes = vec![run.put(&record, before).await?];
    for grown in grown_types.values() {
        let old = run.element(grown.id()).await?.map(|(hash, _)| hash);
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
    let Some(original) = types.get(&id).cloned() else {
        return Ok(Err(why(TypeError::UnknownType(id))));
    };
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
                settable(context, &parameter)?;
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
    if run.edit.cmd == "type.set" {
        // The type's formulas, and those of every element of it, may not fail for the value.
        let holders = match run.holders(context).await? {
            Ok(holders) => holders,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let (mut was, mut now) = (
            vec![type_worked(context, &original)],
            vec![type_worked(context, ty)],
        );
        for (_, record, _) in &holders.elements {
            if records::type_of(record).as_ref() == Some(&id) {
                was.push(element_worked(context, record, Some(&original)));
                now.push(element_worked(context, record, Some(ty)));
            }
        }
        if let Err(refusal) = no_new_failure(context, &was, &now) {
            return Ok(Err(refusal));
        }
    }
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
        settable(context, &parameter)?;
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
    let after = Element {
        props,
        ..record.clone()
    };
    let was = [element_worked(context, &record, ty.as_ref())];
    let now = [element_worked(context, &after, ty.as_ref())];
    if let Err(refusal) = no_new_failure(context, &was, &now) {
        return Ok(Err(refusal));
    }
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
    let to = match to {
        Ok(to) => to,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let props = (|| {
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
    let after = Element {
        props,
        ..record.clone()
    };
    let was = [element_worked(context, &record, from.as_ref())];
    let now = [element_worked(context, &after, Some(&to))];
    if let Err(refusal) = no_new_failure(context, &was, &now) {
        return Ok(Err(refusal));
    }
    run.put_claimed(&after, Some(hash)).await
}

/// `parameter.change`: a parameter's new name, its formula given, changed or taken away. Its
/// kind, holder and choices stay (construct-params' `redefine`), and so does every value stored
/// under its ID.
async fn change<T: Txn>(run: &mut Run<'_, T>, context: &Context, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.parameter, "a parameter")?;
        let old = context
            .table
            .definition(&id)
            .cloned()
            .ok_or_else(|| why(ProjectError::UnknownParameter(id.clone())))?;
        if args.name.is_none() && args.formula.is_none() {
            return Err("This command needs a name or a formula.".to_owned());
        }
        let name = args.name.unwrap_or_else(|| old.name().to_owned());
        let formula = match args.formula {
            None => old.formula().map(str::to_owned),
            Some(NewFormula::Away) => None,
            Some(NewFormula::Text(text)) => Some(text),
        };
        let base = plain(&old, &name)?;
        let def = match &formula {
            Some(text) => construct_formulas::define(base, text).map_err(why)?,
            None => base,
        };
        let mut table = context.table.redefine(def.clone()).map_err(why)?;
        if name != old.name() {
            table = renamed_in_formulas(&table, &id, old.name(), &name)?;
        }
        formulas_checked(&table, &context.categories, &id)?;
        let after = Context::new(table.clone())?;
        Ok::<_, String>((old, def, table, after))
    })();
    let (old, def, table, after) = match made {
        Ok(made) => made,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let (record, before) = match write_table(run, &table).await? {
        Ok(write) => write,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let holders = match run.holders(context).await? {
        Ok(holders) => holders,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let id = def.id().clone();
    let (holder, bound) = table.binding(&id).unwrap_or((def.holder(), Vec::new()));
    let bound: BTreeSet<CategoryId> = bound.into_iter().collect();
    // Taking a formula away leaves each holder the value it last gave, as its own.
    let taken_away = old.formula().is_some() && def.formula().is_none();
    let mut writes = vec![(record, before)];
    let (mut was, mut now) = (Vec::new(), Vec::new());
    let mut types_after: BTreeMap<Id, ElementType> = BTreeMap::new();
    for ty in holders
        .types
        .iter()
        .filter(|ty| bound.contains(&ty.category()))
    {
        let last = type_worked(context, ty);
        let mut next = ty.clone();
        if taken_away && holder == Holder::Type {
            let kept = if let Some(Ok(value)) = last.1.get(&id) {
                next.set(&after.params, &id, value.clone())
            } else {
                next.clear(&id);
                Ok(())
            };
            if let Err(e) = kept {
                return Ok(Err(why(e)));
            }
            if &next != ty {
                let old = run.element(ty.id()).await?.map(|(hash, _)| hash);
                writes.push((next.to_record(run.document), old));
            }
        }
        was.push(last);
        now.push(type_worked(&after, &next));
        types_after.insert(ty.id().clone(), next);
    }
    for (hash, record, ty) in &holders.elements {
        if !Context::category_of_element(record).is_some_and(|c| bound.contains(&c)) {
            continue;
        }
        let last = element_worked(context, record, ty.as_ref());
        let mut next = record.clone();
        if taken_away && holder == Holder::Instance {
            match last.1.get(&id) {
                Some(Ok(value)) => next.props.insert(id.to_string(), value.stored()),
                _ => next.props.remove(id.as_str()),
            };
            if &next != record {
                writes.push((next.clone(), Some(hash.clone())));
            }
        }
        let ty_after = ty.as_ref().map(|ty| types_after.get(ty.id()).unwrap_or(ty));
        was.push(last);
        now.push(element_worked(&after, &next, ty_after));
    }
    if let Err(refusal) = no_new_failure(&after, &was, &now) {
        return Ok(Err(refusal));
    }
    run.put_all(writes).await
}
