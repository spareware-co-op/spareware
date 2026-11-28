//! View templates, per-element overrides and view filters, through the edit log (CON-A8-F3-P4,
//! CON-A8-F3-P6). The commands are listed with their arguments in [`crate::cloud::views`]; the
//! types and every rule are construct-views' (`ViewTemplate`, `Overrides`, `Filters`), and this
//! file only reads the document, asks them and writes the one record each command changes, with
//! the same access check and claims as every view command. Nothing here reads a clock or a random
//! number.
//!
//! What a person can rely on:
//!
//! - A **view template** is its own record. A view **tied** to one (`view.template.tie`) shows the
//!   template's included settings in place of its own, so changing the template changes every tied
//!   view in the same step and no view record is rewritten. A tied view's own command that would
//!   change an included setting (`view.set` of a scale or detail, `view.graphics.*`,
//!   `view.categories.set`, `view.filter.*`) is refused, in words, naming the template. A template
//!   **applied** once (`view.template.apply`) copies the included settings into a view that stays
//!   free. Untying (`view.template.untie`) keeps what the view was showing. A template cannot be
//!   deleted while a view is tied to it, and an undo that would leave a view tied to a template
//!   the model lacks is turned back.
//! - An **override** (`view.override.set` and `view.override.clear`) is one element's look in one
//!   view, over its category's rule and its filters. It is the view's own even when the view is
//!   tied.
//! - A **filter** is a rule on a category and one of its parameters, with its own look; the
//!   list's order decides which wins ([`construct_views::Filters::style_for`]). Filters are asked
//!   of an element's parameters whenever a view is drawn ([`element_styles`]), so a parameter
//!   that changes changes the drawing at once.
//!
//! A drafting view or a legend shows no model, so it takes no template, override, filter or hidden
//! category.

use crate::cloud::views::{
    Answer, Args, Run, World, category_of, edge_of, need, put_changed, style_over, subject,
};
use construct_categories::{CategoryId, CategoryKind, VIEWS};
use construct_model::element::Element;
use construct_params::Definition;
use construct_regen::parameters::{self as records, Context};
use construct_views::{
    DetailLevel, ElementStyle, Filter, Filters, Operator, Scale, Setting, Settings, TEMPLATE_KIND,
    VIEW_KIND, View, ViewError, ViewTemplate, ViewTemplates,
};
use kit_core::Id;
use kit_ops::Change;
use kit_store::{Place, Reader, StoreError, Txn};
use kit_sync::{Applied, claims};
use serde::Serialize;
use std::collections::BTreeMap;

/// The scale and detail level of a template made with no view to take them from.
const FRESH_SCALE: u32 = 100;
const FRESH_DETAIL: DetailLevel = DetailLevel::Medium;

/// Refuses an argument the command does not take, naming it.
pub(super) fn stray(cmd: &str, args: &Args) -> Option<String> {
    let given = [
        ("template", args.template.is_some()),
        ("source", args.source.is_some()),
        ("element", args.element.is_some()),
        ("included", args.included.is_some()),
        ("filter", args.filter.is_some()),
        ("parameter", args.parameter.is_some()),
        ("op", args.op.is_some()),
        ("value", args.value.is_some()),
        ("place", args.place.is_some()),
        ("appearance", args.appearance.is_some()),
        ("shown", args.shown.is_some()),
    ];
    let allowed: &[&str] = match cmd {
        "view.categories.set" => &["template", "shown"],
        "view.override.set" => &["element", "appearance"],
        "view.override.clear" => &["element"],
        "view.filter.add" => &["template", "parameter", "op", "value", "appearance"],
        "view.filter.set" => &[
            "template",
            "filter",
            "parameter",
            "op",
            "value",
            "appearance",
        ],
        "view.filter.remove" => &["template", "filter"],
        "view.filter.move" => &["template", "filter", "place"],
        "view.template.create" | "view.template.update" => &["template", "source"],
        "view.template.set" => &["template", "included"],
        "view.graphics.set"
        | "view.graphics.clear"
        | "view.template.rename"
        | "view.template.delete"
        | "view.template.tie"
        | "view.template.apply" => &["template"],
        _ => &[],
    };
    given
        .iter()
        .find(|(key, on)| *on && !allowed.contains(key))
        .map(|(key, _)| format!("{cmd} has no {key}."))
}

