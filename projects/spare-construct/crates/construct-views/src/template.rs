//! View templates (CON-A8-F3-P4): a saved set of view settings that many views follow.
//!
//! A [`ViewTemplate`] is its own record (kind [`TEMPLATE_KIND`], of the views category, with no
//! shape). It holds the five [`Settings`] a view has (scale, detail level, graphics rules by
//! category, which categories show, filters) and, for each, an "included" flag ([`Setting`]): a
//! setting that is not included is the view's own business and the template says nothing of it.
//!
//! Two ways to use one, and they are different on purpose:
//!
//! - **Tied.** A view names a template ([`View::template`]) and shows the template's included
//!   settings in place of its own: [`ViewTemplate::apply_to`] is the one function that makes that
//!   so, and [`ViewTemplates::effective`] applies it to every tied view of a document, so a view
//!   changes when its template changes and nothing is rewritten. A tied view cannot change an
//!   included setting by itself: [`ViewTemplates::tied_refusal`] is the refusal, in words, that
//!   every command that would change one asks for. Settings the template does not include, and a
//!   view's overrides (CON-A8-F3-P6), stay the view's own.
//! - **Applied once.** [`ViewTemplate::copy_into`] copies the included settings into a view that
//!   stays untied: afterwards the view is free to change them and the template and the view part.
//!
//! A template applies to the views that show a model (plans, sections, elevations, three
//! dimensional views); a drafting view or a legend takes none ([`ViewTemplates::can_tie`]). Where a
//! view's kind cannot hold part of a template's graphics (a cut rule in a three-dimensional view),
//! that part is left out of what the view shows, so one template serves every kind of model view.
//!
//! Interim: view range and crop region are not settings yet; a plan's range and crop belong to the
//! plan until the jobs that add them to a template do (piece A79 adds crops to construct-views).
//!
//! Stored form (FORMAT.md): the element record's `props` hold `@category` (`7000`), `@vt.scale`
//! and `@vt.detail` (the view's own spelling), `@vt.included` (the keys of the included settings,
//! in [`Setting::ALL`] order, separated by commas, `""` for none) and the keys the view uses for
//! the rest: `@view.graphics`, `@view.hidden` and `@view.filters`, each written only when it has
//! something. A record that is not exactly as written is refused, never repaired.

use crate::error::ViewError;
use crate::filters::{FILTERS_KEY, Filters};
use crate::graphics::{GRAPHICS_KEY, Graphics};
use crate::kind::DetailLevel;
use crate::settings::{HIDDEN_KEY, Setting, Settings, hidden_from_text, hidden_to_text};
use crate::view::{CATEGORY_KEY, View};
use crate::views::Views;
use construct_categories::VIEWS;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::Value;
use construct_styles::Scale;
use kit_core::Id;
use std::collections::{BTreeMap, BTreeSet};

/// The `kind` of every view template record, and its recipe's operation.
pub const TEMPLATE_KIND: &str = "view_template";
/// The scale of a template, in the spelling a view's scale has.
pub const TEMPLATE_SCALE_KEY: &str = "@vt.scale";
/// The detail level of a template, in the spelling a view's detail level has.
pub const TEMPLATE_DETAIL_KEY: &str = "@vt.detail";
/// The settings a template includes, as keys joined by commas.
pub const TEMPLATE_INCLUDED_KEY: &str = "@vt.included";

const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a template record: Spare Construct's own operation `view_template`, version 1,
/// with no arguments. It builds no shape.
#[must_use]
pub fn template_recipe() -> Recipe {
    Recipe::construct(TEMPLATE_KIND, BTreeMap::new())
}

/// A template name as kept: trimmed, and never empty.
fn clean_name(name: &str) -> Result<String, ViewError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ViewError::EmptyTemplateName);
    }
    Ok(name.to_owned())
}

/// A view template: a name, the five settings and which of them it includes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewTemplate {
    id: Id,
    name: String,
    settings: Settings,
    included: BTreeSet<Setting>,
}

impl ViewTemplate {
    /// A template of these settings with the ones in `included` switched on.
    ///
    /// # Errors
    /// [`ViewError::EmptyTemplateName`] if the name is only spaces.
    pub fn new(
        id: Id,
        name: &str,
        settings: Settings,
        included: BTreeSet<Setting>,
    ) -> Result<Self, ViewError> {
        Ok(Self {
            id,
            name: clean_name(name)?,
            settings,
            included,
        })
    }

