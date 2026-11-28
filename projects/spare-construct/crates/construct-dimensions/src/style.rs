//! Dimension styles: how a dimension is drawn and how its value reads, as project data. A style
//! names the mark at each witness line (a tick, an arrow or a dot), where the text sits (above the
//! line or in it), the units and places its values are shown in (construct-params' own formats;
//! none means the project's), how far a witness line stops short of what it measures and how far
//! it runs past the dimension's line, how high the text is, and whether a radius or a diameter
//! marks its centre. Sizes are as printed on paper; a drawing scales them to its view's scale.
//!
//! Two built-in styles are always there and cannot be changed or taken away
//! ([`DimensionStyle::builtin`]); a project adds its own as element records of the dimensions
//! category with no shape (FORMAT.md beside this crate), each with a name unique among the
//! project's styles and the built-in ones, case aside. A dimension names its style by ID; one that
//! names none is drawn in the first built-in style.

use crate::dimension::{CATEGORY_KEY, NO_PLACEMENT};
use crate::error::DimensionError;
use construct_categories::DIMENSIONS;
use construct_model::element::Element;
use construct_model::geometry::Recipe;
use construct_params::{Format, Kind, Length, Unit, Units, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// The `kind` of every dimension style record, and its recipe's operation.
pub const STYLE_KIND: &str = "dimension.style";
/// The mark at each witness line: `choice:tick`, `choice:arrow` or `choice:dot`.
pub const MARKER_KEY: &str = "@dimension.style.marker";
/// Where the text sits: `choice:above` or `choice:inline`.
pub const TEXT_PLACE_KEY: &str = "@dimension.style.text";
/// The unit lengths are shown in, `choice:mm` or `choice:m`, with [`LENGTH_PLACES_KEY`].
pub const LENGTH_UNIT_KEY: &str = "@dimension.style.length.unit";
/// The places lengths are shown to, a whole number.
pub const LENGTH_PLACES_KEY: &str = "@dimension.style.length.places";
/// The places angles are shown to in degrees, a whole number.
pub const ANGLE_PLACES_KEY: &str = "@dimension.style.angle.places";
/// How far a witness line stops short of its reference, on paper, a stored length.
pub const GAP_KEY: &str = "@dimension.style.gap";
/// How far a witness line runs past the dimension's line, on paper, a stored length.
pub const EXTENSION_KEY: &str = "@dimension.style.extension";
/// How high the text is, on paper, a stored length.
pub const HEIGHT_KEY: &str = "@dimension.style.height";
/// Whether a radius or a diameter marks its arc's centre: `yesno:yes` or `yesno:no`.
pub const CENTRE_KEY: &str = "@dimension.style.centre";
/// The ID of the first built-in style, the one a dimension that names none is drawn in.
pub const TICKS_ID: &str = "dimension-style.ticks";
/// The ID of the second built-in style.
pub const ARROWS_ID: &str = "dimension-style.arrows";
/// The most characters a style's name holds.
pub const MAX_NAME: usize = 80;
/// The largest gap, extension or text height a style takes, on paper: 50 mm.
pub const MOST_SIZE: Length = Length::from_mm(50);

/// The recipe of a style record: Spare Construct's own operation `dimension.style`, version 1,
/// with no arguments. It builds no shape.
#[must_use]
pub fn style_recipe() -> Recipe {
    Recipe::construct(STYLE_KIND, BTreeMap::new())
}

/// The mark drawn where each witness line meets the dimension's line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Marker {
    /// A short slash across the line.
    Tick,
    /// An arrowhead pointing at the witness line.
    Arrow,
    /// A filled dot.
    Dot,
}

impl Marker {
    /// Every marker.
    pub const ALL: [Self; 3] = [Self::Tick, Self::Arrow, Self::Dot];

    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Tick => "tick",
            Self::Arrow => "arrow",
            Self::Dot => "dot",
        }
    }

    /// The marker a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }
}

/// Where a dimension's text sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextPlace {
    /// Above the dimension's line, turned to read along it.
    Above,
    /// In the line, which is broken for it.
    Inline,
}

impl TextPlace {
    /// Every place.
    pub const ALL: [Self; 2] = [Self::Above, Self::Inline];

    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Above => "above",
            Self::Inline => "inline",
        }
    }

    /// The place a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == key)
    }
}

/// Everything a style says, apart from its ID and name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    /// The mark at each witness line.
    pub marker: Marker,
    /// Where the text sits.
    pub text: TextPlace,
    /// How lengths are shown; `None`: as the project shows them.
    pub length: Option<Format>,
    /// How many places angles are shown to, in degrees; `None`: as the project shows them.
    pub angle_places: Option<u8>,
    /// How far a witness line stops short of its reference, on paper.
    pub gap: Length,
    /// How far a witness line runs past the dimension's line, on paper.
    pub extension: Length,
    /// How high the text is, on paper.
    pub height: Length,
    /// Whether a radius or a diameter marks its arc's centre.
    pub centre_mark: bool,
}