/// Runs one of the commands of this file.
pub(super) async fn apply<T: Txn>(run: &mut Run<'_, T>, cmd: &str, args: Args) -> Answer {
    match cmd {
        "view.categories.set" => categories(run, args).await,
        "view.override.set" => override_set(run, args).await,
        "view.override.clear" => override_clear(run, args).await,
        "view.filter.add" | "view.filter.set" | "view.filter.remove" | "view.filter.move" => {
            filter(run, cmd, args).await
        }
        "view.template.create" => create(run, args).await,
        "view.template.rename" => rename(run, args).await,
        "view.template.delete" => delete(run, args).await,
        "view.template.set" => set(run, args).await,
        "view.template.update" => update(run, args).await,
        "view.template.tie" => tie(run, args).await,
        "view.template.untie" => untie(run, args).await,
        "view.template.apply" => apply_once(run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// What a command about settings changes: a view or a view template, with the hash of its record.
enum Owner {
    View(Id, Box<View>),
    Template(Id, ViewTemplate),
}

impl Owner {
    fn settings(&self) -> Settings {
        match self {
            Self::View(_, view) => view.settings(),
            Self::Template(_, template) => template.settings().clone(),
        }
    }

    /// Asks whether this can hold a setting the command changes: a view that shows no model
    /// cannot, and a tied view cannot change one its template includes.
    fn check_free(&self, world: &World, setting: Setting) -> Result<(), String> {
        match self {
            Self::View(_, view) => {
                if !view.kind().shows_model() {
                    return Err(ViewError::NoOverridesOnKind {
                        kind: view.kind().key().replace('_', " "),
                    }
                    .to_string());
                }
                world
                    .templates()
                    .tied_refusal(view, setting)
                    .map_err(|e| e.to_string())
            }
            Self::Template(..) => Ok(()),
        }
    }
}

/// A template of this ID as it is now, with its record's hash.
async fn template_of<T: Txn>(
    run: &mut Run<'_, T>,
    id: &Id,
) -> Result<Result<(Id, ViewTemplate), String>, StoreError> {
    Ok(match run.current(id).await? {
        Some((hash, record)) if record.kind == TEMPLATE_KIND => ViewTemplate::from_record(&record)
            .map(|template| (hash, template))
            .map_err(|e| e.to_string()),
        _ => Err(ViewError::UnknownTemplate(id.clone()).to_string()),
    })
}

/// The world and the view or template a settings command names (one of them, not both).
async fn owner<T: Txn>(
    run: &mut Run<'_, T>,
    args: &Args,
) -> Result<Result<(World, Owner), String>, StoreError> {
    match (&args.view, &args.template) {
        (Some(_), Some(_)) => Ok(Err("Name a view or a view template, not both.".to_owned())),
        (None, None) => Ok(Err(
            "This command needs a view or a view template.".to_owned()
        )),
        (Some(view), None) => Ok(match subject(run, Some(view.clone())).await? {
            Ok((world, hash, view)) => Ok((world, Owner::View(hash, Box::new(view)))),
            Err(refusal) => Err(refusal),
        }),
        (None, Some(id)) => {
            let world = match run.world().await? {
                Ok(world) => world,
                Err(refusal) => return Ok(Err(refusal)),
            };
            Ok(match template_of(run, id).await? {
                Ok((hash, template)) => Ok((world, Owner::Template(hash, template))),
                Err(refusal) => Err(refusal),
            })
        }
    }
}

/// Writes a template as the command's whole result; nothing at all when it did not change.
async fn put_template<T: Txn>(
    run: &mut Run<'_, T>,
    world: &World,
    before: (Id, ViewTemplate),
    changed: ViewTemplate,
) -> Answer {
    let (hash, old) = before;
    if changed == old {
        return Ok(Ok(Applied::default()));
    }
    let mut templates = world.templates().clone();
    if let Err(e) = templates.replace(changed.clone()) {
        return Ok(Err(e.to_string()));
    }
    run.put_claimed(&changed.to_record(run.document), Some(hash))
        .await
}

/// Writes the owner with these settings in place of its own.
async fn put_settings<T: Txn>(
    run: &mut Run<'_, T>,
    world: &World,
    owner: Owner,
    settings: Settings,
) -> Answer {
    match owner {
        Owner::View(hash, view) => {
            let view = *view;
            let changed = view.clone().with_settings(settings);
            put_changed(run, world, (hash, view), changed).await
        }
        Owner::Template(hash, template) => {
            let changed = template.clone().with_settings(settings);
            put_template(run, world, (hash, template), changed).await
        }
    }
}

/// The graphics commands for a view template: the rules a template holds are the rules a view
/// holds, set and cleared the same way (the views module's own commands do a view's).
pub(super) async fn graphics<T: Txn>(run: &mut Run<'_, T>, args: Args, set: bool) -> Answer {
    let (world, owner) = match owner(run, &args).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Owner::Template(_, template) = &owner else {
        return Ok(Err("Name a view or a view template, not both.".to_owned()));
    };
    let done = (|| -> Result<Settings, String> {
        let mut settings = template.settings().clone();
        let category = category_of(&args, &world)?;
        if set {
            let edge = edge_of(need(args.edge.as_deref(), "an edge (cut or projection)")?)?;
            let named = need(args.style.as_ref(), "a style")?;
            let base = crate::cloud::views::look_of_settings(&world, &settings, category, edge)?;
            let style = style_over(base.style, named)?;
            settings.graphics = settings.graphics.with_style(category, edge, style);
            settings
                .graphics
                .check_rules(world.categories(), world.line_styles())
                .map_err(|e| e.to_string())?;
        } else {
            if args.style.is_some() {
                return Err("view.graphics.clear takes no style.".to_owned());
            }
            let edge = args.edge.as_deref().map(edge_of).transpose()?;
            settings.graphics = settings.graphics.without(category, edge);
        }
        Ok(settings)
    })();
    match done {
        Ok(settings) => put_settings(run, &world, owner, settings).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn categories<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, owner) = match owner(run, &args).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let done = (|| -> Result<Settings, String> {
        owner.check_free(&world, Setting::Categories)?;
        let category = category_of(&args, &world)?;
        let found = world
            .categories()
            .get(category)
            .ok_or_else(|| ViewError::UnknownCategory(category).to_string())?;
        if found.id == VIEWS || found.parent == Some(VIEWS) {
            return Err(ViewError::NotStyled(category).to_string());
        }
        let on = need(args.shown, "shown (true or false)")?;
        let mut settings = owner.settings();
        if on {
            settings.hidden.remove(&category);
        } else {
            settings.hidden.insert(category);
        }
        Ok(settings)
    })();
    match done {
        Ok(settings) => put_settings(run, &world, owner, settings).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// An element's category, as its record says.
fn category_of_record(record: &Element) -> Option<CategoryId> {
    records::stored_category(record).or_else(|| records::category_of(&record.recipe).ok())
}

async fn override_set<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if !view.kind().shows_model() {
        return Ok(Err(ViewError::NoOverridesOnKind {
            kind: view.kind().key().replace('_', " "),
        }
        .to_string()));
    }
    let element = match need(args.element.clone(), "an element") {
        Ok(element) => element,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((_, record)) = run.current(&element).await? else {
        return Ok(Err(format!("There is no element {element} in this model.")));
    };
    let category = match category_of_record(&record) {
        Some(category) if category != VIEWS && record.kind != VIEW_KIND => category,
        _ => {
            return Ok(Err(format!(
                "{} is not something a view draws, so it has no override.",
                record.name
            )));
        }
    };
    let made = (|| -> Result<View, String> {
        let style = need(
            args.appearance.as_ref(),
            "a look (any of pen, pattern, colour, halftone, shown)",
        )?
        .style()?;
        if style.is_empty() {
            return Err(
                "A look names at least one of pen, pattern, colour, halftone and shown.".to_owned(),
            );
        }
        let kind = world
            .categories()
            .get(category)
            .map_or(CategoryKind::Model, |found| found.kind);
        style
            .check(world.line_styles(), kind)
            .map_err(|e| e.to_string())?;
        // What the person names now goes over what the element already has in this view.
        let merged = view
            .overrides()
            .get(&element)
            .map_or_else(|| style.clone(), |before| style.or(before));
        Ok(view
            .clone()
            .with_overrides(view.overrides().with(&element, merged)))
    })();
    match made {
        Ok(changed) => put_changed(run, &world, (hash, view), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn override_clear<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = view
        .clone()
        .with_overrides(view.overrides().without(args.element.as_ref()));
    put_changed(run, &world, (hash, view), changed).await
}

/// The definition of the parameter a filter reads, among those the category has, named by its ID
/// or by its name (case aside).
fn parameter_of<'a>(
    context: &'a Context,
    world: &World,
    category: CategoryId,
    text: &str,
) -> Result<&'a Definition, String> {
    let wanted = text.trim().to_lowercase();
    context
        .table
        .parameters_of(world.categories(), category)
        .into_iter()
        .map(|applicable| applicable.definition)
        .find(|definition| {
            definition.id().as_str() == text.trim() || definition.name().to_lowercase() == wanted
        })
        .ok_or_else(|| format!("{text:?} is not a parameter of this category."))
}

/// The filter a command asks for: the parts it names over `base`, checked against the category
/// and the parameter's kind.
fn built(
    base: Option<&Filter>,
    args: &Args,
    world: &World,
    context: &Context,
) -> Result<Filter, String> {
    let category = if args.category.is_some() {
        category_of(args, world)?
    } else {
        base.map(|filter| filter.category)
            .ok_or_else(|| "This command needs a category.".to_owned())?
    };
    let found = world
        .categories()
        .get(category)
        .ok_or_else(|| ViewError::UnknownCategory(category).to_string())?;
    if found.id == VIEWS || found.parent == Some(VIEWS) {
        return Err(ViewError::NotStyled(category).to_string());
    }
    let name = args
        .name
        .clone()
        .or_else(|| base.map(|filter| filter.name.clone()))
        .ok_or_else(|| "This command needs a name.".to_owned())?;
    let named = match (&args.parameter, base) {
        (Some(text), _) => Some(text.clone()),
        (None, Some(filter)) => Some(filter.parameter.to_string()),
        (None, None) => None,
    };
    let definition = parameter_of(
        context,
        world,
        category,
        &named.ok_or_else(|| "This command needs a parameter.".to_owned())?,
    )?;
    let op = match (&args.op, base) {
        (Some(key), _) => Operator::from_key(key).ok_or_else(|| {
            let keys: Vec<&str> = Operator::ALL.iter().map(|op| op.key()).collect();
            format!("{key:?} is not an operator: {}.", keys.join(", "))
        })?,
        (None, Some(filter)) => filter.op,
        (None, None) => return Err("This command needs an operator (op).".to_owned()),
    };
    if !op.fits(definition.kind()) {
        return Err(ViewError::OperatorKind {
            op: op.key().to_owned(),
            parameter: definition.name().to_owned(),
        }
        .to_string());
    }
    let same_parameter = base.is_some_and(|filter| filter.parameter == *definition.id());
    let value = match (&args.value, base) {
        (Some(text), _) => definition
            .parse(text, &crate::cloud::views::units())
            .map_err(|e| e.to_string())?,
        (None, Some(filter)) if same_parameter => filter.value.clone(),
        _ => return Err("This command needs a value.".to_owned()),
    };
    let style = match (&args.appearance, base) {
        (Some(look), _) => look.style()?,
        (None, Some(filter)) => filter.style.clone(),
        (None, None) => ElementStyle::default(),
    };
    style
        .check(world.line_styles(), found.kind)
        .map_err(|e| e.to_string())?;
    Ok(Filter {
        id: base.map_or(0, |filter| filter.id),
        name,
        category,
        parameter: definition.id().clone(),
        op,
        value,
        style,
    })
}

/// The filters after a filter command.
fn filters_changed(
    cmd: &str,
    args: &Args,
    world: &World,
    context: &Context,
    filters: &Filters,
) -> Result<Filters, String> {
    let number = || need(args.filter, "a filter (its number)");
    let changed = match cmd {
        "view.filter.add" => {
            let filter = built(None, args, world, context)?;
            filters.added(filter).map_err(|e| e.to_string())?.0
        }
        "view.filter.set" => {
            let number = number()?;
            let base = filters
                .get(number)
                .ok_or_else(|| ViewError::UnknownFilter(number).to_string())?;
            let filter = built(Some(base), args, world, context)?;
            filters.replaced(filter).map_err(|e| e.to_string())?
        }
        "view.filter.remove" => filters.removed(number()?).map_err(|e| e.to_string())?,
        _ => {
            let place = need(args.place, "a place in the list (0 is the first)")?;
            filters.moved(number()?, place).map_err(|e| e.to_string())?
        }
    };
    changed
        .check(world.categories())
        .map_err(|e| e.to_string())?;
    Ok(changed)
}

async fn filter<T: Txn>(run: &mut Run<'_, T>, cmd: &str, args: Args) -> Answer {
    let (world, owner) = match owner(run, &args).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let context = match records::read_context(run.txn, run.project, run.document).await? {
        Ok(context) => context,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let done = (|| -> Result<Settings, String> {
        owner.check_free(&world, Setting::Filters)?;
        let mut settings = owner.settings();
        settings.filters = filters_changed(cmd, &args, &world, &context, &settings.filters)?;
        Ok(settings)
    })();
    match done {
        Ok(settings) => put_settings(run, &world, owner, settings).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn create<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let id = match need(args.template.clone(), "a template (the new ID)") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let made = (|| -> Result<ViewTemplate, String> {
        let name = need(args.name.as_deref(), "a name")?;
        let settings = match &args.source {
            Some(source) => world
                .views()
                .get(source)
                .ok_or_else(|| ViewError::UnknownView(source.clone()).to_string())?
                .settings(),
            None => Settings {
                scale: Scale::new(FRESH_SCALE).map_err(|e| e.to_string())?,
                detail: FRESH_DETAIL,
                graphics: construct_views::Graphics::new(),
                hidden: std::collections::BTreeSet::new(),
                filters: Filters::new(),
            },
        };
        ViewTemplate::new(
            id.clone(),
            name,
            settings,
            Setting::ALL.into_iter().collect(),
        )
        .map_err(|e| e.to_string())
    })();
    let template = match made {
        Ok(template) => template,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(template.id()).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut templates = world.templates().clone();
    if let Err(e) = templates.add(template.clone()) {
        return Ok(Err(e.to_string()));
    }
    run.put_claimed(&template.to_record(run.document), None)
        .await
}

async fn named<T: Txn>(
    run: &mut Run<'_, T>,
    args: &Args,
) -> Result<Result<(World, Id, ViewTemplate), String>, StoreError> {
    let id = match need(args.template.clone(), "a template") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let world = match run.world().await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(match template_of(run, &id).await? {
        Ok((hash, template)) => Ok((world, hash, template)),
        Err(refusal) => Err(refusal),
    })
}

async fn rename<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, template) = match named(run, &args).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let renamed = need(args.name.as_deref(), "a name")
        .and_then(|name| template.clone().renamed(name).map_err(|e| e.to_string()));
    match renamed {
        Ok(changed) => put_template(run, &world, (hash, template), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, template) = match named(run, &args).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let tied = world
        .templates()
        .tied_views(template.id(), world.views())
        .len();
    if tied > 0 {
        return Ok(Err(ViewError::TemplateInUse {
            template: template.name().to_owned(),
            views: tied,
        }
        .to_string()));
    }
    let id = template.id().clone();
    if let Err(refusal) = run.claim(&id).await? {
        return Ok(Err(refusal));
    }
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

async fn set<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, template) = match named(run, &args).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = (|| -> Result<ViewTemplate, String> {
        let mut settings = template.settings().clone();
        if let Some(n) = args.scale {
            settings.scale = crate::cloud::views::scale_of(n)?;
        }
        if let Some(key) = &args.detail {
            settings.detail = crate::cloud::views::detail_of(key)?;
        }
        let mut included = template.included().clone();
        for (key, on) in args.included.iter().flatten() {
            let setting = Setting::from_key(key).ok_or_else(|| {
                let keys: Vec<&str> = Setting::ALL.iter().map(|s| s.key()).collect();
                format!("{key:?} is not a setting: {}.", keys.join(", "))
            })?;
            if *on {
                included.insert(setting);
            } else {
                included.remove(&setting);
            }
        }
        Ok(template
            .clone()
            .with_settings(settings)
            .with_included(included))
    })();
    match changed {
        Ok(changed) => put_template(run, &world, (hash, template), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn update<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, template) = match named(run, &args).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = need(
        args.source.as_ref(),
        "a view to take the settings from (source)",
    )
    .and_then(|source| {
        world
            .views()
            .get(source)
            .ok_or_else(|| ViewError::UnknownView(source.clone()).to_string())
    })
    .map(|view| template.clone().with_settings(view.settings()));
    match changed {
        Ok(changed) => put_template(run, &world, (hash, template), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The view a tie, untie or apply is about, and the template it names, if it names one.
async fn view_and_template<T: Txn>(
    run: &mut Run<'_, T>,
    args: &Args,
    wants_template: bool,
) -> Result<Result<(World, Id, View, Option<ViewTemplate>), String>, StoreError> {
    let (world, hash, view) = match subject(run, args.view.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if !wants_template {
        return Ok(Ok((world, hash, view, None)));
    }
    let id = match need(args.template.clone(), "a template") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    Ok(match template_of(run, &id).await? {
        Ok((_, template)) => Ok((world, hash, view, Some(template))),
        Err(refusal) => Err(refusal),
    })
}

async fn tie<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view, template) = match view_and_template(run, &args, true).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some(template) = template else {
        return Ok(Err("This command needs a template.".to_owned()));
    };
    if let Err(e) = ViewTemplates::can_tie(&view) {
        return Ok(Err(e.to_string()));
    }
    // The record holds what the view shows, so untying it later changes nothing it shows.
    let changed = template
        .apply_to(&view)
        .with_template(Some(template.id().clone()));
    put_changed(run, &world, (hash, view), changed).await
}

async fn untie<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view, _) = match view_and_template(run, &args, false).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if view.template().is_none() {
        return Ok(Err(format!(
            "{} is not tied to a view template.",
            view.name()
        )));
    }
    // It keeps what it was showing: the settings the template gave it.
    let shown = world
        .views()
        .get(view.id())
        .map_or_else(|| view.settings(), View::settings);
    let changed = view.clone().with_settings(shown).with_template(None);
    put_changed(run, &world, (hash, view), changed).await
}

async fn apply_once<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (world, hash, view, template) = match view_and_template(run, &args, true).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some(template) = template else {
        return Ok(Err("This command needs a template.".to_owned()));
    };
    if let Err(e) = ViewTemplates::can_tie(&view) {
        return Ok(Err(e.to_string()));
    }
    if let Some(held) = view.template().and_then(|id| world.templates().get(id)) {
        return Ok(Err(format!(
            "{} follows the view template {}: untie it before applying another.",
            view.name(),
            held.name()
        )));
    }
    let changed = template.copy_into(&view);
    put_changed(run, &world, (hash, view), changed).await
}

/// A view template as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TemplateSummary {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The `n` of 1:n it holds.
    pub scale: u32,
    /// `coarse`, `medium` or `fine`.
    pub detail: String,
    /// The settings it includes, by key (`scale`, `detail`, `graphics`, `categories`, `filters`).
    pub included: Vec<&'static str>,
    /// The categories it does not draw, by number.
    pub hidden: Vec<u32>,
    /// How many graphics rules it holds.
    pub rules: usize,
    /// How many filters it holds.
    pub filters: usize,
    /// The IDs of the views tied to it, by the views' names.
    pub views: Vec<String>,
}

/// The summary of one template.
#[must_use]
pub fn template_summary(world: &World, template: &ViewTemplate) -> TemplateSummary {
    let settings = template.settings();
    TemplateSummary {
        id: template.id().to_string(),
        name: template.name().to_owned(),
        scale: settings.scale.denominator(),
        detail: settings.detail.key().to_owned(),
        included: Setting::ALL
            .into_iter()
            .filter(|setting| template.includes(*setting))
            .map(Setting::key)
            .collect(),
        hidden: settings.hidden.iter().map(|id| id.0).collect(),
        rules: settings.graphics.rules().count(),
        filters: settings.filters.len(),
        views: world
            .templates()
            .tied_views(template.id(), world.views())
            .into_iter()
            .map(|view| view.id().to_string())
            .collect(),
    }
}

/// Every template of the world, by name (case aside), then by ID.
#[must_use]
pub fn template_summaries(world: &World) -> Vec<TemplateSummary> {
    world
        .templates()
        .ordered()
        .into_iter()
        .map(|template| template_summary(world, template))
        .collect()
}

/// The parameter a filter may read, as a page offers it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ParameterSummary {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its kind, as it is stored (`text`, `length`...).
    pub kind: &'static str,
    /// The operators it can be given.
    pub operators: Vec<&'static str>,
    /// The categories it is bound to, by number.
    pub categories: Vec<u32>,
}

/// One filter of a list, as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FilterSummary {
    /// Its number in the list.
    pub id: u32,
    /// Its name.
    pub name: String,
    /// The category it applies to.
    pub category: u32,
    /// That category's name.
    pub category_name: String,
    /// The parameter it reads.
    pub parameter: String,
    /// That parameter's name, or its ID when the parameter is gone.
    pub parameter_name: String,
    /// The operator's key.
    pub op: &'static str,
    /// The value it compares with, as shown.
    pub value: String,
    /// How what matches looks.
    pub look: ElementStyle,
}

/// The filters of a view or a template, in the order that decides which wins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FilterListing {
    /// `view` or `template`.
    pub of: &'static str,
    /// The view's or the template's ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Whether the owner is a view tied to a template that includes filters: they are then the
    /// template's to change.
    pub follows_template: Option<String>,
    /// The filters.
    pub filters: Vec<FilterSummary>,
    /// Every parameter a filter may read.
    pub parameters: Vec<ParameterSummary>,
    /// The operators there are.
    pub operators: Vec<&'static str>,
}

fn filter_listing(
    world: &World,
    context: &Context,
    of: &'static str,
    id: &Id,
    name: &str,
    follows_template: Option<String>,
    filters: &Filters,
) -> FilterListing {
    let units = crate::cloud::views::units();
    let summaries = filters
        .iter()
        .map(|filter| {
            let definition = context.table.definition(&filter.parameter);
            FilterSummary {
                id: filter.id,
                name: filter.name.clone(),
                category: filter.category.0,
                category_name: world
                    .categories()
                    .get(filter.category)
                    .map_or_else(String::new, |found| found.name),
                parameter: filter.parameter.to_string(),
                parameter_name: definition.map_or_else(
                    || filter.parameter.to_string(),
                    |found| found.name().to_owned(),
                ),
                op: filter.op.key(),
                value: definition
                    .and_then(|found| found.format(&filter.value, &units).ok())
                    .unwrap_or_else(|| filter.value.stored()),
                look: filter.style.clone(),
            }
        })
        .collect();
    let parameters = context
        .table
        .definitions()
        .into_iter()
        .map(|definition| ParameterSummary {
            id: definition.id().to_string(),
            name: definition.name().to_owned(),
            kind: definition.kind().tag(),
            operators: Operator::ALL
                .into_iter()
                .filter(|op| op.fits(definition.kind()))
                .map(Operator::key)
                .collect(),
            categories: context
                .table
                .binding(definition.id())
                .map(|(_, categories)| categories.iter().map(|c| c.0).collect())
                .unwrap_or_default(),
        })
        .collect();
    FilterListing {
        of,
        id: id.to_string(),
        name: name.to_owned(),
        follows_template,
        filters: summaries,
        parameters,
        operators: Operator::ALL.into_iter().map(Operator::key).collect(),
    }
}

/// The filters of the view or the template `id` (the world says which it is), as of the reader.
/// `None` when the document has neither.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is a record that cannot be read, in
/// words.
pub async fn filters_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    id: &Id,
) -> Result<Result<Option<FilterListing>, String>, StoreError> {
    let world = match crate::cloud::views::read_world(reader, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    let context = match records::read_context(reader, project, document).await? {
        Ok(context) => context,
        Err(why) => return Ok(Err(why)),
    };
    if let Some(view) = world.views().get(id) {
        let follows = view
            .template()
            .and_then(|template| world.templates().get(template))
            .filter(|template| template.includes(Setting::Filters))
            .map(|template| template.name().to_owned());
        return Ok(Ok(Some(filter_listing(
            &world,
            &context,
            "view",
            id,
            view.name(),
            follows,
            view.filters(),
        ))));
    }
    Ok(Ok(world.templates().get(id).map(|template| {
        filter_listing(
            &world,
            &context,
            "template",
            id,
            template.name(),
            None,
            &template.settings().filters,
        )
    })))
}

/// One element's override in a view, as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OverrideSummary {
    /// The element's ID.
    pub element: String,
    /// Its name, when it is still in the model.
    pub name: Option<String>,
    /// What the view does to its look.
    pub look: ElementStyle,
}

/// A view's overrides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OverridesListing {
    /// The view.
    pub view: crate::cloud::views::ViewSummary,
    /// Its overrides, by element ID.
    pub overrides: Vec<OverrideSummary>,
}