    /// A template made from a view: its five settings, every one included.
    ///
    /// # Errors
    /// [`ViewError::EmptyTemplateName`] if the name is only spaces.
    pub fn from_view(id: Id, name: &str, view: &View) -> Result<Self, ViewError> {
        Self::new(
            id,
            name,
            view.settings(),
            Setting::ALL.into_iter().collect(),
        )
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Its name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its settings, whether included or not.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The settings it includes.
    #[must_use]
    pub fn included(&self) -> &BTreeSet<Setting> {
        &self.included
    }

    /// Whether it includes this setting.
    #[must_use]
    pub fn includes(&self, setting: Setting) -> bool {
        self.included.contains(&setting)
    }

    /// The template with another name.
    ///
    /// # Errors
    /// [`ViewError::EmptyTemplateName`] if the name is only spaces.
    pub fn renamed(mut self, name: &str) -> Result<Self, ViewError> {
        self.name = clean_name(name)?;
        Ok(self)
    }

    /// The template with other settings.
    #[must_use]
    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// The template with other settings included.
    #[must_use]
    pub fn with_included(mut self, included: BTreeSet<Setting>) -> Self {
        self.included = included;
        self
    }

    /// What `view` shows when it is tied to this template: its own settings with the included
    /// ones taken from the template, the graphics as the view's kind can hold them. The view's
    /// name, kind, staging, overrides and the settings not included are its own.
    #[must_use]
    pub fn apply_to(&self, view: &View) -> View {
        let fitted = self.settings.fit_to(view.kind());
        view.clone()
            .with_settings(view.settings().taking(&fitted, &self.included))
    }

    /// `view` with the included settings copied in, left as free as it was: tied to nothing new.
    #[must_use]
    pub fn copy_into(&self, view: &View) -> View {
        self.apply_to(view)
    }

    /// The template as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), VIEWS.0.to_string());
        props.insert(
            TEMPLATE_SCALE_KEY.to_owned(),
            Value::Whole(i64::from(self.settings.scale.denominator())).stored(),
        );
        props.insert(
            TEMPLATE_DETAIL_KEY.to_owned(),
            Value::Choice(self.settings.detail.key().to_owned()).stored(),
        );
        props.insert(
            TEMPLATE_INCLUDED_KEY.to_owned(),
            Setting::ALL
                .into_iter()
                .filter(|setting| self.included.contains(setting))
                .map(Setting::key)
                .collect::<Vec<_>>()
                .join(","),
        );
        if let Some(text) = self.settings.graphics.to_text() {
            props.insert(GRAPHICS_KEY.to_owned(), text);
        }
        if let Some(text) = hidden_to_text(&self.settings.hidden) {
            props.insert(HIDDEN_KEY.to_owned(), text);
        }
        if let Some(text) = self.settings.filters.to_text() {
            props.insert(FILTERS_KEY.to_owned(), text);
        }
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: TEMPLATE_KIND.to_owned(),
            name: self.name.clone(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: template_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a template from its element record. Anything [`ViewTemplate::to_record`] would not
    /// write is refused, never repaired.
    ///
    /// # Errors
    /// [`ViewError::Malformed`], naming the record, for a record that is not a template as
    /// written.
    pub fn from_record(record: &Element) -> Result<Self, ViewError> {
        let malformed = |why: &str| {
            ViewError::Malformed(format!("The view template record {} {why}.", record.id))
        };
        if record.kind != TEMPLATE_KIND || record.recipe != template_recipe() {
            return Err(malformed(
                "is not of kind view_template, with the view template recipe",
            ));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
            return Err(malformed("has a level, a placement or references"));
        }
        let stored = |key: &str| {
            let text = record.props.get(key)?;
            Value::from_stored(text).filter(|value| value.stored() == *text)
        };
        let Some(Value::Whole(scale)) = stored(TEMPLATE_SCALE_KEY) else {
            return Err(malformed("holds no scale in its one spelling"));
        };
        let Some(scale) = u32::try_from(scale).ok().and_then(|n| Scale::new(n).ok()) else {
            return Err(malformed(
                "holds a scale that is not 1:n with n from 1 to 1,000,000",
            ));
        };
        let Some(Value::Choice(detail)) = stored(TEMPLATE_DETAIL_KEY) else {
            return Err(malformed("holds no detail level in its one spelling"));
        };
        let Some(detail) = DetailLevel::from_key(&detail) else {
            return Err(malformed(
                "holds a detail level that is not coarse, medium or fine",
            ));
        };
        let Some(included_text) = record.props.get(TEMPLATE_INCLUDED_KEY) else {
            return Err(malformed("holds no list of included settings"));
        };
        let mut included = BTreeSet::new();
        if !included_text.is_empty() {
            for key in included_text.split(',') {
                let setting = Setting::from_key(key)
                    .ok_or_else(|| malformed("includes a setting that is not one of ours"))?;
                included.insert(setting);
            }
        }
        let graphics = match record.props.get(GRAPHICS_KEY) {
            None => Graphics::new(),
            Some(text) => Graphics::from_text(text).map_err(|why| malformed(&why))?,
        };
        let hidden = match record.props.get(HIDDEN_KEY) {
            None => BTreeSet::new(),
            Some(text) => hidden_from_text(text).map_err(|why| malformed(&why))?,
        };
        let filters = match record.props.get(FILTERS_KEY) {
            None => Filters::new(),
            Some(text) => Filters::from_text(text).map_err(|why| malformed(&why))?,
        };
        let settings = Settings {
            scale,
            detail,
            graphics,
            hidden,
            filters,
        };
        let template = Self::new(record.id.clone(), &record.name, settings, included)
            .map_err(|_| malformed("has no name"))?;
        if template.to_record(&record.document) != *record {
            return Err(malformed("is not as a view template record is written"));
        }
        Ok(template)
    }
}

/// A document's view templates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewTemplates {
    by_id: BTreeMap<Id, ViewTemplate>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl ViewTemplates {
    /// No templates.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A document's templates from its records: every record of kind view template.
    ///
    /// # Errors
    /// [`ViewError::Malformed`] for a template record that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, ViewError> {
        let mut templates = Self::new();
        for record in records
            .into_iter()
            .filter(|record| record.kind == TEMPLATE_KIND)
        {
            let template = ViewTemplate::from_record(record)?;
            templates.by_id.insert(template.id().clone(), template);
        }
        Ok(templates)
    }

    /// The template of this ID.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&ViewTemplate> {
        self.by_id.get(id)
    }