impl Look {
    /// The first built-in style's look: ticks, the text above the line, the project's units, a
    /// 1.5 mm gap, a 2 mm extension, 2.5 mm text and a centre mark.
    #[must_use]
    pub const fn ticks() -> Self {
        Self {
            marker: Marker::Tick,
            text: TextPlace::Above,
            length: None,
            angle_places: None,
            gap: Length::from_steps(9_600),
            extension: Length::from_mm(2),
            height: Length::from_steps(16_000),
            centre_mark: true,
        }
    }

    /// The units values are shown in: the project's, with this look's own formats for lengths
    /// and angles where it has them.
    #[must_use]
    pub fn units(&self, project: &Units) -> Units {
        let mut units = *project;
        if let Some(format) = self.length {
            // A look is checked when a style is made, so its format is one a length takes; an
            // unchecked one leaves the project's.
            let _ = units.set(Kind::Length, format);
        }
        if let Some(places) = self.angle_places {
            let _ = units.set(
                Kind::Angle,
                Format {
                    unit: Unit::Degree,
                    places,
                },
            );
        }
        units
    }

    fn check(&self) -> Result<(), DimensionError> {
        let bad = |why: String| Err(DimensionError::BadStyle(why));
        if let Some(format) = self.length {
            let mut units = Units::metric();
            if units.set(Kind::Length, format).is_err() {
                return bad(format!(
                    "A style shows lengths in mm or m, to at most the unit's places: not {} to {} places.",
                    format.unit.symbol(),
                    format.places
                ));
            }
        }
        if let Some(places) = self.angle_places {
            let mut units = Units::metric();
            let format = Format {
                unit: Unit::Degree,
                places,
            };
            if units.set(Kind::Angle, format).is_err() {
                return bad(format!(
                    "A style shows angles in degrees to at most {} places, not {places}.",
                    Unit::Degree.max_places()
                ));
            }
        }
        for (what, size, least) in [
            ("gap", self.gap, 0),
            ("extension", self.extension, 0),
            ("text height", self.height, 1),
        ] {
            if size.steps() < least || size > MOST_SIZE {
                let floor = if least == 0 { "not below 0" } else { "above 0" };
                return bad(format!(
                    "A style's {what} is {floor} and at most {MOST_SIZE} on paper, not {size}."
                ));
            }
        }
        Ok(())
    }
}

/// A dimension style: an ID, a name and a [`Look`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DimensionStyle {
    id: Id,
    name: String,
    look: Look,
}

impl DimensionStyle {
    /// A style.
    ///
    /// # Errors
    /// [`DimensionError::BadStyle`] for a name that is empty, has a space at an end or is too
    /// long, an ID that is a built-in style's, and a look with a unit a length is not shown in,
    /// too many places, or a size out of range.
    pub fn new(id: Id, name: &str, look: Look) -> Result<Self, DimensionError> {
        if is_builtin(&id) {
            return Err(DimensionError::BadStyle(format!(
                "{id} is a built-in style's ID."
            )));
        }
        Self::checked(id, name, look)
    }

    fn checked(id: Id, name: &str, look: Look) -> Result<Self, DimensionError> {
        if name.is_empty() || name.trim() != name || name.chars().count() > MAX_NAME {
            return Err(DimensionError::BadStyle(format!(
                "A style's name has 1 to {MAX_NAME} characters and no space at either end."
            )));
        }
        look.check()?;
        Ok(Self {
            id,
            name: name.to_owned(),
            look,
        })
    }

    /// The two built-in styles: "Ticks" (the first, and the one a dimension that names none is
    /// drawn in) and "Arrows" (the same with arrowheads). Our own: ticks are what a building
    /// drawing in metric units usually shows, arrows what a detail often does. (Both IDs are
    /// letters, a dash and a full stop, which every ID takes; a test pins that both are here.)
    #[must_use]
    pub fn builtin() -> Vec<Self> {
        let ticks = Look::ticks();
        let arrows = Look {
            marker: Marker::Arrow,
            ..ticks
        };
        [(TICKS_ID, "Ticks", ticks), (ARROWS_ID, "Arrows", arrows)]
            .into_iter()
            .filter_map(|(id, name, look)| {
                Id::parse(id).ok().map(|id| Self {
                    id,
                    name: name.to_owned(),
                    look,
                })
            })
            .collect()
    }

    /// Its ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Its name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What it says.
    #[must_use]
    pub fn look(&self) -> &Look {
        &self.look
    }