/// The overrides of the view `id`: `None` when the document has no such view.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is a record that cannot be read, in
/// words.
pub async fn overrides_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    id: &Id,
) -> Result<Result<Option<OverridesListing>, String>, StoreError> {
    let world = match crate::cloud::views::read_world(reader, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    let Some(view) = world.views().get(id) else {
        return Ok(Ok(None));
    };
    let mut overrides = Vec::new();
    for (element, look) in view.overrides().iter() {
        let name = records::read_element(reader, project, document, element)
            .await?
            .map(|(_, record)| record.name);
        overrides.push(OverrideSummary {
            element: element.to_string(),
            name,
            look: look.clone(),
        });
    }
    Ok(Ok(Some(OverridesListing {
        view: crate::cloud::views::summary(view, world.levels()),
        overrides,
    })))
}

/// The style each of `elements` has in the view `id` from its filters and its own override, put
/// together: the override wins any part it names and the filters give the rest. An element with
/// neither is left out. The filters read each element's parameters now, through the same reading a
/// shape and the properties panel use, so a parameter that changed has changed the answer.
///
/// # Errors
/// A [`StoreError`] when the store fails; the inner error is a record that cannot be read, in
/// words.
pub async fn element_styles<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    world: &World,
    id: &Id,
    elements: &[Element],
) -> Result<Result<BTreeMap<Id, ElementStyle>, String>, StoreError> {
    let mut found = BTreeMap::new();
    let Some(view) = world.views().get(id) else {
        return Ok(Ok(found));
    };
    if view.filters().is_empty() && view.overrides().is_empty() {
        return Ok(Ok(found));
    }
    let context = match records::read_context(reader, project, document).await? {
        Ok(context) => context,
        Err(why) => return Ok(Err(why)),
    };
    for record in elements {
        let Some(category) = category_of_record(record) else {
            continue;
        };
        let from_filters = if view.filters().is_empty() {
            ElementStyle::default()
        } else {
            let ty = match records::type_of(record) {
                Some(type_id) => records::read_type(reader, project, document, &type_id, &context)
                    .await?
                    .ok(),
                None => None,
            };
            let read = |parameter: &Id| {
                context
                    .value_of(record, ty.as_ref(), parameter)
                    .ok()
                    .flatten()
            };
            view.filters()
                .style_for(world.categories(), category, &read)
        };
        let own = view
            .overrides()
            .get(&record.id)
            .cloned()
            .unwrap_or_default();
        let style = own.or(&from_filters);
        if !style.is_empty() {
            found.insert(record.id.clone(), style);
        }
    }
    Ok(Ok(found))
}
