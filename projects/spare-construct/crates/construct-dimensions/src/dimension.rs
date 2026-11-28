//! One dimension: its ID, the view it is in, how it lies, its references and its equalities,
//! written as an element record of the dimensions category and read back (FORMAT.md beside this
//! crate).
//!
//! A dimension holds no value. Its value is worked out from its references each time it is asked
//! ([`crate::measure`]), so it is never out of date.

use crate::error::DimensionError;
use crate::layout::{Direction, Layout};
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
/// The most references a dimension string holds.
pub const MAX_REFERENCES: usize = 64;

const NO_PLACEMENT: Placement = Placement { x: 0, y: 0, z: 0 };

/// The recipe of a dimension record: Spare Construct's own operation `dimension`, version 1, with
/// no arguments. It builds no shape.
#[must_use]
pub fn dimension_recipe() -> Recipe {
    Recipe::construct(DIMENSION_KIND, BTreeMap::new())
}

/// A set of segments that are to be equal: their numbers, from 0, ascending, at least two.
pub type Equality = Vec<usize>;

/// A linear dimension: a string of segments between references, in a view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dimension {
    id: Id,
    view: Id,
    layout: Layout,
    references: Vec<Reference>,
    equalities: Vec<Equality>,
}

fn check_references(references: &[Reference]) -> Result<(), DimensionError> {
    if references.len() < 2 {
        return Err(DimensionError::TooFewReferences(references.len()));
    }
    if references.len() > MAX_REFERENCES {
        return Err(DimensionError::TooManyReferences(references.len()));
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
        check_references(&references)?;
        Ok(Self {
            id,
            view,
            layout,
            references,
            equalities: Vec::new(),
        })
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

    /// The dimension with its offset line laid out otherwise.
    #[must_use]
    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.layout = layout;
        self
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
        if !self.equalities.is_empty() {
            return Err(DimensionError::HasEqualities);
        }
        if at > self.references.len() {
            return Err(DimensionError::NoSuchReference(at));
        }
        self.references.insert(at, reference);
        check_references(&self.references)?;
        Ok(self)
    }

    /// The dimension with the reference at position `at` taken away.
    ///
    /// # Errors
    /// [`DimensionError::HasEqualities`] when segments are held equal;
    /// [`DimensionError::NoSuchReference`] for a position that is not there; the rules of
    /// [`Dimension::new`] (a dimension keeps two references).
    pub fn with_reference_removed(mut self, at: usize) -> Result<Self, DimensionError> {
        if !self.equalities.is_empty() {
            return Err(DimensionError::HasEqualities);
        }
        if at >= self.references.len() {
            return Err(DimensionError::NoSuchReference(at));
        }
        self.references.remove(at);
        check_references(&self.references)?;
        Ok(self)
    }

    /// The dimension with these equalities instead of the ones it has (none clears them).
    ///
    /// # Errors
    /// [`DimensionError::BadEquality`] for a group with fewer than two segments, a segment that
    /// is not there, a segment twice (in a group or in two), or groups or segments out of order.
    pub fn with_equalities(mut self, groups: Vec<Equality>) -> Result<Self, DimensionError> {
        let bad = |why: &str| Err(DimensionError::BadEquality(why.to_owned()));
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
        match self.layout {
            Layout::Horizontal { y } => put(&mut props, Y_KEY, y),
            Layout::Vertical { x } => put(&mut props, X_KEY, x),
            Layout::Aligned { through, direction } => {
                put(&mut props, THROUGH_X_KEY, through.x);
                put(&mut props, THROUGH_Y_KEY, through.y);
                put(&mut props, DIRECTION_X_KEY, direction.dx());
                put(&mut props, DIRECTION_Y_KEY, direction.dy());
            }
        }
        if !self.equalities.is_empty() {
            props.insert(
                EQUAL_KEY.to_owned(),
                Value::Text(equal_text(&self.equalities)).stored(),
            );
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
        let layout = match layout_key.as_str() {
            "horizontal" => Layout::horizontal(length(Y_KEY)?),
            "vertical" => Layout::vertical(length(X_KEY)?),
            "aligned" => Direction::new(length(DIRECTION_X_KEY)?, length(DIRECTION_Y_KEY)?)
                .and_then(|direction| {
                    Layout::aligned(
                        Point::new(length(THROUGH_X_KEY)?, length(THROUGH_Y_KEY)?),
                        direction,
                    )
                }),
            _ => {
                return Err(malformed(
                    "holds a layout that is not horizontal, vertical or aligned",
                ));
            }
        }
        .map_err(|e| malformed(&e.to_string()))?;
        let references = record
            .references
            .iter()
            .map(Reference::from_model)
            .collect::<Result<Vec<_>, _>>()?;
        let mut dimension = Self::new(record.id.clone(), view, layout, references)
            .map_err(|e| malformed(&e.to_string()))?;
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
