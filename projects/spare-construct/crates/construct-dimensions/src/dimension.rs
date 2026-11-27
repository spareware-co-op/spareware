//! One dimension: its ID, the view it is in, how it lies, its references, its equalities, its
//! style and its overrides, written as an element record of the dimensions category and read back
//! (FORMAT.md beside this crate).
//!
//! A dimension holds no value. Its value is worked out from its references each time it is asked
//! ([`crate::measure`]), so it is never out of date.

use crate::error::DimensionError;
use crate::layout::{Direction, Layout, Ray};
use crate::overrides::Overrides;
use crate::reference::Reference;
use construct_categories::DIMENSIONS;
use construct_grids::Point;
use construct_model::element::{Element, Placement};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of every dimension record, and its recipe's operation.
pub const DIMENSION_KIND: &str = "dimension";
/// The name every dimension record carries.
pub const DIMENSION_NAME: &str = "Dimension";
/// The record's category.
pub const CATEGORY_KEY: &str = "@category";
/// The view the dimension is in, an ID.
pub const VIEW_KEY: &str = "@dimension.view";
/// How it lies: `choice:horizontal`, `choice:vertical` or `choice:aligned`.
pub const LAYOUT_KEY: &str = "@dimension.layout";
/// The y of a horizontal dimension's offset line, a stored length.
pub const Y_KEY: &str = "@dimension.y";
/// The x of a vertical dimension's offset line, a stored length.
pub const X_KEY: &str = "@dimension.x";
/// The x of the point an aligned dimension's offset line goes through, a stored length.
pub const THROUGH_X_KEY: &str = "@dimension.through.x";
/// The y of that point.
pub const THROUGH_Y_KEY: &str = "@dimension.through.y";
/// The x of an aligned dimension's direction, a stored length.
pub const DIRECTION_X_KEY: &str = "@dimension.direction.x";
/// The y of that direction.
pub const DIRECTION_Y_KEY: &str = "@dimension.direction.y";
/// The equalities, as stored text: groups of segment numbers, `0,1;3,4,5`.
pub const EQUAL_KEY: &str = "@dimension.equal";
/// An angular dimension's arc radius, a stored length.
pub const RADIUS_KEY: &str = "@dimension.radius";
/// Which way along its first line an angular dimension opens: `choice:forward` or
/// `choice:backward`.
pub const FIRST_KEY: &str = "@dimension.first";
/// Which way along its second line.
pub const SECOND_KEY: &str = "@dimension.second";
/// The x of the way a radial or diameter dimension's line runs, a stored length.
pub const TOWARD_X_KEY: &str = "@dimension.toward.x";
/// The y of that way.
pub const TOWARD_Y_KEY: &str = "@dimension.toward.y";
/// How far an arc length dimension's arc is outside the arc it measures, a stored length.
pub const OFFSET_KEY: &str = "@dimension.offset";
/// `yesno:yes` on an ordinate dimension: its values are read from its first reference.
pub const ORDINATE_KEY: &str = "@dimension.ordinate";
/// The ID of its style; none: the first built-in style.
pub const STYLE_KEY: &str = "@dimension.style";
/// A text shown in place of its values.
pub const TEXT_KEY: &str = "@dimension.text";
/// A text shown before its values.
pub const PREFIX_KEY: &str = "@dimension.prefix";
/// A text shown after its values.
pub const SUFFIX_KEY: &str = "@dimension.suffix";
/// The most references a dimension string holds.
pub const MAX_REFERENCES: usize = 64;

pub(crate) const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a dimension record: Spare Construct's own operation `dimension`, version 1, with
/// no arguments. It builds no shape.
#[must_use]
pub fn dimension_recipe() -> Recipe {
    Recipe::construct(DIMENSION_KIND, BTreeMap::new())
}

/// A set of segments that are to be equal: their numbers, from 0, ascending, at least two.
pub type Equality = Vec<usize>;