    /// How many there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Every template, by name without regard to case, then by ID.
    #[must_use]
    pub fn ordered(&self) -> Vec<&ViewTemplate> {
        let mut all: Vec<&ViewTemplate> = self.by_id.values().collect();
        all.sort_by(|a, b| {
            a.name()
                .to_lowercase()
                .cmp(&b.name().to_lowercase())
                .then_with(|| a.id().cmp(b.id()))
        });
        all
    }

    fn name_free(&self, name: &str, except: &Id) -> Result<(), ViewError> {
        if self
            .by_id
            .values()
            .any(|template| template.id() != except && same_name(template.name(), name))
        {
            return Err(ViewError::DuplicateTemplateName(name.to_owned()));
        }
        Ok(())
    }

    /// Adds a template.
    ///
    /// # Errors
    /// [`ViewError::DuplicateTemplate`] for an ID one has, [`ViewError::DuplicateTemplateName`]
    /// for a name one has (case aside).
    pub fn add(&mut self, template: ViewTemplate) -> Result<(), ViewError> {
        if self.by_id.contains_key(template.id()) {
            return Err(ViewError::DuplicateTemplate(template.id().clone()));
        }
        self.name_free(template.name(), template.id())?;
        self.by_id.insert(template.id().clone(), template);
        Ok(())
    }

    /// Puts a changed template in the place of the one of its ID.
    ///
    /// # Errors
    /// [`ViewError::UnknownTemplate`] if none has its ID, [`ViewError::DuplicateTemplateName`] for
    /// a name another has.
    pub fn replace(&mut self, template: ViewTemplate) -> Result<(), ViewError> {
        if !self.by_id.contains_key(template.id()) {
            return Err(ViewError::UnknownTemplate(template.id().clone()));
        }
        self.name_free(template.name(), template.id())?;
        self.by_id.insert(template.id().clone(), template);
        Ok(())
    }

    /// The views of `views` that are tied to the template `id`, by name.
    #[must_use]
    pub fn tied_views<'a>(&self, id: &Id, views: &'a Views) -> Vec<&'a View> {
        views
            .ordered()
            .into_iter()
            .filter(|view| view.template() == Some(id))
            .collect()
    }

    /// Asks whether `view` may be tied to a template: only a view that shows a model.
    ///
    /// # Errors
    /// [`ViewError::NoTemplateOnKind`] for a drafting view or a legend.
    pub fn can_tie(view: &View) -> Result<(), ViewError> {
        if view.kind().shows_model() {
            Ok(())
        } else {
            Err(ViewError::NoTemplateOnKind {
                kind: view.kind().key().replace('_', " "),
            })
        }
    }

    /// Refuses a change by a tied view to a setting its template includes: the view follows the
    /// template, so the change is the template's.
    ///
    /// # Errors
    /// [`ViewError::TiedSetting`], naming the view, the template and the setting.
    pub fn tied_refusal(&self, view: &View, setting: Setting) -> Result<(), ViewError> {
        let Some(template) = view.template().and_then(|id| self.get(id)) else {
            return Ok(());
        };
        if template.includes(setting) {
            return Err(ViewError::TiedSetting {
                view: view.name().to_owned(),
                template: template.name().to_owned(),
                setting: setting.words().to_owned(),
            });
        }
        Ok(())
    }

    /// The views as they show: each view tied to a template of this document with the template's
    /// included settings in place of its own, the others as they are. A tie to a template the
    /// document does not have changes nothing (it reads as the view's own settings).
    #[must_use]
    pub fn effective(&self, views: &Views) -> Views {
        views.mapped(|view| match view.template().and_then(|id| self.get(id)) {
            Some(template) => template.apply_to(view),
            None => view.clone(),
        })
    }
}
