//! One view: its permanent ID, name, scale, detail level, kind and the data of that kind, its
//! staging fields and its phase settings, written as an element record of the views category and
//! read back. How a view picks a pen's width.

use crate::cut::{CutLine, Point, Side};
use crate::error::ViewError;
use crate::kind::{DetailLevel, PlanSetup, ViewKind};
use crate::listing::Model;
use crate::range::ViewRange;
use construct_categories::VIEWS;
use construct_levels::{LEVEL_KEY, LEVEL_OFFSET_KEY, Tie, base_of, write_base};
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use construct_phases::{ElementStaging, OptionChoice, ViewSettings};
use construct_styles::{Micrometres, PenTable, Scale};
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of every view record, and its recipe's operation.
pub const VIEW_KIND: &str = "view";
/// The record's category, as every record carries it (construct-types' key).
pub const CATEGORY_KEY: &str = "@category";
/// The view's kind, in construct-params' stored choice text.
pub const KIND_KEY: &str = "@view.kind";
/// The view's scale: the `n` of 1:n, in construct-params' stored whole number text.
pub const SCALE_KEY: &str = "@view.scale";
/// The view's detail level, in construct-params' stored choice text.
pub const DETAIL_KEY: &str = "@view.detail";
/// The view's phase settings, as the phases crate's JSON, when it has any.
pub const PHASING_KEY: &str = "@view.phasing";
/// The view's own staging fields (the phase it was created and demolished in, its design option),
/// as the phases crate's JSON.
pub const STAGING_KEY: &str = "@staging";
/// A plan's range: the level its top is tied to, then the offset key beside it, and so on.
pub const RANGE_KEYS: [(&str, &str); 4] = [
    ("@view.range.top", "@view.range.top.offset"),
    ("@view.range.cut", "@view.range.cut.offset"),
    ("@view.range.bottom", "@view.range.bottom.offset"),
    ("@view.range.depth", "@view.range.depth.offset"),
];
/// A section's or elevation's line: x and y of the first end, then of the second, each a stored
/// length.
pub const LINE_KEYS: [&str; 4] = [
    "@view.line.x0",
    "@view.line.y0",
    "@view.line.x1",
    "@view.line.y1",
];
/// A section's or elevation's side, in construct-params' stored choice text.
pub const LOOK_KEY: &str = "@view.look";
/// A section's or elevation's depth, a stored length.
pub const FAR_KEY: &str = "@view.far";

/// Where a view record sits: a view has no shape, so nowhere.
const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a view record: Spare Construct's own operation `view`, version 1, with no
/// arguments. It builds no shape. Its version is the view record's format version.
#[must_use]
pub fn view_recipe() -> Recipe {
    Recipe::construct(VIEW_KIND, BTreeMap::new())
}

/// A view name as kept: trimmed, and never empty.
pub(crate) fn clean_name(name: &str) -> Result<String, ViewError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ViewError::EmptyName);
    }
    Ok(name.to_owned())
}

/// A view: an element of the views category with a name, a scale, a detail level and the data of
/// its kind. A view has no shape; what it shows is asked of [`crate::list`]. Its name is kept
/// unique in the document by [`crate::Views`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    id: Id,
    name: String,
    scale: Scale,
    detail: DetailLevel,
    kind: ViewKind,
    staging: ElementStaging,
    phasing: Option<ViewSettings>,
}