/// What a dimension measures, by its layout and whether it is an ordinate one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The distances between neighbouring references along an offset line.
    Linear,
    /// The distance of each reference from the first along an offset line.
    Ordinate,
    /// The angle between two lines.
    Angular,
    /// An arc's radius.
    Radial,
    /// An arc's diameter.
    Diameter,
    /// An arc's length.
    ArcLength,
}

impl Kind {
    /// The key it is named by in commands and listings.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Ordinate => "ordinate",
            Self::Angular => "angular",
            Self::Radial => "radial",
            Self::Diameter => "diameter",
            Self::ArcLength => "arc_length",
        }
    }

    /// It in a sentence: `An angular dimension`.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Self::Linear => "A linear dimension",
            Self::Ordinate => "An ordinate dimension",
            Self::Angular => "An angular dimension",
            Self::Radial => "A radial dimension",
            Self::Diameter => "A diameter dimension",
            Self::ArcLength => "An arc length dimension",
        }
    }
}

/// A dimension: a string of segments between references, an angle, or an arc's radius, diameter
/// or length, in a view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dimension {
    id: Id,
    view: Id,
    layout: Layout,
    references: Vec<Reference>,
    equalities: Vec<Equality>,
    ordinate: bool,
    style: Option<Id>,
    overrides: Overrides,
}

fn kind_of(layout: &Layout, ordinate: bool) -> Kind {
    match layout {
        Layout::Horizontal { .. } | Layout::Vertical { .. } | Layout::Aligned { .. } => {
            if ordinate {
                Kind::Ordinate
            } else {
                Kind::Linear
            }
        }
        Layout::Angular { .. } => Kind::Angular,
        Layout::Radial { .. } => Kind::Radial,
        Layout::Diameter { .. } => Kind::Diameter,
        Layout::ArcLength { .. } => Kind::ArcLength,
    }
}

fn check_references(
    layout: &Layout,
    ordinate: bool,
    references: &[Reference],
) -> Result<(), DimensionError> {
    if ordinate && !layout.is_line() {
        return Err(DimensionError::References(
            "An ordinate dimension lies along an offset line: horizontal, vertical or aligned."
                .to_owned(),
        ));
    }
    let (fewest, most) = layout.references();
    if fewest != most && references.len() < fewest {
        return Err(DimensionError::TooFewReferences(references.len()));
    }
    if references.len() > MAX_REFERENCES {
        return Err(DimensionError::TooManyReferences(references.len()));
    }
    if fewest == most && references.len() != fewest {
        let wanted = if fewest == 1 {
            "one reference"
        } else {
            "two references"
        };
        return Err(DimensionError::References(format!(
            "{} measures {wanted}; this has {}.",
            kind_of(layout, ordinate).words(),
            references.len()
        )));
    }
    for (at, pair) in references.windows(2).enumerate() {
        if pair.first() == pair.get(1) {
            return Err(DimensionError::RepeatedReference(at));
        }
    }
    Ok(())
}

impl Dimension {
    /// A dimension of `references`, in order, laid out as `layout`, in `view`, with no
    /// equalities.
    ///
    /// # Errors
    /// [`DimensionError::TooFewReferences`], [`DimensionError::TooManyReferences`] or
    /// [`DimensionError::RepeatedReference`] (two neighbours the same).
    pub fn new(
        id: Id,
        view: Id,
        layout: Layout,
        references: Vec<Reference>,
    ) -> Result<Self, DimensionError> {
        check_references(&layout, false, &references)?;
        Ok(Self {
            id,
            view,
            layout,
            references,
            equalities: Vec::new(),
            ordinate: false,
            style: None,
            overrides: Overrides::none(),
        })
    }