    /// Whether it is a built-in one.
    #[must_use]
    pub fn is_builtin(&self) -> bool {
        is_builtin(&self.id)
    }

    /// The style renamed.
    ///
    /// # Errors
    /// As [`DimensionStyle::new`].
    pub fn renamed(&self, name: &str) -> Result<Self, DimensionError> {
        self.refuse_builtin()?;
        Self::checked(self.id.clone(), name, self.look)
    }

    /// The style with another look.
    ///
    /// # Errors
    /// As [`DimensionStyle::new`].
    pub fn with_look(&self, look: Look) -> Result<Self, DimensionError> {
        self.refuse_builtin()?;
        Self::checked(self.id.clone(), &self.name, look)
    }

    fn refuse_builtin(&self) -> Result<(), DimensionError> {
        if self.is_builtin() {
            return Err(DimensionError::BadStyle(format!(
                "{} is a built-in style: it is not changed. Make a style of your own from it.",
                self.name
            )));
        }
        Ok(())
    }

    /// The style as an element record of `document` (FORMAT.md beside this crate). A built-in
    /// style is never a record: its record is the one it would have.
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let look = &self.look;
        let mut props = BTreeMap::new();
        props.insert(CATEGORY_KEY.to_owned(), DIMENSIONS.0.to_string());
        let choice = |key: &str| Value::Choice(key.to_owned()).stored();
        props.insert(MARKER_KEY.to_owned(), choice(look.marker.key()));
        props.insert(TEXT_PLACE_KEY.to_owned(), choice(look.text.key()));
        if let Some(format) = look.length {
            props.insert(LENGTH_UNIT_KEY.to_owned(), choice(format.unit.symbol()));
            props.insert(
                LENGTH_PLACES_KEY.to_owned(),
                Value::Whole(i64::from(format.places)).stored(),
            );
        }
        if let Some(places) = look.angle_places {
            props.insert(
                ANGLE_PLACES_KEY.to_owned(),
                Value::Whole(i64::from(places)).stored(),
            );
        }
        props.insert(GAP_KEY.to_owned(), Value::Length(look.gap).stored());
        props.insert(
            EXTENSION_KEY.to_owned(),
            Value::Length(look.extension).stored(),
        );
        props.insert(HEIGHT_KEY.to_owned(), Value::Length(look.height).stored());
        props.insert(
            CENTRE_KEY.to_owned(),
            Value::YesNo(look.centre_mark).stored(),
        );
        Element {
            id: self.id.clone(),
            document: document.clone(),
            kind: STYLE_KIND.to_owned(),
            name: self.name.clone(),
            level: 0,
            placement: NO_PLACEMENT,
            recipe: style_recipe(),
            props,
            references: Vec::new(),
        }
    }

    /// Reads a style from its element record. Anything [`DimensionStyle::to_record`] would not
    /// write is refused, never repaired; so is a record with a built-in style's ID.
    ///
    /// # Errors
    /// [`DimensionError::Malformed`], naming the record.
    pub fn from_record(record: &Element) -> Result<Self, DimensionError> {
        let malformed = |why: &str| {
            DimensionError::Malformed(format!("The dimension style record {} {why}.", record.id))
        };
        if record.kind != STYLE_KIND || record.recipe != style_recipe() {
            return Err(malformed(
                "is not of kind dimension.style, with the dimension style recipe",
            ));
        }
        let value = |key: &str| -> Result<Option<Value>, DimensionError> {
            match record.props.get(key) {
                None => Ok(None),
                Some(text) => Value::from_stored(text)
                    .filter(|value| value.stored() == *text)
                    .map(Some)
                    .ok_or_else(|| {
                        malformed(&format!("holds {key} that is not in its one spelling"))
                    }),
            }
        };
        let choice = |key: &str| match value(key)? {
            Some(Value::Choice(found)) => Ok(found),
            _ => Err(malformed(&format!("holds no choice of {key}"))),
        };
        let length = |key: &str| match value(key)? {
            Some(Value::Length(found)) => Ok(found),
            _ => Err(malformed(&format!("holds no length {key}"))),
        };
        let places = |key: &str| match value(key)? {
            None => Ok(None),
            Some(Value::Whole(n)) => u8::try_from(n)
                .map(Some)
                .map_err(|_| malformed(&format!("holds {key} that is not a number of places"))),
            Some(_) => Err(malformed(&format!(
                "holds {key} that is not a whole number"
            ))),
        };
        let marker = Marker::from_key(&choice(MARKER_KEY)?)
            .ok_or_else(|| malformed("holds a marker that is not tick, arrow or dot"))?;
        let text = TextPlace::from_key(&choice(TEXT_PLACE_KEY)?)
            .ok_or_else(|| malformed("holds a text place that is not above or inline"))?;
        let length_format = match (value(LENGTH_UNIT_KEY)?, places(LENGTH_PLACES_KEY)?) {
            (None, None) => None,
            (Some(Value::Choice(symbol)), Some(places)) => {
                let unit = Unit::ALL
                    .into_iter()
                    .find(|unit| unit.kind() == Kind::Length && unit.symbol() == symbol)
                    .ok_or_else(|| malformed("holds a length unit that is not one"))?;
                Some(Format { unit, places })
            }
            _ => {
                return Err(malformed(
                    "holds a length unit without its places, or places without it",
                ));
            }
        };
        let Some(Value::YesNo(centre_mark)) = value(CENTRE_KEY)? else {
            return Err(malformed("holds no yes or no for its centre mark"));
        };
        let look = Look {
            marker,
            text,
            length: length_format,
            angle_places: places(ANGLE_PLACES_KEY)?,
            gap: length(GAP_KEY)?,
            extension: length(EXTENSION_KEY)?,
            height: length(HEIGHT_KEY)?,
            centre_mark,
        };
        let style = Self::new(record.id.clone(), &record.name, look)
            .map_err(|e| malformed(&e.to_string()))?;
        if style.to_record(&record.document) != *record {
            return Err(malformed("is not as a dimension style record is written"));
        }
        Ok(style)
    }
}