impl View {
    /// A view with no phase settings: it shows every element whatever its phase.
    ///
    /// # Errors
    /// [`ViewError::EmptyName`] if the name is only spaces.
    pub fn new(
        id: Id,
        name: &str,
        scale: Scale,
        detail: DetailLevel,
        kind: ViewKind,
        staging: ElementStaging,
    ) -> Result<Self, ViewError> {
        Ok(Self {
            id,
            name: clean_name(name)?,
            scale,
            detail,
            kind,
            staging,
            phasing: None,
        })
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

    /// Its scale, 1:n.
    #[must_use]
    pub fn scale(&self) -> Scale {
        self.scale
    }

    /// Its detail level.
    #[must_use]
    pub fn detail(&self) -> DetailLevel {
        self.detail
    }

    /// Its kind and the data of that kind.
    #[must_use]
    pub fn kind(&self) -> &ViewKind {
        &self.kind
    }

    /// The phase and design option the view itself was made in, as every element carries them.
    #[must_use]
    pub fn staging(&self) -> &ElementStaging {
        &self.staging
    }

    /// The phase it shows, its phase filter and its option choices, if it has them.
    #[must_use]
    pub fn phasing(&self) -> Option<&ViewSettings> {
        self.phasing.as_ref()
    }

    /// The view with another name.
    ///
    /// # Errors
    /// [`ViewError::EmptyName`] if the name is only spaces.
    pub fn renamed(mut self, name: &str) -> Result<Self, ViewError> {
        self.name = clean_name(name)?;
        Ok(self)
    }

    /// The view at another scale.
    #[must_use]
    pub fn with_scale(mut self, scale: Scale) -> Self {
        self.scale = scale;
        self
    }

    /// The view at another detail level.
    #[must_use]
    pub fn with_detail(mut self, detail: DetailLevel) -> Self {
        self.detail = detail;
        self
    }

    /// The view with the data of another kind.
    #[must_use]
    pub fn with_kind(mut self, kind: ViewKind) -> Self {
        self.kind = kind;
        self
    }

    /// The view with other staging fields.
    #[must_use]
    pub fn with_staging(mut self, staging: ElementStaging) -> Self {
        self.staging = staging;
        self
    }

    /// The view with phase settings, or with none. Option choices are kept in the order of their
    /// sets' IDs, so one set of choices is one byte string however it was given.
    #[must_use]
    pub fn with_phasing(mut self, phasing: Option<ViewSettings>) -> Self {
        self.phasing = phasing.map(|mut settings| {
            settings
                .choices
                .sort_by(|a: &OptionChoice, b| a.set.cmp(&b.set));
            settings
        });
        self
    }

    /// Asks whether the view stands in `model`: a plan's level and every level of its range exist
    /// and the range is in order, its own staging and its phase settings name phases and options
    /// the model has.
    ///
    /// # Errors
    /// The first refusal: [`ViewError::UnknownLevel`], [`ViewError::RangeOrder`],
    /// [`ViewError::OutOfRange`] or [`ViewError::Phase`].
    pub fn check(&self, model: &Model<'_>) -> Result<(), ViewError> {
        if let Some(plan) = self.kind.plan() {
            if model.levels.resolve(&plan.level).is_none() {
                return Err(ViewError::UnknownLevel(plan.level.clone()));
            }
            plan.range.resolve(model.levels)?;
        }
        self.staging.check(model.phases, model.options)?;
        if let Some(settings) = &self.phasing {
            if model.phases.position(&settings.phase).is_none() {
                return Err(
                    construct_phases::PhaseError::UnknownPhase(settings.phase.clone()).into(),
                );
            }
            for choice in &settings.choices {
                model.options.check(&construct_phases::OptionRef {
                    set: choice.set.clone(),
                    option: choice.option.clone(),
                })?;
            }
        }
        Ok(())
    }

    /// The printed width of pen `number` in this view: the pen table's row for the view's scale,
    /// so a pen prints thinner on a view of a smaller scale.
    ///
    /// # Errors
    /// [`ViewError::Style`] if the table has no such pen.
    pub fn pen_width(&self, pens: &PenTable, number: u16) -> Result<Micrometres, ViewError> {
        pens.width(number, self.scale)
            .map_err(|e| ViewError::Style(e.to_string()))
    }

    /// The view as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), VIEWS.0.to_string());
        props.insert(
            KIND_KEY.to_owned(),
            Value::Choice(self.kind.key().to_owned()).stored(),
        );
        props.insert(
            SCALE_KEY.to_owned(),
            Value::Whole(i64::from(self.scale.denominator())).stored(),
        );
        props.insert(
            DETAIL_KEY.to_owned(),
            Value::Choice(self.detail.key().to_owned()).stored(),
        );
        // Plain structs of IDs and strings: serialising one cannot fail, and a record that held
        // nothing here would be refused when read.
        props.insert(
            STAGING_KEY.to_owned(),
            serde_json::to_string(&self.staging).unwrap_or_default(),
        );
        if let Some(settings) = &self.phasing {
            props.insert(
                PHASING_KEY.to_owned(),
                serde_json::to_string(settings).unwrap_or_default(),
            );
        }
        if let Some(plan) = self.kind.plan() {
            write_plan(&mut props, plan);
        }
        if let Some(line) = self.kind.cut_line() {
            write_line(&mut props, line);
        }
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: VIEW_KIND.to_owned(),
            name: self.name.clone(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: view_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a view from its element record. Anything [`View::to_record`] would not write is
    /// refused, never repaired, so one view has one byte string.
    ///
    /// # Errors
    /// [`ViewError::Malformed`], naming the record, for a record that is not a view record as
    /// written.
    pub fn from_record(record: &Element) -> Result<Self, ViewError> {
        let malformed =
            |why: &str| ViewError::Malformed(format!("The view record {} {why}.", record.id));
        if record.kind != VIEW_KIND || record.recipe != view_recipe() {
            return Err(malformed("is not of kind view, with the view recipe"));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT || !record.references.is_empty() {
            return Err(malformed("has a level, a placement or references"));
        }
        let Some(Value::Choice(kind_key)) = stored(record, KIND_KEY) else {
            return Err(malformed("holds no kind in its one spelling"));
        };
        let Some(Value::Whole(scale)) = stored(record, SCALE_KEY) else {
            return Err(malformed("holds no scale in its one spelling"));
        };
        let Some(scale) = u32::try_from(scale).ok().and_then(|n| Scale::new(n).ok()) else {
            return Err(malformed(
                "holds a scale that is not 1:n with n from 1 to 1,000,000",
            ));
        };
        let Some(Value::Choice(detail)) = stored(record, DETAIL_KEY) else {
            return Err(malformed("holds no detail level in its one spelling"));
        };
        let Some(detail) = DetailLevel::from_key(&detail) else {
            return Err(malformed(
                "holds a detail level that is not coarse, medium or fine",
            ));
        };
        let Some(staging) = record
            .props
            .get(STAGING_KEY)
            .and_then(|text| serde_json::from_str::<ElementStaging>(text).ok())
        else {
            return Err(malformed("holds no staging fields"));
        };
        let phasing = match record.props.get(PHASING_KEY) {
            None => None,
            Some(text) => match serde_json::from_str::<ViewSettings>(text) {
                Ok(settings) => Some(settings),
                Err(_) => return Err(malformed("holds phase settings that do not read")),
            },
        };
        let kind = read_kind(record, &kind_key).map_err(|why| malformed(&why))?;
        let view = Self::new(
            record.id.clone(),
            &record.name,
            scale,
            detail,
            kind,
            staging,
        )
        .map_err(|_| malformed("has no name"))?
        .with_phasing(phasing);
        if view.to_record(&record.document) != *record {
            return Err(malformed("is not as a view record is written"));
        }
        Ok(view)
    }
}

/// A stored value under `key`, if it is in its one spelling.
fn stored(record: &Element, key: &str) -> Option<Value> {
    let text = record.props.get(key)?;
    Value::from_stored(text).filter(|value| value.stored() == *text)
}

/// A stored length under `key`, if it is in its one spelling.
fn length(record: &Element, key: &str) -> Option<Length> {
    match stored(record, key)? {
        Value::Length(length) => Some(length),
        _ => None,
    }
}

/// The kind a record's key names, with the data of that kind read from `props`.
fn read_kind(record: &Element, key: &str) -> Result<ViewKind, String> {
    match key {
        "floor_plan" => Ok(ViewKind::FloorPlan(read_plan(record)?)),
        "ceiling_plan" => Ok(ViewKind::CeilingPlan(read_plan(record)?)),
        "section" => Ok(ViewKind::Section(read_line(record)?)),
        "elevation" => Ok(ViewKind::Elevation(read_line(record)?)),
        "three_d" => Ok(ViewKind::ThreeD),
        "drafting" => Ok(ViewKind::Drafting),
        "legend" => Ok(ViewKind::Legend),
        _ => Err("holds a kind that is not one of ours".to_owned()),
    }
}

/// A tie under two keys, both present and in their one spelling.
fn read_tie(record: &Element, key: &str, offset_key: &str) -> Result<Tie, String> {
    let level = record
        .props
        .get(key)
        .and_then(|text| Id::parse(text).ok())
        .ok_or_else(|| format!("holds no level under {key}"))?;
    let offset = length(record, offset_key)
        .ok_or_else(|| format!("holds no offset in its one spelling under {offset_key}"))?;
    Ok(Tie { level, offset })
}

fn read_plan(record: &Element) -> Result<PlanSetup, String> {
    let level = match base_of(record) {
        Ok(Some(tie)) if tie.offset == Length::from_steps(0) => tie.level,
        _ => return Err("is a plan with no level".to_owned()),
    };
    let tie = |index: usize| -> Result<Tie, String> {
        let (key, offset_key) = RANGE_KEYS
            .get(index)
            .ok_or_else(|| "has a range of fewer than four heights".to_owned())?;
        read_tie(record, key, offset_key)
    };
    Ok(PlanSetup {
        level,
        range: ViewRange {
            top: tie(0)?,
            cut: tie(1)?,
            bottom: tie(2)?,
            depth: tie(3)?,
        },
    })
}

fn read_line(record: &Element) -> Result<CutLine, String> {
    let end = |index: usize| -> Result<Length, String> {
        let key = LINE_KEYS
            .get(index)
            .ok_or_else(|| "has a line of fewer than four numbers".to_owned())?;
        length(record, key)
            .ok_or_else(|| format!("holds no length in its one spelling under {key}"))
    };
    let Some(Value::Choice(look)) = stored(record, LOOK_KEY) else {
        return Err("holds no side to look to".to_owned());
    };
    let look = Side::from_key(&look).ok_or_else(|| "looks to neither left nor right".to_owned())?;
    let far = length(record, FAR_KEY).ok_or_else(|| "holds no depth".to_owned())?;
    let from = Point {
        x: end(0)?,
        y: end(1)?,
    };
    let to = Point {
        x: end(2)?,
        y: end(3)?,
    };
    CutLine::new(from, to, look, far).map_err(|e| e.to_string())
}

fn write_plan(props: &mut BTreeMap<String, String>, plan: &PlanSetup) {
    // The plan stands on its level as an element does: the tie construct-levels counts, so the
    // level is not deleted while a plan is of it.
    write_base(
        props,
        Some(&Tie {
            level: plan.level.clone(),
            offset: Length::from_steps(0),
        }),
    );
    let ties = [
        &plan.range.top,
        &plan.range.cut,
        &plan.range.bottom,
        &plan.range.depth,
    ];
    for ((key, offset_key), tie) in RANGE_KEYS.iter().zip(ties) {
        props.insert((*key).to_owned(), tie.level.to_string());
        props.insert((*offset_key).to_owned(), Value::Length(tie.offset).stored());
    }
}

fn write_line(props: &mut BTreeMap<String, String>, line: &CutLine) {
    let ends = [line.from().x, line.from().y, line.to().x, line.to().y];
    for (key, end) in LINE_KEYS.iter().zip(ends) {
        props.insert((*key).to_owned(), Value::Length(end).stored());
    }
    props.insert(
        LOOK_KEY.to_owned(),
        Value::Choice(line.look().key().to_owned()).stored(),
    );
    props.insert(FAR_KEY.to_owned(), Value::Length(line.far()).stored());
}

/// The keys of a plan's level tie, which are construct-levels' own.
pub const PLAN_LEVEL_KEYS: [&str; 2] = [LEVEL_KEY, LEVEL_OFFSET_KEY];