    /// An ordinate dimension of `references` along `layout`'s offset line: each value is the
    /// distance of a reference from the first, in the line's direction.
    ///
    /// # Errors
    /// [`DimensionError::References`] for a layout with no offset line; the rules of
    /// [`Dimension::new`].
    pub fn ordinate(
        id: Id,
        view: Id,
        layout: Layout,
        references: Vec<Reference>,
    ) -> Result<Self, DimensionError> {
        check_references(&layout, true, &references)?;
        Ok(Self {
            id,
            view,
            layout,
            references,
            equalities: Vec::new(),
            ordinate: true,
            style: None,
            overrides: Overrides::none(),
        })
    }

    /// What it measures.
    #[must_use]
    pub fn kind(&self) -> Kind {
        kind_of(&self.layout, self.ordinate)
    }

    /// Whether it is an ordinate dimension.
    #[must_use]
    pub fn is_ordinate(&self) -> bool {
        self.ordinate
    }

    /// The ID of the style it names; `None`: the first built-in style.
    #[must_use]
    pub fn style(&self) -> Option<&Id> {
        self.style.as_ref()
    }

    /// Its overrides of the shown text.
    #[must_use]
    pub fn overrides(&self) -> &Overrides {
        &self.overrides
    }

    /// The dimension in the style `style` (`None`, or the first built-in style's ID: the first
    /// built-in style, which is held as none so that a dimension has one spelling). Whether the
    /// style is there is the caller's to check against the document's styles.
    #[must_use]
    pub fn with_style(mut self, style: Option<Id>) -> Self {
        self.style = style.filter(|id| id.as_str() != crate::style::TICKS_ID);
        self
    }

    /// The dimension with these overrides instead of the ones it has.
    #[must_use]
    pub fn with_overrides(mut self, overrides: Overrides) -> Self {
        self.overrides = overrides;
        self
    }