/// Whether an ID is a built-in style's.
#[must_use]
pub fn is_builtin(id: &Id) -> bool {
    id.as_str() == TICKS_ID || id.as_str() == ARROWS_ID
}

/// A document's dimension styles, with the built-in ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Styles {
    builtin: Vec<DimensionStyle>,
    own: BTreeMap<Id, DimensionStyle>,
}

impl Default for Styles {
    fn default() -> Self {
        Self::new()
    }
}

impl Styles {
    /// The built-in styles only.
    #[must_use]
    pub fn new() -> Self {
        Self {
            builtin: DimensionStyle::builtin(),
            own: BTreeMap::new(),
        }
    }

    /// The built-in styles and those among `records` (the document's records of every kind).
    ///
    /// # Errors
    /// [`DimensionError::Malformed`] for a style record that is not as written.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, DimensionError> {
        let mut found = Self::new();
        for record in records {
            if record.kind == STYLE_KIND {
                let style = DimensionStyle::from_record(record)?;
                found.own.insert(style.id.clone(), style);
            }
        }
        Ok(found)
    }

    /// The style with this ID: a built-in one or the document's.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&DimensionStyle> {
        self.builtin
            .iter()
            .find(|style| style.id == *id)
            .or_else(|| self.own.get(id))
    }

    /// The look of the style a dimension naming `id` (or none) is drawn in: the first built-in
    /// style's when it names none, or one that is not there.
    #[must_use]
    pub fn look_of(&self, id: Option<&Id>) -> Look {
        id.and_then(|id| self.get(id))
            .map_or_else(Look::ticks, |style| style.look)
    }

    /// Adds a document's style.
    ///
    /// # Errors
    /// [`DimensionError::BadStyle`] when its ID is taken or its name is another style's, case
    /// aside.
    pub fn add(&mut self, style: DimensionStyle) -> Result<(), DimensionError> {
        if self.get(&style.id).is_some() {
            return Err(DimensionError::BadStyle(format!(
                "There is already a style {}.",
                style.id
            )));
        }
        self.name_free(&style)?;
        self.own.insert(style.id.clone(), style);
        Ok(())
    }

    /// Replaces a document's style by one of the same ID.
    ///
    /// # Errors
    /// [`DimensionError::BadStyle`] for a built-in style, one that is not there, or a name that
    /// is another style's.
    pub fn replace(&mut self, style: DimensionStyle) -> Result<(), DimensionError> {
        if !self.own.contains_key(&style.id) {
            return Err(DimensionError::BadStyle(format!(
                "There is no style {} of this model's own.",
                style.id
            )));
        }
        self.name_free(&style)?;
        self.own.insert(style.id.clone(), style);
        Ok(())
    }

    fn name_free(&self, style: &DimensionStyle) -> Result<(), DimensionError> {
        match self.named(&style.name) {
            Some(other) if other.id != style.id => Err(DimensionError::BadStyle(format!(
                "There is already a style named {}.",
                other.name
            ))),
            _ => Ok(()),
        }
    }

    /// Every style: the built-in ones, then the document's by ID.
    #[must_use]
    pub fn all(&self) -> Vec<&DimensionStyle> {
        self.builtin.iter().chain(self.own.values()).collect()
    }

    /// The style whose name is `name`, case aside.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<&DimensionStyle> {
        let wanted = name.to_lowercase();
        self.all()
            .into_iter()
            .find(|style| style.name.to_lowercase() == wanted)
    }
}