    /// The dimension laid out otherwise, which may be of another kind (a radius made a
    /// diameter).
    ///
    /// # Errors
    /// The rules of [`Dimension::new`] for the new layout and the references it has; an
    /// ordinate dimension keeps an offset line; a dimension with equalities keeps a linear
    /// layout.
    pub fn relaid(mut self, layout: Layout) -> Result<Self, DimensionError> {
        check_references(&layout, self.ordinate, &self.references)?;
        if !self.equalities.is_empty() && !layout.is_line() {
            return Err(DimensionError::HasEqualities);
        }
        self.layout = layout;
        Ok(self)
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The view it is in.
    #[must_use]
    pub fn view(&self) -> &Id {
        &self.view
    }

    /// How it lies.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Its references, in the order they are measured.
    #[must_use]
    pub fn references(&self) -> &[Reference] {
        &self.references
    }

    /// How many segments it has: one fewer than its references.
    #[must_use]
    pub fn segments(&self) -> usize {
        self.references.len().saturating_sub(1)
    }

    /// Its equalities: sets of segments to be equal. Stored; enforced by no code yet (the crate's
    /// header), and reported by [`crate::Measured::equalities`].
    #[must_use]
    pub fn equalities(&self) -> &[Equality] {
        &self.equalities
    }

    /// The dimension with its offset line laid out otherwise. A layout that does not take the
    /// references it has (an angle for a string of three) is not taken, and the dimension is
    /// given back as it was: [`Dimension::relaid`] says why.
    #[must_use]
    pub fn with_layout(self, layout: Layout) -> Self {
        let kept = self.clone();
        self.relaid(layout).unwrap_or(kept)
    }

    /// The dimension with `reference` added at position `at` (0 puts it first; the number of
    /// references puts it last).
    ///
    /// # Errors
    /// [`DimensionError::HasEqualities`] when segments are held equal;
    /// [`DimensionError::NoSuchReference`] for a position past the end; the rules of
    /// [`Dimension::new`].
    pub fn with_reference_added(
        mut self,
        at: usize,
        reference: Reference,
    ) -> Result<Self, DimensionError> {
        self.string_only()?;
        if !self.equalities.is_empty() {
            return Err(DimensionError::HasEqualities);
        }
        if at > self.references.len() {
            return Err(DimensionError::NoSuchReference(at));
        }
        self.references.insert(at, reference);
        check_references(&self.layout, self.ordinate, &self.references)?;
        Ok(self)
    }

    /// Only a string along an offset line has references added and taken away.
    fn string_only(&self) -> Result<(), DimensionError> {
        if self.layout.is_line() {
            Ok(())
        } else {
            Err(DimensionError::References(format!(
                "{} keeps the references it was made with: make another to measure others.",
                self.kind().words()
            )))
        }
    }

    /// The dimension with the reference at position `at` taken away.
    ///
    /// # Errors
    /// [`DimensionError::HasEqualities`] when segments are held equal;
    /// [`DimensionError::NoSuchReference`] for a position that is not there; the rules of
    /// [`Dimension::new`] (a dimension keeps two references).
    pub fn with_reference_removed(mut self, at: usize) -> Result<Self, DimensionError> {
        self.string_only()?;
        if !self.equalities.is_empty() {
            return Err(DimensionError::HasEqualities);
        }
        if at >= self.references.len() {
            return Err(DimensionError::NoSuchReference(at));
        }
        self.references.remove(at);
        check_references(&self.layout, self.ordinate, &self.references)?;
        Ok(self)
    }

    /// The dimension with these equalities instead of the ones it has (none clears them).
    ///
    /// # Errors
    /// [`DimensionError::BadEquality`] for a group with fewer than two segments, a segment that
    /// is not there, a segment twice (in a group or in two), or groups or segments out of order.
    pub fn with_equalities(mut self, groups: Vec<Equality>) -> Result<Self, DimensionError> {
        let bad = |why: &str| Err(DimensionError::BadEquality(why.to_owned()));
        if !groups.is_empty() && self.kind() != Kind::Linear {
            return bad(&format!(
                "{} has no string of segments to hold equal",
                self.kind().words().to_lowercase()
            ));
        }
        let mut taken: Option<usize> = None;
        let mut last_first: Option<usize> = None;
        for group in &groups {
            if group.len() < 2 {
                return bad("a group needs at least two segments");
            }
            if group.windows(2).any(|pair| pair.first() >= pair.get(1)) {
                return bad("a group lists its segments once each, ascending");
            }
            let Some(&first) = group.first() else {
                return bad("a group needs at least two segments");
            };
            if last_first.is_some_and(|earlier| first <= earlier)
                || taken.is_some_and(|t| first <= t)
            {
                return bad("the groups are in order of their first segment and share none");
            }
            let Some(&last) = group.last() else {
                return bad("a group needs at least two segments");
            };
            if last >= self.segments() {
                return Err(DimensionError::BadEquality(format!(
                    "segment {} is not there: this dimension has {}",
                    last.saturating_add(1),
                    self.segments()
                )));
            }
            last_first = Some(first);
            taken = Some(last);
        }
        self.equalities = groups;
        Ok(self)
    }

    /// The dimension as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), DIMENSIONS.0.to_string());
        props.insert(VIEW_KEY.to_owned(), self.view.to_string());
        props.insert(
            LAYOUT_KEY.to_owned(),
            Value::Choice(self.layout.key().to_owned()).stored(),
        );
        let put = |props: &mut BTreeMap<String, String>, key: &str, length: Length| {
            props.insert(key.to_owned(), Value::Length(length).stored());
        };
        let choice = |props: &mut BTreeMap<String, String>, key: &str, ray: Ray| {
            props.insert(key.to_owned(), Value::Choice(ray.key().to_owned()).stored());
        };
        match self.layout {
            Layout::Horizontal { y } => put(&mut props, Y_KEY, y),
            Layout::Vertical { x } => put(&mut props, X_KEY, x),
            Layout::Aligned { through, direction } => {
                put(&mut props, THROUGH_X_KEY, through.x);
                put(&mut props, THROUGH_Y_KEY, through.y);
                put(&mut props, DIRECTION_X_KEY, direction.dx());
                put(&mut props, DIRECTION_Y_KEY, direction.dy());
            }
            Layout::Angular {
                first,
                second,
                radius,
            } => {
                choice(&mut props, FIRST_KEY, first);
                choice(&mut props, SECOND_KEY, second);
                put(&mut props, RADIUS_KEY, radius);
            }
            Layout::Radial { toward } | Layout::Diameter { toward } => {
                put(&mut props, TOWARD_X_KEY, toward.dx());
                put(&mut props, TOWARD_Y_KEY, toward.dy());
            }
            Layout::ArcLength { offset } => put(&mut props, OFFSET_KEY, offset),
        }
        if !self.equalities.is_empty() {
            props.insert(
                EQUAL_KEY.to_owned(),
                Value::Text(equal_text(&self.equalities)).stored(),
            );
        }
        if self.ordinate {
            props.insert(ORDINATE_KEY.to_owned(), Value::YesNo(true).stored());
        }
        if let Some(style) = &self.style {
            props.insert(STYLE_KEY.to_owned(), style.to_string());
        }
        for (key, part) in [
            (TEXT_KEY, self.overrides.text()),
            (PREFIX_KEY, self.overrides.prefix()),
            (SUFFIX_KEY, self.overrides.suffix()),
        ] {
            if let Some(part) = part {
                props.insert(key.to_owned(), Value::Text(part.to_owned()).stored());
            }
        }
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: DIMENSION_KIND.to_owned(),
            name: DIMENSION_NAME.to_owned(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: dimension_recipe(),
            props,
            references: self.references.iter().map(Reference::to_model).collect(),
        }
    }

    /// Reads a dimension from its element record. Anything [`Dimension::to_record`] would not
    /// write is refused, never repaired.
    ///
    /// # Errors
    /// [`DimensionError::Malformed`], naming the record, for a record that is not a dimension
    /// record as written.
    pub fn from_record(record: &Element) -> Result<Self, DimensionError> {
        let malformed = |why: &str| {
            DimensionError::Malformed(format!("The dimension record {} {why}.", record.id))
        };
        if record.kind != DIMENSION_KIND || record.recipe != dimension_recipe() {
            return Err(malformed(
                "is not of kind dimension, with the dimension recipe",
            ));
        }
        if record.level != 0 || record.placement != NO_PLACEMENT {
            return Err(malformed("has a level or a placement"));
        }
        let view = record
            .props
            .get(VIEW_KEY)
            .and_then(|text| Id::parse(text).ok())
            .ok_or_else(|| malformed("holds no view ID"))?;
        let layout = layout_of_record(record, &malformed)?;
        let references = record
            .references
            .iter()
            .map(Reference::from_model)
            .collect::<Result<Vec<_>, _>>()?;
        let ordinate = match record.props.get(ORDINATE_KEY) {
            None => false,
            Some(text) if Value::from_stored(text) == Some(Value::YesNo(true)) => true,
            Some(_) => return Err(malformed("holds an ordinate mark that is not yesno:yes")),
        };
        let made = if ordinate {
            Self::ordinate(record.id.clone(), view, layout, references)
        } else {
            Self::new(record.id.clone(), view, layout, references)
        };
        let mut dimension = made.map_err(|e| malformed(&e.to_string()))?;
        if let Some(text) = record.props.get(STYLE_KEY) {
            let style =
                Id::parse(text).map_err(|_| malformed("holds a style that is not an ID"))?;
            dimension = dimension.with_style(Some(style));
        }
        dimension = dimension.with_overrides(overrides_of_record(record, &malformed)?);
        if let Some(text) = record.props.get(EQUAL_KEY) {
            let Some(Value::Text(spelled)) = Value::from_stored(text) else {
                return Err(malformed("holds equalities that are not text"));
            };
            let groups = parse_equal(&spelled).ok_or_else(|| {
                malformed("holds equalities that are not groups of segment numbers")
            })?;
            dimension = dimension
                .with_equalities(groups)
                .map_err(|e| malformed(&e.to_string()))?;
        }
        if dimension.to_record(&record.document) != *record {
            return Err(malformed("is not as a dimension record is written"));
        }
        Ok(dimension)
    }
}

/// A dimension record's layout, read as [`Dimension::to_record`] writes it.
fn layout_of_record(
    record: &Element,
    malformed: &dyn Fn(&str) -> DimensionError,
) -> Result<Layout, DimensionError> {
    let length = |key: &str| -> Result<Length, DimensionError> {
        let text = record
            .props
            .get(key)
            .ok_or_else(|| malformed(&format!("holds no {key}")))?;
        match Value::from_stored(text).filter(|value| value.stored() == *text) {
            Some(Value::Length(length)) => Ok(length),
            _ => Err(malformed(&format!(
                "holds {key} that is not in its one spelling"
            ))),
        }
    };
    let Some(Value::Choice(layout_key)) = record
        .props
        .get(LAYOUT_KEY)
        .and_then(|text| Value::from_stored(text))
    else {
        return Err(malformed("holds no layout"));
    };
    let ray = |key: &str| -> Result<Ray, DimensionError> {
        match record
            .props
            .get(key)
            .and_then(|text| Value::from_stored(text))
        {
            Some(Value::Choice(found)) => Ray::from_key(&found)
                .ok_or_else(|| malformed(&format!("holds {key} that is not forward or backward"))),
            _ => Err(malformed(&format!("holds no {key}"))),
        }
    };
    let toward = || Direction::new(length(TOWARD_X_KEY)?, length(TOWARD_Y_KEY)?);
    match layout_key.as_str() {
        "horizontal" => Layout::horizontal(length(Y_KEY)?),
        "vertical" => Layout::vertical(length(X_KEY)?),
        "aligned" => Direction::new(length(DIRECTION_X_KEY)?, length(DIRECTION_Y_KEY)?).and_then(
            |direction| {
                Layout::aligned(
                    Point::new(length(THROUGH_X_KEY)?, length(THROUGH_Y_KEY)?),
                    direction,
                )
            },
        ),
        "angular" => Layout::angular(ray(FIRST_KEY)?, ray(SECOND_KEY)?, length(RADIUS_KEY)?),
        "radial" => toward().map(Layout::radial),
        "diameter" => toward().map(Layout::diameter),
        "arc_length" => Layout::arc_length(length(OFFSET_KEY)?),
        _ => {
            return Err(malformed(
                "holds a layout that is not horizontal, vertical, aligned, angular, radial, \
                 diameter or arc_length",
            ));
        }
    }
    .map_err(|e| malformed(&e.to_string()))
}

/// A dimension record's overrides, read as [`Dimension::to_record`] writes them.
fn overrides_of_record(
    record: &Element,
    malformed: &dyn Fn(&str) -> DimensionError,
) -> Result<Overrides, DimensionError> {
    let part = |key: &str| -> Result<Option<String>, DimensionError> {
        match record.props.get(key).map(|text| Value::from_stored(text)) {
            None => Ok(None),
            Some(Some(Value::Text(found))) => Ok(Some(found)),
            Some(_) => Err(malformed(&format!("holds {key} that is not text"))),
        }
    };
    Overrides::new(part(TEXT_KEY)?, part(PREFIX_KEY)?, part(SUFFIX_KEY)?)
        .map_err(|e| malformed(&e.to_string()))
}

/// `0,1;3,4`: the groups' segment numbers.
fn equal_text(groups: &[Equality]) -> String {
    groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn parse_equal(text: &str) -> Option<Vec<Equality>> {
    text.split(';')
        .map(|group| {
            group
                .split(',')
                .map(|number| number.parse::<usize>().ok())
                .collect::<Option<Vec<_>>>()
        })
        .collect()
}
